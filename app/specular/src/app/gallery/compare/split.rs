//! Compare's split view (DESIGN.md §6.3): the second image over the first, cut at a divider.
//! The first image is on the reading direction's start side (left, or right in a
//! right-to-left language), as Develop's before/after split; turned across, the first is
//! on top. Two `compare_image` draws, the second clipped: no shader of its own.

use specular_gfx::Rect;

/// `[` / `]` move the divider by this much of the pane.
pub(super) const STEP: f32 = 0.05;
/// How near the divider (logical px) a press grabs it (its strip is a control).
pub(super) const GRAB: f32 = 10.0;

/// The divider: where it is (0‥1 of the pane from the start edge, or the top) and which way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct Split {
    pub at: f32,
    /// Across (the first image on top) rather than down.
    pub across: bool,
    /// Being dragged.
    pub drag: bool,
}

impl Default for Split {
    fn default() -> Self {
        Split {
            at: 0.5,
            across: false,
            drag: false,
        }
    }
}

impl Split {
    /// The divider's line in pane `r` (logical px): x for a divider down, else y.
    pub(super) fn line(&self, r: Rect, rtl: bool) -> f32 {
        let t = self.at.clamp(0.0, 1.0);
        match (self.across, rtl) {
            (true, _) => r.y + r.h * t,
            (false, false) => r.x + r.w * t,
            (false, true) => r.right() - r.w * t,
        }
    }

    /// The part of `r` the second image shows in: the end side of the divider.
    pub(super) fn clip(&self, r: Rect, rtl: bool) -> Rect {
        let m = self.line(r, rtl);
        match (self.across, rtl) {
            (true, _) => Rect::new(r.x, m, r.w, r.bottom() - m),
            (false, false) => Rect::new(m, r.y, r.right() - m, r.h),
            (false, true) => Rect::new(r.x, r.y, m - r.x, r.h),
        }
    }

    /// The divider moved to the pointer at `p` over pane `r`.
    pub(super) fn drag_to(&mut self, p: (f32, f32), r: Rect, rtl: bool) {
        let t = if self.across {
            (p.1 - r.y) / r.h.max(1.0)
        } else if rtl {
            (r.right() - p.0) / r.w.max(1.0)
        } else {
            (p.0 - r.x) / r.w.max(1.0)
        };
        self.at = t.clamp(0.0, 1.0);
    }

    /// `[` (towards the start) or `]`.
    pub(super) fn step(&mut self, up: bool) {
        let t = self.at + if up { STEP } else { -STEP };
        self.at = ((t / STEP).round() * STEP).clamp(0.0, 1.0);
    }

    /// The divider as a percentage (screen readers).
    pub(super) fn percent(&self) -> f64 {
        f64::from((self.at * 100.0).round())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_divider_clamps_and_mirrors() {
        let r = Rect::new(100.0, 50.0, 400.0, 200.0);
        let mut s = Split::default();
        assert_eq!(s.line(r, false), 300.0);
        // The second image right of the divider, or left of it right to left.
        assert_eq!(s.clip(r, false), Rect::new(300.0, 50.0, 200.0, 200.0));
        s.drag_to((200.0, 0.0), r, false);
        assert!((s.at - 0.25).abs() < 1e-6);
        assert_eq!(s.line(r, false), 200.0);
        assert_eq!(s.line(r, true), 400.0);
        assert_eq!(s.clip(r, true), Rect::new(100.0, 50.0, 300.0, 200.0));
        // Dragged past either edge: held at it.
        s.drag_to((-50.0, 0.0), r, false);
        assert_eq!(s.at, 0.0);
        s.drag_to((900.0, 0.0), r, false);
        assert_eq!(s.at, 1.0);
        // Right to left, a drag to the right edge is the start.
        s.drag_to((500.0, 0.0), r, true);
        assert_eq!(s.at, 0.0);
        // Steps stay on the grid and in range.
        s.at = 0.98;
        s.step(true);
        assert_eq!(s.at, 1.0);
        for _ in 0..30 {
            s.step(false);
        }
        assert_eq!(s.at, 0.0);
        assert_eq!(s.percent(), 0.0);
    }

    #[test]
    fn across_the_first_image_is_on_top() {
        let r = Rect::new(0.0, 0.0, 400.0, 200.0);
        let mut s = Split {
            across: true,
            ..Split::default()
        };
        assert_eq!(s.line(r, true), 100.0);
        assert_eq!(s.clip(r, false), Rect::new(0.0, 100.0, 400.0, 100.0));
        s.drag_to((0.0, 150.0), r, true);
        assert!((s.at - 0.75).abs() < 1e-6);
    }
}
