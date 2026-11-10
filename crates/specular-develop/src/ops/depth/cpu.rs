//! CPU twins of the depth kernels (tests only): the same f32 arithmetic per pixel.

use super::blur;
use crate::mask::depth::DepthMap;

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    crate::ops::effects::smoothstep(e0, e1, x)
}

/// Relight over an RGBA image with uniforms `u` ([`super::relight::Relight::uniforms`]).
pub fn relight(img: &[f32], w: u32, h: u32, map: &DepthMap, u: [f32; 4]) -> Vec<f32> {
    let mut out = img.to_vec();
    for y in 0..h {
        for x in 0..w {
            let t = map.at(x as i32, y as i32, w, h);
            if t < 0.0 {
                continue;
            }
            let s = smoothstep(u[2], u[3], t);
            let k = (u[0] + (u[1] - u[0]) * s).exp2();
            let i = 4 * (y * w + x) as usize;
            for c in 0..3 {
                out[i + c] *= k;
            }
        }
    }
    out
}

fn boost(u: &[f32; 6], c: &[f32]) -> f32 {
    let l = 0.2627 * c[0] + 0.6780 * c[1] + 0.0593 * c[2];
    1.0 + 3.0 * u[3] * smoothstep(0.7, 3.0, l)
}

/// The depth blur over an RGBA image with uniforms `u` ([`blur::uniforms`]).
pub fn depth_blur(img: &[f32], w: u32, h: u32, map: &DepthMap, u: [f32; 6]) -> Vec<f32> {
    let at = |x: i32, y: i32| {
        let (cx, cy) = (x.clamp(0, w as i32 - 1), y.clamp(0, h as i32 - 1));
        let i = 4 * (cy as u32 * w + cx as u32) as usize;
        &img[i..i + 4]
    };
    let n = u[5].to_bits();
    let r = u[0];
    let taps = blur::tap_table(r, n);
    let mut out = img.to_vec();
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let t0 = map.at(x, y, w, h);
            if t0 < 0.0 || r < 0.5 {
                continue;
            }
            let c0 = at(x, y);
            let coc0 = blur::coc(&u, t0);
            let sp = (r * (std::f32::consts::PI / n as f32).sqrt()).max(1.0);
            let wt0 =
                (coc0 / sp + 0.5).clamp(0.0, 1.0) / (coc0 * coc0).max(sp * sp) * boost(&u, c0);
            let mut acc = [c0[0] * wt0, c0[1] * wt0, c0[2] * wt0];
            let mut ws = wt0;
            let tb = (2 * n * blur::turn(x, y)) as usize;
            for i in 0..n as usize {
                let (ox, oy) = (taps[tb + 2 * i], taps[tb + 2 * i + 1]);
                let rho = (ox * ox + oy * oy).sqrt();
                let sx = x + ox as i32;
                let sy = y + oy as i32;
                if sx == x && sy == y {
                    continue;
                }
                let ts = map.at(sx, sy, w, h);
                let mut cs = blur::coc(&u, ts);
                if ts > t0 {
                    cs = cs.min(coc0);
                }
                let k = ((cs - rho) / sp + 0.5).clamp(0.0, 1.0);
                if k <= 0.0 {
                    continue;
                }
                let c = at(sx, sy);
                let wt = k / (cs * cs).max(sp * sp) * boost(&u, c);
                for ch in 0..3 {
                    acc[ch] += c[ch] * wt;
                }
                ws += wt;
            }
            let i = 4 * (y as u32 * w + x as u32) as usize;
            if ws > 0.0 {
                for ch in 0..3 {
                    out[i + ch] = acc[ch] / ws;
                }
            }
        }
    }
    out
}
