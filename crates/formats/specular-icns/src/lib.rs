//! Apple icon container (ICNS).
//!
//! An `.icns` file is a big-endian chunk list: the 8-byte header `icns` +
//! total length, then `[fourcc][u32 length incl. header][payload]` entries.
//! Each icon type fixes the pixel size; the payload is either
//!
//! * a PNG or JPEG 2000 stream (`ic07`–`ic14`, `icp4`–`icp6`, sometimes
//!   `ic04`/`ic05`) — located here, decoded by the PNG / JP2 decoders;
//! * `ARGB` + four run-length coded channels (`ic04`, `ic05`, `icsb`, …);
//! * legacy 24-bit RGB (`is32`, `il32`, `ih32`, `it32`), run-length coded
//!   per channel, with a separate 8-bit alpha mask (`s8mk`, `l8mk`, `h8mk`,
//!   `t8mk`);
//! * 1-bit black-and-white with a 1-bit mask (`ICN#`, `ics#`, `ich#`, `icm#`).
//!
//! Every icon in the file becomes one page, sorted largest first. Old 4- and
//! 8-bit palette icons, the table of contents, version, name and the nested
//! selected/template variants (`slct`, `sbtp`) are skipped.
//!
//! The run-length scheme: a control byte `n < 0x80` copies the next `n + 1`
//! bytes; `n >= 0x80` repeats the next byte `n - 0x80 + 3` times. Channels
//! are coded one after the other (R, G, B — or A, R, G, B after `ARGB`).
//!
//! Hostile input is bounded by construction: pixel sizes come from the fixed
//! type table (at most 1024×1024), never from the payload.

use std::fmt;
use std::ops::Range;

/// Upper bound on entries walked; real files have a few dozen.
pub const MAX_ENTRIES: usize = 4096;

/// What an entry's payload holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A complete PNG stream.
    Png,
    /// A JPEG 2000 stream (JP2 file or raw codestream).
    Jp2,
    /// `ARGB` + run-length coded A, R, G, B channels.
    Argb,
    /// Legacy 24-bit RGB (run-length coded, or raw interleaved when the
    /// length is exactly `w*h*3`) with an optional 8-bit mask entry.
    Rgb24,
    /// 1-bit image followed by a 1-bit mask (or no mask).
    Mono,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Icon {
    pub fourcc: [u8; 4],
    /// Pixel size the type promises. PNG / JPEG 2000 payloads carry their
    /// own size, which the decoder of that payload reports.
    pub width: u32,
    pub height: u32,
    /// 2 for Retina (`@2x`) variants.
    pub scale: u8,
    pub kind: Kind,
    /// Payload byte range in the file (after the 8-byte entry header and,
    /// for `it32`, the 4-byte zero prefix).
    pub data: Range<usize>,
    /// The 8-bit mask entry's payload for [`Kind::Rgb24`].
    pub mask: Option<Range<usize>>,
}

impl Icon {
    /// The fourcc as text (`ic10`, `is32`, …), lossy for odd bytes.
    pub fn name(&self) -> String {
        self.fourcc
            .iter()
            .map(|&b| if b.is_ascii_graphic() { b as char } else { '?' })
            .collect()
    }

    /// The page label: point size and `@2x` for Retina variants,
    /// e.g. `512×512 @2x` for a 1024-pixel `ic10`.
    pub fn label(&self) -> String {
        let s = u32::from(self.scale.max(1));
        if s > 1 {
            format!("{}×{} @{}x", self.width / s, self.height / s, s)
        } else {
            format!("{}×{}", self.width, self.height)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    NotIcns,
    Truncated,
    NoIcons,
    /// The payload is PNG / JPEG 2000 and must go to that decoder.
    Embedded,
    /// The payload doesn't match its type.
    Corrupt(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotIcns => f.write_str("not an ICNS file"),
            Error::Truncated => f.write_str("ICNS file is truncated"),
            Error::NoIcons => f.write_str("ICNS file holds no supported icon"),
            Error::Embedded => f.write_str("ICNS icon is a PNG or JPEG 2000 stream"),
            Error::Corrupt(why) => write!(f, "corrupt ICNS icon: {why}"),
        }
    }
}

impl std::error::Error for Error {}

/// Whether `head` starts like an ICNS file.
pub fn detect(head: &[u8]) -> bool {
    head.len() >= 8 && &head[..4] == b"icns"
}

enum Class {
    /// Payload decided by magic: PNG, JPEG 2000 or `ARGB`.
    Modern,
    Rgb24 {
        mask: [u8; 4],
        prefix: usize,
    },
    Mono,
}

/// (pixel width, pixel height, scale, class) of the supported types.
fn type_info(cc: &[u8; 4]) -> Option<(u32, u32, u8, Class)> {
    use Class::*;
    Some(match cc {
        b"ICN#" => (32, 32, 1, Mono),
        b"ICON" => (32, 32, 1, Mono),
        b"ics#" => (16, 16, 1, Mono),
        b"ich#" => (48, 48, 1, Mono),
        b"icm#" => (16, 12, 1, Mono),
        b"is32" => (
            16,
            16,
            1,
            Rgb24 {
                mask: *b"s8mk",
                prefix: 0,
            },
        ),
        b"il32" => (
            32,
            32,
            1,
            Rgb24 {
                mask: *b"l8mk",
                prefix: 0,
            },
        ),
        b"ih32" => (
            48,
            48,
            1,
            Rgb24 {
                mask: *b"h8mk",
                prefix: 0,
            },
        ),
        b"it32" => (
            128,
            128,
            1,
            Rgb24 {
                mask: *b"t8mk",
                prefix: 4,
            },
        ),
        b"ic04" | b"icp4" => (16, 16, 1, Modern),
        b"ic05" | b"icp5" => (32, 32, 1, Modern),
        b"icp6" => (64, 64, 1, Modern),
        b"ic07" => (128, 128, 1, Modern),
        b"ic08" => (256, 256, 1, Modern),
        b"ic09" => (512, 512, 1, Modern),
        b"ic10" => (1024, 1024, 2, Modern),
        b"ic11" => (32, 32, 2, Modern),
        b"ic12" => (64, 64, 2, Modern),
        b"ic13" => (256, 256, 2, Modern),
        b"ic14" => (512, 512, 2, Modern),
        b"icsb" => (18, 18, 1, Modern),
        b"icsB" => (36, 36, 2, Modern),
        b"sb24" => (24, 24, 1, Modern),
        b"SB24" => (48, 48, 2, Modern),
        _ => return None,
    })
}

const PNG_SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
const JP2_SIG: &[u8] = b"\x00\x00\x00\x0cjP  \r\n\x87\n";
const J2K_SIG: &[u8] = b"\xff\x4f\xff\x51";

/// Walks the entries, returning `(fourcc, payload range)` pairs. Stops at
/// the declared end (clamped to the data) or the first broken entry.
type EntryList = Vec<([u8; 4], Range<usize>)>;

fn entries(data: &[u8]) -> Result<(EntryList, bool), Error> {
    if data.len() < 8 || &data[..4] != b"icns" {
        return Err(
            if data.len() < 8 && b"icns".starts_with(&data[..data.len().min(4)]) {
                Error::Truncated
            } else {
                Error::NotIcns
            },
        );
    }
    let declared = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
    let end = if declared >= 8 {
        declared.min(data.len())
    } else {
        data.len()
    };
    let mut truncated = declared > data.len();
    let mut out = Vec::new();
    let mut pos = 8;
    while pos + 8 <= end && out.len() < MAX_ENTRIES {
        let cc = [data[pos], data[pos + 1], data[pos + 2], data[pos + 3]];
        let len = u32::from_be_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
            as usize;
        if len < 8 {
            break;
        }
        let stop = pos.saturating_add(len);
        if stop > end {
            // Keep what is there of the last entry; decoders cope with a
            // short payload.
            truncated = true;
            out.push((cc, pos + 8..end));
            break;
        }
        out.push((cc, pos + 8..stop));
        pos = stop;
    }
    Ok((out, truncated))
}

/// Parsed file: the icons, largest first.
#[derive(Clone, Debug, Default)]
pub struct Icns {
    pub icons: Vec<Icon>,
    /// The file ends before its declared length or mid-entry.
    pub truncated: bool,
}

/// Lists the supported icons, largest pixel size first; at equal size
/// 1x before 2x, then file order.
pub fn parse(data: &[u8]) -> Result<Icns, Error> {
    let (list, truncated) = entries(data)?;
    let find = |cc: &[u8; 4]| list.iter().find(|(c, _)| c == cc).map(|(_, r)| r.clone());
    let mut icons = Vec::new();
    for (cc, range) in &list {
        let Some((w, h, scale, class)) = type_info(cc) else {
            continue;
        };
        let payload = &data[range.clone()];
        let (kind, data_range, mask) = match class {
            Class::Modern => {
                let kind = if payload.starts_with(PNG_SIG) {
                    Kind::Png
                } else if payload.starts_with(JP2_SIG) || payload.starts_with(J2K_SIG) {
                    Kind::Jp2
                } else if payload.starts_with(b"ARGB") {
                    Kind::Argb
                } else {
                    continue;
                };
                (kind, range.clone(), None)
            }
            Class::Rgb24 { mask, prefix } => {
                let start = (range.start + prefix).min(range.end);
                (Kind::Rgb24, start..range.end, find(&mask))
            }
            Class::Mono => (Kind::Mono, range.clone(), None),
        };
        icons.push(Icon {
            fourcc: *cc,
            width: w,
            height: h,
            scale,
            kind,
            data: data_range,
            mask,
        });
    }
    if icons.is_empty() {
        return Err(if truncated {
            Error::Truncated
        } else {
            Error::NoIcons
        });
    }
    // Stable: equal keys keep file order.
    icons.sort_by_key(|i| {
        (
            std::cmp::Reverse(u64::from(i.width) * u64::from(i.height)),
            i.scale,
        )
    });
    Ok(Icns { icons, truncated })
}

/// An icon decoded here (everything but PNG / JPEG 2000).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    /// Straight (unpremultiplied) RGBA8, row-major.
    pub pixels: Vec<u8>,
    /// Some samples were missing and are filled (colour 0, mask opaque).
    pub truncated: bool,
}

/// Unpacks run-length coded bytes into `out`, which it fills completely;
/// returns how many bytes of `src` were used and whether it ran short.
fn unpack_rle(src: &[u8], out: &mut [u8]) -> (usize, bool) {
    let mut i = 0;
    let mut o = 0;
    while o < out.len() {
        let Some(&n) = src.get(i) else {
            return (i, true);
        };
        i += 1;
        if n < 0x80 {
            let count = usize::from(n) + 1;
            let avail = src.len() - i;
            let take = count.min(avail).min(out.len() - o);
            out[o..o + take].copy_from_slice(&src[i..i + take]);
            o += take;
            i += count.min(avail);
            if count > avail {
                return (i, o < out.len());
            }
        } else {
            let count = usize::from(n) - 0x80 + 3;
            let Some(&v) = src.get(i) else {
                return (i, true);
            };
            i += 1;
            let take = count.min(out.len() - o);
            out[o..o + take].fill(v);
            o += take;
        }
    }
    (i, false)
}

/// Decodes an [`Kind::Argb`], [`Kind::Rgb24`] or [`Kind::Mono`] icon of a
/// file previously given to [`parse`].
pub fn decode(data: &[u8], icon: &Icon) -> Result<Rgba, Error> {
    let (w, h) = (icon.width as usize, icon.height as usize);
    let n = w * h;
    let payload = data.get(icon.data.clone()).ok_or(Error::Truncated)?;
    let mut px = vec![0u8; n * 4];
    let mut truncated = false;
    match icon.kind {
        Kind::Png | Kind::Jp2 => return Err(Error::Embedded),
        Kind::Argb => {
            let body = payload.get(4..).ok_or(Error::Truncated)?;
            let mut planes = vec![0u8; n * 4];
            let (_, short) = unpack_rle(body, &mut planes);
            truncated |= short;
            for p in 0..n {
                px[p * 4] = planes[n + p];
                px[p * 4 + 1] = planes[2 * n + p];
                px[p * 4 + 2] = planes[3 * n + p];
                px[p * 4 + 3] = planes[p];
            }
        }
        Kind::Rgb24 => {
            if payload.len() == n * 3 {
                for (o, i) in px.chunks_exact_mut(4).zip(payload.chunks_exact(3)) {
                    o[..3].copy_from_slice(i);
                }
            } else {
                let mut planes = vec![0u8; n * 3];
                let (_, short) = unpack_rle(payload, &mut planes);
                truncated |= short;
                for p in 0..n {
                    px[p * 4] = planes[p];
                    px[p * 4 + 1] = planes[n + p];
                    px[p * 4 + 2] = planes[2 * n + p];
                }
            }
            match icon.mask.as_ref().and_then(|r| data.get(r.clone())) {
                Some(mask) => {
                    for p in 0..n {
                        px[p * 4 + 3] = match mask.get(p) {
                            Some(&a) => a,
                            None => {
                                truncated = true;
                                255
                            }
                        };
                    }
                }
                None => px.chunks_exact_mut(4).for_each(|o| o[3] = 255),
            }
        }
        Kind::Mono => {
            let row = w.div_ceil(8);
            let plane = row * h;
            if payload.len() < plane {
                truncated = true;
            }
            // A mask follows when the entry holds two planes; `ICON` has
            // none and is opaque.
            let has_mask = payload.len() >= plane * 2;
            let bit = |base: usize, x: usize, y: usize| -> Option<bool> {
                payload
                    .get(base + y * row + x / 8)
                    .map(|b| b & (0x80 >> (x % 8)) != 0)
            };
            for y in 0..h {
                for x in 0..w {
                    let o = (y * w + x) * 4;
                    let black = bit(0, x, y).unwrap_or(false);
                    let v = if black { 0 } else { 255 };
                    px[o..o + 3].fill(v);
                    px[o + 3] = if has_mask {
                        if bit(plane, x, y) == Some(true) {
                            255
                        } else {
                            0
                        }
                    } else {
                        255
                    };
                }
            }
        }
    }
    Ok(Rgba {
        width: icon.width,
        height: icon.height,
        pixels: px,
        truncated,
    })
}
