//! The noise model of the denoiser.
//!
//! Sensor noise is Poisson–Gaussian: a sample of mean `x` has variance `a·x + b` (shot noise
//! plus read noise; Foi, Trimeche, Katkovnik and Egiazarian 2008). The model comes from:
//!
//! 1. the DNG `NoiseProfile` tag when the file has one ([`RawNoise::from_profile`]);
//! 2. otherwise a fit on the image itself ([`RawNoise::estimate`]): the same-colour samples
//!    are cut into 8×8 blocks; each block gives its mean and a noise variance from the
//!    residual of a four-neighbour prediction (blind to linear gradients); blocks are
//!    binned by mean. In each bin a low quantile gives a first noise level (flat blocks
//!    have the least variance); then blocks above the 90th percentile of pure noise at
//!    that level are dropped as texture and the median of the rest, divided by the
//!    45th percentile of pure noise, gives the next level (three rounds: a trimmed median
//!    that texture cannot pull up). The percentiles are the estimator's own on pure noise.
//!    Then `variance = a·mean + b` is fitted to the bins by weighted least squares.
//!
//! Both give the model in normalised raw units (black 0, white 1) per CFA colour.
//! [`RawNoise::to_working`] carries it through the raw front end — white balance, the
//! demosaic or binning (each averages a known number of samples), the camera matrix — to
//! the developed buffer's units ([`NoiseModel`]), and a reduction to a smaller size scales
//! the variance by the area ratio. The denoiser stabilises the variance with the
//! generalised Anscombe transform and needs nothing else.

use std::sync::LazyLock;

use specular_raw::SensorData;
use specular_raw::sensor::Layout;

use crate::model::{Fields, Value};
use crate::ops::raw::Quality;

/// Noise variance `a·v + b` per channel (R, G, B) of a buffer, `v` in its units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoiseModel {
    pub a: [f32; 3],
    pub b: [f32; 3],
}

impl NoiseModel {
    /// The model used when nothing better is known: a mid-ISO sensor (about ISO 800 on a
    /// current full-frame body), in white = 1 units.
    pub const FALLBACK: NoiseModel = NoiseModel {
        a: [2.0e-4; 3],
        b: [2.0e-7; 3],
    };

    /// The same noise after the variance is multiplied by `k` (an average over `1/k`
    /// independent samples).
    pub fn scaled(&self, k: f32) -> NoiseModel {
        NoiseModel {
            a: self.a.map(|v| v * k),
            b: self.b.map(|v| v * k),
        }
    }

    /// The model from fields `noise_a`, `noise_b` (three floats each), when valid.
    pub fn from_fields(f: &Fields) -> Option<NoiseModel> {
        let a = f.floats_or("noise_a", [0.0; 3]);
        let b = f.floats_or("noise_b", [-1.0; 3]);
        let ok = a.iter().all(|v| v.is_finite() && *v > 0.0)
            && b.iter().all(|v| v.is_finite() && *v >= 0.0);
        ok.then_some(NoiseModel { a, b })
    }

    /// Writes the model into fields.
    pub fn write(&self, f: &mut Fields) {
        f.set("noise_a", Value::Floats(self.a.to_vec()));
        f.set("noise_b", Value::Floats(self.b.to_vec()));
    }

    /// Kept in a sane range (a degenerate fit must not divide by zero).
    pub fn sane(&self) -> NoiseModel {
        NoiseModel {
            a: self.a.map(|v| v.clamp(1e-9, 1.0)),
            b: self.b.map(|v| v.clamp(0.0, 1.0)),
        }
    }
}

/// Where a raw noise model came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoiseSource {
    /// The DNG NoiseProfile tag.
    Profile,
    /// Fitted on the image.
    Estimated,
    /// Neither worked: [`NoiseModel::FALLBACK`].
    Fallback,
}

/// Noise variance `a·x + b` per CFA colour (R, G, B), `x` normalised (black 0, white 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawNoise {
    pub a: [f64; 3],
    pub b: [f64; 3],
    pub source: NoiseSource,
}

/// Same-colour samples per block side.
pub(super) const BLOCK: usize = 8;
/// Quantile of the block variances taken per bin for the first level.
const QUANTILE: f64 = 0.1;
/// Fewest blocks a bin needs.
const MIN_BLOCKS: usize = 24;
/// Bins over the signal range.
const BINS: usize = 32;

/// A block's variance estimate on pure white noise of unit variance, sorted (the
/// estimator's own distribution: measured once with a fixed seed, 40,000 blocks).
static PURE: LazyLock<Vec<f64>> = LazyLock::new(|| {
    let mut rng = 0x2545_F491_4F6C_DD1Du64;
    let mut normal = move || {
        // Box–Muller on a xorshift generator.
        let mut u = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            ((rng >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        };
        let (u1, u2) = (u(), u());
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    };
    let mut v: Vec<f64> = (0..40_000)
        .map(|_| {
            let b: Vec<f64> = (0..BLOCK * BLOCK).map(|_| normal()).collect();
            block_stats(|i, j| b[j * BLOCK + i]).1
        })
        .collect();
    v.sort_by(f64::total_cmp);
    v
});

/// Quantile `p` of the pure-noise distribution.
fn pure(p: f64) -> f64 {
    let v = &*PURE;
    v[((p * v.len() as f64) as usize).min(v.len() - 1)]
}

/// The noise variance of one bin's sorted block variances (see the module documentation).
fn bin_variance(v: &[f64]) -> f64 {
    let mut s2 = v[(QUANTILE * v.len() as f64) as usize] / pure(QUANTILE);
    let (hi, mid) = (pure(0.9), pure(0.45));
    for _ in 0..3 {
        let kept = v.partition_point(|&x| x <= s2 * hi);
        if kept < MIN_BLOCKS / 2 {
            break;
        }
        // The kept set is (mostly) pure noise below its 90th percentile: its median is the
        // 45th percentile.
        s2 = v[kept / 2] / mid;
    }
    s2
}

/// Mean and noise variance of one block (`at(i, j)`, `i, j < BLOCK`): the residual of
/// predicting each inner sample by its four neighbours has variance `1.25·σ²` for white
/// noise and is zero on any linear gradient.
pub(super) fn block_stats(at: impl Fn(usize, usize) -> f64) -> (f64, f64) {
    let mut mean = 0.0;
    for j in 0..BLOCK {
        for i in 0..BLOCK {
            mean += at(i, j);
        }
    }
    mean /= (BLOCK * BLOCK) as f64;
    let mut ss = 0.0;
    for j in 1..BLOCK - 1 {
        for i in 1..BLOCK - 1 {
            let r = at(i, j) - 0.25 * (at(i - 1, j) + at(i + 1, j) + at(i, j - 1) + at(i, j + 1));
            ss += r * r;
        }
    }
    (mean, ss / (((BLOCK - 2) * (BLOCK - 2)) as f64 * 1.25))
}

/// Fits `variance = a·mean + b` to block statistics. `None` when fewer than two bins have
/// enough blocks.
pub fn fit_blocks(blocks: &mut [(f64, f64)]) -> Option<(f64, f64)> {
    if blocks.len() < 2 * MIN_BLOCKS {
        return None;
    }
    blocks.sort_by(|p, q| p.0.total_cmp(&q.0));
    let lo = blocks.first()?.0;
    let hi = blocks.last()?.0;
    if hi.partial_cmp(&lo) != Some(std::cmp::Ordering::Greater) {
        return None;
    }
    // Bins equal in sqrt(mean): denser where the signal is low and b matters.
    let (sl, sh) = (lo.max(0.0).sqrt(), hi.max(0.0).sqrt());
    let mut pts = Vec::new();
    let mut start = 0;
    for k in 0..BINS {
        let edge = sl + (sh - sl) * (k + 1) as f64 / BINS as f64;
        let mut end = start;
        while end < blocks.len() && (k + 1 == BINS || blocks[end].0.max(0.0).sqrt() < edge) {
            end += 1;
        }
        let bin = &blocks[start..end];
        if bin.len() >= MIN_BLOCKS {
            let mut v: Vec<f64> = bin.iter().map(|b| b.1).collect();
            v.sort_by(f64::total_cmp);
            let q = bin_variance(&v);
            let m = bin.iter().map(|b| b.0).sum::<f64>() / bin.len() as f64;
            if q.is_finite() && q > 0.0 {
                pts.push((m, q, bin.len() as f64));
            }
        }
        start = end;
    }
    if pts.len() < 2 {
        return None;
    }
    // Weighted least squares with relative errors: weight n / v².
    let solve = |with_b: bool| {
        let (mut sw, mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for &(m, v, n) in &pts {
            let w = n / (v * v);
            sw += w;
            sx += w * m;
            sy += w * v;
            sxx += w * m * m;
            sxy += w * m * v;
        }
        if with_b {
            let det = sw * sxx - sx * sx;
            if det.abs() < 1e-300 {
                return None;
            }
            Some(((sw * sxy - sx * sy) / det, (sxx * sy - sx * sxy) / det))
        } else {
            (sxx > 0.0).then(|| (sxy / sxx, 0.0))
        }
    };
    let (a, b) = solve(true)?;
    let (a, b) = if b < 0.0 { solve(false)? } else { (a, b) };
    (a.is_finite() && a > 0.0 && b.is_finite()).then_some((a, b))
}

impl RawNoise {
    /// From a DNG NoiseProfile: one pair for every plane, or one per colour plane (R, G, B).
    pub fn from_profile(p: &[[f64; 2]]) -> Option<RawNoise> {
        let pick = |c: usize| match p.len() {
            1 => Some(p[0]),
            3 => Some(p[c]),
            _ => None,
        };
        let v = [pick(0)?, pick(1)?, pick(2)?];
        Some(RawNoise {
            a: v.map(|q| q[0]),
            b: v.map(|q| q[1]),
            source: NoiseSource::Profile,
        })
    }

    /// The model of a sensor: its NoiseProfile, else a fit, else the fallback.
    pub fn of_sensor(s: &SensorData) -> RawNoise {
        s.meta
            .dng
            .as_ref()
            .and_then(|d| RawNoise::from_profile(&d.noise_profile))
            .or_else(|| RawNoise::estimate(s))
            .unwrap_or(RawNoise {
                a: NoiseModel::FALLBACK.a.map(f64::from),
                b: NoiseModel::FALLBACK.b.map(f64::from),
                source: NoiseSource::Fallback,
            })
    }

    /// Fits the model on the sensor's own samples (see the module documentation).
    pub fn estimate(s: &SensorData) -> Option<RawNoise> {
        let m = &s.meta;
        let c = s.crop;
        // Same-colour lattices: (origin x, origin y, step, channel, colour).
        let mut lattices = Vec::new();
        match &m.layout {
            Layout::Bayer(_) => {
                for oy in 0..2 {
                    for ox in 0..2 {
                        let col = m.layout.color(c.x + ox, c.y + oy);
                        lattices.push((ox, oy, 2, 0, col));
                    }
                }
            }
            Layout::Pattern(p) => {
                for oy in 0..p.height {
                    for ox in 0..p.width {
                        let col = m.layout.color(c.x + ox, c.y + oy);
                        lattices.push((ox, oy, p.width.max(p.height), 0, col));
                    }
                }
            }
            Layout::Linear => {
                for ch in 0..s.cpp.min(3) {
                    lattices.push((0, 0, 1, ch, ch as u8));
                }
            }
            Layout::Mono => lattices.push((0, 0, 1, 0, 1)),
        }
        let value = |x: u32, y: u32, ch: u32| -> f64 {
            let (sx, sy) = (c.x + x, c.y + y);
            let v = s.sample(sx, sy, ch);
            let l = match &m.linearization {
                Some(t) if !t.is_empty() => f64::from(t[(v as usize).min(t.len() - 1)]),
                _ => f64::from(v),
            };
            let b = f64::from(m.black.at(sx, sy, ch));
            (l - b) / (f64::from(m.white) - b).max(1.0)
        };
        let clip = {
            let b = f64::from(m.black.max());
            0.9 * (f64::from(m.clip_white) - b) / (f64::from(m.white) - b).max(1.0)
        };
        // Every lattice's block rows in one parallel pass (results kept in lattice and row
        // order); each block's samples are read once into a small array.
        let mut rows: Vec<(usize, usize, usize, usize)> = Vec::new();
        let mut shape: Vec<(u32, u32, u32, u32, usize, usize)> = Vec::new();
        for (l, &(ox, oy, step, ch, _)) in lattices.iter().enumerate() {
            if ox >= c.w || oy >= c.h {
                shape.push((ox, oy, step, ch, 0, 1));
                continue;
            }
            let lw = ((c.w - ox).div_ceil(step)) as usize;
            let lh = ((c.h - oy).div_ceil(step)) as usize;
            // At most about 8,000 blocks per lattice, spread evenly.
            let (nx, ny) = (lw / BLOCK, lh / BLOCK);
            let stride = ((nx * ny) as f64 / 8000.0).sqrt().ceil().max(1.0) as usize;
            shape.push((ox, oy, step, ch, nx, stride));
            rows.extend((0..ny).step_by(stride).map(|by| (l, by, nx, stride)));
        }
        use rayon::prelude::*;
        let found: Vec<(usize, Vec<(f64, f64)>)> = rows
            .into_par_iter()
            .map(|(l, by, nx, stride)| {
                let (ox, oy, step, ch, ..) = shape[l];
                let span = BLOCK as u32;
                let blocks = (0..nx)
                    .step_by(stride)
                    .filter_map(|bx| {
                        let mut v = [0.0f64; BLOCK * BLOCK];
                        let mut top = f64::MIN;
                        for j in 0..BLOCK {
                            for i in 0..BLOCK {
                                let x = ox + (bx as u32 * span + i as u32) * step;
                                let y = oy + (by as u32 * span + j as u32) * step;
                                let q = value(x, y, ch);
                                v[j * BLOCK + i] = q;
                                top = top.max(q);
                            }
                        }
                        (top < clip).then(|| block_stats(|i, j| v[j * BLOCK + i]))
                    })
                    .collect();
                (l, blocks)
            })
            .collect();
        let mut per_colour: [Vec<(f64, f64)>; 3] = Default::default();
        for (l, blocks) in found {
            let col = lattices[l].4;
            per_colour[usize::from(col.min(2))].extend(blocks);
        }
        if m.layout == Layout::Mono {
            let g = per_colour[1].clone();
            per_colour[0].clone_from(&g);
            per_colour[2] = g;
        }
        let mut a = [0.0; 3];
        let mut b = [0.0; 3];
        for k in 0..3 {
            let (ak, bk) = fit_blocks(&mut per_colour[k])?;
            a[k] = ak;
            b[k] = bk.max(0.0);
        }
        Some(RawNoise {
            a,
            b,
            source: NoiseSource::Estimated,
        })
    }

    /// The model of the raw front end's output (linear Rec.2020, white = 1) for `quality`,
    /// white balance multipliers `wb` and output matrix `m` (white-balanced camera RGB →
    /// Rec.2020), with the variance scaled by `reduce` (the area ratio of a later
    /// reduction, 1 for none).
    pub fn to_working(
        &self,
        layout: &Layout,
        quality: Quality,
        wb: [f32; 3],
        m: &[[f32; 3]; 3],
        reduce: f32,
    ) -> NoiseModel {
        // White balance: x' = wb·x, so var(x') = wb·a·x' + wb²·b.
        let a: [f64; 3] = std::array::from_fn(|k| f64::from(wb[k]) * self.a[k]);
        let b: [f64; 3] = std::array::from_fn(|k| f64::from(wb[k]).powi(2) * self.b[k]);
        if *layout == Layout::Mono {
            // One sample feeds all three channels (rows of the matrix sum to one).
            let k = f64::from(reduce);
            return NoiseModel {
                a: [(a[1] * k) as f32; 3],
                b: [(b[1] * k) as f32; 3],
            }
            .sane();
        }
        let f = demosaic_factor(layout, quality);
        // A neutral pixel has x'_k = y for every k, so var_c(y) = Σ_k m_ck² f_k (A_k y + B_k).
        let out = |coef: &[f64; 3]| -> [f32; 3] {
            std::array::from_fn(|ch| {
                let s: f64 = (0..3)
                    .map(|k| f64::from(m[ch][k]).powi(2) * f[k] * coef[k])
                    .sum();
                (s * f64::from(reduce)) as f32
            })
        };
        NoiseModel {
            a: out(&a),
            b: out(&b),
        }
        .sane()
    }
}

/// The variance factor per colour of the front end's interpolation or binning: the share
/// of one sample's variance left in an output pixel. Binning averages a known number of
/// samples per colour; the demosaic factors were measured on flat noise (see the tests).
pub fn demosaic_factor(layout: &Layout, quality: Quality) -> [f64; 3] {
    match (layout, quality) {
        (Layout::Linear | Layout::Mono, _) => [1.0; 3],
        // 2x2: one red, two greens, one blue.
        (Layout::Bayer(_), Quality::Half) => [1.0, 0.5, 1.0],
        (Layout::Bayer(_), Quality::Fast) => MHC_FACTOR,
        (Layout::Bayer(_), Quality::Full) => RCD_FACTOR,
        // 4x4 means over a 6x6 pattern of 8 red, 20 green and 8 blue.
        (Layout::Pattern(_), Quality::Half) => [36.0 / 128.0, 36.0 / 320.0, 36.0 / 128.0],
        (Layout::Pattern(_), _) => XTRANS_FACTOR,
    }
}

/// Measured output variance over input variance per colour (flat grey, white noise; the
/// X-Trans factor is an estimate, not measured).
pub const RCD_FACTOR: [f64; 3] = [0.54, 0.54, 0.54];
pub const MHC_FACTOR: [f64; 3] = [0.73, 0.73, 0.73];
pub const XTRANS_FACTOR: [f64; 3] = [0.6, 0.6, 0.6];

/// Fits the model on a linear RGBA image (non-raw sources, already in the working space):
/// per channel, 8×8 blocks of pixels. `None` when the image is too small or flat to tell.
pub fn estimate_rgb(w: u32, h: u32, rgba: &[f32]) -> Option<NoiseModel> {
    let (w, h) = (w as usize, h as usize);
    if rgba.len() < w * h * 4 {
        return None;
    }
    let mut out = NoiseModel {
        a: [0.0; 3],
        b: [0.0; 3],
    };
    for c in 0..3 {
        let mut blocks: Vec<(f64, f64)> = Vec::new();
        for by in 0..h / BLOCK {
            for bx in 0..w / BLOCK {
                let at = |i: usize, j: usize| {
                    f64::from(rgba[((by * BLOCK + j) * w + bx * BLOCK + i) * 4 + c])
                };
                let mut top = f64::MIN;
                for j in 0..BLOCK {
                    for i in 0..BLOCK {
                        top = top.max(at(i, j));
                    }
                }
                if top < 0.95 {
                    blocks.push(block_stats(at));
                }
            }
        }
        let (a, b) = fit_blocks(&mut blocks)?;
        out.a[c] = a as f32;
        out.b[c] = b.max(0.0) as f32;
    }
    Some(out.sane())
}

/// The generalised Anscombe transform of `z` for a channel with noise `a·z + b`:
/// `ia = 1/a`, `s2 = b/a²`. Linear below a small floor, so it stays invertible for
/// negative and tiny values. Unit noise variance after it.
pub fn gat(z: f32, ia: f32, s2: f32) -> f32 {
    let u = z * ia + 0.375 + s2;
    if u >= 0.0625 {
        2.0 * u.sqrt()
    } else {
        0.5 + (u - 0.0625) * 4.0
    }
}

/// The algebraic inverse of [`gat`] in `u` form.
fn gat_alg(d: f32) -> f32 {
    if d >= 0.5 {
        0.25 * d * d
    } else {
        0.0625 + (d - 0.5) * 0.25
    }
}

/// The inverse of [`gat`]: `m` = 0 the algebraic inverse (for data that was not
/// denoised), 1 the closed-form exact unbiased inverse of Mäkitalo and Foi (2013) for a
/// denoised estimate of `E[gat(z)]`, joined continuously to the algebraic one below 1.
pub fn gat_inv(d: f32, ia: f32, s2: f32, m: f32) -> f32 {
    let alg = gat_alg(d);
    let ub = if d >= 1.0 {
        let r = 1.0 / d;
        0.25 * d * d + 0.306_186_22 * r - 1.375 * r * r + 0.765_465_5 * r * r * r + 0.25
    } else {
        alg - 0.053_348_27
    };
    let u = alg + (ub - alg) * m;
    (u - 0.375 - s2) / ia
}
