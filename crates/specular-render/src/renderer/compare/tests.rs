use specular_core::hdr::{f16_to_f32, f32_to_f16};
use specular_core::{Orientation, PixelLayout, Raster, RasterPlane};
use specular_gfx::{Gpu, HDR, wgpu};
use specular_inspect::quality::{DE_BINS, de_bin, de_edge, ssim_luma};
use specular_inspect::{
    ChannelView, DiffKind, DiffStats, MetricSpace, delta_e_reference, diff_colour, reference,
    reference_in, srgb_decode, ssim_reference,
};

use super::*;
use crate::renderer::GpuImage;

fn validate(label: &str, src: &str) {
    let module = naga::front::wgsl::parse_str(src).unwrap_or_else(|e| {
        panic!("{label}: {}", e.emit_to_string(src));
    });
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("{label}: {e:?}"));
}

#[test]
fn shaders_validate() {
    validate("compare diff", &diff_source());
    validate("compare reduce", &reduce_source());
}

/// An RGBA raster of `w` × `h` from `px(x, y)`.
fn raster(w: u32, h: u32, px: impl Fn(u32, u32) -> [u8; 4]) -> Raster {
    let mut data = Vec::with_capacity(w as usize * h as usize * 4);
    for y in 0..h {
        for x in 0..w {
            data.extend_from_slice(&px(x, y));
        }
    }
    Raster {
        width: w,
        height: h,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Rgba,
        planes: vec![RasterPlane {
            width: w,
            height: h,
            stride: w as usize * 4,
            data,
            h_div: 1,
            v_div: 1,
        }],
        truncated: false,
    }
}

/// The pixel of `r` as the renderer draws it over nothing: linear, premultiplied, through
/// the half-float target (the sample targets are [`HDR`]); `channel` as the shader isolates.
fn drawn(r: &Raster, x: u32, y: u32, channel: ChannelView) -> [f32; 4] {
    let i = (y * r.width + x) as usize * 4;
    let p = &r.planes[0].data[i..i + 4];
    let lin = |c: u8| srgb_decode(c as f32 / 255.0);
    let a = p[3] as f32 / 255.0;
    let half = |v: f32| f16_to_f32(f32_to_f16(v));
    let v = match channel {
        ChannelView::Red => Some(lin(p[0])),
        ChannelView::Green => Some(lin(p[1])),
        ChannelView::Blue => Some(lin(p[2])),
        _ => None,
    };
    match v {
        Some(v) => [half(v), half(v), half(v), 1.0],
        None => [
            half(lin(p[0]) * a),
            half(lin(p[1]) * a),
            half(lin(p[2]) * a),
            half(a),
        ],
    }
}

/// The CPU reference over the top-left `w` × `h` of both rasters.
fn cpu(a: &Raster, b: &Raster, w: u32, h: u32, channel: ChannelView, alpha: bool) -> DiffStats {
    let mut pa = Vec::new();
    let mut pb = Vec::new();
    for y in 0..h {
        for x in 0..w {
            pa.push(drawn(a, x, y, channel));
            pb.push(drawn(b, x, y, channel));
        }
    }
    reference(&pa, &pb, alpha)
}

struct Rig {
    gpu: Gpu,
    renderer: ImageRenderer,
    images: GpuImageCache,
    cmp: Comparer,
}

fn rig(rasters: &[&Raster]) -> Option<Rig> {
    let gpu = match Gpu::new() {
        Ok(g) => g,
        Err(e) if std::env::var_os("SPECULAR_REQUIRE_GPU").is_none() => {
            eprintln!("no GPU, skipped: {e}");
            return None;
        }
        Err(e) => panic!("SPECULAR_REQUIRE_GPU is set but there is no adapter: {e}"),
    };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let mut images = GpuImageCache::default();
    for (k, r) in rasters.iter().enumerate() {
        let mut img: GpuImage = renderer.create_image(&gpu, k as u64 + 1, r.width, r.height, None);
        renderer.upload_overview(&gpu, &mut img, r);
        images.insert(img);
    }
    let cmp = Comparer::spawn(&gpu, || {}).wait().expect("comparer");
    Some(Rig {
        gpu,
        renderer,
        images,
        cmp,
    })
}

fn image(id: u64, r: &Raster) -> MetricImage {
    MetricImage {
        id,
        stored: (r.width, r.height),
        orient: Orientation::NORMAL,
        place: Place::IDENTITY,
    }
}

fn measure(rig: &mut Rig, pairs: &[MetricPair]) -> MetricsReadback {
    let mut enc = rig.gpu.device.create_command_encoder(&Default::default());
    assert!(rig.cmp.request(
        &rig.gpu,
        &mut rig.renderer,
        &mut rig.images,
        &mut enc,
        pairs
    ));
    assert!(rig.cmp.busy());
    // A second request while one is in flight is refused.
    assert!(!rig.cmp.request(
        &rig.gpu,
        &mut rig.renderer,
        &mut rig.images,
        &mut enc,
        pairs
    ));
    rig.gpu.queue.submit([enc.finish()]);
    rig.cmp.after_submit();
    loop {
        if let Some(out) = rig.cmp.poll(&rig.gpu) {
            assert!(!rig.cmp.busy());
            return out;
        }
        let _ = rig.gpu.device.poll(wgpu::PollType::wait_indefinitely());
    }
}

/// The GPU's metrics against the CPU reference: `exact` when the reference was computed from
/// the very samples the GPU reduced (only the summation order differs), else from the
/// rasters' own values (which the GPU's sRGB decode and half-float rounding move by a few
/// parts in ten thousand).
fn assert_close(got: &DiffStats, want: &DiffStats, exact: bool) {
    assert_eq!(got.pixels, want.pixels);
    let tol = if exact { 1e-5 } else { 3e-3 };
    let close = |g: f64, w: f64| (g - w).abs() <= tol * w.abs() + 1e-9;
    assert!(close(got.mae, want.mae), "MAE {} vs {}", got.mae, want.mae);
    assert!(close(got.mse, want.mse), "MSE {} vs {}", got.mse, want.mse);
    // The relative error divides by the reference, magnifying the decode's differences in
    // the shadows.
    let rel_tol = if exact { tol } else { 1e-2 };
    assert!(
        (got.mre - want.mre).abs() <= rel_tol * want.mre.abs() + 1e-9,
        "MRE {} vs {}",
        got.mre,
        want.mre
    );
    assert!(
        (got.max - want.max).abs() <= if exact { 1e-6 } else { 2e-3 },
        "max {} vs {}",
        got.max,
        want.max
    );
    assert!(
        (got.psnr() - want.psnr()).abs() < 10.0 * tol || got.psnr() == want.psnr(),
        "PSNR {} vs {}",
        got.psnr(),
        want.psnr()
    );
}

/// The CPU reference from the samples the GPU reduces: both images rendered with `p`'s
/// sample views as `Comparer::request` renders them, read back.
fn sampled(r: &mut Rig, p: &MetricPair) -> DiffStats {
    let (px, _, _) = samples(r, p);
    reference_in(p.space, &px[0], &px[1], p.alpha)
}

/// The samples `Comparer::request` reduces for `p` (both images) and their size.
fn samples(r: &mut Rig, p: &MetricPair) -> (Vec<Vec<[f32; 4]>>, u32, u32) {
    let (w, h, va, vb) = metric_views(p).unwrap();
    let opts = RenderOptions {
        hq: false,
        canvas: [0.0; 4],
        checker_a: [0.0; 4],
        checker_b: [0.0; 4],
        checker_cell: 1.0e6,
        grid: None,
        nearest_from: 0.0,
        channel: p.channel,
        ..RenderOptions::default()
    };
    let mut px = Vec::new();
    for (id, view) in [(p.a.id, &va), (p.b.id, &vb)] {
        let tex = crate::renderer::texture_2d(
            &r.gpu.device,
            "sample",
            w,
            h,
            1,
            1,
            HDR,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let mut enc = r.gpu.device.create_command_encoder(&Default::default());
        let img = r.images.get(id).unwrap();
        r.renderer.render(
            &r.gpu,
            &mut enc,
            &tex.create_view(&Default::default()),
            (w, h),
            Some(img),
            view,
            &opts,
        );
        r.gpu.queue.submit([enc.finish()]);
        px.push(read_hdr(&r.gpu, &tex));
    }
    (px, w, h)
}

/// A gradient with fine detail: no two neighbours alike, so a sampling slip shows.
fn gradient(x: u32, y: u32) -> [u8; 4] {
    [
        (x * 7 % 256) as u8,
        (y * 5 % 256) as u8,
        ((x * y) % 251) as u8,
        255,
    ]
}

#[test]
fn gpu_metrics_match_the_cpu_reference() {
    // Different sizes, aligned top-left: compared where they overlap (90 × 70).
    let a = raster(120, 70, gradient);
    let b = raster(90, 100, |x, y| {
        let mut p = gradient(x, y);
        // Noise on part of it, a block of large differences on another.
        p[0] = p[0].wrapping_add(((x * 13 + y * 7) % 9) as u8);
        if x > 60 && y < 20 {
            p[1] = 255 - p[1];
        }
        p
    });
    let Some(mut r) = rig(&[&a, &b]) else { return };
    let pair = MetricPair {
        a: image(1, &a),
        b: image(2, &b),
        region: (0.0, 0.0, 90.0, 70.0),
        channel: ChannelView::Normal,
        alpha: false,
        space: MetricSpace::Linear,
    };
    let out = measure(&mut r, &[pair]);
    let got = out.pairs[0].expect("measured");
    assert_eq!(got.samples, (90, 70));
    assert_eq!(got.density, 1.0);
    let want = cpu(&a, &b, 90, 70, ChannelView::Normal, false);
    assert!(want.mse > 0.0);
    assert_close(&got.stats, &want, false);
    assert_close(&got.stats, &sampled(&mut r, &pair), true);

    // Identical images: no error at all, infinite PSNR.
    let same = MetricPair {
        b: image(1, &a),
        ..pair
    };
    let out = measure(&mut r, &[same, pair]);
    let s = out.pairs[0].expect("measured").stats;
    assert_eq!((s.mae, s.mse, s.max), (0.0, 0.0, 0.0));
    assert!(s.psnr().is_infinite());
    // The second pair of one request lands in its own slot.
    assert_close(&out.pairs[1].expect("measured").stats, &want, false);

    // A window of the overlap (the visible region when zoomed in).
    let win = MetricPair {
        region: (10.0, 20.0, 50.0, 60.0),
        ..pair
    };
    let out = measure(&mut r, &[win]);
    let got = out.pairs[0].expect("measured");
    assert_eq!(got.samples, (40, 40));
    let crop = |s: &Raster| {
        raster(40, 40, |x, y| {
            let i = ((y + 20) * s.width + x + 10) as usize * 4;
            s.planes[0].data[i..i + 4].try_into().unwrap()
        })
    };
    let want = cpu(&crop(&a), &crop(&b), 40, 40, ChannelView::Normal, false);
    assert_close(&got.stats, &want, false);
    assert_close(&got.stats, &sampled(&mut r, &win), true);
}

#[test]
fn metrics_follow_alpha_and_the_channel_view() {
    // Same coverage, different colour hidden under full transparency, and a coverage
    // difference in one block.
    let a = raster(64, 48, |x, y| {
        let mut p = gradient(x, y);
        p[3] = if x < 16 { 0 } else { 255 };
        p
    });
    let b = raster(64, 48, |x, y| {
        let mut p = gradient(x, y);
        if x < 16 {
            p = [255, 0, 255, 0];
        }
        if x >= 48 && y >= 32 {
            p[3] = 128;
        }
        // Green differs everywhere (the red channel view does not see it).
        p[1] = p[1].wrapping_add(40);
        p
    });
    let Some(mut r) = rig(&[&a, &b]) else { return };
    let pair = MetricPair {
        a: image(1, &a),
        b: image(2, &b),
        region: (0.0, 0.0, 64.0, 48.0),
        channel: ChannelView::Normal,
        alpha: true,
        space: MetricSpace::Linear,
    };
    let red = MetricPair {
        channel: ChannelView::Red,
        alpha: false,
        ..pair
    };
    let out = measure(&mut r, &[pair, red]);
    let got = out.pairs[0].expect("measured").stats;
    let want = cpu(&a, &b, 64, 48, ChannelView::Normal, true);
    assert_close(&got, &want, false);
    assert_close(&got, &sampled(&mut r, &pair), true);
    // The hidden colour alone would not count: only the visible part of the image differs.
    let hidden_only = cpu(
        &raster(16, 48, |x, y| {
            a.planes[0].data[((y * 64 + x) * 4) as usize..][..4]
                .try_into()
                .unwrap()
        }),
        &raster(16, 48, |x, y| {
            b.planes[0].data[((y * 64 + x) * 4) as usize..][..4]
                .try_into()
                .unwrap()
        }),
        16,
        48,
        ChannelView::Normal,
        true,
    );
    assert_eq!(hidden_only.mse, 0.0);
    // Red channel view: the green difference is gone, the hidden red under transparency
    // shows (channel views show colour at full strength) and so counts.
    let got = out.pairs[1].expect("measured").stats;
    let want = cpu(&a, &b, 64, 48, ChannelView::Red, false);
    assert_close(&got, &want, false);
    assert_close(&got, &sampled(&mut r, &red), true);
}

#[test]
fn metric_views_subsample_large_regions() {
    let a = MetricImage {
        id: 1,
        stored: (6000, 4000),
        orient: Orientation::NORMAL,
        place: Place::IDENTITY,
    };
    let p = MetricPair {
        a,
        b: MetricImage { id: 2, ..a },
        region: (0.0, 0.0, 6000.0, 4000.0),
        channel: ChannelView::Normal,
        alpha: false,
        space: MetricSpace::Linear,
    };
    let (w, h, va, _) = metric_views(&p).unwrap();
    assert_eq!((w, h), (METRIC_SIZE, 1365));
    let (x0, y0, x1, y1) = va.visible_display();
    assert!(x0 < 0.5 && y0 < 0.5 && x1 > 5999.0 && y1 > 3995.0);
    // A 100 % window: one sample per pixel.
    let p = MetricPair {
        region: (1000.0, 500.0, 2600.0, 1400.0),
        ..p
    };
    let (w, h, va, vb) = metric_views(&p).unwrap();
    assert_eq!((w, h), (1600, 900));
    assert_eq!(va.scale(), 1.0);
    assert_eq!(va.visible_display(), (1000.0, 500.0, 2600.0, 1400.0));
    assert_eq!(vb.visible_display(), va.visible_display());
    assert!(
        metric_views(&MetricPair {
            region: (5.0, 5.0, 5.0, 9.0),
            ..p
        })
        .is_none()
    );
}

/// Reads the RGBA half floats of an [`HDR`] texture.
fn read_hdr(gpu: &Gpu, tex: &wgpu::Texture) -> Vec<[f32; 4]> {
    let (w, h) = (tex.width(), tex.height());
    let row = (w * 8).div_ceil(256) * 256;
    let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: row as u64 * h as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        tex.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([enc.finish()]);
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    let data = buf.slice(..).get_mapped_range().unwrap();
    let mut out = Vec::new();
    for y in 0..h as usize {
        for x in 0..w as usize {
            let o = y * row as usize + x * 8;
            let c =
                |k: usize| f16_to_f32(u16::from_le_bytes([data[o + k * 2], data[o + k * 2 + 1]]));
            out.push([c(0), c(1), c(2), c(3)]);
        }
    }
    out
}

#[test]
fn difference_view_maps_the_error_to_the_ramp() {
    let a = raster(40, 30, |_, _| [128, 128, 128, 255]);
    // Brighter by a fixed step in the right half, the same in the left.
    let b = raster(32, 30, |x, _| {
        if x < 16 {
            [128, 128, 128, 255]
        } else {
            [160, 160, 160, 255]
        }
    });
    let Some(mut r) = rig(&[&a, &b]) else { return };
    let (w, h) = (40, 30);
    let target = |label| {
        crate::renderer::texture_2d(
            &r.gpu.device,
            label,
            w,
            h,
            1,
            1,
            HDR,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        )
    };
    let (ta, tb, td) = (target("a"), target("b"), target("diff"));
    let views = [&ta, &tb, &td].map(|t| t.create_view(&Default::default()));
    let opts = RenderOptions {
        canvas: [0.0; 4],
        checker_a: [0.0; 4],
        checker_b: [0.0; 4],
        nearest_from: 0.0,
        ..RenderOptions::default()
    };
    let mut enc = r.gpu.device.create_command_encoder(&Default::default());
    for (id, stored, view) in [(1, (40, 30), &views[0]), (2, (32, 30), &views[1])] {
        let v = View::fixed(
            stored,
            Orientation::NORMAL,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: w as f32,
                h: h as f32,
            },
            1.0,
            (20.0, 15.0),
        );
        let img = r.images.get(id).unwrap();
        r.renderer
            .render(&r.gpu, &mut enc, view, (w, h), Some(img), &v, &opts);
    }
    let step = srgb_decode(160.0 / 255.0) - srgb_decode(128.0 / 255.0);
    let full = step * 2.0;
    r.cmp.draw_diff(
        &r.gpu,
        &mut enc,
        &views[0],
        &views[1],
        &views[2],
        &DiffParams {
            kind: specular_inspect::DiffKind::Absolute,
            full_scale: full,
            overlap: [0.0, 0.0, 32.0, 30.0],
            alpha: false,
            space: MetricSpace::Linear,
        },
    );
    r.gpu.queue.submit([enc.finish()]);
    let px = read_hdr(&r.gpu, &td);
    let at = |x: usize, y: usize| px[y * w as usize + x];
    // Equal: the ramp's black, opaque.
    assert_eq!(at(4, 10), [0.0, 0.0, 0.0, 1.0]);
    // Half of full scale: the ramp's middle stop, decoded to linear.
    let mid = specular_inspect::false_colour(0.5).map(srgb_decode);
    let got = at(24, 10);
    for c in 0..3 {
        assert!((got[c] - mid[c]).abs() < 0.01, "{got:?} vs {mid:?}");
    }
    assert_eq!(got[3], 1.0);
    // Outside the overlap: clear.
    assert_eq!(at(36, 10), [0.0; 4]);
}

/// A build that panics ends as failed (and wakes the caller) instead of leaving the
/// difference view waiting.
#[test]
fn a_panicking_build_reports_failure() {
    let gpu = match Gpu::new() {
        Ok(g) => g,
        Err(e) if std::env::var_os("SPECULAR_REQUIRE_GPU").is_none() => {
            eprintln!("no GPU, skipped: {e}");
            return;
        }
        Err(e) => panic!("SPECULAR_REQUIRE_GPU is set but there is no adapter: {e}"),
    };
    specular_log::service::arm("test-compare", 1);
    let (tx, rx) = std::sync::mpsc::channel();
    let mut b = Comparer::spawn_on("test-compare", &gpu, move || {
        let _ = tx.send(());
    });
    rx.recv_timeout(std::time::Duration::from_secs(30))
        .expect("woken after the failure");
    assert!(b.failed());
    assert!(b.try_take().is_none());
}

/// A pair of 96 × 80 images with structure: the second blurred and tinted on one side.
fn structured() -> (Raster, Raster) {
    let a = raster(96, 80, |x, y| {
        let v = 128.0 + 90.0 * ((x as f32 * 0.31).sin() * (y as f32 * 0.17).cos());
        [v as u8, (x * 2) as u8, (255 - y * 3) as u8, 255]
    });
    let b = raster(96, 80, |x, y| {
        let mut s = [0u32; 3];
        for dy in 0..3u32 {
            for dx in 0..3u32 {
                let (xx, yy) = (
                    (x + dx).saturating_sub(1).min(95),
                    (y + dy).saturating_sub(1).min(79),
                );
                let i = ((yy * 96 + xx) * 4) as usize;
                for (c, sc) in s.iter_mut().enumerate() {
                    *sc += u32::from(a.planes[0].data[i + c]);
                }
            }
        }
        let tint = if x > 60 { 30 } else { 0 };
        [
            (s[0] / 9 + tint).min(255) as u8,
            (s[1] / 9) as u8,
            (s[2] / 9) as u8,
            255,
        ]
    });
    (a, b)
}

/// SSIM and ΔE2000 from the GPU against the CPU references on the very samples the GPU
/// reduced, in both metric spaces; MAE and PSNR follow the space too.
#[test]
fn ssim_and_delta_e_match_the_cpu_reference() {
    let (a, b) = structured();
    let Some(mut r) = rig(&[&a, &b]) else { return };
    for space in [MetricSpace::Linear, MetricSpace::Srgb] {
        let pair = MetricPair {
            a: image(1, &a),
            b: image(2, &b),
            region: (0.0, 0.0, 96.0, 80.0),
            channel: ChannelView::Normal,
            alpha: false,
            space,
        };
        let same = MetricPair {
            b: image(1, &a),
            ..pair
        };
        let out = measure(&mut r, &[pair, same]);
        let got = out.pairs[0].expect("measured");
        let (px, w, h) = samples(&mut r, &pair);
        let luma = |v: &[[f32; 4]]| v.iter().map(|p| ssim_luma(space, *p)).collect::<Vec<f32>>();
        let want = ssim_reference(&luma(&px[0]), &luma(&px[1]), w as usize, h as usize).unwrap();
        let ssim = got.ssim.expect("SSIM");
        assert!(
            (ssim - want).abs() < 1e-4,
            "{space:?}: SSIM {ssim} vs {want}"
        );
        assert!(want < 1.0 - 1e-4 && want > 0.3, "{want}");
        let de = delta_e_reference(&px[0], &px[1]);
        let g = got.delta_e;
        assert_eq!(g.pixels, de.pixels);
        assert!(
            (g.mean - de.mean).abs() <= 1e-4 * de.mean.max(1.0),
            "ΔE mean {} vs {}",
            g.mean,
            de.mean
        );
        assert!(
            (g.max - de.max).abs() <= 1e-3 * de.max.max(1.0),
            "ΔE max {} vs {}",
            g.max,
            de.max
        );
        let i = de_bin(de.p95 as f32).min(DE_BINS - 2);
        let width = f64::from(de_edge(i + 2) - de_edge(i));
        assert!(
            (g.p95 - de.p95).abs() <= width,
            "ΔE p95 {} vs {}",
            g.p95,
            de.p95
        );
        assert_close(
            &got.stats,
            &reference_in(space, &px[0], &px[1], false),
            true,
        );
        // Identical images: SSIM 1, no ΔE.
        let s = out.pairs[1].expect("measured");
        assert!((s.ssim.unwrap() - 1.0).abs() < 1e-5, "{:?}", s.ssim);
        // Not exactly 0 where the GPU fuses multiply-adds (Metal): a² + b² − C² need not
        // cancel. A ΔE of 1 is a just noticeable difference.
        assert!(
            s.delta_e.mean < 1e-4 && s.delta_e.max < 1e-4,
            "{:?}",
            (s.delta_e.mean, s.delta_e.max)
        );
    }
    // A region smaller than the window: no SSIM, the rest still measured.
    let tiny = MetricPair {
        a: image(1, &a),
        b: image(2, &b),
        region: (0.0, 0.0, 8.0, 40.0),
        channel: ChannelView::Normal,
        alpha: false,
        space: MetricSpace::Linear,
    };
    let out = measure(&mut r, &[tiny]);
    let got = out.pairs[0].expect("measured");
    assert_eq!(got.ssim, None);
    assert!(got.stats.mae > 0.0 && got.delta_e.mean > 0.0);
}

/// Images placed in the frame: the second a crop of the first, shifted back onto it, compares
/// equal over the overlap; a half-size copy scaled up to the first compares close to it.
#[test]
fn placed_images_compare_the_aligned_region() {
    let a = raster(80, 60, gradient);
    let crop = raster(50, 40, |x, y| gradient(x + 12, y + 7));
    let half = raster(40, 30, |x, y| {
        let v = (x * 255 / 40) as u8;
        let _ = y;
        [v, v, v, 255]
    });
    let full = raster(80, 60, |x, _| {
        let v = (x / 2 * 255 / 40) as u8;
        [v, v, v, 255]
    });
    let Some(mut r) = rig(&[&a, &crop, &half, &full]) else {
        return;
    };
    let mut b = image(2, &crop);
    b.place = Place::shifted((12.0, 7.0));
    let pair = MetricPair {
        a: image(1, &a),
        b,
        region: (12.0, 7.0, 62.0, 47.0),
        channel: ChannelView::Normal,
        alpha: false,
        space: MetricSpace::Linear,
    };
    let mut h = image(3, &half);
    h.place = Place {
        scale: 2.0,
        ..Place::IDENTITY
    };
    let scaled = MetricPair {
        a: image(4, &full),
        b: h,
        region: (0.0, 0.0, 80.0, 60.0),
        ..pair
    };
    let out = measure(&mut r, &[pair, scaled]);
    let got = out.pairs[0].expect("measured");
    assert_eq!(got.samples, (50, 40));
    assert_eq!(got.stats.mse, 0.0, "{:?}", got.stats);
    assert!((got.ssim.unwrap() - 1.0).abs() < 1e-5);
    // Nearest sampling of a 2× copy is the same columns twice.
    let got = out.pairs[1].expect("measured");
    assert_eq!(got.samples, (80, 60));
    assert!(got.stats.mae < 1e-3, "{:?}", got.stats);
}

#[test]
fn places_map_both_ways() {
    let p = Place {
        scale: 1.5,
        rotation: 0.05,
        offset: (10.0, -4.0),
    };
    let d = (123.0, 45.0);
    let f = p.to_frame(d);
    let back = p.from_frame(f);
    assert!((back.0 - d.0).abs() < 1e-3 && (back.1 - d.1).abs() < 1e-3);
    assert_eq!(Place::IDENTITY.to_frame(d), d);
    // The inner rectangle of a turned image is inside all four corners' box, and smaller.
    let (x0, y0, x1, y1) = p.inner_rect((200.0, 100.0));
    let (bx0, by0, bx1, by1) = Place { rotation: 0.0, ..p }.inner_rect((200.0, 100.0));
    assert!(x1 - x0 < bx1 - bx0 && y1 - y0 < by1 - by0);
    assert!(x0 > p.to_frame((0.0, 0.0)).0 - 1e-3);
    // A view through the place shows the frame point asked for at the viewport's centre.
    let vp = PxRect {
        x: 0.0,
        y: 0.0,
        w: 64.0,
        h: 48.0,
    };
    let v = p.view((400, 300), Orientation::NORMAL, vp, 2.0, (150.0, 60.0));
    let d = v.screen_to_display((32.0, 24.0));
    let f = p.to_frame(d);
    assert!(
        (f.0 - 150.0).abs() < 1e-2 && (f.1 - 60.0).abs() < 1e-2,
        "{f:?}"
    );
    // One frame unit is 2 viewport px whatever the place's scale.
    let d2 = v.screen_to_display((34.0, 24.0));
    let f2 = p.to_frame(d2);
    assert!(((f2.0 - f.0).hypot(f2.1 - f.1) - 1.0).abs() < 1e-3);
}

/// Every kind of the difference view against `diff_colour` on the very pixels rendered,
/// in both spaces (the output is half-float, so within a half-float step).
#[test]
fn difference_kinds_match_the_cpu_colours() {
    let (a, b) = structured();
    let Some(mut r) = rig(&[&a, &b]) else { return };
    let (w, h) = (96, 80);
    let target = |label| {
        crate::renderer::texture_2d(
            &r.gpu.device,
            label,
            w,
            h,
            1,
            1,
            HDR,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        )
    };
    let (ta, tb, td) = (target("a"), target("b"), target("diff"));
    let views = [&ta, &tb, &td].map(|t| t.create_view(&Default::default()));
    let opts = RenderOptions {
        canvas: [0.0; 4],
        checker_a: [0.0; 4],
        checker_b: [0.0; 4],
        nearest_from: 0.0,
        ..RenderOptions::default()
    };
    let mut enc = r.gpu.device.create_command_encoder(&Default::default());
    for (id, view) in [(1, &views[0]), (2, &views[1])] {
        let v = View::fixed(
            (w, h),
            Orientation::NORMAL,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: w as f32,
                h: h as f32,
            },
            1.0,
            (48.0, 40.0),
        );
        let img = r.images.get(id).unwrap();
        r.renderer
            .render(&r.gpu, &mut enc, view, (w, h), Some(img), &v, &opts);
    }
    r.gpu.queue.submit([enc.finish()]);
    let (pa, pb) = (read_hdr(&r.gpu, &ta), read_hdr(&r.gpu, &tb));
    let half = |v: f32| f16_to_f32(f32_to_f16(v));
    let mut worst = 0f32;
    for space in [MetricSpace::Linear, MetricSpace::Srgb] {
        for kind in DiffKind::ALL {
            let full = kind.full_scale(3);
            let mut enc = r.gpu.device.create_command_encoder(&Default::default());
            r.cmp.draw_diff(
                &r.gpu,
                &mut enc,
                &views[0],
                &views[1],
                &views[2],
                &DiffParams {
                    kind,
                    full_scale: full,
                    overlap: [0.0, 0.0, w as f32, h as f32],
                    alpha: false,
                    space,
                },
            );
            r.gpu.queue.submit([enc.finish()]);
            let got = read_hdr(&r.gpu, &td);
            for (i, g) in got.iter().enumerate() {
                let want = diff_colour(kind, space, pa[i], pb[i], false, kind.gain(3));
                for c in 0..4 {
                    let e = (g[c] - half(want[c])).abs();
                    worst = worst.max(e);
                    assert!(
                        e <= 2e-3 * want[c].abs().max(0.25),
                        "{kind:?} {space:?} px {i} ch {c}: {} vs {}",
                        g[c],
                        want[c]
                    );
                }
            }
        }
    }
    eprintln!("difference kinds: largest GPU − CPU difference {worst:e}");
}

/// The probe reads one texel of each pane's render where asked.
#[test]
fn the_probe_reads_both_panes() {
    let a = raster(8, 8, |x, y| [(x * 30) as u8, (y * 30) as u8, 0, 255]);
    let Some(mut r) = rig(&[&a]) else { return };
    let make = |c: [f64; 4]| {
        let t = crate::renderer::texture_2d(
            &r.gpu.device,
            "pane",
            16,
            12,
            1,
            1,
            HDR,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let mut enc = r.gpu.device.create_command_encoder(&Default::default());
        let view = t.create_view(&Default::default());
        {
            let _ = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: c[0],
                            g: c[1],
                            b: c[2],
                            a: c[3],
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }
        r.gpu.queue.submit([enc.finish()]);
        t
    };
    let ta = make([0.25, 0.5, 1.0, 1.0]);
    let tb = make([0.125, 0.0, 2.0, 0.5]);
    let mut enc = r.gpu.device.create_command_encoder(&Default::default());
    assert!(!r.cmp.probe_busy());
    assert!(
        r.cmp
            .request_probe(&mut enc, &[(&ta, (3, 4)), (&tb, (100, 100))])
    );
    assert!(r.cmp.probe_busy());
    assert!(!r.cmp.request_probe(&mut enc, &[(&ta, (0, 0))]));
    r.gpu.queue.submit([enc.finish()]);
    r.cmp.probe_after_submit();
    let got = loop {
        if let Some(v) = r.cmp.poll_probe(&r.gpu) {
            break v;
        }
        let _ = r.gpu.device.poll(wgpu::PollType::wait_indefinitely());
    };
    assert_eq!(got, vec![[0.25, 0.5, 1.0, 1.0], [0.125, 0.0, 2.0, 0.5]]);
    assert!(!r.cmp.probe_busy());
}
