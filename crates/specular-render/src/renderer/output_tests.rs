//! The whole SDR display path, measured at the 8-bit output: the image renderer into the
//! compositor's half-float scene, the compositor's final pass (sRGB encode and blue-noise
//! dither) into an 8-bit target, read back. An 8-bit sRGB image drawn at 1:1 and at the
//! nearest-neighbour zooms must come out bit-exact: every output pixel holds the source
//! code of the pixel it covers. 16-bit images come out within one code, from the half-float
//! overview and from their 16-bit detail tiles, which hold the source codes exactly. GPU tests
//! skip without an adapter.

use specular_core::{Orientation, PixelLayout, Raster, RasterPlane};
use specular_gfx::{Compositor, Gpu, wgpu};

use super::hdr_tests::gpu;
use super::*;
use crate::view::{PxRect, View};

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

fn gray16(w: u32, h: u32, px: impl Fn(u32, u32) -> u16) -> Raster {
    let mut data = Vec::with_capacity((w * h * 2) as usize);
    for y in 0..h {
        for x in 0..w {
            data.extend_from_slice(&px(x, y).to_ne_bytes());
        }
    }
    Raster {
        width: w,
        height: h,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Gray16,
        planes: vec![RasterPlane {
            width: w,
            height: h,
            stride: w as usize * 2,
            data,
            h_div: 1,
            v_div: 1,
        }],
        truncated: false,
    }
}

/// An sRGB-coded 16-bit RGBA raster (alpha opaque).
fn rgb16(w: u32, h: u32, px: impl Fn(u32, u32) -> [u16; 3]) -> Raster {
    let mut data = Vec::with_capacity((w * h * 8) as usize);
    for y in 0..h {
        for x in 0..w {
            for v in px(x, y).into_iter().chain([u16::MAX]) {
                data.extend_from_slice(&v.to_ne_bytes());
            }
        }
    }
    Raster {
        width: w,
        height: h,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Rgba16(specular_core::hdr::HdrEncoding::SRGB),
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

/// Pseudo-random 16-bit codes, every channel independent.
fn noise16(x: u32, y: u32) -> [u16; 3] {
    let mut h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 13;
    [
        h as u16,
        (h >> 16) as u16,
        (h.wrapping_mul(0x27D4_EB2F) >> 8) as u16,
    ]
}

/// Every 8-bit code on every channel, in three rotations, then pseudo-random colours.
fn chart(x: u32, y: u32) -> [u8; 3] {
    if y < 3 {
        let v = x as u8;
        return match y {
            0 => [v, v.wrapping_add(85), v.wrapping_add(170)],
            1 => [v.wrapping_add(170), v, v.wrapping_add(85)],
            _ => [v.wrapping_add(85), v.wrapping_add(170), v],
        };
    }
    let mut h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    [(h >> 24) as u8, (h >> 16) as u8, (h >> 8) as u8]
}

/// Where the drawn pixels come from.
#[derive(Clone, Copy, PartialEq)]
enum Source {
    /// The whole-image overview.
    Overview,
    /// Detail tiles alone (no overview).
    Tiles,
}

/// Draws `raster` at `scale` (centred, output size rounded up) through the compositor into
/// an 8-bit target, from `src`; returns (width, height, RGBA8 pixels) and the output → source
/// mapping (scale, x offset, y offset).
fn draw_out(
    gpu: &Gpu,
    r: &mut ImageRenderer,
    raster: &Raster,
    scale: f32,
    opts: &RenderOptions,
) -> (u32, u32, Vec<[u8; 4]>, (f64, f64, f64)) {
    draw_from(gpu, r, raster, scale, opts, Source::Overview)
}

fn draw_from(
    gpu: &Gpu,
    r: &mut ImageRenderer,
    raster: &Raster,
    scale: f32,
    opts: &RenderOptions,
    src: Source,
) -> (u32, u32, Vec<[u8; 4]>, (f64, f64, f64)) {
    let (iw, ih) = (raster.width, raster.height);
    let w = (iw as f32 * scale).ceil() as u32;
    let h = (ih as f32 * scale).ceil() as u32;
    let fmt = wgpu::TextureFormat::Rgba8Unorm;
    let comp = Compositor::new(gpu, fmt, 8, w, h);
    let mut img = r.create_image(gpu, 43, iw, ih, None);
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
    match src {
        Source::Overview => r.upload_overview(gpu, &mut img, raster),
        Source::Tiles => upload_all_tiles(gpu, r, &mut img, raster, &view),
    }
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("output test target"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: fmt,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let tv = target.create_view(&Default::default());
    let row = (w * 4).div_ceil(256) * 256;
    let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("output readback"),
        size: (row * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    r.render(
        gpu,
        &mut enc,
        &comp.scene.view,
        (w, h),
        Some(&img),
        &view,
        opts,
    );
    comp.begin(&mut enc);
    comp.finish(&mut enc, &tv);
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
            let o = (y * row + x * 4) as usize;
            out.push([data[o], data[o + 1], data[o + 2], data[o + 3]]);
        }
    }
    drop(data);
    r.release_image(img.id);
    // View::fixed keeps log2(zoom): the scale the shader sees.
    let s = (scale.log2()).exp2() as f64;
    let ox = w as f64 * 0.5 - iw as f64 * 0.5 * s;
    let oy = h as f64 * 0.5 - ih as f64 * 0.5 * s;
    (w, h, out, (s, ox, oy))
}

#[test]
fn eight_bit_images_are_bit_exact_at_one_and_nearest_zooms() {
    let Some(gpu) = gpu() else { return };
    let mut r = ImageRenderer::new(&gpu, 32 << 20);
    let (iw, ih) = (256, 23);
    let raster = rgb8(iw, ih, chart);
    let mut bad = Vec::new();
    for (opts, what) in [
        (RenderOptions::default(), "settled (Lanczos)"),
        (
            RenderOptions {
                hq: false,
                ..Default::default()
            },
            "moving (bilinear)",
        ),
    ] {
        for s in [1.0f32, 4.0, 5.0, 8.0] {
            let (w, h, px, (sc, ox, oy)) = draw_out(&gpu, &mut r, &raster, s, &opts);
            let (mut wrong, mut worst) = (0usize, 0i32);
            for y in 0..h {
                for x in 0..w {
                    let sx = ((x as f64 + 0.5 - ox) / sc).floor() as u32;
                    let sy = ((y as f64 + 0.5 - oy) / sc).floor() as u32;
                    let want = chart(sx.min(iw - 1), sy.min(ih - 1));
                    let got = px[(y * w + x) as usize];
                    let d = (0..3)
                        .map(|c| (got[c] as i32 - want[c] as i32).abs())
                        .max()
                        .unwrap();
                    if d != 0 {
                        wrong += 1;
                        worst = worst.max(d);
                    }
                }
            }
            eprintln!(
                "8-bit output, {what} zoom {s}: {wrong} of {} pixels differ (worst {worst} codes)",
                w * h
            );
            if wrong > 0 {
                bad.push(format!("{what} zoom {s}: {wrong} pixels, worst {worst}"));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// 16-bit grey at 1:1: within one 8-bit code of the exact value, and close to unbiased: the
/// mean of a flat 16 × 16 patch stays within 0.25 code of it. What is left of a bias comes
/// from the half-float overview and composite (each truncates, up to 0.05 code), the
/// undithered zone around each code (up to 0.125) and the blue noise's mean over the patch.
#[test]
fn sixteen_bit_images_round_within_one_code_unbiased() {
    let Some(gpu) = gpu() else { return };
    let mut r = ImageRenderer::new(&gpu, 32 << 20);
    // 64 flat patches of 16 × 16, each at a value between 8-bit codes.
    let level = |x: u32, y: u32| (((y / 16) * 8 + x / 16) * 1021 + 85) as u16;
    let raster = gray16(128, 128, level);
    let (w, h, px, _) = draw_out(&gpu, &mut r, &raster, 1.0, &RenderOptions::default());
    let (mut worst, mut worst_bias) = (0f64, 0f64);
    for p in 0..64u32 {
        let (bx, by) = ((p % 8) * 16, (p / 8) * 16);
        let v = level(bx, by) as f64 / 65535.0;
        let want = v * 255.0; // grey 16-bit images are sRGB-encoded, like 8-bit ones
        let mut sum = 0f64;
        for y in by..by + 16 {
            for x in bx..bx + 16 {
                let got = px[(y * w + x) as usize][1] as f64;
                worst = worst.max((got - want).abs());
                sum += got;
            }
        }
        worst_bias = worst_bias.max((sum / 256.0 - want).abs());
    }
    assert_eq!((w, h), (128, 128));
    eprintln!("16-bit output at 1:1: worst {worst:.3} codes, worst patch bias {worst_bias:.3}");
    assert!(worst < 1.0, "worst {worst}");
    assert!(worst_bias < 0.25, "bias {worst_bias}");
}

/// Uploads every detail tile `view` wants, from the whole-image raster.
fn upload_all_tiles(
    gpu: &Gpu,
    r: &mut ImageRenderer,
    img: &mut GpuImage,
    raster: &Raster,
    view: &View,
) {
    let (level, tiles) = r.wanted_tiles(img, view).expect("tiles wanted");
    assert_eq!(level, 0);
    assert_eq!(r.upload_tiles(gpu, img, raster, 0, &tiles), tiles.len());
}

/// The stored codes of 16-bit tile `slot` (a layer of the 16-bit array), RGBA per texel.
fn read_tile16(gpu: &Gpu, r: &ImageRenderer, slot: u32) -> Vec<[u16; 4]> {
    let t = r.tiles16.as_ref().expect("16-bit tiles");
    let layer = slot - r.pool.wide_from();
    let row = (SLOT * 8).div_ceil(256) * 256;
    let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("tile readback"),
        size: (row * SLOT) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &t.tex,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: 0,
                y: 0,
                z: layer,
            },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(SLOT),
            },
        },
        wgpu::Extent3d {
            width: SLOT,
            height: SLOT,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([enc.finish()]);
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    let data = buf.slice(..).get_mapped_range().expect("mapped");
    let out = data
        .chunks_exact(row as usize)
        .flat_map(|r| r[..SLOT as usize * 8].chunks_exact(8))
        .map(|c| std::array::from_fn(|i| u16::from_le_bytes([c[2 * i], c[2 * i + 1]])))
        .collect();
    drop(data);
    out
}

/// 16-bit detail tiles hold the source's 16-bit codes exactly: an sRGB-coded RGB image and a
/// grey one, each larger than one tile, read back texel by texel (borders included).
#[test]
fn sixteen_bit_tiles_store_the_source_codes_exactly() {
    let Some(gpu) = gpu() else { return };
    let mut r = ImageRenderer::new(&gpu, 32 << 20);
    let grey = |x: u32, y: u32| (x * 131 + y * 977) as u16;
    for (raster, what) in [
        (rgb16(700, 530, noise16), "RGB"),
        (gray16(600, 520, grey), "grey"),
    ] {
        let (iw, ih) = (raster.width, raster.height);
        let mut img = r.create_image(&gpu, 44, iw, ih, None);
        let view = View::fixed(
            (iw, ih),
            Orientation::NORMAL,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: iw as f32,
                h: ih as f32,
            },
            1.0,
            (iw as f32 * 0.5, ih as f32 * 0.5),
        );
        upload_all_tiles(&gpu, &mut r, &mut img, &raster, &view);
        assert!(img.wide_tiles);
        let tiles: Vec<_> = r.pool.tiles_of(img.id).collect();
        assert_eq!(tiles.len(), 4);
        let (mut n, mut wrong, mut worst) = (0u64, 0u64, 0i32);
        for (_, tx, ty, slot) in tiles {
            assert!(slot >= r.pool.wide_from());
            let texels = read_tile16(&gpu, &r, slot);
            for sy in 0..SLOT {
                for sx in 0..SLOT {
                    let x = (tx * tiles::TILE + sx) as i64 - tiles::BORDER as i64;
                    let y = (ty * tiles::TILE + sy) as i64 - tiles::BORDER as i64;
                    if x < 0 || y < 0 || x >= iw as i64 || y >= ih as i64 {
                        continue;
                    }
                    let want = match what {
                        "RGB" => noise16(x as u32, y as u32),
                        _ => [grey(x as u32, y as u32); 3],
                    };
                    let got = texels[(sy * SLOT + sx) as usize];
                    n += 1;
                    let d = (0..3)
                        .map(|c| (got[c] as i32 - want[c] as i32).abs())
                        .max()
                        .unwrap();
                    if d != 0 || got[3] != u16::MAX {
                        wrong += 1;
                        worst = worst.max(d);
                    }
                }
            }
        }
        eprintln!("16-bit tiles, {what}: {wrong} of {n} texels differ (worst {worst} codes)");
        assert_eq!(wrong, 0, "{what}: worst {worst}");
        r.release_image(img.id);
    }
}

/// 16-bit images drawn from their detail tiles: grey at 1:1 within one 8-bit code and close
/// to unbiased, as from the overview; 8-bit values held in 16 bits (× 257) bit-exact at 1:1
/// and the nearest-neighbour zooms, like an 8-bit image.
#[test]
fn sixteen_bit_tiles_draw_within_one_code_and_8_bit_values_exactly() {
    let Some(gpu) = gpu() else { return };
    let mut r = ImageRenderer::new(&gpu, 32 << 20);
    let level = |x: u32, y: u32| (((y / 16) * 8 + x / 16) * 1021 + 85) as u16;
    let raster = gray16(128, 128, level);
    let opts = RenderOptions::default();
    let (w, _, px, _) = draw_from(&gpu, &mut r, &raster, 1.0, &opts, Source::Tiles);
    let (mut worst, mut worst_bias) = (0f64, 0f64);
    for p in 0..64u32 {
        let (bx, by) = ((p % 8) * 16, (p / 8) * 16);
        let want = level(bx, by) as f64 / 65535.0 * 255.0;
        let mut sum = 0f64;
        for y in by..by + 16 {
            for x in bx..bx + 16 {
                let got = px[(y * w + x) as usize][1] as f64;
                worst = worst.max((got - want).abs());
                sum += got;
            }
        }
        worst_bias = worst_bias.max((sum / 256.0 - want).abs());
    }
    eprintln!("16-bit tiles at 1:1: worst {worst:.3} codes, worst patch bias {worst_bias:.3}");
    assert!(worst < 1.0, "worst {worst}");
    assert!(worst_bias < 0.25, "bias {worst_bias}");

    let (iw, ih) = (256, 23);
    let wide = rgb16(iw, ih, |x, y| chart(x, y).map(|v| v as u16 * 257));
    let mut bad = Vec::new();
    for s in [1.0f32, 4.0, 5.0, 8.0] {
        let (w, h, px, (sc, ox, oy)) = draw_from(&gpu, &mut r, &wide, s, &opts, Source::Tiles);
        let mut wrong = 0usize;
        for y in 0..h {
            for x in 0..w {
                let sx = ((x as f64 + 0.5 - ox) / sc).floor() as u32;
                let sy = ((y as f64 + 0.5 - oy) / sc).floor() as u32;
                let want = chart(sx.min(iw - 1), sy.min(ih - 1));
                let got = px[(y * w + x) as usize];
                wrong += usize::from((0..3).any(|c| got[c] != want[c]));
            }
        }
        eprintln!(
            "8-bit values in 16-bit tiles, zoom {s}: {wrong} of {} pixels differ",
            w * h
        );
        if wrong > 0 {
            bad.push(format!("zoom {s}: {wrong} pixels"));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// The first image with wide samples splits the tile budget evenly between the 8-bit and the
/// 16-bit array: tile VRAM stays within the budget.
#[test]
fn the_16_bit_tile_array_shares_the_tile_budget() {
    let Some(gpu) = gpu() else { return };
    let budget = 256u64 << 20;
    let mut r = ImageRenderer::new(&gpu, budget);
    let before = r.tile_bytes();
    assert!(before <= budget);
    let raster = gray16(64, 64, |x, y| (x * 1000 + y) as u16);
    let _ = draw_from(
        &gpu,
        &mut r,
        &raster,
        1.0,
        &RenderOptions::default(),
        Source::Tiles,
    );
    let (narrow, wide) = (r.pool.part_capacity(false), r.pool.part_capacity(true));
    eprintln!(
        "tile arrays: {before} bytes before, {} after ({narrow} 8-bit, {wide} 16-bit layers)",
        r.tile_bytes()
    );
    assert!(wide > 0 && narrow > 0);
    assert!(r.tile_bytes() <= budget, "{} > {budget}", r.tile_bytes());
}

/// GPU time of drawing from 8-bit detail tiles (every tap decoded from its sRGB code in the
/// shader): a 2048² image from its tiles into a 1600 × 1000 half-float target, ten draws per
/// submission, median of 30. A measurement, not a check:
/// `cargo test -p specular-render bench_tile_decode -- --ignored --nocapture`.
#[test]
#[ignore = "benchmark"]
fn bench_tile_decode() {
    use specular_gfx::HDR;
    let Some(gpu) = gpu() else { return };
    let mut r = ImageRenderer::new(&gpu, 256 << 20);
    let (iw, ih) = (2048u32, 2048u32);
    let raster = rgb8(iw, ih, chart);
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
    let mut img = r.create_image(&gpu, 45, iw, ih, None);
    for (what, scale, hq) in [
        ("1:1 Lanczos", 1.0f32, true),
        ("0.8x Lanczos", 0.8, true),
        ("2.5x Lanczos", 2.5, true),
        ("1:1 bilinear", 1.0, false),
    ] {
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
        if let Some((level, tiles)) = r.wanted_tiles(&img, &view) {
            r.upload_tiles(&gpu, &mut img, &raster, level, &tiles);
        }
        let opts = RenderOptions {
            hq,
            ..Default::default()
        };
        let mut times = Vec::new();
        for k in 0..33 {
            let t = std::time::Instant::now();
            let mut enc = gpu.device.create_command_encoder(&Default::default());
            for _ in 0..10 {
                r.render(&gpu, &mut enc, &tv, (w, h), Some(&img), &view, &opts);
            }
            gpu.queue.submit([enc.finish()]);
            let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
            if k >= 3 {
                times.push(t.elapsed().as_secs_f64() * 1e3 / 10.0);
            }
        }
        times.sort_by(f64::total_cmp);
        eprintln!(
            "BENCH tile decode, {what}: median {:.3} ms, p10 {:.3} ms per draw ({adapter})",
            times[times.len() / 2],
            times[times.len() / 10]
        );
    }
}

/// The loupe's full-resolution tiles must not redraw the view at fit: the scene's key
/// ([`GpuImage::revision_from`], [`ImageRenderer::tiles_from`] from the view's level) only
/// changes with uploads to the levels that view reads.
#[test]
fn finer_tiles_leave_a_coarser_view_unchanged() {
    let Some(gpu) = gpu() else { return };
    let mut r = ImageRenderer::new(&gpu, 64 << 20);
    let (iw, ih) = (4096, 4096);
    let mut img = r.create_image(&gpu, 46, iw, ih, None);
    let fit = View::fixed(
        (iw, ih),
        Orientation::NORMAL,
        PxRect {
            x: 0.0,
            y: 0.0,
            w: 1024.0,
            h: 1024.0,
        },
        0.25,
        (iw as f32 * 0.5, ih as f32 * 0.5),
    );
    let level = ImageRenderer::view_level(&img, &fit);
    assert_eq!(level, 2);
    let from = level - 1;
    let key =
        |r: &ImageRenderer, img: &GpuImage| (img.revision_from(from), r.tiles_from(img, from));
    // The view's own level: the key changes.
    let k0 = key(&r, &img);
    let coarse = rgb8(1024, 1024, |x, y| [x as u8, y as u8, 7]);
    assert_eq!(r.upload_tiles(&gpu, &mut img, &coarse, 2, &[(0, 0)]), 1);
    let k1 = key(&r, &img);
    assert_ne!(k0, k1);
    // Full-resolution tiles (the loupe's): it does not.
    let full = rgb8(1040, 1040, |x, y| [y as u8, x as u8, 9]);
    assert_eq!(
        r.upload_tiles(&gpu, &mut img, &full, 0, &[(0, 0), (1, 0), (0, 1), (1, 1)]),
        4
    );
    assert_eq!(key(&r, &img), k1);
    // Everything else still sees the new pixels.
    assert!(img.revision_from(0) > k1.0);
    assert_eq!(r.tiles_from(&img, 0), 5);
}
