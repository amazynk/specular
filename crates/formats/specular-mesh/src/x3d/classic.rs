//! The classic VRML encoding (VRML 97 and X3D's `.x3dv`): a streaming lexer and a parser
//! that needs no node schema. A field's value is told by its first token: `[` a list, a
//! number a run of numbers, a string one string, `TRUE`/`FALSE` a boolean, a node type
//! followed by `{` (or `DEF`/`USE`) a node. PROTO and EXTERNPROTO declarations are skipped
//! (their names kept so instances can be noted), ROUTEs and the X3D header statements too.

use std::collections::{HashMap, VecDeque};

use super::scene::{SNode, Scene, Val, number};
use crate::{Error, Result};

#[derive(Clone, Debug, PartialEq)]
enum Tok<'a> {
    Num(f64),
    Str(String),
    Id(&'a [u8]),
    Sym(u8),
    Eof,
}

struct Lexer<'a> {
    b: &'a [u8],
    i: usize,
    ahead: VecDeque<Tok<'a>>,
}

fn is_id_char(c: u8) -> bool {
    c > 0x20
        && !matches!(
            c,
            b'"' | b'#' | b'\'' | b',' | b'[' | b']' | b'{' | b'}' | 0x7f
        )
}

impl<'a> Lexer<'a> {
    fn lex(&mut self) -> Tok<'a> {
        let b = self.b;
        loop {
            while self.i < b.len() && (b[self.i].is_ascii_whitespace() || b[self.i] == b',') {
                self.i += 1;
            }
            if self.i < b.len() && b[self.i] == b'#' {
                while self.i < b.len() && b[self.i] != b'\n' && b[self.i] != b'\r' {
                    self.i += 1;
                }
                continue;
            }
            break;
        }
        let Some(&c) = b.get(self.i) else {
            return Tok::Eof;
        };
        match c {
            b'[' | b']' | b'{' | b'}' => {
                self.i += 1;
                Tok::Sym(c)
            }
            b'"' => {
                self.i += 1;
                let mut out = Vec::new();
                while self.i < b.len() && b[self.i] != b'"' {
                    if b[self.i] == b'\\' && self.i + 1 < b.len() {
                        self.i += 1;
                    }
                    out.push(b[self.i]);
                    self.i += 1;
                }
                self.i += 1;
                Tok::Str(String::from_utf8_lossy(&out).into_owned())
            }
            b'\'' => {
                // Not VRML; skipped as a symbol.
                self.i += 1;
                Tok::Sym(c)
            }
            _ => {
                let s = self.i;
                while self.i < b.len() && is_id_char(b[self.i]) {
                    self.i += 1;
                }
                if self.i == s {
                    self.i += 1;
                    return Tok::Sym(c);
                }
                let t = &b[s..self.i];
                if c.is_ascii_digit() || c == b'-' || c == b'+' || c == b'.' {
                    match number(t) {
                        Some(v) => Tok::Num(v),
                        None => Tok::Id(t),
                    }
                } else {
                    Tok::Id(t)
                }
            }
        }
    }

    fn peek(&mut self, k: usize) -> &Tok<'a> {
        while self.ahead.len() <= k {
            let t = self.lex();
            self.ahead.push_back(t);
        }
        &self.ahead[k]
    }

    fn next(&mut self) -> Tok<'a> {
        match self.ahead.pop_front() {
            Some(t) => t,
            None => self.lex(),
        }
    }
}

/// Deepest nesting of nodes and lists.
const MAX_DEPTH: usize = 400;

struct Parser<'a, 's> {
    lx: Lexer<'a>,
    scene: &'s mut Scene,
    defs: HashMap<String, u32>,
}

/// Parses classic-encoded `bytes` into `scene`; the root nodes.
pub(crate) fn parse(bytes: &[u8], scene: &mut Scene) -> Result<Vec<u32>> {
    let mut p = Parser {
        lx: Lexer {
            b: bytes,
            i: 0,
            ahead: VecDeque::new(),
        },
        scene,
        defs: HashMap::new(),
    };
    let mut roots = Vec::new();
    loop {
        match p.lx.peek(0) {
            Tok::Eof => break,
            Tok::Sym(_) | Tok::Num(_) | Tok::Str(_) => {
                // Stray tokens between statements.
                p.lx.next();
            }
            Tok::Id(_) => {
                if !p.statement()?
                    && let Some(n) = p.node(0)?
                {
                    roots.push(n);
                }
            }
        }
    }
    Ok(roots)
}

fn text(t: &[u8]) -> String {
    String::from_utf8_lossy(t).into_owned()
}

impl Parser<'_, '_> {
    /// A statement that is not a node (PROTO, EXTERNPROTO, ROUTE and X3D's header
    /// statements): consumed, `true`.
    fn statement(&mut self) -> Result<bool> {
        let Tok::Id(id) = *self.lx.peek(0) else {
            return Ok(false);
        };
        match id {
            b"PROTO" => {
                self.lx.next();
                if let Tok::Id(name) = self.lx.next() {
                    self.scene.protos.insert(text(name));
                }
                self.skip_balanced()?; // [ interface ]
                self.skip_balanced()?; // { body }
            }
            b"EXTERNPROTO" => {
                self.lx.next();
                if let Tok::Id(name) = self.lx.next() {
                    self.scene.protos.insert(text(name));
                }
                self.skip_balanced()?;
                // The URL: a string or a list of them.
                match self.lx.peek(0) {
                    Tok::Sym(b'[') => self.skip_balanced()?,
                    Tok::Str(_) => {
                        self.lx.next();
                    }
                    _ => {}
                }
            }
            b"ROUTE" => {
                // ROUTE a.b TO c.d
                for _ in 0..4 {
                    self.lx.next();
                }
            }
            b"PROFILE" | b"COMPONENT" => {
                self.lx.next();
                self.lx.next();
            }
            b"UNIT" => {
                for _ in 0..4 {
                    self.lx.next();
                }
            }
            b"META" => {
                self.lx.next();
                let k = self.lx.next();
                let v = self.lx.next();
                if let (Tok::Str(k), Tok::Str(v)) = (k, v) {
                    match k.as_str() {
                        "title" => self.scene.title = Some(v),
                        "generator" => self.scene.generator = Some(v),
                        _ => {}
                    }
                }
            }
            b"IMPORT" | b"EXPORT" => {
                // IMPORT a.b AS c / EXPORT a AS b
                self.lx.next();
                self.lx.next();
                if matches!(self.lx.peek(0), Tok::Id(b"AS")) {
                    self.lx.next();
                    self.lx.next();
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Skips one `[…]` or `{…}` group (nothing when the next token opens neither).
    fn skip_balanced(&mut self) -> Result<()> {
        if !matches!(self.lx.peek(0), Tok::Sym(b'[' | b'{')) {
            return Ok(());
        }
        let mut depth = 0usize;
        loop {
            match self.lx.next() {
                Tok::Sym(b'[' | b'{') => depth += 1,
                Tok::Sym(b']' | b'}') => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(());
                    }
                }
                Tok::Eof => return Err(Error::Malformed("VRML: unclosed PROTO".into())),
                _ => {}
            }
        }
    }

    /// A node, `DEF name node`, `USE name` or `NULL`.
    fn node(&mut self, depth: usize) -> Result<Option<u32>> {
        if depth > MAX_DEPTH {
            return Err(Error::Malformed("VRML: nodes nested too deeply".into()));
        }
        let mut name = None;
        match self.lx.next() {
            Tok::Id(b"DEF") => {
                if let Tok::Id(n) = self.lx.next() {
                    name = Some(text(n));
                }
            }
            Tok::Id(b"USE") => {
                return Ok(match self.lx.next() {
                    Tok::Id(n) => self.defs.get(&text(n)).copied(),
                    _ => None,
                });
            }
            Tok::Id(b"NULL") => return Ok(None),
            Tok::Id(t) => return self.body(text(t), None, depth).map(Some),
            Tok::Eof => return Err(Error::Malformed("VRML: unexpected end".into())),
            _ => return Ok(None),
        }
        let kind = match self.lx.next() {
            Tok::Id(t) => text(t),
            _ => return Err(Error::Malformed("VRML: DEF without a node".into())),
        };
        let n = self.body(kind, name.clone(), depth)?;
        if let Some(name) = name {
            self.defs.insert(name, n);
        }
        Ok(Some(n))
    }

    /// `{ fields }` of a node of `kind` (its type already read).
    fn body(&mut self, kind: String, name: Option<String>, depth: usize) -> Result<u32> {
        if self.lx.next() != Tok::Sym(b'{') {
            return Err(Error::Malformed(format!("VRML: {kind} without '{{'")));
        }
        let mut fields = Vec::new();
        loop {
            match self.lx.peek(0) {
                Tok::Sym(b'}') => {
                    self.lx.next();
                    break;
                }
                Tok::Eof => return Err(Error::Malformed(format!("VRML: unclosed {kind}"))),
                Tok::Id(_) => {
                    if self.statement()? {
                        continue;
                    }
                    let Tok::Id(f) = self.lx.next() else {
                        continue;
                    };
                    let f = text(f);
                    if let Some(v) = self.value(depth)? {
                        fields.push((f, v));
                    }
                }
                _ => {
                    self.lx.next();
                }
            }
        }
        self.scene.add(SNode { kind, name, fields })
    }

    /// A field's value, `None` for `IS` and for a field written without one.
    fn value(&mut self, depth: usize) -> Result<Option<Val>> {
        let starts_node = |p: &mut Self| match *p.lx.peek(0) {
            Tok::Id(b"DEF" | b"USE") => true,
            Tok::Id(_) => matches!(p.lx.peek(1), Tok::Sym(b'{')),
            _ => false,
        };
        match self.lx.peek(0).clone() {
            Tok::Sym(b'[') => {
                self.lx.next();
                let (mut nums, mut strs, mut nodes) = (Vec::new(), Vec::new(), Vec::new());
                loop {
                    match self.lx.peek(0).clone() {
                        Tok::Sym(b']') => {
                            self.lx.next();
                            break;
                        }
                        Tok::Eof => return Err(Error::Malformed("VRML: unclosed list".into())),
                        Tok::Num(v) => {
                            self.lx.next();
                            nums.push(v);
                        }
                        Tok::Str(s) => {
                            self.lx.next();
                            strs.push(s);
                        }
                        Tok::Id(b"TRUE") => {
                            self.lx.next();
                            nums.push(1.0);
                        }
                        Tok::Id(b"FALSE") => {
                            self.lx.next();
                            nums.push(0.0);
                        }
                        Tok::Id(b"NULL") => {
                            self.lx.next();
                        }
                        _ if starts_node(self) => {
                            if let Some(n) = self.node(depth + 1)? {
                                nodes.push(n);
                            }
                        }
                        _ => {
                            self.lx.next();
                        }
                    }
                }
                Ok(Some(if !nodes.is_empty() {
                    Val::Nodes(nodes)
                } else if !strs.is_empty() {
                    Val::Strs(strs)
                } else {
                    Val::Nums(nums)
                }))
            }
            Tok::Num(_) => {
                let mut nums = Vec::new();
                while let Tok::Num(v) = *self.lx.peek(0) {
                    self.lx.next();
                    nums.push(v);
                }
                Ok(Some(Val::Nums(nums)))
            }
            Tok::Str(s) => {
                self.lx.next();
                Ok(Some(Val::Strs(vec![s])))
            }
            Tok::Id(b"TRUE") => {
                self.lx.next();
                Ok(Some(Val::Bool(true)))
            }
            Tok::Id(b"FALSE") => {
                self.lx.next();
                Ok(Some(Val::Bool(false)))
            }
            Tok::Id(b"NULL") => {
                self.lx.next();
                Ok(Some(Val::Nodes(Vec::new())))
            }
            Tok::Id(b"IS") => {
                self.lx.next();
                self.lx.next();
                Ok(None)
            }
            _ if starts_node(self) => Ok(Some(Val::Nodes(
                self.node(depth + 1)?.into_iter().collect(),
            ))),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nodes_fields_def_use_and_protos() {
        let src = br#"#VRML V2.0 utf8
PROTO Thing [ field SFFloat size 1 ] { Box { size IS size } }
DEF T Transform { translation 1 2 3
  children [
    DEF S Shape { appearance Appearance { material Material { diffuseColor 1 0 0 } }
      geometry Box { size 2, 2, 2 } }
    USE S
    Thing { size 3 }
  ]
}
ROUTE A.b TO C.d
WorldInfo { title "Hello \"world\"" info [ "a" "b" ] }
"#;
        let mut sc = Scene::default();
        let roots = parse(src, &mut sc).unwrap();
        assert_eq!(roots.len(), 2);
        let t = roots[0];
        assert_eq!(sc.kind(t), "Transform");
        assert_eq!(sc.nodes[t as usize].name.as_deref(), Some("T"));
        assert_eq!(sc.vec(t, "translation", [0.0; 3]), [1.0, 2.0, 3.0]);
        let kids = sc.nodes(t, "children");
        assert_eq!(kids.len(), 3);
        assert_eq!(kids[0], kids[1]);
        assert_eq!(sc.kind(kids[2]), "Thing");
        assert!(sc.protos.contains("Thing"));
        let g = sc.node(kids[0], "geometry").unwrap();
        assert_eq!(sc.vec(g, "size", [0.0; 3]), [2.0; 3]);
        assert_eq!(
            sc.strings(roots[1], "title"),
            vec!["Hello \"world\"".to_owned()]
        );
    }

    #[test]
    fn junk_does_not_panic() {
        for src in [
            &b"Transform { children [ Shape { geometry"[..],
            b"DEF",
            b"]]]}}}{{{",
            b"PROTO X [",
            b"USE nothing",
            b"Shape { geometry IndexedFaceSet { coordIndex [ 0 1 2 -1 ",
        ] {
            let mut sc = Scene::default();
            let _ = parse(src, &mut sc);
        }
        let deep = "Group { children [ ".repeat(1000);
        let mut sc = Scene::default();
        assert!(parse(deep.as_bytes(), &mut sc).is_err());
    }
}
