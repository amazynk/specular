//! Develop's side panel (DESIGN.md §6.10): a header with Undo, Redo, Before/after and Close,
//! then collapsible module cards holding sliders, a curve editor and colour wheels, then
//! the history list. It sits on the right, on the left in right-to-left languages; sliders
//! mirror with it, the curve and the wheels do not (they are mathematical axes).
//!
//! The panel owns its controls' values; the application fills them from the edit and reads
//! them back when the panel reports a [`Event::Value`]. Keyboard: Tab and Shift+Tab walk
//! every control in reading order; Enter or Space presses buttons, opens and closes cards
//! and turns modules on and off; each control takes its own keys (see [`crate::slider`],
//! [`crate::curve`], [`crate::wheels`]); Up and Down walk the history; Esc gives the keyboard
//! back to the image.
//!
//! [`speed_hud`] draws the speed-edit readout (a control changed by holding its key and
//! scrolling over the image).

use specular_gfx::{Align, DrawList, Icon, Rect, Text, TextStyle, draw_icon};
use specular_i18n::t;
use specular_input::{Key, KeyCombo, NamedKey};
use specular_theme::{Color, Palette, durations, metrics};

use crate::choice::Choice;
use crate::curve::Curve;
use crate::scrollbar::{self as bar, Geometry};
use crate::slider::{self, KeyResult, Slider};
use crate::wheels::{self, Wheel};

/// `title` whole when it fits in `max_w`, else cut to the longest start that does, with "…".
fn fit_title(text: &mut Text, title: &str, style: TextStyle, max_w: f32) -> String {
    if text.measure(title, style) <= max_w {
        return title.to_owned();
    }
    let chars: Vec<char> = title.chars().collect();
    let cut = |n: usize| {
        let mut s: String = chars[..n].iter().collect::<String>().trim_end().to_owned();
        s.push('…');
        specular_i18n::balance_isolates(&s)
    };
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if text.measure(&cut(mid), style) <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    cut(lo)
}

/// Header, card header, gap and history row heights, and paddings (logical px at the
/// standard text size).
const HEADER: f32 = 48.0;
const CARD_HEADER: f32 = 38.0;
const GAP: f32 = 8.0;
const HISTORY_ROW: f32 = 28.0;
const WHEEL_LABEL: f32 = 20.0;
const BUTTON: f32 = 30.0;
/// A note's line height.
const NOTE_LINE: f32 = 20.0;

/// A note's lines, each clipped to the row, the first in the text colour, the rest dimmer.
#[allow(clippy::too_many_arguments)]
fn draw_note(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    r: Rect,
    rtl: bool,
    ts: f32,
    t: &str,
) {
    let g = ts.max(1.0);
    let pad = (9.0 * g).round();
    let style = TextStyle::new(metrics::FONT_LABEL * ts);
    let line = NOTE_LINE * ts;
    dl.push_clip(r);
    for (i, l) in t.lines().enumerate() {
        let dir = if rtl {
            specular_i18n::Direction::Rtl
        } else {
            specular_i18n::Direction::Ltr
        };
        let l = specular_i18n::isolate_label(l, dir);
        let b = text.layout(&l, style, None, Align::Start);
        let x = if rtl {
            r.right() - pad - b.width.min(r.w - 2.0 * pad)
        } else {
            r.x + pad
        };
        let y = r.y + u_round(3.0 * g) + line * i as f32 + (line - b.height) * 0.5;
        let c = if i == 0 { pal.text } else { pal.text_muted };
        text.draw(dl, &b, x, y, c);
    }
    dl.pop_clip();
}

fn u_round(v: f32) -> f32 {
    v.round()
}
/// A second press within this long (s) and this close (px) is a double-click.
const DOUBLE_S: f64 = 0.4;
const DOUBLE_PX: f32 = 6.0;

/// A control's kind and value.
#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    Slider(Slider),
    Curve(Curve),
    /// Colour wheels, each with its luminance slider.
    Wheels(Vec<(Wheel, Slider)>),
    /// A choice among named options, or a button ([`crate::choice`]).
    Choice(Choice),
    /// Text to read, one row per line (`\n`, not wrapped: clipped to the card); no keyboard
    /// stop (another tool's edit).
    Note(String),
}

/// A control of a card; `id` is the application's.
#[derive(Clone, Debug, PartialEq)]
pub struct Control {
    pub id: u32,
    pub body: Body,
}

/// A module card: a header that opens and closes it, a switch (when the module can be
/// turned off), and its controls.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub id: u32,
    pub title: String,
    /// `None`: no switch (always on).
    pub enabled: Option<bool>,
    pub expanded: bool,
    pub controls: Vec<Control>,
}

/// The header's buttons, in reading order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PanelButton {
    Undo,
    Redo,
    BeforeAfter,
    /// Opens Export (the same as Ctrl+Shift+E).
    Export,
    Close,
}

const BUTTONS: [PanelButton; 5] = [
    PanelButton::Undo,
    PanelButton::Redo,
    PanelButton::BeforeAfter,
    PanelButton::Export,
    PanelButton::Close,
];

/// Something in the panel the keyboard or the pointer can be on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Item {
    Button(PanelButton),
    /// A card's header (card index; `cards.len()` is the history's).
    Header(usize),
    /// A card's ⋯ button, which opens its menu (cards listed in [`DevelopPanel::menus`]).
    More(usize),
    Switch(usize),
    /// A part of a control: a slider or curve is part 0; wheel `i` is part `2i`, its
    /// luminance slider `2i + 1`.
    Control {
        card: usize,
        control: usize,
        part: u8,
    },
    /// The history list (one stop for the keyboard).
    History,
    /// A row of the history (pointer only).
    HistoryRow(usize),
}

/// What the panel asks the application to do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// A control's value changed: read it with [`DevelopPanel::control`]. `done`: the end of
    /// a drag, a key or a typed value (a drag in progress sends `false`).
    Value {
        control: u32,
        part: u8,
        done: bool,
    },
    /// A card's switch.
    Enabled {
        card: u32,
        on: bool,
    },
    /// A card's ⋯ button: open the card's menu (the one a right-click on its header opens)
    /// at `at` (logical px). `item` is the card's header.
    Menu {
        item: Item,
        at: (f32, f32),
    },
    /// Show the edit as it was after history row `n` (0: the original).
    GoTo(usize),
    Button(PanelButton),
    /// The keyboard went back to the image (Esc).
    Left,
}

#[derive(Clone, Copy, Debug)]
struct Drag {
    item: Item,
    last: (f32, f32),
    rect: Rect,
}

/// Where things were drawn (logical px).
#[derive(Clone, Debug, Default)]
struct Layout {
    panel: Rect,
    header: Rect,
    body: Rect,
    /// The scope dock's room under the header.
    dock: Rect,
    title: Rect,
    content_h: f32,
    items: Vec<(Rect, Item)>,
    /// Card frames (card index; `cards.len()` the history's).
    cards: Vec<(usize, Rect)>,
    /// Wheel labels: (rect, card, control, wheel).
    wheel_labels: Vec<(Rect, usize, usize, usize)>,
    /// The history list's box.
    history: Option<Rect>,
    /// The scrollbar, when the cards do not fit.
    bar: Option<Geometry>,
}

/// Room after a card's title for its changed dot (logical px).
const DOT_SPACE: f32 = 12.0;

/// Develop's panel; see the module docs.
#[derive(Clone, Debug, Default)]
pub struct DevelopPanel {
    pub title: String,
    pub cards: Vec<Card>,
    /// The header's buttons, in reading order (all of them for Develop).
    pub buttons: Vec<PanelButton>,
    /// The history card shows (Develop's; other panels built on this one leave it out).
    pub show_history: bool,
    /// History rows, the first the original.
    pub history: Vec<String>,
    /// The row the edit is at.
    pub history_at: usize,
    pub history_expanded: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    /// Before/after shows the original.
    pub before: bool,
    /// Room kept under the header for the scope dock (logical px; 0: none).
    pub dock: f32,
    /// The cards (by id) whose header has a ⋯ button for their menu ([`Event::Menu`]).
    pub menus: Vec<u32>,
    /// The card (by id) whose ⋯ menu is open: its button says so to a screen reader.
    pub menu_open: Option<u32>,
    /// The cards (by id) whose settings differ from a fresh edit's: a dot after the title.
    pub changed: Vec<u32>,
    /// The cards (by id) left out: not drawn, no keyboard stops, not in the accessibility
    /// tree (tools not added yet). Their controls still hold values.
    pub hidden: Vec<u32>,
    /// Controls that cannot change the picture now, by id, with why (shown dimmed; the
    /// reason is their tooltip and their screen-reader description). They still work.
    pub dimmed: Vec<(u32, String)>,
    /// The time (s) the app draws at: a dimmed control's reason shows after the tooltip
    /// wait ([`DevelopPanel::tip_wake`]).
    pub clock: f64,
    /// The item under the pointer and the keyboard's, each with when it got there (stamped
    /// at the first draw after).
    tip_hover: (Option<Item>, f64),
    tip_focus: (Option<Item>, f64),
    /// Where the last draw put a dimmed control's reason.
    tip_at: Option<Rect>,
    focus: Option<Item>,
    /// The focus came from a press: no ring until a key moves it (the ring shows where the
    /// keyboard is; after a click the pointer already shows that).
    pressed: bool,
    hover: Option<Item>,
    drag: Option<Drag>,
    scroll: f32,
    /// Dragging the scrollbar's thumb: where it was grabbed, from the thumb's top.
    bar_grab: Option<f32>,
    /// The pointer is over the scrollbar (it widens).
    bar_hover: bool,
    /// Scroll the focused item into view at the next draw.
    reveal: bool,
    layout: Layout,
    last_click: Option<(Item, f64, (f32, f32))>,
    rtl: bool,
    ts: f32,
}

impl DevelopPanel {
    pub fn new(title: impl Into<String>, cards: Vec<Card>) -> DevelopPanel {
        DevelopPanel {
            title: title.into(),
            cards,
            buttons: BUTTONS.to_vec(),
            show_history: true,
            history: Vec::new(),
            history_expanded: true,
            ..Default::default()
        }
    }

    /// Where the scope dock goes (under the header, the panel's width; from the last draw).
    pub fn dock_rect(&self) -> Rect {
        self.layout.dock
    }

    /// The panel's width for a window `w` wide at text scale `ts` (logical px).
    pub fn width(w: f32, ts: f32) -> f32 {
        (metrics::PANEL_WIDTH * ts.max(1.0))
            .round()
            .min((w * 0.45).round())
    }

    pub fn control(&self, id: u32) -> Option<&Control> {
        self.cards
            .iter()
            .flat_map(|c| c.controls.iter())
            .find(|c| c.id == id)
    }

    pub fn control_mut(&mut self, id: u32) -> Option<&mut Control> {
        self.cards
            .iter_mut()
            .flat_map(|c| c.controls.iter_mut())
            .find(|c| c.id == id)
    }

    /// The slider of control `id`, when it is one.
    pub fn slider_mut(&mut self, id: u32) -> Option<&mut Slider> {
        match &mut self.control_mut(id)?.body {
            Body::Slider(s) => Some(s),
            _ => None,
        }
    }

    pub fn card_mut(&mut self, id: u32) -> Option<&mut Card> {
        self.cards.iter_mut().find(|c| c.id == id)
    }

    /// Where the keyboard is (`None`: not in the panel).
    pub fn focus(&self) -> Option<Item> {
        self.focus
    }

    /// Gives the panel the keyboard on `item` (`None`: its first control).
    pub fn set_focus(&mut self, item: Option<Item>) {
        let item = item.or_else(|| self.order().first().copied());
        // Put back where it was (a rebuilt panel): ringed as before; anywhere else: ringed.
        if item != self.focus {
            self.pressed = false;
        }
        self.focus = item;
        self.reveal = true;
    }

    /// Takes the keyboard away (typing is cancelled).
    pub fn blur(&mut self) {
        self.cancel_typing();
        self.focus = None;
        self.pressed = false;
    }

    /// Whether `it` has the keyboard and shows it (its focus ring).
    fn ringed(&self, it: Item) -> bool {
        self.focus == Some(it) && !self.pressed
    }

    /// What the pointer is over, as of the last move.
    pub fn hovered(&self) -> Option<Item> {
        self.hover
    }

    /// What is at `p` (logical px) as last drawn (a right-click's target).
    pub fn item_at(&self, p: (f32, f32)) -> Option<Item> {
        self.hit(p)
    }

    /// Where `item` was last drawn (a menu opened from the keyboard goes there).
    pub fn item_rect(&self, item: Item) -> Option<Rect> {
        self.rect_of(item)
    }

    /// The card and control ids of `item` (`None` for the header buttons and the history).
    pub fn ids_of(&self, item: Item) -> Option<(u32, Option<u32>)> {
        match item {
            Item::Header(c) | Item::More(c) | Item::Switch(c) => {
                self.cards.get(c).map(|c| (c.id, None))
            }
            Item::Control { card, control, .. } => {
                let c = self.cards.get(card)?;
                Some((c.id, c.controls.get(control).map(|x| x.id)))
            }
            _ => None,
        }
    }

    /// Whether `p` (logical px) is over the panel as last drawn.
    pub fn contains(&self, p: (f32, f32)) -> bool {
        self.layout.panel.contains(p)
    }

    /// The panel's rectangle as last drawn.
    pub fn rect(&self) -> Rect {
        self.layout.panel
    }

    pub fn dragging(&self) -> bool {
        self.drag.is_some() || self.bar_grab.is_some()
    }

    /// Whether the pointer is over the scrollbar or drags its thumb (it is drawn wider).
    pub fn bar_hot(&self) -> bool {
        self.bar_hover || self.bar_grab.is_some()
    }

    /// Every keyboard stop, in reading order.
    pub fn order(&self) -> Vec<Item> {
        let mut v: Vec<Item> = self.buttons.iter().map(|b| Item::Button(*b)).collect();
        for (ci, c) in self.cards.iter().enumerate() {
            if self.hidden.contains(&c.id) {
                continue;
            }
            v.push(Item::Header(ci));
            if self.menus.contains(&c.id) {
                v.push(Item::More(ci));
            }
            if c.enabled.is_some() {
                v.push(Item::Switch(ci));
            }
            if !c.expanded {
                continue;
            }
            for (k, ctl) in c.controls.iter().enumerate() {
                let parts = match &ctl.body {
                    Body::Slider(_) | Body::Curve(_) | Body::Choice(_) => 1,
                    Body::Note(_) => 0,
                    Body::Wheels(w) => 2 * w.len() as u8,
                };
                for part in 0..parts {
                    v.push(Item::Control {
                        card: ci,
                        control: k,
                        part,
                    });
                }
            }
        }
        if !self.show_history {
            return v;
        }
        v.push(Item::Header(self.cards.len()));
        if self.history_expanded && !self.history.is_empty() {
            v.push(Item::History);
        }
        v
    }

    fn cancel_typing(&mut self) {
        for c in &mut self.cards {
            for ctl in &mut c.controls {
                match &mut ctl.body {
                    Body::Slider(s) => {
                        s.end_typing(false);
                    }
                    Body::Wheels(w) => {
                        for (_, s) in w {
                            s.end_typing(false);
                        }
                    }
                    Body::Curve(_) | Body::Choice(_) | Body::Note(_) => {}
                }
            }
        }
    }

    /// Sets what is being typed into the focused slider; its event when that changed it.
    fn commit_typing(&mut self) -> Option<Event> {
        let item = self.focus?;
        let (id, s) = self.slider_of(item)?;
        s.end_typing(true).then_some(Event::Value {
            control: id,
            part: part_of(item),
            done: true,
        })
    }

    /// The slider an item is, with its control's id.
    fn slider_of(&mut self, item: Item) -> Option<(u32, &mut Slider)> {
        let Item::Control {
            card,
            control,
            part,
        } = item
        else {
            return None;
        };
        let ctl = self.cards.get_mut(card)?.controls.get_mut(control)?;
        let id = ctl.id;
        match &mut ctl.body {
            Body::Slider(s) => Some((id, s)),
            Body::Wheels(w) if part % 2 == 1 => {
                Some((id, &mut w.get_mut(usize::from(part / 2))?.1))
            }
            _ => None,
        }
    }

    fn control_at(&mut self, card: usize, control: usize) -> Option<&mut Control> {
        self.cards.get_mut(card)?.controls.get_mut(control)
    }

    fn toggle_card(&mut self, ci: usize) {
        if ci == self.cards.len() {
            self.history_expanded = !self.history_expanded;
        } else if let Some(c) = self.cards.get_mut(ci) {
            c.expanded = !c.expanded;
        }
    }

    fn toggle_switch(&mut self, ci: usize) -> Option<Event> {
        let c = self.cards.get_mut(ci)?;
        let on = !c.enabled?;
        c.enabled = Some(on);
        Some(Event::Enabled { card: c.id, on })
    }

    /// A key while the panel has the keyboard; `None` when it is not the panel's.
    pub fn key(&mut self, combo: KeyCombo) -> Option<Vec<Event>> {
        let item = self.focus?;
        self.pressed = false;
        let rtl = self.rtl;
        let m = combo.mods;
        if combo.key == Key::Named(NamedKey::Tab) && !m.ctrl && !m.alt {
            let ev: Vec<Event> = self.commit_typing().into_iter().collect();
            self.cancel_typing();
            let order = self.order();
            let n = order.len() as isize;
            let at = order.iter().position(|i| *i == item).unwrap_or(0) as isize;
            let next = (at + if m.shift { -1 } else { 1 }).rem_euclid(n.max(1));
            self.focus = order.get(next as usize).copied();
            self.reveal = true;
            return Some(ev);
        }
        let typing = self
            .slider_of(item)
            .is_some_and(|(_, s)| s.typing().is_some());
        if combo.key == Key::Named(NamedKey::Escape) && !typing {
            self.focus = None;
            return Some(vec![Event::Left]);
        }
        let press = matches!(combo.key, Key::Named(NamedKey::Enter | NamedKey::Space)) && !m.any();
        match item {
            Item::Button(b) if press => Some(vec![Event::Button(b)]),
            Item::Header(ci) if press => {
                self.toggle_card(ci);
                Some(Vec::new())
            }
            Item::Switch(ci) if press => Some(self.toggle_switch(ci).into_iter().collect()),
            Item::More(ci) if press => Some(vec![self.menu_event(ci)]),
            Item::History => {
                let last = self.history.len().saturating_sub(1);
                let to = match combo.key {
                    Key::Named(NamedKey::ArrowUp) => self.history_at.saturating_sub(1),
                    Key::Named(NamedKey::ArrowDown) => (self.history_at + 1).min(last),
                    Key::Named(NamedKey::Home) => 0,
                    Key::Named(NamedKey::End) => last,
                    _ => return None,
                };
                Some(if to != self.history_at {
                    vec![Event::GoTo(to)]
                } else {
                    Vec::new()
                })
            }
            Item::Control {
                card,
                control,
                part,
            } => {
                let ctl = self.control_at(card, control)?;
                let id = ctl.id;
                let r = match &mut ctl.body {
                    Body::Slider(s) => s.key(combo, rtl),
                    Body::Curve(c) => c.key(combo),
                    Body::Choice(c) => c.key(combo, rtl),
                    Body::Note(_) => KeyResult::Ignored,
                    Body::Wheels(w) => {
                        let (wheel, lum) = w.get_mut(usize::from(part / 2))?;
                        if part % 2 == 0 {
                            wheel.key(combo)
                        } else {
                            lum.key(combo, rtl)
                        }
                    }
                };
                match r {
                    KeyResult::Ignored => None,
                    KeyResult::Handled => Some(Vec::new()),
                    KeyResult::Changed => Some(vec![Event::Value {
                        control: id,
                        part,
                        done: true,
                    }]),
                }
            }
            _ => None,
        }
    }

    fn hit(&self, p: (f32, f32)) -> Option<Item> {
        if !self.layout.panel.contains(p) {
            return None;
        }
        self.layout
            .items
            .iter()
            .rev()
            .find(|(r, it)| {
                let r = match it {
                    // The thumb reaches past the row a little.
                    Item::Control { .. } => r.inset(-2.0),
                    _ => *r,
                };
                // Scrolled items only where the body shows them.
                let visible = matches!(it, Item::Button(_)) || self.layout.body.contains(p);
                visible && r.contains(p)
            })
            .map(|(_, i)| *i)
    }

    fn rect_of(&self, item: Item) -> Option<Rect> {
        self.layout
            .items
            .iter()
            .find(|(_, i)| *i == item)
            .map(|(r, _)| *r)
    }

    /// Card `ci`'s menu, opened under its ⋯ button (at the button's start edge, so it
    /// reads down from where it was asked for).
    fn menu_event(&self, ci: usize) -> Event {
        let at = self.rect_of(Item::More(ci)).map_or((0.0, 0.0), |r| {
            (if self.rtl { r.right() } else { r.x }, r.bottom())
        });
        Event::Menu {
            item: Item::Header(ci),
            at,
        }
    }

    /// The primary button went down at `p` at time `now` (s); `fine`: Shift held. `None`
    /// when `p` is not over the panel.
    pub fn press(&mut self, p: (f32, f32), fine: bool, now: f64) -> Option<Vec<Event>> {
        let _ = fine;
        if !self.layout.panel.contains(p) {
            // A press elsewhere sets what was typed.
            let ev = self.commit_typing();
            self.cancel_typing();
            return ev.map(|e| vec![e]);
        }
        let mut ev: Vec<Event> = Vec::new();
        // The scrollbar: grab the thumb, or page towards the pointer.
        if let Some(g) = self.layout.bar.filter(|g| g.track.contains(p)) {
            ev.extend(self.commit_typing());
            self.cancel_typing();
            let top = g.thumb_top(self.scroll);
            if p.1 < top {
                self.scroll -= g.page();
            } else if p.1 > top + g.thumb_len() {
                self.scroll += g.page();
            } else {
                self.bar_grab = Some(p.1 - top);
            }
            self.clamp_scroll();
            return Some(ev);
        }
        let Some(item) = self.hit(p) else {
            return Some(ev);
        };
        if self.focus != Some(item) {
            ev.extend(self.commit_typing());
            self.cancel_typing();
        }
        let double = self.last_click.is_some_and(|(i, t, q)| {
            i == item
                && now - t < DOUBLE_S
                && (q.0 - p.0).abs() < DOUBLE_PX
                && (q.1 - p.1).abs() < DOUBLE_PX
        });
        self.last_click = if double { None } else { Some((item, now, p)) };
        let rect = self.rect_of(item).unwrap_or_default();
        let rtl = self.rtl;
        let ts = self.ts;
        self.focus = Some(match item {
            Item::HistoryRow(_) => Item::History,
            i => i,
        });
        self.pressed = true;
        match item {
            Item::Button(b) => ev.push(Event::Button(b)),
            Item::Header(ci) => self.toggle_card(ci),
            Item::Switch(ci) => ev.extend(self.toggle_switch(ci)),
            Item::More(ci) => ev.push(self.menu_event(ci)),
            Item::HistoryRow(i) => {
                if i != self.history_at {
                    ev.push(Event::GoTo(i));
                }
            }
            Item::History => {}
            Item::Control {
                card,
                control,
                part,
            } => {
                let Some(ctl) = self.control_at(card, control) else {
                    return Some(ev);
                };
                let id = ctl.id;
                if let Body::Choice(c) = &mut ctl.body {
                    // Every click steps (or presses); nothing to drag.
                    if c.press() {
                        ev.push(Event::Value {
                            control: id,
                            part,
                            done: true,
                        });
                    }
                    self.last_click = None;
                    return Some(ev);
                }
                let changed = match &mut ctl.body {
                    Body::Slider(s) => press_slider(s, p, rect, rtl, ts, double),
                    Body::Curve(c) => {
                        if double {
                            c.double_click(p, rect)
                        } else {
                            c.press(p, rect)
                        }
                    }
                    Body::Wheels(w) => {
                        let Some((wheel, lum)) = w.get_mut(usize::from(part / 2)) else {
                            return Some(ev);
                        };
                        if part % 2 == 1 {
                            press_slider(lum, p, rect, rtl, ts, double)
                        } else if double {
                            wheel.release();
                            wheel.reset()
                        } else if wheels::on_disc(p, rect) {
                            wheel.press(p, rect)
                        } else {
                            false
                        }
                    }
                    Body::Choice(_) | Body::Note(_) => false,
                };
                if !double {
                    self.drag = Some(Drag {
                        item,
                        last: p,
                        rect,
                    });
                }
                if changed {
                    ev.push(Event::Value {
                        control: id,
                        part,
                        done: double,
                    });
                }
            }
        }
        Some(ev)
    }

    /// The pointer moved to `p` (`None`: left the window); `fine`: Shift held.
    pub fn pointer_move(&mut self, p: Option<(f32, f32)>, fine: bool) -> Vec<Event> {
        self.hover = p.and_then(|p| self.hit(p));
        self.bar_hover = p.is_some_and(|p| self.layout.bar.is_some_and(|g| g.track.contains(p)));
        if let (Some(p), Some(grab), Some(g)) = (p, self.bar_grab, self.layout.bar) {
            self.scroll = g.scroll_for_top(p.1 - grab);
            self.clamp_scroll();
            return Vec::new();
        }
        let (Some(p), Some(mut d)) = (p, self.drag) else {
            return Vec::new();
        };
        let (dx, dy) = (p.0 - d.last.0, p.1 - d.last.1);
        d.last = p;
        self.drag = Some(d);
        let rtl = self.rtl;
        let Item::Control {
            card,
            control,
            part,
        } = d.item
        else {
            return Vec::new();
        };
        let track_w = slider::track_of(d.rect, self.ts).w;
        let Some(ctl) = self.control_at(card, control) else {
            return Vec::new();
        };
        let id = ctl.id;
        let changed = match &mut ctl.body {
            Body::Slider(s) => s.drag(dx, track_w, fine, rtl),
            Body::Curve(c) => c.drag_to(p, d.rect),
            Body::Wheels(w) => match w.get_mut(usize::from(part / 2)) {
                Some((_, lum)) if part % 2 == 1 => lum.drag(dx, track_w, fine, rtl),
                Some((wheel, _)) => wheel.drag_by(dx, dy, d.rect, fine),
                None => false,
            },
            Body::Choice(_) | Body::Note(_) => false,
        };
        if changed {
            vec![Event::Value {
                control: id,
                part,
                done: false,
            }]
        } else {
            Vec::new()
        }
    }

    /// The primary button was let go; whether it ends a drag, and its last event.
    pub fn release(&mut self) -> Option<Vec<Event>> {
        if self.bar_grab.take().is_some() {
            return Some(Vec::new());
        }
        let d = self.drag.take()?;
        let Item::Control {
            card,
            control,
            part,
        } = d.item
        else {
            return Some(Vec::new());
        };
        let Some(ctl) = self.control_at(card, control) else {
            return Some(Vec::new());
        };
        let id = ctl.id;
        match &mut ctl.body {
            Body::Curve(c) => {
                c.release();
            }
            Body::Wheels(w) => {
                for (wheel, _) in w {
                    wheel.release();
                }
            }
            Body::Slider(_) | Body::Choice(_) | Body::Note(_) => {}
        }
        Some(vec![Event::Value {
            control: id,
            part,
            done: true,
        }])
    }

    /// A wheel turn of `dy` lines over `p`: scrolls the panel. Whether it was over it.
    pub fn wheel(&mut self, p: (f32, f32), dy: f32) -> bool {
        if !self.layout.panel.contains(p) {
            return false;
        }
        self.scroll -= dy * 40.0;
        self.clamp_scroll();
        true
    }

    fn clamp_scroll(&mut self) {
        let max = (self.layout.content_h - self.layout.body.h).max(0.0);
        self.scroll = self.scroll.clamp(0.0, max);
    }

    /// A screen reader's request on node `id` (an id from [`a11y`](Self::a11y)); `value` is
    /// a set-value request's number. `None` when the id is not the panel's.
    pub fn a11y_action(
        &mut self,
        base: u64,
        id: u64,
        action: accesskit::Action,
        value: Option<f64>,
    ) -> Option<Vec<Event>> {
        use accesskit::Action as A;
        let item = self
            .order_with_rows()
            .into_iter()
            .find(|i| node_id(base, *i) == id)?;
        let rtl = self.rtl;
        match action {
            A::Focus => {
                self.pressed = false;
                self.focus = Some(match item {
                    Item::HistoryRow(_) => Item::History,
                    i => i,
                });
                self.reveal = true;
                Some(Vec::new())
            }
            A::Click => match item {
                Item::Button(b) => Some(vec![Event::Button(b)]),
                Item::Header(ci) => {
                    self.toggle_card(ci);
                    Some(Vec::new())
                }
                Item::Switch(ci) => Some(self.toggle_switch(ci).into_iter().collect()),
                Item::More(ci) => Some(vec![self.menu_event(ci)]),
                Item::HistoryRow(i) => Some(vec![Event::GoTo(i)]),
                Item::Control { card, control, .. } => {
                    let ctl = self.control_at(card, control)?;
                    let id = ctl.id;
                    let Body::Choice(c) = &mut ctl.body else {
                        return None;
                    };
                    Some(if c.press() {
                        vec![Event::Value {
                            control: id,
                            part: 0,
                            done: true,
                        }]
                    } else {
                        Vec::new()
                    })
                }
                _ => None,
            },
            A::Increment | A::Decrement | A::SetValue => {
                let part = part_of(item);
                let (cid, s) = self.slider_of(item)?;
                let changed = match (action, value) {
                    (A::Increment, _) => s.nudge(1.0),
                    (A::Decrement, _) => s.nudge(-1.0),
                    (_, Some(v)) => s.set(v as f32),
                    _ => false,
                };
                let _ = rtl;
                Some(if changed {
                    vec![Event::Value {
                        control: cid,
                        part,
                        done: true,
                    }]
                } else {
                    Vec::new()
                })
            }
            _ => None,
        }
    }

    fn order_with_rows(&self) -> Vec<Item> {
        let mut v = self.order();
        v.extend((0..self.history.len()).map(Item::HistoryRow));
        v
    }

    /// Lays the panel out in `area` (the window, logical px) at text scale `ts`; `title_w`:
    /// the title's width. A title that does not fit beside the buttons keeps the header's
    /// first row to itself and the buttons move to a second row.
    fn lay_out(&self, text: &mut Text, area: Rect, ts: f32, rtl: bool, title_w: f32) -> Layout {
        let g = ts.max(1.0);
        let u = |v: f32| (v * g).round();
        let w = DevelopPanel::width(area.w, ts);
        let panel = Rect::new(
            if rtl { area.x } else { area.right() - w },
            area.y,
            w,
            area.h,
        );
        // Laid out left to right from x = 0, then placed (mirrored in right to left).
        let place = |r: Rect| {
            let x = if rtl { w - r.x - r.w } else { r.x };
            Rect::new(panel.x + x, r.y, r.w, r.h)
        };
        let pad = u(metrics::PAD);
        let mut items = Vec::new();
        let b = u(BUTTON);
        let nb = self.buttons.len();
        let mut bx = w - pad - b * nb as f32 - u(4.0) * nb.saturating_sub(1) as f32;
        let beside = (bx - 2.0 * pad).max(0.0);
        let wrap = nb > 0 && title_w > beside;
        let (header, title, by) = if wrap {
            let header = Rect::new(panel.x, panel.y, w, u(HEADER) + b + u(6.0));
            let title = place(Rect::new(pad, header.y, w - 2.0 * pad, u(HEADER)));
            (header, title, header.y + u(HEADER) - u(4.0))
        } else {
            let header = Rect::new(panel.x, panel.y, w, u(HEADER));
            let title = place(Rect::new(pad, header.y, beside, header.h));
            (header, title, header.y + (header.h - b) * 0.5)
        };
        for &btn in &self.buttons {
            items.push((place(Rect::new(bx, by, b, b)), Item::Button(btn)));
            bx += b + u(4.0);
        }
        let dock = Rect::new(panel.x, header.bottom(), w, self.dock.max(0.0).round());
        let body = Rect::new(
            panel.x,
            dock.bottom(),
            w,
            (panel.h - header.h - dock.h).max(0.0),
        );
        let mut y = body.y + u(4.0) - self.scroll;
        let y0 = y;
        let card_x = pad;
        let card_w = w - 2.0 * pad;
        let inner_x = card_x + u(4.0);
        let inner_w = card_w - u(8.0);
        let mut cards = Vec::new();
        let mut wheel_labels = Vec::new();
        let sw = u(34.0);
        for (ci, c) in self.cards.iter().enumerate() {
            if self.hidden.contains(&c.id) {
                continue;
            }
            let top = y;
            let hh = u(CARD_HEADER);
            let switch_w = if c.enabled.is_some() {
                sw + u(8.0)
            } else {
                0.0
            };
            let more = self.menus.contains(&c.id);
            let more_w = if more { u(32.0) } else { 0.0 };
            items.push((
                place(Rect::new(card_x, y, card_w - switch_w - more_w, hh)),
                Item::Header(ci),
            ));
            if more {
                // Square, centred on the header's height, just before the switch.
                let side = u(28.0);
                items.push((
                    place(Rect::new(
                        card_x + card_w - switch_w - more_w + (more_w - side) * 0.5,
                        y + (hh - side) * 0.5,
                        side,
                        side,
                    )),
                    Item::More(ci),
                ));
            }
            if c.enabled.is_some() {
                items.push((
                    place(Rect::new(card_x + card_w - sw - u(8.0), y, sw + u(4.0), hh)),
                    Item::Switch(ci),
                ));
            }
            y += hh;
            if c.expanded {
                for (k, ctl) in c.controls.iter().enumerate() {
                    match &ctl.body {
                        Body::Slider(_) | Body::Choice(_) => {
                            // A label that wraps makes its row taller (never over the
                            // track or the next row).
                            let h = match &ctl.body {
                                Body::Choice(c) => c.row_height(text, inner_w, ts, rtl),
                                Body::Slider(s) => s.row_height(text, inner_w, ts),
                                _ => u(slider::ROW),
                            };
                            items.push((
                                place(Rect::new(inner_x, y, inner_w, h)),
                                Item::Control {
                                    card: ci,
                                    control: k,
                                    part: 0,
                                },
                            ));
                            y += h;
                        }
                        Body::Note(t) => {
                            let h = u(NOTE_LINE) * t.lines().count().max(1) as f32 + u(6.0);
                            items.push((
                                place(Rect::new(inner_x, y, inner_w, h)),
                                Item::Control {
                                    card: ci,
                                    control: k,
                                    part: 0,
                                },
                            ));
                            y += h;
                        }
                        Body::Curve(_) => {
                            let side = inner_w.min(u(240.0)) - u(16.0);
                            let x = inner_x + (inner_w - side) * 0.5;
                            y += u(6.0);
                            // The square is never mirrored, only placed.
                            items.push((
                                place(Rect::new(x, y, side, side)),
                                Item::Control {
                                    card: ci,
                                    control: k,
                                    part: 0,
                                },
                            ));
                            y += side + u(10.0);
                        }
                        Body::Wheels(ws) => {
                            // Up to three in a row; four as two rows of two.
                            let per_row = if ws.len() > 3 { 2 } else { ws.len().max(1) };
                            let n = per_row as f32;
                            let gap = u(6.0);
                            let col = (inner_w - gap * (n - 1.0)) / n;
                            let disc = (col - u(12.0)).min(u(120.0)).max(u(40.0));
                            let row_h = u(4.0)
                                + u(WHEEL_LABEL)
                                + u(2.0)
                                + disc
                                + u(2.0)
                                + u(slider::ROW)
                                + u(4.0);
                            for (i, _) in ws.iter().enumerate() {
                                let cx = inner_x + (i % per_row) as f32 * (col + gap);
                                let ry = y + (i / per_row) as f32 * row_h;
                                let lab = Rect::new(cx, ry + u(4.0), col, u(WHEEL_LABEL));
                                wheel_labels.push((place(lab), ci, k, i));
                                let d = Rect::new(
                                    cx + (col - disc) * 0.5,
                                    lab.bottom() + u(2.0),
                                    disc,
                                    disc,
                                );
                                items.push((
                                    place(d),
                                    Item::Control {
                                        card: ci,
                                        control: k,
                                        part: 2 * i as u8,
                                    },
                                ));
                                let s = Rect::new(cx, d.bottom() + u(2.0), col, u(slider::ROW));
                                items.push((
                                    place(s),
                                    Item::Control {
                                        card: ci,
                                        control: k,
                                        part: 2 * i as u8 + 1,
                                    },
                                ));
                            }
                            y += row_h * ws.len().div_ceil(per_row) as f32;
                        }
                    }
                }
                y += u(6.0);
            }
            cards.push((ci, place(Rect::new(card_x, top, card_w, y - top))));
            y += u(GAP);
        }
        // History.
        let mut history = None;
        if self.show_history {
            let hi = self.cards.len();
            let top = y;
            let hh = u(CARD_HEADER);
            items.push((place(Rect::new(card_x, y, card_w, hh)), Item::Header(hi)));
            y += hh;
            if self.history_expanded && !self.history.is_empty() {
                let rh = u(HISTORY_ROW);
                let list = Rect::new(inner_x, y, inner_w, rh * self.history.len() as f32);
                items.push((place(list), Item::History));
                for i in 0..self.history.len() {
                    items.push((
                        place(Rect::new(inner_x, y + rh * i as f32, inner_w, rh)),
                        Item::HistoryRow(i),
                    ));
                }
                history = Some(place(list));
                y = list.bottom() + u(6.0);
            }
            cards.push((hi, place(Rect::new(card_x, top, card_w, y - top))));
            y += u(GAP);
        }
        Layout {
            panel,
            header,
            body,
            dock,
            title,
            content_h: y - y0,
            items,
            cards,
            wheel_labels,
            history,
            bar: None,
        }
    }

    /// Draws the panel at the window's `area` edge (logical px) at text scale `ts`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        area: Rect,
        rtl: bool,
        ts: f32,
    ) {
        self.rtl = rtl;
        self.ts = ts;
        let tstyle = TextStyle::new(metrics::FONT_TITLE * ts).weight(600.0);
        let title_w = text.measure(&self.title, tstyle);
        let mut l = self.lay_out(text, area, ts, rtl, title_w);
        // Keep the scroll in range, and the keyboard's control in view.
        let max = (l.content_h - l.body.h).max(0.0);
        let mut scroll = self.scroll.clamp(0.0, max);
        if self.reveal {
            self.reveal = false;
            if let Some(r) = self.focus.and_then(|f| {
                l.items
                    .iter()
                    .find(|(_, i)| *i == f && !matches!(f, Item::Button(_)))
                    .map(|(r, _)| *r)
            }) {
                let margin = 8.0;
                if r.y < l.body.y + margin {
                    scroll -= l.body.y + margin - r.y;
                } else if r.bottom() > l.body.bottom() - margin {
                    scroll += r.bottom() - (l.body.bottom() - margin);
                }
                scroll = scroll.clamp(0.0, max);
            }
        }
        if scroll != self.scroll {
            self.scroll = scroll;
            l = self.lay_out(text, area, ts, rtl, title_w);
        }
        let g = ts.max(1.0);
        let u = |v: f32| (v * g).round();
        // The panel itself.
        dl.shadow(l.panel, 0.0, pal.shadow, metrics::SHADOW_BLUR, 0.0);
        dl.rect(l.panel, 0.0, pal.sidebar);
        let edge = if rtl {
            l.panel.right() - 1.0
        } else {
            l.panel.x
        };
        dl.rect(
            Rect::new(edge, l.panel.y, 1.0, l.panel.h),
            0.0,
            pal.separator,
        );
        // Header: the title (cut with "…" only when even a row of its own is too narrow;
        // the screen reader has it whole), the buttons.
        let shown = fit_title(text, &self.title, tstyle, l.title.w);
        let tb = text.layout(&shown, tstyle, None, Align::Start);
        let tx = if rtl {
            l.title.right() - tb.width
        } else {
            l.title.x
        };
        dl.push_clip(l.title);
        text.draw(
            dl,
            &tb,
            tx,
            l.title.y + (l.title.h - tb.height) * 0.5,
            pal.text,
        );
        dl.pop_clip();
        dl.rect(
            Rect::new(l.panel.x, l.header.bottom() - 1.0, l.panel.w, 1.0),
            0.0,
            pal.separator,
        );
        for (r, it) in &l.items {
            let Item::Button(b) = *it else { continue };
            let enabled = match b {
                PanelButton::Undo => self.can_undo,
                PanelButton::Redo => self.can_redo,
                _ => true,
            };
            let on = b == PanelButton::BeforeAfter && self.before;
            let hot = self.hover == Some(*it) && enabled;
            if on {
                dl.rect(*r, metrics::RADIUS_SM, pal.accent_soft);
            } else if hot {
                dl.rect(*r, metrics::RADIUS_SM, pal.surface_hover);
            }
            let (icon, mirror) = match b {
                PanelButton::Undo => (Icon::Undo, rtl),
                PanelButton::Redo => (Icon::Undo, !rtl),
                PanelButton::BeforeAfter => (Icon::Image, false),
                PanelButton::Export => (Icon::Download, false),
                PanelButton::Close => (Icon::Close, false),
            };
            let col = if !enabled {
                pal.text_faint
            } else if on {
                pal.accent
            } else {
                pal.text_muted
            };
            draw_icon(dl, icon, r.inset(u(6.0)), col, metrics::ICON_STROKE, mirror);
            if self.ringed(*it) {
                dl.rect_border(
                    *r,
                    metrics::RADIUS_SM,
                    Color::TRANSPARENT,
                    pal.focus_ring,
                    2.0,
                );
            }
        }
        // Body: the cards, then the history.
        dl.push_clip(l.body);
        for (ci, r) in &l.cards {
            dl.rect(*r, metrics::RADIUS_MD, pal.surface_raised);
            let _ = ci;
        }
        let label = TextStyle::new(metrics::FONT_LABEL * ts).weight(600.0);
        for (r, it) in &l.items {
            match *it {
                Item::Header(ci) => {
                    let (title, expanded, off, changed) = match self.cards.get(ci) {
                        Some(c) => (
                            c.title.clone(),
                            c.expanded,
                            c.enabled == Some(false),
                            self.changed.contains(&c.id),
                        ),
                        None => (t!("develop-history"), self.history_expanded, false, false),
                    };
                    if self.hover == Some(*it) {
                        dl.rect(*r, metrics::RADIUS_MD, pal.surface_hover);
                    }
                    let chev = u(16.0);
                    let cx = if rtl {
                        r.right() - u(10.0) - chev
                    } else {
                        r.x + u(10.0)
                    };
                    let icon = match (expanded, rtl) {
                        (true, _) => Icon::ChevronDown,
                        (false, false) => Icon::ChevronRight,
                        (false, true) => Icon::ChevronLeft,
                    };
                    draw_icon(
                        dl,
                        icon,
                        Rect::new(cx, r.y + (r.h - chev) * 0.5, chev, chev),
                        pal.text_muted,
                        metrics::ICON_STROKE,
                        false,
                    );
                    let dot = u(DOT_SPACE);
                    let avail = (r.w - chev - u(28.0) - if changed { dot } else { 0.0 }).max(0.0);
                    let b = text.layout(&title, label, Some(avail), Align::Start);
                    let tw = b.width.min(avail);
                    let x = if rtl {
                        cx - u(8.0) - tw
                    } else {
                        cx + chev + u(8.0)
                    };
                    // A card that is off: its title muted (its controls show as `dimmed`).
                    let ink = if off { pal.text_muted } else { pal.text };
                    text.draw(dl, &b, x, r.y + (r.h - b.height) * 0.5, ink);
                    if changed {
                        let rad = u(3.0);
                        let dx = if rtl {
                            x - dot * 0.5
                        } else {
                            x + tw + dot * 0.5
                        };
                        let fill = if off { pal.text_muted } else { pal.accent };
                        dl.circle((dx, r.y + r.h * 0.5), rad, fill);
                    }
                    if self.ringed(*it) {
                        dl.rect_border(
                            r.inset(1.0),
                            metrics::RADIUS_MD,
                            Color::TRANSPARENT,
                            pal.focus_ring,
                            2.0,
                        );
                    }
                }
                Item::More(ci) => {
                    // Only while its header has the pointer or the keyboard: a ⋯ on every
                    // card at rest is noise (the same menu is a right-click on the header).
                    let on_row = |i: Option<Item>| match i {
                        Some(Item::Header(c) | Item::More(c) | Item::Switch(c)) => c == ci,
                        _ => false,
                    };
                    if !on_row(self.hover) && !on_row(self.focus) {
                        continue;
                    }
                    if self.hover == Some(*it) {
                        dl.rect(*r, metrics::RADIUS_SM, pal.surface_hover);
                    }
                    let side = u(18.0);
                    let ir = Rect::new(
                        r.x + (r.w - side) * 0.5,
                        r.y + (r.h - side) * 0.5,
                        side,
                        side,
                    );
                    let colour = if self.hover == Some(*it) || self.focus == Some(*it) {
                        pal.text
                    } else {
                        pal.text_muted
                    };
                    draw_icon(dl, Icon::More, ir, colour, metrics::ICON_STROKE, false);
                    if self.ringed(*it) {
                        dl.rect_border(
                            r.inset(1.0),
                            metrics::RADIUS_SM,
                            Color::TRANSPARENT,
                            pal.focus_ring,
                            2.0,
                        );
                    }
                }
                Item::Switch(ci) => {
                    // Small and quiet: whether a card is off shows in the card itself
                    // (muted title, dimmed controls), not in a coloured switch.
                    let on = self.cards.get(ci).and_then(|c| c.enabled).unwrap_or(true);
                    let hot = self.hover == Some(*it);
                    let sw = Rect::new(0.0, 0.0, u(24.0), u(14.0));
                    let sw = Rect::new(
                        r.x + (r.w - sw.w) * 0.5,
                        r.y + (r.h - sw.h) * 0.5,
                        sw.w,
                        sw.h,
                    );
                    let ink = match (on, hot) {
                        (_, true) => pal.text,
                        (true, false) => pal.text_muted,
                        (false, false) => pal.text_faint,
                    };
                    if on {
                        dl.rect(sw, sw.h * 0.5, ink);
                    } else {
                        dl.rect_border(sw, sw.h * 0.5, Color::TRANSPARENT, ink, 1.5);
                    }
                    let kr = sw.h * 0.5 - 3.0;
                    // The knob sits at the reading direction's end when on.
                    let at_end = on != rtl;
                    let kx = if at_end {
                        sw.right() - 3.0 - kr
                    } else {
                        sw.x + 3.0 + kr
                    };
                    let knob = if on { pal.surface_raised } else { ink };
                    dl.circle((kx, sw.center().1), kr, knob);
                    if self.ringed(*it) {
                        dl.rect_border(
                            sw.inset(-3.0),
                            sw.h * 0.5 + 3.0,
                            Color::TRANSPARENT,
                            pal.focus_ring,
                            2.0,
                        );
                    }
                }
                Item::Control {
                    card,
                    control,
                    part,
                } => {
                    let Some(ctl) = self.cards.get(card).and_then(|c| c.controls.get(control))
                    else {
                        continue;
                    };
                    let focused = self.ringed(*it);
                    let hot = self.hover == Some(*it) || self.drag.is_some_and(|d| d.item == *it);
                    let dim = self.dimmed_reason(ctl.id).is_some();
                    match &ctl.body {
                        Body::Slider(s) => {
                            s.draw(dl, text, pal, *r, rtl, ts, focused, hot);
                        }
                        Body::Curve(c) => c.draw(dl, text, pal, *r, ts, focused),
                        Body::Choice(c) => c.draw(dl, text, pal, *r, rtl, ts, focused, hot),
                        Body::Note(t) => draw_note(dl, text, pal, *r, rtl, ts, t),
                        Body::Wheels(ws) => {
                            if let Some((wheel, lum)) = ws.get(usize::from(part / 2)) {
                                if part % 2 == 0 {
                                    wheel.draw(dl, pal, *r, focused);
                                } else {
                                    lum.draw(dl, text, pal, *r, rtl, ts, focused, hot);
                                }
                            }
                        }
                    }
                    // Dimmed: the card's colour over it (the focus ring stays readable).
                    if dim {
                        dl.rect(*r, metrics::RADIUS_SM, pal.surface_raised.with_alpha(0.55));
                        if focused {
                            dl.rect_border(
                                r.inset(1.0),
                                metrics::RADIUS_SM,
                                Color::TRANSPARENT,
                                pal.focus_ring,
                                2.0,
                            );
                        }
                    }
                }
                Item::History => {
                    if self.ringed(*it) {
                        dl.rect_border(
                            r.inset(-2.0),
                            metrics::RADIUS_SM,
                            Color::TRANSPARENT,
                            pal.focus_ring,
                            2.0,
                        );
                    }
                }
                Item::HistoryRow(i) => {
                    let current = i == self.history_at;
                    if current {
                        dl.rect(*r, metrics::RADIUS_SM, pal.accent_soft);
                    } else if self.hover == Some(*it) {
                        dl.rect(*r, metrics::RADIUS_SM, pal.surface_hover);
                    }
                    let s = self.history.get(i).cloned().unwrap_or_default();
                    let style = TextStyle::new(metrics::FONT_LABEL * ts);
                    let avail = r.w - u(16.0);
                    let b = text.layout(&s, style, Some(avail), Align::Start);
                    let x = if rtl {
                        r.right() - u(8.0) - b.width.min(avail)
                    } else {
                        r.x + u(8.0)
                    };
                    // Steps past the current one can be redone: shown muted (still AA).
                    let col = if i > self.history_at {
                        pal.text_muted
                    } else {
                        pal.text
                    };
                    dl.push_clip(*r);
                    text.draw(dl, &b, x, r.y + (r.h - b.height) * 0.5, col);
                    dl.pop_clip();
                }
                Item::Button(_) => {}
            }
        }
        let small = TextStyle::new(metrics::FONT_SMALL * ts);
        for (r, ci, k, i) in &l.wheel_labels {
            let Some(Body::Wheels(ws)) = self
                .cards
                .get(*ci)
                .and_then(|c| c.controls.get(*k))
                .map(|c| &c.body)
            else {
                continue;
            };
            let Some((w, _)) = ws.get(*i) else { continue };
            let b = text.layout(&w.label, small, Some(r.w), Align::Center);
            text.draw(dl, &b, r.x + (r.w - b.width) * 0.5, r.y, pal.text_muted);
        }
        dl.pop_clip();
        // Why a dimmed control does nothing now: under the pointer, else the keyboard's,
        // after the tooltip wait (the screen reader has it at once, as the description).
        if self.tip_hover.0 != self.hover {
            self.tip_hover = (self.hover, self.clock);
        }
        if self.tip_focus.0 != self.focus {
            self.tip_focus = (self.focus, self.clock);
        }
        let wait = f64::from(durations::TOOLTIP_DELAY);
        let tip = [self.tip_hover, self.tip_focus]
            .into_iter()
            .filter(|(_, t)| self.clock - t >= wait)
            .filter_map(|(it, _)| it)
            .find_map(|it| {
                let why = self.dimmed_item(it)?;
                let r = l.items.iter().find(|(_, i)| *i == it)?.0;
                (r.bottom() > l.body.y && r.y < l.body.bottom()).then(|| (why.to_owned(), r))
            });
        self.tip_at =
            tip.map(|(why, r)| tip_beside_row(dl, text, pal, &why, r, l.panel, area, rtl, ts));
        // The scrollbar, when the cards do not fit: the shared one's sizes, always shown,
        // wider while hovered or dragged.
        l.bar = None;
        if l.content_h > l.body.h + 0.5 && l.body.h > bar::MIN_THUMB {
            let track = Rect::new(
                if rtl {
                    l.panel.x + bar::EDGE
                } else {
                    l.panel.right() - bar::EDGE - bar::HIT
                },
                l.body.y + bar::ENDS,
                bar::HIT,
                l.body.h - bar::ENDS * 2.0,
            );
            let g = Geometry {
                track,
                view: l.body.h,
                max: l.content_h - l.body.h,
            };
            let hot = self.bar_hot();
            let w = if hot { bar::WIDE } else { bar::THIN };
            let x = if rtl {
                track.x + bar::EDGE
            } else {
                track.right() - bar::EDGE - w
            };
            if hot {
                let tr = Rect::new(x, track.y, w, track.h);
                dl.rect(tr, w * 0.5, pal.text_faint.with_alpha(0.12));
            }
            let thumb = Rect::new(x, g.thumb_top(self.scroll), w, g.thumb_len());
            let color = if hot { pal.text_muted } else { pal.text_faint };
            dl.rect(thumb, w * 0.5, color);
            l.bar = Some(g);
        }
        self.layout = l;
    }

    /// Why the control at item `it` is dimmed, if it is.
    fn dimmed_item(&self, it: Item) -> Option<&str> {
        let Item::Control { card, control, .. } = it else {
            return None;
        };
        let ctl = self.cards.get(card)?.controls.get(control)?;
        self.dimmed_reason(ctl.id)
    }

    /// When a dimmed control's reason is due (the pointer's or the keyboard's control
    /// waiting out the tooltip delay), so the app draws then; `None` when nothing waits.
    /// From the last draw's stamps.
    pub fn tip_wake(&self) -> Option<f64> {
        let wait = f64::from(durations::TOOLTIP_DELAY);
        [self.tip_hover, self.tip_focus]
            .into_iter()
            .filter(|(it, t)| {
                it.is_some_and(|it| self.dimmed_item(it).is_some()) && self.clock - t < wait
            })
            .map(|(_, t)| t + wait)
            .reduce(f64::min)
    }

    /// Where the last draw showed a dimmed control's reason.
    pub fn tip_rect(&self) -> Option<Rect> {
        self.tip_at
    }

    /// Why the control `id` is dimmed, if it is.
    pub fn dimmed_reason(&self, id: u32) -> Option<&str> {
        self.dimmed
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, why)| why.as_str())
    }

    /// The panel's screen-reader nodes under the window root, from id `base` up; `scale`
    /// is physical px per logical px. Call after [`draw`](Self::draw).
    pub fn a11y(&self, tree: &mut accesskit::TreeUpdate, base: u64, scale: f64) {
        use accesskit::{Action, Node, NodeId, Role, Toggled};
        let bounds = |r: Rect| accesskit::Rect {
            x0: f64::from(r.x) * scale,
            y0: f64::from(r.y) * scale,
            x1: f64::from(r.x + r.w) * scale,
            y1: f64::from(r.y + r.h) * scale,
        };
        let l = &self.layout;
        let mut nodes: Vec<(NodeId, Node)> = Vec::new();
        let mut root = Node::new(Role::Pane);
        root.set_label(self.title.as_str());
        root.set_bounds(bounds(l.panel));
        let mut root_children = Vec::new();
        let rect_of = |it: Item| {
            l.items
                .iter()
                .find(|(_, i)| *i == it)
                .map(|(r, _)| *r)
                .unwrap_or_default()
        };
        let toggled = |on: bool| if on { Toggled::True } else { Toggled::False };
        for &b in &self.buttons {
            let it = Item::Button(b);
            let mut n = Node::new(Role::Button);
            n.set_label(match b {
                PanelButton::Undo => t!("develop-undo"),
                PanelButton::Redo => t!("develop-redo"),
                PanelButton::BeforeAfter => t!("develop-before-after"),
                PanelButton::Export => t!("palette-develop-export"),
                PanelButton::Close => t!("develop-close"),
            });
            if b == PanelButton::BeforeAfter {
                n.set_toggled(toggled(self.before));
            }
            let enabled = match b {
                PanelButton::Undo => self.can_undo,
                PanelButton::Redo => self.can_redo,
                _ => true,
            };
            if enabled {
                n.add_action(Action::Click);
                n.add_action(Action::Focus);
            } else {
                n.set_disabled();
            }
            n.set_bounds(bounds(rect_of(it)));
            let id = NodeId(node_id(base, it));
            root_children.push(id);
            nodes.push((id, n));
        }
        let card_rect = |ci: usize| {
            l.cards
                .iter()
                .find(|(i, _)| *i == ci)
                .map(|(_, r)| *r)
                .unwrap_or_default()
        };
        for (ci, c) in self.cards.iter().enumerate() {
            if self.hidden.contains(&c.id) {
                continue;
            }
            let gid = NodeId(group_id(base, ci));
            let mut grp = Node::new(Role::Group);
            grp.set_label(c.title.as_str());
            grp.set_bounds(bounds(card_rect(ci)));
            let mut kids = Vec::new();
            let hid = Item::Header(ci);
            let mut h = Node::new(Role::Button);
            h.set_label(c.title.as_str());
            h.set_expanded(c.expanded);
            if self.changed.contains(&c.id) {
                h.set_description(t!("develop-card-changed"));
            }
            h.add_action(Action::Click);
            h.add_action(Action::Focus);
            h.set_bounds(bounds(rect_of(hid)));
            kids.push(NodeId(node_id(base, hid)));
            nodes.push((NodeId(node_id(base, hid)), h));
            if self.menus.contains(&c.id) {
                let it = Item::More(ci);
                let mut m = Node::new(Role::Button);
                m.set_label(t!("develop-card-menu", module = c.title.clone()));
                m.set_has_popup(accesskit::HasPopup::Menu);
                m.set_expanded(self.menu_open == Some(c.id));
                m.add_action(Action::Click);
                m.add_action(Action::Focus);
                m.set_bounds(bounds(rect_of(it)));
                kids.push(NodeId(node_id(base, it)));
                nodes.push((NodeId(node_id(base, it)), m));
            }
            if let Some(on) = c.enabled {
                let it = Item::Switch(ci);
                let mut s = Node::new(Role::Switch);
                s.set_label(t!("develop-module-on", module = c.title.clone()));
                s.set_toggled(toggled(on));
                s.add_action(Action::Click);
                s.add_action(Action::Focus);
                s.set_bounds(bounds(rect_of(it)));
                kids.push(NodeId(node_id(base, it)));
                nodes.push((NodeId(node_id(base, it)), s));
            }
            if c.expanded {
                for (k, ctl) in c.controls.iter().enumerate() {
                    let item = |part: u8| Item::Control {
                        card: ci,
                        control: k,
                        part,
                    };
                    let why = self.dimmed_reason(ctl.id);
                    let described = |mut n: Node| {
                        if let Some(w) = why {
                            n.set_description(w);
                        }
                        n
                    };
                    match &ctl.body {
                        Body::Slider(s) => {
                            let it = item(0);
                            kids.push(NodeId(node_id(base, it)));
                            nodes.push((
                                NodeId(node_id(base, it)),
                                described(s.a11y_node(bounds(rect_of(it)))),
                            ));
                        }
                        Body::Curve(cv) => {
                            let it = item(0);
                            kids.push(NodeId(node_id(base, it)));
                            nodes.push((
                                NodeId(node_id(base, it)),
                                described(cv.a11y_node(bounds(rect_of(it)))),
                            ));
                        }
                        Body::Note(t) => {
                            let it = item(0);
                            let mut n = accesskit::Node::new(accesskit::Role::Label);
                            n.set_value(t.replace('\n', " "));
                            n.set_bounds(bounds(rect_of(it)));
                            kids.push(NodeId(node_id(base, it)));
                            nodes.push((NodeId(node_id(base, it)), n));
                        }
                        Body::Choice(ch) => {
                            let it = item(0);
                            kids.push(NodeId(node_id(base, it)));
                            nodes.push((
                                NodeId(node_id(base, it)),
                                described(ch.a11y_node(bounds(rect_of(it)))),
                            ));
                        }
                        Body::Wheels(ws) => {
                            for (i, (w, lum)) in ws.iter().enumerate() {
                                let d = item(2 * i as u8);
                                let s = item(2 * i as u8 + 1);
                                kids.push(NodeId(node_id(base, d)));
                                nodes.push((
                                    NodeId(node_id(base, d)),
                                    described(w.a11y_node(bounds(rect_of(d)))),
                                ));
                                kids.push(NodeId(node_id(base, s)));
                                nodes.push((
                                    NodeId(node_id(base, s)),
                                    described(lum.a11y_node(bounds(rect_of(s)))),
                                ));
                            }
                        }
                    }
                }
            }
            grp.set_children(kids);
            root_children.push(gid);
            nodes.push((gid, grp));
        }
        // History.
        if self.show_history {
            let hi = self.cards.len();
            let gid = NodeId(group_id(base, hi));
            let mut grp = Node::new(Role::Group);
            grp.set_label(t!("develop-history"));
            grp.set_bounds(bounds(card_rect(hi)));
            let mut kids = Vec::new();
            let hid = Item::Header(hi);
            let mut h = Node::new(Role::Button);
            h.set_label(t!("develop-history"));
            h.set_expanded(self.history_expanded);
            h.add_action(Action::Click);
            h.add_action(Action::Focus);
            h.set_bounds(bounds(rect_of(hid)));
            kids.push(NodeId(node_id(base, hid)));
            nodes.push((NodeId(node_id(base, hid)), h));
            if let Some(r) = l.history {
                let lid = NodeId(node_id(base, Item::History));
                let mut list = Node::new(Role::ListBox);
                list.set_label(t!("develop-history"));
                list.set_bounds(bounds(r));
                list.add_action(Action::Focus);
                let mut rows = Vec::new();
                for (i, s) in self.history.iter().enumerate() {
                    let it = Item::HistoryRow(i);
                    let mut n = Node::new(Role::ListBoxOption);
                    n.set_label(s.as_str());
                    n.set_selected(i == self.history_at);
                    n.add_action(Action::Click);
                    n.set_bounds(bounds(rect_of(it)));
                    rows.push(NodeId(node_id(base, it)));
                    nodes.push((NodeId(node_id(base, it)), n));
                }
                list.set_children(rows);
                kids.push(lid);
                nodes.push((lid, list));
            }
            grp.set_children(kids);
            root_children.push(gid);
            nodes.push((gid, grp));
        }
        root.set_children(root_children);
        let win = tree_root(tree);
        if let Some((_, w)) = tree.nodes.iter_mut().find(|(id, _)| *id == win) {
            w.push_child(NodeId(base));
        }
        tree.nodes.push((NodeId(base), root));
        tree.nodes.extend(nodes);
        if let Some(f) = self.focus {
            tree.focus = NodeId(node_id(base, f));
        }
    }
}

fn tree_root(tree: &accesskit::TreeUpdate) -> accesskit::NodeId {
    tree.tree.as_ref().map_or(accesskit::NodeId(1), |t| t.root)
}

/// A press on a slider's row: double-click resets; on the thumb, nothing yet (a drag
/// follows); elsewhere on the track it jumps there.
fn press_slider(
    s: &mut Slider,
    p: (f32, f32),
    row: Rect,
    rtl: bool,
    ts: f32,
    double: bool,
) -> bool {
    if double {
        return s.reset();
    }
    let g = ts.max(1.0);
    let track = slider::track_of(row, ts);
    let f = s.fraction();
    let tx = if rtl {
        track.right() - f * track.w
    } else {
        track.x + f * track.w
    };
    let on_thumb = (p.0 - tx).abs() <= 10.0 * g;
    let on_track = p.1 >= track.y - 10.0 * g;
    if on_thumb || !on_track {
        return false;
    }
    s.set(s.value_at(p.0, track, rtl))
}

fn part_of(item: Item) -> u8 {
    match item {
        Item::Control { part, .. } => part,
        _ => 0,
    }
}

/// The screen-reader id of an item (stable across frames).
pub fn node_id(base: u64, item: Item) -> u64 {
    base + match item {
        Item::Button(b) => 1 + BUTTONS.iter().position(|x| *x == b).unwrap_or(0) as u64,
        Item::Header(c) => 0x1000 * (c as u64 + 1),
        Item::Switch(c) => 0x1000 * (c as u64 + 1) + 1,
        Item::More(c) => 0x1000 * (c as u64 + 1) + 2,
        Item::Control {
            card,
            control,
            part,
        } => 0x1000 * (card as u64 + 1) + 0x10 + 0x10 * control as u64 + u64::from(part),
        Item::History => 0x10_0000,
        Item::HistoryRow(i) => 0x10_0001 + i as u64,
    }
}

fn group_id(base: u64, card: usize) -> u64 {
    base + 0x1000 * (card as u64 + 1) + 0xFFF
}

/// The speed-edit readout over the image `area` (logical px): the control's name, its value
/// and a bar of where it sits in its range.
#[allow(clippy::too_many_arguments)]
pub fn speed_hud(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    area: Rect,
    s: &Slider,
    rtl: bool,
    ts: f32,
) -> Rect {
    let g = ts.max(1.0);
    let style = TextStyle::new(metrics::FONT_BODY * ts).weight(600.0);
    let value = s.value_text();
    let line = t!(
        "develop-speed-hud",
        control = s.spec.label.clone(),
        value = value
    );
    let b = text.layout(&line, style, None, Align::Start);
    let w = (b.width + 32.0 * g).max(180.0 * g);
    let h = b.height + 30.0 * g;
    let r = Rect::new(
        (area.center().0 - w * 0.5).round(),
        (area.y + 24.0 * g).round(),
        w,
        h,
    );
    dl.shadow(r, metrics::RADIUS_MD, pal.shadow, 12.0, 4.0);
    dl.glass(r, metrics::RADIUS_MD, pal.glass, pal.glass_border, 1.0);
    text.draw(dl, &b, r.x + (w - b.width) * 0.5, r.y + 8.0 * g, pal.text);
    let bar = Rect::new(r.x + 16.0 * g, r.bottom() - 12.0 * g, w - 32.0 * g, 3.0);
    dl.rect(bar, 1.5, pal.text_faint.with_alpha(0.45));
    let f = s.fraction();
    let fw = bar.w * f;
    let fill = if rtl {
        Rect::new(bar.right() - fw, bar.y, fw, bar.h)
    } else {
        Rect::new(bar.x, bar.y, fw, bar.h)
    };
    dl.rect(fill, 1.5, pal.accent);
    r
}

#[cfg(test)]
mod tests;

/// A dimmed control's reason, placed so it never covers the control's row or the row
/// above: under the row when it fits in the panel's height, else beside the panel (over
/// the picture), level with the row.
#[allow(clippy::too_many_arguments)]
fn tip_beside_row(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    s: &str,
    row: Rect,
    panel: Rect,
    area: Rect,
    rtl: bool,
    ts: f32,
) -> Rect {
    let style = TextStyle::new((metrics::FONT_SMALL + 0.5) * ts).break_anywhere();
    let max_w = (320.0 * ts.max(1.0)).round().min(panel.w - 12.0).max(80.0);
    let b = text.layout(s, style, Some(max_w), Align::Start);
    let (w, h) = (b.width + 16.0, b.height + 10.0);
    let below = row.bottom() + 4.0;
    let r = if below + h <= panel.bottom() - 4.0 {
        let x = if rtl {
            (row.right() - w).max(panel.x + 6.0)
        } else {
            row.x.min(panel.right() - w - 6.0).max(panel.x + 6.0)
        };
        Rect::new(x, below, w, h)
    } else {
        let x = if panel.x > area.x + 1.0 {
            panel.x - w - 8.0
        } else {
            panel.right() + 8.0
        };
        let x = x.clamp(area.x + 6.0, (area.right() - w - 6.0).max(area.x + 6.0));
        let y = row.y.clamp(area.y, (area.bottom() - h).max(area.y));
        Rect::new(x, y, w, h)
    };
    dl.shadow(r, metrics::RADIUS_SM, pal.shadow, 10.0, 3.0);
    dl.rect(r, metrics::RADIUS_SM, pal.tooltip_bg);
    text.draw(dl, &b, r.x + 8.0, r.y + 5.0, pal.tooltip_text);
    r
}
