//! Painting a field onto a pixel grid on the CPU: face values flat, node values
//! interpolated across each triangle; pixels no face covers are NaN.

use rayon::prelude::*;

use crate::{Display, Location};

/// Where a pixel grid lies in display coordinates: pixel (`i`, `j`)'s centre is at
/// `x0 + (i + 0.5) dx`, `y1 - (j + 0.5) dy` (rows run down from the top).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub x0: f64,
    pub y1: f64,
    pub dx: f64,
    pub dy: f64,
    pub width: usize,
    pub height: usize,
}

impl Frame {
    /// A `width` × `height` grid over `extent` (min x, min y, max x, max y).
    pub fn over(extent: [f64; 4], width: usize, height: usize) -> Frame {
        Frame {
            x0: extent[0],
            y1: extent[3],
            dx: (extent[2] - extent[0]) / width.max(1) as f64,
            dy: (extent[3] - extent[1]) / height.max(1) as f64,
            width,
            height,
        }
    }

    /// Display coordinates to continuous pixel coordinates ((0, 0) the top-left corner).
    pub fn to_pixel(&self, x: f64, y: f64) -> (f64, f64) {
        ((x - self.x0) / self.dx, (self.y1 - y) / self.dy)
    }

    /// Continuous pixel coordinates to display coordinates.
    pub fn to_display(&self, px: f64, py: f64) -> (f64, f64) {
        (self.x0 + px * self.dx, self.y1 - py * self.dy)
    }
}

const BAND: usize = 32;

/// Paints `values` (one per face or node) over `frame`.
pub fn raster(d: &Display, values: &[f32], location: Location, frame: &Frame) -> Vec<f32> {
    let (w, h) = (frame.width, frame.height);
    let mut out = vec![f32::NAN; w * h];
    if w == 0 || h == 0 || d.tris.is_empty() {
        return out;
    }
    let px: Vec<[f32; 2]> = d
        .positions
        .iter()
        .map(|p| {
            let (x, y) = frame.to_pixel(p[0], p[1]);
            [x as f32, y as f32]
        })
        .collect();
    let bands = h.div_ceil(BAND);
    // Triangles per band of rows (compressed).
    let range = |t: &[u32; 3]| -> Option<(usize, usize)> {
        let ys = t.map(|v| px[v as usize][1]);
        let lo = ys[0].min(ys[1]).min(ys[2]) - 0.5;
        let hi = ys[0].max(ys[1]).max(ys[2]) - 0.5;
        if !(hi >= 0.0 && lo < h as f32) {
            return None;
        }
        let a = (lo.max(0.0) as usize) / BAND;
        let b = ((hi.ceil() as usize).min(h - 1)) / BAND;
        Some((a, b))
    };
    let mut starts = vec![0u32; bands + 1];
    for t in &d.tris {
        if let Some((a, b)) = range(t) {
            for s in &mut starts[a..=b] {
                *s += 1;
            }
        }
    }
    let mut acc = 0u32;
    for s in &mut starts {
        let c = *s;
        *s = acc;
        acc += c;
    }
    let mut fill = starts.clone();
    let mut items = vec![0u32; acc as usize];
    for (i, t) in d.tris.iter().enumerate() {
        if let Some((a, b)) = range(t) {
            for f in &mut fill[a..=b] {
                items[*f as usize] = i as u32;
                *f += 1;
            }
        }
    }
    out.par_chunks_mut(BAND * w)
        .enumerate()
        .for_each(|(band, rows)| {
            let y0 = band * BAND;
            let nrows = rows.len() / w;
            for &t in &items[starts[band] as usize..starts[band + 1] as usize] {
                let tri = d.tris[t as usize];
                let [a, b, c] = tri.map(|v| px[v as usize]);
                let vals: [f32; 3] = match location {
                    Location::Face => {
                        let v = values
                            .get(d.tri_face[t as usize] as usize)
                            .copied()
                            .unwrap_or(f32::NAN);
                        [v; 3]
                    }
                    Location::Node => tri.map(|v| {
                        values
                            .get(d.src[v as usize] as usize)
                            .copied()
                            .unwrap_or(f32::NAN)
                    }),
                };
                let det = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
                if det == 0.0 || !det.is_finite() {
                    continue;
                }
                let inv = 1.0 / det;
                let xlo = (a[0].min(b[0]).min(c[0]) - 0.5).max(0.0).ceil() as usize;
                let xhi = (a[0].max(b[0]).max(c[0]) - 0.5).min(w as f32 - 1.0);
                if xhi < 0.0 {
                    continue;
                }
                let xhi = xhi.floor() as usize;
                let ylo = (a[1].min(b[1]).min(c[1]) - 0.5).max(y0 as f32).ceil() as usize;
                let yhi = (a[1].max(b[1]).max(c[1]) - 0.5).min((y0 + nrows) as f32 - 1.0);
                if yhi < y0 as f32 {
                    continue;
                }
                let yhi = yhi.floor() as usize;
                for j in ylo..=yhi {
                    let y = j as f32 + 0.5;
                    let row = &mut rows[(j - y0) * w..(j - y0 + 1) * w];
                    for (i, o) in row.iter_mut().enumerate().take(xhi + 1).skip(xlo) {
                        let x = i as f32 + 0.5;
                        let w0 = ((b[1] - c[1]) * (x - c[0]) + (c[0] - b[0]) * (y - c[1])) * inv;
                        let w1 = ((c[1] - a[1]) * (x - c[0]) + (a[0] - c[0]) * (y - c[1])) * inv;
                        let w2 = 1.0 - w0 - w1;
                        if w0 >= 0.0 && w1 >= 0.0 && w2 >= -1e-6 {
                            *o = w0 * vals[0] + w1 * vals[1] + w2 * vals[2];
                        }
                    }
                }
            }
        });
    out
}

/// The mesh edges that touch `visible` (continuous pixel coordinates of `frame`: min x, min
/// y, max x, max y) as pixel segments (x0, y0, x1, y1); `None` when more than `max` do (a
/// wireframe that dense would fill the view).
pub fn segments_in(
    d: &Display,
    frame: &Frame,
    visible: [f64; 4],
    max: usize,
) -> Option<Vec<[f32; 4]>> {
    let px = |v: u32| {
        let p = d.positions[v as usize];
        frame.to_pixel(p[0], p[1])
    };
    let [vx0, vy0, vx1, vy1] = visible;
    let picked: Vec<[f32; 4]> = d
        .edges
        .par_iter()
        .filter_map(|&[a, b]| {
            let (ax, ay) = px(a);
            let (bx, by) = px(b);
            let out = ax.max(bx) < vx0 || ax.min(bx) > vx1 || ay.max(by) < vy0 || ay.min(by) > vy1;
            (!out).then_some([ax as f32, ay as f32, bx as f32, by as f32])
        })
        .collect();
    (picked.len() <= max).then_some(picked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Coords, Mesh};

    #[test]
    fn flat_and_interpolated() {
        let nodes = vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]];
        let m =
            Mesh::from_parts(nodes, &[&[0, 1, 2, 3]], Coords::Projected { unit: None }).unwrap();
        let d = Display::build(&m);
        let f = Frame::over(d.extent, 4, 4);
        let v = raster(&d, &[7.0], Location::Face, &f);
        assert!(v.iter().all(|&x| x == 7.0), "{v:?}");
        // Node values linear in x: every pixel takes its centre's x.
        let v = raster(&d, &[0.0, 4.0, 4.0, 0.0], Location::Node, &f);
        for j in 0..4 {
            for i in 0..4 {
                assert!((v[j * 4 + i] - (i as f32 + 0.5)).abs() < 1e-4, "{v:?}");
            }
        }
        // Outside the mesh: NaN.
        let f = Frame::over([0.0, 0.0, 8.0, 4.0], 8, 4);
        let v = raster(&d, &[1.0], Location::Face, &f);
        assert!(v[7].is_nan() && v[0] == 1.0);
        // The wireframe: a quad's four sides and its fan diagonal is not an edge.
        let s = segments_in(&d, &f, [0.0, 0.0, 8.0, 4.0], 10).unwrap();
        assert_eq!(s.len(), 4);
        assert!(segments_in(&d, &f, [0.0, 0.0, 8.0, 4.0], 3).is_none());
        assert_eq!(
            segments_in(&d, &f, [6.0, 0.0, 8.0, 4.0], 10).unwrap().len(),
            0
        );
    }
}
