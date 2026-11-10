//! A small XML tree for the scene formats read whole (COLLADA): elements by local name (no
//! namespaces), attributes and text unescaped. UTF-8 with or without a byte-order mark, and
//! UTF-16 with one, are read.

use quick_xml::events::Event;

use crate::{Error, Result};

#[derive(Clone, Debug, Default)]
pub(crate) struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
    pub text: String,
}

impl Node {
    pub fn attr(&self, k: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(a, _)| a == k)
            .map(|(_, v)| v.as_str())
    }

    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }

    pub fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// Depth-first search for an element.
    pub fn find(&self, name: &str) -> Option<&Node> {
        if self.name == name {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find(name))
    }

    /// Every element of `name` below, depth first.
    pub fn descendants<'a>(&'a self, name: &str, out: &mut Vec<&'a Node>) {
        for c in &self.children {
            if c.name == name {
                out.push(c);
            }
            c.descendants(name, out);
        }
    }

    pub fn text_trim(&self) -> &str {
        self.text.trim()
    }
}

/// UTF-8 text of `bytes`: BOMs removed, UTF-16 converted.
fn utf8(bytes: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    if let Some(b) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        return std::borrow::Cow::Borrowed(b);
    }
    let le = bytes.starts_with(&[0xff, 0xfe]);
    let be = bytes.starts_with(&[0xfe, 0xff]);
    if le || be {
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
        let s = String::from_utf16_lossy(&units);
        // The declaration still says UTF-16; quick-xml reads the bytes as UTF-8.
        let s = s.replacen("encoding=\"UTF-16\"", "encoding=\"UTF-8\"", 1);
        let s = s.replacen("encoding=\"utf-16\"", "encoding=\"UTF-8\"", 1);
        return std::borrow::Cow::Owned(s.into_bytes());
    }
    std::borrow::Cow::Borrowed(bytes)
}

fn unescape(s: &str) -> String {
    quick_xml::escape::unescape(s).map_or_else(|_| s.to_owned(), |c| c.into_owned())
}

/// Parses `bytes` into its root element. At most `max_nodes` elements.
pub(crate) fn parse(bytes: &[u8], max_nodes: usize) -> Result<Node> {
    let data = utf8(bytes);
    let mut r = quick_xml::Reader::from_reader(&data[..]);
    let mut stack: Vec<Node> = vec![Node::default()];
    let mut count = 0usize;
    let name_of = |e: &quick_xml::events::BytesStart| {
        String::from_utf8_lossy(e.local_name().as_ref()).into_owned()
    };
    let attrs_of = |e: &quick_xml::events::BytesStart| {
        e.attributes()
            .with_checks(false)
            .flatten()
            .map(|a| {
                (
                    String::from_utf8_lossy(a.key.local_name().as_ref()).into_owned(),
                    unescape(&String::from_utf8_lossy(&a.value)),
                )
            })
            .collect::<Vec<_>>()
    };
    loop {
        let ev = r
            .read_event()
            .map_err(|e| Error::Malformed(format!("XML: {e}")))?;
        match ev {
            Event::Start(e) => {
                count += 1;
                if count > max_nodes || stack.len() > 512 {
                    return Err(Error::TooLarge("XML with too many elements".into()));
                }
                stack.push(Node {
                    name: name_of(&e),
                    attrs: attrs_of(&e),
                    ..Node::default()
                });
            }
            Event::Empty(e) => {
                count += 1;
                if count > max_nodes {
                    return Err(Error::TooLarge("XML with too many elements".into()));
                }
                let n = Node {
                    name: name_of(&e),
                    attrs: attrs_of(&e),
                    ..Node::default()
                };
                stack.last_mut().expect("root").children.push(n);
            }
            Event::End(_) => {
                if stack.len() < 2 {
                    return Err(Error::Malformed("XML: unbalanced end tag".into()));
                }
                let n = stack.pop().expect("checked");
                stack.last_mut().expect("root").children.push(n);
            }
            Event::Text(t) => {
                let s = t.decode().unwrap_or_default();
                stack.last_mut().expect("root").text.push_str(&s);
            }
            Event::CData(t) => {
                let s = String::from_utf8_lossy(&t);
                stack.last_mut().expect("root").text.push_str(&s);
            }
            Event::GeneralRef(g) => {
                let name = g.decode().unwrap_or_default();
                let s = unescape(&format!("&{name};"));
                stack.last_mut().expect("root").text.push_str(&s);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if stack.len() != 1 {
        return Err(Error::Malformed("XML: unclosed elements".into()));
    }
    stack
        .pop()
        .and_then(|d| d.children.into_iter().next())
        .ok_or_else(|| Error::Malformed("XML: no root element".into()))
}
