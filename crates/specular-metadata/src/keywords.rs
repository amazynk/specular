//! Keywords: hierarchy, synonyms and private keywords; reading and writing them in XMP; the
//! Lightroom keyword-list text format; type-ahead suggestions and recently used keywords.
//!
//! A keyword is its path from the top of the hierarchy, levels joined by `|` as in
//! `lr:hierarchicalSubject` (`Places|France|Paris`); a keyword without parents is one level.
//! Keywords are told apart without regard to case.
//!
//! In XMP (what [`write_xmp`] writes):
//! - `lr:hierarchicalSubject` holds every keyword as its full path, private ones too: it is
//!   what a later read takes the hierarchy from.
//! - `dc:subject` holds what other programs should see: each keyword that is not private,
//!   the levels above it that are not private, and its synonyms (as Lightroom exports with
//!   "Export Containing Keywords" and "Export Synonyms" on).
//!
//! Reading ([`from_xmp`]) takes the paths of `lr:hierarchicalSubject`, and adds each
//! `dc:subject` entry that is not already a level of one of them (nor a synonym the caller
//! knows). Without either property the file has no keywords of its own in that packet, and
//! a sidecar without them leaves the image's embedded ones in force ([`from_file_head`]: XMP,
//! else IPTC Keywords).
//!
//! The Lightroom keyword list ([`Vocabulary::from_lightroom`], [`Vocabulary::to_lightroom`])
//! is plain text, one keyword per line, a tab per level of depth; `{name}` lines are synonyms
//! of the keyword above them, and `[name]` marks a keyword left out of exports (private).

use std::collections::HashMap;

/// The separator of a keyword path's levels.
pub const SEP: char = '|';

/// `path` with `from` (or the level of it that `path` is below) renamed to `to`:
/// `Places|Pari|Louvre` with `Places|Pari` → `Places|Paris` is `Places|Paris|Louvre`. `None`
/// when `path` is not `from` or below it.
pub fn renamed(path: &str, from: &str, to: &str) -> Option<String> {
    if !is_within(path, from) {
        return None;
    }
    // `is_within` compares lowercased; the rest after `from`'s levels keeps its spelling.
    let rest: Vec<&str> = levels(path).skip(levels(from).count()).collect();
    let mut out = to.to_owned();
    for l in rest {
        out.push(SEP);
        out.push_str(l);
    }
    Some(out)
}

/// A photo's keywords with `from` and those below it renamed to `to` (a keyword it then
/// has twice is kept once: a rename onto an existing keyword merges them).
pub fn rename_in(current: &[String], from: &str, to: &str) -> Vec<String> {
    let mut out = Vec::with_capacity(current.len());
    for k in current {
        let k = renamed(k, from, to).unwrap_or_else(|| k.clone());
        push_unique(&mut out, &k);
    }
    out
}

/// Recently used keywords kept at most.
pub const RECENTS: usize = 20;

/// The levels of `path`.
pub fn levels(path: &str) -> impl Iterator<Item = &str> {
    path.split(SEP)
}

/// The last level of `path` (the keyword's own name).
pub fn leaf(path: &str) -> &str {
    path.rsplit(SEP).next().unwrap_or(path)
}

/// The path of the level above, None at the top.
pub fn parent(path: &str) -> Option<&str> {
    path.rsplit_once(SEP).map(|(p, _)| p)
}

/// How deep `path` is (0 at the top).
pub fn depth(path: &str) -> usize {
    path.matches(SEP).count()
}

/// The comparison key of a keyword (case does not tell keywords apart).
pub fn key(path: &str) -> String {
    path.to_lowercase()
}

/// Whether `a` and `b` name the same keyword.
pub fn same(a: &str, b: &str) -> bool {
    a == b || key(a) == key(b)
}

/// Whether `path` is `ancestor` or below it.
pub fn is_within(path: &str, ancestor: &str) -> bool {
    let (p, a) = (key(path), key(ancestor));
    p == a || (p.starts_with(&a) && p[a.len()..].starts_with(SEP))
}

/// A keyword as typed into a path: levels split at `|` or ` > ` (and `>` alone), each
/// trimmed, inner runs of white space made one space, empty levels dropped. None when nothing
/// is left.
pub fn normalise(typed: &str) -> Option<String> {
    let mut out = String::new();
    for level in typed.split(['|', '>']) {
        let l: Vec<&str> = level.split_whitespace().collect();
        if l.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(SEP);
        }
        out.push_str(&l.join(" "));
    }
    (!out.is_empty()).then_some(out)
}

/// `path` for people to read: "Places › France › Paris".
pub fn display(path: &str) -> String {
    path.replace(SEP, " › ")
}

/// Adds `k` to `list` unless it is there already (in any case).
fn push_unique(list: &mut Vec<String>, k: &str) {
    if !list.iter().any(|x| same(x, k)) {
        list.push(k.to_owned());
    }
}

// ----- Reading -------------------------------------------------------------------------------

/// The keywords of a parsed XMP packet; None when it has neither `dc:subject` nor
/// `lr:hierarchicalSubject` values. `synonym` says whether a flat entry is a known synonym
/// (left out: it stands for its keyword, which the hierarchy names).
pub fn from_xmp_with(x: &crate::xmp::Xmp, synonym: &dyn Fn(&str) -> bool) -> Option<Vec<String>> {
    let hier = x.get_all("lr:hierarchicalSubject");
    let flat = x.get_all("dc:subject");
    if hier.is_none() && flat.is_none() {
        return None;
    }
    let mut out: Vec<String> = Vec::new();
    for h in hier.into_iter().flatten() {
        if let Some(p) = normalise(h) {
            push_unique(&mut out, &p);
        }
    }
    // Every level of the paths, for the flat entries they cover.
    let covered: std::collections::HashSet<String> = out
        .iter()
        .flat_map(|p| levels(p).map(key).collect::<Vec<_>>())
        .collect();
    let hier_given = !out.is_empty();
    for f in flat.into_iter().flatten() {
        // A flat entry is one keyword: a `|` in it is part of its name, not a level.
        let name: Vec<&str> = f.split_whitespace().collect();
        let name = name.join(" ").replace(SEP, "/");
        if name.is_empty() || covered.contains(&key(&name)) || (hier_given && synonym(&name)) {
            continue;
        }
        push_unique(&mut out, &name);
    }
    Some(out)
}

/// [`from_xmp_with`] without synonyms.
pub fn from_xmp(x: &crate::xmp::Xmp) -> Option<Vec<String>> {
    from_xmp_with(x, &|_| false)
}

/// The keywords of an XMP packet's text (see [`from_xmp`]); None when it names none. An empty
/// `dc:subject` or `lr:hierarchicalSubject` written by [`write_xmp`] counts as naming none
/// of its own but still being the word on them: `Some(vec![])`.
pub fn from_xmp_text(s: &str, synonym: &dyn Fn(&str) -> bool) -> Option<Vec<String>> {
    let x = crate::xmp::parse(s.as_bytes());
    match from_xmp_with(&x, synonym) {
        Some(k) => Some(k),
        None if find_property(s, "lr:hierarchicalSubject").is_some()
            || find_property(s, "dc:subject").is_some() =>
        {
            Some(Vec::new())
        }
        None => None,
    }
}

/// How far into a file the embedded keywords are looked for (JPEG APP1 and APP13, PNG iTXt,
/// TIFF and WebP put them near the start).
pub const HEAD: usize = 256 << 10;

/// The keywords embedded in a file whose first bytes are `head`: its XMP packet's, else (a
/// JPEG's) the IPTC Keywords. Empty when it has none.
pub fn from_file_head(head: &[u8]) -> Vec<String> {
    let (xmp, iptc) = if head.starts_with(&[0xFF, 0xD8]) {
        let (x, i) = jpeg_segments(head);
        (x.or_else(|| packet(head)), i)
    } else {
        (packet(head), None)
    };
    if let Some(k) = xmp.and_then(|x| from_xmp(&crate::xmp::parse(x))) {
        return k;
    }
    iptc.map(iptc_keywords).unwrap_or_default()
}

/// The Keywords datasets (2:25) of an IPTC-IIM block.
pub fn iptc_keywords(iim: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    for (k, v) in crate::iptc::parse(iim).map(|t| t.props).unwrap_or_default() {
        if k == "dc:subject" {
            push_unique(&mut out, &v.replace(SEP, "/"));
        }
    }
    out
}

/// A JPEG's XMP (APP1) and IPTC-IIM (APP13) blocks among the segments before the image
/// data; what a cut-off head still holds.
fn jpeg_segments(data: &[u8]) -> (Option<&[u8]>, Option<&[u8]>) {
    const XMP_SIG: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
    const PHOTOSHOP_SIG: &[u8] = b"Photoshop 3.0\0";
    let (mut xmp, mut iptc) = (None, None);
    let mut i = 2;
    while i + 4 <= data.len() && data[i] == 0xFF {
        let marker = data[i + 1];
        if marker == 0xFF {
            i += 1;
            continue;
        }
        if matches!(marker, 0xD9 | 0xDA) {
            break;
        }
        let len = usize::from(u16::from_be_bytes([data[i + 2], data[i + 3]]));
        let Some(seg) = data.get(i + 4..i + 2 + len.max(2)) else {
            break;
        };
        if marker == 0xE1 && xmp.is_none() && seg.starts_with(XMP_SIG) {
            xmp = Some(&seg[XMP_SIG.len()..]);
        } else if marker == 0xED && iptc.is_none() && seg.starts_with(PHOTOSHOP_SIG) {
            iptc = crate::iptc::from_resources(&seg[PHOTOSHOP_SIG.len()..]);
        }
        i += 2 + len.max(2);
    }
    (xmp, iptc)
}

/// The first `<x:xmpmeta>…</x:xmpmeta>` packet in `head`.
pub(crate) fn packet(head: &[u8]) -> Option<&[u8]> {
    const OPEN: &[u8] = b"<x:xmpmeta";
    const CLOSE: &[u8] = b"</x:xmpmeta>";
    let at = find_bytes(head, OPEN)?;
    let len = find_bytes(&head[at..], CLOSE)?;
    Some(&head[at..at + len + CLOSE.len()])
}

pub(crate) fn find_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

// ----- Writing -------------------------------------------------------------------------------

const NS_DC: &str = "http://purl.org/dc/elements/1.1/";
const NS_LR: &str = "http://ns.adobe.com/lightroom/1.0/";

/// What `dc:subject` holds for `keywords` (see the module notes): public keywords, their
/// public ancestors and their synonyms, each once.
pub fn subject(keywords: &[String], vocab: Option<&Vocabulary>) -> Vec<String> {
    let private = |p: &str| vocab.is_some_and(|v| v.is_private(p));
    let mut out = Vec::new();
    for k in keywords {
        if private(k) {
            continue;
        }
        let mut up = parent(k);
        let mut ancestors = Vec::new();
        while let Some(a) = up {
            if !private(a) {
                ancestors.push(leaf(a));
            }
            up = parent(a);
        }
        push_unique(&mut out, leaf(k));
        for a in ancestors {
            push_unique(&mut out, a);
        }
        if let Some(e) = vocab.and_then(|v| v.get(k)) {
            for s in &e.synonyms {
                push_unique(&mut out, s);
            }
        }
    }
    out
}

/// `existing` (an XMP packet's text, or None for a new sidecar) with `dc:subject` and
/// `lr:hierarchicalSubject` set to `keywords`. Everything else is kept byte for byte. Both
/// are written even when empty, so a sidecar can say "no keywords" over an image's own.
pub fn write_xmp(
    existing: Option<&str>,
    keywords: &[String],
    vocab: Option<&Vocabulary>,
) -> String {
    write_lists(existing, &subject(keywords, vocab), keywords)
}

/// What an exported copy carries of a sidecar's keywords: its `dc:subject` (which leaves
/// private keywords out), and the `lr:hierarchicalSubject` paths whose own name is in it.
/// None when the sidecar names no keywords.
pub fn for_export(sidecar: &str) -> Option<(Vec<String>, Vec<String>)> {
    from_xmp_text(sidecar, &|_| false)?;
    let x = crate::xmp::parse(sidecar.as_bytes());
    let subject: Vec<String> = x
        .get_all("dc:subject")
        .map(<[String]>::to_vec)
        .unwrap_or_default();
    let public: std::collections::HashSet<String> = subject.iter().map(|s| key(s)).collect();
    let hier = x
        .get_all("lr:hierarchicalSubject")
        .into_iter()
        .flatten()
        .filter_map(|h| normalise(h))
        .filter(|h| public.contains(&key(leaf(h))))
        .collect();
    Some((subject, hier))
}

/// `existing` with `dc:subject` and `lr:hierarchicalSubject` set to the lists given (see
/// [`write_xmp`]).
pub fn write_lists(existing: Option<&str>, subject: &[String], hierarchy: &[String]) -> String {
    let dc = bag("dc:subject", subject);
    let lr = bag("lr:hierarchicalSubject", hierarchy);
    let Some(s) = existing.filter(|s| s.contains("<rdf:Description")) else {
        return minimal(&dc, &lr);
    };
    let mut s = s.to_owned();
    for (name, element) in [("dc:subject", &dc), ("lr:hierarchicalSubject", &lr)] {
        s = match find_property(&s, name) {
            // An attribute leaves its tag; the element goes in as a new one.
            Some(r) if !s[r.clone()].trim_start().starts_with('<') => {
                let rest = format!("{}{}", &s[..r.start], &s[r.end..]);
                insert_property(&rest, element)
            }
            Some(r) => format!("{}{element}{}", &s[..r.start], &s[r.end..]),
            None => insert_property(&s, element),
        };
    }
    s = ensure_ns(s, "dc", NS_DC);
    ensure_ns(s, "lr", NS_LR)
}

pub(crate) fn bag(name: &str, values: &[String]) -> String {
    if values.is_empty() {
        return format!("<{name}>\n    <rdf:Bag/>\n   </{name}>");
    }
    let mut s = format!("<{name}>\n    <rdf:Bag>\n");
    for v in values {
        s.push_str("     <rdf:li>");
        s.push_str(&escape(v));
        s.push_str("</rdf:li>\n");
    }
    s.push_str(&format!("    </rdf:Bag>\n   </{name}>"));
    s
}

fn minimal(dc: &str, lr: &str) -> String {
    format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
         <x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"Specular\">\n \
         <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n  \
         <rdf:Description rdf:about=\"\"\n    \
         xmlns:dc=\"{NS_DC}\"\n    \
         xmlns:lr=\"{NS_LR}\">\n   \
         {dc}\n   \
         {lr}\n  \
         </rdf:Description>\n \
         </rdf:RDF>\n\
         </x:xmpmeta>\n\
         <?xpacket end=\"w\"?>\n"
    )
}

/// The byte range of property `name`'s element in `s` (`<name>…</name>` or `<name/>`), or
/// of its attribute (`name="…"`, a single value some writers use).
pub(crate) fn find_property(s: &str, name: &str) -> Option<std::ops::Range<usize>> {
    let b = s.as_bytes();
    let mut from = 0;
    while let Some(off) = s[from..].find(name) {
        let i = from + off;
        from = i + name.len();
        let before = i.checked_sub(1).map(|k| b[k]);
        let after = &s[i + name.len()..];
        if before == Some(b'<') {
            // `<name>` or `<name …>` or `<name/>`, not `<name2`.
            let next = after.chars().next();
            if !matches!(next, Some('>' | '/' | ' ' | '\t' | '\r' | '\n')) {
                continue;
            }
            let open_end = i + name.len() + after.find('>')? + 1;
            if s[..open_end].ends_with("/>") {
                return Some(i - 1..open_end);
            }
            let close = format!("</{name}>");
            let end = s[open_end..].find(&close)?;
            return Some(i - 1..open_end + end + close.len());
        }
        if !before.is_some_and(|c| c.is_ascii_whitespace()) {
            continue;
        }
        let t = after.trim_start();
        let Some(t) = t.strip_prefix('=') else {
            continue;
        };
        let t2 = t.trim_start();
        let Some(q) = t2.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            continue;
        };
        let v0 = s.len() - t2.len() + 1;
        let len = s[v0..].find(q)?;
        // The space before the attribute goes with it.
        let start = s[..i].trim_end().len();
        return Some(start..v0 + len + 1);
    }
    None
}

/// `element` added to the first `rdf:Description` (opened up when it is an empty element).
pub(crate) fn insert_property(s: &str, element: &str) -> String {
    let Some(at) = s.find("<rdf:Description") else {
        return s.to_owned();
    };
    let Some(end) = tag_end(s, at) else {
        return s.to_owned();
    };
    if s[..end].ends_with("/>") {
        let head = s[..end - 2].trim_end();
        return format!("{head}>\n   {element}\n  </rdf:Description>{}", &s[end..]);
    }
    format!("{}\n   {element}{}", &s[..end], &s[end..])
}

/// The index after the `>` that ends the tag opened at `at` (quotes respected).
pub(crate) fn tag_end(s: &str, at: usize) -> Option<usize> {
    let mut quote = None;
    for (i, c) in s[at..].char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '>') => return Some(at + i + 1),
            _ => {}
        }
    }
    None
}

fn ensure_ns(s: String, prefix: &str, uri: &str) -> String {
    if s.contains(&format!("xmlns:{prefix}=")) {
        return s;
    }
    s.replacen(
        "<rdf:Description",
        &format!("<rdf:Description xmlns:{prefix}=\"{uri}\""),
        1,
    )
}

pub(crate) fn escape(v: &str) -> String {
    v.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ----- The keyword list ----------------------------------------------------------------------

/// One keyword of the list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    /// Its full path.
    pub path: String,
    /// Other names it is found and exported by.
    pub synonyms: Vec<String>,
    /// Left out of `dc:subject` and of exports.
    pub private: bool,
}

/// The user's keyword list: every keyword known, with its synonyms and whether it is
/// private. Each keyword's parents are in the list too.
#[derive(Clone, Debug, Default)]
pub struct Vocabulary {
    entries: Vec<Entry>,
    index: HashMap<String, usize>,
    /// Folded synonym → the keywords that have it.
    synonyms: HashMap<String, Vec<usize>>,
    /// Bumped by every change (callers cache what they build from the list by it).
    pub generation: u64,
}

impl Vocabulary {
    pub fn new() -> Vocabulary {
        Vocabulary::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn get(&self, path: &str) -> Option<&Entry> {
        self.index.get(&key(path)).map(|&i| &self.entries[i])
    }

    pub fn contains(&self, path: &str) -> bool {
        self.index.contains_key(&key(path))
    }

    /// The keyword as the list spells it (`paris` → `Places|France|Paris` is not found:
    /// only the same path in another case is).
    pub fn spelling<'a>(&'a self, path: &'a str) -> &'a str {
        self.get(path).map_or(path, |e| e.path.as_str())
    }

    pub fn is_private(&self, path: &str) -> bool {
        self.get(path).is_some_and(|e| e.private)
    }

    /// Whether `name` is a synonym of some keyword.
    pub fn is_synonym(&self, name: &str) -> bool {
        self.synonyms.contains_key(&key(name))
    }

    /// The keywords that `name` is a synonym of.
    pub fn synonym_of(&self, name: &str) -> Vec<&str> {
        self.synonyms
            .get(&key(name))
            .map(|v| v.iter().map(|&i| self.entries[i].path.as_str()).collect())
            .unwrap_or_default()
    }

    /// Adds `path` and the levels above it (those known keep their spelling). Returns the
    /// path as the list spells it.
    pub fn add(&mut self, path: &str) -> String {
        let mut spelled = String::new();
        for level in levels(path) {
            if !spelled.is_empty() {
                spelled.push(SEP);
            }
            spelled.push_str(level);
            match self.index.get(&key(&spelled)) {
                Some(&i) => spelled = self.entries[i].path.clone(),
                None => {
                    self.index.insert(key(&spelled), self.entries.len());
                    self.entries.push(Entry {
                        path: spelled.clone(),
                        ..Entry::default()
                    });
                    self.generation += 1;
                }
            }
        }
        spelled
    }

    /// Adds every keyword of `paths` (see [`Vocabulary::add`]).
    pub fn learn<'a>(&mut self, paths: impl IntoIterator<Item = &'a String>) {
        for p in paths {
            if !self.contains(p) {
                self.add(p);
            }
        }
    }

    pub fn set_private(&mut self, path: &str, private: bool) {
        if let Some(&i) = self.index.get(&key(path))
            && self.entries[i].private != private
        {
            self.entries[i].private = private;
            self.generation += 1;
        }
    }

    /// Sets the synonyms of `path` (trimmed, empty and repeated ones and the keyword's own
    /// name dropped).
    pub fn set_synonyms(&mut self, path: &str, synonyms: &[String]) {
        let Some(&i) = self.index.get(&key(path)) else {
            return;
        };
        let own = leaf(&self.entries[i].path).to_owned();
        let mut list = Vec::new();
        for s in synonyms {
            let s: Vec<&str> = s.split_whitespace().collect();
            let s = s.join(" ").replace(SEP, "/");
            if !s.is_empty() && !same(&s, &own) {
                push_unique(&mut list, &s);
            }
        }
        if self.entries[i].synonyms == list {
            return;
        }
        for old in &self.entries[i].synonyms {
            let k = key(old);
            if let Some(v) = self.synonyms.get_mut(&k) {
                v.retain(|&j| j != i);
                if v.is_empty() {
                    self.synonyms.remove(&k);
                }
            }
        }
        for s in &list {
            self.synonyms.entry(key(s)).or_default().push(i);
        }
        self.entries[i].synonyms = list;
        self.generation += 1;
    }

    /// Removes `path` and every keyword below it. Returns how many left the list.
    pub fn remove(&mut self, path: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|e| !is_within(&e.path, path));
        let n = before - self.entries.len();
        if n > 0 {
            self.reindex();
        }
        n
    }

    /// Renames `from` and every keyword below it to `to` (a move when the levels above
    /// change). Onto a keyword that exists, the two merge: synonyms joined, private if either
    /// was. Returns how many keywords were renamed (0: `from` is not listed, or `to` is
    /// `from` or below it).
    pub fn rename(&mut self, from: &str, to: &str) -> usize {
        if !self.contains(from) || is_within(to, from) {
            return 0;
        }
        let moved: Vec<Entry> = self
            .entries
            .iter()
            .filter(|e| is_within(&e.path, from))
            .cloned()
            .collect();
        self.entries.retain(|e| !is_within(&e.path, from));
        self.reindex();
        for e in &moved {
            let Some(new) = renamed(&e.path, from, to) else {
                continue;
            };
            let p = self.add(&new);
            let mut syn = self.get(&p).map(|x| x.synonyms.clone()).unwrap_or_default();
            for s in &e.synonyms {
                push_unique(&mut syn, s);
            }
            self.set_synonyms(&p, &syn);
            if e.private {
                self.set_private(&p, true);
            }
        }
        moved.len()
    }

    /// Takes in `other` (an imported list): new keywords join, known ones gain its synonyms,
    /// and its private marks apply. Returns how many keywords were new.
    pub fn merge(&mut self, other: &Vocabulary) -> usize {
        let before = self.entries.len();
        for e in &other.entries {
            let p = self.add(&e.path);
            let mut syn = self.get(&p).map(|x| x.synonyms.clone()).unwrap_or_default();
            for s in &e.synonyms {
                push_unique(&mut syn, s);
            }
            self.set_synonyms(&p, &syn);
            if e.private {
                self.set_private(&p, true);
            }
        }
        self.entries.len() - before
    }

    fn reindex(&mut self) {
        self.index.clear();
        self.synonyms.clear();
        for (i, e) in self.entries.iter().enumerate() {
            self.index.insert(key(&e.path), i);
            for s in &e.synonyms {
                self.synonyms.entry(key(s)).or_default().push(i);
            }
        }
        self.generation += 1;
    }

    /// The keywords in tree order: each after its parent, siblings sorted by name (case
    /// and accents as `fold` folds them).
    pub fn tree(&self, fold: &dyn Fn(&str) -> String) -> Vec<&Entry> {
        let mut order: Vec<(Vec<String>, usize)> = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| (levels(&e.path).map(fold).collect(), i))
            .collect();
        order.sort();
        order.into_iter().map(|(_, i)| &self.entries[i]).collect()
    }

    /// Reads a Lightroom keyword list (see the module notes). Lines are tab-indented, one tab
    /// per level; a deeper jump is taken as one level down. Blank lines are skipped; a byte
    /// order mark and Windows line ends are fine.
    pub fn from_lightroom(text: &str) -> Vocabulary {
        let mut v = Vocabulary::new();
        // The path at each depth so far.
        let mut stack: Vec<String> = Vec::new();
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        for line in text.lines() {
            let tabs = line.chars().take_while(|&c| c == '\t').count();
            let name = line.trim();
            if name.is_empty() {
                continue;
            }
            if let Some(syn) = name.strip_prefix('{').and_then(|n| n.strip_suffix('}')) {
                // A synonym of the keyword one level up from it.
                let owner = stack.get(tabs.saturating_sub(1).min(stack.len().saturating_sub(1)));
                if let Some(owner) = owner.cloned() {
                    let mut syn_list = v
                        .get(&owner)
                        .map(|e| e.synonyms.clone())
                        .unwrap_or_default();
                    syn_list.push(syn.to_owned());
                    v.set_synonyms(&owner, &syn_list);
                }
                continue;
            }
            let (name, private) = match name.strip_prefix('[').and_then(|n| n.strip_suffix(']')) {
                Some(n) => (n, true),
                None => (name, false),
            };
            let name: Vec<&str> = name.split_whitespace().collect();
            let name = name.join(" ").replace(SEP, "/");
            if name.is_empty() {
                continue;
            }
            let d = tabs.min(stack.len());
            stack.truncate(d);
            let path = match stack.last() {
                Some(p) => format!("{p}{SEP}{name}"),
                None => name,
            };
            let path = v.add(&path);
            if private {
                v.set_private(&path, true);
            }
            stack.push(path);
        }
        v
    }

    /// The list as a Lightroom keyword list, in tree order (by plain lower case), with
    /// Windows line ends as Lightroom writes them.
    pub fn to_lightroom(&self) -> String {
        let fold = |s: &str| s.to_lowercase();
        let mut out = String::new();
        for e in self.tree(&fold) {
            let d = depth(&e.path);
            let name = leaf(&e.path);
            out.push_str(&"\t".repeat(d));
            if e.private {
                out.push('[');
                out.push_str(name);
                out.push(']');
            } else {
                out.push_str(name);
            }
            out.push_str("\r\n");
            for s in &e.synonyms {
                out.push_str(&"\t".repeat(d + 1));
                out.push('{');
                out.push_str(s);
                out.push('}');
                out.push_str("\r\n");
            }
        }
        out
    }
}

// ----- Suggestions and recents ---------------------------------------------------------------

/// A keyword offered for what was typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub path: String,
    /// The synonym that matched, when it was one.
    pub synonym: Option<String>,
}

/// Up to `limit` keywords for `typed`: from the list, then `known` (keywords found on files,
/// such as the catalogue's), best first. A keyword whose own name starts with the text comes
/// first, then one with a word of its name starting so, a synonym starting so, its name
/// holding the text, and last a level above it starting so; recently used ones go first
/// within each, then shorter names, then by name. `fold` folds case and accents.
pub fn suggest(
    vocab: &Vocabulary,
    known: &[String],
    recents: &Recents,
    typed: &str,
    fold: &dyn Fn(&str) -> String,
    limit: usize,
) -> Vec<Suggestion> {
    let Some(want) = normalise(typed) else {
        return Vec::new();
    };
    // With levels typed ("places>fr"), the levels above must match too.
    let want_levels: Vec<String> = levels(&want).map(fold).collect();
    let (want_leaf, want_up) = want_levels.split_last().expect("one level at least");
    let mut found: Vec<((u8, usize, usize, String), Suggestion)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let paths = vocab
        .entries
        .iter()
        .map(|e| (e.path.as_str(), e.synonyms.as_slice()))
        .chain(known.iter().map(|k| (k.as_str(), &[][..])));
    for (path, synonyms) in paths {
        if !seen.insert(key(path)) {
            continue;
        }
        let lv: Vec<String> = levels(path).map(fold).collect();
        let (own, up) = lv.split_last().expect("one level at least");
        if !want_up.is_empty() && !ups_match(up, want_up) {
            continue;
        }
        let word_start = own
            .split(|c: char| !c.is_alphanumeric())
            .any(|w| w.starts_with(want_leaf.as_str()));
        let mut synonym = None;
        let rank = if own.starts_with(want_leaf.as_str()) {
            0
        } else if word_start {
            1
        } else if let Some(s) = synonyms
            .iter()
            .find(|s| fold(s).starts_with(want_leaf.as_str()))
        {
            synonym = Some(s.clone());
            2
        } else if own.contains(want_leaf.as_str()) {
            3
        } else if want_up.is_empty() && up.iter().any(|u| u.starts_with(want_leaf.as_str())) {
            4
        } else {
            continue;
        };
        let recent = recents.rank(path).unwrap_or(usize::MAX);
        found.push((
            (rank, recent, own.chars().count(), lv.join("\u{1}")),
            Suggestion {
                path: path.to_owned(),
                synonym,
            },
        ));
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found.into_iter().take(limit).map(|(_, s)| s).collect()
}

/// Whether each typed upper level starts one of a keyword's upper levels, in order.
fn ups_match(up: &[String], want: &[String]) -> bool {
    let mut it = up.iter();
    want.iter().all(|w| it.any(|u| u.starts_with(w.as_str())))
}

/// Recently used keywords, the latest first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recents {
    list: Vec<String>,
}

impl Recents {
    pub fn list(&self) -> &[String] {
        &self.list
    }

    /// `path` was used: it goes first.
    pub fn used(&mut self, path: &str) {
        self.list.retain(|p| !same(p, path));
        self.list.insert(0, path.to_owned());
        self.list.truncate(RECENTS);
    }

    /// How recently `path` was used (0 the latest), None when it was not.
    pub fn rank(&self, path: &str) -> Option<usize> {
        self.list.iter().position(|p| same(p, path))
    }

    /// Forgets `path` and what is below it (the keyword left the list).
    pub fn forget(&mut self, path: &str) {
        self.list.retain(|p| !is_within(p, path));
    }

    /// One keyword per line.
    pub fn from_text(s: &str) -> Recents {
        let mut r = Recents::default();
        for l in s.lines().rev() {
            if let Some(p) = normalise(l) {
                r.used(&p);
            }
        }
        r
    }

    pub fn to_text(&self) -> String {
        let mut s = String::new();
        for p in &self.list {
            s.push_str(p);
            s.push('\n');
        }
        s
    }
}

// ----- Changes ---------------------------------------------------------------------------------

/// A change to some files' keywords.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Add(String),
    Remove(String),
}

/// `current` with `changes` applied: an added keyword joins once; a removed one leaves (the
/// keywords below it stay: each was given on its own).
pub fn apply(current: &[String], changes: &[Change]) -> Vec<String> {
    let mut out = current.to_vec();
    for c in changes {
        match c {
            Change::Add(p) => push_unique(&mut out, p),
            Change::Remove(p) => out.retain(|k| !same(k, p)),
        }
    }
    out
}

#[cfg(test)]
mod tests;
