//! Geometry the IFC reader builds in: `f64` vectors, affine maps and indexed triangle shapes
//! (closed solids where the source is one), with their volume, area, bounds, and normals
//! smoothed within a crease angle.

use std::collections::HashMap;

pub(crate) type P3 = [f64; 3];
pub(crate) type P2 = [f64; 2];

pub(crate) fn add(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub(crate) fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(crate) fn scale(a: P3, s: f64) -> P3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

pub(crate) fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(crate) fn cross(a: P3, b: P3) -> P3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(crate) fn len(a: P3) -> f64 {
    dot(a, a).sqrt()
}

/// `a` scaled to unit length, `None` when it has none.
pub(crate) fn unit(a: P3) -> Option<P3> {
    let l = len(a);
    (l > 1e-12 && l.is_finite()).then(|| scale(a, 1.0 / l))
}

/// An affine map: rows of `[x y z t]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Mat(pub [[f64; 4]; 3]);

impl Mat {
    pub const IDENTITY: Mat = Mat([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ]);

    /// The map with these axes (columns) and origin.
    pub fn from_axes(x: P3, y: P3, z: P3, o: P3) -> Mat {
        Mat([
            [x[0], y[0], z[0], o[0]],
            [x[1], y[1], z[1], o[1]],
            [x[2], y[2], z[2], o[2]],
        ])
    }

    pub fn translation(t: P3) -> Mat {
        let mut m = Mat::IDENTITY;
        for (r, v) in m.0.iter_mut().zip(t) {
            r[3] = v;
        }
        m
    }

    /// `self ∘ o`: `o` first, then `self`.
    pub fn mul(&self, o: &Mat) -> Mat {
        let a = &self.0;
        let b = &o.0;
        let mut r = [[0f64; 4]; 3];
        for (i, row) in r.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
                if j == 3 {
                    *v += a[i][3];
                }
            }
        }
        Mat(r)
    }

    pub fn point(&self, p: P3) -> P3 {
        let m = &self.0;
        [0, 1, 2].map(|i| m[i][0] * p[0] + m[i][1] * p[1] + m[i][2] * p[2] + m[i][3])
    }

    pub fn vector(&self, p: P3) -> P3 {
        let m = &self.0;
        [0, 1, 2].map(|i| m[i][0] * p[0] + m[i][1] * p[1] + m[i][2] * p[2])
    }

    pub fn origin(&self) -> P3 {
        [self.0[0][3], self.0[1][3], self.0[2][3]]
    }

    pub fn det(&self) -> f64 {
        let m = &self.0;
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    }

    /// The inverse, `None` when singular.
    pub fn inverse(&self) -> Option<Mat> {
        let d = self.det();
        if d.abs() < 1e-300 || !d.is_finite() {
            return None;
        }
        let m = &self.0;
        let c = |r0: usize, c0: usize, r1: usize, c1: usize| {
            m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0]
        };
        let inv3 = [
            [c(1, 1, 2, 2) / d, -c(0, 1, 2, 2) / d, c(0, 1, 1, 2) / d],
            [-c(1, 0, 2, 2) / d, c(0, 0, 2, 2) / d, -c(0, 0, 1, 2) / d],
            [c(1, 0, 2, 1) / d, -c(0, 0, 2, 1) / d, c(0, 0, 1, 1) / d],
        ];
        let t = self.origin();
        let mut r = [[0f64; 4]; 3];
        for i in 0..3 {
            for j in 0..3 {
                r[i][j] = inv3[i][j];
            }
            r[i][3] = -(inv3[i][0] * t[0] + inv3[i][1] * t[1] + inv3[i][2] * t[2]);
        }
        Some(Mat(r))
    }
}

/// An indexed triangle shape.
#[derive(Clone, Debug, Default)]
pub(crate) struct Shape {
    pub pos: Vec<P3>,
    pub tri: Vec<[u32; 3]>,
    /// sRGB colour per triangle, when the source gives colours face by face.
    pub colors: Option<Vec<[u8; 4]>>,
    /// The shape bounds a volume (a closed shell); booleans need it.
    pub closed: bool,
}

impl Shape {
    pub fn is_empty(&self) -> bool {
        self.tri.is_empty()
    }

    pub fn vertex(&mut self, p: P3) -> u32 {
        self.pos.push(p);
        (self.pos.len() - 1) as u32
    }

    /// Appends `o` (both keep their triangles; colours padded white).
    pub fn append(&mut self, o: &Shape) {
        let base = self.pos.len() as u32;
        let had = self.tri.len();
        self.pos.extend_from_slice(&o.pos);
        self.tri.extend(
            o.tri
                .iter()
                .map(|t| [t[0] + base, t[1] + base, t[2] + base]),
        );
        match (&mut self.colors, &o.colors) {
            (Some(c), Some(oc)) => c.extend_from_slice(oc),
            (Some(c), None) => c.resize(self.tri.len(), [255; 4]),
            (None, Some(oc)) => {
                let mut c = vec![[255u8; 4]; had];
                c.extend_from_slice(oc);
                self.colors = Some(c);
            }
            (None, None) => {}
        }
        self.closed = self.closed && o.closed;
    }

    pub fn transform(&mut self, m: &Mat) {
        for p in &mut self.pos {
            *p = m.point(*p);
        }
        if m.det() < 0.0 {
            self.flip();
        }
    }

    pub fn flip(&mut self) {
        for t in &mut self.tri {
            t.swap(1, 2);
        }
    }

    /// Signed volume (positive for an outward-facing closed shell).
    pub fn volume(&self) -> f64 {
        let o = self.pos.first().copied().unwrap_or([0.0; 3]);
        self.tri
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| sub(self.pos[i as usize], o));
                dot(a, cross(b, c)) / 6.0
            })
            .sum()
    }

    pub fn area(&self) -> f64 {
        self.tri
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| self.pos[i as usize]);
                len(cross(sub(b, a), sub(c, a))) / 2.0
            })
            .sum()
    }

    /// Turns a closed shell outward (by the sign of its volume).
    pub fn orient_outward(&mut self) {
        if self.volume() < 0.0 {
            self.flip();
        }
    }

    pub fn bounds(&self) -> Option<(P3, P3)> {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for t in &self.tri {
            for &i in t {
                let p = self.pos[i as usize];
                for c in 0..3 {
                    lo[c] = lo[c].min(p[c]);
                    hi[c] = hi[c].max(p[c]);
                }
            }
        }
        (lo[0] <= hi[0]).then_some((lo, hi))
    }

    /// Whether every coordinate is finite.
    pub fn is_finite(&self) -> bool {
        self.pos.iter().flatten().all(|v| v.is_finite())
    }
}

/// Unit normals per triangle corner: each corner averages (by area) the triangles at its
/// position whose normals are within `crease` radians of its own triangle's, so curved
/// surfaces shade smooth and edges stay sharp.
pub(crate) fn crease_normals(s: &Shape, crease: f64) -> Vec<[[f32; 3]; 3]> {
    let cos = crease.cos();
    let face: Vec<P3> = s
        .tri
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| s.pos[i as usize]);
            cross(sub(b, a), sub(c, a))
        })
        .collect();
    let unit_face: Vec<P3> = face.iter().map(|&n| unit(n).unwrap_or([0.0; 3])).collect();
    // Corners by position.
    let key = |p: P3| p.map(|v| v.to_bits());
    let mut at: HashMap<[u64; 3], Vec<u32>> = HashMap::with_capacity(s.pos.len());
    for (t, tri) in s.tri.iter().enumerate() {
        for &i in tri {
            at.entry(key(s.pos[i as usize])).or_default().push(t as u32);
        }
    }
    s.tri
        .iter()
        .enumerate()
        .map(|(t, tri)| {
            let own = unit_face[t];
            tri.map(|i| {
                let mut n = [0.0; 3];
                if let Some(list) = at.get(&key(s.pos[i as usize])) {
                    for &o in list {
                        let o = o as usize;
                        if o == t || dot(unit_face[o], own) >= cos {
                            n = add(n, face[o]);
                        }
                    }
                }
                let n = unit(n).unwrap_or(own);
                [n[0] as f32, n[1] as f32, n[2] as f32]
            })
        })
        .collect()
}

/// The Newell normal of a polygon (not normalised; its length is twice the area).
pub(crate) fn newell(p: &[P3]) -> P3 {
    let mut n = [0.0; 3];
    for k in 0..p.len() {
        let (a, b) = (p[k], p[(k + 1) % p.len()]);
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    n
}

/// Two unit axes spanning the plane with normal `n` (`u × v = n`).
pub(crate) fn plane_basis(n: P3) -> (P3, P3) {
    let n = unit(n).unwrap_or([0.0, 0.0, 1.0]);
    let a = if n[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let u = unit(cross(a, n)).unwrap_or([1.0, 0.0, 0.0]);
    let v = cross(n, u);
    (u, v)
}

/// Signed area of a 2D ring (positive counter-clockwise).
pub(crate) fn ring_area(r: &[P2]) -> f64 {
    let mut a = 0.0;
    for k in 0..r.len() {
        let (p, q) = (r[k], r[(k + 1) % r.len()]);
        a += p[0] * q[1] - q[0] * p[1];
    }
    a / 2.0
}

/// Whether `p` is inside ring `r` (even-odd).
pub(crate) fn in_ring(p: P2, r: &[P2]) -> bool {
    let mut inside = false;
    let n = r.len();
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let (a, b) = (r[i], r[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0];
            if p[0] < x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// Drops consecutive duplicate points (and a closing point equal to the first).
pub(crate) fn dedup_ring<T: Copy + PartialEq + Into<P3Like>>(r: &mut Vec<T>, eps: f64) {
    r.dedup_by(|a, b| (*a).into().dist(&(*b).into()) <= eps);
    while r.len() > 1 && r[0].into().dist(&r[r.len() - 1].into()) <= eps {
        r.pop();
    }
}

/// A point of either dimension, for [`dedup_ring`].
pub(crate) struct P3Like(P3);

impl P3Like {
    fn dist(&self, o: &P3Like) -> f64 {
        len(sub(self.0, o.0))
    }
}

impl From<P3> for P3Like {
    fn from(p: P3) -> Self {
        P3Like(p)
    }
}

impl From<P2> for P3Like {
    fn from(p: P2) -> Self {
        P3Like([p[0], p[1], 0.0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrices() {
        let m = Mat::from_axes(
            [0.0, 1.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 2.0, 3.0],
        );
        let p = m.point([1.0, 0.0, 0.0]);
        assert_eq!(p, [1.0, 3.0, 3.0]);
        let inv = m.inverse().unwrap();
        let q = inv.point(p);
        assert!(len(sub(q, [1.0, 0.0, 0.0])) < 1e-12);
        let mm = m.mul(&inv);
        assert!(len(sub(mm.point([5.0, 6.0, 7.0]), [5.0, 6.0, 7.0])) < 1e-12);
    }

    #[test]
    fn rings() {
        let sq = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        assert_eq!(ring_area(&sq), 1.0);
        assert!(in_ring([0.5, 0.5], &sq));
        assert!(!in_ring([1.5, 0.5], &sq));
        let mut r = vec![[0.0, 0.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0]];
        dedup_ring(&mut r, 1e-9);
        assert_eq!(r.len(), 3);
    }
}
