//! A movie's full-screen play mode: `f` while a movie is shown (Settings › Keys can move it),
//! the zoom presets' Full screen, the transport's full-screen button, F11 or Enter on a movie,
//! and a double click where the setting gives one to full screen. The window goes full
//! screen, the picture is fitted on black (small movies enlarged), the chrome hides its bars
//! and panels and shows the transport over the picture while the pointer moves (widgets'
//! `Chrome::set_video_fullscreen`). Esc (before any other meaning) or the same key again
//! goes back to the windowed layout as it was: the window's full-screen state, size and
//! position, the zoom, and the panels (the chrome keeps them open underneath).
//!
//! The window is reached through [`WindowOps`], so tests never touch the desktop session.
//! A sound file has no picture to fill the screen: `f` does nothing there.

use specular_gfx::winit::dpi::{PhysicalPosition, PhysicalSize};
use specular_gfx::winit::window::{Fullscreen, Window};
use specular_render::{View, ZoomMode};

use super::App;

/// What play mode does to the window.
pub(super) trait WindowOps {
    fn is_fullscreen(&self) -> bool;
    fn set_fullscreen(&self, on: bool);
    /// Where the window is and how big, to put it back.
    fn geometry(&self) -> Geometry;
    /// Puts the window back where it was and as big (after leaving full screen).
    fn restore(&self, g: Geometry);
}

/// The windowed window's place and size (physical pixels).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Geometry {
    /// `None` where the platform does not say (Wayland).
    pub position: Option<(i32, i32)>,
    pub size: (u32, u32),
    /// Maximised: the window manager puts it back by itself.
    pub maximized: bool,
}

impl WindowOps for Window {
    fn is_fullscreen(&self) -> bool {
        self.fullscreen().is_some()
    }

    fn set_fullscreen(&self, on: bool) {
        Window::set_fullscreen(self, on.then_some(Fullscreen::Borderless(None)));
    }

    fn geometry(&self) -> Geometry {
        let s = self.inner_size();
        Geometry {
            position: self.outer_position().ok().map(|p| (p.x, p.y)),
            size: (s.width, s.height),
            maximized: self.is_maximized(),
        }
    }

    fn restore(&self, g: Geometry) {
        if g.maximized {
            return;
        }
        let _ = self.request_inner_size(PhysicalSize::new(g.size.0, g.size.1));
        if let Some((x, y)) = g.position {
            self.set_outer_position(PhysicalPosition::new(x, y));
        }
    }
}

/// The view's zoom as it was, to put back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Zoom {
    mode: ZoomMode,
    scale: f32,
    center: (f32, f32),
    upscale_small: bool,
}

impl Zoom {
    fn of(v: &View) -> Zoom {
        Zoom {
            mode: v.mode,
            scale: v.target_scale(),
            center: v.center(),
            upscale_small: v.upscale_small,
        }
    }

    fn restore(self, v: &mut View) {
        v.upscale_small = self.upscale_small;
        match self.mode {
            ZoomMode::Fit => v.zoom_fit(),
            ZoomMode::Fill => v.zoom_fill(),
            ZoomMode::FitWidth => v.zoom_fit_side(true, true),
            ZoomMode::FitHeight => v.zoom_fit_side(false, true),
            ZoomMode::Free => {
                v.zoom_to(self.scale, None);
                v.center_on(self.center, false);
            }
        }
        v.settle();
    }
}

/// The windowed layout play mode came from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Saved {
    /// The window was already full screen (F11): it stays so on leaving.
    pub was_fullscreen: bool,
    /// Where the window was (`None` when it was full screen).
    pub geometry: Option<Geometry>,
    pub zoom: Zoom,
}

/// Play mode's state: on while it holds the layout to go back to.
#[derive(Debug, Default)]
pub(super) struct VideoFull {
    saved: Option<Saved>,
}

impl VideoFull {
    pub fn active(&self) -> bool {
        self.saved.is_some()
    }

    /// Into play mode: remembers the layout, puts the window full screen and fits the
    /// picture (enlarging a small one). False when already on.
    pub fn enter(&mut self, win: Option<&dyn WindowOps>, view: &mut View) -> bool {
        if self.saved.is_some() {
            return false;
        }
        let was_fullscreen = win.is_some_and(|w| w.is_fullscreen());
        self.saved = Some(Saved {
            was_fullscreen,
            geometry: win.filter(|_| !was_fullscreen).map(|w| w.geometry()),
            zoom: Zoom::of(view),
        });
        if let Some(w) = win
            && !was_fullscreen
        {
            w.set_fullscreen(true);
        }
        fit(view);
        true
    }

    /// Out of play mode: the window and the zoom as they were. `None` when it was off.
    pub fn leave(&mut self, win: Option<&dyn WindowOps>, view: &mut View) -> Option<Saved> {
        let saved = self.saved.take()?;
        if let Some(w) = win
            && !saved.was_fullscreen
        {
            w.set_fullscreen(false);
            if let Some(g) = saved.geometry {
                w.restore(g);
            }
        }
        saved.zoom.restore(view);
        Some(saved)
    }
}

/// The picture fitted to the screen, enlarged when smaller.
fn fit(view: &mut View) {
    view.upscale_small = true;
    view.zoom_fit();
    view.settle();
}

impl App {
    fn window_ops(&self) -> Option<&dyn WindowOps> {
        self.gfx.as_ref().map(|g| &*g.window as &dyn WindowOps)
    }

    /// The movie on screen has a picture to fill the screen with (not a sound file).
    pub(super) fn video_picture_current(&self) -> bool {
        self.video_current() && self.video_facts().is_some_and(|f| f.sound.is_none())
    }

    /// `f` on a movie (and F11, Enter or the transport's button there): into play mode or
    /// back out. Nothing on a sound file.
    pub(super) fn video_full_toggle(&mut self) {
        if self.video_full.active() {
            self.video_full_leave();
        } else {
            self.video_full_enter();
        }
    }

    /// Into play mode when a movie with a picture is shown.
    pub(super) fn video_full_enter(&mut self) -> bool {
        if !self.video_picture_current() {
            return false;
        }
        let mut vf = std::mem::take(&mut self.video_full);
        let mut view = std::mem::take(&mut self.view);
        let entered = vf.enter(self.window_ops(), &mut view);
        self.view = view;
        self.video_full = vf;
        if entered {
            self.fullscreen = true;
            self.video_full_shown = self.shown;
            let now = self.now();
            self.chrome.set_video_fullscreen(true, now);
            self.after_view_change();
            self.scene = None;
        }
        entered
    }

    /// Back to the windowed layout as it was.
    pub(super) fn video_full_leave(&mut self) -> bool {
        let mut vf = std::mem::take(&mut self.video_full);
        let mut view = std::mem::take(&mut self.view);
        let saved = vf.leave(self.window_ops(), &mut view);
        self.view = view;
        self.video_full = vf;
        let Some(saved) = saved else { return false };
        self.fullscreen = saved.was_fullscreen;
        let now = self.now();
        self.chrome.set_video_fullscreen(false, now);
        self.after_view_change();
        self.scene = None;
        true
    }

    /// Each frame: play mode ends when the file shown is no longer a movie with a picture
    /// (once it is known), and a new movie shown in it is fitted again.
    pub(super) fn video_full_follow(&mut self) {
        if !self.video_full.active() {
            return;
        }
        let st = self.action_state();
        let settled = matches!(
            st.image,
            specular_input::ImageState::Ready | specular_input::ImageState::Failed
        ) || st.image == specular_input::ImageState::None;
        if settled && !self.video_picture_current() {
            self.video_full_leave();
            return;
        }
        if self.shown.is_some() && self.shown != self.video_full_shown {
            self.video_full_shown = self.shown;
            fit(&mut self.view);
            self.after_view_change();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use specular_render::PxRect;
    use specular_widgets::Chrome;

    use super::*;

    /// The window, stubbed: what play mode asked of it.
    #[derive(Default)]
    struct FakeWindow {
        fullscreen: Cell<bool>,
        geometry: Cell<Option<Geometry>>,
        calls: RefCell<Vec<String>>,
    }

    impl WindowOps for FakeWindow {
        fn is_fullscreen(&self) -> bool {
            self.fullscreen.get()
        }
        fn set_fullscreen(&self, on: bool) {
            self.fullscreen.set(on);
            self.calls.borrow_mut().push(format!("fullscreen {on}"));
        }
        fn geometry(&self) -> Geometry {
            self.geometry.get().unwrap()
        }
        fn restore(&self, g: Geometry) {
            self.geometry.set(Some(g));
            self.calls
                .borrow_mut()
                .push(format!("restore {:?}", g.size));
        }
    }

    fn movie_view() -> View {
        let mut v = View::new();
        v.set_viewport(PxRect {
            x: 0.0,
            y: 0.0,
            w: 1200.0,
            h: 800.0,
        });
        // A small movie: fit leaves it at 100 % unless enlarged.
        v.set_image(640, 360, Default::default());
        v.zoom_fit();
        v.settle();
        v
    }

    const WINDOWED: Geometry = Geometry {
        position: Some((120, 80)),
        size: (1200, 800),
        maximized: false,
    };

    #[test]
    fn enter_and_leave_restore_the_windowed_layout() {
        let win = FakeWindow::default();
        win.geometry.set(Some(WINDOWED));
        let mut view = movie_view();
        // Zoomed in and panned: the layout to come back to.
        view.zoom_to(3.0, None);
        view.center_on((100.0, 90.0), false);
        view.settle();
        let before = Zoom::of(&view);
        let mut chrome = Chrome::new();
        chrome.set_size(1200.0, 800.0);
        chrome.set_info(true, false);
        let inset = chrome.info_inset();
        assert!(inset > 0.0);

        let mut vf = VideoFull::default();
        assert!(vf.enter(Some(&win), &mut view));
        chrome.set_video_fullscreen(true, 1.0);
        assert!(vf.active());
        assert!(win.fullscreen.get());
        // Fitted and enlarged to the screen; the panel out of the way.
        assert_eq!(view.mode, ZoomMode::Fit);
        assert!(view.upscale_small);
        assert!((view.target_scale() - 1200.0 / 640.0).abs() < 1e-3);
        assert_eq!(chrome.info_inset(), 0.0);
        // A second enter changes nothing.
        assert!(!vf.enter(Some(&win), &mut view));

        // The user resizes nothing; the window manager moved the window while full screen.
        win.geometry.set(Some(Geometry {
            position: Some((0, 0)),
            size: (1920, 1080),
            maximized: false,
        }));
        let saved = vf.leave(Some(&win), &mut view).unwrap();
        chrome.set_video_fullscreen(false, 2.0);
        assert!(!vf.active());
        assert!(!saved.was_fullscreen);
        assert!(!win.fullscreen.get());
        assert_eq!(win.geometry.get(), Some(WINDOWED));
        assert_eq!(
            *win.calls.borrow(),
            ["fullscreen true", "fullscreen false", "restore (1200, 800)"]
        );
        assert_eq!(Zoom::of(&view), before);
        assert_eq!(chrome.info_inset(), inset);
        // Leaving twice does nothing.
        assert!(vf.leave(Some(&win), &mut view).is_none());
        assert_eq!(win.calls.borrow().len(), 3);
    }

    #[test]
    fn full_screen_before_stays_full_screen() {
        let win = FakeWindow::default();
        win.fullscreen.set(true);
        let mut view = movie_view();
        let before = Zoom::of(&view);
        let mut vf = VideoFull::default();
        assert!(vf.enter(Some(&win), &mut view));
        let saved = vf.leave(Some(&win), &mut view).unwrap();
        assert!(saved.was_fullscreen);
        assert!(saved.geometry.is_none());
        // The window was never touched: F11's full screen stays.
        assert!(win.calls.borrow().is_empty());
        assert!(win.fullscreen.get());
        assert_eq!(Zoom::of(&view), before);
    }

    #[test]
    fn a_fit_mode_comes_back_as_that_mode() {
        let mut view = movie_view();
        view.zoom_fit_side(true, true);
        let mut vf = VideoFull::default();
        vf.enter(None, &mut view);
        assert_eq!(view.mode, ZoomMode::Fit);
        vf.leave(None, &mut view);
        assert_eq!(view.mode, ZoomMode::FitWidth);
        assert!(!view.upscale_small);
    }
}
