//! Finding the patches from four marked corners, and sampling them.
//!
//! - **Snap.** Each click moves to the centre of the flat area around it, within half a
//!   patch: a box half a patch wide is scored by its relative spread at every offset; the
//!   offsets scoring close to the best, connected to the one nearest the click, are the
//!   patch's flat middle, and their centroid is its centre.
//! - **Locate.** The four snapped corners give a homography from the chart's grid to the
//!   picture; every patch is snapped near its predicted centre, and the homography refitted
//!   to all of them by least squares (twice, dropping patches that moved more than a
//!   quarter patch), which follows small lens distortion and marking errors.
//! - **Sample.** An inset square half the patch pitch wide, mapped through the homography
//!   (so it follows perspective), sampled bilinearly; per channel the trimmed mean (10 % cut
//!   at each end). Patches with a clipped sample, a spread above 5 % of their mean (glare,
//!   dust) or a sample outside the picture are flagged: the fit leaves them out and the
//!   report names them.

use super::layout::Layout;
use crate::ops::geometry::homography::Homography;

/// A linear RGB picture (RGBA, f32, rows packed).
#[derive(Clone, Copy, Debug)]
pub struct Picture<'a> {
    pub w: u32,
    pub h: u32,
    pub rgba: &'a [f32],
}

impl Picture<'_> {
    fn px(&self, x: i64, y: i64) -> [f32; 3] {
        let x = x.clamp(0, i64::from(self.w) - 1) as usize;
        let y = y.clamp(0, i64::from(self.h) - 1) as usize;
        let i = (y * self.w as usize + x) * 4;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2]]
    }

    /// Bilinear sample at pixel-centre coordinates (pixel (0, 0) covers 0..1).
    fn bilinear(&self, x: f64, y: f64) -> [f32; 3] {
        let (fx, fy) = (x - 0.5, y - 0.5);
        let (x0, y0) = (fx.floor(), fy.floor());
        let (ax, ay) = ((fx - x0) as f32, (fy - y0) as f32);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let p = [
            self.px(x0, y0),
            self.px(x0 + 1, y0),
            self.px(x0, y0 + 1),
            self.px(x0 + 1, y0 + 1),
        ];
        std::array::from_fn(|c| {
            let top = p[0][c] + (p[1][c] - p[0][c]) * ax;
            let bot = p[2][c] + (p[3][c] - p[2][c]) * ax;
            top + (bot - top) * ay
        })
    }

    fn inside(&self, x: f64, y: f64) -> bool {
        x >= 0.0 && y >= 0.0 && x <= f64::from(self.w) && y <= f64::from(self.h)
    }
}

/// Why a patch is left out of the fit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Flag {
    /// A sample at or above the clip level.
    Clipped,
    /// Spread above 5 % of the mean: glare, dust or a mark on the patch.
    Uneven,
    /// The patch is (partly) outside the picture.
    Outside,
}

/// One sampled patch.
#[derive(Clone, Debug, PartialEq)]
pub struct Sampled {
    /// Centre in picture pixels.
    pub centre: (f64, f64),
    /// Trimmed mean per channel.
    pub rgb: [f64; 3],
    /// Largest per-channel spread (trimmed standard deviation) over the mean.
    pub spread: f64,
    pub flag: Option<Flag>,
}

/// Where the chart is.
#[derive(Clone, Debug, PartialEq)]
pub struct Located {
    /// Grid units → picture pixels.
    pub grid_to_picture: Homography,
    /// Centre of every patch (layout order), picture pixels.
    pub centres: Vec<(f64, f64)>,
    /// The snapped corner positions (click order).
    pub corners: [(f64, f64); 4],
}

/// The flat-area score of a box of half-size `b` centred at (`cx`, `cy`): the relative
/// spread summed over the channels, from an integral image of the search region.
struct Integral {
    x0: i64,
    y0: i64,
    w: usize,
    h: usize,
    /// (sum, sum of squares) per channel, (w + 1) × (h + 1).
    s: Vec<[f64; 6]>,
}

impl Integral {
    fn new(p: &Picture, x0: i64, y0: i64, w: usize, h: usize) -> Integral {
        let mut s = vec![[0f64; 6]; (w + 1) * (h + 1)];
        for y in 0..h {
            let mut row = [0f64; 6];
            for x in 0..w {
                let v = p.px(x0 + x as i64, y0 + y as i64);
                for c in 0..3 {
                    let f = f64::from(v[c]);
                    row[c] += f;
                    row[c + 3] += f * f;
                }
                let up = s[y * (w + 1) + x + 1];
                s[(y + 1) * (w + 1) + x + 1] = std::array::from_fn(|k| up[k] + row[k]);
            }
        }
        Integral { x0, y0, w, h, s }
    }

    fn score(&self, cx: i64, cy: i64, b: i64) -> f64 {
        let xa = (cx - b - self.x0).clamp(0, self.w as i64) as usize;
        let xb = (cx + b + 1 - self.x0).clamp(0, self.w as i64) as usize;
        let ya = (cy - b - self.y0).clamp(0, self.h as i64) as usize;
        let yb = (cy + b + 1 - self.y0).clamp(0, self.h as i64) as usize;
        let n = ((xb - xa) * (yb - ya)) as f64;
        if n < 1.0 {
            return f64::INFINITY;
        }
        let at = |x: usize, y: usize| self.s[y * (self.w + 1) + x];
        let (a, bb, c, d) = (at(xb, yb), at(xa, yb), at(xb, ya), at(xa, ya));
        let mut total = 0.0;
        for k in 0..3 {
            let sum = a[k] - bb[k] - c[k] + d[k];
            let sq = a[k + 3] - bb[k + 3] - c[k + 3] + d[k + 3];
            let mean = sum / n;
            let var = (sq / n - mean * mean).max(0.0);
            total += var.sqrt() / mean.abs().max(1e-3);
        }
        total
    }
}

/// Moves `click` to the centre of the flat patch around it (within `pitch / 2`).
pub fn snap(p: &Picture, click: (f64, f64), pitch: f64) -> (f64, f64) {
    let pitch = pitch.max(4.0);
    let reach = (pitch * 0.5).round() as i64;
    let b = ((pitch * 0.25).round() as i64).max(1);
    let (cx, cy) = (click.0.floor() as i64, click.1.floor() as i64);
    let step = ((pitch / 24.0).floor() as i64).max(1);
    let (x0, y0) = (cx - reach - b - 1, cy - reach - b - 1);
    let side = (2 * (reach + b + 1) + 1) as usize;
    let ig = Integral::new(p, x0, y0, side, side);
    let n = (2 * reach / step + 1) as usize;
    let mut grid = vec![f64::INFINITY; n * n];
    for j in 0..n {
        for i in 0..n {
            let ox = -reach + i as i64 * step;
            let oy = -reach + j as i64 * step;
            if ox * ox + oy * oy <= reach * reach {
                grid[j * n + i] = ig.score(cx + ox, cy + oy, b);
            }
        }
    }
    let best = grid.iter().copied().fold(f64::INFINITY, f64::min);
    if !best.is_finite() {
        return click;
    }
    let limit = best * 1.5 + 2e-3;
    let ok: Vec<bool> = grid.iter().map(|&s| s <= limit).collect();
    // The candidate nearest the click, then its connected region.
    let mid = (n / 2) as i64;
    let Some(start) = (0..n * n).filter(|&k| ok[k]).min_by_key(|&k| {
        let (i, j) = ((k % n) as i64 - mid, (k / n) as i64 - mid);
        i * i + j * j
    }) else {
        return click;
    };
    let mut seen = vec![false; n * n];
    let mut stack = vec![start];
    seen[start] = true;
    let (mut sx, mut sy, mut cnt) = (0f64, 0f64, 0f64);
    while let Some(k) = stack.pop() {
        let (i, j) = (k % n, k / n);
        sx += (-reach + i as i64 * step) as f64;
        sy += (-reach + j as i64 * step) as f64;
        cnt += 1.0;
        let mut push = |ii: usize, jj: usize| {
            let q = jj * n + ii;
            if ok[q] && !seen[q] {
                seen[q] = true;
                stack.push(q);
            }
        };
        if i > 0 {
            push(i - 1, j);
        }
        if i + 1 < n {
            push(i + 1, j);
        }
        if j > 0 {
            push(i, j - 1);
        }
        if j + 1 < n {
            push(i, j + 1);
        }
    }
    (cx as f64 + 0.5 + sx / cnt, cy as f64 + 0.5 + sy / cnt)
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// Local patch pitch in pixels at grid point `g` (along the grid's x).
fn local_pitch(h: &Homography, g: (f64, f64)) -> f64 {
    let a = h.apply(g);
    let bx = h.apply((g.0 + 1.0, g.1));
    let by = h.apply((g.0, g.1 + 1.0));
    match (a, bx, by) {
        (Some(a), Some(bx), Some(by)) => dist(a, bx).min(dist(a, by)),
        _ => 0.0,
    }
}

/// Finds the chart from its four corner patches marked in `clicks` (layout corner order).
pub fn locate(layout: &Layout, p: &Picture, clicks: [(f64, f64); 4]) -> Option<Located> {
    let grid: [(f64, f64); 4] = layout.corners.map(|i| layout.patches[i].at);
    let (cols, rows) = (f64::from(layout.cols - 1), f64::from(layout.rows - 1));
    let pitch = (dist(clicks[0], clicks[1]) / cols
        + dist(clicks[3], clicks[2]) / cols
        + dist(clicks[0], clicks[3]) / rows
        + dist(clicks[1], clicks[2]) / rows)
        / 4.0;
    if !(pitch.is_finite() && pitch >= 4.0) {
        return None;
    }
    let corners = clicks.map(|c| snap(p, c, pitch));
    let mut h = Homography::from_quads(grid, corners)?;
    for _ in 0..2 {
        let mut pairs = Vec::new();
        for patch in &layout.patches {
            let Some(want) = h.apply(patch.at) else {
                continue;
            };
            let lp = local_pitch(&h, patch.at) * patch.pitch.0.min(patch.pitch.1);
            if !p.inside(want.0, want.1) || lp < 4.0 {
                continue;
            }
            let got = snap(p, want, lp * 0.6);
            if dist(got, want) <= lp * 0.25 {
                pairs.push((patch.at, got));
            }
        }
        if pairs.len() < 8 {
            break;
        }
        match Homography::fit(&pairs) {
            Some(n) => h = n,
            None => break,
        }
    }
    // Each centre where its patch snapped to, when that is near the fit (layouts whose
    // rows sit off the grid, such as the IT8 grey scale, land on their patches); else the
    // fit's.
    let centres = layout
        .patches
        .iter()
        .map(|q| {
            let Some(want) = h.apply(q.at) else {
                return (f64::NAN, f64::NAN);
            };
            let lp = local_pitch(&h, q.at) * q.pitch.0.min(q.pitch.1);
            if !p.inside(want.0, want.1) || lp < 4.0 {
                return want;
            }
            let got = snap(p, want, lp * 0.6);
            if dist(got, want) <= lp * 0.3 {
                got
            } else {
                want
            }
        })
        .collect();
    Some(Located {
        grid_to_picture: h,
        centres,
        corners,
    })
}

/// Samples every patch. `clip`: per-channel levels at or above which a sample is clipped.
pub fn sample(layout: &Layout, p: &Picture, at: &Located, clip: [f32; 3]) -> Vec<Sampled> {
    let h = &at.grid_to_picture;
    let back = h.inverse();
    let mut out: Vec<(Sampled, [f64; 3])> = layout
        .patches
        .iter()
        .zip(&at.centres)
        .map(|(patch, &centre)| {
            let half = (patch.pitch.0 * 0.25, patch.pitch.1 * 0.25);
            // The square around the located centre, in grid units.
            let g = back
                .and_then(|b| b.apply(centre))
                .filter(|g| g.0.is_finite() && g.1.is_finite())
                .unwrap_or(patch.at);
            let side_px = local_pitch(h, patch.at) * patch.pitch.0.min(patch.pitch.1) * 0.5;
            let n = (side_px.round() as usize).clamp(4, 24);
            let mut vals: [Vec<f32>; 3] = Default::default();
            let mut outside = !centre.0.is_finite();
            let mut clipped = false;
            for j in 0..n {
                for i in 0..n {
                    let u = g.0 - half.0 + 2.0 * half.0 * (i as f64 + 0.5) / n as f64;
                    let v = g.1 - half.1 + 2.0 * half.1 * (j as f64 + 0.5) / n as f64;
                    let Some(q) = h.apply((u, v)) else {
                        outside = true;
                        continue;
                    };
                    if !p.inside(q.0, q.1) {
                        outside = true;
                        continue;
                    }
                    let s = p.bilinear(q.0, q.1);
                    for c in 0..3 {
                        clipped |= s[c] >= clip[c];
                        vals[c].push(s[c]);
                    }
                }
            }
            let mut rgb = [0f64; 3];
            let mut sd = [0f64; 3];
            for c in 0..3 {
                (rgb[c], sd[c]) = trimmed(&mut vals[c]);
            }
            let flag = if outside {
                Some(Flag::Outside)
            } else if clipped {
                Some(Flag::Clipped)
            } else {
                None
            };
            let s = Sampled {
                centre,
                rgb,
                spread: 0.0,
                flag,
            };
            (s, sd)
        })
        .collect();
    // Spread relative to the mean, with a floor at 5 % of the brightest patch so the dark
    // patches' noise does not flag them.
    let top = out
        .iter()
        .filter(|s| s.0.flag.is_none())
        .map(|s| s.0.rgb[1])
        .fold(0f64, f64::max);
    let floor = 0.05 * top;
    for (s, sd) in &mut out {
        s.spread = (0..3)
            .map(|c| sd[c] / s.rgb[c].abs().max(floor).max(1e-6))
            .fold(0f64, f64::max);
        if s.flag.is_none() && s.spread > 0.05 {
            s.flag = Some(Flag::Uneven);
        }
    }
    out.into_iter().map(|s| s.0).collect()
}

/// (mean, standard deviation) after dropping 10 % at each end.
fn trimmed(v: &mut [f32]) -> (f64, f64) {
    if v.is_empty() {
        return (0.0, 0.0);
    }
    v.sort_by(f32::total_cmp);
    let cut = v.len() / 10;
    let kept = &v[cut..v.len() - cut];
    let n = kept.len() as f64;
    let mean = kept.iter().map(|&x| f64::from(x)).sum::<f64>() / n;
    let var = kept
        .iter()
        .map(|&x| (f64::from(x) - mean).powi(2))
        .sum::<f64>()
        / n;
    (mean, var.sqrt())
}
