//! Defringe: purple and green fringes beside high-contrast edges lose their
//! chroma, in OkLab, after demosaic.
//!
//! - **Edge mask:** the luminance range over a square of [`RADIUS`] source pixels, in
//!   stops (`log2(max/min)`), through a smooth step from the edge threshold to one stop
//!   above it. Only pixels near a strong edge (a backlit branch, a window frame) qualify.
//! - **Hue windows:** around the purple and green hues (OkLab hue angle, degrees), full
//!   inside half the range, fading to nothing at the range.
//! - **Correction:** chroma times `1 − amount · mask · max(purple, green)`; lightness and
//!   hue stay. Amount 0 is the identity (the node is left out).
//!
//! Lateral CA itself is corrected earlier (raw CA, or the warp); this removes what is left
//! (axial CA, blooming), as darktable's defringe idea does, written from that description.

use crate::graph::{NodeKind, Op, Program, Roi};
use crate::model::{Fields, Params, Value};
use crate::ops::detail::kernels::{LUM, MINMAX, fw};

/// The edge mask's reach in source pixels.
pub const RADIUS: f32 = 4.0;
/// Luminance floor of the edge mask (scene-linear).
const FLOOR: f32 = 1e-3;

/// Defringe params (fields of [`crate::model::ModuleKind::DEFRINGE`], version 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Defringe {
    /// 0 … 1.
    pub amount: f32,
    /// Hue centres, degrees (OkLab).
    pub purple_hue: f32,
    pub green_hue: f32,
    /// Hue window width, degrees (10 … 90).
    pub hue_range: f32,
    /// Edge threshold, stops of luminance range (0.5 … 6).
    pub edge: f32,
}

impl Default for Defringe {
    fn default() -> Self {
        Self {
            amount: 0.0,
            purple_hue: 320.0,
            green_hue: 140.0,
            hue_range: 40.0,
            edge: 2.0,
        }
    }
}

impl Defringe {
    pub fn from_fields(f: &Fields) -> Self {
        let d = Self::default();
        let get = |n: &str, dv: f32, lo: f32, hi: f32| {
            let v = f.f32_or(n, dv);
            if v.is_finite() { v.clamp(lo, hi) } else { dv }
        };
        Self {
            amount: get("amount", d.amount, 0.0, 1.0),
            purple_hue: get("purple_hue", d.purple_hue, 0.0, 360.0),
            green_hue: get("green_hue", d.green_hue, 0.0, 360.0),
            hue_range: get("hue_range", d.hue_range, 10.0, 90.0),
            edge: get("edge", d.edge, 0.5, 6.0),
        }
    }

    pub fn default_fields() -> Fields {
        let d = Self::default();
        Fields::new()
            .with("amount", Value::Float(d.amount))
            .with("purple_hue", Value::Float(d.purple_hue))
            .with("green_hue", Value::Float(d.green_hue))
            .with("hue_range", Value::Float(d.hue_range))
            .with("edge", Value::Float(d.edge))
    }

    fn of(p: &Params) -> Self {
        Self::from_fields(&p.to_fields())
    }
}

/// The mask radius in view pixels at `scale`.
pub fn radius(scale: f32) -> u32 {
    (RADIUS * scale.clamp(1e-3, 1.0)).round().max(1.0) as u32
}

/// Hue window weight: 1 within half the range of the centre, 0 beyond the range.
pub fn hue_weight(hue_deg: f32, centre: f32, range: f32) -> f32 {
    let mut d = (hue_deg - centre).abs() % 360.0;
    if d > 180.0 {
        d = 360.0 - d;
    }
    1.0 - smoothstep(0.5 * range, range, d)
}

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Planes: luminance (0), row minimum (n) and maximum (2n), square minimum (3n) and
/// maximum (4n).
fn program(input: Roi, scale: f32) -> Program {
    let (w, h) = (input.w, input.h);
    let n = w * h;
    let r = radius(scale);
    let mut prog = Program::default();
    prog.step(&LUM, [w, h], vec![0, 0, w, input.x, input.y, fw(FLOOR)]);
    prog.step(&MINMAX, [w, h], vec![0, n, 2 * n, w, h, 0, r, 0]);
    prog.step(&MINMAX, [w, h], vec![n, 3 * n, 4 * n, w, h, 1, r, 2 * n]);
    prog.words = 5 * u64::from(n);
    prog
}

/// Defringe, param version 1.
pub struct DefringeOp;

impl Op for DefringeOp {
    fn name(&self) -> &'static str {
        "defringe_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Neighbourhood
    }
    fn halo(&self, _p: &Params, scale: f32) -> u32 {
        radius(scale) + 1
    }
    fn param_words(&self) -> u32 {
        6
    }
    fn write_params(&self, p: &Params, _scale: f32, out: &mut [u32]) {
        let d = Defringe::of(p);
        out[0] = fw(d.amount);
        out[1] = fw(d.purple_hue);
        out[2] = fw(d.green_hue);
        out[3] = fw(d.hue_range);
        out[4] = fw(d.edge);
        out[5] = 0;
    }
    fn helpers(&self) -> &'static [&'static str] {
        static H: std::sync::LazyLock<[&'static str; 2]> =
            std::sync::LazyLock::new(|| [*crate::colour::oklab::OKLAB_WGSL, DEFRINGE_WGSL]);
        &*H
    }
    fn identity(&self, p: &Params) -> bool {
        Defringe::of(p).amount == 0.0
    }
    fn local(&self, _p: &Params, input: Roi, scale: f32) -> Option<Program> {
        Some(program(input, scale))
    }
    fn wgsl(&self) -> &'static str {
        "    let c = src_at(x, y);
    let rw = pu(18u);
    let rh = pu(19u);
    let n = rw * rh;
    let lx = u32(clamp(x - i32(pu(16u)), 0, i32(rw) - 1));
    let ly = u32(clamp(y - i32(pu(17u)), 0, i32(rh) - 1));
    let i = ly * rw + lx;
    let lo = lc(3u * n + i);
    let hi = lc(4u * n + i);
    return dv_defringe(c, lo, hi, p);"
    }
}

const DEFRINGE_WGSL: &str = "fn dv_df_hue(h: f32, centre: f32, range: f32) -> f32 {
    var d = abs(h - centre) % 360.0;
    if (d > 180.0) { d = 360.0 - d; }
    return 1.0 - smoothstep(0.5 * range, range, d);
}
fn dv_defringe(c: vec4<f32>, lo: f32, hi: f32, p: u32) -> vec4<f32> {
    let edge = log2(max(hi, 1e-3) / max(lo, 1e-3));
    let m = smoothstep(pf(p + 4u), pf(p + 4u) + 1.0, edge);
    if (m <= 0.0) { return c; }
    let lab = dv_oklab(c.rgb);
    let ch = length(lab.yz);
    if (ch <= 1e-7) { return c; }
    var h = degrees(atan2(lab.z, lab.y));
    if (h < 0.0) { h += 360.0; }
    let wh = max(dv_df_hue(h, pf(p + 1u), pf(p + 3u)), dv_df_hue(h, pf(p + 2u), pf(p + 3u)));
    let k = clamp(1.0 - pf(p) * m * wh, 0.0, 1.0);
    if (k >= 1.0) { return c; }
    return vec4<f32>(dv_oklab_inv(vec3<f32>(lab.x, lab.yz * k)), c.a);
}
";

/// The CPU twin of the kernel at one pixel (tests and tools): `lo`, `hi` are the
/// luminance range around it.
pub fn cpu_pixel(d: &Defringe, c: [f32; 3], lo: f32, hi: f32) -> [f32; 3] {
    let edge = (hi.max(FLOOR) / lo.max(FLOOR)).log2();
    let m = smoothstep(d.edge, d.edge + 1.0, edge);
    if m <= 0.0 {
        return c;
    }
    let lab = crate::colour::oklab::from_rec2020_f32(c);
    let ch = (lab[1] * lab[1] + lab[2] * lab[2]).sqrt();
    if ch <= 1e-7 {
        return c;
    }
    let mut h = lab[2].atan2(lab[1]).to_degrees();
    if h < 0.0 {
        h += 360.0;
    }
    let wh = hue_weight(h, d.purple_hue, d.hue_range).max(hue_weight(h, d.green_hue, d.hue_range));
    let k = (1.0 - d.amount * m * wh).clamp(0.0, 1.0);
    if k >= 1.0 {
        return c;
    }
    crate::colour::oklab::to_rec2020_f32([lab[0], lab[1] * k, lab[2] * k])
}
