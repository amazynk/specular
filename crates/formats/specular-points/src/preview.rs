//! A CPU renderer for thumbnails: the cloud from the mesh view's isometric direction (or
//! from above, for flat clouds such as aerial surveys), fitted, as depth-tested square
//! splats coloured by RGB (or by elevation through Batlow), with depth edges darkened,
//! supersampled 2×2 on a transparent background.

use specular_inspect::ColourMap;
use specular_mesh::preview::ISO;

use crate::{Bounds, PointCloud};

/// Points drawn at most.
const MAX_DRAWN: usize = 2_000_000;

/// The view of a cloud: right, down, forward. Flat clouds (height under a tenth of their
/// width) are seen from above, north up.
fn basis(b: &Bounds) -> [[f32; 3]; 3] {
    let s = b.size();
    if s[2] < 0.1 * s[0].max(s[1]) {
        [[1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]]
    } else {
        ISO
    }
}

/// Right, down and forward of a point (relative to the box's centre) in the preview's view.
fn projector(b: &Bounds) -> impl Fn([f32; 3]) -> [f32; 3] {
    let [right, down, fwd] = basis(b);
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let centre = b.centre();
    move |p: [f32; 3]| {
        let q = [p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]];
        [dot(q, right), dot(q, down), dot(q, fwd)]
    }
}

/// The box's corners on the preview's screen: low and high right and down.
fn footprint(b: &Bounds) -> ([f32; 2], [f32; 2]) {
    let proj = projector(b);
    let (mut lo, mut hi) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
    let (bmin, bmax) = (b.min, b.max);
    for i in 0..8 {
        let p = [0, 1, 2].map(|k| if i >> k & 1 == 0 { bmin[k] } else { bmax[k] });
        let q = proj(p);
        for k in 0..2 {
            lo[k] = lo[k].min(q[k]);
            hi[k] = hi[k].max(q[k]);
        }
    }
    (lo, hi)
}

/// Width over height of the preview's view of the cloud, between 1:4 and 4:1.
pub fn aspect(b: &Bounds) -> f32 {
    let (lo, hi) = footprint(b);
    let (w, h) = (hi[0] - lo[0], hi[1] - lo[1]);
    if !(w.is_finite() && h.is_finite()) || w <= 0.0 && h <= 0.0 {
        return 1.0;
    }
    (w / h.max(1e-30)).clamp(0.25, 4.0)
}

/// Renders `cloud` to straight sRGB RGBA8 of `width × height`.
pub fn render(cloud: &PointCloud, width: u32, height: u32) -> Vec<u8> {
    let (w, h) = (width.max(1) as usize, height.max(1) as usize);
    let ss = 2usize;
    let (sw, sh) = (w * ss, h * ss);
    let mut out = vec![0u8; w * h * 4];
    if cloud.is_empty() {
        return out;
    }
    // An even subsample: a prefix of every (shuffled) leaf.
    let frac = (MAX_DRAWN as f64 / cloud.len() as f64).min(1.0);
    let mut idx: Vec<u32> = Vec::with_capacity(cloud.len().min(MAX_DRAWN + cloud.leaves.len()));
    for l in &cloud.leaves {
        let k = ((f64::from(l.count) * frac).ceil() as u32).min(l.count);
        idx.extend(l.first..l.first + k);
    }
    let proj = projector(&cloud.bounds);
    let (bmin, bmax) = (cloud.bounds.min, cloud.bounds.max);
    let (lo, hi) = footprint(&cloud.bounds);
    let ext = [(hi[0] - lo[0]).max(1e-20), (hi[1] - lo[1]).max(1e-20)];
    let scale = 0.94 * (sw as f32 / ext[0]).min(sh as f32 / ext[1]);
    let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    // Splats sized so the drawn points about cover the cloud's footprint.
    let area = ext[0] * ext[1] * scale * scale;
    // At least one output pixel per 512 of the long side, up to four times that for sparse
    // clouds: fixed 1–3 pixel splats nearly vanished once a big rendering was fitted,
    // exported or thumbnailed (a 50-point XYZ cloud; the visual review).
    let min_side = ss as f32 * (w.max(h) as f32 / 512.0).max(1.0);
    let max_side = (4.0 * min_side).max(6.0);
    let side = ((area / idx.len() as f32).sqrt() * 0.9).clamp(min_side.min(2.0), max_side) as i32;
    let rgb = cloud.info.channels.rgb;
    let (z0, dz) = (bmin[2], (bmax[2] - bmin[2]).max(1e-20));
    let mut depth = vec![f32::INFINITY; sw * sh];
    let mut colour = vec![[0u8; 3]; sw * sh];
    for &i in &idx {
        let p = cloud.positions[i as usize];
        let q = proj(p);
        let x = ((q[0] - mid[0]) * scale + sw as f32 / 2.0) as i32 - side / 2;
        let y = ((q[1] - mid[1]) * scale + sh as f32 / 2.0) as i32 - side / 2;
        let a = cloud.attrs[i as usize][0];
        let c = if rgb {
            [a as u8, (a >> 8) as u8, (a >> 16) as u8]
        } else {
            ColourMap::Batlow
                .sample((p[2] - z0) / dz)
                .map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8)
        };
        for yy in y.max(0)..(y + side).min(sh as i32) {
            for xx in x.max(0)..(x + side).min(sw as i32) {
                let o = yy as usize * sw + xx as usize;
                if q[2] < depth[o] {
                    depth[o] = q[2];
                    colour[o] = c;
                }
            }
        }
    }
    // Depth edges: darken where a neighbour is much nearer (a cheap eye-dome look).
    let unit = ext[0].max(ext[1]) / 200.0;
    let shade = |x: usize, y: usize| -> f32 {
        let d = depth[y * sw + x];
        let mut s = 0.0f32;
        for (dx, dy) in [(-2i32, 0i32), (2, 0), (0, -2), (0, 2)] {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if nx < 0 || ny < 0 || nx >= sw as i32 || ny >= sh as i32 {
                continue;
            }
            let n = depth[ny as usize * sw + nx as usize];
            if n.is_finite() {
                s += ((d - n) / unit).max(0.0);
            }
        }
        (-0.6 * s).exp()
    };
    for y in 0..h {
        for x in 0..w {
            let (mut acc, mut n) = ([0f32; 3], 0u32);
            for sy in 0..ss {
                for sx in 0..ss {
                    let (px, py) = (x * ss + sx, y * ss + sy);
                    let o = py * sw + px;
                    if depth[o].is_finite() {
                        let k = shade(px, py);
                        for c in 0..3 {
                            acc[c] += f32::from(colour[o][c]) * k;
                        }
                        n += 1;
                    }
                }
            }
            if n > 0 {
                let o = (y * w + x) * 4;
                for c in 0..3 {
                    out[o + c] = (acc[c] / n as f32).round() as u8;
                }
                out[o + 3] = (n * 255 / (ss * ss) as u32) as u8;
            }
        }
    }
    out
}
