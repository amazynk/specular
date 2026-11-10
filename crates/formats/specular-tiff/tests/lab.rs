//! L*a*b* TIFF (CIELab, ICCLab, ITULab) through the hook with specular-color's
//! `LabConverter`, as specular-decode installs it, against lcms2 (test oracle only).
//! tests/lab_fallback.rs covers the built-in formula without a hook.

mod common;
#[path = "common/lab.rs"]
mod lab;

use common::{Comp, Spec, write};
use lab::{Alpha, CIELAB, Case, ICCLAB, ITULAB, diff, make, matrix};
use specular_tiff::{
    DecodeOptions, Decoded, LabRowConverter, LabSamples, Region, Tiff, downscale, set_lab_hook,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

static CALLS: AtomicUsize = AtomicUsize::new(0);

fn hook() -> Option<LabRowConverter> {
    static C: OnceLock<Arc<specular_color::LabConverter>> = OnceLock::new();
    let c = C.get_or_init(Default::default).clone();
    Some(Arc::new(move |s: &LabSamples, rgba: &mut [u8]| {
        CALLS.fetch_add(1, Ordering::Relaxed);
        let k = &s.coding;
        let coding = specular_color::LabCoding {
            wide: k.wide,
            big_endian: k.big_endian,
            signed: k.signed,
            scale: k.scale,
            offset: k.offset,
        };
        c.convert_samples(s.channels, s.step, &coding, rgba)
    }))
}

fn decode(f: &[u8], scale: u32, region: Option<Region>) -> Decoded {
    set_lab_hook(Some(hook));
    let t = Tiff::open(f).expect("open");
    t.pages()[0].check_supported().expect("supported");
    t.decode(
        f,
        0,
        &DecodeOptions {
            scale,
            region,
            ..Default::default()
        },
    )
    .expect("decode")
}

#[test]
fn matrix_matches_lcms() {
    let cases = matrix();
    assert!(cases.len() > 200);
    let before = CALLS.load(Ordering::Relaxed);
    for (i, case) in cases.iter().enumerate() {
        let (f, want) = make(case, 37, 19, i as u32);
        let d = decode(&f, 1, None);
        assert!(!d.truncated, "{case:?}");
        let (dmax, alpha) = diff(&d.data, &want);
        assert!(
            dmax <= 1 && alpha,
            "{case:?}: max diff {dmax}, alpha ok {alpha}"
        );
    }
    assert!(CALLS.load(Ordering::Relaxed) > before, "the hook was used");
}

#[test]
fn itulab_decode_tag_and_tiles() {
    for wide in [false, true] {
        for decode_tag in [None, Some([0.0, 100.0, -100.0, 100.0, -50.5, 120.25])] {
            for planar in [false, true] {
                let case = Case {
                    decode: decode_tag,
                    planar,
                    tile: Some((16, 16)),
                    comp: Comp::Deflate,
                    alpha: Alpha::Unassociated,
                    ..Case::new(ITULAB, wide)
                };
                let (f, want) = make(&case, 41, 23, 7);
                let (dmax, alpha) = diff(&decode(&f, 1, None).data, &want);
                assert!(dmax <= 1 && alpha, "{case:?}: {dmax}");
            }
        }
    }
}

#[test]
fn bad_decode_tag_is_rejected() {
    let case = Case {
        decode: Some([0.0, 100.0, -1e6, 1e6, 0.0, 1.0]),
        ..Case::new(ITULAB, false)
    };
    let (f, _) = make(&case, 8, 8, 1);
    let t = Tiff::open(&f).unwrap();
    assert!(t.pages()[0].check_supported().is_err());
}

/// L*a*b* of other depths (bit-packed or whole bytes, either byte order, chunky or planar,
/// with the horizontal predictor) and float L*a*b* (32/64-bit, with the floating-point
/// predictor), against lcms2 from the samples' values.
#[test]
fn other_depths_match_lcms() {
    let mut n = 0;
    for photo in [CIELAB, ICCLAB, ITULAB] {
        for bits in [1u16, 4, 7, 10, 12, 20, 24] {
            for (planar, alpha, comp, predictor, le) in [
                (false, Alpha::None, Comp::None, 1, true),
                (true, Alpha::Unassociated, Comp::Lzw, 1, false),
                (false, Alpha::Unassociated, Comp::Deflate, 2, true),
                (false, Alpha::None, Comp::PackBits, 2, false),
            ] {
                let case = Case {
                    planar,
                    alpha,
                    comp,
                    predictor,
                    le,
                    bits: Some(bits),
                    ..Case::new(photo, false)
                };
                let (f, want) = make(&case, 37, 19, n);
                n += 1;
                let d = decode(&f, 1, None);
                assert!(!d.truncated, "{case:?}");
                let (dmax, alpha) = diff(&d.data, &want);
                assert!(
                    dmax <= 1 && alpha,
                    "{case:?}: max diff {dmax}, alpha ok {alpha}"
                );
            }
        }
    }
    for photo in [CIELAB, ICCLAB] {
        for bits in [32u16, 64] {
            for (planar, alpha, comp, le) in [
                (false, Alpha::None, Comp::None, true),
                (true, Alpha::Unassociated, Comp::Deflate, false),
                (false, Alpha::Unassociated, Comp::Lzw, true),
            ] {
                let case = Case {
                    planar,
                    alpha,
                    comp,
                    le,
                    bits: Some(bits),
                    float: true,
                    ..Case::new(photo, false)
                };
                let (f, want) = make(&case, 37, 19, n);
                n += 1;
                let d = decode(&f, 1, None);
                assert!(!d.truncated, "{case:?}");
                let (dmax, alpha) = diff(&d.data, &want);
                assert!(
                    dmax <= 1 && alpha,
                    "{case:?}: max diff {dmax}, alpha ok {alpha}"
                );
            }
        }
    }
}

/// A reduced-scale decode of one large strip (the fused convert-and-reduce path), of tiles and
/// of small strips equals the box filter of the full decode, regions included.
#[test]
fn scaled_equals_downscaled() {
    let layouts = [(None, None), (Some(7), None), (None, Some((16, 32)))];
    for (k, (rows, tile)) in layouts.into_iter().enumerate() {
        for (photo, wide, planar, alpha) in [
            (CIELAB, false, false, Alpha::None),
            (ICCLAB, true, true, Alpha::Unassociated),
            (ITULAB, false, true, Alpha::Associated),
        ] {
            let case = Case {
                planar,
                alpha,
                rows,
                tile,
                comp: Comp::Lzw,
                ..Case::new(photo, wide)
            };
            let (f, _) = make(&case, 83, 61, k as u32);
            let full = decode(&f, 1, None);
            for scale in [2u32, 4, 8] {
                let d = decode(&f, scale, None);
                let ds = downscale(&full, scale);
                assert_eq!((d.width, d.height), (ds.width, ds.height));
                assert!(d.data == ds.data, "{case:?} scale {scale}");
                let r = Region {
                    x: 1,
                    y: 2,
                    w: d.width - 2,
                    h: d.height - 3,
                };
                let part = decode(&f, scale, Some(r));
                assert_eq!(part.data, crop(&d, r), "{case:?} scale {scale} region");
            }
        }
    }
}

fn crop(d: &Decoded, r: Region) -> Vec<u8> {
    let bpp = d.format.bytes_per_pixel();
    let mut v = Vec::new();
    for y in r.y..r.y + r.h {
        let s = y as usize * d.stride + r.x as usize * bpp;
        v.extend_from_slice(&d.data[s..s + r.w as usize * bpp]);
    }
    v
}

/// 8-bit L*a*b* in JPEG strips (the samples coded as the JPEG's three components).
#[test]
fn jpeg_compressed() {
    let (w, h) = (64u32, 40u32);
    for photo in [ICCLAB, ITULAB] {
        // Smooth samples (JPEG rings on edges).
        let mut src = Vec::new();
        for y in 0..h {
            for x in 0..w {
                src.extend_from_slice(&[(x * 2 + y) as u8 + 40, 100 + x as u8, 160 - y as u8]);
            }
        }
        let strips: Vec<Vec<u8>> = (0..h.div_ceil(16))
            .map(|i| {
                let rows = 16.min(h - i * 16);
                let a = (i * 16 * w * 3) as usize;
                let mut out = Vec::new();
                jpeg_encoder::Encoder::new(&mut out, 98)
                    .encode(
                        &src[a..a + (rows * w * 3) as usize],
                        w as u16,
                        rows as u16,
                        // Ycbcr: the samples are stored as given (no colour transform), as in
                        // TIFF's JPEG for non-YCbCr photometrics.
                        jpeg_encoder::ColorType::Ycbcr,
                    )
                    .unwrap();
                out
            })
            .collect();
        let mut s = Spec::new(w, h, 3, 8, photo, Vec::new());
        s.comp = Comp::Jpeg;
        s.rows_per_strip = 16;
        s.pre = Some(strips);
        let jf = write(&[s], false, true);
        let plain = write(&[Spec::new(w, h, 3, 8, photo, src.clone())], false, true);
        let (j, p) = (decode(&jf, 1, None), decode(&plain, 1, None));
        let (dmax, _) = diff(&j.data, &p.data);
        assert!(dmax < 12, "photometric {photo}: {dmax}");
        // A scaled decode is close to the box filter of the full one.
        let (j4, p4) = (decode(&jf, 4, None), downscale(&p, 4));
        let d4 = diff(&j4.data, &p4.data).0;
        assert!(d4 < 12, "photometric {photo} at 1/4: {d4}");
    }
}

fn fixture(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/lab")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// ImageMagick's L*a*b* TIFFs (tests/fixtures/lab/make.sh) decode back to the RGB they were
/// made from. ImageMagick computes L*a*b* relative to D65 by plain XYZ scaling, the decoder
/// takes it as D50 (ICC, Bradford), so saturated colours are a few codes apart.
#[test]
fn imagemagick_fixtures() {
    let rgb = decode(&fixture("rose-rgb.tif"), 1, None);
    let mut first = None;
    let rgb4 = decode(&fixture("rose-lab4-rgb.tif"), 1, None);
    for (name, tol, mean_max) in [
        ("rose-lab8.tif", 16, 2.5),
        ("rose-lab16.tif", 16, 2.5),
        ("rose-lab8-icc.tif", 16, 2.5),
        ("rose-lab8-tiled.tif", 16, 2.5),
        ("rose-lab12.tif", 16, 2.5),
        ("rose-labf.tif", 16, 2.5),
        // 4 bits, against ImageMagick's own decode of it: a* and b* step by 1/15 of their
        // range, so the two conversions' small differences grow at a few pixels.
        ("rose-lab4.tif", 64, 2.5),
    ] {
        let f = fixture(name);
        let d = decode(&f, 1, None);
        let rgb = if name == "rose-lab4.tif" { &rgb4 } else { &rgb };
        let (dmax, alpha) = diff(&d.data, &rgb.data);
        let mean = d
            .data
            .iter()
            .zip(&rgb.data)
            .map(|(&a, &b)| a.abs_diff(b) as f64)
            .sum::<f64>()
            / d.data.len() as f64;
        eprintln!("{name}: max {dmax}, mean {mean:.2}");
        assert!(
            alpha && dmax <= tol && mean < mean_max,
            "{name}: max {dmax}, mean {mean:.2}"
        );
        // The 8-bit files hold the same samples: the attached RGB profile is ignored.
        if name.contains("lab8") {
            let first = first.get_or_insert_with(|| d.data.clone());
            assert!(*first == d.data, "{name} differs from rose-lab8.tif");
        }
    }
    let icc = Tiff::open(&fixture("rose-lab8-icc.tif")).unwrap();
    assert!(
        icc.pages()[0]
            .icc_profile(&fixture("rose-lab8-icc.tif"))
            .is_some()
    );
}
