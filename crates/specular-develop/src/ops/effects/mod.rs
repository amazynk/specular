//! The effects: creative looks after the tone and colour
//! work, each a module of its own.
//!
//! - [`negative`]: film negative inversion, a Cineon-style density model (film base,
//!   density range, paper grade and gloss), colour or black and white. Runs first in the
//!   scene stage, so every later module sees a positive.
//! - [`dehaze`]: the dark channel prior (He, Sun and Tang 2009) with a guided-filter
//!   transmission (fast guided filter, He and Sun 2015); negative strength adds haze.
//! - [`graduated`]: a graduated ND filter: an exposure (and optional tint) gradient with
//!   angle and softness.
//! - [`vignette`]: a radial exposure falloff with centre, shape, roundness and feather,
//!   following the crop, with highlight protection.
//! - [`glow`]: bloom and Orton-style soften: a blurred bright pass added or screened back.
//! - [`grain`]: film-like grain, shaped noise (a cheap stand-in for the Boolean grain model
//!   of Newson et al. 2017) whose size is a fraction of the image, so preview and export
//!   look alike; seeded by the photo and the module.
//! - [`frame`]: borders and an SVG watermark with variables, applied to exports and prints
//!   only (not a graph node).
//!
//! Point effects (vignette, graduated ND, grain, negative) fuse with the point ops around
//! them. Glow and dehaze are global ops: their blurs and transmission run once on the
//! whole-image statistics grid, so a slider on them is as cheap as a point op and a view of
//! any size or tile sees the same result. Every geometry is in fractions of the image the
//! scene stage sees, which is after the crop: effects follow the crop.

pub mod dehaze;
pub mod frame;
pub mod glow;
pub mod graduated;
pub mod grain;
pub(crate) mod kernels;
pub mod negative;
pub mod vignette;

#[cfg(test)]
pub(crate) mod cpu;
#[cfg(test)]
mod tests;

use crate::model::Fields;

/// A float field, clamped, with `d` for missing or non-finite values.
pub(crate) fn get(f: &Fields, name: &str, d: f32, lo: f32, hi: f32) -> f32 {
    let v = f.f32_or(name, d);
    if v.is_finite() { v.clamp(lo, hi) } else { d }
}

/// `smoothstep` as WGSL computes it (edges in either order).
pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Rec.2020 luminance.
pub fn luma(c: [f32; 3]) -> f32 {
    0.2627 * c[0] + 0.6780 * c[1] + 0.0593 * c[2]
}

/// Shared WGSL: luminance and the pixel's place in the frame in units of the long side.
pub(crate) const FX_WGSL: &str = "const DV_FX_LUMA = vec3<f32>(0.2627, 0.6780, 0.0593);
fn dv_fx_pos(px: i32, py: i32, cx: f32, cy: f32) -> vec2<f32> {
    let w = f32(pu(4u));
    let h = f32(pu(5u));
    let l = max(w, h);
    return vec2<f32>((f32(px) + 0.5 - cx * w) / l, (f32(py) + 0.5 - cy * h) / l);
}
";

/// The CPU twin of `dv_fx_pos` for a `w`×`h` image.
pub fn fx_pos(px: i32, py: i32, cx: f32, cy: f32, w: u32, h: u32) -> [f32; 2] {
    let (w, h) = (w as f32, h as f32);
    let l = w.max(h);
    [
        (px as f32 + 0.5 - cx * w) / l,
        (py as f32 + 0.5 - cy * h) / l,
    ]
}
