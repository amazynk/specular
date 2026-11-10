//! Shapes shared by the board formats that describe copper as features (ODB++, IPC-2581):
//! standard pad shapes, lines and arcs drawn with a round or square pen, and contours
//! (islands with holes), all into Gerber-like [`Object`]s in millimetres.

use std::f64::consts::{PI, TAU};

use specular_vector::kurbo::{Affine, BezPath, Point, Rect, Shape as _, Vec2};

use crate::geom;
use crate::gerber::{Object, ObjectKind, Segment};

/// A pad shape centred on the origin (mm).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Prim {
    Circle(f64),
    Rect(f64, f64),
    /// Width, height, corner radius.
    RoundRect(f64, f64, f64),
    /// Width, height, corner cut.
    Chamfer(f64, f64, f64),
    Oval(f64, f64),
    Diamond(f64, f64),
    /// Width, height, corner cut (an octagon).
    Octagon(f64, f64, f64),
    /// Outer and inner diameter.
    Donut(f64, f64),
    /// Outer and inner side.
    SquareDonut(f64, f64),
    /// Width, height, corner size (pointy left and right).
    Hexagon(f64, f64, f64),
    Ellipse(f64, f64),
    /// Base and height.
    Triangle(f64, f64),
    /// Outer and inner diameter, spoke angle (degrees), spokes, gap.
    Thermal(f64, f64, f64, u32, f64),
    /// Any closed outline (holes reversed).
    Path(BezPath),
}

fn rounded_rect(w: f64, h: f64, r: f64) -> BezPath {
    let r = r.clamp(0.0, w.min(h) / 2.0);
    if r <= geom::EPS {
        return geom::rect(Point::ZERO, w, h);
    }
    specular_vector::kurbo::RoundedRect::new(-w / 2.0, -h / 2.0, w / 2.0, h / 2.0, r)
        .to_path(geom::FLAT * 0.1)
}

fn cut_rect(w: f64, h: f64, c: f64) -> BezPath {
    let c = c.clamp(0.0, w.min(h) / 2.0);
    let (x, y) = (w / 2.0, h / 2.0);
    geom::polygon(&[
        Point::new(-x + c, -y),
        Point::new(x - c, -y),
        Point::new(x, -y + c),
        Point::new(x, y - c),
        Point::new(x - c, y),
        Point::new(-x + c, y),
        Point::new(-x, y - c),
        Point::new(-x, -y + c),
    ])
}

impl Prim {
    /// The outline at the origin, oriented for a non-zero fill.
    pub fn outline(&self) -> BezPath {
        let o = Point::ZERO;
        match *self {
            Prim::Circle(d) => geom::circle(o, d / 2.0),
            Prim::Rect(w, h) => geom::rect(o, w, h),
            Prim::RoundRect(w, h, r) => geom::orient(rounded_rect(w, h, r), false),
            Prim::Chamfer(w, h, c) | Prim::Octagon(w, h, c) => cut_rect(w, h, c),
            Prim::Oval(w, h) => geom::obround(o, w, h),
            Prim::Diamond(w, h) => geom::polygon(&[
                Point::new(w / 2.0, 0.0),
                Point::new(0.0, h / 2.0),
                Point::new(-w / 2.0, 0.0),
                Point::new(0.0, -h / 2.0),
            ]),
            Prim::Donut(od, id) => {
                let mut p = geom::circle(o, od / 2.0);
                if id > 0.0 && id < od {
                    geom::push(&mut p, geom::circle(o, id / 2.0), true);
                }
                p
            }
            Prim::SquareDonut(od, id) => {
                let mut p = geom::rect(o, od, od);
                if id > 0.0 && id < od {
                    geom::push(&mut p, geom::rect(o, id, id), true);
                }
                p
            }
            Prim::Hexagon(w, h, r) => {
                let (x, y) = (w / 2.0, h / 2.0);
                let r = r.clamp(0.0, x);
                geom::polygon(&[
                    Point::new(-x, 0.0),
                    Point::new(-x + r, -y),
                    Point::new(x - r, -y),
                    Point::new(x, 0.0),
                    Point::new(x - r, y),
                    Point::new(-x + r, y),
                ])
            }
            Prim::Ellipse(w, h) => {
                let e = specular_vector::kurbo::Ellipse::new(o, Vec2::new(w / 2.0, h / 2.0), 0.0);
                geom::orient(e.to_path(geom::FLAT * 0.1), false)
            }
            Prim::Triangle(b, h) => geom::polygon(&[
                Point::new(-b / 2.0, -h / 2.0),
                Point::new(b / 2.0, -h / 2.0),
                Point::new(0.0, h / 2.0),
            ]),
            Prim::Thermal(od, id, angle, n, gap) => thermal(od, id, angle, n.clamp(1, 16), gap),
            Prim::Path(ref p) => p.clone(),
        }
    }
}

/// A round thermal: the ring between `id` and `od` with `n` gaps of width `gap`, the first
/// at `angle` degrees.
fn thermal(od: f64, id: f64, angle: f64, n: u32, gap: f64) -> BezPath {
    let (ro, ri) = (od / 2.0, (id / 2.0).max(0.0));
    if ri >= ro {
        return BezPath::new();
    }
    let mid = (ro + ri) / 2.0;
    let hw = (ro - ri) / 2.0;
    // Half the gap as an angle at the ring's middle, plus the round ends.
    let half = ((gap / 2.0 + hw) / mid).min(PI / f64::from(n));
    let step = TAU / f64::from(n);
    let mut out = BezPath::new();
    for k in 0..n {
        let a0 = angle.to_radians() + step * f64::from(k) + half;
        let sweep = step - 2.0 * half;
        if sweep > 0.0 {
            geom::push(
                &mut out,
                geom::arc_band(Point::ZERO, mid, a0, sweep, hw),
                false,
            );
        }
    }
    out
}

/// A flash of `prim` at `at`, rotated `rot` degrees counter-clockwise after mirroring in x
/// when `mirror`.
pub(crate) fn flash(prim: &Prim, at: Point, rot: f64, mirror: bool, dark: bool) -> Object {
    let mut a = Affine::translate(at.to_vec2()) * Affine::rotate(rot.to_radians());
    if mirror {
        a *= Affine::FLIP_X;
    }
    let p = prim.outline();
    Object {
        path: geom::transformed(&p, a),
        dark,
        kind: ObjectKind::Flash,
        segment: None,
        holes: matches!(
            prim,
            Prim::Donut(..) | Prim::SquareDonut(..) | Prim::Thermal(..) | Prim::Path(_)
        ),
    }
}

/// The pen of a line or arc.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Pen {
    Round(f64),
    /// Square ends extended by half the width.
    Square(f64),
}

impl Pen {
    pub fn width(self) -> f64 {
        match self {
            Pen::Round(w) | Pen::Square(w) => w,
        }
    }
}

/// A straight draw from `a` to `b`.
pub(crate) fn line(a: Point, b: Point, pen: Pen, dark: bool) -> Object {
    let w = pen.width();
    let seg = Segment::Line(a, b);
    if w <= geom::EPS {
        let mut p = BezPath::new();
        p.move_to(a);
        p.line_to(b);
        return Object {
            path: p,
            dark,
            kind: ObjectKind::Hairline,
            segment: Some(seg),
            holes: false,
        };
    }
    let path = match pen {
        Pen::Round(_) => geom::capsule(a, b, w / 2.0),
        Pen::Square(_) => {
            let d = b - a;
            let len = d.hypot();
            let ang = if len > geom::EPS { d.atan2() } else { 0.0 };
            let c = a.midpoint(b);
            let r = geom::rect(Point::ZERO, len + w, w);
            geom::transformed(&r, Affine::translate(c.to_vec2()) * Affine::rotate(ang))
        }
    };
    Object {
        path,
        dark,
        kind: ObjectKind::Draw,
        segment: Some(seg),
        holes: false,
    }
}

/// An arc draw from `a` to `b` about `c`, clockwise when `cw` (a full circle when the ends
/// meet).
pub(crate) fn arc(a: Point, b: Point, c: Point, cw: bool, pen: Pen, dark: bool) -> Object {
    let r = (a - c).hypot();
    let a0 = (a - c).atan2();
    let a1 = (b - c).atan2();
    let sweep = geom::sweep(a0, a1, !cw, (a - b).hypot() < geom::EPS);
    let seg = Segment::Arc {
        center: c,
        radius: r,
        start: a0,
        sweep,
    };
    let w = pen.width();
    let path = if w <= geom::EPS {
        let mut p = BezPath::new();
        p.move_to(a);
        append_arc(&mut p, c, r, a0, sweep);
        p
    } else {
        geom::arc_band(c, r, a0, sweep, w / 2.0)
    };
    Object {
        path,
        dark,
        kind: if w <= geom::EPS {
            ObjectKind::Hairline
        } else {
            ObjectKind::Draw
        },
        segment: Some(seg),
        holes: false,
    }
}

fn append_arc(p: &mut BezPath, c: Point, r: f64, a0: f64, sweep: f64) {
    let arc = specular_vector::kurbo::Arc::new(c, Vec2::new(r, r), a0, sweep, 0.0);
    for el in arc.append_iter(geom::FLAT * 0.1) {
        p.push(el);
    }
}

/// Builds contours: moves, lines and arcs, closed into islands and holes.
#[derive(Default)]
pub(crate) struct Contour {
    cur: BezPath,
    at: Point,
    /// Islands and holes so far, oriented.
    pub path: BezPath,
    hole: bool,
    open: bool,
}

impl Contour {
    pub fn begin(&mut self, p: Point, hole: bool) {
        self.end();
        self.cur = BezPath::new();
        self.cur.move_to(p);
        self.at = p;
        self.hole = hole;
        self.open = true;
    }

    pub fn line_to(&mut self, p: Point) {
        if self.open {
            self.cur.line_to(p);
            self.at = p;
        }
    }

    pub fn arc_to(&mut self, p: Point, c: Point, cw: bool) {
        if !self.open {
            return;
        }
        let r = (self.at - c).hypot();
        let a0 = (self.at - c).atan2();
        let a1 = (p - c).atan2();
        let sweep = geom::sweep(a0, a1, !cw, (self.at - p).hypot() < geom::EPS);
        if r > geom::EPS {
            append_arc(&mut self.cur, c, r, a0, sweep);
        }
        self.cur.line_to(p);
        self.at = p;
    }

    pub fn end(&mut self) {
        if self.open {
            self.cur.close_path();
            let sub = std::mem::take(&mut self.cur);
            if sub.area().abs() > 1e-12 {
                geom::push(&mut self.path, sub, self.hole);
            }
            self.open = false;
        }
    }

    /// The contours as a region object.
    pub fn finish(mut self, dark: bool) -> Option<Object> {
        self.end();
        if self.path.elements().is_empty() {
            return None;
        }
        Some(Object {
            path: self.path,
            dark,
            kind: ObjectKind::Region,
            segment: None,
            holes: true,
        })
    }
}

/// Bounds of objects.
pub(crate) fn bounds(objs: &[Object]) -> Option<Rect> {
    let mut r: Option<Rect> = None;
    for o in objs {
        let b = o.path.bounding_box();
        if b.x0.is_finite() && b.y0.is_finite() && b.x1.is_finite() && b.y1.is_finite() {
            r = Some(r.map_or(b, |r| r.union(b)));
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_have_their_areas() {
        let area = |p: &Prim| p.outline().area();
        assert!((area(&Prim::Circle(2.0)) - PI).abs() < 1e-3);
        assert!((area(&Prim::Rect(2.0, 3.0)) - 6.0).abs() < 1e-9);
        assert!((area(&Prim::Donut(4.0, 2.0)) - 3.0 * PI).abs() < 1e-3);
        assert!((area(&Prim::SquareDonut(4.0, 2.0)) - 12.0).abs() < 1e-9);
        assert!((area(&Prim::Diamond(2.0, 2.0)) - 2.0).abs() < 1e-9);
        assert!((area(&Prim::Octagon(2.0, 2.0, 0.5)) - (4.0 - 0.5)).abs() < 1e-9);
        let rr = area(&Prim::RoundRect(2.0, 2.0, 0.5));
        assert!((rr - (4.0 - (4.0 - PI) * 0.25)).abs() < 1e-3, "{rr}");
        let t = area(&Prim::Thermal(4.0, 2.0, 45.0, 4, 0.5));
        assert!(t > 0.0 && t < 3.0 * PI, "{t}");
        let o = Prim::Oval(4.0, 2.0).outline().area();
        assert!((o - (PI + 4.0)).abs() < 1e-3, "{o}");
    }

    #[test]
    fn lines_arcs_and_contours() {
        let l = line(Point::ZERO, Point::new(10.0, 0.0), Pen::Square(1.0), true);
        assert!((l.path.area() - 11.0).abs() < 1e-9);
        let r = line(Point::ZERO, Point::new(10.0, 0.0), Pen::Round(1.0), true);
        assert!((r.path.area() - (10.0 + PI / 4.0)).abs() < 1e-3);
        // A half circle clockwise from (1,0) to (-1,0) about the origin goes below.
        let a = arc(
            Point::new(1.0, 0.0),
            Point::new(-1.0, 0.0),
            Point::ZERO,
            true,
            Pen::Round(0.0),
            true,
        );
        assert!(a.path.bounding_box().y0 < -0.99);
        let mut c = Contour::default();
        c.begin(Point::ZERO, false);
        for p in [(4.0, 0.0), (4.0, 4.0), (0.0, 4.0)] {
            c.line_to(p.into());
        }
        c.begin(Point::new(1.0, 1.0), true);
        for p in [(1.0, 2.0), (2.0, 2.0), (2.0, 1.0)] {
            c.line_to(p.into());
        }
        let o = c.finish(true).unwrap();
        assert!((o.path.area() - 15.0).abs() < 1e-9);
    }
}
