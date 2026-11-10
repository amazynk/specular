//! The manual's page format: a small Markdown subset with front matter, parsed into blocks
//! of inline spans. Nothing in it is HTML; the help panel draws the blocks itself.
//!
//! ```text
//! ---
//! id: viewer
//! title: Viewing images
//! section: modes
//! contexts: viewer
//! keywords: zoom, pan, rotate
//! ---
//! An opening paragraph.
//!
//! ## Zooming {#zoom}
//! Press [[key:zoom-in]] to magnify. **Bold**, `code`, [links](gallery#marks).
//!
//! - a list item
//! 1. a numbered step
//! > A note.
//!
//! ::: keys viewer
//! ```
//!
//! `[[key:<action id>]]` shows the action's key as bound now; `::: keys <table>` the whole key
//! table of a mode (generated from the keymap); `::: commands` points at the palette.

/// Where a page is listed in the contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Section {
    /// Getting started.
    Start,
    /// One page per mode or profile.
    Modes,
    /// "How do I…" pages.
    Tasks,
    /// Keys, settings, privacy, troubleshooting.
    Reference,
}

impl Section {
    pub const ALL: [Section; 4] = [
        Section::Start,
        Section::Modes,
        Section::Tasks,
        Section::Reference,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Section::Start => "start",
            Section::Modes => "modes",
            Section::Tasks => "tasks",
            Section::Reference => "reference",
        }
    }

    pub fn from_id(s: &str) -> Option<Section> {
        Section::ALL.into_iter().find(|x| x.id() == s)
    }

    /// Fluent message ID of the section's heading in the contents.
    pub fn message_id(self) -> &'static str {
        match self {
            Section::Start => "help-section-start",
            Section::Modes => "help-section-modes",
            Section::Tasks => "help-section-tasks",
            Section::Reference => "help-section-reference",
        }
    }
}

/// A piece of inline text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Span {
    Text(String),
    Strong(String),
    Code(String),
    /// A link to a page (empty: this one) and an anchor in it (empty: its top).
    Link {
        text: String,
        page: String,
        anchor: String,
    },
    /// The key bound to an action (its `keymap.toml` id).
    Key(String),
}

/// A block of a page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Heading {
        level: u8,
        text: Vec<Span>,
        anchor: String,
    },
    Para(Vec<Span>),
    Bullet(Vec<Span>),
    Numbered(u32, Vec<Span>),
    Note(Vec<Span>),
    /// A key table, generated from the keymap: `viewer`, `gallery`, `compare`, `cull`,
    /// `medical`, `science`, `drawing`, `mesh` or `relight`.
    Keys(String),
    /// "Every command is in the palette": a pointer to it with its key.
    Commands,
}

/// A parsed page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    pub id: String,
    pub title: String,
    pub section: Section,
    /// Places the page is the help for (F1 there opens it).
    pub contexts: Vec<String>,
    pub keywords: Vec<String>,
    pub blocks: Vec<Block>,
}

/// A problem in a page's source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl Page {
    /// Every anchor the page defines.
    pub fn anchors(&self) -> impl Iterator<Item = &str> {
        self.blocks.iter().filter_map(|b| match b {
            Block::Heading { anchor, .. } => Some(anchor.as_str()),
            _ => None,
        })
    }

    /// Every link: (page, anchor), the page filled in for links within this one.
    pub fn links(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for b in &self.blocks {
            for s in spans_of(b) {
                if let Span::Link { page, anchor, .. } = s {
                    let page = if page.is_empty() { &self.id } else { page };
                    out.push((page.clone(), anchor.clone()));
                }
            }
        }
        out
    }

    /// Every action named by `[[key:…]]`.
    pub fn key_refs(&self) -> Vec<&str> {
        let mut out = Vec::new();
        for b in &self.blocks {
            for s in spans_of(b) {
                if let Span::Key(id) = s {
                    out.push(id.as_str());
                }
            }
        }
        out
    }

    /// The structure translations must keep: anchors, key tables, key references and
    /// links, in order.
    pub fn skeleton(&self) -> Vec<String> {
        let mut out = vec![format!("section:{}", self.section.id())];
        for b in &self.blocks {
            match b {
                Block::Heading { anchor, .. } => out.push(format!("#{anchor}")),
                Block::Keys(k) => out.push(format!("keys:{k}")),
                Block::Commands => out.push("commands".into()),
                _ => {}
            }
            for s in spans_of(b) {
                match s {
                    Span::Key(id) => out.push(format!("key:{id}")),
                    Span::Link { page, anchor, .. } => out.push(format!("link:{page}#{anchor}")),
                    _ => {}
                }
            }
        }
        out
    }
}

/// The spans of a block (none for the generated ones).
pub fn spans_of(b: &Block) -> &[Span] {
    match b {
        Block::Heading { text, .. }
        | Block::Para(text)
        | Block::Bullet(text)
        | Block::Numbered(_, text)
        | Block::Note(text) => text,
        Block::Keys(_) | Block::Commands => &[],
    }
}

/// The text of spans as read, with `key` giving the key shown for an action id.
pub fn plain(spans: &[Span], key: &dyn Fn(&str) -> String) -> String {
    let mut s = String::new();
    for sp in spans {
        match sp {
            Span::Text(t) | Span::Strong(t) | Span::Code(t) => s.push_str(t),
            Span::Link { text, .. } => s.push_str(text),
            Span::Key(id) => s.push_str(&key(id)),
        }
    }
    s
}

/// A heading's anchor when it names none: its words, lowercased, joined by '-'.
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for w in s
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        if !out.is_empty() {
            out.push('-');
        }
        out.extend(w.chars().flat_map(char::to_lowercase));
    }
    out
}

/// Parses one page.
pub fn parse(src: &str) -> Result<Page, ParseError> {
    let err = |line: usize, m: &str| ParseError {
        line,
        message: m.to_owned(),
    };
    let mut lines = src.lines().enumerate().peekable();
    match lines.next() {
        Some((_, l)) if l.trim() == "---" => {}
        _ => return Err(err(1, "a page starts with front matter (---)")),
    }
    let (mut id, mut title, mut section) = (None, None, None);
    let (mut contexts, mut keywords) = (Vec::new(), Vec::new());
    let list = |v: &str| -> Vec<String> {
        v.split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect()
    };
    loop {
        let Some((n, l)) = lines.next() else {
            return Err(err(1, "the front matter has no closing ---"));
        };
        let l = l.trim();
        if l == "---" {
            break;
        }
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let Some((k, v)) = l.split_once(':') else {
            return Err(err(n + 1, "front matter lines are `key: value`"));
        };
        let v = v.trim();
        match k.trim() {
            "id" => id = Some(v.to_owned()),
            "title" => title = Some(v.to_owned()),
            "section" => {
                section = Some(Section::from_id(v).ok_or_else(|| err(n + 1, "unknown section"))?)
            }
            "contexts" => contexts = list(v),
            "keywords" => keywords = list(v),
            _ => return Err(err(n + 1, "unknown front matter key")),
        }
    }
    let id = id.ok_or_else(|| err(1, "no id"))?;
    let title = title.ok_or_else(|| err(1, "no title"))?;
    let section = section.ok_or_else(|| err(1, "no section"))?;

    let mut blocks = Vec::new();
    // Lines of the paragraph (or list item, note) being read, and its kind.
    let mut cur: Option<(usize, Kind, String)> = None;
    let flush = |cur: &mut Option<(usize, Kind, String)>,
                 blocks: &mut Vec<Block>|
     -> Result<(), ParseError> {
        if let Some((n, kind, text)) = cur.take() {
            let spans = inline(&text).map_err(|m| err(n, &m))?;
            blocks.push(match kind {
                Kind::Para => Block::Para(spans),
                Kind::Bullet => Block::Bullet(spans),
                Kind::Numbered(k) => Block::Numbered(k, spans),
                Kind::Note => Block::Note(spans),
            });
        }
        Ok(())
    };
    for (n, raw) in lines {
        let n = n + 1;
        let l = raw.trim_end();
        let t = l.trim_start();
        if t.is_empty() {
            flush(&mut cur, &mut blocks)?;
            continue;
        }
        if let Some(rest) = t.strip_prefix(":::") {
            flush(&mut cur, &mut blocks)?;
            let mut w = rest.split_whitespace();
            match (w.next(), w.next(), w.next()) {
                (Some("keys"), Some(table), None) => blocks.push(Block::Keys(table.to_owned())),
                (Some("commands"), None, None) => blocks.push(Block::Commands),
                _ => return Err(err(n, "unknown directive")),
            }
            continue;
        }
        if t.starts_with('#') {
            flush(&mut cur, &mut blocks)?;
            let level = t.chars().take_while(|c| *c == '#').count();
            if !(2..=3).contains(&level) {
                return Err(err(n, "headings are ## or ### (the title is the page's)"));
            }
            let mut text = t[level..].trim();
            let mut anchor = None;
            if let Some(open) = text.rfind("{#")
                && text.ends_with('}')
            {
                anchor = Some(text[open + 2..text.len() - 1].trim().to_owned());
                text = text[..open].trim();
            }
            let spans = inline(text).map_err(|m| err(n, &m))?;
            let anchor = anchor.unwrap_or_else(|| slug(&plain(&spans, &|k| k.to_owned())));
            blocks.push(Block::Heading {
                level: level as u8,
                text: spans,
                anchor,
            });
            continue;
        }
        let start = if let Some(rest) = t.strip_prefix("- ") {
            Some((Kind::Bullet, rest))
        } else if let Some(rest) = t.strip_prefix("> ") {
            Some((Kind::Note, rest))
        } else if let Some((num, rest)) = t.split_once(". ")
            && !num.is_empty()
            && num.chars().all(|c| c.is_ascii_digit())
        {
            Some((Kind::Numbered(num.parse().unwrap_or(1)), rest))
        } else {
            None
        };
        match (start, &mut cur) {
            // `>` again: the note goes on.
            (Some((Kind::Note, rest)), Some((_, Kind::Note, text))) => {
                text.push(' ');
                text.push_str(rest.trim());
            }
            (Some((kind, rest)), _) => {
                flush(&mut cur, &mut blocks)?;
                cur = Some((n, kind, rest.trim().to_owned()));
            }
            // A continuation line (indented under a list item, or the next line of a
            // paragraph or note: `>` again or not).
            (None, Some((_, kind, text))) => {
                let t = if *kind == Kind::Note {
                    t.strip_prefix('>').map_or(t, str::trim_start)
                } else {
                    t
                };
                text.push(' ');
                text.push_str(t);
            }
            (None, None) => cur = Some((n, Kind::Para, t.to_owned())),
        }
    }
    flush(&mut cur, &mut blocks)?;
    Ok(Page {
        id,
        title,
        section,
        contexts,
        keywords,
        blocks,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Para,
    Bullet,
    Numbered(u32),
    Note,
}

/// Inline markup: `**strong**`, `` `code` ``, `[text](page#anchor)`, `[[key:action]]`.
fn inline(s: &str) -> Result<Vec<Span>, String> {
    let mut out = Vec::new();
    let mut text = String::new();
    let mut rest = s;
    let push_text = |out: &mut Vec<Span>, text: &mut String| {
        if !text.is_empty() {
            out.push(Span::Text(std::mem::take(text)));
        }
    };
    while let Some(c) = rest.chars().next() {
        if let Some(r) = rest.strip_prefix("[[") {
            let end = r.find("]]").ok_or("unclosed [[")?;
            let inner = &r[..end];
            let id = inner
                .strip_prefix("key:")
                .ok_or("[[…]] is [[key:<action>]]")?;
            push_text(&mut out, &mut text);
            out.push(Span::Key(id.trim().to_owned()));
            rest = &r[end + 2..];
        } else if let Some(r) = rest.strip_prefix("**") {
            let end = r.find("**").ok_or("unclosed **")?;
            push_text(&mut out, &mut text);
            out.push(Span::Strong(r[..end].to_owned()));
            rest = &r[end + 2..];
        } else if let Some(r) = rest.strip_prefix('`') {
            let end = r.find('`').ok_or("unclosed `")?;
            push_text(&mut out, &mut text);
            out.push(Span::Code(r[..end].to_owned()));
            rest = &r[end + 1..];
        } else if let Some(r) = rest.strip_prefix('[')
            && let Some(close) = r.find("](")
            && let Some(end) = r[close + 2..].find(')')
        {
            let label = &r[..close];
            let target = &r[close + 2..close + 2 + end];
            let (page, anchor) = target.split_once('#').unwrap_or((target, ""));
            push_text(&mut out, &mut text);
            out.push(Span::Link {
                text: label.to_owned(),
                page: page.to_owned(),
                anchor: anchor.to_owned(),
            });
            rest = &r[close + 2 + end + 1..];
        } else {
            text.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    push_text(&mut out, &mut text);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "---\nid: demo\ntitle: Demo page\nsection: tasks\ncontexts: viewer, gallery\nkeywords: one, two\n---\nFirst line\ncontinues here.\n\n## Zooming in {#zoom}\nPress [[key:zoom-in]] or see [the gallery](gallery#marks) and [here](#zoom).\n\n- **Bold** item\n  continued\n1. Step `one`\n2. Step two\n> Note: careful\n> still the note\n\n::: keys viewer\n::: commands\n### Plain heading\n";

    #[test]
    fn parses_every_block() {
        let p = parse(SRC).unwrap();
        assert_eq!(p.id, "demo");
        assert_eq!(p.section, Section::Tasks);
        assert_eq!(p.contexts, ["viewer", "gallery"]);
        assert_eq!(p.keywords, ["one", "two"]);
        assert_eq!(
            p.blocks[0],
            Block::Para(vec![Span::Text("First line continues here.".into())])
        );
        assert!(
            matches!(&p.blocks[1], Block::Heading { level: 2, anchor, .. } if anchor == "zoom")
        );
        let Block::Para(s) = &p.blocks[2] else {
            panic!()
        };
        assert_eq!(s[1], Span::Key("zoom-in".into()));
        assert_eq!(
            s[3],
            Span::Link {
                text: "the gallery".into(),
                page: "gallery".into(),
                anchor: "marks".into()
            }
        );
        assert_eq!(
            p.blocks[3],
            Block::Bullet(vec![
                Span::Strong("Bold".into()),
                Span::Text(" item continued".into())
            ])
        );
        assert!(matches!(p.blocks[4], Block::Numbered(1, _)));
        assert!(matches!(p.blocks[5], Block::Numbered(2, _)));
        assert_eq!(
            p.blocks[6],
            Block::Note(vec![Span::Text("Note: careful still the note".into())])
        );
        assert_eq!(p.blocks[7], Block::Keys("viewer".into()));
        assert_eq!(p.blocks[8], Block::Commands);
        assert!(
            matches!(&p.blocks[9], Block::Heading { level: 3, anchor, .. } if anchor == "plain-heading")
        );
        assert_eq!(
            p.links(),
            [
                ("gallery".to_owned(), "marks".to_owned()),
                ("demo".to_owned(), "zoom".to_owned())
            ]
        );
        assert_eq!(p.key_refs(), ["zoom-in"]);
    }

    #[test]
    fn rejects_broken_sources() {
        assert!(parse("no front matter").is_err());
        assert!(parse("---\nid: a\ntitle: b\n---\n").is_err(), "no section");
        let base = "---\nid: a\ntitle: b\nsection: start\n---\n";
        assert!(parse(&format!("{base}# Title again")).is_err());
        assert!(parse(&format!("{base}::: video x")).is_err());
        assert!(parse(&format!("{base}**open")).is_err());
        assert!(parse(&format!("{base}[[action:x]]")).is_err());
        let e = parse(&format!("{base}ok\n\n`open")).unwrap_err();
        assert_eq!(e.line, 8);
    }
}
