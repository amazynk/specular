//! Synthetic star fields with a known truth, for tests and timings: Gaussian or Moffat stars
//! of a given FWHM along both axes and angle, on a flat or sloping background with Gaussian
//! noise. Values are the profile sampled at pixel centres.

use rayon::prelude::*;

/// A star's profile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Profile {
    Gaussian,
    /// Moffat with this β.
    Moffat(f64),
}

/// The point spread function every star of a field has.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Psf {
    pub profile: Profile,
    /// FWHM along the major and minor axes (px).
    pub fwhm_major: f64,
    pub fwhm_minor: f64,
    /// The major axis' angle from +x towards +y (radians).
    pub angle: f64,
}

impl Psf {
    pub fn round(profile: Profile, fwhm: f64) -> Psf {
        Psf {
            profile,
            fwhm_major: fwhm,
            fwhm_minor: fwhm,
            angle: 0.0,
        }
    }

    /// `sqrt(1 − (minor / major)²)`.
    pub fn eccentricity(&self) -> f64 {
        (1.0 - (self.fwhm_minor / self.fwhm_major).powi(2))
            .max(0.0)
            .sqrt()
    }

    /// The geometric mean FWHM.
    pub fn fwhm(&self) -> f64 {
        (self.fwhm_major * self.fwhm_minor).sqrt()
    }

    /// The half-flux diameter of a round profile of FWHM [`Psf::fwhm`] (all of its light).
    pub fn hfd(&self) -> f64 {
        let f = self.fwhm();
        match self.profile {
            // r½ = σ·sqrt(2 ln 2) = FWHM / 2.
            Profile::Gaussian => f,
            Profile::Moffat(b) => {
                let alpha = f / (2.0 * (2f64.powf(1.0 / b) - 1.0).sqrt());
                2.0 * alpha * (2f64.powf(1.0 / (b - 1.0)) - 1.0).sqrt()
            }
        }
    }

    /// The value at offset (`dx`, `dy`) from the centre of a star of peak 1.
    pub fn at(&self, dx: f64, dy: f64) -> f64 {
        let (s, c) = self.angle.sin_cos();
        // Along the major axis and across it.
        let u = dx * c + dy * s;
        let v = -dx * s + dy * c;
        let (hu, hv) = (self.fwhm_major / 2.0, self.fwhm_minor / 2.0);
        // Normalised so the half maximum lies at u = hu, v = hv.
        let r2 = (u / hu).powi(2) + (v / hv).powi(2);
        match self.profile {
            Profile::Gaussian => (-std::f64::consts::LN_2 * r2).exp(),
            Profile::Moffat(b) => (1.0 + (2f64.powf(1.0 / b) - 1.0) * r2).powf(-b),
        }
    }

    /// The radius beyond which a star of peak `peak` adds less than `floor`.
    fn reach(&self, peak: f64, floor: f64) -> f64 {
        let ratio = (peak / floor).max(1.0);
        let h = self.fwhm_major / 2.0;
        let r = match self.profile {
            Profile::Gaussian => h * (ratio.ln() / std::f64::consts::LN_2).sqrt(),
            Profile::Moffat(b) => {
                h * ((ratio.powf(1.0 / b) - 1.0) / (2f64.powf(1.0 / b) - 1.0)).sqrt()
            }
        };
        r.clamp(3.0, 80.0)
    }
}

/// A star placed in a field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub x: f64,
    pub y: f64,
    pub peak: f64,
}

/// A field to render.
#[derive(Clone, Debug)]
pub struct Field {
    pub width: usize,
    pub height: usize,
    pub background: f64,
    /// Background change per pixel along x and y.
    pub slope: (f64, f64),
    pub noise: f64,
    pub psf: Psf,
    pub stars: Vec<Placed>,
    pub seed: u64,
}

/// xorshift64*: small, fast and reproducible.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Standard normal (Box–Muller).
    pub fn normal(&mut self) -> f64 {
        let u = self.unit().max(1e-300);
        let v = self.unit();
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

impl Field {
    /// A field of `count` stars on a jittered grid at least `margin` px from the edges and
    /// from each other's cells, peaks between `peak.0` and `peak.1` (log-uniform).
    #[allow(clippy::too_many_arguments)]
    pub fn grid(
        width: usize,
        height: usize,
        psf: Psf,
        count: usize,
        peak: (f64, f64),
        background: f64,
        noise: f64,
        seed: u64,
    ) -> Field {
        let mut rng = Rng::new(seed ^ 0x5157_4152);
        let margin = (psf.fwhm_major * 4.0).max(12.0);
        let aspect = width as f64 / height as f64;
        let ny = ((count as f64 / aspect).sqrt().ceil() as usize).max(1);
        let nx = count.div_ceil(ny).max(1);
        let cw = (width as f64 - 2.0 * margin) / nx as f64;
        let ch = (height as f64 - 2.0 * margin) / ny as f64;
        let mut stars = Vec::with_capacity(count);
        'all: for j in 0..ny {
            for i in 0..nx {
                if stars.len() == count {
                    break 'all;
                }
                let jx = 0.25 + 0.5 * rng.unit();
                let jy = 0.25 + 0.5 * rng.unit();
                let p = peak.0 * (peak.1 / peak.0).powf(rng.unit());
                stars.push(Placed {
                    x: margin + (i as f64 + jx) * cw,
                    y: margin + (j as f64 + jy) * ch,
                    peak: p,
                });
            }
        }
        Field {
            width,
            height,
            background,
            slope: (0.0, 0.0),
            noise,
            psf,
            stars,
            seed,
        }
    }

    /// The field's values, rows top-down.
    pub fn render(&self) -> Vec<f32> {
        let (w, h) = (self.width, self.height);
        let mut out = vec![0.0f32; w * h];
        // Background and noise, a row per task with its own generator.
        out.par_chunks_mut(w.max(1))
            .enumerate()
            .for_each(|(y, row)| {
                let mut rng = Rng::new(self.seed.wrapping_add(y as u64 * 0x1000_0001));
                for (x, o) in row.iter_mut().enumerate() {
                    let b = self.background + self.slope.0 * x as f64 + self.slope.1 * y as f64;
                    *o = (b + self.noise * rng.normal()) as f32;
                }
            });
        // Stars, each within its reach.
        let floor = (self.noise * 1e-3).max(1e-6);
        for s in &self.stars {
            let r = self.psf.reach(s.peak, floor);
            let x0 = (s.x - r).floor().max(0.0) as usize;
            let x1 = ((s.x + r).ceil() as usize).min(w.saturating_sub(1));
            let y0 = (s.y - r).floor().max(0.0) as usize;
            let y1 = ((s.y + r).ceil() as usize).min(h.saturating_sub(1));
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let v = s.peak * self.psf.at(x as f64 - s.x, y as f64 - s.y);
                    out[y * w + x] += v as f32;
                }
            }
        }
        out
    }
}

/// A FITS file of `values` (`width × height`, rows top-down as displayed) as 16-bit unsigned
/// integers (`BZERO` 32768, values rounded and clamped), the way cameras write subframes,
/// with the header `cards` (keyword, value as written: strings in quotes).
pub fn fits_u16(values: &[f32], width: usize, height: usize, cards: &[(&str, &str)]) -> Vec<u8> {
    let mut h: Vec<u8> = Vec::new();
    let mut card = |k: &str, v: &str| {
        let c = format!("{k:<8}= {v:>20}");
        h.extend(format!("{c:<80.80}").as_bytes());
    };
    card("SIMPLE", "T");
    card("BITPIX", "16");
    card("NAXIS", "2");
    card("NAXIS1", &width.to_string());
    card("NAXIS2", &height.to_string());
    card("BZERO", "32768");
    card("BSCALE", "1");
    for (k, v) in cards {
        card(k, v);
    }
    h.extend(format!("{:<80}", "END").as_bytes());
    h.resize(h.len().div_ceil(2880) * 2880, b' ');
    // FITS rows run bottom-up.
    for y in (0..height).rev() {
        for &v in &values[y * width..(y + 1) * width] {
            let u = v.round().clamp(0.0, 65535.0) as i32 - 32768;
            h.extend((u as i16).to_be_bytes());
        }
    }
    h.resize(h.len().div_ceil(2880) * 2880, 0);
    h
}
