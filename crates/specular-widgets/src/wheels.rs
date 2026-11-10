//! A colour wheel: a hue and a strength picked on a disc (hue around it, strength out from
//! the centre). Develop shows three, for shadows, midtones and highlights, each with a
//! luminance slider under it.
//!
//! - **Pointer:** press or drag on the disc to put the puck there; Shift while dragging
//!   moves it ten times slower; double-click resets.
//! - **Keys:** arrows move the puck (Shift coarsely, Alt or Ctrl finely); Delete or
//!   Backspace resets.
//!
//! Hue is an angle, counter-clockwise from red at the right. The disc is a colour space,
//! not a reading direction: it is never mirrored.

use specular_gfx::{DrawList, Rect};
use specular_i18n::{I18n, t};
use specular_input::{Key, KeyCombo, NamedKey};
use specular_theme::{Color, Palette};

use crate::slider::KeyResult;

/// Rings and wedges the disc is drawn with.
const RINGS: usize = 5;
const WEDGES: usize = 40;

/// A colour wheel's hue (degrees, 0–360) and strength (0–1).
#[derive(Clone, Debug, PartialEq)]
pub struct Wheel {
    pub label: String,
    pub hue: f32,
    pub strength: f32,
    drag: bool,
}

impl Wheel {
    pub fn new(label: impl Into<String>) -> Wheel {
        Wheel {
            label: label.into(),
            hue: 0.0,
            strength: 0.0,
            drag: false,
        }
    }

    /// The puck as x, y on the unit disc (y up).
    pub fn xy(&self) -> (f32, f32) {
        let a = self.hue.to_radians();
        (self.strength * a.cos(), self.strength * a.sin())
    }

    /// Puts the puck at (x, y) on the unit disc (y up; clamped to the disc); whether it moved.
    pub fn set_xy(&mut self, x: f32, y: f32) -> bool {
        let (h, s) = (self.hue, self.strength);
        let r = x.hypot(y);
        if r < 1e-4 {
            self.strength = 0.0;
        } else {
            self.strength = r.min(1.0);
            self.hue = y.atan2(x).to_degrees().rem_euclid(360.0);
        }
        (h, s) != (self.hue, self.strength)
    }

    /// Sets hue and strength (clamped); whether they changed.
    pub fn set(&mut self, hue: f32, strength: f32) -> bool {
        if !hue.is_finite() || !strength.is_finite() {
            return false;
        }
        let new = (hue.rem_euclid(360.0), strength.clamp(0.0, 1.0));
        let changed = new != (self.hue, self.strength);
        (self.hue, self.strength) = new;
        changed
    }

    pub fn reset(&mut self) -> bool {
        self.set(0.0, 0.0)
    }

    /// A key; see the module docs.
    pub fn key(&mut self, combo: KeyCombo) -> KeyResult {
        let m = combo.mods;
        let step = if m.shift {
            0.1
        } else if m.alt || m.ctrl {
            0.005
        } else {
            0.02
        };
        let (x, y) = self.xy();
        let changed = match combo.key {
            Key::Named(NamedKey::ArrowLeft) => self.set_xy(x - step, y),
            Key::Named(NamedKey::ArrowRight) => self.set_xy(x + step, y),
            Key::Named(NamedKey::ArrowUp) => self.set_xy(x, y + step),
            Key::Named(NamedKey::ArrowDown) => self.set_xy(x, y - step),
            Key::Named(NamedKey::Delete | NamedKey::Backspace) => self.reset(),
            _ => return KeyResult::Ignored,
        };
        if changed {
            KeyResult::Changed
        } else {
            KeyResult::Handled
        }
    }

    /// A press at `p` on the disc in `r`: the puck goes there. Whether it moved.
    pub fn press(&mut self, p: (f32, f32), r: Rect) -> bool {
        self.drag = true;
        let (x, y) = unit(p, r);
        self.set_xy(x, y)
    }

    /// A drag by (dx, dy) logical px over the disc in `r`; `fine` (Shift) is ten times
    /// slower. Whether it moved.
    pub fn drag_by(&mut self, dx: f32, dy: f32, r: Rect, fine: bool) -> bool {
        if !self.drag {
            return false;
        }
        let k = if fine { 0.1 } else { 1.0 } / (r.w * 0.5).max(1.0);
        let (x, y) = self.xy();
        self.set_xy(x + dx * k, y - dy * k)
    }

    pub fn release(&mut self) {
        self.drag = false;
    }

    pub fn dragging(&self) -> bool {
        self.drag
    }

    /// "Hue 30°, strength 20%" (or "Neutral").
    pub fn value_text(&self) -> String {
        if self.strength <= 0.0 {
            return t!("develop-wheel-neutral");
        }
        let l = I18n::global().localizer();
        t!(
            "develop-wheel-value",
            hue = l.number(f64::from(self.hue.round()), 0),
            strength = l.percent(f64::from(self.strength), 0)
        )
    }

    /// Draws the disc into the square `r`.
    pub fn draw(&self, dl: &mut DrawList, pal: &Palette, r: Rect, focused: bool) {
        let c = r.center();
        let rad = r.w.min(r.h) * 0.5;
        let ring_w = rad / RINGS as f32;
        let sweep = std::f32::consts::TAU / WEDGES as f32;
        let grey = mid_grey(pal);
        for ring in 0..RINGS {
            let s = (ring as f32 + 0.5) / RINGS as f32;
            let rr = ring_w * (ring as f32 + 0.5);
            for w in 0..WEDGES {
                let hue = (w as f32 + 0.5) / WEDGES as f32 * 360.0;
                // Screen angles run clockwise (y down): a hue's angle is negated.
                let start = -(w as f32 + 1.0) * sweep;
                let col = mix(grey, hue_colour(hue), s);
                // Wedges overlap a little so no seams show.
                dl.arc(c, rr, start - 0.01, sweep + 0.02, ring_w + 0.6, col);
            }
        }
        dl.stroke_rect(
            Rect::new(c.0 - rad, c.1 - rad, 2.0 * rad, 2.0 * rad),
            rad,
            pal.separator,
            1.0,
        );
        // Crosshair at the centre.
        dl.line((c.0 - 4.0, c.1), (c.0 + 4.0, c.1), 1.0, pal.text_faint);
        dl.line((c.0, c.1 - 4.0), (c.0, c.1 + 4.0), 1.0, pal.text_faint);
        let (x, y) = self.xy();
        let p = (c.0 + x * (rad - 1.0), c.1 - y * (rad - 1.0));
        let pr = 6.0;
        dl.circle(p, pr + 1.5, Color::BLACK.with_alpha(0.6));
        dl.circle(p, pr, Color::WHITE);
        dl.circle(p, pr - 2.5, mix(grey, hue_colour(self.hue), self.strength));
        if focused {
            dl.rect_border(
                Rect::new(
                    c.0 - rad - 3.0,
                    c.1 - rad - 3.0,
                    2.0 * rad + 6.0,
                    2.0 * rad + 6.0,
                ),
                rad + 3.0,
                Color::TRANSPARENT,
                pal.focus_ring,
                2.0,
            );
        }
    }

    /// The disc's screen-reader node: a slider whose value is the hue and strength (arrows
    /// move it). `bounds` in physical px.
    pub fn a11y_node(&self, bounds: accesskit::Rect) -> accesskit::Node {
        use accesskit::{Action, Node, Role};
        let mut n = Node::new(Role::Slider);
        n.set_label(self.label.as_str());
        n.set_value(self.value_text());
        n.set_description(t!("develop-wheel-keys"));
        n.set_bounds(bounds);
        n.add_action(Action::Focus);
        n
    }
}

/// `p` over the disc in `r` as unit-disc coordinates (y up).
fn unit(p: (f32, f32), r: Rect) -> (f32, f32) {
    let c = r.center();
    let rad = (r.w.min(r.h) * 0.5).max(1.0);
    ((p.0 - c.0) / rad, (c.1 - p.1) / rad)
}

/// Whether `p` is on the disc in `r`.
pub fn on_disc(p: (f32, f32), r: Rect) -> bool {
    let (x, y) = unit(p, r);
    x.hypot(y) <= 1.0
}

fn mid_grey(pal: &Palette) -> Color {
    // The disc's centre: a grey that reads on both themes.
    let _ = pal;
    Color::rgba(0.5, 0.5, 0.5, 1.0)
}

fn mix(a: Color, b: Color, k: f32) -> Color {
    let k = k.clamp(0.0, 1.0);
    Color::rgba(
        a.r + (b.r - a.r) * k,
        a.g + (b.g - a.g) * k,
        a.b + (b.b - a.b) * k,
        1.0,
    )
}

/// A fully saturated colour of `hue` (degrees), as a medium-light sRGB colour.
pub fn hue_colour(hue: f32) -> Color {
    let h = hue.rem_euclid(360.0) / 60.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    // Softened towards white so the puck stays visible on any wedge.
    let soft = |v: f32| 0.15 + 0.8 * v;
    Color::rgba(soft(r), soft(g), soft(b), 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_input::Mods;

    #[test]
    fn arrows_move_the_puck_on_the_disc() {
        let mut w = Wheel::new("Shadows");
        let k = |n| KeyCombo::new(Key::Named(n), Mods::NONE);
        assert_eq!(w.key(k(NamedKey::ArrowRight)), KeyResult::Changed);
        assert!((w.hue - 0.0).abs() < 1e-4 && (w.strength - 0.02).abs() < 1e-6);
        w.key(k(NamedKey::ArrowLeft));
        w.key(k(NamedKey::ArrowLeft));
        assert!((w.hue - 180.0).abs() < 1e-3);
        w.key(KeyCombo::new(Key::Named(NamedKey::ArrowUp), Mods::SHIFT));
        let (x, y) = w.xy();
        assert!((x + 0.02).abs() < 1e-5 && (y - 0.1).abs() < 1e-5);
        // Never off the disc.
        for _ in 0..40 {
            w.key(KeyCombo::new(Key::Named(NamedKey::ArrowUp), Mods::SHIFT));
        }
        assert!(w.strength <= 1.0);
        assert_eq!(w.key(k(NamedKey::Delete)), KeyResult::Changed);
        assert_eq!(w.strength, 0.0);
        assert_eq!(w.key(k(NamedKey::Enter)), KeyResult::Ignored);
    }

    #[test]
    fn press_and_drag_follow_the_pointer() {
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let mut w = Wheel::new("Midtones");
        assert!(w.press((50.0, 0.0), r));
        assert!((w.hue - 90.0).abs() < 1e-3 && (w.strength - 1.0).abs() < 1e-6);
        // Fine: a tenth of the way.
        w.drag_by(0.0, 50.0, r, true);
        assert!((w.strength - 0.9).abs() < 1e-5);
        w.release();
        assert!(!w.drag_by(10.0, 0.0, r, false));
        assert!(on_disc((50.0, 50.0), r) && !on_disc((2.0, 2.0), r));
    }

    #[test]
    fn hue_colours_are_ordered() {
        let red = hue_colour(0.0);
        let green = hue_colour(120.0);
        let blue = hue_colour(240.0);
        assert!(red.r > red.g && red.r > red.b);
        assert!(green.g > green.r && green.g > green.b);
        assert!(blue.b > blue.r && blue.b > blue.g);
    }
}
