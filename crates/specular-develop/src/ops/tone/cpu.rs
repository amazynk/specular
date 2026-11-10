//! CPU references of the tone kernels (tests, and the tone equalizer's mask percentiles for
//! zone picks): the program steps run in Rust with the same words and the same f32
//! arithmetic in the same order as the WGSL, and the per-pixel parts of the tone ops. Independent textbook versions of the guided filter and
//! the fast local Laplacian filter check the programs themselves (see `tests.rs`).

use crate::graph::{Program, Roi};

/// An RGBA f32 image the kernels sample (clamped to its edge, as `src_at`).
#[derive(Clone, Copy)]
pub struct Img<'a> {
    pub w: u32,
    pub h: u32,
    pub px: &'a [f32],
}

impl Img<'_> {
    pub fn at(&self, x: i64, y: i64) -> [f32; 4] {
        let cx = x.clamp(0, i64::from(self.w) - 1) as usize;
        let cy = y.clamp(0, i64::from(self.h) - 1) as usize;
        let i = (cy * self.w as usize + cx) * 4;
        [self.px[i], self.px[i + 1], self.px[i + 2], self.px[i + 3]]
    }
}

pub const LUMA: [f32; 3] = super::LUMA;

pub fn dot(c: [f32; 3]) -> f32 {
    c[0] * LUMA[0] + c[1] * LUMA[1] + c[2] * LUMA[2]
}

fn bw(a: i32) -> f32 {
    match a {
        0 => 0.375,
        -1 | 1 => 0.25,
        _ => 0.0625,
    }
}

pub fn remap(v: f32, g: f32, amt: f32, sig: f32) -> f32 {
    let d = v - g;
    let u = d / (2.0 * sig);
    if u.abs() >= 1.0 {
        return v;
    }
    let b = 1.0 - u * u;
    v + amt * d * b * b
}

fn tap(s: &[f32], o: u32, w: u32, h: u32, x: i32, y: i32) -> f32 {
    let cx = x.clamp(0, w as i32 - 1) as u32;
    let cy = y.clamp(0, h as i32 - 1) as u32;
    s[(o + cy * w + cx) as usize]
}

fn ew(odd: bool, i: u32) -> f32 {
    match (odd, i) {
        (true, 0) => 0.0,
        (true, _) => 0.5,
        (false, 1) => 0.75,
        _ => 0.125,
    }
}

fn up(s: &[f32], o: u32, w1: u32, h1: u32, x: u32, y: u32) -> f32 {
    let mx = (x >> 1) as i32 - 1;
    let my = (y >> 1) as i32 - 1;
    let (ox, oy) = (x & 1 == 1, y & 1 == 1);
    let mut acc = 0.0;
    for b in 0..3u32 {
        let mut row = 0.0;
        for a in 0..3u32 {
            row += ew(ox, a) * tap(s, o, w1, h1, mx + a as i32, my + b as i32);
        }
        acc += ew(oy, b) * row;
    }
    acc
}

fn f(w: u32) -> f32 {
    f32::from_bits(w)
}

/// Runs `prog` over `region` of `img` with statistics `st`; returns the scratch.
pub fn run_program(prog: &Program, img: Img<'_>, st: &[f32], region: Roi) -> Vec<f32> {
    let _ = region;
    let mut s = vec![0f32; prog.words as usize];
    for step in &prog.steps {
        let p = &step.words;
        let [tx, ty] = step.threads;
        for y in 0..ty {
            for x in 0..tx {
                run_step(step.kernel.name, p, &mut s, img, st, x, y, [tx, ty]);
            }
        }
    }
    s
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_step(
    name: &str,
    p: &[u32],
    s: &mut [f32],
    img: Img<'_>,
    st: &[f32],
    tx: u32,
    ty: u32,
    threads: [u32; 2],
) {
    match name {
        "grid_reduce_v1" => {
            let (sw, sh) = (img.w as f32, img.h as f32);
            let fx = sw / threads[0] as f32;
            let fy = sh / threads[1] as f32;
            let x0 = tx as f32 * fx;
            let x1 = x0 + fx;
            let y0 = ty as f32 * fy;
            let y1 = y0 + fy;
            let mut acc = [0f32; 4];
            let mut wsum = 0f32;
            let ye = (y1.ceil() as u32).min(img.h);
            let xe = (x1.ceil() as u32).min(img.w);
            for y in y0.floor() as u32..ye {
                let wy = ((y + 1) as f32).min(y1) - (y as f32).max(y0);
                for x in x0.floor() as u32..xe {
                    let wx = ((x + 1) as f32).min(x1) - (x as f32).max(x0);
                    let w = wx * wy;
                    if w > 0.0 {
                        let c = img.at(i64::from(x), i64::from(y));
                        for k in 0..4 {
                            acc[k] += c[k] * w;
                        }
                        wsum += w;
                    }
                }
            }
            let o = (4 * (ty * threads[0] + tx)) as usize;
            for k in 0..4 {
                s[o + k] = acc[k] / wsum.max(1e-12);
            }
        }
        "tone_put_v1" => s[(p[0] + tx) as usize] = f(p[1 + tx as usize]),
        "tone_copy_st_v1" => s[(p[1] + tx) as usize] = st[(p[0] + tx) as usize],
        "tone_lum_v1" => {
            let c = img.at(i64::from(p[2] + tx), i64::from(p[3] + ty));
            s[(p[0] + ty * p[1] + tx) as usize] =
                dot([c[0], c[1], c[2]]).max(1.525_878_9e-5).log2();
        }
        "tone_box_h_v1" => {
            let (so, d, w, r, planes, ps, mode, po) =
                (p[0], p[1], p[2], p[4] as i32, p[5], p[6], p[7], p[8]);
            let lo = (tx as i32 - r).max(0);
            let hi = (tx as i32 + r).min(w as i32 - 1);
            let n = (hi - lo + 1) as f32;
            let row = ty * w;
            let at = (row + tx) as usize;
            if mode == 0 {
                for c in 0..planes {
                    let mut sum = 0f32;
                    for i in lo..=hi {
                        sum += s[(so + c * ps + row + i as u32) as usize];
                    }
                    s[d as usize + (c * ps) as usize + at] = sum / n;
                }
                return;
            }
            let (mut si, mut sp, mut sip, mut sii) = (0f32, 0f32, 0f32, 0f32);
            for i in lo..=hi {
                let q = row + i as u32;
                let gi = s[(so + q) as usize];
                let mut pv = s[(po + q) as usize];
                if mode == 2 {
                    pv -= gi;
                }
                si += gi;
                sp += pv;
                sip += gi * pv;
                sii += gi * gi;
            }
            let ps = ps as usize;
            s[d as usize + at] = si / n;
            s[d as usize + ps + at] = sp / n;
            s[d as usize + 2 * ps + at] = sip / n;
            s[d as usize + 3 * ps + at] = sii / n;
        }
        "tone_box_v_v1" => {
            let (so, d, w, h, r, planes, ps, mode) =
                (p[0], p[1], p[2], p[3], p[4] as i32, p[5], p[6], p[7]);
            let lo = (ty as i32 - r).max(0);
            let hi = (ty as i32 + r).min(h as i32 - 1);
            let n = (hi - lo + 1) as f32;
            let at = (ty * w + tx) as usize;
            if mode == 0 {
                for c in 0..planes {
                    let mut sum = 0f32;
                    for i in lo..=hi {
                        sum += s[(so + c * ps + i as u32 * w + tx) as usize];
                    }
                    s[d as usize + (c * ps) as usize + at] = sum / n;
                }
                return;
            }
            let mut m = [0f32; 4];
            for i in lo..=hi {
                let q = i as u32 * w + tx;
                for (k, v) in m.iter_mut().enumerate() {
                    *v += s[(so + k as u32 * ps + q) as usize];
                }
            }
            let m = m.map(|v| v / n);
            let var_i = (m[3] - m[0] * m[0]).max(0.0);
            let a = (m[2] - m[0] * m[1]) / (var_i + f(p[8]));
            s[d as usize + at] = a;
            s[(d + ps) as usize + at] = m[1] - a * m[0];
        }
        "tone_reduce_v1" => {
            let (so, ws, hs, ss, d, wd, ds, planes) =
                (p[0], p[1], p[2], p[3], p[4], p[5], p[7], p[8]);
            let mode = p[9];
            let (x, y) = (tx as i32 * 2, ty as i32 * 2);
            let at = ty * wd + tx;
            if mode == 1 {
                let (mut mn, mut mx) = (3.0e38f32, -3.0e38f32);
                for b in -2..=2 {
                    for a in -2..=2 {
                        mn = mn.min(tap(s, p[10], ws, hs, x + a, y + b));
                        mx = mx.max(tap(s, p[11], ws, hs, x + a, y + b));
                    }
                }
                s[(p[12] + at) as usize] = mn;
                s[(p[13] + at) as usize] = mx;
            }
            for j in 0..planes {
                if mode == 2 {
                    let (lo, hi) = (s[(p[11] + at) as usize], s[(p[12] + at) as usize]);
                    let (g0, dg) = (s[p[14] as usize], s[p[14] as usize + 1]);
                    let g = g0 + j as f32 * dg;
                    let reach = 2.0 * f(p[15]);
                    if g - reach >= hi || g + reach <= lo {
                        s[(d + j * ds + at) as usize] = s[(p[10] + at) as usize];
                        continue;
                    }
                }
                let mut acc = 0f32;
                for b in -2..=2 {
                    let mut row = 0f32;
                    for a in -2..=2 {
                        row += bw(a) * tap(s, so + j * ss, ws, hs, x + a, y + b);
                    }
                    acc += bw(b) * row;
                }
                s[(d + j * ds + ty * wd + tx) as usize] = acc;
            }
        }
        "tone_reduce_l1_v1" | "tone_reduce_l1_lum_v1" => {
            let (so, ws, hs, ox, oy) = (p[0], p[1], p[2], p[3], p[4]);
            let (g1, mno, mxo, r1, wd, ds, k, rng) =
                (p[5], p[6], p[7], p[8], p[9], p[10], p[11], p[12]);
            let (amt, sig) = (f(p[13]), f(p[14]));
            let (x, y) = (tx as i32 * 2, ty as i32 * 2);
            let mut v = [0f32; 25];
            for b in 0..5 {
                for a in 0..5 {
                    let cx = (x + a - 2).clamp(0, ws as i32 - 1) as u32;
                    let cy = (y + b - 2).clamp(0, hs as i32 - 1) as u32;
                    v[(b * 5 + a) as usize] = if name == "tone_reduce_l1_lum_v1" {
                        let c = img.at(i64::from(ox + cx), i64::from(oy + cy));
                        dot([c[0], c[1], c[2]]).max(1.525_878_9e-5).log2()
                    } else {
                        s[(so + cy * ws + cx) as usize]
                    };
                }
            }
            let at = (ty * wd + tx) as usize;
            let (mut mn, mut mx) = (3.0e38f32, -3.0e38f32);
            for q in v {
                mn = mn.min(q);
                mx = mx.max(q);
            }
            s[mno as usize + at] = mn;
            s[mxo as usize + at] = mx;
            let mut plain = 0f32;
            for b in 0..5 {
                let mut row = 0f32;
                for a in 0..5 {
                    row += bw(a - 2) * v[(b * 5 + a) as usize];
                }
                plain += bw(b - 2) * row;
            }
            s[g1 as usize + at] = plain;
            let (g0, dg) = (s[rng as usize], s[rng as usize + 1]);
            for j in 0..k {
                let g = g0 + j as f32 * dg;
                let mut acc = plain;
                if g - 2.0 * sig < mx && g + 2.0 * sig > mn {
                    acc = 0.0;
                    for b in 0..5 {
                        let mut row = 0f32;
                        for a in 0..5 {
                            row += bw(a - 2) * remap(v[(b * 5 + a) as usize], g, amt, sig);
                        }
                        acc += bw(b - 2) * row;
                    }
                }
                s[(r1 + j * ds) as usize + at] = acc;
            }
        }
        "tone_collapse_v1" => {
            let (gl, wl, gl1, wl1, hl1) = (p[0], p[1], p[3], p[4], p[5]);
            let (rl, rls, rl1, rl1s, k, rng) = (p[6], p[7], p[8], p[9], p[10], p[11]);
            let (amt, sig, ol1, ol) = (f(p[12]), f(p[13]), p[14], p[15]);
            let idx = ty * wl + tx;
            let g = s[(gl + idx) as usize];
            let mut lap = g - up(s, gl1, wl1, hl1, tx, ty);
            if p[16] != 0 {
                let (g0, dg) = (s[rng as usize], s[rng as usize + 1]);
                let jf = ((g - g0) / dg).clamp(0.0, (k - 1) as f32);
                let j0 = (jf.floor() as u32).min(k - 2);
                let fr = jf - j0 as f32;
                let mut l2 = [0f32; 2];
                for (i, l) in l2.iter_mut().enumerate() {
                    let j = j0 + i as u32;
                    let gr = if rl == u32::MAX {
                        remap(g, g0 + j as f32 * dg, amt, sig)
                    } else {
                        s[(rl + j * rls + idx) as usize]
                    };
                    *l = gr - up(s, rl1 + j * rl1s, wl1, hl1, tx, ty);
                }
                lap = l2[0] * (1.0 - fr) + l2[1] * fr;
            }
            s[(ol + idx) as usize] = lap + up(s, ol1, wl1, hl1, tx, ty);
        }
        "tone_down_v1" => {
            let (w, h, fct) = (p[1], p[2], p[5]);
            let x1 = (tx * fct + fct).min(w);
            let y1 = (ty * fct + fct).min(h);
            let mut sum = 0f32;
            for y in ty * fct..y1 {
                for x in tx * fct..x1 {
                    sum += s[(p[0] + y * w + x) as usize];
                }
            }
            s[(p[3] + ty * p[4] + tx) as usize] = sum / ((x1 - tx * fct) * (y1 - ty * fct)) as f32;
        }
        "tone_hist_clear_v1" => s[(p[0] + tx) as usize] = 0.0,
        "tone_hist_add_v2" => {
            if tx >= p[4] || ty >= p[9] {
                return;
            }
            let at = ty * p[4] + tx;
            let v = match p[0] {
                0 => s[(p[1] + at) as usize],
                1 => {
                    let c = img.at(i64::from(tx), i64::from(ty));
                    let b = f(p[8]);
                    dot([c[0] - b, c[1] - b, c[2] - b])
                        .max(1.525_878_9e-5)
                        .log2()
                }
                _ => {
                    let (a, b, g) = (
                        s[(p[1] + at) as usize],
                        s[(p[2] + at) as usize],
                        s[(p[3] + at) as usize],
                    );
                    a * g + b
                }
            };
            let (lo, hi) = (f(p[6]), f(p[7]));
            let bin = ((v - lo) * 1024.0 / (hi - lo)).floor().clamp(0.0, 1023.0) as u32;
            let i = (p[5] + bin) as usize;
            s[i] = f32::from_bits(s[i].to_bits() + 1);
        }
        "tone_hist_pct_v2" => {
            if (tx, ty) != (0, 0) {
                return;
            }
            let hist = p[0] as usize;
            let (lo, hi) = (f(p[1]), f(p[2]));
            let counts: Vec<f32> = (0..1024).map(|b| s[hist + b].to_bits() as f32).collect();
            let count = |b: usize| counts[b];
            let mut total = 0f32;
            for b in 0..1024 {
                total += count(b);
            }
            for q in 0..p[4] as usize {
                let goal = f(p[5 + q]) * total;
                let mut cum = 0f32;
                let mut res = hi;
                for b in 0..1024 {
                    let c = count(b);
                    if c > 0.0 && cum + c >= goal {
                        res = lo + (b as f32 + (goal - cum) / c) * (hi - lo) / 1024.0;
                        break;
                    }
                    cum += c;
                }
                s[p[3] as usize + q] = res;
            }
        }
        "tone_range_v1" => {
            let lo = s[p[0] as usize] - f(p[2]);
            let hi = s[p[0] as usize + 1] + f(p[2]);
            s[p[1] as usize] = lo;
            s[p[1] as usize + 1] = ((hi - lo) / (p[3] - 1) as f32).max(1e-3);
        }
        n => panic!("no CPU reference for kernel {n}"),
    }
}

/// `dv_grid`: a bilinear sample of statistics plane `o` at image pixel (x, y) of a
/// `w`×`h` image.
pub fn grid(st: &[f32], o: u32, x: i32, y: i32, w: u32, h: u32) -> f32 {
    let gw = st[0] as u32;
    let gh = st[1] as u32;
    let gx = ((x as f32 + 0.5) * gw as f32 / w as f32 - 0.5).clamp(0.0, (gw - 1) as f32);
    let gy = ((y as f32 + 0.5) * gh as f32 / h as f32 - 0.5).clamp(0.0, (gh - 1) as f32);
    let x0 = gx.floor() as u32;
    let y0 = gy.floor() as u32;
    let x1 = (x0 + 1).min(gw - 1);
    let y1 = (y0 + 1).min(gh - 1);
    let fx = gx - x0 as f32;
    let fy = gy - y0 as f32;
    let at = |xx: u32, yy: u32| st[(o + yy * gw + xx) as usize];
    let mix = |a: f32, b: f32, t: f32| a * (1.0 - t) + b * t;
    let top = mix(at(x0, y0), at(x1, y0), fx);
    let bot = mix(at(x0, y1), at(x1, y1), fx);
    mix(top, bot, fy)
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn zones(u: &[f32], m: f32) -> f32 {
    let v = (m + 8.0).clamp(0.0, 8.0);
    let i = (v.floor() as usize).min(7);
    let fr = v - i as f32;
    let g0 = u[i.saturating_sub(1)];
    let (g1, g2) = (u[i], u[i + 1]);
    let g3 = u[(i + 2).min(8)];
    0.5 * (2.0 * g1
        + (g2 - g0) * fr
        + (2.0 * g0 - 5.0 * g1 + 4.0 * g2 - g3) * fr * fr
        + (3.0 * g1 - g0 - 3.0 * g2 + g3) * fr * fr * fr)
}

/// The tone equalizer's kernel at image pixel (x, y) of a `w`×`h` image.
pub fn tone_eq(u: &[f32], c: [f32; 4], st: &[f32], x: i32, y: i32, w: u32, h: u32) -> [f32; 4] {
    let n = st[0] as u32 * st[1] as u32;
    let l = dot([c[0], c[1], c[2]]).max(1.525_878_9e-5).log2();
    let mut m = grid(st, 16, x, y, w, h) * l + grid(st, 16 + n, x, y, w, h);
    if u[13] != 0.0 {
        let lo = st[4];
        m = (m - lo) * 7.0 / (st[5] - lo).max(0.5) - 7.5;
    }
    m = (m + 2.473_931_2) * u[12] - 2.473_931_2 + u[11];
    let d = m + 2.473_931_2;
    let ws = 1.0 - smooth(-4.0, 0.0, d);
    let wh = smooth(0.0, 3.0, d);
    let g = u[0] * ws + u[1] * wh + zones(&u[2..11], m);
    let k = g.exp2();
    [c[0] * k, c[1] * k, c[2] * k, c[3]]
}

/// Local contrast's kernel at (x, y): `words` its uniforms, `region` the program's.
#[allow(clippy::too_many_arguments)]
pub fn local_contrast(
    words: &[u32],
    c: [f32; 4],
    st: &[f32],
    lc: &[f32],
    region: Roi,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
) -> [f32; 4] {
    let yl = dot([c[0], c[1], c[2]]);
    if yl <= 0.0 {
        return c;
    }
    let l = yl.max(1.525_878_9e-5).log2();
    let mut d = 0f32;
    let tl = words[2];
    if f(words[0]) != 0.0 && tl > 0 {
        let r = super::local_contrast::aligned(region, tl);
        let lv = super::local_contrast::region_levels(r, tl);
        if lv > 0 {
            let (k, amt, sig) = (words[3], f(words[4]), f(words[5]));
            let (aw, ah) = (r.w, r.h);
            let (lx, ly) = (x as u32 - r.x, y as u32 - r.y);
            let (w1, h1) = (aw.div_ceil(2), ah.div_ceil(2));
            let n1 = w1 * h1;
            let g1 = 16;
            let r1 = g1 + n1;
            let out1 = if lv >= 2 { r1 + k * n1 } else { g1 };
            let (g0, dg) = (lc[0], lc[1]);
            let g = l;
            let jf = ((g - g0) / dg).clamp(0.0, (k - 1) as f32);
            let j0 = (jf.floor() as u32).min(k - 2);
            let fr = jf - j0 as f32;
            let a0 = remap(g, g0 + j0 as f32 * dg, amt, sig) - up(lc, r1 + j0 * n1, w1, h1, lx, ly);
            let a1 = remap(g, g0 + (j0 + 1) as f32 * dg, amt, sig)
                - up(lc, r1 + (j0 + 1) * n1, w1, h1, lx, ly);
            let out0 = a0 * (1.0 - fr) + a1 * fr + up(lc, out1, w1, h1, lx, ly);
            d += out0 - l;
        }
    }
    if f(words[1]) != 0.0 {
        let n = st[0] as u32 * st[1] as u32;
        d += grid(st, 16, x, y, w, h) * l + grid(st, 16 + n, x, y, w, h);
    }
    let k = d.exp2();
    [c[0] * k, c[1] * k, c[2] * k, c[3]]
}

fn enc(v: f32) -> f32 {
    if v > 0.0 { v.powf(1.0 / 2.4) } else { v }
}

fn dec(v: f32) -> f32 {
    if v > 0.0 { v.powf(2.4) } else { v }
}

fn curve(u: &[u32], o: usize, x: f32) -> f32 {
    let n = u[o] as usize;
    let (xs, ys, ms) = (o + 1, o + 17, o + 33);
    if x <= 0.0 {
        return f(u[ys]) + f(u[ms]) * x;
    }
    if x >= 1.0 {
        return f(u[ys + n - 1]) + f(u[ms + n - 1]) * (x - 1.0);
    }
    let mut i = n - 2;
    for j in 0..n - 1 {
        if x < f(u[xs + j + 1]) {
            i = j;
            break;
        }
    }
    let x0 = f(u[xs + i]);
    let hh = f(u[xs + i + 1]) - x0;
    let t = (x - x0) / hh;
    let t2 = t * t;
    let t3 = t2 * t;
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;
    h00 * f(u[ys + i])
        + h10 * hh * f(u[ms + i])
        + h01 * f(u[ys + i + 1])
        + h11 * hh * f(u[ms + i + 1])
}

fn tone_curve(u: &[u32], v: f32, levels: bool) -> f32 {
    let mut x = enc(v);
    if levels {
        x = (x - f(u[1])) / (f(u[2]) - f(u[1]));
        if x > 0.0 {
            x = x.powf(f(u[3]));
        }
    }
    if u[4] != 0 {
        x = curve(u, 4, x);
    }
    dec(x)
}

/// Curves and levels on one pixel.
pub fn curves(u: &[u32], c: [f32; 4]) -> [f32; 4] {
    let levels = f(u[1]) != 0.0 || f(u[2]) != 1.0 || f(u[3]) != 1.0;
    let mut v = [c[0], c[1], c[2]];
    if levels || u[4] != 0 {
        match u[0] {
            0 => v = v.map(|x| tone_curve(u, x, levels)),
            m => {
                let nm = if m == 2 {
                    v[0].max(v[1].max(v[2]))
                } else {
                    dot(v)
                };
                if nm > 0.0 {
                    let k = tone_curve(u, nm, levels) / nm;
                    v = v.map(|x| x * k);
                }
            }
        }
    }
    for (ch, x) in v.iter_mut().enumerate() {
        let o = 53 + ch * 49;
        if u[o] != 0 {
            *x = dec(curve(u, o, enc(*x)));
        }
    }
    [v[0], v[1], v[2], c[3]]
}
