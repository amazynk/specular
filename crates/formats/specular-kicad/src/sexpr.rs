//! The S-expressions KiCad writes: lists in parentheses of bare symbols, numbers and quoted
//! strings (with `\"`, `\\`, `\n`, `\t` and `\r` escapes). Atoms borrow the text; a quoted
//! string is unescaped only when it has escapes. Parsing is iterative with a depth limit.

use std::borrow::Cow;

use crate::Error;

/// A bare symbol or number, or a quoted string.
#[derive(Clone, Debug, PartialEq)]
pub struct Atom<'a> {
    pub text: Cow<'a, str>,
    pub quoted: bool,
}

/// A list or an atom.
#[derive(Clone, Debug, PartialEq)]
pub enum Node<'a> {
    List(Vec<Node<'a>>),
    Atom(Atom<'a>),
}

static EMPTY: [Node<'static>; 0] = [];

impl<'a> Node<'a> {
    /// The atom's text.
    pub fn atom(&self) -> Option<&str> {
        match self {
            Node::Atom(a) => Some(&a.text),
            Node::List(_) => None,
        }
    }

    /// The atom's text as a number.
    pub fn number(&self) -> Option<f64> {
        let v: f64 = self.atom()?.parse().ok()?;
        v.is_finite().then_some(v)
    }

    /// A list's elements (empty for an atom).
    pub fn items(&self) -> &[Node<'a>] {
        match self {
            Node::List(v) => v,
            Node::Atom(_) => &EMPTY,
        }
    }

    /// The first element of a list when it is an atom: what the list is.
    pub fn head(&self) -> Option<&str> {
        self.items().first()?.atom()
    }

    /// Whether this is a list headed `name`.
    pub fn is(&self, name: &str) -> bool {
        self.head() == Some(name)
    }

    /// The elements after the head.
    pub fn args(&self) -> &[Node<'a>] {
        self.items().get(1..).unwrap_or(&[])
    }

    /// The `i`-th element after the head, as text.
    pub fn arg(&self, i: usize) -> Option<&str> {
        self.args().get(i)?.atom()
    }

    /// The `i`-th element after the head, as a number.
    pub fn num(&self, i: usize) -> Option<f64> {
        self.args().get(i)?.number()
    }

    /// The first list among the elements headed `name`.
    pub fn child(&self, name: &str) -> Option<&Node<'a>> {
        self.args().iter().find(|n| n.is(name))
    }

    /// Every list among the elements headed `name`.
    pub fn children<'s>(&'s self, name: &'s str) -> impl Iterator<Item = &'s Node<'a>> + 's {
        self.args().iter().filter(move |n| n.is(name))
    }

    /// The lists among the elements.
    pub fn lists(&self) -> impl Iterator<Item = &Node<'a>> {
        self.args().iter().filter(|n| matches!(n, Node::List(_)))
    }

    /// Whether a bare (unquoted) atom `flag` is among the elements (`hide`, `locked`).
    pub fn has_flag(&self, flag: &str) -> bool {
        self.args()
            .iter()
            .any(|n| matches!(n, Node::Atom(a) if !a.quoted && a.text == flag))
    }

    /// The first argument of child `name` as text (`(layer "F.Cu")` → "F.Cu").
    pub fn value(&self, name: &str) -> Option<&str> {
        self.child(name)?.arg(0)
    }

    /// The first argument of child `name` as a number.
    pub fn value_num(&self, name: &str) -> Option<f64> {
        self.child(name)?.num(0)
    }

    /// A yes/no setting: `(name yes)`, `(name no)`, or a bare `name` meaning yes. `None` when
    /// it is not there.
    pub fn yes(&self, name: &str) -> Option<bool> {
        if let Some(c) = self.child(name) {
            return Some(!matches!(c.arg(0), Some("no" | "false")));
        }
        self.has_flag(name).then_some(true)
    }
}

/// Parses the first expression of `text` (a list). What follows it is ignored.
pub fn parse(text: &str) -> Result<Node<'_>, Error> {
    parse_with_depth(text, crate::Limits::default().depth)
}

/// [`parse`] with a nesting limit.
pub fn parse_with_depth(text: &str, max_depth: usize) -> Result<Node<'_>, Error> {
    let b = text.as_bytes();
    let mut i = 0;
    let mut stack: Vec<Vec<Node<'_>>> = Vec::new();
    loop {
        // Whitespace (and stray bytes outside a list before the first one).
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            return Err(if stack.is_empty() {
                Error::NotThisFormat
            } else {
                Error::Truncated(format!("{} lists left open", stack.len()))
            });
        }
        match b[i] {
            b'(' => {
                if stack.len() >= max_depth {
                    return Err(Error::Malformed(format!(
                        "lists nested deeper than {max_depth}"
                    )));
                }
                stack.push(Vec::new());
                i += 1;
            }
            b')' => {
                let Some(done) = stack.pop() else {
                    return Err(Error::Malformed("a ')' without its '('".into()));
                };
                i += 1;
                let node = Node::List(done);
                match stack.last_mut() {
                    Some(parent) => parent.push(node),
                    None => return Ok(node),
                }
            }
            b'"' => {
                let start = i + 1;
                let mut j = start;
                let mut escaped = false;
                while j < b.len() && b[j] != b'"' {
                    if b[j] == b'\\' {
                        escaped = true;
                        j += 1;
                    }
                    j += 1;
                }
                if j >= b.len() {
                    return Err(Error::Truncated("a string left open".into()));
                }
                let raw = &text[start..j];
                let s = if escaped {
                    Cow::Owned(unescape(raw))
                } else {
                    Cow::Borrowed(raw)
                };
                i = j + 1;
                let atom = Node::Atom(Atom {
                    text: s,
                    quoted: true,
                });
                match stack.last_mut() {
                    Some(p) => p.push(atom),
                    None => return Err(Error::NotThisFormat),
                }
            }
            _ => {
                let start = i;
                while i < b.len() && !b[i].is_ascii_whitespace() && !matches!(b[i], b'(' | b')') {
                    i += 1;
                }
                // `start..i` ends on an ASCII byte or the end: char boundaries.
                let atom = Node::Atom(Atom {
                    text: Cow::Borrowed(&text[start..i]),
                    quoted: false,
                });
                match stack.last_mut() {
                    Some(p) => p.push(atom),
                    None => return Err(Error::NotThisFormat),
                }
            }
        }
    }
}

fn unescape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut it = raw.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(o) => out.push(o),
            None => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lists_atoms_and_strings() {
        let n = parse(
            "(kicad_pcb (version 20240108) (layer \"F.Cu\") (text \"a \\\"b\\\"\\nc\") hide)",
        )
        .unwrap();
        assert!(n.is("kicad_pcb"));
        assert_eq!(n.value_num("version"), Some(20240108.0));
        assert_eq!(n.value("layer"), Some("F.Cu"));
        assert_eq!(n.value("text"), Some("a \"b\"\nc"));
        assert!(n.has_flag("hide"));
        assert_eq!(n.yes("hide"), Some(true));
        let m = parse("(x (hide no))").unwrap();
        assert_eq!(m.yes("hide"), Some(false));
        assert_eq!(m.yes("locked"), None);
    }

    #[test]
    fn errors_without_panicking() {
        assert!(matches!(parse("(a (b"), Err(Error::Truncated(_))));
        assert!(matches!(parse("(a \"b"), Err(Error::Truncated(_))));
        assert!(matches!(parse(")"), Err(Error::Malformed(_))));
        assert!(matches!(parse("   "), Err(Error::NotThisFormat)));
        assert!(matches!(parse("abc"), Err(Error::NotThisFormat)));
        let deep = "(".repeat(1000);
        assert!(matches!(parse(&deep), Err(Error::Malformed(_))));
        // Multi-byte text in atoms and strings.
        let n = parse("(t \"µΩ\" µm)").unwrap();
        assert_eq!(n.arg(0), Some("µΩ"));
        assert_eq!(n.arg(1), Some("µm"));
    }
}
