//! Dehaze: the dark channel prior (He, Sun and Tang 2009). In haze-free outdoor
//! pictures most patches have a channel near zero; haze lifts it towards the atmospheric
//! light, so the dark channel measures the haze.
//!
//! On the whole-image statistics grid (at most 1024 px a side):
//! - **Dark channel** `D = min over a patch of min(R, G, B)`, the patch 2·r + 1 px with
//!   r = 1.2 % of the long side (He et al.'s 15 px on 600 px images).
//! - **Atmospheric light** `A`: the mean colour of the pixels whose dark channel is in the
//!   top 0.1 % (a histogram's 99.9th percentile).
//! - **Transmission** `t̃ = 1 − ω · min over the patch of min(I_c / A_c)`, ω = 0.95.
//! - **Refinement** by the guided filter (He, Sun and Tang 2013) guided by luminance over
//!   Y(A), in its fast form (He and Sun 2015): coefficients on the grid subsampled by
//!   s = r/4, radius 4 % of the long side, ε = 10⁻³; each pixel's `t = Ā·I + B̄` with its own
//!   full-resolution guide value, so edges stay sharp at 1:1.
//!
//! In the kernel, strength `k > 0` removes haze: `t_k = max(1 − k(1 − t), 0.1)`,
//! `J = (I − A)/t_k + A`. `k < 0` adds it: `J = I(1 − h) + A·h` with `h = 0.8·|k|·(0.25 +
//! 0.75(1 − t))`, more where the picture is already hazy (far away). The statistics hang
//! only on the module's input, so moving the slider re-runs just the kernel: the first frame
//! is already the refined one, well inside the heavy-op budget.

use super::kernels::{ATMOS, ATMOS_ROWS, DARK, DOWN, GUIDE, MIN};
use super::{FX_WGSL, get};
use crate::graph::{NodeKind, Op, Program};
use crate::model::{Fields, Params, Value};
use crate::ops::tone::kernels::{self as tk, Alloc, BINS, GRID_WGSL, fw};

/// Dehaze params (fields of [`crate::model::ModuleKind::DEHAZE`], version 1).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Dehaze {
    /// −1 (add haze) … 1 (remove it).
    pub strength: f32,
}

/// ω: a little haze is kept so far things still look far.
pub const OMEGA: f32 = 0.95;
/// The patch radius, fraction of the long side.
pub const PATCH: f32 = 0.012;
/// The guided filter's radius, fraction of the long side, and ε.
pub const GUIDED: f32 = 0.04;
pub const EPS: f32 = 1e-3;
/// The dark channel's histogram range (linear) and the quantile of the atmosphere.
pub const HIST_HI: f32 = 8.0;
pub const QUANTILE: f32 = 0.999;

/// Statistics layout: words 0–1 the coefficient grid's size, 4–6 A, 7 Y(A), 8 the dark
/// channel's threshold, Ā from 16 and B̄ after it.
pub const STATS_A: u32 = 4;
pub const STATS_COEFF: u32 = 16;

impl Dehaze {
    pub fn from_fields(f: &Fields) -> Self {
        Self {
            strength: get(f, "strength", 0.0, -1.0, 1.0),
        }
    }

    pub fn default_fields() -> Fields {
        Fields::new().with("strength", Value::Float(0.0))
    }
}

/// The patch radius, the guided filter's subsampling and its radius there, on a `w`×`h`
/// grid.
pub fn radii(w: u32, h: u32) -> (u32, u32, u32) {
    let l = w.max(h) as f32;
    let rp = ((PATCH * l).round() as u32).max(1);
    let rg = ((GUIDED * l).round() as u32).max(1);
    let s = (rg / 4).clamp(1, 8);
    let rc = ((rg as f32 / s as f32).round() as u32).max(1);
    (rp, s, rc)
}

/// The statistics program on a `w`×`h` grid.
pub fn stats_program(w: u32, h: u32) -> Program {
    let (rp, s, rc) = radii(w, h);
    let (cw, ch) = (w.div_ceil(s), h.div_ceil(s));
    let n = u64::from(cw) * u64::from(ch);
    let full = u64::from(w) * u64::from(h);
    let mut a = Alloc(u64::from(STATS_COEFF));
    let coeff = a.take(2 * n);
    let p0 = a.take(full);
    let p1 = a.take(full);
    let dark = a.take(full);
    let guide = a.take(full);
    let hist = a.take(u64::from(BINS));
    let rows = a.take(4 * u64::from(h));
    let (gi, gp) = if s > 1 {
        (a.take(n), a.take(n))
    } else {
        (guide, p1)
    };
    let mut prog = Program::default();
    prog.step(&tk::PUT, [2, 1], vec![0, fw(cw as f32), fw(ch as f32)]);
    // The dark channel.
    prog.step(&DARK, [w, h], vec![0, p0, w, STATS_A]);
    prog.step(&MIN, [w, h], vec![p0, p1, w, h, 0, rp, fw(0.0), fw(1.0)]);
    prog.step(&MIN, [w, h], vec![p1, dark, w, h, 1, rp, fw(0.0), fw(1.0)]);
    // The atmosphere: the top 0.1 % of the dark channel.
    prog.step(&tk::HIST_CLEAR, [BINS, 1], vec![hist]);
    tk::hist_add(
        &mut prog,
        true,
        (w, h),
        [0, dark, 0, 0, w, hist, fw(0.0), fw(HIST_HI), 0],
    );
    tk::hist_pct(
        &mut prog,
        true,
        vec![hist, fw(0.0), fw(HIST_HI), 8, 1, fw(QUANTILE)],
    );
    prog.step(
        &ATMOS_ROWS,
        [h, 1],
        vec![dark, w, 8, rows, fw(HIST_HI / BINS as f32)],
    );
    prog.step(&ATMOS, [1, 1], vec![rows, h, STATS_A]);
    // The transmission, then the guide.
    prog.step(&DARK, [w, h], vec![1, p0, w, STATS_A]);
    prog.step(&MIN, [w, h], vec![p0, dark, w, h, 0, rp, fw(0.0), fw(1.0)]);
    prog.step(
        &MIN,
        [w, h],
        vec![dark, p1, w, h, 1, rp, fw(1.0), fw(-OMEGA)],
    );
    prog.step(&GUIDE, [w, h], vec![guide, w, STATS_A + 3]);
    if s > 1 {
        prog.step(&DOWN, [cw, ch], vec![guide, w, h, gi, cw, s, 1, 0, 0]);
        prog.step(&DOWN, [cw, ch], vec![p1, w, h, gp, cw, s, 1, 0, 0]);
    }
    tk::guided(
        &mut prog,
        &mut a,
        gi,
        gp,
        false,
        coeff,
        (cw, ch),
        rc,
        EPS,
        true,
    );
    prog.words = a.0;
    prog
}

/// Dehaze, param version 1.
pub struct DehazeOp;

impl Op for DehazeOp {
    fn name(&self) -> &'static str {
        "dehaze_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Global
    }
    fn heavy(&self) -> bool {
        true
    }
    fn param_words(&self) -> u32 {
        1
    }
    fn write_params(&self, p: &Params, _: f32, out: &mut [u32]) {
        out[0] = fw(Dehaze::from_fields(&p.to_fields()).strength);
    }
    fn helpers(&self) -> &'static [&'static str] {
        &[FX_WGSL, GRID_WGSL, DEHAZE_WGSL]
    }
    fn identity(&self, p: &Params) -> bool {
        Dehaze::from_fields(&p.to_fields()).strength == 0.0
    }
    fn has_stats(&self, _: &Params) -> bool {
        true
    }
    fn stats(&self, _: &Params, w: u32, h: u32, _: f32) -> Option<Program> {
        Some(stats_program(w, h))
    }
    fn wgsl(&self) -> &'static str {
        "    return dv_dehaze(c, px, py, p);"
    }
}

const DEHAZE_WGSL: &str = "fn dv_dehaze(c: vec4<f32>, px: i32, py: i32, p: u32) -> vec4<f32> {
    let n = u32(st(0u)) * u32(st(1u));
    let a = vec3<f32>(st(4u), st(5u), st(6u));
    let g = clamp(dot(c.rgb, DV_FX_LUMA) / max(st(7u), 1e-4), 0.0, 2.0);
    let t = clamp(dv_grid(16u, px, py) * g + dv_grid(16u + n, px, py), 0.0, 1.0);
    let k = pf(p);
    if (k >= 0.0) {
        let tk = max(1.0 - k * (1.0 - t), 0.1);
        return vec4<f32>(max((c.rgb - a) / tk + a, vec3<f32>(0.0)), c.a);
    }
    let h = 0.8 * -k * (0.25 + 0.75 * (1.0 - t));
    return vec4<f32>(c.rgb * (1.0 - h) + a * h, c.a);
}
";

/// The CPU twin of the kernel: `st` the statistics, `ab` = (Ā, B̄) sampled at the pixel.
pub fn cpu_pixel(k: f32, c: [f32; 4], st: &[f32], ab: [f32; 2]) -> [f32; 4] {
    let a = [st[4], st[5], st[6]];
    let g = (super::luma([c[0], c[1], c[2]]) / st[7].max(1e-4)).clamp(0.0, 2.0);
    let t = (ab[0] * g + ab[1]).clamp(0.0, 1.0);
    if k >= 0.0 {
        let tk = (1.0 - k * (1.0 - t)).max(0.1);
        let j = |i: usize| ((c[i] - a[i]) / tk + a[i]).max(0.0);
        return [j(0), j(1), j(2), c[3]];
    }
    let h = 0.8 * -k * (0.25 + 0.75 * (1.0 - t));
    let j = |i: usize| c[i] * (1.0 - h) + a[i] * h;
    [j(0), j(1), j(2), c[3]]
}
