//! STEP physical files (ISO 10303-21) as IFC writes them: one pass indexes every instance of
//! the DATA section (its number, its entity type and where its arguments start); arguments
//! are parsed only when asked for. Strings, comments and nested lists are skipped by a byte
//! scanner, so a file of hundreds of megabytes indexes in well under a second; the index is
//! 16 bytes per instance.

use std::collections::HashMap;

use crate::{Error, Result};

/// One instance: its number, interned type and the offset of its argument list's `(`.
#[derive(Clone, Copy, Debug)]
struct Ent {
    id: u32,
    ty: u32,
    start: u64,
}

/// Type number of complex instances (`#1=(A()B());`), whose parts are not read.
pub(crate) const COMPLEX: u32 = u32::MAX;

/// An argument value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum V<'a> {
    /// `$`
    Null,
    /// `*`
    Star,
    Int(i64),
    Real(f64),
    /// A string's raw bytes between the quotes (escapes not yet decoded).
    Str(&'a [u8]),
    /// `.NAME.` without the dots.
    Enum(&'a [u8]),
    Ref(u32),
    /// `"…"` hex binary.
    Bin(&'a [u8]),
    List(Vec<V<'a>>),
    /// `IFCLABEL('x')`: a typed value (select of a defined type).
    Typed(&'a [u8], Box<V<'a>>),
}

impl<'a> V<'a> {
    pub fn as_ref(&self) -> Option<u32> {
        match self {
            V::Ref(r) => Some(*r),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            V::Real(r) => Some(*r),
            V::Int(i) => Some(*i as f64),
            V::Typed(_, v) => v.as_f64(),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            V::Int(i) => Some(*i),
            V::Real(r) if r.fract() == 0.0 => Some(*r as i64),
            V::Typed(_, v) => v.as_i64(),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[V<'a>]> {
        match self {
            V::List(l) => Some(l),
            _ => None,
        }
    }

    pub fn as_enum(&self) -> Option<&'a [u8]> {
        match self {
            V::Enum(e) => Some(e),
            _ => None,
        }
    }

    /// `.T.` / `.F.` (`.U.` and anything else: `None`).
    pub fn as_bool(&self) -> Option<bool> {
        match self.as_enum()? {
            b"T" => Some(true),
            b"F" => Some(false),
            _ => None,
        }
    }

    /// The decoded text of a string (or of a typed string value).
    pub fn as_str(&self) -> Option<String> {
        match self {
            V::Str(s) => Some(decode_string(s)),
            V::Typed(_, v) => v.as_str(),
            _ => None,
        }
    }

    /// The references in a list (others skipped).
    pub fn refs(&self) -> Vec<u32> {
        self.as_list()
            .map(|l| l.iter().filter_map(V::as_ref).collect())
            .unwrap_or_default()
    }

    /// The numbers in a list.
    pub fn reals(&self) -> Vec<f64> {
        self.as_list()
            .map(|l| l.iter().filter_map(V::as_f64).collect())
            .unwrap_or_default()
    }
}

/// How instance numbers map to the index.
enum IdMap {
    /// `slot[id]`, `u32::MAX` for none (numbers are dense enough).
    Dense(Vec<u32>),
    /// The instances are sorted by number: binary search.
    Sorted,
}

/// The indexed DATA section of a file, and its HEADER entities.
pub(crate) struct Step<'a> {
    pub bytes: &'a [u8],
    ents: Vec<Ent>,
    map: IdMap,
    types: Vec<String>,
    by_name: HashMap<String, u32>,
    /// HEADER entities by name (FILE_DESCRIPTION, FILE_NAME, FILE_SCHEMA), their argument
    /// offsets.
    header: Vec<(String, u64)>,
    pub max_id: u32,
}

/// Most instances indexed (the index takes 16 bytes each).
pub(crate) const MAX_INSTANCES: usize = 400_000_000;

impl<'a> Step<'a> {
    /// Indexes `bytes`, refusing what is not a STEP file.
    pub fn index(bytes: &'a [u8]) -> Result<Step<'a>> {
        let mut s = Scanner { b: bytes, i: 0 };
        s.ws();
        if !s.eat(b"ISO-10303-21;") {
            return Err(Error::Malformed(
                "not a STEP file (no ISO-10303-21 line)".into(),
            ));
        }
        let mut header = Vec::new();
        // HEADER; ... ENDSEC;
        s.ws();
        if s.eat(b"HEADER;") {
            loop {
                s.ws();
                if s.eat(b"ENDSEC;") || s.done() {
                    break;
                }
                let name = s.keyword();
                if name.is_empty() {
                    return Err(Error::Malformed("bad STEP header".into()));
                }
                s.ws();
                let start = s.i as u64;
                if !s.skip_args() {
                    return Err(Error::Malformed("unterminated STEP header".into()));
                }
                header.push((String::from_utf8_lossy(name).into_owned(), start));
                s.ws();
                s.eat(b";");
            }
        }
        // DATA; (possibly after other sections, possibly more than one).
        let mut ents: Vec<Ent> = Vec::new();
        let mut types: Vec<String> = Vec::new();
        let mut by_name: HashMap<String, u32> = HashMap::new();
        let mut by_bytes: HashMap<&'a [u8], u32> = HashMap::new();
        let mut sorted = true;
        let mut last = 0u32;
        let mut max_id = 0u32;
        loop {
            s.ws();
            if s.done() || s.eat(b"END-ISO-10303-21;") {
                break;
            }
            if s.eat(b"DATA") {
                // DATA; or DATA('name',(schema));
                s.ws();
                if s.peek() == Some(b'(') && !s.skip_args() {
                    return Err(Error::Malformed("bad DATA section".into()));
                }
                s.ws();
                s.eat(b";");
                loop {
                    s.ws();
                    if s.done() {
                        break;
                    }
                    if s.eat(b"ENDSEC;") {
                        break;
                    }
                    if s.peek() != Some(b'#') {
                        return Err(Error::Malformed(format!(
                            "unexpected text in the DATA section at byte {}",
                            s.i
                        )));
                    }
                    s.i += 1;
                    let Some(id) = s.number() else {
                        return Err(Error::Malformed(format!(
                            "bad instance number at byte {}",
                            s.i
                        )));
                    };
                    s.ws();
                    if !s.eat(b"=") {
                        return Err(Error::Malformed(format!("no '=' after #{id}")));
                    }
                    s.ws();
                    let ty = if s.peek() == Some(b'(') {
                        COMPLEX
                    } else {
                        let name = s.keyword();
                        if name.is_empty() {
                            return Err(Error::Malformed(format!("#{id} has no type")));
                        }
                        match by_bytes.get(name) {
                            Some(&t) => t,
                            None => {
                                let t = types.len() as u32;
                                let upper = String::from_utf8_lossy(name).to_ascii_uppercase();
                                by_name.insert(upper.clone(), t);
                                types.push(upper);
                                by_bytes.insert(name, t);
                                t
                            }
                        }
                    };
                    s.ws();
                    let start = s.i as u64;
                    if !s.skip_args() {
                        return Err(Error::Malformed(format!("#{id} is not terminated")));
                    }
                    s.ws();
                    if !s.eat(b";") {
                        return Err(Error::Malformed(format!("no ';' after #{id}")));
                    }
                    if ents.len() >= MAX_INSTANCES {
                        return Err(Error::TooLarge(format!(
                            "more than {MAX_INSTANCES} instances"
                        )));
                    }
                    if id <= last && !ents.is_empty() {
                        sorted = false;
                    }
                    last = id;
                    max_id = max_id.max(id);
                    ents.push(Ent { id, ty, start });
                }
            } else {
                // Another section (ANCHOR, REFERENCE, SIGNATURE): skip to its ENDSEC.
                match find(&s.b[s.i..], b"ENDSEC;") {
                    Some(k) => s.i += k + 7,
                    None => break,
                }
            }
        }
        if !sorted {
            ents.sort_by_key(|e| e.id);
            ents.dedup_by_key(|e| e.id);
        }
        let map = if (max_id as usize) <= ents.len().saturating_mul(4).saturating_add(1 << 16) {
            let mut slot = vec![u32::MAX; max_id as usize + 1];
            for (k, e) in ents.iter().enumerate() {
                slot[e.id as usize] = k as u32;
            }
            IdMap::Dense(slot)
        } else {
            IdMap::Sorted
        };
        Ok(Step {
            bytes,
            ents,
            map,
            types,
            by_name,
            header,
            max_id,
        })
    }

    pub fn len(&self) -> usize {
        self.ents.len()
    }

    fn slot(&self, id: u32) -> Option<usize> {
        match &self.map {
            IdMap::Dense(v) => v
                .get(id as usize)
                .copied()
                .filter(|&k| k != u32::MAX)
                .map(|k| k as usize),
            IdMap::Sorted => self.ents.binary_search_by_key(&id, |e| e.id).ok(),
        }
    }

    /// The type number of a type name (upper case), if any instance has it.
    pub fn type_number(&self, name: &str) -> Option<u32> {
        self.by_name.get(name).copied()
    }

    /// The entity type of `#id` (upper case), `None` for missing or complex instances.
    pub fn ty(&self, id: u32) -> Option<&str> {
        let e = self.ents[self.slot(id)?];
        self.types.get(e.ty as usize).map(String::as_str)
    }

    pub fn ty_num(&self, id: u32) -> Option<u32> {
        Some(self.ents[self.slot(id)?].ty)
    }

    /// Every instance number of these types, in file order.
    pub fn of_types(&self, names: &[&str]) -> Vec<u32> {
        let tys: Vec<u32> = names.iter().filter_map(|n| self.type_number(n)).collect();
        if tys.is_empty() {
            return Vec::new();
        }
        self.ents
            .iter()
            .filter(|e| tys.contains(&e.ty))
            .map(|e| e.id)
            .collect()
    }

    /// Every (instance number, type number).
    pub fn all(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.ents.iter().map(|e| (e.id, e.ty))
    }

    /// The raw argument bytes of `#id` from its `(`.
    pub fn raw(&self, id: u32) -> Option<&'a [u8]> {
        let e = self.ents[self.slot(id)?];
        self.bytes.get(e.start as usize..)
    }

    /// The arguments of `#id`.
    pub fn args(&self, id: u32) -> Option<Vec<V<'a>>> {
        let raw = self.raw(id)?;
        let mut p = Parser { b: raw, i: 0 };
        match p.value(0)? {
            V::List(l) => Some(l),
            _ => None,
        }
    }

    /// The arguments of a header entity.
    pub fn header_args(&self, name: &str) -> Option<Vec<V<'a>>> {
        let (_, start) = self.header.iter().find(|(n, _)| n == name)?;
        let mut p = Parser {
            b: self.bytes.get(*start as usize..)?,
            i: 0,
        };
        match p.value(0)? {
            V::List(l) => Some(l),
            _ => None,
        }
    }

    /// The first `n` numbers of a point-like instance (`IFCCARTESIANPOINT((x,y,z))`,
    /// `IFCDIRECTION((x,y,z))`), parsed without building values.
    pub fn coords(&self, id: u32) -> Option<([f64; 3], usize)> {
        let raw = self.raw(id)?;
        // "((" then numbers.
        let mut i = 0;
        let mut depth = 0;
        while i < raw.len() && depth < 2 {
            match raw[i] {
                b'(' => depth += 1,
                b' ' | b'\t' | b'\r' | b'\n' => {}
                _ => return None,
            }
            i += 1;
        }
        let mut out = [0.0; 3];
        let mut n = 0;
        loop {
            while i < raw.len() && matches!(raw[i], b' ' | b'\t' | b'\r' | b'\n' | b',') {
                i += 1;
            }
            if i >= raw.len() || raw[i] == b')' {
                break;
            }
            let start = i;
            while i < raw.len() && !matches!(raw[i], b',' | b')' | b' ' | b'\r' | b'\n' | b'\t') {
                i += 1;
            }
            let v = parse_real(&raw[start..i])?;
            if n < 3 {
                out[n] = v;
            }
            n += 1;
        }
        Some((out, n.min(3)))
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

struct Scanner<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Scanner<'a> {
    fn done(&self) -> bool {
        self.i >= self.b.len()
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    /// Skips white space and comments.
    fn ws(&mut self) {
        loop {
            while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() {
                self.i += 1;
            }
            if self.b[self.i.min(self.b.len())..].starts_with(b"/*") {
                match find(&self.b[self.i + 2..], b"*/") {
                    Some(k) => self.i += k + 4,
                    None => self.i = self.b.len(),
                }
            } else {
                break;
            }
        }
    }

    fn eat(&mut self, lit: &[u8]) -> bool {
        if self.b[self.i.min(self.b.len())..].starts_with(lit) {
            self.i += lit.len();
            true
        } else {
            false
        }
    }

    fn keyword(&mut self) -> &'a [u8] {
        let start = self.i;
        while self.i < self.b.len()
            && (self.b[self.i].is_ascii_alphanumeric() || self.b[self.i] == b'_')
        {
            self.i += 1;
        }
        &self.b[start..self.i]
    }

    fn number(&mut self) -> Option<u32> {
        let start = self.i;
        let mut v: u64 = 0;
        while self.i < self.b.len() && self.b[self.i].is_ascii_digit() {
            v = v * 10 + u64::from(self.b[self.i] - b'0');
            if v > u64::from(u32::MAX) {
                return None;
            }
            self.i += 1;
        }
        (self.i > start).then_some(v as u32)
    }

    /// Skips a parenthesised argument list (strings and comments inside), from its `(`.
    fn skip_args(&mut self) -> bool {
        if self.peek() != Some(b'(') {
            return false;
        }
        let b = self.b;
        let mut depth = 0usize;
        let mut i = self.i;
        while i < b.len() {
            match b[i] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        self.i = i + 1;
                        return true;
                    }
                }
                b'\'' => {
                    // To the closing quote; '' is an escaped quote.
                    i += 1;
                    loop {
                        match b[i..].iter().position(|&c| c == b'\'') {
                            Some(k) => {
                                i += k + 1;
                                if b.get(i) == Some(&b'\'') {
                                    i += 1;
                                } else {
                                    break;
                                }
                            }
                            None => return false,
                        }
                    }
                    continue;
                }
                b'"' => match b[i + 1..].iter().position(|&c| c == b'"') {
                    Some(k) => i += k + 1,
                    None => return false,
                },
                b'/' if b.get(i + 1) == Some(&b'*') => match find(&b[i + 2..], b"*/") {
                    Some(k) => i += k + 3,
                    None => return false,
                },
                _ => {}
            }
            i += 1;
        }
        false
    }
}

/// Deepest list nesting parsed.
const MAX_NESTING: usize = 64;

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
        if self.b[self.i.min(self.b.len())..].starts_with(b"/*") {
            match find(&self.b[self.i..], b"*/") {
                Some(k) => self.i += k + 2,
                None => self.i = self.b.len(),
            }
            self.ws();
        }
    }

    fn value(&mut self, depth: usize) -> Option<V<'a>> {
        if depth > MAX_NESTING {
            return None;
        }
        self.ws();
        let c = *self.b.get(self.i)?;
        match c {
            b'(' => {
                self.i += 1;
                let mut items = Vec::new();
                loop {
                    self.ws();
                    match self.b.get(self.i)? {
                        b')' => {
                            self.i += 1;
                            return Some(V::List(items));
                        }
                        b',' => self.i += 1,
                        _ => items.push(self.value(depth + 1)?),
                    }
                }
            }
            b'$' => {
                self.i += 1;
                Some(V::Null)
            }
            b'*' => {
                self.i += 1;
                Some(V::Star)
            }
            b'#' => {
                self.i += 1;
                let start = self.i;
                let mut v: u64 = 0;
                while self.i < self.b.len() && self.b[self.i].is_ascii_digit() {
                    v = v * 10 + u64::from(self.b[self.i] - b'0');
                    if v > u64::from(u32::MAX) {
                        return None;
                    }
                    self.i += 1;
                }
                (self.i > start).then_some(V::Ref(v as u32))
            }
            b'\'' => {
                self.i += 1;
                let start = self.i;
                loop {
                    let k = self.b[self.i..].iter().position(|&c| c == b'\'')?;
                    self.i += k + 1;
                    if self.b.get(self.i) == Some(&b'\'') {
                        self.i += 1;
                    } else {
                        return Some(V::Str(&self.b[start..self.i - 1]));
                    }
                }
            }
            b'"' => {
                self.i += 1;
                let start = self.i;
                let k = self.b[self.i..].iter().position(|&c| c == b'"')?;
                self.i += k + 1;
                Some(V::Bin(&self.b[start..start + k]))
            }
            b'.' if self
                .b
                .get(self.i + 1)
                .is_some_and(|c| c.is_ascii_alphabetic()) =>
            {
                self.i += 1;
                let start = self.i;
                let k = self.b[self.i..].iter().position(|&c| c == b'.')?;
                self.i += k + 1;
                Some(V::Enum(&self.b[start..start + k]))
            }
            b'-' | b'+' | b'.' | b'0'..=b'9' => {
                let start = self.i;
                self.i += 1;
                while self.i < self.b.len()
                    && matches!(
                        self.b[self.i],
                        b'0'..=b'9' | b'.' | b'e' | b'E' | b'-' | b'+'
                    )
                {
                    self.i += 1;
                }
                let t = &self.b[start..self.i];
                if t.iter().any(|c| matches!(c, b'.' | b'e' | b'E')) {
                    parse_real(t).map(V::Real)
                } else {
                    std::str::from_utf8(t).ok()?.parse::<i64>().ok().map(V::Int)
                }
            }
            c if c.is_ascii_alphabetic() => {
                let start = self.i;
                while self.i < self.b.len()
                    && (self.b[self.i].is_ascii_alphanumeric() || self.b[self.i] == b'_')
                {
                    self.i += 1;
                }
                let name = &self.b[start..self.i];
                self.ws();
                if self.b.get(self.i) != Some(&b'(') {
                    return None;
                }
                // A typed value holds one value in parentheses.
                let inner = self.value(depth + 1)?;
                let inner = match inner {
                    V::List(mut l) if l.len() == 1 => l.pop()?,
                    other => other,
                };
                Some(V::Typed(name, Box::new(inner)))
            }
            _ => None,
        }
    }
}

/// A STEP real (`1.`, `-2.5E-3`, `.5`).
pub(crate) fn parse_real(t: &[u8]) -> Option<f64> {
    let s = std::str::from_utf8(t).ok()?;
    if let Ok(v) = s.parse::<f64>() {
        return Some(v);
    }
    // "1.E-5": Rust wants a digit after the point.
    let fixed = s.replace(".E", ".0E").replace(".e", ".0e");
    fixed.parse::<f64>().ok()
}

/// The text of a STEP string: `''` is a quote, `\\` a backslash, `\S\c` a Latin-1 high
/// character, `\X\hh` a byte, `\X2\hhhh…\X0\` UTF-16 and `\X4\hhhhhhhh…\X0\` UTF-32.
pub(crate) fn decode_string(raw: &[u8]) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    let hex = |b: &[u8]| -> Option<u32> {
        let s = std::str::from_utf8(b).ok()?;
        u32::from_str_radix(s, 16).ok()
    };
    while i < raw.len() {
        let c = raw[i];
        if c == b'\'' && raw.get(i + 1) == Some(&b'\'') {
            out.push('\'');
            i += 2;
        } else if c == b'\\' {
            let rest = &raw[i..];
            if rest.starts_with(b"\\\\") {
                out.push('\\');
                i += 2;
            } else if rest.starts_with(b"\\S\\") && rest.len() >= 4 {
                out.push(char::from_u32(u32::from(rest[3]) + 128).unwrap_or('?'));
                i += 4;
            } else if rest.starts_with(b"\\X\\") && rest.len() >= 5 {
                match hex(&rest[3..5]) {
                    Some(v) => out.push(char::from_u32(v).unwrap_or('?')),
                    None => out.push('?'),
                }
                i += 5;
            } else if rest.starts_with(b"\\X2\\") || rest.starts_with(b"\\X4\\") {
                let width = if rest[2] == b'2' { 4 } else { 8 };
                let body_end = find(&rest[4..], b"\\X0\\").map_or(rest.len(), |k| k + 4);
                let body = &rest[4..body_end];
                let units: Vec<u32> = body.chunks(width).filter_map(hex).collect();
                if width == 4 {
                    let u16s: Vec<u16> = units.iter().map(|&u| u as u16).collect();
                    out.push_str(&String::from_utf16_lossy(&u16s));
                } else {
                    out.extend(units.iter().map(|&u| char::from_u32(u).unwrap_or('?')));
                }
                i += (body_end + 4).min(rest.len());
            } else if rest.starts_with(b"\\P") && rest.len() >= 4 && rest[3] == b'\\' {
                // Code page switch: ignored.
                i += 4;
            } else {
                out.push('\\');
                i += 1;
            }
        } else if c < 0x80 {
            out.push(c as char);
            i += 1;
        } else {
            // Non-ASCII bytes some writers leave raw: UTF-8 if valid, else Latin-1.
            let end = raw[i..]
                .iter()
                .position(|&b| b < 0x80)
                .map_or(raw.len(), |k| i + k);
            match std::str::from_utf8(&raw[i..end]) {
                Ok(s) => out.push_str(s),
                Err(_) => out.extend(raw[i..end].iter().map(|&b| b as char)),
            }
            i = end;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &[u8] = b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('ViewDefinition [CoordinationView]'),'2;1');\nFILE_NAME('a.ifc','2024-01-01T00:00:00',('Ann'),('Org'),'pre','sys','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n#1=IFCCARTESIANPOINT((1.,2.5,-3.E-1));\n/* a comment; with ( */\n#5= IFCLABEL_TEST('It''s (x)',.T.,$,*,#1,(1,2,(3)),IFCLABEL('a'),\"0FF\");\n#3=(A()B(1));\n#2=IFCDIRECTION((0.,0.,1.));\nENDSEC;\nEND-ISO-10303-21;\n";

    #[test]
    fn indexes_and_parses() {
        let s = Step::index(FILE).unwrap();
        assert_eq!(s.len(), 4);
        assert_eq!(s.ty(1), Some("IFCCARTESIANPOINT"));
        assert_eq!(s.ty(3), None);
        assert_eq!(s.ty_num(3), Some(COMPLEX));
        assert_eq!(s.coords(1), Some(([1.0, 2.5, -0.3], 3)));
        let a = s.args(5).unwrap();
        assert_eq!(a[0].as_str().as_deref(), Some("It's (x)"));
        assert_eq!(a[1].as_bool(), Some(true));
        assert_eq!(a[2], V::Null);
        assert_eq!(a[3], V::Star);
        assert_eq!(a[4].as_ref(), Some(1));
        assert_eq!(a[5].as_list().unwrap().len(), 3);
        assert_eq!(a[6].as_str().as_deref(), Some("a"));
        assert_eq!(a[7], V::Bin(b"0FF"));
        let schema = s.header_args("FILE_SCHEMA").unwrap();
        assert_eq!(
            schema[0].as_list().unwrap()[0].as_str().as_deref(),
            Some("IFC4")
        );
        assert_eq!(s.of_types(&["IFCDIRECTION"]), vec![2]);
    }

    #[test]
    fn strings_decode() {
        assert_eq!(decode_string(b"Gr\\X2\\00FC\\X0\\n"), "Grün");
        assert_eq!(decode_string(b"\\S\\|"), "ü");
        assert_eq!(decode_string(b"a\\X\\E9b"), "aéb");
        assert_eq!(decode_string(b"\\X4\\0001F600\\X0\\"), "😀");
    }

    #[test]
    fn junk_is_refused() {
        assert!(Step::index(b"hello").is_err());
        assert!(Step::index(b"ISO-10303-21;\nDATA;\n#1=X('unterminated);\n").is_err());
        assert!(Step::index(b"ISO-10303-21;\nDATA;\n#1=X((((;\n").is_err());
        // Nesting beyond the cap parses as nothing rather than overflowing.
        let mut deep = b"ISO-10303-21;\nDATA;\n#1=X(".to_vec();
        deep.extend(std::iter::repeat_n(b'(', 500));
        deep.extend(std::iter::repeat_n(b')', 501));
        deep.extend(b";\nENDSEC;\n");
        let s = Step::index(&deep).unwrap();
        assert!(s.args(1).is_none());
    }
}
