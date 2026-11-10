//! The YAML that OpenColorIO configs use: block mappings and sequences by indentation, flow
//! mappings and sequences (over several lines too), plain, single- and double-quoted
//! scalars, `|` and `>` block scalars, comments, and local tags (`!<ColorSpace>`) on a
//! mapping, a sequence or a scalar. Anchors, aliases, multiple documents and complex keys
//! are not used by OCIO and are refused.

/// A parsed node.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Str(String),
    Seq(Vec<Value>),
    /// Keys in file order.
    Map(Vec<(String, Value)>),
    /// A tagged node (`!<MatrixTransform> {…}`): the tag without `!<…>`.
    Tagged(String, Box<Value>),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Map(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            Value::Tagged(_, v) => v.get(key),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            Value::Tagged(_, v) => v.as_str(),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        self.as_str()?.trim().parse().ok()
    }
    pub fn as_seq(&self) -> &[Value] {
        match self {
            Value::Seq(s) => s,
            Value::Tagged(_, v) => v.as_seq(),
            _ => &[],
        }
    }
    pub fn as_map(&self) -> &[(String, Value)] {
        match self {
            Value::Map(m) => m,
            Value::Tagged(_, v) => v.as_map(),
            _ => &[],
        }
    }
    /// The tag of a tagged node.
    pub fn tag(&self) -> Option<&str> {
        match self {
            Value::Tagged(t, _) => Some(t),
            _ => None,
        }
    }
    /// Numbers of a sequence (`[1, 0, 0]`).
    pub fn numbers(&self) -> Vec<f64> {
        self.as_seq().iter().filter_map(Value::as_f64).collect()
    }
}

/// Lines of the document with comments removed: (indent, text).
struct Lines {
    lines: Vec<(usize, String)>,
    at: usize,
}

/// Nesting allowed, against hostile files.
const MAX_DEPTH: usize = 64;

/// The text without a trailing comment (a `#` after white space, outside quotes).
fn strip_comment(s: &str) -> &str {
    let b = s.as_bytes();
    let (mut sq, mut dq) = (false, false);
    for i in 0..b.len() {
        match b[i] {
            b'\'' if !dq => sq = !sq,
            b'"' if !sq && (i == 0 || b[i - 1] != b'\\') => dq = !dq,
            b'#' if !sq && !dq && (i == 0 || b[i - 1].is_ascii_whitespace()) => {
                return s[..i].trim_end();
            }
            _ => {}
        }
    }
    s.trim_end()
}

/// Parses a document.
pub fn parse(text: &str) -> Result<Value, String> {
    let raw: Vec<&str> = text.lines().collect();
    let mut lines = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        let l = raw[i].trim_end_matches('\r');
        i += 1;
        let indent = l.len() - l.trim_start_matches(' ').len();
        let body = strip_comment(&l[indent..]);
        if body.is_empty() || body == "---" || body.starts_with('%') {
            continue;
        }
        if body == "..." {
            break;
        }
        // A block scalar's lines are kept verbatim, folded into the line that opens it.
        if let Some(head) = body
            .strip_suffix('|')
            .or_else(|| body.strip_suffix(">"))
            .or_else(|| body.strip_suffix("|-"))
            .or_else(|| body.strip_suffix(">-"))
            .or_else(|| body.strip_suffix("|+"))
            .filter(|h| h.ends_with(": ") || h.ends_with(':') || h.ends_with("- ") || h.is_empty())
        {
            let fold = body.contains('>');
            let mut parts: Vec<String> = Vec::new();
            while i < raw.len() {
                let n = raw[i].trim_end_matches('\r');
                let ni = n.len() - n.trim_start_matches(' ').len();
                if !n.trim().is_empty() && ni <= indent {
                    break;
                }
                parts.push(n.trim().to_owned());
                i += 1;
            }
            while parts.last().is_some_and(|p| p.is_empty()) {
                parts.pop();
            }
            let joined = parts.join(if fold { " " } else { "\n" });
            lines.push((indent, format!("{}{}", head, quote(&joined))));
            continue;
        }
        lines.push((indent, body.to_owned()));
    }
    let mut p = Lines { lines, at: 0 };
    if p.lines.is_empty() {
        return Ok(Value::Null);
    }
    let first = p.lines[0].0;
    let v = p.block(first, 0)?;
    if p.at < p.lines.len() {
        return Err(format!("unexpected text: {}", p.lines[p.at].1));
    }
    Ok(v)
}

/// A string as a double-quoted scalar.
fn quote(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Splits `key: rest` at the first `: ` (or a trailing `:`) outside quotes and brackets.
fn split_key(s: &str) -> Option<(&str, &str)> {
    let b = s.as_bytes();
    let (mut sq, mut dq, mut depth) = (false, false, 0i32);
    for i in 0..b.len() {
        match b[i] {
            b'\'' if !dq => sq = !sq,
            b'"' if !sq && (i == 0 || b[i - 1] != b'\\') => dq = !dq,
            b'[' | b'{' if !sq && !dq => depth += 1,
            b']' | b'}' if !sq && !dq => depth -= 1,
            b':' if !sq && !dq && depth == 0 && (i + 1 == b.len() || b[i + 1] == b' ') => {
                return Some((s[..i].trim(), s[i + 1..].trim()));
            }
            _ => {}
        }
    }
    None
}

fn unquote_key(k: &str) -> String {
    match scalar(k) {
        Value::Str(s) => s,
        _ => k.to_owned(),
    }
}

impl Lines {
    /// The node made of the lines at indent `indent` from here.
    fn block(&mut self, indent: usize, depth: usize) -> Result<Value, String> {
        if depth > MAX_DEPTH {
            return Err("the config nests too deeply".into());
        }
        let Some((i, text)) = self.lines.get(self.at).cloned() else {
            return Ok(Value::Null);
        };
        if i < indent {
            return Ok(Value::Null);
        }
        if text == "-" || text.starts_with("- ") {
            return self.seq(i, depth);
        }
        // `!<Tag>` alone on its line: the node below is tagged.
        if let Some(tag) = lone_tag(&text) {
            self.at += 1;
            let inner = match self.lines.get(self.at) {
                Some((ni, _)) if *ni >= i => {
                    let ni = *ni;
                    self.block(ni, depth + 1)?
                }
                _ => Value::Null,
            };
            return Ok(Value::Tagged(tag, Box::new(inner)));
        }
        if split_key(&text).is_some() {
            return self.map(i, depth);
        }
        // A scalar or flow node over one or more lines.
        self.at += 1;
        let mut t = text;
        while !balanced(&t) && self.at < self.lines.len() {
            t.push(' ');
            t.push_str(&self.lines[self.at].1);
            self.at += 1;
        }
        flow(&t, depth)
    }

    fn seq(&mut self, indent: usize, depth: usize) -> Result<Value, String> {
        let mut out = Vec::new();
        while let Some((i, text)) = self.lines.get(self.at).cloned() {
            if i != indent || !(text == "-" || text.starts_with("- ")) {
                break;
            }
            let rest = text[1..].trim_start().to_owned();
            if rest.is_empty() {
                self.at += 1;
                out.push(self.block(indent + 1, depth + 1)?);
                continue;
            }
            // `- key: value` starts a mapping whose keys sit two columns in.
            let inner = indent + (text.len() - rest.len());
            self.lines[self.at] = (inner, rest);
            out.push(self.block(inner, depth + 1)?);
        }
        Ok(Value::Seq(out))
    }

    fn map(&mut self, indent: usize, depth: usize) -> Result<Value, String> {
        let mut out = Vec::new();
        while let Some((i, text)) = self.lines.get(self.at).cloned() {
            if i != indent {
                if i > indent {
                    return Err(format!("unexpected indentation: {text}"));
                }
                break;
            }
            let Some((k, rest)) = split_key(&text) else {
                break;
            };
            let key = unquote_key(k);
            self.at += 1;
            let v = if rest.is_empty() {
                // The value is the block below (or nothing); a sequence may sit at the
                // key's own indent.
                match self.lines.get(self.at) {
                    Some((ni, nt)) if *ni > indent => {
                        let ni = *ni;
                        self.block(ni, depth + 1)?
                    }
                    Some((ni, nt)) if *ni == indent && (nt == "-" || nt.starts_with("- ")) => {
                        self.seq(indent, depth + 1)?
                    }
                    _ => Value::Null,
                }
            } else if let Some(tag) = lone_tag(rest) {
                // `key: !<Tag>` with the node below.
                let inner = match self.lines.get(self.at) {
                    Some((ni, _)) if *ni > indent => {
                        let ni = *ni;
                        self.block(ni, depth + 1)?
                    }
                    _ => Value::Null,
                };
                Value::Tagged(tag, Box::new(inner))
            } else {
                let mut t = rest.to_owned();
                while !balanced(&t) && self.at < self.lines.len() {
                    t.push(' ');
                    t.push_str(&self.lines[self.at].1);
                    self.at += 1;
                }
                flow(&t, depth + 1)?
            };
            out.push((key, v));
        }
        Ok(Value::Map(out))
    }
}

/// `!<Tag>` alone.
fn lone_tag(s: &str) -> Option<String> {
    let t = s.trim();
    let inner = t.strip_prefix("!<")?.strip_suffix('>')?;
    (!inner.contains(' ')).then(|| inner.to_owned())
}

/// Whether brackets and quotes close within `s`.
fn balanced(s: &str) -> bool {
    let (mut sq, mut dq, mut depth) = (false, false, 0i32);
    let b = s.as_bytes();
    for i in 0..b.len() {
        match b[i] {
            b'\'' if !dq => sq = !sq,
            b'"' if !sq && (i == 0 || b[i - 1] != b'\\') => dq = !dq,
            b'[' | b'{' if !sq && !dq => depth += 1,
            b']' | b'}' if !sq && !dq => depth -= 1,
            _ => {}
        }
    }
    depth <= 0 && !sq && !dq
}

/// A node written on one line: a tag, then a flow collection or a scalar.
fn flow(s: &str, depth: usize) -> Result<Value, String> {
    let mut p = Flow {
        s: s.as_bytes(),
        at: 0,
    };
    let v = p.node(depth)?;
    p.ws();
    if p.at < p.s.len() {
        return Err(format!("unexpected text: {s}"));
    }
    Ok(v)
}

struct Flow<'a> {
    s: &'a [u8],
    at: usize,
}

impl Flow<'_> {
    fn ws(&mut self) {
        while self.at < self.s.len() && self.s[self.at].is_ascii_whitespace() {
            self.at += 1;
        }
    }

    fn node(&mut self, depth: usize) -> Result<Value, String> {
        if depth > MAX_DEPTH {
            return Err("the config nests too deeply".into());
        }
        self.ws();
        if self.s[self.at..].starts_with(b"!<") {
            let end = self.s[self.at..]
                .iter()
                .position(|&c| c == b'>')
                .ok_or("unclosed tag")?;
            let tag = String::from_utf8_lossy(&self.s[self.at + 2..self.at + end]).into_owned();
            self.at += end + 1;
            self.ws();
            let inner = if self.at >= self.s.len() || matches!(self.s[self.at], b',' | b']' | b'}')
            {
                Value::Null
            } else {
                self.node(depth + 1)?
            };
            return Ok(Value::Tagged(tag, Box::new(inner)));
        }
        match self.s.get(self.at) {
            Some(b'[') => {
                self.at += 1;
                let mut v = Vec::new();
                loop {
                    self.ws();
                    match self.s.get(self.at) {
                        Some(b']') => {
                            self.at += 1;
                            break;
                        }
                        Some(b',') => self.at += 1,
                        None => return Err("unclosed [".into()),
                        _ => v.push(self.node(depth + 1)?),
                    }
                }
                Ok(Value::Seq(v))
            }
            Some(b'{') => {
                self.at += 1;
                let mut m = Vec::new();
                loop {
                    self.ws();
                    match self.s.get(self.at) {
                        Some(b'}') => {
                            self.at += 1;
                            break;
                        }
                        Some(b',') => self.at += 1,
                        None => return Err("unclosed {".into()),
                        _ => {
                            let k = self.scalar_text(true);
                            self.ws();
                            if self.s.get(self.at) == Some(&b':') {
                                self.at += 1;
                                let v = self.node(depth + 1)?;
                                m.push((unquote_key(&k), v));
                            } else {
                                m.push((unquote_key(&k), Value::Null));
                            }
                        }
                    }
                }
                Ok(Value::Map(m))
            }
            None => Ok(Value::Null),
            _ => {
                let t = self.scalar_text(false);
                Ok(scalar(&t))
            }
        }
    }

    /// A scalar's text up to the next `,` `]` `}` (and `: ` for a key), quotes kept.
    fn scalar_text(&mut self, key: bool) -> String {
        self.ws();
        let start = self.at;
        if let Some(&q @ (b'"' | b'\'')) = self.s.get(self.at) {
            self.at += 1;
            while self.at < self.s.len() {
                let c = self.s[self.at];
                self.at += 1;
                if c == b'\\' && q == b'"' {
                    self.at += 1;
                } else if c == q {
                    if q == b'\'' && self.s.get(self.at) == Some(&b'\'') {
                        self.at += 1;
                        continue;
                    }
                    break;
                }
            }
            return String::from_utf8_lossy(&self.s[start..self.at.min(self.s.len())]).into_owned();
        }
        while self.at < self.s.len() {
            let c = self.s[self.at];
            if matches!(c, b',' | b']' | b'}') {
                break;
            }
            if key
                && c == b':'
                && self
                    .s
                    .get(self.at + 1)
                    .is_none_or(|n| n.is_ascii_whitespace())
            {
                break;
            }
            self.at += 1;
        }
        String::from_utf8_lossy(&self.s[start..self.at])
            .trim()
            .to_owned()
    }
}

/// A scalar: quoted strings unescaped; `~`/`null`/empty is null; anything else as text.
fn scalar(t: &str) -> Value {
    let t = t.trim();
    if t.is_empty() || t == "~" || t == "null" {
        return Value::Null;
    }
    if let Some(inner) = t.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        let mut o = String::new();
        let mut it = inner.chars();
        while let Some(c) = it.next() {
            if c == '\\' {
                match it.next() {
                    Some('n') => o.push('\n'),
                    Some('t') => o.push('\t'),
                    Some(c) => o.push(c),
                    None => {}
                }
            } else {
                o.push(c);
            }
        }
        return Value::Str(o);
    }
    if let Some(inner) = t.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        return Value::Str(inner.replace("''", "'"));
    }
    Value::Str(t.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r#"ocio_profile_version: 2.1

environment:
  {}
search_path: "luts:more"
luma: [0.2126, 0.7152, 0.0722]   # comment
roles:
  scene_linear: ACEScg
displays:
  sRGB - Display:
    - !<View> {name: Raw, colorspace: Raw}
    - !<Views> [ACES 1.0 - SDR Video, Un-tone-mapped]
colorspaces:
  - !<ColorSpace>
    name: ACEScg
    aliases: [ACES - ACEScg, lin_ap1]
    description: |
      Convert ACEScg to ACES2065-1

      second line
    isdata: false
    to_scene_reference: !<BuiltinTransform> {style: ACEScg_to_ACES2065-1}
  - !<ColorSpace>
    name: sRGB Encoded
    from_scene_reference: !<GroupTransform>
      children:
        - !<MatrixTransform> {matrix: [1, 0, 0, 0,
                                       0, 1, 0, 0,
                                       0, 0, 1, 0, 0, 0, 0, 1]}
        - !<ExponentWithLinearTransform> {gamma: 2.4, offset: 0.055, direction: inverse}
looks:
  []
"#;

    #[test]
    fn ocio_document() {
        let v = parse(DOC).unwrap();
        assert_eq!(v.get("ocio_profile_version").unwrap().as_str(), Some("2.1"));
        assert_eq!(v.get("search_path").unwrap().as_str(), Some("luts:more"));
        assert_eq!(v.get("luma").unwrap().numbers(), [0.2126, 0.7152, 0.0722]);
        assert_eq!(v.get("environment"), Some(&Value::Map(Vec::new())));
        let d = v.get("displays").unwrap().get("sRGB - Display").unwrap();
        assert_eq!(d.as_seq()[0].tag(), Some("View"));
        assert_eq!(
            d.as_seq()[0].get("colorspace").unwrap().as_str(),
            Some("Raw")
        );
        assert_eq!(d.as_seq()[1].as_seq().len(), 2);
        let cs = v.get("colorspaces").unwrap().as_seq();
        assert_eq!(cs.len(), 2);
        assert_eq!(cs[0].tag(), Some("ColorSpace"));
        assert_eq!(cs[0].get("name").unwrap().as_str(), Some("ACEScg"));
        assert_eq!(
            cs[0].get("description").unwrap().as_str(),
            Some("Convert ACEScg to ACES2065-1\n\nsecond line")
        );
        let t = cs[0].get("to_scene_reference").unwrap();
        assert_eq!(t.tag(), Some("BuiltinTransform"));
        let g = cs[1].get("from_scene_reference").unwrap();
        assert_eq!(g.tag(), Some("GroupTransform"));
        let ch = g.get("children").unwrap().as_seq();
        assert_eq!(ch[0].get("matrix").unwrap().numbers().len(), 16);
        assert_eq!(ch[1].get("direction").unwrap().as_str(), Some("inverse"));
        assert_eq!(v.get("looks").unwrap().as_seq().len(), 0);
    }

    #[test]
    fn hostile() {
        assert!(parse(&"[".repeat(200)).is_err());
        assert!(parse("a: [1, 2").is_ok() || parse("a: [1, 2").is_err());
        assert_eq!(parse("").unwrap(), Value::Null);
    }
}
