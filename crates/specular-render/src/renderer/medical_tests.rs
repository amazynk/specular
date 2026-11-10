//! Window/level on the GPU against [`Window::apply`], the f64 reference of DICOM PS3.3
//! C.11.2.1.2 (itself checked against hand-computed values from the standard). GPU tests skip
//! without an adapter.

use specular_gfx::FULLSCREEN_WGSL;

use super::super::Gsdf;
use super::super::hdr_tests::{draw_scaled, draw_with, gpu};
use super::*;

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn win(center: f64, width: f64, shape: VoiShape, invert: bool) -> Window {
    Window {
        center,
        width,
        shape,
        invert,
    }
}

#[test]
fn debayer_shader_validates() {
    let src = format!(
        "{FULLSCREEN_WGSL}\n{}",
        include_str!("../shaders/debayer.wgsl")
    );
    let module = naga::front::wgsl::parse_str(&src)
        .unwrap_or_else(|e| panic!("debayer: {}", e.emit_to_string(&src)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("debayer: {e:?}"));
}

/// The GPU demosaic against [`nd::Cfa::debayer_at`], every pattern, odd and even sizes,
/// edges included, drawn 1:1 through a linear look per channel.
#[test]
fn debayer_matches_the_reference() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    for (w, h) in [(9usize, 6usize), (8, 7)] {
        let raw: Vec<f32> = (0..w * h)
            .map(|i| ((i * 2654435761usize) % 1000) as f32)
            .collect();
        for cfa in nd::Cfa::ALL {
            let mut img = renderer.create_image(&gpu, 5, w as u32, h as u32, None);
            renderer.set_value_cfa(&gpu, &mut img, &raw, cfa);
            assert_eq!(img.value_channels(), 3);
            let cols = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
            let look = ScienceLook {
                stretch: Stretch::Linear,
                channels: cols
                    .iter()
                    .map(|&colour| ChannelLook {
                        lo: 0.0,
                        hi: 1000.0,
                        colour,
                        on: true,
                        mid: 0.5,
                    })
                    .collect(),
                luts: Vec::new(),
                colour_table: None,
                hillshade: None,
                isotherm: None,
            };
            assert!(renderer.set_science(&gpu, &mut img, Some(look.clone())));
            let out = draw_with(&gpu, &mut renderer, &img, RenderOptions::default());
            for y in 0..h {
                for x in 0..w {
                    let want = look.colour(&cfa.debayer_at(&raw, w, h, x, y));
                    for ch in 0..3 {
                        let wl = srgb_to_linear(f64::from(want[ch])) as f32;
                        let got = out[y * w + x][ch];
                        assert!(
                            (got - wl).abs() <= 2e-3 + wl * 4e-3,
                            "{} {w}x{h} ({x},{y}) channel {ch}: {got} vs {wl}",
                            cfa.name()
                        );
                    }
                }
            }
        }
    }
}

/// A frame past the texture limit is demosaiced on the CPU (row by row) and still holds
/// three value channels.
#[test]
fn oversized_cfa_is_demosaiced_on_the_cpu() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    renderer.max_tex = 4;
    let (w, h) = (9usize, 6usize);
    let raw: Vec<f32> = (0..w * h).map(|i| (i % 7) as f32).collect();
    let mut img = renderer.create_image(&gpu, 5, w as u32, h as u32, None);
    renderer.set_value_cfa(&gpu, &mut img, &raw, nd::Cfa::RGGB);
    assert_eq!(img.value_channels(), 3);
}

#[test]
fn shader_validates() {
    let src = format!(
        "{FULLSCREEN_WGSL}\n{}",
        include_str!("../shaders/medical.wgsl")
    );
    let module = naga::front::wgsl::parse_str(&src)
        .unwrap_or_else(|e| panic!("medical: {}", e.emit_to_string(&src)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("medical: {e:?}"));
}

/// The reference against PS3.3 C.11.2.1.2.1 and C.11.2.1.3, worked by hand.
#[test]
fn window_maths_matches_the_standard() {
    // LINEAR, c = 40, w = 400 (CT soft tissue): edges at 39.5 ∓ 199.5.
    let w = win(40.0, 400.0, VoiShape::Linear, false);
    assert_eq!(w.apply(-160.0), 0.0);
    assert_eq!(w.apply(240.0), 1.0);
    assert!((w.apply(39.5) - 0.5).abs() < 1e-12);
    assert!((w.apply(0.0) - ((0.0 - 39.5) / 399.0 + 0.5)).abs() < 1e-12);
    // W = 1: a threshold at c − 0.5, values at it are black, above it white.
    let t = win(0.0, 1.0, VoiShape::Linear, false);
    assert_eq!(t.apply(-1.0), 0.0);
    assert_eq!(t.apply(-0.5), 0.0);
    assert_eq!(t.apply(-0.49), 1.0);
    assert_eq!(t.apply(0.0), 1.0);
    // LINEAR_EXACT: edges at c ∓ w/2, exactly.
    let e = win(100.0, 50.0, VoiShape::LinearExact, false);
    assert_eq!(e.apply(75.0), 0.0);
    assert_eq!(e.apply(125.0), 1.0);
    assert!((e.apply(100.0) - 0.5).abs() < 1e-12);
    assert!((e.apply(112.5) - 0.75).abs() < 1e-12);
    // SIGMOID: 1 / (1 + e^(−4 (x − c) / w)).
    let s = win(0.0, 100.0, VoiShape::Sigmoid, false);
    assert!((s.apply(0.0) - 0.5).abs() < 1e-12);
    assert!((s.apply(25.0) - 1.0 / (1.0 + (-1.0f64).exp())).abs() < 1e-12);
    // Inversion after the window (MONOCHROME1).
    let i = win(40.0, 400.0, VoiShape::Linear, true);
    assert_eq!(i.grey(-1000.0), 1.0);
    assert_eq!(i.grey(1000.0), 0.0);
}

/// A row of values, one per pixel, uploaded as an image `n`×2.
fn upload(gpu: &Gpu, renderer: &mut ImageRenderer, id: u64, row: &[f32]) -> GpuImage {
    let n = row.len() as u32;
    let mut img = renderer.create_image(gpu, id, n, 2, None);
    let data: Vec<f32> = row.iter().chain(row).copied().collect();
    renderer.set_values(gpu, &mut img, &data);
    img
}

fn check(out: &[[f32; 4]], row: &[f32], w: &Window, what: &str) {
    for (i, &x) in row.iter().enumerate() {
        let want = srgb_to_linear(w.grey(f64::from(x))) as f32;
        let got = out[i];
        assert!(
            got[0] == got[1] && got[1] == got[2] && got[3] == 1.0,
            "{what}: pixel {i} not opaque grey: {got:?}"
        );
        // The target is half-float: 11 significant bits.
        assert!(
            (got[0] - want).abs() <= 1e-4 + want * 2e-3,
            "{what}: value {x}: {} vs {want}",
            got[0]
        );
    }
}

#[test]
fn gpu_window_matches_the_reference() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    // Signed 16-bit CT values (−1024 air … 3071 bone), and fractional ones near the edges.
    let mut row: Vec<f32> = (-32..32).map(|i| i as f32 * 64.0 - 1.0).collect();
    row.extend([
        i16::MIN as f32,
        i16::MAX as f32,
        -160.0,
        -159.0,
        239.0,
        240.0,
        39.5,
        -0.5,
        -0.49,
        0.0,
        0.5,
        75.0,
        125.0,
        112.5,
    ]);
    let img = upload(&gpu, &mut renderer, 1, &row);
    for w in [
        win(40.0, 400.0, VoiShape::Linear, false),
        win(-600.0, 1500.0, VoiShape::Linear, false),
        win(0.0, 1.0, VoiShape::Linear, false),
        win(100.0, 50.0, VoiShape::LinearExact, false),
        win(0.0, 0.0, VoiShape::LinearExact, false),
        win(0.0, 1000.0, VoiShape::Sigmoid, false),
        win(40.0, 80.0, VoiShape::Sigmoid, true),
        win(40.0, 400.0, VoiShape::Linear, true),
        win(-32768.0, 65536.0, VoiShape::LinearExact, false),
    ] {
        let mut img = upload(&gpu, &mut renderer, 2, &row);
        assert!(img.set_window(Some(w)));
        let out = draw_with(&gpu, &mut renderer, &img, RenderOptions::default());
        check(&out, &row, &w, &format!("{w:?}"));
    }
    // Without a window the image draws its ordinary pixels (none here: the canvas).
    let out = draw_with(&gpu, &mut renderer, &img, RenderOptions::default());
    assert!(out.iter().all(|p| p[0] == 0.0 && p[3] == 1.0));
}

/// Science images: every stretch, one channel and three composited, against
/// [`ScienceLook::colour`]; an undefined value adds nothing, and a pixel with none defined
/// shows the canvas.
#[test]
fn gpu_science_matches_the_reference() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let n = 64usize;
    let a: Vec<f32> = (0..n).map(|i| i as f32 * 3.0 - 20.0).collect();
    let b: Vec<f32> = (0..n).map(|i| ((i * 37) % 64) as f32 * 0.01).collect();
    let mut c: Vec<f32> = (0..n).map(|i| 1e4 * (i as f32 / 63.0).powi(3)).collect();
    c[5] = f32::NAN;
    let two = |r: &[f32]| r.iter().chain(r).copied().collect::<Vec<f32>>();
    let (a2, b2, c2) = (two(&a), two(&b), two(&c));
    // Channels 4 to 7 (a second layer of the values): shifted copies, one with a hole.
    let more: Vec<Vec<f32>> = (1..5usize)
        .map(|s| {
            let mut r: Vec<f32> = (0..n)
                .map(|i| ((i * (s + 3)) % 64) as f32 + s as f32)
                .collect();
            if s == 2 {
                r[9] = f32::NAN;
            }
            two(&r)
        })
        .collect();
    let grey = [1.0, 1.0, 1.0];
    let cols = [
        [1.0, 0.2, 0.0],
        [0.0, 1.0, 0.3],
        [0.1, 0.0, 1.0],
        [0.3, 0.3, 0.0],
        [0.0, 0.2, 0.2],
        [0.25, 0.0, 0.25],
        [0.1, 0.1, 0.1],
    ];
    for stretch in [
        Stretch::Linear,
        Stretch::Sqrt,
        Stretch::Log,
        Stretch::Asinh,
        Stretch::HistEq,
        Stretch::Auto,
    ] {
        let seven: Vec<&[f32]> = [&a2[..], &b2[..], &c2[..]]
            .into_iter()
            .chain(more.iter().map(|r| &r[..]))
            .collect();
        for planes in [vec![&c2[..]], vec![&a2[..], &b2[..], &c2[..]], seven] {
            let mut img = renderer.create_image(&gpu, 3, n as u32, 2, None);
            renderer.set_value_channels(&gpu, &mut img, &planes);
            assert_eq!(img.value_channels(), planes.len() as u32);
            let rows: Vec<&[f32]> = planes.iter().map(|p| &p[..n]).collect();
            let channels: Vec<ChannelLook> = rows
                .iter()
                .enumerate()
                .map(|(k, r)| {
                    let (lo, hi) = nd::Limits::MinMax.compute(r, n).unwrap();
                    ChannelLook {
                        lo: lo + (hi - lo) * 0.1,
                        hi: hi - (hi - lo) * 0.2,
                        colour: if rows.len() == 1 { grey } else { cols[k] },
                        on: k != 1 || stretch != Stretch::Sqrt,
                        mid: 0.15 + 0.1 * k as f32,
                    }
                })
                .collect();
            let luts = if stretch == Stretch::HistEq {
                rows.iter()
                    .zip(&channels)
                    .map(|(r, c)| nd::histeq_lut(r, c.lo, c.hi))
                    .collect()
            } else {
                Vec::new()
            };
            // One channel through a file's colour table, for some stretches.
            let table = (rows.len() == 1 && matches!(stretch, Stretch::Linear | Stretch::Asinh))
                .then(|| {
                    let mut t = [[0u8; 3]; 256];
                    for (i, e) in t.iter_mut().enumerate() {
                        *e = [
                            i as u8,
                            255 - i as u8,
                            (i as i32 - 128).unsigned_abs().min(255) as u8,
                        ];
                    }
                    std::sync::Arc::new(t)
                });
            let look = ScienceLook {
                stretch,
                channels,
                luts,
                colour_table: table,
                hillshade: None,
                isotherm: None,
            };
            assert!(renderer.set_science(&gpu, &mut img, Some(look.clone())));
            assert_eq!(img.science(), Some(&look));
            let out = draw_with(&gpu, &mut renderer, &img, RenderOptions::default());
            for i in 0..n {
                // Undefined values are left out: their channel is off for the pixel.
                let v: Vec<f32> = rows.iter().map(|r| r[i]).collect();
                let mut here = look.clone();
                for (c, x) in here.channels.iter_mut().zip(&v) {
                    c.on &= x.is_finite();
                }
                let shown = here.channels.iter().any(|c| c.on);
                let want = here.colour(&v);
                for ch in 0..3 {
                    if !shown {
                        assert_eq!(out[i][ch], 0.0, "{stretch:?}: pixel {i} shows the canvas");
                        continue;
                    }
                    let w = srgb_to_linear(f64::from(want[ch])) as f32;
                    let got = out[i][ch];
                    assert!(
                        (got - w).abs() <= 2e-3 + w * 4e-3,
                        "{stretch:?} {}ch: pixel {i} channel {ch}: {got} vs {w}",
                        rows.len()
                    );
                }
            }
            // Back to the ordinary pixels.
            assert!(renderer.set_science(&gpu, &mut img, None));
            let out = draw_with(&gpu, &mut renderer, &img, RenderOptions::default());
            assert!(out.iter().all(|p| p[0] == 0.0 && p[3] == 1.0));
        }
    }
}

#[test]
fn window_changes_bump_the_revision() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let mut img = upload(&gpu, &mut renderer, 3, &[0.0, 1.0]);
    let r0 = img.revision;
    let w = win(0.0, 10.0, VoiShape::Linear, false);
    img.set_window(Some(w));
    assert_eq!(img.revision, r0 + 1);
    img.set_window(Some(w));
    assert_eq!(img.revision, r0 + 1, "unchanged window, same revision");
    assert_eq!(img.window(), Some(w));
    // New values (the next frame) keep the window.
    renderer.set_values(&gpu, &mut img, &[5.0, 6.0, 5.0, 6.0]);
    assert_eq!(img.window(), Some(w));
    assert!(img.memory_bytes() > 0);
    renderer.clear_values(&mut img);
    assert!(!img.has_values() && !img.set_window(Some(w)));
}

/// Downscaled, the values are averaged before the window (mips), as one would see by
/// averaging the tissue: a 0 / 100 checkerboard at ¼ shows the window's grey for 50.
#[test]
fn minified_values_average_before_the_window() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let n = 64u32;
    let data: Vec<f32> = (0..n * n)
        .map(|i| {
            if (i % n + i / n).is_multiple_of(2) {
                0.0
            } else {
                100.0
            }
        })
        .collect();
    let mut img = renderer.create_image(&gpu, 4, n, n, None);
    renderer.set_values(&gpu, &mut img, &data);
    let w = win(50.0, 200.0, VoiShape::LinearExact, false);
    img.set_window(Some(w));
    let out = draw_scaled(
        &gpu,
        &mut renderer,
        &img,
        RenderOptions {
            hq: true,
            ..RenderOptions::default()
        },
        0.25,
    );
    let want = srgb_to_linear(w.grey(50.0)) as f32;
    for p in &out {
        assert!((p[0] - want).abs() < 2e-3, "{} vs {want}", p[0]);
    }
    // Magnified (bilinear) between a 0 and a 100 pixel: values in between, not just two
    // greys.
    let out = draw_scaled(
        &gpu,
        &mut renderer,
        &img,
        RenderOptions {
            nearest_from: 64.0,
            ..RenderOptions::default()
        },
        4.0,
    );
    let distinct: std::collections::BTreeSet<u32> =
        out.iter().map(|p| (p[0] * 1000.0) as u32).collect();
    assert!(distinct.len() > 4, "{distinct:?}");
}

/// Shutters draw in their grey or colour and overlays in their inks, after the window and
/// the inversion; overlays toggle, and the next frame's values drop the mask.
#[test]
fn gpu_shutters_and_overlays() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let row = [10.0f32, 20.0, 30.0, 40.0, 50.0, 60.0];
    let mut img = upload(&gpu, &mut renderer, 5, &row);
    let w = win(35.0, 60.0, VoiShape::LinearExact, true);
    img.set_window(Some(w));
    // Pixel 0 shuttered, 1 overlay ink 1, 2 both (the overlay is on top), 3 ink 2; both
    // rows.
    let bits = [1u8, 2, 3, 4, 0, 0];
    let mask: Vec<u8> = bits.iter().chain(&bits).copied().collect();
    let r0 = img.revision;
    let inks = [[1.0f32; 3], [1.0, 0.5, 0.0]];
    let vm = |shutter| ValueMask {
        mask: &mask,
        shutter,
        inks: &inks,
    };
    renderer.set_graphics(&gpu, &mut img, Some(vm([0.25; 3])));
    assert!(img.has_graphics() && img.revision == r0 + 1);
    let grey = |g: f64| srgb_to_linear(g) as f32;
    let out = draw_with(&gpu, &mut renderer, &img, RenderOptions::default());
    let want = [[grey(0.25); 3], [1.0; 3], [1.0; 3], [1.0, grey(0.5), 0.0]];
    for (i, want) in want.iter().enumerate() {
        for k in 0..3 {
            assert!(
                (out[i][k] - want[k]).abs() < 2e-3,
                "pixel {i}: {:?}",
                out[i]
            );
        }
    }
    check(&out[4..6], &row[4..], &w, "unmasked");
    // A shutter colour.
    renderer.set_graphics(&gpu, &mut img, Some(vm([0.0, 1.0, 0.0])));
    let out = draw_with(&gpu, &mut renderer, &img, RenderOptions::default());
    assert!(
        out[0][0] < 2e-3 && out[0][1] > 0.998 && out[0][2] < 2e-3,
        "{:?}",
        out[0]
    );
    renderer.set_graphics(&gpu, &mut img, Some(vm([0.25; 3])));
    img.set_overlays(false);
    assert_eq!(img.revision, r0 + 4);
    let out = draw_with(&gpu, &mut renderer, &img, RenderOptions::default());
    for i in [0, 2] {
        assert!(
            (out[i][0] - grey(0.25)).abs() < 2e-3,
            "pixel {i}: {:?}",
            out[i]
        );
    }
    check(&out[1..2], &row[1..2], &w, "overlay hidden");
    renderer.set_values(&gpu, &mut img, &[0.0; 12]);
    assert!(!img.has_graphics());
    let r = img.revision;
    renderer.set_graphics(&gpu, &mut img, None);
    assert_eq!(img.revision, r, "no mask before or after: same revision");
}

#[test]
fn oversized_masks_keep_every_bit() {
    // 5×3 by 2: a shutter or overlay anywhere in a box survives, the highest ink (not the
    // OR of two inks' numbers) with a shutter.
    let mask = [0u8, 0, 0, 0, 2, 0, 1, 0, 0, 0, 0, 0, 0, 2, 0];
    assert_eq!(downscale_mask(&mask, 5, 3, 2), vec![1, 0, 2, 0, 2, 0]);
    let mask = [2u8, 4, 0, 0, 1, 0, 0, 0];
    assert_eq!(downscale_mask(&mask, 4, 2, 2), vec![5, 0]);
    let mask = [6u8, 3, 0, 0];
    assert_eq!(downscale_mask(&mask, 4, 1, 2), vec![7, 0]);
}

#[test]
fn oversized_values_are_box_downscaled() {
    let data: Vec<f32> = (0..5 * 3).map(|i| i as f32).collect();
    let (out, w, h) = downscale(&data, 5, 3, 2);
    assert_eq!((w, h), (3, 2));
    // Row 0: (0+1+5+6)/4, (2+3+7+8)/4, (4+9)/2; row 1: (10+11)/2, (12+13)/2, 14.
    assert_eq!(out, vec![3.0, 5.0, 6.5, 10.5, 12.5, 14.0]);
}

/// Frame time of a window drag: every frame a new window, the whole image drawn into a
/// 1600×1000 target and waited for. A measurement, not a check:
/// `cargo test --release -p specular-render bench_window_drag -- --ignored --nocapture`.
#[test]
#[ignore = "benchmark"]
fn bench_window_drag() {
    use specular_core::Orientation;
    use specular_gfx::{HDR, wgpu};

    use crate::view::{PxRect, View};

    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 256 << 20);
    let (w, h) = (1600u32, 1000u32);
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bench target"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let tv = target.create_view(&Default::default());
    let adapter = gpu.adapter.get_info().name;
    for (name, n, lo, hi) in [
        ("512² CT", 512u32, -1024.0f32, 3071.0f32),
        ("3000² DX", 3000, 0.0, 16383.0),
    ] {
        let values: Vec<f32> = (0..n * n)
            .map(|i| lo + (hi - lo) * ((i % n + i / n) as f32 / (2 * n) as f32))
            .collect();
        let t = std::time::Instant::now();
        let mut img = renderer.create_image(&gpu, u64::from(n), n, n, None);
        renderer.set_values(&gpu, &mut img, &values);
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let upload = t.elapsed();
        for (fit, scale) in [("fit", h as f32 / n as f32), ("1:1", 1.0)] {
            let view = View::fixed(
                (n, n),
                Orientation::NORMAL,
                PxRect {
                    x: 0.0,
                    y: 0.0,
                    w: w as f32,
                    h: h as f32,
                },
                scale,
                (n as f32 * 0.5, n as f32 * 0.5),
            );
            let opts = RenderOptions::default();
            let frames = 200;
            let mut times = Vec::with_capacity(frames);
            for k in 0..frames + 10 {
                let t = std::time::Instant::now();
                let c = lo + (hi - lo) * (k as f32 / frames as f32);
                assert!(img.set_window(Some(win(
                    f64::from(c),
                    400.0 + k as f64,
                    VoiShape::Linear,
                    false
                ))));
                let mut enc = gpu.device.create_command_encoder(&Default::default());
                renderer.render(&gpu, &mut enc, &tv, (w, h), Some(&img), &view, &opts);
                gpu.queue.submit([enc.finish()]);
                let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
                if k >= 10 {
                    times.push(t.elapsed().as_secs_f64() * 1e3);
                }
            }
            times.sort_by(f64::total_cmp);
            eprintln!(
                "{name} {fit}: upload {:.1} ms, drag frame median {:.2} ms, p95 {:.2} ms ({adapter})",
                upload.as_secs_f64() * 1e3,
                times[frames / 2],
                times[frames * 95 / 100],
            );
        }
    }
}

/// No-data (NaN) in a science image: the canvas shows there at every zoom, and it never
/// mixes into its neighbours (the mips average only values); one channel with false colour
/// goes through the map.
#[test]
fn science_no_data_shows_the_canvas() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let n = 64u32;
    // The left half no data, the right half 100.
    let data: Vec<f32> = (0..n * n)
        .map(|i| if i % n < n / 2 { f32::NAN } else { 100.0 })
        .collect();
    let mut img = renderer.create_image(&gpu, 7, n, n, None);
    renderer.set_value_channels(&gpu, &mut img, &[&data]);
    let look = ScienceLook {
        stretch: Stretch::Linear,
        channels: vec![ChannelLook {
            lo: 0.0,
            hi: 200.0,
            colour: [1.0, 1.0, 1.0],
            on: true,
            mid: 0.5,
        }],
        luts: Vec::new(),
        colour_table: None,
        hillshade: None,
        isotherm: None,
    };
    assert!(renderer.set_science(&gpu, &mut img, Some(look)));
    let canvas = [0.25, 0.5, 0.75, 1.0];
    let half = srgb_to_linear(0.5) as f32;
    for (scale, hq) in [(0.25, true), (0.5, false), (1.0, false), (4.0, false)] {
        let out = draw_scaled(
            &gpu,
            &mut renderer,
            &img,
            RenderOptions {
                canvas,
                hq,
                ..RenderOptions::default()
            },
            scale,
        );
        let w = (n as f32 * scale).ceil() as usize;
        for (i, p) in out.iter().enumerate() {
            let x = i % w;
            // A pixel's width either side of the edge may go either way when magnified.
            let edge = (x as f32 + 0.5) / scale - (n / 2) as f32;
            if edge.abs() < 1.0 {
                continue;
            }
            let want = if edge < 0.0 {
                [canvas[0], canvas[1], canvas[2]]
            } else {
                [half; 3]
            };
            for c in 0..3 {
                assert!(
                    (p[c] - want[c]).abs() < 2e-3,
                    "scale {scale}: pixel {x} channel {c}: {} vs {}",
                    p[c],
                    want[c]
                );
            }
        }
    }
    // False colour: the stretched value through the map.
    let map = specular_inspect::ColourMap::Viridis;
    let out = draw_scaled(
        &gpu,
        &mut renderer,
        &img,
        RenderOptions {
            canvas,
            false_colour: Some(map),
            ..RenderOptions::default()
        },
        1.0,
    );
    let want = map.sample_linear(0.5);
    let p = out[(n / 2 * n + n - 4) as usize];
    for c in 0..3 {
        assert!((p[c] - want[c]).abs() < 4e-3, "{p:?} vs {want:?}");
    }
    assert_eq!(&out[(n / 2 * n + 4) as usize][..3], &canvas[..3]);
}

#[test]
fn isotherm_paints_values_past_the_level() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let n = 64u32;
    // A ramp of temperatures 0..=63 along x.
    let data: Vec<f32> = (0..n * n).map(|i| (i % n) as f32).collect();
    let mut img = renderer.create_image(&gpu, 9, n, n, None);
    renderer.set_value_channels(&gpu, &mut img, &[&data]);
    let red = [1.0, 0.0, 0.0];
    for above in [true, false] {
        let iso = Isotherm {
            above,
            level: 40.0,
            colour: red,
        };
        let look = ScienceLook {
            stretch: Stretch::Linear,
            channels: vec![ChannelLook {
                lo: 0.0,
                hi: 63.0,
                colour: [1.0, 1.0, 1.0],
                on: true,
                mid: 0.5,
            }],
            luts: Vec::new(),
            colour_table: None,
            hillshade: None,
            isotherm: Some(iso),
        };
        assert!(renderer.set_science(&gpu, &mut img, Some(look.clone())));
        let out = draw_scaled(&gpu, &mut renderer, &img, RenderOptions::default(), 1.0);
        for x in [2u32, 30, 39, 41, 50, 62] {
            let p = out[(n / 2 * n + x) as usize];
            let want = look.colour(&[x as f32]);
            assert_eq!(iso.marks(x as f32), want == red, "{x}");
            for c in 0..3 {
                let w = srgb_to_linear(f64::from(want[c])) as f32;
                assert!(
                    (p[c] - w).abs() < 3e-3,
                    "above {above}, x {x}, channel {c}: {} vs {w}",
                    p[c]
                );
            }
        }
    }
}

#[test]
fn downscaled_no_data_is_left_out() {
    let nan = f32::NAN;
    let data = [nan, 2.0, nan, nan, 4.0, nan, nan, nan];
    let (out, w, h) = downscale(&data, 4, 2, 2);
    assert_eq!((w, h), (2, 1));
    assert_eq!(out[0], 3.0);
    assert!(out[1].is_nan());
}

/// Terrain for the hillshade tests: ridges and a slope, `w`×`h`, a hole of no data.
fn terrain(w: usize, h: usize) -> Vec<f32> {
    let mut v: Vec<f32> = (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as f32, (i / w) as f32);
            40.0 * (x * 0.4).sin() * (y * 0.3).cos() + 3.0 * x - 2.0 * y + ((i * 7919) % 13) as f32
        })
        .collect();
    v[h / 2 * w + w / 2] = f32::NAN;
    v
}

/// [`Hillshade::shade`] against the slope-and-aspect form cartographic tools publish
/// (Burrough and McDonnell: cos zenith · cos slope + sin zenith · sin slope ·
/// cos(sun − aspect), the gradient from Horn's weights), on terrain, with hand-checked
/// values for a flat plane and a 45° slope facing west.
#[test]
fn hillshade_matches_the_slope_aspect_form() {
    let (w, h) = (17usize, 13usize);
    let v = terrain(w, h);
    for (az, alt, step, z) in [
        (315.0f32, 45.0f32, 30.0f32, 1.0f32),
        (135.0, 20.0, 10.0, 2.5),
        (0.0, 70.0, 90.0, 1.0),
    ] {
        let hs = Hillshade {
            azimuth: az,
            altitude: alt,
            step: [step, -step],
            z,
            kind: Relief::Shade,
        };
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let n: Vec<f64> = (0..9)
                    .map(|k| f64::from(v[(y + k / 3 - 1) * w + x + k % 3 - 1]))
                    .collect();
                if n.iter().any(|x| !x.is_finite()) {
                    assert_eq!(
                        hs.shade(&v, w, h, x, y).is_none(),
                        !v[y * w + x].is_finite()
                    );
                    continue;
                }
                let s = f64::from(step);
                // Rows run south: the published form's y gradient is down the rows.
                let dzdx = ((n[2] + 2.0 * n[5] + n[8]) - (n[0] + 2.0 * n[3] + n[6])) / (8.0 * s);
                let dzdy = ((n[6] + 2.0 * n[7] + n[8]) - (n[0] + 2.0 * n[1] + n[2])) / (8.0 * s);
                let slope = (f64::from(z) * dzdx.hypot(dzdy)).atan();
                let aspect = dzdy.atan2(-dzdx);
                let zen = (90.0 - f64::from(alt)).to_radians();
                let az_math = (360.0 - f64::from(az) + 90.0).to_radians();
                let want = (zen.cos() * slope.cos()
                    + zen.sin() * slope.sin() * (az_math - aspect).cos())
                .max(0.0);
                let got = f64::from(hs.shade(&v, w, h, x, y).unwrap());
                assert!(
                    (got - want).abs() < 1e-5,
                    "{az}/{alt} at {x},{y}: {got} vs {want}"
                );
            }
        }
    }
    let hs = Hillshade::default();
    let flat = vec![100.0f32; 9];
    let s = hs.shade(&flat, 3, 3, 1, 1).unwrap();
    assert!((s - 45f32.to_radians().sin()).abs() < 1e-6);
    // Rising one metre a metre to the east: faces west, towards the north-west sun.
    let east: Vec<f32> = (0..9).map(|i| (i % 3) as f32).collect();
    let s = hs.shade(&east, 3, 3, 1, 1).unwrap();
    assert!((s - (0.5 + 0.5f32.sqrt() / 2.0)).abs() < 1e-6, "{s}");
}

/// The GPU hillshade against [`Hillshade::shade`] pixel for pixel at 1:1 (grey, and
/// darkening false colour), no data showing the canvas; minified and magnified, a flat
/// plane keeps its shade.
#[test]
fn gpu_hillshade_matches_the_reference() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let (w, h) = (24usize, 18usize);
    let v = terrain(w, h);
    let mut img = renderer.create_image(&gpu, 9, w as u32, h as u32, None);
    renderer.set_value_channels(&gpu, &mut img, &[&v]);
    let hs = Hillshade {
        step: [30.0, -30.0],
        ..Hillshade::default()
    };
    let look = ScienceLook {
        stretch: Stretch::Linear,
        channels: vec![ChannelLook {
            lo: -60.0,
            hi: 120.0,
            colour: [1.0, 1.0, 1.0],
            on: true,
            mid: 0.5,
        }],
        luts: Vec::new(),
        colour_table: None,
        hillshade: Some(hs),
        isotherm: None,
    };
    assert!(renderer.set_science(&gpu, &mut img, Some(look.clone())));
    let map = specular_inspect::ColourMap::Viridis;
    for fc in [None, Some(map)] {
        let out = draw_with(
            &gpu,
            &mut renderer,
            &img,
            RenderOptions {
                false_colour: fc,
                ..RenderOptions::default()
            },
        );
        for y in 0..h {
            for x in 0..w {
                let got = out[y * w + x];
                let Some(sh) = hs.shade(&v, w, h, x, y) else {
                    assert_eq!(got[..3], [0.0; 3], "no data at {x},{y} shows the canvas");
                    continue;
                };
                let base = match fc {
                    None => [1.0; 3],
                    Some(m) => m.sample(look.colour(&[v[y * w + x]])[0]),
                };
                for c in 0..3 {
                    let want = srgb_to_linear(f64::from(base[c] * sh)) as f32;
                    assert!(
                        (got[c] - want).abs() <= 2e-3 + want * 4e-3,
                        "{fc:?} at {x},{y} channel {c}: {} vs {want}",
                        got[c]
                    );
                }
            }
        }
    }
    // A flat plane: the same shade at every level.
    let flat = vec![250.0f32; 64 * 64];
    let mut img = renderer.create_image(&gpu, 10, 64, 64, None);
    renderer.set_value_channels(&gpu, &mut img, &[&flat]);
    assert!(renderer.set_science(&gpu, &mut img, Some(look)));
    let want = srgb_to_linear(f64::from(45f32.to_radians().sin())) as f32;
    for scale in [0.25, 0.5, 1.0, 3.0] {
        let out = draw_scaled(&gpu, &mut renderer, &img, RenderOptions::default(), scale);
        for p in &out {
            assert!((p[0] - want).abs() < 2e-3, "scale {scale}: {p:?} vs {want}");
        }
    }
}

/// The GPU slope and aspect views against [`Hillshade::colour`] pixel for pixel at 1:1, no
/// data showing the canvas, a flat plane in the aspect's grey.
#[test]
fn gpu_slope_and_aspect_match_the_reference() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let (w, h) = (24usize, 18usize);
    let mut v = terrain(w, h);
    // A flat patch (aspect undefined there).
    for y in 1..5 {
        for x in 1..6 {
            v[y * w + x] = 75.0;
        }
    }
    let mut img = renderer.create_image(&gpu, 11, w as u32, h as u32, None);
    renderer.set_value_channels(&gpu, &mut img, &[&v]);
    for kind in [Relief::Slope, Relief::Aspect] {
        let hs = Hillshade {
            step: [30.0, -20.0],
            kind,
            ..Hillshade::default()
        };
        let look = ScienceLook {
            stretch: Stretch::Linear,
            channels: vec![ChannelLook {
                lo: -60.0,
                hi: 120.0,
                colour: [1.0, 1.0, 1.0],
                on: true,
                mid: 0.5,
            }],
            luts: Vec::new(),
            colour_table: None,
            hillshade: Some(hs),
            isotherm: None,
        };
        assert!(renderer.set_science(&gpu, &mut img, Some(look)));
        let out = draw_with(&gpu, &mut renderer, &img, RenderOptions::default());
        let mut flat = 0;
        for y in 0..h {
            for x in 0..w {
                let got = out[y * w + x];
                let Some(want) = hs.colour(&v, w, h, x, y) else {
                    assert_eq!(got[..3], [0.0; 3], "no data at {x},{y} shows the canvas");
                    continue;
                };
                if want == [FLAT_GREY; 3] {
                    flat += 1;
                }
                for c in 0..3 {
                    let want = srgb_to_linear(f64::from(want[c])) as f32;
                    assert!(
                        (got[c] - want).abs() <= 3e-3 + want * 5e-3,
                        "{kind:?} at {x},{y} channel {c}: {} vs {want}",
                        got[c]
                    );
                }
            }
        }
        if kind == Relief::Aspect {
            assert!(flat >= 3, "{flat}");
        }
    }
}

/// A GSDF display (PS3.14): P-values through the window come out as the luminance
/// colour-science's `eotf_DICOMGSDF` gives for equal JND steps between the display's black
/// and white (`specular-dicom/tests/data/gsdf_oracle.tsv`, rows `P`), as relative luminance
/// over that range. Off, the same greys stay sRGB.
#[test]
fn gsdf_display_matches_colour_science() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let oracle = include_str!("../../../formats/specular-dicom/tests/data/gsdf_oracle.tsv");
    // (Lmin, Lmax, [(P, L)]) per display range.
    type Range = (f64, f64, Vec<(f64, f64)>);
    let mut ranges: Vec<Range> = Vec::new();
    for line in oracle.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let ["P", lmin, lmax, p, l] = f[..] else {
            continue;
        };
        let v: Vec<f64> = [lmin, lmax, p, l]
            .iter()
            .map(|s| s.parse().unwrap())
            .collect();
        match ranges.last_mut() {
            Some(r) if (r.0, r.1) == (v[0], v[1]) => r.2.push((v[2], v[3])),
            _ => ranges.push((v[0], v[1], vec![(v[2], v[3])])),
        }
    }
    assert_eq!(ranges.len(), 4);
    let identity = win(0.5, 1.0, VoiShape::LinearExact, false);
    let (mut worst, mut worst_jnd) = (0f64, 0f64);
    let mut bad = Vec::new();
    for (l_min, l_max, rows) in &ranges {
        let fails = bad.len();
        let row: Vec<f32> = rows.iter().map(|r| r.0 as f32).collect();
        let mut img = upload(&gpu, &mut renderer, 3, &row);
        assert!(img.set_window(Some(identity)));
        let g = Gsdf {
            j_min: specular_dicom::gsdf::jnd_index(*l_min),
            j_max: specular_dicom::gsdf::jnd_index(*l_max),
            l_min: *l_min,
            l_max: *l_max,
        };
        let opts = RenderOptions {
            gsdf: Some(g),
            ..RenderOptions::default()
        };
        let out = draw_with(&gpu, &mut renderer, &img, opts);
        for (i, &(p, want)) in rows.iter().enumerate() {
            let got = out[i];
            assert!(got[0] == got[1] && got[1] == got[2], "pixel {i}: {got:?}");
            let lum = l_min + f64::from(got[0]) * (l_max - l_min);
            let err = (lum / want - 1.0).abs();
            worst = worst.max(err);
            let jnd = (specular_dicom::gsdf::jnd_index(lum)
                - specular_dicom::gsdf::jnd_index(want))
            .abs();
            worst_jnd = worst_jnd.max(jnd);
            // Within the half-float target's step (11 significant bits, truncated), which
            // sets the floor: about 0.1 JND in the brightest steps.
            let lin = (want - l_min) / (l_max - l_min);
            if (f64::from(got[0]) - lin).abs() > lin * 1.5e-3 + 2e-5 {
                bad.push(format!(
                    "{l_min}..{l_max} cd/m², P {p}: {lum} cd/m² vs {want} ({jnd} JND)"
                ));
            }
        }
        // One line per display range for tools/ci/oracle-pass-rates.sh.
        let status = if bad.len() == fails { "pass" } else { "fail" };
        println!("ORACLE\tDICOM GSDF\t{l_min}-{l_max} cd/m2\t{status}");
        let plain = draw_with(&gpu, &mut renderer, &img, RenderOptions::default());
        check(&plain, &row, &identity, "sRGB greys");
    }
    assert!(bad.is_empty(), "{bad:#?}");
    eprintln!("GSDF display: worst error {worst_jnd:.4} JND, {worst:.2e} relative luminance");
}
