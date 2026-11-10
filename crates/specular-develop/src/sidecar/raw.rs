//! Raw slices of an XMP text, for writing back what this build does not own byte for byte:
//! other tools' attributes and elements, and a newer Specular's fields and modules.

use roxmltree::Node;

/// The start tag of element `n` (`<name …>` or `<name …/>`), as written.
pub(super) fn start_tag<'a>(text: &'a str, n: Node) -> &'a str {
    let r = n.range();
    let s = &text[r.clone()];
    let mut quote = None;
    for (i, c) in s.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, '>') => return &s[..=i],
            _ => {}
        }
    }
    s
}

/// The namespace declarations of a start tag, each as written (`xmlns:dc="…"`).
pub(super) fn xmlns_decls(tag: &str) -> Vec<&str> {
    let b = tag.as_bytes();
    let mut out = Vec::new();
    // Past `<name`.
    let mut i = tag
        .find(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
        .unwrap_or(tag.len());
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || b[i] == b'>' || b[i] == b'/' {
            break;
        }
        let start = i;
        while i < b.len() && b[i] != b'=' && !b[i].is_ascii_whitespace() {
            i += 1;
        }
        let name = &tag[start..i];
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'=') {
            i += 1;
        }
        let Some(&q) = b.get(i) else { break };
        if q != b'"' && q != b'\'' {
            break;
        }
        let Some(len) = tag[i + 1..].find(q as char) else {
            break;
        };
        i += len + 2;
        if name == "xmlns" || name.starts_with("xmlns:") {
            out.push(&tag[start..i]);
        }
    }
    out
}

/// Element children of `n`.
pub(super) fn elements<'a, 'i>(n: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> {
    n.children().filter(Node::is_element)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_tags_and_their_namespaces() {
        let t = r#"<a:x xmlns:a="u" b='q>' xmlns="d" a:c="1"><y/></a:x>"#;
        let doc = roxmltree::Document::parse(t).unwrap();
        let tag = start_tag(t, doc.root_element());
        assert_eq!(tag, r#"<a:x xmlns:a="u" b='q>' xmlns="d" a:c="1">"#);
        assert_eq!(xmlns_decls(tag), vec![r#"xmlns:a="u""#, r#"xmlns="d""#]);
        let y = doc.root_element().first_child().unwrap();
        assert_eq!(start_tag(t, y), "<y/>");
        assert!(xmlns_decls("<y/>").is_empty());
    }
}
