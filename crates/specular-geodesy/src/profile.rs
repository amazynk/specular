//! An elevation profile along a path: heights sampled at even geodesic distances (and at
//! every vertex), with the path's length, lowest and highest point, and total climb and
//! descent.

use crate::geodesic::{direct, inverse};

/// One sample of a profile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    /// Along the path from its start, metres.
    pub distance: f64,
    pub lat: f64,
    pub lon: f64,
    /// `None` where the height source has no value.
    pub height: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Profile {
    pub samples: Vec<Sample>,
    /// The path's length, metres.
    pub length: f64,
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// Sums of the rises and falls between consecutive known heights, metres (≥ 0).
    pub climb: f64,
    pub descent: f64,
}

/// The profile of a path of (latitude, longitude) vertices over `height(lat, lon)`, with
/// `samples` evenly spaced points (at least two) plus the vertices.
pub fn profile(
    path: &[(f64, f64)],
    samples: usize,
    height: impl Fn(f64, f64) -> Option<f64>,
) -> Profile {
    if path.is_empty() {
        return Profile::default();
    }
    // Each segment: start distance, length, azimuth.
    let mut segs = Vec::with_capacity(path.len().saturating_sub(1));
    let mut length = 0.0;
    for w in path.windows(2) {
        let inv = inverse(w[0].0, w[0].1, w[1].0, w[1].1);
        segs.push((length, inv.s12, inv.azi1));
        length += inv.s12;
    }
    let mut at: Vec<(f64, f64, f64)> = Vec::new();
    // The vertices.
    at.push((0.0, path[0].0, path[0].1));
    for (i, s) in segs.iter().enumerate() {
        let (la, lo) = path[i + 1];
        at.push((s.0 + s.1, la, lo));
    }
    // Even samples.
    let n = samples.max(2);
    if length > 0.0 {
        let mut k = 0;
        for i in 1..n - 1 {
            let d = length * i as f64 / (n - 1) as f64;
            while k + 1 < segs.len() && segs[k].0 + segs[k].1 < d {
                k += 1;
            }
            let (start, _, azi) = segs[k];
            let (la0, lo0) = path[k];
            let (la, lo, _) = direct(la0, lo0, azi, d - start);
            at.push((d, la, lo));
        }
    }
    at.sort_by(|a, b| a.0.total_cmp(&b.0));
    at.dedup_by(|b, a| (b.0 - a.0).abs() < 1e-9);
    let samples: Vec<Sample> = at
        .into_iter()
        .map(|(distance, lat, lon)| Sample {
            distance,
            lat,
            lon,
            height: height(lat, lon).filter(|h| h.is_finite()),
        })
        .collect();
    let known: Vec<f64> = samples.iter().filter_map(|s| s.height).collect();
    let min = known.iter().copied().reduce(f64::min);
    let max = known.iter().copied().reduce(f64::max);
    let (mut climb, mut descent) = (0.0, 0.0);
    for w in known.windows(2) {
        let d = w[1] - w[0];
        if d > 0.0 {
            climb += d;
        } else {
            descent -= d;
        }
    }
    Profile {
        samples,
        length,
        min,
        max,
        climb,
        descent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ramp_and_plane() {
        // Heights rising 100 m per degree of longitude along the equator.
        let p = profile(&[(0.0, 0.0), (0.0, 2.0)], 21, |_, lon| Some(100.0 * lon));
        assert_eq!(p.samples.len(), 21);
        assert!((p.min.unwrap()).abs() < 1e-9);
        assert!((p.max.unwrap() - 200.0).abs() < 1e-9);
        assert!((p.climb - 200.0).abs() < 1e-9 && p.descent == 0.0);
        assert!((p.samples[10].lon - 1.0).abs() < 1e-9);
        assert!((p.length - 2.0 * crate::WGS84_A * 1f64.to_radians()).abs() < 1e-6);
        // Out and back over a flat plane at 50 m: no climb.
        let p = profile(&[(10.0, 10.0), (10.5, 10.5), (10.0, 10.0)], 50, |_, _| {
            Some(50.0)
        });
        assert_eq!((p.min, p.max), (Some(50.0), Some(50.0)));
        assert_eq!((p.climb, p.descent), (0.0, 0.0));
        // The middle vertex is a sample.
        assert!(p.samples.iter().any(|s| (s.lat - 10.5).abs() < 1e-12));
        // Up and down a ridge: climb and descent both 100 m; no data off the ridge's west.
        let p = profile(&[(0.0, -1.0), (0.0, 1.0)], 41, |_, lon| {
            (lon > -0.51).then(|| 100.0 - 100.0 * lon.abs())
        });
        assert!(p.samples.iter().any(|s| s.height.is_none()));
        assert!((p.max.unwrap() - 100.0).abs() < 1e-6);
        assert!((p.descent - 100.0).abs() < 1e-6, "{}", p.descent);
        assert!((p.climb - 50.0).abs() < 1e-6, "{}", p.climb);
    }
}
