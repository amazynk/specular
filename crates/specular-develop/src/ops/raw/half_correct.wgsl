// Half size with corrections: the per-pixel steps, shared by the three-pass path
// (`hcell`, `hhot`, `hout`) and the fused passes (`hfuse`, `hfuseb`).

// Step 1: per output pixel the colours of the `half` window (calibrated), a clipped colour
// stored as -1 - v, and the lowest clip level in w.
//@ fn hcell
fn hcell_v(x: i32, y: i32) -> vec4<f32> {
    let cw = i32(du(H_CW));
    let ch = i32(du(H_CH));
    let t = df(H_HL_T);
    var sum = vec3<f32>(0.0);
    var n = vec3<f32>(0.0);
    var clipped = vec3<f32>(0.0);
    var minsat = 3.0e38;
    var r0 = 0;
    var r1 = 2;
    if (du(H_LAYOUT) == 1u) { r0 = -1; r1 = 3; }
    for (var dy = r0; dy < r1; dy += 1) {
        for (var dx = r0; dx < r1; dx += 1) {
            let sx = clamp(2 * x + dx, 0, cw - 1);
            let sy = clamp(2 * y + dy, 0, ch - 1);
            let k = ccolor(sx, sy);
            let vs = prep2(sx, sy);
            let v = vs.x;
            let sat = vs.y;
            minsat = min(minsat, sat);
            // Per channel by selection, not a dynamic index (which spills to memory).
            let m = vec3<u32>(0u, 1u, 2u) == vec3<u32>(k);
            if (v >= t * sat) { clipped = select(clipped, vec3<f32>(1.0), m); }
            sum += select(vec3<f32>(0.0), vec3<f32>(v), m);
            n += select(vec3<f32>(0.0), vec3<f32>(1.0), m);
        }
    }
    var c = sum / max(n, vec3<f32>(1.0));
    for (var k = 0u; k < 3u; k += 1u) {
        if (clipped[k] > 0.0) { c[k] = -1.0 - max(c[k], 0.0); }
    }
    return vec4<f32>(c, minsat);
}

// The fused passes' step 1 for the workgroup's cells (`HT`, its origin `ht_x0`, `ht_y0`),
// then a barrier.
//@ fn htfill
fn ht_fill(li: u32, w: i32, h: i32) {
    for (var i = li; i < HT_W * HT_W; i += 256u) {
        let cx = clamp(ht_x0 + i32(i % HT_W), 0, w - 1);
        let cy = clamp(ht_y0 + i32(i / HT_W), 0, h - 1);
        HT[i] = hcell_v(cx, cy);
    }
    workgroupBarrier();
}

// The same for X-Trans (4x4 windows overlapping by half): the prepared samples of nine
// cell rows at a time are read once into workgroup memory (`XS`) and step 1 sums them
// from there, in `hcell_v`'s order, so the cells are the same bit for bit. The window
// covers the clamped cells' samples (at most 2 * 17 + 4 columns, 2 * 8 + 4 rows).
//@ fn htfillx
const XS_W: u32 = 38u;
const XS_H: u32 = 20u;
var<workgroup> XS: array<vec2<f32>, 760>;
fn hcell_xs(x: i32, y: i32, sx0: i32, sy0: i32) -> vec4<f32> {
    let cw = i32(du(H_CW));
    let ch = i32(du(H_CH));
    let t = df(H_HL_T);
    var sum = vec3<f32>(0.0);
    var n = vec3<f32>(0.0);
    var clipped = vec3<f32>(0.0);
    var minsat = 3.0e38;
    for (var dy = -1; dy < 3; dy += 1) {
        for (var dx = -1; dx < 3; dx += 1) {
            let sx = clamp(2 * x + dx, 0, cw - 1);
            let sy = clamp(2 * y + dy, 0, ch - 1);
            let k = ccolor(sx, sy);
            let vs = XS[u32(sy - sy0) * XS_W + u32(sx - sx0)];
            let v = vs.x;
            let sat = vs.y;
            minsat = min(minsat, sat);
            let m = vec3<u32>(0u, 1u, 2u) == vec3<u32>(k);
            if (v >= t * sat) { clipped = select(clipped, vec3<f32>(1.0), m); }
            sum += select(vec3<f32>(0.0), vec3<f32>(v), m);
            n += select(vec3<f32>(0.0), vec3<f32>(1.0), m);
        }
    }
    var c = sum / max(n, vec3<f32>(1.0));
    for (var k = 0u; k < 3u; k += 1u) {
        if (clipped[k] > 0.0) { c[k] = -1.0 - max(c[k], 0.0); }
    }
    return vec4<f32>(c, minsat);
}
fn ht_fill(li: u32, w: i32, h: i32) {
    let cw = i32(du(H_CW));
    let ch = i32(du(H_CH));
    let sx0 = 2 * clamp(ht_x0, 0, w - 1) - 1;
    for (var part = 0u; part < 2u; part += 1u) {
        let r0 = part * 9u;
        let sy0 = 2 * clamp(ht_y0 + i32(r0), 0, h - 1) - 1;
        for (var i = li; i < XS_W * XS_H; i += 256u) {
            let sx = clamp(sx0 + i32(i % XS_W), 0, cw - 1);
            let sy = clamp(sy0 + i32(i / XS_W), 0, ch - 1);
            XS[i] = prep2(sx, sy);
        }
        workgroupBarrier();
        for (var i = li; i < HT_W * 9u; i += 256u) {
            let cx = clamp(ht_x0 + i32(i % HT_W), 0, w - 1);
            let cy = clamp(ht_y0 + i32(r0 + i / HT_W), 0, h - 1);
            HT[r0 * HT_W + i] = hcell_xs(cx, cy, sx0, sy0);
        }
        workgroupBarrier();
    }
}

// Step 2: hot pixels against the same colour of the eight neighbouring pixels (`h4`, the
// step 1 values, defined by each pass), then highlights per pixel (as the full-size `hl`,
// with the pixel's own colours) at its place on the sensor.
//@ fn hhot
fn hhot_v(x: i32, y: i32) -> vec4<f32> {
    let c0 = h4(x, y);
    var v = vec3<f32>(0.0);
    var cl = vec3<f32>(0.0);
    for (var k = 0u; k < 3u; k += 1u) {
        if (c0[k] < 0.0) {
            v = with3(v, k, -1.0 - c0[k]);
            cl = with3(cl, k, 1.0);
        } else {
            v = with3(v, k, c0[k]);
        }
    }
    let t = df(H_HOT);
    if (t > 0.0) {
        for (var k = 0u; k < 3u; k += 1u) {
            var vals: array<f32, 8>;
            var lo = 3.0e38;
            var hi = -3.0e38;
            var m = 0u;
            for (var dy = -1; dy <= 1; dy += 1) {
                for (var dx = -1; dx <= 1; dx += 1) {
                    if (dx == 0 && dy == 0) { continue; }
                    var q = h4(x + dx, y + dy)[k];
                    if (q < 0.0) { q = -1.0 - q; }
                    vals[m] = q;
                    m += 1u;
                    lo = min(lo, q);
                    hi = max(hi, q);
                }
            }
            let fl = 0.01 * df(H_WB + k);
            let hot = v[k] - hi > t * max(hi, fl);
            let dead = v[k] * (1.0 + t) < lo && lo > fl;
            if (hot || dead) {
                for (var i = 1u; i < 8u; i += 1u) {
                    let e = vals[i];
                    var j = i;
                    while (j > 0u && vals[j - 1u] > e) {
                        vals[j] = vals[j - 1u];
                        j -= 1u;
                    }
                    vals[j] = e;
                }
                v = with3(v, k, 0.5 * (vals[3] + vals[4]));
                cl = with3(cl, k, 0.0);
            }
        }
    }
    let mode = du(H_HL);
    let ht = df(H_HL_T);
    let minsat = c0.w;
    if (mode == 1u) {
        v = min(v, vec3<f32>(ht * minsat));
    } else if (mode >= 2u && max(cl.x, max(cl.y, cl.z)) > 0.0) {
        // Sensor (crop) position of the pixel's centre.
        let px = 2 * x + 1;
        let py = 2 * y + 1;
        var chi = hl_chroma(px, py);
        var boost = 0.0;
        if (mode == 3u) {
            let rb = hl_rebuild(px, py);
            if (rb.w >= 0.0) {
                chi = rb.xyz;
                boost = rb.w;
            }
        }
        chi = max(chi, vec3<f32>(0.02));
        var su = 0.0;
        var sc = 0.0;
        var s = 0.0;
        for (var k = 0u; k < 3u; k += 1u) {
            if (cl[k] == 0.0) {
                su += max(v[k], 0.0);
                sc += chi[k];
            } else {
                s = max(s, v[k] / chi[k]);
            }
        }
        if (sc > 0.0) { s = max(s, su / sc); }
        s *= 1.0 + boost;
        let wbmin = min(df(H_WB), min(df(H_WB + 1u), df(H_WB + 2u)));
        for (var k = 0u; k < 3u; k += 1u) {
            if (cl[k] > 0.0) {
                let cap = 4.0 * minsat * df(H_WB + k) / max(wbmin, 1e-6);
                v = with3(v, k, max(v[k], min(s * chi[k], cap)));
            }
        }
    }
    return vec4<f32>(v, minsat);
}
