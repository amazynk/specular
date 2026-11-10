//! Metadata of Photoshop and Canon CR3 files, found from headers alone (see
//! [`crate::exif_locate`] for the other formats).
//!
//! * PSD/PSB: the image resources section (after the 26-byte header and the colour mode data)
//!   holds the ICC profile (resource 1039), the Exif TIFF structure (1058, else 1059) and the
//!   XMP packet (1060). Only the resource headers and the wanted resources are read.
//! * CR3 (ISO-BMFF, major brand `crx `): the Canon `uuid` box in `moov` holds four TIFF
//!   structures, CMT1 (IFD0), CMT2 (the Exif IFD), CMT3 (the maker note) and CMT4 (the GPS IFD),
//!   each with its own header and offsets. They are merged into one little-endian TIFF whose
//!   IFD0 points at the Exif and GPS IFDs, as a JPEG's would, and whose Exif IFD carries a
//!   maker note made of the few CMT3 tags the info panel reads (never a serial number); the
//!   XMP packet is the body of a top-level `uuid` box.

use std::borrow::Cow;

use crate::exif_locate::{At, MAX_EXIF, MAX_STEPS, MAX_XMP};

pub(crate) fn is_psd(h: &[u8]) -> bool {
    h.starts_with(b"8BPS\0\x01") || h.starts_with(b"8BPS\0\x02")
}

/// Whether `h` (the head of a file) is a Canon CR3: an ISO-BMFF `ftyp` of major brand `crx `.
pub fn is_cr3(h: &[u8]) -> bool {
    h.get(4..12) == Some(b"ftypcrx ")
}

/// Image resource ids.
pub(crate) const PSD_ICC: u16 = 1039;
pub(crate) const PSD_EXIF1: u16 = 1058;
pub(crate) const PSD_EXIF3: u16 = 1059;
pub(crate) const PSD_XMP: u16 = 1060;

/// The body of the first image resource `id` of a PSD/PSB, if non-empty and at most `max`
/// bytes.
pub(crate) fn psd_resource<'a>(s: &impl At<'a>, id: u16, max: u64) -> Option<Cow<'a, [u8]>> {
    let (off, len) = psd_resources(s)
        .into_iter()
        .find(|r| r.0 == id)
        .map(|r| (r.1, r.2))?;
    if len == 0 || len > max {
        return None;
    }
    s.at(off, len)
}

/// (id, body offset, body length) of the resources of interest (ICC, Exif, XMP), in file
/// order.
fn psd_resources<'a>(s: &impl At<'a>) -> Vec<(u16, u64, u64)> {
    let mut out = Vec::new();
    let be32 = |b: &[u8]| u64::from(u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    let Some(cm) = s.at(26, 4) else { return out };
    let Some(at) = 30u64.checked_add(be32(&cm)) else {
        return out;
    };
    let Some(len) = s.at(at, 4) else { return out };
    let mut i = at + 4;
    let Some(end) = i.checked_add(be32(&len)) else {
        return out;
    };
    for _ in 0..MAX_STEPS {
        // Signature, id, name length.
        let Some(h) = s.at(i, 7).filter(|_| i + 7 <= end) else {
            break;
        };
        if !matches!(&h[..4], b"8BIM" | b"MeSa" | b"AgHg" | b"PHUT" | b"DCSR") {
            break;
        }
        let id = u16::from_be_bytes([h[4], h[5]]);
        // The Pascal name (length byte included) is padded to an even size.
        let name = (1 + u64::from(h[6]) + 1) & !1;
        let size_at = i + 6 + name;
        let Some(n) = s.at(size_at, 4) else { break };
        let n = be32(&n);
        let body = size_at + 4;
        let Some(next) = body.checked_add(n).map(|e| e + (n & 1)) else {
            break;
        };
        if body + n > end {
            break;
        }
        if matches!(id, PSD_ICC | PSD_EXIF1 | PSD_EXIF3 | PSD_XMP) && !out.iter().any(|r| r.0 == id)
        {
            out.push((id, body, n));
        }
        i = next;
    }
    out
}

/// One box of an ISOBMFF file inside `[.., end)`: type, body offset and body length (a size of
/// 0 runs to `end`).
fn child<'a>(s: &impl At<'a>, i: u64, end: u64) -> Option<([u8; 4], u64, u64)> {
    if i.checked_add(8)? > end {
        return None;
    }
    let h = s.at(i, 8)?;
    let kind = [h[4], h[5], h[6], h[7]];
    let (size, head) = match u32::from_be_bytes([h[0], h[1], h[2], h[3]]) {
        0 => (end - i, 8),
        1 => {
            let l = s.at(i + 8, 8)?;
            (u64::from_be_bytes(l.as_ref().try_into().ok()?), 16)
        }
        n => (u64::from(n), 8),
    };
    let len = size.checked_sub(head)?;
    if i.checked_add(size)? > end {
        return None;
    }
    Some((kind, i + head, len))
}

/// The bodies of the boxes in `[start, end)` passing `want`, at most `cap` of them.
fn find_boxes<'a>(
    s: &impl At<'a>,
    start: u64,
    end: u64,
    mut want: impl FnMut([u8; 4], u64, u64) -> bool,
) -> Vec<([u8; 4], u64, u64)> {
    let mut out = Vec::new();
    let mut i = start;
    for _ in 0..MAX_STEPS {
        let Some((kind, body, len)) = child(s, i, end) else {
            break;
        };
        if want(kind, body, len) {
            out.push((kind, body, len));
        }
        i = body + len;
    }
    out
}

const CANON_UUID: [u8; 16] = [
    0x85, 0xc0, 0xb6, 0x87, 0x82, 0x0f, 0x11, 0xe0, 0x81, 0x11, 0xf4, 0xce, 0x46, 0x2b, 0x6a, 0x48,
];
const XMP_UUID: [u8; 16] = [
    0xbe, 0x7a, 0xcf, 0xcb, 0x97, 0xa9, 0x42, 0xe8, 0x9c, 0x71, 0x99, 0x94, 0x91, 0xe3, 0xaf, 0xac,
];

fn is_uuid<'a>(s: &impl At<'a>, kind: [u8; 4], body: u64, len: u64, uuid: &[u8; 16]) -> bool {
    &kind == b"uuid" && len >= 16 && s.at(body, 16).is_some_and(|u| u.as_ref() == uuid)
}

/// The Exif of a CR3: CMT1, CMT2, CMT4 and the [`CANON_KEEP`] tags of CMT3 merged into one
/// TIFF structure.
pub(crate) fn cr3_exif<'a>(s: &impl At<'a>) -> Option<Cow<'a, [u8]>> {
    let size = s.size();
    let moov = *find_boxes(s, 0, size, |k, _, _| &k == b"moov").first()?;
    let (_, mb, ml) = moov;
    let canon = *find_boxes(s, mb, mb + ml, |k, b, l| is_uuid(s, k, b, l, &CANON_UUID)).first()?;
    let (_, cb, cl) = canon;
    let mut cmt: [Option<Cow<'a, [u8]>>; 4] = [None, None, None, None];
    for (kind, body, len) in find_boxes(s, cb + 16, cb + cl, |k, _, _| {
        matches!(&k, b"CMT1" | b"CMT2" | b"CMT3" | b"CMT4")
    }) {
        let slot = &mut cmt[usize::from(kind[3] - b'1')];
        if slot.is_none() && len <= MAX_EXIF {
            *slot = s.at(body, len);
        }
    }
    let [ifd0, exif, maker, gps] = cmt;
    merge_tiff(
        ifd0.as_deref(),
        exif.as_deref(),
        maker.as_deref(),
        gps.as_deref(),
    )
    .map(Cow::Owned)
}

/// The Canon maker-note tags kept from CMT3: CameraSettings (focus mode, image stabilisation,
/// the lens's focal range and widest aperture) and LensModel. Everything else is left out,
/// serial numbers (0x000C, 0x0096) included.
pub(crate) const CANON_KEEP: [u16; 2] = [0x0001, 0x0095];

/// The XMP packet of a CR3: the body of the top-level XMP `uuid` box, without its trailing
/// padding (NULs or white space).
pub(crate) fn cr3_xmp<'a>(s: &impl At<'a>) -> Option<Cow<'a, [u8]>> {
    let (_, body, len) =
        *find_boxes(s, 0, s.size(), |k, b, l| is_uuid(s, k, b, l, &XMP_UUID)).first()?;
    if len - 16 > MAX_XMP {
        return None;
    }
    let x = s.at(body + 16, len - 16)?;
    let keep = x
        .iter()
        .rposition(|&b| b != 0 && !b.is_ascii_whitespace())
        .map_or(0, |p| p + 1);
    let x = match x {
        Cow::Borrowed(b) => Cow::Borrowed(&b[..keep]),
        Cow::Owned(mut v) => {
            v.truncate(keep);
            Cow::Owned(v)
        }
    };
    (!x.is_empty()).then_some(x)
}

/// One IFD entry, its value little-endian.
struct Entry {
    tag: u16,
    typ: u16,
    count: u32,
    value: Vec<u8>,
}

/// Tags that point into the structure they came from (sub-IFDs, the maker note), dropped when
/// merging.
const POINTERS: [u16; 5] = [0x8769, 0x8825, 0xA005, 0x014A, 0x927C];

/// The entries of IFD0 of the TIFF structure `t`, values converted to little-endian. Entries
/// of unknown type or with values outside `t` are skipped.
fn read_ifd(t: &[u8]) -> Option<Vec<Entry>> {
    let le = match t.get(..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let u16_at = |o: usize| {
        let b = t.get(o..o.checked_add(2)?)?;
        let a = [b[0], b[1]];
        Some(if le {
            u16::from_le_bytes(a)
        } else {
            u16::from_be_bytes(a)
        })
    };
    let u32_at = |o: usize| {
        let b = t.get(o..o.checked_add(4)?)?;
        let a = [b[0], b[1], b[2], b[3]];
        Some(if le {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        })
    };
    if u16_at(2)? != 42 {
        return None;
    }
    let ifd = u32_at(4)? as usize;
    let n = usize::from(u16_at(ifd)?).min(1024);
    let mut out: Vec<Entry> = Vec::with_capacity(n);
    // Value bytes held: entries repeating one large value must not copy it a thousand times.
    let mut held = 0usize;
    for k in 0..n {
        let e = ifd + 2 + 12 * k;
        let (Some(tag), Some(typ), Some(count)) = (u16_at(e), u16_at(e + 2), u32_at(e + 4)) else {
            break;
        };
        // Bytes per value, and the unit swapped (rationals are two LONGs).
        let (size, unit) = match typ {
            1 | 2 | 6 | 7 => (1, 1),
            3 | 8 => (2, 2),
            4 | 9 | 11 | 13 => (4, 4),
            5 | 10 => (8, 4),
            12 | 16 | 17 | 18 => (8, 8),
            _ => continue,
        };
        // The first of repeated tags is the one written.
        if POINTERS.contains(&tag) || out.iter().any(|e| e.tag == tag) {
            continue;
        }
        let Some(len) = (count as usize).checked_mul(size) else {
            continue;
        };
        if held.saturating_add(len) as u64 > MAX_EXIF {
            continue;
        }
        let at = if len <= 4 {
            e + 8
        } else {
            match u32_at(e + 8) {
                Some(o) => o as usize,
                None => continue,
            }
        };
        let Some(v) = at.checked_add(len).and_then(|end| t.get(at..end)) else {
            continue;
        };
        held += len;
        let mut value = v.to_vec();
        if !le {
            for u in value.chunks_exact_mut(unit) {
                u.reverse();
            }
        }
        out.push(Entry {
            tag,
            typ,
            count,
            value,
        });
    }
    Some(out)
}

/// One little-endian TIFF structure from the IFD0s of `ifd0` (as IFD0), `exif` (as the Exif
/// IFD), `maker` (the [`CANON_KEEP`] tags of it as the Exif IFD's maker note, an IFD at the
/// start of the note with offsets from the TIFF header, as Canon writes it in a JPEG) and `gps`
/// (as the GPS IFD), each a TIFF structure of its own. None when none parse or the result
/// would exceed [`MAX_EXIF`].
fn merge_tiff(
    ifd0: Option<&[u8]>,
    exif: Option<&[u8]>,
    maker: Option<&[u8]>,
    gps: Option<&[u8]>,
) -> Option<Vec<u8>> {
    let maker = maker
        .and_then(read_ifd)
        .map(|mut e| {
            e.retain(|e| CANON_KEEP.contains(&e.tag));
            e
        })
        .filter(|e| !e.is_empty());
    let mut exif = exif.and_then(read_ifd).filter(|e| !e.is_empty());
    let gps = gps.and_then(read_ifd).filter(|e| !e.is_empty());
    let mut main = ifd0.and_then(read_ifd).unwrap_or_default();
    if main.is_empty() && exif.is_none() && gps.is_none() {
        return None;
    }
    if maker.is_some() {
        // Count and offset are set once the note is written.
        exif.get_or_insert_default().push(Entry {
            tag: TAG_MAKER_NOTE,
            typ: 7,
            count: 0,
            value: vec![0; 4],
        });
    }
    for (tag, sub) in [(0x8769, &exif), (0x8825, &gps)] {
        if sub.is_some() {
            main.push(Entry {
                tag,
                typ: 4,
                count: 1,
                value: vec![0; 4],
            });
        }
    }
    let mut out = b"II*\0\x08\0\0\0".to_vec();
    let ptrs = write_ifd(&mut out, main);
    let mut note_field = None;
    for (tag, sub) in [(0x8769u16, exif), (0x8825, gps)] {
        let Some(sub) = sub else { continue };
        align(&mut out);
        let at = u32::try_from(out.len()).ok()?;
        if let Some(&(_, p)) = ptrs.iter().find(|(t, _)| *t == tag) {
            out[p..p + 4].copy_from_slice(&at.to_le_bytes());
        }
        let fields = write_ifd(&mut out, sub);
        note_field = note_field.or(fields
            .iter()
            .find(|(t, _)| *t == TAG_MAKER_NOTE)
            .map(|f| f.1));
    }
    if let (Some(note), Some(p)) = (maker, note_field) {
        align(&mut out);
        let at = out.len();
        write_ifd(&mut out, note);
        let len = u32::try_from(out.len() - at).ok()?;
        out[p - 4..p].copy_from_slice(&len.to_le_bytes());
        out[p..p + 4].copy_from_slice(&u32::try_from(at).ok()?.to_le_bytes());
    }
    (out.len() as u64 <= MAX_EXIF).then_some(out)
}

const TAG_MAKER_NOTE: u16 = 0x927C;

/// Pads `out` to an even length (IFDs start on a word boundary).
fn align(out: &mut Vec<u8>) {
    if out.len() % 2 == 1 {
        out.push(0);
    }
}

/// Appends an IFD (entries sorted by tag, values after it, word-aligned; no next IFD) and
/// returns where each entry's value field is.
fn write_ifd(out: &mut Vec<u8>, mut entries: Vec<Entry>) -> Vec<(u16, usize)> {
    entries.sort_by_key(|e| e.tag);
    entries.dedup_by_key(|e| e.tag);
    if out.len() % 2 == 1 {
        out.push(0);
    }
    let start = out.len();
    let mut data = start + 2 + 12 * entries.len() + 4;
    let mut tail = Vec::new();
    let mut fields = Vec::with_capacity(entries.len());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    for e in &entries {
        out.extend_from_slice(&e.tag.to_le_bytes());
        out.extend_from_slice(&e.typ.to_le_bytes());
        out.extend_from_slice(&e.count.to_le_bytes());
        fields.push((e.tag, out.len()));
        if e.value.len() <= 4 {
            let mut v = [0; 4];
            v[..e.value.len()].copy_from_slice(&e.value);
            out.extend_from_slice(&v);
        } else {
            out.extend_from_slice(&(data as u32).to_le_bytes());
            tail.extend_from_slice(&e.value);
            if tail.len() % 2 == 1 {
                tail.push(0);
            }
            data = start + 2 + 12 * entries.len() + 4 + tail.len();
        }
    }
    out.extend_from_slice(&[0; 4]);
    out.extend(tail);
    fields
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A TIFF structure (`le` or big-endian) whose IFD0 has `entries` (tag, type, count, value
    /// bytes in the file's byte order); values over 4 bytes go after the IFD.
    pub(crate) fn tiff(le: bool, entries: &[(u16, u16, u32, Vec<u8>)]) -> Vec<u8> {
        let u16b = |v: u16| if le { v.to_le_bytes() } else { v.to_be_bytes() };
        let u32b = |v: u32| if le { v.to_le_bytes() } else { v.to_be_bytes() };
        let mut t = if le {
            b"II*\0".to_vec()
        } else {
            b"MM\0*".to_vec()
        };
        t.extend_from_slice(&u32b(8));
        t.extend_from_slice(&u16b(entries.len() as u16));
        let mut data = 8 + 2 + 12 * entries.len() as u32 + 4;
        let mut tail = Vec::new();
        for (tag, typ, count, v) in entries {
            t.extend_from_slice(&u16b(*tag));
            t.extend_from_slice(&u16b(*typ));
            t.extend_from_slice(&u32b(*count));
            if v.len() <= 4 {
                let mut b = [0; 4];
                b[..v.len()].copy_from_slice(v);
                t.extend_from_slice(&b);
            } else {
                t.extend_from_slice(&u32b(data));
                tail.extend_from_slice(v);
                data += v.len() as u32;
            }
        }
        t.extend_from_slice(&[0; 4]);
        t.extend(tail);
        t
    }

    /// Rationals in the given byte order.
    pub(crate) fn rationals(le: bool, v: &[(u32, u32)]) -> Vec<u8> {
        let u32b = |v: u32| if le { v.to_le_bytes() } else { v.to_be_bytes() };
        v.iter()
            .flat_map(|&(n, d)| [u32b(n), u32b(d)])
            .flatten()
            .collect()
    }

    /// CMT1 (Make, Model, a stray Exif pointer), CMT2 (DateTimeOriginal, ISO) and CMT4 (a
    /// position: 48°30' N, 11°15' W), the GPS block big-endian.
    pub(crate) fn cmt_blocks() -> [Vec<u8>; 3] {
        let short = |v: u16| v.to_le_bytes().to_vec();
        let cmt1 = tiff(
            true,
            &[
                (0x010F, 2, 6, b"Canon\0".to_vec()),
                (0x0110, 2, 10, b"Canon EOS\0".to_vec()),
                (0x0112, 3, 1, short(1)),
                (0x8769, 4, 1, 999u32.to_le_bytes().to_vec()),
            ],
        );
        let cmt2 = tiff(
            true,
            &[
                (0x8827, 3, 1, short(400)),
                (0x9003, 2, 20, b"2024:05:06 07:08:09\0".to_vec()),
                (0x829A, 5, 1, rationals(true, &[(1, 250)])),
            ],
        );
        let cmt4 = tiff(
            false,
            &[
                (0x0000, 1, 4, vec![2, 3, 0, 0]),
                (0x0001, 2, 2, b"N\0".to_vec()),
                (0x0002, 5, 3, rationals(false, &[(48, 1), (30, 1), (0, 1)])),
                (0x0003, 2, 2, b"W\0".to_vec()),
                (
                    0x0004,
                    5,
                    3,
                    rationals(false, &[(11, 1), (1500, 100), (0, 1)]),
                ),
            ],
        );
        [cmt1, cmt2, cmt4]
    }

    /// A fake serial number planted in [`cmt3`] (0x000C) that must never be carried over.
    pub(crate) const SERIAL: u32 = 0xA1B2_C3D4;

    /// CMT3: a Canon maker note with CameraSettings (One-shot AF… AI Servo, IS on, a 35 mm
    /// prime at f/2), LensModel padded with junk after the NUL, and a serial number.
    pub(crate) fn cmt3() -> Vec<u8> {
        let mut cs = [0u16; 40];
        cs[0] = 80;
        cs[7] = 1;
        cs[23] = 35;
        cs[24] = 35;
        cs[25] = 1;
        cs[26] = 64;
        cs[34] = 1;
        let cs: Vec<u8> = cs.iter().flat_map(|v| v.to_le_bytes()).collect();
        tiff(
            true,
            &[
                (0x0001, 3, 40, cs),
                (0x000C, 4, 1, SERIAL.to_le_bytes().to_vec()),
                (0x0095, 2, 24, b"Fixture 35mm f/2\0\x07junk!\0".to_vec()),
            ],
        )
    }

    fn bx(kind: &[u8], body: &[u8]) -> Vec<u8> {
        let mut b = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        b.extend_from_slice(kind);
        b.extend_from_slice(body);
        b
    }

    pub(crate) const XMP: &[u8] = b"<x:xmpmeta xmlns:x='adobe:ns:meta/'/>";

    /// A CR3 skeleton: ftyp, moov with a track stub and the Canon uuid box (CNCV, CMT1–4,
    /// THMB), the XMP uuid box (padded with NULs), a preview uuid box, mdat.
    pub(crate) fn cr3(gps: bool) -> Vec<u8> {
        let [c1, c2, c4] = cmt_blocks();
        let mut canon = CANON_UUID.to_vec();
        canon.extend(bx(b"CNCV", b"CanonCR3_001/00.11.00/00.00.00"));
        canon.extend(bx(b"CMT1", &c1));
        canon.extend(bx(b"CMT2", &c2));
        canon.extend(bx(b"CMT3", &cmt3()));
        if gps {
            canon.extend(bx(b"CMT4", &c4));
        }
        canon.extend(bx(b"THMB", &[0; 24]));
        let mut moov = bx(b"mvhd", &[0; 100]);
        moov.extend(bx(b"uuid", &canon));
        moov.extend(bx(b"trak", &bx(b"tkhd", &[0; 84])));
        let mut xmp = XMP_UUID.to_vec();
        xmp.extend_from_slice(XMP);
        xmp.extend_from_slice(&[0; 13]);
        let mut prvw = [0xea, 0xf4, 0x2b, 0x5e, 0x1c, 0x98, 0x4b, 0x88].to_vec();
        prvw.extend_from_slice(&[0xb9, 0xfb, 0xb7, 0xdc, 0x40, 0x6e, 0x4d, 0x16]);
        prvw.extend_from_slice(&[0; 32]);
        let mut f = bx(b"ftyp", b"crx \0\0\0\x01crx isom");
        f.extend(bx(b"moov", &moov));
        f.extend(bx(b"uuid", &xmp));
        f.extend(bx(b"uuid", &prvw));
        f.extend(bx(b"mdat", &[0; 64]));
        f
    }

    #[test]
    fn merged_cr3_exif() {
        let f = cr3(true);
        let t = crate::exif_tiff(&f).expect("exif").into_owned();
        assert_eq!(
            crate::read_exif_tiff(std::io::Cursor::new(&f)).as_deref(),
            Some(&t[..])
        );
        let (lat, lon) = crate::exif_position(&t).expect("position");
        assert!(
            (lat - 48.5).abs() < 1e-9 && (lon + 11.25).abs() < 1e-9,
            "{lat} {lon}"
        );
        assert_eq!(
            crate::exif_date_taken(&t).as_deref(),
            Some("2024:05:06 07:08:09")
        );
        let (cam, has_gps) = crate::exif_facts(&t).expect("facts");
        assert!(has_gps);
        assert_eq!(cam.iso, Some(400));
        assert_eq!(cam.exposure_time, Some(1.0 / 250.0));
        assert_eq!(cam.camera, "Canon EOS");
        assert_eq!(crate::exif_orientation(&t), Some(1));
        // Without CMT4: no GPS pointer.
        let g = cr3(false);
        let t = crate::exif_tiff(&g).expect("exif");
        assert_eq!(crate::exif_facts(&t).map(|f| f.1), Some(false));
        assert!(crate::exif_position(&t).is_none());
        for n in 0..f.len() {
            let _ = crate::exif_tiff(&f[..n]);
            let _ = crate::read_exif_tiff(std::io::Cursor::new(&f[..n]));
            let _ = crate::embedded_metadata(&f[..n]);
        }
    }

    /// CMT3 becomes the Exif IFD's maker note with only the kept tags: the lens name from it
    /// (CMT2 has no LensModel), the CameraSettings intact, the serial number gone.
    #[test]
    fn cr3_maker_note() {
        let t = crate::exif_tiff(&cr3(false)).expect("exif").into_owned();
        let (cam, _) = crate::exif_facts(&t).expect("facts");
        assert_eq!(cam.lens.as_deref(), Some("Fixture 35mm f/2"));
        let serial = SERIAL.to_le_bytes();
        assert!(!t.windows(4).any(|w| w == serial), "serial leaked");
        // The maker note: an IFD with tags 0x0001 and 0x0095 at the offset 0x927C names.
        let u16a = |p: usize| u16::from_le_bytes([t[p], t[p + 1]]) as usize;
        let u32a = |p: usize| u32::from_le_bytes(t[p..p + 4].try_into().unwrap()) as usize;
        let find = |ifd: usize, tag: usize| {
            (0..u16a(ifd))
                .map(|i| ifd + 2 + 12 * i)
                .find(|&e| u16a(e) == tag)
        };
        let exif = u32a(find(8, 0x8769).expect("exif ifd") + 8);
        let mn = find(exif, 0x927C).expect("maker note");
        assert_eq!(u16a(mn + 2), 7);
        let note = u32a(mn + 8);
        assert!(note + u32a(mn + 4) <= t.len());
        let tags: Vec<usize> = (0..u16a(note)).map(|i| u16a(note + 2 + 12 * i)).collect();
        assert_eq!(tags, [0x0001, 0x0095]);
        let cs = u32a(find(note, 0x0001).unwrap() + 8);
        assert_eq!(u16a(cs + 2 * 7), 1);
        assert_eq!(u16a(cs + 2 * 26), 64);
        // A maker note alone (no CMT2) still gets an Exif IFD to hang from.
        let [a, _, _] = cmt_blocks();
        let t = merge_tiff(Some(&a), None, Some(&cmt3()), None).expect("merged");
        assert_eq!(
            crate::exif_facts(&t).and_then(|f| f.0.lens).as_deref(),
            Some("Fixture 35mm f/2")
        );
        // A note without kept tags adds nothing.
        let bare = merge_tiff(Some(&a), None, None, None).expect("merged");
        let serial_only = tiff(true, &[(0x000C, 4, 1, serial.to_vec())]);
        assert_eq!(
            merge_tiff(Some(&a), None, Some(&serial_only), None),
            Some(bare)
        );
    }

    #[test]
    fn cr3_xmp_trims_padding() {
        let f = cr3(true);
        let e = crate::embedded_metadata(&f);
        assert_eq!(e.xmp.as_deref(), Some(XMP));
        assert_eq!(e.icc, None);
    }

    /// Corrupt IFDs: every truncation and every single-byte change of the blocks merges or
    /// fails without panicking.
    #[test]
    fn merge_survives_damage() {
        let [a, b, c] = cmt_blocks();
        let d = cmt3();
        for blk in [&a, &b, &c, &d] {
            for n in 0..blk.len() {
                let _ = merge_tiff(
                    Some(&blk[..n]),
                    Some(&b[..n.min(b.len())]),
                    Some(&blk[..n]),
                    Some(blk),
                );
                let mut m = blk.clone();
                m[n] ^= 0xFF;
                if let Some(t) = merge_tiff(Some(&m), Some(&m), Some(&m), Some(&m)) {
                    let _ = crate::exif_position(&t);
                    let _ = crate::exif_facts(&t);
                }
            }
        }
        assert!(merge_tiff(None, None, None, None).is_none());
        assert!(merge_tiff(Some(b"junk"), None, Some(&d), None).is_none());
    }

    /// A PSD/PSB with colour mode data and resources: a decoy resource with an odd name and
    /// odd size, then ICC, Exif (1058), XMP.
    pub(crate) fn psd(version: u8, icc: &[u8], exif: &[u8], xmp: &[u8]) -> Vec<u8> {
        let mut f = b"8BPS\0".to_vec();
        f.push(version);
        f.extend_from_slice(&[0; 6]);
        f.extend_from_slice(&3u16.to_be_bytes());
        f.extend_from_slice(&1u32.to_be_bytes());
        f.extend_from_slice(&1u32.to_be_bytes());
        f.extend_from_slice(&8u16.to_be_bytes());
        f.extend_from_slice(&3u16.to_be_bytes());
        // Colour mode data.
        f.extend_from_slice(&6u32.to_be_bytes());
        f.extend_from_slice(&[1; 6]);
        let res = |id: u16, name: &[u8], data: &[u8]| {
            let mut r = b"8BIM".to_vec();
            r.extend_from_slice(&id.to_be_bytes());
            r.push(name.len() as u8);
            r.extend_from_slice(name);
            if (name.len() + 1) % 2 == 1 {
                r.push(0);
            }
            r.extend_from_slice(&(data.len() as u32).to_be_bytes());
            r.extend_from_slice(data);
            if data.len() % 2 == 1 {
                r.push(0);
            }
            r
        };
        let mut rs = res(1005, b"ab", &[7; 5]);
        rs.extend(res(PSD_ICC, b"", icc));
        rs.extend(res(PSD_EXIF1, b"x", exif));
        rs.extend(res(PSD_XMP, b"", xmp));
        f.extend_from_slice(&(rs.len() as u32).to_be_bytes());
        f.extend(rs);
        // Layer and mask information (PSB: 8-byte length), then image data.
        if version == 2 {
            f.extend_from_slice(&[0; 8]);
        } else {
            f.extend_from_slice(&[0; 4]);
        }
        f.extend_from_slice(&[0, 0, 1, 2, 3]);
        f
    }

    #[test]
    fn psd_resources_found() {
        let [_, _, gps] = cmt_blocks();
        let icc: Vec<u8> = (0..301u32).map(|i| i as u8).collect();
        for v in [1, 2] {
            let f = psd(v, &icc, &gps, XMP);
            let e = crate::embedded_metadata(&f);
            assert_eq!(e.xmp.as_deref(), Some(XMP));
            assert_eq!(e.icc.as_deref(), Some(&icc[..]));
            assert_eq!(crate::exif_tiff(&f).as_deref(), Some(&gps[..]));
            assert_eq!(
                crate::read_exif_tiff(std::io::Cursor::new(&f)).as_deref(),
                Some(&gps[..])
            );
            for n in 0..f.len() {
                let _ = crate::exif_tiff(&f[..n]);
                let _ = crate::embedded_metadata(&f[..n]);
            }
        }
        // Empty resources are absent.
        let f = psd(1, &[], &[], &[]);
        let e = crate::embedded_metadata(&f);
        assert_eq!(e, crate::Embedded::default());
        assert!(crate::exif_tiff(&psd(1, &[], b"not tiff", &[])).is_none());
    }
}

/// Synthetic hostile inputs: none may panic or blow up memory.
#[cfg(test)]
mod hostile {
    use super::*;

    /// A CR3 CMT block whose IFD0 has 1024 entries all pointing at one 3 MB value: one copy
    /// of each tag, and no more than MAX_EXIF bytes in all.
    #[test]
    fn repeated_large_values_are_held_once() {
        const BLOB: u32 = 3 << 20;
        let n = 1024usize;
        let mut t = b"II*\0".to_vec();
        t.extend_from_slice(&8u32.to_le_bytes());
        t.extend_from_slice(&(n as u16).to_le_bytes());
        let blob_at = (8 + 2 + 12 * n + 4) as u32;
        for k in 0..n {
            // Half repeat one tag, half are distinct tags.
            let tag = if k % 2 == 0 {
                0x9286
            } else {
                0xC000 + k as u16
            };
            t.extend_from_slice(&tag.to_le_bytes());
            t.extend_from_slice(&7u16.to_le_bytes());
            t.extend_from_slice(&BLOB.to_le_bytes());
            t.extend_from_slice(&blob_at.to_le_bytes());
        }
        t.extend_from_slice(&[0; 4]);
        t.resize((blob_at + BLOB) as usize, 0);
        let entries = read_ifd(&t).expect("ifd");
        let held: usize = entries.iter().map(|e| e.value.len()).sum();
        assert!(held as u64 <= MAX_EXIF, "{held} bytes held");
        assert_eq!(entries.iter().filter(|e| e.tag == 0x9286).count(), 1);
        let merged = merge_tiff(Some(&t), Some(&t), Some(&t), Some(&t));
        assert!(merged.is_none_or(|o| o.len() as u64 <= MAX_EXIF));
    }
}
