//! Office 97–2003 compound files ([MS-CFB]) and their property sets ([MS-OLEPS]): the
//! summary information (`\x05SummaryInformation`: title, author, dates, counts and the
//! `PIDSI_THUMBNAIL` clipboard data) and the document summary information
//! (`\x05DocumentSummaryInformation`: category, company, slides, notes, lines, paragraphs).
//! Also recognises password-protected OOXML packages (`EncryptionInfo`, `EncryptedPackage`).

use specular_cfb::Cfb;

use crate::{App, Bytes, Document, Error, Family, MAX_PART, Preview, PreviewKind, Properties};

const SUMMARY: &str = "\u{5}SummaryInformation";
const DOC_SUMMARY: &str = "\u{5}DocumentSummaryInformation";

const VT_I2: u16 = 2;
const VT_I4: u16 = 3;
const VT_BOOL: u16 = 11;
const VT_UI2: u16 = 18;
const VT_UI4: u16 = 19;
const VT_I8: u16 = 20;
const VT_UI8: u16 = 21;
const VT_INT: u16 = 22;
const VT_UINT: u16 = 23;
const VT_LPSTR: u16 = 30;
const VT_LPWSTR: u16 = 31;
const VT_FILETIME: u16 = 64;
const VT_CF: u16 = 71;

const CF_METAFILEPICT: u32 = 3;
const CF_DIB: u32 = 8;
const CF_ENHMETAFILE: u32 = 14;

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        d.get(o..o.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        d.get(o..o.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn u64_at(d: &[u8], o: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        d.get(o..o.checked_add(8)?)?.try_into().ok()?,
    ))
}

/// A property value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Value<'a> {
    Str(String),
    Int(i64),
    /// FILETIME: 100 ns units since 1601 (a date), or a duration (`PIDSI_EDITTIME`).
    Time(u64),
    /// Clipboard data: the format tag (-1 Windows, -2 Macintosh, …), the format and the
    /// bytes after them.
    Clip(i32, u32, &'a [u8]),
}

/// The properties of a property set stream's first section, by identifier.
pub(crate) fn property_set(ps: &[u8]) -> Vec<(u32, Value<'_>)> {
    let mut out = Vec::new();
    if u16_at(ps, 0) != Some(0xFFFE) {
        return out;
    }
    let Some(off) = u32_at(ps, 28 + 16).map(|o| o as usize) else {
        return out;
    };
    let Some(sec) = ps.get(off..) else {
        return out;
    };
    let size = u32_at(sec, 0).map_or(0, |s| s as usize).min(sec.len());
    let sec = &sec[..size];
    let count = u32_at(sec, 4).unwrap_or(0).min(4096) as usize;
    let mut codepage = 1252u16;
    let mut entries = Vec::with_capacity(count);
    for j in 0..count {
        let (Some(id), Some(at)) = (u32_at(sec, 8 + j * 8), u32_at(sec, 12 + j * 8)) else {
            break;
        };
        entries.push((id, at as usize));
        if id == 1
            && let Some(cp) = u16_at(sec, at as usize + 4)
        {
            codepage = cp;
        }
    }
    for (id, at) in entries {
        if id <= 1 {
            continue;
        }
        if let Some(v) = value(sec, at, codepage) {
            out.push((id, v));
        }
    }
    out
}

fn value(sec: &[u8], at: usize, codepage: u16) -> Option<Value<'_>> {
    let ty = u16_at(sec, at)?;
    let v = at + 4;
    Some(match ty {
        VT_I2 => Value::Int(i64::from(u16_at(sec, v)? as i16)),
        VT_UI2 | VT_BOOL => Value::Int(i64::from(u16_at(sec, v)?)),
        VT_I4 | VT_INT => Value::Int(i64::from(u32_at(sec, v)? as i32)),
        VT_UI4 | VT_UINT => Value::Int(i64::from(u32_at(sec, v)?)),
        VT_I8 | VT_UI8 => Value::Int(u64_at(sec, v)? as i64),
        VT_FILETIME => Value::Time(u64_at(sec, v)?),
        VT_LPSTR => {
            let n = u32_at(sec, v)? as usize;
            let b = sec.get(v + 4..(v + 4).checked_add(n)?)?;
            Value::Str(decode_string(b, codepage))
        }
        VT_LPWSTR => {
            let n = u32_at(sec, v)? as usize;
            let b = sec.get(v + 4..(v + 4).checked_add(n.checked_mul(2)?)?)?;
            Value::Str(utf16(b))
        }
        VT_CF => {
            let n = u32_at(sec, v)? as usize;
            let body = sec.get(v + 4..(v + 4).checked_add(n)?.min(sec.len()))?;
            let tag = u32_at(body, 0)? as i32;
            match tag {
                -1 | -2 => Value::Clip(tag, u32_at(body, 4)?, body.get(8..)?),
                -3 => Value::Clip(tag, 0, body.get(20..)?),
                t if t > 0 => {
                    let skip = 4usize.checked_add(t as usize)?;
                    Value::Clip(tag, 0, body.get(skip..)?)
                }
                _ => Value::Clip(tag, 0, body.get(4..)?),
            }
        }
        _ => return None,
    })
}

fn utf16(b: &[u8]) -> String {
    let units: Vec<u16> = b
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16_lossy(&units)
}

/// Windows-1252 bytes 0x80–0x9F (the rest of the upper half is Latin-1).
const CP1252: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
    '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
];

/// A code-page string up to its terminating NUL: UTF-16 (1200), UTF-8 (65001), else
/// Windows-1252 (the code page of nearly every Western Office file; other single-byte pages
/// read their ASCII right).
fn decode_string(b: &[u8], codepage: u16) -> String {
    match codepage {
        1200 => utf16(b),
        65001 => {
            let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
            String::from_utf8_lossy(&b[..end]).into_owned()
        }
        _ => b
            .iter()
            .take_while(|&&c| c != 0)
            .map(|&c| match c {
                0x80..=0x9F => CP1252[usize::from(c - 0x80)],
                c => char::from(c),
            })
            .collect(),
    }
}

/// A FILETIME as an ISO 8601 UTC date and time; `None` for 0 and dates before 1970 or after
/// 9999 (unset or damaged).
pub(crate) fn filetime(ft: u64) -> Option<String> {
    let secs = (ft / 10_000_000) as i64 - 11_644_473_600;
    if ft == 0 || !(0..253_402_300_800).contains(&secs) {
        return None;
    }
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Days to civil date (proleptic Gregorian), after H. Hinnant's public-domain algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    Some(format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    ))
}

pub(crate) fn probe(data: &[u8]) -> Option<(Family, App)> {
    let cfb = Cfb::open(data).ok()?;
    if cfb.exists("EncryptedPackage") && cfb.exists("EncryptionInfo") {
        return Some((Family::Ooxml, App::Word));
    }
    app_of(&cfb).map(|a| (Family::Legacy, a))
}

fn app_of(cfb: &Cfb) -> Option<App> {
    if cfb.exists("WordDocument") {
        Some(App::Word)
    } else if cfb.exists("Workbook") || cfb.exists("Book") {
        Some(App::Excel)
    } else if cfb.exists("PowerPoint Document") {
        Some(App::PowerPoint)
    } else {
        None
    }
}

pub(crate) fn open(data: &[u8], hint: Option<App>) -> Result<Document, Error> {
    let cfb = Cfb::open(data).map_err(|_| Error::NotOffice)?;
    if cfb.exists("EncryptedPackage") && cfb.exists("EncryptionInfo") {
        return Err(Error::Encrypted(hint.map_or("Office document", App::noun)));
    }
    let app = app_of(&cfb).ok_or(Error::NotOffice)?;
    let read = |name: &str| {
        cfb.entry(name)
            .filter(|e| e.size <= MAX_PART)
            .and_then(|_| cfb.read(name).ok())
    };
    let mut props = Properties::default();
    let mut preview = None;
    if let Some(ps) = read(SUMMARY) {
        for (id, v) in property_set(&ps) {
            let s = || match &v {
                Value::Str(s) => Some(s.trim().to_owned()).filter(|s| !s.is_empty()),
                _ => None,
            };
            let n = || match &v {
                Value::Int(i) => u64::try_from(*i).ok(),
                _ => None,
            };
            let t = || match &v {
                Value::Time(t) => filetime(*t),
                _ => None,
            };
            match id {
                2 => props.title = s(),
                3 => props.subject = s(),
                4 => props.author = s(),
                5 => props.keywords = s(),
                6 => props.description = s(),
                7 => props.template = s(),
                8 => props.last_modified_by = s(),
                9 => props.revision = s(),
                10 => {
                    if let Value::Time(t) = v {
                        props.edit_minutes = Some(t / 600_000_000).filter(|&m| m > 0);
                    }
                }
                11 => props.printed = t(),
                12 => props.created = t(),
                13 => props.modified = t(),
                14 => props.pages = n(),
                15 => props.words = n(),
                16 => props.characters = n(),
                17 => {
                    if let Value::Clip(tag, cf, bytes) = v {
                        preview = clipboard(tag, cf, bytes);
                    }
                }
                18 => props.application = s(),
                _ => {}
            }
        }
    }
    if let Some(ps) = read(DOC_SUMMARY) {
        for (id, v) in property_set(&ps) {
            let s = match &v {
                Value::Str(s) => Some(s.trim().to_owned()).filter(|s| !s.is_empty()),
                _ => None,
            };
            let n = match &v {
                Value::Int(i) => u64::try_from(*i).ok(),
                _ => None,
            };
            match id {
                2 => props.category = s,
                5 => props.lines = n,
                6 => props.paragraphs = n,
                7 => props.slides = n,
                8 => props.notes = n,
                9 => props.hidden_slides = n,
                15 => props.company = s,
                _ => {}
            }
        }
    }
    // Word's file information block says whether the file is a template (`fDot`).
    let template = app == App::Word
        && cfb
            .read("WordDocument")
            .ok()
            .and_then(|w| u16_at(&w, 0x0A))
            .is_some_and(|f| f & 1 != 0);
    let macros = cfb.list().any(|(path, _)| {
        path.split('/')
            .any(|p| p == "VBA" || p == "_VBA_PROJECT_CUR" || p == "Macros")
    });
    Ok(Document {
        family: Family::Legacy,
        app,
        template,
        macros,
        binary: false,
        preview: preview.map(|(kind, bytes)| Preview {
            kind,
            bytes: Bytes::Owned(bytes),
            part: "SummaryInformation".into(),
        }),
        props,
    })
}

/// A thumbnail's clipboard data as an image file.
fn clipboard(tag: i32, cf: u32, data: &[u8]) -> Option<(PreviewKind, Vec<u8>)> {
    match (tag, cf) {
        (-1, CF_METAFILEPICT) => Some((PreviewKind::Wmf, packed_metafile(data)?.to_vec())),
        (-1, CF_DIB) => Some((PreviewKind::Bmp, dib_to_bmp(data)?)),
        (-1, CF_ENHMETAFILE) => Some((PreviewKind::Emf, data.to_vec())),
        // A Macintosh PICT ('PICT', stored either way round): the 512-byte file header added.
        (-2, f) if f == u32::from_be_bytes(*b"PICT") || f == u32::from_le_bytes(*b"PICT") => {
            let mut v = vec![0u8; 512];
            v.extend_from_slice(data);
            Some((PreviewKind::Pict, v))
        }
        _ => PreviewKind::sniff(data).map(|k| (k, data.to_vec())),
    }
}

/// The WMF inside packed `CF_METAFILEPICT` data: after the 8-byte METAFILEPICT header (mapping
/// mode, extents, handle), or the 16-byte 32-bit form, or bare.
fn packed_metafile(data: &[u8]) -> Option<&[u8]> {
    let header =
        |o: usize| matches!(u16_at(data, o), Some(1 | 2)) && u16_at(data, o + 2) == Some(9);
    [8, 16, 0]
        .into_iter()
        .find(|&o| header(o))
        .map(|o| &data[o..])
}

/// A BMP file around a packed DIB: the 14-byte file header, with the pixel offset after the
/// info header, the bit-field masks and the colour table.
pub(crate) fn dib_to_bmp(dib: &[u8]) -> Option<Vec<u8>> {
    let header = u32_at(dib, 0)? as usize;
    if !(12..=124).contains(&header) || dib.len() < header {
        return None;
    }
    let (bits, compression, used) = if header == 12 {
        (u16_at(dib, 10)?, 0, 0)
    } else {
        (u16_at(dib, 14)?, u32_at(dib, 16)?, u32_at(dib, 32)?)
    };
    let entry = if header == 12 { 3 } else { 4 };
    let colours = match (used, bits) {
        (0, b) if b <= 8 => 1usize << b,
        (0, _) => 0,
        (u, _) => (u as usize).min(256),
    };
    // BI_BITFIELDS (3) and BI_ALPHABITFIELDS (6) with a 40-byte header: masks follow it.
    let masks = match (header, compression) {
        (40, 3) => 12,
        (40, 6) => 16,
        _ => 0,
    };
    let offset = 14 + header + masks + colours * entry;
    let size = u32::try_from(14 + dib.len()).ok()?;
    let mut out = Vec::with_capacity(14 + dib.len());
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&u32::try_from(offset).ok()?.to_le_bytes());
    out.extend_from_slice(dib);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filetimes() {
        assert_eq!(filetime(0), None);
        // 2001-09-09T01:46:40Z is Unix 1e9.
        let ft = (1_000_000_000u64 + 11_644_473_600) * 10_000_000;
        assert_eq!(filetime(ft).as_deref(), Some("2001-09-09T01:46:40Z"));
        let leap = (951_782_400u64 + 11_644_473_600) * 10_000_000; // 2000-02-29
        assert_eq!(filetime(leap).as_deref(), Some("2000-02-29T00:00:00Z"));
        assert_eq!(filetime(u64::MAX), None);
    }

    #[test]
    fn code_pages() {
        assert_eq!(decode_string(b"Caf\xe9 \x80\0junk", 1252), "Café €");
        assert_eq!(decode_string("Ünï\0".as_bytes(), 65001), "Ünï");
        assert_eq!(decode_string(&[0x41, 0, 0x42, 0, 0, 0], 1200), "AB");
    }

    #[test]
    fn hostile_property_sets_do_not_panic() {
        let mut ps = vec![0xFE, 0xFF, 0, 0, 0, 0, 0, 0];
        ps.extend_from_slice(&[0; 16]);
        ps.extend_from_slice(&1u32.to_le_bytes());
        ps.extend_from_slice(&[0; 16]);
        ps.extend_from_slice(&48u32.to_le_bytes());
        for tail in [
            vec![],
            vec![0xFF; 8],
            [100u32, 0xFFFF_FFFF, 2, 9, 0xFFFF_FFF0]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect(),
            [64u32, 3, 17, 40, 2, 52, 4, 60, 0x47, 0x7FFF_FFFF]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect(),
        ] {
            let mut p = ps.clone();
            p.extend_from_slice(&tail);
            for cut in 0..p.len() {
                let _ = property_set(&p[..cut]);
            }
        }
        for d in [
            &[][..],
            &[40, 0, 0, 0][..],
            &[12, 0, 0, 0, 1, 0, 1, 0, 1, 0, 8, 0][..],
        ] {
            let _ = dib_to_bmp(d);
            let _ = packed_metafile(d);
            let _ = clipboard(-1, CF_METAFILEPICT, d);
        }
    }
}
