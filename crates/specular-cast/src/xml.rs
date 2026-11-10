//! XML from devices on the network: escaping what we send, and a guarded parse of what comes
//! back (size and nesting capped before roxmltree, which recurses per element).

use crate::{Error, Result};

/// The largest description or SOAP reply read.
pub(crate) const MAX_BYTES: usize = 512 * 1024;
/// The deepest nesting accepted (descriptions are a dozen levels deep).
const MAX_DEPTH: usize = 64;

/// `s` escaped for XML text and attribute values.
pub(crate) fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            // Control characters are not allowed in XML 1.0 (tab, newline and return are).
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => o.push(c),
        }
    }
    o
}

/// The nesting of `text`, roughly (tags opened minus closed), stopping past `MAX_DEPTH`.
fn too_deep(text: &str) -> bool {
    let b = text.as_bytes();
    let mut depth = 0usize;
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'<' {
            match b.get(i + 1) {
                Some(b'/') => depth = depth.saturating_sub(1),
                Some(b'?' | b'!') => {}
                Some(_) => {
                    // An empty element (`<a/>`) opens and closes.
                    let end = b[i..].iter().position(|&c| c == b'>').map(|p| i + p);
                    if !end.is_some_and(|e| e > 0 && b[e - 1] == b'/') {
                        depth += 1;
                        if depth > MAX_DEPTH {
                            return true;
                        }
                    }
                }
                None => {}
            }
        }
        i += 1;
    }
    false
}

/// Parses a device's XML, refusing what is too large or too deep.
pub(crate) fn parse(text: &str) -> Result<roxmltree::Document<'_>> {
    if text.len() > MAX_BYTES {
        return Err(Error::BadReply("too large"));
    }
    if too_deep(text) {
        return Err(Error::BadReply("nested too deep"));
    }
    let opt = roxmltree::ParsingOptions {
        allow_dtd: false,
        nodes_limit: 50_000,
        ..roxmltree::ParsingOptions::default()
    };
    roxmltree::Document::parse_with_options(text, opt).map_err(|_| Error::BadReply("not XML"))
}

/// The text of the first child of `n` named `name` (any namespace), trimmed.
pub(crate) fn child_text<'a>(n: roxmltree::Node<'a, 'a>, name: &str) -> Option<&'a str> {
    n.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
        .and_then(|c| c.text())
        .map(str::trim)
}

/// `s` made safe to show: control and bidi-override characters dropped, whitespace
/// collapsed, at most `max` characters.
pub(crate) fn display_text(s: &str, max: usize) -> String {
    let mut o = String::new();
    let mut space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            space = !o.is_empty();
            continue;
        }
        if c.is_control() || matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}') {
            continue;
        }
        if space {
            o.push(' ');
            space = false;
        }
        o.push(c);
        if o.chars().count() >= max {
            break;
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes() {
        assert_eq!(escape("a<b>&\"c'\u{1}"), "a&lt;b&gt;&amp;&quot;c&apos;");
    }

    #[test]
    fn refuses_deep_nesting() {
        let deep = "<a>".repeat(100) + &"</a>".repeat(100);
        assert_eq!(parse(&deep).err(), Some(Error::BadReply("nested too deep")));
        let ok = "<a><b/><b/><c>x</c></a>";
        assert!(parse(ok).is_ok());
        let flat = "<a>".to_string() + &"<b/>".repeat(1000) + "</a>";
        assert!(parse(&flat).is_ok());
    }

    #[test]
    fn display_text_is_clean() {
        assert_eq!(
            display_text("  Living\u{202E} Room\n\tTV ", 64),
            "Living Room TV"
        );
        assert_eq!(display_text("abcdef", 3), "abc");
    }
}
