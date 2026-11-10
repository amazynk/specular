//! CPU references of the detail kernels (tests only): the program steps and node kernels in
//! Rust with the same words and f32 arithmetic as the WGSL. Textbook versions in `tests.rs`
//! check the programs themselves.

use super::capture::{self, Capture};
use super::denoise::{self, Denoise};
use super::noise::{gat, gat_inv};
use super::sharpen::{self, Sharpen};
use crate::graph::{Program, Roi};
use crate::ops::tone::cpu::Img;

const LUMA: [f32; 3] = [0.2627, 0.6780, 0.0593];

fn dot(c: [f32; 4]) -> f32 {
    c[0] * LUMA[0] + c[1] * LUMA[1] + c[2] * LUMA[2]
}

fn pt(s: &[f32], o: u32, w: u32, h: u32, x: i32, y: i32) -> f32 {
    let cx = x.clamp(0, w as i32 - 1) as u32;
    let cy = y.clamp(0, h as i32 - 1) as u32;
    s[(o + cy * w + cx) as usize]
}

fn f(words: &[u32], i: usize) -> f32 {
    f32::from_bits(words[i])
}

/// Runs `prog` over `region` of `img` into a fresh scratch.
pub fn run_program(prog: &Program, img: &Img<'_>, region: Roi) -> Vec<f32> {
    let _ = region;
    let mut s = vec![0f32; prog.words as usize];
    for st in &prog.steps {
        let [tw, th] = st.threads;
        let p = &st.words;
        // The fused transform and first level: the two steps it stands for.
        if st.kernel.name == "detail_atrous0_v1" {
            let (w, h, n) = (p[2], p[3], p[4]);
            let mut vst = vec![0, w, p[5], p[6]];
            vst.extend_from_slice(&p[7..13]);
            vst.push(n);
            let mut l0 = vec![0, p[0], p[1], w, h, n, 1];
            l0.extend_from_slice(&p[13..17]);
            l0.extend([1, p[17]]);
            for (name, words) in [("detail_vst_v1", vst), ("detail_atrous_v2", l0)] {
                for ty in 0..h {
                    for tx in 0..w {
                        step(name, &mut s, &words, img, tx, ty);
                    }
                }
            }
            continue;
        }
        for ty in 0..th {
            for tx in 0..tw {
                step(st.kernel.name, &mut s, p, img, tx, ty);
            }
        }
    }
    s
}

fn step(name: &str, s: &mut [f32], p: &[u32], img: &Img<'_>, tx: u32, ty: u32) {
    match name {
        "detail_vst_v1" => {
            let c = img.at(i64::from(p[2] + tx), i64::from(p[3] + ty));
            let r = gat(c[0], f(p, 4), f(p, 7));
            let g = gat(c[1], f(p, 5), f(p, 8));
            let b = gat(c[2], f(p, 6), f(p, 9));
            let n = p[10];
            let i = (p[0] + ty * p[1] + tx) as usize;
            s[i] = (r + g + b) * 0.577_350_26;
            s[i + n as usize] = (r - b) * 0.707_106_77;
            s[i + 2 * n as usize] = (r - 2.0 * g + b) * 0.408_248_3;
        }
        "detail_atrous_wg_v1" => {
            let (x, y) = super::kernels::atrous_wg_pixel(tx, ty, p[6]);
            if x < p[3] && y < p[4] {
                step("detail_atrous_v2", s, p, img, x, y);
            }
        }
        "detail_atrous_v2" => {
            let (co, no, ao, w, h, n) = (p[0], p[1], p[2], p[3], p[4], p[5]);
            let st = p[6] as i32;
            let ir = f(p, 10);
            let i = (ty * w + tx) as usize;
            let get = |o: u32| s[o as usize + i];
            let c = [get(co), get(co + n), get(co + 2 * n)];
            let mut acc = [0f32; 3];
            let mut ws = 0f32;
            let hw = |a: i32| match a.abs() {
                0 => 0.375f32,
                1 => 0.25,
                _ => 0.0625,
            };
            for j in -2i32..=2 {
                let y = ty as i32 + j * st;
                for k in -2i32..=2 {
                    let x = tx as i32 + k * st;
                    let q = [
                        pt(s, co, w, h, x, y),
                        pt(s, co + n, w, h, x, y),
                        pt(s, co + 2 * n, w, h, x, y),
                    ];
                    let d = [q[0] - c[0], q[1] - c[1], q[2] - c[2]];
                    let wt =
                        hw(k) * hw(j) * (-(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) * ir).exp();
                    for ch in 0..3 {
                        acc[ch] += wt * q[ch];
                    }
                    ws += wt;
                }
            }
            let nx = acc.map(|v| v / ws);
            let th = [f(p, 7), f(p, 8), f(p, 9)];
            let mut sh = [0f32; 3];
            for ch in 0..3 {
                let dt = c[ch] - nx[ch];
                sh[ch] = dt.signum() * (dt.abs() - th[ch]).max(0.0);
                if dt == 0.0 {
                    sh[ch] = 0.0;
                }
                if p[12] != 0 {
                    sh[ch] += nx[ch];
                }
                if p[11] == 0 {
                    sh[ch] += s[(ao + ch as u32 * n) as usize + i];
                }
            }
            for ch in 0..3u32 {
                if p[12] == 0 {
                    s[(no + ch * n) as usize + i] = nx[ch as usize];
                }
                s[(ao + ch * n) as usize + i] = sh[ch as usize];
            }
        }
        "detail_nlm_v1" | "detail_nlm_wg_v2" if tx < p[2] && ty < p[3] => {
            let (so, w, h) = (p[0], p[2], p[3]);
            let (sr, pr) = (p[4] as i32, p[5] as i32);
            let ih = f(p, 6);
            let (x, y) = (tx as i32, ty as i32);
            let np = ((2 * pr + 1) * (2 * pr + 1)) as f32;
            let mut acc = 0f32;
            let mut ws = 0f32;
            for dy in -sr..=sr {
                for dx in -sr..=sr {
                    let mut d2 = 0f32;
                    for py in -pr..=pr {
                        for px in -pr..=pr {
                            let e = pt(s, so, w, h, x + px, y + py)
                                - pt(s, so, w, h, x + dx + px, y + dy + py);
                            d2 += e * e;
                        }
                    }
                    let wt = (-(d2 / np) * ih).exp();
                    acc += wt * pt(s, so, w, h, x + dx, y + dy);
                    ws += wt;
                }
            }
            s[(p[1] + ty * w + tx) as usize] = acc / ws;
        }
        "detail_lum_v1" => {
            let c = img.at(i64::from(p[3] + tx), i64::from(p[4] + ty));
            let v = dot(c).max(f(p, 5));
            let i = ty * p[2] + tx;
            s[(p[0] + i) as usize] = v;
            s[(p[1] + i) as usize] = v;
        }
        "detail_blur_v1" => {
            let (so, w, h) = (p[0], p[2], p[3]);
            let vert = p[4] != 0;
            let r = p[5] as i32;
            let mode = p[6];
            let (x, y) = (tx as i32, ty as i32);
            let mut acc = f(p, 13) * pt(s, so, w, h, x, y);
            for k in 1..=r {
                let wt = f(p, 13 + k as usize);
                acc += if vert {
                    wt * (pt(s, so, w, h, x, y - k) + pt(s, so, w, h, x, y + k))
                } else {
                    wt * (pt(s, so, w, h, x - k, y) + pt(s, so, w, h, x + k, y))
                };
            }
            let i = ty * w + tx;
            let d_o = (p[1] + i) as usize;
            let damp = f(p, 12);
            s[d_o] = match mode {
                0 => acc,
                1 => {
                    (s[(p[7] + i) as usize] / acc.max(1e-12)).clamp(1.0 / (1.0 + damp), 1.0 + damp)
                }
                _ => {
                    let lo = s[(p[9] + i) as usize];
                    let hi = s[(p[10] + i) as usize];
                    let m = f(p, 11) * (hi - lo);
                    (s[(p[8] + i) as usize] * acc).clamp(lo - m, hi + m)
                }
            };
        }
        "detail_minmax_v1" => {
            let (so, w, h) = (p[0], p[3], p[4]);
            let vert = p[5] != 0;
            let r = p[6] as i32;
            let s2 = p[7];
            let (x, y) = (tx as i32, ty as i32);
            let mut lo = 3.0e38f32;
            let mut hi = -3.0e38f32;
            for k in -r..=r {
                if vert {
                    lo = lo.min(pt(s, so, w, h, x, y + k));
                    hi = hi.max(pt(s, s2, w, h, x, y + k));
                } else {
                    let v = pt(s, so, w, h, x + k, y);
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
            }
            let i = ty * w + tx;
            s[(p[1] + i) as usize] = lo;
            s[(p[2] + i) as usize] = hi;
        }
        // Threads past the plane (the workgroup form rounds them up to 16).
        "detail_nlm_wg_v2" => {}
        other => panic!("no CPU reference for {other}"),
    }
}

/// A neighbourhood op on the CPU: its program over `input`, then its kernel over `out`.
fn node(
    img: &Img<'_>,
    input: Roi,
    out: Roi,
    prog: &Program,
    px: impl Fn(&[f32], [f32; 4], u32, u32) -> [f32; 4],
) -> Vec<f32> {
    let s = run_program(prog, img, input);
    let mut v = Vec::with_capacity(out.pixels() as usize * 4);
    for y in out.y..out.bottom() {
        for x in out.x..out.right() {
            let c = img.at(i64::from(x), i64::from(y));
            let i = (y - input.y) * input.w + (x - input.x);
            v.extend_from_slice(&px(&s, c, i, input.w * input.h));
        }
    }
    v
}

/// Denoise of `out` reading `input` (which must cover `out` plus the halo, clipped).
pub fn denoise(img: &Img<'_>, d: &Denoise, input: Roi, out: Roi, scale: f32) -> Vec<f32> {
    let prog = denoise::program(d, input, scale);
    let n = d.noise;
    let ia = n.a.map(|a| 1.0 / a);
    let s2: [f32; 3] = std::array::from_fn(|c| n.b[c] / (n.a[c] * n.a[c]));
    let m = d.luminance.max(d.colour).min(1.0);
    let nlm = d.nlm && d.luminance > 0.0;
    node(img, input, out, &prog, |s, c, i, n| {
        let (i, n) = (i as usize, n as usize);
        let yv = if nlm { s[i] } else { s[6 * n + i] };
        let u = s[7 * n + i];
        let v = s[8 * n + i];
        let dr = yv * 0.577_350_26 + u * 0.707_106_77 + v * 0.408_248_3;
        let dg = yv * 0.577_350_26 - v * 0.816_496_6;
        let db = yv * 0.577_350_26 - u * 0.707_106_77 + v * 0.408_248_3;
        [
            gat_inv(dr, ia[0], s2[0], m),
            gat_inv(dg, ia[1], s2[1], m),
            gat_inv(db, ia[2], s2[2], m),
            c[3],
        ]
    })
}

/// Capture sharpening of `out` reading `input`.
pub fn capture(img: &Img<'_>, c: &Capture, input: Roi, out: Roi, scale: f32) -> Vec<f32> {
    let prog = capture::program(c, input, scale);
    let on = capture::plan(c, scale).2 > 0;
    node(img, input, out, &prog, |s, px, i, n| {
        let yl = dot(px);
        if !on || yl <= 1e-6 {
            return px;
        }
        let d = s[i as usize];
        let u = s[(n + i) as usize];
        let nl = d + c.amount * (u - d);
        let k = nl / d;
        [px[0] * k, px[1] * k, px[2] * k, px[3]]
    })
}

/// Sharpen of `out` reading `input`.
pub fn sharpen(img: &Img<'_>, sh: &Sharpen, input: Roi, out: Roi, scale: f32) -> Vec<f32> {
    let prog = sharpen::program(sh, input, scale);
    let (_, _, rm, amount) = sharpen::plan(sh, scale);
    let (lo_t, hi_t) = (0.06 * sh.masking, 0.2 * sh.masking);
    let over = 0.2 * sh.detail;
    let (rw, rh) = (input.w, input.h);
    let mut v = Vec::with_capacity(out.pixels() as usize * 4);
    let s = run_program(&prog, img, input);
    let n = rw * rh;
    for y in out.y..out.bottom() {
        for x in out.x..out.right() {
            let c = img.at(i64::from(x), i64::from(y));
            let lx = (x - input.x) as i32;
            let ly = (y - input.y) as i32;
            let at = |o: u32, dx: i32, dy: i32| pt(&s, o, rw, rh, lx + dx, ly + dy);
            let yv = at(0, 0, 0);
            let b = at(2 * n, 0, 0);
            let gx = at(2 * n, 1, 0) - at(2 * n, -1, 0);
            let gy = at(2 * n, 0, 1) - at(2 * n, 0, -1);
            let e = (gx.abs() + gy.abs()) / (2.0 * (b.abs() + 1e-3));
            let mask = if hi_t > 0.0 {
                let t = ((e - lo_t) / (hi_t - lo_t)).clamp(0.0, 1.0);
                t * t * (3.0 - 2.0 * t)
            } else {
                1.0
            };
            let mut lo = 3.0e38f32;
            let mut hi = -3.0e38f32;
            for k in -(rm as i32)..=rm as i32 {
                lo = lo.min(at(3 * n, 0, k));
                hi = hi.max(at(4 * n, 0, k));
            }
            let o = over * (hi - lo);
            let ny = (yv + amount * mask * (yv - b)).clamp(lo - o, hi + o);
            let yl = dot(c);
            if yl <= 1e-9 {
                v.extend_from_slice(&c);
            } else {
                let k = (ny / yl).clamp(0.0, 8.0);
                v.extend_from_slice(&[c[0] * k, c[1] * k, c[2] * k, c[3]]);
            }
        }
    }
    v
}
