//! Own LandXML reader (1.0 to 1.2, written from the published schema): TIN surfaces (points,
//! faces, breaklines, boundaries), horizontal alignments (lines, circular arcs and spirals;
//! clothoids computed exactly by integrating the curvature), profiles (design PVIs with
//! parabolic and circular vertical curves, and ground lines), units and the coordinate
//! system. [`scene`] turns it into a vector drawing: a plan with the surfaces coloured by
//! elevation, contours, breaklines, boundaries and the alignments with their stationing, and a
//! chart per profile.
//!
//! LandXML writes coordinates northing first: `N E Z`. Here x is the easting and y the
//! northing.

pub mod align;
pub mod model;
mod scene;
pub mod tin;

use std::collections::HashMap;

use roxmltree::{Document, Node, ParsingOptions};

pub use scene::scene;

/// XML nodes a file may have.
const MAX_NODES: u32 = 60_000_000;

#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum Error {
    #[error("not a LandXML file")]
    NotLandXml,
    #[error("{0}")]
    Malformed(String),
    /// Plain English: why a file or part of it is refused.
    #[error("{0}")]
    Unsupported(String),
    #[error("too large")]
    TooLarge,
}

/// Whether `head` starts a LandXML file.
pub fn is_landxml(head: &[u8]) -> bool {
    let n = head.len().min(4096);
    String::from_utf8_lossy(&head[..n]).contains("<LandXML")
}

/// Linear unit of the file.
#[derive(Clone, Debug, PartialEq)]
pub struct Units {
    /// As written ("meter", "USSurveyFoot").
    pub linear: String,
    /// Metres per unit.
    pub metres: f64,
    pub imperial: bool,
}

impl Default for Units {
    fn default() -> Self {
        Units {
            linear: "meter".into(),
            metres: 1.0,
            imperial: false,
        }
    }
}

/// The coordinate system as the file names it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Crs {
    pub name: Option<String>,
    pub epsg: Option<String>,
    pub horizontal_datum: Option<String>,
    pub vertical_datum: Option<String>,
    pub projection: Option<String>,
    /// The OGC WKT, when given.
    pub wkt: Option<String>,
}

impl Crs {
    /// One line for the info panel: the name, its EPSG code and datums.
    pub fn describe(&self) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(n) = self.name.as_ref().or(self.projection.as_ref()) {
            parts.push(n.clone());
        }
        if let Some(e) = &self.epsg {
            parts.push(format!("EPSG:{e}"));
        }
        if let Some(h) = &self.horizontal_datum {
            parts.push(h.clone());
        }
        if let Some(v) = &self.vertical_datum {
            parts.push(v.clone());
        }
        if parts.is_empty() {
            self.wkt.as_ref().map(|w| w.chars().take(80).collect())
        } else {
            Some(parts.join(", "))
        }
    }
}

/// A boundary of a surface.
#[derive(Clone, Debug, PartialEq)]
pub struct Boundary {
    pub name: String,
    /// "outer", "void", "island" as written.
    pub kind: String,
    pub points: Vec<[f64; 2]>,
}

/// A TIN surface: points (x east, y north, z) and triangles of point indices.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Surface {
    pub name: String,
    pub points: Vec<[f64; 3]>,
    pub faces: Vec<[u32; 3]>,
    /// Faces the file marks invisible (`i="1"`), not drawn or measured.
    pub hidden_faces: u64,
    pub breaklines: Vec<Vec<[f64; 3]>>,
    pub boundaries: Vec<Boundary>,
}

/// One element of a horizontal alignment.
#[derive(Clone, Debug, PartialEq)]
pub enum Geom {
    Line {
        start: [f64; 2],
        end: [f64; 2],
    },
    Arc {
        start: [f64; 2],
        end: [f64; 2],
        center: [f64; 2],
        /// Counter-clockwise.
        ccw: bool,
    },
    Spiral {
        start: [f64; 2],
        /// The tangent's direction at the start (radians from east, counter-clockwise).
        dir: f64,
        length: f64,
        /// Curvature at the start and the end (1/radius, positive turning left).
        k0: f64,
        k1: f64,
        /// The end the file states (checked against the computed one).
        end: Option<[f64; 2]>,
        /// The spiral type as written, when not a clothoid.
        other: Option<String>,
    },
}

/// A vertical element of a design profile.
#[derive(Clone, Debug, PartialEq)]
pub struct Pvi {
    pub station: f64,
    pub elevation: f64,
    /// A vertical curve's length centred on the PVI: parabolic, or circular with a radius.
    pub curve: Option<f64>,
    pub circular_radius: Option<f64>,
}

/// A profile of an alignment.
#[derive(Clone, Debug, PartialEq)]
pub enum Profile {
    Design { name: String, pvis: Vec<Pvi> },
    Ground { name: String, points: Vec<[f64; 2]> },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Alignment {
    pub name: String,
    pub sta_start: f64,
    /// The length the file states.
    pub length: Option<f64>,
    pub elements: Vec<Geom>,
    pub profiles: Vec<Profile>,
}

/// A parcel: its boundary (lines and curves), name, the area the file states and its
/// label point.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Parcel {
    pub name: String,
    pub area: Option<f64>,
    pub center: Option<[f64; 2]>,
    pub boundary: Vec<Geom>,
}

/// A survey point (`CgPoint`): (x east, y north), its elevation when given, name and code
/// (`code`, else `desc`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CgPoint {
    pub name: String,
    pub code: Option<String>,
    pub at: [f64; 2],
    pub z: Option<f64>,
}

/// A whole file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LandXml {
    pub version: String,
    pub units: Units,
    pub crs: Option<Crs>,
    pub application: Option<String>,
    pub project: Option<String>,
    pub surfaces: Vec<Surface>,
    pub alignments: Vec<Alignment>,
    pub parcels: Vec<Parcel>,
    pub points: Vec<CgPoint>,
    /// Plain English, deduplicated.
    pub warnings: Vec<String>,
}

fn warn(w: &mut Vec<String>, s: impl Into<String>) {
    let s = s.into();
    if w.len() < 64 && !w.contains(&s) {
        w.push(s);
    }
}

fn is(n: Node<'_, '_>, name: &str) -> bool {
    n.is_element() && n.tag_name().name() == name
}

fn child<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.children().find(|c| is(*c, name))
}

fn children<'a, 'i: 'a>(n: Node<'a, 'i>, name: &'a str) -> impl Iterator<Item = Node<'a, 'i>> {
    n.children().filter(move |c| is(*c, name))
}

fn f(n: Node<'_, '_>, a: &str) -> Option<f64> {
    n.attribute(a)
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite())
}

fn nums(s: &str) -> Vec<f64> {
    s.split_whitespace()
        .filter_map(|t| t.parse::<f64>().ok())
        .collect()
}

/// A point element's text, `N E [Z]`, as (x east, y north).
fn ne(n: Node<'_, '_>) -> Option<[f64; 2]> {
    let v = nums(n.text()?);
    Some([*v.get(1)?, *v.first()?])
}

fn ne_child(n: Node<'_, '_>, name: &str) -> Option<[f64; 2]> {
    child(n, name).and_then(ne)
}

/// Radius text to curvature: "INF" or 0 is straight.
fn curvature(r: Option<&str>) -> f64 {
    match r.map(str::trim) {
        None => 0.0,
        Some(s) if s.eq_ignore_ascii_case("INF") => 0.0,
        Some(s) => match s.parse::<f64>() {
            Ok(r) if r.is_finite() && r != 0.0 => 1.0 / r,
            _ => 0.0,
        },
    }
}

fn units(root: Node<'_, '_>) -> Units {
    let Some(u) = child(root, "Units") else {
        return Units::default();
    };
    let Some(c) = u.children().find(|c| c.is_element()) else {
        return Units::default();
    };
    {
        let imperial = c.tag_name().name() == "Imperial";
        let lin = c
            .attribute("linearUnit")
            .unwrap_or(if imperial { "foot" } else { "meter" });
        let metres = match lin.to_ascii_lowercase().as_str() {
            "millimeter" => 0.001,
            "centimeter" => 0.01,
            "kilometer" => 1000.0,
            "foot" | "intlfoot" => 0.3048,
            "ussurveyfoot" => 1200.0 / 3937.0,
            "inch" => 0.0254,
            "mile" => 1609.344,
            _ => 1.0,
        };
        Units {
            linear: lin.to_string(),
            metres,
            imperial,
        }
    }
}

fn surface(s: Node<'_, '_>, warnings: &mut Vec<String>) -> Surface {
    let mut out = Surface {
        name: s.attribute("name").unwrap_or("").to_string(),
        ..Surface::default()
    };
    let Some(def) = child(s, "Definition") else {
        return out;
    };
    if let Some(t) = def.attribute("surfType")
        && !t.eq_ignore_ascii_case("TIN")
    {
        warn(
            warnings,
            format!(
                "Surface {} is a {t} surface; only TIN surfaces are drawn.",
                out.name
            ),
        );
        return out;
    }
    let mut ids: HashMap<&str, u32> = HashMap::new();
    if let Some(pnts) = child(def, "Pnts") {
        for p in children(pnts, "P") {
            let v = nums(p.text().unwrap_or(""));
            if v.len() < 2 {
                continue;
            }
            let z = v.get(2).copied().unwrap_or(0.0);
            if let Some(id) = p.attribute("id") {
                ids.insert(id, out.points.len() as u32);
            }
            out.points.push([v[1], v[0], z]);
        }
    }
    if let Some(faces) = child(def, "Faces") {
        for fc in children(faces, "F") {
            if fc.attribute("i") == Some("1") {
                out.hidden_faces += 1;
                continue;
            }
            let t = fc.text().unwrap_or("");
            let idx: Vec<u32> = t
                .split_whitespace()
                .filter_map(|id| ids.get(id).copied())
                .collect();
            if idx.len() == 3 {
                out.faces.push([idx[0], idx[1], idx[2]]);
            }
        }
    }
    if let Some(src) = child(s, "SourceData") {
        if let Some(bl) = child(src, "Breaklines") {
            for b in children(bl, "Breakline") {
                let pts = child(b, "PntList3D")
                    .map(|l| {
                        nums(l.text().unwrap_or(""))
                            .chunks_exact(3)
                            .map(|c| [c[1], c[0], c[2]])
                            .collect::<Vec<_>>()
                    })
                    .or_else(|| {
                        child(b, "PntList2D").map(|l| {
                            nums(l.text().unwrap_or(""))
                                .chunks_exact(2)
                                .map(|c| [c[1], c[0], f64::NAN])
                                .collect()
                        })
                    })
                    .unwrap_or_default();
                if pts.len() >= 2 {
                    out.breaklines.push(pts);
                }
            }
        }
        if let Some(bs) = child(src, "Boundaries") {
            for b in children(bs, "Boundary") {
                let (list, step) = match child(b, "PntList2D") {
                    Some(l) => (l, 2),
                    None => match child(b, "PntList3D") {
                        Some(l) => (l, 3),
                        None => continue,
                    },
                };
                let pts: Vec<[f64; 2]> = nums(list.text().unwrap_or(""))
                    .chunks_exact(step)
                    .map(|c| [c[1], c[0]])
                    .collect();
                if pts.len() >= 3 {
                    out.boundaries.push(Boundary {
                        name: b.attribute("name").unwrap_or("").to_string(),
                        kind: b.attribute("bndType").unwrap_or("outer").to_string(),
                        points: pts,
                    });
                }
            }
        }
    }
    out
}

fn alignment(a: Node<'_, '_>, warnings: &mut Vec<String>) -> Alignment {
    let mut out = Alignment {
        name: a.attribute("name").unwrap_or("").to_string(),
        sta_start: f(a, "staStart").unwrap_or(0.0),
        length: f(a, "length"),
        ..Alignment::default()
    };
    if let Some(cg) = child(a, "CoordGeom") {
        for e in cg.children().filter(|c| c.is_element()) {
            match e.tag_name().name() {
                "Line" => {
                    if let (Some(start), Some(end)) = (ne_child(e, "Start"), ne_child(e, "End")) {
                        out.elements.push(Geom::Line { start, end });
                    }
                }
                "Curve" => {
                    let (Some(start), Some(end)) = (ne_child(e, "Start"), ne_child(e, "End"))
                    else {
                        continue;
                    };
                    let ccw = !e
                        .attribute("rot")
                        .is_some_and(|r| r.eq_ignore_ascii_case("cw"));
                    let center = ne_child(e, "Center").or_else(|| {
                        // No centre: from the radius and the turning side.
                        let r = f(e, "radius")?;
                        align::arc_center(start, end, r, ccw)
                    });
                    match center {
                        Some(center) => out.elements.push(Geom::Arc {
                            start,
                            end,
                            center,
                            ccw,
                        }),
                        None => warn(
                            warnings,
                            format!("A curve of alignment {} has no centre or radius.", out.name),
                        ),
                    }
                }
                "Spiral" => {
                    let (Some(start), Some(len)) = (ne_child(e, "Start"), f(e, "length")) else {
                        continue;
                    };
                    let end = ne_child(e, "End");
                    // The start tangent points at the PI (else at the end).
                    let toward = ne_child(e, "PI").or(end);
                    let Some(t) = toward else { continue };
                    let dir = (t[1] - start[1]).atan2(t[0] - start[0]);
                    let sign = if e
                        .attribute("rot")
                        .is_some_and(|r| r.eq_ignore_ascii_case("cw"))
                    {
                        -1.0
                    } else {
                        1.0
                    };
                    let k0 = curvature(e.attribute("radiusStart")).abs() * sign;
                    let k1 = curvature(e.attribute("radiusEnd")).abs() * sign;
                    let ty = e.attribute("spiType").unwrap_or("clothoid");
                    let other = (!ty.eq_ignore_ascii_case("clothoid")).then(|| ty.to_string());
                    if let Some(o) = &other {
                        warn(
                            warnings,
                            format!(
                                "{o} spirals are drawn as clothoids of the same length and radii."
                            ),
                        );
                    }
                    out.elements.push(Geom::Spiral {
                        start,
                        dir,
                        length: len,
                        k0,
                        k1,
                        end,
                        other,
                    });
                }
                "IrregularLine" | "Chain" => warn(
                    warnings,
                    format!(
                        "Alignment {}: irregular lines and chains are not drawn.",
                        out.name
                    ),
                ),
                _ => {}
            }
        }
    }
    if let Some(prof) = child(a, "Profile") {
        for pa in children(prof, "ProfAlign") {
            let mut pvis = Vec::new();
            for p in pa.children().filter(|c| c.is_element()) {
                let v = nums(p.text().unwrap_or(""));
                if v.len() < 2 {
                    continue;
                }
                let (curve, radius) = match p.tag_name().name() {
                    "PVI" => (None, None),
                    "ParaCurve" | "UnsymParaCurve" => (f(p, "length"), None),
                    "CircCurve" => (f(p, "length"), f(p, "radius")),
                    _ => continue,
                };
                pvis.push(Pvi {
                    station: v[0],
                    elevation: v[1],
                    curve,
                    circular_radius: radius,
                });
            }
            if !pvis.is_empty() {
                out.profiles.push(Profile::Design {
                    name: pa.attribute("name").unwrap_or("Design").to_string(),
                    pvis,
                });
            }
        }
        for ps in children(prof, "ProfSurf") {
            let pts: Vec<[f64; 2]> = child(ps, "PntList2D")
                .map(|l| {
                    nums(l.text().unwrap_or(""))
                        .chunks_exact(2)
                        .map(|c| [c[0], c[1]])
                        .collect()
                })
                .unwrap_or_default();
            if pts.len() >= 2 {
                out.profiles.push(Profile::Ground {
                    name: ps.attribute("name").unwrap_or("Ground").to_string(),
                    points: pts,
                });
            }
        }
    }
    out
}

/// Reads a LandXML file.
/// Survey points at most.
const MAX_POINTS: usize = 1_000_000;

/// The file's survey points (`CgPoints` at the top, nested groups too): `N E [Z]` text, or a
/// `pntRef` to a point named before or after.
fn cg_points(root: Node<'_, '_>) -> Vec<CgPoint> {
    let mut out: Vec<CgPoint> = Vec::new();
    let mut refs: Vec<(usize, &str)> = Vec::new();
    for g in children(root, "CgPoints") {
        for p in g.descendants().filter(|n| is(*n, "CgPoint")) {
            if out.len() >= MAX_POINTS {
                break;
            }
            let v = nums(p.text().unwrap_or(""));
            let mut c = CgPoint {
                name: p.attribute("name").unwrap_or("").to_string(),
                code: p
                    .attribute("code")
                    .or_else(|| p.attribute("desc"))
                    .map(str::to_string),
                ..CgPoint::default()
            };
            if v.len() >= 2 {
                c.at = [v[1], v[0]];
                c.z = v.get(2).copied();
            } else if let Some(r) = p.attribute("pntRef") {
                refs.push((out.len(), r));
            } else {
                continue;
            }
            out.push(c);
        }
    }
    let by_name: HashMap<String, ([f64; 2], Option<f64>)> = out
        .iter()
        .enumerate()
        .filter(|(i, _)| !refs.iter().any(|(k, _)| k == i))
        .map(|(_, c)| (c.name.clone(), (c.at, c.z)))
        .collect();
    let mut lost = Vec::new();
    for (i, r) in refs {
        match by_name.get(r) {
            Some(&(at, z)) => {
                out[i].at = at;
                out[i].z = z;
            }
            None => lost.push(i),
        }
    }
    for i in lost.into_iter().rev() {
        out.remove(i);
    }
    out
}

pub fn parse(bytes: &[u8]) -> Result<LandXml, Error> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| Error::Malformed("the file is not UTF-8 XML".into()))?;
    let opt = ParsingOptions {
        allow_dtd: false,
        nodes_limit: MAX_NODES,
        ..ParsingOptions::default()
    };
    let doc = Document::parse_with_options(text, opt).map_err(|e| match e {
        roxmltree::Error::NodesLimitReached => Error::TooLarge,
        e => Error::Malformed(format!("damaged XML: {e}")),
    })?;
    let root = doc.root_element();
    if root.tag_name().name() != "LandXML" {
        return Err(Error::NotLandXml);
    }
    let mut out = LandXml {
        version: root.attribute("version").unwrap_or("").to_string(),
        units: units(root),
        ..LandXml::default()
    };
    if let Some(c) = child(root, "CoordinateSystem") {
        let a = |k: &str| c.attribute(k).map(str::to_string);
        out.crs = Some(Crs {
            name: a("name").or_else(|| a("desc")),
            epsg: a("epsgCode"),
            horizontal_datum: a("horizontalDatum"),
            vertical_datum: a("verticalDatum"),
            projection: a("projectedCoordinateSystemName"),
            wkt: a("ogcWktCode"),
        });
    }
    if let Some(app) = child(root, "Application") {
        let name = app.attribute("name").unwrap_or("");
        let v = app.attribute("version").unwrap_or("");
        let s = format!("{name} {v}").trim().to_string();
        if !s.is_empty() {
            out.application = Some(s);
        }
    }
    out.project = child(root, "Project").and_then(|p| p.attribute("name").map(Into::into));
    let mut warnings = Vec::new();
    for ss in children(root, "Surfaces") {
        for s in children(ss, "Surface") {
            out.surfaces.push(surface(s, &mut warnings));
        }
    }
    for al in children(root, "Alignments") {
        for a in children(al, "Alignment") {
            out.alignments.push(alignment(a, &mut warnings));
        }
    }
    for ps in children(root, "Parcels") {
        for p in ps.descendants().filter(|n| is(*n, "Parcel")) {
            let name = p.attribute("name").unwrap_or("").to_string();
            let boundary = alignment(p, &mut warnings).elements;
            out.parcels.push(Parcel {
                name,
                area: f(p, "area"),
                center: ne_child(p, "Center"),
                boundary,
            });
        }
    }
    out.points = cg_points(root);
    let unsupported: Vec<&str> = root
        .children()
        .filter(|c| c.is_element())
        .map(|c| c.tag_name().name())
        .filter(|n| {
            matches!(
                *n,
                "PipeNetworks" | "PlanFeatures" | "Roadways" | "Survey" | "GradeModel"
            )
        })
        .collect();
    for n in unsupported {
        warn(&mut warnings, format!("{n} are not drawn."));
    }
    if out.surfaces.iter().all(|s| s.faces.is_empty())
        && out.alignments.is_empty()
        && out.parcels.is_empty()
        && out.points.is_empty()
    {
        return Err(Error::Unsupported(
            "This LandXML file has no TIN surface, alignment, parcel or survey point to show."
                .into(),
        ));
    }
    out.warnings = warnings;
    Ok(out)
}

#[cfg(test)]
mod tests;
