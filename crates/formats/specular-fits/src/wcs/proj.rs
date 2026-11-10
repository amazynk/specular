//! The spherical projections of FITS WCS Paper II (Calabretta & Greisen 2002) and the
//! HEALPix projections (Calabretta & Roukema 2007): native spherical (φ, θ) ↔ projection
//! plane (x, y), all in degrees with the sphere's radius R₀ = 180°/π.
//!
//! Inverses that have no closed form (ZPN, AIR, PCO) are solved by bracketed bisection to
//! machine precision rather than to a tolerance.

use super::{D2R, R2D, Unsupported, wrap180};

/// R₀ = 180°/π: the projection plane is in degrees.
const R0: f64 = R2D;
const SQRT2: f64 = std::f64::consts::SQRT_2;

/// A celestial projection (`CTYPE` code).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Projection {
    // Zenithal.
    Azp,
    Szp,
    Tan,
    Stg,
    Sin,
    Arc,
    Zpn,
    Zea,
    Air,
    /// Legacy north-celestial-pole (AIPS): `SIN` with ξ = 0, η = cot δ₀.
    Ncp,
    // Cylindrical.
    Cyp,
    Cea,
    Car,
    Mer,
    // Pseudo-cylindrical.
    Sfl,
    /// Legacy global sinusoidal (AIPS): `SFL` with the reference point on the equator.
    Gls,
    Par,
    Mol,
    Ait,
    // Conic.
    Cop,
    Coe,
    Cod,
    Coo,
    // Polyconic and pseudo-conic.
    Bon,
    Pco,
    // Quad-cube.
    Tsc,
    Csc,
    Qsc,
    // HEALPix.
    Hpx,
    Xph,
}

impl Projection {
    pub(super) fn from_code(c: &str) -> Option<Projection> {
        use Projection::*;
        Some(match c {
            "AZP" => Azp,
            "SZP" => Szp,
            "TAN" => Tan,
            "STG" => Stg,
            "SIN" => Sin,
            "ARC" => Arc,
            "ZPN" => Zpn,
            "ZEA" => Zea,
            "AIR" => Air,
            "NCP" => Ncp,
            "CYP" => Cyp,
            "CEA" => Cea,
            "CAR" => Car,
            "MER" => Mer,
            "SFL" => Sfl,
            "GLS" => Gls,
            "PAR" => Par,
            "MOL" => Mol,
            "AIT" => Ait,
            "COP" => Cop,
            "COE" => Coe,
            "COD" => Cod,
            "COO" => Coo,
            "BON" => Bon,
            "PCO" => Pco,
            "TSC" => Tsc,
            "CSC" => Csc,
            "QSC" => Qsc,
            "HPX" => Hpx,
            "XPH" => Xph,
            _ => return None,
        })
    }

    pub fn code(self) -> &'static str {
        use Projection::*;
        match self {
            Azp => "AZP",
            Szp => "SZP",
            Tan => "TAN",
            Stg => "STG",
            Sin => "SIN",
            Arc => "ARC",
            Zpn => "ZPN",
            Zea => "ZEA",
            Air => "AIR",
            Ncp => "NCP",
            Cyp => "CYP",
            Cea => "CEA",
            Car => "CAR",
            Mer => "MER",
            Sfl => "SFL",
            Gls => "GLS",
            Par => "PAR",
            Mol => "MOL",
            Ait => "AIT",
            Cop => "COP",
            Coe => "COE",
            Cod => "COD",
            Coo => "COO",
            Bon => "BON",
            Pco => "PCO",
            Tsc => "TSC",
            Csc => "CSC",
            Qsc => "QSC",
            Hpx => "HPX",
            Xph => "XPH",
        }
    }
}

/// The projection equations with their parameters.
#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Azp {
        mu: f64,
        cg: f64,
        sg: f64,
        tg: f64,
    },
    Szp {
        xp: f64,
        yp: f64,
        zp: f64,
    },
    Tan,
    Stg,
    Sin {
        xi: f64,
        eta: f64,
    },
    Arc,
    /// Coefficients of R(ζ)/R₀, ζ = 90° − θ in radians; the first turning point ζmax.
    Zpn {
        c: Vec<f64>,
        zmax: f64,
        rmax: f64,
    },
    Zea,
    /// ln(cos ξb) / tan²ξb (−½ for θb = 90°).
    Air {
        cb: f64,
    },
    Cyp {
        mu: f64,
        lambda: f64,
    },
    Cea {
        lambda: f64,
    },
    Car,
    Mer,
    Sfl,
    Par,
    Mol,
    Ait,
    Conic(Conic),
    Bon {
        y0: f64,
    },
    Pco,
    Cube(Cube),
    Hpx {
        h: f64,
        k: f64,
    },
    Xph,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Cube {
    Tsc,
    Csc,
    Qsc,
}

/// A conic: x = R sin Cφ, y = −R cos Cφ + Y₀, with R(θ) per kind.
#[derive(Clone, Debug, PartialEq)]
struct Conic {
    kind: ConicKind,
    /// θa, the cone constant C and Y₀ = R(θa).
    ta: f64,
    c: f64,
    y0: f64,
}

#[derive(Clone, Debug, PartialEq)]
enum ConicKind {
    /// R₀ cos η.
    Cop { k: f64, cot: f64 },
    /// γ = sin θ₁ + sin θ₂ and 1 + sin θ₁ sin θ₂.
    Coe { gamma: f64, s: f64 },
    /// R(θ) = θa − θ + t.
    Cod { t: f64 },
    /// R(θ) = ψ tan^C((90° − θ)/2).
    Coo { psi: f64 },
}

/// A projection set up from its parameters, with the fiducial point and the plane offset
/// of Paper II §2.5.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Prj {
    pub code: Projection,
    kind: Kind,
    /// Native coordinates of the fiducial point.
    pub phi0: f64,
    pub theta0: f64,
    /// Plane offset: (x₀, y₀) is subtracted from projected coordinates (`PVi_0` true).
    x0: f64,
    y0: f64,
}

/// Projection parameters: `pv(m)` reads `PVi_m` of the latitude axis.
pub(super) struct Params<'a> {
    pub pv: &'a dyn Fn(usize) -> Option<f64>,
    /// The reference point's latitude (`CRVAL` of the latitude axis), for NCP and GLS.
    pub delta0: f64,
}

fn bad(code: Projection) -> Unsupported {
    Unsupported::Parameters(code.code().to_owned())
}

impl Prj {
    /// Sets up `code` from its parameters; `fiducial` is `(φ₀, θ₀, offset)` from the
    /// longitude axis' `PVi_0..2` when given.
    pub fn new(
        code: Projection,
        p: &Params<'_>,
        fiducial: (Option<f64>, Option<f64>, bool),
    ) -> Result<Prj, Unsupported> {
        use Projection as P;
        let pv = |m: usize, d: f64| (p.pv)(m).unwrap_or(d);
        let mut theta0 = 0.0;
        let mut phi0 = 0.0;
        let kind = match code {
            P::Azp => {
                let mu = pv(1, 0.0);
                let g = pv(2, 0.0);
                let (sg, cg) = (g * D2R).sin_cos();
                if mu == -1.0 || cg == 0.0 {
                    return Err(bad(code));
                }
                theta0 = 90.0;
                Kind::Azp {
                    mu,
                    cg,
                    sg,
                    tg: sg / cg,
                }
            }
            P::Szp => {
                let mu = pv(1, 0.0);
                let (sp, cp) = (pv(2, 0.0) * D2R).sin_cos();
                let (st, ct) = (pv(3, 90.0) * D2R).sin_cos();
                let zp = mu * st + 1.0;
                if zp == 0.0 {
                    return Err(bad(code));
                }
                theta0 = 90.0;
                Kind::Szp {
                    xp: -mu * ct * sp,
                    yp: mu * ct * cp,
                    zp,
                }
            }
            P::Tan => {
                theta0 = 90.0;
                Kind::Tan
            }
            P::Stg => {
                theta0 = 90.0;
                Kind::Stg
            }
            P::Sin => {
                theta0 = 90.0;
                Kind::Sin {
                    xi: pv(1, 0.0),
                    eta: pv(2, 0.0),
                }
            }
            P::Ncp => {
                // wcslib's fix (and AIPS): SIN with η = cot δ₀.
                let t = (p.delta0 * D2R).tan();
                if t == 0.0 {
                    return Err(bad(code));
                }
                theta0 = 90.0;
                Kind::Sin {
                    xi: 0.0,
                    eta: 1.0 / t,
                }
            }
            P::Arc => {
                theta0 = 90.0;
                Kind::Arc
            }
            P::Zpn => {
                theta0 = 90.0;
                zpn(code, &(0..30).map(|m| pv(m, 0.0)).collect::<Vec<_>>())?
            }
            P::Zea => {
                theta0 = 90.0;
                Kind::Zea
            }
            P::Air => {
                theta0 = 90.0;
                let tb = pv(1, 90.0);
                let xb = (90.0 - tb) * D2R / 2.0;
                let cb = if xb.abs() < 1e-12 {
                    -0.5
                } else if xb.abs() < std::f64::consts::FRAC_PI_2 {
                    let t = xb.tan();
                    xb.cos().ln() / (t * t)
                } else {
                    return Err(bad(code));
                };
                Kind::Air { cb }
            }
            P::Cyp => {
                let mu = pv(1, 1.0);
                let lambda = pv(2, 1.0);
                if lambda <= 0.0 || mu == -lambda {
                    return Err(bad(code));
                }
                Kind::Cyp { mu, lambda }
            }
            P::Cea => {
                let lambda = pv(1, 1.0);
                if !(lambda > 0.0 && lambda <= 1.0) {
                    return Err(bad(code));
                }
                Kind::Cea { lambda }
            }
            P::Car => Kind::Car,
            P::Mer => Kind::Mer,
            P::Sfl => Kind::Sfl,
            P::Gls => Kind::Sfl,
            P::Par => Kind::Par,
            P::Mol => Kind::Mol,
            P::Ait => Kind::Ait,
            P::Cop | P::Coe | P::Cod | P::Coo => {
                let ta = (p.pv)(1).ok_or_else(|| bad(code))?;
                let eta = pv(2, 0.0);
                theta0 = ta;
                Kind::Conic(conic(code, ta, eta)?)
            }
            P::Bon => {
                let t1 = (p.pv)(1).ok_or_else(|| bad(code))?;
                if t1 == 0.0 {
                    // Bonne with θ₁ = 0 is the sinusoidal projection.
                    Kind::Sfl
                } else if t1.abs() > 90.0 {
                    return Err(bad(code));
                } else {
                    Kind::Bon {
                        y0: R0 / (t1 * D2R).tan() + t1,
                    }
                }
            }
            P::Pco => Kind::Pco,
            P::Tsc => Kind::Cube(Cube::Tsc),
            P::Csc => Kind::Cube(Cube::Csc),
            P::Qsc => Kind::Cube(Cube::Qsc),
            P::Hpx => {
                let h = pv(1, 4.0);
                let k = pv(2, 3.0);
                if h < 1.0 || h.fract() != 0.0 || k < 1.0 || k.fract() != 0.0 {
                    return Err(bad(code));
                }
                Kind::Hpx { h, k }
            }
            P::Xph => {
                theta0 = 90.0;
                Kind::Xph
            }
        };
        let mut prj = Prj {
            code,
            kind,
            phi0,
            theta0,
            x0: 0.0,
            y0: 0.0,
        };
        // GLS: the reference point is on the equator at the reference latitude (wcslib's
        // translation to SFL).
        if code == P::Gls && p.delta0 != 0.0 && fiducial.0.is_none() && fiducial.1.is_none() {
            prj.theta0 = p.delta0;
            let (x0, y0) = prj.forward(0.0, p.delta0).ok_or_else(|| bad(code))?;
            prj.x0 = x0;
            prj.y0 = y0;
            return Ok(prj);
        }
        if let Some(f) = fiducial.0 {
            phi0 = f;
        }
        if let Some(t) = fiducial.1 {
            theta0 = t;
        }
        if !(-90.0..=90.0).contains(&theta0) {
            return Err(bad(code));
        }
        prj.phi0 = phi0;
        prj.theta0 = theta0;
        // The offset applies only when PVi_0 is true and both φ₀ and θ₀ are given (Paper II
        // §2.5; as wcslib).
        if fiducial.2 && fiducial.0.is_some() && fiducial.1.is_some() {
            let (x0, y0) = prj.forward(phi0, theta0).ok_or_else(|| bad(code))?;
            prj.x0 = x0;
            prj.y0 = y0;
        }
        Ok(prj)
    }

    /// Projection plane (degrees) → native (φ, θ), `None` outside the projection's domain.
    pub fn deproject(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let (phi, theta) = self.inverse(x + self.x0, y + self.y0)?;
        // Bounds, with wcslib's tolerance for rounding.
        let tol = 1e-13;
        if !(phi.is_finite() && theta.is_finite())
            || phi.abs() > 180.0 + tol
            || theta.abs() > 90.0 + tol
        {
            return None;
        }
        Some((phi.clamp(-180.0, 180.0), theta.clamp(-90.0, 90.0)))
    }

    /// Native (φ, θ) → projection plane (degrees), `None` where undefined or on a side the
    /// projection does not show (checked by going back: points that do not deproject to
    /// themselves are hidden or ambiguous).
    pub fn project(&self, phi: f64, theta: f64) -> Option<(f64, f64)> {
        let phi = wrap180(phi);
        let (x, y) = self.forward(phi, theta)?;
        let (p2, t2) = self.inverse(x, y)?;
        // CSC's inverse is an approximation of its forward, good to about 40″ (0.011°).
        let tol = if self.code == Projection::Csc {
            0.05
        } else {
            1e-6
        };
        let ok = (t2 - theta).abs() < tol
            && ((90.0 - theta.abs()) < 1e-9
                || wrap180(p2 - phi).abs() * theta.to_radians().cos() < tol);
        ok.then_some((x - self.x0, y - self.y0))
    }

    /// The projection equations (no offset).
    fn forward(&self, phi: f64, theta: f64) -> Option<(f64, f64)> {
        let (sp, cp) = (phi * D2R).sin_cos();
        let (st, ct) = (theta * D2R).sin_cos();
        // Zenithal: x = R sin φ, y = −R cos φ.
        let zen = |r: f64| Some((r * sp, -r * cp));
        match &self.kind {
            Kind::Azp { mu, cg, tg, .. } => {
                let d = mu + st + ct * cp * tg;
                if d <= 0.0 {
                    return None;
                }
                let r = R0 * (mu + 1.0) * ct / d;
                Some((r * sp, -r * cp / cg))
            }
            Kind::Szp { xp, yp, zp } => {
                let s = 1.0 - st;
                let d = zp - s;
                if d == 0.0 {
                    return None;
                }
                Some((
                    R0 * (zp * ct * sp - xp * s) / d,
                    -R0 * (zp * ct * cp + yp * s) / d,
                ))
            }
            Kind::Tan => {
                if st <= 0.0 {
                    return None;
                }
                zen(R0 * ct / st)
            }
            Kind::Stg => {
                if 1.0 + st <= 0.0 {
                    return None;
                }
                zen(2.0 * R0 * ct / (1.0 + st))
            }
            Kind::Sin { xi, eta } => {
                let s = 1.0 - st;
                Some((R0 * (ct * sp + xi * s), -R0 * (ct * cp - eta * s)))
            }
            Kind::Arc => zen(90.0 - theta),
            Kind::Zpn { c, zmax, .. } => {
                let z = (90.0 - theta) * D2R;
                if z > zmax + 1e-13 {
                    return None;
                }
                zen(R0 * poly(c, z))
            }
            Kind::Zea => zen(2.0 * R0 * ((90.0 - theta) * D2R / 2.0).sin()),
            Kind::Air { cb } => {
                let xi = (90.0 - theta) * D2R / 2.0;
                if xi >= std::f64::consts::FRAC_PI_2 {
                    return None;
                }
                zen(2.0 * R0 * air(xi, *cb))
            }
            Kind::Cyp { mu, lambda } => {
                let d = mu + ct;
                if d == 0.0 {
                    return None;
                }
                Some((lambda * phi, R0 * (mu + lambda) * st / d))
            }
            Kind::Cea { lambda } => Some((phi, R0 * st / lambda)),
            Kind::Car => Some((phi, theta)),
            Kind::Mer => {
                if theta.abs() >= 90.0 {
                    return None;
                }
                Some((phi, R0 * ((90.0 + theta) * D2R / 2.0).tan().ln()))
            }
            Kind::Sfl => Some((phi * ct, theta)),
            Kind::Par => {
                let s = (theta / 3.0 * D2R).sin();
                Some((phi * (1.0 - 4.0 * s * s), 180.0 * s))
            }
            Kind::Mol => {
                let g = mollweide_gamma(theta * D2R);
                Some((
                    2.0 * SQRT2 / std::f64::consts::PI * phi * g.cos(),
                    SQRT2 * R0 * g.sin(),
                ))
            }
            Kind::Ait => {
                let (sh, ch) = (phi * D2R / 2.0).sin_cos();
                let g = R0 * (2.0 / (1.0 + ct * ch)).sqrt();
                Some((2.0 * g * ct * sh, g * st))
            }
            Kind::Conic(k) => {
                let r = k.radius(theta)?;
                let (sa, ca) = (k.c * phi * D2R).sin_cos();
                Some((r * sa, -r * ca + k.y0))
            }
            Kind::Bon { y0 } => {
                let r = y0 - theta;
                let a = if r == 0.0 { 0.0 } else { phi * ct / r };
                let (sa, ca) = a.sin_cos();
                Some((r * sa, -r * ca + y0))
            }
            Kind::Pco => {
                if theta == 0.0 {
                    return Some((phi, 0.0));
                }
                let e = phi * D2R * st;
                let cot = ct / st;
                let h = (e / 2.0).sin();
                Some((R0 * cot * e.sin(), theta + R0 * cot * 2.0 * h * h))
            }
            Kind::Cube(c) => Some(cube_forward(*c, phi, theta)),
            Kind::Hpx { h, k } => Some(hpx_forward(*h, *k, phi, theta)),
            Kind::Xph => Some(xph_forward(phi, theta)),
        }
    }

    /// The inverse equations (no offset), unchecked bounds.
    fn inverse(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let r = x.hypot(y);
        let az = || if r == 0.0 { 0.0 } else { x.atan2(-y) * R2D };
        match &self.kind {
            Kind::Azp { mu, cg, sg, .. } => {
                let yc = y * cg;
                let rr = x.hypot(yc);
                if rr == 0.0 {
                    return Some((0.0, 90.0));
                }
                let phi = x.atan2(-yc) * R2D;
                let d = R0 * (mu + 1.0) + y * sg;
                if d == 0.0 {
                    return None;
                }
                let rho = rr / d;
                let s = rho * mu / (rho * rho + 1.0).sqrt();
                if s.abs() > 1.0 + 1e-13 {
                    return None;
                }
                let psi = 1f64.atan2(rho) * R2D;
                let w = s.clamp(-1.0, 1.0).asin() * R2D;
                let mut a = psi - w;
                let mut b = psi + w + 180.0;
                if a > 90.0 {
                    a -= 360.0;
                }
                if b > 90.0 {
                    b -= 360.0;
                }
                Some((phi, a.max(b)))
            }
            Kind::Szp { xp, yp, zp } => {
                let (xx, yy) = (x / R0, y / R0);
                let x1 = (xx - xp) / zp;
                let y1 = (yy - yp) / zp;
                slant(xx, yy, x1, y1)
            }
            Kind::Tan => Some((az(), R0.atan2(r) * R2D)),
            Kind::Stg => Some((az(), 90.0 - 2.0 * (r / (2.0 * R0)).atan() * R2D)),
            Kind::Sin { xi, eta } => {
                if *xi == 0.0 && *eta == 0.0 {
                    let s = r / R0;
                    if s > 1.0 + 1e-13 {
                        return None;
                    }
                    return Some((az(), s.min(1.0).acos() * R2D));
                }
                slant(x / R0, y / R0, *xi, *eta)
            }
            Kind::Arc => Some((az(), 90.0 - r)),
            Kind::Zpn { c, zmax, rmax } => {
                let rr = r / R0;
                let z = if rr <= c[0] {
                    if rr < c[0] - 1e-13 {
                        return None;
                    }
                    0.0
                } else if rr >= *rmax {
                    if rr > rmax + 1e-13 {
                        return None;
                    }
                    *zmax
                } else {
                    bisect(0.0, *zmax, |z| poly(c, z) - rr)
                };
                Some((az(), 90.0 - z * R2D))
            }
            Kind::Zea => {
                let s = r / (2.0 * R0);
                if s > 1.0 + 1e-13 {
                    return None;
                }
                Some((az(), 90.0 - 2.0 * s.min(1.0).asin() * R2D))
            }
            Kind::Air { cb } => {
                let rr = r / (2.0 * R0);
                let xi = if rr == 0.0 {
                    0.0
                } else {
                    // Bracket ξ in [0, π/2): air() rises from 0 to ∞.
                    let mut hi = 1.0;
                    while air(hi, *cb) < rr {
                        hi = (hi + std::f64::consts::FRAC_PI_2) / 2.0;
                        if std::f64::consts::FRAC_PI_2 - hi < 1e-15 {
                            return None;
                        }
                    }
                    bisect(0.0, hi, |xi| air(xi, *cb) - rr)
                };
                Some((az(), 90.0 - 2.0 * xi * R2D))
            }
            Kind::Cyp { mu, lambda } => {
                let eta = y / (R0 * (mu + lambda));
                let s = eta * mu / (eta * eta + 1.0).sqrt();
                if s.abs() > 1.0 + 1e-13 {
                    return None;
                }
                Some((
                    x / lambda,
                    eta.atan2(1.0) * R2D + s.clamp(-1.0, 1.0).asin() * R2D,
                ))
            }
            Kind::Cea { lambda } => {
                let s = lambda * y / R0;
                if s.abs() > 1.0 + 1e-13 {
                    return None;
                }
                Some((x, s.clamp(-1.0, 1.0).asin() * R2D))
            }
            Kind::Car => Some((x, y)),
            Kind::Mer => Some((x, 2.0 * (y * D2R).exp().atan() * R2D - 90.0)),
            Kind::Sfl => {
                let c = (y * D2R).cos();
                let phi = if c == 0.0 { 0.0 } else { x / c };
                Some((phi, y))
            }
            Kind::Par => {
                let s = y / 180.0;
                if s.abs() > 1.0 + 1e-13 {
                    return None;
                }
                let s = s.clamp(-1.0, 1.0);
                let d = 1.0 - 4.0 * s * s;
                let phi = if d == 0.0 {
                    if x.abs() > 1e-13 {
                        return None;
                    }
                    0.0
                } else {
                    x / d
                };
                Some((phi, 3.0 * s.asin() * R2D))
            }
            Kind::Mol => {
                let s = y / (SQRT2 * R0);
                if s.abs() > 1.0 + 1e-13 {
                    return None;
                }
                let s = s.clamp(-1.0, 1.0);
                let g = s.asin();
                let c = g.cos();
                let z = (2.0 * g + (2.0 * g).sin()) / std::f64::consts::PI;
                if z.abs() > 1.0 + 1e-13 {
                    return None;
                }
                let phi = if c < 1e-15 {
                    if x.abs() > 1e-13 {
                        return None;
                    }
                    0.0
                } else {
                    std::f64::consts::PI * x / (2.0 * SQRT2 * c)
                };
                Some((phi, z.clamp(-1.0, 1.0).asin() * R2D))
            }
            Kind::Ait => {
                let u = x * D2R / 4.0;
                let v = y * D2R / 2.0;
                let z2 = 1.0 - u * u - v * v;
                if z2 < 0.5 - 1e-13 {
                    return None;
                }
                let z2 = z2.max(0.5);
                let z = z2.sqrt();
                let phi = 2.0 * (z * x * D2R / 2.0).atan2(2.0 * z2 - 1.0) * R2D;
                let s = y * z * D2R;
                (s.abs() <= 1.0 + 1e-13).then(|| (phi, s.clamp(-1.0, 1.0).asin() * R2D))
            }
            Kind::Conic(k) => {
                let dy = k.y0 - y;
                let mut r = x.hypot(dy);
                if k.ta < 0.0 {
                    r = -r;
                }
                let a = if r == 0.0 {
                    0.0
                } else {
                    (x / r).atan2(dy / r) * R2D
                };
                Some((a / k.c, k.latitude(r)?))
            }
            Kind::Bon { y0 } => {
                let dy = y0 - y;
                let mut r = x.hypot(dy);
                if *y0 < 0.0 {
                    r = -r;
                }
                let theta = y0 - r;
                let ct = (theta * D2R).cos();
                let phi = if r == 0.0 || ct == 0.0 {
                    0.0
                } else {
                    (x / r).atan2(dy / r) * r / ct
                };
                Some((phi, theta))
            }
            Kind::Pco => pco_inverse(x, y),
            Kind::Cube(c) => cube_inverse(*c, x, y),
            Kind::Hpx { h, k } => hpx_inverse(*h, *k, x, y),
            Kind::Xph => xph_inverse(x, y),
        }
    }
}

/// Root of the increasing `f` on `[lo, hi]` (f(lo) ≤ 0 ≤ f(hi)) to machine precision.
fn bisect(mut lo: f64, mut hi: f64, f: impl Fn(f64) -> f64) -> f64 {
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if mid <= lo || mid >= hi {
            break;
        }
        if f(mid) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

fn poly(c: &[f64], z: f64) -> f64 {
    c.iter().rev().fold(0.0, |acc, &k| acc * z + k)
}

fn dpoly(c: &[f64], z: f64) -> f64 {
    c.iter()
        .enumerate()
        .skip(1)
        .rev()
        .fold(0.0, |acc, (m, &k)| acc * z + m as f64 * k)
}

/// ZPN: the polynomial and its first turning point.
fn zpn(code: Projection, all: &[f64]) -> Result<Kind, Unsupported> {
    let n = all
        .iter()
        .rposition(|&c| c != 0.0)
        .ok_or_else(|| bad(code))?;
    if n == 0 {
        return Err(bad(code));
    }
    let c = all[..=n].to_vec();
    let pi = std::f64::consts::PI;
    if dpoly(&c, 0.0) <= 0.0 {
        return Err(bad(code));
    }
    // The first ζ in (0, π] where the derivative stops being positive.
    let mut zmax = pi;
    let step = pi / 360.0;
    let mut z0 = 0.0;
    for j in 1..=360 {
        let z = j as f64 * step;
        if dpoly(&c, z) <= 0.0 {
            zmax = bisect(z0, z, |t| -dpoly(&c, t));
            break;
        }
        z0 = z;
    }
    let rmax = poly(&c, zmax);
    Ok(Kind::Zpn { c, zmax, rmax })
}

/// AIR: R(ξ) / 2R₀.
fn air(xi: f64, cb: f64) -> f64 {
    if xi < 1e-8 {
        // ln(cos ξ)/tan ξ → −ξ/2.
        return -(-xi / 2.0 + cb * xi);
    }
    let t = xi.tan();
    -(xi.cos().ln() / t + cb * t)
}

/// SIN with ξ, η and SZP: the root of (X − sX₁)² + (Y − sY₁)² = 2s − s², s = 1 − sin θ, nearer
/// the pole.
fn slant(x: f64, y: f64, x1: f64, y1: f64) -> Option<(f64, f64)> {
    let a = 1.0 + x1 * x1 + y1 * y1;
    let b = 1.0 + x * x1 + y * y1;
    let c = x * x + y * y;
    let d = b * b - a * c;
    if d < -1e-13 {
        return None;
    }
    let sq = d.max(0.0).sqrt();
    let s_near = if b + sq == 0.0 { 0.0 } else { c / (b + sq) };
    let s_far = (b + sq) / a;
    let s = if (-1e-13..=2.0 + 1e-13).contains(&s_near) {
        s_near
    } else if (-1e-13..=2.0 + 1e-13).contains(&s_far) {
        s_far
    } else {
        return None;
    };
    let s = s.clamp(0.0, 2.0);
    let theta = (1.0 - s).clamp(-1.0, 1.0).asin() * R2D;
    let (u, v) = (x - x1 * s, -(y - y1 * s));
    let phi = if u == 0.0 && v == 0.0 {
        0.0
    } else {
        u.atan2(v) * R2D
    };
    Some((phi, theta))
}

/// Mollweide's auxiliary angle: 2γ + sin 2γ = π sin θ.
fn mollweide_gamma(theta: f64) -> f64 {
    let pi = std::f64::consts::PI;
    let target = pi * theta.sin();
    if (theta.abs() - std::f64::consts::FRAC_PI_2).abs() < 1e-15 {
        return theta;
    }
    let f = |g: f64| 2.0 * g + (2.0 * g).sin() - target;
    let half = std::f64::consts::FRAC_PI_2;
    // f is increasing on [−π/2, π/2].
    bisect(-half, half, f)
}

impl Conic {
    fn radius(&self, theta: f64) -> Option<f64> {
        match &self.kind {
            ConicKind::Cop { k, cot } => {
                let a = (theta - self.ta) * D2R;
                if a.cos() <= 0.0 {
                    return None;
                }
                Some(k * (cot - a.tan()))
            }
            ConicKind::Coe { gamma, s } => {
                let q = s - gamma * (theta * D2R).sin();
                if q < 0.0 {
                    return None;
                }
                Some(R0 / self.c * q.sqrt())
            }
            ConicKind::Cod { t } => Some(self.ta - theta + t),
            ConicKind::Coo { psi } => {
                if theta == -90.0 * self.c.signum() {
                    return None;
                }
                let t = ((90.0 - theta) * D2R / 2.0).tan();
                if t == 0.0 {
                    return Some(0.0);
                }
                Some(psi * t.powf(self.c))
            }
        }
    }

    fn latitude(&self, r: f64) -> Option<f64> {
        match &self.kind {
            ConicKind::Cop { k, cot } => Some(self.ta + (cot - r / k).atan() * R2D),
            ConicKind::Coe { gamma, s } => {
                let w = r / (2.0 * R0);
                let v = s / gamma - gamma * w * w;
                if v.abs() > 1.0 + 1e-13 {
                    return None;
                }
                Some(v.clamp(-1.0, 1.0).asin() * R2D)
            }
            ConicKind::Cod { t } => Some(self.ta + t - r),
            ConicKind::Coo { psi } => {
                if r == 0.0 {
                    return Some(if self.c < 0.0 { -90.0 } else { 90.0 });
                }
                let q = r / psi;
                if q <= 0.0 {
                    return None;
                }
                Some(90.0 - 2.0 * q.powf(1.0 / self.c).atan() * R2D)
            }
        }
    }
}

fn conic(code: Projection, ta: f64, eta: f64) -> Result<Conic, Unsupported> {
    let (t1, t2) = (ta - eta, ta + eta);
    if ta == 0.0 || ta.abs() > 90.0 || t1.abs() > 90.0 || t2.abs() > 90.0 {
        return Err(bad(code));
    }
    let tan_ta = (ta * D2R).tan();
    let (kind, c) = match code {
        Projection::Cop => {
            let k = R0 * (eta * D2R).cos();
            if k == 0.0 {
                return Err(bad(code));
            }
            (
                ConicKind::Cop {
                    k,
                    cot: 1.0 / tan_ta,
                },
                (ta * D2R).sin(),
            )
        }
        Projection::Coe => {
            let (s1, s2) = ((t1 * D2R).sin(), (t2 * D2R).sin());
            let gamma = s1 + s2;
            if gamma == 0.0 {
                return Err(bad(code));
            }
            (
                ConicKind::Coe {
                    gamma,
                    s: 1.0 + s1 * s2,
                },
                gamma / 2.0,
            )
        }
        Projection::Cod => {
            let e = eta * D2R;
            let (c, t) = if eta == 0.0 {
                ((ta * D2R).sin(), R0 / tan_ta)
            } else {
                ((ta * D2R).sin() * e.sin() / e, R0 * e / e.tan() / tan_ta)
            };
            (ConicKind::Cod { t }, c)
        }
        Projection::Coo => {
            let tan_half = |t: f64| ((90.0 - t) * D2R / 2.0).tan();
            let (c1, c2) = ((t1 * D2R).cos(), (t2 * D2R).cos());
            let c = if t1 == t2 {
                (t1 * D2R).sin()
            } else {
                (c2 / c1).ln() / (tan_half(t2) / tan_half(t1)).ln()
            };
            if c == 0.0 || !c.is_finite() {
                return Err(bad(code));
            }
            let psi = R0 * c1 / (c * tan_half(t1).powf(c));
            if !psi.is_finite() || psi == 0.0 {
                return Err(bad(code));
            }
            (ConicKind::Coo { psi }, c)
        }
        _ => unreachable!(),
    };
    let mut k = Conic {
        kind,
        ta,
        c,
        y0: 0.0,
    };
    k.y0 = k.radius(ta).ok_or_else(|| bad(code))?;
    Ok(k)
}

/// PCO's inverse: θ from X² + (Y − θ)² − 2(Y − θ) cot θ = 0 on (0, Y] (|Y| mirrored).
fn pco_inverse(x: f64, y: f64) -> Option<(f64, f64)> {
    let (xx, yy) = (x * D2R, y * D2R);
    if yy == 0.0 {
        return Some((x, 0.0));
    }
    let ya = yy.abs();
    let half = std::f64::consts::FRAC_PI_2;
    let hi = ya.min(half);
    let f = |t: f64| {
        let d = ya - t;
        xx * xx + d * d - 2.0 * d * t.cos() / t.sin()
    };
    if f(hi) < 0.0 {
        return None;
    }
    let t = bisect(0.0, hi, f);
    let tn = t.tan();
    let e = (xx * tn).atan2(1.0 - (ya - t) * tn);
    let st = t.sin();
    let phi = if st == 0.0 { xx } else { e / st };
    let theta = t * R2D * yy.signum();
    Some((phi * R2D, theta))
}

// Quad-cube faces: centres in the plane, and (ξ, η, ζ) ↔ direction cosines (l, m, n).
const FACE_CENTRE: [(f64, f64); 6] = [
    (0.0, 90.0),
    (0.0, 0.0),
    (90.0, 0.0),
    (180.0, 0.0),
    (270.0, 0.0),
    (0.0, -90.0),
];

fn face_of(l: f64, m: f64, n: f64) -> (usize, f64, f64, f64) {
    // (face, ξ, η, ζ)
    let cands = [
        (0, m, -l, n),
        (1, m, n, l),
        (2, -l, n, m),
        (3, -m, n, -l),
        (4, l, n, -m),
        (5, m, l, -n),
    ];
    let mut best = cands[0];
    for c in &cands[1..] {
        if c.3 > best.3 {
            best = *c;
        }
    }
    best
}

fn face_dir(face: usize, xi: f64, eta: f64, zeta: f64) -> (f64, f64, f64) {
    match face {
        0 => (-eta, xi, zeta),
        1 => (zeta, xi, eta),
        2 => (-xi, zeta, eta),
        3 => (-zeta, -xi, eta),
        4 => (xi, -zeta, eta),
        _ => (eta, xi, -zeta),
    }
}

/// CSC forward on a face (Chan & O'Neill 1975, Paper II eq. 182–184). Evaluated in double
/// precision (the COBE software and wcslib use single).
fn csc_f(chi: f64, psi: f64) -> f64 {
    let gstar = 1.374_848_477_32;
    let mm = 0.004_869_491_981;
    let gamma = -0.131_616_714_74;
    let omega1 = -0.159_596_235_474;
    let c00 = 0.141_189_631_152;
    let c10 = 0.080_970_128_652_5;
    let c01 = -0.281_528_535_557;
    let c11 = 0.153_841_128_76;
    let c20 = -0.178_251_207_466;
    let c02 = 0.106_959_469_314;
    let d0 = 0.075_919_620_046_7;
    let d1 = -0.021_776_249_069_9;
    let x2 = chi * chi;
    let y2 = psi * psi;
    let csum = c00 + c10 * x2 + c01 * y2 + c20 * x2 * x2 + c11 * x2 * y2 + c02 * y2 * y2;
    let dsum = d0 + d1 * x2;
    chi * gstar
        + chi * x2 * (1.0 - gstar)
        + chi * y2 * (1.0 - x2) * (gamma + (mm - gamma) * x2 + (1.0 - y2) * csum)
        + chi * x2 * (1.0 - x2) * (omega1 - (1.0 - x2) * dsum)
}

/// CSC inverse on a face (Paper II eq. 185–186), an approximation of the forward's inverse.
fn csc_g(x: f64, y: f64) -> f64 {
    // P[j][i]: coefficient of x^{2i} y^{2j}.
    const P: [&[f64]; 7] = [
        &[
            -0.272_926_96,
            -0.076_299_69,
            -0.227_970_56,
            0.548_523_84,
            -0.629_300_65,
            0.257_957_94,
            0.025_843_75,
        ],
        &[
            -0.028_194_52,
            -0.014_715_65,
            0.480_515_09,
            -1.741_144_54,
            1.715_475_08,
            -0.530_223_37,
        ],
        &[
            0.270_581_60,
            -0.568_009_38,
            0.308_033_17,
            0.989_381_02,
            -0.831_804_69,
        ],
        &[-0.604_415_60, 1.508_800_86, -0.936_785_76, 0.086_938_41],
        &[0.934_120_77, -1.416_019_20, 0.338_874_46],
        &[-0.639_153_06, 0.520_322_38],
        &[0.143_815_85],
    ];
    let x2 = x * x;
    let y2 = y * y;
    let mut sum = 0.0;
    let mut yj = 1.0;
    for row in P {
        let mut xi = 1.0;
        for &c in row {
            sum += c * xi * yj;
            xi *= x2;
        }
        yj *= y2;
    }
    x + x * (1.0 - x2) * sum
}

fn cube_forward(c: Cube, phi: f64, theta: f64) -> (f64, f64) {
    let (sp, cp) = (phi * D2R).sin_cos();
    let (st, ct) = (theta * D2R).sin_cos();
    let (l, m, n) = (ct * cp, ct * sp, st);
    let (face, xi, eta, zeta) = face_of(l, m, n);
    let (u, v) = match c {
        Cube::Tsc => (xi / zeta, eta / zeta),
        Cube::Csc => {
            let (chi, psi) = (xi / zeta, eta / zeta);
            (csc_f(chi, psi), csc_f(psi, chi))
        }
        Cube::Qsc => qsc_forward(xi, eta, zeta),
    };
    let (xc, yc) = FACE_CENTRE[face];
    (xc + 45.0 * u, yc + 45.0 * v)
}

/// QSC on a face: (ξ, η, ζ) → face coordinates in [−1, 1] (Paper II eq. 189–194).
fn qsc_forward(xi: f64, eta: f64, zeta: f64) -> (f64, f64) {
    if xi == 0.0 && eta == 0.0 {
        return (0.0, 0.0);
    }
    // 1 − ζ without cancellation.
    let one_minus = (xi * xi + eta * eta) / (1.0 + zeta);
    let along = |a: f64, b: f64| -> (f64, f64) {
        // a dominant: ω = b / a.
        let w = b / a;
        let s = (one_minus / (1.0 - 1.0 / (2.0 + w * w).sqrt())).sqrt() * a.signum();
        let t = s / 15.0 * ((w.atan() - (w / (2.0 * (1.0 + w * w)).sqrt()).asin()) * R2D);
        (s, t)
    };
    if xi.abs() > eta.abs() {
        along(xi, eta)
    } else {
        let (v, u) = along(eta, xi);
        (u, v)
    }
}

/// QSC inverse on a face: face coordinates → (ξ, η, ζ).
fn qsc_inverse(u: f64, v: f64) -> (f64, f64, f64) {
    if u == 0.0 && v == 0.0 {
        return (0.0, 0.0, 1.0);
    }
    let back = |a: f64, b: f64| -> (f64, f64, f64) {
        // a dominant: returns (along a, along b, ζ).
        let t = 15.0 * b / a * D2R;
        let w = t.sin() / (t.cos() - std::f64::consts::FRAC_1_SQRT_2);
        let om = a * a * (1.0 - 1.0 / (2.0 + w * w).sqrt()); // 1 − ζ
        let zeta = 1.0 - om;
        let q = (om * (2.0 - om)).max(0.0); // 1 − ζ²
        let pa = (q / (1.0 + w * w)).sqrt() * a.signum();
        (pa, w * pa, zeta)
    };
    if u.abs() > v.abs() {
        back(u, v)
    } else {
        let (e, x, z) = back(v, u);
        (x, e, z)
    }
}

fn cube_inverse(c: Cube, x: f64, y: f64) -> Option<(f64, f64)> {
    let tol = 1e-12;
    let mut xf = x / 45.0;
    let mut yf = y / 45.0;
    if xf.abs() <= 1.0 + tol {
        if yf.abs() > 3.0 + tol {
            return None;
        }
    } else if xf.abs() > 7.0 + tol || yf.abs() > 1.0 + tol {
        return None;
    }
    if xf < -1.0 - tol {
        xf += 8.0;
    }
    let face = if xf > 5.0 + tol {
        xf -= 6.0;
        4
    } else if xf > 3.0 + tol {
        xf -= 4.0;
        3
    } else if xf > 1.0 + tol {
        xf -= 2.0;
        2
    } else if yf > 1.0 + tol {
        yf -= 2.0;
        0
    } else if yf < -1.0 - tol {
        yf += 2.0;
        5
    } else {
        1
    };
    let (u, v) = (xf.clamp(-1.0, 1.0), yf.clamp(-1.0, 1.0));
    let (xi, eta, zeta) = match c {
        Cube::Tsc | Cube::Csc => {
            let (chi, psi) = if c == Cube::Tsc {
                (u, v)
            } else {
                (csc_g(u, v), csc_g(v, u))
            };
            let z = 1.0 / (1.0 + chi * chi + psi * psi).sqrt();
            (chi * z, psi * z, z)
        }
        Cube::Qsc => qsc_inverse(u, v),
    };
    let (l, m, n) = face_dir(face, xi, eta, zeta);
    let phi = if l == 0.0 && m == 0.0 {
        0.0
    } else {
        m.atan2(l) * R2D
    };
    Some((phi, n.clamp(-1.0, 1.0).asin() * R2D))
}

/// HPX with H facets around and K facets from pole to pole.
fn hpx_forward(h: f64, k: f64, phi: f64, theta: f64) -> (f64, f64) {
    let s = (theta * D2R).sin();
    if s.abs() <= (k - 1.0) / k {
        return (phi, 90.0 * k / h * s);
    }
    let sigma = (k * (1.0 - s.abs())).sqrt();
    let pc = hpx_centre(h, k, phi, theta >= 0.0);
    (
        pc + (phi - pc) * sigma,
        theta.signum() * 180.0 / h * ((k + 1.0) / 2.0 - sigma),
    )
}

/// The central longitude of the polar facet containing `phi`.
fn hpx_centre(h: f64, k: f64, phi: f64, north: bool) -> f64 {
    let w = 360.0 / h;
    let odd = (k as i64) % 2 == 1;
    if odd || north {
        let i = ((phi + 180.0) / w).floor().clamp(0.0, h - 1.0);
        -180.0 + (i + 0.5) * w
    } else {
        // Even K: the southern facets are offset by half a facet.
        let i = ((phi + 180.0) / w).round().clamp(0.0, h);
        -180.0 + i * w
    }
}

fn hpx_inverse(h: f64, k: f64, x: f64, y: f64) -> Option<(f64, f64)> {
    let tol = 1e-12;
    let ylim = 90.0 * (k - 1.0) / h;
    if y.abs() <= ylim {
        if x.abs() > 180.0 + tol {
            return None;
        }
        let s = y * h / (90.0 * k);
        return Some((x, s.clamp(-1.0, 1.0).asin() * R2D));
    }
    let sigma = (k + 1.0) / 2.0 - y.abs() * h / 180.0;
    if sigma < -tol {
        return None;
    }
    let sigma = sigma.max(0.0);
    let pc = hpx_centre(h, k, x.clamp(-180.0, 180.0 - 1e-9), y > 0.0);
    let half = 180.0 / h;
    if (x - pc).abs() > sigma * half + tol {
        return None;
    }
    let phi = if sigma == 0.0 {
        pc
    } else {
        pc + (x - pc) / sigma
    };
    let theta = y.signum() * (1.0 - sigma * sigma / k).clamp(-1.0, 1.0).asin() * R2D;
    Some((phi, theta))
}

/// XPH: HPX (H = 4, K = 3) with each 90° column turned to radiate from the north pole.
fn xph_forward(phi: f64, theta: f64) -> (f64, f64) {
    let q = (((phi + 180.0) / 90.0).floor() as i64).clamp(0, 3);
    let pc = -135.0 + 90.0 * q as f64;
    let (xh, yh) = hpx_forward(4.0, 3.0, phi, theta);
    let xi = xh - pc;
    let eta = 90.0 - yh;
    let (x, y) = ((eta + xi) / SQRT2, (xi - eta) / SQRT2);
    // Column q = 2 (0° ≤ φ < 90°) is unrotated; the others turn by 90° steps.
    rot(x, y, (q + 2) % 4)
}

fn rot(x: f64, y: f64, k: i64) -> (f64, f64) {
    match k.rem_euclid(4) {
        0 => (x, y),
        1 => (-y, x),
        2 => (-x, -y),
        _ => (y, -x),
    }
}

fn xph_inverse(x: f64, y: f64) -> Option<(f64, f64)> {
    if x == 0.0 && y == 0.0 {
        return Some((0.0, 90.0));
    }
    // The plane quadrant gives the column: x ≥ 0, y ≤ 0 is 0° ≤ φ < 90°.
    let k = if x >= 0.0 && y < 0.0 {
        0
    } else if x > 0.0 && y >= 0.0 {
        1
    } else if x <= 0.0 && y > 0.0 {
        2
    } else {
        3
    };
    let (u, v) = rot(x, y, -k);
    let xi = (u + v) / SQRT2;
    let eta = (u - v) / SQRT2;
    let q = (k + 2) % 4;
    let pc = -135.0 + 90.0 * q as f64;
    let (xh, yh) = (pc + xi, 90.0 - eta);
    let (phi, theta) = hpx_inverse(4.0, 3.0, xh, yh)?;
    // Each column keeps its own longitudes.
    if (phi - pc).abs() > 45.0 + 1e-9 {
        return None;
    }
    Some((phi, theta))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prj(code: &str, pv: &[(usize, f64)]) -> Prj {
        let pv = pv.to_vec();
        let f = move |m: usize| pv.iter().find(|p| p.0 == m).map(|p| p.1);
        Prj::new(
            Projection::from_code(code).unwrap(),
            &Params {
                pv: &f,
                delta0: 30.0,
            },
            (None, None, false),
        )
        .unwrap()
    }

    /// Every projection goes there and back over a grid of the sphere it shows.
    #[test]
    fn round_trips() {
        let cases: &[(&str, &[(usize, f64)])] = &[
            ("AZP", &[(1, 2.0), (2, 30.0)]),
            ("SZP", &[(1, 2.0), (2, 180.0), (3, 60.0)]),
            ("TAN", &[]),
            ("STG", &[]),
            ("SIN", &[]),
            ("SIN", &[(1, 0.1), (2, -0.2)]),
            ("NCP", &[]),
            ("ARC", &[]),
            (
                "ZPN",
                &[(0, 0.05), (1, 0.975), (2, -0.807), (3, 0.337), (4, -0.065)],
            ),
            ("ZEA", &[]),
            ("AIR", &[(1, 45.0)]),
            ("AIR", &[]),
            ("CYP", &[(1, 1.0), (2, 0.7)]),
            ("CEA", &[(1, 0.75)]),
            ("CAR", &[]),
            ("MER", &[]),
            ("SFL", &[]),
            ("PAR", &[]),
            ("MOL", &[]),
            ("AIT", &[]),
            ("COP", &[(1, 45.0), (2, 25.0)]),
            ("COE", &[(1, -45.0), (2, 25.0)]),
            ("COD", &[(1, 45.0), (2, 25.0)]),
            ("COO", &[(1, 45.0), (2, 25.0)]),
            ("BON", &[(1, 45.0)]),
            ("PCO", &[]),
            ("TSC", &[]),
            ("QSC", &[]),
            ("HPX", &[]),
            ("HPX", &[(1, 6.0), (2, 4.0)]),
            ("XPH", &[]),
        ];
        for (code, pv) in cases {
            let p = prj(code, pv);
            let mut n = 0;
            for i in -17..=17 {
                for j in -8..=8 {
                    let (phi, theta) = (i as f64 * 10.0 + 0.3, j as f64 * 10.0 + 0.7);
                    let Some((x, y)) = p.project(phi, theta) else {
                        continue;
                    };
                    let (p2, t2) = p
                        .deproject(x, y)
                        .unwrap_or_else(|| panic!("{code} {phi} {theta} → {x} {y}"));
                    assert!(
                        (t2 - theta).abs() < 1e-9 && wrap180(p2 - phi).abs() < 1e-9,
                        "{code} {pv:?}: ({phi}, {theta}) → ({x}, {y}) → ({p2}, {t2})"
                    );
                    n += 1;
                }
            }
            assert!(n > 100, "{code}: {n} points");
        }
    }
}
