//! Stain separation in the image shader against `specular_pathology::stain::render`, the
//! CPU reference, on analytic stain mixtures and arbitrary colours. GPU tests skip without
//! an adapter.

use specular_core::{PixelLayout, Raster, RasterPlane};
use specular_inspect::srgb_decode;
use specular_pathology::stain::{self, Mode, StainSet, Stains};

use super::hdr_tests::{draw_with, gpu};
use super::*;

/// The view the app builds from a [`stain::Display`].
pub(crate) fn view(d: &stain::Display, visible: [bool; 3], mode: Mode) -> StainView {
    let f = |v: f64| v as f32;
    let tint = stain::tints(d, visible);
    StainView {
        // Shader rows are the unmixing matrix's columns.
        unmix: [0, 1, 2].map(|s| [0, 1, 2].map(|k| f(d.unmix[k][s]))),
        recon: d.recon.map(|r| r.map(f)),
        tint: tint.map(|r| r.map(f)),
        visible,
        od: mode == Mode::Od,
        od_max: stain::OD_MAX as f32,
    }
}

/// Pixels: pure and mixed stains at several concentrations, white, black and odd colours.
fn pixels(s: &Stains) -> Vec<[u8; 3]> {
    let mut v = vec![[255, 255, 255], [0, 0, 0], [30, 200, 90], [250, 20, 240]];
    for c in [
        [0.6, 0.0, 0.0],
        [0.0, 0.5, 0.0],
        [0.0, 0.0, 0.4],
        [0.3, 0.4, 0.2],
        [1.2, 0.2, 0.0],
    ] {
        let od = stain::vec_mat(c, &s.vectors);
        v.push(od.map(|x| (10f64.powf(-x) * 255.0).round().clamp(0.0, 255.0) as u8));
    }
    v
}

#[test]
fn stain_views_match_the_cpu_reference() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    for set in StainSet::ALL {
        let s = Stains::published(set);
        let px = pixels(&s);
        let w = px.len() as u32;
        let data: Vec<u8> = px.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect();
        let r = Raster {
            width: w,
            height: 1,
            origin: (0, 0),
            scale: 1,
            layout: PixelLayout::Rgba,
            planes: vec![RasterPlane {
                width: w,
                height: 1,
                stride: w as usize * 4,
                data,
                h_div: 1,
                v_div: 1,
            }],
            truncated: false,
        };
        let mut img = renderer.create_image(&gpu, 9, w, 1, None);
        renderer.upload_overview(&gpu, &mut img, &r);
        let d = stain::display(&s, None).unwrap();
        for (visible, mode) in [
            ([true, false, false], Mode::Intensity),
            ([false, true, false], Mode::Intensity),
            ([true, true, true], Mode::Intensity),
            ([false, false, true], Mode::Od),
            ([true, true, false], Mode::Od),
        ] {
            let out = draw_with(
                &gpu,
                &mut renderer,
                &img,
                RenderOptions {
                    stain: Some(view(&d, visible, mode)),
                    ..RenderOptions::default()
                },
            );
            for (i, p) in px.iter().enumerate() {
                let want = stain::render(p.map(|v| f64::from(v) / 255.0), &d, visible, mode);
                for ch in 0..3 {
                    let lin = srgb_decode(want[ch] as f32);
                    let got = out[i][ch];
                    assert!(
                        (got - lin).abs() <= 2e-3 + lin * 4e-3,
                        "{set:?} {visible:?} {mode:?} pixel {p:?} channel {ch}: {got} vs {lin}"
                    );
                }
                assert_eq!(out[i][3], 1.0);
            }
        }
    }
}

#[test]
fn every_stain_shown_as_light_is_the_image() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let s = Stains::published(StainSet::HeDab);
    let px = pixels(&s);
    // Leave out the colours no H&E-DAB mix makes (negative concentrations are clamped).
    let px: Vec<[u8; 3]> = px
        .into_iter()
        .filter(|p| {
            stain::concentrations(p.map(|v| f64::from(v) / 255.0), &s.unmix().unwrap())
                .iter()
                .all(|c| *c >= -1e-9)
        })
        .collect();
    let w = px.len() as u32;
    let data: Vec<u8> = px.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect();
    let r = Raster {
        width: w,
        height: 1,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Rgba,
        planes: vec![RasterPlane {
            width: w,
            height: 1,
            stride: w as usize * 4,
            data,
            h_div: 1,
            v_div: 1,
        }],
        truncated: false,
    };
    let mut img = renderer.create_image(&gpu, 10, w, 1, None);
    renderer.upload_overview(&gpu, &mut img, &r);
    let d = stain::display(&s, None).unwrap();
    let out = draw_with(
        &gpu,
        &mut renderer,
        &img,
        RenderOptions {
            stain: Some(view(&d, [true; 3], Mode::Intensity)),
            ..RenderOptions::default()
        },
    );
    for (i, p) in px.iter().enumerate() {
        for ch in 0..3 {
            let lin = srgb_decode(f32::from(p[ch]) / 255.0);
            assert!((out[i][ch] - lin).abs() <= 2e-3 + lin * 4e-3, "{p:?} {ch}");
        }
    }
}
