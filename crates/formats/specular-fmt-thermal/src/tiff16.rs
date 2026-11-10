//! The one TIFF FLIR raw data uses: the first IFD, one 16-bit unsigned sample per pixel,
//! uncompressed strips, either byte order.

use crate::{Result, malformed};

const F: &str = "FLIR TIFF raw data";

struct R<'a> {
    d: &'a [u8],
    le: bool,
}

impl R<'_> {
    fn u16(&self, at: usize) -> Result<u16> {
        let b: [u8; 2] = self
            .d
            .get(at..at + 2)
            .and_then(|s| s.try_into().ok())
            .ok_or_else(|| malformed(F, "truncated"))?;
        Ok(if self.le {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        })
    }
    fn u32(&self, at: usize) -> Result<u32> {
        let b: [u8; 4] = self
            .d
            .get(at..at + 4)
            .and_then(|s| s.try_into().ok())
            .ok_or_else(|| malformed(F, "truncated"))?;
        Ok(if self.le {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    }
    /// The values of entry `e` (SHORT or LONG), inline or at their offset.
    fn values(&self, e: usize) -> Result<Vec<u32>> {
        let ty = self.u16(e + 2)?;
        let n = self.u32(e + 4)? as usize;
        let size = match ty {
            3 => 2,
            4 => 4,
            _ => return Err(malformed(F, format!("field type {ty}"))),
        };
        if n > self.d.len() / size + 1 {
            return Err(malformed(F, "field count"));
        }
        let at = if n * size <= 4 {
            e + 8
        } else {
            self.u32(e + 8)? as usize
        };
        (0..n)
            .map(|i| {
                if size == 2 {
                    self.u16(at + 2 * i).map(u32::from)
                } else {
                    self.u32(at + 4 * i)
                }
            })
            .collect()
    }
}

pub fn is_tiff(d: &[u8]) -> bool {
    d.starts_with(b"II*\0") || d.starts_with(b"MM\0*")
}

/// Width, height and the samples, row-major.
pub fn read(d: &[u8]) -> Result<(usize, usize, Vec<u16>)> {
    if !is_tiff(d) {
        return Err(malformed(F, "no TIFF header"));
    }
    let r = R {
        d,
        le: d[0] == b'I',
    };
    let ifd = r.u32(4)? as usize;
    let n = usize::from(r.u16(ifd)?);
    let (mut w, mut h, mut bps, mut spp, mut comp, mut fmt) = (0, 0, 1, 1, 1, 1);
    let (mut offsets, mut counts) = (Vec::new(), Vec::new());
    for i in 0..n {
        let e = ifd + 2 + 12 * i;
        let first = |r: &R| r.values(e).map(|v| v.first().copied().unwrap_or(0));
        match r.u16(e)? {
            256 => w = first(&r)? as usize,
            257 => h = first(&r)? as usize,
            258 => bps = first(&r)?,
            259 => comp = first(&r)?,
            273 => offsets = r.values(e)?,
            277 => spp = first(&r)?,
            279 => counts = r.values(e)?,
            339 => fmt = first(&r)?,
            _ => {}
        }
    }
    if bps != 16 || spp != 1 || comp != 1 || fmt != 1 {
        return Err(crate::Error::Unsupported {
            format: F,
            what: format!("{bps}-bit, {spp} samples, compression {comp}, sample format {fmt}"),
        });
    }
    if w == 0 || h == 0 || offsets.len() != counts.len() || offsets.is_empty() {
        return Err(malformed(F, "missing size or strips"));
    }
    // The strips come from the file, so an image larger than it cannot be there.
    let need = w
        .checked_mul(h)
        .and_then(|n| n.checked_mul(2))
        .filter(|&n| n <= d.len())
        .ok_or_else(|| malformed(F, "strips shorter than the image"))?;
    let mut bytes = Vec::with_capacity(need);
    for (&o, &c) in offsets.iter().zip(&counts) {
        let s = d
            .get(o as usize..(o as usize).saturating_add(c as usize))
            .ok_or_else(|| malformed(F, "strip past the end"))?;
        // Strips listed over and over add nothing past the image.
        bytes.extend_from_slice(&s[..s.len().min(need - bytes.len())]);
    }
    if bytes.len() < need {
        return Err(malformed(F, "strips shorter than the image"));
    }
    let px = bytes
        .chunks_exact(2)
        .map(|b| {
            if r.le {
                u16::from_le_bytes([b[0], b[1]])
            } else {
                u16::from_be_bytes([b[0], b[1]])
            }
        })
        .collect();
    Ok((w, h, px))
}

#[cfg(test)]
mod hostile {
    use super::*;

    /// A little-endian TIFF with these (tag, type, count, value) entries and `tail` after
    /// the IFD.
    fn tiff(entries: &[(u16, u16, u32, u32)], tail: &[u8]) -> Vec<u8> {
        let mut d = b"II*\0".to_vec();
        d.extend_from_slice(&8u32.to_le_bytes());
        d.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        for &(tag, ty, n, v) in entries {
            d.extend_from_slice(&tag.to_le_bytes());
            d.extend_from_slice(&ty.to_le_bytes());
            d.extend_from_slice(&n.to_le_bytes());
            d.extend_from_slice(&v.to_le_bytes());
        }
        d.extend_from_slice(tail);
        d
    }

    /// 4294967295 × 4294967295 pixels: the image size overflows.
    #[test]
    fn size_of_the_largest_dimensions() {
        let d = tiff(
            &[
                (256, 4, 1, u32::MAX),
                (257, 4, 1, u32::MAX),
                (258, 3, 1, 16),
                (273, 4, 1, 0),
                (279, 4, 1, 8),
            ],
            &[],
        );
        assert!(read(&d).is_err());
    }

    /// 4096 strip offsets all pointing at the whole file, for a 2 × 2 image.
    #[test]
    fn one_strip_listed_over_and_over() {
        let at = 8 + 2 + 12 * 5 + 4;
        let n = 4096u32;
        let offsets = (at + 4 * n as usize) as u32;
        let mut tail = vec![0u8; 4];
        for _ in 0..n {
            tail.extend_from_slice(&0u32.to_le_bytes());
        }
        let mut counts = Vec::new();
        for _ in 0..n {
            counts.extend_from_slice(&4096u32.to_le_bytes());
        }
        tail.extend(counts);
        let d = tiff(
            &[
                (256, 4, 1, 2),
                (257, 4, 1, 2),
                (258, 3, 1, 16),
                (273, 4, n, at as u32),
                (279, 4, n, offsets),
            ],
            &tail,
        );
        let (w, h, px) = read(&d).unwrap();
        assert_eq!((w, h, px.len()), (2, 2, 4));
    }
}
