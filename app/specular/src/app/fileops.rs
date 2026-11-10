//! File operations from the viewer and the gallery: rename (F2) with its sidecars following,
//! batch rename from a pattern (Shift+F2), copy and move to a folder (F8, F7) with recent
//! folders and a choice for names that are taken, permanent rotation (Alt+[ and Alt+]), undo
//! and redo of all of them (Ctrl+Z; Ctrl+Shift+Z or Ctrl+Y), sharing, sending by email, and
//! exporting and importing the settings with a preview of what would change.
//!
//! The work itself is specular-fileops' and runs on background workers; results come back
//! through this module's channel, drained once per event-loop turn ([`App::fileops_tick`]).
//! One operation that changes files runs at a time, so the journal stays in order.

#[cfg(test)]
mod shots;
mod view;

/// Develop's sheet shots draw through it.
#[cfg(test)]
pub(super) use self::view::draw as draw_sheet;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crossbeam_channel::{Receiver, Sender};
use specular_convert::{NameFacts, Pattern, Probe};
use specular_fileops::batch::{self, Row};
use specular_fileops::history::{self, Kind};
use specular_fileops::names::{self, NameProblem, Platform};
use specular_fileops::orient::{self, Method, RotateError, RotateOptions, Rotated, TagReason};
use specular_fileops::transfer::{self, Conflict, Item, Mode, Outcome, Progress};
use specular_fileops::{History, Op, Step, Transform, Trash, UndoError};
use specular_gfx::{DrawList, Rect, Text};
use specular_i18n::{FluentArgs, I18n};
use specular_input::{Action, Key, KeyCombo, Mode as KeyMode, NamedKey};
use specular_settings::{ImportPreview, Origin, Warning};
use specular_theme::Palette;
use specular_widgets::accesskit;
use specular_widgets::line_edit::{LineEdit, PendingClipboard};

pub(super) use self::view::{ButtonView, ChoiceView, Drawn, FieldView, Part, RowView, SheetView};
use super::App;
use super::integration::PasteTo;

/// Folders remembered for copy and move.
const MAX_RECENT: usize = 8;
/// Toast button keys of this module: stopping a copy or move, and the offers to rotate a
/// JPEG losslessly by trimming its edge (by the token of the tag rotation they replace).
const CANCEL_KEY: u64 = 1 << 44;
const TRIM_BASE: u64 = 1 << 45;
/// Seconds between progress updates of a running copy or move.
const PROGRESS_EVERY: f64 = 0.25;

/// The system trash, for "replace" and for undoing a copy.
struct SystemTrash;

impl Trash for SystemTrash {
    fn trash(&self, path: &Path) -> std::io::Result<()> {
        trash::delete(path).map_err(|e| std::io::Error::other(crate::error_text::trash_reason(&e)))
    }

    fn restore(&self, path: &Path) -> std::io::Result<()> {
        crate::clipboard::restore_now(path).map_err(std::io::Error::other)
    }
}

/// What came back from a worker.
enum Msg {
    /// A rename or batch rename ended.
    Renamed {
        token: u64,
        result: Result<Op, String>,
    },
    Progress(Progress),
    Transferred {
        token: u64,
        mode: Mode,
        folder: PathBuf,
        outcome: Outcome,
    },
    Rotated {
        token: u64,
        path: PathBuf,
        turn: Transform,
        /// The display cache key the file had (stale when the rotation kept size and date).
        before: Option<crate::display_cache::Key>,
        result: Result<Rotated, RotateError>,
    },
    /// An undo (or redo) ended; `op` is the operation as it was, for putting back.
    Undone {
        token: u64,
        redo: bool,
        op: Op,
        result: Result<Op, UndoError>,
    },
    /// The folder dialog of copy / move answered.
    Picked(Option<PathBuf>),
    /// Capture dates and cameras for batch rename (`done`: the last batch).
    Probed {
        list: Vec<(PathBuf, Probe)>,
        done: bool,
    },
    /// The settings file to import, read.
    ImportRead(PathBuf, Result<String, String>),
    Exported(Result<PathBuf, String>),
    /// Sharing or the email program could not start.
    Failed(String),
}

/// The batch rename sheet.
struct Batch {
    files: Vec<PathBuf>,
    field: LineEdit,
    probes: HashMap<PathBuf, Probe>,
    probing: bool,
    /// The rows for the field's text (rebuilt when it or the probes change).
    rows: Vec<Row>,
    rows_for: Option<(String, usize)>,
    top: usize,
}

/// Copy or move to a folder.
struct TransferSheet {
    mode: Mode,
    files: Vec<PathBuf>,
    folders: Vec<PathBuf>,
    chosen: Option<PathBuf>,
    conflict: Conflict,
    /// "Replace" with names taken: asking first.
    confirm: bool,
    picking: bool,
    /// The plan for the chosen folder.
    items: Vec<Item>,
}

struct ImportSheet {
    path: PathBuf,
    preview: ImportPreview,
    /// Key binding changes: (action name, keys before, keys after).
    keys: Vec<(String, String, String)>,
    key_problems: Vec<String>,
    top: usize,
}

enum Kind2 {
    Rename {
        file: PathBuf,
        field: Box<LineEdit>,
    },
    Batch(Box<Batch>),
    Transfer(Box<TransferSheet>),
    Import(Box<ImportSheet>),
    /// One of Develop's sheets (`develop/sheet.rs`): drawn and answered here.
    Develop(Box<super::develop::DevelopSheet>),
    /// One of the Library's album sheets (`gallery/albums.rs`).
    Album(Box<super::gallery::albums::AlbumSheet>),
}

struct Sheet {
    kind: Kind2,
    focus: Part,
    drawn: Drawn,
}

/// A copy or move running in the background.
struct Running {
    cancel: Arc<AtomicBool>,
    toast: Option<u64>,
    toast_at: f64,
    last: Option<Progress>,
    mode: Mode,
    count: usize,
}

pub(super) struct FileOps {
    history: History,
    sheet: Option<Sheet>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    /// An operation that changes files (or its undo) is running.
    busy: bool,
    running: Option<Running>,
    /// Tag rotations whose toast offers the lossless rotation with trimming: by toast key,
    /// the token, the file and the turn.
    trim_offers: HashMap<u64, (u64, PathBuf, Transform)>,
    /// The last batch rename pattern (this session).
    pattern: String,
    /// What an input method is composing in the field (IME preedit).
    preedit: String,
}

impl Default for FileOps {
    fn default() -> FileOps {
        let (tx, rx) = crossbeam_channel::unbounded();
        FileOps {
            history: History::default(),
            sheet: None,
            tx,
            rx,
            busy: false,
            running: None,
            trim_offers: HashMap::new(),
            pattern: "{name}".to_owned(),
            preedit: String::new(),
        }
    }
}

impl FileOps {
    pub(super) fn is_open(&self) -> bool {
        self.sheet.is_some()
    }

    pub(super) fn wants_text(&self) -> bool {
        self.sheet.as_ref().is_some_and(|s| s.focus == Part::Field)
    }

    pub(super) fn draw(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        size: (f32, f32),
        rtl: bool,
        ts: f32,
    ) -> Option<SheetView> {
        let v = self.view()?;
        let s = self.sheet.as_mut()?;
        s.drawn = view::draw(dl, text, pal, &v, size, rtl, ts);
        Some(v)
    }

    pub(super) fn a11y(&self, tree: &mut accesskit::TreeUpdate, v: &SheetView, scale: f64) {
        if let Some(s) = &self.sheet {
            view::a11y(tree, v, &s.drawn, scale);
        }
    }

    fn view(&self) -> Option<SheetView> {
        let s = self.sheet.as_ref()?;
        let mut v = match &s.kind {
            Kind2::Rename { file, field } => rename_view(file, field, s.focus),
            Kind2::Batch(b) => batch_view(b, s.focus),
            Kind2::Transfer(t) => transfer_view(t, s.focus),
            Kind2::Import(i) => import_view(i, s.focus),
            Kind2::Develop(d) => d.view(s.focus),
            Kind2::Album(a) => a.view(s.focus),
        };
        if let Some(f) = v.field.as_mut().filter(|_| s.focus == Part::Field) {
            f.preedit.clone_from(&self.preedit);
        }
        Some(v)
    }

    /// What the input method composes in the field (empty: nothing).
    pub(super) fn set_preedit(&mut self, s: &str) {
        if self.wants_text() {
            s.clone_into(&mut self.preedit);
        } else {
            self.preedit.clear();
        }
    }

    /// Where the input method's window opens while the field has the keyboard: at the
    /// composition and the caret.
    pub(super) fn ime_area(&self) -> Option<Rect> {
        let s = self.sheet.as_ref().filter(|_| self.wants_text())?;
        s.drawn.ime.or_else(|| {
            s.drawn
                .hits
                .iter()
                .find(|(_, p)| *p == Part::Field)
                .map(|h| h.0)
        })
    }
}

// ----- Texts ---------------------------------------------------------------------------

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

fn tr(id: &str) -> String {
    I18n::global().tr(id)
}

fn tr_args(id: &str, args: &[(&str, String)]) -> String {
    let mut a = FluentArgs::new();
    for (k, v) in args {
        // Counts are numbers, so plural forms select on them.
        match v.parse::<i64>() {
            Ok(n) if matches!(*k, "count" | "problems" | "percent" | "width" | "height") => {
                a.set(*k, n)
            }
            _ => a.set(*k, v.clone()),
        }
    }
    I18n::global().tr_args(id, &a)
}

/// Why a name can't be used, as a sentence.
pub(super) fn name_problem_text(p: &NameProblem) -> String {
    match p {
        NameProblem::InvalidChar(c) => {
            let shown = if c.is_control() {
                format!("U+{:04X}", *c as u32)
            } else {
                c.to_string()
            };
            tr_args(p.message_id(), &[("char", shown)])
        }
        NameProblem::Reserved(n) => tr_args(p.message_id(), &[("name", n.clone())]),
        _ => tr(p.message_id()),
    }
}

/// Why an undo or redo can't go ahead.
fn undo_problem_text(e: &UndoError) -> String {
    let mut args = vec![("file", file_name(e.path()))];
    if let UndoError::Io(_, reason) = e {
        args.push(("reason", reason.clone()));
    }
    tr_args(e.message_id(), &args)
}

/// What kind of operation, for "Undid …".
fn kind_text(k: Kind) -> String {
    tr(k.message_id())
}

fn rotate_problem_text(e: &RotateError, name: &str) -> String {
    let mut args = vec![("file", name.to_owned())];
    if let RotateError::Damaged(r) | RotateError::Io(r) = e {
        args.push(("reason", r.clone()));
    }
    tr_args(e.message_id(), &args)
}

/// The toast after a rotation: how it was saved, and for a tag on a JPEG, why.
fn rotated_text(r: &Rotated, name: &str) -> String {
    let mut args = vec![("file", name.to_owned())];
    match r.method {
        Method::Lossless {
            trimmed: Some((w, h)),
        } => {
            args.push(("width", w.to_string()));
            args.push(("height", h.to_string()));
        }
        Method::Sidecar => args.push(("sidecar", file_name(&r.written))),
        _ => {}
    }
    let mut s = tr_args(r.method.message_id(), &args);
    if let Method::Tag {
        reason: Some(reason),
    } = r.method
    {
        let why = match reason {
            TagReason::NotAligned { width, height } => tr_args(
                reason.message_id(),
                &[("width", width.to_string()), ("height", height.to_string())],
            ),
            _ => tr(reason.message_id()),
        };
        s = format!("{s} {why}");
    }
    s
}

/// The extension of a name without the dot.
fn ext_of(name: &str) -> &str {
    names::extension(name).trim_start_matches('.')
}

// ----- Views -----------------------------------------------------------------------------

fn field_view(label: String, f: &LineEdit) -> FieldView {
    let sel = f.selection();
    FieldView {
        label,
        text: f.text().to_owned(),
        caret: f.caret(),
        selection: (sel.start, sel.end),
        preedit: String::new(),
    }
}

fn buttons(ok: String, ok_enabled: bool, cancel: String) -> Vec<ButtonView> {
    vec![
        ButtonView {
            label: ok,
            enabled: ok_enabled,
            default: true,
        },
        ButtonView {
            label: cancel,
            enabled: true,
            default: false,
        },
    ]
}

/// The rename sheet: the name with its stem selected, why it can't be used, and which
/// sidecars follow.
fn rename_view(file: &Path, field: &LineEdit, focus: Part) -> SheetView {
    let old = file_name(file);
    let new = field.text();
    let problem = if new == old {
        None
    } else {
        names::validate(file, new, Platform::current()).err()
    };
    let mut notes = Vec::new();
    let sidecars = names::sidecars(file, true);
    if !sidecars.is_empty() {
        let list: Vec<String> = sidecars.iter().map(|p| file_name(p)).collect();
        notes.push(tr_args(
            "rename-sidecars-follow",
            &[("files", list.join(", "))],
        ));
    }
    if problem.is_none() && new != old && !ext_of(&old).eq_ignore_ascii_case(ext_of(new)) {
        notes.push(tr("rename-extension-changed"));
    }
    SheetView {
        title: tr("rename-title"),
        subtitle: file.parent().map(|d| d.display().to_string()),
        field: Some(field_view(tr("rename-field-label"), field)),
        error: problem.as_ref().map(name_problem_text),
        notes,
        list_label: None,
        rows: Vec::new(),
        multi: false,
        more: None,
        choice: None,
        buttons: buttons(
            tr("rename-button"),
            problem.is_none() && new != old && !new.trim().is_empty(),
            tr("fileop-cancel"),
        ),
        focus,
    }
}

/// The names a pattern gives the batch: `None` with the reason when it doesn't parse.
fn batch_names(b: &Batch, pattern: &str) -> Result<Vec<(PathBuf, String)>, String> {
    let p = Pattern::parse(pattern).map_err(|e| super::convert::pattern_error(&e))?;
    Ok(b.files
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let name = file_name(f);
            let stem = &name[names::stem_range(&name)];
            let probe = b.probes.get(f);
            let camera = probe.and_then(|p| p.2.as_deref());
            let facts = NameFacts {
                stem,
                index: i + 1,
                date: probe.and_then(|p| p.1),
                width: probe.map_or(0, |p| p.0.0),
                height: probe.map_or(0, |p| p.0.1),
                ext: ext_of(&name),
                camera,
                ..Default::default()
            };
            let mut n = p.render(&facts);
            // A file without an extension gets none (`render` appends a dot otherwise).
            if ext_of(&name).is_empty() && !p.has_ext() {
                n = n.trim_end_matches('.').to_owned();
            }
            (f.clone(), n)
        })
        .collect())
}

fn rebuild_rows(b: &mut Batch) {
    let key = (b.field.text().to_owned(), b.probes.len());
    if b.rows_for.as_ref() == Some(&key) {
        return;
    }
    b.rows = match batch_names(b, &key.0) {
        Ok(names_in) => batch::preview(&names_in, Platform::current()),
        Err(_) => Vec::new(),
    };
    b.rows_for = Some(key);
    b.top = b.top.min(b.rows.len().saturating_sub(view::MAX_ROWS));
}

fn batch_view(b: &Batch, focus: Part) -> SheetView {
    let pattern = b.field.text();
    let error = Pattern::parse(pattern)
        .err()
        .map(|e| super::convert::pattern_error(&e));
    let changing = b
        .rows
        .iter()
        .filter(|r| r.problem.is_none() && !r.unchanged())
        .count();
    let problems = b.rows.iter().filter(|r| r.problem.is_some()).count();
    let mut notes = vec![tr("batch-rename-tokens")];
    if error.is_none() {
        notes.push(tr_args(
            "batch-rename-summary",
            &[
                ("count", changing.to_string()),
                ("problems", problems.to_string()),
            ],
        ));
    }
    if b.probing {
        notes.push(tr("batch-rename-reading"));
    }
    let rows: Vec<RowView> = b
        .rows
        .iter()
        .skip(b.top)
        .take(view::MAX_ROWS)
        .map(|r| RowView {
            start: file_name(&r.from),
            end: Some(if r.unchanged() {
                tr("batch-rename-unchanged")
            } else {
                r.new_name.clone()
            }),
            problem: r.problem.as_ref().map(|p| match p {
                batch::RowProblem::Name(n) => name_problem_text(n),
                batch::RowProblem::Duplicate => tr(p.message_id()),
            }),
            quiet: r.unchanged(),
            choosable: false,
            chosen: false,
        })
        .collect();
    let below = b.rows.len().saturating_sub(b.top + rows.len());
    SheetView {
        title: tr_args(
            "batch-rename-title",
            &[("count", b.files.len().to_string())],
        ),
        subtitle: None,
        field: Some(field_view(tr("batch-rename-field-label"), &b.field)),
        error,
        notes,
        list_label: Some(tr("batch-rename-preview")),
        rows,
        multi: false,
        more: (below > 0).then(|| tr_args("batch-rename-more", &[("count", below.to_string())])),
        choice: None,
        buttons: buttons(tr("rename-button"), changing > 0, tr("fileop-cancel")),
        focus,
    }
}

/// The rows of the copy / move sheet: the recent folders, then "Choose a folder…".
fn transfer_rows(t: &TransferSheet) -> Vec<RowView> {
    let mut rows: Vec<RowView> = t
        .folders
        .iter()
        .map(|f| RowView {
            start: file_name(f),
            end: f.parent().map(|p| p.display().to_string()),
            problem: None,
            quiet: true,
            choosable: true,
            chosen: t.chosen.as_ref() == Some(f),
        })
        .collect();
    let other = t.chosen.as_ref().filter(|c| !t.folders.contains(c));
    rows.push(RowView {
        start: match other {
            Some(c) => file_name(c),
            None => tr("transfer-choose-folder"),
        },
        end: other
            .and_then(|c| c.parent())
            .map(|p| p.display().to_string()),
        problem: None,
        quiet: true,
        choosable: true,
        chosen: other.is_some(),
    });
    rows
}

fn transfer_view(t: &TransferSheet, focus: Part) -> SheetView {
    let n = t.files.len();
    let copy = t.mode == Mode::Copy;
    let taken = transfer::conflicts(&t.items);
    let folder = t.chosen.as_ref().map(|c| file_name(c)).unwrap_or_default();
    if t.confirm {
        return SheetView {
            title: tr_args(
                "transfer-replace-title",
                &[("count", taken.to_string()), ("folder", folder)],
            ),
            subtitle: None,
            field: None,
            error: None,
            notes: vec![tr("transfer-replace-note")],
            list_label: None,
            rows: Vec::new(),
            multi: false,
            more: None,
            choice: None,
            buttons: buttons(tr("transfer-replace"), true, tr("transfer-back")),
            focus,
        };
    }
    let title = match (copy, n) {
        (true, 1) => tr_args(
            "transfer-copy-title-one",
            &[("file", file_name(&t.files[0]))],
        ),
        (true, _) => tr_args("transfer-copy-title", &[("count", n.to_string())]),
        (false, 1) => tr_args(
            "transfer-move-title-one",
            &[("file", file_name(&t.files[0]))],
        ),
        (false, _) => tr_args("transfer-move-title", &[("count", n.to_string())]),
    };
    let mut notes = Vec::new();
    let sidecars: usize = t.items.iter().map(|i| i.sidecars.len()).sum();
    if sidecars > 0 {
        notes.push(tr_args(
            "transfer-sidecars",
            &[("count", sidecars.to_string())],
        ));
    }
    if taken > 0 {
        notes.push(tr_args(
            "transfer-taken",
            &[("count", taken.to_string()), ("folder", folder)],
        ));
    }
    let ok = if copy {
        tr("transfer-copy")
    } else {
        tr("transfer-move")
    };
    SheetView {
        title,
        subtitle: None,
        field: None,
        error: None,
        notes,
        list_label: Some(if t.folders.is_empty() {
            tr("transfer-folder")
        } else {
            tr("transfer-recent")
        }),
        rows: transfer_rows(t),
        multi: false,
        more: None,
        choice: Some(ChoiceView {
            label: tr("transfer-when-taken"),
            options: vec![
                tr("transfer-skip"),
                tr("transfer-keep-both"),
                tr("transfer-replace-option"),
            ],
            selected: match t.conflict {
                Conflict::Skip => 0,
                Conflict::KeepBoth => 1,
                Conflict::Replace => 2,
            },
        }),
        buttons: buttons(ok, t.chosen.is_some(), tr("fileop-cancel")),
        focus,
    }
}

/// A preview row for what is left out: the setting (its label when it is one) and why.
fn warning_text(w: &Warning) -> (String, String) {
    let i18n = I18n::global();
    match w {
        Warning::InvalidValue { key, reason } => (
            specular_settings::find(key).map_or_else(|| key.clone(), |m| m.label(i18n)),
            reason.message(i18n),
        ),
        Warning::UnknownKey { key } => (key.clone(), tr("settings-file-import-unknown")),
        _ => (String::new(), w.message(i18n)),
    }
}

/// Every line of the import preview: settings that change, key bindings that change, then
/// what is left out.
fn import_lines(i: &ImportSheet) -> Vec<RowView> {
    let i18n = I18n::global();
    let mut rows: Vec<RowView> = i
        .preview
        .changes
        .iter()
        .map(|c| RowView {
            start: c.meta.label(i18n),
            end: Some(tr_args(
                "settings-file-import-change",
                &[
                    ("before", c.meta.display_value(&c.before, i18n)),
                    ("after", c.meta.display_value(&c.after, i18n)),
                ],
            )),
            problem: None,
            quiet: false,
            choosable: false,
            chosen: false,
        })
        .collect();
    rows.extend(i.keys.iter().map(|(name, before, after)| RowView {
        start: name.clone(),
        end: Some(tr_args(
            "settings-file-import-change",
            &[("before", before.clone()), ("after", after.clone())],
        )),
        problem: None,
        quiet: false,
        choosable: false,
        chosen: false,
    }));
    rows.extend(i.preview.problems.iter().map(|w| {
        let (what, why) = warning_text(w);
        RowView {
            start: what,
            end: None,
            problem: Some(why),
            quiet: false,
            choosable: false,
            chosen: false,
        }
    }));
    rows.extend(i.key_problems.iter().map(|p| RowView {
        start: tr("settings-file-import-keymap"),
        end: None,
        problem: Some(p.clone()),
        quiet: false,
        choosable: false,
        chosen: false,
    }));
    rows
}

fn import_view(i: &ImportSheet, focus: Part) -> SheetView {
    let all = import_lines(i);
    let total = all.len();
    let rows: Vec<RowView> = all.into_iter().skip(i.top).take(view::MAX_ROWS).collect();
    let below = total.saturating_sub(i.top + rows.len());
    let mut notes = Vec::new();
    let changes = i.preview.changes.len() + i.keys.len();
    notes.push(if changes == 0 {
        tr("settings-file-import-nothing")
    } else {
        tr_args(
            "settings-file-import-summary",
            &[("count", changes.to_string())],
        )
    });
    if !i.preview.secrets.is_empty() {
        notes.push(tr_args(
            "settings-file-import-secrets",
            &[("count", i.preview.secrets.len().to_string())],
        ));
    }
    if i.preview.commands > 0 {
        notes.push(tr_args(
            "settings-file-import-commands",
            &[("count", i.preview.commands.to_string())],
        ));
    }
    SheetView {
        title: tr("settings-file-import-title"),
        subtitle: Some(i.path.display().to_string()),
        field: None,
        error: None,
        notes,
        list_label: (total > 0).then(|| tr("settings-file-import-changes")),
        rows,
        multi: false,
        more: (below > 0).then(|| tr_args("batch-rename-more", &[("count", below.to_string())])),
        choice: None,
        buttons: buttons(
            tr("settings-file-import-button"),
            changes > 0,
            tr("fileop-cancel"),
        ),
        focus,
    }
}

/// What an operation did to paths, for refreshing the listings: (moved from → to, files
/// that appeared or went, files rewritten). `undo` reads the steps backwards.
fn effects(op: &Op, undo: bool) -> (Vec<(PathBuf, PathBuf)>, Vec<PathBuf>, Vec<PathBuf>) {
    let (mut moved, mut touched, mut written) = (Vec::new(), Vec::new(), Vec::new());
    for s in &op.steps {
        match s {
            Step::Moved { from, to, .. } => {
                moved.push(if undo {
                    (to.clone(), from.clone())
                } else {
                    (from.clone(), to.clone())
                });
            }
            Step::Copied { to, .. } => touched.push(to.clone()),
            Step::Trashed { path } => touched.push(path.clone()),
            Step::Wrote { path, before, .. } => {
                if before.is_some() {
                    written.push(path.clone());
                } else {
                    touched.push(path.clone());
                }
            }
        }
    }
    (moved, touched, written)
}

/// The recent folders setting as a list.
fn recent_folders(s: &str) -> Vec<PathBuf> {
    s.lines()
        .filter(|l| !l.trim().is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .take(MAX_RECENT)
        .collect()
}

/// `folder` first in the recent list.
fn remember(s: &str, folder: &Path) -> String {
    let mut v: Vec<PathBuf> = vec![folder.to_path_buf()];
    v.extend(
        s.lines()
            .filter(|l| !l.trim().is_empty())
            .map(PathBuf::from)
            .filter(|p| p != folder),
    );
    v.truncate(MAX_RECENT);
    v.iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

impl App {
    /// A file operation's task panicked ([`crate::loader::Event::BackgroundFailed`]): the
    /// next operation may start.
    pub(super) fn fileops_lost(&mut self, task: &str) {
        if task.starts_with("fop-") {
            self.fileops.busy = false;
        }
    }

    // ----- Opening the sheets -----------------------------------------------------------

    /// The files a file action works on: the gallery's selection (or focus), else the image
    /// shown.
    pub(super) fn fileops_targets(&self) -> Vec<PathBuf> {
        if self.gallery.active {
            return self
                .gallery
                .targets()
                .into_iter()
                .filter(|p| p.is_file())
                .collect();
        }
        self.integration_file().into_iter().collect()
    }

    fn fileops_toast(&mut self, text: String, undo: Option<u64>) {
        let now = self.now();
        self.chrome.toast(text, undo, now);
        self.redraw = true;
    }

    fn fileops_busy(&mut self) -> bool {
        if self.fileops.busy {
            self.fileops_toast(tr("fileop-busy"), None);
            return true;
        }
        false
    }

    /// File actions while the gallery shows: they work on its selection, whatever the
    /// viewer behind has open. True: handled.
    pub(super) fn fileops_gallery_action(&mut self, a: Action) -> bool {
        if !self.gallery.active {
            return false;
        }
        match a {
            Action::Rename
            | Action::BatchRename
            | Action::CopyToFolder
            | Action::MoveToFolder
            | Action::RotateFileLeft
            | Action::RotateFileRight
            | Action::SetWallpaper
            | Action::Share
            | Action::SendByEmail
            | Action::ShareWithZynk
            | Action::SendWithZynk => self.fileops_action(a),
            _ => false,
        }
    }

    /// The file actions; true when `a` is one.
    pub(super) fn fileops_action(&mut self, a: Action) -> bool {
        match a {
            Action::Redo => self.fileops_redo(),
            Action::Rename => self.fileops_rename_open(),
            Action::BatchRename => self.fileops_batch_open(),
            Action::CopyToFolder => self.fileops_transfer_open(Mode::Copy),
            Action::MoveToFolder => self.fileops_transfer_open(Mode::Move),
            Action::RotateFileLeft => self.fileops_rotate(Transform::ROTATE_270),
            Action::RotateFileRight => self.fileops_rotate(Transform::ROTATE_90),
            Action::SetWallpaper => self.set_wallpaper(),
            Action::Share => self.fileops_share(),
            Action::SendByEmail => self.fileops_email(),
            Action::ShareWithZynk => self.share_with_zynk(),
            Action::SendWithZynk => self.send_with_zynk(),
            Action::ExportSettings => self.fileops_export_settings(),
            Action::ImportSettings => self.fileops_import_settings(),
            _ => return false,
        }
        self.redraw = true;
        true
    }

    fn fileops_open(&mut self, kind: Kind2, focus: Part) {
        self.fileops.sheet = Some(Sheet {
            kind,
            focus,
            drawn: Drawn::default(),
        });
        self.redraw = true;
    }

    /// Shows one of Develop's sheets, the keyboard on its name field or its first row.
    pub(super) fn fileops_open_develop(&mut self, d: super::develop::DevelopSheet) {
        let focus = if d.field.is_some() {
            Part::Field
        } else if d.rows.is_empty() {
            Part::Button(0)
        } else {
            Part::Row(0)
        };
        self.fileops_open(Kind2::Develop(Box::new(d)), focus);
    }

    /// Shows an album sheet, the keyboard on its field.
    pub(super) fn fileops_open_album(&mut self, a: super::gallery::albums::AlbumSheet) {
        let focus = a.focus();
        self.fileops_open(Kind2::Album(Box::new(a)), focus);
    }

    /// Closes the album sheet, giving it back.
    pub(super) fn fileops_take_album(&mut self) -> Option<super::gallery::albums::AlbumSheet> {
        if !matches!(
            self.fileops.sheet.as_ref().map(|s| &s.kind),
            Some(Kind2::Album(_))
        ) {
            return None;
        }
        let s = self.fileops.sheet.take()?;
        self.redraw = true;
        match s.kind {
            Kind2::Album(a) => Some(*a),
            _ => None,
        }
    }

    /// Closes Develop's sheet, giving it back.
    pub(super) fn fileops_take_develop(&mut self) -> Option<super::develop::DevelopSheet> {
        if !matches!(
            self.fileops.sheet.as_ref().map(|s| &s.kind),
            Some(Kind2::Develop(_))
        ) {
            return None;
        }
        let s = self.fileops.sheet.take()?;
        self.redraw = true;
        match s.kind {
            Kind2::Develop(d) => Some(*d),
            _ => None,
        }
    }

    fn fileops_rename_open(&mut self) {
        if self.fileops_busy() {
            return;
        }
        let targets = self.fileops_targets();
        if targets.len() > 1 {
            return self.fileops_batch_open();
        }
        let Some(file) = targets.into_iter().next() else {
            return;
        };
        let name = file_name(&file);
        let mut field = LineEdit::new(&name);
        field.select(names::stem_range(&name));
        self.fileops_open(
            Kind2::Rename {
                file,
                field: Box::new(field),
            },
            Part::Field,
        );
    }

    fn fileops_batch_open(&mut self) {
        if self.fileops_busy() {
            return;
        }
        let mut files = self.fileops_targets();
        if files.len() < 2 {
            // Not a selection: the whole folder.
            files = if self.gallery.active {
                self.gallery.files()
            } else {
                self.entries.clone()
            };
            files.retain(|p| p.is_file());
        }
        if files.is_empty() {
            return;
        }
        let pattern = self.fileops.pattern.clone();
        let mut field = LineEdit::new(&pattern);
        field.select_all();
        let mut b = Batch {
            files,
            field,
            probes: HashMap::new(),
            probing: false,
            rows: Vec::new(),
            rows_for: None,
            top: 0,
        };
        rebuild_rows(&mut b);
        self.fileops_open(Kind2::Batch(Box::new(b)), Part::Field);
        self.fileops_probe_if_needed();
    }

    /// Reads capture dates, cameras and sizes for batch rename once the pattern needs them.
    fn fileops_probe_if_needed(&mut self) {
        let Some(Sheet {
            kind: Kind2::Batch(b),
            ..
        }) = self.fileops.sheet.as_mut()
        else {
            return;
        };
        let needs = Pattern::parse(b.field.text())
            .is_ok_and(|p| p.uses_date() || p.uses_camera() || p.uses_size());
        if !needs || b.probing || !b.probes.is_empty() {
            return;
        }
        b.probing = true;
        let files = b.files.clone();
        let registry = self.loader.registry();
        let tx = self.fileops.tx.clone();
        self.loader.background("fop-probe", move |l| {
            let mut list = Vec::new();
            for f in files {
                if let Some(p) = specular_convert::probe(&registry, &f) {
                    list.push((f, p));
                }
                if list.len() >= 64 {
                    let _ = tx.send(Msg::Probed {
                        list: std::mem::take(&mut list),
                        done: false,
                    });
                    l.waker()();
                }
            }
            let _ = tx.send(Msg::Probed { list, done: true });
            l.waker()();
        });
    }

    fn fileops_transfer_open(&mut self, mode: Mode) {
        if self.fileops_busy() {
            return;
        }
        let files = self.fileops_targets();
        if files.is_empty() {
            return;
        }
        let folders = recent_folders(&self.settings.browsing.recent_folders);
        let chosen = folders.first().cloned();
        let mut t = TransferSheet {
            mode,
            files,
            folders,
            chosen,
            conflict: Conflict::KeepBoth,
            confirm: false,
            picking: false,
            items: Vec::new(),
        };
        replan(&mut t);
        let focus = if t.folders.is_empty() {
            Part::Row(0)
        } else {
            Part::Button(0)
        };
        self.fileops_open(Kind2::Transfer(Box::new(t)), focus);
    }

    fn fileops_close(&mut self) {
        if let Some(Sheet {
            kind: Kind2::Batch(b),
            ..
        }) = &self.fileops.sheet
            && Pattern::parse(b.field.text()).is_ok()
        {
            self.fileops.pattern = b.field.text().to_owned();
        }
        self.fileops.sheet = None;
        self.redraw = true;
    }

    // ----- Running operations ---------------------------------------------------------

    fn fileops_rename_start(&mut self) {
        let Some(Sheet {
            kind: Kind2::Rename { file, field },
            ..
        }) = &self.fileops.sheet
        else {
            return;
        };
        let (file, new) = (file.clone(), field.text().to_owned());
        if new == file_name(&file) {
            return self.fileops_close();
        }
        if names::validate(&file, &new, Platform::current()).is_err() {
            return;
        }
        self.fileops_close();
        let token = self.next_undo;
        self.next_undo += 1;
        self.fileops.busy = true;
        let tx = self.fileops.tx.clone();
        self.loader.background("fop-rename", move |l| {
            let result = transfer::rename(&file, &new).map_err(|(p, e)| {
                format!("{}: {}", file_name(&p), crate::error_text::io_reason(&e))
            });
            let _ = tx.send(Msg::Renamed { token, result });
            l.waker()();
        });
    }

    fn fileops_batch_start(&mut self) {
        let Some(Sheet {
            kind: Kind2::Batch(b),
            ..
        }) = self.fileops.sheet.as_mut()
        else {
            return;
        };
        rebuild_rows(b);
        let rows = b.rows.clone();
        if !rows.iter().any(|r| r.problem.is_none() && !r.unchanged()) {
            return;
        }
        self.fileops_close();
        let token = self.next_undo;
        self.next_undo += 1;
        self.fileops.busy = true;
        let tx = self.fileops.tx.clone();
        self.loader.background("fop-batch", move |l| {
            let result = batch::apply(&rows).map_err(|(p, e)| {
                format!("{}: {}", file_name(&p), crate::error_text::io_reason(&e))
            });
            let _ = tx.send(Msg::Renamed { token, result });
            l.waker()();
        });
    }

    fn fileops_transfer_start(&mut self) {
        let Some(Sheet {
            kind: Kind2::Transfer(t),
            ..
        }) = self.fileops.sheet.as_mut()
        else {
            return;
        };
        let Some(folder) = t.chosen.clone() else {
            return;
        };
        replan(t);
        if t.conflict == Conflict::Replace && transfer::conflicts(&t.items) > 0 && !t.confirm {
            t.confirm = true;
            if let Some(s) = self.fileops.sheet.as_mut() {
                s.focus = Part::Button(1);
            }
            self.redraw = true;
            return;
        }
        let (items, mode, conflict, count) = (t.items.clone(), t.mode, t.conflict, t.files.len());
        self.fileops_close();
        let recent = remember(&self.settings.browsing.recent_folders, &folder);
        self.store
            .update(Origin::Program, |s| s.browsing.recent_folders = recent);
        let token = self.next_undo;
        self.next_undo += 1;
        self.fileops.busy = true;
        let cancel = Arc::new(AtomicBool::new(false));
        self.fileops.running = Some(Running {
            cancel: cancel.clone(),
            toast: None,
            toast_at: self.now(),
            last: None,
            mode,
            count,
        });
        let tx = self.fileops.tx.clone();
        self.loader.background("fop-transfer", move |l| {
            let wake = l.waker();
            let mut last = std::time::Instant::now();
            let outcome = transfer::run(&items, mode, conflict, false, &SystemTrash, &mut |p| {
                if last.elapsed().as_secs_f64() >= PROGRESS_EVERY {
                    last = std::time::Instant::now();
                    let _ = tx.send(Msg::Progress(p.clone()));
                    wake();
                }
                !cancel.load(Ordering::Relaxed)
            });
            let _ = tx.send(Msg::Transferred {
                token,
                mode,
                folder,
                outcome,
            });
            wake();
        });
    }

    fn fileops_rotate(&mut self, turn: Transform) {
        if self.fileops_busy() {
            return;
        }
        let targets = self.fileops_targets();
        let Some(path) = targets.into_iter().next() else {
            return;
        };
        if !orient::rotatable_name(&path) {
            let name = file_name(&path);
            return self.fileops_toast(rotate_problem_text(&RotateError::Unsupported, &name), None);
        }
        let opts = RotateOptions {
            trim: self.settings.browsing.rotate_trim_edges,
            keep_mtime: self.settings.browsing.rotate_keep_modified,
            displayed: None,
        };
        self.fileops_rotate_with(path, turn, opts);
    }

    fn fileops_rotate_with(&mut self, path: PathBuf, turn: Transform, opts: RotateOptions) {
        let token = self.next_undo;
        self.next_undo += 1;
        self.fileops.busy = true;
        let tx = self.fileops.tx.clone();
        self.loader.background("fop-rotate", move |l| {
            let before = std::fs::metadata(&path)
                .ok()
                .and_then(|m| crate::display_cache::Key::new(&path, m.len(), m.modified().ok()));
            let result = orient::rotate(&path, turn, opts);
            let _ = tx.send(Msg::Rotated {
                token,
                path,
                turn,
                before,
                result,
            });
            l.waker()();
        });
    }

    /// "Trim and rotate losslessly" on a tag rotation's toast: the tag change is undone and
    /// the JPEG rotated again with its edge trimmed.
    fn fileops_trim_offer(&mut self, key: u64) {
        let Some((token, path, turn)) = self.fileops.trim_offers.remove(&key) else {
            return;
        };
        self.chrome.dismiss_undo(key);
        if self.fileops_busy() {
            return;
        }
        let Some(op) = self.fileops.history.take(token) else {
            return;
        };
        self.fileops.busy = true;
        let new = self.next_undo;
        self.next_undo += 1;
        let keep_mtime = self.settings.browsing.rotate_keep_modified;
        let tx = self.fileops.tx.clone();
        self.loader.background("fop-rotate", move |l| {
            let before = std::fs::metadata(&path)
                .ok()
                .and_then(|m| crate::display_cache::Key::new(&path, m.len(), m.modified().ok()));
            let result = match history::undo(&op, &SystemTrash) {
                Ok(_) => orient::rotate(
                    &path,
                    turn,
                    RotateOptions {
                        trim: true,
                        keep_mtime,
                        displayed: None,
                    },
                ),
                Err(e) => Err(RotateError::Io(undo_problem_text(&e))),
            };
            let _ = tx.send(Msg::Rotated {
                token: new,
                path,
                turn,
                before,
                result,
            });
            l.waker()();
        });
    }

    /// Undo of operation `u`; false when `u` is not ours.
    pub(super) fn fileops_undo(&mut self, u: u64) -> bool {
        if u == CANCEL_KEY {
            if let Some(r) = &self.fileops.running {
                r.cancel.store(true, Ordering::Relaxed);
            }
            return true;
        }
        if self.fileops.trim_offers.contains_key(&u) {
            self.fileops_trim_offer(u);
            return true;
        }
        if !self.fileops.history.contains(u) {
            return false;
        }
        if self.fileops_busy() {
            return true;
        }
        let Some(op) = self.fileops.history.take(u) else {
            return true;
        };
        self.chrome.dismiss_undo(u);
        if self.last_undo == Some(u) {
            self.last_undo = self.fileops.history.last_token();
        }
        self.fileops.trim_offers.retain(|_, (t, _, _)| *t != u);
        self.fileops.busy = true;
        let tx = self.fileops.tx.clone();
        self.loader.background("fop-undo", move |l| {
            let result = history::undo(&op, &SystemTrash);
            let _ = tx.send(Msg::Undone {
                token: u,
                redo: false,
                op,
                result,
            });
            l.waker()();
        });
        true
    }

    fn fileops_redo(&mut self) {
        if self.fileops_busy() {
            return;
        }
        let Some((token, op)) = self.fileops.history.take_redo() else {
            return self.fileops_toast(tr("fileop-nothing-to-redo"), None);
        };
        self.fileops.busy = true;
        let tx = self.fileops.tx.clone();
        self.loader.background("fop-redo", move |l| {
            let result = history::redo(&op, &SystemTrash);
            let _ = tx.send(Msg::Undone {
                token,
                redo: true,
                op,
                result,
            });
            l.waker()();
        });
    }

    // ----- Sharing, email, settings files ----------------------------------------------

    fn fileops_share(&mut self) {
        let files = self.fileops_targets();
        if files.is_empty() {
            return;
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let tx = self.fileops.tx.clone();
            self.loader.background("fop-share", move |l| {
                if let Err(e) = specular_platform::share::open_with_chooser(&files[0]) {
                    let _ = tx.send(Msg::Failed(tr_args(
                        "share-failed",
                        &[("reason", crate::error_text::io_reason(&e))],
                    )));
                    l.waker()();
                }
            });
        }
        #[cfg(not(all(unix, not(target_os = "macos"))))]
        {
            let r = match self.gfx.as_ref() {
                Some(g) => specular_platform::share::share(&files, &*g.window),
                None => Err(std::io::Error::from(std::io::ErrorKind::Unsupported)),
            };
            if let Err(e) = r {
                self.fileops_toast(
                    tr_args(
                        "share-failed",
                        &[("reason", crate::error_text::io_reason(&e))],
                    ),
                    None,
                );
            }
        }
    }

    fn fileops_email(&mut self) {
        let files = self.fileops_targets();
        if files.is_empty() {
            return;
        }
        let tx = self.fileops.tx.clone();
        self.loader.background("fop-email", move |l| {
            if let Err(e) = specular_platform::share::send_by_email(&files) {
                let _ = tx.send(Msg::Failed(tr_args(
                    "email-failed",
                    &[("reason", crate::error_text::io_reason(&e))],
                )));
                l.waker()();
            }
        });
    }

    pub(super) fn fileops_export_settings(&mut self) {
        let text = self.store.export_bundle(Some(&self.keymap.to_toml()));
        let tx = self.fileops.tx.clone();
        let title = tr("settings-file-export-dialog");
        self.loader.background("dlg-export", move |l| {
            let Some(p) = rfd::FileDialog::new()
                .set_title(&title)
                .add_filter("TOML", &["toml"])
                .set_file_name("specular-settings.toml")
                .save_file()
            else {
                return;
            };
            let r = specular_settings::write_atomic(&p, &text)
                .map(|()| p)
                .map_err(|e| crate::error_text::io_reason(&e));
            let _ = tx.send(Msg::Exported(r));
            l.waker()();
        });
    }

    pub(super) fn fileops_import_settings(&mut self) {
        let tx = self.fileops.tx.clone();
        let title = tr("settings-file-import-dialog");
        self.loader.background("dlg-import", move |l| {
            let Some(p) = rfd::FileDialog::new()
                .set_title(&title)
                .add_filter("TOML", &["toml"])
                .pick_file()
            else {
                return;
            };
            let r = std::fs::metadata(&p)
                .and_then(|m| {
                    if m.len() > 4 << 20 {
                        Err(std::io::Error::from(std::io::ErrorKind::FileTooLarge))
                    } else {
                        std::fs::read_to_string(&p)
                    }
                })
                .map_err(|e| crate::error_text::io_reason(&e));
            let _ = tx.send(Msg::ImportRead(p, r));
            l.waker()();
        });
    }

    /// The import sheet for the text of `path`.
    fn fileops_import_preview(&mut self, path: PathBuf, text: &str) {
        let preview = match specular_settings::preview_import(&self.settings, text) {
            Ok(p) => p,
            Err(w) => {
                return self.fileops_toast(
                    tr_args(
                        "settings-file-import-unreadable",
                        &[("file", file_name(&path)), ("reason", w.to_string())],
                    ),
                    None,
                );
            }
        };
        let mut keys = Vec::new();
        let mut key_problems = Vec::new();
        if let Some(k) = &preview.keymap {
            let (new, problems) = specular_input::Keymap::from_toml(k);
            key_problems.extend(problems.iter().map(|p| p.to_string()));
            for mode in KeyMode::ALL {
                for a in Action::BINDABLE.iter().filter(|a| a.mode() == mode) {
                    let old = self.keymap.keys_in(mode, *a);
                    let now = new.keys_in(mode, *a);
                    if old != now {
                        let list = |v: &[specular_input::KeySeq]| {
                            if v.is_empty() {
                                tr("settings-file-import-no-key")
                            } else {
                                v.iter()
                                    .map(|k| k.to_string())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            }
                        };
                        keys.push((a.name(), list(&old), list(&now)));
                    }
                }
            }
        }
        let sheet = ImportSheet {
            path,
            preview,
            keys,
            key_problems,
            top: 0,
        };
        self.fileops_open(Kind2::Import(Box::new(sheet)), Part::Button(0));
    }

    fn fileops_import_apply(&mut self) {
        let Some(Sheet {
            kind: Kind2::Import(i),
            ..
        }) = self.fileops.sheet.take()
        else {
            return;
        };
        let changed = self.store.apply_import(&i.preview);
        let mut n = changed.len();
        if !i.keys.is_empty()
            && let Some(k) = &i.preview.keymap
            && self.keymap.import(k).is_ok()
        {
            self.keys_save_imported();
            n += i.keys.len();
        }
        self.fileops_toast(
            tr_args("settings-file-import-done", &[("count", n.to_string())]),
            None,
        );
    }

    // ----- Results ------------------------------------------------------------------------

    /// Once per event-loop turn: what the workers sent.
    pub(super) fn fileops_tick(&mut self) {
        while let Ok(m) = self.fileops.rx.try_recv() {
            self.redraw = true;
            match m {
                Msg::Renamed { token, result } => {
                    self.fileops.busy = false;
                    match result {
                        Ok(op) => self.fileops_done(token, op),
                        Err(e) => {
                            self.fileops_toast(tr_args("rename-failed", &[("reason", e)]), None)
                        }
                    }
                }
                Msg::Progress(p) => {
                    let now = self.now();
                    if let Some(r) = self.fileops.running.as_mut() {
                        r.last = Some(p);
                        if r.toast.is_none() || now - r.toast_at >= 0.5 {
                            let text = progress_text(r);
                            r.toast = Some(match r.toast {
                                Some(id) => self.chrome.update_toast(id, text, now),
                                None => self.chrome.toast_action(
                                    text,
                                    tr("transfer-stop"),
                                    CANCEL_KEY,
                                    now,
                                ),
                            });
                            r.toast_at = now;
                        }
                    }
                }
                Msg::Transferred {
                    token,
                    mode,
                    folder,
                    outcome,
                } => {
                    self.fileops.busy = false;
                    if let Some(r) = self.fileops.running.take()
                        && let Some(id) = r.toast
                    {
                        self.chrome.dismiss_toast(id);
                    }
                    self.fileops_transferred(token, mode, &folder, outcome);
                }
                Msg::Rotated {
                    token,
                    path,
                    turn,
                    before,
                    result,
                } => {
                    self.fileops.busy = false;
                    self.fileops_rotated(token, path, turn, before, result);
                }
                Msg::Undone {
                    token,
                    redo,
                    op,
                    result,
                } => {
                    self.fileops.busy = false;
                    self.fileops_undone(token, redo, op, result);
                }
                Msg::Picked(p) => {
                    if let Some(Sheet {
                        kind: Kind2::Transfer(t),
                        focus,
                        ..
                    }) = self.fileops.sheet.as_mut()
                    {
                        t.picking = false;
                        if let Some(p) = p {
                            t.chosen = Some(p);
                            replan(t);
                            *focus = Part::Button(0);
                        }
                    }
                }
                Msg::Probed { list, done } => {
                    if let Some(Sheet {
                        kind: Kind2::Batch(b),
                        ..
                    }) = self.fileops.sheet.as_mut()
                    {
                        b.probes.extend(list);
                        if done {
                            b.probing = false;
                        }
                        rebuild_rows(b);
                    }
                }
                Msg::ImportRead(p, r) => match r {
                    Ok(text) => self.fileops_import_preview(p, &text),
                    Err(e) => self.fileops_toast(
                        tr_args(
                            "settings-file-import-unreadable",
                            &[("file", file_name(&p)), ("reason", e)],
                        ),
                        None,
                    ),
                },
                Msg::Exported(r) => match r {
                    Ok(p) => self.fileops_toast(
                        tr_args(
                            "settings-file-export-done",
                            &[("file", p.display().to_string())],
                        ),
                        None,
                    ),
                    Err(e) => self.fileops_toast(
                        tr_args("settings-file-export-failed", &[("reason", e)]),
                        None,
                    ),
                },
                Msg::Failed(s) => self.fileops_toast(s, None),
            }
        }
    }

    /// A finished operation: journalled under `token`, listings refreshed, a toast with Undo.
    fn fileops_done(&mut self, token: u64, op: Op) {
        if op.is_empty() {
            return;
        }
        let (moved, touched, written) = effects(&op, false);
        let text = match op.kind {
            Kind::Rename => tr_args(
                "fileop-renamed",
                &[(
                    "file",
                    moved.first().map(|m| file_name(&m.1)).unwrap_or_default(),
                )],
            ),
            _ => {
                let n = moved
                    .iter()
                    .filter(|(f, _)| !names::is_sidecar_name(f))
                    .count();
                tr_args("fileop-renamed-count", &[("count", n.to_string())])
            }
        };
        self.fileops.history.push(token, op);
        self.last_undo = Some(token);
        self.fileops_changed(&moved, &touched, &written);
        self.fileops_toast(text, Some(token));
        self.chrome.announce_status(tr("fileop-done-status"));
    }

    fn fileops_transferred(&mut self, token: u64, mode: Mode, folder: &Path, out: Outcome) {
        let (moved, mut touched, written) = effects(&out.op, false);
        touched.extend(moved.iter().map(|m| m.1.clone()));
        let folder_name = file_name(folder);
        let mut text = match mode {
            Mode::Copy => tr_args(
                "fileop-copied",
                &[("count", out.done.to_string()), ("folder", folder_name)],
            ),
            Mode::Move => tr_args(
                "fileop-moved",
                &[("count", out.done.to_string()), ("folder", folder_name)],
            ),
        };
        if out.skipped > 0 {
            text = format!(
                "{text} {}",
                tr_args("fileop-skipped", &[("count", out.skipped.to_string())])
            );
        }
        if out.cancelled {
            text = format!("{text} {}", tr("fileop-stopped"));
        }
        let undo = (!out.op.is_empty()).then_some(token);
        if let Some(t) = undo {
            self.fileops.history.push(t, out.op);
            self.last_undo = Some(t);
        }
        self.fileops_changed(&moved, &touched, &written);
        self.fileops_toast(text, undo);
        if let Some((p, e)) = out.failed.first() {
            let now = self.now();
            self.chrome.warning(
                tr_args(
                    "fileop-failed-file",
                    &[
                        ("file", file_name(p)),
                        ("reason", e.clone()),
                        ("count", out.failed.len().to_string()),
                    ],
                ),
                now,
            );
        }
    }

    fn fileops_rotated(
        &mut self,
        token: u64,
        path: PathBuf,
        turn: Transform,
        before: Option<crate::display_cache::Key>,
        result: Result<Rotated, RotateError>,
    ) {
        let name = file_name(&path);
        let r = match result {
            Ok(r) => r,
            Err(e) => return self.fileops_toast(rotate_problem_text(&e, &name), None),
        };
        let text = rotated_text(&r, &name);
        if r.op.is_empty() {
            return self.fileops_toast(text, None);
        }
        // Kept its size and date: the cached renderings of the old pixels must go.
        if let Some(k) = before {
            let after = std::fs::metadata(&path)
                .ok()
                .and_then(|m| crate::display_cache::Key::new(&path, m.len(), m.modified().ok()));
            if after.as_ref() == Some(&k) {
                self.loader.forget_display(&k);
            }
        }
        let (moved, touched, written) = effects(&r.op, false);
        self.fileops.history.push(token, r.op);
        self.last_undo = Some(token);
        self.fileops_changed(&moved, &touched, &written);
        let now = self.now();
        if let Method::Tag {
            reason: Some(TagReason::NotAligned { .. }),
        } = r.method
        {
            // The lossless rotation is one press away, with the user's consent to trim.
            let key = TRIM_BASE + token;
            self.fileops.trim_offers.insert(key, (token, path, turn));
            self.chrome
                .toast_action(text, tr("rotate-trim-offer"), key, now);
            self.last_undo = Some(token);
        } else {
            self.chrome.toast(text, Some(token), now);
        }
        self.redraw = true;
    }

    fn fileops_undone(&mut self, token: u64, redo: bool, op: Op, result: Result<Op, UndoError>) {
        let kind = op.kind;
        match result {
            Ok(back) => {
                let (moved, touched, written) = effects(&op, !redo);
                if redo {
                    self.fileops.history.redone(token, back);
                    self.last_undo = Some(token);
                    self.fileops_toast(
                        tr_args("fileop-redone", &[("what", kind_text(kind))]),
                        Some(token),
                    );
                } else {
                    self.fileops.history.undone(token, back);
                    self.fileops_toast(
                        tr_args("fileop-undone", &[("what", kind_text(kind))]),
                        None,
                    );
                }
                self.fileops_changed(&moved, &touched, &written);
            }
            Err(e) => {
                if redo {
                    self.fileops.history.restore_redo(token, op);
                } else {
                    self.fileops.history.restore(token, op);
                }
                let now = self.now();
                self.chrome.warning(undo_problem_text(&e), now);
                self.redraw = true;
            }
        }
    }

    /// Brings the viewer and the gallery up to date after files were renamed or moved
    /// (`moved`), appeared or went (`touched`) or were rewritten (`written`).
    fn fileops_changed(
        &mut self,
        moved: &[(PathBuf, PathBuf)],
        touched: &[PathBuf],
        written: &[PathBuf],
    ) {
        let cur_path = self.current.and_then(|i| self.entries.get(i)).cloned();
        let mut entries = self.entries.clone();
        let mut new_cur = cur_path.clone();
        let mut removed_at = None;
        for (from, to) in moved {
            self.fileops_forget_image(from);
            let Some(i) = entries.iter().position(|p| p == from) else {
                continue;
            };
            if from.parent() == to.parent() {
                entries[i] = to.clone();
                if cur_path.as_ref() == Some(from) {
                    new_cur = Some(to.clone());
                }
            } else {
                entries.remove(i);
                if cur_path.as_ref() == Some(from) {
                    new_cur = None;
                    removed_at = Some(i);
                }
            }
        }
        for p in written {
            self.fileops_forget_image(p);
        }
        if entries != self.entries {
            self.set_entries(entries);
        }
        let reopen = cur_path != new_cur || cur_path.as_ref().is_some_and(|c| written.contains(c));
        match (&new_cur, removed_at) {
            (Some(c), _) => {
                if let Some(i) = self.index_of(c) {
                    self.current = Some(i);
                    if reopen {
                        self.shown = None;
                        self.go_to(i);
                    }
                }
            }
            (None, Some(i)) if !self.entries.is_empty() => {
                self.shown = None;
                self.go_to(i.min(self.entries.len() - 1));
            }
            (None, Some(_)) => {
                self.current = None;
                self.shown = None;
            }
            (None, None) => {}
        }
        // Files that arrived in or left the folder shown: list it again (keeps the current
        // image current).
        if let Some(dir) = self.folder.clone() {
            let here = |p: &PathBuf| p.parent() == Some(dir.as_path());
            if moved.iter().any(|(f, t)| here(f) || here(t)) || touched.iter().any(here) {
                self.scan_folder(dir);
            }
        }
        if self.gallery.started() {
            let mut all: Vec<PathBuf> = moved
                .iter()
                .flat_map(|(f, t)| [f.clone(), t.clone()])
                .collect();
            all.extend(touched.iter().cloned());
            all.extend(written.iter().cloned());
            self.gallery_files_changed(all, moved);
        }
        self.update_title();
        self.redraw = true;
    }

    /// Drops what is decoded of `path` (renamed, moved or rewritten).
    fn fileops_forget_image(&mut self, path: &Path) {
        let Some(id) = self.by_path.remove(path) else {
            return;
        };
        if let Some(img) = self.images.remove(&id) {
            img.token.cancel();
        }
        self.gpu_images.remove(id);
        if let Some(g) = self.gfx.as_mut() {
            g.renderer.release_image(id);
        }
    }

    // ----- Input -------------------------------------------------------------------------

    pub(super) fn fileops_text(&mut self, s: &str) {
        let Some(sheet) = self.fileops.sheet.as_mut() else {
            return;
        };
        if sheet.focus != Part::Field {
            return;
        }
        // One line: a pasted line break ends it.
        let s = s.lines().next().unwrap_or("");
        match &mut sheet.kind {
            Kind2::Rename { field, .. } => {
                field.insert(s);
            }
            Kind2::Develop(d) => {
                if let Some((_, f)) = d.field.as_mut() {
                    f.insert(s);
                }
            }
            Kind2::Album(a) => {
                a.field.insert(s);
            }
            Kind2::Batch(b) => {
                b.field.insert(s);
                rebuild_rows(b);
            }
            _ => return,
        }
        self.fileops_probe_if_needed();
        self.redraw = true;
    }

    pub(super) fn fileops_paste(&mut self, s: &str) {
        self.fileops_text(s);
    }

    fn fileops_field(&mut self) -> Option<&mut LineEdit> {
        match &mut self.fileops.sheet.as_mut()?.kind {
            Kind2::Rename { field, .. } => Some(&mut **field),
            Kind2::Batch(b) => Some(&mut b.field),
            Kind2::Develop(d) => d.field.as_mut().map(|f| &mut f.1),
            Kind2::Album(a) => Some(&mut a.field),
            _ => None,
        }
    }

    /// The field's keys; true when it used the key.
    fn fileops_field_key(&mut self, combo: Option<&KeyCombo>, text: Option<&str>) -> bool {
        let rtl = self
            .fileops
            .sheet
            .as_ref()
            .and_then(|s| s.drawn.field_text.as_ref())
            .is_some_and(|(b, _)| b.rtl);
        if let Some(c) = combo {
            // Enter, Escape and Tab are the sheet's.
            if matches!(
                c.key,
                Key::Named(NamedKey::Enter | NamedKey::Escape | NamedKey::Tab)
            ) {
                return false;
            }
            let mut clip = PendingClipboard::default();
            let Some(f) = self.fileops_field() else {
                return false;
            };
            if f.key(c.key, c.mods, rtl, &mut clip).used() {
                self.field_clipboard(clip, PasteTo::FileOps);
                if let Some(Sheet {
                    kind: Kind2::Batch(b),
                    ..
                }) = self.fileops.sheet.as_mut()
                {
                    rebuild_rows(b);
                }
                self.fileops_probe_if_needed();
                self.redraw = true;
                return true;
            }
            if c.mods.ctrl || c.mods.alt || c.mods.meta {
                return false;
            }
        }
        let Some(t) = text else {
            return false;
        };
        let s: String = t.chars().filter(|c| !c.is_control()).collect();
        if s.is_empty() {
            return false;
        }
        self.last_key_text = Some((s.clone(), std::time::Instant::now()));
        self.fileops_text(&s);
        true
    }

    /// Keys while a sheet is open (it takes them all); `text` is what the key types.
    pub(super) fn fileops_key(&mut self, combo: Option<&KeyCombo>, text: Option<&str>) {
        let Some(v) = self.fileops.view() else {
            return;
        };
        self.redraw = true;
        if v.focus == Part::Field && self.fileops_field_key(combo, text) {
            return;
        }
        let Some(c) = combo else { return };
        let order = v.focusables();
        let at = order.iter().position(|p| *p == v.focus).unwrap_or(0);
        let rtl = self.rtl();
        let step = |me: &mut App, by: isize| {
            if order.is_empty() {
                return;
            }
            let i = (at as isize + by).rem_euclid(order.len() as isize) as usize;
            if let Some(s) = me.fileops.sheet.as_mut() {
                s.focus = order[i];
            }
        };
        let plain = !c.mods.any();
        match c.key {
            Key::Named(NamedKey::Escape) => self.fileops_cancel(),
            Key::Named(NamedKey::Tab) => step(self, if c.mods.shift { -1 } else { 1 }),
            Key::Named(NamedKey::Enter) => match v.focus {
                Part::Field | Part::Choice(_) => self.fileops_hit(Part::Button(0)),
                p => self.fileops_hit(p),
            },
            Key::Named(NamedKey::Space) if v.focus != Part::Field => self.fileops_hit(v.focus),
            Key::Named(k @ (NamedKey::ArrowUp | NamedKey::ArrowDown)) if plain => {
                let down = k == NamedKey::ArrowDown;
                match v.focus {
                    Part::Row(_) | Part::Field | Part::Choice(_) => {
                        step(self, if down { 1 } else { -1 })
                    }
                    _ => self.fileops_scroll(if down { 1 } else { -1 }),
                }
            }
            Key::Named(k @ (NamedKey::PageUp | NamedKey::PageDown)) => {
                let n = view::MAX_ROWS as isize;
                self.fileops_scroll(if k == NamedKey::PageDown { n } else { -n });
            }
            Key::Named(k @ (NamedKey::ArrowLeft | NamedKey::ArrowRight)) if plain => {
                let forward = (k == NamedKey::ArrowRight) != rtl;
                match v.focus {
                    Part::Choice(i) => {
                        let n = v.choice.as_ref().map_or(1, |c| c.options.len());
                        let j = if forward {
                            (i + 1).min(n - 1)
                        } else {
                            i.saturating_sub(1)
                        };
                        self.fileops_hit(Part::Choice(j));
                    }
                    Part::Button(_) => step(self, if forward { 1 } else { -1 }),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    pub(super) fn fileops_wheel(&mut self, lines: f32) {
        let by = (-lines * 3.0).round() as isize;
        if by != 0 {
            self.fileops_scroll(by);
        }
    }

    fn fileops_scroll(&mut self, by: isize) {
        let Some(s) = self.fileops.sheet.as_mut() else {
            return;
        };
        let (top, total) = match &mut s.kind {
            Kind2::Batch(b) => {
                let n = b.rows.len();
                (&mut b.top, n)
            }
            Kind2::Import(i) => {
                let n = import_lines(i).len();
                (&mut i.top, n)
            }
            Kind2::Develop(d) => d.row_window(),
            Kind2::Album(a) => a.row_window(),
            _ => return,
        };
        let max = total.saturating_sub(view::MAX_ROWS);
        *top = (*top as isize + by).clamp(0, max as isize) as usize;
        self.redraw = true;
    }

    fn fileops_cancel(&mut self) {
        if let Some(Sheet {
            kind: Kind2::Transfer(t),
            focus,
            ..
        }) = self.fileops.sheet.as_mut()
            && t.confirm
        {
            t.confirm = false;
            *focus = Part::Button(0);
            self.redraw = true;
            return;
        }
        self.fileops_close();
    }

    /// A part pressed (click, Enter, Space, a screen reader).
    fn fileops_hit(&mut self, p: Part) {
        let Some(s) = self.fileops.sheet.as_mut() else {
            return;
        };
        s.focus = match p {
            Part::Button(_) => s.focus,
            _ => p,
        };
        self.redraw = true;
        match p {
            Part::Button(1) => self.fileops_cancel(),
            Part::Button(_) => match &s.kind {
                Kind2::Rename { .. } => self.fileops_rename_start(),
                Kind2::Batch(_) => self.fileops_batch_start(),
                Kind2::Transfer(_) => self.fileops_transfer_start(),
                Kind2::Import(_) => self.fileops_import_apply(),
                Kind2::Develop(d) => {
                    if d.ready() {
                        self.develop_sheet_ok();
                    }
                }
                Kind2::Album(a) => {
                    if a.ready() {
                        self.albums_sheet_ok();
                    }
                }
            },
            Part::Choice(i) => {
                if let Kind2::Transfer(t) = &mut s.kind {
                    t.conflict = [Conflict::Skip, Conflict::KeepBoth, Conflict::Replace][i.min(2)];
                    s.focus = Part::Choice(i);
                }
                if let Kind2::Develop(d) = &mut s.kind {
                    let last = d.choice_text.as_ref().map_or(1, |c| c.1.len().max(1) - 1);
                    d.mode = d.mode.map(|_| i.min(last));
                    s.focus = Part::Choice(i.min(last));
                }
                if let Kind2::Album(a) = &mut s.kind {
                    a.set_choice(i);
                    s.focus = Part::Choice(i.min(2));
                }
            }
            Part::Row(i) => {
                if let Kind2::Develop(d) = &mut s.kind {
                    d.toggle(i);
                }
                if let Kind2::Album(a) = &mut s.kind
                    && a.press(i)
                {
                    s.focus = Part::Field;
                }
                if let Kind2::Transfer(t) = &mut s.kind {
                    if let Some(f) = t.folders.get(i) {
                        t.chosen = Some(f.clone());
                        replan(t);
                    } else {
                        self.fileops_pick_folder();
                    }
                }
            }
            Part::Field => {}
        }
    }

    fn fileops_pick_folder(&mut self) {
        let Some(Sheet {
            kind: Kind2::Transfer(t),
            ..
        }) = self.fileops.sheet.as_mut()
        else {
            return;
        };
        if t.picking {
            return;
        }
        t.picking = true;
        let start = t.chosen.clone().or_else(|| {
            t.files
                .first()
                .and_then(|f| f.parent())
                .map(Path::to_path_buf)
        });
        let title = if t.mode == Mode::Copy {
            tr("transfer-copy-dialog")
        } else {
            tr("transfer-move-dialog")
        };
        let tx = self.fileops.tx.clone();
        self.loader.background("dlg-fileops", move |l| {
            let mut d = rfd::FileDialog::new().set_title(&title);
            if let Some(dir) = start {
                d = d.set_directory(dir);
            }
            let _ = tx.send(Msg::Picked(d.pick_folder()));
            l.waker()();
        });
    }

    pub(super) fn fileops_click(&mut self, p: (f32, f32), down: bool) {
        let (now, shift) = (self.now(), self.mods.shift_key());
        let Some(s) = self.fileops.sheet.as_mut() else {
            return;
        };
        if !down {
            if let Some(f) = self.fileops_field() {
                f.release();
            }
            return;
        }
        let hit = s
            .drawn
            .hits
            .iter()
            .rev()
            .find(|(r, _)| r.contains(p))
            .map(|x| x.1);
        let Some(h) = hit else { return };
        if h == Part::Field
            && let Some((block, x0)) = s.drawn.field_text.clone()
        {
            let at = block.hit_byte(p.0 - x0);
            s.focus = Part::Field;
            if let Some(f) = self.fileops_field() {
                f.press(at, p.0, now, shift);
            }
            self.redraw = true;
            return;
        }
        self.fileops_hit(h);
    }

    pub(super) fn fileops_drag(&mut self, p: (f32, f32)) {
        let Some(s) = self.fileops.sheet.as_ref() else {
            return;
        };
        let Some((block, x0)) = s.drawn.field_text.clone() else {
            return;
        };
        if let Some(f) = self.fileops_field()
            && f.dragging()
        {
            f.drag_to(block.hit_byte(p.0 - x0));
            self.redraw = true;
        }
    }

    pub(super) fn fileops_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        if self.fileops.sheet.is_none() {
            return false;
        }
        let Some(part) = view::a11y_target(req.target_node.0) else {
            return (view::A11Y_BASE..view::A11Y_BASE + 0x1000).contains(&req.target_node.0);
        };
        self.redraw = true;
        match (req.action, part) {
            (accesskit::Action::SetValue, Part::Field) => {
                if let Some(accesskit::ActionData::Value(s)) = &req.data {
                    let s = s.to_string();
                    if let Some(f) = self.fileops_field() {
                        f.set_text(&s);
                    }
                    if let Some(Sheet {
                        kind: Kind2::Batch(b),
                        ..
                    }) = self.fileops.sheet.as_mut()
                    {
                        rebuild_rows(b);
                    }
                }
            }
            (accesskit::Action::Focus, p) => {
                if let Some(s) = self.fileops.sheet.as_mut() {
                    s.focus = p;
                }
            }
            (accesskit::Action::Click, p) => self.fileops_hit(p),
            _ => {}
        }
        true
    }
}

/// The progress toast of a copy or move.
fn progress_text(r: &Running) -> String {
    let pct = r
        .last
        .as_ref()
        .filter(|p| p.total > 0)
        .map_or(0, |p| (p.done * 100 / p.total).min(100));
    let id = if r.mode == Mode::Copy {
        "fileop-progress-copy"
    } else {
        "fileop-progress-move"
    };
    tr_args(
        id,
        &[("count", r.count.to_string()), ("percent", pct.to_string())],
    )
}

/// The plan for the chosen folder (which names are taken).
fn replan(t: &mut TransferSheet) {
    t.items = match &t.chosen {
        Some(dir) => transfer::plan(&t.files, dir),
        None => Vec::new(),
    };
}

/// Message ids this module builds at run time (the catalogue test checks them).
#[cfg(test)]
pub(super) fn dynamic_ids() -> Vec<&'static str> {
    let mut v = vec![
        "fileop-progress-copy",
        "fileop-progress-move",
        "rename-problem-duplicate",
    ];
    for p in [
        NameProblem::Empty,
        NameProblem::InvalidChar('/'),
        NameProblem::Reserved(String::new()),
        NameProblem::TooLong,
        NameProblem::DotName,
        NameProblem::TrailingDotOrSpace,
        NameProblem::Exists,
        NameProblem::ReadOnlyFolder,
    ] {
        v.push(p.message_id());
    }
    for k in [
        Kind::Rename,
        Kind::BatchRename,
        Kind::Move,
        Kind::Copy,
        Kind::Rotate,
    ] {
        v.push(k.message_id());
    }
    let p = PathBuf::new();
    for e in [
        UndoError::Changed(p.clone()),
        UndoError::Missing(p.clone()),
        UndoError::Taken(p.clone()),
        UndoError::Io(p, String::new()),
    ] {
        v.push(e.message_id());
    }
    for e in [
        RotateError::Unsupported,
        RotateError::TooLarge,
        RotateError::Damaged(String::new()),
        RotateError::Io(String::new()),
    ] {
        v.push(e.message_id());
    }
    for m in [
        Method::Lossless { trimmed: None },
        Method::Lossless {
            trimmed: Some((1, 1)),
        },
        Method::Tag { reason: None },
        Method::Sidecar,
    ] {
        v.push(m.message_id());
    }
    for r in [
        TagReason::Unreadable,
        TagReason::MultiPicture,
        TagReason::Trailer,
        TagReason::NotAligned {
            width: 1,
            height: 1,
        },
    ] {
        v.push(r.message_id());
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_built_id_is_in_english() {
        for id in dynamic_ids() {
            assert!(specular_i18n::english_has(id), "{id} missing from i18n/en");
        }
    }

    #[test]
    fn recent_folders_keep_order_and_limit() {
        let mut s = String::new();
        for i in 0..12 {
            s = remember(&s, Path::new(&format!("/f{i}")));
        }
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines.len(), MAX_RECENT);
        assert_eq!(lines[0], "/f11");
        let s = remember(&s, Path::new("/f5"));
        assert_eq!(s.lines().next(), Some("/f5"));
        assert_eq!(s.lines().filter(|l| *l == "/f5").count(), 1);
    }

    #[test]
    fn effects_read_both_ways() {
        let mut op = Op::new(Kind::Rename);
        let stamp = specular_fileops::Stamp {
            len: 1,
            modified: None,
        };
        op.steps.push(Step::Moved {
            from: "/a/x.jpg".into(),
            to: "/a/y.jpg".into(),
            stamp,
        });
        let (m, _, _) = effects(&op, false);
        assert_eq!(m, vec![("/a/x.jpg".into(), "/a/y.jpg".into())]);
        let (m, _, _) = effects(&op, true);
        assert_eq!(m, vec![("/a/y.jpg".into(), "/a/x.jpg".into())]);
    }

    /// The problems, rotations and progress pick their message at run time; each passes
    /// what its English message uses (`message_args.rs` reads only literal ids).
    #[test]
    fn fileops_messages_get_their_arguments() {
        specular_i18n::record_argument_problems(true);
        for p in [
            NameProblem::Empty,
            NameProblem::InvalidChar('/'),
            NameProblem::InvalidChar('\u{1}'),
            NameProblem::Reserved("CON".into()),
            NameProblem::TooLong,
            NameProblem::DotName,
            NameProblem::TrailingDotOrSpace,
            NameProblem::Exists,
            NameProblem::ReadOnlyFolder,
        ] {
            let _ = name_problem_text(&p);
        }
        let f = PathBuf::from("/a/x.jpg");
        for e in [
            UndoError::Changed(f.clone()),
            UndoError::Missing(f.clone()),
            UndoError::Taken(f.clone()),
            UndoError::Io(f.clone(), "denied".into()),
        ] {
            let _ = undo_problem_text(&e);
        }
        for e in [
            RotateError::Unsupported,
            RotateError::TooLarge,
            RotateError::Damaged("bad".into()),
            RotateError::Io("denied".into()),
        ] {
            let _ = rotate_problem_text(&e, "x.jpg");
        }
        let tags = [
            None,
            Some(TagReason::Unreadable),
            Some(TagReason::MultiPicture),
            Some(TagReason::Trailer),
            Some(TagReason::NotAligned {
                width: 8,
                height: 16,
            }),
        ];
        let methods = [
            Method::Lossless { trimmed: None },
            Method::Lossless {
                trimmed: Some((8, 16)),
            },
            Method::Sidecar,
        ]
        .into_iter()
        .chain(tags.map(|reason| Method::Tag { reason }));
        for method in methods {
            let r = Rotated {
                method,
                op: Op::new(Kind::Rename),
                orientation: 6,
                written: PathBuf::from("/a/x.xmp"),
            };
            let _ = rotated_text(&r, "x.jpg");
        }
        for mode in [Mode::Copy, Mode::Move] {
            let _ = progress_text(&Running {
                cancel: Arc::new(AtomicBool::new(false)),
                toast: None,
                toast_at: 0.0,
                last: Some(Progress {
                    done: 1,
                    total: 3,
                    file: f.clone(),
                }),
                mode,
                count: 3,
            });
        }
        let problems = specular_i18n::argument_problems();
        specular_i18n::record_argument_problems(false);
        assert!(problems.is_empty(), "{problems:#?}");
    }
}
