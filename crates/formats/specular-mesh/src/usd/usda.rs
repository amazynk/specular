//! USDA, the text form of a USD layer: layer metadata, `def` / `over` / `class` prims with
//! their metadata (references, payloads, variant selections, instanceable…), attributes with
//! default values, time samples and connections, relationships, variant sets, nested prims.
//! Values are read without their declared types (numbers, tuples, arrays, strings, asset and
//! scene paths, dictionaries); numeric arrays are packed by arity.

use super::value::{Arc, Layer, Spec, SpecKind, V, child_path};
use crate::{Error, Result};

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Ident(String),
    Num(f64),
    Str(String),
    Asset(String),
    Path(String),
    Punct(u8),
    Eof,
}

struct Lexer<'a> {
    s: &'a [u8],
    i: usize,
    peeked: Option<Tok>,
    /// Open values and prim bodies ([`MAX_NESTING`]).
    depth: usize,
}

/// Values (`[`, `(`, `{`) and prims nest at most this deep; real layers stay far below.
const MAX_NESTING: usize = 128;

fn bad<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Malformed(format!("USDA: {}", msg.into())))
}

impl<'a> Lexer<'a> {
    /// One level deeper (a value or prim body opens); an error past [`MAX_NESTING`].
    fn enter(&mut self) -> Result<()> {
        self.depth += 1;
        if self.depth > MAX_NESTING {
            return bad("nested too deeply");
        }
        Ok(())
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    fn skip_space(&mut self) {
        while self.i < self.s.len() {
            let c = self.s[self.i];
            if c.is_ascii_whitespace() {
                self.i += 1;
            } else if c == b'#' {
                while self.i < self.s.len() && self.s[self.i] != b'\n' {
                    self.i += 1;
                }
            } else if c == b'/' && self.s.get(self.i + 1) == Some(&b'*') {
                self.i += 2;
                while self.i + 1 < self.s.len() && &self.s[self.i..self.i + 2] != b"*/" {
                    self.i += 1;
                }
                self.i = (self.i + 2).min(self.s.len());
            } else {
                break;
            }
        }
    }

    fn peek(&mut self) -> Result<&Tok> {
        if self.peeked.is_none() {
            let t = self.lex()?;
            self.peeked = Some(t);
        }
        Ok(self.peeked.as_ref().expect("set above"))
    }

    fn next(&mut self) -> Result<Tok> {
        match self.peeked.take() {
            Some(t) => Ok(t),
            None => self.lex(),
        }
    }

    fn lex(&mut self) -> Result<Tok> {
        self.skip_space();
        let s = self.s;
        let Some(&c) = s.get(self.i) else {
            return Ok(Tok::Eof);
        };
        let start = self.i;
        match c {
            b'"' | b'\'' => {
                let triple = s.get(self.i..self.i + 3) == Some(&[c, c, c][..]);
                let q = if triple { 3 } else { 1 };
                self.i += q;
                let mut out = Vec::new();
                loop {
                    let Some(&b) = s.get(self.i) else {
                        return bad("unterminated string");
                    };
                    if b == b'\\' {
                        let e = s.get(self.i + 1).copied().unwrap_or(b'\\');
                        out.push(match e {
                            b'n' => b'\n',
                            b't' => b'\t',
                            o => o,
                        });
                        self.i += 2;
                        continue;
                    }
                    if b == c && (!triple || s.get(self.i..self.i + 3) == Some(&[c, c, c][..])) {
                        self.i += q;
                        break;
                    }
                    if b == b'\n' && !triple {
                        return bad("newline in a string");
                    }
                    out.push(b);
                    self.i += 1;
                }
                Ok(Tok::Str(String::from_utf8_lossy(&out).into_owned()))
            }
            b'@' => {
                let triple = s.get(self.i..self.i + 3) == Some(&b"@@@"[..]);
                let q = if triple { 3 } else { 1 };
                self.i += q;
                let from = self.i;
                loop {
                    let Some(&b) = s.get(self.i) else {
                        return bad("unterminated asset path");
                    };
                    if b == b'@' && (!triple || s.get(self.i..self.i + 3) == Some(&b"@@@"[..])) {
                        break;
                    }
                    self.i += 1;
                }
                let v = String::from_utf8_lossy(&s[from..self.i]).into_owned();
                self.i += q;
                Ok(Tok::Asset(v))
            }
            b'<' => {
                self.i += 1;
                let from = self.i;
                while self.i < s.len() && s[self.i] != b'>' {
                    self.i += 1;
                }
                let v = String::from_utf8_lossy(&s[from..self.i]).into_owned();
                self.i += 1;
                Ok(Tok::Path(v))
            }
            b'-' | b'+' | b'.' | b'0'..=b'9' => {
                // "-inf" is an identifier with a sign.
                if (c == b'-' || c == b'+')
                    && s.get(self.i + 1).is_some_and(|b| b.is_ascii_alphabetic())
                {
                    self.i += 1;
                    let t = self.lex()?;
                    return Ok(match t {
                        Tok::Ident(n) if c == b'-' => Tok::Ident(format!("-{n}")),
                        t => t,
                    });
                }
                if c == b'.' && !s.get(self.i + 1).is_some_and(u8::is_ascii_digit) {
                    self.i += 1;
                    return Ok(Tok::Punct(b'.'));
                }
                self.i += 1;
                while self.i < s.len() {
                    let b = s[self.i];
                    let exp_sign = (b == b'-' || b == b'+') && matches!(s[self.i - 1], b'e' | b'E');
                    if b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E') || exp_sign {
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                let t = std::str::from_utf8(&s[start..self.i]).unwrap_or("");
                match t.parse::<f64>() {
                    Ok(v) => Ok(Tok::Num(v)),
                    Err(_) => bad(format!("number {t}")),
                }
            }
            c if c.is_ascii_alphabetic() || c == b'_' || c >= 0x80 => {
                while self.i < s.len() {
                    let b = s[self.i];
                    let dot_word =
                        b == b'.' && s.get(self.i + 1).is_some_and(|n| n.is_ascii_alphabetic());
                    if b.is_ascii_alphanumeric() || b == b'_' || b == b':' || b >= 0x80 || dot_word
                    {
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                // A trailing ':' belongs to time samples' syntax, not the name.
                while self.i > start + 1 && s[self.i - 1] == b':' {
                    self.i -= 1;
                }
                Ok(Tok::Ident(
                    String::from_utf8_lossy(&s[start..self.i]).into_owned(),
                ))
            }
            _ => {
                self.i += 1;
                Ok(Tok::Punct(c))
            }
        }
    }

    fn expect(&mut self, p: u8) -> Result<()> {
        match self.next()? {
            Tok::Punct(c) if c == p => Ok(()),
            t => bad(format!("expected '{}', found {t:?}", p as char)),
        }
    }

    fn eat(&mut self, p: u8) -> Result<bool> {
        if *self.peek()? == Tok::Punct(p) {
            self.next()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// Parses a USDA layer.
pub(crate) fn parse(bytes: &[u8]) -> Result<Layer> {
    if !bytes.starts_with(b"#usda") {
        return bad("no #usda header");
    }
    let mut lx = Lexer {
        s: bytes,
        i: 0,
        peeked: None,
        depth: 0,
    };
    // Skip the header line.
    while lx.i < bytes.len() && bytes[lx.i] != b'\n' {
        lx.i += 1;
    }
    let mut layer = Layer::default();
    layer.spec_mut("/", SpecKind::PseudoRoot);
    if *lx.peek()? == Tok::Punct(b'(') {
        lx.next()?;
        let mut root = Spec::new(SpecKind::PseudoRoot);
        metadata(&mut lx, &mut root)?;
        layer.specs.insert("/".into(), root);
    }
    let mut children = Vec::new();
    prims_until(&mut lx, &mut layer, "/", &mut children, None)?;
    layer
        .spec_mut("/", SpecKind::PseudoRoot)
        .set("primChildren", V::Strs(children));
    Ok(layer)
}

/// Reads prim and property statements until `end` (or the end of the file), under `parent`.
fn prims_until(
    lx: &mut Lexer,
    layer: &mut Layer,
    parent: &str,
    children: &mut Vec<String>,
    end: Option<u8>,
) -> Result<()> {
    lx.enter()?;
    let r = prims_body(lx, layer, parent, children, end);
    lx.leave();
    r
}

fn prims_body(
    lx: &mut Lexer,
    layer: &mut Layer,
    parent: &str,
    children: &mut Vec<String>,
    end: Option<u8>,
) -> Result<()> {
    let mut properties = Vec::new();
    let mut variant_sets = Vec::new();
    loop {
        match lx.peek()?.clone() {
            Tok::Eof if end.is_none() => break,
            Tok::Eof => return bad("unexpected end of file"),
            Tok::Punct(c) if Some(c) == end => {
                lx.next()?;
                break;
            }
            Tok::Punct(b';') => {
                lx.next()?;
            }
            Tok::Ident(w) if matches!(w.as_str(), "def" | "over" | "class") => {
                lx.next()?;
                let (type_name, name) = match lx.next()? {
                    Tok::Str(n) => (String::new(), n),
                    Tok::Ident(t) => match lx.next()? {
                        Tok::Str(n) => (t, n),
                        o => return bad(format!("prim name, found {o:?}")),
                    },
                    o => return bad(format!("prim name, found {o:?}")),
                };
                let path = child_path(parent, &name);
                let mut spec = Spec::new(SpecKind::Prim);
                spec.set("specifier", V::Ident(w));
                if !type_name.is_empty() {
                    spec.set("typeName", V::Str(type_name));
                }
                if lx.eat(b'(')? {
                    metadata(lx, &mut spec)?;
                }
                lx.expect(b'{')?;
                layer.specs.insert(path.clone(), spec);
                let mut sub = Vec::new();
                prims_until(lx, layer, &path, &mut sub, Some(b'}'))?;
                let s = layer.spec_mut(&path, SpecKind::Prim);
                s.set("primChildren", V::Strs(sub));
                if !children.contains(&name) {
                    children.push(name);
                }
            }
            Tok::Ident(w) if w == "variantSet" => {
                lx.next()?;
                let Tok::Str(set) = lx.next()? else {
                    return bad("variant set name");
                };
                lx.expect(b'=')?;
                lx.expect(b'{')?;
                let mut names = Vec::new();
                loop {
                    match lx.next()? {
                        Tok::Punct(b'}') => break,
                        Tok::Str(v) => {
                            let vpath = format!("{parent}{{{set}={v}}}");
                            let mut spec = Spec::new(SpecKind::Variant);
                            if lx.eat(b'(')? {
                                metadata(lx, &mut spec)?;
                            }
                            lx.expect(b'{')?;
                            layer.specs.insert(vpath.clone(), spec);
                            let mut sub = Vec::new();
                            prims_until(lx, layer, &vpath, &mut sub, Some(b'}'))?;
                            layer
                                .spec_mut(&vpath, SpecKind::Variant)
                                .set("primChildren", V::Strs(sub));
                            names.push(v);
                        }
                        o => return bad(format!("variant, found {o:?}")),
                    }
                }
                layer
                    .spec_mut(&format!("{parent}{{{set}=}}"), SpecKind::VariantSet)
                    .set("variantChildren", V::Strs(names));
                variant_sets.push(set);
            }
            Tok::Ident(_) => {
                property(lx, layer, parent, &mut properties)?;
            }
            t => return bad(format!("unexpected {t:?}")),
        }
    }
    if !variant_sets.is_empty() {
        let s = layer.spec_mut(parent, SpecKind::Prim);
        // Sets declared in the body join those of the metadata.
        let mut all = s
            .get("variantSetNames")
            .and_then(V::strs)
            .unwrap_or_default();
        for v in variant_sets {
            if !all.contains(&v) {
                all.push(v);
            }
        }
        s.set("variantSetNames", V::Strs(all));
    }
    Ok(())
}

/// A property statement: `[custom] [uniform|varying] type[[]] name [= value] [(meta)]`,
/// `name.connect = …`, `name.timeSamples = {…}`, `rel name [= targets]`.
fn property(lx: &mut Lexer, layer: &mut Layer, prim: &str, props: &mut Vec<String>) -> Result<()> {
    let mut words = Vec::new();
    let mut is_rel = false;
    while let Tok::Ident(w) = lx.peek()?.clone() {
        lx.next()?;
        if w == "rel" {
            is_rel = true;
        }
        // `[]` after a type name.
        if *lx.peek()? == Tok::Punct(b'[') {
            lx.next()?;
            lx.expect(b']')?;
        }
        words.push(w);
    }
    let Some(full) = words.pop() else {
        return bad("property without a name");
    };
    let (name, suffix) = match full.rsplit_once('.') {
        Some((n, s @ ("connect" | "timeSamples"))) => (n.to_owned(), Some(s.to_owned())),
        _ => (full, None),
    };
    let path = format!("{prim}.{name}");
    let kind = if is_rel {
        SpecKind::Relationship
    } else {
        SpecKind::Attribute
    };
    let mut value = None;
    if lx.eat(b'=')? {
        value = Some(match suffix.as_deref() {
            Some("timeSamples") => time_samples(lx)?,
            _ => self::value(lx)?,
        });
    }
    let spec = layer.spec_mut(&path, kind);
    match (suffix.as_deref(), value) {
        (Some("connect"), Some(v)) => spec.set("connectionPaths", V::Paths(v.paths())),
        (Some("timeSamples"), Some(v)) => spec.set("timeSamples", v),
        (_, Some(v)) if is_rel => spec.set("targetPaths", V::Paths(v.paths())),
        (_, Some(v)) => spec.set("default", v),
        _ => {}
    }
    if words.contains(&"uniform".to_owned()) {
        spec.set("variability", V::Ident("uniform".into()));
    }
    if lx.eat(b'(')? {
        let mut m = Spec::new(kind);
        metadata(lx, &mut m)?;
        let spec = layer.spec_mut(&path, kind);
        for (k, v) in m.fields {
            spec.set(&k, v);
        }
    }
    if !props.contains(&name) {
        props.push(name);
    }
    Ok(())
}

fn time_samples(lx: &mut Lexer) -> Result<V> {
    lx.expect(b'{')?;
    let mut out = Vec::new();
    loop {
        match lx.next()? {
            Tok::Punct(b'}') => break,
            Tok::Punct(b',') => {}
            Tok::Num(t) => {
                lx.expect(b':')?;
                out.push((t, value(lx)?));
            }
            o => return bad(format!("time sample, found {o:?}")),
        }
    }
    Ok(V::Samples(out))
}

/// A metadata block's entries up to its `)`: `key = value`, list-op forms (`prepend
/// references = …`), and bare doc strings.
fn metadata(lx: &mut Lexer, spec: &mut Spec) -> Result<()> {
    loop {
        match lx.next()? {
            Tok::Punct(b')') => return Ok(()),
            Tok::Punct(b';') => {}
            Tok::Str(doc) => spec.set("documentation", V::Str(doc)),
            Tok::Ident(mut key) => {
                let mut op = None;
                if matches!(
                    key.as_str(),
                    "prepend" | "append" | "add" | "delete" | "reorder"
                ) {
                    op = Some(key.clone());
                    match lx.next()? {
                        Tok::Ident(k) => key = k,
                        o => return bad(format!("metadata key, found {o:?}")),
                    }
                }
                // Typed dictionary entries (`dictionary customData = {…}`).
                if let Tok::Ident(k) = lx.peek()?.clone() {
                    lx.next()?;
                    key = k;
                }
                lx.expect(b'=')?;
                let v = match key.as_str() {
                    "references" | "payload" => arcs(lx)?,
                    "variants" => value(lx)?,
                    _ => value(lx)?,
                };
                if op.as_deref() == Some("delete") {
                    continue;
                }
                let field = match key.as_str() {
                    "variants" => "variantSelection",
                    "variantSets" => "variantSetNames",
                    "subLayers" => "subLayers",
                    "doc" => "documentation",
                    k => k,
                };
                // Prepended and appended arcs join what is there.
                if let (Some(V::Arcs(old)), V::Arcs(new)) = (spec.get(field).cloned(), &v)
                    && op.is_some()
                {
                    let mut all = new.clone();
                    all.extend(old);
                    spec.set(field, V::Arcs(all));
                    continue;
                }
                spec.set(field, v);
            }
            o => return bad(format!("metadata, found {o:?}")),
        }
    }
}

/// References or payloads: `@a.usd@</Prim>`, `</Prim>`, `None`, or a list of them, each
/// optionally with its own `( offset = … )`.
fn arcs(lx: &mut Lexer) -> Result<V> {
    let one = |lx: &mut Lexer| -> Result<Option<Arc>> {
        let mut a = Arc::default();
        match lx.next()? {
            Tok::Asset(s) => {
                a.asset = s;
                if let Tok::Path(p) = lx.peek()?.clone() {
                    lx.next()?;
                    a.prim = p;
                }
            }
            Tok::Path(p) => a.prim = p,
            Tok::Ident(n) if n == "None" => return Ok(None),
            o => return bad(format!("reference, found {o:?}")),
        }
        if lx.eat(b'(')? {
            let mut skip = Spec::new(SpecKind::Other);
            metadata(lx, &mut skip)?;
        }
        Ok(Some(a))
    };
    let mut out = Vec::new();
    if lx.eat(b'[')? {
        loop {
            if lx.eat(b']')? {
                break;
            }
            if lx.eat(b',')? {
                continue;
            }
            out.extend(one(lx)?);
        }
    } else {
        out.extend(one(lx)?);
    }
    Ok(V::Arcs(out))
}

/// A value: number, string, asset, path, identifier, `(tuple)`, `[list]`, `{dictionary}`.
fn value(lx: &mut Lexer) -> Result<V> {
    lx.enter()?;
    let v = value_body(lx);
    lx.leave();
    v
}

fn value_body(lx: &mut Lexer) -> Result<V> {
    Ok(match lx.next()? {
        Tok::Num(v) => V::Num(v),
        Tok::Str(s) => V::Str(s),
        Tok::Asset(s) => V::Asset(s),
        Tok::Path(s) => V::Path(s),
        Tok::Ident(s) => match s.as_str() {
            "true" => V::Bool(true),
            "false" => V::Bool(false),
            "None" => V::None,
            _ => V::Ident(s),
        },
        Tok::Punct(b'(') => pack(seq(lx, b')')?),
        Tok::Punct(b'[') => pack(seq(lx, b']')?),
        Tok::Punct(b'{') => {
            let mut d = Vec::new();
            loop {
                match lx.next()? {
                    Tok::Punct(b'}') => break,
                    Tok::Punct(b',' | b';') => {}
                    // `type key = value` or `"key": value` (time-sample style).
                    Tok::Ident(t) => {
                        let key = match lx.next()? {
                            Tok::Ident(k) | Tok::Str(k) => k,
                            o => return bad(format!("dictionary key after {t}, found {o:?}")),
                        };
                        lx.expect(b'=')?;
                        d.push((key, value(lx)?));
                    }
                    Tok::Str(k) => {
                        if !lx.eat(b'=')? {
                            lx.expect(b':')?;
                        }
                        d.push((k, value(lx)?));
                    }
                    o => return bad(format!("dictionary, found {o:?}")),
                }
            }
            V::Dict(d)
        }
        o => return bad(format!("value, found {o:?}")),
    })
}

fn seq(lx: &mut Lexer, close: u8) -> Result<Vec<V>> {
    let mut out = Vec::new();
    loop {
        if lx.eat(close)? {
            return Ok(out);
        }
        if lx.eat(b',')? {
            continue;
        }
        out.push(value(lx)?);
    }
}

/// Packs a list of numbers or of equal-length number tuples into [`V::Nums`], a list of
/// strings into [`V::Strs`], a list of paths into [`V::Paths`].
fn pack(items: Vec<V>) -> V {
    if items.is_empty() {
        return V::Nums {
            data: Vec::new(),
            arity: 1,
        };
    }
    let num = |v: &V| v.as_f64().filter(|_| matches!(v, V::Num(_) | V::Ident(_)));
    if items.iter().all(|v| num(v).is_some()) {
        return V::Nums {
            data: items.iter().filter_map(num).collect(),
            arity: 1,
        };
    }
    if let V::Nums { arity: 1, data } = &items[0] {
        let a = data.len();
        if items
            .iter()
            .all(|v| matches!(v, V::Nums { arity: 1, data } if data.len() == a))
        {
            let mut out = Vec::with_capacity(a * items.len());
            for v in &items {
                if let V::Nums { data, .. } = v {
                    out.extend_from_slice(data);
                }
            }
            return V::Nums {
                data: out,
                arity: a.max(1),
            };
        }
    }
    if items.iter().all(|v| matches!(v, V::Str(_))) {
        return V::Strs(
            items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect(),
        );
    }
    if items.iter().all(|v| matches!(v, V::Path(_))) {
        return V::Paths(items.iter().flat_map(V::paths).collect());
    }
    V::List(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parses on a small stack: nesting past the cap must fail, not overflow.
    fn parse_small(text: &[u8]) -> Result<Layer> {
        specular_thread::on_stack(256 * 1024, || parse(text))
    }

    #[test]
    fn deep_values_are_refused() {
        // `[[[[…` 100 000 deep, then `(((…` and `{{{…` (one byte a level).
        for open in ["[", "(", "{double a = "] {
            let mut t = b"#usda 1.0\ndef \"A\" {\n  double[] x = ".to_vec();
            t.extend(open.repeat(100_000).bytes());
            let Err(e) = parse_small(&t) else {
                panic!("accepted")
            };
            assert!(e.to_string().contains("nested too deeply"), "{e}");
        }
        // 100 levels still read.
        let mut t = b"#usda 1.0\ndef \"A\" {\n  custom double x = ".to_vec();
        t.extend("[".repeat(100).bytes());
        t.extend("]".repeat(100).bytes());
        t.extend(b"\n}\n");
        parse_small(&t).unwrap();
    }

    #[test]
    fn deep_prims_are_refused() {
        let mut t = b"#usda 1.0\n".to_vec();
        t.extend("def \"a\" {".repeat(100_000).bytes());
        let Err(e) = parse_small(&t) else {
            panic!("accepted")
        };
        assert!(e.to_string().contains("nested too deeply"), "{e}");
        // Variant bodies count too.
        let mut t = b"#usda 1.0\n".to_vec();
        t.extend(
            "def \"a\" { variantSet \"v\" = { \"x\" {"
                .repeat(50_000)
                .bytes(),
        );
        let Err(e) = parse_small(&t) else {
            panic!("accepted")
        };
        assert!(e.to_string().contains("nested too deeply"), "{e}");
    }

    #[test]
    fn a_small_layer() {
        let text = br#"#usda 1.0
(
    defaultPrim = "World"
    metersPerUnit = 0.01
    upAxis = "Z"
    doc = """Made by hand"""
)

def Xform "World" (
    kind = "assembly"
    prepend references = @./other.usda@</Thing>
    variants = {
        string shape = "Box"
    }
    prepend variantSets = "shape"
)
{
    float3 xformOp:translate = (1, 2, -3.5e-1)
    uniform token[] xformOpOrder = ["xformOp:translate"]

    def Mesh "M" {
        point3f[] points = [(0,0,0), (1,0,0), (0,1,0)]
        int[] faceVertexCounts = [3]
        int[] faceVertexIndices = [0, 1, 2]
        texCoord2f[] primvars:st = [(0, 0), (1, 0), (0, 1)] (
            interpolation = "faceVarying"
        )
        rel material:binding = </World/Mat>
        color3f[] primvars:displayColor.timeSamples = {
            0: [(1, 0, 0)],
            10: [(0, 1, 0)],
        }
    }
    variantSet "shape" = {
        "Box" {
            def Cube "C" { double size = 2 }
        }
        "Ball" (doc = "x") {
            def Sphere "S" { }
        }
    }
    def Shader "T" {
        asset inputs:file = @textures/a b.png@
        float2 inputs:st.connect = </World/T.outputs:result>
        float inputs:scale = -inf
    }
}
"#;
        let l = parse(text).unwrap();
        assert_eq!(l.meta("upAxis").and_then(V::as_str), Some("Z"));
        assert_eq!(l.meta("metersPerUnit").and_then(V::as_f64), Some(0.01));
        let w = &l.specs["/World"];
        assert_eq!(w.get("typeName").and_then(V::as_str), Some("Xform"));
        assert_eq!(
            w.get("references"),
            Some(&V::Arcs(vec![Arc {
                asset: "./other.usda".into(),
                prim: "/Thing".into()
            }]))
        );
        assert_eq!(
            w.get("variantSelection").and_then(|d| d.dict_get("shape")),
            Some(&V::Str("Box".into()))
        );
        assert_eq!(
            w.get("primChildren"),
            Some(&V::Strs(vec!["M".into(), "T".into()]))
        );
        let t = &l.specs["/World.xformOp:translate"];
        assert_eq!(
            t.get("default"),
            Some(&V::Nums {
                data: vec![1.0, 2.0, -0.35],
                arity: 1
            })
        );
        let p = &l.specs["/World/M.points"];
        assert!(matches!(p.get("default"), Some(V::Nums { arity: 3, data }) if data.len() == 9));
        let st = &l.specs["/World/M.primvars:st"];
        assert_eq!(
            st.get("interpolation").and_then(V::as_str),
            Some("faceVarying")
        );
        let rel = &l.specs["/World/M.material:binding"];
        assert_eq!(
            rel.get("targetPaths"),
            Some(&V::Paths(vec!["/World/Mat".into()]))
        );
        let c = &l.specs["/World/M.primvars:displayColor"];
        assert!(matches!(c.get("timeSamples"), Some(V::Samples(s)) if s.len() == 2));
        assert!(l.specs.contains_key("/World{shape=Box}C"));
        assert_eq!(
            l.specs["/World{shape=Box}C.size"].get("default"),
            Some(&V::Num(2.0))
        );
        assert!(l.specs.contains_key("/World{shape=Ball}S"));
        let f = &l.specs["/World/T.inputs:file"];
        assert_eq!(f.get("default"), Some(&V::Asset("textures/a b.png".into())));
        let st = &l.specs["/World/T.inputs:st"];
        assert_eq!(
            st.get("connectionPaths"),
            Some(&V::Paths(vec!["/World/T.outputs:result".into()]))
        );
        assert_eq!(
            l.specs["/World/T.inputs:scale"]
                .get("default")
                .and_then(V::as_f64),
            Some(f64::NEG_INFINITY)
        );
    }

    #[test]
    fn junk_is_an_error() {
        for t in [
            &b"#usda 1.0\ndef \"A\" {"[..],
            b"#usda 1.0\n( x = ",
            b"#usda 1.0\ndef X \"A\" { float a = (1, }",
            b"not usda",
        ] {
            assert!(parse(t).is_err());
        }
    }
}
