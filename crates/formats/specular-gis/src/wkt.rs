//! CRS definitions as text: WKT1 (OGC and ESRI `.prj` flavours), WKT2 (ISO 19162) and
//! PROJJSON, read far enough to name the CRS, find its EPSG code and rebuild a supported
//! projection from its method and parameters.

use crate::crs::{self, Crs, CrsKind, LinearUnit};
use crate::datum::{Helmert, ToWgs84};
use crate::proj::{Ellipsoid, Method, Projection};

/// A WKT node: `KEYWORD[arg, arg, ...]`.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Str(String),
    Num(f64),
    /// A bare word (WKT2 enumerations such as `north`).
    Word(String),
    Node(Node),
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Node {
    pub keyword: String,
    pub args: Vec<Value>,
}

impl Node {
    /// The first string argument (the name of most nodes).
    pub fn name(&self) -> Option<&str> {
        self.args.iter().find_map(|a| match a {
            Value::Str(s) => Some(s.as_str()),
            _ => None,
        })
    }

    /// Child nodes with this keyword (case-insensitive).
    pub fn children<'a>(&'a self, kw: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.args.iter().filter_map(move |a| match a {
            Value::Node(n) if n.keyword.eq_ignore_ascii_case(kw) => Some(n),
            _ => None,
        })
    }

    pub fn child<'a>(&'a self, kws: &[&'a str]) -> Option<&'a Node> {
        kws.iter().find_map(|k| self.children(k).next())
    }

    /// The `i`th numeric argument.
    pub fn num(&self, i: usize) -> Option<f64> {
        self.args
            .iter()
            .filter_map(|a| match a {
                Value::Num(v) => Some(*v),
                _ => None,
            })
            .nth(i)
    }

    /// EPSG code from `AUTHORITY["EPSG","4326"]` (WKT1) or `ID["EPSG",4326]` (WKT2).
    pub fn epsg(&self) -> Option<u32> {
        let a = self.child(&["AUTHORITY", "ID"])?;
        if !a.name()?.eq_ignore_ascii_case("EPSG") {
            return None;
        }
        match a.args.get(1)? {
            Value::Num(v) => Some(*v as u32),
            Value::Str(s) => s.trim().parse().ok(),
            _ => None,
        }
    }
}

/// Parse WKT text into its root node. Lenient: `[`/`(` brackets, either quotes.
pub fn parse(text: &str) -> Option<Node> {
    let b = text.as_bytes();
    let mut i = 0;
    let n = parse_node(b, &mut i, 0)?;
    Some(n)
}

fn skip_ws(b: &[u8], i: &mut usize) {
    while *i < b.len() && (b[*i].is_ascii_whitespace() || b[*i] == b',') {
        *i += 1;
    }
}

fn parse_node(b: &[u8], i: &mut usize, depth: usize) -> Option<Node> {
    if depth > 64 {
        return None;
    }
    skip_ws(b, i);
    let start = *i;
    while *i < b.len() && (b[*i].is_ascii_alphanumeric() || b[*i] == b'_') {
        *i += 1;
    }
    let keyword = std::str::from_utf8(&b[start..*i]).ok()?.to_string();
    skip_ws(b, i);
    let mut node = Node {
        keyword,
        args: Vec::new(),
    };
    if *i >= b.len() || !(b[*i] == b'[' || b[*i] == b'(') {
        return Some(node);
    }
    *i += 1;
    loop {
        skip_ws(b, i);
        if *i >= b.len() {
            return Some(node);
        }
        match b[*i] {
            b']' | b')' => {
                *i += 1;
                return Some(node);
            }
            b'"' => {
                *i += 1;
                let mut s = Vec::new();
                while *i < b.len() {
                    if b[*i] == b'"' {
                        // WKT2 escapes a quote by doubling it.
                        if b.get(*i + 1) == Some(&b'"') {
                            s.push(b'"');
                            *i += 2;
                            continue;
                        }
                        *i += 1;
                        break;
                    }
                    s.push(b[*i]);
                    *i += 1;
                }
                node.args
                    .push(Value::Str(String::from_utf8_lossy(&s).into_owned()));
            }
            c if c == b'-' || c == b'+' || c == b'.' || c.is_ascii_digit() => {
                let st = *i;
                *i += 1;
                while *i < b.len()
                    && (b[*i].is_ascii_digit() || matches!(b[*i], b'.' | b'e' | b'E' | b'-' | b'+'))
                {
                    *i += 1;
                }
                let v = std::str::from_utf8(&b[st..*i]).ok()?.parse().ok()?;
                node.args.push(Value::Num(v));
            }
            c if c.is_ascii_alphabetic() => {
                let mark = *i;
                let child = parse_node(b, i, depth + 1)?;
                if child.args.is_empty() && *i > mark && !matches!(b.get(*i - 1), Some(b']' | b')'))
                {
                    node.args.push(Value::Word(child.keyword));
                } else {
                    node.args.push(Value::Node(child));
                }
            }
            _ => {
                *i += 1;
            }
        }
    }
}

fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// The CRS a WKT definition describes.
pub fn crs_from_wkt(text: &str) -> Option<Crs> {
    let root = parse(text)?;
    let mut c = crs_from_node(&root)?;
    c.wkt = Some(text.trim().to_string());
    Some(c)
}

fn crs_from_node(root: &Node) -> Option<Crs> {
    let kw = root.keyword.to_ascii_uppercase();
    match kw.as_str() {
        // Compound and bound CRSs: the horizontal (or source) part.
        "COMPD_CS" | "COMPOUNDCRS" => {
            let first = root.args.iter().find_map(|a| match a {
                Value::Node(n) => crs_from_node(n),
                _ => None,
            })?;
            Some(Crs {
                name: root.name().unwrap_or(&first.name).to_string(),
                epsg: root.epsg().or(first.epsg),
                ..first
            })
        }
        "BOUNDCRS" => {
            let src = root.child(&["SOURCECRS"])?;
            src.args.iter().find_map(|a| match a {
                Value::Node(n) => crs_from_node(n),
                _ => None,
            })
        }
        "GEOGCS" | "GEOGCRS" | "GEODCRS" | "GEODETICCRS" | "GEOGRAPHICCRS" => {
            let name = root.name().unwrap_or("").to_string();
            if let Some(code) = root.epsg()
                && let Some(mut c) = crs::from_epsg(code)
            {
                c.name = name;
                let d = datum_of(root);
                if matches!(d.shift, ToWgs84::One(_)) {
                    c.datum_shift = d.shift;
                }
                return Some(c);
            }
            if let Some(code) = crs::epsg_from_esri_name(&name)
                && let Some(mut c) = crs::from_epsg(code)
                && c.kind == CrsKind::Geographic
            {
                c.name = name;
                return Some(c);
            }
            let d = datum_of(root);
            let geocentric = kw == "GEODCRS"
                && root
                    .child(&["CS"])
                    .and_then(|cs| cs.args.first())
                    .is_some_and(
                        |a| matches!(a, Value::Word(w) if w.eq_ignore_ascii_case("Cartesian")),
                    );
            Some(Crs {
                epsg: None,
                name: name.clone(),
                kind: if geocentric {
                    CrsKind::Other
                } else {
                    CrsKind::Geographic
                },
                geographic: Some(name),
                datum: d.name,
                ellipsoid: d.ellipsoid,
                ellipsoid_name: d.ellipsoid_name,
                projection: None,
                method_name: None,
                unit: None,
                wkt: None,
                projjson: None,
                pm: d.pm,
                datum_shift: d.shift,
            })
        }
        "PROJCS" | "PROJCRS" | "PROJECTEDCRS" => {
            let name = root.name().unwrap_or("").to_string();
            let base = root.child(&["GEOGCS", "BASEGEOGCRS", "BASEGEODCRS", "GEODCRS", "GEOGCRS"]);
            let d = base.map(datum_of).unwrap_or_default();
            let (datum, ell, ell_name) = (d.name.clone(), d.ellipsoid, d.ellipsoid_name.clone());
            let base_name = base.and_then(|b| b.name()).map(str::to_string);
            let unit = linear_unit(root);
            let epsg = root.epsg().or_else(|| crs::epsg_from_esri_name(&name));
            let mut c = match epsg.and_then(crs::from_epsg) {
                Some(c) if c.kind == CrsKind::Projected => c,
                _ => {
                    let (method, method_name) = method_of(root, &unit);
                    Crs {
                        epsg: None,
                        name: name.clone(),
                        kind: CrsKind::Projected,
                        geographic: base_name.clone(),
                        datum: datum.clone(),
                        ellipsoid: ell,
                        ellipsoid_name: ell_name.clone(),
                        projection: method.map(|m| Projection::new(m, ell)),
                        method_name: if method.is_some() { None } else { method_name },
                        unit: Some(unit.clone()),
                        wkt: None,
                        projjson: None,
                        pm: d.pm,
                        datum_shift: d.shift.clone(),
                    }
                }
            };
            c.name = name;
            if c.geographic.is_none() {
                c.geographic = base_name;
            }
            if datum.is_some() {
                c.datum = datum;
            }
            // A TOWGS84 given in the definition wins over the dataset's choice.
            if matches!(d.shift, ToWgs84::One(_)) || !c.datum_shift.is_known() {
                c.datum_shift = d.shift;
            }
            Some(c)
        }
        "VERT_CS" | "VERTCRS" | "LOCAL_CS" | "ENGCRS" | "ENGINEERINGCRS" | "GEOCCS" => {
            let mut c = Crs::named(root.name().unwrap_or("").to_string(), CrsKind::Other);
            c.epsg = root.epsg();
            Some(c)
        }
        _ => None,
    }
}

/// A geographic CRS node's datum: its name, ellipsoid, prime meridian and relation to
/// WGS 84 (a WKT1 TOWGS84 clause, else the EPSG dataset's shifts for that datum name).
#[derive(Clone, Debug)]
struct Datum {
    name: Option<String>,
    ellipsoid: Ellipsoid,
    ellipsoid_name: Option<String>,
    pm: f64,
    shift: ToWgs84,
}

impl Default for Datum {
    fn default() -> Datum {
        Datum {
            name: None,
            ellipsoid: Ellipsoid::WGS84,
            ellipsoid_name: None,
            pm: 0.0,
            shift: ToWgs84::Unknown,
        }
    }
}

fn datum_of(geog: &Node) -> Datum {
    let datum = geog.child(&["DATUM", "TRF", "GEODETICDATUM", "ENSEMBLE"]);
    let sph = datum
        .and_then(|d| d.child(&["SPHEROID", "ELLIPSOID"]))
        .or_else(|| geog.child(&["SPHEROID", "ELLIPSOID"]));
    let ell = sph
        .and_then(|s| Some(Ellipsoid::inv(s.num(0)?, s.num(1).unwrap_or(0.0))))
        .filter(|e| e.a > 6_000_000.0 && e.a < 7_000_000.0)
        .unwrap_or(Ellipsoid::WGS84);
    let name = datum.and_then(|d| d.name()).map(str::to_string);
    let towgs84 = datum.and_then(|d| d.child(&["TOWGS84"])).and_then(|t| {
        let v: Vec<f64> = (0..7).map_while(|i| t.num(i)).collect();
        Helmert::from_towgs84(&v)
    });
    // PRIMEM["Paris",2.33722917] in degrees (WKT1 and ESRI), or with an ANGLEUNIT (WKT2).
    let pm = geog
        .child(&["PRIMEM", "PRIMEMERIDIAN"])
        .and_then(|p| {
            let v = p.num(0)?;
            Some(match p.child(&["ANGLEUNIT", "UNIT"]) {
                Some(u) => (v * u.num(0).unwrap_or(std::f64::consts::PI / 180.0)).to_degrees(),
                None => v,
            })
        })
        .filter(|v| v.is_finite() && v.abs() <= 180.0)
        .unwrap_or(0.0);
    let shift = match towgs84 {
        Some(h) if h.is_null() && ell == Ellipsoid::WGS84 => ToWgs84::Same,
        Some(h) => ToWgs84::One(h),
        None => name
            .as_deref()
            .map(crate::epsg::to_wgs84_by_datum)
            .unwrap_or_default(),
    };
    Datum {
        name,
        ellipsoid: ell,
        ellipsoid_name: sph.and_then(|s| s.name()).map(str::to_string),
        pm,
        shift,
    }
}

fn linear_unit(root: &Node) -> LinearUnit {
    // WKT1: UNIT as a direct child; WKT2: LENGTHUNIT in the CS axes or at the root.
    let u = root.child(&["UNIT", "LENGTHUNIT"]).or_else(|| {
        root.children("AXIS")
            .find_map(|a| a.child(&["LENGTHUNIT", "UNIT"]))
    });
    match u {
        Some(u) => LinearUnit {
            name: u.name().unwrap_or("metre").to_string(),
            metres: u.num(0).filter(|v| *v > 0.0).unwrap_or(1.0),
        },
        None => LinearUnit::metre(),
    }
}

/// The projection method and parameters of a projected CRS node (degrees, metres).
fn method_of(root: &Node, unit: &LinearUnit) -> (Option<Method>, Option<String>) {
    // WKT1: PROJECTION + PARAMETERs on the root; WKT2: CONVERSION[METHOD, PARAMETER...].
    let conv = root
        .child(&["CONVERSION", "DERIVINGCONVERSION"])
        .unwrap_or(root);
    let mname = conv
        .child(&["PROJECTION", "METHOD"])
        .and_then(|m| m.name())
        .unwrap_or("")
        .to_string();
    let mcode = conv.child(&["METHOD"]).and_then(|m| m.epsg());
    let mut p = Params::default();
    for par in conv.children("PARAMETER") {
        let Some(v) = par.num(0) else { continue };
        let key = norm(par.name().unwrap_or(""));
        // WKT2 units per parameter; WKT1 lengths are in the CRS unit, angles in degrees.
        let v = if let Some(lu) = par.child(&["LENGTHUNIT"]) {
            v * lu.num(0).unwrap_or(1.0)
        } else if let Some(au) = par.child(&["ANGLEUNIT"]) {
            (v * au.num(0).unwrap_or(std::f64::consts::PI / 180.0)).to_degrees()
        } else if par.child(&["SCALEUNIT"]).is_some() {
            v
        } else if is_length(&key) {
            v * unit.metres
        } else {
            v
        };
        p.set(&key, v);
    }
    (
        build_method(&norm(&mname), mcode, &p),
        Some(mname).filter(|s| !s.is_empty()),
    )
}

fn is_length(key: &str) -> bool {
    key.contains("easting") || key.contains("northing")
}

#[derive(Default, Debug)]
struct Params {
    lat0: Option<f64>,
    lon0: Option<f64>,
    k0: Option<f64>,
    fe: Option<f64>,
    fn_: Option<f64>,
    lat1: Option<f64>,
    lat2: Option<f64>,
    /// Azimuth of the initial line (oblique Mercator).
    azimuth: Option<f64>,
    /// Angle from the rectified to the skew grid (oblique Mercator).
    gamma: Option<f64>,
    /// Latitude of the standard parallel (polar stereographic variant B).
    lat_ts: Option<f64>,
}

impl Params {
    fn set(&mut self, key: &str, v: f64) {
        let slot = match key {
            "latitudeoforigin"
            | "latitudeofnaturalorigin"
            | "latitudeoffalseorigin"
            | "latitudeofcenter"
            | "latitudeofprojectioncentre"
            | "latitudeofcentre" => &mut self.lat0,
            "centralmeridian"
            | "longitudeofnaturalorigin"
            | "longitudeoffalseorigin"
            | "longitudeofcenter"
            | "longitudeofcentre"
            | "longitudeoforigin" => &mut self.lon0,
            "scalefactor"
            | "scalefactoratnaturalorigin"
            | "scalefactoronstandardparallel"
            | "scalefactoroninitialline"
            | "scalefactoratprojectioncentre" => &mut self.k0,
            "falseeasting" | "eastingatfalseorigin" | "eastingatprojectioncentre" => &mut self.fe,
            "falsenorthing" | "northingatfalseorigin" | "northingatprojectioncentre" => {
                &mut self.fn_
            }
            "azimuth" | "azimuthofinitialline" | "azimuthatprojectioncentre" => &mut self.azimuth,
            "rectifiedgridangle" | "anglefromrectifiedtoskewgrid" => &mut self.gamma,
            "latitudeofstandardparallel" | "standardparallel" => &mut self.lat_ts,
            "standardparallel1" | "latitudeof1ststandardparallel" => &mut self.lat1,
            "standardparallel2" | "latitudeof2ndstandardparallel" => &mut self.lat2,
            _ => return,
        };
        *slot = Some(v);
    }
}

fn build_method(name: &str, code: Option<u32>, p: &Params) -> Option<Method> {
    let lat0 = p.lat0.unwrap_or(0.0);
    let lon0 = p.lon0.unwrap_or(0.0);
    let fe = p.fe.unwrap_or(0.0);
    let fn_ = p.fn_.unwrap_or(0.0);
    let k0 = p.k0.unwrap_or(1.0);
    let has = |s: &str| name.contains(s);
    let polar_lat = || p.lat_ts.or(p.lat1).or(p.lat0);
    let hotine = |centre: bool| -> Option<Method> {
        let alpha = p.azimuth?;
        Some(Method::HotineObliqueMercator {
            latc: lat0,
            lonc: lon0,
            alpha,
            gamma: p.gamma.unwrap_or(alpha),
            kc: k0,
            fe,
            fn_,
            centre,
        })
    };
    Some(match code {
        Some(9808) => Method::TransverseMercatorSouth {
            lat0,
            lon0,
            k0,
            fe,
            fn_,
        },
        Some(9810) => Method::PolarStereographicA {
            south: lat0 < 0.0,
            lon0,
            k0,
            fe,
            fn_,
        },
        Some(9829) => Method::PolarStereographic {
            lat_ts: polar_lat()?,
            lon0,
            south: polar_lat()? < 0.0,
            fe,
            fn_,
        },
        Some(9820) => Method::LambertAzimuthal {
            lat0,
            lon0,
            fe,
            fn_,
        },
        Some(9809) => Method::ObliqueStereographic {
            lat0,
            lon0,
            k0,
            fe,
            fn_,
        },
        Some(9812) => hotine(false)?,
        Some(9815) => hotine(true)?,
        Some(9806) => Method::CassiniSoldner {
            lat0,
            lon0,
            fe,
            fn_,
        },
        Some(1028) => Method::EquidistantCylindrical {
            lat_ts: p.lat1.unwrap_or(0.0),
            lon0,
            fe,
            fn_,
        },
        Some(1029) => Method::Equirectangular {
            lat_ts: p.lat1.unwrap_or(0.0),
            lon0,
            fe,
            fn_,
        },
        Some(9807) => Method::TransverseMercator {
            lat0,
            lon0,
            k0,
            fe,
            fn_,
        },
        Some(1024) => Method::WebMercator { lon0, fe, fn_ },
        Some(9804) => Method::Mercator { lon0, k0, fe, fn_ },
        Some(9805) => Method::MercatorB {
            lon0,
            lat1: p.lat1?,
            fe,
            fn_,
        },
        Some(9802) => Method::LambertConic {
            lat0,
            lon0,
            lat1: p.lat1?,
            lat2: p.lat2?,
            k0: 1.0,
            fe,
            fn_,
        },
        Some(9801) => Method::LambertConic {
            lat0,
            lon0,
            lat1: lat0,
            lat2: lat0,
            k0,
            fe,
            fn_,
        },
        Some(9822) => Method::Albers {
            lat0,
            lon0,
            lat1: p.lat1?,
            lat2: p.lat2?,
            fe,
            fn_,
        },
        _ if has("transverse") && has("mercator") && has("southorientated") => {
            Method::TransverseMercatorSouth {
                lat0,
                lon0,
                k0,
                fe,
                fn_,
            }
        }
        _ if has("hotine") || has("obliquemercator") && !has("swiss") => {
            // OGC WKT1 "Hotine_Oblique_Mercator" is variant A; "..._Azimuth_Center" (ESRI)
            // and "Oblique_Mercator" (GDAL's variant B) have their origin at the centre.
            hotine(!has("naturalorigin") && (has("center") || has("centre") || !has("hotine")))?
        }
        _ if has("polarstereographic")
            || has("stereographicnorthpole")
            || has("stereographicsouthpole") =>
        {
            match polar_lat() {
                // Variant A: true scale at the pole, given as a scale factor.
                Some(l) if l.abs() == 90.0 && p.lat_ts.is_none() && p.lat1.is_none() => {
                    Method::PolarStereographicA {
                        south: l < 0.0 || has("southpole"),
                        lon0,
                        k0,
                        fe,
                        fn_,
                    }
                }
                Some(l) => Method::PolarStereographic {
                    lat_ts: if has("southpole") && l > 0.0 { -l } else { l },
                    lon0,
                    south: l < 0.0 || has("southpole"),
                    fe,
                    fn_,
                },
                None => return None,
            }
        }
        _ if has("obliquestereographic") || has("doublestereographic") => {
            Method::ObliqueStereographic {
                lat0,
                lon0,
                k0,
                fe,
                fn_,
            }
        }
        _ if has("lambertazimuthal") => Method::LambertAzimuthal {
            lat0,
            lon0,
            fe,
            fn_,
        },
        _ if has("cassini") => Method::CassiniSoldner {
            lat0,
            lon0,
            fe,
            fn_,
        },
        _ if has("equirectangular") || has("equidistantcylindrical") || has("platecarree") => {
            Method::EquidistantCylindrical {
                lat_ts: p.lat1.unwrap_or(0.0),
                lon0,
                fe,
                fn_,
            }
        }
        _ if has("transverse") && has("mercator") && !has("southorientated") => {
            Method::TransverseMercator {
                lat0,
                lon0,
                k0,
                fe,
                fn_,
            }
        }
        _ if has("pseudomercator")
            || has("mercatorauxiliarysphere")
            || has("popularvisualisation") =>
        {
            Method::WebMercator { lon0, fe, fn_ }
        }
        _ if has("mercator") && !has("oblique") => match p.lat1 {
            Some(lat1) if lat1 != 0.0 || name.contains("2sp") => Method::MercatorB {
                lon0,
                lat1,
                fe,
                fn_,
            },
            _ => Method::Mercator { lon0, k0, fe, fn_ },
        },
        _ if has("lambert") && has("conformal") || has("lambertconicconformal") => {
            match (p.lat1, p.lat2) {
                (Some(l1), Some(l2)) if !name.contains("1sp") => Method::LambertConic {
                    lat0,
                    lon0,
                    lat1: l1,
                    lat2: l2,
                    k0: 1.0,
                    fe,
                    fn_,
                },
                // ESRI writes 1SP as Lambert_Conformal_Conic with one standard parallel.
                (Some(l1), None) => Method::LambertConic {
                    lat0: l1,
                    lon0,
                    lat1: l1,
                    lat2: l1,
                    k0,
                    fe,
                    fn_,
                },
                _ => Method::LambertConic {
                    lat0,
                    lon0,
                    lat1: lat0,
                    lat2: lat0,
                    k0,
                    fe,
                    fn_,
                },
            }
        }
        _ if has("albers") => Method::Albers {
            lat0,
            lon0,
            lat1: p.lat1?,
            lat2: p.lat2?,
            fe,
            fn_,
        },
        _ => return None,
    })
}

/// The CRS a PROJJSON definition describes: name, EPSG id and kind (and, through the EPSG
/// table, the projection).
pub fn crs_from_projjson(text: &str) -> Option<Crs> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let name = v.get("name")?.as_str()?.to_string();
    let typ = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let epsg = v
        .get("id")
        .filter(|id| id.get("authority").and_then(|a| a.as_str()) == Some("EPSG"))
        .and_then(|id| id.get("code"))
        .and_then(|c| c.as_u64().or_else(|| c.as_str()?.parse().ok()))
        .map(|c| c as u32);
    let mut c = match epsg.and_then(crs::from_epsg) {
        Some(c) => c,
        None => Crs::named(
            name.clone(),
            match typ {
                "GeographicCRS" => CrsKind::Geographic,
                "ProjectedCRS" => CrsKind::Projected,
                _ => CrsKind::Other,
            },
        ),
    };
    c.name = name;
    c.projjson = Some(text.trim().to_string());
    Some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    const UTM_ESRI: &str = r#"PROJCS["WGS_1984_UTM_Zone_33N",GEOGCS["GCS_WGS_1984",DATUM["D_WGS_1984",SPHEROID["WGS_1984",6378137.0,298.257223563]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]],PROJECTION["Transverse_Mercator"],PARAMETER["False_Easting",500000.0],PARAMETER["False_Northing",0.0],PARAMETER["Central_Meridian",15.0],PARAMETER["Scale_Factor",0.9996],PARAMETER["Latitude_Of_Origin",0.0],UNIT["Meter",1.0]]"#;

    #[test]
    fn esri_prj() {
        let c = crs_from_wkt(UTM_ESRI).unwrap();
        assert_eq!(c.name, "WGS_1984_UTM_Zone_33N");
        assert_eq!(c.epsg, Some(32633));
        let (lon, _) = c.to_lonlat(500_000.0, 1000.0).unwrap();
        assert!((lon - 15.0).abs() < 1e-12);
    }

    #[test]
    fn custom_lcc_wkt1_and_wkt2() {
        let w1 = r#"PROJCS["Custom LCC",GEOGCS["NAD83",DATUM["North_American_Datum_1983",SPHEROID["GRS 1980",6378137,298.257222101]],PRIMEM["Greenwich",0],UNIT["degree",0.0174532925199433]],PROJECTION["Lambert_Conformal_Conic_2SP"],PARAMETER["standard_parallel_1",33],PARAMETER["standard_parallel_2",45],PARAMETER["latitude_of_origin",39],PARAMETER["central_meridian",-96],PARAMETER["false_easting",0],PARAMETER["false_northing",0],UNIT["metre",1]]"#;
        let c = crs_from_wkt(w1).unwrap();
        assert!(c.projection.is_some(), "{c:?}");
        let (lon, lat) = c.to_lonlat(0.0, 0.0).unwrap();
        assert!((lon + 96.0).abs() < 1e-12 && (lat - 39.0).abs() < 1e-10);
        let w2 = r#"PROJCRS["NAD83 / Conus Albers",BASEGEOGCRS["NAD83",DATUM["North American Datum 1983",ELLIPSOID["GRS 1980",6378137,298.257222101,LENGTHUNIT["metre",1]]],PRIMEM["Greenwich",0,ANGLEUNIT["degree",0.0174532925199433]]],CONVERSION["Conus Albers",METHOD["Albers Equal Area",ID["EPSG",9822]],PARAMETER["Latitude of false origin",23,ANGLEUNIT["degree",0.0174532925199433]],PARAMETER["Longitude of false origin",-96,ANGLEUNIT["degree",0.0174532925199433]],PARAMETER["Latitude of 1st standard parallel",29.5,ANGLEUNIT["degree",0.0174532925199433]],PARAMETER["Latitude of 2nd standard parallel",45.5,ANGLEUNIT["degree",0.0174532925199433]],PARAMETER["Easting at false origin",0,LENGTHUNIT["metre",1]],PARAMETER["Northing at false origin",0,LENGTHUNIT["metre",1]]],CS[Cartesian,2],AXIS["easting (X)",east,ORDER[1],LENGTHUNIT["metre",1]],AXIS["northing (Y)",north,ORDER[2],LENGTHUNIT["metre",1]],ID["EPSG",5070]]"#;
        let c = crs_from_wkt(w2).unwrap();
        assert_eq!(c.epsg, Some(5070));
        assert_eq!(c.name, "NAD83 / Conus Albers");
        let geo = crs_from_wkt(r#"GEOGCRS["WGS 84",DATUM["World Geodetic System 1984",ELLIPSOID["WGS 84",6378137,298.257223563]],CS[ellipsoidal,2],ID["EPSG",4326]]"#).unwrap();
        assert_eq!((geo.epsg, geo.kind), (Some(4326), CrsKind::Geographic));
    }
}
