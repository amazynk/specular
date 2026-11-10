//! Plane-to-plane projective maps (homographies), in f64: from four point pairs exactly, or
//! from more by least squares (the normalised direct linear transform of Hartley, "In
//! defense of the eight-point algorithm", 1997). Shared by the chart profiles and the
//! perspective tools.

/// A point pair (from, to).
pub type Pair = ((f64, f64), (f64, f64));

/// `p' = H p` in homogeneous coordinates, row-major, `h[2][2]` normalised to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Homography(pub [[f64; 3]; 3]);

impl Homography {
    pub const IDENTITY: Homography =
        Homography([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);

    /// Maps `p`; `None` on the line at infinity.
    pub fn apply(&self, p: (f64, f64)) -> Option<(f64, f64)> {
        let h = &self.0;
        let w = h[2][0] * p.0 + h[2][1] * p.1 + h[2][2];
        if w.abs() < 1e-12 || !w.is_finite() {
            return None;
        }
        Some((
            (h[0][0] * p.0 + h[0][1] * p.1 + h[0][2]) / w,
            (h[1][0] * p.0 + h[1][1] * p.1 + h[1][2]) / w,
        ))
    }

    /// The inverse map, when there is one.
    pub fn inverse(&self) -> Option<Homography> {
        let m = &self.0;
        let c = |a: usize, b: usize, c: usize, d: usize| m[a][b] * m[c][d] - m[a][d] * m[c][b];
        let det = m[0][0] * c(1, 1, 2, 2) - m[0][1] * c(1, 0, 2, 2) + m[0][2] * c(1, 0, 2, 1);
        if det.abs() < 1e-18 || !det.is_finite() {
            return None;
        }
        let inv = [
            [c(1, 1, 2, 2), -c(0, 1, 2, 2), c(0, 1, 1, 2)],
            [-c(1, 0, 2, 2), c(0, 0, 2, 2), -c(0, 0, 1, 2)],
            [c(1, 0, 2, 1), -c(0, 0, 2, 1), c(0, 0, 1, 1)],
        ];
        Homography(inv.map(|r| r.map(|v| v / det))).normalised()
    }

    fn normalised(self) -> Option<Homography> {
        let s = self.0[2][2];
        if s.abs() < 1e-15 || !s.is_finite() {
            // Keep the scale of the largest entry instead.
            let big = self.0.iter().flatten().fold(0f64, |a, v| a.max(v.abs()));
            return (big > 0.0 && big.is_finite())
                .then(|| Homography(self.0.map(|r| r.map(|v| v / big))));
        }
        Some(Homography(self.0.map(|r| r.map(|v| v / s))))
    }

    /// The map taking `from[i]` to `to[i]` (four pairs, no three points on a line).
    pub fn from_quads(from: [(f64, f64); 4], to: [(f64, f64); 4]) -> Option<Homography> {
        Homography::fit(&from.into_iter().zip(to).collect::<Vec<_>>())
    }

    /// The least-squares map for `pairs` (from, to): at least four, normalised DLT.
    pub fn fit(pairs: &[Pair]) -> Option<Homography> {
        if pairs.len() < 4 {
            return None;
        }
        let (ta, a) = normaliser(pairs.iter().map(|p| p.0));
        let (tb, b) = normaliser(pairs.iter().map(|p| p.1));
        // Fix h22 = 1 and solve the 8 unknowns by normal equations (well conditioned after
        // normalisation; a chart never maps its centre to infinity).
        let mut ata = [[0f64; 8]; 8];
        let mut atb = [0f64; 8];
        for (p, q) in a.iter().zip(&b) {
            let (x, y) = *p;
            let (u, v) = *q;
            let r1 = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y];
            let r2 = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y];
            for (r, t) in [(r1, u), (r2, v)] {
                for i in 0..8 {
                    for j in 0..8 {
                        ata[i][j] += r[i] * r[j];
                    }
                    atb[i] += r[i] * t;
                }
            }
        }
        let h = solve8(ata, atb)?;
        let hn = Homography([[h[0], h[1], h[2]], [h[3], h[4], h[5]], [h[6], h[7], 1.0]]);
        // H = Tb⁻¹ · Hn · Ta.
        let tbi = tb.inverse()?;
        Homography(mul(&mul(&tbi.0, &hn.0), &ta.0)).normalised()
    }
}

fn mul(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

/// Translates the points to their centroid and scales them to a mean distance of √2.
fn normaliser(points: impl Iterator<Item = (f64, f64)>) -> (Homography, Vec<(f64, f64)>) {
    let p: Vec<(f64, f64)> = points.collect();
    let n = p.len() as f64;
    let (cx, cy) = p
        .iter()
        .fold((0.0, 0.0), |a, q| (a.0 + q.0 / n, a.1 + q.1 / n));
    let d = p
        .iter()
        .map(|q| ((q.0 - cx).powi(2) + (q.1 - cy).powi(2)).sqrt())
        .sum::<f64>()
        / n;
    let s = if d > 1e-12 {
        std::f64::consts::SQRT_2 / d
    } else {
        1.0
    };
    let t = Homography([[s, 0.0, -s * cx], [0.0, s, -s * cy], [0.0, 0.0, 1.0]]);
    let out = p.iter().map(|q| (s * (q.0 - cx), s * (q.1 - cy))).collect();
    (t, out)
}

/// Gaussian elimination with partial pivoting.
fn solve8(mut a: [[f64; 8]; 8], mut b: [f64; 8]) -> Option<[f64; 8]> {
    for c in 0..8 {
        let p = (c..8).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))?;
        if a[p][c].abs() < 1e-12 {
            return None;
        }
        a.swap(c, p);
        b.swap(c, p);
        let (pivot, pb) = (a[c], b[c]);
        for (row, br) in a.iter_mut().zip(b.iter_mut()).skip(c + 1) {
            let f = row[c] / pivot[c];
            for (x, y) in row[c..].iter_mut().zip(&pivot[c..]) {
                *x -= f * y;
            }
            *br -= f * pb;
        }
    }
    let mut x = [0f64; 8];
    for r in (0..8).rev() {
        let s: f64 = (r + 1..8).map(|k| a[r][k] * x[k]).sum();
        x[r] = (b[r] - s) / a[r][r];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_pairs_map_exactly_and_invert() {
        let from = [(0.0, 0.0), (5.0, 0.0), (5.0, 3.0), (0.0, 3.0)];
        let to = [(102.0, 51.0), (611.0, 80.0), (590.0, 402.0), (95.0, 370.0)];
        let h = Homography::from_quads(from, to).unwrap();
        for (a, b) in from.iter().zip(&to) {
            let p = h.apply(*a).unwrap();
            assert!(
                (p.0 - b.0).abs() < 1e-8 && (p.1 - b.1).abs() < 1e-8,
                "{p:?} vs {b:?}"
            );
        }
        let back = h.inverse().unwrap();
        let q = back.apply(h.apply((2.5, 1.5)).unwrap()).unwrap();
        assert!((q.0 - 2.5).abs() < 1e-9 && (q.1 - 1.5).abs() < 1e-9);
    }

    #[test]
    fn many_noisy_pairs_fit_by_least_squares() {
        let truth = Homography([[80.0, 6.0, 100.0], [-4.0, 78.0, 50.0], [1e-4, 2e-4, 1.0]]);
        let pairs: Vec<_> = (0..24)
            .map(|i| {
                let p = ((i % 6) as f64, (i / 6) as f64);
                let q = truth.apply(p).unwrap();
                let e = if i % 2 == 0 { 0.05 } else { -0.05 };
                (p, (q.0 + e, q.1 - e))
            })
            .collect();
        let h = Homography::fit(&pairs).unwrap();
        let (a, b) = (
            h.apply((2.5, 1.5)).unwrap(),
            truth.apply((2.5, 1.5)).unwrap(),
        );
        assert!((a.0 - b.0).abs() < 0.1 && (a.1 - b.1).abs() < 0.1);
        assert!(Homography::fit(&pairs[..3]).is_none());
    }
}
