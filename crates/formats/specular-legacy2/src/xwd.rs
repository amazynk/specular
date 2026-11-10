//! X Window Dump (`xwd`), X11 (file version 7) and X10 (file version 6).
//!
//! The header words are in the byte order of the machine that wrote the file; it is found by
//! reading the version in both orders. The pixels follow the image's own `byte_order`,
//! `bitmap_unit` and `bitmap_bit_order`:
//!
//! - **ZPixmap**: `bits_per_pixel` 1, 2, 4, 8, 16, 24 or 32 per pixel, rows `bytes_per_line`
//!   apart. Wider pixels are assembled in `byte_order`; 2- and 4-bit pixels take the high bits
//!   first in an MSB-first image; 1-bit pixels follow the bit order.
//! - **XYBitmap / XYPixmap**: one bit plane per depth bit, most significant plane first, each
//!   `height` rows of `bytes_per_line`; bits are read in `bitmap_unit`-sized units assembled in
//!   `byte_order`, bit 0 of a unit being the leftmost pixel when `bitmap_bit_order` is LSB-first.
//!
//! Pixel values become colours by the visual class: grey and colour-mapped classes look the
//! value up in the dump's colour map (StaticGray/GrayScale without one scale the value to
//! white); TrueColor extracts the red, green and blue masks and scales each to 8 bits;
//! DirectColor looks each extracted component up in the colour map's red, green and blue.
//! An XYBitmap without a colour map shows 1 bits black on white.
//!
//! X10 dumps (40-byte header) have no masks or units: XY format is one 16-bit-unit bit plane per
//! display plane, LSB first; Z format one byte per pixel (two above 8 planes), with a colour
//! map of `window_ncolors` 12-byte entries (pixel, red, green, blue) when the display has more
//! than one plane.

use rayon::prelude::*;

use crate::{Error, Image, Info, Model, Pixels, Result, check_dims, check_payload, u16_to_u8};

const X11_HEADER: usize = 100;
const X10_HEADER: usize = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fmt {
    XyBitmap,
    XyPixmap,
    ZPixmap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Visual {
    StaticGray,
    GrayScale,
    StaticColor,
    PseudoColor,
    TrueColor,
    DirectColor,
}

impl Visual {
    fn name(self) -> &'static str {
        match self {
            Visual::StaticGray => "StaticGray",
            Visual::GrayScale => "GrayScale",
            Visual::StaticColor => "StaticColor",
            Visual::PseudoColor => "PseudoColor",
            Visual::TrueColor => "TrueColor",
            Visual::DirectColor => "DirectColor",
        }
    }
}

struct Header {
    x10: bool,
    /// Header words are big-endian.
    be: bool,
    header_size: usize,
    format: Fmt,
    depth: u32,
    width: u32,
    height: u32,
    xoffset: u32,
    /// Image bytes most significant first.
    msb_bytes: bool,
    unit: u32,
    /// Bits most significant (leftmost pixel) first.
    msb_bits: bool,
    pad: u32,
    bpp: u32,
    bpl: usize,
    visual: Visual,
    masks: [u32; 3],
    bits_per_rgb: u32,
    ncolors: usize,
    colormap_entries: u32,
    window: (u32, u32, i32, i32, u32),
    name: String,
}

fn word(d: &[u8], i: usize, be: bool) -> Option<u32> {
    let b: [u8; 4] = d.get(i * 4..i * 4 + 4)?.try_into().ok()?;
    Some(if be {
        u32::from_be_bytes(b)
    } else {
        u32::from_le_bytes(b)
    })
}

fn half(d: &[u8], o: usize, be: bool) -> Option<u16> {
    let b: [u8; 2] = d.get(o..o + 2)?.try_into().ok()?;
    Some(if be {
        u16::from_be_bytes(b)
    } else {
        u16::from_le_bytes(b)
    })
}

/// (X10, big-endian) when the header looks like a dump in that order.
fn kind(head: &[u8]) -> Option<(bool, bool)> {
    for be in [true, false] {
        let (Some(size), Some(version)) = (word(head, 0, be), word(head, 1, be)) else {
            continue;
        };
        match version {
            7 if (X11_HEADER as u32..X11_HEADER as u32 + 4096).contains(&size) => {
                let fmt = word(head, 2, be)?;
                let depth = word(head, 3, be)?;
                if fmt <= 2
                    && (1..=32).contains(&depth)
                    && word(head, 13, be).is_none_or(|v| v <= 5)
                {
                    return Some((false, be));
                }
            }
            6 if (X10_HEADER as u32..X10_HEADER as u32 + 4096).contains(&size) => {
                let planes = word(head, 3, be)?;
                let fmt = word(head, 4, be)?;
                if fmt <= 1 && (1..=16).contains(&planes) {
                    return Some((true, be));
                }
            }
            _ => {}
        }
    }
    None
}

/// The header size and version 7 (X11) or 6 (X10) in either byte order, with a known pixmap
/// format and depth.
pub fn probe(head: &[u8]) -> bool {
    kind(head).is_some()
}

fn name_of(d: &[u8], from: usize, to: usize) -> String {
    let b = d.get(from..to.min(d.len())).unwrap_or(&[]);
    let b = &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())];
    String::from_utf8_lossy(b).trim().to_string()
}

fn header(d: &[u8]) -> Result<Header> {
    let (x10, be) = kind(d).ok_or(Error::NotRecognised)?;
    let w = |i| word(d, i, be).ok_or_else(|| Error::Truncated("header".into()));
    let header_size = w(0)? as usize;
    if x10 {
        if d.len() < X10_HEADER {
            return Err(Error::Truncated("header".into()));
        }
        let planes = w(3)?;
        let z = w(4)? == 1;
        let (width, height) = (w(5)?, w(6)?);
        let hw = |o| half(d, o, be).unwrap_or(0);
        let ncolors = hw(38) as usize;
        let window = (
            hw(28) as u32,
            hw(30) as u32,
            hw(32) as i16 as i32,
            hw(34) as i16 as i32,
            hw(36) as u32,
        );
        let bpp = if !z {
            1
        } else if planes > 8 {
            16
        } else {
            8
        };
        let bpl = if z {
            (width as usize) * (bpp as usize / 8)
        } else {
            (width as usize).div_ceil(16) * 2
        };
        return Ok(Header {
            x10,
            be,
            header_size,
            format: if z {
                Fmt::ZPixmap
            } else if planes == 1 {
                Fmt::XyBitmap
            } else {
                Fmt::XyPixmap
            },
            depth: planes,
            width,
            height,
            xoffset: 0,
            msb_bytes: be,
            unit: 16,
            msb_bits: false,
            pad: 16,
            bpp,
            bpl,
            visual: if planes == 1 {
                Visual::StaticGray
            } else {
                Visual::PseudoColor
            },
            masks: [0; 3],
            bits_per_rgb: 16,
            ncolors: if planes > 1 { ncolors } else { 0 },
            colormap_entries: ncolors as u32,
            window,
            name: name_of(d, X10_HEADER, header_size),
        });
    }
    if d.len() < X11_HEADER {
        return Err(Error::Truncated("header".into()));
    }
    let format = match w(2)? {
        0 => Fmt::XyBitmap,
        1 => Fmt::XyPixmap,
        _ => Fmt::ZPixmap,
    };
    let visual = match w(13)? {
        0 => Visual::StaticGray,
        1 => Visual::GrayScale,
        2 => Visual::StaticColor,
        3 => Visual::PseudoColor,
        4 => Visual::TrueColor,
        _ => Visual::DirectColor,
    };
    Ok(Header {
        x10,
        be,
        header_size,
        format,
        depth: w(3)?,
        width: w(4)?,
        height: w(5)?,
        xoffset: w(6)?,
        msb_bytes: w(7)? != 0,
        unit: w(8)?,
        msb_bits: w(9)? != 0,
        pad: w(10)?,
        bpp: w(11)?,
        bpl: w(12)? as usize,
        visual,
        masks: [w(14)?, w(15)?, w(16)?],
        bits_per_rgb: w(17)?,
        colormap_entries: w(18)?,
        ncolors: w(19)? as usize,
        window: (w(20)?, w(21)?, w(22)? as i32, w(23)? as i32, w(24)?),
        name: name_of(d, X11_HEADER, header_size),
    })
}

/// Checks what the decoder can read and returns the bytes per plane row.
fn validate(h: &Header) -> Result<()> {
    check_dims(h.width as u64, h.height as u64)?;
    if h.xoffset > 4096 {
        return Err(Error::Malformed(format!("x offset {}", h.xoffset)));
    }
    match h.format {
        Fmt::ZPixmap => {
            if !matches!(h.bpp, 1 | 2 | 4 | 8 | 16 | 24 | 32) {
                return Err(Error::Unsupported(format!(
                    "X window dumps with {} bits per pixel are not supported",
                    h.bpp
                )));
            }
            if h.depth > h.bpp {
                return Err(Error::Malformed(format!(
                    "depth {} in {}-bit pixels",
                    h.depth, h.bpp
                )));
            }
        }
        _ => {
            if !matches!(h.unit, 8 | 16 | 32) {
                return Err(Error::Unsupported(format!(
                    "X window dumps with {}-bit bitmap units are not supported",
                    h.unit
                )));
            }
            if h.format == Fmt::XyBitmap && h.depth != 1 {
                return Err(Error::Malformed(format!("XYBitmap of depth {}", h.depth)));
            }
        }
    }
    let bits_needed = (h.xoffset as u64 + h.width as u64)
        * if h.format == Fmt::ZPixmap {
            h.bpp as u64
        } else {
            1
        };
    if (h.bpl as u64) * 8 < bits_needed {
        return Err(Error::Malformed(format!(
            "{} bytes per line for {} pixels",
            h.bpl, h.width
        )));
    }
    if h.ncolors > 65536 {
        return Err(Error::Malformed(format!("{} colours", h.ncolors)));
    }
    Ok(())
}

fn grey_visual(v: Visual) -> bool {
    matches!(v, Visual::StaticGray | Visual::GrayScale)
}

fn format_name(f: Fmt) -> &'static str {
    match f {
        Fmt::XyBitmap => "XYBitmap",
        Fmt::XyPixmap => "XYPixmap",
        Fmt::ZPixmap => "ZPixmap",
    }
}

pub fn info(data: &[u8], _page: u32) -> Result<Info> {
    let h = header(data)?;
    validate(&h)?;
    let model = if h.depth == 1 && (h.ncolors == 0 || grey_visual(h.visual)) {
        Model::Bilevel
    } else if grey_visual(h.visual) {
        Model::Grey
    } else if matches!(h.visual, Visual::StaticColor | Visual::PseudoColor) {
        Model::Palette
    } else {
        Model::Rgb
    };
    let coding = if h.x10 {
        format!(
            "X10, {}-plane {}",
            h.depth,
            if h.format == Fmt::ZPixmap {
                "Z format"
            } else {
                "XY format"
            }
        )
    } else {
        format!(
            "{}-bit {}, {}, {} first",
            h.depth,
            h.visual.name(),
            format_name(h.format),
            if h.msb_bytes { "MSB" } else { "LSB" }
        )
    };
    let mut fields = vec![
        (
            "File version".to_string(),
            if h.x10 { "6 (X10)" } else { "7 (X11)" }.to_string(),
        ),
        (
            "Header byte order".into(),
            if h.be { "big-endian" } else { "little-endian" }.into(),
        ),
        ("Pixmap format".into(), format_name(h.format).into()),
        ("Depth".into(), h.depth.to_string()),
        ("Visual class".into(), h.visual.name().into()),
    ];
    if !h.x10 {
        fields.extend([
            ("Bits per pixel".to_string(), h.bpp.to_string()),
            ("Bytes per line".into(), h.bpl.to_string()),
            (
                "Image byte order".into(),
                if h.msb_bytes {
                    "MSB first"
                } else {
                    "LSB first"
                }
                .into(),
            ),
            ("Bitmap unit".into(), h.unit.to_string()),
            (
                "Bitmap bit order".into(),
                if h.msb_bits { "MSB first" } else { "LSB first" }.into(),
            ),
            ("Bitmap pad".into(), h.pad.to_string()),
            ("Bits per RGB".into(), h.bits_per_rgb.to_string()),
            ("Colour map entries".into(), h.colormap_entries.to_string()),
        ]);
        if matches!(h.visual, Visual::TrueColor | Visual::DirectColor) {
            fields.push((
                "Masks".into(),
                format!(
                    "red {:#x}, green {:#x}, blue {:#x}",
                    h.masks[0], h.masks[1], h.masks[2]
                ),
            ));
        }
        if h.xoffset != 0 {
            fields.push(("X offset".into(), h.xoffset.to_string()));
        }
    }
    fields.push(("Colours".into(), h.ncolors.to_string()));
    let (ww, wh, wx, wy, bw) = h.window;
    fields.push(("Window".into(), format!("{ww} × {wh} at {wx}, {wy}")));
    if bw != 0 {
        fields.push(("Window border".into(), bw.to_string()));
    }
    let mut summary = Vec::new();
    if !h.name.is_empty() {
        fields.push(("Window name".into(), h.name.clone()));
        summary.push(("window", h.name.clone()));
    }
    Ok(Info {
        width: h.width,
        height: h.height,
        pages: 1,
        bit_depth: h.depth.min(32) as u8,
        model,
        has_alpha: false,
        orientation: 1,
        coding,
        summary,
        fields,
    })
}

/// The colour map: pixel value → (r, g, b) 16-bit.
struct Cmap {
    /// Indexed by pixel value where the entries' pixel values are small, else by position.
    by_pixel: Vec<Option<[u16; 3]>>,
}

fn colormap(d: &[u8], h: &Header, at: usize) -> Cmap {
    let entries: Vec<(usize, [u16; 3])> = (0..h.ncolors)
        .map_while(|i| {
            let o = at + i * 12;
            let e = d.get(o..o + 12)?;
            let pixel = word(e, 0, h.be).unwrap_or(0) as usize;
            let rgb = [
                half(e, 4, h.be).unwrap_or(0),
                half(e, 6, h.be).unwrap_or(0),
                half(e, 8, h.be).unwrap_or(0),
            ];
            Some((pixel, rgb))
        })
        .collect();
    // Two entries claiming one pixel value (screen dumps whose entries' pixel fields are all
    // 0): the pixel fields mean nothing, so the whole table goes by position, as ImageMagick
    // reads it; indexing by pixel would drop all but the last entry.
    let mut seen = std::collections::HashSet::new();
    let by_position = !entries.iter().all(|&(p, _)| seen.insert(p));
    let mut by_pixel: Vec<Option<[u16; 3]>> = Vec::new();
    for (i, &(pixel, rgb)) in entries.iter().enumerate() {
        // Pixel values beyond the table size (and a few wild ones) fall back to position.
        let key = if !by_position && pixel < h.ncolors.max(256) * 2 && pixel < 1 << 16 {
            pixel
        } else {
            i
        };
        if by_pixel.len() <= key {
            by_pixel.resize(key + 1, None);
        }
        by_pixel[key] = Some(rgb);
    }
    Cmap { by_pixel }
}

impl Cmap {
    fn get(&self, p: u32) -> Option<[u16; 3]> {
        self.by_pixel.get(p as usize).copied().flatten()
    }
}

/// Reads bit `x` of a row of `unit`-sized units.
#[inline]
fn bit(row: &[u8], x: usize, unit: usize, msb_bytes: bool, msb_bits: bool) -> u32 {
    let ubytes = unit / 8;
    let u = x / unit;
    let b = x % unit;
    // Bit significance within the unit: LSB-first means pixel 0 is bit 0.
    let sig = if msb_bits { unit - 1 - b } else { b };
    // Byte holding bit `sig` of the unit value.
    let byte_in_unit = if msb_bytes {
        ubytes - 1 - sig / 8
    } else {
        sig / 8
    };
    let v = row.get(u * ubytes + byte_in_unit).copied().unwrap_or(0);
    ((v >> (sig % 8)) & 1) as u32
}

/// Pixel values of row `y` into `vals`.
fn row_values(d: &[u8], h: &Header, start: usize, y: usize, vals: &mut [u32]) -> bool {
    let w = h.width as usize;
    let x0 = h.xoffset as usize;
    match h.format {
        Fmt::ZPixmap => {
            let Some(row) = d.get(start + y * h.bpl..start + (y + 1) * h.bpl) else {
                return false;
            };
            let unit = if matches!(h.unit, 8 | 16 | 32) {
                h.unit as usize
            } else {
                8
            };
            match h.bpp {
                1 => {
                    for (x, v) in vals.iter_mut().enumerate() {
                        *v = bit(row, x0 + x, unit, h.msb_bytes, h.msb_bits);
                    }
                }
                2 | 4 => {
                    // Several pixels a byte: the first in the high bits of an MSB-first image.
                    let bpp = h.bpp as usize;
                    let per = 8 / bpp;
                    let mask = (1u8 << bpp) - 1;
                    for (x, v) in vals.iter_mut().enumerate() {
                        let p = x0 + x;
                        let b = row.get(p / per).copied().unwrap_or(0);
                        let i = p % per;
                        let shift = if h.msb_bytes {
                            8 - bpp * (i + 1)
                        } else {
                            bpp * i
                        };
                        *v = ((b >> shift) & mask) as u32;
                    }
                }
                8 => {
                    for (v, &b) in vals.iter_mut().zip(&row[x0..x0 + w]) {
                        *v = b as u32;
                    }
                }
                _ => {
                    let n = h.bpp as usize / 8;
                    for (v, p) in vals
                        .iter_mut()
                        .zip(row[x0 * n..(x0 + w) * n].chunks_exact(n))
                    {
                        *v = if h.msb_bytes {
                            p.iter().fold(0u32, |a, &b| (a << 8) | b as u32)
                        } else {
                            p.iter().rev().fold(0u32, |a, &b| (a << 8) | b as u32)
                        };
                    }
                }
            }
            true
        }
        Fmt::XyBitmap | Fmt::XyPixmap => {
            let planes = h.depth as usize;
            let plane_bytes = h.bpl * h.height as usize;
            vals.fill(0);
            for p in 0..planes {
                let o = start + p * plane_bytes + y * h.bpl;
                let Some(row) = d.get(o..o + h.bpl) else {
                    return false;
                };
                let shift = planes - 1 - p;
                for (x, v) in vals.iter_mut().enumerate() {
                    *v |= bit(row, x0 + x, h.unit as usize, h.msb_bytes, h.msb_bits) << shift;
                }
            }
            true
        }
    }
}

/// Mask → (shift, bits).
fn mask_shape(m: u32) -> (u32, u32) {
    if m == 0 {
        return (0, 0);
    }
    let shift = m.trailing_zeros();
    (shift, (m >> shift).trailing_ones())
}

/// Scale a `bits`-bit value to 8 bits.
fn scale8(v: u32, bits: u32) -> u8 {
    match bits {
        0 => 0,
        8 => v as u8,
        b if b > 8 => (v >> (b - 8)) as u8,
        b => {
            let max = (1u32 << b) - 1;
            ((v * 255 + max / 2) / max) as u8
        }
    }
}

pub fn decode(data: &[u8], _page: u32) -> Result<Image> {
    let h = header(data)?;
    validate(&h)?;
    let cmap_at = h.header_size;
    let start = cmap_at + h.ncolors * 12;
    let cmap = colormap(data, &h, cmap_at);
    let planes = if h.format == Fmt::ZPixmap {
        1
    } else {
        h.depth as usize
    };
    let need = (h.bpl as u64) * h.height as u64 * planes as u64;
    check_payload(data.len().saturating_sub(start), need, 1)?;
    let (w, ht) = (h.width as usize, h.height as usize);
    let grey = grey_visual(h.visual) || (h.depth == 1 && h.ncolors == 0);
    let channels = if grey { 1 } else { 4 };
    let mut out = vec![crate::GREY; w * ht * channels];
    if channels == 4 {
        for px in out.chunks_exact_mut(4) {
            px[3] = 255;
        }
    }
    let shapes = h.masks.map(mask_shape);
    let maxv = if h.depth >= 32 {
        u32::MAX
    } else {
        (1u32 << h.depth) - 1
    };
    let truncated = std::sync::atomic::AtomicBool::new(false);
    out.par_chunks_mut(w * channels).enumerate().for_each_init(
        || vec![0u32; w],
        |vals, (y, o)| {
            if !row_values(data, &h, start, y, vals) {
                truncated.store(true, std::sync::atomic::Ordering::Relaxed);
                return;
            }
            let vals = &*vals;
            if grey {
                for (dst, &v) in o.iter_mut().zip(vals) {
                    *dst = if h.depth == 1 && h.format == Fmt::XyBitmap && h.ncolors == 0 {
                        // A bitmap: 1 bits are the foreground (black).
                        if v != 0 { 0 } else { 255 }
                    } else if let Some(c) = cmap.get(v) {
                        // Grey visuals: the map's luminance (its components are equal).
                        u16_to_u8(
                            ((c[0] as u32 * 299 + c[1] as u32 * 587 + c[2] as u32 * 114 + 500)
                                / 1000) as u16,
                        )
                    } else {
                        scale8(v.min(maxv), h.depth)
                    };
                }
                return;
            }
            for (px, &v) in o.chunks_exact_mut(4).zip(vals) {
                let rgb = match h.visual {
                    Visual::TrueColor | Visual::DirectColor if h.masks != [0; 3] => {
                        let comp = |i: usize| (v & h.masks[i]) >> shapes[i].0;
                        if h.visual == Visual::DirectColor && h.ncolors > 0 {
                            let look = |i: usize| {
                                cmap.get(comp(i))
                                    .map(|c| u16_to_u8(c[i]))
                                    .unwrap_or_else(|| scale8(comp(i), shapes[i].1))
                            };
                            [look(0), look(1), look(2)]
                        } else {
                            [
                                scale8(comp(0), shapes[0].1),
                                scale8(comp(1), shapes[1].1),
                                scale8(comp(2), shapes[2].1),
                            ]
                        }
                    }
                    _ => match cmap.get(v) {
                        Some(c) => [u16_to_u8(c[0]), u16_to_u8(c[1]), u16_to_u8(c[2])],
                        None => {
                            let g = scale8(v.min(maxv), h.depth);
                            [g, g, g]
                        }
                    },
                };
                px[..3].copy_from_slice(&rgb);
            }
        },
    );
    let truncated = truncated.into_inner();
    Ok(Image {
        width: h.width,
        height: h.height,
        pixels: if grey {
            Pixels::Gray8(out)
        } else {
            Pixels::Rgba8(out)
        },
        truncated,
    })
}
