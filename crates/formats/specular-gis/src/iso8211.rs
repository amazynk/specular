//! ISO/IEC 8211 records (the exchange format of S-57 charts): a data descriptive record
//! (leader `L`) defining each field's subfields and their formats, then data records
//! (leader `D`, or `R` reusing the previous directory's layout) of tagged fields. Fields are
//! split into subfields by their format controls: fixed-width text and numbers (`A(n)`,
//! `I(n)`, `R(n)`), unit-terminated text (`A`), little-endian binary (`b11`, `b12`, `b14`,
//! `b21`…`b24`, `b48`) and bit strings (`B(40)`); `*` in the subfield labels marks the
//! labels that repeat to the end of the field.

use std::collections::HashMap;

/// Unit terminator (between variable-length subfields) and field terminator.
pub const UT: u8 = 0x1F;
pub const FT: u8 = 0x1E;

/// A subfield's format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fmt {
    /// Text of a fixed width, or to the unit terminator.
    Text(Option<usize>),
    /// An integer written as text.
    Int(Option<usize>),
    /// A real written as text.
    Real(Option<usize>),
    /// Binary, little-endian: unsigned (`b1n`), signed (`b2n`) or IEEE float (`b4n`).
    Unsigned(usize),
    Signed(usize),
    Float(usize),
    /// A bit string of so many bytes (`B(40)`: 5 bytes).
    Bits(usize),
}

/// One subfield's value.
#[derive(Clone, Debug, PartialEq)]
pub enum Sub<'a> {
    Text(&'a [u8]),
    Int(i64),
    Real(f64),
    Bytes(&'a [u8]),
    /// Absent (an empty text number, or past the end of the field).
    Null,
}

impl Sub<'_> {
    pub fn int(&self) -> Option<i64> {
        match self {
            Sub::Int(i) => Some(*i),
            Sub::Real(f) if f.fract() == 0.0 => Some(*f as i64),
            Sub::Text(t) => std::str::from_utf8(t).ok()?.trim().parse().ok(),
            _ => None,
        }
    }
    pub fn real(&self) -> Option<f64> {
        match self {
            Sub::Int(i) => Some(*i as f64),
            Sub::Real(f) => Some(*f),
            Sub::Text(t) => std::str::from_utf8(t).ok()?.trim().parse().ok(),
            _ => None,
        }
    }
    pub fn bytes(&self) -> &[u8] {
        match self {
            Sub::Text(t) | Sub::Bytes(t) => t,
            _ => &[],
        }
    }
}

/// A field's definition.
#[derive(Clone, Debug)]
pub struct FieldDefn {
    pub name: String,
    pub labels: Vec<String>,
    pub formats: Vec<Fmt>,
    /// Subfields from this one on repeat to the end of the field.
    pub repeat_from: Option<usize>,
}

/// The field definitions of a file, by tag.
#[derive(Clone, Debug, Default)]
pub struct Module {
    pub fields: HashMap<String, FieldDefn>,
}

/// One data record: (tag, field bytes without the terminator), in order.
#[derive(Clone, Debug, Default)]
pub struct Record<'a> {
    pub fields: Vec<(&'a str, &'a [u8])>,
}

impl<'a> Record<'a> {
    pub fn field(&self, tag: &str) -> Option<&'a [u8]> {
        self.fields.iter().find(|(t, _)| *t == tag).map(|(_, d)| *d)
    }
    pub fn all(&self, tag: &'a str) -> impl Iterator<Item = &'a [u8]> + '_ {
        self.fields
            .iter()
            .filter(move |(t, _)| *t == tag)
            .map(|(_, d)| *d)
    }
}

fn ascii_num(b: &[u8]) -> Option<usize> {
    if b.is_empty() || !b.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(b).ok()?.parse().ok()
}

/// A record: its leader identifier, (tag, data) of each field, and its length.
type RawRecord<'a> = (u8, Vec<(&'a str, &'a [u8])>, usize);

/// A leader and directory: (tag, data) of each field and the record's length.
fn record(data: &[u8]) -> Result<RawRecord<'_>, String> {
    if data.len() < 24 {
        return Err("a record is cut short".into());
    }
    // A length of 0 means "unknown": the directory says where the record ends.
    let declared = ascii_num(&data[..5]).ok_or("a record's length is not a number")?;
    if (declared != 0 && declared < 24) || declared > data.len() {
        return Err("a record runs past the end of the file".into());
    }
    let rec = if declared == 0 {
        data
    } else {
        &data[..declared]
    };
    let leader_id = rec[6];
    let base = ascii_num(&rec[12..17]).ok_or("a record's field area address")?;
    let size_len = usize::from(rec[20].wrapping_sub(b'0'));
    let size_pos = usize::from(rec[21].wrapping_sub(b'0'));
    let size_tag = usize::from(rec[23].wrapping_sub(b'0'));
    if !(1..=9).contains(&size_len) || !(1..=9).contains(&size_pos) || !(1..=9).contains(&size_tag)
    {
        return Err("a record's entry map".into());
    }
    let entry = size_tag + size_len + size_pos;
    let dir_end = rec[24..]
        .iter()
        .position(|&b| b == FT)
        .map(|p| p + 24)
        .ok_or("a record's directory has no end")?;
    if base > rec.len() || base <= dir_end {
        return Err("a record's directory".into());
    }
    let mut fields = Vec::new();
    let mut end_of_fields = base;
    for e in rec[24..dir_end].chunks_exact(entry) {
        let tag = std::str::from_utf8(&e[..size_tag]).map_err(|_| "a field tag")?;
        let flen = ascii_num(&e[size_tag..size_tag + size_len]).ok_or("a field length")?;
        let fpos = ascii_num(&e[size_tag + size_len..]).ok_or("a field position")?;
        let start = base.checked_add(fpos).ok_or("a field position")?;
        let end = start.checked_add(flen).ok_or("a field length")?;
        let d = rec.get(start..end).ok_or("a field runs past its record")?;
        end_of_fields = end_of_fields.max(end);
        let d = d.strip_suffix(&[FT]).unwrap_or(d);
        fields.push((tag, d));
    }
    let len = if declared == 0 {
        end_of_fields
    } else {
        declared
    };
    Ok((leader_id, fields, len))
}

/// Expands format controls `(b11,2b24,3(A,I(2)))` into one format per subfield.
fn formats(s: &str) -> Result<Vec<Fmt>, String> {
    let s = s.trim();
    let inner = s
        .strip_prefix('(')
        .and_then(|r| r.strip_suffix(')'))
        .unwrap_or(s);
    let mut out = Vec::new();
    expand(inner, &mut out, 0)?;
    Ok(out)
}

fn expand(s: &str, out: &mut Vec<Fmt>, depth: usize) -> Result<(), String> {
    if depth > 8 || out.len() > 4096 {
        return Err("format controls nest too deeply".into());
    }
    // Split at commas outside parentheses.
    let mut items = Vec::new();
    let (mut level, mut start) = (0i32, 0usize);
    for (i, c) in s.char_indices() {
        match c {
            '(' => level += 1,
            ')' => level -= 1,
            ',' if level == 0 => {
                items.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    items.push(&s[start..]);
    for item in items.into_iter().map(str::trim).filter(|i| !i.is_empty()) {
        let digits = item.bytes().take_while(u8::is_ascii_digit).count();
        let rest = &item[digits..];
        // A repeat count before a format, except the width digits of `bWN`.
        let count = if digits > 0 {
            item[..digits].parse::<usize>().map_err(|_| "a count")?
        } else {
            1
        };
        if count > 4096 {
            return Err("a format repeat count".into());
        }
        if let Some(group) = rest.strip_prefix('(') {
            let group = group
                .strip_suffix(')')
                .ok_or("unbalanced format controls")?;
            for _ in 0..count {
                expand(group, out, depth + 1)?;
            }
            continue;
        }
        let width = |r: &str| -> Result<Option<usize>, String> {
            match r.strip_prefix('(') {
                Some(w) => w
                    .strip_suffix(')')
                    .and_then(|w| w.trim().parse().ok())
                    .map(Some)
                    .ok_or_else(|| format!("format {item}")),
                None if r.is_empty() => Ok(None),
                None => Err(format!("format {item}")),
            }
        };
        let f = match rest.as_bytes().first() {
            Some(b'A') => Fmt::Text(width(&rest[1..])?),
            Some(b'I') => Fmt::Int(width(&rest[1..])?),
            Some(b'R' | b'S') => Fmt::Real(width(&rest[1..])?),
            Some(b'B') => {
                let bits = width(&rest[1..])?.ok_or_else(|| format!("format {item}"))?;
                Fmt::Bits(bits.div_ceil(8))
            }
            Some(b'b') if rest.len() == 3 => {
                let n = usize::from(rest.as_bytes()[2].wrapping_sub(b'0'));
                if !matches!(n, 1 | 2 | 4 | 8) {
                    return Err(format!("format {item}"));
                }
                match rest.as_bytes()[1] {
                    b'1' => Fmt::Unsigned(n),
                    b'2' => Fmt::Signed(n),
                    b'4' if n >= 4 => Fmt::Float(n),
                    _ => return Err(format!("format {item}")),
                }
            }
            _ => return Err(format!("format {item}")),
        };
        out.extend(std::iter::repeat_n(f, count));
    }
    Ok(())
}

/// Reads the descriptive record and the data records; a damaged record ends the reading
/// (the reason comes back with the records before it).
pub fn parse(data: &[u8]) -> Result<(Module, Vec<Record<'_>>, Option<String>), String> {
    let (id, ddr, len) = record(data)?;
    if id != b'L' {
        return Err("not an ISO 8211 file (no descriptive record)".into());
    }
    let fcl = ascii_num(&data[10..12]).unwrap_or(9).min(32);
    let mut module = Module::default();
    for (tag, d) in ddr {
        if d.len() < fcl {
            continue;
        }
        let mut parts = d[fcl..].split(|&b| b == UT);
        let name = String::from_utf8_lossy(parts.next().unwrap_or(&[])).into_owned();
        let desc = String::from_utf8_lossy(parts.next().unwrap_or(&[])).into_owned();
        let fmts = String::from_utf8_lossy(parts.next().unwrap_or(&[])).into_owned();
        if fmts.trim().is_empty() {
            continue;
        }
        let Ok(formats) = formats(&fmts) else {
            continue;
        };
        let repeat_from = desc.find('*').map(|i| desc[..i].matches('!').count());
        let labels: Vec<String> = desc
            .split('!')
            .map(|l| l.trim_start_matches('*').to_owned())
            .filter(|l| !l.is_empty())
            .collect();
        module.fields.insert(
            tag.to_owned(),
            FieldDefn {
                name,
                labels,
                formats,
                repeat_from,
            },
        );
    }
    let mut records = Vec::new();
    let mut p = len;
    let mut damage = None;
    while p < data.len() {
        if data[p..]
            .iter()
            .all(|&b| b == 0 || b == b' ' || b == b'\n' || b == b'\r')
        {
            break;
        }
        match record(&data[p..]) {
            Ok((_, fields, n)) => {
                records.push(Record { fields });
                p += n;
            }
            Err(e) => {
                damage = Some(e);
                break;
            }
        }
        if records.len() > 50_000_000 {
            damage = Some("too many records".into());
            break;
        }
    }
    Ok((module, records, damage))
}

impl FieldDefn {
    /// The field's subfields: one group, or (with repeating labels) the fixed ones then one
    /// group per repetition. Each group is (label, value) in order.
    pub fn groups<'a>(&'a self, d: &'a [u8], ut: u8) -> Vec<Vec<(&'a str, Sub<'a>)>> {
        let mut out = Vec::new();
        let mut p = 0usize;
        let n = self.labels.len().min(self.formats.len());
        let read = |i: usize, p: &mut usize| -> Sub<'a> {
            let fmt = self.formats[i];
            let take = |p: &mut usize, w: usize| -> Option<&'a [u8]> {
                let s = d.get(*p..*p + w)?;
                *p += w;
                Some(s)
            };
            let var = |p: &mut usize| -> &'a [u8] {
                let rest = &d[(*p).min(d.len())..];
                let end = rest.iter().position(|&b| b == ut).unwrap_or(rest.len());
                *p += end + 1;
                &rest[..end]
            };
            let num_text = |t: &'a [u8], int: bool| -> Sub<'a> {
                let s = std::str::from_utf8(t).unwrap_or("").trim();
                if s.is_empty() {
                    Sub::Null
                } else if int {
                    s.parse().map_or(Sub::Null, Sub::Int)
                } else {
                    s.parse().map_or(Sub::Null, Sub::Real)
                }
            };
            match fmt {
                Fmt::Text(Some(w)) => take(p, w).map_or(Sub::Null, Sub::Text),
                Fmt::Text(None) => Sub::Text(var(p)),
                Fmt::Int(Some(w)) => take(p, w).map_or(Sub::Null, |t| num_text(t, true)),
                Fmt::Int(None) => num_text(var(p), true),
                Fmt::Real(Some(w)) => take(p, w).map_or(Sub::Null, |t| num_text(t, false)),
                Fmt::Real(None) => num_text(var(p), false),
                Fmt::Bits(w) => take(p, w).map_or(Sub::Null, Sub::Bytes),
                Fmt::Unsigned(w) | Fmt::Signed(w) | Fmt::Float(w) => {
                    let Some(b) = take(p, w) else {
                        return Sub::Null;
                    };
                    let mut a = [0u8; 8];
                    a[..w].copy_from_slice(b);
                    match fmt {
                        Fmt::Unsigned(_) => Sub::Int(u64::from_le_bytes(a) as i64),
                        Fmt::Signed(_) => {
                            let shift = 64 - 8 * w as u32;
                            Sub::Int((i64::from_le_bytes(a) << shift) >> shift)
                        }
                        _ if w == 4 => {
                            Sub::Real(f64::from(f32::from_le_bytes([a[0], a[1], a[2], a[3]])))
                        }
                        _ => Sub::Real(f64::from_le_bytes(a)),
                    }
                }
            }
        };
        let fixed = self.repeat_from.unwrap_or(n).min(n);
        if fixed > 0 || self.repeat_from.is_none() {
            let g: Vec<_> = (0..fixed)
                .map(|i| (self.labels[i].as_str(), read(i, &mut p)))
                .collect();
            out.push(g);
        }
        if fixed < n {
            while p < d.len() && out.len() < 10_000_000 {
                let before = p;
                let g: Vec<_> = (fixed..n)
                    .map(|i| (self.labels[i].as_str(), read(i, &mut p)))
                    .collect();
                out.push(g);
                if p == before {
                    break;
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_controls() {
        assert_eq!(
            formats("(b11,b14,2b11,3A,2A(8),R(4),B(40))").unwrap(),
            [
                Fmt::Unsigned(1),
                Fmt::Unsigned(4),
                Fmt::Unsigned(1),
                Fmt::Unsigned(1),
                Fmt::Text(None),
                Fmt::Text(None),
                Fmt::Text(None),
                Fmt::Text(Some(8)),
                Fmt::Text(Some(8)),
                Fmt::Real(Some(4)),
                Fmt::Bits(5),
            ]
        );
        assert_eq!(
            formats("(2(b24,I(2)))").unwrap(),
            [
                Fmt::Signed(4),
                Fmt::Int(Some(2)),
                Fmt::Signed(4),
                Fmt::Int(Some(2))
            ]
        );
        assert!(formats("(b33)").is_err());
        assert!(formats("(99999A)").is_err());
    }

    #[test]
    fn repeating_groups() {
        let d = FieldDefn {
            name: "2-D coordinate".into(),
            labels: vec!["YCOO".into(), "XCOO".into()],
            formats: vec![Fmt::Signed(4), Fmt::Signed(4)],
            repeat_from: Some(0),
        };
        let mut b = Vec::new();
        for v in [10i32, -20, 30, 40] {
            b.extend(v.to_le_bytes());
        }
        let g = d.groups(&b, UT);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0][1], ("XCOO", Sub::Int(-20)));
        assert_eq!(g[1][0], ("YCOO", Sub::Int(30)));
        let a = FieldDefn {
            name: "attributes".into(),
            labels: vec!["ATTL".into(), "ATVL".into()],
            formats: vec![Fmt::Unsigned(2), Fmt::Text(None)],
            repeat_from: Some(0),
        };
        let g = a.groups(b"\x74\x00Harbour\x1f\x57\x00\x1f", UT);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0][0].1, Sub::Int(116));
        assert_eq!(g[0][1].1.bytes(), b"Harbour");
        assert_eq!(g[1][1].1.bytes(), b"");
    }
}
