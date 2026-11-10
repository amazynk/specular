//! A bounded YAML 1.1 subset: what ASDF trees use.
//!
//! Block mappings and sequences, flow collections, plain, single- and double-quoted, literal
//! and folded scalars, `%TAG` directives with `!handle!suffix`, `!suffix` and `!<verbatim>`
//! tags, anchors and aliases, and `<<` merge keys. Plain scalars resolve as PyYAML does (YAML
//! 1.1: `yes`/`no`/`on`/`off` are booleans, `0o17`/`017` octal, `.inf`/`.nan` floats). Complex
//! (`?`) keys and collections as keys are refused. Nesting depth, the number of nodes (aliases
//! count every node they copy, so alias bombs stop early), the length of one scalar and the
//! total text held are all capped; anything beyond is refused with [`YamlError::Limit`].

use std::collections::HashMap;
use std::fmt;

/// Deepest nesting of collections.
pub const MAX_DEPTH: usize = 200;
/// Most nodes in the tree, counting every node an alias copies.
pub const MAX_NODES: usize = 2_000_000;
/// Longest single scalar, in bytes.
pub const MAX_SCALAR: usize = 16 << 20;
/// Most scalar text held in the whole tree, in bytes, counting alias copies.
pub const MAX_BYTES: usize = 64 << 20;

const CORE: &str = "tag:yaml.org,2002:";

/// One node with its tag (resolved to a full tag, e.g. `tag:stsci.edu:asdf/core/ndarray-1.1.0`;
/// `None` for untagged nodes and for YAML's own core tags, which only steer resolution).
#[derive(Clone, Debug, PartialEq)]
pub struct Value {
    pub tag: Option<Box<str>>,
    pub node: Node,
}

/// A node's content.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Null,
    Bool(bool),
    Int(i128),
    Float(f64),
    Str(String),
    Seq(Vec<Value>),
    /// Entries in file order; keys are the key scalars' text.
    Map(Vec<(String, Value)>),
}

/// Why a tree was not read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum YamlError {
    /// Not valid YAML (or not in the subset in a way that looks like an error).
    Syntax { line: usize, msg: String },
    /// Valid YAML this reader does not handle.
    Unsupported { line: usize, what: &'static str },
    /// Over one of the caps.
    Limit(&'static str),
}

impl fmt::Display for YamlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            YamlError::Syntax { line, msg } => write!(f, "YAML line {line}: {msg}"),
            YamlError::Unsupported { line, what } => {
                write!(f, "YAML line {line}: {what} not supported")
            }
            YamlError::Limit(what) => write!(f, "YAML tree too large: {what}"),
        }
    }
}

impl std::error::Error for YamlError {}

type R<T> = Result<T, YamlError>;

impl Value {
    fn plain(node: Node) -> Value {
        Value { tag: None, node }
    }

    /// A mapping entry (the last one, when a key repeats).
    pub fn get(&self, key: &str) -> Option<&Value> {
        match &self.node {
            Node::Map(m) => m.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Follows mapping keys.
    pub fn path(&self, keys: &[&str]) -> Option<&Value> {
        keys.iter().try_fold(self, |v, k| v.get(k))
    }

    pub fn as_str(&self) -> Option<&str> {
        match &self.node {
            Node::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self.node {
            Node::Int(i) => i64::try_from(i).ok(),
            _ => None,
        }
    }

    /// An integer or float as `f64`.
    pub fn as_f64(&self) -> Option<f64> {
        match self.node {
            Node::Int(i) => Some(i as f64),
            Node::Float(f) => Some(f),
            _ => None,
        }
    }

    pub fn as_seq(&self) -> Option<&[Value]> {
        match &self.node {
            Node::Seq(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&[(String, Value)]> {
        match &self.node {
            Node::Map(m) => Some(m),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self.node, Node::Null)
    }

    /// Whether the tag names `name` (any version, any authority): `ndarray` matches
    /// `tag:stsci.edu:asdf/core/ndarray-1.1.0` and `asdf://…/tags/ndarray-1.1.0`.
    pub fn tag_is(&self, name: &str) -> bool {
        self.tag.as_deref().is_some_and(|t| tag_name(t) == name)
    }

    /// A scalar as display text (`None` for collections).
    pub fn scalar_text(&self) -> Option<String> {
        Some(match &self.node {
            Node::Null => "null".to_owned(),
            Node::Bool(b) => b.to_string(),
            Node::Int(i) => i.to_string(),
            Node::Float(f) => float_text(*f),
            Node::Str(s) => s.clone(),
            Node::Seq(_) | Node::Map(_) => return None,
        })
    }
}

/// A float as YAML would write it.
pub fn float_text(f: f64) -> String {
    if f.is_nan() {
        ".nan".to_owned()
    } else if f.is_infinite() {
        if f > 0.0 { ".inf" } else { "-.inf" }.to_owned()
    } else {
        format!("{f:?}")
    }
}

/// The name part of a tag: its last path segment without the `-x.y.z` version.
pub fn tag_name(tag: &str) -> &str {
    let last = tag.rsplit(['/', ':']).next().unwrap_or(tag);
    match last.rfind('-') {
        Some(p) if last[p + 1..].starts_with(|c: char| c.is_ascii_digit()) => &last[..p],
        _ => last,
    }
}

/// Parses the first document of `text` (directives, then `---`, then one node, ending at
/// `...`, the next `---` or the end). Leading `#` comment lines (ASDF's header) are skipped.
pub fn parse(text: &str) -> R<Value> {
    let mut p = P {
        src: text,
        s: text.as_bytes(),
        i: 0,
        depth: 0,
        nodes: 0,
        bytes: 0,
        anchors: HashMap::new(),
        tags: vec![("!".into(), "!".into()), ("!!".into(), CORE.into())],
    };
    p.document()
}

struct Anchor {
    value: Value,
    nodes: usize,
    bytes: usize,
    /// Nesting levels in `value` (1 for a scalar): an alias must not take the tree past
    /// [`MAX_DEPTH`], else chained anchors build trees deep enough to overflow the stack.
    height: usize,
}

struct P<'a> {
    src: &'a str,
    s: &'a [u8],
    i: usize,
    depth: usize,
    nodes: usize,
    bytes: usize,
    anchors: HashMap<String, Anchor>,
    tags: Vec<(String, String)>,
}

#[derive(Clone, Copy, PartialEq)]
enum Style {
    Plain,
    Quoted,
}

fn is_blank(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | 0)
}

fn is_flow_ind(b: u8) -> bool {
    matches!(b, b',' | b'[' | b']' | b'{' | b'}')
}

impl<'a> P<'a> {
    fn peek(&self) -> u8 {
        self.s.get(self.i).copied().unwrap_or(0)
    }

    fn at(&self, k: usize) -> u8 {
        self.s.get(self.i + k).copied().unwrap_or(0)
    }

    fn eof(&self) -> bool {
        self.i >= self.s.len()
    }

    fn line(&self) -> usize {
        1 + self.s[..self.i.min(self.s.len())]
            .iter()
            .filter(|&&b| b == b'\n')
            .count()
    }

    fn line_start(&self, i: usize) -> usize {
        self.s[..i]
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |p| p + 1)
    }

    fn col(&self) -> usize {
        self.i - self.line_start(self.i)
    }

    /// Whether only spaces precede the cursor on its line.
    fn first_on_line(&self) -> bool {
        self.s[self.line_start(self.i)..self.i]
            .iter()
            .all(|&b| b == b' ' || b == b'\t')
    }

    fn err<T>(&self, msg: impl Into<String>) -> R<T> {
        Err(YamlError::Syntax {
            line: self.line(),
            msg: msg.into(),
        })
    }

    fn unsupported<T>(&self, what: &'static str) -> R<T> {
        Err(YamlError::Unsupported {
            line: self.line(),
            what,
        })
    }

    fn enter(&mut self) -> R<()> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(YamlError::Limit("nesting too deep"));
        }
        Ok(())
    }

    fn charge(&mut self, nodes: usize, bytes: usize) -> R<()> {
        self.nodes += nodes;
        self.bytes += bytes;
        if self.nodes > MAX_NODES {
            return Err(YamlError::Limit("too many nodes"));
        }
        if self.bytes > MAX_BYTES {
            return Err(YamlError::Limit("too much text"));
        }
        Ok(())
    }

    fn skip_inline(&mut self) {
        while matches!(self.peek(), b' ' | b'\t' | b'\r') {
            self.i += 1;
        }
    }

    /// Spaces, then a comment to the end of the line.
    fn skip_inline_comment(&mut self) {
        self.skip_inline();
        if self.peek() == b'#' {
            while !self.eof() && self.peek() != b'\n' {
                self.i += 1;
            }
        }
    }

    /// Spaces, comments and line breaks.
    fn skip_all(&mut self) {
        loop {
            self.skip_inline_comment();
            if self.peek() == b'\n' {
                self.i += 1;
            } else {
                break;
            }
        }
    }

    /// At a `---` or `...` line (column 0).
    fn doc_marker(&self) -> bool {
        let rest = &self.s[self.i..];
        (rest.starts_with(b"---") || rest.starts_with(b"..."))
            && is_blank(self.at(3))
            && self.line_start(self.i) == self.i
    }

    /// At a `- ` sequence entry indicator.
    fn seq_entry(&self) -> bool {
        self.peek() == b'-' && is_blank(self.at(1))
    }

    fn document(&mut self) -> R<Value> {
        // Directives and comments before `---`.
        loop {
            self.skip_all();
            if self.peek() == b'%' {
                let end = self.s[self.i..]
                    .iter()
                    .position(|&b| b == b'\n')
                    .map_or(self.s.len(), |p| self.i + p);
                let line = self.src[self.i..end].trim();
                let mut parts = line.split_whitespace();
                if parts.next() == Some("%TAG") {
                    match (parts.next(), parts.next()) {
                        (Some(h), Some(prefix)) => {
                            self.tags.retain(|(k, _)| k != h);
                            self.tags.push((h.to_owned(), prefix.to_owned()));
                        }
                        _ => return self.err("malformed %TAG directive"),
                    }
                }
                self.i = end;
                continue;
            }
            break;
        }
        let v = if self.s[self.i..].starts_with(b"---") && is_blank(self.at(3)) {
            self.i += 3;
            self.skip_inline_comment();
            if !self.eof() && self.peek() != b'\n' {
                self.block_node(-1)?
            } else {
                self.skip_all();
                if self.eof() || self.doc_marker() {
                    Value::plain(Node::Null)
                } else {
                    self.block_node(-1)?
                }
            }
        } else if self.eof() {
            Value::plain(Node::Null)
        } else {
            self.block_node(-1)?
        };
        self.skip_all();
        if !self.eof() && !self.doc_marker() {
            return self.err("unexpected content after the document");
        }
        Ok(v)
    }

    /// A block-context node at the cursor; `n` is the parent's indentation.
    fn block_node(&mut self, n: isize) -> R<Value> {
        self.enter()?;
        let r = self.block_node_inner(n);
        self.depth -= 1;
        r
    }

    fn block_node_inner(&mut self, n: isize) -> R<Value> {
        let (tag, anchor) = self.props(false)?;
        let had = tag.is_some() || anchor.is_some();
        let v = if had {
            self.skip_inline_comment();
            if self.eof() || self.peek() == b'\n' {
                // The content is on the following lines.
                self.skip_all();
                let col = self.col() as isize;
                if self.eof() || self.doc_marker() || col < n || (col == n && !self.seq_entry()) {
                    self.charge(1, 0)?;
                    Value::plain(Node::Null)
                } else {
                    self.block_bare(n, tag.as_deref())?
                }
            } else {
                self.block_bare(n, tag.as_deref())?
            }
        } else {
            self.block_bare(n, None)?
        };
        let v = self.apply_tag(v, tag)?;
        if let Some(a) = anchor {
            self.set_anchor(a, &v)?;
        }
        Ok(v)
    }

    /// Node content after any properties. `tag` is passed so that plain scalars resolve
    /// knowing it; it is applied by the caller.
    fn block_bare(&mut self, n: isize, tag: Option<&str>) -> R<Value> {
        let col = self.col();
        match self.peek() {
            b'*' => self.alias(false),
            b'-' if is_blank(self.at(1)) => self.block_seq(col),
            b'[' | b'{' => {
                let v = self.flow_node()?;
                let save = self.i;
                self.skip_inline();
                if self.peek() == b':' && is_blank(self.at(1)) {
                    return self.unsupported("a collection as a mapping key");
                }
                self.i = save;
                Ok(v)
            }
            b'|' | b'>' => {
                let s = self.block_scalar(n)?;
                self.scalar(s, Style::Quoted, tag)
            }
            b'?' if is_blank(self.at(1)) => self.unsupported("complex mapping keys"),
            b'"' | b'\'' => {
                let line = self.line_start(self.i);
                let s = self.quoted(false)?;
                if self.line_start(self.i) == line && self.key_follows() {
                    return self.block_map(col, s);
                }
                self.scalar(s, Style::Quoted, tag)
            }
            _ => {
                let (s, multi) = self.plain(n, false)?;
                if !multi && self.key_follows() {
                    return self.block_map(col, s);
                }
                self.scalar(s, Style::Plain, tag)
            }
        }
    }

    /// Whether `:` and a blank follow (after spaces); leaves the cursor on the `:` if so.
    fn key_follows(&mut self) -> bool {
        let save = self.i;
        self.skip_inline();
        if self.peek() == b':' && is_blank(self.at(1)) {
            true
        } else {
            self.i = save;
            false
        }
    }

    fn block_map(&mut self, c: usize, first: String) -> R<Value> {
        self.charge(1, 0)?;
        let mut map: Vec<(String, Value)> = Vec::new();
        let mut key = first;
        loop {
            // At the ':'.
            self.i += 1;
            let v = self.map_value(c)?;
            map.push((key, v));
            self.skip_all();
            if self.eof() || self.doc_marker() {
                break;
            }
            let col = self.col();
            if col < c {
                break;
            }
            if col > c || !self.first_on_line() {
                return self.err("bad indentation of a mapping entry");
            }
            if self.seq_entry() {
                // A sequence at the parent mapping's indentation ends this mapping.
                break;
            }
            key = self.block_key()?;
        }
        Ok(Value::plain(Node::Map(merge(map))))
    }

    fn block_key(&mut self) -> R<String> {
        let k = match self.peek() {
            b'"' | b'\'' => self.quoted(false)?,
            b'?' if is_blank(self.at(1)) => return self.unsupported("complex mapping keys"),
            b'[' | b'{' => return self.unsupported("a collection as a mapping key"),
            b'!' | b'&' => {
                let (_, anchor) = self.props(false)?;
                let k = self.block_key_bare()?;
                if let Some(a) = anchor {
                    let v = Value::plain(Node::Str(k.clone()));
                    self.set_anchor(a, &v)?;
                }
                return Ok(k);
            }
            b'*' => {
                let v = self.alias(false)?;
                let k = v.scalar_text().unwrap_or_default();
                if !self.key_follows() {
                    return self.err("expected ':' after a mapping key");
                }
                return Ok(k);
            }
            _ => self.plain_line()?,
        };
        if !self.key_follows() {
            return self.err("expected ':' after a mapping key");
        }
        Ok(k)
    }

    fn block_key_bare(&mut self) -> R<String> {
        let k = match self.peek() {
            b'"' | b'\'' => self.quoted(false)?,
            _ => self.plain_line()?,
        };
        if !self.key_follows() {
            return self.err("expected ':' after a mapping key");
        }
        Ok(k)
    }

    /// A one-line plain scalar (a key).
    fn plain_line(&mut self) -> R<String> {
        let start = self.i;
        let mut end = self.i;
        while !self.eof() {
            let b = self.peek();
            if b == b'\n' || (b == b':' && is_blank(self.at(1))) {
                break;
            }
            if b == b'#' && self.i > start && matches!(self.s[self.i - 1], b' ' | b'\t') {
                break;
            }
            self.i += 1;
            if !matches!(b, b' ' | b'\t' | b'\r') {
                end = self.i;
            }
        }
        self.i = end;
        if end - start > MAX_SCALAR {
            return Err(YamlError::Limit("scalar too long"));
        }
        self.charge(0, end - start)?;
        Ok(self.src[start..end].to_owned())
    }

    /// The value after a block mapping key's `:` (the cursor is just past it).
    fn map_value(&mut self, c: usize) -> R<Value> {
        self.skip_inline_comment();
        if !self.eof() && self.peek() != b'\n' {
            return self.block_node(c as isize);
        }
        self.skip_all();
        if self.eof() || self.doc_marker() {
            self.charge(1, 0)?;
            return Ok(Value::plain(Node::Null));
        }
        let col = self.col();
        if col > c || (col == c && self.seq_entry()) {
            self.block_node(c as isize)
        } else {
            self.charge(1, 0)?;
            Ok(Value::plain(Node::Null))
        }
    }

    fn block_seq(&mut self, c: usize) -> R<Value> {
        self.charge(1, 0)?;
        let mut items = Vec::new();
        loop {
            // At the '-'.
            self.i += 1;
            self.skip_inline_comment();
            let item = if !self.eof() && self.peek() != b'\n' {
                self.block_node(c as isize)?
            } else {
                self.skip_all();
                if self.eof() || self.doc_marker() || self.col() <= c {
                    self.charge(1, 0)?;
                    Value::plain(Node::Null)
                } else {
                    self.block_node(c as isize)?
                }
            };
            items.push(item);
            self.skip_all();
            if self.eof() || self.doc_marker() {
                break;
            }
            let col = self.col();
            if col == c && self.seq_entry() && self.first_on_line() {
                continue;
            }
            if col > c {
                return self.err("bad indentation of a sequence entry");
            }
            break;
        }
        Ok(Value::plain(Node::Seq(items)))
    }

    /// Tag and anchor properties, in either order.
    fn props(&mut self, flow: bool) -> R<(Option<String>, Option<String>)> {
        let mut tag = None;
        let mut anchor = None;
        loop {
            match self.peek() {
                b'!' if tag.is_none() => tag = Some(self.tag(flow)?),
                b'&' if anchor.is_none() => {
                    self.i += 1;
                    anchor = Some(self.name(flow)?);
                }
                _ => break,
            }
            self.skip_inline();
        }
        Ok((tag, anchor))
    }

    fn name(&mut self, flow: bool) -> R<String> {
        let start = self.i;
        while !self.eof() && !is_blank(self.peek()) && !(flow && is_flow_ind(self.peek())) {
            self.i += 1;
        }
        if self.i == start {
            return self.err("empty anchor or alias name");
        }
        Ok(self.src[start..self.i].to_owned())
    }

    fn tag(&mut self, flow: bool) -> R<String> {
        // At '!'.
        if self.at(1) == b'<' {
            let start = self.i + 2;
            let Some(len) = self.s[start..]
                .iter()
                .position(|&b| b == b'>' || b == b'\n')
            else {
                return self.err("unterminated verbatim tag");
            };
            if self.s[start + len] != b'>' {
                return self.err("unterminated verbatim tag");
            }
            self.i = start + len + 1;
            return Ok(self.src[start..start + len].to_owned());
        }
        let start = self.i;
        while !self.eof() && !is_blank(self.peek()) && !(flow && is_flow_ind(self.peek())) {
            self.i += 1;
        }
        let tok = &self.src[start..self.i];
        if tok == "!" {
            return Ok("!".to_owned());
        }
        let (handle, suffix) = if let Some(rest) = tok.strip_prefix("!!") {
            ("!!", rest)
        } else if let Some(p) = tok[1..].find('!') {
            (&tok[..p + 2], &tok[p + 2..])
        } else {
            ("!", &tok[1..])
        };
        let Some((_, prefix)) = self.tags.iter().find(|(h, _)| h == handle) else {
            return self.err(format!("undeclared tag handle {handle}"));
        };
        Ok(format!("{prefix}{}", percent_decode(suffix)))
    }

    fn alias(&mut self, flow: bool) -> R<Value> {
        self.i += 1;
        let name = self.name(flow)?;
        let Some(a) = self.anchors.get(&name) else {
            return self.err(format!("unknown alias *{name}"));
        };
        if self.depth.saturating_sub(1) + a.height > MAX_DEPTH {
            return Err(YamlError::Limit("nesting too deep"));
        }
        let (v, nodes, bytes) = (a.value.clone(), a.nodes, a.bytes);
        self.charge(nodes, bytes)?;
        Ok(v)
    }

    /// Keeps a copy of `v` for its aliases: charged like the tree, as 200 nested anchors
    /// each copying a tree near the caps would otherwise hold 200 times them.
    fn set_anchor(&mut self, name: String, v: &Value) -> R<()> {
        let (nodes, bytes, height) = size(v);
        self.charge(nodes, bytes)?;
        self.anchors.insert(
            name,
            Anchor {
                value: v.clone(),
                nodes,
                bytes,
                height,
            },
        );
        Ok(())
    }

    /// Resolves a scalar's text by its style and tag.
    fn scalar(&mut self, s: String, style: Style, tag: Option<&str>) -> R<Value> {
        self.charge(1, 0)?;
        let node = match tag {
            None if style == Style::Plain => resolve_plain(s),
            None | Some("!") => Node::Str(s),
            Some(t) => match t.strip_prefix(CORE) {
                Some("str") => Node::Str(s),
                Some("null") => Node::Null,
                Some("bool") => match resolve_plain(s.clone()) {
                    b @ Node::Bool(_) => b,
                    _ => return self.err(format!("not a boolean: {s}")),
                },
                Some("int") => match resolve_plain(s.clone()) {
                    i @ Node::Int(_) => i,
                    _ => return self.err(format!("not an integer: {s}")),
                },
                Some("float") => match resolve_plain(s.clone()) {
                    Node::Int(i) => Node::Float(i as f64),
                    f @ Node::Float(_) => f,
                    _ => return self.err(format!("not a float: {s}")),
                },
                _ if style == Style::Plain && t.starts_with(CORE) => resolve_plain(s),
                _ => Node::Str(s),
            },
        };
        Ok(Value::plain(node))
    }

    /// Puts a tag on a finished node; YAML's own tags and the non-specific `!` are dropped.
    fn apply_tag(&self, mut v: Value, tag: Option<String>) -> R<Value> {
        if let Some(t) = tag
            && t != "!"
            && !t.starts_with(CORE)
        {
            v.tag = Some(t.into_boxed_str());
        }
        Ok(v)
    }

    /// A plain scalar; `n` is the parent's indentation (continuation lines must be deeper,
    /// except in flow context). Returns the folded text and whether it spans lines.
    fn plain(&mut self, n: isize, flow: bool) -> R<(String, bool)> {
        let mut out = String::new();
        let mut multi = false;
        loop {
            let start = self.i;
            let mut end = self.i;
            while !self.eof() {
                let b = self.peek();
                if b == b'\n' {
                    break;
                }
                if b == b':' && (is_blank(self.at(1)) || (flow && is_flow_ind(self.at(1)))) {
                    break;
                }
                if b == b'#' && self.i > 0 && matches!(self.s[self.i - 1], b' ' | b'\t' | b'\n') {
                    break;
                }
                if flow && is_flow_ind(b) {
                    break;
                }
                self.i += 1;
                if !matches!(b, b' ' | b'\t' | b'\r') {
                    end = self.i;
                }
            }
            out.push_str(&self.src[start..end]);
            if out.len() > MAX_SCALAR {
                return Err(YamlError::Limit("scalar too long"));
            }
            self.i = end;
            let save = self.i;
            self.skip_inline();
            if self.peek() != b'\n' {
                self.i = save;
                break;
            }
            // Look past blank lines for a continuation.
            let mut j = self.i;
            let mut breaks = 0;
            let (indent, c) = loop {
                j += 1;
                breaks += 1;
                let ls = j;
                while matches!(self.s.get(j), Some(b' ' | b'\t' | b'\r')) {
                    j += 1;
                }
                match self.s.get(j) {
                    Some(b'\n') => continue,
                    Some(&c) => break (j - ls, c),
                    None => break (0, 0),
                }
            };
            let line_start = self.line_start(j);
            let marker = indent == 0
                && (self.s[line_start..].starts_with(b"---")
                    || self.s[line_start..].starts_with(b"..."))
                && matches!(
                    self.s.get(line_start + 3),
                    None | Some(b' ' | b'\t' | b'\r' | b'\n')
                );
            if c == 0
                || c == b'#'
                || marker
                || (!flow && indent as isize <= n)
                || (flow && (is_flow_ind(c) || c == b':'))
            {
                self.i = save;
                break;
            }
            if breaks == 1 {
                out.push(' ');
            } else {
                out.extend(std::iter::repeat_n('\n', breaks - 1));
            }
            self.i = j;
            multi = true;
        }
        self.charge(0, out.len())?;
        Ok((out, multi))
    }

    /// A single- or double-quoted scalar at the cursor.
    fn quoted(&mut self, _flow: bool) -> R<String> {
        let q = self.peek();
        self.i += 1;
        let mut out = String::new();
        loop {
            if out.len() > MAX_SCALAR {
                return Err(YamlError::Limit("scalar too long"));
            }
            let start = self.i;
            while !self.eof() && !matches!(self.peek(), b'\n' | b'\'' | b'"' | b'\\') {
                self.i += 1;
            }
            out.push_str(&self.src[start..self.i]);
            if self.eof() {
                return self.err("unterminated quoted scalar");
            }
            match self.peek() {
                b'\'' if q == b'\'' => {
                    if self.at(1) == b'\'' {
                        out.push('\'');
                        self.i += 2;
                    } else {
                        self.i += 1;
                        break;
                    }
                }
                b'"' if q == b'"' => {
                    self.i += 1;
                    break;
                }
                b'\\' if q == b'"' => {
                    self.i += 1;
                    if self.peek() == b'\n' || (self.peek() == b'\r' && self.at(1) == b'\n') {
                        // Escaped line break: joined without a space.
                        while self.peek() != b'\n' {
                            self.i += 1;
                        }
                        self.i += 1;
                        while matches!(self.peek(), b' ' | b'\t') {
                            self.i += 1;
                        }
                        continue;
                    }
                    let c = self.escape()?;
                    out.push(c);
                }
                b'\n' => {
                    while out.ends_with([' ', '\t', '\r']) {
                        out.pop();
                    }
                    let mut breaks = 0;
                    while self.peek() == b'\n' {
                        self.i += 1;
                        breaks += 1;
                        while matches!(self.peek(), b' ' | b'\t' | b'\r') {
                            self.i += 1;
                        }
                    }
                    if self.eof() {
                        return self.err("unterminated quoted scalar");
                    }
                    if breaks == 1 {
                        out.push(' ');
                    } else {
                        out.extend(std::iter::repeat_n('\n', breaks - 1));
                    }
                }
                other => {
                    // The other quote, or a backslash in single quotes: literal.
                    out.push(other as char);
                    self.i += 1;
                }
            }
        }
        if out.len() > MAX_SCALAR {
            return Err(YamlError::Limit("scalar too long"));
        }
        self.charge(0, out.len())?;
        Ok(out)
    }

    fn escape(&mut self) -> R<char> {
        let c = self.peek();
        self.i += 1;
        let hex = |p: &mut Self, n: usize| -> R<char> {
            let Some(h) = p.src.get(p.i..p.i + n) else {
                return p.err("truncated escape");
            };
            let v = u32::from_str_radix(h, 16).ok().and_then(char::from_u32);
            p.i += n;
            match v {
                Some(ch) => Ok(ch),
                None => p.err("bad escape"),
            }
        };
        Ok(match c {
            b'0' => '\0',
            b'a' => '\x07',
            b'b' => '\x08',
            b't' | b'\t' => '\t',
            b'n' => '\n',
            b'v' => '\x0b',
            b'f' => '\x0c',
            b'r' => '\r',
            b'e' => '\x1b',
            b' ' => ' ',
            b'"' => '"',
            b'/' => '/',
            b'\\' => '\\',
            b'N' => '\u{85}',
            b'_' => '\u{a0}',
            b'L' => '\u{2028}',
            b'P' => '\u{2029}',
            b'x' => hex(self, 2)?,
            b'u' => hex(self, 4)?,
            b'U' => hex(self, 8)?,
            _ => return self.err("unknown escape"),
        })
    }

    /// A literal (`|`) or folded (`>`) block scalar; `n` is the parent's indentation.
    fn block_scalar(&mut self, n: isize) -> R<String> {
        let literal = self.peek() == b'|';
        self.i += 1;
        #[derive(PartialEq)]
        enum Chomp {
            Strip,
            Clip,
            Keep,
        }
        let mut chomp = Chomp::Clip;
        let mut inc: Option<usize> = None;
        for _ in 0..2 {
            match self.peek() {
                b'+' => chomp = Chomp::Keep,
                b'-' => chomp = Chomp::Strip,
                d @ b'1'..=b'9' => inc = Some(usize::from(d - b'0')),
                _ => break,
            }
            self.i += 1;
        }
        self.skip_inline_comment();
        if !self.eof() && self.peek() != b'\n' {
            return self.err("unexpected text after a block scalar indicator");
        }
        if !self.eof() {
            self.i += 1;
        }
        let base = n.max(0) as usize;
        let mut indent = inc.map(|d| base + d);
        let mut lines: Vec<&str> = Vec::new();
        let mut total = 0usize;
        while !self.eof() {
            let ls = self.i;
            let mut k = ls;
            while self.s.get(k) == Some(&b' ') {
                k += 1;
            }
            let le = self.s[ls..]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(self.s.len(), |p| ls + p);
            let blank = self.s[k..le].iter().all(|&b| b == b'\r' || b == b'\t');
            let spaces = k - ls;
            let next = (le + 1).min(self.s.len());
            if !blank && spaces == 0 {
                let rest = &self.s[ls..];
                if (rest.starts_with(b"---") || rest.starts_with(b"..."))
                    && matches!(
                        self.s.get(ls + 3),
                        None | Some(b' ' | b'\t' | b'\r' | b'\n')
                    )
                {
                    break;
                }
            }
            let ind = match indent {
                Some(i) => i,
                None if blank => {
                    lines.push("");
                    self.i = next;
                    continue;
                }
                None => {
                    if (spaces as isize) <= n {
                        break;
                    }
                    indent = Some(spaces);
                    spaces
                }
            };
            if blank {
                let text = if spaces > ind {
                    self.src[ls + ind..le].trim_end_matches('\r')
                } else {
                    ""
                };
                lines.push(text);
                self.i = next;
                continue;
            }
            if spaces < ind {
                break;
            }
            let text = self.src[ls + ind..le].trim_end_matches('\r');
            total += text.len() + 1;
            if total > MAX_SCALAR {
                return Err(YamlError::Limit("scalar too long"));
            }
            lines.push(text);
            self.i = next;
        }
        if !self.eof() || self.s.last() == Some(&b'\n') {
            // Leave the cursor at the start of the line that ended the scalar.
            if self.i > 0 && self.i <= self.s.len() && self.s.get(self.i - 1) != Some(&b'\n') {
                self.i = self.line_start(self.i);
            }
        }
        let trailing = lines.iter().rev().take_while(|l| l.is_empty()).count();
        let body = &lines[..lines.len() - trailing];
        let mut out = String::new();
        if literal {
            out = body.join("\n");
        } else {
            let mut empties = 0usize;
            let mut prev: Option<&str> = None;
            for l in body {
                if l.is_empty() {
                    empties += 1;
                    continue;
                }
                if let Some(p) = prev {
                    let more = p.starts_with([' ', '\t']) || l.starts_with([' ', '\t']);
                    if !more && empties == 0 {
                        out.push(' ');
                    } else {
                        let k = if more { empties + 1 } else { empties };
                        out.extend(std::iter::repeat_n('\n', k));
                    }
                } else {
                    out.extend(std::iter::repeat_n('\n', empties));
                }
                empties = 0;
                out.push_str(l);
                prev = Some(l);
            }
        }
        match chomp {
            Chomp::Strip => {}
            Chomp::Clip => {
                if !body.is_empty() {
                    out.push('\n');
                }
            }
            Chomp::Keep => {
                let k = trailing + usize::from(!body.is_empty());
                out.extend(std::iter::repeat_n('\n', k));
            }
        }
        self.charge(0, out.len())?;
        Ok(out)
    }

    /// Spaces, line breaks and comments inside a flow collection.
    fn skip_flow(&mut self) {
        loop {
            match self.peek() {
                b' ' | b'\t' | b'\r' | b'\n' => self.i += 1,
                b'#' if self.i == 0 || is_blank(self.s[self.i - 1]) => {
                    while !self.eof() && self.peek() != b'\n' {
                        self.i += 1;
                    }
                }
                _ => break,
            }
        }
    }

    fn flow_node(&mut self) -> R<Value> {
        self.enter()?;
        let r = self.flow_node_inner();
        self.depth -= 1;
        r
    }

    fn flow_node_inner(&mut self) -> R<Value> {
        let (tag, anchor) = self.props(true)?;
        self.skip_flow();
        let v = match self.peek() {
            b'[' => self.flow_seq()?,
            b'{' => self.flow_map()?,
            b'*' => self.alias(true)?,
            b'"' | b'\'' => {
                let s = self.quoted(true)?;
                self.scalar(s, Style::Quoted, tag.as_deref())?
            }
            b',' | b']' | b'}' => {
                self.charge(1, 0)?;
                Value::plain(Node::Null)
            }
            0 => return self.err("unterminated flow collection"),
            _ => {
                let (s, _) = self.plain(-1, true)?;
                self.scalar(s, Style::Plain, tag.as_deref())?
            }
        };
        let v = self.apply_tag(v, tag)?;
        if let Some(a) = anchor {
            self.set_anchor(a, &v)?;
        }
        Ok(v)
    }

    fn flow_seq(&mut self) -> R<Value> {
        self.charge(1, 0)?;
        self.i += 1;
        let mut items = Vec::new();
        loop {
            self.skip_flow();
            match self.peek() {
                b']' => {
                    self.i += 1;
                    break;
                }
                0 => return self.err("unterminated flow sequence"),
                b'?' if is_blank(self.at(1)) => return self.unsupported("complex mapping keys"),
                _ => {}
            }
            let v = self.flow_node()?;
            self.skip_flow();
            if self.peek() == b':' {
                // A single-pair mapping inside a flow sequence.
                self.i += 1;
                self.skip_flow();
                let Some(k) = v.scalar_text() else {
                    return self.unsupported("a collection as a mapping key");
                };
                let val = self.flow_node()?;
                self.charge(1, 0)?;
                items.push(Value::plain(Node::Map(vec![(k, val)])));
                self.skip_flow();
            } else {
                items.push(v);
            }
            match self.peek() {
                b',' => self.i += 1,
                b']' => {}
                _ => return self.err("expected ',' or ']' in a flow sequence"),
            }
        }
        Ok(Value::plain(Node::Seq(items)))
    }

    fn flow_map(&mut self) -> R<Value> {
        self.charge(1, 0)?;
        self.i += 1;
        let mut map = Vec::new();
        loop {
            self.skip_flow();
            let key = match self.peek() {
                b'}' => {
                    self.i += 1;
                    break;
                }
                0 => return self.err("unterminated flow mapping"),
                b'?' if is_blank(self.at(1)) => return self.unsupported("complex mapping keys"),
                b'[' | b'{' => return self.unsupported("a collection as a mapping key"),
                _ => {
                    let k = self.flow_node()?;
                    match k.scalar_text() {
                        Some(t) => t,
                        None => return self.unsupported("a collection as a mapping key"),
                    }
                }
            };
            self.skip_flow();
            let v = if self.peek() == b':' {
                self.i += 1;
                self.skip_flow();
                if matches!(self.peek(), b',' | b'}') {
                    self.charge(1, 0)?;
                    Value::plain(Node::Null)
                } else {
                    self.flow_node()?
                }
            } else {
                self.charge(1, 0)?;
                Value::plain(Node::Null)
            };
            map.push((key, v));
            self.skip_flow();
            match self.peek() {
                b',' => self.i += 1,
                b'}' => {}
                _ => return self.err("expected ',' or '}' in a flow mapping"),
            }
        }
        Ok(Value::plain(Node::Map(merge(map))))
    }
}

/// Applies `<<` merge keys: entries of the merged mapping(s) the mapping lacks.
fn merge(map: Vec<(String, Value)>) -> Vec<(String, Value)> {
    if !map.iter().any(|(k, _)| k == "<<") {
        return map;
    }
    let mut out: Vec<(String, Value)> = Vec::new();
    let mut merged: Vec<(String, Value)> = Vec::new();
    for (k, v) in map {
        if k == "<<" {
            let sources: Vec<Value> = match v.node {
                Node::Map(_) => vec![v],
                Node::Seq(s) => s,
                _ => continue,
            };
            for s in sources {
                if let Node::Map(m) = s.node {
                    for (mk, mv) in m {
                        if !merged.iter().any(|(k2, _)| *k2 == mk) {
                            merged.push((mk, mv));
                        }
                    }
                }
            }
        } else {
            out.push((k, v));
        }
    }
    for (k, v) in merged {
        if !out.iter().any(|(k2, _)| *k2 == k) {
            out.push((k, v));
        }
    }
    out
}

/// Nodes, text bytes and nesting levels in a value.
fn size(v: &Value) -> (usize, usize, usize) {
    match &v.node {
        Node::Str(s) => (1, s.len(), 1),
        Node::Seq(items) => items.iter().fold((1, 0, 1), |(n, b, h), i| {
            let (n2, b2, h2) = size(i);
            (n + n2, b + b2, h.max(h2 + 1))
        }),
        Node::Map(m) => m.iter().fold((1, 0, 1), |(n, b, h), (k, i)| {
            let (n2, b2, h2) = size(i);
            (n + n2, b + b2 + k.len(), h.max(h2 + 1))
        }),
        _ => (1, 0, 1),
    }
}

fn percent_decode(s: &str) -> String {
    if !s.contains('%') {
        return s.to_owned();
    }
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let Some(v) = s
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// YAML 1.1 implicit resolution of a plain scalar (PyYAML's resolver).
fn resolve_plain(s: String) -> Node {
    match s.as_str() {
        "" | "~" | "null" | "Null" | "NULL" => return Node::Null,
        "yes" | "Yes" | "YES" | "true" | "True" | "TRUE" | "on" | "On" | "ON" => {
            return Node::Bool(true);
        }
        "no" | "No" | "NO" | "false" | "False" | "FALSE" | "off" | "Off" | "OFF" => {
            return Node::Bool(false);
        }
        _ => {}
    }
    if let Some(i) = parse_int(&s) {
        return Node::Int(i);
    }
    if let Some(f) = parse_float(&s) {
        return Node::Float(f);
    }
    Node::Str(s)
}

fn split_sign(s: &str) -> (bool, &str) {
    if let Some(r) = s.strip_prefix('-') {
        (true, r)
    } else {
        (false, s.strip_prefix('+').unwrap_or(s))
    }
}

fn parse_int(s: &str) -> Option<i128> {
    let (neg, body) = split_sign(s);
    if body.is_empty() || body.starts_with('_') {
        return None;
    }
    let digits = |t: &str, radix: u32| -> Option<i128> {
        let clean: String = t.chars().filter(|&c| c != '_').collect();
        if clean.is_empty() || !clean.chars().all(|c| c.is_digit(radix)) {
            return None;
        }
        i128::from_str_radix(&clean, radix).ok()
    };
    let v = if let Some(r) = body.strip_prefix("0x") {
        digits(r, 16)?
    } else if let Some(r) = body.strip_prefix("0b") {
        digits(r, 2)?
    } else if let Some(r) = body.strip_prefix("0o") {
        digits(r, 8)?
    } else if body.len() > 1 && body.starts_with('0') {
        digits(&body[1..], 8)?
    } else if body.contains(':') {
        // Sexagesimal: 190:20:30.
        let mut v: i128 = 0;
        for (k, part) in body.split(':').enumerate() {
            let d = digits(part, 10)?;
            if k > 0 && d >= 60 {
                return None;
            }
            v = v.checked_mul(60)?.checked_add(d)?;
        }
        v
    } else {
        digits(body, 10)?
    };
    Some(if neg { -v } else { v })
}

fn parse_float(s: &str) -> Option<f64> {
    let (neg, body) = split_sign(s);
    let sign = if neg { -1.0 } else { 1.0 };
    match body {
        ".inf" | ".Inf" | ".INF" => return Some(sign * f64::INFINITY),
        ".nan" | ".NaN" | ".NAN" if s == body => return Some(f64::NAN),
        _ => {}
    }
    let clean: String = body.chars().filter(|&c| c != '_').collect();
    if clean.is_empty() || !clean.starts_with(|c: char| c.is_ascii_digit() || c == '.') {
        return None;
    }
    if clean.contains(':') {
        // Sexagesimal float: 190:20:30.15.
        let mut v = 0.0;
        for part in clean.split(':') {
            v = v * 60.0 + part.parse::<f64>().ok()?;
        }
        return Some(sign * v);
    }
    // Digits, at most one '.', an optional exponent; at least one digit.
    let (mant, exp) = match clean.find(['e', 'E']) {
        Some(p) => (&clean[..p], Some(&clean[p + 1..])),
        None => (clean.as_str(), None),
    };
    if !mant.chars().any(|c| c.is_ascii_digit())
        || mant.chars().filter(|&c| c == '.').count() > 1
        || !mant.chars().all(|c| c.is_ascii_digit() || c == '.')
    {
        return None;
    }
    if let Some(e) = exp {
        let e = e.strip_prefix(['+', '-']).unwrap_or(e);
        if e.is_empty() || !e.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
    } else if !mant.contains('.') {
        return None;
    }
    clean.parse::<f64>().ok().map(|f| sign * f)
}

#[cfg(test)]
mod tests;
