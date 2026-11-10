//! Black & white: the picture as luminance, with a brightness mix per
//! hue band (the colour filters of black-and-white film, and Lightroom's B&W mix).
//!
//! Grey is Rec.2020 luminance Y, scaled by `2^(2 · mix(h) · m)`: `mix(h)` is the band
//! curve at the pixel's OkLab hue (the colour equalizer's smooth blend of eight band
//! values, each −1–1), `m` grows with saturation (C / L) up to 0.25, so colourful areas
//! move by up to ±2 EV and neutrals keep their luminance. The module is off by default;
//! when on, it always turns the picture grey.

use crate::graph::{NodeKind, Op};
use crate::model::{Fields, Params, Value};

use super::{BAND_KAPPA, BAND_WORDS, BANDS, num, write_bands};

/// Saturation (C / L) at which a pixel takes its band's full mix.
pub const FULL: f32 = 0.25;
/// EV per unit of mix at full saturation.
pub const EV: f32 = 2.0;

/// Black & white params (fields of [`crate::model::ModuleKind::BLACK_WHITE`], v1).
#[derive(Clone, Debug, PartialEq)]
pub struct BlackWhite {
    pub mix: [f32; 8],
}

impl BlackWhite {
    pub fn from_fields(f: &Fields) -> Self {
        Self {
            mix: BANDS.map(|b| num(f, &format!("mix_{b}"), 0.0, -1.0, 1.0)),
        }
    }

    pub fn defaults() -> Fields {
        let mut f = Fields::new();
        for b in BANDS {
            f.set(&format!("mix_{b}"), Value::Float(0.0));
        }
        f
    }

    /// κ, the saturation for the full mix, then the band nodes (mix in EV).
    pub fn uniforms(&self) -> Vec<u32> {
        let mut u = vec![0u32; WORDS];
        u[0] = BAND_KAPPA.to_bits();
        u[1] = FULL.to_bits();
        write_bands(&self.mix.map(|m| [m * EV, 0.0, 0.0]), &mut u[2..]);
        u
    }
}

pub const WORDS: usize = 2 + 8 * BAND_WORDS;

/// Black & white, param version 1.
pub struct BlackWhiteOp;

impl Op for BlackWhiteOp {
    fn name(&self) -> &'static str {
        "black_white_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Point
    }
    fn param_words(&self) -> u32 {
        WORDS as u32
    }
    fn write_params(&self, p: &Params, _: f32, out: &mut [u32]) {
        out.copy_from_slice(&BlackWhite::from_fields(&p.to_fields()).uniforms());
    }
    fn helpers(&self) -> &'static [&'static str] {
        static H: std::sync::LazyLock<[&'static str; 3]> = std::sync::LazyLock::new(|| {
            [
                *crate::colour::oklab::OKLAB_WGSL,
                super::super::tone::kernels::COMMON_WGSL,
                super::BANDS_WGSL,
            ]
        });
        &*H
    }
    fn wgsl(&self) -> &'static str {
        "    let y = dot(c.rgb, DV_LUMA);
    let lab = dv_oklab(c.rgb);
    let ch = length(lab.yz);
    var g = y;
    if (ch > 0.0) {
        let s = ch / max(abs(lab.x), 1e-6);
        let d = dv_bands(p + 2u, pf(p), lab.y / ch, lab.z / ch).x;
        g = y * exp2(d * smoothstep(0.0, pf(p + 1u), s));
    }
    return vec4<f32>(g, g, g, c.a);"
    }
}
