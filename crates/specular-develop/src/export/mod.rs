//! Export: an edit rendered at full resolution through the same graph as
//! the preview, then written as a new file.
//!
//! - **Render** ([`render`]): the raw front end at full size (f32 storage when the device
//!   holds it), then the edit's graph with the output transform into the export space, in
//!   bands of rows read back as f32 (the engine tiles each band when it must). No separate
//!   code path: the same [`Graph::build_for`](crate::graph::Graph::build_for) and ops.
//! - **Resize** in linear light (Lanczos-3, streamed band by band), then the space's
//!   transfer curve, 8 or 16 bits, or linear float; then the turn (EXIF orientation and the
//!   user's quarter turns), so files are upright with orientation 1.
//! - **Spaces** ([`Space`]): sRGB, Display P3, Rec.2020 and an Adobe RGB (1998) compatible
//!   space, each with its own matrix/shaper ICC profile written here ([`Space::icc`]);
//!   float files are linear in the same primaries.
//! - **Files**: JPEG, PNG, AVIF, JPEG XL and 8/16-bit TIFF through `specular-convert`'s
//!   encoders; 32-bit float TIFF and OpenEXR (half or float) by its float writers. Exif and
//!   XMP are copied from the source (orientation reset, location optional), the profile
//!   embedded (EXR carries its chromaticities instead).
//! - **HDR** ([`hdr`]): PQ or HLG in AVIF, JPEG XL and PNG, or an SDR file with an ISO
//!   21496-1 gain map (JPEG, AVIF, JPEG XL), both renditions from the one edit.
//! - **Presets** ([`preset`]): size, format, quality, space, naming and folder, with a plain
//!   text form for the user's own.
//! - **Batch** ([`batch`]): a queue rendered one image at a time on the GPU while the
//!   previous one encodes, with progress, time left and Stop.

pub mod batch;
pub mod hdr;
mod hresize;
pub mod preset;
pub mod print;
mod quantise;
pub mod recipes;
pub mod render;
pub mod resample;

pub use batch::{Job, Outcome, Progress, Report};
pub use hdr::HdrOutput;
pub use preset::{Depth, FileFormat, Preset};
pub use render::{Exported, Exporter, Rendition, Samples};

use crate::colour::{self, M3};
use crate::ops::output::OutputSpace;

/// The colour space of an exported file (all D65).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Space {
    Srgb,
    DisplayP3,
    Rec2020,
    /// The Adobe RGB (1998) primaries and gamma 563/256, in a profile of our own.
    AdobeRgb,
}

/// Adobe RGB (1998) primaries.
const ADOBE_PRIMARIES: [[f64; 2]; 3] = [[0.64, 0.33], [0.21, 0.71], [0.15, 0.06]];
/// Adobe RGB (1998)'s exponent, 2 51/256.
const ADOBE_GAMMA: f64 = 563.0 / 256.0;

/// BT.2020's α and β (the precise pair, so the two pieces of the curve meet).
const BT_ALPHA: f64 = 1.099_296_826_809_44;
const BT_BETA: f64 = 0.018_053_968_510_807;

/// How linear values become stored ones.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Transfer {
    /// IEC 61966-2-1.
    Srgb,
    /// ITU-R BT.709 / BT.2020 (the 10-bit constants).
    Bt709,
    Gamma(f64),
    Linear,
}

impl Transfer {
    /// Linear (0..1) → stored (0..1).
    pub fn encode(self, l: f64) -> f64 {
        let l = l.clamp(0.0, 1.0);
        match self {
            Transfer::Srgb if l <= 0.003_130_8 => 12.92 * l,
            Transfer::Srgb => 1.055 * l.powf(1.0 / 2.4) - 0.055,
            Transfer::Bt709 if l < BT_BETA => 4.5 * l,
            Transfer::Bt709 => BT_ALPHA * l.powf(0.45) - (BT_ALPHA - 1.0),
            Transfer::Gamma(g) => l.powf(1.0 / g),
            Transfer::Linear => l,
        }
    }

    /// Stored (0..1) → linear: the inverse of [`Transfer::encode`] (tests, ΔE checks).
    pub fn decode(self, e: f64) -> f64 {
        let e = e.clamp(0.0, 1.0);
        match self {
            Transfer::Srgb if e <= 0.04045 => e / 12.92,
            Transfer::Srgb => ((e + 0.055) / 1.055).powf(2.4),
            Transfer::Bt709 if e < 4.5 * BT_BETA => e / 4.5,
            Transfer::Bt709 => ((e + BT_ALPHA - 1.0) / BT_ALPHA).powf(1.0 / 0.45),
            Transfer::Gamma(g) => e.powf(g),
            Transfer::Linear => e,
        }
    }

    /// The ICC `parametricCurveType` values of the decoding direction.
    fn para(self) -> Vec<f64> {
        match self {
            Transfer::Srgb => vec![2.4, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.04045],
            Transfer::Bt709 => vec![
                1.0 / 0.45,
                1.0 / BT_ALPHA,
                (BT_ALPHA - 1.0) / BT_ALPHA,
                1.0 / 4.5,
                4.5 * BT_BETA,
            ],
            Transfer::Gamma(g) => vec![g],
            Transfer::Linear => vec![1.0],
        }
    }
}

impl Space {
    pub const ALL: [Space; 4] = [
        Space::Srgb,
        Space::DisplayP3,
        Space::Rec2020,
        Space::AdobeRgb,
    ];

    /// Stable name (presets, the command line).
    pub fn id(self) -> &'static str {
        match self {
            Space::Srgb => "srgb",
            Space::DisplayP3 => "display-p3",
            Space::Rec2020 => "rec2020",
            Space::AdobeRgb => "adobe-rgb",
        }
    }

    pub fn from_id(s: &str) -> Option<Space> {
        Space::ALL.into_iter().find(|v| v.id() == s)
    }

    /// CIE xy of red, green and blue.
    pub fn primaries(self) -> [[f64; 2]; 3] {
        match self {
            Space::Srgb => colour::REC709_PRIMARIES,
            Space::DisplayP3 => colour::P3_PRIMARIES,
            Space::Rec2020 => colour::REC2020_PRIMARIES,
            Space::AdobeRgb => ADOBE_PRIMARIES,
        }
    }

    /// The curve of integer files (float files are linear).
    pub fn transfer(self) -> Transfer {
        match self {
            Space::Srgb | Space::DisplayP3 => Transfer::Srgb,
            Space::Rec2020 => Transfer::Bt709,
            Space::AdobeRgb => Transfer::Gamma(ADOBE_GAMMA),
        }
    }

    /// Linear Rec.2020 → this space's linear RGB.
    pub fn from_rec2020(self) -> M3 {
        let to = colour::rgb_to_xyz(&self.primaries(), colour::D65_XY);
        let inv = colour::inv(&to).unwrap_or(colour::IDENTITY);
        colour::mul(&inv, &colour::rec2020_to_xyz())
    }

    /// This space's linear RGB → linear Rec.2020.
    pub fn to_rec2020(self) -> M3 {
        colour::inv(&self.from_rec2020()).unwrap_or(colour::IDENTITY)
    }

    /// The graph's output transform into this space (SDR, gamut compressed).
    pub fn output(self) -> OutputSpace {
        OutputSpace {
            from_rec2020: crate::ops::m3_f32(&self.from_rec2020()),
            ..OutputSpace::rec709()
        }
    }

    /// The profile's description.
    pub fn description(self, linear: bool) -> &'static str {
        match (self, linear) {
            (Space::Srgb, false) => "sRGB (Specular)",
            (Space::Srgb, true) => "Linear sRGB (Specular)",
            (Space::DisplayP3, false) => "Display P3 (Specular)",
            (Space::DisplayP3, true) => "Linear Display P3 (Specular)",
            (Space::Rec2020, false) => "Rec. 2020 (Specular)",
            (Space::Rec2020, true) => "Linear Rec. 2020 (Specular)",
            (Space::AdobeRgb, false) => "Compatible with Adobe RGB (1998) (Specular)",
            (Space::AdobeRgb, true) => "Linear, Adobe RGB (1998) primaries (Specular)",
        }
    }

    /// The ICC v4 matrix/shaper profile embedded in files of this space (`linear` for
    /// float files).
    pub fn icc(self, linear: bool) -> Vec<u8> {
        let [r, g, b] = self.primaries();
        let m = specular_color::math::rgb_to_pcs(r, g, b, colour::D65_XY);
        let t = if linear {
            Transfer::Linear
        } else {
            self.transfer()
        };
        specular_color::profiles::matrix_para_icc(&m, &t.para(), self.description(linear))
    }

    /// The EXR `chromaticities` attribute: red, green, blue and white xy.
    pub fn chromaticities(self) -> [f32; 8] {
        let p = self.primaries();
        let w = colour::D65_XY;
        [
            p[0][0], p[0][1], p[1][0], p[1][1], p[2][0], p[2][1], w[0], w[1],
        ]
        .map(|v| v as f32)
    }
}

#[cfg(test)]
mod tests;
