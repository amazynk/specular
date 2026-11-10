//! Star candidates: local maxima above the background whose 3×3 mean stands out from the
//! noise, then a first look at each from its moments (centroid, width, flux) that drops hot
//! pixels, edge stars and the extra maxima on a large star's core.

// `!(a > b)` is meant: a blank (NaN) pixel fails every test.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use rayon::prelude::*;

use crate::Image;
use crate::background::Background;

/// A local maximum that stands out.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Peak {
    pub x: usize,
    pub y: usize,
    /// Above the background.
    pub height: f32,
}

/// A candidate after its moments: kept as a star when its width is plausible.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cand {
    pub x: usize,
    pub y: usize,
    pub cx: f32,
    pub cy: f32,
    pub height: f32,
    /// Standard deviation of the light about the centroid (px), from the second moments.
    pub sigma: f32,
    /// Its brightest pixels sit on a plateau at the frame's maximum.
    pub saturated: bool,
    /// It drops by more than 65 % of its height to its brightest neighbour: a hot pixel or a
    /// cosmic ray hit (also on a star's wing), not starlight spread by the optics and the
    /// air (a star of 2 px FWHM drops by half).
    pub sharp: bool,
}

/// Local maxima of `img` at least `k` noise units above the background, as the 3×3 mean
/// around them (its noise is a third of a pixel's), and the frame's largest value.
pub(crate) fn peaks(img: &Image<'_>, bg: &Background, k: f32) -> (Vec<Peak>, f32) {
    let (w, h) = (img.width, img.height);
    if w < 3 || h < 3 {
        return (Vec::new(), f32::NAN);
    }
    let v = img.values;
    let (found, max) = (1..h - 1)
        .into_par_iter()
        .fold(
            || (Vec::new(), f32::NEG_INFINITY),
            |(mut out, mut max): (Vec<Peak>, f32), y| {
                let up = &v[(y - 1) * w..y * w];
                let row = &v[y * w..(y + 1) * w];
                let down = &v[(y + 1) * w..(y + 2) * w];
                let (cols, lo) = bg.row_centres(y);
                let tile = bg.tile();
                let mut x = 1;
                while x < w - 1 {
                    let c = row[x];
                    if c > max {
                        max = c;
                    }
                    let tx = x / tile;
                    let sigma = bg.tile_at(x, y).1;
                    // A single pixel at least `k` / 3 noise units up (anything weaker cannot
                    // make a 3×3 mean `k` units up while being its maximum): first against
                    // the lowest level nearby, then the exact one.
                    let need = sigma * k / 3.0;
                    if !(c > lo[tx.min(lo.len() - 1)] + need) {
                        x += 1;
                        continue;
                    }
                    let level = bg.level_in_row(&cols, x);
                    if !(c > level + need) {
                        x += 1;
                        continue;
                    }
                    // A maximum: not below any neighbour, and above the ones before it (so a
                    // flat top yields one peak).
                    let before = [up[x - 1], up[x], up[x + 1], row[x - 1]];
                    let after = [row[x + 1], down[x - 1], down[x], down[x + 1]];
                    if before.iter().all(|&n| c > n) && after.iter().all(|&n| c >= n) {
                        let sum: f32 = before.iter().chain(after.iter()).sum::<f32>() + c;
                        let mean = sum / 9.0 - level;
                        if mean * 3.0 >= k * sigma {
                            out.push(Peak {
                                x,
                                y,
                                height: c - level,
                            });
                        }
                        // The next pixel is not above this one: no maximum there.
                        x += 2;
                        continue;
                    }
                    x += 1;
                }
                // The edge columns count towards the maximum too.
                for c in [row[0], row[w - 1]] {
                    if c > max {
                        max = c;
                    }
                }
                (out, max)
            },
        )
        .reduce(
            || (Vec::new(), f32::NEG_INFINITY),
            |(mut a, ma), (b, mb)| {
                a.extend(b);
                (a, ma.max(mb))
            },
        );
    (found, max)
}

/// The window radius that suits a star of standard deviation `sigma`.
pub(crate) fn radius_for(sigma: f32) -> usize {
    ((sigma * 5.0).ceil() as usize).clamp(4, 32)
}

/// Centroid, width and saturation of the light around `p` within `r` px (the pixels at least
/// 1.5 noise units above the local background: isophotal, so noise and hot pixels do not
/// widen it), or `None` at the edge or without light.
fn moments(img: &Image<'_>, bg: &Background, p: Peak, r: usize, sat: f32) -> Option<Cand> {
    let (w, h) = (img.width, img.height);
    if p.x < r || p.y < r || p.x + r >= w || p.y + r >= h {
        return None;
    }
    let level = bg.level_at(p.x as f32, p.y as f32);
    let cut = 1.5 * bg.tile_at(p.x, p.y).1;
    let (mut s, mut sx, mut sy) = (0.0f64, 0.0f64, 0.0f64);
    let mut plateau = 0;
    for y in p.y - r..=p.y + r {
        let row = &img.values[y * w..(y + 1) * w];
        for (x, &v) in row.iter().enumerate().take(p.x + r + 1).skip(p.x - r) {
            if v >= sat {
                plateau += 1;
            }
            let d = v - level;
            if d > cut {
                let d = f64::from(d);
                s += d;
                sx += d * x as f64;
                sy += d * y as f64;
            }
        }
    }
    if s <= 0.0 {
        return None;
    }
    let (cx, cy) = (sx / s, sy / s);
    let mut sxx = 0.0f64;
    for y in p.y - r..=p.y + r {
        let row = &img.values[y * w..(y + 1) * w];
        for (x, &v) in row.iter().enumerate().take(p.x + r + 1).skip(p.x - r) {
            let d = v - level;
            if d > cut {
                let (dx, dy) = (x as f64 - cx, y as f64 - cy);
                sxx += f64::from(d) * (dx * dx + dy * dy);
            }
        }
    }
    let sigma = (sxx / s / 2.0).sqrt() as f32;
    let v = |x: usize, y: usize| img.values[y * w + x];
    let n4 = v(p.x - 1, p.y)
        .max(v(p.x + 1, p.y))
        .max(v(p.x, p.y - 1))
        .max(v(p.x, p.y + 1));
    let peak = p.height + level;
    let sharp = !(peak - n4 <= 0.65 * (peak - level));
    Some(Cand {
        x: p.x,
        y: p.y,
        cx: cx as f32,
        cy: cy as f32,
        height: p.height,
        sigma,
        saturated: plateau >= 3,
        sharp,
    })
}

/// The candidates from `peaks`: each looked at in a window that suits its width, extra
/// maxima on a brighter star's core dropped, hot pixels (narrower than `min_sigma`) and stars
/// too close to the edge left out. Brightest first.
pub(crate) fn candidates(
    img: &Image<'_>,
    bg: &Background,
    mut peaks: Vec<Peak>,
    sat: f32,
    min_sigma: f32,
) -> Vec<Cand> {
    peaks.sort_by(|a, b| b.height.total_cmp(&a.height));
    let looked: Vec<Option<Cand>> = peaks
        .par_iter()
        .map(|&p| {
            let first = moments(img, bg, p, 4, sat)?;
            let r = radius_for(first.sigma);
            if r > 4 {
                moments(img, bg, p, r, sat)
            } else {
                Some(first)
            }
        })
        .collect();
    // Brightest first: a maximum within a brighter star's core (its width, at least 2 px)
    // belongs to that star.
    let (w, h) = (img.width, img.height);
    let cell = 32usize;
    let gx = w.div_ceil(cell);
    let mut grid: Vec<Vec<(f32, f32, f32)>> = vec![Vec::new(); gx * h.div_ceil(cell)];
    let mut out = Vec::new();
    // A hot pixel takes no room: a star under it keeps its own peak.
    for c in looked
        .into_iter()
        .flatten()
        .filter(|c| c.saturated || !c.sharp)
    {
        let (x, y) = (c.cx, c.cy);
        let (gx0, gy0) = (c.x / cell, c.y / cell);
        let mut taken = false;
        'near: for gy in gy0.saturating_sub(1)..=gy0 + 1 {
            for gxx in gx0.saturating_sub(1)..=gx0 + 1 {
                let Some(list) = grid.get(gy * gx + gxx).filter(|_| gxx < gx) else {
                    continue;
                };
                for &(ox, oy, or) in list {
                    let (dx, dy) = (ox - x, oy - y);
                    if dx * dx + dy * dy < or * or {
                        taken = true;
                        break 'near;
                    }
                }
            }
        }
        if taken {
            continue;
        }
        let core = (c.sigma * 2.5).clamp(2.0, cell as f32);
        grid[gy0 * gx + gx0].push((x, y, core));
        if c.saturated || (c.sigma >= min_sigma && !c.sharp) {
            out.push(c);
        }
    }
    out
}
