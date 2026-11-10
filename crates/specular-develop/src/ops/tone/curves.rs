//! RGB curves and levels, a point op before the view transform.
//!
//! - **Encoding.** Scene-linear values are shown on the curve as `v^(1/2.4)` (diffuse
//!   white 1.0 at the top right, mid grey near the middle) and decoded after; values above
//!   1 continue the curve with its end slope, values below 0 with its start slope, so
//!   nothing clips.
//! - **Levels** first: black and white points on that axis, and a grey point (0–1 between
//!   them) that a power curve sends to the middle.
//! - **Master curve** (`points`): the widget's monotone cubic (Fritsch–Carlson 1980, the
//!   same f32 arithmetic as `specular_widgets::curve`), so what is drawn is what renders.
//!   `preserve_colour` applies it to a norm and scales the three channels by the change:
//!   `luminance` (Rec.2020 Y, the default), `max` (the largest channel), or `none` (each
//!   channel on its own).
//! - **Channel curves** (`points_r`, `points_g`, `points_b`) then act on one channel each.
//! - Identity curves and levels skip their work: the defaults are the identity exactly.

use crate::graph::{NodeKind, Op};
use crate::model::{Fields, Params, Value};

/// Most points a curve holds (the widget's limit).
pub const MAX_POINTS: usize = 16;
/// Uniform words of one curve: count, then x, y and tangent of each point.
const CURVE_WORDS: usize = 1 + 3 * MAX_POINTS;

/// How the master curve keeps colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preserve {
    None,
    Luminance,
    Max,
}

/// Curves and levels params (fields of [`crate::model::ModuleKind::CURVES`], version 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Curves {
    pub master: Vec<[f32; 2]>,
    pub channels: [Vec<[f32; 2]>; 3],
    pub preserve: Preserve,
    pub levels_black: f32,
    pub levels_grey: f32,
    pub levels_white: f32,
}

/// The identity curve.
pub fn identity() -> Vec<[f32; 2]> {
    vec![[0.0, 0.0], [1.0, 1.0]]
}

/// A valid curve from a field (as the widget takes it), else the identity.
fn points(f: &Fields, name: &str) -> Vec<[f32; 2]> {
    let Some(Value::Floats(v)) = f.get(name) else {
        return identity();
    };
    let pts: Vec<[f32; 2]> = v.chunks_exact(2).map(|p| [p[0], p[1]]).collect();
    let ok = v.len() % 2 == 0
        && (2..=MAX_POINTS).contains(&pts.len())
        && pts[0][0] == 0.0
        && pts[pts.len() - 1][0] == 1.0
        && pts.windows(2).all(|w| w[1][0] > w[0][0])
        && pts
            .iter()
            .all(|p| p.iter().all(|c| c.is_finite() && (0.0..=1.0).contains(c)));
    if ok { pts } else { identity() }
}

impl Curves {
    pub fn from_fields(f: &Fields) -> Self {
        let lv = |n: &str, d: f32| {
            let v = f.f32_or(n, d);
            if v.is_finite() { v } else { d }
        };
        let black = lv("levels_black", 0.0).clamp(-0.5, 0.95);
        let white = lv("levels_white", 1.0).clamp(black + 0.05, 2.0);
        Self {
            master: points(f, "points"),
            channels: [
                points(f, "points_r"),
                points(f, "points_g"),
                points(f, "points_b"),
            ],
            preserve: match f.text("preserve_colour") {
                Some("none") => Preserve::None,
                Some("max") => Preserve::Max,
                _ => Preserve::Luminance,
            },
            levels_black: black,
            levels_grey: lv("levels_grey", 0.5).clamp(0.05, 0.95),
            levels_white: white,
        }
    }

    /// The levels' power: grey goes to the middle.
    pub fn gamma(&self) -> f32 {
        (0.5f64.ln() / f64::from(self.levels_grey).ln()) as f32
    }

    /// The kernel's uniforms.
    pub fn uniforms(&self) -> Vec<u32> {
        let mut u = vec![0u32; 4 + 4 * CURVE_WORDS];
        u[0] = match self.preserve {
            Preserve::None => 0,
            Preserve::Luminance => 1,
            Preserve::Max => 2,
        };
        u[1] = self.levels_black.to_bits();
        u[2] = self.levels_white.to_bits();
        u[3] = self.gamma().to_bits();
        for (i, c) in std::iter::once(&self.master)
            .chain(&self.channels)
            .enumerate()
        {
            write_curve(c, &mut u[4 + i * CURVE_WORDS..4 + (i + 1) * CURVE_WORDS]);
        }
        u
    }
}

/// The widget's Fritsch–Carlson tangents (the same f32 arithmetic).
pub fn tangents(pts: &[[f32; 2]]) -> Vec<f32> {
    let n = pts.len();
    let d: Vec<f32> = pts
        .windows(2)
        .map(|w| (w[1][1] - w[0][1]) / (w[1][0] - w[0][0]).max(f32::EPSILON))
        .collect();
    let mut m = vec![0.0f32; n];
    m[0] = d[0];
    m[n - 1] = d[n - 2];
    for i in 1..n - 1 {
        m[i] = if d[i - 1] * d[i] <= 0.0 {
            0.0
        } else {
            (d[i - 1] + d[i]) * 0.5
        };
    }
    for i in 0..n - 1 {
        if d[i] == 0.0 {
            m[i] = 0.0;
            m[i + 1] = 0.0;
            continue;
        }
        let a = m[i] / d[i];
        let b = m[i + 1] / d[i];
        let s = a * a + b * b;
        if s > 9.0 {
            let k = 3.0 / s.sqrt();
            m[i] = k * a * d[i];
            m[i + 1] = k * b * d[i];
        }
    }
    m
}

/// One curve's words: 0 points for the identity (skipped).
fn write_curve(pts: &[[f32; 2]], out: &mut [u32]) {
    if pts == identity().as_slice() {
        out[0] = 0;
        return;
    }
    let m = tangents(pts);
    out[0] = pts.len() as u32;
    for (i, (p, t)) in pts.iter().zip(&m).enumerate() {
        out[1 + i] = p[0].to_bits();
        out[1 + MAX_POINTS + i] = p[1].to_bits();
        out[1 + 2 * MAX_POINTS + i] = t.to_bits();
    }
}

/// Curves and levels, param version 1.
pub struct CurvesOp;

const CURVES_WGSL: &str = "fn dv_enc(v: f32) -> f32 {
    if (v > 0.0) { return pow(v, 1.0 / 2.4); }
    return v;
}
fn dv_dec(v: f32) -> f32 {
    if (v > 0.0) { return pow(v, 2.4); }
    return v;
}
fn dv_curve(o: u32, x: f32) -> f32 {
    let n = pu(o);
    let xs = o + 1u;
    let ys = o + 17u;
    let ms = o + 33u;
    if (x <= 0.0) { return pf(ys) + pf(ms) * x; }
    if (x >= 1.0) { return pf(ys + n - 1u) + pf(ms + n - 1u) * (x - 1.0); }
    var i = n - 2u;
    for (var j = 0u; j + 1u < n; j++) {
        if (x < pf(xs + j + 1u)) { i = j; break; }
    }
    let x0 = pf(xs + i);
    let h = pf(xs + i + 1u) - x0;
    let t = (x - x0) / h;
    let t2 = t * t;
    let t3 = t2 * t;
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;
    return h00 * pf(ys + i) + h10 * h * pf(ms + i) + h01 * pf(ys + i + 1u)
        + h11 * h * pf(ms + i + 1u);
}
fn dv_tone_curve(p: u32, v: f32, levels: bool) -> f32 {
    var x = dv_enc(v);
    if (levels) {
        x = (x - pf(p + 1u)) / (pf(p + 2u) - pf(p + 1u));
        if (x > 0.0) { x = pow(x, pf(p + 3u)); }
    }
    if (pu(p + 4u) != 0u) { x = dv_curve(p + 4u, x); }
    return dv_dec(x);
}
";

impl Op for CurvesOp {
    fn name(&self) -> &'static str {
        "curves_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Point
    }
    fn param_words(&self) -> u32 {
        (4 + 4 * CURVE_WORDS) as u32
    }
    fn identity(&self, p: &Params) -> bool {
        let c = Curves::from_fields(&p.to_fields());
        let id = identity();
        c.master == id
            && c.channels.iter().all(|ch| *ch == id)
            && c.levels_black == 0.0
            && c.levels_white == 1.0
            && c.gamma() == 1.0
    }
    fn write_params(&self, p: &Params, _: f32, out: &mut [u32]) {
        out.copy_from_slice(&Curves::from_fields(&p.to_fields()).uniforms());
    }
    fn helpers(&self) -> &'static [&'static str] {
        &[super::kernels::COMMON_WGSL, CURVES_WGSL]
    }
    fn wgsl(&self) -> &'static str {
        "    let levels = pf(p + 1u) != 0.0 || pf(p + 2u) != 1.0 || pf(p + 3u) != 1.0;
    var v = c.rgb;
    if (levels || pu(p + 4u) != 0u) {
        let mode = pu(p);
        if (mode == 0u) {
            v = vec3<f32>(dv_tone_curve(p, v.x, levels), dv_tone_curve(p, v.y, levels),
                dv_tone_curve(p, v.z, levels));
        } else {
            var nm = dot(v, DV_LUMA);
            if (mode == 2u) { nm = max(v.x, max(v.y, v.z)); }
            if (nm > 0.0) { v = v * (dv_tone_curve(p, nm, levels) / nm); }
        }
    }
    for (var ch = 0u; ch < 3u; ch++) {
        let o = p + 53u + ch * 49u;
        if (pu(o) != 0u) { v[ch] = dv_dec(dv_curve(o, dv_enc(v[ch]))); }
    }
    return vec4<f32>(v, c.a);"
    }
}
