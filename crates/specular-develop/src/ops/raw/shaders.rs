//! Kernel sources of the raw front end: bindings (typed per pass), `common.wgsl`, the pass
//! body from `passes.wgsl`, and `main`.
//!
//! Bindings of every pass: 0 the uniform words, 1 the destination, 2 and 3 sources A and B,
//! 4 the stored samples (u16 pairs; the corrected mosaic as f32 when `U_PREPARED`), 5 the
//! data buffer, 6 the calibration (two f16 per crop pixel).

use crate::graph::Precision;

const COMMON: &str = include_str!("common.wgsl");
const PASSES: &str = include_str!("passes.wgsl");
const CORRECT: &str = include_str!("correct.wgsl");
const CORRECT_COMMON: &str = include_str!("correct_common.wgsl");
const HALF_CORRECT: &str = include_str!("half_correct.wgsl");
const XT_COMMON: &str = include_str!("xt_common.wgsl");

/// Threads per workgroup side.
pub(crate) const WG: u32 = 16;

/// A buffer's element type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Ty {
    /// One f32 per pixel (the mosaic).
    F1,
    /// vec4<f32> per element.
    V4,
    /// The output: rgba32float or packed rgba16float.
    Out,
    /// The corrected mosaic: one f32 per pixel, written with `put1`.
    Mos,
    /// Two packed words per element (statistics cells).
    P2,
}

/// One kind of dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Pass {
    Prep,
    Direct,
    Mosaic,
    Half,
    Mhc,
    Rcd1,
    Rcd2,
    Rcd3,
    Rcd4,
    /// Step 4 with the dual demosaic blend.
    Rcd4d,
    Xt1,
    Xt2,
    Xt3,
    /// Steps 4 to 6 in one pass (workgroup memory).
    Xt6,
    // Corrections on the mosaic and their statistics.
    /// Full size, the three-pass path the fused `CFuse` replaces (the tests compare them).
    #[cfg_attr(not(test), expect(dead_code))]
    CPrep,
    #[cfg_attr(not(test), expect(dead_code))]
    Hot,
    #[cfg_attr(not(test), expect(dead_code))]
    Hl,
    /// `CPrep`, `Hot` and `Hl` in one pass (workgroup memory).
    CFuse,
    /// The green plane for X-Trans raw CA.
    XtG,
    Ca,
    /// Statistics cells from the stored samples.
    RCell,
    HlTile,
    Block,
    /// Half size with corrections, the three-pass path (the tests compare the fused
    /// passes with it; `HOut` also follows `HFuseB`).
    #[cfg_attr(not(test), expect(dead_code))]
    HCell,
    #[cfg_attr(not(test), expect(dead_code))]
    HHot,
    HOut,
    /// `HCell`, `HHot` and `HOut` without raw CA in one pass (workgroup memory).
    HFuse,
    /// `HCell` and `HHot` in one pass, for `HOut` with raw CA.
    HFuseB,
    /// `HFuse` and `HFuseB` for X-Trans: the overlapping 4x4 windows' samples read once
    /// into workgroup memory (same values bit for bit).
    HFuseX,
    HFuseBX,
    /// Area reduction of a stripe of full-size output into the reduced output
    /// ([`super::Options::reduce_to`]).
    Reduce,
}

impl Pass {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Pass::Prep => "prep",
            Pass::Direct => "direct",
            Pass::Mosaic => "mosaic",
            Pass::Half => "half",
            Pass::Mhc => "mhc",
            Pass::Rcd1 => "rcd1",
            Pass::Rcd2 => "rcd2",
            Pass::Rcd3 => "rcd3",
            Pass::Rcd4 => "rcd4",
            Pass::Rcd4d => "rcd4d",
            Pass::Xt1 => "xt1",
            Pass::Xt2 => "xt2",
            Pass::Xt3 => "xt3",
            Pass::Xt6 => "xt6",
            Pass::CPrep => "cprep",
            Pass::CFuse => "cfuse",
            Pass::Hot => "hot",
            Pass::Hl => "hl",
            Pass::XtG => "xtg",
            Pass::Ca => "ca",
            Pass::RCell => "rcell",
            Pass::HCell => "hcell",
            Pass::HHot => "hhot",
            Pass::HOut => "hout",
            Pass::HFuse => "hfuse",
            Pass::HFuseB => "hfuseb",
            Pass::HFuseX => "hfusex",
            Pass::HFuseBX => "hfusebx",
            Pass::HlTile => "hltile",
            Pass::Block => "block",
            Pass::Reduce => "reduce",
        }
    }

    /// Types of the destination and sources A and B.
    pub(crate) fn types(self) -> [Ty; 3] {
        use Ty::*;
        match self {
            Pass::Prep => [F1, F1, F1],
            Pass::Direct | Pass::Mosaic | Pass::Half => [Out, F1, F1],
            Pass::Mhc => [Out, F1, F1],
            Pass::Rcd1 | Pass::Xt1 => [V4, F1, F1],
            Pass::Rcd2 | Pass::Rcd3 | Pass::Xt3 => [V4, V4, F1],
            Pass::Rcd4 | Pass::Rcd4d => [Out, V4, F1],
            Pass::Xt2 => [V4, F1, V4],
            Pass::Xt6 => [Out, V4, V4],
            Pass::CPrep => [V4, F1, F1],
            Pass::Hot => [V4, V4, F1],
            Pass::Hl => [F1, V4, F1],
            Pass::CFuse => [F1, F1, F1],
            Pass::Ca => [Mos, F1, F1],
            Pass::XtG => [F1, F1, F1],
            Pass::RCell => [P2, F1, F1],
            Pass::HCell => [V4, F1, F1],
            Pass::HHot => [V4, V4, F1],
            Pass::HOut | Pass::HFuse | Pass::HFuseX => [Out, V4, F1],
            Pass::HFuseB | Pass::HFuseBX => [V4, V4, F1],
            Pass::HlTile | Pass::Block => [F1, P2, F1],
            Pass::Reduce => [Out, Out, F1],
        }
    }

    /// Whether the pass brings its own `main` (one that synchronises its workgroup).
    pub(crate) fn fused(self) -> bool {
        matches!(
            self,
            Pass::HFuse | Pass::HFuseB | Pass::HFuseX | Pass::HFuseBX
        )
    }

    /// The pass whose body this one shares (the X-Trans fused passes differ in their
    /// step 1 only).
    fn body_of(self) -> Pass {
        match self {
            Pass::HFuseX => Pass::HFuse,
            Pass::HFuseBX => Pass::HFuseB,
            p => p,
        }
    }

    /// Whether the destination is the output.
    pub(crate) fn writes_output(self) -> bool {
        self.types()[0] == Ty::Out
    }
}

fn ty(t: Ty, out: Precision) -> &'static str {
    match (t, out) {
        (Ty::F1, _) => "f32",
        (Ty::V4, _) | (Ty::Out, Precision::F32) => "vec4<f32>",
        (Ty::Out, Precision::F16) | (Ty::P2, _) => "vec2<u32>",
        (Ty::Mos, _) => "f32",
    }
}

fn body(name: &str) -> &'static str {
    section(
        &[PASSES, CORRECT],
        &format!("//@ pass {name}\n"),
        "//@ pass ",
    )
}

/// The text after `tag` in the first of `files` holding it, to the next `next` (or the end).
fn section(files: &[&'static str], tag: &str, next: &str) -> &'static str {
    let (src, at) = files
        .iter()
        .find_map(|s| s.find(tag).map(|at| (*s, at)))
        .expect("every pass has a body");
    let start = at + tag.len();
    let end = src[start..].find(next).map_or(src.len(), |e| start + e);
    &src[start..end]
}

/// A per-pixel step of the half-size corrections (`half_correct.wgsl`).
fn half_fn(name: &str) -> &'static str {
    section(&[HALF_CORRECT], &format!("//@ fn {name}\n"), "//@ fn ")
}

/// The kernel of `pass` writing `out` precision (when its destination is the output).
pub(crate) fn source(pass: Pass, out: Precision) -> String {
    let [d, a, b] = pass.types();
    let mut s = format!(
        "@group(0) @binding(0) var<uniform> U: array<vec4<u32>, 4>;\n\
         @group(0) @binding(1) var<storage, read_write> dst: array<{}>;\n\
         @group(0) @binding(2) var<storage, read> srca: array<{}>;\n\
         @group(0) @binding(3) var<storage, read> srcb: array<{}>;\n\
         @group(0) @binding(4) var<storage, read> RAW: array<u32>;\n\
         @group(0) @binding(5) var<storage, read> D: array<u32>;\n\
         @group(0) @binding(6) var<storage, read> CAL: array<u32>;\n",
        ty(d, out),
        ty(a, out),
        ty(b, out),
    );
    s.push_str(COMMON);
    if d == Ty::Out {
        let store = match out {
            Precision::F32 => "dst[i] = v;",
            Precision::F16 => "dst[i] = vec2<u32>(pack2x16float(v.xy), pack2x16float(v.zw));",
        };
        s.push_str(&format!(
            "fn put(x: i32, y: i32, v: vec4<f32>) {{\n    \
                 let i = uu(U_D_BASE) + u32(y - ui(U_D_Y0)) * uu(U_W) + u32(x);\n    \
                 {store}\n}}\n"
        ));
    }
    if a == Ty::Out {
        // Source A in the output's format (a stripe of a reduced run).
        s.push_str(match out {
            Precision::F32 => "fn lda(i: u32) -> vec4<f32> { return srca[i]; }\n",
            Precision::F16 => {
                "fn lda(i: u32) -> vec4<f32> {\n    \
                     let p = srca[i];\n    \
                     return vec4<f32>(unpack2x16float(p.x), unpack2x16float(p.y));\n}\n"
            }
        });
    }
    if d == Ty::Mos {
        s.push_str(
            "fn put1(x: i32, y: i32, v: f32) {\n    \
                 dst[uu(U_D_BASE) + u32(y - ui(U_D_Y0)) * uu(U_W) + u32(x)] = v;\n}\n",
        );
    }
    if matches!(pass, Pass::Hot | Pass::Hl | Pass::HHot) || pass.fused() {
        // Its (sample, clip level) pairs from source A.
        s.push_str(
            "fn ld(p: vec2<i32>) -> vec2<f32> {\n    \
                 return srca[u32(p.y - ui(U_A_Y0)) * uu(U_W) + u32(p.x)].xy;\n}\n",
        );
    }
    if matches!(pass, Pass::Hot | Pass::Hl | Pass::HHot | Pass::CFuse) || pass.fused() {
        s.push_str(CORRECT_COMMON);
    }
    if matches!(pass, Pass::XtG | Pass::Ca | Pass::Xt1 | Pass::Xt2) {
        s.push_str(XT_COMMON);
    }
    if matches!(pass, Pass::HCell) || pass.fused() {
        s.push_str(half_fn("hcell"));
    }
    if matches!(pass, Pass::HHot) || pass.fused() {
        s.push_str(half_fn("hhot"));
    }
    if matches!(pass, Pass::HFuse | Pass::HFuseB) {
        s.push_str(half_fn("htfill"));
    }
    if matches!(pass, Pass::HFuseX | Pass::HFuseBX) {
        s.push_str(half_fn("htfillx"));
    }
    s.push_str(body(pass.body_of().name()));
    if pass.fused() || matches!(pass, Pass::Xt2 | Pass::Xt3 | Pass::Xt6 | Pass::CFuse) {
        // Its own `main` (workgroup memory and a barrier).
        return s;
    }
    s.push_str(&format!(
        "@compute @workgroup_size({WG}, {WG}, 1)\n\
         fn main(@builtin(global_invocation_id) gid: vec3<u32>) {{\n    \
             if (gid.x >= uu(U_W) || gid.y >= uu(U_ROWS)) {{ return; }}\n    \
             run(i32(gid.x), i32(uu(U_Y0) + gid.y));\n}}\n"
    ));
    s
}
