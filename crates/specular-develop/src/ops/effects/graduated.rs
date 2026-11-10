//! Graduated ND: an exposure gradient, as a graduated neutral-density filter in front
//! of the lens, with an optional colour tint.
//!
//! - **Geometry.** A line through `centre` (fractions of the width and height) at `angle`
//!   degrees (0: horizontal, the full effect above it; positive turns clockwise on screen).
//!   Distances are in units of the long side, so the angle is the true one on any aspect.
//!   The effect is full `softness / 2` before the line and gone `softness / 2` after it, in a
//!   smooth step.
//! - **Effect.** `amount` EV (negative darkens, as an ND filter does) and a multiply by a
//!   tint of hue `tint_hue` (OkLab hue angle) at luminance 1, mixed in by `tint_amount`, both
//!   weighted by the gradient.
//! - The on-picture handles are the masks' linear-gradient handles: `start` is where the full
//!   effect ends and `end` where none is left ([`Graduated::ends`]).

use super::{FX_WGSL, get, smoothstep};
use crate::graph::{NodeKind, Op};
use crate::model::{Fields, Params, Value};

/// Graduated ND params (fields of [`crate::model::ModuleKind::GRADUATED_ND`], version 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Graduated {
    /// EV where the effect is full (−4 … 4).
    pub amount: f32,
    /// The middle of the transition, fractions of the width and height.
    pub centre: [f32; 2],
    /// Degrees (−180 … 180).
    pub angle: f32,
    /// Width of the transition, fraction of the long side (0 … 2).
    pub softness: f32,
    /// OkLab hue of the tint, degrees.
    pub tint_hue: f32,
    /// 0 … 1.
    pub tint_amount: f32,
}

impl Default for Graduated {
    fn default() -> Self {
        Self {
            amount: 0.0,
            centre: [0.5, 0.4],
            angle: 0.0,
            softness: 0.4,
            tint_hue: 250.0,
            tint_amount: 0.0,
        }
    }
}

impl Graduated {
    pub fn from_fields(f: &Fields) -> Self {
        let d = Self::default();
        let cf = f.floats_or("centre", d.centre);
        let c = |v: f32, dv: f32| {
            if v.is_finite() {
                v.clamp(-1.0, 2.0)
            } else {
                dv
            }
        };
        Self {
            amount: get(f, "amount", d.amount, -4.0, 4.0),
            centre: [c(cf[0], d.centre[0]), c(cf[1], d.centre[1])],
            angle: get(f, "angle", d.angle, -180.0, 180.0),
            softness: get(f, "softness", d.softness, 0.0, 2.0),
            tint_hue: get(f, "tint_hue", d.tint_hue, 0.0, 360.0),
            tint_amount: get(f, "tint_amount", d.tint_amount, 0.0, 1.0),
        }
    }

    pub fn default_fields() -> Fields {
        let d = Self::default();
        Fields::new()
            .with("amount", Value::Float(d.amount))
            .with("centre", Value::Floats(d.centre.to_vec()))
            .with("angle", Value::Float(d.angle))
            .with("softness", Value::Float(d.softness))
            .with("tint_hue", Value::Float(d.tint_hue))
            .with("tint_amount", Value::Float(d.tint_amount))
    }

    /// The writes `from_fields` reads back.
    pub fn write(&self, f: &mut Fields) {
        f.set("centre", Value::Floats(self.centre.to_vec()));
        f.set("angle", Value::Float(self.angle));
        f.set("softness", Value::Float(self.softness));
    }

    /// The direction from the full side to the clear side, in units of the long side
    /// (y down).
    pub fn direction(&self) -> [f64; 2] {
        let (s, c) = f64::from(self.angle).to_radians().sin_cos();
        [-s, c]
    }

    /// Where the full effect ends and where none is left, as points in fractions of a
    /// `w`×`h` frame.
    pub fn ends(&self, w: f64, h: f64) -> ([f64; 2], [f64; 2]) {
        let l = w.max(h).max(1e-9);
        let d = self.direction();
        let k = f64::from(self.softness) / 2.0;
        let c = [f64::from(self.centre[0]), f64::from(self.centre[1])];
        let at = |t: f64| [c[0] + d[0] * t * l / w, c[1] + d[1] * t * l / h];
        (at(-k), at(k))
    }

    /// The gradient through `start` and `end` (fractions of a `w`×`h` frame): centre,
    /// angle and softness. The amount and tint stay.
    pub fn set_ends(&mut self, start: [f64; 2], end: [f64; 2], w: f64, h: f64) {
        let l = w.max(h).max(1e-9);
        let d = [(end[0] - start[0]) * w / l, (end[1] - start[1]) * h / l];
        let len = d[0].hypot(d[1]);
        self.centre = [
            ((start[0] + end[0]) / 2.0) as f32,
            ((start[1] + end[1]) / 2.0) as f32,
        ];
        if len > 1e-6 {
            // direction = (−sin a, cos a).
            self.angle = (-d[0]).atan2(d[1]).to_degrees() as f32;
        }
        self.softness = (len as f32).clamp(0.0, 2.0);
    }

    /// The tint at luminance 1, linear Rec.2020.
    pub fn tint_rgb(&self) -> [f32; 3] {
        let (s, c) = f64::from(self.tint_hue).to_radians().sin_cos();
        let rgb = crate::colour::oklab::to_rec2020([0.75, 0.12 * c, 0.12 * s]);
        let rgb = rgb.map(|v| v.max(0.0));
        let y = 0.2627 * rgb[0] + 0.6780 * rgb[1] + 0.0593 * rgb[2];
        rgb.map(|v| (v / y.max(1e-9)) as f32)
    }

    /// The kernel's uniforms: amount, centre x, y, −sin, cos, half softness, tint r, g, b,
    /// tint amount.
    pub fn uniforms(&self) -> [f32; 10] {
        let d = self.direction();
        let t = self.tint_rgb();
        [
            self.amount,
            self.centre[0],
            self.centre[1],
            d[0] as f32,
            d[1] as f32,
            (self.softness / 2.0).max(1e-4),
            t[0],
            t[1],
            t[2],
            self.tint_amount,
        ]
    }
}

/// Graduated ND, param version 1.
pub struct GraduatedOp;

impl Op for GraduatedOp {
    fn name(&self) -> &'static str {
        "graduated_nd_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Point
    }
    fn param_words(&self) -> u32 {
        10
    }
    fn write_params(&self, p: &Params, _: f32, out: &mut [u32]) {
        crate::ops::put(out, &Graduated::from_fields(&p.to_fields()).uniforms());
    }
    fn helpers(&self) -> &'static [&'static str] {
        &[FX_WGSL, GRADUATED_WGSL]
    }
    fn identity(&self, p: &Params) -> bool {
        let g = Graduated::from_fields(&p.to_fields());
        g.amount == 0.0 && g.tint_amount == 0.0
    }
    fn wgsl(&self) -> &'static str {
        "    return dv_graduated(c, px, py, p);"
    }
}

const GRADUATED_WGSL: &str =
    "fn dv_graduated(c: vec4<f32>, px: i32, py: i32, p: u32) -> vec4<f32> {
    let q = dv_fx_pos(px, py, pf(p + 1u), pf(p + 2u));
    let s = dot(q, vec2<f32>(pf(p + 3u), pf(p + 4u)));
    let k = pf(p + 5u);
    let w = 1.0 - smoothstep(-k, k, s);
    let tint = mix(vec3<f32>(1.0), vec3<f32>(pf(p + 6u), pf(p + 7u), pf(p + 8u)), pf(p + 9u) * w);
    return vec4<f32>(c.rgb * exp2(pf(p) * w) * tint, c.a);
}
";

/// The CPU twin of the kernel at pixel (`px`, `py`) of a `w`×`h` image.
pub fn cpu_pixel(u: &[f32; 10], c: [f32; 4], px: i32, py: i32, w: u32, h: u32) -> [f32; 4] {
    let q = super::fx_pos(px, py, u[1], u[2], w, h);
    let s = q[0] * u[3] + q[1] * u[4];
    let wt = 1.0 - smoothstep(-u[5], u[5], s);
    let g = (u[0] * wt).exp2();
    let t = |i: usize| 1.0 + (u[6 + i] - 1.0) * (u[9] * wt);
    [c[0] * g * t(0), c[1] * g * t(1), c[2] * g * t(2), c[3]]
}
