//! Datum shifts to WGS 84: seven-parameter Helmert transformations (and their three-
//! parameter and Molodensky-Badekas forms) between geocentric coordinates, as the EPSG
//! dataset gives them for each geographic CRS with its area of use.
//!
//! A CRS can have several (ED50 has one per country): like PROJ without its grid files,
//! the shift whose area holds the point is used, the most accurate first, then the one
//! with the largest area. Grid-based shifts (NADCON, NTv2) are not used, so where PROJ has
//! a grid the two differ by up to a few metres (stated in the oracle results).

use crate::proj::Ellipsoid;

/// Arc-seconds to radians.
const ARCSEC: f64 = std::f64::consts::PI / (180.0 * 3600.0);

/// A Helmert transformation from a datum to WGS 84, in the position-vector convention.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Helmert {
    /// Translations (metres).
    pub t: [f64; 3],
    /// Rotations (arc-seconds, position vector).
    pub r: [f64; 3],
    /// Scale difference (parts per million).
    pub ds: f64,
    /// Molodensky-Badekas evaluation point (metres); `None` rotates about the geocentre.
    pub pivot: Option<[f64; 3]>,
}

impl Helmert {
    /// No shift (datums equal to WGS 84 at the metre level, e.g. ETRS89, NAD83, GDA94).
    pub const NULL: Helmert = Helmert {
        t: [0.0; 3],
        r: [0.0; 3],
        ds: 0.0,
        pivot: None,
    };

    /// Geocentric translations only (a TOWGS84 with three values).
    pub fn translation(t: [f64; 3]) -> Helmert {
        Helmert { t, ..Helmert::NULL }
    }

    /// From a WKT1 TOWGS84 clause (3 or 7 values, position vector).
    pub fn from_towgs84(v: &[f64]) -> Option<Helmert> {
        match v.len() {
            3 => Some(Helmert::translation([v[0], v[1], v[2]])),
            n if n >= 7 => Some(Helmert {
                t: [v[0], v[1], v[2]],
                r: [v[3], v[4], v[5]],
                ds: v[6],
                pivot: None,
            }),
            _ => None,
        }
        .filter(|h| h.t.iter().chain(&h.r).all(|x| x.is_finite()) && h.ds.is_finite())
    }

    pub fn is_null(&self) -> bool {
        self.t == [0.0; 3] && self.r == [0.0; 3] && self.ds == 0.0
    }

    /// Geocentric coordinates on the source datum to WGS 84 (`inverse`: the other way).
    fn apply(&self, p: [f64; 3], inverse: bool) -> [f64; 3] {
        let (rx, ry, rz) = (self.r[0] * ARCSEC, self.r[1] * ARCSEC, self.r[2] * ARCSEC);
        let m = 1.0 + self.ds * 1e-6;
        let c = self.pivot.unwrap_or([0.0; 3]);
        if !inverse {
            let (x, y, z) = (p[0] - c[0], p[1] - c[1], p[2] - c[2]);
            [
                c[0] + self.t[0] + m * (x - rz * y + ry * z),
                c[1] + self.t[1] + m * (rz * x + y - rx * z),
                c[2] + self.t[2] + m * (-ry * x + rx * y + z),
            ]
        } else {
            // The exact inverse of the linearised rotation matrix (R⁻¹ by its adjugate).
            let (x, y, z) = (
                (p[0] - c[0] - self.t[0]) / m,
                (p[1] - c[1] - self.t[1]) / m,
                (p[2] - c[2] - self.t[2]) / m,
            );
            let r = [[1.0, -rz, ry], [rz, 1.0, -rx], [-ry, rx, 1.0]];
            let inv = invert3(r);
            [
                c[0] + inv[0][0] * x + inv[0][1] * y + inv[0][2] * z,
                c[1] + inv[1][0] * x + inv[1][1] * y + inv[1][2] * z,
                c[2] + inv[2][0] * x + inv[2][1] * y + inv[2][2] * z,
            ]
        }
    }
}

fn invert3(m: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    let d = 1.0 / det;
    [
        [
            (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * d,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * d,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * d,
        ],
        [
            (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * d,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * d,
            (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * d,
        ],
        [
            (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * d,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * d,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * d,
        ],
    ]
}

/// A Helmert transformation with its area of use (degrees) and accuracy (metres).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shift {
    pub helmert: Helmert,
    /// West, south, east, north; west > east crosses the antimeridian.
    pub area: [f32; 4],
    /// Metres, when stated.
    pub accuracy: Option<f32>,
}

impl Shift {
    fn contains(&self, lon: f64, lat: f64) -> bool {
        let [w, s, e, n] = self.area.map(f64::from);
        let in_lon = if w <= e {
            (w..=e).contains(&lon)
        } else {
            lon >= w || lon <= e
        };
        in_lon && (s..=n).contains(&lat)
    }

    fn size(&self) -> f64 {
        let [w, s, e, n] = self.area.map(f64::from);
        let dl = if w <= e { e - w } else { 360.0 - (w - e) };
        dl * (n - s)
    }
}

/// How a CRS's datum relates to WGS 84.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum ToWgs84 {
    /// Not known: coordinates stay on the CRS's own datum.
    #[default]
    Unknown,
    /// The datum is WGS 84 (or a realisation of it).
    Same,
    /// One shift everywhere (a WKT TOWGS84 clause).
    One(Helmert),
    /// The dataset's shifts, chosen by position.
    Choose(std::sync::Arc<[Shift]>),
}

impl ToWgs84 {
    pub fn is_known(&self) -> bool {
        !matches!(self, ToWgs84::Unknown)
    }

    /// The shift used at a position (longitude, latitude on the source datum).
    pub fn at(&self, lon: f64, lat: f64) -> Option<Helmert> {
        match self {
            ToWgs84::Unknown => None,
            ToWgs84::Same => Some(Helmert::NULL),
            ToWgs84::One(h) => Some(*h),
            ToWgs84::Choose(list) => {
                let acc = |s: &Shift| s.accuracy.map_or(f64::INFINITY, f64::from);
                let inside = list.iter().filter(|s| s.contains(lon, lat)).min_by(|a, b| {
                    acc(a)
                        .total_cmp(&acc(b))
                        .then(b.size().total_cmp(&a.size()))
                });
                inside
                    .or_else(|| list.iter().max_by(|a, b| a.size().total_cmp(&b.size())))
                    .map(|s| s.helmert)
            }
        }
    }
}

/// Geodetic (degrees, metres above the ellipsoid) to geocentric (metres).
pub fn to_geocentric(ell: Ellipsoid, lon: f64, lat: f64, h: f64) -> [f64; 3] {
    let (sp, cp) = lat.to_radians().sin_cos();
    let (sl, cl) = lon.to_radians().sin_cos();
    let e2 = ell.e2();
    let n = ell.a / (1.0 - e2 * sp * sp).sqrt();
    [
        (n + h) * cp * cl,
        (n + h) * cp * sl,
        (n * (1.0 - e2) + h) * sp,
    ]
}

/// Geocentric (metres) to geodetic (degrees, metres): Bowring's start and two Newton-like
/// refinements, well below a millimetre near the surface.
pub fn from_geocentric(ell: Ellipsoid, p: [f64; 3]) -> (f64, f64, f64) {
    let [x, y, z] = p;
    let e2 = ell.e2();
    let a = ell.a;
    let b = a * (1.0 - ell.f);
    let lon = y.atan2(x);
    let r = x.hypot(y);
    if r < 1e-9 {
        let lat = 90f64.copysign(z);
        return (lon.to_degrees(), lat, z.abs() - b);
    }
    let ep2 = (a * a - b * b) / (b * b);
    let th = (z * a).atan2(r * b);
    let (st, ct) = th.sin_cos();
    let mut lat = (z + ep2 * b * st * st * st).atan2(r - e2 * a * ct * ct * ct);
    let mut h = 0.0;
    for _ in 0..3 {
        let s = lat.sin();
        let n = a / (1.0 - e2 * s * s).sqrt();
        h = r / lat.cos() - n;
        lat = (z / r / (1.0 - e2 * n / (n + h))).atan();
    }
    (lon.to_degrees(), lat.to_degrees(), h)
}

/// Longitude and latitude on a datum (ellipsoid `ell`) to WGS 84 by `h`, at height 0.
pub fn shift_to_wgs84(ell: Ellipsoid, h: &Helmert, lon: f64, lat: f64) -> (f64, f64) {
    if h.is_null() {
        return (lon, lat);
    }
    let p = h.apply(to_geocentric(ell, lon, lat, 0.0), false);
    let (lo, la, _) = from_geocentric(Ellipsoid::WGS84, p);
    (lo, la)
}

/// Longitude and latitude on WGS 84 to the datum (ellipsoid `ell`) of `h`, at height 0.
pub fn shift_from_wgs84(ell: Ellipsoid, h: &Helmert, lon: f64, lat: f64) -> (f64, f64) {
    if h.is_null() {
        return (lon, lat);
    }
    let p = h.apply(to_geocentric(Ellipsoid::WGS84, lon, lat, 0.0), true);
    let (lo, la, _) = from_geocentric(ell, p);
    (lo, la)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geocentric_round_trip() {
        for &(lon, lat, h) in &[
            (10.0, 50.0, 0.0),
            (-120.5, -33.2, 1234.0),
            (179.9, 89.0, 5.0),
        ] {
            let p = to_geocentric(Ellipsoid::WGS84, lon, lat, h);
            let (lo, la, hh) = from_geocentric(Ellipsoid::WGS84, p);
            assert!(
                (lo - lon).abs() < 1e-11 && (la - lat).abs() < 1e-11 && (hh - h).abs() < 1e-6,
                "{lo} {la} {hh}"
            );
        }
    }

    #[test]
    fn helmert_round_trip_and_guidance_example() {
        // EPSG guidance note 7-2, §2.4.3.3: WGS 72 to WGS 84 (position vector).
        let h = Helmert {
            t: [0.0, 0.0, 4.5],
            r: [0.0, 0.0, 0.554],
            ds: 0.219,
            pivot: None,
        };
        let p = h.apply([3_657_660.66, 255_768.55, 5_201_382.11], false);
        assert!(
            (p[0] - 3_657_660.78).abs() < 0.01
                && (p[1] - 255_778.43).abs() < 0.01
                && (p[2] - 5_201_387.75).abs() < 0.01,
            "{p:?}"
        );
        let back = h.apply(p, true);
        assert!((back[1] - 255_768.55).abs() < 1e-6);
        // Both ways at height 0, as PROJ does in 2D: the height the shift adds is dropped,
        // so the way back lands within a few millimetres.
        let (lo, la) = shift_to_wgs84(Ellipsoid::INTL1924, &h, 4.0, 52.0);
        let (lo2, la2) = shift_from_wgs84(Ellipsoid::INTL1924, &h, lo, la);
        assert!(
            (lo2 - 4.0).abs() < 1e-7 && (la2 - 52.0).abs() < 1e-7,
            "{lo2} {la2}"
        );
    }
}
