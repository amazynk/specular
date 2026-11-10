//! The merge itself, on frames already in the reference's geometry.
//!
//! - **Exposures** relative to the reference, measured on the data: the median ratio of
//!   same-place samples well inside both frames' ranges, chained between neighbouring
//!   exposures; the metadata only where the data is too thin.
//! - **Weights** after Granados et al., "Optimal HDR reconstruction with linear digital
//!   cameras" (CVPR 2010): each frame's radiance estimate `v/e` weighted by the inverse of its
//!   variance `(a·e·r + b)/e²` from the frame's raw noise model, so long exposures win in the
//!   shadows and every unclipped frame adds where they are all good. Saturated and
//!   near-saturated samples never count.
//! - **Ghosts**: on the block proxy, a frame whose radiance differs from the reference's by
//!   more than the noise explains (`|z| > threshold` and over 3 %) is left out there, with a
//!   dilated, feathered edge; where the reference is clipped the nearest unclipped exposure
//!   stands in for it. The union is the ghost mask.

use rayon::prelude::*;

use super::{GhostMask, Mosaic, Plane};

/// Raw noise per CFA colour: variance `a·x + b`, `x` normalised.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Noise {
    pub a: [f32; 3],
    pub b: [f32; 3],
}

/// The ratio `frame / base` of two aligned planes, from samples both hold well. `None`
/// when fewer than 200 such samples exist.
pub(crate) fn ratio(frame: &Plane, base: &Plane) -> Option<f64> {
    let step = (frame.v.len() / 400_000).max(1);
    let mut r: Vec<f32> = frame
        .v
        .par_iter()
        .zip(base.v.par_iter())
        .step_by(step)
        .filter_map(|(&a, &b)| {
            let ok = |v: f32| v.is_finite() && (0.01..=0.85).contains(&v);
            (ok(a) && ok(b)).then(|| a / b)
        })
        .collect();
    if r.len() < 200 {
        return None;
    }
    let mid = r.len() / 2;
    let (_, m, _) = r.select_nth_unstable_by(mid, f32::total_cmp);
    Some(f64::from(*m))
}

/// Exposures relative to frame `reference`: neighbours by `guess` (the metadata's, or the
/// planes' mean levels) measured against each other and chained.
pub(crate) fn exposures(
    planes: &[Plane],
    guess: &[Option<f64>],
    reference: usize,
) -> Option<Vec<f64>> {
    let n = planes.len();
    let level = |p: &Plane| {
        let (s, c) = p.v.iter().step_by(97).fold((0.0f64, 0usize), |(s, c), &v| {
            if v.is_finite() {
                (s + f64::from(v), c + 1)
            } else {
                (s + 1.0, c + 1)
            }
        });
        s / c.max(1) as f64
    };
    let key: Vec<f64> = (0..n)
        .map(|i| guess[i].unwrap_or_else(|| level(&planes[i]).max(1e-9)))
        .collect();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| key[a].total_cmp(&key[b]));
    // Log exposure of each frame relative to the first in order.
    let mut log = vec![0.0f64; n];
    for w in order.windows(2) {
        let (lo, hi) = (w[0], w[1]);
        let step = match ratio(&planes[hi], &planes[lo]) {
            Some(r) if r > 0.0 => r.ln(),
            _ => match (guess[hi], guess[lo]) {
                (Some(a), Some(b)) if a > 0.0 && b > 0.0 => (a / b).ln(),
                _ => return None,
            },
        };
        log[hi] = log[lo] + step;
    }
    Some((0..n).map(|i| (log[i] - log[reference]).exp()).collect())
}

/// Per-frame weights for the ghost-free merge on the block grid (1 keep, 0 leave out), and
/// the mask (255 where any frame was left out). Without `threshold` every frame is kept.
pub(crate) fn ghosts(
    planes: &[Plane],
    e: &[f64],
    noise: &[Noise],
    reference: usize,
    block: u32,
    threshold: Option<f32>,
) -> (Vec<Vec<f32>>, GhostMask) {
    let b = block.max(1) as usize;
    let p0 = &planes[reference];
    let (gw, gh) = ((p0.w as usize).div_ceil(b), (p0.h as usize).div_ceil(b));
    let n = planes.len();
    let Some(k) = threshold else {
        return (
            vec![vec![1.0; gw * gh]; n],
            GhostMask {
                width: gw as u32,
                height: gh as u32,
                block,
                mask: vec![0; gw * gh],
            },
        );
    };
    // Block radiance and its variance (green model), NaN where not usable.
    let blocks: Vec<(Vec<f32>, Vec<f32>)> = planes
        .par_iter()
        .enumerate()
        .map(|(i, p)| {
            let (w, cpp) = (p.w as usize, p.cpp as usize);
            let mut r = vec![f32::NAN; gw * gh];
            let mut var = vec![f32::NAN; gw * gh];
            let ei = e[i] as f32;
            for gy in 0..gh {
                for gx in 0..gw {
                    let mut s = 0.0f32;
                    let mut c = 0usize;
                    let mut ok = true;
                    for y in gy * b..((gy + 1) * b).min(p.h as usize) {
                        for x in gx * b..((gx + 1) * b).min(w) {
                            for ch in 0..cpp {
                                let v = p.v[(y * w + x) * cpp + ch];
                                if !v.is_finite() {
                                    ok = false;
                                }
                                s += v;
                                c += 1;
                            }
                        }
                    }
                    if ok && c > 0 {
                        let m = s / c as f32;
                        let rad = m / ei;
                        r[gy * gw + gx] = rad;
                        let nz = &noise[i];
                        var[gy * gw + gx] = (nz.a[1] * m.max(0.0) + nz.b[1]) / (ei * ei * c as f32);
                    }
                }
            }
            (r, var)
        })
        .collect();
    // Anchor per block: the reference, else the usable frame nearest its exposure.
    let mut by_closeness: Vec<usize> = (0..n).collect();
    by_closeness.sort_by(|&a, &c| {
        let da = e[a].ln().abs() + if e[a] > 1.0 { 1e-3 } else { 0.0 };
        let dc = e[c].ln().abs() + if e[c] > 1.0 { 1e-3 } else { 0.0 };
        da.total_cmp(&dc)
    });
    // Each frame against the anchor over the 5×5 blocks around, where both are usable (a
    // one-sided mean at the edge of a clipped area would be biased).
    let flags: Vec<Vec<u8>> = (0..n)
        .into_par_iter()
        .map(|i| {
            let mut f = vec![0u8; gw * gh];
            for gy in 0..gh {
                for gx in 0..gw {
                    let g = gy * gw + gx;
                    let Some(&a) = by_closeness.iter().find(|&&j| blocks[j].0[g].is_finite())
                    else {
                        continue;
                    };
                    if a == i || !blocks[i].0[g].is_finite() {
                        continue;
                    }
                    let (mut d, mut v, mut base, mut c) = (0.0f32, 0.0f32, 0.0f32, 0u32);
                    for y in gy.saturating_sub(2)..(gy + 3).min(gh) {
                        for x in gx.saturating_sub(2)..(gx + 3).min(gw) {
                            let q = y * gw + x;
                            let (ri, ra) = (blocks[i].0[q], blocks[a].0[q]);
                            if ri.is_finite() && ra.is_finite() {
                                d += ri - ra;
                                v += blocks[i].1[q] + blocks[a].1[q];
                                base += ra;
                                c += 1;
                            }
                        }
                    }
                    if c == 0 {
                        continue;
                    }
                    let (d, base) = (d / c as f32, base / c as f32);
                    let z = d / (v / (c * c) as f32).max(1e-20).sqrt();
                    if z.abs() > k && d.abs() > 0.03 * base.abs().max(1e-6) {
                        f[g] = 1;
                    }
                }
            }
            f
        })
        .collect();
    let keep: Vec<Vec<f32>> = flags
        .par_iter()
        .map(|f| {
            let grown = dilate(f, gw, gh, 2);
            let soft: Vec<f32> = grown.iter().map(|&v| f32::from(v)).collect();
            blur(&soft, gw, gh, 2)
                .iter()
                .map(|v| 1.0 - v.clamp(0.0, 1.0))
                .collect()
        })
        .collect();
    let mask = (0..gw * gh)
        .map(|g| {
            let worst = keep.iter().map(|k| k[g]).fold(1.0f32, f32::min);
            ((1.0 - worst) * 255.0).round() as u8
        })
        .collect();
    (
        keep,
        GhostMask {
            width: gw as u32,
            height: gh as u32,
            block,
            mask,
        },
    )
}

/// Box mean of radius `r` over the finite values (NaN where the centre is not finite).
fn blur(v: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    let mut out = vec![f32::NAN; v.len()];
    out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, o) in row.iter_mut().enumerate() {
            if !v[y * w + x].is_finite() {
                continue;
            }
            let (mut s, mut c) = (0.0f32, 0u32);
            for yy in y.saturating_sub(r)..(y + r + 1).min(h) {
                for xx in x.saturating_sub(r)..(x + r + 1).min(w) {
                    let q = v[yy * w + xx];
                    if q.is_finite() {
                        s += q;
                        c += 1;
                    }
                }
            }
            *o = s / c as f32;
        }
    });
    out
}

fn dilate(v: &[u8], w: usize, h: usize, r: usize) -> Vec<u8> {
    let mut out = vec![0u8; v.len()];
    out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, o) in row.iter_mut().enumerate() {
            'n: for yy in y.saturating_sub(r)..(y + r + 1).min(h) {
                for xx in x.saturating_sub(r)..(x + r + 1).min(w) {
                    if v[yy * w + xx] != 0 {
                        *o = 1;
                        break 'n;
                    }
                }
            }
        }
    });
    out
}

/// The weighted merge: radiance per sample in reference units (1 = its white).
#[allow(clippy::too_many_arguments)]
pub(crate) fn merge(
    planes: &[Plane],
    mosaic: &Mosaic,
    e: &[f64],
    noise: &[Noise],
    keep: &[Vec<f32>],
    mask: &GhostMask,
    cancel: &specular_core::CancelToken,
) -> Option<Vec<f32>> {
    let p0 = &planes[0];
    let (w, cpp) = (p0.w as usize, p0.cpp as usize);
    let n = planes.len();
    let b = mask.block.max(1) as usize;
    let gw = mask.width as usize;
    let ef: Vec<f32> = e.iter().map(|&v| v as f32).collect();
    // Where every frame is clipped: the shortest exposure's saturation.
    let shortest = (0..n).min_by(|&a, &c| e[a].total_cmp(&e[c])).unwrap_or(0);
    let clip = planes[shortest].sat / ef[shortest];
    let mut out = vec![0f32; p0.v.len()];
    let rows = out
        .par_chunks_mut(w * cpp)
        .enumerate()
        .try_for_each(|(y, row)| {
            if y % 64 == 0 && cancel.is_cancelled() {
                return Err(());
            }
            let mut v = [0f32; 16];
            let mut k = [0f32; 16];
            for x in 0..w {
                let g = (y / b) * gw + x / b;
                for c in 0..cpp {
                    let col = if cpp == 1 {
                        mosaic.color(x as u32, y as u32) as usize
                    } else {
                        c
                    };
                    let i0 = (y * w + x) * cpp + c;
                    let (mut sv, mut se, mut any) = (0.0f32, 0.0f32, false);
                    for i in 0..n {
                        v[i] = planes[i].v[i0];
                        k[i] = keep[i][g];
                        if v[i].is_finite() && k[i] > 0.0 {
                            sv += k[i] * v[i];
                            se += k[i] * ef[i];
                            any = true;
                        }
                    }
                    if !any {
                        // Ghost-masked everywhere usable: fall back to the usable frames.
                        for i in 0..n {
                            if v[i].is_finite() {
                                k[i] = 1.0;
                                sv += v[i];
                                se += ef[i];
                                any = true;
                            }
                        }
                    }
                    if !any {
                        row[x * cpp + c] = clip;
                        continue;
                    }
                    let r0 = (sv / se).max(0.0);
                    let (mut sw, mut sr) = (0.0f32, 0.0f32);
                    for i in 0..n {
                        if !(v[i].is_finite() && k[i] > 0.0) {
                            continue;
                        }
                        let nz = &noise[i];
                        let var = nz.a[col] * ef[i] * r0 + nz.b[col];
                        let wi = k[i] * ef[i] * ef[i] / var.max(1e-12);
                        sw += wi;
                        sr += wi * v[i] / ef[i];
                    }
                    row[x * cpp + c] = sr / sw;
                }
            }
            Ok(())
        });
    rows.ok().map(|_| out)
}
