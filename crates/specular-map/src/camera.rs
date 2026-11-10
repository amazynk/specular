//! The map camera: a centre in world units, a zoom level and the view's size in logical px.
//! At zoom `z` one world unit is `TILE · 2^z` logical px. The world repeats east–west; north–
//! south the view never leaves the world (the minimum zoom makes it at least as tall as the
//! view).

use crate::{MAX_ZOOM, TILE};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// The view's centre in world units (x in [0, 1)).
    pub center: (f64, f64),
    pub zoom: f64,
    /// The view's size in logical px.
    pub size: (f64, f64),
}

impl Default for Camera {
    fn default() -> Camera {
        Camera {
            center: (0.5, 0.5),
            zoom: 0.0,
            size: (TILE, TILE),
        }
    }
}

impl Camera {
    pub fn new(size: (f64, f64)) -> Camera {
        let mut c = Camera {
            size,
            ..Camera::default()
        };
        c.zoom = c.min_zoom();
        c.clamp();
        c
    }

    /// Logical px per world unit.
    pub fn scale(&self) -> f64 {
        TILE * self.zoom.exp2()
    }

    /// The least zoom: the world is then as tall as the view (or one tile, if taller).
    pub fn min_zoom(&self) -> f64 {
        (self.size.1.max(1.0) / TILE).log2().max(0.0)
    }

    /// The integer level clusters are taken from.
    pub fn level(&self) -> u8 {
        self.zoom.floor().clamp(0.0, MAX_ZOOM) as u8
    }

    /// Screen position (logical px from the view's top left) of a world point, taking the
    /// copy of the world nearest the centre.
    pub fn to_screen(&self, p: (f64, f64)) -> (f64, f64) {
        let s = self.scale();
        let dx = (p.0 - self.center.0 + 0.5).rem_euclid(1.0) - 0.5;
        (
            dx * s + self.size.0 / 2.0,
            (p.1 - self.center.1) * s + self.size.1 / 2.0,
        )
    }

    /// The world point under a screen position (x not wrapped).
    pub fn to_world(&self, p: (f64, f64)) -> (f64, f64) {
        let s = self.scale();
        (
            self.center.0 + (p.0 - self.size.0 / 2.0) / s,
            self.center.1 + (p.1 - self.size.1 / 2.0) / s,
        )
    }

    /// The world rectangle in view, `(x0, y0, x1, y1)`; x not wrapped, y may pass 0 and 1
    /// only when the view is taller than the world.
    pub fn visible(&self) -> (f64, f64, f64, f64) {
        let (x0, y0) = self.to_world((0.0, 0.0));
        let (x1, y1) = self.to_world(self.size);
        (x0, y0, x1, y1)
    }

    /// Keeps the centre in the world and the zoom in range.
    pub fn clamp(&mut self) {
        self.zoom = self
            .zoom
            .clamp(self.min_zoom(), MAX_ZOOM.max(self.min_zoom()));
        let half = self.size.1 / 2.0 / self.scale();
        self.center.0 = self.center.0.rem_euclid(1.0);
        self.center.1 = if half >= 0.5 {
            0.5
        } else {
            self.center.1.clamp(half, 1.0 - half)
        };
    }

    /// Moves the view by a screen offset (dragging the map by `d` moves the world by `d`).
    pub fn pan(&mut self, d: (f64, f64)) {
        let s = self.scale();
        self.center.0 -= d.0 / s;
        self.center.1 -= d.1 / s;
        self.clamp();
    }

    /// Zooms by `dz` levels keeping the world point under screen position `at` in place.
    pub fn zoom_about(&mut self, dz: f64, at: (f64, f64)) {
        let before = self.to_world(at);
        self.zoom += dz;
        self.clamp();
        let after = self.to_world(at);
        self.center.0 += before.0 - after.0;
        self.center.1 += before.1 - after.1;
        self.clamp();
    }

    /// Shows the world rectangle `(x0, y0, x1, y1)` (x1 may exceed 1 for a box across the
    /// antimeridian) with `margin` logical px around it, no deeper than `max_zoom`.
    pub fn fit(&mut self, b: (f64, f64, f64, f64), margin: f64, max_zoom: f64) {
        let w = (self.size.0 - 2.0 * margin).max(1.0);
        let h = (self.size.1 - 2.0 * margin).max(1.0);
        let bw = (b.2 - b.0).max(1e-12);
        let bh = (b.3 - b.1).max(1e-12);
        let s = (w / bw).min(h / bh);
        self.zoom = (s / TILE).log2().min(max_zoom);
        self.center = ((b.0 + b.2) / 2.0, (b.1 + b.3) / 2.0);
        self.clamp();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6
    }

    #[test]
    fn screen_and_world_agree() {
        let mut c = Camera::new((800.0, 600.0));
        c.zoom = 5.0;
        c.center = (0.3, 0.4);
        let p = (0.3012, 0.3995);
        assert!(close(c.to_world(c.to_screen(p)), p));
        assert!(close(c.to_screen(c.center), (400.0, 300.0)));
        // The nearest copy of the world: a point just west of the antimeridian seen from
        // just east of it.
        c.center = (0.001, 0.5);
        let (x, _) = c.to_screen((0.999, 0.5));
        assert!((x - (400.0 - 0.002 * c.scale())).abs() < 1e-6);
    }

    #[test]
    fn the_view_stays_in_the_world() {
        let mut c = Camera::new((800.0, 600.0));
        assert!(
            (c.scale() - 600.0).abs() < 1e-9,
            "the world fills the height"
        );
        c.pan((0.0, 10_000.0));
        assert_eq!(c.center.1, 0.5);
        c.zoom = 4.0;
        c.pan((0.0, 1e9));
        let (_, y0, _, _) = c.visible();
        assert!(y0.abs() < 1e-9, "top edge at the top of the world: {y0}");
        c.pan((-0.75 * c.scale(), 0.0));
        assert!((0.0..1.0).contains(&c.center.0));
        c.zoom = 99.0;
        c.clamp();
        assert_eq!(c.zoom, MAX_ZOOM);
    }

    #[test]
    fn zooming_keeps_the_point_under_the_pointer() {
        let mut c = Camera::new((1000.0, 700.0));
        c.zoom = 6.0;
        c.center = (0.47, 0.38);
        let at = (120.0, 610.0);
        let w = c.to_world(at);
        c.zoom_about(1.5, at);
        assert!((c.zoom - 7.5).abs() < 1e-12);
        assert!(close(c.to_world(at), w));
        c.zoom_about(-0.25, at);
        assert!(close(c.to_world(at), w));
    }

    #[test]
    fn fit_shows_the_box() {
        let mut c = Camera::new((1000.0, 700.0));
        let b = (0.47, 0.38, 0.48, 0.385);
        c.fit(b, 50.0, MAX_ZOOM);
        let (x0, y0, x1, y1) = c.visible();
        assert!(x0 <= b.0 && y0 <= b.1 && x1 >= b.2 && y1 >= b.3);
        // Tight in at least one direction: 900 px for 0.01 wide.
        assert!((c.scale() * 0.01 - 900.0).abs() < 1e-6);
        // A point: no deeper than asked.
        c.fit((0.2, 0.2, 0.2, 0.2), 50.0, 12.0);
        assert_eq!(c.zoom, 12.0);
    }
}
