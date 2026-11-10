//! A small, bounded ZIP reader for OPC packages: the central directory (ZIP64 included), stored
//! and DEFLATE members. Sizes come from the central directory, so members written with data
//! descriptors (as XPS printer drivers do) read the same as others.

use std::borrow::Cow;
use std::ops::Range;

/// Central directory entries read at most.
pub const MAX_ENTRIES: usize = 200_000;
/// DEFLATE cannot expand more than ~1032:1; a declared size beyond that is a lie.
const MAX_RATIO: u64 = 1100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZipError {
    NotZip,
    Truncated,
    Corrupt,
    Encrypted,
    Method,
    TooLarge,
    Inflate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    method: u16,
    flags: u16,
    compressed: u64,
    /// Declared uncompressed size.
    pub size: u64,
    local: u64,
}

impl Entry {
    pub fn encrypted(&self) -> bool {
        self.flags & 1 != 0
    }
}

#[derive(Clone, Debug, Default)]
pub struct Zip {
    pub entries: Vec<Entry>,
}

fn u16le(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        d.get(at..at.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn u32le(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        d.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn u64le(d: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        d.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
}

impl Zip {
    /// Reads the central directory.
    pub fn parse(data: &[u8]) -> Result<Zip, ZipError> {
        if data.len() < 22 {
            return Err(ZipError::NotZip);
        }
        let lo = data.len().saturating_sub(22 + 0xffff);
        let eocd = (lo..=data.len() - 22)
            .rev()
            .find(|&i| data[i..].starts_with(b"PK\x05\x06"))
            .ok_or(ZipError::NotZip)?;
        let mut count = u64::from(u16le(data, eocd + 10).ok_or(ZipError::Truncated)?);
        let mut cd_off = u64::from(u32le(data, eocd + 16).ok_or(ZipError::Truncated)?);
        // ZIP64: the locator sits just before the end record.
        if eocd >= 20 && data[eocd - 20..].starts_with(b"PK\x06\x07") {
            let rec = u64le(data, eocd - 20 + 8).ok_or(ZipError::Truncated)?;
            let rec = usize::try_from(rec).map_err(|_| ZipError::Corrupt)?;
            if data
                .get(rec..)
                .is_some_and(|d| d.starts_with(b"PK\x06\x06"))
            {
                count = u64le(data, rec + 32).ok_or(ZipError::Truncated)?;
                cd_off = u64le(data, rec + 48).ok_or(ZipError::Truncated)?;
            }
        }
        let mut p = usize::try_from(cd_off).map_err(|_| ZipError::Corrupt)?;
        let n = usize::try_from(count)
            .unwrap_or(usize::MAX)
            .min(MAX_ENTRIES);
        let mut entries = Vec::with_capacity(n.min(4096));
        for _ in 0..n {
            let h = data
                .get(p..p.saturating_add(46))
                .ok_or(ZipError::Truncated)?;
            if &h[..4] != b"PK\x01\x02" {
                return Err(ZipError::Corrupt);
            }
            let flags = u16le(h, 8).unwrap_or(0);
            let method = u16le(h, 10).unwrap_or(0);
            let mut compressed = u64::from(u32le(h, 20).unwrap_or(0));
            let mut size = u64::from(u32le(h, 24).unwrap_or(0));
            let name_len = usize::from(u16le(h, 28).unwrap_or(0));
            let extra_len = usize::from(u16le(h, 30).unwrap_or(0));
            let comment = usize::from(u16le(h, 32).unwrap_or(0));
            let mut local = u64::from(u32le(h, 42).unwrap_or(0));
            let name = data
                .get(p + 46..p + 46 + name_len)
                .ok_or(ZipError::Truncated)?;
            let extra = data
                .get(p + 46 + name_len..p + 46 + name_len + extra_len)
                .ok_or(ZipError::Truncated)?;
            // ZIP64 extended information: only the fields saturated in the header, in order.
            let mut q = 0;
            while q + 4 <= extra.len() {
                let id = u16le(extra, q).unwrap_or(0);
                let len = usize::from(u16le(extra, q + 2).unwrap_or(0));
                let body = extra.get(q + 4..q + 4 + len).unwrap_or(&[]);
                if id == 1 {
                    let mut r = 0;
                    let mut next = |v: &mut u64| {
                        if *v == u64::from(u32::MAX)
                            && let Some(x) = u64le(body, r)
                        {
                            *v = x;
                            r += 8;
                        }
                    };
                    next(&mut size);
                    next(&mut compressed);
                    next(&mut local);
                }
                q += 4 + len;
            }
            entries.push(Entry {
                name: String::from_utf8_lossy(name).into_owned(),
                method,
                flags,
                compressed,
                size,
                local,
            });
            p += 46 + name_len + extra_len + comment;
        }
        Ok(Zip { entries })
    }

    /// The member's compressed bytes in `data`.
    fn raw_range(data: &[u8], e: &Entry) -> Result<Range<usize>, ZipError> {
        let at = usize::try_from(e.local).map_err(|_| ZipError::Truncated)?;
        let h = data
            .get(at..at.saturating_add(30))
            .ok_or(ZipError::Truncated)?;
        if &h[..4] != b"PK\x03\x04" {
            return Err(ZipError::Corrupt);
        }
        let start = at
            + 30
            + usize::from(u16le(h, 26).unwrap_or(0))
            + usize::from(u16le(h, 28).unwrap_or(0));
        let end = start.saturating_add(usize::try_from(e.compressed).unwrap_or(usize::MAX));
        if end > data.len() {
            return Err(ZipError::Truncated);
        }
        Ok(start..end)
    }

    /// The whole member, inflated if needed; refused beyond `max` bytes.
    pub fn read<'d>(data: &'d [u8], e: &Entry, max: u64) -> Result<Cow<'d, [u8]>, ZipError> {
        if e.encrypted() {
            return Err(ZipError::Encrypted);
        }
        if e.size > max {
            return Err(ZipError::TooLarge);
        }
        let raw = &data[Self::raw_range(data, e)?];
        match e.method {
            0 => {
                if e.size != e.compressed {
                    return Err(ZipError::Corrupt);
                }
                Ok(Cow::Borrowed(raw))
            }
            8 => {
                if e.size > e.compressed.saturating_mul(MAX_RATIO).saturating_add(1024) {
                    return Err(ZipError::Corrupt);
                }
                let size = usize::try_from(e.size).map_err(|_| ZipError::TooLarge)?;
                let out = specular_png::inflate_raw(raw, size).map_err(|_| ZipError::Inflate)?;
                if out.len() != size {
                    return Err(ZipError::Corrupt);
                }
                Ok(Cow::Owned(out))
            }
            _ => Err(ZipError::Method),
        }
    }
}
