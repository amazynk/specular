//! Colour calibration: white balance as a chromatic adaptation, the
//! channel mixer, and gamut compression, on linear Rec.2020.
//!
//! - **White balance.** The image arrives balanced for its as-shot illuminant
//!   (`as_shot_x/y`: the raw front end applied the camera's multipliers; other images carry
//!   their own white). The user's `white_x/y` names the illuminant the scene was really lit
//!   by, so the op adapts `white → as shot` with CAT16 or Bradford (complete adaptation).
//!   This equals adapting the unbalanced scene from `white` to D65: the von Kries scalings
//!   compose (`D65/white · as_shot/D65 = as_shot/white`). White = as shot is the identity.
//! - **Mixer**: the 3×3 after the adaptation (identity by default).
//! - **Gamut compression**: [`crate::colour::compress_gamut`] in Rec.2020, for the
//!   negative values out-of-gamut camera colours bring.
//!
//! All of it is one matrix and one compression per pixel.

use crate::colour::{self, Cat, M3};
use crate::graph::{NodeKind, Op};
use crate::model::Params;
use crate::model::params::{Adaptation, ColourCalibration, ModuleParams};

/// The colour calibration op, param version 1.
pub struct ColourCalibrationOp;

/// The linear Rec.2020 matrix of `c`: the mixer after the adaptation.
pub fn matrix(c: &ColourCalibration) -> M3 {
    let white = [f64::from(c.white_x), f64::from(c.white_y)];
    let shot = [f64::from(c.as_shot_x), f64::from(c.as_shot_y)];
    let cat = match c.adaptation {
        Adaptation::Cat16 => Some(Cat::Cat16),
        Adaptation::Bradford => Some(Cat::Bradford),
        Adaptation::None => None,
    };
    let adapt = match cat {
        Some(cat) if colour::valid_xy(white) && colour::valid_xy(shot) => {
            colour::adaptation_rec2020(cat, white, shot)
        }
        _ => colour::IDENTITY,
    };
    let mixer: M3 = if c.mixer.iter().all(|v| v.is_finite()) {
        std::array::from_fn(|i| std::array::from_fn(|j| f64::from(c.mixer[i * 3 + j])))
    } else {
        colour::IDENTITY
    };
    colour::mul(&mixer, &adapt)
}

/// The 15 uniforms: matrix (9), threshold, strength, limits (3), power.
pub fn uniforms(p: &Params) -> [f32; 15] {
    let c = ColourCalibration::from_fields(&p.to_fields());
    let m = super::m3_f32(&matrix(&c));
    let strength = if c.gamut_compression.is_finite() {
        c.gamut_compression.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let threshold = if c.gamut_threshold.is_finite() {
        c.gamut_threshold.clamp(0.0, 0.99)
    } else {
        0.8
    };
    let l = colour::GAMUT_LIMITS;
    let mut u = [0f32; 15];
    u[..9].copy_from_slice(&m);
    u[9] = threshold;
    u[10] = strength;
    u[11] = l[0] as f32;
    u[12] = l[1] as f32;
    u[13] = l[2] as f32;
    u[14] = colour::GAMUT_POWER as f32;
    u
}

/// WGSL twin of [`colour::compress_gamut`] (params at `p`: threshold, strength, limits,
/// power).
pub(crate) const GAMUT_WGSL: &str =
    "fn dv_compress_distance(d: f32, thr: f32, lim: f32, pwr: f32) -> f32 {
    if (d <= thr || lim <= thr) { return d; }
    let s = (lim - thr) / pow(pow((1.0 - thr) / (lim - thr), -pwr) - 1.0, 1.0 / pwr);
    let x = (d - thr) / s;
    return thr + s * x / pow(1.0 + pow(x, pwr), 1.0 / pwr);
}
fn dv_gamut(v: vec3<f32>, p: u32) -> vec3<f32> {
    let ach = max(v.x, max(v.y, v.z));
    let k = pf(p + 1u);
    if (ach <= 0.0 || k <= 0.0) { return v; }
    let thr = pf(p);
    let pwr = pf(p + 5u);
    let d = (vec3<f32>(ach) - v) / ach;
    let c = vec3<f32>(
        dv_compress_distance(d.x, thr, pf(p + 2u), pwr),
        dv_compress_distance(d.y, thr, pf(p + 3u), pwr),
        dv_compress_distance(d.z, thr, pf(p + 4u), pwr));
    let d2 = d + (c - d) * min(k, 1.0);
    return vec3<f32>(ach) - d2 * ach;
}
";

impl Op for ColourCalibrationOp {
    fn name(&self) -> &'static str {
        "colour_calibration_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Point
    }
    fn param_words(&self) -> u32 {
        15
    }
    fn write_params(&self, p: &Params, _: f32, out: &mut [u32]) {
        super::put(out, &uniforms(p));
    }
    fn helpers(&self) -> &'static [&'static str] {
        &[super::MAT3_WGSL, GAMUT_WGSL]
    }
    fn wgsl(&self) -> &'static str {
        "    return vec4<f32>(dv_gamut(dv_mat3(p, c.rgb), p + 9u), c.a);"
    }
}
