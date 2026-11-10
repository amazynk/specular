//! DXF group tokenizer: ASCII (`\r\n` or `\n`, codes with leading spaces) and binary (the
//! "AutoCAD Binary DXF" sentinel, 1-byte codes in R12 and 2-byte codes later, values typed by
//! code range). Strings stay bytes and are decoded where they are used.

use crate::CadError;

pub(crate) const BINARY_SENTINEL: &[u8] = b"AutoCAD Binary DXF\r\n\x1a\0";

#[derive(Clone, Copy, Debug)]
pub(crate) enum Val<'a> {
    /// ASCII values, and binary strings: the bytes as stored (no line ending).
    Text(&'a [u8]),
    Real(f64),
    Int(i64),
    /// A binary chunk of a binary file, skipped (ASCII files keep chunks as hex text).
    Bin,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Group<'a> {
    pub code: i32,
    pub val: Val<'a>,
}

pub(crate) fn trim(b: &[u8]) -> &[u8] {
    let s = b
        .iter()
        .position(|c| !c.is_ascii_whitespace())
        .unwrap_or(b.len());
    let e = b
        .iter()
        .rposition(|c| !c.is_ascii_whitespace())
        .map_or(s, |e| e + 1);
    &b[s..e.max(s)]
}

pub(crate) fn parse_f64(b: &[u8]) -> Option<f64> {
    let t = trim(b);
    let s = std::str::from_utf8(t).ok()?;
    let v: f64 = s.parse().ok()?;
    v.is_finite().then_some(v)
}

pub(crate) fn parse_i64(b: &[u8]) -> Option<i64> {
    let t = trim(b);
    let s = std::str::from_utf8(t).ok()?;
    s.parse::<i64>().ok().or_else(|| {
        let f: f64 = s.parse().ok()?;
        (f.is_finite() && f.abs() < 9e18).then_some(f as i64)
    })
}

impl<'a> Group<'a> {
    /// The value as a number (0 when it is not one, or not finite).
    pub fn f64(&self) -> f64 {
        match self.val {
            Val::Real(v) => v,
            Val::Int(v) => v as f64,
            Val::Text(b) => parse_f64(b).unwrap_or(0.0),
            Val::Bin => 0.0,
        }
    }

    pub fn i64(&self) -> i64 {
        match self.val {
            Val::Int(v) => v,
            Val::Real(v) if v.is_finite() && v.abs() < 9e18 => v as i64,
            Val::Text(b) => parse_i64(b).unwrap_or(0),
            _ => 0,
        }
    }

    /// The raw string bytes (empty for numbers).
    pub fn bytes(&self) -> &'a [u8] {
        match self.val {
            Val::Text(b) => b,
            _ => b"",
        }
    }

    /// The string with surrounding whitespace removed.
    pub fn tbytes(&self) -> &'a [u8] {
        trim(self.bytes())
    }

    /// Whether the (trimmed) string equals `s`, ASCII case-insensitively.
    pub fn is(&self, s: &str) -> bool {
        self.tbytes().eq_ignore_ascii_case(s.as_bytes())
    }

    /// Handles as numbers (hex text, or a binary file's string).
    pub fn handle(&self) -> u64 {
        let b = self.tbytes();
        std::str::from_utf8(b)
            .ok()
            .and_then(|s| u64::from_str_radix(s, 16).ok())
            .unwrap_or(0)
    }
}

/// Tokenizes a whole file. Garbage after valid groups ends the stream with a warning;
/// garbage before any group is [`CadError::Malformed`].
pub(crate) fn tokenize<'a>(
    bytes: &'a [u8],
    warnings: &mut Vec<String>,
) -> Result<Vec<Group<'a>>, CadError> {
    if bytes.starts_with(BINARY_SENTINEL) {
        binary(&bytes[BINARY_SENTINEL.len()..], warnings)
    } else {
        ascii(bytes, warnings)
    }
}

fn ascii<'a>(bytes: &'a [u8], warnings: &mut Vec<String>) -> Result<Vec<Group<'a>>, CadError> {
    let mut b = bytes;
    if b.starts_with(b"\xEF\xBB\xBF") {
        b = &b[3..];
    }
    let mut out = Vec::with_capacity(b.len() / 12);
    let mut lines = Lines { b, pos: 0 };
    let mut line_no = 0usize;
    while let Some(code_line) = lines.next_line() {
        line_no += 1;
        let t = trim(code_line);
        if t.is_empty() && lines.pos >= b.len() {
            break;
        }
        let Some(code) = parse_code(t) else {
            if out.is_empty() {
                return Err(CadError::Malformed(format!(
                    "line {line_no}: group code expected"
                )));
            }
            warnings.push(format!("stopped at line {line_no}: group code expected"));
            break;
        };
        let Some(val) = lines.next_line() else {
            warnings.push("file ends inside a group".into());
            break;
        };
        line_no += 1;
        out.push(Group {
            code,
            val: Val::Text(val),
        });
        if code == 0 && trim(val) == b"EOF" {
            break;
        }
    }
    Ok(out)
}

fn parse_code(t: &[u8]) -> Option<i32> {
    if t.is_empty() || t.len() > 6 {
        return None;
    }
    let (neg, digits) = match t[0] {
        b'-' => (true, &t[1..]),
        _ => (false, t),
    };
    if digits.is_empty() {
        return None;
    }
    let mut v: i32 = 0;
    for &c in digits {
        if !c.is_ascii_digit() {
            return None;
        }
        v = v * 10 + (c - b'0') as i32;
    }
    Some(if neg { -v } else { v })
}

struct Lines<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Lines<'a> {
    /// The next line without its `\n` / `\r\n`.
    fn next_line(&mut self) -> Option<&'a [u8]> {
        if self.pos >= self.b.len() {
            return None;
        }
        let rest = &self.b[self.pos..];
        let (line, adv) = match memchr_nl(rest) {
            Some(i) => (&rest[..i], i + 1),
            None => (rest, rest.len()),
        };
        self.pos += adv;
        Some(line.strip_suffix(b"\r").unwrap_or(line))
    }
}

fn memchr_nl(b: &[u8]) -> Option<usize> {
    b.iter().position(|&c| c == b'\n')
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Str,
    F64,
    I16,
    I32,
    I64,
    Bool,
    Bin,
}

fn kind(code: i32) -> Kind {
    match code {
        10..=59 | 110..=149 | 210..=239 | 460..=469 | 1010..=1059 => Kind::F64,
        60..=79 | 170..=179 | 270..=289 | 370..=389 | 400..=409 | 1060..=1070 => Kind::I16,
        90..=99 | 420..=429 | 440..=449 | 450..=459 | 1071 => Kind::I32,
        160..=169 => Kind::I64,
        290..=299 => Kind::Bool,
        310..=319 | 1004 => Kind::Bin,
        _ => Kind::Str,
    }
}

fn binary<'a>(b: &'a [u8], warnings: &mut Vec<String>) -> Result<Vec<Group<'a>>, CadError> {
    // R12 writes 1-byte codes (255 escapes a 2-byte one); R13 and later 2-byte codes. The
    // first group is `0 SECTION`, so a second byte of 'S' means 1-byte codes.
    let short = b.get(1) == Some(&b'S');
    let mut out = Vec::with_capacity(b.len() / 10);
    let mut p = 0usize;
    let le16 =
        |p: usize| -> Option<i32> { Some(i16::from_le_bytes([*b.get(p)?, *b.get(p + 1)?]) as i32) };
    while p < b.len() {
        let code = if short {
            let c = b[p];
            p += 1;
            if c == 255 {
                let Some(c) = le16(p) else { break };
                p += 2;
                c
            } else {
                c as i32
            }
        } else {
            let Some(c) = le16(p) else { break };
            p += 2;
            c
        };
        let val = match kind(code) {
            Kind::Str => {
                let rest = &b[p.min(b.len())..];
                let Some(n) = rest.iter().position(|&c| c == 0) else {
                    warnings.push("binary file ends inside a string".into());
                    break;
                };
                p += n + 1;
                Val::Text(&rest[..n])
            }
            Kind::F64 => {
                let Some(s) = b.get(p..p + 8) else { break };
                p += 8;
                let v = f64::from_le_bytes(s.try_into().unwrap_or([0; 8]));
                Val::Real(if v.is_finite() { v } else { 0.0 })
            }
            Kind::I16 => {
                let Some(v) = le16(p) else { break };
                p += 2;
                Val::Int(v as i64)
            }
            Kind::I32 => {
                let Some(s) = b.get(p..p + 4) else { break };
                p += 4;
                Val::Int(i32::from_le_bytes(s.try_into().unwrap_or([0; 4])) as i64)
            }
            Kind::I64 => {
                let Some(s) = b.get(p..p + 8) else { break };
                p += 8;
                Val::Int(i64::from_le_bytes(s.try_into().unwrap_or([0; 8])))
            }
            Kind::Bool => {
                let Some(&v) = b.get(p) else { break };
                p += 1;
                Val::Int(v as i64)
            }
            Kind::Bin => {
                let Some(&n) = b.get(p) else { break };
                if b.get(p + 1..p + 1 + n as usize).is_none() {
                    break;
                };
                p += 1 + n as usize;
                Val::Bin
            }
        };
        let eof = code == 0 && matches!(val, Val::Text(t) if t == b"EOF");
        out.push(Group { code, val });
        if eof {
            break;
        }
    }
    if out.is_empty() {
        return Err(CadError::Malformed("empty binary DXF".into()));
    }
    Ok(out)
}

/// Writes groups as a binary DXF (R13+ 2-byte codes); for tests.
#[cfg(test)]
pub(crate) fn to_binary(groups: &[Group<'_>]) -> Vec<u8> {
    let mut o = BINARY_SENTINEL.to_vec();
    for g in groups {
        o.extend_from_slice(&(g.code as i16).to_le_bytes());
        match kind(g.code) {
            Kind::Str => {
                o.extend_from_slice(g.bytes());
                o.push(0);
            }
            Kind::F64 => o.extend_from_slice(&g.f64().to_le_bytes()),
            Kind::I16 => o.extend_from_slice(&(g.i64() as i16).to_le_bytes()),
            Kind::I32 => o.extend_from_slice(&(g.i64() as i32).to_le_bytes()),
            Kind::I64 => o.extend_from_slice(&g.i64().to_le_bytes()),
            Kind::Bool => o.push(g.i64() as u8),
            Kind::Bin => {
                let hex = g.tbytes();
                let bytes: Vec<u8> = hex
                    .chunks(2)
                    .filter_map(|c| u8::from_str_radix(std::str::from_utf8(c).ok()?, 16).ok())
                    .take(255)
                    .collect();
                o.push(bytes.len() as u8);
                o.extend_from_slice(&bytes);
            }
        }
    }
    o
}
