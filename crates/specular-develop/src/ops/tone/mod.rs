//! The tone ops: the guided tone equalizer, local contrast
//! by the fast local Laplacian filter, and RGB curves with levels. Auto exposure, the
//! other global statistic, is in [`super::exposure`] on these [`kernels`].

pub mod curves;
pub mod equalizer;
pub mod kernels;
pub mod local_contrast;

// The CPU forms of the kernels: tests, and the tone equalizer's mask percentiles for zone
// picks ([`equalizer::ToneEq::mask_percentiles`]).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) mod cpu;
#[cfg(test)]
pub(crate) mod tests;

/// Rec.2020 luminance weights (the Y row of [`crate::colour::rec2020_to_xyz`]).
pub const LUMA: [f32; 3] = [0.2627, 0.6780, 0.0593];
