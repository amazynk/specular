//! Depth blur: a lens blur driven by the photo's depth map.
//!
//! - **Circle of confusion.** A thin lens blurs a point by a disc whose radius grows with
//!   the difference of inverse depths from the focus plane; normalised depth is linear in
//!   inverse depth, so a pixel at depth `t` gets radius
//!   `R·min(1, max(0, |t − focus| − range/2) / 0.5)`, `R` = `amount` of the picture's long
//!   side. In-focus depths (within `range/2` of the focus) stay sharp; with `near` off,
//!   depths nearer than the focus stay sharp too.
//! - **Gather as a scatter.** Each output pixel looks at `n` taps on a golden-angle spiral
//!   over a disc of radius `R` (one of 16 turns of it by the pixel's place in a 4×4 tile,
//!   so the pattern breaks up into fine noise; the taps are whole pixel offsets made on the
//!   CPU and read from the table, so the GPU and the reference take the same taps) and takes a tap when the tap's own disc reaches it
//!   (a soft edge one tap spacing wide), weighted by 1/area so a wide disc spreads its light
//!   thin. A tap farther than the pixel counts with at most the pixel's own radius, so a
//!   sharp subject keeps its edge against a blurred background; nearer taps spread over it
//!   (a blurred foreground covers what is behind it). The pixel itself always counts.
//! - **Bright lights** above about 1 (scene-linear) gain weight by `highlights`, so they
//!   bloom into discs as a real lens draws them.
//!
//! The kernel reads the input within `R` px (its halo) and the depth table, so tiles are
//! the whole. A drag renders a draft with fewer taps.

use std::sync::Arc;

use super::super::effects::get;
use crate::graph::{NodeKind, Op, OpData, Placement};
use crate::mask::depth::DEPTH_WGSL;
use crate::model::mask::DEPTH_MODEL;
use crate::model::{Fields, ModuleKind, Params, Value};

/// Taps of a refined render, and of a draft.
pub const TAPS: u32 = 160;
pub const DRAFT_TAPS: u32 = 48;

/// Depth blur params (fields of [`ModuleKind::DEPTH_BLUR`], version 1).
#[derive(Clone, Debug, PartialEq)]
pub struct DepthBlur {
    /// The largest blur radius, a fraction of the long side (0 … 0.05).
    pub amount: f32,
    /// The depth in focus (0 nearest … 1 farthest).
    pub focus: f32,
    /// Depths within half of this of the focus stay sharp (0 … 1).
    pub range: f32,
    /// Extra weight of bright lights (0 … 1).
    pub highlights: f32,
    /// Blur what is nearer than the focus too.
    pub near: bool,
    /// The depth model (manifest id).
    pub model: String,
}

impl Default for DepthBlur {
    fn default() -> Self {
        Self {
            amount: 0.0,
            focus: 0.2,
            range: 0.1,
            highlights: 0.3,
            near: true,
            model: DEPTH_MODEL.to_owned(),
        }
    }
}

impl DepthBlur {
    pub fn from_fields(f: &Fields) -> Self {
        let d = Self::default();
        Self {
            amount: get(f, "amount", d.amount, 0.0, 0.05),
            focus: get(f, "focus", d.focus, 0.0, 1.0),
            range: get(f, "range", d.range, 0.0, 1.0),
            highlights: get(f, "highlights", d.highlights, 0.0, 1.0),
            near: f.bool_or("near", d.near),
            model: super::model_of(f),
        }
    }

    pub fn of(p: &Params) -> Self {
        Self::from_fields(&p.to_fields())
    }

    pub fn default_fields() -> Fields {
        let d = Self::default();
        Fields::new()
            .with("amount", Value::Float(d.amount))
            .with("focus", Value::Float(d.focus))
            .with("range", Value::Float(d.range))
            .with("highlights", Value::Float(d.highlights))
            .with("near", Value::Bool(d.near))
            .with("model", Value::Text(d.model))
    }
}

/// The largest radius in pixels of a placed module.
pub fn radius_px(f: &Fields) -> f32 {
    DepthBlur::from_fields(f).amount * super::image_long(f)
}

/// The kernel's uniforms: radius (px), focus, half range, highlights, near (0/1), taps.
pub fn uniforms(f: &Fields) -> [f32; 6] {
    let d = DepthBlur::from_fields(f);
    let taps = if f.bool_or(super::DRAFT, false) {
        DRAFT_TAPS
    } else {
        TAPS
    };
    [
        radius_px(f),
        d.focus,
        0.5 * d.range,
        d.highlights,
        if d.near { 1.0 } else { 0.0 },
        f32::from_bits(taps),
    ]
}

/// The circle of confusion (px) at depth `t` for uniforms `u`.
pub fn coc(u: &[f32; 6], t: f32) -> f32 {
    let d = t - u[1];
    if d < 0.0 && u[4] < 0.5 {
        return 0.0;
    }
    u[0] * ((d.abs() - u[2]).max(0.0) / 0.5).min(1.0)
}

/// Depth blur, param version 1.
pub struct DepthBlurOp;

impl Op for DepthBlurOp {
    fn name(&self) -> &'static str {
        "depth_blur_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Neighbourhood
    }
    fn heavy(&self) -> bool {
        true
    }
    fn halo(&self, p: &Params, _scale: f32) -> u32 {
        radius_px(&p.to_fields()).ceil() as u32 + 1
    }
    fn param_words(&self) -> u32 {
        6
    }
    fn write_params(&self, p: &Params, _: f32, out: &mut [u32]) {
        crate::ops::put(out, &uniforms(&p.to_fields()));
    }
    fn helpers(&self) -> &'static [&'static str] {
        &[DEPTH_WGSL, BLUR_WGSL]
    }
    fn identity(&self, p: &Params) -> bool {
        DepthBlur::of(p).amount <= 0.0
    }
    fn place(&self, p: &Params, at: &Placement<'_>) -> Option<Params> {
        Some(super::place(ModuleKind::DEPTH_BLUR, p, at))
    }
    fn data(&self, p: &Params) -> Option<Arc<OpData>> {
        let u = uniforms(&p.to_fields());
        let mut words = super::data_of(p).words.clone();
        words.extend(tap_table(u[0], u[5].to_bits()));
        Some(Arc::new(OpData::new(words)))
    }
    fn wgsl(&self) -> &'static str {
        "    return dv_depth_blur(x, y, p);"
    }
}

/// Turns of the tap spiral (a 4×4 tile of pixels takes one each).
pub const TURNS: u32 = 16;

/// The taps: for each of [`TURNS`] turns, `n` whole-pixel offsets (x, y) on a golden-angle
/// spiral over a disc of radius `r`.
pub fn tap_table(r: f32, n: u32) -> Vec<f32> {
    let mut v = Vec::with_capacity((2 * TURNS * n) as usize);
    for k in 0..TURNS {
        let rot = k as f64 / f64::from(TURNS) * std::f64::consts::TAU;
        for i in 0..n {
            let rho = f64::from(r) * ((f64::from(i) + 0.5) / f64::from(n)).sqrt();
            let a = f64::from(i) * 2.399_963_229_728_653 + rot;
            v.push((rho * a.cos()).round() as f32);
            v.push((rho * a.sin()).round() as f32);
        }
    }
    v
}

/// The turn pixel (`x`, `y`) takes.
pub fn turn(x: i32, y: i32) -> u32 {
    (x as u32 & 3) * 4 + (y as u32 & 3)
}

const BLUR_WGSL: &str = "fn dv_db_coc(p: u32, t: f32) -> f32 {
    let d = t - pf(p + 1u);
    if (d < 0.0 && pf(p + 4u) < 0.5) { return 0.0; }
    return pf(p) * min(max(abs(d) - pf(p + 2u), 0.0) / 0.5, 1.0);
}
fn dv_db_boost(p: u32, c: vec3<f32>) -> f32 {
    let l = dot(c, vec3<f32>(0.2627, 0.6780, 0.0593));
    return 1.0 + 3.0 * pf(p + 3u) * smoothstep(0.7, 3.0, l);
}
fn dv_depth_blur(x: i32, y: i32, p: u32) -> vec4<f32> {
    let c0 = src_at(x, y);
    let r = pf(p);
    let t0 = dv_depth_at(f32(x), f32(y));
    if (t0 < 0.0 || r < 0.5) { return c0; }
    let n = pu(p + 5u);
    let coc0 = dv_db_coc(p, t0);
    let sp = max(r * sqrt(3.14159265 / f32(n)), 1.0);
    var w = clamp(coc0 / sp + 0.5, 0.0, 1.0) / max(coc0 * coc0, sp * sp) * dv_db_boost(p, c0.rgb);
    var acc = c0.rgb * w;
    var ws = w;
    let tb = 2u + u32(dt(0u)) * u32(dt(1u)) + 2u * n * ((u32(x) & 3u) * 4u + (u32(y) & 3u));
    for (var i = 0u; i < n; i++) {
        let ox = dt(tb + 2u * i);
        let oy = dt(tb + 2u * i + 1u);
        let rho = sqrt(ox * ox + oy * oy);
        let sx = x + i32(ox);
        let sy = y + i32(oy);
        if (sx == x && sy == y) { continue; }
        let ts = dv_depth_at(f32(sx), f32(sy));
        var cs = dv_db_coc(p, ts);
        if (ts > t0) { cs = min(cs, coc0); }
        let k = clamp((cs - rho) / sp + 0.5, 0.0, 1.0);
        if (k <= 0.0) { continue; }
        let c = src_at(sx, sy);
        let wt = k / max(cs * cs, sp * sp) * dv_db_boost(p, c.rgb);
        acc += c.rgb * wt;
        ws += wt;
    }
    if (ws <= 0.0) { return c0; }
    return vec4<f32>(acc / ws, c0.a);
}
";
