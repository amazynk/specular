//! Rearranging the toolbar by dragging its items, any time no dialog is open, and holding
//! its Previous and Next buttons to flip through the folder.
//!
//! A press on an item of the bar picks it up once the pointer has moved a few pixels, or
//! after it has been held still for a moment. Previous and Next are only picked up by
//! moving: held still, they repeat instead (like the arrow keys). While an item is dragged
//! the bar reflows around the place it would land; releasing it there moves it (one undo
//! step of Settings), releasing it off the bar or pressing Esc puts it back. Nothing is
//! hidden this way, and nothing moves in or out of the More menu.

use specular_gfx::{Rect, draw_icon};
use specular_settings::{ToolbarItem, Value};
use specular_theme::metrics;

use crate::ctx::{Ctx, HAlign, Target};
use crate::toolbar_edit::{Change, Src, bar_drop, current, drop_on};
use crate::{Chrome, Command, Response, ViewerState};

/// The setting that turns dragging on the bar on.
pub(crate) const SETTING: &str = "viewing.toolbar_drag";
/// Pointer travel (logical px) before a press on the bar picks its item up. A little more
/// than a click wobbles.
pub(crate) const PICKUP_DISTANCE: f32 = 6.0;
/// How long a still press on the bar takes to pick its item up (not Previous or Next).
pub(crate) const PICKUP_HOLD: f64 = 0.4;
/// How long Previous or Next is held before it repeats, and how often it then does (decided:
/// 0.4 s, then every 0.08 s; a test pins them).
pub(crate) const REPEAT_DELAY: f64 = 0.4;
pub(crate) const REPEAT_EVERY: f64 = 0.08;

/// The toolbar as last drawn.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct BarGeom {
    pub bar: Rect,
    /// The slots of the items on the bar (those before the More button), in list order.
    pub slots: Vec<Rect>,
    /// The list the bar was drawn from (with the items in the More menu).
    pub items: Vec<ToolbarItem>,
    pub rtl: bool,
    /// The size of the window it was laid out in (a press ends when that changes).
    pub window: (f32, f32),
}

/// A press on an item of the bar, and the drag it may become.
#[derive(Clone, Debug)]
pub(crate) struct BarDrag {
    /// Its position in the list, and what it is.
    pub src: usize,
    pub item: ToolbarItem,
    pub start: (f32, f32),
    pub at: (f32, f32),
    /// Where in its slot it was grabbed, and the slot's size.
    grab: (f32, f32),
    size: (f32, f32),
    since: f64,
    /// A still press picks it up (not Previous or Next, which repeat instead).
    hold_ok: bool,
    pub active: bool,
    /// The bar as it was when the press began: drops are worked out on it, so the bar
    /// reflowing under the pointer never makes the landing place jump back and forth.
    pub geom: BarGeom,
}

impl BarDrag {
    /// Where it lands: before this position of the list, or `None` off the bar.
    pub(crate) fn drop_at(&self) -> Option<usize> {
        let g = &self.geom;
        bar_drop(&g.slots, g.bar, self.at, g.rtl)
    }

    /// The list a release now would make, and the change; `None` when nothing would change.
    pub(crate) fn prospective(&self) -> Option<(Vec<ToolbarItem>, Change)> {
        drop_on(&self.geom.items, Src::Bar(self.src), Some(self.drop_at()?))
    }

    /// The list to draw while dragging, and the position of the dragged item in it.
    pub(crate) fn shown(&self) -> (Vec<ToolbarItem>, usize) {
        match self.prospective() {
            Some((l, Change::Moved(_, i))) => (l, i),
            _ => (self.geom.items.clone(), self.src),
        }
    }
}

/// `shown` (the bar's items after a drag) with the stored items the bar left out for what
/// is on screen (MPR without a volume) put back after the item they followed.
pub(crate) fn with_hidden(stored: &[ToolbarItem], shown: &[ToolbarItem]) -> Vec<ToolbarItem> {
    let mut out = shown.to_vec();
    for (i, it) in stored.iter().enumerate() {
        if it.is_layout() || shown.contains(it) {
            continue;
        }
        let at = match i.checked_sub(1).map(|p| stored[p]) {
            None => 0,
            Some(prev) => out
                .iter()
                .position(|x| *x == prev)
                .map_or(out.len().min(i), |p| p + 1),
        };
        out.insert(at, *it);
    }
    out
}

#[cfg(test)]
#[test]
fn hidden_items_keep_their_place_after_a_drag() {
    use ToolbarItem as T;
    let stored = [
        T::Prev,
        T::Next,
        T::Info,
        T::Mpr,
        T::Fullscreen,
        T::Settings,
    ];
    // Settings dragged to the front while MPR was hidden.
    let shown = [T::Settings, T::Prev, T::Next, T::Info, T::Fullscreen];
    assert_eq!(
        with_hidden(&stored, &shown),
        [
            T::Settings,
            T::Prev,
            T::Next,
            T::Info,
            T::Mpr,
            T::Fullscreen
        ]
    );
    // Info, which MPR followed, moved: MPR goes with it.
    let shown = [T::Info, T::Prev, T::Next, T::Fullscreen, T::Settings];
    assert_eq!(
        with_hidden(&stored, &shown),
        [
            T::Info,
            T::Mpr,
            T::Prev,
            T::Next,
            T::Fullscreen,
            T::Settings
        ]
    );
    assert_eq!(with_hidden(&stored, &stored), stored);
}

/// Previous or Next held down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NavHold {
    next: bool,
    since: f64,
    /// When it next repeats.
    due: f64,
    /// It has repeated: its release is not a click.
    fired: bool,
}

/// Whether dragging on the bar is on (`viewing.toolbar_drag`, on unless turned off).
fn enabled(st: &ViewerState) -> bool {
    !matches!(st.store.get(SETTING), Ok(Value::Bool(false)))
}

impl Chrome {
    /// A primary press: on an item of the bar (with no dialog open, not in the gallery) it
    /// may become a drag; on Previous or Next it may repeat.
    pub(crate) fn bar_item_press(
        &mut self,
        h: Option<&Target>,
        p: (f32, f32),
        now: f64,
        st: &ViewerState,
    ) {
        self.bar_press = None;
        self.nav_hold = None;
        if self.modal.is_some() || self.gallery {
            return;
        }
        let Some(g) = &self.bar_geom else {
            return;
        };
        let shown = g.slots.len();
        let src = match h {
            // A row of the More menu is not on the bar.
            Some(Target::Toolbar(it)) if !self.toolbar_overflow.contains(it) => g.items
                [..shown.min(g.items.len())]
                .iter()
                .position(|x| x == it),
            Some(Target::BarSlot(i)) => Some(*i),
            _ => None,
        };
        let Some(src) = src.filter(|i| *i < shown && *i < g.items.len()) else {
            return;
        };
        let item = g.items[src];
        let flips = matches!(item, ToolbarItem::Prev | ToolbarItem::Next);
        if flips && Target::Toolbar(item).disabled(st).is_none() {
            self.nav_hold = Some(NavHold {
                next: item == ToolbarItem::Next,
                since: now,
                due: now + REPEAT_DELAY,
                fired: false,
            });
        }
        // The bar must show what is stored (it does, but a change may be on its way).
        if !enabled(st) || crate::toolbar::for_content(current(st.store), st) != g.items {
            return;
        }
        let r = g.slots[src];
        self.bar_press = Some(BarDrag {
            src,
            item,
            start: p,
            at: p,
            grab: (p.0 - r.x, p.1 - r.y),
            size: (r.w, r.h),
            since: now,
            hold_ok: !flips,
            active: false,
            geom: g.clone(),
        });
    }

    /// Picks up the pressed item: the press is no longer a click.
    fn pick_up(&mut self) {
        if let Some(d) = &mut self.bar_press {
            d.active = true;
        }
        self.nav_hold = None;
        self.pressed = None;
        self.hover = None;
        self.overflow_menu = None;
        self.zoom_menu = false;
        self.toolbar_menu = None;
    }

    /// Follows the pointer; true while an item is dragged (the pointer is captured).
    pub(crate) fn bar_item_move(&mut self, p: (f32, f32)) -> bool {
        let Some(d) = &mut self.bar_press else {
            return false;
        };
        d.at = p;
        let (dx, dy) = (p.0 - d.start.0, p.1 - d.start.1);
        if !d.active && dx * dx + dy * dy > PICKUP_DISTANCE * PICKUP_DISTANCE {
            self.pick_up();
        }
        self.bar_press.as_ref().is_some_and(|d| d.active)
    }

    /// Each frame: picks up an item held still long enough, and repeats a held Previous or
    /// Next (queued for [`Chrome::take_commands`]). Returns how long until it needs to look
    /// again.
    pub(crate) fn bar_hold_tick(&mut self, now: f64) -> Option<f64> {
        let mut wake: Option<f64> = None;
        if let Some(d) = &self.bar_press
            && !d.active
            && d.hold_ok
        {
            let left = PICKUP_HOLD - (now - d.since);
            if left <= 0.0 {
                self.pick_up();
            } else {
                wake = Some(left);
            }
        }
        if let Some(h) = &mut self.nav_hold {
            if now >= h.due {
                self.pending.push(Command::NavHold {
                    next: h.next,
                    held: now - h.since,
                });
                h.fired = true;
                h.due = (h.due + REPEAT_EVERY).max(now);
                // A repeating button is not picked up, and its release is not a click.
                self.bar_press = None;
                self.pressed = None;
            }
            let left = (h.due - now).max(0.0);
            wake = Some(wake.map_or(left, |w| w.min(left)));
        }
        wake
    }

    /// A release: drops the dragged item, or ends a repeating Previous or Next. `None` when
    /// neither (the release is then a click).
    pub(crate) fn bar_item_release(&mut self, st: &ViewerState) -> Option<Response> {
        if self.nav_hold.take().is_some_and(|h| h.fired) {
            self.bar_press = None;
            return Some(Response::cmd(Command::NavHoldEnd));
        }
        self.show_drag = None;
        let d = self.bar_press.take()?;
        if !d.active {
            return None;
        }
        let stored = current(st.store);
        if crate::toolbar::for_content(stored.clone(), st) == d.geom.items
            && let Some((l, c)) = d.prospective()
        {
            let l = with_hidden(&stored, &l);
            let c = match c {
                Change::Moved(it, _) => {
                    Change::Moved(it, l.iter().position(|x| *x == it).unwrap_or(0))
                }
                c => c,
            };
            self.apply_toolbar(st, &l, c);
        }
        Some(Response::used())
    }

    /// Esc: puts a dragged item back. False when none was being dragged.
    pub(crate) fn bar_item_cancel(&mut self) -> bool {
        self.show_drag = None;
        if self.bar_press.as_ref().is_some_and(|d| d.active) {
            self.bar_press = None;
            return true;
        }
        false
    }

    /// Whether an item of the bar is being dragged, or Previous or Next is held: the bar
    /// stays shown.
    pub(crate) fn bar_busy(&self) -> bool {
        self.bar_press.is_some() || self.nav_hold.is_some()
    }

    /// Shows item `from` of the bar picked up and held over item `to`, a little above the bar
    /// (screenshots), once the bar has been drawn.
    #[doc(hidden)]
    pub fn show_bar_drag(&mut self, from: usize, to: usize) {
        self.show_drag = Some((from, to));
    }

    /// Picks up the item asked for by [`Chrome::show_bar_drag`] (again after the window
    /// changed size); false while the bar has not been drawn.
    pub(crate) fn apply_show_drag(&mut self) -> bool {
        let Some((from, to)) = self.show_drag.filter(|_| self.bar_press.is_none()) else {
            return false;
        };
        let Some(g) = self.bar_geom.clone() else {
            return false;
        };
        let (Some(a), Some(b)) = (g.slots.get(from).copied(), g.slots.get(to).copied()) else {
            return false;
        };
        let start = a.center();
        let at = (b.center().0 + b.w * 0.3, b.center().1 - 6.0);
        self.bar_press = Some(BarDrag {
            src: from,
            item: g.items[from],
            start,
            at,
            grab: (start.0 - a.x, start.1 - a.y),
            size: (a.w, a.h),
            since: self.last_now,
            hold_ok: false,
            active: true,
            geom: g,
        });
        true
    }

    /// What the chrome asked for since the last call, outside input events (a held
    /// Previous or Next repeating).
    pub fn take_commands(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.pending)
    }
}

/// The empty place a dragged item would land in, slot `r`.
pub(crate) fn socket(cx: &mut Ctx, r: Rect) {
    let radius = metrics::RADIUS_SM + 2.0;
    cx.dl.rect_border(
        r.inset(2.0),
        radius,
        cx.pal.accent_soft,
        cx.pal.accent.with_alpha(0.45),
        1.0,
    );
}

/// The dragged item under the pointer, lifted off the bar: on it, with an accent edge; off it
/// (a release puts it back), fainter.
pub(crate) fn ghost(
    ch: &Chrome,
    cx: &mut Ctx,
    st: &ViewerState,
    zoom: Option<&specular_gfx::TextBlock>,
) {
    let Some(d) = ch.bar_press.as_ref().filter(|d| d.active) else {
        return;
    };
    let on = d.drop_at().is_some();
    let g = Rect::new(
        (d.at.0 - d.grab.0).round(),
        (d.at.1 - d.grab.1).round(),
        d.size.0,
        d.size.1,
    );
    let radius = metrics::RADIUS_SM + 4.0;
    if !on {
        cx.dl.push_opacity(0.6);
    }
    cx.dl.shadow(g, radius, cx.pal.shadow, 14.0, 5.0);
    cx.dl
        .glass(g, radius, cx.pal.glass, cx.pal.glass_border, 1.0);
    let edge = if on { cx.pal.accent } else { cx.pal.text_faint };
    cx.dl
        .rect_border(g, radius, specular_theme::Color::TRANSPARENT, edge, 1.5);
    match d.item {
        ToolbarItem::ZoomLevel => {
            if let Some(b) = zoom {
                cx.block_in(b, g, HAlign::Center, cx.pal.text);
            }
        }
        ToolbarItem::Spacer | ToolbarItem::Separator => {
            let s = 16.0f32.min(g.w - 4.0).min(g.h - 4.0);
            crate::toolbar_edit::glyph(cx, d.item, g.centered(s, s), cx.pal.text_muted);
        }
        it => {
            if let Some((icon, flip, _)) = crate::toolbar::item_icon(it, st, ch.info_open, cx.rtl) {
                let ic = g.centered(metrics::ICON, metrics::ICON);
                draw_icon(cx.dl, icon, ic, cx.pal.text, metrics::ICON_STROKE, flip);
            }
        }
    }
    if !on {
        cx.dl.pop_opacity();
    }
}
