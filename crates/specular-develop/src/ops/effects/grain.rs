//! Film grain: shaped noise in log exposure, a cheap stand-in for the Boolean grain
//! model of Newson, Delon and Galerne (2017), whose filtered output is close to a smooth,
//! band-limited random field.
//!
//! - **Field.** Two layers of value noise (iid lattice values from an integer hash,
//!   interpolated with a smooth step), the second on a lattice turned by 36.87°, so no grid
//!   shows; scaled to unit variance on average. The lattice cell is `grain_cell(size)` of
//!   the image's long side, so grain has the same size relative to the picture at every
//!   view and export size.
//! - **Scale.** A view pixel shows the mean of the field over its footprint, as a
//!   downscaled export does: point samples while a cell spans 2 px or more, a 2×2 or 4×4
//!   grid of samples down to half a pixel, and below that an independent value per pixel
//!   with the variance of the box mean, `(1/f − 2M/f²) / m` for footprint `f` cells (exact
//!   for f ≥ 2; [`box_sigma`]). So a fit view shows the same grain a downscaled export has.
//! - **Response.** The pixel's exposure moves by `amount · 0.5 · n` stops, weighted by a
//!   Gaussian in log luminance around `2.5 · bias` EV from mid grey (σ 2.2 EV): grain is
//!   strongest in the midtones (or the shadows, or the highlights) and fades towards black
//!   and white, as on film.
//! - **Colour.** Mono: one field for all channels. Colour: per channel `0.6·n₀ + 0.8·n_c`
//!   (correlated as dye clouds are).
//! - **Seed.** The `seed` field (set from the photo when the module is made), never time:
//!   the same edit gives the same grain in preview and export, every time.

use super::{get, luma};
use crate::graph::{NodeKind, Op};
use crate::model::{Fields, Params, Value};

/// Grain params (fields of [`crate::model::ModuleKind::GRAIN`], version 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grain {
    /// 0 … 1.
    pub amount: f32,
    /// 0 … 1 (see [`grain_cell`]).
    pub size: f32,
    /// −1 (shadows) … 1 (highlights).
    pub bias: f32,
    pub colour: bool,
    pub seed: u32,
}

impl Default for Grain {
    fn default() -> Self {
        Self {
            amount: 0.0,
            size: 0.25,
            bias: 0.0,
            colour: false,
            seed: 0,
        }
    }
}

/// The lattice cell as a fraction of the long side: 0.0002 (size 0) to 0.0032 (size 1),
/// about 1.2 to 19 px on a 6000 px photo.
pub fn grain_cell(size: f32) -> f32 {
    0.0002 + 0.003 * size * size
}

/// Mean variance of one interpolated layer of uniform [−1, 1] values: (1/3)·m².
const M_SMOOTH: f32 = 26.0 / 35.0;
/// `∫₀² τ R(τ) dτ` of the smooth-step hat's autocorrelation R.
const M_MOMENT: f32 = 0.208_730_16;
/// Unit variance for the sum of two layers: 1 / √(2 · m² / 3).
const NORM: f32 = 1.648_695;

/// The standard deviation of the unit field's mean over a square of `f` cells a side
/// (f ≥ 2): `(1/f − 2M/f²) / m`.
pub fn box_sigma(f: f32) -> f32 {
    (1.0 / f - 2.0 * M_MOMENT / (f * f)) / M_SMOOTH
}

impl Grain {
    pub fn from_fields(f: &Fields) -> Self {
        let d = Self::default();
        Self {
            amount: get(f, "amount", d.amount, 0.0, 1.0),
            size: get(f, "size", d.size, 0.0, 1.0),
            bias: get(f, "bias", d.bias, -1.0, 1.0),
            colour: f.bool_or("colour", d.colour),
            seed: f.i64_or("seed", 0) as u32,
        }
    }

    pub fn default_fields() -> Fields {
        let d = Self::default();
        Fields::new()
            .with("amount", Value::Float(d.amount))
            .with("size", Value::Float(d.size))
            .with("bias", Value::Float(d.bias))
            .with("colour", Value::Bool(d.colour))
            .with("seed", Value::Int(0))
    }

    /// The kernel's uniforms: stops per unit noise, cell (fraction of the long side), bias
    /// (EV from mid grey), colour (0/1), seed (bits).
    pub fn uniforms(&self) -> [u32; 5] {
        [
            (self.amount * 0.5).to_bits(),
            grain_cell(self.size).to_bits(),
            (self.bias * 2.5).to_bits(),
            u32::from(self.colour),
            self.seed,
        ]
    }
}

/// A seed for a photo: FNV-1a of what names it (its file name), never zero.
pub fn seed_for(name: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in name.bytes() {
        h = (h ^ u32::from(b)).wrapping_mul(0x0100_0193);
    }
    h.max(1)
}

/// Film grain, param version 1.
pub struct GrainOp;

impl Op for GrainOp {
    fn name(&self) -> &'static str {
        "grain_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Point
    }
    fn param_words(&self) -> u32 {
        5
    }
    fn write_params(&self, p: &Params, _: f32, out: &mut [u32]) {
        out[..5].copy_from_slice(&Grain::from_fields(&p.to_fields()).uniforms());
    }
    fn helpers(&self) -> &'static [&'static str] {
        &[super::FX_WGSL, GRAIN_WGSL]
    }
    fn identity(&self, p: &Params) -> bool {
        Grain::from_fields(&p.to_fields()).amount == 0.0
    }
    fn wgsl(&self) -> &'static str {
        "    return dv_grain(c, px, py, p);"
    }
}

const GRAIN_WGSL: &str = "fn dv_gr_hash(a: u32) -> u32 {
    let s = a * 747796405u + 2891336453u;
    let w = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (w >> 22u) ^ w;
}
fn dv_gr_u(a: u32) -> f32 {
    return f32(a >> 8u) * (2.0 / 16777215.0) - 1.0;
}
fn dv_gr_val(ix: i32, iy: i32, k: u32) -> f32 {
    return dv_gr_u(dv_gr_hash(bitcast<u32>(ix) ^ dv_gr_hash(bitcast<u32>(iy) ^ k)));
}
fn dv_gr_layer(x: f32, y: f32, k: u32) -> f32 {
    let fx = floor(x);
    let fy = floor(y);
    let tx = x - fx;
    let ty = y - fy;
    let sx = tx * tx * (3.0 - 2.0 * tx);
    let sy = ty * ty * (3.0 - 2.0 * ty);
    let ix = i32(fx);
    let iy = i32(fy);
    let a = mix(dv_gr_val(ix, iy, k), dv_gr_val(ix + 1, iy, k), sx);
    let b = mix(dv_gr_val(ix, iy + 1, k), dv_gr_val(ix + 1, iy + 1, k), sx);
    return mix(a, b, sy);
}
fn dv_gr_field(x: f32, y: f32, k: u32) -> f32 {
    let a = dv_gr_layer(x, y, k);
    let b = dv_gr_layer(0.8 * x - 0.6 * y + 17.31, 0.6 * x + 0.8 * y + 41.73, k ^ 1540483477u);
    return (a + b) * 1.648695;
}
fn dv_gr_noise(px: i32, py: i32, f: f32, k: u32) -> f32 {
    if (f > 2.0) {
        let h = dv_gr_hash(bitcast<u32>(px) ^ dv_gr_hash(bitcast<u32>(py) ^ k ^ 2654435769u));
        let h2 = dv_gr_hash(h);
        let h3 = dv_gr_hash(h2);
        let s = (1.0 / f - 0.41746032 / (f * f)) / 0.74285716;
        return (dv_gr_u(h) + dv_gr_u(h2) + dv_gr_u(h3)) * s;
    }
    var n = 1u;
    if (f > 1.0) { n = 4u; } else if (f > 0.5) { n = 2u; }
    var acc = 0.0;
    for (var j = 0u; j < n; j++) {
        for (var i = 0u; i < n; i++) {
            let x = (f32(px) + (f32(i) + 0.5) / f32(n)) * f;
            let y = (f32(py) + (f32(j) + 0.5) / f32(n)) * f;
            acc += dv_gr_field(x, y, k);
        }
    }
    return acc / f32(n * n);
}
fn dv_grain(c: vec4<f32>, px: i32, py: i32, p: u32) -> vec4<f32> {
    let l = max(f32(pu(4u)), f32(pu(5u)));
    let f = 1.0 / (pf(p + 1u) * l);
    let seed = pu(p + 4u);
    let e = log2(max(dot(c.rgb, DV_FX_LUMA), 1.52587890625e-5)) + 2.4739312 - pf(p + 2u);
    let a = pf(p) * exp(-e * e / 9.68);
    let n0 = dv_gr_noise(px, py, f, seed);
    var n = vec3<f32>(n0);
    if (pu(p + 3u) != 0u) {
        n = 0.6 * n0 + 0.8 * vec3<f32>(
            dv_gr_noise(px, py, f, seed ^ 2654435769u),
            dv_gr_noise(px, py, f, seed ^ 1013904242u),
            dv_gr_noise(px, py, f, seed ^ 3668340011u));
    }
    return vec4<f32>(c.rgb * exp2(a * n), c.a);
}
";

fn hash(a: u32) -> u32 {
    let s = a.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    let w = ((s >> ((s >> 28) + 4)) ^ s).wrapping_mul(277_803_737);
    (w >> 22) ^ w
}

fn unit(a: u32) -> f32 {
    (a >> 8) as f32 * (2.0 / 16_777_215.0) - 1.0
}

fn val(ix: i32, iy: i32, k: u32) -> f32 {
    unit(hash(ix as u32 ^ hash(iy as u32 ^ k)))
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn layer(x: f32, y: f32, k: u32) -> f32 {
    let (fx, fy) = (x.floor(), y.floor());
    let (tx, ty) = (x - fx, y - fy);
    let sx = tx * tx * (3.0 - 2.0 * tx);
    let sy = ty * ty * (3.0 - 2.0 * ty);
    let (ix, iy) = (fx as i32, fy as i32);
    let a = mix(val(ix, iy, k), val(ix + 1, iy, k), sx);
    let b = mix(val(ix, iy + 1, k), val(ix + 1, iy + 1, k), sx);
    mix(a, b, sy)
}

/// The unit-variance field at lattice point (x, y) for key `k`.
pub fn field(x: f32, y: f32, k: u32) -> f32 {
    let a = layer(x, y, k);
    let b = layer(
        0.8 * x - 0.6 * y + 17.31,
        0.6 * x + 0.8 * y + 41.73,
        k ^ 1_540_483_477,
    );
    (a + b) * NORM
}

/// The noise a pixel shows for footprint `f` cells.
pub fn noise(px: i32, py: i32, f: f32, k: u32) -> f32 {
    if f > 2.0 {
        let h = hash(px as u32 ^ hash(py as u32 ^ k ^ 2_654_435_769));
        let h2 = hash(h);
        let h3 = hash(h2);
        return (unit(h) + unit(h2) + unit(h3)) * box_sigma(f);
    }
    let n = if f > 1.0 {
        4
    } else if f > 0.5 {
        2
    } else {
        1
    };
    let mut acc = 0.0;
    for j in 0..n {
        for i in 0..n {
            let x = (px as f32 + (i as f32 + 0.5) / n as f32) * f;
            let y = (py as f32 + (j as f32 + 0.5) / n as f32) * f;
            acc += field(x, y, k);
        }
    }
    acc / (n * n) as f32
}

/// The CPU twin of the kernel at pixel (`px`, `py`) of a `w`×`h` image.
pub fn cpu_pixel(u: &[u32; 5], c: [f32; 4], px: i32, py: i32, w: u32, h: u32) -> [f32; 4] {
    let l = (w as f32).max(h as f32);
    let fu = |i: usize| f32::from_bits(u[i]);
    let f = 1.0 / (fu(1) * l);
    let seed = u[4];
    let e = luma([c[0], c[1], c[2]]).max(1.525_878_9e-5).log2() + 2.473_931_2 - fu(2);
    let a = fu(0) * (-e * e / 9.68).exp();
    let n0 = noise(px, py, f, seed);
    let n = if u[3] != 0 {
        [
            0.6 * n0 + 0.8 * noise(px, py, f, seed ^ 2_654_435_769),
            0.6 * n0 + 0.8 * noise(px, py, f, seed ^ 1_013_904_242),
            0.6 * n0 + 0.8 * noise(px, py, f, seed ^ 3_668_340_011),
        ]
    } else {
        [n0; 3]
    };
    [
        c[0] * (a * n[0]).exp2(),
        c[1] * (a * n[1]).exp2(),
        c[2] * (a * n[2]).exp2(),
        c[3],
    ]
}
