//! FITS headers: 80-character cards in 2880-byte blocks up to `END` (FITS 4.0 §4).
//!
//! Values are parsed as the standard's fixed and free formats allow: strings in single quotes
//! (`''` for a quote, trailing blanks dropped), long strings continued over `CONTINUE` cards
//! (a value ending in `&`), logicals `T` / `F`, integers, reals (with `E` or Fortran `D`
//! exponents) and complex numbers `(re, im)`. `HIERARCH` keywords (ESO) keep their full
//! dotted name with single spaces. Comments follow the first `/` outside a string.

use std::collections::HashMap;

/// One FITS block.
pub const BLOCK: usize = 2880;
/// One header card.
pub const CARD: usize = 80;

/// A keyword's value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Str(String),
    Logical(bool),
    Int(i64),
    Float(f64),
    Complex(f64, f64),
    /// A value indicator with nothing after it.
    Undefined,
}

impl Value {
    /// Parses a value field (after `= `), returning the value and the comment.
    pub fn parse(field: &str) -> (Value, String) {
        let t = field.trim_start();
        if let Some(rest) = t.strip_prefix('\'') {
            // A string: '' is a quote; the string ends at a lone quote.
            let mut s = String::new();
            let mut chars = rest.char_indices().peekable();
            let mut end = rest.len();
            while let Some((i, c)) = chars.next() {
                if c == '\'' {
                    if chars.peek().is_some_and(|&(_, d)| d == '\'') {
                        s.push('\'');
                        chars.next();
                    } else {
                        end = i + 1;
                        break;
                    }
                } else {
                    s.push(c);
                }
            }
            let s = s.trim_end().to_owned();
            return (Value::Str(s), comment(&rest[end.min(rest.len())..]));
        }
        let (v, c) = match t.find('/') {
            Some(i) => (t[..i].trim(), t[i + 1..].trim().to_owned()),
            None => (t.trim(), String::new()),
        };
        (Value::parse_bare(v), c)
    }

    /// Parses an unquoted value: a logical, an integer, a real or a complex number;
    /// anything else is kept as a string.
    pub fn parse_bare(v: &str) -> Value {
        let v = v.trim();
        match v {
            "" => return Value::Undefined,
            "T" => return Value::Logical(true),
            "F" => return Value::Logical(false),
            _ => {}
        }
        if let Some(inner) = v.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
            let mut p = inner.split(',');
            if let (Some(a), Some(b), None) = (p.next(), p.next(), p.next())
                && let (Some(a), Some(b)) = (real(a), real(b))
            {
                return Value::Complex(a, b);
            }
        }
        if let Ok(i) = v.strip_prefix('+').unwrap_or(v).parse::<i64>() {
            return Value::Int(i);
        }
        match real(v) {
            Some(f) => Value::Float(f),
            None => Value::Str(v.to_owned()),
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Value::Int(i) => Some(i as f64),
            Value::Float(f) => Some(f),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            Value::Int(i) => Some(i),
            Value::Float(f) if f.fract() == 0.0 && f.abs() < 9.0e18 => Some(f as i64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            Value::Logical(b) => Some(b),
            _ => None,
        }
    }

    /// The value as text for display (strings without quotes).
    pub fn display(&self) -> String {
        match self {
            Value::Str(s) => s.clone(),
            Value::Logical(b) => if *b { "T" } else { "F" }.to_owned(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => format!("{f}"),
            Value::Complex(a, b) => format!("({a}, {b})"),
            Value::Undefined => String::new(),
        }
    }
}

/// A real number in FITS notation (`1.5E3`, `1.5D3`, `.5`, `+3.`).
fn real(s: &str) -> Option<f64> {
    let s = s.trim();
    let s = s.strip_prefix('+').unwrap_or(s);
    if s.is_empty() || !s.bytes().any(|b| b.is_ascii_digit()) {
        return None;
    }
    if s.bytes().any(|b| {
        !(b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'+' | b'E' | b'e' | b'D' | b'd'))
    }) {
        return None;
    }
    s.replace(['D', 'd'], "E").parse().ok()
}

fn comment(rest: &str) -> String {
    match rest.find('/') {
        Some(i) => rest[i + 1..].trim().to_owned(),
        None => String::new(),
    }
}

/// One header card (a long string's `CONTINUE` cards folded into it).
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    /// Upper-case keyword; `HIERARCH` keywords without the prefix (`ESO DET DIT`).
    pub key: String,
    /// `None` for commentary cards (COMMENT, HISTORY, blank keyword, no value indicator).
    pub value: Option<Value>,
    /// The comment, or the text of a commentary card.
    pub comment: String,
}

/// A parsed header.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Header {
    pub cards: Vec<Card>,
}

impl Header {
    /// Builds a header from keyword / value-text pairs (XISF `FITSKeyword` elements): the
    /// value text is parsed as a card's value field would be.
    pub fn from_pairs<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str, &'a str)>) -> Header {
        let cards = pairs
            .into_iter()
            .map(|(k, v, c)| {
                let key = k.trim().to_ascii_uppercase();
                let value = if matches!(key.as_str(), "COMMENT" | "HISTORY" | "") {
                    None
                } else {
                    Some(Value::parse(v).0)
                };
                let comment = if value.is_none() && c.is_empty() {
                    v.to_owned()
                } else {
                    c.to_owned()
                };
                Card {
                    key,
                    value,
                    comment,
                }
            })
            .collect();
        Header { cards }
    }

    /// The last value of `key` (later cards win, as most readers do).
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.cards
            .iter()
            .rev()
            .find(|c| c.key == key && c.value.is_some())
            .and_then(|c| c.value.as_ref())
    }

    pub fn f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(Value::as_f64)
    }

    pub fn i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(Value::as_i64)
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    pub fn bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(Value::as_bool)
    }

    pub fn has(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// The last value of every keyword, indexed once for many lookups.
    pub(crate) fn keys(&self) -> Keys<'_> {
        let mut map = HashMap::with_capacity(self.cards.len());
        for c in &self.cards {
            if let Some(v) = &c.value {
                map.insert(c.key.as_str(), v);
            }
        }
        Keys(map)
    }
}

/// [`Header`] lookups through an index (the same answers as [`Header::get`] and its kin).
pub(crate) struct Keys<'a>(HashMap<&'a str, &'a Value>);

impl Keys<'_> {
    pub(crate) fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key).copied()
    }

    pub(crate) fn f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(Value::as_f64)
    }

    pub(crate) fn i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(Value::as_i64)
    }

    pub(crate) fn str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    pub(crate) fn has(&self, key: &str) -> bool {
        self.get(key).is_some()
    }
}

/// Parses one header starting at `data[0]`. Returns the header and its length in bytes
/// (whole blocks), or `None` when no `END` card is found.
pub fn parse(data: &[u8]) -> Option<(Header, usize)> {
    let mut cards: Vec<Card> = Vec::new();
    // A string value ending in '&' waits for CONTINUE cards.
    let mut continuing = false;
    let mut pos = 0;
    loop {
        let raw = data.get(pos..pos + CARD)?;
        pos += CARD;
        let line: String = raw
            .iter()
            .map(|&b| {
                if (0x20..0x7f).contains(&b) {
                    b as char
                } else {
                    ' '
                }
            })
            .collect();
        let key = line[..8].trim_end();
        if key == "END" {
            break;
        }
        if key == "CONTINUE" && continuing {
            let (v, c) = Value::parse(&line[8..]);
            if let (Some(last), Value::Str(more)) = (cards.last_mut(), v)
                && let Some(Value::Str(s)) = &mut last.value
            {
                s.pop(); // the '&'
                s.push_str(&more);
                continuing = s.ends_with('&');
                if !c.is_empty() {
                    if !last.comment.is_empty() {
                        last.comment.push(' ');
                    }
                    last.comment.push_str(&c);
                }
                continue;
            }
        }
        let card = if key == "HIERARCH" {
            match line[8..].split_once('=') {
                Some((k, v)) => {
                    let (value, comment) = Value::parse(v);
                    Card {
                        key: k
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ")
                            .to_ascii_uppercase(),
                        value: Some(value),
                        comment,
                    }
                }
                None => Card {
                    key: "HIERARCH".into(),
                    value: None,
                    comment: line[8..].trim().to_owned(),
                },
            }
        } else if &line[8..10] == "= " && !matches!(key, "COMMENT" | "HISTORY" | "") {
            let (value, comment) = Value::parse(&line[10..]);
            Card {
                key: key.to_ascii_uppercase(),
                value: Some(value),
                comment,
            }
        } else {
            Card {
                key: key.to_ascii_uppercase(),
                value: None,
                comment: line[8..].trim_end().to_owned(),
            }
        };
        continuing = matches!(&card.value, Some(Value::Str(s)) if s.ends_with('&'));
        cards.push(card);
    }
    Some((Header { cards }, pos.div_ceil(BLOCK) * BLOCK))
}

/// Whether `head` starts a FITS file: `SIMPLE  =` with `T` in column 30.
pub fn sniff(head: &[u8]) -> bool {
    head.len() >= 30 && head.starts_with(b"SIMPLE  = ") && {
        let v = &head[10..30];
        v.iter().rev().find(|&&b| b != b' ') == Some(&b'T')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(s: &str) -> Vec<u8> {
        let mut b = s.as_bytes().to_vec();
        b.resize(CARD, b' ');
        b
    }

    fn block(cards: &[&str]) -> Vec<u8> {
        let mut d: Vec<u8> = cards.iter().flat_map(|c| card(c)).collect();
        d.extend(card("END"));
        d.resize(d.len().div_ceil(BLOCK) * BLOCK, b' ');
        d
    }

    #[test]
    fn values() {
        let h = block(&[
            "SIMPLE  =                    T / conforms",
            "BITPIX  =                  -32",
            "OBJECT  = 'M 31 ''core''    ' / name",
            "EXPTIME =              1.5D+02",
            "CRVAL1  =   -1.234567890123E+1",
            "CPLX    = (1.0, -2.5)",
            "EMPTY   =",
            "COMMENT   just text",
            "HIERARCH ESO DET  DIT = 0.5 / s",
            "BAD     = garbage",
        ]);
        let (hd, len) = parse(&h).unwrap();
        assert_eq!(len, BLOCK);
        assert_eq!(hd.bool("SIMPLE"), Some(true));
        assert_eq!(hd.i64("BITPIX"), Some(-32));
        assert_eq!(hd.str("OBJECT"), Some("M 31 'core'"));
        assert_eq!(hd.cards[2].comment, "name");
        assert_eq!(hd.f64("EXPTIME"), Some(150.0));
        assert_eq!(hd.f64("CRVAL1"), Some(-12.34567890123));
        assert_eq!(hd.get("CPLX"), Some(&Value::Complex(1.0, -2.5)));
        assert_eq!(hd.get("EMPTY"), Some(&Value::Undefined));
        assert_eq!(hd.cards[7].key, "COMMENT");
        assert_eq!(hd.cards[7].comment, "  just text");
        assert_eq!(hd.f64("ESO DET DIT"), Some(0.5));
        assert_eq!(hd.str("BAD"), Some("garbage"));
    }

    #[test]
    fn continue_strings() {
        let h = block(&[
            "SIMPLE  =                    T",
            "LONG    = 'first part &'  / c1",
            "CONTINUE  'second &'",
            "CONTINUE  'end' / c2",
            "NEXT    = 'x&'",
            "OTHER   = 1",
        ]);
        let (hd, _) = parse(&h).unwrap();
        assert_eq!(hd.str("LONG"), Some("first part second end"));
        assert_eq!(hd.cards[1].comment, "c1 c2");
        // A '&' without CONTINUE stays.
        assert_eq!(hd.str("NEXT"), Some("x&"));
    }

    #[test]
    fn missing_end() {
        let mut h = card("SIMPLE  =                    T");
        h.resize(BLOCK, b' ');
        assert!(parse(&h).is_none());
    }

    #[test]
    fn sniffing() {
        assert!(sniff(&card("SIMPLE  =                    T")));
        assert!(!sniff(&card("SIMPLE  =                    F")));
        assert!(!sniff(b"SIMPLE"));
    }

    #[test]
    fn pairs() {
        let h = Header::from_pairs([
            ("CTYPE1", "'RA---TAN'", ""),
            ("CRVAL1", "10.5", "deg"),
            ("HISTORY", "made", ""),
        ]);
        assert_eq!(h.str("CTYPE1"), Some("RA---TAN"));
        assert_eq!(h.f64("CRVAL1"), Some(10.5));
        assert_eq!(h.cards[2].comment, "made");
    }
}
