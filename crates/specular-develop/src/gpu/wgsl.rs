//! WGSL generation: one prelude, then the op functions of a group, then `main`.
//!
//! Every kernel has the same bindings:
//! - 0: the uniform block, [`PARAM_WORDS`](crate::graph::PARAM_WORDS) u32 words. Words 0–19
//!   are the dispatch header, the group's op params follow at word 20.
//! - 1: the destination pixels (read-write storage); a program step's scratch `S`.
//! - 2: the source pixels (read-only storage).
//! - 3: the statistics of the group's first node, `st(i)` (read-only; a dummy otherwise).
//! - 4: the scratch of the group's local program, `lc(i)` (read-only; a dummy otherwise).
//! - 5: the table of the group's node with data, `dt(i)` (read-only; a dummy otherwise).
//! - 6: the mask table of the render, `mku(i)` / `mkf(i)` ([`crate::mask`]; a dummy when
//!   no module blends).
//!
//! Header words:
//!
//! | Words | Meaning |
//! |---|---|
//! | 0–3 | the region computed: x, y, width, height (image pixels; a step's threads) |
//! | 4–5 | image width and height (sampling clamps to these) |
//! | 6–9 | source view: origin x, origin y, row stride, base index |
//! | 10–13 | destination view, the same way |
//! | 16–19 | the region a program covers: x, y, width, height |
//!
//! Image pixel (x, y) of a view is element `base + (y − oy)·stride + (x − ox)`. Pixels are
//! `vec4<f32>` (rgba32float) or `vec2<u32>` holding four packed f16 (rgba16float).
//!
//! The source depends only on the ops (kinds and versions), whether each blends, and the two
//! storage precisions, never on param values, masks, regions or halos: a slider never makes
//! a new kernel.

use crate::graph::{Graph, HEADER_WORDS, Kernel, PARAM_WORDS, Precision};

/// Threads per workgroup side (16×16 = 256, the WebGPU default limit).
pub const WG: u32 = 16;

fn pixel_type(p: Precision) -> &'static str {
    match p {
        Precision::F32 => "vec4<f32>",
        Precision::F16 => "vec2<u32>",
    }
}

/// Uniforms, the source binding and the helpers every kernel and step shares.
fn common(src: Precision) -> String {
    let load = match src {
        Precision::F32 => "return src[i];",
        Precision::F16 => {
            "let w = src[i];\n    return vec4<f32>(unpack2x16float(w.x), unpack2x16float(w.y));"
        }
    };
    format!(
        "@group(0) @binding(0) var<uniform> P: array<vec4<u32>, {vecs}>;\n\
         @group(0) @binding(2) var<storage, read> src: array<{st}>;\n\
         @group(0) @binding(3) var<storage, read> ST: array<f32>;\n\
         @group(0) @binding(5) var<storage, read> DT: array<f32>;\n\
         @group(0) @binding(6) var<storage, read> MK: array<u32>;\n\
         fn pu(i: u32) -> u32 {{ return P[i >> 2u][i & 3u]; }}\n\
         fn mku(i: u32) -> u32 {{ return MK[i]; }}\n\
         fn mkf(i: u32) -> f32 {{ return bitcast<f32>(MK[i]); }}\n\
         fn pf(i: u32) -> f32 {{ return bitcast<f32>(pu(i)); }}\n\
         fn st(i: u32) -> f32 {{ return ST[i]; }}\n\
         fn dt(i: u32) -> f32 {{ return DT[i]; }}\n\
         fn src_at(x: i32, y: i32) -> vec4<f32> {{\n    \
             let cx = u32(clamp(x, 0, i32(pu(4u)) - 1));\n    \
             let cy = u32(clamp(y, 0, i32(pu(5u)) - 1));\n    \
             let i = pu(9u) + (cy - pu(7u)) * pu(8u) + (cx - pu(6u));\n    \
             {load}\n\
         }}\n",
        vecs = PARAM_WORDS / 4,
        st = pixel_type(src),
    )
}

fn prelude(src: Precision, dst: Precision) -> String {
    let store = match dst {
        Precision::F32 => "dst[i] = v;",
        Precision::F16 => "dst[i] = vec2<u32>(pack2x16float(v.xy), pack2x16float(v.zw));",
    };
    format!(
        "{}@group(0) @binding(1) var<storage, read_write> dst: array<{dt}>;\n\
         @group(0) @binding(4) var<storage, read> LC: array<f32>;\n\
         fn lc(i: u32) -> f32 {{ return LC[i]; }}\n\
         fn dst_put(x: u32, y: u32, v: vec4<f32>) {{\n    \
             let i = pu(13u) + (y - pu(11u)) * pu(12u) + (x - pu(10u));\n    \
             {store}\n\
         }}\n",
        common(src),
        dt = pixel_type(dst),
    )
}

/// The kernel of group `g`.
pub fn group_source(graph: &Graph, g: usize, src: Precision, dst: Precision) -> String {
    run_source(graph, g..g + 1, src, dst)
}

/// One kernel for the groups `run`: consecutive groups of point-like nodes, the
/// pixel rounded to the storage between them as their own kernels store and load it (f16:
/// `pack2x16float`, then `unpack2x16float`; f32 is exact), so the result is theirs bit for
/// bit without the pictures in between.
pub fn run_source(
    graph: &Graph,
    run: std::ops::Range<usize>,
    src: Precision,
    dst: Precision,
) -> String {
    let mut s = prelude(src, dst);
    let first = graph.groups[run.start].nodes.start;
    let nodes = &graph.nodes[first..graph.groups[run.end - 1].nodes.end];
    debug_assert!(
        graph.groups[run.start + 1..run.end].iter().all(|g| g.fused),
        "only point groups follow in a run"
    );
    // Node indexes (in `nodes`) before which the pixel was stored and loaded.
    let ends: Vec<usize> = graph.groups[run.start..run.end - 1]
        .iter()
        .map(|g| g.nodes.end - first)
        .collect();
    let mut defined: Vec<&str> = Vec::new();
    let mut helpers: Vec<&str> = Vec::new();
    if nodes.iter().any(|n| n.blend.is_some()) {
        for h in crate::mask::shader::HELPERS.iter() {
            if !helpers.contains(h) {
                helpers.push(h);
                s.push_str(h);
            }
        }
    }
    for n in nodes {
        let name = n.op.name();
        if defined.contains(&name) {
            continue;
        }
        defined.push(name);
        for h in n.op.helpers() {
            if !helpers.contains(h) {
                helpers.push(h);
                s.push_str(h);
            }
        }
        let sig = if n.op.kind().pointlike() {
            "c: vec4<f32>, px: i32, py: i32, p: u32"
        } else {
            "x: i32, y: i32, p: u32"
        };
        s.push_str(&format!(
            "fn op_{name}({sig}) -> vec4<f32> {{\n{}\n}}\n",
            n.op.wgsl()
        ));
    }
    s.push_str(&format!(
        "@compute @workgroup_size({WG}, {WG}, 1)\n\
         fn main(@builtin(global_invocation_id) gid: vec3<u32>) {{\n    \
             if (gid.x >= pu(2u) || gid.y >= pu(3u)) {{ return; }}\n    \
             let x = pu(0u) + gid.x;\n    \
             let y = pu(1u) + gid.y;\n"
    ));
    let mut at = HEADER_WORDS;
    // A kernel node leads its group (alone); point nodes may follow in a run.
    let lead = match nodes.first() {
        Some(n) if !n.op.kind().pointlike() => {
            s.push_str(&format!(
                "    var c = op_{}(i32(x), i32(y), {at}u);\n",
                n.op.name()
            ));
            if n.blend.is_some() {
                let q = at + n.op.param_words();
                s.push_str(&format!(
                    "    c = dv_blend(src_at(i32(x), i32(y)), c, i32(x), i32(y), {q}u);\n"
                ));
            }
            at += n.words();
            1
        }
        _ => {
            s.push_str("    var c = src_at(i32(x), i32(y));\n");
            0
        }
    };
    for (i, n) in nodes.iter().enumerate().skip(lead) {
        if ends.contains(&i) && dst == Precision::F16 {
            s.push_str(
                "    c = vec4<f32>(unpack2x16float(pack2x16float(c.xy)), \
                 unpack2x16float(pack2x16float(c.zw)));\n",
            );
        }
        if n.blend.is_some() {
            s.push_str(&format!("    let b{i} = c;\n"));
        }
        s.push_str(&format!(
            "    c = op_{}(c, i32(x), i32(y), {at}u);\n",
            n.op.name()
        ));
        if n.blend.is_some() {
            let q = at + n.op.param_words();
            s.push_str(&format!(
                "    c = dv_blend(b{i}, c, i32(x), i32(y), {q}u);\n"
            ));
        }
        at += n.words();
    }
    s.push_str("    dst_put(x, y, c);\n}\n");
    s
}

/// A plain copy (an empty fused group): an empty graph, or a fully cached one.
pub fn copy_source(src: Precision, dst: Precision) -> String {
    format!(
        "{}@compute @workgroup_size({WG}, {WG}, 1)\n\
         fn main(@builtin(global_invocation_id) gid: vec3<u32>) {{\n    \
             if (gid.x >= pu(2u) || gid.y >= pu(3u)) {{ return; }}\n    \
             let x = pu(0u) + gid.x;\n    \
             let y = pu(1u) + gid.y;\n    \
             dst_put(x, y, src_at(i32(x), i32(y)));\n}}\n",
        prelude(src, dst)
    )
}

/// The kernel of a program step reading a `src` image.
pub fn step_source(k: &Kernel, src: Precision) -> String {
    let mut s = common(src);
    // The pictures' storage precision, for steps that keep their scratch at it.
    s.push_str(&format!(
        "const STEP_F16: bool = {};\n",
        src == Precision::F16
    ));
    if k.atomic {
        s.push_str("@group(0) @binding(1) var<storage, read_write> S: array<atomic<u32>>;\n");
    } else if k.body.starts_with(crate::graph::U32_BODY) {
        s.push_str("@group(0) @binding(1) var<storage, read_write> S: array<u32>;\n");
    } else {
        s.push_str("@group(0) @binding(1) var<storage, read_write> S: array<f32>;\n");
    }
    for h in k.helpers {
        s.push_str(h);
    }
    // A body may start with both markers (`//u32`, then `//workgroup`).
    let rest = k
        .body
        .strip_prefix(crate::graph::U32_BODY)
        .unwrap_or(k.body);
    if let Some(body) = rest.strip_prefix(crate::graph::WORKGROUP_BODY) {
        // Once per workgroup (see `Kernel`): no thread returns early, so the body's barriers
        // stay in uniform control flow.
        s.push_str(&format!(
            "fn step_{name}(w: u32, li: u32, p: u32) {{\n{body}\n}}\n\
             @compute @workgroup_size({WG}, {WG}, 1)\n\
             fn main(@builtin(workgroup_id) wid: vec3<u32>,\n        \
                 @builtin(local_invocation_index) li: u32) {{\n    \
                 step_{name}(wid.y * (pu(2u) / {WG}u) + wid.x, li, {HEADER_WORDS}u);\n}}\n",
            name = k.name,
        ));
        return s;
    }
    s.push_str(&format!(
        "fn step_{name}(t: vec2<u32>, p: u32) {{\n{body}\n}}\n\
         @compute @workgroup_size({WG}, {WG}, 1)\n\
         fn main(@builtin(global_invocation_id) gid: vec3<u32>) {{\n    \
             if (gid.x >= pu(2u) || gid.y >= pu(3u)) {{ return; }}\n    \
             step_{name}(gid.xy, {HEADER_WORDS}u);\n}}\n",
        name = k.name,
        body = k.body,
    ));
    s
}

/// Source pixels a row of [`GRID_REDUCE_WG`]'s workgroup memory holds.
pub const GRID_ROW_MAX: u32 = 512;

/// [`GRID_REDUCE`] per workgroup of 16×16 grid cells: the source rows under the
/// workgroup are read one at a time into workgroup memory by all its threads (side by side,
/// so the reads are contiguous), and each cell sums its pixels from there. Every weight,
/// product and sum is [`GRID_REDUCE`]'s, in the same order, so the grid is its grid bit for
/// bit. Words: grid width and height (the threads are rounded up to 16). The cells of a
/// workgroup must span at most [`GRID_ROW_MAX`] source pixels a row ([`grid_reduce_wg_fits`]).
pub static GRID_REDUCE_WG: Kernel = Kernel {
    name: "grid_reduce_wg_v1",
    atomic: false,
    helpers: &["var<workgroup> GR_T: array<vec4<f32>, 512>;\n"],
    body: "//workgroup
    let gw = pu(p);
    let gh = pu(p + 1u);
    let sw = f32(pu(4u));
    let sh = f32(pu(5u));
    let fx = sw / f32(gw);
    let fy = sh / f32(gh);
    let tgx = pu(2u) / 16u;
    let cx0 = (w % tgx) * 16u;
    let cy0 = (w / tgx) * 16u;
    let tx = cx0 + li % 16u;
    let ty = cy0 + li / 16u;
    let on = tx < gw && ty < gh;
    let x0 = f32(tx) * fx;
    let x1 = x0 + fx;
    let y0 = f32(ty) * fy;
    let y1 = y0 + fy;
    let xs = u32(floor(x0));
    let ys = u32(floor(y0));
    let ye = min(u32(ceil(y1)), pu(5u));
    let xe = min(u32(ceil(x1)), pu(4u));
    // The workgroup's pixels: from its first cell's first to its last cell's last.
    let lx = f32(min(cx0 + 15u, gw - 1u)) * fx;
    let ly = f32(min(cy0 + 15u, gh - 1u)) * fy;
    let bx0 = u32(floor(f32(cx0) * fx));
    let bx1 = min(u32(ceil(lx + fx)), pu(4u));
    let by0 = u32(floor(f32(cy0) * fy));
    let by1 = min(u32(ceil(ly + fy)), pu(5u));
    var acc = vec4<f32>(0.0);
    var wsum = 0.0;
    for (var y = by0; y < by1; y++) {
        workgroupBarrier();
        for (var k = li; k < bx1 - bx0; k += 256u) {
            GR_T[k] = src_at(i32(bx0 + k), i32(y));
        }
        workgroupBarrier();
        if (on && y >= ys && y < ye) {
            let wy = min(f32(y + 1u), y1) - max(f32(y), y0);
            for (var x = xs; x < xe; x++) {
                let wx = min(f32(x + 1u), x1) - max(f32(x), x0);
                let ww = wx * wy;
                if (ww > 0.0) {
                    acc += GR_T[x - bx0] * ww;
                    wsum += ww;
                }
            }
        }
    }
    if (on) {
        let v = acc / max(wsum, 1e-12);
        let o = 4u * (ty * gw + tx);
        S[o] = v.x;
        S[o + 1u] = v.y;
        S[o + 2u] = v.z;
        S[o + 3u] = v.w;
    }",
};

/// Whether [`GRID_REDUCE_WG`] holds a row of a `gw`-wide grid of a `sw`-wide source.
pub fn grid_reduce_wg_fits(sw: u32, gw: u32) -> bool {
    // 16 cells and a pixel either side for the rounding.
    gw > 0 && u64::from(sw) * 16 / u64::from(gw) + 3 <= u64::from(GRID_ROW_MAX)
}

/// The statistics grid: the whole source reduced to `pu(2)`×`pu(3)` by area-weighted
/// averages, written as rgba32float pixels to the scratch.
pub static GRID_REDUCE: Kernel = Kernel {
    name: "grid_reduce_v1",
    atomic: false,
    helpers: &[],
    body: "    let sw = f32(pu(4u));
    let sh = f32(pu(5u));
    let fx = sw / f32(pu(2u));
    let fy = sh / f32(pu(3u));
    let x0 = f32(t.x) * fx;
    let x1 = x0 + fx;
    let y0 = f32(t.y) * fy;
    let y1 = y0 + fy;
    var acc = vec4<f32>(0.0);
    var wsum = 0.0;
    let ye = min(u32(ceil(y1)), pu(5u));
    let xe = min(u32(ceil(x1)), pu(4u));
    for (var y = u32(floor(y0)); y < ye; y++) {
        let wy = min(f32(y + 1u), y1) - max(f32(y), y0);
        for (var x = u32(floor(x0)); x < xe; x++) {
            let wx = min(f32(x + 1u), x1) - max(f32(x), x0);
            let w = wx * wy;
            if (w > 0.0) {
                acc += src_at(i32(x), i32(y)) * w;
                wsum += w;
            }
        }
    }
    let v = acc / max(wsum, 1e-12);
    let o = 4u * (t.y * pu(2u) + t.x);
    S[o] = v.x;
    S[o + 1u] = v.y;
    S[o + 2u] = v.z;
    S[o + 3u] = v.w;",
};
