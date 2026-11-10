//! FlashPix (`.fpx`, Kodak/Microsoft/HP 1996): an OLE compound file holding a resolution
//! pyramid of 64 x 64 tiles, as the FlashPix Format Specification 1.0.1 describes it.
//!
//! The `Data Object Store 000001` storage has an `Image Contents` property set (number of
//! resolutions 0x01000000; per resolution r, width 0x02rr0000, height 0x02rr0001, colour
//! 0x02rr0002; JPEG tables 0x03tt0001) and one `Resolution rrrr` storage per level, the
//! smallest first, each with a `Subimage 0000 Header` (from byte 28: header length, width,
//! height, tiles, tile width, tile height, channels, tile table offset and entry length; a
//! tile entry is offset, size, compression and its parameter) and the tile data. Tiles are
//! uncompressed (channels interleaved), one colour, or JPEG (abbreviated streams whose tables
//! come from the property set; the parameter's bytes are interleaving, chroma subsampling,
//! whether the JPEG holds YCbCr to convert, and the table index).
//!
//! Decoded: one to four channels in the NIF RGB, PhotoYCC and monochrome colour spaces, with
//! opacity (premultiplied, as the specification stores it). JPEG tiles with four channels are
//! refused by name. The largest resolution is decoded whole; smaller levels are cut from it.

use std::sync::Arc;

use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};

use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, Result, Session};

pub struct FpxDecoder;

const STORE: &str = "Data Object Store 000001";
const CONTENTS: &str = "Data Object Store 000001/\u{5}Image Contents";

/// Colour spaces of the subimage colour property.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Space {
    Mono,
    PhotoYcc,
    NifRgb,
}

/// The values of a property set section we use: (id, value).
enum Value {
    Int(u32),
    Blob(Vec<u8>),
}

fn u32le(d: &[u8], p: usize) -> Option<u32> {
    d.get(p..p + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// The first section of an OLE property set stream.
fn properties(d: &[u8]) -> Option<Vec<(u32, Value)>> {
    if d.get(..2)? != [0xfe, 0xff] {
        return None;
    }
    let sec = u32le(d, 44)? as usize;
    let count = u32le(d, sec + 4)? as usize;
    let mut out = Vec::new();
    for i in 0..count.min(4096) {
        let id = u32le(d, sec + 8 + 8 * i)?;
        let off = sec + u32le(d, sec + 12 + 8 * i)? as usize;
        let v = match u32le(d, off)? & 0xffff {
            2 => Value::Int(u32::from(u16::from_le_bytes([
                *d.get(off + 4)?,
                *d.get(off + 5)?,
            ]))),
            3 | 19 => Value::Int(u32le(d, off + 4)?),
            65 => {
                let n = u32le(d, off + 4)? as usize;
                Value::Blob(d.get(off + 8..off + 8 + n)?.to_vec())
            }
            _ => continue,
        };
        out.push((id, v));
    }
    Some(out)
}

fn int(p: &[(u32, Value)], id: u32) -> Option<u32> {
    p.iter().find_map(|(i, v)| match v {
        Value::Int(n) if *i == id => Some(*n),
        _ => None,
    })
}

fn blob(p: &[(u32, Value)], id: u32) -> Option<&[u8]> {
    p.iter().find_map(|(i, v)| match v {
        Value::Blob(b) if *i == id => Some(b.as_slice()),
        _ => None,
    })
}

/// The colour space and whether the last channel is opacity, from the subimage colour blob
/// (subimages, channels, then per channel the space in the high bits and the channel in the
/// low 15).
fn colour(b: &[u8]) -> Option<(Space, usize, bool)> {
    let n = u32le(b, 4)? as usize;
    if !(1..=4).contains(&n) {
        return None;
    }
    let first = u32le(b, 8)?;
    let space = match (first >> 16) & 0x7fff {
        1 => Space::Mono,
        2 => Space::PhotoYcc,
        3 => Space::NifRgb,
        // Opacity alone, or another space: shown as grey.
        _ => Space::Mono,
    };
    let last = u32le(b, 4 + 4 * n)?;
    let alpha = n > 1 && last & 0x7fff == 0x7ffe;
    Some((space, n, alpha))
}

struct Layout {
    width: u32,
    height: u32,
    space: Space,
    channels: usize,
    alpha: bool,
    /// Index of the largest resolution.
    level: u32,
    levels: u32,
}

fn layout(cfb: &specular_cfb::Cfb) -> Result<(Layout, Vec<(u32, Value)>)> {
    let bad = || DecodeError::Malformed("FlashPix image contents".into());
    let props = cfb
        .read(CONTENTS)
        .ok()
        .and_then(|d| properties(&d))
        .ok_or_else(bad)?;
    let levels = int(&props, 0x0100_0000)
        .filter(|&n| (1..=32).contains(&n))
        .ok_or_else(bad)?;
    let level = levels - 1;
    let id = |k: u32| 0x0200_0000 | (level << 16) | k;
    let width = int(&props, id(0)).ok_or_else(bad)?;
    let height = int(&props, id(1)).ok_or_else(bad)?;
    let (space, channels, alpha) = blob(&props, id(2)).and_then(colour).ok_or_else(bad)?;
    if width == 0 || height == 0 || width > 1 << 16 || height > 1 << 16 {
        return Err(bad());
    }
    Ok((
        Layout {
            width,
            height,
            space,
            channels,
            alpha,
            level,
            levels,
        },
        props,
    ))
}

impl Decoder for FpxDecoder {
    fn format(&self) -> Format {
        Format::Fpx
    }
    fn probe(&self, _head: &[u8]) -> bool {
        false
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        specular_cfb::sniff(data)
            && specular_cfb::Cfb::open(data).is_ok_and(|c| c.exists(CONTENTS) && c.exists(STORE))
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["fpx"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let cfb = specular_cfb::Cfb::open(source.bytes())
            .map_err(|_| DecodeError::Malformed("not a FlashPix compound file".into()))?;
        let (l, _) = layout(&cfb)?;
        let model = match (l.space, l.alpha) {
            (Space::Mono, false) => "Greyscale",
            (Space::Mono, true) => "Greyscale + opacity",
            (Space::PhotoYcc, false) => "PhotoYCC",
            (Space::PhotoYcc, true) => "PhotoYCC + opacity",
            (Space::NifRgb, false) => "RGB",
            (Space::NifRgb, true) => "RGB + opacity",
        };
        let info = ImageInfo {
            format: Format::Fpx,
            width: l.width,
            height: l.height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: l.alpha,
            bit_depth: 8,
            progressive: false,
            coding: format!("FlashPix, {model}, {} resolutions", l.levels),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(Arc::new(FpxSession {
            source,
            info,
            model,
            levels: Levels::new(),
        }))
    }
}

struct FpxSession {
    source: Arc<Source>,
    info: ImageInfo,
    model: &'static str,
    levels: Levels,
}

/// PhotoYCC (8-bit, as FlashPix stores it) to RGB.
fn ycc_to_rgb(y: u8, c1: u8, c2: u8) -> [u8; 3] {
    let y = 1.3584 * y as f32;
    let c1 = 2.2179 * (c1 as f32 - 156.0);
    let c2 = 1.8215 * (c2 as f32 - 137.0);
    let px = |v: f32| v.round().clamp(0.0, 255.0) as u8;
    [px(y + c2), px(y - 0.194 * c1 - 0.509 * c2), px(y + c1)]
}

/// One tile as `tw x th x channels` interleaved bytes. `cw`: the tile's width inside the
/// picture (uncompressed tiles at the right edge store only those columns).
fn tile(
    data: &[u8],
    (comp, param): (u32, u32),
    (tw, th, n, cw): (usize, usize, usize, usize),
    tables: impl Fn(u32) -> Option<Vec<u8>>,
) -> Result<Vec<u8>> {
    let size = tw * th * n;
    match comp {
        0 => {
            let cw = cw.clamp(1, tw);
            let mut t = vec![0u8; size];
            for (y, row) in data.chunks(cw * n).take(th).enumerate() {
                t[y * tw * n..][..row.len()].copy_from_slice(row);
            }
            Ok(t)
        }
        // One colour: the parameter holds the channels' values.
        1 => {
            let c = param.to_le_bytes();
            Ok((0..tw * th).flat_map(|_| c[..n].to_vec()).collect())
        }
        2 => {
            if n == 4 {
                return Err(DecodeError::Unsupported(
                    "FlashPix JPEG tiles with four channels".into(),
                ));
            }
            let p = param.to_le_bytes();
            let convert = p[2] != 0;
            let mut stream = Vec::new();
            if p[3] != 0
                && let Some(t) = tables(p[3] as u32)
            {
                // The tables stream without its EOI, then the tile without its SOI.
                stream.extend_from_slice(t.strip_suffix(&[0xff, 0xd9]).unwrap_or(&t));
                stream.extend_from_slice(data.get(2..).unwrap_or(&[]));
            } else {
                stream.extend_from_slice(data);
            }
            let planes = specular_jpeg::decode(&stream, &specular_jpeg::DecodeOptions::default())
                .map_err(crate::jpeg::map_err)?;
            let mut out = vec![0u8; size];
            if n == 3 && convert {
                let rgba = specular_jpeg::planes_to_rgba(&planes);
                let (w, h) = (planes.width as usize, planes.height as usize);
                for y in 0..th.min(h) {
                    for x in 0..tw.min(w) {
                        let s = (y * w + x) * 4;
                        out[(y * tw + x) * 3..][..3].copy_from_slice(&rgba[s..s + 3]);
                    }
                }
            } else {
                // The channels as stored, at their own resolution (repeated when subsampled).
                for (c, pl) in planes.planes.iter().take(n).enumerate() {
                    let (pw, ph) = (pl.width as usize, pl.height as usize);
                    for y in 0..th {
                        for x in 0..tw {
                            let sx = (x * pw / tw).min(pw.saturating_sub(1));
                            let sy = (y * ph / th).min(ph.saturating_sub(1));
                            out[(y * tw + x) * n + c] =
                                pl.data.get(sy * pl.stride + sx).copied().unwrap_or(0);
                        }
                    }
                }
            }
            Ok(out)
        }
        _ => Err(DecodeError::Unsupported(format!(
            "FlashPix tile compression {comp}"
        ))),
    }
}

impl FpxSession {
    fn decode_full(&self, cancel: &CancelToken) -> Result<Raster> {
        let cfb = specular_cfb::Cfb::open(self.source.bytes())
            .map_err(|_| DecodeError::Malformed("FlashPix compound file".into()))?;
        let (l, props) = layout(&cfb)?;
        let dir = format!("{STORE}/Resolution {:04}/Subimage 0000", l.level);
        let bad = || DecodeError::Malformed("FlashPix subimage header".into());
        let head = cfb.read(&format!("{dir} Header")).map_err(|_| bad())?;
        let data = cfb.read(&format!("{dir} Data")).map_err(|_| bad())?;
        let f = |i: usize| u32le(&head, 28 + 4 * i).ok_or_else(bad);
        let (w, h, tiles) = (f(1)? as usize, f(2)? as usize, f(3)? as usize);
        let (tw, th, n) = (f(4)? as usize, f(5)? as usize, f(6)? as usize);
        let (table, entry) = (28 + f(7)? as usize, f(8)? as usize);
        if (w, h) != (l.width as usize, l.height as usize)
            || n != l.channels
            || !(1..=256).contains(&tw)
            || !(1..=256).contains(&th)
            || entry < 16
        {
            return Err(bad());
        }
        let across = w.div_ceil(tw);
        if tiles < across * h.div_ceil(th) {
            return Err(bad());
        }
        let tables = |i: u32| blob(&props, 0x0300_0001 | (i << 16)).map(<[u8]>::to_vec);
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(w * h * 4)
            .map_err(|_| DecodeError::TooLarge)?;
        rgba.resize(w * h * 4, 255);
        let mut truncated = false;
        for t in 0..across * h.div_ceil(th) {
            if cancel.is_cancelled() {
                return Err(DecodeError::Cancelled);
            }
            let e = table + t * entry;
            let (off, len) = (
                u32le(&head, e).ok_or_else(bad)?,
                u32le(&head, e + 4).ok_or_else(bad)?,
            );
            let comp = (
                u32le(&head, e + 8).ok_or_else(bad)?,
                u32le(&head, e + 12).ok_or_else(bad)?,
            );
            if off == u32::MAX || comp.0 == u32::MAX {
                truncated = true;
                continue;
            }
            // The data stream starts with a 28-byte header too; offsets count from its end.
            let start = 28 + off as usize;
            let src = data
                .get(start..start.saturating_add(len as usize).min(data.len()))
                .unwrap_or(&[]);
            let (x0, y0) = ((t % across) * tw, (t / across) * th);
            let px = match tile(src, comp, (tw, th, n, w - x0), tables) {
                Ok(p) => p,
                Err(e @ DecodeError::Unsupported(_)) => return Err(e),
                Err(_) => {
                    truncated = true;
                    continue;
                }
            };
            for y in 0..th.min(h - y0) {
                for x in 0..tw.min(w - x0) {
                    let s = &px[(y * tw + x) * n..][..n];
                    let a = if l.alpha { s[n - 1] } else { 255 };
                    let c = if l.alpha { &s[..n - 1] } else { s };
                    let mut rgb = match (l.space, c.len()) {
                        (Space::PhotoYcc, 3) => ycc_to_rgb(c[0], c[1], c[2]),
                        (_, 3) => [c[0], c[1], c[2]],
                        (_, _) => [c[0]; 3],
                    };
                    // Stored premultiplied by opacity.
                    if l.alpha && a > 0 && a < 255 {
                        for v in &mut rgb {
                            *v = ((*v as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
                        }
                    }
                    let o = ((y0 + y) * w + x0 + x) * 4;
                    rgba[o..o + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], a]);
                }
            }
        }
        Ok(whole::interleaved(
            w as u32,
            h as u32,
            PixelLayout::Rgba,
            rgba,
            truncated,
        ))
    }
}

impl Session for FpxSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(self.model)
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| self.decode_full(cancel),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        let s = req.scale.max(1);
        let mut m = whole::level_bytes(w, h, s, 4);
        if !self.levels.has(s) && !self.levels.has(1) {
            m += whole::level_bytes(w, h, 1, 4) + self.source.len() as u64;
        }
        m
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_blobs() {
        let b = |v: &[u32]| -> Vec<u8> { v.iter().flat_map(|x| x.to_le_bytes()).collect() };
        assert_eq!(
            colour(&b(&[1, 3, 0x0003_0000, 0x0003_0001, 0x0003_0002])),
            Some((Space::NifRgb, 3, false))
        );
        assert_eq!(
            colour(&b(&[
                1,
                4,
                0x8003_8000,
                0x8003_8001,
                0x8003_8002,
                0x8003_7ffe
            ])),
            Some((Space::NifRgb, 4, true))
        );
        assert_eq!(
            colour(&b(&[1, 1, 0x8001_0000])),
            Some((Space::Mono, 1, false))
        );
        assert_eq!(colour(&b(&[1, 9])), None);
    }

    #[test]
    fn photo_ycc_white_and_black() {
        assert_eq!(ycc_to_rgb(0, 156, 137), [0, 0, 0]);
        let w = ycc_to_rgb(188, 156, 137);
        assert!(w.iter().all(|&v| v >= 254), "{w:?}");
    }

    #[test]
    fn single_colour_and_short_tiles() {
        let t = tile(&[], (1, 0x00_30_20_10), (2, 2, 3, 2), |_| None).unwrap();
        assert_eq!(t, [0x10, 0x20, 0x30].repeat(4));
        // An edge tile stores only its picture columns.
        let t = tile(&[1, 2, 3, 4, 5, 6], (0, 0), (2, 2, 3, 1), |_| None).unwrap();
        assert_eq!(t, [1, 2, 3, 0, 0, 0, 4, 5, 6, 0, 0, 0]);
        let t = tile(&[9; 5], (0, 0), (2, 2, 3, 2), |_| None).unwrap();
        assert_eq!(t.len(), 12);
        assert!(tile(&[], (7, 0), (2, 2, 1, 2), |_| None).is_err());
    }
}
