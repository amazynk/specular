//! The mini player: a small window that stays on top, without its title bar, the picture
//! fitted and the controls shown over it while the pointer is there (as in full-screen play
//! mode). The same key (or leaving the movie) puts the window back as it was.
//!
//! The window is reached through [`MiniOps`], so tests never touch the desktop session.

use specular_gfx::winit::dpi::{PhysicalPosition, PhysicalSize};
use specular_gfx::winit::window::{Window, WindowLevel};

use super::super::App;

/// The mini player's long side (physical pixels at scale 1).
const LONG_SIDE: f64 = 480.0;

/// What the mini player does to the window.
pub(crate) trait MiniOps {
    fn size(&self) -> (u32, u32);
    fn position(&self) -> Option<(i32, i32)>;
    fn scale(&self) -> f64;
    /// Small, on top and without decorations (`true`), or back to a normal window.
    fn set_mini(&self, on: bool);
    fn resize(&self, size: (u32, u32));
    fn move_to(&self, at: (i32, i32));
}

impl MiniOps for Window {
    fn size(&self) -> (u32, u32) {
        let s = self.inner_size();
        (s.width, s.height)
    }
    fn position(&self) -> Option<(i32, i32)> {
        self.outer_position().ok().map(|p| (p.x, p.y))
    }
    fn scale(&self) -> f64 {
        self.scale_factor()
    }
    fn set_mini(&self, on: bool) {
        self.set_window_level(if on {
            WindowLevel::AlwaysOnTop
        } else {
            WindowLevel::Normal
        });
        self.set_decorations(!on);
    }
    fn resize(&self, size: (u32, u32)) {
        let _ = self.request_inner_size(PhysicalSize::new(size.0, size.1));
    }
    fn move_to(&self, at: (i32, i32)) {
        self.set_outer_position(PhysicalPosition::new(at.0, at.1));
    }
}

/// The window before the mini player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Before {
    size: (u32, u32),
    position: Option<(i32, i32)>,
}

#[derive(Debug, Default)]
pub(crate) struct Mini {
    before: Option<Before>,
}

/// The mini window's size for a picture of `aspect` (width / height).
pub(crate) fn mini_size(aspect: f64, scale: f64) -> (u32, u32) {
    let a = if aspect.is_finite() && aspect > 0.1 {
        aspect
    } else {
        16.0 / 9.0
    };
    let long = LONG_SIDE * scale.max(0.5);
    if a >= 1.0 {
        (long.round() as u32, (long / a).round() as u32)
    } else {
        ((long * a).round() as u32, long.round() as u32)
    }
}

impl Mini {
    pub(crate) fn active(&self) -> bool {
        self.before.is_some()
    }

    /// Into the mini player.
    pub(crate) fn enter(&mut self, win: &dyn MiniOps, aspect: f64) {
        if self.before.is_some() {
            return;
        }
        self.before = Some(Before {
            size: win.size(),
            position: win.position(),
        });
        win.set_mini(true);
        win.resize(mini_size(aspect, win.scale()));
    }

    /// Back to the window as it was.
    pub(crate) fn leave(&mut self, win: &dyn MiniOps) -> bool {
        let Some(b) = self.before.take() else {
            return false;
        };
        win.set_mini(false);
        win.resize(b.size);
        if let Some(p) = b.position {
            win.move_to(p);
        }
        true
    }
}

impl App {
    /// Shift+m on a movie: into the mini player or back.
    pub(in crate::app) fn mini_toggle(&mut self) {
        if self.player.mini.active() {
            self.mini_leave();
            return;
        }
        if !self.video_picture_current() {
            return;
        }
        // From full-screen play mode, the window comes back first.
        if self.video_full.active() {
            self.video_full_leave();
        }
        let (dw, dh) = self.view.display_size();
        let aspect = f64::from(dw) / f64::from(dh.max(1.0));
        let Some(gfx) = self.gfx.as_ref() else { return };
        let win = std::sync::Arc::clone(&gfx.window);
        self.player.mini.enter(&*win, aspect);
        let now = self.now();
        // The bars and panels hide; the controls show over the picture on hover.
        self.chrome.set_video_fullscreen(true, now);
        self.view.upscale_small = true;
        self.view.zoom_fit();
        self.after_view_change();
        self.scene = None;
        self.redraw = true;
    }

    pub(in crate::app) fn mini_leave(&mut self) {
        let Some(gfx) = self.gfx.as_ref() else { return };
        let win = std::sync::Arc::clone(&gfx.window);
        if self.player.mini.leave(&*win) {
            let now = self.now();
            self.chrome.set_video_fullscreen(false, now);
            self.view.zoom_fit();
            self.after_view_change();
            self.scene = None;
            self.redraw = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use super::*;

    #[derive(Default)]
    struct Fake {
        size: Cell<(u32, u32)>,
        at: Cell<Option<(i32, i32)>>,
        mini: Cell<bool>,
        log: RefCell<Vec<String>>,
    }

    impl MiniOps for Fake {
        fn size(&self) -> (u32, u32) {
            self.size.get()
        }
        fn position(&self) -> Option<(i32, i32)> {
            self.at.get()
        }
        fn scale(&self) -> f64 {
            2.0
        }
        fn set_mini(&self, on: bool) {
            self.mini.set(on);
            self.log.borrow_mut().push(format!("mini {on}"));
        }
        fn resize(&self, s: (u32, u32)) {
            self.size.set(s);
        }
        fn move_to(&self, at: (i32, i32)) {
            self.at.set(Some(at));
        }
    }

    #[test]
    fn enter_and_leave_restore_the_window() {
        let f = Fake::default();
        f.size.set((1600, 1000));
        f.at.set(Some((40, 30)));
        let mut m = Mini::default();
        m.enter(&f, 16.0 / 9.0);
        assert!(m.active() && f.mini.get());
        assert_eq!(f.size.get(), (960, 540));
        f.at.set(Some((900, 700)));
        assert!(m.leave(&f));
        assert!(!f.mini.get());
        assert_eq!((f.size.get(), f.at.get()), ((1600, 1000), Some((40, 30))));
        assert!(!m.leave(&f), "only once");
    }

    #[test]
    fn portrait_videos_stand_up() {
        assert_eq!(mini_size(9.0 / 16.0, 1.0), (270, 480));
        assert_eq!(mini_size(f64::NAN, 1.0), (480, 270));
    }
}
