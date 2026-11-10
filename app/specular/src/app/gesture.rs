//! Touchpad gestures (DESIGN.md §12.6): pinch zooms 1:1 with the fingers, a two-finger double
//! tap ("smart magnify") toggles fit and 100 %, rotation is only watched, and on macOS a
//! sideways two-finger swipe over an image that cannot pan sideways turns to the next or
//! previous image (`specular_platform::gesture::PageSwipes`, AppKit's swipe tracking).
//!
//! Sources: winit's `PinchGesture` / `DoubleTapGesture` / `RotationGesture` (macOS), and
//! `specular_platform::gesture` for Wayland and X11, which winit 0.30 does not cover. Windows
//! precision touchpads send pinch as Ctrl+wheel with fractional notches (`App::on_wheel`).
//!
//! macOS interleaves two-finger scroll events with a pinch (the fingers' centre drifts), and
//! those scrolls would pan the image under the fingers; while a pinch runs, and briefly after,
//! precise (pixel) scrolls are dropped.
//!
//! Everything here logs under the `input` target: `SPECULAR_LOG=input:trace` shows each event.

use specular_gfx::winit::event::{MouseScrollDelta, TouchPhase};
use specular_gfx::winit::event_loop::ActiveEventLoop;
use specular_input::Action;
use specular_log::{INPUT, debug, trace};
use specular_platform::gesture::{
    GesturePhase, PageSwipes, Pinch, SwipeDirection, TouchpadGestures,
};
use specular_platform::pen::PenPressure;

use super::App;

/// Pixel scrolls this soon after a pinch ended are its tail, not a pan (seconds).
const SCROLL_QUIET_AFTER: f64 = 0.15;
/// A pinch that sent nothing for this long has lost its end event (seconds).
const STALE: f64 = 0.5;
/// Rotation a gesture must pass before it would turn the image (degrees). Turning on a
/// gesture is not enabled (it is easy to rotate a little while pinching); passing this is
/// logged so the threshold can be tried out.
pub(super) const ROTATE_THRESHOLD: f32 = 45.0;

/// What a pinch event asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Step {
    /// Zoom by this factor at the pointer.
    Zoom(f32),
    /// The pinch ended normally: settle onto fit or 100 % when close.
    Snap,
    Nothing,
}

/// Pinch bookkeeping, separate from the app so it can be tested on recorded sequences.
#[derive(Clone, Debug, Default)]
pub(super) struct PinchFilter {
    active: bool,
    /// Time of the last pinch event (seconds, app clock).
    last: Option<f64>,
    /// Rotation of the running rotation gesture (degrees, counter-clockwise).
    turned: f32,
    turn_reported: bool,
}

impl PinchFilter {
    pub(super) fn pinch(&mut self, phase: TouchPhase, delta: f64, now: f64) -> Step {
        self.last = Some(now);
        match phase {
            TouchPhase::Started | TouchPhase::Moved => {
                self.active = true;
                // macOS's magnification: the new finger distance is (1 + delta) times the
                // old. NaN and absurd jumps are dropped (winit warns of NaN).
                if !delta.is_finite() || delta == 0.0 {
                    return Step::Nothing;
                }
                Step::Zoom((1.0 + delta as f32).clamp(0.5, 2.0))
            }
            TouchPhase::Ended => {
                if std::mem::take(&mut self.active) {
                    Step::Snap
                } else {
                    Step::Nothing
                }
            }
            TouchPhase::Cancelled => {
                self.active = false;
                Step::Nothing
            }
        }
    }

    /// Whether a precise scroll at `now` belongs to a pinch.
    pub(super) fn eats_scroll(&self, now: f64) -> bool {
        let Some(last) = self.last else {
            return false;
        };
        let quiet = now - last;
        if self.active {
            quiet < STALE
        } else {
            quiet < SCROLL_QUIET_AFTER
        }
    }

    /// Adds a rotation step; true once per gesture when it passes [`ROTATE_THRESHOLD`].
    pub(super) fn rotate(&mut self, phase: TouchPhase, degrees: f32) -> bool {
        if phase == TouchPhase::Started {
            self.turned = 0.0;
            self.turn_reported = false;
        }
        if degrees.is_finite() {
            self.turned += degrees;
        }
        let passed = !self.turn_reported && self.turned.abs() >= ROTATE_THRESHOLD;
        self.turn_reported |= passed;
        if matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled) {
            self.turned = 0.0;
            self.turn_reported = false;
        }
        passed
    }
}

/// The app's gesture state.
#[derive(Default)]
pub(super) struct Gestures {
    filter: PinchFilter,
    /// Gallery tile size a pinch is heading for, and the size it last set: `gallery_resize`
    /// ignores changes under half a pixel, so small steps accumulate here.
    pub(super) gallery_size: Option<(f32, f32)>,
    /// Wayland / X11 listener (none where winit reports gestures itself).
    source: Option<TouchpadGestures>,
    rx: Option<crossbeam_channel::Receiver<Pinch>>,
    /// macOS page swipes (none elsewhere) and the swipes they completed.
    swipes: Option<PageSwipes>,
    swipe_rx: Option<crossbeam_channel::Receiver<SwipeDirection>>,
    /// X11 pen pressure (none where winit reports it, and on Wayland), and its last reading.
    pen: Option<PenPressure>,
    pen_reading: std::sync::Arc<std::sync::Mutex<PenReading>>,
    tried: bool,
}

/// A pen pressure reading from the Linux listener, and whether the app has used it yet.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct PenReading {
    latest: Option<(f32, std::time::Instant)>,
    unread: bool,
}

/// A reading this old no longer belongs to a stroke that starts now.
const PEN_FRESH: std::time::Duration = std::time::Duration::from_millis(150);

impl PenReading {
    pub(super) fn set(&mut self, p: f32, at: std::time::Instant) {
        self.latest = Some((p, at));
        self.unread = true;
    }

    /// The reading if it is unread (marking it read).
    fn take(&mut self) -> Option<f32> {
        std::mem::take(&mut self.unread)
            .then_some(self.latest)
            .flatten()
            .map(|(p, _)| p)
    }

    /// The reading if it was taken within [`PEN_FRESH`] of `now`.
    fn fresh(&self, now: std::time::Instant) -> Option<f32> {
        self.latest
            .filter(|(_, at)| now.saturating_duration_since(*at) <= PEN_FRESH)
            .map(|(p, _)| p)
    }
}

impl App {
    /// A pinch event: from winit (macOS) or the Linux listener.
    pub(super) fn on_pinch(&mut self, delta: f64, phase: TouchPhase) {
        let now = self.now();
        let step = self.gestures.filter.pinch(phase, delta, now);
        trace!(
            target: INPUT,
            "pinch {phase:?} delta {delta:+.5} at {:?} -> {step:?} (scale {:.4})",
            self.pointer,
            self.view.scale()
        );
        if self.chrome.modal().is_some() {
            return;
        }
        if self.gallery_active() {
            if let Step::Zoom(f) = step {
                self.gallery_scale_by(f);
            }
            return;
        }
        if self.shown.is_none() {
            return;
        }
        match step {
            Step::Zoom(f) => {
                self.view.zoom_direct_by(f, self.pointer);
                self.after_view_change();
            }
            Step::Snap => {
                if self.view.soft_snap(self.pointer) {
                    debug!(target: INPUT, "pinch ended near fit or 100 %: snapped");
                    self.after_view_change();
                }
            }
            Step::Nothing => {}
        }
    }

    /// Two-finger double tap (macOS smart magnify): fit ↔ 100 % at the pointer.
    pub(super) fn on_smart_magnify(&mut self) {
        trace!(target: INPUT, "smart magnify at {:?}", self.pointer);
        if self.chrome.modal().is_some() || self.gallery_active() || self.shown.is_none() {
            return;
        }
        let p = self.pointer.unwrap_or_else(|| self.view.viewport.center());
        self.view.toggle_fit_actual(p);
        self.after_view_change();
    }

    /// Rotation gesture: logged only (see [`ROTATE_THRESHOLD`]).
    pub(super) fn on_rotation(&mut self, degrees: f32, phase: TouchPhase) {
        trace!(target: INPUT, "rotation {phase:?} {degrees:+.2}°");
        if self.gestures.filter.rotate(phase, degrees) {
            debug!(
                target: INPUT,
                "rotation gesture passed {ROTATE_THRESHOLD}° (turning on gestures is not enabled)"
            );
        }
    }

    /// Logs a wheel event; true when it is a pinch's stray scroll and must be dropped.
    pub(super) fn pinch_eats_wheel(&mut self, delta: MouseScrollDelta, phase: TouchPhase) -> bool {
        let eat = matches!(delta, MouseScrollDelta::PixelDelta(_))
            && self.gestures.filter.eats_scroll(self.now());
        trace!(
            target: INPUT,
            "wheel {delta:?} {phase:?} ctrl {}{}",
            self.mods.control_key(),
            if eat { " (dropped: pinch running)" } else { "" }
        );
        eat
    }

    /// Starts the Linux listener and the macOS page swipes once the window exists, and handles
    /// what they sent. Called from `about_to_wait`; the listeners wake the loop.
    pub(super) fn drain_gestures(&mut self, el: &ActiveEventLoop) {
        if !self.gestures.tried
            && let Some(g) = &self.gfx
        {
            self.gestures.tried = true;
            let (tx, rx) = crossbeam_channel::unbounded();
            let wake = self.loader.waker();
            let src = TouchpadGestures::new(
                &*g.window,
                Box::new(move |p| {
                    if tx.send(p).is_ok() {
                        wake();
                    }
                }),
            );
            debug!(target: INPUT, "{src:?}");
            if src.active() {
                self.gestures.source = Some(src);
                self.gestures.rx = Some(rx);
            }
            let (tx, rx) = crossbeam_channel::unbounded();
            let wake = self.loader.waker();
            let swipes = PageSwipes::new(Box::new(move |d| {
                if tx.send(d).is_ok() {
                    wake();
                }
            }));
            debug!(target: INPUT, "{swipes:?}");
            if swipes.active() {
                self.gestures.swipes = Some(swipes);
                self.gestures.swipe_rx = Some(rx);
            }
            let reading = self.gestures.pen_reading.clone();
            let wake = self.loader.waker();
            let pen = PenPressure::new(
                &*g.window,
                Box::new(move |p| {
                    if let Ok(mut r) = reading.lock() {
                        r.set(p, std::time::Instant::now());
                    }
                    wake();
                }),
            );
            debug!(target: INPUT, "{pen:?}");
            if pen.active() {
                self.gestures.pen = Some(pen);
            }
        }
        self.drain_swipes(el);
        let pen = self
            .gestures
            .pen_reading
            .lock()
            .ok()
            .and_then(|mut r| r.take());
        if let Some(p) = pen {
            self.develop_pressure(p);
        }
        let Some(rx) = &self.gestures.rx else {
            return;
        };
        let events: Vec<Pinch> = rx.try_iter().collect();
        for p in events {
            let phase = match p.phase {
                GesturePhase::Begin => TouchPhase::Started,
                GesturePhase::Update => TouchPhase::Moved,
                GesturePhase::End => TouchPhase::Ended,
                GesturePhase::Cancel => TouchPhase::Cancelled,
            };
            self.on_rotation(p.rotation, phase);
            self.on_pinch(p.delta, phase);
        }
    }

    /// The Linux pen listener's pressure, if it is recent enough to belong to a stroke that
    /// starts now (Develop's brush).
    pub(super) fn pen_pressure_now(&self) -> Option<f32> {
        let r = self.gestures.pen_reading.lock().ok()?;
        r.fresh(std::time::Instant::now())
    }

    /// Turns the page for each completed swipe, then tells the swipe monitor which swipes a
    /// gesture starting now may become.
    fn drain_swipes(&mut self, el: &ActiveEventLoop) {
        let Some(rx) = &self.gestures.swipe_rx else {
            return;
        };
        let done: Vec<SwipeDirection> = rx.try_iter().collect();
        for d in done {
            debug!(target: INPUT, "page swipe {d:?}");
            // The fingers drag the page: moving left brings in the next one.
            let next = (d == SwipeDirection::Left) != self.arrows_reversed();
            self.action(
                if next {
                    Action::NextImage
                } else {
                    Action::PrevImage
                },
                el,
            );
        }
        let (left, right) = if self.swipe_allowed() {
            let (next, prev) = (self.can_navigate(1), self.can_navigate(-1));
            if self.arrows_reversed() {
                (prev, next)
            } else {
                (next, prev)
            }
        } else {
            (false, false)
        };
        if let Some(s) = &self.gestures.swipes {
            s.offer(left, right);
        }
    }

    /// Whether a sideways two-finger scroll would have nothing else to do: the viewer shows an
    /// image no wider than the window, and no dialog or chrome under the pointer takes the
    /// wheel (`App::on_wheel`).
    fn swipe_allowed(&self) -> bool {
        let dialog = self.password.is_open()
            || self.raw_volume.is_open()
            || self.dicomweb.dialog.is_some()
            || self.omero.dialog.is_some()
            || self.print.dialog.is_some()
            || self.convert.dialog.is_some()
            || self.video_export.dialog.is_some()
            || self.anon.visible()
            || self.figure.visible();
        let other_view =
            self.gallery_active() || self.mpr_active() || self.mesh_active() || self.cad_active();
        let p = self.pointer.unwrap_or_else(|| self.view.viewport.center());
        let fits = self.view.display_size().0 * self.view.scale() <= self.view.viewport.w + 0.5;
        self.current.is_some()
            && !dialog
            && !other_view
            && fits
            && !self.mods.control_key()
            && !self.chrome.wheel_over_chrome(self.logical(p))
    }

    /// Stops the Linux listener and the page swipes; must run before winit closes its display
    /// (`exiting`).
    pub(super) fn close_gestures(&mut self) {
        self.gestures.source = None;
        self.gestures.rx = None;
        self.gestures.swipes = None;
        self.gestures.swipe_rx = None;
    }
}

#[cfg(test)]
mod tests {
    use specular_render::{PxRect, View};

    use super::*;

    /// The X11 pen listener's readings: each is used once while painting, and a stroke that
    /// starts takes only a recent one (an old reading is from another stroke or the mouse).
    #[test]
    fn pen_readings_are_used_once_and_only_while_fresh() {
        let t0 = std::time::Instant::now();
        let mut r = PenReading::default();
        assert_eq!(r.take(), None);
        assert_eq!(r.fresh(t0), None);
        r.set(0.4, t0);
        assert_eq!(r.take(), Some(0.4));
        assert_eq!(r.take(), None, "read once");
        assert_eq!(
            r.fresh(t0 + PEN_FRESH),
            Some(0.4),
            "still fresh for a press"
        );
        assert_eq!(
            r.fresh(t0 + PEN_FRESH + std::time::Duration::from_millis(1)),
            None
        );
        r.set(0.7, t0 + PEN_FRESH);
        assert_eq!(r.take(), Some(0.7));
    }

    /// What a Mac sends for a slow pinch-out, as winit 0.30 delivers it: each magnify comes
    /// with a cursor move, and precise scroll events (some zero) interleave.
    enum Rec {
        Pinch(TouchPhase, f64),
        Scroll(f32),
    }

    fn mac_pinch_out() -> Vec<(f64, Rec)> {
        let mut v = vec![
            (0.0, Rec::Scroll(0.0)),
            (0.004, Rec::Pinch(TouchPhase::Started, 0.0)),
        ];
        let mut t = 0.004;
        for i in 0..40 {
            t += 0.008;
            v.push((
                t,
                Rec::Pinch(TouchPhase::Moved, 0.004 + 0.0005 * (i % 3) as f64),
            ));
            if i % 7 == 3 {
                v.push((t + 0.001, Rec::Scroll(if i % 2 == 0 { 0.0 } else { 3.0 })));
            }
        }
        v.push((t + 0.01, Rec::Pinch(TouchPhase::Ended, 0.0)));
        v.push((t + 0.05, Rec::Scroll(-2.0)));
        v
    }

    fn view() -> View {
        let mut v = View::new();
        v.set_viewport(PxRect {
            x: 0.0,
            y: 0.0,
            w: 1000.0,
            h: 500.0,
        });
        v.set_image(4000, 2000, specular_core::Orientation::NORMAL);
        v.zoom_fit();
        v.settle();
        v
    }

    /// Replays a recording the way `App::on_pinch` / `on_wheel` handle it.
    fn replay(rec: &[(f64, Rec)], v: &mut View, at: (f32, f32)) -> (u32, u32) {
        let mut f = PinchFilter::default();
        let (mut pans, mut eaten) = (0, 0);
        for (t, e) in rec {
            match *e {
                Rec::Pinch(phase, d) => match f.pinch(phase, d, *t) {
                    Step::Zoom(k) => v.zoom_direct_by(k, Some(at)),
                    Step::Snap => {
                        v.soft_snap(Some(at));
                    }
                    Step::Nothing => {}
                },
                Rec::Scroll(dy) => {
                    if f.eats_scroll(*t) {
                        eaten += 1;
                    } else {
                        v.pan_by(0.0, -dy, false);
                        pans += 1;
                    }
                }
            }
        }
        (pans, eaten)
    }

    #[test]
    fn mac_pinch_zooms_continuously_about_the_fingers() {
        let mut v = view();
        let at = (700.0, 200.0);
        let before = v.screen_to_display(at);
        let fit = v.scale();
        let rec = mac_pinch_out();
        let expect: f64 = rec
            .iter()
            .map(|(_, e)| match e {
                Rec::Pinch(TouchPhase::Moved, d) => 1.0 + d,
                _ => 1.0,
            })
            .product();
        let (pans, eaten) = replay(&rec, &mut v, at);
        // Every step lands at once, none lost to dead zones or animation.
        assert!(
            ((v.scale() / fit) as f64 / expect - 1.0).abs() < 1e-3,
            "{} vs {expect}",
            v.scale() / fit
        );
        assert!(v.scale() > fit * 1.15);
        // The point under the fingers stayed there; the interleaved scrolls did not pan.
        let after = v.screen_to_display(at);
        assert!(
            (before.0 - after.0).abs() < 0.5 && (before.1 - after.1).abs() < 0.5,
            "{before:?} {after:?}"
        );
        assert_eq!(eaten, 7, "scrolls during the pinch and just after it");
        assert_eq!(pans, 1, "the scroll before the pinch");
    }

    #[test]
    fn pinch_ending_near_fit_snaps_onto_it() {
        let mut v = view();
        let fit = v.scale();
        let rec = [
            (0.0, Rec::Pinch(TouchPhase::Started, 0.0)),
            (0.01, Rec::Pinch(TouchPhase::Moved, 0.02)),
            (0.02, Rec::Pinch(TouchPhase::Ended, 0.0)),
        ];
        replay(&rec, &mut v, (500.0, 250.0));
        v.settle();
        assert!((v.scale() / fit - 1.0).abs() < 1e-4);
    }

    #[test]
    fn nan_cancel_and_stray_phases_do_nothing() {
        let mut f = PinchFilter::default();
        assert_eq!(f.pinch(TouchPhase::Started, f64::NAN, 0.0), Step::Nothing);
        assert_eq!(
            f.pinch(TouchPhase::Moved, f64::INFINITY, 0.01),
            Step::Nothing
        );
        assert_eq!(
            f.pinch(TouchPhase::Moved, 9.0, 0.02),
            Step::Zoom(2.0),
            "clamped"
        );
        assert_eq!(f.pinch(TouchPhase::Cancelled, 0.0, 0.03), Step::Nothing);
        assert_eq!(
            f.pinch(TouchPhase::Ended, 0.0, 0.04),
            Step::Nothing,
            "no snap after cancel"
        );
        // A lost end: scrolls come back after the stale time.
        f.pinch(TouchPhase::Moved, 0.01, 1.0);
        assert!(f.eats_scroll(1.2));
        assert!(!f.eats_scroll(1.0 + STALE + 0.01));
    }

    #[test]
    fn rotation_reports_once_past_the_threshold() {
        let mut f = PinchFilter::default();
        assert!(!f.rotate(TouchPhase::Started, 0.0));
        let mut hits = 0;
        for _ in 0..30 {
            hits += u32::from(f.rotate(TouchPhase::Moved, 2.0));
        }
        assert_eq!(hits, 1);
        f.rotate(TouchPhase::Ended, 0.0);
        assert!(!f.rotate(TouchPhase::Started, 10.0));
        assert!(
            !f.rotate(TouchPhase::Moved, -20.0),
            "a small wobble does nothing"
        );
    }

    /// The handlers as committed in d657740 (pinch: `zoom_to(target × (1 + delta))`; precise
    /// scrolls zoom by `1.0035^px` under the default `wheel_behaviour = Zoom`), driven with
    /// 60 Hz frames. They do zoom on every frame, so a pinch that does nothing at all on a Mac
    /// cannot come from this logic; it means the events never reach `window_event`.
    #[test]
    fn committed_handlers_zoom_every_frame() {
        let mut v = view();
        let at = (700.0, 200.0);
        let fit = v.scale();
        let rec = mac_pinch_out();
        let mut shown = vec![v.scale()];
        let mut next_frame = 1.0 / 60.0;
        for (t, e) in &rec {
            while *t >= next_frame {
                v.step(1.0 / 60.0);
                shown.push(v.scale());
                next_frame += 1.0 / 60.0;
            }
            match *e {
                Rec::Pinch(_, d) => {
                    let f = (1.0 + d as f32).clamp(0.5, 2.0);
                    v.zoom_to(v.target_scale() * f, Some(at));
                }
                Rec::Scroll(dy) => {
                    let lines = dy / 40.0;
                    v.zoom_to(v.target_scale() * 1.0035f32.powf(lines * 40.0), Some(at));
                }
            }
        }
        v.settle();
        // Each frame from the first pinch step on shows a larger image.
        let grew = shown.windows(2).skip(1).filter(|w| w[1] > w[0]).count();
        assert!(grew + 2 >= shown.len() - 1, "{shown:?}");
        assert!(v.scale() > fit * 1.15, "{} vs fit {fit}", v.scale());
    }
}
