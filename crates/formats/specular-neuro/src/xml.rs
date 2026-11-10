//! A small, bounded XML reader for GIFTI and the CIFTI-2 extension: elements with their
//! attributes and the spans of their character data, namespace prefixes dropped. Comments,
//! processing instructions, a DOCTYPE (with an internal subset) and CDATA are understood;
//! DTDs and external entities are not.
//!
//! Text is kept as spans of the source, not copied: a GIFTI `<Data>` element can hold most
//! of a file, and it is decoded straight from those bytes.

use std::borrow::Cow;

use crate::{Error, Result};

/// Deepest nesting accepted.
const MAX_DEPTH: usize = 128;
/// Most elements accepted in one document.
const MAX_ELEMENTS: usize = 1 << 21;
/// Most attributes on one element.
const MAX_ATTRS: usize = 256;

/// A piece of an element's character data: source bytes from..to, and whether they are a
/// CDATA section (taken literally) or plain text (with references).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub from: usize,
    pub to: usize,
    pub cdata: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Element {
    /// Local name (no namespace prefix).
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Element>,
    /// The character data directly inside the element, in order.
    pub spans: Vec<Span>,
}

impl Element {
    /// An attribute's value by local name.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// The first child element named `name`.
    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.name == name)
    }

    /// The child elements named `name`.
    pub fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// The character data as bytes: borrowed when it is one plain span without references
    /// (the usual `<Data>`), else assembled.
    pub fn bytes<'a>(&self, src: &'a [u8]) -> Cow<'a, [u8]> {
        if let [s] = self.spans.as_slice()
            && (s.cdata || !src[s.from..s.to].contains(&b'&'))
        {
            return Cow::Borrowed(&src[s.from..s.to]);
        }
        let mut out = Vec::new();
        for s in &self.spans {
            let b = &src[s.from..s.to];
            if s.cdata {
                out.extend_from_slice(b);
            } else {
                out.extend_from_slice(unescape(&String::from_utf8_lossy(b)).as_bytes());
            }
        }
        Cow::Owned(out)
    }

    /// The character data as text, trimmed.
    pub fn text(&self, src: &[u8]) -> String {
        String::from_utf8_lossy(&self.bytes(src)).trim().to_owned()
    }

    /// The trimmed text of the first child named `name`, when it has any.
    pub fn child_text(&self, name: &str, src: &[u8]) -> Option<String> {
        self.child(name)
            .map(|c| c.text(src))
            .filter(|t| !t.is_empty())
    }
}

fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// Replaces the predefined and numeric character references.
pub fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';').filter(|&e| e <= 12) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..end];
        let c = match ent {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => ent
                .strip_prefix("#x")
                .or_else(|| ent.strip_prefix("#X"))
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| ent.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match c {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    let first = needle[0];
    let mut i = 0;
    while i + needle.len() <= hay.len() {
        let j = hay[i..=hay.len() - needle.len()]
            .iter()
            .position(|&b| b == first)?;
        i += j;
        if hay[i..].starts_with(needle) {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn is_name(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':') || b >= 0x80
}

/// Parses a document and returns its root element.
pub fn parse(src: &[u8]) -> Result<Element> {
    let bad = |m: &str| Error::Malformed(format!("XML: {m}"));
    let b = src;
    let mut i = 0;
    // A UTF-8 byte order mark.
    if b.starts_with(&[0xef, 0xbb, 0xbf]) {
        i = 3;
    }
    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;
    let mut count = 0usize;
    while i < b.len() {
        if b[i] != b'<' {
            let end = b[i..]
                .iter()
                .position(|&c| c == b'<')
                .map_or(b.len(), |e| i + e);
            if let Some(top) = stack.last_mut()
                && b[i..end].iter().any(|c| !c.is_ascii_whitespace())
            {
                top.spans.push(Span {
                    from: i,
                    to: end,
                    cdata: false,
                });
            }
            i = end;
            continue;
        }
        let rest = &b[i..];
        if rest.starts_with(b"<!--") {
            i += find(rest, b"-->").ok_or_else(|| bad("unterminated comment"))? + 3;
            continue;
        }
        if rest.starts_with(b"<![CDATA[") {
            let end = find(rest, b"]]>").ok_or_else(|| bad("unterminated CDATA"))?;
            if let Some(top) = stack.last_mut() {
                top.spans.push(Span {
                    from: i + 9,
                    to: i + end,
                    cdata: true,
                });
            }
            i += end + 3;
            continue;
        }
        if rest.starts_with(b"<?") {
            i += find(rest, b"?>").ok_or_else(|| bad("unterminated declaration"))? + 2;
            continue;
        }
        if rest.starts_with(b"<!") {
            // DOCTYPE, possibly with an internal subset in brackets.
            let mut depth = 0i32;
            let mut j = 2;
            loop {
                let c = *rest.get(j).ok_or_else(|| bad("unterminated DOCTYPE"))?;
                match c {
                    b'[' => depth += 1,
                    b']' => depth -= 1,
                    b'>' if depth <= 0 => break,
                    _ => {}
                }
                j += 1;
            }
            i += j + 1;
            continue;
        }
        if rest.starts_with(b"</") {
            let end = rest
                .iter()
                .position(|&c| c == b'>')
                .ok_or_else(|| bad("unterminated end tag"))?;
            let name = String::from_utf8_lossy(&rest[2..end]);
            let name = local(name.trim());
            let el = stack.pop().ok_or_else(|| bad("unbalanced end tag"))?;
            if el.name != name {
                return Err(bad(&format!("<{}> closed by </{name}>", el.name)));
            }
            i += end + 1;
            match stack.last_mut() {
                Some(parent) => parent.children.push(el),
                None => {
                    root = Some(el);
                    break;
                }
            }
            continue;
        }
        // A start tag.
        let mut j = 1;
        while j < rest.len() && is_name(rest[j]) {
            j += 1;
        }
        if j == 1 {
            return Err(bad("bad tag"));
        }
        count += 1;
        if count > MAX_ELEMENTS {
            return Err(Error::TooLarge);
        }
        let mut el = Element {
            name: local(&String::from_utf8_lossy(&rest[1..j])).to_owned(),
            ..Element::default()
        };
        let mut closed = false;
        loop {
            while j < rest.len() && rest[j].is_ascii_whitespace() {
                j += 1;
            }
            match rest.get(j) {
                None => return Err(bad("unterminated tag")),
                Some(b'>') => {
                    j += 1;
                    break;
                }
                Some(b'/') if rest.get(j + 1) == Some(&b'>') => {
                    j += 2;
                    closed = true;
                    break;
                }
                _ => {}
            }
            let k0 = j;
            while j < rest.len() && is_name(rest[j]) {
                j += 1;
            }
            if j == k0 {
                return Err(bad("bad attribute"));
            }
            let key = local(&String::from_utf8_lossy(&rest[k0..j])).to_owned();
            while j < rest.len() && rest[j].is_ascii_whitespace() {
                j += 1;
            }
            if rest.get(j) != Some(&b'=') {
                return Err(bad("attribute without a value"));
            }
            j += 1;
            while j < rest.len() && rest[j].is_ascii_whitespace() {
                j += 1;
            }
            let q = *rest.get(j).ok_or_else(|| bad("unterminated tag"))?;
            if q != b'"' && q != b'\'' {
                return Err(bad("unquoted attribute"));
            }
            let v0 = j + 1;
            let v1 = rest[v0..]
                .iter()
                .position(|&c| c == q)
                .ok_or_else(|| bad("unterminated attribute"))?
                + v0;
            if el.attrs.len() >= MAX_ATTRS {
                return Err(Error::TooLarge);
            }
            el.attrs
                .push((key, unescape(&String::from_utf8_lossy(&rest[v0..v1]))));
            j = v1 + 1;
        }
        i += j;
        if closed {
            match stack.last_mut() {
                Some(parent) => parent.children.push(el),
                None => {
                    root = Some(el);
                    break;
                }
            }
        } else {
            if stack.len() >= MAX_DEPTH {
                return Err(bad("nested too deeply"));
            }
            stack.push(el);
        }
    }
    root.ok_or_else(|| {
        if stack.is_empty() {
            bad("no root element")
        } else {
            Error::Truncated("XML".into())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_elements_attributes_and_text() {
        let src = br#"<?xml version="1.0"?><!DOCTYPE GIFTI SYSTEM "x.dtd" [ <!ENTITY a "b"> ]>
<g:A x="1" y='a&amp;b'><!-- c --><B>one &lt; two</B><C><![CDATA[<raw>]]></C><D/></g:A>"#;
        let r = parse(src).unwrap();
        assert_eq!(r.name, "A");
        assert_eq!(r.attr("y"), Some("a&b"));
        assert_eq!(r.child_text("B", src).as_deref(), Some("one < two"));
        assert_eq!(r.child_text("C", src).as_deref(), Some("<raw>"));
        assert!(r.child("D").is_some());
    }

    #[test]
    fn refuses_broken_documents() {
        assert!(parse(b"<A><B></A>").is_err());
        assert!(parse(b"<A>").is_err());
        assert!(parse(b"").is_err());
        assert!(parse(b"<A x=1/>").is_err());
        let deep = "<a>".repeat(200);
        assert!(parse(deep.as_bytes()).is_err());
    }
}
