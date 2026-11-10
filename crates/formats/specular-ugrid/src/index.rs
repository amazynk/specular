//! Finding the face under a point: a uniform grid over the display triangles, each cell
//! listing the triangles whose bounding box reaches it.

use crate::Display;

/// The triangle under a point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub face: u32,
    pub tri: u32,
    /// The triangle's mesh nodes.
    pub nodes: [u32; 3],
    /// Barycentric weights of the point on those nodes.
    pub weights: [f64; 3],
}

impl Hit {
    /// The node nearest the point among the triangle's.
    pub fn nearest_node(&self) -> u32 {
        let w = self.weights;
        let k = if w[0] >= w[1] && w[0] >= w[2] {
            0
        } else if w[1] >= w[2] {
            1
        } else {
            2
        };
        self.nodes[k]
    }
}

/// A uniform grid over the triangles of a [`Display`].
#[derive(Clone, Debug, Default)]
pub struct Locator {
    origin: [f64; 2],
    cell: [f64; 2],
    cols: usize,
    rows: usize,
    /// Cell `c` lists `items[starts[c]..starts[c + 1]]`.
    starts: Vec<u32>,
    items: Vec<u32>,
}

impl Locator {
    pub fn build(d: &Display) -> Locator {
        let n = d.tris.len();
        if n == 0 {
            return Locator::default();
        }
        let mut lo = [f64::MAX; 2];
        let mut hi = [f64::MIN; 2];
        for t in &d.tris {
            for &v in t {
                let p = d.positions[v as usize];
                lo = [lo[0].min(p[0]), lo[1].min(p[1])];
                hi = [hi[0].max(p[0]), hi[1].max(p[1])];
            }
        }
        let (w, h) = ((hi[0] - lo[0]).max(1e-12), (hi[1] - lo[1]).max(1e-12));
        // About one triangle per cell, up to 4M cells.
        let target = (n as f64).clamp(1.0, 4_194_304.0);
        let cols = ((target * w / h).sqrt().ceil() as usize).clamp(1, 8192);
        let rows = ((target / cols as f64).ceil() as usize).clamp(1, 8192);
        let cell = [w / cols as f64, h / rows as f64];
        let span = |t: &[u32; 3]| {
            let (mut a, mut b) = ([f64::MAX; 2], [f64::MIN; 2]);
            for &v in t {
                let p = d.positions[v as usize];
                a = [a[0].min(p[0]), a[1].min(p[1])];
                b = [b[0].max(p[0]), b[1].max(p[1])];
            }
            let c = |x: f64, o: f64, s: f64, m: usize| (((x - o) / s) as usize).min(m - 1);
            (
                c(a[0], lo[0], cell[0], cols),
                c(a[1], lo[1], cell[1], rows),
                c(b[0], lo[0], cell[0], cols),
                c(b[1], lo[1], cell[1], rows),
            )
        };
        let mut counts = vec![0u32; cols * rows + 1];
        for t in &d.tris {
            let (x0, y0, x1, y1) = span(t);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    counts[y * cols + x] += 1;
                }
            }
        }
        let mut starts = vec![0u32; cols * rows + 1];
        let mut acc = 0u32;
        for (s, c) in starts.iter_mut().zip(&counts) {
            *s = acc;
            acc = acc.saturating_add(*c);
        }
        starts[cols * rows] = acc;
        let mut fill = starts.clone();
        let mut items = vec![0u32; acc as usize];
        for (i, t) in d.tris.iter().enumerate() {
            let (x0, y0, x1, y1) = span(t);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let c = y * cols + x;
                    items[fill[c] as usize] = i as u32;
                    fill[c] += 1;
                }
            }
        }
        Locator {
            origin: lo,
            cell,
            cols,
            rows,
            starts,
            items,
        }
    }

    /// The triangle and face at (`x`, `y`) in display coordinates.
    pub fn locate(&self, d: &Display, x: f64, y: f64) -> Option<Hit> {
        if self.cols == 0 || !x.is_finite() || !y.is_finite() {
            return None;
        }
        let cx = (x - self.origin[0]) / self.cell[0];
        let cy = (y - self.origin[1]) / self.cell[1];
        if cx < 0.0 || cy < 0.0 || cx > self.cols as f64 || cy > self.rows as f64 {
            return None;
        }
        let c = (cy as usize).min(self.rows - 1) * self.cols + (cx as usize).min(self.cols - 1);
        let list = &self.items[self.starts[c] as usize..self.starts[c + 1] as usize];
        let mut best: Option<(f64, Hit)> = None;
        for &t in list {
            let tri = d.tris[t as usize];
            let [a, b, c] = tri.map(|v| d.positions[v as usize]);
            let det = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
            if det == 0.0 || !det.is_finite() {
                continue;
            }
            let w0 = ((b[1] - c[1]) * (x - c[0]) + (c[0] - b[0]) * (y - c[1])) / det;
            let w1 = ((c[1] - a[1]) * (x - c[0]) + (a[0] - c[0]) * (y - c[1])) / det;
            let w2 = 1.0 - w0 - w1;
            let inside = w0.min(w1).min(w2);
            let hit = Hit {
                face: d.tri_face[t as usize],
                tri: t,
                nodes: tri.map(|v| d.src[v as usize]),
                weights: [w0, w1, w2],
            };
            if inside >= 0.0 {
                return Some(hit);
            }
            // On an edge within rounding: keep the closest.
            if inside > -1e-9 && best.is_none_or(|(b, _)| inside > b) {
                best = Some((inside, hit));
            }
        }
        best.map(|(_, h)| h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Coords, Mesh};

    #[test]
    fn locates_against_brute_force() {
        // A 30 × 20 grid of quads, each split oddly, with a triangle fan polygon.
        let (nx, ny) = (31u32, 21u32);
        let mut nodes = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                let jitter = ((i * 7 + j * 13) % 5) as f64 * 0.05;
                nodes.push([f64::from(i) + jitter, f64::from(j) - jitter]);
            }
        }
        let mut faces: Vec<Vec<u32>> = Vec::new();
        for j in 0..ny - 1 {
            for i in 0..nx - 1 {
                let a = j * nx + i;
                if (i + j) % 3 == 0 {
                    faces.push(vec![a, a + 1, a + nx + 1]);
                    faces.push(vec![a, a + nx + 1, a + nx]);
                } else {
                    faces.push(vec![a, a + 1, a + nx + 1, a + nx]);
                }
            }
        }
        let refs: Vec<&[u32]> = faces.iter().map(Vec::as_slice).collect();
        let m = Mesh::from_parts(nodes, &refs, Coords::Projected { unit: None }).unwrap();
        let d = Display::build(&m);
        let loc = Locator::build(&d);
        let mut found = 0;
        for k in 0..2000 {
            let x = (k as f64 * 0.618_034).fract() * 31.0;
            let y = (k as f64 * 0.414_214).fract() * 21.0;
            let brute = d.tris.iter().position(|t| {
                let [a, b, c] = t.map(|v| d.positions[v as usize]);
                let s = |p: [f64; 2], q: [f64; 2]| {
                    (q[0] - p[0]) * (y - p[1]) - (q[1] - p[1]) * (x - p[0])
                };
                let (s0, s1, s2) = (s(a, b), s(b, c), s(c, a));
                (s0 >= 0.0 && s1 >= 0.0 && s2 >= 0.0) || (s0 <= 0.0 && s1 <= 0.0 && s2 <= 0.0)
            });
            let hit = loc.locate(&d, x, y);
            assert_eq!(
                brute.map(|t| d.tri_face[t]),
                hit.map(|h| h.face),
                "at {x} {y}"
            );
            found += usize::from(hit.is_some());
        }
        assert!(found > 1500);
    }
}
