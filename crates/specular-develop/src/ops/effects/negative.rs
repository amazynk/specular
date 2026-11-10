//! Negative inversion: a scanned or photographed film negative made a positive,
//! with a Cineon-style density model, as a darkroom print would.
//!
//! - **Density.** Each channel's transmittance relative to the film base (the unexposed
//!   rebate, picked from the picture or typed in; for colour film it holds the orange
//!   mask): `d = −log10(c / base)`, the density the exposure added (D-min is the base). Black
//!   and white film uses the luminance's density for all three channels.
//! - **Print.** The paper's exposure in log10 units is `x = d_max − d + exposure·log10 2`:
//!   the densest negative (the brightest scene) gives paper white. The paper's density
//!   rises from a soft toe (`softplus` with k = 4) with contrast `γ = 1.5 · 1.25^(grade −
//!   2)` and saturates at the paper's maximum density, 1.7 (matte) to 2.3 (gloss):
//!   `D = Dmax · (1 − exp(−γ·s(x)/Dmax))`.
//! - **Output.** Reflectance `10^(−D)`: paper white is 1, the deepest black `10^(−Dmax)`,
//!   linear light that later modules and the view transform treat as any scene.
//!
//! The model follows Kodak's Cineon description of printing density and darktable's public
//! description of negadoctor's controls (film base, D-min, paper grade, gloss).

use super::{get, luma};
use crate::graph::{NodeKind, Op};
use crate::model::{Fields, Params, Value};

/// Negative inversion params (fields of [`crate::model::ModuleKind::NEGATIVE`], version 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Negative {
    /// The film base (D-min) as linear RGB as the module sees it.
    pub base: [f32; 3],
    /// Density range of the negative the print spans (0.5 … 4).
    pub d_max: f32,
    /// Print exposure, EV (−4 … 4): positive darkens the print.
    pub exposure: f32,
    /// Paper grade (0 … 5).
    pub grade: f32,
    /// 0 matte … 1 gloss.
    pub gloss: f32,
    /// Black-and-white film.
    pub black_white: bool,
}

impl Default for Negative {
    fn default() -> Self {
        Self {
            base: [1.0, 1.0, 1.0],
            d_max: 1.5,
            exposure: 0.0,
            grade: 2.0,
            gloss: 0.5,
            black_white: false,
        }
    }
}

impl Negative {
    pub fn from_fields(f: &Fields) -> Self {
        let d = Self::default();
        let b = f.floats_or("base", d.base);
        let b = b.map(|v| {
            if v.is_finite() {
                v.clamp(1e-4, 64.0)
            } else {
                1.0
            }
        });
        Self {
            base: b,
            d_max: get(f, "d_max", d.d_max, 0.5, 4.0),
            exposure: get(f, "exposure", d.exposure, -4.0, 4.0),
            grade: get(f, "grade", d.grade, 0.0, 5.0),
            gloss: get(f, "gloss", d.gloss, 0.0, 1.0),
            black_white: f.bool_or("black_white", d.black_white),
        }
    }

    pub fn default_fields() -> Fields {
        let d = Self::default();
        Fields::new()
            .with("base", Value::Floats(d.base.to_vec()))
            .with("d_max", Value::Float(d.d_max))
            .with("exposure", Value::Float(d.exposure))
            .with("grade", Value::Float(d.grade))
            .with("gloss", Value::Float(d.gloss))
            .with("black_white", Value::Bool(d.black_white))
    }

    /// The paper's contrast.
    pub fn gamma(&self) -> f32 {
        1.5 * 1.25f32.powf(self.grade - 2.0)
    }

    /// The paper's maximum density.
    pub fn paper_dmax(&self) -> f32 {
        1.7 + 0.6 * self.gloss
    }

    /// The kernel's uniforms: 1/base (3), d_max + exposure·log10 2, γ, paper Dmax,
    /// 1/Y(base), black and white (0/1).
    pub fn uniforms(&self) -> [f32; 8] {
        [
            1.0 / self.base[0],
            1.0 / self.base[1],
            1.0 / self.base[2],
            self.d_max + self.exposure * std::f32::consts::LOG10_2,
            self.gamma(),
            self.paper_dmax(),
            1.0 / luma(self.base).max(1e-6),
            if self.black_white { 1.0 } else { 0.0 },
        ]
    }
}

/// Negative inversion, param version 1.
pub struct NegativeOp;

impl Op for NegativeOp {
    fn name(&self) -> &'static str {
        "negative_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Point
    }
    fn param_words(&self) -> u32 {
        8
    }
    fn write_params(&self, p: &Params, _: f32, out: &mut [u32]) {
        crate::ops::put(out, &Negative::from_fields(&p.to_fields()).uniforms());
    }
    fn helpers(&self) -> &'static [&'static str] {
        &[super::FX_WGSL, NEGATIVE_WGSL]
    }
    fn wgsl(&self) -> &'static str {
        "    return dv_negative(c, p);"
    }
}

const NEGATIVE_WGSL: &str = "fn dv_ng_print(t: f32, p: u32) -> f32 {
    let d = -log2(max(t, 1e-6)) * 0.30103;
    let x = pf(p + 3u) - d;
    let s = select(log(1.0 + exp(4.0 * x)) * 0.25, x + log(1.0 + exp(-4.0 * x)) * 0.25, x > 0.0);
    let dm = pf(p + 5u);
    let dp = dm * (1.0 - exp(-pf(p + 4u) * s / dm));
    return exp2(-dp * 3.3219281);
}
fn dv_negative(c: vec4<f32>, p: u32) -> vec4<f32> {
    if (pf(p + 7u) != 0.0) {
        let v = dv_ng_print(dot(c.rgb, DV_FX_LUMA) * pf(p + 6u), p);
        return vec4<f32>(v, v, v, c.a);
    }
    return vec4<f32>(
        dv_ng_print(c.r * pf(p), p),
        dv_ng_print(c.g * pf(p + 1u), p),
        dv_ng_print(c.b * pf(p + 2u), p),
        c.a);
}
";

fn print(t: f32, u: &[f32; 8]) -> f32 {
    let d = -t.max(1e-6).log2() * std::f32::consts::LOG10_2;
    let x = u[3] - d;
    let s = if x > 0.0 {
        x + (1.0 + (-4.0 * x).exp()).ln() * 0.25
    } else {
        (1.0 + (4.0 * x).exp()).ln() * 0.25
    };
    let dm = u[5];
    let dp = dm * (1.0 - (-u[4] * s / dm).exp());
    (-dp * std::f32::consts::LOG2_10).exp2()
}

/// The CPU twin of the kernel.
pub fn cpu_pixel(u: &[f32; 8], c: [f32; 4]) -> [f32; 4] {
    if u[7] != 0.0 {
        let v = print(luma([c[0], c[1], c[2]]) * u[6], u);
        return [v, v, v, c[3]];
    }
    [
        print(c[0] * u[0], u),
        print(c[1] * u[1], u),
        print(c[2] * u[2], u),
        c[3],
    ]
}
