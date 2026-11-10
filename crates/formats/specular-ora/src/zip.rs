//! A small, bounded ZIP reader: the central directory, stored and DEFLATE
//! members. Enough for OpenRaster / Krita documents, nothing more (no ZIP64,
//! no encryption, no other methods).

use std::borrow::Cow;
use std::ops::Range;

use crate::Error;

/// Central directory entries walked at most.
pub const MAX_ENTRIES: usize = 65_535;
/// Largest member inflated (the merged image of a large canvas).
pub const MAX_MEMBER: u64 = 1 << 30;
/// DEFLATE cannot expand more than ~1032:1; a declared size beyond that is a lie.
const MAX_RATIO: u64 = 1100;

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
    pub fn is_stored(&self) -> bool {
        self.method == 0
    }
}

#[derive(Clone, Debug, Default)]
pub struct Zip {
    pub entries: Vec<Entry>,
}

fn u16le(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}

fn u32le(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

impl Zip {
    /// Reads the central directory.
    pub fn parse(data: &[u8]) -> Result<Zip, Error> {
        // End of central directory: 22 bytes + up to 64 KiB of comment.
        let lo = data.len().saturating_sub(22 + 0xffff);
        let eocd = (lo..data.len().saturating_sub(21))
            .rev()
            .find(|&i| data[i..].starts_with(b"PK\x05\x06"))
            .ok_or(Error::Truncated("ZIP directory missing"))?;
        let count = u16le(data, eocd + 10).ok_or(Error::Truncated("ZIP directory"))?;
        let cd_size = u32le(data, eocd + 12).ok_or(Error::Truncated("ZIP directory"))?;
        let cd_off = u32le(data, eocd + 16).ok_or(Error::Truncated("ZIP directory"))?;
        if count == 0xffff || cd_size == u32::MAX || cd_off == u32::MAX {
            return Err(Error::Unsupported("ZIP64 archives".into()));
        }
        let mut p = cd_off as usize;
        let mut entries = Vec::new();
        for _ in 0..usize::from(count).min(MAX_ENTRIES) {
            let h = data
                .get(p..p + 46)
                .ok_or(Error::Truncated("ZIP directory"))?;
            if &h[..4] != b"PK\x01\x02" {
                return Err(Error::Corrupt("ZIP directory entry"));
            }
            let flags = u16le(h, 8).unwrap_or(0);
            let method = u16le(h, 10).unwrap_or(0);
            let compressed = u64::from(u32le(h, 20).unwrap_or(0));
            let size = u64::from(u32le(h, 24).unwrap_or(0));
            let name_len = usize::from(u16le(h, 28).unwrap_or(0));
            let extra = usize::from(u16le(h, 30).unwrap_or(0));
            let comment = usize::from(u16le(h, 32).unwrap_or(0));
            let local = u64::from(u32le(h, 42).unwrap_or(0));
            let name = data
                .get(p + 46..p + 46 + name_len)
                .ok_or(Error::Truncated("ZIP directory"))?;
            entries.push(Entry {
                name: String::from_utf8_lossy(name).into_owned(),
                method,
                flags,
                compressed,
                size,
                local,
            });
            p += 46 + name_len + extra + comment;
        }
        // Members whose bytes overlap (several entries naming one local header, or one
        // member's data holding the next members) make a small file inflate to any size.
        let mut spans: Vec<(u64, u64)> = entries
            .iter()
            .map(|e| (e.local, e.local.saturating_add(e.compressed)))
            .collect();
        spans.sort_unstable();
        if spans
            .windows(2)
            .any(|w| w[1].0 < w[0].1 || w[1].0 == w[0].0)
        {
            return Err(Error::Corrupt("overlapping ZIP members"));
        }
        Ok(Zip { entries })
    }

    pub fn find(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// The member's compressed bytes in `data`.
    pub fn raw_range(&self, data: &[u8], e: &Entry) -> Result<Range<usize>, Error> {
        let at = usize::try_from(e.local).map_err(|_| Error::Truncated("ZIP member"))?;
        let h = data
            .get(at..at.saturating_add(30))
            .ok_or(Error::Truncated("ZIP member"))?;
        if &h[..4] != b"PK\x03\x04" {
            return Err(Error::Corrupt("ZIP member header"));
        }
        let start = at
            + 30
            + usize::from(u16le(h, 26).unwrap_or(0))
            + usize::from(u16le(h, 28).unwrap_or(0));
        let end = start.saturating_add(usize::try_from(e.compressed).unwrap_or(usize::MAX));
        if end > data.len() {
            return Err(Error::Truncated("ZIP member"));
        }
        Ok(start..end)
    }

    fn check(&self, e: &Entry, max: u64) -> Result<(), Error> {
        if e.flags & 1 != 0 {
            return Err(Error::Unsupported(format!("encrypted member {}", e.name)));
        }
        if e.method != 0 && e.method != 8 {
            return Err(Error::Unsupported(format!(
                "ZIP compression method {} ({})",
                e.method, e.name
            )));
        }
        if e.size > max.min(MAX_MEMBER) {
            return Err(Error::TooLarge(e.name.clone()));
        }
        if e.method == 8 && e.size > e.compressed.saturating_mul(MAX_RATIO).saturating_add(1024) {
            return Err(Error::Corrupt("ZIP member size"));
        }
        if e.method == 0 && e.size != e.compressed {
            return Err(Error::Corrupt("ZIP member size"));
        }
        Ok(())
    }

    /// A stored member's bytes, for zero-copy use; `None` if it is compressed.
    pub fn stored_range(&self, data: &[u8], e: &Entry) -> Result<Option<Range<usize>>, Error> {
        self.check(e, MAX_MEMBER)?;
        if !e.is_stored() {
            return Ok(None);
        }
        Ok(Some(self.raw_range(data, e)?))
    }

    /// The whole member, inflated if needed; refused beyond `max` bytes.
    pub fn read<'d>(&self, data: &'d [u8], e: &Entry, max: u64) -> Result<Cow<'d, [u8]>, Error> {
        self.check(e, max)?;
        let raw = &data[self.raw_range(data, e)?];
        if e.is_stored() {
            return Ok(Cow::Borrowed(raw));
        }
        let out = specular_png::inflate_raw(raw, e.size as usize)
            .map_err(|err| Error::Inflate(e.name.clone(), err.to_string()))?;
        // The inflater's limit is approximate; the directory's size is the contract.
        if out.len() as u64 != e.size {
            return Err(Error::Corrupt("ZIP member size"));
        }
        Ok(Cow::Owned(out))
    }

    /// The first `n` bytes of the member (fewer if it is shorter), without
    /// inflating the rest.
    pub fn read_prefix<'d>(
        &self,
        data: &'d [u8],
        e: &Entry,
        n: usize,
    ) -> Result<Cow<'d, [u8]>, Error> {
        self.check(e, MAX_MEMBER)?;
        let raw = &data[self.raw_range(data, e)?];
        let n = n.min(e.size as usize);
        if e.is_stored() {
            return Ok(Cow::Borrowed(&raw[..raw.len().min(n)]));
        }
        specular_png::inflate_raw_prefix(raw, n)
            .map(Cow::Owned)
            .map_err(|err| Error::Inflate(e.name.clone(), err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stored ZIP whose central directory lists `members` as (name, local header offset,
    /// compressed size) over one 16-byte member at offset 0.
    fn zip_with(members: &[(&str, u32, u32)]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"PK\x03\x04");
        out.extend_from_slice(&[0; 22]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.push(b'a');
        out.extend_from_slice(&[7; 16]);
        let cd_off = out.len() as u32;
        let mut cd = Vec::new();
        for (name, local, size) in members {
            let mut h = vec![0u8; 46];
            h[..4].copy_from_slice(b"PK\x01\x02");
            h[20..24].copy_from_slice(&size.to_le_bytes());
            h[24..28].copy_from_slice(&size.to_le_bytes());
            h[28..30].copy_from_slice(&(name.len() as u16).to_le_bytes());
            h[42..46].copy_from_slice(&local.to_le_bytes());
            cd.extend_from_slice(&h);
            cd.extend_from_slice(name.as_bytes());
        }
        out.extend_from_slice(&cd);
        out.extend_from_slice(b"PK\x05\x06");
        out.extend_from_slice(&[0; 4]);
        let n = members.len() as u16;
        out.extend_from_slice(&n.to_le_bytes());
        out.extend_from_slice(&n.to_le_bytes());
        out.extend_from_slice(&(cd.len() as u32).to_le_bytes());
        out.extend_from_slice(&cd_off.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }

    /// Entries sharing one member's bytes (the overlapping-member ZIP bomb) are refused; a
    /// plain archive opens.
    #[test]
    fn overlapping_members_are_refused() {
        let one = zip_with(&[("a", 0, 16)]);
        let z = Zip::parse(&one).unwrap();
        assert_eq!(z.read(&one, &z.entries[0], 1 << 20).unwrap().len(), 16);
        for bomb in [
            zip_with(&[("a", 0, 16), ("b", 0, 16)]),
            zip_with(&[("a", 0, 16), ("b", 8, 4)]),
        ] {
            assert!(matches!(Zip::parse(&bomb), Err(Error::Corrupt(_))));
        }
    }
}
