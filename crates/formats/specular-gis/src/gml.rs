//! GML simple features (GML 2, 3.1.1 and 3.2; WFS 1.x/2.0 responses): feature members of a
//! collection, one layer per feature type. A property holding a GML geometry gives the
//! feature its geometry (the first one; others are counted), plain properties become
//! attributes (nested ones flattened as `parent_child`), and `gml:id` is the `gml_id` field.
//!
//! Geometries: Point, LineString, LinearRing, Polygon, Envelope/Box, Curve (LineStringSegment,
//! Arc, ArcString, Circle, …), Surface/PolygonPatch, Triangle, Multi* (members or
//! members-plural), MultiCurve/MultiSurface, Composite*, Orientable*, MultiGeometry.
//! Coordinates come from `pos`, `posList` (with `srsDimension`), `coordinates` (with `cs`,
//! `ts`, `decimal`) and GML 2 `coord`. A srsName given as a URN or http URI follows the EPSG
//! axis order: latitude first for geographic CRSs and northing first where EPSG says so;
//! those are swapped to x = east.

use crate::crs::{self, Crs, CrsKind};
use crate::model::{Dataset, Geometry, Layer, MAX_FEATURES, MAX_VERTICES, Pt, Value};
use roxmltree::Node;

pub fn sniff(head: &[u8]) -> bool {
    let s = String::from_utf8_lossy(&head[..head.len().min(4096)]);
    (s.contains("opengis.net/gml") || s.contains("<gml:"))
        && !s.contains("<kml")
        && !s.contains("topografix.com/GPX")
}

fn local<'a>(n: &Node<'a, '_>) -> &'a str {
    n.tag_name().name()
}

fn is_gml(n: &Node<'_, '_>) -> bool {
    n.tag_name()
        .namespace()
        .is_some_and(|ns| ns.starts_with("http://www.opengis.net/gml"))
}

fn elements<'a, 'i>(n: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> {
    n.children().filter(Node::is_element)
}

fn child<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    elements(n).find(|c| local(c) == name)
}

const GEOMETRIES: &[&str] = &[
    "Point",
    "LineString",
    "LinearRing",
    "Polygon",
    "Envelope",
    "Box",
    "Curve",
    "Surface",
    "PolyhedralSurface",
    "TriangulatedSurface",
    "Tin",
    "Triangle",
    "MultiPoint",
    "MultiLineString",
    "MultiCurve",
    "MultiPolygon",
    "MultiSurface",
    "MultiGeometry",
    "CompositeCurve",
    "CompositeSurface",
    "OrientableCurve",
    "OrientableSurface",
    "Arc",
    "Circle",
    "ArcString",
    "Solid",
    "CompositeSolid",
    "MultiSolid",
];

fn is_geometry(n: &Node<'_, '_>) -> bool {
    (is_gml(n) && GEOMETRIES.contains(&local(n))) || elevated(n).is_some()
}

/// AIXM's ElevatedPoint/ElevatedCurve/ElevatedSurface: GML geometries with a height.
fn elevated<'a>(n: &Node<'a, '_>) -> Option<&'static str> {
    Some(match local(n) {
        "ElevatedPoint" => "Point",
        "ElevatedCurve" => "Curve",
        "ElevatedSurface" => "Surface",
        _ => return None,
    })
}

/// How a srsName maps to a CRS and whether its coordinates come northing/latitude first.
#[derive(Clone, Debug)]
struct Srs {
    crs: Option<Crs>,
    swap: bool,
    name: String,
}

/// `short_urn`: a plain "EPSG:n" follows the EPSG axis order too (FME writes it so).
fn srs(name: &str, short_urn: bool) -> Srs {
    let code = crs::epsg_from_ref(name);
    let c = code.and_then(crs::from_epsg);
    let u = name.to_ascii_lowercase();
    // URNs and http URIs (not the old ".../epsg.xml#code" form) follow the EPSG axis order.
    let authority_order = ((u.starts_with("urn:") || u.starts_with("http"))
        && !u.contains("epsg.xml#")
        && !u.contains("crs84"))
        || (short_urn && u.starts_with("epsg:"));
    let swap = authority_order
        && match (&c, code) {
            (Some(c), _) if c.kind == CrsKind::Geographic => true,
            (Some(c), Some(code)) if c.kind == CrsKind::Projected => {
                crate::epsg_axes::north_first(code)
            }
            _ => false,
        };
    Srs {
        crs: c,
        swap,
        name: name.to_string(),
    }
}

struct Ctx {
    /// The srsName in force (inherited from enclosing geometries or the collection).
    default_srs: Option<String>,
    first_srs: Option<String>,
    short_urn: bool,
    /// Coordinates per position when a geometry does not say (3 in CityGML).
    default_dim: usize,
    vertices: usize,
    warnings: Vec<String>,
}

impl Ctx {
    fn warn(&mut self, w: impl Into<String>) {
        let w = w.into();
        if !self.warnings.contains(&w) && self.warnings.len() < 16 {
            self.warnings.push(w);
        }
    }
}

fn numbers(s: &str) -> Vec<f64> {
    s.split_ascii_whitespace()
        .filter_map(|t| t.parse::<f64>().ok())
        .collect()
}

fn dims(n: Node<'_, '_>, inherited: usize) -> usize {
    n.attribute("srsDimension")
        .or_else(|| n.attribute("dimension"))
        .and_then(|d| d.trim().parse::<usize>().ok())
        .filter(|d| (1..=4).contains(d))
        .unwrap_or(inherited)
}

/// The coordinates directly under a geometry element (pos, posList, coordinates, coord,
/// pointProperty/Point children of curves).
fn points(n: Node<'_, '_>, dim: usize, swap: bool) -> Vec<Pt> {
    let mut out: Vec<Pt> = Vec::new();
    let put = |out: &mut Vec<Pt>, x: f64, y: f64| {
        out.push(if swap { [y, x] } else { [x, y] });
    };
    for c in elements(n) {
        match local(&c) {
            "posList" => {
                let d = dims(c, dim).max(2);
                let v = numbers(c.text().unwrap_or(""));
                for p in v.chunks_exact(d) {
                    put(&mut out, p[0], p[1]);
                }
            }
            "pos" => {
                let v = numbers(c.text().unwrap_or(""));
                if v.len() >= 2 {
                    put(&mut out, v[0], v[1]);
                }
            }
            "coordinates" => {
                // Undeclared separators: the first one in the text separates coordinates
                // (some writers swap the defaults, as in "0 0,0 1,1 1").
                let text0 = c.text().unwrap_or("").trim();
                let first_sep = text0.chars().find(|ch| *ch == ',' || ch.is_whitespace());
                let swapped = c.attribute("cs").is_none()
                    && c.attribute("ts").is_none()
                    && first_sep.is_some_and(char::is_whitespace)
                    && text0.contains(',');
                let cs = c.attribute("cs").unwrap_or(if swapped { " " } else { "," });
                let ts = c.attribute("ts").unwrap_or(if swapped { "," } else { " " });
                let dec = c.attribute("decimal").unwrap_or(".");
                let text = c.text().unwrap_or("");
                let tuples: Vec<&str> = if ts.trim().is_empty() {
                    text.split_ascii_whitespace().collect()
                } else {
                    text.split(ts)
                        .map(str::trim)
                        .filter(|t| !t.is_empty())
                        .collect()
                };
                for t in tuples {
                    let v: Vec<f64> = t
                        .split(cs)
                        .filter_map(|x| {
                            let x = x.trim();
                            if dec == "." {
                                x.parse().ok()
                            } else {
                                x.replace(dec, ".").parse().ok()
                            }
                        })
                        .collect();
                    if v.len() >= 2 {
                        put(&mut out, v[0], v[1]);
                    }
                }
            }
            "coord" => {
                let x = child(c, "X")
                    .and_then(|e| e.text())
                    .and_then(|t| t.trim().parse().ok());
                let y = child(c, "Y")
                    .and_then(|e| e.text())
                    .and_then(|t| t.trim().parse().ok());
                if let (Some(x), Some(y)) = (x, y) {
                    put(&mut out, x, y);
                }
            }
            "pointProperty" | "pointRep" => {
                if let Some(p) = child(c, "Point") {
                    out.extend(points(p, dims(p, dim), swap));
                }
            }
            _ => {}
        }
    }
    out
}

struct G<'c> {
    ctx: &'c mut Ctx,
}

impl G<'_> {
    fn swap_for(&mut self, n: Node<'_, '_>, inherited: Option<&Srs>) -> Option<Srs> {
        match n.attribute("srsName") {
            Some(s) => {
                if self.ctx.first_srs.is_none() {
                    self.ctx.first_srs = Some(s.to_string());
                }
                Some(srs(s, self.ctx.short_urn))
            }
            None => inherited.cloned().or_else(|| {
                let u = self.ctx.short_urn;
                self.ctx.default_srs.clone().map(|s| srs(&s, u))
            }),
        }
    }

    /// A curve or ring as one point list (segments joined).
    fn line(&mut self, n: Node<'_, '_>, s: Option<&Srs>, dim: usize, depth: usize) -> Vec<Pt> {
        let swap = s.is_some_and(|s| s.swap);
        match elevated(&n).unwrap_or(local(&n)) {
            "LineString" | "LinearRing" | "LineStringSegment" | "GeodesicString" | "Geodesic" => {
                points(n, dim, swap)
            }
            "Arc" | "ArcString" => {
                let p = points(n, dim, swap);
                crate::model::stroke_circular(&p)
            }
            "Circle" => {
                let p = points(n, dim, swap);
                match (p.len() >= 3)
                    .then(|| circumcentre(p[0], p[1], p[2]))
                    .flatten()
                {
                    Some(o) => {
                        let r = (p[0][0] - o[0]).hypot(p[0][1] - o[1]);
                        let a0 = (p[0][1] - o[1]).atan2(p[0][0] - o[0]);
                        (0..=90)
                            .map(|k| {
                                let a = a0 + std::f64::consts::TAU * f64::from(k) / 90.0;
                                [o[0] + r * a.cos(), o[1] + r * a.sin()]
                            })
                            .collect()
                    }
                    None => p,
                }
            }
            "CircleByCenterPoint" | "ArcByCenterPoint" => {
                let c = points(n, dim, swap);
                let r = child(n, "radius")
                    .and_then(|r| r.text())
                    .and_then(|t| t.trim().parse::<f64>().ok());
                let (Some(&c), Some(r)) = (c.first(), r) else {
                    return Vec::new();
                };
                let deg = |name| {
                    child(n, name)
                        .and_then(|e| e.text())
                        .and_then(|t| t.trim().parse::<f64>().ok())
                };
                let (a0, a1) = match local(&n) {
                    "CircleByCenterPoint" => (0.0, 360.0),
                    _ => (
                        deg("startAngle").unwrap_or(0.0),
                        deg("endAngle").unwrap_or(360.0),
                    ),
                };
                let steps = (((a1 - a0).abs() / 4.0).ceil() as usize).clamp(2, 3600);
                (0..=steps)
                    .map(|k| {
                        let a = (a0 + (a1 - a0) * k as f64 / steps as f64).to_radians();
                        // Angles are measured on the CRS's axes as given.
                        if swap {
                            [c[0] + r * a.sin(), c[1] + r * a.cos()]
                        } else {
                            [c[0] + r * a.cos(), c[1] + r * a.sin()]
                        }
                    })
                    .collect()
            }
            "Curve" | "CompositeCurve" | "OrientableCurve" | "Ring" | "segments" => {
                if depth > 16 {
                    return Vec::new();
                }
                let reverse =
                    local(&n) == "OrientableCurve" && n.attribute("orientation") == Some("-");
                let mut out: Vec<Pt> = Vec::new();
                for c in elements(n) {
                    let part = match local(&c) {
                        "segments" => self.line(c, s, dim, depth + 1),
                        "curveMember" | "baseCurve" | "baseCurveProperty" | "curveProperty" => {
                            match elements(c).next() {
                                Some(g) => {
                                    let s2 = self.swap_for(g, s);
                                    self.line(g, s2.as_ref(), dims(g, dim), depth + 1)
                                }
                                None => Vec::new(),
                            }
                        }
                        _ if is_gml(&c) => self.line(c, s, dims(c, dim), depth + 1),
                        _ => Vec::new(),
                    };
                    let skip = usize::from(!out.is_empty() && out.last() == part.first());
                    out.extend_from_slice(&part[skip.min(part.len())..]);
                }
                if reverse {
                    out.reverse();
                }
                out
            }
            _ => Vec::new(),
        }
    }

    fn rings(
        &mut self,
        n: Node<'_, '_>,
        s: Option<&Srs>,
        dim: usize,
        depth: usize,
    ) -> Vec<Vec<Pt>> {
        let mut out = Vec::new();
        for c in elements(n) {
            match local(&c) {
                "exterior" | "outerBoundaryIs" | "interior" | "innerBoundaryIs" => {
                    for r in elements(c) {
                        let s2 = self.swap_for(r, s);
                        out.push(self.line(r, s2.as_ref(), dims(r, dim), depth + 1));
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// Members of a multi or composite geometry.
    fn members(
        &mut self,
        n: Node<'_, '_>,
        s: Option<&Srs>,
        dim: usize,
        depth: usize,
    ) -> Vec<Geometry> {
        let mut out = Vec::new();
        for c in elements(n) {
            let name = local(&c);
            if name.ends_with("Members") || name == "patches" || name == "trianglePatches" {
                for g in elements(c) {
                    if let Some(g) = self.geometry(g, s, dim, depth + 1) {
                        out.push(g);
                    }
                }
            } else if (name.ends_with("Member")
                || name == "baseSurface"
                || name == "exterior" && local(&n).contains("Solid"))
                && let Some(g) = elements(c).next()
                && let Some(g) = self.geometry(g, s, dim, depth + 1)
            {
                out.push(g);
            }
        }
        out
    }

    fn geometry(
        &mut self,
        n: Node<'_, '_>,
        inherited: Option<&Srs>,
        dim: usize,
        depth: usize,
    ) -> Option<Geometry> {
        if depth > 32 {
            return None;
        }
        let s = self.swap_for(n, inherited);
        let s = s.as_ref();
        let dim = dims(n, dim);
        let swap = s.is_some_and(|s| s.swap);
        let g = match elevated(&n).unwrap_or(local(&n)) {
            "Point" => {
                let p = points(n, dim, swap);
                Geometry::Point(*p.first()?)
            }
            "LineString" | "Curve" | "CompositeCurve" | "OrientableCurve" | "Arc" | "Circle"
            | "ArcString" | "LinearRing" => Geometry::LineString(self.line(n, s, dim, depth)),
            "Polygon" | "PolygonPatch" | "Triangle" => {
                Geometry::Polygon(self.rings(n, s, dim, depth))
            }
            "Envelope" | "Box" => {
                let mut p = points(n, dim, swap);
                for c in elements(n) {
                    if matches!(local(&c), "lowerCorner" | "upperCorner") {
                        let v = numbers(c.text().unwrap_or(""));
                        if v.len() >= 2 {
                            p.push(if swap { [v[1], v[0]] } else { [v[0], v[1]] });
                        }
                    }
                }
                if p.len() < 2 {
                    return None;
                }
                let (a, b) = (p[0], p[1]);
                Geometry::Polygon(vec![vec![
                    [a[0], a[1]],
                    [b[0], a[1]],
                    [b[0], b[1]],
                    [a[0], b[1]],
                    [a[0], a[1]],
                ]])
            }
            "Surface"
            | "PolyhedralSurface"
            | "TriangulatedSurface"
            | "Tin"
            | "CompositeSurface"
            | "OrientableSurface"
            | "MultiSurface"
            | "MultiPolygon"
            | "Shell"
            | "Solid"
            | "CompositeSolid"
            | "MultiSolid" => {
                let mut polys = Vec::new();
                for g in self.members(n, s, dim, depth) {
                    match g {
                        Geometry::Polygon(p) => polys.push(p),
                        Geometry::MultiPolygon(m) => polys.extend(m),
                        _ => {}
                    }
                }
                if local(&n).contains("Solid") {
                    self.ctx.warn("solids are drawn as their faces");
                }
                Geometry::MultiPolygon(polys)
            }
            "MultiPoint" => Geometry::MultiPoint(
                self.members(n, s, dim, depth)
                    .into_iter()
                    .filter_map(|g| match g {
                        Geometry::Point(p) => Some(p),
                        _ => None,
                    })
                    .collect(),
            ),
            "MultiLineString" | "MultiCurve" => Geometry::MultiLineString(
                self.members(n, s, dim, depth)
                    .into_iter()
                    .flat_map(|g| match g {
                        Geometry::LineString(l) => vec![l],
                        Geometry::MultiLineString(m) => m,
                        _ => Vec::new(),
                    })
                    .collect(),
            ),
            "MultiGeometry" => Geometry::Collection(self.members(n, s, dim, depth)),
            _ => return None,
        };
        self.ctx.vertices += g.vertex_count();
        Some(g)
    }
}

fn circumcentre(a: Pt, b: Pt, c: Pt) -> Option<Pt> {
    let d = 2.0 * (a[0] * (b[1] - c[1]) + b[0] * (c[1] - a[1]) + c[0] * (a[1] - b[1]));
    if d.abs() < f64::EPSILON {
        return None;
    }
    let sq = |p: Pt| p[0] * p[0] + p[1] * p[1];
    Some([
        (sq(a) * (b[1] - c[1]) + sq(b) * (c[1] - a[1]) + sq(c) * (a[1] - b[1])) / d,
        (sq(a) * (c[0] - b[0]) + sq(b) * (a[0] - c[0]) + sq(c) * (b[0] - a[0])) / d,
    ])
}

/// The first GML geometry below `n` (a property, or a nested object's properties).
fn find_geometry<'a, 'i>(n: Node<'a, 'i>, depth: usize) -> Option<Node<'a, 'i>> {
    if depth > 8 {
        return None;
    }
    for c in elements(n) {
        if is_geometry(&c) {
            return Some(c);
        }
    }
    elements(n)
        .filter(|c| local(c) != "boundedBy")
        .find_map(|c| find_geometry(c, depth + 1))
}

fn text_of(n: Node<'_, '_>) -> String {
    n.descendants()
        .filter(Node::is_text)
        .filter_map(|t| t.text())
        .collect::<String>()
        .trim()
        .to_string()
}

/// A feature's attributes and geometry.
/// A property element's own attributes as `prop_attr` fields (`uom`, `xlink:href`, …).
fn prop_attributes(p: Node<'_, '_>, name: &str, attrs: &mut Vec<(String, Value)>) {
    for a in p.attributes() {
        let ns = a.namespace().unwrap_or("");
        if ns.contains("XMLSchema-instance") || ns.contains("opengis.net/gml") {
            continue;
        }
        if ns.contains("xlink") && a.name() != "href" {
            continue;
        }
        attrs.push((
            format!("{name}_{}", a.name()),
            Value::Text(a.value().to_string()),
        ));
    }
}

/// Repeated properties (lists) as one value: the texts joined by ", ".
fn merge_repeats(attrs: Vec<(String, Value)>) -> Vec<(String, Value)> {
    let mut out: Vec<(String, Value)> = Vec::with_capacity(attrs.len());
    for (k, v) in attrs {
        match out.iter_mut().find(|(o, _)| *o == k) {
            Some((_, prev)) => {
                let joined = match (&*prev, &v) {
                    (Value::Null, _) => v.text(),
                    (p, Value::Null) => p.text(),
                    (p, v) => format!("{}, {}", p.text(), v.text()),
                };
                *prev = Value::Text(joined);
            }
            None => out.push((k, v)),
        }
    }
    out
}

fn feature(f: Node<'_, '_>, g: &mut G<'_>, attrs: &mut Vec<(String, Value)>) -> Option<Geometry> {
    let mut geom: Option<Geometry> = None;
    if let Some(id) = f
        .attributes()
        .find(|a| {
            a.name() == "id"
                && a.namespace()
                    .is_some_and(|ns| ns.contains("opengis.net/gml"))
        })
        .or_else(|| f.attributes().find(|a| a.name() == "fid"))
    {
        attrs.push(("gml_id".into(), Value::Text(id.value().to_string())));
    }
    let mut own: Vec<(String, Value)> = Vec::new();
    // AIXM keeps a feature's properties in time slices: read the slice's as the feature's.
    let mut props: Vec<Node<'_, '_>> = Vec::new();
    for p in elements(f) {
        match (local(&p), elements(p).next()) {
            ("timeSlice", Some(slice)) => props.extend(elements(slice)),
            _ => props.push(p),
        }
    }
    for p in props {
        // CityGML generic attributes: named by their "name", valued by gen:value.
        if local(&p).ends_with("Attribute")
            && p.tag_name()
                .namespace()
                .is_some_and(|n| n.contains("citygml"))
            && let Some(n) = p.attribute("name")
        {
            let v = child(p, "value").map(text_of).unwrap_or_default();
            own.push((n.to_string(), Value::Text(v)));
            continue;
        }
        let name = local(&p);
        // A feature's boundedBy (in any namespace) is its envelope, not its geometry.
        if name == "boundedBy"
            || is_gml(&p)
                && matches!(
                    name,
                    "name" | "description" | "identifier" | "metaDataProperty"
                )
        {
            if name != "boundedBy" {
                let t = text_of(p);
                if !t.is_empty() {
                    own.push((name.to_string(), Value::Text(t)));
                }
            }
            continue;
        }
        if let Some(gn) = find_geometry(p, 0) {
            if geom.is_none() {
                geom = g.geometry(gn, None, g.ctx.default_dim, 0);
            }
            continue;
        }
        let kids: Vec<Node<'_, '_>> = elements(p).collect();
        if p.attribute(("http://www.w3.org/2001/XMLSchema-instance", "nil")) == Some("true") {
            own.push((name.to_string(), Value::Null));
            continue;
        }
        if kids.is_empty() {
            let t = p.text().unwrap_or("").trim().to_string();
            // An empty element carrying only a link is a reference, not a value.
            if !(t.is_empty() && p.attributes().next().is_some()) {
                own.push((name.to_string(), Value::Text(t)));
            }
            prop_attributes(p, name, &mut own);
        } else {
            prop_attributes(p, name, &mut own);
            // Nested properties, one level: parent_child.
            for k in kids {
                if find_geometry(k, 0).is_some() || is_geometry(&k) {
                    if geom.is_none() {
                        let gn = if is_geometry(&k) {
                            Some(k)
                        } else {
                            find_geometry(k, 0)
                        };
                        if let Some(gn) = gn {
                            geom = g.geometry(gn, None, g.ctx.default_dim, 0);
                        }
                    }
                    continue;
                }
                let kn = local(&k);
                let grand: Vec<Node<'_, '_>> = elements(k).collect();
                if grand.is_empty() {
                    own.push((format!("{name}_{kn}"), Value::Text(text_of(k))));
                } else {
                    // An object inside a property (INSPIRE base:Identifier): its children.
                    for c in grand {
                        own.push((format!("{name}_{}", local(&c)), Value::Text(text_of(c))));
                    }
                }
            }
        }
    }
    attrs.extend(merge_repeats(own));
    geom
}

/// Typed values: a column whose texts all read as integers becomes integers, as numbers
/// becomes floats.
fn type_columns(l: &mut Layer) {
    // Every column is read through every row: a layer of many thousands of keys stays text.
    if l.fields().len() > crate::model::DENSE_FIELDS {
        return;
    }
    for c in 0..l.fields().len() {
        let mut all_int = true;
        let mut all_num = true;
        let mut any = false;
        for f in &l.features {
            match f.values.get(c) {
                Some(Value::Text(s)) if !s.is_empty() => {
                    any = true;
                    let t = s.trim();
                    if t.parse::<i64>().is_err()
                        || t.starts_with('+')
                        || (t.len() > 1 && t.starts_with('0'))
                    {
                        all_int = false;
                    }
                    if t.parse::<f64>().is_err()
                        || t.eq_ignore_ascii_case("nan")
                        || t.eq_ignore_ascii_case("inf")
                    {
                        all_num = false;
                    }
                }
                Some(Value::Text(_) | Value::Null) | None => {}
                _ => {
                    all_int = false;
                    all_num = false;
                }
            }
        }
        if !any || l.fields()[c] == "gml_id" || !(all_int || all_num) {
            continue;
        }
        for f in &mut l.features {
            if let Some(v) = f.values.get_mut(c)
                && let Value::Text(s) = v
            {
                let t = s.trim();
                *v = if t.is_empty() {
                    Value::Null
                } else if all_int {
                    t.parse().map(Value::Int).unwrap_or(Value::Null)
                } else {
                    t.parse().map(Value::Float).unwrap_or(Value::Null)
                };
            }
        }
    }
}

/// The feature elements of a document: members of feature collections, recursively
/// (WFS joins and nested collections), else the root's children that carry geometry.
fn features<'a, 'i>(root: Node<'a, 'i>) -> Vec<Node<'a, 'i>> {
    let mut out = Vec::new();
    fn walk<'a, 'i>(n: Node<'a, 'i>, out: &mut Vec<Node<'a, 'i>>, depth: usize) {
        if depth > 8 {
            return;
        }
        for c in elements(n) {
            let name = local(&c);
            // featureMember(s), wfs:member, CityGML cityObjectMember, OS MasterMap
            // topographicMember, AIXM hasMember, …
            let member = name.ends_with("Member")
                || name.ends_with("Members")
                || matches!(name, "member" | "members" | "featureMemberList");
            if !member {
                continue;
            }
            for f in elements(c) {
                let fname = local(&f);
                if fname.contains("FeatureCollection")
                    || fname == "Tuple"
                    || fname == "SimpleFeatureCollection"
                {
                    walk(f, out, depth + 1);
                } else if !is_geometry(&f) {
                    out.push(f);
                }
            }
        }
    }
    walk(root, &mut out, 0);
    if out.is_empty() && !is_geometry(&root) {
        // A collection without member wrappers, or a single feature.
        let kids: Vec<Node<'a, 'i>> = elements(root)
            .filter(|c| !(is_gml(c) && matches!(local(c), "boundedBy" | "name" | "description")))
            .collect();
        if kids
            .iter()
            .any(|k| elements(*k).any(|p| find_geometry(p, 0).is_some()))
        {
            out = kids;
        } else if elements(root).any(|p| find_geometry(p, 0).is_some()) {
            out.push(root);
        }
    }
    out
}

pub fn read(data: &[u8], name: &str) -> Result<Dataset, String> {
    let text = crate::xml_text(data)?;
    // The XML parser recurses per element level.
    if !specular_limits::xml_within_depth(text.as_bytes(), specular_limits::MAX_XML_DEPTH) {
        return Err("the XML nests too deeply".into());
    }
    let doc = roxmltree::Document::parse_with_options(
        &text,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..roxmltree::ParsingOptions::default()
        },
    )
    .map_err(|e| format!("not valid XML: {e}"))?;
    let root = doc.root_element();
    let mut ctx = Ctx {
        default_srs: None,
        first_srs: None,
        short_urn: root
            .namespaces()
            .any(|n| n.uri().contains("safe.com/gml/fme")),
        default_dim: if root.namespaces().any(|n| n.uri().contains("citygml")) {
            3
        } else {
            2
        },
        vertices: 0,
        warnings: Vec::new(),
    };
    // The collection's boundedBy srsName is the default for geometries without one.
    if let Some(e) = child(root, "boundedBy").and_then(|b| elements(b).next())
        && let Some(s) = e.attribute("srsName")
    {
        ctx.default_srs = Some(s.to_string());
    }
    let mut layers: Vec<Layer> = Vec::new();
    let mut standalone: Option<Geometry> = None;
    if is_geometry(&root) {
        let mut g = G { ctx: &mut ctx };
        let d = g.ctx.default_dim;
        standalone = g.geometry(root, None, d, 0);
    }
    for f in features(root) {
        if layers.iter().map(|l| l.features.len()).sum::<usize>() >= MAX_FEATURES
            || ctx.vertices > MAX_VERTICES
        {
            ctx.warn("too many features; the rest are not shown");
            break;
        }
        let lname = local(&f).to_string();
        let mut attrs = Vec::new();
        let geom = {
            let mut g = G { ctx: &mut ctx };
            feature(f, &mut g, &mut attrs)
        };
        let li = match layers.iter().position(|l| l.name == lname) {
            Some(i) => i,
            None => {
                layers.push(Layer::new(lname.clone()));
                layers.len() - 1
            }
        };
        let id = attrs
            .iter()
            .find(|(k, _)| k == "gml_id")
            .map(|(_, v)| v.text());
        layers[li].push(id, geom, attrs);
    }
    if let Some(g) = standalone {
        let mut l = Layer::new(name);
        l.push(Some("0".into()), Some(g), Vec::new());
        layers.push(l);
    }
    if layers.is_empty() {
        return Err("the GML document holds no features".into());
    }
    for l in &mut layers {
        // Rows pushed before a field appeared are short: pad them.
        let n = crate::model::dense_width(l.fields().len());
        for f in &mut l.features {
            if f.values.len() < n {
                f.values.resize(n, Value::Null);
            }
        }
        type_columns(l);
    }
    let first = ctx.first_srs.clone().or(ctx.default_srs.clone());
    let s = first.as_deref().map(|f| srs(f, ctx.short_urn));
    let mut ds = Dataset {
        format: "GML",
        layers,
        crs: s.as_ref().and_then(|s| s.crs.clone()),
        crs_note: s
            .as_ref()
            .filter(|s| s.crs.is_none())
            .map(|s| s.name.clone()),
        ..Dataset::default()
    };
    if first.is_none() {
        ds.crs_note = Some("not given".into());
    }
    for w in ctx.warnings {
        ds.warn(w);
    }
    Ok(ds)
}

/// A coverage's grid georeference: a GMLJP2 `gml:RectifiedGrid` (OGC 05-047r3 GMLJP2 1,
/// 08-085r4 GMLJP2 2).
#[derive(Clone, Debug)]
pub struct RectifiedGrid {
    /// Pixel corner coordinates (y down) to map coordinates, x = east.
    pub to_map: crate::geotiff::Affine2,
    /// From the srsName, when it names an EPSG CRS this crate knows.
    pub crs: Option<Crs>,
    /// The srsName as written ("" when none).
    pub srs_name: String,
    /// What the grid says, as text: srsName, origin, offset vectors, grid limits.
    pub keys: Vec<(String, String)>,
}

/// The first `gml:RectifiedGrid` of a GMLJP2 XML box. Its origin is the centre of the first
/// pixel and its offset vectors step one column and one row; a srsName given as a URN or
/// http URI follows the EPSG axis order, which is swapped to x = east. `None` without a
/// two-dimensional grid with finite, non-singular vectors.
pub fn rectified_grid(xml: &[u8]) -> Option<RectifiedGrid> {
    let text = crate::xml_text(xml).ok()?;
    if !text.contains("RectifiedGrid")
        || !specular_limits::xml_within_depth(text.as_bytes(), specular_limits::MAX_XML_DEPTH)
    {
        return None;
    }
    let doc = roxmltree::Document::parse_with_options(
        &text,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..roxmltree::ParsingOptions::default()
        },
    )
    .ok()?;
    let grid = doc
        .descendants()
        .find(|n| n.is_element() && is_gml(n) && local(n) == "RectifiedGrid")?;
    let point = child(grid, "origin").and_then(|o| {
        elements(o)
            .find(|p| local(p) == "Point")
            .or_else(|| Some(o).filter(|o| child(*o, "pos").is_some()))
    })?;
    let pos = child(point, "pos")
        .map(|p| numbers(&text_of(p)))
        .or_else(|| child(point, "coordinates").map(|c| numbers(&text_of(c).replace(',', " "))))?;
    let vectors: Vec<Vec<f64>> = elements(grid)
        .filter(|n| local(n) == "offsetVector")
        .map(|n| numbers(&text_of(n)))
        .collect();
    if pos.len() < 2 || vectors.len() < 2 || vectors.iter().any(|v| v.len() < 2) {
        return None;
    }
    let srs_name = point
        .attribute("srsName")
        .or_else(|| grid.attribute("srsName"))
        .or_else(|| vectors_srs(grid))
        .unwrap_or_default()
        .to_string();
    let s = srs(&srs_name, false);
    let (mut o, mut x, mut y) = (
        [pos[0], pos[1]],
        [vectors[0][0], vectors[0][1]],
        [vectors[1][0], vectors[1][1]],
    );
    if s.swap {
        o.swap(0, 1);
        x.swap(0, 1);
        y.swap(0, 1);
    }
    // The origin is the first pixel's centre: half a step back along both vectors.
    let to_map = crate::geotiff::Affine2 {
        a: x[0],
        b: y[0],
        c: o[0] - 0.5 * x[0] - 0.5 * y[0],
        d: x[1],
        e: y[1],
        f: o[1] - 0.5 * x[1] - 0.5 * y[1],
    };
    let det = to_map.a * to_map.e - to_map.b * to_map.d;
    let finite = [to_map.a, to_map.b, to_map.c, to_map.d, to_map.e, to_map.f]
        .iter()
        .all(|v| v.is_finite());
    if !finite || det == 0.0 {
        return None;
    }
    let fmt = |v: &[f64]| {
        v.iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut keys = vec![
        ("srsName".to_string(), srs_name.clone()),
        ("origin".to_string(), fmt(&pos)),
        ("offsetVector 1".to_string(), fmt(&vectors[0])),
        ("offsetVector 2".to_string(), fmt(&vectors[1])),
    ];
    if let Some(l) = grid
        .descendants()
        .find(|n| n.is_element() && local(n) == "GridEnvelope")
    {
        let lim = |k| child(l, k).map(|n| text_of(n)).unwrap_or_default();
        keys.push((
            "limits".to_string(),
            format!("{} – {}", lim("low"), lim("high")),
        ));
    }
    keys.retain(|(_, v)| !v.trim().is_empty());
    Some(RectifiedGrid {
        to_map,
        crs: s.crs,
        srs_name,
        keys,
    })
}

/// A srsName on an offset vector (some writers put it there rather than on the origin).
fn vectors_srs<'a>(grid: Node<'a, '_>) -> Option<&'a str> {
    elements(grid)
        .filter(|n| local(n) == "offsetVector")
        .find_map(|n| n.attribute("srsName"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gml32_features() {
        let g = br#"<?xml version="1.0"?>
<wfs:FeatureCollection xmlns:wfs="http://www.opengis.net/wfs/2.0" xmlns:gml="http://www.opengis.net/gml/3.2" xmlns:app="urn:app">
 <wfs:member><app:town gml:id="t1"><app:name>Bern</app:name><app:pop>133000</app:pop>
   <app:geom><gml:Point srsName="urn:ogc:def:crs:EPSG::4326"><gml:pos>46.95 7.45</gml:pos></gml:Point></app:geom></app:town></wfs:member>
 <wfs:member><app:town gml:id="t2"><app:name>Thun</app:name><app:pop>43000</app:pop>
   <app:geom><gml:Point srsName="urn:ogc:def:crs:EPSG::4326"><gml:pos>46.76 7.63</gml:pos></gml:Point></app:geom></app:town></wfs:member>
 <wfs:member><app:lake gml:id="l1"><app:area>48.3</app:area>
   <app:geom><gml:Polygon srsName="EPSG:4326"><gml:exterior><gml:LinearRing><gml:posList>7 46 8 46 8 47 7 46</gml:posList></gml:LinearRing></gml:exterior></gml:Polygon></app:geom></app:lake></wfs:member>
</wfs:FeatureCollection>"#;
        assert!(sniff(g));
        let d = read(g, "x").unwrap();
        assert_eq!(d.layers.len(), 2);
        let t = &d.layers[0];
        assert_eq!(t.name, "town");
        assert_eq!(t.fields(), vec!["gml_id", "name", "pop"]);
        // URN: latitude first, swapped.
        assert_eq!(t.features[0].geometry, Some(Geometry::Point([7.45, 46.95])));
        assert_eq!(t.features[1].values[2], Value::Int(43000));
        assert_eq!(d.crs.as_ref().and_then(|c| c.epsg), Some(4326));
        // EPSG:4326 short form: as written.
        let Some(Geometry::Polygon(r)) = &d.layers[1].features[0].geometry else {
            panic!()
        };
        assert_eq!(r[0][1], [8.0, 46.0]);
        assert_eq!(d.layers[1].features[0].values[1], Value::Float(48.3));
    }

    #[test]
    fn gml2_coordinates_and_multi() {
        let g = br#"<ogr:FeatureCollection xmlns:ogr="http://ogr.maptools.org/" xmlns:gml="http://www.opengis.net/gml">
<gml:featureMember><ogr:roads fid="r.0"><ogr:geometryProperty><gml:MultiLineString srsName="EPSG:27700"><gml:lineStringMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:lineStringMember><gml:lineStringMember><gml:LineString><gml:coord><gml:X>5</gml:X><gml:Y>6</gml:Y></gml:coord><gml:coord><gml:X>7</gml:X><gml:Y>8</gml:Y></gml:coord></gml:LineString></gml:lineStringMember></gml:MultiLineString></ogr:geometryProperty><ogr:ref>A1</ogr:ref></ogr:roads></gml:featureMember>
</ogr:FeatureCollection>"#;
        let d = read(g, "x").unwrap();
        assert_eq!(
            d.layers[0].features[0].geometry,
            Some(Geometry::MultiLineString(vec![
                vec![[1.0, 2.0], [3.0, 4.0]],
                vec![[5.0, 6.0], [7.0, 8.0]]
            ]))
        );
        assert_eq!(d.crs.as_ref().and_then(|c| c.epsg), Some(27700));
        assert_eq!(d.layers[0].features[0].id.as_deref(), Some("r.0"));
    }

    #[test]
    fn north_first_projected_uri() {
        assert!(srs("http://www.opengis.net/def/crs/EPSG/0/31468", false).swap);
        assert!(!srs("http://www.opengis.net/def/crs/EPSG/0/25832", false).swap);
        assert!(!srs("EPSG:4326", false).swap);
        assert!(srs("EPSG:4326", true).swap);
        assert!(!srs("http://www.opengis.net/gml/srs/epsg.xml#4326", false).swap);
    }

    #[test]
    fn gmljp2_rectified_grids() {
        let grid = |srs: &str, origin: &str, v1: &str, v2: &str| {
            format!(
                r#"<gml:FeatureCollection xmlns:gml="http://www.opengis.net/gml">
                <gml:featureMember><gml:FeatureCollection><gml:featureMember>
                <gml:RectifiedGridCoverage><gml:rectifiedGridDomain>
                <gml:RectifiedGrid dimension="2">
                  <gml:limits><gml:GridEnvelope><gml:low>0 0</gml:low><gml:high>23 15</gml:high></gml:GridEnvelope></gml:limits>
                  <gml:axisName>x</gml:axisName><gml:axisName>y</gml:axisName>
                  <gml:origin><gml:Point srsName="{srs}"><gml:pos>{origin}</gml:pos></gml:Point></gml:origin>
                  <gml:offsetVector srsName="{srs}">{v1}</gml:offsetVector>
                  <gml:offsetVector srsName="{srs}">{v2}</gml:offsetVector>
                </gml:RectifiedGrid></gml:rectifiedGridDomain></gml:RectifiedGridCoverage>
                </gml:featureMember></gml:FeatureCollection></gml:featureMember>
                </gml:FeatureCollection>"#
            )
        };
        // Projected, east first: origin is the first pixel's centre.
        let g = rectified_grid(
            grid(
                "urn:ogc:def:crs:EPSG::32633",
                "500005 4599995",
                "10 0",
                "0 -10",
            )
            .as_bytes(),
        )
        .unwrap();
        let t = g.to_map;
        assert_eq!(
            [t.a, t.b, t.c, t.d, t.e, t.f],
            [10.0, 0.0, 500000.0, 0.0, -10.0, 4600000.0]
        );
        assert_eq!(g.crs.and_then(|c| c.epsg), Some(32633));
        assert!(
            g.keys
                .iter()
                .any(|(k, v)| k == "limits" && v == "0 0 – 23 15")
        );
        // Geographic URN: latitude first, swapped to x = longitude.
        let g = rectified_grid(
            grid(
                "urn:ogc:def:crs:EPSG::4326",
                "52.4375 5.625",
                "0 0.25",
                "-0.125 0",
            )
            .as_bytes(),
        )
        .unwrap();
        let t = g.to_map;
        assert_eq!(
            [t.a, t.b, t.c, t.d, t.e, t.f],
            [0.25, 0.0, 5.5, 0.0, -0.125, 52.5]
        );
        // The short "EPSG:4326" form is taken as written (x = longitude).
        let g = rectified_grid(grid("EPSG:4326", "5.625 52.4375", "0.25 0", "0 -0.125").as_bytes())
            .unwrap();
        assert_eq!((g.to_map.c, g.to_map.f), (5.5, 52.5));
        // Degenerate or missing vectors: no grid.
        assert!(rectified_grid(grid("EPSG:4326", "0 0", "0 0", "0 0").as_bytes()).is_none());
        assert!(rectified_grid(grid("EPSG:4326", "0 0", "1", "0 -1").as_bytes()).is_none());
        assert!(rectified_grid(b"<x/>").is_none());
        assert!(rectified_grid(b"not xml RectifiedGrid <").is_none());
    }
}
