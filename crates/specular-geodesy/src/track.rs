//! GPS tracks in time: where the track was at a moment (along the geodesic between the two
//! fixes around it), and photos placed on it by their capture times with a camera clock
//! offset.

use crate::geodesic;

/// One GPS fix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fix {
    /// Seconds since 1970-01-01 UTC.
    pub t: f64,
    pub lat: f64,
    pub lon: f64,
    /// Metres above the ellipsoid or sea level, as the file gives it.
    pub ele: Option<f64>,
}

/// A track's fixes in time order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Timeline {
    pub fixes: Vec<Fix>,
}

impl Timeline {
    /// The fixes sorted by time (fixes without a finite time or position dropped).
    pub fn new(mut fixes: Vec<Fix>) -> Timeline {
        fixes.retain(|f| f.t.is_finite() && f.lat.is_finite() && f.lon.is_finite());
        fixes.sort_by(|a, b| a.t.total_cmp(&b.t));
        Timeline { fixes }
    }

    /// The first and last time.
    pub fn span(&self) -> Option<(f64, f64)> {
        Some((self.fixes.first()?.t, self.fixes.last()?.t))
    }

    /// The fix before `t` and the one at or after it.
    fn around(&self, t: f64) -> Option<(usize, usize)> {
        let (t0, t1) = self.span()?;
        if t < t0 || t > t1 {
            return None;
        }
        let i = self.fixes.partition_point(|f| f.t < t);
        if i == 0 {
            return Some((0, 0));
        }
        Some((i - 1, i.min(self.fixes.len() - 1)))
    }

    /// Where the track was at `t` (lat, lon), and how far in time the nearer fix is.
    pub fn at(&self, t: f64) -> Option<((f64, f64), f64)> {
        let (a, b) = self.around(t)?;
        let (fa, fb) = (self.fixes[a], self.fixes[b]);
        if a == b || fb.t <= fa.t {
            return Some(((fb.lat, fb.lon), (t - fb.t).abs()));
        }
        let k = (t - fa.t) / (fb.t - fa.t);
        let p = geodesic::interpolate(fa.lat, fa.lon, fb.lat, fb.lon, k);
        Some((p, (t - fa.t).min(fb.t - t)))
    }

    /// The index of the fix nearest in time to `t`.
    pub fn nearest(&self, t: f64) -> Option<usize> {
        if self.fixes.is_empty() {
            return None;
        }
        let i = self.fixes.partition_point(|f| f.t < t);
        let best = match i {
            0 => 0,
            n if n >= self.fixes.len() => self.fixes.len() - 1,
            n if (self.fixes[n].t - t) < (t - self.fixes[n - 1].t) => n,
            n => n - 1,
        };
        Some(best)
    }

    /// The distance along the track up to `t`, metres.
    pub fn distance_to(&self, t: f64) -> f64 {
        let mut s = 0.0;
        for w in self.fixes.windows(2) {
            if w[1].t <= t {
                s += geodesic::inverse(w[0].lat, w[0].lon, w[1].lat, w[1].lon).s12;
            } else {
                if w[0].t < t
                    && let Some(((lat, lon), _)) = self.at(t)
                {
                    s += geodesic::inverse(w[0].lat, w[0].lon, lat, lon).s12;
                }
                break;
            }
        }
        s
    }
}

/// Where a photo goes on a track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placement {
    /// On the track at (lat, lon); `gap`: seconds to the nearer fix.
    On { lat: f64, lon: f64, gap: f64 },
    /// Taken before the track started or after it ended (by this many seconds).
    Outside(f64),
    /// Between two fixes further apart than allowed (a gap in the recording).
    Gap(f64),
}

/// Places photos taken at `times` (the camera's clock, seconds) on `track`, the camera's
/// clock being `offset` seconds ahead of GPS time (so GPS time = camera time − offset).
/// Photos within `max_gap` seconds of a fix are placed; others are reported.
pub fn place(track: &Timeline, times: &[f64], offset: f64, max_gap: f64) -> Vec<Placement> {
    let span = track.span();
    times
        .iter()
        .map(|&ct| {
            let t = ct - offset;
            match (span, track.at(t)) {
                (_, Some(((lat, lon), gap))) if gap <= max_gap => Placement::On { lat, lon, gap },
                (_, Some((_, gap))) => Placement::Gap(gap),
                (Some((t0, _)), None) if t < t0 => Placement::Outside(t - t0),
                (Some((_, t1)), None) => Placement::Outside(t - t1),
                (None, _) => Placement::Outside(f64::INFINITY),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walk() -> Timeline {
        // North along a meridian, 0.001° a minute, with a 10-minute gap.
        let mut v = Vec::new();
        for i in 0..10 {
            v.push(Fix {
                t: 1000.0 + 60.0 * f64::from(i),
                lat: 47.0 + 0.001 * f64::from(i),
                lon: 8.0,
                ele: None,
            });
        }
        v.push(Fix {
            t: 1000.0 + 60.0 * 19.0,
            lat: 47.019,
            lon: 8.0,
            ele: Some(400.0),
        });
        v.reverse();
        Timeline::new(v)
    }

    #[test]
    fn positions_between_fixes() {
        let w = walk();
        assert_eq!(w.span(), Some((1000.0, 2140.0)));
        let ((lat, lon), gap) = w.at(1030.0).unwrap();
        assert!(
            (lat - 47.0005).abs() < 1e-7 && (lon - 8.0).abs() < 1e-9,
            "{lat}"
        );
        assert_eq!(gap, 30.0);
        assert!(w.at(999.0).is_none());
        assert_eq!(w.nearest(1089.0), Some(1));
        let d = w.distance_to(1120.0);
        assert!((d - 222.4).abs() < 1.0, "{d}");
    }

    #[test]
    fn photos_with_an_offset() {
        let w = walk();
        // The camera runs 1 h fast.
        let times = [
            1030.0 + 3600.0,
            900.0 + 3600.0,
            1840.0 + 3600.0,
            2140.0 + 3600.0,
        ];
        let p = place(&w, &times, 3600.0, 120.0);
        assert!(matches!(p[0], Placement::On { gap, .. } if gap == 30.0));
        assert_eq!(p[1], Placement::Outside(-100.0));
        assert!(matches!(p[2], Placement::Gap(g) if g > 120.0));
        assert!(matches!(p[3], Placement::On { gap, .. } if gap == 0.0));
    }
}
