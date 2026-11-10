//! Synthetic charts: camera values made from the reference through a known camera (a body
//! matrix, a hue twist the matrix cannot undo, noise), rendered with perspective, vignetting
//! and a glare spot.

use super::cgats::{RefPatch, Reference};
use super::colour::{self, Lab, M3, V3};
use super::layout::{ChartKind, Layout};
use super::output::{self, Shot};
use super::sample::{self, Flag, Picture, Sampled};
use super::solve::{self, Input};
use crate::ops::geometry::homography::Homography;

/// ColorChecker 24, X-Rite's published L*a*b* D50 (test data only, as `specular-raw`'s tests
/// use it).
pub(crate) const CHECKER_LAB: [Lab; 24] = [
    [37.54, 14.37, 14.92],
    [64.66, 19.27, 17.5],
    [49.32, -3.82, -22.54],
    [43.46, -12.74, 22.72],
    [54.94, 9.61, -24.79],
    [70.48, -32.26, -0.37],
    [62.73, 35.83, 56.5],
    [39.43, 10.75, -45.17],
    [50.57, 48.64, 16.67],
    [30.1, 22.54, -20.87],
    [71.77, -24.13, 58.19],
    [71.51, 18.24, 67.37],
    [28.37, 15.42, -49.8],
    [54.38, -39.72, 32.27],
    [42.43, 51.05, 28.62],
    [81.8, 2.67, 80.41],
    [50.63, 51.28, -14.12],
    [49.57, -29.71, -28.32],
    [95.19, -1.03, 2.93],
    [81.29, -0.57, 0.44],
    [66.89, -0.75, -0.06],
    [50.76, -0.13, 0.14],
    [35.63, -0.46, -0.48],
    [20.64, 0.07, -0.46],
];

pub(crate) fn checker() -> Layout {
    let r = Reference {
        title: "test".into(),
        patches: CHECKER_LAB
            .iter()
            .enumerate()
            .map(|(i, l)| RefPatch {
                id: (i + 1).to_string(),
                lab: *l,
            })
            .collect(),
    };
    Layout::new(ChartKind::ColorChecker24, &r).unwrap()
}

/// XYZ D50 → camera raw: a body matrix (a Canon-like XYZ → camera, rows mixed).
pub(crate) const CAMERA: M3 = [
    [0.6722, 0.1350, -0.0613],
    [-0.0910, 0.9200, 0.1721],
    [0.0140, 0.0850, 0.5530],
];

/// The camera's nonlinear part: a hue twist and saturation change in ProPhoto HSV that no
/// matrix undoes.
fn twist(xyz: V3) -> V3 {
    let to_pro = colour::inv(&colour::prophoto_to_xyz()).unwrap();
    let p = colour::mulv(&to_pro, xyz);
    if p.iter().any(|v| *v <= 0.0) {
        return xyz;
    }
    let [h, s, v] = colour::rgb_to_hsv(p);
    let a = h * std::f64::consts::PI / 3.0;
    let h2 = (h + (6.0 / 60.0) * (2.0 * a).sin() * s).rem_euclid(6.0);
    let s2 = (s * (1.0 + 0.10 * a.cos())).min(1.0);
    colour::mulv(&colour::prophoto_to_xyz(), colour::hsv_to_rgb([h2, s2, v]))
}

/// Deterministic noise in −1..1.
fn noise(i: u64) -> f64 {
    let mut x = i.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 32;
    (x as f64 / u64::MAX as f64) * 2.0 - 1.0
}

/// The camera's white-balanced values for each patch (as-shot multipliers `wb`).
fn camera_samples(layout: &Layout, twisted: bool, sigma: f64, wb: V3) -> Vec<Sampled> {
    layout
        .patches
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut xyz = colour::lab_to_xyz(p.reference);
            if twisted {
                xyz = twist(xyz);
            }
            let raw = colour::mulv(&CAMERA, xyz);
            let rgb: V3 = std::array::from_fn(|c| {
                0.8 * raw[c] * wb[c] * (1.0 + sigma * noise((i * 3 + c) as u64))
            });
            Sampled {
                centre: (0.0, 0.0),
                rgb,
                spread: 0.0,
                flag: None,
            }
        })
        .collect()
}

const WB: V3 = [1.9, 1.0, 1.45];

fn before() -> M3 {
    colour::IDENTITY
}

#[test]
fn matrix_only_fit_recovers_the_camera_matrix() {
    let l = checker();
    let s = camera_samples(&l, false, 0.0, WB);
    let fit = solve::solve(&Input {
        layout: &l,
        samples: &s,
        before: before(),
        table: false,
    })
    .unwrap();
    // The truth: neutral-normalised camera → XYZ D50, white-preserving.
    let n = colour::mulv(&CAMERA, colour::d50());
    let mut truth = colour::mul(&colour::inv(&CAMERA).unwrap(), &colour::diag(n));
    let k = colour::d50()[1] / (0..3).map(|j| truth[1][j]).sum::<f64>();
    for r in &mut truth {
        for v in r.iter_mut() {
            *v *= k;
        }
    }
    let worst = (0..9)
        .map(|i| (fit.fm[i / 3][i % 3] - truth[i / 3][i % 3]).abs())
        .fold(0f64, f64::max);
    eprintln!(
        "matrix: max |FM - truth| {worst:.2e}, mean ΔE {:.4}",
        fit.report.matrix.mean
    );
    assert!(worst < 1e-3, "{:?} vs {truth:?}", fit.fm);
    assert!(fit.report.matrix.max < 0.05);
    // The closed form alone is already the truth here.
    let c: Vec<V3> = s
        .iter()
        .map(|p| std::array::from_fn(|k| p.rgb[k] / fit.neutral[k]))
        .collect();
    let x: Vec<V3> = l
        .patches
        .iter()
        .map(|p| colour::lab_to_xyz(p.reference))
        .collect();
    let wp = solve::white_preserving(&c, &x, colour::d50()).unwrap();
    let wsum: f64 = wp[1].iter().sum();
    assert!((wsum - colour::d50()[1]).abs() < 1e-12);
}

#[test]
fn matrix_and_table_fit_a_twisted_camera() {
    let l = checker();
    let s = camera_samples(&l, true, 0.005, WB);
    let fit = solve::solve(&Input {
        layout: &l,
        samples: &s,
        before: before(),
        table: true,
    })
    .unwrap();
    let r = &fit.report;
    eprintln!(
        "twisted camera ΔE2000 mean/median/max: matrix {:.2}/{:.2}/{:.2}, matrix+table {:.2}/{:.2}/{:.2}, cross-validated {:.2}, root-polynomial {:.2}, λ {:e}",
        r.matrix.mean,
        r.matrix.median,
        r.matrix.max,
        r.after.mean,
        r.after.median,
        r.after.max,
        r.cross_validated.mean,
        r.root_polynomial.mean,
        r.lambda
    );
    assert!(r.after.mean < 1.0, "after {:?}", r.after);
    assert!(r.after.mean < r.matrix.mean);
    assert!(r.cross_validated.mean < 2.0, "cv {:?}", r.cross_validated);
    let t = fit.table.as_ref().unwrap();
    assert_eq!((t.hue, t.sat), (12, 4));
    for h in 0..12 {
        assert_eq!(t.data[h * 4], [0.0, 1.0, 1.0], "identity at saturation 0");
    }
    assert!(
        t.data
            .iter()
            .all(|e| e[0].abs() <= 15.0 && (0.6..=1.5).contains(&e[1]))
    );
    assert!(r.uneven_light.is_none());
}

#[test]
fn too_few_patches_and_no_greys_are_refused() {
    let l = checker();
    let mut s = camera_samples(&l, false, 0.0, WB);
    for p in s.iter_mut().skip(10) {
        p.flag = Some(Flag::Clipped);
    }
    let input = Input {
        layout: &l,
        samples: &s,
        before: before(),
        table: true,
    };
    assert_eq!(
        solve::solve(&input).err(),
        Some(solve::SolveError::TooFewPatches { usable: 10 })
    );
    let mut s = camera_samples(&l, false, 0.0, WB);
    for p in s.iter_mut().skip(18) {
        p.flag = Some(Flag::Uneven);
    }
    let input = Input {
        layout: &l,
        samples: &s,
        before: before(),
        table: true,
    };
    assert_eq!(
        solve::solve(&input).err(),
        Some(solve::SolveError::NoNeutrals)
    );
}

/// A chart picture: perspective `h` (grid → pixels), patches 80 % of the pitch on a dark
/// surround, vignetting, glare on patch `glare`, noise.
pub(crate) fn render(
    layout: &Layout,
    values: &[V3],
    h: &Homography,
    (w, ht): (u32, u32),
    glare: Option<usize>,
) -> Vec<f32> {
    let inv = h.inverse().unwrap();
    let mut out = vec![0f32; (w * ht * 4) as usize];
    let (cx, cy) = (f64::from(w) / 2.0, f64::from(ht) / 2.0);
    let rmax = cx.hypot(cy);
    for y in 0..ht {
        for x in 0..w {
            let (px, py) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
            let g = inv.apply((px, py)).unwrap();
            let surround = [0.03, 0.03, 0.03];
            let mut v = surround;
            for (i, p) in layout.patches.iter().enumerate() {
                if (g.0 - p.at.0).abs() < 0.4 && (g.1 - p.at.1).abs() < 0.4 {
                    v = values[i];
                    if glare == Some(i) {
                        // A bright streak across the patch.
                        let d = (g.0 - p.at.0 + 0.05).abs();
                        let k = (1.0 - d / 0.12).max(0.0) * 0.6;
                        v = v.map(|c| c + k);
                    }
                }
            }
            let r = (px - cx).hypot(py - cy) / rmax;
            let vig = 1.0 - 0.02 * r * r;
            let i = ((y * w + x) * 4) as usize;
            for c in 0..3 {
                let n = 1.0 + 0.005 * noise(u64::from(y * w + x) * 3 + c as u64);
                out[i + c] = (v[c] * vig * n) as f32;
            }
            out[i + 3] = 1.0;
        }
    }
    out
}

pub(crate) fn tilted(w: u32, h: u32) -> Homography {
    let (w, h) = (f64::from(w), f64::from(h));
    Homography::from_quads(
        [(0.0, 0.0), (5.0, 0.0), (5.0, 3.0), (0.0, 3.0)],
        [
            (0.17 * w, 0.20 * h),
            (0.84 * w, 0.14 * h),
            (0.80 * w, 0.82 * h),
            (0.13 * w, 0.76 * h),
        ],
    )
    .unwrap()
}

#[test]
fn chart_is_found_sampled_and_profiled_through_perspective() {
    let l = checker();
    let truth = camera_samples(&l, true, 0.0, WB);
    let values: Vec<V3> = truth.iter().map(|s| s.rgb).collect();
    let (w, ht) = (720, 520);
    let h = tilted(w, ht);
    let glare = 9; // purple
    let px = render(&l, &values, &h, (w, ht), Some(glare));
    let pic = Picture {
        w,
        h: ht,
        rgba: &px,
    };
    // Clicks a third of a patch off the corner centres.
    let pitch = 88.0;
    let clicks = l.corners.map(|i| {
        let c = h.apply(l.patches[i].at).unwrap();
        (c.0 + 0.3 * pitch * 0.7, c.1 - 0.3 * pitch * 0.7)
    });
    let at = sample::locate(&l, &pic, clicks).unwrap();
    let inv = h.inverse().unwrap();
    let worst = l
        .patches
        .iter()
        .zip(&at.centres)
        .map(|(p, c)| {
            let g = inv.apply(*c).unwrap();
            (g.0 - p.at.0).hypot(g.1 - p.at.1)
        })
        .fold(0f64, f64::max);
    eprintln!("detection: worst centre {worst:.3} patch");
    assert!(worst < 0.3, "worst centre {worst} patch");
    let s = sample::sample(&l, &pic, &at, [10.0; 3]);
    let flagged: Vec<usize> = (0..24).filter(|&i| s[i].flag.is_some()).collect();
    assert_eq!(
        flagged,
        [glare],
        "{:?}",
        s.iter().map(|x| x.spread).collect::<Vec<_>>()
    );
    assert_eq!(s[glare].flag, Some(Flag::Uneven));
    let fit = solve::solve(&Input {
        layout: &l,
        samples: &s,
        before: before(),
        table: true,
    })
    .unwrap();
    let r = &fit.report;
    eprintln!(
        "rendered chart: ΔE2000 mean matrix {:.2}, after {:.2}, cross-validated {:.2}",
        r.matrix.mean, r.after.mean, r.cross_validated.mean
    );
    assert!(r.after.mean < 1.0 && r.cross_validated.mean < 2.0);
    assert_eq!(r.patches[glare].left_out, Some(Flag::Uneven));
}

#[test]
fn a_patch_outside_the_picture_or_clipped_is_left_out() {
    let l = checker();
    let truth = camera_samples(&l, false, 0.0, WB);
    let mut values: Vec<V3> = truth.iter().map(|s| s.rgb).collect();
    values[5] = [1.2, 1.2, 1.2]; // bluish green, over the clip level
    let (w, ht) = (600, 420);
    // The bottom row falls off the picture.
    let h = Homography::from_quads(
        [(0.0, 0.0), (5.0, 0.0), (5.0, 3.0), (0.0, 3.0)],
        [(80.0, 70.0), (560.0, 70.0), (560.0, 430.0), (80.0, 430.0)],
    )
    .unwrap();
    let px = render(&l, &values, &h, (w, ht), None);
    let pic = Picture {
        w,
        h: ht,
        rgba: &px,
    };
    let at = sample::Located {
        grid_to_picture: h,
        centres: l.patches.iter().map(|p| h.apply(p.at).unwrap()).collect(),
        corners: l.corners.map(|i| h.apply(l.patches[i].at).unwrap()),
    };
    let s = sample::sample(&l, &pic, &at, [1.0; 3]);
    assert_eq!(s[23].flag, Some(Flag::Outside));
    assert_eq!(s[5].flag, Some(Flag::Clipped));
    assert!(
        s[..18]
            .iter()
            .enumerate()
            .all(|(i, p)| i == 5 || p.flag.is_none())
    );
}

#[test]
fn snapping_finds_the_patch_middle() {
    let l = checker();
    let values: Vec<V3> = (0..24).map(|i| [0.1 + 0.03 * i as f64; 3]).collect();
    let h = Homography::from_quads(
        [(0.0, 0.0), (5.0, 0.0), (5.0, 3.0), (0.0, 3.0)],
        [(60.0, 60.0), (460.0, 60.0), (460.0, 300.0), (60.0, 300.0)],
    )
    .unwrap();
    let px = render(&l, &values, &h, (520, 360), None);
    let pic = Picture {
        w: 520,
        h: 360,
        rgba: &px,
    };
    let c = h.apply((2.0, 1.0)).unwrap();
    for off in [(-30.0, 0.0), (25.0, 25.0), (0.0, -35.0)] {
        let got = sample::snap(&pic, (c.0 + off.0, c.1 + off.1), 80.0);
        assert!(
            (got.0 - c.0).hypot(got.1 - c.1) < 6.0,
            "{off:?}: {got:?} vs {c:?}"
        );
    }
}

/// The DCP written for a fit, read back and applied the DNG way, gives the solver's own
/// forward map.
#[test]
fn dcp_round_trip_equals_the_solvers_forward_map() {
    let l = checker();
    let s = camera_samples(&l, true, 0.002, WB);
    let fit = solve::solve(&Input {
        layout: &l,
        samples: &s,
        before: before(),
        table: true,
    })
    .unwrap();
    // A sensor shot with as-shot multipliers WB.
    let mut meta = crate::ops::raw::tests::sensor(&crate::ops::raw::tests::bayer(8, 8)).meta;
    meta.wb = WB.map(|v| v as f32);
    let raw_neutral = super::raw_neutral(fit.neutral, &meta);
    let shot = Shot {
        fit: &fit,
        raw_neutral,
        white_xy: Some([0.3457, 0.3585]),
    };
    let dcp = output::dcp("Test Camera", "Chart", &[shot]).unwrap();
    assert_eq!(dcp.illuminants, [23, 0]);
    let back = specular_raw::Dcp::read(&dcp.write()).unwrap();
    meta.apply_dcp(&back).unwrap();
    let prof = meta.profile.clone().expect("a profile with its table");
    let mut to709 = colour::mul(&colour::xyz_to_rec709(), &colour::prophoto_to_xyz());
    colour::normalize_rows(&mut to709);
    // The photo's own white balance is the chart's: values in its white-balanced units.
    let mut worst = 0f64;
    for i in 0..400u64 {
        let c: V3 = [
            0.05 + 0.6 * (noise(i * 3) * 0.5 + 0.5),
            0.05 + 0.6 * (noise(i * 3 + 1) * 0.5 + 0.5),
            0.05 + 0.6 * (noise(i * 3 + 2) * 0.5 + 0.5),
        ];
        // Front-end camera values (as-shot white balance). The DNG reader white-balances
        // with the photo's own neutral, so its ForwardMatrix sees these values as they are.
        let cam: [f32; 3] = std::array::from_fn(|k| c[k] as f32);
        let got = prof.apply(cam);
        let want = colour::mulv(
            &to709,
            solve::forward_prophoto(&fit.fm, fit.table.as_ref(), c),
        );
        for k in 0..3 {
            worst = worst.max((f64::from(got[k]) - want[k]).abs());
        }
    }
    eprintln!("DCP round trip: max |reader - solver| {worst:.2e}");
    assert!(worst < 1e-4, "{worst}");
    // The ColorMatrix gives back the chart's raw neutral for the light's white.
    let cm = back.color_matrix[0].unwrap();
    let cm: M3 = std::array::from_fn(|r| [cm[3 * r], cm[3 * r + 1], cm[3 * r + 2]]);
    let n = colour::mulv(&cm, colour::xy_to_xyz([0.3457, 0.3585]));
    for k in 0..3 {
        assert!(
            (n[k] / n[1] - raw_neutral[k]).abs() < 1e-4,
            "{n:?} vs {raw_neutral:?}"
        );
    }
}

#[test]
fn mixer_and_cube_for_other_images() {
    let l = checker();
    // Linear Rec.2020 "camera" with the twist: the mixer alone gets the matrix part.
    let s = camera_samples(&l, true, 0.0, [1.0; 3]);
    let fit = solve::solve(&Input {
        layout: &l,
        samples: &s,
        before: colour::rec2020_to_xyz(),
        table: true,
    })
    .unwrap();
    let m = output::mixer(&fit);
    let m: M3 = std::array::from_fn(|r| [m[3 * r], m[3 * r + 1], m[3 * r + 2]].map(f64::from));
    // Neutral in → neutral (Rec.2020 white) out.
    let g = colour::mulv(&m, fit.neutral);
    assert!(
        (g[0] - g[1]).abs() < 1e-5 && (g[2] - g[1]).abs() < 1e-5,
        "{g:?}"
    );
    let text = output::cube(&fit, output::CubeModel::Table);
    let dir = std::env::temp_dir().join(format!("specular-chart-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t.cube");
    std::fs::write(&path, &text).unwrap();
    let ops = specular_ocio::files::read(&path, "tetrahedral").unwrap();
    assert!(!ops.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
    // Through mixer and cube, the patches land near their references.
    let (to_ap1, from_ap1) = (
        crate::ops::colour::lut::LutSpace::Acescct.to_primaries(),
        colour::inv(&crate::ops::colour::lut::LutSpace::Acescct.to_primaries()).unwrap(),
    );
    let mut de = Vec::new();
    for (i, p) in l.patches.iter().enumerate() {
        let x = colour::mulv(&m, s[i].rgb.map(|v| v * fit.exposure));
        let e = colour::mulv(&to_ap1, x).map(output::linear_to_acescct);
        let mut px = e;
        for op in &ops {
            op.apply(&mut px);
        }
        let y = colour::mulv(&from_ap1, px.map(output::acescct_to_linear));
        let lab = colour::xyz_to_lab(colour::mulv(&colour::rec2020_to_xyz(), y));
        de.push(colour::delta_e2000(lab, p.reference));
    }
    let mean = de.iter().sum::<f64>() / de.len() as f64;
    eprintln!(
        "mixer + cube: mean ΔE2000 {mean:.2} (matrix {:.2})",
        fit.report.matrix.mean
    );
    assert!(mean < fit.report.matrix.mean && mean < 1.5, "{mean}");
}

/// The real IT8.7/2 card of the rawrest corpus (Phase One H 25, no batch reference file):
/// the chart found from its four corners and sampled; the spread of the patches is
/// reported. `SPECULAR_IT8_CORNERS` gives the corner clicks (picture pixels of the half-size
/// develop, `x,y;x,y;x,y;x,y`); `SPECULAR_IT8_PPM` writes the develop for looking.
///
///     SPECULAR_RAWREST_CORPUS=<dir> cargo test -p specular-develop --lib \
///       chart::tests::rawrest_it8_card_is_found_and_sampled -- --ignored --nocapture
#[test]
#[ignore = "needs the rawrest corpus"]
fn rawrest_it8_card_is_found_and_sampled() {
    let Some(dir) = std::env::var_os("SPECULAR_RAWREST_CORPUS") else {
        return;
    };
    let path = std::path::Path::new(&dir).join("Phase One/H 25/H25_IT8.7-2_Card.TIF");
    let file = std::fs::read(path).unwrap();
    let img = specular_raw::parse(&file)
        .unwrap()
        .decode(&file, &specular_raw::DecodeOptions::default())
        .unwrap();
    // Half size, linear (the develop's output is sRGB-encoded 16-bit).
    let (w, h) = (img.width / 2, img.height / 2);
    let mut px = vec![0f32; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            for c in 0..3 {
                let mut s = 0.0;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let j = (((2 * y + dy) * img.width + 2 * x + dx) * 3) as usize + c;
                    s += specular_color::math::srgb_to_linear(f32::from(img.data[j]) / 65535.0);
                }
                px[i + c] = s / 4.0;
            }
            px[i + 3] = 1.0;
        }
    }
    if let Some(out) = std::env::var_os("SPECULAR_IT8_PPM") {
        let mut b = format!("P6\n{w} {h}\n255\n").into_bytes();
        for p in px.chunks_exact(4) {
            for v in &p[..3] {
                b.push((specular_color::math::linear_to_srgb(*v).clamp(0.0, 1.0) * 255.0) as u8);
            }
        }
        std::fs::write(out, b).unwrap();
    }
    let Some(corners) = std::env::var("SPECULAR_IT8_CORNERS").ok() else {
        eprintln!("picture {w}x{h}; set SPECULAR_IT8_CORNERS to locate");
        return;
    };
    let c: Vec<(f64, f64)> = corners
        .split(';')
        .map(|p| {
            let mut it = p.split(',').map(|v| v.trim().parse::<f64>().unwrap());
            (it.next().unwrap(), it.next().unwrap())
        })
        .collect();
    let mut ids: Vec<String> = (0..12u8)
        .flat_map(|r| (1..=22).map(move |k| format!("{}{k:02}", (b'A' + r) as char)))
        .collect();
    ids.extend((0..24).map(|g| format!("GS{g}")));
    let r = Reference {
        title: String::new(),
        patches: ids
            .into_iter()
            .map(|id| RefPatch {
                id,
                lab: [50.0, 0.0, 0.0],
            })
            .collect(),
    };
    let l = Layout::new(ChartKind::It8, &r).unwrap();
    let pic = Picture { w, h, rgba: &px };
    let at = sample::locate(&l, &pic, [c[0], c[1], c[2], c[3]]).unwrap();
    let s = sample::sample(&l, &pic, &at, [0.99; 3]);
    let mut spreads: Vec<f64> = s.iter().map(|x| x.spread).collect();
    spreads.sort_by(f64::total_cmp);
    let flagged: Vec<&str> = s
        .iter()
        .zip(&l.patches)
        .filter(|(x, _)| x.flag.is_some())
        .map(|(_, p)| p.id.as_str())
        .collect();
    let gs_moved = l
        .patches
        .iter()
        .zip(&at.centres)
        .filter(|(p, _)| p.id.starts_with("GS"))
        .map(|(p, c)| {
            let want = at.grid_to_picture.apply(p.at).unwrap();
            (want.0 - c.0).hypot(want.1 - c.1)
        })
        .fold(0f64, f64::max);
    eprintln!(
        "rawrest IT8: {} patches, spread median {:.3} p90 {:.3} max {:.3}; flagged {:?}; grey row snapped up to {gs_moved:.1} px",
        s.len(),
        spreads[spreads.len() / 2],
        spreads[spreads.len() * 9 / 10],
        spreads[spreads.len() - 1],
        flagged
    );
    if let Some(out) = std::env::var_os("SPECULAR_IT8_CENTRES") {
        let t: String = at
            .centres
            .iter()
            .map(|c| format!("{:.1} {:.1}\n", c.0, c.1))
            .collect();
        std::fs::write(out, t).unwrap();
    }
}

/// colour-science (BSD) as the oracle: Cheung 2004 (3 terms) and Finlayson 2015
/// root-polynomial (degree 2) least squares, and CIEDE2000, on the same patches.
///
///     tools/quality/setup-venv.sh   # or any venv with colour-science 0.4.7
///     SPECULAR_CHART_PY=<venv>/bin/python cargo test -p specular-develop --lib \
///       chart::tests::fits_match_colour_science -- --ignored --nocapture
#[test]
#[ignore = "needs SPECULAR_CHART_PY (a python with colour-science)"]
fn fits_match_colour_science() {
    let Some(py) = std::env::var_os("SPECULAR_CHART_PY") else {
        return;
    };
    let l = checker();
    let s = camera_samples(&l, true, 0.003, WB);
    let rgb: Vec<V3> = s.iter().map(|p| p.rgb).collect();
    let xyz: Vec<V3> = l
        .patches
        .iter()
        .map(|p| colour::lab_to_xyz(p.reference))
        .collect();
    let pairs: Vec<[Lab; 2]> = (0..24)
        .map(|i| {
            let a = l.patches[i].reference;
            let b = colour::xyz_to_lab(colour::mulv(&CAMERA, xyz[i]));
            [a, b]
        })
        .collect();
    let dir = std::env::temp_dir().join(format!("specular-chart-oracle-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("in.json");
    let body = serde_json::json!({ "rgb": rgb, "xyz": xyz, "lab_pairs": pairs });
    std::fs::write(&input, body.to_string()).unwrap();
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/quality/chart_oracle.py"
    );
    let out = std::process::Command::new(py)
        .arg("-I")
        .arg(script)
        .arg(&input)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let mat = |k: &str| -> Vec<Vec<f64>> { serde_json::from_value(v[k].clone()).unwrap() };
    let cheung = mat("cheung");
    let ours = solve::matrix_least_squares(&rgb, &xyz).unwrap();
    let mut worst = 0f64;
    for r in 0..3 {
        for c in 0..3 {
            worst = worst.max((cheung[r][c] - ours[r][c]).abs());
        }
    }
    let root = mat("root2");
    let ours_root = solve::root_polynomial(&rgb, &xyz).unwrap();
    let terms = mat("root2_terms");
    let our_terms = solve::root_terms(rgb[0]);
    let mut worst_root = 0f64;
    for r in 0..3 {
        for k in 0..6 {
            worst_root = worst_root.max((root[r][k] - ours_root[r][k]).abs());
        }
    }
    let worst_terms = (0..6)
        .map(|k| (terms[0][k] - our_terms[k]).abs())
        .fold(0f64, f64::max);
    let de: Vec<f64> = serde_json::from_value(v["de2000"].clone()).unwrap();
    let worst_de = pairs
        .iter()
        .zip(&de)
        .map(|(p, d)| (colour::delta_e2000(p[0], p[1]) - d).abs())
        .fold(0f64, f64::max);
    let over = |w: f64| usize::from(w > 1e-6);
    eprintln!(
        "colour-science: Cheung 2004 max |diff| {worst:.1e}, root-polynomial {worst_root:.1e} \
         (terms {worst_terms:.1e}), CIEDE2000 {worst_de:.1e} over 24 pairs; mismatches {}",
        over(worst) + over(worst_root) + over(worst_terms) + over(worst_de)
    );
    assert!(worst < 1e-6 && worst_root < 1e-6 && worst_terms < 1e-12 && worst_de < 1e-6);
}
