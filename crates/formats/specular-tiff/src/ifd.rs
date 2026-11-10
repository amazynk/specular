//! TIFF / BigTIFF header and IFD parsing.

use crate::{Error, Result};

/// Upper bound on IFDs walked (main chain plus SubIFDs), against loops and bombs.
const MAX_IFDS: usize = 65_536;
/// Upper bound on entries in one IFD.
const MAX_ENTRIES: u64 = 4096;
/// Upper bound on values in one array tag (strip / tile offsets).
pub(crate) const MAX_VALUES: u64 = 1 << 24;
/// Values (array tag values read, plus virtual strips made from them) one file may cost
/// beyond one per byte of the file: many IFDs can share one large offset array, so the
/// per-tag cap alone does not bound the work and memory of a whole file.
const VALUE_SLACK: u64 = 4 * MAX_VALUES;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Entry {
    pub tag: u16,
    pub typ: u16,
    pub count: u64,
    /// File offset of the value bytes (inline values point into the entry itself).
    pub pos: u64,
    /// NDPI files over 4 GiB: the high 32 bits of an inline single LONG value (an offset),
    /// from the word the file stores after the IFD for each entry; 0 elsewhere.
    pub hi: u64,
}

pub(crate) struct Reader<'a> {
    pub data: &'a [u8],
    pub le: bool,
    pub big: bool,
    /// Hamamatsu NDPI over 4 GiB: classic TIFF whose next-IFD pointers are 8 bytes, followed
    /// by one 32-bit high word per entry that extends its value or value offset to 64 bits.
    pub ndpi: bool,
    /// Values charged so far against [`Reader::charge`]'s budget.
    pub spent: std::cell::Cell<u64>,
}

/// NDPI's source-lens tag, present in every IFD of a Hamamatsu file.
pub(crate) const NDPI_MAGNIFICATION: u16 = 65420;

pub(crate) fn type_size(typ: u16) -> Option<u64> {
    Some(match typ {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 | 13 => 4,
        5 | 10 | 12 | 16 | 17 | 18 => 8,
        _ => return None,
    })
}

impl<'a> Reader<'a> {
    /// Parse the header; returns the reader and the first IFD offset.
    pub fn new(data: &'a [u8]) -> Result<(Reader<'a>, u64)> {
        if data.len() < 8 {
            return Err(Error::Truncated("header"));
        }
        let le = match &data[..2] {
            b"II" => true,
            b"MM" => false,
            _ => return Err(Error::Malformed("not a TIFF file")),
        };
        let mut r = Reader {
            data,
            le,
            big: false,
            ndpi: false,
            spent: std::cell::Cell::new(0),
        };
        match r.u16(2)? {
            42 => {
                let first = r.u32(4)? as u64;
                let mut r = r.clone_with(false);
                // Only a file past 4 GiB can need 64-bit offsets; smaller NDPI files are
                // plain TIFF.
                if data.len() as u64 > u32::MAX as u64
                    && let Ok((entries, _)) = r.ifd(first)
                    && find(&entries, NDPI_MAGNIFICATION).is_some()
                {
                    r.ndpi = true;
                }
                Ok((r, first))
            }
            43 => {
                if r.u16(4)? != 8 {
                    return Err(Error::Malformed("BigTIFF offset size"));
                }
                r.big = true;
                let first = r.u64(8)?;
                Ok((r, first))
            }
            _ => Err(Error::Malformed("bad TIFF version")),
        }
    }

    fn clone_with(&self, big: bool) -> Reader<'a> {
        Reader {
            data: self.data,
            le: self.le,
            big,
            ndpi: self.ndpi,
            spent: self.spent.clone(),
        }
    }

    /// Account for `n` values read or made from this file's tags: at most one per byte of
    /// the file plus [`VALUE_SLACK`] in all, else [`Error::TooLarge`].
    pub fn charge(&self, n: u64) -> Result<()> {
        let total = self.spent.get().saturating_add(n);
        if total > (self.data.len() as u64).saturating_add(VALUE_SLACK) {
            return Err(Error::TooLarge);
        }
        self.spent.set(total);
        Ok(())
    }

    pub fn bytes(&self, pos: u64, len: u64) -> Result<&'a [u8]> {
        let end = pos.checked_add(len).ok_or(Error::Malformed("offset"))?;
        if end > self.data.len() as u64 {
            return Err(Error::Truncated("value outside file"));
        }
        Ok(&self.data[pos as usize..end as usize])
    }

    pub fn u16(&self, pos: u64) -> Result<u16> {
        let b: [u8; 2] = self.bytes(pos, 2)?.try_into().unwrap_or_default();
        Ok(if self.le {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        })
    }
    pub fn u32(&self, pos: u64) -> Result<u32> {
        let b: [u8; 4] = self.bytes(pos, 4)?.try_into().unwrap_or_default();
        Ok(if self.le {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    }
    pub fn u64(&self, pos: u64) -> Result<u64> {
        let b: [u8; 8] = self.bytes(pos, 8)?.try_into().unwrap_or_default();
        Ok(if self.le {
            u64::from_le_bytes(b)
        } else {
            u64::from_be_bytes(b)
        })
    }

    /// Read an IFD: its entries (sorted by tag) and the next IFD offset.
    pub fn ifd(&self, pos: u64) -> Result<(Vec<Entry>, u64)> {
        let (count, first, esize) = if self.big {
            (self.u64(pos)?, pos + 8, 20u64)
        } else {
            (self.u16(pos)? as u64, pos + 2, 12u64)
        };
        if count == 0 || count > MAX_ENTRIES {
            return Err(Error::Malformed("IFD entry count"));
        }
        let mut entries = Vec::with_capacity(count as usize);
        // NDPI: 8-byte next pointer, then a high word per entry.
        let next_pos = first + count * esize;
        let high = |i: u64| -> u64 {
            if self.ndpi {
                self.u32(next_pos + 8 + 4 * i).map_or(0, |v| v as u64)
            } else {
                0
            }
        };
        for i in 0..count {
            let e = first + i * esize;
            let tag = self.u16(e)?;
            let typ = self.u16(e + 2)?;
            let (cnt, vpos) = if self.big {
                (self.u64(e + 4)?, e + 12)
            } else {
                (self.u32(e + 4)? as u64, e + 8)
            };
            let Some(ts) = type_size(typ) else {
                continue; // unknown type: skip the entry, as libtiff does
            };
            let len = ts.saturating_mul(cnt);
            let inline = if self.big { 8 } else { 4 };
            let hi = high(i) << 32;
            let (pos, hi) = if len <= inline {
                // Only a single LONG (an offset) takes the high word.
                (
                    vpos,
                    if cnt == 1 && matches!(typ, 4 | 13) {
                        hi
                    } else {
                        0
                    },
                )
            } else if self.big {
                (self.u64(vpos)?, 0)
            } else {
                (self.u32(vpos)? as u64 | hi, 0)
            };
            entries.push(Entry {
                tag,
                typ,
                count: cnt,
                pos,
                hi,
            });
        }
        // A missing next pointer (truncated after the entries) just ends the chain.
        let next = if self.big || self.ndpi {
            self.u64(next_pos).unwrap_or(0)
        } else {
            self.u32(next_pos).map(|v| v as u64).unwrap_or(0)
        };
        entries.sort_by_key(|e| e.tag);
        Ok((entries, next))
    }

    /// All values of an integer tag as u64.
    pub fn uints(&self, e: &Entry) -> Result<Vec<u64>> {
        if e.count > MAX_VALUES {
            return Err(Error::TooLarge);
        }
        let ts = type_size(e.typ).unwrap_or(1);
        let raw = self.bytes(e.pos, ts * e.count)?;
        self.charge(e.count)?;
        let mut v = Vec::with_capacity(e.count as usize);
        match e.typ {
            1 | 6 | 7 => v.extend(raw.iter().map(|&b| b as u64)),
            3 | 8 => v.extend(raw.chunks_exact(2).map(|c| {
                let b = [c[0], c[1]];
                (if self.le {
                    u16::from_le_bytes(b)
                } else {
                    u16::from_be_bytes(b)
                }) as u64
            })),
            4 | 9 | 13 => v.extend(raw.chunks_exact(4).map(|c| {
                let b = [c[0], c[1], c[2], c[3]];
                (if self.le {
                    u32::from_le_bytes(b)
                } else {
                    u32::from_be_bytes(b)
                }) as u64
                    | e.hi
            })),
            16..=18 => v.extend(raw.chunks_exact(8).map(|c| {
                let mut b = [0u8; 8];
                b.copy_from_slice(c);
                if self.le {
                    u64::from_le_bytes(b)
                } else {
                    u64::from_be_bytes(b)
                }
            })),
            _ => return Err(Error::Malformed("non-integer tag type")),
        }
        Ok(v)
    }

    /// All values of a RATIONAL / SRATIONAL / FLOAT / DOUBLE tag (or an integer one) as
    /// f64.
    pub fn reals(&self, e: &Entry) -> Result<Vec<f64>> {
        if matches!(e.typ, 11 | 12) {
            if e.count > MAX_VALUES {
                return Err(Error::TooLarge);
            }
            let size = if e.typ == 11 { 4 } else { 8 };
            let raw = self.bytes(e.pos, size * e.count)?;
            self.charge(e.count)?;
            return Ok(raw
                .chunks_exact(size as usize)
                .map(|c| match *c {
                    [a, b, c, d] if self.le => f32::from_le_bytes([a, b, c, d]) as f64,
                    [a, b, c, d] => f32::from_be_bytes([a, b, c, d]) as f64,
                    _ => {
                        let mut b = [0u8; 8];
                        b.copy_from_slice(c);
                        if self.le {
                            f64::from_le_bytes(b)
                        } else {
                            f64::from_be_bytes(b)
                        }
                    }
                })
                .collect());
        }
        if !matches!(e.typ, 5 | 10) {
            return Ok(self.uints(e)?.into_iter().map(|v| v as f64).collect());
        }
        if e.count > MAX_VALUES {
            return Err(Error::TooLarge);
        }
        let raw = self.bytes(e.pos, 8 * e.count)?;
        self.charge(e.count)?;
        let word = |c: &[u8]| {
            let b = [c[0], c[1], c[2], c[3]];
            if self.le {
                u32::from_le_bytes(b)
            } else {
                u32::from_be_bytes(b)
            }
        };
        Ok(raw
            .chunks_exact(8)
            .map(|c| {
                let (n, d) = (word(&c[..4]), word(&c[4..]));
                let (n, d) = if e.typ == 10 {
                    (n as i32 as f64, d as i32 as f64)
                } else {
                    (n as f64, d as f64)
                };
                if d == 0.0 { 0.0 } else { n / d }
            })
            .collect())
    }

    pub fn uint(&self, e: &Entry) -> Result<u64> {
        let one = Entry { count: 1, ..*e };
        if e.count == 0 {
            return Err(Error::Malformed("empty tag"));
        }
        self.uints(&one).map(|v| v[0])
    }

    /// Byte range of a tag's value.
    pub fn range(&self, e: &Entry) -> Result<std::ops::Range<usize>> {
        let len = type_size(e.typ).unwrap_or(1).saturating_mul(e.count);
        self.bytes(e.pos, len)?;
        Ok(e.pos as usize..(e.pos + len) as usize)
    }
}

pub(crate) fn find(entries: &[Entry], tag: u16) -> Option<&Entry> {
    entries
        .binary_search_by_key(&tag, |e| e.tag)
        .ok()
        .map(|i| &entries[i])
}

/// Walk the IFD chain from `first`, calling `f` for each IFD offset. Stops on loops.
pub(crate) fn walk_chain(
    r: &Reader,
    first: u64,
    visited: &mut std::collections::HashSet<u64>,
    mut f: impl FnMut(u64, &[Entry]) -> Result<()>,
) -> Result<()> {
    let mut pos = first;
    while pos != 0 {
        if visited.len() >= MAX_IFDS || !visited.insert(pos) {
            break;
        }
        let (entries, next) = match r.ifd(pos) {
            Ok(v) => v,
            Err(e) if visited.len() == 1 => return Err(e),
            Err(_) => break, // a damaged later IFD ends the chain
        };
        f(pos, &entries)?;
        pos = next;
    }
    Ok(())
}
