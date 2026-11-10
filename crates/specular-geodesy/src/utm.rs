//! UTM and UPS on WGS 84.
//!
//! Transverse Mercator through the conformal latitude and Krüger's series in the third
//! flattening to sixth order (accurate to a few nanometres within a zone and to well under a
//! millimetre several zones out); UPS as the polar stereographic projection (scale 0.994,
//! false easting and northing 2 000 000 m). The standard zone follows the MGRS rules,
//! including the widened zones of south-west Norway (32V) and Svalbard (31X, 33X, 35X, 37X).

use crate::{WGS84_A, WGS84_F};
use std::sync::OnceLock;

/// A UTM or UPS position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Utm {
    /// 1..=60; 0 for UPS.
    pub zone: u8,
    /// Northern hemisphere (UTM) or north pole (UPS).
    pub north: bool,
    pub easting: f64,
    pub northing: f64,
}

const K0: f64 = 0.9996;
const UPS_K0: f64 = 0.994;
const FALSE_EASTING: f64 = 500_000.0;
const FALSE_NORTHING_SOUTH: f64 = 10_000_000.0;
const UPS_FALSE: f64 = 2_000_000.0;

/// The latitude band letters from 80° S, 8° each (X spans 72° N to 84° N).
pub(crate) const BANDS: &[u8; 20] = b"CDEFGHJKLMNPQRSTUVWX";

struct Tm {
    e: f64,
    e2: f64,
    /// Rectifying radius A = a/(1 + n) (1 + n²/4 + n⁴/64 + n⁶/256).
    big_a: f64,
    alpha: [f64; 7],
    beta: [f64; 7],
    /// UPS: 2a / √((1 + e)^(1+e) (1 − e)^(1−e)).
    ups_c: f64,
}

fn tm() -> &'static Tm {
    static T: OnceLock<Tm> = OnceLock::new();
    T.get_or_init(|| {
        let f = WGS84_F;
        let a = WGS84_A;
        let e2 = f * (2.0 - f);
        let e = e2.sqrt();
        let n = f / (2.0 - f);
        let n2 = n * n;
        let (n3, n4, n5, n6) = (n2 * n, n2 * n2, n2 * n2 * n, n2 * n2 * n2);
        let big_a = a / (1.0 + n) * (1.0 + n2 / 4.0 + n4 / 64.0 + n6 / 256.0);
        let alpha = [
            0.0,
            n / 2.0 - 2.0 / 3.0 * n2 + 5.0 / 16.0 * n3 + 41.0 / 180.0 * n4 - 127.0 / 288.0 * n5
                + 7891.0 / 37800.0 * n6,
            13.0 / 48.0 * n2 - 3.0 / 5.0 * n3 + 557.0 / 1440.0 * n4 + 281.0 / 630.0 * n5
                - 1_983_433.0 / 1_935_360.0 * n6,
            61.0 / 240.0 * n3 - 103.0 / 140.0 * n4
                + 15061.0 / 26880.0 * n5
                + 167_603.0 / 181_440.0 * n6,
            49561.0 / 161_280.0 * n4 - 179.0 / 168.0 * n5 + 6_601_661.0 / 7_257_600.0 * n6,
            34729.0 / 80640.0 * n5 - 3_418_889.0 / 1_995_840.0 * n6,
            212_378_941.0 / 319_334_400.0 * n6,
        ];
        let beta = [
            0.0,
            n / 2.0 - 2.0 / 3.0 * n2 + 37.0 / 96.0 * n3 - 1.0 / 360.0 * n4 - 81.0 / 512.0 * n5
                + 96199.0 / 604_800.0 * n6,
            n2 / 48.0 + n3 / 15.0 - 437.0 / 1440.0 * n4 + 46.0 / 105.0 * n5
                - 1_118_711.0 / 3_870_720.0 * n6,
            17.0 / 480.0 * n3 - 37.0 / 840.0 * n4 - 209.0 / 4480.0 * n5 + 5569.0 / 90720.0 * n6,
            4397.0 / 161_280.0 * n4 - 11.0 / 504.0 * n5 - 830_251.0 / 7_257_600.0 * n6,
            4583.0 / 161_280.0 * n5 - 108_847.0 / 3_991_680.0 * n6,
            20_648_693.0 / 638_668_800.0 * n6,
        ];
        let ups_c = 2.0 * a / ((1.0 + e).powf(1.0 + e) * (1.0 - e).powf(1.0 - e)).sqrt();
        Tm {
            e,
            e2,
            big_a,
            alpha,
            beta,
            ups_c,
        }
    })
}

impl Tm {
    /// tan of the conformal latitude from tan φ.
    fn taup(&self, tau: f64) -> f64 {
        let tau1 = tau.hypot(1.0);
        let sig = (self.e * (self.e * tau / tau1).atanh()).sinh();
        tau * sig.hypot(1.0) - sig * tau1
    }

    /// tan φ from tan of the conformal latitude (Newton's method).
    fn tau(&self, taup: f64) -> f64 {
        let mut tau = taup / (1.0 - self.e2);
        for _ in 0..8 {
            let tp = self.taup(tau);
            let d = (taup - tp) * (1.0 + (1.0 - self.e2) * tau * tau)
                / ((1.0 - self.e2) * tp.hypot(1.0) * tau.hypot(1.0));
            tau += d;
            if d.abs() <= 1e-15 * tau.abs().max(1.0) {
                break;
            }
        }
        tau
    }

    /// Transverse Mercator (k0 = 1) of latitude `lat`, longitude `dlon` from the central
    /// meridian (degrees): (x, y) metres.
    fn forward(&self, lat: f64, dlon: f64) -> (f64, f64) {
        let lam = dlon.to_radians();
        let (sl, cl) = lam.sin_cos();
        let (xip, etap) = if lat.abs() >= 90.0 {
            (std::f64::consts::FRAC_PI_2.copysign(lat), 0.0)
        } else {
            let taup = self.taup(lat.to_radians().tan());
            (taup.atan2(cl), (sl / taup.hypot(cl)).asinh())
        };
        let (mut xi, mut eta) = (xip, etap);
        for j in 1..=6 {
            let k = 2.0 * j as f64;
            xi += self.alpha[j] * (k * xip).sin() * (k * etap).cosh();
            eta += self.alpha[j] * (k * xip).cos() * (k * etap).sinh();
        }
        (self.big_a * eta, self.big_a * xi)
    }

    /// The inverse of [`Tm::forward`]: (latitude, longitude from the central meridian).
    fn inverse(&self, x: f64, y: f64) -> (f64, f64) {
        let xi = y / self.big_a;
        let eta = x / self.big_a;
        let (mut xip, mut etap) = (xi, eta);
        for j in 1..=6 {
            let k = 2.0 * j as f64;
            xip -= self.beta[j] * (k * xi).sin() * (k * eta).cosh();
            etap -= self.beta[j] * (k * xi).cos() * (k * eta).sinh();
        }
        let (sx, cx) = xip.sin_cos();
        let sh = etap.sinh();
        let r = sh.hypot(cx);
        let lam = sh.atan2(cx);
        if r == 0.0 {
            return (90f64.copysign(sx), 0.0);
        }
        let taup = sx / r;
        let tau = self.tau(taup);
        (tau.atan().to_degrees(), lam.to_degrees())
    }
}

/// The latitude band letter, 80° S to 84° N.
pub fn band(lat: f64) -> Option<char> {
    if !(-80.0..=84.0).contains(&lat) {
        return None;
    }
    let i = (((lat + 80.0) / 8.0).floor() as i64).clamp(0, 19) as usize;
    Some(BANDS[i] as char)
}

/// The southern edge of a band, degrees.
pub(crate) fn band_south(letter: char) -> Option<f64> {
    let i = BANDS.iter().position(|&b| b as char == letter)?;
    Some(-80.0 + 8.0 * i as f64)
}

/// The UTM zone of a point (with the Norway and Svalbard exceptions); `None` outside
/// 80° S..84° N.
pub fn utm_zone(lat: f64, lon: f64) -> Option<u8> {
    if !(-80.0..=84.0).contains(&lat) || !lon.is_finite() {
        return None;
    }
    let lon = crate::angle::normalize(lon);
    let lon = if lon == 180.0 { -180.0 } else { lon };
    let mut zone = (((lon + 180.0) / 6.0).floor() as i64).clamp(0, 59) + 1;
    if (56.0..64.0).contains(&lat) && (3.0..12.0).contains(&lon) {
        zone = 32;
    }
    if lat >= 72.0 && (0.0..42.0).contains(&lon) {
        zone = if lon < 9.0 {
            31
        } else if lon < 21.0 {
            33
        } else if lon < 33.0 {
            35
        } else {
            37
        };
    }
    Some(zone as u8)
}

/// The central meridian of a zone, degrees.
pub(crate) fn central_meridian(zone: u8) -> f64 {
    f64::from(zone) * 6.0 - 183.0
}

/// UTM in the point's standard zone, or UPS outside 80° S..84° N.
pub fn to_utm(lat: f64, lon: f64) -> Option<Utm> {
    if !(lat.is_finite() && lon.is_finite()) || lat.abs() > 90.0 {
        return None;
    }
    match utm_zone(lat, lon) {
        Some(z) => to_utm_in(lat, lon, z, lat >= 0.0),
        None => to_utm_in(lat, lon, 0, lat > 0.0),
    }
}

/// UTM in a given zone and hemisphere (zone 0: UPS about the given pole).
pub fn to_utm_in(lat: f64, lon: f64, zone: u8, north: bool) -> Option<Utm> {
    if !(lat.is_finite() && lon.is_finite()) || lat.abs() > 90.0 || zone > 60 {
        return None;
    }
    let t = tm();
    if zone == 0 {
        // Polar stereographic: ρ = C · tan(π/4 − χ/2), χ the conformal latitude.
        let phi = if north { lat } else { -lat };
        let rho = if phi >= 90.0 {
            0.0
        } else {
            let taup = t.taup(phi.to_radians().tan());
            // tan(π/4 − χ/2) = √(1 + τ′²) − τ′, written without cancellation.
            let tq = if taup >= 0.0 {
                1.0 / (taup.hypot(1.0) + taup)
            } else {
                taup.hypot(1.0) - taup
            };
            UPS_K0 * t.ups_c * tq
        };
        let (s, c) = crate::angle::sincosd(lon);
        let x = UPS_FALSE + rho * s;
        let y = if north {
            UPS_FALSE - rho * c
        } else {
            UPS_FALSE + rho * c
        };
        return Some(Utm {
            zone: 0,
            north,
            easting: x,
            northing: y,
        });
    }
    let dlon = crate::angle::diff(central_meridian(zone), lon);
    let (x, y) = t.forward(lat, dlon);
    Some(Utm {
        zone,
        north,
        easting: FALSE_EASTING + K0 * x,
        northing: K0 * y + if north { 0.0 } else { FALSE_NORTHING_SOUTH },
    })
}

/// Latitude and longitude of a UTM or UPS position.
pub fn from_utm(u: &Utm) -> Option<(f64, f64)> {
    if !(u.easting.is_finite() && u.northing.is_finite()) || u.zone > 60 {
        return None;
    }
    let t = tm();
    if u.zone == 0 {
        let x = u.easting - UPS_FALSE;
        let y = u.northing - UPS_FALSE;
        let rho = x.hypot(y);
        let tq = rho / (UPS_K0 * t.ups_c);
        // τ′ = tan χ with tan(π/4 − χ/2) = tq: τ′ = (1 − tq²) / (2 tq).
        let lat = if rho == 0.0 {
            90.0
        } else {
            let taup = (1.0 - tq * tq) / (2.0 * tq);
            t.tau(taup).atan().to_degrees()
        };
        let lon = if rho == 0.0 {
            0.0
        } else if u.north {
            x.atan2(-y).to_degrees()
        } else {
            x.atan2(y).to_degrees()
        };
        let lat = if u.north { lat } else { -lat };
        return Some((lat, crate::angle::normalize(lon) + 0.0));
    }
    let x = (u.easting - FALSE_EASTING) / K0;
    let y = (u.northing - if u.north { 0.0 } else { FALSE_NORTHING_SOUTH }) / K0;
    let (lat, dlon) = t.inverse(x, y);
    let lon = crate::angle::normalize(central_meridian(u.zone) + dlon);
    (lat.is_finite() && lon.is_finite()).then_some((lat, lon))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zones_and_bands() {
        assert_eq!(utm_zone(47.0, 8.5), Some(32));
        assert_eq!(utm_zone(60.0, 4.0), Some(32)); // Norway
        assert_eq!(utm_zone(60.0, 2.0), Some(31));
        assert_eq!(utm_zone(78.0, 8.0), Some(31)); // Svalbard
        assert_eq!(utm_zone(78.0, 10.0), Some(33));
        assert_eq!(utm_zone(78.0, 40.0), Some(37));
        assert_eq!(utm_zone(0.0, 180.0), Some(1));
        assert_eq!(utm_zone(0.0, -180.0), Some(1));
        assert_eq!(utm_zone(0.0, 179.9), Some(60));
        assert_eq!(utm_zone(85.0, 0.0), None);
        assert_eq!(band(47.0), Some('T'));
        assert_eq!(band(84.0), Some('X'));
        assert_eq!(band(-80.0), Some('C'));
        assert_eq!(band(0.0), Some('N'));
        assert_eq!(band(-0.1), Some('M'));
    }

    #[test]
    fn central_meridian_and_round_trip() {
        let u = to_utm(0.0, 9.0).unwrap();
        assert_eq!(u.zone, 32);
        assert!((u.easting - 500_000.0).abs() < 1e-9 && u.northing.abs() < 1e-9);
        for (lat, lon) in [(47.3, 8.5), (-33.9, 151.2), (83.9, -40.0), (-79.9, 170.0)] {
            let u = to_utm(lat, lon).unwrap();
            let (la, lo) = from_utm(&u).unwrap();
            assert!(
                (la - lat).abs() < 1e-11 && (lo - lon).abs() < 1e-11,
                "{lat} {lon}"
            );
        }
        for (lat, lon) in [(89.0, 30.0), (-85.0, -120.0), (90.0, 0.0), (84.5, 179.0)] {
            let u = to_utm(lat, lon).unwrap();
            assert_eq!(u.zone, 0);
            let (la, lo) = from_utm(&u).unwrap();
            assert!((la - lat).abs() < 1e-11, "{lat} {la}");
            if lat.abs() < 90.0 {
                assert!((lo - lon).abs() < 1e-9, "{lon} {lo}");
            }
        }
    }
}
