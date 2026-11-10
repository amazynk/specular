//! Grey images, their local-threshold binarisation and a perspective transform.

/// A grey image, 8 bits a pixel, row by row.
#[derive(Clone, Debug)]
pub struct Gray {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u8>,
}

impl Gray {
    /// From RGBA8 (premultiplied or not): luma, transparent pixels as white.
    pub fn from_rgba(w: usize, h: usize, rgba: &[u8]) -> Gray {
        let px = rgba
            .chunks_exact(4)
            .take(w * h)
            .map(|p| {
                let y = (u32::from(p[0]) * 77 + u32::from(p[1]) * 150 + u32::from(p[2]) * 29) >> 8;
                let a = u32::from(p[3]);
                // over white
                ((y * a + 255 * (255 - a)) / 255) as u8
            })
            .collect();
        Gray { w, h, px }
    }

    /// Smoothed by a 3×3 box (against noise).
    pub fn smoothed(&self) -> Gray {
        let (w, h) = (self.w, self.h);
        let mut px = self.px.clone();
        if w < 3 || h < 3 {
            return Gray { w, h, px };
        }
        // Each column's sum over three rows, then three columns of those.
        let mut col = vec![0u16; w];
        for y in 1..h - 1 {
            let (a, b, c) = (
                &self.px[(y - 1) * w..y * w],
                &self.px[y * w..(y + 1) * w],
                &self.px[(y + 1) * w..(y + 2) * w],
            );
            for (((s, &a), &b), &c) in col.iter_mut().zip(a).zip(b).zip(c) {
                *s = u16::from(a) + u16::from(b) + u16::from(c);
            }
            let out = &mut px[y * w + 1..(y + 1) * w - 1];
            for (o, t) in out.iter_mut().zip(col.windows(3)) {
                *o = ((t[0] + t[1] + t[2]) / 9) as u8;
            }
        }
        Gray { w, h, px }
    }

    /// Half the size, each pixel the mean of four.
    pub fn half(&self) -> Gray {
        let (w, h) = (self.w / 2, self.h / 2);
        let mut px = vec![0u8; w * h];
        for y in 0..h {
            let a = &self.px[2 * y * self.w..2 * y * self.w + 2 * w];
            let b = &self.px[(2 * y + 1) * self.w..(2 * y + 1) * self.w + 2 * w];
            let out = &mut px[y * w..(y + 1) * w];
            for ((o, a), b) in out.iter_mut().zip(a.chunks_exact(2)).zip(b.chunks_exact(2)) {
                let s = u16::from(a[0]) + u16::from(a[1]) + u16::from(b[0]) + u16::from(b[1]);
                *o = ((s + 2) / 4) as u8;
            }
        }
        Gray { w, h, px }
    }
}

/// Dark (true) and light pixels.
#[derive(Clone, Debug)]
pub struct Binary {
    pub w: usize,
    pub h: usize,
    pub dark: Vec<bool>,
}

impl Binary {
    pub fn get(&self, x: usize, y: usize) -> bool {
        self.dark[y * self.w + x]
    }

    /// Whether the pixel at (x, y) (rounded, inside) is dark.
    pub fn at(&self, x: f32, y: f32) -> Option<bool> {
        if !(x >= 0.0 && y >= 0.0) {
            return None;
        }
        let (xi, yi) = (x as usize, y as usize);
        (xi < self.w && yi < self.h).then(|| self.dark[yi * self.w + xi])
    }

    pub fn inverted(&self) -> Binary {
        Binary {
            w: self.w,
            h: self.h,
            dark: self.dark.iter().map(|d| !d).collect(),
        }
    }
}

/// The runs of dark pixels of a binary picture, row by row: found once, shared by the QR
/// finder scan and the Data Matrix outline search.
#[derive(Clone, Debug, Default)]
pub struct Runs {
    /// Where each row's runs start in `runs`, and one past the last row.
    pub row: Vec<u32>,
    /// First and last x of each run.
    pub runs: Vec<(u32, u32)>,
}

impl Runs {
    pub fn of(b: &Binary) -> Runs {
        let mut row = Vec::with_capacity(b.h + 1);
        let mut runs = Vec::new();
        for y in 0..b.h {
            row.push(runs.len() as u32);
            push_runs(&b.dark[y * b.w..(y + 1) * b.w], &mut runs);
        }
        row.push(runs.len() as u32);
        Runs { row, runs }
    }

    /// The runs of the inverted picture (`w` pixels a row): the gaps.
    pub fn inverted(&self, w: usize) -> Runs {
        let mut out = Runs {
            row: Vec::with_capacity(self.row.len()),
            runs: Vec::with_capacity(self.runs.len() + self.row.len()),
        };
        for y in 0..self.row.len().saturating_sub(1) {
            out.row.push(out.runs.len() as u32);
            let mut x = 0u32;
            for &(a, c) in self.row(y) {
                if a > x {
                    out.runs.push((x, a - 1));
                }
                x = c + 1;
            }
            if (x as usize) < w {
                out.runs.push((x, w as u32 - 1));
            }
        }
        out.row.push(out.runs.len() as u32);
        out
    }

    /// The runs of row `y`.
    pub fn row(&self, y: usize) -> &[(u32, u32)] {
        &self.runs[self.row[y] as usize..self.row[y + 1] as usize]
    }
}

/// The dark runs of one row, added to `runs`: long light or dark stretches compared
/// sixteen pixels at a time.
fn push_runs(line: &[bool], runs: &mut Vec<(u32, u32)>) {
    const N: usize = 16;
    let w = line.len();
    let mut x = 0;
    loop {
        while x + N <= w && line[x..x + N] == [false; N] {
            x += N;
        }
        while x < w && !line[x] {
            x += 1;
        }
        if x >= w {
            return;
        }
        let s = x;
        while x + N <= w && line[x..x + N] == [true; N] {
            x += N;
        }
        while x < w && line[x] {
            x += 1;
        }
        runs.push((s as u32, x as u32 - 1));
    }
}

const BLOCK: usize = 8;

/// The range of grey under which a block counts as flat: 24, or more in a noisy picture
/// (five times the noise, estimated from the median step between neighbouring pixels on
/// every fourth row), at most 160 so that edges stay edges.
fn flat_range(g: &Gray) -> u8 {
    let mut hist = [0u32; 256];
    let mut n = 0u32;
    for y in (0..g.h).step_by(4) {
        let row = &g.px[y * g.w..(y + 1) * g.w];
        for p in row.windows(2) {
            hist[usize::from(p[0].abs_diff(p[1]))] += 1;
            n += 1;
        }
    }
    let (mut acc, mut median) = (0u32, 0usize);
    for (i, &c) in hist.iter().enumerate() {
        acc += c;
        if 2 * acc >= n {
            median = i;
            break;
        }
    }
    // median |difference| of two samples ≈ 0.95 σ
    (5 * median as u32).clamp(24, 160) as u8
}

/// Sum, least and most of eight pixels.
#[inline(always)]
fn stats8(p: &[u8; BLOCK]) -> (u32, u8, u8) {
    let s = u32::from(p[0])
        + u32::from(p[1])
        + u32::from(p[2])
        + u32::from(p[3])
        + u32::from(p[4])
        + u32::from(p[5])
        + u32::from(p[6])
        + u32::from(p[7]);
    let mn = p[0]
        .min(p[1])
        .min(p[2].min(p[3]))
        .min(p[4].min(p[5]).min(p[6].min(p[7])));
    let mx = p[0]
        .max(p[1])
        .max(p[2].max(p[3]))
        .max(p[4].max(p[5]).max(p[6].max(p[7])));
    (s, mn, mx)
}

/// Local thresholds over 8×8 blocks: each block's mean (or, where flat, a guess from its
/// darkest pixel and its neighbours), averaged over the 5×5 blocks around it.
pub fn binarize(g: &Gray) -> Binary {
    binarize_runs(g).0
}

/// As [`binarize`], with the dark runs.
pub fn binarize_runs(g: &Gray) -> (Binary, Runs) {
    let (w, h) = (g.w, g.h);
    if w < BLOCK * 5 || h < BLOCK * 5 {
        // small: one threshold, halfway between the extremes
        let (mn, mx) =
            g.px.iter()
                .fold((255u8, 0u8), |(a, b), &p| (a.min(p), b.max(p)));
        let t = (u16::from(mn) + u16::from(mx)) / 2;
        let b = Binary {
            w,
            h,
            dark: g.px.iter().map(|&p| u16::from(p) <= t).collect(),
        };
        let runs = Runs::of(&b);
        return (b, runs);
    }
    let bw = w.div_ceil(BLOCK);
    let bh = h.div_ceil(BLOCK);
    let flat = flat_range(g);
    let mut avg = vec![0u32; bw * bh];
    for by in 0..bh {
        for bx in 0..bw {
            let x0 = (bx * BLOCK).min(w - BLOCK);
            let y0 = (by * BLOCK).min(h - BLOCK);
            let (mut sum, mut mn, mut mx) = (0u32, 255u8, 0u8);
            for y in y0..y0 + BLOCK {
                let r: &[u8; BLOCK] = g.px[y * w + x0..y * w + x0 + BLOCK].try_into().unwrap();
                let (s, a, b) = stats8(r);
                sum += s;
                mn = mn.min(a);
                mx = mx.max(b);
            }
            let mut a = sum / (BLOCK * BLOCK) as u32;
            if mx - mn <= flat {
                // flat: probably all light or all dark; assume light unless the
                // neighbours already seen say otherwise
                a = u32::from(mn) / 2;
                if by > 0 && bx > 0 {
                    let nb = (avg[(by - 1) * bw + bx]
                        + 2 * avg[by * bw + bx - 1]
                        + avg[(by - 1) * bw + bx - 1])
                        / 4;
                    if u32::from(mn) < nb {
                        a = nb;
                    }
                }
            }
            avg[by * bw + bx] = a;
        }
    }
    // Each block's threshold: the mean of the 5 × 5 blocks around it.
    let mut thr = vec![0u8; bw * bh];
    for by in 0..bh {
        for bx in 0..bw {
            let cx = bx.clamp(2, bw.saturating_sub(3).max(2));
            let cy = by.clamp(2, bh.saturating_sub(3).max(2));
            let mut s = 0u32;
            let mut n = 0u32;
            for yy in cy.saturating_sub(2)..=(cy + 2).min(bh - 1) {
                for xx in cx.saturating_sub(2)..=(cx + 2).min(bw - 1) {
                    s += avg[yy * bw + xx];
                    n += 1;
                }
            }
            thr[by * bw + bx] = (s / n.max(1)).min(255) as u8;
        }
    }
    // Row by row (the last block row and column overlap the one before and win there),
    // gathering the dark runs on the way.
    let mut dark = vec![false; w * h];
    let mut runs = Runs {
        row: Vec::with_capacity(h + 1),
        runs: Vec::new(),
    };
    for y in 0..h {
        let by = if y >= h - BLOCK { bh - 1 } else { y / BLOCK };
        runs.row.push(runs.runs.len() as u32);
        let src = &g.px[y * w..(y + 1) * w];
        let out = &mut dark[y * w..(y + 1) * w];
        for bx in 0..bw {
            let x0 = (bx * BLOCK).min(w - BLOCK);
            let t = thr[by * bw + bx];
            let p: &[u8; BLOCK] = src[x0..x0 + BLOCK].try_into().unwrap();
            let o: &mut [bool; BLOCK] = (&mut out[x0..x0 + BLOCK]).try_into().unwrap();
            *o = [
                p[0] <= t,
                p[1] <= t,
                p[2] <= t,
                p[3] <= t,
                p[4] <= t,
                p[5] <= t,
                p[6] <= t,
                p[7] <= t,
            ];
        }
        push_runs(out, &mut runs.runs);
    }
    runs.row.push(runs.runs.len() as u32);
    (Binary { w, h, dark }, runs)
}

/// A point and where a transform should take it.
pub type PointPair = ((f64, f64), (f64, f64));

/// A projective transform of the plane.
#[derive(Clone, Copy, Debug)]
pub struct Homography {
    m: [f64; 9],
}

impl Homography {
    /// The unit square (0,0), (1,0), (1,1), (0,1) to a quadrilateral.
    fn square_to_quad(q: [(f64, f64); 4]) -> Homography {
        let [(x0, y0), (x1, y1), (x2, y2), (x3, y3)] = q;
        let dx3 = x0 - x1 + x2 - x3;
        let dy3 = y0 - y1 + y2 - y3;
        if dx3.abs() < 1e-12 && dy3.abs() < 1e-12 {
            return Homography {
                m: [x1 - x0, x2 - x1, x0, y1 - y0, y2 - y1, y0, 0.0, 0.0, 1.0],
            };
        }
        let (dx1, dx2, dy1, dy2) = (x1 - x2, x3 - x2, y1 - y2, y3 - y2);
        let den = dx1 * dy2 - dx2 * dy1;
        let g = (dx3 * dy2 - dx2 * dy3) / den;
        let h = (dx1 * dy3 - dx3 * dy1) / den;
        Homography {
            m: [
                x1 - x0 + g * x1,
                x3 - x0 + h * x3,
                x0,
                y1 - y0 + g * y1,
                y3 - y0 + h * y3,
                y0,
                g,
                h,
                1.0,
            ],
        }
    }

    /// The inverse transform (up to scale, which a projective map ignores).
    pub fn inverse(&self) -> Homography {
        self.adjugate()
    }

    /// The affine map (x, y) → (a x + b y + c, d x + e y + f).
    pub fn affine(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> Homography {
        Homography {
            m: [a, b, c, d, e, f, 0.0, 0.0, 1.0],
        }
    }

    fn adjugate(&self) -> Homography {
        let [a, b, c, d, e, f, g, h, i] = self.m;
        Homography {
            m: [
                e * i - f * h,
                c * h - b * i,
                b * f - c * e,
                f * g - d * i,
                a * i - c * g,
                c * d - a * f,
                d * h - e * g,
                b * g - a * h,
                a * e - b * d,
            ],
        }
    }

    /// `self`, then `o`.
    pub fn then(&self, o: &Homography) -> Homography {
        // o ∘ self
        let a = &o.m;
        let b = &self.m;
        let mut m = [0.0; 9];
        for r in 0..3 {
            for c in 0..3 {
                m[r * 3 + c] = (0..3).map(|k| a[r * 3 + k] * b[k * 3 + c]).sum();
            }
        }
        Homography { m }
    }

    /// The transform taking quadrilateral `from` to quadrilateral `to` (corners in order).
    pub fn quad_to_quad(from: [(f64, f64); 4], to: [(f64, f64); 4]) -> Homography {
        let s_to = Homography::square_to_quad(to);
        let s_from = Homography::square_to_quad(from).adjugate();
        s_from.then(&s_to)
    }

    /// The transform that best takes each `from` point to its `to` point (least squares,
    /// four pairs or more, coordinates normalised for a well-conditioned solve).
    pub fn fit(pairs: &[PointPair]) -> Option<Homography> {
        if pairs.len() < 4 {
            return None;
        }
        // normalise each side: centred, mean distance √2
        let norm = |pts: &mut dyn Iterator<Item = (f64, f64)>| {
            let v: Vec<(f64, f64)> = pts.collect();
            let n = v.len() as f64;
            let (cx, cy) = v
                .iter()
                .fold((0.0, 0.0), |a, p| (a.0 + p.0 / n, a.1 + p.1 / n));
            let d = v.iter().map(|p| (p.0 - cx).hypot(p.1 - cy)).sum::<f64>() / n;
            let s = if d > 1e-12 {
                std::f64::consts::SQRT_2 / d
            } else {
                1.0
            };
            (cx, cy, s)
        };
        let (fx, fy, fs) = norm(&mut pairs.iter().map(|p| p.0));
        let (tx, ty, ts) = norm(&mut pairs.iter().map(|p| p.1));
        // h0 u + h1 v + h2 - h6 u x - h7 v x = x, and the same for y
        let mut a = [[0.0f64; 9]; 8];
        for &((u, v), (x, y)) in pairs {
            let (u, v) = ((u - fx) * fs, (v - fy) * fs);
            let (x, y) = ((x - tx) * ts, (y - ty) * ts);
            for (row, rhs) in [
                ([u, v, 1.0, 0.0, 0.0, 0.0, -u * x, -v * x], x),
                ([0.0, 0.0, 0.0, u, v, 1.0, -u * y, -v * y], y),
            ] {
                for i in 0..8 {
                    for j in 0..8 {
                        a[i][j] += row[i] * row[j];
                    }
                    a[i][8] += row[i] * rhs;
                }
            }
        }
        // Gaussian elimination with partial pivoting
        for c in 0..8 {
            let p = (c..8).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))?;
            if a[p][c].abs() < 1e-12 {
                return None;
            }
            a.swap(c, p);
            for r in 0..8 {
                if r != c {
                    let f = a[r][c] / a[c][c];
                    for k in c..9 {
                        a[r][k] -= f * a[c][k];
                    }
                }
            }
        }
        let h: Vec<f64> = (0..8).map(|i| a[i][8] / a[i][i]).collect();
        let n = Homography {
            m: [h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], 1.0],
        };
        // undo the normalisations: T⁻¹ · N · F
        let f = Homography {
            m: [fs, 0.0, -fx * fs, 0.0, fs, -fy * fs, 0.0, 0.0, 1.0],
        };
        let t_inv = Homography {
            m: [1.0 / ts, 0.0, tx, 0.0, 1.0 / ts, ty, 0.0, 0.0, 1.0],
        };
        Some(f.then(&n).then(&t_inv))
    }

    pub fn map(&self, x: f64, y: f64) -> (f64, f64) {
        let m = &self.m;
        let w = m[6] * x + m[7] * y + m[8];
        (
            (m[0] * x + m[1] * y + m[2]) / w,
            (m[3] * x + m[4] * y + m[5]) / w,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fitted_homography_maps_its_points() {
        let to = [(5.0, 7.0), (40.0, 3.0), (45.0, 50.0), (2.0, 38.0)];
        let h = Homography::quad_to_quad([(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], to);
        let pairs: Vec<_> = (0..25)
            .map(|i| {
                let p = ((i % 5) as f64 * 2.5, (i / 5) as f64 * 2.5);
                (p, h.map(p.0, p.1))
            })
            .collect();
        let g = Homography::fit(&pairs).unwrap();
        for (p, q) in pairs {
            let r = g.map(p.0, p.1);
            assert!((r.0 - q.0).abs() < 1e-6 && (r.1 - q.1).abs() < 1e-6);
        }
    }

    #[test]
    fn homography_maps_corners() {
        let from = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let to = [(5.0, 7.0), (40.0, 3.0), (45.0, 50.0), (2.0, 38.0)];
        let h = Homography::quad_to_quad(from, to);
        for (f, t) in from.iter().zip(&to) {
            let (x, y) = h.map(f.0, f.1);
            assert!((x - t.0).abs() < 1e-9 && (y - t.1).abs() < 1e-9, "{x} {y}");
        }
    }

    #[test]
    fn runs_follow_the_pixels() {
        let (w, h) = (77, 53);
        let px: Vec<u8> = (0..w * h)
            .map(|i| ((i * 7919 + (i / w) * 31) % 251) as u8)
            .collect();
        let (b, runs) = binarize_runs(&Gray { w, h, px });
        let of = Runs::of(&b);
        assert_eq!((&runs.row, &runs.runs), (&of.row, &of.runs));
        let inv = Runs::of(&b.inverted());
        let r = runs.inverted(w);
        assert_eq!((&r.row, &r.runs), (&inv.row, &inv.runs));
    }

    #[test]
    fn binarize_finds_dark_on_a_gradient() {
        let (w, h) = (64, 64);
        let mut px = vec![0u8; w * h];
        for y in 0..h {
            for x in 0..w {
                let base = 120 + (x as u32 * 2) as u8 / 2;
                px[y * w + x] = if (x / 8 + y / 8) % 2 == 0 {
                    base / 3
                } else {
                    base
                };
            }
        }
        let b = binarize(&Gray { w, h, px });
        assert!(b.get(3, 3));
        assert!(!b.get(11, 3));
        assert!(b.get(60, 4) == ((60 / 8) % 2 == 0));
    }
}
