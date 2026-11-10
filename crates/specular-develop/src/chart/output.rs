//! What a fit becomes.
//!
//! - **Raws: a DCP** ([`dcp`]): ForwardMatrix, a ColorMatrix derived from it, the
//!   HueSatMap, for the light the chart was shot in. Single illuminant for now; [`dcp`]
//!   takes one or two shots, so a second (daylight and tungsten) is a follow-up.
//!   - ColorMatrix = (B · FM · diag(1 / n))⁻¹, with n the camera's raw neutral under the
//!     chart's light and B the Bradford adaptation from D50 to that light, scaled so the
//!     light's white maps to a largest camera value of 1. That is what the DNG reader
//!     inverts, so reading back gives the same neutral.
//! - **Other images: the internal path** ([`mixer`], [`cube`]): the colour calibration
//!   module's 3×3 mixer in the working space (linear Rec.2020), and optionally a 33³ `.cube`
//!   for the table (or a root-polynomial fit) after it.
//!   - The cube runs in the LUT module's `acescct` space, not `linear_rec2020`: a linear
//!     domain would clip highlights above 1 and spend few grid points on shadows.
//! - **Files** live in `<data>/profiles/<camera>/<name>.dcp` (or `.cube`), `<data>` being
//!   Specular's data folder (`$XDG_DATA_HOME/specular` on Linux).

use std::path::{Path, PathBuf};

use specular_raw::Dcp;
use specular_raw::sensor::HsvTableData;

use super::colour::{self, M3, V3};
use super::solve::{Fit, Table, apply_root, forward_prophoto, prophoto_matrix};

/// One solved shot and its light.
#[derive(Clone, Debug)]
pub struct Shot<'a> {
    pub fit: &'a Fit,
    /// The camera's raw neutral under the chart's light (as-shot neutral over the chart's
    /// grey, G = 1).
    pub raw_neutral: V3,
    /// The light's chromaticity (CIE 1931 xy), when known; D65 otherwise.
    pub white_xy: Option<[f64; 2]>,
}

/// EXIF light source codes the profile may name, with their correlated colour temperature.
const ILLUMINANTS: [(u32, f64); 5] = [
    (17, 2856.0), // Standard light A
    (23, 5003.0), // D50
    (20, 5503.0), // D55
    (21, 6504.0), // D65
    (22, 7504.0), // D75
];

/// McCamy's correlated colour temperature of `xy`.
pub fn cct(xy: [f64; 2]) -> f64 {
    let n = (xy[0] - 0.3320) / (0.1858 - xy[1]);
    449.0 * n.powi(3) + 3525.0 * n.powi(2) + 6823.3 * n + 5520.33
}

/// The standard illuminant nearest to `xy` (in mireds).
pub fn illuminant_code(xy: Option<[f64; 2]>) -> u32 {
    let Some(xy) = xy else { return 21 };
    let t = cct(xy);
    if !(1000.0..25000.0).contains(&t) {
        return 21;
    }
    ILLUMINANTS
        .iter()
        .min_by(|a, b| {
            (1e6 / a.1 - 1e6 / t)
                .abs()
                .total_cmp(&(1e6 / b.1 - 1e6 / t).abs())
        })
        .map_or(21, |e| e.0)
}

fn flat(m: &M3) -> [f64; 9] {
    std::array::from_fn(|i| m[i / 3][i % 3])
}

/// The ColorMatrix for a shot.
pub fn color_matrix(shot: &Shot) -> Option<M3> {
    let xy = shot.white_xy.unwrap_or(colour::D65_XY);
    let white = colour::xy_to_xyz(xy);
    let b = colour::bradford(colour::d50(), white);
    let n = shot.raw_neutral;
    let m = colour::mul(
        &colour::mul(&b, &shot.fit.fm),
        &colour::diag(n.map(|v| 1.0 / v)),
    );
    let mut cm = colour::inv(&m)?;
    let cam = colour::mulv(&cm, white);
    let top = cam.iter().copied().fold(0f64, f64::max);
    if !(top.is_finite() && top > 0.0) {
        return None;
    }
    for r in &mut cm {
        for v in r.iter_mut() {
            *v /= top;
        }
    }
    Some(cm)
}

fn table_data(t: &Table) -> HsvTableData {
    HsvTableData {
        hue: t.hue,
        sat: t.sat,
        val: 1,
        srgb: false,
        data: t.data.iter().map(|e| e.map(|v| v as f32)).collect(),
    }
}

/// The camera profile for `shots` (one, or two lights; the first is CalibrationIlluminant1).
pub fn dcp(camera_model: &str, name: &str, shots: &[Shot]) -> Option<Dcp> {
    let first = shots.first()?;
    let second = shots.get(1);
    let cm1 = color_matrix(first)?;
    let cm2 = match second {
        Some(s) => Some(color_matrix(s)?),
        None => None,
    };
    Some(Dcp {
        camera_model: camera_model.to_owned(),
        name: name.to_owned(),
        copyright: "Made in Specular from a chart shot".to_owned(),
        // No restrictions: the user's own measurement.
        embed_policy: 3,
        illuminants: [
            illuminant_code(first.white_xy),
            second.map_or(0, |s| illuminant_code(s.white_xy)),
        ],
        color_matrix: [Some(flat(&cm1)), cm2.as_ref().map(flat)],
        forward_matrix: [Some(flat(&first.fit.fm)), second.map(|s| flat(&s.fit.fm))],
        hue_sat_map: [
            first.fit.table.as_ref().map(table_data),
            second.and_then(|s| s.fit.table.as_ref().map(table_data)),
        ],
        look_table: None,
    })
}

/// The colour calibration mixer for other images (linear Rec.2020 in and out, row-major):
/// the chart's white balance, then the ForwardMatrix, back to Rec.2020.
pub fn mixer(fit: &Fit) -> [f32; 9] {
    let to2020 = colour::inv(&colour::rec2020_to_xyz()).unwrap_or(colour::IDENTITY);
    let m = colour::mul(
        &colour::mul(&to2020, &fit.fm),
        &colour::diag(fit.neutral.map(|v| 1.0 / v)),
    );
    std::array::from_fn(|i| m[i / 3][i % 3] as f32)
}

/// What the cube adds after the mixer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CubeModel {
    /// The HueSatMap in ProPhoto (as a DCP would apply it).
    Table,
    /// The root-polynomial fit instead of the matrix.
    RootPolynomial,
}

/// Grid points per axis.
pub const CUBE_SIZE: usize = 33;

const CCT_A: f64 = 10.540_237_741_654_5;
const CCT_B: f64 = 0.072_905_534_195_835_5;
const CCT_BREAK: f64 = 0.155_251_141_552_511;

/// ACEScct → linear AP1.
pub fn acescct_to_linear(y: f64) -> f64 {
    if y <= CCT_BREAK {
        (y - CCT_B) / CCT_A
    } else {
        2f64.powf(y * 17.52 - 9.72).min(65504.0)
    }
}

/// Linear AP1 → ACEScct.
pub fn linear_to_acescct(x: f64) -> f64 {
    if x <= 0.0078125 {
        CCT_A * x + CCT_B
    } else {
        (x.log2() + 9.72) / 17.52
    }
}

fn ap1_matrices() -> (M3, M3) {
    let to = crate::ops::colour::lut::LutSpace::Acescct.to_primaries();
    (to, colour::inv(&to).unwrap_or(colour::IDENTITY))
}

/// The cube that follows [`mixer`] (input and output ACEScct), as `.cube` text.
pub fn cube(fit: &Fit, model: CubeModel) -> String {
    let (to_ap1, from_ap1) = ap1_matrices();
    let mix = mixer(fit).map(f64::from);
    let mix: M3 = std::array::from_fn(|r| [mix[3 * r], mix[3 * r + 1], mix[3 * r + 2]]);
    let unmix = colour::inv(&mix).unwrap_or(colour::IDENTITY);
    let xyz_to_2020 = colour::inv(&colour::rec2020_to_xyz()).unwrap_or(colour::IDENTITY);
    let pro_to_xyz = colour::prophoto_to_xyz();
    let n = fit.neutral;
    let mut out = String::with_capacity(CUBE_SIZE.pow(3) * 30);
    out.push_str("TITLE \"Specular chart profile\"\n");
    out.push_str("# Input and output: ACEScct (Specular LUT space acescct), after the mixer.\n");
    out.push_str(&format!("LUT_3D_SIZE {CUBE_SIZE}\n"));
    let s = (CUBE_SIZE - 1) as f64;
    for b in 0..CUBE_SIZE {
        for g in 0..CUBE_SIZE {
            for r in 0..CUBE_SIZE {
                let enc = [r, g, b].map(|v| v as f64 / s);
                let ap1 = enc.map(acescct_to_linear);
                let x = colour::mulv(&from_ap1, ap1);
                // Back to the neutral-normalised camera values the fit works on.
                let cam = colour::mulv(&unmix, x);
                let c: V3 = std::array::from_fn(|k| cam[k] / n[k]);
                let y = corrected(fit, model, c, x, &xyz_to_2020, &pro_to_xyz);
                let o = colour::mulv(&to_ap1, y).map(linear_to_acescct);
                out.push_str(&format!("{:.6} {:.6} {:.6}\n", o[0], o[1], o[2]));
            }
        }
    }
    out
}

/// Rec.2020 after the model for neutral-normalised camera value `c` (`x`: the mixer's
/// result, kept where the model does not apply).
fn corrected(fit: &Fit, model: CubeModel, c: V3, x: V3, xyz_to_2020: &M3, pro_to_xyz: &M3) -> V3 {
    match model {
        CubeModel::RootPolynomial => {
            if c.iter().any(|v| *v < 0.0) {
                return x;
            }
            colour::mulv(xyz_to_2020, apply_root(&fit.root_polynomial, c))
        }
        CubeModel::Table => {
            let Some(t) = &fit.table else { return x };
            // The table is value-independent: run it on the colour scaled into 0–1, then
            // scale back, so highlights keep their correction.
            let p = colour::mulv(&prophoto_matrix(&fit.fm), c);
            let top = p.iter().copied().fold(0f64, f64::max);
            if p.iter().any(|v| *v < 0.0) || top <= 0.0 {
                return x;
            }
            // Headroom for the table's value scale (up to 1.5) below the readers' clip at 1.
            let k = top.max(1.0) * 1.5;
            let q = forward_prophoto(&fit.fm, Some(t), c.map(|v| v / k)).map(|v| v * k);
            colour::mulv(xyz_to_2020, colour::mulv(pro_to_xyz, q))
        }
    }
}

/// A file name from a profile name: letters, digits, spaces, `-`, `_` and `.` kept (others
/// become `_`), trimmed, at most 80 characters; `profile` when nothing is left.
pub fn file_stem(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .take(80)
        .collect();
    let s = s.trim().trim_matches('.').trim().to_owned();
    if s.is_empty() {
        "profile".to_owned()
    } else {
        s
    }
}

/// The folder of a camera's profiles under Specular's data folder `data` (its
/// `$XDG_DATA_HOME/specular` on Linux): `<data>/profiles/<camera>`.
pub fn profile_dir(data: &Path, camera_key: &str) -> PathBuf {
    data.join("profiles").join(file_stem(camera_key))
}

/// The content hash the edit stores with the path (BLAKE3, hex).
pub fn content_hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acescct_round_trips() {
        for x in [0.0, 0.001, 0.0078125, 0.18, 1.0, 16.0, 200.0] {
            let y = linear_to_acescct(x);
            assert!((acescct_to_linear(y) - x).abs() < 1e-9 * x.max(1.0), "{x}");
        }
    }

    #[test]
    fn illuminants_and_names() {
        assert_eq!(illuminant_code(Some([0.4476, 0.4074])), 17);
        assert_eq!(illuminant_code(Some(colour::D65_XY)), 21);
        assert_eq!(illuminant_code(Some([0.3457, 0.3585])), 23);
        assert_eq!(illuminant_code(None), 21);
        assert_eq!(file_stem("  Studio / tungsten?  "), "Studio _ tungsten_");
        assert_eq!(file_stem("..."), "profile");
        assert!(profile_dir(Path::new("/d"), "Canon EOS R5").ends_with("profiles/Canon EOS R5"));
    }
}
