//! The overlay scrollbar shared by every scrolling area (info panel, cheat sheet, palette
//! list, settings, its sidebar and its dropdowns, the language list, the About box).
//!
//! It stays out of the way: it shows while the area scrolls, while the pointer moves over the
//! area and while the pointer is near the track, and fades out about a second after the last
//! activity. While hovered or dragged it stays visible and its thumb widens. The hit strip is
//! [`HIT`] pixels wide whatever the thumb's width.
//!
//! * Dragging the thumb follows the pointer 1:1, keeping the point where it was grabbed under
//!   the pointer. The drag captures the pointer: moves and the release go to the scrollbar
//!   wherever the pointer is, and the image never pans.
//! * Pressing the track pages towards the pointer; holding repeats until the thumb reaches it.
//! * PgUp, PgDn, Home and End scroll the focused area.
//!
//! The track sits on the end edge of its area: right in left-to-right layouts, left in
//! right-to-left ones. Colours come from the theme palette.
//!
//! There is no AccessKit node for it yet (the chrome has no accessibility tree).

use specular_gfx::Rect;
use specular_input::NamedKey;
use specular_theme::durations;

use crate::Chrome;
use crate::ctx::{Ctx, Target};

/// A scrolling area.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Area {
    Info,
    CheatSheet,
    Palette,
    Settings,
    /// The section list in Settings' sidebar.
    SettingsNav,
    Dropdown,
    Language,
    About,
    Report,
    Insights,
    DicomTags,
    ConfigEditor,
    Studies,
    Work,
    Help,
    Downloads,
    Queue,
}

const AREAS: usize = 17;

impl Area {
    pub(crate) const ALL: [Area; AREAS] = [
        Area::Info,
        Area::CheatSheet,
        Area::Palette,
        Area::Settings,
        Area::SettingsNav,
        Area::Dropdown,
        Area::Language,
        Area::About,
        Area::Report,
        Area::Insights,
        Area::DicomTags,
        Area::ConfigEditor,
        Area::Studies,
        Area::Work,
        Area::Help,
        Area::Downloads,
        Area::Queue,
    ];

    fn index(self) -> usize {
        self as usize
    }
}

/// Width of the hit strip (the thumb is thinner). All sizes are logical pixels, so they
/// scale with the display's DPI.
pub(crate) const HIT: f32 = 14.0;
/// Thumb width at rest and while hovered or dragged.
pub(crate) const THIN: f32 = 7.0;
pub(crate) const WIDE: f32 = 11.0;
/// Shortest thumb, however long the content.
pub(crate) const MIN_THUMB: f32 = 28.0;
/// Seconds of inactivity before the scrollbar fades out.
pub(crate) const IDLE: f64 = 1.0;
/// Horizontal distance from the track within which the pointer keeps the scrollbar shown.
pub(crate) const NEAR: f32 = 24.0;
/// Holding the track: first repeat, then one page per interval.
pub(crate) const REPEAT_DELAY: f64 = 0.35;
pub(crate) const REPEAT_INTERVAL: f64 = 0.06;
/// Inset of the track from the area's end edge and from its top and bottom.
pub(crate) const EDGE: f32 = 2.0;
pub(crate) const ENDS: f32 = 4.0;

/// Where the thumb is for a scroll offset, and back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Geometry {
    /// The hit strip; the thumb moves along its height.
    pub track: Rect,
    /// Height of the visible part of the content.
    pub view: f32,
    /// Largest scroll offset (content height minus `view`).
    pub max: f32,
}

impl Geometry {
    pub fn thumb_len(&self) -> f32 {
        let content = (self.view + self.max).max(1.0);
        (self.track.h * self.view / content)
            .max(MIN_THUMB.min(self.track.h))
            .min(self.track.h)
    }

    /// Room the thumb moves in.
    fn free(&self) -> f32 {
        (self.track.h - self.thumb_len()).max(0.0)
    }

    /// Top of the thumb for `scroll`.
    pub fn thumb_top(&self, scroll: f32) -> f32 {
        let f = if self.max > 0.0 {
            (scroll / self.max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.track.y + self.free() * f
    }

    /// The scroll offset that puts the thumb's top at `top`.
    pub fn scroll_for_top(&self, top: f32) -> f32 {
        let free = self.free();
        if free <= 0.0 {
            return 0.0;
        }
        ((top - self.track.y) / free).clamp(0.0, 1.0) * self.max
    }

    /// One page: the visible height less a little overlap for context.
    pub fn page(&self) -> f32 {
        (self.view - 40.0).max(self.view * 0.5)
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Bar {
    /// Geometry from the last frame that drew the area (`None`: not drawn or nothing to scroll).
    geo: Option<Geometry>,
    /// The scroll range of the area's last layout (`None`: never drawn). Input clamps to it,
    /// so no frame is laid out past either end.
    max: Option<f32>,
    /// The scrolled area, for "pointer over the area".
    area: Rect,
    alpha: f32,
    /// 0 thin … 1 wide.
    widen: f32,
    /// Last scroll or pointer movement over the area.
    active_at: f64,
    last_scroll: f32,
    last_draw: f64,
}

/// A pointer grab on a scrollbar: dragging the thumb or paging by holding the track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Grab {
    /// `offset`: pointer y minus the thumb's top when the drag started.
    Drag { area: Area, offset: f32 },
    /// `dir` +1 pages down, -1 up; `y` is the pointer; `next` the next repeat (seconds).
    Page {
        area: Area,
        dir: f32,
        y: f32,
        next: f64,
    },
}

impl Grab {
    fn area(&self) -> Area {
        match *self {
            Grab::Drag { area, .. } | Grab::Page { area, .. } => area,
        }
    }
}

#[derive(Default)]
pub(crate) struct Scrollbars {
    bars: [Bar; AREAS],
    pub grab: Option<Grab>,
    /// The area PgUp/PgDn/Home/End scroll when no dialog owns the keyboard (the info panel,
    /// after a click or scroll in it).
    pub focus: Option<Area>,
    /// Screenshots: keep every scrollbar shown.
    pub pinned: bool,
    /// Seconds until the scrollbars need another frame (fade-out, paging repeat).
    wake: Option<f64>,
}

impl Scrollbars {
    fn want(&mut self, d: f64) {
        self.wake = Some(self.wake.map_or(d, |w| w.min(d)));
    }

    /// Start of a frame: forget last frame's geometry (areas not drawn have no scrollbar).
    pub(crate) fn begin_frame(&mut self) {
        for b in &mut self.bars {
            b.geo = None;
        }
    }

    /// End of a frame: when the scrollbars next need a frame.
    pub(crate) fn take_wake(&mut self) -> Option<f64> {
        self.wake.take()
    }

    pub(crate) fn geometry(&self, a: Area) -> Option<Geometry> {
        self.bars[a.index()].geo
    }

    /// The pointer moved to `p`: an area under it shows its scrollbar.
    pub(crate) fn pointer_moved(&mut self, p: (f32, f32), now: f64) {
        for b in &mut self.bars {
            if b.geo.is_some() && b.area.contains(p) {
                b.active_at = now;
            }
        }
    }

    /// Whether the scrollbar of `a` is on screen at all (tests, logs).
    /// Tests: the scrolled view of the area's last frame.
    #[cfg(test)]
    pub(crate) fn view(&self, a: Area) -> Rect {
        self.bars[a.index()].area
    }

    /// Forgets the scroll range of `a` until it is drawn again (its content changed, and an
    /// offset set for the new content must not be clamped to the old).
    pub(crate) fn forget_range(&mut self, a: Area) {
        self.bars[a.index()].max = None;
    }

    /// Tests: the geometry a frame would have recorded.
    #[cfg(test)]
    pub(crate) fn set_geometry(&mut self, a: Area, g: Geometry) {
        self.bars[a.index()].geo = Some(g);
        self.bars[a.index()].area =
            Rect::new(g.track.x - 300.0, g.track.y, 300.0 + g.track.w, g.track.h);
    }
}

impl Chrome {
    pub(crate) fn scroll_mut(&mut self, a: Area) -> &mut f32 {
        match a {
            Area::Info => &mut self.info_scroll,
            Area::CheatSheet => &mut self.cheat_scroll,
            Area::Palette => &mut self.palette.scroll,
            Area::Settings => &mut self.settings.scroll,
            Area::SettingsNav => &mut self.settings.nav_scroll,
            Area::Dropdown => &mut self.settings.dropdown_scroll,
            Area::Language => &mut self.language.scroll,
            Area::About => &mut self.about_scroll,
            Area::Report => &mut self.report_scroll,
            Area::Insights => &mut self.insights_scroll,
            Area::DicomTags => &mut self.dicom_tags.scroll,
            Area::ConfigEditor => &mut self.editor.scroll,
            Area::Studies => &mut self.studies.scroll,
            Area::Work => &mut self.work_scroll,
            Area::Help => &mut self.help.scroll,
            Area::Downloads => &mut self.downloads_scroll,
            Area::Queue => self.queue_scroll_mut(),
        }
    }

    fn scroll_of(&mut self, a: Area) -> f32 {
        let s = *self.scroll_mut(a);
        if s.is_nan() {
            self.bars.bars[a.index()].last_scroll
        } else {
            s
        }
    }

    /// Scrolls by `d` pixels (wheel), within the range of the last layout.
    pub(crate) fn scroll_by(&mut self, a: Area, d: f32) {
        let s = self.scroll_of(a);
        *self.scroll_mut(a) = s + d;
        self.clamp_scroll(a);
    }

    /// Clamps an area's offset to the range of its last layout. An offset still waiting to
    /// be placed (NaN: "bring the selection into view") is left alone.
    pub(crate) fn clamp_scroll(&mut self, a: Area) {
        let Some(max) = self.bars.bars[a.index()].max else {
            return;
        };
        let s = self.scroll_mut(a);
        if !s.is_nan() {
            *s = s.clamp(0.0, max);
        }
    }

    /// [`Chrome::clamp_scroll`] for every area (after input, before a frame).
    pub(crate) fn clamp_scrolls(&mut self) {
        for a in Area::ALL {
            self.clamp_scroll(a);
        }
    }

    fn set_scroll(&mut self, a: Area, v: f32, now: f64) {
        let max = self.bars.geometry(a).map_or(f32::INFINITY, |g| g.max);
        *self.scroll_mut(a) = v.clamp(0.0, max.max(0.0));
        self.bars.bars[a.index()].active_at = now;
    }

    /// Primary button pressed on the scrollbar of `a` at height `y`: grab the thumb, or page
    /// towards the pointer and keep paging while the button is held.
    pub(crate) fn bar_press(&mut self, a: Area, y: f32, now: f64) {
        let Some(g) = self.bars.geometry(a) else {
            return;
        };
        self.bars.focus = Some(a);
        let scroll = self.scroll_of(a);
        let top = g.thumb_top(scroll);
        let len = g.thumb_len();
        if y >= top && y <= top + len {
            self.bars.grab = Some(Grab::Drag {
                area: a,
                offset: y - top,
            });
        } else {
            let dir = if y < top { -1.0 } else { 1.0 };
            self.set_scroll(a, scroll + dir * g.page(), now);
            self.bars.grab = Some(Grab::Page {
                area: a,
                dir,
                y,
                next: now + REPEAT_DELAY,
            });
        }
        self.bars.bars[a.index()].active_at = now;
    }

    /// Pointer moved while a scrollbar has the pointer grabbed. Returns false when none has.
    pub(crate) fn bar_drag(&mut self, y: f32, now: f64) -> bool {
        match self.bars.grab {
            Some(Grab::Drag { area, offset }) => {
                if let Some(g) = self.bars.geometry(area) {
                    let s = g.scroll_for_top(y - offset);
                    self.set_scroll(area, s, now);
                }
                true
            }
            Some(Grab::Page {
                area, dir, next, ..
            }) => {
                self.bars.grab = Some(Grab::Page { area, dir, y, next });
                true
            }
            None => false,
        }
    }

    /// Button released: ends the grab. Returns whether there was one.
    pub(crate) fn bar_release(&mut self) -> bool {
        self.bars.grab.take().is_some()
    }

    /// Paging repeat while the track is held: one page per [`REPEAT_INTERVAL`] after
    /// [`REPEAT_DELAY`], until the thumb reaches the pointer.
    pub(crate) fn bar_tick(&mut self, now: f64) {
        let Some(Grab::Page { area, dir, y, next }) = self.bars.grab else {
            return;
        };
        let Some(g) = self.bars.geometry(area) else {
            return;
        };
        if now < next {
            self.bars.want(next - now);
            return;
        }
        let scroll = self.scroll_of(area);
        let top = g.thumb_top(scroll);
        let reached = if dir < 0.0 {
            top <= y
        } else {
            top + g.thumb_len() >= y
        };
        if !reached {
            self.set_scroll(area, scroll + dir * g.page(), now);
        }
        self.bars.grab = Some(Grab::Page {
            area,
            dir,
            y,
            next: now + REPEAT_INTERVAL,
        });
        self.bars.want(REPEAT_INTERVAL);
    }

    /// PgUp, PgDn, Home and End on the focused area. Returns whether the key scrolled.
    pub(crate) fn bar_key(&mut self, a: Area, key: NamedKey, now: f64) -> bool {
        // Nothing to scroll: leave the key to whoever else wants it (PgDn = next image).
        let Some(g) = self.bars.geometry(a) else {
            return false;
        };
        let s = self.scroll_of(a);
        let v = match key {
            NamedKey::PageUp => s - g.page(),
            NamedKey::PageDown => s + g.page(),
            NamedKey::Home => 0.0,
            NamedKey::End => g.max,
            _ => return false,
        };
        self.set_scroll(a, v, now);
        true
    }
}

/// Draws the scrollbar of `a` for content scrolled inside `view` (`max` is the largest
/// scroll offset), on the end edge of `container`, and records its hit strip. Call after the
/// area has clamped its scroll offset.
pub(crate) fn draw(ch: &mut Chrome, cx: &mut Ctx, a: Area, view: Rect, container: Rect, max: f32) {
    let now = cx.now;
    let rm = ch.reduce_motion;
    // The content may have shrunk since the last layout (an area that learns its height
    // while laying out): clamp, and draw again at once at the right offset.
    let max = max.max(0.0);
    let cur = *ch.scroll_mut(a);
    if !cur.is_nan() && cur != cur.clamp(0.0, max) {
        *ch.scroll_mut(a) = cur.clamp(0.0, max);
        ch.bars.want(0.0);
    }
    ch.bars.bars[a.index()].max = Some(max);
    let scroll = ch.scroll_of(a);
    let pointer = ch.pointer;
    let pinned = ch.bars.pinned;
    let grabbed = ch.bars.grab.is_some_and(|g| g.area() == a);
    let bars = &mut ch.bars;
    let b = &mut bars.bars[a.index()];
    let dt = ((now - b.last_draw) as f32).clamp(0.0, 0.1);
    // An area that just appeared (panel opened, dialog shown) flashes its scrollbar once, so
    // it is obvious that there is more to see.
    if now - b.last_draw > 0.5 {
        b.active_at = now;
    }
    b.last_draw = now;
    b.area = view;
    if max <= 0.5 || view.h < MIN_THUMB {
        b.geo = None;
        b.alpha = 0.0;
        return;
    }
    let track = Rect::new(
        container.right() - EDGE - HIT,
        view.y + ENDS,
        HIT,
        view.h - ENDS * 2.0,
    );
    let track = cx.m_in(track, container);
    let g = Geometry {
        track,
        view: view.h,
        max,
    };
    b.geo = Some(g);
    if (scroll - b.last_scroll).abs() > 0.01 {
        b.active_at = now;
    }
    b.last_scroll = scroll;
    let near = pointer.is_some_and(|p| track.inset_xy(-NEAR, 0.0).contains(p));
    let hovered = pointer.is_some_and(|p| track.contains(p));
    let recent = now - b.active_at < IDLE;
    let visible = grabbed || near || recent || pinned;
    let mut animating = false;
    let mut approach = |v: &mut f32, target: f32| {
        if rm {
            *v = target;
            return;
        }
        let k = 1.0 - (-dt / (durations::FADE * 0.45)).exp();
        *v += (target - *v) * k;
        if (*v - target).abs() < 0.01 {
            *v = target;
        } else {
            animating = true;
        }
    };
    approach(&mut b.alpha, if visible { 1.0 } else { 0.0 });
    approach(&mut b.widen, if hovered || grabbed { 1.0 } else { 0.0 });
    let (alpha, widen) = (b.alpha, b.widen);
    let fade_in = b.active_at + IDLE - now;
    if animating {
        bars.want(0.0);
    } else if visible && !grabbed && !near && !pinned {
        bars.want(fade_in.max(0.0) + 0.01);
    }

    cx.hit(track, Target::Scrollbar(a));
    if alpha <= 0.0 {
        return;
    }
    cx.dl.push_opacity(alpha);
    let w = THIN + (WIDE - THIN) * widen;
    // The thumb hugs the end edge of the strip; mirrored with the strip in RTL.
    let end_x = |w: f32| {
        if cx.rtl {
            track.x + EDGE
        } else {
            track.right() - EDGE - w
        }
    };
    if widen > 0.0 {
        let tr = Rect::new(end_x(w), track.y, w, track.h);
        cx.dl
            .rect(tr, w * 0.5, cx.pal.text_faint.with_alpha(0.12 * widen));
    }
    let top = g.thumb_top(scroll);
    let thumb = Rect::new(end_x(w), top, w, g.thumb_len());
    let color = if grabbed {
        cx.pal.text_muted
    } else {
        cx.pal.text_faint.lerp(cx.pal.text_muted, widen)
    };
    cx.dl.rect(thumb, w * 0.5, color);
    cx.dl.pop_opacity();
}
