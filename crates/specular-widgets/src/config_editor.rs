//! The built-in config file editor (DESIGN.md, "Built-in config editor"): settings.toml or
//! keymap.toml ([`EditorFile`]).
//!
//! A full-window sheet over the viewer: the file as text with syntax colours from the theme
//! ([`toml_lex`]), line numbers, the caret's line highlighted, matching brackets and quotes,
//! undo/redo ([`Buffer`]), find, and problems found as you type by a pluggable [`Linter`]
//! (the loader's own checks: `specular_settings::lint` for settings.toml,
//! [`Keymap::check`] for keymap.toml). Problems are underlined in place, marked in the
//! gutter and listed in the problems pane.
//!
//! Saving is refused while errors remain; the editor says why and moves the keyboard to the
//! problems. A clean save writes the text exactly as typed (atomically) and applies it at
//! once: settings.toml through [`SettingsStore::write_text`], keymap.toml through the
//! application ([`Command::KeymapSave`], answered by [`Chrome::keymap_saved`]), which
//! writes and applies it as it does an edit made on the Keys page. A keymap.toml that does
//! not exist yet opens as the bindings in effect and is created on save. When the file or a
//! setting changes elsewhere while
//! there are unsaved edits, a bar offers Reload or Keep mine; without edits the editor just
//! follows. Nothing is ever overwritten or thrown away silently.
//!
//! Everything works from the keyboard: Tab indents (Shift+Tab outdents), F6 or Ctrl+Tab
//! moves between the text, the find field, the problems and the buttons, F8 goes to the next
//! problem, Esc goes back one step at a time. The code stays left to right in right-to-left
//! languages; the chrome around it mirrors.
//!
//! The text takes the pointer and the clipboard keys as the one-line fields do
//! ([`LineEdit`]): a double click selects a word, a triple click the line, Shift+click
//! extends the selection, and a drag after a double or triple click extends it by words or
//! lines; Ctrl+Insert copies, Shift+Insert pastes and Shift+Delete cuts.

use std::ops::Range;
use std::sync::Arc;

use specular_gfx::{Icon, Rect, TextBlock, TextStyle};
use specular_i18n::{I18n, t};
use specular_input::{Action, Key, KeyCombo, Keymap, NamedKey};
use specular_settings::{SettingsStore, Severity};
use specular_theme::{Color, metrics};

use crate::ctx::{Ctx, HAlign, Target};
use crate::line_edit::{self, Clicks, Drag, LineEdit, PrimaryOffer, TextClipboard, Unit};
use crate::text_buffer::{Buffer, EditKind};
use crate::toml_lex::{self, Tok};
use crate::{Button, Chrome, Command, Modal, Offer, Response, ViewerState, scrollbar};

/// Spaces a Tab inserts.
const INDENT: usize = 4;
/// Quiet time after an edit before the text is checked again (seconds).
const LINT_DELAY: f64 = 0.15;
/// Problem rows the pane shows at most before it scrolls.
const PROBLEM_ROWS: usize = 5;
/// How far bracket matching looks (lines).
const MATCH_LINES: usize = 400;
/// A drag past an edge of the text scrolls toward it at this many pixels a second for every
/// pixel the pointer is past it.
const AUTOSCROLL_RATE: f32 = 10.0;
/// The longest step one frame of that scrolling takes (seconds): a late frame doesn't jump.
const AUTOSCROLL_MAX_STEP: f64 = 0.1;

/// One problem a [`Linter`] found.
#[derive(Clone, Debug, PartialEq)]
pub struct Lint {
    /// Byte range in the text (may be empty: a point).
    pub span: Range<usize>,
    pub severity: Severity,
    /// Localised message.
    pub message: String,
}

/// Checks the text of a configuration file. Errors block saving; warnings and notes don't.
pub trait Linter {
    fn lint(&self, text: &str) -> Vec<Lint>;
}

/// settings.toml: the loader's parser and schema (`specular_settings::lint`).
pub struct SettingsLinter;

impl Linter for SettingsLinter {
    fn lint(&self, text: &str) -> Vec<Lint> {
        let i18n = I18n::global();
        specular_settings::lint(text)
            .into_iter()
            .map(|d| Lint {
                message: d.message(i18n),
                span: d.span,
                severity: d.severity,
            })
            .collect()
    }
}

/// keymap.toml: the keymap loader's own checks ([`Keymap::check`]), worded as on
/// Settings → Keys. Problems the loader works around by ignoring something, and keys bound
/// twice, are errors; a key that begins a sequence or that the system may take is a warning
/// (the Keys page binds those when asked to).
pub struct KeymapLinter;

impl Linter for KeymapLinter {
    fn lint(&self, text: &str) -> Vec<Lint> {
        let mut v: Vec<Lint> = Keymap::check(text)
            .into_iter()
            .map(|p| Lint {
                message: crate::keys_edit::problem_text(&p.error),
                severity: if p.error.is_warning() {
                    Severity::Warning
                } else {
                    Severity::Error
                },
                span: p.span.unwrap_or(0..0),
            })
            .collect();
        // In the order of the text, as the problems pane and F8 go.
        v.sort_by_key(|l| l.span.start);
        v
    }
}

/// The file the editor has open.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EditorFile {
    #[default]
    Settings,
    Keymap,
}

/// Why settings.toml was not written, as a reason that follows "Can't save settings.toml:":
/// the common I/O failures in the user's language, never the path again or "(os error N)".
fn write_reason(e: &specular_settings::SettingsError) -> String {
    use specular_settings::SettingsError as E;
    use std::io::ErrorKind as K;
    match e {
        E::Read { source, .. } | E::Write { source, .. } => match source.kind() {
            K::NotFound => t!("error-reason-not-found"),
            K::PermissionDenied => t!("error-reason-permission-write"),
            K::StorageFull | K::QuotaExceeded => t!("error-reason-disk-full"),
            K::ReadOnlyFilesystem => t!("error-reason-read-only"),
            _ => {
                let s = source.to_string();
                match s.rfind(" (os error ") {
                    Some(i) => s[..i].to_owned(),
                    None => s,
                }
            }
        },
        E::FileHasErrors { .. } => t!("error-reason-file-has-errors"),
        E::Import { reason, .. } | E::Watch { reason, .. } => reason.clone(),
    }
}

impl EditorFile {
    /// The dialog's title (and its accessible name).
    pub(crate) fn title(self) -> String {
        match self {
            EditorFile::Settings => t!("config-editor-title"),
            EditorFile::Keymap => t!("config-editor-keymap-title"),
        }
    }

    /// The accessible name of the text.
    fn field(self) -> String {
        match self {
            EditorFile::Settings => t!("config-editor-text"),
            EditorFile::Keymap => t!("config-editor-keymap-text"),
        }
    }

    fn saved(self) -> String {
        match self {
            EditorFile::Settings => t!("config-editor-saved"),
            EditorFile::Keymap => t!("config-editor-keymap-saved"),
        }
    }

    fn save_failed(self, reason: &str) -> String {
        let reason = reason.to_owned();
        match self {
            EditorFile::Settings => t!("config-editor-save-failed", reason = reason),
            EditorFile::Keymap => t!("config-editor-keymap-save-failed", reason = reason),
        }
    }

    fn conflict(self) -> String {
        match self {
            EditorFile::Settings => t!("config-editor-conflict"),
            EditorFile::Keymap => t!("config-editor-keymap-conflict"),
        }
    }

    fn discard_question(self) -> String {
        match self {
            EditorFile::Settings => t!("config-editor-discard-question"),
            EditorFile::Keymap => t!("config-editor-keymap-discard-question"),
        }
    }
}

/// keymap.toml as the application last read or wrote it ([`Chrome::set_keymap_file`]).
#[derive(Clone, Debug, Default)]
pub(crate) struct KeymapFile {
    /// Where it is (`None`: no settings folder, e.g. headless).
    pub path: Option<String>,
    /// Its text (`None`: it does not exist yet).
    pub text: Option<String>,
    /// Counts changes, for the editor to notice them.
    pub generation: u64,
}

/// Where the open file's text comes from, to follow changes made elsewhere.
#[derive(Clone, Copy)]
pub(crate) enum Source<'a> {
    Settings(&'a SettingsStore),
    /// The file, and the bindings in effect (the text of a file not created yet).
    Keymap(&'a KeymapFile, &'a Keymap),
}

impl Source<'_> {
    /// Generations that change when the text may have.
    fn gens(self) -> (u64, u64) {
        match self {
            Source::Settings(store) => (store.text_generation(), store.generation()),
            Source::Keymap(f, _) => (f.generation, 0),
        }
    }

    fn text(self) -> String {
        match self {
            Source::Settings(store) => store.file_text(),
            Source::Keymap(f, km) => f.text.clone().unwrap_or_else(|| km.to_toml()),
        }
    }
}

/// The source of `file`.
fn source<'a>(file: EditorFile, kf: &'a KeymapFile, st: &ViewerState<'a>) -> Source<'a> {
    match file {
        EditorFile::Settings => Source::Settings(st.store),
        EditorFile::Keymap => Source::Keymap(kf, st.keymap),
    }
}

/// What a save did.
#[derive(Debug, PartialEq)]
pub(crate) enum Saved {
    /// Written and applied.
    Done,
    /// Refused (errors, a conflict) or failed: see the editor's alert and conflict.
    Not,
    /// The application writes this text ([`Command::KeymapSave`]).
    Ask(String),
}

/// Something the pointer can hit in the editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditorTarget {
    /// The text (and its gutter).
    Text,
    /// A row of the problems pane.
    Problem(usize),
    Save,
    Revert,
    Reload,
    KeepMine,
    Discard,
    KeepEditing,
    FindField,
    FindPrev,
    FindNext,
    FindClose,
}

/// Where the keyboard is when no button has it (`Chrome::modal_focus`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Region {
    #[default]
    Text,
    Find,
    Problems,
}

/// What follows "Discard your changes?" when it is accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum After {
    /// Go back (to Settings, or close).
    Back,
    /// Quit the application (Ctrl+Q with unsaved edits).
    Quit,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Alert {
    /// "Can't save: N problems" (N is counted as drawn: it goes when they are fixed).
    CantSave,
    Failed(String),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Find {
    pub query: String,
    /// The current match (index into the matches).
    pub current: usize,
    /// The find field's caret, selection and undo (the truth is `query`).
    pub query_edit: LineEdit,
}

/// The Settings page the editor was opened from, to go back to as it was.
#[derive(Clone, Debug)]
pub(crate) struct SettingsPlace {
    section: usize,
    scroll: f32,
    focus: Option<Target>,
}

/// The visible lines as last drawn, for the pointer.
#[derive(Clone, Default)]
struct Geom {
    /// x of the start of every line.
    x0: f32,
    /// y of the top of line 0.
    y0: f32,
    line_h: f32,
    lines: Vec<(usize, Arc<TextBlock>)>,
    page: usize,
}

/// The editor's state (it lives in the chrome while the sheet is open).
pub(crate) struct Editor {
    pub buf: Buffer,
    /// Spans and end state of every line lexed so far (lines `0..lex.len()`).
    lex: Vec<(Vec<toml_lex::Span>, toml_lex::State)>,
    linter: Box<dyn Linter>,
    pub problems: Vec<Lint>,
    linted: Option<u64>,
    edited_at: f64,
    /// The file as last loaded or saved: the buffer is modified when it differs.
    pub base: String,
    /// The store's text and settings generations `base` belongs to.
    gens: (u64, u64),
    /// The file changed elsewhere while there were edits: its new text.
    pub conflict: Option<String>,
    /// The conflict bar is up (Esc hides it; saving brings it back).
    pub conflict_shown: bool,
    pub alert: Option<Alert>,
    pub saved_at: Option<f64>,
    /// "Discard your changes?" is up.
    pub discard: Option<After>,
    pub find: Option<Find>,
    pub region: Region,
    pub problem_sel: usize,
    pub problems_top: usize,
    pub scroll: f32,
    pub scroll_x: f32,
    geom: Geom,
    /// A press on the text being dragged (what it selected first, and by what it extends).
    pub drag: Option<Drag>,
    /// Where the pointer was last seen while dragging (kept when it leaves the window).
    drag_at: Option<(f32, f32)>,
    /// The drag's selection is to follow the pointer again at the next frame (the text
    /// scrolled under it).
    drag_rehit: bool,
    /// When the drag last scrolled the text past an edge (`None`: it isn't).
    autoscroll_at: Option<f64>,
    /// The last presses on the text, for double and triple clicks.
    clicks: Clicks,
    pub reveal: bool,
    /// Copy-on-select for the text (Linux and the BSDs).
    pub primary: PrimaryOffer,
    /// The problem the caret was last in (announced when it enters one).
    caret_problem: Option<(Range<usize>, String)>,
    pub return_to: Option<SettingsPlace>,
    pub path: String,
    pub file: EditorFile,
    /// The text handed to the application to save ([`Saved::Ask`]).
    saving: Option<String>,
}

impl Default for Editor {
    fn default() -> Self {
        Editor::new("", Box::new(SettingsLinter))
    }
}

fn severity_color(s: &specular_theme::Syntax, sev: Severity) -> Color {
    match sev {
        Severity::Error => s.error,
        Severity::Warning => s.warning,
        Severity::Info => s.info,
    }
}

fn tok_color(s: &specular_theme::Syntax, t: Tok) -> Color {
    match t {
        Tok::Text => s.text,
        Tok::Table => s.table,
        Tok::Key => s.key,
        Tok::String => s.string,
        Tok::Escape => s.escape,
        Tok::Number => s.number,
        Tok::Boolean => s.boolean,
        Tok::Datetime => s.datetime,
        Tok::Comment => s.comment,
        Tok::Punct => s.punctuation,
    }
}

/// The colour of the byte at `at`: plain text inside a selection or find match (`marks`),
/// else its token's colour.
fn marked_color(
    s: &specular_theme::Syntax,
    marks: &[std::ops::Range<usize>],
    at: usize,
    token: Color,
) -> Color {
    if marks.iter().any(|m| m.contains(&at)) {
        s.text
    } else {
        token
    }
}

impl Editor {
    pub(crate) fn new(text: &str, linter: Box<dyn Linter>) -> Editor {
        Editor {
            buf: Buffer::new(text),
            lex: Vec::new(),
            linter,
            problems: Vec::new(),
            linted: None,
            edited_at: 0.0,
            base: text.to_owned(),
            gens: (0, 0),
            conflict: None,
            conflict_shown: false,
            alert: None,
            saved_at: None,
            discard: None,
            find: None,
            region: Region::Text,
            problem_sel: 0,
            problems_top: 0,
            scroll: 0.0,
            scroll_x: 0.0,
            geom: Geom::default(),
            drag: None,
            drag_at: None,
            drag_rehit: false,
            autoscroll_at: None,
            clicks: Clicks::default(),
            reveal: false,
            primary: PrimaryOffer::default(),
            caret_problem: None,
            return_to: None,
            path: String::new(),
            file: EditorFile::Settings,
            saving: None,
        }
    }

    /// Copy-on-select for the text: a new selection made in it becomes the primary
    /// selection (`how`: now, after keys, or when due). Selections find makes are not offered
    /// ([`PrimaryOffer::pass_over`]), not even once Esc leaves the match selected in the
    /// text, until the user changes them; nothing is while "Discard your changes?" is up.
    pub(crate) fn offer_primary(&mut self, how: Offer, clip: &mut dyn TextClipboard) {
        if self.region != Region::Text || self.discard.is_some() {
            return;
        }
        let (sel, text) = (self.buf.selection(), self.buf.selected_text());
        let dragging = self.drag.is_some();
        match how {
            Offer::Now => {
                self.primary.offer(sel, text, dragging, clip);
            }
            Offer::Later(now) => self.primary.later(sel, text, dragging, now),
            Offer::IfDue(now) => {
                if self.primary.due(now) {
                    self.primary.offer(sel, text, dragging, clip);
                }
            }
        }
    }

    /// The word (or run of spaces, or of other characters) around byte `at`, within its line:
    /// what a double click selects ([`line_edit::word_at`], as in the text fields).
    fn word_at(&self, at: usize) -> Range<usize> {
        let r = self.buf.line_range(self.buf.line_of(at));
        let at = at.clamp(r.start, r.end);
        let w = line_edit::word_at(&self.buf.text()[r.clone()], at - r.start);
        r.start + w.start..r.start + w.end
    }

    /// The line holding byte `at` with its line break: what a triple click selects.
    fn line_at(&self, at: usize) -> Range<usize> {
        let l = self.buf.line_of(at);
        let end = if l + 1 < self.buf.line_count() {
            self.buf.line_range(l + 1).start
        } else {
            self.buf.text().len()
        };
        self.buf.line_range(l).start..end
    }

    /// The unit `u` around byte `at`.
    fn span(&self, u: Unit, at: usize) -> Range<usize> {
        match u {
            Unit::Char => at..at,
            Unit::Word => self.word_at(at),
            Unit::Line => self.line_at(at),
        }
    }

    /// A drag reached byte `at`: the selection grows from where it started to there, by its
    /// unit.
    fn drag_to(&mut self, at: usize) {
        if let Some(d) = &self.drag {
            let sel = d.to(at, |u, at| self.span(u, at));
            self.buf.select(sel);
        }
    }

    /// Line `l` as screen readers get it: from its start to the next line's, so with its
    /// line break.
    fn run_range(&self, l: usize) -> Range<usize> {
        let end = if l + 1 < self.buf.line_count() {
            self.buf.line_range(l + 1).start
        } else {
            self.buf.text().len()
        };
        self.buf.line_range(l).start..end
    }

    /// Byte `b` as a screen reader position: (line, character within the line). A line's
    /// start is on that line, not past the end of the one before.
    fn a11y_pos(&self, b: usize) -> (usize, usize) {
        let l = self.buf.line_of(b);
        let r = self.run_range(l);
        (
            l,
            self.buf.text()[r.start..b.clamp(r.start, r.end)]
                .chars()
                .count(),
        )
    }

    /// The byte of a screen reader position (a character past the line's end: its end, which
    /// is the next line's start). `None` past the last line.
    fn a11y_byte(&self, (l, i): (usize, usize)) -> Option<usize> {
        if l >= self.buf.line_count() {
            return None;
        }
        let r = self.run_range(l);
        let line = &self.buf.text()[r.clone()];
        Some(r.start + line.char_indices().nth(i).map_or(line.len(), |(b, _)| b))
    }

    /// Copies the selection ([`Command::EditorCopy`]); with `cut` it is deleted too (one undo
    /// step). Nothing without a selection.
    fn copy(&mut self, cut: bool, now: f64) -> Response {
        let s = self.buf.selected_text();
        if s.is_empty() {
            return Response::used();
        }
        let s = s.to_owned();
        if cut {
            self.buf.insert("", EditKind::Other);
            self.edited_at = now;
            self.reveal = true;
        }
        Response::cmd(Command::EditorCopy(s))
    }

    /// The text differs from the file.
    pub(crate) fn modified(&self) -> bool {
        self.buf.text() != self.base
    }

    /// Lexes lines up to `upto` (re-lexing from the first line changed since).
    fn lex_to(&mut self, upto: usize) {
        if let Some(d) = self.buf.take_dirty() {
            self.lex.truncate(d);
        }
        let upto = upto.min(self.buf.line_count() - 1);
        while self.lex.len() <= upto {
            let l = self.lex.len();
            let entry = self.lex.last().map_or_else(Default::default, |x| x.1);
            let r = toml_lex::lex_line(self.buf.line(l), entry);
            self.lex.push(r);
        }
    }

    /// The token at byte `at`.
    fn tok_at(&mut self, at: usize) -> Tok {
        let l = self.buf.line_of(at);
        self.lex_to(l);
        let start = self.buf.line_range(l).start;
        toml_lex::tok_at(&self.lex[l].0, at - start)
    }

    /// Keeps the problems' spans inside the text while it is edited (until it is checked
    /// again).
    fn fit_problems(&mut self) {
        if self.linted == Some(self.buf.version()) {
            return;
        }
        let text = self.buf.text();
        let fix = |at: usize| {
            let mut at = at.min(text.len());
            while !text.is_char_boundary(at) {
                at -= 1;
            }
            at
        };
        for p in &mut self.problems {
            p.span = fix(p.span.start)..fix(p.span.end).max(fix(p.span.start));
        }
    }

    /// Checks the text now if it changed since it was last checked.
    pub(crate) fn relint(&mut self) {
        let v = self.buf.version();
        if self.linted == Some(v) {
            return;
        }
        let text = self.buf.text();
        let len = text.len();
        let fix = |at: usize| {
            let mut at = at.min(len);
            while !text.is_char_boundary(at) {
                at -= 1;
            }
            at
        };
        let mut p = self.linter.lint(text);
        for l in &mut p {
            l.span = fix(l.span.start)..fix(l.span.end.max(l.span.start));
        }
        p.sort_by(|a, b| {
            a.span
                .start
                .cmp(&b.span.start)
                .then(a.severity.cmp(&b.severity))
        });
        self.problems = p;
        self.linted = Some(v);
        self.problem_sel = self.problem_sel.min(self.problems.len().saturating_sub(1));
    }

    pub(crate) fn errors(&self) -> usize {
        self.problems
            .iter()
            .filter(|p| p.severity == Severity::Error)
            .count()
    }

    /// The problem under byte `at`, the most severe first.
    fn problem_at(&self, at: usize) -> Option<usize> {
        self.problems
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                p.span.contains(&at)
                    || (p.span.is_empty() || p.span.end == at)
                        && p.span.start <= at
                        && at <= p.span.end
            })
            .min_by_key(|(_, p)| p.severity)
            .map(|(i, _)| i)
    }

    /// Find matches (ASCII case-insensitive).
    pub(crate) fn matches(&self) -> Vec<Range<usize>> {
        let Some(f) = &self.find else {
            return Vec::new();
        };
        if f.query.is_empty() {
            return Vec::new();
        }
        let hay = self.buf.text().to_ascii_lowercase();
        let q = f.query.to_ascii_lowercase();
        hay.match_indices(&q).map(|(i, _)| i..i + q.len()).collect()
    }

    /// The find field has the keyboard (unless a button of the editor has it): find is open,
    /// its region is current and no question is asked.
    pub(crate) fn find_field_on(&self) -> bool {
        self.region == Region::Find && self.find.is_some() && self.discard.is_none()
    }

    /// The find field while it has the keyboard, caught up with its query.
    pub(crate) fn find_edit(&mut self) -> Option<&mut LineEdit> {
        if !self.find_field_on() {
            return None;
        }
        let f = self.find.as_mut()?;
        f.query_edit.sync(&f.query);
        Some(&mut f.query_edit)
    }

    /// The find field while it has the keyboard, when its editing state is current.
    pub(crate) fn find_field(&self) -> Option<&LineEdit> {
        if !self.find_field_on() {
            return None;
        }
        let f = self.find.as_ref()?;
        f.query_edit.of(&f.query)
    }

    /// The find field edited: incremental search, the first match from where the search
    /// started (the start of the current match).
    pub(crate) fn find_edited(&mut self) {
        let Some(f) = &mut self.find else {
            return;
        };
        if f.query == f.query_edit.text() {
            return;
        }
        f.query = f.query_edit.text().to_owned();
        let start = self.buf.selection().start;
        self.buf.set_caret(start, false);
        self.find_step(false);
    }

    /// Selects the next (or previous) find match from the caret.
    pub(crate) fn find_step(&mut self, back: bool) {
        let m = self.matches();
        if m.is_empty() {
            return;
        }
        let sel = self.buf.selection();
        let i = if back {
            m.iter()
                .rposition(|r| r.start < sel.start)
                .unwrap_or(m.len() - 1)
        } else {
            m.iter()
                .position(|r| r.start > sel.start || (r.start == sel.start && sel.is_empty()))
                .unwrap_or(0)
        };
        self.buf.select(m[i].clone());
        // A match is find's selection, not the user's: not offered as the primary selection
        // (also once Esc leaves it selected in the text) until the user changes it.
        self.primary
            .pass_over(m[i].clone(), self.buf.selected_text());
        if let Some(f) = &mut self.find {
            f.current = i;
        }
        self.reveal = true;
    }

    /// The match the find bar counts as current: the selected one, else the first after the
    /// caret.
    fn find_current(&self, m: &[Range<usize>]) -> usize {
        let sel = self.buf.selection();
        m.iter()
            .position(|r| *r == sel)
            .or_else(|| m.iter().position(|r| r.start >= sel.start))
            .unwrap_or(0)
    }

    /// The pair of brackets (or the quotes of a string) at the caret.
    fn bracket_pair(&mut self) -> Option<(usize, usize)> {
        let c = self.buf.caret();
        let text_len = self.buf.text().len();
        let mut cands = vec![c];
        if c > 0 {
            let p = self.buf.text()[..c]
                .char_indices()
                .next_back()
                .map_or(0, |(i, _)| i);
            cands.push(p);
        }
        for at in cands {
            if at >= text_len {
                continue;
            }
            let ch = self.buf.text()[at..].chars().next()?;
            let tok = self.tok_at(at);
            match ch {
                // A table header's brackets (`[a]`, `[[a]]`): the ends of its span.
                '[' | ']' if tok == Tok::Table => {
                    let l = self.buf.line_of(at);
                    self.lex_to(l);
                    let start = self.buf.line_range(l).start;
                    let span = self.lex[l]
                        .0
                        .iter()
                        .find(|s| (s.start as usize..s.end as usize).contains(&(at - start)))
                        .copied()?;
                    let (s, e) = (start + span.start as usize, start + span.end as usize);
                    let t = self.buf.text().as_bytes();
                    let double = e - s >= 4 && t[s + 1] == b'[' && t[e - 2] == b']';
                    for (a, b) in [(s, e - 1), (s + 1, e - 2)]
                        .into_iter()
                        .take(1 + double as usize)
                    {
                        if (at == a || at == b) && t[a] == b'[' && t[b] == b']' {
                            return Some((at, if at == a { b } else { a }));
                        }
                    }
                }
                '[' | '{' | ']' | '}' if tok == Tok::Punct => {
                    let (open, close, fwd) = match ch {
                        '[' => ('[', ']', true),
                        '{' => ('{', '}', true),
                        ']' => ('[', ']', false),
                        _ => ('{', '}', false),
                    };
                    return self.scan_bracket(at, open, close, fwd).map(|o| (at, o));
                }
                '"' | '\'' if tok == Tok::String => {
                    let l = self.buf.line_of(at);
                    self.lex_to(l);
                    let start = self.buf.line_range(l).start;
                    let span = self.lex[l]
                        .0
                        .iter()
                        .find(|s| (s.start as usize..s.end as usize).contains(&(at - start)))
                        .copied()?;
                    let (s, e) = (start + span.start as usize, start + span.end as usize);
                    let line = self.buf.text();
                    if e > s + 1 && line[s..].starts_with(ch) && line[..e].ends_with(ch) {
                        return Some((s, e - 1));
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn scan_bracket(&mut self, at: usize, open: char, close: char, fwd: bool) -> Option<usize> {
        let l0 = self.buf.line_of(at);
        let n = self.buf.line_count();
        let lines: Vec<usize> = if fwd {
            (l0..n.min(l0 + MATCH_LINES)).collect()
        } else {
            (l0.saturating_sub(MATCH_LINES)..=l0).rev().collect()
        };
        if let Some(&last) = lines.iter().max() {
            self.lex_to(last);
        }
        let mut depth = 0i32;
        for l in lines {
            let r = self.buf.line_range(l);
            let line = &self.buf.text()[r.clone()];
            let spans = &self.lex[l].0;
            let mut chars: Vec<(usize, char)> = line.char_indices().collect();
            if !fwd {
                chars.reverse();
            }
            for (i, c) in chars {
                let abs = r.start + i;
                if (fwd && abs < at) || (!fwd && abs > at) {
                    continue;
                }
                if (c != open && c != close) || toml_lex::tok_at(spans, i) != Tok::Punct {
                    continue;
                }
                let d = if c == open { 1 } else { -1 };
                depth += if fwd { d } else { -d };
                if depth == 0 {
                    return Some(abs);
                }
            }
        }
        None
    }

    /// Notices changes made elsewhere: without edits the editor follows them (the caret
    /// stays on its line); with edits it asks (the conflict bar).
    pub(crate) fn poll(&mut self, src: Source) {
        let gens = src.gens();
        if gens == self.gens {
            return;
        }
        self.gens = gens;
        let theirs = src.text();
        if theirs == self.base {
            return;
        }
        if !self.modified() {
            self.buf.replace_all(&theirs);
            self.base = theirs;
            self.conflict = None;
        } else {
            self.conflict = Some(theirs);
            self.conflict_shown = true;
        }
    }

    /// Reload: their text replaces the buffer (undoable).
    pub(crate) fn reload(&mut self) {
        if let Some(theirs) = self.conflict.take() {
            self.buf.replace_all(&theirs);
            self.base = theirs;
        }
        self.conflict_shown = false;
        self.reveal = true;
    }

    /// Keep mine: their text becomes what the buffer is compared with; saving then replaces
    /// it on purpose.
    pub(crate) fn keep_mine(&mut self) {
        if let Some(theirs) = self.conflict.take() {
            self.base = theirs;
        }
        self.conflict_shown = false;
    }

    /// Saves, unless the text has errors (then says so and moves to the problems) or the
    /// file changed elsewhere (then asks first). keymap.toml is handed to the application.
    pub(crate) fn save(&mut self, src: Source, now: f64) -> Saved {
        self.poll(src);
        if self.conflict.is_some() {
            self.conflict_shown = true;
            return Saved::Not;
        }
        self.relint();
        if self.errors() > 0 {
            self.alert = Some(Alert::CantSave);
            self.region = Region::Problems;
            self.problem_sel = self
                .problems
                .iter()
                .position(|p| p.severity == Severity::Error)
                .unwrap_or(0);
            return Saved::Not;
        }
        let text = self.buf.text().to_owned();
        let store = match src {
            Source::Settings(store) => store,
            Source::Keymap(..) => {
                self.saving = Some(text.clone());
                return Saved::Ask(text);
            }
        };
        match store.write_text(&text) {
            Ok(_) => {
                self.saved(text, src.gens(), now);
                Saved::Done
            }
            Err(e) => {
                self.alert = Some(Alert::Failed(write_reason(&e)));
                Saved::Not
            }
        }
    }

    /// `text` is the file now (at generations `gens`).
    fn saved(&mut self, text: String, gens: (u64, u64), now: f64) {
        self.base = text;
        self.gens = gens;
        self.alert = None;
        self.saved_at = Some(now);
    }

    /// Moves the caret to problem `i` (selecting its span) and the keyboard to the text.
    pub(crate) fn jump_to(&mut self, i: usize) {
        let Some(p) = self.problems.get(i) else {
            return;
        };
        self.problem_sel = i;
        let span = p.span.clone();
        self.buf.select(span.end..span.start);
        self.region = Region::Text;
        self.reveal = true;
    }

    /// F8: the next problem after the caret (Shift: before), wrapping round.
    pub(crate) fn next_problem(&mut self, back: bool) {
        self.relint();
        let n = self.problems.len();
        if n == 0 {
            return;
        }
        let c = self.buf.selection().start;
        let i = if back {
            self.problems
                .iter()
                .rposition(|p| p.span.start < c)
                .unwrap_or(n - 1)
        } else {
            self.problems
                .iter()
                .position(|p| p.span.start > c)
                .unwrap_or(0)
        };
        self.jump_to(i);
    }

    /// Enter: a new line indented like this one (one more after an opening bracket).
    fn newline(&mut self) {
        let (l, _) = self.buf.caret_line_col();
        let mut indent = self.buf.indent_of(l).to_owned();
        let before =
            self.buf.text()[self.buf.line_range(l).start..self.buf.selection().start].trim_end();
        if before.ends_with(['[', '{']) {
            indent.push_str(&" ".repeat(INDENT));
        }
        self.buf.insert(&format!("\n{indent}"), EditKind::Other);
    }

    /// Tab: indents the selected lines, or inserts spaces to the next tab stop.
    fn tab(&mut self, back: bool) {
        let lines = self.buf.selected_lines();
        if back {
            self.buf.outdent_lines(INDENT);
        } else if self.buf.has_selection() && lines.len() > 1 {
            self.buf.indent_lines(&" ".repeat(INDENT));
        } else {
            let (l, _) = self.buf.caret_line_col();
            let start = self.buf.line_range(l).start;
            let col = self.buf.text()[start..self.buf.selection().start]
                .chars()
                .count();
            let n = INDENT - col % INDENT;
            self.buf.insert(&" ".repeat(n), EditKind::Other);
        }
    }
}

/// Lines of `page` height.
fn page_lines(g: &Geom) -> isize {
    g.page.max(1) as isize
}

impl Chrome {
    /// Opens the settings.toml editor on `store`'s file. From Settings, Esc comes back to the
    /// page it was opened from.
    pub fn open_config_editor(&mut self, store: &SettingsStore) {
        self.open_config_editor_with(store, None);
    }

    /// The editor with `text` in place of the file's (as if typed: screenshots, tests).
    #[doc(hidden)]
    pub fn open_config_editor_with(&mut self, store: &SettingsStore, text: Option<&str>) {
        let path = store
            .path()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        self.open_editor(
            EditorFile::Settings,
            Source::Settings(store),
            path,
            Box::new(SettingsLinter),
            text,
        );
    }

    /// Opens the keymap.toml editor (`keymap`: the bindings in effect, shown when the file
    /// does not exist yet). From Settings → Keys, Esc comes back there.
    pub fn open_keymap_editor(&mut self, keymap: &Keymap) {
        self.open_keymap_editor_with(keymap, None);
    }

    /// The keymap editor with `text` in place of the file's (screenshots, tests).
    #[doc(hidden)]
    pub fn open_keymap_editor_with(&mut self, keymap: &Keymap, text: Option<&str>) {
        let kf = std::mem::take(&mut self.keymap_file);
        let path = kf.path.clone().unwrap_or_default();
        self.open_editor(
            EditorFile::Keymap,
            Source::Keymap(&kf, keymap),
            path,
            Box::new(KeymapLinter),
            text,
        );
        self.keymap_file = kf;
    }

    fn open_editor(
        &mut self,
        file: EditorFile,
        src: Source,
        path: String,
        linter: Box<dyn Linter>,
        text: Option<&str>,
    ) {
        let from = (self.modal == Some(Modal::Settings)).then(|| SettingsPlace {
            section: self.settings.section,
            scroll: self.settings.scroll,
            focus: self.modal_focus.clone(),
        });
        let mut ed = Editor::new(&src.text(), linter);
        ed.gens = src.gens();
        if let Some(text) = text {
            ed.buf = Buffer::new(text);
        }
        ed.file = file;
        ed.path = path;
        ed.return_to = from;
        ed.relint();
        *self.editor = ed;
        self.modal = None;
        self.open_modal(Modal::ConfigEditor);
    }

    /// keymap.toml as the application read or wrote it (`path`: `None` without a settings
    /// folder; `text`: `None` while it does not exist). The keymap editor follows it.
    pub fn set_keymap_file(&mut self, path: Option<&std::path::Path>, text: Option<&str>) {
        let path = path.map(|p| p.display().to_string());
        let kf = &mut self.keymap_file;
        if kf.path != path || kf.text.as_deref() != text {
            kf.path = path;
            kf.text = text.map(str::to_owned);
            kf.generation += 1;
        }
    }

    /// The application's answer to [`Command::KeymapSave`]: the text was written and applied
    /// (after [`set_keymap_file`](Self::set_keymap_file)), or why not.
    pub fn keymap_saved(&mut self, result: Result<(), String>) {
        if !self.config_editor_open() || self.editor.file != EditorFile::Keymap {
            return;
        }
        let Some(text) = self.editor.saving.take() else {
            return;
        };
        let now = self.last_now;
        let msg = match result {
            Ok(()) => {
                let gens = (self.keymap_file.generation, 0);
                self.editor.saved(text, gens, now);
                EditorFile::Keymap.saved()
            }
            Err(reason) => {
                let msg = EditorFile::Keymap.save_failed(&reason);
                self.editor.alert = Some(Alert::Failed(reason));
                msg
            }
        };
        self.announce(msg);
    }

    /// Whether the config editor is open (either file).
    pub fn config_editor_open(&self) -> bool {
        self.modal == Some(Modal::ConfigEditor)
    }

    /// The file the open editor has (tests).
    #[doc(hidden)]
    pub fn config_editor_file(&self) -> Option<EditorFile> {
        self.config_editor_open().then_some(self.editor.file)
    }

    /// The text of the open editor (tests).
    #[doc(hidden)]
    pub fn config_editor_text(&self) -> Option<&str> {
        self.config_editor_open().then(|| self.editor.buf.text())
    }

    /// Text from the clipboard, for [`Command::EditorPaste`]: typed into the editor (or its
    /// find field) at the caret.
    pub fn paste_text(&mut self, s: &str) {
        if self.field_paste(s) {
            return;
        }
        if !self.config_editor_open() || self.editor.discard.is_some() {
            return;
        }
        let s = s.replace("\r\n", "\n");
        let ed = &mut self.editor;
        // (The find field, when open, took it above.)
        if ed.region != Region::Find {
            ed.region = Region::Text;
            ed.buf.insert(&s, EditKind::Other);
            ed.edited_at = self.last_now;
            ed.reveal = true;
        }
        self.modal_focus = None;
    }

    /// Goes back from the editor one step: to the Settings page it was opened from (same
    /// section, scroll and focused control), or out of the dialog. Edits are not checked
    /// here (see [`editor_back`](Self::editor_back)).
    pub(crate) fn editor_leave(&mut self) {
        let place = self.editor.return_to.take();
        *self.editor = Editor::default();
        self.close_modal();
        if let Some(p) = place {
            self.modal = Some(Modal::Settings);
            self.settings.section = p.section;
            self.settings.scroll = p.scroll;
            // As it was left: no scrolling to the focused control.
            self.modal_focus = p.focus;
        }
    }

    /// Esc in the editor: dismisses the innermost thing first (the discard question, the
    /// find bar, the conflict bar, the alert), then goes back, asking first when there are
    /// unsaved edits.
    pub(crate) fn editor_back(&mut self) {
        let ed = &mut self.editor;
        if ed.discard.is_some() {
            ed.discard = None;
            self.modal_focus = None;
        } else if ed.find.is_some() {
            ed.find = None;
            ed.region = Region::Text;
            self.modal_focus = None;
        } else if ed.conflict.is_some() && ed.conflict_shown {
            ed.conflict_shown = false;
        } else if ed.alert.is_some() {
            ed.alert = None;
        } else if self.modal_focus.is_some() || ed.region != Region::Text {
            // From a button or the problems, back to the text first.
            self.modal_focus = None;
            ed.region = Region::Text;
        } else if ed.modified() {
            self.editor_ask_discard(After::Back);
        } else {
            self.editor_leave();
        }
    }

    /// The Close button: goes back (asking first when there are unsaved edits).
    pub(crate) fn editor_close(&mut self) {
        if self.editor.modified() {
            self.editor_ask_discard(After::Back);
        } else {
            self.editor_leave();
        }
    }

    fn editor_ask_discard(&mut self, then: After) {
        self.editor.discard = Some(then);
        self.modal_focus = Some(Target::Editor(EditorTarget::KeepEditing));
        self.announce(self.editor.file.discard_question());
    }

    /// Ctrl+Q while the editor has unsaved edits asks first. `true`: handled.
    pub(crate) fn editor_quit(&mut self) -> bool {
        if self.modal != Some(Modal::ConfigEditor) || !self.editor.modified() {
            return false;
        }
        self.editor_ask_discard(After::Quit);
        true
    }

    pub(crate) fn editor_click(&mut self, e: EditorTarget, st: &ViewerState) -> Response {
        let now = self.last_now;
        match e {
            EditorTarget::Text => {}
            EditorTarget::Problem(i) => {
                self.editor.jump_to(i);
                self.modal_focus = None;
            }
            EditorTarget::Save => return self.editor_save(st, now),
            EditorTarget::Revert => {
                let base = self.editor.base.clone();
                self.editor.buf.replace_all(&base);
                self.editor.alert = None;
                self.editor.reveal = true;
            }
            EditorTarget::Reload => {
                self.editor.reload();
                self.modal_focus = None;
            }
            EditorTarget::KeepMine => {
                self.editor.keep_mine();
                self.modal_focus = None;
            }
            EditorTarget::KeepEditing => {
                self.editor.discard = None;
                self.modal_focus = None;
            }
            EditorTarget::Discard => {
                let then = self.editor.discard.take();
                self.editor_leave();
                if then == Some(After::Quit) {
                    return Response::cmd(Command::Action(Action::Quit));
                }
            }
            EditorTarget::FindField => {
                self.editor.region = Region::Find;
                self.modal_focus = None;
            }
            EditorTarget::FindPrev => self.editor.find_step(true),
            EditorTarget::FindNext => self.editor.find_step(false),
            EditorTarget::FindClose => {
                self.editor.find = None;
                self.editor.region = Region::Text;
                self.modal_focus = None;
            }
        }
        Response::used()
    }

    fn editor_save(&mut self, st: &ViewerState, now: f64) -> Response {
        let file = self.editor.file;
        let src = source(file, &self.keymap_file, st);
        match self.editor.save(src, now) {
            Saved::Done => self.announce(file.saved()),
            Saved::Ask(text) => return Response::cmd(Command::KeymapSave(text)),
            Saved::Not if self.editor.conflict.is_some() => {
                self.modal_focus = Some(Target::Editor(EditorTarget::Reload));
                self.announce(file.conflict());
            }
            Saved::Not => {
                if let Some(a) = &self.editor.alert {
                    let msg = match a {
                        Alert::CantSave => t!(
                            "config-editor-cant-save",
                            count = self.editor.errors() as i64
                        ),
                        Alert::Failed(r) => file.save_failed(r),
                    };
                    if matches!(a, Alert::CantSave) {
                        self.modal_focus = None;
                    }
                    self.announce(msg);
                }
            }
        }
        Response::used()
    }

    /// Follows changes made elsewhere to the open file.
    fn editor_poll(&mut self, st: &ViewerState) {
        let src = source(self.editor.file, &self.keymap_file, st);
        self.editor.poll(src);
    }

    /// Typed text.
    pub(crate) fn editor_text(&mut self, s: &str) {
        let ed = &mut self.editor;
        if ed.discard.is_some() {
            return;
        }
        match ed.region {
            Region::Find => {
                // Incremental: the first match from where the search started.
                if let Some(e) = ed.find_edit() {
                    e.insert(s);
                }
                ed.find_edited();
            }
            Region::Problems => {}
            Region::Text => {
                ed.buf.insert(s, EditKind::Type);
                ed.edited_at = self.last_now;
                ed.reveal = true;
            }
        }
    }

    /// The Tab order of F6 / Ctrl+Tab: the text, the find field, the problems, the buttons.
    pub(crate) fn editor_stops(&self) -> Vec<Option<Target>> {
        let ed = &self.editor;
        let mut v = vec![None];
        if ed.find.is_some() {
            v.push(Some(Target::Editor(EditorTarget::FindField)));
        }
        if !ed.problems.is_empty() {
            v.push(Some(Target::Editor(EditorTarget::Problem(0))));
        }
        if ed.conflict.is_some() && ed.conflict_shown {
            v.push(Some(Target::Editor(EditorTarget::Reload)));
            v.push(Some(Target::Editor(EditorTarget::KeepMine)));
        }
        v.push(Some(Target::Editor(EditorTarget::Revert)));
        v.push(Some(Target::Editor(EditorTarget::Save)));
        v.push(Some(Target::ModalClose));
        v
    }

    fn editor_cycle(&mut self, back: bool) {
        let stops = self.editor_stops();
        let ed = &self.editor;
        let here = match (&self.modal_focus, ed.region) {
            (Some(t), _) => Some(t.clone()),
            (None, Region::Text) => None,
            (None, Region::Find) => Some(Target::Editor(EditorTarget::FindField)),
            (None, Region::Problems) => Some(Target::Editor(EditorTarget::Problem(0))),
        };
        let n = stops.len();
        let at = stops.iter().position(|s| *s == here).unwrap_or(0);
        let to = if back { (at + n - 1) % n } else { (at + 1) % n };
        let ed = &mut self.editor;
        self.modal_focus = None;
        match &stops[to] {
            None => ed.region = Region::Text,
            Some(Target::Editor(EditorTarget::FindField)) => ed.region = Region::Find,
            Some(Target::Editor(EditorTarget::Problem(_))) => ed.region = Region::Problems,
            Some(t) => self.modal_focus = Some(t.clone()),
        }
        self.reveal_focus = true;
    }

    /// A key while the editor is open (Esc has gone to `back` already).
    pub(crate) fn editor_key(&mut self, combo: &KeyCombo, st: &ViewerState) -> Response {
        let m = combo.mods;
        let ctrl = m.ctrl || m.meta;
        let named = match combo.key {
            Key::Named(n) => Some(n),
            Key::Char(_) => None,
        };
        let ch = match combo.key {
            Key::Char(c) => Some(c.to_ascii_lowercase()),
            Key::Named(_) => None,
        };
        self.editor_poll(st);
        // "Discard your changes?": its two buttons.
        if self.editor.discard.is_some() {
            let pair = [
                Target::Editor(EditorTarget::KeepEditing),
                Target::Editor(EditorTarget::Discard),
            ];
            match named {
                Some(
                    NamedKey::Tab | NamedKey::ArrowLeft | NamedKey::ArrowRight | NamedKey::F(6),
                ) => {
                    self.modal_focus = Some(if self.modal_focus.as_ref() == Some(&pair[0]) {
                        pair[1].clone()
                    } else {
                        pair[0].clone()
                    });
                }
                Some(NamedKey::Enter | NamedKey::Space) => {
                    let t = self.modal_focus.clone().unwrap_or(pair[0].clone());
                    return self.activate(t, st);
                }
                _ => {}
            }
            return Response::used();
        }
        // Moving between the parts of the editor.
        if named == Some(NamedKey::F(6)) && !ctrl && !m.alt
            || named == Some(NamedKey::Tab) && ctrl
            || named == Some(NamedKey::Tab)
                && (self.modal_focus.is_some() || self.editor.region != Region::Text)
        {
            self.editor_cycle(m.shift);
            return Response::used();
        }
        // Everywhere in the editor.
        if ctrl && !m.alt {
            match ch {
                Some('s') => return self.editor_save(st, self.last_now),
                Some('f') => {
                    let ed = &mut self.editor;
                    let sel = ed.buf.selected_text();
                    let seed = (!sel.is_empty() && !sel.contains('\n')).then(|| sel.to_owned());
                    let f = ed.find.get_or_insert_with(Default::default);
                    if let Some(s) = seed {
                        f.query = s;
                    }
                    // The query selected: what is typed replaces it.
                    f.query_edit.sync(&f.query);
                    f.query_edit.select_all();
                    ed.region = Region::Find;
                    self.modal_focus = None;
                    return Response::used();
                }
                Some('g') => {
                    self.editor.find_step(m.shift);
                    return Response::used();
                }
                _ => {}
            }
        }
        match named {
            Some(NamedKey::F(3)) if !ctrl => {
                self.editor.find_step(m.shift);
                return Response::used();
            }
            Some(NamedKey::F(8)) if !ctrl => {
                self.editor.next_problem(m.shift);
                self.modal_focus = None;
                return Response::used();
            }
            _ => {}
        }
        // A button with the keyboard.
        if let Some(t) = self.modal_focus.clone() {
            if matches!(named, Some(NamedKey::Enter | NamedKey::Space)) {
                return self.activate(t, st);
            }
            return Response::used();
        }
        match self.editor.region {
            Region::Find => {
                // Up, Down and Enter step through the matches; the rest is the field's
                // (`Chrome::field_key`).
                let step = matches!(
                    named,
                    Some(NamedKey::Enter | NamedKey::ArrowDown | NamedKey::ArrowUp)
                );
                if !step && let Some(r) = self.field_key(combo) {
                    return r;
                }
                let ed = &mut self.editor;
                match named {
                    Some(NamedKey::Enter | NamedKey::ArrowDown) => ed.find_step(m.shift),
                    Some(NamedKey::ArrowUp) => ed.find_step(true),
                    _ => {}
                }
                Response::used()
            }
            Region::Problems => {
                let ed = &mut self.editor;
                let n = ed.problems.len();
                if n == 0 {
                    ed.region = Region::Text;
                    return Response::used();
                }
                let s = ed.problem_sel.min(n - 1);
                ed.problem_sel = match named {
                    Some(NamedKey::ArrowDown) => (s + 1).min(n - 1),
                    Some(NamedKey::ArrowUp) => s.saturating_sub(1),
                    Some(NamedKey::Home) => 0,
                    Some(NamedKey::End) => n - 1,
                    Some(NamedKey::PageDown) => (s + PROBLEM_ROWS).min(n - 1),
                    Some(NamedKey::PageUp) => s.saturating_sub(PROBLEM_ROWS),
                    Some(NamedKey::Enter | NamedKey::Space) => {
                        ed.jump_to(s);
                        return Response::used();
                    }
                    _ => s,
                };
                if ed.problem_sel != s
                    && let Some(p) = ed.problems.get(ed.problem_sel)
                {
                    let line = ed.buf.line_of(p.span.start) + 1;
                    let msg = t!(
                        "config-editor-problem-at",
                        line = line as i64,
                        message = p.message.clone()
                    );
                    self.announce(msg);
                }
                Response::used()
            }
            Region::Text => self.editor_text_key(combo, named, ch),
        }
    }

    fn editor_text_key(
        &mut self,
        combo: &KeyCombo,
        named: Option<NamedKey>,
        ch: Option<char>,
    ) -> Response {
        let m = combo.mods;
        let ctrl = m.ctrl || m.meta;
        let shift = m.shift;
        let now = self.last_now;
        let ed = &mut self.editor;
        let page = page_lines(&ed.geom);
        let mut edited = false;
        match named {
            Some(NamedKey::ArrowLeft) if ctrl => ed.buf.word_left(shift),
            Some(NamedKey::ArrowRight) if ctrl => ed.buf.word_right(shift),
            Some(NamedKey::ArrowLeft) => ed.buf.left(shift),
            Some(NamedKey::ArrowRight) => ed.buf.right(shift),
            Some(NamedKey::ArrowUp) if ctrl => {
                ed.scroll -= ed.geom.line_h;
                return Response::used();
            }
            Some(NamedKey::ArrowDown) if ctrl => {
                ed.scroll += ed.geom.line_h;
                return Response::used();
            }
            Some(NamedKey::ArrowUp) => ed.buf.vertical(-1, shift),
            Some(NamedKey::ArrowDown) => ed.buf.vertical(1, shift),
            Some(NamedKey::PageUp) => {
                ed.buf.vertical(-page, shift);
                ed.scroll -= page as f32 * ed.geom.line_h;
            }
            Some(NamedKey::PageDown) => {
                ed.buf.vertical(page, shift);
                ed.scroll += page as f32 * ed.geom.line_h;
            }
            Some(NamedKey::Home) if ctrl => ed.buf.doc_start(shift),
            Some(NamedKey::End) if ctrl => ed.buf.doc_end(shift),
            Some(NamedKey::Home) => ed.buf.home(shift),
            Some(NamedKey::End) => ed.buf.end(shift),
            Some(NamedKey::Backspace) => {
                ed.buf.backspace(ctrl);
                edited = true;
            }
            // Shift+Delete cuts (without a selection: the character after the caret).
            Some(NamedKey::Delete) if shift && !ctrl && ed.buf.has_selection() => {
                return ed.copy(true, now);
            }
            Some(NamedKey::Delete) => {
                ed.buf.delete(ctrl);
                edited = true;
            }
            // Ctrl+Insert copies, Shift+Insert pastes.
            Some(NamedKey::Insert) if ctrl && !shift && !m.alt => return ed.copy(false, now),
            Some(NamedKey::Insert) if shift && !ctrl && !m.alt => {
                return Response::cmd(Command::EditorPaste);
            }
            Some(NamedKey::Enter) if !ctrl => {
                ed.newline();
                edited = true;
            }
            Some(NamedKey::Tab) if !ctrl => {
                ed.tab(shift);
                edited = true;
            }
            _ if ctrl && !m.alt => match ch {
                Some('a') => ed.buf.select_all(),
                Some('c') => return ed.copy(false, now),
                Some('x') => return ed.copy(true, now),
                Some('v') => return Response::cmd(Command::EditorPaste),
                Some('z') if shift => edited = ed.buf.redo(),
                Some('z') => edited = ed.buf.undo(),
                Some('y') => edited = ed.buf.redo(),
                _ => return Response::used(),
            },
            _ => return Response::used(),
        }
        if edited {
            ed.edited_at = now;
        }
        ed.reveal = true;
        Response::used()
    }

    /// The byte under `p` (clamped to the lines last drawn).
    fn editor_byte_at(&self, p: (f32, f32)) -> Option<usize> {
        let g = &self.editor.geom;
        let (first, last) = (g.lines.first()?, g.lines.last()?);
        let l = ((p.1 - g.y0) / g.line_h).floor().max(0.0) as usize;
        let (l, block) = if l < first.0 {
            (first.0, &first.1)
        } else if l > last.0 {
            (last.0, &last.1)
        } else {
            let (l, b) = g.lines.iter().find(|(n, _)| *n == l)?;
            (*l, b)
        };
        let r = self.editor.buf.line_range(l);
        Some(r.start + block.hit_byte(p.0 - g.x0).min(r.len()))
    }

    /// The primary button went down on the text at `p` (`now` in seconds): one press puts
    /// the caret (Shift: extends the selection to it), a second in the same place soon after
    /// selects the word, a third the line (with its line break). A drag
    /// ([`editor_drag`](Self::editor_drag)) extends it by the same unit.
    pub(crate) fn editor_press(&mut self, p: (f32, f32), now: f64) {
        if self.editor.discard.is_some() {
            return;
        }
        let shift = self.mods.shift;
        if let Some(at) = self.editor_byte_at(p) {
            let ed = &mut self.editor;
            let n = ed.clicks.press(p, now, shift);
            let unit = Unit::of_clicks(n);
            let origin = if n == 1 && shift {
                ed.buf.set_caret(at, true);
                ed.buf.anchor()..ed.buf.anchor()
            } else {
                let origin = ed.span(unit, at);
                ed.buf.select(origin.clone());
                origin
            };
            ed.drag = Some(Drag { origin, unit });
            ed.drag_at = Some(p);
            ed.region = Region::Text;
            ed.reveal = true;
        }
    }

    /// A middle click at `p` on the text (where there is a primary selection): the caret goes
    /// there, the selection let go, and the primary selection is pasted at it
    /// ([`Command::PrimaryPaste`], which comes back through [`Chrome::paste_text`] as one undo
    /// step). `None` when it is not on the text.
    pub(crate) fn editor_middle(
        &mut self,
        p: (f32, f32),
        button: Button,
        hit: Option<&Target>,
    ) -> Option<Response> {
        if !self.primary_selection
            || button != Button::Middle
            || !self.config_editor_open()
            || self.editor.discard.is_some()
            || hit != Some(&Target::Editor(EditorTarget::Text))
        {
            return None;
        }
        let at = self.editor_byte_at(p)?;
        let ed = &mut self.editor;
        ed.buf.set_caret(at, false);
        ed.region = Region::Text;
        ed.drag = None;
        ed.clicks.clear();
        ed.reveal = true;
        self.modal_focus = None;
        Some(Response::cmd(Command::PrimaryPaste))
    }

    /// The pointer moved: extends a drag selection. `true` while dragging. Past an edge of
    /// the text it selects to the line at that edge, and the next frames scroll toward it
    /// ([`draw`]).
    pub(crate) fn editor_drag(&mut self, p: (f32, f32)) -> bool {
        if self.modal != Some(Modal::ConfigEditor) || self.editor.drag.is_none() {
            return false;
        }
        self.editor.drag_at = Some(p);
        if let Some(at) = self.editor_byte_at(p) {
            let ed = &mut self.editor;
            ed.drag_to(at);
            ed.reveal = true;
        }
        true
    }

    /// The wheel scrolled the text: a drag goes on selecting to what is now under the
    /// pointer (at the next frame, once the lines there are laid out).
    pub(crate) fn editor_wheeled(&mut self) {
        let ed = &mut self.editor;
        if ed.drag.is_some() {
            ed.drag_rehit = true;
        }
    }

    /// The node the screen reader gets for the editor's field: role, value, name, and
    /// whether it is the text (which goes line by line, [`editor_a11y_lines`]) rather than
    /// the find field.
    ///
    /// [`editor_a11y_lines`]: Self::editor_a11y_lines
    pub(crate) fn editor_a11y_field(&self) -> (accesskit::Role, String, String, bool) {
        let ed = &self.editor;
        if ed.region == Region::Find {
            let q = ed
                .find
                .as_ref()
                .map(|f| f.query.clone())
                .unwrap_or_default();
            return (
                accesskit::Role::SearchInput,
                q,
                t!("config-editor-find-placeholder"),
                false,
            );
        }
        (
            accesskit::Role::MultilineTextInput,
            ed.buf.text().to_owned(),
            ed.file.field(),
            true,
        )
    }

    /// The text's lines for the screen reader, each with its line break, and where it is
    /// drawn when it is.
    pub(crate) fn editor_a11y_lines(&self) -> Vec<(&str, Option<Rect>)> {
        let ed = &self.editor;
        let g = &ed.geom;
        let mut drawn = g.lines.iter().peekable();
        (0..ed.buf.line_count())
            .map(|l| {
                let at = drawn.next_if(|(n, _)| *n == l).map(|(_, b)| {
                    let y = g.y0 + l as f32 * g.line_h;
                    Rect::new(g.x0, y, b.width.max(1.0), g.line_h)
                });
                (&ed.buf.text()[ed.run_range(l)], at)
            })
            .collect()
    }

    /// The text's selection for the screen reader: anchor and caret as (line, character
    /// within the line).
    pub(crate) fn editor_a11y_selection(&self) -> ((usize, usize), (usize, usize)) {
        let ed = &self.editor;
        (ed.a11y_pos(ed.buf.anchor()), ed.a11y_pos(ed.buf.caret()))
    }

    /// AccessKit's SetTextSelection on the text: anchor and focus as (line, character within
    /// the line). The text gets the keyboard and the caret is scrolled into view. `false`
    /// when a position is past the last line, or the text can't take it now.
    pub(crate) fn editor_set_a11y_selection(
        &mut self,
        anchor: (usize, usize),
        focus: (usize, usize),
    ) -> bool {
        if !self.config_editor_open() || self.editor.discard.is_some() {
            return false;
        }
        let ed = &mut self.editor;
        let (Some(a), Some(f)) = (ed.a11y_byte(anchor), ed.a11y_byte(focus)) else {
            return false;
        };
        ed.buf.select(a..f);
        ed.region = Region::Text;
        ed.drag = None;
        ed.clicks.clear();
        ed.reveal = true;
        self.modal_focus = None;
        true
    }

    /// The problem row with the keyboard (the pane has it and no button does).
    pub(crate) fn editor_focused_problem(&self) -> Option<Target> {
        let ed = &self.editor;
        (self.modal == Some(Modal::ConfigEditor)
            && self.modal_focus.is_none()
            && ed.discard.is_none()
            && ed.region == Region::Problems
            && !ed.problems.is_empty())
        .then_some(Target::Editor(EditorTarget::Problem(ed.problem_sel)))
    }

    /// The problems as the screen reader lists them.
    pub(crate) fn editor_problem_label(&self, i: usize) -> Option<String> {
        let ed = &self.editor;
        let p = ed.problems.get(i)?;
        let line = ed.buf.line_of(p.span.start) + 1;
        let sev = match p.severity {
            Severity::Error => t!("config-editor-severity-error"),
            Severity::Warning => t!("config-editor-severity-warning"),
            Severity::Info => t!("config-editor-severity-info"),
        };
        Some(format!(
            "{sev}: {}",
            t!(
                "config-editor-problem-at",
                line = line as i64,
                message = p.message.clone()
            )
        ))
    }
}

/// A push button of the editor (`primary`: the accent default).
fn button(cx: &mut Ctx, r: Rect, label: &TextBlock, t: Target, primary: bool, enabled: bool) {
    let hov = enabled && cx.is_hover(&t);
    let (fill, text) = if primary {
        (
            if hov {
                cx.pal.accent_fill.lerp(cx.pal.text, 0.12)
            } else {
                cx.pal.accent_fill
            },
            cx.pal.accent_text,
        )
    } else {
        (
            if hov {
                cx.pal.surface_hover
            } else {
                cx.pal.surface_raised
            },
            if enabled {
                cx.pal.text
            } else {
                cx.pal.text_faint
            },
        )
    };
    cx.dl
        .rect_border(r, metrics::RADIUS_SM + 2.0, fill, cx.pal.separator, 1.0);
    cx.block_in(label, r, HAlign::Center, text);
    if cx.is_focused(&t) {
        cx.button_ring(r, metrics::RADIUS_SM + 2.0);
    }
    cx.hit(r, t);
}

fn squiggle(cx: &mut Ctx, x0: f32, x1: f32, y: f32, c: Color) {
    let mut x = x0;
    let mut up = true;
    while x < x1 - 0.01 {
        let nx = (x + 2.5).min(x1);
        let (a, b) = if up {
            (y + 1.5, y - 0.5)
        } else {
            (y - 0.5, y + 1.5)
        };
        cx.dl.line((x, a), (nx, b), 1.2, c);
        up = !up;
        x = nx;
    }
}

/// Draws the editor sheet.
pub(crate) fn draw(ch: &mut Chrome, cx: &mut Ctx, st: &ViewerState, want: &mut dyn FnMut(f64)) {
    let now = cx.now;
    ch.editor_poll(st);
    // Check the text once typing pauses.
    if ch.editor.linted != Some(ch.editor.buf.version()) {
        let wait = ch.editor.edited_at + LINT_DELAY - now;
        if wait <= 0.0 {
            ch.editor.relint();
        } else {
            want(wait);
        }
    }
    ch.editor.fit_problems();
    if ch.editor.alert == Some(Alert::CantSave) && ch.editor.errors() == 0 {
        ch.editor.alert = None;
    }
    let pal = cx.pal;
    let syn = pal.syntax;
    // Clicks around the sheet do nothing (the scrim would close it).
    cx.hit(Rect::new(0.0, 0.0, cx.w, cx.h), Target::ModalBody);
    let margin = 16.0f32.min(cx.w * 0.03).max(8.0);
    let r = Rect::new(margin, margin, cx.w - margin * 2.0, cx.h - margin * 2.0);
    cx.sheet(r);
    let pad = 16.0;
    let inner = Rect::new(r.x + pad, r.y + pad, r.w - pad * 2.0, r.h - pad * 2.0);
    let label_s = cx.style(metrics::FONT_LABEL).weight(550.0);
    let small = cx.style(metrics::FONT_SMALL);
    let bh = cx.u(32.0);

    // ----- Header: title and path, Modified, Revert, Save, Close ---------------------
    let mut y = inner.y;
    let close = cx.m_in(Rect::new(inner.right() - 32.0, y, 32.0, bh), r);
    cx.close_button(close);
    let save_l = cx.layout(&t!("config-editor-save"), label_s, None);
    let revert_l = cx.layout(&t!("config-editor-revert"), label_s, None);
    let save_w = save_l.width + 32.0;
    let revert_w = revert_l.width + 28.0;
    let mut end = inner.right() - 32.0 - 12.0;
    let save_r = cx.m_in(Rect::new(end - save_w, y, save_w, bh), r);
    button(
        cx,
        save_r,
        &save_l,
        Target::Editor(EditorTarget::Save),
        true,
        true,
    );
    end -= save_w + 8.0;
    let modified = ch.editor.modified();
    let revert_r = cx.m_in(Rect::new(end - revert_w, y, revert_w, bh), r);
    button(
        cx,
        revert_r,
        &revert_l,
        Target::Editor(EditorTarget::Revert),
        false,
        modified,
    );
    end -= revert_w + 12.0;
    if modified {
        let ml = cx.layout(&t!("config-editor-modified"), small, None);
        let mr = cx.m_in(Rect::new(end - ml.width, y, ml.width, bh), r);
        cx.block_in(&ml, mr, HAlign::Center, pal.text_muted);
        end -= ml.width + 12.0;
    }
    let title_s = cx.style(metrics::FONT_TITLE).weight(650.0);
    let file = ch.editor.file;
    let title = cx.layout(&file.title(), title_s, None);
    let tr = cx.m_in(Rect::new(inner.x, y, title.width, bh), r);
    cx.block_in(&title, tr, HAlign::Start, pal.text);
    let path_x = inner.x + title.width + 12.0;
    if !ch.editor.path.is_empty() && end - path_x > 40.0 {
        let pr = cx.m_in(Rect::new(path_x, y + 1.0, end - path_x, bh), r);
        // A path reads left to right whatever the language.
        let mut path = format!("\u{2066}{}\u{2069}", ch.editor.path);
        if file == EditorFile::Keymap && ch.keymap_file.text.is_none() {
            path = specular_i18n::dot_join(&[path, t!("config-editor-new-file")]);
        }
        cx.label_fit(&path, small, pr, HAlign::Start, pal.text_muted, true);
    }
    y += bh + 12.0;

    // ----- Bars: conflict, alert, find ----------------------------------------------
    let bar_h = cx.u(38.0);
    if ch.editor.conflict.is_some() && ch.editor.conflict_shown {
        let br = Rect::new(inner.x, y, inner.w, bar_h);
        cx.dl
            .rect(br, metrics::RADIUS_SM, syn.warning.with_alpha(0.16));
        let reload_l = cx.layout(&t!("config-editor-reload"), label_s, None);
        let keep_l = cx.layout(&t!("config-editor-keep-mine"), label_s, None);
        let bw = |b: &TextBlock| b.width + 24.0;
        let kh = bar_h - 8.0;
        let kr = cx.m_in(
            Rect::new(br.right() - 4.0 - bw(&keep_l), y + 4.0, bw(&keep_l), kh),
            br,
        );
        button(
            cx,
            kr,
            &keep_l,
            Target::Editor(EditorTarget::KeepMine),
            false,
            true,
        );
        let rr = cx.m_in(
            Rect::new(
                br.right() - 12.0 - bw(&keep_l) - bw(&reload_l),
                y + 4.0,
                bw(&reload_l),
                kh,
            ),
            br,
        );
        button(
            cx,
            rr,
            &reload_l,
            Target::Editor(EditorTarget::Reload),
            true,
            true,
        );
        let text_w = br.w - 32.0 - bw(&keep_l) - bw(&reload_l) - 24.0;
        let lr = cx.m_in(Rect::new(br.x + 12.0, y, text_w, bar_h), br);
        cx.label_fit(
            &file.conflict(),
            label_s,
            lr,
            HAlign::Start,
            pal.text,
            false,
        );
        y += bar_h + 8.0;
    }
    if let Some(a) = &ch.editor.alert {
        let msg = match a {
            Alert::CantSave => t!("config-editor-cant-save", count = ch.editor.errors() as i64),
            Alert::Failed(reason) => file.save_failed(reason),
        };
        let br = Rect::new(inner.x, y, inner.w, bar_h);
        cx.dl
            .rect(br, metrics::RADIUS_SM, syn.error.with_alpha(0.16));
        let ic = cx
            .m_in(Rect::new(br.x + 10.0, y, 20.0, bar_h), br)
            .centered(18.0, 18.0);
        specular_gfx::draw_icon(cx.dl, Icon::Alert, ic, syn.error, 1.6, false);
        let lr = cx.m_in(Rect::new(br.x + 38.0, y, br.w - 48.0, bar_h), br);
        cx.label_fit(&msg, label_s, lr, HAlign::Start, pal.text, false);
        y += bar_h + 8.0;
    }
    let find_matches = ch.editor.matches();
    if ch.editor.find.is_some() {
        let fh = cx.u(32.0);
        let fw = (inner.w * 0.45)
            .clamp(160.0, cx.u(360.0))
            .min(inner.w - 160.0);
        let fr = cx.m_in(Rect::new(inner.x, y, fw, fh), r);
        let focused = ch.editor.region == Region::Find
            && ch.modal_focus.is_none()
            && ch.editor.discard.is_none();
        let q = ch
            .editor
            .find
            .as_ref()
            .map(|f| f.query.clone())
            .unwrap_or_default();
        cx.field(
            fr,
            &q,
            ch.editor.find.as_ref().map(|f| &f.query_edit),
            &t!("config-editor-find-placeholder"),
            focused,
            false,
        );
        cx.hit(fr, Target::Editor(EditorTarget::FindField));
        let mut x = inner.x + fw + 6.0;
        for (t, icon) in [
            (EditorTarget::FindPrev, Icon::ChevronUp),
            (EditorTarget::FindNext, Icon::ChevronDown),
            (EditorTarget::FindClose, Icon::Close),
        ] {
            let br = cx.m_in(Rect::new(x, y, fh, fh), r);
            cx.icon_button(br, icon, Target::Editor(t), pal.text_muted, false, false);
            x += fh + 2.0;
        }
        let count = if q.is_empty() {
            String::new()
        } else if find_matches.is_empty() {
            t!("config-editor-find-none")
        } else {
            t!(
                "config-editor-find-count",
                current = (ch.editor.find_current(&find_matches) + 1) as i64,
                total = find_matches.len() as i64
            )
        };
        if !count.is_empty() {
            let cr = cx.m_in(Rect::new(x + 8.0, y, inner.right() - x - 8.0, fh), r);
            cx.label_fit(&count, small, cr, HAlign::Start, pal.text_muted, false);
        }
        y += fh + 8.0;
    }

    // ----- Problems pane and status line at the bottom -------------------------------
    let row_h = cx.u(24.0);
    let head_h = cx.u(28.0);
    let n_problems = ch.editor.problems.len();
    let rows = n_problems.min(PROBLEM_ROWS);
    let pane_h = head_h + rows as f32 * row_h;
    let status_h = cx.u(24.0);
    let pane = Rect::new(inner.x, inner.bottom() - status_h - pane_h, inner.w, pane_h);
    let code = Rect::new(inner.x, y, inner.w, (pane.y - 8.0 - y).max(40.0));

    // ----- The code ------------------------------------------------------------------
    draw_code(ch, cx, code, &find_matches, want);

    // Problems.
    let head = Rect::new(pane.x, pane.y, pane.w, head_h);
    let head_text = if n_problems == 0 {
        t!("config-editor-no-problems")
    } else {
        t!("config-editor-problems-count", count = n_problems as i64)
    };
    let hr = cx.m_in(Rect::new(head.x + 4.0, head.y, head.w - 8.0, head.h), head);
    cx.label_fit(
        &head_text,
        label_s,
        hr,
        HAlign::Start,
        if n_problems == 0 {
            pal.text_muted
        } else {
            pal.text
        },
        false,
    );
    let ed = &mut ch.editor;
    if n_problems > 0 {
        let sel = ed.problem_sel.min(n_problems - 1);
        if sel < ed.problems_top {
            ed.problems_top = sel;
        } else if sel >= ed.problems_top + rows {
            ed.problems_top = sel + 1 - rows;
        }
        ed.problems_top = ed.problems_top.min(n_problems - rows);
    }
    let pane_focus = ed.region == Region::Problems && ch.modal_focus.is_none();
    let top = ch.editor.problems_top;
    for i in top..(top + rows).min(n_problems) {
        let ed = &ch.editor;
        let p = &ed.problems[i];
        let rr = Rect::new(
            pane.x,
            head.bottom() + (i - top) as f32 * row_h,
            pane.w,
            row_h,
        );
        let t = Target::Editor(EditorTarget::Problem(i));
        let selected = i == ed.problem_sel;
        if selected && pane_focus {
            cx.dl.rect(rr, metrics::RADIUS_SM, pal.accent_soft);
        } else if cx.is_hover(&t) {
            cx.dl.rect(rr, metrics::RADIUS_SM, pal.surface_hover);
        }
        let (line, col) = {
            let l = ed.buf.line_of(p.span.start);
            let start = ed.buf.line_range(l).start;
            (
                l + 1,
                ed.buf.text()[start..p.span.start].chars().count() + 1,
            )
        };
        let color = severity_color(&syn, p.severity);
        let dot = cx.m_in(Rect::new(rr.x + 8.0, rr.y, 8.0, rr.h), rr);
        cx.dl.circle(dot.center(), 3.5, color);
        let pos = format!("{line}:{col}");
        let pos_w = cx.u(64.0);
        let pr = cx.m_in(Rect::new(rr.x + 24.0, rr.y, pos_w, rr.h), rr);
        let mono = cx.style(metrics::FONT_SMALL).code();
        cx.label_fit(&pos, mono, pr, HAlign::Start, pal.text_muted, false);
        let msg = p.message.clone();
        let mr = cx.m_in(
            Rect::new(rr.x + 24.0 + pos_w, rr.y, rr.w - 32.0 - pos_w, rr.h),
            rr,
        );
        cx.label_fit(&msg, small, mr, HAlign::Start, pal.text, false);
        if selected && pane_focus {
            cx.focus_ring(rr, metrics::RADIUS_SM);
        }
        cx.hit(rr, t);
    }

    // Status: the problem at the caret, and where the caret is.
    let ed = &mut ch.editor;
    let (cl, cc) = ed.buf.caret_line_col();
    let col = ed.buf.text()[ed.buf.line_range(cl).start..ed.buf.caret()]
        .chars()
        .count();
    let srow = Rect::new(inner.x, inner.bottom() - status_h, inner.w, status_h);
    let _ = cc;
    let pos = t!(
        "config-editor-position",
        line = (cl + 1) as i64,
        column = (col + 1) as i64
    );
    let pos_b = cx.layout(&pos, small, None);
    let posr = cx.m_in(
        Rect::new(
            srow.right() - pos_b.width - 4.0,
            srow.y,
            pos_b.width,
            srow.h,
        ),
        srow,
    );
    cx.block_in(&pos_b, posr, HAlign::Center, pal.text_muted);
    let caret_problem = ed.problem_at(ed.buf.caret());
    let status_msg = if let Some(i) = caret_problem {
        Some((
            ed.problems[i].message.clone(),
            severity_color(&syn, ed.problems[i].severity),
        ))
    } else {
        ed.saved_at
            .filter(|t| now - t < 4.0)
            .map(|_| (ed.file.saved(), pal.text_muted))
    };
    if let Some(t) = ed.saved_at
        && now - t < 4.0
    {
        want(t + 4.0 - now);
    }
    if let Some((msg, color)) = status_msg {
        let mr = cx.m_in(
            Rect::new(srow.x + 4.0, srow.y, srow.w - pos_b.width - 24.0, srow.h),
            srow,
        );
        cx.label_fit(&msg, small, mr, HAlign::Start, color, false);
    }
    // The screen reader hears a problem when the caret enters it.
    let ed = &mut ch.editor;
    let cur = caret_problem.map(|i| (ed.problems[i].span.clone(), ed.problems[i].message.clone()));
    if cur != ed.caret_problem {
        ed.caret_problem = cur.clone();
        if let Some((_, msg)) = cur
            && ed.region == Region::Text
        {
            ch.announce(msg);
        }
    }

    // ----- "Discard your changes?" ---------------------------------------------------
    if ch.editor.discard.is_some() {
        cx.dl.rect(r, metrics::RADIUS_LG, pal.scrim);
        let w = cx.u(420.0).min(r.w - 32.0);
        let q = cx.layout(
            &ch.editor.file.discard_question(),
            cx.style(metrics::FONT_BODY).weight(550.0),
            Some(w - 48.0),
        );
        let keep_l = cx.layout(&t!("config-editor-keep-editing"), label_s, None);
        let disc_l = cx.layout(&t!("config-editor-discard"), label_s, None);
        let h = 24.0 + q.height + 20.0 + bh + 20.0;
        let dr = Rect::new(((cx.w - w) * 0.5).round(), ((cx.h - h) * 0.5).round(), w, h);
        cx.sheet(dr);
        let qr = cx.m_in(Rect::new(dr.x + 24.0, dr.y + 24.0, w - 48.0, q.height), dr);
        cx.block_top(&q, qr, HAlign::Start, pal.text);
        let by = dr.bottom() - 20.0 - bh;
        let dw = disc_l.width + 32.0;
        let kw = keep_l.width + 32.0;
        let disc_r = cx.m_in(Rect::new(dr.right() - 20.0 - dw, by, dw, bh), dr);
        let keep_r = cx.m_in(Rect::new(dr.right() - 28.0 - dw - kw, by, kw, bh), dr);
        button(
            cx,
            keep_r,
            &keep_l,
            Target::Editor(EditorTarget::KeepEditing),
            false,
            true,
        );
        // The destructive choice is not the default.
        let t = Target::Editor(EditorTarget::Discard);
        let hov = cx.is_hover(&t);
        cx.dl.rect_border(
            disc_r,
            metrics::RADIUS_SM + 2.0,
            if hov {
                pal.danger_fill.lerp(pal.text, 0.12)
            } else {
                pal.danger_fill
            },
            pal.separator,
            1.0,
        );
        cx.block_in(&disc_l, disc_r, HAlign::Center, pal.accent_text);
        if cx.is_focused(&t) {
            cx.button_ring(disc_r, metrics::RADIUS_SM + 2.0);
        }
        cx.hit(disc_r, t);
    }
}

/// The text with its gutter, scrolled in `area`.
fn draw_code(
    ch: &mut Chrome,
    cx: &mut Ctx,
    area: Rect,
    find_matches: &[Range<usize>],
    want: &mut dyn FnMut(f64),
) {
    let pal = cx.pal;
    let syn = pal.syntax;
    // The code reads left to right in every language: so does its gutter and scrollbar.
    let rtl = cx.rtl;
    cx.rtl = false;
    let style: TextStyle = cx.style(13.0).code();
    let probe = cx.layout("0", style, None);
    let line_h = probe.height.ceil();
    let char_w = probe.width.max(1.0);
    let ed = &mut ch.editor;
    let n_lines = ed.buf.line_count();
    let digits = (n_lines.max(1).ilog10() + 1).max(3) as f32;
    let gutter_w = (digits * char_w + 26.0).round();
    cx.dl
        .rect_border(area, metrics::RADIUS_SM, syn.background, pal.separator, 1.0);
    let gutter = Rect::new(area.x + 1.0, area.y + 1.0, gutter_w, area.h - 2.0);
    cx.dl.rect_full(
        gutter,
        [metrics::RADIUS_SM - 1.0, 0.0, 0.0, metrics::RADIUS_SM - 1.0],
        syn.gutter,
        Color::TRANSPARENT,
        0.0,
    );
    let view = Rect::new(
        gutter.right(),
        area.y + 1.0,
        area.right() - 1.0 - gutter.right(),
        area.h - 2.0,
    );
    let top_pad = 6.0;
    let text_pad = 10.0;
    let content_h = n_lines as f32 * line_h + top_pad * 2.0;
    let max = (content_h - view.h).max(0.0);
    let page = ((view.h / line_h).floor() as usize)
        .saturating_sub(1)
        .max(1);

    // Keep the caret in view after it moved.
    if ed.reveal {
        let (cl, _) = ed.buf.caret_line_col();
        ed.reveal = false;
        let cy = top_pad + cl as f32 * line_h;
        if cy < ed.scroll {
            ed.scroll = cy - top_pad;
        } else if cy + line_h > ed.scroll + view.h {
            ed.scroll = cy + line_h + top_pad - view.h;
        }
        let r = ed.buf.line_range(cl);
        let b = cx.layout(&ed.buf.text()[r.clone()], style, None);
        let cx_ = b.caret_x(ed.buf.caret() - r.start);
        let tw = view.w - text_pad * 2.0 - scrollbar::HIT;
        if cx_ < ed.scroll_x {
            ed.scroll_x = (cx_ - char_w * 4.0).max(0.0);
        } else if cx_ > ed.scroll_x + tw {
            ed.scroll_x = cx_ - tw + char_w * 4.0;
        }
    }
    // A drag past an edge scrolls toward it, the faster the further past (the selection
    // follows below, as the wheel's does).
    let mut rehit = std::mem::take(&mut ed.drag_rehit);
    let past = |v: f32, lo: f32, hi: f32| {
        if v < lo {
            v - lo
        } else if v > hi {
            v - hi
        } else {
            0.0
        }
    };
    let (dy, dx) = match ed.drag_at.filter(|_| ed.drag.is_some()) {
        Some(p) => (
            past(p.1, view.y, view.bottom()),
            past(p.0, view.x, view.right()),
        ),
        None => (0.0, 0.0),
    };
    // Right: no further than the widest line shown needs.
    let tw = view.w - text_pad * 2.0 - scrollbar::HIT;
    let widest = ed
        .geom
        .lines
        .iter()
        .map(|(_, b)| b.width)
        .fold(0.0, f32::max);
    let most_x = (widest - tw + char_w * 4.0).max(ed.scroll_x);
    let scrolls = (dy < 0.0 && ed.scroll > 0.0)
        || (dy > 0.0 && ed.scroll < max)
        || (dx < 0.0 && ed.scroll_x > 0.0)
        || (dx > 0.0 && ed.scroll_x < most_x);
    if scrolls {
        let step =
            ed.autoscroll_at
                .map_or(0.0, |t| (cx.now - t).clamp(0.0, AUTOSCROLL_MAX_STEP)) as f32;
        ed.autoscroll_at = Some(cx.now);
        ed.scroll += dy * AUTOSCROLL_RATE * step;
        ed.scroll_x = (ed.scroll_x + dx * AUTOSCROLL_RATE * step).min(most_x);
        rehit = true;
        want(1.0 / 60.0);
    } else {
        ed.autoscroll_at = None;
    }
    ed.scroll = ed.scroll.clamp(0.0, max);
    ed.scroll_x = ed.scroll_x.max(0.0);
    let scroll = ed.scroll;
    let y0 = view.y + top_pad - scroll;
    let x0 = view.x + text_pad - ed.scroll_x;
    let first = ((scroll - top_pad) / line_h).floor().max(0.0) as usize;
    let last = (((scroll + view.h) / line_h).ceil() as usize).min(n_lines - 1);
    // The drag selects to the line under the pointer, or the first or last one shown.
    if rehit && let Some(p) = ed.drag_at.filter(|_| ed.drag.is_some()) {
        let l = ((p.1 - y0) / line_h)
            .floor()
            .clamp(first as f32, last as f32) as usize;
        let r = ed.buf.line_range(l);
        let b = cx.layout(&ed.buf.text()[r.clone()], style, None);
        let at = r.start + b.hit_byte(p.0 - x0).min(r.len());
        ed.drag_to(at);
    }
    let (cl, _) = ed.buf.caret_line_col();
    ed.lex_to(last);
    let pair = ed.bracket_pair();
    let ed = &mut ch.editor;
    let sel = ed.buf.selection();
    let caret = ed.buf.caret();
    let show_caret = ed.region == Region::Text && ch.modal_focus.is_none() && ed.discard.is_none();
    let cur_find = ed.find.as_ref().map(|_| {
        let i = ed.find_current(find_matches);
        find_matches.get(i).cloned()
    });
    let mut lines = Vec::new();
    // What an input method is composing: shown in the caret's line, at the caret.
    let preedit = std::mem::take(&mut cx.preedit);
    let mut ime_range = None;
    cx.dl.push_clip(Rect::new(area.x, view.y, area.w, view.h));
    for l in first..=last {
        let r = ed.buf.line_range(l);
        let ly = y0 + l as f32 * line_h;
        let line = &ed.buf.text()[r.clone()];
        let composing = (show_caret && l == cl && !preedit.is_empty())
            .then(|| crate::line_edit::compose(line, caret - r.start, &preedit));
        let block = match &composing {
            Some((shown, _)) => cx.layout(shown, style, None),
            None => cx.layout(line, style, None),
        };
        let pre = composing.as_ref().map(|c| c.1.clone());
        // A byte of the line to one of the text shown (after the composition: moved on).
        let shown_at = |b: usize| {
            let o = b.saturating_sub(r.start).min(r.len());
            match &pre {
                Some(p) if o >= p.start => o + p.len(),
                _ => o,
            }
        };
        let xb = |b: usize| x0 + block.caret_x(shown_at(b));
        let row = Rect::new(view.x, ly, view.w, line_h);
        if l == cl {
            cx.dl.rect(row, 0.0, syn.current_line);
            cx.dl.rect(
                Rect::new(gutter.x, ly, gutter.w, line_h),
                0.0,
                syn.current_line,
            );
        }
        cx.dl.push_clip(view);
        // Find matches, selection, bracket pair.
        let line_end = r.end + 1;
        for m in find_matches {
            if m.end <= r.start || m.start >= line_end {
                continue;
            }
            let c = if cur_find.as_ref() == Some(&Some(m.clone())) {
                pal.selection
            } else {
                syn.find_match
            };
            let (a, b) = (xb(m.start.max(r.start)), xb(m.end.min(r.end)));
            cx.dl
                .rect(Rect::new(a, ly, (b - a).max(2.0), line_h), 2.0, c);
        }
        if !sel.is_empty() && sel.start < line_end && sel.end >= r.start {
            let a = xb(sel.start.max(r.start));
            let mut b = xb(sel.end.min(r.end));
            if sel.end > r.end {
                b += char_w * 0.6;
            }
            cx.dl.rect(
                Rect::new(a, ly, (b - a).max(0.0), line_h),
                0.0,
                pal.selection,
            );
        }
        if let Some((p, q)) = pair {
            for at in [p, q] {
                if at >= r.start && at < r.end {
                    let (a, b) = (xb(at), xb(at + 1));
                    cx.dl
                        .rect(Rect::new(a, ly, b - a, line_h), 2.0, syn.bracket_match);
                }
            }
        }
        // The text, coloured by token. Selected and found text is plain text: the token
        // colours don't all keep 4.5:1 on the selection and find tints.
        let spans = &ed.lex[l].0;
        let marks: Vec<std::ops::Range<usize>> = std::iter::once(sel.clone())
            .chain(find_matches.iter().cloned())
            .filter(|m| m.start < line_end && m.end > r.start)
            .collect();
        let ty = ly + (line_h - block.height) * 0.5;
        cx.text.draw_colored(cx.dl, &block, x0, ty, &mut |b| {
            let o = match &pre {
                Some(p) if p.contains(&b) => return syn.text,
                Some(p) if b >= p.end => b - p.len(),
                _ => b,
            };
            marked_color(
                &syn,
                &marks,
                r.start + o,
                tok_color(&syn, toml_lex::tok_at(spans, o)),
            )
        });
        if let Some(p) = &pre {
            crate::line_edit::draw_preedit_underline(
                cx.dl,
                &block,
                x0,
                p.clone(),
                ty + block.height,
                syn.text,
            );
            ime_range = Some((l, p.clone()));
        }
        // Problems: squiggles under their span (a point gets one character's width).
        let mut worst: Option<Severity> = None;
        for p in &ed.problems {
            let (s, e) = (p.span.start, p.span.end);
            let on = if s == e {
                s >= r.start && s <= r.end
            } else {
                s < line_end && e > r.start
            };
            if !on {
                continue;
            }
            worst = Some(worst.map_or(p.severity, |w| w.min(p.severity)));
            let a = xb(s.max(r.start));
            let mut b = xb(e.min(r.end));
            if b - a < char_w {
                b = a + char_w;
            }
            squiggle(
                cx,
                a,
                b,
                ly + line_h - 2.5,
                severity_color(&syn, p.severity),
            );
        }
        if show_caret && l == cl {
            let x = xb(caret);
            cx.dl.rect(
                Rect::new(x - 0.75, ly + 1.0, 1.5, line_h - 2.0),
                0.0,
                pal.accent,
            );
        }
        cx.dl.pop_clip();
        // The gutter: number and problem mark.
        let num = format!("{}", l + 1);
        let nb = cx.layout(&num, style, None);
        let nc = if l == cl {
            syn.line_number_current
        } else {
            syn.line_number
        };
        cx.text
            .draw(cx.dl, &nb, gutter.right() - 8.0 - nb.width, ty, nc);
        if let Some(w) = worst {
            cx.dl.circle(
                (gutter.x + 8.0, ly + line_h * 0.5),
                3.0,
                severity_color(&syn, w),
            );
        }
        lines.push((l, block));
    }
    cx.dl.pop_clip();
    // The input method's candidates open at the caret, under the composition.
    if show_caret && let Some((_, b)) = lines.iter().find(|(l, _)| *l == cl) {
        let r = ed.buf.line_range(cl);
        let ly = y0 + cl as f32 * line_h;
        cx.text_field = Some(view);
        cx.ime_area = Some(match ime_range.filter(|(l, _)| *l == cl) {
            Some((_, p)) => crate::line_edit::ime_area(b, x0, p, ly, line_h, view),
            None => Rect::new(x0 + b.caret_x(caret - r.start), ly, 1.5, line_h),
        });
    }
    cx.preedit = preedit;
    // A problem's message under the pointer.
    let hover_problem = ch.pointer.filter(|p| view.contains(*p)).and_then(|p| {
        let l = ((p.1 - y0) / line_h).floor();
        if l < 0.0 {
            return None;
        }
        let (_, b) = lines.iter().find(|(n, _)| *n == l as usize)?;
        let r = ch.editor.buf.line_range(l as usize);
        let at = r.start + b.hit_byte(p.0 - x0).min(r.len());
        let i = ch.editor.problem_at(at)?;
        Some((i, Rect::new(p.0 - 4.0, y0 + l * line_h, 8.0, line_h)))
    });
    ch.editor.geom = Geom {
        x0,
        y0,
        line_h,
        lines,
        page,
    };
    cx.hit(area, Target::Editor(EditorTarget::Text));
    scrollbar::draw(ch, cx, scrollbar::Area::ConfigEditor, view, area, max);
    if let Some((i, anchor)) = hover_problem {
        let msg = ch.editor.problems[i].message.clone();
        cx.tooltip(&msg, anchor);
    }
    cx.rtl = rtl;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HudStats, Input, Overlays};
    use specular_gfx::{DrawList, Text};
    use specular_input::{ActionState, ImageState, Keymap, Mods};
    use specular_settings::Settings;
    use specular_theme::Palette;

    fn with_store<R>(store: &SettingsStore, f: impl FnOnce(&ViewerState) -> R) -> R {
        with_keymap(store, &Keymap::default(), f)
    }

    /// Selected and found text is drawn as plain text, which keeps 4.5:1 on the selection
    /// and find tints over the page and the caret's line in both themes (the token colours
    /// fall to 3.4:1 there).
    #[test]
    fn marked_text_meets_aa_on_its_tint() {
        let ratio = |a: Color, b: Color| {
            let (la, lb) = (a.luminance(), b.luminance());
            (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
        };
        let over =
            |top: Color, bottom: Color| bottom.lerp(top.with_alpha(1.0), top.a).with_alpha(1.0);
        for p in [Palette::dark(), Palette::light()] {
            let s = &p.syntax;
            let marks = [2..5, 9..10];
            assert_eq!(marked_color(s, &marks, 3, s.key), s.text);
            assert_eq!(marked_color(s, &marks, 9, s.key), s.text);
            assert_eq!(marked_color(s, &marks, 5, s.key), s.key);
            for page in [s.background, over(s.current_line, s.background)] {
                for tint in [p.selection, s.find_match] {
                    let r = ratio(s.text, over(tint, page));
                    assert!(r >= 4.5, "{:?} {tint:?}: {r:.2}", p.mode);
                }
            }
        }
    }

    fn with_keymap<R>(
        store: &SettingsStore,
        keymap: &Keymap,
        f: impl FnOnce(&ViewerState) -> R,
    ) -> R {
        let hud = HudStats::default();
        let settings = Settings::default();
        let st = ViewerState {
            actions: ActionState {
                image: ImageState::Ready,
                count: 1,
                animated: false,
                video: false,
                sound: false,
                sound_file: false,
                in_archive: false,
                has_alpha: false,
                gallery_views: None,
                ..ActionState::default()
            },
            has_image: true,
            index: 0,
            count: 1,
            zoom: 1.0,
            at_fit: true,
            fullscreen: false,
            file_name: Some("a.jpg"),
            dims: Some((40, 30)),
            loading: false,
            error: None,
            truncated: false,
            metadata: None,
            hud: &hud,
            keymap,
            store,
            settings: &settings,
            overlays: Overlays::default(),
            playback: None,
            motion: None,
            spread: None,
            pages: None,
        };
        f(&st)
    }

    struct T {
        c: Chrome,
        text: Text,
        now: f64,
    }

    impl T {
        fn new() -> T {
            let mut c = Chrome::new();
            c.set_size(1000.0, 700.0);
            c.set_accessibility(true);
            c.poke(1.0);
            T {
                c,
                text: Text::new(),
                now: 1.0,
            }
        }

        fn draw(&mut self, st: &ViewerState) {
            let mut dl = DrawList::new(1.0);
            self.now += 0.05;
            self.c
                .draw(&mut dl, &mut self.text, st, &Palette::dark(), self.now);
        }

        /// Lets the debounced check run.
        fn settle(&mut self, st: &ViewerState) {
            self.now += 1.0;
            self.draw(st);
        }

        fn key(&mut self, st: &ViewerState, key: Key, mods: Mods) -> Response {
            let r = self.c.input(
                &Input::Key {
                    combo: KeyCombo { key, mods },
                    repeat: false,
                },
                st,
                self.now,
            );
            self.draw(st);
            r
        }

        fn named(&mut self, st: &ViewerState, k: NamedKey) -> Response {
            self.key(st, Key::Named(k), Mods::NONE)
        }

        fn ctrl(&mut self, st: &ViewerState, c: char) -> Response {
            self.key(st, Key::Char(c), Mods::CTRL)
        }

        fn type_text(&mut self, st: &ViewerState, s: &str) {
            self.c.input(&Input::Text(s.to_owned()), st, self.now);
            self.draw(st);
        }

        fn esc(&mut self, st: &ViewerState) {
            self.named(st, NamedKey::Escape);
        }

        fn announced(&self) -> String {
            self.c
                .announce
                .as_ref()
                .map(|a| a.1.clone())
                .unwrap_or_default()
        }
    }

    #[test]
    fn save_is_refused_while_errors_remain() {
        let store = SettingsStore::in_memory();
        with_store(&store, |st| {
            let mut t = T::new();
            t.c.open_config_editor_with(
                st.store,
                Some("[viewing]\ninitial_zoom = \"huge\"\nbogus = 1\n"),
            );
            t.draw(st);
            assert_eq!(t.c.editor.problems.len(), 2);
            let before = store.file_text();
            t.ctrl(st, 's');
            // Nothing written; the editor says why and the problems pane has the keyboard.
            assert_eq!(store.file_text(), before);
            assert_eq!(t.c.editor.alert, Some(Alert::CantSave));
            assert_eq!(t.c.editor.region, Region::Problems);
            assert!(t.announced().contains('2'), "{}", t.announced());
            let tree = t.c.accessibility_tree(st, "x", 1.0);
            let focus = tree.nodes.iter().find(|(id, _)| *id == tree.focus).unwrap();
            assert_eq!(focus.1.role(), accesskit::Role::ListBoxOption);
            let label = focus
                .1
                .label()
                .unwrap_or_default()
                .replace(['\u{2068}', '\u{2069}'], "");
            assert!(label.contains("Line 2"), "{label}");
            // Enter on a problem jumps to it, selecting its span.
            t.named(st, NamedKey::Enter);
            assert_eq!(t.c.editor.region, Region::Text);
            assert_eq!(t.c.editor.buf.selected_text(), "\"huge\"");
            // Fixing one leaves one: still refused, and the alert counts what is left.
            t.type_text(st, "\"fit\"");
            t.settle(st);
            assert_eq!(t.c.editor.errors(), 1);
            t.ctrl(st, 's');
            assert_eq!(store.file_text(), before);
            // Fix the other (F8 goes to it) and save: applied at once.
            t.named(st, NamedKey::F(8));
            assert_eq!(t.c.editor.buf.selected_text(), "bogus");
            t.named(st, NamedKey::End);
            t.key(st, Key::Named(NamedKey::Home), Mods::SHIFT);
            t.named(st, NamedKey::Delete);
            t.named(st, NamedKey::Delete);
            t.settle(st);
            assert_eq!(t.c.editor.errors(), 0, "{:?}", t.c.editor.problems);
            assert!(t.c.editor.alert.is_none());
            t.ctrl(st, 's');
            assert_eq!(store.file_text(), "[viewing]\ninitial_zoom = \"fit\"\n");
            assert_eq!(
                store.snapshot().viewing.initial_zoom,
                specular_settings::InitialZoom::Fit
            );
            assert!(!t.c.editor.modified());
            // Saved text is not taken for an edit made elsewhere.
            t.draw(st);
            assert!(t.c.editor.conflict.is_none());
        });
    }

    #[test]
    fn changes_elsewhere_follow_or_ask() {
        use specular_settings::Origin;
        let store = SettingsStore::in_memory();
        with_store(&store, |st| {
            let mut t = T::new();
            t.c.open_config_editor(st.store);
            t.draw(st);
            // No edits: the editor follows silently.
            store
                .set_text("viewing.initial_zoom", "fill", Origin::Palette)
                .unwrap();
            t.draw(st);
            assert!(t.c.editor.conflict.is_none());
            assert!(t.c.editor.buf.text().contains("initial_zoom = \"fill\""));
            assert!(!t.c.editor.modified());
            // With edits: the bar, and saving waits for an answer.
            t.key(st, Key::Named(NamedKey::End), Mods::CTRL);
            t.type_text(st, "# mine");
            assert!(t.c.editor.modified());
            store
                .set_text("viewing.initial_zoom", "fit", Origin::Palette)
                .unwrap();
            t.draw(st);
            assert!(t.c.editor.conflict.is_some() && t.c.editor.conflict_shown);
            let mine = t.c.editor.buf.text().to_owned();
            t.ctrl(st, 's');
            assert!(!store.file_text().contains("# mine"));
            assert_eq!(t.c.modal_focus, Some(Target::Editor(EditorTarget::Reload)));
            // Esc goes back to the text, then hides the bar; Save brings it back instead of
            // overwriting.
            t.esc(st);
            t.esc(st);
            assert!(!t.c.editor.conflict_shown);
            t.ctrl(st, 's');
            assert!(t.c.editor.conflict_shown);
            assert!(!store.file_text().contains("# mine"));
            // Keep mine: the next save replaces theirs on purpose.
            let r = t.c.activate(Target::Editor(EditorTarget::KeepMine), st);
            assert!(r.consumed);
            assert_eq!(t.c.editor.buf.text(), mine);
            t.settle(st);
            t.ctrl(st, 's');
            assert_eq!(store.file_text(), mine);
            // Reload: their text replaces mine...
            t.type_text(st, "\n# again");
            store
                .set_text("viewing.initial_zoom", "fit", Origin::Palette)
                .unwrap();
            t.draw(st);
            assert!(t.c.editor.conflict.is_some());
            t.c.activate(Target::Editor(EditorTarget::Reload), st);
            assert_eq!(t.c.editor.buf.text(), store.file_text());
            assert!(!t.c.editor.modified());
            // ... and undo brings mine back.
            t.ctrl(st, 'z');
            assert!(t.c.editor.buf.text().contains("# again"));
        });
    }

    #[test]
    fn esc_goes_back_one_step_to_the_settings_page() {
        use crate::settings::SettingsTarget as S;
        const KEY: &str = "advanced.edit_settings_file";
        let store = SettingsStore::in_memory();
        with_store(&store, |st| {
            let mut t = T::new();
            t.c.set_size(1000.0, 400.0);
            t.c.open_modal(Modal::Settings);
            // As the sidebar lists it here (a section with nothing to show on this platform is
            // left out), not among every section.
            let section = t.c.section_index("advanced").unwrap();
            t.c.settings.section = section;
            t.draw(st);
            t.c.settings.scroll = 40.0;
            t.draw(st);
            assert_eq!(t.c.settings.scroll, 40.0, "the page scrolls");
            let focus = Some(Target::Settings(S::Act(KEY)));
            t.c.modal_focus = focus.clone();
            t.c.settings_click(S::Act(KEY), st);
            assert!(t.c.config_editor_open());
            t.draw(st);
            // The find bar goes first...
            t.ctrl(st, 'f');
            t.type_text(st, "version");
            assert!(t.c.editor.find.is_some());
            t.esc(st);
            assert!(t.c.editor.find.is_none() && t.c.config_editor_open());
            // ... then, with edits, the question (Keep editing is the default)...
            t.type_text(st, "x");
            t.esc(st);
            assert!(
                t.c.editor.discard.is_some(),
                "edits are not dropped silently"
            );
            assert_eq!(
                t.c.modal_focus,
                Some(Target::Editor(EditorTarget::KeepEditing))
            );
            t.esc(st);
            assert!(t.c.editor.discard.is_none());
            assert!(t.c.editor.modified(), "Esc on the question keeps editing");
            t.esc(st);
            t.named(st, NamedKey::Tab);
            assert_eq!(t.c.modal_focus, Some(Target::Editor(EditorTarget::Discard)));
            t.named(st, NamedKey::Enter);
            // ... and back on the Settings page it came from, as it was.
            assert_eq!(t.c.modal, Some(Modal::Settings));
            assert_eq!(t.c.settings.section, section);
            assert_eq!(t.c.settings.scroll, 40.0);
            assert_eq!(t.c.modal_focus, focus);
            assert!(!store.file_text().contains('x'));
            // Opened from elsewhere (the palette) and unchanged: Esc closes it.
            t.c.close_modal();
            t.c.open_config_editor(st.store);
            t.draw(st);
            t.esc(st);
            assert_eq!(t.c.modal, None);
        });
    }

    #[test]
    fn quit_with_unsaved_edits_asks_first() {
        let store = SettingsStore::in_memory();
        with_store(&store, |st| {
            let mut t = T::new();
            t.c.open_config_editor(st.store);
            t.draw(st);
            t.type_text(st, "x");
            let r = t.ctrl(st, 'q');
            assert!(r.commands.is_empty());
            assert_eq!(t.c.editor.discard, Some(After::Quit));
            let r = t.c.activate(Target::Editor(EditorTarget::Discard), st);
            assert_eq!(r.commands, vec![Command::Action(Action::Quit)]);
            assert_eq!(t.c.modal, None);
        });
    }

    #[test]
    fn editing_keys_clipboard_and_find() {
        let store = SettingsStore::in_memory();
        with_store(&store, |st| {
            let mut t = T::new();
            t.c.open_config_editor_with(st.store, Some(""));
            t.draw(st);
            t.type_text(st, "[viewing]");
            t.named(st, NamedKey::Enter);
            t.type_text(st, "a = {");
            t.named(st, NamedKey::Enter);
            // Auto-indent after an opening brace; Tab inserts spaces.
            assert_eq!(t.c.editor.buf.text(), "[viewing]\na = {\n    ");
            t.named(st, NamedKey::Tab);
            assert!(t.c.editor.buf.text().ends_with("\n        "));
            t.ctrl(st, 'z');
            assert!(t.c.editor.buf.text().ends_with("\n    "));
            t.ctrl(st, 'a');
            let r = t.ctrl(st, 'c');
            assert_eq!(
                r.commands,
                vec![Command::EditorCopy("[viewing]\na = {\n    ".into())]
            );
            let r = t.ctrl(st, 'x');
            assert_eq!(r.commands.len(), 1);
            assert_eq!(t.c.editor.buf.text(), "");
            assert_eq!(t.ctrl(st, 'v').commands, vec![Command::EditorPaste]);
            t.c.paste_text("[viewing]\r\ninitial_zoom = \"fit\"\r\n");
            assert_eq!(t.c.editor.buf.text(), "[viewing]\ninitial_zoom = \"fit\"\n");
            // Find selects matches as typed; F3 steps on.
            t.key(st, Key::Named(NamedKey::Home), Mods::CTRL);
            t.ctrl(st, 'f');
            t.type_text(st, "i");
            assert_eq!(t.c.editor.buf.selection(), 2..3);
            t.named(st, NamedKey::F(3));
            assert_eq!(t.c.editor.buf.selection(), 5..6);
            // The bracket of a table header pairs with its close.
            t.esc(st);
            t.key(st, Key::Named(NamedKey::Home), Mods::CTRL);
            assert_eq!(t.c.editor.bracket_pair(), Some((0, 8)));
            // The screen reader has the text, with the caret.
            let tree = t.c.accessibility_tree(st, "x", 1.0);
            let focus = tree.nodes.iter().find(|(id, _)| *id == tree.focus).unwrap();
            assert_eq!(focus.1.role(), accesskit::Role::MultilineTextInput);
            assert_eq!(focus.1.value(), Some(t.c.editor.buf.text()));
        });
    }

    /// Where there is a primary selection, the text's selection is offered to it: on release
    /// after a drag, and after keys once they go up or rest (not on every step of a held
    /// Shift+Arrow). A middle click on the text puts the caret there and asks the app for
    /// the primary selection, pasted as one undo step. A selection the find field makes is
    /// not offered. Nothing here touches a real clipboard: the commands are only looked at.
    #[test]
    fn primary_selection_copy_on_select_and_middle_click() {
        use crate::Button;
        let store = SettingsStore::in_memory();
        let primary = |cmds: &[Command]| -> Vec<String> {
            cmds.iter()
                .filter_map(|c| match c {
                    Command::PrimaryCopy(s) => Some(s.clone()),
                    _ => None,
                })
                .collect()
        };
        with_store(&store, |st| {
            let doc = "[viewing]\ninitial_zoom = \"fit\"\n";
            let mut t = T::new();
            t.c.primary_selection = true;
            t.c.open_config_editor_with(st.store, Some(doc));
            t.draw(st);
            // Where byte `at` of line `l` is drawn.
            let point = |t: &T, l: usize, at: usize| {
                let g = &t.c.editor.geom;
                let (_, b) = g.lines.iter().find(|(n, _)| *n == l).expect("drawn");
                (
                    g.x0 + b.caret_x(at) + 0.25,
                    g.y0 + (l as f32 + 0.5) * g.line_h,
                )
            };
            let button = |t: &mut T, p: (f32, f32), button: Button, down: bool| {
                let ev = Input::PointerButton {
                    x: p.0,
                    y: p.1,
                    button,
                    down,
                };
                let r = t.c.input(&ev, st, t.now);
                t.draw(st);
                r.commands
            };
            // A drag offers what it selected once, when the button goes up.
            let r = {
                let p = point(&t, 1, 0);
                button(&mut t, p, Button::Primary, true)
            };
            assert!(primary(&r).is_empty());
            let (x, y) = point(&t, 1, 7);
            let r = t.c.input(&Input::PointerMove { x, y }, st, t.now);
            assert!(primary(&r.commands).is_empty());
            assert_eq!(t.c.editor.buf.selected_text(), "initial");
            let r = button(&mut t, (x, y), Button::Primary, false);
            assert_eq!(primary(&r), ["initial"]);
            assert!(t.c.take_commands().is_empty());
            // A held Shift+Right offers nothing on each step...
            for _ in 0..5 {
                let r = t.key(st, Key::Named(NamedKey::ArrowRight), Mods::SHIFT);
                assert!(primary(&r.commands).is_empty());
                assert!(primary(&t.c.take_commands()).is_empty());
            }
            assert_eq!(t.c.editor.buf.selected_text(), "initial_zoom");
            assert!(t.c.primary_pending());
            // ...and the selection once the keys rest (the frame after a short idle).
            t.settle(st);
            assert_eq!(primary(&t.c.take_commands()), ["initial_zoom"]);
            assert!(!t.c.primary_pending());
            // Or at once when the key goes up.
            t.key(st, Key::Named(NamedKey::End), Mods::SHIFT);
            let up = Input::KeyUp {
                combo: KeyCombo {
                    key: Key::Named(NamedKey::End),
                    mods: Mods::SHIFT,
                },
            };
            let r = t.c.input(&up, st, t.now);
            assert_eq!(primary(&r.commands), ["initial_zoom = \"fit\""]);
            // The clipboard keys don't offer it again.
            let r = t.ctrl(st, 'c');
            assert_eq!(
                r.commands,
                [Command::EditorCopy("initial_zoom = \"fit\"".into())]
            );
            t.settle(st);
            assert!(t.c.take_commands().is_empty());
            // A middle click: the caret goes there, the selection is let go, and the app is
            // asked for the primary selection, pasted there as one undo step.
            let r = {
                let p = point(&t, 0, 9);
                button(&mut t, p, Button::Middle, true)
            };
            assert_eq!(r, [Command::PrimaryPaste]);
            assert!(
                {
                    let p = point(&t, 0, 9);
                    button(&mut t, p, Button::Middle, false)
                }
                .is_empty()
            );
            assert_eq!(t.c.editor.buf.selection(), 9..9);
            t.c.paste_text("\n# zoom");
            assert_eq!(
                t.c.editor.buf.text(),
                "[viewing]\n# zoom\ninitial_zoom = \"fit\"\n"
            );
            t.ctrl(st, 'z');
            assert_eq!(t.c.editor.buf.text(), doc);
            // A selection the find field makes is not offered.
            t.ctrl(st, 'f');
            t.type_text(st, "zoom");
            assert_eq!(t.c.editor.buf.selected_text(), "zoom");
            let r = t.c.input(&up, st, t.now);
            assert!(primary(&r.commands).is_empty());
            t.settle(st);
            assert!(primary(&t.c.take_commands()).is_empty());
            // Esc leaves the match selected in the text (as VS Code and gedit do), still not
            // offered: neither when the key goes up nor once the keys rest...
            t.esc(st);
            assert!(t.c.editor.find.is_none());
            assert_eq!(t.c.editor.region, Region::Text);
            assert_eq!(t.c.editor.buf.selected_text(), "zoom");
            let esc_up = Input::KeyUp {
                combo: KeyCombo {
                    key: Key::Named(NamedKey::Escape),
                    mods: Mods::NONE,
                },
            };
            let r = t.c.input(&esc_up, st, t.now);
            assert!(primary(&r.commands).is_empty());
            t.settle(st);
            assert!(primary(&t.c.take_commands()).is_empty());
            assert!(!t.c.primary_pending());
            // ...until the user changes it.
            t.key(st, Key::Named(NamedKey::ArrowRight), Mods::SHIFT);
            let r = t.c.input(&up, st, t.now);
            assert_eq!(primary(&r.commands), ["zoom "]);
            // A middle click off the text (here, outside the sheet) is not the text's.
            let off = (4.0, 4.0);
            let r = button(&mut t, off, Button::Middle, true);
            assert!(!r.contains(&Command::PrimaryPaste));
            // With no primary selection, neither happens.
            t.c.primary_selection = false;
            t.key(st, Key::Named(NamedKey::Home), Mods::SHIFT);
            let r = t.c.input(&up, st, t.now);
            assert!(primary(&r.commands).is_empty());
            let r = {
                let p = point(&t, 1, 2);
                button(&mut t, p, Button::Middle, true)
            };
            assert!(r.is_empty());
            assert_eq!(t.c.editor.buf.text(), doc);
        });
    }

    /// The text takes clicks as the one-line fields do: a double click selects a word (within
    /// its line, right-to-left words too), a triple click the line with its break, a drag
    /// then extends by words or lines either way, and Shift+click extends the selection
    /// (however quick: it is never a double click). What a click selects is offered as the
    /// primary selection once the button goes up, and the screen reader sees it. Ctrl+Insert
    /// copies, Shift+Insert pastes and Shift+Delete cuts (one undo step), all through the
    /// app's commands: nothing here touches a real clipboard.
    #[test]
    fn clicks_select_words_and_lines_and_the_other_clipboard_keys() {
        use crate::Button;
        let store = SettingsStore::in_memory();
        let primary = |cmds: &[Command]| -> Vec<String> {
            cmds.iter()
                .filter_map(|c| match c {
                    Command::PrimaryCopy(s) => Some(s.clone()),
                    _ => None,
                })
                .collect()
        };
        with_store(&store, |st| {
            let doc = "[viewing]\ninitial_zoom = \"fit\"\nzoom_step = 1.5\ntitle = \"שלום עולם\"\n";
            let mut t = T::new();
            t.c.primary_selection = true;
            t.c.open_config_editor_with(st.store, Some(doc));
            t.draw(st);
            // Where byte `at` (of the text) is drawn.
            let point = |t: &T, at: usize| {
                let ed = &t.c.editor;
                let l = ed.buf.line_of(at);
                let g = &ed.geom;
                let (_, b) = g.lines.iter().find(|(n, _)| *n == l).expect("drawn");
                (
                    g.x0 + b.caret_x(at - ed.buf.line_range(l).start) + 0.25,
                    g.y0 + (l as f32 + 0.5) * g.line_h,
                )
            };
            let button = |t: &mut T, at: usize, down: bool| {
                let p = point(t, at);
                let ev = Input::PointerButton {
                    x: p.0,
                    y: p.1,
                    button: Button::Primary,
                    down,
                };
                let r = t.c.input(&ev, st, t.now);
                t.draw(st);
                r.commands
            };
            let click = |t: &mut T, at: usize| {
                let mut r = button(t, at, true);
                r.extend(button(t, at, false));
                r
            };
            let move_to = |t: &mut T, at: usize| {
                let (x, y) = point(t, at);
                let r = t.c.input(&Input::PointerMove { x, y }, st, t.now);
                assert!(primary(&r.commands).is_empty(), "not while dragging");
            };
            let sel = |t: &T| {
                let b = &t.c.editor.buf;
                (b.anchor(), b.caret())
            };
            let at = |s: &str| doc.find(s).unwrap();
            let zoom = at("initial_zoom");
            let step = at("zoom_step");
            // A double click selects the word under it, offered on release.
            click(&mut t, zoom + 3);
            let r = button(&mut t, zoom + 3, true);
            assert_eq!(t.c.editor.buf.selected_text(), "initial_zoom");
            assert!(primary(&r).is_empty());
            // Dragged on, it grows by whole words, down...
            move_to(&mut t, step + 2);
            assert_eq!(sel(&t), (zoom, step + "zoom_step".len()));
            // ...and up, keeping the word it started on.
            move_to(&mut t, 3);
            assert_eq!(sel(&t), (zoom + "initial_zoom".len(), 1));
            assert_eq!(primary(&button(&mut t, 3, false)), [&doc[1..22]]);
            // A word is within its line: a double click at a line's end takes its last word.
            t.now += 1.0;
            click(&mut t, step - 1);
            click(&mut t, step - 1);
            assert_eq!(t.c.editor.buf.selected_text(), "\"");
            // Right-to-left words.
            t.now += 1.0;
            let olam = at("עולם") + 4;
            click(&mut t, olam);
            click(&mut t, olam);
            assert_eq!(t.c.editor.buf.selected_text(), "עולם");
            // A triple click selects the line with its break; a drag goes by lines.
            t.now += 1.0;
            click(&mut t, step + 4);
            click(&mut t, step + 4);
            let r = button(&mut t, step + 4, true);
            let line = "zoom_step = 1.5\n";
            assert_eq!(t.c.editor.buf.selected_text(), line);
            move_to(&mut t, zoom + 2);
            assert_eq!(sel(&t), (step + line.len(), zoom));
            move_to(&mut t, doc.len() - 2);
            assert_eq!(sel(&t), (step, doc.len()));
            let mut r2 = button(&mut t, doc.len() - 2, false);
            r2.extend(r);
            assert_eq!(primary(&r2), [&doc[step..]]);
            // The screen reader sees it, by lines and characters within them.
            let tree = t.c.accessibility_tree(st, "x", 1.0);
            let focus = tree.nodes.iter().find(|(id, _)| *id == tree.focus).unwrap();
            let s = focus.1.text_selection().expect("a selection");
            let pos = |p: accesskit::TextPosition| (p.node, p.character_index);
            assert_eq!(
                (pos(s.anchor), pos(s.focus)),
                (
                    (crate::a11y::editor_line_id(2), 0),
                    (crate::a11y::editor_line_id(4), 0)
                )
            );
            // A fourth press starts over: the caret.
            t.now += 1.0;
            for _ in 0..4 {
                click(&mut t, zoom + 3);
            }
            assert_eq!(sel(&t), (zoom + 3, zoom + 3));
            // Shift+click extends from the anchor, and quick ones are no double click.
            t.now += 1.0;
            t.c.input(&Input::Modifiers(Mods::SHIFT), st, t.now);
            let r = click(&mut t, step + 4);
            assert_eq!(sel(&t), (zoom + 3, step + 4));
            assert_eq!(primary(&r), [&doc[zoom + 3..step + 4]]);
            click(&mut t, 2);
            assert_eq!(sel(&t), (zoom + 3, 2));
            t.c.input(&Input::Modifiers(Mods::NONE), st, t.now);
            // Typed over, a word double-clicked goes in one undo step.
            t.now += 1.0;
            click(&mut t, step + 1);
            click(&mut t, step + 1);
            t.type_text(st, "zoom_steps");
            assert!(t.c.editor.buf.text().contains("zoom_steps = 1.5"));
            t.ctrl(st, 'z');
            assert_eq!(t.c.editor.buf.text(), doc);
            assert_eq!(t.c.editor.buf.selected_text(), "zoom_step");
            // Ctrl+Insert copies; Shift+Delete cuts, one undo step.
            let shift_key = |t: &mut T, k: NamedKey| t.key(st, Key::Named(k), Mods::SHIFT);
            let r = t.key(st, Key::Named(NamedKey::Insert), Mods::CTRL);
            assert_eq!(r.commands, [Command::EditorCopy("zoom_step".into())]);
            assert_eq!(t.c.editor.buf.text(), doc);
            let r = shift_key(&mut t, NamedKey::Delete);
            assert_eq!(r.commands, [Command::EditorCopy("zoom_step".into())]);
            assert!(t.c.editor.buf.text().contains("\n = 1.5"));
            t.ctrl(st, 'z');
            assert_eq!(t.c.editor.buf.text(), doc);
            // Without a selection Shift+Delete deletes the next character and copies nothing.
            t.c.editor.buf.set_caret(0, false);
            let r = shift_key(&mut t, NamedKey::Delete);
            assert!(r.commands.is_empty());
            assert_eq!(t.c.editor.buf.text(), &doc[1..]);
            // Shift+Insert asks the app for the clipboard, pasted at the caret.
            let r = shift_key(&mut t, NamedKey::Insert);
            assert_eq!(r.commands, [Command::EditorPaste]);
            t.c.paste_text("[");
            assert_eq!(t.c.editor.buf.text(), doc);
            // Nothing selected: Ctrl+Insert copies nothing.
            let r = t.key(st, Key::Named(NamedKey::Insert), Mods::CTRL);
            assert!(r.commands.is_empty());
        });
    }

    /// The primary selections a response offers.
    fn offered(cmds: &[Command]) -> Vec<String> {
        cmds.iter()
            .filter_map(|c| match c {
                Command::PrimaryCopy(s) => Some(s.clone()),
                _ => None,
            })
            .collect()
    }

    /// Where byte `at` of the text is drawn (it must be).
    fn point_of(t: &T, at: usize) -> (f32, f32) {
        let ed = &t.c.editor;
        let l = ed.buf.line_of(at);
        let g = &ed.geom;
        let (_, b) = g.lines.iter().find(|(n, _)| *n == l).expect("drawn");
        (
            g.x0 + b.caret_x(at - ed.buf.line_range(l).start) + 0.25,
            g.y0 + (l as f32 + 0.5) * g.line_h,
        )
    }

    /// The text's frame as last drawn (its view is 1 px inside).
    fn text_area(t: &T) -> Rect {
        t.c.hits
            .iter()
            .find(|(_, h)| *h == Target::Editor(EditorTarget::Text))
            .map(|(r, _)| *r)
            .expect("the text is drawn")
    }

    /// A frame at `now`: what it asked for next.
    fn frame_at(t: &mut T, st: &ViewerState, now: f64) -> Option<f64> {
        t.now = now;
        let mut dl = DrawList::new(1.0);
        t.c.draw(&mut dl, &mut t.text, st, &Palette::dark(), now)
    }

    fn primary_button(t: &mut T, st: &ViewerState, p: (f32, f32), down: bool) -> Response {
        let ev = Input::PointerButton {
            x: p.0,
            y: p.1,
            button: Button::Primary,
            down,
        };
        t.c.input(&ev, st, t.now)
    }

    /// Screen readers get the text as one run per line (with its break) and its selection
    /// as positions in those runs, and set it the same way: backwards, across lines, at a
    /// run's end (the next line's start), in right-to-left text. What they select is offered
    /// as the primary selection; a position past the last line is refused.
    #[test]
    fn screen_readers_see_and_set_the_selection_by_lines() {
        use crate::a11y::editor_line_id as run;
        use accesskit::{
            Action as AkAction, ActionData, ActionRequest, NodeId, TextPosition, TextSelection,
            TreeId,
        };
        let store = SettingsStore::in_memory();
        with_store(&store, |st| {
            let doc = "[viewing]\ntitle = \"שלום עולם\"\nzoom_step = 1.5\n";
            let mut t = T::new();
            t.c.primary_selection = true;
            t.c.open_config_editor_with(st.store, Some(doc));
            t.draw(st);
            let field = NodeId(7);
            let tree = t.c.accessibility_tree(st, "x", 1.0);
            let node = |id: NodeId| {
                tree.nodes
                    .iter()
                    .find(|(i, _)| *i == id)
                    .map(|(_, n)| n.clone())
            };
            let f = node(field).expect("the text is the field");
            assert_eq!(tree.focus, field);
            assert!(f.supports_action(AkAction::SetTextSelection));
            assert_eq!(f.children(), (0..4).map(run).collect::<Vec<_>>());
            let lines = [
                "[viewing]\n",
                "title = \"שלום עולם\"\n",
                "zoom_step = 1.5\n",
                "",
            ];
            for (l, want) in lines.iter().enumerate() {
                let n = node(run(l)).expect("a run per line");
                assert_eq!(n.role(), accesskit::Role::TextRun);
                assert_eq!(n.value(), Some(*want));
                assert_eq!(n.character_lengths().len(), want.chars().count());
                assert!(n.bounds().is_some(), "drawn, so placed");
            }
            let sel_of = |t: &mut T| {
                t.draw(st);
                let tree = t.c.accessibility_tree(st, "x", 1.0);
                let f = tree.nodes.iter().find(|(i, _)| *i == field).unwrap();
                let s = f.1.text_selection().unwrap();
                (
                    (s.anchor.node, s.anchor.character_index),
                    (s.focus.node, s.focus.character_index),
                )
            };
            let set = |t: &mut T, target: NodeId, a: (usize, usize), f: (usize, usize)| {
                let at = |(l, i)| TextPosition {
                    node: run(l),
                    character_index: i,
                };
                let req = ActionRequest {
                    action: AkAction::SetTextSelection,
                    target_tree: TreeId::ROOT,
                    target_node: target,
                    data: Some(ActionData::SetTextSelection(TextSelection {
                        anchor: at(a),
                        focus: at(f),
                    })),
                };
                t.c.accessibility_action(&req, st)
            };
            // Backwards over a right-to-left word: "עולם" is characters 14..18 of line 1.
            let r = set(&mut t, field, (1, 18), (1, 14));
            assert!(r.consumed);
            assert_eq!(t.c.editor.buf.selected_text(), "עולם");
            let b = &t.c.editor.buf;
            assert!(b.caret() < b.anchor(), "the caret where it was put");
            assert_eq!(offered(&r.commands), ["עולם"]);
            assert_eq!(sel_of(&mut t), ((run(1), 18), (run(1), 14)));
            // Across lines, from a run's end: that is the next line's start.
            let r = set(&mut t, run(0), (0, 10), (2, 4));
            assert!(r.consumed);
            assert_eq!(
                t.c.editor.buf.selected_text(),
                "title = \"שלום עולם\"\nzoom"
            );
            assert_eq!(sel_of(&mut t), ((run(1), 0), (run(2), 4)));
            // A character past a line's end is its end.
            assert!(set(&mut t, field, (1, 9), (2, 99)).consumed);
            assert_eq!(
                t.c.editor.buf.selected_text(),
                "שלום עולם\"\nzoom_step = 1.5\n"
            );
            assert_eq!(sel_of(&mut t), ((run(1), 9), (run(3), 0)));
            // A line that isn't there: nothing changes.
            let r = set(&mut t, field, (0, 0), (9, 0));
            assert!(!r.consumed);
            assert_eq!(sel_of(&mut t), ((run(1), 9), (run(3), 0)));
            // A caret, at the end of the right-to-left line.
            assert!(set(&mut t, run(1), (1, 19), (1, 19)).consumed);
            assert_eq!(t.c.editor.buf.caret(), doc.find("\"\nzoom").unwrap() + 1);
            assert_eq!(sel_of(&mut t), ((run(1), 19), (run(1), 19)));
        });
    }

    /// Dragging past the bottom or top of the text scrolls it toward the pointer, by a speed
    /// that grows with how far past it is (at most [`AUTOSCROLL_MAX_STEP`] per frame), the
    /// selection following to the line shown at that edge; it stops when the button is let
    /// go. Past the right edge of a long line it scrolls sideways.
    #[test]
    fn dragging_past_an_edge_scrolls() {
        let store = SettingsStore::in_memory();
        with_store(&store, |st| {
            let long = format!("long = \"{}\"\n", "x".repeat(400));
            let doc: String = (0..200)
                .map(|i| format!("key_{i} = {i}\n"))
                .chain([long])
                .collect();
            let mut t = T::new();
            t.c.primary_selection = true;
            t.c.open_config_editor_with(st.store, Some(&doc));
            frame_at(&mut t, st, 10.0);
            let area = text_area(&t);
            let bottom = area.bottom() - 1.0;
            let start = doc.find("key_2 ").unwrap();
            let p = point_of(&t, start);
            primary_button(&mut t, st, p, true);
            frame_at(&mut t, st, 10.0);
            assert_eq!(t.c.editor.scroll, 0.0);
            // 40 px below the text.
            let r = t.c.input(
                &Input::PointerMove {
                    x: p.0,
                    y: bottom + 40.0,
                },
                st,
                10.0,
            );
            assert!(offered(&r.commands).is_empty(), "not while dragging");
            // The first frame starts the clock (and shows the line the caret went to), the
            // next ones scroll by the time passed.
            let wake = frame_at(&mut t, st, 10.0).expect("frames while it scrolls");
            assert!(wake <= 1.0 / 60.0 + 1e-6);
            let s0 = t.c.editor.scroll;
            frame_at(&mut t, st, 10.05);
            let rate = 40.0 * AUTOSCROLL_RATE;
            let near = |a: f32, b: f32| (a - b).abs() < 0.01;
            assert!(near(t.c.editor.scroll, s0 + rate * 0.05));
            // A late frame steps no more than the longest step.
            frame_at(&mut t, st, 11.0);
            let s = s0 + rate * (0.05 + AUTOSCROLL_MAX_STEP as f32);
            assert!(near(t.c.editor.scroll, s));
            // Further past, faster.
            t.c.input(
                &Input::PointerMove {
                    x: p.0,
                    y: bottom + 80.0,
                },
                st,
                11.0,
            );
            frame_at(&mut t, st, 11.05);
            let s = t.c.editor.scroll;
            frame_at(&mut t, st, 11.1);
            assert!(near(t.c.editor.scroll, s + 80.0 * AUTOSCROLL_RATE * 0.05));
            // The selection runs from where the press was to the last line shown.
            let ed = &t.c.editor;
            let last = ed.geom.lines.last().unwrap().0;
            assert_eq!(ed.buf.anchor(), start);
            assert_eq!(ed.buf.line_of(ed.buf.caret()), last);
            // Up past the top: back up.
            let top = area.y + 1.0;
            t.c.input(
                &Input::PointerMove {
                    x: p.0,
                    y: top - 20.0,
                },
                st,
                11.1,
            );
            frame_at(&mut t, st, 11.15);
            let s = t.c.editor.scroll;
            frame_at(&mut t, st, 11.2);
            let s3 = s - 20.0 * AUTOSCROLL_RATE * 0.05;
            assert!(near(t.c.editor.scroll, s3));
            let ed = &t.c.editor;
            let first = ed.geom.lines.first().unwrap().0;
            assert_eq!(ed.buf.line_of(ed.buf.caret()), first);
            assert_eq!(ed.buf.anchor(), start);
            // Let go: it stops, and offers what it selected.
            let r = primary_button(&mut t, st, (p.0, top - 20.0), false);
            assert_eq!(offered(&r.commands).len(), 1);
            frame_at(&mut t, st, 11.5);
            assert!(near(t.c.editor.scroll, s3));
            assert_eq!(t.c.editor.autoscroll_at, None);
            // Sideways along the long last line.
            t.ctrl(st, 'a');
            t.key(st, Key::Named(NamedKey::End), Mods::CTRL);
            t.key(st, Key::Named(NamedKey::ArrowUp), Mods::NONE);
            t.key(st, Key::Named(NamedKey::Home), Mods::NONE);
            frame_at(&mut t, st, 12.0);
            assert_eq!(t.c.editor.scroll_x, 0.0);
            let lstart = doc.find("long").unwrap();
            let p = point_of(&t, lstart);
            primary_button(&mut t, st, p, true);
            let right = area.right() - 1.0;
            t.c.input(
                &Input::PointerMove {
                    x: right + 30.0,
                    y: p.1,
                },
                st,
                12.0,
            );
            frame_at(&mut t, st, 12.0);
            let sx = t.c.editor.scroll_x;
            frame_at(&mut t, st, 12.1);
            assert!(near(t.c.editor.scroll_x, sx + 30.0 * AUTOSCROLL_RATE * 0.1));
            let ed = &t.c.editor;
            assert_eq!(ed.buf.anchor(), lstart);
            assert!(ed.buf.caret() > lstart + 20, "the selection follows");
            // Not past the end of the line.
            for i in 0..100 {
                frame_at(&mut t, st, 12.2 + i as f64 * 0.1);
            }
            let sx = t.c.editor.scroll_x;
            frame_at(&mut t, st, 30.0);
            assert_eq!(t.c.editor.scroll_x, sx, "it stopped at the end");
            assert_eq!(t.c.editor.autoscroll_at, None);
            assert_eq!(t.c.editor.buf.caret(), doc.len() - 1);
            primary_button(&mut t, st, (right + 30.0, p.1), false);
        });
    }

    /// The wheel turned during a drag scrolls, and the selection goes on to what is then
    /// under the pointer.
    #[test]
    fn the_wheel_during_a_drag_extends_the_selection() {
        let store = SettingsStore::in_memory();
        with_store(&store, |st| {
            let doc: String = (0..200).map(|i| format!("key_{i} = {i}\n")).collect();
            let mut t = T::new();
            t.c.open_config_editor_with(st.store, Some(&doc));
            frame_at(&mut t, st, 10.0);
            let start = doc.find("key_2 ").unwrap();
            let p = point_of(&t, start + 2);
            primary_button(&mut t, st, p, true);
            frame_at(&mut t, st, 10.0);
            let wheel = |t: &mut T, dy: f32| {
                let ev = Input::Wheel {
                    x: p.0,
                    y: p.1,
                    dx: 0.0,
                    dy,
                };
                assert!(t.c.input(&ev, st, t.now).consumed);
            };
            // Three lines' worth down: the pointer is over a line further on.
            wheel(&mut t, -1.0);
            frame_at(&mut t, st, 10.05);
            let g = &t.c.editor.geom;
            let scroll = t.c.editor.scroll;
            assert_eq!(scroll, 48.0);
            let l = ((p.1 - g.y0) / g.line_h).floor() as usize;
            assert!(l > 2, "a line further on: {l}");
            let ed = &t.c.editor;
            assert_eq!(ed.buf.anchor(), start + 2);
            assert_eq!(ed.buf.caret(), ed.buf.line_range(l).start + 2);
            // And back: the selection shrinks again.
            wheel(&mut t, 1.0);
            frame_at(&mut t, st, 10.1);
            assert_eq!(t.c.editor.scroll, 0.0);
            let b = &t.c.editor.buf;
            assert_eq!((b.anchor(), b.caret()), (start + 2, start + 2));
            // Without a drag the wheel only scrolls.
            primary_button(&mut t, st, p, false);
            wheel(&mut t, -1.0);
            frame_at(&mut t, st, 10.15);
            let b = &t.c.editor.buf;
            assert_eq!((b.anchor(), b.caret()), (start + 2, start + 2));
        });
    }

    /// The find field is a text field: Ctrl+F seeds it from the selection, selected, so
    /// typing replaces it; the caret keys and Shift select, the clipboard goes through the
    /// app, Ctrl+Z undoes, and the search follows every edit; Enter, Up and Down still step
    /// through the matches. A click puts its caret (and gives it the keyboard), and screen
    /// readers see and set its selection.
    #[test]
    fn find_field_is_a_text_field() {
        use crate::Button;
        let store = SettingsStore::in_memory();
        with_store(&store, |st| {
            let mut t = T::new();
            t.c.primary_selection = false;
            let doc = "[viewing]\ninitial_zoom = \"fit\"\nzoom_step = 1.5\n";
            t.c.open_config_editor_with(st.store, Some(doc));
            t.draw(st);
            let query = |t: &T| t.c.editor.find.as_ref().unwrap().query.clone();
            // "zoom" of initial_zoom selected: Ctrl+F seeds the field with it, selected.
            t.c.editor.buf.select(18..22);
            t.ctrl(st, 'f');
            assert_eq!(query(&t), "zoom");
            assert_eq!(t.c.field_edit().unwrap().selected_text(), "zoom");
            // Typing replaces it, and the search follows.
            t.type_text(st, "step");
            assert_eq!(query(&t), "step");
            let at = doc.find("step").unwrap();
            assert_eq!(t.c.editor.buf.selection(), at..at + 4);
            // Shift+Home selects; Ctrl+C asks the app to copy; Backspace deletes it.
            t.named(st, NamedKey::ArrowLeft);
            t.key(st, Key::Named(NamedKey::Home), Mods::SHIFT);
            assert_eq!(t.c.field_edit().unwrap().selected_text(), "ste");
            let r = t.ctrl(st, 'c');
            assert_eq!(r.commands, vec![Command::EditorCopy("ste".into())]);
            t.named(st, NamedKey::Backspace);
            assert_eq!(query(&t), "p");
            t.ctrl(st, 'z');
            assert_eq!(query(&t), "step");
            // A paste over everything, as one line.
            t.ctrl(st, 'a');
            assert_eq!(t.ctrl(st, 'v').commands, vec![Command::EditorPaste]);
            t.c.paste_text("zoom\r\n");
            assert_eq!(query(&t), "zoom ");
            t.named(st, NamedKey::Backspace);
            assert_eq!(query(&t), "zoom");
            // Enter, Down and Up step through the matches.
            let m = t.c.editor.matches();
            assert_eq!(m.len(), 2);
            let first = t.c.editor.buf.selection();
            assert!(m.contains(&first));
            t.named(st, NamedKey::Enter);
            let second = t.c.editor.buf.selection();
            assert!(m.contains(&second) && second != first);
            t.named(st, NamedKey::ArrowUp);
            assert_eq!(t.c.editor.buf.selection(), first);
            assert_eq!(query(&t), "zoom");
            // The screen reader sees the field's selection, and sets it.
            t.key(st, Key::Named(NamedKey::Home), Mods::SHIFT);
            let tree = t.c.accessibility_tree(st, "x", 1.0);
            let focus = tree.nodes.iter().find(|(id, _)| *id == tree.focus).unwrap();
            assert_eq!(focus.1.role(), accesskit::Role::SearchInput);
            let sel = focus.1.text_selection().expect("a selection");
            assert_eq!(
                (sel.anchor.character_index, sel.focus.character_index),
                (4, 0)
            );
            // From the text, a click in the field gives it the keyboard and puts the caret.
            t.c.editor.region = Region::Text;
            t.draw(st);
            let g = t.c.field_geom.clone().expect("the find field is drawn");
            let (b, x0) = g.text.clone().expect("its text is drawn");
            for down in [true, false] {
                let ev = Input::PointerButton {
                    x: x0 + b.caret_x(2) + 0.5,
                    y: g.rect.center().1,
                    button: Button::Primary,
                    down,
                };
                t.c.input(&ev, st, t.now);
            }
            assert_eq!(t.c.editor.region, Region::Find);
            assert_eq!(t.c.field_edit().unwrap().caret(), 2);
            // Typing goes there.
            t.type_text(st, "_");
            assert_eq!(query(&t), "zo_om");
        });
    }

    #[test]
    fn a_problem_is_announced_when_the_caret_enters_it() {
        let store = SettingsStore::in_memory();
        with_store(&store, |st| {
            let mut t = T::new();
            t.c.open_config_editor_with(st.store, Some("[viewing]\nbogus = 1\n"));
            t.draw(st);
            t.named(st, NamedKey::ArrowDown);
            assert!(t.announced().contains("bogus"), "{}", t.announced());
        });
    }

    #[test]
    fn typing_and_checking_3000_lines_stays_quick() {
        let mut text = String::new();
        let sections = ["viewing", "browsing", "gallery", "advanced"];
        for i in 0..3000 {
            if i % 50 == 0 {
                text.push_str(&format!("[{}]\n", sections[(i / 50) % 4]));
            } else {
                text.push_str(&format!(
                    "# line {i} with \"quotes\", [brackets] and = signs\n"
                ));
            }
        }
        let store = SettingsStore::in_memory();
        with_store(&store, |st| {
            let mut t = T::new();
            t.c.open_config_editor_with(st.store, Some(&text));
            t.draw(st);
            t.key(st, Key::Named(NamedKey::End), Mods::CTRL);
            let start = std::time::Instant::now();
            for _ in 0..20 {
                t.type_text(st, "x");
            }
            let typing = start.elapsed() / 20;
            t.c.editor.buf.insert("y", EditKind::Type);
            let start = std::time::Instant::now();
            t.c.editor.relint();
            let lint = start.elapsed();
            eprintln!("3000 lines: {typing:?} per typed key and frame, check {lint:?}");
            assert!(typing.as_millis() < 100, "{typing:?}");
        });
    }
    // ----- keymap.toml ------------------------------------------------------------------

    /// 1-based line and column (in characters) of byte `at`.
    fn line_col(text: &str, at: usize) -> (usize, usize) {
        let before = &text[..at];
        let line = before.matches('\n').count() + 1;
        let col = before[before.rfind('\n').map_or(0, |i| i + 1)..]
            .chars()
            .count()
            + 1;
        (line, col)
    }

    /// The one problem of `text` whose message contains `needle`: its severity and place.
    fn keymap_lint(text: &str, needle: &str) -> (Severity, (usize, usize), String) {
        let found: Vec<Lint> = KeymapLinter
            .lint(text)
            .into_iter()
            .filter(|l| l.message.contains(needle))
            .collect();
        assert_eq!(
            found.len(),
            1,
            "{needle:?} in {:?}",
            KeymapLinter.lint(text)
        );
        let l = &found[0];
        let msg = l.message.replace(['\u{2068}', '\u{2069}'], "");
        (l.severity, line_col(text, l.span.start), msg)
    }

    #[test]
    fn keymap_linter_reports_each_problem_where_it_is() {
        use Severity::{Error, Warning};
        // Not TOML at all.
        let (sev, (line, _), _) = keymap_lint("[viewer]\n\"a\" = \n", "could not be read");
        assert_eq!((sev, line), (Error, 2));
        // An unknown table, with the mode it was probably meant to be.
        let text = "[viewer]\n\"x\" = \"next\"\n\n[veiwer]\n\"a\" = \"next\"\n";
        let (sev, at, msg) = keymap_lint(text, "veiwer");
        assert_eq!((sev, at), (Error, (4, 2)));
        assert!(msg.contains("[viewer]"), "{msg}");
        // A key that is no key.
        let text = "[viewer]\n\"x\" = \"next\"\n\"Hyper+y\" = \"prev\"\n";
        assert_eq!(keymap_lint(text, "Hyper+y").0, Error);
        assert_eq!(keymap_lint(text, "Hyper+y").1, (3, 1));
        // An unknown action, with the closest one.
        let text = "[viewer]\n\"x\" = \"zoom-inn\"\n";
        let (sev, at, msg) = keymap_lint(text, "zoom-inn");
        assert_eq!((sev, at), (Error, (2, 7)));
        assert!(msg.contains("“zoom-in”"), "{msg}");
        // Far from every action: no suggestion.
        let (_, _, msg) = keymap_lint("[viewer]\n\"x\" = \"qqqqqqqqqq\"\n", "qqqqqqqqqq");
        assert!(!msg.contains('?'), "{msg}");
        // An unknown preset.
        let (sev, at, _) = keymap_lint("preset = \"nope\"\n", "nope");
        assert_eq!((sev, at), (Error, (1, 10)));
        // The same keys twice in a mode: an error (the Keys page never keeps both).
        let text = "[viewer]\n\"Ctrl+x\" = \"next\"\n\"ctrl+x\" = \"prev\"\n";
        let (sev, at, _) = keymap_lint(text, "twice");
        assert_eq!((sev, at), (Error, (3, 1)));
        // A key that begins a sequence: a warning (the Keys page can keep both).
        let text = "[viewer]\n\"g\" = \"next\"\n\"g g\" = \"first\"\n";
        let (sev, at, _) = keymap_lint(text, "begins");
        assert_eq!(sev, Warning);
        assert_eq!(at, (2, 1));
        // A key the system takes: a warning (the Keys page binds it anyway when asked).
        if !cfg!(target_os = "macos") {
            let text = "[viewer]\n\"x\" = \"next\"\n\"Alt+F4\" = \"prev\"\n";
            let (sev, at, msg) = keymap_lint(text, "never reach");
            assert_eq!((sev, at), (Warning, (3, 1)));
            assert!(msg.contains("closes the window"), "{msg}");
        }
    }

    #[test]
    fn exported_keymaps_lint_clean_and_round_trip() {
        use specular_input::{KeySeq, Mode, Preset};
        let default = Keymap::viewer_default();
        assert_eq!(KeymapLinter.lint(&default.to_toml()), vec![]);
        // Changed: another preset, a key moved, a sequence and a cull binding.
        let mut km = Keymap::viewer_default();
        km.apply_edit(&specular_input::KeymapEdit::Preset(Preset::ALL[1]));
        let seq = |s: &str| KeySeq::parse(s).unwrap();
        km.apply_edit(&specular_input::KeymapEdit::Bind {
            mode: Mode::Viewer,
            keys: seq("Ctrl+Shift+F9"),
            action: Action::NextImage,
            old: None,
            displace: specular_input::Displace::Replace,
        });
        km.apply_edit(&specular_input::KeymapEdit::Bind {
            mode: Mode::Viewer,
            keys: seq("z x"),
            action: Action::ZoomFit,
            old: None,
            displace: specular_input::Displace::Replace,
        });
        let text = km.to_toml();
        assert_eq!(KeymapLinter.lint(&text), vec![], "{text}");
        let (back, problems) = Keymap::from_toml(&text);
        assert!(problems.is_empty());
        assert!(back.same_bindings(&km));
        assert_eq!(back.to_toml(), text);
        for m in Mode::ALL {
            assert_eq!(KeymapLinter.lint(&km.mode_to_toml(m)), vec![]);
        }
    }

    #[test]
    fn keymap_save_is_refused_with_errors_and_handed_to_the_app_when_clean() {
        let store = SettingsStore::in_memory();
        let km = Keymap::viewer_default();
        with_keymap(&store, &km, |st| {
            let mut t = T::new();
            t.c.set_keymap_file(Some(std::path::Path::new("/cfg/keymap.toml")), None);
            t.c.open_keymap_editor_with(st.keymap, Some("[viewer]\n\"x\" = \"bogus\"\n"));
            t.draw(st);
            assert_eq!(t.c.config_editor_file(), Some(EditorFile::Keymap));
            assert_eq!(t.c.editor.errors(), 1);
            let r = t.ctrl(st, 's');
            assert!(r.commands.is_empty(), "{:?}", r.commands);
            assert_eq!(t.c.editor.alert, Some(Alert::CantSave));
            assert_eq!(t.c.editor.region, Region::Problems);
            // Fixed: Save hands the text to the application, which writes and applies it.
            t.c.editor.buf.replace_all("[viewer]\n\"x\" = \"prev\"\n");
            t.settle(st);
            let r = t.ctrl(st, 's');
            let text = "[viewer]\n\"x\" = \"prev\"\n".to_owned();
            assert_eq!(r.commands, vec![Command::KeymapSave(text.clone())]);
            assert!(t.c.editor.modified(), "saved only once the app says so");
            t.c.set_keymap_file(Some(std::path::Path::new("/cfg/keymap.toml")), Some(&text));
            t.c.keymap_saved(Ok(()));
            assert!(!t.c.editor.modified());
            assert!(t.c.editor.alert.is_none());
            assert_eq!(t.announced(), "keymap.toml saved and applied.");
            // Its own save is not taken for a change made elsewhere.
            t.draw(st);
            assert!(t.c.editor.conflict.is_none());
            // A failed write says why and keeps the edits.
            t.esc(st);
            assert_eq!(t.c.editor.region, Region::Text);
            t.key(st, Key::Named(NamedKey::End), Mods::CTRL);
            t.type_text(st, "# x\n");
            t.ctrl(st, 's');
            t.c.keymap_saved(Err("disk full".into()));
            assert_eq!(t.c.editor.alert, Some(Alert::Failed("disk full".into())));
            assert!(t.announced().contains("disk full"));
            assert!(t.c.editor.modified());
        });
    }

    #[test]
    fn keymap_editor_follows_or_asks_about_changes_elsewhere() {
        let store = SettingsStore::in_memory();
        let km = Keymap::viewer_default();
        let path = std::path::Path::new("/cfg/keymap.toml");
        with_keymap(&store, &km, |st| {
            let mut t = T::new();
            // No file yet: the bindings in effect, as exported; saving creates it.
            t.c.set_keymap_file(Some(path), None);
            t.c.open_keymap_editor(st.keymap);
            t.draw(st);
            assert_eq!(t.c.editor.buf.text(), km.to_toml());
            assert!(!t.c.editor.modified());
            assert_eq!(t.c.editor.path, path.display().to_string());
            // Written elsewhere (the Keys page, a text editor) without edits here: followed.
            t.c.set_keymap_file(Some(path), Some("[viewer]\n\"x\" = \"next\"\n"));
            t.draw(st);
            assert_eq!(t.c.editor.buf.text(), "[viewer]\n\"x\" = \"next\"\n");
            assert!(t.c.editor.conflict.is_none());
            // With edits: the reload bar.
            t.key(st, Key::Named(NamedKey::End), Mods::CTRL);
            t.type_text(st, "# mine\n");
            t.c.set_keymap_file(Some(path), Some("[viewer]\n\"y\" = \"next\"\n"));
            t.draw(st);
            assert!(t.c.editor.conflict.is_some() && t.c.editor.conflict_shown);
            let r = t.ctrl(st, 's');
            assert!(r.commands.is_empty(), "saving waits for an answer");
            assert_eq!(t.c.modal_focus, Some(Target::Editor(EditorTarget::Reload)));
            assert_eq!(t.announced(), "keymap.toml changed elsewhere.");
            t.c.activate(Target::Editor(EditorTarget::Reload), st);
            assert_eq!(t.c.editor.buf.text(), "[viewer]\n\"y\" = \"next\"\n");
            assert!(!t.c.editor.modified());
        });
    }

    #[test]
    fn keymap_editor_opens_from_the_keys_page_and_esc_returns_there() {
        use crate::settings::SettingsTarget as S;
        let store = SettingsStore::in_memory();
        let km = Keymap::viewer_default();
        with_keymap(&store, &km, |st| {
            let mut t = T::new();
            t.c.open_settings_at(crate::keys_edit::KEY);
            let section = t.c.settings.section;
            t.draw(st);
            // The button, named for the file.
            let tree = t.c.accessibility_tree(st, "x", 1.0);
            let label = |tree: &accesskit::TreeUpdate, l: &str| {
                tree.nodes.iter().any(|(_, n)| n.label() == Some(l))
            };
            assert!(label(&tree, "Edit keymap.toml"));
            let focus = Some(Target::Settings(S::KeyEditFile));
            t.c.modal_focus = focus.clone();
            t.named(st, NamedKey::Enter);
            assert_eq!(t.c.config_editor_file(), Some(EditorFile::Keymap));
            // The dialog and its text are named for keymap.toml.
            let tree = t.c.accessibility_tree(st, "x", 1.0);
            assert!(label(&tree, "keymap.toml"));
            assert!(label(&tree, "Key bindings file"));
            assert!(!label(&tree, "settings.toml"));
            // Esc: back on the Keys page, on the button.
            t.esc(st);
            assert_eq!(t.c.modal, Some(Modal::Settings));
            assert_eq!(t.c.settings.section, section);
            assert_eq!(t.c.modal_focus, focus);
            // From the palette: Esc closes it.
            t.c.close_modal();
            t.c.open_modal(Modal::Palette);
            t.type_text(st, "keymap.toml");
            let i =
                t.c.palette
                    .items
                    .iter()
                    .position(|e| matches!(e.run, crate::palette::Run::EditKeymapFile))
                    .expect("palette command");
            t.c.palette.selected = i;
            t.named(st, NamedKey::Enter);
            assert_eq!(t.c.config_editor_file(), Some(EditorFile::Keymap));
            t.esc(st);
            assert_eq!(t.c.modal, None);
        });
    }

    /// How long checking a large keymap.toml takes (it runs once typing pauses).
    #[test]
    fn checking_a_2000_binding_keymap_stays_quick() {
        let mut text = String::from("[viewer]\n");
        let keys = "abcdefghijklmnopqrstuvwxyz0123456789";
        let mods = [
            "",
            "Ctrl+",
            "Alt+",
            "Shift+",
            "Ctrl+Alt+",
            "Ctrl+Shift+",
            "Alt+Shift+",
        ];
        let mut n = 0;
        'all: for a in keys.chars() {
            for b in keys.chars() {
                for m in mods {
                    if n == 2000 {
                        break 'all;
                    }
                    text.push_str(&format!("\"{m}{a} {b}\" = \"next\"\n"));
                    n += 1;
                }
            }
        }
        let start = std::time::Instant::now();
        let runs = 5;
        for _ in 0..runs {
            std::hint::black_box(KeymapLinter.lint(&text));
        }
        let each = start.elapsed() / runs;
        let start = std::time::Instant::now();
        let problems = Keymap::check(&text).len();
        let check = start.elapsed();
        eprintln!(
            "keymap.toml with {n} bindings: lint {each:?} ({problems} problems; the loader's \
             check {check:?})"
        );
        specular_log::timing::assert_within(
            "linting a 2000-binding keymap",
            each,
            std::time::Duration::from_millis(149),
        );
    }
}
