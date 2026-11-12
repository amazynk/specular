// Raw front end: functions of the correction passes that read (sample, clip level)
// pairs through `ld` (`hot`, `hl`, `cfuse`).

// A position inside the domain: reflected on Bayer (keeps the phase), clamped otherwise.
fn cpos(x: i32, y: i32) -> vec2<i32> {
    let w = i32(uu(U_W));
    let h = i32(uu(U_H));
    if (du(H_LAYOUT) == 0u) { return vec2<i32>(refl(x, w), refl(y, h)); }
    return vec2<i32>(clamp(x, 0, w - 1), clamp(y, 0, h - 1));
}

// `ld(p)`: the (sample, clip level) pair at domain position p, from the pass's source
// (source A, or the fused pass's workgroup tiles).

// Hot and dead pixels: a sample above its brightest same-colour neighbour (within two
// pixels) by more than `threshold` times that neighbour, or below the darkest over
// 1 + threshold, becomes the neighbours' median.
fn hot_fix(x: i32, y: i32) -> vec2<f32> {
    let c0 = ld(vec2<i32>(x, y));
    let t = df(H_HOT);
    if (t <= 0.0) { return c0; }
    let col = ccolor(x, y);
    var n = 0u;
    var lo = 3.0e38;
    var hi = -3.0e38;
    for (var dy = -2; dy <= 2; dy += 1) {
        for (var dx = -2; dx <= 2; dx += 1) {
            let q = cpos(x + dx, y + dy);
            if ((q.x == x && q.y == y) || ccolor(q.x, q.y) != col) { continue; }
            let v = ld(q).x;
            n += 1u;
            lo = min(lo, v);
            hi = max(hi, v);
        }
    }
    if (n < 2u) { return c0; }
    let v = c0.x;
    let fl = 0.01 * c0.y;
    let hot = v - hi > t * max(hi, fl);
    let dead = v * (1.0 + t) < lo && lo > fl;
    if (!hot && !dead) { return c0; }
    // The neighbours gathered again only for a pixel to fix (the same values in the
    // same order): an indexed array costs every pixel otherwise.
    var vals: array<f32, 24>;
    n = 0u;
    for (var dy = -2; dy <= 2; dy += 1) {
        for (var dx = -2; dx <= 2; dx += 1) {
            let q = cpos(x + dx, y + dy);
            if ((q.x == x && q.y == y) || ccolor(q.x, q.y) != col) { continue; }
            vals[n] = ld(q).x;
            n += 1u;
        }
    }
    for (var i = 1u; i < n; i += 1u) {
        let k = vals[i];
        var j = i;
        while (j > 0u && vals[j - 1u] > k) {
            vals[j] = vals[j - 1u];
            j -= 1u;
        }
        vals[j] = k;
    }
    var med = vals[n / 2u];
    if ((n & 1u) == 0u) { med = 0.5 * (vals[n / 2u - 1u] + vals[n / 2u]); }
    return vec2<f32>(med, c0.y);
}

fn hl_gv(i: u32) -> vec3<f32> {
    return vec3<f32>(df(i), df(i + 1u), df(i + 2u));
}

// The chromaticity grid, bilinear at (x, y); neutral without one.
fn hl_chroma(x: i32, y: i32) -> vec3<f32> {
    let gw = du(H_HL_GW);
    let gh = du(H_HL_GH);
    if (gw == 0u || gh == 0u) { return vec3<f32>(1.0 / 3.0); }
    let ts = f32(du(H_HL_TS));
    let fx = clamp((f32(x) + 0.5) / ts - 0.5, 0.0, f32(gw - 1u));
    let fy = clamp((f32(y) + 0.5) / ts - 0.5, 0.0, f32(gh - 1u));
    let x0 = u32(fx);
    let y0 = u32(fy);
    let x1 = min(x0 + 1u, gw - 1u);
    let y1 = min(y0 + 1u, gh - 1u);
    let ax = fx - f32(x0);
    let ay = fy - f32(y0);
    let g = du(H_HL_GRID);
    let top = mix(hl_gv(g + (y0 * gw + x0) * 3u), hl_gv(g + (y0 * gw + x1) * 3u), ax);
    let bot = mix(hl_gv(g + (y1 * gw + x0) * 3u), hl_gv(g + (y1 * gw + x1) * 3u), ax);
    return mix(top, bot, ay);
}

// Rebuild: the area's chromaticity and the bilinear brightness boost, or w < 0 outside
// every area.
fn hl_rebuild(x: i32, y: i32) -> vec4<f32> {
    let bw = du(H_RB_BW);
    let bh = du(H_RB_BH);
    if (bw == 0u || bh == 0u) { return vec4<f32>(-1.0); }
    let bsu = du(H_RB_BS);
    let off = du(H_RB_OFF);
    let nx = min(u32(x) / bsu, bw - 1u);
    let ny = min(u32(y) / bsu, bh - 1u);
    let id = du(off + (ny * bw + nx) * 2u);
    if (id == 0u) { return vec4<f32>(-1.0); }
    let bs = f32(bsu);
    let fx = clamp((f32(x) + 0.5) / bs - 0.5, 0.0, f32(bw - 1u));
    let fy = clamp((f32(y) + 0.5) / bs - 0.5, 0.0, f32(bh - 1u));
    let x0 = u32(fx);
    let y0 = u32(fy);
    let x1 = min(x0 + 1u, bw - 1u);
    let y1 = min(y0 + 1u, bh - 1u);
    let ax = fx - f32(x0);
    let ay = fy - f32(y0);
    let top = mix(df(off + (y0 * bw + x0) * 2u + 1u), df(off + (y0 * bw + x1) * 2u + 1u), ax);
    let bot = mix(df(off + (y1 * bw + x0) * 2u + 1u), df(off + (y1 * bw + x1) * 2u + 1u), ax);
    let seg = du(H_RB_SEG) + (id - 1u) * 3u;
    return vec4<f32>(df(seg), df(seg + 1u), df(seg + 2u), mix(top, bot, ay));
}

// Highlights: a clipped sample from the unclipped colours around it and the chromaticity
// (see `ops::correct::highlights`).
fn hl_value(x: i32, y: i32) -> f32 {
    let c0 = ld(vec2<i32>(x, y));
    let mode = du(H_HL);
    let t = df(H_HL_T);
    let v = c0.x;
    if (mode == 0u) { return v; }
    if (mode == 1u) {
        // Clip: every sample at the lowest clip level around it (neutral highlights).
        var ms = 3.0e38;
        for (var dy = -1; dy <= 1; dy += 1) {
            for (var dx = -1; dx <= 1; dx += 1) {
                ms = min(ms, ld(cpos(x + dx, y + dy)).y);
            }
        }
        return min(v, t * ms);
    }
    if (v < t * c0.y) { return v; }
    var um = vec3<f32>(0.0);
    var un = vec3<f32>(0.0);
    var cm = vec3<f32>(0.0);
    for (var dy = -1; dy <= 1; dy += 1) {
        for (var dx = -1; dx <= 1; dx += 1) {
            let q = cpos(x + dx, y + dy);
            let s = ld(q);
            let k = ccolor(q.x, q.y);
            if (s.x >= t * s.y) {
                cm[k] = max(cm[k], s.x);
            } else {
                um[k] += s.x;
                un[k] += 1.0;
            }
        }
    }
    var chi = hl_chroma(x, y);
    var boost = 0.0;
    if (mode == 3u) {
        let rb = hl_rebuild(x, y);
        if (rb.w >= 0.0) {
            chi = rb.xyz;
            boost = rb.w;
        }
    }
    chi = max(chi, vec3<f32>(0.02));
    var su = 0.0;
    var sc = 0.0;
    for (var k = 0u; k < 3u; k += 1u) {
        if (un[k] > 0.0 && cm[k] == 0.0) {
            su += max(um[k] / un[k], 0.0);
            sc += chi[k];
        }
    }
    var s = 0.0;
    if (sc > 0.0) { s = su / sc; }
    for (var k = 0u; k < 3u; k += 1u) {
        if (cm[k] > 0.0) { s = max(s, cm[k] / chi[k]); }
    }
    s *= 1.0 + boost;
    let col = ccolor(x, y);
    return max(v, min(s * chi[col], 4.0 * c0.y));
}
