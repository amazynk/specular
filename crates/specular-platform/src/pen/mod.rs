//! Pen pressure where winit 0.30 has none (Develop's brush, DESIGN.md §12.6).
//!
//! winit gives pressure on Windows (pens arrive as touches with a force) and macOS
//! (`TouchpadPressure`). On Linux it gives none: its X11 backend fills `Touch::force` with
//! `None` and does not read tablet valuators, and its Wayland backend does not bind
//! `zwp_tablet_manager_v2` at all. So:
//!
//! * X11: XInput 2 raw motion and raw button events, selected on the root window over our own
//!   x11rb connection (`x11.rs`). Raw events go to every client that asks, so winit's own
//!   pointer events are untouched: the tablet still moves the pointer, and this only adds the
//!   pressure valuator ("Abs Pressure") of the pen device. XWayland passes it on too.
//! * Wayland: nothing. Binding `zwp_tablet_seat_v2` makes the compositor stop emulating the
//!   pointer for that client's surfaces (wlroots, Mutter and KWin alike), so the pen would no
//!   longer press buttons, drag sliders or move the picture unless every control learned tablet
//!   events. That waits for tablet support in winit; until then a pen paints at full pressure.
//!
//! Readings arrive on a background thread and go to the `sink` the application gives, as
//! pressure from 0 to 1. Only changes are sent; the application uses them while a stroke is
//! being painted.

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
#[cfg(all(unix, not(target_os = "macos")))]
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

#[cfg(all(unix, not(target_os = "macos")))]
mod x11;

/// Callback for pressure readings (0 to 1); called on a background thread.
pub type Sink = Box<dyn Fn(f32) + Send + Sync>;

/// The listener works from its thread and stops in `Drop`; nothing reads it.
#[allow(dead_code)]
enum Imp {
    #[cfg(all(unix, not(target_os = "macos")))]
    X11(x11::Listener),
    None,
}

/// Pen pressure for one window (Linux X11). Create it once the window exists.
pub struct PenPressure {
    imp: Imp,
}

impl std::fmt::Debug for PenPressure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.imp {
            #[cfg(all(unix, not(target_os = "macos")))]
            Imp::X11(_) => "PenPressure(XInput 2 raw events)",
            Imp::None => "PenPressure(none: winit's own, Wayland, or unsupported)",
        })
    }
}

impl PenPressure {
    /// Starts listening for pen pressure, handing readings to `sink`. Never fails: where winit
    /// already reports pressure (Windows, macOS), on Wayland, or where the X server cannot,
    /// nothing listens.
    pub fn new<W: HasWindowHandle + HasDisplayHandle>(window: &W, sink: Sink) -> PenPressure {
        let (Ok(wh), Ok(dh)) = (window.window_handle(), window.display_handle()) else {
            return PenPressure { imp: Imp::None };
        };
        let imp = match (wh.as_raw(), dh.as_raw()) {
            #[cfg(all(unix, not(target_os = "macos")))]
            (RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_), _) => {
                match x11::Listener::new(sink) {
                    Ok(l) => Imp::X11(l),
                    Err(e) => {
                        specular_log::info!(target: specular_log::INPUT, "pen pressure unavailable: {e}");
                        Imp::None
                    }
                }
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            (RawWindowHandle::Wayland(_), RawDisplayHandle::Wayland(_)) => {
                specular_log::debug!(target: specular_log::INPUT, "pen pressure: none on Wayland (winit 0.30 has no tablet input)");
                drop(sink);
                Imp::None
            }
            _ => {
                drop(sink);
                Imp::None
            }
        };
        PenPressure { imp }
    }

    /// Whether this listens for anything.
    pub fn active(&self) -> bool {
        !matches!(self.imp, Imp::None)
    }
}
