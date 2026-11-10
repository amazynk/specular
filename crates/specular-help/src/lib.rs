//! Specular's manual: offline, bundled, drawn by the app's own help panel (no browser, no web
//! view), searchable as you type.
//!
//! # Sources and translation
//!
//! Pages are Markdown-like files ([`doc`]) in `manual/<locale>/<page>.md`, embedded at build
//! time. Translations are a **parallel tree per locale**, not Fluent messages: a page is long,
//! structured prose (headings, lists, notes, links, key references) that translators work on
//! as a whole and that reads badly cut into short Fluent messages, with its ids and anchors
//! repeated in every catalogue. A locale may translate some pages; each page it lacks falls
//! back to English. A translated page must keep its English page's skeleton (the anchors,
//! key tables, key references and links, in order): `translations_keep_the_skeleton` checks
//! it, so links into a page work in every language. The help panel's own strings (title,
//! search field, buttons) are Fluent messages in `help.ftl`, like the rest of the UI.
//!
//! # Topics
//!
//! [`context_page`] is the page F1 opens for what is on screen; [`disabled_topic`] the page
//! (and section) explaining why a command is unavailable, so every disabled reason links to
//! its help.

pub mod doc;
pub mod search;

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use specular_i18n::I18n;
use specular_input::{Action, Disabled, Mode};

pub use doc::{Block, Page, Section, Span};
pub use search::Hit;

include!(concat!(env!("OUT_DIR"), "/pages.rs"));

pub use specular_input::Topic;

/// The manual in one language: its pages (in contents order) and their index.
pub struct Manual {
    locale: String,
    pages: Vec<Page>,
    by_id: HashMap<String, usize>,
    index: search::Index,
}

impl std::fmt::Debug for Manual {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Manual")
            .field("locale", &self.locale)
            .field("pages", &self.pages.len())
            .finish_non_exhaustive()
    }
}

/// The locales with pages of their own (English first).
pub fn locales() -> Vec<&'static str> {
    PAGES.iter().map(|(l, _)| *l).collect()
}

/// A locale's page sources by file name (empty for a locale without pages).
pub fn sources(locale: &str) -> &'static [(&'static str, &'static str)] {
    PAGES
        .iter()
        .find(|(l, _)| *l == locale)
        .map_or(&[], |(_, p)| *p)
}

/// The names of a key table's actions, for search: a mode's actions, or the gallery's and
/// compare's fixed keys.
pub fn table_words(table: &str) -> String {
    let i18n = I18n::global();
    let mut s = String::new();
    let mut push = |id: &str| {
        s.push_str(&i18n.tr(id));
        s.push(' ');
    };
    match table {
        "gallery" => specular_input::fixed::GALLERY_KEYS
            .iter()
            .for_each(|k| push(k.label)),
        "compare" => specular_input::fixed::COMPARE_KEYS
            .iter()
            .for_each(|k| push(k.label)),
        t => {
            if let Some(m) = Mode::from_id(t) {
                m.actions().for_each(|a| {
                    s.push_str(&a.name());
                    s.push(' ');
                });
            }
        }
    }
    s
}

/// The name of the action `[[key:id]]` names (in the interface's language), as search and
/// snippets read it; the id itself if it names none.
pub fn key_name(id: &str) -> String {
    match Action::from_id(id) {
        Some(a) => a.name(),
        None => id.to_owned(),
    }
}

/// The key tables `::: keys` accepts: the keymap's modes, the gallery and compare.
pub fn key_tables() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = Mode::ALL.iter().map(|m| m.id()).collect();
    v.insert(1, "gallery");
    v.insert(2, "compare");
    v
}

impl Manual {
    /// The manual in `locale` (a language tag such as `de` or `pt-BR`; a region falls back to
    /// its language, then English, page by page). A page that fails to parse is left out (the
    /// tests keep every embedded page parsing).
    pub fn load(locale: &str) -> Manual {
        let lang = locale.split(['-', '_']).next().unwrap_or(locale);
        let chain: Vec<&str> = [locale, lang, "en"]
            .into_iter()
            .fold(Vec::new(), |mut v, l| {
                if !v.contains(&l) {
                    v.push(l);
                }
                v
            });
        let mut pages = Vec::new();
        for (file, _) in sources("en") {
            let src = chain
                .iter()
                .find_map(|l| sources(l).iter().find(|(f, _)| f == file))
                .map(|(_, s)| *s);
            let parsed = src.map(doc::parse).and_then(Result::ok).or_else(|| {
                // A broken translation falls back to English.
                sources("en")
                    .iter()
                    .find(|(f, _)| f == file)
                    .and_then(|(_, s)| doc::parse(s).ok())
            });
            if let Some(p) = parsed {
                pages.push(p);
            }
        }
        // Contents order: by section, then the `order` the file names give (`NN-id.md`).
        pages.sort_by_key(|p| p.section);
        let by_id = pages
            .iter()
            .enumerate()
            .map(|(i, p)| (p.id.clone(), i))
            .collect();
        let index = search::Index::build(&pages, &table_words, &key_name);
        Manual {
            locale: chain[0].to_owned(),
            pages,
            by_id,
            index,
        }
    }

    /// The manual in the interface's language, built once per language.
    pub fn current() -> Arc<Manual> {
        static CACHE: OnceLock<parking::Mutex<Option<Arc<Manual>>>> = OnceLock::new();
        let lang = I18n::global().language().to_string();
        let cache = CACHE.get_or_init(Default::default);
        let mut c = cache.lock();
        match &*c {
            Some(m) if m.locale == lang || m.locale == lang.split('-').next().unwrap_or("") => {
                m.clone()
            }
            _ => {
                let m = Arc::new(Manual::load(&lang));
                *c = Some(m.clone());
                m
            }
        }
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }

    /// In contents order.
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.by_id.get(id).copied()
    }

    pub fn page(&self, id: &str) -> Option<&Page> {
        self.index_of(id).map(|i| &self.pages[i])
    }

    /// The best `limit` sections for `query`.
    pub fn search(&self, query: &str, limit: usize) -> Vec<Hit> {
        self.index.search(query, limit)
    }

    /// The page that is the help for `context` (a page listing it in `contexts`), else the
    /// welcome page.
    pub fn for_context(&self, context: &str) -> usize {
        self.pages
            .iter()
            .position(|p| p.contexts.iter().any(|c| c == context))
            .or_else(|| self.index_of(WELCOME))
            .unwrap_or(0)
    }
}

/// The first page.
pub const WELCOME: &str = "welcome";

/// Every help context the app asks for (F1 in each place it can be pressed); each has a page
/// (`every_context_has_a_page`).
pub const CONTEXTS: &[&str] = &[
    "empty",
    "viewer",
    "gallery",
    "library",
    "compare",
    "cull",
    "reading",
    "animation",
    "video",
    "slideshow",
    "medical",
    "mpr",
    "science",
    "maps",
    "drawing",
    "mesh",
    "relight",
    "develop",
    "palette",
    "cheatsheet",
    "settings",
    "profile",
    "customise-keys",
    "convert",
    "print",
    "measure",
    "similar",
    "anonymise",
    "about",
    "crash",
    "diagnostics",
    "updates",
    "downloads",
    "error",
];

/// The help context of a keymap mode (F1 while it is active).
pub fn context_page(mode: Mode) -> &'static str {
    mode.id()
}

/// The page and section explaining why a command is unavailable (declared with the reason,
/// [`Disabled::topic`]).
pub fn disabled_topic(d: Disabled) -> Topic {
    d.topic()
}

/// The page about an action (declared with the action, [`Action::topic`]).
pub fn action_topic(a: Action) -> Topic {
    a.topic()
}

/// What the About box says about this build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildInfo {
    pub version: &'static str,
    /// The commit and its date ("4a3c0658 2026-10-11"); empty outside a checkout.
    pub commit: &'static str,
    /// "release" or "debug".
    pub profile: &'static str,
    /// "x86_64-linux".
    pub target: String,
}

pub fn build_info() -> BuildInfo {
    BuildInfo {
        version: env!("CARGO_PKG_VERSION"),
        commit: env!("SPECULAR_BUILD_INFO"),
        profile: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        target: format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
    }
}

/// A third-party crate built into Specular.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Licence {
    pub name: &'static str,
    pub version: &'static str,
    /// The SPDX expression its manifest declares.
    pub licence: &'static str,
}

/// Generated from `cargo metadata` (`licences_in_sync` keeps it current).
const THIRD_PARTY: &str = include_str!("../data/third-party.tsv");

/// Every third-party crate built into the app, by name.
pub fn third_party() -> Vec<Licence> {
    THIRD_PARTY
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| {
            let mut f = l.split('\t');
            Some(Licence {
                name: f.next()?,
                version: f.next()?,
                licence: f.next()?,
            })
        })
        .collect()
}

/// The crates grouped by licence expression, the most common first.
pub fn third_party_by_licence() -> Vec<(&'static str, Vec<Licence>)> {
    let mut groups: Vec<(&'static str, Vec<Licence>)> = Vec::new();
    for l in third_party() {
        match groups.iter_mut().find(|(k, _)| *k == l.licence) {
            Some((_, v)) => v.push(l),
            None => groups.push((l.licence, vec![l])),
        }
    }
    groups.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));
    groups
}

/// A small mutex for the cache, without another dependency.
mod parking {
    #[derive(Default)]
    pub struct Mutex<T>(std::sync::Mutex<T>);

    impl<T> Mutex<T> {
        pub fn lock(&self) -> std::sync::MutexGuard<'_, T> {
            self.0.lock().unwrap_or_else(|e| e.into_inner())
        }
    }
}

#[cfg(test)]
mod tests;
