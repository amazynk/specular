//! Channel isolation in the image shader against `ChannelView::grey`, the CPU reference.
//! GPU tests skip without an adapter.

use specular_core::hdr::{self, Primaries};
use specular_core::{PixelLayout, Raster, RasterPlane};
use specular_inspect::{ChannelView, srgb_decode};

use super::hdr_tests::{draw_scaled, draw_with, gpu};
use super::*;

fn raster(w: u32, h: u32, layout: PixelLayout, bpp: usize, data: Vec<u8>) -> Raster {
    Raster {
        width: w,
        height: h,
        origin: (0, 0),
        scale: 1,
        layout,
        planes: vec![RasterPlane {
            width: w,
            height: h,
            stride: w as usize * bpp,
            data,
            h_div: 1,
            v_div: 1,
        }],
        truncated: false,
    }
}

/// Straight-alpha 8-bit pixels: opaque orange, half-transparent orange, transparent blue,
/// opaque green-ish.
const PIXELS: [[u8; 4]; 4] = [
    [255, 128, 0, 255],
    [255, 128, 0, 128],
    [0, 0, 255, 0],
    [64, 200, 32, 255],
];

fn straight(px: [u8; 4]) -> [f32; 4] {
    let c = |i: usize| srgb_decode(px[i] as f32 / 255.0);
    [c(0), c(1), c(2), px[3] as f32 / 255.0]
}

fn premul(px: [u8; 4]) -> [f32; 4] {
    let [r, g, b, a] = straight(px);
    [r * a, g * a, b * a, a]
}

const ISOLATED: [ChannelView; 5] = [
    ChannelView::Red,
    ChannelView::Green,
    ChannelView::Blue,
    ChannelView::Alpha,
    ChannelView::Luma,
];

#[test]
fn rgba8_channels_are_grey_and_unpremultiplied() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let (w, h) = (PIXELS.len() as u32, 2);
    let data: Vec<u8> = (0..h)
        .flat_map(|_| PIXELS.iter().flatten().copied())
        .collect();
    let r = raster(w, h, PixelLayout::Rgba, 4, data);
    let mut img = renderer.create_image(&gpu, 1, w, h, None);
    renderer.upload_overview(&gpu, &mut img, &r);
    for view in ISOLATED {
        let out = draw_with(
            &gpu,
            &mut renderer,
            &img,
            RenderOptions {
                channel: view,
                ..RenderOptions::default()
            },
        );
        for (i, px) in PIXELS.iter().enumerate() {
            let want = view.grey(straight(*px)).unwrap();
            let got = out[i];
            assert!(
                got[0] == got[1] && got[1] == got[2] && got[3] == 1.0,
                "{view:?} pixel {i}: not opaque grey: {got:?}"
            );
            // 8-bit sRGB storage of the straight value: within a code step.
            assert!(
                (got[0] - want).abs() <= 0.005 + want * 0.01,
                "{view:?} pixel {i}: {} vs {want}",
                got[0]
            );
        }
        // The half-transparent orange shows its full red, not half of it, and the fully
        // transparent pixel its blue.
        if view == ChannelView::Red {
            assert!(out[1][0] > 0.99, "unpremultiplied red: {}", out[1][0]);
        }
        if view == ChannelView::Blue {
            assert!(out[2][0] > 0.99, "hidden blue: {}", out[2][0]);
        }
    }
}

#[test]
fn alpha_backdrop_views_composite_over_the_given_backdrop() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let (w, h) = (PIXELS.len() as u32, 1);
    let r = raster(w, h, PixelLayout::Rgba, 4, PIXELS.concat());
    let mut img = renderer.create_image(&gpu, 2, w, h, None);
    renderer.upload_overview(&gpu, &mut img, &r);
    let matte = [0.0, 1.0, 0.0, 1.0];
    let out = draw_with(
        &gpu,
        &mut renderer,
        &img,
        RenderOptions {
            channel: ChannelView::AlphaMatte,
            checker_a: matte,
            checker_b: matte,
            ..RenderOptions::default()
        },
    );
    // Transparent: the matte. Opaque: the image. Half: the blend.
    assert!(
        (out[2][1] - 1.0).abs() < 0.01 && out[2][0] < 0.01,
        "{:?}",
        out[2]
    );
    let p = premul(PIXELS[0]);
    assert!((out[0][0] - p[0]).abs() < 0.01 && (out[0][1] - p[1]).abs() < 0.01);
    let p = premul(PIXELS[1]);
    let want_g = p[1] + (1.0 - p[3]);
    assert!(
        (out[1][1] - want_g).abs() < 0.02,
        "{} vs {want_g}",
        out[1][1]
    );
}

/// A transparent image whose colour is blue, left eighth opaque red. The colour views show the
/// hidden blue through every sampling path (nearest, bilinear, trilinear and Lanczos over the
/// overview and its mips, bilinear and Lanczos over tiles), and the normal view never lets
/// it bleed into the red edge.
#[test]
fn hidden_colour_survives_filtering_and_never_bleeds() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 64 << 20);
    let n = 128u32;
    let edge = n / 8;
    let px = |x: u32| {
        if x < edge {
            [255, 0, 0, 255]
        } else {
            [0, 0, 255, 0]
        }
    };
    let full: Vec<u8> = (0..n * n).flat_map(|i| px(i % n)).collect();
    let half: Vec<u8> = (0..n * n / 4).flat_map(|i| px(i % (n / 2) * 2)).collect();
    let black = [0.0, 0.0, 0.0, 1.0];
    for tiled in [false, true] {
        let mut img = renderer.create_image(&gpu, 10 + tiled as u64, n, n, None);
        if tiled {
            // A half-size overview, so that tiles serve every scale from 1 up.
            renderer.upload_overview(
                &gpu,
                &mut img,
                &raster(n / 2, n / 2, PixelLayout::Rgba, 4, half.clone()),
            );
            let r = raster(n, n, PixelLayout::Rgba, 4, full.clone());
            assert_eq!(renderer.upload_tiles(&gpu, &mut img, &r, 0, &[(0, 0)]), 1);
        } else {
            renderer.upload_overview(
                &gpu,
                &mut img,
                &raster(n, n, PixelLayout::Rgba, 4, full.clone()),
            );
        }
        let scales: &[f32] = if tiled {
            &[1.0, 2.5]
        } else {
            &[0.3, 0.7, 1.0, 2.5]
        };
        for &scale in scales {
            for hq in [false, true] {
                let opts = |channel| RenderOptions {
                    hq,
                    nearest_from: 100.0,
                    channel,
                    checker_a: black,
                    checker_b: black,
                    ..RenderOptions::default()
                };
                let what = format!("tiled {tiled} scale {scale} hq {hq}");
                let w = (n as f32 * scale).ceil() as usize;
                let blue = draw_scaled(&gpu, &mut renderer, &img, opts(ChannelView::Blue), scale);
                let red = draw_scaled(&gpu, &mut renderer, &img, opts(ChannelView::Red), scale);
                let normal =
                    draw_scaled(&gpu, &mut renderer, &img, opts(ChannelView::Normal), scale);
                for (i, ((b, r), c)) in blue.iter().zip(&red).zip(&normal).enumerate() {
                    let x = (i % w) as f32 / scale;
                    let y = (i / w) as f32 / scale;
                    // Clear of the edge and its filter footprint (mips included), and of the
                    // image border (partly covered pixels), the hidden side is blue.
                    let inner = |v: f32, from: f32| v > from && v < n as f32 - 4.0 / scale;
                    if inner(x, edge as f32 + 8.0 / scale.min(1.0)) && inner(y, 4.0 / scale) {
                        assert!(
                            b[0] > 0.98 && r[0] < 0.02,
                            "{what} pixel {i}: blue {b:?} red {r:?}"
                        );
                    }
                    assert!(c[1] < 1e-3 && c[2] < 1e-3, "{what} pixel {i}: bleed {c:?}");
                }
            }
        }
    }
}

/// Half-float HDR: the colour views show the channel after tone mapping, i.e. the same value
/// the normal view puts on screen, even above SDR white.
#[test]
fn hdr_channels_follow_tone_mapping() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let vals: [[f32; 4]; 4] = [
        [6.0, 2.0, 0.5, 1.0],
        [0.2, 0.9, 3.0, 1.0],
        [0.5, 0.25, 0.1, 1.0],
        [1.5, 1.5, 1.5, 1.0],
    ];
    let (w, h) = (vals.len() as u32, 1);
    let data = vals
        .iter()
        .flatten()
        .flat_map(|v| hdr::f32_to_f16(*v).to_ne_bytes())
        .collect();
    let r = raster(w, h, PixelLayout::Rgba16F(Primaries::Bt709), 8, data);
    for display in [1.0, 4.0] {
        renderer.set_display_peak(display);
        let mut img = renderer.create_image(&gpu, 3, w, h, None);
        renderer.upload_overview(&gpu, &mut img, &r);
        assert!(img.hdr);
        let normal = draw_with(&gpu, &mut renderer, &img, RenderOptions::default());
        for view in ISOLATED {
            let out = draw_with(
                &gpu,
                &mut renderer,
                &img,
                RenderOptions {
                    channel: view,
                    ..RenderOptions::default()
                },
            );
            for i in 0..vals.len() {
                let want = view.grey(normal[i]).unwrap();
                let got = out[i][0];
                assert!(
                    (got - want).abs() <= 0.01 * want.max(1.0),
                    "display {display} {view:?} pixel {i}: {got} vs {want}"
                );
            }
        }
        // Tone mapping is on at display peak 1: the 6.0 red is brought down.
        if display == 1.0 {
            assert!(normal[0][0] <= 1.01, "{:?}", normal[0]);
        }
    }
}

/// Draw cost of the image shader: a 4096² RGBA image (overview plus level-0 tiles) drawn
/// into a 2560×1440 target from the overview (scale 0.25, 0.35) and from tiles (1, 3), bilinear
/// and Lanczos. Run with `--ignored --nocapture`; prints milliseconds per frame.
#[test]
#[ignore]
fn bench_draw() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 512 << 20);
    let n = 4096u32;
    let data: Vec<u8> = (0..n * n)
        .flat_map(|i| {
            let (x, y) = (i % n, i / n);
            [(x * 7) as u8, (y * 5) as u8, (x ^ y) as u8, (x / 16) as u8]
        })
        .collect();
    let r = raster(n, n, PixelLayout::Rgba, 4, data);
    let mut img = renderer.create_image(&gpu, 9, n, n, None);
    renderer.upload_overview(&gpu, &mut img, &r);
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    eprintln!("overview upload {:.1} ms", renderer.last_upload_ms);
    let tiles: Vec<(u32, u32)> = (0..8).flat_map(|y| (0..8).map(move |x| (x, y))).collect();
    let stored = renderer.upload_tiles(&gpu, &mut img, &r, 0, &tiles);
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    eprintln!("{stored} tiles upload {:.1} ms", renderer.last_upload_ms);
    let (w, h) = (2560u32, 1440u32);
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
        format: specular_gfx::HDR,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let tv = target.create_view(&Default::default());
    for scale in [0.25f32, 0.35, 1.0, 3.0] {
        let view = crate::view::View::fixed(
            (n, n),
            specular_core::Orientation::NORMAL,
            crate::view::PxRect {
                x: 0.0,
                y: 0.0,
                w: w as f32,
                h: h as f32,
            },
            scale,
            (n as f32 * 0.5, n as f32 * 0.5),
        );
        for hq in [false, true] {
            for channel in [ChannelView::Normal, ChannelView::Red] {
                let opts = RenderOptions {
                    hq,
                    nearest_from: 100.0,
                    channel,
                    ..RenderOptions::default()
                };
                let frames = 60;
                let mut best = f64::MAX;
                for _ in 0..3 {
                    let t = std::time::Instant::now();
                    for _ in 0..frames {
                        let mut enc = gpu.device.create_command_encoder(&Default::default());
                        renderer.render(&gpu, &mut enc, &tv, (w, h), Some(&img), &view, &opts);
                        gpu.queue.submit([enc.finish()]);
                    }
                    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
                    best = best.min(t.elapsed().as_secs_f64() * 1e3 / frames as f64);
                }
                eprintln!("scale {scale} hq {hq} {channel:?}: {best:.3} ms/frame");
            }
        }
    }
}
