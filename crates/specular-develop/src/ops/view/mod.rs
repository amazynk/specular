//! The view transforms (the neutral sigmoid is
//! the default look). Scene-linear Rec.2020 in, display-linear Rec.2020 out,
//! with display white at `white_target` (times the HDR headroom, [`for_headroom`]) and
//! black at `black_target`. Three methods share one op and one kernel (the method is a
//! uniform, so switching never builds a pipeline): the sigmoid here, the filmic-style and
//! AgX-style curves in [`curves`]. All three keep scene mid grey on display mid grey.
//!
//! The sigmoid's curve is a generalised log-logistic (Naka–Rushton family):
//!
//! ```text
//! F(x) = (1 / (1 + (k / x)^g))^s        x > 0;  F(x) = 0 otherwise
//! y    = black + (white − black) · F(x)
//! ```
//!
//! - `g` is the contrast (slope in log space around the middle);
//! - `s = 2^skew` shapes the toe against the shoulder: skew > 0 deepens the shadows and
//!   opens the highlights, skew < 0 the reverse;
//! - `k` is solved so that scene mid grey (0.1845) lands on display 0.1845 whatever the
//!   other settings, so contrast, whites and blacks never shift the exposure.
//!
//! The curve never reaches white: scene values approach it smoothly, with no clipping.
//!
//! **Hue.** Per channel, the curve shifts hues and runs bright colours to white;
//! `hue_preservation` blends towards the ratio-keeping form, where the largest and smallest
//! channels go through the curve and the middle one keeps its relative position between
//! them (so hue is kept). **Primaries inset / outset**: before the curve the colours are
//! pulled towards their own luminance by `inset` (a matrix that keeps white), after it they
//! are pushed out by `outset` (the inverse form), which gives smoother, less garish
//! highlights on saturated light.
//!
//! The filmic-style method uses the same hue handling; the AgX-style one always works per
//! channel on its own inset primaries ([`curves::agx_matrices`]), its point being the
//! smooth path of bright saturated colours to white.
//!
//! **HDR.** One edit renders both an SDR and an HDR look:
//! [`headroom`] is the display's (or the export's) peak capped by the edit's HDR limit,
//! and [`for_headroom`] scales the white target by it, so mid grey and the shadows stay
//! put and only the highlights gain room. Headroom 1 is the SDR rendition.

pub mod curves;
#[cfg(test)]
mod tests;

use std::borrow::Cow;

use crate::colour::M3;
use crate::graph::{NodeKind, Op};
use crate::model::params::{ModuleParams, ViewMethod, ViewTransform};
use crate::model::{Edit, ModuleKind, Params};

/// Scene and display mid grey.
pub const GREY: f64 = 0.1845;

/// Largest HDR limit, in EV above SDR white.
pub const MAX_HDR_LIMIT_EV: f32 = 6.0;

/// The view transform, param version 1 (all three methods).
pub struct ViewOp;

/// The sigmoid's constants: contrast `g`, scale `k`, skew power `s`, white, black.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Curve {
    pub g: f64,
    pub k: f64,
    pub s: f64,
    pub white: f64,
    pub black: f64,
}

impl Curve {
    pub fn new(v: &ViewTransform) -> Curve {
        let fin = |x: f32, d: f64| if x.is_finite() { f64::from(x) } else { d };
        let g = fin(v.contrast, 1.5).clamp(0.1, 10.0);
        let s = fin(v.skew, 0.0).clamp(-4.0, 4.0).exp2();
        let black = fin(v.black_target, 0.0).clamp(-1.0, 0.5);
        let white = fin(v.white_target, 1.0).clamp(black + 0.05, 1000.0);
        let r = ((GREY - black) / (white - black)).clamp(0.01, 0.99);
        let q = r.powf(1.0 / s);
        let k = GREY * ((1.0 - q) / q).powf(1.0 / g);
        Curve {
            g,
            k,
            s,
            white,
            black,
        }
    }

    /// The curve at one scene value.
    pub fn eval(&self, x: f64) -> f64 {
        let f = if x > 0.0 {
            (1.0 / (1.0 + (self.k / x).powf(self.g))).powf(self.s)
        } else {
            0.0
        };
        self.black + (self.white - self.black) * f
    }
}

/// Inset (`a`) and outset (`o`) matrices about Rec.2020 luminance.
pub fn inset_outset(a: f64, o: f64) -> (M3, M3) {
    let w = super::tone::LUMA.map(f64::from);
    let a = a.clamp(0.0, 0.9);
    let o = o.clamp(0.0, 0.9);
    let inset = std::array::from_fn(|i| {
        std::array::from_fn(|j| (1.0 - a) * f64::from(u8::from(i == j)) + a * w[j])
    });
    let outset = std::array::from_fn(|i| {
        std::array::from_fn(|j| f64::from(u8::from(i == j)) / (1.0 - o) - o / (1.0 - o) * w[j])
    });
    (inset, outset)
}

/// Number of uniforms.
pub const WORDS: usize = 32;

/// The uniforms: inset (9), outset (9), method (0 sigmoid, 1 filmic, 2 AgX), hue
/// preservation, then 12 curve constants (the sigmoid uses the first five: g, k, s,
/// white, black).
pub fn uniforms(p: &Params) -> [f32; WORDS] {
    let v = ViewTransform::from_fields(&p.to_fields());
    let fin = |x: f32, d: f32| if x.is_finite() { x } else { d };
    let inset = f64::from(fin(v.inset, 0.0));
    let outset = f64::from(fin(v.outset, 0.0));
    let c = Curve::new(&v);
    let skew = c.s.log2();
    let (mi, mo, method, hue, curve) = match v.method {
        ViewMethod::Sigmoid => {
            let (mi, mo) = inset_outset(inset, outset);
            let mut k = [0f32; 12];
            k[..5].copy_from_slice(&[c.g, c.k, c.s, c.white, c.black].map(|x| x as f32));
            (mi, mo, 0.0, fin(v.hue_preservation, 1.0), k)
        }
        ViewMethod::Filmic => {
            let (mi, mo) = inset_outset(inset, outset);
            let f = curves::Filmic::new(c.g, skew, c.white, c.black);
            (mi, mo, 1.0, fin(v.hue_preservation, 1.0), f.uniforms())
        }
        ViewMethod::Agx => {
            let (mi, mo) = curves::agx_matrices(inset, outset);
            let a = curves::Agx::new(c.g, skew, c.white, c.black);
            (mi, mo, 2.0, 0.0, a.uniforms())
        }
    };
    let mut u = [0f32; WORDS];
    u[..9].copy_from_slice(&super::m3_f32(&mi));
    u[9..18].copy_from_slice(&super::m3_f32(&mo));
    u[18] = method;
    u[19] = hue.clamp(0.0, 1.0);
    u[20..].copy_from_slice(&curve);
    u
}

/// The curve of `v`'s method at one neutral scene value (tests and the panel).
pub fn eval(v: &ViewTransform, x: f64) -> f64 {
    let c = Curve::new(v);
    let skew = c.s.log2();
    match v.method {
        ViewMethod::Sigmoid => c.eval(x),
        ViewMethod::Filmic => curves::Filmic::new(c.g, skew, c.white, c.black).eval(x),
        ViewMethod::Agx => curves::Agx::new(c.g, skew, c.white, c.black).eval(x),
    }
}

/// The view transform settings of `edit` (defaults when it has none).
pub fn settings(edit: &Edit) -> ViewTransform {
    edit.modules
        .iter()
        .find(|m| m.kind == ModuleKind::VIEW_TRANSFORM && m.enabled)
        .map_or_else(ViewTransform::default, |m| {
            ViewTransform::from_fields(&m.params.to_fields())
        })
}

/// The HDR limit of `edit` as a headroom (units of SDR white): `2^hdr_limit_ev`.
pub fn limit(edit: &Edit) -> f32 {
    let ev = settings(edit).hdr_limit_ev;
    let ev = if ev.is_finite() {
        ev.clamp(0.0, MAX_HDR_LIMIT_EV)
    } else {
        0.0
    };
    ev.exp2()
}

/// The HDR headroom `edit` renders with for a display (or export) peak `peak` (units of SDR
/// white): the peak capped by the edit's HDR limit, never below 1.
pub fn headroom(edit: &Edit, peak: f32) -> f32 {
    let peak = if peak.is_finite() { peak } else { 1.0 };
    peak.min(limit(edit)).max(1.0)
}

/// `edit` rendered for `headroom` (from [`headroom`]): every view transform's white target
/// times the headroom. Headroom 1 gives the edit back unchanged (the SDR rendition).
pub fn for_headroom(edit: &Edit, headroom: f32) -> Cow<'_, Edit> {
    if !headroom.is_finite() || headroom <= 1.0 {
        return Cow::Borrowed(edit);
    }
    let mut e = edit.clone();
    for m in &mut e.modules {
        if m.kind == ModuleKind::VIEW_TRANSFORM {
            let mut v = ViewTransform::from_fields(&m.params.to_fields());
            let w = if v.white_target.is_finite() {
                v.white_target
            } else {
                1.0
            };
            v.white_target = w * headroom;
            m.params.overlay(&v.to_fields());
        }
    }
    Cow::Owned(e)
}

/// One curve value: `p` is the method word; the constants follow the hue word.
const VIEW_WGSL: &str = "fn dv_view_rational(x: f32, s: f32, l: f32, pw: f32) -> f32 {
    let a = s * x;
    if (a <= 0.0) { return 0.0; }
    return a / pow(1.0 + pow(a / l, pw), 1.0 / pw);
}
fn dv_view(x: f32, p: u32) -> f32 {
    let m = pf(p);
    let q = p + 2u;
    if (m < 0.5) {
        var f = 0.0;
        if (x > 0.0) {
            f = pow(1.0 / (1.0 + pow(pf(q + 1u) / x, pf(q))), pf(q + 2u));
        }
        return pf(q + 4u) + (pf(q + 3u) - pf(q + 4u)) * f;
    }
    if (m < 1.5) {
        // Filmic-style: lo, hi, c, u_t, y_t, k0, u_s, y_s, k1, white, black.
        if (x <= 0.0) { return pf(q + 10u); }
        let u = log2(x / 0.1845);
        if (u <= pf(q)) { return pf(q + 10u); }
        if (u >= pf(q + 1u)) { return pf(q + 9u); }
        if (u < pf(q + 3u)) {
            let b = pf(q + 10u);
            return b + (pf(q + 4u) - b) * pow((u - pf(q)) / (pf(q + 3u) - pf(q)), pf(q + 5u));
        }
        if (u > pf(q + 6u)) {
            let w = pf(q + 9u);
            let r = (pf(q + 1u) - u) / (pf(q + 1u) - pf(q + 6u));
            return w - (w - pf(q + 7u)) * pow(r, pf(q + 8u));
        }
        return 0.1845 * exp2(pf(q + 2u) * u);
    }
    // AgX-style: lo, hi, slope, d_g, d_max, L toe, L shoulder, p toe, p shoulder, white,
    // black, power.
    let b = pf(q + 10u);
    if (x <= 0.0) { return b; }
    let u = log2(x / 0.1845);
    var d = 0.0;
    if (u <= pf(q)) {
        d = 0.0;
    } else if (u >= pf(q + 1u)) {
        d = pf(q + 4u);
    } else if (u >= 0.0) {
        d = pf(q + 3u) + dv_view_rational(u, pf(q + 2u), pf(q + 6u), pf(q + 8u));
    } else {
        d = pf(q + 3u) - dv_view_rational(-u, pf(q + 2u), pf(q + 5u), pf(q + 7u));
    }
    d = clamp(d, 0.0, pf(q + 4u));
    var e = 0.0;
    if (d > 0.0) { e = pow(d, pf(q + 11u)); }
    return b + (1.0 - b) * e;
}
";

impl Op for ViewOp {
    fn name(&self) -> &'static str {
        "view_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Point
    }
    fn param_words(&self) -> u32 {
        WORDS as u32
    }
    fn write_params(&self, p: &Params, _: f32, out: &mut [u32]) {
        super::put(out, &uniforms(p));
    }
    fn helpers(&self) -> &'static [&'static str] {
        &[super::MAT3_WGSL, VIEW_WGSL]
    }
    fn wgsl(&self) -> &'static str {
        "    let v = dv_mat3(p, c.rgb);
    let q = p + 18u;
    let pc = vec3<f32>(dv_view(v.x, q), dv_view(v.y, q), dv_view(v.z, q));
    let h = pf(q + 1u);
    var o = pc;
    let mx = max(v.x, max(v.y, v.z));
    let mn = min(v.x, min(v.y, v.z));
    if (h > 0.0 && mx > mn) {
        let fx = dv_view(mx, q);
        let fn_ = dv_view(mn, q);
        let hp = vec3<f32>(fn_) + (fx - fn_) * ((v - vec3<f32>(mn)) / (mx - mn));
        o = mix(pc, hp, h);
    }
    return vec4<f32>(dv_mat3(p + 9u, o), c.a);"
    }
}
