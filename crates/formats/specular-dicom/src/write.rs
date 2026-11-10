//! DICOM Part 10 writer: an editable data set ([`Object`]) built from a parsed file, and its
//! serialisation as Explicit VR Little Endian (PS3.5 7.1.2) with regenerated File Meta
//! Information (PS3.10 7.1).
//!
//! Values borrow the source buffer until edited; big-endian values are swapped to little
//! endian per VR; encapsulated pixel data (Basic Offset Table and fragments) is copied byte
//! for byte. Sequences and items are written with defined lengths. Group lengths (gggg,0000),
//! Length to End and command elements are dropped: they would be stale after editing.

use std::borrow::Cow;

use crate::dict::{self, Vr};
use crate::parse::{DataSet, ITEM, SEQ_END};
use crate::{Error, Result};

/// Implementation Class UID written into File Meta Information: Specular's, a UUID-derived
/// UID (PS3.5 B.2) fixed once for the application.
pub const IMPLEMENTATION_CLASS_UID: &str = "2.25.200916792433431372612062054342034996935";
/// Implementation Version Name (SH, at most 16 characters).
pub const IMPLEMENTATION_VERSION: &str = concat!("SPECULAR_", env!("CARGO_PKG_VERSION"));

/// Explicit VR Little Endian.
pub const EXPLICIT_LE: &str = "1.2.840.10008.1.2.1";

/// One element's value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value<'a> {
    /// Little-endian value bytes (unpadded or padded; the writer pads to even length).
    Bytes(Cow<'a, [u8]>),
    /// Sequence items.
    Items(Vec<Object<'a>>),
    /// Encapsulated pixel data: Basic Offset Table and fragments, written as they are.
    Fragments {
        offset_table: Cow<'a, [u8]>,
        fragments: Vec<Cow<'a, [u8]>>,
    },
}

/// One data element.
#[derive(Clone, Debug, PartialEq)]
pub struct Elem<'a> {
    pub tag: u32,
    pub vr: Vr,
    pub value: Value<'a>,
}

impl Elem<'_> {
    /// Value bytes (empty for sequences and encapsulated data).
    pub fn bytes(&self) -> &[u8] {
        match &self.value {
            Value::Bytes(b) => b,
            _ => &[],
        }
    }

    /// The value as text, trailing padding removed (lossy for non-UTF-8 bytes).
    pub fn text(&self) -> Cow<'_, str> {
        let b = self.bytes();
        let end = b
            .iter()
            .rposition(|&c| c != b' ' && c != 0)
            .map_or(0, |i| i + 1);
        String::from_utf8_lossy(&b[..end])
    }

    pub fn items(&self) -> &[Object<'_>] {
        match &self.value {
            Value::Items(i) => i,
            _ => &[],
        }
    }
}

/// An editable data set, elements sorted by tag.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Object<'a> {
    pub elements: Vec<Elem<'a>>,
}

impl<'a> Object<'a> {
    /// Copies the structure of a parsed data set; values borrow `buf` (the buffer the data
    /// set was parsed from). Big-endian values are swapped; elements of unknown VR (implicit
    /// VR files) take the de-identification table's or dictionary's VR where one is known.
    pub fn from_dataset(ds: &DataSet, buf: &'a [u8]) -> Object<'a> {
        let mut elements = Vec::with_capacity(ds.elements.len());
        // Native pixel data in big endian is in words of Bits Allocated (the decoder reads
        // 32- and 64-bit samples whole), not the 16-bit words of OW.
        let bits_allocated = ds
            .elements
            .iter()
            .find(|e| e.tag == 0x0028_0100)
            .and_then(|e| buf.get(e.value.clone()))
            .and_then(|b| b.get(..2))
            .map(|b| u16::from_be_bytes([b[0], b[1]]));
        for e in &ds.elements {
            let mut vr = e.vr;
            if vr == Vr::UN
                && let Some(v) = known_vr(e.tag)
            {
                vr = v;
            }
            let value = if let Some(f) = &e.fragments {
                Value::Fragments {
                    offset_table: Cow::Borrowed(buf.get(f.offset_table.clone()).unwrap_or(&[])),
                    fragments: f
                        .items
                        .iter()
                        .map(|r| Cow::Borrowed(buf.get(r.clone()).unwrap_or(&[])))
                        .collect(),
                }
            } else if e.vr == Vr::SQ {
                Value::Items(
                    e.items
                        .iter()
                        .map(|i| Object::from_dataset(i, buf))
                        .collect(),
                )
            } else {
                let b = buf.get(e.value.clone()).unwrap_or(&[]);
                if ds.big_endian {
                    let pixel = matches!(e.tag, 0x7FE0_0008 | 0x7FE0_0009 | 0x7FE0_0010);
                    match bits_allocated {
                        Some(n @ (32 | 64)) if pixel && e.vr == Vr::OW => {
                            Value::Bytes(swap_width(b, n as usize / 8))
                        }
                        _ => Value::Bytes(swap(b, e.vr)),
                    }
                } else {
                    Value::Bytes(Cow::Borrowed(b))
                }
            };
            elements.push(Elem {
                tag: e.tag,
                vr: if e.vr == Vr::SQ { Vr::SQ } else { vr },
                value,
            });
        }
        Object { elements }
    }

    fn index(&self, tag: u32) -> std::result::Result<usize, usize> {
        self.elements.binary_search_by_key(&tag, |e| e.tag)
    }

    pub fn get(&self, tag: u32) -> Option<&Elem<'a>> {
        self.index(tag).ok().map(|i| &self.elements[i])
    }

    pub fn get_mut(&mut self, tag: u32) -> Option<&mut Elem<'a>> {
        self.index(tag).ok().map(|i| &mut self.elements[i])
    }

    /// Text value of `tag`, trailing padding removed.
    pub fn text(&self, tag: u32) -> Option<Cow<'_, str>> {
        self.get(tag).map(|e| e.text())
    }

    /// Inserts or replaces an element.
    pub fn put(&mut self, tag: u32, vr: Vr, value: Value<'a>) {
        let e = Elem { tag, vr, value };
        match self.index(tag) {
            Ok(i) => self.elements[i] = e,
            Err(i) => self.elements.insert(i, e),
        }
    }

    /// Inserts or replaces a text (or other byte) value.
    pub fn put_bytes(&mut self, tag: u32, vr: Vr, bytes: impl Into<Vec<u8>>) {
        self.put(tag, vr, Value::Bytes(Cow::Owned(bytes.into())));
    }

    pub fn put_text(&mut self, tag: u32, vr: Vr, text: &str) {
        self.put_bytes(tag, vr, text.as_bytes());
    }

    pub fn remove(&mut self, tag: u32) -> Option<Elem<'a>> {
        self.index(tag).ok().map(|i| self.elements.remove(i))
    }
}

/// VR for an element read without one: the de-identification table's, else the dictionary's.
fn known_vr(tag: u32) -> Option<Vr> {
    crate::anon::table_vr(tag)
        .or_else(|| dict::lookup(tag).map(|(v, _)| v))
        .filter(|&v| v != Vr::UN)
}

/// Swaps a big-endian value to little endian for its VR.
fn swap(b: &[u8], vr: Vr) -> Cow<'_, [u8]> {
    let width = match vr {
        Vr::US | Vr::SS | Vr::OW | Vr::AT => 2,
        Vr::UL | Vr::SL | Vr::FL | Vr::OL | Vr::OF => 4,
        Vr::FD | Vr::OD | Vr::SV | Vr::UV | Vr::OV => 8,
        _ => return Cow::Borrowed(b),
    };
    swap_width(b, width)
}

fn swap_width(b: &[u8], width: usize) -> Cow<'_, [u8]> {
    let mut v = b.to_vec();
    for c in v.chunks_exact_mut(width) {
        c.reverse();
    }
    Cow::Owned(v)
}

/// Whether `tag` is never written: group lengths, Length to End, command elements.
fn dropped(tag: u32) -> bool {
    tag & 0xFFFF == 0 || tag == 0x0008_0001 || tag >> 16 == 0
}

fn pad_byte(vr: Vr) -> u8 {
    if vr.is_text() && vr != Vr::UI {
        b' '
    } else {
        0
    }
}

fn put_tag(out: &mut Vec<u8>, tag: u32) {
    out.extend_from_slice(&((tag >> 16) as u16).to_le_bytes());
    out.extend_from_slice(&(tag as u16).to_le_bytes());
}

fn vr_bytes(vr: Vr) -> [u8; 2] {
    let s = format!("{vr:?}");
    let b = s.as_bytes();
    [b[0], b[1]]
}

/// Writes an element header; returns the position of a 4-byte length to patch, if any.
fn header(out: &mut Vec<u8>, tag: u32, vr: Vr, len: u32) -> usize {
    put_tag(out, tag);
    out.extend_from_slice(&vr_bytes(vr));
    if vr.long_length() {
        out.extend_from_slice(&[0, 0]);
        let at = out.len();
        out.extend_from_slice(&len.to_le_bytes());
        at
    } else {
        out.extend_from_slice(&(len as u16).to_le_bytes());
        usize::MAX
    }
}

fn patch(out: &mut [u8], at: usize, from: usize) -> Result<()> {
    let len = u32::try_from(out.len() - from)
        .ok()
        .filter(|&l| l != u32::MAX)
        .ok_or(Error::TooLarge(out.len() as u64))?;
    out[at..at + 4].copy_from_slice(&len.to_le_bytes());
    Ok(())
}

/// Records where items of one top-level sequence start (for directory record offsets).
pub(crate) struct Track {
    pub tag: u32,
    pub offsets: Vec<usize>,
}

/// Serialises `obj` as Explicit VR Little Endian onto `out`.
pub fn encode(obj: &Object, out: &mut Vec<u8>) -> Result<()> {
    encode_tracked(obj, out, &mut None, 0)
}

pub(crate) fn encode_tracked(
    obj: &Object,
    out: &mut Vec<u8>,
    track: &mut Option<Track>,
    depth: u32,
) -> Result<()> {
    for e in &obj.elements {
        if dropped(e.tag) {
            continue;
        }
        match &e.value {
            Value::Bytes(b) => {
                let len = b.len() + (b.len() & 1);
                let mut vr = e.vr;
                if vr == Vr::SQ {
                    vr = Vr::UN;
                }
                if !vr.long_length() && len > 0xFFFE {
                    // Too long for a 16-bit length: only UN can hold it.
                    vr = Vr::UN;
                }
                let len = u32::try_from(len)
                    .ok()
                    .filter(|&l| l != u32::MAX)
                    .ok_or(Error::TooLarge(len as u64))?;
                header(out, e.tag, vr, len);
                out.extend_from_slice(b);
                if b.len() & 1 == 1 {
                    out.push(pad_byte(vr));
                }
            }
            Value::Items(items) => {
                let at = header(out, e.tag, Vr::SQ, 0);
                let start = out.len();
                let tracked = depth == 0 && track.as_ref().is_some_and(|t| t.tag == e.tag);
                for item in items {
                    if tracked && let Some(t) = track.as_mut() {
                        t.offsets.push(out.len());
                    }
                    put_tag(out, ITEM);
                    let iat = out.len();
                    out.extend_from_slice(&0u32.to_le_bytes());
                    let istart = out.len();
                    encode_tracked(item, out, track, depth + 1)?;
                    patch(out, iat, istart)?;
                }
                patch(out, at, start)?;
            }
            Value::Fragments {
                offset_table,
                fragments,
            } => {
                let vr = if e.vr == Vr::OW { Vr::OW } else { Vr::OB };
                header(out, e.tag, vr, u32::MAX);
                for f in std::iter::once(offset_table).chain(fragments) {
                    put_tag(out, ITEM);
                    let len = f.len() + (f.len() & 1);
                    let len = u32::try_from(len)
                        .ok()
                        .filter(|&l| l != u32::MAX)
                        .ok_or(Error::TooLarge(len as u64))?;
                    out.extend_from_slice(&len.to_le_bytes());
                    out.extend_from_slice(f);
                    if f.len() & 1 == 1 {
                        out.push(0);
                    }
                }
                put_tag(out, SEQ_END);
                out.extend_from_slice(&0u32.to_le_bytes());
            }
        }
    }
    Ok(())
}

/// The File Meta Information to write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileMeta {
    pub sop_class: String,
    pub sop_instance: String,
    pub transfer_syntax: String,
}

impl FileMeta {
    /// From the data set's SOP Class and Instance UIDs.
    pub fn of(obj: &Object, transfer_syntax: &str) -> FileMeta {
        FileMeta {
            sop_class: obj.text(0x0008_0016).unwrap_or_default().into_owned(),
            sop_instance: obj.text(0x0008_0018).unwrap_or_default().into_owned(),
            transfer_syntax: transfer_syntax.to_owned(),
        }
    }
}

/// A complete Part 10 file: preamble, "DICM", File Meta Information, then `obj` as Explicit
/// VR Little Endian. `meta.transfer_syntax` names the pixel data's encoding: Explicit VR
/// Little Endian for native pixel data, or the encapsulated syntax the fragments are in.
pub fn part10(obj: &Object, meta: &FileMeta) -> Result<Vec<u8>> {
    part10_tracked(obj, meta, &mut None)
}

pub(crate) fn part10_tracked(
    obj: &Object,
    meta: &FileMeta,
    track: &mut Option<Track>,
) -> Result<Vec<u8>> {
    let mut out = vec![0u8; 128];
    out.extend_from_slice(b"DICM");
    let mut m = Object::default();
    m.put_bytes(0x0002_0001, Vr::OB, [0u8, 1]);
    m.put_text(0x0002_0002, Vr::UI, &meta.sop_class);
    m.put_text(0x0002_0003, Vr::UI, &meta.sop_instance);
    m.put_text(0x0002_0010, Vr::UI, &meta.transfer_syntax);
    m.put_text(0x0002_0012, Vr::UI, IMPLEMENTATION_CLASS_UID);
    let version = &IMPLEMENTATION_VERSION[..IMPLEMENTATION_VERSION.len().min(16)];
    m.put_text(0x0002_0013, Vr::SH, version);
    let mut group = Vec::new();
    encode(&m, &mut group)?;
    header(&mut out, 0x0002_0000, Vr::UL, 4);
    out.extend_from_slice(&(group.len() as u32).to_le_bytes());
    out.extend_from_slice(&group);
    let body: Object = Object {
        elements: obj
            .elements
            .iter()
            .filter(|e| e.tag >> 16 != 2)
            .cloned()
            .collect(),
    };
    encode_tracked(&body, &mut out, track, 0)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Dicom;

    fn explicit_file(body: &[u8]) -> Vec<u8> {
        let obj = Object::default();
        let mut f = part10(
            &obj,
            &FileMeta {
                sop_class: "1.2.840.10008.5.1.4.1.1.7".into(),
                sop_instance: "1.2.3.4".into(),
                transfer_syntax: EXPLICIT_LE.into(),
            },
        )
        .unwrap();
        f.extend_from_slice(body);
        f
    }

    #[test]
    fn meta_round_trips() {
        let mut obj = Object::default();
        obj.put_text(0x0008_0016, Vr::UI, "1.2.840.10008.5.1.4.1.1.7");
        obj.put_text(0x0008_0018, Vr::UI, "1.2.3.4.5");
        obj.put_text(0x0010_0010, Vr::PN, "Doe^Jane");
        let f = part10(&obj, &FileMeta::of(&obj, EXPLICIT_LE)).unwrap();
        let d = Dicom::parse(&f).unwrap();
        let v = d.values(&f);
        let t = |ds: &DataSet, tag| {
            String::from_utf8_lossy(v.text_bytes(ds.get(tag).unwrap())).into_owned()
        };
        assert_eq!(t(&d.meta, 0x0002_0003), "1.2.3.4.5");
        assert_eq!(t(&d.meta, 0x0002_0010), EXPLICIT_LE);
        assert_eq!(t(&d.meta, 0x0002_0012), IMPLEMENTATION_CLASS_UID);
        assert_eq!(t(&d.dataset, 0x0010_0010), "Doe^Jane");
        // Group length counts the meta elements after it.
        let gl = u32::from_le_bytes(f[140..144].try_into().unwrap()) as usize;
        assert_eq!(&f[144 + gl..144 + gl + 4], &[8, 0, 0x16, 0]);
        assert!(IMPLEMENTATION_VERSION.len() <= 16);
    }

    #[test]
    fn sequences_and_fragments_round_trip() {
        // A data set with a nested sequence, an odd-length value and encapsulated pixels.
        let mut inner = Object::default();
        inner.put_text(0x0008_1150, Vr::UI, "1.2.3");
        inner.put_text(0x0008_1155, Vr::UI, "1.2.3.4");
        let mut obj = Object::default();
        obj.put_bytes(0x0028_0010, Vr::US, 1u16.to_le_bytes());
        obj.put_bytes(0x0028_0011, Vr::US, 1u16.to_le_bytes());
        obj.put_bytes(0x0028_0100, Vr::US, 8u16.to_le_bytes());
        obj.put(
            0x0008_1140,
            Vr::SQ,
            Value::Items(vec![inner.clone(), inner]),
        );
        obj.put_text(0x0008_103E, Vr::LO, "odd");
        obj.put(
            0x7FE0_0010,
            Vr::OB,
            Value::Fragments {
                offset_table: Cow::Owned(vec![0, 0, 0, 0]),
                fragments: vec![Cow::Owned(vec![1, 2, 3]), Cow::Owned(vec![4, 5])],
            },
        );
        let mut body = Vec::new();
        encode(&obj, &mut body).unwrap();
        let f = explicit_file(&body);
        let d = Dicom::parse(&f).unwrap();
        let back = Object::from_dataset(&d.dataset, d.buf(&f));
        assert_eq!(back.get(0x0008_1140).unwrap().items().len(), 2);
        assert_eq!(back.text(0x0008_103E).unwrap(), "odd");
        match &back.get(0x7FE0_0010).unwrap().value {
            Value::Fragments {
                offset_table,
                fragments,
            } => {
                assert_eq!(&offset_table[..], &[0, 0, 0, 0]);
                assert_eq!(&fragments[0][..], &[1, 2, 3, 0]);
                assert_eq!(&fragments[1][..], &[4, 5]);
            }
            v => panic!("{v:?}"),
        }
        // Writing what was read gives the same bytes.
        let mut again = Vec::new();
        encode(&back, &mut again).unwrap();
        assert_eq!(again, body);
    }

    #[test]
    fn big_endian_values_swap() {
        assert_eq!(&swap(&[0, 1, 0, 2], Vr::US)[..], &[1, 0, 2, 0]);
        assert_eq!(&swap(&[0, 0, 0, 1], Vr::UL)[..], &[1, 0, 0, 0]);
        assert_eq!(&swap(b"AB", Vr::CS)[..], b"AB");
    }

    #[test]
    fn group_lengths_dropped() {
        let mut obj = Object::default();
        obj.put_bytes(0x0008_0000, Vr::UL, 4u32.to_le_bytes());
        obj.put_text(0x0008_0060, Vr::CS, "CT");
        let mut out = Vec::new();
        encode(&obj, &mut out).unwrap();
        assert_eq!(out, b"\x08\x00\x60\x00CS\x02\x00CT");
    }
}
