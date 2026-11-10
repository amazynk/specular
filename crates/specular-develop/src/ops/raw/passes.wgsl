// Raw front end: the pass bodies. `ops::raw::shaders` puts each between the bindings and
// `main` (`//@ pass <name>` starts a pass; `put`, `ma` and `mb` come from the prelude).

//@ pass prep
// The mosaic: one normalised, white-balanced sample per pixel.
fn run(x: i32, y: i32) {
    dst[u32(y - ui(U_D_Y0)) * uu(U_W) + u32(x)] = prep(x, y, 0u);
}

//@ pass direct
// Linear RGB and monochrome data at full size: no demosaic.
fn run(x: i32, y: i32) {
    var c = vec3<f32>(prep(x, y, 0u));
    if (du(H_CPP) == 3u) { c = vec3<f32>(c.x, prep(x, y, 1u), prep(x, y, 2u)); }
    put(x, y, finish_full(c, u32(x), u32(y)));
}

//@ pass mosaic
// The mosaic as output (red channel), for checks and highlight work.
fn run(x: i32, y: i32) {
    put(x, y, vec4<f32>(prep(x, y, 0u), 0.0, 0.0, 1.0));
}

//@ pass half
// Half size: one output pixel per 2x2 block. Bayer: the block's samples per colour.
// Larger patterns: per-colour means over the 4x4 window around the block. Linear and
// monochrome: the block mean.
fn run(x: i32, y: i32) {
    let cw = i32(du(H_CW));
    let ch = i32(du(H_CH));
    let l = du(H_LAYOUT);
    var sum = vec3<f32>(0.0);
    var n = vec3<f32>(0.0);
    var r0 = 0;
    var r1 = 2;
    if (l == 1u) { r0 = -1; r1 = 3; }
    for (var dy = r0; dy < r1; dy += 1) {
        for (var dx = r0; dx < r1; dx += 1) {
            let sx = clamp(2 * x + dx, 0, cw - 1);
            let sy = clamp(2 * y + dy, 0, ch - 1);
            if (l <= 1u) {
                let c = ccolor(sx, sy);
                let v = prep(sx, sy, 0u);
                if (c == 0u) { sum.x += v; n.x += 1.0; }
                else if (c == 1u) { sum.y += v; n.y += 1.0; }
                else { sum.z += v; n.z += 1.0; }
            } else if (du(H_CPP) == 3u) {
                sum += vec3<f32>(prep(sx, sy, 0u), prep(sx, sy, 1u), prep(sx, sy, 2u));
                n += vec3<f32>(1.0);
            } else {
                sum += vec3<f32>(prep(sx, sy, 0u));
                n += vec3<f32>(1.0);
            }
        }
    }
    let c = sum / max(n, vec3<f32>(1.0));
    let ix = du(H_CX) + 2u * u32(x);
    let iy = du(H_CY) + 2u * u32(y);
    put(x, y, finish(c, ix, iy, f32(ix) + 1.0, f32(iy) + 1.0));
}

//@ pass mhc
// Malvar-He-Cutler (2004): bilinear plus a gradient correction, 5x5 kernels.
fn m(x: i32, y: i32) -> f32 {
    let r = refl(y, i32(uu(U_H)));
    return srca[u32(r - ui(U_A_Y0)) * uu(U_W) + u32(refl(x, i32(uu(U_W))))];
}
fn run(x: i32, y: i32) {
    let c = ccolor(x, y);
    let v = m(x, y);
    let cross = m(x, y - 1) + m(x, y + 1) + m(x - 1, y) + m(x + 1, y);
    let diag = m(x - 1, y - 1) + m(x + 1, y - 1) + m(x - 1, y + 1) + m(x + 1, y + 1);
    let far = m(x, y - 2) + m(x, y + 2) + m(x - 2, y) + m(x + 2, y);
    let farh = m(x - 2, y) + m(x + 2, y);
    let farv = m(x, y - 2) + m(x, y + 2);
    var rgb = vec3<f32>(0.0);
    if (c == 1u) {
        // Green: red and blue from the row or the column.
        let h = (5.0 * v + 4.0 * (m(x - 1, y) + m(x + 1, y)) - diag - farh + 0.5 * farv) / 8.0;
        let w = (5.0 * v + 4.0 * (m(x, y - 1) + m(x, y + 1)) - diag - farv + 0.5 * farh) / 8.0;
        if (ccolor(x + 1, y) == 0u) {
            rgb = vec3<f32>(h, v, w);
        } else {
            rgb = vec3<f32>(w, v, h);
        }
    } else {
        let g = (4.0 * v + 2.0 * cross - far) / 8.0;
        let o = (6.0 * v + 2.0 * diag - 1.5 * far) / 8.0;
        if (c == 0u) { rgb = vec3<f32>(v, g, o); } else { rgb = vec3<f32>(o, g, v); }
    }
    put(x, y, finish_full(rgb, u32(x), u32(y)));
}

//@ pass rcd1
// Ratio-corrected directional demosaic, step 1: local luminance (3x3 binomial) and the
// vertical/horizontal and diagonal discriminations from high-pass energy.
fn m(x: i32, y: i32) -> f32 {
    let r = refl(y, i32(uu(U_H)));
    return srca[u32(r - ui(U_A_Y0)) * uu(U_W) + u32(refl(x, i32(uu(U_W))))];
}
fn hp(x: i32, y: i32, dx: i32, dy: i32) -> f32 {
    let a = abs(m(x - dx, y - dy) - m(x + dx, y + dy));
    let b = abs(2.0 * m(x, y) - m(x - 2 * dx, y - 2 * dy) - m(x + 2 * dx, y + 2 * dy));
    let g = a + 0.5 * b;
    return g * g;
}
fn stat(x: i32, y: i32, dx: i32, dy: i32) -> f32 {
    return hp(x - dx, y - dy, dx, dy) + hp(x, y, dx, dy) + hp(x + dx, y + dy, dx, dy);
}
fn run(x: i32, y: i32) {
    let lpf = 0.25 * m(x, y)
        + 0.125 * (m(x, y - 1) + m(x, y + 1) + m(x - 1, y) + m(x + 1, y))
        + 0.0625 * (m(x - 1, y - 1) + m(x + 1, y - 1) + m(x - 1, y + 1) + m(x + 1, y + 1));
    let e = 1e-10;
    let v = stat(x, y, 0, 1);
    let h = stat(x, y, 1, 0);
    let p = stat(x, y, 1, 1);
    let q = stat(x, y, -1, 1);
    dst[u32(y - ui(U_D_Y0)) * uu(U_W) + u32(x)] =
        vec4<f32>(m(x, y), lpf, (v + e) / (v + h + 2.0 * e), (p + e) / (p + q + 2.0 * e));
}

//@ pass rcd2
// Step 2: green at red and blue sites from the four neighbours, each scaled by the ratio of
// local luminances and weighted by inverse gradients; the discriminations refined.
fn a(x: i32, y: i32) -> vec4<f32> {
    let r = refl(y, i32(uu(U_H)));
    return srca[u32(r - ui(U_A_Y0)) * uu(U_W) + u32(refl(x, i32(uu(U_W))))];
}
fn disc(c: f32, n: f32) -> f32 {
    if (abs(0.5 - c) < abs(0.5 - n)) { return n; }
    return c;
}
fn est(x: i32, y: i32, dx: i32, dy: i32, lc: f32) -> f32 {
    let ln = max(a(x + 2 * dx, y + 2 * dy).y, 0.0);
    return a(x + dx, y + dy).x * (2.0 * lc + 1e-4) / (lc + ln + 1e-4);
}
fn grad(x: i32, y: i32, dx: i32, dy: i32) -> f32 {
    return 1e-5 + abs(a(x + dx, y + dy).x - a(x - dx, y - dy).x)
        + abs(a(x, y).x - a(x + 2 * dx, y + 2 * dy).x)
        + abs(a(x + dx, y + dy).x - a(x + 3 * dx, y + 3 * dy).x)
        + abs(a(x + 2 * dx, y + 2 * dy).x - a(x + 4 * dx, y + 4 * dy).x);
}
fn run(x: i32, y: i32) {
    let p = a(x, y);
    let d0 = a(x - 1, y - 1);
    let d1 = a(x + 1, y - 1);
    let d2 = a(x - 1, y + 1);
    let d3 = a(x + 1, y + 1);
    let vh = disc(p.z, 0.25 * (d0.z + d1.z + d2.z + d3.z));
    let pq = disc(p.w, 0.25 * (d0.w + d1.w + d2.w + d3.w));
    var g = p.x;
    if (ccolor(x, y) != 1u) {
        let lc = max(p.y, 0.0);
        let gn = grad(x, y, 0, -1);
        let gs = grad(x, y, 0, 1);
        let gw = grad(x, y, -1, 0);
        let ge = grad(x, y, 1, 0);
        let v = (gs * est(x, y, 0, -1, lc) + gn * est(x, y, 0, 1, lc)) / (gn + gs);
        let h = (ge * est(x, y, -1, 0, lc) + gw * est(x, y, 1, 0, lc)) / (gw + ge);
        g = v + (h - v) * vh;
    }
    dst[u32(y - ui(U_D_Y0)) * uu(U_W) + u32(x)] = vec4<f32>(p.x, g, vh, pq);
}

//@ pass rcd3
// Step 3: red at blue sites and blue at red sites, by colour difference along the two
// diagonals, weighted by inverse gradients and blended by the diagonal discrimination.
fn a(x: i32, y: i32) -> vec4<f32> {
    let r = refl(y, i32(uu(U_H)));
    return srca[u32(r - ui(U_A_Y0)) * uu(U_W) + u32(refl(x, i32(uu(U_W))))];
}
fn cd(x: i32, y: i32) -> f32 {
    let q = a(x, y);
    return q.x - q.y;
}
fn dgrad(x: i32, y: i32, dx: i32, dy: i32) -> f32 {
    return 1e-5 + abs(cd(x + dx, y + dy) - cd(x - dx, y - dy))
        + abs(cd(x + dx, y + dy) - cd(x + 3 * dx, y + 3 * dy))
        + abs(a(x, y).y - a(x + 2 * dx, y + 2 * dy).y);
}
fn dir(x: i32, y: i32, dx: i32, dy: i32) -> f32 {
    let g0 = dgrad(x, y, -dx, -dy);
    let g1 = dgrad(x, y, dx, dy);
    return (g1 * cd(x - dx, y - dy) + g0 * cd(x + dx, y + dy)) / (g0 + g1);
}
fn run(x: i32, y: i32) {
    let p = a(x, y);
    let c = ccolor(x, y);
    var rgb = vec3<f32>(0.0, p.y, 0.0);
    if (c != 1u) {
        let pe = dir(x, y, 1, 1);
        let qe = dir(x, y, -1, 1);
        let o = p.y + pe + (qe - pe) * p.w;
        if (c == 0u) { rgb = vec3<f32>(p.x, p.y, o); } else { rgb = vec3<f32>(o, p.y, p.x); }
    }
    dst[u32(y - ui(U_D_Y0)) * uu(U_W) + u32(x)] = vec4<f32>(rgb, p.z);
}

//@ pass rcd4
// Step 4: red and blue at green sites by colour difference from the four neighbours, blended
// by the vertical/horizontal discrimination; then the output transform.
fn a(x: i32, y: i32) -> vec4<f32> {
    let r = refl(y, i32(uu(U_H)));
    return srca[u32(r - ui(U_A_Y0)) * uu(U_W) + u32(refl(x, i32(uu(U_W))))];
}
fn cdc(x: i32, y: i32, k: u32) -> f32 {
    let q = a(x, y);
    return select(q.z, q.x, k == 0u) - q.y;
}
fn grd(x: i32, y: i32, dx: i32, dy: i32, k: u32) -> f32 {
    return 1e-5 + abs(a(x, y).y - a(x + 2 * dx, y + 2 * dy).y)
        + abs(cdc(x + dx, y + dy, k) - cdc(x - dx, y - dy, k))
        + abs(cdc(x + dx, y + dy, k) - cdc(x + 3 * dx, y + 3 * dy, k));
}
fn chan(x: i32, y: i32, k: u32, vh: f32) -> f32 {
    let gn = grd(x, y, 0, -1, k);
    let gs = grd(x, y, 0, 1, k);
    let gw = grd(x, y, -1, 0, k);
    let ge = grd(x, y, 1, 0, k);
    let v = (gs * cdc(x, y - 1, k) + gn * cdc(x, y + 1, k)) / (gn + gs);
    let h = (ge * cdc(x - 1, y, k) + gw * cdc(x + 1, y, k)) / (gw + ge);
    return a(x, y).y + v + (h - v) * vh;
}
fn run(x: i32, y: i32) {
    let p = a(x, y);
    var rgb = p.xyz;
    if (ccolor(x, y) == 1u) {
        rgb = vec3<f32>(chan(x, y, 0u, p.w), p.y, chan(x, y, 2u, p.w));
    }
    put(x, y, finish_full(rgb, u32(x), u32(y)));
}

//@ pass rcd4d
// Step 4 with the dual blend: the ratio-corrected directional result in detail, bilinear
// interpolation from the mosaic (source B) in flat areas. The blend weight is a smooth step
// of the local green contrast (four times the range of 3x3 means over a 3x3 window, over the sum of its
// ends) around the threshold.
fn a(x: i32, y: i32) -> vec4<f32> {
    let r = refl(y, i32(uu(U_H)));
    return srca[u32(r - ui(U_A_Y0)) * uu(U_W) + u32(refl(x, i32(uu(U_W))))];
}
fn m(x: i32, y: i32) -> f32 {
    let r = refl(y, i32(uu(U_H)));
    return srcb[u32(r - ui(U_B_Y0)) * uu(U_W) + u32(refl(x, i32(uu(U_W))))];
}
fn cdc(x: i32, y: i32, k: u32) -> f32 {
    let q = a(x, y);
    return select(q.z, q.x, k == 0u) - q.y;
}
fn grd(x: i32, y: i32, dx: i32, dy: i32, k: u32) -> f32 {
    return 1e-5 + abs(a(x, y).y - a(x + 2 * dx, y + 2 * dy).y)
        + abs(cdc(x + dx, y + dy, k) - cdc(x - dx, y - dy, k))
        + abs(cdc(x + dx, y + dy, k) - cdc(x + 3 * dx, y + 3 * dy, k));
}
fn chan(x: i32, y: i32, k: u32, vh: f32) -> f32 {
    let gn = grd(x, y, 0, -1, k);
    let gs = grd(x, y, 0, 1, k);
    let gw = grd(x, y, -1, 0, k);
    let ge = grd(x, y, 1, 0, k);
    let v = (gs * cdc(x, y - 1, k) + gn * cdc(x, y + 1, k)) / (gn + gs);
    let h = (ge * cdc(x - 1, y, k) + gw * cdc(x + 1, y, k)) / (gw + ge);
    return a(x, y).y + v + (h - v) * vh;
}
fn bilinear(x: i32, y: i32) -> vec3<f32> {
    let c = ccolor(x, y);
    let v = m(x, y);
    if (c == 1u) {
        let h = 0.5 * (m(x - 1, y) + m(x + 1, y));
        let w = 0.5 * (m(x, y - 1) + m(x, y + 1));
        if (ccolor(x + 1, y) == 0u) { return vec3<f32>(h, v, w); }
        return vec3<f32>(w, v, h);
    }
    let g = 0.25 * (m(x, y - 1) + m(x, y + 1) + m(x - 1, y) + m(x + 1, y));
    let o = 0.25 * (m(x - 1, y - 1) + m(x + 1, y - 1) + m(x - 1, y + 1) + m(x + 1, y + 1));
    if (c == 0u) { return vec3<f32>(v, g, o); }
    return vec3<f32>(o, g, v);
}
fn run(x: i32, y: i32) {
    let p = a(x, y);
    var rgb = p.xyz;
    if (ccolor(x, y) == 1u) {
        rgb = vec3<f32>(chan(x, y, 0u, p.w), p.y, chan(x, y, 2u, p.w));
    }
    let t = bitcast<f32>(uu(U_DUAL));
    if (t > 0.0) {
        // The range of the 3x3 means of green over a 3x3 window: noise averages out of the
        // means, structure does not.
        var lo = 3.0e38;
        var hi = -3.0e38;
        for (var dy = -1; dy <= 1; dy++) {
            for (var dx = -1; dx <= 1; dx++) {
                var g = 0.0;
                for (var j = -1; j <= 1; j++) {
                    for (var i = -1; i <= 1; i++) {
                        g += a(x + dx + i, y + dy + j).y;
                    }
                }
                lo = min(lo, g);
                hi = max(hi, g);
            }
        }
        let k = 4.0 * (hi - lo) / (abs(hi) + abs(lo) + 1e-3);
        let wr = smoothstep(0.5 * t, 1.5 * t, k);
        rgb = mix(bilinear(x, y), rgb, wr);
    }
    put(x, y, finish_full(rgb, u32(x), u32(y)));
}

//@ pass xt1
// X-Trans, step 1: green at red/blue sites along each of four lines (horizontal, vertical,
// both diagonals), through the line's greens within two pixels (Lagrange), limited to the
// nearest ones' range; the gradient-weighted mean of all green neighbours where a line has
// none on one side.
fn m(x: i32, y: i32) -> f32 {
    let r = clamp(y, 0, i32(uu(U_H)) - 1);
    return srca[u32(r - ui(U_A_Y0)) * uu(U_W) + u32(clamp(x, 0, i32(uu(U_W)) - 1))];
}
// Through the packed 6x6 table (xt_common.wgsl) when the pattern has one.
fn cc(x: i32, y: i32) -> u32 {
    return xc(clamp(x, 0, i32(uu(U_W)) - 1), clamp(y, 0, i32(uu(U_H)) - 1));
}
fn fallback(x: i32, y: i32) -> f32 {
    var sw = 0.0;
    var sv = 0.0;
    for (var k = 0; k < 8; k += 1) {
        var dy = 0;
        var dx = 0;
        var base = 2.0;
        switch k {
            case 0: { dy = 0; dx = -1; }
            case 1: { dy = 0; dx = 1; }
            case 2: { dy = -1; dx = 0; }
            case 3: { dy = 1; dx = 0; }
            case 4: { dy = -1; dx = -1; base = 1.0; }
            case 5: { dy = -1; dx = 1; base = 1.0; }
            case 6: { dy = 1; dx = -1; base = 1.0; }
            default: { dy = 1; dx = 1; base = 1.0; }
        }
        if (cc(x + dx, y + dy) != 1u) { continue; }
        let nb = m(x + dx, y + dy);
        var g = 0.0;
        if (cc(x - dx, y - dy) == 1u) {
            g = abs(nb - m(x - dx, y - dy));
        } else if (cc(x + 2 * dx, y + 2 * dy) == 1u) {
            g = 2.0 * abs(nb - m(x + 2 * dx, y + 2 * dy));
        } else {
            g = abs(nb - m(x, y));
        }
        let t = 0.125 + g;
        let w = base / (t * t);
        sw += w;
        sv += w * nb;
    }
    if (sw > 0.0) { return sv / sw; }
    return m(x, y);
}
fn step(k: i32) -> i32 { return select(select(select(2, 1, k == 2), -1, k == 1), -2, k == 0); }
fn run(x: i32, y: i32) {
    xt_load();
    let v0 = m(x, y);
    var g = vec4<f32>(v0);
    if (cc(x, y) != 1u) {
        var fb = -1.0;
        var have_fb = false;
        for (var l = 0; l < 4; l += 1) {
            var dy = 0;
            var dx = 0;
            switch l {
                case 0: { dy = 0; dx = 1; }
                case 1: { dy = 1; dx = 0; }
                case 2: { dy = 1; dx = 1; }
                default: { dy = 1; dx = -1; }
            }
            var mask = 0u;
            for (var k = 0; k < 4; k += 1) {
                if (cc(x + step(k) * dx, y + step(k) * dy) == 1u) { mask |= 1u << u32(k); }
            }
            var neg = -1;
            if ((mask & 2u) != 0u) { neg = 1; } else if ((mask & 1u) != 0u) { neg = 0; }
            var pos = -1;
            if ((mask & 4u) != 0u) { pos = 2; } else if ((mask & 8u) != 0u) { pos = 3; }
            var r = 0.0;
            if (neg >= 0 && pos >= 0) {
                var v = 0.0;
                for (var k = 0; k < 4; k += 1) {
                    if ((mask & (1u << u32(k))) == 0u) { continue; }
                    var w = 1.0;
                    let xk = f32(step(k));
                    for (var j = 0; j < 4; j += 1) {
                        if (j == k || (mask & (1u << u32(j))) == 0u) { continue; }
                        let xj = f32(step(j));
                        w *= -xj / (xk - xj);
                    }
                    v += w * m(x + step(k) * dx, y + step(k) * dy);
                }
                let va = m(x + step(neg) * dx, y + step(neg) * dy);
                let vb = m(x + step(pos) * dx, y + step(pos) * dy);
                r = clamp(v, min(va, vb), max(va, vb));
            } else {
                if (!have_fb) { fb = fallback(x, y); have_fb = true; }
                r = fb;
            }
            g[l] = r;
        }
    }
    dst[u32(y - ui(U_D_Y0)) * uu(U_W) + u32(x)] = g;
}

//@ pass xt2
// X-Trans, step 2: red and blue for each line's green by colour difference with the
// same-colour samples within two pixels (inverse squared distance, green similarity).
// The mosaic and step 1's greens are read once per 16x16 tile and two pixels around.
fn m(x: i32, y: i32) -> f32 {
    let r = clamp(y, 0, i32(uu(U_H)) - 1);
    return srca[u32(r - ui(U_A_Y0)) * uu(U_W) + u32(clamp(x, 0, i32(uu(U_W)) - 1))];
}
fn gr(x: i32, y: i32) -> vec4<f32> {
    let r = clamp(y, 0, i32(uu(U_H)) - 1);
    return srcb[u32(r - ui(U_B_Y0)) * uu(U_W) + u32(clamp(x, 0, i32(uu(U_W)) - 1))];
}
// Through the packed 6x6 table (xt_common.wgsl) when the pattern has one.
fn cc(x: i32, y: i32) -> u32 {
    return xc(clamp(x, 0, i32(uu(U_W)) - 1), clamp(y, 0, i32(uu(U_H)) - 1));
}
const XT2_T: u32 = 20u;
// The tile's mosaic and greens (each at its clamped image position).
var<workgroup> TM: array<f32, 400>;
var<workgroup> TG: array<vec4<f32>, 400>;
// Their colours (looked up once per tile position, not per tap).
var<workgroup> TC: array<u32, 400>;
@compute @workgroup_size(16, 16, 1)
fn main(@builtin(local_invocation_id) lid: vec3<u32>, @builtin(workgroup_id) wid: vec3<u32>) {
    xt_load();
    let bx = i32(wid.x * 16u) - 2;
    let by = i32(uu(U_Y0) + wid.y * 16u) - 2;
    for (var i = lid.y * 16u + lid.x; i < XT2_T * XT2_T; i += 256u) {
        let x = bx + i32(i % XT2_T);
        let y = by + i32(i / XT2_T);
        TM[i] = m(x, y);
        TG[i] = gr(x, y);
        TC[i] = cc(x, y);
    }
    workgroupBarrier();
    let gx = wid.x * 16u + lid.x;
    let gy = wid.y * 16u + lid.y;
    if (gx >= uu(U_W) || gy >= uu(U_ROWS)) { return; }
    let x = i32(gx);
    let y = i32(uu(U_Y0) + gy);
    // Tile index of (x, y): two pixels in from the tile's corner.
    let t0 = (lid.y + 2u) * XT2_T + lid.x + 2u;
    let c0 = TC[t0];
    let gi = TG[t0];
    var swr = vec4<f32>(0.0);
    var sdr = vec4<f32>(0.0);
    var swb = vec4<f32>(0.0);
    var sdb = vec4<f32>(0.0);
    for (var dy = -2; dy <= 2; dy += 1) {
        for (var dx = -2; dx <= 2; dx += 1) {
            if (dx == 0 && dy == 0) { continue; }
            let t = u32(i32(t0) + dy * i32(XT2_T) + dx);
            let c = TC[t];
            if (c == 1u || c == c0) { continue; }
            let inv = 1.0 / f32(dx * dx + dy * dy);
            let gq = TG[t];
            let s = vec4<f32>(1.0) + abs(gq - gi) * 8.0;
            let w = vec4<f32>(inv) / (s * s);
            let d = w * (vec4<f32>(TM[t]) - gq);
            if (c == 0u) { swr += w; sdr += d; } else { swb += w; sdb += d; }
        }
    }
    let v = TM[t0];
    var r = select(vec4<f32>(0.0), gi + sdr / swr, swr > vec4<f32>(0.0));
    var b = select(vec4<f32>(0.0), gi + sdb / swb, swb > vec4<f32>(0.0));
    if (c0 == 0u) { r = vec4<f32>(v); }
    if (c0 == 2u) { b = vec4<f32>(v); }
    let o = (u32(y - ui(U_D_Y0)) * uu(U_W) + u32(x)) * 3u;
    dst[o] = r;
    dst[o + 1u] = gi;
    dst[o + 2u] = b;
}

//@ pass xt3
// X-Trans, step 3: second derivative of each line's reconstruction along the line, in a
// cube-root opponent space. Each pixel's opponent colours (four lines) are made once per
// workgroup tile (16x16 and a pixel around) and shared: 1.27 instead of 3 cube roots per
// line and pixel.
fn rgbl(x: i32, y: i32, l: i32) -> vec3<f32> {
    let r = clamp(y, 0, i32(uu(U_H)) - 1);
    let o = (u32(r - ui(U_A_Y0)) * uu(U_W) + u32(clamp(x, 0, i32(uu(U_W)) - 1))) * 3u;
    return vec3<f32>(srca[o][l], srca[o + 1u][l], srca[o + 2u][l]);
}
fn pc(v: f32) -> f32 {
    if (v <= 0.0) { return 0.0; }
    return pow(v, 1.0 / 3.0);
}
fn opp(c: vec3<f32>) -> vec3<f32> {
    let r = pc(c.x);
    let g = pc(c.y);
    let b = pc(c.z);
    return vec3<f32>((r + 2.0 * g + b) * 0.25, r - g, b - g);
}
const XT3_T: u32 = 18u;
// The tile's opponent colours: pixel i, line l, component c at i * 12 + l * 3 + c.
var<workgroup> OP: array<f32, 3888>;
fn op(i: u32, l: u32) -> vec3<f32> {
    let k = i * 12u + l * 3u;
    return vec3<f32>(OP[k], OP[k + 1u], OP[k + 2u]);
}
@compute @workgroup_size(16, 16, 1)
fn main(@builtin(local_invocation_id) lid: vec3<u32>, @builtin(workgroup_id) wid: vec3<u32>) {
    let bx = i32(wid.x * 16u) - 1;
    let by = i32(uu(U_Y0) + wid.y * 16u) - 1;
    for (var i = lid.y * 16u + lid.x; i < XT3_T * XT3_T; i += 256u) {
        let x = bx + i32(i % XT3_T);
        let y = by + i32(i / XT3_T);
        for (var l = 0; l < 4; l += 1) {
            let o = opp(rgbl(x, y, l));
            let k = i * 12u + u32(l) * 3u;
            OP[k] = o.x;
            OP[k + 1u] = o.y;
            OP[k + 2u] = o.z;
        }
    }
    workgroupBarrier();
    let gx = wid.x * 16u + lid.x;
    let gy = wid.y * 16u + lid.y;
    if (gx >= uu(U_W) || gy >= uu(U_ROWS)) { return; }
    let x = i32(gx);
    let y = i32(uu(U_Y0) + gy);
    let w = i32(uu(U_W));
    let h = i32(uu(U_H));
    var d = vec4<f32>(0.0);
    for (var l = 0; l < 4; l += 1) {
        var dy = 0;
        var dx = 0;
        switch l {
            case 0: { dy = 0; dx = 1; }
            case 1: { dy = 1; dx = 0; }
            case 2: { dy = 1; dx = 1; }
            default: { dy = 1; dx = -1; }
        }
        let a = op(ti(x, y, bx, by, w, h), u32(l));
        let b = op(ti(x + dx, y + dy, bx, by, w, h), u32(l));
        let c = op(ti(x - dx, y - dy, bx, by, w, h), u32(l));
        let e = 2.0 * a - b - c;
        d[l] = dot(e, e);
    }
    dst[u32(y - ui(U_D_Y0)) * uu(U_W) + u32(x)] = d;
}
// The tile index of image pixel (x, y), clamped into the image as `rgbl` clamps: a clamped
// pixel's colours are its edge neighbour's, also in the tile.
fn ti(x: i32, y: i32, bx: i32, by: i32, w: i32, h: i32) -> u32 {
    let cx = clamp(x, 0, w - 1);
    let cy = clamp(y, 0, h - 1);
    return u32(cy - by) * XT3_T + u32(cx - bx);
}

//@ pass xt6
// X-Trans, steps 4 to 6 in one pass over 16x16 tiles (workgroup memory):
// 4. per line, how many of the 3x3 neighbours' derivatives (step 3, source A, read once
//    per tile) are within 8 times this pixel's smallest (the tile and two pixels around);
// 5. those counts summed over five columns (the tile's columns, its rows and two around);
// 6. summed over five rows; the mean of the lines scoring within 1/8 of the best (step 2's
//    colours, source B); then the output transform.
// Sums in the order the separate passes made them: the same numbers.
fn a(x: i32, y: i32) -> vec4<f32> {
    let r = clamp(y, 0, i32(uu(U_H)) - 1);
    return srca[u32(r - ui(U_A_Y0)) * uu(U_W) + u32(clamp(x, 0, i32(uu(U_W)) - 1))];
}
const XT6_T: u32 = 20u;
const XT6_S: u32 = 22u;
// Step 3 over the tile and three pixels around (22x22), at clamped image positions; then
// (reused) step 5 over the tile's columns and its rows with two around (20 rows of 16).
var<workgroup> S3: array<vec4<f32>, 484>;
// Step 4 over the tile and two pixels around (20x20), at clamped image positions.
var<workgroup> K4: array<vec4<f32>, 400>;
@compute @workgroup_size(16, 16, 1)
fn main(@builtin(local_invocation_id) lid: vec3<u32>, @builtin(workgroup_id) wid: vec3<u32>) {
    let w = i32(uu(U_W));
    let h = i32(uu(U_H));
    let bx = i32(wid.x * 16u) - 2;
    let by = i32(uu(U_Y0) + wid.y * 16u) - 2;
    let li = lid.y * 16u + lid.x;
    for (var i = li; i < XT6_S * XT6_S; i += 256u) {
        S3[i] = a(bx - 1 + i32(i % XT6_S), by - 1 + i32(i / XT6_S));
    }
    workgroupBarrier();
    for (var i = li; i < XT6_T * XT6_T; i += 256u) {
        let x = clamp(bx + i32(i % XT6_T), 0, w - 1);
        let y = clamp(by + i32(i / XT6_T), 0, h - 1);
        let c = S3[u32(y - by + 1) * XT6_S + u32(x - bx + 1)];
        let tr = 8.0 * min(min(c.x, c.y), min(c.z, c.w));
        var k = vec4<f32>(0.0);
        for (var dy = -1; dy <= 1; dy += 1) {
            let ny = clamp(y + dy, 0, h - 1);
            for (var dx = -1; dx <= 1; dx += 1) {
                let nx = clamp(x + dx, 0, w - 1);
                let v = S3[u32(ny - by + 1) * XT6_S + u32(nx - bx + 1)];
                k += select(vec4<f32>(0.0), vec4<f32>(1.0), v <= vec4<f32>(tr));
            }
        }
        K4[i] = k;
    }
    workgroupBarrier();
    for (var i = li; i < XT6_T * 16u; i += 256u) {
        let ty = i / 16u;
        let x = bx + 2 + i32(i % 16u);
        let y = clamp(by + i32(ty), 0, h - 1);
        var s = vec4<f32>(0.0);
        for (var dx = -2; dx <= 2; dx += 1) {
            let cx = clamp(x + dx, 0, w - 1);
            s += K4[u32(y - by) * XT6_T + u32(cx - bx)];
        }
        S3[ty * 16u + (i % 16u)] = s;
    }
    workgroupBarrier();
    let gx = wid.x * 16u + lid.x;
    let gy = wid.y * 16u + lid.y;
    if (gx >= uu(U_W) || gy >= uu(U_ROWS)) { return; }
    let x = i32(gx);
    let y = i32(uu(U_Y0) + gy);
    var s = vec4<f32>(0.0);
    for (var dy = -2; dy <= 2; dy += 1) {
        let cy = clamp(y + dy, 0, h - 1);
        s += S3[u32(cy - by) * 16u + lid.x];
    }
    let best = max(max(s.x, s.y), max(s.z, s.w));
    let fl = best - floor(best / 8.0);
    let o = (u32(y - ui(U_B_Y0)) * uu(U_W) + u32(x)) * 3u;
    let r = srcb[o];
    let g = srcb[o + 1u];
    let b = srcb[o + 2u];
    var acc = vec3<f32>(0.0);
    var n = 0.0;
    for (var l = 0; l < 4; l += 1) {
        if (s[l] >= fl) {
            acc += vec3<f32>(r[l], g[l], b[l]);
            n += 1.0;
        }
    }
    put(x, y, finish_full(acc / max(n, 1.0), u32(x), u32(y)));
}

//@ pass reduce
// A stripe of full-size output (srca, at the output precision, full-size rows from U_A_Y0, U[13] of them)
// area-reduced into output rows: the preview's reduction weights. U_W x U_H is the reduced
// size, U[8] x U[9] the full size.
fn run(x: i32, y: i32) {
    let sw = uu(8u);
    let sh = uu(9u);
    let r0 = uu(U_A_Y0);
    let r1 = r0 + uu(13u);
    let sx = f32(sw) / f32(uu(U_W));
    let sy = f32(sh) / f32(uu(U_H));
    let x0 = f32(x) * sx;
    let x1 = x0 + sx;
    let y0 = f32(y) * sy;
    let y1 = y0 + sy;
    var acc = vec4<f32>(0.0);
    var wsum = 0.0;
    let ys = max(u32(floor(y0)), r0);
    let ye = min(min(u32(ceil(y1)), sh), r1);
    let xe = min(u32(ceil(x1)), sw);
    for (var yy = ys; yy < ye; yy++) {
        let wy = min(f32(yy + 1u), y1) - max(f32(yy), y0);
        let row = (yy - r0) * sw;
        for (var xx = u32(floor(x0)); xx < xe; xx++) {
            let wx = min(f32(xx + 1u), x1) - max(f32(xx), x0);
            let w = wx * wy;
            if (w > 0.0) {
                acc += lda(row + xx) * w;
                wsum += w;
            }
        }
    }
    put(x, y, acc / max(wsum, 1e-12));
}
