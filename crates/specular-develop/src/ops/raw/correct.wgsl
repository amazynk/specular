// Raw front end: the correction passes on the mosaic (see `ops::raw::correct` and
// `ops::correct`). Same conventions as `passes.wgsl`; `put1` stores one f32 of the
// corrected mosaic.

//@ pass cprep
// Corrections, step 1: the prepared sample and what it reads at the clip level.
fn run(x: i32, y: i32) {
    dst[u32(y - ui(U_D_Y0)) * uu(U_W) + u32(x)] = vec4<f32>(prep(x, y, 0u), prep_sat(x, y, 0u), 0.0, 0.0);
}

//@ pass hot
// Step 2: hot and dead pixels.
fn run(x: i32, y: i32) {
    dst[u32(y - ui(U_D_Y0)) * uu(U_W) + u32(x)] = vec4<f32>(hot_fix(x, y), 0.0, 0.0);
}

//@ pass hl
// Step 3: highlight reconstruction (see `ops::correct::highlights`).
fn run(x: i32, y: i32) {
    dst[u32(y - ui(U_D_Y0)) * uu(U_W) + u32(x)] = hl_value(x, y);
}

//@ pass cfuse
// Steps 1 to 3 at full size in one pass (workgroup memory): each 16x16 tile prepares its
// samples and three pixels around, fixes hot pixels on it and one pixel around, then
// rebuilds highlights. Every value is read where the three passes read it (`cpos`), so the
// corrected mosaic is theirs, bit for bit.
const CF_A: u32 = 22u;
const CF_B: u32 = 18u;
var<workgroup> CFA: array<vec2<f32>, 484>;
var<workgroup> CFB: array<vec2<f32>, 324>;
var<private> cf_stage: u32;
var<private> cf_x0: i32;
var<private> cf_y0: i32;
fn ld(p: vec2<i32>) -> vec2<f32> {
    if (cf_stage == 0u) {
        return CFA[u32(p.y - cf_y0 + 3) * CF_A + u32(p.x - cf_x0 + 3)];
    }
    return CFB[u32(p.y - cf_y0 + 1) * CF_B + u32(p.x - cf_x0 + 1)];
}
@compute @workgroup_size(16, 16, 1)
fn main(@builtin(local_invocation_id) lid: vec3<u32>, @builtin(workgroup_id) wid: vec3<u32>) {
    cf_x0 = i32(wid.x * 16u);
    cf_y0 = i32(uu(U_Y0) + wid.y * 16u);
    let li = lid.y * 16u + lid.x;
    for (var i = li; i < CF_A * CF_A; i += 256u) {
        let p = cpos(cf_x0 - 3 + i32(i % CF_A), cf_y0 - 3 + i32(i / CF_A));
        // Where `cpos` puts it: lookups go by the position they read.
        let t = u32(p.y - cf_y0 + 3) * CF_A + u32(p.x - cf_x0 + 3);
        if (t == i) {
            CFA[i] = prep2(p.x, p.y);
        }
    }
    workgroupBarrier();
    cf_stage = 0u;
    for (var i = li; i < CF_B * CF_B; i += 256u) {
        let p = cpos(cf_x0 - 1 + i32(i % CF_B), cf_y0 - 1 + i32(i / CF_B));
        let t = u32(p.y - cf_y0 + 1) * CF_B + u32(p.x - cf_x0 + 1);
        if (t == i) {
            CFB[i] = hot_fix(p.x, p.y);
        }
    }
    workgroupBarrier();
    cf_stage = 1u;
    let gx = cf_x0 + i32(lid.x);
    let gy = wid.y * 16u + lid.y;
    if (u32(gx) >= uu(U_W) || gy >= uu(U_ROWS)) { return; }
    let y = cf_y0 + i32(lid.y);
    dst[u32(y - ui(U_D_Y0)) * uu(U_W) + u32(gx)] = hl_value(gx, y);
}

//@ pass xtg
// Step 3b (X-Trans raw CA): the green plane, green's sites tent-averaged at every pixel
// (`xt_green` at integer positions), for the `ca` pass's ratios.
fn run(x: i32, y: i32) {
    xt_load();
    dst[u32(y - ui(U_D_Y0)) * uu(U_W) + u32(x)] = xt_green(vec2<f32>(f32(x), f32(y)));
}

//@ pass ca
// Step 4: raw chromatic aberration: red and blue resampled on their own lattices at
// p + (p - c)(a + b rho^2), Catmull-Rom held within the nearest four sites. (`mo` and the
// X-Trans helpers are in xt_common.wgsl.)
fn cr(t: f32) -> vec4<f32> {
    let t2 = t * t;
    let t3 = t2 * t;
    return vec4<f32>(
        -0.5 * t3 + t2 - 0.5 * t,
        1.5 * t3 - 2.5 * t2 + 1.0,
        -1.5 * t3 + 2.0 * t2 + 0.5 * t,
        0.5 * t3 - 0.5 * t2);
}
// The green plane of the `xtg` pass (source B) at pixel (x, y), reflected into the image.
fn gp(x: i32, y: i32) -> f32 {
    let r = refl(y, i32(uu(U_H)));
    return srcb[u32(r - ui(U_B_Y0)) * uu(U_W) + u32(refl(x, i32(uu(U_W))))];
}
// Green at q: the green plane interpolated bilinearly (smooth: it is already a tent
// average of the green sites).
fn xt_g(q: vec2<f32>) -> f32 {
    let ix = i32(floor(q.x));
    let iy = i32(floor(q.y));
    let fx = q.x - f32(ix);
    let fy = q.y - f32(iy);
    let a = mix(gp(ix, iy), gp(ix + 1, iy), fx);
    let b = mix(gp(ix, iy + 1), gp(ix + 1, iy + 1), fx);
    return mix(a, b, fy);
}
// Colour `col`'s ratio to green moved by -d (`col` at s shows the scene green shows at
// s - d), at q: the same tent over its sites in the 4x4 pixels around q. Guarded by
// XT_EPS so dark samples act as differences.
const XT_EPS: f32 = 0.002;
fn xt_ratio(q: vec2<f32>, col: u32, d: vec2<f32>) -> f32 {
    let ix = i32(floor(q.x));
    let iy = i32(floor(q.y));
    let w = i32(uu(U_W));
    let h = i32(uu(U_H));
    var s = 0.0;
    var n = 0.0;
    for (var j = -1; j <= 2; j += 1) {
        let sy = iy + j;
        let wy = max(0.0, 1.0 - abs(f32(sy) - q.y) * 0.5);
        for (var i = -1; i <= 2; i += 1) {
            let sx = ix + i;
            if (xc(refl(sx, w), refl(sy, h)) != col) { continue; }
            let wt = wy * max(0.0, 1.0 - abs(f32(sx) - q.x) * 0.5);
            let g = xt_g(vec2<f32>(f32(sx) - d.x, f32(sy) - d.y));
            s += wt * (mo(sx, sy) + XT_EPS) / (g + XT_EPS);
            n += wt;
        }
    }
    return s / max(n, 1e-6);
}
// The lowest and highest of colour `col`'s sites in the 4x4 pixels around q.
fn xt_range(q: vec2<f32>, col: u32) -> vec2<f32> {
    let ix = i32(floor(q.x));
    let iy = i32(floor(q.y));
    let w = i32(uu(U_W));
    let h = i32(uu(U_H));
    var lo = 3.0e38;
    var hi = -3.0e38;
    for (var j = -1; j <= 2; j += 1) {
        for (var i = -1; i <= 2; i += 1) {
            if (xc(refl(ix + i, w), refl(iy + j, h)) != col) { continue; }
            let s = mo(ix + i, iy + j);
            lo = min(lo, s);
            hi = max(hi, s);
        }
    }
    return vec2<f32>(lo, hi);
}
fn run(x: i32, y: i32) {
    let v = mo(x, y);
    let col = ccolor(x, y);
    if (du(H_CA) == 0u || col == 1u) { put1(x, y, v); return; }
    var a = df(H_CA_RA);
    var b = df(H_CA_RB);
    if (col == 2u) {
        a = df(H_CA_BA);
        b = df(H_CA_BB);
    }
    let dx = f32(x) - df(H_CA_CX);
    let dy = f32(y) - df(H_CA_CY);
    let rr = df(H_CA_R);
    let k = a + b * (dx * dx + dy * dy) / max(rr * rr, 1e-6);
    let d = clamp(vec2<f32>(dx * k, dy * k), vec2<f32>(-6.0), vec2<f32>(6.0));
    if (du(H_LAYOUT) != 0u) {
        // X-Trans: the colour's sites are irregular. Green at p times the colour's ratio
        // to green (aligned by d, so smooth) interpolated at p + d by normalised
        // convolution; held within the colour's sites around p + d. (Ratios leave less
        // than differences do where a grey edge has different levels per colour.)
        xt_load();
        let q = vec2<f32>(f32(x) + d.x, f32(y) + d.y);
        let lh = xt_range(q, col);
        var o = (gp(x, y) + XT_EPS) * xt_ratio(q, col, d) - XT_EPS;
        if (lh.x <= lh.y) { o = clamp(o, lh.x, lh.y); }
        put1(x, y, o);
        return;
    }
    // This colour's sites share (x, y)'s parity: lattice coordinates around them.
    let px = x & 1;
    let py = y & 1;
    let u = (f32(x) + d.x - f32(px)) * 0.5;
    let w = (f32(y) + d.y - f32(py)) * 0.5;
    let iu = i32(floor(u));
    let iw = i32(floor(w));
    let wx = cr(u - f32(iu));
    let wy = cr(w - f32(iw));
    var acc = 0.0;
    var lo = 3.0e38;
    var hi = -3.0e38;
    for (var j = 0; j < 4; j += 1) {
        var row = 0.0;
        for (var i = 0; i < 4; i += 1) {
            let s = mo(2 * (iu + i - 1) + px, 2 * (iw + j - 1) + py);
            row += wx[i] * s;
            if ((i == 1 || i == 2) && (j == 1 || j == 2)) {
                lo = min(lo, s);
                hi = max(hi, s);
            }
        }
        acc += wy[j] * row;
    }
    put1(x, y, clamp(acc, lo, hi));
}

//@ pass rcell
// Statistics, step 1: per cell (4x4 Bayer, 6x6 X-Trans: whole periods) the mean of each
// colour and which colours have a clipped sample (bits 0..2), from the stored samples
// (prepared and calibrated; no hot-pixel fix).
fn run(i: i32, j: i32) {
    let cs = i32(du(H_CS));
    let cw = i32(du(H_CW));
    let ch = i32(du(H_CH));
    let t = df(H_HL_T);
    var s = vec3<f32>(0.0);
    var n = vec3<f32>(0.0);
    var fl = 0u;
    for (var dy = 0; dy < cs; dy += 1) {
        for (var dx = 0; dx < cs; dx += 1) {
            let x = min(i * cs + dx, cw - 1);
            let y = min(j * cs + dy, ch - 1);
            let k = ccolor(x, y);
            let v = prep(x, y, 0u);
            if (v >= t * prep_sat(x, y, 0u)) { fl |= 1u << k; }
            s[k] += v;
            n[k] += 1.0;
        }
    }
    let m = s / max(n, vec3<f32>(1.0));
    dst[u32(j - ui(U_D_Y0)) * uu(U_W) + u32(i)] = vec2<u32>(pack2x16float(m.xy), pack2x16float(vec2<f32>(m.z, f32(fl))));
}

//@ pass hcell
// Half size with corrections, step 1 (`hcell_v`).
fn run(x: i32, y: i32) {
    dst[u32(y - ui(U_D_Y0)) * uu(U_W) + u32(x)] = hcell_v(x, y);
}

//@ pass hhot
// Half size with corrections, step 2 (`hhot_v`) on step 1's values in source A.
fn h4(x: i32, y: i32) -> vec4<f32> {
    let xx = clamp(x, 0, i32(uu(U_W)) - 1);
    let yy = clamp(y, 0, i32(uu(U_H)) - 1);
    return srca[u32(yy - ui(U_A_Y0)) * uu(U_W) + u32(xx)];
}
fn run(x: i32, y: i32) {
    dst[u32(y - ui(U_D_Y0)) * uu(U_W) + u32(x)] = hhot_v(x, y);
}

//@ pass hfuse
// Half size with corrections in one pass (no raw CA): steps 1 and 2 through workgroup
// memory, then the output transform. Each workgroup makes step 1 for its 16x16 pixels and
// a one-pixel ring (clamped at the image edges, as `hhot` reads them), so the result is
// the three-pass path's, bit for bit.
const HT_W: u32 = 18u;
var<workgroup> HT: array<vec4<f32>, 324>;
var<private> ht_x0: i32;
var<private> ht_y0: i32;
fn h4(x: i32, y: i32) -> vec4<f32> {
    return HT[u32(y - ht_y0) * HT_W + u32(x - ht_x0)];
}
@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>,
        @builtin(workgroup_id) wid: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    ht_x0 = i32(wid.x * 16u) - 1;
    ht_y0 = i32(uu(U_Y0) + wid.y * 16u) - 1;
    let w = i32(uu(U_W));
    let h = i32(uu(U_H));
    ht_fill(li, w, h);
    if (gid.x >= uu(U_W) || gid.y >= uu(U_ROWS)) { return; }
    let x = i32(gid.x);
    let y = i32(uu(U_Y0) + gid.y);
    // `hhot` reads its neighbours clamped to the image: so does the tile (held at the
    // clamped cell), so h4 sees the same values.
    emit(x, y, hhot_v(x, y));
}
fn emit(x: i32, y: i32, v: vec4<f32>) {
    let ix = du(H_CX) + 2u * u32(x);
    let iy = du(H_CY) + 2u * u32(y);
    put(x, y, finish(v.xyz, ix, iy, f32(ix) + 1.0, f32(iy) + 1.0));
}

//@ pass hfuseb
// Steps 1 and 2 in one pass (as `hfuse`) into a buffer, for `hout` (raw CA).
const HT_W: u32 = 18u;
var<workgroup> HT: array<vec4<f32>, 324>;
var<private> ht_x0: i32;
var<private> ht_y0: i32;
fn h4(x: i32, y: i32) -> vec4<f32> {
    return HT[u32(y - ht_y0) * HT_W + u32(x - ht_x0)];
}
@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>,
        @builtin(workgroup_id) wid: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    ht_x0 = i32(wid.x * 16u) - 1;
    ht_y0 = i32(uu(U_Y0) + wid.y * 16u) - 1;
    let w = i32(uu(U_W));
    let h = i32(uu(U_H));
    ht_fill(li, w, h);
    if (gid.x >= uu(U_W) || gid.y >= uu(U_ROWS)) { return; }
    let x = i32(gid.x);
    let y = i32(uu(U_Y0) + gid.y);
    // `hhot` reads its neighbours clamped to the image: so does the tile (held at the
    // clamped cell), so h4 sees the same values.
    emit(x, y, hhot_v(x, y));
}
fn emit(x: i32, y: i32, v: vec4<f32>) {
    dst[u32(y - ui(U_D_Y0)) * uu(U_W) + u32(x)] = v;
}

//@ pass hout
// Half size with corrections, step 3: red and blue sampled at the raw CA shift (bilinear,
// half the sensor shift in output pixels), then the output transform.
fn b4(x: i32, y: i32) -> vec3<f32> {
    let xx = clamp(x, 0, i32(uu(U_W)) - 1);
    let yy = clamp(y, 0, i32(uu(U_H)) - 1);
    return srca[u32(yy - ui(U_A_Y0)) * uu(U_W) + u32(xx)].xyz;
}
fn bil(px: f32, py: f32, k: u32) -> f32 {
    let x0 = floor(px);
    let y0 = floor(py);
    let fx = px - x0;
    let fy = py - y0;
    let ix = i32(x0);
    let iy = i32(y0);
    let a = mix(b4(ix, iy)[k], b4(ix + 1, iy)[k], fx);
    let b = mix(b4(ix, iy + 1)[k], b4(ix + 1, iy + 1)[k], fx);
    return mix(a, b, fy);
}
fn run(x: i32, y: i32) {
    var c = b4(x, y);
    if (du(H_CA) == 1u) {
        let dx = f32(2 * x) + 0.5 - df(H_CA_CX);
        let dy = f32(2 * y) + 0.5 - df(H_CA_CY);
        let rr = max(df(H_CA_R) * df(H_CA_R), 1e-6);
        let r2 = (dx * dx + dy * dy) / rr;
        let kr = df(H_CA_RA) + df(H_CA_RB) * r2;
        let kb = df(H_CA_BA) + df(H_CA_BB) * r2;
        let dr = clamp(vec2<f32>(dx * kr, dy * kr), vec2<f32>(-6.0), vec2<f32>(6.0)) * 0.5;
        let db = clamp(vec2<f32>(dx * kb, dy * kb), vec2<f32>(-6.0), vec2<f32>(6.0)) * 0.5;
        c.x = bil(f32(x) + dr.x, f32(y) + dr.y, 0u);
        c.z = bil(f32(x) + db.x, f32(y) + db.y, 2u);
    }
    let ix = du(H_CX) + 2u * u32(x);
    let iy = du(H_CY) + 2u * u32(y);
    put(x, y, finish(c, ix, iy, f32(ix) + 1.0, f32(iy) + 1.0));
}

//@ pass hltile
// Statistics, step 2: per tile of 16x16 cells, the brightness-squared weighted
// chromaticity of unclipped cells within two cells of a clipped one.
fn cellv(i: i32, j: i32) -> vec4<f32> {
    let ii = clamp(i, 0, i32(du(H_CELLS_W)) - 1);
    let jj = clamp(j, 0, i32(du(H_CELLS_H)) - 1);
    let p = srca[u32(jj) * du(H_CELLS_W) + u32(ii)];
    return vec4<f32>(unpack2x16float(p.x), unpack2x16float(p.y));
}
fn run(tx: i32, ty: i32) {
    let cw = i32(du(H_CELLS_W));
    let chh = i32(du(H_CELLS_H));
    let i0 = tx * 16;
    let j0 = ty * 16;
    let i1 = min(i0 + 16, cw);
    let j1 = min(j0 + 16, chh);
    let o = (u32(ty) * uu(U_W) + u32(tx)) * 6u;
    // Any clipped cell in reach?
    var any = false;
    for (var j = j0 - 2; j < j1 + 2; j += 1) {
        for (var i = i0 - 2; i < i1 + 2; i += 1) {
            if (j >= 0 && i >= 0 && j < chh && i < cw && u32(cellv(i, j).w) != 0u) { any = true; }
        }
    }
    var acc = vec3<f32>(0.0);
    var ws = 0.0;
    var cand = 0.0;
    var nclip = 0.0;
    if (any) {
        for (var j = j0; j < j1; j += 1) {
            for (var i = i0; i < i1; i += 1) {
                let c = cellv(i, j);
                if (u32(c.w) != 0u) {
                    nclip += 1.0;
                    continue;
                }
                let sum = c.x + c.y + c.z;
                if (sum <= 1e-6 || min(c.x, min(c.y, c.z)) < 0.0) { continue; }
                var near = false;
                for (var dj = -2; dj <= 2; dj += 1) {
                    for (var di = -2; di <= 2; di += 1) {
                        let q = vec2<i32>(i + di, j + dj);
                        if (q.x >= 0 && q.y >= 0 && q.x < cw && q.y < chh && u32(cellv(q.x, q.y).w) != 0u) {
                            near = true;
                        }
                    }
                }
                if (!near) { continue; }
                let w = sum * sum;
                acc += w * c.xyz / sum;
                ws += w;
                cand += 1.0;
            }
        }
    }
    dst[o] = acc.x;
    dst[o + 1u] = acc.y;
    dst[o + 2u] = acc.z;
    dst[o + 3u] = ws;
    dst[o + 4u] = cand;
    dst[o + 5u] = nclip;
}

//@ pass block
// Statistics for rebuild: per block of 4x4 cells, the cells clipped in every colour, the
// unclipped cells and their RGB sum, and the cells.
fn cellv(i: i32, j: i32) -> vec4<f32> {
    let p = srca[u32(j) * du(H_CELLS_W) + u32(i)];
    return vec4<f32>(unpack2x16float(p.x), unpack2x16float(p.y));
}
fn run(bx: i32, by: i32) {
    let cw = i32(du(H_CELLS_W));
    let chh = i32(du(H_CELLS_H));
    var blown = 0.0;
    var unc = 0.0;
    var rgb = vec3<f32>(0.0);
    var total = 0.0;
    for (var j = by * 4; j < min(by * 4 + 4, chh); j += 1) {
        for (var i = bx * 4; i < min(bx * 4 + 4, cw); i += 1) {
            let c = cellv(i, j);
            let fl = u32(c.w);
            total += 1.0;
            if (fl == 7u) { blown += 1.0; }
            if (fl == 0u && min(c.x, min(c.y, c.z)) >= 0.0) {
                unc += 1.0;
                rgb += c.xyz;
            }
        }
    }
    let o = (u32(by) * uu(U_W) + u32(bx)) * 6u;
    dst[o] = blown;
    dst[o + 1u] = unc;
    dst[o + 2u] = rgb.x;
    dst[o + 3u] = rgb.y;
    dst[o + 4u] = rgb.z;
    dst[o + 5u] = total;
}
