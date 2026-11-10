//! The photo's depth map: depth mask rows, the
//! depth lens blur and relight.
//!
//! - **Network**: the app runs the Depth feature's model (`specular_ai::remove::DepthNet`)
//!   on the same picture AI masks see ([`super::ai::Picture`]: the whole frame upright).
//!   It gives relative inverse depth (larger is nearer), up to an unknown scale and shift.
//! - **Normalised depth** ([`normalise`]): 0 at the nearest and 1 at the farthest point of
//!   the photo (robust ends: the 1st and 99.5th percentiles of inverse depth), linear in
//!   inverse depth, which is what a lens's blur grows with. Sliders and the focus point all
//!   work in this 0..1.
//! - **Raster**: upsampled to the picture (bilinear; a depth map's edges are refined at
//!   render time like AI masks') and resampled into a raster over the source frame
//!   ([`super::ai::frame_raster`]), so it follows crop, straighten and lens changes without
//!   being computed again. Kept in the AI mask store ([`super::ai::store`]) and the app's
//!   disk cache under [`key`] (photo and model); the sidecar keeps only the model's id.
//! - **On the picture** ([`DepthMap::on_picture`]): ops read depth from their table, the
//!   raster resampled onto the graph's picture at most [`PICTURE_LONG`] texels a side
//!   through the warp (a 65² grid of frame points, bilinear in between), so the cache keys
//!   change when the depth map arrives.

use super::Geometry;
use super::brush::Raster;
use crate::model::Edit;
use crate::model::edit::SourceKey;
use crate::model::mask::MaskExpr;

const KEY_CONTEXT: &str = "specular-develop 2026-11-01 depth map v1";
/// Longest side of the depth table an op reads.
pub const PICTURE_LONG: u32 = 512;
/// Points per side of the frame grid that maps the picture to the frame.
const GRID: usize = 65;

/// A photo's identity for depth maps and removal patches: its size and the hash of its
/// first and last 64 KiB (the edit's source key), so a copy on another computer keys the
/// same.
pub fn photo(source: &SourceKey) -> [u8; 32] {
    let mut h = blake3::Hasher::new_derive_key("specular-develop 2026-11-01 photo v1");
    h.update(&source.size.to_le_bytes());
    h.update(&source.head_tail);
    *h.finalize().as_bytes()
}

/// The depth map's key for `photo` ([`photo`]) made with `model`.
pub fn key(photo: &[u8; 32], model: &str) -> [u8; 32] {
    let mut h = blake3::Hasher::new_derive_key(KEY_CONTEXT);
    h.update(photo);
    h.update(model.as_bytes());
    *h.finalize().as_bytes()
}

fn walk(e: &MaskExpr, f: &mut dyn FnMut(&str)) {
    match e {
        MaskExpr::Depth(d) => f(&d.model),
        MaskExpr::Invert(a) | MaskExpr::Opacity(_, a) => walk(a, f),
        MaskExpr::Combine(_, a, b) => {
            walk(a, f);
            walk(b, f);
        }
        _ => {}
    }
}

/// Whether a mask expression has a depth row.
pub fn has_depth(e: &MaskExpr) -> bool {
    let mut any = false;
    walk(e, &mut |_| any = true);
    any
}

/// The depth models `edit`'s enabled modules ask for, in mask rows or as the depth ops'
/// own (`model` field), once each.
pub fn models(edit: &Edit) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |m: &str| {
        if !m.is_empty() && !out.iter().any(|o| o == m) {
            out.push(m.to_owned());
        }
    };
    for m in edit.modules.iter().filter(|m| m.enabled) {
        if let Some(b) = &m.blend {
            walk(&b.mask, &mut add);
        }
        if crate::ops::depth::uses_depth(m.kind)
            && let Some(model) = m.params.to_fields().text("model")
        {
            add(model);
        }
    }
    out
}

/// The `p`-quantile of finite values (0..1), by sorting a sample.
fn quantile(v: &[f32], p: f64) -> f32 {
    let mut s: Vec<f32> = v
        .iter()
        .copied()
        .filter(|x| x.is_finite())
        .step_by((v.len() / 65536).max(1))
        .collect();
    if s.is_empty() {
        return 0.0;
    }
    s.sort_by(f32::total_cmp);
    s[((s.len() - 1) as f64 * p).round() as usize]
}

/// Normalised depth from the network's inverse depth: 0 nearest, 1 farthest.
pub fn normalise(inv: &[f32]) -> Vec<f32> {
    let lo = quantile(inv, 0.01);
    let hi = quantile(inv, 0.995);
    let span = (hi - lo).max(1e-6);
    inv.iter()
        .map(|&d| {
            if d.is_finite() {
                (1.0 - (d - lo) / span).clamp(0.0, 1.0)
            } else {
                1.0
            }
        })
        .collect()
}

/// The depth raster over the source frame from the network's inverse depth `inv`
/// (`nw × nh` over the whole picture of `gw × gh`; `geometry` maps picture points to frame
/// points, `frame` the frame's size in sensor pixels).
pub fn raster(
    inv: &[f32],
    (nw, nh): (usize, usize),
    (gw, gh): (usize, usize),
    geometry: &Geometry<'_>,
    frame: (f64, f64),
) -> Raster {
    let t = normalise(inv);
    let up = super::ai::resample(&t, nw, nh, gw, gh);
    super::ai::frame_raster(&up, (gw, gh), geometry, frame)
}

/// A depth raster for the disk cache: a small header and the 16-bit texels (depth needs
/// finer steps than a mask's 8 bits).
pub fn encode(r: &Raster) -> Vec<u8> {
    let mut b = Vec::with_capacity(16 + 2 * (r.w * r.h) as usize);
    b.extend_from_slice(b"SPDM1\0\0\0");
    b.extend_from_slice(&r.w.to_le_bytes());
    b.extend_from_slice(&r.h.to_le_bytes());
    for y in 0..r.h {
        for x in 0..r.w {
            b.extend_from_slice(&r.data[(y * r.stride + x) as usize].to_le_bytes());
        }
    }
    b
}

/// [`encode`]'s inverse; None for anything malformed.
pub fn decode(b: &[u8]) -> Option<Raster> {
    if b.get(..8)? != b"SPDM1\0\0\0" {
        return None;
    }
    let w = u32::from_le_bytes(b.get(8..12)?.try_into().ok()?);
    let h = u32::from_le_bytes(b.get(12..16)?.try_into().ok()?);
    let n = (w as usize).checked_mul(h as usize)?;
    if w == 0 || h == 0 || w > super::ai::RASTER_LONG || h > super::ai::RASTER_LONG {
        return None;
    }
    if b.len() != 16 + 2 * n {
        return None;
    }
    let tex: Vec<u16> = b[16..]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    super::ai::raster(w, h, &tex)
}

/// The depth table an op reads: `[w, h, depth…]` over the graph's picture (0..1 each), or
/// `[0, 0]` without a depth map.
#[derive(Clone, Debug, PartialEq)]
pub struct DepthMap {
    pub w: u32,
    pub h: u32,
    pub data: Vec<f32>,
}

impl DepthMap {
    /// None: an empty table.
    pub fn none() -> DepthMap {
        DepthMap {
            w: 0,
            h: 0,
            data: Vec::new(),
        }
    }

    /// The table's words.
    pub fn words(&self) -> Vec<f32> {
        let mut v = Vec::with_capacity(2 + self.data.len());
        v.push(self.w as f32);
        v.push(self.h as f32);
        v.extend_from_slice(&self.data);
        v
    }

    /// The raster `r` (over the frame) resampled onto the graph's picture of `image`
    /// pixels that `geometry` describes.
    pub fn on_picture(r: &Raster, geometry: &Geometry<'_>, image: (u32, u32)) -> DepthMap {
        let (iw, ih) = (image.0.max(1), image.1.max(1));
        let long = iw.max(ih);
        let s = f64::from(PICTURE_LONG.min(long)) / f64::from(long);
        let w = ((f64::from(iw) * s).round() as u32).max(1);
        let h = ((f64::from(ih) * s).round() as u32).max(1);
        // Frame points on a grid over the picture.
        let mut grid = Vec::with_capacity(GRID * GRID);
        for j in 0..GRID {
            for i in 0..GRID {
                let (u, v) = (i as f64 / (GRID - 1) as f64, j as f64 / (GRID - 1) as f64);
                grid.push(geometry.frame_point(u, v));
            }
        }
        let mut data = Vec::with_capacity((w * h) as usize);
        for y in 0..h {
            let v = (f64::from(y) + 0.5) / f64::from(h) * (GRID - 1) as f64;
            let j0 = (v.floor() as usize).min(GRID - 2);
            let ty = v - j0 as f64;
            for x in 0..w {
                let u = (f64::from(x) + 0.5) / f64::from(w) * (GRID - 1) as f64;
                let i0 = (u.floor() as usize).min(GRID - 2);
                let tx = u - i0 as f64;
                let g = |i: usize, j: usize| grid[j * GRID + i];
                let lerp = |a: (f64, f64), b: (f64, f64), t: f64| {
                    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
                };
                let top = lerp(g(i0, j0), g(i0 + 1, j0), tx);
                let bot = lerp(g(i0, j0 + 1), g(i0 + 1, j0 + 1), tx);
                let (fx, fy) = lerp(top, bot, ty);
                // Held to the outer texels' centres (the raster fades to 0 past them).
                let cx = fx.clamp(0.5 / f64::from(r.w), 1.0 - 0.5 / f64::from(r.w));
                let cy = fy.clamp(0.5 / f64::from(r.h), 1.0 - 0.5 / f64::from(r.h));
                data.push(r.sample(cx as f32, cy as f32));
            }
        }
        DepthMap { w, h, data }
    }

    /// Bilinear depth at picture pixel (`px`, `py`) of a `iw × ih` picture, as the kernels
    /// read it (`dv_depth_at`); -1 without a map.
    pub fn at(&self, px: i32, py: i32, iw: u32, ih: u32) -> f32 {
        if self.w == 0 || self.h == 0 {
            return -1.0;
        }
        let gx =
            ((px as f32 + 0.5) / iw as f32 * self.w as f32 - 0.5).clamp(0.0, (self.w - 1) as f32);
        let gy =
            ((py as f32 + 0.5) / ih as f32 * self.h as f32 - 0.5).clamp(0.0, (self.h - 1) as f32);
        let (x0, y0) = (gx.floor() as u32, gy.floor() as u32);
        let (x1, y1) = ((x0 + 1).min(self.w - 1), (y0 + 1).min(self.h - 1));
        let (tx, ty) = (gx - x0 as f32, gy - y0 as f32);
        let d = |x: u32, y: u32| self.data[(y * self.w + x) as usize];
        let top = d(x0, y0) + (d(x1, y0) - d(x0, y0)) * tx;
        let bot = d(x0, y1) + (d(x1, y1) - d(x0, y1)) * tx;
        top + (bot - top) * ty
    }
}

/// Shared WGSL: depth at a picture pixel from the op's table (`dt`), as [`DepthMap::at`];
/// -1 without a map.
pub const DEPTH_WGSL: &str = "fn dv_depth_at(px: f32, py: f32) -> f32 {
    let w = u32(dt(0u));
    let h = u32(dt(1u));
    if (w == 0u || h == 0u) { return -1.0; }
    let gx = clamp((px + 0.5) / f32(pu(4u)) * f32(w) - 0.5, 0.0, f32(w - 1u));
    let gy = clamp((py + 0.5) / f32(pu(5u)) * f32(h) - 0.5, 0.0, f32(h - 1u));
    let x0 = u32(floor(gx));
    let y0 = u32(floor(gy));
    let x1 = min(x0 + 1u, w - 1u);
    let y1 = min(y0 + 1u, h - 1u);
    let tx = gx - f32(x0);
    let ty = gy - f32(y0);
    let a = dt(2u + y0 * w + x0);
    let b = dt(2u + y0 * w + x1);
    let c = dt(2u + y1 * w + x0);
    let d = dt(2u + y1 * w + x1);
    let top = a + (b - a) * tx;
    let bot = c + (d - c) * tx;
    return top + (bot - top) * ty;
}
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalised_depth_is_near_zero_far_one() {
        // Inverse depth: large near.
        let inv: Vec<f32> = (0..1000).map(|i| i as f32).collect();
        let t = normalise(&inv);
        assert!(t[999] < 0.01 && t[0] > 0.99);
        assert!(t.windows(2).all(|w| w[1] <= w[0]));
        // Constant input: no division by zero.
        assert!(normalise(&[3.0; 10]).iter().all(|v| v.is_finite()));
    }

    #[test]
    fn keys_follow_photo_and_model() {
        let s = SourceKey {
            size: 10,
            mtime_ns: 5,
            head_tail: [1; 32],
        };
        let p = photo(&s);
        // The modification time does not count (a copy keys the same).
        assert_eq!(p, photo(&SourceKey { mtime_ns: 99, ..s }));
        assert_ne!(key(&p, "a"), key(&p, "b"));
        assert_ne!(key(&p, "a"), key(&photo(&SourceKey { size: 11, ..s }), "a"));
    }

    #[test]
    fn disk_form_round_trips() {
        let tex: Vec<u16> = (0..12).map(|i| i * 5000).collect();
        let r = super::super::ai::raster(4, 3, &tex).unwrap();
        let back = decode(&encode(&r)).unwrap();
        assert_eq!((back.w, back.h), (4, 3));
        for y in 0..3 {
            for x in 0..4 {
                assert_eq!(back.texel(x, y), r.texel(x, y));
            }
        }
        assert!(decode(&encode(&r)[..20]).is_none());
    }

    #[test]
    fn picture_table_follows_the_frame() {
        // A horizontal ramp over the frame, the picture the frame itself.
        let w = 64u32;
        let tex: Vec<u16> = (0..w * 32)
            .map(|i| ((i % w) as f32 / (w - 1) as f32 * 65535.0) as u16)
            .collect();
        let r = super::super::ai::raster(w, 32, &tex).unwrap();
        let g = Geometry::Frame { w: 64.0, h: 32.0 };
        let m = DepthMap::on_picture(&r, &g, (2048, 1024));
        assert_eq!((m.w, m.h), (512, 256));
        assert!(m.at(0, 500, 2048, 1024) < 0.02);
        assert!(m.at(2047, 10, 2048, 1024) > 0.98);
        let mid = m.at(1024, 512, 2048, 1024);
        assert!((mid - 0.5).abs() < 0.02, "{mid}");
        assert_eq!(DepthMap::none().at(1, 1, 4, 4), -1.0);
    }
}
