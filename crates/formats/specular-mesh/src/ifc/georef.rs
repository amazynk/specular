//! Where a model sits on the Earth, for putting several models in one scene.
//!
//! - The map conversion: `IfcMapConversion` (and `IfcMapConversionScaled`) with its
//!   `IfcProjectedCRS` in IFC4 and later; in IFC2X3 the `ePSet_MapConversion` and
//!   `ePSet_ProjectedCRS` property sets (buildingSMART's georeferencing guide).
//! - The site's reference point: `IfcSite` RefLatitude, RefLongitude and RefElevation, at
//!   the site's placement.
//! - True north and the model context's world coordinate system.
//!
//! Map coordinates follow IfcOpenShell's reading (`ifcopenshell.util.geolocation`): the
//! project point goes through the inverse of the world coordinate system, then
//! `E = s·fx·(x cos θ) − s·fy·(y sin θ) + E0`, `N = s·fx·(x sin θ) + s·fy·(y cos θ) + N0`,
//! `H = s·fz·z + H0`, in the map's unit.

use super::Ctx;
use super::geom::Mat;
use super::step::V;

/// A map conversion's parameters as written (lengths in the map's unit).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapConversion {
    pub eastings: f64,
    pub northings: f64,
    pub height: f64,
    /// The project's x axis in map coordinates (not necessarily a unit vector).
    pub abscissa: f64,
    pub ordinate: f64,
    /// Project length unit to map unit.
    pub scale: f64,
    /// `IfcMapConversionScaled`'s factors (1 otherwise).
    pub factors: [f64; 3],
}

/// The projected coordinate reference system the map conversion targets.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Crs {
    /// `EPSG:32760` and the like.
    pub name: String,
    pub description: Option<String>,
    pub geodetic_datum: Option<String>,
    pub vertical_datum: Option<String>,
    pub map_projection: Option<String>,
    pub map_zone: Option<String>,
    /// Metres per map unit, when the CRS names its unit.
    pub map_unit_metres: Option<f64>,
}

/// The site's reference point on the Earth.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SiteRef {
    pub latitude: f64,
    pub longitude: f64,
    /// Metres above the datum.
    pub elevation: Option<f64>,
    /// Where the reference point is: the site placement's origin in the project (metres).
    pub origin: [f64; 3],
}

/// Everything a model says about its place.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Georef {
    pub map: Option<MapConversion>,
    pub crs: Option<Crs>,
    pub site: Option<SiteRef>,
    /// True north in the project's XY plane (unit vector), when given.
    pub true_north: Option<[f64; 2]>,
    /// The model context's world coordinate system, in project metres (rows `[x y z t]`),
    /// when it is not the identity.
    pub wcs: Option<[[f64; 4]; 3]>,
    /// Metres per project length unit.
    pub unit_metres: f64,
}

/// A row-major affine map's inverse (rotation and translation only are expected).
fn invert(m: &[[f64; 4]; 3]) -> Option<[[f64; 4]; 3]> {
    Mat(*m).inverse().map(|i| i.0)
}

fn apply(m: &[[f64; 4]; 3], p: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|r| m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2] + m[r][3])
}

impl Georef {
    /// Metres per map unit: the CRS's unit, else metres.
    pub fn map_unit_metres(&self) -> f64 {
        self.crs
            .as_ref()
            .and_then(|c| c.map_unit_metres)
            .filter(|m| *m > 0.0)
            .unwrap_or(1.0)
    }

    /// A project point (metres) on the map: eastings, northings and height in metres.
    /// None without a map conversion.
    pub fn to_map(&self, p: [f64; 3]) -> Option<[f64; 3]> {
        let m = self.map.as_ref()?;
        let p = match self.wcs.as_ref().and_then(invert) {
            Some(inv) => apply(&inv, p),
            None => p,
        };
        let u = self.unit_metres.max(1e-12);
        let [x, y, z] = p.map(|v| v / u);
        let th = m.ordinate.atan2(m.abscissa);
        let (s, c) = th.sin_cos();
        let [fx, fy, fz] = m.factors;
        let e = m.scale * fx * c * x - m.scale * fy * s * y + m.eastings;
        let n = m.scale * fx * s * x + m.scale * fy * c * y + m.northings;
        let h = m.scale * fz * z + m.height;
        let mu = self.map_unit_metres();
        Some([e * mu, n * mu, h * mu])
    }

    /// A map point (eastings, northings, height in metres) in the project (metres). None
    /// without a usable map conversion.
    pub fn from_map(&self, q: [f64; 3]) -> Option<[f64; 3]> {
        let m = self.map.as_ref()?;
        let mu = self.map_unit_metres();
        let [e, n, h] = q.map(|v| v / mu);
        let th = m.ordinate.atan2(m.abscissa);
        let (s, c) = th.sin_cos();
        let [fx, fy, fz] = m.factors;
        if m.scale == 0.0 || fx == 0.0 || fy == 0.0 || fz == 0.0 {
            return None;
        }
        let (de, dn) = (e - m.eastings, n - m.northings);
        let x = (c * de + s * dn) / (m.scale * fx);
        let y = (-s * de + c * dn) / (m.scale * fy);
        let z = (h - m.height) / (m.scale * fz);
        let u = self.unit_metres;
        let p = [x * u, y * u, z * u];
        Some(match &self.wcs {
            Some(w) => apply(w, p),
            None => p,
        })
    }

    /// The CRS's name, when the model names one.
    pub fn crs_name(&self) -> Option<&str> {
        self.crs
            .as_ref()
            .map(|c| c.name.trim())
            .filter(|n| !n.is_empty())
    }
}

/// An `IfcCompoundPlaneAngleMeasure` (degrees, minutes, seconds, millionths) in degrees.
fn compound_angle(v: &V) -> Option<f64> {
    let parts: Vec<f64> = v.as_list()?.iter().filter_map(V::as_f64).collect();
    let d = *parts.first()?;
    let m = parts.get(1).copied().unwrap_or(0.0);
    let s = parts.get(2).copied().unwrap_or(0.0);
    let us = parts.get(3).copied().unwrap_or(0.0);
    // The parts share one sign.
    Some(d + m / 60.0 + s / 3600.0 + us / 3.6e9)
}

/// Reads the model's place.
pub(super) fn find(ctx: &Ctx) -> Georef {
    let s = &ctx.s;
    let mut g = Georef {
        unit_metres: ctx.length,
        ..Georef::default()
    };
    // The 3D model context: its world coordinate system and true north.
    let contexts = s.of_types(&["IFCGEOMETRICREPRESENTATIONCONTEXT"]);
    let model = contexts
        .iter()
        .copied()
        .find(|&c| {
            s.args(c)
                .and_then(|a| a.get(1).and_then(V::as_str))
                .is_some_and(|t| t.eq_ignore_ascii_case("model"))
        })
        .or_else(|| contexts.first().copied());
    if let Some(a) = model.and_then(|c| s.args(c)) {
        if let Some(w) = a.get(4).filter(|v| v.as_ref().is_some()) {
            let m = ctx.placement(w);
            if m != Mat::IDENTITY {
                let mut rows = m.0;
                for r in &mut rows {
                    r[3] *= ctx.length;
                }
                g.wcs = Some(rows);
            }
        }
        if let Some(d) = a.get(5).and_then(V::as_ref).and_then(|d| s.args(d))
            && let Some(r) = d.first().map(V::reals)
            && r.len() >= 2
        {
            let l = (r[0] * r[0] + r[1] * r[1]).sqrt();
            if l > 1e-12 && l.is_finite() {
                g.true_north = Some([r[0] / l, r[1] / l]);
            }
        }
    }
    // IFC4 and later: the map conversion and its CRS.
    if let Some(mc) = s
        .of_types(&["IFCMAPCONVERSION", "IFCMAPCONVERSIONSCALED"])
        .into_iter()
        .next()
        && let Some(a) = s.args(mc)
    {
        let f = |i: usize| a.get(i).and_then(V::as_f64);
        let scaled = s.ty(mc) == Some("IFCMAPCONVERSIONSCALED");
        g.map = Some(MapConversion {
            eastings: f(2).unwrap_or(0.0),
            northings: f(3).unwrap_or(0.0),
            height: f(4).unwrap_or(0.0),
            abscissa: f(5).unwrap_or(0.0),
            ordinate: f(6).unwrap_or(0.0),
            scale: f(7).filter(|v| *v != 0.0).unwrap_or(1.0),
            factors: if scaled {
                [
                    f(8).unwrap_or(1.0),
                    f(9).unwrap_or(1.0),
                    f(10).unwrap_or(1.0),
                ]
            } else {
                [1.0; 3]
            },
        });
        if let Some(crs) = a.get(1).and_then(V::as_ref)
            && let Some(c) = s.args(crs)
        {
            let txt = |i: usize| c.get(i).and_then(V::as_str).filter(|t| !t.is_empty());
            // IfcProjectedCRS: Name, Description, GeodeticDatum, VerticalDatum,
            // MapProjection, MapZone, MapUnit.
            g.crs = Some(Crs {
                name: txt(0).unwrap_or_default(),
                description: txt(1),
                geodetic_datum: txt(2),
                vertical_datum: txt(3),
                map_projection: txt(4),
                map_zone: txt(5),
                map_unit_metres: c
                    .get(6)
                    .and_then(V::as_ref)
                    .and_then(|u| super::unit_factor(s, u, 0))
                    .map(|(_, f, _)| f),
            });
        }
    }
    // IFC2X3: the property sets of the georeferencing guide.
    if g.map.is_none() {
        let mut conv: Option<MapConversion> = None;
        let mut crs: Option<Crs> = None;
        for ps in s.of_types(&["IFCPROPERTYSET"]) {
            let Some(a) = s.args(ps) else { continue };
            let name = a.get(2).and_then(V::as_str).unwrap_or_default();
            let is_conv = name.eq_ignore_ascii_case("epset_mapconversion");
            let is_crs = name.eq_ignore_ascii_case("epset_projectedcrs");
            if !is_conv && !is_crs {
                continue;
            }
            // (name, number, text) of each single value.
            let mut vals: Vec<(String, Option<f64>, Option<String>)> = Vec::new();
            for p in a.get(4).map(V::refs).unwrap_or_default() {
                let Some(pa) = s.args(p) else { continue };
                if s.ty(p) != Some("IFCPROPERTYSINGLEVALUE") {
                    continue;
                }
                let pn = pa.first().and_then(V::as_str).unwrap_or_default();
                let v = pa.get(2);
                vals.push((
                    pn.to_ascii_lowercase(),
                    v.and_then(V::as_f64),
                    v.and_then(V::as_str),
                ));
            }
            let num = |k: &str| vals.iter().find(|v| v.0 == k).and_then(|v| v.1);
            let text = |k: &str| {
                vals.iter()
                    .find(|v| v.0 == k)
                    .and_then(|v| v.2.clone())
                    .filter(|t| !t.is_empty())
            };
            if is_conv {
                conv = Some(MapConversion {
                    eastings: num("eastings").unwrap_or(0.0),
                    northings: num("northings").unwrap_or(0.0),
                    height: num("orthogonalheight").unwrap_or(0.0),
                    abscissa: num("xaxisabscissa").unwrap_or(0.0),
                    ordinate: num("xaxisordinate").unwrap_or(0.0),
                    scale: num("scale").filter(|v| *v != 0.0).unwrap_or(1.0),
                    factors: [1.0; 3],
                });
            } else {
                let unit = text("mapunit").and_then(|u| match u.to_ascii_lowercase().as_str() {
                    "metre" | "meter" | "m" => Some(1.0),
                    "millimetre" | "millimeter" | "mm" => Some(1e-3),
                    "foot" | "ft" => Some(0.3048),
                    "us survey foot" => Some(1200.0 / 3937.0),
                    _ => None,
                });
                crs = Some(Crs {
                    name: text("name").unwrap_or_default(),
                    description: text("description"),
                    geodetic_datum: text("geodeticdatum"),
                    vertical_datum: text("verticaldatum"),
                    map_projection: text("mapprojection"),
                    map_zone: text("mapzone"),
                    map_unit_metres: unit,
                });
            }
        }
        g.map = conv;
        if g.map.is_some() {
            g.crs = crs;
        }
    }
    if let Some(m) = &mut g.map
        && m.abscissa == 0.0
        && m.ordinate == 0.0
    {
        m.abscissa = 1.0;
    }
    // The site's reference point (the first site that gives one).
    for site in s.of_types(&["IFCSITE"]) {
        let Some(a) = s.args(site) else { continue };
        let (Some(lat), Some(lon)) = (
            a.get(9).and_then(compound_angle),
            a.get(10).and_then(compound_angle),
        ) else {
            continue;
        };
        let place = a
            .get(5)
            .and_then(V::as_ref)
            .map_or(Mat::IDENTITY, |p| ctx.object_placement(p, 0));
        g.site = Some(SiteRef {
            latitude: lat,
            longitude: lon,
            elevation: a.get(11).and_then(V::as_f64).map(|e| e * ctx.length),
            origin: place.origin().map(|v| v * ctx.length),
        });
        break;
    }
    g
}

/// Facts for the info panel.
pub(super) fn facts(g: &Georef) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some(c) = g.crs_name() {
        out.push(("Coordinate system".to_owned(), c.to_owned()));
    }
    if let Some(m) = &g.map {
        let th = m.ordinate.atan2(m.abscissa).to_degrees();
        out.push((
            "Map conversion".to_owned(),
            format!(
                "E {:.3}, N {:.3}, H {:.3}, rotation {th:.4}°, scale {}",
                m.eastings, m.northings, m.height, m.scale
            ),
        ));
    }
    if let Some(s) = &g.site {
        out.push((
            "Site reference".to_owned(),
            format!("{:.6}°, {:.6}°", s.latitude, s.longitude),
        ));
    }
    out
}
