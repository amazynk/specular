//! ICC conformance against Little CMS (the reference CMM), reported as ΔE2000.
//!
//! - `synthetic_profiles_match_lcms` (fast, always runs): profiles written here byte by byte
//!   (matrix/TRC with every `para` function type and `curv` tables, gray on XYZ and Lab PCS,
//!   `mft1`, `mft2`, `mAB `/`mBA ` on Lab and XYZ PCS, RGB and CMYK, v2 and v4, with A2B0/1/2
//!   that differ so the wrong intent's table shows) plus the committed fuzz seeds.
//! - `icc_corpus_conformance` (`--ignored`): every profile in `tools/corpus/quality/icc`
//!   (`tools/quality/fetch-icc.sh`: the ICC registry's v2/v4 sRGB, probe, ROMM/RIMM, BT.601/
//!   709/2020, DCI-P3, XYZ and PRMG profiles, the Little CMS testbed) and the system's
//!   `/usr/share/color/icc`; prints the table `tools/quality/README.md` describes.
//!
//! Directions measured, each per intent (perceptual, relative, saturation, absolute):
//! - `fwd`: device → L*a*b* D50, ΔE00 against Little CMS on a device grid;
//! - `inv`: L*a*b* → device (B2A or inverse matrix/TRC), both results taken back to L*a*b*
//!   through Little CMS so the error is visual;
//! - `gpu`: the shipped path for RGB/gray images, the baked [`SourceLut`] (curves or 33³
//!   cube, half floats, interpolated like the shader) into sRGB, against Little CMS's
//!   profile → sRGB with black point compensation;
//! - `disp`: the display side, [`Display`] (curves or cube) from sRGB-encoded BT.709 into
//!   the profile as a monitor profile.

use specular_color::gpu::{CUBE_SIZE, CURVE_SIZE, Display, SourceLut, source_lut_for};
use specular_color::math::{
    bt709_to_pcs, delta_e2000, f16_to_f32, linear_to_srgb, mul3, srgb_to_linear, xyz_to_lab,
};
use specular_color::{ColorSpace, Intent, Profile, Transform, profiles};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
struct Stats {
    v: Vec<f32>,
    /// Where the largest difference was (input, ours, reference).
    worst: String,
}

impl Stats {
    fn add(&mut self, de: f32, at: impl FnOnce() -> String) {
        let de = if de.is_finite() { de } else { 999.0 };
        if de > self.max() {
            self.worst = at();
        }
        self.v.push(de);
    }
    fn n(&self) -> usize {
        self.v.len()
    }
    fn mean(&self) -> f32 {
        self.v.iter().sum::<f32>() / self.v.len().max(1) as f32
    }
    fn max(&self) -> f32 {
        self.v.iter().copied().fold(0.0, f32::max)
    }
    fn p99(&self) -> f32 {
        if self.v.is_empty() {
            return 0.0;
        }
        let mut s = self.v.clone();
        s.sort_by(f32::total_cmp);
        s[((s.len() - 1) as f32 * 0.99).round() as usize]
    }
}

// ---------------------------------------------------------------------------------------
// Grids and conversions
// ---------------------------------------------------------------------------------------

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

/// A device grid plus deterministic random points (all values exact in 16 bits).
/// Grid plus pseudo-random samples; `quick` keeps the default (synthetic) test fast.
fn device_samples(ch: usize, quick: bool) -> Vec<Vec<f32>> {
    let (steps, extra) = match (ch, quick) {
        (1, false) => (256, 0),
        (1, true) => (64, 0),
        (3, false) => (9, 1500),
        (3, true) => (6, 200),
        (_, false) => (6, 1500),
        (_, true) => (4, 200),
    };
    let mut v = grid(ch, steps);
    let mut seed = 0x1234_5678u32;
    for _ in 0..extra {
        v.push(
            (0..ch)
                .map(|_| {
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    (seed >> 16) as f32 / 65535.0
                })
                .collect(),
        );
    }
    v
}

fn lab_norm(lab: [f64; 3]) -> [f32; 3] {
    [
        (lab[0] / 100.0) as f32,
        ((lab[1] + 128.0) / 255.0) as f32,
        ((lab[2] + 128.0) / 255.0) as f32,
    ]
}

fn lab_denorm(v: &[f32]) -> [f32; 3] {
    [v[0] * 100.0, v[1] * 255.0 - 128.0, v[2] * 255.0 - 128.0]
}

/// XYZ device values in our normalization (u1Fixed15: 1.0 ↔ 65535/32768).
const XYZ_SCALE: f64 = 65535.0 / 32768.0;

fn srgb_lab(v: [f32; 3]) -> [f32; 3] {
    let l = v.map(|x| srgb_to_linear(x.clamp(0.0, 1.0)));
    xyz_to_lab(mul3(&bt709_to_pcs().to_f32(), l))
}

// ---------------------------------------------------------------------------------------
// Little CMS side
// ---------------------------------------------------------------------------------------

fn lintent(i: Intent) -> lcms2::Intent {
    match i {
        Intent::Perceptual => lcms2::Intent::Perceptual,
        Intent::Relative => lcms2::Intent::RelativeColorimetric,
        Intent::Saturation => lcms2::Intent::Saturation,
        Intent::Absolute => lcms2::Intent::AbsoluteColorimetric,
    }
}

fn lab_d50() -> lcms2::Profile {
    let d50 = specular_color::math::D50;
    let sum: f64 = d50.iter().map(|&v| v as f64).sum();
    let white = lcms2::CIExyY {
        x: d50[0] as f64 / sum,
        y: d50[1] as f64 / sum,
        Y: 1.0,
    };
    lcms2::Profile::new_lab4_context(lcms2::GlobalContext::new(), &white).unwrap()
}

/// Device values as Little CMS doubles for a colour space (CMYK in percent, XYZ unscaled,
/// L*a*b* in L*/a*/b*), or `None` for spaces this runner doesn't drive.
fn lcms_format(cs: ColorSpace) -> Option<(lcms2::PixelFormat, usize)> {
    Some(match cs {
        ColorSpace::Rgb => (lcms2::PixelFormat::RGB_DBL, 3),
        ColorSpace::Gray => (lcms2::PixelFormat::GRAY_DBL, 1),
        ColorSpace::Cmyk => (lcms2::PixelFormat::CMYK_DBL, 4),
        ColorSpace::Lab => (lcms2::PixelFormat::Lab_DBL, 3),
        ColorSpace::Xyz => (lcms2::PixelFormat::XYZ_DBL, 3),
        _ => return None,
    })
}

fn to_lcms(cs: ColorSpace, v: &[f32]) -> Vec<f64> {
    match cs {
        ColorSpace::Cmyk => v.iter().map(|&x| x as f64 * 100.0).collect(),
        ColorSpace::Lab => lab_denorm(v).iter().map(|&x| x as f64).collect(),
        ColorSpace::Xyz => v.iter().map(|&x| x as f64 * XYZ_SCALE).collect(),
        _ => v.iter().map(|&x| x as f64).collect(),
    }
}

fn from_lcms(cs: ColorSpace, v: &[f64]) -> Vec<f32> {
    match cs {
        ColorSpace::Cmyk => v.iter().map(|&x| (x / 100.0) as f32).collect(),
        ColorSpace::Lab => lab_norm([v[0], v[1], v[2]]).to_vec(),
        ColorSpace::Xyz => v.iter().map(|&x| (x / XYZ_SCALE) as f32).collect(),
        _ => v.iter().map(|&x| x as f32).collect(),
    }
}

/// Runs a Little CMS transform on flat f64 pixels (`ni` / `no` values each).
#[allow(clippy::too_many_arguments)]
fn lcms_run(
    src: &lcms2::Profile,
    fin: (lcms2::PixelFormat, usize),
    dst: &lcms2::Profile,
    fout: (lcms2::PixelFormat, usize),
    intent: lcms2::Intent,
    bpc: bool,
    input: &[f64],
) -> Option<Vec<f64>> {
    fn go<const N: usize, const M: usize>(
        src: &lcms2::Profile,
        fin: lcms2::PixelFormat,
        dst: &lcms2::Profile,
        fout: lcms2::PixelFormat,
        intent: lcms2::Intent,
        bpc: bool,
        input: &[f64],
    ) -> Option<Vec<f64>> {
        let flags = if bpc {
            lcms2::Flags::NO_CACHE
                | lcms2::Flags::NO_OPTIMIZE
                | lcms2::Flags::BLACKPOINT_COMPENSATION
        } else {
            lcms2::Flags::NO_CACHE | lcms2::Flags::NO_OPTIMIZE
        };
        let t: lcms2::Transform<[f64; N], [f64; M], _, _> =
            lcms2::Transform::new_flags(src, fin, dst, fout, intent, flags).ok()?;
        let px: Vec<[f64; N]> = input
            .chunks_exact(N)
            .map(|c| std::array::from_fn(|i| c[i]))
            .collect();
        let mut out = vec![[0f64; M]; px.len()];
        t.transform_pixels(&px, &mut out);
        Some(out.into_iter().flatten().collect())
    }
    match (fin.1, fout.1) {
        (1, 3) => go::<1, 3>(src, fin.0, dst, fout.0, intent, bpc, input),
        (3, 3) => go::<3, 3>(src, fin.0, dst, fout.0, intent, bpc, input),
        (4, 3) => go::<4, 3>(src, fin.0, dst, fout.0, intent, bpc, input),
        (3, 1) => go::<3, 1>(src, fin.0, dst, fout.0, intent, bpc, input),
        (3, 4) => go::<3, 4>(src, fin.0, dst, fout.0, intent, bpc, input),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------
// The measurements
// ---------------------------------------------------------------------------------------

const INTENTS: [Intent; 4] = [
    Intent::Perceptual,
    Intent::Relative,
    Intent::Saturation,
    Intent::Absolute,
];

#[derive(Debug)]
struct Row {
    profile: String,
    kind: String,
    dir: &'static str,
    intent: Intent,
    stats: Stats,
}

fn kind(p: &Profile) -> String {
    let class = String::from_utf8_lossy(&p.class).to_string();
    let lut = if p.a2b.iter().any(Option::is_some) {
        "LUT".to_string()
    } else if p.matrix.is_some() {
        "matrix/TRC".into()
    } else {
        "TRC".into()
    };
    format!(
        "v{} {class} {:?}/{} {lut}",
        p.version,
        p.color_space,
        if p.pcs_lab { "Lab" } else { "XYZ" }
    )
}

/// Every direction for one profile. `Err` when one side refuses the profile.
fn measure(name: &str, data: &[u8], quick: bool) -> Result<Vec<Row>, String> {
    let lp = lcms2::Profile::new_icc(data).map_err(|e| format!("lcms refuses: {e}"))?;
    let p = Profile::parse(data).map_err(|e| format!("ours refuses: {e}"))?;
    let cs = p.color_space;
    let fmt = lcms_format(cs).ok_or_else(|| format!("colour space {cs:?} not driven"))?;
    let k = kind(&p);
    let lab = profiles::lab();
    let llab = lab_d50();
    let lab_fmt = (lcms2::PixelFormat::Lab_DBL, 3);
    let mut rows = Vec::new();
    let input = device_samples(fmt.1, quick);
    let flat: Vec<f64> = input.iter().flat_map(|v| to_lcms(cs, v)).collect();
    // Relative device → Lab through Little CMS, to judge device values visually.
    let dev_to_lab = |dev: &[f64]| {
        lcms_run(
            &lp,
            fmt,
            &llab,
            lab_fmt,
            lcms2::Intent::RelativeColorimetric,
            false,
            dev,
        )
    };
    // Inverse-direction inputs: the profile's own colours (relative), so the B2A tables are
    // judged inside their gamut rather than on how they clip.
    let labs: Vec<[f64; 3]> = dev_to_lab(&flat)
        .map(|v| v.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect())
        .unwrap_or_default();
    for intent in INTENTS {
        // fwd: device → Lab.
        if let (Ok(t), Some(want)) = (
            Transform::new(&p, &lab, intent, false),
            lcms_run(&lp, fmt, &llab, lab_fmt, lintent(intent), false, &flat),
        ) {
            let mut s = Stats::default();
            let mut o = [0f32; 3];
            for (v, w) in input.iter().zip(want.chunks_exact(3)) {
                t.eval(v, &mut o);
                let w = [w[0] as f32, w[1] as f32, w[2] as f32];
                s.add(delta_e2000(lab_denorm(&o), w), || {
                    format!("{v:.4?} → {:.3?} want {w:.3?}", lab_denorm(&o))
                });
            }
            rows.push(Row {
                profile: name.into(),
                kind: k.clone(),
                dir: "fwd",
                intent,
                stats: s,
            });
        }
        // inv: Lab → device.
        if !matches!(cs, ColorSpace::Lab | ColorSpace::Xyz)
            && let (Ok(t), Some(want)) = (
                Transform::new(&lab, &p, intent, false),
                lcms_run(
                    &llab,
                    lab_fmt,
                    &lp,
                    fmt,
                    lintent(intent),
                    false,
                    &labs.concat(),
                ),
            )
        {
            let n = fmt.1;
            let mut ours = Vec::new();
            let mut o = vec![0f32; n];
            for &l in &labs {
                t.eval(&lab_norm(l), &mut o);
                ours.extend(to_lcms(
                    cs,
                    &o.iter().map(|v| v.clamp(0.0, 1.0)).collect::<Vec<_>>(),
                ));
            }
            let want_c: Vec<f64> = want
                .chunks_exact(n)
                .flat_map(|c| {
                    let v: Vec<f32> = from_lcms(cs, c).iter().map(|v| v.clamp(0.0, 1.0)).collect();
                    to_lcms(cs, &v)
                })
                .collect();
            if let (Some(a), Some(b)) = (dev_to_lab(&ours), dev_to_lab(&want_c)) {
                let mut s = Stats::default();
                for (i, (x, y)) in a.chunks_exact(3).zip(b.chunks_exact(3)).enumerate() {
                    let (x, y) = (
                        [x[0] as f32, x[1] as f32, x[2] as f32],
                        [y[0] as f32, y[1] as f32, y[2] as f32],
                    );
                    s.add(delta_e2000(x, y), || {
                        format!(
                            "{:?} → {:.4?} want {:.4?}",
                            labs[i],
                            &ours[i * n..i * n + n],
                            &want_c[i * n..i * n + n]
                        )
                    });
                }
                rows.push(Row {
                    profile: name.into(),
                    kind: k.clone(),
                    dir: "inv",
                    intent,
                    stats: s,
                });
            }
        }
        // gpu: the shipped source path (RGB/gray images), BPC on as the default policy.
        if matches!(cs, ColorSpace::Rgb | ColorSpace::Gray) && intent != Intent::Absolute {
            let lsrgb = lcms2::Profile::new_srgb();
            let srgb_fmt = (lcms2::PixelFormat::RGB_DBL, 3);
            if let Some(want) = lcms_run(&lp, fmt, &lsrgb, srgb_fmt, lintent(intent), true, &flat) {
                let lut = source_lut_for(&p, intent, true);
                let mut s = Stats::default();
                for (v, w) in input.iter().zip(want.chunks_exact(3)) {
                    let rgb = if v.len() == 1 {
                        [v[0]; 3]
                    } else {
                        [v[0], v[1], v[2]]
                    };
                    let lin = match &lut {
                        Some(l) => eval_source_lut(l, rgb),
                        None => rgb.map(srgb_to_linear),
                    };
                    let enc = lin.map(|x| linear_to_srgb(x.clamp(0.0, 1.0)));
                    let w = [w[0] as f32, w[1] as f32, w[2] as f32];
                    s.add(delta_e2000(srgb_lab(enc), srgb_lab(w)), || {
                        format!("{v:.4?} → {enc:.4?} want {w:.4?}")
                    });
                }
                rows.push(Row {
                    profile: name.into(),
                    kind: k.clone(),
                    dir: "gpu",
                    intent,
                    stats: s,
                });
            }
        }
        // disp: the profile as the monitor profile.
        if cs == ColorSpace::Rgb && intent != Intent::Absolute {
            let d = Display::for_profile(&p, intent, true);
            let lsrgb = lcms2::Profile::new_srgb();
            let srgb_fmt = (lcms2::PixelFormat::RGB_DBL, 3);
            let ins = grid(3, if quick { 6 } else { 9 });
            let sflat: Vec<f64> = ins.iter().flatten().map(|&x| x as f64).collect();
            if let Some(want) = lcms_run(&lsrgb, srgb_fmt, &lp, fmt, lintent(intent), true, &sflat)
            {
                let mut ours = Vec::new();
                for v in &ins {
                    let lin = [v[0], v[1], v[2]].map(srgb_to_linear);
                    ours.extend(d.apply(lin).map(|x| x.clamp(0.0, 1.0) as f64));
                }
                let want_c: Vec<f64> = want.iter().map(|v| v.clamp(0.0, 1.0)).collect();
                if let (Some(a), Some(b)) = (dev_to_lab(&ours), dev_to_lab(&want_c)) {
                    let mut s = Stats::default();
                    for (i, (x, y)) in a.chunks_exact(3).zip(b.chunks_exact(3)).enumerate() {
                        let (x, y) = (
                            [x[0] as f32, x[1] as f32, x[2] as f32],
                            [y[0] as f32, y[1] as f32, y[2] as f32],
                        );
                        s.add(delta_e2000(x, y), || {
                            format!(
                                "{:.4?} → {:.4?} want {:.4?}",
                                ins[i],
                                &ours[i * 3..i * 3 + 3],
                                &want_c[i * 3..i * 3 + 3]
                            )
                        });
                    }
                    rows.push(Row {
                        profile: name.into(),
                        kind: k.clone(),
                        dir: "disp",
                        intent,
                        stats: s,
                    });
                }
            }
        }
    }
    Ok(rows)
}

/// CPU model of `convert.wgsl`'s `source()`: texel-centred linear filtering of the half-float
/// tables.
fn eval_source_lut(l: &SourceLut, rgb: [f32; 3]) -> [f32; 3] {
    match l {
        SourceLut::Curves { data, matrix } => {
            let c: [f32; 3] = std::array::from_fn(|ch| {
                let p = rgb[ch].clamp(0.0, 1.0) * (CURVE_SIZE - 1) as f32;
                let i = (p as usize).min(CURVE_SIZE - 2);
                let f = p - i as f32;
                let a = f16_to_f32(data[i * 4 + ch]);
                let b = f16_to_f32(data[(i + 1) * 4 + ch]);
                a + (b - a) * f
            });
            std::array::from_fn(|r| {
                matrix[r * 4] * c[0]
                    + matrix[r * 4 + 1] * c[1]
                    + matrix[r * 4 + 2] * c[2]
                    + matrix[r * 4 + 3]
            })
        }
        SourceLut::Cube { data } => {
            let n = CUBE_SIZE;
            let p = rgb.map(|v| v.clamp(0.0, 1.0) * (n - 1) as f32);
            let i = p.map(|v| (v as usize).min(n - 2));
            let f = [p[0] - i[0] as f32, p[1] - i[1] as f32, p[2] - i[2] as f32];
            let mut out = [0f32; 3];
            for (c, o) in out.iter_mut().enumerate() {
                let mut acc = 0.0;
                for corner in 0..8 {
                    let d = [corner & 1, (corner >> 1) & 1, (corner >> 2) & 1];
                    let w: f32 = (0..3)
                        .map(|k| if d[k] == 1 { f[k] } else { 1.0 - f[k] })
                        .product();
                    let idx = ((i[2] + d[2]) * n + i[1] + d[1]) * n + i[0] + d[0];
                    acc += w * f16_to_f32(data[idx * 4 + c]);
                }
                *o = acc.max(0.0);
            }
            out
        }
    }
}

fn table(rows: &[Row]) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "| profile | kind | dir | intent | n | mean ΔE00 | p99 | max | worst at |\n|---|---|---|---|---|---|---|---|---|"
    );
    for r in rows {
        let _ = writeln!(
            s,
            "| {} | {} | {} | {:?} | {} | {:.4} | {:.4} | {:.4} | {} |",
            r.profile,
            r.kind,
            r.dir,
            r.intent,
            r.stats.n(),
            r.stats.mean(),
            r.stats.p99(),
            r.stats.max(),
            if r.stats.max() > 0.1 {
                r.stats.worst.as_str()
            } else {
                ""
            }
        );
    }
    s
}

// ---------------------------------------------------------------------------------------
// Synthetic profiles, written byte by byte
// ---------------------------------------------------------------------------------------

fn s15(v: f64) -> [u8; 4] {
    ((v * 65536.0).round() as i32).to_be_bytes()
}

fn xyz_tag(v: [f64; 3]) -> Vec<u8> {
    let mut b = b"XYZ \0\0\0\0".to_vec();
    for x in v {
        b.extend(s15(x));
    }
    b
}

fn para(kind: u16, params: &[f64]) -> Vec<u8> {
    let mut b = b"para\0\0\0\0".to_vec();
    b.extend(kind.to_be_bytes());
    b.extend([0, 0]);
    for &p in params {
        b.extend(s15(p));
    }
    b
}

fn curv_table(f: impl Fn(f64) -> f64, n: usize) -> Vec<u8> {
    let mut b = b"curv\0\0\0\0".to_vec();
    b.extend((n as u32).to_be_bytes());
    for i in 0..n {
        let y = f(i as f64 / (n - 1) as f64).clamp(0.0, 1.0);
        b.extend(((y * 65535.0).round() as u16).to_be_bytes());
    }
    b
}

fn curv_gamma(g: f64) -> Vec<u8> {
    let mut b = b"curv\0\0\0\0".to_vec();
    b.extend(1u32.to_be_bytes());
    b.extend(((g * 256.0).round() as u16).to_be_bytes());
    b
}

fn pad4(b: &mut Vec<u8>) {
    while !b.len().is_multiple_of(4) {
        b.push(0);
    }
}

/// Smooth, non-separable test function: device values → PCS-encoded values in [0, 1].
fn shade(v: &[f64], outputs: usize, twist: f64) -> Vec<f64> {
    let n = v.len();
    let mean = v.iter().sum::<f64>() / n as f64;
    let ink = n == 4;
    let l = if ink {
        // Subtractive: more ink is darker; K dominates.
        let c = (v[0] * 0.3 + v[1] * 0.45 + v[2] * 0.15) * (1.0 - 0.6 * v[3]) + 0.6 * v[3];
        0.97 - 0.9 * c.clamp(0.0, 1.0).powf(0.9)
    } else {
        0.02 + 0.95 * (0.25 * v[0] + 0.65 * v[1] + 0.1 * v[2]).powf(0.85 + 0.1 * twist)
    };
    let (a, b) = if ink {
        (
            0.5 + 0.25 * (v[1] - v[0] * 0.6) * (1.0 - v[3]),
            0.5 + 0.25 * (v[2] - v[1] * 0.4) * (1.0 - v[3]),
        )
    } else {
        (
            0.5 + 0.3 * (v[0] - v[1]) * (1.0 - 0.3 * mean) + 0.04 * twist,
            0.5 + 0.3 * (v[1] - v[2]) * (0.7 + 0.3 * v[0]) - 0.03 * twist,
        )
    };
    let lab = [l, a.clamp(0.0, 1.0), b.clamp(0.0, 1.0)];
    lab[..outputs].to_vec()
}

/// Lab (v4-normalized) → an XYZ-PCS encoding (u1Fixed15 / 1.99997).
fn lab_to_xyz_enc(v: &[f64]) -> Vec<f64> {
    let lab = [
        (v[0] * 100.0) as f32,
        (v[1] * 255.0 - 128.0) as f32,
        (v[2] * 255.0 - 128.0) as f32,
    ];
    specular_color::math::lab_to_xyz(lab)
        .iter()
        .map(|&x| (x as f64 / XYZ_SCALE).clamp(0.0, 1.0))
        .collect()
}

fn clut_values(ni: usize, g: usize, no: usize, f: &dyn Fn(&[f64]) -> Vec<f64>) -> Vec<f64> {
    let mut out = Vec::new();
    let total = g.pow(ni as u32);
    for idx in 0..total {
        // First input slowest.
        let mut v = vec![0.0; ni];
        let mut r = idx;
        for d in (0..ni).rev() {
            v[d] = (r % g) as f64 / (g - 1) as f64;
            r /= g;
        }
        let o = f(&v);
        assert_eq!(o.len(), no);
        out.extend(o);
    }
    out
}

/// `mft2` (wide) or `mft1` with non-linear input and output tables.
fn mft(wide: bool, ni: usize, no: usize, g: usize, f: &dyn Fn(&[f64]) -> Vec<f64>) -> Vec<u8> {
    let mut b = if wide { b"mft2" } else { b"mft1" }.to_vec();
    b.extend([0, 0, 0, 0, ni as u8, no as u8, g as u8, 0]);
    for i in 0..9 {
        b.extend(s15(if i % 4 == 0 { 1.0 } else { 0.0 }));
    }
    let (nin, nout) = if wide {
        (256usize, 512usize)
    } else {
        (256, 256)
    };
    if wide {
        b.extend((nin as u16).to_be_bytes());
        b.extend((nout as u16).to_be_bytes());
    }
    let put = |b: &mut Vec<u8>, v: f64| {
        let v = v.clamp(0.0, 1.0);
        if wide {
            b.extend(((v * 65535.0).round() as u16).to_be_bytes());
        } else {
            b.push((v * 255.0).round() as u8);
        }
    };
    for _ in 0..ni {
        for i in 0..nin {
            let x = i as f64 / (nin - 1) as f64;
            put(&mut b, x.powf(0.8));
        }
    }
    // The CLUT sees the input tables' output: invert them in the function.
    let fi = |v: &[f64]| f(&v.iter().map(|x| x.powf(1.25)).collect::<Vec<_>>());
    for v in clut_values(ni, g, no, &fi) {
        // Output tables are x^1.1 below; pre-invert.
        put(&mut b, v.powf(1.0 / 1.1));
    }
    for _ in 0..no {
        for i in 0..nout {
            let x = i as f64 / (nout - 1) as f64;
            put(&mut b, x.powf(1.1));
        }
    }
    b
}

/// `mAB ` (device → PCS: A curves, CLUT, M curves, matrix, B curves) or `mBA ` (PCS →
/// device: B curves, matrix, M curves, CLUT, A curves). `prec` is the CLUT precision (1, 2).
fn mab(
    a2b: bool,
    ni: usize,
    no: usize,
    g: usize,
    prec: u8,
    f: &dyn Fn(&[f64]) -> Vec<f64>,
) -> Vec<u8> {
    let mut b = if a2b { b"mAB " } else { b"mBA " }.to_vec();
    b.extend([0, 0, 0, 0, ni as u8, no as u8, 0, 0]);
    b.extend([0u8; 20]); // offsets, filled below
    let (dev, pcs) = if a2b { (ni, no) } else { (no, ni) };
    let curves = |b: &mut Vec<u8>, n: usize, k: usize| {
        for c in 0..n {
            let mut t = match (k + c) % 3 {
                0 => para(0, &[1.0 + 0.1 * c as f64]),
                1 => para(3, &[2.2, 0.95, 0.05, 0.08, 0.04]),
                _ => curv_table(|x| x.powf(0.9 + 0.05 * c as f64), 64),
            };
            pad4(&mut t);
            b.extend(t);
        }
    };
    let set = |b: &mut Vec<u8>, slot: usize, off: usize| {
        b[12 + slot * 4..16 + slot * 4].copy_from_slice(&(off as u32).to_be_bytes());
    };
    // B curves (PCS side): identity-ish para gamma 1.0 keeps the PCS meaning intact.
    let ob = b.len();
    for _ in 0..pcs {
        b.extend(para(0, &[1.0]));
    }
    set(&mut b, 0, ob);
    let mut m_off = 0;
    let mut cm_off = 0;
    if pcs == 3 {
        // Identity matrix with zero offsets, and gamma-1 M curves: exercises the parsing of
        // both elements without moving the PCS.
        m_off = b.len();
        for i in 0..9 {
            b.extend(s15(if i % 4 == 0 { 1.0 } else { 0.0 }));
        }
        for _ in 0..3 {
            b.extend(s15(0.0));
        }
        cm_off = b.len();
        for _ in 0..3 {
            b.extend(para(0, &[1.0]));
        }
    }
    set(&mut b, 1, m_off);
    set(&mut b, 2, cm_off);
    // A curves on the device side (non-linear: the CLUT is built against their output).
    let oa_curves_gamma: Vec<f64> = (0..dev).map(|c| 1.0 + 0.1 * c as f64).collect();
    let oclut = b.len();
    let mut gp = [0u8; 16];
    for x in gp.iter_mut().take(ni) {
        *x = g as u8;
    }
    b.extend(gp);
    b.extend([prec, 0, 0, 0]);
    let fwd = |v: &[f64]| -> Vec<f64> {
        if a2b {
            // CLUT input = A(x) = x^γ; CLUT(y) = f(y^(1/γ)).
            let x: Vec<f64> = v
                .iter()
                .zip(&oa_curves_gamma)
                .map(|(y, g)| y.powf(1.0 / g))
                .collect();
            f(&x)
        } else {
            // Output A curves are x^γ: pre-invert.
            f(v).iter()
                .zip(&oa_curves_gamma)
                .map(|(y, g)| y.clamp(0.0, 1.0).powf(1.0 / g))
                .collect()
        }
    };
    for v in clut_values(ni, g, no, &fwd) {
        let v = v.clamp(0.0, 1.0);
        if prec == 2 {
            b.extend(((v * 65535.0).round() as u16).to_be_bytes());
        } else {
            b.push((v * 255.0).round() as u8);
        }
    }
    pad4(&mut b);
    set(&mut b, 3, oclut);
    let oa = b.len();
    for &gm in &oa_curves_gamma {
        b.extend(para(0, &[gm]));
    }
    set(&mut b, 4, oa);
    let _ = curves; // non-trivial curves are exercised by the B side of other profiles
    b
}

/// Assembles a profile from tags.
fn icc(
    version: u8,
    class: &[u8; 4],
    cs: &[u8; 4],
    pcs: &[u8; 4],
    tags: Vec<(&[u8; 4], Vec<u8>)>,
) -> Vec<u8> {
    let mut h = vec![0u8; 128];
    h[8] = version;
    h[9] = if version >= 4 { 0x30 } else { 0x10 };
    h[12..16].copy_from_slice(class);
    h[16..20].copy_from_slice(cs);
    h[20..24].copy_from_slice(pcs);
    h[36..40].copy_from_slice(b"acsp");
    let d50 = specular_color::math::D50;
    for (i, v) in d50.iter().enumerate() {
        h[68 + i * 4..72 + i * 4].copy_from_slice(&s15(*v as f64));
    }
    let n = tags.len();
    let mut table = (n as u32).to_be_bytes().to_vec();
    let mut data = Vec::new();
    let base = 128 + 4 + 12 * n;
    for (sig, t) in &tags {
        let off = base + data.len();
        table.extend(*sig);
        table.extend((off as u32).to_be_bytes());
        table.extend((t.len() as u32).to_be_bytes());
        data.extend(t);
        pad4(&mut data);
    }
    let mut out = h;
    out.extend(table);
    out.extend(data);
    let len = out.len() as u32;
    out[0..4].copy_from_slice(&len.to_be_bytes());
    out
}

fn desc(version: u8, s: &str) -> Vec<u8> {
    if version >= 4 {
        let mut b = b"mluc\0\0\0\0".to_vec();
        b.extend(1u32.to_be_bytes());
        b.extend(12u32.to_be_bytes());
        b.extend(b"enUS");
        let u: Vec<u8> = s.encode_utf16().flat_map(|c| c.to_be_bytes()).collect();
        b.extend((u.len() as u32).to_be_bytes());
        b.extend(28u32.to_be_bytes());
        b.extend(u);
        b
    } else {
        let mut b = b"desc\0\0\0\0".to_vec();
        b.extend((s.len() as u32 + 1).to_be_bytes());
        b.extend(s.as_bytes());
        b.push(0);
        b.extend([0u8; 4 + 4 + 2 + 1 + 67]);
        b
    }
}

/// D50-adapted Display P3 colorants (as written by common P3 profiles).
const P3_COLS: [[f64; 3]; 3] = [
    [0.515_1, 0.241_2, -0.001_1],
    [0.292_0, 0.692_2, 0.041_9],
    [0.157_0, 0.066_6, 0.784_1],
];

fn matrix_profile(version: u8, name: &str, trcs: [Vec<u8>; 3]) -> Vec<u8> {
    let [r, g, b] = trcs;
    icc(
        version,
        b"mntr",
        b"RGB ",
        b"XYZ ",
        vec![
            (b"desc", desc(version, name)),
            (b"wtpt", xyz_tag([0.9642, 1.0, 0.8249])),
            (b"rXYZ", xyz_tag(P3_COLS[0])),
            (b"gXYZ", xyz_tag(P3_COLS[1])),
            (b"bXYZ", xyz_tag(P3_COLS[2])),
            (b"rTRC", r),
            (b"gTRC", g),
            (b"bTRC", b),
        ],
    )
}

/// A2B0/1/2 (and optionally B2A0/1/2) LUT profile; each intent's table is twisted
/// differently so a wrong-table pick shows up.
#[allow(clippy::type_complexity)]
fn lut_profile(
    version: u8,
    cs: &[u8; 4],
    pcs_lab: bool,
    name: &str,
    a2b: &dyn Fn(f64) -> Vec<u8>,
    b2a: Option<&dyn Fn(f64) -> Vec<u8>>,
) -> Vec<u8> {
    let mut tags: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"desc", desc(version, name)),
        (b"wtpt", xyz_tag([0.9642, 1.0, 0.8249])),
        (b"A2B0", a2b(0.0)),
        (b"A2B1", a2b(1.0)),
        (b"A2B2", a2b(-1.0)),
    ];
    if let Some(f) = b2a {
        tags.push((b"B2A0", f(0.0)));
        tags.push((b"B2A1", f(1.0)));
        tags.push((b"B2A2", f(-1.0)));
    }
    let class = if b2a.is_some() { b"prtr" } else { b"scnr" };
    icc(
        version,
        class,
        cs,
        if pcs_lab { b"Lab " } else { b"XYZ " },
        tags,
    )
}

/// Lab (normalized) → a smooth "inverse-ish" device colour, for B2A tables.
fn unshade(v: &[f64], outputs: usize, twist: f64) -> Vec<f64> {
    // Smooth and inside (0, 1) everywhere: kinks from clipping would make the result depend
    // on which tetrahedron each CMM picks at near-ties.
    let (l, a, b) = (v[0], v[1], v[2]);
    let w = 0.05 * twist;
    let rgb = [
        0.05 + 0.9 * ((0.7 - w) * l + (0.3 + w) * a),
        0.05 + 0.9 * (0.8 * l + 0.2 * (1.0 - a)),
        0.05 + 0.9 * ((0.7 + w) * l + (0.3 - w) * (1.0 - b)),
    ];
    if outputs == 3 {
        rgb.to_vec()
    } else {
        let mean = rgb.iter().sum::<f64>() / 3.0;
        let k = 0.8 * (1.0 - mean).powi(2);
        let mut c: Vec<f64> = rgb.iter().map(|x| (1.0 - x) * (1.0 - k) * 0.9).collect();
        c.push(k);
        c
    }
}

#[allow(clippy::vec_init_then_push)] // one profile per block reads best
fn synthetic_profiles() -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<(String, Vec<u8>)> = Vec::new();
    // Matrix/TRC: every parametric function type, v4.
    v.push((
        "rgb_v4_para_types_0_1_2".into(),
        matrix_profile(
            4,
            "para 0 1 2",
            [
                para(0, &[2.2]),
                para(1, &[2.4, 1.05, -0.05]),
                para(2, &[2.0, 0.97, 0.02, 0.01]),
            ],
        ),
    ));
    v.push((
        "rgb_v4_para_types_3_4".into(),
        matrix_profile(
            4,
            "para 3 4",
            [
                para(3, &[2.4, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.04045]),
                para(4, &[2.2, 0.98, 0.0, 0.08, 0.03, 0.02, 0.018_029]),
                para(3, &[1.8, 1.0, 0.0, 1.0 / 16.0, 0.031_25]),
            ],
        ),
    ));
    // Matrix/TRC: curv gamma and tables, v2.
    v.push((
        "rgb_v2_curv".into(),
        matrix_profile(
            2,
            "curv",
            [
                curv_gamma(1.8),
                curv_table(|x| x.powf(2.6), 1024),
                curv_table(|x| (x * 1.1 - 0.05).max(0.0).powf(2.0), 37),
            ],
        ),
    ));
    // Gray on XYZ and on Lab PCS.
    v.push((
        "gray_v4_para_xyz".into(),
        icc(
            4,
            b"mntr",
            b"GRAY",
            b"XYZ ",
            vec![
                (b"desc", desc(4, "gray")),
                (b"wtpt", xyz_tag([0.9642, 1.0, 0.8249])),
                (b"kTRC", para(2, &[2.2, 0.95, 0.01, 0.05])),
            ],
        ),
    ));
    v.push((
        "gray_v2_table_lab".into(),
        icc(
            2,
            b"scnr",
            b"GRAY",
            b"Lab ",
            vec![
                (b"desc", desc(2, "gray lab")),
                (b"wtpt", xyz_tag([0.9642, 1.0, 0.8249])),
                (b"kTRC", curv_table(|x| x.powf(0.9), 256)),
            ],
        ),
    ));
    // LUT-based RGB.
    let f3 = |t: f64| move |x: &[f64]| shade(x, 3, t);
    let x3 = |t: f64| move |x: &[f64]| lab_to_xyz_enc(&shade(x, 3, t));
    let u3 = |t: f64| move |x: &[f64]| unshade(x, 3, t);
    v.push((
        "rgb_v2_mft2_lab".into(),
        lut_profile(
            2,
            b"RGB ",
            true,
            "mft2 lab",
            &|t| {
                mft(true, 3, 3, 17, &|x| {
                    // v2 Lab encoding: L 0..100 in 0..0xFF00.
                    f3(t)(x).iter().map(|v| v * 65280.0 / 65535.0).collect()
                })
            },
            Some(&|t| {
                mft(true, 3, 3, 17, &|x| {
                    u3(t)(&x.iter().map(|v| v * 65535.0 / 65280.0).collect::<Vec<_>>())
                })
            }),
        ),
    ));
    // A Lab device space (like ITULab.icc): its darkest colour is L* = 0, a* = b* = 0, not
    // the device zero (a* = b* = −128), or the perceptual black point compensation tints
    // everything blue.
    v.push((
        "lab_v2_mft2_lab".into(),
        lut_profile(
            2,
            b"Lab ",
            true,
            "lab device",
            &|t| {
                mft(true, 3, 3, 17, &|x| {
                    let l = x[0] * (1.0 - 0.03 * t * (1.0 - x[0]));
                    vec![l, x[1], x[2]]
                })
            },
            None,
        ),
    ));
    v.push((
        "rgb_v2_mft1_lab".into(),
        lut_profile(
            2,
            b"RGB ",
            true,
            "mft1 lab",
            &|t| mft(false, 3, 3, 9, &f3(t)),
            None,
        ),
    ));
    v.push((
        "rgb_v2_mft2_xyz".into(),
        lut_profile(
            2,
            b"RGB ",
            false,
            "mft2 xyz",
            &|t| mft(true, 3, 3, 9, &x3(t)),
            None,
        ),
    ));
    v.push((
        "rgb_v4_mab_lab".into(),
        lut_profile(
            4,
            b"RGB ",
            true,
            "mAB lab",
            &|t| mab(true, 3, 3, 17, 2, &f3(t)),
            Some(&|t| mab(false, 3, 3, 17, 2, &u3(t))),
        ),
    ));
    v.push((
        "rgb_v4_mab8_xyz".into(),
        lut_profile(
            4,
            b"RGB ",
            false,
            "mAB xyz 8-bit",
            &|t| mab(true, 3, 3, 9, 1, &x3(t)),
            None,
        ),
    ));
    // LUT-based CMYK.
    let f4 = |t: f64| move |x: &[f64]| shade(x, 3, t);
    let u4 = |t: f64| move |x: &[f64]| unshade(x, 4, t);
    v.push((
        "cmyk_v4_mab_lab".into(),
        lut_profile(
            4,
            b"CMYK",
            true,
            "cmyk mAB",
            &|t| mab(true, 4, 3, 9, 2, &f4(t)),
            Some(&|t| mab(false, 3, 4, 17, 2, &u4(t))),
        ),
    ));
    v.push((
        "cmyk_v2_mft2_lab".into(),
        lut_profile(
            2,
            b"CMYK",
            true,
            "cmyk mft2",
            &|t| {
                mft(true, 4, 3, 9, &|x| {
                    f4(t)(x).iter().map(|v| v * 65280.0 / 65535.0).collect()
                })
            },
            Some(&|t| {
                mft(true, 3, 4, 17, &|x| {
                    u4(t)(&x.iter().map(|v| v * 65535.0 / 65280.0).collect::<Vec<_>>())
                })
            }),
        ),
    ));
    // The committed fuzz seeds (written by tools/fuzz/seeds.py).
    let seeds = Path::new(env!("CARGO_MANIFEST_DIR")).join("fuzz/seeds/profile");
    if let Ok(rd) = std::fs::read_dir(&seeds) {
        let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        files.sort();
        for f in files {
            if let Ok(d) = std::fs::read(&f) {
                v.push((
                    format!("seed:{}", f.file_name().unwrap().to_string_lossy()),
                    d,
                ));
            }
        }
    }
    v
}

/// Tolerances (ΔE2000) per direction: (mean, max). `fwd`/`inv` compare two exact CMMs on the
/// same tables; `gpu`/`disp` include the half-float 33³ / 1024-sample baking and still have
/// to stay below a just-noticeable difference.
fn tolerance(dir: &str) -> (f32, f32) {
    match dir {
        "fwd" => (0.05, 0.5),
        "inv" => (0.1, 1.0),
        // The GPU paths sample a 33³ cube trilinearly; coarse source LUTs with
        // non-linear input curves differ most in deep shadows.
        _ => (0.2, 2.5),
    }
}

fn check(rows: &[Row], exempt: &dyn Fn(&Row) -> bool) -> Vec<String> {
    let mut bad = Vec::new();
    for r in rows {
        let (m, x) = tolerance(r.dir);
        if (r.stats.mean() > m || r.stats.max() > x) && !exempt(r) {
            bad.push(format!(
                "{} {} {:?}: mean {:.4} max {:.4} (tolerance {m}/{x})",
                r.profile,
                r.dir,
                r.intent,
                r.stats.mean(),
                r.stats.max()
            ));
        }
    }
    bad
}

#[test]
fn synthetic_profiles_match_lcms() {
    let mut rows = Vec::new();
    let mut refused = Vec::new();
    for (name, data) in synthetic_profiles() {
        match measure(&name, &data, true) {
            Ok(r) => rows.extend(r),
            Err(e) => refused.push(format!("{name}: {e}")),
        }
    }
    eprintln!("{}", table(&rows));
    for r in &refused {
        eprintln!("refused: {r}");
    }
    let bad = check(&rows, &|_| false);
    assert!(bad.is_empty(), "out of tolerance:\n{}", bad.join("\n"));
    // Every synthetic profile (not the seeds, some of which are deliberately odd) is used.
    assert!(
        refused.iter().all(|r| r.starts_with("seed:")),
        "refused: {refused:?}"
    );
    assert!(rows.len() > 100, "{} rows", rows.len());
}

fn corpus_dirs() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut v = vec![match std::env::var_os("SPECULAR_QUALITY_CORPUS") {
        Some(d) => PathBuf::from(d).join("icc"),
        None => root.join("tools/corpus/quality/icc"),
    }];
    v.push("/usr/share/color/icc".into());
    v.push("/usr/share/color/icc/colord".into());
    v
}

/// Run with `cargo test -p specular-color --test icc_conformance -- --ignored --nocapture`
/// after `tools/quality/fetch-icc.sh`. Set `SPECULAR_QUALITY_OUT` to also write the table.
#[test]
#[ignore = "needs tools/quality/fetch-icc.sh"]
fn icc_corpus_conformance() {
    let mut files = Vec::new();
    for d in corpus_dirs() {
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                let p = e.path();
                let ext = p
                    .extension()
                    .map(|e| e.to_string_lossy().to_ascii_lowercase())
                    .unwrap_or_default();
                if p.is_file() && (ext == "icc" || ext == "icm") {
                    files.push(p);
                }
            }
        }
    }
    files.sort();
    files.dedup_by(|a, b| a.file_name() == b.file_name());
    assert!(
        !files.is_empty(),
        "no profiles: run tools/quality/fetch-icc.sh"
    );
    let mut rows = Vec::new();
    let mut skipped = Vec::new();
    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let data = std::fs::read(f).unwrap();
        match measure(&name, &data, false) {
            Ok(r) => rows.extend(r),
            Err(e) => skipped.push(format!("{name}: {e}")),
        }
    }
    let mut out = table(&rows);
    for s in &skipped {
        let _ = writeln!(out, "skipped: {s}");
    }
    eprintln!("{out}");
    if let Some(dir) = std::env::var_os("SPECULAR_QUALITY_OUT") {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(Path::new(&dir).join("icc-conformance.md"), &out);
    }
    let bad = check(&rows, &known_deviation);
    // One line per profile for tools/ci/oracle-pass-rates.sh.
    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let status = if skipped.iter().any(|s| s.starts_with(&format!("{name}: "))) {
            "skip"
        } else if bad.iter().any(|b| b.starts_with(&format!("{name} "))) {
            "fail"
        } else {
            "pass"
        };
        println!("ORACLE\tICC\t{name}\t{status}");
    }
    eprintln!(
        "{} profiles, {} measurements, {} skipped, {} out of tolerance",
        files.len(),
        rows.len(),
        skipped.len(),
        bad.len()
    );
    assert!(bad.is_empty(), "out of tolerance:\n{}", bad.join("\n"));
}

/// Deviations understood and accepted (documented in tools/quality/README.md).
fn known_deviation(r: &Row) -> bool {
    match r.profile.as_str() {
        // Carries D2Bx/B2Dx (multiProcessElement) tags that deliberately disagree with its
        // A2Bx/B2Ax tags; Little CMS prefers the float tags, we read the 16-bit ones (allowed).
        "Probev2_ICCv4.icc" => true,
        // kTRC is a one-entry curv with gamma 0: Little CMS maps black to 0^0 = 1 (white);
        // we keep black black. Every other sample agrees.
        "Gray_linear.icc" => r.stats.p99() < 0.01,
        _ => false,
    }
}
