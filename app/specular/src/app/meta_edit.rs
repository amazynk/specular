//! The metadata editor (`Ctrl+Alt+I`, the palette's "Edit metadata", the gallery's menu and
//! the Info panel's "Edit metadata…"): the IPTC fields photographers fill in, for the
//! selected photos in the gallery and the Library, the photo on screen in cull mode, or the
//! image in the viewer. Docked at the trailing edge ([`side_panel`]).
//!
//! - **Fields** (`specular_metadata::edit::Field`), grouped: description, creator and contact,
//!   rights, location shown, location created, workflow. Rating and label stay on their own
//!   keys and marks.
//! - **Several photos:** a field all of them agree on shows its value; one they differ on
//!   shows "Mixed" and is changed only when typed into (Backspace in the empty field clears
//!   it on all). Keywords show those all have; typing adds or takes off only those.
//! - **Saving:** a field is written when it is left (Enter, Tab, the arrows, a click
//!   elsewhere, closing): only that field, to every photo, read afresh ([`files`]).
//! - **Templates:** named sets of values (copyright and creator, say) at the top; a click
//!   applies one to the photos. "New template" and "Edit template" edit one with the same
//!   fields; its keywords are added to the photos' own or replace them.
//!
//! Writing goes to XMP sidecars, or with `gallery.metadata_into_files` into JPEG, TIFF and DNG
//! files themselves ([`files`]). The Library's catalogue is told what was written.

mod files;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crossbeam_channel::{Receiver, Sender};
use specular_gfx::winit::event::KeyEvent;
use specular_gfx::{DrawList, Rect, Text};
use specular_i18n::t;
use specular_input::{Key, Mods, NamedKey};
use specular_metadata::edit::template::{KeywordMode, Template};
use specular_metadata::edit::{self, Edit, Field, Group, KeywordEdit, Record};
use specular_metadata::keywords as kw;
use specular_theme::Palette;
use specular_widgets::accesskit;
use specular_widgets::line_edit::{LineEdit, PendingClipboard};

use super::App;
use super::side_panel::{self, Check, Metrics, Painter};

/// AccessKit ids of the panel.
pub(super) const A11Y_META: u64 = 0x7400_0000_0000_0000;
/// The selection is looked at again at most this often (s).
const TARGETS_S: f64 = 0.25;
/// A big selection is read in parts this large.
const READ_PART: usize = 256;

/// A section of the body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Section {
    Templates,
    Group(Group),
    /// The template being edited.
    Template,
}

/// One row of the body.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Row {
    Header(Section),
    Note(String),
    /// A saved template (applies it).
    Template(usize),
    /// The edited template's name.
    Name,
    Field(Field),
    /// The edited template's keywords replace the photos' own.
    KeywordMode,
}

impl Row {
    fn actionable(&self) -> bool {
        matches!(
            self,
            Row::Template(_) | Row::Name | Row::Field(_) | Row::KeywordMode
        )
    }

    fn editable(&self) -> bool {
        matches!(self, Row::Name | Row::Field(_))
    }
}

/// A control of the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hit {
    Panel,
    Close,
    Row(usize),
    Foot(Button),
}

/// A button at the panel's foot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Button {
    NewTemplate,
    EditTemplate,
    RemoveTemplate,
    Done,
    ConfirmRemove,
    Cancel,
}

/// What the fields show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Photos,
    /// The template at this index.
    Template(usize),
}

/// One field's editing state.
#[derive(Clone, Debug, Default)]
pub(crate) struct Slot {
    pub edit: LineEdit,
    /// What it showed before typing.
    pub original: String,
    /// The photos differ on it.
    pub mixed: bool,
    /// Backspace in an empty mixed field: clear it on every photo.
    pub clearing: bool,
    /// Keywords: some photos have others than those shown.
    pub others: bool,
}

impl Slot {
    fn changed(&self) -> bool {
        self.clearing || self.edit.text().trim() != self.original.trim()
    }
}

/// Work done off the UI thread.
pub(crate) enum Msg {
    Read(Vec<(PathBuf, Record)>),
    Wrote(Vec<(PathBuf, Result<Record, String>)>, Vec<Field>),
}

/// What a key or click did.
#[derive(Debug, PartialEq)]
pub(crate) enum KeyOut {
    /// Not the panel's.
    No,
    Used,
    /// Write this to the photos.
    Write(Edit),
    /// Apply the template at this index to the photos.
    Apply(usize),
    /// The templates changed: save them.
    Templates,
    Close,
    Button(Button),
}

/// The panel and what it knows.
pub(crate) struct MetaEdit {
    pub open: bool,
    pub mode: Mode,
    /// Every field's state, in [`Field::all`] order.
    pub slots: Vec<(Field, Slot)>,
    /// The edited template's name.
    pub name: LineEdit,
    pub rows: Vec<Row>,
    /// The row with the keyboard focus.
    pub focus: Option<usize>,
    /// The focused row's field has the keyboard.
    pub editing: bool,
    /// The template the foot's buttons act on (the focused or last clicked one).
    pub current: Option<usize>,
    pub scroll: f32,
    pub rect: Option<Rect>,
    pub hits: Vec<(Rect, Hit)>,
    /// A template waiting for "Remove" to go.
    pub confirm: Option<usize>,
    pub status: Option<String>,
    /// The photos it acts on.
    pub targets: Vec<PathBuf>,
    targets_at: f64,
    /// Fields of photos read or written.
    pub files: HashMap<PathBuf, Record>,
    reading: HashSet<PathBuf>,
    pub templates: Vec<Template>,
    /// Writing goes into JPEG, TIFF and DNG files too (`gallery.metadata_into_files`).
    pub into_files: bool,
    /// The config folder (None: nothing is saved, as in screenshots and tests).
    dir: Option<PathBuf>,
    loaded: bool,
    pub tx: Sender<Msg>,
    pub rx: Receiver<Msg>,
    pub writing: usize,
}

impl Default for MetaEdit {
    fn default() -> MetaEdit {
        let (tx, rx) = crossbeam_channel::unbounded();
        MetaEdit {
            open: false,
            mode: Mode::Photos,
            slots: Field::all().map(|f| (f, Slot::default())).collect(),
            name: LineEdit::new(""),
            rows: Vec::new(),
            focus: None,
            editing: false,
            current: None,
            scroll: 0.0,
            rect: None,
            hits: Vec::new(),
            confirm: None,
            status: None,
            targets: Vec::new(),
            targets_at: f64::NEG_INFINITY,
            files: HashMap::new(),
            reading: HashSet::new(),
            templates: Vec::new(),
            into_files: false,
            dir: None,
            loaded: false,
            tx,
            rx,
            writing: 0,
        }
    }
}

/// A field's values as typed: lists joined with "; ", keyword levels with " > ".
fn shown(f: Field, values: &[String]) -> String {
    let v: Vec<String> = if f == Field::Keywords {
        values.iter().map(|k| k.replace(kw::SEP, " > ")).collect()
    } else {
        values.to_vec()
    };
    specular_widgets::line_edit::single_line(&v.join("; "))
}

/// The string id of a field's label.
fn field_label(f: Field) -> String {
    specular_i18n::I18n::global().tr(&format!("metaedit-field-{}", f.key()))
}

fn group_label(g: Group) -> String {
    match g {
        Group::Description => t!("metaedit-group-description"),
        Group::Creator => t!("metaedit-group-creator"),
        Group::Rights => t!("metaedit-group-rights"),
        Group::Shown => t!("metaedit-group-shown"),
        Group::Created => t!("metaedit-group-created"),
        Group::Workflow => t!("metaedit-group-workflow"),
    }
}

impl MetaEdit {
    /// The panel has the keyboard.
    pub fn has_focus(&self) -> bool {
        self.open && (self.editing || self.focus.is_some() || self.confirm.is_some())
    }

    /// Takes the saved templates from `dir` the first time.
    pub fn load(&mut self, dir: Option<PathBuf>) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        if let Some(d) = &dir {
            self.templates = files::load(d);
        }
        self.dir = dir;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.editing = false;
        self.focus = None;
        self.confirm = None;
        self.mode = Mode::Photos;
        self.rect = None;
        self.hits.clear();
    }

    pub fn slot(&self, f: Field) -> &Slot {
        &self
            .slots
            .iter()
            .find(|(g, _)| *g == f)
            .expect("every field has a slot")
            .1
    }

    fn slot_mut(&mut self, f: Field) -> &mut Slot {
        &mut self
            .slots
            .iter_mut()
            .find(|(g, _)| *g == f)
            .expect("every field has a slot")
            .1
    }

    /// The field being typed into, if any.
    fn editing_field(&self) -> Option<Field> {
        match self.focus.and_then(|k| self.rows.get(k)) {
            Some(Row::Field(f)) if self.editing => Some(*f),
            _ => None,
        }
    }

    /// The photos read, of those it acts on.
    fn loaded(&self) -> Vec<&Record> {
        self.targets
            .iter()
            .filter_map(|p| self.files.get(p))
            .collect()
    }

    /// The fields' text from the photos (or the template), leaving alone the one being
    /// typed into once it was changed.
    pub fn refresh(&mut self) {
        let keep = self.editing_field().filter(|f| self.slot(*f).changed());
        match self.mode {
            Mode::Template(i) => {
                let Some(t) = self.templates.get(i).cloned() else {
                    self.mode = Mode::Photos;
                    return self.refresh();
                };
                for (f, s) in &mut self.slots {
                    if Some(*f) == keep {
                        continue;
                    }
                    let text = shown(*f, t.get(*f).unwrap_or(&[]));
                    *s = Slot {
                        original: text.clone(),
                        edit: LineEdit::new(&text),
                        ..Slot::default()
                    };
                }
            }
            Mode::Photos => {
                let recs: Vec<Record> = self.loaded().into_iter().cloned().collect();
                for (f, s) in &mut self.slots {
                    if Some(*f) == keep {
                        continue;
                    }
                    let (text, mixed, others) = if recs.is_empty() {
                        (String::new(), false, false)
                    } else if *f == Field::Keywords {
                        let lists: Vec<&[String]> =
                            recs.iter().map(|r| r.values(Field::Keywords)).collect();
                        let common: Vec<String> = lists[0]
                            .iter()
                            .filter(|k| lists.iter().all(|l| l.iter().any(|x| kw::same(x, k))))
                            .cloned()
                            .collect();
                        let others = lists.iter().any(|l| l.len() > common.len());
                        (shown(*f, &common), false, others)
                    } else {
                        let first = recs[0].values(*f);
                        if recs.iter().all(|r| r.values(*f) == first) {
                            (shown(*f, first), false, false)
                        } else {
                            (String::new(), true, false)
                        }
                    };
                    if s.original != text || s.mixed != mixed || s.changed() {
                        *s = Slot {
                            original: text.clone(),
                            edit: LineEdit::new(&text),
                            mixed,
                            others,
                            clearing: false,
                        };
                    } else {
                        s.others = others;
                    }
                }
            }
        }
    }

    /// The rows for the mode.
    pub fn build_rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        match self.mode {
            Mode::Photos => {
                rows.push(Row::Header(Section::Templates));
                if self.templates.is_empty() {
                    rows.push(Row::Note(t!("metaedit-templates-none")));
                }
                rows.extend((0..self.templates.len()).map(Row::Template));
                if self.targets.is_empty() {
                    rows.push(Row::Note(t!("metaedit-none-selected")));
                    return rows;
                }
                if self.loaded().len() < self.targets.len() {
                    rows.push(Row::Note(t!("metaedit-reading")));
                }
            }
            Mode::Template(_) => {
                rows.push(Row::Header(Section::Template));
                rows.push(Row::Name);
                rows.push(Row::Note(t!("metaedit-template-hint")));
            }
        }
        let mut group = None;
        for f in Field::all() {
            if group != Some(f.group()) {
                group = Some(f.group());
                rows.push(Row::Header(Section::Group(f.group())));
            }
            rows.push(Row::Field(f));
            if f == Field::Keywords && matches!(self.mode, Mode::Template(_)) {
                rows.push(Row::KeywordMode);
            }
        }
        if self.mode == Mode::Photos {
            rows.push(Row::Note(if self.into_files {
                t!("metaedit-writes-files")
            } else {
                t!("metaedit-writes-sidecars")
            }));
        }
        rows
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

    /// Puts the focus on row `k` (a field takes the keyboard, caret at the end).
    fn focus_row(&mut self, k: Option<usize>) {
        self.focus = k;
        let row = k.and_then(|k| self.rows.get(k)).cloned();
        self.editing = row.as_ref().is_some_and(Row::editable);
        if let Some(Row::Template(i)) = row {
            self.current = Some(i);
        }
        match row {
            Some(Row::Field(f)) => {
                let s = self.slot_mut(f);
                let n = s.edit.text().len();
                s.edit.set_caret(n, false);
            }
            Some(Row::Name) => {
                let n = self.name.text().len();
                self.name.set_caret(n, false);
            }
            _ => {}
        }
    }

    /// Ends typing in the focused field: what to write, if it changed.
    pub fn commit(&mut self) -> Option<KeyOut> {
        let row = self.focus.and_then(|k| self.rows.get(k)).cloned();
        match (row, self.mode) {
            (Some(Row::Name), Mode::Template(i)) => {
                let name = self.name.text().trim().to_owned();
                let t = self.templates.get_mut(i)?;
                if name.is_empty() || name == t.name {
                    self.name.set_text(&t.name.clone());
                    return None;
                }
                t.name = name;
                Some(KeyOut::Templates)
            }
            (Some(Row::Field(f)), Mode::Template(i)) => {
                let s = self.slot(f);
                if !s.changed() {
                    return None;
                }
                let text = s.edit.text().to_owned();
                let t = self.templates.get_mut(i)?;
                t.set(f, edit::parse_text(f, &text));
                let shown_now = shown(f, t.get(f).unwrap_or(&[]));
                let s = self.slot_mut(f);
                s.original = shown_now.clone();
                s.edit.set_text(&shown_now);
                Some(KeyOut::Templates)
            }
            (Some(Row::Field(f)), Mode::Photos) => {
                let s = self.slot(f).clone();
                if !s.changed() || self.targets.is_empty() {
                    return None;
                }
                let values = if s.clearing {
                    Vec::new()
                } else {
                    edit::parse_text(f, s.edit.text())
                };
                let e = if f == Field::Keywords {
                    let old = edit::parse_text(f, &s.original);
                    let k = if s.clearing {
                        KeywordEdit::Replace(Vec::new())
                    } else {
                        let mut c: Vec<kw::Change> = values
                            .iter()
                            .filter(|k| !old.iter().any(|o| kw::same(o, k)))
                            .map(|k| kw::Change::Add(k.clone()))
                            .collect();
                        c.extend(
                            old.iter()
                                .filter(|o| !values.iter().any(|k| kw::same(o, k)))
                                .map(|o| kw::Change::Remove(o.clone())),
                        );
                        KeywordEdit::Changes(c)
                    };
                    Edit {
                        set: Vec::new(),
                        keywords: Some(k),
                    }
                } else {
                    Edit {
                        set: vec![(f, values)],
                        keywords: None,
                    }
                };
                self.assume(&e);
                let s = self.slot_mut(f);
                let text = specular_widgets::line_edit::single_line(s.edit.text().trim());
                s.original = text.clone();
                s.edit.set_text(&text);
                s.mixed = false;
                s.clearing = false;
                Some(KeyOut::Write(e))
            }
            _ => None,
        }
    }

    /// Puts the photos' fields as assumed after `e` (the write's result follows).
    pub fn assume(&mut self, e: &Edit) {
        for t in &self.targets {
            if let Some(r) = self.files.get_mut(t) {
                *r = e.apply(r);
            }
        }
    }

    /// Moves the focus by `dir` rows, ending the typing in the field left.
    fn move_focus(&mut self, dir: isize) -> KeyOut {
        let out = self.commit();
        if let Some(k) = self.step(self.focus, dir) {
            self.focus_row(Some(k));
        }
        out.unwrap_or(KeyOut::Used)
    }

    /// Activating row `k` (Enter, Space, a click).
    fn activate(&mut self, k: usize) -> KeyOut {
        match self.rows.get(k) {
            Some(Row::Template(i)) if !self.targets.is_empty() => KeyOut::Apply(*i),
            Some(Row::KeywordMode) => {
                if let Mode::Template(i) = self.mode
                    && let Some(t) = self.templates.get_mut(i)
                {
                    t.keyword_mode = match t.keyword_mode {
                        KeywordMode::Add => KeywordMode::Replace,
                        KeywordMode::Replace => KeywordMode::Add,
                    };
                    return KeyOut::Templates;
                }
                KeyOut::Used
            }
            _ => KeyOut::Used,
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
                Key::Named(NamedKey::Enter) => KeyOut::Button(Button::ConfirmRemove),
                Key::Named(NamedKey::Escape) => KeyOut::Button(Button::Cancel),
                _ => KeyOut::Used,
            };
        }
        if let Key::Named(n) = key
            && !mods.ctrl
            && !mods.alt
            && !mods.meta
        {
            match n {
                NamedKey::Escape => {
                    let f = self.editing_field();
                    if let Some(f) = f.filter(|f| self.slot(*f).changed()) {
                        // Back to what it was.
                        let s = self.slot_mut(f);
                        s.clearing = false;
                        let o = s.original.clone();
                        s.edit.set_text(&o);
                    } else if self.mode != Mode::Photos && self.focus.is_some() {
                        self.focus = None;
                        self.editing = false;
                    } else {
                        // The grid (or the photo) has the keyboard again.
                        self.focus = None;
                        self.editing = false;
                    }
                    return KeyOut::Used;
                }
                NamedKey::ArrowDown => return self.move_focus(1),
                NamedKey::ArrowUp => return self.move_focus(-1),
                NamedKey::Tab => return self.move_focus(if mods.shift { -1 } else { 1 }),
                NamedKey::Enter if self.editing => return self.move_focus(1),
                NamedKey::Enter | NamedKey::Space if !self.editing => {
                    return match self.focus {
                        Some(k) => self.activate(k),
                        None => KeyOut::Used,
                    };
                }
                NamedKey::Backspace | NamedKey::Delete => {
                    if let Some(f) = self.editing_field() {
                        let s = self.slot_mut(f);
                        if s.mixed && s.edit.is_empty() {
                            s.clearing = true;
                            return KeyOut::Used;
                        }
                    }
                }
                _ => {}
            }
        }
        if !self.editing {
            return if plain { KeyOut::Used } else { KeyOut::No };
        }
        let field = self.editing_field();
        let used = {
            let edit = match self.focus.and_then(|k| self.rows.get(k)).cloned() {
                Some(Row::Field(f)) => &mut self.slot_mut(f).edit,
                Some(Row::Name) => &mut self.name,
                _ => return KeyOut::Used,
            };
            let typed: String = text
                .filter(|_| plain)
                .map(|t| t.chars().filter(|c| !c.is_control()).collect())
                .unwrap_or_default();
            if edit.key(key, mods, false, clip).used() {
                true
            } else if !typed.is_empty() {
                edit.insert(&typed);
                true
            } else {
                false
            }
        };
        if used {
            if let Some(f) = field {
                self.slot_mut(f).clearing = false;
            }
            return KeyOut::Used;
        }
        // Other keys with Ctrl or Alt (the palette, undo) go on; plain ones stay here.
        if plain { KeyOut::Used } else { KeyOut::No }
    }

    /// Text typed by a key without a shortcut form (a dead key's result, an IME's).
    pub fn type_text(&mut self, t: &str) -> KeyOut {
        if !self.has_focus() || self.confirm.is_some() || !self.editing {
            return KeyOut::No;
        }
        let s: String = t.chars().filter(|c| !c.is_control()).collect();
        if s.is_empty() {
            return KeyOut::No;
        }
        self.paste(&s);
        KeyOut::Used
    }

    /// Text into the field being typed into.
    pub fn paste(&mut self, s: &str) {
        match self.focus.and_then(|k| self.rows.get(k)).cloned() {
            Some(Row::Field(f)) => {
                let slot = self.slot_mut(f);
                slot.edit
                    .paste(&specular_widgets::line_edit::single_line(s));
                slot.clearing = false;
            }
            Some(Row::Name) => {
                self.name
                    .paste(&specular_widgets::line_edit::single_line(s));
            }
            _ => {}
        }
    }

    /// A click on `hit`.
    pub fn click(&mut self, hit: Hit) -> KeyOut {
        match hit {
            Hit::Close => KeyOut::Close,
            Hit::Row(k) => {
                if self.focus == Some(k) && self.editing {
                    return KeyOut::Used;
                }
                let out = self.commit();
                self.focus_row(Some(k));
                match out {
                    Some(o) => o,
                    None => {
                        if self.rows.get(k).is_some_and(|r| !r.editable()) {
                            self.activate(k)
                        } else {
                            KeyOut::Used
                        }
                    }
                }
            }
            Hit::Foot(b) => KeyOut::Button(b),
            Hit::Panel => KeyOut::Used,
        }
    }

    /// Runs a foot button (the field being typed into is written first, by the caller).
    /// What follows (saving the templates) comes back.
    pub fn button(&mut self, b: Button) -> KeyOut {
        match b {
            Button::NewTemplate => {
                let n = self.templates.len() + 1;
                let name = t!("metaedit-template-default-name", n = n as i64);
                self.templates.push(Template::new(&name));
                self.edit_template(self.templates.len() - 1);
                KeyOut::Templates
            }
            Button::EditTemplate => {
                if let Some(i) = self.current.filter(|i| *i < self.templates.len()) {
                    self.edit_template(i);
                }
                KeyOut::Used
            }
            Button::RemoveTemplate => {
                let i = match self.mode {
                    Mode::Template(i) => Some(i),
                    Mode::Photos => self.current,
                };
                self.confirm = i.filter(|i| *i < self.templates.len());
                KeyOut::Used
            }
            Button::Done => {
                self.mode = Mode::Photos;
                self.focus = None;
                self.editing = false;
                self.scroll = 0.0;
                self.refresh();
                KeyOut::Used
            }
            Button::ConfirmRemove => {
                let Some(i) = self.confirm.take() else {
                    return KeyOut::Used;
                };
                let t = self.templates.remove(i);
                self.status = Some(t!("metaedit-template-removed", name = t.name));
                self.mode = Mode::Photos;
                self.current = None;
                self.focus = None;
                self.editing = false;
                self.refresh();
                KeyOut::Templates
            }
            Button::Cancel => {
                self.confirm = None;
                KeyOut::Used
            }
        }
    }

    /// Edits template `i` with the fields.
    fn edit_template(&mut self, i: usize) {
        self.mode = Mode::Template(i);
        self.current = Some(i);
        self.name = LineEdit::new(&self.templates[i].name);
        self.name.select_all();
        self.scroll = 0.0;
        self.refresh();
        self.rows = self.build_rows();
        let k = self.rows.iter().position(|r| *r == Row::Name);
        self.focus = k;
        self.editing = true;
    }

    /// Fields read or written: kept, and shown.
    fn learn(&mut self, read: Vec<(PathBuf, Record)>) {
        for (p, r) in read {
            self.reading.remove(&p);
            self.files.insert(p, r);
        }
        self.refresh();
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

    /// Row heights.
    fn heights(&self, m: &Metrics) -> Vec<f32> {
        self.rows
            .iter()
            .map(|r| match r {
                Row::Header(_) => m.head_h,
                Row::Field(_) | Row::Name => m.u(20.0) + m.field_h + m.u(6.0),
                Row::Note(_) => m.row_h + m.u(4.0),
                _ => m.row_h,
            })
            .collect()
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
            self.focus = None;
            self.editing = false;
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
        let (title, note) = match self.mode {
            Mode::Photos => (
                t!("metaedit-title"),
                self.status
                    .clone()
                    .or_else(|| (photos > 0).then(|| t!("metaedit-photos", count = photos as i64))),
            ),
            Mode::Template(_) => (t!("metaedit-template-title"), None),
        };
        let (close, top) = p.frame(&title, note.as_deref());
        self.hits.push((close, Hit::Close));
        let foot_rows = if self.confirm.is_some() { 2.0 } else { 1.0 };
        let body_top = top;
        let body_bottom = panel.bottom() - foot_rows * m.foot_h;
        let body = Rect::new(
            panel.x,
            body_top,
            panel.w,
            (body_bottom - body_top).max(0.0),
        );
        let heights = self.heights(&m);
        let content: f32 = heights.iter().sum::<f32>() + m.pad;
        if let Some(k) = self.focus {
            let y: f32 = heights[..k].iter().sum();
            self.scroll = side_panel::scroll_to(self.scroll, y, heights[k], body.h);
        }
        self.scroll = self.scroll.clamp(0.0, (content - body.h).max(0.0));
        p.dl.push_clip(p.flip(body));
        let mut y = body.y - self.scroll;
        for (k, row) in self.rows.iter().enumerate() {
            let h = heights[k];
            if y + h <= body.y || y >= body.bottom() {
                y += h;
                continue;
            }
            let lr = Rect::new(body.x + m.u(6.0), y, body.w - m.u(12.0), h);
            let rr = p.flip(lr);
            let focused = self.focus == Some(k);
            match row {
                Row::Header(s) => {
                    let name = match s {
                        Section::Templates => t!("metaedit-group-templates"),
                        Section::Group(g) => group_label(*g),
                        Section::Template => t!("metaedit-template-title"),
                    };
                    p.header(y, &name);
                }
                Row::Note(s) => {
                    let r = Rect::new(lr.x + m.u(8.0), y, lr.w - m.u(16.0), h);
                    let s = side_panel::fit(p.text, s, m.small, r.w);
                    p.label(&s, m.small, r, pal.text_muted);
                }
                Row::Template(i) => {
                    p.row_back(rr, focused);
                    let r = Rect::new(lr.x + m.u(8.0), y, lr.w - m.u(16.0), h);
                    let name = self.templates.get(*i).map_or("", |t| t.name.as_str());
                    let s = side_panel::fit(p.text, name, m.row, r.w);
                    let c = if self.current == Some(*i) {
                        pal.accent
                    } else {
                        pal.text
                    };
                    p.label(&s, m.row, r, c);
                    self.hits.push((rr.intersect(&p.flip(body)), Hit::Row(k)));
                }
                Row::KeywordMode => {
                    p.row_back(rr, focused);
                    let on = match self.mode {
                        Mode::Template(i) => self
                            .templates
                            .get(i)
                            .is_some_and(|t| t.keyword_mode == KeywordMode::Replace),
                        Mode::Photos => false,
                    };
                    let bs = m.u(14.0);
                    let x0 = lr.x + m.u(8.0);
                    p.check(
                        p.flip(Rect::new(x0, y + (h - bs) * 0.5, bs, bs)),
                        if on { Check::On } else { Check::Off },
                    );
                    let tx = x0 + bs + m.u(8.0);
                    let r = Rect::new(tx, y, (lr.right() - m.u(8.0) - tx).max(0.0), h);
                    let s = side_panel::fit(
                        p.text,
                        &t!("metaedit-template-replace-keywords"),
                        m.small,
                        r.w,
                    );
                    p.label(&s, m.small, r, pal.text);
                    self.hits.push((rr.intersect(&p.flip(body)), Hit::Row(k)));
                }
                Row::Name | Row::Field(_) => {
                    let (label, edit, placeholder, note) = match row {
                        Row::Field(f) => {
                            let s = self.slot(*f);
                            let ph = if s.clearing {
                                t!("metaedit-clearing")
                            } else if s.mixed {
                                t!("metaedit-mixed")
                            } else {
                                String::new()
                            };
                            let note = (s.others && self.mode == Mode::Photos)
                                .then(|| t!("metaedit-keywords-others"));
                            (field_label(*f), &s.edit, ph, note)
                        }
                        _ => (
                            t!("metaedit-template-name"),
                            &self.name,
                            String::new(),
                            None,
                        ),
                    };
                    let lab = Rect::new(
                        panel.x + m.pad,
                        y + m.u(2.0),
                        panel.w - 2.0 * m.pad,
                        m.u(18.0),
                    );
                    let used = {
                        let s = side_panel::fit(p.text, &label, m.small, lab.w);
                        p.label(&s, m.small, lab, pal.text_muted)
                    };
                    if let Some(n) = note {
                        let nr = Rect::new(
                            lab.x + used + m.u(6.0),
                            lab.y,
                            (lab.w - used - m.u(6.0)).max(0.0),
                            lab.h,
                        );
                        let n = side_panel::fit(p.text, &format!("· {n}"), m.small, nr.w);
                        p.label(&n, m.small, nr, pal.text_faint);
                    }
                    p.field(
                        y + m.u(20.0),
                        edit,
                        &placeholder,
                        focused && self.editing && self.open,
                    );
                    self.hits.push((rr.intersect(&p.flip(body)), Hit::Row(k)));
                }
            }
            y += h;
        }
        p.dl.pop_clip();
        let y1 = body_bottom;
        if let Some(i) = self.confirm {
            let name = self
                .templates
                .get(i)
                .map_or(String::new(), |t| t.name.clone());
            let r = Rect::new(panel.x + m.pad, y1, panel.w - 2.0 * m.pad, m.foot_h * 0.5);
            let q = t!("metaedit-template-remove-ask", name = name);
            let q = side_panel::fit(p.text, &q, m.small, r.w);
            p.label(&q, m.small, r, pal.text);
            let r2 = Rect::new(r.x, y1 + m.foot_h * 0.5, r.w, m.foot_h * 0.5);
            let keep = side_panel::fit(p.text, &t!("metaedit-template-remove-keep"), m.small, r2.w);
            p.label(&keep, m.small, r2, pal.text_muted);
            let b = p.foot(
                y1 + m.foot_h,
                &[
                    (&t!("metaedit-template-remove-yes"), false, true),
                    (&t!("app-cancel"), false, true),
                ],
            );
            self.hits.push((b[0], Hit::Foot(Button::ConfirmRemove)));
            self.hits.push((b[1], Hit::Foot(Button::Cancel)));
            return;
        }
        match self.mode {
            Mode::Photos => {
                let on = self.current.is_some_and(|i| i < self.templates.len());
                let b = p.foot(
                    y1,
                    &[
                        (&t!("metaedit-template-new"), false, true),
                        (&t!("metaedit-template-edit"), false, on),
                        (&t!("metaedit-template-remove"), false, on),
                    ],
                );
                self.hits.push((b[0], Hit::Foot(Button::NewTemplate)));
                if on {
                    self.hits.push((b[1], Hit::Foot(Button::EditTemplate)));
                    self.hits.push((b[2], Hit::Foot(Button::RemoveTemplate)));
                }
            }
            Mode::Template(_) => {
                let b = p.foot(
                    y1,
                    &[
                        (&t!("metaedit-template-done"), false, true),
                        (&t!("metaedit-template-remove"), false, true),
                    ],
                );
                self.hits.push((b[0], Hit::Foot(Button::Done)));
                self.hits.push((b[1], Hit::Foot(Button::RemoveTemplate)));
            }
        }
    }

    pub fn hit_at(&self, p: (f32, f32)) -> Option<Hit> {
        self.hits
            .iter()
            .rev()
            .find(|h| h.0.contains(p))
            .map(|h| h.1)
    }

    /// The label a row's AccessKit node gets.
    fn row_label(&self, row: &Row) -> Option<String> {
        Some(match row {
            Row::Template(i) => self.templates.get(*i)?.name.clone(),
            Row::Name => t!("metaedit-template-name"),
            Row::Field(f) => field_label(*f),
            Row::KeywordMode => t!("metaedit-template-replace-keywords"),
            _ => return None,
        })
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
        group.set_label(t!("metaedit-title"));
        group.set_bounds(bounds(panel));
        let mut ids = Vec::new();
        let mut nodes = Vec::new();
        let mut focus = None;
        for (i, (r, hit)) in self.hits.iter().enumerate() {
            let id = NodeId(A11Y_META + 1 + i as u64);
            let mut n = match hit {
                Hit::Panel => continue,
                Hit::Close => {
                    let mut n = Node::new(Role::Button);
                    n.set_label(t!("metaedit-close"));
                    n
                }
                Hit::Row(k) => {
                    let Some(row) = self.rows.get(*k) else {
                        continue;
                    };
                    let Some(label) = self.row_label(row) else {
                        continue;
                    };
                    let mut n = match row {
                        Row::Field(f) => {
                            let mut n = Node::new(Role::TextInput);
                            let s = self.slot(*f);
                            n.set_value(s.edit.text().to_owned());
                            if s.mixed && s.edit.is_empty() {
                                n.set_description(t!("metaedit-mixed"));
                            }
                            n
                        }
                        Row::Name => {
                            let mut n = Node::new(Role::TextInput);
                            n.set_value(self.name.text().to_owned());
                            n
                        }
                        Row::KeywordMode => {
                            let mut n = Node::new(Role::CheckBox);
                            let on = matches!(self.mode, Mode::Template(i)
                                if self.templates.get(i).is_some_and(|t| t.keyword_mode == KeywordMode::Replace));
                            n.set_toggled(if on { Toggled::True } else { Toggled::False });
                            n
                        }
                        _ => Node::new(Role::Button),
                    };
                    n.set_label(label);
                    if self.focus == Some(*k) {
                        focus = Some(id);
                    }
                    n
                }
                Hit::Foot(b) => {
                    let mut n = Node::new(Role::Button);
                    n.set_label(match b {
                        Button::NewTemplate => t!("metaedit-template-new"),
                        Button::EditTemplate => t!("metaedit-template-edit"),
                        Button::RemoveTemplate => t!("metaedit-template-remove"),
                        Button::Done => t!("metaedit-template-done"),
                        Button::ConfirmRemove => t!("metaedit-template-remove-yes"),
                        Button::Cancel => t!("app-cancel"),
                    });
                    n
                }
            };
            n.set_bounds(bounds(*r));
            n.add_action(accesskit::Action::Click);
            if matches!(hit, Hit::Row(k) if self.rows.get(*k).is_some_and(Row::editable)) {
                n.add_action(accesskit::Action::Focus);
            }
            ids.push(id);
            nodes.push((id, n));
        }
        group.set_children(ids);
        if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
            root.push_child(NodeId(A11Y_META));
        }
        tree.nodes.push((NodeId(A11Y_META), group));
        tree.nodes.extend(nodes);
        if let Some(f) = focus.filter(|_| self.has_focus()) {
            tree.focus = f;
        }
    }

    /// The hit of AccessKit node `id`.
    pub fn a11y_hit(&self, id: u64) -> Option<Hit> {
        let i = id.checked_sub(A11Y_META + 1)?;
        self.hits.get(usize::try_from(i).ok()?).map(|h| h.1)
    }
}

// ----- The application's side ----------------------------------------------------------------

/// Where the panel acts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Where {
    Gallery,
    Cull,
    Viewer,
}

/// What import writes to each photo it copied: template `t`'s fields (its keywords added or
/// replacing, as the template says) and the keywords typed (`a; b > c`), added. `None` when
/// that is nothing.
pub(in crate::app) fn import_edit(t: Option<&Template>, typed: &str) -> Option<Edit> {
    let mut e = t.map(Template::edit).unwrap_or_default();
    let typed = edit::parse_text(Field::Keywords, typed);
    if !typed.is_empty() {
        let adds = typed.into_iter().map(kw::Change::Add);
        e.keywords = Some(match e.keywords.take() {
            Some(KeywordEdit::Replace(mut v)) => {
                for k in adds {
                    if let kw::Change::Add(k) = k
                        && !v.iter().any(|x| kw::same(x, &k))
                    {
                        v.push(k);
                    }
                }
                KeywordEdit::Replace(v)
            }
            Some(KeywordEdit::Changes(mut c)) => {
                c.extend(adds);
                KeywordEdit::Changes(c)
            }
            None => KeywordEdit::Changes(adds.collect()),
        });
    }
    (!e.is_empty()).then_some(e)
}

/// Writes import's `e` to the XMP sidecars of `images` (never into the files: they are the
/// card's bytes, verified); the ones that failed, and why.
pub(in crate::app) fn write_import(
    images: &[PathBuf],
    e: &Edit,
    vocab: &kw::Vocabulary,
) -> Vec<(PathBuf, String)> {
    files::write_all(images, e, vocab, false)
        .into_iter()
        .filter_map(|(p, r)| r.err().map(|e| (p, e)))
        .collect()
}

impl App {
    /// The config folder the templates are kept in (None in screenshots and headless).
    pub(in crate::app) fn meta_dir(&self) -> Option<PathBuf> {
        if self.args.screenshot.is_some() || self.headless() {
            return None;
        }
        self.store
            .path()
            .and_then(|p| p.parent().map(Path::to_path_buf))
    }

    /// Where the panel acts now (None: nowhere it can).
    pub(super) fn meta_where(&self) -> Option<Where> {
        if self.cull_keyword_targets().is_some() {
            Some(Where::Cull)
        } else if self.gallery.active {
            Some(Where::Gallery)
        } else if self.current.is_some() && !self.entries.is_empty() {
            Some(Where::Viewer)
        } else {
            None
        }
    }

    fn meta_targets(&self, w: Where) -> Vec<PathBuf> {
        match w {
            Where::Cull => self.cull_keyword_targets().unwrap_or_default(),
            Where::Gallery => self.gallery.keyword_targets(),
            Where::Viewer => self
                .current
                .and_then(|i| self.entries.get(i))
                .filter(|p| p.is_file())
                .cloned()
                .into_iter()
                .collect(),
        }
    }

    /// `Ctrl+Alt+I`: opens the panel with the keyboard in its first field; again closes it.
    pub(super) fn meta_toggle(&mut self) {
        let m = &self.gallery.meta_edit;
        if m.open && m.has_focus() {
            self.meta_close();
        } else {
            self.meta_open();
        }
        self.redraw = true;
    }

    /// Opens the panel (the Info panel's button, the gallery's menu).
    pub(super) fn meta_open(&mut self) {
        let Some(w) = self.meta_where() else { return };
        let dir = self.meta_dir();
        // Keywords typed here join the keyword list.
        self.gallery.keywords.load(dir.clone());
        if self.gallery.keywords.open {
            self.gallery.keywords.close();
        }
        if w == Where::Viewer && self.chrome.info_open() {
            self.chrome.toggle_info();
        }
        let into = self.settings.gallery.metadata_into_files;
        let m = &mut self.gallery.meta_edit;
        m.load(dir);
        m.into_files = into;
        m.open = true;
        m.targets_at = f64::NEG_INFINITY;
        m.status = None;
        m.rows = m.build_rows();
        let first = m.rows.iter().position(|r| matches!(r, Row::Field(_)));
        m.focus_row(first);
        if w == Where::Gallery {
            self.gallery_keywords_opened();
        }
        self.redraw = true;
    }

    /// Closes the panel, writing the field being typed into.
    pub(super) fn meta_close(&mut self) {
        if let Some(out) = self.gallery.meta_edit.commit() {
            self.meta_out(out);
        }
        self.gallery.meta_edit.close();
        self.gallery_keywords_closed();
        self.redraw = true;
    }

    /// Once per event-loop turn: results from the workers, the selection.
    pub(super) fn meta_tick(&mut self) {
        let now = self.now();
        let msgs: Vec<Msg> = self.gallery.meta_edit.rx.try_iter().collect();
        for m in msgs {
            self.meta_msg(m, now);
            self.redraw = true;
        }
        if !self.gallery.meta_edit.open {
            return;
        }
        let Some(w) = self.meta_where() else {
            self.meta_close();
            return;
        };
        if now - self.gallery.meta_edit.targets_at >= TARGETS_S {
            let t = self.meta_targets(w);
            self.gallery.meta_edit.targets_at = now;
            if t != self.gallery.meta_edit.targets {
                // What was typed goes to the photos it was typed for.
                if let Some(out) = self.gallery.meta_edit.commit() {
                    self.meta_out(out);
                }
                let m = &mut self.gallery.meta_edit;
                m.targets = t;
                if m.writing == 0 {
                    m.status = None;
                }
                m.refresh();
                self.redraw = true;
            }
        }
        self.meta_read();
    }

    fn meta_read(&mut self) {
        let m = &mut self.gallery.meta_edit;
        let todo = m.unread_part();
        if todo.is_empty() {
            return;
        }
        let (tx, wake) = (m.tx.clone(), self.loader.waker());
        self.loader.background("meta-read", move |_| {
            let _ = tx.send(Msg::Read(files::read_all(&todo)));
            wake();
        });
    }

    fn meta_msg(&mut self, msg: Msg, now: f64) {
        let m = &mut self.gallery.meta_edit;
        match msg {
            Msg::Read(r) => m.learn(r),
            Msg::Wrote(r, fields) => {
                m.writing = m.writing.saturating_sub(1);
                let mut ok = Vec::new();
                let mut failed = Vec::new();
                for (p, res) in r {
                    match res {
                        Ok(rec) => {
                            m.files.insert(p.clone(), rec.clone());
                            ok.push((p, rec));
                        }
                        Err(e) => {
                            specular_log::warn!(target: specular_log::UI, "metadata of {}: {e}", p.display());
                            // Read again: what is on disk is what counts.
                            m.files.remove(&p);
                            failed.push(e);
                        }
                    }
                }
                m.refresh();
                if let Some(e) = failed.first() {
                    let msg = t!(
                        "metaedit-write-failed",
                        count = failed.len() as i64,
                        error = e.as_str()
                    );
                    m.status = Some(msg.clone());
                    self.chrome.toast(msg, None, now);
                }
                // The keyword panel reads them again.
                if fields.contains(&Field::Keywords) {
                    for (p, _) in &ok {
                        self.gallery.keywords.files.remove(p);
                    }
                }
                if let Some(lib) = self.gallery.library_handle() {
                    if fields.contains(&Field::Keywords) {
                        lib.set_keywords(
                            ok.iter()
                                .map(|(p, r)| (p.clone(), r.values(Field::Keywords).to_vec()))
                                .collect(),
                        );
                    }
                    lib.set_iptc(ok.iter().map(|(p, r)| (p.clone(), r.to_lines())).collect());
                }
                self.gallery_keywords_written();
            }
        }
    }

    /// Writes `e` to the photos the panel acts on.
    fn meta_write(&mut self, e: Edit, said: String) {
        let m = &mut self.gallery.meta_edit;
        if m.targets.is_empty() || e.is_empty() {
            return;
        }
        m.status = (!said.is_empty()).then(|| said.clone());
        if !said.is_empty() {
            self.chrome.announce_status(said);
        }
        // New keywords join the list and the recently used ones.
        if let Some(k) = &e.keywords {
            let added: Vec<String> = match k {
                KeywordEdit::Replace(v) => v.clone(),
                KeywordEdit::Changes(c) => c
                    .iter()
                    .filter_map(|c| match c {
                        kw::Change::Add(p) => Some(p.clone()),
                        kw::Change::Remove(_) => None,
                    })
                    .collect(),
            };
            let now = self.now();
            self.gallery.keywords.learned(&added, now);
        }
        let m = &mut self.gallery.meta_edit;
        m.writing += 1;
        let targets = m.targets.clone();
        let into = m.into_files;
        let fields = e.fields();
        let (tx, vocab, wake) = (
            m.tx.clone(),
            self.gallery.keywords.vocab.clone(),
            self.loader.waker(),
        );
        if self.args.screenshot.is_some() {
            return;
        }
        self.loader.background("meta-write", move |_| {
            let r = files::write_all(&targets, &e, &vocab, into);
            let _ = tx.send(Msg::Wrote(r, fields));
            wake();
        });
    }

    fn meta_save_templates(&mut self) {
        let m = &self.gallery.meta_edit;
        let Some(dir) = m.dir.clone() else { return };
        let list = m.templates.clone();
        self.loader.background("meta-templates", move |_| {
            if let Err(e) = files::save(&dir, &list) {
                specular_log::warn!(target: specular_log::UI, "metadata templates not saved: {e}");
            }
        });
    }

    /// A key while the panel shows. True: the panel used it.
    pub(super) fn meta_key(&mut self, ev: &KeyEvent) -> bool {
        if !self.gallery.meta_edit.has_focus() {
            return false;
        }
        let Some(combo) = self.key_combo(ev) else {
            let Some(t) = ev.text.as_ref() else {
                return false;
            };
            let out = self.gallery.meta_edit.type_text(t.as_str());
            return self.meta_out(out);
        };
        // The shortcut again closes the panel.
        if self.keymap.lookup(&combo) == Some(specular_input::Action::EditMetadata) {
            self.meta_close();
            return true;
        }
        let mut clip = PendingClipboard::default();
        let text = ev.text.as_ref().map(|t| t.to_string());
        let out = self
            .gallery
            .meta_edit
            .key(combo.key, combo.mods, text.as_deref(), &mut clip);
        self.field_clipboard(clip, super::integration::PasteTo::MetaEdit);
        self.meta_out(out)
    }

    fn meta_out(&mut self, out: KeyOut) -> bool {
        self.redraw = true;
        match out {
            KeyOut::No => false,
            KeyOut::Used => true,
            KeyOut::Write(e) => {
                let field = e.fields().first().copied();
                let said = field.map_or(String::new(), |f| {
                    let name = field_label(f);
                    let cleared = e.set.iter().any(|(_, v)| v.is_empty())
                        || matches!(&e.keywords, Some(KeywordEdit::Replace(v)) if v.is_empty());
                    if cleared {
                        t!("metaedit-cleared", field = name)
                    } else {
                        t!("metaedit-saved", field = name)
                    }
                });
                self.meta_write(e, said);
                true
            }
            KeyOut::Apply(i) => {
                let m = &mut self.gallery.meta_edit;
                let Some(t) = m.templates.get(i).cloned() else {
                    return true;
                };
                m.current = Some(i);
                let e = t.edit();
                m.assume(&e);
                m.refresh();
                let said = t!(
                    "metaedit-template-applied",
                    count = m.targets.len() as i64,
                    name = t.name.as_str()
                );
                self.meta_write(e, said);
                true
            }
            KeyOut::Templates => {
                self.meta_save_templates();
                true
            }
            KeyOut::Close => {
                self.meta_close();
                true
            }
            KeyOut::Button(b) => {
                if let Some(o) = self.gallery.meta_edit.commit() {
                    self.meta_out(o);
                }
                let out = self.gallery.meta_edit.button(b);
                self.meta_out(out)
            }
        }
    }

    /// A press over the panel (logical px). True: it was the panel's.
    pub(super) fn meta_press(&mut self, p: (f32, f32)) -> bool {
        let m = &mut self.gallery.meta_edit;
        if !m.open || !m.rect.is_some_and(|r| r.contains(p)) {
            // A click elsewhere ends the typing and gives the keyboard back.
            if m.open && m.has_focus() {
                let out = m.commit();
                m.focus = None;
                m.editing = false;
                if let Some(o) = out {
                    self.meta_out(o);
                }
            }
            return false;
        }
        let Some(h) = m.hit_at(p) else { return true };
        let out = m.click(h);
        self.meta_out(out)
    }

    /// The wheel over the panel. True: it scrolled.
    pub(super) fn meta_wheel(&mut self, p: (f32, f32), dy: f32) -> bool {
        let m = &mut self.gallery.meta_edit;
        if !m.open || !m.rect.is_some_and(|r| r.contains(p)) {
            return false;
        }
        m.scroll = (m.scroll - dy).max(0.0);
        self.redraw = true;
        true
    }

    /// Text pasted into the field.
    pub(super) fn meta_paste(&mut self, s: &str) {
        let m = &mut self.gallery.meta_edit;
        if m.open && m.editing {
            m.paste(s);
            self.redraw = true;
        }
    }

    /// An assistive technology's action on node `id`. True: it was the panel's.
    pub(super) fn meta_a11y_action(&mut self, id: u64) -> bool {
        let Some(h) = self.gallery.meta_edit.a11y_hit(id) else {
            return false;
        };
        let out = self.gallery.meta_edit.click(h);
        self.meta_out(out);
        true
    }
}

/// Where the panel goes over the viewer's image area `area`: the trailing edge, from the top
/// to `bottom`.
pub(super) fn viewer_rect(
    area: (f32, f32, f32, f32),
    bottom: f32,
    text_scale: f32,
    rtl: bool,
) -> Rect {
    let (ax, ay, aw, _) = area;
    let w = side_panel::width(aw, text_scale);
    let x = if rtl { ax } else { ax + aw - w };
    Rect::new(x, ay, w, (bottom - ay).max(0.0))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod shots;
