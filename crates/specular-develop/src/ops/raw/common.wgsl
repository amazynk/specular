// Raw front end: functions shared by every pass (see `ops::raw` for the bindings, the
// uniform words and the layout of the data buffer D).

fn uu(i: u32) -> u32 { return U[i >> 2u][i & 3u]; }
fn ui(i: u32) -> i32 { return bitcast<i32>(uu(i)); }
fn du(i: u32) -> u32 { return D[i]; }
fn df(i: u32) -> f32 { return bitcast<f32>(D[i]); }

// `v` with component `k` set to `x`. Shaders write a vector's component this way, not as
// `v[k] = x`: with an index known only at run time, FXC (the Direct3D 12 shader compiler wgpu
// uses by default) cannot compile that store ("not natively addressable") and the pipeline is
// invalid on Windows. The result is the same, component for component.
fn with3(v: vec3<f32>, k: u32, x: f32) -> vec3<f32> {
    return select(v, vec3<f32>(x), vec3<u32>(0u, 1u, 2u) == vec3<u32>(k));
}
fn with4(v: vec4<f32>, k: u32, x: f32) -> vec4<f32> {
    return select(v, vec4<f32>(x), vec4<u32>(0u, 1u, 2u, 3u) == vec4<u32>(k));
}

// Uniform words.
const U_Y0: u32 = 0u;      // first row this dispatch computes (domain rows)
const U_ROWS: u32 = 1u;    // rows it computes
const U_W: u32 = 2u;       // domain width
const U_H: u32 = 3u;       // domain height
const U_A_Y0: u32 = 4u;    // domain row of row 0 of source A
const U_B_Y0: u32 = 5u;    // ... of source B
const U_D_Y0: u32 = 6u;    // ... of the destination
const U_D_BASE: u32 = 7u;  // element skip of the destination view
const U_RAW_BASE: u32 = 8u; // sample skip of the raw view
const U_RAW_ROW0: u32 = 9u; // stored row of row 0 of the raw view
const U_RAW_ROWS: u32 = 10u; // rows of the raw view
const U_DUAL: u32 = 11u;   // dual demosaic contrast threshold (f32; 0 = off)
const U_PREPARED: u32 = 12u; // 1: RAW holds the corrected mosaic (f32, crop rows)
const U_CAL_BASE: u32 = 13u; // word skip of the calibration view
const U_CAL_ROW0: u32 = 14u; // crop row of row 0 of the calibration view

// Data buffer header words.
const H_LAYOUT: u32 = 0u;
const H_CFA_W: u32 = 1u;
const H_CFA_H: u32 = 2u;
const H_CFA_OFF: u32 = 3u;
const H_BL_ROWS: u32 = 4u;
const H_BL_COLS: u32 = 5u;
const H_BL_CPP: u32 = 6u;
const H_BL_OFF: u32 = 7u;
const H_BL_OX: u32 = 8u;
const H_BL_OY: u32 = 9u;
const H_BC_OFF: u32 = 10u;
const H_BC_LEN: u32 = 11u;
const H_BR_OFF: u32 = 12u;
const H_BR_LEN: u32 = 13u;
const H_LIN_OFF: u32 = 14u;
const H_LIN_LEN: u32 = 15u;
const H_WHITE: u32 = 16u;
const H_WB: u32 = 17u;
const H_L1_OFF: u32 = 20u;
const H_L1_N: u32 = 21u;
const H_L2_OFF: u32 = 22u;
const H_L2_N: u32 = 23u;
const H_L3_OFF: u32 = 24u;
const H_L3_N: u32 = 25u;
const H_MAT: u32 = 26u;
const H_PROF: u32 = 35u;
const H_W: u32 = 36u;
const H_CPP: u32 = 38u;
const H_CX: u32 = 39u;
const H_CY: u32 = 40u;
const H_CW: u32 = 41u;
const H_CH: u32 = 42u;
const H_BAD: u32 = 43u;
const H_BAD_V: u32 = 44u;
const H_BAD_RECT: u32 = 45u;
const H_OUTPUT: u32 = 49u;
// Corrections (see `ops::raw::correct`).
const H_CLIP: u32 = 50u;
const H_CAL: u32 = 51u;
const H_HOT: u32 = 52u;
const H_HL: u32 = 53u;
const H_HL_T: u32 = 54u;
const H_HL_GRID: u32 = 55u;
const H_HL_GW: u32 = 56u;
const H_HL_GH: u32 = 57u;
const H_HL_TS: u32 = 58u;
const H_RB_OFF: u32 = 59u;
const H_RB_BW: u32 = 60u;
const H_RB_BH: u32 = 61u;
const H_RB_BS: u32 = 62u;
const H_RB_SEG: u32 = 63u;
const H_CA: u32 = 64u;
const H_CA_CX: u32 = 65u;
const H_CA_CY: u32 = 66u;
const H_CA_R: u32 = 67u;
const H_CA_RA: u32 = 68u;
const H_CA_RB: u32 = 69u;
const H_CA_BA: u32 = 70u;
const H_CA_BB: u32 = 71u;
const H_CS: u32 = 72u;
const H_CELLS_W: u32 = 73u;
const H_CELLS_H: u32 = 74u;
const H_FLOAT: u32 = 75u;
const H_XT6: u32 = 76u;
const H_XT6_ON: u32 = 79u;

const REC: u32 = 24u;

// ---- Sensor samples -------------------------------------------------------------------------

fn raw_at(sx: u32, sy: u32, c: u32) -> u32 {
    let cpp = du(H_CPP);
    let i = uu(U_RAW_BASE) + ((sy - uu(U_RAW_ROW0)) * du(H_W) + sx) * cpp + c;
    let w = RAW[i >> 1u];
    return select(w & 0xffffu, w >> 16u, (i & 1u) == 1u);
}

// Colour (0 red, 1 green, 2 blue) at stored position (sx, sy).
fn color_at(sx: u32, sy: u32) -> u32 {
    let l = du(H_LAYOUT);
    if (l == 0u) {
        return du(du(H_CFA_OFF) + (sy & 1u) * 2u + (sx & 1u));
    }
    if (l == 1u) {
        let cw = du(H_CFA_W);
        let ch = du(H_CFA_H);
        return du(du(H_CFA_OFF) + (sy % ch) * cw + sx % cw);
    }
    return 1u;
}

// Colour at domain (crop) position: a 6x6 pattern (X-Trans) through its packed table
// (H_XT6, already in the crop's phase), without divisions by the pattern's size.
fn ccolor(x: i32, y: i32) -> u32 {
    if (du(H_XT6_ON) == 1u) {
        let rx = u32((x % 6 + 6) % 6);
        let ry = u32((y % 6 + 6) % 6);
        return (du(H_XT6 + ry / 2u) >> ((ry & 1u) * 12u + rx * 2u)) & 3u;
    }
    return color_at(du(H_CX) + u32(x), du(H_CY) + u32(y));
}

fn black_at(sx: u32, sy: u32, c: u32) -> f32 {
    let rows = du(H_BL_ROWS);
    let cols = du(H_BL_COLS);
    let cpp = du(H_BL_CPP);
    let ox = i32(du(H_BL_OX));
    let oy = i32(du(H_BL_OY));
    var r = 0u;
    var k = 0u;
    // Power-of-two repeats (1 and 2, nearly always) by a mask: the same
    // non-negative remainder without two integer divisions per sample.
    if ((rows & (rows - 1u)) == 0u && (cols & (cols - 1u)) == 0u) {
        r = u32(i32(sy) - oy) & (rows - 1u);
        k = u32(i32(sx) - ox) & (cols - 1u);
    } else {
        r = u32(((i32(sy) - oy) % i32(rows) + i32(rows)) % i32(rows));
        k = u32(((i32(sx) - ox) % i32(cols) + i32(cols)) % i32(cols));
    }
    var b = df(du(H_BL_OFF) + (r * cols + k) * cpp + min(c, cpp - 1u));
    if (i32(sx) >= ox && sx - u32(ox) < du(H_BC_LEN)) {
        b += df(du(H_BC_OFF) + sx - u32(ox));
    }
    if (i32(sy) >= oy && sy - u32(oy) < du(H_BR_LEN)) {
        b += df(du(H_BR_OFF) + sy - u32(oy));
    }
    return b;
}

fn in_rect(x: u32, y: u32, at: u32) -> bool {
    return x >= du(at) && y >= du(at + 1u) && x - du(at) < du(at + 2u) && y - du(at + 1u) < du(at + 3u);
}

fn lerp1(a: f32, b: f32, f: f32) -> f32 { return a + (b - a) * f; }

// Map coordinate of a gain map along one axis: (i0, i1, fraction).
fn gm_coord(rel: f32, origin: f32, spacing: f32, points: u32) -> vec3<f32> {
    if (points < 2u) { return vec3<f32>(0.0, 0.0, 0.0); }
    let m = clamp((rel - origin) / spacing, 0.0, f32(points - 1u));
    let i = min(floor(m), f32(points - 1u));
    return vec3<f32>(i, min(i + 1.0, f32(points - 1u)), m - i);
}

// One point opcode (record at `r`) on value `v` of plane `p` (of `cpp`) at integer stored
// position (ix, iy) with centre (xc, yc).
fn point_op(r: u32, v: f32, ix: u32, iy: u32, xc: f32, yc: f32, p: u32, cpp: u32) -> f32 {
    let kind = du(r);
    let rx = du(r + 1u);
    let ry = du(r + 2u);
    if (!in_rect(ix, iy, r + 1u)) { return v; }
    let rp = du(r + 7u);
    let cp = du(r + 8u);
    if ((iy - ry) % rp != 0u || (ix - rx) % cp != 0u) { return v; }
    let p0 = du(r + 5u);
    let pn = du(r + 6u);
    if (p < min(p0, cpp) || p >= min(p0 + min(pn, 64u), cpp)) { return v; }
    let off = du(r + 13u);
    let n = du(r + 14u);
    if (kind == 1u) {
        // MapTable: index by the 16-bit value.
        let i = min(u32(max(v * 65535.0 + 0.5, 0.0)), n - 1u);
        let w = du(off + (i >> 1u));
        return f32(select(w & 0xffffu, w >> 16u, (i & 1u) == 1u)) / 65535.0;
    }
    if (kind == 2u) {
        var y = 0.0;
        for (var k = n; k > 0u; k -= 1u) {
            y = y * v + df(off + k - 1u);
        }
        return y;
    }
    if (kind == 3u) {
        let pv = du(r + 15u);
        let ph = du(r + 16u);
        let planes = du(r + 21u);
        let bx = f32(du(r + 9u));
        let by = f32(du(r + 10u));
        let bw = f32(max(du(r + 11u), 1u));
        let bh = f32(max(du(r + 12u), 1u));
        let cy = gm_coord((yc - by) / bh, df(r + 19u), df(r + 17u), pv);
        let cx = gm_coord((xc - bx) / bw, df(r + 20u), df(r + 18u), ph);
        let mp = min(p - p0, planes - 1u);
        let i0 = u32(cy.x);
        let i1 = u32(cy.y);
        let j0 = u32(cx.x);
        let j1 = u32(cx.y);
        let l = lerp1(df(off + (i0 * ph + j0) * planes + mp), df(off + (i1 * ph + j0) * planes + mp), cy.z);
        let rr = lerp1(df(off + (i0 * ph + j1) * planes + mp), df(off + (i1 * ph + j1) * planes + mp), cy.z);
        return v * (l + (rr - l) * cx.z);
    }
    if (kind == 4u) {
        let dx = xc - df(r + 20u);
        let dy = yc - df(r + 21u);
        let r2 = (dx * dx + dy * dy) * df(r + 22u);
        let g = 1.0 + r2 * (df(r + 15u) + r2 * (df(r + 16u) + r2 * (df(r + 17u) + r2 * (df(r + 18u) + r2 * df(r + 19u)))));
        return v * g;
    }
    // 5, 6: delta per row / column; 7, 8: scale per row / column.
    var i = 0u;
    if (kind == 5u || kind == 7u) { i = (iy - ry) / rp; } else { i = (ix - rx) / cp; }
    if (i >= n) { return v; }
    let d = df(off + i);
    if (kind <= 6u) { return v + d; }
    return v * d;
}

fn run_list(head: u32, v0: f32, ix: u32, iy: u32, xc: f32, yc: f32, p: u32, cpp: u32) -> f32 {
    var v = v0;
    let off = du(head);
    let n = du(head + 1u);
    for (var k = 0u; k < n; k += 1u) {
        v = point_op(off + k * REC, v, ix, iy, xc, yc, p, cpp);
    }
    return v;
}

// FixBadPixelsConstant on the stored samples (list 1, first): the inverse-distance mean of
// the good same-colour samples within two pixels.
fn stored(sx: u32, sy: u32, c: u32) -> f32 {
    if (du(H_FLOAT) == 1u) {
        // f32 words, one per sample (HDR merges): never bad pixels, never clipped.
        let i = uu(U_RAW_BASE) + ((sy - uu(U_RAW_ROW0)) * du(H_W) + sx) * du(H_CPP) + c;
        return bitcast<f32>(RAW[i]);
    }
    let v = raw_at(sx, sy, c);
    if (du(H_BAD) == 0u || v != du(H_BAD_V) || !in_rect(sx, sy, H_BAD_RECT)) {
        return f32(v);
    }
    let col = color_at(sx, sy);
    let cfa = du(H_LAYOUT) <= 1u;
    var sw = 0.0;
    var sv = 0.0;
    for (var dy = -2; dy <= 2; dy += 1) {
        for (var dx = -2; dx <= 2; dx += 1) {
            if (dx == 0 && dy == 0) { continue; }
            let nx = i32(sx) + dx;
            let ny = i32(sy) + dy;
            if (nx < 0 || ny < 0 || !in_rect(u32(nx), u32(ny), H_BAD_RECT)) { continue; }
            if (u32(ny) < uu(U_RAW_ROW0) || u32(ny) - uu(U_RAW_ROW0) >= uu(U_RAW_ROWS)) { continue; }
            if (cfa && color_at(u32(nx), u32(ny)) != col) { continue; }
            let q = raw_at(u32(nx), u32(ny), c);
            if (q == du(H_BAD_V)) { continue; }
            let w = 1.0 / f32(dx * dx + dy * dy);
            sw += w;
            sv += w * f32(q);
        }
    }
    if (sw > 0.0) { return floor(sv / sw + 0.5); }
    return f32(v);
}

// The stored sample through OpcodeList1 and the linearization table (linear units).
fn lin(sx: u32, sy: u32, c: u32) -> f32 {
    let cpp = du(H_CPP);
    var v = stored(sx, sy, c);
    let xc = f32(sx) + 0.5;
    let yc = f32(sy) + 0.5;
    if (du(H_L1_N) > 0u) {
        v = run_list(H_L1_OFF, v / 65535.0, sx, sy, xc, yc, c, cpp) * 65535.0;
    }
    let ln = du(H_LIN_LEN);
    if (ln > 0u) {
        let i = min(u32(clamp(v + 0.5, 0.0, 65535.0)), ln - 1u);
        let w = du(du(H_LIN_OFF) + (i >> 1u));
        v = f32(select(w & 0xffffu, w >> 16u, (i & 1u) == 1u));
    }
    return v;
}

// A linear value at domain (crop) position (x, y), channel c: black and white, the dark
// frame and flat field, OpcodeList2, white balance. Never clipped.
fn norm(v0: f32, x: i32, y: i32, c: u32) -> f32 {
    let sx = du(H_CX) + u32(x);
    let sy = du(H_CY) + u32(y);
    let cpp = du(H_CPP);
    let b = black_at(sx, sy, c);
    var v = (v0 - b) / max(df(H_WHITE) - b, 1.0);
    if (du(H_CAL) == 1u) {
        let g = unpack2x16float(CAL[uu(U_CAL_BASE) + (u32(y) - uu(U_CAL_ROW0)) * du(H_CW) + u32(x)]);
        v = (v - g.y) * g.x;
    }
    if (du(H_L2_N) > 0u) {
        v = run_list(H_L2_OFF, v, sx, sy, f32(sx) + 0.5, f32(sy) + 0.5, c, cpp);
    }
    var col = c;
    if (cpp == 1u) { col = color_at(sx, sy); }
    return v * df(H_WB + col);
}

// The normalised, white-balanced sample of channel c at domain (crop) position (x, y):
// OpcodeList1, linearization, black and white, calibration, OpcodeList2, white balance.
// Never clipped. With U_PREPARED the corrected mosaic instead.
fn prep(x: i32, y: i32, c: u32) -> f32 {
    if (uu(U_PREPARED) == 1u) {
        return bitcast<f32>(RAW[uu(U_RAW_BASE) + (u32(y) - uu(U_RAW_ROW0)) * du(H_CW) + u32(x)]);
    }
    return norm(lin(du(H_CX) + u32(x), du(H_CY) + u32(y), c), x, y, c);
}

// What the sample at (x, y) reads at the sensor's clip level.
fn prep_sat(x: i32, y: i32, c: u32) -> f32 {
    return norm(df(H_CLIP), x, y, c);
}

// `norm` of two values at one place (the black level, calibration and colour looked up once).
fn norm2(v0: vec2<f32>, x: i32, y: i32, c: u32) -> vec2<f32> {
    let sx = du(H_CX) + u32(x);
    let sy = du(H_CY) + u32(y);
    let cpp = du(H_CPP);
    let b = black_at(sx, sy, c);
    var v = (v0 - b) / max(df(H_WHITE) - b, 1.0);
    if (du(H_CAL) == 1u) {
        let g = unpack2x16float(CAL[uu(U_CAL_BASE) + (u32(y) - uu(U_CAL_ROW0)) * du(H_CW) + u32(x)]);
        v = (v - g.y) * g.x;
    }
    if (du(H_L2_N) > 0u) {
        let xc = f32(sx) + 0.5;
        let yc = f32(sy) + 0.5;
        v = vec2<f32>(run_list(H_L2_OFF, v.x, sx, sy, xc, yc, c, cpp),
            run_list(H_L2_OFF, v.y, sx, sy, xc, yc, c, cpp));
    }
    var col = c;
    if (cpp == 1u) { col = color_at(sx, sy); }
    return v * df(H_WB + col);
}

// `prep` and `prep_sat` of channel 0 at (x, y) together.
fn prep2(x: i32, y: i32) -> vec2<f32> {
    if (uu(U_PREPARED) == 1u) {
        return vec2<f32>(prep(x, y, 0u), prep_sat(x, y, 0u));
    }
    let v0 = lin(du(H_CX) + u32(x), du(H_CY) + u32(y), 0u);
    return norm2(vec2<f32>(v0, df(H_CLIP)), x, y, 0u);
}

// ---- Camera RGB to the output ---------------------------------------------------------------

fn rgb_to_hsv(c: vec3<f32>) -> vec3<f32> {
    let v = max(c.x, max(c.y, c.z));
    let gap = v - min(c.x, min(c.y, c.z));
    if (gap <= 0.0) { return vec3<f32>(0.0, 0.0, v); }
    var h = 0.0;
    if (c.x == v) {
        h = (c.y - c.z) / gap;
        if (h < 0.0) { h += 6.0; }
    } else if (c.y == v) {
        h = 2.0 + (c.z - c.x) / gap;
    } else {
        h = 4.0 + (c.x - c.y) / gap;
    }
    return vec3<f32>(h, gap / v, v);
}

fn hsv_to_rgb(hsv: vec3<f32>) -> vec3<f32> {
    let s = hsv.y;
    let v = hsv.z;
    if (s <= 0.0) { return vec3<f32>(v); }
    var h = hsv.x;
    if (h >= 6.0 || h < 0.0) { h = 0.0; }
    let i = min(u32(h), 5u);
    let f = h - f32(i);
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    switch i {
        case 0u: { return vec3<f32>(v, t, p); }
        case 1u: { return vec3<f32>(q, v, p); }
        case 2u: { return vec3<f32>(p, v, t); }
        case 3u: { return vec3<f32>(p, q, v); }
        case 4u: { return vec3<f32>(t, p, v); }
        default: { return vec3<f32>(v, p, q); }
    }
}

fn srgb_enc(v: f32) -> f32 {
    if (v <= 0.0031308) { return v * 12.92; }
    return 1.055 * pow(v, 1.0 / 2.4) - 0.055;
}

fn srgb_dec(v: f32) -> f32 {
    if (v <= 0.04045) { return v / 12.92; }
    return pow((v + 0.055) / 1.055, 2.4);
}

fn hdr_enc(x: f32) -> f32 { return x * (256.0 + x) / (256.0 * (1.0 + x)); }
fn hdr_dec(y: f32) -> f32 { return 16.0 * (8.0 * y - 8.0 + sqrt(max(64.0 * y * y - 127.0 * y + 64.0, 0.0))); }

fn tab(t: u32, v: u32, h: u32, s: u32) -> vec3<f32> {
    let i = du(t + 4u) + ((v * du(t) + h) * du(t + 1u) + s) * 3u;
    return vec3<f32>(df(i), df(i + 1u), df(i + 2u));
}

// Trilinear lookup of the HSV table at block `t`: hue wraps, saturation and value clamp.
fn hsv_lookup(t: u32, h6: f32, s: f32, v: f32) -> vec3<f32> {
    let nh = du(t);
    let ns = du(t + 1u);
    let nv = du(t + 2u);
    let hs = h6 * f32(nh) / 6.0;
    var h0 = min(u32(max(hs, 0.0)), nh - 1u);
    let hf = clamp(hs - f32(h0), 0.0, 1.0);
    var h1 = h0 + 1u;
    if (h1 >= nh) { h1 = 0u; }
    if (nh == 1u) { h0 = 0u; }
    let ss = clamp(s, 0.0, 1.0) * f32(ns - 1u);
    let s0 = min(u32(ss), ns - 2u);
    let sf = ss - f32(s0);
    var v0 = 0u;
    var vf = 0.0;
    if (nv > 1u) {
        let vs = clamp(v, 0.0, 1.0) * f32(nv - 1u);
        v0 = min(u32(vs), nv - 2u);
        vf = vs - f32(v0);
    }
    let a0 = mix(mix(tab(t, v0, h0, s0), tab(t, v0, h1, s0), hf), mix(tab(t, v0, h0, s0 + 1u), tab(t, v0, h1, s0 + 1u), hf), sf);
    if (nv <= 1u) { return a0; }
    let v1 = v0 + 1u;
    let a1 = mix(mix(tab(t, v1, h0, s0), tab(t, v1, h1, s0), hf), mix(tab(t, v1, h0, s0 + 1u), tab(t, v1, h1, s0 + 1u), hf), sf);
    return mix(a0, a1, vf);
}

// One table stage on linear ProPhoto RGB (values >= 0), never clipping the value.
fn hsv_stage(t: u32, hdr: bool, p0: vec3<f32>) -> vec3<f32> {
    let enc = hdr && du(t + 2u) > 1u;
    var p = p0;
    if (enc) { p = vec3<f32>(hdr_enc(p.x), hdr_enc(p.y), hdr_enc(p.z)); }
    let hsv = rgb_to_hsv(p);
    let srgb = du(t + 3u) == 1u;
    var ve = hsv.z;
    if (srgb) { ve = srgb_enc(ve); }
    let d = hsv_lookup(t, hsv.x, hsv.y, ve);
    var h = hsv.x + d.x / 60.0;
    h = h - 6.0 * floor(h / 6.0);
    if (!(h < 6.0) || !(h >= 0.0)) { h = 0.0; }
    let s = min(hsv.y * d.y, 1.0);
    var v = max(ve * d.z, 0.0);
    if (srgb) { v = srgb_dec(v); }
    var o = hsv_to_rgb(vec3<f32>(h, s, v));
    if (enc) { o = vec3<f32>(hdr_dec(o.x), hdr_dec(o.y), hdr_dec(o.z)); }
    return o;
}

fn mat3(at: u32, c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        df(at) * c.x + df(at + 1u) * c.y + df(at + 2u) * c.z,
        df(at + 3u) * c.x + df(at + 4u) * c.y + df(at + 5u) * c.z,
        df(at + 6u) * c.x + df(at + 7u) * c.y + df(at + 8u) * c.z);
}

// White-balanced camera RGB at domain pixel (x, y) (sensor centre (xc, yc)) → output: list 3,
// then the matrix or the DNG profile.
fn finish(c0: vec3<f32>, ix: u32, iy: u32, xc: f32, yc: f32) -> vec4<f32> {
    var c = c0;
    if (du(H_L3_N) > 0u) {
        c = vec3<f32>(
            run_list(H_L3_OFF, c.x, ix, iy, xc, yc, 0u, 3u),
            run_list(H_L3_OFF, c.y, ix, iy, xc, yc, 1u, 3u),
            run_list(H_L3_OFF, c.z, ix, iy, xc, yc, 2u, 3u));
    }
    let out = du(H_OUTPUT);
    if (out == 1u) { return vec4<f32>(c, 1.0); }
    let pr = du(H_PROF);
    if (pr == 0u) { return vec4<f32>(mat3(H_MAT, c), 1.0); }
    // DNG profile: camera → ProPhoto, HueSatMap, LookTable, → output.
    var p = mat3(pr, c);
    let hdr = du(pr + 18u) == 1u;
    let hsm = du(pr + 19u);
    let lt = du(pr + 20u);
    if (hsm != 0u || lt != 0u) {
        // Negative components (outside the camera gamut) bypass the tables.
        let neg = min(p, vec3<f32>(0.0));
        var q = max(p, vec3<f32>(0.0));
        if (hsm != 0u) { q = hsv_stage(hsm, hdr, q); }
        if (lt != 0u) { q = hsv_stage(lt, hdr, q); }
        p = q + neg;
    }
    return vec4<f32>(mat3(pr + 9u, p), 1.0);
}

// Stored position and centre of domain pixel (x, y) at full size.
fn finish_full(c: vec3<f32>, x: u32, y: u32) -> vec4<f32> {
    let ix = du(H_CX) + x;
    let iy = du(H_CY) + y;
    return finish(c, ix, iy, f32(ix) + 0.5, f32(iy) + 0.5);
}

// Reflect a coordinate into [0, n) about its first and last pixel (keeps the Bayer phase).
fn refl(v: i32, n: i32) -> i32 {
    var r = v;
    if (r < 0) { r = -r; }
    if (r >= n) { r = 2 * n - 2 - r; }
    return clamp(r, 0, n - 1);
}
