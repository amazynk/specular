//! Inspecting an image without a mouse, and the value under the cursor for screen readers.
//!
//! **The keyboard crosshair** ([`Action::ToggleCrosshair`]): arrow keys move a crosshair over
//! the image, one image pixel at a time when zoomed in and one screen pixel when zoomed out,
//! ten times as far with Shift. It stands in for the pointer: the pixel inspector, the
//! medical, science and map readouts and the measuring tools follow it, and Enter or Space
//! clicks where it is (placing a measurement point, picking a map feature). Esc puts it
//! away. Moving the mouse moves the crosshair to the pointer.
//!
//! **Reading the value under the cursor** ([`Action::AnnounceCursor`], and Settings ›
//! Accessibility › "Read the value under the cursor"): the readout already on screen, the
//! same text, goes to the chrome's polite status node. The option reads it once the cursor
//! rests ([`CursorAnnouncer`]); the key reads it at once.

use std::time::{Duration, Instant};

use specular_gfx::winit::event::{ElementState, MouseButton};
use specular_gfx::winit::event_loop::ActiveEventLoop;
use specular_gfx::{DrawList, Rect};
use specular_i18n::t;
use specular_input::{Action, Key, KeyCombo, NamedKey};
use specular_settings::Verbosity;
use specular_theme::{Color, Palette};
use specular_widgets::cursor_readout::{CursorAnnouncer, pixel_readout, spoken};

use super::App;

/// Keyboard steps per Shift step.
const BIG_STEP: f32 = 10.0;
/// How far the crosshair's arms reach from its centre (logical px).
const ARM: f32 = 14.0;
/// The gap around the centre, so the pixel itself stays visible (logical px).
const GAP: f32 = 3.0;

#[derive(Default)]
pub(super) struct Cursor {
    /// The keyboard crosshair, in stored-image coordinates, while it is on.
    crosshair: Option<(f32, f32)>,
    announcer: CursorAnnouncer,
}

/// One crosshair step in screen px: an image pixel when zoomed in, a screen pixel when not.
pub(super) fn step_px(scale: f32, shift: bool) -> f32 {
    let one = scale.max(1.0);
    if shift { one * BIG_STEP } else { one }
}

/// The arrow key's direction on screen.
fn arrow(key: &Key) -> Option<(f32, f32)> {
    match key {
        Key::Named(NamedKey::ArrowLeft) => Some((-1.0, 0.0)),
        Key::Named(NamedKey::ArrowRight) => Some((1.0, 0.0)),
        Key::Named(NamedKey::ArrowUp) => Some((0.0, -1.0)),
        Key::Named(NamedKey::ArrowDown) => Some((0.0, 1.0)),
        _ => None,
    }
}

/// `p` kept inside a `w × h` image (stored px).
fn clamp_stored(p: (f32, f32), (w, h): (u32, u32)) -> (f32, f32) {
    let edge = |v: f32, n: u32| v.clamp(0.0, (n.max(1) as f32 - 0.001).max(0.0));
    (edge(p.0, w), edge(p.1, h))
}

/// How far to pan (screen px) so `p` sits `margin` inside `vp`; zero when it already does.
pub(super) fn pan_into_view(p: (f32, f32), vp: Rect, margin: f32) -> (f32, f32) {
    let m = margin.min(vp.w * 0.5).min(vp.h * 0.5);
    let axis = |v: f32, lo: f32, hi: f32| {
        if v < lo + m {
            lo + m - v
        } else if v > hi - m {
            hi - m - v
        } else {
            0.0
        }
    };
    (axis(p.0, vp.x, vp.right()), axis(p.1, vp.y, vp.bottom()))
}

/// Draws the crosshair at `c` (logical px): four arms around a gap, dark under light so it
/// shows on any image.
pub(super) fn draw_crosshair(dl: &mut DrawList, pal: &Palette, c: (f32, f32), clip: Rect) {
    dl.push_clip(clip);
    let arms = [
        ((c.0 - ARM, c.1), (c.0 - GAP, c.1)),
        ((c.0 + GAP, c.1), (c.0 + ARM, c.1)),
        ((c.0, c.1 - ARM), (c.0, c.1 - GAP)),
        ((c.0, c.1 + GAP), (c.0, c.1 + ARM)),
    ];
    let under = Color::rgba(0.0, 0.0, 0.0, 0.75);
    for (a, b) in arms {
        dl.line(a, b, 3.5, under);
    }
    for (a, b) in arms {
        dl.line(a, b, 1.5, pal.focus_ring);
    }
    dl.pop_clip();
}

impl App {
    /// The crosshair and announce actions; true when `a` was one.
    pub(super) fn cursor_action(&mut self, a: Action) -> bool {
        match a {
            Action::ToggleCrosshair => {
                self.crosshair_toggle();
                true
            }
            Action::AnnounceCursor => {
                self.announce_cursor_now();
                true
            }
            _ => false,
        }
    }

    /// Whether the crosshair can be over this view: a 2D image is shown.
    fn crosshair_possible(&self) -> bool {
        self.shown.is_some()
            && !self.gallery_active()
            && !self.mpr_active()
            && !self.mesh_active()
            && !self.font_active()
            && !self.text_active()
            && !self.relight_active()
            && !self.spread_active()
            && self.view.stored_size().0 > 0
    }

    fn crosshair_toggle(&mut self) {
        if self.cursor.crosshair.take().is_some() {
            self.crosshair_toast(t!("crosshair-off"));
            self.redraw = true;
            return;
        }
        if !self.crosshair_possible() {
            return;
        }
        // Where the pointer is over the image, else the middle of the view.
        let vp = self.view.viewport;
        let from = self.view.stored_from_screen();
        let size = self.view.stored_size();
        let start = self
            .pointer
            .filter(|p| super::rect_contains(vp, *p))
            .map(|p| from.apply(p))
            .filter(|&(x, y)| x >= 0.0 && y >= 0.0 && x < size.0 as f32 && y < size.1 as f32)
            .unwrap_or_else(|| from.apply(vp.center()));
        let start = clamp_stored(start, size);
        // Pixel centres, so the readout names the pixel the crosshair is on.
        self.cursor.crosshair = Some((start.0.floor() + 0.5, start.1.floor() + 0.5));
        self.crosshair_toast(t!("crosshair-on"));
        self.crosshair_apply();
    }

    fn crosshair_toast(&mut self, text: String) {
        let now = self.now();
        self.chrome.toast(text, None, now);
    }

    /// The crosshair's position on screen (physical px), while it is on.
    pub(super) fn crosshair_screen(&self) -> Option<(f32, f32)> {
        let (x, y) = self.cursor.crosshair?;
        let m = self.view.screen_from_stored_f64();
        let (x, y) = (f64::from(x), f64::from(y));
        Some((
            (m[0] * x + m[1] * y + m[2]) as f32,
            (m[3] * x + m[4] * y + m[5]) as f32,
        ))
    }

    /// The crosshair stands in for the pointer: everything that follows the pointer runs.
    fn crosshair_apply(&mut self) {
        let Some(p) = self.crosshair_screen() else {
            return;
        };
        self.pointer = Some(p);
        if !(self.anon_mask_motion(p) || self.measure_motion(p)) {
            self.geo_motion(p);
            self.gis_motion(p);
            let _ = self.science_motion(p) || self.oct_motion(p) || self.medical_motion(p);
        }
        self.redraw = true;
    }

    /// A key while the crosshair is on: arrows move it, Enter or Space clicks, Esc puts it
    /// away. True when the key was the crosshair's.
    pub(super) fn crosshair_key(&mut self, combo: &KeyCombo, el: &ActiveEventLoop) -> bool {
        if self.cursor.crosshair.is_none() {
            return false;
        }
        if !self.crosshair_possible() {
            self.cursor.crosshair = None;
            return false;
        }
        let m = combo.mods;
        if m.ctrl || m.alt || m.meta {
            return false;
        }
        if let Some((dx, dy)) = arrow(&combo.key) {
            let step = step_px(self.view.scale(), m.shift);
            self.crosshair_move(dx * step, dy * step);
            return true;
        }
        match combo.key {
            Key::Named(NamedKey::Escape) if !m.shift => {
                self.cursor.crosshair = None;
                self.crosshair_toast(t!("crosshair-off"));
                self.redraw = true;
                true
            }
            Key::Named(NamedKey::Enter | NamedKey::Space) if !m.shift => {
                self.crosshair_click(el);
                true
            }
            _ => false,
        }
    }

    /// Moves the crosshair by a screen-space delta, keeping it on the image and in view.
    fn crosshair_move(&mut self, dx: f32, dy: f32) {
        let Some(p) = self.crosshair_screen() else {
            return;
        };
        let to = self.view.stored_from_screen().apply((p.0 + dx, p.1 + dy));
        let to = clamp_stored(to, self.view.stored_size());
        self.cursor.crosshair = Some(to);
        if let Some(q) = self.crosshair_screen() {
            let margin = 24.0 * self.dl.scale.max(0.1);
            let vp = self.view.viewport;
            let (px, py) = pan_into_view(q, Rect::new(vp.x, vp.y, vp.w, vp.h), margin);
            if px != 0.0 || py != 0.0 {
                self.view.pan_by(px, py, false);
            }
        }
        self.crosshair_apply();
    }

    /// A primary click where the crosshair is.
    fn crosshair_click(&mut self, el: &ActiveEventLoop) {
        let Some(p) = self.crosshair_screen() else {
            return;
        };
        self.pointer = Some(p);
        self.on_mouse_button(ElementState::Pressed, MouseButton::Left, el);
        self.pointer = Some(p);
        self.on_mouse_button(ElementState::Released, MouseButton::Left, el);
        self.redraw = true;
    }

    /// The mouse moved to `p` (physical px): the crosshair follows it onto the image.
    pub(super) fn crosshair_follow_pointer(&mut self, p: (f32, f32)) {
        if self.cursor.crosshair.is_none() || !super::rect_contains(self.view.viewport, p) {
            return;
        }
        let to = self.view.stored_from_screen().apply(p);
        let size = self.view.stored_size();
        if to.0 >= 0.0 && to.1 >= 0.0 && to.0 < size.0 as f32 && to.1 < size.1 as f32 {
            self.cursor.crosshair = Some(to);
            self.redraw = true;
        }
    }

    /// Whether the pixel inspector should follow the pointer without its panel open: the
    /// crosshair or the cursor readout needs its value.
    pub(super) fn cursor_wants_pixel(&self) -> bool {
        self.cursor.crosshair.is_some() || self.announce_cursor_on()
    }

    fn announce_cursor_on(&self) -> bool {
        self.settings.accessibility.announce_cursor
            && self.a11y.as_ref().is_some_and(|a| a.is_active())
    }

    /// The readout at the pointer or crosshair as shown on screen, as one sentence: the
    /// medical, science or map readout when one is on, else the pixel inspector's.
    pub(super) fn cursor_readout(&self) -> Option<String> {
        let verbosity = self.settings.accessibility.screen_reader_verbosity;
        let all = verbosity == Verbosity::Verbose;
        let lines = self
            .medical_cursor_lines(all)
            .or_else(|| self.science_cursor_lines(all))
            .or_else(|| self.geo_cursor_lines());
        if let Some(lines) = lines {
            let lines = if verbosity == Verbosity::Concise {
                &lines[..lines.len().min(1)]
            } else {
                lines
            };
            return spoken(lines);
        }
        self.chrome.inspect.pixel.as_ref().map(pixel_readout)
    }

    /// The announce key: reads the readout now (or says there is nothing to read).
    fn announce_cursor_now(&mut self) {
        let text = self
            .cursor_readout()
            .unwrap_or_else(|| t!("announce-cursor-nothing"));
        self.cursor.announcer.read_now(&text);
        self.chrome.announce_status(text);
        self.redraw = true;
    }

    /// Each pass of the event loop: the crosshair follows the view (zoom, pan, a new
    /// image), and the readout is read out once it rests. Returns when to look again.
    pub(super) fn cursor_tick(&mut self) -> Option<Instant> {
        if self.cursor.crosshair.is_some() {
            if !self.crosshair_possible() {
                self.cursor.crosshair = None;
                self.redraw = true;
            } else if let Some(p) = self.crosshair_screen()
                && self
                    .pointer
                    .is_none_or(|q| (q.0 - p.0).abs() + (q.1 - p.1).abs() > 0.5)
            {
                self.crosshair_apply();
            }
        }
        if !self.announce_cursor_on() {
            self.cursor.announcer.reset();
            return None;
        }
        let now = self.now();
        let text = self.cursor_readout();
        self.cursor.announcer.update(text, now);
        if let Some(text) = self.cursor.announcer.due(now) {
            self.chrome.announce_status(text);
            self.redraw = true;
        }
        let due = self.cursor.announcer.wake()?;
        Some(Instant::now() + Duration::from_secs_f64((due - now).max(0.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_are_a_pixel_of_whichever_is_bigger() {
        assert_eq!(step_px(0.25, false), 1.0);
        assert_eq!(step_px(0.25, true), 10.0);
        assert_eq!(step_px(8.0, false), 8.0);
        assert_eq!(step_px(8.0, true), 80.0);
    }

    #[test]
    fn crosshair_stays_on_the_image() {
        assert_eq!(clamp_stored((-3.0, 5.0), (10, 10)), (0.0, 5.0));
        let (x, y) = clamp_stored((12.0, 99.0), (10, 20));
        assert!(x < 10.0 && x > 9.9 && y < 20.0 && y > 19.9);
    }

    #[test]
    fn panning_brings_the_crosshair_back_into_view() {
        let vp = Rect::new(0.0, 0.0, 800.0, 600.0);
        assert_eq!(pan_into_view((400.0, 300.0), vp, 20.0), (0.0, 0.0));
        assert_eq!(pan_into_view((790.0, 300.0), vp, 20.0), (-10.0, 0.0));
        assert_eq!(pan_into_view((5.0, -40.0), vp, 20.0), (15.0, 60.0));
    }

    #[test]
    fn arrows_point_on_screen() {
        assert_eq!(arrow(&Key::Named(NamedKey::ArrowLeft)), Some((-1.0, 0.0)));
        assert_eq!(arrow(&Key::Named(NamedKey::ArrowDown)), Some((0.0, 1.0)));
        assert_eq!(arrow(&Key::Char('x')), None);
    }
}
