use super::preset::{Depth, FileFormat, Preset};
use super::render::turn;
use super::{Space, Transfer};
use crate::colour;
use crate::preview::Orient;
use specular_convert::{Conflict, MetaMode, OutputDir, Resize};

fn max_diff(a: &colour::M3, b: &colour::M3) -> f64 {
    let mut m = 0f64;
    for i in 0..3 {
        for j in 0..3 {
            m = m.max((a[i][j] - b[i][j]).abs());
        }
    }
    m
}

/// sRGB's matrix is the colour module's Rec.2020 → Rec.709, and every space round-trips.
#[test]
fn space_matrices_agree() {
    assert!(max_diff(&Space::Srgb.from_rec2020(), &colour::rec2020_to_rec709()) < 1e-12);
    assert!(max_diff(&Space::Rec2020.from_rec2020(), &colour::IDENTITY) < 1e-12);
    for s in Space::ALL {
        let m = colour::mul(&s.to_rec2020(), &s.from_rec2020());
        assert!(max_diff(&m, &colour::IDENTITY) < 1e-12, "{s:?}");
        // White stays white.
        let w = colour::mulv(&s.from_rec2020(), [1.0, 1.0, 1.0]);
        assert!(w.iter().all(|v| (v - 1.0).abs() < 1e-9), "{s:?} {w:?}");
        assert_eq!(Space::from_id(s.id()), Some(s));
    }
}

/// Every curve decodes what it encodes, and the ICC `para` form evaluates the same.
#[test]
fn transfers_round_trip() {
    for t in [
        Transfer::Srgb,
        Transfer::Bt709,
        Transfer::Gamma(563.0 / 256.0),
        Transfer::Linear,
    ] {
        for i in 0..=1000 {
            let l = f64::from(i) / 1000.0;
            let e = t.encode(l);
            assert!((t.decode(e) - l).abs() < 1e-9, "{t:?} {l}");
        }
    }
}

/// The embedded profiles parse back with our ICC reader: D50 colorants of the space
/// (Bradford from D65) and the space's curve on every channel.
#[test]
fn profiles_parse_back() {
    for s in Space::ALL {
        for linear in [false, true] {
            let bytes = s.icc(linear);
            let p = specular_color::Profile::parse(&bytes).expect("parses");
            assert!(p.is_matrix_shaper(), "{s:?}");
            let [r, g, b] = s.primaries();
            let m = specular_color::math::rgb_to_pcs(r, g, b, colour::D65_XY);
            assert!(
                p.matrix.as_ref().is_some_and(|pm| pm.max_diff(&m) < 1e-4),
                "{s:?}"
            );
            let t = if linear {
                Transfer::Linear
            } else {
                s.transfer()
            };
            for c in p.trc.as_ref().expect("curves") {
                for i in 0..=64 {
                    let x = i as f32 / 64.0;
                    let want = t.decode(f64::from(x)) as f32;
                    assert!((c.eval(x) - want).abs() < 2e-4, "{s:?} {x}: {c:?}");
                }
            }
        }
    }
}

/// The tabulated curve stays within a 16-bit code of the exact one.
#[test]
fn the_sample_curve_is_exact_to_sixteen_bits() {
    for t in [
        Transfer::Srgb,
        Transfer::Bt709,
        Transfer::Gamma(563.0 / 256.0),
    ] {
        let c = super::render::curve_for_tests(t);
        let mut worst = 0f64;
        for i in 0..=200_000 {
            let l = f64::from(i) / 200_000.0;
            let l = l * l;
            let d = (f64::from(c(l as f32)) - t.encode(l)).abs();
            worst = worst.max(d);
        }
        assert!(worst * 65535.0 < 0.5, "{t:?}: {} codes", worst * 65535.0);
    }
}

/// Presets survive their text form; bad values keep defaults; formats hold their depths.
#[test]
fn presets_round_trip_as_text() {
    let mut mine = Preset {
        name: "Print, A3".into(),
        format: FileFormat::Tiff,
        depth: Depth::Float,
        quality: 77,
        space: Space::DisplayP3,
        size: Resize::Fit {
            width: 4000,
            height: 3000,
        },
        upscale: true,
        metadata: MetaMode::Strip,
        pattern: "{name}-{index:3}".into(),
        folder: OutputDir::Folder("/srv/out dir".into()),
        conflict: Conflict::Skip,
        ..Preset::default()
    };
    let mut all = Preset::builtin();
    all.push(mine.clone());
    mine.name = "Small".into();
    mine.size = Resize::Percent(25);
    mine.folder = OutputDir::Subfolder(String::new());
    mine.format = FileFormat::Jxl;
    mine.depth = Depth::Sixteen;
    mine.lossless = true;
    all.push(mine);
    let text = Preset::write_all(&all);
    let back = Preset::parse_all(&text);
    assert_eq!(back.len(), all.len());
    for (a, b) in all.iter().zip(&back) {
        assert_eq!(a.clone().normalised(), b.clone(), "{text}");
    }
    // Junk and unknown keys are skipped; a JPEG cannot be 16-bit.
    let p = Preset::parse_all(
        "junk before\n[x]\nformat = jpeg\ndepth = 16\nquality = 900\nshiny = yes\nsize = fit 0 5\n",
    );
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].depth, Depth::Eight);
    assert_eq!(p[0].quality, 90);
    assert_eq!(p[0].size, Resize::None);
    // Shipped presets by id, the user's by name.
    assert_eq!(
        Preset::find("web", &[]).unwrap().size,
        Resize::LongEdge(2048)
    );
    assert_eq!(
        Preset::find("print, a3", &back).unwrap().depth,
        Depth::Float
    );
    assert!(Preset::find("nope", &back).is_none());
    for f in FileFormat::ALL {
        assert_eq!(FileFormat::from_id(f.id()), Some(f));
        for d in f.depths() {
            assert_eq!(Depth::from_id(d.id()), Some(*d));
        }
    }
}

/// Turning matches [`Orient::source_of`] (EXIF 6: a quarter turn clockwise).
#[test]
fn turning_moves_whole_pixels() {
    // 3×2: a b c / d e f, one sample each repeated 3 times.
    let src: Vec<u8> = (0..6u8).flat_map(|v| [v, v, v]).collect();
    let out = turn(&src, 3, 2, Orient::from_exif(6));
    let firsts: Vec<u8> = out.chunks(3).map(|p| p[0]).collect();
    assert_eq!(firsts, [3, 0, 4, 1, 5, 2]);
    assert_eq!(turn(&src, 3, 2, Orient::default()), src);
}

/// Turning in tiles gives every orientation's pixels, also where the size is not a whole
/// number of tiles.
#[test]
fn turning_in_tiles_matches_pixel_by_pixel() {
    let (w, h) = (130usize, 71usize);
    let src: Vec<u32> = (0..(w * h) as u32)
        .flat_map(|v| [v, v + 1, v + 2])
        .collect();
    for e in 1..=8u16 {
        let o = Orient::from_exif(e);
        let (ow, oh) = o.size(w as u32, h as u32);
        let mut want = Vec::with_capacity(src.len());
        for y in 0..oh {
            for x in 0..ow {
                let (sx, sy) = o.source_of(x, y, w as u32, h as u32);
                let s = (sy as usize * w + sx as usize) * 3;
                want.extend_from_slice(&src[s..s + 3]);
            }
        }
        assert_eq!(turn(&src, w, h, o), want, "orientation {e}");
    }
}

/// The integer files' curves (every export space's).
fn integer_curves() -> Vec<(Transfer, super::render::Curve)> {
    [Space::Srgb, Space::Rec2020, Space::AdobeRgb]
        .map(|s| (s.transfer(), super::render::Curve::new(s.transfer())))
        .into()
}

/// The GPU codes 8-bit samples by counting thresholds, exact only when the curve
/// never decreases: checked for every f32 value of 0..=1 and every integer file's curve.
#[test]
fn the_curves_never_decrease() {
    use rayon::prelude::*;
    let one = 1f32.to_bits();
    for (t, c) in integer_curves() {
        let chunk = 1u32 << 20;
        let bad = (0..one.div_ceil(chunk))
            .into_par_iter()
            .map(|k| {
                let (a, b) = (k * chunk, ((k + 1) * chunk).min(one));
                let mut prev = c.at(f32::from_bits(a));
                let mut bad = 0u64;
                for bits in a + 1..=b {
                    let v = c.at(f32::from_bits(bits));
                    bad += u64::from(v < prev);
                    prev = v;
                }
                bad
            })
            .sum::<u64>();
        assert_eq!(bad, 0, "{t:?}: {bad} decreases");
    }
}

/// The thresholds give the CPU's 8-bit code of every value tried: around every
/// threshold, on a dense grid, outside 0..=1 and NaN.
#[test]
fn thresholds_give_the_cpus_codes() {
    use super::quantise::Thresholds;
    for (t, c) in integer_curves() {
        let th = Thresholds::new(8, |l| u16::from(c.code8(l)));
        let mut tried = 0usize;
        let mut check = |l: f32| {
            assert_eq!(
                th.code(l),
                u16::from(c.code8(l)),
                "{t:?} at {l:e} ({:#x})",
                l.to_bits()
            );
            tried += 1;
        };
        for &x in th.t.iter().filter(|x| x.is_finite()) {
            for d in -4i32..=4 {
                check(f32::from_bits(x.to_bits().wrapping_add_signed(d)));
            }
        }
        for i in 0..=200_000 {
            check(i as f32 / 200_000.0);
        }
        for l in [
            f32::NAN,
            -f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -0.0,
            0.0,
            -1.0,
            1.0,
            1.5,
            1e30,
            f32::MIN_POSITIVE,
            f32::from_bits(1),
        ] {
            check(l);
        }
        eprintln!("{t:?}: {tried} values");
    }
}

/// The 16-bit thresholds give the CPU's 16-bit code of every value tried: around
/// every threshold, on a dense grid, outside 0..=1 and NaN.
#[test]
fn sixteen_bit_thresholds_give_the_cpus_codes() {
    use super::quantise::Thresholds;
    use rayon::prelude::*;
    for (t, c) in integer_curves() {
        let th = Thresholds::new(16, |l| c.code16(l));
        let near: Vec<f32> =
            th.t.iter()
                .filter(|x| x.is_finite())
                .flat_map(|x| {
                    (-4i32..=4).map(move |d| f32::from_bits(x.to_bits().wrapping_add_signed(d)))
                })
                .chain((0..=2_000_000).map(|i| i as f32 / 2_000_000.0))
                .chain([
                    f32::NAN,
                    -f32::NAN,
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                    -0.0,
                    0.0,
                    -1.0,
                    1.0,
                    1.5,
                    1e30,
                    f32::MIN_POSITIVE,
                    f32::from_bits(1),
                ])
                .collect();
        let bad: Vec<f32> = near
            .par_iter()
            .copied()
            .filter(|&l| th.code(l) != c.code16(l))
            .collect();
        assert!(
            bad.is_empty(),
            "{t:?}: {} differ, first {:e}",
            bad.len(),
            bad[0]
        );
        eprintln!("{t:?}: {} values", near.len());
    }
}

/// The batch writer takes the rendered samples into the file rather than copying them: the
/// same bytes as [`super::batch::encode`] (which copies), and the time of each, 24 MP 8-bit
/// TIFF, paired rounds, best of each.
#[test]
fn batch_encode_takes_the_samples() {
    use super::batch::{encode, encode_taking};
    use super::render::{ExportStats, Exported, Samples};
    let (w, h) = (6000u32, 4000u32);
    let img = Exported {
        width: w,
        height: h,
        samples: Samples::Eight(
            (0..w as usize * h as usize * 3)
                .map(|i| (i / 3 % 251) as u8)
                .collect(),
        ),
        stats: ExportStats::default(),
        gain_map: None,
        alpha: None,
    };
    let p = Preset {
        format: FileFormat::Tiff,
        depth: Depth::Eight,
        ..Preset::default()
    };
    let (mut copying, mut taking) = (std::time::Duration::MAX, std::time::Duration::MAX);
    for _ in 0..3 {
        let t0 = std::time::Instant::now();
        let a = encode(&img, &p, None, None).unwrap();
        copying = copying.min(t0.elapsed());
        let copy = img.clone();
        let t0 = std::time::Instant::now();
        let b = encode_taking(copy, &p, None, None).unwrap();
        taking = taking.min(t0.elapsed());
        assert!(a == b, "the files differ");
    }
    eprintln!("24 MP 8-bit TIFF: copying {copying:.1?}, taking {taking:.1?}");
}
