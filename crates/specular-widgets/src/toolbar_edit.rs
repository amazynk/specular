//! Customising the toolbar: the ordered list in Settings → Viewing (with a preview of the bar
//! when the real one is hidden or covered), which "Customise toolbar…" in the toolbar's
//! right-click menu, the palette and the toolbar's screen-reader action open. Buttons are
//! also dragged along and off the live bar ([`crate::bar_drag`]). Both edit
//! `viewing.toolbar_items` through the store, one undo step per change, and announce each
//! change to screen readers.

use std::sync::Arc;

use specular_gfx::{Icon, Rect, TextBlock, draw_icon};
use specular_i18n::t;
use specular_input::{Key, KeyCombo, NamedKey};
use specular_settings::{Origin, SettingEnum, SettingsStore, ToolbarItem, Value};
use specular_theme::{Color, metrics};

use crate::ctx::{Ctx, HAlign, Target};
use crate::settings::SettingsTarget;
use crate::toolbar::{item_glyph, item_name};
use crate::{Chrome, Response, ViewerState};

/// The setting both editors change.
pub(crate) const KEY: &str = "viewing.toolbar_items";
/// Pointer travel (logical px) before a press on a row or a button becomes a drag.
const DRAG_START: f32 = 4.0;
/// How far around the bar a dragged button still lands on it (beyond: it is removed).
const BAR_SLOP: f32 = 24.0;
/// Height of the preview strip above the list in Settings.
const PREVIEW_H: f32 = 66.0;

// ----- The list ------------------------------------------------------------

/// The toolbar's items as stored.
pub(crate) fn current(store: &SettingsStore) -> Vec<ToolbarItem> {
    match store.get(KEY) {
        Ok(Value::List(l)) => l.iter().filter_map(|s| ToolbarItem::from_name(s)).collect(),
        _ => Vec::new(),
    }
}

/// The buttons that are not on the toolbar, in the schema's order.
pub(crate) fn hidden(list: &[ToolbarItem]) -> Vec<ToolbarItem> {
    ToolbarItem::ALL
        .iter()
        .copied()
        .filter(|it| !it.is_layout() && !list.contains(it))
        .collect()
}

/// Moves the item at `from` to `to` (clamped); returns where it went.
pub(crate) fn move_to(list: &mut Vec<ToolbarItem>, from: usize, to: usize) -> usize {
    if from >= list.len() {
        return from;
    }
    let it = list.remove(from);
    let to = to.min(list.len());
    list.insert(to, it);
    to
}

/// Inserts `item` before position `at` (clamped); returns where it went. A button is on the
/// toolbar at most once, so inserting one that is already there moves it.
pub(crate) fn insert(list: &mut Vec<ToolbarItem>, at: usize, item: ToolbarItem) -> usize {
    let mut at = at.min(list.len());
    if !item.is_layout()
        && let Some(i) = list.iter().position(|x| *x == item)
    {
        list.remove(i);
        if i < at {
            at -= 1;
        }
    }
    list.insert(at, item);
    at
}

/// Shows a hidden button where it is by default: after the nearest button before it in the
/// schema's order that is on the toolbar (as the old chips did), so a custom order is kept.
pub(crate) fn show(list: &mut Vec<ToolbarItem>, item: ToolbarItem) -> usize {
    let all = ToolbarItem::ALL;
    let before = all.iter().position(|o| *o == item).unwrap_or(0);
    let at = all[..before]
        .iter()
        .rev()
        .find_map(|o| list.iter().position(|x| x == o))
        .map_or(0, |i| i + 1);
    insert(list, at, item)
}

/// Saves `list` as the toolbar, as one undo step of the Settings window. True when the
/// setting changed (and so there is a step to undo).
pub(crate) fn commit(store: &SettingsStore, list: &[ToolbarItem]) -> bool {
    let v = list.iter().map(|i| i.as_str().to_owned()).collect();
    store.begin_undo_group();
    let changed = matches!(
        store.set(KEY, Value::List(v), Origin::SettingsWindow),
        Ok(true)
    );
    store.end_undo_group();
    changed
}

/// What a change did, for the screen-reader announcement and to know where focus goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Change {
    Moved(ToolbarItem, usize),
    Added(ToolbarItem, usize),
    Removed(ToolbarItem),
}

impl Change {
    fn text(self, count: usize) -> String {
        let pos = |i: usize| ((i + 1) as i64, count as i64);
        match self {
            Change::Moved(it, i) => {
                let (index, count) = pos(i);
                t!(
                    "toolbar-announce-moved",
                    item = item_name(it),
                    index = index,
                    count = count
                )
            }
            Change::Added(it, i) => {
                let (index, count) = pos(i);
                t!(
                    "toolbar-announce-added",
                    item = item_name(it),
                    index = index,
                    count = count
                )
            }
            Change::Removed(it) => t!("toolbar-announce-removed", item = item_name(it)),
        }
    }
}

/// What is being dragged: an item of the bar (or a shown row in Settings) by position, or a
/// hidden button (a row among the hidden ones in Settings).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Src {
    Bar(usize),
    Hidden(ToolbarItem),
}

/// The list after dropping `src` before position `at` (`None`: off the bar, or on the hidden
/// buttons), and what changed; `None` when nothing does.
pub(crate) fn drop_on(
    list: &[ToolbarItem],
    src: Src,
    at: Option<usize>,
) -> Option<(Vec<ToolbarItem>, Change)> {
    let mut l = list.to_vec();
    match (src, at) {
        (Src::Bar(i), Some(ins)) => {
            let it = *list.get(i)?;
            // Positions count the dragged item, which leaves its place first.
            let to = if ins > i { ins - 1 } else { ins };
            if to == i {
                return None;
            }
            let at = move_to(&mut l, i, to);
            Some((l, Change::Moved(it, at)))
        }
        (Src::Bar(i), None) => {
            let it = *list.get(i)?;
            l.remove(i);
            Some((l, Change::Removed(it)))
        }
        (Src::Hidden(it), Some(ins)) => {
            let at = insert(&mut l, ins, it);
            Some((l, Change::Added(it, at)))
        }
        (Src::Hidden(_), None) => None,
    }
}

/// Where a pointer at `p` drops on the bar: before the first slot whose centre is after it in
/// reading order. `None` when it is off the bar.
pub(crate) fn bar_drop(slots: &[Rect], bar: Rect, p: (f32, f32), rtl: bool) -> Option<usize> {
    if !bar.inset(-BAR_SLOP).contains(p) {
        return None;
    }
    Some(
        slots
            .iter()
            .filter(|r| {
                if rtl {
                    r.center().0 > p.0
                } else {
                    r.center().0 < p.0
                }
            })
            .count(),
    )
}

/// Where a row of the Settings list dragged to height `y` drops: before the first shown row
/// whose middle is below it, or among the hidden buttons (`None`) below their heading.
pub(crate) fn row_drop(rows: &[Rect], shown: usize, hidden_top: f32, y: f32) -> Option<usize> {
    if y >= hidden_top {
        return None;
    }
    Some(
        rows[..shown.min(rows.len())]
            .iter()
            .filter(|r| r.center().1 < y)
            .count(),
    )
}

/// The row `i` of the Settings list as a drag source: shown rows come first, then the hidden
/// buttons.
fn row_src(list: &[ToolbarItem], i: usize) -> Option<Src> {
    if i < list.len() {
        Some(Src::Bar(i))
    } else {
        hidden(list).get(i - list.len()).map(|it| Src::Hidden(*it))
    }
}

/// The row an item is on after a change (it may have moved to the hidden ones).
fn row_of(list: &[ToolbarItem], c: Change) -> Option<usize> {
    match c {
        Change::Moved(_, i) | Change::Added(_, i) => Some(i),
        Change::Removed(it) => hidden(list)
            .iter()
            .position(|h| *h == it)
            .map(|k| list.len() + k),
    }
}

// ----- Interaction state -------------------------------------------------------

/// A press that becomes a drag once the pointer has moved a few pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Drag<S> {
    pub src: S,
    pub start: (f32, f32),
    pub at: (f32, f32),
    pub active: bool,
}

impl<S> Drag<S> {
    fn new(src: S, p: (f32, f32)) -> Self {
        Drag {
            src,
            start: p,
            at: p,
            active: false,
        }
    }

    fn to(&mut self, p: (f32, f32)) {
        self.at = p;
        let (dx, dy) = (p.0 - self.start.0, p.1 - self.start.1);
        self.active |= dx * dx + dy * dy > DRAG_START * DRAG_START;
    }
}

/// The part of a row of the list in Settings that has the keyboard focus: the row itself
/// (Space shows or hides its button), its Move up and Move down buttons, or its menu button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Part {
    #[default]
    Row,
    Up,
    Down,
    Menu,
}

/// An entry of a row's menu (⋯).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RowOp {
    Start,
    End,
    /// Hide the button (a spacer or separator is removed), or show a hidden one.
    Toggle,
}

/// The entries of row `i`'s menu, and whether each can run: a button on the toolbar can move
/// to its start or end (not where it already is) and be hidden; a hidden one can be shown.
pub(crate) fn row_ops(list: &[ToolbarItem], i: usize) -> Vec<(RowOp, bool)> {
    if i < list.len() {
        vec![
            (RowOp::Start, i > 0),
            (RowOp::End, i + 1 < list.len()),
            (RowOp::Toggle, true),
        ]
    } else {
        vec![(RowOp::Toggle, true)]
    }
}

/// Whether row `i` (of a list with `shown` buttons on the toolbar) has its part `p`, and it
/// can be used: Move up and Move down are on the toolbar's rows only, and do nothing at the
/// ends.
pub(crate) fn part_enabled(shown: usize, i: usize, p: Part) -> bool {
    match p {
        Part::Row | Part::Menu => true,
        Part::Up => i < shown && i > 0,
        Part::Down => i + 1 < shown,
    }
}

/// The list in Settings: the focused row (the search field has focus when `None`) and which
/// part of it, the row whose menu is open, the drag under way, and where the rows were last
/// drawn.
#[derive(Default)]
pub(crate) struct ListUi {
    pub focus: Option<usize>,
    pub part: Part,
    /// The row whose menu is open, and the entry the keyboard is on.
    pub menu: Option<usize>,
    pub menu_sel: Option<usize>,
    /// The open menu's button, as last drawn.
    menu_anchor: Option<Rect>,
    pub drag: Option<Drag<usize>>,
    rows: Vec<Rect>,
    shown: usize,
    hidden_top: f32,
    /// The list was laid out on the page last drawn.
    pub on_page: bool,
    /// The real toolbar shows the changes (it is visible beside the dialog), so the list has
    /// no preview of its own.
    pub live: bool,
    /// The list was last drawn right to left.
    rtl: bool,
    /// A row whose menu opens once the list is laid out (screenshots).
    show_menu: Option<usize>,
}

impl ListUi {
    /// Forgets where the rows were (each frame, before the list is drawn again).
    pub(crate) fn clear_rows(&mut self) {
        self.rows.clear();
        self.menu_anchor = None;
        self.on_page = false;
    }

    /// Closes a row's menu; true when one was open.
    pub(crate) fn close_menu(&mut self) -> bool {
        self.menu_sel = None;
        self.menu.take().is_some()
    }
}

/// Whether the Settings list needs its own preview of the toolbar: when the real bar is not
/// on screen (`bar` is `None`: no bar, hidden, or the gallery is showing), or the dialog
/// (`sheet`) covers part of it.
pub(crate) fn preview_needed(bar: Option<Rect>, sheet: Rect) -> bool {
    bar.is_none_or(|b| {
        b.x < sheet.right() && sheet.x < b.right() && b.y < sheet.bottom() && sheet.y < b.bottom()
    })
}

impl Chrome {
    /// Opens the toolbar's right-click menu over the middle of the toolbar (screenshots).
    #[doc(hidden)]
    pub fn show_toolbar_menu(&mut self) {
        // NaN: placed when drawn, as the window may not have its final size yet.
        self.toolbar_menu = Some((f32::NAN, f32::NAN));
    }

    /// Opens the toolbar's More menu, if some buttons do not fit (screenshots).
    #[doc(hidden)]
    pub fn show_overflow_menu(&mut self) {
        self.overflow_menu = Some(Some(0));
    }

    /// Opens the menu of row `i` of the toolbar list in Settings, as the keyboard does
    /// (screenshots; the Settings page with the list must be open).
    #[doc(hidden)]
    pub fn show_toolbar_row_menu(&mut self, i: usize) {
        self.settings.tools.show_menu = Some(i);
    }

    /// Queues `text` for screen readers (a polite live region; it expires after a while).
    pub(crate) fn announce(&mut self, text: String) {
        let n = self.announce.as_ref().map_or(1, |a| a.0 + 1);
        self.announce = Some((n, text, self.last_now));
    }

    /// Saves `list` (one undo step) and announces `c`.
    pub(crate) fn apply_toolbar(&mut self, st: &ViewerState, list: &[ToolbarItem], c: Change) {
        commit(st.store, list);
        self.announce(c.text(list.len()));
    }

    /// A primary press: on a row of the Settings list, it may become a drag.
    pub(crate) fn toolbar_press(&mut self, h: Option<&Target>, p: (f32, f32)) {
        if let Some(Target::Settings(SettingsTarget::ToolRow(i))) = h {
            self.settings.tools.drag = Some(Drag::new(*i, p));
            self.settings.tools.focus = Some(*i);
        }
    }

    /// Follows the pointer with a drag; true while one is under way (the pointer is
    /// captured).
    pub(crate) fn toolbar_drag_move(&mut self, p: (f32, f32)) -> bool {
        if let Some(d) = &mut self.settings.tools.drag {
            d.to(p);
            return d.active;
        }
        false
    }

    /// A release: drops what is being dragged. `None` when nothing was (the release is then
    /// a click).
    pub(crate) fn toolbar_release(&mut self, st: &ViewerState) -> Option<Response> {
        let d = self.settings.tools.drag.take()?;
        if !d.active {
            return None;
        }
        let list = current(st.store);
        let ui = &self.settings.tools;
        let at = row_drop(&ui.rows, ui.shown, ui.hidden_top, d.at.1);
        if let Some(src) = row_src(&list, d.src)
            && let Some((l, c)) = drop_on(&list, src, at)
        {
            self.settings.tools.focus = row_of(&l, c);
            self.apply_toolbar(st, &l, c);
        }
        Some(Response::used())
    }

    /// Esc on the list in Settings: closes a row's menu, else cancels a drag; false when
    /// there was neither.
    pub(crate) fn toolbar_back(&mut self) -> bool {
        if self.settings.tools.close_menu() {
            return true;
        }
        if let Some(d) = &mut self.settings.tools.drag
            && d.active
        {
            self.settings.tools.drag = None;
            return true;
        }
        false
    }

    /// "Customise toolbar…" (the toolbar's right-click menu, the palette, the toolbar's
    /// screen-reader action): Settings, at the toolbar's list.
    pub(crate) fn customise_toolbar(&mut self) {
        self.toolbar_menu = None;
        self.open_settings_at(KEY);
    }

    /// Moves item `i` of the bar by `d` places (a keyboard or screen-reader request).
    pub(crate) fn toolbar_nudge(&mut self, st: &ViewerState, i: usize, d: isize) -> Option<usize> {
        let to = i.checked_add_signed(d)?;
        self.toolbar_move(st, i, to)
    }

    /// Moves item `i` of the bar to position `to` (one undo step, announced); `None` when
    /// either is not on the bar, or they are the same.
    pub(crate) fn toolbar_move(&mut self, st: &ViewerState, i: usize, to: usize) -> Option<usize> {
        let list = current(st.store);
        let it = *list.get(i)?;
        if to >= list.len() || to == i {
            return None;
        }
        let mut l = list;
        move_to(&mut l, i, to);
        self.apply_toolbar(st, &l, Change::Moved(it, to));
        Some(to)
    }

    /// Keys for the list in Settings. Tab moves between the search field and the list. In the
    /// list ↑/↓ move the focus from row to row and ←/→ (in reading order) between the parts
    /// of a row: the row, Move up, Move down and its menu (⋯). Alt+↑/↓ move the focused
    /// button, Alt+Home/End to the start or end of the toolbar; Space or Enter shows or hides
    /// it, or presses the focused part; Delete takes it off; Shift+F10 opens the row's menu.
    /// Returns false for keys it leaves alone.
    pub(crate) fn tools_key(&mut self, combo: &KeyCombo, st: &ViewerState) -> bool {
        let m = combo.mods;
        if m.ctrl || m.meta {
            return false;
        }
        let Key::Named(k) = combo.key else {
            return false;
        };
        if self.settings.tools.menu.is_some() {
            return self.row_menu_key(k, m.any(), st);
        }
        let ui = &mut self.settings.tools;
        let Some(f) = ui.focus else {
            if k == NamedKey::Tab && !ui.rows.is_empty() {
                ui.focus = Some(0);
                ui.part = Part::Row;
                return true;
            }
            return false;
        };
        let list = current(st.store);
        let shown = list.len();
        let n = shown + hidden(&list).len();
        let f = f.min(n.saturating_sub(1));
        let part = ui.part;
        let step: isize = match k {
            NamedKey::ArrowUp => -1,
            NamedKey::ArrowDown => 1,
            _ => 0,
        };
        // ←/→ in reading order: on towards the row's end, or back.
        let along: isize = match (k, ui.rtl) {
            (NamedKey::ArrowRight, false) | (NamedKey::ArrowLeft, true) => 1,
            (NamedKey::ArrowLeft, false) | (NamedKey::ArrowRight, true) => -1,
            _ => 0,
        };
        match k {
            NamedKey::Tab => {
                self.settings.tools.focus = None;
                self.settings.tools.part = Part::Row;
            }
            _ if step != 0 && m.alt => {
                if f < shown {
                    let to = self.toolbar_nudge(st, f, step).unwrap_or(f);
                    self.focus_row(to, part, shown);
                }
            }
            NamedKey::Home | NamedKey::End if m.alt && !m.shift => {
                if f < shown {
                    let to = if k == NamedKey::Home { 0 } else { shown - 1 };
                    let to = self.toolbar_move(st, f, to).unwrap_or(f);
                    self.focus_row(to, part, shown);
                }
            }
            _ if step != 0 && !m.shift => {
                let to = (f as isize + step).clamp(0, n as isize - 1) as usize;
                self.focus_row(to, part, shown);
            }
            _ if along != 0 && !m.any() => {
                const ORDER: [Part; 4] = [Part::Row, Part::Up, Part::Down, Part::Menu];
                let at = ORDER.iter().position(|p| *p == part).unwrap_or(0) as isize;
                let next = (1..4)
                    .map(|d| at + d * along)
                    .take_while(|i| (0..4).contains(i))
                    .map(|i| ORDER[i as usize])
                    .find(|p| part_enabled(shown, f, *p));
                if let Some(p) = next {
                    self.settings.tools.part = p;
                }
            }
            NamedKey::Home if !m.any() => self.focus_row(0, Part::Row, shown),
            NamedKey::End if !m.any() => self.focus_row(n - 1, Part::Row, shown),
            NamedKey::F(10) if m.shift && !m.alt => self.open_row_menu(f, st, true),
            NamedKey::Enter | NamedKey::Space if !m.any() => match part {
                Part::Row => self.toggle_row(st, f),
                Part::Up | Part::Down => {
                    let d = if part == Part::Up { -1 } else { 1 };
                    let to = self.toolbar_nudge(st, f, d).unwrap_or(f);
                    self.focus_row(to, part, shown);
                }
                Part::Menu => self.open_row_menu(f, st, true),
            },
            NamedKey::Delete | NamedKey::Backspace if !m.any() => {
                if f < shown {
                    self.toggle_row(st, f);
                }
            }
            _ => return false,
        }
        true
    }

    /// Puts the keyboard on part `part` of row `i` (in a list with `shown` buttons on the
    /// toolbar): when that part can't be used there (Move up on the first row, or on a hidden
    /// one), on the other arrow, else on the row.
    fn focus_row(&mut self, i: usize, part: Part, shown: usize) {
        let other = match part {
            Part::Up => Part::Down,
            Part::Down => Part::Up,
            p => p,
        };
        let ui = &mut self.settings.tools;
        ui.focus = Some(i);
        ui.part = [part, other]
            .into_iter()
            .find(|p| part_enabled(shown, i, *p))
            .unwrap_or(Part::Row);
    }

    /// Opens the menu of row `i`; from the keyboard, on its first entry that can run.
    pub(crate) fn open_row_menu(&mut self, i: usize, st: &ViewerState, keys: bool) {
        let ops = row_ops(&current(st.store), i);
        let ui = &mut self.settings.tools;
        ui.focus = Some(i);
        ui.part = Part::Menu;
        ui.menu = Some(i);
        ui.menu_sel = if keys {
            ops.iter().position(|(_, on)| *on)
        } else {
            None
        };
    }

    /// Keys while a row's menu is open: ↑/↓, Home and End move between its entries that can
    /// run, Enter or Space runs one, Tab closes it (as Esc does); other keys do nothing.
    fn row_menu_key(&mut self, k: NamedKey, mods: bool, st: &ViewerState) -> bool {
        let Some(row) = self.settings.tools.menu else {
            return false;
        };
        let ops = row_ops(&current(st.store), row);
        let on: Vec<usize> = (0..ops.len()).filter(|i| ops[*i].1).collect();
        let ui = &mut self.settings.tools;
        let at = ui.menu_sel.and_then(|s| on.iter().position(|e| *e == s));
        let len = on.len();
        if len == 0 {
            return true;
        }
        match k {
            _ if mods => {}
            NamedKey::ArrowDown => {
                ui.menu_sel = Some(on[at.map_or(0, |p| (p + 1) % len)]);
            }
            NamedKey::ArrowUp => {
                ui.menu_sel = Some(on[at.map_or(len - 1, |p| (p + len - 1) % len)]);
            }
            NamedKey::Home => ui.menu_sel = Some(on[0]),
            NamedKey::End => ui.menu_sel = Some(on[len - 1]),
            NamedKey::Tab => {
                ui.close_menu();
            }
            NamedKey::Enter | NamedKey::Space => {
                if let Some(p) = at {
                    self.row_op(row, ops[on[p]].0, st);
                }
            }
            _ => {}
        }
        true
    }

    /// Runs an entry of row `i`'s menu and closes it; the keyboard stays on the row's menu
    /// button, wherever the row went.
    pub(crate) fn row_op(&mut self, i: usize, op: RowOp, st: &ViewerState) {
        let list = current(st.store);
        let shown = list.len();
        // An entry that can't run here (Move to start on the first row) leaves the menu open.
        if !row_ops(&list, i).contains(&(op, true)) {
            return;
        }
        self.settings.tools.close_menu();
        match op {
            RowOp::Start | RowOp::End => {
                let to = if op == RowOp::Start { 0 } else { shown - 1 };
                let to = self.toolbar_move(st, i, to).unwrap_or(i);
                self.focus_row(to, Part::Menu, shown);
            }
            RowOp::Toggle => {
                self.toggle_row(st, i);
                self.settings.tools.part = Part::Menu;
            }
        }
    }

    /// Shows or hides the button of row `i` (a spacer or separator is removed); the focus
    /// follows it.
    fn toggle_row(&mut self, st: &ViewerState, i: usize) {
        let mut l = current(st.store);
        let c = if i < l.len() {
            let it = l.remove(i);
            Change::Removed(it)
        } else {
            let Some(it) = hidden(&l).get(i - l.len()).copied() else {
                return;
            };
            Change::Added(it, show(&mut l, it))
        };
        let f = row_of(&l, c).unwrap_or(i.min(l.len()));
        let part = self.settings.tools.part;
        self.focus_row(f, part, l.len());
        self.apply_toolbar(st, &l, c);
    }

    /// Clicks on the list in Settings.
    pub(crate) fn tools_click(&mut self, s: SettingsTarget, st: &ViewerState) {
        let shown = || current(st.store).len();
        match s {
            SettingsTarget::ToolRow(i) => {
                self.settings.tools.focus = Some(i);
                self.settings.tools.part = Part::Row;
            }
            SettingsTarget::ToolShow(i) | SettingsTarget::ToolRemove(i) => {
                self.settings.tools.part = Part::Row;
                self.toggle_row(st, i);
            }
            SettingsTarget::ToolUp(i) | SettingsTarget::ToolDown(i) => {
                let (d, part) = if matches!(s, SettingsTarget::ToolUp(_)) {
                    (-1, Part::Up)
                } else {
                    (1, Part::Down)
                };
                let to = self.toolbar_nudge(st, i, d).unwrap_or(i);
                self.focus_row(to, part, shown());
            }
            SettingsTarget::ToolMenu(i) => {
                if self.settings.tools.menu == Some(i) {
                    self.settings.tools.close_menu();
                } else {
                    self.open_row_menu(i, st, false);
                }
            }
            SettingsTarget::ToolMenuItem(i, op) => self.row_op(i, op, st),
            SettingsTarget::ToolAdd(it) => {
                let mut l = current(st.store);
                let at = insert(&mut l, usize::MAX, it);
                self.settings.tools.focus = Some(at);
                self.settings.tools.part = Part::Row;
                self.apply_toolbar(st, &l, Change::Added(it, at));
            }
            SettingsTarget::ToolReset => {
                st.store.begin_undo_group();
                let _ = st.store.reset(KEY, Origin::SettingsWindow);
                st.store.end_undo_group();
                self.announce(t!("toolbar-announce-reset"));
            }
            _ => {}
        }
    }

    /// Screen-reader custom actions: on a shown row of the Settings list 1 moves it up, 2
    /// down, 3 to the start and 4 to the end; on the toolbar, 10 opens that list.
    pub(crate) fn toolbar_custom_action(&mut self, t: Option<Target>, id: i32, st: &ViewerState) {
        match (t, id) {
            (None, 10) => self.customise_toolbar(),
            (Some(Target::Settings(SettingsTarget::ToolRow(i))), 1 | 2) => {
                if let Some(to) = self.toolbar_nudge(st, i, if id == 1 { -1 } else { 1 }) {
                    self.settings.tools.focus = Some(to);
                    self.settings.tools.part = Part::Row;
                }
            }
            (Some(Target::Settings(SettingsTarget::ToolRow(i))), 3 | 4) => {
                let op = if id == 3 { RowOp::Start } else { RowOp::End };
                self.row_op(i, op, st);
                self.settings.tools.part = Part::Row;
            }
            _ => {}
        }
    }

    /// The target that has the keyboard focus in the toolbar list in Settings, for the
    /// accessibility tree: in a row's open menu, its entry the keyboard is on (else its first
    /// that can run).
    pub(crate) fn toolbar_focus(&self, st: &ViewerState) -> Option<Target> {
        let ui = &self.settings.tools;
        if let Some(row) = ui.menu {
            let ops = row_ops(&current(st.store), row);
            let k = ui.menu_sel.or_else(|| ops.iter().position(|(_, on)| *on))?;
            let op = ops.get(k)?.0;
            return Some(Target::Settings(SettingsTarget::ToolMenuItem(row, op)));
        }
        let f = ui.focus?;
        Some(Target::Settings(match ui.part {
            Part::Row => SettingsTarget::ToolRow(f),
            Part::Up => SettingsTarget::ToolUp(f),
            Part::Down => SettingsTarget::ToolDown(f),
            Part::Menu => SettingsTarget::ToolMenu(f),
        }))
    }

    /// Moves the keyboard focus of the toolbar list to `t` (an assistive technology's Focus
    /// request).
    pub(crate) fn toolbar_focus_on(&mut self, t: &Target, st: &ViewerState) {
        if let Target::Settings(s) = t {
            let ui = &mut self.settings.tools;
            let (i, part) = match s {
                SettingsTarget::ToolRow(i) => (*i, Part::Row),
                SettingsTarget::ToolUp(i) => (*i, Part::Up),
                SettingsTarget::ToolDown(i) => (*i, Part::Down),
                SettingsTarget::ToolMenu(i) => (*i, Part::Menu),
                SettingsTarget::ToolMenuItem(i, op) => {
                    if ui.menu == Some(*i) {
                        let ops = row_ops(&current(st.store), *i);
                        ui.menu_sel = ops.iter().position(|(o, _)| o == op);
                    }
                    return;
                }
                _ => return,
            };
            ui.focus = Some(i);
            ui.part = part;
        }
    }
}

// ----- Drawing ---------------------------------------------------------------

/// An item's glyph in `r`: its icon, or "%" for the zoom readout.
pub(crate) fn glyph(cx: &mut Ctx, it: ToolbarItem, r: Rect, color: Color) {
    match item_glyph(it, cx.rtl) {
        Some((icon, flip)) => {
            let s = r.w.min(r.h);
            draw_icon(cx.dl, icon, r.centered(s, s), color, 1.5, flip);
        }
        None => {
            let style = cx.style(metrics::FONT_SMALL).weight(650.0);
            let b = cx.layout("%", style, None);
            cx.block_in(&b, r, HAlign::Center, color);
        }
    }
}

/// A keyboard focus ring around `r`.
fn focus_ring(cx: &mut Ctx, r: Rect, radius: f32) {
    cx.dl
        .rect_border(r, radius, Color::TRANSPARENT, cx.pal.focus_ring, 2.0);
}

/// The toolbar's right-click menu at `p`.
pub(crate) fn context_menu(cx: &mut Ctx, p: (f32, f32)) {
    let style = cx.style(metrics::FONT_BODY);
    let label = t!("toolbar-customise");
    let b = cx.layout(&label, style, None);
    let rh = cx.u(30.0);
    let w = (b.width + 32.0).min(cx.w - 16.0);
    let h = rh + 8.0;
    let x = if cx.rtl { p.0 - w } else { p.0 };
    let x = x.clamp(8.0, (cx.w - w - 8.0).max(8.0));
    // Above the pointer: the toolbar is at the bottom of the window.
    let y = (p.1 - h - 4.0).clamp(8.0, (cx.h - h - 8.0).max(8.0));
    let r = Rect::new(x.round(), y.round(), w, h);
    cx.dl
        .shadow(r, metrics::RADIUS_MD, cx.pal.shadow, 18.0, 6.0);
    cx.dl.glass(
        r,
        metrics::RADIUS_MD,
        cx.pal.glass,
        cx.pal.glass_border,
        1.0,
    );
    cx.hit(r, Target::ToolbarBg);
    let entry = Rect::new(r.x + 4.0, r.y + 4.0, r.w - 8.0, rh);
    let t = Target::ToolbarMenu;
    if cx.is_hover(&t) {
        cx.dl.rect(entry, metrics::RADIUS_SM, cx.pal.surface_hover);
    }
    cx.label_fit(
        &label,
        style,
        entry.inset_xy(12.0, 0.0),
        HAlign::Start,
        cx.pal.text,
        false,
    );
    cx.hit(entry, t);
    cx.popovers.push(r);
}

// ----- The list in Settings -------------------------------------------------

/// The editor's text buttons (add a spacer, add a separator, reset), wrapping within
/// `width`: rectangle relative to their top-left corner, label, target. Returns them and
/// their height.
fn actions(cx: &mut Ctx, width: f32) -> (Vec<(Rect, Arc<TextBlock>, SettingsTarget)>, f32) {
    let style = cx.style(metrics::FONT_LABEL).weight(550.0);
    let (h, gap) = (cx.u(28.0), 8.0);
    let (mut x, mut y) = (0.0f32, 0.0f32);
    let mut out = Vec::new();
    for (label, tgt) in [
        (
            t!("settings-toolbar-add-spacer"),
            SettingsTarget::ToolAdd(ToolbarItem::Spacer),
        ),
        (
            t!("settings-toolbar-add-separator"),
            SettingsTarget::ToolAdd(ToolbarItem::Separator),
        ),
        (t!("settings-toolbar-reset"), SettingsTarget::ToolReset),
    ] {
        let label = cx.ellipsize(&label, style, width - 24.0, false);
        let b = cx.layout(&label, style, None);
        let w = (b.width + 24.0).min(width);
        if x > 0.0 && x + w > width {
            x = 0.0;
            y += h + gap;
        }
        out.push((Rect::new(x, y, w, h), b, tgt));
        x += w + gap;
    }
    (out, y + h)
}

/// Height of the editor for a row `width` wide: the preview (unless the real toolbar shows
/// the changes, `live`), the list (buttons on the toolbar, then the hidden ones under a
/// heading), the buttons and the hint.
pub(crate) fn editor_height(cx: &mut Ctx, st: &ViewerState, width: f32, live: bool) -> f32 {
    let list = current(st.store);
    let n = list.len() + hidden(&list).len();
    let (_, ah) = actions(cx, width);
    let hint = cx.layout(
        &t!("settings-toolbar-hint"),
        cx.style(metrics::FONT_SMALL),
        Some(width),
    );
    let preview = if live { 0.0 } else { PREVIEW_H + 10.0 };
    preview + n as f32 * cx.u(34.0) + cx.u(28.0) + 2.0 + 10.0 + ah + 8.0 + hint.height
}

/// Draws the editor at `r` (its full width, from its top).
pub(crate) fn draw_editor(ch: &mut Chrome, cx: &mut Ctx, st: &ViewerState, r: Rect) {
    let list = current(st.store);
    let hid = hidden(&list);
    let n = list.len() + hid.len();
    let (rh, head) = (cx.u(34.0), cx.u(28.0));
    let live = ch.settings.tools.live;
    let pv = Rect::new(r.x, r.y, r.w, if live { 0.0 } else { PREVIEW_H });
    let top = if live { r.y } else { pv.bottom() + 10.0 };
    let lb = Rect::new(r.x, top, r.w, n as f32 * rh + head + 2.0);
    let hidden_top = lb.y + 1.0 + list.len() as f32 * rh;
    let mut rows = Vec::with_capacity(n);
    for i in 0..n {
        let y = lb.y + 1.0 + i as f32 * rh + if i >= list.len() { head } else { 0.0 };
        rows.push(Rect::new(lb.x + 1.0, y, lb.w - 2.0, rh));
    }
    let ui = &mut ch.settings.tools;
    ui.shown = list.len();
    ui.hidden_top = hidden_top;
    if let Some(i) = ui.show_menu.take().filter(|i| *i < n) {
        ui.focus = Some(i);
        ui.part = Part::Menu;
        ui.menu = Some(i);
        // Move to start (the first row's first entry that can run is Move to end).
        ui.menu_sel = Some(usize::from(i == 0));
    }
    ui.rtl = cx.rtl;
    let drag = ui.drag.filter(|d| d.active);
    // Where the dragged row would land, and the toolbar it would make (in the preview).
    let drop = drag.map(|d| row_drop(&rows, list.len(), hidden_top, d.at.1));
    let prospective = drag
        .zip(drop)
        .and_then(|(d, at)| drop_on(&list, row_src(&list, d.src)?, at));
    ui.focus = ui.focus.map(|f| f.min(n.saturating_sub(1)));
    let focus = ui.focus;
    let part = ui.part;
    let menu = ui.menu;

    if !live {
        cx.dl.rect(pv, metrics::RADIUS_MD, cx.pal.sidebar);
        let shown_items = prospective.as_ref().map_or(&list, |p| &p.0);
        crate::toolbar::preview(cx, st, ch.info_open, shown_items, pv.inset(10.0));
    }

    cx.dl.rect_border(
        lb,
        metrics::RADIUS_MD,
        cx.pal.surface_raised,
        cx.pal.separator,
        1.0,
    );
    let heading = Rect::new(lb.x + 1.0, hidden_top, lb.w - 2.0, head);
    if matches!(drop, Some(None)) {
        // Dropping here hides the button.
        let zone = Rect::new(
            heading.x,
            heading.y,
            heading.w,
            lb.bottom() - 1.0 - heading.y,
        );
        cx.dl
            .rect(zone, metrics::RADIUS_MD - 1.0, cx.pal.accent_soft);
    } else {
        cx.dl.rect(heading, 0.0, cx.pal.sidebar.with_alpha(0.6));
    }
    let hs = cx.style(metrics::FONT_SMALL).weight(600.0);
    let hr = cx.m_in(heading.inset_xy(12.0, 0.0), lb);
    cx.label_fit(
        &t!("settings-toolbar-hidden"),
        hs,
        hr,
        HAlign::Start,
        cx.pal.text_muted,
        false,
    );
    for (i, rr) in rows.iter().enumerate() {
        let item = if i < list.len() {
            list[i]
        } else {
            hid[i - list.len()]
        };
        let source = drag.is_some_and(|d| d.src == i);
        if source {
            cx.dl.push_opacity(0.3);
        }
        let row = Row {
            i,
            item,
            shown: list.len(),
            focus: (focus == Some(i)).then_some(part),
            menu_open: menu == Some(i),
            hover: drag.is_none(),
        };
        let menu_r = tool_row(cx, *rr, row);
        if menu == Some(i) {
            ch.settings.tools.menu_anchor = Some(menu_r);
        }
        if source {
            cx.dl.pop_opacity();
        }
        if i + 1 < n && i + 1 != list.len() {
            let line = Rect::new(rr.x + 12.0, rr.bottom() - 0.5, rr.w - 24.0, 1.0);
            cx.dl.rect(line, 0.0, cx.pal.separator.with_alpha(0.6));
        }
    }
    // The insertion line, and the dragged row following the pointer.
    if let (Some(d), Some(drop)) = (drag, drop) {
        if let Some(ins) = drop {
            let y = match (rows[..list.len()].get(ins), list.len()) {
                (Some(r), _) => r.y,
                (None, 0) => lb.y + 1.0,
                (None, s) => rows[s - 1].bottom(),
            };
            let line = Rect::new(lb.x + 6.0, y - 1.0, lb.w - 12.0, 2.0);
            cx.dl.rect(line, 1.0, cx.pal.accent);
        }
        let item = if d.src < list.len() {
            list.get(d.src).copied()
        } else {
            hid.get(d.src - list.len()).copied()
        };
        if let Some(item) = item {
            let y = (d.at.1 - rh * 0.5).clamp(lb.y, lb.bottom() - rh);
            let fr = Rect::new(lb.x + 1.0, y, lb.w - 2.0, rh);
            cx.dl
                .shadow(fr, metrics::RADIUS_SM, cx.pal.shadow, 14.0, 4.0);
            cx.dl.rect_border(
                fr,
                metrics::RADIUS_SM,
                cx.pal.surface_raised,
                cx.pal.accent,
                1.0,
            );
            let end = cx.u(34.0) + 20.0;
            row_content(cx, fr, item, d.src < list.len(), end);
        }
    }
    ch.settings.tools.rows = rows;

    let ay = lb.bottom() + 10.0;
    let (acts, ah) = actions(cx, r.w);
    for (ar, b, tgt) in acts {
        let br = cx.m_in(Rect::new(r.x + ar.x, ay + ar.y, ar.w, ar.h), r);
        let tgt = Target::Settings(tgt);
        let fill = if cx.is_hover(&tgt) {
            cx.pal.surface_pressed
        } else {
            cx.pal.surface_raised
        };
        cx.dl
            .rect_border(br, metrics::RADIUS_SM + 1.0, fill, cx.pal.separator, 1.0);
        cx.block_in(&b, br, HAlign::Center, cx.pal.text);
        cx.hit(br, tgt);
    }
    let hint = cx.layout(
        &t!("settings-toolbar-hint"),
        cx.style(metrics::FONT_SMALL),
        Some(r.w),
    );
    let hr = cx.m_in(Rect::new(r.x, ay + ah + 8.0, r.w, hint.height), r);
    cx.block_top(&hint, hr, HAlign::Start, cx.pal.text_muted);
}

/// A row's grip, glyph and name; `end` is the width its controls take at the end.
fn row_content(cx: &mut Ctx, rr: Rect, item: ToolbarItem, shown: bool, end: f32) -> Rect {
    let grip = cx.m_in(Rect::new(rr.x + 6.0, rr.y, 18.0, rr.h), rr);
    draw_icon(
        cx.dl,
        Icon::Grip,
        grip.centered(16.0, 16.0),
        cx.pal.text_faint,
        1.5,
        false,
    );
    let icon = cx.u(18.0);
    let gr = cx.m_in(Rect::new(rr.x + 30.0, rr.y, icon, rr.h), rr);
    let c = if shown {
        cx.pal.text
    } else {
        cx.pal.text_muted
    };
    glyph(cx, item, gr.centered(icon, icon), c);
    let x = 30.0 + icon + 12.0;
    let nr = cx.m_in(Rect::new(rr.x + x, rr.y, rr.w - x - end, rr.h), rr);
    let style = cx.style(metrics::FONT_BODY);
    cx.label_fit(&item_name(item), style, nr, HAlign::Start, c, false);
    nr
}

/// What [`tool_row`] draws.
struct Row {
    i: usize,
    item: ToolbarItem,
    /// How many buttons are on the toolbar (rows `0..shown`).
    shown: usize,
    /// The part of the row with the keyboard focus, if it has it.
    focus: Option<Part>,
    menu_open: bool,
    /// Hover effects (off while a row is dragged).
    hover: bool,
}

/// One row of the list: on the toolbar (Move up, Move down, its menu, and a switch, or a
/// remove button for a spacer or separator) or hidden (its menu and the switch off). Returns
/// where its menu button is.
fn tool_row(cx: &mut Ctx, rr: Rect, row: Row) -> Rect {
    let Row {
        i,
        item,
        shown: n_shown,
        focus,
        menu_open,
        hover,
    } = row;
    let shown = i < n_shown;
    let t = Target::Settings(SettingsTarget::ToolRow(i));
    if hover && cx.is_hover(&t) {
        cx.dl.rect(rr, metrics::RADIUS_SM, cx.pal.surface_hover);
    }
    // The controls, from the end of the row: the switch (or remove), the menu, then the
    // arrows. Laid out left to right, then mirrored.
    let s = cx.u(28.0);
    let cy = rr.center().1 - s * 0.5;
    let (sw_w, sw_h) = (cx.u(34.0), cx.u(20.0));
    let ctl_x = rr.right() - 12.0 - sw_w;
    let menu_r = Rect::new(ctl_x - 8.0 - s, cy, s, s);
    let down_r = Rect::new(menu_r.x - 4.0 - s, cy, s, s);
    let up_r = Rect::new(down_r.x - s, cy, s, s);
    let first = if shown { up_r.x } else { menu_r.x };
    row_content(cx, rr, item, shown, rr.right() - first + 8.0);
    cx.hit(rr, t);

    let ring = |cx: &mut Ctx, r: Rect| focus_ring(cx, r.inset(-1.0), metrics::RADIUS_SM + 3.0);
    if shown {
        for (r, p, icon, tgt) in [
            (up_r, Part::Up, Icon::ChevronUp, SettingsTarget::ToolUp(i)),
            (
                down_r,
                Part::Down,
                Icon::ChevronDown,
                SettingsTarget::ToolDown(i),
            ),
        ] {
            let r = cx.m_in(r, rr);
            let on = part_enabled(n_shown, i, p);
            cx.icon_button_e(
                r,
                icon,
                Target::Settings(tgt),
                cx.pal.text_muted,
                false,
                false,
                on,
            );
            if focus == Some(p) {
                ring(cx, r);
            }
        }
    }
    let menu_r = cx.m_in(menu_r, rr);
    cx.icon_button(
        menu_r,
        Icon::More,
        Target::Settings(SettingsTarget::ToolMenu(i)),
        cx.pal.text_muted,
        false,
        menu_open,
    );
    if focus == Some(Part::Menu) {
        ring(cx, menu_r);
    }

    if shown && item.is_layout() {
        let br = cx.m_in(
            Rect::new(rr.right() - 10.0 - s, rr.center().1 - s * 0.5, s, s),
            rr,
        );
        cx.icon_button(
            br,
            Icon::Close,
            Target::Settings(SettingsTarget::ToolRemove(i)),
            cx.pal.text_muted,
            false,
            false,
        );
    } else {
        let sw = Rect::new(ctl_x, rr.center().1 - sw_h * 0.5, sw_w, sw_h);
        let sw = cx.m_in(sw, rr);
        let tgt = Target::Settings(SettingsTarget::ToolShow(i));
        let track = if shown {
            cx.pal.accent_fill
        } else {
            cx.pal.surface_pressed
        };
        let rad = sw_h * 0.5;
        cx.dl.rect(sw, rad, track);
        if cx.is_hover(&tgt) {
            cx.dl.rect(sw, rad, cx.pal.surface_hover);
        }
        let knob_x = if shown != cx.rtl {
            sw.right() - rad
        } else {
            sw.x + rad
        };
        cx.dl
            .circle((knob_x, sw.center().1), rad - 3.0, Color::WHITE);
        cx.hit(sw.inset(-4.0), tgt);
    }
    if focus == Some(Part::Row) {
        focus_ring(cx, rr.inset(2.0), metrics::RADIUS_SM);
    }
    menu_r
}

/// The label of an entry of a row's menu (`shown`: the row's button is on the toolbar).
pub(crate) fn op_label(op: RowOp, item: ToolbarItem, shown: bool) -> String {
    match op {
        RowOp::Start => t!("settings-toolbar-move-start"),
        RowOp::End => t!("settings-toolbar-move-end"),
        RowOp::Toggle if !shown => t!("settings-toolbar-menu-show"),
        RowOp::Toggle if item.is_layout() => t!("settings-toolbar-menu-remove"),
        RowOp::Toggle => t!("settings-toolbar-menu-hide"),
    }
}

/// The item of row `i` of the list, and whether it is on the toolbar.
pub(crate) fn row_item(list: &[ToolbarItem], i: usize) -> Option<(ToolbarItem, bool)> {
    match list.get(i) {
        Some(it) => Some((*it, true)),
        None => hidden(list).get(i - list.len()).map(|it| (*it, false)),
    }
}

/// A row's open menu, under its ⋯ button (above it when there is no room below), over
/// everything else in the window: a click anywhere else closes it.
pub(crate) fn draw_row_menu(ch: &mut Chrome, cx: &mut Ctx, st: &ViewerState) {
    let ui = &mut ch.settings.tools;
    let Some(row) = ui.menu else {
        return;
    };
    // Its row is not on screen (scrolled away, another page): the menu goes with it.
    let Some(anchor) = ui.menu_anchor else {
        ui.close_menu();
        return;
    };
    let sel = ui.menu_sel;
    let list = current(st.store);
    let Some((item, shown)) = row_item(&list, row) else {
        ui.close_menu();
        return;
    };
    let ops = row_ops(&list, row);
    let style = cx.style(metrics::FONT_BODY);
    let blocks: Vec<_> = ops
        .iter()
        .map(|(op, _)| cx.layout(&op_label(*op, item, shown), style, None))
        .collect();
    let rh = cx.u(30.0);
    let widest = blocks.iter().map(|b| b.width).fold(0.0f32, f32::max);
    let w = (widest + 32.0).max(cx.u(140.0)).min(cx.w - 16.0);
    let h = rh * ops.len() as f32 + 8.0;
    cx.hit(
        Rect::new(0.0, 0.0, cx.w, cx.h),
        Target::Settings(SettingsTarget::ToolMenuBg),
    );
    // Its end edge lines up with the button's.
    let x = if cx.rtl { anchor.x } else { anchor.right() - w };
    let x = x.clamp(8.0, (cx.w - w - 8.0).max(8.0));
    let below = anchor.bottom() + 4.0;
    let y = if below + h <= cx.h - 8.0 {
        below
    } else {
        (anchor.y - 4.0 - h).max(8.0)
    };
    let r = Rect::new(x.round(), y.round(), w, h);
    cx.dl
        .shadow(r, metrics::RADIUS_MD, cx.pal.shadow, 18.0, 6.0);
    cx.dl.glass(
        r,
        metrics::RADIUS_MD,
        cx.pal.glass,
        cx.pal.glass_border,
        1.0,
    );
    cx.hit(r, Target::Settings(SettingsTarget::ToolMenuBg));
    for (k, ((op, on), b)) in ops.iter().zip(&blocks).enumerate() {
        let e = Rect::new(r.x + 4.0, r.y + 4.0 + k as f32 * rh, r.w - 8.0, rh);
        let t = Target::Settings(SettingsTarget::ToolMenuItem(row, *op));
        if *on && (cx.is_hover(&t) || sel == Some(k)) {
            cx.dl.rect(e, metrics::RADIUS_SM, cx.pal.surface_hover);
        }
        if *on && sel == Some(k) {
            focus_ring(cx, e, metrics::RADIUS_SM);
        }
        let c = if *on { cx.pal.text } else { cx.pal.text_faint };
        let lr = cx.m_in(e.inset_xy(12.0, 0.0), e);
        cx.block_in(b, lr, HAlign::Start, c);
        cx.hit(e, t);
    }
    cx.popovers.push(r);
}
