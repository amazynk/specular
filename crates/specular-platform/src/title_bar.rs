//! The window's title bar in the colour of the content under it, so the window reads as one
//! surface (macOS: the title bar made transparent over the window's background colour).
//! Elsewhere the system draws its own title bar, and this does nothing.

use raw_window_handle::HasWindowHandle;

/// Shows the title bar of `window` in `rgb` (sRGB, 0..=1), the colour along the top of the
/// content, with its title or without (`show_title`; the window keeps the title, for the
/// Window menu and screen readers). Call on the main thread, again whenever either changes.
pub fn blend(window: &dyn HasWindowHandle, rgb: [f32; 3], show_title: bool) {
    #[cfg(target_os = "macos")]
    macos::blend(window, rgb, show_title);
    #[cfg(not(target_os = "macos"))]
    let _ = (window, rgb, show_title);
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSColor, NSView, NSWindowTitleVisibility};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    #[allow(unsafe_code)]
    pub(super) fn blend(window: &dyn HasWindowHandle, rgb: [f32; 3], show_title: bool) {
        if MainThreadMarker::new().is_none() {
            return;
        }
        let Ok(handle) = window.window_handle() else {
            return;
        };
        let RawWindowHandle::AppKit(h) = handle.as_raw() else {
            return;
        };
        // SAFETY: winit's AppKit handle points at the window's live NSView, used on the main
        // thread (checked above) while the window exists (the caller holds it).
        let view: &NSView = unsafe { h.ns_view.cast::<NSView>().as_ref() };
        let Some(ns_window) = view.window() else {
            return;
        };
        let [r, g, b] = rgb.map(f64::from);
        let color = NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, 1.0);
        // The title bar shows the window's background through it, which is also what shows
        // for a moment while the window grows, before the content is drawn again.
        ns_window.setTitlebarAppearsTransparent(true);
        ns_window.setBackgroundColor(Some(&color));
        ns_window.setTitleVisibility(if show_title {
            NSWindowTitleVisibility::Visible
        } else {
            NSWindowTitleVisibility::Hidden
        });
    }
}
