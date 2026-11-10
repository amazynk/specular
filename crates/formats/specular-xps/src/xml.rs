//! XML parsing with bounds, and the helpers the markup readers share.

use roxmltree::{Document, Node, ParsingOptions};

/// Most XML nodes in one part.
pub const MAX_NODES: u32 = 4_000_000;

/// The markup compatibility namespace (`mc:AlternateContent`).
pub const MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";

/// Namespaces whose `mc:Choice` content we understand.
const KNOWN: &[&str] = &[
    "http://schemas.microsoft.com/xps/2005/06",
    "http://schemas.openxps.org/oxps/v1.0",
];

/// Deepest element nesting accepted (the markup itself is drawn to [`crate::page::MAX_DEPTH`]);
/// deeper documents are refused before the XML parser, which recurses per level.
pub const MAX_NESTING: usize = specular_limits::MAX_XML_DEPTH;

/// Why a part's XML was not read.
#[derive(Debug)]
pub enum XmlError {
    Parse,
    TooDeep,
}

pub fn parse(text: &str) -> Result<Document<'_>, XmlError> {
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

/// The element children of `n`, with `mc:AlternateContent` replaced by the content of its
/// first understood `mc:Choice`, else its `mc:Fallback`.
pub fn children<'a, 'i>(n: Node<'a, 'i>) -> Vec<Node<'a, 'i>> {
    let mut out = Vec::new();
    expand(n, &mut out, 0);
    out
}

fn expand<'a, 'i>(n: Node<'a, 'i>, out: &mut Vec<Node<'a, 'i>>, depth: u32) {
    for c in n.children().filter(Node::is_element) {
        if c.tag_name().namespace() == Some(MC) {
            if c.tag_name().name() == "AlternateContent" && depth < 8 {
                let pick =
                    c.children()
                        .filter(Node::is_element)
                        .find(|ch| match ch.tag_name().name() {
                            "Choice" => ch.attribute("Requires").is_some_and(|r| {
                                r.split_whitespace().all(|p| {
                                    ch.lookup_namespace_uri(Some(p))
                                        .is_some_and(|u| KNOWN.contains(&u))
                                })
                            }),
                            "Fallback" => true,
                            _ => false,
                        });
                if let Some(p) = pick {
                    expand(p, out, depth + 1);
                }
            }
            continue;
        }
        out.push(c);
    }
}

/// The `x:Key` of a resource.
pub fn key<'a>(n: Node<'a, '_>) -> Option<&'a str> {
    n.attributes()
        .find(|a| a.name() == "Key")
        .map(|a| a.value())
}

/// `{StaticResource name}` → `name`.
pub fn static_resource(v: &str) -> Option<&str> {
    let v = v.trim();
    let inner = v.strip_prefix('{')?.strip_suffix('}')?.trim();
    let rest = inner.strip_prefix("StaticResource")?;
    Some(rest.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nesting_depth() {
        let deep = "<a>".repeat(600) + &"</a>".repeat(600);
        assert!(matches!(parse(&deep), Err(XmlError::TooDeep)));
        assert!(parse("<a><b/></a>").is_ok());
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
