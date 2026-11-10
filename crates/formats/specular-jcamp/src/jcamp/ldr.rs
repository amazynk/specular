//! The text of a JCAMP-DX file as labelled data records (`##LABEL= value`) grouped into
//! blocks (`##TITLE=` … `##END=`), nested for LINK files.

use std::borrow::Cow;
use std::ops::Range;

/// The file as text: UTF-8 (a BOM dropped), UTF-16 with a BOM, else Latin-1.
pub(crate) fn decode(bytes: &[u8]) -> Cow<'_, str> {
    let b = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    if let Some(rest) = b.strip_prefix(b"\xFF\xFE") {
        let u: Vec<u16> = rest
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return Cow::Owned(String::from_utf16_lossy(&u));
    }
    if let Some(rest) = b.strip_prefix(b"\xFE\xFF") {
        let u: Vec<u16> = rest
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return Cow::Owned(String::from_utf16_lossy(&u));
    }
    match std::str::from_utf8(b) {
        Ok(s) => Cow::Borrowed(s),
        Err(_) => Cow::Owned(b.iter().map(|&c| char::from(c)).collect()),
    }
}

/// Lines ended by LF, CR LF or a lone CR (old Macintosh files).
pub(crate) fn lines(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let b = text.as_bytes();
    let mut start = 0;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\n' => {
                out.push(&text[start..i]);
                start = i + 1;
            }
            b'\r' => {
                out.push(&text[start..i]);
                if b.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if start < b.len() {
        out.push(&text[start..]);
    }
    out
}

/// A label as compared: upper case without spaces, dashes, slashes and underscores
/// (`##DATA TYPE`, `##DATATYPE` and `##data_type` are one label).
pub(crate) fn normalise(label: &str) -> String {
    label
        .chars()
        .filter(|c| !matches!(c, ' ' | '\t' | '-' | '/' | '_'))
        .flat_map(char::to_uppercase)
        .collect()
}

/// One labelled data record.
#[derive(Clone, Debug)]
pub(crate) struct Ldr<'a> {
    /// The label as compared ([`normalise`]) and as written.
    pub key: String,
    pub raw: &'a str,
    /// What follows `=` on the label's own line.
    pub first: &'a str,
    /// The lines after it, up to the next record.
    pub more: Range<usize>,
}

/// The records of the text, in order.
pub(crate) fn records<'a>(lines: &[&'a str]) -> Vec<Ldr<'a>> {
    let mut out: Vec<Ldr<'a>> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim_start();
        let Some(rest) = t.strip_prefix("##") else {
            continue;
        };
        if let Some(last) = out.last_mut() {
            last.more.end = i;
        }
        let (raw, first) = match rest.find('=') {
            Some(e) => (&rest[..e], &rest[e + 1..]),
            None => (rest, ""),
        };
        out.push(Ldr {
            key: normalise(raw),
            raw: raw.trim(),
            first,
            more: i + 1..i + 1,
        });
    }
    if let Some(last) = out.last_mut() {
        last.more.end = lines.len();
    }
    out
}

impl<'a> Ldr<'a> {
    /// The value as one line: comments removed, lines joined by a space, trimmed.
    pub fn value(&self, lines: &[&'a str]) -> String {
        let mut s = strip(self.first).trim().to_string();
        for l in &lines[self.more.clone()] {
            let l = strip(l).trim();
            if l.is_empty() {
                continue;
            }
            if !s.is_empty() {
                s.push(' ');
            }
            s.push_str(l);
        }
        s
    }

    /// The value's own line only, without its comment.
    pub fn head(&self) -> &'a str {
        strip(self.first).trim()
    }
}

fn strip(l: &str) -> &str {
    match l.find("$$") {
        Some(i) => &l[..i],
        None => l,
    }
}

/// A block: its records (children's excluded) and the blocks nested in it.
#[derive(Debug, Default)]
pub(crate) struct Block<'a> {
    pub ldrs: Vec<Ldr<'a>>,
    pub children: Vec<Block<'a>>,
    /// It ended with `##END=` (else the file stopped first).
    pub closed: bool,
}

impl<'a> Block<'a> {
    pub fn get(&self, key: &str) -> Option<&Ldr<'a>> {
        self.ldrs.iter().find(|l| l.key == key)
    }
}

/// The blocks of the file: each `##TITLE=` opens one (inside the open one, as LINK files
/// nest them), `##END=` closes it. Records before the first title are dropped.
pub(crate) fn blocks<'a>(ldrs: Vec<Ldr<'a>>) -> Vec<Block<'a>> {
    let mut roots: Vec<Block<'a>> = Vec::new();
    let mut stack: Vec<Block<'a>> = Vec::new();
    for l in ldrs {
        match l.key.as_str() {
            "TITLE" => {
                let mut b = Block::default();
                b.ldrs.push(l);
                stack.push(b);
            }
            "END" => {
                if let Some(mut b) = stack.pop() {
                    b.closed = true;
                    match stack.last_mut() {
                        Some(p) => p.children.push(b),
                        None => roots.push(b),
                    }
                }
            }
            _ => {
                if let Some(b) = stack.last_mut() {
                    b.ldrs.push(l);
                }
            }
        }
    }
    while let Some(b) = stack.pop() {
        match stack.last_mut() {
            Some(p) => p.children.push(b),
            None => roots.push(b),
        }
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_blocks() {
        let t = "##TITLE= a\r##DATA TYPE= LINK $$ c\r\n##TITLE=b\n##XYDATA=(X++(Y..Y))\n1 2 3\n4 5\n##END=\n##END=";
        let ls = lines(t);
        let r = records(&ls);
        assert_eq!(r[1].key, "DATATYPE");
        assert_eq!(r[1].head(), "LINK");
        assert_eq!(r[3].more, 4..6);
        let b = blocks(r);
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].children.len(), 1);
        assert!(b[0].children[0].get("XYDATA").is_some());
        assert_eq!(
            normalise(" Spectrometer/Data_System"),
            "SPECTROMETERDATASYSTEM"
        );
        assert_eq!(decode(b"\xFF\xFEA\0"), "A");
        assert_eq!(decode(b"\xB5m"), "\u{b5}m");
    }
}
