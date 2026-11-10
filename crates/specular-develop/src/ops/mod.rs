//! The module ops, one per module.
//!
//! - [`raw`]: the raw front end, which makes the graph's source.
//! - [`exposure`], [`colour_calibration`], [`view`]: the alpha's scene-linear and view ops,
//!   point ops that fuse into one kernel; exposure's auto mode is a global statistic.
//! - [`tone`]: the tone equalizer (guided-filter mask), local contrast (fast local
//!   Laplacian) and curves with levels.
//! - [`colour`]: colour grading in OkLab: colour balance, the colour equalizer, point
//!   colour, black & white and 3D LUTs.
//! - [`detail`]: the noise model, denoise (wavelets, then non-local means), capture
//!   sharpening (Richardson–Lucy) and sharpen (unsharp mask with a details mask).
//! - [`correct`]: highlight reconstruction, hot pixels, raw and RGB chromatic aberration,
//!   defringe, flat-field and dark-frame calibration; the CFA steps run inside the
//!   raw front end.
//! - [`retouch`]: heal, clone, fill and blur spots, on the picture or one layer of a
//!   frequency separation.
//! - [`effects`]: vignette, graduated ND, grain, glow, dehaze, negative inversion, borders
//!   and watermark.
//! - [`depth`]: depth blur and relight, from the photo's depth map.
//! - [`output`]: the output transform the caller appends ([`crate::graph::Graph::push_output`]):
//!   linear Rec.2020 → the display storage space or an export space, gamut compressed.
//!
//! Each op writes its params as a few f32 uniforms computed in f64 on the CPU (matrices,
//! curve constants), so a slider rewrites bytes and never builds a kernel. Every op has a CPU
//! reference in [`cpu_ref`] (tests only) doing the same f32 arithmetic.

pub mod colour;
pub mod colour_calibration;
pub mod correct;
pub mod depth;
pub mod detail;
pub mod effects;
pub mod exposure;
pub mod geometry;
pub mod output;
pub mod raw;
pub mod retouch;
pub mod tone;
pub mod view;

#[cfg(test)]
pub(crate) mod cpu_ref;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_quality;

use std::sync::Arc;

use crate::graph::OpRegistry;
use crate::model::ModuleKind;

/// Registers the shipped ops.
pub fn register_builtin(r: &mut OpRegistry) {
    r.register(ModuleKind::EXPOSURE, 1, Arc::new(exposure::ExposureOp));
    r.register(
        ModuleKind::COLOUR_CALIBRATION,
        1,
        Arc::new(colour_calibration::ColourCalibrationOp),
    );
    r.register(
        ModuleKind::TONE_EQUALIZER,
        1,
        Arc::new(tone::equalizer::ToneEqOp),
    );
    r.register(
        ModuleKind::LOCAL_CONTRAST,
        1,
        Arc::new(tone::local_contrast::LocalContrastOp),
    );
    r.register(ModuleKind::CURVES, 1, Arc::new(tone::curves::CurvesOp));
    r.register(
        ModuleKind::COLOUR_BALANCE,
        1,
        Arc::new(colour::balance::ColourBalanceOp),
    );
    r.register(
        ModuleKind::COLOUR_EQUALIZER,
        1,
        Arc::new(colour::equalizer::ColourEqualizerOp),
    );
    r.register(
        ModuleKind::POINT_COLOUR,
        1,
        Arc::new(colour::point::PointColourOp),
    );
    r.register(
        ModuleKind::BLACK_WHITE,
        1,
        Arc::new(colour::bw::BlackWhiteOp),
    );
    r.register(ModuleKind::LUT_3D, 1, Arc::new(colour::lut::Lut3dOp));
    r.register(ModuleKind::VIEW_TRANSFORM, 1, Arc::new(view::ViewOp));
    r.register(ModuleKind::DENOISE, 1, Arc::new(detail::denoise::DenoiseOp));
    r.register(
        ModuleKind::CAPTURE_SHARPENING,
        1,
        Arc::new(detail::capture::CaptureOp),
    );
    r.register(ModuleKind::SHARPEN, 1, Arc::new(detail::sharpen::SharpenOp));
    r.register(ModuleKind::RETOUCH, 1, Arc::new(retouch::RetouchOp));
    r.register(ModuleKind::DIFFUSE, 1, Arc::new(detail::diffuse::DiffuseOp));
    r.register(
        ModuleKind::SURFACE_BLUR,
        1,
        Arc::new(detail::surface::SurfaceBlurOp),
    );
    r.register(
        ModuleKind::DEFRINGE,
        1,
        Arc::new(correct::defringe::DefringeOp),
    );
    use effects::*;
    r.register(ModuleKind::NEGATIVE, 1, Arc::new(negative::NegativeOp));
    r.register(ModuleKind::DEHAZE, 1, Arc::new(dehaze::DehazeOp));
    r.register(
        ModuleKind::GRADUATED_ND,
        1,
        Arc::new(graduated::GraduatedOp),
    );
    r.register(ModuleKind::VIGNETTE, 1, Arc::new(vignette::VignetteOp));
    r.register(ModuleKind::GLOW, 1, Arc::new(glow::GlowOp));
    r.register(ModuleKind::GRAIN, 1, Arc::new(grain::GrainOp));
    r.register(ModuleKind::RELIGHT, 1, Arc::new(depth::relight::RelightOp));
    r.register(
        ModuleKind::DEPTH_BLUR,
        1,
        Arc::new(depth::blur::DepthBlurOp),
    );
}

/// Writes f32 values into uniform words.
pub(crate) fn put(out: &mut [u32], vals: &[f32]) {
    for (w, v) in out.iter_mut().zip(vals) {
        *w = v.to_bits();
    }
}

/// A row-major 3×3 f64 matrix as nine f32.
pub(crate) fn m3_f32(m: &crate::colour::M3) -> [f32; 9] {
    std::array::from_fn(|i| m[i / 3][i % 3] as f32)
}

/// Shared WGSL: a 3×3 matrix of nine param words times a colour.
pub(crate) const MAT3_WGSL: &str = "fn dv_mat3(p: u32, v: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        pf(p) * v.x + pf(p + 1u) * v.y + pf(p + 2u) * v.z,
        pf(p + 3u) * v.x + pf(p + 4u) * v.y + pf(p + 5u) * v.z,
        pf(p + 6u) * v.x + pf(p + 7u) * v.y + pf(p + 8u) * v.z);
}
";
