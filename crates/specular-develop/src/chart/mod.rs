//! Camera profiles from chart shots (ColorChecker, IT8): the user marks the chart's four
//! corner patches on a photo of it and loads its reference file; the patches are found and
//! sampled, a profile is fitted, and a ΔE2000 report shows how good it is.
//!
//! - **Raws** get a DNG camera profile (`.dcp`) that the raw front end uses through
//!   [`InputProfile::File`](crate::model::params::InputProfile::File) and that other raw
//!   editors read too.
//! - **Other images** get the colour calibration module's mixer, and optionally a `.cube`
//!   for the LUT module.
//!
//! # Steps
//!
//! 1. **Camera values** ([`chart_edit`], [`camera_values`]): the photo rendered with the
//!    camera matrix only (as-shot white balance, no exposure, no tone), the front end's
//!    output matrix undone per pixel: white-balanced camera RGB, exact while nothing clips.
//!    Other images: their linear RGB.
//! 2. **Find** ([`sample::locate`]): the corners snapped to their patches, a homography,
//!    every patch snapped and the homography refitted.
//! 3. **Sample** ([`sample::sample`]): inset squares, trimmed means; clipped, uneven and
//!    outside patches flagged and left out.
//! 4. **Solve** ([`solve::solve`]): white balance and exposure from the greys, the
//!    ForwardMatrix, the HueSatMap, the report.
//! 5. **Output** ([`output`]): the DCP, or the mixer and cube.
//!
//! Single-light profiles; [`output::dcp`] already takes two shots. No flat-field: uneven
//! light is reported as a warning ([`solve::Report::uneven_light`]).

pub mod cgats;
pub mod colour;
pub mod layout;
pub mod output;
pub mod sample;
pub mod solve;

#[cfg(test)]
mod tests;

/// The sensor colour and camera key the app needs for a profile (the app reads raws through
/// Develop only).
pub use specular_raw::{SensorMeta, camera_key};

use crate::model::params::{Highlights, InputProfile, RawFrontEnd};
use crate::model::{Edit, ModuleInstance, ModuleKind, Params};
use colour::{M3, V3};

/// The edit a chart photo is sampled with: the camera matrix only (the photo's own demosaic
/// choice, highlights clipped rather than rebuilt), nothing after it. Empty for other
/// images.
pub fn chart_edit(edit: &Edit, raw: bool) -> Edit {
    let mut e = Edit::empty(edit.source);
    if raw {
        let mut p = match edit
            .modules
            .iter()
            .find(|m| m.kind == ModuleKind::RAW_FRONT_END)
            .map(|m| &m.params)
        {
            Some(Params::RawFrontEnd(p)) => p.clone(),
            _ => RawFrontEnd::default(),
        };
        p.input_profile = InputProfile::Matrix;
        p.profile_path.clear();
        p.profile_hash.clear();
        p.highlights = Highlights::Clip;
        e.modules.push(ModuleInstance::with_params(p, 0));
    }
    e
}

/// The front end's camera → Rec.2020 matrix for [`chart_edit`] (with the baseline gain).
pub fn output_matrix(meta: &SensorMeta) -> M3 {
    crate::ops::raw::data::output_matrix(meta).map(|r| r.map(f64::from))
}

/// Turns the chart picture (linear Rec.2020 RGBA from [`chart_edit`]) into the values the
/// solver works on: white-balanced camera RGB for raws, the picture itself otherwise.
pub fn camera_values(rgba: &mut [f32], meta: Option<&SensorMeta>) {
    let Some(meta) = meta else { return };
    let Some(inv) = colour::inv(&output_matrix(meta)) else {
        return;
    };
    for px in rgba.chunks_exact_mut(4) {
        let v = [px[0], px[1], px[2]].map(f64::from);
        let c = colour::mulv(&inv, v);
        for k in 0..3 {
            px[k] = c[k] as f32;
        }
    }
}

/// The levels at or above which a sample counts as clipped, in [`camera_values`] units.
pub fn clip_levels(meta: Option<&SensorMeta>) -> [f32; 3] {
    match meta {
        Some(m) => {
            let w = if m.white > 0.0 {
                (m.clip_white / m.white).clamp(0.1, 1.0)
            } else {
                1.0
            };
            m.wb.map(|g| 0.98 * g * w)
        }
        None => [0.99; 3],
    }
}

/// The current colours (neutral-normalised camera values → XYZ D50): the camera matrix for
/// raws, the picture's own Rec.2020 otherwise.
pub fn before_matrix(meta: Option<&SensorMeta>) -> M3 {
    match meta {
        Some(m) => {
            let to_xyz = colour::inv(&colour::xyz_to_rec709()).unwrap_or(colour::IDENTITY);
            colour::mul(&to_xyz, &m.rgb_cam.map(|r| r.map(f64::from)))
        }
        None => colour::rec2020_to_xyz(),
    }
}

/// The camera's raw neutral under the chart's light: the fit's neutral (in white-balanced
/// units) without the as-shot balance, G = 1.
pub fn raw_neutral(neutral: V3, meta: &SensorMeta) -> V3 {
    let n: V3 = std::array::from_fn(|c| neutral[c] / f64::from(meta.wb[c]));
    n.map(|v| v / n[1])
}

/// The chart light's chromaticity: through the file's DNG ColorMatrix when it has one, else
/// the body's published matrix; `None` when neither is known.
pub fn white_xy(meta: &SensorMeta, make: &str, model: &str, raw_neutral: V3) -> Option<[f64; 2]> {
    let from_cm = meta
        .dng
        .as_ref()
        .and_then(|d| d.color_matrix[0])
        .and_then(|cm| {
            colour::inv(&std::array::from_fn(|r| {
                [cm[3 * r], cm[3 * r + 1], cm[3 * r + 2]]
            }))
        })
        .map(|m| colour::mulv(&m, raw_neutral))
        .and_then(|xyz| {
            let s: f64 = xyz.iter().sum();
            (s > 0.0 && xyz.iter().all(|v| *v >= 0.0)).then(|| [xyz[0] / s, xyz[1] / s])
        });
    from_cm.or_else(|| {
        let wb = raw_neutral.map(|v| (1.0 / v) as f32);
        specular_raw::as_shot_xy(make, model, wb)
    })
}
