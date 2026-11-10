//! The editable fields in IPTC-IIM (the application record, 2), read and written.
//!
//! Writing keeps every dataset it does not manage, as it was (text in Latin-1 is made UTF-8,
//! as record 1 then declares), and puts the datasets in record and number order. Values
//! longer than IIM allows are cut at a character boundary: XMP keeps them whole, and readers
//! that follow the Metadata Working Group prefer it.

use super::{Field, Record};
use crate::keywords as kw;

/// The datasets of the fields: (field, dataset number, longest value in bytes).
const DATASETS: &[(Field, u8, usize)] = &[
    (Field::Title, 5, 64),
    (Field::Keywords, 25, 64),
    (Field::Instructions, 40, 256),
    (Field::Creator, 80, 32),
    (Field::CreatorJobTitle, 85, 32),
    (Field::ShownCity, 90, 32),
    (Field::ShownSublocation, 92, 32),
    (Field::ShownState, 95, 32),
    (Field::ShownCountryCode, 100, 3),
    (Field::ShownCountry, 101, 64),
    (Field::JobId, 103, 32),
    (Field::Headline, 105, 256),
    (Field::Credit, 110, 32),
    (Field::Source, 115, 32),
    (Field::Copyright, 116, 128),
    (Field::Description, 120, 2000),
];

/// Datasets read at most.
const MAX_DATASETS: usize = 4096;

/// The longest value IIM takes for `f` (bytes), None when IIM has no place for it.
pub fn iim_limit(f: Field) -> Option<usize> {
    DATASETS.iter().find(|d| d.0 == f).map(|d| d.2)
}

/// One dataset: record, number, value.
type Dataset = (u8, u8, Vec<u8>);

/// The datasets of an IIM block (as far as they read).
fn datasets(b: &[u8]) -> Vec<Dataset> {
    let mut out = Vec::new();
    let mut i = 0usize;
    for _ in 0..MAX_DATASETS {
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
        out.push((record, number, v.to_vec()));
    }
    out
}

fn declares_utf8(ds: &[Dataset]) -> bool {
    ds.iter()
        .any(|(r, n, v)| *r == 1 && *n == 90 && v.windows(3).any(|w| w == b"\x1b%G"))
}

/// The editable fields an IIM block has values for.
pub fn read_iim(b: &[u8]) -> Record {
    let ds = datasets(b);
    let utf8 = declares_utf8(&ds);
    let mut lists: std::collections::BTreeMap<Field, Vec<String>> = Default::default();
    for (r, n, v) in &ds {
        if *r != 2 {
            continue;
        }
        let Some(&(f, _, _)) = DATASETS.iter().find(|d| d.1 == *n) else {
            continue;
        };
        let t = crate::iptc::text(v, utf8);
        if t.is_empty() {
            continue;
        }
        let t = if f == Field::Keywords {
            t.replace(kw::SEP, "/")
        } else {
            t
        };
        lists.entry(f).or_default().push(t);
    }
    let mut rec = Record::new();
    for (f, v) in lists {
        rec.set(f, v);
    }
    rec
}

/// Whether record 2 dataset `n` holds text (not the version, a rasterised caption or a
/// preview).
fn is_text(n: u8) -> bool {
    !matches!(n, 0 | 125 | 200..=202)
}

/// The longest prefix of `s` within `max` bytes, cut at a character boundary.
fn cut(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut e = max;
    while !s.is_char_boundary(e) {
        e -= 1;
    }
    &s[..e]
}

/// `existing` (None: a new block) with the datasets of `fields` set from `rec`. Keywords go
/// in as `subject` gives them (names, not paths). None when the block would hold nothing.
pub fn write_iim(
    existing: Option<&[u8]>,
    rec: &Record,
    fields: &[Field],
    subject: &[String],
) -> Option<Vec<u8>> {
    let old = existing.map(datasets).unwrap_or_default();
    let utf8 = declares_utf8(&old);
    let managed: Vec<u8> = DATASETS
        .iter()
        .filter(|d| fields.contains(&d.0))
        .map(|d| d.1)
        .collect();
    let mut out: Vec<Dataset> = Vec::new();
    for (r, n, v) in old {
        if r == 1 && n == 90 {
            continue;
        }
        if r == 2 && managed.contains(&n) {
            continue;
        }
        // Text in another character set becomes UTF-8 (record 1 says so below).
        let v = if r == 2 && is_text(n) && !utf8 && std::str::from_utf8(&v).is_err() {
            crate::iptc::text(&v, false).into_bytes()
        } else {
            v
        };
        out.push((r, n, v));
    }
    for &(f, n, max) in DATASETS {
        if !fields.contains(&f) {
            continue;
        }
        let values: Vec<String> = if f == Field::Keywords {
            subject.to_vec()
        } else {
            rec.values(f).to_vec()
        };
        for v in values {
            let v = cut(v.trim(), max);
            if !v.is_empty() {
                out.push((2, n, v.as_bytes().to_vec()));
            }
        }
    }
    if !out.iter().any(|(r, n, _)| *r == 2 && *n != 0) {
        return None;
    }
    out.push((1, 90, b"\x1b%G".to_vec()));
    if !out.iter().any(|(r, n, _)| *r == 2 && *n == 0) {
        out.push((2, 0, vec![0, 4]));
    }
    // Record order, then dataset number; repeated datasets keep their order.
    out.sort_by_key(|(r, n, _)| (*r, *n));
    let mut b = Vec::new();
    for (r, n, v) in out {
        b.extend([0x1C, r, n]);
        if v.len() < 0x8000 {
            b.extend((v.len() as u16).to_be_bytes());
        } else {
            b.extend([0x80, 0x04]);
            b.extend((v.len() as u32).to_be_bytes());
        }
        b.extend(v);
    }
    Some(b)
}
