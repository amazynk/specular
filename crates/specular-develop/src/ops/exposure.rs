//! Exposure: `(rgb − black_offset) · 2^exposure_ev` on scene-linear
//! values, alpha kept. Nothing is clipped.
//!
//! **Auto** (`auto`): the gain puts the `auto_percentile` of the scene's luminance (after
//! the black offset) on `auto_target_ev`, and `exposure_ev` adds to it as compensation.
//! The percentile comes from a histogram of the whole-image statistics grid (a global
//! statistic), so it is the same in every tile and at every view size.

use crate::graph::{NodeKind, Op, Program};
use crate::model::Params;
use crate::model::params::{Exposure, ModuleParams};
use crate::ops::tone::kernels::{self, BINS, fw};

/// The exposure op, param version 1.
pub struct ExposureOp;

fn finite(v: f32, lo: f32, hi: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { 0.0 }
}

/// The four uniforms: gain, black offset, auto (0 or 1), and in auto the EV the
/// percentile goes to (target plus compensation).
pub fn uniforms(p: &Params) -> [f32; 4] {
    let e = Exposure::from_fields(&p.to_fields());
    let ev = finite(e.exposure_ev, -20.0, 20.0);
    let black = if e.black_offset.is_finite() {
        e.black_offset
    } else {
        0.0
    };
    [
        (f64::from(ev).exp2()) as f32,
        black,
        if e.auto { 1.0 } else { 0.0 },
        ev + finite(e.auto_target_ev, -20.0, 20.0),
    ]
}

/// Statistics: words 0–1 the grid size, word 2 the luminance percentile in EV.
pub fn stats_program(p: &Params, w: u32, h: u32) -> Program {
    let e = Exposure::from_fields(&p.to_fields());
    let q = finite(e.auto_percentile, 0.0, 100.0) / 100.0;
    let black = if e.black_offset.is_finite() {
        e.black_offset
    } else {
        0.0
    };
    let hist = 16;
    let mut prog = Program {
        words: 16 + u64::from(BINS),
        steps: Vec::new(),
        persist: None,
    };
    prog.step(&kernels::PUT, [2, 1], vec![0, fw(w as f32), fw(h as f32)]);
    prog.step(&kernels::HIST_CLEAR, [BINS, 1], vec![hist]);
    kernels::hist_add(
        &mut prog,
        true,
        (w, h),
        [1, 0, 0, 0, w, hist, fw(-20.0), fw(12.0), fw(black)],
    );
    kernels::hist_pct(
        &mut prog,
        true,
        vec![hist, fw(-20.0), fw(12.0), 2, 1, fw(q)],
    );
    prog
}

impl Op for ExposureOp {
    fn name(&self) -> &'static str {
        "exposure_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Point
    }
    fn param_words(&self) -> u32 {
        4
    }
    fn write_params(&self, p: &Params, _: f32, out: &mut [u32]) {
        super::put(out, &uniforms(p));
    }
    fn has_stats(&self, p: &Params) -> bool {
        Exposure::from_fields(&p.to_fields()).auto
    }
    fn stats(&self, p: &Params, w: u32, h: u32, _: f32) -> Option<Program> {
        Some(stats_program(p, w, h))
    }
    fn wgsl(&self) -> &'static str {
        "    var g = pf(p);
    if (pf(p + 2u) != 0.0) { g = exp2(pf(p + 3u) - st(2u)); }
    return vec4<f32>((c.rgb - vec3<f32>(pf(p + 1u))) * g, c.a);"
    }
}
