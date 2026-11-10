//! Part 10 file and data set parsing (PS3.10 7, PS3.5 7): the preamble and `DICM` prefix (or
//! none), the explicit-VR little-endian file meta information, then the data set in the
//! transfer syntax's encoding, with sequences and items of defined and undefined length and
//! encapsulated pixel data (PS3.5 A.4).
//!
//! Elements keep byte ranges into the buffer they were read from; values are interpreted on
//! demand. Nothing allocates in proportion to a declared length: only the element list grows,
//! and every element consumes at least its 8-byte header.

use std::ops::Range;

use crate::dict::{self, Vr};
use crate::{Error, Result};

pub(crate) const ITEM: u32 = 0xFFFE_E000;
pub(crate) const ITEM_END: u32 = 0xFFFE_E00D;
pub(crate) const SEQ_END: u32 = 0xFFFE_E0DD;
pub(crate) const PIXEL_DATA: u32 = 0x7FE0_0010;
/// Float Pixel Data (OF) and Double Float Pixel Data (OD).
pub(crate) const FLOAT_PIXEL_DATA: u32 = 0x7FE0_0008;
pub(crate) const DOUBLE_FLOAT_PIXEL_DATA: u32 = 0x7FE0_0009;
const UNDEFINED: u32 = 0xFFFF_FFFF;

/// Deepest sequence nesting accepted (real files rarely exceed 5).
const MAX_DEPTH: u32 = 24;
/// Elements per file, all levels (a sanity cap far above real files).
const MAX_ELEMENTS: usize = 4 << 20;

/// One data element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Element {
    pub tag: u32,
    pub vr: Vr,
    /// Value bytes (empty for sequences and encapsulated pixel data).
    pub value: Range<usize>,
    /// Items of a sequence.
    pub items: Vec<DataSet>,
    /// Encapsulated pixel data.
    pub fragments: Option<Box<Fragments>>,
}

/// Encapsulated pixel data: the Basic Offset Table and the fragments that follow it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Fragments {
    /// Value of the first item, the Basic Offset Table (may be empty).
    pub offset_table: Range<usize>,
    /// Values of the fragment items.
    pub items: Vec<Range<usize>>,
    /// Offset of each fragment's item header from the first fragment's item header (the
    /// origin of Basic and Extended Offset Table entries).
    pub item_offsets: Vec<u64>,
}

/// A data set (the file's, or a sequence item).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DataSet {
    /// Sorted by tag.
    pub elements: Vec<Element>,
    /// Values are big endian (Explicit VR Big Endian).
    pub big_endian: bool,
}

impl DataSet {
    pub fn get(&self, tag: u32) -> Option<&Element> {
        self.elements
            .binary_search_by_key(&tag, |e| e.tag)
            .ok()
            .map(|i| &self.elements[i])
    }

    /// First item of sequence `tag`.
    pub fn item(&self, tag: u32) -> Option<&DataSet> {
        self.get(tag)?.items.first()
    }
}

#[derive(Clone, Copy)]
struct Mode {
    explicit: bool,
    big: bool,
}

pub(crate) struct Parser<'a> {
    buf: &'a [u8],
    count: usize,
    /// The buffer ended inside an element.
    pub truncated: bool,
    /// Top-level tags from this one on are not read ([`header`]).
    stop: Option<u32>,
    /// The top level reached a tag at or after `stop`: that tag.
    pub stopped_at: Option<u32>,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(buf: &'a [u8]) -> Parser<'a> {
        Parser {
            buf,
            count: 0,
            truncated: false,
            stop: None,
            stopped_at: None,
        }
    }

    fn u16_at(&self, p: usize, big: bool) -> Option<u16> {
        let b: [u8; 2] = self.buf.get(p..p.checked_add(2)?)?.try_into().ok()?;
        Some(if big {
            u16::from_be_bytes(b)
        } else {
            u16::from_le_bytes(b)
        })
    }

    fn u32_at(&self, p: usize, big: bool) -> Option<u32> {
        let b: [u8; 4] = self.buf.get(p..p.checked_add(4)?)?.try_into().ok()?;
        Some(if big {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        })
    }

    fn tag_at(&self, p: usize, big: bool) -> Option<u32> {
        Some(((self.u16_at(p, big)? as u32) << 16) | self.u16_at(p + 2, big)? as u32)
    }

    /// Parses elements from `pos` up to `end`, stopping early (after it) at an item
    /// delimiter, or when `group_only` is set at the first element of another group.
    /// Returns the data set and the position after it.
    fn dataset(
        &mut self,
        mut pos: usize,
        end: usize,
        mode: Mode,
        depth: u32,
        group_only: Option<u16>,
    ) -> Result<(DataSet, usize)> {
        if depth > MAX_DEPTH {
            return Err(Error::Malformed("sequences nested too deeply"));
        }
        let mut elements: Vec<Element> = Vec::new();
        while pos < end {
            let Some(tag) = self.tag_at(pos, mode.big) else {
                // Fewer than 4 trailing bytes: padding.
                break;
            };
            if let Some(g) = group_only
                && (tag >> 16) as u16 != g
            {
                break;
            }
            if depth == 0
                && let Some(s) = self.stop
                && tag >= s
                && tag != ITEM_END
                && tag != SEQ_END
                && tag != ITEM
            {
                self.stopped_at = Some(tag);
                break;
            }
            if tag == ITEM_END {
                pos += 8;
                break;
            }
            if tag == SEQ_END || tag == ITEM {
                // A delimiter where an element belongs: the enclosing level handles it.
                break;
            }
            match self.element(pos, end, mode, depth) {
                Ok((e, next)) => {
                    self.count += 1;
                    if self.count > MAX_ELEMENTS {
                        return Err(Error::Malformed("too many data elements"));
                    }
                    elements.push(e);
                    if next <= pos {
                        break;
                    }
                    pos = next;
                }
                Err(Error::Truncated) if !elements.is_empty() || depth > 0 => {
                    self.truncated = true;
                    pos = end;
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        if !elements.windows(2).all(|w| w[0].tag < w[1].tag) {
            // Out-of-order or repeated tags (broken writers): keep the first of each.
            elements.sort_by_key(|e| e.tag);
            elements.dedup_by_key(|e| e.tag);
        }
        Ok((
            DataSet {
                elements,
                big_endian: mode.big,
            },
            pos,
        ))
    }

    /// One element at `pos`; returns it and the position after it.
    fn element(
        &mut self,
        pos: usize,
        end: usize,
        mode: Mode,
        depth: u32,
    ) -> Result<(Element, usize)> {
        let tag = self.tag_at(pos, mode.big).ok_or(Error::Truncated)?;
        let (vr, len, mut p) = if mode.explicit {
            let b = self.buf.get(pos + 4..pos + 6).ok_or(Error::Truncated)?;
            match Vr::from_bytes([b[0], b[1]]) {
                Some(vr) if vr.long_length() => {
                    let len = self.u32_at(pos + 8, mode.big).ok_or(Error::Truncated)?;
                    (vr, len, pos + 12)
                }
                Some(vr) => {
                    let len = self.u16_at(pos + 6, mode.big).ok_or(Error::Truncated)? as u32;
                    (vr, len, pos + 8)
                }
                None => {
                    // Not a VR: an implicit-VR element inside an explicit data set (seen
                    // in the wild); read it as such.
                    let len = self.u32_at(pos + 4, mode.big).ok_or(Error::Truncated)?;
                    (dict::implicit_vr(tag), len, pos + 8)
                }
            }
        } else {
            let len = self.u32_at(pos + 4, mode.big).ok_or(Error::Truncated)?;
            (dict::implicit_vr(tag), len, pos + 8)
        };
        let mut e = Element {
            tag,
            vr,
            value: p..p,
            items: Vec::new(),
            fragments: None,
        };
        if len == UNDEFINED {
            if tag == PIXEL_DATA {
                let (f, next) = self.fragments(p, end, mode.big)?;
                e.fragments = Some(Box::new(f));
                return Ok((e, next));
            }
            // SQ, or UN / anything else of undefined length, which can only be a sequence
            // (UN content is implicit VR little endian, PS3.5 6.2.2).
            let m = if vr == Vr::SQ {
                mode
            } else {
                Mode {
                    explicit: false,
                    big: false,
                }
            };
            e.vr = Vr::SQ;
            let (items, next) = self.items(p, end, m, depth, true)?;
            e.items = items;
            return Ok((e, next));
        }
        let len = len as usize;
        let vend = p.checked_add(len).ok_or(Error::Truncated)?;
        if vend > end {
            if tag == PIXEL_DATA || vr == Vr::SQ {
                // Keep what is there; the pixel decoder reports the shortfall.
                self.truncated = true;
                e.value = p..end;
                return Ok((e, end));
            }
            return Err(Error::Truncated);
        }
        let sq_by_dict = vr == Vr::UN && dict::lookup(tag).is_some_and(|(v, _)| v == Vr::SQ);
        if vr == Vr::SQ || sq_by_dict {
            let m = if vr == Vr::SQ {
                mode
            } else {
                Mode {
                    explicit: false,
                    big: false,
                }
            };
            match self.items(p, vend, m, depth, false) {
                Ok((items, _)) => {
                    e.vr = Vr::SQ;
                    e.items = items;
                }
                Err(_) if sq_by_dict => {}
                Err(err) => return Err(err),
            }
            p = vend;
            e.value = p..p;
            return Ok((e, vend));
        }
        e.value = p..vend;
        Ok((e, vend))
    }

    /// Items of a sequence from `pos`: up to the sequence delimiter (`undefined`) or `end`.
    fn items(
        &mut self,
        mut pos: usize,
        end: usize,
        mode: Mode,
        depth: u32,
        undefined: bool,
    ) -> Result<(Vec<DataSet>, usize)> {
        let mut items = Vec::new();
        while pos < end {
            let Some(tag) = self.tag_at(pos, mode.big) else {
                self.truncated |= undefined;
                return Ok((items, end));
            };
            let len = self.u32_at(pos + 4, mode.big).ok_or(Error::Truncated)?;
            pos += 8;
            match tag {
                SEQ_END => return Ok((items, pos)),
                ITEM if len == UNDEFINED => {
                    let (ds, next) = self.dataset(pos, end, mode, depth + 1, None)?;
                    items.push(ds);
                    pos = next;
                }
                ITEM => {
                    let iend = pos.saturating_add(len as usize).min(end);
                    let (ds, _) = self.dataset(pos, iend, mode, depth + 1, None)?;
                    items.push(ds);
                    pos = iend;
                }
                ITEM_END => {}
                _ => return Err(Error::Malformed("expected a sequence item")),
            }
            self.count += 1;
            if self.count > MAX_ELEMENTS {
                return Err(Error::Malformed("too many data elements"));
            }
        }
        if undefined {
            self.truncated = true;
        }
        Ok((items, pos))
    }

    /// Encapsulated pixel data from `pos` (after the element header).
    fn fragments(&mut self, mut pos: usize, end: usize, big: bool) -> Result<(Fragments, usize)> {
        let mut f = Fragments::default();
        let mut first = true;
        let mut origin = 0usize;
        loop {
            let Some(tag) = self.tag_at(pos, big) else {
                self.truncated = true;
                return Ok((f, end));
            };
            let Some(len) = self.u32_at(pos + 4, big) else {
                self.truncated = true;
                return Ok((f, end));
            };
            match tag {
                SEQ_END => return Ok((f, pos + 8)),
                ITEM => {
                    let start = pos + 8;
                    let vend = start.saturating_add(len as usize);
                    let (vend, cut) = if vend > end {
                        (end, true)
                    } else {
                        (vend, false)
                    };
                    if first {
                        f.offset_table = start..vend;
                        first = false;
                        origin = vend;
                    } else {
                        f.item_offsets.push((pos - origin) as u64);
                        f.items.push(start..vend);
                        self.count += 1;
                        if self.count > MAX_ELEMENTS {
                            return Err(Error::Malformed("too many fragments"));
                        }
                    }
                    if cut {
                        self.truncated = true;
                        return Ok((f, end));
                    }
                    pos = vend;
                }
                _ => {
                    // Junk where an item belongs: end of the pixel data.
                    self.truncated = true;
                    return Ok((f, pos));
                }
            }
        }
    }
}

/// The file layout found by [`split`].
pub(crate) struct Layout {
    /// The file meta information group (empty without one).
    pub meta: DataSet,
    /// Offset of the data set (after the meta information).
    pub body: usize,
}

/// Whether `b` is the start of an explicit-VR element header in little endian with a known
/// VR, in group `group`.
fn explicit_header(b: &[u8], group: u16) -> bool {
    b.len() >= 6
        && u16::from_le_bytes([b[0], b[1]]) == group
        && Vr::from_bytes([b[4], b[5]]).is_some()
}

/// Cheap content check: `DICM` after the 128-byte preamble, or a preamble-less file starting
/// with a plausible file meta (0002) or identifying (0008) group element.
pub fn sniff(data: &[u8]) -> bool {
    if data.get(128..132) == Some(b"DICM") {
        return true;
    }
    no_preamble_start(data)
}

/// A preamble-less file: the first two elements are in group 0002 or 0008, ascending, and
/// parse as explicit or implicit VR little endian elements of plausible length.
fn no_preamble_start(data: &[u8]) -> bool {
    if data.len() < 16 {
        return false;
    }
    let group = u16::from_le_bytes([data[0], data[1]]);
    if group != 0x0002 && group != 0x0008 {
        return false;
    }
    let elem = u16::from_le_bytes([data[2], data[3]]);
    if elem > 0x0020 {
        return false;
    }
    let explicit = Vr::from_bytes([data[4], data[5]]);
    let (next, vr_ok) = match explicit {
        Some(vr) if vr.long_length() => (
            12 + u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize,
            true,
        ),
        Some(_) => (8 + u16::from_le_bytes([data[6], data[7]]) as usize, true),
        None => (
            8usize
                .saturating_add(u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize),
            false,
        ),
    };
    if next > 4096 || next + 8 > data.len() {
        return false;
    }
    let tag2 = (
        u16::from_le_bytes([data[next], data[next + 1]]),
        u16::from_le_bytes([data[next + 2], data[next + 3]]),
    );
    let ascending = (tag2.0 == group && tag2.1 > elem) || (tag2.0 > group && tag2.0 <= 0x0010);
    ascending && (!vr_ok || Vr::from_bytes([data[next + 4], data[next + 5]]).is_some())
}

/// Finds and parses the file meta information.
pub(crate) fn split(data: &[u8]) -> Result<Layout> {
    let start = if data.get(128..132) == Some(b"DICM") {
        132
    } else if no_preamble_start(data) {
        0
    } else if data.get(0..4) == Some(b"DICM") {
        // Preamble dropped but prefix kept (rare).
        4
    } else {
        return Err(Error::NotDicom);
    };
    if !data
        .get(start..)
        .is_some_and(|b| b.len() >= 8 && b[..2] == [2, 0])
    {
        // No meta information group: the data set starts here.
        return Ok(Layout {
            meta: DataSet::default(),
            body: start,
        });
    }
    let mut p = Parser::new(data);
    let explicit = explicit_header(&data[start..], 0x0002);
    let (meta, body) = p.dataset(
        start,
        data.len(),
        Mode {
            explicit,
            big: false,
        },
        0,
        Some(0x0002),
    )?;
    Ok(Layout { meta, body })
}

/// Parses the data set in `buf[pos..]`.
pub(crate) fn body(
    buf: &[u8],
    pos: usize,
    explicit: bool,
    big: bool,
) -> Result<(DataSet, bool, bool)> {
    let mut p = Parser::new(buf);
    // A file whose syntax says explicit but whose first element has no VR is implicit (and
    // the reverse): trust the bytes.
    let explicit = match buf.get(pos + 4..pos + 6) {
        Some(b) if pos + 8 <= buf.len() => {
            let looks_explicit = Vr::from_bytes([b[0], b[1]]).is_some();
            if explicit != looks_explicit && !big {
                looks_explicit
            } else {
                explicit
            }
        }
        _ => explicit,
    };
    let (ds, _) = p.dataset(pos, buf.len(), Mode { explicit, big }, 0, None)?;
    Ok((ds, p.truncated, explicit))
}

/// What [`header`] read.
pub(crate) struct Header {
    pub dataset: DataSet,
    /// The first top-level tag at or after the stop tag, when the prefix reached one.
    pub stopped_at: Option<u32>,
}

/// Parses the data set in `buf[pos..]` up to (not including) the first top-level element
/// whose tag is `stop` or later: the header before the pixel data, without reading it.
pub(crate) fn header(
    buf: &[u8],
    pos: usize,
    explicit: bool,
    big: bool,
    stop: u32,
) -> Result<Header> {
    let mut p = Parser::new(buf);
    p.stop = Some(stop);
    let explicit = match buf.get(pos + 4..pos + 6) {
        Some(b) if pos + 8 <= buf.len() && !big => Vr::from_bytes([b[0], b[1]]).is_some(),
        _ => explicit,
    };
    let (dataset, _) = p.dataset(pos, buf.len(), Mode { explicit, big }, 0, None)?;
    Ok(Header {
        dataset,
        stopped_at: p.stopped_at,
    })
}

// ---------------------------------------------------------------------------------- values

/// Typed access to element values in the buffer the data set was read from.
#[derive(Clone, Copy)]
pub struct Values<'a> {
    pub buf: &'a [u8],
    pub big: bool,
}

impl<'a> Values<'a> {
    pub fn bytes(&self, e: &Element) -> &'a [u8] {
        self.buf.get(e.value.clone()).unwrap_or(&[])
    }

    /// Text value with padding removed (raw bytes; see [`crate::text`] for decoding).
    pub fn text_bytes(&self, e: &Element) -> &'a [u8] {
        let b = self.bytes(e);
        let end = b
            .iter()
            .rposition(|&c| c != b' ' && c != 0)
            .map_or(0, |i| i + 1);
        let start = b[..end].iter().position(|&c| c != b' ').unwrap_or(end);
        &b[start..end]
    }

    /// The `i`th backslash-separated text value, trimmed.
    pub fn text_n(&self, e: &Element, i: usize) -> Option<&'a str> {
        let b = self.text_bytes(e);
        let s = std::str::from_utf8(b).ok()?;
        s.split('\\')
            .nth(i)
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }

    pub fn u16s(&self, e: &Element) -> impl Iterator<Item = u16> + 'a {
        let big = self.big;
        self.bytes(e).chunks_exact(2).map(move |c| {
            if big {
                u16::from_be_bytes([c[0], c[1]])
            } else {
                u16::from_le_bytes([c[0], c[1]])
            }
        })
    }

    pub fn u32s(&self, e: &Element) -> impl Iterator<Item = u32> + 'a {
        let big = self.big;
        self.bytes(e).chunks_exact(4).map(move |c| {
            let b = [c[0], c[1], c[2], c[3]];
            if big {
                u32::from_be_bytes(b)
            } else {
                u32::from_le_bytes(b)
            }
        })
    }

    pub fn u64s(&self, e: &Element) -> impl Iterator<Item = u64> + 'a {
        let big = self.big;
        self.bytes(e).chunks_exact(8).map(move |c| {
            let b: [u8; 8] = c.try_into().unwrap_or([0; 8]);
            if big {
                u64::from_be_bytes(b)
            } else {
                u64::from_le_bytes(b)
            }
        })
    }

    /// First value of a binary integer element (US, SS, UL, SL; also IS text).
    pub fn int(&self, e: &Element) -> Option<i64> {
        match e.vr {
            Vr::US | Vr::OW => self.u16s(e).next().map(i64::from),
            Vr::SS => self.u16s(e).next().map(|v| v as i16 as i64),
            Vr::UL | Vr::OL => self.u32s(e).next().map(i64::from),
            Vr::SL => self.u32s(e).next().map(|v| v as i32 as i64),
            Vr::IS | Vr::DS => self.number(e, 0).map(|v| v as i64),
            Vr::UN | Vr::OB => match self.bytes(e).len() {
                2 => self.u16s(e).next().map(i64::from),
                4 => self.u32s(e).next().map(i64::from),
                _ => None,
            },
            _ => None,
        }
    }

    /// The `i`th numeric value: DS / IS text, or binary FL / FD / integers.
    pub fn number(&self, e: &Element, i: usize) -> Option<f64> {
        match e.vr {
            Vr::DS | Vr::IS | Vr::UN | Vr::LO | Vr::SH | Vr::CS => {
                let v: f64 = self.text_n(e, i)?.parse().ok()?;
                v.is_finite().then_some(v)
            }
            Vr::FD => {
                let b = self.bytes(e).get(i * 8..i * 8 + 8)?;
                let v = if self.big {
                    f64::from_be_bytes(b.try_into().ok()?)
                } else {
                    f64::from_le_bytes(b.try_into().ok()?)
                };
                v.is_finite().then_some(v)
            }
            Vr::FL => {
                let b = self.bytes(e).get(i * 4..i * 4 + 4)?;
                let v = if self.big {
                    f32::from_be_bytes(b.try_into().ok()?)
                } else {
                    f32::from_le_bytes(b.try_into().ok()?)
                } as f64;
                v.is_finite().then_some(v)
            }
            Vr::US => self.u16s(e).nth(i).map(f64::from),
            Vr::SS => self.u16s(e).nth(i).map(|v| v as i16 as f64),
            Vr::UL => self.u32s(e).nth(i).map(f64::from),
            Vr::SL => self.u32s(e).nth(i).map(|v| v as i32 as f64),
            _ => None,
        }
    }

    /// Number of backslash-separated values of a text element.
    pub fn count(&self, e: &Element) -> usize {
        let b = self.text_bytes(e);
        if b.is_empty() {
            0
        } else {
            b.iter().filter(|&&c| c == b'\\').count() + 1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniff_needs_structure() {
        let mut f = vec![0u8; 128];
        f.extend(b"DICM");
        assert!(sniff(&f));
        assert!(!sniff(b"\x08\x00\x05\x00garbage-garbage-garbage"));
        // (0008,0005) CS "ISO_IR 100" then (0008,0008).
        let mut p = vec![8, 0, 5, 0, b'C', b'S', 10, 0];
        p.extend(b"ISO_IR 100");
        p.extend([8, 0, 8, 0, b'C', b'S', 2, 0, b'A', b'B']);
        assert!(sniff(&p));
    }
}
