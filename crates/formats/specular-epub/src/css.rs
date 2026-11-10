//! A small CSS reader: rules with type, class, id, attribute and `:first-child` selectors and
//! descendant/child combinators, `@media` blocks read through, `@font-face` kept, `@import`
//! listed. Selectors it cannot match (siblings, `::before`, `:nth-child`…) drop their rule.

use std::collections::HashMap;

use crate::xml::{Doc, NodeId};

#[derive(Debug, Clone)]
pub struct Decl {
    pub name: String,
    pub value: String,
    pub important: bool,
}

#[derive(Debug, Clone, Default)]
pub struct FontFace {
    pub family: String,
    /// `url(…)` sources as written (relative to the stylesheet).
    pub src: Vec<String>,
    pub weight: Option<f32>,
    pub italic: bool,
    /// Directory of the stylesheet the rule came from.
    pub base: String,
}

#[derive(Debug, Clone, Default)]
struct Compound {
    tag: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
    attrs: Vec<(String, Option<String>)>,
    first_child: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Comb {
    Descendant,
    Child,
}

#[derive(Debug, Clone)]
struct Selector {
    /// Rightmost last; the combinator joins a compound to the one before it.
    parts: Vec<(Comb, Compound)>,
    spec: u32,
}

#[derive(Debug, Clone)]
struct Rule {
    sel: Selector,
    order: u32,
    decls: std::sync::Arc<Vec<Decl>>,
}

/// The rules of every stylesheet of a page, in cascade order.
#[derive(Debug, Clone, Default)]
pub struct Styles {
    rules: Vec<Rule>,
    by_id: HashMap<String, Vec<u32>>,
    by_class: HashMap<String, Vec<u32>>,
    by_tag: HashMap<String, Vec<u32>>,
    any: Vec<u32>,
    pub font_faces: Vec<FontFace>,
    order: u32,
}

/// Most rules kept from all of a page's stylesheets.
const MAX_RULES: usize = 50_000;

fn strip_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find("/*") {
        out.push_str(&rest[..p]);
        rest = match rest[p + 2..].find("*/") {
            Some(e) => &rest[p + 2 + e + 2..],
            None => "",
        };
    }
    out.push_str(rest);
    out
}

/// The end of the block opened just before `s` (index of its `}`), skipping nested blocks
/// and strings.
fn block_end(s: &str) -> usize {
    let mut depth = 0i32;
    let mut quote = None;
    for (i, c) in s.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '{') => depth += 1,
            (None, '}') => {
                if depth == 0 {
                    return i;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    s.len()
}

pub fn parse_decls(s: &str) -> Vec<Decl> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut quote = None;
    let mut start = 0;
    let mut push = |part: &str| {
        if let Some((k, v)) = part.split_once(':') {
            let name = k.trim().to_ascii_lowercase();
            let mut v = v.trim();
            let mut important = false;
            if let Some(p) = v.to_ascii_lowercase().rfind("!important") {
                v = v[..p].trim();
                important = true;
            }
            if !name.is_empty() && !v.is_empty() && out.len() < 1000 {
                out.push(Decl {
                    name,
                    value: v.to_owned(),
                    important,
                });
            }
        }
    };
    for (i, c) in s.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '(') => depth += 1,
            (None, ')') => depth -= 1,
            (None, ';') if depth <= 0 => {
                push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    push(&s[start..]);
    out
}

fn parse_compound(s: &str) -> Option<Compound> {
    let mut c = Compound::default();
    let b = s.as_bytes();
    let mut i = 0;
    let ident_end = |from: usize| {
        let mut j = from;
        while j < b.len()
            && (b[j].is_ascii_alphanumeric() || b[j] == b'-' || b[j] == b'_' || b[j] >= 0x80)
        {
            j += 1;
        }
        j
    };
    if i < b.len() && b[i] == b'*' {
        i += 1;
    } else if i < b.len() && (b[i].is_ascii_alphabetic() || b[i] >= 0x80) {
        let e = ident_end(i);
        c.tag = Some(s[i..e].to_ascii_lowercase());
        i = e;
        if i < b.len() && b[i] == b'|' {
            return None;
        }
    }
    while i < b.len() {
        match b[i] {
            b'.' => {
                let e = ident_end(i + 1);
                if e == i + 1 {
                    return None;
                }
                c.classes.push(s[i + 1..e].to_owned());
                i = e;
            }
            b'#' => {
                let e = ident_end(i + 1);
                if e == i + 1 {
                    return None;
                }
                c.id = Some(s[i + 1..e].to_owned());
                i = e;
            }
            b'[' => {
                let e = s[i..].find(']').map(|o| i + o)?;
                let inner = &s[i + 1..e];
                if let Some((k, v)) = inner.split_once('=') {
                    let k = k.trim_end_matches(['~', '|', '^', '$', '*']).trim();
                    if inner.contains("|=")
                        || inner.contains("^=")
                        || inner.contains("$=")
                        || inner.contains("*=")
                    {
                        return None;
                    }
                    let word = inner.contains("~=");
                    let v = v.trim().trim_matches(['"', '\'']).to_owned();
                    let key = if word {
                        format!("~{}", k.to_ascii_lowercase())
                    } else {
                        k.to_ascii_lowercase()
                    };
                    c.attrs.push((key, Some(v)));
                } else {
                    c.attrs.push((inner.trim().to_ascii_lowercase(), None));
                }
                i = e + 1;
            }
            b':' => {
                if b.get(i + 1) == Some(&b':') {
                    return None;
                }
                let e = ident_end(i + 1);
                match s[i + 1..e].to_ascii_lowercase().as_str() {
                    "first-child" => c.first_child = true,
                    "link" | "visited" => {}
                    _ => return None,
                }
                i = e;
            }
            _ => return None,
        }
    }
    Some(c)
}

fn parse_selector(s: &str) -> Option<Selector> {
    let s = s.trim();
    if s.is_empty() || s.contains('+') || s.contains('~') && !s.contains("~=") {
        return None;
    }
    let spaced = s.replace('>', " > ");
    let mut parts = Vec::new();
    let mut comb = Comb::Descendant;
    for tok in spaced.split_whitespace() {
        if tok == ">" {
            comb = Comb::Child;
            continue;
        }
        parts.push((comb, parse_compound(tok)?));
        comb = Comb::Descendant;
    }
    if parts.is_empty() || parts.len() > 16 {
        return None;
    }
    let mut spec = 0u32;
    for (_, c) in &parts {
        spec += u32::from(c.id.is_some()) << 16;
        spec += ((c.classes.len() + c.attrs.len() + usize::from(c.first_child)) as u32) << 8;
        spec += u32::from(c.tag.is_some());
    }
    Some(Selector { parts, spec })
}

impl Styles {
    /// Adds a stylesheet; `base` is its directory (for `url()`s). Returns its `@import`s.
    pub fn add(&mut self, css: &str, base: &str) -> Vec<String> {
        let css = strip_comments(css);
        let mut imports = Vec::new();
        self.add_block(&css, base, &mut imports, 0);
        imports
    }

    fn add_block(&mut self, s: &str, base: &str, imports: &mut Vec<String>, depth: u32) {
        let mut rest = s;
        loop {
            rest = rest.trim_start();
            if rest.is_empty() || self.rules.len() >= MAX_RULES {
                return;
            }
            if let Some(at) = rest.strip_prefix('@') {
                let brace = at.find('{');
                let semi = at.find(';');
                match (brace, semi) {
                    (Some(b), sm) if sm.is_none_or(|x| x > b) => {
                        let head = at[..b].trim().to_ascii_lowercase();
                        let body = &at[b + 1..];
                        let end = block_end(body);
                        let inner = &body[..end];
                        if head.starts_with("media") && depth < 4 {
                            if !head.contains("print") || head.contains("screen") {
                                self.add_block(inner, base, imports, depth + 1);
                            }
                        } else if head.starts_with("supports") && depth < 4 {
                            self.add_block(inner, base, imports, depth + 1);
                        } else if head == "font-face" {
                            self.font_face(inner, base);
                        }
                        rest = body.get(end + 1..).unwrap_or("");
                    }
                    (_, Some(sm)) => {
                        let stmt = at[..sm].trim();
                        if let Some(i) = stmt.strip_prefix("import")
                            && let Some(u) = url_of(i)
                        {
                            imports.push(u);
                        }
                        rest = &at[sm + 1..];
                    }
                    _ => return,
                }
                continue;
            }
            let Some(b) = rest.find('{') else { return };
            let sels = &rest[..b];
            let body = &rest[b + 1..];
            let end = block_end(body);
            let decls = std::sync::Arc::new(parse_decls(&body[..end]));
            rest = body.get(end + 1..).unwrap_or("");
            if decls.is_empty() {
                continue;
            }
            for sel in sels.split(',') {
                let Some(sel) = parse_selector(sel) else {
                    continue;
                };
                let idx = self.rules.len() as u32;
                let key = &sel.parts.last().map(|p| &p.1);
                if let Some(k) = key {
                    if let Some(id) = &k.id {
                        self.by_id.entry(id.clone()).or_default().push(idx);
                    } else if let Some(c) = k.classes.first() {
                        self.by_class.entry(c.clone()).or_default().push(idx);
                    } else if let Some(t) = &k.tag {
                        self.by_tag.entry(t.clone()).or_default().push(idx);
                    } else {
                        self.any.push(idx);
                    }
                }
                self.order += 1;
                self.rules.push(Rule {
                    sel,
                    order: self.order,
                    decls: decls.clone(),
                });
            }
        }
    }

    fn font_face(&mut self, body: &str, base: &str) {
        let mut f = FontFace {
            base: base.to_owned(),
            ..Default::default()
        };
        for d in parse_decls(body) {
            match d.name.as_str() {
                "font-family" => f.family = unquote(&d.value).to_owned(),
                "src" => {
                    let mut rest = d.value.as_str();
                    while let Some(p) = rest.to_ascii_lowercase().find("url(") {
                        let after = &rest[p..];
                        if let Some(u) = url_of(after) {
                            f.src.push(u);
                        }
                        rest = &after[4..];
                    }
                }
                "font-weight" => f.weight = crate::style::parse_weight(&d.value, 400.0),
                "font-style" => f.italic = matches!(d.value.trim(), "italic" | "oblique"),
                _ => {}
            }
        }
        if !f.family.is_empty() && !f.src.is_empty() && self.font_faces.len() < 256 {
            self.font_faces.push(f);
        }
    }

    /// The declarations that apply to `node`, lowest priority first (author rules by
    /// specificity and order, then the `style` attribute, then `!important` ones).
    pub fn matching(&self, doc: &Doc, node: NodeId) -> Vec<Decl> {
        let mut cand: Vec<u32> = Vec::new();
        if let Some(id) = doc.attr(node, "id")
            && let Some(v) = self.by_id.get(id)
        {
            cand.extend(v);
        }
        if let Some(cls) = doc.attr(node, "class") {
            for c in cls.split_whitespace() {
                if let Some(v) = self.by_class.get(c) {
                    cand.extend(v);
                }
            }
        }
        if let Some(t) = doc.local(node)
            && let Some(v) = self.by_tag.get(t)
        {
            cand.extend(v);
        }
        cand.extend(&self.any);
        cand.sort_unstable();
        cand.dedup();
        let mut hits: Vec<&Rule> = cand
            .into_iter()
            .map(|i| &self.rules[i as usize])
            .filter(|r| matches(doc, node, &r.sel.parts))
            .collect();
        hits.sort_by_key(|r| (r.sel.spec, r.order));
        let mut normal = Vec::new();
        let mut important = Vec::new();
        for r in hits {
            for d in r.decls.iter() {
                if d.important {
                    important.push(d.clone());
                } else {
                    normal.push(d.clone());
                }
            }
        }
        if let Some(st) = doc.attr_exact(node, "style") {
            for d in parse_decls(st) {
                if d.important {
                    important.push(d);
                } else {
                    normal.push(d);
                }
            }
        }
        normal.extend(important);
        normal
    }
}

fn unquote(s: &str) -> &str {
    s.trim().trim_matches(['"', '\'']).trim()
}

/// `url(x)`, `url("x")` or `"x"` at the start of `s`.
fn url_of(s: &str) -> Option<String> {
    let s = s.trim();
    let lower = s.to_ascii_lowercase();
    if let Some(p) = lower.find("url(") {
        let inner = &s[p + 4..];
        let e = inner.find(')')?;
        let u = unquote(&inner[..e]);
        return (!u.is_empty()).then(|| u.to_owned());
    }
    let u = unquote(s.trim_end_matches(';'));
    let u = u.split_whitespace().next()?;
    (!u.is_empty()).then(|| unquote(u).to_owned())
}

fn element_matches(doc: &Doc, n: NodeId, c: &Compound) -> bool {
    if doc.name(n).is_none() {
        return false;
    }
    if let Some(t) = &c.tag
        && doc.local(n) != Some(t.as_str())
    {
        return false;
    }
    if let Some(id) = &c.id
        && doc.attr(n, "id") != Some(id.as_str())
    {
        return false;
    }
    if !c.classes.is_empty() {
        let cls = doc.attr(n, "class").unwrap_or("");
        if !c
            .classes
            .iter()
            .all(|k| cls.split_whitespace().any(|x| x == k))
        {
            return false;
        }
    }
    for (k, v) in &c.attrs {
        let (key, word) = match k.strip_prefix('~') {
            Some(k) => (k, true),
            None => (k.as_str(), false),
        };
        let Some(have) = doc.attr(n, key) else {
            return false;
        };
        if let Some(v) = v {
            let ok = if word {
                have.split_whitespace().any(|x| x == v)
            } else {
                have == v
            };
            if !ok {
                return false;
            }
        }
    }
    if c.first_child {
        let p = doc.node(n).parent;
        let first = doc
            .children(p)
            .iter()
            .copied()
            .find(|&s| doc.name(s).is_some());
        if first != Some(n) {
            return false;
        }
    }
    true
}

/// Whether `n` matches the selector. Descendant combinators match the nearest ancestor that
/// fits (no backtracking): linear in depth × parts, whatever the markup.
fn matches(doc: &Doc, n: NodeId, parts: &[(Comb, Compound)]) -> bool {
    let Some(((comb, last), before)) = parts.split_last() else {
        return true;
    };
    let (mut comb, mut before) = (*comb, before);
    if !element_matches(doc, n, last) {
        return false;
    }
    let mut cur = n;
    while let Some(((next_comb, c), rest)) = before.split_last() {
        let mut p = doc.node(cur).parent;
        let found = match comb {
            Comb::Child => (p != 0 && element_matches(doc, p, c)).then_some(p),
            Comb::Descendant => loop {
                if p == 0 {
                    break None;
                }
                if element_matches(doc, p, c) {
                    break Some(p);
                }
                p = doc.node(p).parent;
            },
        };
        let Some(f) = found else { return false };
        cur = f;
        comb = *next_comb;
        before = rest;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cascade_and_selectors() {
        let mut s = Styles::default();
        s.add(
            "/* c */ p { color: red } .x { color: blue } div > p.x { color: green !important }
             @media screen { #a { margin: 1em } } @font-face { font-family: 'F'; src: url(f.otf) }
             p::before { content: 'x' } h1 + p { color: pink } @import url(other.css);",
            "OEBPS/",
        );
        assert_eq!(s.font_faces.len(), 1);
        assert_eq!(s.font_faces[0].src, vec!["f.otf".to_string()]);
        let d = Doc::parse(b"<div><p class='x' id='a' style='color: black'>t</p></div>");
        let p = d.find(0, "p").unwrap();
        let m = s.matching(&d, p);
        let colors: Vec<&str> = m
            .iter()
            .filter(|x| x.name == "color")
            .map(|x| x.value.as_str())
            .collect();
        assert_eq!(colors, ["red", "blue", "black", "green"]);
        assert!(m.iter().any(|x| x.name == "margin"));
    }
}
