//! The WGSL of masks and blending: the mask program interpreter (`dv_mask`) and the blend
//! (`dv_blend`), helpers of every kernel with a masked node. [`super::cpu_ref`] mirrors
//! them for the tests.

use std::sync::LazyLock;

/// Reading the mask table, the coordinate grid, shape distances and the interpreter.
pub const MASK_WGSL: &str = "var<private> dv_detail: f32;
fn dv_mk_g(go: u32, n: u32, i: u32, j: u32) -> vec2<f32> {
    let o = go + 2u * (j * n + i);
    return vec2<f32>(mkf(o), mkf(o + 1u));
}
fn dv_mk_frame(px: i32, py: i32) -> vec2<f32> {
    let n = mku(1u);
    let go = mku(2u);
    let fx = clamp((f32(px) + 0.5) / f32(pu(4u)), 0.0, 1.0) * f32(n - 1u);
    let fy = clamp((f32(py) + 0.5) / f32(pu(5u)), 0.0, 1.0) * f32(n - 1u);
    let i0 = min(u32(floor(fx)), n - 2u);
    let j0 = min(u32(floor(fy)), n - 2u);
    let tx = fx - f32(i0);
    let ty = fy - f32(j0);
    let top = mix(dv_mk_g(go, n, i0, j0), dv_mk_g(go, n, i0 + 1u, j0), tx);
    let bot = mix(dv_mk_g(go, n, i0, j0 + 1u), dv_mk_g(go, n, i0 + 1u, j0 + 1u), tx);
    return mix(top, bot, ty);
}
fn dv_ss(e0: f32, e1: f32, x: f32) -> f32 {
    if (e1 <= e0) { return select(0.0, 1.0, x >= e0); }
    let t = clamp((x - e0) / (e1 - e0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}
fn dv_edge(d: f32, w: f32) -> f32 {
    return 1.0 - dv_ss(-w, 0.0, d);
}
fn dv_rot(d: vec2<f32>, cs: f32, sn: f32) -> vec2<f32> {
    return vec2<f32>(d.x * cs + d.y * sn, -d.x * sn + d.y * cs);
}
fn dv_sd_ellipse(q: vec2<f32>, r: vec2<f32>) -> f32 {
    let k1 = length(q / (r * r));
    if (k1 < 1e-12) { return -min(r.x, r.y); }
    let k0 = length(q / r);
    return k0 * (k0 - 1.0) / k1;
}
fn dv_sd_box(q: vec2<f32>, h: vec2<f32>, rr: f32) -> f32 {
    let d = abs(q) - h + vec2<f32>(rr);
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0) - rr;
}
fn dv_mk_tex(off: u32, stride: u32, w: u32, h: u32, x: i32, y: i32) -> f32 {
    if (x < 0 || y < 0 || x >= i32(w) || y >= i32(h)) { return 0.0; }
    let i = u32(y) * stride + u32(x);
    let word = mku(off + (i >> 1u));
    return f32((word >> ((i & 1u) * 16u)) & 0xFFFFu) / 65535.0;
}
fn dv_mk_brush(k: u32, f: vec2<f32>) -> f32 {
    let t = 8u + 4u * k;
    let off = mku(t);
    let w = mku(t + 1u);
    let h = mku(t + 2u);
    let stride = mku(t + 3u);
    let gx = f.x * f32(w) - 0.5;
    let gy = f.y * f32(h) - 0.5;
    if (gx < -1.0 || gy < -1.0 || gx > f32(w) || gy > f32(h)) { return 0.0; }
    let x0 = floor(gx);
    let y0 = floor(gy);
    let tx = gx - x0;
    let ty = gy - y0;
    let xi = i32(x0);
    let yi = i32(y0);
    let top = mix(dv_mk_tex(off, stride, w, h, xi, yi), dv_mk_tex(off, stride, w, h, xi + 1, yi), tx);
    let bot = mix(dv_mk_tex(off, stride, w, h, xi, yi + 1), dv_mk_tex(off, stride, w, h, xi + 1, yi + 1), tx);
    return mix(top, bot, ty);
}
fn dv_mk_chan(ch: u32, c: vec3<f32>) -> f32 {
    if (ch == 0u) { return log2(max(dot(c, DV_LUMA), 1.52587890625e-5) / 0.18); }
    if (ch == 1u) { let l = dv_oklab(c); return length(l.yz); }
    if (ch == 2u) {
        let l = dv_oklab(c);
        var h = degrees(atan2(l.z, l.y));
        if (h < 0.0) { h += 360.0; }
        return h;
    }
    if (ch == 3u) { return c.x; }
    if (ch == 4u) { return c.y; }
    return c.z;
}
fn dv_mk_poly(pc: u32, p: vec2<f32>) -> vec2<f32> {
    let n = mku(pc + 1u);
    let b = pc + 2u;
    var v0 = vec2<f32>(mkf(b), mkf(b + 1u));
    var dmin = dot(p - v0, p - v0);
    var fw = mkf(b + 2u);
    var s = 1.0;
    var j = n - 1u;
    for (var i = 0u; i < n; i++) {
        let oi = b + 3u * i;
        let oj = b + 3u * j;
        let vi = vec2<f32>(mkf(oi), mkf(oi + 1u));
        let vj = vec2<f32>(mkf(oj), mkf(oj + 1u));
        let e = vj - vi;
        let w = p - vi;
        let t = clamp(dot(w, e) / max(dot(e, e), 1e-20), 0.0, 1.0);
        let q = w - e * t;
        let d = dot(q, q);
        if (d < dmin) {
            dmin = d;
            fw = mix(mkf(oi + 2u), mkf(oj + 2u), t);
        }
        let c0 = p.y >= vi.y;
        let c1 = p.y < vj.y;
        let c2 = e.x * w.y > e.y * w.x;
        if ((c0 && c1 && c2) || (!c0 && !c1 && !c2)) { s = -s; }
        j = i;
    }
    return vec2<f32>(s * sqrt(dmin), fw);
}
fn dv_mask(prog: u32, a: vec3<f32>, b: vec3<f32>, px: i32, py: i32) -> f32 {
    let f = dv_mk_frame(px, py);
    let p = f * vec2<f32>(mkf(3u), mkf(4u));
    let aa = mkf(5u);
    var st: array<f32, 8>;
    var sp = 0u;
    var pc = prog;
    for (var guard = 0u; guard < 4096u; guard++) {
        let op = mku(pc);
        if (op == 0u || sp > 8u) { break; }
        var v = 0.0;
        var leaf = true;
        switch op {
            case 1u: { v = 1.0; pc += 1u; }
            case 2u: {
                let q = dv_rot(p - vec2<f32>(mkf(pc + 1u), mkf(pc + 2u)), mkf(pc + 5u), mkf(pc + 6u));
                let d = dv_sd_ellipse(q, vec2<f32>(mkf(pc + 3u), mkf(pc + 4u)));
                v = dv_edge(d, max(mkf(pc + 7u), aa));
                pc += 8u;
            }
            case 3u: {
                let q = dv_rot(p - vec2<f32>(mkf(pc + 1u), mkf(pc + 2u)), mkf(pc + 5u), mkf(pc + 6u));
                let d = dv_sd_box(q, vec2<f32>(mkf(pc + 3u), mkf(pc + 4u)), mkf(pc + 7u));
                v = dv_edge(d, max(mkf(pc + 8u), aa));
                pc += 9u;
            }
            case 4u: {
                let r = dv_mk_poly(pc, p);
                v = dv_edge(r.x, max(r.y, aa));
                pc += 2u + 3u * mku(pc + 1u);
            }
            case 5u: {
                let s0 = vec2<f32>(mkf(pc + 1u), mkf(pc + 2u));
                let e = vec2<f32>(mkf(pc + 3u), mkf(pc + 4u)) - s0;
                let ee = dot(e, e);
                if (ee < 1e-20) {
                    v = select(0.0, 1.0, dot(p - s0, p - s0) < 1e-20);
                } else {
                    v = 1.0 - dv_ss(0.0, 1.0, dot(p - s0, e) / ee);
                }
                pc += 5u;
            }
            case 6u: {
                let q = dv_rot(p - vec2<f32>(mkf(pc + 1u), mkf(pc + 2u)), mkf(pc + 3u), mkf(pc + 4u));
                let ri = length(q / vec2<f32>(mkf(pc + 5u), mkf(pc + 6u)));
                let ro = length(q / vec2<f32>(mkf(pc + 7u), mkf(pc + 8u)));
                if (ri <= 1.0) {
                    v = 1.0;
                } else if (ro >= 1.0) {
                    v = 0.0;
                } else {
                    let t = (1.0 - 1.0 / ri) / max(1.0 / ro - 1.0 / ri, 1e-20);
                    v = 1.0 - dv_ss(0.0, 1.0, t);
                }
                pc += 9u;
            }
            case 7u: { v = dv_mk_brush(mku(pc + 1u), f); pc += 2u; }
            case 8u: {
                let ch = mku(pc + 1u);
                let c = select(a, b, mku(pc + 2u) != 0u);
                let l0 = mkf(pc + 3u);
                var x = dv_mk_chan(ch, c);
                if (ch == 2u) { x = l0 + (((x - l0) % 360.0) + 360.0) % 360.0; }
                v = dv_ss(l0, mkf(pc + 4u), x) * (1.0 - dv_ss(mkf(pc + 5u), mkf(pc + 6u), x));
                pc += 7u;
            }
            case 9u: {
                let n = mku(pc + 1u);
                let lab = dv_oklab(a);
                var d = 1e30;
                for (var i = 0u; i < n; i++) {
                    let o = pc + 4u + 3u * i;
                    d = min(d, length(lab - vec3<f32>(mkf(o), mkf(o + 1u), mkf(o + 2u))));
                }
                let r = mkf(pc + 2u);
                v = 1.0 - dv_ss(r, r + mkf(pc + 3u), d);
                pc += 4u + 3u * n;
            }
            case 16u: { v = 0.0; pc += 1u; }
            case 19u: {
                let t = dv_mk_brush(mku(pc + 1u), f);
                let lo = mkf(pc + 2u);
                let hi = mkf(pc + 3u);
                let fe = mkf(pc + 4u);
                var wa = 1.0;
                if (lo > 0.0) { wa = dv_ss(lo - fe, lo, t); }
                var wb = 1.0;
                if (hi < 1.0) { wb = 1.0 - dv_ss(hi, hi + fe, t); }
                v = wa * wb;
                pc += 5u;
            }
            case 17u: {
                let hs = 0.5 * mkf(pc + 2u);
                v = dv_ss(mkf(pc + 1u) - hs, mkf(pc + 1u) + hs, dv_detail);
                pc += 3u;
            }
            default: {
                leaf = false;
                if (op == 10u && sp >= 1u) {
                    st[sp - 1u] = 1.0 - st[sp - 1u];
                    pc += 1u;
                } else if (op == 15u && sp >= 1u) {
                    st[sp - 1u] = st[sp - 1u] * mkf(pc + 1u);
                    pc += 2u;
                } else if (op == 18u && sp >= 1u) {
                    st[sp - 1u] = clamp(
                        (st[sp - 1u] - 0.5) * (1.0 + mkf(pc + 1u)) + 0.5 + mkf(pc + 2u),
                        0.0,
                        1.0,
                    );
                    pc += 3u;
                } else if (op >= 11u && op <= 14u && sp >= 2u) {
                    let y = st[sp - 1u];
                    let x = st[sp - 2u];
                    var r = 0.0;
                    if (op == 11u) { r = max(x, y); }
                    if (op == 12u) { r = min(x, y); }
                    if (op == 13u) { r = x * (1.0 - y); }
                    if (op == 14u) { r = abs(x - y); }
                    sp -= 1u;
                    st[sp - 1u] = r;
                    pc += 1u;
                } else {
                    sp = 9u;
                }
            }
        }
        if (leaf && sp < 8u) {
            st[sp] = v;
            sp += 1u;
        }
    }
    if (sp == 0u || sp > 8u) { return 0.0; }
    return clamp(st[0], 0.0, 1.0);
}
";

/// Blend modes and `dv_blend(a, b, px, py, q)`: the module's input `a`, its output `b`, the
/// blend's words at `q`.
pub const BLEND_WGSL: &str = "fn dv_enc(x: vec3<f32>) -> vec3<f32> {
    let v = max(x, vec3<f32>(0.0));
    return v / (v + vec3<f32>(0.18));
}
fn dv_dec(y: vec3<f32>) -> vec3<f32> {
    let v = clamp(y, vec3<f32>(0.0), vec3<f32>(0.9999));
    return 0.18 * v / (vec3<f32>(1.0) - v);
}
fn dv_softl(a: f32, b: f32) -> f32 {
    if (b <= 0.5) { return a - (1.0 - 2.0 * b) * a * (1.0 - a); }
    var d = sqrt(a);
    if (a <= 0.25) { d = ((16.0 * a - 12.0) * a + 4.0) * a; }
    return a + (2.0 * b - 1.0) * (d - a);
}
fn dv_ovl(a: f32, b: f32) -> f32 {
    if (a < 0.5) { return 2.0 * a * b; }
    return 1.0 - 2.0 * (1.0 - a) * (1.0 - b);
}
fn dv_mode(mode: u32, a: vec3<f32>, b: vec3<f32>) -> vec3<f32> {
    if (mode == 0u) { return b; }
    let ea = dv_enc(a);
    let eb = dv_enc(b);
    switch mode {
        case 1u: { return dv_dec(ea * eb); }
        case 2u: { return dv_dec(ea / max(eb, vec3<f32>(1e-6))); }
        case 3u: { return dv_dec(ea + eb); }
        case 4u: { return dv_dec(max(ea - eb, vec3<f32>(0.0))); }
        case 5u: { return max(a, b); }
        case 6u: { return min(a, b); }
        case 7u: {
            let ya = dot(a, DV_LUMA);
            if (ya <= 1e-9) { return b; }
            return a * (dot(b, DV_LUMA) / ya);
        }
        case 8u: {
            let yb = dot(b, DV_LUMA);
            if (yb <= 1e-9) { return a; }
            return b * (dot(a, DV_LUMA) / yb);
        }
        case 9u, 10u: {
            let la = dv_oklab(a);
            let lb = dv_oklab(b);
            let ca = length(la.yz);
            let cb = length(lb.yz);
            if (mode == 9u) {
                if (cb < 1e-9) { return a; }
                return dv_oklab_inv(vec3<f32>(la.x, lb.yz * (ca / cb)));
            }
            if (ca < 1e-9) { return a; }
            return dv_oklab_inv(vec3<f32>(la.x, la.yz * (cb / ca)));
        }
        case 11u: { return dv_dec(vec3<f32>(1.0) - (vec3<f32>(1.0) - ea) * (vec3<f32>(1.0) - eb)); }
        case 12u: { return dv_dec(vec3<f32>(dv_ovl(ea.x, eb.x), dv_ovl(ea.y, eb.y), dv_ovl(ea.z, eb.z))); }
        case 13u: { return dv_dec(vec3<f32>(dv_softl(ea.x, eb.x), dv_softl(ea.y, eb.y), dv_softl(ea.z, eb.z))); }
        default: { return b; }
    }
}
fn dv_blend(a: vec4<f32>, b: vec4<f32>, px: i32, py: i32, q: u32) -> vec4<f32> {
    let flags = pu(q + 6u);
    var m = 0.0;
    if ((flags & 1u) != 0u) {
        m = clamp(a.w, 0.0, 1.0);
    } else {
        m = dv_mask(pu(q + 2u), a.rgb, b.rgb, px, py);
    }
    // Untouched when both are 0, so a borrowed mask matches its source bit for bit.
    if (pf(q + 4u) != 0.0 || pf(q + 5u) != 0.0) {
        m = clamp((m - 0.5) * (1.0 + pf(q + 4u)) + 0.5 + pf(q + 5u), 0.0, 1.0);
    }
    if (pu(q + 3u) != 0u) { m = 1.0 - m; }
    let k = m * pf(q + 1u);
    let r = mix(a.rgb, dv_mode(pu(q), a.rgb, b.rgb), k);
    var w = b.w;
    if ((flags & 1u) != 0u) { w = 1.0; }
    if ((flags & 2u) != 0u) { w = m; }
    return vec4<f32>(r, w);
}
";

/// Every helper a kernel with a masked node needs, in order.
pub static HELPERS: LazyLock<[&'static str; 4]> = LazyLock::new(|| {
    [
        crate::ops::tone::kernels::COMMON_WGSL,
        *crate::colour::oklab::OKLAB_WGSL,
        MASK_WGSL,
        BLEND_WGSL,
    ]
});
