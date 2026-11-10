//! Tagged sources: Display P3 and Adobe RGB values come out as the same colours in
//! linear Rec.2020 (outside sRGB where they are), untagged ones through the sRGB curve,
//! transparency straight, and CMYK files named.

use super::*;
use crate::colour::{mulv, rec2020_to_rec709};
use crate::export::{Space, Transfer};
use specular_core::RasterPlane;

/// An 8-bit RGBA raster of `px` (one row).
fn rgba8(px: &[[u8; 4]]) -> Raster {
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
            data: px.iter().flatten().copied().collect(),
            h_div: 1,
            v_div: 1,
        }],
        truncated: false,
    }
}

const SAMPLES: [[u8; 4]; 6] = [
    [255, 0, 0, 255],
    [0, 255, 0, 255],
    [0, 0, 255, 255],
    [128, 64, 200, 255],
    [255, 255, 255, 255],
    [30, 200, 90, 128],
];

/// The input of `px` tagged with `space`'s profile, against the space's own matrix and
/// curve: the worst difference relative to the value (at least 0.05), and how many values
/// differ by more than `tol`.
fn compare(space: Space, tol: f64) -> (f64, usize) {
    let icc = space.icc(false);
    let r = rgba8(&SAMPLES);
    let Some(Input::Rgb { rgba, .. }) = input(&r, Some(&icc), None, &CancelToken::new()) else {
        panic!("no input");
    };
    let m = space.to_rec2020();
    let t = space.transfer();
    let (mut worst, mut off) = (0f64, 0);
    for (p, got) in SAMPLES.iter().zip(rgba.chunks_exact(4)) {
        let enc = [p[0], p[1], p[2]].map(|v| t.decode(f64::from(v) / 255.0));
        let want = mulv(&m, enc);
        for c in 0..3 {
            let d = (f64::from(got[c]) - want[c]).abs() / want[c].abs().max(0.05);
            worst = worst.max(d);
            off += usize::from(d > tol);
        }
        // Straight alpha.
        assert!((f64::from(got[3]) - f64::from(p[3]) / 255.0).abs() < 1e-6);
    }
    (worst, off)
}

#[test]
fn display_p3_sources_keep_their_colours() {
    let (worst, off) = compare(Space::DisplayP3, 5e-3);
    assert_eq!(off, 0, "worst {worst:.2e}");
    // P3 red is outside sRGB: negative green and blue in BT.709 terms.
    let icc = Space::DisplayP3.icc(false);
    let r = rgba8(&[[255, 0, 0, 255]]);
    let Some(Input::Rgb { rgba, .. }) = input(&r, Some(&icc), None, &CancelToken::new()) else {
        panic!("no input");
    };
    let v = mulv(
        &rec2020_to_rec709(),
        [rgba[0], rgba[1], rgba[2]].map(f64::from),
    );
    assert!(v[1] < -0.03 && v[2] < -0.01, "{v:?}");
}

#[test]
fn adobe_rgb_sources_keep_their_colours() {
    let (worst, off) = compare(Space::AdobeRgb, 5e-3);
    assert_eq!(off, 0, "worst {worst:.2e}");
}

#[test]
fn untagged_sources_are_srgb() {
    let r = rgba8(&SAMPLES);
    let Some(Input::Rgb { rgba, .. }) = input(&r, None, None, &CancelToken::new()) else {
        panic!("no input");
    };
    let m = Space::Srgb.to_rec2020();
    for (p, got) in SAMPLES.iter().zip(rgba.chunks_exact(4)) {
        let enc = [p[0], p[1], p[2]].map(|v| Transfer::Srgb.decode(f64::from(v) / 255.0));
        let want = mulv(&m, enc);
        for c in 0..3 {
            assert!(
                (f64::from(got[c]) - want[c]).abs() < 2e-3 * want[c].abs().max(0.05),
                "{p:?}: {got:?} vs {want:?}"
            );
        }
    }
}

#[test]
fn reduction_averages_coverage_weighted_colour() {
    // A transparent black pixel beside an opaque red one: the average is half-covered red,
    // not dark red.
    let r = rgba8(&[[0, 0, 0, 0], [255, 0, 0, 255]]);
    let Some(Input::Rgb { width, rgba, .. }) = input(&r, None, Some(1), &CancelToken::new()) else {
        panic!("no input");
    };
    assert_eq!(width, 1);
    assert!((rgba[3] - 0.5).abs() < 1e-6);
    let red = mulv(&Space::Srgb.to_rec2020(), [1.0, 0.0, 0.0]);
    for c in 0..3 {
        assert!((f64::from(rgba[c]) - red[c]).abs() < 2e-3, "{rgba:?}");
    }
}

#[test]
fn cmyk_files_are_named() {
    assert_eq!(Note::of_colour_model(Some("CMYK")), Some(Note::Cmyk));
    assert_eq!(Note::of_colour_model(Some("YCCK")), Some(Note::Cmyk));
    assert_eq!(Note::of_colour_model(Some("RGB")), None);
    assert_eq!(Note::of_colour_model(None), None);
}

/// A CMYK JPEG reads through the registry as RGB (its decoder converts it), and the export
/// reader takes it.
#[test]
fn a_cmyk_jpeg_is_converted_to_rgb() {
    use jpeg_encoder::{ColorType, Encoder};
    let (w, h) = (16u16, 8u16);
    // Ink: cyan left, magenta right, no black.
    let cmyk: Vec<u8> = (0..h)
        .flat_map(|_| {
            (0..w).flat_map(move |x| {
                if x < 8 {
                    [200, 0, 0, 0]
                } else {
                    [0, 200, 0, 0]
                }
            })
        })
        .collect();
    let mut jpg = Vec::new();
    Encoder::new(&mut jpg, 95)
        .encode(&cmyk, w, h, ColorType::Cmyk)
        .unwrap();
    let dir = std::env::temp_dir().join(format!("specular-develop-cmyk-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("cmyk.jpg");
    std::fs::write(&p, &jpg).unwrap();
    let reg = specular_decode::Registry::new();
    let s = reg.open_path(&p).unwrap();
    assert_eq!(
        Note::of_source(s.colour_model(), None, &jpg),
        Some(Note::Cmyk)
    );
    // An RGB JPEG is not.
    let mut rgb = Vec::new();
    Encoder::new(&mut rgb, 90)
        .encode(&[10u8; 8 * 8 * 3], 8, 8, ColorType::Rgb)
        .unwrap();
    assert_eq!(Note::of_source(None, None, &rgb), None);
    let read = crate::export::batch::read_input(&p, &reg).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    let Input::Rgb { width, rgba, .. } = read.input else {
        panic!("not RGB");
    };
    assert_eq!(width, u32::from(w));
    // Cyan ink: little red; magenta ink: little green.
    let px = |x: usize| &rgba[x * 4..x * 4 + 4];
    let (c, m) = (px(2), px(13));
    assert!(c[0] < c[1] && c[0] < c[2], "cyan {c:?}");
    assert!(m[1] < m[0] && m[1] < m[2], "magenta {m:?}");
}
