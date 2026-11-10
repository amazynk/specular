//! Non-linear spectral axes of FITS WCS Paper III (Greisen et al. 2006, §3 and Table 3):
//! `CTYPE` `XXXX-P2X`, an axis of type `XXXX` sampled linearly in another basic type.
//!
//! The four basic types are frequency (F), vacuum wavelength (W), air wavelength (A) and
//! apparent radial velocity (V). Each axis type belongs to one of them: `FREQ`, `ENER`,
//! `WAVN` and `VRAD` to F; `WAVE`, `VOPT` and `ZOPT` to W; `AWAV` to A; `VELO` and `BETA`
//! to V. The code's second letter is the axis type's own basic type, the first the one
//! the axis is linear in. `CRVAL` and `CDELT` are given in the axis type at the reference
//! pixel; the linear type's value there and its step (from the derivative at the
//! reference) give every other pixel, which is then turned back into the axis type.
//!
//! Air wavelengths use Paper III's refractive index (Cox 2000, eq. 65, vacuum wavelength
//! in µm); the air → vacuum inverse is iterated. Velocities need the rest frequency
//! (`RESTFRQ`, or the older `RESTFREQ`) or rest wavelength (`RESTWAV`). Units are SI
//! unless `CUNIT` names a scaled one. `-TAB` and the grism codes (`GRI`, `GRA`) are not
//! evaluated.

/// Speed of light (m/s) and the Planck constant (J s), exact in SI.
const C: f64 = 299_792_458.0;
const H: f64 = 6.626_070_15e-34;

/// A basic spectral type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Basic {
    F,
    W,
    A,
    V,
}

impl Basic {
    fn of_letter(c: u8) -> Option<Basic> {
        Some(match c {
            b'F' => Basic::F,
            b'W' => Basic::W,
            b'A' => Basic::A,
            b'V' => Basic::V,
            _ => return None,
        })
    }
}

/// An axis type (the `CTYPE` name).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Freq,
    Ener,
    Wavn,
    Vrad,
    Wave,
    Vopt,
    Zopt,
    Awav,
    Velo,
    Beta,
}

impl Kind {
    fn of_name(name: &str) -> Option<Kind> {
        Some(match name {
            "FREQ" => Kind::Freq,
            "ENER" => Kind::Ener,
            "WAVN" => Kind::Wavn,
            "VRAD" => Kind::Vrad,
            "WAVE" => Kind::Wave,
            "VOPT" => Kind::Vopt,
            "ZOPT" => Kind::Zopt,
            "AWAV" => Kind::Awav,
            "VELO" => Kind::Velo,
            "BETA" => Kind::Beta,
            _ => return None,
        })
    }

    fn basic(self) -> Basic {
        match self {
            Kind::Freq | Kind::Ener | Kind::Wavn | Kind::Vrad => Basic::F,
            Kind::Wave | Kind::Vopt | Kind::Zopt => Basic::W,
            Kind::Awav => Basic::A,
            Kind::Velo | Kind::Beta => Basic::V,
        }
    }

    /// SI units per unit of `CUNIT` (empty: SI).
    fn unit(self, u: &str) -> Option<f64> {
        let u = u.trim();
        let s = match self {
            Kind::Freq => match u {
                "" | "Hz" => 1.0,
                "kHz" => 1e3,
                "MHz" => 1e6,
                "GHz" => 1e9,
                "THz" => 1e12,
                _ => return None,
            },
            Kind::Ener => match u {
                "" | "J" => 1.0,
                "erg" => 1e-7,
                "eV" => 1.602_176_634e-19,
                "keV" => 1.602_176_634e-16,
                "MeV" => 1.602_176_634e-13,
                _ => return None,
            },
            Kind::Wavn => match u {
                "" | "/m" | "m-1" | "m^-1" | "1/m" => 1.0,
                "/cm" | "cm-1" | "cm^-1" | "1/cm" => 100.0,
                _ => return None,
            },
            Kind::Vrad | Kind::Vopt | Kind::Velo => match u {
                "" | "m/s" | "m s-1" | "m.s-1" | "m s^-1" => 1.0,
                "km/s" | "km s-1" | "km.s-1" | "km s^-1" => 1e3,
                _ => return None,
            },
            Kind::Wave | Kind::Awav => match u {
                "" | "m" => 1.0,
                "cm" => 1e-2,
                "mm" => 1e-3,
                "um" => 1e-6,
                "nm" => 1e-9,
                "Angstrom" | "angstrom" | "Ang" | "A" => 1e-10,
                _ => return None,
            },
            Kind::Zopt | Kind::Beta => match u {
                "" => 1.0,
                _ => return None,
            },
        };
        Some(s)
    }

    /// The basic type's value (SI) of an axis value `x` (SI).
    fn to_basic(self, x: f64, r: &Rest) -> f64 {
        match self {
            Kind::Freq | Kind::Wave | Kind::Awav | Kind::Velo => x,
            Kind::Ener => x / H,
            Kind::Wavn => C * x,
            Kind::Vrad => r.nu0 * (1.0 - x / C),
            Kind::Vopt => r.lambda0 * (1.0 + x / C),
            Kind::Zopt => r.lambda0 * (1.0 + x),
            Kind::Beta => C * x,
        }
    }

    /// The inverse of [`Kind::to_basic`].
    fn of_basic(self, s: f64, r: &Rest) -> f64 {
        match self {
            Kind::Freq | Kind::Wave | Kind::Awav | Kind::Velo => s,
            Kind::Ener => s * H,
            Kind::Wavn => s / C,
            Kind::Vrad => C * (1.0 - s / r.nu0),
            Kind::Vopt => C * (s / r.lambda0 - 1.0),
            Kind::Zopt => s / r.lambda0 - 1.0,
            Kind::Beta => s / C,
        }
    }

    /// d(basic) / d(axis value), constant for every type.
    fn d_basic(self, r: &Rest) -> f64 {
        match self {
            Kind::Freq | Kind::Wave | Kind::Awav | Kind::Velo => 1.0,
            Kind::Ener => 1.0 / H,
            Kind::Wavn | Kind::Beta => C,
            Kind::Vrad => -r.nu0 / C,
            Kind::Vopt => r.lambda0 / C,
            Kind::Zopt => r.lambda0,
        }
    }

    /// Whether the conversion needs a rest frequency / wavelength.
    fn needs_rest(self) -> bool {
        matches!(self, Kind::Vrad | Kind::Vopt | Kind::Zopt)
    }
}

/// The rest frequency (Hz) and wavelength (m); zero when the header gives neither.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rest {
    nu0: f64,
    lambda0: f64,
}

/// Paper III eq. 65: the refractive index of air at vacuum wavelength `lambda` (m).
fn air_index(lambda: f64) -> f64 {
    let s = 1.0 / (lambda * 1e6).powi(2);
    1.0 + 1e-6 * (287.6155 + 1.628_87 * s + 0.013_60 * s * s)
}

/// Vacuum wavelength of air wavelength `la` (m): `la · n(λ)`, iterated to a fixed point.
fn air_to_vacuum(la: f64) -> f64 {
    let mut l = la;
    for _ in 0..8 {
        l = la * air_index(l);
    }
    l
}

/// Vacuum frequency (Hz) of a basic-type value.
fn nu_of(b: Basic, s: f64, r: &Rest) -> f64 {
    match b {
        Basic::F => s,
        Basic::W => C / s,
        Basic::A => C / air_to_vacuum(s),
        Basic::V => r.nu0 * ((C - s) / (C + s)).sqrt(),
    }
}

/// The basic-type value of vacuum frequency `nu` (Hz).
fn of_nu(b: Basic, nu: f64, r: &Rest) -> f64 {
    match b {
        Basic::F => nu,
        Basic::W => C / nu,
        Basic::A => {
            let l = C / nu;
            l / air_index(l)
        }
        Basic::V => {
            let (a, n) = (r.nu0 * r.nu0, nu * nu);
            C * (a - n) / (a + n)
        }
    }
}

/// d(frequency) / d(basic value) at `s`.
fn d_nu(b: Basic, s: f64, r: &Rest) -> f64 {
    match b {
        Basic::F => 1.0,
        Basic::W => -C / (s * s),
        Basic::A => {
            // Smooth and far from zero: a central difference is exact to ~1e-10.
            let h = s * 1e-6;
            (nu_of(b, s + h, r) - nu_of(b, s - h, r)) / (2.0 * h)
        }
        Basic::V => -r.nu0 * C / ((C + s).powf(1.5) * (C - s).sqrt()),
    }
}

/// A non-linear spectral axis, worked out once from its header.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Spectral {
    kind: Kind,
    linear: Basic,
    rest: Rest,
    /// SI units per unit of the axis.
    unit: f64,
    /// The linear type's value at the reference pixel, and its change per unit of
    /// intermediate world coordinate.
    p_ref: f64,
    dp: f64,
}

impl Spectral {
    /// The axis for `CTYPE` name `name` and algorithm `code` (`F2W`, …), or `None` when the
    /// code is not a Paper III `P2X` code, does not fit the name, or a value it needs
    /// (rest frequency, unit) is missing.
    pub(crate) fn new(
        name: &str,
        code: &str,
        cunit: &str,
        crval: f64,
        restfrq: Option<f64>,
        restwav: Option<f64>,
    ) -> Option<Spectral> {
        let c = code.as_bytes();
        if c.len() != 3 || c[1] != b'2' {
            return None;
        }
        let (linear, own) = (Basic::of_letter(c[0])?, Basic::of_letter(c[2])?);
        let kind = Kind::of_name(name)?;
        if kind.basic() != own || linear == own {
            return None;
        }
        let unit = kind.unit(cunit)?;
        let ok = |v: &f64| v.is_finite() && *v > 0.0;
        let (nu0, lambda0) = match (restfrq.filter(ok), restwav.filter(ok)) {
            (Some(f), _) => (f, C / f),
            (None, Some(w)) => (C / w, w),
            (None, None) => (0.0, 0.0),
        };
        let rest = Rest { nu0, lambda0 };
        if nu0 == 0.0 && (kind.needs_rest() || linear == Basic::V || own == Basic::V) {
            return None;
        }
        let x = crval * unit;
        let s = kind.to_basic(x, &rest);
        let nu = nu_of(own, s, &rest);
        let p_ref = of_nu(linear, nu, &rest);
        // dP/dx = dν/dS · dS/dx / (dν/dP), all at the reference.
        let dp = d_nu(own, s, &rest) * kind.d_basic(&rest) / d_nu(linear, p_ref, &rest) * unit;
        (p_ref.is_finite() && dp.is_finite() && dp != 0.0).then_some(Spectral {
            kind,
            linear,
            rest,
            unit,
            p_ref,
            dp,
        })
    }

    /// The axis value (in `CUNIT`) at intermediate world coordinate `w` (an offset in
    /// `CUNIT` from `CRVAL`, `CDELT` applied).
    pub(crate) fn world(&self, w: f64) -> f64 {
        let p = self.p_ref + w * self.dp;
        let nu = nu_of(self.linear, p, &self.rest);
        let s = of_nu(self.kind.basic(), nu, &self.rest);
        self.kind.of_basic(s, &self.rest) / self.unit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, rel: f64) -> bool {
        (a - b).abs() <= rel * b.abs().max(1e-300)
    }

    /// `WAVE-F2W`: linear in frequency, so wavelength is `c / (ν_r + w·dν/dλ)`.
    #[test]
    fn wavelength_linear_in_frequency() {
        let l0 = 500e-9;
        let s = Spectral::new("WAVE", "F2W", "", l0, None, None).unwrap();
        assert!(close(s.world(0.0), l0, 1e-14));
        let dl = 1e-10;
        let nu = C / l0 - dl * C / (l0 * l0);
        for k in [-100.0, -1.0, 1.0, 50.0] {
            let want = C / (C / l0 + k * (nu - C / l0));
            assert!(close(s.world(k * dl), want, 1e-13), "{k}");
        }
        // CUNIT scales the axis: the same axis in nm.
        let n = Spectral::new("WAVE", "F2W", "nm", 500.0, None, None).unwrap();
        assert!(close(n.world(0.1 * 7.0), s.world(1e-10 * 7.0) * 1e9, 1e-12));
    }

    /// `FREQ-W2F`, the inverse: frequency linear in wavelength.
    #[test]
    fn frequency_linear_in_wavelength() {
        let nu0 = 1.4e9;
        let s = Spectral::new("FREQ", "W2F", "Hz", nu0, None, None).unwrap();
        let d = 1e5;
        let dl = -C / (nu0 * nu0) * d;
        for k in [-3.0, 2.0, 1000.0] {
            let want = C / (C / nu0 + k * dl);
            assert!(close(s.world(k * d), want, 1e-12), "{k}");
        }
    }

    /// `VOPT-F2W` (optical velocity sampled in frequency, the common radio cube axis):
    /// V = c (ν₀/ν − 1) with ν linear.
    #[test]
    fn optical_velocity_linear_in_frequency() {
        let nu0 = 1_420_405_751.768;
        let v_r = 1.0e6; // m/s
        let s = Spectral::new("VOPT", "F2W", "m/s", v_r, Some(nu0), None).unwrap();
        let nu_r = nu0 / (1.0 + v_r / C);
        // dν/dV = −ν₀ / c / (1 + V/c)².
        let dnu = -nu0 / C / (1.0 + v_r / C).powi(2);
        for k in [-20.0, 0.0, 5.0, 300.0] {
            let w = k * 1e3;
            let nu = nu_r + w * dnu;
            assert!(close(s.world(w), C * (nu0 / nu - 1.0), 1e-11), "{k}");
        }
        // km/s, from RESTWAV instead.
        let k = Spectral::new("VOPT", "F2W", "km/s", 1000.0, None, Some(C / nu0)).unwrap();
        assert!(close(k.world(5.0), s.world(5e3) / 1e3, 1e-11));
        // Without a rest frequency there is no velocity.
        assert!(Spectral::new("VOPT", "F2W", "m/s", v_r, None, None).is_none());
    }

    /// `VELO-F2V`: relativistic velocity with frequency linear.
    #[test]
    fn relativistic_velocity_linear_in_frequency() {
        let nu0 = 1.0e11;
        let s = Spectral::new("VELO", "F2V", "m/s", 0.0, Some(nu0), None).unwrap();
        // At v = 0, dν/dv = −ν₀ / c.
        let w = 1e6;
        let nu = nu0 - w * nu0 / C;
        let want = C * (nu0 * nu0 - nu * nu) / (nu0 * nu0 + nu * nu);
        assert!(close(s.world(w), want, 1e-12));
        assert!(s.world(0.0).abs() < 1e-6);
    }

    /// `WAVE-A2W` and `AWAV-W2A`: Paper III's air index (n − 1 ≈ 2.79e-4 at 500 nm).
    #[test]
    fn air_and_vacuum() {
        let n = air_index(500e-9);
        assert!(close(
            n - 1.0,
            1e-6 * (287.6155 + 1.62887 / 0.25 + 0.0136 / 0.0625),
            1e-12
        ));
        let la = 500e-9;
        assert!(close(
            air_to_vacuum(la) / air_index(air_to_vacuum(la)),
            la,
            1e-15
        ));
        let s = Spectral::new("AWAV", "W2A", "", la, None, None).unwrap();
        assert!(close(s.world(0.0), la, 1e-14));
        // Linear in vacuum wavelength: the step in air shrinks by about n.
        let a = Spectral::new("WAVE", "A2W", "", air_to_vacuum(la), None, None).unwrap();
        assert!(close(a.world(0.0), air_to_vacuum(la), 1e-14));
        let step = s.world(1e-10) - s.world(0.0);
        assert!(close(step, 1e-10, 1e-3), "{step}");
    }

    #[test]
    fn refused() {
        // The name's basic type must be the code's second letter.
        assert!(Spectral::new("FREQ", "F2W", "", 1e9, None, None).is_none());
        assert!(Spectral::new("WAVE", "W2W", "", 1e-6, None, None).is_none());
        assert!(Spectral::new("WAVE", "TAB", "", 1e-6, None, None).is_none());
        assert!(Spectral::new("WAVE", "F2W", "parsec", 1e-6, None, None).is_none());
        assert!(Spectral::new("WAVE", "F2W", "", 0.0, None, None).is_none());
    }
}
