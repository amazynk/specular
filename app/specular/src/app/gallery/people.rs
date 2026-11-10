//! The People view (Local AI's People, DESIGN.md §10): the people found in this library's
//! photos, each shown by their clearest face, named or numbered, with how many faces are
//! theirs.
//!
//! - Enter (or a double click) shows their photos in the Library (`person:#<id>`).
//! - `F2` names a person (typed in place; Enter keeps it, Esc leaves it).
//! - Space selects; `M` merges the selected people into the focused one, after asking.
//! - Dragging a person onto another merges them, after asking (both named; photos on disk
//!   don't change). The question is the same dialog either way, keyboard first: it starts
//!   on Merge, Tab goes round Cancel and Merge, Esc keeps them apart.
//! - `F` opens a person's faces; there `Delete` takes a face from them ("Not this person":
//!   the face stays, grouping never puts it back), Esc goes back.
//!
//! Grouping is off until it is turned on for the library (Settings › Local AI); the view
//! says so. Everything is read from and written to the catalogue on threads of their own.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

use specular_gfx::{DrawList, Icon, Rect, Text, TextStyle};
use specular_i18n::t;
use specular_input::{KeyCombo, NamedKey};
use specular_widgets::{PaletteItem, accesskit};

use super::draw::{Frame, text_at, text_in, truncate};
use super::{A11Y_BASE, App, item};

/// The view's palette codes.
pub(super) mod code {
    pub const OPEN: u64 = 800;
    pub const CLOSE: u64 = 801;
    pub const NAME: u64 = 802;
    pub const MERGE: u64 = 803;
    pub const FACES: u64 = 804;
    pub const SHOW: u64 = 805;
    pub const NOT_THIS: u64 = 806;

    pub fn owns(c: u64) -> bool {
        (800..850).contains(&c)
    }
}

/// The view's screen-reader nodes: the group, its status, then one per card.
pub(super) const A11Y_PEOPLE: u64 = A11Y_BASE + (16 << 40);
const A11Y_STATUS: u64 = 1;
const A11Y_CARD: u64 = 100;

const TITLE: TextStyle = TextStyle::new(15.0).weight(600.0);
const BODY: TextStyle = TextStyle::new(13.0);
const SMALL: TextStyle = TextStyle::new(11.0);
const HEADER: f32 = 44.0;
/// A card's face side at text scale 1, logical px.
const CARD: f32 = 112.0;

/// Someone found.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Card {
    pub id: i64,
    pub name: Option<String>,
    pub faces: u64,
    /// The face shown: its photo and box (fractions of the photo).
    pub cover: Option<(PathBuf, [f32; 4])>,
}

/// One face of a person (their faces level).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct FaceCard {
    pub id: i64,
    pub path: PathBuf,
    pub rect: [f32; 4],
}

/// What the catalogue gave.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Loaded {
    /// Grouping is on for this library (None: People is not installed).
    pub grouping: Option<bool>,
    pub people: Vec<Card>,
    /// The faces of the person opened.
    pub faces: Option<(i64, Vec<FaceCard>)>,
}

#[derive(Default)]
pub(super) struct PeopleView {
    pub open: bool,
    pub data: Loaded,
    /// The person whose faces are shown (None: everyone).
    pub person: Option<i64>,
    pub focus: usize,
    pub selected: BTreeSet<i64>,
    /// A name being typed: the person and the text.
    pub naming: Option<(i64, String)>,
    rx: Option<Receiver<Loaded>>,
    pub loading: bool,
    /// Thumbnails wanted on screen (path, size class).
    pub wanted: Vec<(PathBuf, u32)>,
    hits: Vec<(Rect, usize)>,
    buttons: Vec<(Rect, u64)>,
    last_click: Option<(usize, f64)>,
    /// A person card pressed and maybe being dragged.
    pub drag: Option<Drag>,
    /// The merge asked about: (people merged, person they merge into).
    pub merge_ask: Option<(Vec<i64>, i64)>,
}

/// A press on a person card that may become a drag onto another.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Drag {
    /// The card pressed (index).
    pub from: usize,
    pub start: (f32, f32),
    pub at: (f32, f32),
    /// Moved far enough to be a drag.
    pub moving: bool,
}

/// Pointer travel (logical px) before a press on a card becomes a drag.
const DRAG_START: f32 = 6.0;

impl PeopleView {
    /// Card `i`'s rectangle as last drawn (screenshot tests).
    #[cfg(test)]
    pub(super) fn card_rect(&self, i: usize) -> Option<Rect> {
        self.hits.iter().find(|h| h.1 == i).map(|h| h.0)
    }

    /// How many cards are shown (people, or the opened person's faces).
    fn len(&self) -> usize {
        match (&self.person, &self.data.faces) {
            (Some(p), Some((q, f))) if p == q => f.len(),
            (Some(_), _) => 0,
            (None, _) => self.data.people.len(),
        }
    }

    fn focused_person(&self) -> Option<&Card> {
        if self.person.is_some() {
            return None;
        }
        self.data.people.get(self.focus)
    }

    /// What `M` merges: the selected people (but the focused one) into the focused one.
    pub(super) fn merge_request(&self) -> Option<(Vec<i64>, i64)> {
        let into = self.focused_person()?.id;
        let from: Vec<i64> = self
            .selected
            .iter()
            .copied()
            .filter(|&i| i != into)
            .collect();
        (!from.is_empty()).then_some((from, into))
    }

    /// The question for merging `from` into `into` (ids): the title and what follows.
    pub(super) fn merge_question_for(&self, from: &[i64], into: i64) -> Option<(String, String)> {
        let card = |id: i64| self.data.people.iter().find(|c| c.id == id);
        let into = card(into)?;
        match from {
            [one] => Some(merge_question(card(*one)?, into)),
            _ => Some((
                t!(
                    "people-merge-ask-many",
                    count = from.len() as i64,
                    into = label(into)
                ),
                t!("people-merge-ask-detail", into = label(into)),
            )),
        }
    }

    fn focused_face(&self) -> Option<&FaceCard> {
        match (&self.person, &self.data.faces) {
            (Some(p), Some((q, f))) if p == q => f.get(self.focus),
            _ => None,
        }
    }
}

/// A person's label: their name, or "Person #12".
pub(super) fn label(c: &Card) -> String {
    match &c.name {
        Some(n) => n.clone(),
        None => t!("people-unnamed", id = c.id),
    }
}

/// The question asked when `from` is dropped on `into`: the title and what follows.
pub(super) fn merge_question(from: &Card, into: &Card) -> (String, String) {
    (
        t!("people-merge-ask", from = label(from), into = label(into)),
        t!("people-merge-ask-detail", into = label(into)),
    )
}

fn dir(rtl: bool) -> specular_i18n::Direction {
    if rtl {
        specular_i18n::Direction::Rtl
    } else {
        specular_i18n::Direction::Ltr
    }
}

/// Reads people (and the opened person's faces) from the catalogue.
fn load(db: &std::path::Path, person: Option<i64>) -> Loaded {
    use specular_library::ai;
    let Ok(conn) = ai::open(db) else {
        return Loaded::default();
    };
    if !ai::has(&conn, "people").unwrap_or(false) {
        return Loaded::default();
    }
    let path = |file: i64| ai::file_path(&conn, file).ok().flatten();
    let face_of = |id: i64| -> Option<ai::FaceRow> {
        conn.query_row(
            "SELECT file_id, x, y, w, h FROM faces WHERE id = ?1",
            [id],
            |r| {
                Ok(ai::FaceRow {
                    id,
                    file_id: r.get(0)?,
                    rect: [r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?],
                    ..Default::default()
                })
            },
        )
        .ok()
    };
    let people = ai::people(&conn)
        .unwrap_or_default()
        .into_iter()
        .map(|p| Card {
            id: p.id,
            name: p.name,
            faces: p.faces,
            cover: p
                .cover
                .and_then(face_of)
                .and_then(|f| Some((path(f.file_id)?, f.rect))),
        })
        .collect();
    let faces = person.map(|p| {
        let f = ai::person_faces(&conn, p)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|f| {
                Some(FaceCard {
                    id: f.id,
                    path: path(f.file_id)?,
                    rect: f.rect,
                })
            })
            .collect();
        (p, f)
    });
    Loaded {
        grouping: ai::face_grouping(&conn).ok(),
        people,
        faces,
    }
}

/// A change the user made, written on a thread.
enum Change {
    Name(i64, String),
    Merge(Vec<i64>, i64),
    NotThis(i64),
}

impl App {
    fn people_db() -> Option<PathBuf> {
        crate::app::ai::pack().and_then(|p| p.catalogue().map(PathBuf::from))
    }

    /// Reads the people again (after `change`, when given), on a thread.
    fn people_reload(&mut self, change: Option<Change>) {
        let Some(db) = Self::people_db() else {
            self.gallery.people.loading = false;
            return;
        };
        let person = self.gallery.people.person;
        let (tx, rx) = channel();
        self.gallery.people.rx = Some(rx);
        self.gallery.people.loading = true;
        let wake = self.loader.waker();
        let spawned = specular_thread::builder("ai-people").spawn(move || {
            use specular_library::ai;
            if let Some(c) = change
                && let Ok(mut conn) = ai::open(&db)
            {
                let r = match c {
                    Change::Name(p, n) => ai::name_person(&conn, p, Some(&n)),
                    Change::Merge(from, into) => from
                        .into_iter()
                        .filter(|&f| f != into)
                        .try_for_each(|f| ai::merge_people(&mut conn, f, into)),
                    Change::NotThis(face) => ai::place_face(&conn, face, None),
                };
                if let Err(e) = r {
                    specular_log::warn!(target: specular_log::UI, "people: {e}");
                }
            }
            let _ = tx.send(load(&db, person));
            wake();
        });
        if let Err(e) = spawned {
            specular_log::warn!(target: specular_log::UI, "people: {e}");
        }
    }

    /// Once per turn while the view is open: what the catalogue gave.
    pub(super) fn people_poll(&mut self) {
        let pv = &mut self.gallery.people;
        if let Some(rx) = &pv.rx
            && let Ok(d) = rx.try_recv()
        {
            pv.data = d;
            pv.rx = None;
            pv.loading = false;
            pv.focus = pv.focus.min(pv.len().saturating_sub(1));
            let ids: BTreeSet<i64> = pv.data.people.iter().map(|c| c.id).collect();
            pv.selected.retain(|i| ids.contains(i));
            self.gallery.wish_key = 0;
            self.redraw = true;
        }
    }

    pub(super) fn people_open(&mut self) {
        let pv = &mut self.gallery.people;
        pv.open = true;
        pv.person = None;
        pv.focus = 0;
        pv.naming = None;
        pv.selected.clear();
        self.people_reload(None);
        self.redraw = true;
    }

    pub(super) fn people_close(&mut self) {
        let pv = &mut self.gallery.people;
        pv.open = false;
        pv.naming = None;
        pv.wanted.clear();
        self.gallery.wish_key = 0;
        self.redraw = true;
    }

    /// The palette entries: open the view, and its actions while open.
    pub(super) fn people_menu_items(&self) -> Vec<PaletteItem> {
        let pv = &self.gallery.people;
        if !pv.open {
            return vec![item(code::OPEN, t!("people-open"), None, Icon::Image)];
        }
        let mut v = Vec::new();
        if pv.person.is_none() {
            v.push(item(
                code::SHOW,
                t!("people-show-photos"),
                Some("Enter"),
                Icon::Search,
            ));
            v.push(item(code::NAME, t!("people-name"), Some("F2"), Icon::Image));
            v.push(item(
                code::FACES,
                t!("people-faces-open"),
                Some("F"),
                Icon::Grid,
            ));
            if !pv.selected.is_empty() {
                v.push(item(code::MERGE, t!("people-merge"), Some("M"), Icon::Plus));
            }
        } else {
            v.push(item(
                code::NOT_THIS,
                t!("people-not-this"),
                Some("Delete"),
                Icon::Minus,
            ));
        }
        v.push(item(
            code::CLOSE,
            t!("people-done"),
            Some("Esc"),
            Icon::Close,
        ));
        v
    }

    pub(super) fn people_code(&mut self, c: u64) {
        match c {
            code::OPEN => {
                if self.gallery.people.open {
                    self.people_close();
                } else {
                    self.people_open();
                }
            }
            code::CLOSE => self.people_close(),
            code::NAME => self.people_start_naming(),
            code::MERGE => self.people_merge(),
            code::FACES => self.people_faces(),
            code::SHOW => self.people_show(),
            code::NOT_THIS => self.people_not_this(),
            _ => {}
        }
        self.redraw = true;
    }

    fn people_start_naming(&mut self) {
        let pv = &mut self.gallery.people;
        if let Some(c) = pv.focused_person() {
            pv.naming = Some((c.id, c.name.clone().unwrap_or_default()));
        }
    }

    /// `M`: asks before merging the selected people into the focused one.
    fn people_merge(&mut self) {
        let pv = &self.gallery.people;
        let Some((from, into)) = pv.merge_request() else {
            return;
        };
        let Some((title, detail)) = pv.merge_question_for(&from, into) else {
            return;
        };
        self.people_ask_merge(from, into, title, detail);
    }

    /// The merge question (the same dialog for `M` and a drag).
    fn people_ask_merge(&mut self, from: Vec<i64>, into: i64, title: String, detail: String) {
        self.gallery.people.merge_ask = Some((from, into));
        self.chrome.confirm_with_detail(
            title,
            detail,
            t!("people-merge"),
            crate::app::items::PEOPLE_MERGE,
        );
        self.redraw = true;
    }

    fn people_faces(&mut self) {
        let Some(id) = self.gallery.people.focused_person().map(|c| c.id) else {
            return;
        };
        let pv = &mut self.gallery.people;
        pv.person = Some(id);
        pv.focus = 0;
        self.people_reload(None);
    }

    fn people_back(&mut self) {
        let pv = &mut self.gallery.people;
        let was = pv.person.take();
        pv.focus = was
            .and_then(|p| pv.data.people.iter().position(|c| c.id == p))
            .unwrap_or(0);
        self.people_reload(None);
    }

    fn people_not_this(&mut self) {
        let Some(face) = self.gallery.people.focused_face().map(|f| f.id) else {
            return;
        };
        self.people_reload(Some(Change::NotThis(face)));
        self.gallery.announce = Some(t!("people-face-removed"));
    }

    /// Their photos in the Library.
    fn people_show(&mut self) {
        let Some(id) = self.gallery.people.focused_person().map(|c| c.id) else {
            return;
        };
        self.people_close();
        self.library_enter();
        let ga = &mut self.gallery;
        ga.filter.text = format!("person:#{id}");
        ga.dirty = true;
        ga.relayout = true;
        self.redraw = true;
    }

    /// Keys while the view is open. True when used.
    pub(super) fn people_key(&mut self, ev_text: Option<&str>, combo: Option<&KeyCombo>) -> bool {
        let named = combo.and_then(|c| match c.key {
            specular_input::Key::Named(n) => Some(n),
            _ => None,
        });
        let plain = combo.is_none_or(|c| !c.mods.ctrl && !c.mods.alt && !c.mods.meta);
        // Typing a name.
        if let Some((id, s)) = self.gallery.people.naming.as_mut() {
            match named {
                Some(NamedKey::Enter) => {
                    let (id, s) = (*id, s.trim().to_owned());
                    self.gallery.people.naming = None;
                    self.people_reload(Some(Change::Name(id, s)));
                }
                Some(NamedKey::Escape) => self.gallery.people.naming = None,
                Some(NamedKey::Backspace) => {
                    s.pop();
                }
                _ => {
                    if let Some(t) = ev_text.filter(|t| plain && !t.chars().any(char::is_control)) {
                        s.push_str(t);
                    } else {
                        return false;
                    }
                }
            }
            self.redraw = true;
            return true;
        }
        let n = self.gallery.people.len();
        let cols = self.people_columns();
        let rtl = self.rtl();
        let pv = &mut self.gallery.people;
        let step = |pv: &mut PeopleView, d: isize| {
            if n > 0 {
                pv.focus = (pv.focus as isize + d).clamp(0, n as isize - 1) as usize;
            }
        };
        match named {
            Some(NamedKey::Escape) => {
                if pv.person.is_some() {
                    self.people_back();
                } else {
                    self.people_close();
                }
            }
            Some(NamedKey::ArrowLeft) => step(pv, if rtl { 1 } else { -1 }),
            Some(NamedKey::ArrowRight) => step(pv, if rtl { -1 } else { 1 }),
            Some(NamedKey::ArrowUp) => step(pv, -(cols as isize)),
            Some(NamedKey::ArrowDown) => step(pv, cols as isize),
            Some(NamedKey::Home) => pv.focus = 0,
            Some(NamedKey::End) => pv.focus = n.saturating_sub(1),
            Some(NamedKey::Enter) => {
                if pv.person.is_none() {
                    self.people_show();
                }
            }
            Some(NamedKey::F(2)) => self.people_start_naming(),
            Some(NamedKey::Delete) => self.people_not_this(),
            Some(NamedKey::Space) => {
                if let Some(id) = pv.focused_person().map(|c| c.id)
                    && !pv.selected.remove(&id)
                {
                    pv.selected.insert(id);
                }
            }
            _ => match ev_text.filter(|_| plain).map(str::to_lowercase).as_deref() {
                Some("m") => self.people_merge(),
                Some("f") => self.people_faces(),
                _ => return false,
            },
        }
        self.redraw = true;
        true
    }

    /// A click: a card focuses (twice: opens), a header button acts.
    pub(super) fn people_button(&mut self, down: bool, p: (f32, f32)) {
        if !down {
            self.people_release(p);
            return;
        }
        let now = self.now();
        let pv = &mut self.gallery.people;
        if let Some(&(_, c)) = pv.buttons.iter().find(|(r, _)| r.contains(p)) {
            self.people_code(c);
            return;
        }
        let Some(&(_, i)) = pv.hits.iter().find(|(r, _)| r.contains(p)) else {
            return;
        };
        let double = pv.last_click.is_some_and(|(j, t)| j == i && now - t < 0.4);
        if pv.person.is_none() && pv.naming.is_none() {
            pv.drag = Some(Drag {
                from: i,
                start: p,
                at: p,
                moving: false,
            });
        }
        pv.focus = i;
        pv.last_click = Some((i, now));
        if double && pv.person.is_none() {
            self.people_show();
        }
        self.redraw = true;
    }

    /// The pointer moved in the view: a pressed card follows it once it has moved a little.
    pub(super) fn people_pointer_move(&mut self, p: (f32, f32)) {
        let Some(d) = self.gallery.people.drag.as_mut() else {
            return;
        };
        d.at = p;
        if !d.moving && (p.0 - d.start.0).hypot(p.1 - d.start.1) >= DRAG_START {
            d.moving = true;
        }
        self.redraw = true;
    }

    /// The person card under `p` other than the one dragged.
    fn people_drop_target(&self, p: (f32, f32)) -> Option<usize> {
        let pv = &self.gallery.people;
        let from = pv.drag?.from;
        pv.hits
            .iter()
            .find(|(r, i)| *i != from && r.contains(p))
            .map(|&(_, i)| i)
    }

    /// The button let go: a drag dropped on another person asks to merge them.
    fn people_release(&mut self, p: (f32, f32)) {
        let target = self.people_drop_target(p);
        let pv = &mut self.gallery.people;
        let Some(d) = pv.drag.take() else {
            return;
        };
        self.redraw = true;
        let Some(j) = target.filter(|_| d.moving) else {
            return;
        };
        let (Some(a), Some(b)) = (pv.data.people.get(d.from), pv.data.people.get(j)) else {
            return;
        };
        let (from, into) = (a.id, b.id);
        let (title, detail) = merge_question(a, b);
        self.people_ask_merge(vec![from], into, title, detail);
    }

    /// The merge asked about was confirmed (after `M` or a drag).
    pub(in crate::app) fn people_merge_confirmed(&mut self) {
        let Some((from, into)) = self.gallery.people.merge_ask.take() else {
            return;
        };
        self.gallery.people.selected.clear();
        if let Some(i) = self
            .gallery
            .people
            .data
            .people
            .iter()
            .position(|c| c.id == into)
        {
            self.gallery.people.focus = i;
        }
        let n = from.len() as i64 + 1;
        self.people_reload(Some(Change::Merge(from, into)));
        self.gallery.announce = Some(t!("people-merged", count = n));
    }

    fn people_columns(&self) -> usize {
        let ts = self.gallery.text_scale.clamp(1.0, 3.0);
        let cell = CARD * ts.min(1.6) + 24.0;
        ((self.gallery.vw - 32.0) / cell).floor().max(1.0) as usize
    }
}

impl super::Gallery {
    /// Draws the view over the grid. True while still loading.
    pub(super) fn draw_people(&mut self, dl: &mut DrawList, text: &mut Text, f: &Frame) -> bool {
        if !self.people.open {
            return false;
        }
        let pal = f.pal;
        let rtl = f.rtl;
        let (w, h) = (self.vw, self.vh);
        let ts = self.text_scale.clamp(1.0, 3.0);
        let st = |s: TextStyle| TextStyle::new(s.size * ts).weight(s.weight);
        let head = HEADER * ts;
        dl.rect(Rect::new(0.0, 0.0, w, h), 0.0, pal.canvas);
        // Header: title, then the buttons at the trailing end.
        let pv = &self.people;
        let title = match pv
            .person
            .and_then(|p| pv.data.people.iter().find(|c| c.id == p))
        {
            Some(c) => t!("people-faces-of", name = label(c)),
            None => t!("people-title"),
        };
        text_at(
            text,
            dl,
            &title,
            st(TITLE),
            Rect::new(16.0, 0.0, w - 32.0, head),
            pal.text,
            rtl,
        );
        let mut buttons = Vec::new();
        let actions: Vec<(u64, String)> = if pv.person.is_some() {
            vec![
                (code::NOT_THIS, t!("people-not-this")),
                (code::CLOSE, t!("people-back")),
            ]
        } else {
            let mut v = vec![
                (code::SHOW, t!("people-show-photos")),
                (code::NAME, t!("people-name")),
            ];
            if !pv.selected.is_empty() {
                v.push((code::MERGE, t!("people-merge")));
            }
            v.push((code::FACES, t!("people-faces-open")));
            v.push((code::CLOSE, t!("people-done")));
            v
        };
        let mut x = w - 16.0;
        for (c, l) in actions.iter().rev() {
            let bw = text.measure(l, st(BODY)) + 20.0;
            x -= bw;
            let r = Rect::new(
                if rtl { w - x - bw } else { x },
                (head - 28.0 * ts) * 0.5,
                bw,
                28.0 * ts,
            );
            dl.rect(r, 6.0, pal.surface_raised);
            text_in(text, dl, l, st(BODY), r, pal.text);
            buttons.push((r, *c));
            x -= 8.0;
        }
        dl.rect(Rect::new(0.0, head - 1.0, w, 1.0), 0.0, pal.separator);
        // A message instead of cards.
        let msg = match (pv.data.grouping, pv.len()) {
            _ if pv.loading && pv.data.people.is_empty() => Some(t!("people-loading")),
            (None, _) => Some(t!("people-not-installed")),
            (Some(false), 0) => Some(t!("people-grouping-off")),
            (_, 0) if pv.person.is_none() => Some(t!("people-none")),
            _ => None,
        };
        let mut hits = Vec::new();
        let mut wanted = Vec::new();
        if let Some(m) = msg {
            let r = Rect::new(32.0, head + 24.0, w - 64.0, 48.0 * ts);
            text_in(text, dl, &m, st(BODY), r, pal.text_muted);
        } else {
            let side = CARD * ts.min(1.6);
            let cell = side + 24.0;
            let cols = ((w - 32.0) / cell).floor().max(1.0) as usize;
            let cap = st(BODY).size + st(SMALL).size + 20.0;
            let row_h = side + cap + 16.0;
            let n = pv.len();
            // Keep the focused card in view.
            let rows_fit = (((h - head - 16.0) / row_h).floor() as usize).max(1);
            let first_row = (pv.focus / cols).saturating_sub(rows_fit - 1);
            for i in first_row * cols..n {
                let (r, c) = (i / cols - first_row, i % cols);
                let y = head + 16.0 + r as f32 * row_h;
                if y > h {
                    break;
                }
                let cx = 16.0 + c as f32 * cell;
                let x = if rtl { w - cx - cell } else { cx };
                let card = Rect::new(x, y, cell - 8.0, row_h - 8.0);
                let focused = i == pv.focus;
                let (path, rect, line1, line2, sel) = match pv.person {
                    None => {
                        let c = &pv.data.people[i];
                        let cover = c.cover.clone();
                        (
                            cover.as_ref().map(|p| p.0.clone()),
                            cover.map(|p| p.1),
                            label(c),
                            t!("people-face-count", count = c.faces as i64),
                            pv.selected.contains(&c.id),
                        )
                    }
                    Some(_) => {
                        let f = &pv.data.faces.as_ref().expect("len").1[i];
                        let name = f
                            .path
                            .file_name()
                            .map(|n| specular_i18n::isolate(&n.to_string_lossy()))
                            .unwrap_or_default();
                        (
                            Some(f.path.clone()),
                            Some(f.rect),
                            name,
                            String::new(),
                            false,
                        )
                    }
                };
                if focused || sel {
                    dl.rect(
                        card,
                        8.0,
                        if sel {
                            pal.accent.with_alpha(0.25)
                        } else {
                            pal.surface_raised
                        },
                    );
                }
                if focused {
                    dl.stroke_rect(card, 8.0, pal.accent, 2.0);
                }
                let img = Rect::new(x + (cell - 8.0 - side) * 0.5, y + 8.0, side, side);
                let class = specular_thumbs::class_for(side * f.scale / 0.5);
                let shown = path.as_ref().zip(rect).and_then(|(p, fr)| {
                    wanted.push((p.clone(), class));
                    let pl = self.atlas.as_mut()?.best(p, class)?;
                    let (iw, ih) = self
                        .metas
                        .get(p.as_path())
                        .map(|m| (m.width.max(1) as f32, m.height.max(1) as f32))
                        .unwrap_or((pl.uv[2], pl.uv[3]));
                    Some((pl, crop_uv(pl_uv(&pl), fr, iw / ih)))
                });
                match shown {
                    Some((pl, uv)) => dl.image_over(img, uv, pl.tex, side * 0.5, self.backdrop),
                    None => {
                        // Before its thumbnail: the photo's average colour, as the grid does.
                        let c = path
                            .as_ref()
                            .and_then(|p| self.averages.get(p.as_path()))
                            .map_or(pal.surface, |a| super::draw::avg_color(*a));
                        dl.rect(img, side * 0.5, c);
                    }
                }
                let l1 = truncate(text, &line1, st(BODY), card.w - 8.0);
                text_in(
                    text,
                    dl,
                    &l1,
                    st(BODY),
                    Rect::new(card.x, img.y + side + 4.0, card.w, st(BODY).size + 6.0),
                    pal.text,
                );
                if !line2.is_empty() {
                    let r2 = Rect::new(
                        card.x,
                        img.y + side + st(BODY).size + 10.0,
                        card.w,
                        st(SMALL).size + 6.0,
                    );
                    text_in(text, dl, &line2, st(SMALL), r2, pal.text_muted);
                }
                hits.push((card, i));
            }
        }
        // A person being dragged: the card it would merge into ringed, its name at the
        // pointer.
        if let Some(d) = self.people.drag.filter(|d| d.moving)
            && let Some(c) = self.people.data.people.get(d.from)
        {
            if let Some(&(r, _)) = hits.iter().find(|(r, i)| *i != d.from && r.contains(d.at)) {
                dl.stroke_rect(r, 8.0, pal.accent, 3.0);
            }
            let name = specular_i18n::isolate_label(&label(c), dir(rtl));
            let tw = text.measure(&name, st(BODY)) + 20.0;
            let chip_h = 26.0 * ts;
            // After the pointer in the reading direction, kept inside the view.
            let x = if rtl {
                d.at.0 - 12.0 - tw
            } else {
                d.at.0 + 12.0
            };
            let chip = Rect::new(
                x.clamp(0.0, (w - tw).max(0.0)),
                (d.at.1 + 12.0).clamp(0.0, (h - chip_h).max(0.0)),
                tw,
                chip_h,
            );
            dl.shadow(chip, 6.0, pal.shadow, 10.0, 3.0);
            dl.rect(chip, 6.0, pal.surface_raised);
            dl.stroke_rect(chip, 6.0, pal.accent, 1.5);
            text_in(text, dl, &name, st(BODY), chip, pal.text);
        }
        // The name being typed, over the focused card's caption.
        if let Some((_, s)) = &self.people.naming {
            let r = Rect::new(
                w * 0.5 - 160.0 * ts,
                h * 0.5 - 30.0 * ts,
                320.0 * ts,
                60.0 * ts,
            );
            dl.shadow(r, 8.0, pal.shadow, 14.0, 4.0);
            dl.rect(r, 8.0, pal.surface_raised);
            let q = Rect::new(r.x + 12.0, r.y + 6.0, r.w - 24.0, 18.0 * ts);
            text_at(
                text,
                dl,
                &t!("people-name-prompt"),
                st(SMALL),
                q,
                pal.text_muted,
                rtl,
            );
            let line = Rect::new(r.x + 12.0, r.y + 26.0 * ts, r.w - 24.0, 26.0 * ts);
            dl.rect(line, 4.0, pal.surface);
            let shown = format!("{s}▏");
            text_at(
                text,
                dl,
                &shown,
                st(BODY),
                line.inset_xy(6.0, 0.0),
                pal.text,
                rtl,
            );
        }
        let pv = &mut self.people;
        pv.hits = hits;
        pv.buttons = buttons;
        pv.wanted = wanted;
        pv.loading
    }

    /// The view's screen-reader nodes. True when it is open.
    pub(super) fn people_a11y(&self, tree: &mut accesskit::TreeUpdate, scale: f64) -> bool {
        use accesskit::{Action, Live, Node, NodeId, Role};
        let pv = &self.people;
        if !pv.open {
            return false;
        }
        let bounds = |r: Rect| accesskit::Rect {
            x0: f64::from(r.x) * scale,
            y0: f64::from(r.y) * scale,
            x1: f64::from(r.x + r.w) * scale,
            y1: f64::from(r.y + r.h) * scale,
        };
        let mut group = Node::new(Role::Group);
        group.set_label(t!("people-title"));
        group.set_bounds(bounds(Rect::new(0.0, 0.0, self.vw, self.vh)));
        let mut children = Vec::new();
        let mut nodes = Vec::new();
        let status_id = NodeId(A11Y_PEOPLE + A11Y_STATUS);
        let mut status = Node::new(Role::Status);
        status.set_live(Live::Polite);
        status.set_label(t!("people-count", count = pv.data.people.len() as i64));
        children.push(status_id);
        nodes.push((status_id, status));
        for (r, i) in &pv.hits {
            let id = NodeId(A11Y_PEOPLE + A11Y_CARD + *i as u64);
            let mut n = Node::new(Role::Button);
            n.set_bounds(bounds(*r));
            let l = match pv.person {
                None => pv
                    .data
                    .people
                    .get(*i)
                    .map(|c| {
                        format!(
                            "{}, {}",
                            label(c),
                            t!("people-face-count", count = c.faces as i64)
                        )
                    })
                    .unwrap_or_default(),
                Some(_) => t!("people-face-a11y", index = *i as i64 + 1),
            };
            n.set_label(l);
            n.add_action(Action::Click);
            if *i == pv.focus {
                n.set_selected(true);
            }
            children.push(id);
            nodes.push((id, n));
        }
        group.set_children(children);
        let group_id = NodeId(A11Y_PEOPLE);
        if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
            root.push_child(group_id);
        }
        tree.nodes.push((group_id, group));
        tree.nodes.extend(nodes);
        if tree.focus == NodeId(1) && !pv.hits.is_empty() {
            tree.focus = NodeId(A11Y_PEOPLE + A11Y_CARD + pv.focus as u64);
        }
        true
    }
}

/// An atlas placement's normalised image rect (x, y, w, h).
fn pl_uv(pl: &super::atlas::Placed) -> [f32; 4] {
    pl.uv
}

/// The square crop around face `fr` (fractions of the photo, `aspect` = width / height),
/// grown by half, inside the image's `uv` rect.
pub(super) fn crop_uv(uv: [f32; 4], fr: [f32; 4], aspect: f32) -> [f32; 4] {
    let [fx, fy, fw, fh] = fr;
    // Side in photo pixels relative to its height.
    let side_h = (fw * aspect).max(fh) * 1.5;
    let (sw, sh) = ((side_h / aspect).min(1.0), side_h.min(1.0));
    let cx = (fx + fw * 0.5).clamp(sw * 0.5, 1.0 - sw * 0.5);
    let cy = (fy + fh * 0.5).clamp(sh * 0.5, 1.0 - sh * 0.5);
    [
        uv[0] + (cx - sw * 0.5) * uv[2],
        uv[1] + (cy - sh * 0.5) * uv[3],
        sw * uv[2],
        sh * uv[3],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crops_are_square_around_the_face_and_inside_the_photo() {
        // A 3:2 photo, a face in the middle 10 % wide.
        let c = crop_uv([0.0, 0.0, 1.0, 1.0], [0.45, 0.4, 0.1, 0.15], 1.5);
        let (pw, ph) = (c[2] * 1.5, c[3]);
        assert!((pw - ph).abs() < 1e-5, "{c:?}");
        assert!((c[0] + c[2] * 0.5 - 0.5).abs() < 1e-5);
        // At the edge: moved in, not cut.
        let e = crop_uv([0.5, 0.5, 0.25, 0.25], [0.0, 0.0, 0.2, 0.2], 1.0);
        assert_eq!((e[0], e[1]), (0.5, 0.5));
        assert!(e[2] > 0.0 && e[2] <= 0.25);
    }

    #[test]
    fn unnamed_people_get_a_number() {
        let c = Card {
            id: 12,
            name: None,
            faces: 3,
            cover: None,
        };
        assert!(label(&c).contains("12"));
        let named = Card {
            name: Some("Ana".into()),
            ..c
        };
        assert_eq!(label(&named), "Ana");
    }
}
