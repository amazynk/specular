//! Levels made of images placed at positions of their own: the stitched scans of 3DHISTECH
//! MIRAX (camera images that overlap), Roche/Ventana BIF (tiles with recorded joints) and the
//! tile grids of Olympus VSI frame files.
//!
//! A [`MosaicLevel`] lists its placed images in paint order (a later one is drawn over an
//! earlier one where they overlap) with a coarse bin index, so a read decodes only the images
//! that reach the region. Decoded images are kept in a small least-recently-used cache: a
//! viewer's tiles are smaller than most stored images, which neighbouring requests share.
//! Reads go through the level whose downsample is the largest power of two dividing the
//! requested scale, then box-reduce the rest, one band of output rows at a time (bounded
//! memory whatever the factor).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use parking_lot::Mutex;
use rayon::prelude::*;

use crate::composite::{Plane, box_reduce_plane, box_reduce_rgba};
use crate::{Error, Files, Position, ReadOptions, Region, Result, Rgba};

/// Most bytes of decoded images kept.
const CACHE_BYTES: usize = 96 << 20;
/// Output rows read per band.
const BAND_ROWS: u32 = 256;
/// Bin edge of the placement index, in level pixels.
const BIN: u32 = 1024;
/// Most bins per level (coarser bins for huge levels).
const MAX_BINS: u64 = 1 << 20;

/// One placed image (or part of one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Placed {
    /// Top-left in level pixels (may be negative or past the level's edge).
    pub x: i64,
    pub y: i64,
    pub w: u32,
    pub h: u32,
    /// The stored image, as the source numbers it.
    pub src: u32,
    /// Top-left of the part shown, in the decoded image's pixels.
    pub sx: u32,
    pub sy: u32,
}

/// One resolution.
pub(crate) struct MosaicLevel {
    pub width: u32,
    pub height: u32,
    /// Full-resolution pixels per level pixel (a power of two).
    pub factor: u32,
    /// In paint order.
    pub tiles: Vec<Placed>,
    bin: u32,
    cols: u32,
    rows: u32,
    bins: Vec<Vec<u32>>,
}

impl MosaicLevel {
    pub fn new(width: u32, height: u32, factor: u32, tiles: Vec<Placed>) -> MosaicLevel {
        let mut bin = BIN;
        while u64::from(width.div_ceil(bin)) * u64::from(height.div_ceil(bin)) > MAX_BINS {
            bin = bin.saturating_mul(2);
        }
        let (cols, rows) = (width.div_ceil(bin).max(1), height.div_ceil(bin).max(1));
        let mut bins = vec![Vec::new(); cols as usize * rows as usize];
        for (i, t) in tiles.iter().enumerate() {
            let Some((c0, r0, c1, r1)) = span(t, bin, cols, rows, width, height) else {
                continue;
            };
            for r in r0..=r1 {
                for c in c0..=c1 {
                    bins[(r * cols + c) as usize].push(i as u32);
                }
            }
        }
        MosaicLevel {
            width,
            height,
            factor,
            tiles,
            bin,
            cols,
            rows,
            bins,
        }
    }

    /// The placed images reaching `r` (level pixels), in paint order.
    fn hits(&self, r: Region) -> Vec<u32> {
        let p = Placed {
            x: i64::from(r.x),
            y: i64::from(r.y),
            w: r.w,
            h: r.h,
            src: 0,
            sx: 0,
            sy: 0,
        };
        let Some((c0, r0, c1, r1)) =
            span(&p, self.bin, self.cols, self.rows, self.width, self.height)
        else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for row in r0..=r1 {
            for c in c0..=c1 {
                out.extend(
                    self.bins[(row * self.cols + c) as usize]
                        .iter()
                        .copied()
                        .filter(|&i| overlaps(&self.tiles[i as usize], r)),
                );
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

/// The bins a placed rectangle covers (clipped to the level), inclusive.
fn span(
    t: &Placed,
    bin: u32,
    cols: u32,
    rows: u32,
    w: u32,
    h: u32,
) -> Option<(u32, u32, u32, u32)> {
    let x0 = t.x.max(0);
    let y0 = t.y.max(0);
    let x1 = (t.x + i64::from(t.w)).min(i64::from(w));
    let y1 = (t.y + i64::from(t.h)).min(i64::from(h));
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    let b = i64::from(bin);
    Some((
        ((x0 / b) as u32).min(cols - 1),
        ((y0 / b) as u32).min(rows - 1),
        (((x1 - 1) / b) as u32).min(cols - 1),
        (((y1 - 1) / b) as u32).min(rows - 1),
    ))
}

fn overlaps(t: &Placed, r: Region) -> bool {
    t.x < i64::from(r.x) + i64::from(r.w)
        && t.x + i64::from(t.w) > i64::from(r.x)
        && t.y < i64::from(r.y) + i64::from(r.h)
        && t.y + i64::from(t.h) > i64::from(r.y)
}

/// Where a mosaic's stored images come from.
pub(crate) trait TileSource: Send + Sync + std::any::Any {
    /// Image `src` of level `level` at plane `pos` as RGBA8; `None` when it is not stored
    /// (drawn as background).
    fn rgba(
        &self,
        files: Files,
        level: usize,
        src: u32,
        pos: Position,
        cancel: Option<&AtomicBool>,
    ) -> Result<Option<Rgba>>;

    /// One channel's values of image `src`; `None` when not stored.
    fn channel(
        &self,
        _files: Files,
        _level: usize,
        _src: u32,
        _channel: u32,
        _pos: Position,
        _cancel: Option<&AtomicBool>,
    ) -> Result<Option<Plane>> {
        Err(Error::Unsupported("channels of an RGB slide".into()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Key {
    level: usize,
    src: u32,
    /// `u32::MAX` for RGBA.
    channel: u32,
    pos: Position,
}

#[derive(Clone)]
enum Cached {
    Rgba(Arc<Rgba>),
    Plane(Arc<Plane>),
    Missing,
}

impl Cached {
    fn bytes(&self) -> usize {
        match self {
            Cached::Rgba(r) => r.data.len(),
            Cached::Plane(p) => p.values.len() * 4,
            Cached::Missing => 16,
        }
    }
}

#[derive(Default)]
struct Cache {
    map: HashMap<Key, (Cached, u64)>,
    tick: u64,
    bytes: usize,
}

impl Cache {
    fn get(&mut self, k: &Key) -> Option<Cached> {
        self.tick += 1;
        let t = self.tick;
        self.map.get_mut(k).map(|e| {
            e.1 = t;
            e.0.clone()
        })
    }

    fn put(&mut self, k: Key, v: Cached) {
        self.tick += 1;
        self.bytes += v.bytes();
        if let Some(old) = self.map.insert(k, (v, self.tick)) {
            self.bytes -= old.0.bytes();
        }
        while self.bytes > CACHE_BYTES && self.map.len() > 1 {
            let Some((&oldest, _)) = self.map.iter().min_by_key(|(_, e)| e.1) else {
                break;
            };
            if let Some(e) = self.map.remove(&oldest) {
                self.bytes -= e.0.bytes();
            }
        }
    }
}

/// A stitched pyramid.
pub(crate) struct Mosaic {
    /// Full resolution first.
    pub levels: Vec<MosaicLevel>,
    /// Where nothing is placed.
    pub background: [u8; 4],
    /// The value of channels where nothing is placed.
    pub background_value: f32,
    pub source: Box<dyn TileSource>,
    cache: Mutex<Cache>,
}

impl Mosaic {
    pub fn new(
        levels: Vec<MosaicLevel>,
        background: [u8; 4],
        source: Box<dyn TileSource>,
    ) -> Mosaic {
        Mosaic {
            levels,
            background,
            background_value: 0.0,
            source,
            cache: Mutex::new(Cache::default()),
        }
    }

    pub fn memory_bytes(&self) -> usize {
        self.cache.lock().bytes
    }

    pub fn source_any(&self) -> &dyn std::any::Any {
        self.source.as_ref()
    }

    pub fn trim(&self) -> usize {
        let mut c = self.cache.lock();
        let b = c.bytes;
        c.map.clear();
        c.bytes = 0;
        b
    }

    /// The level to read a 1/`scale` downscale from and the factor left.
    fn choose(&self, scale: u32) -> (usize, u32) {
        let mut best = (0, scale);
        for (i, l) in self.levels.iter().enumerate() {
            if l.factor <= scale && scale.is_multiple_of(l.factor) && scale / l.factor < best.1 {
                best = (i, scale / l.factor);
            }
        }
        best
    }

    fn image(
        &self,
        files: Files,
        level: usize,
        src: u32,
        channel: Option<u32>,
        pos: Position,
        cancel: Option<&AtomicBool>,
    ) -> Result<Cached> {
        let key = Key {
            level,
            src,
            channel: channel.unwrap_or(u32::MAX),
            pos,
        };
        if let Some(v) = self.cache.lock().get(&key) {
            return Ok(v);
        }
        let v = match channel {
            None => match self.source.rgba(files, level, src, pos, cancel) {
                Ok(Some(r)) => Cached::Rgba(Arc::new(r)),
                Ok(None) => Cached::Missing,
                Err(e @ (Error::Cancelled | Error::TooLarge | Error::Io(_))) => return Err(e),
                // A damaged image is drawn as background, and marks the read truncated.
                Err(_) => return Ok(Cached::Missing),
            },
            Some(c) => match self.source.channel(files, level, src, c, pos, cancel) {
                Ok(Some(p)) => Cached::Plane(Arc::new(p)),
                Ok(None) => Cached::Missing,
                Err(
                    e @ (Error::Cancelled | Error::TooLarge | Error::Io(_) | Error::Unsupported(_)),
                ) => {
                    return Err(e);
                }
                Err(_) => return Ok(Cached::Missing),
            },
        };
        self.cache.lock().put(key, v.clone());
        Ok(v)
    }

    /// Region `o.region` of the 1/`o.scale` downscale (clipped to `full`/scale by the caller)
    /// as RGBA8.
    pub fn read(&self, files: Files, o: &ReadOptions) -> Result<Rgba> {
        let r = o
            .region
            .ok_or_else(|| Error::Malformed("no region".into()))?;
        let mut out = vec![0u8; r.w as usize * r.h as usize * 4];
        let mut truncated = false;
        let mut y = 0;
        while y < r.h {
            crate::check_cancel(o.cancel)?;
            let bh = BAND_ROWS.min(r.h - y);
            let band = Region {
                x: r.x,
                y: r.y + y,
                w: r.w,
                h: bh,
            };
            let img = self.read_band(files, band, o, None)?;
            let Band::Rgba(img) = img else {
                return Err(Error::Malformed("band".into()));
            };
            truncated |= img.truncated;
            let n = r.w as usize * 4;
            out[y as usize * n..(y + bh) as usize * n]
                .copy_from_slice(&img.data[..bh as usize * n]);
            y += bh;
        }
        Ok(Rgba {
            width: r.w,
            height: r.h,
            data: out,
            truncated,
        })
    }

    /// One channel's values of a region of the 1/`o.scale` downscale.
    pub fn read_channel(&self, files: Files, channel: u32, o: &ReadOptions) -> Result<Plane> {
        let r = o
            .region
            .ok_or_else(|| Error::Malformed("no region".into()))?;
        let mut out = vec![0f32; r.w as usize * r.h as usize];
        let mut truncated = false;
        let mut y = 0;
        while y < r.h {
            crate::check_cancel(o.cancel)?;
            let bh = BAND_ROWS.min(r.h - y);
            let band = Region {
                x: r.x,
                y: r.y + y,
                w: r.w,
                h: bh,
            };
            let Band::Plane(p) = self.read_band(files, band, o, Some(channel))? else {
                return Err(Error::Malformed("band".into()));
            };
            truncated |= p.truncated;
            let n = r.w as usize;
            out[y as usize * n..(y + bh) as usize * n]
                .copy_from_slice(&p.values[..bh as usize * n]);
            y += bh;
        }
        Ok(Plane {
            width: r.w,
            height: r.h,
            values: out,
            truncated,
        })
    }

    /// One band (`r`, in output pixels) through the chosen level.
    fn read_band(
        &self,
        files: Files,
        r: Region,
        o: &ReadOptions,
        channel: Option<u32>,
    ) -> Result<Band> {
        let (li, k) = self.choose(o.scale.max(1));
        let level = &self.levels[li];
        // The band in level pixels; the canvas is the band's full size (parts past the level's
        // edge stay background).
        let (cw, ch) = (r.w.saturating_mul(k), r.h.saturating_mul(k));
        if u64::from(cw) * u64::from(ch) > crate::MAX_READ_PIXELS * 4 {
            return Err(Error::TooLarge);
        }
        let lr = Region {
            x: r.x.saturating_mul(k),
            y: r.y.saturating_mul(k),
            w: cw,
            h: ch,
        };
        let hits = level.hits(lr);
        let srcs: Vec<u32> = {
            let mut s: Vec<u32> = hits.iter().map(|&i| level.tiles[i as usize].src).collect();
            s.sort_unstable();
            s.dedup();
            s
        };
        let load = |&src: &u32| -> Result<(u32, Cached)> {
            crate::check_cancel(o.cancel)?;
            Ok((
                src,
                self.image(files, li, src, channel, o.position, o.cancel)?,
            ))
        };
        let decoded: HashMap<u32, Cached> = if o.parallel && srcs.len() > 1 {
            srcs.par_iter().map(load).collect::<Result<_>>()?
        } else {
            srcs.iter().map(load).collect::<Result<_>>()?
        };
        let mut truncated = false;
        let n = cw as usize * ch as usize;
        match channel {
            None => {
                let mut canvas = Vec::with_capacity(n * 4);
                for _ in 0..n {
                    canvas.extend_from_slice(&self.background);
                }
                for &i in &hits {
                    let t = &level.tiles[i as usize];
                    match decoded.get(&t.src) {
                        Some(Cached::Rgba(img)) => {
                            truncated |= img.truncated;
                            blit(
                                &mut canvas,
                                cw,
                                ch,
                                lr,
                                t,
                                &img.data,
                                img.width,
                                img.height,
                                4,
                            );
                        }
                        _ => truncated = true,
                    }
                }
                let img = Rgba {
                    width: cw,
                    height: ch,
                    data: canvas,
                    truncated,
                };
                Ok(Band::Rgba(box_reduce_rgba(&img, k)))
            }
            Some(_) => {
                let mut canvas = vec![self.background_value; n];
                for &i in &hits {
                    let t = &level.tiles[i as usize];
                    match decoded.get(&t.src) {
                        Some(Cached::Plane(p)) => {
                            truncated |= p.truncated;
                            blit(&mut canvas, cw, ch, lr, t, &p.values, p.width, p.height, 1);
                        }
                        _ => truncated = true,
                    }
                }
                let p = Plane {
                    width: cw,
                    height: ch,
                    values: canvas,
                    truncated,
                };
                Ok(Band::Plane(box_reduce_plane(&p, k)))
            }
        }
    }
}

enum Band {
    Rgba(Rgba),
    Plane(Plane),
}

/// Copy the part of placed image `t` (decoded `sw`×`sh`, `c` samples per pixel) inside the
/// canvas (`cw`×`ch`, whose top-left is `lr`'s in level pixels).
#[allow(clippy::too_many_arguments)]
fn blit<T: Copy>(
    canvas: &mut [T],
    cw: u32,
    ch: u32,
    lr: Region,
    t: &Placed,
    src: &[T],
    sw: u32,
    sh: u32,
    c: usize,
) {
    // The visible part of the placed rectangle, limited by the decoded image.
    let w = i64::from(t.w.min(sw.saturating_sub(t.sx)));
    let h = i64::from(t.h.min(sh.saturating_sub(t.sy)));
    let x0 = t.x.max(i64::from(lr.x));
    let y0 = t.y.max(i64::from(lr.y));
    let x1 = (t.x + w).min(i64::from(lr.x) + i64::from(cw));
    let y1 = (t.y + h).min(i64::from(lr.y) + i64::from(ch));
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let n = (x1 - x0) as usize * c;
    for y in y0..y1 {
        let sy = (y - t.y) as usize + t.sy as usize;
        let sx = (x0 - t.x) as usize + t.sx as usize;
        let s = (sy * sw as usize + sx) * c;
        let d =
            ((y - i64::from(lr.y)) as usize * cw as usize + (x0 - i64::from(lr.x)) as usize) * c;
        if let (Some(dst), Some(from)) = (canvas.get_mut(d..d + n), src.get(s..s + n)) {
            dst.copy_from_slice(from);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Images of solid grey `src`, 4×4 each.
    struct Solid;

    impl TileSource for Solid {
        fn rgba(
            &self,
            _: Files,
            _: usize,
            src: u32,
            _: Position,
            _: Option<&AtomicBool>,
        ) -> Result<Option<Rgba>> {
            if src == 99 {
                return Ok(None);
            }
            let v = src as u8;
            Ok(Some(Rgba {
                width: 4,
                height: 4,
                data: [v, v, v, 255].repeat(16),
                truncated: false,
            }))
        }
    }

    fn read(m: &Mosaic, scale: u32, r: Region) -> Rgba {
        m.read(
            Files { data: &[] },
            &ReadOptions {
                scale,
                region: Some(r),
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn placed(x: i64, y: i64, src: u32) -> Placed {
        Placed {
            x,
            y,
            w: 4,
            h: 4,
            src,
            sx: 0,
            sy: 0,
        }
    }

    #[test]
    fn later_images_cover_earlier_and_background_fills_gaps() {
        // Two overlapping images (the second drawn last) and one gap column.
        let l = MosaicLevel::new(
            10,
            4,
            1,
            vec![placed(0, 0, 10), placed(2, 0, 20), placed(9, 0, 99)],
        );
        let m = Mosaic::new(vec![l], [255, 255, 255, 255], Box::new(Solid));
        let img = read(
            &m,
            1,
            Region {
                x: 0,
                y: 0,
                w: 10,
                h: 1,
            },
        );
        let row: Vec<u8> = img.data.chunks(4).map(|p| p[0]).collect();
        assert_eq!(row, [10, 10, 20, 20, 20, 20, 255, 255, 255, 255]);
        assert!(img.truncated, "an image that is not stored marks the read");
    }

    #[test]
    fn scaled_reads_use_the_coarser_level_and_reduce_the_rest() {
        let l0 = MosaicLevel::new(
            8,
            8,
            1,
            vec![
                placed(0, 0, 10),
                placed(4, 0, 30),
                placed(0, 4, 50),
                placed(4, 4, 70),
            ],
        );
        let l1 = MosaicLevel::new(4, 4, 2, vec![placed(0, 0, 40)]);
        let m = Mosaic::new(vec![l0, l1], [0, 0, 0, 255], Box::new(Solid));
        // 1/2 comes from level 1 as stored.
        let img = read(
            &m,
            2,
            Region {
                x: 0,
                y: 0,
                w: 4,
                h: 4,
            },
        );
        assert!(img.data.chunks(4).all(|p| p[0] == 40));
        // 1/4 is level 1 halved.
        let img = read(
            &m,
            4,
            Region {
                x: 0,
                y: 0,
                w: 2,
                h: 2,
            },
        );
        assert_eq!((img.width, img.height), (2, 2));
        assert!(img.data.chunks(4).all(|p| p[0] == 40));
        // A region past the image is background.
        let img = read(
            &m,
            1,
            Region {
                x: 6,
                y: 6,
                w: 4,
                h: 4,
            },
        );
        assert_eq!(img.data[0], 70);
        assert_eq!(img.data[(3 * 4 + 3) * 4], 0);
    }

    #[test]
    fn parts_of_images_and_negative_positions() {
        let t = Placed {
            x: -2,
            y: -1,
            w: 3,
            h: 3,
            src: 7,
            sx: 1,
            sy: 1,
        };
        let l = MosaicLevel::new(4, 4, 1, vec![t]);
        let m = Mosaic::new(vec![l], [1, 1, 1, 255], Box::new(Solid));
        let img = read(
            &m,
            1,
            Region {
                x: 0,
                y: 0,
                w: 4,
                h: 4,
            },
        );
        let v: Vec<u8> = img.data.chunks(4).map(|p| p[0]).collect();
        assert_eq!(&v[..4], &[7, 1, 1, 1]);
        assert_eq!(&v[4..8], &[7, 1, 1, 1]);
        assert_eq!(&v[8..12], &[1, 1, 1, 1]);
    }

    #[test]
    fn cache_is_bounded_and_trimmed() {
        let l = MosaicLevel::new(4, 4, 1, vec![placed(0, 0, 3)]);
        let m = Mosaic::new(vec![l], [0; 4], Box::new(Solid));
        read(
            &m,
            1,
            Region {
                x: 0,
                y: 0,
                w: 4,
                h: 4,
            },
        );
        assert!(m.memory_bytes() > 0);
        assert!(m.trim() > 0);
        assert_eq!(m.memory_bytes(), 0);
    }
}
