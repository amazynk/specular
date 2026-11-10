//! The sky background and the noise: a mesh of tiles, each with a sigma-clipped median and
//! standard deviation of a subsample of its pixels, smoothed with a 3×3 median over the mesh
//! (so a tile mostly covered by a bright star or a galaxy takes its neighbours' level), and
//! read back bilinearly between tile centres.

use rayon::prelude::*;

use crate::Image;

/// Tile side in pixels for a normal frame.
const TILE: usize = 64;
/// Clip at this many standard deviations from the median.
const CLIP: f32 = 3.0;
/// Clipping rounds at most.
const ROUNDS: usize = 5;
/// MAD to standard deviation for normal noise.
pub(crate) const MAD_SIGMA: f32 = 1.482_602_2;

/// The background model of one frame.
#[derive(Clone, Debug)]
pub struct Background {
    tile: usize,
    nx: usize,
    ny: usize,
    /// The frame's size (the last tile of a row or column may be partial).
    w: usize,
    h: usize,
    /// Level and standard deviation per tile, rows of `nx`.
    level: Vec<f32>,
    sigma: Vec<f32>,
    /// The median of the tiles' levels and of their standard deviations.
    pub median: f32,
    pub noise: f32,
    /// The mean absolute deviation of the samples from their tile's level (stars and nebula
    /// included): the signal part of the SNR estimate.
    pub mean_dev: f32,
}

/// The sigma-clipped median and standard deviation (from the MAD) of `v`, which it reorders;
/// `None` when nothing finite is in it.
pub(crate) fn clipped(v: &mut Vec<f32>) -> Option<(f32, f32)> {
    v.retain(|x| x.is_finite());
    if v.is_empty() {
        return None;
    }
    let mut dev = Vec::with_capacity(v.len());
    let mut med = 0.0;
    let mut sig = 0.0;
    for _ in 0..ROUNDS {
        med = median(v);
        dev.clear();
        dev.extend(v.iter().map(|x| (x - med).abs()));
        sig = median(&mut dev) * MAD_SIGMA;
        if sig <= 0.0 {
            break;
        }
        let (lo, hi) = (med - CLIP * sig, med + CLIP * sig);
        let before = v.len();
        v.retain(|x| (lo..=hi).contains(x));
        if v.len() == before || v.len() < 8 {
            break;
        }
    }
    Some((med, sig))
}

/// The median of `v` (reordered; the upper middle for an even count). `v` must not be empty.
pub(crate) fn median(v: &mut [f32]) -> f32 {
    let mid = v.len() / 2;
    let (_, m, _) = v.select_nth_unstable_by(mid, f32::total_cmp);
    *m
}

/// The median of the finite values of `v`, or `None`.
pub(crate) fn median_of(v: impl IntoIterator<Item = f32>) -> Option<f32> {
    let mut v: Vec<f32> = v.into_iter().filter(|x| x.is_finite()).collect();
    (!v.is_empty()).then(|| median(&mut v))
}

impl Background {
    /// Estimates the background of `img`.
    pub fn estimate(img: &Image<'_>) -> Background {
        let (w, h) = (img.width, img.height);
        // At least 4 tiles across the shorter side on small images, down to 16 px.
        let tile = TILE.min((w.min(h) / 4).max(16)).max(1);
        let nx = w.div_ceil(tile).max(1);
        let ny = h.div_ceil(tile).max(1);
        // Every third pixel of every third row of a full tile, every second of a smaller one.
        let step = if tile >= 48 {
            3
        } else if tile >= 32 {
            2
        } else {
            1
        };
        let tiles: Vec<(f32, f32, f64, usize, f32)> = (0..nx * ny)
            .into_par_iter()
            .map_init(Vec::new, |buf: &mut Vec<f32>, t| {
                let (tx, ty) = (t % nx, t / nx);
                let (x0, y0) = (tx * tile, ty * tile);
                let (x1, y1) = ((x0 + tile).min(w), (y0 + tile).min(h));
                buf.clear();
                for y in (y0..y1).step_by(step) {
                    let row = &img.values[y * w..y * w + w];
                    buf.extend(row[x0..x1].iter().step_by(step).copied());
                }
                let all: Vec<f32> = buf.clone();
                let top = all
                    .iter()
                    .copied()
                    .filter(|v| v.is_finite())
                    .fold(f32::NEG_INFINITY, f32::max);
                match clipped(buf) {
                    // Too few clean samples (a blank border, a saturated blob): filled in
                    // from the neighbours below.
                    Some((m, s)) if buf.len() * 4 >= all.len() => {
                        let (sum, n) = all
                            .iter()
                            .filter(|v| v.is_finite())
                            .fold((0.0f64, 0usize), |(s, n), v| {
                                (s + f64::from((v - m).abs()), n + 1)
                            });
                        (m, s, sum, n, top)
                    }
                    _ => (f32::NAN, f32::NAN, 0.0, 0, top),
                }
            })
            .collect();
        let mut level: Vec<f32> = tiles.iter().map(|t| t.0).collect();
        let mut sigma: Vec<f32> = tiles.iter().map(|t| t.1).collect();
        let (dev_sum, dev_n) = tiles.iter().fold((0.0, 0), |(s, n), t| (s + t.2, n + t.3));
        let median_level = median_of(level.iter().copied()).unwrap_or(0.0);
        let mut noise = median_of(sigma.iter().copied()).unwrap_or(0.0);
        // A noise-free frame (simulated, or a blank sky clipped to zero) has no spread to
        // scale by: a thousandth of the range above the background stands in, so its stars
        // are still found. Real frames never get here.
        if noise.is_nan() || noise <= 0.0 {
            let top = tiles.iter().map(|t| t.4).fold(f32::NEG_INFINITY, f32::max);
            let range = top - median_level;
            noise = if range.is_finite() && range > 0.0 {
                range * 1e-3
            } else {
                0.0
            };
        }
        for v in level.iter_mut().filter(|v| !v.is_finite()) {
            *v = median_level;
        }
        // Tiles without spread (a blank border) take the frame's noise.
        for v in sigma.iter_mut().filter(|v| v.is_nan() || **v <= 0.0) {
            *v = noise;
        }
        let level = median_filter(&level, nx, ny);
        let sigma = median_filter(&sigma, nx, ny);
        Background {
            tile,
            nx,
            ny,
            w,
            h,
            level,
            sigma,
            median: median_level,
            noise,
            mean_dev: if dev_n > 0 {
                (dev_sum / dev_n as f64) as f32
            } else {
                0.0
            },
        }
    }

    /// The tile holding pixel (`x`, `y`): its level and standard deviation.
    #[inline]
    pub fn tile_at(&self, x: usize, y: usize) -> (f32, f32) {
        let i = (y / self.tile).min(self.ny - 1) * self.nx + (x / self.tile).min(self.nx - 1);
        (self.level[i], self.sigma[i])
    }

    /// The tile side in pixels.
    pub fn tile(&self) -> usize {
        self.tile
    }

    /// Where position `p` (a pixel's index) falls between the centres of `n` tiles along a
    /// side of `len` px: the two tiles and the weight of the second, linear inside and
    /// extrapolated half a tile past the outer centres (so a gradient reaches the edges).
    fn coord(&self, p: f32, n: usize, len: usize) -> (usize, usize, f32) {
        if n == 1 {
            return (0, 0, 0.0);
        }
        let t = self.tile as f32;
        let centre = |i: usize| {
            let x0 = (i * self.tile) as f32;
            (x0 + ((i + 1) * self.tile).min(len) as f32) * 0.5
        };
        let p = p + 0.5;
        let i0 = ((p / t - 0.5).floor().max(0.0) as usize).min(n - 2);
        let (c0, c1) = (centre(i0), centre(i0 + 1));
        let a = ((p - c0) / (c1 - c0)).clamp(-0.5, 1.5);
        (i0, i0 + 1, a)
    }

    /// Row `y`'s level at each tile centre, and per tile the lowest level its pixels can
    /// interpolate to (for a quick test before [`Background::level_in_row`]).
    pub fn row_centres(&self, y: usize) -> (Vec<f32>, Vec<f32>) {
        let (y0, y1, ay) = self.coord(y as f32, self.ny, self.h);
        let col: Vec<f32> = (0..self.nx)
            .map(|x| self.level[y0 * self.nx + x] * (1.0 - ay) + self.level[y1 * self.nx + x] * ay)
            .collect();
        // Extrapolation past the outer centres can go a little below the neighbours.
        let lo = (0..self.nx)
            .map(|i| {
                let near = &col[i.saturating_sub(1)..=(i + 1).min(self.nx - 1)];
                let lo = near.iter().copied().fold(f32::INFINITY, f32::min);
                let hi = near.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                lo - (hi - lo)
            })
            .collect();
        (col, lo)
    }

    /// The level at column `x` of a row whose tile-centre levels are `col`
    /// ([`Background::row_centres`]).
    #[inline]
    pub fn level_in_row(&self, col: &[f32], x: usize) -> f32 {
        let (x0, x1, ax) = self.coord(x as f32, self.nx, self.w);
        col[x0] * (1.0 - ax) + col[x1] * ax
    }

    /// The level at (`x`, `y`), bilinear between tile centres.
    pub fn level_at(&self, x: f32, y: f32) -> f32 {
        let (x0, x1, ax) = self.coord(x, self.nx, self.w);
        let (y0, y1, ay) = self.coord(y, self.ny, self.h);
        let l = |x: usize, y: usize| self.level[y * self.nx + x];
        let top = l(x0, y0) * (1.0 - ax) + l(x1, y0) * ax;
        let bottom = l(x0, y1) * (1.0 - ax) + l(x1, y1) * ax;
        top * (1.0 - ay) + bottom * ay
    }
}

/// A 3×3 median over a `nx` × `ny` mesh (edges use the cells that exist).
fn median_filter(v: &[f32], nx: usize, ny: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(v.len());
    let mut n = Vec::with_capacity(9);
    for y in 0..ny {
        for x in 0..nx {
            n.clear();
            for yy in y.saturating_sub(1)..=(y + 1).min(ny - 1) {
                for xx in x.saturating_sub(1)..=(x + 1).min(nx - 1) {
                    n.push(v[yy * nx + xx]);
                }
            }
            out.push(median(&mut n));
        }
    }
    out
}
