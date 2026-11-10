//! Colour-vision-safe display aids in the image shader (DESIGN.md §6.2): false colour through
//! the GPU colour-map table, channel-colour presets, HiLo and auto-range, each against the CPU
//! reference in `specular-inspect`. GPU tests skip without an adapter.

use specular_core::{PixelLayout, Raster, RasterPlane};
use specular_inspect::{BitRange, ChannelColours, ColourMap, srgb_decode, srgb_encode};

use super::hdr_tests::{draw_with, gpu};
use super::*;

fn raster(w: u32, layout: PixelLayout, bpp: usize, data: Vec<u8>) -> Raster {
    Raster {
        width: w,
        height: 1,
        origin: (0, 0),
        scale: 1,
        layout,
        planes: vec![RasterPlane {
            width: w,
            height: 1,
            stride: w as usize * bpp,
            data,
            h_div: 1,
            v_div: 1,
        }],
        truncated: false,
    }
}

fn gray16(codes: &[u16]) -> Raster {
    let data = codes.iter().flat_map(|c| c.to_le_bytes()).collect();
    raster(codes.len() as u32, PixelLayout::Gray16, 2, data)
}

/// A 12-bit sample widened the way the TIFF decoder does it.
fn widen12(v: u32) -> u16 {
    (v * 65535 / 4095) as u16
}

const HILO: HiLo = HiLo {
    high: [1.0, 0.0, 0.0],
    low: [0.0, 0.0, 1.0],
};

fn near(a: [f32; 4], b: [f32; 3], tol: f32) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() <= tol)
}

#[test]
fn false_colour_follows_the_map_table() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let greys: Vec<u8> = vec![0, 32, 64, 128, 192, 255];
    let w = greys.len() as u32;
    let data = greys.iter().flat_map(|&g| [g, g, g, 255]).collect();
    let r = raster(w, PixelLayout::Rgba, 4, data);
    let mut img = renderer.create_image(&gpu, 1, w, 1, None);
    renderer.upload_overview(&gpu, &mut img, &r);
    let mut worst = 0f32;
    for map in ColourMap::ALL {
        let out = draw_with(
            &gpu,
            &mut renderer,
            &img,
            RenderOptions {
                false_colour: Some(map),
                ..RenderOptions::default()
            },
        );
        for (i, &g) in greys.iter().enumerate() {
            let want = map.sample(g as f32 / 255.0).map(srgb_decode);
            for c in 0..3 {
                // Compared encoded: 8-bit storage and the half-float target.
                worst = worst.max((srgb_encode(out[i][c]) - srgb_encode(want[c])).abs());
            }
        }
    }
    eprintln!("false colour: max encoded |GPU − CPU| {worst:.5}");
    assert!(worst < 2.5 / 255.0, "{worst}");
}

#[test]
fn channel_colour_presets_recolour_the_channels() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let px: [[u8; 3]; 4] = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [200, 100, 50]];
    let data = px.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect();
    let r = raster(4, PixelLayout::Rgba, 4, data);
    let mut img = renderer.create_image(&gpu, 2, 4, 1, None);
    renderer.upload_overview(&gpu, &mut img, &r);
    for preset in ChannelColours::ALL {
        let out = draw_with(
            &gpu,
            &mut renderer,
            &img,
            RenderOptions {
                channel_colours: preset,
                ..RenderOptions::default()
            },
        );
        for (i, p) in px.iter().enumerate() {
            let want = preset.apply(p.map(|v| srgb_decode(v as f32 / 255.0)));
            assert!(
                near(out[i], want, 0.01),
                "{preset:?} {i}: {:?} {want:?}",
                out[i]
            );
        }
    }
    // Red-green-safe: the first two channels differ in more than red against green.
    let m = ChannelColours::MagentaGreen.colours();
    assert!(m[0][2] > 0.5 && m[1][2] == 0.0);
}

#[test]
fn hilo_marks_8bit_limits() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let greys: [u8; 5] = [0, 1, 128, 254, 255];
    let data = greys.iter().flat_map(|&g| [g, g, g, 255]).collect();
    let r = raster(5, PixelLayout::Rgba, 4, data);
    let mut img = renderer.create_image(&gpu, 3, 5, 1, None);
    renderer.upload_overview(&gpu, &mut img, &r);
    let out = draw_with(
        &gpu,
        &mut renderer,
        &img,
        RenderOptions {
            hilo: Some(HILO),
            ..RenderOptions::default()
        },
    );
    assert!(near(out[0], HILO.low, 0.01), "{:?}", out[0]);
    assert!(near(out[4], HILO.high, 0.01), "{:?}", out[4]);
    for (i, o) in out.iter().enumerate().take(4).skip(1) {
        assert!(o[0] == o[1] && o[1] == o[2], "{i}: {o:?}");
    }
}

#[test]
fn auto_range_stretches_high_bit_grey_and_keeps_hilo_exact() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let bits = BitRange::new(12, false);
    // A 12-bit scan: the extremes of the bit depth, their neighbours and a mid-range ramp.
    let v12 = [0u32, 1, 100, 1000, 2000, 4094, 4095];
    let codes: Vec<u16> = v12.iter().map(|&v| widen12(v)).collect();
    for &c in &codes {
        assert_eq!(bits.is_high16(c), c == widen12(4095));
        assert_eq!(bits.is_low16(c), c == 0);
    }
    let r = gray16(&codes);
    let w = codes.len() as u32;
    let mut img = renderer.create_image(&gpu, 4, w, 1, None);
    renderer.set_auto_range(&gpu, &mut img, Some(12));
    renderer.upload_overview(&gpu, &mut img, &r);
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    assert_eq!(img.auto_range(), Some([0, 65535]));
    let out = draw_with(
        &gpu,
        &mut renderer,
        &img,
        RenderOptions {
            hilo: Some(HILO),
            ..RenderOptions::default()
        },
    );
    assert!(near(out[0], HILO.low, 0.01), "min: {:?}", out[0]);
    assert!(near(out[6], HILO.high, 0.01), "max: {:?}", out[6]);
    for (i, o) in out.iter().enumerate().take(6).skip(1) {
        assert!(o[0] == o[1] && o[1] == o[2], "{i} marked: {o:?}");
    }

    // Without the bit-depth extremes the range is the data's own: 100 ‥ 2000 fills the
    // display (within 1/255 ‥ 254/255).
    let codes: Vec<u16> = [100u32, 1000, 2000].iter().map(|&v| widen12(v)).collect();
    let r = gray16(&codes);
    let mut img = renderer.create_image(&gpu, 5, 3, 1, None);
    renderer.set_auto_range(&gpu, &mut img, Some(12));
    renderer.upload_overview(&gpu, &mut img, &r);
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    assert_eq!(img.auto_range(), Some([codes[0], codes[2]]));
    let out = draw_with(&gpu, &mut renderer, &img, RenderOptions::default());
    let enc = |p: [f32; 4]| srgb_encode(p[0]);
    let mid = (codes[1] - codes[0]) as f32 / (codes[2] - codes[0]) as f32;
    eprintln!(
        "auto-range: {:?}",
        out.iter().map(|&p| enc(p)).collect::<Vec<_>>()
    );
    assert!((enc(out[0]) - 1.0 / 255.0).abs() < 1e-3, "{}", enc(out[0]));
    assert!(
        (enc(out[2]) - 254.0 / 255.0).abs() < 1e-3,
        "{}",
        enc(out[2])
    );
    assert!((enc(out[1]) - mid).abs() < 2e-3, "{} vs {mid}", enc(out[1]));

    // Off: the plain codes.
    let mut img = renderer.create_image(&gpu, 6, 3, 1, None);
    renderer.upload_overview(&gpu, &mut img, &r);
    assert_eq!(img.auto_range(), None);
    let out = draw_with(&gpu, &mut renderer, &img, RenderOptions::default());
    assert!((enc(out[2]) - codes[2] as f32 / 65535.0).abs() < 2e-3);
}
