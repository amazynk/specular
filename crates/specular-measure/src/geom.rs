//! Shapes and their calibrated geometry.

use std::f64::consts::PI;

use crate::calib::{Calibration, Quantity};

/// A point in stored pixel coordinates (see the crate docs).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct P {
    pub x: f64,
    pub y: f64,
}

impl P {
    pub const fn new(x: f64, y: f64) -> P {
        P { x, y }
    }

    /// The length of the vector.
    pub fn hypot(self) -> f64 {
        self.x.hypot(self.y)
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl std::ops::Sub for P {
    type Output = P;
    fn sub(self, o: P) -> P {
        P::new(self.x - o.x, self.y - o.y)
    }
}

impl std::ops::Add for P {
    type Output = P;
    fn add(self, o: P) -> P {
        P::new(self.x + o.x, self.y + o.y)
    }
}

/// A measurement's shape.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Point(P),
    Line(P, P),
    /// An open path of two or more points.
    Polyline(Vec<P>),
    /// The angle at `vertex` between the arms to `a` and `b`.
    Angle {
        a: P,
        vertex: P,
        b: P,
    },
    /// Corners in any order.
    Rect {
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
    },
    /// Axis-aligned.
    Ellipse {
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
    },
    /// A closed polygon (the last point joins the first).
    Polygon(Vec<P>),
    /// A closed hand-drawn outline; covered like a polygon.
    Freehand(Vec<P>),
}

/// Vertices of the polygon that approximates an ellipse in exports.
pub const ELLIPSE_VERTICES: usize = 72;

impl Shape {
    /// A rectangle from two opposite corners.
    pub fn rect(a: P, b: P) -> Shape {
        Shape::Rect {
            x0: a.x.min(b.x),
            y0: a.y.min(b.y),
            x1: a.x.max(b.x),
            y1: a.y.max(b.y),
        }
    }

    /// The ellipse inscribed in the box with opposite corners `a` and `b`.
    pub fn ellipse_in(a: P, b: P) -> Shape {
        Shape::Ellipse {
            cx: (a.x + b.x) / 2.0,
            cy: (a.y + b.y) / 2.0,
            rx: (b.x - a.x).abs() / 2.0,
            ry: (b.y - a.y).abs() / 2.0,
        }
    }

    /// A stable identifier for files.
    pub fn key(&self) -> &'static str {
        match self {
            Shape::Point(_) => "point",
            Shape::Line(..) => "line",
            Shape::Polyline(_) => "polyline",
            Shape::Angle { .. } => "angle",
            Shape::Rect { .. } => "rect",
            Shape::Ellipse { .. } => "ellipse",
            Shape::Polygon(_) => "polygon",
            Shape::Freehand(_) => "freehand",
        }
    }

    /// Whether the shape encloses pixels (has statistics and an area).
    pub fn is_area(&self) -> bool {
        matches!(
            self,
            Shape::Rect { .. } | Shape::Ellipse { .. } | Shape::Polygon(_) | Shape::Freehand(_)
        )
    }

    /// Whether the shape is a path a line profile can follow.
    pub fn is_path(&self) -> bool {
        matches!(self, Shape::Line(..) | Shape::Polyline(_))
    }

    /// The defining points: vertices, the two ends, the arms and vertex, or two box corners.
    pub fn points(&self) -> Vec<P> {
        match self {
            Shape::Point(p) => vec![*p],
            Shape::Line(a, b) => vec![*a, *b],
            Shape::Angle { a, vertex, b } => vec![*a, *vertex, *b],
            Shape::Rect { x0, y0, x1, y1 } => vec![P::new(*x0, *y0), P::new(*x1, *y1)],
            Shape::Ellipse { cx, cy, rx, ry } => {
                vec![P::new(cx - rx, cy - ry), P::new(cx + rx, cy + ry)]
            }
            Shape::Polyline(v) | Shape::Polygon(v) | Shape::Freehand(v) => v.clone(),
        }
    }

    /// The same shape moved by `d`.
    pub fn translated(&self, d: P) -> Shape {
        let t = |p: &P| *p + d;
        match self {
            Shape::Point(p) => Shape::Point(t(p)),
            Shape::Line(a, b) => Shape::Line(t(a), t(b)),
            Shape::Angle { a, vertex, b } => Shape::Angle {
                a: t(a),
                vertex: t(vertex),
                b: t(b),
            },
            Shape::Rect { x0, y0, x1, y1 } => Shape::Rect {
                x0: x0 + d.x,
                y0: y0 + d.y,
                x1: x1 + d.x,
                y1: y1 + d.y,
            },
            Shape::Ellipse { cx, cy, rx, ry } => Shape::Ellipse {
                cx: cx + d.x,
                cy: cy + d.y,
                rx: *rx,
                ry: *ry,
            },
            Shape::Polyline(v) => Shape::Polyline(v.iter().map(t).collect()),
            Shape::Polygon(v) => Shape::Polygon(v.iter().map(t).collect()),
            Shape::Freehand(v) => Shape::Freehand(v.iter().map(t).collect()),
        }
    }

    /// The same shape with every coordinate multiplied by `s` (about the origin): the shape on
    /// a copy of the image reduced by `1 / s`.
    pub fn scaled(&self, s: f64) -> Shape {
        let t = |p: &P| P::new(p.x * s, p.y * s);
        match self {
            Shape::Point(p) => Shape::Point(t(p)),
            Shape::Line(a, b) => Shape::Line(t(a), t(b)),
            Shape::Angle { a, vertex, b } => Shape::Angle {
                a: t(a),
                vertex: t(vertex),
                b: t(b),
            },
            Shape::Rect { x0, y0, x1, y1 } => Shape::Rect {
                x0: x0 * s,
                y0: y0 * s,
                x1: x1 * s,
                y1: y1 * s,
            },
            Shape::Ellipse { cx, cy, rx, ry } => Shape::Ellipse {
                cx: cx * s,
                cy: cy * s,
                rx: rx * s,
                ry: ry * s,
            },
            Shape::Polyline(v) => Shape::Polyline(v.iter().map(t).collect()),
            Shape::Polygon(v) => Shape::Polygon(v.iter().map(t).collect()),
            Shape::Freehand(v) => Shape::Freehand(v.iter().map(t).collect()),
        }
    }

    /// Whether every coordinate is finite and an area shape has at least three vertices.
    pub fn is_valid(&self) -> bool {
        let pts = self.points();
        pts.iter().all(|p| p.is_finite())
            && match self {
                Shape::Polyline(v) => v.len() >= 2,
                Shape::Polygon(v) | Shape::Freehand(v) => v.len() >= 3,
                _ => true,
            }
    }

    /// The bounding box `(x0, y0, x1, y1)`.
    pub fn bounds(&self) -> (f64, f64, f64, f64) {
        match *self {
            Shape::Rect { x0, y0, x1, y1 } => (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)),
            Shape::Ellipse { cx, cy, rx, ry } => {
                (cx - rx.abs(), cy - ry.abs(), cx + rx.abs(), cy + ry.abs())
            }
            _ => self.points().iter().fold(
                (
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                ),
                |(a, b, c, d), p| (a.min(p.x), b.min(p.y), c.max(p.x), d.max(p.y)),
            ),
        }
    }

    /// The outline as drawn and exported: the path for lines (and the two arms for an angle),
    /// the closed ring (without repeating the first point) for areas, an ellipse as
    /// [`ELLIPSE_VERTICES`] points.
    pub fn outline(&self) -> Vec<P> {
        match *self {
            Shape::Rect { x0, y0, x1, y1 } => vec![
                P::new(x0, y0),
                P::new(x1, y0),
                P::new(x1, y1),
                P::new(x0, y1),
            ],
            Shape::Ellipse { cx, cy, rx, ry } => (0..ELLIPSE_VERTICES)
                .map(|i| {
                    let t = 2.0 * PI * i as f64 / ELLIPSE_VERTICES as f64;
                    P::new(cx + rx * t.cos(), cy + ry * t.sin())
                })
                .collect(),
            _ => self.points(),
        }
    }

    /// Whether `p` is inside, by the coverage rules of the crate docs (areas only; a point
    /// shape contains the points of its pixel).
    pub fn contains(&self, p: P) -> bool {
        match *self {
            Shape::Rect { .. } => {
                let (x0, y0, x1, y1) = self.bounds();
                p.x >= x0 && p.x < x1 && p.y >= y0 && p.y < y1
            }
            Shape::Ellipse { cx, cy, rx, ry } => ellipse_contains(cx, cy, rx, ry, p.x, p.y),
            Shape::Polygon(ref v) | Shape::Freehand(ref v) => {
                crossings(v, p.y).filter(|&x| x <= p.x).count() % 2 == 1
            }
            Shape::Point(q) => q.x.floor() == p.x.floor() && q.y.floor() == p.y.floor(),
            _ => false,
        }
    }

    /// The path length of a line or polyline.
    pub fn length(&self, cal: &Calibration) -> Option<Quantity> {
        match self {
            Shape::Line(a, b) => Some(cal.length(cal.distance(*a, *b))),
            Shape::Polyline(v) => Some(cal.length(path_length(cal, v, false))),
            _ => None,
        }
    }

    /// The perimeter of an area shape. An ellipse's is Ramanujan's second approximation of
    /// the mapped ellipse (exact for circles; the relative error grows with eccentricity to
    /// at most 4·10⁻⁴ for a flat one).
    pub fn perimeter(&self, cal: &Calibration) -> Option<Quantity> {
        let v = match *self {
            Shape::Rect { .. } | Shape::Polygon(_) | Shape::Freehand(_) => {
                path_length(cal, &self.outline(), true)
            }
            Shape::Ellipse { rx, ry, .. } => {
                let (a, b) = mapped_ellipse_axes(cal, rx.abs(), ry.abs());
                let h = ((a - b) / (a + b)).powi(2);
                if a + b == 0.0 {
                    0.0
                } else {
                    PI * (a + b) * (1.0 + 3.0 * h / (10.0 + (4.0 - 3.0 * h).sqrt()))
                }
            }
            _ => return None,
        };
        Some(cal.length(v))
    }

    /// The geometric area: exact for rectangles and ellipses, the shoelace area for polygons
    /// (exact when simple; a self-intersecting outline counts its lobes with opposite signs).
    pub fn area(&self, cal: &Calibration) -> Option<Quantity> {
        let px = self.area_px()?;
        Some(cal.area(px * cal.pixel_area()))
    }

    /// The geometric area in pixels².
    pub fn area_px(&self) -> Option<f64> {
        Some(match *self {
            Shape::Rect { .. } => {
                let (x0, y0, x1, y1) = self.bounds();
                (x1 - x0) * (y1 - y0)
            }
            Shape::Ellipse { rx, ry, .. } => PI * rx.abs() * ry.abs(),
            Shape::Polygon(ref v) | Shape::Freehand(ref v) => shoelace(v).abs(),
            _ => return None,
        })
    }

    /// The angle in degrees (0..=180) at the vertex, in physical space.
    pub fn angle(&self, cal: &Calibration) -> Option<Quantity> {
        let Shape::Angle { a, vertex, b } = *self else {
            return None;
        };
        let u = cal.map(a.x - vertex.x, a.y - vertex.y);
        let w = cal.map(b.x - vertex.x, b.y - vertex.y);
        let (nu, nw) = (u.hypot(), w.hypot());
        if nu == 0.0 || nw == 0.0 {
            return None;
        }
        // atan2 of cross and dot keeps precision near 0° and 180°.
        let cross = u.x * w.y - u.y * w.x;
        let dot = u.x * w.x + u.y * w.y;
        Some(cal.angle(cross.abs().atan2(dot).to_degrees()))
    }
}

/// The closed-ellipse test of the coverage rules.
pub(crate) fn ellipse_contains(cx: f64, cy: f64, rx: f64, ry: f64, x: f64, y: f64) -> bool {
    rx != 0.0 && ry != 0.0 && ((x - cx) / rx).powi(2) + ((y - cy) / ry).powi(2) <= 1.0
}

/// The x of every edge of the closed ring `v` crossing the horizontal line `y` (an edge
/// crosses when exactly one end has `y' <= y`), unsorted.
pub(crate) fn crossings(v: &[P], y: f64) -> impl Iterator<Item = f64> + '_ {
    let n = v.len();
    (0..n).filter_map(move |i| {
        let (a, b) = (v[i], v[(i + 1) % n]);
        ((a.y <= y) != (b.y <= y)).then(|| a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y))
    })
}

/// Signed shoelace area of a closed ring.
pub(crate) fn shoelace(v: &[P]) -> f64 {
    let n = v.len();
    if n < 3 {
        return 0.0;
    }
    // Relative to the first vertex, which keeps precision far from the origin.
    let o = v[0];
    let mut s = 0.0;
    for i in 1..n - 1 {
        let (a, b) = (v[i] - o, v[i + 1] - o);
        s += a.x * b.y - b.x * a.y;
    }
    s / 2.0
}

fn path_length(cal: &Calibration, v: &[P], closed: bool) -> f64 {
    let open: f64 = v.windows(2).map(|w| cal.distance(w[0], w[1])).sum();
    match (closed, v.first(), v.last()) {
        (true, Some(&a), Some(&b)) if v.len() > 2 => open + cal.distance(b, a),
        _ => open,
    }
}

/// The semi-axes of the image of the ellipse with semi-axes `rx`, `ry` under `cal`: the
/// singular values of `m · diag(rx, ry)`.
fn mapped_ellipse_axes(cal: &Calibration, rx: f64, ry: f64) -> (f64, f64) {
    let [[a, b], [c, d]] = cal.m;
    let (a, b, c, d) = (a * rx, b * ry, c * rx, d * ry);
    let s1 = (a * a + b * b + c * c + d * d) / 2.0;
    let det = (a * d - b * c).abs();
    let disc = (s1 * s1 - det * det).max(0.0).sqrt();
    let big = (s1 + disc).sqrt();
    let small = if big > 0.0 { det / big } else { 0.0 };
    (big, small)
}
