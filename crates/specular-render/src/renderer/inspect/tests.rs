use specular_core::{Orientation, PixelLayout, Raster, RasterPlane};
use specular_gfx::{Gpu, HDR, wgpu};
use specular_inspect::{Channel, Histogram, srgb_decode};

use super::*;

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
    validate("inspect", &overlay_source());
    validate("histogram", HISTOGRAM_WGSL);
}

#[test]
fn readback_layout_fits() {
    const {
        assert!(PROBE_OFFSET.is_multiple_of(256));
        assert!(PROBE_OFFSET >= HIST_BYTES);
        assert!(READBACK_BYTES >= PROBE_OFFSET + 32);
    }
}

/// An RGB raster: a gradient with a clipped white block and a black block.
fn test_raster(w: u32, h: u32) -> Raster {
    let px = |x: u32, y: u32| -> [u8; 3] {
        if x < 8 && y < 8 {
            [255, 255, 255]
        } else if x >= w - 8 && y >= h - 8 {
            [0, 0, 0]
        } else {
            [
                (x * 255 / w) as u8,
                (y * 255 / h) as u8,
                ((x + y) * 97 % 256) as u8,
            ]
        }
    };
    let planes = (0..3)
        .map(|c| RasterPlane {
            width: w,
            height: h,
            stride: w as usize,
            data: (0..h)
                .flat_map(|y| (0..w).map(move |x| (x, y)))
                .map(|(x, y)| px(x, y)[c])
                .collect(),
            h_div: 1,
            v_div: 1,
        })
        .collect();
    Raster {
        width: w,
        height: h,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Rgb,
        planes,
        truncated: false,
    }
}

fn reference(r: &Raster, low: u8, high: u8) -> Histogram {
    reference_in(r, (0, 0, r.width, r.height), low, high)
}

/// The CPU reference histogram of the window `(x, y, w, h)` of `r`.
fn reference_in(r: &Raster, win: (u32, u32, u32, u32), low: u8, high: u8) -> Histogram {
    let (x0, y0, w, h) = win;
    let mut hist = Histogram::new();
    for y in y0 as usize..(y0 + h) as usize {
        for x in x0 as usize..(x0 + w) as usize {
            let c =
                |i: usize| srgb_decode(r.planes[i].data[y * r.width as usize + x] as f32 / 255.0);
            hist.add_linear([c(0), c(1), c(2)], low, high);
        }
    }
    hist
}

/// Asserts `got` equals `want` for R, G, B and the clip counts, and that every luma sample
/// is within one bin of the reference (the GPU's sRGB decode and the CPU's differ in the
/// last float bits, which moves values sitting on a bin boundary).
fn assert_matches(got: &Histogram, want: &Histogram) {
    assert_eq!(got.total, want.total);
    for c in Channel::RGB {
        assert_eq!(got.bins(c), want.bins(c), "{c:?}");
    }
    let cum = |b: &[u32; 256]| {
        b.iter()
            .scan(0u32, |a, v| {
                *a += v;
                Some(*a)
            })
            .collect::<Vec<_>>()
    };
    let (g, e) = (cum(got.bins(Channel::Luma)), cum(want.bins(Channel::Luma)));
    for i in 0..256 {
        let lo = if i == 0 { 0 } else { e[i - 1] };
        let hi = e[(i + 1).min(255)];
        assert!(
            lo <= g[i] && g[i] <= hi,
            "luma moved more than one bin at {i}"
        );
    }
    assert_eq!(got.clipped_high, want.clipped_high);
    assert_eq!(got.clipped_low, want.clipped_low);
}

struct Rig {
    gpu: Gpu,
    renderer: ImageRenderer,
    img: GpuImage,
    raster: Raster,
}

fn rig(w: u32, h: u32) -> Option<Rig> {
    let gpu = match Gpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("no GPU, skipped: {e}");
            return None;
        }
    };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let mut img = renderer.create_image(&gpu, 1, w, h, None);
    let raster = test_raster(w, h);
    renderer.upload_overview(&gpu, &mut img, &raster);
    Some(Rig {
        gpu,
        renderer,
        img,
        raster,
    })
}

fn whole(w: u32, h: u32, scale: f32) -> View {
    View::fixed(
        (w, h),
        Orientation::NORMAL,
        PxRect {
            x: 0.0,
            y: 0.0,
            w: w as f32 * scale,
            h: h as f32 * scale,
        },
        scale,
        (w as f32 * 0.5, h as f32 * 0.5),
    )
}

fn run(
    r: &Rig,
    insp: &mut Inspector,
    view: &View,
    hist: Option<HistogramRequest>,
    probe: Option<(u32, u32)>,
) -> Readback {
    let mut enc = r.gpu.device.create_command_encoder(&Default::default());
    assert!(insp.request(&r.gpu, &r.renderer, &mut enc, &r.img, view, hist, probe));
    assert!(insp.busy());
    // A second request while one is in flight is refused.
    assert!(!insp.request(&r.gpu, &r.renderer, &mut enc, &r.img, view, hist, probe));
    r.gpu.queue.submit([enc.finish()]);
    insp.after_submit();
    loop {
        if let Some(out) = insp.poll(&r.gpu) {
            assert!(!insp.busy());
            return out;
        }
        let _ = r.gpu.device.poll(wgpu::PollType::wait_indefinitely());
    }
}

#[test]
fn gpu_histogram_matches_the_cpu_reference() {
    let (w, h) = (96, 80);
    let Some(r) = rig(w, h) else { return };
    let mut insp = Inspector::new(&r.gpu, &r.renderer);
    let req = HistogramRequest {
        clip_low: 1,
        clip_high: 254,
    };
    let out = run(&r, &mut insp, &whole(w, h, 1.0), Some(req), None);
    let got = out.histogram.expect("histogram");
    let want = reference(&r.raster, 1, 254);
    assert_eq!(got.total, w * h);
    // The overview stores 8-bit sRGB, so channel codes survive the trip exactly.
    assert_matches(&got, &want);
    assert!(got.clipped_high >= 64 && got.clipped_low >= 64);
    assert!(out.probe.is_none());
}

#[test]
fn histogram_covers_only_the_visible_part() {
    let (w, h) = (96, 80);
    let Some(r) = rig(w, h) else { return };
    let mut insp = Inspector::new(&r.gpu, &r.renderer);
    // Zoomed 4× on the centre: a 24×20 window of the image.
    let view = View::fixed(
        (w, h),
        Orientation::NORMAL,
        PxRect {
            x: 0.0,
            y: 0.0,
            w: 96.0,
            h: 80.0,
        },
        4.0,
        (48.0, 40.0),
    );
    let req = HistogramRequest {
        clip_low: 1,
        clip_high: 254,
    };
    let got = run(&r, &mut insp, &view, Some(req), None)
        .histogram
        .unwrap();
    assert_eq!(got.total, 24 * 20);
    // Exactly the 24×20 window around the centre; the clipped corner blocks are out of view.
    assert_matches(&got, &reference_in(&r.raster, (36, 30, 24, 20), 1, 254));
    // A large image is sampled down to at most HIST_SIZE² pixels.
    let big = whole(w, h, 1.0);
    let (tw, th, _) = sample_view(&big, &r.img);
    assert_eq!((tw, th), (w, h));
}

#[test]
fn probe_reads_the_exact_texel() {
    let (w, h) = (96, 80);
    let Some(r) = rig(w, h) else { return };
    let mut insp = Inspector::new(&r.gpu, &r.renderer);
    let view = whole(w, h, 1.0);
    for (x, y) in [(0, 0), (40, 30), (95, 79), (10, 70)] {
        let p = run(&r, &mut insp, &view, None, Some((x, y)))
            .probe
            .expect("probe");
        let i = (y * w + x) as usize;
        let want = [0, 1, 2].map(|c| r.raster.planes[c].data[i]);
        assert_eq!(&p.srgb[..3], &want, "at {x},{y}");
        assert_eq!(p.srgb[3], 255);
        // A full-size overview is full resolution.
        assert!(p.exact);
    }
}

#[test]
fn overlay_marks_clipped_pixels() {
    let (w, h) = (96, 80);
    let Some(r) = rig(w, h) else { return };
    let insp = Inspector::new(&r.gpu, &r.renderer);
    let d = &r.gpu.device;
    let tex = super::super::texture_2d(
        d,
        "test scene",
        w,
        h,
        1,
        1,
        HDR,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let tv = tex.create_view(&Default::default());
    let view = whole(w, h, 1.0);
    let opts = RenderOptions::default();
    let mut enc = d.create_command_encoder(&Default::default());
    {
        // Clear to transparent so only overlay pixels remain.
        let _ = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &tv,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
    }
    let o = OverlayOptions {
        clipping: true,
        ..OverlayOptions::default()
    };
    insp.draw_overlay(
        &r.gpu,
        &r.renderer,
        &mut enc,
        &tv,
        (w, h),
        &r.img,
        &view,
        &opts,
        &o,
    );
    let row = (w * 8).div_ceil(256) * 256;
    let buf = d.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (row * h) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
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
    r.gpu.queue.submit([enc.finish()]);
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    let _ = d.poll(wgpu::PollType::wait_indefinitely());
    let data = buf.slice(..).get_mapped_range().unwrap();
    let px = |x: u32, y: u32| -> [f32; 4] {
        let o = (y * row + x * 8) as usize;
        let h: &[u16] = bytemuck::cast_slice(&data[o..o + 8]);
        [0, 1, 2, 3].map(|i| f16_to_f32(h[i]))
    };
    // White block: highlight colour; black block: shadow colour; middle: untouched.
    // The target is half float: compare to its precision.
    let near = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3);
    assert!(near(px(2, 2), o.highlight_color), "{:?}", px(2, 2));
    assert!(
        near(px(w - 2, h - 2), o.shadow_color),
        "{:?}",
        px(w - 2, h - 2)
    );
    assert_eq!(px(48, 40)[3], 0.0);
}

fn f16_to_f32(h: u16) -> f32 {
    let s = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = ((h >> 10) & 0x1f) as i32;
    let m = (h & 0x3ff) as f32;
    s * match e {
        0 => m * 2f32.powi(-24),
        31 => f32::INFINITY,
        _ => (1.0 + m / 1024.0) * 2f32.powi(e - 15),
    }
}

/// Per-frame cost of the passes at a 2560×1440 viewport (run with `--ignored --nocapture`).
#[test]
#[ignore]
fn inspect_cost() {
    let (w, h) = (2048, 1536);
    let Some(r) = rig(w, h) else { return };
    let mut insp = Inspector::new(&r.gpu, &r.renderer);
    eprintln!(
        "adapter: {} ({:?})",
        r.gpu.info.name, r.gpu.info.device_type
    );
    let d = &r.gpu.device;
    let size = (2560, 1440);
    let scene = super::super::texture_2d(
        d,
        "bench scene",
        size.0,
        size.1,
        1,
        1,
        HDR,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    )
    .create_view(&Default::default());
    let mut view = View::new();
    view.set_image(w, h, Orientation::NORMAL);
    view.set_viewport(PxRect {
        x: 0.0,
        y: 0.0,
        w: size.0 as f32,
        h: size.1 as f32,
    });
    view.zoom_fit();
    view.settle();
    let opts = RenderOptions::default();
    let time = |label: &str, f: &mut dyn FnMut(&mut wgpu::CommandEncoder)| {
        let n = 30;
        // Warm up, then time n submissions waited on one by one (GPU + submit latency).
        for i in 0..n + 3 {
            if i == 3 {
                let _ = d.poll(wgpu::PollType::wait_indefinitely());
            }
            let t0 = std::time::Instant::now();
            let mut enc = d.create_command_encoder(&Default::default());
            f(&mut enc);
            r.gpu.queue.submit([enc.finish()]);
            let _ = d.poll(wgpu::PollType::wait_indefinitely());
            if i == n + 2 {
                eprintln!(
                    "{label}: {:.3} ms (last of {n})",
                    t0.elapsed().as_secs_f64() * 1e3
                );
            }
        }
    };
    {
        // The image pass itself, for scale.
        let mut rr = ImageRenderer::new(&r.gpu, 32 << 20);
        let mut img = rr.create_image(&r.gpu, 2, w, h, None);
        rr.upload_overview(&r.gpu, &mut img, &r.raster);
        let mut last = 0.0;
        for _ in 0..33 {
            let t0 = std::time::Instant::now();
            let mut enc = d.create_command_encoder(&Default::default());
            rr.render(&r.gpu, &mut enc, &scene, size, Some(&img), &view, &opts);
            r.gpu.queue.submit([enc.finish()]);
            let _ = d.poll(wgpu::PollType::wait_indefinitely());
            last = t0.elapsed().as_secs_f64() * 1e3;
        }
        eprintln!("image pass (for scale): {last:.3} ms");
    }
    for (label, o) in [
        (
            "clipping overlay",
            OverlayOptions {
                clipping: true,
                ..Default::default()
            },
        ),
        (
            "peaking overlay",
            OverlayOptions {
                peaking: true,
                ..Default::default()
            },
        ),
        (
            "clipping + peaking",
            OverlayOptions {
                clipping: true,
                peaking: true,
                ..Default::default()
            },
        ),
    ] {
        time(label, &mut |enc| {
            insp.draw_overlay(
                &r.gpu,
                &r.renderer,
                enc,
                &scene,
                size,
                &r.img,
                &view,
                &opts,
                &o,
            )
        });
    }
    let req = HistogramRequest {
        clip_low: 1,
        clip_high: 254,
    };
    let mut lat = Vec::new();
    for _ in 0..30 {
        let t0 = std::time::Instant::now();
        let mut enc = d.create_command_encoder(&Default::default());
        insp.request(
            &r.gpu,
            &r.renderer,
            &mut enc,
            &r.img,
            &view,
            Some(req),
            Some((10, 10)),
        );
        let rec = t0.elapsed().as_secs_f64() * 1e3;
        r.gpu.queue.submit([enc.finish()]);
        insp.after_submit();
        let out = loop {
            if let Some(o) = insp.poll(&r.gpu) {
                break o;
            }
            let _ = d.poll(wgpu::PollType::wait_indefinitely());
        };
        lat.push((rec, t0.elapsed().as_secs_f64() * 1e3, out.latency_ms));
    }
    let (rec, total, _) = lat[lat.len() - 1];
    eprintln!(
        "histogram + probe: record {rec:.3} ms CPU, round trip {total:.3} ms (GPU, not blocking)"
    );
    let t0 = std::time::Instant::now();
    let mut hh = Histogram::new();
    for _ in 0..30 {
        hh = Histogram::from_words(&vec![1u32; Histogram::WORDS]).unwrap();
        let p = hh.peak(&Channel::ALL, true);
        for c in Channel::ALL {
            std::hint::black_box(hh.curve(c, true, p, 128));
        }
    }
    eprintln!(
        "histogram decode + curves: {:.4} ms",
        t0.elapsed().as_secs_f64() * 1e3 / 30.0
    );
    std::hint::black_box(hh);
}
