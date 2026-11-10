//! Corner features and their binary descriptors, for finding how panorama frames overlap.
//!
//! - **Corners**: FAST-9 (Rosten & Drummond, "Machine learning for high-speed corner
//!   detection", ECCV 2006): a pixel is a corner when 9 contiguous pixels of the 16 on a
//!   radius-3 circle are all brighter or all darker than it by a threshold. Each corner is
//!   scored by the Harris measure (det − 0.04·trace² of the gradient structure tensor over a
//!   7×7 window), kept when it is the best in its 3×3 neighbourhood, and the strongest are
//!   taken per cell of a grid, so the whole frame is covered and not only its busiest part.
//! - **Orientation**: the intensity centroid of a radius-15 disc (Rosin 1999, as in ORB).
//! - **Descriptor**: 256 intensity comparisons of point pairs in a 31×31 patch of the
//!   smoothed image (BRIEF, Calonder et al., ECCV 2010), with the pattern rotated to the
//!   corner's orientation (steered BRIEF, Rublee et al., "ORB", ICCV 2011). The pattern is
//!   our own: pairs drawn once from an isotropic Gaussian (σ = patch/5) with a fixed seed;
//!   positions are read bilinearly at the exact rotation, not from quantised tables.
//! - **Matching**: Hamming distance, best against second best below 0.8 (Lowe's ratio
//!   test), and only pairs that are each other's best (cross-check).
//!
//! The input is a luma image (the log-luma proxy of [`super::align`]); NaN marks unusable
//! pixels, and no corner or descriptor reads one.

// Kernel loops read clearest with indices.
#![allow(clippy::needless_range_loop)]

use rayon::prelude::*;

use super::align::Proxy;

/// The patch's half side.
const PATCH: f32 = 15.0;
/// Pixels kept from the border: the rotated patch, its smoothing and the circle.
const BORDER: usize = 24;

/// A corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keypoint {
    pub x: f32,
    pub y: f32,
    /// Harris measure.
    pub score: f32,
    /// Radians.
    pub angle: f32,
}

/// 256 bits.
pub type Descriptor = [u64; 4];

/// A frame's features.
#[derive(Clone, Debug, Default)]
pub struct Features {
    pub points: Vec<Keypoint>,
    pub descriptors: Vec<Descriptor>,
}

/// How many corners to look for.
#[derive(Clone, Copy, Debug)]
pub struct DetectOptions {
    /// Corners wanted over the whole image.
    pub wanted: usize,
    /// Grid cells along the long side.
    pub cells: usize,
}

impl Default for DetectOptions {
    fn default() -> Self {
        DetectOptions {
            wanted: 2000,
            cells: 16,
        }
    }
}

/// The 16 circle offsets of FAST, clockwise from the top.
const CIRCLE: [(i32, i32); 16] = [
    (0, -3),
    (1, -3),
    (2, -2),
    (3, -1),
    (3, 0),
    (3, 1),
    (2, 2),
    (1, 3),
    (0, 3),
    (-1, 3),
    (-2, 2),
    (-3, 1),
    (-3, 0),
    (-3, -1),
    (-2, -2),
    (-1, -3),
];

/// A grey image normalised to about 0…1 (1st to 99th percentile), NaN kept.
#[derive(Clone, Debug)]
pub struct Grey {
    pub w: usize,
    pub h: usize,
    pub v: Vec<f32>,
}

impl Grey {
    pub fn of(p: &Proxy) -> Grey {
        let mut finite: Vec<f32> = p.v.iter().copied().filter(|v| v.is_finite()).collect();
        let (lo, hi) = if finite.len() < 16 {
            (0.0, 1.0)
        } else {
            let n = finite.len();
            let (_, lo, _) = finite.select_nth_unstable_by(n / 100, f32::total_cmp);
            let lo = *lo;
            let (_, hi, _) = finite.select_nth_unstable_by(n - 1 - n / 100, f32::total_cmp);
            (lo, *hi)
        };
        let k = 1.0 / (hi - lo).max(1e-6);
        Grey {
            w: p.w,
            h: p.h,
            v: p.v.iter().map(|&v| (v - lo) * k).collect(),
        }
    }

    #[inline]
    fn at(&self, x: usize, y: usize) -> f32 {
        self.v[y * self.w + x]
    }

    /// Bilinear (NaN outside).
    #[inline]
    fn sample(&self, x: f32, y: f32) -> f32 {
        if !(x >= 0.0 && y >= 0.0) {
            return f32::NAN;
        }
        let (x0, y0) = (x as usize, y as usize);
        if x0 + 1 >= self.w || y0 + 1 >= self.h {
            return f32::NAN;
        }
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let a = self.at(x0, y0);
        let b = self.at(x0 + 1, y0);
        let c = self.at(x0, y0 + 1);
        let d = self.at(x0 + 1, y0 + 1);
        let top = a + (b - a) * fx;
        let bot = c + (d - c) * fx;
        top + (bot - top) * fy
    }

    /// Gaussian smoothing (separable, radius 3σ); NaN spreads.
    fn smooth(&self, sigma: f32) -> Grey {
        let r = (3.0 * sigma).ceil() as i64;
        let k: Vec<f32> = (-r..=r)
            .map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp())
            .collect();
        let s: f32 = k.iter().sum();
        let k: Vec<f32> = k.iter().map(|v| v / s).collect();
        let (w, h) = (self.w, self.h);
        let mut tmp = vec![f32::NAN; w * h];
        tmp.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            for x in r as usize..w.saturating_sub(r as usize) {
                let mut a = 0.0;
                for (i, kv) in k.iter().enumerate() {
                    a += kv * self.v[y * w + x + i - r as usize];
                }
                row[x] = a;
            }
        });
        let mut out = vec![f32::NAN; w * h];
        out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            if y < r as usize || y + r as usize >= h {
                return;
            }
            for (x, o) in row.iter_mut().enumerate() {
                let mut a = 0.0;
                for (i, kv) in k.iter().enumerate() {
                    a += kv * tmp[(y + i - r as usize) * w + x];
                }
                *o = a;
            }
        });
        Grey { w, h, v: out }
    }
}

/// Whether (x, y) is a FAST-9 corner at threshold `t`.
#[inline]
fn is_fast(g: &Grey, x: usize, y: usize, t: f32) -> bool {
    let p = g.at(x, y);
    if !p.is_finite() {
        return false;
    }
    let mut ring = [0i8; 16];
    // Quick rejection on the four compass points: a 9-arc covers at least two of them.
    for (k, &(dx, dy)) in CIRCLE.iter().enumerate() {
        let q = g.at((x as i32 + dx) as usize, (y as i32 + dy) as usize);
        if !q.is_finite() {
            return false;
        }
        ring[k] = if q > p + t {
            1
        } else if q < p - t {
            -1
        } else {
            0
        };
    }
    let compass = [ring[0], ring[4], ring[8], ring[12]];
    if compass.iter().filter(|&&v| v == 1).count() < 2
        && compass.iter().filter(|&&v| v == -1).count() < 2
    {
        return false;
    }
    for want in [1i8, -1] {
        let mut run = 0;
        for k in 0..32 {
            if ring[k % 16] == want {
                run += 1;
                if run >= 9 {
                    return true;
                }
            } else {
                run = 0;
            }
        }
    }
    false
}

/// The Harris measure at (x, y) over a 7×7 window of central-difference gradients.
fn harris(g: &Grey, x: usize, y: usize) -> f32 {
    let (mut a, mut b, mut c) = (0.0f32, 0.0f32, 0.0f32);
    for yy in y - 3..=y + 3 {
        for xx in x - 3..=x + 3 {
            let ix = 0.5 * (g.at(xx + 1, yy) - g.at(xx - 1, yy));
            let iy = 0.5 * (g.at(xx, yy + 1) - g.at(xx, yy - 1));
            a += ix * ix;
            b += ix * iy;
            c += iy * iy;
        }
    }
    let r = a * c - b * b - 0.04 * (a + c) * (a + c);
    if r.is_finite() { r } else { f32::NEG_INFINITY }
}

/// The intensity-centroid orientation of the disc around (x, y).
fn orientation(g: &Grey, x: f32, y: f32) -> f32 {
    let r = PATCH as i32;
    let (mut m10, mut m01) = (0.0f32, 0.0f32);
    let (cx, cy) = (x.round() as i32, y.round() as i32);
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy > r * r {
                continue;
            }
            let v = g.at((cx + dx) as usize, (cy + dy) as usize);
            if v.is_finite() {
                m10 += dx as f32 * v;
                m01 += dy as f32 * v;
            }
        }
    }
    m01.atan2(m10)
}

/// The test pattern: 256 point pairs (x1, y1, x2, y2) within the patch, from an isotropic
/// Gaussian of σ = 31/5 with a fixed seed.
fn pattern() -> &'static [[f32; 4]; 256] {
    static P: std::sync::OnceLock<[[f32; 4]; 256]> = std::sync::OnceLock::new();
    P.get_or_init(|| {
        let mut s = 0x9E37_79B9_7F4A_7C15u64;
        let mut uniform = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            ((s >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        };
        let sigma = 31.0 / 5.0;
        let mut out = [[0f32; 4]; 256];
        for pair in out.iter_mut() {
            loop {
                // Box–Muller, redrawn until all four coordinates fall inside the patch
                // and the two points differ.
                let mut v = [0f64; 4];
                for k in 0..2 {
                    let (u1, u2) = (uniform(), uniform());
                    let m = (-2.0 * u1.ln()).sqrt() * sigma;
                    let a = std::f64::consts::TAU * u2;
                    v[2 * k] = m * a.cos();
                    v[2 * k + 1] = m * a.sin();
                }
                let v = v.map(|c| c.round());
                if v.iter().all(|c| c.abs() <= f64::from(PATCH)) && (v[0], v[1]) != (v[2], v[3]) {
                    *pair = v.map(|c| c as f32);
                    break;
                }
            }
        }
        out
    })
}

/// The steered BRIEF descriptor at (x, y) for orientation `angle` on the smoothed image;
/// `None` when the patch reads an unusable pixel.
fn brief(s: &Grey, x: f32, y: f32, angle: f32) -> Option<Descriptor> {
    let (sn, c) = angle.sin_cos();
    let mut d = [0u64; 4];
    for (k, p) in pattern().iter().enumerate() {
        let rot = |px: f32, py: f32| (x + c * px - sn * py, y + sn * px + c * py);
        let (ax, ay) = rot(p[0], p[1]);
        let (bx, by) = rot(p[2], p[3]);
        let (a, b) = (s.sample(ax, ay), s.sample(bx, by));
        if !(a.is_finite() && b.is_finite()) {
            return None;
        }
        if a < b {
            d[k / 64] |= 1 << (k % 64);
        }
    }
    Some(d)
}

/// The orientation and descriptor at a given position (rotation invariance tests).
pub fn describe(g: &Grey, x: f32, y: f32) -> Option<(f32, Descriptor)> {
    let s = g.smooth(2.0);
    let a = orientation(g, x, y);
    brief(&s, x, y, a).map(|d| (a, d))
}

/// The corners and descriptors of `g`.
pub fn detect(g: &Grey, o: &DetectOptions) -> Features {
    let (w, h) = (g.w, g.h);
    if w <= 2 * BORDER || h <= 2 * BORDER {
        return Features::default();
    }
    // FAST at falling thresholds until there are enough corners.
    let mut corners: Vec<(usize, usize)> = Vec::new();
    for t in [0.06f32, 0.04, 0.025, 0.015, 0.008] {
        corners = (BORDER..h - BORDER)
            .into_par_iter()
            .flat_map_iter(|y| {
                (BORDER..w - BORDER)
                    .filter(move |&x| is_fast(g, x, y, t))
                    .map(move |x| (x, y))
            })
            .collect();
        if corners.len() >= o.wanted * 3 {
            break;
        }
    }
    // Harris scores and 3×3 non-maximum suppression among corners.
    let mut score = vec![f32::NEG_INFINITY; w * h];
    let scored: Vec<(usize, usize, f32)> = corners
        .par_iter()
        .map(|&(x, y)| (x, y, harris(g, x, y)))
        .collect();
    for &(x, y, s) in &scored {
        score[y * w + x] = s;
    }
    let kept: Vec<(usize, usize, f32)> = scored
        .into_iter()
        .filter(|&(x, y, s)| {
            s.is_finite()
                && s > 0.0
                && (y - 1..=y + 1).all(|yy| {
                    (x - 1..=x + 1).all(|xx| {
                        let q = score[yy * w + xx];
                        (xx, yy) == (x, y) || q < s || (q == s && (yy, xx) > (y, x))
                    })
                })
        })
        .collect();
    // Grid bucketing: the strongest per cell.
    let cell = (w.max(h) / o.cells.max(1)).max(8);
    let (cw, ch) = (w.div_ceil(cell), h.div_ceil(cell));
    let per = o.wanted.div_ceil(cw * ch).max(1);
    let mut buckets: Vec<Vec<(usize, usize, f32)>> = vec![Vec::new(); cw * ch];
    for k in kept {
        buckets[(k.1 / cell) * cw + k.0 / cell].push(k);
    }
    let mut chosen: Vec<(usize, usize, f32)> = Vec::new();
    for b in &mut buckets {
        b.sort_by(|a, c| c.2.total_cmp(&a.2));
        chosen.extend(b.iter().take(per));
    }
    let s = g.smooth(2.0);
    let found: Vec<(Keypoint, Descriptor)> = chosen
        .par_iter()
        .filter_map(|&(x, y, score)| {
            let (fx, fy) = (x as f32, y as f32);
            let angle = orientation(g, fx, fy);
            let d = brief(&s, fx, fy, angle)?;
            Some((
                Keypoint {
                    x: fx,
                    y: fy,
                    score,
                    angle,
                },
                d,
            ))
        })
        .collect();
    let (points, descriptors) = found.into_iter().unzip();
    Features {
        points,
        descriptors,
    }
}

#[inline]
pub fn hamming(a: &Descriptor, b: &Descriptor) -> u32 {
    (a[0] ^ b[0]).count_ones()
        + (a[1] ^ b[1]).count_ones()
        + (a[2] ^ b[2]).count_ones()
        + (a[3] ^ b[3]).count_ones()
}

/// The best and second-best distances and the best index of `d` among `set`.
fn best_two(d: &Descriptor, set: &[Descriptor]) -> (u32, u32, usize) {
    let (mut b1, mut b2, mut at) = (u32::MAX, u32::MAX, usize::MAX);
    for (j, e) in set.iter().enumerate() {
        let h = hamming(d, e);
        if h < b1 {
            b2 = b1;
            b1 = h;
            at = j;
        } else if h < b2 {
            b2 = h;
        }
    }
    (b1, b2, at)
}

/// A match: (index in `a`, index in `b`, distance).
pub type Match = (usize, usize, u32);

/// Matches of `a`'s features in `b` that pass the ratio test (`ratio`, 0.8) both ways and the
/// cross-check, and are no further than `max` bits apart.
pub fn match_features(a: &Features, b: &Features, ratio: f32, max: u32) -> Vec<Match> {
    if a.descriptors.is_empty() || b.descriptors.is_empty() {
        return Vec::new();
    }
    let ab: Vec<(u32, u32, usize)> = a
        .descriptors
        .par_iter()
        .map(|d| best_two(d, &b.descriptors))
        .collect();
    let ba: Vec<usize> = b
        .descriptors
        .par_iter()
        .map(|d| best_two(d, &a.descriptors).2)
        .collect();
    ab.iter()
        .enumerate()
        .filter(|&(i, &(d1, d2, j))| {
            j != usize::MAX
                && d1 <= max
                && (d2 == u32::MAX || (d1 as f32) < ratio * d2 as f32)
                && ba[j] == i
        })
        .map(|(i, &(d1, _, j))| (i, j, d1))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A textured test image (blobs and edges at several scales) seen rotated by `angle`
    /// about (cx, cy).
    fn texture(w: usize, h: usize, angle: f64, cx: f64, cy: f64) -> Proxy {
        let (s, c) = angle.sin_cos();
        let v = (0..w * h)
            .map(|k| {
                let (x, y) = ((k % w) as f64 - cx, (k / w) as f64 - cy);
                // The scene position this pixel shows.
                let (u, v) = (c * x + s * y, -s * x + c * y);
                let mut t = 0.5;
                let mut r = 0x1234_5678u64;
                for _ in 0..60 {
                    r = r
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    let bx = ((r >> 33) % 400) as f64 - 200.0;
                    r = r
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    let by = ((r >> 33) % 400) as f64 - 200.0;
                    let rad = 4.0 + ((r >> 20) % 12) as f64;
                    let k = if (r >> 40) & 1 == 0 { 0.4 } else { -0.35 };
                    let d2 = ((u - bx).powi(2) + (v - by).powi(2)) / (rad * rad);
                    t += k * (-d2).exp();
                }
                t += 0.1 * (u / 9.0).sin() * (v / 13.0).cos();
                t as f32
            })
            .collect();
        Proxy { w, h, v }
    }

    #[test]
    fn steered_brief_is_rotation_invariant() {
        let (w, h) = (240usize, 240usize);
        let (cx, cy) = (119.5, 119.5);
        let base = Grey::of(&texture(w, h, 0.0, cx, cy));
        let f = detect(&base, &DetectOptions::default());
        assert!(f.points.len() > 30, "{} corners", f.points.len());
        // Corners near the centre stay inside every rotated image.
        let central: Vec<usize> = (0..f.points.len())
            .filter(|&i| {
                let p = f.points[i];
                ((p.x as f64 - cx).powi(2) + (p.y as f64 - cy).powi(2)).sqrt() < 60.0
            })
            .collect();
        assert!(central.len() >= 8);
        let mut worst_mean = 0.0f64;
        for deg in (0..360).step_by(30) {
            let a = f64::from(deg).to_radians();
            let g = Grey::of(&texture(w, h, a, cx, cy));
            let (s, c) = a.sin_cos();
            let mut sum = 0u32;
            for &i in &central {
                let p = f.points[i];
                let (u, v) = (p.x as f64 - cx, p.y as f64 - cy);
                // The scene point (u, v) shows at pixel R(a)·(u, v) in the rotated image.
                let (x, y) = (c * u - s * v + cx, s * u + c * v + cy);
                let (_, d) = describe(&g, x as f32, y as f32).expect("inside");
                sum += hamming(&d, &f.descriptors[i]);
            }
            let mean = f64::from(sum) / central.len() as f64;
            worst_mean = worst_mean.max(mean);
        }
        // Unrelated descriptors differ in about 128 bits.
        assert!(worst_mean < 32.0, "mean distance {worst_mean:.1} of 256");
    }

    #[test]
    fn matching_pairs_rotated_views() {
        let (w, h) = (240usize, 240usize);
        let a = Grey::of(&texture(w, h, 0.0, 119.5, 119.5));
        let b = Grey::of(&texture(w, h, 0.5, 119.5, 119.5));
        let fa = detect(&a, &DetectOptions::default());
        let fb = detect(&b, &DetectOptions::default());
        let m = match_features(&fa, &fb, 0.8, 80);
        let (s, c) = 0.5f64.sin_cos();
        let good = m
            .iter()
            .filter(|&&(i, j, _)| {
                let (p, q) = (fa.points[i], fb.points[j]);
                let (u, v) = (p.x as f64 - 119.5, p.y as f64 - 119.5);
                let (x, y) = (c * u - s * v + 119.5, s * u + c * v + 119.5);
                (x - q.x as f64).hypot(y - q.y as f64) < 2.0
            })
            .count();
        assert!(good >= 15, "{good} good of {} matches", m.len());
        assert!(good * 10 >= m.len() * 8, "{good} good of {}", m.len());
    }
}
