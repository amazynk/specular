//! The SFNT table directory, TrueType/OpenType collection headers, and writing a standalone
//! SFNT from a list of tables (what WOFF and WOFF2 unwrap to).

use crate::Error;
use crate::read::Reader;

/// Most tables in one font's directory.
pub(crate) const MAX_TABLES: u16 = 512;
/// Most fonts in a collection.
pub(crate) const MAX_FACES: u32 = 4096;

pub(crate) const TRUETYPE: u32 = 0x0001_0000;
pub(crate) const OTTO: u32 = u32::from_be_bytes(*b"OTTO");
pub(crate) const TRUE: u32 = u32::from_be_bytes(*b"true");
pub(crate) const TYP1: u32 = u32::from_be_bytes(*b"typ1");
pub(crate) const TTCF: u32 = u32::from_be_bytes(*b"ttcf");

/// One table of a font: its tag and where its bytes are in the font's data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableRecord {
    pub tag: [u8; 4],
    pub offset: u32,
    pub length: u32,
}

impl TableRecord {
    /// The tag as text ("glyf", "OS/2").
    pub fn tag_str(&self) -> String {
        String::from_utf8_lossy(&self.tag).into_owned()
    }
}

/// Whether `v` starts an SFNT font (not a collection).
pub(crate) fn is_sfnt_version(v: u32) -> bool {
    matches!(v, TRUETYPE | OTTO | TRUE | TYP1)
}

/// The table directory at `at` in `data` (offsets absolute in `data`). Tables whose bytes are
/// not all in `data` are left out.
pub(crate) fn directory(data: &[u8], at: usize) -> Result<(u32, Vec<TableRecord>), Error> {
    let mut r = Reader::at(data, at);
    let version = r.u32().ok_or(Error::Truncated)?;
    if !is_sfnt_version(version) {
        return Err(Error::NotAFont);
    }
    let n = r.u16().ok_or(Error::Truncated)?;
    if n > MAX_TABLES {
        return Err(Error::TooManyTables);
    }
    r.skip(6).ok_or(Error::Truncated)?;
    let mut tables = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let tag = r.tag().ok_or(Error::Truncated)?;
        let _sum = r.u32().ok_or(Error::Truncated)?;
        let offset = r.u32().ok_or(Error::Truncated)?;
        let length = r.u32().ok_or(Error::Truncated)?;
        let end = offset as u64 + length as u64;
        if end <= data.len() as u64 && !tables.iter().any(|t: &TableRecord| t.tag == tag) {
            tables.push(TableRecord {
                tag,
                offset,
                length,
            });
        }
    }
    Ok((version, tables))
}

/// The offsets of the fonts of a TTC/OTC.
pub(crate) fn collection_offsets(data: &[u8]) -> Result<Vec<u32>, Error> {
    let mut r = Reader::new(data);
    if r.u32() != Some(TTCF) {
        return Err(Error::NotAFont);
    }
    r.skip(4).ok_or(Error::Truncated)?;
    let n = r.u32().ok_or(Error::Truncated)?;
    if n == 0 {
        return Err(Error::NoFaces);
    }
    if n > MAX_FACES {
        return Err(Error::TooManyFaces);
    }
    (0..n).map(|_| r.u32().ok_or(Error::Truncated)).collect()
}

fn checksum(d: &[u8]) -> u32 {
    let mut sum = 0u32;
    for c in d.chunks(4) {
        let mut b = [0u8; 4];
        b[..c.len()].copy_from_slice(c);
        sum = sum.wrapping_add(u32::from_be_bytes(b));
    }
    sum
}

/// A standalone SFNT with `tables` (sorted by tag, each 4-byte aligned, checksums set).
pub(crate) fn write(version: u32, mut tables: Vec<([u8; 4], Vec<u8>)>) -> Vec<u8> {
    tables.sort_by_key(|t| t.0);
    let n = tables.len() as u16;
    let mut pow = 1u16;
    let mut log = 0u16;
    while pow.saturating_mul(2) <= n {
        pow *= 2;
        log += 1;
    }
    let search = pow.saturating_mul(16);
    let total: usize = tables.iter().map(|t| (t.1.len() + 3) & !3).sum();
    let mut out = Vec::with_capacity(12 + 16 * tables.len() + total);
    out.extend_from_slice(&version.to_be_bytes());
    out.extend_from_slice(&n.to_be_bytes());
    out.extend_from_slice(&search.to_be_bytes());
    out.extend_from_slice(&log.to_be_bytes());
    out.extend_from_slice(&(n.saturating_mul(16).saturating_sub(search)).to_be_bytes());
    let mut offset = 12 + 16 * tables.len();
    for (tag, data) in &tables {
        out.extend_from_slice(tag);
        out.extend_from_slice(&checksum(data).to_be_bytes());
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        offset += (data.len() + 3) & !3;
    }
    for (_, data) in &tables {
        out.extend_from_slice(data);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }
    out
}
