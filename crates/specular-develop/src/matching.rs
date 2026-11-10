//! Match to reference (the idea from Capture One's smart
//! adjustments, our own method): make a target photo's exposure and white balance match a
//! reference's, written as ordinary exposure and colour calibration params.
//!
//! Both photos are read as scene-linear Rec.2020 before their exposure (the raw front end's
//! output, at a small size). Then, from each photo's mid-tones (the 2nd to 98th luminance
//! percentile, no clipped channel: the content both photos share, not their highlights,
//! shadows or clipped areas):
//!
//! - **White balance** from a grey estimate: the mean of the mid-tones (grey world on the
//!   trimmed set). The reference's grey after its own colour calibration is where the
//!   target's grey must land, so the target's scene white solves, per cone channel of the
//!   target's adaptation transform, `white ∝ as_shot · grey_target / grey_reference_out`
//!   (von Kries scalings compose; see `ops::colour_calibration`).
//! - **Exposure** from the median scene luminance: with the new white balance, the target's
//!   median Y moves to the reference's median Y after the reference's exposure and colour
//!   calibration.

use crate::colour::{self, Cat, M3};
use crate::model::params::{Adaptation, ColourCalibration, Exposure, ModuleParams, WhiteSource};
use crate::model::{Edit, ModuleKind};
use crate::ops::colour_calibration;

/// Luminance percentiles kept as the shared content.
const LOW_PCT: f64 = 0.02;
const HIGH_PCT: f64 = 0.98;
/// Pixels kept for the statistics (subsampled evenly beyond this).
const MAX_PIXELS: usize = 1 << 16;

/// A photo's mid-tone pixels, scene-linear Rec.2020 before its exposure.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    pub pixels: Vec<[f64; 3]>,
}

/// Rec.2020 luminance.
fn luma(p: [f64; 3]) -> f64 {
    0.2627 * p[0] + 0.6780 * p[1] + 0.0593 * p[2]
}

impl Scene {
    /// The mid-tones of RGBA pixels (alpha ignored): channels at or above `clip` and
    /// non-finite or non-positive luminance are left out, then everything outside the
    /// luminance percentiles. `None` when too little is left to judge (under 64 pixels).
    pub fn from_rgba(rgba: &[f32], clip: f32) -> Option<Scene> {
        let n = rgba.len() / 4;
        let step = n.div_ceil(MAX_PIXELS * 2).max(1);
        let mut px: Vec<[f64; 3]> = rgba
            .chunks_exact(4)
            .step_by(step)
            .filter(|p| p[..3].iter().all(|v| v.is_finite() && *v < clip))
            .map(|p| [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])])
            .filter(|p| luma(*p) > 0.0)
            .collect();
        if px.len() < 64 {
            return None;
        }
        px.sort_by(|a, b| luma(*a).total_cmp(&luma(*b)));
        let lo = (px.len() as f64 * LOW_PCT) as usize;
        let hi = ((px.len() as f64 * HIGH_PCT) as usize).max(lo + 1);
        let mut kept: Vec<[f64; 3]> = px[lo..hi.min(px.len())].to_vec();
        if kept.len() > MAX_PIXELS {
            let k = kept.len().div_ceil(MAX_PIXELS);
            kept = kept.into_iter().step_by(k).collect();
        }
        Some(Scene { pixels: kept })
    }

    /// The grey estimate: the mean of the mid-tones.
    pub fn grey(&self) -> [f64; 3] {
        let n = self.pixels.len().max(1) as f64;
        let mut s = [0.0; 3];
        for p in &self.pixels {
            for k in 0..3 {
                s[k] += p[k];
            }
        }
        s.map(|v| v / n)
    }

    /// The median luminance after the linear map `m` and a gain of `2^ev`.
    pub fn median_y(&self, m: &M3, ev: f64) -> f64 {
        let mut y: Vec<f64> = self
            .pixels
            .iter()
            .map(|p| luma(colour::mulv(m, *p)))
            .collect();
        if y.is_empty() {
            return 0.0;
        }
        let mid = y.len() / 2;
        let (_, v, _) = y.select_nth_unstable_by(mid, f64::total_cmp);
        *v * ev.exp2()
    }
}

/// The exposure and white the target gets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Solved {
    pub exposure_ev: f32,
    /// The scene illuminant (CIE 1931 xy) for colour calibration.
    pub white_xy: [f32; 2],
}

fn calibration(e: &Edit) -> ColourCalibration {
    e.module(ModuleKind::COLOUR_CALIBRATION, 0)
        .filter(|m| m.enabled)
        .map(|m| ColourCalibration::from_fields(&m.params.to_fields()))
        .unwrap_or_default()
}

fn exposure(e: &Edit) -> Exposure {
    e.module(ModuleKind::EXPOSURE, 0)
        .filter(|m| m.enabled)
        .map(|m| Exposure::from_fields(&m.params.to_fields()))
        .unwrap_or_default()
}

/// Solves the target's exposure and white so it matches the reference. `None` when the
/// numbers do not allow it (a black grey, an unusable white).
pub fn solve(
    reference: &Scene,
    ref_edit: &Edit,
    target: &Scene,
    target_edit: &Edit,
) -> Option<Solved> {
    let rc = calibration(ref_edit);
    let re = exposure(ref_edit);
    let tc = calibration(target_edit);
    let ref_m = colour_calibration::matrix(&rc);
    // Where the reference's grey lands, and where the target's must, before the target's
    // mixer.
    let out_r = colour::mulv(&ref_m, reference.grey());
    let mixer: M3 =
        std::array::from_fn(|i| std::array::from_fn(|j| f64::from(tc.mixer[i * 3 + j])));
    let want = colour::mulv(&colour::inv(&mixer)?, out_r);
    let cat = match tc.adaptation {
        Adaptation::Bradford => Cat::Bradford,
        _ => Cat::Cat16,
    };
    let to_lms = colour::mul(cat.matrix(), &colour::rec2020_to_xyz());
    let g_t = colour::mulv(&to_lms, target.grey());
    let w_lms = colour::mulv(&to_lms, want);
    let shot = colour::mulv(
        cat.matrix(),
        colour::xy_to_xyz([f64::from(tc.as_shot_x), f64::from(tc.as_shot_y)]),
    );
    if (0..3).any(|k| g_t[k] <= 0.0 || w_lms[k] <= 0.0 || shot[k] <= 0.0) {
        return None;
    }
    let white_lms: [f64; 3] = std::array::from_fn(|k| shot[k] * g_t[k] / w_lms[k]);
    let white_xyz = colour::mulv(&colour::inv(cat.matrix())?, white_lms);
    let xy = colour::xyz_to_xy(white_xyz);
    if !colour::valid_xy(xy) {
        return None;
    }
    let mut new_c = tc.clone();
    new_c.white_x = xy[0] as f32;
    new_c.white_y = xy[1] as f32;
    let t_m = colour_calibration::matrix(&new_c);
    let y_r = reference.median_y(&ref_m, f64::from(re.exposure_ev));
    let y_t = target.median_y(&t_m, 0.0);
    if !(y_r > 0.0 && y_t > 0.0) {
        return None;
    }
    let ev = (y_r / y_t).log2().clamp(-10.0, 10.0);
    Some(Solved {
        exposure_ev: ev as f32,
        white_xy: [xy[0] as f32, xy[1] as f32],
    })
}

/// `edit` with the solved exposure and white written in (white source custom, auto
/// exposure off); everything else stays.
pub fn applied(edit: &Edit, s: &Solved) -> Edit {
    let mut e = edit.clone();
    if let Some(m) = e.module_mut(ModuleKind::EXPOSURE, 0) {
        let mut p = Exposure::from_fields(&m.params.to_fields());
        p.exposure_ev = s.exposure_ev;
        p.auto = false;
        m.params.overlay(&p.to_fields());
        m.enabled = true;
    }
    if let Some(m) = e.module_mut(ModuleKind::COLOUR_CALIBRATION, 0) {
        let mut p = ColourCalibration::from_fields(&m.params.to_fields());
        p.white_x = s.white_xy[0];
        p.white_y = s.white_xy[1];
        p.white_source = WhiteSource::Custom;
        m.params.overlay(&p.to_fields());
        m.enabled = true;
    }
    e
}

/// The Planckian locus (CIE 1931 xy) at `t` kelvin, 1667–25000 K (Kim et al.'s cubic
/// fits).
pub fn planck_xy(t: f64) -> [f64; 2] {
    let t = t.clamp(1667.0, 25_000.0);
    let (t1, t2, t3) = (1e3 / t, 1e6 / (t * t), 1e9 / (t * t * t));
    let x = if t <= 4000.0 {
        -0.266_123_9 * t3 - 0.234_358_9 * t2 + 0.877_695_6 * t1 + 0.179_910
    } else {
        -3.025_846_9 * t3 + 2.107_037_9 * t2 + 0.222_634_7 * t1 + 0.240_390
    };
    let (x2, x3) = (x * x, x * x * x);
    let y = if t <= 2222.0 {
        -1.106_381_4 * x3 - 1.348_110_20 * x2 + 2.185_558_32 * x - 0.202_196_83
    } else if t <= 4000.0 {
        -0.954_947_6 * x3 - 1.374_185_93 * x2 + 2.091_370_15 * x - 0.167_488_67
    } else {
        3.081_758_0 * x3 - 5.873_386_70 * x2 + 3.751_129_97 * x - 0.370_014_83
    };
    [x, y]
}

/// The correlated colour temperature of `xy` (kelvin): the nearest point of the Planckian
/// locus in CIE 1960 uv.
pub fn cct(xy: [f64; 2]) -> f64 {
    let uv = |p: [f64; 2]| {
        let d = -2.0 * p[0] + 12.0 * p[1] + 3.0;
        [4.0 * p[0] / d, 6.0 * p[1] / d]
    };
    let target = uv(xy);
    let dist = |mired: f64| {
        let p = uv(planck_xy(1e6 / mired));
        (p[0] - target[0]).hypot(p[1] - target[1])
    };
    // Golden-section search in mireds (the locus is close to uniform there).
    let (mut a, mut b) = (40.0, 600.0);
    let g = (5f64.sqrt() - 1.0) / 2.0;
    for _ in 0..80 {
        let c = b - g * (b - a);
        let d = a + g * (b - a);
        if dist(c) < dist(d) {
            b = d;
        } else {
            a = c;
        }
    }
    1e6 / ((a + b) / 2.0)
}

#[cfg(test)]
mod tests;
