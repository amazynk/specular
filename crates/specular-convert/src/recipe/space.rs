//! Output colour spaces of recipes: sRGB, Display P3, an Adobe RGB (1998) compatible space
//! and ProPhoto RGB (ROMM), each with an ICC v4 matrix/shaper profile written here.
//!
//! Linear light comes in BT.709 or Rec.2020 primaries (D65) and is converted through the
//! D50 connection space with Bradford adaptation, as an ICC colour managed reader does
//! with relative colorimetric intent, so a reader that honours the profile shows the same
//! colours. Values outside the target gamut are clipped per channel.

use specular_color::math::{Mat3, rgb_to_pcs};
use std::sync::OnceLock;

const D65: [f64; 2] = [0.3127, 0.3290];
const D50: [f64; 2] = [0.3457, 0.3585];

/// Primaries of the linear light handed to a recipe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Primaries {
    Bt709,
    Rec2020,
}

impl Primaries {
    fn to_pcs(self) -> Mat3 {
        match self {
            Primaries::Bt709 => rgb_to_pcs([0.64, 0.33], [0.30, 0.60], [0.15, 0.06], D65),
            Primaries::Rec2020 => rgb_to_pcs([0.708, 0.292], [0.170, 0.797], [0.131, 0.046], D65),
        }
    }
}

/// The colour space of an exported file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Space {
    Srgb,
    DisplayP3,
    /// Adobe RGB (1998) primaries and gamma 563/256, in a profile of our own.
    AdobeRgb,
    /// ROMM RGB (ISO 22028-2): wide primaries, D50, gamma 1.8 with a linear toe.
    ProPhoto,
}

impl Space {
    pub const ALL: [Space; 4] = [
        Space::Srgb,
        Space::DisplayP3,
        Space::AdobeRgb,
        Space::ProPhoto,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Space::Srgb => "srgb",
            Space::DisplayP3 => "display-p3",
            Space::AdobeRgb => "adobe-rgb",
            Space::ProPhoto => "prophoto",
        }
    }

    pub fn from_id(s: &str) -> Option<Space> {
        Space::ALL.into_iter().find(|v| v.id() == s)
    }

    /// CIE xy of red, green, blue and white.
    pub fn chromaticities(self) -> [[f64; 2]; 4] {
        match self {
            Space::Srgb => [[0.64, 0.33], [0.30, 0.60], [0.15, 0.06], D65],
            Space::DisplayP3 => [[0.680, 0.320], [0.265, 0.690], [0.150, 0.060], D65],
            Space::AdobeRgb => [[0.64, 0.33], [0.21, 0.71], [0.15, 0.06], D65],
            Space::ProPhoto => [[0.7347, 0.2653], [0.1596, 0.8404], [0.0366, 0.0001], D50],
        }
    }

    /// RGB → XYZ (D50 connection space).
    pub fn to_pcs(self) -> Mat3 {
        let [r, g, b, w] = self.chromaticities();
        rgb_to_pcs(r, g, b, w)
    }

    /// Linear light in `from` primaries → this space's linear RGB.
    pub fn from_linear(self, from: Primaries) -> [f32; 9] {
        let inv = self.to_pcs().inverse().unwrap_or(Mat3::IDENTITY);
        inv.mul(&from.to_pcs()).to_f32()
    }

    /// Linear (0..1) → stored (0..1).
    pub fn encode(self, l: f32) -> f32 {
        let l = l.clamp(0.0, 1.0);
        match self {
            Space::Srgb | Space::DisplayP3 => {
                if l <= 0.003_130_8 {
                    12.92 * l
                } else {
                    1.055 * l.powf(1.0 / 2.4) - 0.055
                }
            }
            Space::AdobeRgb => l.powf(256.0 / 563.0),
            Space::ProPhoto => {
                if l < 1.0 / 512.0 {
                    16.0 * l
                } else {
                    l.powf(1.0 / 1.8)
                }
            }
        }
    }

    /// Stored (0..1) → linear: the inverse of [`Space::encode`].
    pub fn decode(self, e: f32) -> f32 {
        let e = e.clamp(0.0, 1.0);
        match self {
            Space::Srgb | Space::DisplayP3 => {
                if e <= 0.04045 {
                    e / 12.92
                } else {
                    ((e + 0.055) / 1.055).powf(2.4)
                }
            }
            Space::AdobeRgb => e.powf(563.0 / 256.0),
            Space::ProPhoto => {
                if e < 16.0 / 512.0 {
                    e / 16.0
                } else {
                    e.powf(1.8)
                }
            }
        }
    }

    /// The ICC `parametricCurveType` values of the decoding direction.
    fn para(self) -> Vec<f64> {
        match self {
            Space::Srgb | Space::DisplayP3 => {
                vec![2.4, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.04045]
            }
            Space::AdobeRgb => vec![563.0 / 256.0],
            Space::ProPhoto => vec![1.8, 1.0, 0.0, 1.0 / 16.0, 16.0 / 512.0],
        }
    }

    /// The profile's description.
    pub fn description(self) -> &'static str {
        match self {
            Space::Srgb => "sRGB (Specular)",
            Space::DisplayP3 => "Display P3 (Specular)",
            Space::AdobeRgb => "Compatible with Adobe RGB (1998) (Specular)",
            Space::ProPhoto => "ProPhoto RGB (ROMM) (Specular)",
        }
    }

    /// The ICC v4 profile embedded in files of this space.
    pub fn icc(self) -> &'static [u8] {
        static P: [OnceLock<Vec<u8>>; 4] = [
            OnceLock::new(),
            OnceLock::new(),
            OnceLock::new(),
            OnceLock::new(),
        ];
        let i = Space::ALL.iter().position(|s| *s == self).unwrap_or(0);
        P[i].get_or_init(|| {
            specular_color::profiles::matrix_para_icc(
                &self.to_pcs(),
                &self.para(),
                self.description(),
            )
        })
    }
}
