//! Colour balance: four colour wheels and the global colour sliders, in
//! OkLab.
//!
//! - **Ranges.** Shadows, midtones and highlights are weights of the pixel's luminance in
//!   EV from mid grey (Rec.2020 Y / 0.1845): logistic steps centred at −2.5 EV (shadows)
//!   and +1.5 EV (highlights), 1 EV wide; midtones are what is left. Global weighs 1.
//! - **Wheels.** Each range has a hue (degrees on the wheel, the hue of the fully saturated
//!   sRGB colour the wheel shows there), a strength (0–1) and a luminance (−1–1). The
//!   luminance scales OkLab L by up to ±1 EV-equivalent (`2^(±1/3)`) with a and b, so
//!   saturation holds. The strength adds a tint `0.12 · strength · L` along the hue's
//!   direction: proportional to L, so black stays black and the tint holds across exposure.
//! - **Vibrance** scales chroma more on dull colours (fading out by saturation 0.5),
//!   **chroma** scales it evenly, **saturation** scales it and trades lightness against it
//!   (a quarter of the saturation change, so colours that gain saturation darken, as the
//!   Helmholtz–Kohlrausch effect makes them look brighter), **brilliance** brightens or
//!   darkens colourful pixels (up to ±1 EV, by saturation up to 0.3) and leaves neutrals.
//!   Saturation here is OkLab C / L, which exposure does not change.
//!
//! All chroma changes scale a and b together: hue never moves, neutrals stay neutral.

use crate::graph::{NodeKind, Op};
use crate::model::{Fields, Params};

use super::{hsv_direction, num};

/// The wheels' field-name prefixes, in uniform order.
pub const RANGES: [&str; 4] = ["shadows", "midtones", "highlights", "global"];

/// Tint per unit of wheel strength, in OkLab chroma per unit L.
pub const TINT: f32 = 0.12;

/// Colour balance params (fields of [`crate::model::ModuleKind::COLOUR_BALANCE`], v1).
#[derive(Clone, Debug, PartialEq)]
pub struct ColourBalance {
    /// Per range: hue (degrees), strength (0–1), luminance (−1–1).
    pub wheels: [[f32; 3]; 4],
    pub vibrance: f32,
    pub chroma: f32,
    pub saturation: f32,
    pub brilliance: f32,
}

impl ColourBalance {
    pub fn from_fields(f: &Fields) -> Self {
        let wheels = RANGES.map(|r| {
            let hue = f.f32_or(&format!("{r}_hue"), 0.0);
            [
                if hue.is_finite() {
                    hue.rem_euclid(360.0)
                } else {
                    0.0
                },
                num(f, &format!("{r}_chroma"), 0.0, 0.0, 1.0),
                num(f, &format!("{r}_luminance"), 0.0, -1.0, 1.0),
            ]
        });
        Self {
            wheels,
            vibrance: num(f, "vibrance", 0.0, -1.0, 1.0),
            chroma: num(f, "chroma", 0.0, -1.0, 1.0),
            saturation: num(f, "saturation", 0.0, -1.0, 1.0),
            brilliance: num(f, "brilliance", 0.0, -1.0, 1.0),
        }
    }

    /// The default fields (what the panel starts from).
    pub fn defaults() -> Fields {
        let mut f = Fields::new();
        for n in ["vibrance", "saturation", "chroma", "brilliance"] {
            f.set(n, crate::model::Value::Float(0.0));
        }
        for r in RANGES {
            for p in ["hue", "chroma", "luminance"] {
                f.set(&format!("{r}_{p}"), crate::model::Value::Float(0.0));
            }
        }
        f
    }

    pub fn is_identity(&self) -> bool {
        self.wheels.iter().all(|w| w[1] == 0.0 && w[2] == 0.0)
            && self.vibrance == 0.0
            && self.chroma == 0.0
            && self.saturation == 0.0
            && self.brilliance == 0.0
    }

    /// The kernel's uniforms: mask (3), per range tint a, b and log2 L gain (12), then
    /// vibrance, chroma, saturation, brilliance.
    pub fn uniforms(&self) -> [f32; WORDS] {
        let mut u = [0f32; WORDS];
        u[0] = 1.0; // 1 / width of the range steps (EV)
        u[1] = -2.5; // shadows centre (EV)
        u[2] = 1.5; // highlights centre (EV)
        for (i, w) in self.wheels.iter().enumerate() {
            let d = hsv_direction(f64::from(w[0]));
            let k = f64::from(TINT * w[1]);
            u[3 + i * 3] = (d[0] * k) as f32;
            u[4 + i * 3] = (d[1] * k) as f32;
            u[5 + i * 3] = w[2] / 3.0;
        }
        u[15] = self.vibrance;
        u[16] = self.chroma;
        u[17] = self.saturation;
        u[18] = self.brilliance / 3.0;
        u
    }
}

/// Uniform words of the op.
pub const WORDS: usize = 19;

/// Colour balance, param version 1.
pub struct ColourBalanceOp;

impl Op for ColourBalanceOp {
    fn name(&self) -> &'static str {
        "colour_balance_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Point
    }
    fn param_words(&self) -> u32 {
        WORDS as u32
    }
    fn identity(&self, p: &Params) -> bool {
        ColourBalance::from_fields(&p.to_fields()).is_identity()
    }
    fn write_params(&self, p: &Params, _: f32, out: &mut [u32]) {
        super::super::put(out, &ColourBalance::from_fields(&p.to_fields()).uniforms());
    }
    fn helpers(&self) -> &'static [&'static str] {
        static H: std::sync::LazyLock<[&'static str; 2]> = std::sync::LazyLock::new(|| {
            [
                *crate::colour::oklab::OKLAB_WGSL,
                super::super::tone::kernels::COMMON_WGSL,
            ]
        });
        &*H
    }
    fn wgsl(&self) -> &'static str {
        "    let y = dot(c.rgb, DV_LUMA);
    let ev = log2(max(y, 1e-9) / 0.1845);
    let ws = 1.0 / (1.0 + exp((ev - pf(p + 1u)) * pf(p)));
    let wh = 1.0 / (1.0 + exp(-(ev - pf(p + 2u)) * pf(p)));
    let wm = max(1.0 - ws - wh, 0.0);
    let wt = vec4<f32>(ws, wm, wh, 1.0);
    var ta = 0.0;
    var tb = 0.0;
    var gl = 0.0;
    for (var i = 0u; i < 4u; i++) {
        let q = p + 3u + i * 3u;
        ta += wt[i] * pf(q);
        tb += wt[i] * pf(q + 1u);
        gl += wt[i] * pf(q + 2u);
    }
    let lab = dv_oklab(c.rgb);
    let f = exp2(gl);
    let l = lab.x * f;
    var ab = lab.yz * f + vec2<f32>(ta, tb) * l;
    let s = length(ab) / max(abs(l), 1e-6);
    let fv = 1.0 + pf(p + 15u) * (1.0 - smoothstep(0.0, 0.5, s));
    let sat = pf(p + 17u);
    let k = max(fv * (1.0 + pf(p + 16u)) * (1.0 + sat), 0.0);
    let fl = (1.0 - 0.25 * sat * min(s, 1.0)) * exp2(pf(p + 18u) * smoothstep(0.0, 0.3, s));
    return vec4<f32>(dv_oklab_inv(vec3<f32>(l * fl, ab * (k * fl))), c.a);"
    }
}
