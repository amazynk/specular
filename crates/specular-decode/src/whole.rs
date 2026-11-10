//! Shared pieces for decoders that produce a whole image in one go (PSD, the legacy formats,
//! developed camera RAW): conversion of interleaved 8-bit pixels to a [`Raster`], box-filter
//! downscaling to the exact size of a level, and a cache of whole levels from which regions are
//! cropped.

use crate::{DecodeError, DecodeRequest, Result};
use parking_lot::Mutex;
use specular_core::{CancelToken, PixelLayout, Raster, RasterPlane};
use std::sync::Arc;

/// Whole levels above this size are not cached (decoded again per request).
pub(crate) const LEVEL_CACHE_MAX: usize = 768 << 20;

/// An interleaved 8-bit image (`Gray`: 1 byte per pixel, `Rgba`: 4) as a one-plane raster.
pub(crate) fn interleaved(
    width: u32,
    height: u32,
    layout: PixelLayout,
    data: Vec<u8>,
    truncated: bool,
) -> Raster {
    let bpp = layout.pixel_bytes();
    Raster {
        width,
        height,
        origin: (0, 0),
        scale: 1,
        layout,
        planes: vec![RasterPlane {
            width,
            height,
            stride: width as usize * bpp,
            data,
            h_div: 1,
            v_div: 1,
        }],
        truncated,
    }
}

/// Box-filter the whole image `src` (at `src.scale`) down to level 1/`scale` of an image of
/// `full_w`×`full_h` pixels: exactly `full_w.div_ceil(scale)`×`full_h.div_ceil(scale)`. Blocks
/// running past the edge of `src` average what is there (and repeat the last row or column
/// when `src` is a pixel short, as a floor-sized half-resolution develop is). RGBA is averaged
/// with alpha weighting, so transparent pixels do not darken edges. Planes must not be
/// subsampled.
pub(crate) fn downscale(src: &Raster, full_w: u32, full_h: u32, scale: u32) -> Raster {
    if src.layout.is_wide() {
        return crate::linear::downscale_wide(src, full_w, full_h, scale);
    }
    let (ow, oh) = (full_w.div_ceil(scale), full_h.div_ceil(scale));
    downscale_part(
        src,
        (src.width, src.height),
        scale,
        crate::Rect {
            x: 0,
            y: 0,
            w: ow,
            h: oh,
        },
    )
}

/// The source pixels `a..b` (along one axis of a level `n` pixels long) that output pixels
/// `o..o + len` of a box filter by `f` average (`len` > 0); see [`downscale`].
pub(crate) fn source_span(o: u32, len: u32, f: u32, n: u32) -> (u32, u32) {
    let span = |o: usize| {
        let (f, n) = (f.max(1) as usize, n as usize);
        let a = (o * f).min(n.saturating_sub(1));
        (a, (o * f + f).clamp(a + 1, n.max(a + 1)))
    };
    let (a, _) = span(o as usize);
    let (_, b) = span(o as usize + len.max(1) as usize - 1);
    (a as u32, b as u32)
}

/// Region `out` of [`downscale`]'s result, from `src`: a part (at `src.origin`, 8-bit, not
/// wide) of a level of `level_w`×`level_h` pixels holding at least the pixels
/// [`source_span`] names for `out`. Equal to the same pixels of the whole downscale.
pub(crate) fn downscale_part(
    src: &Raster,
    (level_w, level_h): (u32, u32),
    scale: u32,
    out: crate::Rect,
) -> Raster {
    let f = (scale / src.scale.max(1)).max(1) as usize;
    let (ow, oh) = (out.w, out.h);
    let (sx, sy) = (src.origin.0 as usize, src.origin.1 as usize);
    let rgba = src.layout == PixelLayout::Rgba;
    let planes = src
        .planes
        .iter()
        .map(|p| {
            let bpp = p.bytes_per_pixel();
            let (pw, ph) = (level_w as usize, level_h as usize);
            let span = |o: usize, n: usize| {
                let a = (o * f).min(n.saturating_sub(1));
                (a, (o * f + f).clamp(a + 1, n.max(a + 1)))
            };
            let stride = ow as usize * bpp;
            let mut data = vec![0u8; stride * oh as usize];
            if pw == 0 || ph == 0 || p.width == 0 || p.height == 0 {
                return RasterPlane {
                    width: ow,
                    height: oh,
                    stride,
                    data,
                    h_div: 1,
                    v_div: 1,
                };
            }
            let mut sum = vec![0u64; bpp];
            for (oy, out_row) in data.chunks_exact_mut(stride.max(1)).enumerate() {
                let (y0, y1) = span(out.y as usize + oy, ph);
                for ox in 0..ow as usize {
                    let (x0, x1) = span(out.x as usize + ox, pw);
                    sum.fill(0);
                    let mut n = 0u64;
                    for y in y0..y1 {
                        let row = &p.data[(y - sy) * p.stride..];
                        for px in row[(x0 - sx) * bpp..(x1 - sx) * bpp].chunks_exact(bpp) {
                            if rgba && bpp == 4 {
                                let a = u64::from(px[3]);
                                for c in 0..3 {
                                    sum[c] += u64::from(px[c]) * a;
                                }
                                sum[3] += a;
                            } else {
                                for (s, &v) in sum.iter_mut().zip(px) {
                                    *s += u64::from(v);
                                }
                            }
                            n += 1;
                        }
                    }
                    let o = &mut out_row[ox * bpp..(ox + 1) * bpp];
                    if rgba && bpp == 4 {
                        let a = sum[3];
                        for c in 0..3 {
                            o[c] = (sum[c] + a / 2).checked_div(a).unwrap_or(0) as u8;
                        }
                        o[3] = ((a + n / 2) / n.max(1)) as u8;
                    } else {
                        for (o, s) in o.iter_mut().zip(&sum) {
                            *o = ((s + n / 2) / n.max(1)) as u8;
                        }
                    }
                }
            }
            RasterPlane {
                width: ow,
                height: oh,
                stride,
                data,
                h_div: 1,
                v_div: 1,
            }
        })
        .collect();
    Raster {
        width: ow,
        height: oh,
        origin: (out.x, out.y),
        scale,
        layout: src.layout,
        planes,
        truncated: src.truncated,
    }
}

/// Cache of whole levels. A request at 1/s is served from the cached level, else cut down from
/// a *base* level (decoded once and cached when small enough), and regions are cropped from it.
pub(crate) struct Levels {
    levels: Mutex<Vec<Arc<Raster>>>,
    /// Held while a level is produced, so concurrent requests share one base decode.
    work: Mutex<()>,
}

impl Levels {
    pub(crate) fn new() -> Levels {
        Levels {
            levels: Mutex::new(Vec::new()),
            work: Mutex::new(()),
        }
    }

    fn find(&self, scale: u32) -> Option<Arc<Raster>> {
        self.levels
            .lock()
            .iter()
            .find(|l| l.scale == scale)
            .cloned()
    }

    fn keep(&self, r: &Arc<Raster>) {
        if r.memory_bytes() <= LEVEL_CACHE_MAX {
            let mut ls = self.levels.lock();
            if !ls.iter().any(|l| l.scale == r.scale) {
                ls.push(r.clone());
            }
        }
    }

    /// The whole level 1/`scale` of a `full_w`×`full_h` image. `base_of(scale)` names the
    /// level it is cut down from and `decode_base(base)` decodes that level (its raster must
    /// have `scale == base`).
    pub(crate) fn level(
        &self,
        scale: u32,
        (full_w, full_h): (u32, u32),
        base_of: impl Fn(u32) -> u32,
        decode_base: impl FnOnce(u32) -> Result<Raster>,
        cancel: &CancelToken,
    ) -> Result<Arc<Raster>> {
        if let Some(l) = self.find(scale) {
            return Ok(l);
        }
        let _work = self.work.lock();
        if let Some(l) = self.find(scale) {
            return Ok(l);
        }
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let b = base_of(scale);
        let base = match self.find(b) {
            Some(l) => l,
            None => {
                let mut r = decode_base(b)?;
                r.scale = b;
                let r = Arc::new(r);
                self.keep(&r);
                r
            }
        };
        if b == scale {
            return Ok(base);
        }
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let l = Arc::new(downscale(&base, full_w, full_h, scale));
        self.keep(&l);
        Ok(l)
    }

    /// [`Levels::level`], then the request's region cropped from it.
    pub(crate) fn decode(
        &self,
        req: &DecodeRequest,
        size: (u32, u32),
        base_of: impl Fn(u32) -> u32,
        decode_base: impl FnOnce(u32) -> Result<Raster>,
        cancel: &CancelToken,
    ) -> Result<Raster> {
        let level = self.level(req.scale.max(1), size, base_of, decode_base, cancel)?;
        Ok(match req.region {
            Some(r) => crate::jpeg::crop(&level, r),
            None => (*level).clone(),
        })
    }

    /// Whether level 1/`scale` is cached.
    pub(crate) fn has(&self, scale: u32) -> bool {
        self.find(scale).is_some()
    }

    pub(crate) fn memory_bytes(&self) -> usize {
        self.levels.lock().iter().map(|l| l.memory_bytes()).sum()
    }

    /// Drops every cached level ([`crate::Session::trim_memory`]); returns their bytes.
    pub(crate) fn clear(&self) -> usize {
        drop_levels(&self.levels)
    }
}

/// Empties a session's cache of whole levels (the `Mutex<Vec<Arc<Raster>>>` most decoders
/// crop regions from), for [`crate::Session::trim_memory`]: returns the bytes it counted in
/// `memory_bytes`. A level a decode is still using is freed when that decode is done with it;
/// the next request decodes it again.
pub(crate) fn drop_levels(levels: &Mutex<Vec<Arc<Raster>>>) -> usize {
    let dropped = std::mem::take(&mut *levels.lock());
    dropped.iter().map(|l| l.memory_bytes()).sum()
}

/// Bytes of the whole level 1/`scale` of a `w`×`h` image with `bpp` bytes per pixel.
pub(crate) fn level_bytes(w: u32, h: u32, scale: u32, bpp: u64) -> u64 {
    let s = scale.max(1);
    u64::from(w.div_ceil(s)) * u64::from(h.div_ceil(s)) * bpp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downscale_exact_size_and_alpha_weighting() {
        // 3×1 RGBA: opaque red, transparent black, opaque blue.
        let src = interleaved(
            3,
            1,
            PixelLayout::Rgba,
            vec![255, 0, 0, 255, 0, 0, 0, 0, 0, 0, 255, 255],
            false,
        );
        let d = downscale(&src, 3, 1, 2);
        assert_eq!((d.width, d.height, d.scale), (2, 1, 2));
        // The transparent pixel does not darken the red one.
        assert_eq!(&d.planes[0].data, &[255, 0, 0, 128, 0, 0, 255, 255]);
    }

    #[test]
    fn downscale_from_short_half_level() {
        // A 5×3 image whose half level came out floor-sized (2×1) is padded to 3×2.
        let mut half = interleaved(2, 1, PixelLayout::Gray, vec![10, 20], false);
        half.scale = 2;
        let d = downscale(&half, 5, 3, 2);
        assert_eq!((d.width, d.height), (3, 2));
        assert_eq!(&d.planes[0].data, &[10, 20, 20, 10, 20, 20]);
        let q = downscale(&half, 5, 3, 4);
        assert_eq!((q.width, q.height), (2, 1));
        assert_eq!(&q.planes[0].data, &[15, 20]);
    }
}
