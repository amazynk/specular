//! IPTC-IIM (the "IPTC" block of JPEG APP13, TIFF tag 33723 and PSD image resource 1028):
//! the application record's (2) descriptive datasets as [`TagFacts`], with their XMP
//! equivalents (IPTC Photo Metadata Standard mapping) for the summary.
//!
//! A dataset is a 0x1C tag marker, record and dataset numbers, and a big-endian length (an
//! extended length when its top bit is set). Text is UTF-8 when record 1 says so (coded
//! character set ESC % G), and is read as UTF-8 when valid, else as Latin-1.

use crate::TagFacts;

/// Datasets read at most.
const MAX_DATASETS: usize = 4096;

/// The application record datasets read: (number, name, XMP property).
const DATASETS: &[(u8, &str, Option<&str>)] = &[
    (5, "ObjectName", Some("dc:title")),
    (10, "Urgency", None),
    (15, "Category", None),
    (20, "SupplementalCategories", None),
    (25, "Keywords", Some("dc:subject")),
    (40, "SpecialInstructions", None),
    (55, "DateCreated", None),
    (60, "TimeCreated", None),
    (65, "OriginatingProgram", None),
    (70, "ProgramVersion", None),
    (80, "By-line", Some("dc:creator")),
    (85, "By-lineTitle", None),
    (90, "City", Some("photoshop:City")),
    (92, "Sub-location", None),
    (95, "Province-State", Some("photoshop:State")),
    (100, "Country-PrimaryLocationCode", None),
    (
        101,
        "Country-PrimaryLocationName",
        Some("photoshop:Country"),
    ),
    (103, "OriginalTransmissionReference", None),
    (105, "Headline", Some("photoshop:Headline")),
    (110, "Credit", Some("photoshop:Credit")),
    (115, "Source", Some("photoshop:Source")),
    (116, "CopyrightNotice", Some("dc:rights")),
    (120, "Caption-Abstract", Some("dc:description")),
    (122, "Writer-Editor", None),
];

/// The IIM block `b`'s descriptive tags, or None when it holds none.
pub(crate) fn parse(b: &[u8]) -> Option<TagFacts> {
    let mut utf8 = false;
    let mut i = 0usize;
    let mut out = TagFacts {
        kind: "IPTC".to_owned(),
        ..Default::default()
    };
    let (mut date, mut time) = (None::<String>, None::<String>);
    for _ in 0..MAX_DATASETS {
        // Padding between datasets (some writers align to even lengths).
        while b.get(i) == Some(&0) {
            i += 1;
        }
        let Some(h) = b.get(i..i + 5) else { break };
        if h[0] != 0x1C {
            break;
        }
        let (record, number) = (h[1], h[2]);
        let mut len = usize::from(u16::from_be_bytes([h[3], h[4]]));
        i += 5;
        if len & 0x8000 != 0 {
            // Extended: the low bits give how many bytes the length takes.
            let n = len & 0x7FFF;
            if n == 0 || n > 4 {
                break;
            }
            let Some(l) = b.get(i..i + n) else { break };
            len = l.iter().fold(0usize, |a, &x| (a << 8) | usize::from(x));
            i += n;
        }
        let Some(v) = b.get(i..i.saturating_add(len)) else {
            break;
        };
        i += len;
        if record == 1 && number == 90 {
            utf8 = v.windows(3).any(|w| w == b"\x1b%G");
            continue;
        }
        if record != 2 {
            continue;
        }
        let Some(&(_, name, xmp)) = DATASETS.iter().find(|d| d.0 == number) else {
            continue;
        };
        let text = text(v, utf8);
        if text.is_empty() {
            continue;
        }
        match number {
            55 => date = Some(text.clone()),
            60 => time = Some(text.clone()),
            _ => {}
        }
        if let Some(x) = xmp {
            out.props.push((x.to_owned(), text.clone()));
        }
        out.fields.push((name.to_owned(), text));
    }
    if let Some(d) = date.as_deref().and_then(|d| iso_date(d, time.as_deref())) {
        out.props.push(("photoshop:DateCreated".to_owned(), d));
    }
    (!out.fields.is_empty()).then_some(out)
}

/// The text of a dataset: UTF-8 when declared or valid, else Latin-1; trailing NULs and
/// spaces dropped.
pub(crate) fn text(v: &[u8], utf8: bool) -> String {
    let s = match std::str::from_utf8(v) {
        Ok(s) => s.to_owned(),
        Err(_) if utf8 => String::from_utf8_lossy(v).into_owned(),
        Err(_) => v.iter().map(|&c| char::from(c)).collect(),
    };
    s.trim_end_matches(['\0', ' ']).trim().to_owned()
}

/// "CCYYMMDD" and "HHMMSS±HHMM" as ISO 8601 ("2021-03-24T16:29:00+01:00"); the date alone
/// without a time.
fn iso_date(d: &str, t: Option<&str>) -> Option<String> {
    let d = d.trim();
    if d.len() != 8 || !d.bytes().all(|c| c.is_ascii_digit()) || &d[4..] == "0000" {
        return None;
    }
    let date = format!("{}-{}-{}", &d[..4], &d[4..6], &d[6..8]);
    let Some(t) = t.map(str::trim).filter(|t| t.len() >= 6) else {
        return Some(date);
    };
    if !t[..6].bytes().all(|c| c.is_ascii_digit()) {
        return Some(date);
    }
    let zone = match t.get(6..) {
        Some(z) if z.len() == 5 && (z.starts_with('+') || z.starts_with('-')) => {
            format!("{}:{}", &z[..3], &z[3..])
        }
        _ => String::new(),
    };
    Some(format!(
        "{date}T{}:{}:{}{zone}",
        &t[..2],
        &t[2..4],
        &t[4..6]
    ))
}

/// The IIM block of a Photoshop image resource list ("8BIM" blocks; JPEG APP13 after
/// "Photoshop 3.0\0"): resource 1028.
pub(crate) fn from_resources(b: &[u8]) -> Option<&[u8]> {
    let mut i = 0usize;
    for _ in 0..1024 {
        let h = b.get(i..i + 6)?;
        if &h[..4] != b"8BIM" {
            return None;
        }
        let id = u16::from_be_bytes([h[4], h[5]]);
        // A Pascal name padded to an even length.
        let name_len = usize::from(*b.get(i + 6)?);
        let mut j = i + 6 + 1 + name_len;
        j += j % 2;
        let size = u32::from_be_bytes(b.get(j..j + 4)?.try_into().ok()?) as usize;
        j += 4;
        let data = b.get(j..j.checked_add(size)?)?;
        if id == 0x0404 {
            return Some(data);
        }
        i = j + size + size % 2;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dataset(record: u8, number: u8, v: &[u8]) -> Vec<u8> {
        let mut b = vec![0x1C, record, number];
        b.extend((v.len() as u16).to_be_bytes());
        b.extend(v);
        b
    }

    #[test]
    fn reads_the_application_record() {
        let b = [
            dataset(1, 90, b"\x1b%G"),
            dataset(2, 0, &[0, 4]),
            dataset(2, 5, "Café".as_bytes()),
            dataset(2, 25, b"one"),
            dataset(2, 25, b"two"),
            dataset(2, 55, b"20210324"),
            dataset(2, 60, b"162900+0100"),
            dataset(2, 120, b"A caption\0"),
        ]
        .concat();
        let t = parse(&b).unwrap();
        let p = |k: &str| {
            t.props
                .iter()
                .filter(|(n, _)| n == k)
                .map(|(_, v)| v.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(p("dc:title"), ["Café"]);
        assert_eq!(p("dc:subject"), ["one", "two"]);
        assert_eq!(p("dc:description"), ["A caption"]);
        assert_eq!(p("photoshop:DateCreated"), ["2021-03-24T16:29:00+01:00"]);
        assert_eq!(t.fields.len(), 6);
        // Latin-1 without a declaration.
        let t = parse(&dataset(2, 80, b"Jos\xe9")).unwrap();
        assert_eq!(t.fields[0].1, "José");
        for k in 0..b.len() {
            let _ = parse(&b[..k]);
        }
    }

    #[test]
    fn finds_the_iptc_resource() {
        let iim = dataset(2, 5, b"T");
        let mut r = b"8BIM\x04\x0c\0\0".to_vec();
        r.extend(2u32.to_be_bytes());
        r.extend([1, 2]);
        r.extend(b"8BIM\x04\x04\0\0");
        r.extend((iim.len() as u32).to_be_bytes());
        r.extend(&iim);
        assert_eq!(from_resources(&r), Some(&iim[..]));
        assert!(from_resources(b"8BIM").is_none());
    }
}
