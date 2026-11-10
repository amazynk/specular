//! Resampling a frame's mosaic into the reference frame's geometry without demosaicing it:
//! every output sample is interpolated from the frame's samples of the same colour.
//!
//! - **Bayer**: each of the four 2×2 positions is its own rectangular lattice with a 2-pixel
//!   pitch; Catmull–Rom on the lattice of the output sample's position (exact at even
//!   integer shifts).
//! - **Other patterns (X-Trans)**: normalised convolution with a tent of radius 2 over the
//!   same-colour samples around the position.
//! - **Linear and monochrome data**: Catmull–Rom per channel.
//!
//! A position whose taps leave the frame is NaN (unusable); one touching a saturated sample
//! (+∞) is +∞.
//!
//! The geometry is any back-map ([`Map`]): a bracket's rotation, a homography or a
//! panorama's projection. Where the map shrinks (one output sample covers more than one
//! source sample, as near a sphere's poles) the kernel widens by the local scale, which
//! low-passes the source before it is sampled (no aliasing).

use rayon::prelude::*;

use super::align::Transform;
use super::homography::Homography;
use super::{Mosaic, Plane};

/// An output position's source position.
pub trait Map: Sync {
    /// Where output sample (x, y) reads the source; `None`: nowhere (outside the map).
    fn src(&self, x: f64, y: f64) -> Option<(f64, f64)>;

    /// Source samples per output sample along the short axis at (x, y) (above 1 the map
    /// shrinks the source and the kernel widens).
    fn shrink(&self, x: f64, y: f64) -> f64 {
        let _ = (x, y);
        1.0
    }

    /// Whether the map leaves a `w × h` area as it is.
    fn is_identity(&self, w: u32, h: u32) -> bool {
        let _ = (w, h);
        false
    }
}

impl Map for Transform {
    fn src(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        Some(self.apply(x, y))
    }
    fn is_identity(&self, w: u32, h: u32) -> bool {
        Transform::is_identity(self, w, h)
    }
}

impl Map for Homography {
    fn src(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let h = &self.0;
        // Behind the camera.
        (h[6] * x + h[7] * y + h[8] > 0.0).then(|| self.apply(x, y))
    }
    fn shrink(&self, x: f64, y: f64) -> f64 {
        self.area_scale(x, y).sqrt()
    }
    fn is_identity(&self, w: u32, h: u32) -> bool {
        Homography::is_identity(self, w, h)
    }
}

#[inline]
fn catmull(t: f32) -> [f32; 4] {
    let t2 = t * t;
    let t3 = t2 * t;
    [
        0.5 * (-t3 + 2.0 * t2 - t),
        0.5 * (3.0 * t3 - 5.0 * t2 + 2.0),
        0.5 * (-3.0 * t3 + 4.0 * t2 + t),
        0.5 * (t3 - t2),
    ]
}

/// Catmull–Rom widened by `s` (≥ 1) at lattice position (i, j): the taps within 2s,
/// weights normalised.
#[allow(clippy::too_many_arguments)]
fn lattice_wide(p: &Plane, ox: i64, oy: i64, pitch: i64, i: f64, j: f64, c: usize, s: f64) -> f32 {
    let reach = (2.0 * s).ceil() as i64;
    let (ci, cj) = (i.floor() as i64, j.floor() as i64);
    let (w, h, cpp) = (i64::from(p.w), i64::from(p.h), p.cpp as usize);
    let (x_first, y_first) = (ox + pitch * (ci - reach + 1), oy + pitch * (cj - reach + 1));
    let (x_last, y_last) = (ox + pitch * (ci + reach), oy + pitch * (cj + reach));
    if x_first < 0 || y_first < 0 || x_last >= w || y_last >= h {
        return f32::NAN;
    }
    let k = |d: f64| -> f64 {
        let t = (d / s).abs();
        // Catmull–Rom as a kernel of distance.
        if t < 1.0 {
            1.5 * t * t * t - 2.5 * t * t + 1.0
        } else if t < 2.0 {
            -0.5 * t * t * t + 2.5 * t * t - 4.0 * t + 2.0
        } else {
            0.0
        }
    };
    let wx: Vec<f64> = (ci - reach + 1..=ci + reach)
        .map(|a| k(a as f64 - i))
        .collect();
    let wy: Vec<f64> = (cj - reach + 1..=cj + reach)
        .map(|b| k(b as f64 - j))
        .collect();
    let (mut sum, mut sw) = (0.0f64, 0.0f64);
    for (b, wyb) in wy.iter().enumerate() {
        let y = (y_first + pitch * b as i64) as usize;
        let row = &p.v[y * w as usize * cpp..];
        for (a, wxa) in wx.iter().enumerate() {
            let x = (x_first + pitch * a as i64) as usize;
            let v = row[x * cpp + c];
            if v == f32::INFINITY {
                return f32::INFINITY;
            }
            sum += wxa * wyb * f64::from(v);
            sw += wxa * wyb;
        }
    }
    if sw.abs() > 1e-12 {
        (sum / sw) as f32
    } else {
        f32::NAN
    }
}

/// Catmull–Rom at lattice position (i, j) of the lattice `(ox + pitch·a, oy + pitch·b)`,
/// channel `c`.
#[inline]
fn lattice(p: &Plane, ox: i64, oy: i64, pitch: i64, i: f64, j: f64, c: usize) -> f32 {
    let (i0, j0) = (i.floor(), j.floor());
    let wx = catmull((i - i0) as f32);
    let wy = catmull((j - j0) as f32);
    let (i0, j0) = (i0 as i64, j0 as i64);
    let (w, h, cpp) = (i64::from(p.w), i64::from(p.h), p.cpp as usize);
    let x_first = ox + pitch * (i0 - 1);
    let y_first = oy + pitch * (j0 - 1);
    if x_first < 0 || y_first < 0 || x_first + 3 * pitch >= w || y_first + 3 * pitch >= h {
        return f32::NAN;
    }
    let mut sum = 0.0f32;
    let mut saturated = false;
    for (b, wyb) in wy.iter().enumerate() {
        let y = (y_first + pitch * b as i64) as usize;
        let row = &p.v[y * w as usize * cpp..];
        let mut r = 0.0f32;
        for (a, wxa) in wx.iter().enumerate() {
            let x = (x_first + pitch * a as i64) as usize;
            let v = row[x * cpp + c];
            if v == f32::INFINITY {
                saturated = true;
            }
            r += wxa * v;
        }
        sum += wyb * r;
    }
    if saturated { f32::INFINITY } else { sum }
}

/// Normalised convolution over the same-colour samples near (px, py).
fn tent(p: &Plane, m: &Mosaic, color: u8, px: f64, py: f64) -> f32 {
    let (w, h) = (i64::from(p.w), i64::from(p.h));
    let (x0, y0) = (px.floor() as i64, py.floor() as i64);
    if x0 < 2 || y0 < 2 || x0 + 3 >= w || y0 + 3 >= h {
        return f32::NAN;
    }
    let mut sw = 0.0f32;
    let mut sv = 0.0f32;
    for y in y0 - 2..=y0 + 3 {
        let wy = (1.0 - (y as f64 - py).abs() / 2.0).max(0.0) as f32;
        if wy == 0.0 {
            continue;
        }
        for x in x0 - 2..=x0 + 3 {
            if m.color(x as u32, y as u32) != color {
                continue;
            }
            let wx = (1.0 - (x as f64 - px).abs() / 2.0).max(0.0) as f32;
            if wx == 0.0 {
                continue;
            }
            let v = p.v[y as usize * p.w as usize + x as usize];
            if v == f32::INFINITY {
                return f32::INFINITY;
            }
            sw += wx * wy;
            sv += wx * wy * v;
        }
    }
    if sw > 0.0 { sv / sw } else { f32::NAN }
}

/// The kernel widening of a shrink: none up to 1.05 (sharp sampling where the scale is
/// near 1), the shrink itself beyond.
#[inline]
fn widen(shrink: f64) -> f64 {
    if shrink > 1.05 { shrink.min(16.0) } else { 1.0 }
}

/// The value of plane `p` (mosaic `m`) for an output sample of colour `color` whose own
/// parity is (kx, ky), at source position (px, py), channel `c`, widened by `s`.
#[inline]
pub(crate) fn sample(
    p: &Plane,
    m: &Mosaic,
    (kx, ky): (u32, u32),
    color: u8,
    (px, py): (f64, f64),
    c: usize,
    s: f64,
) -> f32 {
    match m {
        Mosaic::Bayer(pat) => {
            // The source lattice of the same colour (same parity when it is that colour).
            let k = if pat[(ky * 2 + kx) as usize] == color {
                (ky * 2 + kx) as usize
            } else {
                pat.iter().position(|&q| q == color).unwrap_or(0)
            };
            let (ox, oy) = ((k & 1) as i64, (k >> 1) as i64);
            let i = (px - ox as f64) / 2.0;
            let j = (py - oy as f64) / 2.0;
            if s > 1.0 {
                lattice_wide(p, ox, oy, 2, i, j, 0, s)
            } else {
                lattice(p, ox, oy, 2, i, j, 0)
            }
        }
        Mosaic::Pattern { .. } => tent(p, m, color, px, py),
        Mosaic::Linear | Mosaic::Mono => {
            if s > 1.0 {
                lattice_wide(p, 0, 0, 1, px, py, c, s)
            } else {
                lattice(p, 0, 0, 1, px, py, c)
            }
        }
    }
}

/// `p` resampled so that sample (x, y) holds the frame's value at `t(x, y)`.
pub(crate) fn warp(p: &Plane, m: &Mosaic, t: &(impl Map + ?Sized)) -> Plane {
    if t.is_identity(p.w, p.h) {
        return p.clone();
    }
    let (w, cpp) = (p.w as usize, p.cpp as usize);
    let mut v = vec![f32::NAN; p.v.len()];
    v.par_chunks_mut(w * cpp).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (fx, fy) = (x as f64, y as f64);
            let Some(q) = t.src(fx, fy) else { continue };
            let s = widen(t.shrink(fx, fy));
            let color = m.color(x as u32, y as u32);
            let k = ((x & 1) as u32, (y & 1) as u32);
            for c in 0..cpp {
                row[x * cpp + c] = sample(p, m, k, color, q, c, s);
            }
        }
    });
    Plane {
        v,
        ..p.clone_empty()
    }
}
