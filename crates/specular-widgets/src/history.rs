//! Going back inside dialogs (Esc, Alt+←, the mouse's Back button): Settings keeps a history
//! of the places the user moved to, a dialog opened from another one returns to it, and a
//! dialog with a typed filter clears it before it closes.
//!
//! # The Settings history
//!
//! A [`Place`] is where Settings is: its section, its search, the [`View`] open over the page,
//! and the scroll offset and keyboard focus to bring back. Every input event and
//! assistive-technology request is wrapped in [`Chrome::settings_mark`] and
//! [`Chrome::settings_record`]: when it took the user to another place (a section, a search,
//! a sub-view) the place before is pushed; when it took them back to the place on top of the
//! history by other means (Cancel, Backspace to an empty field, closing a menu), that entry
//! is dropped, so Esc never steps to where the user already is. Esc then pops one place
//! ([`Chrome::settings_back`]); with nothing left the dialog closes. Opening Settings starts
//! an empty history (`SettingsState::reset`); the stack keeps the last [`HISTORY_MAX`] places
//! and never holds the same place twice in a row.
//!
//! A new sub-view of Settings (the config editor, the key binding configurator) hooks in by:
//! 1. adding a variant to [`View`];
//! 2. reporting it from [`Chrome::settings_view`] while it is open;
//! 3. opening and closing it in [`Chrome::settings_show_view`] (which first leaves every
//!    sub-view).
//!
//! Its input then needs nothing more. A change made outside an input event (an application
//! command) runs inside [`Chrome::settings_go`] so it is recorded too.

use specular_i18n::I18n;

use crate::ctx::Target;
use crate::scrollbar::Area;
use crate::settings::SettingsTarget;
use crate::toolbar_edit::Part;
use crate::{Chrome, Modal};

/// How many places the Settings history keeps (the oldest go first).
pub(crate) const HISTORY_MAX: usize = 32;

/// How many dialogs can wait under the open one.
const PARENTS_MAX: usize = 8;

/// What Settings shows over its page (or its search results).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum View {
    /// The page alone.
    #[default]
    Page,
    /// A panel's action waiting for its confirmation (by setting key).
    PanelConfirm(&'static str),
    /// The menu (⋯) of a row of the toolbar list.
    ToolMenu(usize),
}

/// A place in Settings that the history goes back to.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Place {
    pub section: usize,
    pub search: String,
    pub view: View,
    pub scroll: f32,
    /// The control with the keyboard focus (`None`: the search field, unless `tool`).
    pub focus: Option<Target>,
    /// The toolbar list's focused row and part, when the list has the keyboard.
    pub tool: Option<(usize, Part)>,
}

impl Place {
    /// The same place, wherever it is scrolled and whatever has the focus: the search
    /// results count as one place however the query is edited.
    fn same(&self, o: &Place) -> bool {
        self.view == o.view
            && self.search.is_empty() == o.search.is_empty()
            && (!self.search.is_empty() || self.section == o.section)
    }
}

/// The places Settings goes back to with Esc, the latest last.
#[derive(Default)]
pub(crate) struct History {
    stack: Vec<Place>,
    /// Counts the steps back taken, so the step itself is not recorded as a move.
    backs: u64,
}

impl History {
    pub(crate) fn clear(&mut self) {
        self.stack.clear();
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.stack.len()
    }

    /// Records `p` as the place to go back to; the same place twice in a row is kept once
    /// (the later scroll and focus).
    fn push(&mut self, p: Place) {
        if let Some(top) = self.stack.last_mut()
            && top.same(&p)
        {
            *top = p;
            return;
        }
        self.stack.push(p);
        if self.stack.len() > HISTORY_MAX {
            self.stack.remove(0);
        }
    }
}

impl Chrome {
    /// The sub-view open in Settings.
    pub(crate) fn settings_view(&self) -> View {
        if let Some(k) = self.settings.panel_confirm {
            View::PanelConfirm(k)
        } else if let Some(r) = self.settings.tools.menu {
            View::ToolMenu(r)
        } else {
            View::Page
        }
    }

    /// Leaves every sub-view of Settings, then opens `v`.
    pub(crate) fn settings_show_view(&mut self, v: View) {
        self.settings.panel_confirm = None;
        self.settings.tools.close_menu();
        match v {
            View::Page => {}
            View::PanelConfirm(k) => self.settings.panel_confirm = Some(k),
            View::ToolMenu(r) => self.settings.tools.menu = Some(r),
        }
    }

    /// Where Settings is now.
    pub(crate) fn settings_place(&self) -> Place {
        let s = &self.settings;
        Place {
            section: s.section,
            search: s.search.clone(),
            view: self.settings_view(),
            scroll: s.scroll,
            focus: self.modal_focus.clone(),
            tool: s.tools.focus.map(|r| (r, s.tools.part)),
        }
    }

    /// Before something that may move Settings to another place: where it is now (`None`
    /// when Settings is not the dialog on top).
    pub(crate) fn settings_mark(&self) -> Option<(Place, u64)> {
        (self.modal == Some(Modal::Settings))
            .then(|| (self.settings_place(), self.settings.history.backs))
    }

    /// After it: a move to another place pushes the one before, a move back to the place on
    /// top of the history takes that off. A step back (Esc) is not a move.
    pub(crate) fn settings_record(&mut self, mark: Option<(Place, u64)>) {
        let Some((before, backs)) = mark else {
            return;
        };
        if self.modal != Some(Modal::Settings) || backs != self.settings.history.backs {
            return;
        }
        let now = self.settings_place();
        if now.same(&before) {
            return;
        }
        let h = &mut self.settings.history;
        if h.stack.last().is_some_and(|t| t.same(&now)) {
            h.stack.pop();
        } else {
            h.push(before);
        }
    }

    /// Runs `f`, recording the move it makes in Settings (changes made outside input
    /// events).
    pub(crate) fn settings_go(&mut self, f: impl FnOnce(&mut Chrome)) {
        let mark = self.settings_mark();
        f(self);
        self.settings_record(mark);
    }

    /// One step back in Settings: to the place before (a search is cleared back to where it
    /// started, a sub-view is left). False when there is nowhere to go back to.
    pub(crate) fn settings_back(&mut self) -> bool {
        let now = self.settings_place();
        self.settings.history.backs += 1;
        while let Some(p) = self.settings.history.stack.pop() {
            if !p.same(&now) {
                self.settings_restore(p);
                return true;
            }
        }
        // Nothing recorded (the history was cut short), but on the Keys page: to the page it
        // was reached from (once: `from` is taken, and a step back is not recorded).
        if now.view == View::Page && self.keys_return() {
            return true;
        }
        // Or searching or in a sub-view: to the page.
        if now.search.is_empty() && now.view == View::Page {
            return false;
        }
        let mut p = now;
        if let View::PanelConfirm(k) = p.view
            && p.focus.is_some()
        {
            p.focus = Some(Target::Settings(SettingsTarget::PanelAction(k)));
        }
        p.view = View::Page;
        if !p.search.is_empty() {
            p.search.clear();
            p.scroll = 0.0;
            p.focus = None;
            p.tool = None;
        }
        self.settings_restore(p);
        true
    }

    /// Shows place `p`: its section or search, sub-view, scroll offset and keyboard focus.
    /// A screen reader hears the section (or the search) it came back to.
    fn settings_restore(&mut self, p: Place) {
        let moved = p.section != self.settings.section
            || p.search.is_empty() != self.settings.search.is_empty();
        self.settings_show_view(p.view);
        let s = &mut self.settings;
        s.section = p.section;
        s.search = p.search;
        s.scroll = p.scroll;
        s.dropdown = None;
        s.scroll_to = None;
        s.start = None;
        s.reveal = 0;
        match p.tool {
            Some((row, part)) => {
                s.tools.focus = Some(row);
                s.tools.part = part;
                self.modal_focus = None;
            }
            None => {
                s.tools.focus = None;
                self.modal_focus = p.focus;
            }
        }
        // The offset is the page's as it was left: input must not clamp it to the range of
        // the page shown now, nor the next frame scroll it to the focus.
        self.bars.forget_range(Area::Settings);
        self.reveal_focus = false;
        if moved {
            let text = if self.settings.search.is_empty() {
                self.sections()
                    .get(self.settings.section)
                    .map(|sec| sec.label(I18n::global()))
            } else {
                Some(self.settings.search.clone())
            };
            if let Some(text) = text {
                self.announce(text);
            }
        }
    }

    /// One step back inside the dialog on top, short of closing it: Settings' history, or a
    /// typed filter cleared (the palette, the language list, the tag browser, which then
    /// closes the sequence the selection is in). False when there is none.
    pub(crate) fn dialog_back(&mut self) -> bool {
        match self.modal {
            Some(Modal::Settings) => self.settings_back(),
            Some(Modal::Palette) if !self.palette.query.is_empty() => {
                let p = &mut self.palette;
                p.query.clear();
                p.selected = 0;
                p.scroll = 0.0;
                p.dirty = true;
                true
            }
            Some(Modal::Language) if !self.language.query.is_empty() => {
                let l = &mut self.language;
                l.query.clear();
                l.selected = 0;
                l.scroll = 0.0;
                l.dirty = true;
                true
            }
            Some(Modal::DicomTags) => self.tags_back(),
            Some(Modal::Help) if !self.help.query.is_empty() => self.help.go_back(),
            _ => false,
        }
    }

    /// Esc in the tag browser: clears the filter (the selection stays on its row), then
    /// closes the sequence the selection is on or in, one level at a time.
    fn tags_back(&mut self) -> bool {
        let s = &mut self.dicom_tags;
        if !s.query.is_empty() {
            s.query.clear();
            s.dirty = true;
            s.scroll = f32::NAN;
            return true;
        }
        if s.dirty {
            s.rebuild();
        }
        let Some(&i) = s.visible.get(s.selected) else {
            return false;
        };
        if !s.expanded.contains(&i) {
            // To the sequence it is in: the nearest row above that is shallower.
            let d = s.sheet.rows[i].depth;
            let Some(p) = (0..s.selected)
                .rev()
                .find(|&k| s.sheet.rows[s.visible[k]].depth < d)
            else {
                return false;
            };
            s.selected = p;
        }
        let row = s.visible[s.selected];
        s.expanded.remove(&row);
        s.scroll = f32::NAN;
        s.rebuild();
        true
    }

    /// Opening dialog `m` over the open one: that one waits underneath, as it is, with its
    /// keyboard focus, and Esc comes back to it. Returns true when `m` was already waiting:
    /// the dialogs above it are closed instead.
    pub(crate) fn push_parent(&mut self, m: Modal) -> bool {
        if let Some(i) = self.modal_parents.iter().position(|(p, _)| *p == m) {
            if self.modal_parents[i + 1..]
                .iter()
                .any(|(p, _)| *p == Modal::DicomTags)
            {
                self.dicom_tags = Default::default();
            }
            if self.modal_parents[i + 1..]
                .iter()
                .any(|(p, _)| *p == Modal::Studies)
            {
                self.studies = Default::default();
            }
            self.modal_parents.truncate(i + 1);
            self.close_modal();
            return true;
        }
        if let Some(p) = self.modal {
            // Its dropdown and menus are transient: closed, not left open underneath.
            if p == Modal::Settings {
                self.close_dropdown();
                self.settings.tools.close_menu();
            }
            self.modal_parents.push((p, self.modal_focus.take()));
            if self.modal_parents.len() > PARENTS_MAX {
                self.modal_parents.remove(0);
            }
        }
        false
    }

    /// Alt+← or the mouse's Back button: one step back inside the open dialog, or to the
    /// dialog it was opened from, as Esc does, but it never closes the dialog the user
    /// opened first. True when a dialog is open (the press is its either way).
    pub fn back_in_dialog(&mut self) -> bool {
        if self.modal.is_none() {
            return false;
        }
        if self.settings.dropdown.is_some() {
            self.close_dropdown();
        } else if self.toolbar_back() || self.dialog_back() {
        } else if !self.modal_parents.is_empty() {
            self.close_modal();
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accesskit::{Action as AkAction, ActionRequest, Role, TreeId};
    use crate::settings::sections;
    use crate::{DicomTags, HudStats, Input, Overlays, Response, TagLine, ViewerState};
    use specular_gfx::{DrawList, Text};
    use specular_input::{ActionState, ImageState, Key, KeyCombo, Keymap, Mods, NamedKey};
    use specular_settings::{Settings, SettingsStore};
    use specular_theme::Palette;

    fn with_state(f: impl FnOnce(&ViewerState)) {
        let hud = HudStats::default();
        let keymap = Keymap::default();
        let store = SettingsStore::in_memory();
        let settings = Settings::default();
        let st = ViewerState {
            actions: ActionState {
                image: ImageState::Ready,
                count: 4,
                ..ActionState::default()
            },
            has_image: true,
            index: 1,
            count: 4,
            zoom: 1.0,
            at_fit: true,
            fullscreen: false,
            file_name: Some("beach.jpg"),
            dims: Some((4000, 3000)),
            loading: false,
            error: None,
            truncated: false,
            metadata: None,
            hud: &hud,
            keymap: &keymap,
            store: &store,
            settings: &settings,
            overlays: Overlays::default(),
            playback: None,
            motion: None,
            spread: None,
            pages: None,
        };
        f(&st)
    }

    fn chrome() -> Chrome {
        let mut c = Chrome::new();
        c.set_size(1000.0, 700.0);
        c.set_accessibility(true);
        c.poke(1.0);
        c
    }

    fn draw(c: &mut Chrome, text: &mut Text, st: &ViewerState) {
        let mut dl = DrawList::new(1.0);
        for i in 0..3 {
            dl.clear(1.0);
            c.draw(&mut dl, text, st, &Palette::dark(), 1.0 + i as f64 * 0.016);
        }
    }

    fn press(c: &mut Chrome, st: &ViewerState, k: NamedKey, mods: Mods) -> Response {
        c.input(
            &Input::Key {
                combo: KeyCombo::new(Key::Named(k), mods),
                repeat: false,
            },
            st,
            2.0,
        )
    }

    fn esc(c: &mut Chrome, st: &ViewerState) -> bool {
        press(c, st, NamedKey::Escape, Mods::NONE).consumed
    }

    /// A pointer click on `t` as last drawn.
    fn click(c: &mut Chrome, st: &ViewerState, t: Target) {
        let r = c
            .hits
            .iter()
            .rev()
            .find(|(_, h)| *h == t)
            .map(|(r, _)| *r)
            .unwrap_or_else(|| panic!("{t:?} is on screen"));
        let (x, y) = (r.x + r.w * 0.5, r.y + r.h * 0.5);
        for down in [true, false] {
            c.input(
                &Input::PointerButton {
                    x,
                    y,
                    down,
                    button: crate::Button::Primary,
                },
                st,
                2.0,
            );
        }
    }

    fn section(i: usize) -> Target {
        Target::Settings(SettingsTarget::Section(i))
    }

    fn label(i: usize) -> String {
        sections()[i].label(I18n::global())
    }

    fn announced(c: &Chrome) -> Option<String> {
        c.announce.as_ref().map(|a| a.1.clone())
    }

    /// The focused node of the accessibility tree: its role and name.
    fn a11y_focus(c: &Chrome, st: &ViewerState) -> (Role, Option<String>) {
        let t = c.accessibility_tree(st, "beach.jpg", 1.0);
        let (_, n) = t.nodes.iter().find(|(id, _)| *id == t.focus).unwrap();
        (n.role(), n.label().map(str::to_owned))
    }

    /// Section A → B → C, then Esc twice is back at A (its scroll offset and keyboard focus
    /// too, announced), and a further Esc closes Settings.
    #[test]
    fn sections_go_back_in_order_then_close() {
        let mut text = Text::new();
        with_state(|st| {
            let mut c = chrome();
            c.open_modal(Modal::Settings);
            // Short enough for the first page to scroll.
            c.set_size(1000.0, 320.0);
            draw(&mut c, &mut text, st);
            press(&mut c, st, NamedKey::Tab, Mods::NONE);
            draw(&mut c, &mut text, st);
            c.scroll_settings(40.0);
            draw(&mut c, &mut text, st);
            let (scroll_a, focus_a) = (c.settings.scroll, c.modal_focus.clone());
            assert!(
                scroll_a > 0.0 && focus_a.is_some(),
                "{scroll_a} {focus_a:?}"
            );
            let focused_a = a11y_focus(&c, st);

            // B by a screen reader's click on its tab.
            let tree = c.accessibility_tree(st, "beach.jpg", 1.0);
            let (id, _) = tree
                .nodes
                .iter()
                .find(|(_, n)| n.role() == Role::Tab && n.label() == Some(label(1).as_str()))
                .expect("section tab");
            c.accessibility_action(
                &ActionRequest {
                    action: AkAction::Click,
                    target_tree: TreeId::ROOT,
                    target_node: *id,
                    data: None,
                },
                st,
            );
            draw(&mut c, &mut text, st);
            // C by the pointer.
            click(&mut c, st, section(2));
            draw(&mut c, &mut text, st);
            assert_eq!(c.settings.section, 2);
            assert_eq!(c.settings.history.len(), 2);

            assert!(esc(&mut c, st));
            assert_eq!(c.settings.section, 1);
            assert_eq!(announced(&c), Some(label(1)));
            draw(&mut c, &mut text, st);
            assert!(esc(&mut c, st));
            assert_eq!(c.modal(), Some(Modal::Settings));
            assert_eq!(c.settings.section, 0);
            assert_eq!(announced(&c), Some(label(0)));
            assert_eq!(c.settings.scroll, scroll_a);
            assert_eq!(c.modal_focus, focus_a);
            draw(&mut c, &mut text, st);
            assert_eq!(c.settings.scroll, scroll_a, "drawn where it was left");
            assert_eq!(
                a11y_focus(&c, st),
                focused_a,
                "the screen reader's focus too"
            );

            assert!(esc(&mut c, st));
            assert_eq!(c.modal(), None);
        });
    }

    /// A → Keys → B: Esc walks the history back (B, Keys, A) and then closes; it never steps
    /// from Keys to B again (the page last drawn before Keys) and back, round and round.
    #[test]
    fn esc_from_keys_ends_at_the_start_of_the_history() {
        let mut text = Text::new();
        with_state(|st| {
            let keys = crate::settings::section_index(
                specular_settings::find(crate::keys_edit::KEY)
                    .expect("keys setting")
                    .section
                    .key(),
            )
            .expect("keys section");
            let b = (0..sections().len())
                .find(|&i| i != 0 && i != keys)
                .unwrap();
            let mut c = chrome();
            // Tall enough for every section in the sidebar (shorter windows scroll it).
            c.set_size(1000.0, 1000.0);
            c.open_modal(Modal::Settings);
            draw(&mut c, &mut text, st);
            click(&mut c, st, section(keys));
            draw(&mut c, &mut text, st);
            assert!(c.settings.keys.on_page);
            click(&mut c, st, section(b));
            draw(&mut c, &mut text, st);
            assert_eq!(c.settings.history.len(), 2);

            let mut seen = Vec::new();
            for _ in 0..8 {
                if c.modal() != Some(Modal::Settings) {
                    break;
                }
                esc(&mut c, st);
                draw(&mut c, &mut text, st);
                seen.push((c.modal() == Some(Modal::Settings)).then_some(c.settings.section));
            }
            assert_eq!(seen, [Some(keys), Some(0), None]);
        });
    }

    /// Reached without a recorded move (the history is empty), Esc on the Keys page still
    /// goes back to the page it came from, once; that step is not recorded, so the next Esc
    /// closes Settings rather than returning to Keys.
    #[test]
    fn esc_from_keys_without_a_history_goes_back_once() {
        let mut text = Text::new();
        with_state(|st| {
            let keys = crate::settings::section_index(
                specular_settings::find(crate::keys_edit::KEY)
                    .expect("keys setting")
                    .section
                    .key(),
            )
            .expect("keys section");
            let mut c = chrome();
            c.open_modal(Modal::Settings);
            draw(&mut c, &mut text, st);
            // Not through an input event: nothing recorded.
            c.settings.section = keys;
            draw(&mut c, &mut text, st);
            assert!(c.settings.keys.on_page);
            assert_eq!(c.settings.history.len(), 0);

            assert!(esc(&mut c, st));
            assert_eq!(c.settings.section, 0);
            assert_eq!(c.settings.history.len(), 0, "a step back is not a move");
            draw(&mut c, &mut text, st);
            assert!(esc(&mut c, st));
            assert_eq!(c.modal(), None);
        });
    }

    /// Typing a search records where it started (once, however long the query), Esc clears
    /// it back there, and Backspace to an empty field takes the entry off again.
    #[test]
    fn esc_clears_the_search_back_to_its_section() {
        let mut text = Text::new();
        with_state(|st| {
            let mut c = chrome();
            c.open_modal(Modal::Settings);
            draw(&mut c, &mut text, st);
            click(&mut c, st, section(2));
            draw(&mut c, &mut text, st);
            for s in ["z", "o", "om"] {
                c.input(&Input::Text(s.into()), st, 2.0);
            }
            draw(&mut c, &mut text, st);
            assert_eq!(c.settings.search, "zoom");
            assert_eq!(c.settings.history.len(), 2);

            assert!(esc(&mut c, st));
            assert_eq!(c.settings.search, "");
            assert_eq!(c.settings.section, 2);
            assert_eq!(announced(&c), Some(label(2)));
            assert_eq!(c.settings.history.len(), 1);

            // Typed and erased: nowhere new to go back to.
            c.input(&Input::Text("a".into()), st, 2.0);
            press(&mut c, st, NamedKey::Backspace, Mods::NONE);
            assert_eq!(c.settings.search, "");
            assert_eq!(c.settings.history.len(), 1);

            assert!(esc(&mut c, st));
            assert_eq!(c.settings.section, 0);
            assert!(esc(&mut c, st));
            assert_eq!(c.modal(), None);
        });
    }

    /// With a dropdown open, Esc closes only the dropdown.
    #[test]
    fn esc_closes_an_open_dropdown_first() {
        let mut text = Text::new();
        with_state(|st| {
            let mut c = chrome();
            c.open_modal(Modal::Settings);
            draw(&mut c, &mut text, st);
            click(&mut c, st, section(2));
            draw(&mut c, &mut text, st);
            let dd = c
                .hits
                .iter()
                .find(|(_, t)| matches!(t, Target::Settings(SettingsTarget::Dropdown(_))))
                .map(|(_, t)| t.clone())
                .expect("a dropdown on the page");
            click(&mut c, st, dd);
            assert!(c.settings.dropdown.is_some());
            assert_eq!(c.settings.history.len(), 1);

            assert!(esc(&mut c, st));
            assert_eq!(c.settings.dropdown, None);
            assert_eq!(c.modal(), Some(Modal::Settings));
            assert_eq!(c.settings.section, 2);
            assert_eq!(c.settings.history.len(), 1);
        });
    }

    /// A dialog opened from Settings returns to it with Esc, as it was left; Settings
    /// then goes on back through its own history.
    #[test]
    fn dialog_opened_from_settings_returns_to_it() {
        let mut text = Text::new();
        with_state(|st| {
            let mut c = chrome();
            c.open_modal(Modal::Settings);
            draw(&mut c, &mut text, st);
            click(&mut c, st, section(2));
            draw(&mut c, &mut text, st);
            press(&mut c, st, NamedKey::Tab, Mods::NONE);
            draw(&mut c, &mut text, st);
            let (scroll, focus) = (c.settings.scroll, c.modal_focus.clone());
            assert!(focus.is_some());

            c.show_report(crate::report::sample());
            assert_eq!(c.modal(), Some(Modal::HwReport));
            draw(&mut c, &mut text, st);

            assert!(esc(&mut c, st));
            assert_eq!(c.modal(), Some(Modal::Settings));
            assert_eq!(c.settings.section, 2);
            assert_eq!(c.settings.scroll, scroll);
            assert_eq!(c.modal_focus, focus);
            draw(&mut c, &mut text, st);

            // Its own history is intact: Esc goes on back to where it was opened.
            assert!(esc(&mut c, st));
            assert_eq!(c.modal(), Some(Modal::Settings));
            assert_eq!(c.settings.section, 0);
            assert!(esc(&mut c, st));
            assert_eq!(c.modal(), None);

            // Opened from the viewer, a dialog has nothing under it.
            c.show_report(crate::HwReport::default());
            assert!(esc(&mut c, st));
            assert_eq!(c.modal(), None);
        });
    }

    /// Opening Settings starts a fresh history.
    #[test]
    fn a_fresh_open_has_an_empty_history() {
        let mut text = Text::new();
        with_state(|st| {
            let mut c = chrome();
            c.open_modal(Modal::Settings);
            draw(&mut c, &mut text, st);
            click(&mut c, st, section(1));
            draw(&mut c, &mut text, st);
            click(&mut c, st, section(2));
            assert_eq!(c.settings.history.len(), 2);
            c.close_modal();
            c.open_modal(Modal::Settings);
            assert_eq!(c.settings.history.len(), 0);
            assert!(esc(&mut c, st));
            assert_eq!(c.modal(), None);
        });
    }

    /// The history keeps at most `HISTORY_MAX` places, never the same one twice in a row,
    /// and a move back to the place on top takes it off.
    #[test]
    fn the_history_is_bounded() {
        let mut c = chrome();
        c.open_modal(Modal::Settings);
        for i in 0..100 {
            c.settings_go(|c| c.settings.section = (i + 1) % 3);
        }
        let h = &c.settings.history.stack;
        assert_eq!(h.len(), HISTORY_MAX);
        assert!(h.windows(2).all(|w| !w[0].same(&w[1])));
        let top = h.last().unwrap().section;
        c.settings_go(|c| c.settings.section = top);
        assert_eq!(c.settings.history.len(), HISTORY_MAX - 1);
    }

    /// Alt+← steps back like Esc, and to the dialog underneath, but leaves the first dialog
    /// open at the start of its history.
    #[test]
    fn alt_left_never_closes_the_first_dialog() {
        with_state(|st| {
            let mut c = chrome();
            c.open_modal(Modal::Settings);
            c.settings_go(|c| c.settings.section = 2);
            let alt = Mods {
                alt: true,
                ..Mods::NONE
            };
            c.open_modal(Modal::Language);
            assert!(press(&mut c, st, NamedKey::ArrowLeft, alt).consumed);
            assert_eq!(c.modal(), Some(Modal::Settings));
            assert_eq!(c.settings.section, 2);
            assert!(press(&mut c, st, NamedKey::ArrowLeft, alt).consumed);
            assert_eq!(c.settings.section, 0);
            assert!(press(&mut c, st, NamedKey::ArrowLeft, alt).consumed);
            assert!(c.back_in_dialog());
            assert_eq!(c.modal(), Some(Modal::Settings));
            c.close_modal();
            assert!(!c.back_in_dialog());
        });
    }

    /// The palette and the language list clear a typed query before they close.
    #[test]
    fn esc_clears_a_typed_query_first() {
        with_state(|st| {
            for m in [Modal::Palette, Modal::Language] {
                let mut c = chrome();
                c.open_modal(m);
                c.input(&Input::Text("zo".into()), st, 2.0);
                assert!(!c.palette.query.is_empty() || !c.language.query.is_empty());
                assert!(esc(&mut c, st));
                assert_eq!(c.modal(), Some(m), "{m:?}");
                assert!(c.palette.query.is_empty() && c.language.query.is_empty());
                assert!(esc(&mut c, st));
                assert_eq!(c.modal(), None, "{m:?}");
            }
        });
    }

    /// The tag browser: Esc clears the filter, then closes the sequences the selection is
    /// in, one level at a time, then closes.
    #[test]
    fn tag_browser_clears_its_filter_then_collapses() {
        let line = |depth: u16, keyword: &str, descendants: u32| TagLine {
            depth,
            tag: "(0040,0275)".into(),
            keyword: keyword.into(),
            descendants,
            ..TagLine::default()
        };
        with_state(|st| {
            let mut c = chrome();
            c.show_dicom_tags(DicomTags {
                title: "x.dcm".into(),
                rows: vec![
                    line(0, "Modality", 0),
                    line(0, "RequestAttributesSequence", 2),
                    line(1, "Item 1", 1),
                    line(2, "ScheduledProcedureStepDescription", 0),
                ],
                ..DicomTags::default()
            });
            let s = &mut c.dicom_tags;
            s.rebuild();
            s.selected = 1;
            s.expand(Some(true));
            s.selected = 2;
            s.expand(Some(true));
            s.selected = 3;
            assert_eq!(s.visible, [0, 1, 2, 3]);

            c.input(&Input::Text("procedure".into()), st, 2.0);
            assert!(!c.dicom_tags.query.is_empty());
            assert!(esc(&mut c, st));
            assert_eq!(c.dicom_tags.query, "");
            assert_eq!(c.modal(), Some(Modal::DicomTags));
            assert_eq!(c.dicom_tags.expanded.len(), 2, "the tree as it was");
            // From the element deep inside: out of the item, then out of the sequence.
            c.dicom_tags.rebuild();
            c.dicom_tags.selected = 3;
            assert!(esc(&mut c, st));
            assert_eq!(c.dicom_tags.visible, [0, 1, 2]);
            assert_eq!(c.dicom_tags.selected, 2);
            assert!(esc(&mut c, st));
            assert_eq!(c.dicom_tags.visible, [0, 1]);
            assert_eq!(c.dicom_tags.selected, 1);
            assert!(esc(&mut c, st));
            assert_eq!(c.modal(), None);
        });
    }
}
