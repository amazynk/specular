//! `@font-face` rules of an SVG's style sheets: fonts given as `data:` URLs or as files in the
//! SVG's own folder (TTF, OTF, WOFF, WOFF2, read by `specular-font`), loaded into a copy of
//! the font database and chosen by their `font-family` name before usvg's own selection
//! (which ignores `@font-face`). Other sources (remote URLs, other folders) are left alone.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use resvg::usvg;
use resvg::usvg::fontdb;

/// At most this many faces, and this many bytes of font data, per document.
const MAX_FACES: usize = 32;
const MAX_BYTES: usize = 64 << 20;

/// (family name, font bytes) of each usable `@font-face` rule.
pub(crate) fn faces(xml: &[u8], dir: Option<&Path>) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut total = 0usize;
    for at in memchr::memmem::find_iter(xml, b"@font-face") {
        if out.len() >= MAX_FACES {
            break;
        }
        let rest = &xml[at..];
        let (Some(open), Some(close)) = (memchr::memchr(b'{', rest), memchr::memchr(b'}', rest))
        else {
            continue;
        };
        if close < open {
            continue;
        }
        let block = String::from_utf8_lossy(&rest[open + 1..close]);
        let Some(family) = property(&block, "font-family") else {
            continue;
        };
        let family = family.trim().trim_matches(['"', '\'']).trim().to_owned();
        let Some(src) = property(&block, "src") else {
            continue;
        };
        for url in urls(&src) {
            if let Some(bytes) = fetch(&url, dir, MAX_BYTES - total)
                && let Some(sfnt) = sfnt(&bytes)
            {
                total += sfnt.len();
                out.push((family.clone(), sfnt));
                break;
            }
        }
    }
    out
}

/// The value of `name: value;` in a declaration block (the last one wins, as in CSS).
fn property(block: &str, name: &str) -> Option<String> {
    let mut found = None;
    for decl in split_decls(block) {
        if let Some((k, v)) = decl.split_once(':')
            && k.trim().eq_ignore_ascii_case(name)
        {
            found = Some(v.trim().to_owned());
        }
    }
    found
}

/// Declarations split at `;` outside quotes and parentheses (data URLs hold `;`).
fn split_decls(block: &str) -> Vec<&str> {
    let (mut out, mut start, mut depth, mut quote) = (Vec::new(), 0, 0i32, None);
    for (i, c) in block.char_indices() {
        match (quote, c) {
            (Some(q), _) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '(') => depth += 1,
            (None, ')') => depth -= 1,
            (None, ';') if depth <= 0 => {
                out.push(&block[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&block[start..]);
    out
}

/// The `url(...)` arguments of a `src` value, in order.
fn urls(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = src;
    while let Some(i) = rest.find("url(") {
        let s = &rest[i + 4..];
        let Some(end) = s.find(')') else { break };
        out.push(s[..end].trim().trim_matches(['"', '\'']).to_owned());
        rest = &s[end + 1..];
    }
    out
}

fn fetch(url: &str, dir: Option<&Path>, budget: usize) -> Option<Vec<u8>> {
    if let Some(data) = url.strip_prefix("data:") {
        let (head, body) = data.split_once(',')?;
        if !head.ends_with(";base64") {
            return None;
        }
        let v = base64(body)?;
        return (v.len() <= budget).then_some(v);
    }
    // A file next to the SVG: a bare name, no scheme, no other folder.
    let name = url.split(['?', '#']).next()?;
    if name.is_empty() || name.contains(['/', '\\', ':']) || name.starts_with('.') {
        return None;
    }
    let p = dir?.join(name);
    if std::fs::metadata(&p).ok()?.len() as usize > budget {
        return None;
    }
    std::fs::read(p).ok()
}

fn base64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let (mut acc, mut n) = (0u32, 0u32);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            c if c.is_ascii_whitespace() => continue,
            b'%' => return None,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        n += 6;
        if n >= 8 {
            n -= 8;
            out.push((acc >> n) as u8);
        }
    }
    Some(out)
}

/// TTF/OTF bytes, unwrapping WOFF and WOFF2.
fn sfnt(bytes: &[u8]) -> Option<Vec<u8>> {
    let f = specular_font::open(
        bytes,
        specular_font::Limits {
            max_sfnt_bytes: MAX_BYTES,
            max_table_bytes: MAX_BYTES,
        },
    )
    .ok()?;
    let face = f.faces.first()?;
    Some(face.sfnt().as_ref().clone())
}

/// A font database with the document's faces added, and the face to use per family name
/// (lower case).
pub(crate) fn database(
    base: &Arc<fontdb::Database>,
    faces: Vec<(String, Vec<u8>)>,
) -> (Arc<fontdb::Database>, HashMap<String, fontdb::ID>) {
    let mut db = (**base).clone();
    let mut names = HashMap::new();
    for (family, data) in faces {
        let ids = db.load_font_source(fontdb::Source::Binary(Arc::new(data)));
        if let Some(&id) = ids.first() {
            names.entry(family.to_lowercase()).or_insert(id);
        }
    }
    (Arc::new(db), names)
}

/// usvg's font selection, trying the document's `@font-face` families first.
pub(crate) fn selector(names: HashMap<String, fontdb::ID>) -> usvg::FontSelectionFn<'static> {
    let default = usvg::FontResolver::default_font_selector();
    Box::new(move |font, db| {
        for f in font.families() {
            if let usvg::FontFamily::Named(n) = f
                && let Some(&id) = names.get(&n.to_lowercase())
            {
                return Some(id);
            }
        }
        default(font, db)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rules() {
        let css = br#"<style>@font-face { font-family: "My Face"; src: local(x), url('data:font/ttf;base64,AAEC') format("truetype"); } @font-face{font-family:Other;src:url(../up.ttf)}</style>"#;
        let mut found = Vec::new();
        for at in memchr::memmem::find_iter(css, b"@font-face") {
            let rest = String::from_utf8_lossy(&css[at..]).to_string();
            let open = rest.find('{').unwrap();
            let close = rest.find('}').unwrap();
            let block = &rest[open + 1..close];
            found.push((
                property(block, "font-family"),
                urls(&property(block, "src").unwrap()),
            ));
        }
        assert_eq!(found[0].0.as_deref(), Some("\"My Face\""));
        assert_eq!(found[0].1, vec!["data:font/ttf;base64,AAEC".to_owned()]);
        assert_eq!(found[1].1, vec!["../up.ttf".to_owned()]);
        assert_eq!(base64("AAEC").unwrap(), vec![0, 1, 2]);
        // Not fonts, or not reachable: nothing.
        assert!(faces(css, None).is_empty());
        assert!(fetch("../up.ttf", Some(Path::new("/")), 1 << 20).is_none());
        assert!(fetch("https://x/y.ttf", Some(Path::new("/")), 1 << 20).is_none());
    }
}
