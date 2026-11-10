//! A bounded ZIP reader for zipped OME-Zarr stores: the central directory (ZIP64 included),
//! stored and DEFLATE members. Zarr zips are normally stored, so members are sliced from
//! the archive without copying when possible.

use std::collections::HashMap;
use std::ops::Range;

use crate::{Error, Result};

/// Central directory entries read at most.
const MAX_ENTRIES: u64 = 4 << 20;
/// Largest member inflated.
const MAX_MEMBER: u64 = 1 << 30;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    method: u16,
    compressed: u64,
    pub size: u64,
    local: u64,
}

/// The central directory of an archive, by member name.
#[derive(Clone, Debug, Default)]
pub struct Zip {
    pub entries: HashMap<String, Entry>,
}

fn le(d: &[u8], at: usize, n: usize) -> Option<u64> {
    let b = d.get(at..at.checked_add(n)?)?;
    Some(b.iter().rev().fold(0u64, |a, &x| (a << 8) | u64::from(x)))
}

/// Find the end-of-central-directory record.
fn eocd(d: &[u8]) -> Option<usize> {
    let lo = d.len().saturating_sub(22 + 0xffff);
    (lo..d.len().saturating_sub(21))
        .rev()
        .find(|&i| d[i..].starts_with(b"PK\x05\x06"))
}

impl Zip {
    pub fn parse(d: &[u8]) -> Result<Zip> {
        let bad = |s: &str| Error::Malformed(format!("ZIP: {s}"));
        let e = eocd(d).ok_or_else(|| bad("no end of central directory"))?;
        let mut count = le(d, e + 10, 2).ok_or_else(|| bad("directory"))?;
        let mut cd_size = le(d, e + 12, 4).ok_or_else(|| bad("directory"))?;
        let mut cd_off = le(d, e + 16, 4).ok_or_else(|| bad("directory"))?;
        if count == 0xffff || cd_size == 0xffff_ffff || cd_off == 0xffff_ffff {
            // ZIP64 locator just before the EOCD.
            let loc = e.checked_sub(20).ok_or_else(|| bad("ZIP64 locator"))?;
            if !d[loc..].starts_with(b"PK\x06\x07") {
                return Err(bad("ZIP64 locator"));
            }
            let z = le(d, loc + 8, 8).ok_or_else(|| bad("ZIP64 locator"))? as usize;
            if !d.get(z..).is_some_and(|s| s.starts_with(b"PK\x06\x06")) {
                return Err(bad("ZIP64 directory record"));
            }
            count = le(d, z + 32, 8).ok_or_else(|| bad("ZIP64"))?;
            cd_size = le(d, z + 40, 8).ok_or_else(|| bad("ZIP64"))?;
            cd_off = le(d, z + 48, 8).ok_or_else(|| bad("ZIP64"))?;
        }
        if count > MAX_ENTRIES {
            return Err(Error::TooLarge);
        }
        let start = usize::try_from(cd_off).map_err(|_| bad("directory offset"))?;
        let end = start
            .checked_add(usize::try_from(cd_size).map_err(|_| bad("directory size"))?)
            .filter(|&x| x <= d.len())
            .ok_or_else(|| Error::Truncated("ZIP directory".into()))?;
        let cd = &d[start..end];
        let mut entries = HashMap::new();
        let mut p = 0usize;
        for _ in 0..count {
            if !cd.get(p..).is_some_and(|s| s.starts_with(b"PK\x01\x02")) {
                return Err(bad("directory entry"));
            }
            let g = |o: usize, n: usize| le(cd, p + o, n).ok_or_else(|| bad("directory entry"));
            let method = g(10, 2)? as u16;
            let mut compressed = g(20, 4)?;
            let mut size = g(24, 4)?;
            let name_len = g(28, 2)? as usize;
            let extra_len = g(30, 2)? as usize;
            let comment_len = g(32, 2)? as usize;
            let mut local = g(42, 4)?;
            let name = cd
                .get(p + 46..p + 46 + name_len)
                .ok_or_else(|| bad("entry name"))?;
            let extra = cd
                .get(p + 46 + name_len..p + 46 + name_len + extra_len)
                .ok_or_else(|| bad("entry extra"))?;
            // ZIP64 extended information: the fields that overflowed, in order.
            let mut x = 0;
            while x + 4 <= extra.len() {
                let id = le(extra, x, 2).unwrap_or(0);
                let n = le(extra, x + 2, 2).unwrap_or(0) as usize;
                if id == 1 {
                    let mut q = x + 4;
                    for f in [&mut size, &mut compressed, &mut local] {
                        if *f == 0xffff_ffff {
                            *f = le(extra, q, 8).ok_or_else(|| bad("ZIP64 extra"))?;
                            q += 8;
                        }
                    }
                }
                x += 4 + n;
            }
            let name = String::from_utf8_lossy(name).replace('\\', "/");
            if !name.ends_with('/') {
                entries.insert(
                    name,
                    Entry {
                        method,
                        compressed,
                        size,
                        local,
                    },
                );
            }
            p += 46 + name_len + extra_len + comment_len;
        }
        Ok(Zip { entries })
    }

    /// The member's stored bytes in the archive.
    fn raw_range(d: &[u8], e: &Entry) -> Result<Range<usize>> {
        let l = usize::try_from(e.local).map_err(|_| Error::TooLarge)?;
        if !d.get(l..).is_some_and(|s| s.starts_with(b"PK\x03\x04")) {
            return Err(Error::Malformed("ZIP local header".into()));
        }
        let n = le(d, l + 26, 2).unwrap_or(0) as usize;
        let m = le(d, l + 28, 2).unwrap_or(0) as usize;
        let s = l + 30 + n + m;
        let end = s
            .checked_add(usize::try_from(e.compressed).map_err(|_| Error::TooLarge)?)
            .filter(|&x| x <= d.len())
            .ok_or_else(|| Error::Truncated("ZIP member".into()))?;
        Ok(s..end)
    }

    /// A stored member's range (zero-copy), or `None` if it is compressed.
    pub fn stored(&self, d: &[u8], e: &Entry) -> Result<Option<Range<usize>>> {
        if e.method != 0 {
            return Ok(None);
        }
        Self::raw_range(d, e).map(Some)
    }

    /// The whole member.
    pub fn read(&self, d: &[u8], e: &Entry) -> Result<Vec<u8>> {
        if e.size > MAX_MEMBER {
            return Err(Error::TooLarge);
        }
        let r = Self::raw_range(d, e)?;
        match e.method {
            0 => Ok(d[r].to_vec()),
            8 => specular_png::inflate_raw(&d[r], e.size as usize)
                .map_err(|x| Error::Malformed(format!("ZIP member: {x}"))),
            m => Err(Error::Unsupported(format!("ZIP method {m}"))),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A stored ZIP of `files`.
    pub(crate) fn stored_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut cd = Vec::new();
        for (name, data) in files {
            let off = out.len() as u32;
            out.extend_from_slice(b"PK\x03\x04");
            out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(data);
            cd.extend_from_slice(b"PK\x01\x02");
            cd.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            cd.extend_from_slice(&0u32.to_le_bytes());
            cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
            cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
            cd.extend_from_slice(&(name.len() as u16).to_le_bytes());
            cd.extend_from_slice(&[0; 8]);
            cd.extend_from_slice(&[0; 4]);
            cd.extend_from_slice(&off.to_le_bytes());
            cd.extend_from_slice(name.as_bytes());
        }
        let cd_off = out.len() as u32;
        out.extend_from_slice(&cd);
        out.extend_from_slice(b"PK\x05\x06");
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(cd.len() as u32).to_le_bytes());
        out.extend_from_slice(&cd_off.to_le_bytes());
        out.extend_from_slice(&[0; 2]);
        out
    }

    #[test]
    fn reads_stored_members() {
        let z = stored_zip(&[("a/.zattrs", b"{}"), ("a/0/0", b"xyz")]);
        let zip = Zip::parse(&z).unwrap();
        let e = &zip.entries["a/0/0"];
        assert_eq!(zip.read(&z, e).unwrap(), b"xyz");
        assert!(zip.stored(&z, e).unwrap().is_some());
        for n in 0..z.len() {
            let _ = Zip::parse(&z[..n]);
        }
    }
}
