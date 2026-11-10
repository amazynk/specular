//! Shapes in scene millimetres (y up): lines of a width as filled outlines, arcs through
//! three points, pad outlines, and KiCad text as text blocks.

use std::f64::consts::PI;

use specular_vector::kurbo::{
    self, Affine, Arc, BezPath, Cap, Circle, Join, Point, Shape as _, Stroke, StrokeOpts, Vec2,
};
use specular_vector::{HAlign, Paragraph, Span, TextBlock, VAlign};

use crate::sexpr::Node;

/// Curve flattening tolerance, millimetres.
pub const TOLERANCE: f64 = 0.002;

/// A KiCad point (y down) in scene coordinates (y up).
pub fn pt(x: f64, y: f64) -> Point {
    Point::new(x, -y)
}

/// `(name x y …)` → its point in scene coordinates.
pub fn xy(n: &Node<'_>) -> Option<Point> {
    Some(pt(n.num(0)?, n.num(1)?))
}

/// The point of child `name` (`(start x y)`).
pub fn child_xy(n: &Node<'_>, name: &str) -> Option<Point> {
    xy(n.child(name)?)
}

/// `(at x y [angle])`: the point and the angle in degrees (counter-clockwise on screen).
pub fn at(n: &Node<'_>) -> (Point, f64) {
    match n.child("at") {
        Some(a) => (
            pt(a.num(0).unwrap_or(0.0), a.num(1).unwrap_or(0.0)),
            a.num(2).unwrap_or(0.0),
        ),
        None => (Point::ORIGIN, 0.0),
    }
}

/// A placement: translate to `p`, rotate by `deg` counter-clockwise.
pub fn place(p: Point, deg: f64) -> Affine {
    Affine::translate(p.to_vec2()) * Affine::rotate(deg.to_radians())
}

/// A filled line `width` wide with round ends from `a` to `b`.
pub fn capsule(a: Point, b: Point, width: f64) -> BezPath {
    let r = (width / 2.0).max(1e-6);
    let d = b - a;
    let len = d.hypot();
    if len < 1e-9 {
        return Circle::new(a, r).to_path(TOLERANCE);
    }
    let ang = d.y.atan2(d.x);
    let n = Vec2::new(-d.y, d.x) / len * r;
    let mut p = BezPath::new();
    p.move_to(a + n);
    p.line_to(b + n);
    p.extend(Arc::new(b, (r, r), ang + PI / 2.0, -PI, 0.0).append_iter(TOLERANCE));
    p.line_to(a - n);
    p.extend(Arc::new(a, (r, r), ang - PI / 2.0, -PI, 0.0).append_iter(TOLERANCE));
    p.close_path();
    p
}

/// The outline of `path` stroked `width` wide with round caps and joins.
pub fn outline(path: &BezPath, width: f64) -> BezPath {
    let style = Stroke::new(width.max(1e-6))
        .with_caps(Cap::Round)
        .with_join(Join::Round);
    kurbo::stroke(path.iter(), &style, &StrokeOpts::default(), TOLERANCE)
}

/// The circle through `a`, `m` and `b` as (centre, radius), when they are not in line.
pub fn circle3(a: Point, m: Point, b: Point) -> Option<(Point, f64)> {
    let d = 2.0 * (a.x * (m.y - b.y) + m.x * (b.y - a.y) + b.x * (a.y - m.y));
    if d.abs() < 1e-12 {
        return None;
    }
    let (a2, m2, b2) = (
        a.to_vec2().hypot2(),
        m.to_vec2().hypot2(),
        b.to_vec2().hypot2(),
    );
    let cx = (a2 * (m.y - b.y) + m2 * (b.y - a.y) + b2 * (a.y - m.y)) / d;
    let cy = (a2 * (b.x - m.x) + m2 * (a.x - b.x) + b2 * (m.x - a.x)) / d;
    let c = Point::new(cx, cy);
    Some((c, (a - c).hypot()))
}

/// The arc from `a` through `m` to `b` as a path (a line when they are in line).
pub fn arc3(a: Point, m: Point, b: Point) -> BezPath {
    let mut p = BezPath::new();
    p.move_to(a);
    append_arc3(&mut p, a, m, b);
    p
}

/// Appends the arc from the current point `a` through `m` to `b`.
pub fn append_arc3(p: &mut BezPath, a: Point, m: Point, b: Point) {
    let Some((c, r)) = circle3(a, m, b) else {
        p.line_to(b);
        return;
    };
    let ang = |q: Point| (q.y - c.y).atan2(q.x - c.x);
    let (sa, ma, ea) = (ang(a), ang(m), ang(b));
    let norm = |x: f64| x.rem_euclid(2.0 * PI);
    // Counter-clockwise sweep from a to b, and whether m lies on it.
    let ccw = norm(ea - sa);
    let sweep = if norm(ma - sa) <= ccw {
        ccw
    } else {
        ccw - 2.0 * PI
    };
    p.extend(Arc::new(c, (r, r), sa, sweep, 0.0).append_iter(TOLERANCE));
}

/// The arc around `c` starting at `start`, `deg` degrees counter-clockwise on screen (the
/// KiCad 5 form of arcs).
pub fn arc_angle(c: Point, start: Point, deg: f64) -> BezPath {
    let r = (start - c).hypot();
    let sa = (start.y - c.y).atan2(start.x - c.x);
    let mut p = BezPath::new();
    p.move_to(start);
    p.extend(Arc::new(c, (r, r), sa, deg.to_radians(), 0.0).append_iter(TOLERANCE));
    p
}

/// A closed polygon.
pub fn polygon(points: &[Point]) -> BezPath {
    let mut p = BezPath::new();
    for (i, q) in points.iter().enumerate() {
        if i == 0 {
            p.move_to(*q);
        } else {
            p.line_to(*q);
        }
    }
    if !points.is_empty() {
        p.close_path();
    }
    p
}

/// A rectangle `w` × `h` centred on the origin with corners rounded by `r`.
pub fn round_rect(w: f64, h: f64, r: f64) -> BezPath {
    let r = r.clamp(0.0, w.min(h) / 2.0);
    kurbo::RoundedRect::new(-w / 2.0, -h / 2.0, w / 2.0, h / 2.0, r).to_path(TOLERANCE)
}

/// A stadium `w` × `h` centred on the origin (KiCad's oval).
pub fn oval(w: f64, h: f64) -> BezPath {
    if (w - h).abs() < 1e-9 {
        return Circle::new(Point::ORIGIN, w / 2.0).to_path(TOLERANCE);
    }
    let (a, b, t) = if w > h {
        let d = (w - h) / 2.0;
        (Point::new(-d, 0.0), Point::new(d, 0.0), h)
    } else {
        let d = (h - w) / 2.0;
        (Point::new(0.0, -d), Point::new(0.0, d), w)
    };
    capsule(a, b, t)
}

/// The points of a `(pts (xy …) (arc (start) (mid) (end)) …)` list as a path.
pub fn pts_path(pts: &Node<'_>, close: bool) -> BezPath {
    let mut p = BezPath::new();
    let mut first = true;
    for n in pts.lists() {
        match n.head() {
            Some("xy") => {
                if let Some(q) = xy(n) {
                    if first {
                        p.move_to(q);
                        first = false;
                    } else {
                        p.line_to(q);
                    }
                }
            }
            Some("arc") => {
                let (Some(a), Some(m), Some(b)) =
                    (child_xy(n, "start"), child_xy(n, "mid"), child_xy(n, "end"))
                else {
                    continue;
                };
                if first {
                    p.move_to(a);
                    first = false;
                } else {
                    p.line_to(a);
                }
                append_arc3(&mut p, a, m, b);
            }
            _ => {}
        }
    }
    if close && !first {
        p.close_path();
    }
    p
}

/// How KiCad text is set: `(effects (font (size h w) (thickness t) bold italic) (justify …)
/// hide)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effects {
    /// Glyph height and width, millimetres.
    pub size: (f64, f64),
    pub thickness: f64,
    pub bold: bool,
    pub italic: bool,
    pub halign: HAlign,
    pub valign: VAlign,
    pub mirror: bool,
    pub hidden: bool,
}

impl Default for Effects {
    fn default() -> Self {
        Effects {
            size: (1.27, 1.27),
            thickness: 0.0,
            bold: false,
            italic: false,
            halign: HAlign::Center,
            valign: VAlign::Middle,
            mirror: false,
            hidden: false,
        }
    }
}

impl Effects {
    /// Reads the `effects` child of `n` (defaults when missing), with `hide` flags of `n`
    /// itself, its `(hide yes)` and the effects' own.
    pub fn of(n: &Node<'_>) -> Effects {
        let mut e = Effects::default();
        if let Some(fx) = n.child("effects") {
            if let Some(f) = fx.child("font") {
                if let Some(s) = f.child("size") {
                    let h = s.num(0).unwrap_or(1.27);
                    e.size = (h, s.num(1).unwrap_or(h));
                }
                e.thickness = f.value_num("thickness").unwrap_or(0.0);
                e.bold = f.yes("bold").unwrap_or(false);
                e.italic = f.yes("italic").unwrap_or(false);
            }
            if let Some(j) = fx.child("justify") {
                for a in j.args() {
                    match a.atom() {
                        Some("left") => e.halign = HAlign::Left,
                        Some("right") => e.halign = HAlign::Right,
                        Some("top") => e.valign = VAlign::Top,
                        Some("bottom") => e.valign = VAlign::Bottom,
                        Some("mirror") => e.mirror = true,
                        _ => {}
                    }
                }
            }
            e.hidden = fx.yes("hide").unwrap_or(false);
        }
        if let Some(h) = n.yes("hide") {
            e.hidden = h;
        }
        e
    }
}

/// A text block for `text` at `p`, `deg` degrees counter-clockwise, in `fx`'s size and
/// alignment (mirrored text read from the other side).
pub fn text_block(text: &str, p: Point, deg: f64, fx: &Effects) -> TextBlock {
    let (h, w) = fx.size;
    let h = h.abs().max(1e-3);
    let mut transform = place(p, deg);
    if fx.mirror {
        transform *= Affine::scale_non_uniform(-1.0, 1.0);
    }
    let bold = fx.bold || fx.thickness > h * 0.18;
    TextBlock {
        transform,
        halign: fx.halign,
        valign: fx.valign,
        wrap: 0.0,
        fit: None,
        line_spacing: 1.0,
        paragraphs: text
            .split('\n')
            .map(|line| {
                let mut s = Span::new(line.trim_end_matches('\r'), 0, h);
                s.width_factor = (w.abs() / h).clamp(0.1, 10.0);
                s.bold = bold;
                s.italic = fx.italic;
                Paragraph {
                    spans: vec![s],
                    align: None,
                }
            })
            .collect(),
    }
}

/// KiCad text markup made plain: `~{…}` overbars, `_{…}` and `^{…}` sub- and superscripts
/// keep their text.
pub fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut depth = 0usize;
    while let Some(c) = chars.next() {
        match c {
            '~' | '_' | '^' if chars.peek() == Some(&'{') => {
                chars.next();
                depth += 1;
            }
            '}' if depth > 0 => depth -= 1,
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arcs_go_through_their_middle_point() {
        let (a, m, b) = (
            Point::new(1.0, 0.0),
            Point::new(0.0, 1.0),
            Point::new(-1.0, 0.0),
        );
        let (c, r) = circle3(a, m, b).unwrap();
        assert!(c.distance(Point::ORIGIN) < 1e-9 && (r - 1.0).abs() < 1e-9);
        let bb = arc3(a, m, b).bounding_box();
        assert!(bb.y1 > 0.99 && bb.y0 > -1e-6, "{bb:?}");
        // The other way round, through the bottom.
        let bb = arc3(a, Point::new(0.0, -1.0), b).bounding_box();
        assert!(bb.y0 < -0.99 && bb.y1 < 1e-6, "{bb:?}");
    }

    #[test]
    fn capsules_and_ovals_have_their_size() {
        let c = capsule(Point::new(0.0, 0.0), Point::new(10.0, 0.0), 2.0).bounding_box();
        assert!((c.x0 + 1.0).abs() < 1e-3 && (c.x1 - 11.0).abs() < 1e-3);
        assert!((c.y0 + 1.0).abs() < 1e-3 && (c.y1 - 1.0).abs() < 1e-3);
        let o = oval(1.0, 3.0).bounding_box();
        assert!((o.height() - 3.0).abs() < 1e-3 && (o.width() - 1.0).abs() < 1e-3);
        assert!((capsule(Point::ORIGIN, Point::ORIGIN, 1.0).area().abs() - PI / 4.0).abs() < 1e-3);
    }

    #[test]
    fn markup_is_plain() {
        assert_eq!(plain("~{RESET}"), "RESET");
        assert_eq!(plain("V_{CC}"), "VCC");
        assert_eq!(plain("a~b"), "a~b");
    }
}
