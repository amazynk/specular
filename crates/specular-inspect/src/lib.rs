//! Inspection tools (DESIGN.md §6.2): the CPU side of the histogram, the clipping statistics,
//! the pixel inspector, channel isolation, and compare's difference metrics ([`DiffStats`]).
//!
//! The pixels themselves never come to the CPU: `specular-render` samples the visible part of
//! the image on the GPU, bins it in a compute pass and reads back [`Histogram::WORDS`] words.
//! This crate turns those words into statistics and display curves, and formats the values
//! the pixel inspector shows. The binning rules ([`bin`], [`luma`]) are the shader's, so the
//! CPU reference ([`Histogram::add_linear`]) and the GPU agree bin for bin.

mod channel;
mod colormap;
mod colour;
pub mod cvd;
pub mod diff;
mod histogram;
mod legend;
pub mod quality;
mod sample;

pub use channel::{ChannelColours, ChannelKey, ChannelView};
pub use colormap::{ColourMap, MAP_LEN};
pub use colour::{bin, luma, srgb_decode, srgb_encode, to_byte};
pub use cvd::Cvd;
pub use diff::{
    DEFAULT_SCALE, DiffKind, DiffStats, DiffSums, MetricSpace, PixelError, RAMP, REL_EPS,
    SCALE_RANGE, diff_colour, false_colour, metric_text, pixel_error, pixel_error_in, reference,
    reference_in, signed_error,
};
pub use histogram::{BINS, Channel, Histogram, Stats};
pub use legend::{BitRange, Legend, format_value, tick_step, ticks};
pub use quality::{DeltaEStats, delta_e_reference, ssim_reference};
pub use sample::{DisplayValue, SourceValue, hex, source_pixel, ycbcr_to_rgb};

/// Default highlight clipping threshold (sRGB-encoded 8-bit value, inclusive).
pub const DEFAULT_CLIP_HIGH: u8 = 254;
/// Default shadow clipping threshold (sRGB-encoded 8-bit value, inclusive).
pub const DEFAULT_CLIP_LOW: u8 = 1;

/// Focus-peaking edge threshold (Sobel magnitude of sRGB-encoded luma, 0‥4) for a
/// sensitivity in percent (1‥100). Higher sensitivity marks weaker edges.
pub fn peaking_threshold(sensitivity: u32) -> f32 {
    let s = sensitivity.clamp(1, 100) as f32 / 100.0;
    // 1 % → 1.2 (only hard, high-contrast edges), 100 % → 0.1 (almost any texture).
    1.2 - 1.1 * s
}

#[cfg(test)]
mod tests;
