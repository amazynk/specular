//! The program kernels of the detail ops: the variance-stabilising transform into an
//! opponent space, edge-avoiding à-trous wavelet shrinkage, non-local means, separable
//! Gaussian blurs (plain, and the two halves of a Richardson–Lucy iteration), local minimum
//! and maximum, and luminance planes.
//!
//! Every plane is a row-major f32 array in the scratch `S` at a word offset given in the
//! step's words; reads clamp to the plane's edge, which is the region's edge (the node's
//! halo covers every step's reach). [`super::cpu`] runs the same kernels in Rust for the
//! tests.

use crate::graph::Kernel;

/// Shared WGSL: plane reads, the generalised Anscombe transform and its inverses.
pub const COMMON_WGSL: &str = "const DV_DLUMA = vec3<f32>(0.2627, 0.6780, 0.0593);
fn dv_pt(o: u32, w: u32, h: u32, x: i32, y: i32) -> f32 {
    let cx = u32(clamp(x, 0, i32(w) - 1));
    let cy = u32(clamp(y, 0, i32(h) - 1));
    return S[o + cy * w + cx];
}
fn dv_gat(z: f32, ia: f32, s2: f32) -> f32 {
    let u = z * ia + 0.375 + s2;
    if (u >= 0.0625) { return 2.0 * sqrt(u); }
    return 0.5 + (u - 0.0625) * 4.0;
}
";

/// The inverses of [`COMMON_WGSL`]'s transform (used by the node kernel, which has no `S`).
pub const INVERSE_WGSL: &str = "fn dv_gat_alg(d: f32) -> f32 {
    if (d >= 0.5) { return 0.25 * d * d; }
    return 0.0625 + (d - 0.5) * 0.25;
}
fn dv_gat_inv(d: f32, ia: f32, s2: f32, m: f32) -> f32 {
    let alg = dv_gat_alg(d);
    var ub = 0.0;
    if (d >= 1.0) {
        let r = 1.0 / d;
        ub = 0.25 * d * d + 0.30618621784789724 * r - 1.375 * r * r
            + 0.7654655446197431 * r * r * r + 0.25;
    } else {
        ub = alg - 0.05334826753235061;
    }
    let u = mix(alg, ub, m);
    return (u - 0.375 - s2) / ia;
}
";

/// Words: destination (three planes Y, U, V), width, region x, region y, 1/a (R, G, B),
/// σ'² (R, G, B), plane size. Each channel through the generalised Anscombe transform
/// (unit noise variance), then an orthonormal opponent transform.
pub static VST: Kernel = Kernel {
    name: "detail_vst_v1",
    atomic: false,
    helpers: &[COMMON_WGSL],
    body: "    let c = src_at(i32(pu(p + 2u) + t.x), i32(pu(p + 3u) + t.y));
    let r = dv_gat(c.x, pf(p + 4u), pf(p + 7u));
    let g = dv_gat(c.y, pf(p + 5u), pf(p + 8u));
    let b = dv_gat(c.z, pf(p + 6u), pf(p + 9u));
    let n = pu(p + 10u);
    let i = pu(p) + t.y * pu(p + 1u) + t.x;
    S[i] = (r + g + b) * 0.5773502691896258;
    S[i + n] = (r - b) * 0.7071067811865476;
    S[i + 2u * n] = (r - 2.0 * g + b) * 0.4082482904638631;",
};

/// Words: current (3 planes), next (3 planes), accumulator (3 planes), width, height, plane
/// size, step, thresholds Y, U, V (f32), 1/σ_r² (f32), first, last.
///
/// One edge-avoiding à-trous level (B3 spline taps at `step` spacing, weighted by the
/// opponent distance to the centre): `next` is the smoother image, the detail
/// `current − next` is soft-thresholded per channel and added to the accumulator (which the
/// first level starts and the last level closes with `next`). With zero thresholds the
/// accumulator rebuilds the input exactly. The last level does not store `next` (nothing
/// reads it).
pub static ATROUS: Kernel = Kernel {
    name: "detail_atrous_v2",
    atomic: false,
    helpers: &[COMMON_WGSL],
    body: "    let co = pu(p);
    let no = pu(p + 1u);
    let ao = pu(p + 2u);
    let w = pu(p + 3u);
    let h = pu(p + 4u);
    let n = pu(p + 5u);
    let st = i32(pu(p + 6u));
    let ir = pf(p + 10u);
    let i = t.y * w + t.x;
    let c = vec3<f32>(S[co + i], S[co + n + i], S[co + 2u * n + i]);
    var acc = vec3<f32>(0.0);
    var ws = 0.0;
    for (var j = -2; j <= 2; j++) {
        let hy = select(select(0.0625, 0.25, abs(j) == 1), 0.375, j == 0);
        let y = i32(t.y) + j * st;
        for (var k = -2; k <= 2; k++) {
            let hx = select(select(0.0625, 0.25, abs(k) == 1), 0.375, k == 0);
            let x = i32(t.x) + k * st;
            let q = vec3<f32>(dv_pt(co, w, h, x, y), dv_pt(co + n, w, h, x, y),
                dv_pt(co + 2u * n, w, h, x, y));
            let d = q - c;
            let wt = hx * hy * exp(-dot(d, d) * ir);
            acc += wt * q;
            ws += wt;
        }
    }
    let nx = acc / ws;
    let dt = c - nx;
    let th = vec3<f32>(pf(p + 7u), pf(p + 8u), pf(p + 9u));
    var sh = sign(dt) * max(abs(dt) - th, vec3<f32>(0.0));
    if (pu(p + 12u) != 0u) { sh += nx; }
    if (pu(p + 11u) == 0u) {
        sh += vec3<f32>(S[ao + i], S[ao + n + i], S[ao + 2u * n + i]);
    }
    if (pu(p + 12u) == 0u) {
        S[no + i] = nx.x;
        S[no + n + i] = nx.y;
        S[no + 2u * n + i] = nx.z;
    }
    S[ao + i] = sh.x;
    S[ao + n + i] = sh.y;
    S[ao + 2u * n + i] = sh.z;",
};

/// Largest lattice spacing [`ATROUS_WG`] takes.
pub const ATROUS_WG_MAX_STEP: u32 = 1 << 16;

/// Workgroup memory of [`ATROUS_WG`]: 20×20 lattice points (16 and 2 a side).
const ATROUS_WG_WGSL: &str = "var<workgroup> AT_T: array<vec3<f32>, 400>;
";

/// [`ATROUS`] per workgroup: a workgroup takes 16×16 pixels `step` apart (one
/// residue of the level's lattice; the residues side by side in the dispatch), whose taps
/// all lie on the same lattice: its 20×20 points (each the clamped read [`ATROUS`] makes
/// there) are read once into workgroup memory, instead of 25 reads of three planes per
/// pixel. Same words, the same arithmetic in the same order, so [`ATROUS`]'s values bit for
/// bit. Threads: [`atrous_wg_threads`].
pub static ATROUS_WG: Kernel = Kernel {
    name: "detail_atrous_wg_v1",
    atomic: false,
    helpers: &[COMMON_WGSL, ATROUS_WG_WGSL],
    body: "//workgroup
    let co = pu(p);
    let no = pu(p + 1u);
    let ao = pu(p + 2u);
    let iw = pu(p + 3u);
    let ih = pu(p + 4u);
    let n = pu(p + 5u);
    let su = pu(p + 6u);
    let st = i32(su);
    let ir = pf(p + 10u);
    let gx = pu(2u) / 16u;
    let wx = w % gx;
    let wy = w / gx;
    let x0 = i32((wx / su) * 16u * su + wx % su);
    let y0 = i32((wy / su) * 16u * su + wy % su);
    for (var k = li; k < 400u; k += 256u) {
        let xx = x0 + (i32(k % 20u) - 2) * st;
        let yy = y0 + (i32(k / 20u) - 2) * st;
        AT_T[k] = vec3<f32>(dv_pt(co, iw, ih, xx, yy), dv_pt(co + n, iw, ih, xx, yy),
            dv_pt(co + 2u * n, iw, ih, xx, yy));
    }
    workgroupBarrier();
    let lx = li % 16u;
    let ly = li / 16u;
    let px = x0 + i32(lx) * st;
    let py = y0 + i32(ly) * st;
    if (px >= i32(iw) || py >= i32(ih)) { return; }
    let i = u32(py) * iw + u32(px);
    let o = i32((ly + 2u) * 20u + lx + 2u);
    let c = AT_T[o];
    var acc = vec3<f32>(0.0);
    var ws = 0.0;
    // 2, from a word the compiler cannot see (the step is never 0): the tap loops stay
    // loops, as [`ATROUS`]'s do (unrolled, the compiler rounds some sums differently).
    let r2 = i32(select(2u, 3u, su == 0u));
    for (var j = -2; j <= r2; j++) {
        let hy = select(select(0.0625, 0.25, abs(j) == 1), 0.375, j == 0);
        for (var k = -2; k <= r2; k++) {
            let hx = select(select(0.0625, 0.25, abs(k) == 1), 0.375, k == 0);
            let q = AT_T[o + j * 20 + k];
            let d = q - c;
            let wt = hx * hy * exp(-dot(d, d) * ir);
            acc += wt * q;
            ws += wt;
        }
    }
    let nx = acc / ws;
    let dt = c - nx;
    let th = vec3<f32>(pf(p + 7u), pf(p + 8u), pf(p + 9u));
    var sh = sign(dt) * max(abs(dt) - th, vec3<f32>(0.0));
    if (pu(p + 12u) != 0u) { sh += nx; }
    if (pu(p + 11u) == 0u) {
        sh += vec3<f32>(S[ao + i], S[ao + n + i], S[ao + 2u * n + i]);
    }
    if (pu(p + 12u) == 0u) {
        S[no + i] = nx.x;
        S[no + n + i] = nx.y;
        S[no + 2u * n + i] = nx.z;
    }
    S[ao + i] = sh.x;
    S[ao + n + i] = sh.y;
    S[ao + 2u * n + i] = sh.z;",
};

/// [`ATROUS_WG`]'s threads for a `w × h` plane at lattice spacing `step`: per residue of
/// the lattice in each direction, 16 threads per 16 lattice points.
pub fn atrous_wg_threads(w: u32, h: u32, step: u32) -> [u32; 2] {
    let side = 16 * step;
    [w.div_ceil(side) * side, h.div_ceil(side) * side]
}

/// The pixel [`ATROUS_WG`]'s thread `(tx, ty)` makes at spacing `step` (the CPU reference).
pub fn atrous_wg_pixel(tx: u32, ty: u32, step: u32) -> (u32, u32) {
    let at = |t: u32| {
        let (g, l) = (t / 16, t % 16);
        (g / step) * 16 * step + g % step + l * step
    };
    (at(tx), at(ty))
}

/// Workgroup memory of [`ATROUS_0`]: the transform over the tile and 2 pixels around.
const ATROUS_0_WGSL: &str = "var<workgroup> A0_T: array<vec3<f32>, 400>;
";

/// [`VST`] and the first [`ATROUS`] level (spacing 1) in one pass per 16×16 tile:
/// the transform over the tile and the 2 pixels the level reaches around it goes to
/// workgroup memory (each value at its clamped position, as [`ATROUS`] reads it), never to
/// the scratch. The arithmetic is the two steps' in the same order (tap loops as
/// [`ATROUS_WG`]'s), so their values bit for bit. Words: next (3 planes), accumulator (3
/// planes), width, height, plane size, region x, region y, 1/a (R, G, B), σ'² (R, G, B),
/// thresholds Y, U, V, 1/σ_r², last (the only level). Threads: the region rounded up to 16.
pub static ATROUS_0: Kernel = Kernel {
    name: "detail_atrous0_v1",
    atomic: false,
    helpers: &[COMMON_WGSL, ATROUS_0_WGSL],
    body: "//workgroup
    let no = pu(p);
    let ao = pu(p + 1u);
    let iw = pu(p + 2u);
    let ih = pu(p + 3u);
    let n = pu(p + 4u);
    let gx = pu(2u) / 16u;
    let x0 = i32((w % gx) * 16u);
    let y0 = i32((w / gx) * 16u);
    for (var k = li; k < 400u; k += 256u) {
        let cx = clamp(x0 - 2 + i32(k % 20u), 0, i32(iw) - 1);
        let cy = clamp(y0 - 2 + i32(k / 20u), 0, i32(ih) - 1);
        let c = src_at(i32(pu(p + 5u)) + cx, i32(pu(p + 6u)) + cy);
        let r = dv_gat(c.x, pf(p + 7u), pf(p + 10u));
        let g = dv_gat(c.y, pf(p + 8u), pf(p + 11u));
        let b = dv_gat(c.z, pf(p + 9u), pf(p + 12u));
        A0_T[k] = vec3<f32>((r + g + b) * 0.5773502691896258, (r - b) * 0.7071067811865476,
            (r - 2.0 * g + b) * 0.4082482904638631);
    }
    workgroupBarrier();
    let lx = i32(li % 16u);
    let ly = i32(li / 16u);
    if (x0 + lx >= i32(iw) || y0 + ly >= i32(ih)) { return; }
    let i = u32(y0 + ly) * iw + u32(x0 + lx);
    let o = (ly + 2) * 20 + lx + 2;
    let c = A0_T[o];
    let ir = pf(p + 16u);
    var acc = vec3<f32>(0.0);
    var ws = 0.0;
    // 2, from a word the compiler cannot see (the width is never 0): loops, as ATROUS's.
    let r2 = i32(select(2u, 3u, iw == 0u));
    for (var j = -2; j <= r2; j++) {
        let hy = select(select(0.0625, 0.25, abs(j) == 1), 0.375, j == 0);
        let row = o + j * 20;
        for (var k = -2; k <= r2; k++) {
            let hx = select(select(0.0625, 0.25, abs(k) == 1), 0.375, k == 0);
            let q = A0_T[row + k];
            let d = q - c;
            let wt = hx * hy * exp(-dot(d, d) * ir);
            acc += wt * q;
            ws += wt;
        }
    }
    let nx = acc / ws;
    let dt = c - nx;
    let th = vec3<f32>(pf(p + 13u), pf(p + 14u), pf(p + 15u));
    var sh = sign(dt) * max(abs(dt) - th, vec3<f32>(0.0));
    if (pu(p + 17u) != 0u) {
        sh += nx;
    } else {
        S[no + i] = nx.x;
        S[no + n + i] = nx.y;
        S[no + 2u * n + i] = nx.z;
    }
    S[ao + i] = sh.x;
    S[ao + n + i] = sh.y;
    S[ao + 2u * n + i] = sh.z;",
};

/// Words: source plane, destination plane, width, height, search radius, patch radius,
/// 1/h² (f32). Non-local means (Buades, Coll and Morel 2005) on one plane: each pixel the
/// weighted mean of the pixels in its search window, weighted by `exp(−d²/h²)` of the mean
/// squared difference `d²` of their patches.
pub static NLM: Kernel = Kernel {
    name: "detail_nlm_v1",
    atomic: false,
    helpers: &[COMMON_WGSL],
    body: "    let so = pu(p);
    let w = pu(p + 2u);
    let h = pu(p + 3u);
    let sr = i32(pu(p + 4u));
    let pr = i32(pu(p + 5u));
    let ih = pf(p + 6u);
    let x = i32(t.x);
    let y = i32(t.y);
    let np = f32((2 * pr + 1) * (2 * pr + 1));
    var acc = 0.0;
    var ws = 0.0;
    for (var dy = -sr; dy <= sr; dy++) {
        for (var dx = -sr; dx <= sr; dx++) {
            var d2 = 0.0;
            for (var py = -pr; py <= pr; py++) {
                for (var px = -pr; px <= pr; px++) {
                    let e = dv_pt(so, w, h, x + px, y + py)
                        - dv_pt(so, w, h, x + dx + px, y + dy + py);
                    d2 += e * e;
                }
            }
            let wt = exp(-(d2 / np) * ih);
            acc += wt * dv_pt(so, w, h, x + dx, y + dy);
            ws += wt;
        }
    }
    S[pu(p + 1u) + t.y * w + t.x] = acc / ws;",
};

/// Largest search plus patch radius [`NLM_WG`] holds in workgroup memory.
pub const NLM_WG_REACH: u32 = 5;
/// The patch radius [`NLM_WG`] takes (3×3 patches only).
pub const NLM_WG_PATCH: u32 = 1;

/// Workgroup memory of [`NLM_WG`]: the 16×16 tile with [`NLM_WG_REACH`] around it.
const NLM_WG_WGSL: &str = "const NLM_V: u32 = 26u;
var<workgroup> NLM_T: array<f32, 676>;
";

/// [`NLM`] per workgroup of 16×16 pixels with 3×3 patches: the tile and its surroundings
/// are read once into workgroup memory. Each pixel keeps its own patch in
/// registers and slides a 3×3 window along each row of candidates, reading one new
/// column (three values) per candidate instead of nine. Every difference, square,
/// sum, weight and mean is [`NLM`]'s, in the same order, so the result is [`NLM`]'s bit for
/// bit. Same words; threads rounded up to 16; search plus patch radius at most
/// [`NLM_WG_REACH`], patch radius exactly [`NLM_WG_PATCH`].
pub static NLM_WG: Kernel = Kernel {
    name: "detail_nlm_wg_v2",
    atomic: false,
    helpers: &[COMMON_WGSL, NLM_WG_WGSL],
    body: "//workgroup
    let so = pu(p);
    let iw = pu(p + 2u);
    let ih = pu(p + 3u);
    let sr = i32(pu(p + 4u));
    let pr = i32(pu(p + 5u));
    let hh = pf(p + 6u);
    let tx = pu(2u) / 16u;
    let x0 = i32((w % tx) * 16u);
    let y0 = i32((w / tx) * 16u);
    let c = i32(li % 16u);
    let r = i32(li / 16u);
    for (var k = li; k < NLM_V * NLM_V; k += 256u) {
        NLM_T[k] = dv_pt(so, iw, ih, x0 - 5 + i32(k % NLM_V), y0 - 5 + i32(k / NLM_V));
    }
    workgroupBarrier();
    let np = f32((2 * pr + 1) * (2 * pr + 1));
    // The pixel's own patch, rows a (y − 1), b (y), c (y + 1).
    let v = i32(NLM_V);
    let o = (r + 5) * v + c + 5;
    let a0 = NLM_T[o - v - 1];
    let a1 = NLM_T[o - v];
    let a2 = NLM_T[o - v + 1];
    let b0 = NLM_T[o - 1];
    let b1 = NLM_T[o];
    let b2 = NLM_T[o + 1];
    let c0 = NLM_T[o + v - 1];
    let c1 = NLM_T[o + v];
    let c2 = NLM_T[o + v + 1];
    var acc = 0.0;
    var ws = 0.0;
    for (var dy = -sr; dy <= sr; dy++) {
        // The candidate's left and middle columns at dx = −sr.
        let q0 = o + dy * v - sr;
        var l0 = NLM_T[q0 - v - 1];
        var l1 = NLM_T[q0 - 1];
        var l2 = NLM_T[q0 + v - 1];
        var m0 = NLM_T[q0 - v];
        var m1 = NLM_T[q0];
        var m2 = NLM_T[q0 + v];
        for (var dx = -sr; dx <= sr; dx++) {
            let q = q0 + dx + sr + 1;
            let r0 = NLM_T[q - v];
            let r1 = NLM_T[q];
            let r2 = NLM_T[q + v];
            // Summed by a loop as detail_nlm_v1 does: written out as nine statements, the
            // driver may reorder the sum (RADV did) and the bits move.
            var dd = array<f32, 9>(a0 - l0, a1 - m0, a2 - r0, b0 - l1, b1 - m1, b2 - r1,
                c0 - l2, c1 - m2, c2 - r2);
            var d2 = 0.0;
            for (var i = 0; i < 9; i++) {
                let e = dd[i];
                d2 += e * e;
            }
            let wt = exp(-(d2 / np) * hh);
            acc += wt * m1;
            ws += wt;
            l0 = m0;
            l1 = m1;
            l2 = m2;
            m0 = r0;
            m1 = r1;
            m2 = r2;
        }
    }
    let x = u32(x0 + c);
    let y = u32(y0 + r);
    if (x < iw && y < ih) {
        S[pu(p + 1u) + y * iw + x] = acc / ws;
    }",
};

/// Words: destination plane, second destination (or the first again), width, region x,
/// region y, floor (f32). Rec.2020 luminance of the input, at least the floor, into both.
pub static LUM: Kernel = Kernel {
    name: "detail_lum_v1",
    atomic: false,
    helpers: &[COMMON_WGSL],
    body: "    let c = src_at(i32(pu(p + 3u) + t.x), i32(pu(p + 4u) + t.y));
    let v = max(dot(c.rgb, DV_DLUMA), pf(p + 5u));
    let i = t.y * pu(p + 2u) + t.x;
    S[pu(p) + i] = v;
    S[pu(p + 1u) + i] = v;",
};

/// Words: source, destination, width, height, vertical (0/1), radius, mode, then for
/// mode 1 the observed plane, for mode 2 the estimate, minimum and maximum planes and the
/// margin (f32); damping (f32) at word 12; the taps (f32, centre first) from word 13.
///
/// A 1-D Gaussian convolution (taps normalised by the caller). Mode 0 writes it. Mode 1
/// (Richardson–Lucy, first half) writes `observed / blurred`, held within
/// [1/(1+damping), 1+damping]. Mode 2 (second half) multiplies the estimate by it and
/// holds the estimate within the local range of the observed image, widened by the
/// margin, so the deconvolution cannot ring past it.
pub static BLUR: Kernel = Kernel {
    name: "detail_blur_v1",
    atomic: false,
    helpers: &[COMMON_WGSL],
    body: "    let so = pu(p);
    let w = pu(p + 2u);
    let h = pu(p + 3u);
    let vert = pu(p + 4u) != 0u;
    let r = i32(pu(p + 5u));
    let mode = pu(p + 6u);
    let x = i32(t.x);
    let y = i32(t.y);
    var acc = pf(p + 13u) * dv_pt(so, w, h, x, y);
    for (var k = 1; k <= r; k++) {
        let wt = pf(p + 13u + u32(k));
        if (vert) {
            acc += wt * (dv_pt(so, w, h, x, y - k) + dv_pt(so, w, h, x, y + k));
        } else {
            acc += wt * (dv_pt(so, w, h, x - k, y) + dv_pt(so, w, h, x + k, y));
        }
    }
    let i = t.y * w + t.x;
    let dO = pu(p + 1u) + i;
    if (mode == 0u) {
        S[dO] = acc;
        return;
    }
    let damp = pf(p + 12u);
    if (mode == 1u) {
        let q = S[pu(p + 7u) + i] / max(acc, 1e-12);
        S[dO] = clamp(q, 1.0 / (1.0 + damp), 1.0 + damp);
        return;
    }
    let lo = S[pu(p + 9u) + i];
    let hi = S[pu(p + 10u) + i];
    let m = pf(p + 11u) * (hi - lo);
    S[dO] = clamp(S[pu(p + 8u) + i] * acc, lo - m, hi + m);",
};

/// Words: source, minimum destination, maximum destination, width, height, vertical (0/1),
/// radius, second source (the maximum plane when vertical). Running minimum and maximum
/// over `[−radius, radius]` along a row or a column.
pub static MINMAX: Kernel = Kernel {
    name: "detail_minmax_v1",
    atomic: false,
    helpers: &[COMMON_WGSL],
    body: "    let so = pu(p);
    let w = pu(p + 3u);
    let h = pu(p + 4u);
    let vert = pu(p + 5u) != 0u;
    let r = i32(pu(p + 6u));
    let s2 = pu(p + 7u);
    let x = i32(t.x);
    let y = i32(t.y);
    var lo = 3.0e38;
    var hi = -3.0e38;
    for (var k = -r; k <= r; k++) {
        if (vert) {
            lo = min(lo, dv_pt(so, w, h, x, y + k));
            hi = max(hi, dv_pt(s2, w, h, x, y + k));
        } else {
            let v = dv_pt(so, w, h, x + k, y);
            lo = min(lo, v);
            hi = max(hi, v);
        }
    }
    let i = t.y * w + t.x;
    S[pu(p + 1u) + i] = lo;
    S[pu(p + 2u) + i] = hi;",
};

/// The B3-spline taps of the à-trous transform.
pub const B3: [f64; 5] = [0.0625, 0.25, 0.375, 0.25, 0.0625];

/// Noise of the à-trous transform of unit white noise (linear filter, no edge weights):
/// for levels `0..levels`, the standard deviation of the detail band and of the smooth
/// image after it. Exact, from the filters' coefficients (the 2-D filters are separable,
/// so their norms are products of 1-D norms).
pub fn band_noise(levels: u32) -> Vec<(f64, f64)> {
    // g[j]: the 1-D filter giving the smooth image after j levels (g[0] = δ).
    let mut g: Vec<Vec<f64>> = vec![vec![1.0]];
    for j in 0..levels {
        let prev = &g[j as usize];
        let step = 1usize << j;
        let len = prev.len() + 4 * step;
        let mut next = vec![0.0; len];
        for (i, &v) in prev.iter().enumerate() {
            for (k, &b) in B3.iter().enumerate() {
                next[i + k * step] += v * b;
            }
        }
        g.push(next);
    }
    // Inner product of two centred filters.
    let dot = |a: &[f64], b: &[f64]| {
        let (ca, cb) = (a.len() / 2, b.len() / 2);
        let mut s = 0.0;
        for (i, &v) in a.iter().enumerate() {
            let k = i as isize - ca as isize + cb as isize;
            if k >= 0 && (k as usize) < b.len() {
                s += v * b[k as usize];
            }
        }
        s
    };
    (0..levels as usize)
        .map(|j| {
            let (a, b) = (&g[j], &g[j + 1]);
            let aa = dot(a, a).powi(2);
            let bb = dot(b, b).powi(2);
            let ab = dot(a, b).powi(2);
            ((aa + bb - 2.0 * ab).max(0.0).sqrt(), bb.sqrt())
        })
        .collect()
}

/// Normalised 1-D Gaussian taps (centre first) of standard deviation `sigma` out to
/// `radius`.
pub fn gauss_taps(sigma: f32, radius: u32) -> Vec<f32> {
    let s = f64::from(sigma.max(1e-3));
    let w: Vec<f64> = (0..=radius)
        .map(|k| (-(f64::from(k) * f64::from(k)) / (2.0 * s * s)).exp())
        .collect();
    let total = w[0] + 2.0 * w[1..].iter().sum::<f64>();
    w.iter().map(|v| (v / total) as f32).collect()
}

/// The f32 bits of a word.
pub fn fw(v: f32) -> u32 {
    v.to_bits()
}
