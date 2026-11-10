//! Glow: bloom and Orton-style soften. A blurred copy of the picture's bright parts
//! comes back over it.
//!
//! - **Bright pass.** Each pixel times `max(soft, Y − T)/Y`, `T` = `threshold` EV above mid
//!   grey, with a soft knee half as wide as `T`.
//! - **Blur.** Three box blurs (close to a Gaussian of σ = `radius` of the long side) on
//!   the whole-image statistics grid, subsampled so the boxes stay small (the fast guided
//!   filter's trick): the blur is the same in every view and tile, and the kernel samples it
//!   bilinearly.
//! - **Blend.** Bloom adds `amount ×` the blur (light spills, energy grows). Soften screens
//!   the blur over the picture on `x/(x + 0.18)` (the masks' 0…1 domain, so mid grey stays
//!   put and nothing clips) and mixes by `amount`: the dreamy Orton look.

use super::kernels::{BRIGHT, DOWN};
use super::{FX_WGSL, get};
use crate::graph::{NodeKind, Op, Program};
use crate::model::{Fields, Params, Value};
use crate::ops::tone::kernels::{self as tk, Alloc, GRID_WGSL, fw};

/// Glow params (fields of [`crate::model::ModuleKind::GLOW`], version 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glow {
    /// 0 … 1.
    pub amount: f32,
    /// Blur σ, fraction of the long side (0.002 … 0.1).
    pub radius: f32,
    /// EV above mid grey where the glow starts (−4 … 6).
    pub threshold: f32,
    pub mode: GlowMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlowMode {
    Bloom,
    Soften,
}

impl Default for Glow {
    fn default() -> Self {
        Self {
            amount: 0.0,
            radius: 0.02,
            threshold: 2.0,
            mode: GlowMode::Bloom,
        }
    }
}

/// Where the blurred planes start in the statistics.
pub const PLANES: u32 = 16;

impl Glow {
    pub fn from_fields(f: &Fields) -> Self {
        let d = Self::default();
        Self {
            amount: get(f, "amount", d.amount, 0.0, 1.0),
            radius: get(f, "radius", d.radius, 0.002, 0.1),
            threshold: get(f, "threshold", d.threshold, -4.0, 6.0),
            mode: if f.text("mode") == Some("soften") {
                GlowMode::Soften
            } else {
                GlowMode::Bloom
            },
        }
    }

    pub fn default_fields() -> Fields {
        let d = Self::default();
        Fields::new()
            .with("amount", Value::Float(d.amount))
            .with("radius", Value::Float(d.radius))
            .with("threshold", Value::Float(d.threshold))
            .with("mode", Value::Text("bloom".into()))
    }

    /// The subsampling of a `w`×`h` grid and the box radius there: σ on the grid, at most
    /// about 8 subsampled pixels, three boxes of radius r have variance r(r + 1).
    pub fn boxes(&self, w: u32, h: u32) -> (u32, u32) {
        let sigma = self.radius * w.max(h) as f32;
        let s = ((sigma / 8.0).floor() as u32).clamp(1, 16);
        let sc = sigma / s as f32;
        let r = (((1.0 + 4.0 * sc * sc).sqrt() - 1.0) / 2.0).round() as u32;
        (s, r.max(1))
    }
}

/// The statistics program on a `w`×`h` grid: words 0–1 the blurred grid's size, its three
/// planes from [`PLANES`].
pub fn stats_program(g: &Glow, w: u32, h: u32) -> Program {
    let (s, r) = g.boxes(w, h);
    let (cw, ch) = (w.div_ceil(s), h.div_ceil(s));
    let n = u64::from(cw) * u64::from(ch);
    let full = u64::from(w) * u64::from(h);
    let mut a = Alloc(u64::from(PLANES));
    let out = a.take(3 * n);
    let bright = a.take(3 * full);
    let x = a.take(3 * n);
    let y = a.take(3 * n);
    let t = 0.18 * g.threshold.exp2();
    let mut prog = Program::default();
    prog.step(&tk::PUT, [2, 1], vec![0, fw(cw as f32), fw(ch as f32)]);
    prog.step(
        &BRIGHT,
        [w, h],
        vec![bright, w, full as u32, fw(t), fw(0.5 * t)],
    );
    prog.step(
        &DOWN,
        [cw, ch],
        vec![bright, w, h, x, cw, s, 3, full as u32, n as u32],
    );
    let ps = n as u32;
    // x → y → x → y → x → out.
    let mut from = x;
    for pass in 0..3 {
        prog.step(&tk::BOX_H, [cw, ch], vec![from, y, cw, ch, r, 3, ps, 0, 0]);
        let to = if pass == 2 { out } else { x };
        prog.step(&tk::BOX_V, [cw, ch], vec![y, to, cw, ch, r, 3, ps, 0, 0]);
        from = to;
    }
    prog.words = a.0;
    prog
}

/// Glow, param version 1.
pub struct GlowOp;

impl Op for GlowOp {
    fn name(&self) -> &'static str {
        "glow_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Global
    }
    fn param_words(&self) -> u32 {
        2
    }
    fn write_params(&self, p: &Params, _: f32, out: &mut [u32]) {
        let g = Glow::from_fields(&p.to_fields());
        out[0] = fw(g.amount);
        out[1] = u32::from(g.mode == GlowMode::Soften);
    }
    fn helpers(&self) -> &'static [&'static str] {
        &[FX_WGSL, GRID_WGSL, GLOW_WGSL]
    }
    fn identity(&self, p: &Params) -> bool {
        Glow::from_fields(&p.to_fields()).amount == 0.0
    }
    fn has_stats(&self, _: &Params) -> bool {
        true
    }
    fn stats(&self, p: &Params, w: u32, h: u32, _: f32) -> Option<Program> {
        Some(stats_program(&Glow::from_fields(&p.to_fields()), w, h))
    }
    fn wgsl(&self) -> &'static str {
        "    return dv_glow(c, px, py, p);"
    }
}

const GLOW_WGSL: &str = "fn dv_gl_f(x: vec3<f32>) -> vec3<f32> {
    let v = max(x, vec3<f32>(0.0));
    return v / (v + 0.18);
}
fn dv_glow(c: vec4<f32>, px: i32, py: i32, p: u32) -> vec4<f32> {
    let n = u32(st(0u)) * u32(st(1u));
    let g = vec3<f32>(dv_grid(16u, px, py), dv_grid(16u + n, px, py), dv_grid(16u + 2u * n, px, py));
    let a = pf(p);
    if (pu(p + 1u) == 0u) {
        return vec4<f32>(c.rgb + a * g, c.a);
    }
    let f = 1.0 - (1.0 - dv_gl_f(c.rgb)) * (1.0 - dv_gl_f(g));
    let s = 0.18 * f / max(1.0 - f, vec3<f32>(1e-6));
    return vec4<f32>(mix(c.rgb, s, a), c.a);
}
";

/// The CPU twin of the kernel (`grid(o)` samples statistics plane `o` at the pixel).
pub fn cpu_pixel(amount: f32, soften: bool, c: [f32; 4], g: [f32; 3]) -> [f32; 4] {
    if !soften {
        return [
            c[0] + amount * g[0],
            c[1] + amount * g[1],
            c[2] + amount * g[2],
            c[3],
        ];
    }
    let f = |x: f32| {
        let v = x.max(0.0);
        v / (v + 0.18)
    };
    let out = |i: usize| {
        let s = 1.0 - (1.0 - f(c[i])) * (1.0 - f(g[i]));
        let x = 0.18 * s / (1.0 - s).max(1e-6);
        c[i] + (x - c[i]) * amount
    };
    [out(0), out(1), out(2), c[3]]
}
