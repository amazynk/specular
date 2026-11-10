//! Inspection probes: radiograph evaluation and calibrated photo measurement (NDT).
//!
//! Everything works on a [`Grey`] block of single-channel values (stored grey values of a
//! radiograph, or the luma of a photo) in stored pixel coordinates (see the crate docs).
//!
//! - [`profile`]: values along a line, bilinear, one sample per pixel of length, averaged
//!   over `width` parallel lines one pixel apart.
//! - [`Density`]: a two-point grey → optical density calibration (linear in grey).
//! - [`snr`]: mean / σ (sample standard deviation) in a region of at least 20 × 55 pixels
//!   ([`snr_region`]), and the normalised SNRn = SNR × 88.6 µm / SRb (ISO 17636-2,
//!   EN ISO 16371).
//! - [`duplex`]: the duplex-wire IQI (ISO 19232-5): the wire pairs along a profile, each
//!   pair's dip, the first unresolved pair by the 20 % dip criterion (SRb) and the
//!   interpolated iSRb of ISO 19232-5:2018 / ASTM E2002-15.
//! - [`wire`]: the single-wire IQI (ISO 19232-1): a visible wire's width across it (full width
//!   at half maximum) and the nearest wire of the standard's table.
//! - [`crack`]: the width of a dark crack across a marked line at N points, edges at half
//!   maximum.
//!
//! # Duplex-wire algorithm (the oracle implements the same steps independently)
//!
//! 1. The profile is averaged over 3 samples. Background `b` = its median. The wires are on
//!    the side of `b` with the larger excursion (dark wires on a detector image, light ones on
//!    film density); the signal is `s = ±(v − b)`, wires positive.
//! 2. Noise σn = 1.4826 × the median absolute deviation of `s`; threshold
//!    `T = max(3 σn, 0.02 × max s)`.
//! 3. Runs where `s > T` are wires or merged pairs; runs peaking below 2 T are noise (or
//!    wires too faint to judge) and dropped. A run is joined to the next when the gap between them is shorter
//!    than 1.5 × the narrower run and the wider is at most twice the narrower (the two wires
//!    of a pair are alike, and the gap between them equals a wire's diameter; pairs are much
//!    further apart). A group takes at most two runs: the two wires of a pair.
//! 4. A pair's peaks `A`, `B` are the maxima of `s` in the two halves of its group (split at
//!    the group's middle sample); `C` is the minimum between them. Dip = (A + B − 2C) /
//!    (A + B) × 100 %, kept within 0–100 %. A pair is resolved when the dip is at least 20 %.
//! 5. Pairs are put in order of decreasing size (the profile is reversed when its last group
//!    is wider than its first). The first resolved pair's wire diameter is half its peak
//!    separation (the centre distance of a pair is two diameters), which picks its number
//!    from [`DUPLEX`]; later pairs are numbered on from it.
//! 6. The first pair below 20 % gives the image unsharpness (2 d) and SRb = d. When every
//!    pair seen is resolved, the next pair of the table (too faint to see) is the first
//!    unresolved.
//! 7. iSRb (ISO 19232-5:2018, ASTM E2002-15 §7.2.3): the 20 % dip interpolated over the
//!    wire diameter d. Points (d, dip): the last two resolved pairs before the first
//!    unresolved one, and up to two unresolved pairs from it on whose dip is above 0 (none:
//!    the first unresolved pair, dip 0 when it is too faint to see). Three or more points
//!    are fitted with a least-squares second-order polynomial; iSRb is its 20 % crossing
//!    between the last resolved and the first unresolved pair's diameters. Two points, or
//!    a fit with no crossing there, interpolate linearly between those two pairs. iSRb is
//!    never below the sample spacing (aliasing; E2002 §7.2.3.5) and needs a resolved pair.
//!
//! # PROVENANCE of the wire tables
//!
//! [`WIRES`] (ISO 19232-1 single-wire IQI, W1–W19) and [`DUPLEX`] (ISO 19232-5 duplex-wire
//! IQI, D1–D15) are the nominal wire diameters as published in the standards' public
//! summaries and in IQI makers' data sheets: the R10 preferred-number series from 3.2 mm down
//! to 0.05 mm for single wires, and 0.80 mm down to 0.032 mm for duplex pairs (D14 and D15
//! were added in the 2018 edition). They are facts (nominal sizes), not text of the
//! standards; see `inspect.PROVENANCE.md`.

use crate::calib::Calibration;
use crate::geom::P;

/// Single-wire IQI wires (ISO 19232-1): number and nominal diameter (mm).
pub const WIRES: [(u8, f64); 19] = [
    (1, 3.20),
    (2, 2.50),
    (3, 2.00),
    (4, 1.60),
    (5, 1.25),
    (6, 1.00),
    (7, 0.80),
    (8, 0.63),
    (9, 0.50),
    (10, 0.40),
    (11, 0.32),
    (12, 0.25),
    (13, 0.20),
    (14, 0.16),
    (15, 0.125),
    (16, 0.100),
    (17, 0.080),
    (18, 0.063),
    (19, 0.050),
];

/// Duplex-wire IQI pairs (ISO 19232-5): number and nominal wire diameter (mm); the gap
/// between the two wires of a pair equals the diameter, and the pair's unsharpness is twice
/// it.
pub const DUPLEX: [(u8, f64); 15] = [
    (1, 0.80),
    (2, 0.63),
    (3, 0.50),
    (4, 0.40),
    (5, 0.32),
    (6, 0.25),
    (7, 0.20),
    (8, 0.16),
    (9, 0.13),
    (10, 0.10),
    (11, 0.080),
    (12, 0.063),
    (13, 0.050),
    (14, 0.040),
    (15, 0.032),
];

/// The smallest SNR region (pixels) of ISO 17636-2 / EN ISO 16371: 20 × 55.
pub const SNR_REGION: (u32, u32) = (20, 55);
/// The reference resolution of SNRn (µm).
pub const SNR_REFERENCE_UM: f64 = 88.6;
/// The dip a duplex pair needs to count as resolved (%).
pub const DIP_RESOLVED: f64 = 20.0;
/// Lines averaged across an IQI profile (along the wires, to beat the noise down).
pub const IQI_WIDTH: u32 = 21;
/// Points a crack is measured at.
pub const CRACK_POINTS: usize = 10;

/// What an inspection measurement measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Probe {
    /// Grey values (and densities) along a line.
    Profile,
    /// SNR and SNRn in a region.
    Snr,
    /// A duplex-wire IQI.
    Duplex,
    /// A single-wire IQI's visible wire.
    Wire,
    /// A crack's width across a marked line.
    Crack,
    /// One point's grey value and density; a known density there calibrates.
    Density,
}

impl Probe {
    pub const ALL: [Probe; 6] = [
        Probe::Profile,
        Probe::Snr,
        Probe::Duplex,
        Probe::Wire,
        Probe::Crack,
        Probe::Density,
    ];

    /// A stable identifier for files (the CSV `shape` column) and string ids.
    pub fn key(self) -> &'static str {
        match self {
            Probe::Profile => "profile",
            Probe::Snr => "snr",
            Probe::Duplex => "duplex-iqi",
            Probe::Wire => "wire-iqi",
            Probe::Crack => "crack",
            Probe::Density => "density",
        }
    }

    pub fn from_key(key: &str) -> Option<Probe> {
        Probe::ALL.into_iter().find(|p| p.key() == key)
    }

    /// Points that complete it (`None`: two or more, closed with Enter).
    pub fn points(self) -> Option<usize> {
        match self {
            Probe::Density => Some(1),
            Probe::Crack => None,
            _ => Some(2),
        }
    }
}

/// A block of single-channel values: `w × h` from image pixel `(x0, y0)`, row-major; NaN
/// where there is no value.
#[derive(Clone, Debug, PartialEq)]
pub struct Grey {
    pub x0: u32,
    pub y0: u32,
    pub w: u32,
    pub h: u32,
    pub data: Vec<f64>,
}

impl Grey {
    pub fn new(x0: u32, y0: u32, w: u32, h: u32, data: Vec<f64>) -> Option<Grey> {
        (w > 0 && h > 0 && data.len() == w as usize * h as usize).then_some(Grey {
            x0,
            y0,
            w,
            h,
            data,
        })
    }

    /// The value of image pixel `(x, y)`, if in the block.
    pub fn pixel(&self, x: u32, y: u32) -> Option<f64> {
        let (i, j) = (x.checked_sub(self.x0)?, y.checked_sub(self.y0)?);
        (i < self.w && j < self.h).then(|| self.data[j as usize * self.w as usize + i as usize])
    }

    /// The value at a point (bilinear between pixel centres; the edge pixels extend
    /// outwards).
    pub fn sample(&self, p: P) -> f64 {
        let u = (p.x - 0.5 - f64::from(self.x0)).clamp(0.0, f64::from(self.w - 1));
        let v = (p.y - 0.5 - f64::from(self.y0)).clamp(0.0, f64::from(self.h - 1));
        let i = (u.floor() as u32).min(self.w.saturating_sub(2));
        let j = (v.floor() as u32).min(self.h.saturating_sub(2));
        let (fu, fv) = (u - f64::from(i), v - f64::from(j));
        let at = |i: u32, j: u32| {
            let (i, j) = (i.min(self.w - 1), j.min(self.h - 1));
            self.data[j as usize * self.w as usize + i as usize]
        };
        let top = at(i, j) * (1.0 - fu) + at(i + 1, j) * fu;
        let bottom = at(i, j + 1) * (1.0 - fu) + at(i + 1, j + 1) * fu;
        top * (1.0 - fv) + bottom * fv
    }
}

/// Values along `a`→`b`: `n + 1` samples with `n` = the length rounded up (at least 1),
/// each the mean over `width` parallel lines one pixel apart. Returns the samples and the
/// step between them (pixels).
pub fn profile(g: &Grey, a: P, b: P, width: u32) -> (Vec<f64>, f64) {
    let d = b - a;
    let len = d.hypot();
    let n = (len.ceil() as usize).max(1);
    let (nx, ny) = if len > 0.0 {
        (-d.y / len, d.x / len)
    } else {
        (0.0, 0.0)
    };
    let width = width.max(1);
    let half = f64::from(width - 1) / 2.0;
    let values = (0..=n)
        .map(|k| {
            let t = k as f64 / n as f64;
            let c = P::new(a.x + d.x * t, a.y + d.y * t);
            let sum: f64 = (0..width)
                .map(|j| {
                    let o = f64::from(j) - half;
                    g.sample(P::new(c.x + nx * o, c.y + ny * o))
                })
                .sum();
            sum / f64::from(width)
        })
        .collect();
    (values, len / n as f64)
}

/// Millimetres per pixel along `a`→`b` under `cal` (`None`: uncalibrated, or a unit that
/// is not a length).
pub fn mm_per_px(cal: &Calibration, a: P, b: P) -> Option<f64> {
    let len = (b - a).hypot();
    if !cal.is_calibrated() || len <= 0.0 {
        return None;
    }
    let m = cal.unit.metres()?;
    let v = cal.distance(a, b) * m * 1000.0 / len;
    (v.is_finite() && v > 0.0).then_some(v)
}

fn median(v: &[f64]) -> f64 {
    let mut s: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    if s.is_empty() {
        return f64::NAN;
    }
    s.sort_by(f64::total_cmp);
    let n = s.len();
    if n % 2 == 1 {
        s[n / 2]
    } else {
        (s[n / 2 - 1] + s[n / 2]) / 2.0
    }
}

// ----- Density ---------------------------------------------------------------------------

/// A two-point grey → optical density calibration, linear in grey.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Density {
    pub grey: [f64; 2],
    pub density: [f64; 2],
}

impl Density {
    /// `None` when the two greys are the same (or a value is not finite).
    pub fn new(grey: [f64; 2], density: [f64; 2]) -> Option<Density> {
        let ok = grey.iter().chain(&density).all(|v| v.is_finite()) && grey[0] != grey[1];
        ok.then_some(Density { grey, density })
    }

    pub fn at(&self, g: f64) -> f64 {
        let [g0, g1] = self.grey;
        let [d0, d1] = self.density;
        d0 + (g - g0) * (d1 - d0) / (g1 - g0)
    }
}

// ----- SNR -------------------------------------------------------------------------------

/// The integer region an SNR is measured in: `rect` rounded to whole pixels and grown about
/// its centre to at least 20 × 55 (55 along its longer side), kept inside the image. `None`
/// when the image is smaller than that.
pub fn snr_region(rect: (f64, f64, f64, f64), iw: u32, ih: u32) -> Option<(u32, u32, u32, u32)> {
    let (x0, y0, x1, y1) = (
        rect.0.min(rect.2),
        rect.1.min(rect.3),
        rect.0.max(rect.2),
        rect.1.max(rect.3),
    );
    let (short, long) = SNR_REGION;
    let landscape = x1 - x0 >= y1 - y0;
    let (mw, mh) = if landscape {
        (long, short)
    } else {
        (short, long)
    };
    let fit = |a: f64, b: f64, min: u32, size: u32| -> Option<(u32, u32)> {
        let n = ((b - a).round().max(0.0) as u32).max(min);
        if n > size {
            return None;
        }
        let start = ((a + b) / 2.0 - f64::from(n) / 2.0).round();
        let start = start.clamp(0.0, f64::from(size - n)) as u32;
        Some((start, start + n))
    };
    let (rx0, rx1) = fit(x0, x1, mw, iw)?;
    let (ry0, ry1) = fit(y0, y1, mh, ih)?;
    Some((rx0, ry0, rx1, ry1))
}

/// An SNR reading.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snr {
    /// The region measured (whole pixels, half-open).
    pub region: (u32, u32, u32, u32),
    pub mean: f64,
    /// Sample standard deviation.
    pub sd: f64,
    pub count: usize,
    pub snr: f64,
    /// The SRb it was normalised with (µm) and SNRn.
    pub normalised: Option<(f64, f64)>,
}

/// SNR in `region` (from [`snr_region`]); `None` when it has fewer than two values or no
/// noise.
pub fn snr(g: &Grey, region: (u32, u32, u32, u32), srb_um: Option<f64>) -> Option<Snr> {
    let (x0, y0, x1, y1) = region;
    let mut vals = Vec::with_capacity(((x1 - x0) * (y1 - y0)) as usize);
    for y in y0..y1 {
        for x in x0..x1 {
            if let Some(v) = g.pixel(x, y).filter(|v| v.is_finite()) {
                vals.push(v);
            }
        }
    }
    let n = vals.len();
    if n < 2 {
        return None;
    }
    let mean = vals.iter().sum::<f64>() / n as f64;
    let ss: f64 = vals.iter().map(|v| (v - mean) * (v - mean)).sum();
    let sd = (ss / (n - 1) as f64).sqrt();
    if sd <= 0.0 || !sd.is_finite() {
        return None;
    }
    let snr = mean / sd;
    Some(Snr {
        region,
        mean,
        sd,
        count: n,
        snr,
        normalised: srb_um
            .filter(|s| s.is_finite() && *s > 0.0)
            .map(|s| (s, normalise(snr, s))),
    })
}

/// SNRn = SNR × 88.6 µm / SRb.
pub fn normalise(snr: f64, srb_um: f64) -> f64 {
    snr * SNR_REFERENCE_UM / srb_um
}

// ----- Signal helpers --------------------------------------------------------------------

/// `v` averaged over `k` samples centred on each (fewer at the ends).
fn smooth(v: &[f64], k: usize) -> Vec<f64> {
    let h = k / 2;
    (0..v.len())
        .map(|i| {
            let (a, b) = (i.saturating_sub(h), (i + h).min(v.len() - 1));
            v[a..=b].iter().sum::<f64>() / (b - a + 1) as f64
        })
        .collect()
}

/// The wire signal of a profile smoothed over `k` samples: `(s, background)` with wires
/// positive.
fn signal(v: &[f64], k: usize) -> (Vec<f64>, f64) {
    let v = &smooth(v, k);
    let b = median(v);
    let (lo, hi) = v
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), x| {
            (lo.min(*x), hi.max(*x))
        });
    let sign = if b - lo >= hi - b { -1.0 } else { 1.0 };
    (v.iter().map(|x| sign * (x - b)).collect(), b)
}

/// The full width at half maximum of the peak at `peak` in `s` (samples, interpolated).
fn fwhm_at(s: &[f64], peak: usize, base: f64) -> Option<(f64, f64)> {
    let top = s[peak];
    let half = base + (top - base) / 2.0;
    if top <= base {
        return None;
    }
    let mut l = peak;
    while l > 0 && s[l - 1] > half {
        l -= 1;
    }
    if l == 0 {
        return None;
    }
    let left = (l - 1) as f64 + (half - s[l - 1]) / (s[l] - s[l - 1]);
    let mut r = peak;
    while r + 1 < s.len() && s[r + 1] > half {
        r += 1;
    }
    if r + 1 >= s.len() {
        return None;
    }
    let right = r as f64 + (s[r] - half) / (s[r] - s[r + 1]);
    Some((left, right))
}

// ----- Duplex-wire IQI -------------------------------------------------------------------

/// One wire pair along a duplex profile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pair {
    /// D number and nominal wire diameter (mm), once numbered.
    pub number: Option<(u8, f64)>,
    /// The group's first and last sample (in the profile as drawn).
    pub span: (usize, usize),
    pub dip: f64,
    pub resolved: bool,
}

/// A duplex-wire reading.
#[derive(Clone, Debug, PartialEq)]
pub struct Duplex {
    pub pairs: Vec<Pair>,
    /// Index into `pairs` of the first unresolved pair.
    pub first_unresolved: Option<usize>,
    /// The interpolated basic spatial resolution iSRb (mm, step 7); the interpolated
    /// unsharpness iUT is twice it.
    pub isrb: Option<f64>,
}

impl Duplex {
    /// The first unresolved pair's number and diameter: the image's SRb (mm) is the
    /// diameter, its unsharpness twice it.
    pub fn srb(&self) -> Option<(u8, f64)> {
        self.pairs.get(self.first_unresolved?)?.number
    }
}

/// Why a probe gave no reading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// It needs millimetres: the image has no scale.
    NeedsScale,
    /// No wires (or no crack) stand out of the noise.
    NothingFound,
    /// The SNR region does not fit in the image.
    TooSmall,
    /// The region has no noise (or no values).
    NoNoise,
}

impl Refusal {
    pub fn key(self) -> &'static str {
        match self {
            Refusal::NeedsScale => "needs-scale",
            Refusal::NothingFound => "nothing-found",
            Refusal::TooSmall => "too-small",
            Refusal::NoNoise => "no-noise",
        }
    }
}

/// The wire pairs along profile `v` with `mm` millimetres per sample.
pub fn duplex(v: &[f64], mm: f64) -> Result<Duplex, Refusal> {
    if v.len() < 5 {
        return Err(Refusal::NothingFound);
    }
    let (s, _) = signal(v, 3);
    let dev: Vec<f64> = {
        let m = median(&s);
        s.iter().map(|x| (x - m).abs()).collect()
    };
    let noise = 1.4826 * median(&dev);
    let top = s.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let thr = (3.0 * noise).max(0.02 * top);
    if top <= thr {
        return Err(Refusal::NothingFound);
    }
    // Runs above the threshold.
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut start = None;
    for (i, x) in s.iter().enumerate() {
        match (start, *x > thr) {
            (None, true) => start = Some(i),
            (Some(a), false) => {
                runs.push((a, i - 1));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(a) = start {
        runs.push((a, s.len() - 1));
    }
    // Faint runs (peaking under 2 T) are noise, or wires too faint to judge.
    let peak = |a: usize, b: usize| s[a..=b].iter().copied().fold(f64::NEG_INFINITY, f64::max);
    runs.retain(|&(a, b)| peak(a, b) >= 2.0 * thr);
    // Join the two wires of a pair (step 3).
    let mut groups: Vec<(usize, usize)> = Vec::new();
    let mut prev_w = 0usize;
    let mut single = false;
    for (a, b) in runs {
        let w = b - a + 1;
        if let Some(last) = groups.last_mut()
            && single
        {
            let gap = a - last.1 - 1;
            let (lo, hi) = (prev_w.min(w), prev_w.max(w));
            if 2 * gap < 3 * lo && hi <= 2 * lo {
                last.1 = b;
                single = false;
                continue;
            }
        }
        groups.push((a, b));
        prev_w = w;
        single = true;
    }
    if groups.is_empty() {
        return Err(Refusal::NothingFound);
    }
    // Largest pairs first.
    let width = |g: &(usize, usize)| g.1 - g.0;
    if groups.len() > 1 && width(&groups[groups.len() - 1]) > width(&groups[0]) {
        groups.reverse();
    }
    let argmax = |a: usize, b: usize| (a..=b).fold(a, |m, i| if s[i] > s[m] { i } else { m });
    let mut pairs = Vec::with_capacity(groups.len());
    let mut first_d = None;
    for (i, &(a, b)) in groups.iter().enumerate() {
        let mid = (a + b) / 2;
        let (pa, pb) = if mid > a {
            (argmax(a, mid), argmax(mid + 1, b))
        } else {
            (a, b)
        };
        let (aa, bb) = (s[pa], s[pb]);
        let c = (pa.min(pb)..=pa.max(pb))
            .map(|k| s[k])
            .fold(f64::INFINITY, f64::min);
        let dip = if aa + bb > 0.0 {
            ((aa + bb - 2.0 * c) / (aa + bb) * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };
        let resolved = dip >= DIP_RESOLVED && pa != pb;
        if resolved && first_d.is_none() {
            let sep = pa.abs_diff(pb) as f64 * mm;
            first_d = Some((i, sep / 2.0));
        }
        pairs.push(Pair {
            number: None,
            span: (a, b),
            dip,
            resolved,
        });
    }
    // Number from the first resolved pair (or the first group's width, three diameters).
    let (at, d) = first_d.unwrap_or((0, (width(&groups[0]) + 1) as f64 * mm / 3.0));
    let n0 = nearest(&DUPLEX, d).0 as i32 - at as i32;
    for (i, p) in pairs.iter_mut().enumerate() {
        let n = n0 + i as i32;
        p.number = DUPLEX.iter().find(|(k, _)| i32::from(*k) == n).copied();
    }
    let mut first_unresolved = pairs.iter().position(|p| !p.resolved);
    // Every pair seen is resolved: the next one, too faint to see, is the first unresolved.
    if first_unresolved.is_none()
        && let Some(last) = pairs.last().copied()
        && let Some(next) = last
            .number
            .and_then(|(n, _)| DUPLEX.iter().find(|(k, _)| *k == n + 1))
    {
        first_unresolved = Some(pairs.len());
        pairs.push(Pair {
            number: Some(*next),
            span: (last.span.1, last.span.1),
            dip: 0.0,
            resolved: false,
        });
    }
    let isrb = first_unresolved.and_then(|fu| interpolated_srb(&pairs, fu, mm));
    Ok(Duplex {
        pairs,
        first_unresolved,
        isrb,
    })
}

/// iSRb (mm) from the dips around the first unresolved pair `fu` (step 7); `mm` is the
/// sample spacing.
fn interpolated_srb(pairs: &[Pair], fu: usize, mm: f64) -> Option<f64> {
    let point = |p: &Pair| p.number.map(|(_, d)| (d, p.dip));
    let above: Vec<(f64, f64)> = pairs[..fu].iter().rev().take(2).filter_map(point).collect();
    let &(x_hi, y_hi) = above.first()?;
    let mut below: Vec<(f64, f64)> = pairs[fu..]
        .iter()
        .take(2)
        .filter(|p| !p.resolved && p.dip > 0.0)
        .filter_map(point)
        .collect();
    if below.is_empty() {
        below.extend(point(&pairs[fu]));
    }
    let (x_lo, y_lo) = *pairs.get(fu).and_then(point).as_ref()?;
    if x_lo >= x_hi {
        return None;
    }
    let linear = || {
        if y_hi > y_lo {
            x_lo + (x_hi - x_lo) * (DIP_RESOLVED - y_lo) / (y_hi - y_lo)
        } else {
            x_lo
        }
    };
    let pts: Vec<(f64, f64)> = above.iter().chain(&below).copied().collect();
    let x = if pts.len() >= 3 {
        quadratic_fit(&pts, x_hi)
            .and_then(|[a, b, c]| {
                // Roots of a·u² + b·u + (c − 20) with u = x / x_hi, inside [x_lo, x_hi].
                let c = c - DIP_RESOLVED;
                let roots: Vec<f64> = if a.abs() < 1e-12 {
                    if b == 0.0 { vec![] } else { vec![-c / b] }
                } else {
                    let disc = b * b - 4.0 * a * c;
                    if disc < 0.0 {
                        vec![]
                    } else {
                        let r = disc.sqrt();
                        vec![(-b - r) / (2.0 * a), (-b + r) / (2.0 * a)]
                    }
                };
                let tol = 1e-9 * x_hi;
                roots
                    .into_iter()
                    .map(|u| u * x_hi)
                    .find(|x| *x >= x_lo - tol && *x <= x_hi + tol)
            })
            .unwrap_or_else(linear)
    } else {
        linear()
    };
    Some(x.clamp(x_lo, x_hi).max(mm))
}

/// Least-squares `y = a·u² + b·u + c` with `u = x / scale`, as `[a, b, c]`.
fn quadratic_fit(pts: &[(f64, f64)], scale: f64) -> Option<[f64; 3]> {
    // Normal equations, solved by Gaussian elimination with partial pivoting.
    let mut m = [[0.0f64; 4]; 3];
    for &(x, y) in pts {
        let u = x / scale;
        let row = [u * u, u, 1.0];
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] += row[i] * row[j];
            }
            m[i][3] += row[i] * y;
        }
    }
    for col in 0..3 {
        let piv = (col..3).max_by(|&a, &b| m[a][col].abs().total_cmp(&m[b][col].abs()))?;
        if m[piv][col].abs() < 1e-12 {
            return None;
        }
        m.swap(col, piv);
        for r in 0..3 {
            if r != col {
                let f = m[r][col] / m[col][col];
                let pivot = m[col];
                for (x, p) in m[r].iter_mut().zip(pivot).skip(col) {
                    *x -= f * p;
                }
            }
        }
    }
    Some([m[0][3] / m[0][0], m[1][3] / m[1][1], m[2][3] / m[2][2]])
}

/// The table entry nearest `d` (mm), by ratio.
pub fn nearest(table: &[(u8, f64)], d: f64) -> (u8, f64) {
    let mut best = table[0];
    for e in table {
        if (e.1 / d).ln().abs() < (best.1 / d).ln().abs() {
            best = *e;
        }
    }
    best
}

// ----- Single-wire IQI -------------------------------------------------------------------

/// A single wire across a profile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wire {
    /// The full width at half maximum (mm).
    pub fwhm: f64,
    /// The diameter that gives that width (a round wire's FWHM is √3/2 of it), mm.
    pub diameter: f64,
    /// The nearest wire of the table: its number and nominal diameter (mm).
    pub number: u8,
    pub nominal: f64,
}

/// The wire across profile `v` (mm per sample), the profile averaged over 5 samples first.
pub fn wire(v: &[f64], mm: f64) -> Result<Wire, Refusal> {
    if v.len() < 5 {
        return Err(Refusal::NothingFound);
    }
    let (s, _) = signal(v, 5);
    let peak = (0..s.len()).fold(0, |m, i| if s[i] > s[m] { i } else { m });
    let (l, r) = fwhm_at(&s, peak, 0.0).ok_or(Refusal::NothingFound)?;
    let fwhm = (r - l) * mm;
    let diameter = fwhm / (3f64.sqrt() / 2.0);
    let (number, nominal) = nearest(&WIRES, diameter);
    Ok(Wire {
        fwhm,
        diameter,
        number,
        nominal,
    })
}

// ----- Crack width -----------------------------------------------------------------------

/// One width across a crack.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrackCut {
    /// Where on the crack line (stored pixels).
    pub at: P,
    /// The crack's edges on the cut (stored pixels).
    pub edges: (P, P),
    /// Width in pixels.
    pub px: f64,
}

/// A crack reading: widths at points along the line, in pixels and calibrated.
#[derive(Clone, Debug, PartialEq)]
pub struct Crack {
    pub cuts: Vec<CrackCut>,
    /// Calibrated (min, mean, max) in the calibration's unit, over the cuts found.
    pub widths: (f64, f64, f64),
}

/// Samples per pixel across a crack.
const CUT_STEP: f64 = 0.25;

/// The crack's width across `line` at `n` points spaced evenly along it (at the middles of
/// `n` equal parts), each cut `half` pixels to either side, edges at half maximum of the
/// dark profile (background: the median of the cut's outer quarters).
pub fn crack(
    g: &Grey,
    line: &[P],
    n: usize,
    half: f64,
    cal: &Calibration,
) -> Result<Crack, Refusal> {
    let segs: Vec<(P, P, f64)> = line
        .windows(2)
        .map(|w| (w[0], w[1], (w[1] - w[0]).hypot()))
        .filter(|s| s.2 > 0.0)
        .collect();
    let total: f64 = segs.iter().map(|s| s.2).sum();
    if total <= 0.0 || n == 0 {
        return Err(Refusal::NothingFound);
    }
    let steps = (half / CUT_STEP).round() as i64;
    let mut cuts = Vec::new();
    let mut cal_w = Vec::new();
    for k in 0..n {
        let mut at = total * (k as f64 + 0.5) / n as f64;
        let Some(&(a, b, len)) = segs.iter().find(|s| {
            let found = at <= s.2;
            if !found {
                at -= s.2;
            }
            found
        }) else {
            continue;
        };
        let t = at / len;
        let c = P::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
        let (nx, ny) = (-(b.y - a.y) / len, (b.x - a.x) / len);
        let cut: Vec<f64> = (-steps..=steps)
            .map(|i| {
                let o = i as f64 * CUT_STEP;
                g.sample(P::new(c.x + nx * o, c.y + ny * o))
            })
            .collect();
        let m = cut.len();
        let q = m / 4;
        let outer: Vec<f64> = cut[..q].iter().chain(&cut[m - q..]).copied().collect();
        let bg = median(&outer);
        // The darkest sample in the middle half.
        let lo = (q..m - q).fold(q, |best, i| if cut[i] < cut[best] { i } else { best });
        let s: Vec<f64> = cut.iter().map(|v| bg - v).collect();
        let Some((l, r)) = fwhm_at(&s, lo, 0.0) else {
            continue;
        };
        let off = |i: f64| (i - steps as f64) * CUT_STEP;
        let (ol, or) = (off(l), off(r));
        let edges = (
            P::new(c.x + nx * ol, c.y + ny * ol),
            P::new(c.x + nx * or, c.y + ny * or),
        );
        cal_w.push(cal.distance(edges.0, edges.1));
        cuts.push(CrackCut {
            at: c,
            edges,
            px: or - ol,
        });
    }
    if cuts.is_empty() {
        return Err(Refusal::NothingFound);
    }
    let min = cal_w.iter().copied().fold(f64::INFINITY, f64::min);
    let max = cal_w.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mean = cal_w.iter().sum::<f64>() / cal_w.len() as f64;
    Ok(Crack {
        cuts,
        widths: (min, mean, max),
    })
}

// ----- Readings --------------------------------------------------------------------------

/// What an inspection measurement found.
#[derive(Clone, Debug, PartialEq)]
pub enum Reading {
    Profile {
        values: Vec<f64>,
        /// Pixels between samples.
        step: f64,
        /// The densities of `values`, when the image has a density calibration.
        density: Option<Vec<f64>>,
    },
    Snr(Snr),
    Duplex(Duplex),
    Wire(Wire),
    Crack(Crack),
    Density {
        grey: f64,
        density: Option<f64>,
    },
    Refused(Refusal),
}

/// One exported value of a reading.
#[derive(Clone, Debug, PartialEq)]
pub struct Readout {
    pub key: String,
    pub value: f64,
    /// `grey`, `OD`, `ratio`, `percent`, `mm`, `um`, `number`, `px`, or the calibration's
    /// length unit key.
    pub unit: String,
    /// Where on the image, when the value belongs to a point.
    pub at: Option<P>,
}

impl Reading {
    /// Its values for CSV, in order.
    pub fn readouts(&self, a: P, b: P, unit: &str) -> Vec<Readout> {
        let r = |key: &str, value: f64, unit: &str| Readout {
            key: key.to_owned(),
            value,
            unit: unit.to_owned(),
            at: None,
        };
        match self {
            Reading::Profile {
                values,
                step,
                density,
            } => {
                let len = (b - a).hypot();
                let mut out = Vec::new();
                for (k, v) in values.iter().enumerate() {
                    let t = if len > 0.0 {
                        k as f64 * step / len
                    } else {
                        0.0
                    };
                    let at = Some(P::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t));
                    out.push(Readout {
                        at,
                        ..r(&format!("grey-{}", k + 1), *v, "grey")
                    });
                    if let Some(d) = density.as_ref().and_then(|d| d.get(k)) {
                        out.push(Readout {
                            at,
                            ..r(&format!("density-{}", k + 1), *d, "OD")
                        });
                    }
                }
                out
            }
            Reading::Snr(s) => {
                let mut out = vec![
                    r("mean", s.mean, "grey"),
                    r("sd", s.sd, "grey"),
                    r("pixels", s.count as f64, "number"),
                    r("snr", s.snr, "ratio"),
                ];
                if let Some((srb, n)) = s.normalised {
                    out.push(r("srb", srb, "um"));
                    out.push(r("snrn", n, "ratio"));
                }
                out
            }
            Reading::Duplex(d) => {
                let mut out = Vec::new();
                for p in &d.pairs {
                    let name = p
                        .number
                        .map_or_else(|| "pair".to_owned(), |(n, _)| format!("D{n}"));
                    out.push(r(&format!("{name}-dip"), p.dip, "percent"));
                }
                if let Some((n, dia)) = d.srb() {
                    out.push(r("first-unresolved", f64::from(n), "number"));
                    out.push(r("unsharpness", 2.0 * dia, "mm"));
                    out.push(r("srb", dia * 1000.0, "um"));
                }
                if let Some(i) = d.isrb {
                    out.push(r("isrb", i * 1000.0, "um"));
                    out.push(r("iunsharpness", 2.0 * i, "mm"));
                }
                out
            }
            Reading::Wire(w) => vec![
                r("wire", f64::from(w.number), "number"),
                r("nominal", w.nominal, "mm"),
                r("fwhm", w.fwhm, "mm"),
                r("diameter", w.diameter, "mm"),
            ],
            Reading::Crack(c) => {
                let mut out = vec![
                    r("width-min", c.widths.0, unit),
                    r("width-mean", c.widths.1, unit),
                    r("width-max", c.widths.2, unit),
                ];
                for (k, cut) in c.cuts.iter().enumerate() {
                    out.push(Readout {
                        at: Some(cut.at),
                        ..r(&format!("width-{}", k + 1), cut.px, "px")
                    });
                }
                out
            }
            Reading::Density { grey, density } => {
                let mut out = vec![r("grey", *grey, "grey")];
                if let Some(d) = density {
                    out.push(r("density", *d, "OD"));
                }
                out
            }
            Reading::Refused(_) => Vec::new(),
        }
    }
}

#[cfg(test)]
#[path = "inspect_tests.rs"]
mod tests;
