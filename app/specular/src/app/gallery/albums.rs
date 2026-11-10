//! Albums and the date timeline in the Library.
//!
//! - **Manual albums** hold photos put in by hand: `B` puts the selection in the target
//!   album (the last one used), `Shift+B` asks which; the right-click menu does both, and
//!   takes photos out of the album shown.
//! - **Smart albums** are saved rules: queries joined with and, or and except, top to
//!   bottom (`specular_query::rules`). "Save search…" keeps the filter bar's query as one.
//! - Names with `/` make groups: `Client X / selects` is `selects` in `Client X`.
//! - An album opened in the sidebar scopes the search; the filter bar narrows it further.
//! - **The timeline** above the grid counts the photos per year, month or day (the finest
//!   that fits); a click picks a period, Shift+click a run of them, a click on the picked
//!   period clears it. It writes the `date:` term, so the filter bar shows what it did.
//!
//! The catalogue is never touched on the UI thread: album changes run on a `lib-albums`
//! worker and come back as [`Msg::Albums`].

use std::collections::HashSet;

use specular_gfx::{DrawList, Text, TextStyle};
use specular_input::NamedKey;
use specular_library::albums::{self as al, Album, Kind as AlbumKind, NameProblem};
use specular_query::rules::{Combine, Rule, Rules};
use specular_query::timeline::{self as tl, Bar, Step};
use specular_widgets::line_edit::LineEdit;

use super::draw::{Frame, text_at};
use super::library::Side;
use super::*;
use crate::app::fileops::{ButtonView, ChoiceView, FieldView, Part, RowView, SheetView};

/// Height of the timeline strip (before the text scale).
const TIMELINE_H: f32 = 44.0;
/// The narrowest a bar is drawn.
const BAR_MIN_W: f32 = 6.0;
/// Rows of a sheet's list in view (the sheet's `MAX_ROWS`; it scrolls the rest).
const ROWS: usize = 9;

/// The Library's albums and timeline.
#[derive(Default)]
pub(in crate::app) struct AlbumsView {
    pub(super) list: Vec<Album>,
    /// The album shown (None: the locations).
    pub(super) open: Option<i64>,
    /// Where `B` puts photos.
    pub(super) target: Option<i64>,
    /// Groups folded in the sidebar (full names, lower case).
    pub(super) folded: HashSet<String>,
    /// The albums section of the sidebar is folded.
    pub(super) hidden: bool,
    /// The list has been read once.
    pub(super) loaded: bool,
    /// A change or a read runs on the worker.
    pub(super) busy: bool,
    /// Asked: delete these albums / take these photos out of an album.
    pub(super) delete_ask: Option<Vec<i64>>,
    pub(super) remove_ask: Option<(i64, Vec<PathBuf>)>,
    /// The last search's matches per day (`specular_library::Found::timeline`).
    pub(super) days: Vec<(i64, u64)>,
    /// The bars as drawn, with their rects (for hits and screen readers).
    pub(super) bars: Vec<(Bar, Rect)>,
    /// The bar a Shift+click extends from (its first day).
    pub(super) anchor: Option<i64>,
    /// The timeline strip's rect when drawn.
    pub(super) strip: Option<Rect>,
    /// The bar with the keyboard focus (the timeline is a Tab stop).
    pub(super) kb: Option<usize>,
    /// Settings › Library › Date timeline is off.
    pub(super) hide_timeline: bool,
}

/// A row of the albums tree in the sidebar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Node {
    /// A group (a leading part of album names): its full name, depth and albums inside.
    Group {
        name: String,
        depth: usize,
        albums: usize,
    },
    Album {
        id: i64,
        depth: usize,
    },
}

/// The sidebar's tree of `list` (by name), without what folded groups hold.
pub(super) fn tree(list: &[Album], folded: &HashSet<String>) -> Vec<Node> {
    let mut out = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for a in list {
        let parts: Vec<&str> = a.name.split('/').collect();
        let mut hidden = false;
        for d in 0..parts.len() - 1 {
            let g = parts[..=d].join("/");
            let key = g.to_lowercase();
            if !hidden && seen.insert(key.clone()) {
                let albums = al::in_group(list, &g).count();
                out.push(Node::Group {
                    name: g,
                    depth: d,
                    albums,
                });
            }
            hidden |= folded.contains(&key);
        }
        if !hidden {
            out.push(Node::Album {
                id: a.id,
                depth: parts.len() - 1,
            });
        }
    }
    out
}

/// The sidebar items of the albums section: its heading, then (unless folded) the tree,
/// "New album…" and "New smart album…" (not in a read-only catalogue).
pub(super) fn items(v: &AlbumsView, writable: bool) -> Vec<Side> {
    let mut out = vec![Side::AlbumsTitle];
    if v.hidden {
        return out;
    }
    for (k, n) in tree(&v.list, &v.folded).iter().enumerate() {
        out.push(match n {
            Node::Group { .. } => Side::Group(k),
            Node::Album { id, .. } => Side::Album(*id),
        });
    }
    if writable {
        out.push(Side::NewAlbum);
        out.push(Side::NewSmart);
    }
    out
}

/// A name problem in words.
pub(super) fn name_problem_text(p: &NameProblem) -> String {
    match p {
        NameProblem::Empty => t!("albums-name-empty"),
        NameProblem::Taken(n) => t!("albums-name-taken", album = shown_name(n)),
        NameProblem::Clash(n) => t!("albums-name-clash", album = shown_name(n)),
    }
}

/// A full name as shown: parts joined with " / ".
pub(super) fn shown_name(name: &str) -> String {
    name.split('/').collect::<Vec<_>>().join(" / ")
}

/// What a rule's word is called in the sheet (the first rule has none).
fn combine_text(c: Combine, first: bool) -> String {
    if first {
        return t!("albums-rule-first");
    }
    match c {
        Combine::And => t!("albums-rule-and"),
        Combine::Or => t!("albums-rule-or"),
        Combine::Except => t!("albums-rule-except"),
    }
}

// ----- The sheet ----------------------------------------------------------------------

/// What an album sheet is for (what OK does).
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) enum Purpose {
    /// A new manual album; these photos go in once it is made.
    NewAlbum(Vec<PathBuf>),
    Rename(i64),
    /// A group's new name (its albums move with it).
    RenameGroup(String),
    /// A new smart album (None) or an album's name and rules.
    Smart(Option<i64>),
    /// Which album these photos go in (rows), or a new one (the field).
    AddTo(Vec<PathBuf>),
}

/// An album sheet, shown in the file-operations sheet ([`crate::app::fileops`] draws,
/// focuses and reads it).
#[derive(Clone, Debug)]
pub(in crate::app) struct AlbumSheet {
    pub purpose: Purpose,
    /// The albums there are (names are checked against them; `AddTo` lists the manual ones).
    pub albums: Vec<Album>,
    pub field: LineEdit,
    /// `Smart`: the name, and the rules (the field edits the row chosen).
    pub name: String,
    pub rules: Vec<Rule>,
    /// The row chosen: `Smart` 0 the name, 1.. the rules; `AddTo` the album's index in
    /// [`Self::targets`].
    pub sel: usize,
    /// The first row in view.
    pub top: usize,
}

impl AlbumSheet {
    fn with(purpose: Purpose, albums: Vec<Album>, text: &str) -> AlbumSheet {
        let mut field = LineEdit::new(text);
        field.select(0..text.len());
        AlbumSheet {
            purpose,
            albums,
            field,
            name: String::new(),
            rules: Vec::new(),
            sel: 0,
            top: 0,
        }
    }

    /// Asks for a new manual album's name (in `group` when given).
    pub fn new_album(albums: Vec<Album>, group: Option<&str>, add: Vec<PathBuf>) -> AlbumSheet {
        let text = group
            .map(|g| format!("{} / ", shown_name(g)))
            .unwrap_or_default();
        let mut s = AlbumSheet::with(Purpose::NewAlbum(add), albums, &text);
        s.field.select(text.len()..text.len());
        s
    }

    pub fn rename(albums: Vec<Album>, id: i64) -> Option<AlbumSheet> {
        let name = shown_name(&albums.iter().find(|a| a.id == id)?.name);
        Some(AlbumSheet::with(Purpose::Rename(id), albums, &name))
    }

    pub fn rename_group(albums: Vec<Album>, group: &str) -> AlbumSheet {
        let name = shown_name(group);
        AlbumSheet::with(Purpose::RenameGroup(group.to_owned()), albums, &name)
    }

    /// A smart album's sheet: a new one from `query` (the filter bar's, maybe empty), or
    /// album `id`'s name and rules.
    pub fn smart(
        albums: Vec<Album>,
        id: Option<i64>,
        group: Option<&str>,
        query: &str,
    ) -> Option<AlbumSheet> {
        let (name, rules) = match id {
            Some(id) => {
                let a = albums.iter().find(|a| a.id == id)?;
                (shown_name(&a.name), a.rules.0.clone())
            }
            None => (
                group
                    .map(|g| format!("{} / ", shown_name(g)))
                    .unwrap_or_default(),
                vec![Rule::new(Combine::And, query.trim())],
            ),
        };
        let mut s = AlbumSheet::with(Purpose::Smart(id), albums, &name);
        s.field.select(name.len()..name.len());
        s.name = name;
        s.rules = if rules.is_empty() {
            vec![Rule::default()]
        } else {
            rules
        };
        Some(s)
    }

    /// Which manual album `paths` go in; `target` chosen first.
    pub fn add_to(albums: Vec<Album>, target: Option<i64>, paths: Vec<PathBuf>) -> AlbumSheet {
        let mut s = AlbumSheet::with(Purpose::AddTo(paths), albums, "");
        s.sel = s
            .targets()
            .iter()
            .position(|a| Some(a.id) == target)
            .unwrap_or(0);
        s.top = s.sel.saturating_sub(ROWS - 1);
        s
    }

    /// `AddTo`: the manual albums, by name.
    pub fn targets(&self) -> Vec<&Album> {
        self.albums
            .iter()
            .filter(|a| a.kind == AlbumKind::Manual)
            .collect()
    }

    /// Where the field starts: on it (names), else the first row.
    pub fn focus(&self) -> Part {
        Part::Field
    }

    /// The typed text, trimmed.
    pub fn typed(&self) -> String {
        self.field.text().trim().to_owned()
    }

    /// The album being renamed or edited (its name may stay).
    fn except(&self) -> Option<i64> {
        match &self.purpose {
            Purpose::Rename(id) => Some(*id),
            Purpose::Smart(id) => *id,
            _ => None,
        }
    }

    /// The name typed (Smart: the name row's, the field's while it is chosen).
    pub fn name_text(&self) -> String {
        match self.purpose {
            Purpose::Smart(_) if self.sel != 0 => self.name.clone(),
            _ => self.typed(),
        }
    }

    /// The rules with the field's text in the rule chosen.
    pub fn rules_now(&self) -> Rules {
        let mut r = self.rules.clone();
        if self.sel > 0
            && let Some(x) = r.get_mut(self.sel - 1)
        {
            x.query = self.field.text().to_owned();
        }
        Rules(r)
    }

    /// Why the name can't be used (None: it can, or none is asked).
    pub fn name_problem(&self) -> Option<NameProblem> {
        match &self.purpose {
            Purpose::NewAlbum(_) | Purpose::Rename(_) | Purpose::Smart(_) => {
                al::name_problem(&self.albums, &self.name_text(), self.except())
            }
            Purpose::RenameGroup(g) => {
                let Some(to) = al::normalize(&self.typed()) else {
                    return Some(NameProblem::Empty);
                };
                // Each album's new name, checked against the albums outside the group.
                let inside: Vec<i64> = al::in_group(&self.albums, g).map(|a| a.id).collect();
                let rest: Vec<Album> = self
                    .albums
                    .iter()
                    .filter(|a| !inside.contains(&a.id))
                    .cloned()
                    .collect();
                al::in_group(&self.albums, g).find_map(|a| {
                    al::name_problem(&rest, &format!("{to}{}", &a.name[g.len()..]), None)
                })
            }
            Purpose::AddTo(_) => {
                let t = self.typed();
                if t.is_empty() {
                    None
                } else {
                    al::name_problem(&self.albums, &t, None)
                }
            }
        }
    }

    /// Whether OK can be pressed.
    pub fn ready(&self) -> bool {
        match &self.purpose {
            Purpose::AddTo(_) => {
                if self.typed().is_empty() {
                    !self.targets().is_empty()
                } else {
                    self.name_problem().is_none()
                }
            }
            Purpose::Smart(_) => self.name_problem().is_none() && !self.rules_now().is_empty(),
            _ => self.name_problem().is_none(),
        }
    }

    /// `Smart`: puts the field's text back in the row chosen and loads row `k`'s.
    fn choose(&mut self, k: usize) {
        let text = self.field.text().to_owned();
        if self.sel == 0 {
            self.name = text;
        } else if let Some(r) = self.rules.get_mut(self.sel - 1) {
            r.query = text;
        }
        self.sel = k;
        let t = if k == 0 {
            self.name.clone()
        } else {
            self.rules
                .get(k - 1)
                .map(|r| r.query.clone())
                .unwrap_or_default()
        };
        self.field.set_text(&t);
        self.field.select(t.len()..t.len());
    }

    /// The list's rows: `Smart` the name, the rules, "Add a rule"; `AddTo` the albums.
    fn row_count(&self) -> usize {
        match self.purpose {
            Purpose::Smart(_) => self.rules.len() + 2,
            Purpose::AddTo(_) => self.targets().len(),
            _ => 0,
        }
    }

    /// Row `i` in view pressed. True when the keyboard should go to the field.
    pub fn press(&mut self, i: usize) -> bool {
        let k = self.top + i;
        match self.purpose {
            Purpose::Smart(_) => {
                if k == self.rules.len() + 1 {
                    let combine = if self.rules.is_empty() {
                        Combine::And
                    } else {
                        Combine::Or
                    };
                    self.rules.push(Rule::new(combine, ""));
                }
                self.choose(k.min(self.rules.len()));
                let n = self.row_count();
                if self.sel >= self.top + ROWS {
                    self.top = (self.sel + 1)
                        .saturating_sub(ROWS)
                        .min(n.saturating_sub(ROWS));
                }
                true
            }
            Purpose::AddTo(_) => {
                if k < self.targets().len() {
                    self.sel = k;
                    // A row chosen: no new album.
                    self.field.set_text("");
                }
                false
            }
            _ => false,
        }
    }

    /// The choice row pressed: the chosen rule's word.
    pub fn set_choice(&mut self, i: usize) {
        if let Purpose::Smart(_) = self.purpose
            && self.sel >= 2
            && let Some(r) = self.rules.get_mut(self.sel - 1)
        {
            r.combine = Combine::ALL[i.min(2)];
        }
    }

    /// The rows: (top, total).
    pub fn row_window(&mut self) -> (&mut usize, usize) {
        let n = self.row_count();
        (&mut self.top, n)
    }

    pub fn view(&self, focus: Part) -> SheetView {
        let (title, ok, label) = match &self.purpose {
            Purpose::NewAlbum(_) => (
                t!("albums-new-title"),
                t!("albums-create"),
                t!("albums-name"),
            ),
            Purpose::Rename(_) => (
                t!("albums-rename-title"),
                t!("albums-rename"),
                t!("albums-name"),
            ),
            Purpose::RenameGroup(_) => (
                t!("albums-rename-group-title"),
                t!("albums-rename"),
                t!("albums-name"),
            ),
            Purpose::Smart(None) => (
                t!("albums-new-smart-title"),
                t!("albums-create"),
                if self.sel == 0 {
                    t!("albums-name")
                } else {
                    t!("albums-rule-label", number = self.sel as i64)
                },
            ),
            Purpose::Smart(Some(_)) => (
                t!("albums-edit-smart-title"),
                t!("albums-save"),
                if self.sel == 0 {
                    t!("albums-name")
                } else {
                    t!("albums-rule-label", number = self.sel as i64)
                },
            ),
            Purpose::AddTo(p) => (
                t!("albums-add-title", count = p.len() as i64),
                t!("albums-add"),
                t!("albums-add-new"),
            ),
        };
        let sel = self.field.selection();
        let field = FieldView {
            label,
            text: self.field.text().to_owned(),
            caret: self.field.caret(),
            selection: (sel.start, sel.end),
            preedit: String::new(),
        };
        let mut notes = Vec::new();
        let mut error = self.name_problem().map(|p| name_problem_text(&p));
        let mut rows = Vec::new();
        let mut choice = None;
        let mut list_label = None;
        match &self.purpose {
            Purpose::NewAlbum(_) | Purpose::RenameGroup(_) => notes.push(t!("albums-groups-note")),
            Purpose::Smart(_) => {
                notes.push(t!("albums-rules-note"));
                list_label = Some(t!("albums-rules"));
                let rules = self.rules_now();
                let name = self.name_text();
                let mut all = vec![RowView {
                    start: t!("albums-name"),
                    end: Some(if name.trim().is_empty() {
                        t!("albums-unnamed")
                    } else {
                        name
                    }),
                    problem: None,
                    quiet: false,
                    choosable: true,
                    chosen: self.sel == 0,
                }];
                for (k, r) in rules.0.iter().enumerate() {
                    let blank = r.query.trim().is_empty();
                    all.push(RowView {
                        start: if blank {
                            t!("albums-rule-empty")
                        } else {
                            r.query.trim().to_owned()
                        },
                        end: Some(combine_text(r.combine, k == 0)),
                        problem: None,
                        quiet: blank,
                        choosable: true,
                        chosen: self.sel == k + 1,
                    });
                }
                all.push(RowView {
                    start: t!("albums-rule-add"),
                    end: None,
                    problem: None,
                    quiet: false,
                    choosable: true,
                    chosen: false,
                });
                rows = all.into_iter().skip(self.top).take(ROWS).collect();
                if self.sel >= 1 {
                    // The chosen rule's first mistake.
                    let q =
                        specular_query::parse_in(self.field.text(), specular_query::Scope::Library);
                    if error.is_none()
                        && let Some(e) = q.errors.first()
                    {
                        error = Some(super::facets::query_error(e));
                    }
                }
                if self.sel >= 2 {
                    let c = rules
                        .0
                        .get(self.sel - 1)
                        .map_or(Combine::And, |r| r.combine);
                    choice = Some(ChoiceView {
                        label: t!("albums-rule-combine"),
                        options: Combine::ALL
                            .iter()
                            .map(|c| combine_text(*c, false))
                            .collect(),
                        selected: Combine::ALL.iter().position(|x| *x == c).unwrap_or(0),
                    });
                }
                if rules.is_empty() && error.is_none() {
                    error = Some(t!("albums-rules-none"));
                }
            }
            Purpose::AddTo(_) => {
                let targets = self.targets();
                if targets.is_empty() {
                    notes.push(t!("albums-add-none"));
                } else {
                    list_label = Some(t!("albums-add-list"));
                    let typing = !self.typed().is_empty();
                    rows = targets
                        .iter()
                        .enumerate()
                        .skip(self.top)
                        .take(ROWS)
                        .map(|(k, a)| RowView {
                            start: shown_name(&a.name),
                            end: Some(t!("gallery-items", count = a.files as i64)),
                            problem: None,
                            quiet: typing,
                            choosable: true,
                            chosen: !typing && k == self.sel,
                        })
                        .collect();
                    notes.push(t!("albums-add-note"));
                }
            }
            Purpose::Rename(_) => {}
        }
        let below = self.row_count().saturating_sub(self.top + rows.len());
        SheetView {
            title,
            subtitle: None,
            field: Some(field),
            error,
            notes,
            list_label,
            rows,
            multi: false,
            more: (below > 0).then(|| t!("develop-sheet-more", count = below as i64)),
            choice,
            buttons: vec![
                ButtonView {
                    label: ok,
                    enabled: self.ready(),
                    default: true,
                },
                ButtonView {
                    label: t!("fileop-cancel"),
                    enabled: true,
                    default: false,
                },
            ],
            focus,
        }
    }
}

// ----- Changes on the worker ------------------------------------------------------------

/// An album change for the worker.
#[derive(Clone, Debug)]
pub(in crate::app) enum Op {
    /// Read the list only.
    Load,
    Create {
        name: String,
        kind: AlbumKind,
        rules: Rules,
        /// Photos to put in it once made.
        add: Vec<PathBuf>,
        /// Show it once made.
        open: bool,
    },
    Rename(i64, String),
    RenameGroup(String, String),
    Edit {
        id: i64,
        name: String,
        rules: Rules,
    },
    Delete(Vec<i64>),
    Add(i64, Vec<PathBuf>),
    Remove(i64, Vec<PathBuf>),
}

/// What came of an [`Op`].
pub(in crate::app) struct Done {
    op: Op,
    /// The album list after it (None: could not be read).
    list: Option<Vec<Album>>,
    /// The album made, photos put in (and those not in the Library), photos taken out.
    made: Option<i64>,
    added: usize,
    missing: usize,
    removed: usize,
    error: Option<String>,
}

fn run(lib: &specular_library::Library, op: &Op) -> Done {
    let mut d = Done {
        op: op.clone(),
        list: None,
        made: None,
        added: 0,
        missing: 0,
        removed: 0,
        error: None,
    };
    let r: Result<(), specular_library::Error> = (|| {
        match op {
            Op::Load => {}
            Op::Create {
                name,
                kind,
                rules,
                add,
                ..
            } => {
                let id = lib.album_create(name, *kind, rules)?;
                d.made = Some(id);
                if !add.is_empty() {
                    let (n, missing) = lib.album_add(id, add)?;
                    d.added = n;
                    d.missing = missing.len();
                }
            }
            Op::Rename(id, name) => lib.album_rename(*id, name)?,
            Op::RenameGroup(g, to) => {
                lib.album_rename_group(g, to)?;
            }
            Op::Edit { id, name, rules } => {
                lib.album_rename(*id, name)?;
                lib.album_set_rules(*id, rules)?;
            }
            Op::Delete(ids) => lib.album_delete(ids)?,
            Op::Add(id, paths) => {
                let (n, missing) = lib.album_add(*id, paths)?;
                d.added = n;
                d.missing = missing.len();
            }
            Op::Remove(id, paths) => d.removed = lib.album_remove(*id, paths)?,
        }
        Ok(())
    })();
    if let Err(e) = r {
        d.error = Some(crate::error_text::library_reason(&e));
    }
    match lib.albums() {
        Ok(l) => d.list = Some(l),
        Err(e) => {
            d.error
                .get_or_insert_with(|| crate::error_text::library_reason(&e));
        }
    }
    d
}

impl Gallery {
    /// The tree as the sidebar shows it.
    pub(super) fn albums_tree(&self) -> Vec<Node> {
        let v = &self.library.albums;
        tree(&v.list, &v.folded)
    }

    pub(super) fn album(&self, id: i64) -> Option<&Album> {
        self.library.albums.list.iter().find(|a| a.id == id)
    }

    /// Starts `op` on a worker (its result comes back as [`Msg::Albums`]).
    pub(super) fn albums_run(&mut self, op: Op, wake: crate::loader::Wake) {
        let Some(l) = self.library.lib.clone() else {
            return;
        };
        let tx = self.tx.clone();
        self.library.albums.busy = true;
        let spawned = specular_thread::builder("lib-albums").spawn(move || {
            let d =
                specular_core::isolate::run("albums", || run(&l, &op)).unwrap_or_else(|_| Done {
                    op: Op::Load,
                    list: None,
                    made: None,
                    added: 0,
                    missing: 0,
                    removed: 0,
                    error: Some(t!("error-reason-internal")),
                });
            let _ = tx.send(Msg::Albums(Box::new(d)));
            wake();
        });
        if spawned.is_err() {
            self.library.albums.busy = false;
        }
    }

    /// What the search is scoped to: the album shown, a manual one's id or a smart one's
    /// rules.
    pub(super) fn albums_scope(&self) -> (Option<i64>, Option<Rules>) {
        match self.library.albums.open.and_then(|id| self.album(id)) {
            Some(a) if a.kind == AlbumKind::Manual => (Some(a.id), None),
            Some(a) => (None, Some(a.rules.clone())),
            None => (None, None),
        }
    }

    /// The manual album shown, if any.
    pub(super) fn albums_open_manual(&self) -> Option<&Album> {
        self.library
            .albums
            .open
            .and_then(|id| self.album(id))
            .filter(|a| a.kind == AlbumKind::Manual)
    }

    /// How a sidebar item of the albums section looks: icon, name, the line under it, and
    /// its depth in the tree. None for items not of this section.
    pub(super) fn album_look(&self, s: Side) -> Option<(Icon, String, Option<String>, usize)> {
        let v = &self.library.albums;
        Some(match s {
            Side::AlbumsTitle => (
                if v.hidden {
                    Icon::ChevronRight
                } else {
                    Icon::ChevronDown
                },
                t!("albums-title"),
                None,
                0,
            ),
            Side::Group(k) => {
                let Some(Node::Group {
                    name,
                    depth,
                    albums,
                }) = self.albums_tree().get(k).cloned()
                else {
                    return None;
                };
                (
                    if v.folded.contains(&name.to_lowercase()) {
                        Icon::ChevronRight
                    } else {
                        Icon::ChevronDown
                    },
                    al::leaf(&name).to_owned(),
                    Some(t!("albums-group-count", count = albums as i64)),
                    depth,
                )
            }
            Side::Album(id) => {
                let a = self.album(id)?;
                let mut sub = match a.kind {
                    AlbumKind::Manual => vec![t!("gallery-items", count = a.files as i64)],
                    AlbumKind::Smart => vec![t!("albums-smart")],
                };
                if v.target == Some(id) {
                    sub.push(t!("albums-target"));
                }
                let icon = match a.kind {
                    AlbumKind::Manual => Icon::Image,
                    AlbumKind::Smart => Icon::Search,
                };
                (
                    icon,
                    a.leaf().to_owned(),
                    Some(specular_i18n::dot_join(&sub)),
                    a.name.matches('/').count(),
                )
            }
            Side::NewAlbum => (Icon::Plus, t!("albums-new"), None, 0),
            // With a search typed, the row keeps it.
            Side::NewSmart if !self.filter.text.trim().is_empty() => {
                (Icon::Plus, t!("albums-save-search"), None, 0)
            }
            Side::NewSmart => (Icon::Plus, t!("albums-new-smart"), None, 0),
            _ => return None,
        })
    }

    /// The buttons a row shows on hover or focus (end edge first): edit, delete.
    pub(super) fn album_buttons(&self, s: Side) -> Vec<(Icon, Side)> {
        if self.library.lib.as_ref().is_none_or(|l| l.read_only()) {
            return Vec::new();
        }
        match s {
            Side::Album(id) => vec![
                (Icon::Close, Side::AlbumDelete(id)),
                (Icon::More, Side::AlbumEdit(id)),
            ],
            Side::Group(k) => vec![
                (Icon::Close, Side::GroupDelete(k)),
                (Icon::More, Side::GroupEdit(k)),
            ],
            _ => Vec::new(),
        }
    }

    /// A group's full name by its tree index.
    pub(super) fn albums_group(&self, k: usize) -> Option<String> {
        match self.albums_tree().get(k) {
            Some(Node::Group { name, .. }) => Some(name.clone()),
            _ => None,
        }
    }

    /// The group a new album goes in: the one focused or holding the album shown.
    fn albums_current_group(&self) -> Option<String> {
        let lv = &self.library;
        if let Some(k) = lv.focus
            && let Some(Side::Group(g)) = lv.items().get(k)
        {
            return self.albums_group(*g);
        }
        let a = lv.albums.open.and_then(|id| self.album(id))?;
        let g = a.groups();
        (!g.is_empty()).then(|| g.join("/"))
    }

    // ----- The timeline --------------------------------------------------------------

    /// Height of the timeline strip: shown in the Library when the matches have dates.
    pub(super) fn timeline_h(&self) -> f32 {
        let av = &self.library.albums;
        if self.library.on && !av.hide_timeline && av.days.len() > 1 {
            self.u(TIMELINE_H)
        } else {
            0.0
        }
    }

    /// Draws the timeline strip under the filter bar.
    pub(super) fn draw_timeline(&mut self, dl: &mut DrawList, text: &mut Text, f: &Frame) {
        let h = self.timeline_h();
        let av = &mut self.library.albums;
        av.bars.clear();
        av.strip = None;
        if h <= 0.0 {
            return;
        }
        let pal = f.pal;
        let ts = f.text_scale.clamp(0.75, 3.0);
        let u = |v: f32| (v * ts).round();
        let top = TOP_BAR
            + if self.filter_open {
                self.filter_bar_h()
            } else {
                0.0
            };
        let w = self.grid_w();
        let strip = Rect::new(0.0, top, w, h);
        dl.rect(self.flip(strip), 0.0, pal.surface);
        dl.rect(
            self.flip(Rect::new(0.0, strip.bottom() - 1.0, w, 1.0)),
            0.0,
            pal.separator,
        );
        let pad = u(12.0);
        let small = TextStyle::new(11.0 * ts).tabular();
        let label_h = u(14.0);
        let area = Rect::new(
            pad,
            top + u(6.0),
            (w - 2.0 * pad).max(1.0),
            h - u(8.0) - label_h,
        );
        let max = ((area.w / u(BAR_MIN_W)).floor() as usize).max(1);
        let bars = tl::bars(&self.library.albums.days, max);
        if bars.is_empty() {
            return;
        }
        self.library.albums.strip = Some(self.flip(strip));
        let picked = tl::selected(&self.filter.text);
        // The keyboard's bar, kept on the strip when the bars change.
        if let Some(k) = self.library.albums.kb {
            self.library.albums.kb = Some(k.min(bars.len() - 1));
        }
        let kb = self.library.albums.kb;
        let most = bars.iter().map(|b| b.count).max().unwrap_or(1).max(1);
        let bw = area.w / bars.len() as f32;
        let gap = if bw >= 4.0 { 1.0 } else { 0.0 };
        let loc = I18n::global().localizer();
        let mut last_label_end = f32::NEG_INFINITY;
        for (k, b) in bars.iter().enumerate() {
            let x = area.x + k as f32 * bw;
            let col = Rect::new(x, area.y, bw, area.h + label_h);
            let rr = self.flip(col);
            let hovered = self.pointer.is_some_and(|p| rr.contains(p));
            let on = picked.is_some_and(|(lo, hi)| b.within(lo, hi));
            if hovered {
                dl.rect(rr, u(3.0), pal.surface_hover);
            }
            if b.count > 0 {
                let bh = (area.h * b.count as f32 / most as f32).max(2.0);
                let bar = Rect::new(x + gap * 0.5, area.bottom() - bh, (bw - gap).max(1.0), bh);
                let c = if on {
                    pal.accent
                } else if picked.is_some() {
                    unpicked_bar(pal)
                } else {
                    pal.text_muted
                };
                dl.rect(self.flip(bar), 0.0, c);
            }
            if kb == Some(k) {
                dl.stroke_rect(rr.inset(-1.0), u(3.0), pal.focus_ring, 2.0);
            }
            // Period labels where they fit: years at their first bar, else months.
            let starts = match b.step {
                Step::Year => true,
                Step::Month => b.date().1 == 1 || k == 0,
                Step::Day => b.date().2 == 1 || k == 0,
            };
            if starts {
                let (y, m, _) = b.date();
                let s = match b.step {
                    Step::Year | Step::Month => y.to_string(),
                    Step::Day => loc.year_month(y as i32, m as u8).unwrap_or_default(),
                };
                let tw = text.measure(&s, small);
                if x > last_label_end + u(6.0) && x + tw <= area.right() + pad {
                    let lr = self.flip(Rect::new(x, area.bottom() + u(2.0), tw + 2.0, label_h));
                    text_at(text, dl, &s, small, lr, pal.text_muted, self.rtl);
                    last_label_end = x + tw;
                }
            }
            self.hits.push((rr, Hit::Library(Side::Bar(k))));
            self.library.albums.bars.push((*b, rr));
        }
        // The bar under the pointer, in words, at the strip's end.
        if let Some((b, _)) = self
            .library
            .albums
            .bars
            .iter()
            .find(|(_, r)| self.pointer.is_some_and(|p| r.contains(p)))
        {
            let s = bar_text(b);
            let tw = text.measure(&s, small) + u(12.0);
            let tr = Rect::new(w - pad - tw, top + u(2.0), tw, label_h + u(2.0));
            let tr = self.flip(tr);
            dl.rect(tr, u(4.0), pal.surface_raised);
            similar::put_centered(text, dl, &s, small, tr, pal.text);
        }
    }

    /// The bar the keyboard starts on: the picked period's first, else the last (latest).
    pub(super) fn timeline_start(&self) -> Option<usize> {
        let bars = &self.library.albums.bars;
        if bars.is_empty() {
            return None;
        }
        let picked = tl::selected(&self.filter.text);
        Some(
            bars.iter()
                .position(|(b, _)| picked.is_some_and(|(lo, hi)| b.within(lo, hi)))
                .unwrap_or(bars.len() - 1),
        )
    }

    /// A key while the timeline has the keyboard: ← → between bars (Shift extends the
    /// period picked), Home / End, Enter or Space picks, Tab on, Esc or ↓ to the grid.
    /// True: used.
    pub(super) fn timeline_key(&mut self, n: NamedKey, shift: bool) -> bool {
        let Some(k) = self.library.albums.kb else {
            return false;
        };
        let last = self.library.albums.bars.len().saturating_sub(1);
        let (back, on) = if self.rtl {
            (NamedKey::ArrowRight, NamedKey::ArrowLeft)
        } else {
            (NamedKey::ArrowLeft, NamedKey::ArrowRight)
        };
        let to = match n {
            _ if n == back => Some(k.saturating_sub(1)),
            _ if n == on => Some((k + 1).min(last)),
            NamedKey::Home => Some(0),
            NamedKey::End => Some(last),
            _ => None,
        };
        if let Some(to) = to {
            self.library.albums.kb = Some(to);
            if shift {
                // A run from where the keyboard was (or the run's own start).
                if self.library.albums.anchor.is_none() {
                    let from = self.library.albums.bars.get(k).map(|(b, _)| b.start);
                    self.library.albums.anchor = from;
                }
                self.timeline_press(to, true);
            } else if let Some((b, _)) = self.library.albums.bars.get(to) {
                self.announce = Some(bar_text(b));
            }
            return true;
        }
        match n {
            NamedKey::Enter | NamedKey::Space => self.timeline_press(k, shift),
            NamedKey::Tab if self.filter_open => self.tab(shift),
            NamedKey::Tab | NamedKey::Escape | NamedKey::ArrowDown => {
                self.go(super::filterbar::Stop::Grid)
            }
            NamedKey::ArrowUp if self.filter_open => self.go(super::filterbar::Stop::Field),
            _ => return false,
        }
        true
    }

    /// A timeline bar pressed: its period picked, or a run from the anchor with Shift, or
    /// none when it was the period picked.
    pub(super) fn timeline_press(&mut self, k: usize, shift: bool) {
        let Some(&(b, _)) = self.library.albums.bars.get(k) else {
            return;
        };
        let text = self.filter.text.clone();
        let picked = tl::selected(&text);
        let anchor = self
            .library
            .albums
            .anchor
            .and_then(|a| self.library.albums.bars.iter().find(|(x, _)| x.start == a))
            .map(|x| x.0);
        let value = match anchor {
            Some(a) if shift => Some(tl::range_value(&a, &b)),
            _ if picked
                == Some((
                    b.start * specular_query::date::DAY,
                    b.end * specular_query::date::DAY,
                )) =>
            {
                None
            }
            _ => Some(b.value()),
        };
        if !shift || anchor.is_none() {
            self.library.albums.anchor = value.is_some().then_some(b.start);
        }
        self.filter.text = tl::set(&text, value.as_deref());
        if !self.filter_open && value.is_some() {
            self.filter_open = true;
            self.relayout = true;
        }
        self.dirty = true;
        self.announce = Some(match &value {
            Some(_) => t!("albums-timeline-picked", period = bar_text(&b)),
            None => t!("albums-timeline-cleared"),
        });
    }

    /// AccessKit nodes of the timeline: a group of buttons, one per bar.
    pub(super) fn timeline_a11y(
        &self,
        tree: &mut accesskit::TreeUpdate,
        bounds: &dyn Fn(Rect) -> accesskit::Rect,
    ) {
        use accesskit::{Node, NodeId, Role, Toggled};
        let av = &self.library.albums;
        let Some(r) = av.strip.filter(|_| self.library.on) else {
            return;
        };
        let picked = tl::selected(&self.filter.text);
        let mut group = Node::new(Role::Group);
        group.set_label(t!("albums-timeline"));
        group.set_description(t!("albums-timeline-description"));
        group.set_bounds(bounds(r));
        let mut ids = Vec::new();
        for (k, (b, br)) in av.bars.iter().enumerate() {
            let id = NodeId(ID_BARS + k as u64);
            let mut n = Node::new(Role::Button);
            n.set_label(bar_text(b));
            n.set_toggled(if picked.is_some_and(|(lo, hi)| b.within(lo, hi)) {
                Toggled::True
            } else {
                Toggled::False
            });
            n.set_bounds(bounds(*br));
            n.add_action(accesskit::Action::Click);
            if av.kb == Some(k) && tree.focus == NodeId(1) {
                tree.focus = id;
            }
            tree.nodes.push((id, n));
            ids.push(id);
        }
        group.set_children(ids);
        tree.nodes.push((NodeId(ID_TIMELINE), group));
        if let Some((_, root)) = tree.nodes.iter_mut().find(|(i, _)| *i == NodeId(1)) {
            root.push_child(NodeId(ID_TIMELINE));
        }
    }
}

impl Gallery {
    /// The AccessKit node of an albums section row (sidebar row `k`); its buttons' nodes
    /// go in `tree` and their ids in `extra`.
    pub(super) fn album_node(
        &self,
        side: Side,
        r: Rect,
        bounds: &dyn Fn(Rect) -> accesskit::Rect,
        tree: &mut accesskit::TreeUpdate,
        extra: &mut Vec<accesskit::NodeId>,
        k: usize,
    ) -> accesskit::Node {
        use accesskit::{Node, NodeId, Role, Toggled};
        let v = &self.library.albums;
        let (_, name, sub, _) =
            self.album_look(side)
                .unwrap_or((Icon::Plus, String::new(), None, 0));
        let label = match &sub {
            Some(sub) => format!("{name}, {sub}"),
            None => name.clone(),
        };
        let mut n;
        let mut buttons: Vec<String> = Vec::new();
        match side {
            Side::AlbumsTitle => {
                n = Node::new(Role::Button);
                n.set_label(t!("albums-title"));
                n.set_expanded(!v.hidden);
            }
            Side::Group(g) => {
                n = Node::new(Role::Button);
                n.set_label(label);
                let full = self.albums_group(g).unwrap_or_default();
                n.set_expanded(!v.folded.contains(&full.to_lowercase()));
                if !self.album_buttons(side).is_empty() {
                    let g = shown_name(&full);
                    buttons.push(t!("albums-a11y-rename-group", group = g.as_str()));
                    buttons.push(t!("albums-a11y-delete-group", group = g));
                }
            }
            Side::Album(id) => {
                n = Node::new(Role::RadioButton);
                n.set_label(label);
                n.set_toggled(if v.open == Some(id) {
                    Toggled::True
                } else {
                    Toggled::False
                });
                if !self.album_buttons(side).is_empty() {
                    let a = self
                        .album(id)
                        .map(|a| shown_name(&a.name))
                        .unwrap_or_default();
                    let smart = self.album(id).is_some_and(|a| a.kind == AlbumKind::Smart);
                    buttons.push(if smart {
                        t!("albums-a11y-edit", album = a.as_str())
                    } else {
                        t!("albums-a11y-rename", album = a.as_str())
                    });
                    buttons.push(t!("albums-a11y-delete", album = a));
                }
            }
            _ => {
                n = Node::new(Role::Button);
                n.set_label(name);
            }
        }
        for (j, b) in buttons.into_iter().enumerate() {
            let id = NodeId(ID_ALBUM_BUTTONS + 2 * k as u64 + j as u64);
            let mut bn = Node::new(Role::Button);
            bn.set_label(b);
            bn.set_bounds(bounds(r));
            bn.add_action(accesskit::Action::Click);
            tree.nodes.push((id, bn));
            extra.push(id);
        }
        n
    }
}

impl App {
    /// AccessKit: an action on an album row's button or a timeline bar; false when not
    /// one of them.
    pub(super) fn albums_a11y_action(&mut self, target: u64, click: bool) -> bool {
        let ga = &self.gallery;
        let n_rows = ga.library.rows.len() as u64;
        if (ID_ALBUM_BUTTONS..ID_ALBUM_BUTTONS + 2 * n_rows).contains(&target) {
            let k = ((target - ID_ALBUM_BUTTONS) / 2) as usize;
            let delete = (target - ID_ALBUM_BUTTONS) % 2 == 1;
            let side = match (ga.library.rows.get(k).map(|r| r.0), delete) {
                (Some(Side::Album(id)), false) => Side::AlbumEdit(id),
                (Some(Side::Album(id)), true) => Side::AlbumDelete(id),
                (Some(Side::Group(g)), false) => Side::GroupEdit(g),
                (Some(Side::Group(g)), true) => Side::GroupDelete(g),
                _ => return false,
            };
            if click {
                self.albums_hit(side);
            }
            return true;
        }
        let bars = ga.library.albums.bars.len() as u64;
        if (ID_BARS..ID_BARS + bars).contains(&target) {
            if click {
                self.gallery
                    .timeline_press((target - ID_BARS) as usize, false);
            }
            return true;
        }
        false
    }
}

/// AccessKit ids: the timeline's group and its bars.
pub(super) const ID_TIMELINE: u64 = super::library::A11Y_LIBRARY + (1 << 28);
pub(super) const ID_BARS: u64 = ID_TIMELINE + 1;
/// The albums rows' edit and delete buttons: `ID_ALBUM_BUTTONS + 2k` and `+ 2k + 1` for
/// sidebar row `k`.
pub(super) const ID_ALBUM_BUTTONS: u64 = super::library::A11Y_LIBRARY + (1 << 29);

/// WCAG contrast ratio of two opaque colours.
fn contrast(a: specular_theme::Color, b: specular_theme::Color) -> f32 {
    let (la, lb) = (a.luminance(), b.luminance());
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// The colour of the bars outside the period picked: dimmer than the others, but at least
/// 3:1 against the strip (WCAG's contrast for graphics), in both themes.
pub(super) fn unpicked_bar(pal: &specular_theme::Palette) -> specular_theme::Color {
    let (bg, fg) = (pal.surface.with_alpha(1.0), pal.text_muted.with_alpha(1.0));
    (1..=20)
        .map(|i| bg.lerp(fg, i as f32 / 20.0))
        .find(|c| contrast(*c, bg) >= 3.0)
        .unwrap_or(fg)
}

/// A bar in words: "June 2025, 312 items".
pub(super) fn bar_text(b: &Bar) -> String {
    let loc = I18n::global().localizer();
    let (y, m, d) = b.date();
    let period = match b.step {
        Step::Year => y.to_string(),
        Step::Month => loc.year_month(y as i32, m as u8).unwrap_or_default(),
        Step::Day => loc
            .date(y as i32, m as u8, d as u8, specular_i18n::DateStyle::Medium)
            .unwrap_or_default(),
    };
    specular_i18n::dot_join(&[period, t!("gallery-items", count = b.count as i64)])
}

impl App {
    fn albums_wake(&self) -> crate::loader::Wake {
        self.loader.waker()
    }

    fn albums_op(&mut self, op: Op) {
        let w = self.albums_wake();
        self.gallery.albums_run(op, w);
        self.redraw = true;
    }

    /// An album change came back: the list, a toast, and the search again.
    pub(super) fn albums_done(&mut self, d: Done) {
        let now = self.now();
        let ga = &mut self.gallery;
        let av = &mut ga.library.albums;
        av.busy = false;
        av.loaded = true;
        if let Some(l) = d.list {
            av.list = l;
        }
        let list = av.list.clone();
        let ids: Vec<i64> = av.list.iter().map(|a| a.id).collect();
        if av.open.is_some_and(|id| !ids.contains(&id)) {
            av.open = None;
        }
        if av.target.is_some_and(|id| !ids.contains(&id)) {
            av.target = None;
        }
        let name_of = |id: i64| {
            list.iter()
                .find(|a| a.id == id)
                .map(|a| shown_name(&a.name))
                .unwrap_or_default()
        };
        let mut toast = None;
        if let Some(e) = d.error {
            toast = Some(t!("albums-failed", reason = e));
        } else {
            match &d.op {
                Op::Load => {}
                Op::Create {
                    kind, add, open, ..
                } => {
                    if let Some(id) = d.made {
                        if *kind == AlbumKind::Manual {
                            av.target = Some(id);
                        }
                        if *open {
                            av.open = Some(id);
                            ga.library.location = None;
                        }
                        toast = Some(if add.is_empty() {
                            t!("albums-created", album = name_of(id))
                        } else {
                            added_text(d.added, d.missing, &name_of(id))
                        });
                    }
                }
                Op::Add(id, _) => {
                    av.target = Some(*id);
                    toast = Some(added_text(d.added, d.missing, &name_of(*id)));
                }
                Op::Remove(id, _) => {
                    toast = Some(t!(
                        "albums-removed",
                        count = d.removed as i64,
                        album = name_of(*id)
                    ));
                }
                Op::Delete(ids) => {
                    toast = Some(t!("albums-deleted", count = ids.len() as i64));
                }
                Op::Rename(..) | Op::RenameGroup(..) | Op::Edit { .. } => {}
            }
        }
        if let Some(t) = toast {
            ga.announce = Some(t.clone());
            self.chrome.toast(t, None, now);
        }
        // The album's photos (or its rules) may have changed: search again.
        self.gallery.library_search_soon();
        self.redraw = true;
    }

    /// A sidebar item of the albums section pressed.
    pub(super) fn albums_hit(&mut self, s: Side) {
        let shift = self.mods.shift_key();
        let ga = &mut self.gallery;
        match s {
            Side::AlbumsTitle => {
                ga.library.albums.hidden = !ga.library.albums.hidden;
            }
            Side::Group(k) => {
                if let Some(g) = ga.albums_group(k) {
                    let key = g.to_lowercase();
                    let f = &mut ga.library.albums.folded;
                    if !f.remove(&key) {
                        f.insert(key);
                    }
                }
            }
            Side::Album(id) => {
                ga.library.albums.open = Some(id);
                ga.library.location = None;
                ga.library_search_soon();
            }
            Side::AlbumEdit(id) => self.albums_edit(id),
            Side::AlbumDelete(id) => self.albums_ask_delete(vec![id], None),
            Side::GroupEdit(k) => {
                if let Some(g) = self.gallery.albums_group(k) {
                    let s = AlbumSheet::rename_group(self.gallery.library.albums.list.clone(), &g);
                    self.albums_sheet(s);
                }
            }
            Side::GroupDelete(k) => {
                if let Some(g) = self.gallery.albums_group(k) {
                    let ids: Vec<i64> = al::in_group(&self.gallery.library.albums.list, &g)
                        .map(|a| a.id)
                        .collect();
                    self.albums_ask_delete(ids, Some(g));
                }
            }
            Side::NewAlbum => {
                let g = self.gallery.albums_current_group();
                let s = AlbumSheet::new_album(
                    self.gallery.library.albums.list.clone(),
                    g.as_deref(),
                    Vec::new(),
                );
                self.albums_sheet(s);
            }
            Side::NewSmart => self.albums_new_smart(),
            Side::Bar(k) => self.gallery.timeline_press(k, shift),
            _ => {}
        }
        self.redraw = true;
    }

    /// "New smart album…": its rules start from the filter bar's query.
    pub(super) fn albums_new_smart(&mut self) {
        let g = self.gallery.albums_current_group();
        let q = self.gallery.filter.text.clone();
        if let Some(s) = AlbumSheet::smart(
            self.gallery.library.albums.list.clone(),
            None,
            g.as_deref(),
            &q,
        ) {
            self.albums_sheet(s);
        }
    }

    /// Rename a manual album, or edit a smart one's name and rules.
    fn albums_edit(&mut self, id: i64) {
        let list = self.gallery.library.albums.list.clone();
        let s = match self.gallery.album(id).map(|a| a.kind) {
            Some(AlbumKind::Smart) => AlbumSheet::smart(list, Some(id), None, ""),
            Some(AlbumKind::Manual) => AlbumSheet::rename(list, id),
            None => None,
        };
        if let Some(s) = s {
            self.albums_sheet(s);
        }
    }

    fn albums_sheet(&mut self, s: AlbumSheet) {
        if self.gallery.library.lib.is_none() {
            return;
        }
        self.fileops_open_album(s);
    }

    /// Delete albums: asked first, naming them; the photos stay.
    pub(super) fn albums_ask_delete(&mut self, ids: Vec<i64>, group: Option<String>) {
        if ids.is_empty() {
            return;
        }
        let title = match (&group, ids.as_slice()) {
            (Some(g), _) => t!(
                "albums-delete-group-confirm",
                group = shown_name(g),
                count = ids.len() as i64
            ),
            (None, [id]) => {
                let Some(a) = self.gallery.album(*id) else {
                    return;
                };
                t!("albums-delete-confirm", album = shown_name(&a.name))
            }
            _ => return,
        };
        self.gallery.library.albums.delete_ask = Some(ids);
        self.chrome.confirm_destructive(
            title,
            t!("albums-delete-confirm-detail"),
            t!("albums-delete"),
            crate::app::items::ALBUM_DELETE,
        );
        self.redraw = true;
    }

    /// The deletion was confirmed.
    pub(in crate::app) fn albums_delete_confirmed(&mut self) {
        if let Some(ids) = self.gallery.library.albums.delete_ask.take() {
            self.albums_op(Op::Delete(ids));
        }
    }

    /// `B`: the selection into the target album, or asks which album without one (or with
    /// `choose`, `Shift+B`).
    pub(super) fn albums_add_key(&mut self, choose: bool) {
        let ga = &self.gallery;
        if ga.library.lib.as_ref().is_none_or(|l| l.read_only()) {
            if ga.library.lib.is_none() {
                let now = self.now();
                self.chrome.toast(t!("albums-need-library"), None, now);
                self.redraw = true;
            }
            return;
        }
        let paths = self.fileops_targets();
        if paths.is_empty() {
            return;
        }
        let target = ga
            .library
            .albums
            .target
            .filter(|id| ga.album(*id).is_some_and(|a| a.kind == AlbumKind::Manual));
        match target {
            Some(id) if !choose => self.albums_op(Op::Add(id, paths)),
            _ => {
                let s = AlbumSheet::add_to(ga.library.albums.list.clone(), target, paths);
                self.albums_sheet(s);
            }
        }
    }

    /// "Remove from album": the selection out of the manual album shown, asked first.
    pub(super) fn albums_remove_selected(&mut self) {
        let Some(a) = self.gallery.albums_open_manual().cloned() else {
            return;
        };
        let paths = self.fileops_targets();
        if paths.is_empty() {
            return;
        }
        let title = t!(
            "albums-remove-confirm",
            count = paths.len() as i64,
            album = shown_name(&a.name)
        );
        self.gallery.library.albums.remove_ask = Some((a.id, paths));
        self.chrome.confirm_destructive(
            title,
            t!("albums-remove-confirm-detail"),
            t!("albums-remove"),
            crate::app::items::ALBUM_REMOVE_PHOTOS,
        );
        self.redraw = true;
    }

    pub(in crate::app) fn albums_remove_confirmed(&mut self) {
        if let Some((id, paths)) = self.gallery.library.albums.remove_ask.take() {
            self.albums_op(Op::Remove(id, paths));
        }
    }

    /// The album sheet's OK.
    pub(in crate::app) fn albums_sheet_ok(&mut self) {
        let Some(s) = self.fileops_take_album() else {
            return;
        };
        if !s.ready() {
            self.fileops_open_album(s);
            return;
        }
        let op = match s.purpose.clone() {
            Purpose::NewAlbum(add) => Op::Create {
                name: s.typed(),
                kind: AlbumKind::Manual,
                rules: Rules::default(),
                open: add.is_empty(),
                add,
            },
            Purpose::Rename(id) => Op::Rename(id, s.typed()),
            Purpose::RenameGroup(g) => Op::RenameGroup(g, s.typed()),
            Purpose::Smart(None) => Op::Create {
                name: s.name_text(),
                kind: AlbumKind::Smart,
                rules: s.rules_now(),
                add: Vec::new(),
                open: true,
            },
            Purpose::Smart(Some(id)) => Op::Edit {
                id,
                name: s.name_text(),
                rules: s.rules_now(),
            },
            Purpose::AddTo(paths) => {
                let t = s.typed();
                if t.is_empty() {
                    match s.targets().get(s.sel) {
                        Some(a) => Op::Add(a.id, paths),
                        None => return,
                    }
                } else {
                    Op::Create {
                        name: t,
                        kind: AlbumKind::Manual,
                        rules: Rules::default(),
                        add: paths,
                        open: false,
                    }
                }
            }
        };
        self.albums_op(op);
    }
}

/// "Added N to album" (and how many were not in the Library).
fn added_text(added: usize, missing: usize, album: &str) -> String {
    let mut s = t!("albums-added", count = added as i64, album = album);
    if missing > 0 {
        s.push(' ');
        s.push_str(&t!("albums-added-missing", count = missing as i64));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn album(id: i64, name: &str, kind: AlbumKind) -> Album {
        Album {
            id,
            name: name.into(),
            kind,
            rules: Rules::default(),
            files: 2,
        }
    }

    fn list() -> Vec<Album> {
        vec![
            album(1, "Client X/all", AlbumKind::Manual),
            album(2, "Client X/selects", AlbumKind::Manual),
            album(3, "Client X/web/finals", AlbumKind::Smart),
            album(4, "Portfolio", AlbumKind::Manual),
        ]
    }

    #[test]
    fn unpicked_bars_are_readable_in_both_themes() {
        for pal in [
            specular_theme::Palette::dark(),
            specular_theme::Palette::light(),
        ] {
            let bg = pal.surface.with_alpha(1.0);
            let c = unpicked_bar(&pal);
            assert!(contrast(c, bg) >= 3.0, "{:?}", pal.mode);
            // Still dimmer than the bars when nothing is picked.
            assert!(
                contrast(c, bg) < contrast(pal.text_muted, bg),
                "{:?}",
                pal.mode
            );
            // The old colour (the separator) was not: about 1.2:1.
            assert!(contrast(bg.lerp(pal.separator.with_alpha(1.0), pal.separator.a), bg) < 1.5);
        }
    }

    #[test]
    fn the_tree_has_groups_and_folds() {
        let t = tree(&list(), &HashSet::new());
        assert_eq!(
            t,
            vec![
                Node::Group {
                    name: "Client X".into(),
                    depth: 0,
                    albums: 3
                },
                Node::Album { id: 1, depth: 1 },
                Node::Album { id: 2, depth: 1 },
                Node::Group {
                    name: "Client X/web".into(),
                    depth: 1,
                    albums: 1
                },
                Node::Album { id: 3, depth: 2 },
                Node::Album { id: 4, depth: 0 },
            ]
        );
        let folded: HashSet<String> = ["client x".to_owned()].into();
        let t = tree(&list(), &folded);
        assert_eq!(t.len(), 2, "{t:?}");
        let mut v = AlbumsView {
            list: list(),
            ..AlbumsView::default()
        };
        assert_eq!(items(&v, true).len(), 9);
        assert_eq!(items(&v, false).len(), 7);
        v.hidden = true;
        assert_eq!(items(&v, true), vec![Side::AlbumsTitle]);
    }

    #[test]
    fn the_smart_sheet_edits_name_and_rules() {
        let mut s = AlbumSheet::smart(list(), None, Some("Client X"), "rating:>=4").unwrap();
        assert_eq!(s.typed(), "Client X /");
        assert!(!s.ready(), "a leaf name is wanted");
        s.field.set_text("Client X / picks");
        assert!(s.ready());
        // Rows: name, the rule, "Add a rule".
        assert!(s.press(2));
        assert_eq!(s.sel, 2);
        assert_eq!(s.rules.len(), 2);
        assert_eq!(s.rules[1].combine, Combine::Or);
        s.field.set_text("tag:client");
        s.set_choice(2);
        let r = s.rules_now();
        assert_eq!(r.0[1], Rule::new(Combine::Except, "tag:client"));
        assert_eq!(s.name_text(), "Client X / picks");
        let v = s.view(Part::Field);
        assert_eq!(v.rows.len(), 4);
        assert!(v.choice.is_some());
        assert!(v.buttons[0].enabled);
        // Back to the first rule: no choice of word.
        s.press(1);
        assert_eq!(s.typed(), "rating:>=4");
        assert!(s.view(Part::Field).choice.is_none());
        // Empty rules only: not ready.
        s.field.set_text("");
        s.rules[1].query.clear();
        assert!(!s.ready());
        // A taken name.
        let mut t = AlbumSheet::smart(list(), None, None, "x").unwrap();
        t.field.set_text("portfolio");
        assert!(matches!(t.name_problem(), Some(NameProblem::Taken(_))));
        let mut t = AlbumSheet::smart(list(), Some(3), None, "").unwrap();
        assert_eq!(t.typed(), "Client X / web / finals");
        assert!(t.ready() || t.rules_now().is_empty());
        t.field.set_text("Client X/web/finals");
        assert_eq!(t.name_problem(), None, "its own name");
    }

    #[test]
    fn add_to_lists_manual_albums_or_makes_one() {
        let mut s = AlbumSheet::add_to(list(), Some(2), vec![PathBuf::from("/a.jpg")]);
        assert_eq!(s.targets().len(), 3);
        assert_eq!(s.sel, 1);
        assert!(s.ready());
        s.press(2);
        assert_eq!(s.targets()[s.sel].id, 4);
        s.field.set_text("Client X/new");
        assert!(s.ready());
        s.field.set_text("portfolio");
        assert!(!s.ready());
        let v = s.view(Part::Field);
        assert!(v.error.is_some());
        let none = AlbumSheet::add_to(Vec::new(), None, vec![PathBuf::from("/a.jpg")]);
        assert!(!none.ready());
    }

    #[test]
    fn renaming_a_group_checks_every_album_in_it() {
        let mut s = AlbumSheet::rename_group(list(), "Client X");
        assert_eq!(s.typed(), "Client X");
        assert!(s.ready());
        s.field.set_text("Portfolio");
        assert!(matches!(s.name_problem(), Some(NameProblem::Clash(_))));
        s.field.set_text("Client Y");
        assert!(s.ready());
        s.field.set_text(" / ");
        assert_eq!(s.name_problem(), Some(NameProblem::Empty));
    }
}
