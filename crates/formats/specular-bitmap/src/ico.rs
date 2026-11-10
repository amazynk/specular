//! Windows icons (ICO) and cursors (CUR).
//!
//! A file holds several images; [`Ico::parse`] lists the usable ones largest first (most
//! pixels, then the deepest colour, then directory order), reading each entry's own header
//! rather than trusting the directory. The first is the one shown by default. BMP
//! entries decode through [`Bmp`]; PNG entries (Vista+ icons) are returned as a byte range
//! for the caller's PNG decoder.

use crate::bmp::Bmp;
use crate::{Error, Result, u16le, u32be, u32le};
use std::ops::Range;

pub fn probe(head: &[u8]) -> bool {
    if head.len() < 6 || head[0..2] != [0, 0] || !matches!(head[2..4], [1, 0] | [2, 0]) {
        return false;
    }
    let n = u16le(head, 4).unwrap_or(0) as usize;
    if n == 0 {
        return false;
    }
    // First entry: reserved byte 0, data after the directory.
    if head.len() >= 22 {
        let off = u32le(head, 18).unwrap_or(0) as usize;
        let size = u32le(head, 14).unwrap_or(0);
        if head[9] != 0 || off < 6 + 16 * n || size == 0 {
            return false;
        }
    }
    true
}

/// One usable image of an icon file.
#[derive(Clone, Debug)]
pub enum Entry {
    Bmp(Box<Bmp>),
    /// A PNG stream at this byte range of the file.
    Png {
        range: Range<usize>,
        width: u32,
        height: u32,
    },
}

/// A usable image of the file with its size and bits per pixel (from its own header).
#[derive(Clone, Debug)]
pub struct Page {
    pub entry: Entry,
    pub width: u32,
    pub height: u32,
    pub bits: u32,
}

#[derive(Clone, Debug)]
pub struct Ico {
    /// A cursor (CUR) rather than an icon.
    pub cursor: bool,
    /// Entries in the directory.
    pub count: usize,
    /// The usable entries, largest first; never empty.
    pub pages: Vec<Page>,
}

impl Ico {
    pub fn parse(data: &[u8]) -> Result<Ico> {
        if data.len() < 6 {
            return Err(Error::Truncated("ICO header".into()));
        }
        if data[0..2] != [0, 0] || !matches!(data[2..4], [1, 0] | [2, 0]) {
            return Err(Error::Malformed("not an ICO/CUR file".into()));
        }
        let cursor = data[2] == 2;
        let n = u16le(data, 4).unwrap_or(0) as usize;
        if n == 0 {
            return Err(Error::Malformed("icon without images".into()));
        }
        let mut pages: Vec<Page> = Vec::new();
        let mut first_err = None;
        // Each entry's header scan (alpha, AND mask) reads its byte range. Entries may share
        // or overlap ranges, so 65535 of them could rescan the file 65535 times: the ranges
        // read may add up to a few times the file, which entries of a real icon never reach.
        let mut budget = data.len().saturating_mul(16);
        for i in 0..n {
            let e = 6 + i * 16;
            let (Some(size), Some(off)) = (u32le(data, e + 8), u32le(data, e + 12)) else {
                first_err.get_or_insert(Error::Truncated("ICO directory".into()));
                break;
            };
            let (off, size) = (off as usize, size as usize);
            let end = off.saturating_add(size).min(data.len());
            if off >= end {
                first_err.get_or_insert(Error::Truncated("ICO image data".into()));
                continue;
            }
            budget = match budget.checked_sub(end - off) {
                Some(b) => b,
                None => {
                    first_err.get_or_insert(Error::Malformed("ICO entries overlap".into()));
                    break;
                }
            };
            let img = &data[off..end];
            let cand = if img.starts_with(b"\x89PNG\r\n\x1a\n") {
                let (Some(w), Some(h)) = (u32be(img, 16), u32be(img, 20)) else {
                    first_err.get_or_insert(Error::Truncated("PNG in ICO".into()));
                    continue;
                };
                // Bit depth × channels from IHDR.
                let depth = img.get(24).copied().unwrap_or(8) as u32;
                let ch = match img.get(25) {
                    Some(6) => 4,
                    Some(2) => 3,
                    Some(4) => 2,
                    _ => 1,
                };
                Page {
                    entry: Entry::Png {
                        range: off..end,
                        width: w,
                        height: h,
                    },
                    width: w,
                    height: h,
                    bits: depth * ch,
                }
            } else {
                match Bmp::parse_dib(data, off, end, None, true) {
                    Ok(b) => {
                        let i = crate::Decode::info(&b);
                        Page {
                            width: i.width,
                            height: i.height,
                            bits: b.bpp,
                            entry: Entry::Bmp(Box::new(b)),
                        }
                    }
                    Err(e) => {
                        first_err.get_or_insert(e);
                        continue;
                    }
                }
            };
            pages.push(cand);
        }
        if pages.is_empty() {
            return Err(first_err.unwrap_or(Error::Malformed("no usable icon image".into())));
        }
        // Stable: equal entries keep their directory order.
        pages.sort_by_key(|p| std::cmp::Reverse((p.width as u64 * p.height as u64, p.bits)));
        Ok(Ico {
            cursor,
            count: n,
            pages,
        })
    }

    /// The entry shown by default: the largest.
    pub fn entry(&self) -> &Entry {
        &self.pages[0].entry
    }
}
