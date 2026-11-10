//! World files and projection files: the georeference any raster can carry beside it.
//!
//! A world file (`.tfw`, `.jgw`, `.pgw`, `.wld`, ...) is six numbers, one per line: the x
//! and y size of a pixel with the two rotation terms (A, D, B, E) and the map position of
//! the *centre* of the top-left pixel (C, F). A `.prj` holds the CRS as WKT (ESRI's or
//! OGC's), or in the older ArcInfo keyword form (`Projection UTM`, `Zone 33`, ...).

use crate::crs::{self, Crs, CrsKind, LinearUnit};
use crate::geotiff::Affine2;
use crate::proj::{Ellipsoid, Method};

/// Parses a world file into a pixel-corner transform (the file names pixel centres).
pub fn parse(text: &str) -> Option<Affine2> {
    let mut v = [0f64; 6];
    let mut n = 0;
    for tok in text.split_whitespace() {
        if n == 6 {
            break;
        }
        // Some writers use a decimal comma.
        let t = tok.replace(',', ".");
        v[n] = t.parse().ok().filter(|x: &f64| x.is_finite())?;
        n += 1;
    }
    if n < 6 {
        return None;
    }
    let [a, d, b, e, c, f] = v;
    if a * e - b * d == 0.0 {
        return None;
    }
    Some(Affine2 {
        a,
        b,
        c: c - 0.5 * a - 0.5 * b,
        d,
        e,
        f: f - 0.5 * d - 0.5 * e,
    })
}

/// Writes a pixel-corner transform as a world file (fixtures, and the inverse of [`parse`]).
pub fn write(t: &Affine2) -> String {
    let c = t.c + 0.5 * t.a + 0.5 * t.b;
    let f = t.f + 0.5 * t.d + 0.5 * t.e;
    format!(
        "{:.10}\n{:.10}\n{:.10}\n{:.10}\n{:.10}\n{:.10}\n",
        t.a, t.d, t.b, t.e, c, f
    )
}

/// The CRS of a `.prj` file: WKT, or the ArcInfo keyword form.
pub fn parse_prj(text: &str) -> Option<Crs> {
    let t = text.trim().trim_end_matches('\0');
    if t.is_empty() {
        return None;
    }
    if t.contains('[') {
        return crate::wkt::crs_from_wkt(t);
    }
    if let Some(code) = crs::epsg_from_ref(t) {
        return crs::from_epsg(code);
    }
    legacy(t)
}

/// `Projection UTM / Zone 33 / Datum WGS84 / Units METERS / Spheroid ... / Parameters`.
pub(crate) fn legacy(text: &str) -> Option<Crs> {
    let mut proj = None;
    let mut zone = None;
    let mut datum = None;
    let mut units = None;
    let mut south = false;
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let Some(k) = it.next() else { continue };
        let v: Vec<&str> = it.collect();
        match k.to_ascii_lowercase().as_str() {
            "projection" => proj = v.first().map(|s| s.to_ascii_uppercase()),
            "zone" => {
                zone = v.first().and_then(|z| z.parse::<i32>().ok());
            }
            "datum" => datum = v.first().map(|s| s.to_ascii_uppercase()),
            "units" => units = v.first().map(|s| s.to_ascii_uppercase()),
            "yshift" => {
                south = v
                    .first()
                    .and_then(|s| s.parse::<f64>().ok())
                    .is_some_and(|y| y != 0.0)
            }
            _ => {}
        }
    }
    let datum = datum.unwrap_or_else(|| "WGS84".into());
    let geo_code = match datum.as_str() {
        "WGS84" => 4326,
        "NAD83" => 4269,
        "NAD27" => 4267,
        "ETRS89" => 4258,
        other => crate::envi::datum_epsg(other).unwrap_or(4326),
    };
    match proj.as_deref()? {
        "GEOGRAPHIC" => crs::from_epsg(geo_code),
        "UTM" => {
            let z = zone?;
            let (n, zabs) = (z > 0 && !south, z.unsigned_abs());
            if !(1..=60).contains(&zabs) {
                return None;
            }
            let code = match (geo_code, n) {
                (4326, true) => 32600 + zabs,
                (4326, false) => 32700 + zabs,
                (4269, true) if (1..=23).contains(&zabs) => 26900 + zabs,
                (4267, true) if (1..=22).contains(&zabs) => 26700 + zabs,
                (4258, true) if (28..=38).contains(&zabs) => 25800 + zabs,
                // GDA94 and GDA2020 MGA zones.
                (4283, false) if (48..=58).contains(&zabs) => 28300 + zabs,
                (7844, false) if (46..=59).contains(&zabs) => 7800 + zabs,
                _ => 0,
            };
            crs::from_epsg(code).or_else(|| {
                let ell = crs::from_epsg(geo_code).map_or(Ellipsoid::WGS84, |c| c.ellipsoid);
                let unit = match units.as_deref() {
                    Some("FEET") => LinearUnit::us_survey_foot(),
                    _ => LinearUnit::metre(),
                };
                Some(Crs::custom_projected(
                    format!("{datum} / UTM zone {zabs}{}", if n { 'N' } else { 'S' }),
                    ell,
                    Method::utm(zabs, n),
                    unit,
                ))
            })
        }
        other => {
            let p = parameters(text);
            let par = |i: usize| p.get(i).copied();
            let method = match other {
                "ALBERS" => Some(Method::Albers {
                    lat1: par(0)?,
                    lat2: par(1)?,
                    lon0: par(2)?,
                    lat0: par(3)?,
                    fe: par(4)?,
                    fn_: par(5)?,
                }),
                "LAMBERT" => Some(Method::LambertConic {
                    lat1: par(0)?,
                    lat2: par(1)?,
                    lon0: par(2)?,
                    lat0: par(3)?,
                    k0: 1.0,
                    fe: par(4)?,
                    fn_: par(5)?,
                }),
                "TRANSVERSE" => Some(Method::TransverseMercator {
                    k0: par(0)?,
                    lon0: par(1)?,
                    lat0: par(2)?,
                    fe: par(3)?,
                    fn_: par(4)?,
                }),
                _ => None,
            };
            let Some(method) = method else {
                return Some(Crs::named(format!("{other} ({datum})"), CrsKind::Projected));
            };
            let base = crs::from_epsg(geo_code);
            let ell = base.as_ref().map_or(Ellipsoid::WGS84, |c| c.ellipsoid);
            let unit = match units.as_deref() {
                Some("FEET") => LinearUnit::us_survey_foot(),
                _ => LinearUnit::metre(),
            };
            let mut c =
                Crs::custom_projected(format!("{} ({datum})", method.name()), ell, method, unit);
            c.datum = base.and_then(|b| b.datum);
            Some(crs::identify(&c).unwrap_or(c))
        }
    }
}

/// The values after `Parameters` in an ArcInfo `.prj`: one per line, either a number or
/// degrees, minutes and seconds ("-132 30 0.000 /* central meridian").
fn parameters(text: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let mut on = false;
    for line in text.lines() {
        let body = line.split("/*").next().unwrap_or("").trim();
        if !on {
            on = body.to_ascii_lowercase().starts_with("parameters");
            continue;
        }
        let nums: Vec<&str> = body.split_whitespace().collect();
        let v: Option<Vec<f64>> = nums.iter().map(|s| s.parse::<f64>().ok()).collect();
        match v.as_deref() {
            Some([x]) => out.push(*x),
            Some([d, m, s]) => {
                let neg = nums[0].starts_with('-');
                let mag = d.abs() + m / 60.0 + s / 3600.0;
                out.push(if neg { -mag } else { mag });
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centres_to_corners() {
        let t = parse("30.0\n0.0\n0.0\n-30.0\n500015.0\n4199985.0\n").unwrap();
        assert_eq!(t.c, 500000.0);
        assert_eq!(t.f, 4200000.0);
        assert_eq!(t.apply(1.0, 1.0), (500030.0, 4199970.0));
        let back = parse(&write(&t)).unwrap();
        assert!((back.c - t.c).abs() < 1e-6 && (back.f - t.f).abs() < 1e-6);
        assert!(parse("1\n0\n0\n").is_none());
        assert!(parse("0\n0\n0\n0\n1\n1\n").is_none());
        // Rotated.
        let r = parse("2\n1\n1\n-2\n10\n20\n").unwrap();
        assert_eq!(r.apply(0.5, 0.5), (10.0, 20.0));
    }

    #[test]
    fn prj_forms() {
        let esri = r#"PROJCS["WGS_1984_UTM_Zone_33N",GEOGCS["GCS_WGS_1984",DATUM["D_WGS_1984",SPHEROID["WGS_1984",6378137.0,298.257223563]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]],PROJECTION["Transverse_Mercator"],PARAMETER["False_Easting",500000.0],PARAMETER["False_Northing",0.0],PARAMETER["Central_Meridian",15.0],PARAMETER["Scale_Factor",0.9996],PARAMETER["Latitude_Of_Origin",0.0],UNIT["Meter",1.0]]"#;
        let c = parse_prj(esri).unwrap();
        assert_eq!(c.kind, CrsKind::Projected);
        let (lon, lat) = c.to_lonlat(500000.0, 0.0).unwrap();
        assert!((lon - 15.0).abs() < 1e-9 && lat.abs() < 1e-9);
        let old = "Projection    UTM\nZone          33\nDatum         WGS84\nUnits         METERS\nSpheroid      WGS84\nXshift        0.0\nYshift        0.0\nParameters\n";
        assert_eq!(parse_prj(old).unwrap().epsg, Some(32633));
        let geo = "Projection GEOGRAPHIC\nDatum NAD83\nUnits DD\n";
        assert_eq!(parse_prj(geo).unwrap().epsg, Some(4269));
        assert!(parse_prj("").is_none());
        // ArcInfo Albers with degrees-minutes-seconds parameters: NAD83 / Yukon Albers.
        let albers = "Projection    ALBERS\nDatum         NAD83\nUnits         METERS\nSpheroid      GRS1980\nParameters\n 61 40  0.000 /* 1st standard parallel\n 68  0  0.000 /* 2nd standard parallel\n-132 30  0.000 /* central meridian\n 59  0  0.000 /* latitude of projection's origin\n500000.00000 /* false easting (meters)\n500000.00000 /* false northing (meters)\n";
        assert_eq!(parameters(albers)[2], -132.5);
        let c = parse_prj(albers).unwrap();
        assert_eq!(c.epsg, Some(3578));
        let (lon, lat) = c.to_lonlat(500000.0, 500000.0).unwrap();
        assert!((lon + 132.5).abs() < 1e-9 && (lat - 59.0).abs() < 1e-9);
        // Unlisted parameters: a custom CRS, not a wrong code.
        let odd = albers.replace("-132 30", "-131 30");
        let c = parse_prj(&odd).unwrap();
        assert_eq!(c.epsg, None);
        assert_eq!(c.kind, CrsKind::Projected);
    }
}
