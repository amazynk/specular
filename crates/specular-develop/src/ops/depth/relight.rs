//! Relight: an exposure gain by depth, in scene-linear light (colours keep their
//! ratios). Near parts get `near` EV, far parts `far` EV, blended over a band `transition`
//! wide around `split` (normalised depth, 0 nearest, 1 farthest). Without a depth map the
//! module changes nothing.

use std::sync::Arc;

use super::super::effects::get;
use crate::graph::{NodeKind, Op, OpData, Placement};
use crate::mask::depth::DEPTH_WGSL;
use crate::model::mask::DEPTH_MODEL;
use crate::model::{Fields, ModuleKind, Params, Value};

/// Relight params (fields of [`ModuleKind::RELIGHT`], version 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Relight {
    /// EV for the near part (−4 … 4).
    pub near: f32,
    /// EV for the far part (−4 … 4).
    pub far: f32,
    /// Where near ends and far begins (0 … 1).
    pub split: f32,
    /// Width of the blend between them (0.01 … 1).
    pub transition: f32,
    /// The depth model (manifest id).
    pub model: String,
}

impl Default for Relight {
    fn default() -> Self {
        Self {
            near: 0.0,
            far: 0.0,
            split: 0.5,
            transition: 0.3,
            model: DEPTH_MODEL.to_owned(),
        }
    }
}

impl Relight {
    pub fn from_fields(f: &Fields) -> Self {
        let d = Self::default();
        Self {
            near: get(f, "near", d.near, -4.0, 4.0),
            far: get(f, "far", d.far, -4.0, 4.0),
            split: get(f, "split", d.split, 0.0, 1.0),
            transition: get(f, "transition", d.transition, 0.01, 1.0),
            model: super::model_of(f),
        }
    }

    pub fn default_fields() -> Fields {
        let d = Self::default();
        Fields::new()
            .with("near", Value::Float(d.near))
            .with("far", Value::Float(d.far))
            .with("split", Value::Float(d.split))
            .with("transition", Value::Float(d.transition))
            .with("model", Value::Text(d.model))
    }

    /// The kernel's uniforms: near, far, band start, band end.
    pub fn uniforms(&self) -> [f32; 4] {
        let h = 0.5 * self.transition;
        [self.near, self.far, self.split - h, self.split + h]
    }

    /// The gain in EV at depth `t`.
    pub fn ev(&self, t: f32) -> f32 {
        let u = self.uniforms();
        let s = super::super::effects::smoothstep(u[2], u[3], t);
        u[0] + (u[1] - u[0]) * s
    }
}

/// Relight, param version 1.
pub struct RelightOp;

impl Op for RelightOp {
    fn name(&self) -> &'static str {
        "relight_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Point
    }
    fn param_words(&self) -> u32 {
        4
    }
    fn write_params(&self, p: &Params, _: f32, out: &mut [u32]) {
        crate::ops::put(out, &Relight::from_fields(&p.to_fields()).uniforms());
    }
    fn helpers(&self) -> &'static [&'static str] {
        &[DEPTH_WGSL, RELIGHT_WGSL]
    }
    fn identity(&self, p: &Params) -> bool {
        let r = Relight::from_fields(&p.to_fields());
        r.near == 0.0 && r.far == 0.0
    }
    fn place(&self, p: &Params, at: &Placement<'_>) -> Option<Params> {
        Some(super::place(ModuleKind::RELIGHT, p, at))
    }
    fn data(&self, p: &Params) -> Option<Arc<OpData>> {
        Some(super::data_of(p))
    }
    fn wgsl(&self) -> &'static str {
        "    return dv_relight(c, px, py, p);"
    }
}

const RELIGHT_WGSL: &str = "fn dv_relight(c: vec4<f32>, px: i32, py: i32, p: u32) -> vec4<f32> {
    let t = dv_depth_at(f32(px), f32(py));
    if (t < 0.0) { return c; }
    let s = smoothstep(pf(p + 2u), pf(p + 3u), t);
    let g = pf(p) + (pf(p + 1u) - pf(p)) * s;
    return vec4<f32>(c.rgb * exp2(g), c.a);
}
";
