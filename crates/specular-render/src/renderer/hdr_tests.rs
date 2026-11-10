//! HDR paths of the renderer: shader validation (no GPU needed) and GPU round trips of
//! 16-bit PQ/HLG, half-float and gain-mapped images against the CPU reference in
//! `specular_core::hdr`. GPU tests skip without an adapter.

use specular_core::hdr::{self, HdrEncoding, Primaries, Transfer};
use specular_core::{GainMap, GainMapParams, Orientation, PixelLayout, Raster, RasterPlane};
use specular_gfx::{FULLSCREEN_WGSL, Gpu, HDR, wgpu};

use super::*;
use crate::view::{PxRect, View};

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
    let with = |s: &str| format!("{FULLSCREEN_WGSL}\n{s}");
    validate("convert", &with(include_str!("../shaders/convert.wgsl")));
    validate("image", &with(include_str!("../shaders/image.wgsl")));
    validate("mip", &with(&mip_source(false)));
    validate("mip hdr", &with(&mip_source(true)));
}

pub(super) fn wide_raster(
    w: u32,
    h: u32,
    layout: PixelLayout,
    px: impl Fn(u32, u32) -> [u16; 4],
) -> Raster {
    let mut data = Vec::with_capacity((w * h * 8) as usize);
    for y in 0..h {
        for x in 0..w {
            for v in px(x, y) {
                data.extend_from_slice(&v.to_ne_bytes());
            }
        }
    }
    Raster {
        width: w,
        height: h,
        origin: (0, 0),
        scale: 1,
        layout,
        planes: vec![RasterPlane {
            width: w,
            height: h,
            stride: w as usize * 8,
            data,
            h_div: 1,
            v_div: 1,
        }],
        truncated: false,
    }
}

fn rgb8(w: u32, h: u32, px: impl Fn(u32, u32) -> [u8; 3]) -> Raster {
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

/// Draws the whole image 1:1 (nearest) into an HDR target and reads it back as f32 RGBA.
pub(super) fn draw(gpu: &Gpu, renderer: &mut ImageRenderer, img: &GpuImage) -> Vec<[f32; 4]> {
    draw_with(gpu, renderer, img, RenderOptions::default())
}

/// [`draw`] with other options (`hq` and `nearest_from` are overridden).
pub(super) fn draw_with(
    gpu: &Gpu,
    renderer: &mut ImageRenderer,
    img: &GpuImage,
    opts: RenderOptions,
) -> Vec<[f32; 4]> {
    let opts = RenderOptions {
        hq: false,
        nearest_from: 0.5,
        ..opts
    };
    draw_scaled(gpu, renderer, img, opts, 1.0)
}

/// The whole image at `scale` (output pixels per image pixel, size rounded up), as given.
pub(super) fn draw_scaled(
    gpu: &Gpu,
    renderer: &mut ImageRenderer,
    img: &GpuImage,
    opts: RenderOptions,
    scale: f32,
) -> Vec<[f32; 4]> {
    let (iw, ih) = (img.width, img.height);
    let w = (iw as f32 * scale).ceil() as u32;
    let h = (ih as f32 * scale).ceil() as u32;
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("hdr test target"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let tv = target.create_view(&Default::default());
    let view = View::fixed(
        (iw, ih),
        Orientation::NORMAL,
        PxRect {
            x: 0.0,
            y: 0.0,
            w: w as f32,
            h: h as f32,
        },
        scale,
        (iw as f32 * 0.5, ih as f32 * 0.5),
    );
    let row = (w * 8).div_ceil(256) * 256;
    let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("hdr readback"),
        size: (row * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    renderer.render(gpu, &mut enc, &tv, (w, h), Some(img), &view, &opts);
    enc.copy_texture_to_buffer(
        target.as_image_copy(),
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
    let data = buf.slice(..).get_mapped_range().expect("mapped");
    let mut out = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let o = (y * row + x * 8) as usize;
            let c = |i: usize| {
                hdr::f16_to_f32(u16::from_le_bytes([data[o + 2 * i], data[o + 2 * i + 1]]))
            };
            out.push([c(0), c(1), c(2), c(3)]);
        }
    }
    out
}

pub(super) fn gpu() -> Option<Gpu> {
    match Gpu::new() {
        Ok(g) => Some(g),
        Err(e) => {
            eprintln!("no GPU, skipped: {e}");
            None
        }
    }
}

fn close(got: f32, want: f32, rel: f32) -> bool {
    (got - want).abs() <= rel * want.abs().max(0.05)
}

#[test]
fn pq_and_hlg_round_trip() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let (w, h) = (64, 4);
    for transfer in [Transfer::Pq, Transfer::Hlg, Transfer::Linear] {
        for (display, primaries) in [
            (1.0, Primaries::Bt2020),
            (8.0, Primaries::Bt2020),
            (64.0, Primaries::DisplayP3),
        ] {
            renderer.set_display_peak(display);
            let enc = HdrEncoding {
                transfer,
                primaries,
            };
            let code = |x: u32, c: u32| ((x * 1024 + c * 300) % 65536) as u16;
            let r = wide_raster(w, h, PixelLayout::Rgba16(enc), |x, _| {
                [code(x, 0), code(x, 1), code(x, 2), 65535]
            });
            let mut img = renderer.create_image(&gpu, 7, w, h, None);
            renderer.upload_overview(&gpu, &mut img, &r);
            assert!(img.hdr);
            let peak = img.content_peak;
            let out = draw(&gpu, &mut renderer, &img);
            for x in 0..w {
                let lin = hdr::decode16(enc, [code(x, 0), code(x, 1), code(x, 2)]);
                let m = lin[0].max(lin[1]).max(lin[2]);
                let k = hdr::tone_map_scale(m, peak, display);
                let got = out[x as usize];
                for c in 0..3 {
                    let want = (lin[c] * k).min(65504.0);
                    assert!(
                        close(got[c], want, 0.02),
                        "{transfer:?} display {display} x {x} c {c}: {} vs {want} (peak {peak})",
                        got[c]
                    );
                }
            }
        }
    }
}

#[test]
fn half_float_round_trip() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    renderer.set_display_peak(16.0);
    let (w, h) = (32, 2);
    let v = |x: u32| x as f32 * 0.37;
    let r = wide_raster(w, h, PixelLayout::Rgba16F(Primaries::Bt709), |x, _| {
        [
            hdr::f32_to_f16(v(x)),
            hdr::f32_to_f16(v(x) * 0.5),
            hdr::f32_to_f16(0.25),
            hdr::f32_to_f16(1.0),
        ]
    });
    let mut img = renderer.create_image(&gpu, 8, w, h, None);
    renderer.upload_overview(&gpu, &mut img, &r);
    assert!((img.content_peak - v(31)).abs() < 0.02);
    let out = draw(&gpu, &mut renderer, &img);
    for x in 0..w {
        let want = v(x);
        let k = hdr::tone_map_scale(want, img.content_peak, 16.0);
        assert!(
            close(out[x as usize][0], want * k, 0.01),
            "x {x}: {:?}",
            out[x as usize]
        );
    }
}

#[test]
fn gain_map_follows_the_display_peak() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let (w, h) = (32, 4);
    let base = rgb8(w, h, |x, _| [(x * 8) as u8, 128, 64]);
    let map = {
        let mut data = Vec::new();
        for _ in 0..h {
            for x in 0..w {
                let g = (x * 8) as u8;
                data.extend_from_slice(&[g, g, g, 255]);
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
    };
    let params = GainMapParams {
        gain_min: [0.0; 3],
        gain_max: [2.0; 3],
        gamma: [1.0; 3],
        alt_headroom: 2.0,
        ..Default::default()
    };
    let gm = GainMap {
        map,
        params,
        source: "test",
    };
    let mut img = renderer.create_image(&gpu, 9, w, h, None);
    renderer.upload_overview(&gpu, &mut img, &base);
    renderer.set_gain_map(&gpu, &mut img, Some(&gm));
    assert!(img.is_hdr() && !img.hdr);
    for display in [1.0, 2.0, 4.0, 10.0] {
        renderer.set_display_peak(display);
        let wgt = params.weight(display);
        let out = draw(&gpu, &mut renderer, &img);
        for x in 0..w {
            let g = (x * 8) as f32 / 255.0;
            let b = [(x * 8) as u8, 128, 64].map(|v| hdr::srgb_to_linear(v as f32 / 255.0));
            for c in 0..3 {
                let want = params.apply(c, b[c], g, wgt);
                assert!(
                    close(out[x as usize][c], want, 0.03),
                    "display {display} x {x} c {c}: {} vs {want}",
                    out[x as usize][c]
                );
            }
        }
    }
}

/// An HDR (half-float) base with a per-channel map applied in Display P3: the weight is
/// negative below the base headroom, and the gains go through the application space.
#[test]
fn gain_map_on_an_hdr_base_in_its_application_space() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let (w, h) = (32, 2);
    let b = |x: u32| [x as f32 / 31.0, 0.5, 0.25];
    let base = wide_raster(w, h, PixelLayout::Rgba16F(Primaries::Bt709), |x, _| {
        let c = b(x);
        [
            hdr::f32_to_f16(c[0]),
            hdr::f32_to_f16(c[1]),
            hdr::f32_to_f16(c[2]),
            hdr::f32_to_f16(1.0),
        ]
    });
    let g = |x: u32| [(x * 8) as u8, 255 - (x * 8) as u8, 128];
    let map = {
        let mut data = Vec::new();
        for _ in 0..h {
            for x in 0..w {
                let v = g(x);
                data.extend_from_slice(&[v[0], v[1], v[2], 255]);
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
    };
    let params = GainMapParams {
        gain_min: [0.0, 0.25, 0.0],
        gain_max: [2.0, 1.5, 1.0],
        gamma: [1.0; 3],
        base_offset: [0.0; 3],
        alt_offset: [0.0; 3],
        base_headroom: 2.0,
        alt_headroom: 0.0,
        base_is_hdr: true,
        space: Primaries::DisplayP3,
    };
    let gm = GainMap {
        map,
        params,
        source: "test",
    };
    let mut img = renderer.create_image(&gpu, 10, w, h, None);
    renderer.upload_overview(&gpu, &mut img, &base);
    renderer.set_gain_map(&gpu, &mut img, Some(&gm));
    for display in [1.0, 2.0, 4.0] {
        renderer.set_display_peak(display);
        let wgt = params.weight(display);
        assert!(wgt <= 0.0);
        let out = draw(&gpu, &mut renderer, &img);
        for x in 0..w {
            let gs = g(x).map(|v| v as f32 / 255.0);
            let want = params.apply_rgb(b(x), gs, wgt);
            for c in 0..3 {
                assert!(
                    close(out[x as usize][c], want[c], 0.03),
                    "display {display} x {x} c {c}: {} vs {}",
                    out[x as usize][c],
                    want[c]
                );
            }
        }
    }
}

/// A 16-bit PQ base (as AVIF and HEIC decode HDR stills) whose map leads down to an SDR
/// alternate: the same shader path as any wide base, the weight follows the display from
/// −1 (SDR: the full map down) to 0 (enough headroom: the base as is), and on an SDR
/// display the result matches the CPU SDR rendition the thumbnails use.
#[test]
fn gain_map_on_a_pq_base_towards_sdr() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let (w, h) = (32, 2);
    let enc = HdrEncoding {
        transfer: Transfer::Pq,
        primaries: Primaries::Bt2020,
    };
    // PQ codes from deep shadows to about 1000 nits, different per channel.
    let code = |x: u32, c: u32| (8000 + x * 1300 + c * 2500) as u16;
    let base = wide_raster(w, h, PixelLayout::Rgba16(enc), |x, _| {
        [code(x, 0), code(x, 1), code(x, 2), 65535]
    });
    let g = |x: u32| [(x * 8) as u8, 200 - (x * 4) as u8, 128];
    let map = {
        let mut data = Vec::new();
        for _ in 0..h {
            for x in 0..w {
                let v = g(x);
                data.extend_from_slice(&[v[0], v[1], v[2], 255]);
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
    };
    // libavif's Seine metadata, rounded: the base is 1.3 stops above its SDR alternate.
    let params = GainMapParams {
        gain_min: [-0.26; 3],
        gain_max: [1.28, 1.25, 1.2],
        gamma: [0.93; 3],
        base_offset: [1.0 / 64.0; 3],
        alt_offset: [1.0 / 64.0; 3],
        base_headroom: 1.3,
        alt_headroom: 0.0,
        base_is_hdr: true,
        space: Primaries::Bt709,
    };
    let gm = GainMap {
        map,
        params,
        source: "test",
    };
    let mut img = renderer.create_image(&gpu, 11, w, h, None);
    renderer.upload_overview(&gpu, &mut img, &base);
    renderer.set_gain_map(&gpu, &mut img, Some(&gm));
    assert!(img.hdr);
    let content = img.content_peak;
    for display in [1.0, 0.65f32.exp2(), 1.3f32.exp2(), 8.0] {
        renderer.set_display_peak(display);
        let wgt = params.weight(display);
        assert!((-1.0..=0.0).contains(&wgt));
        let peak = params.peak(content, wgt);
        let out = draw(&gpu, &mut renderer, &img);
        for x in 0..w {
            let lin = hdr::decode16(enc, [code(x, 0), code(x, 1), code(x, 2)]);
            let gs = g(x).map(|v| v as f32 / 255.0);
            let m = params.apply_rgb(lin, gs, wgt);
            let k = hdr::tone_map_scale(m[0].max(m[1]).max(m[2]), peak, display);
            for c in 0..3 {
                assert!(
                    close(out[x as usize][c], m[c] * k, 0.03),
                    "display {display} (w {wgt}) x {x} c {c}: {} vs {} (peak {peak})",
                    out[x as usize][c],
                    m[c] * k
                );
            }
        }
        if display == 1.0 {
            let r = hdr::sdr_rendition(&base, &gm, content).unwrap();
            let cpu = &r.planes[0].data;
            for x in 0..w as usize {
                for c in 0..3 {
                    let v = out[x][c].clamp(0.0, 1.0);
                    let gpu8 = (hdr::linear_to_srgb(v) * 255.0 + 0.5) as u8;
                    assert!(
                        gpu8.abs_diff(cpu[x * 4 + c]) <= 2,
                        "x {x} c {c}: GPU {gpu8} vs CPU {}",
                        cpu[x * 4 + c]
                    );
                }
            }
        }
    }
}
