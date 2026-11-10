//! The decoders' CMYK path (`CmykConverter`: a profile's own table, or a baked one) on solid
//! and near-solid ink (single inks, overprints, high total area coverage and rich black:
//! where press profiles bend most, many colours leave the sRGB gamut, and dark output makes
//! errors most visible) and on random inks over the whole cube.
//!
//! References: Little CMS for the system press profiles (tests skip profiles that aren't
//! installed), and our own exact transform (itself within 0.15 code of Little CMS) for those
//! and the built-in generic CMYK model.

use specular_color::{CmykConverter, Intent, Profile, Transform, profiles};

const DIRS: [&str; 2] = ["/usr/share/color/icc", "/usr/share/color/icc/colord"];
const CMYK: [&str; 4] = [
    "ISOcoated_v2_bas.ICC",
    "PSO_Uncoated_ISO12647_bas.ICC",
    "ISOnewspaper_v4_26_bas.ICC",
    "PSO_LWC_Improved_bas.ICC",
];

fn find(name: &str) -> Option<Vec<u8>> {
    DIRS.iter()
        .find_map(|d| std::fs::read(format!("{d}/{name}")).ok())
}

/// Every combination of these ink levels: 28 561 patches, most of them with at least one
/// ink at 87 % or more.
fn patches() -> Vec<[u8; 4]> {
    const LEVELS: [u8; 13] = [0, 64, 128, 192, 224, 232, 240, 244, 248, 250, 252, 254, 255];
    let mut v = Vec::with_capacity(LEVELS.len().pow(4));
    for c in LEVELS {
        for m in LEVELS {
            for y in LEVELS {
                for k in LEVELS {
                    v.push([c, m, y, k]);
                }
            }
        }
    }
    v
}

/// Random inks (seeded xorshift), uniform over the cube.
fn random_patches(n: usize) -> Vec<[u8; 4]> {
    let mut seed = 0x2545_f491u32;
    (0..n)
        .map(|_| {
            [0; 4].map(|_: u8| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                (seed >> 24) as u8
            })
        })
        .collect()
}

/// Distances (codes) between the converter's output and the rounded reference (fractional
/// codes).
struct Errors {
    /// Largest, and where.
    worst: f64,
    at: [u8; 4],
    /// Largest before rounding the reference.
    raw: f64,
    /// Share of patches (%) matching the rounded reference exactly.
    exact: f64,
}

fn compare(
    conv: &CmykConverter,
    pats: &[[u8; 4]],
    mut want: impl FnMut(usize) -> [f64; 3],
) -> Errors {
    let mut e = Errors {
        worst: 0.0,
        at: [0; 4],
        raw: 0.0,
        exact: 0.0,
    };
    let mut same = 0usize;
    for (i, p) in pats.iter().enumerate() {
        let got = conv.convert(*p);
        let w = want(i);
        let mut pe = 0f64;
        for ch in 0..3 {
            let r = w[ch].clamp(0.0, 255.0);
            pe = pe.max((f64::from(got[ch]) - r.round()).abs());
            e.raw = e.raw.max((f64::from(got[ch]) - r).abs());
        }
        if pe > e.worst {
            e.worst = pe;
            e.at = *p;
        }
        same += usize::from(pe == 0.0);
    }
    e.exact = same as f64 * 100.0 / pats.len() as f64;
    e
}

#[test]
fn near_solid_ink_matches_little_cms() {
    vs_little_cms("near-solid", &patches());
}

#[test]
fn random_ink_matches_little_cms() {
    vs_little_cms("random", &random_patches(50_000));
}

/// Every installed press profile, colorimetric with and without black point compensation
/// and perceptual: within one code of rounded Little CMS everywhere, and equal to it for
/// nearly all patches (the rest sit on a rounding boundary: the two differ by up to 0.15
/// code before rounding).
fn vs_little_cms(set: &str, pats: &[[u8; 4]]) {
    let px: Vec<[f64; 4]> = pats
        .iter()
        .map(|c| c.map(|v| f64::from(v) / 2.55))
        .collect();
    for name in CMYK {
        let Some(data) = find(name) else { continue };
        let src = Profile::parse(&data).unwrap();
        let lsrc = lcms2::Profile::new_icc(&data).unwrap();
        for (intent, li, bpc) in [
            (Intent::Relative, lcms2::Intent::RelativeColorimetric, true),
            (Intent::Relative, lcms2::Intent::RelativeColorimetric, false),
            (Intent::Perceptual, lcms2::Intent::Perceptual, true),
        ] {
            let flags = if bpc {
                lcms2::Flags::BLACKPOINT_COMPENSATION | lcms2::Flags::NO_CACHE
            } else {
                lcms2::Flags::NO_CACHE
            };
            let lt: lcms2::Transform<[f64; 4], [f64; 3]> = lcms2::Transform::new_flags(
                &lsrc,
                lcms2::PixelFormat::CMYK_DBL,
                &lcms2::Profile::new_srgb(),
                lcms2::PixelFormat::RGB_DBL,
                li,
                flags,
            )
            .unwrap();
            let mut want = vec![[0f64; 3]; px.len()];
            lt.transform_pixels(&px, &mut want);
            let conv = CmykConverter::new(&src, intent, bpc).unwrap();
            let e = compare(&conv, pats, |i| want[i].map(|v| v * 255.0));
            eprintln!(
                "{name} {intent:?} bpc={bpc} {set}: {} codes from rounded Little CMS at {:?} \
                 (unrounded {:.2}), {:.2} % equal",
                e.worst, e.at, e.raw, e.exact
            );
            assert!(
                e.worst <= 1.0 && e.exact >= 95.0,
                "{name} {intent:?} bpc={bpc} {set}: {} codes at {:?}, {:.2} % equal",
                e.worst,
                e.at,
                e.exact
            );
        }
    }
}

#[test]
fn near_solid_ink_matches_exact_transform() {
    let pats = patches();
    let mut srcs: Vec<(String, Profile)> = vec![("generic".into(), profiles::generic_cmyk())];
    for name in CMYK {
        if let Some(d) = find(name) {
            srcs.push((name.into(), Profile::parse(&d).unwrap()));
        }
    }
    for (name, src) in srcs {
        let conv = CmykConverter::new(&src, Intent::Relative, true).unwrap();
        let t = Transform::new(&src, &profiles::srgb(), Intent::Relative, true).unwrap();
        let mut o = [0f32; 3];
        let e = compare(&conv, &pats, |i| {
            t.eval(&pats[i].map(|v| f32::from(v) / 255.0), &mut o);
            o.map(|v| f64::from(v) * 255.0)
        });
        eprintln!(
            "{name}: {} codes from the rounded exact transform at {:?} (unrounded {:.2}), \
             {:.2} % equal",
            e.worst, e.at, e.raw, e.exact
        );
        assert!(e.worst <= 1.0, "{name}: {} codes at {:?}", e.worst, e.at);
    }
}
