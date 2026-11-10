//! Suggesting frames to reject: those whose metric lies more than k standard deviations from
//! the median of the set, on the bad side (a larger HFD, FWHM, eccentricity, background or
//! noise; fewer stars or a lower SNR). The standard deviation is robust (1.4826 × MAD), so the
//! bad frames themselves do not widen it. Nothing is rejected here: the user confirms.

use crate::background::{MAD_SIGMA, median};

/// A per-frame metric.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Metric {
    Hfd,
    Fwhm,
    Eccentricity,
    Stars,
    Background,
    Noise,
    Snr,
}

impl Metric {
    pub const ALL: [Metric; 7] = [
        Metric::Hfd,
        Metric::Fwhm,
        Metric::Eccentricity,
        Metric::Stars,
        Metric::Background,
        Metric::Noise,
        Metric::Snr,
    ];

    /// Whether a larger value is worse.
    pub fn higher_is_worse(self) -> bool {
        !matches!(self, Metric::Stars | Metric::Snr)
    }

    /// The short name used in queries and settings (`hfd`, `fwhm`, `ecc`, `stars`, `bg`,
    /// `noise`, `snr`).
    pub fn id(self) -> &'static str {
        match self {
            Metric::Hfd => "hfd",
            Metric::Fwhm => "fwhm",
            Metric::Eccentricity => "ecc",
            Metric::Stars => "stars",
            Metric::Background => "bg",
            Metric::Noise => "noise",
            Metric::Snr => "snr",
        }
    }

    pub fn from_id(s: &str) -> Option<Metric> {
        Metric::ALL
            .into_iter()
            .find(|m| m.id().eq_ignore_ascii_case(s))
    }
}

/// What [`suggest`] found.
#[derive(Clone, Debug, PartialEq)]
pub struct Suggestion {
    pub metric: Metric,
    pub k: f32,
    pub median: f32,
    /// The robust standard deviation.
    pub sigma: f32,
    /// Frames past this value are suggested (above it when larger is worse, else below).
    pub threshold: f32,
    /// Indices into the values given, in order.
    pub frames: Vec<usize>,
}

impl Suggestion {
    /// Whether `v` lies past the threshold.
    pub fn is_out(&self, v: f32) -> bool {
        if self.metric.higher_is_worse() {
            v > self.threshold
        } else {
            v < self.threshold
        }
    }
}

/// Frames whose `metric` (one value per frame, `None` where not measured) lies more than `k`
/// robust standard deviations from the median on its bad side. `None` with fewer than three
/// measured frames. Frames without a value are never suggested.
pub fn suggest(values: &[Option<f32>], metric: Metric, k: f32) -> Option<Suggestion> {
    let mut v: Vec<f32> = values
        .iter()
        .flatten()
        .copied()
        .filter(|v| v.is_finite())
        .collect();
    if v.len() < 3 {
        return None;
    }
    let med = median(&mut v);
    let mut dev: Vec<f32> = v.iter().map(|x| (x - med).abs()).collect();
    let mut sigma = median(&mut dev) * MAD_SIGMA;
    // Half the frames identical: fall back to the mean deviation, so a lone outlier still
    // stands out.
    if sigma <= 0.0 {
        sigma = dev.iter().sum::<f32>() / dev.len() as f32;
    }
    let threshold = if metric.higher_is_worse() {
        med + k * sigma
    } else {
        med - k * sigma
    };
    let mut s = Suggestion {
        metric,
        k,
        median: med,
        sigma,
        threshold,
        frames: Vec::new(),
    };
    if sigma > 0.0 {
        s.frames = values
            .iter()
            .enumerate()
            .filter(|(_, v)| v.is_some_and(|v| v.is_finite() && s.is_out(v)))
            .map(|(i, _)| i)
            .collect();
    }
    Some(s)
}
