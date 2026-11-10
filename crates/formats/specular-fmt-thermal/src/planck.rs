//! FLIR counts to temperature.
//!
//! The camera's calibration fits its detector signal `S` to blackbody temperature `T`
//! (kelvin) with a Planck-shaped curve `S = R1 / (R2 (exp(B / T) - F)) - O`. What the
//! detector sees is the object's own radiance, attenuated by the air and an optional window,
//! plus the reflected surroundings, the air's and the window's own radiance. Following the
//! published model (Minkina and Dudzik, *Infrared Thermography: Errors and
//! Uncertainties*, 2009; the FLIR technical notes), the window sits half-way along the
//! path, the air's transmission over each half is the two-band fit
//! `X exp(-sqrt(d/2) (a1 + b1 sqrt(h))) + (1 - X) exp(-sqrt(d/2) (a2 + b2 sqrt(h)))`
//! where `h` is the water vapour content from relative humidity and air temperature, and
//! the window reflects nothing. The object signal is solved for and inverted through the
//! curve.

use crate::Conditions;

/// The calibration and atmospheric constants of a FLIR camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Planck {
    pub r1: f64,
    pub r2: f64,
    pub b: f64,
    pub f: f64,
    pub o: f64,
    pub alpha1: f64,
    pub alpha2: f64,
    pub beta1: f64,
    pub beta2: f64,
    pub x: f64,
}

/// Defaults where a file records nothing.
const EMISSIVITY: f64 = 1.0;
const DISTANCE: f64 = 1.0;
const AMBIENT: f64 = 20.0;
const HUMIDITY: f64 = 0.5;

/// Water vapour content from relative humidity (a fraction) and air temperature (degrees
/// Celsius).
pub fn h2o(humidity: f64, air: f64) -> f64 {
    humidity
        * (1.5587 + 0.06939 * air - 0.000_278_16 * air * air + 0.000_000_684_55 * air * air * air)
            .exp()
}

/// Atmospheric transmission over `distance` metres, half of it on each side of a window.
pub fn transmission(p: &Planck, distance: f64, h2o: f64) -> f64 {
    let d = (distance / 2.0).sqrt();
    let w = h2o.sqrt();
    p.x * (-d * (p.alpha1 + p.beta1 * w)).exp()
        + (1.0 - p.x) * (-d * (p.alpha2 + p.beta2 * w)).exp()
}

impl Planck {
    /// Counts of a blackbody at `celsius`.
    pub fn counts(&self, celsius: f64) -> f64 {
        self.r1 / (self.r2 * ((self.b / (celsius + 273.15)).exp() - self.f)) - self.o
    }

    /// Temperature of a blackbody giving `counts`.
    pub fn blackbody(&self, counts: f64) -> f64 {
        self.b / (self.r1 / (self.r2 * (counts + self.o)) + self.f).ln() - 273.15
    }

    /// Precomputes the chain for `c` (missing parameters take neutral defaults).
    pub fn solver(&self, c: &Conditions) -> Solver {
        let e = c.emissivity.unwrap_or(EMISSIVITY);
        let distance = c.distance.unwrap_or(DISTANCE);
        let refl = c.reflected.unwrap_or(AMBIENT);
        let atm = c.atmospheric.unwrap_or(AMBIENT);
        let wind = c.window_temperature.unwrap_or(atm);
        let irt = c.window_transmission.unwrap_or(1.0);
        let rh = c.humidity.unwrap_or(HUMIDITY);
        let tau = transmission(self, distance, h2o(rh, atm));
        let (tau1, tau2) = (tau, tau);
        let emiss_wind = 1.0 - irt;
        let refl_wind = 0.0;
        let raw_refl = self.counts(refl);
        let raw_atm = self.counts(atm);
        let raw_wind = self.counts(wind);
        let offset = (1.0 - e) / e * raw_refl
            + (1.0 - tau1) / e / tau1 * raw_atm
            + emiss_wind / e / tau1 / irt * raw_wind
            + refl_wind / e / tau1 / irt * raw_refl
            + (1.0 - tau2) / e / tau1 / irt / tau2 * raw_atm;
        Solver {
            planck: *self,
            scale: 1.0 / e / tau1 / irt / tau2,
            offset,
        }
    }
}

/// The radiometric chain for fixed conditions.
#[derive(Clone, Copy, Debug)]
pub struct Solver {
    planck: Planck,
    scale: f64,
    offset: f64,
}

impl Solver {
    /// Degrees Celsius of the object behind `counts` (NaN outside the calibration).
    pub fn celsius(&self, counts: f64) -> f64 {
        self.planck.blackbody(counts * self.scale - self.offset)
    }
}
