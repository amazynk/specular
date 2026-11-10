//! Gallery input: keys, clicks, rubber-band selection, dragging onto folders and out of the
//! window, the wheel (Ctrl: tile size about the pointer), pinch, and the bars' controls.

use specular_gfx::winit::event::KeyEvent;
use specular_gfx::winit::keyboard::{Key as WK, NamedKey as WN};
use specular_input::fixed::{self, GalleryCmd};
use specular_input::{Key, KeyCombo, NamedKey};
use specular_thumbs::layout::Nav;
use specular_widgets::Button;
use specular_widgets::line_edit::PendingClipboard;

use super::*;

/// Pointer travel (logical px) before a press becomes a band or a drag.
const DRAG_START: f32 = 6.0;
/// Type-to-jump: keys within this time extend the prefix (s).
const JUMP_S: f64 = 1.0;
/// Tile size step of Ctrl +/- and the bar buttons.
const SIZE_STEP: f32 = 1.25;

impl App {
    /// A key press while the gallery shows (no dialog open). True: the gallery used it.
    pub(in crate::app) fn gallery_key(&mut self, ev: &KeyEvent, combo: Option<&KeyCombo>) -> bool {
        if self.gallery.import.open {
            return self.import_key(ev, combo);
        }
        if self.gallery.people.open {
            let t = ev.text.as_ref().map(|t| t.to_string());
            if self.people_key(t.as_deref(), combo) {
                return true;
            }
        }
        if self.gallery.map.open {
            if self.map_key(combo) {
                return true;
            }
            // Escape and the like are the map's; the palette and Ctrl shortcuts go on.
        } else if self.gallery.similar.is_open() {
            return self.similar_key(combo);
        }
        let now = self.now();
        // The facets' search field takes its keys first (Tab too: on to the values).
        if self.gallery.facets_finding() && self.gallery.facets_find_key(ev, combo) {
            self.redraw = true;
            return true;
        }
        // F6 between the regions, Tab into and out of the "why is this slow?" indicator.
        if let Some(c) = combo
            && let Key::Named(n @ (NamedKey::Tab | NamedKey::F(6))) = c.key
            && !c.mods.ctrl
            && !c.mods.alt
            && !c.mods.meta
        {
            let bound = self.keymap.lookup(c).is_some();
            if self
                .gallery
                .region_key(&mut self.chrome, n, c.mods.shift, bound)
            {
                self.redraw = true;
                return true;
            }
        }
        // The indicator presses with Enter or Space and lets go with Escape (the chrome's
        // keys); any other key is the grid's again.
        if self.chrome.indicator_focused() {
            let plain = combo.is_some_and(|c| !c.mods.any());
            if plain
                && matches!(
                    combo.map(|c| c.key),
                    Some(Key::Named(
                        NamedKey::Enter | NamedKey::Space | NamedKey::Escape
                    ))
                )
            {
                return false;
            }
            self.chrome.blur();
            self.redraw = true;
        }
        if self.gallery.library.focus.is_some()
            && let Some(c) = combo
            && let Key::Named(n) = c.key
            && !c.mods.ctrl
            && !c.mods.alt
            && self.library_key(n, c.mods.shift)
        {
            self.redraw = true;
            return true;
        }
        if self.gallery.facets.focus.is_some()
            && let Some(c) = combo
            && let Key::Named(n) = c.key
            && !c.mods.ctrl
            && !c.mods.alt
            && self.gallery.facets_key(n, c.mods.shift)
        {
            self.redraw = true;
            return true;
        }
        // The search field's value picker takes its keys before the field.
        if self.gallery.filter_focus
            && let Some(c) = combo
            && let Key::Named(n) = c.key
            && self.gallery.picker_key(n, c.mods)
        {
            self.redraw = true;
            return true;
        }
        if self.gallery.filter_focus && self.gallery_filter_key(ev, combo) {
            self.redraw = true;
            return true;
        }
        // The Library's timeline: its keys while it has the keyboard; with the filter bar
        // closed, Tab from the grid goes to it (and back).
        if let Some(c) = combo
            && let Key::Named(n) = c.key
            && !c.mods.ctrl
            && !c.mods.alt
            && !c.mods.meta
        {
            let ga = &mut self.gallery;
            let used = if ga.stop() == super::filterbar::Stop::Timeline {
                ga.timeline_key(n, c.mods.shift)
            } else {
                n == NamedKey::Tab
                    && !ga.filter_open
                    && ga.timeline_h() > 0.0
                    && !ga.library.albums.bars.is_empty()
                    && self.keymap.lookup(c).is_none()
                    && {
                        ga.go(super::filterbar::Stop::Timeline);
                        true
                    }
            };
            if used {
                self.redraw = true;
                return true;
            }
        }
        // A button of the filter bar, or Tab from the grid into the bar.
        if self.gallery.filter_open
            && let Some(c) = combo
            && let Key::Named(n) = c.key
            && !c.mods.ctrl
            && !c.mods.alt
            && !c.mods.meta
        {
            let ga = &mut self.gallery;
            let used = if ga.stop() == super::filterbar::Stop::Grid {
                n == NamedKey::Tab && {
                    ga.tab(c.mods.shift);
                    true
                }
            } else {
                ga.bar_key(n, c.mods.shift)
            };
            if used {
                self.redraw = true;
                return true;
            }
        }
        if matches!(ev.logical_key, WK::Named(WN::ContextMenu)) {
            self.gallery_context_menu(None);
            return true;
        }
        let Some(combo) = combo else {
            return self.gallery_jump_key(ev, now);
        };
        let m = combo.mods;
        let plain = !m.ctrl && !m.alt && !m.meta;
        let used = match combo.key {
            // Shift+digit is a shortcut (the channel views, Shift+1…5), not a rating or label:
            // the action goes to quick look (`gallery_channel`).
            Key::Char(c)
                if plain
                    && m.shift
                    && c.is_ascii_digit()
                    && self.keymap.lookup(combo).is_some() =>
            {
                false
            }
            // `L` (Shift+l) is the Library's, not a jump to "L…".
            Key::Char(_)
                if plain
                    && m.shift
                    && matches!(
                        self.keymap.lookup(combo),
                        Some(
                            specular_input::Action::ToggleLibrary
                                | specular_input::Action::Keywords
                                | specular_input::Action::EditMetadata
                        )
                    ) =>
            {
                false
            }
            // A jump in progress takes letters that are shortcuts too.
            Key::Char(_)
                if plain
                    && !self.gallery.jump.0.is_empty()
                    && now - self.gallery.jump.1 < JUMP_S =>
            {
                self.gallery_jump_key(ev, now)
            }
            Key::Char(_) if plain => {
                self.gallery_fixed_key(combo, ev.repeat) || self.gallery_jump_key(ev, now)
            }
            _ => self.gallery_fixed_key(combo, ev.repeat),
        };
        if used {
            self.redraw = true;
        }
        used
    }

    /// Actions (keys, the palette) while the gallery shows. True: handled or swallowed here
    /// (viewer actions do nothing in the gallery); false: the usual handling.
    pub(in crate::app) fn gallery_action(&mut self, a: specular_input::Action) -> bool {
        use specular_input::Action as A;
        if a == A::ToggleGallery {
            self.gallery_toggle();
            return true;
        }
        if a == A::ToggleLibrary {
            self.library_toggle();
            return true;
        }
        if !self.gallery.active {
            return false;
        }
        // Another action (trash, copy, the palette's) closes the Properties card, as a key does.
        if a != A::Back && self.gallery.info.take().is_some() {
            self.redraw = true;
        }
        match a {
            A::Back => {
                self.gallery_escape();
            }
            A::MoveToTrash => self.gallery_trash(),
            A::CopyImage => self.gallery_copy_image(),
            A::CopyPath => self.gallery_copy_paths(),
            A::Reload if self.gallery.library.on => self.library_rescan(),
            A::Reload => {
                if let Some(d) = self.gallery.dir.clone() {
                    self.gallery_relist(d);
                }
            }
            A::NextImage => self.gallery_nav_wrap(Nav::Right, false, false, true),
            A::PrevImage => self.gallery_nav_wrap(Nav::Left, false, false, true),
            A::FirstImage => self.gallery_nav(Nav::Home, false, false),
            A::LastImage => self.gallery_nav(Nav::End, false, false),
            A::ZoomIn => self.gallery_resize(self.gallery.view.size * SIZE_STEP),
            A::ZoomOut => self.gallery_resize(self.gallery.view.size / SIZE_STEP),
            A::Undo
            | A::Open
            | A::OpenWith
            | A::OpenAs
            | A::Paste
            | A::Settings
            | A::Palette
            | A::CheatSheet
            | A::About
            | A::Hud
            | A::Language
            | A::ToggleTheme
            | A::InterfaceLarger
            | A::InterfaceSmaller
            | A::InterfaceReset
            | A::ToggleFullscreen
            | A::Close
            | A::Quit => return false,
            _ => {}
        }
        true
    }

    /// A channel action (Shift+1…5, the palette) while the gallery shows. Quick look and
    /// compare show the channel views, like the viewer (they are one setting, shared, so in
    /// compare one choice applies to every image); the grid does not, so there the action does
    /// nothing rather than change the viewer behind it unseen. True: the gallery took it.
    pub(in crate::app) fn gallery_channel(&mut self, a: specular_input::Action) -> bool {
        use specular_input::Action as A;
        let channel = matches!(
            a,
            A::ChannelRed | A::ChannelGreen | A::ChannelBlue | A::ChannelAlpha | A::ChannelLuma
        );
        if !channel || !self.gallery.active {
            return false;
        }
        let alpha = self.gallery_channel_alpha();
        if channel_applies(a == A::ChannelAlpha, &alpha) {
            self.chrome.handle_action(a);
        }
        self.redraw = true;
        true
    }

    /// For the channel actions' enabled state: while the gallery shows images their views
    /// apply to (quick look, compare, the similar-shots review), whether one of them may have
    /// alpha; None otherwise (the viewer's image decides).
    pub(in crate::app) fn gallery_channel_views(&self) -> Option<bool> {
        if !self.gallery.active {
            return None;
        }
        let alpha = self.gallery_channel_alpha();
        (!alpha.is_empty()).then(|| channel_applies(true, &alpha))
    }

    /// Per image the gallery shows in a channel view (quick look, compare, the similar-shots
    /// review's large images): whether it has alpha, when known.
    fn gallery_channel_alpha(&self) -> Vec<Option<bool>> {
        let ga = &self.gallery;
        // The similar-shots review's large images (none in its list of groups).
        let review: Vec<PathBuf> = match ga.similar.is_open() {
            true => ga
                .similar
                .review_panes(ga.vw, ga.vh)
                .map_or_else(Vec::new, |r| r.members.into_iter().map(|m| m.0).collect()),
            false => Vec::new(),
        };
        let shown: &[PathBuf] = match (&ga.compare, &ga.quick) {
            _ if ga.similar.is_open() => &review,
            (Some(c), _) => c,
            (None, Some(q)) => std::slice::from_ref(q),
            (None, None) => &[],
        };
        // As in the viewer: no alpha view when every image shown is known to have no alpha.
        shown
            .iter()
            .map(|p| {
                self.by_path
                    .get(p)
                    .and_then(|id| self.images.get(id))
                    .and_then(|i| i.info.as_ref())
                    .map(|i| i.has_alpha)
            })
            .collect()
    }

    /// Something else opens in the viewer (a dropped file, the open dialog): the gallery
    /// fades out.
    pub(in crate::app) fn gallery_dismiss(&mut self) {
        if self.gallery.active {
            let now = self.now();
            self.gallery_close(now, None);
        }
    }

    /// A key release (the quick-look key).
    pub(in crate::app) fn gallery_key_up(&mut self, ev: &KeyEvent) {
        // A selection made in the filter field with the keys is offered now.
        if self.gallery.filter_primary_pending() {
            self.filter_primary();
        }
        if self.gallery.quick_held
            && matches!(&ev.logical_key, WK::Character(s) if s.eq_ignore_ascii_case("q"))
        {
            self.gallery.quick_held = false;
            self.gallery.quick = None;
            self.redraw = true;
        }
    }

    /// The gallery's own keys (and compare's while it is open), from the tables the cheat
    /// sheet lists ([`fixed::GALLERY_KEYS`], [`fixed::COMPARE_KEYS`]). True: used.
    fn gallery_fixed_key(&mut self, combo: &KeyCombo, repeat: bool) -> bool {
        if self.gallery.compare.is_some()
            && let Some(l) = fixed::find(fixed::COMPARE_KEYS, combo)
        {
            // A held key does it once.
            if repeat && !l.repeats {
                return true;
            }
            if self.compare_cmd(l.cmd, combo) {
                return true;
            }
        }
        let Some(l) = fixed::find(fixed::GALLERY_KEYS, combo) else {
            return false;
        };
        if repeat && !l.repeats {
            // Once per press; quick look stays open while its key is held.
            return l.cmd == GalleryCmd::QuickLook;
        }
        self.gallery_cmd(l.cmd, combo, repeat)
    }

    /// Runs a command of the gallery's own keys; `combo` is the key that ran it (the arrow,
    /// the digit) with the modifiers the line left free.
    fn gallery_cmd(&mut self, cmd: GalleryCmd, combo: &KeyCombo, repeat: bool) -> bool {
        use GalleryCmd as G;
        // The Properties card is about what was selected: another key closes it and goes on.
        if !matches!(cmd, G::Properties | G::Back) && self.gallery.info.take().is_some() {
            self.redraw = true;
        }
        let m = combo.mods;
        let digit = match combo.key {
            Key::Char(c) => c.to_digit(10).map(|d| d as u8),
            Key::Named(_) => None,
        };
        match cmd {
            G::Move | G::Page | G::Ends => {
                let Key::Named(n) = combo.key else {
                    return false;
                };
                let rtl = self.gallery.rtl;
                let nav = match n {
                    NamedKey::ArrowLeft if rtl => Nav::Right,
                    NamedKey::ArrowLeft => Nav::Left,
                    NamedKey::ArrowRight if rtl => Nav::Left,
                    NamedKey::ArrowRight => Nav::Right,
                    NamedKey::ArrowUp => Nav::Up,
                    NamedKey::ArrowDown => Nav::Down,
                    NamedKey::PageUp => Nav::PageUp,
                    NamedKey::PageDown => Nav::PageDown,
                    NamedKey::Home => Nav::Home,
                    NamedKey::End => Nav::End,
                    _ => return false,
                };
                // Compare takes ←/→ (its own keys); the grid behind it stays put.
                if self.gallery.compare.is_some() {
                    return true;
                }
                // Going round the end takes a fresh press, so holding a key stops there.
                self.gallery_nav_wrap(nav, m.shift, m.ctrl, !repeat && !m.shift);
            }
            G::Up => self.gallery_up(),
            G::HistoryBack => {
                self.gallery_history(false);
            }
            G::HistoryForward => {
                self.gallery_history(true);
            }
            G::Back => return self.gallery_escape(),
            G::Open => {
                if let Some(i) = self.gallery.focus_index() {
                    self.gallery_open(i);
                }
            }
            G::NewWindow => self.gallery_new_window(),
            G::Properties => self.gallery_properties(),
            G::Viewer => self.gallery_toggle(),
            G::NextUnrated => self.gallery_next_unrated(),
            G::Select => {
                let ga = &mut self.gallery;
                if ga.quick.is_some() && !ga.quick_held {
                    ga.quick = None;
                } else if ga.pointer_input_recent()
                    && let Some((i, _)) = ga.hover
                    && let Some(p) = ga.path_of(i)
                {
                    // Space over a tile: quick look at it (Space again closes).
                    ga.focus = Some(p.clone());
                    self.gallery_quick(p, false);
                } else if let Some(p) = ga.focus.clone() {
                    if !ga.selected.remove(&p) {
                        ga.selected.insert(p.clone());
                    }
                    ga.anchor = Some(p);
                }
            }
            G::SelectAll => self.gallery_select_all(),
            G::SelectNone => self.gallery.selected.clear(),
            G::InvertSelection => self.gallery_code(code::INVERT),
            G::Rate | G::RateNone => {
                let r = if cmd == G::RateNone {
                    0
                } else {
                    digit.unwrap_or(0)
                };
                self.gallery_mark(|m| Mark { rating: r, ..m });
            }
            G::Label => {
                let l = digit.unwrap_or(5).saturating_sub(5);
                let all = self
                    .gallery
                    .targets()
                    .iter()
                    .all(|p| self.gallery.mark(p).label == l);
                let l = if all { 0 } else { l };
                self.gallery_mark(|m| Mark { label: l, ..m });
            }
            G::Pick => self.gallery_mark(|m| Mark { flag: 1, ..m }),
            G::Reject => self.gallery_mark(|m| Mark { flag: -1, ..m }),
            G::Unflag => self.gallery_mark(|m| Mark { flag: 0, ..m }),
            G::AlbumAdd => self.albums_add_key(false),
            G::AlbumChoose => self.albums_add_key(true),
            G::QuickLook => {
                let p = self.gallery.focus.clone().or_else(|| {
                    let ga = &self.gallery;
                    ga.hover.and_then(|(i, _)| ga.path_of(i))
                });
                if let Some(p) = p {
                    self.gallery_quick(p, true);
                }
            }
            G::Compare => {
                if self.gallery.compare.is_some() {
                    self.gallery.compare = None;
                } else {
                    self.gallery_compare();
                }
            }
            G::Stack => self.gallery_toggle_stack(),
            // Ctrl+F (⌘F) toggles: it closes the search it is typing in, like the bar's
            // search button; elsewhere it opens the search or puts the keyboard in it.
            G::Filter if m.ctrl && self.gallery.filter_open && self.gallery.filter_focus => {
                self.gallery.close_filter();
            }
            G::Filter => self.gallery_code(code::FILTER),
            G::Bigger => self.gallery_resize(self.gallery.view.size * SIZE_STEP),
            G::Smaller => self.gallery_resize(self.gallery.view.size / SIZE_STEP),
            G::DefaultSize => self.gallery_resize(160.0),
            G::Trash => self.gallery_trash(),
            G::CopyImage => self.gallery_copy_image(),
            G::CopyPaths => self.gallery_copy_paths(),
            G::Refresh if self.gallery.library.on => self.library_rescan(),
            G::Refresh => {
                if let Some(d) = self.gallery.dir.clone() {
                    self.gallery_relist(d);
                }
            }
            G::ContextMenu => self.gallery_context_menu(None),
        }
        true
    }

    /// Shows quick look for `p` (`held`: until the key is released).
    fn gallery_quick(&mut self, p: PathBuf, held: bool) {
        let is_image = self
            .gallery
            .by_path
            .get(p.as_path())
            .is_some_and(|&i| self.gallery.entries[i].kind == EntryKind::Image);
        if !is_image {
            return;
        }
        self.gallery.quick = Some(p.clone());
        self.gallery.quick_held = held;
        self.gallery.wish_key = 0;
        self.redraw = true;
    }

    /// Esc (and the Back action) in the grid: see [`escape_step`]. Menus, dialogs and a focused
    /// filter field take Esc before this; key repeats never get here (one level per press).
    /// Always handled, so Esc in the gallery never falls through to another binding (quit).
    fn gallery_escape(&mut self) -> bool {
        if self.gallery.info.take().is_some() {
            self.redraw = true;
            return true;
        }
        let ga = &mut self.gallery;
        let step = escape_step(EscapeState {
            quick: ga.quick.is_some(),
            compare: ga.compare.is_some(),
            facets: ga.facets.open,
            empty_filter_open: ga.filter_open && ga.filter.is_empty(),
            has_parent: ga.dir.as_deref().and_then(Path::parent).is_some(),
            selection: !ga.selected.is_empty(),
        });
        match step {
            EscapeStep::CloseQuickLook => {
                ga.quick = None;
                ga.quick_held = false;
            }
            EscapeStep::CloseCompare => ga.compare = None,
            EscapeStep::CloseFacets => ga.facets.close(),
            EscapeStep::CloseFilter => {
                ga.filter_open = false;
                ga.filter_focus = false;
            }
            EscapeStep::Up => self.gallery_up(),
            EscapeStep::ClearSelection => ga.selected.clear(),
            EscapeStep::Home => self.welcome_close_all(),
        }
        true
    }

    /// Typing in the filter field.
    fn gallery_filter_key(&mut self, ev: &KeyEvent, combo: Option<&KeyCombo>) -> bool {
        let now = self.now();
        let ga = &mut self.gallery;
        let named = combo.and_then(|c| match c.key {
            Key::Named(n) => Some((n, c.mods)),
            Key::Char(_) => None,
        });
        // While the input method composes, its editing keys are the composition's.
        if named.is_some() && !ga.preedit.is_empty() {
            return true;
        }
        match named {
            Some((NamedKey::Escape, _)) => {
                if ga.filter.text.is_empty() {
                    ga.filter_focus = false;
                    if ga.filter.is_empty() {
                        ga.filter_open = false;
                    }
                } else {
                    ga.filter.text.clear();
                    ga.dirty = true;
                }
                return true;
            }
            Some((NamedKey::Tab, m)) if !m.ctrl && !m.alt => {
                ga.tab(m.shift);
                return true;
            }
            Some((NamedKey::Enter | NamedKey::ArrowDown | NamedKey::Tab, _)) => {
                ga.filter_focus = false;
                if ga.focus_index().is_none() && !ga.tiles.is_empty() {
                    ga.set_focus(0);
                }
                return true;
            }
            _ => {}
        }
        // A space is text (the picker took Space when a row of it was active).
        let space = matches!(named, Some((NamedKey::Space, m)) if !m.ctrl && !m.alt && !m.meta);
        // The keys of a text field: caret and selection, editing, clipboard, undo.
        if let Some(c) = combo {
            let mut clip = PendingClipboard::default();
            if ga.filter_key(c.key, c.mods, &mut clip).used() {
                // Offered once the keys are let go or rest (not on every step of a held
                // Shift+Arrow): see `gallery_key_up` and `filter_primary_tick`.
                if specular_widgets::line_edit::PRIMARY_SELECTION {
                    ga.filter_offer_primary_later(now);
                }
                self.field_clipboard(clip, crate::app::integration::PasteTo::GalleryFilter);
                self.redraw = true;
                return true;
            }
        }
        if named.is_some() && !space {
            return false;
        }
        if self.mods.control_key() || self.mods.alt_key() || self.mods.super_key() {
            return false;
        }
        let Some(t) = &ev.text else { return false };
        let s: String = t.chars().filter(|c| !c.is_control()).collect();
        if s.is_empty() {
            return false;
        }
        self.last_key_text = Some((s.clone(), Instant::now()));
        self.gallery_filter_text(&s);
        true
    }

    /// Text into the filter field (keys and IME commits).
    pub(in crate::app) fn gallery_filter_text(&mut self, s: &str) {
        if self.gallery.import_wants_text() {
            self.import_text(s);
            return;
        }
        if self.gallery.facets_finding() {
            self.gallery.facets_find_text(s);
            self.redraw = true;
            return;
        }
        self.gallery.filter_insert(s);
        self.redraw = true;
    }

    /// True while the filter field takes text (IME commits go to it).
    pub(in crate::app) fn gallery_wants_text(&self) -> bool {
        self.gallery.active
            && (self.gallery.filter_focus
                || self.gallery.facets_finding()
                || self.gallery.import_wants_text())
    }

    /// An IME composition (empty: none) for the focused field: the chrome's dialog or go-to
    /// field first, else the gallery's filter field.
    pub(in crate::app) fn ime_preedit(&mut self, s: String) {
        // The app's sheets take the keys over the chrome: the composition goes to the one
        // whose field has the keyboard (each clears its own otherwise).
        self.thermal_report.set_preedit(&s);
        self.web_gallery.set_preedit(&s);
        self.fileops.set_preedit(&s);
        self.convert.set_preedit(&s);
        self.export_recipes.set_preedit(&s);
        self.dicomweb.set_preedit(&s);
        self.omero.set_preedit(&s);
        self.address.set_preedit(&s);
        self.figure.set_preedit(&s);
        self.bim_set_preedit(&s);
        self.markup.set_preedit(&s);
        self.mpr_rename_preedit(&s);
        self.gis_set_preedit(&s);
        let sheet = self.thermal_report_wants_text()
            || self.web_gallery.wants_text()
            || self.fileops.wants_text()
            || self.export_recipes.wants_text()
            || self.convert.wants_text()
            || self.dicomweb.wants_text()
            || self.omero.wants_text()
            || self.address.wants_text()
            || self.figure.wants_text()
            || self.bim_wants_text()
            || self.markup.wants_text()
            || self.mpr_renaming()
            || self.gis_wants_text();
        if sheet {
            self.gallery.preedit.clear();
            self.chrome_input(specular_widgets::Input::Preedit(String::new()));
            self.redraw = true;
            return;
        }
        let gallery = !self.chrome.wants_text() && self.gallery_wants_text();
        if gallery {
            self.gallery.preedit.clone_from(&s);
            self.chrome_input(specular_widgets::Input::Preedit(String::new()));
        } else {
            self.gallery.preedit.clear();
            self.chrome_input(specular_widgets::Input::Preedit(s));
        }
        self.redraw = true;
    }

    /// Type to jump: the first tile whose name starts with what was typed.
    fn gallery_jump_key(&mut self, ev: &KeyEvent, now: f64) -> bool {
        if self.mods.control_key() || self.mods.alt_key() || self.mods.super_key() {
            return false;
        }
        let Some(t) = &ev.text else { return false };
        let s: String = t.chars().filter(|c| !c.is_control()).collect();
        if s.is_empty() || s == " " {
            return false;
        }
        let ga = &mut self.gallery;
        if now - ga.jump.1 > JUMP_S {
            ga.jump.0.clear();
        }
        ga.jump.0.push_str(&s);
        ga.jump.1 = now;
        let prefix = ga.jump.0.clone();
        // The same letter again steps through the tiles with that letter.
        let repeat = prefix.chars().count() > 1 && prefix.chars().all(|c| prefix.starts_with(c));
        let (needle, from) = if repeat {
            (
                prefix.chars().next().map(String::from).unwrap_or_default(),
                ga.focus_index().map_or(0, |i| i + 1),
            )
        } else {
            (prefix, ga.focus_index().unwrap_or(0))
        };
        let n = ga.tiles.len();
        let hit = (0..n).map(|k| (from + k) % n.max(1)).find(|&i| {
            ga.entry_of(i)
                .is_some_and(|e| model::starts_with_folded(&e.name, &needle))
        });
        if let Some(i) = hit {
            ga.set_focus(i);
            ga.anchor = ga.focus.clone();
        }
        self.redraw = true;
        true
    }

    /// Moves the focus (Shift: extends the selection from the anchor).
    pub(super) fn gallery_nav(&mut self, nav: Nav, extend: bool, keep: bool) {
        self.gallery_nav_wrap(nav, extend, keep, false);
    }

    /// [`Self::gallery_nav`], going round at the ends when `wrap` and the setting allow it.
    pub(super) fn gallery_nav_wrap(&mut self, nav: Nav, extend: bool, keep: bool, wrap: bool) {
        let wrap = wrap && self.settings.gallery.wrap_around;
        let ga = &mut self.gallery;
        ga.pointer_input = false;
        if ga.tiles.is_empty() {
            return;
        }
        let vh = ga.grid_h();
        let i = match ga.focus_index() {
            Some(i) if wrap => ga.layout.nav_wrapping(i, nav, vh),
            Some(i) => ga.layout.nav(i, nav, vh),
            // The first key shows where the focus is: the first tile in view.
            None => ga
                .layout
                .items_in(ga.scroll, ga.scroll + vh)
                .start
                .min(ga.tiles.len() - 1),
        };
        ga.set_focus(i);
        if extend {
            let a = ga.anchor.as_ref().and_then(|p| ga.tile_of(p)).unwrap_or(i);
            let (lo, hi) = (a.min(i), a.max(i));
            ga.selected = (lo..=hi).filter_map(|k| ga.path_of(k)).collect();
        } else if !keep {
            ga.anchor = ga.focus.clone();
        }
        if ga.quick.is_some()
            && let Some(p) = ga.focus.clone()
        {
            let held = ga.quick_held;
            self.gallery_quick(p, held);
        }
    }

    /// New tile size (logical px), keeping the tile under the pointer (or the focus) in place.
    pub(in crate::app) fn gallery_resize(&mut self, size: f32) {
        let ga = &mut self.gallery;
        if ga.view.mode == Mode::Details {
            return;
        }
        let s = size.clamp(model::MIN_SIZE, model::MAX_SIZE);
        if (s - ga.view.size).abs() < 0.5 {
            return;
        }
        ga.view.size = s;
        self.gallery_view_changed();
        self.redraw = true;
    }

    // ----- Pointer --------------------------------------------------------------------------

    /// Pointer motion (logical px).
    pub(in crate::app) fn gallery_pointer_move(&mut self, p: (f32, f32)) {
        let now = self.now();
        // Under the right-click menu no tile is hovered: its tooltip would come up the moment
        // the menu went, for a tile the pointer only crossed on the way to a row.
        let covered = self.chrome.menu_shown();
        let ga = &mut self.gallery;
        let moved = ga
            .pointer
            .is_none_or(|q| (q.0 - p.0).abs() + (q.1 - p.1).abs() > 0.5);
        ga.pointer = Some(p);
        if moved {
            ga.pointer_input = true;
        }
        self.redraw = true;
        if ga.import.open {
            self.import_pointer_move(p);
            return;
        }
        if ga.people.open {
            self.people_pointer_move(p);
            return;
        }
        if ga.map.open {
            self.map_pointer_move(p);
            return;
        }
        if ga.similar.is_open() {
            return;
        }
        if ga.compare.is_some() && self.compare_pointer_move(p) {
            return;
        }
        let ga = &mut self.gallery;
        if ga.slider_drag {
            self.gallery_slider_to(p.0);
            return;
        }
        if ga.filter_drag(p.0) {
            return;
        }
        if let Some(grab) = ga.scrollbar_drag {
            ga.scroll_bar_to(p.1, grab);
            return;
        }
        if let Some(grab) = ga.picker.drag {
            ga.picker_drag_to(p.1, grab);
            return;
        }
        // Hover: tooltip and full-decode prefetch after a moment.
        let over = if !covered
            && ga.press.is_none()
            && ga.quick.is_none()
            && ga.compare.is_none()
            && !ga.facets.rect.is_some_and(|r| r.contains(p))
            && !ga.picker.rect.is_some_and(|r| r.contains(p))
            && !ga
                .info
                .as_ref()
                .and_then(|c| c.rect)
                .is_some_and(|r| r.contains(p))
            && !ga.sel_bar.as_ref().is_some_and(|b| b.rect.contains(p))
        {
            ga.tile_at(p)
        } else {
            None
        };
        if over != ga.hover.map(|h| h.0) {
            ga.hover = over.map(|i| (i, now));
        }
        let Some(mut press) = ga.press.take() else {
            return;
        };
        let dist = (p.0 - press.at.0).abs().max((p.1 - press.at.1).abs());
        if !press.moved && dist >= DRAG_START {
            press.moved = true;
            match press.tile {
                Some(_) => press.drag = true,
                None => press.band = true,
            }
        }
        if press.band {
            // Scroll when the band reaches an edge.
            let top = ga.grid_top();
            let bottom = top + ga.grid_h();
            if p.1 < top {
                ga.scroll_target -= (top - p.1).min(60.0);
            } else if p.1 > bottom {
                ga.scroll_target += (p.1 - bottom).min(60.0);
            }
            ga.scroll_target = ga.layout.clamp_scroll(ga.scroll_target, ga.grid_h());
            let y0 = press.content_y;
            let y1 = p.1 - top + ga.scroll_target;
            let (mut x0, mut x1) = (press.at.0.min(p.0), press.at.0.max(p.0));
            if ga.rtl {
                (x0, x1) = (ga.vw - x1, ga.vw - x0);
            }
            let hit = ga.layout.items_in_rect(x0, y0.min(y1), x1, y0.max(y1));
            let mut sel = press.base.clone();
            for i in hit {
                if let Some(path) = ga.path_of(i) {
                    sel.insert(path);
                }
            }
            ga.selected = sel;
        }
        let drag = press.drag;
        ga.press = Some(press);
        // A drag that leaves the window becomes a drag to other applications.
        if drag && let Some(g) = &self.gfx {
            let s = g.window.inner_size();
            let sc = self.dl.scale.max(0.1);
            let (w, h) = (s.width as f32 / sc, s.height as f32 / sc);
            if p.0 < 0.0 || p.1 < 0.0 || p.0 >= w || p.1 >= h {
                let files: Vec<PathBuf> = self
                    .gallery
                    .targets()
                    .into_iter()
                    .filter(|f| f.is_file())
                    .collect();
                self.gallery.press = None;
                if !files.is_empty() {
                    self.drag_out_files(&files);
                }
            }
        }
    }

    /// The pointer left the window.
    pub(in crate::app) fn gallery_pointer_leave(&mut self) {
        let ga = &mut self.gallery;
        ga.hover = None;
        if ga.press.as_ref().is_none_or(|p| !p.drag) {
            ga.pointer = None;
        }
        self.redraw = true;
    }

    /// A mouse button in the gallery (logical px). True: the gallery used it.
    pub(in crate::app) fn gallery_button(&mut self, down: bool, b: Button, p: (f32, f32)) -> bool {
        let now = self.now();
        self.gallery.pointer = Some(p);
        self.gallery.pointer_input = true;
        self.redraw = true;
        if self.gallery.import.open {
            self.import_button_press(down, b, p);
            return true;
        }
        if self.gallery.people.open {
            if b == specular_widgets::Button::Primary {
                self.people_button(down, p);
            }
            return true;
        }
        if self.gallery.map.open {
            self.map_button(down, b, p);
            return true;
        }
        if self.gallery.similar.is_open() {
            self.similar_button(down, b, p);
            return true;
        }
        if self.gallery.compare.is_some() {
            return self.compare_button(down, b, p);
        }
        if !down {
            return self.gallery_release(b, p, now);
        }
        // A press elsewhere takes the keyboard from the timeline (one on a bar keeps it there).
        if self.gallery.library.albums.kb.is_some() {
            let k = match self.gallery.hit_at(p) {
                Some(Hit::Library(super::library::Side::Bar(k))) => Some(k),
                _ => None,
            };
            self.gallery.library.albums.kb = k;
        }
        if b == Button::Primary && (self.keywords_press(p) || self.meta_press(p)) {
            return true;
        }
        // The Properties card: a click on it stays there, its close button or a click
        // anywhere else closes it (and does nothing else, as a popover's would).
        if self.gallery.info.is_some() {
            if self.gallery.hit_at(p) != Some(Hit::InfoCard) {
                self.gallery.info = None;
            }
            return true;
        }
        // Quick look and compare cover the grid: a click closes them.
        if self.gallery.quick.is_some() || self.gallery.compare.is_some() {
            if b == Button::Primary && self.gallery.compare.is_some() {
                // A click on an image of the comparison focuses it; a double click opens it.
                return true;
            }
            self.gallery.quick = None;
            self.gallery.quick_held = false;
            self.gallery.compare = None;
            return true;
        }
        // A click away from the search field and its picker closes the picker.
        let hit = self.gallery.hit_at(p);
        // Anywhere but on it, a click takes the keyboard from the facets' search.
        if hit != Some(Hit::FacetsFind) {
            self.gallery.facets.find_focus = false;
        }
        if !matches!(
            hit,
            Some(
                Hit::FilterField
                    | Hit::PickerRow(..)
                    | Hit::PickerOp(_)
                    | Hit::PickerPanel
                    | Hit::PickerScroll
            )
        ) {
            self.gallery.picker_dismiss();
        }
        if let Some(h) = hit {
            if b == Button::Primary {
                self.gallery_hit(h, p);
            } else if b == Button::Middle && matches!(h, Hit::FilterField) {
                self.filter_middle_click(p.0);
            }
            return true;
        }
        let ga = &mut self.gallery;
        if p.1 < ga.grid_top() || p.1 > ga.grid_top() + ga.grid_h() {
            // The bars' empty parts.
            return true;
        }
        ga.filter_focus = false;
        ga.facets.focus = None;
        ga.bar.focus = None;
        let tile = ga.tile_at(p);
        match b {
            Button::Secondary => {
                match tile.and_then(|i| ga.path_of(i)) {
                    Some(path) => {
                        if !ga.selected.contains(&path) {
                            ga.selected.clear();
                        }
                        ga.focus = Some(path);
                        self.gallery_context_menu(Some(p));
                    }
                    None => self.gallery_menu_view(Some(p)),
                }
                return true;
            }
            Button::Middle => {
                if let Some(path) = tile.and_then(|i| ga.path_of(i)) {
                    ga.focus = Some(path);
                    self.gallery_new_window();
                }
                return true;
            }
            Button::Primary => {}
        }
        // Double click opens.
        if let (Some(i), Some((t, j))) = (tile, ga.last_click)
            && i == j
            && now - t < crate::app::DOUBLE_CLICK_S
        {
            ga.last_click = None;
            ga.press = None;
            self.gallery_open(i);
            return true;
        }
        ga.last_click = tile.map(|i| (now, i));
        let ctrl = self.mods.control_key() || self.mods.super_key();
        let shift = self.mods.shift_key();
        let content_y = p.1 - ga.grid_top() + ga.scroll;
        let mut base = HashSet::new();
        if let Some(i) = tile {
            let path = ga.path_of(i).expect("tile in range");
            if shift {
                let a = ga.anchor.as_ref().and_then(|q| ga.tile_of(q)).unwrap_or(i);
                let (lo, hi) = (a.min(i), a.max(i));
                let range: Vec<PathBuf> = (lo..=hi).filter_map(|k| ga.path_of(k)).collect();
                if ctrl {
                    ga.selected.extend(range);
                } else {
                    ga.selected = range.into_iter().collect();
                }
            } else if ctrl {
                if !ga.selected.remove(&path) {
                    ga.selected.insert(path.clone());
                }
                ga.anchor = Some(path.clone());
            } else {
                ga.anchor = Some(path.clone());
            }
            ga.focus = Some(path);
        } else {
            if ctrl || shift {
                base = ga.selected.clone();
            } else {
                ga.selected.clear();
            }
        }
        ga.press = Some(Press {
            at: p,
            content_y,
            tile,
            moved: false,
            band: false,
            drag: false,
            base,
        });
        true
    }

    fn gallery_release(&mut self, b: Button, p: (f32, f32), _now: f64) -> bool {
        let ga = &mut self.gallery;
        ga.slider_drag = false;
        if ga.filter_release() {
            self.filter_primary();
            return true;
        }
        if ga.scrollbar_drag.take().is_some() || ga.picker.drag.take().is_some() {
            return true;
        }
        let Some(press) = ga.press.take() else {
            return b == Button::Primary && ga.compare.is_some();
        };
        if b != Button::Primary {
            return true;
        }
        if press.drag {
            // Dropped on a folder tile: move there, or copy (modifier keys, another volume).
            ga.drop_target = None;
            if let Some(i) = ga.tile_at(p)
                && let Some(e) = ga.entry_of(i)
                && e.kind == EntryKind::Folder
            {
                let dest = e.path.clone();
                let files: Vec<PathBuf> = ga.targets().into_iter().filter(|f| *f != dest).collect();
                if !files.is_empty() {
                    let (c, m) = forced_by(self.mods);
                    let copy = drop_copies(&files, &dest, c, m);
                    self.gallery_move(files, dest, copy);
                }
            }
            return true;
        }
        if press.band {
            return true;
        }
        // A plain click on a tile: just that one (the selection stays while it may be dragged).
        let ctrl = self.mods.control_key() || self.mods.super_key() || self.mods.shift_key();
        if !ctrl && press.tile.is_some() {
            ga.selected.clear();
        }
        true
    }

    /// The hit area under `p` (last drawn wins).
    fn gallery_hit(&mut self, h: Hit, p: (f32, f32)) {
        let (now, shift) = (self.now(), self.mods.shift_key());
        let ga = &mut self.gallery;
        // A pressed button's tooltip stays away until the pointer leaves it.
        if let Some(t) = ga.bar_tip.as_mut() {
            t.1 = f64::INFINITY;
        }
        match h {
            Hit::Home => self.welcome_close_all(),
            Hit::Crumb(d) => self.gallery_open_dir(d),
            Hit::Up => self.gallery_up(),
            Hit::Filter => {
                if ga.filter_open {
                    ga.filter_open = false;
                    ga.filter_focus = false;
                    ga.facets.close();
                    if !ga.filter.is_empty() {
                        ga.filter = Filter::default();
                        ga.dirty = true;
                    }
                } else {
                    ga.filter_open = true;
                    ga.filter_focus = true;
                }
                ga.relayout = true;
            }
            Hit::ViewMenu => self.gallery_menu_view(None),
            Hit::More => self.gallery_menu_more(),
            Hit::Layout(m) => {
                let k = MODES.iter().position(|&x| x == m).unwrap_or(0) as u64;
                self.gallery_code(code::LAYOUT + k);
            }
            Hit::SizeDown => {
                let s = ga.view.size / SIZE_STEP;
                self.gallery_resize(s);
            }
            Hit::SizeUp => {
                let s = ga.view.size * SIZE_STEP;
                self.gallery_resize(s);
            }
            Hit::Slider => {
                ga.slider_drag = true;
                self.gallery_slider_to(p.0);
            }
            Hit::Sort => self.gallery_code(code::MENU_SORT),
            Hit::Chip(c) => {
                ga.filter.remove(c);
                ga.dirty = true;
            }
            Hit::FilterField => {
                ga.go(super::filterbar::Stop::Field);
                ga.filter_press(p.0, now, shift);
            }
            Hit::FacetsButton => {
                let open = !ga.facets.open;
                ga.facets_show(open, false);
            }
            Hit::Facet(k) => ga.facets_click(k),
            Hit::FacetsFind => {
                ga.filter_focus = false;
                ga.facets.focus = None;
                ga.library.focus = None;
                ga.bar.focus = None;
                ga.facets.find_focus = true;
            }
            Hit::FacetsPanel
            | Hit::KeywordsPanel
            | Hit::MetaEditPanel
            | Hit::PickerPanel
            | Hit::InfoCard
            | Hit::SelBarBg => {}
            Hit::InfoClose => ga.info = None,
            Hit::SelBar(_) | Hit::SelMore => {
                let r = ga
                    .sel_bar
                    .as_ref()
                    .and_then(|b| b.buttons.iter().find(|(_, bh, _)| *bh == h))
                    .map_or_else(|| Rect::new(p.0, p.1, 0.0, 0.0), |b| b.0);
                self.gallery_sel_bar(h, r);
            }
            Hit::PickerRow(k, on_box) => {
                let many = on_box || self.mods.control_key() || self.mods.super_key();
                self.gallery.picker_click(k, many);
            }
            Hit::PickerOp(k) => ga.picker_op(k),
            Hit::PickerScroll => ga.picker_grab(p.1),
            Hit::Library(s) => self.library_hit(s),
            Hit::CloseFilter => ga.close_filter(),
            Hit::Column(k) => {
                if ga.view.sort == k {
                    self.gallery_code(code::REVERSE);
                } else {
                    let i = SortKey::ALL.iter().position(|&x| x == k).unwrap_or(0) as u64;
                    self.gallery_code(code::SORT + i);
                }
            }
            Hit::Scrollbar => {
                let (ty, th) = ga.thumb();
                let grab = if p.1 >= ty && p.1 <= ty + th {
                    p.1 - ty
                } else {
                    th / 2.0
                };
                ga.scrollbar_drag = Some(grab);
                ga.scroll_bar_to(p.1, grab);
            }
        }
        self.redraw = true;
    }

    fn gallery_slider_to(&mut self, x: f32) {
        let Some(r) = self
            .gallery
            .hits
            .iter()
            .find(|h| h.1 == Hit::Slider)
            .map(|h| h.0)
        else {
            return;
        };
        let mut v = ((x - r.x - 4.0) / (r.w - 8.0)).clamp(0.0, 1.0);
        if self.gallery.rtl {
            v = 1.0 - v;
        }
        let size = model::MIN_SIZE + v * (model::MAX_SIZE - model::MIN_SIZE);
        self.gallery_resize(size);
    }

    /// The wheel (lines, or pixels from touchpads). True: the gallery used it.
    pub(in crate::app) fn gallery_wheel(
        &mut self,
        lines: (f32, f32),
        pixels: bool,
        p: (f32, f32),
    ) -> bool {
        let now = self.now();
        let ga = &mut self.gallery;
        ga.pointer = Some(p);
        ga.pointer_input = true;
        // The card stays by its tile: scrolling the tile away closes it.
        if ga.info.take().is_some() {
            self.redraw = true;
        }
        let ga = &mut self.gallery;
        if ga.import.open {
            self.import_wheel(lines, pixels, p);
            return true;
        }
        if ga.map.open {
            self.map_wheel(lines, pixels, p);
            return true;
        }
        if ga.similar.is_open() {
            self.similar_wheel(lines, pixels, p);
            return true;
        }
        if ga.compare.is_some() {
            self.compare_wheel(lines, pixels, p);
            return true;
        }
        if ga.quick.is_some() {
            return true;
        }
        if self.keywords_wheel(p, lines.1 * 40.0) || self.meta_wheel(p, lines.1 * 40.0) {
            return true;
        }
        let ga = &mut self.gallery;
        if ga.picker_wheel(lines.1 * 40.0, p)
            || ga.facets_wheel(lines.1 * 40.0, p)
            || ga.library_wheel(lines.1 * 40.0, p)
        {
            self.redraw = true;
            return true;
        }
        if self.mods.control_key() {
            let f = if pixels {
                1.004f32.powf(lines.1 * 40.0)
            } else {
                1.12f32.powf(lines.1)
            };
            self.gallery_scale_by(f);
            return true;
        }
        let gh = ga.grid_h();
        if pixels {
            // Touchpads scroll 1:1 (their own inertia follows).
            ga.scroll = ga.layout.clamp_scroll(ga.scroll - lines.1 * 40.0, gh);
            ga.scroll_target = ga.scroll;
        } else {
            let step = (ga.size_shown * 0.75).clamp(60.0, 200.0);
            ga.scroll_target = ga
                .layout
                .clamp_scroll(ga.scroll_target - lines.1 * step, gh);
        }
        ga.scrolled_at = now;
        ga.hover = None;
        self.redraw = true;
        true
    }

    /// Touchpad pinch: tile size.
    /// Scales the tile size by `f` (touchpad pinch, Ctrl+wheel). Steps too small for
    /// [`gallery_resize`](Self::gallery_resize)'s half-pixel threshold add up instead of being
    /// lost, so a slow pinch still resizes smoothly.
    pub(in crate::app) fn gallery_scale_by(&mut self, f: f32) {
        if !f.is_finite() || f <= 0.0 || self.gallery.view.mode == Mode::Details {
            return;
        }
        let size = self.gallery.view.size;
        let want = match self.gestures.gallery_size {
            Some((want, set)) if set == size => want,
            _ => size,
        };
        let want = (want * f).clamp(model::MIN_SIZE, model::MAX_SIZE);
        self.gallery_resize(want);
        self.gestures.gallery_size = Some((want, self.gallery.view.size));
    }
}

impl Gallery {
    pub(super) fn hit_at(&self, p: (f32, f32)) -> Option<Hit> {
        self.hits
            .iter()
            .rev()
            .find(|h| h.0.contains(p))
            .map(|h| h.1.clone())
    }

    /// The scrollbar thumb's top and height (logical px).
    pub(super) fn thumb(&self) -> (f32, f32) {
        let gh = self.grid_h();
        let total = self.layout.extent().max(1.0);
        let track_y = self.grid_top() + 4.0;
        let track_h = gh - 8.0;
        let th = (gh * gh / total).max(28.0).min(track_h);
        let ty = track_y + (track_h - th) * (self.scroll / (total - gh).max(1.0)).clamp(0.0, 1.0);
        (ty, th)
    }

    /// Scrollbar drag: the thumb's top follows the pointer.
    fn scroll_bar_to(&mut self, y: f32, grab: f32) {
        let gh = self.grid_h();
        let total = self.layout.extent();
        let track_y = self.grid_top() + 4.0;
        let track_h = gh - 8.0;
        let th = (gh * gh / total.max(1.0)).max(28.0).min(track_h);
        let f = ((y - grab - track_y) / (track_h - th).max(1.0)).clamp(0.0, 1.0);
        self.scroll = (f * (total - gh)).max(0.0);
        self.scroll_target = self.scroll;
        self.scrolled_at = self.last_frame;
    }

    /// The pointer moved recently (Space then means quick look under it).
    fn pointer_input_recent(&self) -> bool {
        self.pointer_input
    }
}

// ----- Drops onto folder tiles ------------------------------------------------------------

impl App {
    /// Files from another application entered the window (`Some`, once per file) or left it
    /// (`None`).
    pub(in crate::app) fn gallery_file_hover(&mut self, file: Option<PathBuf>) {
        let ga = &mut self.gallery;
        match file {
            Some(p) if ga.active => ga.drop_hover.push(p),
            Some(_) => {}
            None => {
                ga.drop_hover.clear();
                ga.drop_target = None;
            }
        }
        self.redraw = true;
    }

    /// A file from another application was dropped. True when it lands in a folder tile (moved
    /// or copied with the others of the same drop, undoably); false to open it as before.
    ///
    /// winit reports drops without a position: X11 asks the server where the pointer is;
    /// elsewhere (Windows, macOS; Wayland has no drops in winit) a drop still opens.
    pub(in crate::app) fn gallery_file_dropped(&mut self, file: &Path) -> bool {
        self.gallery.drop_hover.clear();
        self.gallery.drop_target = None;
        if !self.gallery.active {
            return false;
        }
        if let Some(d) = self.gallery.dropped.as_mut() {
            if *file != d.dest && !d.dest.starts_with(file) {
                d.files.push(file.to_path_buf());
            }
            return true;
        }
        let Some(dp) = self.drop_pointer() else {
            return false;
        };
        let sc = self.dl.scale.max(0.1) as f64;
        let ga = &mut self.gallery;
        let q = ((dp.x / sc) as f32, (dp.y / sc) as f32);
        let Some(dest) = ga
            .tile_at(q)
            .and_then(|i| ga.entry_of(i))
            .filter(|e| e.kind == EntryKind::Folder)
            .map(|e| e.path.clone())
        else {
            return false;
        };
        if *file == dest || dest.starts_with(file) {
            return false;
        }
        ga.dropped = Some(Dropped {
            dest,
            forced: forced_by_pointer(&dp),
            files: vec![file.to_path_buf()],
        });
        self.redraw = true;
        true
    }

    /// Every tick: carries out a drop from outside (after its event burst), and keeps the
    /// folder tile under a drag highlighted with what a drop there would do.
    pub(super) fn gallery_drop_tick(&mut self) {
        if let Some(d) = self.gallery.dropped.take() {
            let copy = drop_copies(&d.files, &d.dest, d.forced.0, d.forced.1);
            info!(target: UI, "gallery: {} file(s) dropped on {}", d.files.len(), d.dest.display());
            self.gallery_move(d.files, d.dest, copy);
        }
        let ga = &self.gallery;
        let internal = ga.press.as_ref().is_some_and(|p| p.drag);
        let (at, forced) = if internal {
            (ga.pointer, forced_by(self.mods))
        } else if !ga.drop_hover.is_empty() {
            match self.drop_pointer() {
                Some(dp) => {
                    let sc = self.dl.scale.max(0.1) as f64;
                    (
                        Some(((dp.x / sc) as f32, (dp.y / sc) as f32)),
                        forced_by_pointer(&dp),
                    )
                }
                None => (None, (false, false)),
            }
        } else {
            if self.gallery.drop_target.take().is_some() {
                self.redraw = true;
            }
            return;
        };
        let target = at.and_then(|q| ga.tile_at(q)).and_then(|i| {
            ga.entry_of(i)
                .filter(|e| e.kind == EntryKind::Folder)
                .map(|e| (i, e.path.clone()))
        });
        let new = target.map(|(tile, dest)| {
            let copy = match &ga.drop_target {
                // The volume check stats files: once per target and modifier state.
                Some(t) if t.dest == dest && t.forced == forced => t.copy,
                _ => {
                    let files = if internal {
                        ga.targets()
                    } else {
                        ga.drop_hover.clone()
                    };
                    drop_copies(&files, &dest, forced.0, forced.1)
                }
            };
            DropTarget {
                tile,
                dest,
                copy,
                forced,
            }
        });
        if new != self.gallery.drop_target {
            self.gallery.drop_target = new;
            self.redraw = true;
        }
    }

    /// Context menu of a folder tile: the selected files go there (the keyboard way to do
    /// what dragging does).
    pub(super) fn gallery_move_selected_here(&mut self, copy: bool) {
        let ga = &self.gallery;
        let Some(dest) = ga
            .focus_index()
            .and_then(|i| ga.entry_of(i))
            .filter(|e| e.kind == EntryKind::Folder)
            .map(|e| e.path.clone())
        else {
            return;
        };
        let files = ga.selected_except(&dest);
        if !files.is_empty() {
            self.gallery_move(files, dest, copy);
        }
    }
}

/// The modifier keys that force a copy or a move, as the platform's file manager has them:
/// Option copies and Command moves on macOS; Ctrl copies and Shift moves elsewhere.
fn forced_by(m: specular_gfx::winit::keyboard::ModifiersState) -> (bool, bool) {
    if cfg!(target_os = "macos") {
        (m.alt_key(), m.super_key())
    } else {
        (m.control_key(), m.shift_key() && !m.control_key())
    }
}

fn forced_by_pointer(p: &specular_platform::drag::DropPointer) -> (bool, bool) {
    (p.control, p.shift && !p.control)
}

/// Whether a drop of `files` onto `dest` copies: forced either way by the modifier keys,
/// else a drop on the same volume moves and onto another volume copies (the file managers'
/// rule). A few files are enough to tell.
pub(super) fn drop_copies(
    files: &[PathBuf],
    dest: &Path,
    force_copy: bool,
    force_move: bool,
) -> bool {
    if force_copy || force_move {
        return force_copy;
    }
    files.iter().take(16).any(|f| !same_volume(f, dest))
}

#[cfg(unix)]
fn same_volume(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (std::fs::symlink_metadata(a), std::fs::metadata(b)) {
        (Ok(x), Ok(y)) => x.dev() == y.dev(),
        _ => true,
    }
}

#[cfg(windows)]
fn same_volume(a: &Path, b: &Path) -> bool {
    // The drive or share prefix (`C:`, `\\server\\share`).
    let root = |p: &Path| {
        p.components()
            .next()
            .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
    };
    root(a) == root(b)
}

#[cfg(not(any(unix, windows)))]
fn same_volume(_: &Path, _: &Path) -> bool {
    true
}

/// What the gallery shows when Esc comes (see [`escape_step`]).
#[derive(Clone, Copy, Debug, Default)]
struct EscapeState {
    quick: bool,
    compare: bool,
    /// The facets panel is open.
    facets: bool,
    /// The filter bar is open with nothing in it (a focused field takes Esc itself).
    empty_filter_open: bool,
    /// The folder has a parent (not a filesystem or drive root).
    has_parent: bool,
    selection: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EscapeStep {
    CloseQuickLook,
    CloseCompare,
    CloseFacets,
    CloseFilter,
    Up,
    ClearSelection,
    /// Back to the start screen.
    Home,
}

/// Esc in the grid: transient views close first; then up one folder, each press another
/// level (the selection stays behind with the folder: going up is what Esc asks for). At the
/// root (and in the Library) it clears the selection, then goes back to the start screen (it
/// never quits or closes the window). Returning to the viewer is `g` or Enter, as before.
fn escape_step(s: EscapeState) -> EscapeStep {
    if s.quick {
        EscapeStep::CloseQuickLook
    } else if s.compare {
        EscapeStep::CloseCompare
    } else if s.facets {
        EscapeStep::CloseFacets
    } else if s.empty_filter_open {
        EscapeStep::CloseFilter
    } else if s.has_parent {
        EscapeStep::Up
    } else if s.selection {
        EscapeStep::ClearSelection
    } else {
        EscapeStep::Home
    }
}

/// Whether a channel key changes the view while the gallery shows images whose alpha is
/// `alpha` (`None`: not known yet), one per image in quick look or compare; none shown (the
/// grid) takes no channel key. An alpha key needs an image that may have alpha, as the
/// viewer's alpha views are unavailable on an opaque image (DESIGN.md §6.2).
fn channel_applies(alpha_key: bool, alpha: &[Option<bool>]) -> bool {
    !alpha.is_empty() && (!alpha_key || alpha.iter().any(|a| *a != Some(false)))
}

#[cfg(test)]
mod tests {
    #[test]
    fn channel_keys_apply_to_quick_look_and_compare_not_the_grid() {
        use super::channel_applies;
        // The grid: nothing shown takes the key.
        assert!(!channel_applies(false, &[]));
        assert!(!channel_applies(true, &[]));
        // A colour or luminance key: any image shown.
        assert!(channel_applies(false, &[Some(false)]));
        assert!(channel_applies(false, &[Some(false), Some(false), None]));
        // The alpha key: one image that has alpha, or may have (not decoded yet), is enough
        // in compare; every image known to be opaque turns it down, as in the viewer.
        assert!(channel_applies(true, &[Some(false), Some(true)]));
        assert!(channel_applies(true, &[Some(false), None]));
        assert!(!channel_applies(true, &[Some(false)]));
        assert!(!channel_applies(
            true,
            &[Some(false), Some(false), Some(false)]
        ));
    }

    #[test]
    fn escape_closes_transient_views_then_goes_up_then_home() {
        use super::{EscapeState as S, EscapeStep as E, escape_step};
        let all = S {
            quick: true,
            compare: true,
            facets: true,
            empty_filter_open: true,
            has_parent: true,
            selection: true,
        };
        assert_eq!(escape_step(all), E::CloseQuickLook);
        assert_eq!(
            escape_step(S {
                quick: false,
                ..all
            }),
            E::CloseCompare
        );
        assert_eq!(
            escape_step(S {
                quick: false,
                compare: false,
                ..all
            }),
            E::CloseFacets
        );
        let grid = S {
            quick: false,
            compare: false,
            facets: false,
            ..all
        };
        assert_eq!(escape_step(grid), E::CloseFilter);
        // A selection does not hold Esc back from going up.
        let grid = S {
            empty_filter_open: false,
            ..grid
        };
        assert_eq!(escape_step(grid), E::Up);
        // The root: the selection clears, then the start screen (never quit).
        let root = S {
            has_parent: false,
            ..grid
        };
        assert_eq!(escape_step(root), E::ClearSelection);
        assert_eq!(escape_step(S::default()), E::Home);
        assert!(std::path::Path::new("/").parent().is_none());
    }

    use super::*;

    #[test]
    fn drops_move_on_one_volume_unless_told_otherwise() {
        let here = Path::new(env!("CARGO_MANIFEST_DIR"));
        let files = [here.join("Cargo.toml")];
        let dest = here.join("src");
        assert!(!drop_copies(&files, &dest, false, false));
        assert!(drop_copies(&files, &dest, true, false));
        assert!(!drop_copies(&files, &dest, false, true));
        // Ctrl copies even with Shift held (Ctrl+Shift is "link" in file managers, not move).
        let m = specular_gfx::winit::keyboard::ModifiersState::CONTROL
            | specular_gfx::winit::keyboard::ModifiersState::SHIFT;
        if !cfg!(target_os = "macos") {
            assert_eq!(forced_by(m), (true, false));
        }
    }
}
