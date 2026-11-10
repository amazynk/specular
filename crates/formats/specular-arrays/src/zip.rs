//! A bounded ZIP reader for `.npz` archives and zipped Zarr stores: the central directory
//! (ZIP64 included), stored and DEFLATE members. Stored members are sliced from the archive
//! without copying.

use std::collections::HashMap;
use std::ops::Range;

use crate::{Error, Result, bad, le_u16, le_u32, le_u64};

/// Central directory entries read at most.
const MAX_ENTRIES: u64 = 4 << 20;

/// A member of the archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    /// 0 stored, 8 DEFLATE.
    pub method: u16,
    pub compressed: u64,
    pub size: u64,
    local: u64,
    encrypted: bool,
}

/// The central directory of an archive.
#[derive(Clone, Debug, Default)]
pub struct Zip {
    /// In directory order.
    pub entries: Vec<Entry>,
    by_name: HashMap<String, usize>,
}

/// The end-of-central-directory record's offset.
fn eocd(d: &[u8]) -> Option<usize> {
    let lo = d.len().saturating_sub(22 + 0xffff);
    (lo..d.len().saturating_sub(21))
        .rev()
        .find(|&i| d[i..].starts_with(b"PK\x05\x06"))
}

impl Zip {
    /// Whether `head` starts like a ZIP archive (a local header or an empty archive).
    pub fn sniff(head: &[u8]) -> bool {
        head.starts_with(b"PK\x03\x04") || head.starts_with(b"PK\x05\x06")
    }

    pub fn parse(d: &[u8]) -> Result<Zip> {
        let e = eocd(d).ok_or(Error::NotArray)?;
        let mut count = u64::from(le_u16(d, e + 10).unwrap_or(0));
        let mut size = u64::from(le_u32(d, e + 12).unwrap_or(0));
        let mut off = u64::from(le_u32(d, e + 16).unwrap_or(0));
        // ZIP64: the locator just before the record points at the ZIP64 record.
        if e >= 20 && d[e - 20..].starts_with(b"PK\x06\x07") {
            let z = le_u64(d, e - 20 + 8).unwrap_or(u64::MAX);
            if let Ok(z) = usize::try_from(z)
                && d.get(z..).is_some_and(|r| r.starts_with(b"PK\x06\x06"))
            {
                count = le_u64(d, z + 32).unwrap_or(0);
                size = le_u64(d, z + 40).unwrap_or(0);
                off = le_u64(d, z + 48).unwrap_or(0);
            }
        }
        if count > MAX_ENTRIES {
            return Err(Error::TooLarge);
        }
        let start = usize::try_from(off).map_err(|_| Error::TooLarge)?;
        let end = start
            .checked_add(usize::try_from(size).map_err(|_| Error::TooLarge)?)
            .filter(|&x| x <= d.len())
            .ok_or_else(|| Error::Truncated("ZIP central directory".into()))?;
        let cd = &d[start..end];
        let mut zip = Zip::default();
        let mut p = 0usize;
        for _ in 0..count {
            if !cd.get(p..).is_some_and(|r| r.starts_with(b"PK\x01\x02")) {
                return bad("ZIP central directory entry");
            }
            let flags = le_u16(cd, p + 8).unwrap_or(0);
            let method = le_u16(cd, p + 10).unwrap_or(0);
            let mut compressed = u64::from(le_u32(cd, p + 20).unwrap_or(0));
            let mut usize_ = u64::from(le_u32(cd, p + 24).unwrap_or(0));
            let nlen = usize::from(le_u16(cd, p + 28).unwrap_or(0));
            let xlen = usize::from(le_u16(cd, p + 30).unwrap_or(0));
            let clen = usize::from(le_u16(cd, p + 32).unwrap_or(0));
            let mut local = u64::from(le_u32(cd, p + 42).unwrap_or(0));
            let name = cd
                .get(p + 46..p + 46 + nlen)
                .ok_or_else(|| Error::Truncated("ZIP central directory".into()))?;
            let extra = cd.get(p + 46 + nlen..p + 46 + nlen + xlen).unwrap_or(&[]);
            // ZIP64 extended information: the fields saturated in the entry, in order.
            let mut q = 0usize;
            while q + 4 <= extra.len() {
                let id = le_u16(extra, q).unwrap_or(0);
                let n = usize::from(le_u16(extra, q + 2).unwrap_or(0));
                if id == 1 {
                    let mut r = q + 4;
                    for field in [&mut usize_, &mut compressed, &mut local] {
                        if *field == u64::from(u32::MAX)
                            && let Some(v) = le_u64(extra, r)
                        {
                            *field = v;
                            r += 8;
                        }
                    }
                }
                q += 4 + n;
            }
            let name = String::from_utf8_lossy(name).replace('\\', "/");
            zip.by_name.insert(name.clone(), zip.entries.len());
            zip.entries.push(Entry {
                name,
                method,
                compressed,
                size: usize_,
                local,
                encrypted: flags & 1 != 0,
            });
            p += 46 + nlen + xlen + clen;
        }
        Ok(zip)
    }

    pub fn get(&self, name: &str) -> Option<&Entry> {
        self.by_name.get(name).map(|&i| &self.entries[i])
    }

    /// Where a member's stored bytes lie in the archive.
    pub fn data_range(&self, d: &[u8], e: &Entry) -> Result<Range<usize>> {
        if e.encrypted {
            return Err(Error::Unsupported(
                "encrypted ZIP members are not supported".into(),
            ));
        }
        let l = usize::try_from(e.local).map_err(|_| Error::TooLarge)?;
        if !d.get(l..).is_some_and(|r| r.starts_with(b"PK\x03\x04")) {
            return bad("ZIP local header");
        }
        let nlen = usize::from(le_u16(d, l + 26).unwrap_or(0));
        let xlen = usize::from(le_u16(d, l + 28).unwrap_or(0));
        let s = l + 30 + nlen + xlen;
        let end = s
            .checked_add(usize::try_from(e.compressed).map_err(|_| Error::TooLarge)?)
            .ok_or(Error::TooLarge)?;
        if end > d.len() {
            return Err(Error::Truncated(format!("ZIP member {}", e.name)));
        }
        Ok(s..end)
    }

    /// A stored member's bytes, in place; `None` for a compressed one.
    pub fn stored<'a>(&self, d: &'a [u8], e: &Entry) -> Result<Option<&'a [u8]>> {
        let r = self.data_range(d, e)?;
        Ok((e.method == 0).then(|| &d[r]))
    }

    /// The first `n` bytes of a member (fewer if it is shorter).
    pub fn read_prefix(&self, d: &[u8], e: &Entry, n: usize) -> Result<Vec<u8>> {
        let r = self.data_range(d, e)?;
        match e.method {
            0 => Ok(d[r.start..r.end.min(r.start + n)].to_vec()),
            8 => specular_png::inflate_raw_prefix(&d[r], n)
                .map_err(|x| Error::Malformed(format!("ZIP member {}: {x}", e.name))),
            m => Err(unsupported_method(m)),
        }
    }

    /// A whole member, inflated; at most `max` bytes.
    pub fn read(&self, d: &[u8], e: &Entry, max: u64) -> Result<Vec<u8>> {
        if e.size > max {
            return Err(Error::TooLarge);
        }
        let r = self.data_range(d, e)?;
        let n = usize::try_from(e.size).map_err(|_| Error::TooLarge)?;
        match e.method {
            0 => Ok(d[r].to_vec()),
            8 => specular_png::inflate_raw(&d[r], n)
                .map_err(|x| Error::Malformed(format!("ZIP member {}: {x}", e.name))),
            m => Err(unsupported_method(m)),
        }
    }
}

fn unsupported_method(m: u16) -> Error {
    Error::Unsupported(format!(
        "ZIP compression method {m} is not supported (only stored and DEFLATE)"
    ))
}

#[cfg(test)]
pub(crate) mod tests {
    /// A ZIP of stored members (and DEFLATE ones, given as raw deflate streams with their
    /// sizes), optionally with ZIP64 extra fields.
    pub type Member<'a> = (&'a str, &'a [u8], Option<(Vec<u8>, usize)>);

    pub fn build(members: &[Member<'_>], zip64: bool) -> Vec<u8> {
        let mut out = Vec::new();
        let mut cd = Vec::new();
        for (name, data, deflated) in members {
            let local = out.len() as u64;
            let (method, body, size): (u16, &[u8], usize) = match deflated {
                Some((d, n)) => (8, d, *n),
                None => (0, data, data.len()),
            };
            out.extend_from_slice(b"PK\x03\x04");
            out.extend_from_slice(&[20, 0, 0, 0]);
            out.extend_from_slice(&method.to_le_bytes());
            out.extend_from_slice(&[0; 8]);
            out.extend_from_slice(&(body.len() as u32).to_le_bytes());
            out.extend_from_slice(&(size as u32).to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(body);
            cd.extend_from_slice(b"PK\x01\x02");
            cd.extend_from_slice(&[20, 0, 20, 0, 0, 0]);
            cd.extend_from_slice(&method.to_le_bytes());
            cd.extend_from_slice(&[0; 8]);
            let (c, u, l) = if zip64 {
                (u32::MAX, u32::MAX, u32::MAX)
            } else {
                (body.len() as u32, size as u32, local as u32)
            };
            cd.extend_from_slice(&c.to_le_bytes());
            cd.extend_from_slice(&u.to_le_bytes());
            cd.extend_from_slice(&(name.len() as u16).to_le_bytes());
            cd.extend_from_slice(&(if zip64 { 28u16 } else { 0 }).to_le_bytes());
            cd.extend_from_slice(&[0; 10]);
            cd.extend_from_slice(&l.to_le_bytes());
            cd.extend_from_slice(name.as_bytes());
            if zip64 {
                cd.extend_from_slice(&1u16.to_le_bytes());
                cd.extend_from_slice(&24u16.to_le_bytes());
                cd.extend_from_slice(&(size as u64).to_le_bytes());
                cd.extend_from_slice(&(body.len() as u64).to_le_bytes());
                cd.extend_from_slice(&local.to_le_bytes());
            }
        }
        let cd_off = out.len();
        out.extend_from_slice(&cd);
        out.extend_from_slice(b"PK\x05\x06");
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&(members.len() as u16).to_le_bytes());
        out.extend_from_slice(&(members.len() as u16).to_le_bytes());
        out.extend_from_slice(&(cd.len() as u32).to_le_bytes());
        out.extend_from_slice(&(cd_off as u32).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }

    #[test]
    fn reads_stored_and_zip64() {
        for z64 in [false, true] {
            let a = build(&[("a.npy", b"hello", None), ("b/c", b"xy", None)], z64);
            let z = super::Zip::parse(&a).unwrap();
            assert_eq!(z.entries.len(), 2);
            let e = z.get("a.npy").unwrap();
            assert_eq!(z.stored(&a, e).unwrap(), Some(&b"hello"[..]));
            assert_eq!(z.read(&a, z.get("b/c").unwrap(), 10).unwrap(), b"xy");
            assert_eq!(z.read_prefix(&a, e, 2).unwrap(), b"he");
        }
        assert!(super::Zip::parse(b"not a zip").is_err());
    }
}
