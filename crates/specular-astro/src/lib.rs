//! Astronomy subframe metrics: what a set of exposures of the same field needs to be sorted
//! before stacking (DESIGN.md §6.3).
//!
//! For one frame ([`measure`]):
//! 1. **Background and noise** ([`background`]): a mesh of 64 px tiles, each with a
//!    sigma-clipped median and standard deviation (3σ, from the MAD) of a subsample, smoothed
//!    with a 3×3 median over the mesh. The noise is the median of the tiles'.
//! 2. **Detection**: local maxima whose 3×3 mean is at least 5 noise units up; each looked at
//!    from its isophotal moments, extra maxima on a brighter star's core dropped, hot pixels
//!    and stars within a window of the edge left out. Their number is the star count.
//! 3. **Measurement** ([`fit`]) of the brightest unsaturated stars (300 at most): an elliptical
//!    Moffat profile with a flat background, fitted by Levenberg–Marquardt with neighbours
//!    masked out, gives the FWHM along both axes, the eccentricity and the angle; the
//!    half-flux diameter comes from the pixels themselves.
//! 4. **Per frame**: the star count, the median HFD, FWHM and eccentricity, the background,
//!    the noise and an SNR estimate (the mean absolute deviation from the background over the
//!    noise: the square root of PixInsight's SNRWeight).
//!
//! The work is spread over the global rayon pool: tiles, rows and stars in parallel.
//!
//! [`reject`] suggests frames to reject (a metric beyond k·σ of the median), and [`header`]
//! reads the FITS keywords subframes are sorted by (object, filter, exposure, gain, sensor
//! temperature, date, frame type) and the plate scale.
//!
//! Interop with other astronomy software: [`regions`] reads and writes DS9 region files,
//! [`frames`] converts between ICRS, FK5 and galactic coordinates, [`solve`] runs plate
//! solvers the user installed (ASTAP, astrometry.net) and reads their solutions, and
//! [`hips`] paints HiPS sky survey tiles into any view with sky coordinates.

pub mod background;
mod detect;
pub mod fit;
pub mod frames;
pub mod header;
pub mod hips;
pub mod regions;
pub mod reject;
pub mod solve;
pub mod synth;
#[cfg(test)]
mod tests;

use rayon::prelude::*;

pub use background::Background;
pub use fit::Star;
pub use header::SubframeHeader;
pub use reject::{Metric, Suggestion, suggest};

/// A frame's values: `width × height`, rows top-down; NaN for blank pixels.
#[derive(Clone, Copy, Debug)]
pub struct Image<'a> {
    pub values: &'a [f32],
    pub width: usize,
    pub height: usize,
}

/// How a frame is measured.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// A star's 3×3 mean must stand this many noise units above the background.
    pub detect_sigma: f32,
    /// Stars fitted at most (the brightest unsaturated ones).
    pub max_fit: usize,
    /// Values at or above this are saturated; `None`: the frame's maximum, where at least
    /// three pixels of a star reach it.
    pub saturation: Option<f32>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            detect_sigma: 5.0,
            max_fit: 300,
            saturation: None,
        }
    }
}

/// What one frame measures. Sizes are in the frame's pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameMetrics {
    /// Stars detected (saturated ones included).
    pub stars: u32,
    /// Stars measured (fitted), whose medians follow.
    pub measured: u32,
    pub hfd: Option<f32>,
    pub fwhm: Option<f32>,
    pub eccentricity: Option<f32>,
    /// The median background level and the noise (standard deviation), in the frame's
    /// values.
    pub background: f32,
    pub noise: f32,
    pub snr: Option<f32>,
    /// Arc seconds per pixel, when the header gives the plate scale.
    pub scale: Option<f32>,
}

impl FrameMetrics {
    /// The median HFD in arc seconds, when the plate scale is known.
    pub fn hfd_arcsec(&self) -> Option<f32> {
        Some(self.hfd? * self.scale?)
    }

    /// The median FWHM in arc seconds, when the plate scale is known.
    pub fn fwhm_arcsec(&self) -> Option<f32> {
        Some(self.fwhm? * self.scale?)
    }

    /// The value of `m`.
    pub fn get(&self, m: Metric) -> Option<f32> {
        match m {
            Metric::Hfd => self.hfd,
            Metric::Fwhm => self.fwhm,
            Metric::Eccentricity => self.eccentricity,
            Metric::Stars => Some(self.stars as f32),
            Metric::Background => Some(self.background),
            Metric::Noise => Some(self.noise),
            Metric::Snr => self.snr,
        }
    }

    /// The metrics scaled for a frame measured at `1 / factor` of its size (a colour
    /// camera's raw frame measured 2 × 2 binned).
    pub fn scaled(mut self, factor: f32) -> FrameMetrics {
        self.hfd = self.hfd.map(|v| v * factor);
        self.fwhm = self.fwhm.map(|v| v * factor);
        self
    }

    /// The record kept with a thumbnail ([`FrameMetrics::from_bytes`] reads it).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = vec![1u8];
        b.extend_from_slice(&self.stars.to_le_bytes());
        b.extend_from_slice(&self.measured.to_le_bytes());
        for v in [
            self.hfd,
            self.fwhm,
            self.eccentricity,
            Some(self.background),
            Some(self.noise),
            self.snr,
            self.scale,
        ] {
            b.extend_from_slice(&v.unwrap_or(f32::NAN).to_le_bytes());
        }
        b
    }

    /// Reads [`FrameMetrics::to_bytes`]; `None` for another version or a short record.
    pub fn from_bytes(b: &[u8]) -> Option<FrameMetrics> {
        if b.first() != Some(&1) || b.len() < 1 + 8 + 7 * 4 {
            return None;
        }
        let u = |i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap_or_default());
        let f = |k: usize| {
            let v = f32::from_bits(u(9 + 4 * k));
            v.is_finite().then_some(v)
        };
        Some(FrameMetrics {
            stars: u(1),
            measured: u(5),
            hfd: f(0),
            fwhm: f(1),
            eccentricity: f(2),
            background: f(3).unwrap_or(0.0),
            noise: f(4).unwrap_or(0.0),
            snr: f(5),
            scale: f(6),
        })
    }
}

/// Measures a frame: the per-frame metrics and the stars fitted.
pub fn measure_stars(img: &Image<'_>, opts: &Options) -> (FrameMetrics, Vec<Star>) {
    assert_eq!(img.values.len(), img.width * img.height, "frame size");
    let bg = Background::estimate(img);
    let mut m = FrameMetrics {
        background: bg.median,
        noise: bg.noise,
        snr: (bg.noise > 0.0).then(|| bg.mean_dev / bg.noise),
        ..FrameMetrics::default()
    };
    if bg.noise.is_nan() || bg.noise <= 0.0 {
        return (m, Vec::new());
    }
    let (peaks, max) = detect::peaks(img, &bg, opts.detect_sigma);
    let sat = opts.saturation.unwrap_or(max * 0.999);
    let cands = detect::candidates(img, &bg, peaks, sat, 0.35);
    m.stars = cands.len() as u32;
    // Grid of every candidate for masking neighbours.
    let cell = 32usize;
    let gw = img.width.div_ceil(cell).max(1);
    let mut grid: Vec<Vec<u32>> = vec![Vec::new(); gw * img.height.div_ceil(cell).max(1)];
    for (i, c) in cands.iter().enumerate() {
        grid[c.y / cell * gw + c.x / cell].push(i as u32);
    }
    let ctx = fit::Ctx {
        img,
        bg: &bg,
        all: &cands,
        grid: &grid,
        cell,
        grid_w: gw,
    };
    // Brightest first; saturated ones are counted, not measured.
    let pick: Vec<usize> = (0..cands.len())
        .filter(|&i| !cands[i].saturated)
        .take(opts.max_fit)
        .collect();
    let stars: Vec<Star> = pick
        .par_iter()
        .filter_map(|&i| fit::star(&ctx, i))
        .collect();
    m.measured = stars.len() as u32;
    let med = |f: fn(&Star) -> f32| background::median_of(stars.iter().map(f));
    m.hfd = med(|s| s.hfd);
    m.fwhm = med(|s| s.fwhm);
    m.eccentricity = med(|s| s.eccentricity);
    (m, stars)
}

/// Measures a frame (see the crate's documentation).
pub fn measure(img: &Image<'_>, opts: &Options) -> FrameMetrics {
    measure_stars(img, opts).0
}

/// A colour camera's raw frame as luminance at half size: each 2 × 2 cell of the mosaic
/// summed (one red, two green, one blue whatever the pattern), so stars are not cut up by
/// it. Measure that and scale the sizes by 2 ([`FrameMetrics::scaled`]).
pub fn bin2(img: &Image<'_>) -> (Vec<f32>, usize, usize) {
    let (w, h) = (img.width / 2, img.height / 2);
    let mut out = vec![0.0f32; w * h];
    out.par_chunks_mut(w.max(1))
        .enumerate()
        .for_each(|(y, row)| {
            let a = &img.values[2 * y * img.width..];
            let b = &img.values[(2 * y + 1) * img.width..];
            for (x, o) in row.iter_mut().enumerate() {
                *o = a[2 * x] + a[2 * x + 1] + b[2 * x] + b[2 * x + 1];
            }
        });
    (out, w, h)
}
