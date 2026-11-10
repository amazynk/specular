//! Trivial ops that prove the engine (tests only), each with its CPU reference, and the
//! CPU-reference harness: run an edit's test ops over a whole image in plain Rust.
//!
//! The references do the same f32 arithmetic in the same order as the WGSL; sampling clamps
//! to the image edge, as `src_at` does.

use std::sync::Arc;

use super::{Kernel, NodeKind, Op, OpRegistry, Program};
use crate::model::{Edit, Fields, ModuleInstance, ModuleKind, Params, SourceKey, Value};

pub const IDENTITY: ModuleKind = ModuleKind(0xFE01);
pub const GAIN: ModuleKind = ModuleKind(0xFE02);
pub const ADD: ModuleKind = ModuleKind(0xFE03);
pub const BOX3: ModuleKind = ModuleKind(0xFE04);
/// A global statistic: RGB minus the image's mean RGB.
pub const MEAN: ModuleKind = ModuleKind(0xFE05);

fn fields(p: &Params) -> Fields {
    p.to_fields()
}

struct Identity;
impl Op for Identity {
    fn name(&self) -> &'static str {
        "identity_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Point
    }
    fn param_words(&self) -> u32 {
        0
    }
    fn write_params(&self, _: &Params, _: f32, _: &mut [u32]) {}
    fn wgsl(&self) -> &'static str {
        "    return c;"
    }
}

/// RGB × `gain` (alpha kept).
struct Gain;
impl Op for Gain {
    fn name(&self) -> &'static str {
        "gain_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Point
    }
    fn param_words(&self) -> u32 {
        1
    }
    fn write_params(&self, p: &Params, _: f32, out: &mut [u32]) {
        out[0] = fields(p).f32_or("gain", 1.0).to_bits();
    }
    fn wgsl(&self) -> &'static str {
        "    return vec4<f32>(c.rgb * pf(p), c.a);"
    }
}

/// RGB + `offset` (three values; alpha kept).
struct Add;
impl Op for Add {
    fn name(&self) -> &'static str {
        "add_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Point
    }
    fn param_words(&self) -> u32 {
        3
    }
    fn write_params(&self, p: &Params, _: f32, out: &mut [u32]) {
        let o = fields(p).floats_or("offset", [0.0; 3]);
        for (w, v) in out.iter_mut().zip(o) {
            *w = v.to_bits();
        }
    }
    fn wgsl(&self) -> &'static str {
        "    return vec4<f32>(c.rgb + vec3<f32>(pf(p), pf(p + 1u), pf(p + 2u)), c.a);"
    }
}

/// 3×3 box blur of all four channels. Heavy, so its output is a checkpoint.
struct Box3;
impl Op for Box3 {
    fn name(&self) -> &'static str {
        "box3_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Neighbourhood
    }
    fn halo(&self, _: &Params, _: f32) -> u32 {
        1
    }
    fn heavy(&self) -> bool {
        true
    }
    fn param_words(&self) -> u32 {
        0
    }
    fn write_params(&self, _: &Params, _: f32, _: &mut [u32]) {}
    fn wgsl(&self) -> &'static str {
        "    var s = vec4<f32>(0.0);\n\
         \x20   for (var dy = -1; dy <= 1; dy++) {\n\
         \x20       for (var dx = -1; dx <= 1; dx++) {\n\
         \x20           s = s + src_at(x + dx, y + dy);\n\
         \x20       }\n\
         \x20   }\n\
         \x20   return s * (1.0 / 9.0);"
    }
}

/// Row sums of the grid: row `t.x` into words 4 + 4·row.
static MEAN_ROWS: Kernel = Kernel {
    name: "mean_rows_v1",
    atomic: false,
    helpers: &[],
    body: "    var s = vec4<f32>(0.0);
    for (var x = 0u; x < pu(4u); x++) { s += src_at(i32(x), i32(t.x)); }
    let o = 4u + 4u * t.x;
    S[o] = s.x; S[o + 1u] = s.y; S[o + 2u] = s.z; S[o + 3u] = s.w;",
};

/// The mean of the row sums into words 0–3.
static MEAN_TOTAL: Kernel = Kernel {
    name: "mean_total_v1",
    atomic: false,
    helpers: &[],
    body: "    var s = vec4<f32>(0.0);
    for (var y = 0u; y < pu(5u); y++) {
        let o = 4u + 4u * y;
        s += vec4<f32>(S[o], S[o + 1u], S[o + 2u], S[o + 3u]);
    }
    s = s / f32(pu(4u) * pu(5u));
    S[0] = s.x; S[1] = s.y; S[2] = s.z; S[3] = s.w;",
};

struct Mean;
impl Op for Mean {
    fn name(&self) -> &'static str {
        "mean_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Global
    }
    fn param_words(&self) -> u32 {
        0
    }
    fn write_params(&self, _: &Params, _: f32, _: &mut [u32]) {}
    fn has_stats(&self, _: &Params) -> bool {
        true
    }
    fn stats(&self, _: &Params, _w: u32, h: u32, _: f32) -> Option<Program> {
        let mut p = Program {
            words: 4 + 4 * u64::from(h),
            steps: Vec::new(),
            persist: None,
        };
        p.step(&MEAN_ROWS, [h, 1], Vec::new());
        p.step(&MEAN_TOTAL, [1, 1], Vec::new());
        Some(p)
    }
    fn wgsl(&self) -> &'static str {
        "    return vec4<f32>(c.rgb - vec3<f32>(st(0u), st(1u), st(2u)), c.a);"
    }
}

/// The test ops at version 1.
pub fn registry() -> OpRegistry {
    let mut r = OpRegistry::empty();
    r.register(IDENTITY, 1, Arc::new(Identity));
    r.register(GAIN, 1, Arc::new(Gain));
    r.register(ADD, 1, Arc::new(Add));
    r.register(BOX3, 1, Arc::new(Box3));
    r.register(MEAN, 1, Arc::new(Mean));
    r
}

fn instance(kind: ModuleKind, instance: u16, f: Fields) -> ModuleInstance {
    ModuleInstance {
        kind,
        version: 1,
        instance,
        label: None,
        enabled: true,
        params: Params::Other(kind, f),
        blend: None,
    }
}

pub fn identity(i: u16) -> ModuleInstance {
    instance(IDENTITY, i, Fields::new())
}

pub fn gain(i: u16, g: f32) -> ModuleInstance {
    instance(GAIN, i, Fields::new().with("gain", Value::Float(g)))
}

pub fn add(i: u16, o: [f32; 3]) -> ModuleInstance {
    instance(
        ADD,
        i,
        Fields::new().with("offset", Value::Floats(o.to_vec())),
    )
}

pub fn box3(i: u16) -> ModuleInstance {
    instance(BOX3, i, Fields::new())
}

pub fn mean(i: u16) -> ModuleInstance {
    instance(MEAN, i, Fields::new())
}

/// An edit of `modules` in the given order.
pub fn edit(modules: Vec<ModuleInstance>) -> Edit {
    let mut e = Edit::empty(SourceKey::default());
    e.modules = modules;
    e.custom_order = true;
    e
}

/// Sets field `name` of module (`kind`, `inst`).
pub fn set(e: &mut Edit, kind: ModuleKind, inst: u16, name: &str, v: Value) {
    let m = e.module_mut(kind, inst).expect("module in edit");
    let mut f = m.params.to_fields();
    f.set(name, v);
    m.params.set_fields(f);
}

/// The CPU reference of one module over a `w`×`h` RGBA image.
pub fn reference_op(m: &ModuleInstance, w: u32, h: u32, img: &[f32]) -> Vec<f32> {
    let f = m.params.to_fields();
    let mut out = img.to_vec();
    match m.kind {
        IDENTITY => {}
        MEAN => {
            // Row sums, then their mean, in the same f32 order as the kernels.
            let mut s = [0f32; 4];
            for y in 0..h as usize {
                let mut r = [0f32; 4];
                for x in 0..w as usize {
                    for c in 0..4 {
                        r[c] += img[(y * w as usize + x) * 4 + c];
                    }
                }
                for c in 0..4 {
                    s[c] += r[c];
                }
            }
            let n = (w * h) as f32;
            for px in out.chunks_exact_mut(4) {
                for c in 0..3 {
                    px[c] -= s[c] / n;
                }
            }
        }
        GAIN => {
            let g = f.f32_or("gain", 1.0);
            for px in out.chunks_exact_mut(4) {
                for c in &mut px[..3] {
                    *c *= g;
                }
            }
        }
        ADD => {
            let o = f.floats_or("offset", [0.0; 3]);
            for px in out.chunks_exact_mut(4) {
                for (c, v) in px[..3].iter_mut().zip(o) {
                    *c += v;
                }
            }
        }
        BOX3 => {
            let (wi, hi) = (w as i64, h as i64);
            for y in 0..hi {
                for x in 0..wi {
                    let mut s = [0f32; 4];
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            let sx = (x + dx).clamp(0, wi - 1);
                            let sy = (y + dy).clamp(0, hi - 1);
                            let i = ((sy * wi + sx) * 4) as usize;
                            for c in 0..4 {
                                s[c] += img[i + c];
                            }
                        }
                    }
                    let i = ((y * wi + x) * 4) as usize;
                    for c in 0..4 {
                        out[i + c] = s[c] * (1.0 / 9.0);
                    }
                }
            }
        }
        k => panic!("no CPU reference for {k:?}"),
    }
    out
}

/// The CPU reference of a whole edit (enabled modules in order).
pub fn reference(e: &Edit, w: u32, h: u32, img: &[f32]) -> Vec<f32> {
    let mut cur = img.to_vec();
    for m in e.modules.iter().filter(|m| m.enabled) {
        cur = reference_op(m, w, h, &cur);
    }
    cur
}

/// A deterministic synthetic: ramps, a hard edge and hashed noise, values in about [-0.5, 4].
pub fn synthetic(w: u32, h: u32) -> Vec<f32> {
    let mut v = Vec::with_capacity((w as usize) * (h as usize) * 4);
    for y in 0..h {
        for x in 0..w {
            let n = (x.wrapping_mul(0x9E37_79B9) ^ y.wrapping_mul(0x85EB_CA6B))
                .wrapping_mul(0xC2B2_AE35);
            let noise = (n >> 8) as f32 / (1u32 << 24) as f32;
            let edge = if x * 3 > w { 2.0 } else { 0.25 };
            v.push(x as f32 / w as f32 * 4.0 - 0.5);
            v.push(y as f32 / h as f32 + edge);
            v.push(noise * 3.0);
            v.push(1.0 - noise);
        }
    }
    v
}

/// The largest |a − b| and where.
pub fn max_abs(a: &[f32], b: &[f32]) -> (f32, usize) {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .enumerate()
        .map(|(i, (x, y))| ((x - y).abs(), i))
        .fold((0.0, 0), |m, d| if d.0 > m.0 { d } else { m })
}

/// The largest |a − b| / max(1, |b|).
pub fn max_rel(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs() / y.abs().max(1.0))
        .fold(0.0, f32::max)
}
