//! Coordinate reference systems: what a file says about its coordinates, enough to name
//! them, convert projected coordinates back to longitude and latitude, and measure in
//! metres.
//!
//! A small built-in EPSG table covers the common geographic systems, the UTM families
//! (WGS 84, NAD83, NAD27, ETRS89, ED50, GDA94/GDA2020 MGA), Web and World Mercator and a
//! handful of national grids and conic equal-area / conformal systems. Anything else is
//! described by its WKT (see [`crate::wkt`]) or GeoKeys (see [`crate::geotiff`]), which
//! give the method and its parameters directly.

use crate::datum::{self, ToWgs84};
use crate::proj::{Ellipsoid, Method, Projection};

/// What kind of coordinates a CRS has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrsKind {
    /// Longitude and latitude in degrees.
    Geographic,
    /// Easting and northing on a map projection.
    Projected,
    /// Geocentric, engineering or unknown coordinates.
    Other,
}

/// A linear unit: its name and length in metres.
#[derive(Clone, Debug, PartialEq)]
pub struct LinearUnit {
    pub name: String,
    pub metres: f64,
}

impl LinearUnit {
    pub fn metre() -> LinearUnit {
        LinearUnit {
            name: "metre".into(),
            metres: 1.0,
        }
    }
    pub fn us_survey_foot() -> LinearUnit {
        LinearUnit {
            name: "US survey foot".into(),
            metres: 1200.0 / 3937.0,
        }
    }
    pub fn foot() -> LinearUnit {
        LinearUnit {
            name: "foot".into(),
            metres: 0.3048,
        }
    }
}

/// A coordinate reference system.
#[derive(Clone, Debug, PartialEq)]
pub struct Crs {
    /// EPSG code, when the file gave one (or its definition matched one).
    pub epsg: Option<u32>,
    /// Its name, e.g. "WGS 84 / UTM zone 33N".
    pub name: String,
    pub kind: CrsKind,
    /// The geographic CRS it is based on (its own name for a geographic CRS).
    pub geographic: Option<String>,
    /// The datum, when named.
    pub datum: Option<String>,
    pub ellipsoid: Ellipsoid,
    /// The ellipsoid's name, when known.
    pub ellipsoid_name: Option<String>,
    /// Projection, for projected CRSs whose method is supported here.
    pub projection: Option<Projection>,
    /// Projection method name as the file gave it, when it is not supported here.
    pub method_name: Option<String>,
    /// Linear unit of a projected CRS.
    pub unit: Option<LinearUnit>,
    /// The definition the file gave, verbatim.
    pub wkt: Option<String>,
    pub projjson: Option<String>,
    /// The prime meridian, degrees east of Greenwich (Paris for the French NTF systems).
    pub pm: f64,
    /// How its datum relates to WGS 84.
    pub datum_shift: ToWgs84,
}

impl Crs {
    fn geographic(epsg: u32, name: &str, datum: &str, ell: Ellipsoid, ell_name: &str) -> Crs {
        Crs {
            epsg: Some(epsg),
            name: name.into(),
            kind: CrsKind::Geographic,
            geographic: Some(name.into()),
            datum: Some(datum.into()),
            ellipsoid: ell,
            ellipsoid_name: Some(ell_name.into()),
            projection: None,
            method_name: None,
            unit: None,
            wkt: None,
            projjson: None,
            pm: 0.0,
            datum_shift: ToWgs84::Unknown,
        }
    }

    fn projected(epsg: u32, name: String, base: &Crs, method: Method, unit: LinearUnit) -> Crs {
        Crs {
            epsg: Some(epsg),
            name,
            kind: CrsKind::Projected,
            geographic: Some(base.name.clone()),
            datum: base.datum.clone(),
            ellipsoid: base.ellipsoid,
            ellipsoid_name: base.ellipsoid_name.clone(),
            projection: Some(Projection::new(method, base.ellipsoid)),
            method_name: None,
            unit: Some(unit),
            wkt: None,
            projjson: None,
            pm: base.pm,
            datum_shift: base.datum_shift.clone(),
        }
    }

    /// An unnamed projected CRS from a method (parameters in metres and degrees).
    pub fn custom_projected(name: String, ell: Ellipsoid, method: Method, unit: LinearUnit) -> Crs {
        Crs {
            epsg: None,
            name,
            kind: CrsKind::Projected,
            geographic: None,
            datum: None,
            ellipsoid: ell,
            ellipsoid_name: None,
            projection: Some(Projection::new(method, ell)),
            method_name: None,
            unit: Some(unit),
            wkt: None,
            projjson: None,
            pm: 0.0,
            datum_shift: ToWgs84::Unknown,
        }
    }

    /// A CRS known only by name.
    pub fn named(name: String, kind: CrsKind) -> Crs {
        Crs {
            epsg: None,
            name,
            kind,
            geographic: None,
            datum: None,
            ellipsoid: Ellipsoid::WGS84,
            ellipsoid_name: None,
            projection: None,
            method_name: None,
            unit: None,
            wkt: None,
            projjson: None,
            pm: 0.0,
            datum_shift: ToWgs84::Unknown,
        }
    }

    /// Longitude and latitude (degrees, on this CRS's own datum) of a point in its
    /// coordinates; `None` when the projection is not supported or the point is outside
    /// its domain.
    pub fn to_lonlat(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let (lon, lat) = match self.kind {
            CrsKind::Geographic => {
                (x.is_finite() && y.is_finite() && y.abs() <= 90.0).then_some((x, y))?
            }
            CrsKind::Projected => {
                let m = self.unit.as_ref().map_or(1.0, |u| u.metres);
                self.projection.as_ref()?.inverse(x * m, y * m)?
            }
            CrsKind::Other => return None,
        };
        Some((wrap180(lon + self.pm), lat))
    }

    /// Coordinates of a longitude and latitude (degrees, on this CRS's datum) in this CRS.
    pub fn from_lonlat(&self, lon: f64, lat: f64) -> Option<(f64, f64)> {
        let lon = wrap180(lon - self.pm);
        match self.kind {
            CrsKind::Geographic => Some((lon, lat)),
            CrsKind::Projected => {
                let m = self.unit.as_ref().map_or(1.0, |u| u.metres);
                let (x, y) = self.projection.as_ref()?.forward(lon, lat)?;
                Some((x / m, y / m))
            }
            CrsKind::Other => None,
        }
    }

    /// Whether coordinates can be converted to longitude and latitude at all: a geographic
    /// CRS, or a projected one whose method is supported here.
    pub fn convertible(&self) -> bool {
        match self.kind {
            CrsKind::Geographic => true,
            CrsKind::Projected => self.projection.is_some(),
            CrsKind::Other => false,
        }
    }

    /// Longitude and latitude on WGS 84 of a point in this CRS's coordinates: `None` when
    /// the projection is not supported here or the datum's relation to WGS 84 is unknown.
    pub fn to_wgs84(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let (lon, lat) = self.to_lonlat(x, y)?;
        let h = self.datum_shift.at(lon, lat)?;
        let (lon, lat) = datum::shift_to_wgs84(self.ellipsoid, &h, lon, lat);
        (lon.is_finite() && lat.is_finite()).then_some((lon, lat))
    }

    /// This CRS's coordinates of a longitude and latitude on WGS 84.
    pub fn from_wgs84(&self, lon: f64, lat: f64) -> Option<(f64, f64)> {
        let h = self.datum_shift.at(lon, lat)?;
        let (lon, lat) = datum::shift_from_wgs84(self.ellipsoid, &h, lon, lat);
        self.from_lonlat(lon, lat)
    }

    /// Metres per coordinate unit along x near `(x, y)`: the linear unit of a projected
    /// CRS (grid distance), or the length of a degree of longitude at that latitude on the
    /// ellipsoid for a geographic one.
    pub fn metres_per_unit_x(&self, _x: f64, y: f64) -> Option<f64> {
        match self.kind {
            CrsKind::Projected => self.unit.as_ref().map(|u| u.metres),
            CrsKind::Geographic if y.abs() < 89.0 => {
                let (n, _) = self.ellipsoid.radii(y);
                Some(n * y.to_radians().cos() * std::f64::consts::PI / 180.0)
            }
            _ => None,
        }
    }

    /// Metres per unit of y (northing or latitude) at map position (`x`, `y`): the unit's
    /// length when projected, the meridian's degree at the latitude when geographic.
    pub fn metres_per_unit_y(&self, _x: f64, y: f64) -> Option<f64> {
        match self.kind {
            CrsKind::Projected => self.unit.as_ref().map(|u| u.metres),
            CrsKind::Geographic if y.abs() <= 90.0 => {
                let (_, m) = self.ellipsoid.radii(y);
                Some(m * std::f64::consts::PI / 180.0)
            }
            _ => None,
        }
    }

    /// "EPSG:32633", or `None`.
    pub fn epsg_label(&self) -> Option<String> {
        self.epsg.map(|c| format!("EPSG:{c}"))
    }
}

/// A longitude into −180..180.
fn wrap180(lon: f64) -> f64 {
    if (-180.0..=180.0).contains(&lon) {
        lon
    } else {
        (lon + 180.0).rem_euclid(360.0) - 180.0
    }
}

fn utm_zone_name(base: &str, zone: u32, north: bool) -> String {
    format!("{base} / UTM zone {zone}{}", if north { 'N' } else { 'S' })
}

/// A CRS by its EPSG code (or one of the codes Esri and older software used for Web
/// Mercator): the hand-made table first (stable names and the oldest tests), with the
/// dataset's datum shifts; then the full EPSG table ([`crate::epsg`]).
pub fn from_epsg(code: u32) -> Option<Crs> {
    match builtin(code) {
        Some(mut c) => {
            let from_table = crate::epsg::lookup(code);
            match from_table {
                Some(t) => {
                    c.datum_shift = t.datum_shift;
                    c.pm = t.pm;
                }
                None if matches!(code, 900_913 | 3785 | 102_100) => c.datum_shift = ToWgs84::Same,
                None => {}
            }
            Some(c)
        }
        None => crate::epsg::lookup(code),
    }
}

/// The hand-made table.
fn builtin(code: u32) -> Option<Crs> {
    let wgs84 = || {
        Crs::geographic(
            4326,
            "WGS 84",
            "World Geodetic System 1984",
            Ellipsoid::WGS84,
            "WGS 84",
        )
    };
    let nad83 = || {
        Crs::geographic(
            4269,
            "NAD83",
            "North American Datum 1983",
            Ellipsoid::GRS80,
            "GRS 1980",
        )
    };
    let nad27 = || {
        Crs::geographic(
            4267,
            "NAD27",
            "North American Datum 1927",
            Ellipsoid::CLARKE1866,
            "Clarke 1866",
        )
    };
    let etrs89 = || {
        Crs::geographic(
            4258,
            "ETRS89",
            "European Terrestrial Reference System 1989 ensemble",
            Ellipsoid::GRS80,
            "GRS 1980",
        )
    };
    let ed50 = || {
        Crs::geographic(
            4230,
            "ED50",
            "European Datum 1950",
            Ellipsoid::INTL1924,
            "International 1924",
        )
    };
    let gda94 = || {
        Crs::geographic(
            4283,
            "GDA94",
            "Geocentric Datum of Australia 1994",
            Ellipsoid::GRS80,
            "GRS 1980",
        )
    };
    let gda2020 = || {
        Crs::geographic(
            7844,
            "GDA2020",
            "Geocentric Datum of Australia 2020",
            Ellipsoid::GRS80,
            "GRS 1980",
        )
    };
    let m = LinearUnit::metre;
    let tm = |lat0: f64, lon0: f64, k0: f64, fe: f64, fn_: f64| Method::TransverseMercator {
        lat0,
        lon0,
        k0,
        fe,
        fn_,
    };
    Some(match code {
        4326 => wgs84(),
        4979 => {
            let mut c = wgs84();
            c.epsg = Some(4979);
            c
        }
        4269 => nad83(),
        4267 => nad27(),
        4258 => etrs89(),
        4230 => ed50(),
        4283 => gda94(),
        7844 => gda2020(),
        4277 => Crs::geographic(
            4277,
            "OSGB36",
            "Ordnance Survey of Great Britain 1936",
            Ellipsoid::AIRY1830,
            "Airy 1830",
        ),
        4314 => Crs::geographic(
            4314,
            "DHDN",
            "Deutsches Hauptdreiecksnetz",
            Ellipsoid::BESSEL1841,
            "Bessel 1841",
        ),
        4322 => Crs::geographic(
            4322,
            "WGS 72",
            "World Geodetic System 1972",
            Ellipsoid::WGS72,
            "WGS 72",
        ),
        4617 => Crs::geographic(
            4617,
            "NAD83(CSRS)",
            "NAD83 Canadian Spatial Reference System",
            Ellipsoid::GRS80,
            "GRS 1980",
        ),
        4674 => Crs::geographic(
            4674,
            "SIRGAS 2000",
            "Sistema de Referencia Geocentrico para las AmericaS 2000",
            Ellipsoid::GRS80,
            "GRS 1980",
        ),
        4167 => Crs::geographic(
            4167,
            "NZGD2000",
            "New Zealand Geodetic Datum 2000",
            Ellipsoid::GRS80,
            "GRS 1980",
        ),
        4612 => Crs::geographic(
            4612,
            "JGD2000",
            "Japanese Geodetic Datum 2000",
            Ellipsoid::GRS80,
            "GRS 1980",
        ),
        6668 => Crs::geographic(
            6668,
            "JGD2011",
            "Japanese Geodetic Datum 2011",
            Ellipsoid::GRS80,
            "GRS 1980",
        ),
        4490 => Crs::geographic(
            4490,
            "China Geodetic Coordinate System 2000",
            "China 2000",
            Ellipsoid::inv(6_378_137.0, 298.257_222_101),
            "CGCS2000",
        ),
        4284 => Crs::geographic(
            4284,
            "Pulkovo 1942",
            "Pulkovo 1942",
            Ellipsoid::KRASSOVSKY,
            "Krassowsky 1940",
        ),
        4148 => Crs::geographic(
            4148,
            "Hartebeesthoek94",
            "Hartebeesthoek94",
            Ellipsoid::WGS84,
            "WGS 84",
        ),
        4171 => Crs::geographic(
            4171,
            "RGF93 v1",
            "Reseau Geodesique Francais 1993 v1",
            Ellipsoid::GRS80,
            "GRS 1980",
        ),
        3857 | 900_913 | 3785 | 102_100 => {
            let mut c = Crs::projected(
                3857,
                "WGS 84 / Pseudo-Mercator".into(),
                &wgs84(),
                Method::WebMercator {
                    lon0: 0.0,
                    fe: 0.0,
                    fn_: 0.0,
                },
                m(),
            );
            c.epsg = Some(if code == 102_100 { 3857 } else { code });
            c
        }
        3395 => Crs::projected(
            3395,
            "WGS 84 / World Mercator".into(),
            &wgs84(),
            Method::Mercator {
                lon0: 0.0,
                k0: 1.0,
                fe: 0.0,
                fn_: 0.0,
            },
            m(),
        ),
        32601..=32660 => {
            let z = code - 32600;
            Crs::projected(
                code,
                utm_zone_name("WGS 84", z, true),
                &wgs84(),
                Method::utm(z, true),
                m(),
            )
        }
        32701..=32760 => {
            let z = code - 32700;
            Crs::projected(
                code,
                utm_zone_name("WGS 84", z, false),
                &wgs84(),
                Method::utm(z, false),
                m(),
            )
        }
        26903..=26923 => {
            let z = code - 26900;
            Crs::projected(
                code,
                utm_zone_name("NAD83", z, true),
                &nad83(),
                Method::utm(z, true),
                m(),
            )
        }
        26703..=26722 => {
            let z = code - 26700;
            Crs::projected(
                code,
                utm_zone_name("NAD27", z, true),
                &nad27(),
                Method::utm(z, true),
                m(),
            )
        }
        25828..=25838 => {
            let z = code - 25800;
            Crs::projected(
                code,
                utm_zone_name("ETRS89", z, true),
                &etrs89(),
                Method::utm(z, true),
                m(),
            )
        }
        23028..=23038 => {
            let z = code - 23000;
            Crs::projected(
                code,
                utm_zone_name("ED50", z, true),
                &ed50(),
                Method::utm(z, true),
                m(),
            )
        }
        28348..=28358 => {
            let z = code - 28300;
            Crs::projected(
                code,
                format!("GDA94 / MGA zone {z}"),
                &gda94(),
                Method::utm(z, false),
                m(),
            )
        }
        7846..=7859 => {
            let z = code - 7800;
            Crs::projected(
                code,
                format!("GDA2020 / MGA zone {z}"),
                &gda2020(),
                Method::utm(z, false),
                m(),
            )
        }
        2193 => Crs::projected(
            2193,
            "NZGD2000 / New Zealand Transverse Mercator 2000".into(),
            &from_epsg(4167)?,
            tm(0.0, 173.0, 0.9996, 1_600_000.0, 10_000_000.0),
            m(),
        ),
        27700 => Crs::projected(
            27700,
            "OSGB36 / British National Grid".into(),
            &from_epsg(4277)?,
            tm(49.0, -2.0, 0.999_601_271_7, 400_000.0, -100_000.0),
            m(),
        ),
        31466..=31469 => {
            let z = code - 31464;
            Crs::projected(
                code,
                format!("DHDN / 3-degree Gauss-Kruger zone {z}"),
                &from_epsg(4314)?,
                tm(
                    0.0,
                    3.0 * z as f64,
                    1.0,
                    z as f64 * 1_000_000.0 + 500_000.0,
                    0.0,
                ),
                m(),
            )
        }
        3067 => Crs::projected(
            3067,
            "ETRS89 / TM35FIN(E,N)".into(),
            &etrs89(),
            tm(0.0, 27.0, 0.9996, 500_000.0, 0.0),
            m(),
        ),
        2154 => Crs::projected(
            2154,
            "RGF93 v1 / Lambert-93".into(),
            &from_epsg(4171)?,
            Method::LambertConic {
                lat0: 46.5,
                lon0: 3.0,
                lat1: 49.0,
                lat2: 44.0,
                k0: 1.0,
                fe: 700_000.0,
                fn_: 6_600_000.0,
            },
            m(),
        ),
        3034 => Crs::projected(
            3034,
            "ETRS89-extended / LCC Europe".into(),
            &etrs89(),
            Method::LambertConic {
                lat0: 52.0,
                lon0: 10.0,
                lat1: 35.0,
                lat2: 65.0,
                k0: 1.0,
                fe: 4_000_000.0,
                fn_: 2_800_000.0,
            },
            m(),
        ),
        2263 => Crs::projected(
            2263,
            "NAD83 / New York Long Island (ftUS)".into(),
            &nad83(),
            Method::LambertConic {
                lat0: 40.0 + 10.0 / 60.0,
                lon0: -74.0,
                lat1: 41.0 + 2.0 / 60.0,
                lat2: 40.0 + 40.0 / 60.0,
                k0: 1.0,
                fe: 300_000.0,
                fn_: 0.0,
            },
            LinearUnit::us_survey_foot(),
        ),
        3578 => Crs::projected(
            3578,
            "NAD83 / Yukon Albers".into(),
            &nad83(),
            Method::Albers {
                lat0: 59.0,
                lon0: -132.5,
                lat1: 61.0 + 40.0 / 60.0,
                lat2: 68.0,
                fe: 500_000.0,
                fn_: 500_000.0,
            },
            m(),
        ),
        5070 => Crs::projected(
            5070,
            "NAD83 / Conus Albers".into(),
            &nad83(),
            Method::Albers {
                lat0: 23.0,
                lon0: -96.0,
                lat1: 29.5,
                lat2: 45.5,
                fe: 0.0,
                fn_: 0.0,
            },
            m(),
        ),
        3338 => Crs::projected(
            3338,
            "NAD83 / Alaska Albers".into(),
            &nad83(),
            Method::Albers {
                lat0: 50.0,
                lon0: -154.0,
                lat1: 55.0,
                lat2: 65.0,
                fe: 0.0,
                fn_: 0.0,
            },
            m(),
        ),
        3577 => Crs::projected(
            3577,
            "GDA94 / Australian Albers".into(),
            &gda94(),
            Method::Albers {
                lat0: 0.0,
                lon0: 132.0,
                lat1: -18.0,
                lat2: -36.0,
                fe: 0.0,
                fn_: 0.0,
            },
            m(),
        ),
        _ => return None,
    })
}

/// An EPSG code from a CRS reference string: "EPSG:4326", "urn:ogc:def:crs:EPSG::3857",
/// "http://www.opengis.net/def/crs/EPSG/0/32633", or OGC CRS84 (which is 4326 with
/// longitude first, as every reader here takes it).
pub fn epsg_from_ref(s: &str) -> Option<u32> {
    let t = s.trim();
    let u = t.to_ascii_uppercase();
    if u.ends_with("CRS84") || u.ends_with("CRS:84") {
        return Some(4326);
    }
    let i = u.rfind("EPSG")?;
    // The last number after "EPSG": "EPSG/0/32633" and "EPSG:6.6:4326" carry a version
    // before the code.
    t[i + 4..]
        .split(|c: char| !c.is_ascii_digit())
        .rfind(|d| !d.is_empty())?
        .parse()
        .ok()
}

/// The table's projected systems (besides the UTM families) that [`identify`] tries.
const IDENTIFIABLE: &[u32] = &[
    3395, 2193, 27700, 3067, 2154, 3034, 2263, 5070, 3338, 3577, 3578, 3035, 3031, 3413, 2056,
    28992,
];

/// A method's kind and parameters, for comparing definitions.
fn method_params(m: &Method) -> (u8, [f64; 7]) {
    match *m {
        Method::TransverseMercator {
            lat0,
            lon0,
            k0,
            fe,
            fn_,
        } => (0, [lat0, lon0, k0, fe, fn_, 0.0, 0.0]),
        Method::Mercator { lon0, k0, fe, fn_ } => (1, [lon0, k0, fe, fn_, 0.0, 0.0, 0.0]),
        Method::MercatorB {
            lon0,
            lat1,
            fe,
            fn_,
        } => (2, [lon0, lat1, fe, fn_, 0.0, 0.0, 0.0]),
        Method::WebMercator { lon0, fe, fn_ } => (3, [lon0, fe, fn_, 0.0, 0.0, 0.0, 0.0]),
        Method::LambertConic {
            lat0,
            lon0,
            lat1,
            lat2,
            k0,
            fe,
            fn_,
        } => (4, [lat0, lon0, lat1, lat2, k0, fe, fn_]),
        Method::Albers {
            lat0,
            lon0,
            lat1,
            lat2,
            fe,
            fn_,
        } => (5, [lat0, lon0, lat1, lat2, fe, fn_, 0.0]),
        Method::PolarStereographic {
            lat_ts,
            lon0,
            south,
            fe,
            fn_,
        } => (
            6,
            [lat_ts, lon0, f64::from(u8::from(south)), fe, fn_, 0.0, 0.0],
        ),
        Method::RotatedPole {
            pole_lat,
            pole_lon,
            angle,
        } => (7, [pole_lat, pole_lon, angle, 0.0, 0.0, 0.0, 0.0]),
        Method::Sinusoidal { lon0, fe, fn_ } => (8, [lon0, fe, fn_, 0.0, 0.0, 0.0, 0.0]),
        Method::LambertAzimuthal {
            lat0,
            lon0,
            fe,
            fn_,
        } => (9, [lat0, lon0, fe, fn_, 0.0, 0.0, 0.0]),
        Method::Equirectangular {
            lat_ts,
            lon0,
            fe,
            fn_,
        } => (10, [lat_ts, lon0, fe, fn_, 0.0, 0.0, 0.0]),
        Method::TransverseMercatorSouth {
            lat0,
            lon0,
            k0,
            fe,
            fn_,
        } => (11, [lat0, lon0, k0, fe, fn_, 0.0, 0.0]),
        Method::PolarStereographicA {
            south,
            lon0,
            k0,
            fe,
            fn_,
        } => (
            12,
            [f64::from(u8::from(south)), lon0, k0, fe, fn_, 0.0, 0.0],
        ),
        Method::ObliqueStereographic {
            lat0,
            lon0,
            k0,
            fe,
            fn_,
        } => (13, [lat0, lon0, k0, fe, fn_, 0.0, 0.0]),
        Method::HotineObliqueMercator {
            latc,
            lonc,
            alpha,
            gamma,
            kc,
            fe,
            fn_,
            centre,
        } => (
            if centre { 15 } else { 14 },
            [latc, lonc, alpha, gamma, kc, fe, fn_],
        ),
        Method::CassiniSoldner {
            lat0,
            lon0,
            fe,
            fn_,
        } => (16, [lat0, lon0, fe, fn_, 0.0, 0.0, 0.0]),
        Method::EquidistantCylindrical {
            lat_ts,
            lon0,
            fe,
            fn_,
        } => (17, [lat_ts, lon0, fe, fn_, 0.0, 0.0, 0.0]),
        Method::CylindricalEqualArea {
            lat_ts,
            lon0,
            fe,
            fn_,
        } => (18, [lat_ts, lon0, fe, fn_, 0.0, 0.0, 0.0]),
    }
}

/// The EPSG system a definition without a code describes (an ArcInfo `.prj`, an IMAGINE
/// projection): the same method and parameters, ellipsoid and unit, as GDAL identifies
/// them. UTM zones are matched by their families; others by the table's own entries.
pub fn identify(c: &Crs) -> Option<Crs> {
    if c.epsg.is_some() {
        return None;
    }
    let p = c.projection.as_ref()?;
    let (kind, params) = method_params(&p.method);
    let same_ell =
        |e: &Ellipsoid| (e.a - p.ellipsoid.a).abs() < 1e-3 && (e.f - p.ellipsoid.f).abs() < 1e-12;
    let metres = c.unit.as_ref().map_or(1.0, |u| u.metres);
    let utm = match p.method {
        // Parameters stored in radians come back a few ulps off.
        Method::TransverseMercator {
            lat0,
            lon0,
            k0,
            fe,
            fn_,
        } if lat0.abs() < 1e-9
            && (k0 - 0.9996).abs() < 1e-12
            && fe == 500_000.0
            && ((lon0 + 183.0) / 6.0 - ((lon0 + 183.0) / 6.0).round()).abs() < 1e-9 =>
        {
            let zone = ((lon0 + 183.0) / 6.0).round() as u32;
            if fn_ == 0.0 {
                [32600, 26900, 26700, 25800]
                    .map(|base| base + zone)
                    .to_vec()
            } else {
                // South: WGS 84, then GDA94 and GDA2020 MGA (the datum, when named, picks).
                [32700, 28300, 7800].map(|base| base + zone).to_vec()
            }
        }
        _ => Vec::new(),
    };
    utm.into_iter()
        .chain(IDENTIFIABLE.iter().copied())
        .filter_map(from_epsg)
        .find(|k| {
            k.projection.as_ref().is_some_and(|q| {
                let (kk, kp) = method_params(&q.method);
                kk == kind
                    && same_ell(&q.ellipsoid)
                    && kp.iter().zip(&params).all(|(a, b)| (a - b).abs() < 1e-7 * (1.0 + a.abs()))
            }) && k.unit.as_ref().map_or(1.0, |u| u.metres) == metres
                // A datum named on both sides must agree (NAD83 and WGS 84 share GRS 80-like
                // ellipsoids).
                && match (&c.datum, &k.datum) {
                    (Some(a), Some(b)) => datum_key(a) == datum_key(b),
                    _ => true,
                }
        })
}

fn datum_key(name: &str) -> String {
    let n: String = name
        .to_ascii_uppercase()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    match n.as_str() {
        "NORTHAMERICANDATUM1983" | "DNORTHAMERICAN1983" | "NORTHAMERICAN1983" | "NAD83" => {
            "NAD83".into()
        }
        "WORLDGEODETICSYSTEM1984" | "DWGS1984" | "WGS1984" | "WGS84" => "WGS84".into(),
        _ => n,
    }
}

/// An EPSG code for common ESRI WKT names (`.prj` files carry no authority): UTM zones on
/// WGS 84 / NAD83 / NAD27 / ETRS89, the usual geographic systems and Web Mercator.
pub fn epsg_from_esri_name(name: &str) -> Option<u32> {
    let n = name.to_ascii_uppercase().replace([' ', '-'], "_");
    match n.as_str() {
        "GCS_WGS_1984" | "WGS_84" | "WGS_1984" => return Some(4326),
        "GCS_NORTH_AMERICAN_1983" | "NAD83" => return Some(4269),
        "GCS_NORTH_AMERICAN_1927" | "NAD27" => return Some(4267),
        "GCS_ETRS_1989" | "ETRS89" => return Some(4258),
        "GCS_EUROPEAN_1950" | "ED50" => return Some(4230),
        "GCS_GDA_1994" => return Some(4283),
        "WGS_1984_WEB_MERCATOR_AUXILIARY_SPHERE"
        | "WGS_1984_WEB_MERCATOR"
        | "WGS_84_PSEUDO_MERCATOR" => {
            return Some(3857);
        }
        "WGS_1984_WORLD_MERCATOR" | "WGS_84_WORLD_MERCATOR" => return Some(3395),
        "BRITISH_NATIONAL_GRID"
        | "OSGB_1936_BRITISH_NATIONAL_GRID"
        | "OSGB36_BRITISH_NATIONAL_GRID" => {
            return Some(27700);
        }
        "RGF_1993_LAMBERT_93" | "RGF93_LAMBERT_93" | "RGF93_V1_LAMBERT_93" => return Some(2154),
        "NAD_1983_CONTIGUOUS_USA_ALBERS" | "NAD83_CONUS_ALBERS" | "NAD_1983_CONUS_ALBERS" => {
            return Some(5070);
        }
        _ => {}
    }
    let (base, zone) = n.split_once("_UTM_ZONE_")?;
    let north = zone.ends_with('N');
    let south = zone.ends_with('S');
    let z: u32 = zone.trim_end_matches(['N', 'S']).parse().ok()?;
    if !(1..=60).contains(&z) {
        return None;
    }
    let code = match base {
        "WGS_1984" | "WGS_84" if north => 32600 + z,
        "WGS_1984" | "WGS_84" if south => 32700 + z,
        "NAD_1983" | "NAD83" if north && (3..=23).contains(&z) => 26900 + z,
        "NAD_1927" | "NAD27" if north && (3..=22).contains(&z) => 26700 + z,
        "ETRS_1989" | "ETRS89" if north && (28..=38).contains(&z) => 25800 + z,
        "ED_1950" | "ED50" if north && (28..=38).contains(&z) => 23000 + z,
        _ => return None,
    };
    Some(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table() {
        let c = from_epsg(32633).unwrap();
        assert_eq!(c.name, "WGS 84 / UTM zone 33N");
        let (lon, lat) = c.to_lonlat(500_000.0, 0.0).unwrap();
        assert!((lon - 15.0).abs() < 1e-12 && lat.abs() < 1e-12);
        let ny = from_epsg(2263).unwrap();
        let (x, y) = ny.from_lonlat(-74.0, 40.75).unwrap();
        let (lon, lat) = ny.to_lonlat(x, y).unwrap();
        assert!((lon + 74.0).abs() < 1e-10 && (lat - 40.75).abs() < 1e-10);
        assert!((x - 984_250.0).abs() < 1.0, "{x}");
        assert_eq!(epsg_from_ref("urn:ogc:def:crs:EPSG::3857"), Some(3857));
        // A version number before the code.
        assert_eq!(
            epsg_from_ref("http://www.opengis.net/def/crs/EPSG/0/32633"),
            Some(32633)
        );
        assert_eq!(epsg_from_ref("urn:ogc:def:crs:EPSG:6.6:4326"), Some(4326));
        assert_eq!(epsg_from_ref("EPSG:27700"), Some(27700));
        assert_eq!(
            epsg_from_ref("http://www.opengis.net/def/crs/OGC/1.3/CRS84"),
            Some(4326)
        );
        assert_eq!(epsg_from_esri_name("WGS_1984_UTM_Zone_33N"), Some(32633));
        assert_eq!(epsg_from_esri_name("NAD_1983_UTM_Zone_10N"), Some(26910));
    }
}
