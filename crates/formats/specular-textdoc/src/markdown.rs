//! Markdown to XHTML: the CommonMark blocks (ATX and setext headings, paragraphs, block
//! quotes, ordered and bullet lists, fenced and indented code, thematic breaks) and inlines
//! (emphasis, strong, code spans, links, images, autolinks, hard breaks, backslash escapes,
//! entities), with GitHub's tables, strikethrough and task lists.
//!
//! Links are shown, never followed: their text is underlined. Images load only from local
//! paths relative to the file, through the caller; anything on the network shows its
//! description instead. Raw HTML is shown as text.

use std::collections::HashMap;

use specular_epub::{HtmlSection, html_escape as esc};

/// Pictures a document may load.
pub const MAX_PICTURES: usize = 256;

/// The XHTML of a Markdown document.
#[derive(Clone, Debug, Default)]
pub struct Rendered {
    pub section: HtmlSection,
    /// (name under `images/`, bytes) of the pictures loaded.
    pub pictures: Vec<(String, Vec<u8>)>,
    /// The first heading, as a title.
    pub title: Option<String>,
    /// Pictures left out: on the network, missing or not a picture.
    pub missing_pictures: usize,
}

/// The style sheet for rendered Markdown.
pub const CSS: &str = "body{line-height:1.45}\n\
h1,h2{border-bottom:1px solid #ccc;padding-bottom:0.2em}\n\
pre{background:#f3f3f3;padding:0.5em;white-space:pre-wrap}\n\
code{background:#f3f3f3}\n\
blockquote{margin-left:1em;padding-left:0.8em;border-left:3px solid #ccc;color:#555}\n\
table{border-collapse:collapse}\n\
td,th{border:1px solid #bbb;padding:0.2em 0.5em}\n\
u.link{color:#1a5fb4}\n\
p.missing{color:#777;font-style:italic}\n";

/// Loads a picture by the path the document gives (relative to it); `None` when it cannot.
pub type PictureLoader<'a> = &'a dyn Fn(&str) -> Option<Vec<u8>>;

struct Ctx<'a> {
    refs: HashMap<String, (String, Option<String>)>,
    load: PictureLoader<'a>,
    pictures: Vec<(String, Vec<u8>)>,
    by_src: HashMap<String, Option<String>>,
    missing: usize,
    headings: Vec<(String, String, u32)>,
    title: Option<String>,
}

/// Converts `text` to XHTML; `load` reads pictures by their (relative) paths.
pub fn to_xhtml(text: &str, load: PictureLoader) -> Rendered {
    let lines: Vec<&str> = text
        .trim_start_matches('\u{feff}')
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    let mut ctx = Ctx {
        refs: HashMap::new(),
        load,
        pictures: Vec::new(),
        by_src: HashMap::new(),
        missing: 0,
        headings: Vec::new(),
        title: None,
    };
    let lines = collect_refs(&lines, &mut ctx.refs);
    let mut body = String::with_capacity(text.len() * 5 / 4);
    blocks(&lines, &mut ctx, &mut body, 0);
    Rendered {
        section: HtmlSection {
            body,
            headings: ctx.headings,
        },
        pictures: ctx.pictures,
        title: ctx.title,
        missing_pictures: ctx.missing,
    }
}

/// Link reference definitions (`[id]: url "title"`) taken out of the lines.
fn collect_refs<'a>(
    lines: &[&'a str],
    refs: &mut HashMap<String, (String, Option<String>)>,
) -> Vec<&'a str> {
    let mut out = Vec::with_capacity(lines.len());
    let mut fence: Option<(char, usize)> = None;
    for &l in lines {
        if let Some(f) = fence_open(l) {
            fence = match fence {
                Some((c, n)) if f.0 == c && f.1 >= n && f.2.trim().is_empty() => None,
                None => Some((f.0, f.1)),
                other => other,
            };
            out.push(l);
            continue;
        }
        if fence.is_none()
            && indent(l) < 4
            && let Some(rest) = l.trim_start().strip_prefix('[')
            && let Some((id, def)) = rest.split_once("]:")
            && !id.is_empty()
        {
            let def = def.trim();
            let (url, title) = match def.split_once(char::is_whitespace) {
                Some((u, t)) => (
                    u,
                    Some(t.trim().trim_matches(['"', '\'', '(', ')']).to_owned()),
                ),
                None => (def, None),
            };
            let url = url.trim_start_matches('<').trim_end_matches('>');
            refs.entry(id.to_lowercase())
                .or_insert((url.to_owned(), title));
            continue;
        }
        out.push(l);
    }
    out
}

fn indent(l: &str) -> usize {
    let mut n = 0;
    for c in l.chars() {
        match c {
            ' ' => n += 1,
            '\t' => n += 4 - n % 4,
            _ => break,
        }
    }
    n
}

/// `l` with up to `n` columns of indentation removed.
fn dedent(l: &str, n: usize) -> &str {
    let mut col = 0;
    for (i, c) in l.char_indices() {
        if col >= n {
            return &l[i..];
        }
        match c {
            ' ' => col += 1,
            '\t' => col += 4 - col % 4,
            _ => return &l[i..],
        }
    }
    ""
}

/// A code fence: its character, length and info string.
fn fence_open(l: &str) -> Option<(char, usize, &str)> {
    if indent(l) >= 4 {
        return None;
    }
    let t = l.trim_start();
    let c = t.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let n = t.chars().take_while(|&x| x == c).count();
    if n < 3 {
        return None;
    }
    let info = &t[n..];
    if c == '`' && info.contains('`') {
        return None;
    }
    Some((c, n, info.trim()))
}

fn atx(l: &str) -> Option<(u32, &str)> {
    if indent(l) >= 4 {
        return None;
    }
    let t = l.trim_start();
    let n = t.chars().take_while(|&c| c == '#').count();
    if !(1..=6).contains(&n) {
        return None;
    }
    let rest = &t[n..];
    if !rest.is_empty() && !rest.starts_with([' ', '\t']) {
        return None;
    }
    let mut text = rest.trim();
    // Closing hashes.
    let stripped = text.trim_end_matches('#');
    if stripped.is_empty() || stripped.ends_with([' ', '\t']) {
        text = stripped.trim_end();
    }
    Some((n as u32, text))
}

fn thematic(l: &str) -> bool {
    if indent(l) >= 4 {
        return false;
    }
    let t: String = l.chars().filter(|c| !c.is_whitespace()).collect();
    t.len() >= 3
        && (t.chars().all(|c| c == '-')
            || t.chars().all(|c| c == '*')
            || t.chars().all(|c| c == '_'))
}

/// A list item marker: (ordered start number, marker char, content column, rest).
fn list_marker(l: &str) -> Option<(Option<u64>, char, usize, &str)> {
    let ind = indent(l);
    if ind >= 4 {
        return None;
    }
    let t = l.trim_start();
    let lead = l.len() - t.len();
    let (num, mark, mlen) =
        if let Some(c) = t.chars().next().filter(|c| matches!(c, '-' | '*' | '+')) {
            (None, c, 1)
        } else {
            let digits = t.chars().take_while(char::is_ascii_digit).count();
            if digits == 0 || digits > 9 {
                return None;
            }
            let c = t[digits..]
                .chars()
                .next()
                .filter(|c| *c == '.' || *c == ')')?;
            (t[..digits].parse().ok(), c, digits + 1)
        };
    let after = &t[mlen..];
    if !after.is_empty() && !after.starts_with([' ', '\t']) {
        return None;
    }
    let pad = indent(after).clamp(1, 4);
    let pad = if after.trim().is_empty() { 1 } else { pad };
    let _ = lead;
    Some((num, mark, ind + mlen + pad, dedent(after, pad)))
}

/// A table delimiter row: the alignment of each column.
fn table_delims(l: &str) -> Option<Vec<&'static str>> {
    let t = l.trim().trim_start_matches('|').trim_end_matches('|');
    if !t.contains('-') {
        return None;
    }
    let mut out = Vec::new();
    for cell in t.split('|') {
        let c = cell.trim();
        if c.is_empty() || !c.trim_matches(':').chars().all(|x| x == '-') || !c.contains('-') {
            return None;
        }
        out.push(match (c.starts_with(':'), c.ends_with(':')) {
            (true, true) => "center",
            (false, true) => "right",
            _ => "left",
        });
    }
    Some(out)
}

fn table_cells(l: &str) -> Vec<String> {
    let t = l.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t
        .strip_suffix('|')
        .filter(|s| !s.ends_with('\\'))
        .unwrap_or(t);
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut code = false;
    let mut chars = t.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cur.push('|');
                chars.next();
            }
            '`' => {
                code = !code;
                cur.push(c);
            }
            '|' if !code => cells.push(std::mem::take(&mut cur).trim().to_owned()),
            _ => cur.push(c),
        }
    }
    cells.push(cur.trim().to_owned());
    cells
}

fn starts_block(l: &str) -> bool {
    atx(l).is_some()
        || thematic(l)
        || fence_open(l).is_some()
        || (indent(l) < 4 && l.trim_start().starts_with('>'))
        || list_marker(l)
            .is_some_and(|(n, _, _, rest)| !rest.trim().is_empty() && n.is_none_or(|n| n == 1))
}

/// Writes the blocks of `lines` to `out`.
fn blocks(lines: &[&str], ctx: &mut Ctx, out: &mut String, depth: usize) {
    if depth > 32 {
        // Nested past reason: the rest as text.
        out.push_str("<pre>");
        out.push_str(&esc(&lines.join("\n")));
        out.push_str("</pre>");
        return;
    }
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        if l.trim().is_empty() {
            i += 1;
            continue;
        }
        // Fenced code.
        if let Some((c, n, info)) = fence_open(l) {
            let ind = indent(l);
            let mut code = Vec::new();
            i += 1;
            while i < lines.len() {
                if let Some((c2, n2, rest)) = fence_open(lines[i])
                    && c2 == c
                    && n2 >= n
                    && rest.is_empty()
                {
                    i += 1;
                    break;
                }
                code.push(dedent(lines[i], ind));
                i += 1;
            }
            let lang = info.split_whitespace().next().unwrap_or("");
            out.push_str("<pre>");
            if !lang.is_empty() {
                out.push_str(&format!("<code class=\"language-{}\">", esc(lang)));
            } else {
                out.push_str("<code>");
            }
            out.push_str(&esc(&code.join("\n")));
            out.push_str("</code></pre>\n");
            continue;
        }
        // Indented code.
        if indent(l) >= 4 {
            let mut code = Vec::new();
            while i < lines.len() && (indent(lines[i]) >= 4 || lines[i].trim().is_empty()) {
                code.push(dedent(lines[i], 4));
                i += 1;
            }
            while code.last().is_some_and(|l| l.trim().is_empty()) {
                code.pop();
            }
            out.push_str("<pre><code>");
            out.push_str(&esc(&code.join("\n")));
            out.push_str("</code></pre>\n");
            continue;
        }
        if let Some((level, text)) = atx(l) {
            heading(level, text, ctx, out);
            i += 1;
            continue;
        }
        if thematic(l) {
            out.push_str("<hr/>\n");
            i += 1;
            continue;
        }
        // Block quote: its lines without the markers (lazy continuation lines included).
        if l.trim_start().starts_with('>') {
            let mut inner = Vec::new();
            while i < lines.len() {
                let t = lines[i].trim_start();
                if let Some(r) = t.strip_prefix('>') {
                    inner.push(r.strip_prefix(' ').unwrap_or(r));
                } else if !t.is_empty()
                    && !inner.last().is_none_or(|l: &&str| l.trim().is_empty())
                    && !starts_block(lines[i])
                {
                    inner.push(lines[i]);
                } else {
                    break;
                }
                i += 1;
            }
            out.push_str("<blockquote>");
            blocks(&inner, ctx, out, depth + 1);
            out.push_str("</blockquote>\n");
            continue;
        }
        // Lists.
        if let Some((num, mark, _, _)) = list_marker(l) {
            i = list(lines, i, num, mark, ctx, out, depth);
            continue;
        }
        // A table: a header row and a delimiter row.
        if l.contains('|')
            && let Some(aligns) = lines.get(i + 1).and_then(|n| table_delims(n))
            && table_cells(l).len() == aligns.len()
        {
            out.push_str("<table><tr>");
            for (c, a) in table_cells(l).iter().zip(&aligns) {
                out.push_str(&format!("<th style=\"text-align:{a}\">"));
                inline(c, ctx, out);
                out.push_str("</th>");
            }
            out.push_str("</tr>\n");
            i += 2;
            while i < lines.len() && !lines[i].trim().is_empty() && !starts_block(lines[i]) {
                let cells = table_cells(lines[i]);
                out.push_str("<tr>");
                for (k, a) in aligns.iter().enumerate() {
                    out.push_str(&format!("<td style=\"text-align:{a}\">"));
                    if let Some(c) = cells.get(k) {
                        inline(c, ctx, out);
                    }
                    out.push_str("</td>");
                }
                out.push_str("</tr>\n");
                i += 1;
            }
            out.push_str("</table>\n");
            continue;
        }
        // A paragraph, or a setext heading.
        let mut para = vec![l.trim_start()];
        i += 1;
        let mut setext = None;
        while i < lines.len() {
            let n = lines[i];
            if n.trim().is_empty() {
                break;
            }
            let t = n.trim();
            if indent(n) < 4
                && !t.is_empty()
                && (t.chars().all(|c| c == '=') || (t.chars().all(|c| c == '-')))
            {
                setext = Some(if t.starts_with('=') { 1 } else { 2 });
                i += 1;
                break;
            }
            if starts_block(n) {
                break;
            }
            para.push(n.trim_start());
            i += 1;
        }
        let text = para.join("\n");
        match setext {
            Some(level) => heading(level, text.trim(), ctx, out),
            None => {
                out.push_str("<p>");
                inline(text.trim_end(), ctx, out);
                out.push_str("</p>\n");
            }
        }
    }
}

fn heading(level: u32, text: &str, ctx: &mut Ctx, out: &mut String) {
    let id = format!("h{}", ctx.headings.len());
    let mut inner = String::new();
    inline(text, ctx, &mut inner);
    let plain = strip_tags(&inner);
    if ctx.title.is_none() && !plain.is_empty() {
        ctx.title = Some(plain.clone());
    }
    ctx.headings
        .push((plain, id.clone(), level.saturating_sub(1).min(5)));
    out.push_str(&format!("<h{level} id=\"{id}\">{inner}</h{level}>\n"));
}

/// A list starting at line `i`: its items, each parsed as blocks. Returns the line after it.
fn list(
    lines: &[&str],
    mut i: usize,
    num: Option<u64>,
    mark: char,
    ctx: &mut Ctx,
    out: &mut String,
    depth: usize,
) -> usize {
    let ordered = num.is_some();
    match num {
        Some(1) | None => out.push_str(if ordered { "<ol>" } else { "<ul>" }),
        Some(n) => out.push_str(&format!("<ol start=\"{n}\">")),
    }
    let mut loose = false;
    let mut items: Vec<Vec<&str>> = Vec::new();
    while i < lines.len() {
        let Some((n2, m2, col, first)) = list_marker(lines[i]) else {
            break;
        };
        if n2.is_some() != ordered || m2 != mark {
            break;
        }
        let mut item = vec![first];
        i += 1;
        let mut blank = false;
        while i < lines.len() {
            let l = lines[i];
            if l.trim().is_empty() {
                blank = true;
                item.push("");
                i += 1;
                continue;
            }
            if indent(l) >= col {
                if blank {
                    loose = true;
                }
                item.push(dedent(l, col));
                blank = false;
                i += 1;
                continue;
            }
            // A lazy continuation of the item's paragraph.
            if !blank && !starts_block(l) && list_marker(l).is_none() {
                item.push(l.trim_start());
                i += 1;
                continue;
            }
            break;
        }
        while item.last().is_some_and(|l| l.trim().is_empty()) {
            item.pop();
        }
        items.push(item);
        // A blank line, then another item: the list is loose.
        if blank
            && list_marker(lines.get(i).copied().unwrap_or(""))
                .is_some_and(|(n, m, _, _)| n.is_some() == ordered && m == mark)
        {
            loose = true;
        }
    }
    for item in items {
        out.push_str("<li>");
        let (check, item) = task(item);
        if let Some(done) = check {
            out.push_str(if done { "☑ " } else { "☐ " });
        }
        let simple = !loose
            && item
                .iter()
                .all(|l| !l.trim().is_empty() && !starts_block(l) && indent(l) < 4);
        if simple {
            inline(item.join("\n").trim(), ctx, out);
        } else {
            blocks(&item, ctx, out, depth + 1);
        }
        out.push_str("</li>\n");
    }
    out.push_str(if ordered { "</ol>\n" } else { "</ul>\n" });
    i
}

/// A task list item's box (`[ ]`, `[x]`) taken off its first line.
fn task(mut item: Vec<&str>) -> (Option<bool>, Vec<&str>) {
    let Some(first) = item.first().copied() else {
        return (None, item);
    };
    for (pat, done) in [("[ ] ", false), ("[x] ", true), ("[X] ", true)] {
        if let Some(rest) = first.strip_prefix(pat) {
            item[0] = rest;
            return (Some(done), item);
        }
    }
    (None, item)
}

/// The text of rendered inline markup (headings for the contents).
fn strip_tags(h: &str) -> String {
    let mut out = String::with_capacity(h.len());
    let mut tag = false;
    for c in h.chars() {
        match c {
            '<' => tag = true,
            '>' if tag => tag = false,
            _ if !tag => out.push(c),
            _ => {}
        }
    }
    let out = out
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

const ENTITIES: &[(&str, &str)] = &[
    ("amp", "&"),
    ("lt", "<"),
    ("gt", ">"),
    ("quot", "\""),
    ("apos", "'"),
    ("nbsp", "\u{a0}"),
    ("copy", "©"),
    ("reg", "®"),
    ("trade", "™"),
    ("mdash", "—"),
    ("ndash", "–"),
    ("hellip", "…"),
    ("laquo", "«"),
    ("raquo", "»"),
    ("deg", "°"),
    ("times", "×"),
    ("euro", "€"),
];

/// An entity at the start of `s` (after `&`): its text and length.
fn entity(s: &str) -> Option<(String, usize)> {
    let end = s.find(';').filter(|&e| e > 0 && e <= 10)?;
    let name = &s[..end];
    let c = if let Some(n) = name.strip_prefix('#') {
        let v = match n.strip_prefix(['x', 'X']) {
            Some(h) => u32::from_str_radix(h, 16).ok()?,
            None => n.parse().ok()?,
        };
        char::from_u32(v).filter(|c| *c != '\0')?.to_string()
    } else {
        ENTITIES.iter().find(|(k, _)| *k == name)?.1.to_owned()
    };
    Some((c, end + 1))
}

fn is_remote(src: &str) -> bool {
    let lower = src.to_ascii_lowercase();
    lower.starts_with("//")
        || lower.split_once(':').is_some_and(|(scheme, _)| {
            scheme.len() > 1
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
        })
}

impl Ctx<'_> {
    /// The `images/` name of the picture at `src`, loading it once.
    fn picture(&mut self, src: &str) -> Option<String> {
        if let Some(n) = self.by_src.get(src) {
            return n.clone();
        }
        let name = if is_remote(src) || src.starts_with('/') || self.pictures.len() >= MAX_PICTURES
        {
            None
        } else {
            let decoded = percent_decode(src);
            (self.load)(&decoded).map(|bytes| {
                let name = format!("p{}", self.pictures.len());
                self.pictures.push((name.clone(), bytes));
                name
            })
        };
        if name.is_none() {
            self.missing += 1;
        }
        self.by_src.insert(src.to_owned(), name.clone());
        name
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(v) =
                u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16)
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

/// `[text](dest "title")` or `[text][ref]` after the `[` at `s[0]`: (text, destination,
/// bytes used).
fn link_at(s: &str, ctx: &Ctx) -> Option<(String, String, usize)> {
    // The matching `]`, brackets nested, code spans and escapes skipped.
    let b = s.as_bytes();
    let mut depth = 0;
    let mut i = 0;
    let close = loop {
        if i >= b.len() {
            return None;
        }
        match b[i] {
            b'\\' => i += 1,
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    break i;
                }
            }
            _ => {}
        }
        i += 1;
    };
    let text = s[1..close].to_owned();
    let rest = &s[close + 1..];
    if let Some(r) = rest.strip_prefix('(') {
        let end = {
            let mut d = 1;
            let mut k = None;
            for (j, c) in r.char_indices() {
                match c {
                    '(' => d += 1,
                    ')' => {
                        d -= 1;
                        if d == 0 {
                            k = Some(j);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            k?
        };
        let inner = r[..end].trim();
        let dest = if let Some(x) = inner.strip_prefix('<') {
            x.split('>').next().unwrap_or("")
        } else {
            inner.split_whitespace().next().unwrap_or("")
        };
        return Some((text, dest.to_owned(), close + 2 + end + 1));
    }
    let (key, used) = if let Some(r) = rest.strip_prefix('[') {
        let e = r.find(']')?;
        let k = if e == 0 {
            text.clone()
        } else {
            r[..e].to_owned()
        };
        (k, close + 1 + e + 2)
    } else {
        (text.clone(), close + 1)
    };
    let (dest, _) = ctx.refs.get(&key.to_lowercase())?;
    Some((text, dest.clone(), used))
}

/// Writes inline markup for `s`.
fn inline(s: &str, ctx: &mut Ctx, out: &mut String) {
    inline_depth(s, ctx, out, 0);
}

fn inline_depth(s: &str, ctx: &mut Ctx, out: &mut String, depth: usize) {
    if depth > 16 {
        out.push_str(&esc(s));
        return;
    }
    let mut i = 0;
    let b = s.as_bytes();
    let mut text = String::new();
    let flush = |text: &mut String, out: &mut String| {
        out.push_str(&esc(text));
        text.clear();
    };
    while i < b.len() {
        let c = b[i];
        match c {
            b'\\' if i + 1 < b.len() => {
                let n = b[i + 1];
                if n == b'\n' {
                    flush(&mut text, out);
                    out.push_str("<br/>");
                    i += 2;
                    continue;
                }
                if n.is_ascii_punctuation() {
                    text.push(n as char);
                    i += 2;
                    continue;
                }
                text.push('\\');
                i += 1;
            }
            b'\n' => {
                // Two trailing spaces: a hard break.
                if text.ends_with("  ") {
                    let t = text.trim_end_matches(' ').len();
                    text.truncate(t);
                    flush(&mut text, out);
                    out.push_str("<br/>");
                } else {
                    let t = text.trim_end_matches(' ').len();
                    text.truncate(t);
                    text.push(' ');
                }
                i += 1;
            }
            b'`' => {
                let n = s[i..].bytes().take_while(|&x| x == b'`').count();
                let fence = &s[i..i + n];
                match s[i + n..]
                    .find(fence)
                    .filter(|&e| s.as_bytes()[i + n + e + n..].first() != Some(&b'`'))
                {
                    Some(e) => {
                        flush(&mut text, out);
                        let code = s[i + n..i + n + e].replace('\n', " ");
                        let code =
                            if code.starts_with(' ') && code.ends_with(' ') && code.trim() != "" {
                                code[1..code.len() - 1].to_owned()
                            } else {
                                code
                            };
                        out.push_str("<code>");
                        out.push_str(&esc(&code));
                        out.push_str("</code>");
                        i += n + e + n;
                    }
                    None => {
                        text.push_str(fence);
                        i += n;
                    }
                }
            }
            b'!' if b.get(i + 1) == Some(&b'[') => match link_at(&s[i + 1..], ctx) {
                Some((alt, dest, used)) => {
                    flush(&mut text, out);
                    let alt_plain = alt.replace(['*', '_', '`'], "");
                    match ctx.picture(&dest) {
                        Some(name) => out.push_str(&format!(
                            "<img src=\"images/{}\" alt=\"{}\"/>",
                            esc(&name),
                            esc(&alt_plain)
                        )),
                        None => out.push_str(&format!(
                            "<span class=\"missing\">[{}]</span>",
                            esc(if alt_plain.is_empty() {
                                &dest
                            } else {
                                &alt_plain
                            })
                        )),
                    }
                    i += 1 + used;
                }
                None => {
                    text.push('!');
                    i += 1;
                }
            },
            b'[' => match link_at(&s[i..], ctx) {
                Some((label, _dest, used)) => {
                    flush(&mut text, out);
                    out.push_str("<u class=\"link\">");
                    inline_depth(&label, ctx, out, depth + 1);
                    out.push_str("</u>");
                    i += used;
                }
                None => {
                    text.push('[');
                    i += 1;
                }
            },
            b'<' => {
                // An autolink `<https://…>` or `<a@b.c>`: shown, not followed.
                let rest = &s[i + 1..];
                match rest.find('>') {
                    Some(e)
                        if !rest[..e].contains([' ', '<', '\n'])
                            && (is_remote(&rest[..e]) || rest[..e].contains('@')) =>
                    {
                        flush(&mut text, out);
                        out.push_str("<u class=\"link\">");
                        out.push_str(&esc(&rest[..e]));
                        out.push_str("</u>");
                        i += e + 2;
                    }
                    _ => {
                        text.push('<');
                        i += 1;
                    }
                }
            }
            b'&' => match entity(&s[i + 1..]) {
                Some((t, used)) => {
                    text.push_str(&t);
                    i += 1 + used;
                }
                None => {
                    text.push('&');
                    i += 1;
                }
            },
            b'*' | b'_' | b'~' => {
                let n = s[i..].bytes().take_while(|&x| x == c).count();
                let run = &s[i..i + n];
                // Intraword underscores are not emphasis.
                let prev = s[..i].chars().next_back();
                let next = s[i + n..].chars().next();
                let opens = next.is_some_and(|ch| !ch.is_whitespace())
                    && !(c == b'_' && prev.is_some_and(char::is_alphanumeric));
                let tag = match (c, n) {
                    (b'~', 2) => Some(("del", 2)),
                    (b'~', _) => None,
                    (_, 1) => Some(("em", 1)),
                    (_, 2) => Some(("strong", 2)),
                    (_, _) => Some(("strong", 2)),
                };
                if let (true, Some((tag, len))) = (opens, tag) {
                    let marker = &run[..len];
                    if let Some(e) = find_closer(&s[i + len..], marker, c) {
                        flush(&mut text, out);
                        if n == 3 && c != b'~' {
                            out.push_str("<em>");
                        }
                        out.push_str(&format!("<{tag}>"));
                        let inner_start = i + if n == 3 && c != b'~' { 3 } else { len };
                        let inner_end = i + len + e;
                        let inner = &s[inner_start.min(inner_end)..inner_end];
                        inline_depth(inner, ctx, out, depth + 1);
                        out.push_str(&format!("</{tag}>"));
                        let mut used = len + e + len;
                        if n == 3 && c != b'~' {
                            out.push_str("</em>");
                            if s[i + used..].starts_with(c as char) {
                                used += 1;
                            }
                        }
                        i += used;
                        continue;
                    }
                }
                text.push_str(run);
                i += n;
            }
            _ => {
                // Whole characters.
                let ch = s[i..].chars().next().unwrap_or('\u{fffd}');
                text.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    flush(&mut text, out);
}

/// Where the closing `marker` of an emphasis run is in `s` (not after whitespace, not part
/// of a longer run unless it is the run's end).
fn find_closer(s: &str, marker: &str, c: u8) -> Option<usize> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'`' => {
                // Code spans hide markers.
                let n = s[i..].bytes().take_while(|&x| x == b'`').count();
                match s[i + n..].find(&s[i..i + n]) {
                    Some(e) => i += n + e + n,
                    None => i += n,
                }
            }
            x if x == c => {
                let n = s[i..].bytes().take_while(|&y| y == c).count();
                let after_space = i == 0 || b[i - 1].is_ascii_whitespace();
                let next = s[i + n..].chars().next();
                let intraword = c == b'_' && next.is_some_and(char::is_alphanumeric);
                if !after_space && !intraword && n >= marker.len() && i > 0 {
                    // Use the run's last characters, so `**a**b` closes at the run.
                    return Some(i + n - marker.len());
                }
                i += n;
            }
            _ => i += 1,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html(md: &str) -> String {
        to_xhtml(md, &|_| None).section.body
    }

    #[test]
    fn blocks_render() {
        let h = html(
            "# Title\n\nSome *em* and **strong** and `co*de*`.\n\nSetext\n---\n\n> quoted\n> more\n\n\
             - one\n- two\n  - nested\n\n1. first\n2. second\n\n```rust\nfn x() {}\n```\n\n    indented\n\n***\n",
        );
        assert!(h.contains("<h1 id=\"h0\">Title</h1>"), "{h}");
        assert!(
            h.contains("<em>em</em>") && h.contains("<strong>strong</strong>"),
            "{h}"
        );
        assert!(h.contains("<code>co*de*</code>"), "{h}");
        assert!(h.contains("<h2 id=\"h1\">Setext</h2>"), "{h}");
        assert!(h.contains("<blockquote><p>quoted more</p>"), "{h}");
        assert!(h.contains("<ul><li>one</li>"), "{h}");
        assert!(
            h.contains("<li>two<ul><li>nested</li>") || h.contains("<li><p>two</p>"),
            "{h}"
        );
        assert!(h.contains("<ol><li>first</li>"), "{h}");
        assert!(
            h.contains("<code class=\"language-rust\">fn x() {}</code>"),
            "{h}"
        );
        assert!(h.contains("<pre><code>indented</code></pre>"), "{h}");
        assert!(h.contains("<hr/>"), "{h}");
    }

    #[test]
    fn inlines_and_safety() {
        let h = html(
            "a [link](http://x.org \"t\") and <https://y.org> & <b>raw</b> 5 * 3 x_y_z\\*\nline  \nbreak ~~gone~~ &copy;",
        );
        assert!(h.contains("<u class=\"link\">link</u>"), "{h}");
        assert!(h.contains("<u class=\"link\">https://y.org</u>"), "{h}");
        assert!(h.contains("&amp; &lt;b&gt;raw&lt;/b&gt;"), "{h}");
        assert!(h.contains("5 * 3 x_y_z*"), "{h}");
        assert!(h.contains("line<br/>break"), "{h}");
        assert!(h.contains("<del>gone</del> ©"), "{h}");
        assert!(!h.contains("href"), "{h}");
    }

    #[test]
    fn tables_tasks_refs() {
        let h = html(
            "| a | b |\n|:--|--:|\n| 1 | `x|y` |\n| 2 |\n\n- [ ] todo\n- [x] done\n\n[ref link][r]\n\n[r]: https://e.org\n",
        );
        assert!(
            h.contains("<th style=\"text-align:left\">a</th><th style=\"text-align:right\">b</th>"),
            "{h}"
        );
        assert!(h.contains("<code>x|y</code>"), "{h}");
        assert!(
            h.contains("<td style=\"text-align:left\">2</td><td style=\"text-align:right\"></td>"),
            "{h}"
        );
        assert!(h.contains("☐ todo") && h.contains("☑ done"), "{h}");
        assert!(h.contains("<u class=\"link\">ref link</u>"), "{h}");
        assert!(!h.contains("[r]:"), "{h}");
    }

    #[test]
    fn pictures_load_locally_only() {
        let png = b"\x89PNG\r\n\x1a\nrest".to_vec();
        let asked = std::cell::RefCell::new(Vec::new());
        let r = to_xhtml(
            "![logo](img/logo%20a.png) ![remote](https://e.org/x.png) ![abs](/etc/x.png) ![logo again](img/logo%20a.png)",
            &|p| {
                asked.borrow_mut().push(p.to_owned());
                Some(png.clone())
            },
        );
        assert_eq!(*asked.borrow(), ["img/logo a.png"]);
        assert_eq!(r.pictures.len(), 1);
        assert_eq!(r.missing_pictures, 2);
        assert!(
            r.section
                .body
                .contains("<img src=\"images/p0\" alt=\"logo\"/>")
        );
        assert!(r.section.body.contains("[remote]"));
    }

    #[test]
    fn headings_feed_the_contents() {
        let r = to_xhtml("Intro\n\n## A *b*\n### [C](x)\n", &|_| None);
        assert_eq!(
            r.section.headings[0],
            ("A b".to_owned(), "h0".to_owned(), 1)
        );
        assert_eq!(r.section.headings[1].0, "C");
        assert_eq!(r.title.as_deref(), Some("A b"));
        // Deep nesting does not recurse without end.
        let deep = ">".repeat(200) + " x";
        assert!(html(&deep).contains("x"));
    }
}
