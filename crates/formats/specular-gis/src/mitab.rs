//! MapInfo tables: native TAB (the `.tab` header with its `.map` geometry, `.id` object
//! index and `.dat` attributes, or a `.dbf` for "Type DBF" tables) and MIF/MID interchange
//! files.
//!
//! Geometry: points (plain, font and custom symbols), lines, polylines and multiple
//! polylines, regions, arcs, rectangles, rounded rectangles, ellipses, text (a point at the
//! text's lower-left corner, the string as a `text` attribute), multipoints and
//! collections, in their compressed and V300/V450/V650/V800 forms. Arcs, ellipses and
//! rounded corners are stroked the way GDAL strokes them (2° steps for arcs, 180 points for
//! ellipses, 45 per corner) so the vertices line up with other tools.
//!
//! The coordinate system (`CoordSys` in a MIF, the projection block of the `.map` header) is
//! mapped to an EPSG CRS where one matches its datum and parameters, else to a projection
//! built from the parameters. Seamless tables, views, raster registrations and tables
//! linked to other data are refused by name.

use std::path::Path;

use encoding_rs::Encoding;

use crate::crs::{self, Crs, CrsKind, LinearUnit};
use crate::datum::{Helmert, ToWgs84};
use crate::model::{Dataset, Geometry, Layer, MAX_FEATURES, MAX_VERTICES, Pt, Value};
use crate::proj::{Ellipsoid, Method, Projection};

// --------------------------------------------------------------------------------- sniff

fn head_text(head: &[u8]) -> String {
    String::from_utf8_lossy(&head[..head.len().min(4096)]).to_ascii_lowercase()
}

/// A native table header: starts with `!table`.
pub fn sniff_tab(head: &[u8]) -> bool {
    head_text(head)
        .trim_start_matches('\u{feff}')
        .trim_start()
        .starts_with("!table")
}

/// A MIF header: a `Version` line, then `Columns` or `Data` near the start.
pub fn sniff_mif(head: &[u8]) -> bool {
    let t = head_text(head);
    let mut lines = t
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .peekable();
    let Some(first) = lines.next() else {
        return false;
    };
    if !first.trim_start_matches('\u{feff}').starts_with("version") {
        return false;
    }
    lines.any(|l| l.starts_with("columns") || l == "data")
}

// ------------------------------------------------------------------------------ charset

/// The text encoding of a MapInfo charset name.
fn encoding(charset: &str) -> Option<&'static Encoding> {
    let c = charset.trim().trim_matches('"').to_ascii_lowercase();
    let label: &str = match c.as_str() {
        "windowslatin1" => "windows-1252",
        "windowslatin2" => "windows-1250",
        "windowsarabic" => "windows-1256",
        "windowscyrillic" => "windows-1251",
        "windowsgreek" => "windows-1253",
        "windowshebrew" => "windows-1255",
        "windowsturkish" => "windows-1254",
        "windowsbalticrim" => "windows-1257",
        "windowsvietnamese" => "windows-1258",
        "windowsthai" => "windows-874",
        "windowstradchinese" => "big5",
        "windowssimpchinese" => "gbk",
        "windowsjapanese" => "shift_jis",
        "windowskorean" => "euc-kr",
        "codepage866" | "codepage855" => "ibm866",
        "utf-8" | "utf8" => "utf-8",
        "macroman" => "macintosh",
        "maccyrillic" => "x-mac-cyrillic",
        s if s.starts_with("iso8859_") => {
            return Encoding::for_label(format!("iso-8859-{}", &s[8..]).as_bytes());
        }
        _ => return None,
    };
    Encoding::for_label(label.as_bytes())
}

/// Decodes text in `enc`, or for "Neutral" (no charset) as UTF-8 when valid, else
/// Windows Latin 1.
fn decode(b: &[u8], enc: Option<&'static Encoding>) -> String {
    match enc {
        Some(e) => e.decode_without_bom_handling(b).0.into_owned(),
        None => match std::str::from_utf8(b) {
            Ok(s) => s.to_string(),
            Err(_) => encoding_rs::WINDOWS_1252
                .decode_without_bom_handling(b)
                .0
                .into_owned(),
        },
    }
}

// ------------------------------------------------------------------------- coordinate system

/// A MapInfo coordinate system: projection, datum and parameters as a `CoordSys` clause
/// or the `.map` header gives them.
#[derive(Clone, Debug, Default, PartialEq)]
struct CoordSys {
    non_earth: bool,
    /// Projection number (affine and bounds flags removed).
    proj: u32,
    datum: u32,
    /// The ellipsoid of a custom datum (999, 9999) or of the `.map` header.
    ellipsoid: Option<u32>,
    /// Custom datum shift: dx, dy, dz and, for 9999, rx, ry, rz, scale (ppm), prime meridian.
    shift: Vec<f64>,
    unit: Option<String>,
    params: Vec<f64>,
}

/// MapInfo datum numbers to EPSG geographic CRS codes (the common ones).
fn datum_epsg(d: u32) -> Option<u32> {
    Some(match d {
        1 => 4201,
        2 => 4205,
        3 => 4204,
        5 => 4209,
        6 => 4210,
        12 => 4202,
        13 => 4203,
        15 => 4216,
        16 => 4218,
        17 => 4221,
        19 => 4222,
        21 => 4223,
        23 => 4224,
        24 => 4225,
        28 => 4230,
        29 => 4668,
        31 => 4272,
        36 => 4254,
        37 => 4658,
        38 => 4738,
        39 => 4236,
        42 => 4299,
        45 => 4244,
        47 => 4245,
        49 => 4251,
        50 => 4253,
        52 => 4256,
        54 => 4262,
        55 => 4261,
        57 => 4263,
        61 => 4271,
        62 => 4267,
        74 => 4269,
        76 => 4229,
        77 => 4135,
        78 => 4232,
        79 => 4277,
        82 => 4248,
        83 => 4139,
        84 => 4285,
        85 => 4287,
        86 => 4626,
        90 => 4292,
        91 => 4293,
        92 => 4618,
        96 => 4298,
        97 => 4301,
        103 => 4322,
        104 | 157 => 4326,
        105 => 4309,
        106 => 4311,
        107 => 4275,
        108 => 4231,
        109 => 4289,
        110 => 4313,
        112 => 4124,
        113 => 4207,
        114 => 4274,
        115 => 4258,
        116 => 4283,
        117 => 4167,
        150 => 4148,
        151 => 4122,
        152 => 4612,
        154 => 4619,
        1000 => 4314,
        1001 => 4284,
        1002 => 4807,
        1003 => 4149,
        1004 => 4237,
        1006 => 4123,
        _ => return None,
    })
}

/// MapInfo ellipsoid numbers (the common ones).
fn ellipsoid(id: u32) -> Option<(Ellipsoid, &'static str)> {
    Some(match id {
        0 => (Ellipsoid::GRS80, "GRS 1980"),
        2 => (
            Ellipsoid::inv(6_378_160.0, 298.25),
            "Australian National Spheroid",
        ),
        3 => (Ellipsoid::KRASSOVSKY, "Krassowsky 1940"),
        4 => (Ellipsoid::INTL1924, "International 1924"),
        7 => (Ellipsoid::CLARKE1866, "Clarke 1866"),
        9 => (Ellipsoid::AIRY1830, "Airy 1830"),
        10 => (Ellipsoid::BESSEL1841, "Bessel 1841"),
        28 => (Ellipsoid::WGS84, "WGS 84"),
        29 => (Ellipsoid::WGS72, "WGS 72"),
        _ => return None,
    })
}

/// A MapInfo unit name or number as a linear unit.
fn unit(name: &str) -> Option<LinearUnit> {
    let (n, m) = match name.trim().trim_matches('"').to_ascii_lowercase().as_str() {
        "m" | "7" => ("metre", 1.0),
        "km" | "1" => ("kilometre", 1000.0),
        "mi" | "0" => ("mile", 1609.344),
        "in" | "2" => ("inch", 0.0254),
        "ft" | "3" => ("foot", 0.3048),
        "yd" | "4" => ("yard", 0.9144),
        "mm" | "5" => ("millimetre", 0.001),
        "cm" | "6" => ("centimetre", 0.01),
        "survey ft" | "8" => ("US survey foot", 1200.0 / 3937.0),
        "nmi" | "9" => ("nautical mile", 1852.0),
        "li" | "30" => ("link", 0.201_168),
        "ch" | "31" => ("chain", 20.116_8),
        "rd" | "32" => ("rod", 5.029_2),
        _ => return None,
    };
    Some(LinearUnit {
        name: n.into(),
        metres: m,
    })
}

const PROJ_NAMES: &[(u32, &str)] = &[
    (2, "Cylindrical Equal-Area"),
    (5, "Azimuthal Equidistant"),
    (6, "Equidistant Conic"),
    (11, "Miller Cylindrical"),
    (12, "Robinson"),
    (13, "Mollweide"),
    (14, "Eckert IV"),
    (15, "Eckert VI"),
    (17, "Gall"),
    (18, "New Zealand Map Grid"),
    (19, "Lambert Conformal Conic (Belgium 1972)"),
    (22, "Swiss Oblique Mercator"),
    (24, "Polyconic"),
    (25, "Swiss Oblique Mercator"),
    (28, "Azimuthal Equidistant"),
    (32, "Krovak"),
];

/// The method of a projection number with its MapInfo parameter order.
fn method(proj: u32, p: &[f64]) -> Option<Method> {
    let g = |i: usize| p.get(i).copied().unwrap_or(0.0);
    Some(match proj {
        3 => Method::LambertConic {
            lon0: g(0),
            lat0: g(1),
            lat1: g(2),
            lat2: g(3),
            k0: 1.0,
            fe: g(4),
            fn_: g(5),
        },
        4 | 29 => Method::LambertAzimuthal {
            lon0: g(0),
            lat0: g(1),
            fe: 0.0,
            fn_: 0.0,
        },
        7 => Method::HotineObliqueMercator {
            lonc: g(0),
            latc: g(1),
            alpha: g(2),
            gamma: g(2),
            kc: g(3),
            fe: g(4),
            fn_: g(5),
            centre: false,
        },
        // Rectified skew orthomorphic: azimuth and the rectified grid's angle given.
        35 => Method::HotineObliqueMercator {
            lonc: g(0),
            latc: g(1),
            alpha: g(2),
            gamma: g(3),
            kc: g(4),
            fe: g(5),
            fn_: g(6),
            centre: false,
        },
        8 | 21 | 23 | 24 | 26 | 27 | 34 => Method::TransverseMercator {
            lon0: g(0),
            lat0: g(1),
            k0: g(2),
            fe: g(3),
            fn_: g(4),
        },
        9 => Method::Albers {
            lon0: g(0),
            lat0: g(1),
            lat1: g(2),
            lat2: g(3),
            fe: g(4),
            fn_: g(5),
        },
        10 => Method::Mercator {
            lon0: g(0),
            k0: 1.0,
            fe: 0.0,
            fn_: 0.0,
        },
        16 => Method::Sinusoidal {
            lon0: g(0),
            fe: 0.0,
            fn_: 0.0,
        },
        20 => Method::ObliqueStereographic {
            lon0: g(0),
            lat0: g(1),
            k0: g(2),
            fe: g(3),
            fn_: g(4),
        },
        30 => Method::EquidistantCylindrical {
            lon0: g(0),
            lat_ts: g(1),
            fe: g(2),
            fn_: g(3),
        },
        31 => Method::PolarStereographicA {
            lon0: g(0),
            south: g(1) < 0.0,
            k0: g(2),
            fe: g(3),
            fn_: g(4),
        },
        33 => Method::CassiniSoldner {
            lon0: g(0),
            lat0: g(1),
            fe: g(2),
            fn_: g(3),
        },
        _ => return None,
    })
}

/// A projected EPSG CRS on `base` (a geographic EPSG code) that projects like `p` in
/// `metres`-long units: the whole table is searched, comparing projected test points.
fn search_projected(base: u32, p: &Projection, metres: f64) -> Option<Crs> {
    let base_name = crs::from_epsg(base)?.name;
    let lon0 = match p.method {
        Method::TransverseMercator { lon0, .. }
        | Method::LambertConic { lon0, .. }
        | Method::Albers { lon0, .. }
        | Method::Mercator { lon0, .. }
        | Method::ObliqueStereographic { lon0, .. }
        | Method::CassiniSoldner { lon0, .. }
        | Method::LambertAzimuthal { lon0, .. } => lon0,
        _ => return None,
    };
    let pts = [(lon0 + 0.7, 47.0), (lon0 - 0.4, -21.0), (lon0 + 0.1, 3.0)];
    let ours: Vec<Option<(f64, f64)>> = pts.iter().map(|&(x, y)| p.forward(x, y)).collect();
    if ours.iter().all(Option::is_none) {
        return None;
    }
    crate::epsg::codes().into_iter().find_map(|code| {
        if code < 2000 || (4000..5000).contains(&code) {
            return None; // geographic ranges
        }
        let c = crate::epsg::lookup(code)?;
        if c.kind != CrsKind::Projected || c.geographic.as_deref() != Some(base_name.as_str()) {
            return None;
        }
        if c.unit.as_ref().map_or(1.0, |u| u.metres) != metres {
            return None;
        }
        let q = c.projection.as_ref()?;
        let same = pts
            .iter()
            .zip(&ours)
            .all(|(&(x, y), o)| match (q.forward(x, y), o) {
                (Some(a), Some(b)) => {
                    (a.0 / metres - b.0 / metres).abs() < 1e-3
                        && (a.1 / metres - b.1 / metres).abs() < 1e-3
                }
                (None, None) => true,
                _ => false,
            });
        same.then_some(c)
    })
}

/// The CRS of a MapInfo coordinate system, or a note on why there is none.
fn crs_of(cs: &CoordSys) -> (Option<Crs>, Option<String>) {
    if cs.non_earth {
        return (None, Some("non-earth (plain coordinates)".into()));
    }
    // The geographic base: an EPSG datum, else a custom datum on a known ellipsoid.
    let base = datum_epsg(cs.datum).and_then(crs::from_epsg);
    let custom = matches!(cs.datum, 0 | 33 | 999 | 9999) || base.is_none();
    let (ell, ell_name) = match (&base, cs.ellipsoid.and_then(ellipsoid)) {
        (Some(b), _) if !custom => (b.ellipsoid, b.ellipsoid_name.clone()),
        (_, Some((e, n))) => (e, Some(n.to_string())),
        // Datum 33 is "GRS 80".
        _ if cs.datum == 33 => (Ellipsoid::GRS80, Some("GRS 1980".into())),
        (Some(b), None) => (b.ellipsoid, b.ellipsoid_name.clone()),
        (None, None) => (Ellipsoid::WGS84, None),
    };
    let shift = if cs.shift.len() >= 3 && cs.datum >= 999 {
        // MapInfo rotations follow the coordinate-frame convention: negated for TOWGS84.
        let s = &cs.shift;
        let v: Vec<f64> = if s.len() >= 7 {
            vec![s[0], s[1], s[2], -s[3], -s[4], -s[5], s[6]]
        } else {
            s[..3].to_vec()
        };
        Helmert::from_towgs84(&v).map_or(ToWgs84::Unknown, ToWgs84::One)
    } else {
        ToWgs84::Unknown
    };
    let datum_note = || format!("MapInfo datum {}", cs.datum);
    let fill = |mut c: Crs| -> Crs {
        if let Some(b) = base.as_ref().filter(|_| !custom) {
            c.geographic = Some(b.name.clone());
            c.datum = b.datum.clone();
            c.datum_shift = b.datum_shift.clone();
            c.pm = b.pm;
        } else {
            c.datum_shift = shift.clone();
            c.ellipsoid_name = ell_name.clone();
            if cs.datum == 33 {
                c.datum = None;
            }
        }
        c
    };
    match cs.proj {
        1 => match base.as_ref().filter(|_| !custom) {
            Some(b) => (Some(b.clone()), None),
            None => {
                let mut c = Crs::named(
                    format!("Longitude/latitude ({})", datum_note()),
                    CrsKind::Geographic,
                );
                c.ellipsoid = ell;
                (Some(fill(c)), None)
            }
        },
        // Popular Visualisation Pseudo-Mercator: datum 157 with Mercator.
        10 if cs.datum == 157 => (crs::from_epsg(3857), None),
        p => {
            let u = cs
                .unit
                .as_deref()
                .and_then(unit)
                .unwrap_or_else(LinearUnit::metre);
            let Some(m) = method(p, &cs.params) else {
                let name = PROJ_NAMES.iter().find(|(n, _)| *n == p).map_or_else(
                    || format!("MapInfo projection {p}"),
                    |(_, s)| (*s).to_string(),
                );
                let mut c = Crs::named(name.clone(), CrsKind::Projected);
                c.method_name = Some(name);
                c.unit = Some(u);
                c.ellipsoid = ell;
                return (Some(fill(c)), None);
            };
            let metres = u.metres;
            let name = format!(
                "{} ({})",
                m.name(),
                base.as_ref()
                    .filter(|_| !custom)
                    .map_or_else(datum_note, |b| b.name.clone())
            );
            let c = fill(Crs::custom_projected(name, ell, m, u));
            if let Some(k) = crs::identify(&c) {
                return (Some(k), None);
            }
            // The table may list the standard parallels the other way round.
            if let Some(Method::LambertConic { lat1, lat2, .. } | Method::Albers { lat1, lat2, .. }) =
                c.projection.as_ref().map(|p| p.method)
                && lat1 != lat2
            {
                let mut swapped = c.clone();
                if let Some(p) = swapped.projection.as_mut() {
                    let mut m = p.method;
                    if let Method::LambertConic { lat1, lat2, .. }
                    | Method::Albers { lat1, lat2, .. } = &mut m
                    {
                        std::mem::swap(lat1, lat2);
                    }
                    *p = Projection::new(m, p.ellipsoid);
                }
                if let Some(k) = crs::identify(&swapped) {
                    return (Some(k), None);
                }
            }
            let found = (!custom)
                .then(|| {
                    datum_epsg(cs.datum)
                        .and_then(|b| search_projected(b, c.projection.as_ref()?, metres))
                })
                .flatten();
            (Some(found.unwrap_or(c)), None)
        }
    }
}

/// Parses a MIF `CoordSys` clause (after the keyword).
fn parse_coordsys(text: &str) -> Result<CoordSys, String> {
    let toks = tokens(text);
    let mut i;
    let word = |i: usize| -> String {
        match toks.get(i) {
            Some(Tok::Word(w)) => w.to_ascii_lowercase(),
            _ => String::new(),
        }
    };
    let mut cs = CoordSys::default();
    match word(0).as_str() {
        "nonearth" => {
            cs.non_earth = true;
            if word(1) == "units"
                && let Some(Tok::Str(u)) = toks.get(2)
            {
                cs.unit = Some(u.clone());
            }
            return Ok(cs);
        }
        "earth" => i = 1,
        "layout" => {
            cs.non_earth = true;
            return Ok(cs);
        }
        w => return Err(format!("unknown MapInfo CoordSys {w:?}")),
    }
    if word(i) != "projection" {
        return Err("a MapInfo CoordSys without a projection".into());
    }
    i += 1;
    // Comma-separated values up to Bounds/Affine.
    let mut vals: Vec<Tok> = Vec::new();
    while let Some(t) = toks.get(i) {
        match t {
            Tok::Word(w)
                if matches!(
                    w.to_ascii_lowercase().as_str(),
                    "bounds" | "affine" | "units"
                ) =>
            {
                break;
            }
            Tok::Punct(',') => {}
            t => vals.push(t.clone()),
        }
        i += 1;
    }
    let num = |t: Option<&Tok>| -> Option<f64> {
        match t? {
            Tok::Num(v) => Some(*v),
            _ => None,
        }
    };
    let mut k = 0usize;
    let p = num(vals.get(k)).ok_or("a MapInfo CoordSys without a projection number")? as u32;
    k += 1;
    cs.proj = p % 1000;
    if p >= 1000 {
        // Affine flag: the clause names its units after the projection; ignored.
    }
    cs.datum = num(vals.get(k)).unwrap_or(0.0) as u32;
    k += 1;
    if cs.datum == 999 || cs.datum == 9999 {
        cs.ellipsoid = num(vals.get(k)).map(|v| v as u32);
        k += 1;
        let n = if cs.datum == 999 { 3 } else { 8 };
        for _ in 0..n {
            if let Some(v) = num(vals.get(k)) {
                cs.shift.push(v);
                k += 1;
            }
        }
    }
    if cs.proj != 1
        && let Some(Tok::Str(u)) = vals.get(k)
    {
        cs.unit = Some(u.clone());
        k += 1;
    }
    while let Some(v) = num(vals.get(k)) {
        cs.params.push(v);
        k += 1;
    }
    Ok(cs)
}

// ---------------------------------------------------------------------------- tokens

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Word(String),
    Num(f64),
    Str(String),
    Punct(char),
}

fn tokens(text: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let b: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '"' {
            let mut s = String::new();
            i += 1;
            while i < b.len() {
                if b[i] == '"' {
                    if b.get(i + 1) == Some(&'"') {
                        s.push('"');
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                s.push(b[i]);
                i += 1;
            }
            out.push(Tok::Str(s));
        } else if matches!(c, '(' | ')' | ',') {
            out.push(Tok::Punct(c));
            i += 1;
        } else {
            let st = i;
            while i < b.len() && !b[i].is_whitespace() && !matches!(b[i], '(' | ')' | ',' | '"') {
                i += 1;
            }
            let w: String = b[st..i].iter().collect();
            match w.parse::<f64>() {
                Ok(v)
                    if w.starts_with(|c: char| {
                        c.is_ascii_digit() || matches!(c, '-' | '+' | '.')
                    }) =>
                {
                    out.push(Tok::Num(v));
                }
                _ => out.push(Tok::Word(w)),
            }
        }
    }
    out
}

// --------------------------------------------------------------------------- stroking

/// Points along an elliptical arc as GDAL generates them: `n` evenly spaced from `a0` to
/// `a1` (radians), then the end point once more.
fn gen_arc(out: &mut Vec<Pt>, c: Pt, rx: f64, ry: f64, a0: f64, a1: f64, n: usize) {
    let n = n.max(2);
    for k in 0..n {
        let a = a0 + (a1 - a0) * k as f64 / (n - 1) as f64;
        out.push([c[0] + rx * a.cos(), c[1] + ry * a.sin()]);
    }
    out.push([c[0] + rx * a1.cos(), c[1] + ry * a1.sin()]);
}

fn close(mut r: Vec<Pt>) -> Vec<Pt> {
    if let (Some(&f), Some(&l)) = (r.first(), r.last())
        && f != l
    {
        r.push(f);
    }
    r
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Geometry {
    let (a, b) = (x0.min(x1), x0.max(x1));
    let (c, d) = (y0.min(y1), y0.max(y1));
    Geometry::Polygon(vec![vec![[a, c], [b, c], [b, d], [a, d], [a, c]]])
}

fn ellipse(x0: f64, y0: f64, x1: f64, y1: f64) -> Geometry {
    let c = [(x0 + x1) / 2.0, (y0 + y1) / 2.0];
    let (rx, ry) = ((x1 - x0).abs() / 2.0, (y1 - y0).abs() / 2.0);
    let mut r = Vec::with_capacity(182);
    gen_arc(&mut r, c, rx, ry, 0.0, std::f64::consts::TAU, 180);
    Geometry::Polygon(vec![close(r)])
}

/// A rounded rectangle with corner radii `rx`, `ry` (clamped to half the sides).
fn round_rect(x0: f64, y0: f64, x1: f64, y1: f64, rx: f64, ry: f64) -> Geometry {
    let (a, b) = (x0.min(x1), x0.max(x1));
    let (c, d) = (y0.min(y1), y0.max(y1));
    let rx = rx.abs().min((b - a) / 2.0);
    let ry = ry.abs().min((d - c) / 2.0);
    let pi = std::f64::consts::PI;
    let mut r = Vec::with_capacity(185);
    gen_arc(&mut r, [a + rx, c + ry], rx, ry, pi, 1.5 * pi, 45);
    gen_arc(&mut r, [b - rx, c + ry], rx, ry, 1.5 * pi, 2.0 * pi, 45);
    gen_arc(&mut r, [b - rx, d - ry], rx, ry, 0.0, 0.5 * pi, 45);
    gen_arc(&mut r, [a + rx, d - ry], rx, ry, 0.5 * pi, pi, 45);
    Geometry::Polygon(vec![close(r)])
}

/// An arc of the ellipse inscribed in a box, from `start` to `end` degrees
/// (counter-clockwise), in 2° steps.
fn arc(x0: f64, y0: f64, x1: f64, y1: f64, start: f64, end: f64) -> Geometry {
    let c = [(x0 + x1) / 2.0, (y0 + y1) / 2.0];
    let (rx, ry) = ((x1 - x0).abs() / 2.0, (y1 - y0).abs() / 2.0);
    // Angles from the file are brought into one turn: a sweep of 1e15° (or NaN) would
    // otherwise ask for an absurd number of points.
    let turn = |a: f64| {
        if a.is_finite() {
            a.rem_euclid(360.0)
        } else {
            0.0
        }
    };
    let full = end - start >= 360.0;
    let (start, mut end) = (turn(start), turn(end));
    if end < start || (full && end == start) {
        end += 360.0;
    }
    let n = (((end - start).abs() / 2.0) as usize + 1).max(2);
    let mut l = Vec::with_capacity(n + 1);
    gen_arc(&mut l, c, rx, ry, start.to_radians(), end.to_radians(), n);
    Geometry::LineString(l)
}

/// The lower-left corner of a text whose rotated box has this MBR, height and angle.
fn text_point(mbr: [f64; 4], h: f64, angle: f64) -> Pt {
    let a = angle.rem_euclid(360.0);
    let (s, c) = a.to_radians().sin_cos();
    let [x0, y0, x1, y1] = mbr;
    if a < 90.0 {
        [x0 + h * s, y0]
    } else if a < 180.0 {
        [x1, y0 - h * c]
    } else if a < 270.0 {
        [x1 + h * s, y1]
    } else {
        [x0, y1 - h * c]
    }
}

/// Region rings as one polygon, or with hole counts (V450 and later) as polygons each
/// followed by its holes.
fn region(rings: Vec<Vec<Pt>>, holes: Option<Vec<usize>>) -> Option<Geometry> {
    let rings: Vec<(Vec<Pt>, usize)> = match holes {
        Some(h) => rings.into_iter().zip(h).collect(),
        None => rings.into_iter().map(|r| (r, 0)).collect(),
    };
    if rings.is_empty() {
        return None;
    }
    let grouped = rings.iter().any(|(_, h)| *h > 0);
    if !grouped {
        return Some(Geometry::Polygon(
            rings.into_iter().map(|(r, _)| r).collect(),
        ));
    }
    let mut polys: Vec<Vec<Vec<Pt>>> = Vec::new();
    let mut it = rings.into_iter();
    while let Some((outer, h)) = it.next() {
        let mut p = vec![outer];
        for _ in 0..h {
            match it.next() {
                Some((r, _)) => p.push(r),
                None => break,
            }
        }
        polys.push(p);
    }
    Some(if polys.len() == 1 {
        Geometry::Polygon(polys.remove(0))
    } else {
        Geometry::MultiPolygon(polys)
    })
}

fn lines(mut parts: Vec<Vec<Pt>>) -> Geometry {
    if parts.len() == 1 {
        Geometry::LineString(parts.remove(0))
    } else {
        Geometry::MultiLineString(parts)
    }
}

// ------------------------------------------------------------------------- fields

#[derive(Clone, Copy, Debug, PartialEq)]
enum FieldType {
    Char,
    Integer,
    SmallInt,
    LargeInt,
    Decimal,
    Float,
    Date,
    Time,
    DateTime,
    Logical,
}

fn field_type(name: &str) -> Option<FieldType> {
    Some(match name.to_ascii_lowercase().as_str() {
        "char" => FieldType::Char,
        "integer" => FieldType::Integer,
        "smallint" => FieldType::SmallInt,
        "largeint" => FieldType::LargeInt,
        "decimal" => FieldType::Decimal,
        "float" => FieldType::Float,
        "date" => FieldType::Date,
        "time" => FieldType::Time,
        "datetime" => FieldType::DateTime,
        "logical" => FieldType::Logical,
        _ => return None,
    })
}

/// "name Type [(w[, d])]" column definitions from tokens, `n` of them.
fn columns(lines: &[&str]) -> Result<Vec<(String, FieldType)>, String> {
    lines
        .iter()
        .map(|l| {
            let t = tokens(l);
            let name = match t.first() {
                Some(Tok::Word(w) | Tok::Str(w)) => w.clone(),
                Some(Tok::Num(v)) => v.to_string(),
                _ => return Err(format!("a MapInfo column is malformed: {l:?}")),
            };
            let ty = match t.get(1) {
                Some(Tok::Word(w)) => field_type(w),
                _ => None,
            }
            .ok_or_else(|| format!("a MapInfo column type is not known: {l:?}"))?;
            Ok((name, ty))
        })
        .collect()
}

fn date_text(y: u32, m: u32, d: u32) -> Option<String> {
    (y > 0 && (1..=12).contains(&m) && (1..=31).contains(&d))
        .then(|| format!("{y:04}-{m:02}-{d:02}"))
}

fn time_text(ms: u32) -> Option<String> {
    if ms >= 86_400_000 {
        return None;
    }
    let (h, m, s, f) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000);
    Some(if f == 0 {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        format!("{h:02}:{m:02}:{s:02}.{f:03}")
    })
}

/// A value in its MIF/MID text form.
fn text_value(s: &str, ty: FieldType) -> Value {
    let t = s.trim();
    match ty {
        FieldType::Char => Value::Text(s.trim_end().to_string()),
        FieldType::Integer | FieldType::SmallInt | FieldType::LargeInt => {
            if t.is_empty() {
                Value::Int(0)
            } else {
                t.parse::<i64>()
                    .map(Value::Int)
                    .or_else(|_| t.parse::<f64>().map(|f| Value::Int(f as i64)))
                    .unwrap_or_else(|_| Value::Text(t.to_string()))
            }
        }
        FieldType::Decimal | FieldType::Float => {
            if t.is_empty() {
                Value::Float(0.0)
            } else {
                t.parse::<f64>()
                    .map_or_else(|_| Value::Text(t.to_string()), Value::Float)
            }
        }
        FieldType::Date => {
            let d = t.replace(['/', '-'], "");
            let ok = d.len() == 8 && d.bytes().all(|b| b.is_ascii_digit());
            ok.then(|| {
                date_text(
                    d[..4].parse().unwrap_or(0),
                    d[4..6].parse().unwrap_or(0),
                    d[6..8].parse().unwrap_or(0),
                )
            })
            .flatten()
            .map_or(Value::Null, Value::Date)
        }
        FieldType::Time => mid_time(t).map_or(Value::Null, Value::Text),
        FieldType::DateTime => {
            if t.len() >= 8 && t.bytes().all(|b| b.is_ascii_digit()) {
                let d = date_text(
                    t[..4].parse().unwrap_or(0),
                    t[4..6].parse().unwrap_or(0),
                    t[6..8].parse().unwrap_or(0),
                );
                let tm = mid_time(&t[8..]).unwrap_or_else(|| "00:00:00".into());
                d.map_or(Value::Null, |d| Value::Date(format!("{d}T{tm}")))
            } else {
                Value::Null
            }
        }
        FieldType::Logical => Value::Bool(matches!(
            t.chars().next(),
            Some('T' | 't' | 'Y' | 'y' | '1')
        )),
    }
}

/// "hhmmss" or "hhmmssmmm".
fn mid_time(t: &str) -> Option<String> {
    if t.len() < 6 || !t.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let h: u32 = t[..2].parse().ok()?;
    let m: u32 = t[2..4].parse().ok()?;
    let s: u32 = t[4..6].parse().ok()?;
    let f: u32 = t.get(6..9).and_then(|x| x.parse().ok()).unwrap_or(0);
    time_text(((h * 60 + m) * 60 + s) * 1000 + f)
}

// ---------------------------------------------------------------------------- reader

/// A bounds-checked little-endian reader.
struct R<'a> {
    b: &'a [u8],
    i: usize,
}

const TRUNC: &str = "a MapInfo object is truncated";

impl R<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let s = self
            .b
            .get(self.i..self.i + N)
            .ok_or(TRUNC)?
            .try_into()
            .map_err(|_| TRUNC)?;
        self.i += N;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take::<1>()?[0])
    }
    fn i16(&mut self) -> Result<i64, String> {
        Ok(i64::from(i16::from_le_bytes(self.take()?)))
    }
    fn i32(&mut self) -> Result<i64, String> {
        Ok(i64::from(i32::from_le_bytes(self.take()?)))
    }
    fn skip(&mut self, n: usize) -> Result<(), String> {
        if self.i + n > self.b.len() {
            return Err(TRUNC.into());
        }
        self.i += n;
        Ok(())
    }
}

/// The `.map` file: its header transform and its blocks.
struct Map<'a> {
    data: &'a [u8],
    block: usize,
    scale: [f64; 2],
    displ: [f64; 2],
    quadrant: u8,
    /// Coordinates are rounded to the decimals the scale implies (10^round(log10 scale)),
    /// as GDAL does.
    precision: [f64; 2],
    coordsys: CoordSys,
}

/// Reads coordinate data that may run on into the next coordinate block.
struct Coords<'a, 'b> {
    map: &'b Map<'a>,
    pos: usize,
    end: usize,
    hops: usize,
}

impl<'a, 'b> Coords<'a, 'b> {
    fn new(map: &'b Map<'a>, ptr: i64) -> Result<Self, String> {
        let pos = usize::try_from(ptr).map_err(|_| "a MapInfo coordinate pointer is invalid")?;
        let start = pos - pos % map.block;
        let used = map.u16_at(start + 2).ok_or(TRUNC)?;
        Ok(Coords {
            map,
            pos,
            end: (start + 8 + used).min(map.data.len()),
            hops: 0,
        })
    }

    fn byte(&mut self) -> Result<u8, String> {
        if self.pos >= self.end {
            let start = self.end.saturating_sub(1) - (self.end.saturating_sub(1)) % self.map.block;
            let next = self.map.i32_at(start + 4).ok_or(TRUNC)?;
            self.hops += 1;
            if next <= 0 || self.hops > 1 << 22 {
                return Err("MapInfo coordinate data runs past its blocks".into());
            }
            let n = next as usize;
            if !n.is_multiple_of(self.map.block) || n + 8 > self.map.data.len() {
                return Err("a MapInfo coordinate block link is invalid".into());
            }
            let used = self.map.u16_at(n + 2).ok_or(TRUNC)?;
            self.pos = n + 8;
            self.end = (n + 8 + used).min(self.map.data.len());
            if self.pos >= self.end {
                return Err("an empty MapInfo coordinate block".into());
            }
        }
        let v = *self.map.data.get(self.pos).ok_or(TRUNC)?;
        self.pos += 1;
        Ok(v)
    }
    fn bytes<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let mut out = [0u8; N];
        for b in &mut out {
            *b = self.byte()?;
        }
        Ok(out)
    }
    fn i16(&mut self) -> Result<i64, String> {
        Ok(i64::from(i16::from_le_bytes(self.bytes()?)))
    }
    fn i32(&mut self) -> Result<i64, String> {
        Ok(i64::from(i32::from_le_bytes(self.bytes()?)))
    }
    fn coord(&mut self, compressed: bool, org: (i64, i64)) -> Result<(i64, i64), String> {
        Ok(if compressed {
            (org.0 + self.i16()?, org.1 + self.i16()?)
        } else {
            (self.i32()?, self.i32()?)
        })
    }
}

/// How the section headers of a multi-section object are laid out.
#[derive(Clone, Copy)]
struct SectionFormat {
    version: u32,
    compressed: bool,
    /// In a collection (V650) the hole count is 16 bits.
    collection: bool,
    /// The compressed coordinate origin.
    org: (i64, i64),
}

struct Section {
    vertices: usize,
    holes: usize,
}

impl<'a> Map<'a> {
    fn open(data: &'a [u8]) -> Result<Map<'a>, String> {
        let mut r = R { b: data, i: 0x100 };
        if data.len() < 0x200 || r.i32()? != 42_424_242 {
            return Err("the .map file is not a MapInfo map".into());
        }
        let _version = r.i16()?;
        let block = r.i16()?;
        let block = usize::try_from(block)
            .ok()
            .filter(|&b| b >= 512)
            .unwrap_or(512);
        let f = |o: usize| -> f64 {
            data.get(o..o + 8).map_or(0.0, |s| {
                f64::from_le_bytes(s.try_into().unwrap_or_default())
            })
        };
        let quadrant = data[0x161];
        let datum = u16::from_le_bytes([data[0x16A], data[0x16B]]);
        let proj = data[0x16D];
        let ell = data[0x16E];
        let units = data[0x16F];
        let params: Vec<f64> = (0..6).map(|k| f(0x190 + 8 * k)).collect();
        let shift: Vec<f64> = (0..8).map(|k| f(0x1C0 + 8 * k)).collect();
        let coordsys = CoordSys {
            non_earth: proj == 0,
            proj: u32::from(proj),
            datum: u32::from(datum),
            ellipsoid: Some(u32::from(ell)),
            shift: if datum >= 999 { shift } else { Vec::new() },
            unit: Some(units.to_string()),
            params,
        };
        let scale = [f(0x170), f(0x178)];
        if !scale.iter().all(|s| s.is_finite() && *s != 0.0) {
            return Err("the .map file has no coordinate scale".into());
        }
        let precision = scale.map(|s| 10f64.powf(s.abs().log10().round()));
        Ok(Map {
            data,
            block,
            precision,
            scale,
            displ: [f(0x180), f(0x188)],
            quadrant,
            coordsys,
        })
    }

    fn u16_at(&self, o: usize) -> Option<usize> {
        Some(usize::from(u16::from_le_bytes(
            self.data.get(o..o + 2)?.try_into().ok()?,
        )))
    }
    fn i32_at(&self, o: usize) -> Option<i64> {
        Some(i64::from(i32::from_le_bytes(
            self.data.get(o..o + 4)?.try_into().ok()?,
        )))
    }

    fn world(&self, (x, y): (i64, i64)) -> Pt {
        let (x, y) = (x as f64, y as f64);
        let wx = if matches!(self.quadrant, 2 | 3) {
            -(x + self.displ[0]) / self.scale[0]
        } else {
            (x - self.displ[0]) / self.scale[0]
        };
        let wy = if matches!(self.quadrant, 3 | 4) {
            -(y + self.displ[1]) / self.scale[1]
        } else {
            (y - self.displ[1]) / self.scale[1]
        };
        let round = |v: f64, p: f64| {
            if p > 0.0 && p.is_finite() {
                (v * p).round() / p
            } else {
                v
            }
        };
        [round(wx, self.precision[0]), round(wy, self.precision[1])]
    }

    fn dist(&self, d: i64, axis: usize) -> f64 {
        d as f64 / self.scale[axis]
    }

    /// Arc angles in the world frame (the quadrant may mirror the axes).
    fn angles(&self, a0: f64, a1: f64) -> (f64, f64) {
        match self.quadrant {
            2 => (180.0 - a1, 180.0 - a0),
            4 => (-a1, -a0),
            3 => (a0 + 180.0, a1 + 180.0),
            _ => (a0, a1),
        }
    }

    /// Section headers then vertices of a multi-section object.
    fn sections(
        &self,
        c: &mut Coords<'_, '_>,
        n: usize,
        f: SectionFormat,
        budget: &mut usize,
    ) -> Result<Vec<(Vec<Pt>, usize)>, String> {
        let SectionFormat {
            version,
            compressed,
            collection,
            org,
        } = f;
        if n > 1 << 20 {
            return Err("a MapInfo object has too many sections".into());
        }
        let mut hdrs = Vec::with_capacity(n);
        for _ in 0..n {
            let (vertices, holes) = if version >= 450 {
                let v = c.i32()?;
                let h = if collection && version < 800 {
                    c.i16()?
                } else {
                    c.i32()?
                };
                (v, h)
            } else {
                (c.i16()?, c.i16()?)
            };
            c.coord(compressed, org)?;
            c.coord(compressed, org)?;
            c.i32()?; // data offset
            let vertices = usize::try_from(vertices).map_err(|_| "a negative vertex count")?;
            hdrs.push(Section {
                vertices,
                holes: usize::try_from(holes).unwrap_or(0),
            });
        }
        let mut out = Vec::with_capacity(n);
        for h in hdrs {
            if h.vertices > *budget {
                return Err("too many vertices".into());
            }
            *budget -= h.vertices;
            let mut pts = Vec::with_capacity(h.vertices.min(1 << 20));
            for _ in 0..h.vertices {
                pts.push(self.world(c.coord(compressed, org)?));
            }
            out.push((pts, h.holes));
        }
        Ok(out)
    }

    /// The object at `off`: its geometry and, for text, its string.
    fn object(
        &self,
        off: usize,
        enc: Option<&'static Encoding>,
        budget: &mut usize,
    ) -> Result<Option<(Geometry, Option<String>)>, String> {
        let block = off - off % self.block;
        let center = (
            self.i32_at(block + 4).ok_or(TRUNC)?,
            self.i32_at(block + 8).ok_or(TRUNC)?,
        );
        let mut r = R {
            b: self.data,
            i: off,
        };
        let ty = r.u8()?;
        if ty == 0 {
            return Ok(None);
        }
        let id = r.i32()?;
        if id & 0x4000_0000 != 0 {
            return Ok(None); // deleted
        }
        // Compressed types have odd numbers before 0x2e and even after… use the table.
        let compressed = matches!(
            ty,
            0x01 | 0x04
                | 0x07
                | 0x0a
                | 0x0d
                | 0x10
                | 0x13
                | 0x16
                | 0x19
                | 0x25
                | 0x28
                | 0x2b
                | 0x2e
                | 0x31
                | 0x34
                | 0x37
                | 0x3a
                | 0x3d
                | 0x40
                | 0x43
                | 0x46
        );
        let coord = |r: &mut R<'_>| -> Result<(i64, i64), String> {
            Ok(if compressed {
                (center.0 + r.i16()?, center.1 + r.i16()?)
            } else {
                (r.i32()?, r.i32()?)
            })
        };
        let pt = |r: &mut R<'_>| -> Result<Pt, String> { Ok(self.world(coord(r)?)) };
        let g = match ty {
            // Symbols.
            0x01 | 0x02 => Geometry::Point(pt(&mut r)?),
            0x28 | 0x29 => {
                r.skip(1 + 1 + 2 + 3 + 3 + 2)?;
                Geometry::Point(pt(&mut r)?)
            }
            0x2b | 0x2c => {
                r.skip(2)?;
                Geometry::Point(pt(&mut r)?)
            }
            // Line.
            0x04 | 0x05 => {
                let a = pt(&mut r)?;
                let b = pt(&mut r)?;
                Geometry::LineString(vec![a, b])
            }
            // Polylines, multiple polylines, regions.
            0x07 | 0x08 | 0x0d | 0x0e | 0x25 | 0x26 | 0x2e | 0x2f | 0x31 | 0x32 | 0x3d | 0x3e
            | 0x40 | 0x41 => {
                let ptr = r.i32()?;
                let size = (r.i32()? as u32 & 0x7FFF_FFFF) as usize;
                let v800 = matches!(ty, 0x3d | 0x3e | 0x40 | 0x41);
                let single = matches!(ty, 0x07 | 0x08);
                let nsec = if single {
                    1
                } else if v800 {
                    let n = r.i32()?;
                    r.skip(33)?;
                    n
                } else {
                    r.i16()?
                };
                let org = if compressed {
                    r.skip(4)?; // label
                    let o = (r.i32()?, r.i32()?);
                    r.skip(8)?;
                    o
                } else {
                    r.skip(8 + 16)?;
                    (0, 0)
                };
                let mut c = Coords::new(self, ptr)?;
                let region = matches!(ty, 0x0d | 0x0e | 0x2e | 0x2f | 0x3d | 0x3e);
                if single {
                    let n = size / if compressed { 4 } else { 8 };
                    if n > *budget {
                        return Err("too many vertices".into());
                    }
                    *budget -= n;
                    let mut l = Vec::with_capacity(n.min(1 << 20));
                    for _ in 0..n {
                        l.push(self.world(c.coord(compressed, org)?));
                    }
                    Geometry::LineString(l)
                } else {
                    let version = if v800 {
                        800
                    } else if matches!(ty, 0x2e | 0x2f | 0x31 | 0x32) {
                        450
                    } else {
                        300
                    };
                    let n = usize::try_from(nsec).map_err(|_| "a negative section count")?;
                    let secs = self.sections(
                        &mut c,
                        n,
                        SectionFormat {
                            version,
                            compressed,
                            collection: false,
                            org,
                        },
                        budget,
                    )?;
                    if region {
                        let holes = (version >= 450).then(|| secs.iter().map(|s| s.1).collect());
                        match region_of(secs, holes) {
                            Some(g) => g,
                            None => return Ok(None),
                        }
                    } else {
                        lines(secs.into_iter().map(|s| s.0).collect())
                    }
                }
            }
            // Arc.
            0x0a | 0x0b => {
                let a0 = r.i16()? as f64 / 10.0;
                let a1 = r.i16()? as f64 / 10.0;
                let p0 = pt(&mut r)?;
                let p1 = pt(&mut r)?;
                let (a0, a1) = self.angles(a0, a1);
                arc(p0[0], p0[1], p1[0], p1[1], a0, a1)
            }
            // Text.
            0x10 | 0x11 => {
                let ptr = r.i32()?;
                let len = r.i16()?.max(0) as usize;
                r.skip(2)?; // alignment
                let angle = r.i16()? as f64 / 10.0;
                r.skip(2 + 3 + 3)?;
                coord(&mut r)?; // label line end
                let h = if compressed { r.i16()? } else { r.i32()? };
                r.skip(1)?;
                let p0 = pt(&mut r)?;
                let p1 = pt(&mut r)?;
                let mut s = Vec::with_capacity(len.min(1 << 16));
                if len > 0 {
                    let mut c = Coords::new(self, ptr)?;
                    for _ in 0..len {
                        s.push(c.byte()?);
                    }
                }
                let mbr = [
                    p0[0].min(p1[0]),
                    p0[1].min(p1[1]),
                    p0[0].max(p1[0]),
                    p0[1].max(p1[1]),
                ];
                let p = text_point(mbr, self.dist(h, 1), angle);
                return Ok(Some((Geometry::Point(p), Some(decode(&s, enc)))));
            }
            // Rectangle, rounded rectangle, ellipse.
            0x13 | 0x14 | 0x19 | 0x1a => {
                let p0 = pt(&mut r)?;
                let p1 = pt(&mut r)?;
                if matches!(ty, 0x13 | 0x14) {
                    rect(p0[0], p0[1], p1[0], p1[1])
                } else {
                    ellipse(p0[0], p0[1], p1[0], p1[1])
                }
            }
            0x16 | 0x17 => {
                let (w, h) = if compressed {
                    (r.i16()?, r.i16()?)
                } else {
                    (r.i32()?, r.i32()?)
                };
                let p0 = pt(&mut r)?;
                let p1 = pt(&mut r)?;
                round_rect(
                    p0[0],
                    p0[1],
                    p1[0],
                    p1[1],
                    self.dist(w, 0) / 2.0,
                    self.dist(h, 1) / 2.0,
                )
            }
            // Multipoint.
            0x34 | 0x35 | 0x43 | 0x44 => {
                let ptr = r.i32()?;
                let n = usize::try_from(r.i32()?).map_err(|_| "a negative point count")?;
                if matches!(ty, 0x43 | 0x44) {
                    r.skip(33)?;
                }
                r.skip(15 + 1 + 1)?;
                let org = if compressed {
                    r.skip(4)?;
                    (r.i32()?, r.i32()?)
                } else {
                    (0, 0)
                };
                if n > *budget {
                    return Err("too many vertices".into());
                }
                *budget -= n;
                let mut c = Coords::new(self, ptr)?;
                let mut v = Vec::with_capacity(n.min(1 << 20));
                for _ in 0..n {
                    v.push(self.world(c.coord(compressed, org)?));
                }
                Geometry::MultiPoint(v)
            }
            // Collection.
            0x37 | 0x38 | 0x46 | 0x47 => self.collection(&mut r, ty, compressed, budget)?,
            t => return Err(format!("MapInfo object type {t:#x} is not supported")),
        };
        Ok(Some((g, None)))
    }

    fn collection(
        &self,
        r: &mut R<'_>,
        ty: u8,
        compressed: bool,
        budget: &mut usize,
    ) -> Result<Geometry, String> {
        let v800 = matches!(ty, 0x46 | 0x47);
        let ptr = r.i32()?;
        let npts = usize::try_from(r.i32()?).map_err(|_| "a negative point count")?;
        let reg_size = usize::try_from(r.i32()?).map_err(|_| "a bad region size")?;
        let pl_size = usize::try_from(r.i32()?).map_err(|_| "a bad polyline size")?;
        let (nreg, npl) = if v800 {
            (r.i32()?, r.i32()?)
        } else {
            (r.i16()?, r.i16()?)
        };
        let nreg = usize::try_from(nreg).map_err(|_| "a negative section count")?;
        let npl = usize::try_from(npl).map_err(|_| "a negative section count")?;
        r.skip(20)?;
        let org = if compressed {
            (r.i32()?, r.i32()?)
        } else {
            (0, 0)
        };
        let version = if v800 { 800 } else { 650 };
        let mini = if compressed { 12 } else { 24 } + if v800 { 4 } else { 0 };
        let mut parts = Vec::new();
        let mut c = Coords::new(self, ptr)?;
        let skip = |c: &mut Coords<'_, '_>, n: usize| -> Result<(), String> {
            for _ in 0..n {
                c.byte()?;
            }
            Ok(())
        };
        let _ = (reg_size, pl_size);
        if nreg > 0 {
            skip(&mut c, mini)?;
            let secs = self.sections(
                &mut c,
                nreg,
                SectionFormat {
                    version,
                    compressed,
                    collection: true,
                    org,
                },
                budget,
            )?;
            let holes = Some(secs.iter().map(|s| s.1).collect());
            if let Some(g) = region_of(secs, holes) {
                parts.push(g);
            }
        }
        if npl > 0 {
            skip(&mut c, mini)?;
            let secs = self.sections(
                &mut c,
                npl,
                SectionFormat {
                    version,
                    compressed,
                    collection: true,
                    org,
                },
                budget,
            )?;
            parts.push(lines(secs.into_iter().map(|s| s.0).collect()));
        }
        if npts > 0 {
            skip(&mut c, mini)?;
            if npts > *budget {
                return Err("too many vertices".into());
            }
            *budget -= npts;
            let mut v = Vec::with_capacity(npts.min(1 << 20));
            for _ in 0..npts {
                v.push(self.world(c.coord(compressed, org)?));
            }
            parts.push(Geometry::MultiPoint(v));
        }
        Ok(Geometry::Collection(parts))
    }
}

fn region_of(secs: Vec<(Vec<Pt>, usize)>, holes: Option<Vec<usize>>) -> Option<Geometry> {
    region(secs.into_iter().map(|s| s.0).collect(), holes)
}

// ------------------------------------------------------------------------------ .dat

/// A native `.dat` table (dBASE layout, MapInfo binary values).
struct Dat<'a> {
    data: &'a [u8],
    records: usize,
    header: usize,
    reclen: usize,
    /// Offset and width of each field within a record.
    fields: Vec<(usize, usize)>,
}

impl<'a> Dat<'a> {
    fn open(data: &'a [u8]) -> Result<Dat<'a>, String> {
        if data.len() < 32 {
            return Err("the .dat file is too short".into());
        }
        let records = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
        let header = usize::from(u16::from_le_bytes([data[8], data[9]]));
        let reclen = usize::from(u16::from_le_bytes([data[10], data[11]]));
        let mut fields = Vec::new();
        let mut o = 32;
        let mut off = 1;
        while o + 32 <= data.len().min(header.max(33)) && data[o] != 0x0D {
            let w = usize::from(data[o + 16]);
            fields.push((off, w));
            off += w;
            o += 32;
        }
        let reclen = if reclen == 0 { off } else { reclen };
        let avail = data.len().saturating_sub(header) / reclen.max(1);
        Ok(Dat {
            data,
            records: records.min(avail),
            header,
            reclen,
            fields,
        })
    }

    /// Row `i`'s raw field bytes, `None` when deleted.
    fn row(&self, i: usize) -> Option<Vec<&'a [u8]>> {
        let o = self.header + i * self.reclen;
        let r = self.data.get(o..o + self.reclen)?;
        if r[0] == b'*' {
            return None;
        }
        Some(
            self.fields
                .iter()
                .map(|&(f, w)| r.get(f..f + w).unwrap_or(&[]))
                .collect(),
        )
    }
}

fn native_value(b: &[u8], ty: FieldType, enc: Option<&'static Encoding>) -> Value {
    let le = |n: usize| -> Option<u64> {
        let s = b.get(..n)?;
        Some(s.iter().rev().fold(0u64, |a, &x| (a << 8) | u64::from(x)))
    };
    match ty {
        FieldType::Char => {
            let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
            Value::Text(decode(&b[..end], enc).trim_end().to_string())
        }
        FieldType::Integer => le(4).map_or(Value::Null, |v| Value::Int(i64::from(v as u32 as i32))),
        FieldType::SmallInt => {
            le(2).map_or(Value::Null, |v| Value::Int(i64::from(v as u16 as i16)))
        }
        FieldType::LargeInt => le(8).map_or(Value::Null, |v| Value::Int(v as i64)),
        FieldType::Float => le(8).map_or(Value::Null, |v| Value::Float(f64::from_bits(v))),
        FieldType::Decimal => text_value(&decode(b, enc), FieldType::Decimal),
        FieldType::Date => le(4)
            .and_then(|_| {
                date_text(
                    u32::from(u16::from_le_bytes([b[0], b[1]])),
                    u32::from(b[2]),
                    u32::from(b[3]),
                )
            })
            .map_or(Value::Null, Value::Date),
        FieldType::Time => le(4)
            .and_then(|v| time_text(v as u32))
            .map_or(Value::Null, Value::Text),
        FieldType::DateTime => {
            let d = le(4).and_then(|_| {
                date_text(
                    u32::from(u16::from_le_bytes([b[0], b[1]])),
                    u32::from(b[2]),
                    u32::from(b[3]),
                )
            });
            let t = b
                .get(4..8)
                .and_then(|s| time_text(u32::from_le_bytes(s.try_into().unwrap_or_default())));
            match (d, t) {
                (Some(d), Some(t)) => Value::Date(format!("{d}T{t}")),
                (Some(d), None) => Value::Date(format!("{d}T00:00:00")),
                _ => Value::Null,
            }
        }
        FieldType::Logical => Value::Bool(matches!(b.first(), Some(b'T' | b't' | b'Y' | b'y' | 1))),
    }
}

// --------------------------------------------------------------------------- TAB

fn sidecar(path: &Path, ext: &str) -> Option<Vec<u8>> {
    std::fs::read(crate::sidecar::find(path, ext)?).ok()
}

/// What a `.tab` header declares.
struct TabHeader {
    kind: String,
    charset: Option<&'static Encoding>,
    fields: Vec<(String, FieldType)>,
}

fn parse_tab(text: &str) -> Result<TabHeader, String> {
    let lower = text.to_ascii_lowercase();
    if lower.contains("\"\\isseamless\" = \"true\"") {
        return Err(
            "a seamless MapInfo table (a list of other tables) is not supported; open its member tables".into(),
        );
    }
    if lower.contains("create view") {
        return Err(
            "a MapInfo view (a query over other tables) is not supported; open the tables it uses"
                .into(),
        );
    }
    let mut kind = None;
    let mut charset = None;
    let mut fields = Vec::new();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        let t = tokens(l);
        let w0 = match t.first() {
            Some(Tok::Word(w)) => w.to_ascii_lowercase(),
            _ => String::new(),
        };
        match w0.as_str() {
            "type" => {
                let k = match t.get(1) {
                    Some(Tok::Word(w) | Tok::Str(w)) => w.to_ascii_uppercase(),
                    _ => String::new(),
                };
                if let Some(Tok::Word(c)) = t.get(2)
                    && c.eq_ignore_ascii_case("charset")
                    && let Some(Tok::Str(s)) = t.get(3)
                {
                    charset = Some(s.clone());
                }
                kind = Some(k);
            }
            "!charset" => {
                if charset.is_none() {
                    charset = t.get(1).map(|t| match t {
                        Tok::Word(w) | Tok::Str(w) => w.clone(),
                        _ => String::new(),
                    });
                }
            }
            "fields" => {
                let n = match t.get(1) {
                    Some(Tok::Num(v)) => *v as usize,
                    _ => 0,
                };
                let end = (i + 1 + n).min(lines.len());
                fields = columns(&lines[i + 1..end])?;
                i = end;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    let kind = kind.ok_or("the MapInfo table declares no type")?;
    match kind.as_str() {
        "NATIVE" | "DBF" => {}
        "RASTER" => {
            return Err("a MapInfo raster registration, not a vector table".into());
        }
        "LINKED" | "ODBC" => {
            return Err("a MapInfo table linked to a database is not supported; open the database's data directly".into());
        }
        "WFS" | "WMS" => {
            return Err(format!(
                "a MapInfo {kind} layer (a link to a web service) is not supported"
            ));
        }
        k => {
            return Err(format!(
                "a MapInfo table of type {k} (linked data) is not supported; open its source file directly"
            ));
        }
    }
    Ok(TabHeader {
        kind,
        charset: charset.as_deref().and_then(encoding),
        fields,
    })
}

fn new_dataset(
    format: &'static str,
    name: &str,
    fields: &[(String, FieldType)],
) -> (Dataset, Layer) {
    let mut l = Layer::new(name);
    for (f, _) in fields {
        l.field(f);
    }
    (
        Dataset {
            format,
            ..Dataset::default()
        },
        l,
    )
}

fn layer_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("layer")
        .to_string()
}

/// Reads a native MapInfo table: `data` is the `.tab`; the `.map`, `.id` and `.dat` (or
/// `.dbf`) are read beside `path`.
pub fn read_tab(path: &Path, data: &[u8]) -> Result<Dataset, String> {
    // The header is ASCII apart from field names in the table's charset.
    let probe = String::from_utf8_lossy(data);
    let charset = probe
        .lines()
        .find_map(|l| {
            let l = l.trim();
            l.to_ascii_lowercase()
                .starts_with("!charset")
                .then(|| l[8..].trim().to_string())
        })
        .and_then(|c| encoding(&c));
    let text = decode(data, charset);
    let h = parse_tab(&text)?;
    let enc = h.charset.or(charset);
    let name = layer_name(path);
    let (mut ds, mut layer) = new_dataset("MapInfo TAB", &name, &h.fields);

    // Attributes: a native .dat or a .dbf.
    enum Table<'a> {
        Native(Dat<'a>),
        Dbf(crate::shp::Dbf<'a>),
    }
    let dat_bytes = if h.kind == "DBF" {
        sidecar(path, "dbf")
    } else {
        sidecar(path, "dat")
    }
    .ok_or_else(|| {
        format!(
            "the attribute file (.{}) beside the table is missing",
            if h.kind == "DBF" { "dbf" } else { "dat" }
        )
    })?;
    let table = if h.kind == "DBF" {
        let label = enc.map(|e| e.name().to_string());
        Table::Dbf(crate::shp::Dbf::open(&dat_bytes, label.as_deref())?)
    } else {
        Table::Native(Dat::open(&dat_bytes)?)
    };
    let rows = match &table {
        Table::Native(d) => d.records,
        Table::Dbf(d) => d.records,
    };

    let map_bytes = sidecar(path, "map");
    let id_bytes = sidecar(path, "id");
    let map = match (&map_bytes, &id_bytes) {
        (Some(m), Some(_)) => Some(Map::open(m)?),
        _ => None,
    };
    if let Some(m) = &map {
        (ds.crs, ds.crs_note) = crs_of(&m.coordsys);
    } else {
        ds.crs_note = Some("no geometry (.map) beside the table".into());
    }
    let ids = id_bytes.unwrap_or_default();
    let mut budget = MAX_VERTICES;
    let mut errors = 0usize;
    let mut texts = false;
    for row in 0..rows {
        if layer.features.len() >= MAX_FEATURES {
            ds.warn("too many features; the rest are not shown");
            break;
        }
        let values: Vec<Value> = match &table {
            Table::Native(d) => match d.row(row) {
                None => continue,
                Some(raw) => h
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(k, (_, ty))| {
                        raw.get(k)
                            .map_or(Value::Null, |b| native_value(b, *ty, enc))
                    })
                    .collect(),
            },
            Table::Dbf(d) => match d.row(row) {
                None => continue,
                Some(v) => v,
            },
        };
        let mut attrs: Vec<(String, Value)> = h
            .fields
            .iter()
            .map(|(n, _)| n.clone())
            .zip(values)
            .collect();
        let mut geometry = None;
        if let Some(m) = &map {
            let off = ids.get(row * 4..row * 4 + 4).map_or(0, |b| {
                u32::from_le_bytes(b.try_into().unwrap_or_default()) as usize
            });
            if off > 0 {
                match m.object(off, enc, &mut budget) {
                    Ok(Some((g, text))) => {
                        geometry = Some(g);
                        if let Some(t) = text {
                            texts = true;
                            attrs.push(("text".into(), Value::Text(t)));
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        errors += 1;
                        ds.warn(e);
                        if budget == 0 || errors > 1000 {
                            ds.warn("too many damaged objects; the rest are not shown");
                            break;
                        }
                    }
                }
            }
        }
        layer.push(Some((row + 1).to_string()), geometry, attrs);
    }
    let _ = texts;
    ds.layers.push(layer);
    Ok(ds)
}

// ---------------------------------------------------------------------------- MIF

struct Mif {
    delimiter: char,
    charset: Option<&'static Encoding>,
    coordsys: Option<CoordSys>,
    /// Transform: x scale, y scale, x offset, y offset.
    transform: Option<[f64; 4]>,
    fields: Vec<(String, FieldType)>,
}

/// MID records: values split on the delimiter, one record per line; a quoted value may
/// hold the delimiter, line breaks and doubled quotes.
fn mid_records(text: &str, delim: char) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    let mut rec = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else if c != '\r' {
                cur.push(c);
            }
        } else if c == '"' {
            quoted = true;
        } else if c == '\n' {
            rec.push(std::mem::take(&mut cur));
            out.push(std::mem::take(&mut rec));
        } else if c == delim {
            rec.push(std::mem::take(&mut cur));
        } else if c != '\r' {
            cur.push(c);
        }
    }
    if !cur.is_empty() || !rec.is_empty() {
        rec.push(cur);
        out.push(rec);
    }
    out
}

/// Object keywords that start a MIF object.
fn is_object(w: &str) -> bool {
    matches!(
        w.to_ascii_lowercase().as_str(),
        "none"
            | "point"
            | "line"
            | "pline"
            | "region"
            | "arc"
            | "text"
            | "rect"
            | "roundrect"
            | "ellipse"
            | "multipoint"
            | "collection"
    )
}

struct MifParser {
    t: Vec<Tok>,
    i: usize,
    transform: Option<[f64; 4]>,
}

impl MifParser {
    fn num(&mut self) -> Result<f64, String> {
        match self.t.get(self.i) {
            Some(Tok::Num(v)) => {
                self.i += 1;
                Ok(*v)
            }
            Some(t) => Err(format!("a MIF object expects a number, found {t:?}")),
            None => Err("a MIF object is truncated".into()),
        }
    }
    fn count(&mut self) -> Result<usize, String> {
        let v = self.num()?;
        if !(0.0..=1e9).contains(&v) {
            return Err("a MIF count is out of range".into());
        }
        Ok(v as usize)
    }
    fn pt(&mut self) -> Result<Pt, String> {
        let x = self.num()?;
        let y = self.num()?;
        Ok(match self.transform {
            Some([sx, sy, ox, oy]) => [x * sx + ox, y * sy + oy],
            None => [x, y],
        })
    }
    fn pts(&mut self, n: usize, budget: &mut usize) -> Result<Vec<Pt>, String> {
        if n > *budget {
            return Err("too many vertices".into());
        }
        *budget -= n;
        let mut v = Vec::with_capacity(n.min(1 << 20));
        for _ in 0..n {
            v.push(self.pt()?);
        }
        Ok(v)
    }
    fn word(&self, k: usize) -> Option<String> {
        match self.t.get(self.i + k) {
            Some(Tok::Word(w)) => Some(w.to_ascii_lowercase()),
            _ => None,
        }
    }
    /// Skips style clauses up to the next object keyword (or the end); returns a text
    /// angle when an `Angle` clause was among them.
    fn skip_style(&mut self) -> Option<f64> {
        let mut angle = None;
        while let Some(t) = self.t.get(self.i) {
            match t {
                Tok::Word(w) if is_object(w) => break,
                Tok::Word(w) => {
                    let w = w.to_ascii_lowercase();
                    self.i += 1;
                    match w.as_str() {
                        "label" => {
                            // Label Line Simple|Arrow x y
                            for _ in 0..2 {
                                if matches!(self.t.get(self.i), Some(Tok::Word(_))) {
                                    self.i += 1;
                                }
                            }
                            for _ in 0..2 {
                                if matches!(self.t.get(self.i), Some(Tok::Num(_))) {
                                    self.i += 1;
                                }
                            }
                        }
                        "angle" => {
                            if let Some(Tok::Num(v)) = self.t.get(self.i) {
                                angle = Some(*v);
                                self.i += 1;
                            }
                        }
                        _ => {}
                    }
                }
                Tok::Punct('(') => {
                    let mut depth = 0usize;
                    while let Some(t) = self.t.get(self.i) {
                        self.i += 1;
                        match t {
                            Tok::Punct('(') => depth += 1,
                            Tok::Punct(')') => {
                                depth = depth.saturating_sub(1);
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => self.i += 1,
            }
        }
        angle
    }

    /// One object (the keyword at the cursor).
    fn object(
        &mut self,
        depth: usize,
        budget: &mut usize,
    ) -> Result<(Option<Geometry>, Option<String>), String> {
        let w = self.word(0).ok_or("a MIF object keyword is missing")?;
        self.i += 1;
        let g = match w.as_str() {
            "none" => None,
            "point" => Some(Geometry::Point(self.pt()?)),
            "line" => {
                let a = self.pt()?;
                let b = self.pt()?;
                Some(Geometry::LineString(vec![a, b]))
            }
            "pline" => {
                if self.word(0).as_deref() == Some("multiple") {
                    self.i += 1;
                    let n = self.count()?;
                    let mut parts = Vec::with_capacity(n.min(1 << 16));
                    for _ in 0..n {
                        let k = self.count()?;
                        parts.push(self.pts(k, budget)?);
                    }
                    Some(Geometry::MultiLineString(parts))
                } else {
                    let k = self.count()?;
                    Some(Geometry::LineString(self.pts(k, budget)?))
                }
            }
            "region" => {
                let n = self.count()?;
                let mut rings = Vec::with_capacity(n.min(1 << 16));
                for _ in 0..n {
                    let k = self.count()?;
                    rings.push(self.pts(k, budget)?);
                }
                region(rings, None)
            }
            "arc" => {
                let p0 = self.pt()?;
                let p1 = self.pt()?;
                let a0 = self.num()?;
                let a1 = self.num()?;
                Some(arc(p0[0], p0[1], p1[0], p1[1], a0, a1))
            }
            "rect" => {
                let p0 = self.pt()?;
                let p1 = self.pt()?;
                Some(rect(p0[0], p0[1], p1[0], p1[1]))
            }
            "roundrect" => {
                let p0 = self.pt()?;
                let p1 = self.pt()?;
                let d = self.num()?;
                Some(round_rect(p0[0], p0[1], p1[0], p1[1], d / 2.0, d / 2.0))
            }
            "ellipse" => {
                let p0 = self.pt()?;
                let p1 = self.pt()?;
                Some(ellipse(p0[0], p0[1], p1[0], p1[1]))
            }
            "multipoint" => {
                let n = self.count()?;
                Some(Geometry::MultiPoint(self.pts(n, budget)?))
            }
            "text" => {
                let s = match self.t.get(self.i) {
                    Some(Tok::Str(s)) => {
                        self.i += 1;
                        s.clone()
                    }
                    _ => String::new(),
                };
                let p0 = self.pt()?;
                let p1 = self.pt()?;
                let save = self.i;
                let angle = self.skip_style().unwrap_or(0.0);
                self.i = save;
                let mbr = [
                    p0[0].min(p1[0]),
                    p0[1].min(p1[1]),
                    p0[0].max(p1[0]),
                    p0[1].max(p1[1]),
                ];
                let p = text_point(mbr, mbr[3] - mbr[1], angle);
                return Ok((Some(Geometry::Point(p)), Some(s)));
            }
            "collection" => {
                if depth > 0 {
                    return Err("a MIF collection inside a collection".into());
                }
                let n = self.count()?;
                let mut parts = Vec::new();
                for _ in 0..n {
                    self.skip_style();
                    if self.i >= self.t.len() {
                        break;
                    }
                    if let (Some(g), _) = self.object(depth + 1, budget)? {
                        parts.push(g);
                    }
                }
                Some(Geometry::Collection(parts))
            }
            w => return Err(format!("unknown MIF object {w:?}")),
        };
        Ok((g, None))
    }
}

fn parse_mif_header(lines: &[&str]) -> Result<(Mif, usize), String> {
    let mut m = Mif {
        delimiter: '\t',
        charset: None,
        coordsys: None,
        transform: None,
        fields: Vec::new(),
    };
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i].trim();
        let low = l.to_ascii_lowercase();
        let rest = |k: usize| l[k..].trim();
        if low == "data" {
            return Ok((m, i + 1));
        } else if low.starts_with("delimiter") {
            let r = rest(9).trim_matches('"');
            m.delimiter = r.chars().next().unwrap_or('\t');
        } else if low.starts_with("charset") {
            m.charset = encoding(rest(7));
        } else if low.starts_with("coordsys") {
            m.coordsys = Some(parse_coordsys(rest(8))?);
        } else if low.starts_with("transform") {
            let v: Vec<f64> = tokens(rest(9))
                .into_iter()
                .filter_map(|t| match t {
                    Tok::Num(v) => Some(v),
                    _ => None,
                })
                .collect();
            if v.len() == 4 && v[0] != 0.0 && v[1] != 0.0 {
                m.transform = Some([v[0], v[1], v[2], v[3]]);
            }
        } else if low.starts_with("columns") {
            let n: usize = rest(7)
                .parse()
                .map_err(|_| "a MIF Columns count is malformed")?;
            let end = (i + 1 + n).min(lines.len());
            m.fields = columns(&lines[i + 1..end])?;
            i = end;
            continue;
        }
        i += 1;
    }
    Err("the MIF has no Data section".into())
}

/// Reads a MIF file; attributes come from the `.mid` beside `path`.
pub fn read_mif(path: &Path, data: &[u8]) -> Result<Dataset, String> {
    let head = String::from_utf8_lossy(&data[..data.len().min(8192)]);
    let charset = head
        .lines()
        .find(|l| l.trim().to_ascii_lowercase().starts_with("charset"))
        .and_then(|l| encoding(l.trim()[7..].trim()));
    let text = decode(data, charset);
    let lines: Vec<&str> = text.lines().collect();
    let (mif, start) = parse_mif_header(&lines)?;
    let enc = mif.charset.or(charset);
    let name = layer_name(path);
    let (mut ds, mut layer) = new_dataset("MapInfo MIF", &name, &mif.fields);
    match &mif.coordsys {
        Some(cs) => (ds.crs, ds.crs_note) = crs_of(cs),
        // No CoordSys: longitude/latitude on WGS 84, as MapInfo assumes.
        None => {
            ds.crs = crs::from_epsg(4326);
            ds.crs_note = Some("assumed longitude/latitude (no CoordSys)".into());
        }
    }
    let mid_text = sidecar(path, "mid").map(|b| decode(&b, enc));
    if mid_text.is_none() && !mif.fields.is_empty() {
        ds.warn("no attribute file (.mid) beside the file");
    }
    let mid: Vec<Vec<String>> = mid_text
        .as_deref()
        .map(|t| mid_records(t, mif.delimiter))
        .unwrap_or_default();
    let body = lines[start..].join("\n");
    let mut p = MifParser {
        t: tokens(&body),
        i: 0,
        transform: mif.transform,
    };
    let mut budget = MAX_VERTICES;
    let mut row = 0usize;
    loop {
        p.skip_style();
        if p.i >= p.t.len() {
            break;
        }
        if layer.features.len() >= MAX_FEATURES {
            ds.warn("too many features; the rest are not shown");
            break;
        }
        let (g, text) = match p.object(0, &mut budget) {
            Ok(o) => o,
            Err(e) => {
                ds.warn(format!("object {}: {e}; the rest are not shown", row + 1));
                break;
            }
        };
        let vals: &[String] = mid.get(row).map_or(&[], Vec::as_slice);
        let mut attrs: Vec<(String, Value)> = mif
            .fields
            .iter()
            .enumerate()
            .map(|(k, (n, ty))| {
                (
                    n.clone(),
                    vals.get(k).map_or(Value::Null, |v| text_value(v, *ty)),
                )
            })
            .collect();
        if let Some(t) = text {
            attrs.push(("text".into(), Value::Text(t)));
        }
        row += 1;
        layer.push(Some(row.to_string()), g, attrs);
    }
    ds.layers.push(layer);
    Ok(ds)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Arc angles of 1e15° and NaN: at most one turn of 2° steps, not 10^14 points.
    #[test]
    fn arc_sweep_bounded() {
        for (s, e) in [(0.0, 1e15), (-1e300, 1e300), (f64::NAN, 90.0), (0.0, 360.0)] {
            let Geometry::LineString(l) = arc(0.0, 0.0, 2.0, 2.0, s, e) else {
                panic!()
            };
            assert!(l.len() <= 200, "{s} {e}: {}", l.len());
        }
        let Geometry::LineString(l) = arc(0.0, 0.0, 2.0, 2.0, 0.0, 360.0) else {
            panic!()
        };
        assert!(l.len() > 100);
    }

    fn tmpdir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("specular-mitab-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const MIF: &str = "Version 300\nCharset \"WindowsLatin1\"\nDelimiter \",\"\n\
CoordSys Earth Projection 8, 104, \"m\", 3, 0, 0.9996, 500000, 0\n\
Columns 3\n  NAME Char(20)\n  N Integer\n  D Date\nData\n\n\
Point 500000 4000000\n    Symbol (35,0,12)\n\
Pline Multiple 2\n 2\n0 0\n1 1\n 2\n2 2\n3 3\n  Pen (1,2,0)\n\
Region 2\n 4\n0 0\n10 0\n10 10\n0 0\n 4\n1 1\n2 1\n2 2\n1 1\n  Brush (2,16777215,16777215)\n\
Text \"label\"\n 0 1 2 3\n  Font (\"Arial\",0,0,0)\n  Angle 30\n  Label Line Simple 1 2\n\
none\n\
Ellipse -1 -1 1 1\n";

    const MID: &str = "\"a, b\",1,20240131\n\"x\"\"y\",2,\nz,3,19991231\n,4,\n,5,\n";

    #[test]
    fn mif_objects_and_attributes() {
        assert!(sniff_mif(MIF.as_bytes()));
        assert!(!sniff_tab(MIF.as_bytes()));
        let d = tmpdir("mif");
        let p = d.join("t.mif");
        std::fs::write(&p, MIF).unwrap();
        std::fs::write(d.join("t.mid"), MID).unwrap();
        let ds = read_mif(&p, MIF.as_bytes()).unwrap();
        assert_eq!(ds.format, "MapInfo MIF");
        assert_eq!(ds.crs.as_ref().and_then(|c| c.epsg), Some(32631));
        let l = &ds.layers[0];
        assert_eq!(l.features.len(), 6);
        assert_eq!(
            l.features[0].geometry,
            Some(Geometry::Point([500000.0, 4000000.0]))
        );
        assert_eq!(l.features[0].values[0], Value::Text("a, b".into()));
        assert_eq!(l.features[0].values[2], Value::Date("2024-01-31".into()));
        assert_eq!(l.features[1].values[0], Value::Text("x\"y".into()));
        assert!(
            matches!(&l.features[1].geometry, Some(Geometry::MultiLineString(m)) if m.len() == 2)
        );
        assert!(matches!(&l.features[2].geometry, Some(Geometry::Polygon(r)) if r.len() == 2));
        let Some(Geometry::Point(t)) = l.features[3].geometry else {
            panic!()
        };
        assert!((t[0] - 1.0).abs() < 1e-12 && t[1] == 1.0);
        let ti = l.fields().iter().position(|f| f == "text").unwrap();
        assert_eq!(l.features[3].values[ti], Value::Text("label".into()));
        assert_eq!(l.features[4].geometry, None);
        let Some(Geometry::Polygon(e)) = &l.features[5].geometry else {
            panic!()
        };
        assert_eq!(e[0].len(), 182);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn coordsys_clauses() {
        let c = parse_coordsys("Earth Projection 3, 33, \"m\", 3, 46.5, 44, 49, 700000, 6600000")
            .unwrap();
        assert_eq!(crs_of(&c).0.and_then(|c| c.epsg), Some(2154));
        let c = parse_coordsys("Earth Projection 1, 104").unwrap();
        assert_eq!(crs_of(&c).0.and_then(|c| c.epsg), Some(4326));
        let c = parse_coordsys("Earth Projection 10, 157, \"m\", 0").unwrap();
        assert_eq!(crs_of(&c).0.and_then(|c| c.epsg), Some(3857));
        let c = parse_coordsys("NonEarth Units \"m\" Bounds (0, 0) (10, 10)").unwrap();
        assert!(crs_of(&c).0.is_none());
        let c = parse_coordsys("Earth Projection 8, 12, \"m\", 153, 0, 0.9996, 500000, 10000000")
            .unwrap();
        assert_eq!(crs_of(&c).0.and_then(|c| c.epsg), Some(20256));
    }

    /// A native table: one row with a point, one with a compressed polyline, one without
    /// geometry and one deleted.
    fn write_tab(d: &Path) -> std::path::PathBuf {
        let tab = "!table\n!version 300\n!charset WindowsLatin1\n\nDefinition Table\n  Type NATIVE Charset \"WindowsLatin1\"\n  Fields 2\n    name Char (8) ;\n    n Integer ;\n";
        let p = d.join("t.tab");
        std::fs::write(&p, tab).unwrap();
        // .dat: 4 records of 1 + 8 + 4 bytes.
        let mut dat = vec![0u8; 32];
        dat[0] = 3;
        dat[4..8].copy_from_slice(&4u32.to_le_bytes());
        let header = 32 + 2 * 32 + 1;
        dat[8..10].copy_from_slice(&(header as u16).to_le_bytes());
        dat[10..12].copy_from_slice(&13u16.to_le_bytes());
        for (n, w) in [("name", 8u8), ("n", 4)] {
            let mut f = [0u8; 32];
            f[..n.len()].copy_from_slice(n.as_bytes());
            f[11] = b'C';
            f[16] = w;
            dat.extend_from_slice(&f);
        }
        dat.push(0x0D);
        for (k, name) in ["one", "two", "three", "gone"].iter().enumerate() {
            dat.push(if k == 3 { b'*' } else { b' ' });
            let mut s = [b' '; 8];
            s[..name.len()].copy_from_slice(name.as_bytes());
            dat.extend_from_slice(&s);
            dat.extend_from_slice(&(k as i32 * 10).to_le_bytes());
        }
        std::fs::write(d.join("t.dat"), &dat).unwrap();
        // .map: header 1024 bytes, an object block at 1024, a coordinate block at 1536.
        let mut m = vec![0u8; 2048];
        m[0x100..0x104].copy_from_slice(&42_424_242i32.to_le_bytes());
        m[0x104..0x106].copy_from_slice(&300i16.to_le_bytes());
        m[0x106..0x108].copy_from_slice(&512i16.to_le_bytes());
        m[0x161] = 1;
        m[0x16A..0x16C].copy_from_slice(&104u16.to_le_bytes());
        m[0x16D] = 1; // lon/lat
        m[0x16E] = 28;
        m[0x16F] = 7;
        m[0x170..0x178].copy_from_slice(&1000.0f64.to_le_bytes());
        m[0x178..0x180].copy_from_slice(&1000.0f64.to_le_bytes());
        let ob = 1024;
        m[ob..ob + 2].copy_from_slice(&2i16.to_le_bytes());
        m[ob + 4..ob + 8].copy_from_slice(&5000i32.to_le_bytes());
        m[ob + 8..ob + 12].copy_from_slice(&6000i32.to_le_bytes());
        let mut o = Vec::new();
        // Point (uncompressed) at (1.5, 2.5).
        let p1 = ob + 20;
        o.push(0x02u8);
        o.extend_from_slice(&1i32.to_le_bytes());
        o.extend_from_slice(&1500i32.to_le_bytes());
        o.extend_from_slice(&2500i32.to_le_bytes());
        o.push(0);
        // Compressed pline: 3 points relative to origin (5000, 6000).
        let p2 = p1 + o.len();
        o.push(0x07);
        o.extend_from_slice(&2i32.to_le_bytes());
        o.extend_from_slice(&(1536 + 8i32).to_le_bytes());
        o.extend_from_slice(&12i32.to_le_bytes());
        o.extend_from_slice(&[0, 0, 0, 0]); // label
        o.extend_from_slice(&5000i32.to_le_bytes());
        o.extend_from_slice(&6000i32.to_le_bytes());
        o.extend_from_slice(&[0; 8]);
        o.push(0);
        m[ob + 2..ob + 4].copy_from_slice(&(o.len() as i16).to_le_bytes());
        m[p1..p1 + o.len()].copy_from_slice(&o);
        let cb = 1536;
        m[cb..cb + 2].copy_from_slice(&3i16.to_le_bytes());
        m[cb + 2..cb + 4].copy_from_slice(&12i16.to_le_bytes());
        for (k, (x, y)) in [(0i16, 0i16), (1000, 0), (1000, 1000)].iter().enumerate() {
            let at = cb + 8 + 4 * k;
            m[at..at + 2].copy_from_slice(&x.to_le_bytes());
            m[at + 2..at + 4].copy_from_slice(&y.to_le_bytes());
        }
        std::fs::write(d.join("t.map"), &m).unwrap();
        let mut id = Vec::new();
        for off in [p1 as u32, p2 as u32, 0, p1 as u32] {
            id.extend_from_slice(&off.to_le_bytes());
        }
        std::fs::write(d.join("t.id"), &id).unwrap();
        p
    }

    #[test]
    fn native_table() {
        let d = tmpdir("tab");
        let p = write_tab(&d);
        let data = std::fs::read(&p).unwrap();
        assert!(sniff_tab(&data));
        let ds = read_tab(&p, &data).unwrap();
        assert_eq!(ds.crs.as_ref().and_then(|c| c.epsg), Some(4326));
        let l = &ds.layers[0];
        assert_eq!(l.features.len(), 3, "the deleted row is skipped");
        assert_eq!(l.features[0].geometry, Some(Geometry::Point([1.5, 2.5])));
        assert_eq!(
            l.features[0].values,
            vec![Value::Text("one".into()), Value::Int(0)]
        );
        assert_eq!(
            l.features[1].geometry,
            Some(Geometry::LineString(vec![
                [5.0, 6.0],
                [6.0, 6.0],
                [6.0, 7.0]
            ]))
        );
        assert_eq!(l.features[2].geometry, None);
        assert_eq!(l.features[2].id.as_deref(), Some("3"));

        // Damaged copies never panic.
        let map = std::fs::read(d.join("t.map")).unwrap();
        for cut in [0, 300, 1030, 1100, 1540, 1550] {
            std::fs::write(d.join("t.map"), &map[..cut]).unwrap();
            let _ = read_tab(&p, &data);
        }
        let mut bad = map.clone();
        for k in (1024..2048).step_by(3) {
            bad[k] ^= 0xA5;
        }
        std::fs::write(d.join("t.map"), &bad).unwrap();
        let _ = read_tab(&p, &data);
        let dat = std::fs::read(d.join("t.dat")).unwrap();
        std::fs::write(d.join("t.dat"), &dat[..40]).unwrap();
        let _ = read_tab(&p, &data);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn damaged_mif_never_panics() {
        let d = tmpdir("mifbad");
        let p = d.join("t.mif");
        for cut in (0..MIF.len()).step_by(7) {
            let _ = read_mif(&p, &MIF.as_bytes()[..cut]);
        }
        let mut b = MIF.as_bytes().to_vec();
        for k in (0..b.len()).step_by(5) {
            b[k] ^= 0x21;
        }
        let _ = read_mif(&p, &b);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn refusals() {
        let r = |t: &str| parse_tab(t).err().unwrap();
        assert!(
            r("!table\nDefinition Table\n  Type NATIVE\n  \"\\IsSeamless\" = \"TRUE\"\n")
                .contains("seamless")
        );
        assert!(r("!Table\nOpen Table \"a\"\nCreate View v As Select * From a").contains("view"));
        assert!(
            r("!table\nDefinition Table\n  File \"x.tif\"\n  Type \"RASTER\"\n").contains("raster")
        );
        assert!(r("!table\nDefinition Table\n  Type LINKED\n").contains("database"));
        assert!(r("!table\nDefinition Table\n  Type WFS\n").contains("web service"));
        assert!(r("!table\nDefinition Table\n  Type ACCESS\n").contains("ACCESS"));
    }

    #[test]
    fn strokes_like_gdal() {
        let Geometry::LineString(a) = arc(-1.0, -1.0, 1.0, 1.0, 0.0, 360.0) else {
            panic!()
        };
        assert_eq!(a.len(), 182);
        let Geometry::Polygon(r) = round_rect(-1.0, -1.0, 1.0, 1.0, 0.5, 0.5) else {
            panic!()
        };
        assert_eq!(r[0].len(), 185);
        let t = text_point([0.0, 1.0, 2.0, 3.0], 2.0, 30.0);
        assert!((t[0] - 1.0).abs() < 1e-12 && t[1] == 1.0);
    }
}
