//! Full-text search over the manual: an inverted index of folded words (case and accents
//! ignored, as everywhere else in Specular) over sections of pages, built once per language.
//! A query's words must all match; the last one matches as a prefix, so results follow the
//! typing. Words in a page's title count most, then its keywords (both for every section of
//! the page), then a section's heading, then its text.

use std::collections::HashMap;

use specular_i18n::fold_for_search;

use crate::doc::{Block, Page, plain, spans_of};

const W_TITLE: u32 = 8;
const W_KEYWORD: u32 = 5;
const W_HEADING: u32 = 4;
const W_BODY: u32 = 1;

/// A section of a page: from a heading to the next (the first runs from the top).
#[derive(Clone, Debug)]
pub(crate) struct Chunk {
    pub page: usize,
    pub anchor: String,
    pub heading: String,
    /// The section's text, for snippets.
    pub text: String,
}

/// A search result.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    /// Index into [`crate::Manual::pages`].
    pub page: usize,
    /// The section's anchor (empty: the page's top).
    pub anchor: String,
    /// The section's heading (empty: the page's top).
    pub heading: String,
    /// The sentence that matched, shortened.
    pub snippet: String,
    pub score: u32,
}

type Words = Vec<(String, Vec<(u32, u32)>)>;
type Postings = HashMap<String, HashMap<u32, u32>>;

#[derive(Default)]
pub(crate) struct Index {
    pub chunks: Vec<Chunk>,
    /// Words of sections (headings and text) in sorted order, each with (chunk, weight)
    /// postings.
    words: Words,
    /// Words of whole pages (titles and keywords), with (page, weight) postings: they count
    /// for every section of the page.
    page_words: Words,
}

/// Lowercased, accents and marks removed.
pub fn fold(s: &str) -> String {
    fold_for_search(s).into_owned()
}

/// The words of folded text.
pub(crate) fn words(s: &str) -> impl Iterator<Item = &str> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
}

/// A word and its plural-less form ("images" also finds "image").
fn forms(w: &str) -> impl Iterator<Item = &str> {
    let short = (w.len() > 3 && w.ends_with('s') && !w.ends_with("ss")).then(|| &w[..w.len() - 1]);
    std::iter::once(w).chain(short)
}

fn add(map: &mut Postings, at: usize, s: &str, w: u32) {
    let f = fold(s);
    for word in words(&f) {
        for form in forms(word) {
            let e = map
                .entry(form.to_owned())
                .or_default()
                .entry(at as u32)
                .or_default();
            *e = (*e).max(w);
        }
    }
}

fn sorted(map: Postings) -> Words {
    let mut words: Words = map
        .into_iter()
        .map(|(w, m)| {
            let mut v: Vec<(u32, u32)> = m.into_iter().collect();
            v.sort_unstable();
            (w, v)
        })
        .collect();
    words.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    words
}

/// Where the word occurs, with its weight there: the word exactly, or (`prefix`) every word
/// that starts with it, best weight per place. A word that only starts with the query counts
/// a little less than the word itself.
fn postings(list: &Words, w: &str, prefix: bool) -> HashMap<u32, u32> {
    let mut out = HashMap::new();
    let start = list.partition_point(|(x, _)| x.as_str() < w);
    for (x, p) in &list[start..] {
        let hit = if prefix { x.starts_with(w) } else { x == w };
        if !hit {
            break;
        }
        for &(c, wt) in p {
            let wt = if x == w { wt * 2 } else { wt * 2 - 1 };
            let e = out.entry(c).or_insert(0);
            *e = (*e).max(wt);
        }
    }
    out
}

impl Index {
    /// `table_words`: the words of a generated key table (its actions' names); `key_name`:
    /// the name of the action a `[[key:…]]` names, which stands for it in the text.
    pub(crate) fn build(
        pages: &[Page],
        table_words: &dyn Fn(&str) -> String,
        key_name: &dyn Fn(&str) -> String,
    ) -> Index {
        let mut chunks = Vec::new();
        let mut map = Postings::new();
        let mut page_map = Postings::new();
        let key = |id: &str| id.replace('-', " ");
        for (pi, p) in pages.iter().enumerate() {
            let mut chunk = chunks.len();
            chunks.push(Chunk {
                page: pi,
                anchor: String::new(),
                heading: String::new(),
                text: String::new(),
            });
            add(&mut page_map, pi, &p.title, W_TITLE);
            for k in &p.keywords {
                add(&mut page_map, pi, k, W_KEYWORD);
            }
            for b in &p.blocks {
                match b {
                    Block::Heading { text, anchor, .. } => {
                        let h = plain(text, &key);
                        chunk = chunks.len();
                        chunks.push(Chunk {
                            page: pi,
                            anchor: anchor.clone(),
                            heading: h.clone(),
                            text: String::new(),
                        });
                        add(&mut map, chunk, &h, W_HEADING);
                    }
                    Block::Keys(t) => add(&mut map, chunk, &table_words(t), W_BODY),
                    Block::Commands => {}
                    b => {
                        let s = plain(spans_of(b), key_name);
                        add(&mut map, chunk, &s, W_BODY);
                        let text = &mut chunks[chunk].text;
                        if !text.is_empty() {
                            text.push(' ');
                        }
                        text.push_str(&s);
                    }
                }
            }
        }
        Index {
            chunks,
            words: sorted(map),
            page_words: sorted(page_map),
        }
    }

    /// Sections matching every word of the query, best first. A section's score adds, per
    /// word, its own best weight (heading or text) and its page's (title or keywords). A
    /// section is a result when one of the words is in it; a page's top also when the words
    /// are only in its title or keywords.
    pub(crate) fn search(&self, query: &str, limit: usize) -> Vec<Hit> {
        let q = fold(query);
        let terms: Vec<&str> = words(&q).collect();
        let Some((last, rest)) = terms.split_last() else {
            return Vec::new();
        };
        // The last word as typed so far, unless the query ends with a space.
        let open = !q.ends_with(char::is_whitespace);
        let n = self.chunks.len();
        let (mut score, mut alive, mut own) = (vec![0u32; n], vec![true; n], vec![false; n]);
        for (t, prefix) in rest
            .iter()
            .map(|t| (*t, false))
            .chain(std::iter::once((*last, open)))
        {
            let mut local = postings(&self.words, t, prefix);
            let mut page = postings(&self.page_words, t, prefix);
            // Plurals typed in full: "images" finds "image".
            if !prefix
                && local.is_empty()
                && page.is_empty()
                && let Some(short) = forms(t).nth(1)
            {
                local = postings(&self.words, short, false);
                page = postings(&self.page_words, short, false);
            }
            for (c, ch) in self.chunks.iter().enumerate() {
                let l = local.get(&(c as u32)).copied();
                let p = page.get(&(ch.page as u32)).copied();
                if l.is_none() && p.is_none() {
                    alive[c] = false;
                    continue;
                }
                own[c] |= l.is_some();
                score[c] += l.unwrap_or(0) + p.unwrap_or(0);
            }
        }
        let mut hits: Vec<Hit> = self
            .chunks
            .iter()
            .enumerate()
            .filter(|(c, ch)| alive[*c] && (own[*c] || ch.anchor.is_empty()))
            .map(|(c, ch)| Hit {
                page: ch.page,
                anchor: ch.anchor.clone(),
                heading: ch.heading.clone(),
                snippet: snippet(&ch.text, &terms),
                score: score[c],
            })
            .collect();
        hits.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then(a.page.cmp(&b.page))
                .then(a.anchor.cmp(&b.anchor))
        });
        hits.truncate(limit);
        hits
    }
}

/// The first sentence of `text` with one of the words in it, at most about 140 characters.
fn snippet(text: &str, terms: &[&str]) -> String {
    let mut best = None;
    for s in text.split_inclusive(['.', '!', '?']) {
        let f = fold(s);
        if terms.iter().any(|t| f.contains(t)) {
            best = Some(s.trim());
            break;
        }
    }
    let s = best.unwrap_or_else(|| text.split_inclusive('.').next().unwrap_or("").trim());
    shorten(s, 140)
}

/// At most `n` characters, cut at a word with an ellipsis.
pub(crate) fn shorten(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_owned();
    }
    let cut: String = s.chars().take(n).collect();
    let cut = match cut.rfind(' ') {
        Some(i) if i > n / 2 => &cut[..i],
        _ => &cut,
    };
    format!("{}…", cut.trim_end_matches([',', ';', ':', ' ']))
}
