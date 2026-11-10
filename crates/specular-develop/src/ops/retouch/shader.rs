//! Retouch WGSL: the steps (low layers, blur, the multigrid solve) and the kernel that
//! mixes every spot over the picture. Layouts are in [`super::layout`].

use crate::graph::Kernel;

/// The scratch in the steps.
pub const STEP_WGSL: &str = "fn rt_sc(i: u32) -> f32 { return S[i]; }
fn rt_put(c: RtChunk, x: u32, y: u32, v: vec4<f32>) {
    let i = 4u * (c.off + y * c.w + x);
    S[i] = v.x;
    S[i + 1u] = v.y;
    S[i + 2u] = v.z;
    S[i + 3u] = v.w;
}
// A spot whose solve the scratch already holds (see `CHECK`).
fn rt_done(spot: u32) -> bool {
    return S[4u * rt_u(3u) + 8u * spot + 3u] == 1.0;
}
fn rt_job(tab: u32, n: u32, id: u32) -> u32 {
    var lo = 0u;
    var hi = n - 1u;
    loop {
        if (lo >= hi) { break; }
        let mid = (lo + hi + 1u) / 2u;
        if (rt_u(tab + mid * 32u) <= id) { lo = mid; } else { hi = mid - 1u; }
    }
    return tab + lo * 32u;
}
";

/// The scratch in the kernel.
pub const LC_WGSL: &str = "fn rt_sc(i: u32) -> f32 { return lc(i); }
";

/// Table, chunk, distance and layer helpers.
pub const COMMON_WGSL: &str = "fn rt_u(i: u32) -> u32 { return u32(max(dt(i), 0.0)); }
fn rt_i(i: u32) -> i32 { return i32(dt(i)); }
struct RtChunk { off: u32, w: u32, h: u32, ox: i32, oy: i32, s: u32, }
fn rt_chunk(i: u32) -> RtChunk {
    return RtChunk(rt_u(i), rt_u(i + 1u), rt_u(i + 2u), rt_i(i + 3u), rt_i(i + 4u), max(rt_u(i + 5u), 1u));
}
fn rt_cell(c: RtChunk, x: u32, y: u32) -> vec4<f32> {
    let i = 4u * (c.off + y * c.w + x);
    return vec4<f32>(rt_sc(i), rt_sc(i + 1u), rt_sc(i + 2u), rt_sc(i + 3u));
}
fn rt_cell0(c: RtChunk, x: i32, y: i32) -> vec4<f32> {
    if (x < 0 || y < 0 || x >= i32(c.w) || y >= i32(c.h)) { return vec4<f32>(0.0); }
    return rt_cell(c, u32(x), u32(y));
}
fn rt_nb(c: RtChunk, x: u32, y: u32) -> vec3<f32> {
    let xi = i32(x);
    let yi = i32(y);
    return rt_cell0(c, xi - 1, yi).xyz + rt_cell0(c, xi + 1, yi).xyz
        + rt_cell0(c, xi, yi - 1).xyz + rt_cell0(c, xi, yi + 1).xyz;
}
fn rt_bil(c: RtChunk, p: vec2<f32>) -> vec4<f32> {
    let f = (p - vec2<f32>(f32(c.ox), f32(c.oy))) / f32(c.s);
    let g = clamp(f, vec2<f32>(0.0), vec2<f32>(f32(c.w - 1u), f32(c.h - 1u)));
    let x0 = u32(floor(g.x));
    let y0 = u32(floor(g.y));
    let x1 = min(x0 + 1u, c.w - 1u);
    let y1 = min(y0 + 1u, c.h - 1u);
    let a = g - vec2<f32>(f32(x0), f32(y0));
    let top = mix(rt_cell(c, x0, y0), rt_cell(c, x1, y0), a.x);
    let bot = mix(rt_cell(c, x0, y1), rt_cell(c, x1, y1), a.x);
    return mix(top, bot, a.y);
}
fn rt_spot(i: u32) -> u32 { return rt_u(1u) + i * 64u; }
fn rt_dist(sr: u32, p: vec2<f32>) -> f32 {
    let n = rt_u(sr + 11u);
    if (n == 0u) { return length(p - vec2<f32>(dt(sr), dt(sr + 1u))); }
    let ps = rt_u(sr + 10u);
    var a = vec2<f32>(dt(ps), dt(ps + 1u));
    var d = length(p - a);
    for (var k = 1u; k < n; k++) {
        let b = vec2<f32>(dt(ps + 2u * k), dt(ps + 2u * k + 1u));
        let ab = b - a;
        let t = clamp(dot(p - a, ab) / max(dot(ab, ab), 1e-12), 0.0, 1.0);
        d = min(d, length(p - a - ab * t));
        a = b;
    }
    return d;
}
fn rt_live(sr: u32) -> bool {
    let rx = f32(pu(16u));
    let ry = f32(pu(17u));
    return dt(sr + 16u) >= rx && dt(sr + 17u) >= ry
        && dt(sr + 18u) <= rx + f32(pu(18u)) && dt(sr + 19u) <= ry + f32(pu(19u));
}
fn rt_layer(layer: u32, lo: RtChunk, p: vec2<i32>) -> vec3<f32> {
    let v = src_at(p.x, p.y).xyz;
    if (layer == 0u) { return v; }
    let l = rt_bil(lo, vec2<f32>(p)).xyz;
    if (layer == 1u) { return l; }
    return v - l;
}
fn rt_w(k: f32, sigma: f32) -> f32 { return exp(-(k * k) / (2.0 * sigma * sigma)); }
fn rt_patch(sr: u32, at: vec2<i32>) -> vec3<f32> {
    let off = rt_u(sr + 48u);
    let pw = rt_u(sr + 49u);
    let ph = rt_u(sr + 50u);
    let x = f32(at.x) + 0.5;
    let y = f32(at.y) + 0.5;
    let gx = clamp(dt(sr + 51u) * x + dt(sr + 52u) * y + dt(sr + 53u) - 0.5, 0.0, f32(pw - 1u));
    let gy = clamp(dt(sr + 54u) * x + dt(sr + 55u) * y + dt(sr + 56u) - 0.5, 0.0, f32(ph - 1u));
    let x0 = u32(floor(gx));
    let y0 = u32(floor(gy));
    let x1 = min(x0 + 1u, pw - 1u);
    let y1 = min(y0 + 1u, ph - 1u);
    let tx = gx - f32(x0);
    let ty = gy - f32(y0);
    let a = off + 3u * (y0 * pw + x0);
    let b = off + 3u * (y0 * pw + x1);
    let c = off + 3u * (y1 * pw + x0);
    let d = off + 3u * (y1 * pw + x1);
    let ta = vec3<f32>(dt(a), dt(a + 1u), dt(a + 2u));
    let tb = vec3<f32>(dt(b), dt(b + 1u), dt(b + 2u));
    let tc = vec3<f32>(dt(c), dt(c + 1u), dt(c + 2u));
    let td = vec3<f32>(dt(d), dt(d + 1u), dt(d + 2u));
    let top = ta + (tb - ta) * tx;
    let bot = tc + (td - tc) * tx;
    return top + (bot - top) * ty;
}
// A heal's source at picture pixel `at` (offset `q`): the removal patch when the spot has
// one, else the layer at the source.
fn rt_source(sr: u32, layer: u32, ls: RtChunk, at: vec2<i32>, q: vec2<i32>) -> vec3<f32> {
    if (rt_u(sr + 48u) > 0u) { return rt_patch(sr, at); }
    return rt_layer(layer, ls, q);
}
";

/// Every step starts by finding its job, spot and cell.
macro_rules! prologue {
    () => {
        "    let id = t.y * pu(2u) + t.x;
    if (id >= pu(p + 2u)) { return; }
    let jb = rt_job(pu(p), pu(p + 1u), id);
    let sr = rt_spot(rt_u(jb + 1u));
    if (!rt_live(sr)) { return; }
    let X = rt_chunk(jb + 2u);
    let k = id - rt_u(jb);
    let cx = k % X.w;
    let cy = k / X.w;
    let px = X.ox + i32(cx * X.s);
    let py = X.oy + i32(cy * X.s);
"
    };
}

/// [`prologue!`] for the solve's steps: spots already solved are left as they are.
macro_rules! solve_prologue {
    () => {
        concat!(
            prologue!(),
            "    if (rt_done(rt_u(jb + 1u))) { return; }
"
        )
    };
}

const HELPERS: &[&str] = &[COMMON_WGSL, STEP_WGSL];

/// The signature words of spot `t.x` (record words 45–47, see [`super::layout`]) and
/// where the scratch keeps them (after the cells, eight words per spot: three signature
/// words, the flag, the V-cycles the solve ran).
macro_rules! sig_prologue {
    () => {
        "    let i = t.x;
    if (i >= rt_u(0u)) { return; }
    let sr = rt_spot(i);
    let b = 4u * rt_u(3u) + 8u * i;
"
    };
}

/// A new scratch: no spot is solved.
pub static RESET: Kernel = Kernel {
    name: "rt_reset_v2",
    atomic: false,
    helpers: HELPERS,
    body: concat!(
        sig_prologue!(),
        "    S[b] = -1.0;
    S[b + 1u] = -1.0;
    S[b + 2u] = -1.0;
    S[b + 3u] = 0.0;
    S[b + 4u] = 0.0;"
    ),
};

/// Flags the spots whose solve the scratch holds: the signature it was solved with is the
/// spot's now (the engine keeps the scratch only while the input stays the same) and it ran
/// at least word 0 V-cycles (a draft keeps the refined solves of the spots not dragged).
pub static CHECK: Kernel = Kernel {
    name: "rt_check_v2",
    atomic: false,
    helpers: HELPERS,
    body: concat!(
        sig_prologue!(),
        "    let same = S[b] == dt(sr + 45u) && S[b + 1u] == dt(sr + 46u) && S[b + 2u] == dt(sr + 47u)
        && S[b + 4u] >= f32(pu(p));
    S[b + 3u] = select(0.0, 1.0, same);"
    ),
};

/// Records the signature and V-cycles (word 0) of each spot solved now.
pub static SIGN: Kernel = Kernel {
    name: "rt_sign_v2",
    atomic: false,
    helpers: HELPERS,
    body: concat!(
        sig_prologue!(),
        "    if (S[b + 3u] == 1.0) { return; }
    S[b] = dt(sr + 45u);
    S[b + 1u] = dt(sr + 46u);
    S[b + 2u] = dt(sr + 47u);
    S[b + 4u] = f32(pu(p));"
    ),
};

/// The low layer, along rows: the source blurred by the split σ (taps every pixel).
pub static LOW_H: Kernel = Kernel {
    name: "rt_low_h_v1",
    atomic: false,
    helpers: HELPERS,
    body: concat!(
        solve_prologue!(),
        "    let sg = dt(sr + 44u);
    let m = i32(ceil(3.0 * sg));
    var acc = vec3<f32>(0.0);
    var ws = 0.0;
    for (var j = -m; j <= m; j++) {
        let w = rt_w(f32(j), sg);
        acc += w * src_at(px + j, py).xyz;
        ws += w;
    }
    rt_put(X, cx, cy, vec4<f32>(acc / ws, 0.0));"
    ),
};

/// The low layer, along columns of the row pass (`Y`).
pub static LOW_V: Kernel = Kernel {
    name: "rt_low_v_v1",
    atomic: false,
    helpers: HELPERS,
    body: concat!(
        solve_prologue!(),
        "    let Y = rt_chunk(jb + 8u);
    let sg = dt(sr + 44u);
    let m = i32((Y.h - X.h) / 2u);
    var acc = vec3<f32>(0.0);
    var ws = 0.0;
    for (var j = -m; j <= m; j++) {
        let w = rt_w(f32(j * i32(X.s)), sg);
        acc += w * rt_cell(Y, cx, u32(i32(cy) + m + j)).xyz;
        ws += w;
    }
    rt_put(X, cx, cy, vec4<f32>(acc / ws, 0.0));"
    ),
};

/// A blur spot's layer along rows (`Y` the low layer under the target).
pub static BLUR_H: Kernel = Kernel {
    name: "rt_blur_h_v1",
    atomic: false,
    helpers: HELPERS,
    body: concat!(
        solve_prologue!(),
        "    let Y = rt_chunk(jb + 8u);
    let sg = dt(sr + 7u);
    let layer = rt_u(sr + 9u);
    let m = i32(ceil(3.0 * max(sg, 0.3) / f32(X.s)));
    var acc = vec3<f32>(0.0);
    var ws = 0.0;
    for (var j = -m; j <= m; j++) {
        let w = rt_w(f32(j * i32(X.s)), sg);
        acc += w * rt_layer(layer, Y, vec2<i32>(px + j * i32(X.s), py));
        ws += w;
    }
    rt_put(X, cx, cy, vec4<f32>(acc / ws, 0.0));"
    ),
};

/// A blur spot along columns of the row pass (`Y`).
pub static BLUR_V: Kernel = Kernel {
    name: "rt_blur_v_v1",
    atomic: false,
    helpers: HELPERS,
    body: concat!(
        solve_prologue!(),
        "    let Y = rt_chunk(jb + 8u);
    let sg = dt(sr + 7u);
    let m = i32((Y.h - X.h) / 2u);
    var acc = vec3<f32>(0.0);
    var ws = 0.0;
    for (var j = -m; j <= m; j++) {
        let w = rt_w(f32(j * i32(X.s)), sg);
        acc += w * rt_cell(Y, cx, u32(i32(cy) + m + j)).xyz;
        ws += w;
    }
    rt_put(X, cx, cy, vec4<f32>(acc / ws, 0.0));"
    ),
};

/// `G0`: free cells (inside the spot, off the border) start at 0; the others hold the edge
/// value, target minus source for heal, the target for fill (`Y`, `Z`: low layers).
pub static INIT: Kernel = Kernel {
    name: "rt_init_v1",
    atomic: false,
    helpers: HELPERS,
    body: concat!(
        solve_prologue!(),
        "    let at = vec2<i32>(px, py);
    let d = rt_dist(sr, vec2<f32>(at));
    let free = d < dt(sr + 4u) && cx > 0u && cy > 0u && cx + 1u < X.w && cy + 1u < X.h;
    if (free) {
        rt_put(X, cx, cy, vec4<f32>(0.0, 0.0, 0.0, 1.0));
        return;
    }
    let layer = rt_u(sr + 9u);
    var b = rt_layer(layer, rt_chunk(jb + 8u), at);
    if (rt_u(sr + 8u) == 0u) {
        let q = at + vec2<i32>(rt_i(sr + 2u), rt_i(sr + 3u));
        b -= rt_source(sr, layer, rt_chunk(jb + 14u), at, q);
    }
    rt_put(X, cx, cy, vec4<f32>(b, 0.0));"
    ),
};

/// One red–black Gauss–Seidel half sweep (word 3: the colour) of `(4u − Σ)/h² = r`.
pub static SMOOTH: Kernel = Kernel {
    name: "rt_smooth_v1",
    atomic: false,
    helpers: HELPERS,
    body: concat!(
        solve_prologue!(),
        "    if (((cx + cy) & 1u) != pu(p + 3u)) { return; }
    let v = rt_cell(X, cx, cy);
    if (v.w < 0.5) { return; }
    let Y = rt_chunk(jb + 8u);
    var rhs = vec3<f32>(0.0);
    if (Y.w > 0u) { rhs = rt_cell(Y, cx, cy).xyz; }
    let h2 = f32(1u << (2u * rt_u(jb + 26u)));
    rt_put(X, cx, cy, vec4<f32>((rt_nb(X, cx, cy) + h2 * rhs) * 0.25, 1.0));"
    ),
};

/// Coarse cell (`X` error, `Y` right-hand side) from its four children (`Z` error, `W`
/// right-hand side): the mean residual, error 0, free when all four children are.
pub static RESTRICT: Kernel = Kernel {
    name: "rt_restrict_v1",
    atomic: false,
    helpers: HELPERS,
    body: concat!(
        solve_prologue!(),
        "    let Y = rt_chunk(jb + 8u);
    let Z = rt_chunk(jb + 14u);
    let W = rt_chunk(jb + 20u);
    let h2 = f32(1u << (2u * rt_u(jb + 26u)));
    var acc = vec3<f32>(0.0);
    var fr = 0.0;
    for (var b = 0u; b < 2u; b++) {
        for (var a = 0u; a < 2u; a++) {
            let fx = 2u * cx + a;
            let fy = 2u * cy + b;
            if (fx >= Z.w || fy >= Z.h) { continue; }
            let v = rt_cell(Z, fx, fy);
            if (v.w < 0.5) { continue; }
            fr += 0.25;
            var rhs = vec3<f32>(0.0);
            if (W.w > 0u) { rhs = rt_cell(W, fx, fy).xyz; }
            acc += rhs - (4.0 * v.xyz - rt_nb(Z, fx, fy)) / h2;
        }
    }
    rt_put(X, cx, cy, vec4<f32>(0.0, 0.0, 0.0, select(0.0, 1.0, fr > 0.9)));
    rt_put(Y, cx, cy, vec4<f32>(acc * 0.25, 0.0));"
    ),
};

/// Fine free cells (`X`) take the bilinear coarse error (`Z`).
pub static PROLONG: Kernel = Kernel {
    name: "rt_prolong_v1",
    atomic: false,
    helpers: HELPERS,
    body: concat!(
        solve_prologue!(),
        "    let v = rt_cell(X, cx, cy);
    if (v.w < 0.5) { return; }
    let Z = rt_chunk(jb + 14u);
    let e = rt_bil(Z, vec2<f32>(f32(cx), f32(cy)) * 0.5 - vec2<f32>(0.25));
    rt_put(X, cx, cy, vec4<f32>(v.xyz + e.xyz, 1.0));"
    ),
};

/// Workgroup memory of [`COARSE`]: one spot's coarsest level (at most
/// [`super::layout::COARSEST`]² cells) and its right-hand side.
const COARSE_WGSL: &str = "var<workgroup> RT_G: array<vec4<f32>, 64>;
var<workgroup> RT_R: array<vec4<f32>, 64>;
fn rt_g0(w: u32, h: u32, x: i32, y: i32) -> vec4<f32> {
    if (x < 0 || y < 0 || x >= i32(w) || y >= i32(h)) { return vec4<f32>(0.0); }
    return RT_G[u32(y) * w + u32(x)];
}
";

/// Threads of [`COARSE`] per job: one workgroup row, so each job has its workgroup's
/// memory to itself.
pub const COARSE_SPACING: u32 = crate::gpu::wgsl::WG;
const _: () = assert!(COARSE_SPACING == 16, "the kernel's `t.x % 16u`");

/// The coarsest level of a spot, by one thread: word 3 sweeps of Gauss–Seidel over-relaxed
/// by ω = 1.5. The level is swept in workgroup memory (the thread at the start of each
/// workgroup row, [`COARSE_SPACING`] threads per job) and written back after: the same
/// arithmetic in the same order as sweeping it in the scratch.
pub static COARSE: Kernel = Kernel {
    name: "rt_coarse_v2",
    atomic: false,
    helpers: &[COMMON_WGSL, STEP_WGSL, COARSE_WGSL],
    body: "    if (t.y != 0u || t.x % 16u != 0u) { return; }
    let id = t.x / 16u;
    if (id >= pu(p + 2u)) { return; }
    let jb = rt_job(pu(p), pu(p + 1u), id);
    let sr = rt_spot(rt_u(jb + 1u));
    if (!rt_live(sr)) { return; }
    if (rt_done(rt_u(jb + 1u))) { return; }
    let X = rt_chunk(jb + 2u);
    let Y = rt_chunk(jb + 8u);
    let h2 = f32(1u << (2u * rt_u(jb + 26u)));
    let w = min(X.w, 8u);
    let h = min(X.h, 8u);
    for (var y = 0u; y < h; y++) {
        for (var x = 0u; x < w; x++) {
            RT_G[y * w + x] = rt_cell(X, x, y);
            var rhs = vec4<f32>(0.0);
            if (Y.w > 0u) { rhs = rt_cell(Y, x, y); }
            RT_R[y * w + x] = rhs;
        }
    }
    for (var it = 0u; it < pu(p + 3u); it++) {
        for (var y = 0u; y < h; y++) {
            for (var x = 0u; x < w; x++) {
                let v = RT_G[y * w + x];
                if (v.w < 0.5) { continue; }
                let rhs = RT_R[y * w + x].xyz;
                let xi = i32(x);
                let yi = i32(y);
                let nb = rt_g0(w, h, xi - 1, yi).xyz + rt_g0(w, h, xi + 1, yi).xyz
                    + rt_g0(w, h, xi, yi - 1).xyz + rt_g0(w, h, xi, yi + 1).xyz;
                let gs = (nb + h2 * rhs) * 0.25;
                RT_G[y * w + x] = vec4<f32>(v.xyz + 1.5 * (gs - v.xyz), 1.0);
            }
        }
    }
    for (var y = 0u; y < h; y++) {
        for (var x = 0u; x < w; x++) {
            let v = RT_G[y * w + x];
            if (v.w >= 0.5) { rt_put(X, x, y, v); }
        }
    }",
};

/// Shared WGSL of the fused multigrid steps ([`DOWN`], [`UP`], [`TAIL`]).
///
/// The tile steps sweep a tile of a level in workgroup memory (`RT_T`, [`TILE_LOAD`]² cells
/// at most) with a halo of one cell per half sweep (each half sweep leaves one more ring at
/// the edge stale), so the tile's middle gets the same values as sweeping the whole level.
/// Red–black sweeps make each cell's update independent of the order: the same arithmetic
/// as [`SMOOTH`], [`RESTRICT`] and [`PROLONG`] in the same order per cell.
const FUSED_WGSL: &str = "var<workgroup> RT_T: array<vec4<f32>, 900>;
var<workgroup> RT_G: array<vec4<f32>, 64>;
var<workgroup> RT_R: array<vec4<f32>, 64>;
fn rt_g0(w: u32, h: u32, x: i32, y: i32) -> vec4<f32> {
    if (x < 0 || y < 0 || x >= i32(w) || y >= i32(h)) { return vec4<f32>(0.0); }
    return RT_G[u32(y) * w + u32(x)];
}
// `sweeps` red-black sweeps of the n×n tile in RT_T whose cell (0, 0) is level cell (x0, y0);
// `R` the right-hand side (w = 0: none).
fn rt_tsweep(R: RtChunk, h2: f32, x0: i32, y0: i32, n: u32, li: u32, sweeps: u32) {
    for (var s = 0u; s < 2u * sweeps; s++) {
        let col = s & 1u;
        for (var i = li; i < n * n; i += 256u) {
            let a = i % n;
            let b = i / n;
            if (a == 0u || b == 0u || a + 1u == n || b + 1u == n) { continue; }
            let fx = x0 + i32(a);
            let fy = y0 + i32(b);
            if ((u32(fx + fy) & 1u) != col) { continue; }
            let v = RT_T[i];
            if (v.w < 0.5) { continue; }
            var rhs = vec3<f32>(0.0);
            if (R.w > 0u) { rhs = rt_cell(R, u32(fx), u32(fy)).xyz; }
            let nb = RT_T[i - 1u].xyz + RT_T[i + 1u].xyz + RT_T[i - n].xyz + RT_T[i + n].xyz;
            RT_T[i] = vec4<f32>((nb + h2 * rhs) * 0.25, 1.0);
        }
        workgroupBarrier();
    }
}
// The same on a whole level in the scratch (one workgroup).
fn rt_gsweep(E: RtChunk, R: RtChunk, h2: f32, li: u32, sweeps: u32) {
    for (var s = 0u; s < 2u * sweeps; s++) {
        let col = s & 1u;
        for (var i = li; i < E.w * E.h; i += 256u) {
            let cx = i % E.w;
            let cy = i / E.w;
            if (((cx + cy) & 1u) != col) { continue; }
            let v = rt_cell(E, cx, cy);
            if (v.w < 0.5) { continue; }
            var rhs = vec3<f32>(0.0);
            if (R.w > 0u) { rhs = rt_cell(R, cx, cy).xyz; }
            rt_put(E, cx, cy, vec4<f32>((rt_nb(E, cx, cy) + h2 * rhs) * 0.25, 1.0));
        }
        storageBarrier();
    }
}
// The coarsest level as `COARSE` sweeps it (lexicographic Gauss–Seidel, ω = 1.5), on a
// wavefront: cell (x, y) of sweep `it` runs at step 2·it + x + y, after its left and upper
// neighbours of this sweep and its right and lower ones of the last, and no two cells of
// one step touch: the same values as one thread sweeping, in 2·(sweeps − 1) + w + h − 1
// steps instead of sweeps·w·h.
fn rt_sor(X: RtChunk, Y: RtChunk, h2: f32, sweeps: u32, li: u32) {
    let w = min(X.w, 8u);
    let h = min(X.h, 8u);
    let x = li % 8u;
    let y = li / 8u;
    let mine = li < 64u && x < w && y < h;
    if (mine) {
        RT_G[y * w + x] = rt_cell(X, x, y);
        var rhs = vec4<f32>(0.0);
        if (Y.w > 0u) { rhs = rt_cell(Y, x, y); }
        RT_R[y * w + x] = rhs;
    }
    workgroupBarrier();
    let steps = 2u * max(sweeps, 1u) - 2u + w + h - 1u;
    for (var t = 0u; t < steps; t++) {
        if (mine && sweeps > 0u && t >= x + y) {
            let d = t - x - y;
            if ((d & 1u) == 0u && d / 2u < sweeps) {
                let v = RT_G[y * w + x];
                if (v.w >= 0.5) {
                    let rhs = RT_R[y * w + x].xyz;
                    let xi = i32(x);
                    let yi = i32(y);
                    let nb = rt_g0(w, h, xi - 1, yi).xyz + rt_g0(w, h, xi + 1, yi).xyz
                        + rt_g0(w, h, xi, yi - 1).xyz + rt_g0(w, h, xi, yi + 1).xyz;
                    let gs = (nb + h2 * rhs) * 0.25;
                    RT_G[y * w + x] = vec4<f32>(v.xyz + 1.5 * (gs - v.xyz), 1.0);
                }
            }
        }
        workgroupBarrier();
    }
    if (mine) {
        let v = RT_G[y * w + x];
        if (v.w >= 0.5) { rt_put(X, x, y, v); }
    }
}
";

/// Side of the cells a tile step loads ([`DOWN`]: 16 + 2·7).
pub const TILE_LOAD: u32 = 30;
const _: () = assert!(TILE_LOAD * TILE_LOAD <= 900, "RT_T");
const _: () = assert!(
    16 + 2 * (2 * super::layout::PRE + 1) <= TILE_LOAD
        && 16 + 2 * 2 * super::layout::POST <= TILE_LOAD,
    "the tiles' halos"
);

/// Every fused step starts by finding its job (one per workgroup); spots out of this tile or
/// already solved leave the whole workgroup.
macro_rules! wg_prologue {
    () => {
        "//workgroup
    if (w >= pu(p + 2u)) { return; }
    let jb = rt_job(pu(p), pu(p + 1u), w);
    let sr = rt_spot(rt_u(jb + 1u));
    if (!rt_live(sr)) { return; }
    if (rt_done(rt_u(jb + 1u))) { return; }
    let k = w - rt_u(jb);
"
    };
}

const FUSED: &[&str] = &[COMMON_WGSL, STEP_WGSL, FUSED_WGSL];

/// Pre-smoothing of level `l` and restriction to `l + 1`, per tile of 8×8 coarse cells:
/// [`super::layout::PRE`] red–black sweeps of the 16×16 fine cells under it in workgroup
/// memory (halo 7: six half sweeps and the residual's neighbours), then the coarse cells.
/// Chunks: `X` coarse error, `Y` coarse right-hand side, `Z` fine error, `W` fine
/// right-hand side; word 27 the offset of the fine error's copy the sweeps write (the
/// neighbouring tiles still read `Z`). Words: table, jobs, tiles.
pub static DOWN: Kernel = Kernel {
    name: "rt_down_v1",
    atomic: false,
    helpers: FUSED,
    body: concat!(
        wg_prologue!(),
        "    let X = rt_chunk(jb + 2u);
    let Y = rt_chunk(jb + 8u);
    let Z = rt_chunk(jb + 14u);
    let W = rt_chunk(jb + 20u);
    var B = Z;
    B.off = rt_u(jb + 27u);
    let h2 = f32(1u << (2u * rt_u(jb + 26u)));
    let tw = (X.w + 7u) / 8u;
    let ti = k % tw;
    let tj = k / tw;
    let x0 = i32(16u * ti) - 7;
    let y0 = i32(16u * tj) - 7;
    for (var i = li; i < 900u; i += 256u) {
        RT_T[i] = rt_cell0(Z, x0 + i32(i % 30u), y0 + i32(i / 30u));
    }
    workgroupBarrier();
    rt_tsweep(W, h2, x0, y0, 30u, li, 3u);
    let fx = 16u * ti + li % 16u;
    let fy = 16u * tj + li / 16u;
    if (fx < Z.w && fy < Z.h) { rt_put(B, fx, fy, RT_T[(7u + li / 16u) * 30u + 7u + li % 16u]); }
    if (li < 64u) {
        let cx = 8u * ti + li % 8u;
        let cy = 8u * tj + li / 8u;
        if (cx < X.w && cy < X.h) {
            var acc = vec3<f32>(0.0);
            var fr = 0.0;
            for (var b = 0u; b < 2u; b++) {
                for (var a = 0u; a < 2u; a++) {
                    let gx = 2u * cx + a;
                    let gy = 2u * cy + b;
                    if (gx >= Z.w || gy >= Z.h) { continue; }
                    let i = u32(i32(gy) - y0) * 30u + u32(i32(gx) - x0);
                    let v = RT_T[i];
                    if (v.w < 0.5) { continue; }
                    fr += 0.25;
                    var rhs = vec3<f32>(0.0);
                    if (W.w > 0u) { rhs = rt_cell(W, gx, gy).xyz; }
                    let nb = RT_T[i - 1u].xyz + RT_T[i + 1u].xyz + RT_T[i - 30u].xyz
                        + RT_T[i + 30u].xyz;
                    acc += rhs - (4.0 * v.xyz - nb) / h2;
                }
            }
            rt_put(X, cx, cy, vec4<f32>(0.0, 0.0, 0.0, select(0.0, 1.0, fr > 0.9)));
            rt_put(Y, cx, cy, vec4<f32>(acc * 0.25, 0.0));
        }
    }"
    ),
};

/// Prolongation from level `l + 1` and post-smoothing of level `l`, per tile of 16×16 fine
/// cells (halo 6): reads the pre-smoothed copy (word 27, written by [`DOWN`]) and writes the
/// error `X`. Chunks: `X` fine error, `Y` fine right-hand side, `Z` coarse error. Words:
/// table, jobs, tiles.
pub static UP: Kernel = Kernel {
    name: "rt_up_v1",
    atomic: false,
    helpers: FUSED,
    body: concat!(
        wg_prologue!(),
        "    let X = rt_chunk(jb + 2u);
    let Y = rt_chunk(jb + 8u);
    let Z = rt_chunk(jb + 14u);
    var B = X;
    B.off = rt_u(jb + 27u);
    let h2 = f32(1u << (2u * rt_u(jb + 26u)));
    let tw = (X.w + 15u) / 16u;
    let ti = k % tw;
    let tj = k / tw;
    let x0 = i32(16u * ti) - 6;
    let y0 = i32(16u * tj) - 6;
    for (var i = li; i < 784u; i += 256u) {
        let fx = x0 + i32(i % 28u);
        let fy = y0 + i32(i / 28u);
        var v = rt_cell0(B, fx, fy);
        if (v.w >= 0.5) {
            let e = rt_bil(Z, vec2<f32>(f32(fx), f32(fy)) * 0.5 - vec2<f32>(0.25));
            v = vec4<f32>(v.xyz + e.xyz, 1.0);
        }
        RT_T[i] = v;
    }
    workgroupBarrier();
    rt_tsweep(Y, h2, x0, y0, 28u, li, 3u);
    let fx = 16u * ti + li % 16u;
    let fy = 16u * tj + li / 16u;
    let v = RT_T[(6u + li / 16u) * 28u + 6u + li % 16u];
    if (fx < X.w && fy < X.h && v.w >= 0.5) { rt_put(X, fx, fy, v); }"
    ),
};

/// The small levels of a spot's V-cycle by one workgroup in the scratch: from level `l`
/// (word 26; `X` its error, `Y` its right-hand side, `Z` the next level's error) down
/// word 27 levels (each level's error, then its right-hand side, then the next level), the
/// coarsest by [`rt_sor`] with word 3 sweeps, and back up. Words: table, jobs, jobs,
/// coarse sweeps.
pub static TAIL: Kernel = Kernel {
    name: "rt_tail_v1",
    atomic: false,
    helpers: FUSED,
    body: concat!(
        wg_prologue!(),
        "    let l0 = rt_u(jb + 26u);
    let m = rt_u(jb + 27u);
    var es: array<RtChunk, 24>;
    var rs: array<RtChunk, 24>;
    es[0] = rt_chunk(jb + 2u);
    rs[0] = rt_chunk(jb + 8u);
    var c = rt_chunk(jb + 14u);
    for (var j = 1u; j <= m; j++) {
        es[j] = c;
        rs[j] = RtChunk(c.off + c.w * c.h, c.w, c.h, 0, 0, 1u);
        c = RtChunk(c.off + 2u * c.w * c.h, (c.w + 1u) / 2u, (c.h + 1u) / 2u, 0, 0, 1u);
    }
    for (var j = 0u; j < m; j++) {
        let E = es[j];
        let R = rs[j];
        let h2 = f32(1u << (2u * (l0 + j)));
        rt_gsweep(E, R, h2, li, 3u);
        let X = es[j + 1u];
        let Y = rs[j + 1u];
        for (var i = li; i < X.w * X.h; i += 256u) {
            let cx = i % X.w;
            let cy = i / X.w;
            var acc = vec3<f32>(0.0);
            var fr = 0.0;
            for (var b = 0u; b < 2u; b++) {
                for (var a = 0u; a < 2u; a++) {
                    let fx = 2u * cx + a;
                    let fy = 2u * cy + b;
                    if (fx >= E.w || fy >= E.h) { continue; }
                    let v = rt_cell(E, fx, fy);
                    if (v.w < 0.5) { continue; }
                    fr += 0.25;
                    var rhs = vec3<f32>(0.0);
                    if (R.w > 0u) { rhs = rt_cell(R, fx, fy).xyz; }
                    acc += rhs - (4.0 * v.xyz - rt_nb(E, fx, fy)) / h2;
                }
            }
            rt_put(X, cx, cy, vec4<f32>(0.0, 0.0, 0.0, select(0.0, 1.0, fr > 0.9)));
            rt_put(Y, cx, cy, vec4<f32>(acc * 0.25, 0.0));
        }
        storageBarrier();
    }
    rt_sor(es[m], rs[m], f32(1u << (2u * (l0 + m))), pu(p + 3u), li);
    storageBarrier();
    for (var j = m; j > 0u; j--) {
        let E = es[j - 1u];
        let Z = es[j];
        for (var i = li; i < E.w * E.h; i += 256u) {
            let cx = i % E.w;
            let cy = i / E.w;
            let v = rt_cell(E, cx, cy);
            if (v.w < 0.5) { continue; }
            let e = rt_bil(Z, vec2<f32>(f32(cx), f32(cy)) * 0.5 - vec2<f32>(0.25));
            rt_put(E, cx, cy, vec4<f32>(v.xyz + e.xyz, 1.0));
        }
        storageBarrier();
        rt_gsweep(E, rs[j - 1u], f32(1u << (2u * (l0 + j - 1u))), li, 3u);
    }"
    ),
};

/// The kernel: every spot over the picture, in order.
pub const COMPOSITE_WGSL: &str = "fn rt_fine(layer: u32, lo: RtChunk, q: vec2<i32>) -> vec3<f32> {
    var acc = vec3<f32>(0.0);
    var ws = 0.0;
    for (var j = -3; j <= 3; j++) {
        for (var i = -3; i <= 3; i++) {
            let w = rt_w(f32(i), 1.0) * rt_w(f32(j), 1.0);
            acc += w * rt_layer(layer, lo, q + vec2<i32>(i, j));
            ws += w;
        }
    }
    return acc / ws;
}
fn rt_composite(x: i32, y: i32) -> vec4<f32> {
    let base = src_at(x, y);
    var c = base.xyz;
    let n = rt_u(0u);
    let pp = vec2<f32>(f32(x), f32(y));
    for (var i = 0u; i < n; i++) {
        let sr = rt_spot(i);
        if (pp.x < dt(sr + 12u) || pp.y < dt(sr + 13u) || pp.x >= dt(sr + 14u) || pp.y >= dt(sr + 15u)) {
            continue;
        }
        if (!rt_live(sr)) { continue; }
        let r = dt(sr + 4u);
        let d = rt_dist(sr, pp);
        if (d >= r) { continue; }
        let f = dt(sr + 5u);
        var m = clamp(r - d, 0.0, 1.0);
        if (f > 0.0) { m = min(m, 1.0 - smoothstep(r * (1.0 - f), r, d)); }
        m *= dt(sr + 6u);
        let mode = rt_u(sr + 8u);
        let layer = rt_u(sr + 9u);
        let ls = rt_chunk(sr + 32u);
        let q = vec2<i32>(x + rt_i(sr + 2u), y + rt_i(sr + 3u));
        var pt = vec3<f32>(0.0);
        if (mode == 3u) {
            pt = rt_bil(rt_chunk(sr + 38u), pp).xyz;
        } else {
            if (mode == 1u) {
                pt = rt_layer(layer, ls, q);
            } else {
                let g = rt_bil(rt_chunk(sr + 20u), pp).xyz;
                if (mode == 0u) {
                    pt = rt_source(sr, layer, ls, vec2<i32>(x, y), q) + g;
                } else {
                    pt = g + rt_layer(layer, ls, q) - rt_fine(layer, ls, q);
                }
            }
        }
        var other = vec3<f32>(0.0);
        if (layer != 0u) {
            let lo = rt_bil(rt_chunk(sr + 26u), pp).xyz;
            if (layer == 1u) { other = base.xyz - lo; } else { other = lo; }
        }
        c += m * (pt - (c - other));
    }
    return vec4<f32>(c, base.w);
}
";
