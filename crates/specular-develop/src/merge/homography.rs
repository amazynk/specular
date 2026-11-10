//! A plane-to-plane projective map (3×3, row-major, `h[8]` normally 1) and the small dense
//! linear algebra the merge's geometry needs: symmetric Cholesky and Jacobi eigen solves,
//! 3×3 rotations as angle-axis vectors.
//!
//! Self-contained on purpose: Develop's perspective correction has its own homography, and
//! the two can be unified later without either depending on the other.

// Small fixed-size matrices read clearest with indices.
#![allow(clippy::needless_range_loop)]

/// `p' = H p` in homogeneous coordinates, mapping reference positions to a frame's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Homography(pub [f64; 9]);

impl Default for Homography {
    fn default() -> Self {
        Homography::IDENTITY
    }
}

impl Homography {
    pub const IDENTITY: Homography = Homography([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);

    pub fn translation(tx: f64, ty: f64) -> Homography {
        Homography([1.0, 0.0, tx, 0.0, 1.0, ty, 0.0, 0.0, 1.0])
    }

    pub fn scale(sx: f64, sy: f64) -> Homography {
        Homography([sx, 0.0, 0.0, 0.0, sy, 0.0, 0.0, 0.0, 1.0])
    }

    /// Rotation by `angle` (counter-clockwise in a y-down image) and scale `s` about (cx, cy),
    /// then a translation.
    pub fn similarity(s: f64, angle: f64, tx: f64, ty: f64, cx: f64, cy: f64) -> Homography {
        let (sn, c) = angle.sin_cos();
        let (a, b) = (s * c, s * sn);
        Homography([
            a,
            -b,
            cx + tx - a * cx + b * cy,
            b,
            a,
            cy + ty - b * cx - a * cy,
            0.0,
            0.0,
            1.0,
        ])
    }

    #[inline]
    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        let h = &self.0;
        let w = h[6] * x + h[7] * y + h[8];
        (
            (h[0] * x + h[1] * y + h[2]) / w,
            (h[3] * x + h[4] * y + h[5]) / w,
        )
    }

    /// `self ∘ o`: first `o`, then `self`.
    pub fn then_after(&self, o: &Homography) -> Homography {
        Homography(mul3(&self.0, &o.0)).normalised()
    }

    /// `o ∘ self`: first `self`, then `o`.
    pub fn then(&self, o: &Homography) -> Homography {
        o.then_after(self)
    }

    /// Scaled so that `h[8]` is 1 (when it is not near 0).
    pub fn normalised(self) -> Homography {
        let s = self.0[8];
        if s.abs() > 1e-300 {
            Homography(self.0.map(|v| v / s))
        } else {
            self
        }
    }

    pub fn inverse(&self) -> Option<Homography> {
        inv3(&self.0).map(|m| Homography(m).normalised())
    }

    /// The map conjugated by `s`: `S H S⁻¹`, e.g. a proxy-unit homography in sample units.
    pub fn conjugate(&self, s: &Homography) -> Option<Homography> {
        Some(s.then_after(self).then_after(&s.inverse()?))
    }

    pub fn is_finite(&self) -> bool {
        self.0.iter().all(|v| v.is_finite())
    }

    /// The largest distance a corner of a `w × h` area moves.
    pub fn max_shift(&self, w: u32, h: u32) -> f64 {
        let (w, h) = (f64::from(w) - 1.0, f64::from(h) - 1.0);
        [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)]
            .iter()
            .map(|&(x, y)| {
                let (a, b) = self.apply(x, y);
                ((a - x).powi(2) + (b - y).powi(2)).sqrt()
            })
            .fold(0.0, f64::max)
    }

    /// The largest distance between where `self` and `o` put a corner or the centre of a
    /// `w × h` area.
    pub fn distance(&self, o: &Homography, w: u32, h: u32) -> f64 {
        let (w, h) = (f64::from(w) - 1.0, f64::from(h) - 1.0);
        [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h), (w / 2.0, h / 2.0)]
            .iter()
            .map(|&(x, y)| {
                let (p, q) = (self.apply(x, y), o.apply(x, y));
                ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt()
            })
            .fold(0.0, f64::max)
    }

    /// Moves no sample by more than 0.001 of a pixel over a `w × h` area.
    pub fn is_identity(&self, w: u32, h: u32) -> bool {
        self.max_shift(w, h) < 1e-3
    }

    /// The local area scale `|det J|` of the map at (x, y): below 1 the map shrinks.
    pub fn area_scale(&self, x: f64, y: f64) -> f64 {
        let h = &self.0;
        let w = h[6] * x + h[7] * y + h[8];
        let (px, py) = self.apply(x, y);
        let j00 = (h[0] - h[6] * px) / w;
        let j01 = (h[1] - h[7] * px) / w;
        let j10 = (h[3] - h[6] * py) / w;
        let j11 = (h[4] - h[7] * py) / w;
        (j00 * j11 - j01 * j10).abs()
    }
}

pub fn mul3(a: &[f64; 9], b: &[f64; 9]) -> [f64; 9] {
    std::array::from_fn(|k| {
        let (i, j) = (k / 3, k % 3);
        (0..3).map(|m| a[i * 3 + m] * b[m * 3 + j]).sum()
    })
}

pub fn transpose3(a: &[f64; 9]) -> [f64; 9] {
    std::array::from_fn(|k| a[(k % 3) * 3 + k / 3])
}

pub fn mul3v(a: &[f64; 9], v: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i * 3] * v[0] + a[i * 3 + 1] * v[1] + a[i * 3 + 2] * v[2])
}

pub fn det3(m: &[f64; 9]) -> f64 {
    m[0] * (m[4] * m[8] - m[5] * m[7]) - m[1] * (m[3] * m[8] - m[5] * m[6])
        + m[2] * (m[3] * m[7] - m[4] * m[6])
}

pub fn inv3(m: &[f64; 9]) -> Option<[f64; 9]> {
    let d = det3(m);
    if d.abs() < 1e-300 || !d.is_finite() {
        return None;
    }
    let c = |a: usize, b: usize, cc: usize, dd: usize| m[a] * m[dd] - m[b] * m[cc];
    let adj = [
        c(4, 5, 7, 8),
        -c(1, 2, 7, 8),
        c(1, 2, 4, 5),
        -c(3, 5, 6, 8),
        c(0, 2, 6, 8),
        -c(0, 2, 3, 5),
        c(3, 4, 6, 7),
        -c(0, 1, 6, 7),
        c(0, 1, 3, 4),
    ];
    Some(adj.map(|v| v / d))
}

/// The rotation matrix of angle-axis vector `w` (Rodrigues).
pub fn rotation(w: [f64; 3]) -> [f64; 9] {
    let t = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
    let (a, b) = if t < 1e-8 {
        (1.0 - t * t / 6.0, 0.5 - t * t / 24.0)
    } else {
        (t.sin() / t, (1.0 - t.cos()) / (t * t))
    };
    let [x, y, z] = w;
    [
        1.0 - b * (y * y + z * z),
        -a * z + b * x * y,
        a * y + b * x * z,
        a * z + b * x * y,
        1.0 - b * (x * x + z * z),
        -a * x + b * y * z,
        -a * y + b * x * z,
        a * x + b * y * z,
        1.0 - b * (x * x + y * y),
    ]
}

/// The angle-axis vector of rotation matrix `r`.
pub fn angle_axis(r: &[f64; 9]) -> [f64; 3] {
    let c = ((r[0] + r[4] + r[8] - 1.0) / 2.0).clamp(-1.0, 1.0);
    let t = c.acos();
    let v = [r[7] - r[5], r[2] - r[6], r[3] - r[1]];
    if t < 1e-7 {
        return v.map(|x| x / 2.0);
    }
    if std::f64::consts::PI - t < 1e-4 {
        // Near π: the axis from the largest diagonal of (R + I)/2.
        let d = [r[0], r[4], r[8]];
        let k = (0..3).max_by(|&a, &b| d[a].total_cmp(&d[b])).unwrap_or(0);
        let mut axis = [0.0; 3];
        axis[k] = ((d[k] + 1.0) / 2.0).max(0.0).sqrt();
        for j in 0..3 {
            if j != k {
                axis[j] = (r[k * 3 + j] + r[j * 3 + k]) / (4.0 * axis[k]);
            }
        }
        let n = axis.iter().map(|v| v * v).sum::<f64>().sqrt();
        // The sign from the antisymmetric part when it is still measurable.
        let s = if v.iter().zip(&axis).map(|(a, b)| a * b).sum::<f64>() < 0.0 {
            -1.0
        } else {
            1.0
        };
        return axis.map(|a| s * a / n * t);
    }
    let k = t / (2.0 * t.sin());
    v.map(|x| x * k)
}

/// The angle in radians between rotations `a` and `b`.
pub fn rotation_distance(a: &[f64; 9], b: &[f64; 9]) -> f64 {
    let r = mul3(&transpose3(a), b);
    let w = angle_axis(&r);
    (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt()
}

/// Solves `a x = b` for a symmetric positive definite `n × n` system (row-major `a`).
pub fn cholesky_solve(a: &[f64], b: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut l = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..=i {
            let mut s = a[i * n + j];
            for m in 0..j {
                s -= l[i * n + m] * l[j * n + m];
            }
            if i == j {
                if s <= 0.0 || !s.is_finite() {
                    return None;
                }
                l[i * n + i] = s.sqrt();
            } else {
                l[i * n + j] = s / l[j * n + j];
            }
        }
    }
    let mut y = vec![0.0; n];
    for i in 0..n {
        let mut s = b[i];
        for m in 0..i {
            s -= l[i * n + m] * y[m];
        }
        y[i] = s / l[i * n + i];
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut s = y[i];
        for m in i + 1..n {
            s -= l[m * n + i] * x[m];
        }
        x[i] = s / l[i * n + i];
    }
    Some(x)
}

/// Eigen decomposition of a symmetric `n × n` matrix by cyclic Jacobi rotations: the
/// eigenvalues ascending, with their unit eigenvectors (`vectors[k]` for `values[k]`).
pub fn symmetric_eigen(a: &[f64], n: usize) -> (Vec<f64>, Vec<Vec<f64>>) {
    let mut m = a.to_vec();
    let mut v = vec![0.0; n * n];
    for i in 0..n {
        v[i * n + i] = 1.0;
    }
    for _sweep in 0..100 {
        let off: f64 = (0..n)
            .flat_map(|i| (0..n).filter(move |&j| j != i).map(move |j| (i, j)))
            .map(|(i, j)| m[i * n + j] * m[i * n + j])
            .sum();
        let scale: f64 = (0..n).map(|i| m[i * n + i] * m[i * n + i]).sum::<f64>() + off;
        if off <= 1e-30 * scale.max(1e-300) {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                let apq = m[p * n + q];
                if apq.abs() < 1e-300 {
                    continue;
                }
                let theta = (m[q * n + q] - m[p * n + p]) / (2.0 * apq);
                let t = theta.signum().max(0.0) * 2.0 - 1.0;
                let t = t / (theta.abs() + (theta * theta + 1.0).sqrt());
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..n {
                    let (mkp, mkq) = (m[k * n + p], m[k * n + q]);
                    m[k * n + p] = c * mkp - s * mkq;
                    m[k * n + q] = s * mkp + c * mkq;
                }
                for k in 0..n {
                    let (mpk, mqk) = (m[p * n + k], m[q * n + k]);
                    m[p * n + k] = c * mpk - s * mqk;
                    m[q * n + k] = s * mpk + c * mqk;
                }
                for k in 0..n {
                    let (vkp, vkq) = (v[k * n + p], v[k * n + q]);
                    v[k * n + p] = c * vkp - s * vkq;
                    v[k * n + q] = s * vkp + c * vkq;
                }
            }
        }
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| m[a * n + a].total_cmp(&m[b * n + b]));
    let values = order.iter().map(|&k| m[k * n + k]).collect();
    let vectors = order
        .iter()
        .map(|&k| (0..n).map(|i| v[i * n + k]).collect())
        .collect();
    (values, vectors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rodrigues_round_trips_and_inverse_composes_to_identity() {
        for w in [
            [0.0, 0.0, 0.0],
            [0.1, -0.2, 0.3],
            [1.0, 2.0, -0.5],
            [0.0, 3.1, 0.0],
        ] {
            let r = rotation(w);
            let back = angle_axis(&r);
            let r2 = rotation(back);
            assert!(rotation_distance(&r, &r2) < 1e-9, "{w:?} → {back:?}");
            let i = mul3(&transpose3(&r), &r);
            assert!((i[0] - 1.0).abs() < 1e-12 && i[1].abs() < 1e-12);
        }
        let h = Homography([1.1, 0.05, 3.0, -0.02, 0.95, -2.0, 1e-4, -2e-4, 1.0]);
        let id = h.then(&h.inverse().unwrap());
        assert!(id.distance(&Homography::IDENTITY, 400, 300) < 1e-9);
        let s = Homography::similarity(1.02, 0.1, 1.0, 2.0, 50.0, 40.0);
        let (x, y) = s.apply(50.0, 40.0);
        assert!((x - 51.0).abs() < 1e-12 && (y - 42.0).abs() < 1e-12);
    }

    #[test]
    fn jacobi_eigen_of_a_known_matrix() {
        let a = [4.0, 1.0, 0.0, 1.0, 3.0, 0.0, 0.0, 0.0, 1.0];
        let (vals, vecs) = symmetric_eigen(&a, 3);
        assert!((vals[0] - 1.0).abs() < 1e-12);
        let want = [(7.0 - 5f64.sqrt()) / 2.0, (7.0 + 5f64.sqrt()) / 2.0];
        assert!((vals[1] - want[0]).abs() < 1e-12 && (vals[2] - want[1]).abs() < 1e-12);
        for (l, v) in vals.iter().zip(&vecs) {
            for i in 0..3 {
                let av: f64 = (0..3).map(|j| a[i * 3 + j] * v[j]).sum();
                assert!((av - l * v[i]).abs() < 1e-12);
            }
        }
    }
}
