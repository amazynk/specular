//! "Remove (AI)" patches: what the Remove feature's inpainting network made of
//! the picture around a removal spot, kept per photo and spot.
//!
//! - A **patch** is linear Rec.2020 RGB over a rectangle of the source frame (fractions),
//!   so it follows crop, straighten and lens changes like the spot.
//! - A removal spot with its patch renders as a **heal whose source is the patch**: the
//!   patch plus the harmonic correction that takes the surroundings' tones at the spot's
//!   edge (Poisson), mixed in by the spot's feather in linear light. The network's
//!   8-bit look and any tone mismatch at the edge vanish into the correction, so the edge
//!   shows no seam. Without its patch (the model is not installed, or not run yet) the spot
//!   heals from its automatic source.
//! - Patches are found by [`key`] (photo, model, the spot's stroke and size) in [`store`],
//!   which a loader set by the app fills from its disk cache; the sidecar keeps only the
//!   stroke, the model's id and the blend, so another computer computes its own.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use super::Spot;

/// Words of all patches one retouch module's table may hold (above, patches are left
/// out and their spots heal from their source).
pub const MAX_PATCH_WORDS: usize = 12 << 20;
/// Bytes of patches the store keeps in memory.
const STORE_BYTES: u64 = 384 << 20;

/// An inpainted area: `w × h` linear Rec.2020 RGB texels over `rect` (x0, y0, x1, y1,
/// fractions of the frame).
#[derive(Clone, Debug, PartialEq)]
pub struct Patch {
    pub rect: [f32; 4],
    pub w: u32,
    pub h: u32,
    pub rgb: Vec<f32>,
}

impl Patch {
    pub fn bytes(&self) -> u64 {
        4 * self.rgb.len() as u64
    }

    /// Bilinear RGB at texel coordinates (`u`, `v`) (texel `i` centred at `i + ½`),
    /// clamped to the edge: what the kernel reads (`rt_patch`).
    pub fn sample(&self, u: f32, v: f32) -> [f32; 3] {
        let gx = (u - 0.5).clamp(0.0, (self.w - 1) as f32);
        let gy = (v - 0.5).clamp(0.0, (self.h - 1) as f32);
        let (x0, y0) = (gx.floor() as u32, gy.floor() as u32);
        let (x1, y1) = ((x0 + 1).min(self.w - 1), (y0 + 1).min(self.h - 1));
        let (tx, ty) = (gx - x0 as f32, gy - y0 as f32);
        let t = |x: u32, y: u32, c: usize| self.rgb[3 * (y * self.w + x) as usize + c];
        std::array::from_fn(|c| {
            let top = t(x0, y0, c) + (t(x1, y0, c) - t(x0, y0, c)) * tx;
            let bot = t(x0, y1, c) + (t(x1, y1, c) - t(x0, y1, c)) * tx;
            top + (bot - top) * ty
        })
    }
}

/// The key of `spot`'s patch on the photo `photo` ([`crate::mask::depth::photo`]): the
/// model, the stroke (centre, size and brush path) — not the feather or opacity, which
/// apply when rendering.
pub fn key(photo: &[u8; 32], spot: &Spot) -> [u8; 32] {
    let mut h = blake3::Hasher::new_derive_key("specular-develop 2026-11-01 remove patch v1");
    h.update(photo);
    h.update(spot.model.as_bytes());
    h.update(&[0]);
    for v in spot
        .target
        .iter()
        .chain(std::iter::once(&spot.size))
        .chain(spot.path.iter().flatten())
    {
        h.update(&v.to_bits().to_le_bytes());
    }
    *h.finalize().as_bytes()
}

/// Reads a patch missing from memory (the app's disk cache).
pub type Loader = Box<dyn Fn(&[u8; 32]) -> Option<Patch> + Send>;

/// Computed patches by key, bounded in memory, with an optional loader.
pub struct Store {
    items: HashMap<[u8; 32], (Arc<Patch>, u64)>,
    tick: u64,
    loader: Option<Loader>,
    /// Bytes kept in memory.
    bound: u64,
}

impl Default for Store {
    fn default() -> Store {
        Store {
            items: HashMap::new(),
            tick: 0,
            loader: None,
            bound: STORE_BYTES,
        }
    }
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store")
            .field("patches", &self.items.len())
            .finish_non_exhaustive()
    }
}

impl Store {
    pub fn put(&mut self, key: [u8; 32], p: Patch) -> Arc<Patch> {
        self.tick += 1;
        let a = Arc::new(p);
        self.items.insert(key, (a.clone(), self.tick));
        while self.bytes() > self.bound && self.items.len() > 1 {
            let oldest = self
                .items
                .iter()
                .filter(|(k, _)| **k != key)
                .min_by_key(|(_, (_, t))| *t)
                .map(|(k, _)| *k);
            match oldest {
                Some(k) => {
                    self.items.remove(&k);
                }
                None => break,
            }
        }
        a
    }

    pub fn get(&mut self, key: &[u8; 32]) -> Option<Arc<Patch>> {
        self.tick += 1;
        let t = self.tick;
        if let Some((p, used)) = self.items.get_mut(key) {
            *used = t;
            return Some(p.clone());
        }
        let p = self.loader.as_ref().and_then(|l| l(key))?;
        Some(self.put(*key, p))
    }

    pub fn contains(&self, key: &[u8; 32]) -> bool {
        self.items.contains_key(key)
    }

    pub fn remove(&mut self, key: &[u8; 32]) {
        self.items.remove(key);
    }

    pub fn set_loader(&mut self, l: Option<Loader>) {
        self.loader = l;
    }

    pub fn bytes(&self) -> u64 {
        self.items.values().map(|(p, _)| p.bytes()).sum()
    }
}

/// The process's patches.
pub fn store() -> &'static Mutex<Store> {
    static S: OnceLock<Mutex<Store>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Store::default()))
}

/// A patch for the disk cache: a small header, then f16 texels (linear light; f16 keeps
/// highlights and shadows a byte cache would band).
pub fn encode(p: &Patch) -> Vec<u8> {
    let mut b = Vec::with_capacity(32 + 2 * p.rgb.len());
    b.extend_from_slice(b"SPRP1\0\0\0");
    for v in p.rect {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&p.w.to_le_bytes());
    b.extend_from_slice(&p.h.to_le_bytes());
    for v in &p.rgb {
        b.extend_from_slice(&half::f16::from_f32(*v).to_le_bytes());
    }
    b
}

/// [`encode`]'s inverse; None for anything malformed.
pub fn decode(b: &[u8]) -> Option<Patch> {
    if b.get(..8)? != b"SPRP1\0\0\0" {
        return None;
    }
    let f = |i: usize| Some(f32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?));
    let u = |i: usize| Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?));
    let rect = [f(8)?, f(12)?, f(16)?, f(20)?];
    let (w, h) = (u(24)?, u(28)?);
    let n = 3usize.checked_mul(w as usize)?.checked_mul(h as usize)?;
    if w == 0 || h == 0 || n > MAX_PATCH_WORDS || b.len() != 32 + 2 * n {
        return None;
    }
    let rgb = b[32..]
        .chunks_exact(2)
        .map(|c| half::f16::from_le_bytes([c[0], c[1]]).to_f32())
        .collect();
    Some(Patch { rect, w, h, rgb })
}

/// The map from picture pixels (`x`, `y`, pixel centres at `i + ½`) to the patch's texel
/// coordinates: `u = a·x + b·y + c`, `v = d·x + e·y + f`, fitted through three corners of
/// its rectangle placed on the picture (`corner(fx, fy)` maps a frame point to picture
/// pixels). None when the corners do not span an area.
pub fn affine(p: &Patch, corner: &dyn Fn([f32; 2]) -> Option<[f64; 2]>) -> Option<[f32; 6]> {
    let [x0, y0, x1, y1] = p.rect;
    let o = corner([x0, y0])?;
    let a = corner([x1, y0])?;
    let b = corner([x0, y1])?;
    // Picture = o + U·(a − o)/w + V·(b − o)/h; invert the 2×2.
    let (w, h) = (f64::from(p.w), f64::from(p.h));
    let m = [
        (a[0] - o[0]) / w,
        (b[0] - o[0]) / h,
        (a[1] - o[1]) / w,
        (b[1] - o[1]) / h,
    ];
    let det = m[0] * m[3] - m[1] * m[2];
    if det.abs() < 1e-12 || !det.is_finite() {
        return None;
    }
    let inv = [m[3] / det, -m[1] / det, -m[2] / det, m[0] / det];
    Some([
        inv[0] as f32,
        inv[1] as f32,
        (-(inv[0] * o[0] + inv[1] * o[1])) as f32,
        inv[2] as f32,
        inv[3] as f32,
        (-(inv[2] * o[0] + inv[3] * o[1])) as f32,
    ])
}

// --- Making a patch (the app runs the network between these) -----------------------------

/// The picture rectangle (pixels: x0, y0, x1, y1) a removal's network sees: the stroke's box
/// (`points` in picture pixels, brush radius `r` px) with as much context again around it,
/// square where the picture allows, clipped to the `w × h` picture.
pub fn crop_for(points: &[(f64, f64)], r: f64, (w, h): (u32, u32)) -> [f64; 4] {
    let (w, h) = (f64::from(w.max(1)), f64::from(h.max(1)));
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for &(x, y) in points {
        b = [
            b[0].min(x - r),
            b[1].min(y - r),
            b[2].max(x + r),
            b[3].max(y + r),
        ];
    }
    if b[0] > b[2] {
        return [0.0, 0.0, w, h];
    }
    let side = ((b[2] - b[0]).max(b[3] - b[1]) * 2.2 + 16.0).max(64.0);
    let (cx, cy) = ((b[0] + b[2]) * 0.5, (b[1] + b[3]) * 0.5);
    let place = |c: f64, len: f64, max: f64| {
        let len = len.min(max);
        let lo = (c - len * 0.5).clamp(0.0, max - len);
        [lo, lo + len]
    };
    let [x0, x1] = place(cx, side, w);
    let [y0, y1] = place(cy, side, h);
    [x0.floor(), y0.floor(), x1.ceil().min(w), y1.ceil().min(h)]
}

/// The crop `crop` (picture pixels) of an RGBA8 picture `w × h`, area-averaged to `nw × nh`
/// RGB in 0..1 (the network's input).
pub fn crop_rgb(
    rgba8: &[u8],
    (w, h): (u32, u32),
    crop: [f64; 4],
    (nw, nh): (usize, usize),
) -> Vec<f32> {
    let (w, h) = (w as usize, h as usize);
    let sx = (crop[2] - crop[0]) / nw as f64;
    let sy = (crop[3] - crop[1]) / nh as f64;
    let mut out = Vec::with_capacity(3 * nw * nh);
    for j in 0..nh {
        let (y0, y1) = (crop[1] + j as f64 * sy, crop[1] + (j + 1) as f64 * sy);
        for i in 0..nw {
            let (x0, x1) = (crop[0] + i as f64 * sx, crop[0] + (i + 1) as f64 * sx);
            let mut acc = [0f64; 3];
            let mut ws = 0f64;
            let (ya, yb) = (
                y0.floor() as usize,
                (y1.ceil() as usize).max(y0.floor() as usize + 1),
            );
            let (xa, xb) = (
                x0.floor() as usize,
                (x1.ceil() as usize).max(x0.floor() as usize + 1),
            );
            for y in ya..yb.min(h) {
                let wy = (y1.min(y as f64 + 1.0) - y0.max(y as f64))
                    .max(0.0)
                    .max(1e-6);
                for x in xa..xb.min(w) {
                    let wx = (x1.min(x as f64 + 1.0) - x0.max(x as f64))
                        .max(0.0)
                        .max(1e-6);
                    let k = 4 * (y * w + x);
                    for c in 0..3 {
                        acc[c] += wx * wy * f64::from(rgba8[k + c]);
                    }
                    ws += wx * wy;
                }
            }
            for a in acc {
                out.push((a / ws.max(1e-12) / 255.0) as f32);
            }
        }
    }
    out
}

/// The hole the network fills, `nw × nh` over `crop` (picture pixels): 1 within `r + grow`
/// picture pixels of the stroke (`points`, a capsule chain), else 0.
pub fn hole(
    points: &[(f64, f64)],
    r: f64,
    grow: f64,
    crop: [f64; 4],
    (nw, nh): (usize, usize),
) -> Vec<f32> {
    let sx = (crop[2] - crop[0]) / nw as f64;
    let sy = (crop[3] - crop[1]) / nh as f64;
    let rr = r + grow;
    let mut out = vec![0f32; nw * nh];
    for j in 0..nh {
        let py = crop[1] + (j as f64 + 0.5) * sy;
        for i in 0..nw {
            let px = crop[0] + (i as f64 + 0.5) * sx;
            let d = if points.len() == 1 {
                (px - points[0].0).hypot(py - points[0].1)
            } else {
                points
                    .windows(2)
                    .map(|s| {
                        let (a, b) = (s[0], s[1]);
                        let (ex, ey) = (b.0 - a.0, b.1 - a.1);
                        let l2 = ex * ex + ey * ey;
                        let t = if l2 > 0.0 {
                            (((px - a.0) * ex + (py - a.1) * ey) / l2).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        (px - a.0 - ex * t).hypot(py - a.1 - ey * t)
                    })
                    .fold(f64::MAX, f64::min)
            };
            if d <= rr {
                out[j * nw + i] = 1.0;
            }
        }
    }
    out
}

/// Rec.709/sRGB to Rec.2020 primaries (linear), D65: the inverse of the networks' picture
/// transform ([`crate::mask::ai::Picture::rgba8`]).
const TO_2020: [[f32; 3]; 3] = [
    [0.627_404, 0.329_283, 0.043_313],
    [0.069_097, 0.919_540, 0.011_362],
    [0.016_391, 0.088_013, 0.895_595],
];

fn srgb_inv(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.040_45 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// A display-referred RGB value of the networks' picture back in linear Rec.2020 (its
/// exposure `gain` taken out).
pub fn to_linear(rgb: [f32; 3], gain: f32) -> [f32; 3] {
    let l = rgb.map(srgb_inv);
    TO_2020.map(|row| (row[0] * l[0] + row[1] * l[1] + row[2] * l[2]) / gain.max(1e-6))
}

/// The patch from the network's output `out` (display-referred RGB 0..1, `nw × nh` over
/// the picture rectangle `crop`, pixels of a `w × h` picture): over the frame rectangle
/// the crop covers, at the network's resolution, in linear Rec.2020. `to_frame` maps
/// picture fractions to frame fractions, `to_picture` the other way.
pub fn from_network(
    out: &[f32],
    (nw, nh): (usize, usize),
    crop: [f64; 4],
    (w, h): (u32, u32),
    to_frame: &dyn Fn(f64, f64) -> (f64, f64),
    to_picture: &dyn Fn(f64, f64) -> (f64, f64),
    gain: f32,
) -> Patch {
    let (w, h) = (f64::from(w.max(1)), f64::from(h.max(1)));
    let corners = [
        to_frame(crop[0] / w, crop[1] / h),
        to_frame(crop[2] / w, crop[1] / h),
        to_frame(crop[0] / w, crop[3] / h),
        to_frame(crop[2] / w, crop[3] / h),
    ];
    let mut r = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for (x, y) in corners {
        r = [r[0].min(x), r[1].min(y), r[2].max(x), r[3].max(y)];
    }
    // A quarter turn swaps the network's axes in the frame.
    let (ax, ay) = (corners[1].0 - corners[0].0, corners[1].1 - corners[0].1);
    let (pw, ph) = if ax.abs() >= ay.abs() {
        (nw, nh)
    } else {
        (nh, nw)
    };
    let sample = |gx: f64, gy: f64| -> [f32; 3] {
        let gx = gx.clamp(0.0, (nw - 1) as f64);
        let gy = gy.clamp(0.0, (nh - 1) as f64);
        let (x0, y0) = (gx.floor() as usize, gy.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(nw - 1), (y0 + 1).min(nh - 1));
        let (tx, ty) = ((gx - x0 as f64) as f32, (gy - y0 as f64) as f32);
        let t = |x: usize, y: usize, c: usize| out[3 * (y * nw + x) + c];
        std::array::from_fn(|c| {
            let top = t(x0, y0, c) + (t(x1, y0, c) - t(x0, y0, c)) * tx;
            let bot = t(x0, y1, c) + (t(x1, y1, c) - t(x0, y1, c)) * tx;
            top + (bot - top) * ty
        })
    };
    let mut rgb = Vec::with_capacity(3 * pw * ph);
    for j in 0..ph {
        let fy = r[1] + (j as f64 + 0.5) / ph as f64 * (r[3] - r[1]);
        for i in 0..pw {
            let fx = r[0] + (i as f64 + 0.5) / pw as f64 * (r[2] - r[0]);
            let (u, v) = to_picture(fx, fy);
            let gx = (u * w - crop[0]) / (crop[2] - crop[0]) * nw as f64 - 0.5;
            let gy = (v * h - crop[1]) / (crop[3] - crop[1]) * nh as f64 - 0.5;
            rgb.extend_from_slice(&to_linear(sample(gx, gy), gain));
        }
    }
    Patch {
        rect: [r[0] as f32, r[1] as f32, r[2] as f32, r[3] as f32],
        w: pw as u32,
        h: ph as u32,
        rgb,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patch(w: u32, h: u32) -> Patch {
        Patch {
            rect: [0.25, 0.5, 0.75, 1.0],
            w,
            h,
            rgb: (0..3 * w * h).map(|i| i as f32 / 100.0).collect(),
        }
    }

    #[test]
    fn disk_form_round_trips_in_half_precision() {
        let p = patch(5, 3);
        let q = decode(&encode(&p)).unwrap();
        assert_eq!((q.w, q.h, q.rect), (p.w, p.h, p.rect));
        for (a, b) in p.rgb.iter().zip(&q.rgb) {
            assert!((a - b).abs() <= a.abs() * 1e-3 + 1e-6);
        }
        assert!(decode(&encode(&p)[..40]).is_none());
        assert!(decode(b"nonsense").is_none());
    }

    #[test]
    fn keys_ignore_feather_and_opacity() {
        let s = Spot {
            model: "big-lama".into(),
            ..Spot::default()
        };
        let k = key(&[1; 32], &s);
        let soft = Spot {
            feather: 0.9,
            opacity: 0.3,
            ..s.clone()
        };
        assert_eq!(k, key(&[1; 32], &soft));
        let moved = Spot {
            target: [0.4, 0.5],
            ..s.clone()
        };
        assert_ne!(k, key(&[1; 32], &moved));
        assert_ne!(k, key(&[2; 32], &s));
    }

    #[test]
    fn affine_maps_corners_to_texels() {
        let p = patch(40, 20);
        // The frame is 1000 × 500 picture pixels, unwarped.
        let corner = |f: [f32; 2]| Some([f64::from(f[0]) * 1000.0, f64::from(f[1]) * 500.0]);
        let m = affine(&p, &corner).unwrap();
        let at = |x: f32, y: f32| [m[0] * x + m[1] * y + m[2], m[3] * x + m[4] * y + m[5]];
        let a = at(250.0, 250.0);
        assert!(a[0].abs() < 1e-3 && a[1].abs() < 1e-3);
        let b = at(750.0, 500.0);
        assert!((b[0] - 40.0).abs() < 1e-3 && (b[1] - 20.0).abs() < 1e-3);
        // Sampling at a texel's centre reads it.
        let s = p.sample(2.5, 1.5);
        assert!((s[1] - p.rgb[3 * (40 + 2) + 1]).abs() < 1e-6);
    }

    #[test]
    fn linear_round_trip_through_the_display_look() {
        // The networks' look: Rec.2020 → 709, gain, sRGB; to_linear undoes it.
        let c = [0.12f32, 0.2, 0.05];
        let k = 1.7;
        let to709 = [
            [1.660_491f32, -0.587_641_1, -0.072_849_9],
            [-0.124_550_5, 1.132_899_9, -0.008_349_4],
            [-0.018_150_8, -0.100_578_9, 1.118_729_7],
        ];
        let srgb = |v: f32| {
            if v <= 0.003_130_8 {
                12.92 * v
            } else {
                1.055 * v.powf(1.0 / 2.4) - 0.055
            }
        };
        let d = to709.map(|r| srgb((r[0] * c[0] + r[1] * c[1] + r[2] * c[2]) * k));
        let back = to_linear(d, k);
        for (a, b) in back.iter().zip(c) {
            assert!((a - b).abs() < 2e-4, "{back:?} vs {c:?}");
        }
    }

    #[test]
    fn crop_hole_and_patch_line_up() {
        // A 400 × 300 picture that is the frame itself; a stroke at (100, 150), r 10.
        let (w, h) = (400u32, 300u32);
        let crop = crop_for(&[(100.0, 150.0)], 10.0, (w, h));
        assert!(crop[0] <= 90.0 && crop[2] >= 110.0 && crop[2] - crop[0] >= 64.0);
        let n = (64, 64);
        let m = hole(&[(100.0, 150.0)], 10.0, 2.0, crop, n);
        let inside = m.iter().filter(|v| **v > 0.5).count() as f64;
        let px = (crop[2] - crop[0]) / 64.0;
        let want = std::f64::consts::PI * (12.0 / px).powi(2);
        assert!((inside - want).abs() < 0.15 * want, "{inside} vs {want}");
        // A picture with a horizontal ramp; the crop and back give the ramp in the frame.
        let rgba: Vec<u8> = (0..w * h)
            .flat_map(|i| {
                let v = ((i % w) * 255 / (w - 1)) as u8;
                [v, v, v, 255]
            })
            .collect();
        let rgb = crop_rgb(&rgba, (w, h), crop, n);
        let id = |u: f64, v: f64| (u, v);
        let p = from_network(&rgb, n, crop, (w, h), &id, &id, 1.0);
        assert_eq!((p.w, p.h), (64, 64));
        assert!((p.rect[0] - (crop[0] / 400.0) as f32).abs() < 1e-6);
        // Brightness grows to the right in the patch as in the picture.
        let g = |i: u32| p.rgb[3 * (32 * 64 + i) as usize + 1];
        assert!(g(60) > g(4));
    }

    #[test]
    fn store_drops_the_oldest_past_its_bound() {
        let mut s = Store {
            bound: 3 * 4 * 64 * 64 + 100,
            ..Store::default()
        };
        let big = |v: f32| Patch {
            rect: [0.0, 0.0, 1.0, 1.0],
            w: 64,
            h: 64,
            rgb: vec![v; 3 * 64 * 64],
        };
        s.put([1; 32], big(1.0));
        s.put([2; 32], big(2.0));
        assert!(s.bytes() <= s.bound);
        assert!(!s.contains(&[1; 32]) && s.contains(&[2; 32]));
    }
}
