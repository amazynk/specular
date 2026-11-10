//! Bounded XML parsing for the XML-based readers, and small attribute helpers.

pub use roxmltree;
use roxmltree::{Document, Node, ParsingOptions};

/// Most XML nodes in one part.
pub const MAX_NODES: u32 = 4_000_000;
/// Deepest element nesting accepted; deeper documents are refused before the XML parser,
/// which recurses per level.
pub const MAX_NESTING: usize = specular_limits::MAX_XML_DEPTH;

/// Largest XML part parsed: the document tree takes several times the text's size.
pub const MAX_XML_PART: usize = 64 << 20;

/// Why a part's XML was not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XmlError {
    Parse,
    TooDeep,
    TooLarge,
}

impl std::fmt::Display for XmlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            XmlError::Parse => "malformed XML",
            XmlError::TooDeep => "XML nested too deeply",
            XmlError::TooLarge => "XML part too large",
        })
    }
}

/// Parses `text` without DTDs, with [`MAX_NODES`] and [`MAX_NESTING`].
pub fn parse(text: &str) -> Result<Document<'_>, XmlError> {
    if text.len() > MAX_XML_PART {
        return Err(XmlError::TooLarge);
    }
    if !specular_limits::xml_within_depth(text.as_bytes(), MAX_NESTING) {
        return Err(XmlError::TooDeep);
    }
    Document::parse_with_options(
        text,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_NODES,
            ..ParsingOptions::default()
        },
    )
    .map_err(|_| XmlError::Parse)
}

/// UTF-8 text of an XML part (a UTF-8 BOM dropped; UTF-16 with a BOM converted).
pub fn text_of(bytes: &[u8]) -> Option<String> {
    if let Some(b) = bytes.strip_prefix(b"\xef\xbb\xbf") {
        return String::from_utf8(b.to_vec()).ok();
    }
    if bytes.starts_with(b"\xff\xfe") || bytes.starts_with(b"\xfe\xff") {
        let le = bytes[0] == 0xff;
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| {
                if le {
                    u16::from_le_bytes([c[0], c[1]])
                } else {
                    u16::from_be_bytes([c[0], c[1]])
                }
            })
            .collect();
        return String::from_utf16(&units).ok();
    }
    String::from_utf8(bytes.to_vec()).ok()
}

/// The first element child of `n` with local name `name`.
pub fn child<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

/// The element children of `n` with local name `name`.
pub fn children<'a, 'i: 'a>(
    n: Node<'a, 'i>,
    name: &'a str,
) -> impl Iterator<Item = Node<'a, 'i>> + 'a {
    n.children()
        .filter(move |c| c.is_element() && c.tag_name().name() == name)
}

/// An attribute by local name, in any namespace.
pub fn attr<'a>(n: Node<'a, '_>, name: &str) -> Option<&'a str> {
    n.attributes().find(|a| a.name() == name).map(|a| a.value())
}

/// All text below `n`, concatenated.
pub fn all_text(n: Node<'_, '_>) -> String {
    n.descendants()
        .filter(|d| d.is_text())
        .filter_map(|d| d.text())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A part larger than [`MAX_XML_PART`] is refused before its tree is built.
    #[test]
    fn large_part_is_refused() {
        let big = format!("<a>{}</a>", " ".repeat(MAX_XML_PART));
        assert_eq!(parse(&big).err(), Some(XmlError::TooLarge));
    }

    /// Markup that hid nesting from the old pre-scan (a `/>` in a quoted value, a closing
    /// tag inside a comment, CDATA section or processing instruction) is counted; 100 000
    /// levels would overflow the parser's stack.
    #[test]
    fn hidden_nesting_is_refused() {
        specular_thread::on_stack(256 * 1024, || {
            for open in [
                "<a b=\"/>\">",
                "<a b='x>'>",
                "<a><!-- </a> -->",
                "<a><![CDATA[</a>]]>",
                "<a><?p </a>?>",
            ] {
                let deep = open.repeat(100_000) + &"</a>".repeat(100_000);
                assert!(matches!(parse(&deep), Err(XmlError::TooDeep)), "{open}");
            }
        });
    }
}
