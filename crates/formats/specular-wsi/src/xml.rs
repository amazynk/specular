//! A small, bounded XML reader for the descriptions slides carry (OME-XML, Leica SCN): elements
//! with their attributes and text, namespace prefixes dropped. Comments, processing
//! instructions, DOCTYPE and CDATA are understood; DTDs and external entities are not.

use crate::{Error, Result};

/// Deepest nesting accepted.
const MAX_DEPTH: usize = 256;
/// Most elements accepted in one document.
const MAX_ELEMENTS: usize = 1 << 20;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Element {
    /// Local name (no namespace prefix).
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Element>,
    /// Character data directly inside the element, concatenated and trimmed.
    pub text: String,
}

impl Element {
    /// An attribute's value by local name.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// An attribute parsed as a number.
    pub fn num(&self, name: &str) -> Option<f64> {
        self.attr(name)?
            .trim()
            .parse()
            .ok()
            .filter(|v: &f64| v.is_finite())
    }

    /// The first child element named `name`.
    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.name == name)
    }

    /// The child elements named `name`.
    pub fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// The first descendant named `name`, depth first.
    pub fn find(&self, name: &str) -> Option<&Element> {
        for c in &self.children {
            if c.name == name {
                return Some(c);
            }
            if let Some(f) = c.find(name) {
                return Some(f);
            }
        }
        None
    }

    /// The text of the first child named `name`.
    pub fn child_text(&self, name: &str) -> Option<&str> {
        self.child(name)
            .map(|c| c.text.as_str())
            .filter(|t| !t.is_empty())
    }
}

fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// Replace the predefined and numeric character references.
fn unescape(s: &str) -> String {
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

/// Parse a document and return its root element.
pub fn parse(src: &str) -> Result<Element> {
    let bad = |m: &str| Error::Malformed(format!("XML: {m}"));
    let b = src.as_bytes();
    let mut i = 0;
    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;
    let mut count = 0usize;
    while i < b.len() {
        if b[i] != b'<' {
            let end = memchr(b'<', &b[i..]).map_or(b.len(), |e| i + e);
            if let Some(top) = stack.last_mut() {
                let t = src[i..end].trim();
                if !t.is_empty() {
                    if !top.text.is_empty() {
                        top.text.push(' ');
                    }
                    top.text.push_str(&unescape(t));
                }
            }
            i = end;
            continue;
        }
        let rest = &src[i..];
        if rest.starts_with("<!--") {
            i += rest
                .find("-->")
                .ok_or_else(|| bad("unterminated comment"))?
                + 3;
            continue;
        }
        if rest.starts_with("<![CDATA[") {
            let end = rest.find("]]>").ok_or_else(|| bad("unterminated CDATA"))?;
            if let Some(top) = stack.last_mut() {
                top.text.push_str(rest[9..end].trim());
            }
            i += end + 3;
            continue;
        }
        if rest.starts_with("<?") {
            i += rest
                .find("?>")
                .ok_or_else(|| bad("unterminated declaration"))?
                + 2;
            continue;
        }
        if rest.starts_with("<!") {
            // DOCTYPE (an internal subset in brackets is skipped whole).
            let mut depth = 0i32;
            let mut j = i;
            while j < b.len() {
                match b[j] {
                    b'[' => depth += 1,
                    b']' => depth -= 1,
                    b'>' if depth <= 0 => break,
                    _ => {}
                }
                j += 1;
            }
            i = j + 1;
            continue;
        }
        if let Some(close) = rest.strip_prefix("</") {
            let end = close.find('>').ok_or_else(|| bad("unterminated end tag"))?;
            let name = local(close[..end].trim());
            let el = stack.pop().ok_or_else(|| bad("unbalanced end tag"))?;
            if el.name != name {
                return Err(bad("mismatched end tag"));
            }
            match stack.last_mut() {
                Some(p) => p.children.push(el),
                None => {
                    root.get_or_insert(el);
                }
            }
            i += 2 + end + 1;
            continue;
        }
        // Start tag: name, attributes, optional self-closing slash.
        let mut j = i + 1;
        while j < b.len() && !b[j].is_ascii_whitespace() && b[j] != b'>' && b[j] != b'/' {
            j += 1;
        }
        let name = local(&src[i + 1..j]).to_owned();
        if name.is_empty() {
            return Err(bad("empty tag name"));
        }
        let mut el = Element {
            name,
            ..Default::default()
        };
        let mut closed = false;
        loop {
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            match b.get(j) {
                None => return Err(bad("unterminated start tag")),
                Some(b'>') => {
                    j += 1;
                    break;
                }
                Some(b'/') => {
                    if b.get(j + 1) != Some(&b'>') {
                        return Err(bad("stray slash"));
                    }
                    closed = true;
                    j += 2;
                    break;
                }
                _ => {}
            }
            let k0 = j;
            while j < b.len() && b[j] != b'=' && !b[j].is_ascii_whitespace() && b[j] != b'>' {
                j += 1;
            }
            let key = local(&src[k0..j]).to_owned();
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            if b.get(j) != Some(&b'=') {
                return Err(bad("attribute without value"));
            }
            j += 1;
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            let q = *b.get(j).ok_or_else(|| bad("unterminated attribute"))?;
            if q != b'"' && q != b'\'' {
                return Err(bad("unquoted attribute"));
            }
            let v0 = j + 1;
            let v1 = memchr(q, &b[v0..])
                .map(|e| v0 + e)
                .ok_or_else(|| bad("unterminated attribute"))?;
            el.attrs.push((key, unescape(&src[v0..v1])));
            j = v1 + 1;
        }
        i = j;
        count += 1;
        if count > MAX_ELEMENTS {
            return Err(Error::TooLarge);
        }
        if closed {
            match stack.last_mut() {
                Some(p) => p.children.push(el),
                None => {
                    root.get_or_insert(el);
                }
            }
        } else {
            if stack.len() >= MAX_DEPTH {
                return Err(bad("too deep"));
            }
            stack.push(el);
        }
    }
    if !stack.is_empty() {
        return Err(bad("unclosed element"));
    }
    root.ok_or_else(|| bad("no root element"))
}

fn memchr(c: u8, b: &[u8]) -> Option<usize> {
    b.iter().position(|&x| x == c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_elements_attributes_and_text() {
        let x = r#"<?xml version="1.0"?><!-- c --><ome:OME xmlns:ome="u"><Image ID='Image:0' Name="a &amp; b"><Pixels SizeX="10"/><Desc>hi &lt;there&gt; &#65;</Desc></Image></ome:OME>"#;
        let r = parse(x).unwrap();
        assert_eq!(r.name, "OME");
        let im = r.child("Image").unwrap();
        assert_eq!(im.attr("Name"), Some("a & b"));
        assert_eq!(im.child("Pixels").unwrap().num("SizeX"), Some(10.0));
        assert_eq!(im.child_text("Desc"), Some("hi <there> A"));
        assert!(r.find("Pixels").is_some());
    }

    #[test]
    fn rejects_broken_documents() {
        for x in ["<a><b></a>", "<a", "<a b=c/>", "</a>", "", "<a><b>"] {
            assert!(parse(x).is_err(), "{x}");
        }
        let deep = "<a>".repeat(1000) + &"</a>".repeat(1000);
        assert!(parse(&deep).is_err());
    }
}
