//! Dragging the current image out of the window into other applications (DESIGN.md §11).
//!
//! winit can receive drops but cannot start a drag, so the source side lives here, per
//! windowing system:
//!
//! * Wayland: `wl_data_device.start_drag` on winit's own display connection (`wayland.rs`).
//! * X11: an XDND source over a separate x11rb connection (`x11.rs`).
//! * Windows (`DoDragDrop` with an `IDataObject` carrying `CF_HDROP`) and macOS
//!   (`NSDraggingSource` + `beginDraggingSessionWithItems`): not implemented yet;
//!   [`DragOut::supported`] is `false` there and [`DragOut::start`] returns
//!   [`DragError::Unsupported`]. Both need an OLE/AppKit object implemented in Rust and
//!   a modal loop run from the window's input callback.
//!
//! What is offered: the file as `text/uri-list` (`file:///…`, CRLF-terminated, as RFC 2483
//! asks) and its plain path as `text/plain`. Receivers copy or link the file; Specular never
//! moves it.

use std::path::PathBuf;

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
#[cfg(all(unix, not(target_os = "macos")))]
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) mod wayland;
#[cfg(all(unix, not(target_os = "macos")))]
mod x11;

/// The pointer over the window while files from another application are dragged onto it,
/// with the modifier keys held ([`DragOut::drop_pointer`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DropPointer {
    /// Window position, physical px.
    pub x: f64,
    pub y: f64,
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
}

/// Why a drag did not start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DragError {
    /// This platform or windowing system has no drag source here.
    Unsupported,
    /// Not possible right now (setup still running, no button press seen, a drag in progress).
    NotReady,
    Failed(String),
}

impl std::fmt::Display for DragError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DragError::Unsupported => f.write_str("dragging out is not supported here"),
            DragError::NotReady => f.write_str("drag source not ready"),
            DragError::Failed(e) => write!(f, "drag failed: {e}"),
        }
    }
}

impl std::error::Error for DragError {}

enum Imp {
    #[cfg(all(unix, not(target_os = "macos")))]
    Wayland(wayland::Source),
    #[cfg(all(unix, not(target_os = "macos")))]
    X11(x11::Source),
    None,
}

/// A drag source for one window. Create it once the window exists; keep it as long as the
/// window.
pub struct DragOut {
    imp: Imp,
}

impl std::fmt::Debug for DragOut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.imp {
            #[cfg(all(unix, not(target_os = "macos")))]
            Imp::Wayland(_) => "DragOut(Wayland)",
            #[cfg(all(unix, not(target_os = "macos")))]
            Imp::X11(_) => "DragOut(X11)",
            Imp::None => "DragOut(unsupported)",
        })
    }
}

impl DragOut {
    /// Sets up dragging for `window`. Never fails: on systems without support the result just
    /// reports [`DragOut::supported`] `false`.
    pub fn new<W: HasWindowHandle + HasDisplayHandle>(window: &W) -> DragOut {
        let (Ok(wh), Ok(dh)) = (window.window_handle(), window.display_handle()) else {
            return DragOut { imp: Imp::None };
        };
        let imp = match (wh.as_raw(), dh.as_raw()) {
            #[cfg(all(unix, not(target_os = "macos")))]
            (RawWindowHandle::Wayland(w), RawDisplayHandle::Wayland(d)) => {
                match wayland::Source::new(d.display.as_ptr(), w.surface.as_ptr()) {
                    Ok(s) => Imp::Wayland(s),
                    Err(e) => {
                        specular_log::info!(target: specular_log::UI, "drag-out unavailable: {e}");
                        Imp::None
                    }
                }
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            (RawWindowHandle::Xlib(w), _) => match u32::try_from(w.window) {
                Ok(id) => Imp::X11(x11::Source::new(id)),
                Err(_) => Imp::None,
            },
            #[cfg(all(unix, not(target_os = "macos")))]
            (RawWindowHandle::Xcb(w), _) => Imp::X11(x11::Source::new(w.window.get())),
            _ => Imp::None,
        };
        DragOut { imp }
    }

    /// Whether [`start`](Self::start) can work on this system.
    pub fn supported(&self) -> bool {
        !matches!(self.imp, Imp::None)
    }

    /// Whether a drag this source started is still running (X11 only; on Wayland the
    /// compositor runs the drag and the window simply loses the pointer).
    pub fn active(&self) -> bool {
        match &self.imp {
            #[cfg(all(unix, not(target_os = "macos")))]
            Imp::X11(s) => s.active(),
            _ => false,
        }
    }

    /// Where the pointer is over the window during a drop from another application, and the
    /// modifier keys: winit's file-drop events carry neither. X11 only (a pointer query); None
    /// on Wayland (winit has no drop support there), Windows and macOS (no position without
    /// an OLE `IDropTarget` / `NSDraggingDestination` of our own).
    pub fn drop_pointer(&self) -> Option<DropPointer> {
        match &self.imp {
            #[cfg(all(unix, not(target_os = "macos")))]
            Imp::X11(s) => s.pointer(),
            _ => None,
        }
    }

    /// Starts dragging `files`. Call it from the window's pointer-motion handling while the
    /// primary button is still down; afterwards the window should stop treating the press as a
    /// pan (it may never see the release).
    pub fn start(&self, files: &[PathBuf]) -> Result<(), DragError> {
        if files.is_empty() {
            return Err(DragError::NotReady);
        }
        let (uris, text) = payload(files);
        match &self.imp {
            #[cfg(all(unix, not(target_os = "macos")))]
            Imp::Wayland(s) => s.start(uris, text),
            #[cfg(all(unix, not(target_os = "macos")))]
            Imp::X11(s) => s.start(uris, text),
            Imp::None => {
                let _ = (uris, text);
                Err(DragError::Unsupported)
            }
        }
    }
}

/// Runs one drag's work (`thread`) behind a boundary, then clears `busy`: a panic costs that
/// drag (logged), never every drag after it.
#[cfg_attr(any(not(unix), target_os = "macos"), allow(dead_code))]
fn one_drag(thread: &str, busy: &std::sync::atomic::AtomicBool, job: impl FnOnce()) {
    let mut job = Some(job);
    specular_log::service::supervise(
        thread,
        specular_log::service::Policy::Stop,
        || false,
        || {
            specular_log::service::trip();
            if let Some(j) = job.take() {
                j();
            }
        },
        |_| {},
    );
    busy.store(false, std::sync::atomic::Ordering::Release);
}

/// `text/uri-list` (CRLF after each URI) and `text/plain` (paths, one per line).
fn payload(files: &[PathBuf]) -> (Vec<u8>, Vec<u8>) {
    let mut uris = String::new();
    let mut text = String::new();
    for f in files {
        uris.push_str(&crate::launch::file_uri(f));
        uris.push_str("\r\n");
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&f.to_string_lossy());
    }
    (uris.into_bytes(), text.into_bytes())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    /// A drag whose thread panics ends; the next drag can start.
    #[test]
    #[allow(clippy::disallowed_methods)]
    fn a_panicking_drag_frees_the_next() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let busy = std::sync::Arc::new(AtomicBool::new(true));
        let b = busy.clone();
        std::thread::Builder::new()
            .name("test-drag".into())
            .spawn(move || {
                specular_log::service::arm("test-drag", 1);
                one_drag("test-drag", &b, || unreachable!("the hook panics first"));
            })
            .unwrap()
            .join()
            .unwrap();
        assert!(!busy.load(Ordering::Acquire));
        assert_eq!(specular_log::service::deaths("test-drag"), 1);
    }

    #[test]
    fn payload_is_a_uri_list() {
        let (u, t) = payload(&[PathBuf::from("/a b/c.jpg"), PathBuf::from("/d.png")]);
        assert_eq!(u, b"file:///a%20b/c.jpg\r\nfile:///d.png\r\n");
        assert_eq!(t, b"/a b/c.jpg\n/d.png");
    }
}
