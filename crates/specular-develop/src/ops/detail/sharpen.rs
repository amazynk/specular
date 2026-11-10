//! Sharpen: unsharp masking of the linear
//! luminance with a details mask and an overshoot limit, among the scene-linear ops.
//!
//! - **Unsharp mask:** `Y' = Y + amount·mask·(Y − G_σ ⊛ Y)`, the colour channels scaled by
//!   `Y'/Y` (hue and saturation stay).
//! - **Details mask** (Masking): the relative gradient of the blurred luminance,
//!   `|∇(G ⊛ Y)| / (G ⊛ Y)`, through a smooth step whose thresholds rise with Masking, so
//!   flat areas (sky, skin) are left alone and edges and texture are sharpened. Masking 0
//!   sharpens everywhere.
//! - **No halos:** `Y'` is held within the local range of `Y` (minimum and maximum over
//!   the blur's reach), widened by `0.2·detail` of that range. At Detail 0 a step edge
//!   gets no overshoot at all; Detail adds some back for crisper fine texture.
//!
//! **Scale.** Radius is in source pixels. At view scale `s` the blur is `σ·s`; below half a
//! view pixel it stays at half a pixel and the amount falls with the area the source
//! detail covers, `(σ·s / ½)²`, so a reduced preview is not over-sharpened.
//!
//! Amount 0 is the identity (the node is left out).

use super::capture::DETAIL_LUMA_WGSL;
use super::kernels::{BLUR, LUM, MINMAX, fw, gauss_taps};
use crate::graph::{NodeKind, Op, Program, Roi};
use crate::model::{Fields, Params, Value};

/// The smallest blur, in view pixels.
const MIN_VIEW_SIGMA: f32 = 0.5;
/// Overshoot allowed at Detail 1, as a share of the local range.
const OVERSHOOT: f32 = 0.2;
/// Details-mask thresholds at Masking 1 (relative gradient).
const MASK_LO: f32 = 0.06;
const MASK_HI: f32 = 0.2;

/// Sharpen params (fields of [`crate::model::ModuleKind::SHARPEN`], version 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Sharpen {
    /// 0 … 2.
    pub amount: f32,
    /// Blur σ in source pixels, 0.3 … 3.
    pub radius: f32,
    /// Overshoot allowed, 0 … 1.
    pub detail: f32,
    /// Details mask, 0 (everywhere) … 1 (edges only).
    pub masking: f32,
}

impl Default for Sharpen {
    fn default() -> Self {
        Self {
            amount: 0.0,
            radius: 1.0,
            detail: 0.25,
            masking: 0.0,
        }
    }
}

impl Sharpen {
    pub fn from_fields(f: &Fields) -> Self {
        let d = Self::default();
        let get = |n: &str, dv: f32, lo: f32, hi: f32| {
            let v = f.f32_or(n, dv);
            if v.is_finite() { v.clamp(lo, hi) } else { dv }
        };
        Self {
            amount: get("amount", d.amount, 0.0, 2.0),
            radius: get("radius", d.radius, 0.3, 3.0),
            detail: get("detail", d.detail, 0.0, 1.0),
            masking: get("masking", d.masking, 0.0, 1.0),
        }
    }

    pub fn default_fields() -> Fields {
        let d = Self::default();
        Fields::new()
            .with("amount", Value::Float(d.amount))
            .with("radius", Value::Float(d.radius))
            .with("detail", Value::Float(d.detail))
            .with("masking", Value::Float(d.masking))
    }

    fn of(p: &Params) -> Self {
        Self::from_fields(&p.to_fields())
    }
}

/// View σ, blur tap radius, range radius and amount at `scale`.
pub fn plan(s: &Sharpen, scale: f32) -> (f32, u32, u32, f32) {
    let want = s.radius * scale.clamp(1e-3, 1.0);
    let (sigma, amount) = if want >= MIN_VIEW_SIGMA {
        (want, s.amount)
    } else {
        (MIN_VIEW_SIGMA, s.amount * (want / MIN_VIEW_SIGMA).powi(2))
    };
    let rg = (3.0 * sigma).ceil() as u32;
    let rm = (1.5 * sigma).ceil().max(1.0) as u32;
    (sigma, rg, rm, amount)
}

pub fn halo(s: &Sharpen, scale: f32) -> u32 {
    let (_, rg, rm, _) = plan(s, scale);
    (rg + 1).max(rm) + 1
}

/// Planes: luminance `Y` (0), temporary (n), blurred (2n), row minimum (3n) and maximum
/// (4n); the kernel takes the column range itself.
pub fn program(s: &Sharpen, input: Roi, scale: f32) -> Program {
    let (w, h) = (input.w, input.h);
    let n = w * h;
    let (sigma, rg, rm, _) = plan(s, scale);
    let taps = gauss_taps(sigma, rg);
    let blur = |src: u32, dst: u32, vert: bool| {
        let mut v = vec![src, dst, w, h, u32::from(vert), rg, 0, 0, 0, 0, 0, 0, 0];
        v.extend(taps.iter().map(|t| fw(*t)));
        v
    };
    let mut prog = Program::default();
    prog.step(&LUM, [w, h], vec![0, 0, w, input.x, input.y, fw(f32::MIN)]);
    prog.step(&BLUR, [w, h], blur(0, n, false));
    prog.step(&BLUR, [w, h], blur(n, 2 * n, true));
    prog.step(&MINMAX, [w, h], vec![0, 3 * n, 4 * n, w, h, 0, rm, 0]);
    prog.words = 5 * u64::from(n);
    prog
}

/// Sharpen, param version 1.
pub struct SharpenOp;

impl Op for SharpenOp {
    fn name(&self) -> &'static str {
        "sharpen_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Neighbourhood
    }
    fn halo(&self, p: &Params, scale: f32) -> u32 {
        halo(&Sharpen::of(p), scale)
    }
    fn param_words(&self) -> u32 {
        5
    }
    fn write_params(&self, p: &Params, scale: f32, out: &mut [u32]) {
        let s = Sharpen::of(p);
        let (_, _, rm, amount) = plan(&s, scale);
        out[0] = fw(amount);
        out[1] = fw(MASK_LO * s.masking);
        out[2] = fw(MASK_HI * s.masking);
        out[3] = fw(OVERSHOOT * s.detail);
        out[4] = rm;
    }
    fn helpers(&self) -> &'static [&'static str] {
        &[DETAIL_LUMA_WGSL, SHARPEN_WGSL]
    }
    fn identity(&self, p: &Params) -> bool {
        Sharpen::of(p).amount == 0.0
    }
    fn local(&self, p: &Params, input: Roi, scale: f32) -> Option<Program> {
        Some(program(&Sharpen::of(p), input, scale))
    }
    fn wgsl(&self) -> &'static str {
        "    let c = src_at(x, y);
    let rw = pu(18u);
    let rh = pu(19u);
    let n = rw * rh;
    let lx = x - i32(pu(16u));
    let ly = y - i32(pu(17u));
    let yv = lc(u32(ly) * rw + u32(lx));
    let b = dv_sh_at(2u * n, rw, rh, lx, ly);
    let gx = dv_sh_at(2u * n, rw, rh, lx + 1, ly) - dv_sh_at(2u * n, rw, rh, lx - 1, ly);
    let gy = dv_sh_at(2u * n, rw, rh, lx, ly + 1) - dv_sh_at(2u * n, rw, rh, lx, ly - 1);
    let e = (abs(gx) + abs(gy)) / (2.0 * (abs(b) + 1e-3));
    var mask = 1.0;
    if (pf(p + 2u) > 0.0) { mask = smoothstep(pf(p + 1u), pf(p + 2u), e); }
    let rm = i32(pu(p + 4u));
    var lo = 3.0e38;
    var hi = -3.0e38;
    for (var k = -rm; k <= rm; k++) {
        lo = min(lo, dv_sh_at(3u * n, rw, rh, lx, ly + k));
        hi = max(hi, dv_sh_at(4u * n, rw, rh, lx, ly + k));
    }
    let o = pf(p + 3u) * (hi - lo);
    let ny = clamp(yv + pf(p) * mask * (yv - b), lo - o, hi + o);
    let yl = dot(c.rgb, DV_DY);
    if (yl <= 1e-9) { return c; }
    return vec4<f32>(c.rgb * clamp(ny / yl, 0.0, 8.0), c.a);"
    }
}

const SHARPEN_WGSL: &str = "fn dv_sh_at(o: u32, w: u32, h: u32, x: i32, y: i32) -> f32 {
    let cx = u32(clamp(x, 0, i32(w) - 1));
    let cy = u32(clamp(y, 0, i32(h) - 1));
    return lc(o + cy * w + cx);
}
";
