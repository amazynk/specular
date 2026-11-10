//! Windows/OS2 bitmaps (BMP, DIB), also used for the BMP entries of ICO/CUR files.
//!
//! Supported: core (OS/2 1.x), OS/2 2.x (without Huffman/RLE24), BITMAPINFOHEADER and V2–V5
//! headers; 1/2/4/8-bit palettes, RLE4/RLE8, 16/24/32-bit with default or explicit bit fields
//! (including alpha), top-down and bottom-up rows. Embedded JPEG/PNG (compression 4/5) and
//! 64-bit BMPs are reported as unsupported.
//!
//! Uncompressed rows have a fixed stride, so regions are decoded straight from the file in
//! parallel; RLE data is expanded once into a full-resolution index buffer.
//!
//! Alpha: 32-bit pixels carry alpha only with an explicit alpha mask (V3+ headers or
//! ALPHABITFIELDS) or inside an icon, and only if some alpha value is non-zero (many writers
//! leave the byte at 0). Icons without alpha use the 1-bit AND mask.

use crate::px::{expand, expand_bits, rgb_to_rgba, rgba_to_rgba};
use crate::scale::{Plan, Rows, fill_grey, render};
use crate::{Cancel, Decode, Error, Image, Info, Options, PixelFormat, Result, u16le, u32le};

pub fn probe(head: &[u8]) -> bool {
    // "BM", then a DIB header size we know at offset 14 (when the head is long enough).
    head.len() >= 2
        && head.starts_with(b"BM")
        && (head.len() < 18
            || matches!(
                u32le(head, 14),
                Some(12 | 16 | 40 | 52 | 56 | 64 | 108 | 124)
            ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Comp {
    Rgb,
    Rle8,
    Rle4,
    Fields,
}

#[derive(Clone, Copy, Debug)]
struct Field {
    mask: u32,
    shift: u32,
    max: u32,
}

impl Field {
    fn new(mask: u32) -> Field {
        if mask == 0 {
            return Field {
                mask: 0,
                shift: 0,
                max: 0,
            };
        }
        let shift = mask.trailing_zeros();
        Field {
            mask,
            shift,
            max: mask >> shift,
        }
    }
    #[inline(always)]
    fn get(&self, v: u32, default: u8) -> u8 {
        if self.max == 0 {
            return default;
        }
        let x = ((v & self.mask) >> self.shift) as u64;
        ((x * 255 + self.max as u64 / 2) / self.max as u64) as u8
    }
}

#[derive(Clone, Debug)]
pub struct Bmp {
    info: Info,
    top_down: bool,
    pub(crate) bpp: u32,
    comp: Comp,
    palette: Box<[[u8; 4]; 256]>,
    fields: [Field; 4],
    /// Use the alpha channel of 32-bit (or bit-field) pixels.
    use_alpha: bool,
    pixels: usize,
    stride: usize,
    /// Bytes of compressed data available (RLE).
    data_end: usize,
    /// Icon AND mask: offset and stride.
    and_mask: Option<(usize, usize)>,
    /// RLE data leaves some pixels undefined: they decode transparent.
    rle_holes: bool,
}

impl Bmp {
    /// Parse a packed DIB (header, colour table, then the pixels, no file header), as Windows
    /// metafiles and the clipboard store bitmaps. The pixels start at `pixels` when given,
    /// else right after the colour table.
    pub fn parse_packed_dib(data: &[u8], pixels: Option<usize>) -> Result<Bmp> {
        Bmp::parse_dib(data, 0, data.len(), pixels, false)
    }

    /// Parse a BMP file.
    pub fn parse(data: &[u8]) -> Result<Bmp> {
        if data.len() < 18 {
            return Err(Error::Truncated("BMP header".into()));
        }
        if !data.starts_with(b"BM") {
            return Err(Error::Malformed("not a BMP file".into()));
        }
        let off = u32le(data, 10).unwrap_or(0) as usize;
        Bmp::parse_dib(data, 14, data.len(), Some(off), false)
    }

    /// Parse a DIB at `start` (header first) whose data ends at `end`. `pixel_offset` is the
    /// file header's offset (BMP); icons store the pixels right after the palette and the
    /// height counts the AND mask too.
    pub(crate) fn parse_dib(
        data: &[u8],
        start: usize,
        end: usize,
        pixel_offset: Option<usize>,
        icon: bool,
    ) -> Result<Bmp> {
        let end = end.min(data.len());
        let d = data.get(..end).unwrap_or(data);
        let trunc = || Error::Truncated("DIB header".into());
        let hs = u32le(d, start).ok_or_else(trunc)? as usize;
        if !matches!(hs, 12 | 16 | 40 | 52 | 56 | 64 | 108 | 124) {
            return Err(Error::Malformed(format!("DIB header size {hs}")));
        }
        if d.len() < start + hs.min(64) {
            return Err(trunc());
        }
        let core = hs == 12;
        let os2 = matches!(hs, 16 | 64);
        let (w, h, bpp, comp_raw, colors_used) = if core {
            (
                u16le(d, start + 4).ok_or_else(trunc)? as i64,
                u16le(d, start + 6).ok_or_else(trunc)? as i16 as i64,
                u16le(d, start + 10).ok_or_else(trunc)? as u32,
                0,
                0,
            )
        } else {
            (
                u32le(d, start + 4).ok_or_else(trunc)? as i32 as i64,
                u32le(d, start + 8).ok_or_else(trunc)? as i32 as i64,
                u16le(d, start + 14).ok_or_else(trunc)? as u32,
                if hs >= 20 {
                    u32le(d, start + 16).ok_or_else(trunc)?
                } else {
                    0
                },
                if hs >= 36 {
                    u32le(d, start + 32).ok_or_else(trunc)?
                } else {
                    0
                },
            )
        };
        let top_down = h < 0;
        let mut h = h.unsigned_abs();
        if icon {
            h /= 2;
        }
        if w <= 0 || h == 0 || w > u32::MAX as i64 || h > u32::MAX as u64 {
            return Err(Error::Malformed(format!("BMP size {w}×{h}")));
        }
        let (w, h) = (w as u32, h as u32);
        crate::check_dims(w, h)?;
        let comp = match (comp_raw, os2) {
            (0, _) => Comp::Rgb,
            (1, _) => Comp::Rle8,
            (2, _) => Comp::Rle4,
            (3 | 6, false) => Comp::Fields,
            (3, true) => return Err(Error::Unsupported("OS/2 Huffman BMP".into())),
            (4, true) => return Err(Error::Unsupported("OS/2 RLE24 BMP".into())),
            (4, false) => return Err(Error::Unsupported("JPEG in BMP".into())),
            (5, false) => return Err(Error::Unsupported("PNG in BMP".into())),
            (c, _) => return Err(Error::Malformed(format!("BMP compression {c}"))),
        };
        let ok = match comp {
            Comp::Rgb => matches!(bpp, 1 | 2 | 4 | 8 | 16 | 24 | 32),
            Comp::Rle8 => bpp == 8,
            Comp::Rle4 => bpp == 4,
            Comp::Fields => matches!(bpp, 16 | 32),
        };
        if !ok {
            return Err(if bpp == 64 {
                Error::Unsupported("64-bit BMP".into())
            } else {
                Error::Malformed(format!("BMP {bpp}-bit with compression {comp_raw}"))
            });
        }
        if matches!(comp, Comp::Rle4 | Comp::Rle8) && top_down {
            return Err(Error::Malformed("top-down RLE BMP".into()));
        }
        // Bit fields: in the header (V2+), or 12/16 bytes after a 40-byte header.
        let mut p = start + hs;
        let mut masks = [0u32; 4];
        if comp == Comp::Fields {
            let n = if comp_raw == 6 { 4 } else { 3 };
            if hs >= 52 {
                for (i, m) in masks
                    .iter_mut()
                    .enumerate()
                    .take(if hs >= 56 { 4 } else { 3 })
                {
                    *m = u32le(d, start + 40 + i * 4).ok_or_else(trunc)?;
                }
            } else {
                for (i, m) in masks.iter_mut().enumerate().take(n) {
                    *m = u32le(d, p + i * 4).ok_or_else(trunc)?;
                }
                p += n * 4;
            }
        } else if bpp == 16 {
            masks = [0x7c00, 0x03e0, 0x001f, 0];
        } else if bpp == 32 {
            masks = [0xff0000, 0xff00, 0xff, 0];
            // V3+ headers can declare alpha even for BI_RGB.
            if hs >= 56 {
                masks[3] = u32le(d, start + 52).unwrap_or(0);
            }
            if icon {
                masks[3] = 0xff000000;
            }
        }
        // Palette.
        let mut palette = Box::new([[0u8, 0, 0, 255]; 256]);
        let mut pal_n = 0;
        if bpp <= 8 {
            let max = 1usize << bpp;
            pal_n = if colors_used == 0 || colors_used as usize > max {
                max
            } else {
                colors_used as usize
            };
            let es = if core { 3 } else { 4 };
            for (i, e) in palette.iter_mut().enumerate().take(pal_n) {
                if let Some(c) = d.get(p + i * es..p + i * es + 3) {
                    *e = [c[2], c[1], c[0], 255];
                }
            }
            p += pal_n * es;
        }
        let pixels = match pixel_offset {
            Some(o) if o >= start + hs && o < d.len() => o,
            _ => p,
        };
        let stride = ((w as usize * bpp as usize).div_ceil(32)) * 4;
        let fields = masks.map(Field::new);
        let mut has_alpha = false;
        let mut use_alpha = false;
        let mut and_mask = None;
        if bpp == 32 && masks[3] != 0 {
            // Honour alpha only if it's used (all zero is an unset channel). Report it only if
            // some pixel is less than opaque: the same pass, so the truth costs nothing.
            let f = fields[3];
            let full = stride * h as usize;
            let n = full.min(d.len().saturating_sub(pixels));
            let (mut any, mut below) = (false, false);
            for px in d.get(pixels..pixels + n).unwrap_or(&[]).chunks_exact(4) {
                let a = f.get(u32::from_le_bytes([px[0], px[1], px[2], px[3]]), 0);
                any |= a != 0;
                below |= a != 255;
                if any && below {
                    break;
                }
            }
            use_alpha = any;
            // Missing (truncated) pixels decode transparent.
            has_alpha = any && (below || n < full);
        } else if bpp == 16 && masks[3] != 0 {
            use_alpha = true;
            has_alpha = true;
        }
        if icon && !use_alpha {
            let as_ = (w as usize).div_ceil(32) * 4;
            let o = pixels + stride * h as usize;
            if o < d.len() {
                and_mask = Some((o, as_));
                // Transparent only where a mask bit inside the width is set (a few bytes).
                let tail = if w % 8 == 0 {
                    0xff
                } else {
                    !(0xffu8 >> (w % 8))
                };
                let used = (w as usize).div_ceil(8);
                has_alpha = (0..h as usize).any(|y| {
                    let row = d.get(o + y * as_..).unwrap_or(&[]);
                    let row = &row[..row.len().min(used)];
                    row.iter().enumerate().any(|(i, &b)| {
                        let keep = if i + 1 == used { tail } else { 0xff };
                        b & keep != 0
                    })
                });
            }
        }
        let coding = format!(
            "{}{}-bit{}{}",
            if icon { "" } else { "BMP " },
            bpp,
            match comp {
                Comp::Rgb if bpp <= 8 => format!(" palette ({pal_n})"),
                Comp::Rle8 | Comp::Rle4 => format!(" palette ({pal_n}), RLE"),
                Comp::Fields => " bit fields".into(),
                Comp::Rgb => String::new(),
            },
            if top_down { ", top-down" } else { "" }
        );
        let rle = matches!(comp, Comp::Rle4 | Comp::Rle8);
        // Pixels an RLE bitmap leaves undefined (skipped by a delta, an early end of line or
        // of the bitmap, or cut off) are transparent, as browsers show them and BMP Suite
        // prefers (q/pal8rlecut.bmp; round 8: they took palette colour 0).
        let rle_holes = rle && {
            let mut n = 0u64;
            walk_rle(
                d,
                pixels,
                w as usize,
                h as usize,
                comp == Comp::Rle4,
                &|| false,
                |_, _| n += 1,
            )
            .is_ok()
                && n < u64::from(w) * u64::from(h)
        };
        has_alpha |= rle_holes;
        Ok(Bmp {
            info: Info {
                width: w,
                height: h,
                has_alpha,
                alpha: if and_mask.is_some() {
                    crate::AlphaKind::Key
                } else {
                    crate::AlphaKind::Straight
                },
                gray: false,
                model: if bpp <= 8 {
                    crate::Model::Palette
                } else {
                    crate::Model::Rgb
                },
                // Per sample: the widest colour field of 16/32-bit pixels (5 for 5-5-5, 8
                // for plain 24/32-bit); a palette index as stored.
                bit_depth: if bpp <= 8 {
                    bpp as u8
                } else {
                    masks[..3]
                        .iter()
                        .map(|m| m.count_ones())
                        .max()
                        .filter(|&b| b > 0)
                        .unwrap_or(8) as u8
                },
                coding,
                random_access: !rle,
            },
            top_down,
            bpp,
            comp,
            palette,
            fields,
            use_alpha,
            pixels,
            stride,
            data_end: d.len(),
            and_mask,
            rle_holes,
        })
    }

    fn file_row(&self, y: u32) -> usize {
        if self.top_down {
            y as usize
        } else {
            (self.info.height - 1 - y) as usize
        }
    }

    /// Convert complete pixels `x0..x1` of a raw row.
    fn conv(&self, row: &[u8], x0: u32, x1: u32, out: &mut [u8]) {
        let (a, b) = (x0 as usize, x1 as usize);
        match self.bpp {
            1 | 2 | 4 => expand_bits(row, self.bpp, x0, x1, &self.palette, out),
            8 => expand(&row[a..b], &self.palette, out),
            24 => rgb_to_rgba(&row[a * 3..b * 3], out, true),
            32 if self.is_bgra() => rgba_to_rgba(&row[a * 4..b * 4], out, true, !self.use_alpha),
            32 => {
                let [r, g, bl, al] = self.fields;
                for (o, px) in out
                    .chunks_exact_mut(4)
                    .zip(row[a * 4..b * 4].chunks_exact(4))
                {
                    let v = u32::from_le_bytes([px[0], px[1], px[2], px[3]]);
                    o.copy_from_slice(&[
                        r.get(v, 0),
                        g.get(v, 0),
                        bl.get(v, 0),
                        if self.use_alpha { al.get(v, 255) } else { 255 },
                    ]);
                }
            }
            _ => {
                let [r, g, bl, al] = self.fields;
                for (o, px) in out
                    .chunks_exact_mut(4)
                    .zip(row[a * 2..b * 2].chunks_exact(2))
                {
                    let v = u16::from_le_bytes([px[0], px[1]]) as u32;
                    o.copy_from_slice(&[
                        r.get(v, 0),
                        g.get(v, 0),
                        bl.get(v, 0),
                        if self.use_alpha { al.get(v, 255) } else { 255 },
                    ]);
                }
            }
        }
    }

    fn is_bgra(&self) -> bool {
        let m = self.fields.map(|f| f.mask);
        m[0] == 0xff0000 && m[1] == 0xff00 && m[2] == 0xff && (m[3] == 0xff000000 || m[3] == 0)
    }

    fn apply_and_mask(&self, d: &[u8], y: u32, x0: u32, x1: u32, out: &mut [u8]) {
        let Some((o, s)) = self.and_mask else { return };
        let r = o + self.file_row(y) * s;
        for (x, px) in (x0..x1).zip(out.chunks_exact_mut(4)) {
            let byte = d.get(r + (x / 8) as usize).copied().unwrap_or(0);
            if byte & (0x80 >> (x % 8)) != 0 {
                px.fill(0);
            }
        }
    }

    /// Expand RLE data into a full-resolution index buffer (top row first). Returns whether
    /// the data ended before the end-of-bitmap code.
    /// The RLE pixels as palette indices (rows top-down) and, when some pixels are left
    /// undefined (skipped by a delta or an early end of line or bitmap, or cut off), which
    /// are painted (1); and whether the data ended early.
    fn decode_rle(&self, d: &[u8], cancel: Cancel) -> Result<(Vec<u8>, Option<Vec<u8>>, bool)> {
        let (w, h) = (self.info.width as usize, self.info.height as usize);
        let mut idx = vec![0u8; w * h];
        let mut painted = self.rle_holes.then(|| vec![0u8; w * h]);
        let four = self.comp == Comp::Rle4;
        let truncated = walk_rle(
            &d[..self.data_end],
            self.pixels,
            w,
            h,
            four,
            cancel,
            |i, v| {
                idx[i] = v;
                if let Some(m) = painted.as_mut() {
                    m[i] = 1;
                }
            },
        )?;
        Ok((idx, painted, truncated))
    }
}

/// Walks RLE4/RLE8 data from `p`, calling `put(i, index)` for each pixel inside the
/// `w`×`h` bitmap (`i` row-major, rows top-down). Each pixel is put at most once (deltas
/// and line ends only move forward). Returns whether the data ended early.
fn walk_rle(
    d: &[u8],
    mut p: usize,
    w: usize,
    h: usize,
    four: bool,
    cancel: Cancel,
    mut put: impl FnMut(usize, u8),
) -> Result<bool> {
    let (mut x, mut y) = (0usize, 0usize); // y counts file rows (bottom-up)
    let mut at = |x: usize, y: usize, v: u8| {
        if x < w && y < h {
            put((h - 1 - y) * w + x, v);
        }
    };
    let mut rows_done = 0usize;
    loop {
        if y >= h {
            return Ok(false);
        }
        if y != rows_done {
            rows_done = y;
            if y % 256 == 0 && cancel() {
                return Err(Error::Cancelled);
            }
        }
        let (Some(&n), Some(&c)) = (d.get(p), d.get(p + 1)) else {
            return Ok(true);
        };
        p += 2;
        if n > 0 {
            for i in 0..n as usize {
                let v = if four {
                    if i % 2 == 0 { c >> 4 } else { c & 15 }
                } else {
                    c
                };
                at(x, y, v);
                x += 1;
            }
            continue;
        }
        match c {
            0 => {
                x = 0;
                y += 1;
            }
            1 => return Ok(false),
            2 => {
                let (Some(&dx), Some(&dy)) = (d.get(p), d.get(p + 1)) else {
                    return Ok(true);
                };
                p += 2;
                x += dx as usize;
                y += dy as usize;
            }
            n => {
                let n = n as usize;
                let bytes = if four { n.div_ceil(2) } else { n };
                let Some(src) = d.get(p..p + bytes) else {
                    return Ok(true);
                };
                for i in 0..n {
                    let v = if four {
                        let b = src[i / 2];
                        if i % 2 == 0 { b >> 4 } else { b & 15 }
                    } else {
                        src[i]
                    };
                    at(x, y, v);
                    x += 1;
                }
                p += bytes.next_multiple_of(2);
            }
        }
    }
}

struct RawRows<'a> {
    d: &'a [u8],
    b: &'a Bmp,
}

impl Rows for RawRows<'_> {
    fn row(&self, y: u32, x0: u32, x1: u32, out: &mut [u8]) {
        let b = self.b;
        let s = b.pixels + b.file_row(y) * b.stride;
        let e = (s + b.stride).min(b.data_end);
        let row = self.d.get(s..e).unwrap_or(&[]);
        // Whole pixels present in this row.
        let n = ((row.len() * 8) / b.bpp as usize).min(b.info.width as usize) as u32;
        let x1c = x1.min(n).max(x0);
        let k = (x1c - x0) as usize * 4;
        // A row cut short before `x0` has no pixels to convert (and `row[x0..]` is out of range).
        if k > 0 {
            b.conv(row, x0, x1c, &mut out[..k]);
        }
        fill_grey(&mut out[k..], 4);
        b.apply_and_mask(self.d, y, x0, x1, out);
    }
}

struct IndexRows<'a> {
    idx: &'a [u8],
    /// Which pixels the RLE data paints (1); the others are transparent.
    painted: Option<&'a [u8]>,
    w: usize,
    b: &'a Bmp,
}

impl Rows for IndexRows<'_> {
    fn row(&self, y: u32, x0: u32, x1: u32, out: &mut [u8]) {
        let s = y as usize * self.w;
        let r = s + x0 as usize..s + x1 as usize;
        expand(&self.idx[r.clone()], &self.b.palette, out);
        if let Some(m) = self.painted {
            for (px, &p) in out.chunks_exact_mut(4).zip(&m[r]) {
                if p == 0 {
                    px.copy_from_slice(&[0; 4]);
                }
            }
        }
    }
}

impl Decode for Bmp {
    fn info(&self) -> &Info {
        &self.info
    }

    fn decode(&self, data: &[u8], opts: &Options, cancel: Cancel) -> Result<Image> {
        let (w, h) = (self.info.width, self.info.height);
        let plan = Plan::new(w, h, opts)?;
        let opaque = !self.info.has_alpha;
        let per_byte = match self.comp {
            // Runs of 255 and skips: generous.
            Comp::Rle4 | Comp::Rle8 => 1024,
            _ => (8 / u64::from(self.bpp).max(1)).max(1),
        };
        crate::proven(data.len(), w, h, per_byte)?;
        let (out, truncated) = match self.comp {
            Comp::Rle4 | Comp::Rle8 => {
                let (idx, painted, truncated) = self.decode_rle(data, cancel)?;
                let rows = IndexRows {
                    idx: &idx,
                    painted: painted.as_deref(),
                    w: w as usize,
                    b: self,
                };
                (
                    render(&rows, 4, opaque, &plan, opts.parallel, cancel)?,
                    truncated,
                )
            }
            _ => {
                let rows = RawRows { d: data, b: self };
                let need = self.pixels as u64
                    + self.stride as u64 * (h as u64 - 1)
                    + (w as u64 * self.bpp as u64).div_ceil(8);
                (
                    render(&rows, 4, opaque, &plan, opts.parallel, cancel)?,
                    (self.data_end as u64) < need,
                )
            }
        };
        Ok(Image {
            width: plan.rw,
            height: plan.rh,
            format: PixelFormat::Rgba8,
            data: out,
            truncated,
        })
    }
}
