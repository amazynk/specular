//! A flat, non-recursive XML scanner for the small property parts (`docProps/*.xml`,
//! `_rels/.rels`, `[Content_Types].xml`, `meta.xml`): every start tag with its attributes and
//! the text directly inside it, by local name. Nesting depth never costs stack; malformed
//! markup ends the scan with what was read.

/// One element: its local name, attributes (local name, value) and its own text.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub text: String,
}

impl Element {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Elements kept at most from one part.
const MAX_ELEMENTS: usize = 100_000;

fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// Decodes the five named entities and character references.
pub(crate) fn unescape(s: &str) -> String {
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
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => {
                let n = if let Some(h) = ent.strip_prefix("#x").or_else(|| ent.strip_prefix("#X")) {
                    u32::from_str_radix(h, 16).ok()
                } else {
                    ent.strip_prefix('#').and_then(|d| d.parse().ok())
                };
                n.and_then(char::from_u32)
            }
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

fn attributes(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = s;
    loop {
        rest = rest.trim_start();
        let Some(eq) = rest.find('=') else { break };
        let key = rest[..eq].trim();
        rest = rest[eq + 1..].trim_start();
        let Some(q) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            break;
        };
        let Some(end) = rest[1..].find(q) else { break };
        out.push((local(key).to_owned(), unescape(&rest[1..1 + end])));
        rest = &rest[end + 2..];
        if out.len() > 64 {
            break;
        }
    }
    out
}

/// Every element of `data` in document order.
pub(crate) fn elements(data: &[u8]) -> Vec<Element> {
    let s = String::from_utf8_lossy(data);
    let s = s.strip_prefix('\u{feff}').unwrap_or(&s);
    let mut out: Vec<Element> = Vec::new();
    // Index of the element whose text is being read (until the next tag).
    let mut open: Option<usize> = None;
    let mut rest = s;
    while let Some(lt) = rest.find('<') {
        if let Some(i) = open {
            let text = &rest[..lt];
            if !text.trim().is_empty() {
                out[i].text.push_str(&unescape(text));
            }
        }
        rest = &rest[lt..];
        if let Some(r) = rest.strip_prefix("<![CDATA[") {
            let end = r.find("]]>").unwrap_or(r.len());
            if let Some(i) = open {
                out[i].text.push_str(&r[..end]);
            }
            rest = &r[(end + 3).min(r.len())..];
            continue;
        }
        if rest.starts_with("<!--") {
            let end = rest.find("-->").map_or(rest.len(), |e| e + 3);
            rest = &rest[end..];
            continue;
        }
        let Some(gt) = rest.find('>') else { break };
        let tag = &rest[1..gt];
        rest = &rest[gt + 1..];
        if tag.starts_with('?') || tag.starts_with('!') {
            continue;
        }
        if tag.starts_with('/') {
            open = None;
            continue;
        }
        let empty = tag.ends_with('/');
        let tag = tag.trim_end_matches('/');
        let (name, attrs) = match tag.find(|c: char| c.is_ascii_whitespace()) {
            Some(i) => (&tag[..i], attributes(&tag[i..])),
            None => (tag, Vec::new()),
        };
        if out.len() >= MAX_ELEMENTS {
            break;
        }
        out.push(Element {
            name: local(name).to_owned(),
            attrs,
            text: String::new(),
        });
        open = (!empty).then_some(out.len() - 1);
    }
    out
}

/// The trimmed text of the first element called `name`, if not empty.
pub(crate) fn text(els: &[Element], name: &str) -> Option<String> {
    els.iter()
        .find(|e| e.name == name)
        .map(|e| e.text.trim().to_owned())
        .filter(|t| !t.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_names_attributes_and_text() {
        let x = br#"<?xml version="1.0"?><cp:coreProperties xmlns:cp="u"><dc:title>A &amp; B &#x263A;</dc:title>
            <!-- c --><meta:document-statistic meta:page-count='3' meta:word-count="10"/>
            <x><![CDATA[<raw>]]></x></cp:coreProperties>"#;
        let e = elements(x);
        assert_eq!(text(&e, "title").as_deref(), Some("A & B \u{263A}"));
        let s = e.iter().find(|e| e.name == "document-statistic").unwrap();
        assert_eq!(s.attr("page-count"), Some("3"));
        assert_eq!(s.attr("word-count"), Some("10"));
        assert_eq!(text(&e, "x").as_deref(), Some("<raw>"));
    }

    #[test]
    fn deep_and_broken_markup_does_not_panic() {
        let deep = "<a>".repeat(200_000);
        assert_eq!(elements(deep.as_bytes()).len(), MAX_ELEMENTS);
        for s in [
            "<",
            "<a",
            "<a b=",
            "<a b=\"",
            "&#xFFFFFFFF;",
            "<a>&am",
            "<![CDATA[x",
        ] {
            let _ = elements(s.as_bytes());
            let _ = unescape(s);
        }
    }
}
