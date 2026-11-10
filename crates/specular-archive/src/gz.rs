//! Gzip-compressed tars (`.tar.gz`, `.tgz`, `.cbt.gz`): the gzip member is inflated into
//! memory once, at open, and the tar inside is then listed and read like a plain one.
//!
//! A compressed tar has no random access, so there is no cheaper way to reach its last entry.
//! The whole tar is held for the archive's lifetime, and its size reserved from the memory
//! budget for as long; its size is the gzip trailer's `ISIZE`, checked against
//! [`MAX_UNPACKED`], the ratio limit and the budget before anything is allocated, and the
//! stream is refused if it inflates to anything else. Above [`EAGER_MAX`] the output grows as
//! the stream inflates (the own inflater), so a lying `ISIZE` costs only what really decodes. Only the first gzip member is read; the CRC-32 is checked.

use crate::inflate::inflater;
use crate::{Error, Limits, Result};
use specular_core::budget::Reservation;

/// Largest tar inflated from a `.tar.gz`, in bytes.
pub(crate) const MAX_UNPACKED: u64 = 2 << 30;

const FHCRC: u8 = 2;
const FEXTRA: u8 = 4;
const FNAME: u8 = 8;
const FCOMMENT: u8 = 16;

fn malformed(what: &str) -> Error {
    Error::Malformed(format!("gzip: {what}"))
}

pub(crate) fn detect(b: &[u8]) -> bool {
    b.len() >= 18 && b[0] == 0x1f && b[1] == 0x8b && b[2] == 8 && b[3] & 0xe0 == 0
}

/// Where the Deflate stream of the gzip member starts.
fn body_start(b: &[u8]) -> Option<usize> {
    let flags = b[3];
    let mut pos = 10usize;
    if flags & FEXTRA != 0 {
        let xlen = u16::from_le_bytes([*b.get(pos)?, *b.get(pos + 1)?]) as usize;
        pos += 2 + xlen;
    }
    for flag in [FNAME, FCOMMENT] {
        if flags & flag != 0 {
            pos += b.get(pos..)?.iter().position(|&c| c == 0)? + 1;
        }
    }
    if flags & FHCRC != 0 {
        pos += 2;
    }
    (pos <= b.len()).then_some(pos)
}

/// Trailer sizes up to this are inflated by zlib-rs into a buffer of exactly that size; larger
/// ones grow as the stream inflates, so a lying `ISIZE` costs only what really decodes.
const EAGER_MAX: u64 = 64 << 20;

/// The inflated contents of the gzip file `b`, and the memory budget held for them.
pub(crate) fn inflate(b: &[u8], limits: &Limits) -> Result<(Vec<u8>, Reservation)> {
    if !detect(b) {
        return Err(Error::NotAnArchive);
    }
    let start = body_start(b).ok_or_else(|| malformed("truncated header"))?;
    let n = b.len();
    let crc = u32::from_le_bytes(b[n - 8..n - 4].try_into().unwrap_or_default());
    let size = u64::from(u32::from_le_bytes(
        b[n - 4..].try_into().unwrap_or_default(),
    ));
    let max = MAX_UNPACKED.min(limits.max_total_size);
    if size > max {
        return Err(Error::Limit(format!(
            "compressed tar of {size} bytes exceeds the {max} byte limit"
        )));
    }
    limits.check_ratio(size, n as u64)?;
    // The tar is held as long as the archive: so is its share of the memory budget.
    let held = crate::reserve(size)?;
    // ISIZE is the size modulo 4 GiB: a larger tar fails to fit and is refused below.
    let body = b.get(start..n - 8).ok_or_else(|| malformed("truncated"))?;
    let out = if size <= EAGER_MAX {
        let mut out = vec![0u8; size as usize];
        let got = inflater().inflate(body, &mut out).map_err(malformed)?;
        if got as u64 != size {
            return Err(malformed("size differs from the trailer"));
        }
        out
    } else {
        let mut out = Vec::new();
        let limit = size as usize;
        crate::codec::inflate64::inflate(body, false, &mut out, limit)
            .map_err(|e| e.into_error("gzip", limit, true))?;
        if out.len() as u64 != size {
            return Err(malformed("size differs from the trailer"));
        }
        out
    };
    if crc32fast::hash(&out) != crc {
        return Err(Error::Checksum);
    }
    Ok((out, held))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A gzip file of `data` (one stored Deflate block) whose trailer claims `isize` bytes.
    fn lying(data: &[u8], isize: u32) -> Vec<u8> {
        let mut g = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3, 1];
        g.extend_from_slice(&(data.len() as u16).to_le_bytes());
        g.extend_from_slice(&(!(data.len() as u16)).to_le_bytes());
        g.extend_from_slice(data);
        g.extend_from_slice(&crc32fast::hash(data).to_le_bytes());
        g.extend_from_slice(&isize.to_le_bytes());
        g
    }

    /// A 1 KiB stream whose trailer claims 1.5 GiB: the claim is reserved from the budget
    /// (refused when it does not fit), and otherwise the output grows only as far as the
    /// stream really inflates.
    #[test]
    fn lying_trailer_size() {
        let limits = Limits {
            max_ratio: u64::MAX,
            ..Limits::default()
        };
        let g = lying(&[7u8; 1024], 3 << 29);
        let r = specular_core::budget::with_limit(64 << 20, || inflate(&g, &limits));
        assert!(
            matches!(&r, Err(Error::Limit(m)) if m.contains("free RAM")),
            "{:?}",
            r.err()
        );
        let r = specular_core::budget::with_limit(4 << 30, || inflate(&g, &limits));
        assert!(matches!(r, Err(Error::Malformed(_))), "{:?}", r.err());
        // The trailer's own size inflates on either path.
        let (out, held) = inflate(&lying(&[7u8; 1024], 1024), &limits).unwrap();
        assert_eq!((out.len(), held.bytes()), (1024, 1024));
    }

    #[test]
    fn header_fields() {
        let mut h = vec![0x1f, 0x8b, 8, FEXTRA | FNAME | FHCRC, 0, 0, 0, 0, 0, 3];
        h.extend_from_slice(&[2, 0, 9, 9]);
        h.extend_from_slice(b"a.tar\0");
        h.extend_from_slice(&[0, 0]);
        let at = h.len();
        h.extend_from_slice(&[0; 8]);
        assert_eq!(body_start(&h), Some(at));
        // A name without its terminator.
        let bad = [0x1f, 0x8b, 8, FNAME, 0, 0, 0, 0, 0, 3, b'a', b'b'];
        assert_eq!(body_start(&bad), None);
    }
}
