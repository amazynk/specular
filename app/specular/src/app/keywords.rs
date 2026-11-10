//! The keyword panel (`Shift+K`, the palette's "Keywords"): the keywords of the selected
//! photos in the gallery and the Library, or of the photo on screen in cull mode, docked at
//! the trailing edge ([`side_panel`](super::side_panel)).
//!
//! - **On these photos:** every keyword of the photos, with how many have it. A box shows
//!   whether all or some do; clicking it (Space or Enter on the row) gives it to all, or takes
//!   it off when all have it.
//! - **The field:** type a keyword (levels with `>` or `|`: `Places > France > Paris`) and
//!   press Enter to add it to the photos and the list; suggestions from the list, the
//!   Library's keywords and recently used ones appear as you type, Tab takes the first one.
//!   With nothing typed, recently used keywords are offered.
//! - **Keyword list:** every keyword known, as a tree; typing narrows it. The focused
//!   keyword's buttons make it private (left out of `dc:subject` and exports), edit its
//!   synonyms, or remove it from the list (asks first; photos keep their keywords).
//!   **Rename…** and **Move to…** change it (and those below it) in the list and in every
//!   photo that has it: the panel's photos and the Library's, sidecars rewritten off the UI
//!   thread with progress in the downloads row, one undo step. Onto a keyword that exists,
//!   the two merge.
//!   "Import list…" and "Export list…" read and write Lightroom keyword-list text.
//!
//! Keywords are written to each photo's XMP sidecar ([`files`]), in `lr:hierarchicalSubject`
//! and `dc:subject`, off the UI thread, and mirrored into the Library's catalogue. The list
//! and the recents are kept in the config folder (`keywords.txt`, `keywords-recent.txt`).

pub(in crate::app) mod files;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crossbeam_channel::{Receiver, Sender};
use specular_gfx::winit::event::KeyEvent;
use specular_gfx::{DrawList, Rect, Text};
use specular_i18n::t;
use specular_input::{Key, Mods, NamedKey};
use specular_metadata::keywords::{self as kw, Change, Recents, Vocabulary};
use specular_theme::Palette;
use specular_widgets::accesskit;
use specular_widgets::line_edit::{LineEdit, PendingClipboard};

use super::App;
use super::side_panel::{self, Check, Metrics, Painter};

/// AccessKit ids of the panel.
pub(super) const A11Y_KEYWORDS: u64 = 0x7300_0000_0000_0000;
/// Suggestions listed at most while typing.
const SUGGEST: usize = 12;
/// Recently used keywords offered at most.
const RECENT_SHOWN: usize = 8;
/// The selection is looked at again at most this often (s).
const TARGETS_S: f64 = 0.25;
/// The list and recents are saved this long after the last change (s).
const SAVE_S: f64 = 1.0;
/// A big selection is read in parts this large.
const READ_PART: usize = 256;

/// A part of the panel's body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Section {
    OnPhotos,
    Suggestions,
    Recent,
    List,
}

/// Where a keyword row is listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Assigned,
    Suggested,
    Recent,
    Listed,
}

/// One row of the body.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Row {
    Header(Section),
    /// A line of text (nothing selected, reading…).
    Note(String),
    /// Add the typed keyword, new to the list.
    Create(String),
    Keyword {
        path: String,
        kind: Kind,
        check: Check,
        /// Photos with it (of the loaded ones).
        count: usize,
        private: bool,
        /// The synonym that matched what was typed.
        synonym: Option<String>,
    },
}

impl Row {
    /// Rows that take the focus.
    fn actionable(&self) -> bool {
        matches!(self, Row::Create(_) | Row::Keyword { .. })
    }

    fn path(&self) -> Option<&str> {
        match self {
            Row::Keyword { path, .. } => Some(path),
            _ => None,
        }
    }
}

/// A control of the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hit {
    Panel,
    Close,
    Field,
    Row(usize),
    Foot(Button),
}

/// A button at the panel's foot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Button {
    Private,
    Synonyms,
    Forget,
    Rename,
    Move,
    Import,
    Export,
    ConfirmForget,
    Cancel,
}

/// What the field is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Add,
    /// The synonyms of a keyword, separated by commas.
    Synonyms(String),
    /// A keyword's new name (its last level).
    Rename(String),
    /// Where a keyword goes: the keyword it goes under (`Places > France`), or nothing for
    /// the top.
    Move(String),
}

/// Work done off the UI thread.
pub(crate) enum Msg {
    Read(Vec<(PathBuf, Vec<String>)>),
    /// A write's results; `undo`: it was an Undo (not itself undoable).
    Wrote {
        r: files::Written,
        undo: bool,
    },
    Catalogue(Vec<String>),
    /// A rename went through its photos (its write follows as `Wrote`).
    Renamed {
        from: String,
        to: String,
    },
    /// The keywords of a folder listing's photos, for the filter's `tag:`.
    Tags {
        generation: u64,
        read: Vec<(PathBuf, Vec<String>)>,
    },
    Imported(PathBuf, Result<Vocabulary, String>),
    Exported(Result<PathBuf, String>),
}

/// The keywords of the folder's photos for the filter (`tag:` / `keyword:` and the keyword
/// facet), read once per listing while the filter bar is open. The Library has them in its
/// catalogue.
#[derive(Default)]
pub(crate) struct ListingTags {
    /// The listing they were asked for.
    asked: Option<u64>,
    /// Every photo of that listing was read.
    complete: bool,
    /// Each photo's keywords, one per line.
    tags: HashMap<PathBuf, String>,
}

impl ListingTags {
    /// `p`'s keywords, one per line; None while not known.
    pub fn of(&self, p: &Path) -> Option<&str> {
        match self.tags.get(p) {
            Some(t) => Some(t.as_str()),
            None if self.complete => Some(""),
            None => None,
        }
    }

    /// Whether the listing `generation`'s keywords were asked for.
    pub fn asked_for(&self, generation: u64) -> bool {
        self.asked == Some(generation)
    }

    /// Starts over for listing `generation`.
    pub fn ask(&mut self, generation: u64) {
        *self = ListingTags {
            asked: Some(generation),
            ..ListingTags::default()
        };
    }

    /// Keywords read or written for `p`.
    pub fn set(&mut self, p: PathBuf, keywords: &[String]) {
        self.tags.insert(p, keywords.join("\n"));
    }

    /// The listing `generation`'s keywords came back.
    pub fn read(&mut self, generation: u64, read: Vec<(PathBuf, Vec<String>)>) -> bool {
        if self.asked != Some(generation) {
            return false;
        }
        for (p, k) in read {
            if !self.tags.contains_key(&p) {
                self.set(p, &k);
            }
        }
        self.complete = true;
        true
    }
}

/// What a key did.
#[derive(Debug, PartialEq)]
pub(crate) enum KeyOut {
    /// Not the panel's.
    No,
    Used,
    Apply(Vec<Change>),
    /// Rename (or move) keyword `.0` to `.1` everywhere.
    Rename(String, String),
    Close,
    Button(Button),
}

/// The panel and what it knows.
pub(crate) struct Keywords {
    pub open: bool,
    pub edit: LineEdit,
    /// The field has the keyboard.
    pub field_focus: bool,
    /// The row with the keyboard focus.
    pub focus: Option<usize>,
    /// The keyword the foot's buttons act on (the focused or last clicked row's).
    pub current: Option<String>,
    pub scroll: f32,
    /// Where it was drawn, and its controls.
    pub rect: Option<Rect>,
    pub hits: Vec<(Rect, Hit)>,
    pub rows: Vec<Row>,
    pub mode: Mode,
    /// A keyword waiting for "Remove" to leave the list.
    pub confirm: Option<String>,
    /// The last thing done or failed, shown under the title.
    pub status: Option<String>,
    /// The photos it acts on.
    pub targets: Vec<PathBuf>,
    targets_at: f64,
    pub vocab: Vocabulary,
    pub recents: Recents,
    /// Keywords of photos read or written.
    pub files: HashMap<PathBuf, Vec<String>>,
    reading: HashSet<PathBuf>,
    /// The Library's keywords, for suggestions.
    pub catalogue: Vec<String>,
    pub catalogue_asked: bool,
    /// The config folder (None: nothing is saved, as in screenshots and tests).
    dir: Option<PathBuf>,
    loaded: bool,
    save_due: Option<f64>,
    pub tx: Sender<Msg>,
    pub rx: Receiver<Msg>,
    /// Writes in flight (their results replace what was assumed).
    pub writing: usize,
    /// A rename going through photos: from, to, photos done and to do (for the downloads
    /// row).
    pub renaming: Option<Renaming>,
}

/// A keyword rename in progress.
pub(crate) struct Renaming {
    pub from: String,
    pub to: String,
    pub done: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    /// 0 until the photos are found.
    pub total: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl Default for Keywords {
    fn default() -> Keywords {
        let (tx, rx) = crossbeam_channel::unbounded();
        Keywords {
            open: false,
            edit: LineEdit::new(""),
            field_focus: false,
            focus: None,
            current: None,
            scroll: 0.0,
            rect: None,
            hits: Vec::new(),
            rows: Vec::new(),
            mode: Mode::Add,
            confirm: None,
            status: None,
            targets: Vec::new(),
            targets_at: f64::NEG_INFINITY,
            vocab: Vocabulary::new(),
            recents: Recents::default(),
            files: HashMap::new(),
            reading: HashSet::new(),
            catalogue: Vec::new(),
            catalogue_asked: false,
            dir: None,
            loaded: false,
            save_due: None,
            tx,
            rx,
            writing: 0,
            renaming: None,
        }
    }
}

fn fold(s: &str) -> String {
    specular_i18n::fold_for_search(s).into_owned()
}

impl Keywords {
    /// The panel has the keyboard.
    pub fn has_focus(&self) -> bool {
        self.open && (self.field_focus || self.focus.is_some() || self.confirm.is_some())
    }

    /// Takes the saved list and recents from `dir` the first time.
    pub fn load(&mut self, dir: Option<PathBuf>) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        if let Some(d) = &dir {
            let (v, r) = files::load(d);
            self.vocab = v;
            self.recents = r;
        }
        self.dir = dir;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.field_focus = false;
        self.focus = None;
        self.confirm = None;
        self.mode = Mode::Add;
        self.edit.clear();
        self.rect = None;
        self.hits.clear();
    }

    /// The photos' keywords with how many have each (in the order first seen), and how many
    /// photos are read.
    fn counts(&self) -> (Vec<(String, usize)>, usize) {
        let mut out: Vec<(String, usize)> = Vec::new();
        let mut at: HashMap<String, usize> = HashMap::new();
        let mut loaded = 0;
        for p in &self.targets {
            let Some(k) = self.files.get(p) else { continue };
            loaded += 1;
            for path in k {
                match at.get(&kw::key(path)) {
                    Some(&i) => out[i].1 += 1,
                    None => {
                        at.insert(kw::key(path), out.len());
                        out.push((path.clone(), 1));
                    }
                }
            }
        }
        (out, loaded)
    }

    /// The rows for what is selected and typed.
    pub fn build_rows(&self) -> Vec<Row> {
        let (counts, loaded) = self.counts();
        let n = loaded;
        let by_key: HashMap<String, usize> = counts.iter().map(|(p, c)| (kw::key(p), *c)).collect();
        let check = |path: &str| match by_key.get(&kw::key(path)).copied().unwrap_or(0) {
            0 => Check::Off,
            c if c >= n && n > 0 => Check::On,
            _ => Check::Some,
        };
        let count = |path: &str| by_key.get(&kw::key(path)).copied().unwrap_or(0);
        let row = |path: &str, kind: Kind, synonym: Option<String>| Row::Keyword {
            path: self.vocab.spelling(path).to_owned(),
            kind,
            check: check(path),
            count: count(path),
            private: self.vocab.is_private(path),
            synonym,
        };
        let mut rows = Vec::new();
        let typing = self.mode == Mode::Add && !self.edit.text().trim().is_empty();
        rows.push(Row::Header(Section::OnPhotos));
        if self.targets.is_empty() {
            rows.push(Row::Note(t!("keywords-none-selected")));
        } else {
            if loaded < self.targets.len() {
                rows.push(Row::Note(t!("keywords-reading")));
            } else if counts.is_empty() {
                rows.push(Row::Note(t!("keywords-on-none")));
            }
            let mut sorted = counts.clone();
            sorted.sort_by_cached_key(|(p, _)| kw::levels(p).map(fold).collect::<Vec<_>>());
            for (p, _) in &sorted {
                rows.push(row(p, Kind::Assigned, None));
            }
        }
        if typing {
            let typed = self.edit.text();
            rows.push(Row::Header(Section::Suggestions));
            if let Some(want) = kw::normalise(typed)
                && !self.vocab.contains(&want)
                && !self.catalogue.iter().any(|c| kw::same(c, &want))
                && !counts.iter().any(|(c, _)| kw::same(c, &want))
            {
                rows.push(Row::Create(want));
            }
            for s in kw::suggest(
                &self.vocab,
                &self.catalogue,
                &self.recents,
                typed,
                &fold,
                SUGGEST,
            ) {
                rows.push(row(&s.path, Kind::Suggested, s.synonym));
            }
        } else if self.mode == Mode::Add {
            let recent: Vec<&String> = self
                .recents
                .list()
                .iter()
                .filter(|p| check(p) != Check::On)
                .take(RECENT_SHOWN)
                .collect();
            if !recent.is_empty() {
                rows.push(Row::Header(Section::Recent));
                for p in recent {
                    rows.push(row(p, Kind::Recent, None));
                }
            }
        }
        rows.push(Row::Header(Section::List));
        let filter = if typing {
            Some(fold(self.edit.text().trim()))
        } else {
            None
        };
        let tree = self.vocab.tree(&fold);
        if tree.is_empty() {
            rows.push(Row::Note(t!("keywords-list-empty")));
        }
        for e in tree {
            if let Some(f) = &filter
                && !fold(&e.path).contains(f.as_str())
                && !e.synonyms.iter().any(|s| fold(s).contains(f.as_str()))
            {
                continue;
            }
            rows.push(row(&e.path, Kind::Listed, None));
        }
        rows
    }

    /// The changes that activating row `k` makes: a keyword all photos have comes off, any
    /// other goes on all of them.
    pub fn activate(&mut self, k: usize) -> Option<Vec<Change>> {
        if self.targets.is_empty() {
            return None;
        }
        match self.rows.get(k)? {
            Row::Create(p) => Some(vec![Change::Add(p.clone())]),
            Row::Keyword { path, check, .. } => Some(vec![if *check == Check::On {
                Change::Remove(path.clone())
            } else {
                Change::Add(path.clone())
            }]),
            _ => None,
        }
    }

    /// Enter in the field: the focused suggestion, else the typed keyword.
    fn enter(&mut self) -> KeyOut {
        if let Mode::Rename(from) | Mode::Move(from) = &self.mode {
            let from = from.clone();
            let to = match &self.mode {
                Mode::Rename(_) => {
                    let Some(name) = kw::normalise(&self.edit.text().replace(['>', '|'], "/"))
                    else {
                        return KeyOut::Used;
                    };
                    match kw::parent(&from) {
                        Some(up) => format!("{up}{}{name}", kw::SEP),
                        None => name,
                    }
                }
                _ => match kw::normalise(self.edit.text()) {
                    Some(up) => {
                        format!("{}{}{}", self.vocab.spelling(&up), kw::SEP, kw::leaf(&from))
                    }
                    None => kw::leaf(&from).to_owned(),
                },
            };
            if kw::is_within(&to, &from) && !kw::same(&to, &from) {
                self.status = Some(t!("keywords-move-into-itself", keyword = kw::leaf(&from)));
                return KeyOut::Used;
            }
            self.mode = Mode::Add;
            self.edit.clear();
            // Only the spelling changed: still a rename (the case of a keyword is its own).
            if to == from {
                return KeyOut::Used;
            }
            return KeyOut::Rename(from, to);
        }
        if let Mode::Synonyms(path) = &self.mode {
            let syn: Vec<String> = self.edit.text().split(',').map(str::to_owned).collect();
            self.vocab.set_synonyms(&path.clone(), &syn);
            self.mode = Mode::Add;
            self.edit.clear();
            return KeyOut::Used;
        }
        let Some(want) = kw::normalise(self.edit.text()) else {
            return KeyOut::Used;
        };
        if self.targets.is_empty() {
            return KeyOut::Used;
        }
        // A synonym typed whole stands for its keyword.
        let path = match self.vocab.synonym_of(&want).first() {
            Some(p) if !self.vocab.contains(&want) => (*p).to_owned(),
            _ => self.vocab.spelling(&want).to_owned(),
        };
        self.edit.clear();
        KeyOut::Apply(vec![Change::Add(path)])
    }

    /// The first suggestion into the field (Tab).
    fn complete(&mut self) {
        let first = self.rows.iter().find_map(|r| match r {
            Row::Keyword {
                path,
                kind: Kind::Suggested,
                ..
            } => Some(path.clone()),
            _ => None,
        });
        if let Some(p) = first {
            self.edit.set_text(&kw::display(&p).replace(" › ", " > "));
        }
    }

    /// The next actionable row from `from` in direction `dir`.
    fn step(&self, from: Option<usize>, dir: isize) -> Option<usize> {
        let n = self.rows.len() as isize;
        let mut i = match from {
            Some(k) => k as isize + dir,
            None if dir > 0 => 0,
            None => n - 1,
        };
        while (0..n).contains(&i) {
            if self.rows[i as usize].actionable() {
                return Some(i as usize);
            }
            i += dir;
        }
        None
    }

    fn focus_row(&mut self, k: Option<usize>) {
        self.focus = k;
        if let Some(p) = k.and_then(|k| self.rows.get(k)).and_then(Row::path) {
            self.current = Some(p.to_owned());
        }
    }

    /// A key while the panel has the keyboard. `text`: what the key types.
    pub fn key(
        &mut self,
        key: Key,
        mods: Mods,
        text: Option<&str>,
        clip: &mut PendingClipboard,
    ) -> KeyOut {
        if !self.has_focus() {
            return KeyOut::No;
        }
        let plain = !mods.ctrl && !mods.alt && !mods.meta;
        if self.confirm.is_some() {
            return match key {
                Key::Named(NamedKey::Enter) => KeyOut::Button(Button::ConfirmForget),
                Key::Named(NamedKey::Escape) => KeyOut::Button(Button::Cancel),
                _ => KeyOut::Used,
            };
        }
        if let Key::Named(n) = key
            && plain
        {
            match n {
                NamedKey::Escape => {
                    if self.mode != Mode::Add {
                        self.mode = Mode::Add;
                        self.edit.clear();
                    } else if self.focus.is_some() {
                        self.focus = None;
                        self.field_focus = true;
                    } else if !self.edit.is_empty() {
                        self.edit.clear();
                    } else {
                        // The grid (or the photo) has the keyboard again.
                        self.field_focus = false;
                    }
                    return KeyOut::Used;
                }
                NamedKey::ArrowDown => {
                    let k = self.step(self.focus, 1);
                    if k.is_some() {
                        self.field_focus = false;
                        self.focus_row(k);
                    }
                    return KeyOut::Used;
                }
                NamedKey::ArrowUp => {
                    match self.step(self.focus, -1) {
                        Some(k) if self.focus.is_some() => self.focus_row(Some(k)),
                        _ => {
                            self.focus = None;
                            self.field_focus = true;
                        }
                    }
                    return KeyOut::Used;
                }
                NamedKey::Tab if self.field_focus && !mods.shift => {
                    self.complete();
                    return KeyOut::Used;
                }
                NamedKey::Enter if self.field_focus => return self.enter(),
                NamedKey::Enter | NamedKey::Space if self.focus.is_some() => {
                    return match self.focus.and_then(|k| self.activate(k)) {
                        Some(c) => KeyOut::Apply(c),
                        None => KeyOut::Used,
                    };
                }
                NamedKey::Delete if self.focus.is_some() => {
                    // Off the photos (Shift+Delete: off the list, after asking).
                    let k = self.focus.unwrap_or(0);
                    return match self.rows.get(k) {
                        Some(Row::Keyword { path, check, .. }) if *check != Check::Off => {
                            KeyOut::Apply(vec![Change::Remove(path.clone())])
                        }
                        _ => KeyOut::Used,
                    };
                }
                _ => {}
            }
        }
        if self.focus.is_some() && mods.shift && !mods.ctrl && key == Key::Named(NamedKey::Delete) {
            return KeyOut::Button(Button::Forget);
        }
        if key == Key::Named(NamedKey::Escape) {
            return KeyOut::Used;
        }
        // Typing on a row goes to the field.
        if self.focus.is_some() && plain && text.is_some_and(|t| t.chars().any(|c| !c.is_control()))
        {
            self.focus = None;
            self.field_focus = true;
        }
        if self.field_focus {
            if self.edit.key(key, mods, false, clip).used() {
                self.scroll = 0.0;
                return KeyOut::Used;
            }
            if plain && let Some(t) = text {
                let s: String = t.chars().filter(|c| !c.is_control()).collect();
                if !s.is_empty() {
                    self.edit.insert(&s);
                    self.scroll = 0.0;
                    return KeyOut::Used;
                }
            }
        }
        // Other keys with Ctrl or Alt (the palette, undo) go on; plain ones stay here.
        if plain { KeyOut::Used } else { KeyOut::No }
    }

    /// Text typed by a key without a shortcut form (a dead key's result, an IME's).
    pub fn type_text(&mut self, t: &str) -> KeyOut {
        if !self.has_focus() || self.confirm.is_some() {
            return KeyOut::No;
        }
        let s: String = t.chars().filter(|c| !c.is_control()).collect();
        if s.is_empty() {
            return KeyOut::No;
        }
        self.focus = None;
        self.field_focus = true;
        self.edit.insert(&s);
        self.scroll = 0.0;
        KeyOut::Used
    }

    /// A click on `hit`.
    pub fn click(&mut self, hit: Hit) -> KeyOut {
        match hit {
            Hit::Close => KeyOut::Close,
            Hit::Field => {
                self.field_focus = true;
                self.focus = None;
                KeyOut::Used
            }
            Hit::Row(k) => {
                self.field_focus = false;
                self.focus_row(Some(k));
                match self.activate(k) {
                    Some(c) => KeyOut::Apply(c),
                    None => KeyOut::Used,
                }
            }
            Hit::Foot(b) => KeyOut::Button(b),
            Hit::Panel => KeyOut::Used,
        }
    }

    /// The keyword the foot's buttons act on, when it is in the list.
    fn current_entry(&self) -> Option<&kw::Entry> {
        self.current.as_deref().and_then(|p| self.vocab.get(p))
    }

    /// Runs a foot button that needs no dialog. True: something changed.
    pub fn button(&mut self, b: Button) -> bool {
        match b {
            Button::Private => {
                let Some(e) = self.current_entry() else {
                    return false;
                };
                let (p, on) = (e.path.clone(), !e.private);
                self.vocab.set_private(&p, on);
                true
            }
            Button::Synonyms => {
                let Some(e) = self.current_entry() else {
                    return false;
                };
                let text = e.synonyms.join(", ");
                self.mode = Mode::Synonyms(e.path.clone());
                self.edit.set_text(&text);
                self.field_focus = true;
                self.focus = None;
                true
            }
            Button::Forget => {
                let Some(e) = self.current_entry() else {
                    return false;
                };
                self.confirm = Some(e.path.clone());
                true
            }
            Button::Rename | Button::Move => {
                let Some(e) = self.current_entry() else {
                    return false;
                };
                let path = e.path.clone();
                let text = if b == Button::Rename {
                    kw::leaf(&path).to_owned()
                } else {
                    kw::parent(&path).map_or(String::new(), |p| p.replace(kw::SEP, " > "))
                };
                self.mode = if b == Button::Rename {
                    Mode::Rename(path)
                } else {
                    Mode::Move(path)
                };
                self.edit.set_text(&text);
                self.field_focus = true;
                self.focus = None;
                true
            }
            Button::ConfirmForget => {
                let Some(p) = self.confirm.take() else {
                    return false;
                };
                let n = self.vocab.remove(&p);
                self.recents.forget(&p);
                self.current = None;
                self.focus = None;
                self.field_focus = true;
                self.status = Some(t!(
                    "keywords-forgotten",
                    keyword = kw::display(&p),
                    count = n as i64
                ));
                true
            }
            Button::Cancel => {
                self.confirm = None;
                true
            }
            Button::Import | Button::Export => false,
        }
    }

    /// Keywords given to photos elsewhere (the metadata editor): the list and recents learn
    /// them, saved shortly after.
    pub fn learned(&mut self, added: &[String], now: f64) {
        for p in added {
            let p = self.vocab.add(p);
            self.recents.used(&p);
        }
        if !added.is_empty() {
            self.save_due = Some(now + SAVE_S);
        }
    }

    /// Puts the photos' keywords as assumed after `changes` (the write's result follows),
    /// and the list and recents learn the added ones.
    pub fn assume(&mut self, changes: &[Change]) {
        for c in changes {
            if let Change::Add(p) = c {
                let p = self.vocab.add(p);
                self.recents.used(&p);
            }
        }
        for t in &self.targets {
            if let Some(k) = self.files.get_mut(t) {
                *k = kw::apply(k, changes);
            }
        }
    }

    /// Keywords read or written: the list learns those it did not know.
    fn learn(&mut self, read: &[(PathBuf, Vec<String>)]) {
        for (p, k) in read {
            self.vocab.learn(k);
            self.reading.remove(p);
            self.files.insert(p.clone(), k.clone());
        }
    }

    /// The photos not read yet, at most a part.
    fn unread_part(&mut self) -> Vec<PathBuf> {
        let v: Vec<PathBuf> = self
            .targets
            .iter()
            .filter(|p| !self.files.contains_key(*p) && !self.reading.contains(*p))
            .take(READ_PART)
            .cloned()
            .collect();
        self.reading.extend(v.iter().cloned());
        v
    }

    /// Draws the panel into `panel` (as placed on screen).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        panel: Rect,
        text_scale: f32,
        rtl: bool,
        pointer: Option<(f32, f32)>,
    ) {
        self.rows = self.build_rows();
        if let Some(k) = self.focus
            && !self.rows.get(k).is_some_and(Row::actionable)
        {
            self.focus = self
                .step(Some(k.saturating_sub(1)), 1)
                .or(self.step(None, -1));
        }
        let m = Metrics::new(text_scale);
        let mut p = Painter {
            dl,
            text,
            pal,
            m,
            panel,
            rtl,
            pointer,
        };
        self.hits.clear();
        self.rect = Some(panel);
        self.hits.push((panel, Hit::Panel));
        let photos = self.targets.len();
        let note = self
            .status
            .clone()
            .or_else(|| (photos > 0).then(|| t!("keywords-photos", count = photos as i64)));
        let (close, top) = p.frame(&t!("keywords-title"), note.as_deref());
        self.hits.push((close, Hit::Close));
        let placeholder = match &self.mode {
            Mode::Add => t!("keywords-field"),
            Mode::Synonyms(k) => t!("keywords-synonyms-of", keyword = kw::leaf(k)),
            Mode::Rename(k) => t!("keywords-rename-of", keyword = kw::leaf(k)),
            Mode::Move(k) => t!("keywords-move-of", keyword = kw::leaf(k)),
        };
        let field = p.field(top, &self.edit, &placeholder, self.field_focus && self.open);
        self.hits.push((field, Hit::Field));
        // The foot: the current keyword's buttons, or the question; then import and export.
        let entry = self.current_entry().cloned();
        let foot_rows = 3.0;
        let body_top = top + m.field_h + m.u(6.0);
        let body_bottom = panel.bottom() - foot_rows * m.foot_h;
        let body = Rect::new(
            panel.x,
            body_top,
            panel.w,
            (body_bottom - body_top).max(0.0),
        );
        // Row heights and the scroll that keeps the focused row in view.
        let heights: Vec<f32> = self
            .rows
            .iter()
            .map(|r| match r {
                Row::Header(_) => m.head_h,
                _ => m.row_h,
            })
            .collect();
        let content: f32 = heights.iter().sum::<f32>() + m.pad;
        if let Some(k) = self.focus {
            let y: f32 = heights[..k].iter().sum();
            self.scroll = side_panel::scroll_to(self.scroll, y, heights[k], body.h);
        }
        self.scroll = self.scroll.clamp(0.0, (content - body.h).max(0.0));
        p.dl.push_clip(p.flip(body));
        let mut y = body.y - self.scroll;
        let loc = specular_i18n::I18n::global().localizer();
        for (k, row) in self.rows.iter().enumerate() {
            let h = heights[k];
            if y + h <= body.y || y >= body.bottom() {
                y += h;
                continue;
            }
            let lr = Rect::new(body.x + m.u(6.0), y, body.w - m.u(12.0), h);
            let rr = p.flip(lr);
            match row {
                Row::Header(s) => {
                    let name = match s {
                        Section::OnPhotos => t!("keywords-on-photos"),
                        Section::Suggestions => t!("keywords-suggestions"),
                        Section::Recent => t!("keywords-recent"),
                        Section::List => t!("keywords-list"),
                    };
                    p.header(y, &name);
                }
                Row::Note(s) => {
                    let r = Rect::new(lr.x + m.u(8.0), y, lr.w - m.u(16.0), h);
                    let s = side_panel::fit(p.text, s, m.small, r.w);
                    p.label(&s, m.small, r, pal.text_muted);
                }
                Row::Create(path) => {
                    p.row_back(rr, self.focus == Some(k));
                    let r = Rect::new(lr.x + m.u(8.0), y, lr.w - m.u(16.0), h);
                    let s = t!("keywords-create", keyword = kw::display(path));
                    let s = side_panel::fit(p.text, &s, m.row, r.w);
                    p.label(&s, m.row, r, pal.accent);
                    self.hits.push((rr.intersect(&p.flip(body)), Hit::Row(k)));
                }
                Row::Keyword {
                    path,
                    kind,
                    check,
                    count,
                    private,
                    synonym,
                } => {
                    p.row_back(rr, self.focus == Some(k));
                    let bs = m.u(14.0);
                    let depth = if *kind == Kind::Listed {
                        kw::depth(path) as f32
                    } else {
                        0.0
                    };
                    let x0 = lr.x + m.u(8.0) + depth * m.u(14.0);
                    p.check(p.flip(Rect::new(x0, y + (h - bs) * 0.5, bs, bs)), *check);
                    let tx = x0 + bs + m.u(8.0);
                    // The count when some photos have it, the private mark.
                    let mut tail = String::new();
                    if *check == Check::Some && *kind == Kind::Assigned {
                        tail = t!(
                            "keywords-count",
                            count = loc.integer(*count as i64),
                            total = loc.integer(photos as i64)
                        );
                    }
                    if *private {
                        if !tail.is_empty() {
                            tail.push_str(" · ");
                        }
                        tail.push_str(&t!("keywords-private-mark"));
                    }
                    let tw = if tail.is_empty() {
                        0.0
                    } else {
                        p.text.measure(&tail, m.small) + m.u(8.0)
                    };
                    let name = match (kind, synonym) {
                        (Kind::Listed, _) => kw::leaf(path).to_owned(),
                        (_, Some(s)) => format!("{} ({s})", kw::display(path)),
                        _ => kw::display(path),
                    };
                    let nr = Rect::new(tx, y, (lr.right() - m.u(8.0) - tw - tx).max(0.0), h);
                    let s = side_panel::fit(p.text, &name, m.row, nr.w);
                    p.label(&s, m.row, nr, pal.text);
                    if tw > 0.0 {
                        let cr = Rect::new(lr.right() - m.u(8.0) - tw, y, tw, h);
                        p.label_end(&tail, m.small, cr, pal.text_muted);
                    }
                    self.hits.push((rr.intersect(&p.flip(body)), Hit::Row(k)));
                }
            }
            y += h;
        }
        p.dl.pop_clip();
        // First foot row.
        let y1 = body_bottom;
        if let Some(c) = &self.confirm {
            let r = Rect::new(panel.x + m.pad, y1, panel.w - 2.0 * m.pad, m.foot_h * 0.5);
            let q = t!("keywords-forget-ask", keyword = kw::leaf(c));
            let q = side_panel::fit(p.text, &q, m.small, r.w);
            p.label(&q, m.small, r, pal.text);
            let r2 = Rect::new(r.x, y1 + m.foot_h * 0.5, r.w, m.foot_h * 0.5);
            let keep = side_panel::fit(p.text, &t!("keywords-forget-keep"), m.small, r2.w);
            p.label(&keep, m.small, r2, pal.text_muted);
            let b = p.foot(
                y1 + m.foot_h,
                &[
                    (&t!("keywords-forget-yes"), false, true),
                    (&t!("app-cancel"), false, true),
                ],
            );
            self.hits.push((b[0], Hit::Foot(Button::ConfirmForget)));
            self.hits.push((b[1], Hit::Foot(Button::Cancel)));
            return;
        }
        let on = entry.is_some();
        let b = p.foot(
            y1,
            &[
                (
                    &t!("keywords-private"),
                    entry.as_ref().is_some_and(|e| e.private),
                    on,
                ),
                (&t!("keywords-synonyms"), false, on),
                (&t!("keywords-forget"), false, on),
            ],
        );
        if on {
            self.hits.push((b[0], Hit::Foot(Button::Private)));
            self.hits.push((b[1], Hit::Foot(Button::Synonyms)));
            self.hits.push((b[2], Hit::Foot(Button::Forget)));
        }
        // Rename and move (not while one is going through photos).
        let free = on && self.renaming.is_none();
        let b = p.foot(
            y1 + m.foot_h,
            &[
                (&t!("keywords-rename"), false, free),
                (&t!("keywords-move"), false, free),
            ],
        );
        if free {
            self.hits.push((b[0], Hit::Foot(Button::Rename)));
            self.hits.push((b[1], Hit::Foot(Button::Move)));
        }
        let b = p.foot(
            y1 + 2.0 * m.foot_h,
            &[
                (&t!("keywords-import"), false, true),
                (&t!("keywords-export"), false, true),
            ],
        );
        self.hits.push((b[0], Hit::Foot(Button::Import)));
        self.hits.push((b[1], Hit::Foot(Button::Export)));
    }

    pub fn hit_at(&self, p: (f32, f32)) -> Option<Hit> {
        self.hits
            .iter()
            .rev()
            .find(|h| h.0.contains(p))
            .map(|h| h.1)
    }

    /// The panel's AccessKit nodes under the window's root.
    pub fn a11y(&self, tree: &mut accesskit::TreeUpdate, scale: f64) {
        use accesskit::{Node, NodeId, Role, Toggled};
        let Some(panel) = self.rect.filter(|_| self.open) else {
            return;
        };
        let bounds = |r: Rect| accesskit::Rect {
            x0: f64::from(r.x) * scale,
            y0: f64::from(r.y) * scale,
            x1: f64::from(r.x + r.w) * scale,
            y1: f64::from(r.y + r.h) * scale,
        };
        let mut group = Node::new(Role::Group);
        group.set_label(t!("keywords-title"));
        group.set_bounds(bounds(panel));
        let mut ids = Vec::new();
        let mut nodes = Vec::new();
        let mut focus = None;
        for (i, (r, hit)) in self.hits.iter().enumerate() {
            let id = NodeId(A11Y_KEYWORDS + 1 + i as u64);
            let mut n = match hit {
                Hit::Panel => continue,
                Hit::Close => {
                    let mut n = Node::new(Role::Button);
                    n.set_label(t!("keywords-close"));
                    n
                }
                Hit::Field => {
                    let mut n = Node::new(Role::TextInput);
                    n.set_label(match &self.mode {
                        Mode::Add => t!("keywords-field"),
                        Mode::Synonyms(k) => t!("keywords-synonyms-of", keyword = kw::leaf(k)),
                        Mode::Rename(k) => t!("keywords-rename-of", keyword = kw::leaf(k)),
                        Mode::Move(k) => t!("keywords-move-of", keyword = kw::leaf(k)),
                    });
                    n.set_value(self.edit.text().to_owned());
                    if self.field_focus {
                        focus = Some(id);
                    }
                    n
                }
                Hit::Row(k) => {
                    let Some(row) = self.rows.get(*k) else {
                        continue;
                    };
                    let mut n;
                    match row {
                        Row::Create(path) => {
                            n = Node::new(Role::Button);
                            n.set_label(t!("keywords-create", keyword = kw::display(path)));
                        }
                        Row::Keyword {
                            path,
                            check,
                            private,
                            ..
                        } => {
                            n = Node::new(Role::CheckBox);
                            let mut l = kw::display(path);
                            if *private {
                                l = format!("{l}, {}", t!("keywords-private-mark"));
                            }
                            n.set_label(l);
                            n.set_toggled(match check {
                                Check::On => Toggled::True,
                                Check::Some => Toggled::Mixed,
                                Check::Off => Toggled::False,
                            });
                        }
                        _ => continue,
                    }
                    if self.focus == Some(*k) {
                        focus = Some(id);
                    }
                    n
                }
                Hit::Foot(b) => {
                    let mut n = Node::new(Role::Button);
                    n.set_label(match b {
                        Button::Private => t!("keywords-private"),
                        Button::Synonyms => t!("keywords-synonyms"),
                        Button::Forget => t!("keywords-forget"),
                        Button::Rename => t!("keywords-rename"),
                        Button::Move => t!("keywords-move"),
                        Button::Import => t!("keywords-import"),
                        Button::Export => t!("keywords-export"),
                        Button::ConfirmForget => t!("keywords-forget-yes"),
                        Button::Cancel => t!("app-cancel"),
                    });
                    if *b == Button::Private {
                        n.set_toggled(if self.current_entry().is_some_and(|e| e.private) {
                            Toggled::True
                        } else {
                            Toggled::False
                        });
                    }
                    n
                }
            };
            n.set_bounds(bounds(*r));
            n.add_action(accesskit::Action::Click);
            if matches!(hit, Hit::Field) {
                n.add_action(accesskit::Action::Focus);
            }
            ids.push(id);
            nodes.push((id, n));
        }
        group.set_children(ids);
        if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
            root.push_child(NodeId(A11Y_KEYWORDS));
        }
        tree.nodes.push((NodeId(A11Y_KEYWORDS), group));
        tree.nodes.extend(nodes);
        if let Some(f) = focus.filter(|_| self.has_focus()) {
            tree.focus = f;
        }
    }

    /// The hit of AccessKit node `id`.
    pub fn a11y_hit(&self, id: u64) -> Option<Hit> {
        let i = id.checked_sub(A11Y_KEYWORDS + 1)?;
        self.hits.get(usize::try_from(i).ok()?).map(|h| h.1)
    }
}

// ----- The application's side ----------------------------------------------------------------

impl App {
    /// The config folder the keyword list is kept in (None in screenshots and headless).
    fn keywords_dir(&self) -> Option<PathBuf> {
        if self.args.screenshot.is_some() || self.headless() {
            return None;
        }
        self.store
            .path()
            .and_then(|p| p.parent().map(Path::to_path_buf))
    }

    /// Where the panel acts now: cull mode's photo (its RAW and JPEG), or the gallery's
    /// selection. None: neither shows.
    fn keywords_where(&self) -> Option<bool> {
        if self.cull_keyword_targets().is_some() {
            Some(true)
        } else if self.gallery.active {
            Some(false)
        } else {
            None
        }
    }

    /// `Shift+K`: opens the panel with the keyboard in its field; again closes it.
    pub(super) fn keywords_toggle(&mut self) {
        if self.keywords_where().is_none() {
            return;
        }
        let dir = self.keywords_dir();
        if self.gallery.meta_edit.open {
            self.meta_close();
        }
        let k = &mut self.gallery.keywords;
        if k.open && k.has_focus() {
            k.close();
        } else {
            k.load(dir);
            k.open = true;
            k.field_focus = true;
            k.focus = None;
            k.targets_at = f64::NEG_INFINITY;
            self.gallery_keywords_opened();
        }
        self.redraw = true;
    }

    /// Once per event-loop turn: results from the workers, the selection, saving.
    pub(super) fn keywords_tick(&mut self) {
        let now = self.now();
        self.keywords_listing();
        let msgs: Vec<Msg> = self.gallery.keywords.rx.try_iter().collect();
        for m in msgs {
            self.keywords_msg(m, now);
            self.redraw = true;
        }
        if self.gallery.keywords.open {
            match self.keywords_where() {
                None => {
                    self.gallery.keywords.close();
                    self.gallery_keywords_closed();
                    self.redraw = true;
                }
                Some(cull) => {
                    let k = &self.gallery.keywords;
                    if now - k.targets_at >= TARGETS_S {
                        let t = if cull {
                            self.cull_keyword_targets().unwrap_or_default()
                        } else {
                            self.gallery.keyword_targets()
                        };
                        let k = &mut self.gallery.keywords;
                        k.targets_at = now;
                        if t != k.targets {
                            k.targets = t;
                            if !k.status.as_ref().is_some_and(|_| k.writing > 0) {
                                k.status = None;
                            }
                            self.redraw = true;
                        }
                    }
                    self.keywords_read();
                    self.keywords_catalogue();
                }
            }
        }
        let k = &mut self.gallery.keywords;
        if k.save_due.is_some_and(|t| now >= t) {
            k.save_due = None;
            if let Some(dir) = k.dir.clone() {
                let (list, recent) = (k.vocab.to_lightroom(), k.recents.to_text());
                self.loader.background("kw-save", move |_| {
                    if let Err(e) = files::save(&dir, &list, &recent) {
                        specular_log::warn!(target: specular_log::UI, "keyword list not saved: {e}");
                    }
                });
            }
        }
    }

    /// The folder's photos' keywords for the filter, once per listing while the bar is open.
    fn keywords_listing(&mut self) {
        let ga = &mut self.gallery;
        let Some(generation) = ga.listing_tags_wanted() else {
            return;
        };
        ga.listing_tags.ask(generation);
        let photos = ga.listing_photos();
        let k = &ga.keywords;
        let (tx, vocab, wake) = (k.tx.clone(), k.vocab.clone(), self.loader.waker());
        self.loader.background("kw-listing", move |_| {
            let read = files::read_all(&photos, &vocab);
            let _ = tx.send(Msg::Tags { generation, read });
            wake();
        });
    }

    fn keywords_read(&mut self) {
        let k = &mut self.gallery.keywords;
        let todo = k.unread_part();
        if todo.is_empty() {
            return;
        }
        let (tx, vocab, wake) = (k.tx.clone(), k.vocab.clone(), self.loader.waker());
        self.loader.background("kw-read", move |_| {
            let _ = tx.send(Msg::Read(files::read_all(&todo, &vocab)));
            wake();
        });
    }

    /// The Library's keywords, once per opening of the panel.
    fn keywords_catalogue(&mut self) {
        if self.gallery.keywords.catalogue_asked {
            return;
        }
        self.gallery.keywords.catalogue_asked = true;
        let Some(lib) = self.gallery.library_handle() else {
            return;
        };
        let (tx, wake) = (self.gallery.keywords.tx.clone(), self.loader.waker());
        self.loader.background("kw-catalogue", move |_| {
            if let Ok(names) = lib.tag_names() {
                let _ = tx.send(Msg::Catalogue(names));
                wake();
            }
        });
    }

    fn keywords_msg(&mut self, m: Msg, now: f64) {
        let k = &mut self.gallery.keywords;
        match m {
            Msg::Read(r) => k.learn(&r),
            Msg::Catalogue(c) => k.catalogue = c,
            Msg::Renamed { from, to } => {
                let n = k
                    .renaming
                    .take()
                    .map_or(0, |r| r.total.load(std::sync::atomic::Ordering::Relaxed));
                let msg = t!(
                    "keywords-renamed",
                    from = kw::display(&from),
                    to = kw::display(&to),
                    count = n as i64
                );
                k.status = Some(msg.clone());
                self.chrome.announce_status(msg);
                // The Library's suggestions follow.
                k.catalogue_asked = false;
                self.keywords_catalogue();
                self.net_refresh();
            }
            Msg::Tags { generation, read } => {
                if self.gallery.listing_tags.read(generation, read) {
                    self.gallery.keyword_facts_changed();
                }
            }
            Msg::Wrote { r, undo } => {
                k.writing = k.writing.saturating_sub(1);
                let mut ok = Vec::new();
                let mut failed = Vec::new();
                // What Undo puts back: each changed photo's keywords before.
                let mut before = Vec::new();
                // For the `keywords` event: each changed photo's keywords before and after.
                let mut told = Vec::new();
                for (p, res) in r {
                    match res {
                        Ok((was, kws)) => {
                            if was != kws {
                                told.push((p.clone(), was.clone(), kws.clone()));
                                before.push((p.clone(), was));
                            }
                            k.files.insert(p.clone(), kws.clone());
                            self.gallery.listing_tags.set(p.clone(), &kws);
                            ok.push((p, kws));
                        }
                        Err(e) => {
                            specular_log::warn!(target: specular_log::UI, "keywords of {}: {e}", p.display());
                            // Read again: what is on disk is what counts.
                            k.files.remove(&p);
                            failed.push(e);
                        }
                    }
                }
                if let Some(e) = failed.first() {
                    let msg = t!(
                        "keywords-write-failed",
                        count = failed.len() as i64,
                        error = e.as_str()
                    );
                    k.status = Some(msg.clone());
                    self.chrome.toast(msg, None, now);
                }
                if let Some(lib) = self.gallery.library_handle() {
                    lib.set_keywords(ok);
                }
                self.gallery_keywords_written();
                self.gallery.keyword_facts_changed();
                if !undo && !before.is_empty() {
                    self.gallery_keywords_undoable(before);
                }
                if let Some(e) = super::events::keywords(&told)
                    && self.events_listening("keywords")
                {
                    self.emit(e);
                }
            }
            Msg::Imported(path, r) => {
                let file = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let msg = match r {
                    Ok(v) => {
                        let n = k.vocab.merge(&v);
                        k.save_due = Some(now + SAVE_S);
                        t!("keywords-imported", count = n as i64, file = file)
                    }
                    Err(e) => t!("keywords-import-failed", file = file, error = e),
                };
                k.status = Some(msg.clone());
                self.chrome.announce_status(msg);
            }
            Msg::Exported(r) => {
                let msg = match r {
                    Ok(p) => t!(
                        "keywords-exported",
                        file = p
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    ),
                    Err(e) => t!("keywords-export-failed", error = e),
                };
                k.status = Some(msg.clone());
                self.chrome.announce_status(msg);
            }
        }
    }

    /// Applies `changes` to the photos the panel acts on.
    fn keywords_apply(&mut self, changes: Vec<Change>) {
        let now = self.now();
        let k = &mut self.gallery.keywords;
        if k.targets.is_empty() || changes.is_empty() {
            return;
        }
        k.assume(&changes);
        k.save_due = Some(now + SAVE_S);
        let said = match &changes[..] {
            [Change::Add(p)] => t!("keywords-added", keyword = kw::display(p)),
            [Change::Remove(p)] => t!("keywords-removed", keyword = kw::display(p)),
            _ => String::new(),
        };
        k.status = (!said.is_empty()).then(|| said.clone());
        if !said.is_empty() {
            self.chrome.announce_status(said);
        }
        let k = &mut self.gallery.keywords;
        k.writing += 1;
        let targets = k.targets.clone();
        let (tx, vocab, wake) = (k.tx.clone(), k.vocab.clone(), self.loader.waker());
        if self.args.screenshot.is_some() {
            return;
        }
        self.loader.background("kw-write", move |_| {
            let r = files::write_all(&targets, &changes, &vocab);
            let _ = tx.send(Msg::Wrote { r, undo: false });
            wake();
        });
    }

    /// Renames keyword `from` (and those below it) to `to`: in the list at once, then in
    /// every photo that has it (the panel's and the Library's), off the UI thread with
    /// progress in the downloads row. The write comes back as one undo step and is mirrored
    /// into the catalogue.
    fn keywords_rename(&mut self, from: &str, to: &str) {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let now = self.now();
        let k = &mut self.gallery.keywords;
        if k.renaming.is_some() {
            return;
        }
        let merged = k.vocab.contains(to);
        k.vocab.rename(from, to);
        let to = k.vocab.spelling(to).to_owned();
        k.recents.forget(from);
        k.recents.used(&to);
        k.current = Some(to.clone());
        k.save_due = Some(now + SAVE_S);
        let said = if merged {
            t!(
                "keywords-merging",
                from = kw::display(from),
                to = kw::display(&to)
            )
        } else {
            t!(
                "keywords-renaming",
                from = kw::display(from),
                to = kw::display(&to)
            )
        };
        k.status = Some(said.clone());
        self.chrome.announce_status(said);
        // The photos read so far that have it; the Library adds its own.
        let mut known: Vec<PathBuf> = k
            .files
            .iter()
            .filter(|(_, l)| l.iter().any(|x| kw::is_within(x, from)))
            .map(|(p, _)| p.clone())
            .collect();
        known.sort();
        // Assumed at once (the write's result follows).
        for l in k.files.values_mut() {
            if l.iter().any(|x| kw::is_within(x, from)) {
                *l = kw::rename_in(l, from, &to);
            }
        }
        let done = Arc::new(AtomicUsize::new(0));
        let total = Arc::new(AtomicUsize::new(0));
        k.renaming = Some(Renaming {
            from: from.to_owned(),
            to: to.clone(),
            done: done.clone(),
            total: total.clone(),
        });
        k.writing += 1;
        let (tx, vocab, wake) = (k.tx.clone(), k.vocab.clone(), self.loader.waker());
        let lib = self.gallery.library_handle();
        let from = from.to_owned();
        self.net_refresh();
        self.redraw = true;
        if self.args.screenshot.is_some() {
            return;
        }
        self.loader.background("kw-rename", move |_| {
            let mut photos = known;
            if let Some(lib) = lib
                && let Ok(more) = lib.files_with_tag(&from)
            {
                photos.extend(more);
            }
            photos.sort();
            photos.dedup();
            total.store(photos.len(), Ordering::Relaxed);
            let r = files::rename_all(&photos, &from, &to, &vocab, &done);
            let _ = tx.send(Msg::Renamed { from, to });
            let _ = tx.send(Msg::Wrote { r, undo: false });
            wake();
        });
    }

    /// The downloads row of a keyword rename going through photos.
    pub(super) fn keywords_rename_rows(&self) -> Vec<specular_widgets::DownloadRow> {
        use specular_widgets::{DownloadRow, DownloadState};
        use std::sync::atomic::Ordering;
        let Some(r) = &self.gallery.keywords.renaming else {
            return Vec::new();
        };
        let total = r.total.load(Ordering::Relaxed);
        let done = r.done.load(Ordering::Relaxed).min(total);
        vec![DownloadRow {
            title: t!(
                "keywords-rename-row",
                from = kw::display(&r.from),
                to = kw::display(&r.to)
            ),
            detail: if total == 0 {
                t!("keywords-rename-finding")
            } else {
                t!(
                    "keywords-rename-row-detail",
                    done = done as i64,
                    total = total as i64
                )
            },
            status: String::new(),
            fraction: (total > 0).then(|| done as f32 / total as f32),
            state: DownloadState::Running,
            stop: None,
        }]
    }

    /// Undo of a keyword change: each photo gets back the keywords it had (sidecars written
    /// again; the panel and the Library follow when the write comes back).
    pub(super) fn keywords_restore(&mut self, lists: Vec<(PathBuf, Vec<String>)>) {
        let k = &mut self.gallery.keywords;
        for (p, l) in &lists {
            k.files.insert(p.clone(), l.clone());
        }
        k.status = None;
        k.writing += 1;
        let (tx, vocab, wake) = (k.tx.clone(), k.vocab.clone(), self.loader.waker());
        self.redraw = true;
        self.loader.background("kw-undo", move |_| {
            let r = files::restore_all(&lists, &vocab);
            let _ = tx.send(Msg::Wrote { r, undo: true });
            wake();
        });
    }

    /// A key while the panel shows. True: the panel used it.
    pub(super) fn keywords_key(&mut self, ev: &KeyEvent) -> bool {
        if !self.gallery.keywords.has_focus() {
            return false;
        }
        let Some(combo) = self.key_combo(ev) else {
            // A key without a shortcut form: its text.
            let Some(t) = ev.text.as_ref() else {
                return false;
            };
            let out = self.gallery.keywords.type_text(t.as_str());
            return self.keywords_out(out);
        };
        // Shift+K again closes the panel (the field takes a capital K as text).
        if !self.gallery.keywords.field_focus
            && self.keymap.lookup(&combo) == Some(specular_input::Action::Keywords)
        {
            return false;
        }
        let mut clip = PendingClipboard::default();
        let text = ev.text.as_ref().map(|t| t.to_string());
        let out = self
            .gallery
            .keywords
            .key(combo.key, combo.mods, text.as_deref(), &mut clip);
        self.field_clipboard(clip, super::integration::PasteTo::Keywords);
        self.keywords_out(out)
    }

    fn keywords_out(&mut self, out: KeyOut) -> bool {
        self.redraw = true;
        match out {
            KeyOut::No => false,
            KeyOut::Used => true,
            KeyOut::Apply(c) => {
                self.keywords_apply(c);
                true
            }
            KeyOut::Rename(from, to) => {
                self.keywords_rename(&from, &to);
                true
            }
            KeyOut::Close => {
                self.gallery.keywords.close();
                self.gallery_keywords_closed();
                true
            }
            KeyOut::Button(b) => {
                self.keywords_button(b);
                true
            }
        }
    }

    fn keywords_button(&mut self, b: Button) {
        let now = self.now();
        match b {
            Button::Import => self.keywords_import(),
            Button::Export => self.keywords_export(),
            b => {
                if self.gallery.keywords.button(b) {
                    self.gallery.keywords.save_due = Some(now + SAVE_S);
                }
            }
        }
    }

    /// A press over the panel (logical px). True: it was the panel's.
    pub(super) fn keywords_press(&mut self, p: (f32, f32)) -> bool {
        let k = &mut self.gallery.keywords;
        if !k.open || !k.rect.is_some_and(|r| r.contains(p)) {
            // A click elsewhere gives the keyboard back.
            if k.open {
                k.field_focus = false;
                k.focus = None;
            }
            return false;
        }
        let Some(h) = k.hit_at(p) else { return true };
        let out = k.click(h);
        self.keywords_out(out)
    }

    /// The wheel over the panel. True: it scrolled.
    pub(super) fn keywords_wheel(&mut self, p: (f32, f32), dy: f32) -> bool {
        let k = &mut self.gallery.keywords;
        if !k.open || !k.rect.is_some_and(|r| r.contains(p)) {
            return false;
        }
        k.scroll = (k.scroll - dy).max(0.0);
        self.redraw = true;
        true
    }

    /// Text pasted into the field.
    pub(super) fn keywords_paste(&mut self, s: &str) {
        let k = &mut self.gallery.keywords;
        if k.open && k.field_focus {
            k.edit.paste(&specular_widgets::line_edit::single_line(s));
            self.redraw = true;
        }
    }

    /// An assistive technology's action on node `id`. True: it was the panel's.
    pub(super) fn keywords_a11y_action(&mut self, id: u64) -> bool {
        let Some(h) = self.gallery.keywords.a11y_hit(id) else {
            return false;
        };
        let out = self.gallery.keywords.click(h);
        self.keywords_out(out);
        true
    }

    fn keywords_import(&mut self) {
        let (tx, wake) = (self.gallery.keywords.tx.clone(), self.loader.waker());
        let (title, kind) = (t!("keywords-dialog-import"), t!("keywords-file-kind"));
        self.loader.background("dlg-keywords", move |_| {
            let Some(p) = rfd::FileDialog::new()
                .set_title(&title)
                .add_filter(&kind, &["txt"])
                .pick_file()
            else {
                return;
            };
            let r = std::fs::read(&p)
                .map_err(|e| e.to_string())
                .map(|b| Vocabulary::from_lightroom(&decode_text(&b)));
            let _ = tx.send(Msg::Imported(p, r));
            wake();
        });
    }

    fn keywords_export(&mut self) {
        let k = &self.gallery.keywords;
        let (tx, wake) = (k.tx.clone(), self.loader.waker());
        let text = k.vocab.to_lightroom();
        let (title, kind) = (t!("keywords-dialog-export"), t!("keywords-file-kind"));
        let name = t!("keywords-file-name");
        self.loader.background("dlg-keywords", move |_| {
            let Some(p) = rfd::FileDialog::new()
                .set_title(&title)
                .add_filter(&kind, &["txt"])
                .set_file_name(format!("{name}.txt"))
                .save_file()
            else {
                return;
            };
            let r = crate::app::gallery::marks::write_atomic(&p, text.as_bytes())
                .map(|()| p)
                .map_err(|e| e.to_string());
            let _ = tx.send(Msg::Exported(r));
            wake();
        });
    }
}

/// Where the panel goes over cull mode's photo: the trailing edge, from the top of `area`
/// to the strip at `strip_top`.
pub(super) fn cull_rect(
    area: (f32, f32, f32, f32),
    strip_top: f32,
    text_scale: f32,
    rtl: bool,
) -> Rect {
    let (ax, ay, aw, _) = area;
    let w = side_panel::width(aw, text_scale);
    let x = if rtl { ax } else { ax + aw - w };
    Rect::new(x, ay, w, (strip_top - ay).max(0.0))
}

/// A keyword list file's text: UTF-8 (with or without a byte order mark), UTF-16 with one
/// (Windows editors), else Latin-1.
fn decode_text(b: &[u8]) -> String {
    if let Some(rest) = b.strip_prefix(&[0xFF, 0xFE]) {
        let u: Vec<u16> = rest
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&u);
    }
    if let Some(rest) = b.strip_prefix(&[0xFE, 0xFF]) {
        let u: Vec<u16> = rest
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&u);
    }
    match std::str::from_utf8(b) {
        Ok(s) => s.to_owned(),
        Err(_) => b.iter().map(|&c| char::from(c)).collect(),
    }
}

#[cfg(test)]
mod tests;
