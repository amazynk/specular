//! Transforms checked against reference CMMs (moxcms, Little CMS) on the ICC profiles found
//! on the system (tests skip profiles that aren't installed).

use specular_color::math::{bt709_to_pcs, delta_e2000, mul3, srgb_to_linear, xyz_to_lab};
use specular_color::{ColorSpace, Intent, Profile, Transform, profiles};

const DIRS: [&str; 2] = ["/usr/share/color/icc", "/usr/share/color/icc/colord"];

fn find(name: &str) -> Option<Vec<u8>> {
    DIRS.iter()
        .find_map(|d| std::fs::read(format!("{d}/{name}")).ok())
}

fn srgb_lab(v: [f32; 3]) -> [f32; 3] {
    let l = v.map(|x| srgb_to_linear(x.clamp(0.0, 1.0)));
    xyz_to_lab(mul3(&bt709_to_pcs().to_f32(), l))
}

struct Stats {
    n: usize,
    sum: f32,
    max: f32,
}

impl Stats {
    fn new() -> Self {
        Stats {
            n: 0,
            sum: 0.0,
            max: 0.0,
        }
    }
    fn add(&mut self, de: f32) {
        self.n += 1;
        self.sum += de;
        self.max = self.max.max(de);
    }
    fn mean(&self) -> f32 {
        self.sum / self.n.max(1) as f32
    }
}

fn grid(ch: usize, steps: usize) -> Vec<Vec<f32>> {
    let total = steps.pow(ch as u32);
    (0..total)
        .map(|mut i| {
            let mut v = vec![0.0; ch];
            for c in v.iter_mut() {
                *c = (i % steps) as f32 / (steps - 1) as f32;
                i /= steps;
            }
            v
        })
        .collect()
}

const RGB: [&str; 9] = [
    "sRGB.icc",
    "DisplayP3-v4.icc",
    "DisplayP3-v2-magic.icc",
    "AdobeCompat-v4.icc",
    "AdobeCompat-v2.icc",
    "ISO22028-2_ROMM-RGB.icc",
    "ITU-RBT709ReferenceDisplay.icc",
    "Rec2020-v4.icc",
    "AppleCompat-v4.icc",
];

#[test]
fn rgb_profiles_match_moxcms() {
    let dst_m = moxcms::ColorProfile::new_srgb();
    let dst = profiles::srgb();
    let mut tested = 0;
    for name in RGB {
        let Some(data) = find(name) else { continue };
        let Ok(src_m) = moxcms::ColorProfile::new_from_slice(&data) else {
            continue;
        };
        let src = Profile::parse(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
        let opts = moxcms::TransformOptions {
            rendering_intent: moxcms::RenderingIntent::RelativeColorimetric,
            allow_use_cicp_transfer: false,
            ..Default::default()
        };
        let Ok(xm) =
            src_m.create_transform_f32(moxcms::Layout::Rgb, &dst_m, moxcms::Layout::Rgb, opts)
        else {
            continue;
        };
        let ours = Transform::new(&src, &dst, Intent::Relative, false).unwrap();
        let input = grid(3, 9);
        let flat: Vec<f32> = input.iter().flatten().copied().collect();
        let mut out_m = vec![0f32; flat.len()];
        xm.transform(&flat, &mut out_m).unwrap();
        let mut s = Stats::new();
        let mut o = [0f32; 3];
        for (i, v) in input.iter().enumerate() {
            ours.eval(v, &mut o);
            let m = [out_m[i * 3], out_m[i * 3 + 1], out_m[i * 3 + 2]];
            s.add(delta_e2000(srgb_lab(o), srgb_lab(m)));
        }
        eprintln!(
            "{name}: ΔE00 vs moxcms mean {:.4} max {:.4}",
            s.mean(),
            s.max
        );
        assert!(
            s.mean() < 0.2 && s.max < 1.0,
            "{name}: mean {} max {}",
            s.mean(),
            s.max
        );
        tested += 1;
    }
    eprintln!("{tested} RGB profiles compared");
}

const CMYK: [&str; 4] = [
    "ISOcoated_v2_bas.ICC",
    "PSO_Uncoated_ISO12647_bas.ICC",
    "ISOnewspaper_v4_26_bas.ICC",
    "PSO_LWC_Improved_bas.ICC",
];

/// CMYK → sRGB against Little CMS, with and without black point compensation, in all
/// colorimetric/perceptual intents.
#[test]
fn cmyk_profiles_match_lcms() {
    let srgb = profiles::srgb();
    let lsrgb = lcms2::Profile::new_srgb();
    for name in CMYK {
        let Some(data) = find(name) else { continue };
        let src = Profile::parse(&data).unwrap();
        assert_eq!(src.color_space, ColorSpace::Cmyk);
        let lsrc = lcms2::Profile::new_icc(&data).unwrap();
        for (intent, lintent) in [
            (Intent::Relative, lcms2::Intent::RelativeColorimetric),
            (Intent::Perceptual, lcms2::Intent::Perceptual),
        ] {
            for bpc in [false, true] {
                let flags = if bpc {
                    lcms2::Flags::BLACKPOINT_COMPENSATION | lcms2::Flags::NO_CACHE
                } else {
                    lcms2::Flags::NO_CACHE
                };
                let lt: lcms2::Transform<[u16; 4], [u16; 3], _, _> = lcms2::Transform::new_flags(
                    &lsrc,
                    lcms2::PixelFormat::CMYK_16,
                    &lsrgb,
                    lcms2::PixelFormat::RGB_16,
                    lintent,
                    flags,
                )
                .unwrap();
                let ours = Transform::new(&src, &srgb, intent, bpc).unwrap();
                let input = grid(4, 7);
                let li: Vec<[u16; 4]> = input
                    .iter()
                    .map(|v| [0, 1, 2, 3].map(|c| (v[c] * 65535.0).round() as u16))
                    .collect();
                let mut lo = vec![[0u16; 3]; li.len()];
                lt.transform_pixels(&li, &mut lo);
                let mut s = Stats::new();
                let mut o = [0f32; 3];
                for (v, l) in input.iter().zip(&lo) {
                    ours.eval(v, &mut o);
                    s.add(delta_e2000(
                        srgb_lab(o),
                        srgb_lab(l.map(|x| x as f32 / 65535.0)),
                    ));
                }
                eprintln!(
                    "{name} {intent:?} bpc={bpc}: ΔE00 vs lcms mean {:.4} max {:.4}",
                    s.mean(),
                    s.max
                );
                assert!(
                    s.mean() < 0.5 && s.max < 3.0,
                    "{name} {intent:?} bpc={bpc}: mean {} max {}",
                    s.mean(),
                    s.max
                );
            }
        }
    }
}

#[test]
fn cmyk_profiles_match_moxcms() {
    let srgb = profiles::srgb();
    let dst_m = moxcms::ColorProfile::new_srgb();
    for name in CMYK {
        let Some(data) = find(name) else { continue };
        let Ok(src_m) = moxcms::ColorProfile::new_from_slice(&data) else {
            continue;
        };
        let opts = moxcms::TransformOptions {
            rendering_intent: moxcms::RenderingIntent::RelativeColorimetric,
            ..Default::default()
        };
        let Ok(xm) =
            src_m.create_transform_f32(moxcms::Layout::Rgba, &dst_m, moxcms::Layout::Rgb, opts)
        else {
            continue;
        };
        let ours = Transform::new(
            &Profile::parse(&data).unwrap(),
            &srgb,
            Intent::Relative,
            false,
        )
        .unwrap();
        let input = grid(4, 6);
        let flat: Vec<f32> = input.iter().flatten().copied().collect();
        let mut out = vec![0f32; input.len() * 3];
        xm.transform(&flat, &mut out).unwrap();
        let mut s = Stats::new();
        let mut o = [0f32; 3];
        for (i, v) in input.iter().enumerate() {
            ours.eval(v, &mut o);
            s.add(delta_e2000(
                srgb_lab(o),
                srgb_lab([out[i * 3], out[i * 3 + 1], out[i * 3 + 2]]),
            ));
        }
        eprintln!(
            "{name}: ΔE00 vs moxcms mean {:.4} max {:.4}",
            s.mean(),
            s.max
        );
        assert!(
            s.mean() < 0.5 && s.max < 3.0,
            "{name}: mean {} max {}",
            s.mean(),
            s.max
        );
    }
}

/// Lab → CMYK → Lab through the profile's own tables stays close for in-gamut colours.
#[test]
fn cmyk_round_trip() {
    let Some(data) = find("ISOcoated_v2_bas.ICC") else {
        return;
    };
    let p = Profile::parse(&data).unwrap();
    let lab = profiles::lab();
    let to = Transform::new(&lab, &p, Intent::Relative, false).unwrap();
    let back = Transform::new(&p, &lab, Intent::Relative, false).unwrap();
    let mut s = Stats::new();
    let (mut c, mut l) = ([0f32; 4], [0f32; 3]);
    for lab_in in [
        [50.0, 0.0, 0.0],
        [70.0, 10.0, 20.0],
        [40.0, -20.0, -10.0],
        [80.0, 5.0, 50.0],
        [30.0, 15.0, -15.0],
    ] {
        let dev = [
            lab_in[0] / 100.0,
            (lab_in[1] + 128.0) / 255.0,
            (lab_in[2] + 128.0) / 255.0,
        ];
        to.eval(&dev, &mut c);
        back.eval(&c, &mut l);
        let got = [l[0] * 100.0, l[1] * 255.0 - 128.0, l[2] * 255.0 - 128.0];
        s.add(delta_e2000(lab_in, got));
    }
    eprintln!("CMYK round trip ΔE00 mean {:.3} max {:.3}", s.mean(), s.max);
    assert!(s.max < 2.5, "round trip max {}", s.max);
}

/// RGB round trips: device → extended linear BT.709 (unclamped) → device.
#[test]
fn rgb_round_trips() {
    let lin = profiles::linear_bt709();
    for p in [
        profiles::srgb(),
        profiles::display_p3(),
        profiles::adobe_rgb(),
        profiles::bt2020(),
    ]
    .into_iter()
    .chain(
        RGB.iter()
            .filter_map(|n| find(n))
            .filter_map(|d| Profile::parse(&d).ok()),
    ) {
        let a = Transform::new(&p, &lin, Intent::Relative, false).unwrap();
        let b = Transform::new(&lin, &p, Intent::Relative, false).unwrap();
        let (mut m, mut r) = ([0f32; 3], [0f32; 3]);
        let mut worst = 0f32;
        for v in grid(3, 6) {
            a.eval(&v, &mut m);
            b.eval(&m, &mut r);
            for c in 0..3 {
                worst = worst.max((r[c] - v[c]).abs());
            }
        }
        // LUT-based profiles round-trip through their (approximate) inverse tables.
        let tol = if p.is_matrix_shaper() { 3e-3 } else { 2e-2 };
        assert!(worst < tol, "{:x}: worst round-trip error {worst}", p.hash);
    }
}

/// L*a*b* → 8-bit sRGB (PSD Lab composites) within one code of Little CMS (Lab v4 D50 →
/// sRGB, relative colorimetric), through the 8-bit and 16-bit row paths and single colours.
#[test]
fn lab_matches_lcms() {
    use specular_color::LabConverter;
    let d50 = specular_color::math::D50;
    let sum: f64 = d50.iter().map(|&v| v as f64).sum();
    let white = lcms2::CIExyY {
        x: d50[0] as f64 / sum,
        y: d50[1] as f64 / sum,
        Y: 1.0,
    };
    let lab = lcms2::Profile::new_lab4_context(lcms2::GlobalContext::new(), &white).unwrap();
    let t: lcms2::Transform<[f64; 3], [u8; 3], _, _> = lcms2::Transform::new_flags(
        &lab,
        lcms2::PixelFormat::Lab_DBL,
        &lcms2::Profile::new_srgb(),
        lcms2::PixelFormat::RGB_8,
        lcms2::Intent::RelativeColorimetric,
        lcms2::Flags::NO_CACHE,
    )
    .unwrap();
    let conv = LabConverter::default();
    // Every L with neutral a/b, then random planes (many out of gamut). Odd length: the last
    // block is partial.
    let mut seed = 5u32;
    let mut rnd = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 16) as u16
    };
    let n = 50_001 + 256;
    let mut p16: [Vec<u16>; 3] = std::array::from_fn(|_| Vec::with_capacity(n));
    for i in 0..256u16 {
        p16[0].push(i * 257);
        p16[1].push(128 * 257);
        p16[2].push(128 * 257);
    }
    for _ in 256..n {
        for p in &mut p16 {
            p.push(rnd());
        }
    }
    let p8: [Vec<u8>; 3] =
        std::array::from_fn(|c| p16[c].iter().map(|&v| (v >> 8) as u8).collect());
    let mut rgba8 = vec![0u8; n * 4];
    let mut rgba16 = vec![0u8; n * 4];
    conv.convert_planes8(&p8[0], &p8[1], &p8[2], &mut rgba8);
    conv.convert_planes16(&p16[0], &p16[1], &p16[2], &mut rgba16);
    let mut worst = 0;
    let mut same = 0;
    for (wide, rgba) in [(false, &rgba8), (true, &rgba16)] {
        let labs: Vec<[f64; 3]> = (0..n)
            .map(|i| {
                if wide {
                    let v = |c: usize| p16[c][i] as f64;
                    [
                        v(0) * 100.0 / 65535.0,
                        v(1) / 257.0 - 128.0,
                        v(2) / 257.0 - 128.0,
                    ]
                } else {
                    let v = |c: usize| p8[c][i] as f64;
                    [v(0) * 100.0 / 255.0, v(1) - 128.0, v(2) - 128.0]
                }
            })
            .collect();
        let mut want = vec![[0u8; 3]; n];
        t.transform_pixels(&labs, &mut want);
        for (i, (w, l)) in want.iter().zip(&labs).enumerate() {
            let got = &rgba[i * 4..i * 4 + 4];
            assert_eq!(got[3], 255);
            // The single-colour path (its L, a, b rounded differently by an ulp or so).
            let one = conv.convert(l.map(|v| v as f32));
            let d = (0..3)
                .map(|c| (got[c] as i32 - w[c] as i32).abs())
                .max()
                .unwrap();
            assert!(
                (0..3).all(|c| (one[c] as i32 - w[c] as i32).abs() <= 1),
                "{l:?}: ours {one:?} lcms {w:?}"
            );
            assert!(d <= 1, "{l:?}: ours {got:?} lcms {w:?}");
            worst = worst.max(d);
            same += usize::from(d == 0);
        }
    }
    eprintln!(
        "Lab vs lcms2: worst {worst} code(s), {:.2} % identical",
        same as f64 * 50.0 / n as f64
    );
    assert!(same * 100 >= 2 * n * 95, "{same} of {}", 2 * n);
}
