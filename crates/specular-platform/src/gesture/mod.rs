//! Touchpad pinch gestures where winit 0.30 has none (DESIGN.md §12.6, "Trackpad pinch").
//!
//! winit reports pinch, rotation and smart magnify on macOS (`WindowEvent::PinchGesture` and
//! friends); Windows precision touchpads send pinch as Ctrl+wheel. On Linux winit has no gesture
//! support, so this module listens on its own:
//!
//! * Wayland: `zwp_pointer_gestures_v1` pinch on a second event queue over winit's
//!   `wl_display` (`wayland.rs`, the same arrangement as the drag-out source).
//! * X11: XInput 2.4 `GesturePinch*` events selected on winit's window over a separate x11rb
//!   connection (`x11.rs`). Needs an X server with XI 2.4 (Xorg 21.1+, libinput driver 1.2+);
//!   XWayland forwards the compositor's gestures the same way.
//!
//! Events arrive on a background thread and are handed to the `sink` the application gives;
//! they carry what winit's `PinchGesture` carries (a per-event relative scale change), so the
//! application handles every platform alike.

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
#[cfg(all(unix, not(target_os = "macos")))]
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

mod swipe;
#[cfg(all(unix, not(target_os = "macos")))]
mod wayland;
#[cfg(all(unix, not(target_os = "macos")))]
mod x11;

pub use swipe::{PageSwipes, SwipeDirection, SwipeSink};

/// Where a gesture is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GesturePhase {
    Begin,
    Update,
    End,
    Cancel,
}

/// One pinch event, in winit's terms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pinch {
    pub phase: GesturePhase,
    /// Relative change of the finger distance since the previous event (`0.01` = 1 % apart
    /// more), like macOS's `magnification` and winit's `PinchGesture::delta`. Zero on `Begin`
    /// and `End`.
    pub delta: f64,
    /// Rotation since the previous event in degrees, counter-clockwise positive (winit's
    /// `RotationGesture` convention).
    pub rotation: f32,
}

/// Callback for gestures; called on a background thread.
pub type Sink = Box<dyn Fn(Pinch) + Send + Sync>;

/// A sink that remembers whether a pinch is running, so a listener that dies mid-gesture can
/// end it (the application would otherwise wait for an `End` that never comes).
struct Tracked {
    sink: Sink,
    active: std::sync::atomic::AtomicBool,
}

impl Tracked {
    fn send(&self, p: Pinch) {
        use std::sync::atomic::Ordering::Relaxed;
        match p.phase {
            GesturePhase::Begin => self.active.store(true, Relaxed),
            GesturePhase::End | GesturePhase::Cancel => self.active.store(false, Relaxed),
            GesturePhase::Update => {}
        }
        (self.sink)(p);
    }

    fn cancel(&self) {
        if self
            .active
            .swap(false, std::sync::atomic::Ordering::Relaxed)
        {
            (self.sink)(Pinch {
                phase: GesturePhase::Cancel,
                delta: 0.0,
                rotation: 0.0,
            });
        }
    }
}

/// Runs a listener's loop (`thread`, until `stop`) behind a boundary. Each run gets a fresh
/// sink; a panic cancels a pinch in flight and the loop restarts after a back-off.
#[cfg_attr(any(not(unix), target_os = "macos"), allow(dead_code))]
fn supervised(
    thread: &str,
    stop: &std::sync::atomic::AtomicBool,
    sink: Sink,
    mut body: impl FnMut(Sink),
) {
    let tracked = std::sync::Arc::new(Tracked {
        sink,
        active: Default::default(),
    });
    specular_log::service::supervise(
        thread,
        specular_log::service::Policy::RESTART,
        || stop.load(std::sync::atomic::Ordering::Acquire),
        || {
            let t = tracked.clone();
            body(Box::new(move |p| t.send(p)));
        },
        |_| tracked.cancel(),
    );
}

/// Turns the cumulative scale Wayland and XI 2.4 report (relative to where the fingers started)
/// into per-event deltas. Garbage (non-finite or non-positive scales) yields 0 and is skipped.
#[derive(Clone, Copy, Debug)]
pub struct PinchScale {
    last: f64,
}

impl Default for PinchScale {
    fn default() -> Self {
        PinchScale { last: 1.0 }
    }
}

impl PinchScale {
    pub fn begin(&mut self) {
        self.last = 1.0;
    }

    /// The relative change from the previous cumulative `scale` to this one.
    pub fn update(&mut self, scale: f64) -> f64 {
        if !scale.is_finite() || scale <= 0.0 {
            return 0.0;
        }
        let d = scale / self.last - 1.0;
        self.last = scale;
        d
    }
}

/// The listeners work from their threads and stop in `Drop`; nothing reads them.
#[allow(dead_code)]
enum Imp {
    #[cfg(all(unix, not(target_os = "macos")))]
    Wayland(wayland::Listener),
    #[cfg(all(unix, not(target_os = "macos")))]
    X11(x11::Listener),
    None,
}

/// Pinch gestures for one window (Linux). Create it once the window exists; drop it before
/// winit's event loop ends (it shares winit's Wayland connection, see `wayland.rs`).
pub struct TouchpadGestures {
    imp: Imp,
}

impl std::fmt::Debug for TouchpadGestures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.imp {
            #[cfg(all(unix, not(target_os = "macos")))]
            Imp::Wayland(_) => "TouchpadGestures(Wayland pointer-gestures)",
            #[cfg(all(unix, not(target_os = "macos")))]
            Imp::X11(_) => "TouchpadGestures(XInput 2.4)",
            Imp::None => "TouchpadGestures(none: winit's own or unsupported)",
        })
    }
}

impl TouchpadGestures {
    /// Starts listening for `window`'s pinch gestures, handing them to `sink`. Never fails:
    /// where winit already reports gestures (macOS), or the system cannot, nothing listens.
    pub fn new<W: HasWindowHandle + HasDisplayHandle>(window: &W, sink: Sink) -> TouchpadGestures {
        let (Ok(wh), Ok(dh)) = (window.window_handle(), window.display_handle()) else {
            return TouchpadGestures { imp: Imp::None };
        };
        let imp = match (wh.as_raw(), dh.as_raw()) {
            #[cfg(all(unix, not(target_os = "macos")))]
            (RawWindowHandle::Wayland(w), RawDisplayHandle::Wayland(d)) => {
                match wayland::Listener::new(d.display.as_ptr(), w.surface.as_ptr(), sink) {
                    Ok(l) => Imp::Wayland(l),
                    Err(e) => {
                        specular_log::info!(target: specular_log::INPUT, "touchpad gestures unavailable: {e}");
                        Imp::None
                    }
                }
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            (RawWindowHandle::Xlib(w), _) => match u32::try_from(w.window) {
                Ok(id) => x11_or_none(id, sink),
                Err(_) => Imp::None,
            },
            #[cfg(all(unix, not(target_os = "macos")))]
            (RawWindowHandle::Xcb(w), _) => x11_or_none(w.window.get(), sink),
            _ => {
                drop(sink);
                Imp::None
            }
        };
        TouchpadGestures { imp }
    }

    /// Whether this listens for anything.
    pub fn active(&self) -> bool {
        !matches!(self.imp, Imp::None)
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn x11_or_none(window: u32, sink: Sink) -> Imp {
    match x11::Listener::new(window, sink) {
        Ok(l) => Imp::X11(l),
        Err(e) => {
            specular_log::info!(target: specular_log::INPUT, "touchpad gestures unavailable: {e}");
            Imp::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A listener that panics mid-pinch: the pinch is cancelled for the application, and the
    /// loop runs again with a working sink.
    #[test]
    #[allow(clippy::disallowed_methods)]
    fn a_listener_panic_cancels_the_pinch_and_restarts() {
        use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let s = seen.clone();
        let sink: Sink = Box::new(move |p: Pinch| s.lock().unwrap().push(p.phase));
        let t = std::thread::Builder::new()
            .name("test-gesture".into())
            .spawn(move || {
                specular_log::service::arm("test-gesture", 1);
                let stop = AtomicBool::new(false);
                let runs = AtomicU32::new(0);
                let pinch = |phase| Pinch {
                    phase,
                    delta: 0.0,
                    rotation: 0.0,
                };
                supervised("test-gesture", &stop, sink, |sink| {
                    runs.fetch_add(1, Ordering::Relaxed);
                    sink(pinch(GesturePhase::Begin));
                    // The first run dies here, mid-gesture.
                    specular_log::service::trip();
                    sink(pinch(GesturePhase::End));
                });
                runs.into_inner()
            })
            .unwrap();
        assert_eq!(t.join().unwrap(), 2);
        use GesturePhase::*;
        assert_eq!(*seen.lock().unwrap(), [Begin, Cancel, Begin, End]);
    }

    #[test]
    fn cumulative_scale_becomes_relative_steps() {
        let mut s = PinchScale::default();
        s.begin();
        let steps: Vec<f64> = [1.1, 1.21, 1.21, 1.1]
            .iter()
            .map(|&x| s.update(x))
            .collect();
        assert!((steps[0] - 0.1).abs() < 1e-12);
        assert!((steps[1] - 0.1).abs() < 1e-12);
        assert_eq!(steps[2], 0.0);
        assert!((steps[3] - (1.1 / 1.21 - 1.0)).abs() < 1e-12);
        // Multiplying the steps gives the cumulative scale back.
        let total: f64 = steps.iter().map(|d| 1.0 + d).product();
        assert!((total - 1.1).abs() < 1e-12);
    }

    #[test]
    fn garbage_scales_are_skipped() {
        let mut s = PinchScale::default();
        assert_eq!(s.update(f64::NAN), 0.0);
        assert_eq!(s.update(0.0), 0.0);
        assert_eq!(s.update(-2.0), 0.0);
        assert!((s.update(2.0) - 1.0).abs() < 1e-12, "last good scale kept");
        s.begin();
        assert!(
            (s.update(0.5) + 0.5).abs() < 1e-12,
            "a new gesture starts from 1"
        );
    }
}
