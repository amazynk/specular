//! The filter bar's keyboard focus order, its accessibility nodes and the announcement of how
//! many items a changed filter leaves.
//!
//! Tab order while the bar is open: field → the chips shown → Facets → Clear → the facets
//! panel (when open) → the grid, and round again (Shift+Tab backwards). The field and the
//! panel keep their own focus flags (`filter_focus`, `facets.focus`); [`Bar::focus`] holds a
//! focus on one of the bar's buttons.
//!
//! F6 / Shift+F6 move between the gallery's regions: the bar (when open), the grid and the
//! chrome's "why is this slow?" indicator (when up). Tab reaches the indicator after the grid
//! (its label, then its hide button) and goes on to the bar; the grid's arrows are its own.

use specular_gfx::Rect;
use specular_input::{Key, Mods, NamedKey};
use specular_widgets::Chrome;
use specular_widgets::line_edit::{Handled, LineEdit, TextClipboard};

use super::*;

/// Accessibility ids of the bar: [`A11Y_BAR`] is the toolbar, the controls follow it.
pub(super) const A11Y_BAR: u64 = A11Y_BASE + (3 << 40);
const ID_FIELD: u64 = A11Y_BAR + 1;
const ID_FACETS: u64 = A11Y_BAR + 2;
const ID_CLEAR: u64 = A11Y_BAR + 3;
const ID_RESULTS: u64 = A11Y_BAR + 4;
/// The filter field's text (its only child: AT-SPI reads text and caret from it).
const ID_FIELD_TEXT: u64 = A11Y_BAR + 5;
const ID_CHIP: u64 = A11Y_BAR + 16;

/// How long the filter has to stay unchanged before its result count is announced (typing
/// is not interrupted by a count per key).
pub(super) const RESULTS_DELAY_S: f64 = 0.6;

/// A place the keyboard focus can be while the filter bar is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Stop {
    Field,
    /// The `k`-th chip shown.
    Chip(usize),
    Facets,
    Clear,
    /// The Library's date timeline (a bar of it).
    Timeline,
    /// The facets panel.
    Panel,
    Grid,
}

/// The tab order: field, chips, Facets, Clear, the timeline when shown, the panel when
/// open, the grid.
pub(super) fn order(chips: usize, panel: bool, timeline: bool) -> Vec<Stop> {
    let mut v = vec![Stop::Field];
    v.extend((0..chips).map(Stop::Chip));
    v.extend([Stop::Facets, Stop::Clear]);
    if timeline {
        v.push(Stop::Timeline);
    }
    if panel {
        v.push(Stop::Panel);
    }
    v.push(Stop::Grid);
    v
}

/// What F6 moves between in the gallery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Region {
    /// The filter bar (and its facets panel).
    Bar,
    Grid,
    /// The chrome's "why is this slow?" indicator.
    Indicator,
}

#[derive(Debug, Default)]
pub(super) struct Bar {
    /// A focused button of the bar (a chip, Facets or Clear).
    pub(super) focus: Option<Stop>,
    /// How many chips the last frame had room for (the rest are not drawn or reachable).
    pub(super) chips_shown: usize,
    /// Where the last frame drew each control.
    pub(super) rects: Vec<(Stop, Rect)>,
    pub(super) results: Results,
}

/// The polite "N items match" after the filter changed, once typing pauses.
#[derive(Debug, Default)]
pub(super) struct Results {
    /// The filter as last seen (None: the bar is closed).
    seen: Option<String>,
    /// When to announce.
    due: Option<f64>,
    /// The count announced (the status node's text).
    pub(super) said: Option<usize>,
}

impl Results {
    /// Called every frame with the filter text (None while the bar is closed) and the result
    /// count (None while it is still being worked out). Returns the seconds until it wants to
    /// look again.
    pub(super) fn update(
        &mut self,
        filter: Option<&str>,
        now: f64,
        count: Option<usize>,
    ) -> Option<f64> {
        let Some(filter) = filter else {
            *self = Results::default();
            return None;
        };
        match &self.seen {
            // Opening the bar announces nothing; only what it is changed to.
            None => self.seen = Some(filter.to_owned()),
            Some(s) if s != filter => {
                self.seen = Some(filter.to_owned());
                self.due = Some(now + RESULTS_DELAY_S);
            }
            Some(_) => {}
        }
        let due = self.due?;
        if now < due {
            return Some(due - now);
        }
        match count {
            Some(n) => {
                self.due = None;
                self.said = Some(n);
                None
            }
            None => Some(0.1),
        }
    }
}

impl Gallery {
    /// Where the keyboard focus is.
    pub(super) fn stop(&self) -> Stop {
        if self.library.albums.kb.is_some() && self.timeline_h() > 0.0 {
            return Stop::Timeline;
        }
        if !self.filter_open {
            return Stop::Grid;
        }
        if self.facets.focus.is_some() || self.library.focus.is_some() || self.facets_finding() {
            return Stop::Panel;
        }
        if self.filter_focus {
            return Stop::Field;
        }
        match self.bar.focus {
            Some(Stop::Chip(_)) if self.bar.chips_shown == 0 => Stop::Facets,
            Some(Stop::Chip(k)) => Stop::Chip(k.min(self.bar.chips_shown - 1)),
            Some(s) => s,
            None => Stop::Grid,
        }
    }

    /// Whether the grid has the keyboard: not while the filter bar, its facets panel or the
    /// chrome's indicator has it (the focused tile then draws as any other).
    pub(super) fn grid_has_keyboard(&self) -> bool {
        self.stop() == Stop::Grid && !self.chrome_kbd
    }

    /// The filter field's editing state, caught up with the text first: changed another way
    /// (a chip removed, a facet toggled, cleared), it takes the text as one undo step with the
    /// caret at the end.
    pub(super) fn field(&mut self) -> &mut LineEdit {
        if self.filter_field.text() != self.filter.text {
            self.filter_field.set_text(&self.filter.text);
            // What a field cannot hold (a line break) is gone from the query too.
            if self.filter_field.text() != self.filter.text {
                self.filter.text = self.filter_field.text().to_owned();
                self.dirty = true;
            }
        }
        &mut self.filter_field
    }

    /// The field edited: the query follows its text, and the picker its caret.
    fn field_edited(&mut self) {
        self.picker.typed();
        if self.filter.text != self.filter_field.text() {
            self.filter.text = self.filter_field.text().to_owned();
            self.dirty = true;
        }
    }

    /// The filter field's caret: a byte offset into the text (its end unless put elsewhere).
    pub(super) fn filter_caret(&self) -> usize {
        if self.filter_field.text() == self.filter.text {
            self.filter_field.caret()
        } else {
            self.filter.text.len()
        }
    }

    /// The filter field's selection (bytes; empty when there is none or the text changed
    /// since).
    pub(super) fn filter_selection(&self) -> std::ops::Range<usize> {
        if self.filter_field.text() == self.filter.text {
            self.filter_field.selection()
        } else {
            let n = self.filter.text.len();
            n..n
        }
    }

    /// Puts the caret at byte `c` of the text as it is now.
    pub(super) fn set_filter_caret(&mut self, c: usize) {
        self.field().set_caret(c, false);
    }

    /// Text typed or committed by the input method, over the selection.
    pub(super) fn filter_insert(&mut self, s: &str) {
        self.field().insert(s);
        self.field_edited();
    }

    /// Text pasted (from the clipboard, read in the background): one line, over the selection.
    pub(in crate::app) fn filter_paste(&mut self, s: &str) {
        self.field().paste(s);
        self.field_edited();
    }

    /// A key while the filter field has the keyboard: the caret, selection, clipboard and undo
    /// keys of a text field ([`LineEdit::key`]). Ctrl+Backspace and Ctrl+Delete take a whole
    /// term (up to the space before or after the caret); arrows follow the text's direction.
    pub(super) fn filter_key(
        &mut self,
        key: Key,
        mods: Mods,
        clip: &mut dyn TextClipboard,
    ) -> Handled {
        let rtl = self.filter_drawn.as_ref().map_or(self.rtl, |(b, _)| b.rtl);
        let h = self.field().key(key, mods, rtl, clip);
        if h.used() {
            self.field_edited();
        }
        h
    }

    /// [`filter_key`](Self::filter_key) for a caret or deletion key (`word`: Ctrl), without a
    /// clipboard.
    #[cfg(test)]
    pub(super) fn filter_edit(&mut self, n: NamedKey, word: bool) {
        let mods = Mods {
            ctrl: word,
            ..Mods::NONE
        };
        let mut clip = specular_widgets::line_edit::MemoryClipboard::default();
        self.filter_key(Key::Named(n), mods, &mut clip);
    }

    /// The byte of the filter text under `x` as last drawn.
    fn filter_byte_at(&self, x: f32) -> Option<usize> {
        let (b, x0) = self.filter_drawn.as_ref()?;
        Some(b.hit_byte(x - x0).min(self.filter.text.len()))
    }

    /// A press in the filter field at `x` (`now` in seconds; `shift`: extend the selection):
    /// the caret goes to the nearest character boundary, a double click selects the word, a
    /// triple click everything, and a drag ([`filter_drag`](Self::filter_drag)) selects.
    pub(super) fn filter_press(&mut self, x: f32, now: f64, shift: bool) {
        if !self.preedit.is_empty() {
            return;
        }
        if let Some(c) = self.filter_byte_at(x) {
            self.field().press(c, x, now, shift);
            self.picker.typed();
        }
    }

    /// Copy-on-select: the field's selection, when it is a new one, goes to `clip` as the
    /// primary selection (nothing while a drag is still making it).
    pub(super) fn filter_offer_primary(&mut self, clip: &mut dyn TextClipboard) {
        if self.filter_field.text() == self.filter.text {
            self.filter_field.offer_primary(clip);
        }
    }

    /// Copy-on-select after a key: the selection is offered once the keys are let go
    /// ([`filter_offer_primary`](Self::filter_offer_primary)) or have rested a moment
    /// ([`filter_offer_primary_due`](Self::filter_offer_primary_due)), not on every step of a
    /// held Shift+Arrow.
    pub(super) fn filter_offer_primary_later(&mut self, now: f64) {
        if self.filter_field.text() == self.filter.text {
            self.filter_field.offer_primary_later(now);
        }
    }

    /// Offers the selection put off by [`filter_offer_primary_later`](Self::filter_offer_primary_later)
    /// when the keys have rested; when it is still to come, the time to look again.
    pub(super) fn filter_offer_primary_due(
        &mut self,
        now: f64,
        clip: &mut dyn TextClipboard,
    ) -> Option<f64> {
        if self.filter_field.text() == self.filter.text {
            self.filter_field.offer_primary_due(now, clip);
        }
        self.filter_field.primary_wake(now)
    }

    /// A selection made with the keys waits to be offered.
    pub(super) fn filter_primary_pending(&self) -> bool {
        self.filter_field.primary_pending()
    }

    /// A middle click in the filter field at `x`: the caret goes there (the start of an empty
    /// field) and the primary selection is pasted at it, from `clip` (a [`PendingClipboard`]
    /// asks for it: it comes back through [`filter_paste`](Self::filter_paste)).
    ///
    /// [`PendingClipboard`]: specular_widgets::line_edit::PendingClipboard
    pub(super) fn filter_middle(&mut self, x: f32, clip: &mut dyn TextClipboard) {
        if !self.preedit.is_empty() {
            return;
        }
        let c = self.filter_byte_at(x).unwrap_or(0);
        if self.field().paste_primary_at(c, clip) == Handled::Edited {
            self.field_edited();
        }
        self.picker.typed();
    }

    /// A click in the filter field at `x`: the caret goes to the nearest character boundary.
    #[cfg(test)]
    pub(super) fn filter_click(&mut self, x: f32) {
        self.filter_press(x, -1.0, false);
        self.filter_release();
    }

    /// The pointer moved to `x` with the button held after a press in the field; false when
    /// there is no such press.
    pub(super) fn filter_drag(&mut self, x: f32) -> bool {
        if !self.filter_field.dragging() {
            return false;
        }
        if let Some(c) = self.filter_byte_at(x) {
            self.field().drag_to(c);
        }
        true
    }

    /// AccessKit's SetTextSelection (in characters) and SetValue on the filter field; false
    /// when the request is not one of those for it.
    pub(super) fn field_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        use accesskit::{Action, ActionData};
        if !(self.active && self.filter_open)
            || !matches!(req.target_node.0, ID_FIELD | ID_FIELD_TEXT)
        {
            return false;
        }
        match (req.action, &req.data) {
            (Action::SetTextSelection, Some(ActionData::SetTextSelection(sel))) => {
                self.go(Stop::Field);
                self.field()
                    .set_a11y_selection(sel.anchor.character_index, sel.focus.character_index);
                self.picker.typed();
            }
            (Action::SetValue, Some(ActionData::Value(v))) => {
                self.go(Stop::Field);
                self.field().set_text(v);
                self.field_edited();
            }
            _ => return false,
        }
        true
    }

    /// The button went up; true when it ended a press in the field.
    pub(super) fn filter_release(&mut self) -> bool {
        let was = self.filter_field.dragging();
        self.filter_field.release();
        was
    }

    /// Moves the keyboard focus to `s`.
    pub(super) fn go(&mut self, s: Stop) {
        self.filter_focus = false;
        self.facets.focus = None;
        self.facets.find_focus = false;
        self.library.focus = None;
        self.bar.focus = None;
        self.library.albums.kb = None;
        match s {
            Stop::Field => self.filter_focus = true,
            Stop::Timeline => {
                self.library.albums.kb = self.timeline_start().filter(|_| self.timeline_h() > 0.0);
            }
            Stop::Chip(_) | Stop::Facets | Stop::Clear => self.bar.focus = Some(s),
            Stop::Panel if !self.facets.open && self.library_sidebar_shown() => {
                self.library.focus = Some(0);
            }
            Stop::Panel => self.facets.focus = self.facets.value_row(0, 1).or(Some(0)),
            Stop::Grid => {
                if self.focus_index().is_none() && !self.tiles.is_empty() {
                    self.set_focus(0);
                }
            }
        }
    }

    /// Tab (or Shift+Tab: `back`) while the bar is open.
    pub(super) fn tab(&mut self, back: bool) {
        let order = order(
            self.bar.chips_shown,
            self.facets.open || self.library_sidebar_shown(),
            self.timeline_h() > 0.0 && !self.library.albums.bars.is_empty(),
        );
        let at = self.stop();
        let i = order
            .iter()
            .position(|&s| s == at)
            .unwrap_or(order.len() - 1);
        let n = order.len();
        let next = if back { (i + n - 1) % n } else { (i + 1) % n };
        self.go(order[next]);
    }

    /// The region the keyboard is in.
    fn region(&self, ch: &Chrome) -> Region {
        if ch.indicator_focused() {
            Region::Indicator
        } else if self.stop() == Stop::Grid {
            Region::Grid
        } else {
            Region::Bar
        }
    }

    /// Puts the keyboard on region `r` (the indicator: on its hide button when `last`).
    fn enter_region(&mut self, ch: &mut Chrome, r: Region, last: bool) {
        match r {
            Region::Bar => {
                ch.blur();
                self.go(Stop::Field);
            }
            Region::Grid => {
                ch.blur();
                self.go(Stop::Grid);
            }
            Region::Indicator => {
                // The grid keeps its place under it (arrows come back there).
                self.go(Stop::Grid);
                ch.focus_indicator(last);
            }
        }
    }

    /// F6 / Shift+F6 between the regions (the filter bar when open, the grid, the "why is
    /// this slow?" indicator when up), and Tab / Shift+Tab into, across and out of the
    /// indicator: it comes after the grid, before the bar. `tab_bound` / `f6_bound`: the
    /// keymap binds the key (then the grid leaves it to the keymap). True when used.
    pub(super) fn region_key(
        &mut self,
        ch: &mut Chrome,
        n: NamedKey,
        back: bool,
        bound: bool,
    ) -> bool {
        let at = self.region(ch);
        let shown = ch.indicator_shown();
        match n {
            NamedKey::F(6) if !bound => {
                let mut regions = Vec::with_capacity(3);
                if self.filter_open {
                    regions.push(Region::Bar);
                }
                regions.push(Region::Grid);
                if shown {
                    regions.push(Region::Indicator);
                }
                let k = regions.len();
                if k < 2 {
                    return false;
                }
                let i = regions.iter().position(|&r| r == at).unwrap_or(0);
                let to = if back { (i + k - 1) % k } else { (i + 1) % k };
                self.enter_region(ch, regions[to], false);
                true
            }
            NamedKey::Tab if at == Region::Indicator => {
                if !ch.indicator_tab(back) {
                    // Past its end: on to the bar (or round to the grid); before it, the grid.
                    let to = if !back && self.filter_open {
                        Region::Bar
                    } else {
                        Region::Grid
                    };
                    self.enter_region(ch, to, false);
                }
                true
            }
            // Tab from the grid; while the bar is closed only when the keymap leaves Tab free.
            NamedKey::Tab
                if shown && !back && at == Region::Grid && (self.filter_open || !bound) =>
            {
                self.enter_region(ch, Region::Indicator, false);
                true
            }
            // Shift+Tab back from the start of the round: the field, or the grid alone.
            NamedKey::Tab
                if shown
                    && back
                    && ((self.filter_open && self.stop() == Stop::Field)
                        || (!self.filter_open && at == Region::Grid && !bound)) =>
            {
                self.enter_region(ch, Region::Indicator, true);
                true
            }
            _ => false,
        }
    }

    /// Closes the bar and drops the filter (the bar's close button).
    pub(super) fn close_filter(&mut self) {
        self.facets.close();
        self.filter_open = false;
        self.filter_focus = false;
        self.bar.focus = None;
        self.filter = Filter::default();
        self.dirty = true;
        self.relayout = true;
    }

    /// Presses a control of the bar (Enter, Space or an assistive technology's click).
    pub(super) fn press(&mut self, s: Stop) {
        match s {
            Stop::Chip(k) => {
                if let Some((chip, _)) = self.filter.chips().get(k) {
                    self.filter.remove(*chip);
                    self.dirty = true;
                }
                // The focus stays at the same place in the row (on the field once none is left).
                match self.filter.chips().len() {
                    0 => self.go(Stop::Field),
                    n => self.go(Stop::Chip(k.min(n - 1))),
                }
            }
            Stop::Facets => {
                let open = !self.facets.open;
                self.facets_show(open, false);
            }
            Stop::Clear => {
                self.close_filter();
                self.go(Stop::Grid);
            }
            Stop::Field | Stop::Timeline | Stop::Panel | Stop::Grid => self.go(s),
        }
    }

    /// Keys while a button of the bar has the focus. True when used.
    pub(super) fn bar_key(&mut self, n: NamedKey, shift: bool) -> bool {
        let at = self.stop();
        if !matches!(at, Stop::Chip(_) | Stop::Facets | Stop::Clear) {
            return false;
        }
        // The row as it reads on screen: field, chips, Facets, Clear (mirrored right to left).
        let row: Vec<Stop> = order(self.bar.chips_shown, false, false)
            .into_iter()
            .filter(|s| *s != Stop::Grid)
            .collect();
        let i = row.iter().position(|&s| s == at).unwrap_or(0);
        let (left, right) = if self.rtl { (1, -1) } else { (-1, 1) };
        let step = |d: isize| row[(i as isize + d).clamp(0, row.len() as isize - 1) as usize];
        match n {
            NamedKey::Tab => self.tab(shift),
            NamedKey::Enter | NamedKey::Space => self.press(at),
            NamedKey::Delete | NamedKey::Backspace if matches!(at, Stop::Chip(_)) => self.press(at),
            NamedKey::ArrowLeft => self.go(step(left)),
            NamedKey::ArrowRight => self.go(step(right)),
            NamedKey::Home => self.go(row[0]),
            NamedKey::End => self.go(row[row.len() - 1]),
            NamedKey::Escape => self.go(Stop::Field),
            NamedKey::ArrowDown => self.go(Stop::Grid),
            _ => return false,
        }
        true
    }

    /// Per frame: forgets a bar focus once the bar is gone and announces the result count
    /// when the filter has settled. Returns when to look again.
    pub(super) fn bar_step(&mut self, now: f64) -> Option<f64> {
        if !self.filter_open {
            self.bar.focus = None;
            self.bar.rects.clear();
            self.bar.chips_shown = 0;
        }
        let filter = self.filter_open.then_some(self.filter.text.as_str());
        let count = (!self.dirty && !self.library.pending()).then_some(self.tiles.len());
        self.bar.results.update(filter, now, count)
    }

    /// AccessKit nodes of the bar: a toolbar with the search field, the chips (pressed toggle
    /// buttons; pressing one removes its term), Facets and Clear, then the polite result
    /// count. The focus goes to the focused control.
    pub(super) fn bar_a11y(
        &self,
        tree: &mut accesskit::TreeUpdate,
        bounds: &dyn Fn(Rect) -> accesskit::Rect,
    ) {
        use accesskit::{Action, Invalid, Live, Node, NodeId, Role, Toggled};
        if !self.filter_open {
            return;
        }
        let rect = |s: Stop| {
            self.bar
                .rects
                .iter()
                .find(|(x, _)| *x == s)
                .map(|(_, r)| *r)
        };
        let mut ids = Vec::new();
        let mut nodes = Vec::new();
        let mut field = Node::new(Role::SearchInput);
        field.set_label(t!("gallery-a11y-filter-field"));
        field.set_placeholder(t!("gallery-filter-query-placeholder"));
        let sel = if self.filter_field.text() == self.filter.text {
            self.filter_field.a11y_selection()
        } else {
            let n = self.filter.text.chars().count();
            (n, n)
        };
        let run = specular_widgets::text_run_sel(
            &mut field,
            NodeId(ID_FIELD_TEXT),
            &self.filter.text,
            Some(sel),
        );
        field.set_children(vec![NodeId(ID_FIELD_TEXT)]);
        if !self.filter.query().errors.is_empty() {
            field.set_invalid(Invalid::True);
        }
        field.add_action(Action::Focus);
        field.add_action(Action::SetValue);
        field.add_action(Action::SetTextSelection);
        // A combo box: its value picker lists the values of the term being typed.
        self.picker_field_a11y(&mut field);
        nodes.push((NodeId(ID_FIELD), field, Stop::Field));
        let chips = self.filter.chips();
        for (k, (_, term)) in chips.iter().take(self.bar.chips_shown).enumerate() {
            let mut n = Node::new(Role::Button);
            n.set_label(t!("gallery-a11y-chip", term = term.as_str()));
            n.set_description(t!("gallery-a11y-chip-remove"));
            n.set_toggled(Toggled::True);
            n.add_action(Action::Click);
            n.add_action(Action::Focus);
            nodes.push((NodeId(ID_CHIP + k as u64), n, Stop::Chip(k)));
        }
        let mut facets = Node::new(Role::Button);
        facets.set_label(t!("gallery-facets"));
        facets.set_toggled(Toggled::from(self.facets.open));
        facets.set_expanded(self.facets.open);
        facets.add_action(Action::Click);
        facets.add_action(Action::Focus);
        nodes.push((NodeId(ID_FACETS), facets, Stop::Facets));
        let mut clear = Node::new(Role::Button);
        clear.set_label(t!("gallery-filter-clear"));
        clear.add_action(Action::Click);
        clear.add_action(Action::Focus);
        nodes.push((NodeId(ID_CLEAR), clear, Stop::Clear));

        let at = self.stop();
        let mut bar = Node::new(Role::Toolbar);
        bar.set_label(t!("gallery-a11y-filter-bar"));
        if let (Some(a), Some(b)) = (rect(Stop::Field), rect(Stop::Clear)) {
            let (x0, x1) = (a.x.min(b.x), a.right().max(b.right()));
            let (y0, y1) = (a.y.min(b.y), a.bottom().max(b.bottom()));
            bar.set_bounds(bounds(Rect::new(x0, y0, x1 - x0, y1 - y0)));
        }
        for (id, mut n, s) in nodes {
            if let Some(r) = rect(s) {
                n.set_bounds(bounds(r));
            }
            if s == at && tree.focus == NodeId(1) {
                tree.focus = id;
            }
            ids.push(id);
            tree.nodes.push((id, n));
        }
        bar.set_children(ids);
        tree.nodes.push((NodeId(A11Y_BAR), bar));
        tree.nodes.push((NodeId(ID_FIELD_TEXT), run));
        let said = self.bar.results.said.map(|count| {
            let mut n = Node::new(Role::Status);
            n.set_live(Live::Polite);
            n.set_label(t!("gallery-a11y-results", count = count as i64));
            (NodeId(ID_RESULTS), n)
        });
        if let Some((_, root)) = tree.nodes.iter_mut().find(|(i, _)| *i == NodeId(1)) {
            root.push_child(NodeId(A11Y_BAR));
            if let Some((id, _)) = &said {
                root.push_child(*id);
            }
        }
        tree.nodes.extend(said);
    }

    /// The control an accessibility id names, if it is one of the bar's.
    fn bar_stop_of(&self, target: u64) -> Option<Stop> {
        if !self.filter_open {
            return None;
        }
        match target {
            ID_FIELD => Some(Stop::Field),
            ID_FACETS => Some(Stop::Facets),
            ID_CLEAR => Some(Stop::Clear),
            t if (ID_CHIP..ID_CHIP + self.bar.chips_shown as u64).contains(&t) => {
                Some(Stop::Chip((t - ID_CHIP) as usize))
            }
            _ => None,
        }
    }
}

impl App {
    /// AccessKit: a screen reader sets the filter field's text or selection; false when the
    /// request is not that.
    pub(in crate::app) fn gallery_field_a11y_request(
        &mut self,
        req: &accesskit::ActionRequest,
    ) -> bool {
        let done = self.gallery.field_a11y_request(req);
        if done {
            self.redraw = true;
            self.filter_primary();
        }
        done
    }

    /// Copy-on-select in the filter field: a new selection becomes the primary selection
    /// (X11, Wayland; nothing elsewhere).
    pub(super) fn filter_primary(&mut self) {
        if !specular_widgets::line_edit::PRIMARY_SELECTION {
            return;
        }
        let mut clip = specular_widgets::line_edit::PendingClipboard::default();
        self.gallery.filter_offer_primary(&mut clip);
        self.field_clipboard(clip, crate::app::integration::PasteTo::GalleryFilter);
    }

    /// Offers a selection the keys made in the filter field once they have rested; the time
    /// to look again while one waits.
    pub(in crate::app) fn filter_primary_tick(&mut self, now: f64) -> Option<f64> {
        if !specular_widgets::line_edit::PRIMARY_SELECTION || !self.gallery.filter_primary_pending()
        {
            return None;
        }
        let mut clip = specular_widgets::line_edit::PendingClipboard::default();
        let wake = self.gallery.filter_offer_primary_due(now, &mut clip);
        self.field_clipboard(clip, crate::app::integration::PasteTo::GalleryFilter);
        wake
    }

    /// A middle click in the filter field at `x` (X11, Wayland): it takes the keyboard and
    /// the primary selection is pasted where the click was.
    pub(super) fn filter_middle_click(&mut self, x: f32) {
        if !specular_widgets::line_edit::PRIMARY_SELECTION {
            return;
        }
        self.gallery.go(Stop::Field);
        let mut clip = specular_widgets::line_edit::PendingClipboard::default();
        self.gallery.filter_middle(x, &mut clip);
        self.field_clipboard(clip, crate::app::integration::PasteTo::GalleryFilter);
    }

    /// AccessKit: an action on a control of the bar; false when not ours.
    pub(super) fn bar_a11y_action(&mut self, target: u64, click: bool) -> bool {
        let ga = &mut self.gallery;
        let Some(s) = ga.bar_stop_of(target) else {
            return false;
        };
        if click {
            ga.press(s);
        } else {
            ga.go(s);
        }
        self.redraw = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use accesskit::{NodeId, Role, Toggled, TreeUpdate};

    fn open_bar(text: &str, chips_shown: usize) -> Gallery {
        let mut g = Gallery {
            active: true,
            filter_open: true,
            ..Gallery::default()
        };
        g.filter.text = text.into();
        g.bar.chips_shown = chips_shown;
        g
    }

    fn tree_of(g: &Gallery) -> TreeUpdate {
        let mut root = accesskit::Node::new(Role::Window);
        root.set_label("x");
        let mut tree = TreeUpdate {
            nodes: vec![(NodeId(1), root)],
            tree: None,
            tree_id: accesskit::TreeId::ROOT,
            focus: NodeId(1),
        };
        g.a11y(&mut tree, 1.0);
        tree
    }

    fn node(tree: &TreeUpdate, id: u64) -> &accesskit::Node {
        &tree
            .nodes
            .iter()
            .find(|(i, _)| *i == NodeId(id))
            .expect("node")
            .1
    }

    /// The filter field is a text field: Shift and Ctrl select, typing and pasting replace
    /// the selection (a paste as one line), Ctrl+C/X/V go through the clipboard given, Ctrl+Z
    /// and Ctrl+Y undo and redo, and AccessKit sees and sets the selection.
    #[test]
    fn filter_field_selects_copies_pastes_and_undoes() {
        use accesskit::{Action, ActionData, ActionRequest, TextPosition, TextSelection};
        use specular_widgets::line_edit::MemoryClipboard;
        let mut g = open_bar("beach iso:100", 1);
        g.go(Stop::Field);
        let mut clip = MemoryClipboard::default();
        let ctrl = Mods {
            ctrl: true,
            ..Mods::NONE
        };
        let ctrl_shift = Mods {
            shift: true,
            ..ctrl
        };
        let named = |n| Key::Named(n);
        // Shift+Ctrl+Left selects the last word; Shift+Ctrl+Home everything before the caret.
        let k = g.filter_key(named(NamedKey::ArrowLeft), ctrl_shift, &mut clip);
        assert!(k.used());
        let one = g.filter_selection();
        assert_eq!(one, 10..13);
        g.filter_key(named(NamedKey::Home), ctrl_shift, &mut clip);
        assert_eq!(g.filter_selection(), 0..13);
        g.filter_key(named(NamedKey::End), Mods::NONE, &mut clip);
        assert_eq!(g.filter_selection(), 13..13);
        g.filter_key(named(NamedKey::ArrowLeft), ctrl_shift, &mut clip);
        assert_eq!(g.filter_selection(), one);
        // Ctrl+X cuts it, Ctrl+V puts text back over the selection, as one line.
        let k = g.filter_key(Key::Char('x'), ctrl, &mut clip);
        assert_eq!(k, Handled::Edited);
        assert_eq!(clip.text.as_deref(), Some(&"beach iso:100"[one.clone()]));
        assert_eq!(g.filter.text, &"beach iso:100"[..one.start]);
        clip.text = Some("200\nsunset".into());
        g.filter_key(Key::Char('v'), ctrl, &mut clip);
        assert_eq!(g.filter.text, "beach iso:200 sunset");
        assert!(g.dirty);
        // Ctrl+A and typing replaces everything; undo and redo step back and forth.
        g.filter_key(Key::Char('a'), ctrl, &mut clip);
        assert_eq!(g.filter_selection(), 0..20);
        g.filter_insert("dunes");
        assert_eq!(g.filter.text, "dunes");
        g.filter_key(Key::Char('z'), ctrl, &mut clip);
        assert_eq!(g.filter.text, "beach iso:200 sunset");
        g.filter_key(Key::Char('z'), ctrl, &mut clip);
        g.filter_key(Key::Char('z'), ctrl, &mut clip);
        assert_eq!(g.filter.text, "beach iso:100");
        g.filter_key(Key::Char('y'), ctrl, &mut clip);
        g.filter_key(Key::Char('z'), ctrl_shift, &mut clip);
        assert_eq!(g.filter.text, "beach iso:200 sunset");
        // Ctrl+C copies the selection without changing the text.
        g.set_filter_caret(0);
        g.filter_key(named(NamedKey::ArrowRight), ctrl_shift, &mut clip);
        let k = g.filter_key(Key::Char('c'), ctrl, &mut clip);
        assert_eq!(k, Handled::Used);
        assert_eq!(clip.text.as_deref().map(str::trim_end), Some("beach"));
        assert_eq!(g.filter.text, "beach iso:200 sunset");
        // AccessKit: the selection in characters, and SetTextSelection.
        let tree = tree_of(&g);
        let sel = node(&tree, ID_FIELD).text_selection().expect("a selection");
        assert_eq!(sel.anchor.character_index, 0);
        assert!(node(&tree, ID_FIELD).supports_action(Action::SetTextSelection));
        let at = |i| TextPosition {
            node: NodeId(ID_FIELD_TEXT),
            character_index: i,
        };
        let req = ActionRequest {
            action: Action::SetTextSelection,
            target_tree: accesskit::TreeId::ROOT,
            target_node: NodeId(ID_FIELD_TEXT),
            data: Some(ActionData::SetTextSelection(TextSelection {
                anchor: at(10),
                focus: at(6),
            })),
        };
        g.go(Stop::Grid);
        assert!(g.field_a11y_request(&req));
        assert!(g.filter_focus);
        assert_eq!(g.filter_selection(), 6..10);
        assert_eq!(g.filter_caret(), 6);
        let tree = tree_of(&g);
        let sel = node(&tree, ID_FIELD).text_selection().unwrap();
        assert_eq!(
            (sel.anchor.character_index, sel.focus.character_index),
            (10, 6)
        );
    }

    /// The primary selection (X11, Wayland), with an in-memory clipboard only: a selection
    /// made in the filter field is offered once, not while a drag is still making it, and
    /// never touches the clipboard; a middle click puts the caret where it is (the start of
    /// an empty field) and pastes the primary selection there, as one undo step.
    #[test]
    fn filter_field_offers_the_primary_selection_and_takes_a_middle_click() {
        use specular_widgets::line_edit::{MemoryClipboard, PendingClipboard};
        let mut g = open_bar("beach iso:100", 1);
        g.go(Stop::Field);
        let mut clip = MemoryClipboard::default();
        let ctrl_shift = Mods {
            ctrl: true,
            shift: true,
            ..Mods::NONE
        };
        g.filter_key(Key::Named(NamedKey::ArrowLeft), ctrl_shift, &mut clip);
        g.filter_offer_primary(&mut clip);
        assert_eq!(clip.primary.as_deref(), Some("100"));
        assert_eq!(clip.text, None);
        // Offered once: the same selection again is not.
        clip.primary = None;
        g.filter_offer_primary(&mut clip);
        assert_eq!(clip.primary, None);
        // A middle click (no text drawn here: at the start) pastes it, one line.
        clip.primary = Some("sunset\n".into());
        g.filter_middle(0.0, &mut clip);
        assert_eq!(g.filter.text, "sunset beach iso:100");
        assert_eq!(g.filter_selection(), 7..7);
        assert!(g.dirty);
        g.filter_key(
            Key::Char('z'),
            Mods {
                ctrl: true,
                ..Mods::NONE
            },
            &mut clip,
        );
        assert_eq!(g.filter.text, "beach iso:100");
        // The app's pending clipboard asks for the primary selection instead.
        let mut pending = PendingClipboard::default();
        g.filter_middle(0.0, &mut pending);
        assert!(pending.paste_primary);
        assert_eq!(g.filter.text, "beach iso:100");
        g.filter_paste("x ");
        assert_eq!(g.filter.text, "x beach iso:100");
        assert_eq!(clip.text, None);
    }

    /// A selection the keys make in the filter field is offered once they rest (or are let
    /// go), not on every step of a held Shift+Arrow. In-memory clipboard only.
    #[test]
    fn filter_field_offers_a_keyboard_selection_once_the_keys_rest() {
        use specular_widgets::line_edit::{MemoryClipboard, PRIMARY_IDLE_S};
        let mut g = open_bar("beach iso:100", 1);
        g.go(Stop::Field);
        let mut clip = MemoryClipboard::default();
        let mut now = 10.0;
        for _ in 0..3 {
            g.filter_key(Key::Named(NamedKey::ArrowLeft), Mods::SHIFT, &mut clip);
            g.filter_offer_primary_later(now);
            assert!(g.filter_primary_pending());
            now += 0.03;
            let wake = g.filter_offer_primary_due(now, &mut clip);
            assert!(wake.is_some_and(|d| d > 0.0 && d <= PRIMARY_IDLE_S));
            assert_eq!(clip.primary, None);
        }
        // Rested: offered once.
        let wake = g.filter_offer_primary_due(now + PRIMARY_IDLE_S, &mut clip);
        assert_eq!(wake, None);
        assert_eq!(clip.primary.as_deref(), Some("100"));
        assert!(!g.filter_primary_pending());
        // Let go before then: offered at once (as `gallery_key_up` does).
        g.filter_key(Key::Named(NamedKey::ArrowLeft), Mods::SHIFT, &mut clip);
        g.filter_offer_primary_later(now + 1.0);
        g.filter_offer_primary(&mut clip);
        assert_eq!(clip.primary.as_deref(), Some(":100"));
        assert!(!g.filter_primary_pending());
        assert_eq!(clip.text, None);
    }

    #[test]
    fn order_runs_field_chips_buttons_panel_grid() {
        use Stop::*;
        assert_eq!(order(0, false, false), vec![Field, Facets, Clear, Grid]);
        assert_eq!(
            order(2, true, false),
            vec![Field, Chip(0), Chip(1), Facets, Clear, Panel, Grid]
        );
        assert_eq!(
            order(0, true, true),
            vec![Field, Facets, Clear, Timeline, Panel, Grid]
        );
    }

    #[test]
    fn tab_cycles_through_the_bar_and_back() {
        let mut g = open_bar("camera:x-t5 rating>=3", 2);
        g.filter_focus = true;
        let mut seen = vec![g.stop()];
        for _ in 0..5 {
            g.tab(false);
            seen.push(g.stop());
        }
        use Stop::*;
        // No tiles: the grid stop holds no tile focus, the round goes on to the field.
        assert_eq!(seen, vec![Field, Chip(0), Chip(1), Facets, Clear, Grid]);
        g.tab(false);
        assert_eq!(g.stop(), Field);
        assert!(g.filter_focus && g.bar.focus.is_none());
        g.tab(true);
        assert_eq!(g.stop(), Grid);
        g.tab(true);
        assert_eq!(g.stop(), Clear);
        // Once the bar closes the bar focus is no more.
        g.filter_open = false;
        assert_eq!(g.stop(), Grid);
        g.bar_step(0.0);
        assert!(g.bar.focus.is_none());
    }

    #[test]
    fn keys_on_chips_remove_them_and_arrows_walk_the_row() {
        let mut g = open_bar("camera:x-t5 rating>=3 beach", 2);
        g.go(Stop::Chip(1));
        assert!(g.bar_key(NamedKey::ArrowLeft, false));
        assert_eq!(g.stop(), Stop::Chip(0));
        assert!(g.bar_key(NamedKey::ArrowLeft, false));
        assert_eq!(g.stop(), Stop::Field);
        g.go(Stop::Chip(0));
        g.rtl = true;
        assert!(g.bar_key(NamedKey::ArrowLeft, false));
        assert_eq!(g.stop(), Stop::Chip(1), "mirrored right to left");
        g.rtl = false;
        // Enter removes the chip's term; the focus stays in the row.
        assert!(g.bar_key(NamedKey::Enter, false));
        assert!(!g.filter.text.contains("rating"), "{}", g.filter.text);
        assert!(g.filter.text.contains("camera:x-t5"));
        assert_eq!(g.stop(), Stop::Chip(0));
        assert!(g.bar_key(NamedKey::Delete, false));
        assert_eq!(g.filter.text.trim(), "beach");
        assert_eq!(g.stop(), Stop::Field, "no chip left: back to the field");
        // Facets toggles the panel, Clear closes the bar.
        g.go(Stop::Facets);
        assert!(g.bar_key(NamedKey::Space, false));
        assert!(g.facets.open);
        g.go(Stop::Clear);
        assert!(g.bar_key(NamedKey::Enter, false));
        assert!(!g.filter_open && g.filter.text.is_empty() && !g.facets.open);
        assert_eq!(g.stop(), Stop::Grid);
    }

    #[test]
    fn bar_is_a_toolbar_with_a_search_field_and_pressed_chips() {
        let mut g = open_bar("camera:x-t5 iso:<abc", 1);
        g.facets.open = true;
        g.go(Stop::Chip(0));
        let tree = tree_of(&g);
        let bar = node(&tree, A11Y_BAR);
        assert_eq!(bar.role(), Role::Toolbar);
        assert!(bar.label().is_some());
        let field = node(&tree, ID_FIELD);
        assert_eq!(field.role(), Role::SearchInput);
        assert_eq!(field.value(), Some("camera:x-t5 iso:<abc"));
        assert!(field.placeholder().is_some());
        assert!(field.invalid().is_some(), "the query has an error");
        let chip = node(&tree, ID_CHIP);
        assert_eq!(chip.role(), Role::Button);
        assert_eq!(chip.toggled(), Some(Toggled::True));
        assert!(chip.label().is_some_and(|l| l.contains("camera:x-t5")));
        assert!(chip.description().is_some());
        // Only the chips shown are in the tree.
        assert!(!tree.nodes.iter().any(|(i, _)| *i == NodeId(ID_CHIP + 1)));
        let facets = node(&tree, ID_FACETS);
        assert_eq!(facets.toggled(), Some(Toggled::True));
        assert_eq!(facets.is_expanded(), Some(true));
        assert_eq!(node(&tree, ID_CLEAR).role(), Role::Button);
        assert_eq!(tree.focus, NodeId(ID_CHIP));
        // The bar reads before the grid.
        let root = node(&tree, 1);
        let kids = root.children();
        let bar_at = kids
            .iter()
            .position(|i| *i == NodeId(A11Y_BAR))
            .expect("bar");
        let list_at = kids
            .iter()
            .position(|i| *i == NodeId(A11Y_BASE))
            .expect("list");
        assert!(bar_at < list_at);
        // Closed: no bar.
        g.filter_open = false;
        let tree = tree_of(&g);
        assert!(!tree.nodes.iter().any(|(i, _)| *i == NodeId(A11Y_BAR)));
    }

    #[test]
    fn result_count_is_announced_politely_once_typing_pauses() {
        let mut r = Results::default();
        // Opening the bar says nothing.
        assert_eq!(r.update(Some(""), 0.0, Some(10)), None);
        assert_eq!(r.said, None);
        // Typing: each key pushes the announcement back.
        let w = r.update(Some("b"), 1.0, Some(4)).expect("waits");
        assert!((w - RESULTS_DELAY_S).abs() < 1e-9);
        r.update(Some("be"), 1.3, Some(3));
        assert_eq!(
            r.update(Some("be"), 1.3 + RESULTS_DELAY_S - 0.1, Some(3))
                .map(|w| w > 0.0),
            Some(true)
        );
        assert_eq!(r.said, None);
        // Settled, but still filtering: a little longer.
        assert_eq!(r.update(Some("be"), 2.0, None), Some(0.1));
        assert_eq!(r.update(Some("be"), 2.1, Some(2)), None);
        assert_eq!(r.said, Some(2));
        // Unchanged: nothing more to do.
        assert_eq!(r.update(Some("be"), 5.0, Some(2)), None);
        // Closing forgets it.
        r.update(None, 6.0, None);
        assert_eq!(r.said, None);

        let mut g = open_bar("x", 0);
        g.bar.results.said = Some(0);
        let tree = tree_of(&g);
        let n = node(&tree, ID_RESULTS);
        assert_eq!(n.role(), Role::Status);
        assert_eq!(n.live(), Some(accesskit::Live::Polite));
        assert!(n.label().is_some());
    }

    fn chrome_with_indicator() -> Chrome {
        let mut ch = Chrome::new();
        ch.gallery = true;
        ch.set_insight_indicator(Some(("Why is this slow?".into(), "Slow".into())));
        ch
    }

    #[test]
    fn f6_moves_between_bar_grid_and_indicator() {
        let mut ch = chrome_with_indicator();
        let mut g = open_bar("x", 0);
        g.filter_focus = true;
        let f6 = |g: &mut Gallery, ch: &mut Chrome, back: bool| {
            assert!(g.region_key(ch, NamedKey::F(6), back, false));
        };
        f6(&mut g, &mut ch, false);
        assert_eq!((g.stop(), ch.indicator_focused()), (Stop::Grid, false));
        f6(&mut g, &mut ch, false);
        assert!(ch.indicator_focused());
        f6(&mut g, &mut ch, false);
        assert_eq!((g.stop(), ch.indicator_focused()), (Stop::Field, false));
        f6(&mut g, &mut ch, true);
        assert!(ch.indicator_focused());
        f6(&mut g, &mut ch, true);
        assert_eq!((g.stop(), ch.indicator_focused()), (Stop::Grid, false));
        // A bound F6 is the keymap's.
        assert!(!g.region_key(&mut ch, NamedKey::F(6), false, true));
        // The bar closed: grid and indicator; without the indicator F6 has nowhere to go.
        g.filter_open = false;
        f6(&mut g, &mut ch, false);
        assert!(ch.indicator_focused());
        f6(&mut g, &mut ch, false);
        assert!(!ch.indicator_focused());
        ch.set_insight_indicator(None);
        assert!(!g.region_key(&mut ch, NamedKey::F(6), false, false));
        // Other keys are not taken.
        assert!(!g.region_key(&mut ch, NamedKey::ArrowRight, false, false));
    }

    #[test]
    fn tab_goes_through_the_indicator_between_grid_and_bar() {
        let mut ch = chrome_with_indicator();
        let mut g = open_bar("x", 0);
        g.go(Stop::Clear);
        let tab = |g: &mut Gallery, ch: &mut Chrome, back: bool| {
            if !g.region_key(ch, NamedKey::Tab, back, false) {
                g.tab(back);
            }
        };
        // Clear → grid → the indicator's label → its hide button → the field.
        tab(&mut g, &mut ch, false);
        assert_eq!((g.stop(), ch.indicator_focused()), (Stop::Grid, false));
        tab(&mut g, &mut ch, false);
        assert!(ch.indicator_focused());
        tab(&mut g, &mut ch, false);
        assert!(ch.indicator_focused());
        tab(&mut g, &mut ch, false);
        assert_eq!((g.stop(), ch.indicator_focused()), (Stop::Field, false));
        // And back: field → hide → label → grid.
        tab(&mut g, &mut ch, true);
        assert!(ch.indicator_focused());
        tab(&mut g, &mut ch, true);
        assert!(ch.indicator_focused());
        tab(&mut g, &mut ch, true);
        assert_eq!((g.stop(), ch.indicator_focused()), (Stop::Grid, false));
        // The bar closed: Tab from the grid to the indicator and round to the grid, unless
        // the keymap binds Tab.
        g.filter_open = false;
        assert!(!g.region_key(&mut ch, NamedKey::Tab, false, true));
        assert!(!g.region_key(&mut ch, NamedKey::Tab, true, true));
        assert!(g.region_key(&mut ch, NamedKey::Tab, true, false));
        assert!(ch.indicator_focused());
        assert!(g.region_key(&mut ch, NamedKey::Tab, false, false));
        assert!(!ch.indicator_focused());
        assert_eq!(g.stop(), Stop::Grid);
        // No indicator: Tab is the grid's as before.
        ch.set_insight_indicator(None);
        assert!(!g.region_key(&mut ch, NamedKey::Tab, false, false));
    }

    /// The focused tile shows its ring only while the grid has the keyboard: not while the
    /// field, a button of the bar, the facets panel or the indicator has it.
    #[test]
    fn tile_ring_only_with_the_keyboard_in_the_grid() {
        let mut g = open_bar("x", 1);
        g.go(Stop::Grid);
        assert!(g.grid_has_keyboard());
        for s in [Stop::Field, Stop::Chip(0), Stop::Facets, Stop::Clear] {
            g.go(s);
            assert!(!g.grid_has_keyboard(), "{s:?}");
        }
        g.facets.focus = Some(0);
        assert!(!g.grid_has_keyboard());
        g.go(Stop::Grid);
        // The chrome's indicator (the app sets this from the chrome each frame).
        let mut ch = chrome_with_indicator();
        assert!(ch.focus_indicator(false));
        g.chrome_kbd = ch.indicator_focused();
        assert!(!g.grid_has_keyboard());
        ch.blur();
        g.chrome_kbd = ch.indicator_focused();
        assert!(g.grid_has_keyboard());
        // With the bar closed the grid has it.
        g.filter_open = false;
        g.filter_focus = true;
        assert!(g.grid_has_keyboard());
    }
}
