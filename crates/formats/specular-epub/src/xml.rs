//! A lenient XML/XHTML reader: the package files (container, OPF, NCX, nav) and the pages.
//!
//! Books are meant to be well-formed XML but often are not quite: HTML entities without a
//! DTD, unclosed `<p>` and `<li>`, stray end tags, Latin-1 text. Nothing here fails: an end
//! tag closes the innermost open element of its name (or is ignored), a `<p>`, `<li>`,
//! `<td>`… closes an open one of its kind, void elements (`<br>`, `<img>`…) never take
//! children, and unknown entities stay as written. Element and attribute names are kept
//! lower-cased with their prefix (`dc:title`); [`Doc::local`] drops the prefix.

/// Deepest element nesting kept; deeper elements are attached to the deepest one.
const MAX_DEPTH: usize = 256;
/// Most nodes a document may have; the rest of the text is dropped.
const MAX_NODES: usize = 1 << 21;

pub type NodeId = u32;

#[derive(Debug, Clone)]
pub enum Kind {
    /// The document itself (node 0).
    Root,
    Element {
        /// Lower-cased, with any prefix (`dc:title`, `svg`).
        name: String,
        attrs: Vec<(String, String)>,
    },
    Text(String),
}

#[derive(Debug, Clone)]
pub struct Node {
    pub kind: Kind,
    pub parent: NodeId,
    pub children: Vec<NodeId>,
}

/// A parsed document: node 0 is the root.
#[derive(Debug, Clone)]
pub struct Doc {
    pub nodes: Vec<Node>,
}

/// The part of a name after its prefix.
pub fn local_of(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr", "basefont", "frame", "keygen",
];

impl Doc {
    /// An (X)HTML document: void elements and implied end tags as in HTML.
    pub fn parse(bytes: &[u8]) -> Doc {
        parse_str(&decode_text(bytes), true)
    }

    /// A package file (OPF, NCX, container, encryption): plain XML, where `<meta>` has
    /// content.
    pub fn parse_xml(bytes: &[u8]) -> Doc {
        parse_str(&decode_text(bytes), false)
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        &self.nodes[id as usize].children
    }

    /// The element's full lower-cased name; `None` for text and the root.
    pub fn name(&self, id: NodeId) -> Option<&str> {
        match &self.nodes[id as usize].kind {
            Kind::Element { name, .. } => Some(name),
            _ => None,
        }
    }

    /// The element's name without its prefix.
    pub fn local(&self, id: NodeId) -> Option<&str> {
        self.name(id).map(local_of)
    }

    pub fn is(&self, id: NodeId, local: &str) -> bool {
        self.local(id) == Some(local)
    }

    pub fn attrs(&self, id: NodeId) -> &[(String, String)] {
        match &self.nodes[id as usize].kind {
            Kind::Element { attrs, .. } => attrs,
            _ => &[],
        }
    }

    /// The attribute named exactly `name` (lower case, with prefix), else the first whose
    /// name without prefix is `name`.
    pub fn attr(&self, id: NodeId, name: &str) -> Option<&str> {
        let a = self.attrs(id);
        a.iter()
            .find(|(k, _)| k == name)
            .or_else(|| a.iter().find(|(k, _)| local_of(k) == name))
            .map(|(_, v)| v.as_str())
    }

    /// The attribute named exactly `name`.
    pub fn attr_exact(&self, id: NodeId, name: &str) -> Option<&str> {
        self.attrs(id)
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// Element children with the local name `local`.
    pub fn elements<'a>(&'a self, id: NodeId, local: &'a str) -> impl Iterator<Item = NodeId> + 'a {
        self.children(id)
            .iter()
            .copied()
            .filter(move |&c| self.is(c, local))
    }

    pub fn first(&self, id: NodeId, local: &str) -> Option<NodeId> {
        self.elements(id, local).next()
    }

    /// Every element below `id` (depth first, document order) with the local name `local`.
    pub fn descendants(&self, id: NodeId, local: &str) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack: Vec<NodeId> = self.children(id).iter().rev().copied().collect();
        while let Some(n) = stack.pop() {
            if self.is(n, local) {
                out.push(n);
            }
            stack.extend(self.children(n).iter().rev().copied());
        }
        out
    }

    pub fn find(&self, id: NodeId, local: &str) -> Option<NodeId> {
        let mut stack: Vec<NodeId> = self.children(id).iter().rev().copied().collect();
        while let Some(n) = stack.pop() {
            if self.is(n, local) {
                return Some(n);
            }
            stack.extend(self.children(n).iter().rev().copied());
        }
        None
    }

    /// The text of `id` and everything below it, white space collapsed and trimmed.
    pub fn text(&self, id: NodeId) -> String {
        let mut raw = String::new();
        let mut stack = vec![id];
        while let Some(n) = stack.pop() {
            match &self.nodes[n as usize].kind {
                Kind::Text(t) => raw.push_str(t),
                Kind::Element { name, .. } if matches!(local_of(name), "script" | "style") => {}
                _ => stack.extend(self.children(n).iter().rev().copied()),
            }
        }
        collapse_ws(&raw)
    }
}

/// White space runs to one space, trimmed.
pub fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for w in s.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(w);
    }
    out
}

/// Bytes to text: UTF-8 (BOM or not), UTF-16 with a BOM or by its zero bytes, else the
/// encoding the XML declaration names when it is Latin-1 / Windows-1252; invalid UTF-8 is
/// read as Windows-1252.
pub fn decode_text(b: &[u8]) -> String {
    if let Some(rest) = b.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    let utf16 = |b: &[u8], le: bool| {
        let units: Vec<u16> = b
            .chunks_exact(2)
            .map(|c| {
                if le {
                    u16::from_le_bytes([c[0], c[1]])
                } else {
                    u16::from_be_bytes([c[0], c[1]])
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    if let Some(rest) = b.strip_prefix(&[0xFF, 0xFE]) {
        return utf16(rest, true);
    }
    if let Some(rest) = b.strip_prefix(&[0xFE, 0xFF]) {
        return utf16(rest, false);
    }
    if b.len() >= 4 && b[0] == b'<' && b[1] == 0 {
        return utf16(b, true);
    }
    if b.len() >= 4 && b[0] == 0 && b[1] == b'<' {
        return utf16(b, false);
    }
    let head = &b[..b.len().min(200)];
    let head_s = String::from_utf8_lossy(head).to_ascii_lowercase();
    let declared_latin = head_s.starts_with("<?xml")
        && [
            "iso-8859-1",
            "latin1",
            "windows-1252",
            "cp1252",
            "iso-8859-15",
        ]
        .iter()
        .any(|e| head_s.contains(e));
    match std::str::from_utf8(b) {
        Ok(s) if !declared_latin => s.to_owned(),
        _ => b.iter().map(|&c| cp1252(c)).collect(),
    }
}

pub(crate) fn cp1252(c: u8) -> char {
    const HIGH: [u16; 32] = [
        0x20AC, 0x81, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160,
        0x2039, 0x0152, 0x8D, 0x017D, 0x8F, 0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013,
        0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x9D, 0x017E, 0x0178,
    ];
    if (0x80..0xA0).contains(&c) {
        char::from_u32(u32::from(HIGH[(c - 0x80) as usize])).unwrap_or('\u{FFFD}')
    } else {
        char::from(c)
    }
}

/// `<p>` and friends that an opening tag of the same group closes, and the elements that
/// bound the search.
fn closes(open: &str, new: &str) -> bool {
    match new {
        "p" | "div" | "ul" | "ol" | "table" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
        | "blockquote" | "pre" | "dl" | "hr" | "section" | "figure" => open == "p",
        "li" => open == "li",
        "dt" | "dd" => open == "dt" || open == "dd",
        "tr" => matches!(open, "tr" | "td" | "th"),
        "td" | "th" => matches!(open, "td" | "th"),
        "tbody" | "thead" | "tfoot" => {
            matches!(open, "tbody" | "thead" | "tfoot" | "tr" | "td" | "th")
        }
        _ => false,
    }
}

/// Elements an auto-close search does not cross.
fn bounds(open: &str, new: &str) -> bool {
    match new {
        "li" => matches!(open, "ul" | "ol" | "menu"),
        "dt" | "dd" => open == "dl",
        "tr" | "td" | "th" | "tbody" | "thead" | "tfoot" => open == "table",
        _ => matches!(
            open,
            "div"
                | "blockquote"
                | "li"
                | "td"
                | "th"
                | "body"
                | "section"
                | "article"
                | "aside"
                | "figure"
                | "dd"
        ),
    }
}

fn parse_str(s: &str, html: bool) -> Doc {
    let mut nodes = vec![Node {
        kind: Kind::Root,
        parent: 0,
        children: Vec::new(),
    }];
    // Open elements (node, lower-cased local name).
    let mut stack: Vec<(NodeId, String)> = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    let top = |stack: &Vec<(NodeId, String)>| stack.last().map_or(0, |t| t.0);
    let push = |nodes: &mut Vec<Node>, parent: NodeId, kind: Kind| -> Option<NodeId> {
        if nodes.len() >= MAX_NODES {
            return None;
        }
        let id = nodes.len() as NodeId;
        nodes.push(Node {
            kind,
            parent,
            children: Vec::new(),
        });
        nodes[parent as usize].children.push(id);
        Some(id)
    };
    while i < b.len() {
        let Some(lt) = memchr(b'<', &b[i..]).map(|o| i + o) else {
            let t = decode_entities(&s[i..]);
            push(&mut nodes, top(&stack), Kind::Text(t));
            break;
        };
        if lt > i {
            let t = decode_entities(&s[i..lt]);
            if push(&mut nodes, top(&stack), Kind::Text(t)).is_none() {
                break;
            }
        }
        let rest = &s[lt..];
        if let Some(body) = rest.strip_prefix("<!--") {
            i = body.find("-->").map_or(b.len(), |e| lt + 4 + e + 3);
            continue;
        }
        if let Some(body) = rest.strip_prefix("<![CDATA[") {
            let end = body.find("]]>").map_or(rest.len(), |e| 9 + e);
            push(&mut nodes, top(&stack), Kind::Text(rest[9..end].to_owned()));
            i = (lt + end + 3).min(b.len());
            continue;
        }
        if rest.starts_with("<!") {
            // A DOCTYPE, maybe with an internal subset.
            let gt = rest.find('>').unwrap_or(rest.len());
            let sub = rest.find('[').filter(|&o| o < gt);
            i = match sub {
                Some(_) => rest.find("]>").map_or(b.len(), |e| lt + e + 2),
                None => lt + gt + 1,
            };
            continue;
        }
        if rest.starts_with("<?") {
            i = rest.find("?>").map_or(b.len(), |e| lt + e + 2);
            continue;
        }
        if let Some(r) = rest.strip_prefix("</") {
            let gt = r.find('>').unwrap_or(r.len());
            let name = r[..gt].trim().to_ascii_lowercase();
            let local = local_of(&name).to_owned();
            if let Some(pos) = stack.iter().rposition(|(_, n)| *n == local) {
                stack.truncate(pos);
            }
            i = (lt + 2 + gt + 1).min(b.len());
            continue;
        }
        // A start tag.
        let Some((name, attrs, self_closing, end)) = start_tag(rest) else {
            // A lone '<': text.
            push(&mut nodes, top(&stack), Kind::Text("<".into()));
            i = lt + 1;
            continue;
        };
        i = lt + end;
        let local = local_of(&name).to_owned();
        // Implied end tags.
        if html
            && let Some(pos) = stack
                .iter()
                .rposition(|(_, n)| closes(n, &local) || bounds(n, &local))
            && closes(&stack[pos].1, &local)
        {
            stack.truncate(pos);
        }
        let parent = top(&stack);
        let Some(id) = push(&mut nodes, parent, Kind::Element { name, attrs }) else {
            break;
        };
        if matches!(local.as_str(), "script" | "style") && !self_closing {
            // Raw text up to the end tag.
            let tail = &s[i..];
            let mut end = tail.len();
            let mut from = 0;
            while let Some(o) = tail[from..].find("</") {
                let at = from + o + 2;
                let cand = tail.as_bytes().get(at..at + local.len());
                if cand.is_some_and(|c| c.eq_ignore_ascii_case(local.as_bytes())) {
                    end = from + o;
                    break;
                }
                from = at;
            }
            if local == "style" {
                push(&mut nodes, id, Kind::Text(s[i..i + end].to_owned()));
            }
            i += end;
            if i < b.len() {
                i = s[i..].find('>').map_or(b.len(), |g| i + g + 1);
            }
            continue;
        }
        if !self_closing && !(html && VOID.contains(&local.as_str())) && stack.len() < MAX_DEPTH {
            stack.push((id, local));
        }
    }
    Doc { nodes }
}

fn memchr(c: u8, b: &[u8]) -> Option<usize> {
    b.iter().position(|&x| x == c)
}

/// Parses `<name attr="v" …>` at the start of `s`: name, attributes, whether it ends `/>`,
/// and its length.
/// A start tag: name, attributes, whether it closes itself, length.
type StartTag = (String, Vec<(String, String)>, bool, usize);

fn start_tag(s: &str) -> Option<StartTag> {
    let b = s.as_bytes();
    let mut i = 1;
    let name_end = s[i..]
        .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .map_or(s.len(), |o| i + o);
    if name_end == i || !b[i].is_ascii_alphabetic() && b[i] != b'_' && b[i] < 0x80 {
        return None;
    }
    let name = s[i..name_end].to_ascii_lowercase();
    i = name_end;
    let mut attrs = Vec::new();
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            return Some((name, attrs, false, b.len()));
        }
        if b[i] == b'>' {
            return Some((name, attrs, false, i + 1));
        }
        if b[i] == b'/' {
            if b.get(i + 1) == Some(&b'>') {
                return Some((name, attrs, true, i + 2));
            }
            i += 1;
            continue;
        }
        let ks = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && !matches!(b[i], b'=' | b'>' | b'/') {
            i += 1;
        }
        let key = s[ks..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                let vs = i + 1;
                let ve = memchr(q, &b[vs..]).map_or(b.len(), |o| vs + o);
                value = decode_entities(&s[vs..ve]);
                i = (ve + 1).min(b.len());
            } else {
                let vs = i;
                while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' {
                    i += 1;
                }
                value = decode_entities(&s[vs..i]);
            }
        }
        if !key.is_empty() && attrs.len() < 256 {
            attrs.push((key, value));
        }
        if ks == i {
            i += 1;
        }
    }
}

/// Replaces character references and the XML and common HTML named entities.
pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(&rest[..p]);
        rest = &rest[p..];
        let end = rest[1..]
            .find(|c: char| c == ';' || c == '&' || c.is_whitespace() || c == '<')
            .map(|o| o + 1);
        match end {
            Some(e) if rest.as_bytes()[e] == b';' && e <= 32 => {
                let name = &rest[1..e];
                if let Some(c) = entity(name) {
                    out.push(c);
                    rest = &rest[e + 1..];
                    continue;
                }
                out.push('&');
                rest = &rest[1..];
            }
            _ => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<char> {
    if let Some(n) = name.strip_prefix('#') {
        let v = if let Some(h) = n.strip_prefix('x').or_else(|| n.strip_prefix('X')) {
            u32::from_str_radix(h, 16).ok()?
        } else {
            n.parse::<u32>().ok()?
        };
        return Some(
            char::from_u32(v)
                .filter(|&c| c != '\0')
                .unwrap_or('\u{FFFD}'),
        );
    }
    let c: u32 = match name {
        "amp" => 0x26,
        "lt" => 0x3C,
        "gt" => 0x3E,
        "quot" => 0x22,
        "apos" => 0x27,
        "nbsp" => 0xA0,
        "iexcl" => 0xA1,
        "cent" => 0xA2,
        "pound" => 0xA3,
        "curren" => 0xA4,
        "yen" => 0xA5,
        "brvbar" => 0xA6,
        "sect" => 0xA7,
        "uml" => 0xA8,
        "copy" => 0xA9,
        "ordf" => 0xAA,
        "laquo" => 0xAB,
        "not" => 0xAC,
        "shy" => 0xAD,
        "reg" => 0xAE,
        "macr" => 0xAF,
        "deg" => 0xB0,
        "plusmn" => 0xB1,
        "sup2" => 0xB2,
        "sup3" => 0xB3,
        "acute" => 0xB4,
        "micro" => 0xB5,
        "para" => 0xB6,
        "middot" => 0xB7,
        "cedil" => 0xB8,
        "sup1" => 0xB9,
        "ordm" => 0xBA,
        "raquo" => 0xBB,
        "frac14" => 0xBC,
        "frac12" => 0xBD,
        "frac34" => 0xBE,
        "iquest" => 0xBF,
        "times" => 0xD7,
        "divide" => 0xF7,
        "szlig" => 0xDF,
        "agrave" => 0xE0,
        "aacute" => 0xE1,
        "acirc" => 0xE2,
        "atilde" => 0xE3,
        "auml" => 0xE4,
        "aring" => 0xE5,
        "aelig" => 0xE6,
        "ccedil" => 0xE7,
        "egrave" => 0xE8,
        "eacute" => 0xE9,
        "ecirc" => 0xEA,
        "euml" => 0xEB,
        "igrave" => 0xEC,
        "iacute" => 0xED,
        "icirc" => 0xEE,
        "iuml" => 0xEF,
        "eth" => 0xF0,
        "ntilde" => 0xF1,
        "ograve" => 0xF2,
        "oacute" => 0xF3,
        "ocirc" => 0xF4,
        "otilde" => 0xF5,
        "ouml" => 0xF6,
        "oslash" => 0xF8,
        "ugrave" => 0xF9,
        "uacute" => 0xFA,
        "ucirc" => 0xFB,
        "uuml" => 0xFC,
        "yacute" => 0xFD,
        "thorn" => 0xFE,
        "yuml" => 0xFF,
        "Agrave" => 0xC0,
        "Aacute" => 0xC1,
        "Acirc" => 0xC2,
        "Atilde" => 0xC3,
        "Auml" => 0xC4,
        "Aring" => 0xC5,
        "AElig" => 0xC6,
        "Ccedil" => 0xC7,
        "Egrave" => 0xC8,
        "Eacute" => 0xC9,
        "Ecirc" => 0xCA,
        "Euml" => 0xCB,
        "Igrave" => 0xCC,
        "Iacute" => 0xCD,
        "Icirc" => 0xCE,
        "Iuml" => 0xCF,
        "ETH" => 0xD0,
        "Ntilde" => 0xD1,
        "Ograve" => 0xD2,
        "Oacute" => 0xD3,
        "Ocirc" => 0xD4,
        "Otilde" => 0xD5,
        "Ouml" => 0xD6,
        "Oslash" => 0xD8,
        "Ugrave" => 0xD9,
        "Uacute" => 0xDA,
        "Ucirc" => 0xDB,
        "Uuml" => 0xDC,
        "Yacute" => 0xDD,
        "THORN" => 0xDE,
        "OElig" => 0x152,
        "oelig" => 0x153,
        "Scaron" => 0x160,
        "scaron" => 0x161,
        "Yuml" => 0x178,
        "fnof" => 0x192,
        "circ" => 0x2C6,
        "tilde" => 0x2DC,
        "ensp" => 0x2002,
        "emsp" => 0x2003,
        "thinsp" => 0x2009,
        "zwnj" => 0x200C,
        "zwj" => 0x200D,
        "lrm" => 0x200E,
        "rlm" => 0x200F,
        "ndash" => 0x2013,
        "mdash" => 0x2014,
        "lsquo" => 0x2018,
        "rsquo" => 0x2019,
        "sbquo" => 0x201A,
        "ldquo" => 0x201C,
        "rdquo" => 0x201D,
        "bdquo" => 0x201E,
        "dagger" => 0x2020,
        "Dagger" => 0x2021,
        "bull" => 0x2022,
        "hellip" => 0x2026,
        "permil" => 0x2030,
        "prime" => 0x2032,
        "Prime" => 0x2033,
        "lsaquo" => 0x2039,
        "rsaquo" => 0x203A,
        "oline" => 0x203E,
        "frasl" => 0x2044,
        "euro" => 0x20AC,
        "trade" => 0x2122,
        "larr" => 0x2190,
        "uarr" => 0x2191,
        "rarr" => 0x2192,
        "darr" => 0x2193,
        "harr" => 0x2194,
        "minus" => 0x2212,
        "infin" => 0x221E,
        "ne" => 0x2260,
        "le" => 0x2264,
        "ge" => 0x2265,
        "loz" => 0x25CA,
        "spades" => 0x2660,
        "clubs" => 0x2663,
        "hearts" => 0x2665,
        "diams" => 0x2666,
        "alpha" => 0x3B1,
        "beta" => 0x3B2,
        "gamma" => 0x3B3,
        "delta" => 0x3B4,
        "pi" => 0x3C0,
        "sigma" => 0x3C3,
        "omega" => 0x3C9,
        "mu" => 0x3BC,
        _ => return None,
    };
    char::from_u32(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lenient_markup_parses() {
        let d = Doc::parse(
            b"<?xml version='1.0'?><!DOCTYPE html [ <!ENTITY x 'y'> ]><html><body><p>One &amp; two&nbsp;<b>bold<p>Next</b> &bogus; &#x41;<br>end<ul><li>a<li>b</ul></body></html>",
        );
        let body = d.find(0, "body").unwrap();
        let ps = d.descendants(body, "p");
        assert_eq!(ps.len(), 2);
        assert_eq!(d.text(ps[0]), "One & two bold");
        assert!(d.text(ps[1]).starts_with("Next &bogus; A"));
        assert_eq!(d.descendants(body, "li").len(), 2);
        assert_eq!(d.parent_count_ok(), ());
    }

    impl Doc {
        fn parent_count_ok(&self) {
            for (i, n) in self.nodes.iter().enumerate().skip(1) {
                assert!(self.nodes[n.parent as usize].children.contains(&(i as u32)));
            }
        }
    }

    #[test]
    fn latin1_and_utf16() {
        let d = decode_text(b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><a>\xe9</a>");
        assert!(d.contains('é'));
        let mut u = vec![0xFF, 0xFE];
        for c in "<a>é</a>".encode_utf16() {
            u.extend(c.to_le_bytes());
        }
        assert_eq!(decode_text(&u), "<a>é</a>");
    }

    #[test]
    fn hostile_never_panics() {
        for s in [
            "<",
            "<a",
            "<a b='",
            "</",
            "<!--",
            "<![CDATA[",
            "<!DOCTYPE [",
            "&#99999999999;",
            "<a\u{e9}>",
            "<é>x</é>",
            "&",
            "&#;",
            "<style>",
            "<script>x",
        ] {
            let _ = Doc::parse(s.as_bytes());
        }
        let deep = "<div>".repeat(10_000);
        let d = Doc::parse(deep.as_bytes());
        assert!(d.nodes.len() > 9_000);
    }
}
