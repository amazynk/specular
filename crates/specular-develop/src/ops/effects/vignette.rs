//! Vignette: an exposure falloff towards the edges of the frame, in scene-linear
//! light (a gain in EV, so colours keep their ratios).
//!
//! - **Distance.** A pixel's offset from the centre in units of the half long side, divided
//!   by the frame's half extents (roundness 0: the shape follows the frame's aspect) or not
//!   (roundness 1: a circle), measured with a superellipse norm of exponent `2·4^shape`
//!   (shape −1 a diamond, 0 an ellipse, 1 nearly a rectangle) and scaled so the frame's
//!   corner is at 1.
//! - **Falloff.** A smooth step from `midpoint` to `midpoint + 0.02 + feather·(1.25 −
//!   midpoint)`; the gain is `amount` EV times it.
//! - **Highlight protection.** A darkening vignette spares pixels from 1.5 to 4.5 EV above
//!   mid grey by up to `protect`, so bright skies and lamps near the corners don't turn
//!   grey.
//! - The frame is the image the scene stage sees, which is after the crop: the vignette
//!   follows the crop. Amount 0 is the identity.

use super::{FX_WGSL, get, luma, smoothstep};
use crate::graph::{NodeKind, Op};
use crate::model::{Fields, Params, Value};

/// Vignette params (fields of [`crate::model::ModuleKind::VIGNETTE`], version 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vignette {
    /// EV at the corners (−4 … 4); negative darkens.
    pub amount: f32,
    /// Where the falloff starts, a fraction of the corner distance (0 … 1).
    pub midpoint: f32,
    /// 0: follows the frame's aspect; 1: a circle.
    pub roundness: f32,
    /// −1 … 1: diamond, ellipse, rectangle.
    pub shape: f32,
    /// Width of the falloff (0 … 1).
    pub feather: f32,
    /// Centre offset, fractions of the width and height (−0.5 … 0.5).
    pub centre: [f32; 2],
    /// How much bright pixels are spared from darkening (0 … 1).
    pub protect: f32,
}

impl Default for Vignette {
    fn default() -> Self {
        Self {
            amount: 0.0,
            midpoint: 0.5,
            roundness: 0.0,
            shape: 0.0,
            feather: 0.5,
            centre: [0.0, 0.0],
            protect: 0.0,
        }
    }
}

impl Vignette {
    pub fn from_fields(f: &Fields) -> Self {
        let d = Self::default();
        let cf = f.floats_or("centre", d.centre);
        let c = |v: f32| {
            if v.is_finite() {
                v.clamp(-0.5, 0.5)
            } else {
                0.0
            }
        };
        Self {
            amount: get(f, "amount", d.amount, -4.0, 4.0),
            midpoint: get(f, "midpoint", d.midpoint, 0.0, 1.0),
            roundness: get(f, "roundness", d.roundness, 0.0, 1.0),
            shape: get(f, "shape", d.shape, -1.0, 1.0),
            feather: get(f, "feather", d.feather, 0.0, 1.0),
            centre: [c(cf[0]), c(cf[1])],
            protect: get(f, "protect", d.protect, 0.0, 1.0),
        }
    }

    pub fn default_fields() -> Fields {
        let d = Self::default();
        Fields::new()
            .with("amount", Value::Float(d.amount))
            .with("midpoint", Value::Float(d.midpoint))
            .with("roundness", Value::Float(d.roundness))
            .with("shape", Value::Float(d.shape))
            .with("feather", Value::Float(d.feather))
            .with("centre", Value::Floats(d.centre.to_vec()))
            .with("protect", Value::Float(d.protect))
    }

    /// The kernel's uniforms: amount, exponent, roundness, inner, outer, centre x, y,
    /// protect.
    pub fn uniforms(&self) -> [f32; 8] {
        let inner = self.midpoint;
        let outer = self.midpoint + 0.02 + self.feather * (1.25 - self.midpoint);
        [
            self.amount,
            2.0 * 4f32.powf(self.shape),
            self.roundness,
            inner,
            outer,
            0.5 + self.centre[0],
            0.5 + self.centre[1],
            self.protect,
        ]
    }
}

/// Mid grey's EV and the protected band above it.
const PROTECT_LO: f32 = -2.473_931_2 + 1.5;
const PROTECT_HI: f32 = -2.473_931_2 + 4.5;

/// The vignette, param version 1.
pub struct VignetteOp;

impl Op for VignetteOp {
    fn name(&self) -> &'static str {
        "vignette_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Point
    }
    fn param_words(&self) -> u32 {
        8
    }
    fn write_params(&self, p: &Params, _: f32, out: &mut [u32]) {
        crate::ops::put(out, &Vignette::from_fields(&p.to_fields()).uniforms());
    }
    fn helpers(&self) -> &'static [&'static str] {
        &[FX_WGSL, VIGNETTE_WGSL]
    }
    fn identity(&self, p: &Params) -> bool {
        Vignette::from_fields(&p.to_fields()).amount == 0.0
    }
    fn wgsl(&self) -> &'static str {
        "    return dv_vignette(c, px, py, p);"
    }
}

const VIGNETTE_WGSL: &str = "fn dv_vg_norm(v: vec2<f32>, n: f32) -> f32 {
    let a = abs(v);
    let m = max(max(a.x, a.y), 1e-12);
    let q = a / m;
    return m * pow(pow(q.x, n) + pow(q.y, n), 1.0 / n);
}
fn dv_vignette(c: vec4<f32>, px: i32, py: i32, p: u32) -> vec4<f32> {
    let w = f32(pu(4u));
    let h = f32(pu(5u));
    let l = max(w, h);
    let half = vec2<f32>(w / l, h / l);
    let q = 2.0 * dv_fx_pos(px, py, pf(p + 5u), pf(p + 6u));
    let k = mix(half, vec2<f32>(1.0), pf(p + 2u));
    let n = pf(p + 1u);
    let r = dv_vg_norm(q / k, n) / dv_vg_norm(half / k, n);
    var g = pf(p) * smoothstep(pf(p + 3u), pf(p + 4u), r);
    if (g < 0.0 && pf(p + 7u) > 0.0) {
        let e = log2(max(dot(c.rgb, DV_FX_LUMA), 1.52587890625e-5));
        g = g * (1.0 - pf(p + 7u) * smoothstep(-0.9739312, 2.0260688, e));
    }
    return vec4<f32>(c.rgb * exp2(g), c.a);
}
";

fn norm(v: [f32; 2], n: f32) -> f32 {
    let a = [v[0].abs(), v[1].abs()];
    let m = a[0].max(a[1]).max(1e-12);
    let q = [a[0] / m, a[1] / m];
    m * (q[0].powf(n) + q[1].powf(n)).powf(1.0 / n)
}

/// The CPU twin of the kernel at pixel (`px`, `py`) of a `w`×`h` image.
pub fn cpu_pixel(u: &[f32; 8], c: [f32; 4], px: i32, py: i32, w: u32, h: u32) -> [f32; 4] {
    let l = (w as f32).max(h as f32);
    let half = [w as f32 / l, h as f32 / l];
    let pos = super::fx_pos(px, py, u[5], u[6], w, h);
    let q = [2.0 * pos[0], 2.0 * pos[1]];
    let k = [
        half[0] + (1.0 - half[0]) * u[2],
        half[1] + (1.0 - half[1]) * u[2],
    ];
    let n = u[1];
    let r = norm([q[0] / k[0], q[1] / k[1]], n) / norm([half[0] / k[0], half[1] / k[1]], n);
    let mut g = u[0] * smoothstep(u[3], u[4], r);
    if g < 0.0 && u[7] > 0.0 {
        let e = luma([c[0], c[1], c[2]]).max(1.525_878_9e-5).log2();
        g *= 1.0 - u[7] * smoothstep(PROTECT_LO, PROTECT_HI, e);
    }
    let k = g.exp2();
    [c[0] * k, c[1] * k, c[2] * k, c[3]]
}
