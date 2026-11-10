//! CPU references of the effects (tests only): the program kernels in Rust with the same
//! words and f32 arithmetic (tone kernels through [`crate::ops::tone::cpu`]), the
//! statistics grid as the engine makes it, and each op's kernel twin over a whole image.

use super::{dehaze, glow, graduated, grain, negative, vignette};
use crate::graph::{Program, stats_grid};
use crate::ops::tone::cpu::{self as tone_cpu, Img};

fn f(w: u32) -> f32 {
    f32::from_bits(w)
}

/// Runs `prog` with the image `img` as `src_at`; returns the scratch.
pub fn run_program(prog: &Program, img: Img<'_>) -> Vec<f32> {
    let mut s = vec![0f32; prog.words as usize];
    for step in &prog.steps {
        let p = &step.words;
        let [tx, ty] = step.threads;
        for y in 0..ty {
            for x in 0..tx {
                if !fx_step(step.kernel.name, p, &mut s, img, x, y) {
                    tone_cpu::run_step(step.kernel.name, p, &mut s, img, &[], x, y, [tx, ty]);
                }
            }
        }
    }
    s
}

fn fx_step(name: &str, p: &[u32], s: &mut [f32], img: Img<'_>, tx: u32, ty: u32) -> bool {
    let at = |x: u32, y: u32| img.at(i64::from(x), i64::from(y));
    match name {
        "fx_bright_v1" => {
            let c = at(tx, ty);
            let y = tone_cpu::dot([c[0], c[1], c[2]]).max(0.0);
            let (th, k) = (f(p[3]), f(p[4]));
            let q = (y - th + k).clamp(0.0, 2.0 * k);
            let soft = q * q / (4.0 * k + 1e-6);
            let m = soft.max(y - th) / y.max(1e-6);
            let i = (p[0] + ty * p[1] + tx) as usize;
            let ps = p[2] as usize;
            for k in 0..3 {
                s[i + k * ps] = c[k].max(0.0) * m;
            }
        }
        "fx_down_v1" => {
            let (w, h, fct) = (p[1], p[2], p[5]);
            let x1 = (tx * fct + fct).min(w);
            let y1 = (ty * fct + fct).min(h);
            let n = ((x1 - tx * fct) * (y1 - ty * fct)) as f32;
            for c in 0..p[6] {
                let mut sum = 0f32;
                for y in ty * fct..y1 {
                    for x in tx * fct..x1 {
                        sum += s[(p[0] + c * p[7] + y * w + x) as usize];
                    }
                }
                s[(p[3] + c * p[8] + ty * p[4] + tx) as usize] = sum / n;
            }
        }
        "fx_dark_v1" => {
            let c = at(tx, ty);
            let mut v = [c[0].max(0.0), c[1].max(0.0), c[2].max(0.0)];
            if p[0] == 1 {
                let a = p[3] as usize;
                for k in 0..3 {
                    v[k] /= s[a + k].max(1e-4);
                }
            }
            s[(p[1] + ty * p[2] + tx) as usize] = v[0].min(v[1]).min(v[2]);
        }
        "fx_min_v1" => {
            let (w, h, r) = (p[2], p[3], p[5] as i32);
            let mut m = 3.0e38f32;
            if p[4] == 0 {
                let lo = (tx as i32 - r).max(0);
                let hi = (tx as i32 + r).min(w as i32 - 1);
                for i in lo..=hi {
                    m = m.min(s[(p[0] + ty * w + i as u32) as usize]);
                }
            } else {
                let lo = (ty as i32 - r).max(0);
                let hi = (ty as i32 + r).min(h as i32 - 1);
                for i in lo..=hi {
                    m = m.min(s[(p[0] + i as u32 * w + tx) as usize]);
                }
            }
            s[(p[1] + ty * w + tx) as usize] = f(p[6]) + f(p[7]) * m;
        }
        "fx_atmos_rows_v1" => {
            let y = tx;
            let w = p[1];
            let th = s[p[2] as usize] - f(p[4]);
            let mut acc = [0f32; 4];
            for x in 0..w {
                if s[(p[0] + y * w + x) as usize] >= th {
                    let c = at(x, y);
                    for k in 0..3 {
                        acc[k] += c[k].max(0.0);
                    }
                    acc[3] += 1.0;
                }
            }
            let o = (p[3] + 4 * y) as usize;
            s[o..o + 4].copy_from_slice(&acc);
        }
        "fx_atmos_v1" => {
            let mut acc = [0f32; 4];
            for y in 0..p[1] {
                let o = (p[0] + 4 * y) as usize;
                for k in 0..4 {
                    acc[k] += s[o + k];
                }
            }
            let a: [f32; 3] = std::array::from_fn(|k| (acc[k] / acc[3].max(1.0)).max(1e-3));
            let d = p[2] as usize;
            s[d..d + 3].copy_from_slice(&a);
            s[d + 3] = tone_cpu::dot(a);
        }
        "fx_guide_v1" => {
            let c = at(tx, ty);
            let y = tone_cpu::dot([c[0], c[1], c[2]]);
            s[(p[0] + ty * p[1] + tx) as usize] = (y / s[p[2] as usize].max(1e-4)).clamp(0.0, 2.0);
        }
        _ => return false,
    }
    true
}

/// The engine's statistics grid of `img` (area means, at most 1024 px a side).
pub fn grid(img: Img<'_>) -> (u32, u32, Vec<f32>) {
    let (gw, gh) = stats_grid(img.w, img.h);
    let mut s = vec![0f32; (gw * gh * 4) as usize];
    for y in 0..gh {
        for x in 0..gw {
            tone_cpu::run_step("grid_reduce_v1", &[], &mut s, img, &[], x, y, [gw, gh]);
        }
    }
    (gw, gh, s)
}

fn each(img: Img<'_>, mut f: impl FnMut([f32; 4], i32, i32) -> [f32; 4]) -> Vec<f32> {
    let mut out = Vec::with_capacity(img.px.len());
    for y in 0..img.h {
        for x in 0..img.w {
            out.extend_from_slice(&f(img.at(i64::from(x), i64::from(y)), x as i32, y as i32));
        }
    }
    out
}

pub fn vignette(v: &vignette::Vignette, img: Img<'_>) -> Vec<f32> {
    let u = v.uniforms();
    each(img, |c, x, y| {
        vignette::cpu_pixel(&u, c, x, y, img.w, img.h)
    })
}

pub fn graduated(g: &graduated::Graduated, img: Img<'_>) -> Vec<f32> {
    let u = g.uniforms();
    each(img, |c, x, y| {
        graduated::cpu_pixel(&u, c, x, y, img.w, img.h)
    })
}

pub fn grain(g: &grain::Grain, img: Img<'_>) -> Vec<f32> {
    let u = g.uniforms();
    each(img, |c, x, y| grain::cpu_pixel(&u, c, x, y, img.w, img.h))
}

pub fn negative(n: &negative::Negative, img: Img<'_>) -> Vec<f32> {
    let u = n.uniforms();
    each(img, |c, _, _| negative::cpu_pixel(&u, c))
}

/// The statistics of a global op on `img`'s grid.
pub fn stats(img: Img<'_>, prog: impl Fn(u32, u32) -> Program) -> Vec<f32> {
    let (gw, gh, g) = grid(img);
    let gi = Img {
        w: gw,
        h: gh,
        px: &g,
    };
    run_program(&prog(gw, gh), gi)
}

pub fn glow(gl: &glow::Glow, img: Img<'_>) -> Vec<f32> {
    let st = stats(img, |w, h| glow::stats_program(gl, w, h));
    let n = st[0] as u32 * st[1] as u32;
    each(img, |c, x, y| {
        let g = std::array::from_fn(|k| {
            tone_cpu::grid(&st, glow::PLANES + k as u32 * n, x, y, img.w, img.h)
        });
        glow::cpu_pixel(gl.amount, gl.mode == glow::GlowMode::Soften, c, g)
    })
}

pub fn dehaze(d: &dehaze::Dehaze, img: Img<'_>) -> (Vec<f32>, Vec<f32>) {
    let st = stats(img, dehaze::stats_program);
    let n = st[0] as u32 * st[1] as u32;
    let o = dehaze::STATS_COEFF;
    let out = each(img, |c, x, y| {
        let ab = [
            tone_cpu::grid(&st, o, x, y, img.w, img.h),
            tone_cpu::grid(&st, o + n, x, y, img.w, img.h),
        ];
        dehaze::cpu_pixel(d.strength, c, &st, ab)
    });
    (out, st)
}
