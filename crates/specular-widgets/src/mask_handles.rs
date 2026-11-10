//! On-picture handles of a Develop mask: the outline of a shape or
//! gradient, and handles to drag: the centre, the radii or ends, the rotation and the
//! feather ring; and the brush cursor.
//!
//! The application places the handles in screen coordinates (it maps the mask's source
//! coordinates through the warp) and turns moves back into the mask.
//!
//! - **Pointer:** press on a handle and drag; [`MaskHandles::hit`] picks the nearest
//!   handle within reach.
//! - **Keyboard** (while the picture has the keyboard): Tab and Shift+Tab walk the handles,
//!   arrows move the focused one a pixel (Shift ten), Esc lets go. Arrows follow the
//!   picture, not the reading direction: the image does not mirror in right-to-left
//!   languages, so neither do its handles.
//! - **Screen readers:** each handle is a node named by the application ("Ellipse centre",
//!   "Feather"), with its position as its value.

use specular_gfx::{DrawList, Rect};
use specular_input::{Key, KeyCombo, NamedKey};
use specular_theme::{Color, Palette};

/// Reach of a handle for the pointer (logical px at the standard text size).
pub const REACH: f32 = 12.0;
/// Handle radius.
const DOT: f32 = 5.0;

/// What a handle changes (the application decides what that means for its shape).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// Moves the whole shape.
    Centre,
    /// A radius, half size or gradient end.
    Size,
    /// Turns the shape.
    Rotate,
    /// The soft edge.
    Feather,
    /// A polygon's corner.
    Point,
}

/// One handle.
#[derive(Clone, Debug, PartialEq)]
pub struct Handle {
    pub role: Role,
    /// Screen position (logical px).
    pub at: (f32, f32),
    /// Name for screen readers and the status line.
    pub name: String,
}

/// A line to draw (an outline, the feather ring, a gradient's guides).
#[derive(Clone, Debug, PartialEq)]
pub struct Outline {
    pub points: Vec<(f32, f32)>,
    pub closed: bool,
    /// Drawn thinner and fainter (the feather ring, gradient limits).
    pub faint: bool,
}

/// What a key did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KeyOutcome {
    /// Focus moved to this handle.
    Focus(usize),
    /// Move this handle by (dx, dy) screen px.
    Move { handle: usize, dx: f32, dy: f32 },
    /// The keyboard leaves the handles.
    Leave,
}

/// The handles of the mask being edited; see the module docs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MaskHandles {
    pub handles: Vec<Handle>,
    pub outlines: Vec<Outline>,
    /// The handle the keyboard is on.
    pub focus: Option<usize>,
    /// The handle being dragged.
    pub drag: Option<usize>,
    pub hover: Option<usize>,
}

impl MaskHandles {
    /// The nearest handle within reach of `p`.
    pub fn hit(&self, p: (f32, f32), ts: f32) -> Option<usize> {
        let reach = REACH * ts.max(1.0);
        self.handles
            .iter()
            .enumerate()
            .map(|(i, h)| (i, (h.at.0 - p.0).hypot(h.at.1 - p.1)))
            .filter(|(_, d)| *d <= reach)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// Keeps focus, drag and hover within the handles after they changed.
    pub fn set(&mut self, handles: Vec<Handle>, outlines: Vec<Outline>) {
        let n = handles.len();
        self.handles = handles;
        self.outlines = outlines;
        for v in [&mut self.focus, &mut self.drag, &mut self.hover] {
            if v.is_some_and(|i| i >= n) {
                *v = None;
            }
        }
    }

    /// A key while the picture has the keyboard.
    pub fn key(&mut self, combo: KeyCombo) -> Option<KeyOutcome> {
        let n = self.handles.len();
        if n == 0 || combo.mods.ctrl || combo.mods.alt {
            return None;
        }
        let step = if combo.mods.shift { 10.0 } else { 1.0 };
        let mv = |dx: f32, dy: f32, f: Option<usize>| {
            f.map(|handle| KeyOutcome::Move {
                handle,
                dx: dx * step,
                dy: dy * step,
            })
        };
        match combo.key {
            Key::Named(NamedKey::Tab) => {
                let i = match (self.focus, combo.mods.shift) {
                    (None, false) => 0,
                    (None, true) => n - 1,
                    (Some(i), false) => (i + 1) % n,
                    (Some(i), true) => (i + n - 1) % n,
                };
                self.focus = Some(i);
                Some(KeyOutcome::Focus(i))
            }
            Key::Named(NamedKey::Escape) if self.focus.is_some() => {
                self.focus = None;
                Some(KeyOutcome::Leave)
            }
            Key::Named(NamedKey::ArrowLeft) => mv(-1.0, 0.0, self.focus),
            Key::Named(NamedKey::ArrowRight) => mv(1.0, 0.0, self.focus),
            Key::Named(NamedKey::ArrowUp) => mv(0.0, -1.0, self.focus),
            Key::Named(NamedKey::ArrowDown) => mv(0.0, 1.0, self.focus),
            _ => None,
        }
    }

    /// Draws the outlines and the handles.
    pub fn draw(&self, dl: &mut DrawList, pal: &Palette, ts: f32) {
        let g = ts.max(1.0);
        let shade = Color::rgba(0.0, 0.0, 0.0, 0.6);
        for o in &self.outlines {
            let (w, c) = if o.faint {
                (1.0 * g, Color::rgba(1.0, 1.0, 1.0, 0.6))
            } else {
                (1.5 * g, Color::rgba(1.0, 1.0, 1.0, 0.95))
            };
            let mut seg = |a: (f32, f32), b: (f32, f32)| {
                dl.line(a, b, w + 2.0 * g, shade);
                dl.line(a, b, w, c);
            };
            for p in o.points.windows(2) {
                seg(p[0], p[1]);
            }
            if o.closed
                && let (Some(&a), Some(&b)) = (o.points.last(), o.points.first())
            {
                seg(a, b);
            }
        }
        for (i, h) in self.handles.iter().enumerate() {
            let hot = Some(i) == self.drag || Some(i) == self.hover;
            let r = DOT * g * if hot { 1.3 } else { 1.0 };
            dl.circle(h.at, r + 1.5 * g, shade);
            let fill = match h.role {
                Role::Centre => pal.accent,
                Role::Feather => Color::rgba(1.0, 1.0, 1.0, 0.7),
                _ => Color::rgba(1.0, 1.0, 1.0, 1.0),
            };
            dl.circle(h.at, r, fill);
            if Some(i) == self.focus {
                let f = r + 4.0 * g;
                dl.stroke_rect(
                    Rect::new(h.at.0 - f, h.at.1 - f, 2.0 * f, 2.0 * f),
                    f,
                    pal.focus_ring,
                    2.0 * g,
                );
            }
        }
    }

    /// Screen-reader nodes under `parent` (ids `base + 1 + i`).
    pub fn a11y(&self, tree: &mut accesskit::TreeUpdate, base: u64, label: &str, scale: f64) {
        use accesskit::{Action, Node, NodeId, Role as R};
        if self.handles.is_empty() {
            return;
        }
        let mut group = Node::new(R::Group);
        group.set_label(label);
        let mut kids = Vec::new();
        for (i, h) in self.handles.iter().enumerate() {
            let id = NodeId(base + 1 + i as u64);
            let mut n = Node::new(R::Button);
            n.set_label(h.name.as_str());
            n.set_value(format!("{:.0}, {:.0}", h.at.0, h.at.1));
            n.add_action(Action::Focus);
            let r = (REACH as f64) * scale;
            n.set_bounds(accesskit::Rect {
                x0: f64::from(h.at.0) * scale - r,
                y0: f64::from(h.at.1) * scale - r,
                x1: f64::from(h.at.0) * scale + r,
                y1: f64::from(h.at.1) * scale + r,
            });
            kids.push(id);
            tree.nodes.push((id, n));
        }
        group.set_children(kids);
        let root = tree.tree.as_ref().map_or(NodeId(1), |t| t.root);
        if let Some((_, w)) = tree.nodes.iter_mut().find(|(id, _)| *id == root) {
            w.push_child(NodeId(base));
        }
        tree.nodes.push((NodeId(base), group));
        if let Some(f) = self.focus {
            tree.focus = NodeId(base + 1 + f as u64);
        }
    }
}

/// The brush cursor: the outer size and the inner, fully painted part.
pub fn brush_cursor(dl: &mut DrawList, at: (f32, f32), radius: f32, hardness: f32, ts: f32) {
    let g = ts.max(1.0);
    let ring = |dl: &mut DrawList, r: f32, w: f32, c: Color| {
        let n = 48;
        let pts: Vec<(f32, f32)> = (0..=n)
            .map(|i| {
                let a = i as f32 * std::f32::consts::TAU / n as f32;
                (at.0 + r * a.cos(), at.1 + r * a.sin())
            })
            .collect();
        for p in pts.windows(2) {
            dl.line(p[0], p[1], w, c);
        }
    };
    let shade = Color::rgba(0.0, 0.0, 0.0, 0.6);
    let white = Color::rgba(1.0, 1.0, 1.0, 0.9);
    ring(dl, radius, 3.0 * g, shade);
    ring(dl, radius, 1.2 * g, white);
    if hardness > 0.0 && hardness < 1.0 {
        ring(
            dl,
            radius * hardness,
            1.0 * g,
            Color::rgba(1.0, 1.0, 1.0, 0.5),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_input::Mods;

    fn hs() -> MaskHandles {
        let mut m = MaskHandles::default();
        m.set(
            vec![
                Handle {
                    role: Role::Centre,
                    at: (100.0, 100.0),
                    name: "Centre".into(),
                },
                Handle {
                    role: Role::Size,
                    at: (150.0, 100.0),
                    name: "Width".into(),
                },
            ],
            Vec::new(),
        );
        m
    }

    fn key(k: NamedKey, shift: bool) -> KeyCombo {
        KeyCombo {
            key: Key::Named(k),
            mods: Mods {
                shift,
                ..Mods::default()
            },
        }
    }

    #[test]
    fn tab_walks_and_arrows_move() {
        let mut m = hs();
        assert_eq!(m.key(key(NamedKey::Tab, false)), Some(KeyOutcome::Focus(0)));
        assert_eq!(m.key(key(NamedKey::Tab, false)), Some(KeyOutcome::Focus(1)));
        assert_eq!(m.key(key(NamedKey::Tab, false)), Some(KeyOutcome::Focus(0)));
        assert_eq!(m.key(key(NamedKey::Tab, true)), Some(KeyOutcome::Focus(1)));
        assert_eq!(
            m.key(key(NamedKey::ArrowLeft, true)),
            Some(KeyOutcome::Move {
                handle: 1,
                dx: -10.0,
                dy: 0.0
            })
        );
        assert_eq!(m.key(key(NamedKey::Escape, false)), Some(KeyOutcome::Leave));
        assert_eq!(m.key(key(NamedKey::ArrowUp, false)), None);
    }

    #[test]
    fn hit_takes_the_nearest_in_reach() {
        let m = hs();
        assert_eq!(m.hit((104.0, 101.0), 1.0), Some(0));
        assert_eq!(m.hit((140.0, 100.0), 1.0), Some(1));
        assert_eq!(m.hit((125.0, 140.0), 1.0), None);
    }
}
