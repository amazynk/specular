//! Web Mercator (EPSG:3857) in world units: the world is the unit square, x east from 180°W,
//! y south from [`MAX_LAT`] north. One world unit is [`crate::TILE`] logical px at zoom 0.

use std::f64::consts::PI;

/// The latitude Web Mercator stops at (the world is then square), degrees.
pub const MAX_LAT: f64 = 85.051_128_779_806_59;

/// World coordinates of a position (degrees, south and west negative). Latitudes past
/// [`MAX_LAT`] are clamped; longitudes are wrapped into [-180, 180).
pub fn project(lat: f64, lon: f64) -> (f64, f64) {
    let lat = lat.clamp(-MAX_LAT, MAX_LAT);
    let lon = (lon + 180.0).rem_euclid(360.0) - 180.0;
    let x = (lon + 180.0) / 360.0;
    let s = lat.to_radians().sin();
    let y = 0.5 - ((1.0 + s) / (1.0 - s)).ln() / (4.0 * PI);
    (x, y.clamp(0.0, 1.0))
}

/// The position (latitude, longitude in degrees) at world coordinates; x wraps.
pub fn unproject(x: f64, y: f64) -> (f64, f64) {
    let lon = x.rem_euclid(1.0) * 360.0 - 180.0;
    let n = PI * (1.0 - 2.0 * y.clamp(0.0, 1.0));
    let lat = n.sinh().atan().to_degrees();
    (lat, lon)
}

/// Metres on the ground per world unit at world latitude `y` (the scale bar's).
pub fn metres_per_unit(y: f64) -> f64 {
    const EQUATOR_M: f64 = 40_075_016.686;
    let (lat, _) = unproject(0.0, y);
    EQUATOR_M * lat.to_radians().cos()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn known_points() {
        assert_eq!(project(0.0, 0.0), (0.5, 0.5));
        let (x, y) = project(MAX_LAT, -180.0);
        assert!(close(x, 0.0, 1e-12) && close(y, 0.0, 1e-9), "{x} {y}");
        let (x, y) = project(-MAX_LAT, 179.999_999);
        assert!(close(x, 1.0, 1e-8) && close(y, 1.0, 1e-9), "{x} {y}");
        // Lisbon: tile (z=10) 486, 392 by the usual slippy-map formula.
        let (x, y) = project(38.7223, -9.1393);
        assert_eq!(((x * 1024.0) as u32, (y * 1024.0) as u32), (486, 392));
        // Poles clamp; longitudes wrap.
        assert_eq!(project(90.0, 0.0).1, 0.0);
        assert_eq!(project(-90.0, 0.0).1, 1.0);
        assert!(close(project(0.0, 190.0).0, project(0.0, -170.0).0, 1e-12));
    }

    #[test]
    fn round_trip() {
        for lat in [-85.0, -60.5, -1.0, 0.0, 12.25, 38.7223, 51.5, 70.0, 85.0] {
            for lon in [-179.5, -90.0, -9.1393, 0.0, 0.001, 45.0, 139.69, 179.9] {
                let (x, y) = project(lat, lon);
                let (a, o) = unproject(x, y);
                assert!(
                    close(a, lat, 1e-9) && close(o, lon, 1e-9),
                    "{lat},{lon} → {a},{o}"
                );
            }
        }
        // x wraps on the way back.
        let (_, o) = unproject(1.25, 0.5);
        assert!(close(o, -90.0, 1e-9));
    }

    #[test]
    fn scale_shrinks_towards_the_poles() {
        let eq = metres_per_unit(0.5);
        assert!(close(eq, 40_075_016.686, 1e-3));
        let (_, y60) = project(60.0, 0.0);
        assert!(close(metres_per_unit(y60) / eq, 0.5, 1e-9));
    }
}
