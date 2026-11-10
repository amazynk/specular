//! Inverse colour transforms (ISO/IEC 21122-1 Annex F: the reversible colour transform and
//! the Star-Tetrix transform of colour filter array images) and the output scaling with the
//! optional non-linear transforms (Annex G).

use crate::header::{Cfa, Nlt, Tetrix};
use rayon::prelude::*;

/// Inverse reversible colour transform: planes 0, 1, 2 become red, green and blue.
pub(crate) fn inverse_rct(planes: &mut [Vec<i32>]) {
    let [y, u, v, ..] = planes else {
        return;
    };
    const CHUNK: usize = 1 << 14;
    y.par_chunks_mut(CHUNK)
        .zip(u.par_chunks_mut(CHUNK))
        .zip(v.par_chunks_mut(CHUNK))
        .for_each(|((y, u), v)| {
            for ((c0, c1), c2) in y.iter_mut().zip(u.iter_mut()).zip(v.iter_mut()) {
                let g = c0.wrapping_sub(c1.wrapping_add(*c2) >> 2);
                let r = g.wrapping_add(*c2);
                let b = g.wrapping_add(*c1);
                *c0 = r;
                *c1 = g;
                *c2 = b;
            }
        });
}

/// A sample's neighbour at a mosaic offset.
type Neighbour<'a> = &'a dyn Fn(isize, isize) -> i32;

/// Where component `n` of a Star-Tetrix codestream sits in each 2×2 cell of the mosaic.
fn cell(n: usize, cfa: Cfa) -> (usize, usize) {
    let ct = usize::from(!cfa.greens_anti_diagonal());
    ((n & 1) ^ ct, 1 - (n >> 1))
}

/// Inverse Star-Tetrix transform: the four `w`×`h` component planes become one 2w×2h
/// colour filter array mosaic laid out as `cfa`.
pub(crate) fn inverse_tetrix(
    planes: &[Vec<i32>],
    w: usize,
    h: usize,
    cfa: Cfa,
    t: Tetrix,
) -> Vec<i32> {
    let (mw, mh) = (2 * w, 2 * h);
    let mut m = vec![0i32; mw * mh];
    for (n, p) in planes.iter().take(4).enumerate() {
        let (dx, dy) = cell(n, cfa);
        for y in 0..h {
            let row = &mut m[(2 * y + dy) * mw..(2 * y + dy + 1) * mw];
            for (x, &v) in p[y * w..(y + 1) * w].iter().enumerate() {
                row[2 * x + dx] = v;
            }
        }
    }
    if w == 0 || h == 0 {
        return m;
    }
    // Neighbour coordinates mirrored at the edges (and, in-line, inside the row pair).
    let nx = |x: usize, dx: isize| -> usize {
        let v = x as isize + dx;
        if v < 0 {
            (v + 2) as usize
        } else if v >= mw as isize {
            (v - 2) as usize
        } else {
            v as usize
        }
    };
    let ny = |y: usize, dy: isize| -> usize {
        if dy == 0 {
            return y;
        }
        if t.inline {
            return y ^ 1;
        }
        let v = y as isize + dy;
        if v < 0 {
            (v + 2) as usize
        } else if v >= mh as isize {
            (v - 2) as usize
        } else {
            v as usize
        }
    };
    let (e1, e2) = (u32::from(t.e1), u32::from(t.e2));
    // Applies `f(sample, neighbours)` to every sample of component `n`.
    let step = |m: &mut Vec<i32>, n: usize, f: &dyn Fn(i32, Neighbour) -> i32| {
        let (dx, dy) = cell(n, cfa);
        for cy in 0..h {
            let y = 2 * cy + dy;
            for cx in 0..w {
                let x = 2 * cx + dx;
                let get = |ox: isize, oy: isize| m[ny(y, oy) * mw + nx(x, ox)];
                let v = f(m[y * mw + x], &get);
                m[y * mw + x] = v;
            }
        }
    };
    let diag = |g: Neighbour| {
        g(-1, -1)
            .wrapping_add(g(1, -1))
            .wrapping_add(g(-1, 1))
            .wrapping_add(g(1, 1))
    };
    let horiz = |g: Neighbour| g(-1, 0).wrapping_add(g(1, 0));
    let vert = |g: Neighbour| g(0, -1).wrapping_add(g(0, 1));
    // Average, delta, the two greens, then the two colour differences.
    step(&mut m, 0, &|s, g| s.wrapping_sub(diag(g) >> 3));
    step(&mut m, 3, &|s, g| s.wrapping_add(diag(g) >> 2));
    step(&mut m, 0, &|s, g| {
        s.wrapping_sub(
            horiz(g)
                .wrapping_shl(e2)
                .wrapping_add(vert(g).wrapping_shl(e1))
                >> 3,
        )
    });
    step(&mut m, 3, &|s, g| {
        s.wrapping_sub(
            vert(g)
                .wrapping_shl(e2)
                .wrapping_add(horiz(g).wrapping_shl(e1))
                >> 3,
        )
    });
    for n in [1, 2] {
        step(&mut m, n, &|s, g| {
            s.wrapping_add(horiz(g).wrapping_add(vert(g)) >> 2)
        });
    }
    m
}

/// Scales reconstructed values (`bw` bits, centred on zero) to `depth`-bit samples, through
/// the non-linear transform when there is one.
pub(crate) fn output(v: &[i32], out: &mut [u16], bw: u8, depth: u8, nlt: Nlt) {
    let max = (1i64 << depth) - 1;
    let bw = u32::from(bw);
    let depth = u32::from(depth);
    let dclev = (1i64 << bw) >> 1;
    let max_coef = (1i64 << bw) - 1;
    const CHUNK: usize = 1 << 14;
    let mut work = |f: &(dyn Fn(i64) -> i64 + Sync)| {
        out.par_chunks_mut(CHUNK)
            .zip(v.par_chunks(CHUNK))
            .for_each(|(o, i)| {
                for (o, &i) in o.iter_mut().zip(i) {
                    *o = f(i as i64).clamp(0, max) as u16;
                }
            });
    };
    match nlt {
        Nlt::Linear => {
            let s = bw - depth;
            let add = dclev + ((1i64 << s) >> 1);
            work(&|x| (x + add) >> s);
        }
        Nlt::Quadratic { sigma, alpha } => {
            let vdco = alpha as i64 - if sigma { 32768 } else { 0 };
            let s = 2 * bw - depth;
            let sr = (1i64 << s) >> 1;
            work(&|x| {
                let y = (x + dclev).clamp(0, max_coef);
                ((y * y + sr) >> s) + vdco
            });
        }
        Nlt::Extended { t1, t2, e } => {
            let e = bw - u32::from(e);
            let (t1, t2) = (t1 as i64, t2 as i64);
            let b2 = t1 * t1;
            let a1 = b2 + (t1 << e) + (1i64 << (2 * e - 2));
            let b1 = t1 + (1i64 << (e - 1));
            let a3 = b2 + (t2 << e) - (1i64 << (2 * e - 2));
            let b3 = t2 - (1i64 << (e - 1));
            let s = 2 * bw - depth;
            let sr = (1i64 << s) >> 1;
            work(&|x| {
                let v = x + dclev;
                let y = if v < t1 {
                    let d = (b1 - v).clamp(0, max_coef);
                    a1 - d * d
                } else if v < t2 {
                    (v << e) + b2
                } else {
                    let d = (v - b3).clamp(0, max_coef);
                    a3 + d * d
                };
                (y + sr) >> s
            });
        }
    }
}
