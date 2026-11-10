//! AI masks: subject, sky and object-at-click rows.
//!
//! A row ([`AiMask`]) says what to select and with which model; the networks are the app's
//! (`specular_ai::masks`). What they find becomes a **raster over the source frame**, in
//! the same space as a brush raster, so it follows every geometry change without being
//! computed again and combines with every other row (union, intersect, subtract) and the
//! refine pass.
//!
//! - **Picture** ([`Picture`]): the whole frame upright (the file's orientation, no crop or
//!   lens correction), as Develop sees it before exposure, at the preview's size. The
//!   networks get it tone-mapped to 8 bits ([`Picture::rgba8`]).
//! - **Guided upsampling** ([`guided_upsample`]): the network's probabilities (256² to
//!   1024²) brought to the picture's size by a guided filter with the picture's luma as
//!   guide, so edges follow the photo; then resampled into the frame raster
//!   ([`frame_raster`]). At render time the mask's edges are refined once more at the
//!   render's own resolution (an implicit edge refinement, [`EDGE_RADIUS`]), so an export
//!   at full size keeps them on the photo's edges.
//! - **Keys** ([`key`]): a computed mask is found by a key from the photo, the row's kind,
//!   clicks, box and model. The app keys an edit's rows for its photo ([`rekey`]); the
//!   sidecar does not store keys, so another computer keys and computes its own.
//! - **Store** ([`store`]): computed rasters by key, shared by the preview and exports,
//!   bounded in memory; a loader set by the app reads the disk cache on a miss
//!   ([`encode`] / [`decode`]).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use super::Geometry;
use super::brush::Raster;
use crate::model::Edit;
use crate::model::mask::{AiKind, AiMask, MaskExpr};

/// Version of the key derivation and of the raster a key names.
const KEY_CONTEXT: &str = "specular-develop 2026-11-01 ai mask v1";
/// Longest side of a mask raster (texels): finer adds nothing the render's own edge
/// refinement does not.
pub const RASTER_LONG: u32 = 4096;
/// The implicit edge refinement of a mask with an AI row, as a fraction of the long side
/// (used when the row's own edge refinement is off).
pub const EDGE_RADIUS: f32 = 0.004;
/// Bytes of rasters the store keeps in memory.
const STORE_BYTES: u64 = 512 << 20;

/// The key of `m`'s computed mask on the photo `photo` (any stable identity of the file:
/// the app uses its content fingerprint).
pub fn key(photo: &[u8; 32], m: &AiMask) -> [u8; 32] {
    let mut h = blake3::Hasher::new_derive_key(KEY_CONTEXT);
    h.update(photo);
    h.update(crate::model::fields::Choice::name(m.kind).as_bytes());
    h.update(&[0]);
    h.update(m.model.as_bytes());
    h.update(&[0]);
    // Clicks and the box only matter to object masks.
    if m.kind == AiKind::Object {
        for p in &m.points {
            h.update(&p.x.to_le_bytes());
            h.update(&p.y.to_le_bytes());
            h.update(&[u8::from(p.positive)]);
        }
        if let Some(r) = &m.rect {
            h.update(b"box");
            for v in r {
                h.update(&v.to_le_bytes());
            }
        }
    }
    *h.finalize().as_bytes()
}

fn walk_mut(e: &mut MaskExpr, f: &mut dyn FnMut(&mut AiMask)) {
    match e {
        MaskExpr::Ai(m) => f(m),
        MaskExpr::Invert(a) | MaskExpr::Opacity(_, a) => walk_mut(a, f),
        MaskExpr::Combine(_, a, b) => {
            walk_mut(a, f);
            walk_mut(b, f);
        }
        _ => {}
    }
}

fn walk(e: &MaskExpr, f: &mut dyn FnMut(&AiMask)) {
    match e {
        MaskExpr::Ai(m) => f(m),
        MaskExpr::Invert(a) | MaskExpr::Opacity(_, a) => walk(a, f),
        MaskExpr::Combine(_, a, b) => {
            walk(a, f);
            walk(b, f);
        }
        _ => {}
    }
}

/// A photo's identity for [`key`] from what the app knows of the file: its path, size and
/// modification time (nanoseconds since 1970).
pub fn photo_id(path: &[u8], len: u64, mtime_ns: u128) -> [u8; 32] {
    let mut h = blake3::Hasher::new_derive_key("specular-develop 2026-11-01 ai photo");
    h.update(path);
    h.update(&len.to_le_bytes());
    h.update(&mtime_ns.to_le_bytes());
    *h.finalize().as_bytes()
}

/// Keys one row for `photo`.
pub fn rekey_one(m: &mut AiMask, photo: &[u8; 32]) {
    m.key = key(photo, m);
}

/// Keys every AI row of `edit` for `photo`; true when any key changed.
pub fn rekey(edit: &mut Edit, photo: &[u8; 32]) -> bool {
    let mut changed = false;
    for m in &mut edit.modules {
        if let Some(b) = &mut m.blend {
            walk_mut(&mut b.mask, &mut |a| {
                let k = key(photo, a);
                if a.key != k {
                    a.key = k;
                    changed = true;
                }
            });
        }
    }
    changed
}

/// The AI rows of `edit`'s enabled modules, once per key, in module order.
pub fn rows(edit: &Edit) -> Vec<AiMask> {
    let mut out: Vec<AiMask> = Vec::new();
    for m in edit.modules.iter().filter(|m| m.enabled) {
        if let Some(b) = &m.blend {
            walk(&b.mask, &mut |a| {
                if !out.iter().any(|o| o.key == a.key) {
                    out.push(a.clone());
                }
            });
        }
    }
    out
}

/// Whether a mask expression has an AI row.
pub fn has_ai(e: &MaskExpr) -> bool {
    let mut any = false;
    walk(e, &mut |_| any = true);
    any
}

type Loader = Box<dyn Fn(&[u8; 32]) -> Option<Raster> + Send + Sync>;

/// Computed AI rasters by key (see the module docs).
#[derive(Default)]
pub struct Store {
    map: HashMap<[u8; 32], (Arc<Raster>, u64)>,
    bytes: u64,
    tick: u64,
    loader: Option<Loader>,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store")
            .field("rasters", &self.map.len())
            .field("bytes", &self.bytes)
            .finish_non_exhaustive()
    }
}

impl Store {
    /// Keeps `r` for `key` (the oldest unused rasters go past the memory bound).
    pub fn put(&mut self, key: [u8; 32], r: Raster) -> Arc<Raster> {
        self.tick += 1;
        let r = Arc::new(r);
        if let Some((old, _)) = self.map.insert(key, (r.clone(), self.tick)) {
            self.bytes -= old.bytes();
        }
        self.bytes += r.bytes();
        while self.bytes > STORE_BYTES && self.map.len() > 1 {
            let Some((&k, _)) = self
                .map
                .iter()
                .filter(|(k, _)| **k != key)
                .min_by_key(|(_, (_, t))| *t)
            else {
                break;
            };
            if let Some((old, _)) = self.map.remove(&k) {
                self.bytes -= old.bytes();
            }
        }
        r
    }

    /// The raster for `key`: kept, or read by the loader.
    pub fn get(&mut self, key: &[u8; 32]) -> Option<Arc<Raster>> {
        self.tick += 1;
        if let Some((r, t)) = self.map.get_mut(key) {
            *t = self.tick;
            return Some(r.clone());
        }
        let r = self.loader.as_ref().and_then(|l| l(key))?;
        Some(self.put(*key, r))
    }

    /// Kept in memory (no loader).
    pub fn contains(&self, key: &[u8; 32]) -> bool {
        self.map.contains_key(key)
    }

    pub fn remove(&mut self, key: &[u8; 32]) {
        if let Some((old, _)) = self.map.remove(key) {
            self.bytes -= old.bytes();
        }
    }

    /// What reads a raster missing from memory (the app's disk cache).
    pub fn set_loader(&mut self, l: Option<Loader>) {
        self.loader = l;
    }

    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}

/// The process's AI rasters.
pub fn store() -> &'static Mutex<Store> {
    static S: OnceLock<Mutex<Store>> = OnceLock::new();
    S.get_or_init(Mutex::default)
}

/// The picture the networks see: the whole frame upright, linear Rec.2020 RGBA.
#[derive(Clone, Debug)]
pub struct Picture {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<f32>,
}

/// Linear to display: an exposure that puts the median luminance near mid grey, then the
/// sRGB curve (a plain look; the networks need a normal-looking photo, not this edit's).
fn exposure(p: &Picture) -> f32 {
    let mut lum: Vec<f32> = p
        .rgba
        .chunks_exact(4)
        .step_by(7)
        .map(|c| 0.2627 * c[0] + 0.678 * c[1] + 0.0593 * c[2])
        .filter(|v| v.is_finite() && *v > 0.0)
        .collect();
    if lum.is_empty() {
        return 1.0;
    }
    let mid = lum.len() / 2;
    let (_, m, _) = lum.select_nth_unstable_by(mid, f32::total_cmp);
    (0.18 / m.max(1e-6)).clamp(1.0 / 64.0, 64.0)
}

fn srgb(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.003_130_8 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// Rec.2020 to Rec.709/sRGB primaries (linear), D65.
const TO_709: [[f32; 3]; 3] = [
    [1.660_491, -0.587_641_1, -0.072_849_9],
    [-0.124_550_5, 1.132_899_9, -0.008_349_4],
    [-0.018_150_8, -0.100_578_9, 1.118_729_7],
];

impl Picture {
    /// The exposure the networks' look applies (AI remove takes it out of a removal patch).
    pub fn gain(&self) -> f32 {
        exposure(self)
    }

    /// 8-bit sRGB RGBA for the networks.
    pub fn rgba8(&self) -> Vec<u8> {
        let k = exposure(self);
        let mut out = Vec::with_capacity(self.rgba.len());
        for c in self.rgba.chunks_exact(4) {
            for row in &TO_709 {
                let v = (row[0] * c[0] + row[1] * c[1] + row[2] * c[2]) * k;
                out.push((srgb(v) * 255.0 + 0.5) as u8);
            }
            out.push(255);
        }
        out
    }

    /// The guide: display luma 0..1 (the same look as [`Picture::rgba8`]).
    pub fn luma(&self) -> Vec<f32> {
        let k = exposure(self);
        self.rgba
            .chunks_exact(4)
            .map(|c| srgb((0.2627 * c[0] + 0.678 * c[1] + 0.0593 * c[2]) * k))
            .collect()
    }
}

/// Box mean of radius `r` over a `w × h` plane (edge-clamped windows), by summed areas.
fn box_mean(v: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    let mut sat = vec![0f64; (w + 1) * (h + 1)];
    for y in 0..h {
        let mut row = 0f64;
        for x in 0..w {
            row += f64::from(v[y * w + x]);
            sat[(y + 1) * (w + 1) + x + 1] = sat[y * (w + 1) + x + 1] + row;
        }
    }
    let mut out = vec![0f32; w * h];
    for y in 0..h {
        let (y0, y1) = (y.saturating_sub(r), (y + r + 1).min(h));
        for x in 0..w {
            let (x0, x1) = (x.saturating_sub(r), (x + r + 1).min(w));
            let s = sat[y1 * (w + 1) + x1] - sat[y0 * (w + 1) + x1] - sat[y1 * (w + 1) + x0]
                + sat[y0 * (w + 1) + x0];
            out[y * w + x] = (s / ((y1 - y0) * (x1 - x0)) as f64) as f32;
        }
    }
    out
}

/// Bilinear (half-pixel centres, edge-clamped) resample of a `sw × sh` plane to `dw × dh`.
pub fn resample(src: &[f32], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<f32> {
    let mut out = vec![0f32; dw * dh];
    let (kx, ky) = (sw as f32 / dw as f32, sh as f32 / dh as f32);
    for y in 0..dh {
        let fy = ((y as f32 + 0.5) * ky - 0.5).clamp(0.0, (sh - 1) as f32);
        let (y0, ty) = (fy.floor() as usize, fy.fract());
        let y1 = (y0 + 1).min(sh - 1);
        for x in 0..dw {
            let fx = ((x as f32 + 0.5) * kx - 0.5).clamp(0.0, (sw - 1) as f32);
            let (x0, tx) = (fx.floor() as usize, fx.fract());
            let x1 = (x0 + 1).min(sw - 1);
            let top = src[y0 * sw + x0] + (src[y0 * sw + x1] - src[y0 * sw + x0]) * tx;
            let bot = src[y1 * sw + x0] + (src[y1 * sw + x1] - src[y1 * sw + x0]) * tx;
            out[y * dw + x] = top + (bot - top) * ty;
        }
    }
    out
}

/// The network's probabilities `prob` (`pw × ph` over the whole picture) at the guide's
/// size `gw × gh`, edges snapped to the guide by a guided filter (He et al. 2010) of
/// radius `r` pixels and regularisation `eps`. Output clamped to 0..1.
#[allow(clippy::too_many_arguments)]
pub fn guided_upsample(
    prob: &[f32],
    (pw, ph): (usize, usize),
    guide: &[f32],
    (gw, gh): (usize, usize),
    r: usize,
    eps: f32,
) -> Vec<f32> {
    // The bilinear ramp is steepened around ½ first: a network's soft edge spans a whole
    // cell, which would leave the filter little to snap.
    let p: Vec<f32> = resample(prob, pw, ph, gw, gh)
        .into_iter()
        .map(|v| {
            let t = ((v - 0.25) / 0.5).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        })
        .collect();
    let mi = box_mean(guide, gw, gh, r);
    let mp = box_mean(&p, gw, gh, r);
    let ip: Vec<f32> = guide.iter().zip(&p).map(|(a, b)| a * b).collect();
    let ii: Vec<f32> = guide.iter().map(|a| a * a).collect();
    let mip = box_mean(&ip, gw, gh, r);
    let mii = box_mean(&ii, gw, gh, r);
    let n = gw * gh;
    let mut a = vec![0f32; n];
    let mut b = vec![0f32; n];
    for i in 0..n {
        let var = (mii[i] - mi[i] * mi[i]).max(0.0);
        let cov = mip[i] - mi[i] * mp[i];
        a[i] = cov / (var + eps);
        b[i] = mp[i] - a[i] * mi[i];
    }
    let ma = box_mean(&a, gw, gh, r);
    let mb = box_mean(&b, gw, gh, r);
    (0..n)
        .map(|i| (ma[i] * guide[i] + mb[i]).clamp(0.0, 1.0))
        .collect()
}

/// The guided filter's radius for a picture of long side `long`: about a network cell.
pub fn guide_radius(long: usize, cells: usize) -> usize {
    (long / cells.max(1)).clamp(2, 32)
}

/// Resamples `q` (over the picture, `gw × gh`) into a raster over the source frame of
/// `fw × fh` sensor pixels, `geometry` mapping picture points to frame points (the
/// picture's orientation and any warp it had). At most [`RASTER_LONG`] texels a side.
pub fn frame_raster(
    q: &[f32],
    (gw, gh): (usize, usize),
    geometry: &Geometry<'_>,
    (fw, fh): (f64, f64),
) -> Raster {
    let long = fw.max(fh).max(1.0);
    let k = (f64::from(RASTER_LONG) / long).min(1.0);
    let rw = ((fw * k).round() as u32).max(1);
    let rh = ((fh * k).round() as u32).max(1);
    let mut r = Raster::new(super::brush::next_raster_id(), rw, rh);
    // Picture points on a grid → frame points; inverted per texel by locating the frame
    // point in the grid's cells (the map is near affine: an orientation, maybe a small
    // warp).
    const N: usize = 33;
    let mut grid = vec![(0f64, 0f64); N * N];
    for j in 0..N {
        for i in 0..N {
            let (u, v) = (i as f64 / (N - 1) as f64, j as f64 / (N - 1) as f64);
            grid[j * N + i] = geometry.frame_point(u, v);
        }
    }
    // The affine fit of the corners gives the first guess; one Newton-like correction
    // step through the local cell keeps a small warp's error under a texel.
    let p00 = grid[0];
    let p10 = grid[N - 1];
    let p01 = grid[(N - 1) * N];
    let (ax, ay) = (p10.0 - p00.0, p10.1 - p00.1);
    let (bx, by) = (p01.0 - p00.0, p01.1 - p00.1);
    let det = ax * by - ay * bx;
    if det.abs() < 1e-12 {
        return r;
    }
    let inv = |x: f64, y: f64| -> (f64, f64) {
        let (dx, dy) = (x - p00.0, y - p00.1);
        ((dx * by - dy * bx) / det, (ax * dy - ay * dx) / det)
    };
    let fwd = |u: f64, v: f64| -> (f64, f64) {
        let gx = (u.clamp(0.0, 1.0) * (N - 1) as f64).min((N - 1) as f64 - 1e-9);
        let gy = (v.clamp(0.0, 1.0) * (N - 1) as f64).min((N - 1) as f64 - 1e-9);
        let (i, j) = (gx.floor() as usize, gy.floor() as usize);
        let (tx, ty) = (gx - i as f64, gy - j as f64);
        let at = |a: usize, b: usize| grid[b * N + a];
        let (a, b, c, d) = (at(i, j), at(i + 1, j), at(i, j + 1), at(i + 1, j + 1));
        let lx = |p: f64, q: f64, t: f64| p + (q - p) * t;
        (
            lx(lx(a.0, b.0, tx), lx(c.0, d.0, tx), ty),
            lx(lx(a.1, b.1, tx), lx(c.1, d.1, tx), ty),
        )
    };
    let stride = r.stride as usize;
    for y in 0..rh as usize {
        for x in 0..rw as usize {
            let fx = (x as f64 + 0.5) / f64::from(rw);
            let fy = (y as f64 + 0.5) / f64::from(rh);
            let (mut u, mut v) = inv(fx, fy);
            for _ in 0..2 {
                let (px, py) = fwd(u, v);
                let (du, dv) = inv(fx - px + p00.0, fy - py + p00.1);
                u += du;
                v += dv;
            }
            if !(-0.001..=1.001).contains(&u) || !(-0.001..=1.001).contains(&v) {
                continue;
            }
            let sx = (u * gw as f64 - 0.5).clamp(0.0, (gw - 1) as f64);
            let sy = (v * gh as f64 - 0.5).clamp(0.0, (gh - 1) as f64);
            let (x0, y0) = (sx.floor() as usize, sy.floor() as usize);
            let (x1, y1) = ((x0 + 1).min(gw - 1), (y0 + 1).min(gh - 1));
            let (tx, ty) = ((sx - x0 as f64) as f32, (sy - y0 as f64) as f32);
            let top = q[y0 * gw + x0] + (q[y0 * gw + x1] - q[y0 * gw + x0]) * tx;
            let bot = q[y1 * gw + x0] + (q[y1 * gw + x1] - q[y1 * gw + x0]) * tx;
            let val = (top + (bot - top) * ty).clamp(0.0, 1.0);
            r.data[y * stride + x] = (val * 65535.0 + 0.5) as u16;
        }
    }
    r
}

/// A raster from texels (`w × h`, row-major, 0..65535), e.g. read from the disk cache.
pub fn raster(w: u32, h: u32, texels: &[u16]) -> Option<Raster> {
    if w == 0 || h == 0 || texels.len() != w as usize * h as usize || w.max(h) > RASTER_LONG {
        return None;
    }
    let mut r = Raster::new(super::brush::next_raster_id(), w, h);
    let stride = r.stride as usize;
    for y in 0..h as usize {
        r.data[y * stride..y * stride + w as usize]
            .copy_from_slice(&texels[y * w as usize..(y + 1) * w as usize]);
    }
    Some(r)
}

const MAGIC: &[u8; 8] = b"SPAIMSK1";

/// A raster for the disk cache: 8-bit levels, run-length coded (masks are mostly 0 or
/// full), behind a small header.
pub fn encode(r: &Raster) -> Vec<u8> {
    let mut out = Vec::with_capacity(64 + (r.w * r.h / 16) as usize);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&r.w.to_le_bytes());
    out.extend_from_slice(&r.h.to_le_bytes());
    let stride = r.stride as usize;
    let mut run: Option<(u8, u32)> = None;
    let flush = |out: &mut Vec<u8>, run: &mut Option<(u8, u32)>| {
        if let Some((v, n)) = run.take() {
            out.push(v);
            // Run length, LEB128.
            let mut n = n;
            loop {
                let b = (n & 0x7f) as u8;
                n >>= 7;
                if n == 0 {
                    out.push(b);
                    break;
                }
                out.push(b | 0x80);
            }
        }
    };
    for y in 0..r.h as usize {
        for x in 0..r.w as usize {
            let v = ((u32::from(r.data[y * stride + x]) * 255 + 32767) / 65535) as u8;
            match &mut run {
                Some((rv, n)) if *rv == v => *n += 1,
                _ => {
                    flush(&mut out, &mut run);
                    run = Some((v, 1));
                }
            }
        }
    }
    flush(&mut out, &mut run);
    out
}

/// [`encode`]'s inverse; None for anything malformed.
pub fn decode(b: &[u8]) -> Option<Raster> {
    if b.len() < 16 || &b[..8] != MAGIC {
        return None;
    }
    let w = u32::from_le_bytes(b[8..12].try_into().ok()?);
    let h = u32::from_le_bytes(b[12..16].try_into().ok()?);
    if w == 0 || h == 0 || w.max(h) > RASTER_LONG {
        return None;
    }
    let total = w as usize * h as usize;
    let mut tex = Vec::with_capacity(total);
    let mut at = 16;
    while at < b.len() {
        let v = b[at];
        at += 1;
        let mut n = 0u64;
        let mut shift = 0;
        loop {
            let c = *b.get(at)?;
            at += 1;
            n |= u64::from(c & 0x7f) << shift;
            if c & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift > 35 {
                return None;
            }
        }
        if tex.len() as u64 + n > total as u64 {
            return None;
        }
        let t = u16::from(v) * 257;
        tex.extend(std::iter::repeat_n(t, n as usize));
    }
    if tex.len() != total {
        return None;
    }
    raster(w, h, &tex)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::mask::AiPoint;

    fn obj() -> AiMask {
        let mut m = AiMask::new(AiKind::Object, "efficientsam-ti");
        m.points.push(AiPoint {
            x: 0.4,
            y: 0.5,
            positive: true,
        });
        m
    }

    #[test]
    fn keys_follow_photo_prompt_and_model() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let m = obj();
        assert_eq!(key(&a, &m), key(&a, &m));
        assert_ne!(key(&a, &m), key(&b, &m));
        let mut n = m.clone();
        n.points[0].x = 0.41;
        assert_ne!(key(&a, &m), key(&a, &n));
        let mut n = m.clone();
        n.model = "efficientsam-s".into();
        assert_ne!(key(&a, &m), key(&a, &n));
        // The key field itself and clicks on a subject row change nothing.
        let mut s = AiMask::new(AiKind::Subject, "isnet-general-use");
        let k = key(&a, &s);
        s.key = [7; 32];
        s.points.push(AiPoint {
            x: 0.1,
            y: 0.1,
            positive: true,
        });
        assert_eq!(key(&a, &s), k);
    }

    #[test]
    fn rasters_round_trip_through_the_disk_format() {
        let (w, h) = (37u32, 23u32);
        let tex: Vec<u16> = (0..w * h)
            .map(|i| {
                if (i % w) < 10 {
                    0
                } else if (i % w) > 30 {
                    65535
                } else {
                    (i * 997 % 65536) as u16
                }
            })
            .collect();
        let r = raster(w, h, &tex).unwrap();
        let e = encode(&r);
        let d = decode(&e).unwrap();
        assert_eq!((d.w, d.h), (w, h));
        for y in 0..h as i64 {
            for x in 0..w as i64 {
                assert!((d.texel(x, y) - r.texel(x, y)).abs() <= 0.5 / 255.0 + 1e-6);
            }
        }
        // A flat mask is a few bytes.
        let flat = raster(1000, 800, &vec![65535; 800_000]).unwrap();
        assert!(encode(&flat).len() < 32);
        // Damaged files are refused, never a panic.
        assert!(decode(&e[..e.len() - 1]).is_none());
        assert!(decode(b"SPAIMSK1\x01\0\0\0\x01\0\0\0\x05\xff\xff\xff\xff\xff\xff").is_none());
        assert!(decode(b"nonsense").is_none());
    }

    #[test]
    fn guided_upsampling_snaps_edges_to_the_guide() {
        // A sharp vertical edge in the guide at x = 70 of 128; the coarse mask (8 wide)
        // puts its soft edge near it. After the filter the mask's 0.5 crossing is at the
        // guide's edge, within a pixel.
        let (gw, gh) = (128usize, 64usize);
        let guide: Vec<f32> = (0..gw * gh)
            .map(|i| if i % gw < 70 { 0.8 } else { 0.1 })
            .collect();
        let (pw, ph) = (8usize, 4usize);
        let prob: Vec<f32> = (0..pw * ph)
            .map(|i| {
                if i % pw < 4 {
                    1.0
                } else if i % pw == 4 {
                    0.5
                } else {
                    0.0
                }
            })
            .collect();
        let q = guided_upsample(
            &prob,
            (pw, ph),
            &guide,
            (gw, gh),
            guide_radius(gw, pw),
            1e-4,
        );
        let row = &q[32 * gw..33 * gw];
        let lin = resample(&prob, pw, ph, gw, gh);
        let lrow = &lin[32 * gw..33 * gw];
        // Inside and outside stay; across the guide's edge the mask steps, where plain
        // bilinear only ramps.
        assert!(row[60] > 0.8, "{}", row[60]);
        assert!(row[80] < 0.1, "{}", row[80]);
        let step = row[69] - row[70];
        assert!(step > 0.5, "guided step {step}");
        assert!(
            lrow[69] - lrow[70] < 0.1,
            "bilinear step {}",
            lrow[69] - lrow[70]
        );
    }

    #[test]
    fn frame_raster_inverts_the_orientation() {
        use crate::ops::geometry::Plan;
        let _ = Plan::new; // the warp case is covered by the app-level tests
        // Frame geometry (no warp): the raster is the picture resampled.
        let (gw, gh) = (40usize, 20usize);
        let q: Vec<f32> = (0..gw * gh)
            .map(|i| if i % gw < 10 { 1.0 } else { 0.0 })
            .collect();
        let g = Geometry::Frame { w: 400.0, h: 200.0 };
        let r = frame_raster(&q, (gw, gh), &g, (400.0, 200.0));
        assert_eq!((r.w, r.h), (400, 200));
        assert!(r.sample(0.1, 0.5) > 0.99);
        assert!(r.sample(0.5, 0.5) < 0.01);
    }

    #[test]
    fn the_store_is_bounded_and_uses_its_loader() {
        let mut s = Store::default();
        let big = || raster(4096, 4096, &vec![1; 4096 * 4096]).unwrap();
        for i in 0..20u8 {
            s.put([i; 32], big());
        }
        assert!(s.bytes() <= STORE_BYTES);
        assert!(s.contains(&[19; 32]));
        assert!(!s.contains(&[0; 32]));
        s.set_loader(Some(Box::new(|k| {
            (k[0] == 200).then(|| raster(2, 2, &[0, 1, 2, 3]).unwrap())
        })));
        assert!(s.get(&[200; 32]).is_some());
        assert!(s.contains(&[200; 32]));
        assert!(s.get(&[201; 32]).is_none());
    }
}
