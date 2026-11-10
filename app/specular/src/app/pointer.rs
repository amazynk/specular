//! When the mouse pointer hides: in full screen once the chrome has faded out, and in a
//! running slideshow, but only after the pointer has rested for a while, never during a drag
//! and never over chrome a mode keeps on screen (cull mode's strip). A mode that keeps the
//! toolbar away (cull mode, a slideshow) leaves the toolbar's alpha at zero all the time, so
//! "the chrome is hidden" alone is not a sign of idleness there: hiding on it showed the
//! pointer on every move and hid it again on the next frame, which flickered.
//!
//! The loupe (`q`) hides it too, anywhere, as long as the loupe shows over the image.

use specular_gfx::winit::window::CursorIcon;

use super::App;

impl App {
    /// This frame's pointer-hide decision.
    pub(super) fn pointer_hide(&self, now: f64) -> PointerHide {
        let over_chrome = self
            .pointer
            .is_some_and(|p| self.cull_over_strip(self.logical(p)));
        PointerHide {
            fullscreen: self.fullscreen,
            chrome_hidden: self.chrome.chrome_hidden(),
            slideshow_idle: self.slideshow.hides_pointer(now),
            idle: now - self.pointer_moved,
            // A movie's play mode hides it with its controls.
            delay: if self.video_full.active() {
                specular_widgets::PLAY_CONTROLS_IDLE
            } else {
                self.settings.viewing.toolbar_autohide_delay
            },
            over_chrome,
            dragging: self.dragging.is_some() || self.nav_drag || self.zoom_drag.is_some(),
            // `loupe_view` is None once the pointer leaves the image, a menu or dialog
            // opens, or the key is let go.
            loupe: self.loupe_view().is_some(),
        }
    }
}

/// What the decision looks at, gathered once per frame.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct PointerHide {
    pub fullscreen: bool,
    /// The toolbar has faded out and no dialog or go-to field is open.
    pub chrome_hidden: bool,
    /// A slideshow is running and its own pointer timer has run out (any window mode).
    pub slideshow_idle: bool,
    /// Seconds since the pointer last moved (more than a couple of pixels).
    pub idle: f64,
    /// How long the pointer must rest before it hides (the toolbar's auto-hide delay).
    pub delay: f64,
    /// The pointer is over chrome that stays on screen (cull mode's strip).
    pub over_chrome: bool,
    /// A pan, navigator or zoom drag is under way.
    pub dragging: bool,
    /// The loupe (`q`) shows over the image: it stands in for the pointer.
    pub loupe: bool,
}

impl PointerHide {
    /// Whether the pointer is hidden this frame.
    pub fn hide(&self) -> bool {
        if self.loupe {
            return true;
        }
        if self.dragging || self.over_chrome {
            return false;
        }
        self.slideshow_idle || self.fullscreen && self.chrome_hidden && self.idle >= self.delay
    }

    /// When the pointer would hide if it keeps still (s from now), so a frame comes then.
    pub fn wake(&self) -> Option<f64> {
        (self.fullscreen
            && self.chrome_hidden
            && !self.dragging
            && !self.over_chrome
            && self.idle < self.delay)
            .then_some(self.delay - self.idle)
    }
}

/// The pointer's shape: a hand over a control that acts on a click, the arrow elsewhere.
pub(super) fn icon(over_control: bool) -> CursorIcon {
    if over_control {
        CursorIcon::Pointer
    } else {
        CursorIcon::Default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Full screen in cull mode: the toolbar is always away.
    fn cull(idle: f64) -> PointerHide {
        PointerHide {
            fullscreen: true,
            chrome_hidden: true,
            idle,
            delay: 1.5,
            ..PointerHide::default()
        }
    }

    /// Moving the pointer in cull mode never hides it, frame after frame: no flicker.
    #[test]
    fn moving_in_cull_mode_keeps_the_pointer() {
        let mut shown = 0;
        // A frame every 16 ms while the pointer moves (idle stays near zero).
        for f in 0..200 {
            let idle = f64::from(f % 3) * 0.016;
            let h = cull(idle);
            assert!(!h.hide(), "hidden while moving at frame {f}");
            shown += 1;
        }
        assert_eq!(shown, 200);
    }

    /// Resting hides it once the delay has passed, and a frame is asked for then.
    #[test]
    fn hides_only_after_the_idle_delay() {
        assert!(!cull(1.0).hide());
        assert!((cull(1.0).wake().unwrap() - 0.5).abs() < 1e-9);
        assert!(cull(1.5).hide());
        assert!(cull(1.5).wake().is_none());
        assert!(cull(30.0).hide());
    }

    /// Never over the strip, never during a drag, however long it rests.
    #[test]
    fn never_over_the_strip_or_while_dragging() {
        let over = PointerHide {
            over_chrome: true,
            ..cull(60.0)
        };
        assert!(!over.hide());
        assert!(over.wake().is_none());
        let drag = PointerHide {
            dragging: true,
            ..cull(60.0)
        };
        assert!(!drag.hide());
        let slideshow = PointerHide {
            slideshow_idle: true,
            over_chrome: true,
            ..PointerHide::default()
        };
        assert!(!slideshow.hide());
    }

    /// Windowed, or with the toolbar up, it stays; a slideshow's own timer hides it anywhere.
    #[test]
    fn windowed_and_toolbar_up_keep_it() {
        let windowed = PointerHide {
            fullscreen: false,
            ..cull(60.0)
        };
        assert!(!windowed.hide());
        assert!(windowed.wake().is_none());
        let toolbar = PointerHide {
            chrome_hidden: false,
            ..cull(60.0)
        };
        assert!(!toolbar.hide());
        let show = PointerHide {
            slideshow_idle: true,
            ..PointerHide::default()
        };
        assert!(show.hide());
    }

    /// The loupe hides the pointer at once, windowed or not, even mid-drag; when it goes
    /// (key up, pointer off the image, a menu or dialog) the pointer is back on that frame.
    #[test]
    fn loupe_hides_the_pointer_while_it_shows() {
        let loupe = PointerHide {
            loupe: true,
            ..PointerHide::default()
        };
        assert!(loupe.hide());
        assert!(loupe.wake().is_none());
        assert!(
            PointerHide {
                dragging: true,
                ..loupe
            }
            .hide()
        );
        assert!(
            PointerHide {
                fullscreen: true,
                ..loupe
            }
            .hide()
        );
        let gone = PointerHide {
            loupe: false,
            ..loupe
        };
        assert!(!gone.hide());
        // In full screen with the toolbar away, it comes back until the usual idle delay.
        assert!(
            !PointerHide {
                loupe: false,
                ..cull(0.1)
            }
            .hide()
        );
    }

    #[test]
    fn hand_over_controls_only() {
        assert_eq!(icon(true), CursorIcon::Pointer);
        assert_eq!(icon(false), CursorIcon::Default);
    }
}
