//! A small, tolerant XML reader for the metadata blobs of TIA (.emi `ObjectInfo`) and Bruker
//! BCF headers: elements, attributes and text into an arena, entities decoded. Comments,
//! processing instructions and DOCTYPE are skipped, CDATA kept as text. No recursion (the
//! nesting depth is bounded), so hostile input cannot overflow the stack; malformed input
//! yields what was read so far.

/// Windows-1252 code points for 0x80..0xA0 (Bruker writes WINDOWS-1252 headers).
const CP1252: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8D}', 'Ž', '\u{8F}',
    '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9D}', 'ž', 'Ÿ',
];

/// The document as text: UTF-8 when it is valid UTF-8, otherwise Windows-1252.
fn text(src: &[u8]) -> std::borrow::Cow<'_, str> {
    match std::str::from_utf8(src) {
        Ok(s) => s.into(),
        Err(_) => src
            .iter()
            .map(|&c| match c {
                0x80..0xA0 => CP1252[usize::from(c - 0x80)],
                _ => char::from(c),
            })
            .collect::<String>()
            .into(),
    }
}

/// Deepest nesting kept; deeper elements are flattened into their ancestor at this depth.
const MAX_DEPTH: usize = 256;

#[derive(Clone, Debug, Default)]
pub(crate) struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub text: String,
    pub children: Vec<usize>,
}

/// A parsed document: element 0 is a synthetic root holding the top-level elements.
#[derive(Clone, Debug)]
pub(crate) struct Doc {
    pub nodes: Vec<Element>,
}

impl Doc {
    pub fn parse(src: &[u8]) -> Doc {
        let s = text(src);
        let b = s.as_bytes();
        let mut nodes = vec![Element::default()];
        let mut stack = vec![0usize];
        let mut i = 0;
        while i < b.len() {
            if b[i] != b'<' {
                let end = memchr(b'<', &b[i..]).map_or(b.len(), |k| i + k);
                let top = *stack.last().unwrap_or(&0);
                if top != 0 {
                    decode_into(&s[i..end], &mut nodes[top].text);
                }
                i = end;
                continue;
            }
            let rest = &b[i..];
            if rest.starts_with(b"<!--") {
                i = find(b, i + 4, b"-->").map_or(b.len(), |k| k + 3);
            } else if rest.starts_with(b"<![CDATA[") {
                let end = find(b, i + 9, b"]]>").unwrap_or(b.len());
                let top = *stack.last().unwrap_or(&0);
                if top != 0 {
                    nodes[top].text.push_str(&s[i + 9..end]);
                }
                i = (end + 3).min(b.len());
            } else if rest.starts_with(b"<?") {
                i = find(b, i + 2, b"?>").map_or(b.len(), |k| k + 2);
            } else if rest.starts_with(b"<!") {
                i = memchr(b'>', &b[i..]).map_or(b.len(), |k| i + k + 1);
            } else if rest.starts_with(b"</") {
                let end = memchr(b'>', &b[i..]).map_or(b.len(), |k| i + k);
                let name = s[i + 2..end].trim();
                // Pop to the matching element (tolerates unclosed children).
                if let Some(pos) = stack.iter().rposition(|&n| n != 0 && nodes[n].name == name) {
                    stack.truncate(pos);
                }
                i = (end + 1).min(b.len());
            } else {
                let Some((el, self_closing, next)) = tag(&s, i) else {
                    break;
                };
                i = next;
                let idx = nodes.len();
                let parent = *stack.last().unwrap_or(&0);
                nodes.push(el);
                nodes[parent].children.push(idx);
                if !self_closing && stack.len() <= MAX_DEPTH {
                    stack.push(idx);
                }
            }
        }
        Doc { nodes }
    }

    pub fn root(&self) -> usize {
        0
    }

    pub fn el(&self, n: usize) -> &Element {
        &self.nodes[n]
    }

    /// Child elements of `n` named `name`.
    pub fn children<'a>(&'a self, n: usize, name: &'a str) -> impl Iterator<Item = usize> + 'a {
        self.nodes[n]
            .children
            .iter()
            .copied()
            .filter(move |&c| self.nodes[c].name == name)
    }

    pub fn child(&self, n: usize, name: &str) -> Option<usize> {
        self.children(n, name).next()
    }

    /// Follows a `/`-separated path of child names.
    pub fn path(&self, mut n: usize, path: &str) -> Option<usize> {
        for part in path.split('/').filter(|p| !p.is_empty()) {
            n = self.child(n, part)?;
        }
        Some(n)
    }

    /// The trimmed text of the child at `path`.
    pub fn text(&self, n: usize, path: &str) -> Option<&str> {
        self.path(n, path).map(|c| self.nodes[c].text.trim())
    }

    pub fn attr(&self, n: usize, name: &str) -> Option<&str> {
        self.nodes[n]
            .attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// Descendants (depth first, document order) named `name`.
    pub fn descendants<'a>(&'a self, n: usize, name: &'a str) -> Vec<usize> {
        let mut out = Vec::new();
        let mut todo: Vec<usize> = self.nodes[n].children.iter().rev().copied().collect();
        while let Some(c) = todo.pop() {
            if self.nodes[c].name == name {
                out.push(c);
            }
            todo.extend(self.nodes[c].children.iter().rev());
        }
        out
    }
}

fn memchr(c: u8, b: &[u8]) -> Option<usize> {
    b.iter().position(|&x| x == c)
}

fn find(b: &[u8], from: usize, pat: &[u8]) -> Option<usize> {
    b.get(from..)?
        .windows(pat.len())
        .position(|w| w == pat)
        .map(|k| from + k)
}

/// Parses a start tag at `i` (`<name attr="v" ...>` or `.../>`).
fn tag(s: &str, i: usize) -> Option<(Element, bool, usize)> {
    let b = s.as_bytes();
    let mut j = i + 1;
    let start = j;
    while j < b.len() && !b[j].is_ascii_whitespace() && b[j] != b'>' && b[j] != b'/' {
        j += 1;
    }
    let mut el = Element {
        name: s[start..j].to_string(),
        ..Element::default()
    };
    loop {
        while j < b.len() && b[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= b.len() {
            return None;
        }
        match b[j] {
            b'>' => return Some((el, false, j + 1)),
            b'/' => {
                let end = memchr(b'>', &b[j..]).map_or(b.len(), |k| j + k + 1);
                return Some((el, true, end));
            }
            _ => {}
        }
        let ks = j;
        while j < b.len() && b[j] != b'=' && b[j] != b'>' && !b[j].is_ascii_whitespace() {
            j += 1;
        }
        let key = s[ks..j].to_string();
        while j < b.len() && b[j].is_ascii_whitespace() {
            j += 1;
        }
        let mut val = String::new();
        if j < b.len() && b[j] == b'=' {
            j += 1;
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < b.len() && (b[j] == b'"' || b[j] == b'\'') {
                let q = b[j];
                let vs = j + 1;
                let ve = memchr(q, &b[vs..]).map_or(b.len(), |k| vs + k);
                decode_into(&s[vs..ve], &mut val);
                j = (ve + 1).min(b.len());
            } else {
                let vs = j;
                while j < b.len() && !b[j].is_ascii_whitespace() && b[j] != b'>' {
                    j += 1;
                }
                decode_into(&s[vs..j], &mut val);
            }
        } else if j == ks {
            // Not progressing (stray character): skip it.
            j += 1;
            continue;
        }
        el.attrs.push((key, val));
    }
}

/// Appends `src` to `out` with entity references decoded.
pub(crate) fn decode_into(src: &str, out: &mut String) {
    let mut rest = src;
    while let Some(k) = rest.find('&') {
        out.push_str(&rest[..k]);
        rest = &rest[k..];
        let Some(end) = rest[..rest.len().min(12)].find(';') else {
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
                .map(|h| u32::from_str_radix(h, 16).ok())
                .unwrap_or_else(|| ent.strip_prefix('#').and_then(|d| d.parse().ok()))
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
}

/// A number written by MSXML on a locale with decimal commas ("0,9664") or plainly.
pub(crate) fn number(t: &str) -> Option<f64> {
    let t = t.trim();
    t.parse()
        .ok()
        .or_else(|| t.replace(',', ".").parse().ok())
        .filter(|v: &f64| v.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_and_entities() {
        let d = Doc::parse(
            b"<?xml version=\"1.0\"?><!-- c --><a x='1'><b>t &lt;1&gt; &#65;</b><c/>\
              <b k=v>2</b><![CDATA[<raw>]]></a>",
        );
        let a = d.child(d.root(), "a").unwrap();
        assert_eq!(d.attr(a, "x"), Some("1"));
        let bs: Vec<_> = d.children(a, "b").collect();
        assert_eq!(bs.len(), 2);
        assert_eq!(d.el(bs[0]).text, "t <1> A");
        assert_eq!(d.attr(bs[1], "k"), Some("v"));
        assert_eq!(d.el(a).text, "<raw>");
        assert_eq!(d.text(a, "b"), Some("t <1> A"));
        assert!(d.path(a, "c").is_some());
        assert_eq!(d.descendants(d.root(), "b").len(), 2);
    }

    #[test]
    fn tolerates_garbage() {
        for s in [
            &b"<a><b></a>"[..],
            b"<",
            b"<a x=",
            b"</>",
            b"<a>&bogus;&#xFFFFFFFF;</a>",
        ] {
            let _ = Doc::parse(s);
        }
        assert_eq!(number("0,5"), Some(0.5));
        assert_eq!(number("2E4"), Some(20000.0));
    }

    #[test]
    fn windows_1252_when_not_utf8() {
        let d = Doc::parse(b"<a>SE \xB5m \x80</a><b>\xC2\xB5</b>");
        assert_eq!(d.text(d.root(), "a"), Some("SE \u{b5}m \u{20ac}"));
        assert_eq!(
            Doc::parse("<b>\u{b5}</b>".as_bytes()).text(0, "b"),
            Some("\u{b5}")
        );
    }
}
