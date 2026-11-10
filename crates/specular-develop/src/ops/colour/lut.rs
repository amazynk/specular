//! 3D LUT: a LUT file applied in a declared space, read by
//! `specular-ocio` (.cube, .3dl, .spi1d, .spi3d, CLF/CTF) and run on the GPU.
//!
//! - **Space.** A LUT is made for some encoding of its input. The op converts linear
//!   Rec.2020 into the declared space (primaries by matrix, Bradford-adapted for ACES's
//!   D60 white as OpenColorIO's ACES configs do; then the encoding curve), runs the file,
//!   and converts back: `linear_rec2020`, `acescg` (linear AP1), `acescct` (AP1 with the
//!   ACEScct log curve, for scene-referred looks), `srgb` (Rec.709 primaries, sRGB curve)
//!   and `rec709` (Rec.709 primaries, BT.1886 2.4 power). Values outside the table's domain
//!   are clamped to it, as OpenColorIO does.
//! - **On the GPU** the file's operations are written as a table the engine uploads once
//!   per content ([`crate::graph::Op::data`]): matrices, ranges, 1D tables (linear
//!   interpolation), 3D tables with the file's tetrahedral or trilinear interpolation,
//!   exponents (and the sRGB-style moncurve), logs (with camera-log linear segments) and
//!   ASC CDL, the same arithmetic as `specular-ocio`'s CPU processor, so the two agree to
//!   f32 rounding. A storage buffer rather than a sampled 3D texture: hardware filtering
//!   interpolates with 8-bit weights (far from 1e-4) and cannot do tetrahedral.
//! - Files with other operations (half-domain or inverted 1D tables, the ACES fixed
//!   functions) are baked: the whole file sampled on a 65³ grid over 0–1 of the declared
//!   space, looked up tetrahedrally ([`Loaded::exact`] is false).
//! - **Strength** mixes the result with the input in linear Rec.2020.
//!
//! Files load once per path, size and modification time (a small cache shared by every
//! graph build); a missing or unreadable file leaves the module out of the graph and
//! [`status`] says why.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use specular_ocio::ops::{Interp, Negative, Op as OcioOp};

use crate::colour::{self, M3};
use crate::graph::{NodeKind, Op, OpData};
use crate::model::fields::choice;
use crate::model::{Fields, Params, Value};

choice! {
    /// The space a LUT expects its input in.
    LutSpace {
        LinearRec2020 = "linear_rec2020",
        Acescg = "acescg",
        Acescct = "acescct",
        Srgb = "srgb",
        Rec709 = "rec709",
    }
}

impl LutSpace {
    /// The kernel's space id.
    fn id(self) -> u32 {
        match self {
            LutSpace::LinearRec2020 => 0,
            LutSpace::Acescg => 1,
            LutSpace::Acescct => 2,
            LutSpace::Srgb => 3,
            LutSpace::Rec709 => 4,
        }
    }

    /// Linear Rec.2020 → the space's linear primaries.
    pub fn to_primaries(self) -> M3 {
        use specular_ocio::builtin as b;
        match self {
            LutSpace::LinearRec2020 => colour::IDENTITY,
            LutSpace::Acescg | LutSpace::Acescct => {
                let m = b::gamut_to_gamut(&b::REC2020, &b::AP1);
                std::array::from_fn(|i| std::array::from_fn(|j| m[i * 3 + j]))
            }
            LutSpace::Srgb | LutSpace::Rec709 => colour::rec2020_to_rec709(),
        }
    }
}

/// LUT params (fields of [`crate::model::ModuleKind::LUT_3D`], v1).
#[derive(Clone, Debug, PartialEq)]
pub struct Lut3d {
    pub path: String,
    pub space: LutSpace,
    pub tetrahedral: bool,
    pub strength: f32,
}

impl Lut3d {
    pub fn from_fields(f: &Fields) -> Self {
        Self {
            path: f.text("path").unwrap_or("").to_owned(),
            space: f.choice_or("space", LutSpace::default()),
            tetrahedral: f.text("interpolation") != Some("trilinear"),
            strength: super::num(f, "strength", 1.0, 0.0, 1.0),
        }
    }

    pub fn defaults() -> Fields {
        Fields::new()
            .with("path", Value::Text(String::new()))
            .with("space", LutSpace::default().into())
            .with("interpolation", Value::Text("tetrahedral".into()))
            .with("strength", Value::Float(1.0))
    }

    /// Space id, strength, the two matrices.
    pub fn uniforms(&self) -> [f32; WORDS] {
        let to = self.space.to_primaries();
        let from = colour::inv(&to).unwrap_or(colour::IDENTITY);
        let mut u = [0f32; WORDS];
        u[0] = f32::from_bits(self.space.id());
        u[1] = self.strength;
        u[2..11].copy_from_slice(&super::super::m3_f32(&to));
        u[11..20].copy_from_slice(&super::super::m3_f32(&from));
        u
    }
}

pub const WORDS: usize = 20;

/// A LUT file as the GPU runs it.
#[derive(Debug)]
pub struct Loaded {
    pub data: Arc<OpData>,
    /// The file runs as written (false: baked on a 65³ grid).
    pub exact: bool,
    /// Points per axis of its 3D table (0 when it has none).
    pub size: usize,
}

/// Grid of the bake for files with operations the GPU table does not run.
pub const BAKE: usize = 65;

/// Record kinds of the GPU table.
const MATRIX: f32 = 1.0;
const RANGE: f32 = 2.0;
const LUT1D: f32 = 3.0;
const LUT3D: f32 = 4.0;
const EXPONENT: f32 = 5.0;
const EXPLIN: f32 = 6.0;
const LOG: f32 = 7.0;
const CDL: f32 = 8.0;

/// The GPU table of `ops`: the count, then one record per op (`kind, length, …`), or
/// `None` when an op is not one the table runs.
pub fn program(ops: &[OcioOp]) -> Option<Vec<f32>> {
    let mut w = vec![ops.len() as f32];
    for op in ops {
        let at = w.len();
        match op {
            OcioOp::Matrix { m, offset } => {
                w.extend([MATRIX, 0.0]);
                w.extend(m.iter().chain(offset).map(|v| *v as f32));
            }
            OcioOp::Range {
                scale,
                offset,
                lo,
                hi,
            } => {
                w.extend([RANGE, 0.0, *scale as f32, *offset as f32]);
                w.push(lo.map_or(f32::MIN, |v| v as f32));
                w.push(hi.map_or(f32::MAX, |v| v as f32));
            }
            OcioOp::Lut1d {
                values,
                lo,
                hi,
                half_domain: false,
                inverse: false,
                ..
            } if values.len() >= 2 => {
                w.extend([LUT1D, 0.0, values.len() as f32]);
                w.extend(lo.iter().chain(hi).map(|v| *v as f32));
                w.extend(values.iter().flatten().map(|v| *v as f32));
            }
            OcioOp::Lut3d {
                size,
                values,
                lo,
                hi,
                interp,
            } if *size >= 2 && values.len() >= size * size * size => {
                let tet = f32::from(u8::from(*interp == Interp::Tetrahedral));
                w.extend([LUT3D, 0.0, *size as f32, tet]);
                w.extend(lo.iter().chain(hi).map(|v| *v as f32));
                w.extend(
                    values[..size * size * size]
                        .iter()
                        .flatten()
                        .map(|v| *v as f32),
                );
            }
            OcioOp::Exponent { e, neg, inverse } => {
                w.extend([EXPONENT, 0.0]);
                w.extend(e.map(|v| if *inverse { 1.0 / v } else { v } as f32));
                w.push(match neg {
                    Negative::Clamp => 0.0,
                    Negative::Mirror => 1.0,
                    Negative::PassThru | Negative::Linear => 2.0,
                });
            }
            OcioOp::ExpLinear { g, o, neg, inverse } => {
                w.extend([EXPLIN, 0.0, f32::from(u8::from(*inverse))]);
                w.push(f32::from(u8::from(*neg == Negative::Mirror)));
                for k in 0..3 {
                    // The break and slope where the line meets the curve (as OCIO's).
                    let brk = o[k] / (g[k] - 1.0);
                    let slope =
                        ((1.0 + o[k]) / g[k]).powf(g[k]) * ((g[k] - 1.0) / o[k]).powf(g[k] - 1.0);
                    w.extend([g[k], o[k], slope, brk].map(|v| v as f32));
                }
            }
            OcioOp::Log {
                base,
                log_slope,
                log_offset,
                lin_slope,
                lin_offset,
                lin_break,
                linear_slope,
                inverse,
            } => {
                w.extend([LOG, 0.0, f32::from(u8::from(*inverse)), base.ln() as f32]);
                let lb = base.ln();
                for k in 0..3 {
                    let (has, brk, s, off) = match lin_break {
                        Some(b) => {
                            let brk = b[k];
                            let s = linear_slope.map(|s| s[k]).unwrap_or_else(|| {
                                log_slope[k] * lin_slope[k]
                                    / ((lin_slope[k] * brk + lin_offset[k]) * lb)
                            });
                            let y = log_slope[k] * (lin_slope[k] * brk + lin_offset[k]).ln() / lb
                                + log_offset[k];
                            (1.0, brk, s, y - s * brk)
                        }
                        None => (0.0, 0.0, 1.0, 0.0),
                    };
                    w.extend(
                        [
                            log_slope[k],
                            log_offset[k],
                            lin_slope[k],
                            lin_offset[k],
                            has,
                            brk,
                            s,
                            off,
                        ]
                        .map(|v| v as f32),
                    );
                }
            }
            OcioOp::Cdl {
                slope,
                offset,
                power,
                sat,
                clamp,
                inverse,
            } => {
                w.extend([CDL, 0.0, f32::from(u8::from(*clamp))]);
                w.extend([f32::from(u8::from(*inverse)), *sat as f32]);
                w.extend(slope.iter().chain(offset).chain(power).map(|v| *v as f32));
            }
            _ => return None,
        }
        w[at + 1] = (w.len() - at) as f32;
    }
    Some(w)
}

/// `ops` sampled on a [`BAKE`]³ grid over 0–1, as one tetrahedral 3D table.
pub fn bake(ops: &[OcioOp]) -> Vec<f32> {
    let p = specular_ocio::Processor { ops: ops.to_vec() };
    let n = BAKE;
    let mut values = Vec::with_capacity(n * n * n);
    let t = |i: usize| i as f64 / (n - 1) as f64;
    for r in 0..n {
        for g in 0..n {
            for b in 0..n {
                values.push(p.apply([t(r), t(g), t(b)]));
            }
        }
    }
    program(&[OcioOp::Lut3d {
        size: n,
        values,
        lo: [0.0; 3],
        hi: [1.0; 3],
        interp: Interp::Tetrahedral,
    }])
    .expect("a 3D table is a program")
}

/// Reads and prepares a LUT file (no cache).
pub fn read(path: &Path, tetrahedral: bool) -> Result<Loaded, String> {
    let interp = if tetrahedral { "tetrahedral" } else { "linear" };
    let ops = specular_ocio::files::read(path, interp).map_err(|e| e.to_string())?;
    let size = ops
        .iter()
        .find_map(|o| match o {
            OcioOp::Lut3d { size, .. } => Some(*size),
            _ => None,
        })
        .unwrap_or(0);
    let (words, exact) = match program(&ops) {
        Some(w) => (w, true),
        None => (bake(&ops), false),
    };
    Ok(Loaded {
        data: Arc::new(OpData::new(words)),
        exact,
        size,
    })
}

type CacheKey = (PathBuf, bool, u64, Option<SystemTime>);
type Cache = Mutex<Vec<(CacheKey, Result<Arc<Loaded>, String>)>>;

/// Files kept loaded.
const CACHED: usize = 4;

/// The LUT at `path`, loaded once per path, size and modification time.
pub fn load(path: &str, tetrahedral: bool) -> Result<Arc<Loaded>, String> {
    static CACHE: Cache = Mutex::new(Vec::new());
    if path.is_empty() {
        return Err(String::new());
    }
    let p = PathBuf::from(path);
    let meta = std::fs::metadata(&p).ok();
    let key = (
        p.clone(),
        tetrahedral,
        meta.as_ref().map_or(0, |m| m.len()),
        meta.as_ref().and_then(|m| m.modified().ok()),
    );
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, r)) = cache.iter().find(|(k, _)| *k == key) {
        return r.clone();
    }
    let r = read(&p, tetrahedral).map(Arc::new);
    if cache.len() >= CACHED {
        drop(cache.remove(0));
    }
    cache.push((key, r.clone()));
    r
}

/// The module's file as loaded, or why it is not used (empty when no file is chosen).
pub fn status(p: &Params) -> Result<Arc<Loaded>, String> {
    let l = Lut3d::from_fields(&p.to_fields());
    load(&l.path, l.tetrahedral)
}

/// The 3D LUT op, param version 1.
pub struct Lut3dOp;

/// WGSL: the space curves and the table interpreter.
pub(crate) const LUT_WGSL: &str = "fn dv_lut_enc1(sp: u32, x: f32) -> f32 {
    if (sp == 2u) {
        if (x <= 0.0078125) { return 10.5402377416545 * x + 0.0729055341958355; }
        return (log2(x) + 9.72) / 17.52;
    }
    if (sp == 3u) {
        if (x <= 0.0031308) { return 12.92 * x; }
        return 1.055 * pow(x, 1.0 / 2.4) - 0.055;
    }
    if (sp == 4u) {
        if (x == 0.0) { return 0.0; }
        return sign(x) * pow(abs(x), 1.0 / 2.4);
    }
    return x;
}
fn dv_lut_dec1(sp: u32, y: f32) -> f32 {
    if (sp == 2u) {
        if (y <= 0.155251141552511) { return (y - 0.0729055341958355) / 10.5402377416545; }
        return min(exp2(y * 17.52 - 9.72), 65504.0);
    }
    if (sp == 3u) {
        if (y <= 0.04045) { return y / 12.92; }
        return pow((y + 0.055) / 1.055, 2.4);
    }
    if (sp == 4u) {
        if (y == 0.0) { return 0.0; }
        return sign(y) * pow(abs(y), 2.4);
    }
    return y;
}
fn dv_lut_l1(o: u32, n: u32, k: u32, x: f32) -> f32 {
    let f = clamp(x, 0.0, f32(n - 1u));
    let i = min(u32(floor(f)), n - 2u);
    let t = f - f32(i);
    return dt(o + i * 3u + k) * (1.0 - t) + dt(o + (i + 1u) * 3u + k) * t;
}
fn dv_lut_at(o: u32, n: u32, r: u32, g: u32, b: u32) -> vec3<f32> {
    let i = o + ((r * n + g) * n + b) * 3u;
    return vec3<f32>(dt(i), dt(i + 1u), dt(i + 2u));
}
fn dv_lut_3d(o: u32, c: vec3<f32>) -> vec3<f32> {
    let n = u32(dt(o + 2u));
    let lo = vec3<f32>(dt(o + 4u), dt(o + 5u), dt(o + 6u));
    let hi = vec3<f32>(dt(o + 7u), dt(o + 8u), dt(o + 9u));
    let v = o + 10u;
    let nm = f32(n - 1u);
    let x = clamp((c - lo) / max(hi - lo, vec3<f32>(1e-30)) * nm, vec3<f32>(0.0), vec3<f32>(nm));
    let i = min(vec3<u32>(floor(x)), vec3<u32>(n - 2u));
    let f = x - vec3<f32>(i);
    let v000 = dv_lut_at(v, n, i.x, i.y, i.z);
    let v111 = dv_lut_at(v, n, i.x + 1u, i.y + 1u, i.z + 1u);
    let fr = f.x;
    let fg = f.y;
    let fb = f.z;
    if (dt(o + 3u) > 0.0) {
        if (fr > fg) {
            if (fg > fb) {
                return (1.0 - fr) * v000 + (fr - fg) * dv_lut_at(v, n, i.x + 1u, i.y, i.z)
                    + (fg - fb) * dv_lut_at(v, n, i.x + 1u, i.y + 1u, i.z) + fb * v111;
            } else if (fr > fb) {
                return (1.0 - fr) * v000 + (fr - fb) * dv_lut_at(v, n, i.x + 1u, i.y, i.z)
                    + (fb - fg) * dv_lut_at(v, n, i.x + 1u, i.y, i.z + 1u) + fg * v111;
            } else {
                return (1.0 - fb) * v000 + (fb - fr) * dv_lut_at(v, n, i.x, i.y, i.z + 1u)
                    + (fr - fg) * dv_lut_at(v, n, i.x + 1u, i.y, i.z + 1u) + fg * v111;
            }
        } else if (fb > fg) {
            return (1.0 - fb) * v000 + (fb - fg) * dv_lut_at(v, n, i.x, i.y, i.z + 1u)
                + (fg - fr) * dv_lut_at(v, n, i.x, i.y + 1u, i.z + 1u) + fr * v111;
        } else if (fb > fr) {
            return (1.0 - fg) * v000 + (fg - fb) * dv_lut_at(v, n, i.x, i.y + 1u, i.z)
                + (fb - fr) * dv_lut_at(v, n, i.x, i.y + 1u, i.z + 1u) + fr * v111;
        }
        return (1.0 - fg) * v000 + (fg - fr) * dv_lut_at(v, n, i.x, i.y + 1u, i.z)
            + (fr - fb) * dv_lut_at(v, n, i.x + 1u, i.y + 1u, i.z) + fb * v111;
    }
    let c00 = dv_lut_at(v, n, i.x, i.y, i.z) * (1.0 - fb) + dv_lut_at(v, n, i.x, i.y, i.z + 1u) * fb;
    let c01 = dv_lut_at(v, n, i.x, i.y + 1u, i.z) * (1.0 - fb)
        + dv_lut_at(v, n, i.x, i.y + 1u, i.z + 1u) * fb;
    let c10 = dv_lut_at(v, n, i.x + 1u, i.y, i.z) * (1.0 - fb)
        + dv_lut_at(v, n, i.x + 1u, i.y, i.z + 1u) * fb;
    let c11 = dv_lut_at(v, n, i.x + 1u, i.y + 1u, i.z) * (1.0 - fb) + v111 * fb;
    let c0 = c00 * (1.0 - fg) + c01 * fg;
    let c1 = c10 * (1.0 - fg) + c11 * fg;
    return c0 * (1.0 - fr) + c1 * fr;
}
fn dv_lut_pw(x: f32, e: f32) -> f32 {
    if (x <= 0.0) {
        if (e == 0.0) { return 1.0; }
        return 0.0;
    }
    return pow(x, e);
}
fn dv_lut_exp(x: f32, e: f32, neg: f32) -> f32 {
    if (x >= 0.0) { return dv_lut_pw(x, e); }
    if (neg == 0.0) { return 0.0; }
    if (neg == 1.0) { return -dv_lut_pw(-x, e); }
    return x;
}
fn dv_lut_explin(q: u32, inv: bool, mirror: bool, x: f32) -> f32 {
    let g = dt(q);
    let o = dt(q + 1u);
    let s = dt(q + 2u);
    let b = dt(q + 3u);
    var v = x;
    var sg = 1.0;
    if (mirror && x < 0.0) {
        v = -x;
        sg = -1.0;
    }
    if (!inv) {
        if (v < b) { return sg * (v / s); }
        return sg * dv_lut_pw((v + o) / (1.0 + o), g);
    }
    if (v < b / s) { return sg * (v * s); }
    return sg * ((1.0 + o) * dv_lut_pw(v, 1.0 / g) - o);
}
fn dv_lut_log(q: u32, inv: bool, lnb: f32, x: f32) -> f32 {
    let has = dt(q + 4u) > 0.0;
    if (!inv) {
        if (has && x <= dt(q + 5u)) { return dt(q + 6u) * x + dt(q + 7u); }
        let v = max(dt(q + 2u) * x + dt(q + 3u), 1.17549435e-38);
        return dt(q) * log(v) / lnb + dt(q + 1u);
    }
    if (has && x <= dt(q + 6u) * dt(q + 5u) + dt(q + 7u)) {
        return (x - dt(q + 7u)) / dt(q + 6u);
    }
    return (exp(lnb * ((x - dt(q + 1u)) / dt(q))) - dt(q + 3u)) / dt(q + 2u);
}
fn dv_lut_cdl(o: u32, c: vec3<f32>) -> vec3<f32> {
    let clp = dt(o + 2u) > 0.0;
    let sat = dt(o + 4u);
    let lw = vec3<f32>(0.2126, 0.7152, 0.0722);
    var v = c;
    if (dt(o + 3u) == 0.0) {
        for (var k = 0u; k < 3u; k++) {
            var x = v[k] * dt(o + 5u + k) + dt(o + 8u + k);
            if (clp) { x = clamp(x, 0.0, 1.0); }
            if (x > 0.0 || !clp) {
                if (x >= 0.0) { x = dv_lut_pw(x, dt(o + 11u + k)); }
            } else {
                x = 0.0;
            }
            v[k] = x;
        }
        let l = lw.x * v.x + lw.y * v.y + lw.z * v.z;
        v = l + sat * (v - l);
        if (clp) { v = clamp(v, vec3<f32>(0.0), vec3<f32>(1.0)); }
        return v;
    }
    let l = lw.x * v.x + lw.y * v.y + lw.z * v.z;
    v = l + (v - l) / sat;
    if (clp) { v = clamp(v, vec3<f32>(0.0), vec3<f32>(1.0)); }
    for (var k = 0u; k < 3u; k++) {
        var x = v[k];
        if (x > 0.0) {
            x = pow(x, 1.0 / dt(o + 11u + k));
        } else if (clp) {
            x = 0.0;
        }
        x = (x - dt(o + 8u + k)) / dt(o + 5u + k);
        if (clp) { x = clamp(x, 0.0, 1.0); }
        v[k] = x;
    }
    return v;
}
fn dv_lut_run(c: vec3<f32>) -> vec3<f32> {
    var v = c;
    let ns = u32(dt(0u));
    var o = 1u;
    for (var s = 0u; s < ns; s++) {
        let kind = u32(dt(o));
        if (kind == 1u) {
            v = vec3<f32>(
                dt(o + 2u) * v.x + dt(o + 3u) * v.y + dt(o + 4u) * v.z + dt(o + 11u),
                dt(o + 5u) * v.x + dt(o + 6u) * v.y + dt(o + 7u) * v.z + dt(o + 12u),
                dt(o + 8u) * v.x + dt(o + 9u) * v.y + dt(o + 10u) * v.z + dt(o + 13u));
        } else if (kind == 2u) {
            v = min(max(v * dt(o + 2u) + dt(o + 3u), vec3<f32>(dt(o + 4u))), vec3<f32>(dt(o + 5u)));
        } else if (kind == 3u) {
            let n = u32(dt(o + 2u));
            for (var k = 0u; k < 3u; k++) {
                let lo = dt(o + 3u + k);
                let span = max(dt(o + 6u + k) - lo, 1e-30);
                v[k] = dv_lut_l1(o + 9u, n, k, (v[k] - lo) / span * f32(n - 1u));
            }
        } else if (kind == 4u) {
            v = dv_lut_3d(o, v);
        } else if (kind == 5u) {
            for (var k = 0u; k < 3u; k++) { v[k] = dv_lut_exp(v[k], dt(o + 2u + k), dt(o + 5u)); }
        } else if (kind == 6u) {
            for (var k = 0u; k < 3u; k++) {
                v[k] = dv_lut_explin(o + 4u + k * 4u, dt(o + 2u) > 0.0, dt(o + 3u) > 0.0, v[k]);
            }
        } else if (kind == 7u) {
            for (var k = 0u; k < 3u; k++) {
                v[k] = dv_lut_log(o + 4u + k * 8u, dt(o + 2u) > 0.0, dt(o + 3u), v[k]);
            }
        } else if (kind == 8u) {
            v = dv_lut_cdl(o, v);
        }
        o += u32(dt(o + 1u));
    }
    return v;
}
";

impl Op for Lut3dOp {
    fn name(&self) -> &'static str {
        "lut_3d_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Point
    }
    fn param_words(&self) -> u32 {
        WORDS as u32
    }
    fn identity(&self, p: &Params) -> bool {
        Lut3d::from_fields(&p.to_fields()).strength == 0.0 || status(p).is_err()
    }
    fn data(&self, p: &Params) -> Option<Arc<OpData>> {
        status(p).ok().map(|l| l.data.clone())
    }
    fn write_params(&self, p: &Params, _: f32, out: &mut [u32]) {
        super::super::put(out, &Lut3d::from_fields(&p.to_fields()).uniforms());
    }
    fn helpers(&self) -> &'static [&'static str] {
        &[super::super::MAT3_WGSL, LUT_WGSL]
    }
    fn wgsl(&self) -> &'static str {
        "    let sp = pu(p);
    let m = dv_mat3(p + 2u, c.rgb);
    let e = vec3<f32>(dv_lut_enc1(sp, m.x), dv_lut_enc1(sp, m.y), dv_lut_enc1(sp, m.z));
    let r = dv_lut_run(e);
    let d = vec3<f32>(dv_lut_dec1(sp, r.x), dv_lut_dec1(sp, r.y), dv_lut_dec1(sp, r.z));
    let o = dv_mat3(p + 11u, d);
    return vec4<f32>(c.rgb + (o - c.rgb) * pf(p + 1u), c.a);"
    }
}
