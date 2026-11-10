//! Geodesics on the WGS 84 ellipsoid after C. F. F. Karney, "Algorithms for geodesics"
//! (J. Geodesy 87, 2013): the auxiliary sphere, series in the third flattening `n` and in
//! `ε` to sixth order, Newton's method on the azimuth for the inverse problem (with
//! bisection as a fallback) and the astroid starting guess for nearly antipodal points.
//! Distances agree with GeographicLib to a few nanometres.
//!
//! The area of a polygon sums, edge by edge, the area between the edge and the equator: the
//! spherical part from the azimuths, the ellipsoidal correction integrated in closed form
//! from a power series of its integrand (no truncated area series).
//!
//! Positions are (latitude, longitude) in degrees; azimuths are degrees clockwise from north.

use crate::angle::{atan2d, diff, norm2, normalize, sincosd};
use crate::{WGS84_A, WGS84_F};
use std::f64::consts::PI;
use std::sync::OnceLock;

/// Series order in `ε` (and `n`).
const ORDER: usize = 6;
const TINY: f64 = f64::MIN_POSITIVE * 1.0e10;
const TOL0: f64 = f64::EPSILON;
const MAXIT1: usize = 20;
const MAXIT2: usize = MAXIT1 + 53 + 10;

/// The result of the inverse problem.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inverse {
    /// Distance, metres.
    pub s12: f64,
    /// Azimuth at the first point, degrees in (−180, 180].
    pub azi1: f64,
    /// Azimuth at the second point (forward direction), degrees in (−180, 180].
    pub azi2: f64,
}

/// The ellipsoid's derived constants and the series coefficients that depend on `n` only.
struct Ell {
    a: f64,
    f: f64,
    f1: f64,
    e2: f64,
    ep2: f64,
    n: f64,
    b: f64,
    /// Authalic radius squared.
    c2: f64,
    etol2: f64,
    tol1: f64,
    tolb: f64,
    xthresh: f64,
    /// A3(ε) as a polynomial in ε (index = power).
    a3: [f64; ORDER],
    /// C3[l](ε) for l = 1..5 as polynomials in ε (index = power).
    c3: [[f64; ORDER]; ORDER],
    /// Coefficients q_m of the area integrand's divided difference in y = k² sin²σ.
    q: [f64; Q_TERMS],
}

const Q_TERMS: usize = 14;

fn ell() -> &'static Ell {
    static E: OnceLock<Ell> = OnceLock::new();
    E.get_or_init(|| {
        let (a, f) = (WGS84_A, WGS84_F);
        let f1 = 1.0 - f;
        let e2 = f * (2.0 - f);
        let ep2 = e2 / (f1 * f1);
        let n = f / (2.0 - f);
        let b = a * f1;
        let e = e2.sqrt();
        let c2 = (a * a + b * b * e.atanh() / e) / 2.0;
        let tol2 = TOL0.sqrt();
        let etol2 = 0.1 * tol2 / (f.abs().max(0.001) * (1.0 - f / 2.0).min(1.0) / 2.0).sqrt();
        let n2 = n * n;
        let a3 = [
            1.0,
            -(0.5 - 0.5 * n),
            -(0.25 + 0.125 * n - 0.375 * n2),
            -(1.0 / 16.0 + 3.0 / 16.0 * n + 1.0 / 16.0 * n2),
            -(3.0 / 64.0 + 1.0 / 32.0 * n),
            -3.0 / 128.0,
        ];
        let mut c3 = [[0.0; ORDER]; ORDER];
        c3[1] = [
            0.0,
            0.25 - 0.25 * n,
            0.125 - 0.125 * n2,
            3.0 / 64.0 + 3.0 / 64.0 * n - 1.0 / 64.0 * n2,
            5.0 / 128.0 + 1.0 / 64.0 * n,
            3.0 / 128.0,
        ];
        c3[2] = [
            0.0,
            0.0,
            1.0 / 16.0 - 3.0 / 32.0 * n + 1.0 / 32.0 * n2,
            3.0 / 64.0 - 1.0 / 32.0 * n - 3.0 / 64.0 * n2,
            3.0 / 128.0 + 1.0 / 128.0 * n,
            5.0 / 256.0,
        ];
        c3[3] = [
            0.0,
            0.0,
            0.0,
            5.0 / 192.0 - 3.0 / 64.0 * n + 5.0 / 192.0 * n2,
            3.0 / 128.0 - 5.0 / 192.0 * n,
            7.0 / 512.0,
        ];
        c3[4] = [
            0.0,
            0.0,
            0.0,
            0.0,
            7.0 / 512.0 - 7.0 / 256.0 * n,
            7.0 / 512.0,
        ];
        c3[5] = [0.0, 0.0, 0.0, 0.0, 0.0, 21.0 / 2560.0];
        Ell {
            a,
            f,
            f1,
            e2,
            ep2,
            n,
            b,
            c2,
            etol2,
            tol1: 200.0 * TOL0,
            tolb: TOL0 * tol2,
            xthresh: 1000.0 * tol2,
            a3,
            c3,
            q: area_q(ep2),
        }
    })
}

/// The area integrand needs (t(X) − t(y)) / (X − y) for X = e′² and y = k² sin²σ ∈ [0, X],
/// with t(x) = x + √(1 + 1/x) · asinh(√x). Both stay below e′² (≈ 0.0067), where
/// h(x) = t(x) − x = √(1 + x) · asinh(√x)/√x is a fast-converging power series Σ hₙ xⁿ, so
/// the divided difference is exactly 1 + Σ hₙ (Xⁿ − yⁿ)/(X − y): no cancellation near the
/// meridians. Returns its coefficients in powers of y.
fn area_q(x: f64) -> [f64; Q_TERMS] {
    let n = Q_TERMS + 1;
    // asinh(s)/s = Σ aₖ s^{2k}: aₖ = (−1)ᵏ (2k)! / (4ᵏ (k!)² (2k + 1)).
    let mut a = vec![0.0; n];
    let mut central = 1.0; // (2k)! / (4^k (k!)^2)
    for (k, v) in a.iter_mut().enumerate() {
        if k > 0 {
            central *= (2 * k - 1) as f64 / (2 * k) as f64;
        }
        let sign = if k % 2 == 0 { 1.0 } else { -1.0 };
        *v = sign * central / (2 * k + 1) as f64;
    }
    // √(1 + x) = Σ bₖ xᵏ, bₖ = C(1/2, k).
    let mut b = vec![0.0; n];
    b[0] = 1.0;
    for k in 1..n {
        b[k] = b[k - 1] * (0.5 - (k - 1) as f64) / k as f64;
    }
    let mut h = vec![0.0; n];
    for i in 0..n {
        for j in 0..n - i {
            h[i + j] += a[i] * b[j];
        }
    }
    let mut q = [0.0; Q_TERMS];
    q[0] = 1.0;
    for (m, qm) in q.iter_mut().enumerate() {
        let mut xp = 1.0;
        for hn in h.iter().skip(m + 1) {
            *qm += hn * xp;
            xp *= x;
        }
    }
    q
}

/// Σ c[l] sin(2lσ), l ≥ 1, from sin σ and cos σ (Clenshaw summation).
fn sin_series(sinx: f64, cosx: f64, c: &[f64]) -> f64 {
    let ar = 2.0 * (cosx - sinx) * (cosx + sinx);
    let (mut b1, mut b2) = (0.0, 0.0);
    for &ck in c[1..].iter().rev() {
        let b0 = ck + ar * b1 - b2;
        b2 = b1;
        b1 = b0;
    }
    2.0 * sinx * cosx * b1
}

fn poly(c: &[f64], x: f64) -> f64 {
    c.iter().rev().fold(0.0, |acc, v| acc * x + v)
}

/// A1 − 1.
fn a1m1(eps: f64) -> f64 {
    let e2 = eps * eps;
    let t = e2 * (e2 * (e2 + 4.0) + 64.0) / 256.0;
    (t + eps) / (1.0 - eps)
}

/// A2 − 1.
fn a2m1(eps: f64) -> f64 {
    let e2 = eps * eps;
    let t = -e2 * (e2 * (11.0 * e2 + 28.0) + 192.0) / 256.0;
    (t - eps) / (1.0 + eps)
}

fn c1(eps: f64) -> [f64; ORDER + 1] {
    let e = eps;
    let e2 = e * e;
    let (e3, e4, e5, e6) = (e2 * e, e2 * e2, e2 * e2 * e, e2 * e2 * e2);
    [
        0.0,
        -0.5 * e + 3.0 / 16.0 * e3 - 1.0 / 32.0 * e5,
        -1.0 / 16.0 * e2 + 1.0 / 32.0 * e4 - 9.0 / 2048.0 * e6,
        -1.0 / 48.0 * e3 + 3.0 / 256.0 * e5,
        -5.0 / 512.0 * e4 + 3.0 / 512.0 * e6,
        -7.0 / 1280.0 * e5,
        -7.0 / 2048.0 * e6,
    ]
}

fn c1p(eps: f64) -> [f64; ORDER + 1] {
    let e = eps;
    let e2 = e * e;
    let (e3, e4, e5, e6) = (e2 * e, e2 * e2, e2 * e2 * e, e2 * e2 * e2);
    [
        0.0,
        0.5 * e - 9.0 / 32.0 * e3 + 205.0 / 1536.0 * e5,
        5.0 / 16.0 * e2 - 37.0 / 96.0 * e4 + 1335.0 / 4096.0 * e6,
        29.0 / 96.0 * e3 - 75.0 / 128.0 * e5,
        539.0 / 1536.0 * e4 - 2391.0 / 2560.0 * e6,
        3467.0 / 7680.0 * e5,
        38081.0 / 61440.0 * e6,
    ]
}

fn c2(eps: f64) -> [f64; ORDER + 1] {
    let e = eps;
    let e2 = e * e;
    let (e3, e4, e5, e6) = (e2 * e, e2 * e2, e2 * e2 * e, e2 * e2 * e2);
    [
        0.0,
        0.5 * e + 1.0 / 16.0 * e3 + 1.0 / 32.0 * e5,
        3.0 / 16.0 * e2 + 1.0 / 32.0 * e4 + 35.0 / 2048.0 * e6,
        5.0 / 48.0 * e3 + 5.0 / 256.0 * e5,
        35.0 / 512.0 * e4 + 7.0 / 512.0 * e6,
        63.0 / 1280.0 * e5,
        77.0 / 2048.0 * e6,
    ]
}

impl Ell {
    fn a3f(&self, eps: f64) -> f64 {
        poly(&self.a3, eps)
    }

    fn c3f(&self, eps: f64) -> [f64; ORDER] {
        let mut c = [0.0; ORDER];
        for (l, cl) in c.iter_mut().enumerate().skip(1) {
            *cl = poly(&self.c3[l], eps);
        }
        c
    }

    /// Distance (in units of b) and reduced length (units of b) along a line of parameter
    /// `eps` from σ1 to σ2.
    #[allow(clippy::too_many_arguments)]
    fn lengths(
        &self,
        eps: f64,
        sig12: f64,
        (ssig1, csig1, dn1): (f64, f64, f64),
        (ssig2, csig2, dn2): (f64, f64, f64),
    ) -> (f64, f64) {
        let a1 = a1m1(eps);
        let a2 = a2m1(eps);
        let ca = c1(eps);
        let cb = c2(eps);
        let m0x = a1 - a2;
        let a1 = 1.0 + a1;
        let a2 = 1.0 + a2;
        let b1 = sin_series(ssig2, csig2, &ca) - sin_series(ssig1, csig1, &ca);
        let s12b = a1 * (sig12 + b1);
        let b2 = sin_series(ssig2, csig2, &cb) - sin_series(ssig1, csig1, &cb);
        let j12 = m0x * sig12 + (a1 * b1 - a2 * b2);
        let m12b = dn2 * (csig1 * ssig2) - dn1 * (ssig1 * csig2) - csig1 * csig2 * j12;
        (s12b, m12b)
    }

    /// The astroid's positive root κ (Karney 2013, eq. 55).
    fn astroid(x: f64, y: f64) -> f64 {
        let p = x * x;
        let q = y * y;
        let r = (p + q - 1.0) / 6.0;
        if q == 0.0 && r <= 0.0 {
            return 0.0;
        }
        let s = p * q / 4.0;
        let r2 = r * r;
        let r3 = r * r2;
        let disc = s * (s + 2.0 * r3);
        let mut u = r;
        if disc >= 0.0 {
            let mut t3 = s + r3;
            t3 += if t3 < 0.0 { -disc.sqrt() } else { disc.sqrt() };
            let t = t3.cbrt();
            u += t + if t != 0.0 { r2 / t } else { 0.0 };
        } else {
            let ang = (-disc).sqrt().atan2(-(s + r3));
            u += 2.0 * r * (ang / 3.0).cos();
        }
        let v = (u * u + q).sqrt();
        let uv = if u < 0.0 { q / (v - u) } else { u + v };
        let w = (uv - q) / (2.0 * v);
        uv / ((uv + w * w).sqrt() + w)
    }
}

/// What the inverse solution carries for the area.
struct Solved {
    s12: f64,
    salp1: f64,
    calp1: f64,
    salp2: f64,
    calp2: f64,
    /// The area between the geodesic and the equator, m² (signed, for 1 → 2).
    area: f64,
}

/// Lambda12 of Karney (2013): the longitude difference reached by a line leaving point 1 at
/// azimuth α1, minus the target, and its derivative.
struct Lam {
    v: f64,
    salp2: f64,
    calp2: f64,
    sig12: f64,
    ssig1: f64,
    csig1: f64,
    ssig2: f64,
    csig2: f64,
    eps: f64,
    domg12: f64,
    dv: f64,
}

#[allow(clippy::too_many_arguments)]
fn lambda12(
    e: &Ell,
    (sbet1, cbet1, dn1): (f64, f64, f64),
    (sbet2, cbet2, dn2): (f64, f64, f64),
    salp1: f64,
    calp1: f64,
    slam120: f64,
    clam120: f64,
    diffp: bool,
) -> Lam {
    let calp1 = if sbet1 == 0.0 && calp1 == 0.0 {
        -TINY
    } else {
        calp1
    };
    let salp0 = salp1 * cbet1;
    let calp0 = calp1.hypot(salp1 * sbet1);
    let mut ssig1 = sbet1;
    let somg1 = salp0 * sbet1;
    let mut csig1 = calp1 * cbet1;
    let comg1 = csig1;
    norm2(&mut ssig1, &mut csig1);
    let salp2 = if cbet2 != cbet1 { salp0 / cbet2 } else { salp1 };
    let calp2 = if cbet2 != cbet1 || sbet2.abs() != -sbet1 {
        ((calp1 * cbet1).powi(2)
            + if cbet1 < -sbet1 {
                (cbet2 - cbet1) * (cbet1 + cbet2)
            } else {
                (sbet1 - sbet2) * (sbet1 + sbet2)
            })
        .sqrt()
            / cbet2
    } else {
        calp1.abs()
    };
    let mut ssig2 = sbet2;
    let somg2 = salp0 * sbet2;
    let mut csig2 = calp2 * cbet2;
    let comg2 = csig2;
    norm2(&mut ssig2, &mut csig2);
    let sig12 = (csig1 * ssig2 - ssig1 * csig2)
        .max(0.0)
        .atan2(csig1 * csig2 + ssig1 * ssig2);
    let somg12 = (comg1 * somg2 - somg1 * comg2).max(0.0);
    let comg12 = comg1 * comg2 + somg1 * somg2;
    let eta = (somg12 * clam120 - comg12 * slam120).atan2(comg12 * clam120 + somg12 * slam120);
    let k2 = calp0 * calp0 * e.ep2;
    let eps = k2 / (2.0 * (1.0 + (1.0 + k2).sqrt()) + k2);
    let c3 = e.c3f(eps);
    let b312 = sin_series(ssig2, csig2, &c3) - sin_series(ssig1, csig1, &c3);
    let domg12 = -e.f * e.a3f(eps) * salp0 * (sig12 + b312);
    let v = eta + domg12;
    let dv = if diffp {
        if calp2 == 0.0 {
            -2.0 * e.f1 * dn1 / sbet1
        } else {
            let (_, m12b) = e.lengths(eps, sig12, (ssig1, csig1, dn1), (ssig2, csig2, dn2));
            m12b * e.f1 / (calp2 * cbet2)
        }
    } else {
        0.0
    };
    Lam {
        v,
        salp2,
        calp2,
        sig12,
        ssig1,
        csig1,
        ssig2,
        csig2,
        eps,
        domg12,
        dv,
    }
}

/// The reduced latitude's sine and cosine, and √(1 + e′² sin²β).
fn reduced(e: &Ell, lat: f64) -> (f64, f64, f64) {
    let (s, c) = sincosd(lat);
    let mut sb = e.f1 * s;
    let mut cb = c;
    norm2(&mut sb, &mut cb);
    let cb = cb.max(TINY);
    (sb, cb, (1.0 + e.ep2 * sb * sb).sqrt())
}

/// The ellipsoidal part of the area between σ1 and σ2 on a line of `calp0`, `salp0`:
/// e²a² cos α0 sin α0 (I4(σ2) − I4(σ1)), with I4′(σ) = −Q(k² sin²σ) sin σ / 2 integrated in
/// closed form term by term (∫ sin^{2m+1} = −∫ (1 − u²)^m du, u = cos σ).
fn area_ellipsoidal(e: &Ell, salp0: f64, calp0: f64, csig1: f64, csig2: f64) -> f64 {
    if salp0 == 0.0 || calp0 == 0.0 {
        return 0.0;
    }
    let k2 = calp0 * calp0 * e.ep2;
    // P_m(u) = ∫₀ᵘ (1 − v²)^m dv = Σ_j C(m, j) (−1)^j u^{2j+1}/(2j + 1).
    let pm = |m: usize, u: f64| {
        let mut sum = 0.0;
        let mut binom = 1.0;
        let u2 = u * u;
        let mut up = u;
        for j in 0..=m {
            if j > 0 {
                binom *= (m + 1 - j) as f64 / j as f64;
            }
            let sign = if j % 2 == 0 { 1.0 } else { -1.0 };
            sum += sign * binom * up / (2 * j + 1) as f64;
            up *= u2;
        }
        sum
    };
    let mut d = 0.0;
    let mut kp = 1.0;
    for (m, qm) in e.q.iter().enumerate() {
        let term = qm * kp * (pm(m, csig2) - pm(m, csig1));
        d += term;
        kp *= k2;
        if kp < 1e-30 {
            break;
        }
    }
    // I4(σ2) − I4(σ1) = −½ ∫ Q sin σ dσ = ½ Σ q_m k^{2m} (P_m(cos σ2) − P_m(cos σ1)).
    e.e2 * e.a * e.a * calp0 * salp0 * 0.5 * d
}

fn solve(lat1: f64, lon1: f64, lat2: f64, lon2: f64, want_area: bool) -> Solved {
    let e = ell();
    let lat1 = lat1.clamp(-90.0, 90.0);
    let lat2 = lat2.clamp(-90.0, 90.0);
    let mut lon12 = diff(lon1, lon2);
    let mut lonsign = if lon12.is_sign_negative() { -1.0 } else { 1.0 };
    lon12 *= lonsign;
    if lon12 == 0.0 {
        lonsign = 1.0;
    }
    let lon12s = 180.0 - lon12;
    // Put the point with the larger |latitude| first, and make it ≤ 0.
    let swapp = if lat1.abs() < lat2.abs() { -1.0 } else { 1.0 };
    let (mut lat1, mut lat2) = (lat1, lat2);
    if swapp < 0.0 {
        lonsign = -lonsign;
        std::mem::swap(&mut lat1, &mut lat2);
    }
    let latsign = if lat1 < 0.0 { 1.0 } else { -1.0 };
    lat1 *= latsign;
    lat2 *= latsign;
    let (sbet1, cbet1, dn1) = reduced(e, lat1);
    let (mut sbet2, mut cbet2, dn2) = reduced(e, lat2);
    // Make the symmetric cases come out exactly symmetric.
    if cbet1 < -sbet1 {
        if cbet2 == cbet1 {
            sbet2 = sbet1.copysign(sbet2);
        }
    } else if sbet2.abs() == -sbet1 {
        cbet2 = cbet1;
    }
    let lam12 = lon12.to_radians();
    let (slam12, clam12) = if lon12 > 90.0 {
        let (s, c) = sincosd(lon12s);
        (s, -c)
    } else {
        sincosd(lon12)
    };
    let p1 = (sbet1, cbet1, dn1);
    let p2 = (sbet2, cbet2, dn2);

    let (mut salp1, mut calp1, mut salp2, mut calp2);
    let mut s12x = 0.0;
    let mut omg12 = None::<(f64, f64)>;
    let mut sig12 = 0.0;
    let mut meridian = lat1 == -90.0 || slam12 == 0.0;
    // For the area: σ1, σ2 as found.
    let (mut csig1_area, mut csig2_area) = (0.0, 0.0);
    let mut have_sig = false;

    salp1 = 0.0;
    calp1 = 0.0;
    salp2 = 0.0;
    calp2 = 0.0;
    if meridian {
        calp1 = clam12;
        salp1 = slam12;
        calp2 = 1.0;
        salp2 = 0.0;
        let ssig1 = sbet1;
        let csig1 = calp1 * cbet1;
        let ssig2 = sbet2;
        let csig2 = calp2 * cbet2;
        sig12 = (csig1 * ssig2 - ssig1 * csig2)
            .max(0.0)
            .atan2(csig1 * csig2 + ssig1 * ssig2);
        let (s, m) = e.lengths(e.n, sig12, (ssig1, csig1, dn1), (ssig2, csig2, dn2));
        if sig12 < 1.0 || m >= 0.0 {
            s12x = s * e.b;
        } else {
            meridian = false;
        }
    }
    let mut done = meridian;
    if !done && sbet1 == 0.0 && lon12s >= e.f * 180.0 {
        // Along the equator.
        calp1 = 0.0;
        calp2 = 0.0;
        salp1 = 1.0;
        salp2 = 1.0;
        s12x = e.a * lam12;
        let o = lam12 / e.f1;
        omg12 = Some((o.sin(), o.cos()));
        done = true;
    }
    if !done {
        // The starting guess, and the solution itself for short lines.
        let sbet12 = sbet2 * cbet1 - cbet2 * sbet1;
        let cbet12 = cbet2 * cbet1 + sbet2 * sbet1;
        let sbet12a = sbet2 * cbet1 + cbet2 * sbet1;
        let shortline = cbet12 >= 0.0 && sbet12 < 0.5 && cbet2 * lam12 < 0.5;
        let (mut somg12, mut comg12) = (slam12, clam12);
        let mut dnm = 1.0;
        if shortline {
            let mut sbetm2 = (sbet1 + sbet2).powi(2);
            sbetm2 /= sbetm2 + (cbet1 + cbet2).powi(2);
            dnm = (1.0 + e.ep2 * sbetm2).sqrt();
            let o = lam12 / (e.f1 * dnm);
            somg12 = o.sin();
            comg12 = o.cos();
        }
        salp1 = cbet2 * somg12;
        calp1 = if comg12 >= 0.0 {
            sbet12 + cbet2 * sbet1 * somg12 * somg12 / (1.0 + comg12)
        } else {
            sbet12a - cbet2 * sbet1 * somg12 * somg12 / (1.0 - comg12)
        };
        let ssig12 = salp1.hypot(calp1);
        let csig12 = sbet1 * sbet2 + cbet1 * cbet2 * comg12;
        let mut solved_short = false;
        if shortline && ssig12 < e.etol2 {
            salp2 = cbet1 * somg12;
            calp2 = sbet12
                - cbet1
                    * sbet2
                    * if comg12 >= 0.0 {
                        somg12 * somg12 / (1.0 + comg12)
                    } else {
                        1.0 - comg12
                    };
            norm2(&mut salp2, &mut calp2);
            sig12 = ssig12.atan2(csig12);
            solved_short = true;
        } else if e.n.abs() > 0.1 || csig12 >= 0.0 || ssig12 >= 6.0 * e.n.abs() * PI * cbet1 * cbet1
        {
            // The spherical guess is good enough.
        } else {
            // Nearly antipodal: the astroid.
            let lam12x = (-slam12).atan2(-clam12);
            let k2 = sbet1 * sbet1 * e.ep2;
            let eps = k2 / (2.0 * (1.0 + (1.0 + k2).sqrt()) + k2);
            let lamscale = e.f * cbet1 * e.a3f(eps) * PI;
            let betscale = lamscale * cbet1;
            let x = lam12x / lamscale;
            let y = sbet12a / betscale;
            if y > -e.tol1 && x > -1.0 - e.xthresh {
                salp1 = (-x).min(1.0);
                calp1 = -(1.0 - salp1 * salp1).sqrt();
            } else {
                let k = Ell::astroid(x, y);
                let omg12a = lamscale * (-x * k / (1.0 + k));
                let (s, c) = omg12a.sin_cos();
                let (somg, comg) = (s, -c);
                salp1 = cbet2 * somg;
                calp1 = sbet12a - cbet2 * sbet1 * somg * somg / (1.0 - comg);
            }
        }
        if salp1 > 0.0 {
            norm2(&mut salp1, &mut calp1);
        } else {
            salp1 = 1.0;
            calp1 = 0.0;
        }
        if solved_short {
            s12x = sig12 * e.b * dnm;
            let o = lam12 / (e.f1 * dnm);
            omg12 = Some((o.sin(), o.cos()));
        } else {
            // Newton's method on α1, bracketed.
            let (mut salp1a, mut calp1a) = (TINY, 1.0);
            let (mut salp1b, mut calp1b) = (TINY, -1.0);
            let mut tripn = false;
            let mut tripb = false;
            let mut last = None;
            for numit in 0..MAXIT2 {
                let l = lambda12(e, p1, p2, salp1, calp1, slam12, clam12, numit < MAXIT1);
                let v = l.v;
                let dv = l.dv;
                last = Some(l);
                if tripb || v.abs() < if tripn { 8.0 } else { 1.0 } * TOL0 {
                    break;
                }
                if v > 0.0 && (numit > MAXIT1 || calp1 / salp1 > calp1b / salp1b) {
                    salp1b = salp1;
                    calp1b = calp1;
                } else if v < 0.0 && (numit > MAXIT1 || calp1 / salp1 < calp1a / salp1a) {
                    salp1a = salp1;
                    calp1a = calp1;
                }
                if numit < MAXIT1 && dv > 0.0 {
                    let dalp1 = -v / dv;
                    if dalp1.abs() < PI {
                        let (sd, cd) = dalp1.sin_cos();
                        let nsalp1 = salp1 * cd + calp1 * sd;
                        if nsalp1 > 0.0 {
                            calp1 = calp1 * cd - salp1 * sd;
                            salp1 = nsalp1;
                            norm2(&mut salp1, &mut calp1);
                            tripn = v.abs() <= 16.0 * TOL0;
                            continue;
                        }
                    }
                }
                salp1 = (salp1a + salp1b) / 2.0;
                calp1 = (calp1a + calp1b) / 2.0;
                norm2(&mut salp1, &mut calp1);
                tripn = false;
                tripb = (salp1a - salp1).abs() + (calp1a - calp1) < e.tolb
                    || (salp1 - salp1b).abs() + (calp1 - calp1b) < e.tolb;
            }
            let l = last.expect("at least one iteration");
            salp2 = l.salp2;
            calp2 = l.calp2;
            let (s, _) = e.lengths(
                l.eps,
                l.sig12,
                (l.ssig1, l.csig1, dn1),
                (l.ssig2, l.csig2, dn2),
            );
            s12x = s * e.b;
            let o = lam12 - l.domg12;
            omg12 = Some((o.sin(), o.cos()));
            csig1_area = l.csig1;
            csig2_area = l.csig2;
            have_sig = true;
        }
    }

    let mut area = 0.0;
    if want_area {
        let salp0 = salp1 * cbet1;
        let calp0 = calp1.hypot(salp1 * sbet1);
        if calp0 != 0.0 && salp0 != 0.0 {
            let (csig1, csig2) = if have_sig {
                (csig1_area, csig2_area)
            } else {
                let (mut s1, mut c1) = (sbet1, calp1 * cbet1);
                let (mut s2, mut c2) = (sbet2, calp2 * cbet2);
                norm2(&mut s1, &mut c1);
                norm2(&mut s2, &mut c2);
                (c1, c2)
            };
            area = area_ellipsoidal(e, salp0, calp0, csig1, csig2);
        }
        let alp12 = match omg12 {
            Some((somg12, comg12))
                if !meridian
                    && comg12 > -std::f64::consts::FRAC_1_SQRT_2
                    && sbet2 - sbet1 < 1.75 =>
            {
                // Exact on the auxiliary sphere, and accurate for short lines.
                let domg12 = 1.0 + comg12;
                let dbet1 = 1.0 + cbet1;
                let dbet2 = 1.0 + cbet2;
                2.0 * (somg12 * (sbet1 * dbet2 + sbet2 * dbet1))
                    .atan2(domg12 * (sbet1 * sbet2 + dbet1 * dbet2))
            }
            _ => {
                let mut salp12 = salp2 * calp1 - calp2 * salp1;
                let mut calp12 = calp2 * calp1 + salp2 * salp1;
                if salp12 == 0.0 && calp12 < 0.0 {
                    salp12 = TINY * calp1;
                    calp12 = -1.0;
                }
                salp12.atan2(calp12)
            }
        };
        area += e.c2 * alp12;
        area *= swapp * lonsign * latsign;
        area += 0.0;
    }

    if swapp < 0.0 {
        std::mem::swap(&mut salp1, &mut salp2);
        std::mem::swap(&mut calp1, &mut calp2);
    }
    salp1 *= swapp * lonsign;
    calp1 *= swapp * latsign;
    salp2 *= swapp * lonsign;
    calp2 *= swapp * latsign;
    Solved {
        s12: s12x + 0.0,
        salp1,
        calp1,
        salp2,
        calp2,
        area,
    }
}

/// Distance and azimuths between two points (the shortest geodesic).
pub fn inverse(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> Inverse {
    let s = solve(lat1, lon1, lat2, lon2, false);
    Inverse {
        s12: s.s12,
        azi1: atan2d(s.salp1, s.calp1),
        azi2: atan2d(s.salp2, s.calp2),
    }
}

/// The point `s12` metres from (`lat1`, `lon1`) along azimuth `azi1`: its latitude,
/// longitude (in (−180, 180]) and forward azimuth.
pub fn direct(lat1: f64, lon1: f64, azi1: f64, s12: f64) -> (f64, f64, f64) {
    let e = ell();
    let lat1 = lat1.clamp(-90.0, 90.0);
    let azi1 = normalize(azi1);
    let (salp1, calp1) = sincosd(azi1);
    let (sbet1, cbet1, _) = reduced(e, lat1);
    let salp0 = salp1 * cbet1;
    let calp0 = calp1.hypot(salp1 * sbet1);
    let mut ssig1 = sbet1;
    let somg1 = salp0 * sbet1;
    let mut csig1 = if sbet1 != 0.0 || calp1 != 0.0 {
        cbet1 * calp1
    } else {
        1.0
    };
    let comg1 = csig1;
    norm2(&mut ssig1, &mut csig1);
    let k2 = calp0 * calp0 * e.ep2;
    let eps = k2 / (2.0 * (1.0 + (1.0 + k2).sqrt()) + k2);
    let a1 = 1.0 + a1m1(eps);
    let c1a = c1(eps);
    let b11 = sin_series(ssig1, csig1, &c1a);
    let (s, c) = b11.sin_cos();
    let stau1 = ssig1 * c + csig1 * s;
    let ctau1 = csig1 * c - ssig1 * s;
    let c1pa = c1p(eps);
    let a3c = -e.f * salp0 * e.a3f(eps);
    let c3a = e.c3f(eps);
    let b31 = sin_series(ssig1, csig1, &c3a);

    let tau12 = s12 / (e.b * a1);
    let (s, c) = tau12.sin_cos();
    let b12 = -sin_series(stau1 * c + ctau1 * s, ctau1 * c - stau1 * s, &c1pa);
    let sig12 = tau12 - (b12 - b11);
    let (ssig12, csig12) = sig12.sin_cos();
    let ssig2 = ssig1 * csig12 + csig1 * ssig12;
    let mut csig2 = csig1 * csig12 - ssig1 * ssig12;
    let sbet2 = calp0 * ssig2;
    let mut cbet2 = salp0.hypot(calp0 * csig2);
    if cbet2 == 0.0 {
        cbet2 = TINY;
        csig2 = TINY;
    }
    let salp2 = salp0;
    let calp2 = calp0 * csig2;
    let somg2 = salp0 * ssig2;
    let comg2 = csig2;
    let sgn = 1.0f64.copysign(salp0);
    let omg12 = sgn
        * (sig12 - (ssig2.atan2(csig2) - ssig1.atan2(csig1))
            + ((sgn * somg2).atan2(comg2) - (sgn * somg1).atan2(comg1)));
    let b32 = sin_series(ssig2, csig2, &c3a);
    let lam12 = omg12 + a3c * (sig12 + (b32 - b31));
    let lon12 = lam12.to_degrees();
    let lon2 = normalize(normalize(lon1) + normalize(lon12));
    let lat2 = atan2d(sbet2, e.f1 * cbet2);
    let azi2 = atan2d(salp2, calp2);
    (lat2, lon2, azi2)
}

/// The length of an open path of (latitude, longitude) vertices, metres.
pub fn path_length(pts: &[(f64, f64)]) -> f64 {
    pts.windows(2)
        .map(|w| inverse(w[0].0, w[0].1, w[1].0, w[1].1).s12)
        .sum()
}

/// Whether the edge from `lon1` to `lon2` crosses the prime meridian eastward (1),
/// westward (−1) or not (0).
fn transit(lon1: f64, lon2: f64) -> i32 {
    let lon12 = diff(lon1, lon2);
    let lon1 = normalize(lon1);
    let lon2 = normalize(lon2);
    if lon12 > 0.0 && ((lon1 < 0.0 && lon2 >= 0.0) || (lon1 > 0.0 && lon2 == 0.0)) {
        1
    } else if lon12 < 0.0 && lon1 >= 0.0 && lon2 < 0.0 {
        -1
    } else {
        0
    }
}

/// The area enclosed by a ring of (latitude, longitude) vertices joined by geodesics (closed
/// implicitly) and its perimeter: (|area| m², perimeter m). Of the two regions a ring
/// bounds, the smaller is measured; rings around a pole work.
pub fn polygon_area(pts: &[(f64, f64)]) -> (f64, f64) {
    if pts.len() < 2 {
        return (0.0, 0.0);
    }
    let e = ell();
    let mut area = 0.0;
    let mut comp = 0.0; // Kahan compensation
    let mut perimeter = 0.0;
    let mut crossings = 0;
    let n = pts.len();
    for i in 0..n {
        let (la1, lo1) = pts[i];
        let (la2, lo2) = pts[(i + 1) % n];
        let s = solve(la1, lo1, la2, lo2, true);
        perimeter += s.s12;
        let y = s.area - comp;
        let t = area + y;
        comp = (t - area) - y;
        area = t;
        crossings += transit(lo1, lo2);
    }
    let area0 = 4.0 * PI * e.c2;
    let mut a = area;
    if crossings % 2 != 0 {
        a += if a < 0.0 { 1.0 } else { -1.0 } * area0 / 2.0;
    }
    // Into (−area0/2, area0/2], without losing a small area to the reduction.
    if a > area0 / 2.0 {
        a -= area0 * ((a - area0 / 2.0) / area0).ceil().max(1.0);
    } else if a <= -area0 / 2.0 {
        a += area0 * ((-area0 / 2.0 - a) / area0).ceil().max(1.0);
    }
    (a.abs(), perimeter)
}

/// The point at fraction `t` (0 at the first point, 1 at the second) along the geodesic.
pub fn interpolate(lat1: f64, lon1: f64, lat2: f64, lon2: f64, t: f64) -> (f64, f64) {
    let inv = inverse(lat1, lon1, lat2, lon2);
    if inv.s12 == 0.0 {
        return (lat1, normalize(lon1));
    }
    let (la, lo, _) = direct(lat1, lon1, inv.azi1, inv.s12 * t);
    (la, lo)
}

/// The initial bearing from the first point to the second, degrees in [0, 360).
pub fn bearing(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let a = inverse(lat1, lon1, lat2, lon2).azi1;
    let b = if a < 0.0 { a + 360.0 } else { a };
    if b >= 360.0 { 0.0 } else { b + 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_cases() {
        let e = ell();
        // Along the equator: a × Δλ.
        let r = inverse(0.0, 0.0, 0.0, 1.0);
        assert!((r.s12 - e.a * 1f64.to_radians()).abs() < 1e-9, "{r:?}");
        assert_eq!(r.azi1, 90.0);
        // Pole to pole along a meridian: twice the quarter meridian (10 001 965.729 m).
        let r = inverse(-90.0, 0.0, 90.0, 0.0);
        assert!((r.s12 - 2.0 * 10_001_965.729_312_5).abs() < 1e-3, "{r:?}");
        // Coincident points.
        let r = inverse(12.0, 34.0, 12.0, 34.0);
        assert_eq!(r.s12, 0.0);
        // Direct undoes inverse.
        let r = inverse(40.0, -75.0, 51.5, -0.1);
        let (la, lo, az) = direct(40.0, -75.0, r.azi1, r.s12);
        assert!(
            (la - 51.5).abs() < 1e-12 && (lo + 0.1).abs() < 1e-12,
            "{la} {lo}"
        );
        assert!((az - r.azi2).abs() < 1e-9);
        assert!((bearing(0.0, 0.0, 1.0, 0.0)).abs() < 1e-12);
        assert!((bearing(0.0, 0.0, 0.0, -1.0) - 270.0).abs() < 1e-12);
        let (la, lo) = interpolate(0.0, 0.0, 0.0, 10.0, 0.5);
        assert!(la.abs() < 1e-12 && (lo - 5.0).abs() < 1e-9);
    }

    #[test]
    fn area_of_simple_rings() {
        // A 1° square at the equator, about 12 308 km².
        let (a, p) = polygon_area(&[(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0)]);
        assert!((a - 1.2308778361e10).abs() / a < 1e-4, "{a}");
        assert!(p > 4.4e5 && p < 4.5e5, "{p}");
        // Orientation does not matter.
        let (b, _) = polygon_area(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]);
        assert!((a - b).abs() < 1e-3);
        // A ring around the north pole along 80° N: the cap above it.
        let ring: Vec<(f64, f64)> = (0..36).map(|i| (80.0, i as f64 * 10.0)).collect();
        let (cap, _) = polygon_area(&ring);
        assert!(cap > 3.8e12 && cap < 4.0e12, "{cap}");
    }
}
