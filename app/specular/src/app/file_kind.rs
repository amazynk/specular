//! "Which kind of file is …?": the prompt for a file whose extension several formats share
//! and whose content does not settle which it is ([`specular_decode::ident`]).
//!
//! - **When**: an open fails with [`DecodeError::Ambiguous`]; the prompt lists the formats
//!   it may be, the likeliest first (by the decoders' confidence, then by how often this
//!   person picked each for the extension, then by how common the format is), each with a
//!   line saying what it is. A headerless kind of file (`.bin`, `.dat`, `.img`, `.raw`) that
//!   no format recognises gets the prompt too, offering raw samples (the raw volume dialog).
//! - **Remember**: "This file", ".ext files in this folder" or "All .ext files". Rules are
//!   kept in `file-kinds.tsv` in the data folder with the picks' history, and handed to
//!   the decoders ([`ident::set_rules`]) at start and on every change.
//! - **Change it later**: "Open as…" (the gallery's file menu and the palette) asks again,
//!   offering every format the file may be; Settings › Optional decoders lists the rules,
//!   each with Forget… (asks first).
//!
//! Folder listings never ask: a thumbnail opens the likeliest format that works.
//!
//! [`DecodeError::Ambiguous`]: specular_decode::DecodeError::Ambiguous

#[cfg(test)]
mod shots;
mod view;

use std::path::{Path, PathBuf};

use specular_decode::ident::{self, Candidate, Confidence, FolderHint, Rule};
use specular_decode::{Format, Registry};
use specular_gfx::{DrawList, Text};
use specular_i18n::{I18n, t};
use specular_input::{Key, KeyCombo, NamedKey};
use specular_log::{SETTINGS, UI, info, warn};
use specular_theme::Palette;
use specular_widgets::{PanelEvent, PanelRow, SettingPanel, accesskit};

use self::view::{Hit, OptionView, Part, PromptView};
use super::App;
use crate::loader::ImageId;

/// The setting the remembered rules' panel sits under.
pub(super) const UNDER: &str = "decoders.file_kinds";
/// The confirmation of a rule's Forget….
pub(super) const FORGET_ITEM: u64 = super::items::FILE_KIND_FORGET;

/// Extensions of headerless data: offered as raw samples when nothing else reads them.
const HEADERLESS: [&str; 5] = ["bin", "dat", "img", "raw", "vol"];

/// Whether `path` is named like headerless data ([`HEADERLESS`]).
pub(super) fn headerless(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| HEADERLESS.contains(&e.to_string_lossy().to_ascii_lowercase().as_str()))
}

/// Whether the file menu offers "Open as…" for `path`: its extension is shared or names
/// headerless data, or its kind was asked about or chosen.
pub(crate) fn offers_open_as(path: &Path) -> bool {
    let shared = path
        .extension()
        .is_some_and(|e| ident::shared(&e.to_string_lossy().to_ascii_lowercase()).is_some());
    shared
        || headerless(path)
        || ident::chosen(path).is_some()
        || ident::ambiguity_of(path).is_some()
}

/// Rules kept at most; the oldest go first.
const MAX_RULES: usize = 512;

/// How widely an answer is remembered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Scope {
    File,
    Folder,
    Everywhere,
}

const SCOPES: [Scope; 3] = [Scope::File, Scope::Folder, Scope::Everywhere];

/// A row of the prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Choice {
    Format(Candidate),
    /// Raw samples: the raw volume dialog asks for the layout.
    RawSamples,
}

/// An open prompt.
pub(super) struct Dialog {
    pub file: PathBuf,
    /// Lower case, without the dot (empty for none).
    ext: String,
    choices: Vec<Choice>,
    selected: usize,
    scope: Scope,
    focus: Part,
    /// No format recognised the content (only raw samples, or a guess, is left).
    unknown: bool,
    drawn: view::Drawn,
}

#[derive(Default)]
pub(super) struct FileKind {
    pub dialog: Option<Dialog>,
    /// The rules a Forget… is being confirmed for.
    forgetting: Vec<Rule>,
    /// The panel was built from (store revision, UI language generation).
    built: Option<(u64, u64)>,
    revision: u64,
}

// ----- The store ------------------------------------------------------------------------

/// What `file-kinds.tsv` holds: the rules, newest last, and how often each format was
/// picked per extension.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Store {
    pub rules: Vec<Rule>,
    pub picks: Vec<(String, Format, u32)>,
}

impl Store {
    /// Reads the store's `text`; lines it cannot read (a format no longer known) are
    /// dropped.
    pub fn parse(text: &str, registry: &Registry) -> Store {
        let mut s = Store::default();
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            match f.as_slice() {
                ["rule", ext, folder, key] => {
                    if let Some(format) = registry.format_by_key(key) {
                        s.rules.push(Rule {
                            ext: (*ext).to_owned(),
                            folder: (!folder.is_empty()).then(|| PathBuf::from(folder)),
                            format,
                        });
                    }
                }
                ["pick", ext, key, n] => {
                    if let (Some(format), Ok(n)) = (registry.format_by_key(key), n.parse()) {
                        s.picks.push(((*ext).to_owned(), format, n));
                    }
                }
                _ => {}
            }
        }
        s
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for r in &self.rules {
            let folder = r.folder.as_deref().and_then(Path::to_str).unwrap_or("");
            out.push_str(&format!(
                "rule\t{}\t{folder}\t{}\n",
                r.ext,
                ident::key(r.format)
            ));
        }
        for (ext, f, n) in &self.picks {
            out.push_str(&format!("pick\t{ext}\t{}\t{n}\n", ident::key(*f)));
        }
        out
    }

    /// Puts `rule` last, replacing the one for the same extension and folder. A folder whose
    /// path cannot be stored (tabs, line breaks, not UTF-8) is not remembered.
    pub fn add_rule(&mut self, rule: Rule) -> bool {
        if let Some(f) = &rule.folder
            && f.to_str().is_none_or(|s| s.contains(['\t', '\n', '\r']))
        {
            return false;
        }
        self.rules
            .retain(|r| !(r.ext == rule.ext && r.folder == rule.folder));
        self.rules.push(rule);
        let skip = self.rules.len().saturating_sub(MAX_RULES);
        self.rules.drain(..skip);
        true
    }

    pub fn forget(&mut self, rule: &Rule) {
        self.rules.retain(|r| r != rule);
    }

    pub fn picked(&mut self, ext: &str, format: Format) {
        match self.picks.iter_mut().find(|p| p.0 == ext && p.1 == format) {
            Some(p) => p.2 = p.2.saturating_add(1),
            None => self.picks.push((ext.to_owned(), format, 1)),
        }
    }

    fn picks_of(&self, ext: &str, format: Format) -> u32 {
        self.picks
            .iter()
            .find(|p| p.0 == ext && p.1 == format)
            .map_or(0, |p| p.2)
    }

    /// Orders ranked `candidates` among equals (the same confidence and word from the
    /// folder) by how often each was picked for `ext`; the sort is stable, so the decoders'
    /// order stays among the never picked.
    pub fn by_history(&self, ext: &str, candidates: &mut [Candidate]) {
        candidates.sort_by(|a, b| {
            b.confidence
                .cmp(&a.confidence)
                .then(b.hint.cmp(&a.hint))
                .then(
                    self.picks_of(ext, b.format)
                        .cmp(&self.picks_of(ext, a.format)),
                )
        });
    }
}

/// Where the store is kept.
fn store_path() -> PathBuf {
    specular_settings::Locations::detect()
        .data_dir
        .join("file-kinds.tsv")
}

fn load(registry: &Registry) -> Store {
    Store::parse(
        &std::fs::read_to_string(store_path()).unwrap_or_default(),
        registry,
    )
}

fn save(s: &Store) {
    if let Err(e) = specular_settings::write_atomic(&store_path(), &s.to_text()) {
        warn!(target: UI, "file kinds: {e}");
    }
    ident::set_rules(s.rules.clone());
}

/// At start, before the first file opens: hands the remembered rules to the decoders.
pub(crate) fn startup() {
    let s = load(&Registry::new());
    if !s.rules.is_empty() {
        info!(target: SETTINGS, "file kinds: {} remembered rule(s)", s.rules.len());
    }
    ident::set_rules(s.rules);
}

// ----- Words ----------------------------------------------------------------------------

/// The message id of a format's one-line description: `file-kind-about-` and its key in
/// kebab case (`Format::RawVolume` → `file-kind-about-raw-volume`).
pub(super) fn about_id(f: Format) -> String {
    let mut id = String::from("file-kind-about-");
    for (i, ch) in ident::key(f).chars().enumerate() {
        if ch.is_ascii_uppercase() && i > 0 {
            id.push('-');
        }
        id.push(ch.to_ascii_lowercase());
    }
    id
}

fn about(f: Format, ext: &str) -> String {
    let id = about_id(f);
    if I18n::global().localizer().has_message(&id) {
        I18n::global().tr(&id)
    } else {
        t!("file-kind-about-other", ext = ext.to_owned())
    }
}

fn tag(c: Confidence) -> String {
    match c {
        Confidence::Certain => t!("file-kind-certain"),
        Confidence::Likely => t!("file-kind-likely"),
        Confidence::Possible => t!("file-kind-possible"),
        Confidence::Guess => t!("file-kind-guess"),
    }
}

/// The row's tag: how sure the check is (left out when every row is a guess) and what the
/// folder says.
fn tag_of(c: &Candidate, unknown: bool, ext: &str) -> String {
    let mut parts = Vec::new();
    if !unknown {
        parts.push(tag(c.confidence));
    }
    match c.hint {
        Some(FolderHint::SameKind) => parts.push(t!("file-kind-hint-same", ext = ext.to_owned())),
        Some(FolderHint::Companions) => parts.push(t!("file-kind-hint-companions")),
        None => {}
    }
    parts.join(" · ")
}

fn scope_name(s: Scope, ext: &str) -> String {
    match s {
        Scope::File => t!("file-kind-scope-file"),
        Scope::Folder => t!("file-kind-scope-folder", ext = ext.to_owned()),
        Scope::Everywhere => t!("file-kind-scope-everywhere", ext = ext.to_owned()),
    }
}

impl Dialog {
    /// The prompt for `file` with `candidates` ranked; raw samples are offered for
    /// headerless kinds of file.
    fn new(file: &Path, candidates: Vec<Candidate>, chosen: Option<Format>) -> Dialog {
        let ext = file
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let unknown = candidates.iter().all(|c| c.confidence == Confidence::Guess);
        let mut choices: Vec<Choice> = candidates
            .into_iter()
            .filter(|c| c.format != Format::RawVolume)
            .map(Choice::Format)
            .collect();
        if HEADERLESS.contains(&ext.as_str()) {
            choices.push(Choice::RawSamples);
        }
        let selected = chosen
            .and_then(|f| {
                choices.iter().position(|c| match c {
                    Choice::Format(c) => c.format == f,
                    Choice::RawSamples => f == Format::RawVolume,
                })
            })
            .unwrap_or(0);
        Dialog {
            file: file.to_path_buf(),
            ext,
            choices,
            selected,
            scope: Scope::File,
            focus: Part::Option(selected),
            unknown,
            drawn: view::Drawn::default(),
        }
    }

    fn remembers(&self) -> bool {
        matches!(self.choices.get(self.selected), Some(Choice::Format(_))) && !self.ext.is_empty()
    }

    fn order(&self) -> Vec<Part> {
        Part::order(self.choices.len(), self.remembers())
    }

    fn select(&mut self, i: usize) {
        if i < self.choices.len() {
            self.selected = i;
            self.focus = Part::Option(i);
        }
    }

    fn step_scope(&mut self, by: i32) {
        let i = SCOPES.iter().position(|s| *s == self.scope).unwrap_or(0) as i32;
        self.scope = SCOPES[(i + by).rem_euclid(SCOPES.len() as i32) as usize];
    }
}

fn build_view(d: &Dialog) -> PromptView {
    let name = d.file.file_name().map_or_else(
        || d.file.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let only_raw = d.choices == [Choice::RawSamples];
    let subtitle = if only_raw {
        t!("file-kind-why-none")
    } else if d.unknown {
        t!("file-kind-why-name", ext = d.ext.clone())
    } else {
        t!("file-kind-why-content")
    };
    let options = d
        .choices
        .iter()
        .map(|c| match c {
            Choice::Format(c) => OptionView {
                name: c.format.name().to_owned(),
                // Every row is a guess when nothing matched: no tag says so on each.
                tag: tag_of(c, d.unknown, &d.ext),
                about: about(c.format, &d.ext),
            },
            Choice::RawSamples => OptionView {
                name: t!("file-kind-raw"),
                tag: String::new(),
                about: t!("file-kind-about-raw-volume"),
            },
        })
        .collect();
    PromptView {
        title: t!("file-kind-title", file = name),
        subtitle,
        options,
        selected: d.selected,
        scope: d
            .remembers()
            .then(|| (t!("file-kind-scope"), scope_name(d.scope, &d.ext))),
        note: t!("file-kind-later"),
        cancel: t!("app-cancel"),
        open: t!("file-kind-open"),
        focus: d.focus,
    }
}

impl FileKind {
    pub(super) fn is_open(&self) -> bool {
        self.dialog.is_some()
    }

    pub(super) fn view(&self) -> Option<PromptView> {
        self.dialog.as_ref().map(build_view)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        v: Option<&PromptView>,
        size: (f32, f32),
        rtl: bool,
        ts: f32,
    ) {
        if let (Some(v), Some(d)) = (v, self.dialog.as_mut()) {
            d.drawn = view::draw(dl, text, pal, v, size, rtl, ts);
        }
    }

    pub(super) fn a11y(&self, tree: &mut accesskit::TreeUpdate, v: &PromptView, scale: f64) {
        if let Some(d) = &self.dialog {
            view::a11y(tree, v, &d.drawn, scale);
        }
    }
}

/// Whether an open's error text is "no format read it" (nothing else to try).
fn unrecognised(error: &str) -> bool {
    error == specular_decode::DecodeError::UnsupportedFormat.to_string()
        || error.starts_with("none of the .")
}

// ----- Settings › Optional decoders › File kinds -----------------------------------------

/// The panel listing the remembered rules, each with Forget….
pub(super) fn panel(rules: &[Rule]) -> SettingPanel {
    let mut rows = Vec::new();
    for (i, r) in rules.iter().enumerate() {
        rows.push(PanelRow {
            id: format!("rule:{i}"),
            label: t!(
                "file-kind-rule",
                ext = r.ext.clone(),
                format = r.format.name()
            ),
            value: match &r.folder {
                Some(f) => t!("file-kind-rule-folder", folder = f.display().to_string()),
                None => t!("file-kind-rule-everywhere"),
            },
            button: Some(t!("file-kind-forget")),
            ..Default::default()
        });
    }
    SettingPanel {
        intro: if rules.is_empty() {
            t!("file-kind-rules-none")
        } else {
            t!("file-kind-rules-intro")
        },
        rows,
        keywords: t!("file-kind-keywords"),
        ..Default::default()
    }
}

impl App {
    /// An image failed to open: when the content left several formats (or, for headerless
    /// data, none), asks which kind of file it is.
    pub(super) fn file_kind_failed(&mut self, id: ImageId) {
        if self.current_id() != Some(id) || self.file_kind.is_open() || self.raw_volume.is_open() {
            return;
        }
        let Some(img) = self.images.get(&id) else {
            return;
        };
        let path = img.path.clone();
        let error = img.error.clone().unwrap_or_default();
        let candidates = match ident::ambiguity_of(&path) {
            Some(a) => a.candidates,
            None => {
                let ext = path
                    .extension()
                    .map(|e| e.to_string_lossy().to_ascii_lowercase())
                    .unwrap_or_default();
                if !(HEADERLESS.contains(&ext.as_str()) && unrecognised(&error)) {
                    return;
                }
                Vec::new()
            }
        };
        self.file_kind_ask(&path, candidates, None);
    }

    fn file_kind_ask(
        &mut self,
        path: &Path,
        mut candidates: Vec<Candidate>,
        chosen: Option<Format>,
    ) {
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        load(&self.loader.registry()).by_history(&ext, &mut candidates);
        info!(target: UI, "file kind of {}: asking ({} candidate(s))", path.display(), candidates.len());
        self.file_kind.dialog = Some(Dialog::new(path, candidates, chosen));
        self.redraw = true;
    }

    /// "Open as…": asks which kind of file the focused or shown file is, offering every
    /// format it may be.
    pub(super) fn file_kind_open_as(&mut self) {
        let Some(path) = self.integration_file() else {
            return;
        };
        let now = self.now();
        let candidates = match specular_core::Source::open(&path) {
            Ok(src) => self.loader.registry().identify(&src),
            Err(e) => {
                self.chrome.toast(e.to_string(), None, now);
                return;
            }
        };
        let raw_ok = path.extension().is_some_and(|e| {
            HEADERLESS.contains(&e.to_string_lossy().to_ascii_lowercase().as_str())
        });
        if candidates.is_empty() && !raw_ok {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.chrome
                .toast(t!("file-kind-open-as-none", file = name), None, now);
            return;
        }
        self.file_kind_ask(&path, candidates, ident::chosen(&path));
    }

    pub(super) fn file_kind_view(&self) -> Option<PromptView> {
        self.file_kind.view()
    }

    /// Keys while the prompt is open (it takes them all).
    pub(super) fn file_kind_key(&mut self, combo: Option<&KeyCombo>) {
        let rtl = self.rtl();
        let Some(d) = self.file_kind.dialog.as_mut() else {
            return;
        };
        let Some(c) = combo else { return };
        self.redraw = true;
        let order = d.order();
        let at = order.iter().position(|p| *p == d.focus).unwrap_or(0) as isize;
        let move_focus = |d: &mut Dialog, by: isize| {
            d.focus = order[(at + by).rem_euclid(order.len() as isize) as usize];
        };
        match c.key {
            Key::Named(NamedKey::Escape) => self.file_kind_cancel(),
            Key::Named(NamedKey::Enter) => match d.focus {
                Part::Cancel => self.file_kind_cancel(),
                _ => self.file_kind_submit(),
            },
            Key::Named(NamedKey::Space) => match d.focus {
                Part::Option(i) => d.select(i),
                Part::Scope => d.step_scope(1),
                Part::Cancel => self.file_kind_cancel(),
                Part::Open => self.file_kind_submit(),
            },
            Key::Named(NamedKey::Tab) => move_focus(d, if c.mods.shift { -1 } else { 1 }),
            // Arrows move the choice within the formats, as in a radio group.
            Key::Named(k @ (NamedKey::ArrowUp | NamedKey::ArrowDown)) => {
                let down = k == NamedKey::ArrowDown;
                match d.focus {
                    Part::Option(i) => {
                        let n = d.choices.len();
                        d.select(if down { (i + 1) % n } else { (i + n - 1) % n });
                    }
                    _ => move_focus(d, if down { 1 } else { -1 }),
                }
            }
            Key::Named(NamedKey::Home) if matches!(d.focus, Part::Option(_)) => d.select(0),
            Key::Named(NamedKey::End) if matches!(d.focus, Part::Option(_)) => {
                d.select(d.choices.len().saturating_sub(1));
            }
            Key::Named(k @ (NamedKey::ArrowLeft | NamedKey::ArrowRight)) => {
                let forward = (k == NamedKey::ArrowRight) != rtl;
                match d.focus {
                    Part::Scope => d.step_scope(if forward { 1 } else { -1 }),
                    Part::Cancel | Part::Open => {
                        d.focus = if forward { Part::Open } else { Part::Cancel }
                    }
                    Part::Option(_) => {}
                }
            }
            // A digit picks that row.
            Key::Char(ch) if ch.is_ascii_digit() && !(c.mods.ctrl || c.mods.meta) => {
                let i = (ch as u8 - b'0') as usize;
                if i >= 1 {
                    d.select(i - 1);
                }
            }
            _ => {}
        }
    }

    pub(super) fn file_kind_click(&mut self, p: (f32, f32), down: bool) {
        if !down {
            return;
        }
        let Some(d) = self.file_kind.dialog.as_mut() else {
            return;
        };
        // The scope's arrows lie on its row: they come first.
        let mut hits: Vec<_> = d.drawn.hits.iter().filter(|(r, _)| r.contains(p)).collect();
        hits.sort_by_key(|(_, h)| !matches!(h, Hit::Dec | Hit::Inc));
        let Some(&&(_, h)) = hits.first() else {
            return;
        };
        self.redraw = true;
        match h {
            Hit::Dec => {
                d.focus = Part::Scope;
                d.step_scope(-1);
            }
            Hit::Inc => {
                d.focus = Part::Scope;
                d.step_scope(1);
            }
            Hit::Part(Part::Option(i)) => d.select(i),
            Hit::Part(Part::Cancel) => self.file_kind_cancel(),
            Hit::Part(Part::Open) => self.file_kind_submit(),
            Hit::Part(p) => d.focus = p,
        }
    }

    pub(super) fn file_kind_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        let Some(d) = self.file_kind.dialog.as_mut() else {
            return false;
        };
        let Some(part) = view::a11y_target(req.target_node.0) else {
            return (view::A11Y_BASE..view::A11Y_BASE + 0x1000).contains(&req.target_node.0);
        };
        self.redraw = true;
        match (req.action, part) {
            (accesskit::Action::Click, Part::Option(i)) => d.select(i),
            (accesskit::Action::Increment, Part::Scope) => d.step_scope(1),
            (accesskit::Action::Decrement, Part::Scope) => d.step_scope(-1),
            (accesskit::Action::Click, Part::Cancel) => self.file_kind_cancel(),
            (accesskit::Action::Click, Part::Open) => self.file_kind_submit(),
            (accesskit::Action::Focus | accesskit::Action::Click, p) => d.focus = p,
            _ => {}
        }
        true
    }

    /// Open: opens the file as the chosen format (remembering it as asked), or asks for raw
    /// samples' layout.
    fn file_kind_submit(&mut self) {
        let Some(d) = self.file_kind.dialog.take() else {
            return;
        };
        self.redraw = true;
        let Some(choice) = d.choices.get(d.selected).copied() else {
            return;
        };
        match choice {
            Choice::RawSamples => {
                ident::choose(&d.file, Format::RawVolume);
                let len = std::fs::metadata(&d.file).map_or(0, |m| m.len());
                self.raw_volume_open(&d.file, len);
            }
            Choice::Format(c) => {
                info!(target: UI, "{} opens as {} ({:?})", d.file.display(), c.format.name(), d.scope);
                ident::choose(&d.file, c.format);
                let mut store = load(&self.loader.registry());
                store.picked(&d.ext, c.format);
                let folder = match d.scope {
                    Scope::File => None,
                    Scope::Folder => Some(d.file.parent().map(Path::to_path_buf)),
                    Scope::Everywhere => Some(None),
                };
                if let Some(folder) = folder {
                    store.add_rule(Rule {
                        ext: d.ext.clone(),
                        folder,
                        format: c.format,
                    });
                }
                save(&store);
                self.file_kind.revision += 1;
                self.file_kind_reopen(&d.file);
            }
        }
    }

    /// Shows `file` afresh: reloaded when it is the image on screen, opened from the
    /// gallery when it is the focused tile there.
    fn file_kind_reopen(&mut self, file: &Path) {
        self.reload_path(file);
        if self.gallery.active
            && self.gallery.focus_file().as_deref() == Some(file)
            && let Some(i) = self.gallery.focus_index()
        {
            self.gallery_open(i);
        }
    }

    /// Cancel: the file stays shown as not opened.
    fn file_kind_cancel(&mut self) {
        self.file_kind.dialog = None;
        self.redraw = true;
    }

    /// Once per turn with Settings open: keeps the rules' panel up to date.
    pub(super) fn file_kind_tick(&mut self) {
        if self.chrome.modal() != Some(specular_widgets::Modal::Settings) {
            return;
        }
        let built = (self.file_kind.revision, I18n::global().generation());
        if self.file_kind.built != Some(built) {
            self.file_kind.built = Some(built);
            let rules = load(&self.loader.registry()).rules;
            if self.chrome.set_setting_panel(UNDER, Some(panel(&rules))) {
                self.redraw = true;
            }
        }
    }

    /// A rule's Forget…: asks first.
    pub(super) fn file_kind_panel_event(&mut self, e: &PanelEvent) {
        let PanelEvent::Row(row) = e else { return };
        let Some(i) = row
            .strip_prefix("rule:")
            .and_then(|i| i.parse::<usize>().ok())
        else {
            return;
        };
        let Some(rule) = load(&self.loader.registry()).rules.get(i).cloned() else {
            return;
        };
        self.chrome.confirm_destructive(
            t!(
                "file-kind-forget-ask",
                ext = rule.ext.clone(),
                format = rule.format.name()
            ),
            t!("file-kind-forget-detail"),
            t!("file-kind-forget-ok"),
            FORGET_ITEM,
        );
        self.file_kind.forgetting = vec![rule];
        self.redraw = true;
    }

    /// "Forget all…" (the setting's button): asks first, naming how many.
    pub(super) fn file_kind_forget_all(&mut self) {
        let rules = load(&self.loader.registry()).rules;
        let now = self.now();
        if rules.is_empty() {
            self.chrome.toast(t!("file-kind-rules-none"), None, now);
            return;
        }
        self.chrome.confirm_destructive(
            t!("file-kind-forget-all-ask", count = rules.len() as i64),
            t!("file-kind-forget-detail"),
            t!("file-kind-forget-ok"),
            FORGET_ITEM,
        );
        self.file_kind.forgetting = rules;
        self.redraw = true;
    }

    /// Forget… was confirmed: the rules go; files they settled are asked about again.
    pub(super) fn file_kind_forget_confirmed(&mut self) {
        let rules = std::mem::take(&mut self.file_kind.forgetting);
        if rules.is_empty() {
            return;
        }
        let mut store = load(&self.loader.registry());
        for r in &rules {
            store.forget(r);
            info!(target: SETTINGS, "file kinds: forgot .{} → {}", r.ext, r.format.name());
        }
        save(&store);
        self.file_kind.revision += 1;
        let now = self.now();
        let text = match rules.as_slice() {
            [r] => t!(
                "file-kind-forgotten",
                ext = r.ext.clone(),
                format = r.format.name()
            ),
            _ => t!("file-kind-forgotten-all", count = rules.len() as i64),
        };
        self.chrome.toast(text, None, now);
        self.redraw = true;
    }

    /// `--show file-kind`: the prompt for a `.pic` that two formats fit. False for any
    /// other scene.
    pub(super) fn file_kind_show(&mut self, scene: &str) -> bool {
        if scene != "file-kind" {
            return false;
        }
        self.file_kind.dialog = Some(Dialog::new(
            Path::new("/pictures/scan.pic"),
            demo_candidates(),
            None,
        ));
        self.redraw = true;
        true
    }
}

/// The candidates the demo scene and the screenshots show.
fn demo_candidates() -> Vec<Candidate> {
    vec![
        Candidate {
            hint: Some(FolderHint::SameKind),
            ..Candidate::new(Format::Pict, Confidence::Likely)
        },
        Candidate::new(Format::Hdr, Confidence::Possible),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(ext: &str, folder: Option<&str>, format: Format) -> Rule {
        Rule {
            ext: ext.into(),
            folder: folder.map(PathBuf::from),
            format,
        }
    }

    #[test]
    fn the_store_round_trips_and_replaces_rules() {
        let reg = Registry::new();
        let mut s = Store::default();
        assert!(s.add_rule(rule("pic", Some("/a"), Format::Pict)));
        assert!(s.add_rule(rule("pic", None, Format::Hdr)));
        // The same extension and folder: replaced, put last.
        assert!(s.add_rule(rule("pic", Some("/a"), Format::Hdr)));
        assert!(!s.add_rule(rule("pic", Some("/a\tb"), Format::Hdr)));
        s.picked("pic", Format::Hdr);
        s.picked("pic", Format::Hdr);
        s.picked("pic", Format::Pict);
        let text = s.to_text();
        let back = Store::parse(&text, &reg);
        assert_eq!(back, s);
        assert_eq!(
            back.rules,
            [
                rule("pic", None, Format::Hdr),
                rule("pic", Some("/a"), Format::Hdr)
            ]
        );
        // Lines it cannot read are dropped.
        let back = Store::parse(
            "rule\tpic\t\tNoSuchFormat\ngarbage\npick\tx\tPng\tmany\n",
            &reg,
        );
        assert_eq!(back, Store::default());
        s.forget(&rule("pic", None, Format::Hdr));
        assert_eq!(s.rules, [rule("pic", Some("/a"), Format::Hdr)]);
    }

    #[test]
    fn history_orders_equals_only() {
        let mut s = Store::default();
        s.picked("pic", Format::Hdr);
        let c = Candidate::new;
        let mut v = vec![
            c(Format::Pict, Confidence::Likely),
            c(Format::Pcx, Confidence::Guess),
            c(Format::Hdr, Confidence::Guess),
        ];
        s.by_history("pic", &mut v);
        let order: Vec<Format> = v.iter().map(|c| c.format).collect();
        // Picked before, but a weaker check than PICT's: second, ahead of PCX.
        assert_eq!(order, [Format::Pict, Format::Hdr, Format::Pcx]);
    }

    /// The prompt has a line on what each format is for every format it may offer.
    #[test]
    fn every_shared_format_is_described() {
        let loc = I18n::global().localizer();
        let formats = ident::SHARED
            .iter()
            .flat_map(|s| s.formats.iter().copied())
            .chain(ident::COMPANIONS.iter().map(|c| c.0))
            .chain([Format::RawVolume]);
        let missing: Vec<String> = formats
            .map(about_id)
            .filter(|id| !loc.has_message(id))
            .collect();
        assert!(missing.is_empty(), "add to file-kinds.ftl: {missing:?}");
    }

    #[test]
    fn description_ids_are_kebab_case() {
        assert_eq!(about_id(Format::RawVolume), "file-kind-about-raw-volume");
        assert_eq!(about_id(Format::Pict), "file-kind-about-pict");
    }

    #[test]
    fn headerless_data_offers_raw_samples_and_remembers_nothing() {
        let d = Dialog::new(Path::new("/d/blob.bin"), Vec::new(), None);
        assert_eq!(d.choices, [Choice::RawSamples]);
        assert!(!d.remembers());
        assert_eq!(d.order(), [Part::Option(0), Part::Cancel, Part::Open]);
        let v = build_view(&d);
        assert!(v.scope.is_none());
        assert_eq!(v.subtitle, t!("file-kind-why-none"));
    }

    #[test]
    fn the_prompt_lists_the_likeliest_first_with_what_each_is() {
        let mut d = Dialog::new(Path::new("/p/scan.PIC"), demo_candidates(), None);
        assert_eq!(d.ext, "pic");
        assert!(d.remembers());
        let v = build_view(&d);
        assert_eq!(v.options.len(), 2);
        assert_eq!(v.options[0].name, "PICT");
        assert_eq!(
            v.options[0].tag,
            format!(
                "{} · {}",
                t!("file-kind-likely"),
                t!("file-kind-hint-same", ext = "pic")
            )
        );
        assert_eq!(v.options[1].tag, t!("file-kind-possible"));
        assert!(!v.options[0].about.is_empty());
        assert_eq!(v.selected, 0);
        d.step_scope(1);
        assert_eq!(d.scope, Scope::Folder);
        d.step_scope(-2);
        assert_eq!(d.scope, Scope::Everywhere);
        // A choice made before is selected when asked again (Open as…).
        let d = Dialog::new(
            Path::new("/p/scan.pic"),
            demo_candidates(),
            Some(Format::Hdr),
        );
        assert_eq!(d.selected, 1);
        assert_eq!(d.focus, Part::Option(1));
    }

    #[test]
    fn the_rules_panel_lists_each_rule_with_forget() {
        let p = panel(&[
            rule("pic", Some("/a"), Format::Pict),
            rule("img", None, Format::Hdr),
        ]);
        let ids: Vec<&str> = p.rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["rule:0", "rule:1"]);
        assert!(p.rows.iter().all(|r| r.button.is_some()));
        assert_eq!(panel(&[]).intro, t!("file-kind-rules-none"));
    }

    #[test]
    fn only_unrecognised_errors_offer_raw_samples() {
        assert!(unrecognised(
            &specular_decode::DecodeError::UnsupportedFormat.to_string()
        ));
        assert!(unrecognised("none of the .img formats read (VICAR)"));
        assert!(!unrecognised("truncated: eof"));
    }
}
