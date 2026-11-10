//! The colour equalizer: hue, saturation and brightness as curves over
//! hue, in OkLab.
//!
//! - **Curves.** Eight band nodes (red, orange, yellow, green, aqua, blue, purple, magenta,
//!   at the OkLab hues of those fully saturated sRGB colours) each hold a hue shift
//!   (degrees), a saturation change (−1–1, chroma × (1 + s)) and a brightness change
//!   (−1–1, up to ±1 EV-equivalent on L, with a and b). Between nodes the curve is a
//!   normalised von Mises blend (`exp(κ (cos Δh − 1))`): smooth, periodic, and flat where
//!   neighbouring nodes agree; its coefficients are solved so it passes through every
//!   node.
//! - **Hue mask.** The change fades in with saturation (OkLab C / L) from 0 to
//!   [`NEUTRAL`], so the noisy hue of near-neutral pixels moves nothing and neutrals stay
//!   neutral. A point op: the mask is smooth in colour, not in space.

use crate::graph::{NodeKind, Op};
use crate::model::{Fields, Params, Value};

use super::{BAND_KAPPA, BAND_WORDS, BANDS, num, write_bands};

/// Saturation (C / L) at which a pixel takes the full change.
pub const NEUTRAL: f32 = 0.08;

/// Colour equalizer params (fields of [`crate::model::ModuleKind::COLOUR_EQUALIZER`], v1):
/// per band hue shift (degrees, −45–45), saturation and brightness (−1–1).
#[derive(Clone, Debug, PartialEq)]
pub struct ColourEqualizer {
    pub bands: [[f32; 3]; 8],
}

impl ColourEqualizer {
    pub fn from_fields(f: &Fields) -> Self {
        Self {
            bands: BANDS.map(|b| {
                [
                    num(f, &format!("hue_{b}"), 0.0, -45.0, 45.0),
                    num(f, &format!("saturation_{b}"), 0.0, -1.0, 1.0),
                    num(f, &format!("brightness_{b}"), 0.0, -1.0, 1.0),
                ]
            }),
        }
    }

    pub fn defaults() -> Fields {
        let mut f = Fields::new();
        for p in ["hue", "saturation", "brightness"] {
            for b in BANDS {
                f.set(&format!("{p}_{b}"), Value::Float(0.0));
            }
        }
        f
    }

    pub fn is_identity(&self) -> bool {
        self.bands.iter().all(|b| b.iter().all(|v| *v == 0.0))
    }

    /// κ, the neutral edge, then the band nodes (hue shift in radians, saturation, log2 L).
    pub fn uniforms(&self) -> Vec<u32> {
        let mut u = vec![0u32; WORDS];
        u[0] = BAND_KAPPA.to_bits();
        u[1] = NEUTRAL.to_bits();
        let vals = self.bands.map(|b| [b[0].to_radians(), b[1], b[2] / 3.0]);
        write_bands(&vals, &mut u[2..]);
        u
    }
}

pub const WORDS: usize = 2 + 8 * BAND_WORDS;

/// The colour equalizer, param version 1.
pub struct ColourEqualizerOp;

impl Op for ColourEqualizerOp {
    fn name(&self) -> &'static str {
        "colour_equalizer_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Point
    }
    fn param_words(&self) -> u32 {
        WORDS as u32
    }
    fn identity(&self, p: &Params) -> bool {
        ColourEqualizer::from_fields(&p.to_fields()).is_identity()
    }
    fn write_params(&self, p: &Params, _: f32, out: &mut [u32]) {
        out.copy_from_slice(&ColourEqualizer::from_fields(&p.to_fields()).uniforms());
    }
    fn helpers(&self) -> &'static [&'static str] {
        static H: std::sync::LazyLock<[&'static str; 2]> =
            std::sync::LazyLock::new(|| [*crate::colour::oklab::OKLAB_WGSL, super::BANDS_WGSL]);
        &*H
    }
    fn wgsl(&self) -> &'static str {
        "    let lab = dv_oklab(c.rgb);
    let ch = length(lab.yz);
    if (ch <= 0.0) { return c; }
    let s = ch / max(abs(lab.x), 1e-6);
    let m = smoothstep(0.0, pf(p + 1u), s);
    let d = dv_bands(p + 2u, pf(p), lab.y / ch, lab.z / ch) * m;
    let cr = cos(d.x);
    let sr = sin(d.x);
    let fl = exp2(d.z);
    let k = max(1.0 + d.y, 0.0) * fl;
    let ab = vec2<f32>(lab.y * cr - lab.z * sr, lab.y * sr + lab.z * cr) * k;
    return vec4<f32>(dv_oklab_inv(vec3<f32>(lab.x * fl, ab)), c.a);"
    }
}
