//! Slide annotations from the sidecar files viewers write beside a slide: Hamamatsu NDP.view
//! `.ndpa`, Aperio ImageScope `.xml` and ASAP `.xml` (the CAMELYON challenges' format).
//!
//! Every shape comes out in the slide's full-resolution pixels. NDPA stores nanometres from
//! the centre of the slide glass: the slide's own offset from that centre (NDPI tags
//! 65422/65423) and its pixel size place them ([`NdpaFrame`]). Aperio and ASAP store pixels.
//!
//! - NDPA: `ndpviewstate` elements (title, `annotation` of type `freehand` (closed or open,
//!   `specialtype="rectangle"`), `circle`, `pin`, `linearmeasure`), colour `#RRGGBB`.
//! - Aperio: `Annotation` layers (`LineColor` a Windows COLORREF, `Name`) of `Region`s
//!   (`Type` 0 freehand, 1 rectangle, 2 ellipse, 3 arrow, 4 ruler; `Text`) with `Vertex`
//!   points.
//! - ASAP: `Annotation` (`Type` Polygon, Rectangle, Spline, Dot, PointSet, Measurement;
//!   `Color`, `PartOfGroup`) with ordered `Coordinate`s; groups give a colour when the
//!   annotation has none.

use crate::xml::{self, Element};
use crate::{Error, Result};

/// Most shapes kept from one file.
pub const MAX_SHAPES: usize = 200_000;
/// Most points kept from one file (all shapes together).
pub const MAX_POINTS: usize = 8 << 20;
/// Largest sidecar read.
pub const MAX_BYTES: usize = 256 << 20;

/// What a shape is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shape {
    /// A closed outline.
    Polygon,
    /// An open line.
    Polyline,
    /// An axis-aligned rectangle: its outline's corners.
    Rectangle,
    /// An axis-aligned ellipse: two opposite corners of its bounding box.
    Ellipse,
    /// One marker.
    Point,
    /// Several markers.
    Points,
    /// A measured line from the first point to the second.
    Ruler,
    /// An arrow from the first point (its tail) to the second (its head).
    Arrow,
}

impl Shape {
    /// The outline is closed.
    pub fn closed(self) -> bool {
        matches!(self, Shape::Polygon | Shape::Rectangle | Shape::Ellipse)
    }

    /// A stable lower-case name (oracles, the info panel's fallback).
    pub fn id(self) -> &'static str {
        match self {
            Shape::Polygon => "polygon",
            Shape::Polyline => "polyline",
            Shape::Rectangle => "rectangle",
            Shape::Ellipse => "ellipse",
            Shape::Point => "point",
            Shape::Points => "points",
            Shape::Ruler => "ruler",
            Shape::Arrow => "arrow",
        }
    }
}

/// The file an annotation set came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    Ndpa,
    Aperio,
    Asap,
    /// The atoms of a Gaussian cube file, as points over its grid (not a sidecar).
    CubeAtoms,
}

impl Format {
    pub fn name(self) -> &'static str {
        match self {
            Format::Ndpa => "NDP.view (NDPA)",
            Format::Aperio => "Aperio ImageScope",
            Format::Asap => "ASAP",
            Format::CubeAtoms => "Gaussian cube atoms",
        }
    }
}

/// One shape.
#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    /// Its own name or text (may be empty).
    pub name: String,
    /// The layer or group it belongs to (may be empty).
    pub group: String,
    pub shape: Shape,
    /// Its colour as the file gives it.
    pub colour: Option<[u8; 3]>,
    /// In full-resolution slide pixels (see [`Shape`] for what they mean).
    pub points: Vec<(f64, f64)>,
}

impl Annotation {
    /// The outline to draw, in slide pixels: an ellipse as a polygon of `segments` points,
    /// everything else as stored.
    pub fn outline(&self, segments: usize) -> Vec<(f64, f64)> {
        if self.shape != Shape::Ellipse {
            return self.points.clone();
        }
        let (x0, y0, x1, y1) = self.bounds();
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let (rx, ry) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
        let n = segments.max(8);
        (0..n)
            .map(|i| {
                let t = std::f64::consts::TAU * i as f64 / n as f64;
                (cx + rx * t.cos(), cy + ry * t.sin())
            })
            .collect()
    }

    /// The bounding box (x0, y0, x1, y1).
    pub fn bounds(&self) -> (f64, f64, f64, f64) {
        self.points.iter().fold(
            (f64::INFINITY, f64::INFINITY, -f64::INFINITY, -f64::INFINITY),
            |b, &(x, y)| (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y)),
        )
    }

    /// The enclosed area in square pixels (closed shapes; 0 otherwise).
    pub fn area(&self) -> f64 {
        match self.shape {
            Shape::Ellipse => {
                let (x0, y0, x1, y1) = self.bounds();
                std::f64::consts::PI * (x1 - x0) * (y1 - y0) / 4.0
            }
            s if s.closed() && self.points.len() >= 3 => {
                let p = &self.points;
                let mut a = 0.0;
                for i in 0..p.len() {
                    let (x0, y0) = p[i];
                    let (x1, y1) = p[(i + 1) % p.len()];
                    a += x0 * y1 - x1 * y0;
                }
                (a / 2.0).abs()
            }
            _ => 0.0,
        }
    }

    /// The length of a ruler or line in pixels (x and y scaled by `sx`, `sy`).
    pub fn length(&self, sx: f64, sy: f64) -> f64 {
        self.points
            .windows(2)
            .map(|w| ((w[1].0 - w[0].0) * sx).hypot((w[1].1 - w[0].1) * sy))
            .sum()
    }
}

/// A file's annotations.
#[derive(Clone, Debug, PartialEq)]
pub struct Annotations {
    pub format: Format,
    pub items: Vec<Annotation>,
    /// Shapes or points were left out (over [`MAX_SHAPES`] / [`MAX_POINTS`], or unreadable).
    pub skipped: usize,
}

/// How NDPA nanometres map to a slide's pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NdpaFrame {
    /// Nanometres per full-resolution pixel (x, y).
    pub nm_per_px: (f64, f64),
    /// The image centre's offset from the slide centre, nanometres (NDPI 65422/65423).
    pub offset_nm: (f64, f64),
    /// Full-resolution size.
    pub width: u32,
    pub height: u32,
}

impl NdpaFrame {
    fn px(&self, x: f64, y: f64) -> (f64, f64) {
        (
            (x - self.offset_nm.0) / self.nm_per_px.0 + f64::from(self.width) / 2.0,
            (y - self.offset_nm.1) / self.nm_per_px.1 + f64::from(self.height) / 2.0,
        )
    }
}

/// Text of a sidecar: UTF-8 (with or without a BOM) or UTF-16 with a BOM.
pub fn text(data: &[u8]) -> Option<String> {
    if data.len() > MAX_BYTES {
        return None;
    }
    let utf16 = |le: bool| {
        let units: Vec<u16> = data[2..]
            .chunks_exact(2)
            .map(|c| {
                if le {
                    u16::from_le_bytes([c[0], c[1]])
                } else {
                    u16::from_be_bytes([c[0], c[1]])
                }
            })
            .collect();
        String::from_utf16(&units).ok()
    };
    match data {
        [0xEF, 0xBB, 0xBF, rest @ ..] => std::str::from_utf8(rest).ok().map(str::to_owned),
        [0xFF, 0xFE, ..] => utf16(true),
        [0xFE, 0xFF, ..] => utf16(false),
        _ => std::str::from_utf8(data).ok().map(str::to_owned),
    }
}

/// Which annotation format a document's root element is, if any.
pub fn sniff(root: &Element) -> Option<Format> {
    match root.name.as_str() {
        "annotations" if root.find("ndpviewstate").is_some() || root.children.is_empty() => {
            Some(Format::Ndpa)
        }
        "ASAP_Annotations" => Some(Format::Asap),
        "Annotations" if root.all("Annotation").next().is_some() || root.children.is_empty() => {
            Some(Format::Aperio)
        }
        _ => None,
    }
}

/// Parses a sidecar. NDPA needs `frame` (without it, an NDPA file is refused).
pub fn parse(data: &[u8], frame: Option<&NdpaFrame>) -> Result<Annotations> {
    let src = text(data).ok_or_else(|| Error::Malformed("annotations: not text".into()))?;
    let root = xml::parse(&src)?;
    let format = sniff(&root).ok_or(Error::NotSlide)?;
    let mut out = Out {
        items: Vec::new(),
        points: 0,
        skipped: 0,
    };
    match format {
        Format::Ndpa => {
            let f = frame.ok_or_else(|| {
                Error::Unsupported("NDPA annotations without the slide's position".into())
            })?;
            ndpa(&root, f, &mut out);
        }
        Format::Aperio => aperio(&root, &mut out),
        Format::Asap => asap(&root, &mut out),
        // Never sniffed from XML: made by the cube reader.
        Format::CubeAtoms => {}
    }
    Ok(Annotations {
        format,
        items: out.items,
        skipped: out.skipped,
    })
}

struct Out {
    items: Vec<Annotation>,
    points: usize,
    skipped: usize,
}

impl Out {
    fn push(&mut self, a: Annotation) {
        let ok = a.points.iter().all(|p| p.0.is_finite() && p.1.is_finite())
            && match a.shape {
                Shape::Polygon => a.points.len() >= 3,
                Shape::Polyline | Shape::Ruler | Shape::Arrow | Shape::Ellipse => {
                    a.points.len() >= 2
                }
                Shape::Rectangle => a.points.len() >= 2,
                Shape::Point | Shape::Points => !a.points.is_empty(),
            };
        if !ok || self.items.len() >= MAX_SHAPES || self.points + a.points.len() > MAX_POINTS {
            self.skipped += 1;
            return;
        }
        self.points += a.points.len();
        self.items.push(a);
    }
}

/// A number written with a point or (some locales' ASAP files) a decimal comma.
fn num(s: &str) -> Option<f64> {
    let s = s.trim();
    s.parse::<f64>()
        .ok()
        .or_else(|| s.replace(',', ".").parse().ok())
        .filter(|v| v.is_finite())
}

fn attr_num(e: &Element, name: &str) -> Option<f64> {
    e.attr(name).and_then(num)
}

fn child_num(e: &Element, name: &str) -> Option<f64> {
    e.child(name).and_then(|c| num(&c.text))
}

/// `#RRGGBB` (or `RRGGBB`).
fn hex_colour(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 || !h.is_ascii() {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// The four corners of the box through two opposite points.
fn corners(a: (f64, f64), b: (f64, f64)) -> Vec<(f64, f64)> {
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
}

fn ndpa(root: &Element, f: &NdpaFrame, out: &mut Out) {
    for vs in root.all("ndpviewstate") {
        let name = vs.child("title").map_or(String::new(), |t| t.text.clone());
        let Some(a) = vs.child("annotation") else {
            out.skipped += 1;
            continue;
        };
        let colour = a.attr("color").and_then(hex_colour);
        let at = |x: &str, y: &str| -> Option<(f64, f64)> {
            Some(f.px(child_num(a, x)?, child_num(a, y)?))
        };
        let list = || -> Vec<(f64, f64)> {
            a.child("pointlist")
                .into_iter()
                .flat_map(|l| l.all("point"))
                .filter_map(|p| Some(f.px(child_num(p, "x")?, child_num(p, "y")?)))
                .collect()
        };
        let typ = a.attr("type").unwrap_or("");
        let shape_points = match typ {
            "freehand" => {
                let pts = list();
                let closed = a.child("closed").is_none_or(|c| c.text.trim() != "0");
                let rect = a.attr("specialtype") == Some("rectangle")
                    || a.attr("displayname") == Some("AnnotateRectangle");
                Some(if rect && pts.len() >= 2 {
                    let b = bounds(&pts);
                    (Shape::Rectangle, corners((b.0, b.1), (b.2, b.3)))
                } else if closed {
                    (Shape::Polygon, pts)
                } else {
                    (Shape::Polyline, pts)
                })
            }
            "circle" => {
                let c = (child_num(a, "x"), child_num(a, "y"), child_num(a, "radius"));
                match c {
                    (Some(x), Some(y), Some(r)) => {
                        let p0 = f.px(x - r, y - r);
                        let p1 = f.px(x + r, y + r);
                        Some((Shape::Ellipse, vec![p0, p1]))
                    }
                    _ => None,
                }
            }
            "pin" | "pointer" => at("x", "y").map(|p| (Shape::Point, vec![p])),
            "linearmeasure" => match (at("x1", "y1"), at("x2", "y2")) {
                (Some(p), Some(q)) => Some((Shape::Ruler, vec![p, q])),
                _ => None,
            },
            _ => None,
        };
        match shape_points {
            Some((shape, points)) => out.push(Annotation {
                name,
                group: String::new(),
                shape,
                colour,
                points,
            }),
            None => out.skipped += 1,
        }
    }
}

fn bounds(p: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    p.iter().fold(
        (f64::INFINITY, f64::INFINITY, -f64::INFINITY, -f64::INFINITY),
        |b, &(x, y)| (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y)),
    )
}

/// Aperio's LineColor: a Windows COLORREF (0x00BBGGRR) in decimal.
fn colorref(s: &str) -> Option<[u8; 3]> {
    let v: u32 = s.trim().parse().ok()?;
    Some([v as u8, (v >> 8) as u8, (v >> 16) as u8])
}

fn aperio(root: &Element, out: &mut Out) {
    for (li, layer) in root.all("Annotation").enumerate() {
        let colour = layer.attr("LineColor").and_then(colorref);
        let group = layer
            .attr("Name")
            .filter(|n| !n.trim().is_empty())
            .map(str::to_owned)
            .or_else(|| {
                // ImageScope keeps a layer's name as an Attribute too.
                layer
                    .child("Attributes")
                    .and_then(|a| a.child("Attribute"))
                    .and_then(|a| a.attr("Name"))
                    .filter(|n| !n.trim().is_empty())
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| {
                layer
                    .attr("Id")
                    .map_or_else(|| (li + 1).to_string(), str::to_owned)
            });
        for region in layer
            .child("Regions")
            .into_iter()
            .flat_map(|r| r.all("Region"))
        {
            let pts: Vec<(f64, f64)> = region
                .child("Vertices")
                .into_iter()
                .flat_map(|v| v.all("Vertex"))
                .filter_map(|v| Some((attr_num(v, "X")?, attr_num(v, "Y")?)))
                .collect();
            let typ = region.attr("Type").and_then(num).unwrap_or(0.0) as i64;
            let (shape, points) = match typ {
                1 if pts.len() >= 2 => {
                    let b = bounds(&pts);
                    (Shape::Rectangle, corners((b.0, b.1), (b.2, b.3)))
                }
                2 if pts.len() >= 2 => {
                    let b = bounds(&pts);
                    (Shape::Ellipse, vec![(b.0, b.1), (b.2, b.3)])
                }
                3 => (Shape::Arrow, pts),
                4 => (Shape::Ruler, pts),
                _ if pts.len() == 1 => (Shape::Point, pts),
                _ => (Shape::Polygon, pts),
            };
            out.push(Annotation {
                name: region.attr("Text").unwrap_or("").trim().to_owned(),
                group: group.clone(),
                shape,
                colour,
                points,
            });
        }
    }
}

fn asap(root: &Element, out: &mut Out) {
    let groups: Vec<(&str, Option<[u8; 3]>)> = root
        .child("AnnotationGroups")
        .into_iter()
        .flat_map(|g| g.all("Group"))
        .map(|g| {
            (
                g.attr("Name").unwrap_or(""),
                g.attr("Color").and_then(hex_colour),
            )
        })
        .collect();
    for a in root
        .child("Annotations")
        .into_iter()
        .flat_map(|a| a.all("Annotation"))
    {
        let group = a.attr("PartOfGroup").unwrap_or("").trim();
        let group = if group == "None" { "" } else { group };
        let colour = a
            .attr("Color")
            .and_then(hex_colour)
            .or_else(|| groups.iter().find(|g| g.0 == group).and_then(|g| g.1));
        let mut coords: Vec<(f64, f64, f64)> = a
            .child("Coordinates")
            .into_iter()
            .flat_map(|c| c.all("Coordinate"))
            .enumerate()
            .filter_map(|(i, c)| {
                Some((
                    attr_num(c, "Order").unwrap_or(i as f64),
                    attr_num(c, "X")?,
                    attr_num(c, "Y")?,
                ))
            })
            .collect();
        coords.sort_by(|p, q| p.0.total_cmp(&q.0));
        let pts: Vec<(f64, f64)> = coords.into_iter().map(|c| (c.1, c.2)).collect();
        let shape = match a.attr("Type").unwrap_or("") {
            "Rectangle" => Shape::Rectangle,
            "Dot" => Shape::Point,
            "PointSet" => Shape::Points,
            "Measurement" => Shape::Ruler,
            _ => Shape::Polygon,
        };
        let points = if shape == Shape::Rectangle && pts.len() == 2 {
            corners(pts[0], pts[1])
        } else {
            pts
        };
        out.push(Annotation {
            name: a.attr("Name").unwrap_or("").trim().to_owned(),
            group: group.to_owned(),
            shape,
            colour,
            points,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: NdpaFrame = NdpaFrame {
        nm_per_px: (500.0, 250.0),
        offset_nm: (1000.0, -2000.0),
        width: 400,
        height: 200,
    };

    #[test]
    fn ndpa_places_nanometres_on_pixels() {
        let x = br##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<annotations>
 <ndpviewstate id="1"><title>Tumour</title><coordformat>nanometers</coordformat>
  <annotation type="freehand" displayname="AnnotateFreehand" color="#ff0000">
   <closed>1</closed><pointlist>
    <point><x>1000</x><y>-2000</y></point><point><x>6000</x><y>-2000</y></point>
    <point><x>6000</x><y>500</y></point></pointlist></annotation></ndpviewstate>
 <ndpviewstate id="2"><title>Spot</title>
  <annotation type="circle" color="#00ff00"><x>1000</x><y>-2000</y><radius>1000</radius></annotation></ndpviewstate>
 <ndpviewstate id="3"><title>Pin</title>
  <annotation type="pin" color="#0000ff"><x>1500</x><y>-1750</y></annotation></ndpviewstate>
 <ndpviewstate id="4"><title>Ruler</title>
  <annotation type="linearmeasure" color="#ffffff"><x1>1000</x1><y1>-2000</y1><x2>4000</x2><y2>-2000</y2></annotation></ndpviewstate>
 <ndpviewstate id="5"><title>Box</title>
  <annotation type="freehand" specialtype="rectangle" color="#ffff00"><closed>1</closed><pointlist>
   <point><x>1000</x><y>-2000</y></point><point><x>2000</x><y>-2000</y></point>
   <point><x>2000</x><y>-1000</y></point><point><x>1000</x><y>-1000</y></point></pointlist></annotation></ndpviewstate>
</annotations>"##;
        let a = parse(x, Some(&FRAME)).unwrap();
        assert_eq!(a.format, Format::Ndpa);
        assert_eq!(a.items.len(), 5);
        let t = &a.items[0];
        assert_eq!((t.name.as_str(), t.shape), ("Tumour", Shape::Polygon));
        assert_eq!(t.colour, Some([255, 0, 0]));
        assert_eq!(
            t.points,
            vec![(200.0, 100.0), (210.0, 100.0), (210.0, 110.0)]
        );
        assert_eq!(t.area(), 50.0);
        assert_eq!(a.items[1].shape, Shape::Ellipse);
        assert_eq!(a.items[1].points, vec![(198.0, 96.0), (202.0, 104.0)]);
        assert_eq!(a.items[2].points, vec![(201.0, 101.0)]);
        assert_eq!(a.items[3].shape, Shape::Ruler);
        assert_eq!(a.items[3].length(1.0, 1.0), 6.0);
        assert_eq!(a.items[4].shape, Shape::Rectangle);
        assert_eq!(a.items[4].area(), 2.0 * 4.0);
        // Without the slide's position NDPA cannot be placed.
        assert!(parse(x, None).is_err());
    }

    #[test]
    fn aperio_layers_and_regions() {
        let x = br#"<Annotations MicronsPerPixel="0.25">
<Annotation Id="1" Name="" LineColor="65280"><Attributes><Attribute Name="Stroma" Id="0" Value=""/></Attributes>
<Regions><RegionAttributeHeaders/>
<Region Id="1" Type="0" Text="a"><Vertices><Vertex X="0" Y="0" Z="0"/><Vertex X="10" Y="0"/><Vertex X="10" Y="10"/><Vertex X="0" Y="10"/></Vertices></Region>
<Region Id="2" Type="1" Text="box"><Vertices><Vertex X="5" Y="5"/><Vertex X="15" Y="5"/><Vertex X="15" Y="25"/><Vertex X="5" Y="25"/></Vertices></Region>
<Region Id="3" Type="2" Text="oval"><Vertices><Vertex X="0" Y="0"/><Vertex X="20" Y="10"/></Vertices></Region>
<Region Id="4" Type="4" Text="r"><Vertices><Vertex X="0" Y="0"/><Vertex X="3" Y="4"/></Vertices></Region>
</Regions></Annotation>
<Annotation Id="2" Name="Tumor" LineColor="255"><Regions><Region Id="5" Type="0"><Vertices><Vertex X="1.5" Y="2.5"/></Vertices></Region></Regions></Annotation>
</Annotations>"#;
        let a = parse(x, None).unwrap();
        assert_eq!(a.format, Format::Aperio);
        assert_eq!(a.items.len(), 5);
        assert_eq!(a.items[0].group, "Stroma");
        assert_eq!(a.items[0].colour, Some([0, 255, 0]));
        assert_eq!(a.items[0].area(), 100.0);
        assert_eq!(a.items[1].shape, Shape::Rectangle);
        assert_eq!(a.items[1].area(), 200.0);
        assert_eq!(a.items[2].shape, Shape::Ellipse);
        assert!((a.items[2].area() - std::f64::consts::PI * 50.0).abs() < 1e-9);
        assert_eq!(a.items[3].length(1.0, 1.0), 5.0);
        assert_eq!(a.items[4].shape, Shape::Point);
        assert_eq!(a.items[4].colour, Some([255, 0, 0]));
        assert_eq!(a.items[4].group, "Tumor");
    }

    #[test]
    fn asap_orders_coordinates_and_inherits_group_colours() {
        let x = br##"<?xml version="1.0"?>
<ASAP_Annotations><Annotations>
<Annotation Name="Annotation 0" Type="Polygon" PartOfGroup="metastases" Color="#F4FA58">
<Coordinates><Coordinate Order="1" X="10" Y="0"/><Coordinate Order="0" X="0" Y="0"/><Coordinate Order="2" X="10,5" Y="10"/></Coordinates></Annotation>
<Annotation Name="d" Type="Dot" PartOfGroup="None"><Coordinates><Coordinate Order="0" X="3" Y="4"/></Coordinates></Annotation>
<Annotation Name="r" Type="Rectangle" PartOfGroup="g"><Coordinates><Coordinate Order="0" X="0" Y="0"/><Coordinate Order="1" X="4" Y="0"/><Coordinate Order="2" X="4" Y="2"/><Coordinate Order="3" X="0" Y="2"/></Coordinates></Annotation>
<Annotation Name="bad" Type="Polygon"><Coordinates><Coordinate Order="0" X="nan" Y="0"/></Coordinates></Annotation>
</Annotations><AnnotationGroups><Group Name="g" PartOfGroup="None" Color="#64FE2E"><Attributes/></Group></AnnotationGroups></ASAP_Annotations>"##;
        let a = parse(x, None).unwrap();
        assert_eq!(a.format, Format::Asap);
        assert_eq!(a.items.len(), 3);
        assert_eq!(a.skipped, 1);
        assert_eq!(
            a.items[0].points,
            vec![(0.0, 0.0), (10.0, 0.0), (10.5, 10.0)]
        );
        assert_eq!(a.items[0].colour, Some([0xF4, 0xFA, 0x58]));
        assert_eq!(a.items[1].group, "");
        assert_eq!(a.items[2].colour, Some([0x64, 0xFE, 0x2E]));
        assert_eq!(a.items[2].area(), 8.0);
    }

    #[test]
    fn other_xml_is_not_annotations() {
        assert!(matches!(parse(b"<OME/>", None), Err(Error::NotSlide)));
        assert!(parse(b"\xff\xfe<\0a\0/\0>\0", None).is_err());
    }
}
