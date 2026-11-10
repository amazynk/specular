//! A panorama's output projection and frame.
//!
//! The world is the straightened one of [`super::bundle`]: x right, y down, z forward.
//! An output position `(X, Y)` (pixels, `s` pixels per radian at the centre) is a world
//! direction:
//!
//! - **Rectilinear**: `(X/s, Y/s, 1)`, straight lines stay straight; good up to about 110°
//!   across.
//! - **Cylindrical**: longitude `θ = X/s`, height `Y/s`: `(sin θ, Y/s, cos θ)`, for wide
//!   rows of frames.
//! - **Equirectangular**: longitude and latitude, `(sin θ cos φ, sin φ, cos θ cos φ)`, the
//!   whole sphere (360° panoramas).
//!
//! The scale is the frames' median focal length, so the centre of the panorama keeps the
//! frames' own resolution. The frame is the bounding box of every frame's border carried
//! into the projection.

use super::bundle::Camera;

/// How the sphere of directions is laid flat.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Projection {
    #[default]
    Rectilinear,
    Cylindrical,
    Equirectangular,
}

/// Up to this horizontal field of view a rectilinear projection is chosen.
pub const RECTILINEAR_MAX_DEG: f64 = 110.0;

impl Projection {
    /// The direction at projected position (u, v) in radians-units.
    #[inline]
    pub fn dir(self, u: f64, v: f64) -> [f64; 3] {
        match self {
            Projection::Rectilinear => [u, v, 1.0],
            Projection::Cylindrical => {
                let (s, c) = u.sin_cos();
                [s, v, c]
            }
            Projection::Equirectangular => {
                let (s, c) = u.sin_cos();
                let (sp, cp) = v.sin_cos();
                [s * cp, sp, c * cp]
            }
        }
    }

    /// The projected position of direction `d`; `None` where the projection cannot show it.
    #[inline]
    pub fn pos(self, d: [f64; 3]) -> Option<(f64, f64)> {
        match self {
            Projection::Rectilinear => (d[2] > 1e-6).then(|| (d[0] / d[2], d[1] / d[2])),
            Projection::Cylindrical => {
                let r = d[0].hypot(d[2]);
                (r > 1e-9).then(|| (d[0].atan2(d[2]), d[1] / r))
            }
            Projection::Equirectangular => {
                let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                (n > 1e-12).then(|| (d[0].atan2(d[2]), (d[1] / n).clamp(-1.0, 1.0).asin()))
            }
        }
    }
}

/// The output raster: projection, scale and the position of its top-left sample.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutputFrame {
    pub projection: Projection,
    /// Pixels per radian at the centre.
    pub scale: f64,
    /// Projected position of sample (0, 0), in pixels.
    pub x0: f64,
    pub y0: f64,
    pub w: u32,
    pub h: u32,
}

impl OutputFrame {
    #[inline]
    pub fn dir(&self, x: f64, y: f64) -> [f64; 3] {
        self.projection
            .dir((x + self.x0) / self.scale, (y + self.y0) / self.scale)
    }

    #[inline]
    pub fn pos(&self, d: [f64; 3]) -> Option<(f64, f64)> {
        self.projection
            .pos(d)
            .map(|(u, v)| (u * self.scale - self.x0, v * self.scale - self.y0))
    }

    /// The same frame at `k` times the resolution (`k < 1` for a proxy).
    pub fn scaled(&self, k: f64) -> OutputFrame {
        OutputFrame {
            scale: self.scale * k,
            x0: self.x0 * k,
            y0: self.y0 * k,
            w: ((f64::from(self.w) * k).round() as u32).max(1),
            h: ((f64::from(self.h) * k).round() as u32).max(1),
            ..*self
        }
    }

    /// A sub-rectangle of this frame.
    pub fn crop(&self, x: u32, y: u32, w: u32, h: u32) -> OutputFrame {
        OutputFrame {
            x0: self.x0 + f64::from(x),
            y0: self.y0 + f64::from(y),
            w,
            h,
            ..*self
        }
    }
}

/// Points along a `w × h` frame's border, `n` per side.
fn border(w: u32, h: u32, n: usize) -> impl Iterator<Item = (f64, f64)> {
    let (w, h) = (f64::from(w), f64::from(h));
    (0..n).flat_map(move |i| {
        let t = i as f64 / n as f64;
        [(t * w, 0.0), (w, t * h), (w - t * w, h), (0.0, h - t * h)]
    })
}

/// The projected bounding box (u0, v0, u1, v1, radians-units) of `cams` on `sizes` frames.
pub fn bounds(p: Projection, cams: &[(Camera, (u32, u32))]) -> Option<(f64, f64, f64, f64)> {
    let mut b = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for (c, (w, h)) in cams {
        for (x, y) in border(*w, *h, 32).chain([(f64::from(*w) / 2.0, f64::from(*h) / 2.0)]) {
            let Some((u, v)) = p.pos(c.unproject(x, y)) else {
                // A rectilinear view cannot hold a frame looking sideways.
                return None;
            };
            b = (b.0.min(u), b.1.min(v), b.2.max(u), b.3.max(v));
        }
    }
    b.0.is_finite().then_some(b)
}

/// The projection a panorama of `cams` gets: rectilinear up to [`RECTILINEAR_MAX_DEG`]
/// across (and as tall), cylindrical beyond.
pub fn choose(cams: &[(Camera, (u32, u32))]) -> Projection {
    let limit = (RECTILINEAR_MAX_DEG / 2.0).to_radians().tan() * 2.0;
    match bounds(Projection::Rectilinear, cams) {
        Some(b) if b.2 - b.0 <= limit && b.3 - b.1 <= limit => Projection::Rectilinear,
        _ => Projection::Cylindrical,
    }
}

/// The output frame of `cams` in `p` at the median focal length (`max_side` caps the long
/// side, for a proxy).
pub fn frame(
    p: Projection,
    cams: &[(Camera, (u32, u32))],
    max_side: Option<u32>,
) -> Option<OutputFrame> {
    let mut fs: Vec<f64> = cams.iter().map(|c| c.0.f).collect();
    fs.sort_by(f64::total_cmp);
    let mut scale = *fs.get(fs.len() / 2)?;
    let (u0, v0, mut u1, v1) = bounds(p, cams)?;
    if p != Projection::Rectilinear {
        // A full turn at most.
        u1 = u1.min(u0 + std::f64::consts::TAU);
    }
    let (wr, hr) = (u1 - u0, v1 - v0);
    if let Some(m) = max_side {
        scale = scale.min(f64::from(m) / wr.max(hr).max(1e-9));
    }
    let (w, h) = ((wr * scale).ceil(), (hr * scale).ceil());
    // Beyond what a raster can hold (a rectilinear frame looking nearly sideways).
    if !(w >= 1.0 && h >= 1.0 && w <= 1e6 && h <= 1e6) {
        return None;
    }
    Some(OutputFrame {
        projection: p,
        scale,
        x0: (u0 * scale).floor(),
        y0: (v0 * scale).floor(),
        w: w as u32 + 1,
        h: h as u32 + 1,
    })
}

/// The largest axis-aligned rectangle of `true` cells in a `w × h` mask: (x, y, w, h).
pub fn largest_rectangle(mask: &[bool], w: usize, h: usize) -> (usize, usize, usize, usize) {
    let mut heights = vec![0usize; w];
    let mut best = (0, 0, 0, 0);
    let mut stack: Vec<usize> = Vec::with_capacity(w + 1);
    for y in 0..h {
        for x in 0..w {
            heights[x] = if mask[y * w + x] { heights[x] + 1 } else { 0 };
        }
        // Largest rectangle under the histogram.
        stack.clear();
        for x in 0..=w {
            let cur = if x < w { heights[x] } else { 0 };
            while let Some(&top) = stack.last() {
                if heights[top] <= cur {
                    break;
                }
                stack.pop();
                let ht = heights[top];
                let left = stack.last().map_or(0, |&s| s + 1);
                let width = x - left;
                if width * ht > best.2 * best.3 {
                    best = (left, y + 1 - ht, width, ht);
                }
            }
            stack.push(x);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projections_round_trip() {
        for p in [
            Projection::Rectilinear,
            Projection::Cylindrical,
            Projection::Equirectangular,
        ] {
            for (u, v) in [(0.0, 0.0), (0.4, -0.2), (-0.9, 0.3)] {
                let (a, b) = p.pos(p.dir(u, v)).expect("visible");
                assert!((a - u).abs() < 1e-12 && (b - v).abs() < 1e-12, "{p:?}");
            }
        }
    }

    #[test]
    fn largest_rectangle_of_a_mask() {
        let (w, h) = (8, 6);
        let mut m = vec![false; w * h];
        for y in 1..5 {
            for x in 2..7 {
                m[y * w + x] = true;
            }
        }
        m[w + 2] = false;
        m[8] = true;
        assert_eq!(largest_rectangle(&m, w, h), (3, 1, 4, 4));
    }
}
