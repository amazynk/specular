//! A tone curve editor: points on a unit square joined by a monotone cubic (it never
//! overshoots between points, so a curve with rising points never falls).
//!
//! - **Pointer:** press on the curve's area to add a point there (or take the one under the
//!   pointer) and drag it; drag a middle point out of the square, or double-click it, to
//!   remove it. Shift while dragging moves ten times slower.
//! - **Keys:** arrows move the selected point (Shift coarsely, Alt or Ctrl finely); Page Up
//!   and Page Down select the previous or next point, Home and End the first or last; Insert
//!   or `+` adds a point halfway to the next one; Delete or `-` removes the selected point;
//!   Shift+Delete resets the curve.
//!
//! The square is a mathematical plot: it is never mirrored in right-to-left languages.

use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::{I18n, t};
use specular_input::{Key, KeyCombo, NamedKey};
use specular_theme::{Color, Palette, metrics};

use crate::slider::KeyResult;

/// Closest two points may come along x.
pub const MIN_GAP: f32 = 0.02;
/// Most points a curve holds.
pub const MAX_POINTS: usize = 16;
/// A press this close to a point (logical px) takes it.
const GRAB: f32 = 9.0;

/// A tone curve: points sorted by x, the first at x = 0 and the last at x = 1.
#[derive(Clone, Debug, PartialEq)]
pub struct Curve {
    pub label: String,
    pub points: Vec<[f32; 2]>,
    pub selected: usize,
    /// Dragging this point (pointer), and whether it has left the square.
    drag: Option<(usize, bool)>,
}

impl Curve {
    /// The identity: (0, 0) to (1, 1).
    pub fn identity(label: impl Into<String>) -> Curve {
        Curve {
            label: label.into(),
            points: vec![[0.0, 0.0], [1.0, 1.0]],
            selected: 0,
            drag: None,
        }
    }

    /// Takes `points` (x, y pairs) when they make a valid curve; whether it changed.
    pub fn set_points(&mut self, points: &[[f32; 2]]) -> bool {
        let ok = points.len() >= 2
            && points.len() <= MAX_POINTS
            && points[0][0] == 0.0
            && points[points.len() - 1][0] == 1.0
            && points.windows(2).all(|w| w[1][0] > w[0][0])
            && points
                .iter()
                .all(|p| p.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
        if !ok || points == self.points.as_slice() {
            return false;
        }
        self.points = points.to_vec();
        self.selected = self.selected.min(self.points.len() - 1);
        true
    }

    pub fn is_identity(&self) -> bool {
        self.points == [[0.0, 0.0], [1.0, 1.0]]
    }

    /// The curve at `x` (0–1): a monotone cubic (Fritsch–Carlson) through the points.
    pub fn eval(&self, x: f32) -> f32 {
        eval(&self.points, x)
    }

    /// Adds a point on the curve at `x`; its index, or `None` when too close to another or
    /// the curve is full.
    pub fn add_at(&mut self, x: f32) -> Option<usize> {
        let y = self.eval(x);
        self.insert([x, y])
    }

    fn insert(&mut self, p: [f32; 2]) -> Option<usize> {
        if self.points.len() >= MAX_POINTS || !(MIN_GAP..=1.0 - MIN_GAP).contains(&p[0]) {
            return None;
        }
        if self.points.iter().any(|q| (q[0] - p[0]).abs() < MIN_GAP) {
            return None;
        }
        let i = self.points.iter().position(|q| q[0] > p[0])?;
        self.points.insert(i, [p[0], p[1].clamp(0.0, 1.0)]);
        self.selected = i;
        Some(i)
    }

    /// Removes point `i` (never the first or last); whether it did.
    pub fn remove(&mut self, i: usize) -> bool {
        if i == 0 || i + 1 >= self.points.len() {
            return false;
        }
        self.points.remove(i);
        self.selected = (i - 1).min(self.points.len() - 1);
        true
    }

    /// Moves point `i` to (x, y), kept between its neighbours; the ends move only up and
    /// down. Whether it moved.
    pub fn move_to(&mut self, i: usize, x: f32, y: f32) -> bool {
        let n = self.points.len();
        let Some(&old) = self.points.get(i) else {
            return false;
        };
        let x = if i == 0 {
            0.0
        } else if i + 1 == n {
            1.0
        } else {
            let lo = self.points[i - 1][0] + MIN_GAP;
            let hi = self.points[i + 1][0] - MIN_GAP;
            x.clamp(lo, hi.max(lo))
        };
        let p = [x, y.clamp(0.0, 1.0)];
        self.points[i] = p;
        p != old
    }

    /// A key; see the module docs.
    pub fn key(&mut self, combo: KeyCombo) -> KeyResult {
        let m = combo.mods;
        let step = if m.shift {
            0.05
        } else if m.alt || m.ctrl {
            0.002
        } else {
            0.01
        };
        let i = self.selected.min(self.points.len() - 1);
        let [x, y] = self.points[i];
        let moved = match combo.key {
            Key::Named(NamedKey::ArrowLeft) => self.move_to(i, x - step, y),
            Key::Named(NamedKey::ArrowRight) => self.move_to(i, x + step, y),
            Key::Named(NamedKey::ArrowUp) => self.move_to(i, x, y + step),
            Key::Named(NamedKey::ArrowDown) => self.move_to(i, x, y - step),
            Key::Named(NamedKey::PageUp) => {
                self.selected = i.saturating_sub(1);
                return KeyResult::Handled;
            }
            Key::Named(NamedKey::PageDown) => {
                self.selected = (i + 1).min(self.points.len() - 1);
                return KeyResult::Handled;
            }
            Key::Named(NamedKey::Home) => {
                self.selected = 0;
                return KeyResult::Handled;
            }
            Key::Named(NamedKey::End) => {
                self.selected = self.points.len() - 1;
                return KeyResult::Handled;
            }
            Key::Named(NamedKey::Delete | NamedKey::Backspace) if m.shift => {
                let was = self.points.clone();
                *self = Curve::identity(std::mem::take(&mut self.label));
                was != self.points
            }
            Key::Named(NamedKey::Delete | NamedKey::Backspace) => self.remove(i),
            Key::Char('-') if !m.ctrl && !m.alt => self.remove(i),
            Key::Named(NamedKey::Insert) => self.add_after(i),
            Key::Char('+' | '=') if !m.ctrl && !m.alt => self.add_after(i),
            _ => return KeyResult::Ignored,
        };
        if moved {
            KeyResult::Changed
        } else {
            KeyResult::Handled
        }
    }

    /// Adds a point on the curve halfway between `i` and the next one.
    fn add_after(&mut self, i: usize) -> bool {
        let j = (i + 1).min(self.points.len() - 1);
        let (a, b) = if j > i { (i, j) } else { (i - 1, i) };
        let x = (self.points[a][0] + self.points[b][0]) * 0.5;
        self.add_at(x).is_some()
    }

    /// The point under `p` in a square drawn at `r`.
    fn point_at(&self, p: (f32, f32), r: Rect) -> Option<usize> {
        let mut best = None;
        let mut best_d = GRAB * GRAB;
        for (i, q) in self.points.iter().enumerate() {
            let (qx, qy) = to_screen(*q, r);
            let d = (qx - p.0).powi(2) + (qy - p.1).powi(2);
            if d <= best_d {
                best_d = d;
                best = Some(i);
            }
        }
        best
    }

    /// A press at `p` in the square `r`: takes the point under it or adds one on the curve
    /// at its x. Whether the curve changed (a point added).
    pub fn press(&mut self, p: (f32, f32), r: Rect) -> bool {
        if let Some(i) = self.point_at(p, r) {
            self.selected = i;
            self.drag = Some((i, false));
            return false;
        }
        let [x, _] = to_unit(p, r);
        match self.add_at(x) {
            Some(i) => {
                // Straight onto the pointer's height, as it is dragged from there.
                let [_, y] = to_unit(p, r);
                self.move_to(i, x, y);
                self.drag = Some((i, false));
                true
            }
            None => false,
        }
    }

    /// The pointer moved to `p` while pressed; whether the curve changed. A middle point
    /// dragged well out of the square is shown as going and is removed on release.
    pub fn drag_to(&mut self, p: (f32, f32), r: Rect) -> bool {
        let Some((i, _)) = self.drag else {
            return false;
        };
        let out = i > 0 && i + 1 < self.points.len() && !r.inset(-2.0 * GRAB).contains(p);
        self.drag = Some((i, out));
        let [x, y] = to_unit(p, r);
        self.move_to(i, x, y)
    }

    /// The pointer was let go; whether the curve changed (a point dragged out and removed).
    pub fn release(&mut self) -> bool {
        match self.drag.take() {
            Some((i, true)) => self.remove(i),
            _ => false,
        }
    }

    /// A double-click at `p`: removes the middle point under it.
    pub fn double_click(&mut self, p: (f32, f32), r: Rect) -> bool {
        self.drag = None;
        self.point_at(p, r).is_some_and(|i| self.remove(i))
    }

    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// "Point 2 of 4: in 25%, out 30%".
    pub fn point_text(&self) -> String {
        let l = I18n::global().localizer();
        let i = self.selected.min(self.points.len() - 1);
        let [x, y] = self.points[i];
        t!(
            "develop-curve-point",
            index = (i + 1) as i64,
            count = self.points.len() as i64,
            input = l.percent(f64::from(x), 0),
            output = l.percent(f64::from(y), 0)
        )
    }

    /// Draws the editor into the square `r`; `focused` rings it and marks the selected point.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        r: Rect,
        ts: f32,
        focused: bool,
    ) {
        dl.rect(r, metrics::RADIUS_SM, pal.surface);
        // Quarter grid and the identity diagonal.
        for k in 1..4 {
            let f = k as f32 / 4.0;
            let x = r.x + f * r.w;
            let y = r.y + f * r.h;
            dl.line((x, r.y), (x, r.bottom()), 1.0, pal.separator);
            dl.line((r.x, y), (r.right(), y), 1.0, pal.separator);
        }
        dl.line((r.x, r.bottom()), (r.right(), r.y), 1.0, pal.text_faint);
        // The curve.
        const SEGMENTS: usize = 64;
        let mut prev = to_screen([0.0, self.eval(0.0)], r);
        for k in 1..=SEGMENTS {
            let x = k as f32 / SEGMENTS as f32;
            let p = to_screen([x, self.eval(x)], r);
            dl.line(prev, p, 2.0, pal.text);
            prev = p;
        }
        let going = match self.drag {
            Some((i, true)) => Some(i),
            _ => None,
        };
        for (i, q) in self.points.iter().enumerate() {
            let c = to_screen(*q, r);
            let sel = i == self.selected;
            let rad = if sel { 5.5 } else { 4.5 };
            let fill = if going == Some(i) {
                pal.danger
            } else if sel {
                pal.accent
            } else {
                pal.surface_raised
            };
            dl.circle(c, rad + 1.5, pal.text);
            dl.circle(c, rad, fill);
            if sel && focused {
                dl.rect_border(
                    Rect::new(
                        c.0 - rad - 4.0,
                        c.1 - rad - 4.0,
                        2.0 * rad + 8.0,
                        2.0 * rad + 8.0,
                    ),
                    rad + 4.0,
                    Color::TRANSPARENT,
                    pal.focus_ring,
                    2.0,
                );
            }
        }
        if focused {
            // What the keyboard is on, under the square's top edge.
            let style = TextStyle::new(metrics::FONT_SMALL * ts).tabular();
            let b = text.layout(&self.point_text(), style, Some(r.w - 12.0), Align::Start);
            let tag = Rect::new(r.x + 4.0, r.y + 4.0, b.width + 8.0, b.height + 4.0);
            dl.rect(tag, metrics::RADIUS_SM, pal.surface_raised.with_alpha(0.9));
            text.draw(dl, &b, tag.x + 4.0, tag.y + 2.0, pal.text);
            dl.rect_border(
                r,
                metrics::RADIUS_SM,
                Color::TRANSPARENT,
                pal.focus_ring,
                2.0,
            );
        } else {
            dl.stroke_rect(r, metrics::RADIUS_SM, pal.separator, 1.0);
        }
    }

    /// The editor's screen-reader node: a group named after the curve whose value says which
    /// point is selected and where it is. `bounds` in physical px.
    pub fn a11y_node(&self, bounds: accesskit::Rect) -> accesskit::Node {
        use accesskit::{Action, Node, Role};
        let mut n = Node::new(Role::Group);
        n.set_label(self.label.as_str());
        n.set_value(self.point_text());
        n.set_description(t!("develop-curve-keys"));
        n.set_bounds(bounds);
        n.add_action(Action::Focus);
        n
    }
}

/// A unit-square point in the square `r` (y up).
fn to_screen(p: [f32; 2], r: Rect) -> (f32, f32) {
    (r.x + p[0] * r.w, r.bottom() - p[1] * r.h)
}

/// A screen point in the square `r` as unit coordinates (clamped).
fn to_unit(p: (f32, f32), r: Rect) -> [f32; 2] {
    [
        ((p.0 - r.x) / r.w.max(1.0)).clamp(0.0, 1.0),
        ((r.bottom() - p.1) / r.h.max(1.0)).clamp(0.0, 1.0),
    ]
}

/// The monotone cubic through `pts` (sorted by x) at `x`; constant beyond the ends.
pub fn eval(pts: &[[f32; 2]], x: f32) -> f32 {
    let n = pts.len();
    if n == 0 {
        return x;
    }
    if n == 1 || x <= pts[0][0] {
        return pts[0][1];
    }
    if x >= pts[n - 1][0] {
        return pts[n - 1][1];
    }
    // Secants, then tangents limited so each piece stays monotone (Fritsch–Carlson).
    let d: Vec<f32> = pts
        .windows(2)
        .map(|w| (w[1][1] - w[0][1]) / (w[1][0] - w[0][0]).max(f32::EPSILON))
        .collect();
    let mut m = vec![0.0f32; n];
    m[0] = d[0];
    m[n - 1] = d[n - 2];
    for i in 1..n - 1 {
        m[i] = if d[i - 1] * d[i] <= 0.0 {
            0.0
        } else {
            (d[i - 1] + d[i]) * 0.5
        };
    }
    for i in 0..n - 1 {
        if d[i] == 0.0 {
            m[i] = 0.0;
            m[i + 1] = 0.0;
            continue;
        }
        let a = m[i] / d[i];
        let b = m[i + 1] / d[i];
        let s = a * a + b * b;
        if s > 9.0 {
            let k = 3.0 / s.sqrt();
            m[i] = k * a * d[i];
            m[i + 1] = k * b * d[i];
        }
    }
    let i = pts.windows(2).position(|w| x < w[1][0]).unwrap_or(n - 2);
    let (x0, y0) = (pts[i][0], pts[i][1]);
    let (x1, y1) = (pts[i + 1][0], pts[i + 1][1]);
    let h = x1 - x0;
    let t = (x - x0) / h;
    let (t2, t3) = (t * t, t * t * t);
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;
    h00 * y0 + h10 * h * m[i] + h01 * y1 + h11 * h * m[i + 1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_input::Mods;

    fn k(n: NamedKey, mods: Mods) -> KeyCombo {
        KeyCombo::new(Key::Named(n), mods)
    }

    #[test]
    fn identity_is_the_diagonal() {
        let c = Curve::identity("Curve");
        for i in 0..=10 {
            let x = i as f32 / 10.0;
            assert!((c.eval(x) - x).abs() < 1e-6);
        }
    }

    #[test]
    fn rising_points_never_fall_or_overshoot() {
        let pts = [
            [0.0, 0.0],
            [0.2, 0.5],
            [0.25, 0.52],
            [0.7, 0.55],
            [1.0, 1.0],
        ];
        let mut last = -1.0;
        for i in 0..=1000 {
            let x = i as f32 / 1000.0;
            let y = eval(&pts, x);
            assert!(y >= last - 1e-6, "falls at {x}");
            assert!((0.0..=1.0).contains(&y));
            last = y;
        }
        // Through every point.
        for p in pts {
            assert!((eval(&pts, p[0]) - p[1]).abs() < 1e-5);
        }
    }

    #[test]
    fn add_move_and_remove_by_keys() {
        let mut c = Curve::identity("Curve");
        assert_eq!(c.key(k(NamedKey::Insert, Mods::NONE)), KeyResult::Changed);
        assert_eq!(c.points.len(), 3);
        assert_eq!(c.selected, 1);
        assert!((c.points[1][0] - 0.5).abs() < 1e-6);
        assert_eq!(c.key(k(NamedKey::ArrowUp, Mods::SHIFT)), KeyResult::Changed);
        assert!((c.points[1][1] - 0.55).abs() < 1e-6);
        c.key(k(NamedKey::ArrowLeft, Mods::NONE));
        assert!((c.points[1][0] - 0.49).abs() < 1e-6);
        // The ends move only up and down.
        c.key(k(NamedKey::Home, Mods::NONE));
        c.key(k(NamedKey::ArrowRight, Mods::NONE));
        c.key(k(NamedKey::ArrowUp, Mods::NONE));
        assert_eq!(c.points[0], [0.0, 0.01]);
        // The first point is never removed.
        assert_eq!(c.key(k(NamedKey::Delete, Mods::NONE)), KeyResult::Handled);
        c.key(k(NamedKey::PageDown, Mods::NONE));
        assert_eq!(c.key(k(NamedKey::Delete, Mods::NONE)), KeyResult::Changed);
        assert_eq!(c.points.len(), 2);
        c.key(k(NamedKey::Delete, Mods::SHIFT));
        assert!(c.is_identity());
    }

    #[test]
    fn points_keep_their_order() {
        let mut c = Curve::identity("Curve");
        c.add_at(0.3);
        c.add_at(0.6);
        assert!(c.add_at(0.61).is_none(), "too close");
        let i = c.points.iter().position(|p| p[0] == 0.3).unwrap();
        c.move_to(i, 0.9, 0.5);
        assert!(c.points.windows(2).all(|w| w[1][0] > w[0][0]));
    }

    #[test]
    fn pointer_adds_drags_and_drags_out() {
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let mut c = Curve::identity("Curve");
        assert!(c.press((50.0, 30.0), r));
        assert_eq!(c.points.len(), 3);
        assert!((c.points[1][1] - 0.7).abs() < 1e-6);
        assert!(c.drag_to((40.0, 40.0), r));
        assert!(!c.release());
        // Pressed on the point: taken, then dragged out and removed.
        assert!(!c.press((40.0, 40.0), r));
        c.drag_to((40.0, 200.0), r);
        assert!(c.release());
        assert!(c.is_identity());
        c.press((50.0, 50.0), r);
        c.release();
        assert!(c.double_click((50.0, 50.0), r));
        assert!(c.is_identity());
    }
}
