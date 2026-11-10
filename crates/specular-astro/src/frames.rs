//! Celestial frames regions and surveys are given in: ICRS, FK5 at J2000, FK4 at B1950 and
//! galactic (no precession: FK5 positions are taken at J2000, FK4 ones at B1950).
//!
//! The rotations are the IAU ones astropy uses: the ICRS → FK5 J2000 frame bias of USNO
//! Circular 179 (η₀ = −19.9 mas, ξ₀ = 9.1 mas, dα₀ = −22.9 mas), and FK5 J2000 → galactic
//! from the north galactic pole at α = 192.8594812065348°, δ = 27.12825118085622° and the
//! longitude of the north celestial pole 122.9319185680026°.
//!
//! FK4 B1950 → FK5 J2000 (Standish 1982, Aoki et al. 1983, as astropy does it for a
//! position at epoch B1950 without proper motion): the E-terms of aberration are taken off,
//! the B1950 → J2000 matrix applied; the way back applies its transpose and puts the E-terms
//! back by iteration. The E-terms vector comes from the constant of aberration (20.496″),
//! the eccentricity and perigee of the Earth's orbit at B1950 (Explanatory Supplement, 1992)
//! and the IAU 1980 obliquity.

/// A celestial frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sky {
    Icrs,
    Fk5,
    /// FK4 at equinox and epoch B1950, E-terms of aberration included.
    Fk4,
    Galactic,
}

type M = [[f64; 3]; 3];

/// A rotation of the axes by `deg` about axis `k` (0 x, 1 y, 2 z): the passive form.
fn rot(deg: f64, k: usize) -> M {
    let (s, c) = deg.to_radians().sin_cos();
    match k {
        0 => [[1.0, 0.0, 0.0], [0.0, c, s], [0.0, -s, c]],
        1 => [[c, 0.0, -s], [0.0, 1.0, 0.0], [s, 0.0, c]],
        _ => [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]],
    }
}

fn mul(a: &M, b: &M) -> M {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

fn transpose(a: &M) -> M {
    std::array::from_fn(|i| std::array::from_fn(|j| a[j][i]))
}

/// ICRS → FK5 J2000.
fn icrs_to_fk5() -> M {
    let mas = 1.0 / 3.6e6;
    mul(
        &mul(&rot(19.9 * mas, 0), &rot(9.1 * mas, 1)),
        &rot(-22.9 * mas, 2),
    )
}

/// FK5 J2000 → galactic.
fn fk5_to_galactic() -> M {
    mul(
        &mul(
            &rot(180.0 - 122.931_918_568_002_6, 2),
            &rot(90.0 - 27.128_251_180_856_22, 1),
        ),
        &rot(192.859_481_206_534_8, 2),
    )
}

/// FK4 B1950 without E-terms → FK5 J2000 (astropy's `_B1950_TO_J2000_M`, epoch B1950).
const B1950_TO_J2000: M = [
    [
        0.999_925_679_495_687_7,
        -0.011_181_483_220_466_2,
        -0.004_859_003_815_359_2,
    ],
    [
        0.011_181_483_239_171_7,
        0.999_937_484_893_313_5,
        -0.000_027_162_594_714_2,
    ],
    [
        0.004_859_003_772_314_3,
        -0.000_027_170_293_744_0,
        0.999_988_194_602_374_2,
    ],
];

/// The E-terms of aberration at B1950 (a unit-sphere offset of about 0.34″).
fn e_terms() -> [f64; 3] {
    // Julian centuries from J2000 to B1950 (JD 2433282.4235), for the obliquity.
    let t = (2_433_282.423_5 - 2_451_545.0) / 36_525.0;
    let obliquity =
        ((84_381.448_f64 + (-46.815 + (-0.000_59 + 0.001_813 * t) * t) * t) / 3600.0).to_radians();
    let ek = 0.005_693_2_f64.to_radians() * 0.016_730_11;
    let g = (1_015_489.951 / 3600.0_f64).to_radians();
    [
        ek * g.sin(),
        -ek * g.cos() * obliquity.cos(),
        -ek * g.cos() * obliquity.sin(),
    ]
}

fn dot(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(v: [f64; 3]) -> [f64; 3] {
    let n = dot(&v, &v).sqrt();
    v.map(|x| x / n)
}

/// An FK4 direction without its E-terms.
fn remove_e_terms(r: [f64; 3], a: &[f64; 3]) -> [f64; 3] {
    let k = dot(a, &r);
    unit(std::array::from_fn(|i| r[i] - a[i] + k * r[i]))
}

/// The FK4 direction of one without E-terms (the inverse of [`remove_e_terms`], iterated).
fn add_e_terms(r0: [f64; 3], a: &[f64; 3]) -> [f64; 3] {
    let start: [f64; 3] = std::array::from_fn(|i| a[i] + r0[i]);
    let mut r = r0;
    for _ in 0..10 {
        let d = 1.0 + dot(a, &r);
        r = start.map(|x| x / d);
    }
    unit(r)
}

/// The rotation from `a` to `b` (FK4 taken without its E-terms).
fn matrix(a: Sky, b: Sky) -> M {
    let to_fk5 = |s: Sky| match s {
        Sky::Fk5 => rot(0.0, 2),
        Sky::Icrs => icrs_to_fk5(),
        Sky::Fk4 => B1950_TO_J2000,
        Sky::Galactic => transpose(&fk5_to_galactic()),
    };
    // Back to FK4 through the transpose, as astropy does (the matrix is orthogonal to
    // about 1e-11).
    mul(&transpose(&to_fk5(b)), &to_fk5(a))
}

/// `(lng, lat)` (degrees) of frame `a` in frame `b`; longitude in [0, 360).
pub fn convert(a: Sky, b: Sky, lng: f64, lat: f64) -> (f64, f64) {
    if a == b {
        return (lng, lat);
    }
    Rotation::new(a, b).apply(lng, lat)
}

/// A conversion from one frame to another, worked out once for many positions: a rotation,
/// with the FK4 E-terms taken off before it or put back after it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rotation {
    m: Option<M>,
    /// The E-terms to take off first (from FK4) and to put back last (to FK4).
    e_off: Option<[f64; 3]>,
    e_on: Option<[f64; 3]>,
}

impl Rotation {
    pub fn new(a: Sky, b: Sky) -> Rotation {
        let differ = a != b;
        Rotation {
            m: differ.then(|| matrix(a, b)),
            e_off: (differ && a == Sky::Fk4).then(e_terms),
            e_on: (differ && b == Sky::Fk4).then(e_terms),
        }
    }

    /// `(lng, lat)` in degrees; longitude in [0, 360) once converted.
    pub fn apply(&self, lng: f64, lat: f64) -> (f64, f64) {
        let Some(m) = &self.m else {
            return (lng, lat);
        };
        let (l, t) = (lng.to_radians(), lat.to_radians());
        let mut v = [t.cos() * l.cos(), t.cos() * l.sin(), t.sin()];
        if let Some(a) = &self.e_off {
            v = remove_e_terms(v, a);
        }
        let mut r: [f64; 3] = std::array::from_fn(|i| (0..3).map(|j| m[i][j] * v[j]).sum());
        if let Some(a) = &self.e_on {
            r = add_e_terms(r, a);
        }
        (
            r[1].atan2(r[0]).to_degrees().rem_euclid(360.0),
            r[2].clamp(-1.0, 1.0).asin().to_degrees(),
        )
    }
}

/// The frame of a FITS WCS's celestial axes, when it is one of these (`RADESYS` FK5 or an
/// equinox-2000 default is FK5; FK4, or an equinox before 1984 without `RADESYS`, at
/// equinox 1950 or none stated is FK4; ICRS, and anything unstated without an equinox,
/// ICRS).
pub fn of_wcs(w: &specular_fits::wcs::Wcs) -> Option<Sky> {
    use specular_fits::wcs::Frame;
    match &w.celestial.as_ref()?.frame {
        Frame::Galactic => Some(Sky::Galactic),
        Frame::Equatorial { system, equinox } => {
            let s = system.trim().to_ascii_uppercase();
            Some(if s == "FK5" || (s.is_empty() && equinox.is_some()) {
                Sky::Fk5
            } else if s == "FK4" && equinox.is_none_or(|e| (e - 1950.0).abs() < 1e-6) {
                Sky::Fk4
            } else {
                // ICRS; FK4 at other equinoxes, FK4-NO-E and GAPPT are not told apart here.
                Sky::Icrs
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// astropy 8.0.2: `SkyCoord(lng, lat, frame=FK4(equinox="B1950", obstime="B1950"))` to and
    /// from FK5 J2000, ICRS and galactic (`transform_to`, 12 decimals).
    type Case = (Sky, Sky, (f64, f64), (f64, f64));

    const FK4_CASES: [Case; 6] = [
        (
            Sky::Fk4,
            Sky::Fk5,
            (202.0, 47.4),
            (202.526504956587, 47.142422384769),
        ),
        (
            Sky::Fk4,
            Sky::Icrs,
            (10.0, 85.0),
            (11.243758037895, 85.273543857428),
        ),
        (
            Sky::Fk4,
            Sky::Galactic,
            (0.0, 0.0),
            (97.742200947606, -60.181023588486),
        ),
        (
            Sky::Fk5,
            Sky::Fk4,
            (150.0, 2.2),
            (149.353699298448, 2.440306819851),
        ),
        (
            Sky::Icrs,
            Sky::Fk4,
            (300.0, -60.0),
            (298.938167651733, -60.136987651267),
        ),
        (
            Sky::Galactic,
            Sky::Fk4,
            (120.0, -30.0),
            (9.168435818452, 32.528409348002),
        ),
    ];

    #[test]
    fn rotations_match_astropy() {
        // astropy 8.0: SkyCoord(ra, dec, frame=...).transform_to(...).
        let cases = [
            (
                Sky::Icrs,
                Sky::Fk5,
                (150.0, 2.2),
                (150.000_006_225_8, 2.199_995_047_0),
            ),
            (
                Sky::Icrs,
                Sky::Galactic,
                (150.0, 2.2),
                (236.735_603_085_9, 42.021_244_427_4),
            ),
            (
                Sky::Fk5,
                Sky::Galactic,
                (0.0, 90.0),
                (122.931_918_568_0, 27.128_251_180_9),
            ),
            // FK4 (B1950, obstime B1950) → FK5 J2000 / ICRS / galactic, and back.
            FK4_CASES[0],
            FK4_CASES[1],
            FK4_CASES[2],
            FK4_CASES[3],
            FK4_CASES[4],
            FK4_CASES[5],
        ];
        for (a, b, p, want) in cases {
            let got = convert(a, b, p.0, p.1);
            // astropy's own FK4 paths (through FK4NoETerms or straight to FK5) differ by up
            // to 25 µas: FK4 is held to 0.1 mas.
            let tol = if a == Sky::Fk4 || b == Sky::Fk4 {
                3e-8
            } else {
                2e-9
            };
            let off = crate::regions::separation(got.0, got.1, want.0, want.1);
            assert!(off < tol, "{a:?}→{b:?} {got:?} vs {want:?}: {off:e}°");
            let back = convert(b, a, got.0, got.1);
            assert!(crate::regions::separation(back.0, back.1, p.0, p.1) < 1e-9);
        }
    }
}
