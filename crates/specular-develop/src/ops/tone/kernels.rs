//! The program kernels the tone ops share: luminance planes, box means and guided-filter
//! coefficients, Gaussian pyramids with the remapping of the fast local Laplacian filter,
//! histograms and percentiles.
//!
//! Every plane is a row-major f32 array in the scratch `S` at a word offset; offsets, sizes
//! and constants come in each step's words, so one kernel serves every size and op. Sampling
//! clamps to the plane's edge. [`super::cpu`] runs the same kernels in Rust for the tests.

use crate::graph::{Kernel, Program};

/// 2⁻¹⁶: the floor of a luminance before its log (−16 EV).
pub const LUM_FLOOR: f32 = 1.525_878_9e-5;
/// "No plane": a pyramid's level 0, remapped on the fly.
pub const NONE: u32 = u32::MAX;
/// Bins of a histogram.
pub const BINS: u32 = 1024;

/// Rec.2020 luminance, a 5-tap binomial and the remapping function.
pub const COMMON_WGSL: &str = "const DV_LUMA = vec3<f32>(0.2627, 0.6780, 0.0593);
fn dv_bw(a: i32) -> f32 {
    if (a == 0) { return 0.375; }
    if (a == -1 || a == 1) { return 0.25; }
    return 0.0625;
}
fn dv_remap(v: f32, g: f32, amt: f32, sig: f32) -> f32 {
    let d = v - g;
    let u = d / (2.0 * sig);
    if (abs(u) >= 1.0) { return v; }
    let b = 1.0 - u * u;
    return v + amt * d * b * b;
}
";

/// Plane reads from the scratch.
const PLANE_WGSL: &str = "fn dv_tap(o: u32, w: u32, h: u32, x: i32, y: i32) -> f32 {
    let cx = u32(clamp(x, 0, i32(w) - 1));
    let cy = u32(clamp(y, 0, i32(h) - 1));
    return S[o + cy * w + cx];
}
fn dv_ew(odd: bool, i: u32) -> f32 {
    if (odd) {
        if (i == 0u) { return 0.0; }
        return 0.5;
    }
    if (i == 1u) { return 0.75; }
    return 0.125;
}
fn dv_up(o: u32, w1: u32, h1: u32, x: u32, y: u32) -> f32 {
    let mx = i32(x >> 1u) - 1;
    let my = i32(y >> 1u) - 1;
    let ox = (x & 1u) == 1u;
    let oy = (y & 1u) == 1u;
    var acc = 0.0;
    for (var b = 0u; b < 3u; b++) {
        var row = 0.0;
        for (var a = 0u; a < 3u; a++) {
            row += dv_ew(ox, a) * dv_tap(o, w1, h1, mx + i32(a), my + i32(b));
        }
        acc += dv_ew(oy, b) * row;
    }
    return acc;
}
";

/// `S[w0 + i] = word 1 + i` (as f32): small constants into the scratch.
pub static PUT: Kernel = Kernel {
    name: "tone_put_v1",
    atomic: false,
    helpers: &[],
    body: "    S[pu(p) + t.x] = pf(p + 1u + t.x);",
};

/// `S[w1 + i] = st(w0 + i)`: statistics into a local program's scratch.
pub static COPY_ST: Kernel = Kernel {
    name: "tone_copy_st_v1",
    atomic: false,
    helpers: &[],
    body: "    S[pu(p + 1u) + t.x] = st(pu(p) + t.x);",
};

/// Words: plane, width, origin x, origin y. `log2` of the luminance of the input at
/// (origin + t), floored at [`LUM_FLOOR`].
pub static LUM: Kernel = Kernel {
    name: "tone_lum_v1",
    atomic: false,
    helpers: &[COMMON_WGSL],
    body: "    let c = src_at(i32(pu(p + 2u) + t.x), i32(pu(p + 3u) + t.y));
    S[pu(p) + t.y * pu(p + 1u) + t.x] = log2(max(dot(c.rgb, DV_LUMA), 1.52587890625e-5));",
};

/// Words: source, destination, width, height, radius, planes, plane size, mode, second
/// plane. Horizontal means over `[x − r, x + r]` (clipped). Mode 0: each of the planes.
/// Mode 1: the guided-filter moments of guide I (source) and input P (second plane): the
/// planes I, P, I·P, I·I. Mode 2: the same with P = second plane − I.
pub static BOX_H: Kernel = Kernel {
    name: "tone_box_h_v1",
    atomic: false,
    helpers: &[],
    body: "    let so = pu(p);
    let dO = pu(p + 1u);
    let w = pu(p + 2u);
    let r = i32(pu(p + 4u));
    let planes = pu(p + 5u);
    let ps = pu(p + 6u);
    let mode = pu(p + 7u);
    let po = pu(p + 8u);
    let lo = max(i32(t.x) - r, 0);
    let hi = min(i32(t.x) + r, i32(w) - 1);
    let n = f32(hi - lo + 1);
    let row = t.y * w;
    let at = row + t.x;
    if (mode == 0u) {
        for (var c = 0u; c < planes; c++) {
            var s = 0.0;
            for (var i = lo; i <= hi; i++) { s += S[so + c * ps + row + u32(i)]; }
            S[dO + c * ps + at] = s / n;
        }
        return;
    }
    var si = 0.0;
    var sp = 0.0;
    var sip = 0.0;
    var sii = 0.0;
    for (var i = lo; i <= hi; i++) {
        let q = row + u32(i);
        let gi = S[so + q];
        var pv = S[po + q];
        if (mode == 2u) { pv = pv - gi; }
        si += gi;
        sp += pv;
        sip += gi * pv;
        sii += gi * gi;
    }
    S[dO + at] = si / n;
    S[dO + ps + at] = sp / n;
    S[dO + 2u * ps + at] = sip / n;
    S[dO + 3u * ps + at] = sii / n;",
};

/// Words: source, destination, width, height, radius, planes, plane size, mode, ε (f32).
/// Vertical means over `[y − r, y + r]`. Mode 0: each of the planes. Mode 1: from the four
/// moment planes, the guided filter's `a = cov(I, P) / (var(I) + ε)` and `b = P̄ − a·Ī`.
pub static BOX_V: Kernel = Kernel {
    name: "tone_box_v_v1",
    atomic: false,
    helpers: &[],
    body: "    let so = pu(p);
    let dO = pu(p + 1u);
    let w = pu(p + 2u);
    let h = pu(p + 3u);
    let r = i32(pu(p + 4u));
    let planes = pu(p + 5u);
    let ps = pu(p + 6u);
    let mode = pu(p + 7u);
    let lo = max(i32(t.y) - r, 0);
    let hi = min(i32(t.y) + r, i32(h) - 1);
    let n = f32(hi - lo + 1);
    let at = t.y * w + t.x;
    if (mode == 0u) {
        for (var c = 0u; c < planes; c++) {
            var s = 0.0;
            for (var i = lo; i <= hi; i++) { s += S[so + c * ps + u32(i) * w + t.x]; }
            S[dO + c * ps + at] = s / n;
        }
        return;
    }
    var m = vec4<f32>(0.0);
    for (var i = lo; i <= hi; i++) {
        let q = u32(i) * w + t.x;
        m += vec4<f32>(S[so + q], S[so + ps + q], S[so + 2u * ps + q], S[so + 3u * ps + q]);
    }
    m = m / n;
    let var_i = max(m.w - m.x * m.x, 0.0);
    let a = (m.z - m.x * m.y) / (var_i + pf(p + 8u));
    S[dO + at] = a;
    S[dO + ps + at] = m.y - a * m.x;",
};

/// Words: source, width, height, source plane stride, destination, width, height,
/// destination plane stride, planes, mode, then four plane offsets, range and σ (f32). The
/// next Gaussian pyramid level of each plane (5×5 binomial, then every second sample).
///
/// - Mode 1 (the luminance pyramid) also takes the minimum and maximum of the level-0
///   values under each sample: planes min and max of the source level (offsets 1–2; the
///   luminance itself at level 0) into those of the destination (offsets 3–4).
/// - Mode 2 (the remapped pyramids) skips every γ_j more than 2σ from all level-0 values
///   under the sample (offsets 2–3 the destination's min and max): the remapping is the
///   identity there, so the level is the luminance pyramid's (offset 1). The result is the
///   same; most samples of most pixels do no work.
pub static REDUCE: Kernel = Kernel {
    name: "tone_reduce_v1",
    atomic: false,
    helpers: &[COMMON_WGSL, PLANE_WGSL],
    body: "    let so = pu(p);
    let ws = pu(p + 1u);
    let hs = pu(p + 2u);
    let ss = pu(p + 3u);
    let dO = pu(p + 4u);
    let wd = pu(p + 5u);
    let ds = pu(p + 7u);
    let planes = pu(p + 8u);
    let mode = pu(p + 9u);
    let x = i32(t.x) * 2;
    let y = i32(t.y) * 2;
    let at = t.y * wd + t.x;
    if (mode == 1u) {
        var mn = 3.0e38;
        var mx = -3.0e38;
        for (var b = -2; b <= 2; b++) {
            for (var a = -2; a <= 2; a++) {
                mn = min(mn, dv_tap(pu(p + 10u), ws, hs, x + a, y + b));
                mx = max(mx, dv_tap(pu(p + 11u), ws, hs, x + a, y + b));
            }
        }
        S[pu(p + 12u) + at] = mn;
        S[pu(p + 13u) + at] = mx;
    }
    var lo = 0.0;
    var hi = 0.0;
    var g0 = 0.0;
    var dg = 0.0;
    var reach = 0.0;
    if (mode == 2u) {
        lo = S[pu(p + 11u) + at];
        hi = S[pu(p + 12u) + at];
        g0 = S[pu(p + 14u)];
        dg = S[pu(p + 14u) + 1u];
        reach = 2.0 * pf(p + 15u);
    }
    for (var j = 0u; j < planes; j++) {
        if (mode == 2u) {
            let g = g0 + f32(j) * dg;
            if (g - reach >= hi || g + reach <= lo) {
                S[dO + j * ds + at] = S[pu(p + 10u) + at];
                continue;
            }
        }
        var acc = 0.0;
        for (var b = -2; b <= 2; b++) {
            var row = 0.0;
            for (var a = -2; a <= 2; a++) {
                row += dv_bw(a) * dv_tap(so + j * ss, ws, hs, x + a, y + b);
            }
            acc += dv_bw(b) * row;
        }
        S[dO + j * ds + at] = acc;
    }",
};

/// The body of [`REDUCE_L1`] and [`REDUCE_L1_LUM`] around the code that fills the 5×5
/// level-0 taps `v` (two kernels: no branch in the taps' loop, which stay in registers).
macro_rules! l1_body {
    ($fill:literal) => {
        concat!(
            "    let ws = pu(p + 1u);
    let hs = pu(p + 2u);
    let wd = pu(p + 9u);
    let ds = pu(p + 10u);
    let k = pu(p + 11u);
    let rng = pu(p + 12u);
    let amt = pf(p + 13u);
    let sig = pf(p + 14u);
    let x = i32(t.x) * 2;
    let y = i32(t.y) * 2;
    var v: array<f32, 25>;
    for (var b = 0; b < 5; b++) {
        for (var a = 0; a < 5; a++) {
            let cx = u32(clamp(x + a - 2, 0, i32(ws) - 1));
            let cy = u32(clamp(y + b - 2, 0, i32(hs) - 1));
",
            $fill,
            "
        }
    }
    let at = t.y * wd + t.x;
    var mn = 3.0e38;
    var mx = -3.0e38;
    var plain = 0.0;
    for (var b = 0; b < 5; b++) {
        var row = 0.0;
        for (var a = 0; a < 5; a++) {
            let q = v[b * 5 + a];
            mn = min(mn, q);
            mx = max(mx, q);
            row += dv_bw(a - 2) * q;
        }
        plain += dv_bw(b - 2) * row;
    }
    S[pu(p + 5u) + at] = plain;
    S[pu(p + 6u) + at] = mn;
    S[pu(p + 7u) + at] = mx;
    let g0 = S[rng];
    let dg = S[rng + 1u];
    let r1 = pu(p + 8u);
    for (var j = 0u; j < k; j++) {
        let g = g0 + f32(j) * dg;
        var acc = plain;
        if (g - 2.0 * sig < mx && g + 2.0 * sig > mn) {
            acc = 0.0;
            for (var b = 0; b < 5; b++) {
                var row = 0.0;
                for (var a = 0; a < 5; a++) {
                    row += dv_bw(a - 2) * dv_remap(v[b * 5 + a], g, amt, sig);
                }
                acc += dv_bw(b - 2) * row;
            }
        }
        S[r1 + j * ds + at] = acc;
    }"
        )
    };
}

/// Level 1 of the luminance pyramid and of the K remapped pyramids, from one read of the
/// 5×5 level-0 taps (before, [`REDUCE`] mode 1 and a remap step, each with the same
/// arithmetic in the same order, so the planes are the same bit for bit). Words: level-0
/// plane, width, height, origin x, origin y (unused here), G₁, min, max (of the level-0
/// values under each sample, as [`REDUCE`] mode 1), R₁, its width, plane stride, K, range
/// (S offset of γ₀ and Δγ), amount (f32), σ (f32). R₁ holds `r_j(L)` with
/// `γ_j = γ₀ + j·Δγ` (Aubry et al. 2014, fast local Laplacian filter); a γ_j more than 2σ
/// from every tap leaves the taps as they are: their plain sum, G₁.
pub static REDUCE_L1: Kernel = Kernel {
    name: "tone_reduce_l1_v1",
    atomic: false,
    helpers: &[COMMON_WGSL],
    body: l1_body!("            v[b * 5 + a] = S[pu(p) + cy * ws + cx];"),
};

/// [`REDUCE_L1`] with level 0 the `log2` luminance of the input at origin + tap, as
/// [`LUM`] makes it (no level-0 plane, one dispatch fewer). Word 0 is unused.
pub static REDUCE_L1_LUM: Kernel = Kernel {
    name: "tone_reduce_l1_lum_v1",
    atomic: false,
    helpers: &[COMMON_WGSL],
    body: l1_body!(
        "            let c = src_at(i32(pu(p + 3u) + cx), i32(pu(p + 4u) + cy));
            v[b * 5 + a] = log2(max(dot(c.rgb, DV_LUMA), 1.52587890625e-5));"
    ),
};

/// Words: G_l, width, height, G_l+1, width, height, R_l (or [`NONE`]: remap G_l on the
/// fly), its plane stride, R_l+1, its plane stride, K, range, amount (f32), σ (f32),
/// out_l+1, out_l, manipulate. One level of the collapse:
/// `out_l = lap_l + expand(out_l+1)`, where `lap_l` is the Laplacian of the remapped image
/// whose γ brackets `G_l(p)`, interpolated (or of `G` itself when not manipulated).
pub static COLLAPSE: Kernel = Kernel {
    name: "tone_collapse_v1",
    atomic: false,
    helpers: &[COMMON_WGSL, PLANE_WGSL],
    body: "    let gl = pu(p);
    let wl = pu(p + 1u);
    let gl1 = pu(p + 3u);
    let wl1 = pu(p + 4u);
    let hl1 = pu(p + 5u);
    let rl = pu(p + 6u);
    let rls = pu(p + 7u);
    let rl1 = pu(p + 8u);
    let rl1s = pu(p + 9u);
    let k = pu(p + 10u);
    let rng = pu(p + 11u);
    let amt = pf(p + 12u);
    let sig = pf(p + 13u);
    let ol1 = pu(p + 14u);
    let ol = pu(p + 15u);
    let idx = t.y * wl + t.x;
    let g = S[gl + idx];
    var lap = g - dv_up(gl1, wl1, hl1, t.x, t.y);
    if (pu(p + 16u) != 0u) {
        let g0 = S[rng];
        let dg = S[rng + 1u];
        let jf = clamp((g - g0) / dg, 0.0, f32(k - 1u));
        let j0 = min(u32(floor(jf)), k - 2u);
        let f = jf - f32(j0);
        var l2 = vec2<f32>(0.0);
        for (var i = 0u; i < 2u; i++) {
            let j = j0 + i;
            var gr: f32;
            if (rl == 0xffffffffu) {
                gr = dv_remap(g, g0 + f32(j) * dg, amt, sig);
            } else {
                gr = S[rl + j * rls + idx];
            }
            l2[i] = gr - dv_up(rl1 + j * rl1s, wl1, hl1, t.x, t.y);
        }
        lap = mix(l2.x, l2.y, f);
    }
    S[ol + idx] = lap + dv_up(ol1, wl1, hl1, t.x, t.y);",
};

/// Words: source, width, height, destination, destination width, factor s. The mean of
/// each s×s block (clipped at the edges): the coarse guide of the fast guided filter.
pub static DOWN: Kernel = Kernel {
    name: "tone_down_v1",
    atomic: false,
    helpers: &[],
    body: "    let w = pu(p + 1u);
    let h = pu(p + 2u);
    let f = pu(p + 5u);
    let x1 = min(t.x * f + f, w);
    let y1 = min(t.y * f + f, h);
    var s = 0.0;
    for (var y = t.y * f; y < y1; y++) {
        for (var x = t.x * f; x < x1; x++) { s += S[pu(p) + y * w + x]; }
    }
    S[pu(p + 3u) + t.y * pu(p + 4u) + t.x] = s / f32((x1 - t.x * f) * (y1 - t.y * f));",
};

/// Words: histogram. Clears [`BINS`] counts.
pub static HIST_CLEAR: Kernel = Kernel {
    name: "tone_hist_clear_v1",
    atomic: false,
    helpers: &[],
    body: "    S[pu(p) + t.x] = 0.0;",
};

/// Words: mode, plane 1, plane 2, plane 3, width, histogram, low (f32), high (f32),
/// black (f32), height. Counts one value per pixel into [`BINS`] bins over [low, high):
/// mode 0 plane 1; mode 1 `log2` of the input's luminance after subtracting black; mode 2
/// the guided mask plane 1 × plane 3 + plane 2. Per workgroup of 16×16 pixels:
/// counted in workgroup memory, then each bin that counted any added once; the counts are
/// the same. Threads: the size rounded up to 16 ([`hist_add`]).
pub static HIST_ADD: Kernel = Kernel {
    name: "tone_hist_add_v2",
    atomic: true,
    helpers: &[
        COMMON_WGSL,
        "var<workgroup> HA_T: array<atomic<u32>, 1024>;\n",
    ],
    body: "//workgroup
    for (var b = li; b < 1024u; b += 256u) { atomicStore(&HA_T[b], 0u); }
    workgroupBarrier();
    let mode = pu(p);
    let wd = pu(p + 4u);
    let tgx = pu(2u) / 16u;
    let tx = (w % tgx) * 16u + li % 16u;
    let ty = (w / tgx) * 16u + li / 16u;
    if (tx < wd && ty < pu(p + 9u)) {
        let at = ty * wd + tx;
        var v: f32;
        if (mode == 0u) {
            v = bitcast<f32>(atomicLoad(&S[pu(p + 1u) + at]));
        } else if (mode == 1u) {
            let c = src_at(i32(tx), i32(ty));
            let y = dot(c.rgb - vec3<f32>(pf(p + 8u)), DV_LUMA);
            v = log2(max(y, 1.52587890625e-5));
        } else {
            let a = bitcast<f32>(atomicLoad(&S[pu(p + 1u) + at]));
            let b = bitcast<f32>(atomicLoad(&S[pu(p + 2u) + at]));
            let g = bitcast<f32>(atomicLoad(&S[pu(p + 3u) + at]));
            v = a * g + b;
        }
        let lo = pf(p + 6u);
        let hi = pf(p + 7u);
        let bin = u32(clamp(floor((v - lo) * 1024.0 / (hi - lo)), 0.0, 1023.0));
        atomicAdd(&HA_T[bin], 1u);
    }
    workgroupBarrier();
    let hist = pu(p + 5u);
    for (var b = li; b < 1024u; b += 256u) {
        let c = atomicLoad(&HA_T[b]);
        if (c > 0u) { atomicAdd(&S[hist + b], c); }
    }",
};

/// [`HIST_ADD`] over a `w`×`h` plane or input; `words` are its first nine.
pub fn hist_add(prog: &mut Program, on: bool, (w, h): (u32, u32), words: [u32; 9]) {
    let mut v = words.to_vec();
    v.push(h);
    prog.step(
        &HIST_ADD,
        threads(on, w.next_multiple_of(16), h.next_multiple_of(16)),
        v,
    );
}

/// Words: histogram, low (f32), high (f32), output, count, then up to four quantiles (f32).
/// Each quantile's value, linear within its bin. One workgroup: the bins' running
/// sums by a parallel scan of the integer counts, then the first bin that reaches each
/// quantile by an atomic minimum. Below 2²⁴ in all, every running sum is an integer an f32
/// holds exactly, so the values are those of one thread adding the bins one by one in f32
/// (which the kernel still does over that). Threads: 16×16 ([`hist_pct`]).
pub static HIST_PCT: Kernel = Kernel {
    name: "tone_hist_pct_v2",
    atomic: false,
    helpers: &["var<workgroup> HP_C: array<u32, 1024>;
var<workgroup> HP_S: array<u32, 256>;
var<workgroup> HP_B: atomic<u32>;
"],
    body: "//workgroup
    let hist = pu(p);
    let lo = pf(p + 1u);
    let hi = pf(p + 2u);
    // Each thread's four bins, their sum, then an inclusive scan of the 256 sums.
    var own = 0u;
    for (var i = 0u; i < 4u; i++) {
        let c = bitcast<u32>(S[hist + li * 4u + i]);
        HP_C[li * 4u + i] = c;
        own += c;
    }
    HP_S[li] = own;
    workgroupBarrier();
    for (var d = 1u; d < 256u; d *= 2u) {
        var v = HP_S[li];
        if (li >= d) { v += HP_S[li - d]; }
        workgroupBarrier();
        HP_S[li] = v;
        workgroupBarrier();
    }
    let all = workgroupUniformLoad(&HP_S[255]);
    if (all >= 16777216u) {
        // Sums an f32 may round: one thread, as the sums were always made.
        if (li == 0u) {
            var total = 0.0;
            for (var b = 0u; b < 1024u; b++) { total += f32(HP_C[b]); }
            for (var q = 0u; q < pu(p + 4u); q++) {
                let goal = pf(p + 5u + q) * total;
                var cum = 0.0;
                var res = hi;
                for (var b = 0u; b < 1024u; b++) {
                    let c = f32(HP_C[b]);
                    if (c > 0.0 && cum + c >= goal) {
                        res = lo + (f32(b) + (goal - cum) / c) * (hi - lo) / 1024.0;
                        break;
                    }
                    cum += c;
                }
                S[pu(p + 3u) + q] = res;
            }
        }
        return;
    }
    let total = f32(all);
    // Running sums before each of this thread's bins.
    var before: array<u32, 4>;
    var run = HP_S[li] - own;
    for (var i = 0u; i < 4u; i++) {
        before[i] = run;
        run += HP_C[li * 4u + i];
    }
    for (var q = 0u; q < pu(p + 4u); q++) {
        let goal = pf(p + 5u + q) * total;
        if (li == 0u) { atomicStore(&HP_B, 1024u); }
        workgroupBarrier();
        for (var i = 0u; i < 4u; i++) {
            let c = f32(HP_C[li * 4u + i]);
            if (c > 0.0 && f32(before[i]) + c >= goal) {
                atomicMin(&HP_B, li * 4u + i);
                break;
            }
        }
        workgroupBarrier();
        let b = atomicLoad(&HP_B);
        if (li == 0u) {
            var res = hi;
            if (b < 1024u) {
                let c = f32(HP_C[b]);
                // The running sum before bin b: thread b/4's, then its earlier bins.
                var cum = HP_S[b / 4u] - (HP_C[b & ~3u] + HP_C[(b & ~3u) + 1u]
                    + HP_C[(b & ~3u) + 2u] + HP_C[(b & ~3u) + 3u]);
                for (var j = b & ~3u; j < b; j++) { cum += HP_C[j]; }
                res = lo + (f32(b) + (goal - f32(cum)) / c) * (hi - lo) / 1024.0;
            }
            S[pu(p + 3u) + q] = res;
        }
        workgroupBarrier();
    }",
};

/// [`HIST_PCT`] with `words`.
pub fn hist_pct(prog: &mut Program, on: bool, words: Vec<u32>) {
    prog.step(&HIST_PCT, threads(on, 16, 16), words);
}

/// Words: percentiles (low, high), range, σ (f32), K. The γ samples of the local
/// Laplacian: from σ under the low percentile to σ over the high one.
pub static RANGE: Kernel = Kernel {
    name: "tone_range_v1",
    atomic: false,
    helpers: &[],
    body: "    let lo = S[pu(p)] - pf(p + 2u);
    let hi = S[pu(p) + 1u] + pf(p + 2u);
    S[pu(p + 1u)] = lo;
    S[pu(p + 1u) + 1u] = max((hi - lo) / f32(pu(p + 3u) - 1u), 1e-3);",
};

/// f32 bits of a value.
pub fn fw(v: f32) -> u32 {
    v.to_bits()
}

/// Threads `[w, h]`, or none when `on` is false (the step stays, so the structure and
/// kernels never change with a value).
pub fn threads(on: bool, w: u32, h: u32) -> [u32; 2] {
    if on { [w, h] } else { [0, 0] }
}

/// A plane allocator: offsets in f32 words.
#[derive(Clone, Copy, Debug)]
pub struct Alloc(pub u64);

impl Alloc {
    pub fn take(&mut self, words: u64) -> u32 {
        let at = self.0;
        self.0 += words;
        at as u32
    }
}

/// One pyramid level: offset (of plane 0), size, plane stride.
#[derive(Clone, Copy, Debug)]
pub struct Level {
    pub o: u32,
    pub w: u32,
    pub h: u32,
    pub stride: u32,
}

/// Sizes of pyramid levels 0..=n of a `w`×`h` plane.
pub fn level_sizes(w: u32, h: u32, n: u32) -> Vec<(u32, u32)> {
    let mut v = vec![(w, h)];
    for _ in 0..n {
        let (a, b) = *v.last().expect("a level");
        v.push((a.div_ceil(2), b.div_ceil(2)));
    }
    v
}

/// Levels a `w`×`h` plane can have (each at least 4 pixels a side), at most `cap`.
pub fn max_levels(w: u32, h: u32, cap: u32) -> u32 {
    let mut n = 0;
    let (mut a, mut b) = (w, h);
    while n < cap && a.div_ceil(2) >= 4 && b.div_ceil(2) >= 4 {
        a = a.div_ceil(2);
        b = b.div_ceil(2);
        n += 1;
    }
    n
}

/// The fast local Laplacian over plane `l0` (`w`×`h`, log luminance; or [`NONE`]: the
/// `log2` luminance of the input's `w`×`h` region at `origin`, read as level 1 is made,
/// never kept, so only with `out0` `None`), with its range of γ at scratch `rng`: `levels` levels, K samples, remapped with `amount` and `sigma`,
/// levels below `first` left as they are. Collapses into `out0`, or with `None` down to
/// level 1 only (the kernel does level 0 on the fly, see [`LEVEL0_WGSL`]). `on` false
/// keeps the steps without work.
///
/// The first planes taken from `a` are G₁, the K planes of R₁ and out₁ (when there are two
/// levels or more; else out₁ is G₁), in that order.
#[allow(clippy::too_many_arguments)]
pub fn local_laplacian(
    prog: &mut Program,
    a: &mut Alloc,
    l0: u32,
    origin: (u32, u32),
    out0: Option<u32>,
    (w, h): (u32, u32),
    levels: u32,
    k: u32,
    rng: u32,
    amount: f32,
    sigma: f32,
    first: u32,
    on: bool,
) {
    debug_assert!(
        l0 != NONE || out0.is_none(),
        "level 0 is kept for its collapse"
    );
    let sizes = level_sizes(w, h, levels);
    let px = |l: usize| u64::from(sizes[l].0) * u64::from(sizes[l].1);
    let lv = |o: u32, l: usize, stride: u32| Level {
        o,
        w: sizes[l].0,
        h: sizes[l].1,
        stride,
    };
    // Level 1 first: G₁, R₁, out₁.
    let g1 = a.take(px(1));
    let r1 = a.take(px(1) * u64::from(k));
    let out1 = if levels >= 2 { a.take(px(1)) } else { g1 };
    // Gaussian pyramid of L (level 0 is the plane itself).
    let mut g = vec![lv(l0, 0, 0), lv(g1, 1, 0)];
    for l in 2..=levels as usize {
        g.push(lv(a.take(px(l)), l, 0));
    }
    // Remapped pyramids, levels 1..=levels, K planes each.
    let mut r = vec![lv(NONE, 0, 0), lv(r1, 1, px(1) as u32)];
    for l in 2..=levels as usize {
        r.push(lv(a.take(px(l) * u64::from(k)), l, px(l) as u32));
    }
    // Output levels 0..levels−1 (the top is G's).
    let mut out = vec![out0.unwrap_or(NONE), out1];
    for l in 2..levels as usize {
        out.push(a.take(px(l)));
    }
    if levels >= 2 {
        out.push(g[levels as usize].o);
    }
    // Minimum and maximum of the level-0 values under each sample, levels 1..=levels.
    let mut mm = vec![(l0, l0)];
    for l in 1..=levels as usize {
        mm.push((a.take(px(l)), a.take(px(l))));
    }
    // Level 1 of G (with its minimum and maximum) and of the K remapped images in
    // one step, from one read of level 0 (or of the input's luminance).
    let (g1, r1, mm1) = (g[1], r[1], mm[1]);
    prog.step(
        if l0 == NONE {
            &REDUCE_L1_LUM
        } else {
            &REDUCE_L1
        },
        threads(on, r1.w, r1.h),
        vec![
            l0,
            w,
            h,
            origin.0,
            origin.1,
            g1.o,
            mm1.0,
            mm1.1,
            r1.o,
            r1.w,
            r1.stride,
            k,
            rng,
            fw(amount),
            fw(sigma),
        ],
    );
    for l in 1..levels as usize {
        let (s, d) = (g[l], g[l + 1]);
        let (smm, dmm) = (mm[l], mm[l + 1]);
        prog.step(
            &REDUCE,
            threads(on, d.w, d.h),
            vec![
                s.o, s.w, s.h, 0, d.o, d.w, d.h, 0, 1, 1, smm.0, smm.1, dmm.0, dmm.1, 0, 0,
            ],
        );
    }
    for l in 1..levels as usize {
        let (s, d) = (r[l], r[l + 1]);
        let dmm = mm[l + 1];
        prog.step(
            &REDUCE,
            threads(on, d.w, d.h),
            vec![
                s.o,
                s.w,
                s.h,
                s.stride,
                d.o,
                d.w,
                d.h,
                d.stride,
                k,
                2,
                g[l + 1].o,
                dmm.0,
                dmm.1,
                0,
                rng,
                fw(sigma),
            ],
        );
    }
    let lowest = if out0.is_some() { 0 } else { 1 };
    for l in (lowest..levels as usize).rev() {
        let (gl, gl1, rl, rl1) = (g[l], g[l + 1], r[l], r[l + 1]);
        prog.step(
            &COLLAPSE,
            threads(on, gl.w, gl.h),
            vec![
                gl.o,
                gl.w,
                gl.h,
                gl1.o,
                gl1.w,
                gl1.h,
                rl.o,
                rl.stride,
                rl1.o,
                rl1.stride,
                k,
                rng,
                fw(amount),
                fw(sigma),
                out[l + 1],
                out[l],
                u32::from(l as u32 >= first),
            ],
        );
    }
}

/// The guided filter (He, Sun and Tang 2013) of input `p` (or `x − i` with `diff`)
/// guided by plane `i`, `w`×`h`, radius `r`, regulariser `eps`: the mean coefficient
/// planes Ā at `out` and B̄ at `out + w·h`, so the filtered value is `Ā·I + B̄`.
#[allow(clippy::too_many_arguments)]
pub fn guided(
    prog: &mut Program,
    a: &mut Alloc,
    i: u32,
    p: u32,
    diff: bool,
    out: u32,
    (w, h): (u32, u32),
    r: u32,
    eps: f32,
    on: bool,
) {
    let n = u64::from(w) * u64::from(h);
    let m = a.take(4 * n);
    let ab = a.take(2 * n);
    let t2 = a.take(2 * n);
    let ps = n as u32;
    let th = threads(on, w, h);
    let mode = if diff { 2 } else { 1 };
    prog.step(&BOX_H, th, vec![i, m, w, h, r, 4, ps, mode, p]);
    prog.step(&BOX_V, th, vec![m, ab, w, h, r, 4, ps, 1, fw(eps)]);
    prog.step(&BOX_H, th, vec![ab, t2, w, h, r, 2, ps, 0, 0]);
    prog.step(&BOX_V, th, vec![t2, out, w, h, r, 2, ps, 0, 0]);
}

/// Expanding a level of a local program's scratch in a kernel (`lc` reads), for the level-0
/// collapse that [`local_laplacian`] with no `out0` leaves to the kernel.
pub const LEVEL0_WGSL: &str = "fn dv_ltap(o: u32, w: u32, h: u32, x: i32, y: i32) -> f32 {
    let cx = u32(clamp(x, 0, i32(w) - 1));
    let cy = u32(clamp(y, 0, i32(h) - 1));
    return lc(o + cy * w + cx);
}
fn dv_lup(o: u32, w1: u32, h1: u32, x: u32, y: u32) -> f32 {
    let mx = i32(x >> 1u) - 1;
    let my = i32(y >> 1u) - 1;
    let ox = (x & 1u) == 1u;
    let oy = (y & 1u) == 1u;
    var acc = 0.0;
    for (var b = 0u; b < 3u; b++) {
        var row = 0.0;
        for (var a = 0u; a < 3u; a++) {
            row += dv_lew(ox, a) * dv_ltap(o, w1, h1, mx + i32(a), my + i32(b));
        }
        acc += dv_lew(oy, b) * row;
    }
    return acc;
}
fn dv_lew(odd: bool, i: u32) -> f32 {
    if (odd) {
        if (i == 0u) { return 0.0; }
        return 0.5;
    }
    if (i == 1u) { return 0.75; }
    return 0.125;
}
";

/// Shared WGSL for kernels reading statistics: bilinear samples of a grid plane at an
/// image pixel. The grid size is at `st(0)`, `st(1)`.
pub const GRID_WGSL: &str = "fn dv_grid(o: u32, px: i32, py: i32) -> f32 {
    let gw = u32(st(0u));
    let gh = u32(st(1u));
    let gx = clamp((f32(px) + 0.5) * f32(gw) / f32(pu(4u)) - 0.5, 0.0, f32(gw - 1u));
    let gy = clamp((f32(py) + 0.5) * f32(gh) / f32(pu(5u)) - 0.5, 0.0, f32(gh - 1u));
    let x0 = u32(floor(gx));
    let y0 = u32(floor(gy));
    let x1 = min(x0 + 1u, gw - 1u);
    let y1 = min(y0 + 1u, gh - 1u);
    let fx = gx - f32(x0);
    let fy = gy - f32(y0);
    let top = mix(st(o + y0 * gw + x0), st(o + y0 * gw + x1), fx);
    let bot = mix(st(o + y1 * gw + x0), st(o + y1 * gw + x1), fx);
    return mix(top, bot, fy);
}
";
