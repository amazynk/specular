//! Keyboard focus in the chrome (DESIGN.md §11): the toolbar, the transport bar, the info
//! panel and the insights indicator are reachable from the keyboard, and so are the buttons
//! of the performance insights panel.
//!
//! Focus moves in *stops*: a group of controls the keyboard reaches with one Tab. The bars
//! are one stop each (a roving focus: ←/→ move between their controls, mirrored in
//! right-to-left layouts, Home/End go to the ends); the info panel's tabs are another; every
//! other control is a stop of its own. Tab / Shift+Tab go from stop to stop, F6 / Shift+F6
//! from region to region (the bars, the info panel, the indicator), and past the last one
//! focus goes back to the image, where every key does what the keymap says again. From the
//! image, Tab and F6 enter the chrome only while the keymap binds nothing to them.
//!
//! The stops are taken from the last frame's layout, so only what is on screen is reachable.

use specular_gfx::Rect;
use specular_input::{Action, Key, KeyCombo, NamedKey};
use specular_settings::ToolbarItem;

use crate::ctx::Target;
use crate::settings::SettingsTarget;
use crate::toolbar_edit::Part;
use crate::{Chrome, Command, Modal, Response, ViewerState};

/// Where a stop is (F6 moves between regions).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Region {
    /// The start screen's controls (each a stop of its own), before the toolbar.
    Welcome,
    Toolbar,
    Transport,
    Info,
    Indicator,
}

/// One Tab stop: its region and its controls (more than one: a roving group).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Stop {
    pub region: Region,
    pub targets: Vec<Target>,
}

/// Enter or Space held on Previous or Next: the key's repeats step on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct KeyHold {
    next: bool,
    since: f64,
    /// It has repeated: its release ends the flipping.
    fired: bool,
}

/// The region a focusable control of the viewer is in.
pub(crate) fn region_of(t: &Target) -> Option<Region> {
    Some(match t {
        Target::Position | Target::Toolbar(_) | Target::ToolbarMore | Target::ProfileChip(_) => {
            Region::Toolbar
        }
        Target::Transport(_) | Target::Motion | Target::Volume => Region::Transport,
        Target::Welcome(_) | Target::DecoderCard(_) => Region::Welcome,
        Target::InfoClose
        | Target::InfoTab(_)
        | Target::InfoPerf
        | Target::InfoShowOnMap
        | Target::InfoEditMetadata
        | Target::InfoCopy(..)
        | Target::InfoSummary(..) => Region::Info,
        Target::Work
        | Target::Downloads
        | Target::Insight
        | Target::InsightHide
        | Target::PresentingPause(_)
        | Target::PresentingStop(_)
        | Target::CastStop => Region::Indicator,
        _ => return None,
    })
}

pub(crate) fn is_tab(combo: &KeyCombo) -> Option<bool> {
    let m = combo.mods;
    (combo.key == Key::Named(NamedKey::Tab) && !m.ctrl && !m.alt && !m.meta).then_some(m.shift)
}

fn is_f6(combo: &KeyCombo) -> Option<bool> {
    let m = combo.mods;
    (combo.key == Key::Named(NamedKey::F(6)) && !m.ctrl && !m.alt && !m.meta).then_some(m.shift)
}

impl Chrome {
    /// The viewer's Tab stops as last drawn, in order.
    pub(crate) fn stops(&self) -> Vec<Stop> {
        let mut out = Vec::new();
        if self.gallery || self.modal.is_some() {
            return out;
        }
        // The start screen first, in reading order, a stop per control.
        for t in crate::welcome::stops(&self.hits) {
            out.push(Stop {
                region: Region::Welcome,
                targets: vec![t],
            });
        }
        // The "needs a free decoder" card's buttons: one stop (←/→ move between them).
        let mut card: Vec<Target> = Vec::new();
        for (_, t) in &self.hits {
            if matches!(t, Target::DecoderCard(_)) && !card.contains(t) {
                card.push(t.clone());
            }
        }
        if !card.is_empty() {
            out.push(Stop {
                region: Region::Welcome,
                targets: card,
            });
        }
        // The bar as last drawn, also once it has auto-hidden: the focus brings it back (it
        // stays up while it has the keyboard), so a keyboard alone can always reach it.
        // Full-screen play mode hides the bar and the panels: only the transport is reached.
        if !self.bar_stops.is_empty() && !self.video_fullscreen() {
            out.push(Stop {
                region: Region::Toolbar,
                targets: self.bar_stops.clone(),
            });
        }
        let drawn = |want: &dyn Fn(&Target) -> bool| -> Vec<Target> {
            let mut v: Vec<Target> = Vec::new();
            for (_, t) in &self.hits {
                if want(t) && !v.contains(t) {
                    v.push(t.clone());
                }
            }
            v
        };
        let mut transport = drawn(&|t| matches!(t, Target::Transport(_) | Target::Motion));
        let mut volume = self.hits.iter().any(|(_, t)| *t == Target::Volume);
        if transport.is_empty() && self.video_fullscreen() {
            // Faded out: the focus brings the play controls back, the volume slider too.
            transport = self
                .play_stops
                .iter()
                .filter(|t| **t != Target::Volume)
                .cloned()
                .collect();
            volume |= self.play_stops.contains(&Target::Volume);
        }
        if !transport.is_empty() {
            out.push(Stop {
                region: Region::Transport,
                targets: transport,
            });
        }
        // The volume slider: a stop of its own (its arrows change the volume).
        if volume {
            out.push(Stop {
                region: Region::Transport,
                targets: vec![Target::Volume],
            });
        }
        if self.info_open && !self.video_fullscreen() {
            // As drawn: the close button, the tabs (one stop), the performance section.
            let tabs = drawn(&|t| matches!(t, Target::InfoTab(_)));
            for t in drawn(&|t| {
                matches!(
                    t,
                    Target::InfoClose
                        | Target::InfoTab(_)
                        | Target::InfoShowOnMap
                        | Target::InfoEditMetadata
                        | Target::InfoPerf
                )
            }) {
                let targets = match t {
                    Target::InfoTab(false) | Target::InfoTab(true)
                        if out.iter().any(|s| s.targets == tabs) =>
                    {
                        continue;
                    }
                    Target::InfoTab(_) => tabs.clone(),
                    t => vec![t],
                };
                out.push(Stop {
                    region: Region::Info,
                    targets,
                });
            }
            // All fields: the rows are one stop (↑/↓ move along them, Enter copies).
            if self.info_all && !self.info_rows.is_empty() {
                out.push(Stop {
                    region: Region::Info,
                    targets: self.info_rows.clone(),
                });
            }
        }
        // The "presenting on" control: its buttons are one stop (←/→ move between them).
        let presenting = drawn(&|t| {
            matches!(
                t,
                Target::PresentingPause(_) | Target::PresentingStop(_) | Target::CastStop
            )
        });
        if !presenting.is_empty() {
            out.push(Stop {
                region: Region::Indicator,
                targets: presenting,
            });
        }
        // The indicators as they are seen: top to bottom, then along the line.
        let mut ind: Vec<(Rect, Target)> = Vec::new();
        for (r, t) in &self.hits {
            if matches!(
                t,
                Target::Work | Target::Downloads | Target::Insight | Target::InsightHide
            ) && !ind.iter().any(|(_, u)| u == t)
            {
                ind.push((*r, t.clone()));
            }
        }
        let rtl = self.last_rtl;
        ind.sort_by(|(a, _), (b, _)| {
            let line = (a.y - b.y).abs() < a.h.min(b.h) * 0.5;
            if line {
                let (ax, bx) = if rtl {
                    (-a.right(), -b.right())
                } else {
                    (a.x, b.x)
                };
                ax.total_cmp(&bx)
            } else {
                a.y.total_cmp(&b.y)
            }
        });
        for t in ind.into_iter().map(|(_, t)| t) {
            out.push(Stop {
                region: Region::Indicator,
                targets: vec![t],
            });
        }
        out
    }

    /// The stop holding the keyboard focus, if it is still on screen.
    fn focus_stop(&self, stops: &[Stop]) -> Option<usize> {
        let k = self.kbd.as_ref()?;
        stops.iter().position(|s| s.targets.contains(k))
    }

    /// Puts the keyboard on stop `i`: a bar on the control it was last on (else its first),
    /// the info tabs on the selected one.
    fn enter_stop(&mut self, s: &Stop, from_end: bool) {
        let t = match s.region {
            Region::Toolbar => self
                .bar_last
                .clone()
                .filter(|t| s.targets.contains(t))
                // The bar itself, not the chip above it.
                .or_else(|| {
                    s.targets
                        .iter()
                        .find(|t| !matches!(t, Target::ProfileChip(_)))
                        .cloned()
                })
                .unwrap_or_else(|| s.targets[0].clone()),
            Region::Info if matches!(s.targets[0], Target::InfoCopy(..)) => {
                // The first row in view (the last, coming back), else the first.
                let shown = |t: &&Target| self.hits.iter().any(|(_, h)| h == *t);
                let pick = if from_end {
                    s.targets.iter().rev().find(shown)
                } else {
                    s.targets.iter().find(shown)
                };
                self.info_reveal = 4;
                pick.unwrap_or(&s.targets[0]).clone()
            }
            Region::Info if s.targets.len() > 1 => Target::InfoTab(self.info_all),
            _ if from_end => s.targets[s.targets.len() - 1].clone(),
            _ => s.targets[0].clone(),
        };
        self.set_kbd(Some(t));
    }

    /// Moves the keyboard focus (`None`: back to the image).
    pub(crate) fn set_kbd(&mut self, t: Option<Target>) {
        if t != self.kbd {
            self.kbd_since = self.last_now;
            self.kbd_hold = None;
        }
        if let Some(t) = &t
            && region_of(t) == Some(Region::Toolbar)
        {
            self.bar_last = Some(t.clone());
        }
        self.kbd = t;
    }

    /// Tab / Shift+Tab (`by_region`: F6 / Shift+F6) from the focused stop, or from the image.
    fn cycle(&mut self, back: bool, by_region: bool) -> Response {
        let stops = self.stops();
        if stops.is_empty() {
            self.set_kbd(None);
            return Response::default();
        }
        let n = stops.len();
        let at = self.focus_stop(&stops);
        // The next stop, or `n` for the image.
        let i = if by_region {
            let starts: Vec<usize> = (0..n)
                .filter(|&i| i == 0 || stops[i].region != stops[i - 1].region)
                .collect();
            // The start of the region the focus is in.
            let cur = at.map(|a| starts.iter().rposition(|&s| s <= a).unwrap_or(0));
            match (cur, back) {
                (None, false) => starts[0],
                (None, true) => starts[starts.len() - 1],
                (Some(c), false) => starts.get(c + 1).copied().unwrap_or(n),
                (Some(0), true) => n,
                (Some(c), true) => starts[c - 1],
            }
        } else {
            match (at, back) {
                (None, false) => 0,
                (None, true) => n - 1,
                (Some(i), false) => i + 1,
                (Some(0), true) => n,
                (Some(i), true) => i - 1,
            }
        };
        if i >= n {
            self.set_kbd(None);
        } else {
            let s = stops[i].clone();
            self.enter_stop(&s, back && !by_region);
        }
        Response::used()
    }

    /// Keys for the keyboard focus in the viewer (no dialog open). `None`: not the focus's;
    /// the key does what it usually does.
    pub(crate) fn focus_key(
        &mut self,
        combo: &KeyCombo,
        named: Option<NamedKey>,
        repeat: bool,
        st: &ViewerState,
        now: f64,
    ) -> Option<Response> {
        if self.modal.is_some() {
            return None;
        }
        if self.gallery {
            // The gallery moves the focus itself (`focus_indicator`); here only the indicator
            // is pressed.
            let t = self.kbd.clone()?;
            return match named? {
                NamedKey::Enter | NamedKey::Space if self.focusable(&t) => {
                    Some(self.press_focused(t, repeat, st, now))
                }
                _ => None,
            };
        }
        let stops = self.stops();
        let at = self.focus_stop(&stops);
        if at.is_none() && self.kbd.is_some() {
            // What had the focus is gone (closed, or moved into the More menu).
            self.set_kbd(None);
        }
        let bound = st.keymap.lookup(combo).is_some();
        if let Some(back) = is_tab(combo) {
            // From the image Tab is the keymap's when it binds it (DESIGN.md reserves it for
            // Viewer ↔ Library); F6 always gets there.
            if at.is_some() || !bound {
                return Some(self.cycle(back, false));
            }
            return None;
        }
        if let Some(back) = is_f6(combo)
            && !bound
        {
            return Some(self.cycle(back, true));
        }
        let at = at?;
        let t = self.kbd.clone()?;
        if matches!(t, Target::InfoCopy(..)) {
            return self.rows_key(combo, named, repeat, &stops[at].targets, t, st);
        }
        if t == Target::Volume {
            return self.volume_key(combo, named, st, now);
        }
        let group = &stops[at].targets;
        let pos = group.iter().position(|x| *x == t).unwrap_or(0);
        let n = group.len();
        let named = named?;
        // Physical arrows: in right-to-left layouts the bar runs from the right.
        let (fwd, bwd) = if self.last_rtl {
            (NamedKey::ArrowLeft, NamedKey::ArrowRight)
        } else {
            (NamedKey::ArrowRight, NamedKey::ArrowLeft)
        };
        let to = match named {
            k if k == fwd && n > 1 => Some((pos + 1) % n),
            k if k == bwd && n > 1 => Some((pos + n - 1) % n),
            NamedKey::Home if n > 1 => Some(0),
            NamedKey::End if n > 1 => Some(n - 1),
            _ => None,
        };
        if let Some(to) = to {
            let next = group[to].clone();
            self.set_kbd(Some(next));
            return Some(Response::used());
        }
        match named {
            NamedKey::Enter | NamedKey::Space => Some(self.press_focused(t, repeat, st, now)),
            NamedKey::ArrowUp | NamedKey::ArrowDown => {
                let up = named == NamedKey::ArrowUp;
                match t {
                    Target::Toolbar(ToolbarItem::ZoomLevel) if t.disabled(st).is_none() => {
                        if !self.zoom_menu {
                            self.activate(t, st);
                        }
                        self.zoom_sel = Some(if up {
                            crate::toolbar::zoom_entries().len() - 1
                        } else {
                            0
                        });
                        Some(Response::used())
                    }
                    Target::ToolbarMore if !self.toolbar_overflow.is_empty() => {
                        let last = self.toolbar_overflow.len() - 1;
                        self.overflow_menu = Some(Some(if up { last } else { 0 }));
                        self.zoom_menu = false;
                        Some(Response::used())
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Keys on the info panel's All-fields rows: ↑/↓ move a row, PageUp/PageDown a view's
    /// worth, Home/End to the ends (none wraps); Enter, Space and Ctrl+C copy the row's value.
    fn rows_key(
        &mut self,
        combo: &KeyCombo,
        named: Option<NamedKey>,
        repeat: bool,
        rows: &[Target],
        t: Target,
        st: &ViewerState,
    ) -> Option<Response> {
        let m = combo.mods;
        let copy = (m.ctrl || m.meta)
            && !m.alt
            && matches!(combo.key, Key::Char(c) if c.eq_ignore_ascii_case(&'c'));
        if copy || matches!(named, Some(NamedKey::Enter | NamedKey::Space)) {
            if repeat {
                return Some(Response::used());
            }
            return Some(self.activate(t, st));
        }
        let n = rows.len();
        let pos = rows.iter().position(|x| *x == t)?;
        // A page: the rows in view, less one so the last stays in sight.
        let page = rows
            .iter()
            .filter(|r| self.hits.iter().any(|(_, h)| h == *r))
            .count()
            .saturating_sub(1)
            .max(1);
        let to = match named? {
            NamedKey::ArrowDown => (pos + 1).min(n - 1),
            NamedKey::ArrowUp => pos.saturating_sub(1),
            NamedKey::PageDown => (pos + page).min(n - 1),
            NamedKey::PageUp => pos.saturating_sub(page),
            NamedKey::Home => 0,
            NamedKey::End => n - 1,
            _ => return None,
        };
        self.set_kbd(Some(rows[to].clone()));
        self.info_reveal = 4;
        Some(Response::used())
    }

    /// Enter or Space on the focused control: presses it. A menu button opens its menu
    /// with the keyboard on its first row; Previous and Next repeat while the key is held.
    fn press_focused(&mut self, t: Target, repeat: bool, st: &ViewerState, now: f64) -> Response {
        if repeat {
            if let Some(h) = &mut self.kbd_hold {
                h.fired = true;
                return Response::cmd(Command::NavHold {
                    next: h.next,
                    held: now - h.since,
                });
            }
            // Other controls repeat only when their action does (zoom in and out).
            let repeats = t.action(st).is_some_and(Action::repeats)
                && !matches!(t, Target::Toolbar(ToolbarItem::ZoomLevel));
            if !repeats {
                return Response::used();
            }
        }
        match t {
            Target::Toolbar(ToolbarItem::ZoomLevel) => {
                let opening = !self.zoom_menu;
                let r = self.activate(t, st);
                if opening && self.zoom_menu {
                    self.zoom_sel = Some(0);
                }
                r
            }
            Target::ToolbarMore => {
                let r = self.activate(t, st);
                if self.overflow_menu.is_some() && !self.toolbar_overflow.is_empty() {
                    self.overflow_menu = Some(Some(0));
                }
                r
            }
            Target::Toolbar(it @ (ToolbarItem::Prev | ToolbarItem::Next)) => {
                let enabled = t.disabled(st).is_none();
                let r = self.activate(t, st);
                if enabled {
                    self.kbd_hold = Some(KeyHold {
                        next: it == ToolbarItem::Next,
                        since: now,
                        fired: false,
                    });
                }
                r
            }
            _ => self.activate(t, st),
        }
    }

    /// A key let go: ends a held Previous or Next.
    pub(crate) fn key_up(&mut self, combo: &KeyCombo) -> Response {
        if !matches!(combo.key, Key::Named(NamedKey::Enter | NamedKey::Space)) {
            return Response::default();
        }
        match self.kbd_hold.take() {
            Some(h) if h.fired => Response::cmd(Command::NavHoldEnd),
            Some(_) => Response::used(),
            None => Response::default(),
        }
    }

    /// Whether the keyboard reaches `t` (a control drawn this frame).
    pub(crate) fn focusable(&self, t: &Target) -> bool {
        match self.modal {
            Some(Modal::Settings) => self.settings_focusable(t),
            Some(Modal::Palette | Modal::Language) => *t == Target::ModalClose,
            Some(Modal::DicomTags) => matches!(t, Target::ModalClose | Target::TagPatient),
            Some(Modal::Help) => {
                matches!(
                    t,
                    Target::ModalClose | Target::HelpBack | Target::HelpContents
                )
            }
            Some(Modal::Studies) => {
                matches!(t, Target::ModalClose | Target::StudiesPatient)
                    || (*t == Target::StudiesCancel && self.studies.sheet.cancellable)
                    || (*t == Target::StudiesSend && self.studies.sendable())
            }
            Some(_) => self.modal_stops().contains(t),
            None if self.gallery => self.indicator_targets().contains(t),
            None => {
                region_of(t).is_some()
                    && !matches!(t, Target::Toolbar(it) if self.overflow_menu.is_some()
                        && self.toolbar_overflow.contains(it))
            }
        }
    }

    /// The indicators' buttons up now, in Tab order: the background-work indicator, then the
    /// insights indicator and its hide button.
    pub(crate) fn indicator_targets(&self) -> Vec<Target> {
        let mut v = Vec::with_capacity(3);
        if self.work_chip.is_some() {
            v.push(Target::Work);
        }
        if self.downloads_on {
            v.push(Target::Downloads);
        }
        if self.insight_indicator.is_some() {
            v.extend([Target::Insight, Target::InsightHide]);
        }
        v
    }

    /// Whether an indicator is up (background work, or insights): the gallery's F6 reaches
    /// them.
    pub fn indicator_shown(&self) -> bool {
        self.work_chip.is_some() || self.downloads_on || self.insight_indicator.is_some()
    }

    /// Whether a control of the chrome has the keyboard focus (else the keys are the
    /// viewer's: the image's, or a view's own controls').
    pub fn has_keyboard_focus(&self) -> bool {
        self.kbd.is_some()
    }

    /// Whether an indicator has the keyboard focus (the gallery hands it over with
    /// [`Chrome::focus_indicator`]).
    pub fn indicator_focused(&self) -> bool {
        self.modal.is_none()
            && self
                .kbd
                .as_ref()
                .is_some_and(|k| self.indicator_targets().contains(k))
    }

    /// Puts the keyboard on the indicators: their first button, or (`last`) their last.
    /// False when none is up.
    pub fn focus_indicator(&mut self, last: bool) -> bool {
        let v = self.indicator_targets();
        let t = if last { v.last() } else { v.first() };
        let Some(t) = t.cloned() else {
            return false;
        };
        self.set_kbd(Some(t));
        true
    }

    /// Tab (Shift+Tab: `back`) on the indicators: between their buttons. False, with the
    /// focus taken off them, past either end (where it goes is the gallery's).
    pub fn indicator_tab(&mut self, back: bool) -> bool {
        let v = self.indicator_targets();
        let at = self
            .kbd
            .as_ref()
            .and_then(|k| v.iter().position(|t| t == k));
        let to = match (at, back) {
            (Some(i), false) if i + 1 < v.len() => v[i + 1].clone(),
            (Some(i), true) if i > 0 => v[i - 1].clone(),
            _ => {
                self.set_kbd(None);
                return false;
            }
        };
        self.set_kbd(Some(to));
        true
    }

    /// Takes the keyboard focus off the chrome (the gallery moved it elsewhere).
    pub fn blur(&mut self) {
        self.set_kbd(None);
    }

    /// The buttons of the performance insights panel, in Tab order: the items' buttons,
    /// the footer's, then Close.
    pub(crate) fn insight_stops(&self) -> Vec<Target> {
        let ins = &self.insights;
        let mut v: Vec<Target> = ins
            .now
            .iter()
            .enumerate()
            .filter(|(_, it)| it.action.is_some())
            .map(|(i, _)| Target::InsightAction(i))
            .collect();
        if ins.dismiss.is_some() {
            v.push(Target::InsightDismiss);
        }
        if ins.restore.is_some() {
            v.push(Target::InsightRestore);
        }
        if ins.switch.is_some() {
            v.push(Target::InsightSwitch);
        }
        v.push(Target::ModalClose);
        v
    }

    /// The buttons of the open dialog the keyboard reaches, in Tab order. Empty for the
    /// dialogs with a model of their own: Settings (`settings_focus_key`), the palette and
    /// the language list (a field and a list, then Close: `field_dialog_focus_key`).
    pub(crate) fn modal_stops(&self) -> Vec<Target> {
        match self.modal {
            Some(Modal::Insights) => self.insight_stops(),
            Some(Modal::Work) => self.work_stops(),
            Some(Modal::Downloads) => self.downloads_stops(),
            Some(Modal::Queue) => self.queue_stops(),
            // As on screen; Enter with nothing focused is still Move to trash.
            Some(Modal::ConfirmTrash) if self.confirm_alt2.is_some() => vec![
                Target::ConfirmCancel,
                Target::ConfirmAlt2,
                Target::ConfirmAlt,
                Target::ConfirmOk,
            ],
            Some(Modal::ConfirmTrash) if self.confirm_alt.is_some() => {
                vec![Target::ConfirmCancel, Target::ConfirmAlt, Target::ConfirmOk]
            }
            Some(Modal::ConfirmTrash) => vec![Target::ConfirmCancel, Target::ConfirmOk],
            Some(Modal::Profile) => self.profile_stops(),
            Some(Modal::HwReport) => vec![Target::ReportCopy, Target::ModalClose],
            Some(Modal::Crash) => self.crash_stops(),
            Some(Modal::Diagnostics) => self.diag_stops(),
            Some(Modal::About) => vec![
                Target::AboutHelp,
                Target::AboutRenderer,
                Target::AboutDiagnostics,
                Target::AboutDiscord,
                Target::ModalClose,
            ],
            Some(Modal::Plot) => vec![Target::ModalClose],
            // The tabs, then Close.
            Some(Modal::CheatSheet) => self
                .cheat_tabs
                .iter()
                .copied()
                .map(Target::CheatTab)
                .chain([Target::ModalClose])
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Tab, Shift+Tab, Enter and Space in a dialog with buttons: Tab goes round them, Enter
    /// or Space presses the focused one; with none focused Enter keeps the dialog's default.
    /// `None`: not handled here.
    pub(crate) fn modal_focus_key(
        &mut self,
        combo: &KeyCombo,
        named: Option<NamedKey>,
        repeat: bool,
        st: &ViewerState,
    ) -> Option<Response> {
        let stops = self.modal_stops();
        if stops.is_empty() {
            return None;
        }
        if let Some(back) = is_tab(combo) {
            let n = stops.len();
            let at = self
                .modal_focus
                .as_ref()
                .and_then(|f| stops.iter().position(|s| s == f));
            let to = match (at, back) {
                (None, false) => 0,
                (None, true) => n - 1,
                (Some(i), false) => (i + 1) % n,
                (Some(i), true) => (i + n - 1) % n,
            };
            self.modal_focus = Some(stops[to].clone());
            self.reveal_focus = true;
            return Some(Response::used());
        }
        match named? {
            // The first-run question: Enter is Continue wherever the keyboard is but on Skip
            // (Space checks the box it is on); the arrows move between the start choices.
            NamedKey::Enter if self.modal == Some(Modal::Profile) => Some(if repeat {
                Response::used()
            } else if self.modal_focus == Some(Target::ProfileSkip) {
                self.profile_skip()
            } else {
                self.profile_accept(st)
            }),
            NamedKey::ArrowUp | NamedKey::ArrowDown
                if matches!(self.modal_focus, Some(Target::ProfileStart(_))) =>
            {
                self.profile_start_step(if named == Some(NamedKey::ArrowUp) {
                    -1
                } else {
                    1
                });
                Some(Response::used())
            }
            NamedKey::Enter | NamedKey::Space => {
                let t = self.modal_focus.clone().filter(|f| stops.contains(f))?;
                if repeat {
                    return Some(Response::used());
                }
                Some(self.activate(t, st))
            }
            _ => None,
        }
    }

    /// The keyboard in the palette and the language list: their field (with its list) and
    /// Close. Tab and Shift+Tab go from the field to Close and back; on Close, Enter or
    /// Space close the dialog, and any other key goes back to the field and does what it
    /// does there (the arrows move in the list, Backspace edits). `None`: the field's key.
    pub(crate) fn field_dialog_focus_key(
        &mut self,
        combo: &KeyCombo,
        named: Option<NamedKey>,
        repeat: bool,
        st: &ViewerState,
    ) -> Option<Response> {
        if !matches!(self.modal, Some(Modal::Palette | Modal::Language)) {
            return None;
        }
        if is_tab(combo).is_some() {
            // Two places: either way round, Tab goes to the other.
            self.modal_focus = match self.modal_focus {
                None => Some(Target::ModalClose),
                Some(_) => None,
            };
            self.reveal_focus = true;
            return Some(Response::used());
        }
        let t = self.modal_focus.clone()?;
        match named {
            Some(NamedKey::Enter | NamedKey::Space) => Some(if repeat {
                Response::used()
            } else {
                self.activate(t, st)
            }),
            _ => {
                self.modal_focus = None;
                None
            }
        }
    }

    /// The controls of Settings the keyboard reaches with Tab, as last laid out: Reset
    /// section, each row's (the toolbar list is one stop, `ToolRow(0)`), then Close. The
    /// search field is the place before the first and after the last (no `modal_focus`).
    pub(crate) fn settings_stops(&self) -> Vec<Target> {
        let mut v: Vec<Target> = self.settings.stops.iter().map(|s| s.t.clone()).collect();
        v.push(Target::ModalClose);
        v
    }

    /// Whether the keyboard reaches control `t` of Settings with Tab (the parts of the
    /// toolbar list have their own focus).
    fn settings_focusable(&self, t: &Target) -> bool {
        match t {
            Target::Settings(s) if is_tool(s) => false,
            // The key editor's list is one stop with a roving focus: any of its parts.
            Target::Settings(s) if crate::keys_edit::is_list(s) => self
                .settings
                .stops
                .iter()
                .any(|k| matches!(&k.t, Target::Settings(x) if crate::keys_edit::is_list(x))),
            t => self.settings_stops().contains(t),
        }
    }

    /// Where the keyboard is in Settings: an index into `stops`, or `None` for the search
    /// field.
    fn settings_at(&self, stops: &[Target]) -> Option<usize> {
        if self.settings.tools.focus.is_some() {
            let list = Target::Settings(SettingsTarget::ToolRow(0));
            return stops.iter().position(|s| *s == list);
        }
        let f = self.modal_focus.as_ref()?;
        stops.iter().position(|s| s == f).or_else(|| match f {
            // Moved within the key editor's list since it was laid out.
            Target::Settings(x) if crate::keys_edit::is_list(x) => stops
                .iter()
                .position(|s| matches!(s, Target::Settings(x) if crate::keys_edit::is_list(x))),
            _ => None,
        })
    }

    /// Puts the keyboard on stop `t` of Settings (`None`: the search field).
    fn settings_focus(&mut self, t: Option<Target>) {
        self.settings.tools.close_menu();
        match t {
            Some(Target::Settings(SettingsTarget::ToolRow(_))) => {
                self.modal_focus = None;
                self.settings.tools.focus = Some(0);
                self.settings.tools.part = Part::Row;
            }
            t => {
                self.settings.tools.focus = None;
                self.settings.tools.part = Part::Row;
                self.modal_focus = t;
            }
        }
        self.reveal_focus = true;
    }

    /// The keyboard in Settings. Tab and Shift+Tab go from the search field through Reset
    /// section, every row's controls (scrolling each into view) and Close, and back to the
    /// field; Enter or Space press the focused control (a stepper's repeat while held); an
    /// open dropdown takes ↑/↓, Home/End and the page keys, Enter or Space choose. `None`:
    /// the key is not the focus's (the search field, the sections, the toolbar list).
    pub(crate) fn settings_focus_key(
        &mut self,
        combo: &KeyCombo,
        named: Option<NamedKey>,
        repeat: bool,
        st: &ViewerState,
    ) -> Option<Response> {
        if let Some(k) = self.settings.dropdown
            && let Some(r) = self.dropdown_key(k, combo, named, st)
        {
            return Some(r);
        }
        if let Some(back) = is_tab(combo) {
            // Tab in a row's open menu closes it (toolbar_edit.rs).
            if self.settings.tools.menu.is_some() {
                return None;
            }
            self.settings_tab(back);
            return Some(Response::used());
        }
        if self.settings.tools.focus.is_some() {
            return None;
        }
        let stops = self.settings_stops();
        let t = self.modal_focus.clone()?;
        if let Target::Settings(s) = &t
            && crate::keys_edit::is_list(s)
        {
            if combo.mods.ctrl || combo.mods.alt || combo.mods.meta || combo.mods.shift {
                return None;
            }
            return self.keys_list_key(named, repeat, st);
        }
        let Some(i) = stops.iter().position(|s| *s == t) else {
            if let Target::Settings(s) = &t
                && s.is_keys()
            {
                // Put there by the key editor since the page was laid out.
                return match named {
                    Some(NamedKey::Enter | NamedKey::Space) if !combo.mods.ctrl => {
                        Some(if repeat {
                            Response::used()
                        } else {
                            self.activate(t.clone(), st)
                        })
                    }
                    _ => None,
                };
            }
            if !matches!(t, Target::Settings(SettingsTarget::Choose(..))) {
                // Gone (another section, or reset): back to the search field.
                self.modal_focus = None;
            }
            return None;
        };
        let mods = combo.mods;
        if mods.ctrl || mods.alt || mods.meta {
            return None;
        }
        match named? {
            NamedKey::Enter | NamedKey::Space => {
                // A stepper's buttons repeat while held; nothing else does.
                if repeat && !matches!(t, Target::Settings(SettingsTarget::Step(..))) {
                    return Some(Response::used());
                }
                let r = self.activate(t.clone(), st);
                let Target::Settings(s) = t else {
                    return Some(r);
                };
                // Where the keyboard goes when the control goes away or changes.
                let next = stops.get(i + 1).cloned();
                match s {
                    SettingsTarget::Dropdown(k) if self.settings.dropdown == Some(k) => {
                        let cur = st.store.get(k).map(|v| v.to_string()).unwrap_or_default();
                        self.modal_focus = Some(Target::Settings(SettingsTarget::Choose(k, cur)));
                        self.settings.dropdown_reveal = true;
                    }
                    SettingsTarget::Reset(_) | SettingsTarget::ResetSection => {
                        self.modal_focus = next.filter(|n| *n != Target::ModalClose);
                    }
                    SettingsTarget::PanelAction(k) if self.settings.panel_confirm == Some(k) => {
                        self.modal_focus = Some(Target::Settings(SettingsTarget::PanelCancel(k)));
                    }
                    SettingsTarget::PanelConfirm(k) | SettingsTarget::PanelCancel(k)
                        if self.settings.panel_confirm.is_none() =>
                    {
                        self.modal_focus = Some(Target::Settings(SettingsTarget::PanelAction(k)));
                    }
                    _ => {}
                }
                Some(r)
            }
            // ↑/↓ go to the control before or after (the sections are the search field's):
            // ↑ from the first goes back to the field, ↓ stops at the last. Close, at the top
            // of the sheet, is not in the run.
            n @ (NamedKey::ArrowUp | NamedKey::ArrowDown) => {
                if t == Target::ModalClose {
                    return Some(Response::used());
                }
                let to = if n == NamedKey::ArrowUp {
                    i.checked_sub(1).map(|j| stops[j].clone())
                } else {
                    let last = stops.iter().rposition(|s| *s != Target::ModalClose);
                    Some(stops[(i + 1).min(last.unwrap_or(i))].clone())
                };
                self.settings_focus(to);
                Some(Response::used())
            }
            _ => None,
        }
    }

    /// Tab (`back`: Shift+Tab) in Settings.
    fn settings_tab(&mut self, back: bool) {
        // Tab leaves an open dropdown from its button.
        if let Some(k) = self.settings.dropdown.take() {
            self.modal_focus = Some(Target::Settings(SettingsTarget::Dropdown(k)));
        }
        let stops = self.settings_stops();
        let n = stops.len();
        // From the search field Tab starts at the first control of the page, scrolled into
        // view, as Shift+Tab starts at the last: the order is the page's wherever it was
        // scrolled to (a browser's too, scrolling doesn't move its starting point). Opened at
        // a setting (Customise toolbar…), it starts at that setting's first control.
        let start = self
            .settings
            .start
            .take()
            .and_then(|r| self.settings.stops.iter().position(|s| s.row == Some(r)));
        let to = match (self.settings_at(&stops), back) {
            (None, false) => Some(start.unwrap_or(0)),
            (None, true) => Some(n - 1),
            (Some(i), false) => (i + 1 < n).then_some(i + 1),
            (Some(i), true) => i.checked_sub(1),
        };
        self.settings_focus(to.map(|j| stops[j].clone()));
    }

    /// Keys in the open dropdown `k` of Settings.
    fn dropdown_key(
        &mut self,
        k: &'static str,
        combo: &KeyCombo,
        named: Option<NamedKey>,
        st: &ViewerState,
    ) -> Option<Response> {
        if is_tab(combo).is_some() {
            return None;
        }
        let opts = crate::settings::option_values(k, st);
        let n = opts.len();
        if n == 0 {
            return None;
        }
        let at = match &self.modal_focus {
            Some(Target::Settings(SettingsTarget::Choose(fk, v))) if *fk == k => {
                opts.iter().position(|o| o == v)
            }
            _ => None,
        };
        // Without the keyboard on an option, the arrows start from the one chosen now.
        let cur = st.store.get(k).map(|v| v.to_string()).unwrap_or_default();
        let from = at.or_else(|| opts.iter().position(|o| *o == cur));
        const PAGE: usize = 8;
        let to = match named? {
            NamedKey::ArrowDown => from.map_or(0, |i| (i + 1).min(n - 1)),
            NamedKey::ArrowUp => from.map_or(n - 1, |i| i.saturating_sub(1)),
            NamedKey::PageDown => from.map_or(0, |i| (i + PAGE).min(n - 1)),
            NamedKey::PageUp => from.map_or(0, |i| i.saturating_sub(PAGE)),
            NamedKey::Home => 0,
            NamedKey::End => n - 1,
            NamedKey::Enter | NamedKey::Space => {
                let Some(i) = at.or(from) else {
                    return Some(Response::used());
                };
                let had = self.modal_focus.is_some();
                let r = self.activate(
                    Target::Settings(SettingsTarget::Choose(k, opts[i].clone())),
                    st,
                );
                // Opened with the pointer: the keyboard stays in the search field.
                self.modal_focus = had.then_some(Target::Settings(SettingsTarget::Dropdown(k)));
                return Some(r);
            }
            _ => return Some(Response::used()),
        };
        self.modal_focus = Some(Target::Settings(SettingsTarget::Choose(
            k,
            opts[to].clone(),
        )));
        self.settings.dropdown_reveal = true;
        Some(Response::used())
    }
}

/// A part of the toolbar list in Settings (it keeps its own focus, `tools.focus`).
fn is_tool(s: &SettingsTarget) -> bool {
    use SettingsTarget as S;
    matches!(
        s,
        S::ToolRow(_)
            | S::ToolShow(_)
            | S::ToolRemove(_)
            | S::ToolUp(_)
            | S::ToolDown(_)
            | S::ToolMenu(_)
            | S::ToolMenuItem(..)
            | S::ToolMenuBg
    )
}
