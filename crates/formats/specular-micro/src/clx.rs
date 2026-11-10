//! Nikon's CLx variant metadata: the binary "CLX Lite" records of ND2 version 3 files and
//! the XML form of version 2 and legacy files, read into one tree.
//!
//! A CLX Lite record is a type byte, a name length (UTF-16 code units including the NUL) and
//! the UTF-16 name, then the value: fixed-size numbers, a NUL-terminated UTF-16 string, a
//! byte array (u64 length), or a level (u32 item count, u64 length measured from the
//! record's start, the items, then an offset per item). Type 76 marks a zlib-compressed
//! record (ten bytes of header after the type, then the stream). Lists are items with empty
//! names.
//!
//! The XML form has an element per item, with `runtype` and `value` attributes on scalars.
//!
//! Names keep their type prefix (`uiWidth`, `dCalibration`); lookups ignore it, so
//! `"Width"` finds `uiWidth` and the legacy XML's `Width` alike.

use crate::{Error, Result};

/// Deepest nesting accepted.
const MAX_DEPTH: usize = 64;
/// Most items in one tree.
const MAX_ITEMS: usize = 1 << 20;
/// Largest inflated compressed record.
const MAX_INFLATE: usize = 256 << 20;
/// Most compressed records nested in one another (each holds up to `MAX_INFLATE` bytes).
const MAX_COMPRESSED: usize = 4;

/// One value of the tree.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    Text(String),
    Bytes(Vec<u8>),
    /// Named children in file order (list items have empty names).
    Level(Vec<(String, Value)>),
}

/// A name without its lowercase type prefix (`uiWidth` → `Width`, `sPlaneNew` → `PlaneNew`).
pub fn bare(name: &str) -> &str {
    let s = name.trim_start_matches(|c: char| c.is_ascii_lowercase() || c == '_');
    if s.is_empty() { name } else { s }
}

impl Value {
    /// A child by name, ignoring type prefixes on both sides.
    pub fn get(&self, name: &str) -> Option<&Value> {
        let Value::Level(items) = self else {
            return None;
        };
        let want = bare(name);
        items.iter().find(|(k, _)| bare(k) == want).map(|(_, v)| v)
    }

    /// A descendant by a path of names.
    pub fn path(&self, names: &[&str]) -> Option<&Value> {
        names.iter().try_fold(self, |v, n| v.get(n))
    }

    /// The children of a level (empty for a scalar).
    pub fn items(&self) -> &[(String, Value)] {
        match self {
            Value::Level(items) => items,
            _ => &[],
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Bool(b) => Some(f64::from(u8::from(*b))),
            Value::Int(i) => Some(*i as f64),
            Value::UInt(u) => Some(*u as f64),
            Value::Float(f) => Some(*f),
            Value::Text(t) => t.trim().parse().ok(),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::Bool(b) => Some(u64::from(*b)),
            Value::Int(i) => u64::try_from(*i).ok(),
            Value::UInt(u) => Some(*u),
            Value::Float(f) if *f >= 0.0 && f.fract() == 0.0 && *f < 1.8e19 => Some(*f as u64),
            Value::Text(t) => t.trim().parse().ok(),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            Value::Text(t) => Some(matches!(t.trim(), "true" | "True" | "TRUE" | "1")),
            v => v.as_f64().map(|f| f != 0.0),
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Text(t) => Some(t),
            _ => None,
        }
    }

    pub fn f64(&self, name: &str) -> Option<f64> {
        self.get(name)?.as_f64().filter(|v| v.is_finite())
    }

    pub fn u64(&self, name: &str) -> Option<u64> {
        self.get(name)?.as_u64()
    }

    pub fn text(&self, name: &str) -> Option<&str> {
        self.get(name)?.as_str()
    }

    /// The single child of a level that only wraps another (`SLxImageAttributes`,
    /// `<variant><no_name>`), else the value itself.
    pub fn unwrap_single(&self) -> &Value {
        let mut v = self;
        while let Value::Level(items) = v {
            match items.as_slice() {
                [(_, inner @ Value::Level(_))] => v = inner,
                _ => break,
            }
        }
        v
    }

    /// Scalar values as `(path, text)` pairs, depth first, for an info panel; at most `max`.
    pub fn flatten(&self, prefix: &str, max: usize, out: &mut Vec<(String, String)>) {
        for (k, v) in self.items() {
            if out.len() >= max {
                return;
            }
            let name = if k.is_empty() { "#" } else { bare(k) };
            let key = if prefix.is_empty() {
                name.to_owned()
            } else {
                format!("{prefix}.{name}")
            };
            match v {
                Value::Level(_) => v.flatten(&key, max, out),
                Value::Bytes(_) => {}
                Value::Text(t) if t.trim().is_empty() => {}
                Value::Text(t) => out.push((key, t.trim().to_owned())),
                Value::Bool(b) => out.push((key, b.to_string())),
                Value::Int(i) => out.push((key, i.to_string())),
                Value::UInt(u) => out.push((key, u.to_string())),
                Value::Float(f) => out.push((key, f.to_string())),
            }
        }
    }
}

struct Lite<'a> {
    b: &'a [u8],
    items: usize,
    compressed: usize,
}

/// Parse a CLX Lite chunk: its records in order, as one level.
pub fn parse_lite(data: &[u8]) -> Result<Value> {
    parse_nested(data, 0, 0)
}

/// Records at nesting `depth`, inside `compressed` compressed records.
fn parse_nested(data: &[u8], depth: usize, compressed: usize) -> Result<Value> {
    let mut p = Lite {
        b: data,
        items: 0,
        compressed,
    };
    let mut pos = 0;
    let mut out = Vec::new();
    while pos + 2 <= data.len() && data[pos] != 0 {
        let (name, v, next) = p.record(pos, data.len(), depth)?;
        out.push((name, v));
        if next <= pos {
            break;
        }
        pos = next;
    }
    Ok(Value::Level(out))
}

fn bad(m: &str) -> Error {
    Error::Malformed(format!("CLX Lite: {m}"))
}

impl Lite<'_> {
    /// One record at `pos` (ending by `end`): its name, value and the position after it.
    fn record(&mut self, pos: usize, end: usize, depth: usize) -> Result<(String, Value, usize)> {
        if depth > MAX_DEPTH {
            return Err(bad("nested too deeply"));
        }
        self.items += 1;
        if self.items > MAX_ITEMS {
            return Err(bad("too many items"));
        }
        let b = &self.b[..end];
        let (&ty, &nlen) = (
            b.get(pos).ok_or(Error::Truncated("CLX record"))?,
            b.get(pos + 1).ok_or(Error::Truncated("CLX record"))?,
        );
        if ty == 76 {
            // Compressed: skip the 10-byte header, inflate the rest of the record's span.
            if self.compressed >= MAX_COMPRESSED {
                return Err(bad("compressed records nested too deeply"));
            }
            let start = pos + 2 + 10;
            let z = b.get(start..).ok_or(Error::Truncated("CLX compressed"))?;
            let inflated =
                specular_png::inflate_zlib(z, MAX_INFLATE).map_err(|_| bad("compressed record"))?;
            let inner = parse_nested(&inflated, depth + 1, self.compressed + 1)?;
            let v = match inner {
                Value::Level(mut items) if items.len() == 1 => items.remove(0).1,
                v => v,
            };
            return Ok((String::new(), v, end));
        }
        let name_end = pos + 2 + 2 * nlen as usize;
        let name = crate::utf16le(
            b.get(pos + 2..name_end)
                .ok_or(Error::Truncated("CLX name"))?,
        );
        let at = name_end;
        let num = |n: usize| b.get(at..at + n).ok_or(Error::Truncated("CLX value"));
        let (v, next) = match ty {
            1 => (Value::Bool(num(1)?[0] != 0), at + 1),
            2 => (
                Value::Int(i64::from(i32::from_le_bytes(
                    num(4)?.try_into().unwrap_or([0; 4]),
                ))),
                at + 4,
            ),
            3 => (
                Value::UInt(u64::from(u32::from_le_bytes(
                    num(4)?.try_into().unwrap_or([0; 4]),
                ))),
                at + 4,
            ),
            4 => (
                Value::Int(i64::from_le_bytes(num(8)?.try_into().unwrap_or([0; 8]))),
                at + 8,
            ),
            5 | 7 => (
                Value::UInt(u64::from_le_bytes(num(8)?.try_into().unwrap_or([0; 8]))),
                at + 8,
            ),
            6 => (
                Value::Float(f64::from_le_bytes(num(8)?.try_into().unwrap_or([0; 8]))),
                at + 8,
            ),
            8 => {
                // NUL-terminated UTF-16.
                let mut e = at;
                while e + 1 < b.len() && (b[e] != 0 || b[e + 1] != 0) {
                    e += 2;
                }
                (
                    Value::Text(crate::utf16le(&b[at..e.min(b.len())])),
                    (e + 2).min(b.len()),
                )
            }
            9 => {
                let n = crate::le_u64(b, at).ok_or(Error::Truncated("CLX bytes"))?;
                let n = usize::try_from(n).map_err(|_| bad("byte array length"))?;
                let s = at + 8;
                let raw = b.get(s..s.checked_add(n).ok_or_else(|| bad("byte array"))?);
                let raw = raw.ok_or(Error::Truncated("CLX bytes"))?;
                (Value::Bytes(raw.to_vec()), s + n)
            }
            11 => {
                let count = crate::le_u32(b, at).ok_or(Error::Truncated("CLX level"))? as usize;
                let len = crate::le_u64(b, at + 4).ok_or(Error::Truncated("CLX level"))?;
                let len = usize::try_from(len).map_err(|_| bad("level length"))?;
                let lend = pos
                    .checked_add(len)
                    .filter(|&e| e <= b.len() && e >= at + 12);
                let lend = lend.ok_or(Error::Truncated("CLX level"))?;
                let mut items = Vec::with_capacity(count.min(4096));
                let mut p = at + 12;
                for _ in 0..count {
                    if p + 2 > lend {
                        break;
                    }
                    let (n, v, q) = self.record(p, lend, depth + 1)?;
                    items.push((n, v));
                    if q <= p {
                        break;
                    }
                    p = q;
                }
                // The offset table follows the level's span.
                let next = lend
                    .checked_add(count.checked_mul(8).ok_or_else(|| bad("level count"))?)
                    .ok_or_else(|| bad("level count"))?;
                (Value::Level(items), next.min(b.len()))
            }
            _ => return Err(bad(&format!("record type {ty}"))),
        };
        Ok((name, v, next))
    }
}

/// Parse the XML form (`<variant>` documents of version 2 chunks and legacy boxes).
pub fn parse_xml(data: &[u8]) -> Result<Value> {
    let text = if data.starts_with(&[0xff, 0xfe]) {
        crate::utf16le(&data[2..])
    } else {
        String::from_utf8_lossy(data).into_owned()
    };
    let text = text.trim_end_matches('\0');
    let root = specular_wsi::xml::parse(text).map_err(|e| Error::Malformed(e.to_string()))?;
    let (name, v) = xml_value(&root);
    Ok(Value::Level(vec![(name, v)]))
}

fn xml_value(e: &specular_wsi::xml::Element) -> (String, Value) {
    let name = e.name.clone();
    if let (Some(rt), Some(val)) = (e.attr("runtype"), e.attr("value")) {
        let v = match rt {
            "bool" => Value::Bool(matches!(val.trim(), "true" | "True" | "1")),
            "lx_int32" | "lx_int64" => val
                .trim()
                .parse()
                .map(Value::Int)
                .unwrap_or_else(|_| Value::Text(val.to_owned())),
            "lx_uint32" | "lx_uint64" => val
                .trim()
                .parse()
                .map(Value::UInt)
                .unwrap_or_else(|_| Value::Text(val.to_owned())),
            "double" | "float" => Value::Float(val.trim().parse().unwrap_or(f64::NAN)),
            _ => Value::Text(val.to_owned()),
        };
        return (name, v);
    }
    if e.children.is_empty() && !e.text.is_empty() {
        return (name, Value::Text(e.text.clone()));
    }
    // Older files' text info: `<TextInfoItem Text="…" Index="5"/>`.
    if e.children.is_empty()
        && let Some(t) = e.attr("Text")
    {
        return (name, Value::Text(t.to_owned()));
    }
    let items = e
        .children
        .iter()
        .map(|c| {
            let (n, v) = xml_value(c);
            // List items: `no_name` elements.
            (if n == "no_name" { String::new() } else { n }, v)
        })
        .collect();
    (name, Value::Level(items))
}

/// Parse a metadata chunk in whichever form it has.
pub fn parse_any(data: &[u8]) -> Result<Value> {
    let start = data
        .iter()
        .position(|&c| !c.is_ascii_whitespace())
        .unwrap_or(0);
    if data[start..].starts_with(b"<") || data.starts_with(&[0xff, 0xfe, b'<', 0]) {
        parse_xml(data)
    } else {
        parse_lite(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(n: &str) -> Vec<u8> {
        let mut v = vec![(n.encode_utf16().count() + 1) as u8];
        for u in n.encode_utf16() {
            v.extend_from_slice(&u.to_le_bytes());
        }
        v.extend_from_slice(&[0, 0]);
        v
    }

    fn rec(ty: u8, n: &str, val: &[u8]) -> Vec<u8> {
        let mut v = vec![ty];
        v.extend(name(n));
        v.extend_from_slice(val);
        v
    }

    fn level(n: &str, items: &[Vec<u8>]) -> Vec<u8> {
        let mut head = vec![11];
        head.extend(name(n));
        let body: Vec<u8> = items.concat();
        let len = head.len() + 12 + body.len();
        head.extend_from_slice(&(items.len() as u32).to_le_bytes());
        head.extend_from_slice(&(len as u64).to_le_bytes());
        head.extend(body);
        head.extend(vec![0u8; items.len() * 8]);
        head
    }

    #[test]
    fn lite_levels_and_scalars() {
        let mut s = Vec::new();
        for u in "DAPI".encode_utf16() {
            s.extend_from_slice(&u.to_le_bytes());
        }
        s.extend_from_slice(&[0, 0]);
        let doc = level(
            "SLxImageAttributes",
            &[
                rec(3, "uiWidth", &64u32.to_le_bytes()),
                rec(6, "dCalibration", &0.25f64.to_le_bytes()),
                rec(8, "sDescription", &s),
                rec(1, "bFlag", &[1]),
                level("", &[rec(2, "", &(-3i32).to_le_bytes())]),
            ],
        );
        let v = parse_lite(&doc).unwrap();
        let a = v.get("SLxImageAttributes").unwrap();
        assert_eq!(a.u64("Width"), Some(64));
        assert_eq!(a.f64("dCalibration"), Some(0.25));
        assert_eq!(a.text("Description"), Some("DAPI"));
        assert_eq!(a.get("Flag").and_then(Value::as_bool), Some(true));
        assert_eq!(v.unwrap_single().u64("Width"), Some(64));
    }

    #[test]
    fn xml_form() {
        let x = br#"<?xml version="1.0"?><variant version="1.0"><no_name runtype="CLxListVariant"><uiWidth runtype="lx_uint32" value="32"/><dCalibration runtype="double" value="0.5"/><sName runtype="CLxStringW" value="A"/></no_name></variant>"#;
        let v = parse_any(x).unwrap();
        let a = v.unwrap_single();
        assert_eq!(a.u64("Width"), Some(32));
        assert_eq!(a.f64("Calibration"), Some(0.5));
        assert_eq!(a.text("Name"), Some("A"));
    }

    #[test]
    fn garbage_does_not_panic() {
        for n in 0..64u8 {
            let data: Vec<u8> = (0..n).map(|i| i.wrapping_mul(37).wrapping_add(n)).collect();
            let _ = parse_lite(&data);
            let mut d = vec![11, 1, 0, 0];
            d.extend_from_slice(&data);
            let _ = parse_lite(&d);
        }
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    /// `data` as a zlib stream of stored blocks.
    fn stored(data: &[u8]) -> Vec<u8> {
        let mut z = vec![0x78, 0x01];
        let chunks: Vec<&[u8]> = data.chunks(65535).collect();
        for (i, c) in chunks.iter().enumerate() {
            z.push(u8::from(i + 1 == chunks.len()));
            let n = c.len() as u16;
            z.extend_from_slice(&n.to_le_bytes());
            z.extend_from_slice(&(!n).to_le_bytes());
            z.extend_from_slice(c);
        }
        if data.is_empty() {
            z.extend_from_slice(&[1, 0, 0, 0xff, 0xff]);
        }
        let (mut a, mut b) = (1u32, 0u32);
        for &x in data {
            a = (a + u32::from(x)) % 65521;
            b = (b + a) % 65521;
        }
        z.extend_from_slice(&((b << 16) | a).to_be_bytes());
        z
    }

    /// Compressed records nested 2000 deep, each holding the next.
    #[test]
    fn compressed_records_nested_deeply() {
        let mut r = vec![3, 1, 0, 0, 7, 0, 0, 0];
        for _ in 0..2000 {
            let mut outer = vec![76, 0];
            outer.extend_from_slice(&[0; 10]);
            outer.extend(stored(&r));
            r = outer;
        }
        assert!(parse_lite(&r).is_err());
        // A few levels still read.
        let mut r = vec![3, 2, b'a', 0, 0, 0, 7, 0, 0, 0];
        for _ in 0..2 {
            let mut outer = vec![76, 0];
            outer.extend_from_slice(&[0; 10]);
            outer.extend(stored(&r));
            r = outer;
        }
        let v = parse_lite(&r).unwrap();
        assert_eq!(v, Value::Level(vec![(String::new(), Value::UInt(7))]));
    }
}
