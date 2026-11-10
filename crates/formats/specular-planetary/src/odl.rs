//! PDS3 labels: the Object Description Language (ODL, PVL). Statements `KEY = value`, nested
//! `OBJECT`/`END_OBJECT` and `GROUP`/`END_GROUP` blocks, up to `END`; values are quoted text,
//! symbols ('literal' or bare words, dates and times), numbers with an optional `<unit>`
//! (including based integers such as `16#FF7FFFFB#`), `(sequences)` and `{sets}`, which may
//! nest and span lines. `/* comments */` are skipped.

use crate::{Error, Result};

/// Statements parsed at most (a label of millions of lines is not a label).
const MAX_STATEMENTS: usize = 1 << 20;
/// Nesting depth of blocks and of sequences.
const MAX_DEPTH: usize = 64;

/// A value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// Quoted text, with line breaks and runs of spaces folded to one space.
    Text(String),
    /// A symbol: a 'literal' or a bare word (identifiers, dates, times, `N/A`).
    Symbol(String),
    Number {
        value: f64,
        unit: Option<String>,
    },
    /// A based integer (`16#FF7FFFFB#`): for real samples, the bits of a real.
    Based {
        bits: u64,
        radix: u32,
    },
    Seq(Vec<Value>),
    Set(Vec<Value>),
}

impl Value {
    /// The number, or a symbol that reads as one.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number { value, .. } => Some(*value),
            Value::Based { bits, .. } => Some(*bits as f64),
            Value::Symbol(s) | Value::Text(s) => number(s.trim()),
            _ => None,
        }
    }

    pub fn unit(&self) -> Option<&str> {
        match self {
            Value::Number { unit, .. } => unit.as_deref(),
            _ => None,
        }
    }

    /// Text or a symbol, as written (without quotes).
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Text(s) | Value::Symbol(s) => Some(s),
            _ => None,
        }
    }

    /// The items of a sequence or set, or the value alone.
    pub fn items(&self) -> &[Value] {
        match self {
            Value::Seq(v) | Value::Set(v) => v,
            v => std::slice::from_ref(v),
        }
    }

    /// The value as one line of text: numbers as parsed with their unit, lists comma
    /// separated in brackets.
    pub fn display(&self) -> String {
        match self {
            Value::Text(s) | Value::Symbol(s) => s.clone(),
            Value::Number { value, unit } => {
                let n = trim_number(*value);
                match unit {
                    Some(u) => format!("{n} <{u}>"),
                    None => n,
                }
            }
            Value::Based { bits, radix } => match radix {
                16 => format!("16#{bits:X}#"),
                2 => format!("2#{bits:b}#"),
                8 => format!("8#{bits:o}#"),
                r => format!("{r}#{bits}#"),
            },
            Value::Seq(v) => format!(
                "({})",
                v.iter().map(Value::display).collect::<Vec<_>>().join(", ")
            ),
            Value::Set(v) => format!(
                "{{{}}}",
                v.iter().map(Value::display).collect::<Vec<_>>().join(", ")
            ),
        }
    }
}

fn trim_number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// A `base#digits#` token's digits and radix (unsigned).
pub fn based(s: &str) -> Option<(u64, u32)> {
    let (base, rest) = s.split_once('#')?;
    let digits = rest.strip_suffix('#')?;
    let radix: u32 = base.parse().ok().filter(|r| (2..=16).contains(r))?;
    Some((u64::from_str_radix(digits, radix).ok()?, radix))
}

/// A number token: decimal, real with exponent, or `base#digits#`.
pub fn number(s: &str) -> Option<f64> {
    if let Some((base, rest)) = s.split_once('#') {
        let digits = rest.strip_suffix('#')?;
        let (neg, base) = match base.strip_prefix('-') {
            Some(b) => (true, b),
            None => (false, base.strip_prefix('+').unwrap_or(base)),
        };
        let radix: u32 = base.parse().ok().filter(|r| (2..=16).contains(r))?;
        let v = u64::from_str_radix(digits, radix).ok()? as f64;
        return Some(if neg { -v } else { v });
    }
    let first = s.as_bytes().first()?;
    if !(first.is_ascii_digit() || matches!(first, b'-' | b'+' | b'.')) {
        return None;
    }
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// What a block is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockKind {
    Object,
    Group,
}

/// A statement.
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    /// `KEY = value` (a pointer's key keeps its `^`).
    Attr(String, Value),
    Block(Block),
}

/// An `OBJECT` or `GROUP` and what it holds (a label's top level is a block too).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Block {
    pub kind: Option<BlockKind>,
    pub name: String,
    pub entries: Vec<Entry>,
}

impl Block {
    /// The value of `key` among this block's own attributes (case-insensitive).
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.iter().find_map(|e| match e {
            Entry::Attr(k, v) if k.eq_ignore_ascii_case(key) => Some(v),
            _ => None,
        })
    }

    pub fn f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(Value::as_f64)
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    /// The direct sub-blocks.
    pub fn blocks(&self) -> impl Iterator<Item = &Block> {
        self.entries.iter().filter_map(|e| match e {
            Entry::Block(b) => Some(b),
            _ => None,
        })
    }

    /// The first block named `name` at any depth.
    pub fn find(&self, name: &str) -> Option<&Block> {
        for b in self.blocks() {
            if b.name.eq_ignore_ascii_case(name) {
                return Some(b);
            }
            if let Some(f) = b.find(name) {
                return Some(f);
            }
        }
        None
    }

    /// The value of `key` here, else in the first block holding it (depth first).
    pub fn deep(&self, key: &str) -> Option<&Value> {
        self.get(key)
            .or_else(|| self.blocks().find_map(|b| b.deep(key)))
    }

    /// Every attribute as (`BLOCK.KEY`, value text), in label order.
    pub fn flatten(&self, out: &mut Vec<(String, String)>, prefix: &str, max: usize) {
        for e in &self.entries {
            if out.len() >= max {
                return;
            }
            match e {
                Entry::Attr(k, v) => out.push((format!("{prefix}{k}"), v.display())),
                Entry::Block(b) => b.flatten(out, &format!("{prefix}{}.", b.name), max),
            }
        }
    }
}

/// A parsed label: its statements and where it ended (one past the `END` statement's line).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Label {
    pub root: Block,
    pub end: usize,
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn skip_space(&mut self, newlines: bool) {
        while self.i < self.s.len() {
            let c = self.s[self.i];
            if c == b'/' && self.s.get(self.i + 1) == Some(&b'*') {
                match find(&self.s[self.i + 2..], b"*/") {
                    Some(n) => self.i += n + 4,
                    None => self.i = self.s.len(),
                }
            } else if c == b' ' || c == b'\t' || c == 0 || (newlines && is_newline(c)) {
                self.i += 1;
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    /// A bare word: up to space, `=`, a bracket, a comma or the end of the line.
    fn word(&mut self) -> &'a str {
        let start = self.i;
        while let Some(c) = self.peek() {
            if c.is_ascii_whitespace()
                || matches!(c, b'=' | b',' | b')' | b'}' | b'(' | b'{' | b'<' | b'"')
                || (c == b'/' && self.s.get(self.i + 1) == Some(&b'*'))
            {
                break;
            }
            self.i += 1;
        }
        std::str::from_utf8(&self.s[start..self.i]).unwrap_or("")
    }

    fn value(&mut self, depth: usize) -> Result<Value> {
        if depth > MAX_DEPTH {
            return Err(Error::Malformed("label values nest too deep".into()));
        }
        self.skip_space(true);
        match self.peek() {
            Some(b'"') => {
                self.i += 1;
                let start = self.i;
                let n = self.s[start..]
                    .iter()
                    .position(|&c| c == b'"')
                    .unwrap_or(self.s.len() - start);
                self.i = (start + n + 1).min(self.s.len());
                Ok(Value::Text(fold(&self.s[start..start + n])))
            }
            Some(b'\'') => {
                self.i += 1;
                let start = self.i;
                let n = self.s[start..]
                    .iter()
                    .position(|&c| c == b'\'' || is_newline(c))
                    .unwrap_or(self.s.len() - start);
                self.i = (start + n + 1).min(self.s.len());
                Ok(Value::Symbol(fold(&self.s[start..start + n])))
            }
            Some(open @ (b'(' | b'{')) => {
                self.i += 1;
                let close = if open == b'(' { b')' } else { b'}' };
                let mut items = Vec::new();
                loop {
                    self.skip_space(true);
                    match self.peek() {
                        None => break,
                        Some(c) if c == close => {
                            self.i += 1;
                            break;
                        }
                        Some(b',') => self.i += 1,
                        Some(b')' | b'}') => {
                            // A stray closing bracket: end this list.
                            self.i += 1;
                            break;
                        }
                        Some(_) => {
                            let before = self.i;
                            items.push(self.value(depth + 1)?);
                            if self.i == before {
                                self.i += 1;
                            }
                        }
                    }
                }
                Ok(if open == b'(' {
                    Value::Seq(items)
                } else {
                    Value::Set(items)
                })
            }
            _ => {
                let w = self.word();
                if let Some((bits, radix)) = based(w) {
                    return Ok(Value::Based { bits, radix });
                }
                let unit = self.unit();
                match number(w) {
                    Some(value) => Ok(Value::Number { value, unit }),
                    None => Ok(Value::Symbol(w.to_owned())),
                }
            }
        }
    }

    /// A `<unit>` after a number, on the same line.
    fn unit(&mut self) -> Option<String> {
        let save = self.i;
        self.skip_space(false);
        if self.peek() != Some(b'<') {
            self.i = save;
            return None;
        }
        let start = self.i + 1;
        let n = self.s[start..]
            .iter()
            .position(|&c| c == b'>' || is_newline(c))?;
        self.i = start + n + 1;
        Some(fold(&self.s[start..start + n]))
    }

    /// Moves past the rest of the current line.
    fn next_line(&mut self) {
        while let Some(c) = self.peek() {
            self.i += 1;
            if c == b'\n' {
                break;
            }
        }
    }
}

fn is_newline(c: u8) -> bool {
    c == b'\n' || c == b'\r'
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

/// Text with line breaks and runs of whitespace folded to single spaces, trimmed.
fn fold(b: &[u8]) -> String {
    let s = String::from_utf8_lossy(b);
    s.split_ascii_whitespace().collect::<Vec<_>>().join(" ")
}

/// Parses an ODL label from the start of `bytes` up to its `END` statement (or the end of
/// the bytes). Binary data after `END` is never looked at.
pub fn parse(bytes: &[u8]) -> Result<Label> {
    let mut p = Parser { s: bytes, i: 0 };
    let mut stack: Vec<Block> = vec![Block::default()];
    let mut statements = 0usize;
    loop {
        p.skip_space(true);
        let Some(c) = p.peek() else { break };
        statements += 1;
        if statements > MAX_STATEMENTS {
            return Err(Error::Malformed("label too long".into()));
        }
        if !(c.is_ascii_alphanumeric() || c == b'^' || c == b'_') {
            // Not a statement: a binary byte (no END before the data) or stray text.
            if !c.is_ascii() || (c < 0x20 && !c.is_ascii_whitespace()) {
                break;
            }
            p.next_line();
            continue;
        }
        let key = p.word().to_owned();
        p.skip_space(false);
        let upper = key.to_ascii_uppercase();
        if p.peek() != Some(b'=') {
            if upper == "END" {
                p.next_line();
                break;
            }
            if upper == "END_OBJECT" || upper == "END_GROUP" {
                close(&mut stack);
                p.next_line();
                continue;
            }
            // A line that is not a statement (a continuation of free text): skip it.
            p.next_line();
            continue;
        }
        p.i += 1;
        let value = p.value(0)?;
        match upper.as_str() {
            "OBJECT" | "GROUP" | "BEGIN_OBJECT" | "BEGIN_GROUP" => {
                if stack.len() > MAX_DEPTH {
                    return Err(Error::Malformed("label blocks nest too deep".into()));
                }
                stack.push(Block {
                    kind: Some(if upper.ends_with("OBJECT") {
                        BlockKind::Object
                    } else {
                        BlockKind::Group
                    }),
                    name: value.display(),
                    entries: Vec::new(),
                });
            }
            "END_OBJECT" | "END_GROUP" => close(&mut stack),
            _ => {
                if let Some(top) = stack.last_mut() {
                    top.entries.push(Entry::Attr(key, value));
                }
            }
        }
    }
    while stack.len() > 1 {
        close(&mut stack);
    }
    let root = stack.pop().unwrap_or_default();
    Ok(Label { root, end: p.i })
}

fn close(stack: &mut Vec<Block>) {
    if stack.len() > 1
        && let Some(b) = stack.pop()
        && let Some(parent) = stack.last_mut()
    {
        parent.entries.push(Entry::Block(b));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LABEL: &str = "PDS_VERSION_ID = PDS3\r\n\
        /* comment */\r\n\
        RECORD_BYTES = 2072\r\n\
        ^IMAGE = (\"N1.IMG\", 4)\r\n\
        ^HEADER = 3 <BYTES>\r\n\
        DATA_SET_NAME = \"MER 2 MARS PANCAM\r\n      VERSION 1.0\"\r\n\
        FILTER_NAME = ('CL1','CL2')\r\n\
        IMAGE_OBSERVATION_TYPE = {\"OPNAV\"}\r\n\
        START_TIME = 2004-037T02:07:06.418\r\n\
        DETECTOR_TEMPERATURE = -89.243546 <DEGC>\r\n\
        MRO:SPECIAL = 16#FF7FFFFB#\r\n\
        NESTED = ((1, 2),\r\n (3, 4))\r\n\
        OBJECT = IMAGE\r\n  LINES = 1024\r\n  OBJECT = INNER\r\n   X = 1\r\n  END_OBJECT = INNER\r\n\
        END_OBJECT = IMAGE\r\n\
        GROUP = G\r\n  Y = N/A\r\nEND_GROUP\r\n\
        END\r\n\x00\x01binary";

    #[test]
    fn parses_a_label() {
        let l = parse(LABEL.as_bytes()).unwrap();
        let r = &l.root;
        assert_eq!(r.str("PDS_VERSION_ID"), Some("PDS3"));
        assert_eq!(r.f64("record_bytes"), Some(2072.0));
        let p = r.get("^IMAGE").unwrap();
        assert_eq!(p.items()[0].as_str(), Some("N1.IMG"));
        assert_eq!(p.items()[1].as_f64(), Some(4.0));
        assert_eq!(r.get("^HEADER").unwrap().unit(), Some("BYTES"));
        assert_eq!(
            r.str("DATA_SET_NAME"),
            Some("MER 2 MARS PANCAM VERSION 1.0")
        );
        assert_eq!(r.get("FILTER_NAME").unwrap().display(), "(CL1, CL2)");
        assert_eq!(
            r.get("IMAGE_OBSERVATION_TYPE").unwrap().display(),
            "{OPNAV}"
        );
        assert_eq!(r.str("START_TIME"), Some("2004-037T02:07:06.418"));
        assert_eq!(r.get("DETECTOR_TEMPERATURE").unwrap().unit(), Some("DEGC"));
        assert_eq!(r.f64("MRO:SPECIAL"), Some(f64::from(0xFF7F_FFFBu32)));
        assert_eq!(r.get("MRO:SPECIAL").unwrap().display(), "16#FF7FFFFB#");
        assert_eq!(r.get("NESTED").unwrap().items().len(), 2);
        let img = r.find("IMAGE").unwrap();
        assert_eq!(img.f64("LINES"), Some(1024.0));
        assert_eq!(r.find("INNER").unwrap().f64("X"), Some(1.0));
        assert_eq!(r.find("G").unwrap().str("Y"), Some("N/A"));
        assert_eq!(&LABEL.as_bytes()[l.end..l.end + 2], b"\x00\x01");
        let mut flat = Vec::new();
        r.flatten(&mut flat, "", 100);
        assert!(flat.contains(&("IMAGE.INNER.X".into(), "1".into())));
    }

    #[test]
    fn numbers() {
        assert_eq!(number("2#0101#"), Some(5.0));
        assert_eq!(number("-1.5E3"), Some(-1500.0));
        assert_eq!(number("N/A"), None);
        assert_eq!(number("2004-037T02"), None);
    }

    #[test]
    fn hostile_labels() {
        for t in [
            &b"A = ((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((("[..],
            b"OBJECT = X\nOBJECT = Y\n",
            b"A = \"unterminated",
            b"A = (1, 2",
            b"A = 1 <unit",
            b"= 3\nEND_OBJECT\nEND_OBJECT\n",
            b"",
        ] {
            let _ = parse(t);
        }
        let deep = "OBJECT = X\n".repeat(100);
        assert!(parse(deep.as_bytes()).is_err());
    }
}
