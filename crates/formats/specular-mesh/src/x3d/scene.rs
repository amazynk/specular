//! The scene graph both encodings parse into: nodes in an arena, fields by name, values
//! either typed (classic encoding) or raw attribute text (XML encoding) read on demand.

use std::collections::HashSet;

use crate::{Error, Result};

/// A field's value.
#[derive(Clone, Debug)]
pub(crate) enum Val {
    /// An XML attribute's text, read as numbers, strings or a boolean when asked.
    Raw(String),
    Nums(Vec<f64>),
    Strs(Vec<String>),
    Bool(bool),
    Nodes(Vec<u32>),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SNode {
    pub kind: String,
    /// Its DEF name.
    pub name: Option<String>,
    pub fields: Vec<(String, Val)>,
}

/// Most nodes in a scene (with its inlined files).
pub(crate) const MAX_NODES: usize = 8_000_000;

#[derive(Default)]
pub(crate) struct Scene {
    pub nodes: Vec<SNode>,
    /// Names of PROTO and EXTERNPROTO declarations: their instances are not drawn, unless
    /// they name a node type read here (VRML 97 files declare X3D's CAD and triangle-set
    /// nodes as EXTERNPROTOs).
    pub protos: HashSet<String>,
    pub title: Option<String>,
    pub generator: Option<String>,
}

impl Scene {
    pub fn add(&mut self, n: SNode) -> Result<u32> {
        if self.nodes.len() >= MAX_NODES {
            return Err(Error::TooLarge(format!(
                "more than {MAX_NODES} scene nodes"
            )));
        }
        self.nodes.push(n);
        Ok((self.nodes.len() - 1) as u32)
    }

    pub fn kind(&self, n: u32) -> &str {
        self.nodes.get(n as usize).map_or("", |x| x.kind.as_str())
    }

    pub fn field(&self, n: u32, name: &str) -> Option<&Val> {
        self.nodes
            .get(n as usize)?
            .fields
            .iter()
            .rev()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v)
    }

    pub fn nums(&self, n: u32, name: &str) -> Vec<f64> {
        match self.field(n, name) {
            Some(Val::Nums(v)) => v.clone(),
            Some(Val::Raw(s)) => raw_nums(s),
            Some(Val::Bool(b)) => vec![f64::from(u8::from(*b))],
            _ => Vec::new(),
        }
    }

    /// Integer values (indices); non-integers and out-of-range values become −1.
    pub fn ints(&self, n: u32, name: &str) -> Vec<i64> {
        let to = |v: f64| {
            if v.is_finite() && v.fract() == 0.0 && v.abs() < 9e15 {
                v as i64
            } else {
                -1
            }
        };
        match self.field(n, name) {
            Some(Val::Nums(v)) => v.iter().map(|&x| to(x)).collect(),
            Some(Val::Raw(s)) => raw_nums(s).into_iter().map(to).collect(),
            _ => Vec::new(),
        }
    }

    pub fn num(&self, n: u32, name: &str, default: f64) -> f64 {
        match self.field(n, name) {
            Some(Val::Nums(v)) => v.first().copied(),
            Some(Val::Raw(s)) => raw_nums(s).first().copied(),
            _ => None,
        }
        .filter(|v| v.is_finite())
        .unwrap_or(default)
    }

    pub fn vec<const N: usize>(&self, n: u32, name: &str, default: [f64; N]) -> [f64; N] {
        let v = self.nums(n, name);
        if v.len() >= N && v[..N].iter().all(|x| x.is_finite()) {
            std::array::from_fn(|i| v[i])
        } else {
            default
        }
    }

    pub fn boolean(&self, n: u32, name: &str, default: bool) -> bool {
        match self.field(n, name) {
            Some(Val::Bool(b)) => *b,
            Some(Val::Raw(s)) => match s.trim() {
                "true" | "TRUE" | "True" => true,
                "false" | "FALSE" | "False" => false,
                _ => default,
            },
            _ => default,
        }
    }

    pub fn strings(&self, n: u32, name: &str) -> Vec<String> {
        match self.field(n, name) {
            Some(Val::Strs(v)) => v.clone(),
            Some(Val::Raw(s)) => raw_strings(s),
            _ => Vec::new(),
        }
    }

    pub fn nodes(&self, n: u32, name: &str) -> Vec<u32> {
        match self.field(n, name) {
            Some(Val::Nodes(v)) => v.clone(),
            _ => Vec::new(),
        }
    }

    pub fn node(&self, n: u32, name: &str) -> Option<u32> {
        match self.field(n, name) {
            Some(Val::Nodes(v)) => v.first().copied(),
            _ => None,
        }
    }
}

/// Numbers of an XML attribute: separated by white space or commas.
pub(crate) fn raw_nums(s: &str) -> Vec<f64> {
    s.split(|c: char| c.is_ascii_whitespace() || c == ',')
        .filter(|t| !t.is_empty())
        .map(|t| number(t.as_bytes()).unwrap_or(f64::NAN))
        .collect()
}

/// A number token: decimal, or hexadecimal with `0x`.
pub(crate) fn number(t: &[u8]) -> Option<f64> {
    let s = std::str::from_utf8(t).ok()?;
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u64::from_str_radix(h, 16).ok().map(|v| v as f64);
    }
    s.parse::<f64>().ok()
}

/// Strings of an XML MFString attribute: `"a" "b"` with `\"` and `\\` escapes; text without
/// quotes is one string.
pub(crate) fn raw_strings(s: &str) -> Vec<String> {
    if !s.contains('"') {
        let t = s.trim();
        return if t.is_empty() {
            Vec::new()
        } else {
            vec![t.to_owned()]
        };
    }
    let mut out = Vec::new();
    let mut cur: Option<String> = None;
    let mut esc = false;
    for c in s.chars() {
        match (&mut cur, c) {
            (Some(x), _) if esc => {
                x.push(c);
                esc = false;
            }
            (Some(_), '\\') => esc = true,
            (Some(_), '"') => out.push(cur.take().expect("open")),
            (Some(x), _) => x.push(c),
            (None, '"') => cur = Some(String::new()),
            (None, _) => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_values() {
        assert_eq!(raw_nums("1 2,3 0x10"), vec![1.0, 2.0, 3.0, 16.0]);
        assert_eq!(
            raw_strings(r#""a b.png" "c\"d""#),
            vec!["a b.png".to_owned(), "c\"d".to_owned()]
        );
        assert_eq!(raw_strings(" t.png "), vec!["t.png".to_owned()]);
    }
}
