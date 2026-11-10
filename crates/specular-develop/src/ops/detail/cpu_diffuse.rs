//! CPU references of the diffuse and surface blur ops (tests only): diffuse or sharpen and surface
//! blur on a whole RGBA f32 image, written from the module docs, for GPU = CPU checks.

use super::diffuse::{self, Diffuse};
use super::kernels::gauss_taps;
use super::surface::{self, LMAX, PAD, SurfaceBlur};

const Y: [f32; 3] = [0.2627, 0.6780, 0.0593];

fn lum(c: [f32; 3]) -> f32 {
    c[0] * Y[0] + c[1] * Y[1] + c[2] * Y[2]
}

/// An RGB image of planes.
#[derive(Clone)]
struct Rgb {
    w: usize,
    h: usize,
    px: Vec<[f32; 3]>,
}

impl Rgb {
    fn at(&self, x: i64, y: i64) -> [f32; 3] {
        let cx = x.clamp(0, self.w as i64 - 1) as usize;
        let cy = y.clamp(0, self.h as i64 - 1) as usize;
        self.px[cy * self.w + cx]
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn term(xx: [f32; 3], yy: [f32; 3], xy: [f32; 3], g: [f32; 2], e: f32, a: f32) -> [f32; 3] {
    let gl = (g[0] * g[0] + g[1] * g[1]).sqrt();
    let n = if gl > 1e-12 {
        [g[0] / gl, g[1] / gl]
    } else {
        [1.0, 0.0]
    };
    let wn = 1.0 - a.max(0.0) * (1.0 - e);
    let wt = 1.0 - (-a).max(0.0) * (1.0 - e);
    std::array::from_fn(|c| {
        let nn = n[0] * n[0] * xx[c] + 2.0 * n[0] * n[1] * xy[c] + n[1] * n[1] * yy[c];
        let tt = xx[c] + yy[c] - nn;
        wn * nn + wt * tt
    })
}

/// A level stored on grid `at` (every `2^at`-th pixel), read at pixel (`x`, `y`) of grid
/// `by` (`gw`×`gh`, `by ≥ at`), clamped to grid `by`.
#[allow(clippy::too_many_arguments)]
fn tap(l: &Rgb, at: u32, by: u32, gw: usize, gh: usize, x: i64, y: i64) -> [f32; 3] {
    let d = by - at;
    let cx = (x.clamp(0, gw as i64 - 1) << d) as usize;
    let cy = (y.clamp(0, gh as i64 - 1) << d) as usize;
    l.px[cy * l.w + cx]
}

/// The change of one band at a pixel from its 3×3 taps of `L_{j+1}` and `H_j`.
fn band(lf: &[[f32; 3]; 9], hf: &[[f32; 3]; 9], d: &Diffuse, pl: &diffuse::Plan) -> [f32; 3] {
    let d2 = |f: &[[f32; 3]; 9]| {
        let xx: [f32; 3] = std::array::from_fn(|c| f[5][c] - 2.0 * f[4][c] + f[3][c]);
        let yy: [f32; 3] = std::array::from_fn(|c| f[7][c] - 2.0 * f[4][c] + f[1][c]);
        let xy: [f32; 3] = std::array::from_fn(|c| 0.25 * (f[8][c] - f[6][c] - f[2][c] + f[0][c]));
        let g = [0.5 * lum(sub(f[5], f[3])), 0.5 * lum(sub(f[7], f[1]))];
        (xx, yy, xy, g)
    };
    let (lxx, lyy, lxy, gl) = d2(lf);
    let (hxx, hyy, hxy, gh) = d2(hf);
    let lev = lum(lf[4]).max(0.0) + 1e-3;
    let rl = (gl[0] * gl[0] + gl[1] * gl[1]).sqrt() / lev;
    let rh = (gh[0] * gh[0] + gh[1] * gh[1]).sqrt() / lev;
    let el = 1.0 / (1.0 + rl * pl.inv_k * rl * pl.inv_k);
    let eh = 1.0 / (1.0 + rh * pl.inv_k * rh * pl.inv_k);
    let a = d.anisotropy;
    let terms = [
        term(lxx, lyy, lxy, gl, el, a[0]),
        term(lxx, lyy, lxy, gh, eh, a[1]),
        term(hxx, hyy, hxy, gl, el, a[2]),
        term(hxx, hyy, hxy, gh, eh, a[3]),
    ];
    let gate = if pl.threshold > 0.0 {
        smoothstep(pl.threshold, 2.0 * pl.threshold, rl)
    } else {
        1.0
    };
    let mut up = [0f32; 3];
    for (k, t) in terms.iter().enumerate() {
        let sp = d.speed[k];
        let f = if sp > 0.0 { sp } else { sp * gate };
        for c in 0..3 {
            up[c] += f * t[c];
        }
    }
    up
}

/// Bilinear: the sum on grid `m + 1` (`c`) at pixel (`x`, `y`) of grid `m` (its samples
/// are every other pixel here, from 0 in a whole image).
fn upsample(c: &Rgb, x: usize, y: usize) -> [f32; 3] {
    let fx = (x as f32 * 0.5).clamp(0.0, (c.w - 1) as f32);
    let fy = (y as f32 * 0.5).clamp(0.0, (c.h - 1) as f32);
    let (ix, iy) = (fx as usize, fy as usize);
    let (ax, ay) = (fx - ix as f32, fy - iy as f32);
    let (jx, jy) = ((ix + 1).min(c.w - 1), (iy + 1).min(c.h - 1));
    let at = |x: usize, y: usize| c.px[y * c.w + x];
    std::array::from_fn(|k| {
        let a = at(ix, iy)[k] * (1.0 - ax) + at(jx, iy)[k] * ax;
        let b = at(ix, jy)[k] * (1.0 - ax) + at(jx, jy)[k] * ax;
        a * (1.0 - ay) + b * ay
    })
}

/// Diffuse or sharpen of a whole `w`×`h` RGBA image (alpha kept), coarse bands on their
/// reduced grids as the GPU program runs them ([`diffuse::band_grids`]).
pub fn diffuse(img: &[f32], w: u32, h: u32, d: &Diffuse) -> Vec<f32> {
    const B: [f32; 5] = [0.0625, 0.25, 0.375, 0.25, 0.0625];
    let pl = diffuse::plan(d);
    let bg = diffuse::band_grids(&pl, w, h);
    let (w, h) = (w as usize, h as usize);
    let top = bg.iter().copied().max().unwrap_or(0);
    let dims: Vec<(usize, usize)> = (0..=top)
        .map(|m| (w.div_ceil(1 << m), h.div_ceil(1 << m)))
        .collect();
    let mut u = Rgb {
        w,
        h,
        px: img.chunks_exact(4).map(|p| [p[0], p[1], p[2]]).collect(),
    };
    for _ in 0..pl.iterations {
        // L_0 = u on grid 0, then L_{j+1} on band j's grid.
        let mut lv: Vec<(u32, Rgb)> = vec![(0, u.clone())];
        for (j, &m) in bg.iter().enumerate() {
            let (gw, gh) = dims[m as usize];
            let st = (1i64 << j) >> m;
            let (at, src) = &lv[j];
            let mut out = Rgb {
                w: gw,
                h: gh,
                px: vec![[0.0; 3]; gw * gh],
            };
            for y in 0..gh {
                for x in 0..gw {
                    let mut acc = [0f32; 3];
                    for jj in -2i64..=2 {
                        let mut row = [0f32; 3];
                        for kk in -2i64..=2 {
                            let (sx, sy) = (x as i64 + kk * st, y as i64 + jj * st);
                            let v = tap(src, *at, m, gw, gh, sx, sy);
                            for c in 0..3 {
                                row[c] += B[(kk + 2) as usize] * v[c];
                            }
                        }
                        for c in 0..3 {
                            acc[c] += B[(jj + 2) as usize] * row[c];
                        }
                    }
                    out.px[y * gw + x] = acc;
                }
            }
            lv.push((m, out));
        }
        // Each grid's bands, coarsest grid first; each adds the coarser grid's sum.
        let mut coarser: Option<Rgb> = None;
        for m in (0..=top).rev() {
            let (gw, gh) = dims[m as usize];
            let mut sum = vec![[0f32; 3]; gw * gh];
            for y in 0..gh {
                for x in 0..gw {
                    let (xi, yi) = (x as i64, y as i64);
                    let mut acc = [0f32; 3];
                    let mut any = false;
                    for (j, &wt) in pl.weights.iter().enumerate() {
                        if wt <= 0.0 || bg[j] != m {
                            continue;
                        }
                        let s = (1i64 << j) >> m;
                        let (pa, p) = &lv[j];
                        let q = &lv[j + 1].1;
                        let mut lf = [[0f32; 3]; 9];
                        let mut hf = [[0f32; 3]; 9];
                        for jj in 0..3i64 {
                            for kk in 0..3i64 {
                                let (sx, sy) = (xi + (kk - 1) * s, yi + (jj - 1) * s);
                                let qv = tap(q, m, m, gw, gh, sx, sy);
                                lf[(jj * 3 + kk) as usize] = qv;
                                hf[(jj * 3 + kk) as usize] =
                                    sub(tap(p, *pa, m, gw, gh, sx, sy), qv);
                            }
                        }
                        let mut up = band(&lf, &hf, d, &pl);
                        if d.highlights > 0.0 {
                            // Highlights keep every band on grid 0.
                            let c = u.px[y * w + x];
                            let k = smoothstep(
                                0.8 * d.highlights,
                                d.highlights,
                                c[0].max(c[1].max(c[2])),
                            );
                            up = up.map(|v| v * k);
                        }
                        for c in 0..3 {
                            let v = up[c] * (wt * pl.dt);
                            acc[c] = if any { v + acc[c] } else { v };
                        }
                        any = true;
                    }
                    if let Some(cs) = &coarser {
                        let v = upsample(cs, x, y);
                        for c in 0..3 {
                            acc[c] += v[c];
                        }
                    }
                    sum[y * gw + x] = acc;
                }
            }
            coarser = Some(Rgb {
                w: gw,
                h: gh,
                px: sum,
            });
        }
        let acc = coarser.expect("grid 0").px;
        let (hf, hc) = (1i64 << pl.first, 1i64 << pl.last());
        let mut next = u.clone();
        for y in 0..h {
            for x in 0..w {
                let mut lo = [f32::MAX; 3];
                let mut hi = [f32::MIN; 3];
                for s in [hf, hc] {
                    for j in -1..=1 {
                        for k in -1..=1 {
                            let v = u.at(x as i64 + k * s, y as i64 + j * s);
                            for c in 0..3 {
                                lo[c] = lo[c].min(v[c]);
                                hi[c] = hi[c].max(v[c]);
                            }
                        }
                    }
                }
                let i = y * w + x;
                next.px[i] = std::array::from_fn(|c| {
                    let m = pl.margin * (hi[c] - lo[c]);
                    (u.px[i][c] + acc[i][c]).clamp((lo[c] - m).max(lo[c].min(0.0)), hi[c] + m)
                });
            }
        }
        u = next;
    }
    u.px.iter()
        .zip(img.chunks_exact(4))
        .flat_map(|(c, p)| [c[0], c[1], c[2], p[3]])
        .collect()
}

fn lightness(c: [f32; 3]) -> f32 {
    lum(c).max(1e-12).powf(1.0 / 3.0).min(LMAX)
}

/// Surface blur of a whole `w`×`h` RGBA image (alpha kept).
pub fn surface_blur(img: &[f32], w: u32, h: u32, s: &SurfaceBlur) -> Vec<f32> {
    let (wu, hu) = (w as usize, h as usize);
    let src = Rgb {
        w: wu,
        h: hu,
        px: img.chunks_exact(4).map(|p| [p[0], p[1], p[2]]).collect(),
    };
    let ir = 1.0 / s.edges;
    let blurred: Vec<[f32; 3]> = match surface::plan(s) {
        surface::Plan::Direct {
            sigma,
            radius,
            step,
        } => {
            let (r, st) = (radius as i64, step as usize);
            let is2 = 0.5 / (sigma * sigma);
            let ir2 = 0.5 * ir * ir;
            (0..hu * wu)
                .map(|i| {
                    let (x, y) = ((i % wu) as i64, (i / wu) as i64);
                    let l0 = lightness(src.px[i]);
                    let mut acc = [0f32; 4];
                    for j in (-r..=r).step_by(st) {
                        for k in (-r..=r).step_by(st) {
                            let v = src.at(x + k, y + j);
                            let dl = lightness(v) - l0;
                            let wt = (-((j * j + k * k) as f32) * is2 - dl * dl * ir2).exp();
                            for c in 0..3 {
                                acc[c] += wt * v[c];
                            }
                            acc[3] += wt;
                        }
                    }
                    [acc[0] / acc[3], acc[1] / acc[3], acc[2] / acc[3]]
                })
                .collect()
        }
        surface::Plan::Grid { sigma, cell, bins } => {
            let roi = crate::graph::Roi::full(w, h);
            let (gx0, gy0, gw, gh) = surface::grid_extent(roi, cell);
            let (gw, gh, nb, c) = (gw as usize, gh as usize, bins as usize, cell as i64);
            let idx = |x: usize, y: usize, z: usize| (z * gh + y) * gw + x;
            let mut g = vec![[0f32; 4]; gw * gh * nb];
            for (z, row) in g.chunks_mut(gw * gh).enumerate() {
                for gy in 0..gh {
                    for gx in 0..gw {
                        let ax = gx0 as i64 + gx as i64;
                        let ay = gy0 as i64 + gy as i64;
                        let mut acc = [0f32; 4];
                        for y in (ay * c).max(0)..(ay * c + c).min(h as i64) {
                            for x in (ax * c).max(0)..(ax * c + c).min(w as i64) {
                                let v = src.px[y as usize * wu + x as usize];
                                let zf = lightness(v) * ir + PAD as f32;
                                let wt = (1.0 - (zf - z as f32).abs()).max(0.0);
                                for k in 0..3 {
                                    acc[k] += wt * v[k];
                                }
                                acc[3] += wt;
                            }
                        }
                        row[gy * gw + gx] = acc;
                    }
                }
            }
            let space = gauss_taps(sigma / cell as f32, PAD);
            let range = gauss_taps(1.0, PAD);
            for axis in 0..3 {
                let taps = if axis == 2 { &range } else { &space };
                let mut out = vec![[0f32; 4]; g.len()];
                for z in 0..nb {
                    for y in 0..gh {
                        for x in 0..gw {
                            let mut acc = [0f32; 4];
                            for k in -(PAD as i64)..=PAD as i64 {
                                let (mut qx, mut qy, mut qz) = (x as i64, y as i64, z as i64);
                                match axis {
                                    0 => qx += k,
                                    1 => qy += k,
                                    _ => qz += k,
                                }
                                if qx < 0
                                    || qy < 0
                                    || qz < 0
                                    || qx >= gw as i64
                                    || qy >= gh as i64
                                    || qz >= nb as i64
                                {
                                    continue;
                                }
                                let t = taps[k.unsigned_abs() as usize];
                                let v = g[idx(qx as usize, qy as usize, qz as usize)];
                                for e in 0..4 {
                                    acc[e] += t * v[e];
                                }
                            }
                            out[idx(x, y, z)] = acc;
                        }
                    }
                }
                g = out;
            }
            (0..hu * wu)
                .map(|i| {
                    let (x, y) = (i % wu, i / wu);
                    let v = src.px[i];
                    let fx = (x as f32 + 0.5) / c as f32 - 0.5 - gx0 as f32;
                    let fy = (y as f32 + 0.5) / c as f32 - 0.5 - gy0 as f32;
                    let fz = lightness(v) * ir + PAD as f32;
                    let (ix, iy, iz) = (fx.floor(), fy.floor(), fz.floor());
                    let (tx, ty, tz) = (fx - ix, fy - iy, fz - iz);
                    let mut acc = [0f32; 4];
                    for c8 in 0..8 {
                        let (dx, dy, dz) = (c8 & 1, (c8 >> 1) & 1, (c8 >> 2) & 1);
                        let qx = (ix as i64 + dx).clamp(0, gw as i64 - 1) as usize;
                        let qy = (iy as i64 + dy).clamp(0, gh as i64 - 1) as usize;
                        let qz = (iz as i64 + dz).clamp(0, nb as i64 - 1) as usize;
                        let wt = if dx == 1 { tx } else { 1.0 - tx }
                            * if dy == 1 { ty } else { 1.0 - ty }
                            * if dz == 1 { tz } else { 1.0 - tz };
                        let q = g[idx(qx, qy, qz)];
                        for e in 0..4 {
                            acc[e] += wt * q[e];
                        }
                    }
                    if acc[3] > 1e-12 {
                        [acc[0] / acc[3], acc[1] / acc[3], acc[2] / acc[3]]
                    } else {
                        v
                    }
                })
                .collect()
        }
    };
    blurred
        .iter()
        .zip(img.chunks_exact(4))
        .flat_map(|(b, p)| {
            let m = |k: usize| p[k] + s.amount * (b[k] - p[k]);
            [m(0), m(1), m(2), p[3]]
        })
        .collect()
}
