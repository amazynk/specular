//! Slide annotations with class labels, in level-0 pixel coordinates (see the crate docs).

/// A point (x, y), level-0 pixels.
pub type Pt = (f64, f64);

/// A class label: QuPath's classification, COCO's category.
#[derive(Clone, Debug, PartialEq)]
pub struct Class {
    pub name: String,
    /// sRGB, when known.
    pub colour: Option<[u8; 3]>,
}

/// One polygon: its outer ring and holes (each ring open: the last point joins the first).
#[derive(Clone, Debug, PartialEq)]
pub struct Poly {
    pub exterior: Vec<Pt>,
    pub holes: Vec<Vec<Pt>>,
}

impl Poly {
    pub fn new(exterior: Vec<Pt>) -> Poly {
        Poly {
            exterior,
            holes: Vec::new(),
        }
    }

    /// The outer ring's area less the holes' (shoelace).
    pub fn area(&self) -> f64 {
        ring_area(&self.exterior) - self.holes.iter().map(|h| ring_area(h)).sum::<f64>()
    }
}

/// The area enclosed by `ring` (absolute; either winding).
pub fn ring_area(ring: &[Pt]) -> f64 {
    let n = ring.len();
    if n < 3 {
        return 0.0;
    }
    let mut s = 0.0;
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        s += a.0 * b.1 - b.0 * a.1;
    }
    (s / 2.0).abs()
}

/// `ring` without a repeated closing point (GeoJSON rings repeat the first point).
pub fn open_ring(mut ring: Vec<Pt>) -> Vec<Pt> {
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    ring
}

/// An annotation's shape.
#[derive(Clone, Debug, PartialEq)]
pub enum Geometry {
    /// One or more polygons (a multi-part region is one annotation).
    Area(Vec<Poly>),
    /// An axis-aligned rectangle, `x0 <= x1`, `y0 <= y1`.
    Rect {
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
    },
    Point(Pt),
    Points(Vec<Pt>),
    /// An open line of two or more points.
    Line(Vec<Pt>),
}

impl Geometry {
    /// A rectangle from two opposite corners.
    pub fn rect(a: Pt, b: Pt) -> Geometry {
        Geometry::Rect {
            x0: a.0.min(b.0),
            y0: a.1.min(b.1),
            x1: a.0.max(b.0),
            y1: a.1.max(b.1),
        }
    }

    /// One simple polygon.
    pub fn polygon(exterior: Vec<Pt>) -> Geometry {
        Geometry::Area(vec![Poly::new(exterior)])
    }

    /// A stable lower-case name.
    pub fn key(&self) -> &'static str {
        match self {
            Geometry::Area(_) => "polygon",
            Geometry::Rect { .. } => "rectangle",
            Geometry::Point(_) => "point",
            Geometry::Points(_) => "points",
            Geometry::Line(_) => "line",
        }
    }

    /// Covers pixels (polygons and rectangles).
    pub fn is_area(&self) -> bool {
        matches!(self, Geometry::Area(_) | Geometry::Rect { .. })
    }

    /// The polygons of an area (a rectangle as its four corners, clockwise in image
    /// coordinates from the top-left); empty for points and lines.
    pub fn polys(&self) -> Vec<Poly> {
        match self {
            Geometry::Area(p) => p.clone(),
            &Geometry::Rect { x0, y0, x1, y1 } => {
                vec![Poly::new(vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)])]
            }
            _ => Vec::new(),
        }
    }

    /// Every point (rings included).
    pub fn points(&self) -> Vec<Pt> {
        match self {
            Geometry::Area(ps) => ps
                .iter()
                .flat_map(|p| p.exterior.iter().chain(p.holes.iter().flatten()).copied())
                .collect(),
            &Geometry::Rect { x0, y0, x1, y1 } => vec![(x0, y0), (x1, y1)],
            Geometry::Point(p) => vec![*p],
            Geometry::Points(v) | Geometry::Line(v) => v.clone(),
        }
    }

    /// The bounding box (x0, y0, x1, y1); infinite when empty.
    pub fn bounds(&self) -> (f64, f64, f64, f64) {
        self.points().iter().fold(
            (f64::INFINITY, f64::INFINITY, -f64::INFINITY, -f64::INFINITY),
            |b, &(x, y)| (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y)),
        )
    }

    /// The enclosed area, px² (0 for points and lines).
    pub fn area(&self) -> f64 {
        self.polys().iter().map(Poly::area).sum()
    }

    /// Every coordinate is finite and the shape has its points.
    pub fn is_valid(&self) -> bool {
        let pts = self.points();
        let finite = pts.iter().all(|p| p.0.is_finite() && p.1.is_finite());
        finite
            && match self {
                Geometry::Area(ps) => !ps.is_empty() && ps.iter().all(|p| p.exterior.len() >= 3),
                Geometry::Rect { .. } | Geometry::Point(_) => true,
                Geometry::Points(v) => !v.is_empty(),
                Geometry::Line(v) => v.len() >= 2,
            }
    }

    /// `self` scaled by `s` about the origin (level-0 to another level's pixels and back).
    pub fn scaled(&self, s: f64) -> Geometry {
        let f = |p: &Pt| (p.0 * s, p.1 * s);
        match self {
            Geometry::Area(ps) => Geometry::Area(
                ps.iter()
                    .map(|p| Poly {
                        exterior: p.exterior.iter().map(f).collect(),
                        holes: p.holes.iter().map(|h| h.iter().map(f).collect()).collect(),
                    })
                    .collect(),
            ),
            &Geometry::Rect { x0, y0, x1, y1 } => Geometry::Rect {
                x0: x0 * s,
                y0: y0 * s,
                x1: x1 * s,
                y1: y1 * s,
            },
            Geometry::Point(p) => Geometry::Point(f(p)),
            Geometry::Points(v) => Geometry::Points(v.iter().map(f).collect()),
            Geometry::Line(v) => Geometry::Line(v.iter().map(f).collect()),
        }
    }
}

/// One annotation.
#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    /// Its own name (may be empty).
    pub name: String,
    /// Its class (`None`: unclassified).
    pub class: Option<Class>,
    pub geometry: Geometry,
}

impl Annotation {
    pub fn new(geometry: Geometry) -> Annotation {
        Annotation {
            name: String::new(),
            class: None,
            geometry,
        }
    }

    pub fn class_name(&self) -> Option<&str> {
        self.class
            .as_ref()
            .map(|c| c.name.as_str())
            .filter(|n| !n.is_empty())
    }
}

/// The name exports give annotations without a class.
pub const UNCLASSIFIED: &str = "Unclassified";

/// The classes of `items` in first-seen order, unclassified ones under [`UNCLASSIFIED`]
/// (placed where first seen). A class's colour is the first one given for its name.
pub fn classes(items: &[Annotation]) -> Vec<Class> {
    let mut out: Vec<Class> = Vec::new();
    for a in items {
        let name = a.class_name().unwrap_or(UNCLASSIFIED);
        match out.iter_mut().find(|c| c.name == name) {
            Some(c) => {
                if c.colour.is_none() {
                    c.colour = a.class.as_ref().and_then(|c| c.colour);
                }
            }
            None => out.push(Class {
                name: name.to_owned(),
                colour: a.class.as_ref().and_then(|c| c.colour),
            }),
        }
    }
    out
}

/// The index (0-based) of `a`'s class in `classes`.
pub fn class_index(classes: &[Class], a: &Annotation) -> usize {
    let name = a.class_name().unwrap_or(UNCLASSIFIED);
    classes.iter().position(|c| c.name == name).unwrap_or(0)
}

/// Distinct colours for `n` classes without one: golden-angle hues at fixed lightness
/// (the same generator the label overlays use).
pub fn palette(i: usize) -> [u8; 3] {
    let h = (i as f64 * 137.507_764) % 360.0;
    hsv(h, 0.65, 0.95)
}

/// HSV (h in degrees, s and v in 0..=1) to sRGB.
pub fn hsv(h: f64, s: f64, v: f64) -> [u8; 3] {
    let c = v * s;
    let hp = (h.rem_euclid(360.0)) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [r, g, b].map(|t| ((t + m) * 255.0).round().clamp(0.0, 255.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn areas_bounds_and_classes() {
        let mut p = Poly::new(vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
        p.holes
            .push(vec![(2.0, 2.0), (4.0, 2.0), (4.0, 4.0), (2.0, 4.0)]);
        let g = Geometry::Area(vec![p]);
        assert_eq!(g.area(), 96.0);
        assert_eq!(g.bounds(), (0.0, 0.0, 10.0, 10.0));
        let r = Geometry::rect((5.0, 7.0), (1.0, 2.0));
        assert_eq!(r.area(), 20.0);
        assert_eq!(r.scaled(0.5).bounds(), (0.5, 1.0, 2.5, 3.5));
        assert!(!Geometry::Line(vec![(0.0, 0.0)]).is_valid());
        let mk = |c: Option<&str>| Annotation {
            class: c.map(|n| Class {
                name: n.into(),
                colour: Some([1, 2, 3]),
            }),
            ..Annotation::new(Geometry::Point((0.0, 0.0)))
        };
        let items = [
            mk(Some("Tumor")),
            mk(None),
            mk(Some("Stroma")),
            mk(Some("Tumor")),
        ];
        let cs = classes(&items);
        let names: Vec<&str> = cs.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Tumor", UNCLASSIFIED, "Stroma"]);
        assert_eq!(class_index(&cs, &items[2]), 2);
        assert_eq!(open_ring(vec![(0.0, 0.0), (1.0, 0.0), (0.0, 0.0)]).len(), 2);
        assert_eq!(hsv(0.0, 1.0, 1.0), [255, 0, 0]);
        assert_ne!(palette(0), palette(1));
    }
}
