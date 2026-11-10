//! Map projections, forward and inverse, on an ellipsoid. All exact closed forms or series
//! accurate far below a millimetre in their domains:
//!
//! - Transverse Mercator (UTM, national grids): Krüger's series to sixth order in the third
//!   flattening (Karney 2011), good to nanometres within 3900 km of the central meridian;
//! - Mercator (variant A, ellipsoidal) and Web Mercator ("Popular Visualisation Pseudo
//!   Mercator": the spherical formulas on the semi-major axis);
//! - Lambert conformal conic, one and two standard parallels;
//! - Albers equal-area conic;
//! - polar stereographic (variant B: scale true on a standard parallel);
//! - the rotated pole of limited-area weather models (rotated longitude and latitude in
//!   degrees, a rotation of the sphere rather than a projection);
//! - sinusoidal (MODIS land tiles use it on a sphere), Lambert azimuthal equal-area (EASE
//!   grids, ETRS89-LAEA) and equirectangular (plate carrée with a standard parallel).
//!
//! The ellipsoidal latitude is recovered from the conformal or authalic one by Newton's
//! method to machine precision. Angles are in degrees at the interface, lengths in metres.

use std::f64::consts::{FRAC_PI_2, FRAC_PI_4};

/// A reference ellipsoid: semi-major axis (metres) and flattening (0 for a sphere).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ellipsoid {
    pub a: f64,
    pub f: f64,
}

impl Ellipsoid {
    pub const WGS84: Ellipsoid = Ellipsoid::inv(6_378_137.0, 298.257_223_563);
    pub const GRS80: Ellipsoid = Ellipsoid::inv(6_378_137.0, 298.257_222_101);
    pub const CLARKE1866: Ellipsoid = Ellipsoid::inv(6_378_206.4, 294.978_698_213_898);
    pub const INTL1924: Ellipsoid = Ellipsoid::inv(6_378_388.0, 297.0);
    pub const AIRY1830: Ellipsoid = Ellipsoid::inv(6_377_563.396, 299.324_964_6);
    pub const BESSEL1841: Ellipsoid = Ellipsoid::inv(6_377_397.155, 299.152_812_8);
    pub const KRASSOVSKY: Ellipsoid = Ellipsoid::inv(6_378_245.0, 298.3);
    pub const WGS72: Ellipsoid = Ellipsoid::inv(6_378_135.0, 298.26);
    pub const SPHERE: Ellipsoid = Ellipsoid {
        a: 6_378_137.0,
        f: 0.0,
    };

    /// From the semi-major axis and the inverse flattening (0 or infinite: a sphere).
    pub const fn inv(a: f64, rf: f64) -> Ellipsoid {
        Ellipsoid {
            a,
            f: if rf == 0.0 || rf.is_infinite() {
                0.0
            } else {
                1.0 / rf
            },
        }
    }

    /// First eccentricity squared.
    pub fn e2(&self) -> f64 {
        self.f * (2.0 - self.f)
    }

    pub fn e(&self) -> f64 {
        self.e2().sqrt()
    }

    /// Radius of curvature in the prime vertical and in the meridian at latitude `lat`
    /// (degrees): the lengths of a radian along a parallel (× cos lat) and a meridian.
    pub fn radii(&self, lat: f64) -> (f64, f64) {
        let e2 = self.e2();
        let s = lat.to_radians().sin();
        let w = (1.0 - e2 * s * s).sqrt();
        (self.a / w, self.a * (1.0 - e2) / (w * w * w))
    }
}

/// A projection method with its parameters (degrees, metres).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Method {
    TransverseMercator {
        lat0: f64,
        lon0: f64,
        k0: f64,
        fe: f64,
        fn_: f64,
    },
    /// Mercator variant A (one standard parallel at the equator, scale `k0`).
    Mercator {
        lon0: f64,
        k0: f64,
        fe: f64,
        fn_: f64,
    },
    /// Mercator variant B (scale given by a standard parallel `lat1`).
    MercatorB {
        lon0: f64,
        lat1: f64,
        fe: f64,
        fn_: f64,
    },
    /// EPSG:3857's method: spherical Mercator on the ellipsoid's semi-major axis.
    WebMercator { lon0: f64, fe: f64, fn_: f64 },
    /// Lambert conformal conic: two standard parallels (`k0` 1, false origin at `lat0`), or
    /// one (`lat1 == lat2 == lat0`, natural origin, scale `k0`).
    LambertConic {
        lat0: f64,
        lon0: f64,
        lat1: f64,
        lat2: f64,
        k0: f64,
        fe: f64,
        fn_: f64,
    },
    Albers {
        lat0: f64,
        lon0: f64,
        lat1: f64,
        lat2: f64,
        fe: f64,
        fn_: f64,
    },
    /// Polar stereographic variant B: scale true on `lat_ts` (±90: at the pole), centred on
    /// the north pole, or the south pole with `south`; `lon0` points down from the pole.
    PolarStereographic {
        lat_ts: f64,
        lon0: f64,
        south: bool,
        fe: f64,
        fn_: f64,
    },
    /// A rotated pole: x and y are the rotated longitude and latitude as arc lengths on the
    /// semi-major axis (a CRS gives them in degrees with a unit of a·π/180 metres), with the
    /// rotated grid's south pole at geographic (`pole_lat`, `pole_lon`) and a further rotation
    /// of `angle` degrees about the new polar axis (GRIB's convention).
    RotatedPole {
        pole_lat: f64,
        pole_lon: f64,
        angle: f64,
    },
    /// Sinusoidal (Sanson-Flamsteed): equal-area, parallels true to scale.
    Sinusoidal { lon0: f64, fe: f64, fn_: f64 },
    /// Lambert azimuthal equal-area, centred on (`lat0`, `lon0`).
    LambertAzimuthal {
        lat0: f64,
        lon0: f64,
        fe: f64,
        fn_: f64,
    },
    /// Equirectangular: x proportional to longitude (true to scale on `lat_ts`), y to
    /// latitude; spherical, on the semi-major axis.
    Equirectangular {
        lat_ts: f64,
        lon0: f64,
        fe: f64,
        fn_: f64,
    },
    /// Transverse Mercator (South Orientated): westing and southing, as in southern Africa.
    TransverseMercatorSouth {
        lat0: f64,
        lon0: f64,
        k0: f64,
        fe: f64,
        fn_: f64,
    },
    /// Polar stereographic variant A: centred on a pole with scale `k0` there.
    PolarStereographicA {
        south: bool,
        lon0: f64,
        k0: f64,
        fe: f64,
        fn_: f64,
    },
    /// Oblique stereographic (the double projection through Gauss's conformal sphere, as
    /// for the Netherlands' RD New).
    ObliqueStereographic {
        lat0: f64,
        lon0: f64,
        k0: f64,
        fe: f64,
        fn_: f64,
    },
    /// Hotine oblique Mercator: variant A has its false origin at the natural origin,
    /// variant B (`centre`) at the projection centre (`latc`, `lonc`). `alpha` is the
    /// azimuth of the initial line, `gamma` the angle from the rectified to the skew grid.
    HotineObliqueMercator {
        latc: f64,
        lonc: f64,
        alpha: f64,
        gamma: f64,
        kc: f64,
        fe: f64,
        fn_: f64,
        centre: bool,
    },
    /// Equidistant cylindrical on the ellipsoid (EPSG 1028, WGS 84 / World Equidistant
    /// Cylindrical): x along the standard parallel's radius, y the meridian distance.
    EquidistantCylindrical {
        lat_ts: f64,
        lon0: f64,
        fe: f64,
        fn_: f64,
    },
    /// Cassini-Soldner (the ellipsoidal series).
    CassiniSoldner {
        lat0: f64,
        lon0: f64,
        fe: f64,
        fn_: f64,
    },
    /// Lambert cylindrical equal-area, true to scale on the parallels ±`lat_ts` (EPSG 9835;
    /// on a sphere 9834): EASE-Grid (spherical, ±30°) and EASE-Grid 2.0 (WGS 84, ±30°).
    CylindricalEqualArea {
        lat_ts: f64,
        lon0: f64,
        fe: f64,
        fn_: f64,
    },
}

impl Method {
    /// A short English name of the method.
    pub fn name(&self) -> &'static str {
        match self {
            Method::TransverseMercator { .. } => "Transverse Mercator",
            Method::Mercator { .. } | Method::MercatorB { .. } => "Mercator",
            Method::WebMercator { .. } => "Popular Visualisation Pseudo Mercator",
            Method::LambertConic { lat1, lat2, .. } if lat1 == lat2 => {
                "Lambert Conic Conformal (1SP)"
            }
            Method::LambertConic { .. } => "Lambert Conic Conformal (2SP)",
            Method::Albers { .. } => "Albers Equal Area",
            Method::PolarStereographic { .. } => "Polar Stereographic (variant B)",
            Method::RotatedPole { .. } => "Rotated pole",
            Method::Sinusoidal { .. } => "Sinusoidal",
            Method::LambertAzimuthal { .. } => "Lambert Azimuthal Equal Area",
            Method::Equirectangular { .. } => "Equidistant Cylindrical",
            Method::TransverseMercatorSouth { .. } => "Transverse Mercator (South Orientated)",
            Method::PolarStereographicA { .. } => "Polar Stereographic (variant A)",
            Method::ObliqueStereographic { .. } => "Oblique Stereographic",
            Method::HotineObliqueMercator { centre: false, .. } => {
                "Hotine Oblique Mercator (variant A)"
            }
            Method::HotineObliqueMercator { centre: true, .. } => {
                "Hotine Oblique Mercator (variant B)"
            }
            Method::CassiniSoldner { .. } => "Cassini-Soldner",
            Method::CylindricalEqualArea { .. } => "Lambert Cylindrical Equal Area",
            Method::EquidistantCylindrical { .. } => "Equidistant Cylindrical",
        }
    }

    /// UTM zone `zone` (1..=60), north or south.
    pub fn utm(zone: u32, north: bool) -> Method {
        Method::TransverseMercator {
            lat0: 0.0,
            lon0: zone as f64 * 6.0 - 183.0,
            k0: 0.9996,
            fe: 500_000.0,
            fn_: if north { 0.0 } else { 10_000_000.0 },
        }
    }
}

/// A projection: a method on an ellipsoid, with the constants it needs precomputed.
#[derive(Clone, Debug, PartialEq)]
pub struct Projection {
    pub method: Method,
    pub ellipsoid: Ellipsoid,
    k: Consts,
}

#[derive(Clone, Debug, PartialEq)]
enum Consts {
    Tm {
        /// Rectifying radius A, the meridian distance of the origin (A ξ₀).
        a_r: f64,
        m0: f64,
        alpha: [f64; 6],
        beta: [f64; 6],
    },
    /// Conic: cone constant n, F (Lambert) or C (Albers), and ρ₀.
    Cone {
        n: f64,
        c: f64,
        rho0: f64,
    },
    /// Mercator: the scale on the equator (k0 or from the standard parallel).
    Merc {
        k: f64,
    },
    /// Polar stereographic: ρ = `k` · t.
    Polar {
        k: f64,
    },
    /// Rotated pole: the sine and cosine of the tilt 90° + `pole_lat`.
    Rot {
        sin: f64,
        cos: f64,
    },
    /// Sinusoidal: the meridian distance series (Snyder 3-21) and its inverse's
    /// coefficients (3-26).
    Sinu {
        m: [f64; 4],
        inv: [f64; 4],
    },
    /// Lambert azimuthal: q at the pole, the authalic radius, β₁ and D (Snyder 24).
    Laea {
        qp: f64,
        rq: f64,
        sin_b1: f64,
        cos_b1: f64,
        d: f64,
    },
    /// Oblique stereographic: the conformal sphere's radius R, n, c, χ₀ (Gauss).
    Sterea {
        r: f64,
        n: f64,
        c: f64,
        chi0: f64,
    },
    /// Hotine oblique Mercator (EPSG guidance note 7-2, §3.2.4); `lon0` in radians.
    Omerc {
        a: f64,
        b: f64,
        h: f64,
        gamma0: f64,
        lon0: f64,
        uc: f64,
    },
    /// Cassini-Soldner: the meridian distance series and the origin's meridian distance.
    Cass {
        m: [f64; 4],
        m0: f64,
    },
    None,
}

/// The meridian distance series of Snyder 3-21.
fn meridian_series(e2: f64) -> [f64; 4] {
    let (e4, e6) = (e2 * e2, e2 * e2 * e2);
    [
        1.0 - e2 / 4.0 - 3.0 * e4 / 64.0 - 5.0 * e6 / 256.0,
        3.0 * e2 / 8.0 + 3.0 * e4 / 32.0 + 45.0 * e6 / 1024.0,
        15.0 * e4 / 256.0 + 45.0 * e6 / 1024.0,
        35.0 * e6 / 3072.0,
    ]
}

/// Hotine's t: tan(π/4 − φ/2) / ((1 − e sin φ)/(1 + e sin φ))^(e/2).
fn t_hotine(phi: f64, e: f64) -> f64 {
    let s = phi.sin();
    (FRAC_PI_4 - phi / 2.0).tan() / ((1.0 - e * s) / (1.0 + e * s)).powf(e / 2.0)
}

/// Latitude from the conformal latitude χ (the series of EPSG guidance note 7-2), then
/// Newton on the isometric latitude to machine precision.
fn lat_of_chi(chi: f64, e: f64) -> f64 {
    lat_of_psi(psi_of_tau(chi.tan(), 0.0), e)
}

/// ψ (isometric latitude) from tan φ.
fn psi_of_tau(tau: f64, e: f64) -> f64 {
    let s = tau / tau.hypot(1.0);
    tau.asinh() - e * (e * s).atanh()
}

/// τ' (tan of the conformal latitude) from τ = tan φ (Karney 2011, eq. 7).
fn taup(tau: f64, e: f64) -> f64 {
    let tau1 = tau.hypot(1.0);
    let sig = (e * (e * tau / tau1).atanh()).sinh();
    tau * sig.hypot(1.0) - sig * tau1
}

/// τ = tan φ from τ' by Newton's method (Karney 2011, eqs. 19–21).
fn tau_of_taup(taup_: f64, e: f64) -> f64 {
    let e2m = 1.0 - e * e;
    let mut tau = taup_ / e2m;
    let stol = 0.1 * f64::EPSILON.sqrt() * taup_.abs().max(1.0);
    for _ in 0..12 {
        let tp = taup(tau, e);
        let dtau = (taup_ - tp) * (1.0 + e2m * tau * tau) / (e2m * tau.hypot(1.0) * tp.hypot(1.0));
        tau += dtau;
        if dtau.abs() < stol {
            break;
        }
    }
    tau
}

/// Latitude (radians) from the isometric latitude ψ.
fn lat_of_psi(psi: f64, e: f64) -> f64 {
    tau_of_taup(psi.sinh(), e).atan()
}

/// Lambert's m = cos φ / √(1 − e² sin² φ).
fn m_of(phi: f64, e2: f64) -> f64 {
    let s = phi.sin();
    phi.cos() / (1.0 - e2 * s * s).sqrt()
}

/// Lambert's t = exp(−ψ).
fn t_of(phi: f64, e: f64) -> f64 {
    (-psi_of_tau(phi.tan(), e)).exp()
}

/// Albers's q (twice the sine of the authalic latitude, scaled).
fn q_of(phi: f64, e: f64) -> f64 {
    let s = phi.sin();
    if e == 0.0 {
        return 2.0 * s;
    }
    let e2 = e * e;
    (1.0 - e2) * (s / (1.0 - e2 * s * s) + (e * s).atanh() / e)
}

impl Projection {
    pub fn new(method: Method, ellipsoid: Ellipsoid) -> Projection {
        let e = ellipsoid.e();
        let e2 = e * e;
        let k = match method {
            Method::TransverseMercatorSouth {
                lat0,
                lon0,
                k0,
                fe,
                fn_,
            } => {
                // The northern form's constants (forward and inverse mirror its results).
                let north = Method::TransverseMercator {
                    lat0,
                    lon0,
                    k0,
                    fe,
                    fn_,
                };
                return Projection {
                    method,
                    ellipsoid,
                    k: Projection::new(north, ellipsoid).k,
                };
            }
            Method::PolarStereographicA { k0, .. } => Consts::Polar {
                k: k0 * 2.0 * ellipsoid.a
                    / ((1.0 + e).powf(1.0 + e) * (1.0 - e).powf(1.0 - e)).sqrt(),
            },
            Method::ObliqueStereographic { lat0, .. } => {
                let p0 = lat0.to_radians();
                let (s0, c0) = p0.sin_cos();
                let (nu0, rho0) = ellipsoid.radii(lat0);
                let r = (nu0 * rho0).sqrt();
                let n = (1.0 + e2 * c0.powi(4) / (1.0 - e2)).sqrt();
                let s1 = (1.0 + s0) / (1.0 - s0);
                let s2 = (1.0 - e * s0) / (1.0 + e * s0);
                let w1 = (s1 * s2.powf(e)).powf(n);
                let sin_chi0 = (w1 - 1.0) / (w1 + 1.0);
                let c = (n + s0) * (1.0 - sin_chi0) / ((n - s0) * (1.0 + sin_chi0));
                let w2 = c * w1;
                let chi0 = ((w2 - 1.0) / (w2 + 1.0)).asin();
                Consts::Sterea { r, n, c, chi0 }
            }
            Method::HotineObliqueMercator {
                latc,
                lonc,
                alpha,
                kc,
                centre,
                ..
            } => {
                let pc = latc.to_radians();
                let (sc, cc) = pc.sin_cos();
                let b = (1.0 + e2 * cc.powi(4) / (1.0 - e2)).sqrt();
                let a = ellipsoid.a * b * kc * (1.0 - e2).sqrt() / (1.0 - e2 * sc * sc);
                let t0 = t_hotine(pc, e);
                let d = (b * (1.0 - e2).sqrt() / (cc * (1.0 - e2 * sc * sc).sqrt())).max(1.0);
                let sign = if pc < 0.0 { -1.0 } else { 1.0 };
                let f = d + (d * d - 1.0).sqrt() * sign;
                let h = f * t0.powf(b);
                let g = (f - 1.0 / f) / 2.0;
                let al = alpha.to_radians();
                let gamma0 = (al.sin() / d).clamp(-1.0, 1.0).asin();
                let lon0 = lonc.to_radians() - (g * gamma0.tan()).clamp(-1.0, 1.0).asin() / b;
                let uc = if !centre {
                    0.0
                } else if (alpha.abs() - 90.0).abs() < 1e-12 {
                    a * (lonc.to_radians() - lon0)
                } else {
                    (a / b) * (d * d - 1.0).sqrt().atan2(al.cos()) * sign
                };
                Consts::Omerc {
                    a,
                    b,
                    h,
                    gamma0,
                    lon0,
                    uc,
                }
            }
            Method::EquidistantCylindrical { lat_ts, .. } => {
                let (nu1, _) = ellipsoid.radii(lat_ts);
                Consts::Cass {
                    m: meridian_series(e2),
                    m0: nu1 * lat_ts.to_radians().cos(),
                }
            }
            Method::CassiniSoldner { lat0, .. } => {
                let m = meridian_series(e2);
                Consts::Cass {
                    m,
                    m0: meridian(ellipsoid.a, &m, lat0.to_radians()),
                }
            }
            Method::TransverseMercator { lat0, .. } => {
                let n = ellipsoid.f / (2.0 - ellipsoid.f);
                let n2 = n * n;
                let n3 = n2 * n;
                let n4 = n3 * n;
                let n5 = n4 * n;
                let n6 = n5 * n;
                let a_r = ellipsoid.a / (1.0 + n) * (1.0 + n2 / 4.0 + n4 / 64.0 + n6 / 256.0);
                let alpha = [
                    n / 2.0 - 2.0 * n2 / 3.0 + 5.0 * n3 / 16.0 + 41.0 * n4 / 180.0
                        - 127.0 * n5 / 288.0
                        + 7891.0 * n6 / 37800.0,
                    13.0 * n2 / 48.0 - 3.0 * n3 / 5.0 + 557.0 * n4 / 1440.0 + 281.0 * n5 / 630.0
                        - 1_983_433.0 * n6 / 1_935_360.0,
                    61.0 * n3 / 240.0 - 103.0 * n4 / 140.0
                        + 15061.0 * n5 / 26880.0
                        + 167_603.0 * n6 / 181_440.0,
                    49561.0 * n4 / 161_280.0 - 179.0 * n5 / 168.0 + 6_601_661.0 * n6 / 7_257_600.0,
                    34729.0 * n5 / 80640.0 - 3_418_889.0 * n6 / 1_995_840.0,
                    212_378_941.0 * n6 / 319_334_400.0,
                ];
                let beta = [
                    n / 2.0 - 2.0 * n2 / 3.0 + 37.0 * n3 / 96.0 - n4 / 360.0 - 81.0 * n5 / 512.0
                        + 96199.0 * n6 / 604_800.0,
                    n2 / 48.0 + n3 / 15.0 - 437.0 * n4 / 1440.0 + 46.0 * n5 / 105.0
                        - 1_118_711.0 * n6 / 3_870_720.0,
                    17.0 * n3 / 480.0 - 37.0 * n4 / 840.0 - 209.0 * n5 / 4480.0
                        + 5569.0 * n6 / 90720.0,
                    4397.0 * n4 / 161_280.0 - 11.0 * n5 / 504.0 - 830_251.0 * n6 / 7_257_600.0,
                    4583.0 * n5 / 161_280.0 - 108_847.0 * n6 / 3_991_680.0,
                    20_648_693.0 * n6 / 638_668_800.0,
                ];
                // ξ of the origin on the central meridian.
                let xi0p = taup(lat0.to_radians().tan(), e).atan();
                let mut xi0 = xi0p;
                for (j, a) in alpha.iter().enumerate() {
                    xi0 += a * (2.0 * (j + 1) as f64 * xi0p).sin();
                }
                Consts::Tm {
                    a_r,
                    m0: a_r * xi0,
                    alpha,
                    beta,
                }
            }
            Method::LambertConic {
                lat0,
                lat1,
                lat2,
                k0,
                ..
            } => {
                let (p0, p1, p2) = (lat0.to_radians(), lat1.to_radians(), lat2.to_radians());
                let (m1, m2) = (m_of(p1, e2), m_of(p2, e2));
                let (t0, t1, t2) = (t_of(p0, e), t_of(p1, e), t_of(p2, e));
                let n = if (p1 - p2).abs() < 1e-12 {
                    p1.sin()
                } else {
                    (m1.ln() - m2.ln()) / (t1.ln() - t2.ln())
                };
                let f = m1 / (n * t1.powf(n));
                Consts::Cone {
                    n,
                    c: f,
                    rho0: ellipsoid.a * f * t0.powf(n) * k0,
                }
            }
            Method::Albers {
                lat0, lat1, lat2, ..
            } => {
                let (p0, p1, p2) = (lat0.to_radians(), lat1.to_radians(), lat2.to_radians());
                let (m1, m2) = (m_of(p1, e2), m_of(p2, e2));
                let (q0, q1, q2) = (q_of(p0, e), q_of(p1, e), q_of(p2, e));
                let n = if (p1 - p2).abs() < 1e-12 {
                    p1.sin()
                } else {
                    (m1 * m1 - m2 * m2) / (q2 - q1)
                };
                let c = m1 * m1 + n * q1;
                Consts::Cone {
                    n,
                    c,
                    rho0: ellipsoid.a * (c - n * q0).max(0.0).sqrt() / n,
                }
            }
            Method::Mercator { k0, .. } => Consts::Merc { k: k0 },
            Method::MercatorB { lat1, .. } => Consts::Merc {
                k: m_of(lat1.to_radians(), e2),
            },
            Method::WebMercator { .. } => Consts::None,
            Method::PolarStereographic { lat_ts, south, .. } => {
                let pc = if south { -lat_ts } else { lat_ts }.to_radians();
                let k = if (pc - FRAC_PI_2).abs() < 1e-12 {
                    2.0 * ellipsoid.a / ((1.0 + e).powf(1.0 + e) * (1.0 - e).powf(1.0 - e)).sqrt()
                } else {
                    ellipsoid.a * m_of(pc, e2) / t_of(pc, e)
                };
                Consts::Polar { k }
            }
            Method::RotatedPole { pole_lat, .. } => {
                let (sin, cos) = (90.0 + pole_lat).to_radians().sin_cos();
                Consts::Rot { sin, cos }
            }
            Method::Sinusoidal { .. } => {
                let (e4, e6) = (e2 * e2, e2 * e2 * e2);
                let m = [
                    1.0 - e2 / 4.0 - 3.0 * e4 / 64.0 - 5.0 * e6 / 256.0,
                    3.0 * e2 / 8.0 + 3.0 * e4 / 32.0 + 45.0 * e6 / 1024.0,
                    15.0 * e4 / 256.0 + 45.0 * e6 / 1024.0,
                    35.0 * e6 / 3072.0,
                ];
                let r = (1.0 - e2).sqrt();
                let e1 = (1.0 - r) / (1.0 + r);
                let inv = [
                    3.0 * e1 / 2.0 - 27.0 * e1.powi(3) / 32.0,
                    21.0 * e1 * e1 / 16.0 - 55.0 * e1.powi(4) / 32.0,
                    151.0 * e1.powi(3) / 96.0,
                    1097.0 * e1.powi(4) / 512.0,
                ];
                Consts::Sinu { m, inv }
            }
            Method::LambertAzimuthal { lat0, .. } => {
                let qp = q_of(FRAC_PI_2, e);
                let rq = ellipsoid.a * (qp / 2.0).sqrt();
                let p1 = lat0.to_radians();
                let b1 = (q_of(p1, e) / qp).clamp(-1.0, 1.0).asin();
                let (sin_b1, cos_b1) = b1.sin_cos();
                let d = if cos_b1.abs() < 1e-12 {
                    1.0
                } else {
                    ellipsoid.a * m_of(p1, e2) / (rq * cos_b1)
                };
                Consts::Laea {
                    qp,
                    rq,
                    sin_b1,
                    cos_b1,
                    d,
                }
            }
            Method::Equirectangular { .. } | Method::CylindricalEqualArea { .. } => Consts::None,
        };
        Projection {
            method,
            ellipsoid,
            k,
        }
    }

    /// Geographic (degrees) to projected (metres).
    pub fn forward(&self, lon: f64, lat: f64) -> Option<(f64, f64)> {
        if !lon.is_finite() || !lat.is_finite() || lat.abs() > 90.0 {
            return None;
        }
        let ell = self.ellipsoid;
        let e = ell.e();
        let phi = lat.to_radians();
        let out = match (self.method, &self.k) {
            (
                Method::TransverseMercatorSouth {
                    lat0,
                    lon0,
                    k0,
                    fe,
                    fn_,
                },
                _,
            ) => {
                let (x, y) = self.north_tm(lat0, lon0, k0, fe, fn_).forward(lon, lat)?;
                (2.0 * fe - x, 2.0 * fn_ - y)
            }
            (
                Method::PolarStereographicA {
                    lon0,
                    south,
                    fe,
                    fn_,
                    ..
                },
                Consts::Polar { k },
            ) => {
                let (phi, s) = if south { (-phi, -1.0) } else { (phi, 1.0) };
                if phi <= -FRAC_PI_2 + 1e-15 {
                    return None;
                }
                let rho = k * t_of(phi, e);
                let th = wrap(lon - lon0).to_radians();
                (fe + rho * th.sin(), fn_ - s * rho * th.cos())
            }
            (
                Method::ObliqueStereographic {
                    lon0, k0, fe, fn_, ..
                },
                Consts::Sterea { r, n, c, chi0 },
            ) => {
                let s = phi.sin();
                let chi = if lat.abs() >= 90.0 {
                    phi
                } else {
                    let sa = (1.0 + s) / (1.0 - s);
                    let sb = (1.0 - e * s) / (1.0 + e * s);
                    let w = c * (sa * sb.powf(e)).powf(*n);
                    ((w - 1.0) / (w + 1.0)).asin()
                };
                let dl = n * wrap(lon - lon0).to_radians();
                let (sx, cx) = chi.sin_cos();
                let (s0, c0) = chi0.sin_cos();
                let bb = 1.0 + sx * s0 + cx * c0 * dl.cos();
                if bb <= 1e-15 {
                    return None;
                }
                (
                    fe + 2.0 * r * k0 * cx * dl.sin() / bb,
                    fn_ + 2.0 * r * k0 * (sx * c0 - cx * s0 * dl.cos()) / bb,
                )
            }
            (
                Method::HotineObliqueMercator {
                    latc,
                    gamma,
                    fe,
                    fn_,
                    ..
                },
                Consts::Omerc {
                    a,
                    b,
                    h,
                    gamma0,
                    lon0,
                    uc,
                },
            ) => {
                let dl = wrap((lon.to_radians() - lon0).to_degrees()).to_radians();
                let (u, v) = if lat.abs() >= 90.0 {
                    let s = lat.signum();
                    (
                        a * phi / b,
                        a / b * (FRAC_PI_4 - s * gamma0 / 2.0).tan().ln(),
                    )
                } else {
                    let t = t_hotine(phi, e);
                    let q = h / t.powf(*b);
                    let ss = (q - 1.0 / q) / 2.0;
                    let tt = (q + 1.0 / q) / 2.0;
                    let vv = (b * dl).sin();
                    let uu = (-vv * gamma0.cos() + ss * gamma0.sin()) / tt;
                    if (uu.abs() - 1.0).abs() < 1e-15 {
                        return None;
                    }
                    let v = a * ((1.0 - uu) / (1.0 + uu)).ln() / (2.0 * b);
                    let u = a * (ss * gamma0.cos() + vv * gamma0.sin()).atan2((b * dl).cos()) / b;
                    (u, v)
                };
                let u = u - uc.abs() * if latc < 0.0 { -1.0 } else { 1.0 };
                let gc = gamma.to_radians();
                (
                    fe + v * gc.cos() + u * gc.sin(),
                    fn_ + u * gc.cos() - v * gc.sin(),
                )
            }
            (Method::EquidistantCylindrical { lon0, fe, fn_, .. }, Consts::Cass { m, m0: k }) => (
                fe + k * wrap(lon - lon0).to_radians(),
                fn_ + meridian(ell.a, m, phi),
            ),
            (Method::CassiniSoldner { lon0, fe, fn_, .. }, Consts::Cass { m, m0 }) => {
                let e2 = ell.e2();
                let (s, c) = phi.sin_cos();
                let aa = wrap(lon - lon0).to_radians() * c;
                let t = phi.tan().powi(2);
                let cc = e2 * c * c / (1.0 - e2);
                let nu = ell.a / (1.0 - e2 * s * s).sqrt();
                let x = nu
                    * (aa - t * aa.powi(3) / 6.0 - (8.0 - t + 8.0 * cc) * t * aa.powi(5) / 120.0);
                let y = meridian(ell.a, m, phi) - m0
                    + nu * phi.tan() * (aa * aa / 2.0 + (5.0 - t + 6.0 * cc) * aa.powi(4) / 24.0);
                (fe + x, fn_ + y)
            }
            (
                Method::TransverseMercator {
                    lon0, k0, fe, fn_, ..
                },
                Consts::Tm { a_r, m0, alpha, .. },
            ) => {
                let dl = wrap(lon - lon0).to_radians();
                let tau = phi.tan();
                let tp = if lat.abs() == 90.0 {
                    phi.signum() * f64::INFINITY
                } else {
                    taup(tau, e)
                };
                let (xip, etap) = if tp.is_infinite() {
                    (phi.signum() * FRAC_PI_2, 0.0)
                } else {
                    (tp.atan2(dl.cos()), (dl.sin() / tp.hypot(dl.cos())).asinh())
                };
                let (mut xi, mut eta) = (xip, etap);
                for (j, a) in alpha.iter().enumerate() {
                    let k2 = 2.0 * (j + 1) as f64;
                    xi += a * (k2 * xip).sin() * (k2 * etap).cosh();
                    eta += a * (k2 * xip).cos() * (k2 * etap).sinh();
                }
                (fe + k0 * a_r * eta, fn_ + k0 * (a_r * xi - m0))
            }
            (Method::Mercator { lon0, fe, fn_, .. }, Consts::Merc { k })
            | (Method::MercatorB { lon0, fe, fn_, .. }, Consts::Merc { k }) => {
                if lat.abs() >= 90.0 {
                    return None;
                }
                let x = ell.a * k * wrap(lon - lon0).to_radians();
                let y = ell.a * k * psi_of_tau(phi.tan(), e);
                (fe + x, fn_ + y)
            }
            (Method::WebMercator { lon0, fe, fn_ }, _) => {
                if lat.abs() >= 90.0 {
                    return None;
                }
                let x = ell.a * wrap(lon - lon0).to_radians();
                let y = ell.a * (FRAC_PI_4 + phi / 2.0).tan().ln();
                (fe + x, fn_ + y)
            }
            (
                Method::LambertConic {
                    lon0, k0, fe, fn_, ..
                },
                Consts::Cone { n, c, rho0 },
            ) => {
                let rho = if lat.abs() == 90.0 && lat.signum() == n.signum() {
                    0.0
                } else {
                    ell.a * c * t_of(phi, e).powf(*n) * k0
                };
                let th = n * wrap(lon - lon0).to_radians();
                (fe + rho * th.sin(), fn_ + rho0 - rho * th.cos())
            }
            (Method::Albers { lon0, fe, fn_, .. }, Consts::Cone { n, c, rho0 }) => {
                let rho = ell.a * (c - n * q_of(phi, e)).max(0.0).sqrt() / n;
                let th = n * wrap(lon - lon0).to_radians();
                (fe + rho * th.sin(), fn_ + rho0 - rho * th.cos())
            }
            (
                Method::PolarStereographic {
                    lon0,
                    south,
                    fe,
                    fn_,
                    ..
                },
                Consts::Polar { k },
            ) => {
                let (phi, s) = if south { (-phi, -1.0) } else { (phi, 1.0) };
                let rho = if phi <= -FRAC_PI_2 + 1e-15 {
                    return None;
                } else {
                    k * t_of(phi, e)
                };
                let th = wrap(lon - lon0).to_radians();
                (fe + rho * th.sin(), fn_ - s * rho * th.cos())
            }
            (
                Method::RotatedPole {
                    pole_lon, angle, ..
                },
                Consts::Rot { sin, cos },
            ) => {
                let (sl, cl) = (lon - pole_lon).to_radians().sin_cos();
                let (sp, cp) = phi.sin_cos();
                let (x, y, z) = (cp * cl, cp * sl, sp);
                let xr = cos * x + sin * z;
                let zr = -sin * x + cos * z;
                let lat_r = zr.clamp(-1.0, 1.0).asin();
                let lon_r = wrap(y.atan2(xr).to_degrees() - angle).to_radians();
                (ell.a * lon_r, ell.a * lat_r)
            }
            (Method::Sinusoidal { lon0, fe, fn_ }, Consts::Sinu { m, .. }) => {
                let lam = wrap(lon - lon0).to_radians();
                let x = ell.a * lam * m_of(phi, ell.e2());
                (fe + x, fn_ + meridian(ell.a, m, phi))
            }
            (
                Method::LambertAzimuthal {
                    lat0,
                    lon0,
                    fe,
                    fn_,
                },
                Consts::Laea {
                    qp,
                    rq,
                    sin_b1,
                    cos_b1,
                    d,
                },
            ) => {
                let lam = wrap(lon - lon0).to_radians();
                let q = q_of(phi, e);
                if lat0.abs() == 90.0 {
                    let s = lat0.signum();
                    let rho = ell.a * (qp - s * q).max(0.0).sqrt();
                    (fe + rho * lam.sin(), fn_ - s * rho * lam.cos())
                } else {
                    let b = (q / qp).clamp(-1.0, 1.0).asin();
                    let (sb, cb) = b.sin_cos();
                    let den = 1.0 + sin_b1 * sb + cos_b1 * cb * lam.cos();
                    if den <= 1e-15 {
                        return None;
                    }
                    let bb = rq * (2.0 / den).sqrt();
                    (
                        fe + bb * d * cb * lam.sin(),
                        fn_ + bb / d * (cos_b1 * sb - sin_b1 * cb * lam.cos()),
                    )
                }
            }
            (
                Method::Equirectangular {
                    lat_ts,
                    lon0,
                    fe,
                    fn_,
                },
                _,
            ) => {
                let lam = wrap(lon - lon0).to_radians();
                (
                    fe + ell.a * lam * lat_ts.to_radians().cos(),
                    fn_ + ell.a * phi,
                )
            }
            (
                Method::CylindricalEqualArea {
                    lat_ts,
                    lon0,
                    fe,
                    fn_,
                },
                _,
            ) => {
                // Snyder 10-15: x = a k₀ λ, y = a q / (2 k₀), k₀ = m(φ_ts).
                let k0 = m_of(lat_ts.to_radians(), ell.e2());
                if k0 <= 1e-12 {
                    return None;
                }
                let lam = wrap(lon - lon0).to_radians();
                (
                    fe + ell.a * k0 * lam,
                    fn_ + ell.a * q_of(phi, e) / (2.0 * k0),
                )
            }
            _ => return None,
        };
        (out.0.is_finite() && out.1.is_finite()).then_some(out)
    }

    /// Projected (metres) to geographic (degrees: longitude, latitude).
    pub fn inverse(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let ell = self.ellipsoid;
        let e = ell.e();
        let (lon, lat) = match (self.method, &self.k) {
            (
                Method::TransverseMercatorSouth {
                    lat0,
                    lon0,
                    k0,
                    fe,
                    fn_,
                },
                _,
            ) => {
                return self
                    .north_tm(lat0, lon0, k0, fe, fn_)
                    .inverse(2.0 * fe - x, 2.0 * fn_ - y);
            }
            (
                Method::PolarStereographicA {
                    lon0,
                    south,
                    fe,
                    fn_,
                    ..
                },
                Consts::Polar { k },
            ) => {
                let s = if south { -1.0 } else { 1.0 };
                let (dx, dy) = (x - fe, -s * (y - fn_));
                let rho = dx.hypot(dy);
                let lat = if rho == 0.0 {
                    90.0
                } else {
                    lat_of_psi(-(rho / k).ln(), e).to_degrees()
                };
                (lon0 + dx.atan2(dy).to_degrees(), s * lat)
            }
            (
                Method::ObliqueStereographic {
                    lon0, k0, fe, fn_, ..
                },
                Consts::Sterea { r, n, c, chi0 },
            ) => {
                let (de, dn) = (x - fe, y - fn_);
                let g = 2.0 * r * k0 * (FRAC_PI_4 - chi0 / 2.0).tan();
                let h = 4.0 * r * k0 * chi0.tan() + g;
                let i = de.atan2(h + dn);
                let j = de.atan2(g - dn) - i;
                let chi = chi0 + 2.0 * ((dn - de * (j / 2.0).tan()) / (2.0 * r * k0)).atan();
                let lam = (j + 2.0 * i) / n;
                let sc = chi.sin();
                let psi = 0.5 * ((1.0 + sc) / (c * (1.0 - sc))).ln() / n;
                (lon0 + lam.to_degrees(), lat_of_psi(psi, e).to_degrees())
            }
            (
                Method::HotineObliqueMercator {
                    latc,
                    gamma,
                    fe,
                    fn_,
                    ..
                },
                Consts::Omerc {
                    a,
                    b,
                    h,
                    gamma0,
                    lon0,
                    uc,
                },
            ) => {
                let gc = gamma.to_radians();
                let (de, dn) = (x - fe, y - fn_);
                let v = de * gc.cos() - dn * gc.sin();
                let u =
                    dn * gc.cos() + de * gc.sin() + uc.abs() * if latc < 0.0 { -1.0 } else { 1.0 };
                let q = (-b * v / a).exp();
                let ss = (q - 1.0 / q) / 2.0;
                let tt = (q + 1.0 / q) / 2.0;
                let vv = (b * u / a).sin();
                let uu = (vv * gamma0.cos() + ss * gamma0.sin()) / tt;
                if (uu.abs() - 1.0).abs() < 1e-15 {
                    return Some((wrap(lon0.to_degrees()), 90f64.copysign(uu)));
                }
                let t = (h / ((1.0 + uu) / (1.0 - uu)).sqrt()).powf(1.0 / b);
                let chi = FRAC_PI_2 - 2.0 * t.atan();
                let phi = lat_of_chi(chi, e);
                let lam =
                    lon0 - (ss * gamma0.cos() - vv * gamma0.sin()).atan2((b * u / a).cos()) / b;
                (lam.to_degrees(), phi.to_degrees())
            }
            (Method::EquidistantCylindrical { lon0, fe, fn_, .. }, Consts::Cass { m, m0: k }) => {
                let e2 = ell.e2();
                let target = y - fn_;
                let mut phi = target / (ell.a * m[0]);
                for _ in 0..8 {
                    let s = phi.sin();
                    let w = 1.0 - e2 * s * s;
                    let d =
                        (meridian(ell.a, m, phi) - target) / (ell.a * (1.0 - e2) / (w * w.sqrt()));
                    phi -= d;
                    if d.abs() < 1e-15 {
                        break;
                    }
                }
                if !phi.is_finite() || phi.abs() > FRAC_PI_2 + 1e-12 || *k <= 0.0 {
                    return None;
                }
                (
                    lon0 + ((x - fe) / k).to_degrees(),
                    phi.clamp(-FRAC_PI_2, FRAC_PI_2).to_degrees(),
                )
            }
            (Method::CassiniSoldner { lon0, fe, fn_, .. }, Consts::Cass { m, m0 }) => {
                let e2 = ell.e2();
                let m1 = m0 + (y - fn_);
                let mut p1 = m1 / (ell.a * m[0]);
                // Newton on the meridian distance (from the rectifying latitude).
                for _ in 0..8 {
                    let s = p1.sin();
                    let w = 1.0 - e2 * s * s;
                    let d = (meridian(ell.a, m, p1) - m1) / (ell.a * (1.0 - e2) / (w * w.sqrt()));
                    p1 -= d;
                    if d.abs() < 1e-15 {
                        break;
                    }
                }
                let (nu1, rho1) = ell.radii(p1.to_degrees());
                let t1 = p1.tan().powi(2);
                let d = (x - fe) / nu1;
                let phi = p1
                    - (nu1 * p1.tan() / rho1) * (d * d / 2.0 - (1.0 + 3.0 * t1) * d.powi(4) / 24.0);
                let lam = (d - t1 * d.powi(3) / 3.0 + (1.0 + 3.0 * t1) * t1 * d.powi(5) / 15.0)
                    / p1.cos();
                // The series inverse is good to centimetres 400 km out; a few Newton steps
                // on the forward series make it its exact inverse.
                let (mut lo, mut la) = (lon0 + lam.to_degrees(), phi.to_degrees());
                for _ in 0..4 {
                    let Some((fx, fy)) = self.forward(lo, la) else {
                        break;
                    };
                    let (ex, ey) = (x - fx, y - fy);
                    if ex.abs() < 1e-7 && ey.abs() < 1e-7 {
                        break;
                    }
                    let h = 1e-6;
                    let (Some((ax, ay)), Some((bx, by))) =
                        (self.forward(lo + h, la), self.forward(lo, la + h))
                    else {
                        break;
                    };
                    let (j11, j21, j12, j22) =
                        ((ax - fx) / h, (ay - fy) / h, (bx - fx) / h, (by - fy) / h);
                    let det = j11 * j22 - j12 * j21;
                    if det.abs() < 1e-12 {
                        break;
                    }
                    lo += (j22 * ex - j12 * ey) / det;
                    la += (j11 * ey - j21 * ex) / det;
                }
                (lo, la)
            }
            (
                Method::TransverseMercator {
                    lon0, k0, fe, fn_, ..
                },
                Consts::Tm { a_r, m0, beta, .. },
            ) => {
                let xi = ((y - fn_) / k0 + m0) / a_r;
                let eta = (x - fe) / (k0 * a_r);
                let (mut xip, mut etap) = (xi, eta);
                for (j, b) in beta.iter().enumerate() {
                    let k2 = 2.0 * (j + 1) as f64;
                    xip -= b * (k2 * xi).sin() * (k2 * eta).cosh();
                    etap -= b * (k2 * xi).cos() * (k2 * eta).sinh();
                }
                let tp = xip.sin() / etap.sinh().hypot(xip.cos());
                let lam = etap.sinh().atan2(xip.cos());
                let lat = tau_of_taup(tp, e).atan();
                (lon0 + lam.to_degrees(), lat.to_degrees())
            }
            (Method::Mercator { lon0, fe, fn_, .. }, Consts::Merc { k })
            | (Method::MercatorB { lon0, fe, fn_, .. }, Consts::Merc { k }) => {
                let lam = (x - fe) / (ell.a * k);
                let psi = (y - fn_) / (ell.a * k);
                (lon0 + lam.to_degrees(), lat_of_psi(psi, e).to_degrees())
            }
            (Method::WebMercator { lon0, fe, fn_ }, _) => {
                let lam = (x - fe) / ell.a;
                let lat = ((y - fn_) / ell.a).sinh().atan();
                (lon0 + lam.to_degrees(), lat.to_degrees())
            }
            (
                Method::LambertConic {
                    lon0, k0, fe, fn_, ..
                },
                Consts::Cone { n, c, rho0 },
            ) => {
                let (dx, dy) = (x - fe, rho0 - (y - fn_));
                let s = n.signum();
                let rho = s * dx.hypot(dy);
                let th = (s * dx).atan2(s * dy);
                let lon = lon0 + (th / n).to_degrees();
                let lat = if rho == 0.0 {
                    s * 90.0
                } else {
                    let t = (rho / (ell.a * c * k0)).powf(1.0 / n);
                    lat_of_psi(-t.ln(), e).to_degrees()
                };
                (lon, lat)
            }
            (Method::Albers { lon0, fe, fn_, .. }, Consts::Cone { n, c, rho0 }) => {
                let (dx, dy) = (x - fe, rho0 - (y - fn_));
                let s = n.signum();
                let rho = dx.hypot(dy);
                let th = (s * dx).atan2(s * dy);
                let q = (c - (rho * n / ell.a).powi(2)) / n;
                let lon = lon0 + (th / n).to_degrees();
                (lon, lat_of_q(q, e)?.to_degrees())
            }
            (
                Method::PolarStereographic {
                    lon0,
                    south,
                    fe,
                    fn_,
                    ..
                },
                Consts::Polar { k },
            ) => {
                let s = if south { -1.0 } else { 1.0 };
                let (dx, dy) = (x - fe, -s * (y - fn_));
                let rho = dx.hypot(dy);
                let lat = if rho == 0.0 {
                    90.0
                } else {
                    lat_of_psi(-(rho / k).ln(), e).to_degrees()
                };
                (lon0 + dx.atan2(dy).to_degrees(), s * lat)
            }
            (
                Method::RotatedPole {
                    pole_lon, angle, ..
                },
                Consts::Rot { sin, cos },
            ) => {
                let (sl, cl) = ((x / ell.a).to_degrees() + angle).to_radians().sin_cos();
                let (sp, cp) = (y / ell.a).sin_cos();
                let (xr, yr, zr) = (cp * cl, cp * sl, sp);
                let gx = cos * xr - sin * zr;
                let gz = sin * xr + cos * zr;
                let lat = gz.clamp(-1.0, 1.0).asin().to_degrees();
                (yr.atan2(gx).to_degrees() + pole_lon, lat)
            }
            (Method::Sinusoidal { lon0, fe, fn_ }, Consts::Sinu { m, inv }) => {
                let mu = (y - fn_) / (ell.a * m[0]);
                let mut phi = mu;
                for (j, c) in inv.iter().enumerate() {
                    phi += c * (2.0 * (j + 1) as f64 * mu).sin();
                }
                // Newton on the meridian distance takes the series to machine precision.
                let e2 = ell.e2();
                for _ in 0..3 {
                    let s = phi.sin();
                    let w = 1.0 - e2 * s * s;
                    let dm = ell.a * (1.0 - e2) / (w * w.sqrt());
                    phi -= (meridian(ell.a, m, phi) - (y - fn_)) / dm;
                }
                if phi.abs() > FRAC_PI_2 + 1e-12 {
                    return None;
                }
                let phi = phi.clamp(-FRAC_PI_2, FRAC_PI_2);
                let mm = m_of(phi, ell.e2());
                let lam = if mm.abs() < 1e-15 {
                    0.0
                } else {
                    (x - fe) / (ell.a * mm)
                };
                if lam.abs() > std::f64::consts::PI + 1e-9 {
                    return None;
                }
                (lon0 + lam.to_degrees(), phi.to_degrees())
            }
            (
                Method::LambertAzimuthal {
                    lat0,
                    lon0,
                    fe,
                    fn_,
                },
                Consts::Laea {
                    qp,
                    rq,
                    sin_b1,
                    cos_b1,
                    d,
                },
            ) => {
                let (dx, dy) = (x - fe, y - fn_);
                if lat0.abs() == 90.0 {
                    let s = lat0.signum();
                    let rho = dx.hypot(dy);
                    let q = s * (qp - (rho / ell.a).powi(2));
                    (
                        lon0 + dx.atan2(-s * dy).to_degrees(),
                        lat_of_q(q, e)?.to_degrees(),
                    )
                } else {
                    let rho = (dx / d).hypot(d * dy);
                    if rho == 0.0 {
                        return Some((lon0, lat0));
                    }
                    let arg = rho / (2.0 * rq);
                    if arg > 1.0 + 1e-12 {
                        return None;
                    }
                    let ce = 2.0 * arg.min(1.0).asin();
                    let (sc, cc) = ce.sin_cos();
                    let q = qp * (cc * sin_b1 + d * dy * sc * cos_b1 / rho);
                    let lam = (dx * sc).atan2(d * rho * cos_b1 * cc - d * d * dy * sin_b1 * sc);
                    (lon0 + lam.to_degrees(), lat_of_q(q, e)?.to_degrees())
                }
            }
            (
                Method::Equirectangular {
                    lat_ts,
                    lon0,
                    fe,
                    fn_,
                },
                _,
            ) => {
                let c = lat_ts.to_radians().cos();
                if c.abs() < 1e-12 {
                    return None;
                }
                let lat = ((y - fn_) / ell.a).to_degrees();
                if lat.abs() > 90.0 + 1e-9 {
                    return None;
                }
                (
                    lon0 + ((x - fe) / (ell.a * c)).to_degrees(),
                    lat.clamp(-90.0, 90.0),
                )
            }
            (
                Method::CylindricalEqualArea {
                    lat_ts,
                    lon0,
                    fe,
                    fn_,
                },
                _,
            ) => {
                let k0 = m_of(lat_ts.to_radians(), ell.e2());
                if k0 <= 1e-12 {
                    return None;
                }
                let lam = (x - fe) / (ell.a * k0);
                if lam.abs() > std::f64::consts::PI + 1e-9 {
                    return None;
                }
                let q = 2.0 * k0 * (y - fn_) / ell.a;
                let qp = q_of(FRAC_PI_2, e);
                if q.abs() > qp * (1.0 + 1e-12) {
                    return None;
                }
                (
                    lon0 + lam.to_degrees(),
                    lat_of_q(q.clamp(-qp, qp), e)?.to_degrees(),
                )
            }
            _ => return None,
        };
        (lon.is_finite() && lat.is_finite()).then_some((wrap(lon), lat))
    }
}

impl Projection {
    /// The northern Transverse Mercator a south-orientated one mirrors.
    fn north_tm(&self, lat0: f64, lon0: f64, k0: f64, fe: f64, fn_: f64) -> Projection {
        Projection {
            method: Method::TransverseMercator {
                lat0,
                lon0,
                k0,
                fe,
                fn_,
            },
            ellipsoid: self.ellipsoid,
            k: self.k.clone(),
        }
    }
}

/// Latitude (radians) from Albers's q, by Newton's method (Snyder eq. 3-16).
fn lat_of_q(q: f64, e: f64) -> Option<f64> {
    if e == 0.0 {
        return Some((q / 2.0).clamp(-1.0, 1.0).asin());
    }
    let e2 = e * e;
    let qp = q_of(FRAC_PI_2, e);
    if q.abs() >= qp - 1e-15 {
        return Some(FRAC_PI_2.copysign(q));
    }
    let mut phi = (q / 2.0).clamp(-1.0, 1.0).asin();
    for _ in 0..20 {
        let s = phi.sin();
        let w = 1.0 - e2 * s * s;
        let d = w * w / (2.0 * phi.cos()) * (q / (1.0 - e2) - s / w - (e * s).atanh() / e);
        phi += d;
        if d.abs() < 1e-15 {
            break;
        }
    }
    phi.is_finite().then_some(phi)
}

/// The meridian distance from the equator to `phi` (radians), Snyder 3-21.
fn meridian(a: f64, m: &[f64; 4], phi: f64) -> f64 {
    a * (m[0] * phi - m[1] * (2.0 * phi).sin() + m[2] * (4.0 * phi).sin()
        - m[3] * (6.0 * phi).sin())
}

/// A longitude difference into −180..180.
fn wrap(d: f64) -> f64 {
    if (-180.0..=180.0).contains(&d) {
        d
    } else {
        (d + 180.0).rem_euclid(360.0) - 180.0
    }
}

#[cfg(test)]
mod cea_tests {
    use super::*;

    /// EASE-Grid 2.0 (EPSG 6933: WGS 84, ±30°): the grid's published extent, ±17,367,530.45 m
    /// at ±180° and ±7,314,540.83 m at ±85.0445664°; and EASE-Grid's sphere.
    #[test]
    fn cylindrical_equal_area() {
        let p = Projection::new(
            Method::CylindricalEqualArea {
                lat_ts: 30.0,
                lon0: 0.0,
                fe: 0.0,
                fn_: 0.0,
            },
            Ellipsoid::WGS84,
        );
        let (x, _) = p.forward(180.0, 0.0).unwrap();
        assert!((x - 17_367_530.445_161_38).abs() < 1e-3, "{x}");
        let (_, y) = p.forward(0.0, 85.044_566_4).unwrap();
        assert!((y - 7_314_540.830_907_88).abs() < 0.05, "{y}");
        for (lon, lat) in [(10.0, 45.0), (-120.5, -60.25), (179.0, 89.0), (0.0, 0.0)] {
            let (x, y) = p.forward(lon, lat).unwrap();
            let (l, b) = p.inverse(x, y).unwrap();
            assert!(
                (l - lon).abs() < 1e-9 && (b - lat).abs() < 1e-9,
                "{lon} {lat}"
            );
        }
        assert!(p.inverse(0.0, 8e6).is_none());
        // EASE-Grid: R = 6,371,228 m; x = R λ cos 30°, y = R sin φ / cos 30°.
        let r = 6_371_228.0;
        let s = Projection::new(
            Method::CylindricalEqualArea {
                lat_ts: 30.0,
                lon0: 0.0,
                fe: 0.0,
                fn_: 0.0,
            },
            Ellipsoid { a: r, f: 0.0 },
        );
        let c = 30f64.to_radians().cos();
        let (x, y) = s.forward(90.0, 45.0).unwrap();
        assert!((x - r * FRAC_PI_2 * c).abs() < 1e-6);
        assert!((y - r * 45f64.to_radians().sin() / c).abs() < 1e-6);
        let (l, b) = s.inverse(x, y).unwrap();
        assert!((l - 90.0).abs() < 1e-9 && (b - 45.0).abs() < 1e-9);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt(p: &Projection, lon: f64, lat: f64) {
        let (x, y) = p.forward(lon, lat).unwrap();
        let (lo, la) = p.inverse(x, y).unwrap();
        assert!(
            (lo - lon).abs() < 1e-10 && (la - lat).abs() < 1e-10,
            "{:?}: {lon},{lat} -> {x},{y} -> {lo},{la}",
            p.method
        );
    }

    #[test]
    fn utm_known_point() {
        // EPSG guidance note 7-2 style check: a point in UTM 31N.
        let p = Projection::new(Method::utm(31, true), Ellipsoid::WGS84);
        let (x, y) = p.forward(3.0, 0.0).unwrap();
        assert!((x - 500_000.0).abs() < 1e-6 && y.abs() < 1e-6);
        // 1 degree east of the central meridian on the equator.
        let (x, _) = p.forward(4.0, 0.0).unwrap();
        assert!((x - 611_280.650_891_4).abs() < 1e-6, "{x}");
        // pyproj 3.8 (PROJ) for EPSG:32631.
        let (x, y) = p.forward(0.1, 51.5).unwrap();
        assert!(
            (x - 298_722.334_872_97).abs() < 1e-6 && (y - 5_709_417.124_847_53).abs() < 1e-6,
            "{x} {y}"
        );
        for &(lo, la) in &[(0.1, 51.5), (5.9, -33.0), (3.0, 84.0), (-1.0, 10.0)] {
            rt(&p, lo, la);
        }
    }

    #[test]
    fn conics_and_mercators_round_trip() {
        let lcc = Projection::new(
            Method::LambertConic {
                lat0: 46.5,
                lon0: 3.0,
                lat1: 49.0,
                lat2: 44.0,
                k0: 1.0,
                fe: 700_000.0,
                fn_: 6_600_000.0,
            },
            Ellipsoid::GRS80,
        );
        // The Lambert-93 origin.
        let (x, y) = lcc.forward(3.0, 46.5).unwrap();
        assert!((x - 700_000.0).abs() < 1e-6 && (y - 6_600_000.0).abs() < 1e-6);
        let alb = Projection::new(
            Method::Albers {
                lat0: 23.0,
                lon0: -96.0,
                lat1: 29.5,
                lat2: 45.5,
                fe: 0.0,
                fn_: 0.0,
            },
            Ellipsoid::GRS80,
        );
        let web = Projection::new(
            Method::WebMercator {
                lon0: 0.0,
                fe: 0.0,
                fn_: 0.0,
            },
            Ellipsoid::WGS84,
        );
        let merc = Projection::new(
            Method::Mercator {
                lon0: 0.0,
                k0: 1.0,
                fe: 0.0,
                fn_: 0.0,
            },
            Ellipsoid::WGS84,
        );
        for p in [&lcc, &alb, &web, &merc] {
            for &(lo, la) in &[(2.0, 48.0), (-100.0, 40.0), (10.0, -20.0), (-75.5, 23.1)] {
                rt(p, lo, la);
            }
        }
        let polar = |lat_ts, south| {
            Projection::new(
                Method::PolarStereographic {
                    lat_ts,
                    lon0: -45.0,
                    south,
                    fe: 0.0,
                    fn_: 0.0,
                },
                Ellipsoid::WGS84,
            )
        };
        for p in [&polar(70.0, false), &polar(90.0, false)] {
            for &(lo, la) in &[(2.0, 48.0), (-100.0, 40.0), (170.0, 89.0), (-75.5, 23.1)] {
                rt(p, lo, la);
            }
        }
        let sp = polar(-71.0, true);
        for &(lo, la) in &[(2.0, -48.0), (-100.0, -70.0), (170.0, -89.0)] {
            rt(&sp, lo, la);
        }
        // EPSG:3413 (lat_ts 70, lon0 -45) by Snyder eqs. 21-33, 21-34 and 15-9.
        let (x, y) = polar(70.0, false).forward(0.0, 75.0).unwrap();
        assert!(
            (x - 1_155_327.272_303_26).abs() < 1e-3 && (y + 1_155_327.272_303_26).abs() < 1e-3,
            "{x} {y}"
        );
        let rot = Projection::new(
            Method::RotatedPole {
                pole_lat: -40.0,
                pole_lon: 10.0,
                angle: 0.0,
            },
            Ellipsoid::SPHERE,
        );
        // The rotated equator crosses the prime rotated meridian at 50°N 10°E.
        let (x, y) = rot.forward(10.0, 50.0).unwrap();
        assert!(x.abs() < 1e-9 && y.abs() < 1e-9, "{x} {y}");
        for &(lo, la) in &[(2.0, 48.0), (-100.0, 40.0), (10.0, -20.0), (-75.5, 23.1)] {
            rt(&rot, lo, la);
        }
        // MODIS sinusoidal on its 6371007.181 m sphere: tile h08v05's upper-left corner lies
        // on 40°N, and ten degrees of latitude make one tile (1111950.5197 m).
        let r = 6_371_007.181;
        let sin = Projection::new(
            Method::Sinusoidal {
                lon0: 0.0,
                fe: 0.0,
                fn_: 0.0,
            },
            Ellipsoid { a: r, f: 0.0 },
        );
        let (lon, lat) = sin.inverse(-11_119_505.196_667, 4_447_802.078_667).unwrap();
        let want = (-11_119_505.196_667 / (r * 40f64.to_radians().cos())).to_degrees();
        assert!(
            (lat - 40.0).abs() < 1e-6 && (lon - want).abs() < 1e-6,
            "{lon} {lat} {want}"
        );
        let (_, y) = sin.forward(0.0, 10.0).unwrap();
        assert!((y - 1_111_950.519_666_7).abs() < 1e-3, "{y}");
        let sin_e = Projection::new(
            Method::Sinusoidal {
                lon0: 15.0,
                fe: 100.0,
                fn_: -50.0,
            },
            Ellipsoid::WGS84,
        );
        // EASE-Grid (original, north): LAEA on the 6371228 m sphere.
        let ease = Projection::new(
            Method::LambertAzimuthal {
                lat0: 90.0,
                lon0: 0.0,
                fe: 0.0,
                fn_: 0.0,
            },
            Ellipsoid {
                a: 6_371_228.0,
                f: 0.0,
            },
        );
        // ETRS89-LAEA (EPSG:3035): the EPSG guidance note 7-2 example.
        let etrs = Projection::new(
            Method::LambertAzimuthal {
                lat0: 52.0,
                lon0: 10.0,
                fe: 4_321_000.0,
                fn_: 3_210_000.0,
            },
            Ellipsoid::GRS80,
        );
        let (x, y) = etrs.forward(5.0, 50.0).unwrap();
        assert!(
            (x - 3_962_799.45).abs() < 0.01 && (y - 2_999_718.85).abs() < 0.01,
            "{x} {y}"
        );
        let eqc = Projection::new(
            Method::Equirectangular {
                lat_ts: 30.0,
                lon0: 0.0,
                fe: 0.0,
                fn_: 0.0,
            },
            Ellipsoid::WGS84,
        );
        for p in [&sin, &sin_e, &ease, &etrs, &eqc] {
            for &(lo, la) in &[(2.0, 48.0), (-100.0, 40.0), (10.0, -20.0), (-75.5, 23.1)] {
                rt(p, lo, la);
            }
        }
        // Web Mercator's edge.
        let (_, y) = web.forward(0.0, 85.051_128_779_806_59).unwrap();
        assert!((y - 20_037_508.342_789_244).abs() < 1e-3);
    }
}
