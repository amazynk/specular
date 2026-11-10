//! Read-only Compound File Binary files (OLE2 structured storage, [MS-CFB]): the container
//! of Zeiss Xradia TXM / TXRM volumes, Zeiss ZVI and Olympus OIB images, and older Office
//! documents.
//!
//! [`Cfb::open`] reads the header, the FAT (through the DIFAT), the mini FAT and the
//! directory, and indexes every storage and stream by its path ("ImageData1/Image1").
//! [`Cfb::read`] returns a stream's bytes: borrowed from the file when its sectors run on
//! one after another (the usual case for large streams), gathered otherwise. Names compare
//! without regard to ASCII case, as the format compares them.
//!
//! Every chain is checked: sectors past the file's end, loops and chains shorter than the
//! stream are errors, never panics.

use std::borrow::Cow;
use std::collections::HashMap;

const MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
const END: u32 = 0xFFFF_FFFE;
const FREE: u32 = 0xFFFF_FFFF;
const MAX_REG: u32 = 0xFFFF_FFFA;
/// No sibling or child.
const NONE: u32 = 0xFFFF_FFFF;
/// Deepest storage nesting listed.
const MAX_NESTING: usize = 64;

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("not a compound file")]
    NotCfb,
    #[error("malformed compound file: {0}")]
    Malformed(&'static str),
    #[error("compound file ends early")]
    Truncated,
    #[error("no such stream")]
    NotFound,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Whether `data` starts like a compound file.
pub fn sniff(data: &[u8]) -> bool {
    data.starts_with(&MAGIC)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Storage,
    Stream,
    Root,
}

/// A directory entry.
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub kind: Kind,
    /// Bytes in the stream (the mini stream's, for the root).
    pub size: u64,
    start: u32,
    left: u32,
    right: u32,
    child: u32,
    /// A storage, stream or root (object type 1, 2 or 5). Other types (unallocated, or
    /// damaged) are never listed, even when a sibling link reaches them.
    used: bool,
}

/// An open compound file over its bytes.
pub struct Cfb<'a> {
    data: &'a [u8],
    shift: u32,
    mini_shift: u32,
    cutoff: u64,
    /// The FAT's own sectors, in order.
    fat: Vec<u32>,
    mini_fat: Vec<u32>,
    entries: Vec<Entry>,
    /// The mini stream's sectors (regular sectors, in order).
    mini_sectors: Vec<u32>,
    /// Lower-case path ("imagedata1/image1") to entry.
    paths: HashMap<String, usize>,
    /// Entries in directory order with their paths as written.
    listing: Vec<(String, usize)>,
}

fn u16_at(d: &[u8], at: usize) -> Result<u16> {
    d.get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or(Error::Truncated)
}

fn u32_at(d: &[u8], at: usize) -> Result<u32> {
    d.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or(Error::Truncated)
}

fn u64_at(d: &[u8], at: usize) -> Result<u64> {
    let lo = u32_at(d, at)? as u64;
    let hi = u32_at(d, at + 4)? as u64;
    Ok(lo | hi << 32)
}

impl<'a> Cfb<'a> {
    /// Parses the header, allocation tables and directory of `data` (the whole file).
    pub fn open(data: &'a [u8]) -> Result<Cfb<'a>> {
        if !sniff(data) {
            return Err(Error::NotCfb);
        }
        if data.len() < 512 {
            return Err(Error::Truncated);
        }
        if u16_at(data, 28)? != 0xFFFE {
            return Err(Error::Malformed("byte order mark"));
        }
        let shift = u16_at(data, 30)? as u32;
        let mini_shift = u16_at(data, 32)? as u32;
        if !(shift == 9 || shift == 12) || !(1..shift).contains(&mini_shift) {
            return Err(Error::Malformed("sector size"));
        }
        let fat_sectors = u32_at(data, 44)? as usize;
        let dir_start = u32_at(data, 48)?;
        let cutoff = u32_at(data, 56)? as u64;
        let mini_fat_start = u32_at(data, 60)?;
        let mini_fat_count = u32_at(data, 64)? as usize;
        let mut difat_next = u32_at(data, 68)?;
        let difat_count = u32_at(data, 72)? as usize;
        let mut c = Cfb {
            data,
            shift,
            mini_shift,
            cutoff,
            fat: Vec::new(),
            mini_fat: Vec::new(),
            entries: Vec::new(),
            mini_sectors: Vec::new(),
            paths: HashMap::new(),
            listing: Vec::new(),
        };
        let sectors = c.sector_count();
        if fat_sectors > sectors + 1 {
            return Err(Error::Malformed("FAT sector count"));
        }
        // The FAT's own sectors: 109 in the header, the rest along the DIFAT chain.
        let mut fat_ids = Vec::with_capacity(fat_sectors);
        for i in 0..109 {
            let s = u32_at(data, 76 + 4 * i)?;
            if s <= MAX_REG {
                fat_ids.push(s);
            }
        }
        let per = c.sector_size() / 4;
        let mut seen = 0;
        while difat_next <= MAX_REG && seen <= difat_count.min(sectors) {
            let sec = c.sector(difat_next)?;
            for i in 0..per - 1 {
                let s = u32_at(sec, 4 * i)?;
                if s <= MAX_REG {
                    fat_ids.push(s);
                }
            }
            difat_next = u32_at(sec, 4 * (per - 1))?;
            seen += 1;
        }
        fat_ids.truncate(fat_sectors.max(1));
        // Entries are looked up in place (a large file's FAT is tens of megabytes).
        for &s in &fat_ids {
            c.sector(s)?;
        }
        c.fat = fat_ids;
        // Directory.
        let dir = c.chain_bytes(dir_start, None)?;
        let mut entries = Vec::with_capacity(dir.len() / 128);
        for e in dir.chunks_exact(128) {
            let len = (u16_at(e, 64)? as usize).min(64);
            let units: Vec<u16> = e[..len.saturating_sub(2)]
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect();
            let kind = match e[66] {
                1 => Some(Kind::Storage),
                2 => Some(Kind::Stream),
                5 => Some(Kind::Root),
                _ => None,
            };
            let mut size = u64_at(e, 120)?;
            if shift == 9 {
                // Version 3: only the low 32 bits count.
                size &= 0xFFFF_FFFF;
            }
            entries.push(Entry {
                name: String::from_utf16_lossy(&units),
                kind: kind.unwrap_or(Kind::Stream),
                size: if kind.is_some() { size } else { 0 },
                start: u32_at(e, 116)?,
                left: u32_at(e, 68)?,
                right: u32_at(e, 72)?,
                child: if kind.is_some() { u32_at(e, 76)? } else { NONE },
                used: kind.is_some(),
            });
            if kind.is_none() {
                // Unused slot: keep its index, never reach it.
                if let Some(last) = entries.last_mut() {
                    last.left = NONE;
                    last.right = NONE;
                }
            }
        }
        if entries.first().map(|e| e.kind) != Some(Kind::Root) {
            return Err(Error::Malformed("no root entry"));
        }
        c.entries = entries;
        // Mini FAT and mini stream.
        if mini_fat_count > 0 && mini_fat_start <= MAX_REG {
            let b = c.chain_bytes(mini_fat_start, None)?;
            c.mini_fat = b
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect();
        }
        let root = c.entries[0].clone();
        if root.size > 0 && root.start <= MAX_REG {
            // A mini stream whose chain breaks off (a link past the file's end, a loop) keeps
            // the sectors before the break: the small streams stored there still read, the
            // others fail on their own (as olefile does), instead of the whole file failing.
            c.mini_sectors = c.chain_prefix(root.start, root.size);
        }
        c.index();
        Ok(c)
    }

    fn sector_size(&self) -> usize {
        1 << self.shift
    }

    /// Whole sectors after the header.
    fn sector_count(&self) -> usize {
        self.data
            .len()
            .div_ceil(self.sector_size())
            .saturating_sub(1)
    }

    fn sector(&self, s: u32) -> Result<&'a [u8]> {
        let at = (s as usize + 1)
            .checked_mul(self.sector_size())
            .ok_or(Error::Truncated)?;
        let d: &'a [u8] = self.data;
        // A short last sector is read as far as it goes.
        let end = (at + self.sector_size()).min(d.len());
        d.get(at..end)
            .filter(|b| !b.is_empty())
            .ok_or(Error::Truncated)
    }

    /// The sector ids of the regular chain starting at `start`, enough for `size` bytes
    /// (every sector to the end of chain when `None`).
    fn chain(&self, start: u32, size: Option<u64>) -> Result<Vec<u32>> {
        let ss = self.sector_size() as u64;
        let want = size.map(|n| n.div_ceil(ss) as usize);
        let total = self.sector_count();
        let mut out = Vec::with_capacity(want.unwrap_or(1).min(total));
        let mut s = start;
        loop {
            if want == Some(out.len()) {
                break;
            }
            if s == END || s == FREE {
                if want.is_some() {
                    return Err(Error::Malformed("chain shorter than its stream"));
                }
                break;
            }
            // Longer than the file has sectors: a loop.
            if s > MAX_REG || s as usize >= total || out.len() >= total {
                return Err(Error::Malformed("sector chain"));
            }
            out.push(s);
            s = self.next(s)?;
        }
        Ok(out)
    }

    /// The regular chain from `start` for `size` bytes, up to its first bad link (end of
    /// chain too early, a sector past the file's end, a loop).
    fn chain_prefix(&self, start: u32, size: u64) -> Vec<u32> {
        let want = size.div_ceil(self.sector_size() as u64) as usize;
        let total = self.sector_count();
        let mut out = Vec::with_capacity(want.min(total));
        let mut s = start;
        while out.len() < want && s <= MAX_REG && (s as usize) < total && out.len() < total {
            out.push(s);
            match self.next(s) {
                Ok(n) => s = n,
                Err(_) => break,
            }
        }
        out
    }

    /// The FAT entry of sector `s`: the next sector of its chain.
    fn next(&self, s: u32) -> Result<u32> {
        let per = self.sector_size() / 4;
        let f = *self
            .fat
            .get(s as usize / per)
            .ok_or(Error::Malformed("sector chain"))?;
        let at = ((f as usize + 1) << self.shift) + (s as usize % per) * 4;
        u32_at(self.data, at)
    }

    fn chain_bytes(&self, start: u32, size: Option<u64>) -> Result<Vec<u8>> {
        let ids = self.chain(start, size)?;
        let mut out = Vec::with_capacity(ids.len() << self.shift);
        for s in ids {
            let sec = self.sector(s)?;
            out.extend_from_slice(sec);
            out.resize(out.len() + self.sector_size() - sec.len(), 0);
        }
        if let Some(n) = size {
            out.truncate(n as usize);
        }
        Ok(out)
    }

    /// Paths of every entry, walking the sibling trees (each entry once, so loops end).
    fn index(&mut self) {
        let n = self.entries.len();
        let mut seen = vec![false; n];
        seen[0] = true;
        // Storages nest at most MAX_NESTING deep: each entry's path repeats its parents', so
        // a chain of thousands would take memory quadratic in the directory's size.
        let mut stack: Vec<(u32, String, usize)> = vec![(self.entries[0].child, String::new(), 0)];
        let mut listing = Vec::new();
        while let Some((id, parent, depth)) = stack.pop() {
            let i = id as usize;
            if id == NONE || i >= n || seen[i] {
                continue;
            }
            seen[i] = true;
            let e = &self.entries[i];
            if !e.used {
                continue;
            }
            // By depth, not by an empty parent path: a storage may itself be named "".
            let path = if depth == 0 {
                e.name.clone()
            } else {
                format!("{parent}/{}", e.name)
            };
            stack.push((e.left, parent.clone(), depth));
            stack.push((e.right, parent, depth));
            if e.kind == Kind::Storage && depth < MAX_NESTING {
                stack.push((e.child, path.clone(), depth + 1));
            }
            listing.push((path, i));
        }
        listing.sort_by_key(|&(_, i)| i);
        self.paths = listing
            .iter()
            .map(|(p, i)| (p.to_ascii_lowercase(), *i))
            .collect();
        self.listing = listing;
    }

    /// Every storage and stream with its path ("ImageInfo/ImageWidth"), in directory order.
    pub fn list(&self) -> impl Iterator<Item = (&str, &Entry)> {
        self.listing
            .iter()
            .map(|(p, i)| (p.as_str(), &self.entries[*i]))
    }

    /// The entry at `path` (names separated by `/`, any ASCII case).
    pub fn entry(&self, path: &str) -> Option<&Entry> {
        self.paths
            .get(&path.to_ascii_lowercase())
            .map(|&i| &self.entries[i])
    }

    /// Whether a storage or stream is at `path`.
    pub fn exists(&self, path: &str) -> bool {
        self.entry(path).is_some()
    }

    /// The bytes of the stream at `path`.
    pub fn read(&self, path: &str) -> Result<Cow<'a, [u8]>> {
        let e = self.entry(path).ok_or(Error::NotFound)?;
        if e.kind != Kind::Stream {
            return Err(Error::NotFound);
        }
        self.read_entry(e)
    }

    fn read_entry(&self, e: &Entry) -> Result<Cow<'a, [u8]>> {
        let size = e.size;
        if size == 0 {
            return Ok(Cow::Borrowed(&[]));
        }
        if size < self.cutoff {
            return self.read_mini(e.start, size).map(Cow::Owned);
        }
        let ids = self.chain(e.start, Some(size))?;
        let n = usize::try_from(size).map_err(|_| Error::Truncated)?;
        // Sectors one after another: the stream is a slice of the file.
        if ids.windows(2).all(|w| w[1] == w[0] + 1) {
            let at = (ids[0] as usize + 1) << self.shift;
            let d: &'a [u8] = self.data;
            return d.get(at..at + n).map(Cow::Borrowed).ok_or(Error::Truncated);
        }
        let mut out = Vec::with_capacity(n);
        for s in ids {
            let sec = self.sector(s)?;
            let take = (n - out.len()).min(sec.len());
            out.extend_from_slice(&sec[..take]);
        }
        if out.len() < n {
            return Err(Error::Truncated);
        }
        Ok(Cow::Owned(out))
    }

    fn read_mini(&self, start: u32, size: u64) -> Result<Vec<u8>> {
        let ms = 1usize << self.mini_shift;
        let per = self.sector_size() / ms;
        // A stream lies within the mini stream: a chain looping on itself must not read more.
        if size > (self.mini_sectors.len() as u64) << self.shift {
            return Err(Error::Malformed("mini stream"));
        }
        let n = size as usize;
        // `size` is only below the header's cutoff, which may be 4 GiB: reserve no more than
        // the mini FAT can chain.
        let mut out = Vec::with_capacity(n.min(self.mini_fat.len() << self.mini_shift));
        let mut s = start;
        let mut steps = 0;
        while out.len() < n {
            if s > MAX_REG || s as usize >= self.mini_fat.len() || steps > self.mini_fat.len() {
                return Err(Error::Malformed("mini sector chain"));
            }
            let big = *self
                .mini_sectors
                .get(s as usize / per)
                .ok_or(Error::Malformed("mini stream"))?;
            let sec = self.sector(big)?;
            let at = (s as usize % per) * ms;
            let chunk = sec
                .get(at..(at + ms).min(sec.len()))
                .ok_or(Error::Truncated)?;
            let take = (n - out.len()).min(chunk.len());
            out.extend_from_slice(&chunk[..take]);
            s = self.mini_fat[s as usize];
            steps += 1;
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_what_is_not_one() {
        assert_eq!(Cfb::open(b"hello").err(), Some(Error::NotCfb));
        let mut h = MAGIC.to_vec();
        h.resize(100, 0);
        assert_eq!(Cfb::open(&h).err(), Some(Error::Truncated));
        h.resize(512, 0);
        assert_eq!(
            Cfb::open(&h).err(),
            Some(Error::Malformed("byte order mark"))
        );
    }

    #[test]
    fn loops_and_wild_sectors_are_errors() {
        // A header whose directory chain points at itself.
        let mut d = vec![0u8; 512 * 3];
        d[..8].copy_from_slice(&MAGIC);
        d[26] = 3;
        d[28..30].copy_from_slice(&0xFFFEu16.to_le_bytes());
        d[30] = 9;
        d[32] = 6;
        d[44] = 1; // one FAT sector: sector 0
        d[48..52].copy_from_slice(&1u32.to_le_bytes()); // directory at sector 1
        d[56..60].copy_from_slice(&4096u32.to_le_bytes());
        d[60..64].copy_from_slice(&END.to_le_bytes());
        d[68..72].copy_from_slice(&END.to_le_bytes());
        d[76..80].copy_from_slice(&0u32.to_le_bytes());
        for i in 1..109 {
            d[76 + 4 * i..80 + 4 * i].copy_from_slice(&FREE.to_le_bytes());
        }
        // FAT: sector 0 is the FAT, sector 1 points to itself.
        let fat = 512;
        d[fat..fat + 4].copy_from_slice(&0xFFFF_FFFDu32.to_le_bytes());
        d[fat + 4..fat + 8].copy_from_slice(&1u32.to_le_bytes());
        for i in 2..128 {
            d[fat + 4 * i..fat + 4 * i + 4].copy_from_slice(&FREE.to_le_bytes());
        }
        assert_eq!(Cfb::open(&d).err(), Some(Error::Malformed("sector chain")));
        // Directory chain past the end of the file.
        d[48..52].copy_from_slice(&900u32.to_le_bytes());
        assert!(Cfb::open(&d).is_err());
    }

    /// A 4 GiB stream under a 4 GiB mini-stream cutoff, with no mini FAT: an error, without
    /// reserving the stream's size first.
    #[test]
    fn hostile_mini_stream_size() {
        let mut d = vec![0u8; 512 * 3];
        d[..8].copy_from_slice(&MAGIC);
        d[26] = 3;
        d[28..30].copy_from_slice(&0xFFFEu16.to_le_bytes());
        d[30] = 9;
        d[32] = 6;
        d[44] = 1;
        d[48..52].copy_from_slice(&1u32.to_le_bytes());
        d[56..60].copy_from_slice(&u32::MAX.to_le_bytes());
        d[60..64].copy_from_slice(&END.to_le_bytes());
        d[68..72].copy_from_slice(&END.to_le_bytes());
        for i in 1..109 {
            d[76 + 4 * i..80 + 4 * i].copy_from_slice(&FREE.to_le_bytes());
        }
        let fat = 512;
        d[fat..fat + 4].copy_from_slice(&0xFFFF_FFFDu32.to_le_bytes());
        d[fat + 4..fat + 8].copy_from_slice(&END.to_le_bytes());
        for i in 2..128 {
            d[fat + 4 * i..fat + 4 * i + 4].copy_from_slice(&FREE.to_le_bytes());
        }
        let dir = 1024;
        let entry = |d: &mut [u8], at: usize, name: &str, kind: u8, child: u32, size: u64| {
            for (i, u) in name.encode_utf16().enumerate() {
                d[at + 2 * i..at + 2 * i + 2].copy_from_slice(&u.to_le_bytes());
            }
            let len = (name.len() as u16 + 1) * 2;
            d[at + 64..at + 66].copy_from_slice(&len.to_le_bytes());
            d[at + 66] = kind;
            d[at + 68..at + 72].copy_from_slice(&NONE.to_le_bytes());
            d[at + 72..at + 76].copy_from_slice(&NONE.to_le_bytes());
            d[at + 76..at + 80].copy_from_slice(&child.to_le_bytes());
            d[at + 116..at + 120].copy_from_slice(&END.to_le_bytes());
            d[at + 120..at + 128].copy_from_slice(&size.to_le_bytes());
        };
        entry(&mut d, dir, "Root Entry", 5, 1, 0);
        entry(&mut d, dir + 128, "big", 2, NONE, 0xFFFF_FFF0);
        let c = Cfb::open(&d).unwrap();
        assert_eq!(c.entry("big").map(|e| e.size), Some(0xFFFF_FFF0));
        assert!(c.read("big").is_err());
    }

    /// Found against olefile (the oracle samples: Apache POI's test files): a storage named ""
    /// (Notes.ole2) keeps its children under "/name"; an entry of an invalid object type
    /// reached through a sibling link (fuzzed Outlook files) is not listed; a mini stream
    /// whose chain runs past the file's end no longer fails the whole file.
    #[test]
    fn olefile_regressions() {
        let mut d = vec![0u8; 512 * 4];
        d[..8].copy_from_slice(&MAGIC);
        d[26] = 3;
        d[28..30].copy_from_slice(&0xFFFEu16.to_le_bytes());
        d[30] = 9;
        d[32] = 6;
        d[44] = 1;
        d[48..52].copy_from_slice(&1u32.to_le_bytes());
        d[56..60].copy_from_slice(&4096u32.to_le_bytes());
        d[60..64].copy_from_slice(&END.to_le_bytes());
        d[68..72].copy_from_slice(&END.to_le_bytes());
        for i in 1..109 {
            d[76 + 4 * i..80 + 4 * i].copy_from_slice(&FREE.to_le_bytes());
        }
        let fat = 512;
        for i in 0..128 {
            d[fat + 4 * i..fat + 4 * i + 4].copy_from_slice(&FREE.to_le_bytes());
        }
        d[fat..fat + 4].copy_from_slice(&0xFFFF_FFFDu32.to_le_bytes());
        d[fat + 4..fat + 8].copy_from_slice(&END.to_le_bytes());
        // The mini stream starts at sector 2, whose next link points far past the file.
        d[fat + 8..fat + 12].copy_from_slice(&900u32.to_le_bytes());
        let dir = 1024;
        let entry = |d: &mut [u8], at: usize, name: &str, kind: u8, child: u32, right: u32| {
            for (i, u) in name.encode_utf16().enumerate() {
                d[at + 2 * i..at + 2 * i + 2].copy_from_slice(&u.to_le_bytes());
            }
            let len = (name.len() as u16 + 1) * 2;
            d[at + 64..at + 66].copy_from_slice(&len.to_le_bytes());
            d[at + 66] = kind;
            d[at + 68..at + 72].copy_from_slice(&NONE.to_le_bytes());
            d[at + 72..at + 76].copy_from_slice(&right.to_le_bytes());
            d[at + 76..at + 80].copy_from_slice(&child.to_le_bytes());
            d[at + 116..at + 120].copy_from_slice(&END.to_le_bytes());
        };
        entry(&mut d, dir, "Root Entry", 5, 1, NONE);
        entry(&mut d, dir + 128, "", 1, 2, NONE);
        entry(&mut d, dir + 256, "a", 2, NONE, 3);
        entry(&mut d, dir + 384, "junk", 0xFF, NONE, NONE);
        // Root: a 1024-byte mini stream from sector 2.
        d[dir + 116..dir + 120].copy_from_slice(&2u32.to_le_bytes());
        d[dir + 120..dir + 128].copy_from_slice(&1024u64.to_le_bytes());
        let c = Cfb::open(&d).unwrap();
        let paths: Vec<&str> = c.list().map(|(p, _)| p).collect();
        assert_eq!(paths, ["", "/a"]);
        assert!(c.exists("/A") && !c.exists("a") && !c.exists("/junk"));
        assert_eq!(c.mini_sectors, [2]);
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    /// A file of `dirs` directory sectors (from sector 1), then a mini FAT sector whose
    /// first mini sector loops on itself, then a one-sector mini stream; `cutoff` is the
    /// mini-stream cutoff. `entries` fills the directory (root first).
    fn file(dirs: u32, cutoff: u32, entries: &[(String, u8, u32, u32, u64)]) -> Vec<u8> {
        let (mfat, ms) = (dirs + 1, dirs + 2);
        let mut d = vec![0u8; 512 * (ms as usize + 2)];
        d[..8].copy_from_slice(&MAGIC);
        d[26] = 3;
        d[28..30].copy_from_slice(&0xFFFEu16.to_le_bytes());
        d[30] = 9;
        d[32] = 6;
        d[44] = 1;
        d[48..52].copy_from_slice(&1u32.to_le_bytes());
        d[56..60].copy_from_slice(&cutoff.to_le_bytes());
        d[60..64].copy_from_slice(&mfat.to_le_bytes());
        d[64..68].copy_from_slice(&1u32.to_le_bytes());
        d[68..72].copy_from_slice(&END.to_le_bytes());
        for i in 1..109 {
            d[76 + 4 * i..80 + 4 * i].copy_from_slice(&FREE.to_le_bytes());
        }
        let fat = |d: &mut [u8], s: u32, v: u32| {
            let at = 512 + 4 * s as usize;
            d[at..at + 4].copy_from_slice(&v.to_le_bytes());
        };
        for s in 0..128 {
            fat(&mut d, s, FREE);
        }
        fat(&mut d, 0, 0xFFFF_FFFD);
        for s in 1..=dirs {
            fat(&mut d, s, if s == dirs { END } else { s + 1 });
        }
        fat(&mut d, mfat, END);
        fat(&mut d, ms, END);
        let mf = 512 * (mfat as usize + 1);
        for i in 0..128 {
            let v = if i == 0 { 0 } else { FREE };
            d[mf + 4 * i..mf + 4 * i + 4].copy_from_slice(&v.to_le_bytes());
        }
        for (k, (name, kind, child, start, size)) in entries.iter().enumerate() {
            let at = 1024 + 128 * k;
            for (i, u) in name.encode_utf16().enumerate() {
                d[at + 2 * i..at + 2 * i + 2].copy_from_slice(&u.to_le_bytes());
            }
            let len = (name.len() as u16 + 1) * 2;
            d[at + 64..at + 66].copy_from_slice(&len.to_le_bytes());
            d[at + 66] = *kind;
            d[at + 68..at + 72].copy_from_slice(&NONE.to_le_bytes());
            d[at + 72..at + 76].copy_from_slice(&NONE.to_le_bytes());
            d[at + 76..at + 80].copy_from_slice(&child.to_le_bytes());
            d[at + 116..at + 120].copy_from_slice(&start.to_le_bytes());
            d[at + 120..at + 128].copy_from_slice(&size.to_le_bytes());
        }
        d
    }

    /// A mini chain looping on itself, read for a size past the mini stream: it used to
    /// repeat the one mini sector as far as the mini FAT allowed.
    #[test]
    fn looping_mini_chain() {
        let root = ("Root Entry".to_owned(), 5, 1, 3, 512);
        let s = ("s".to_owned(), 2, NONE, 0, 1 << 20);
        let d = file(2, u32::MAX, &[root, s]);
        let c = Cfb::open(&d).unwrap();
        assert!(c.read("s").is_err());
    }

    /// Storages nested 400 deep: listed to 64 levels (paths used to grow with the depth,
    /// quadratic memory).
    #[test]
    fn deep_storages() {
        let n = 400u32;
        let dirs = n.div_ceil(4) + 1;
        let mut e = vec![("Root Entry".to_owned(), 5u8, 1u32, dirs + 2, 512u64)];
        for i in 1..n {
            e.push((
                format!("s{i}"),
                1,
                if i + 1 < n { i + 1 } else { NONE },
                0,
                0,
            ));
        }
        let d = file(dirs, 4096, &e);
        let c = Cfb::open(&d).unwrap();
        let deepest = c.list().map(|(p, _)| p.split('/').count()).max();
        assert_eq!(deepest, Some(MAX_NESTING + 1));
    }
}
