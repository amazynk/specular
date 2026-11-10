//! A movie's picture adjustments in the image shader against `PictureAdjust::apply`, the
//! CPU reference. GPU tests skip without an adapter.

use specular_core::{PixelLayout, Raster, RasterPlane};
use specular_inspect::{srgb_decode, srgb_encode};

use super::hdr_tests::{draw_with, gpu};
use super::*;
use crate::PictureAdjust;

fn row(px: &[[u8; 3]]) -> Raster {
    let w = px.len() as u32;
    Raster {
        width: w,
        height: 1,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Rgba,
        planes: vec![RasterPlane {
            width: w,
            height: 1,
            stride: w as usize * 4,
            data: px.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
            h_div: 1,
            v_div: 1,
        }],
        truncated: false,
    }
}

/// The sRGB EOTF without the clip at white (adjusted colours may go above it).
fn decode(v: f32) -> f32 {
    if v <= 0.040_45 {
        v.max(0.0) / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

#[test]
fn picture_adjustments_match_the_cpu_reference() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let px = [
        [0, 0, 0],
        [255, 255, 255],
        [128, 128, 128],
        [200, 40, 60],
        [30, 180, 90],
        [20, 60, 220],
        [250, 230, 10],
        [90, 70, 50],
    ];
    let r = row(&px);
    let mut img = renderer.create_image(&gpu, 21, r.width, 1, None);
    renderer.upload_overview(&gpu, &mut img, &r);
    let n = PictureAdjust::NEUTRAL;
    let mut mismatches = 0;
    for adj in [
        n,
        PictureAdjust {
            brightness: 0.15,
            ..n
        },
        PictureAdjust { contrast: 1.4, ..n },
        PictureAdjust {
            contrast: 0.6,
            brightness: -0.1,
            ..n
        },
        PictureAdjust {
            saturation: 0.0,
            ..n
        },
        PictureAdjust {
            saturation: 1.6,
            ..n
        },
        PictureAdjust { gamma: 1.8, ..n },
        PictureAdjust { gamma: 0.6, ..n },
        PictureAdjust { hue: 90.0, ..n },
        PictureAdjust {
            brightness: 0.05,
            contrast: 1.2,
            saturation: 0.8,
            gamma: 1.1,
            hue: -45.0,
        },
    ] {
        let out = draw_with(
            &gpu,
            &mut renderer,
            &img,
            RenderOptions {
                picture: Some(adj),
                ..RenderOptions::default()
            },
        );
        for (i, p) in px.iter().enumerate() {
            let enc = p.map(|v| srgb_encode(srgb_decode(f32::from(v) / 255.0)));
            let want = adj.apply(enc);
            for ch in 0..3 {
                let lin = decode(want[ch]);
                let got = out[i][ch];
                if (got - lin).abs() > 2e-3 + lin * 4e-3 {
                    mismatches += 1;
                    eprintln!("{adj:?} pixel {p:?} channel {ch}: {got} vs {lin}");
                }
            }
            assert_eq!(out[i][3], 1.0);
        }
    }
    assert_eq!(mismatches, 0);
}
