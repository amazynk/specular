//! Alignment geometry: element lengths, points along lines, arcs and clothoids (the heading's
//! integral computed with Gauss–Legendre quadrature to well below a micrometre), stationing,
//! and design profile elevations with their vertical curves.

use std::f64::consts::{PI, TAU};

use crate::{Alignment, Geom, Pvi};

/// The centre of the arc from `a` to `b` of radius `r` turning left (`ccw`) or right (the
/// minor arc).
pub fn arc_center(a: [f64; 2], b: [f64; 2], r: f64, ccw: bool) -> Option<[f64; 2]> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let d = dx.hypot(dy);
    if d <= 0.0 || r.abs() < d / 2.0 {
        return None;
    }
    let h = (r * r - d * d / 4.0).sqrt();
    let (mx, my) = ((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0);
    let (nx, ny) = (-dy / d, dx / d);
    let s = if ccw { 1.0 } else { -1.0 };
    Some([mx + s * h * nx, my + s * h * ny])
}

/// The arc's signed sweep (radians, positive counter-clockwise; a full turn when the ends
/// meet).
pub fn arc_sweep(start: [f64; 2], end: [f64; 2], center: [f64; 2], ccw: bool) -> f64 {
    let a0 = (start[1] - center[1]).atan2(start[0] - center[0]);
    let a1 = (end[1] - center[1]).atan2(end[0] - center[0]);
    let mut s = a1 - a0;
    if ccw {
        while s <= 0.0 {
            s += TAU;
        }
    } else {
        while s >= 0.0 {
            s -= TAU;
        }
    }
    if (start[0] - end[0]).hypot(start[1] - end[1]) > 1e-9 && s.abs() >= TAU - 1e-12 {
        s = 0.0;
    }
    s
}

const GL_X: [f64; 5] = [
    -0.906_179_845_938_664,
    -0.538_469_310_105_683_1,
    0.0,
    0.538_469_310_105_683_1,
    0.906_179_845_938_664,
];
const GL_W: [f64; 5] = [
    0.236_926_885_056_189_1,
    0.478_628_670_499_366_5,
    0.568_888_888_888_888_9,
    0.478_628_670_499_366_5,
    0.236_926_885_056_189_1,
];

/// A clothoid's point and heading at arc length `s` from `start`: the heading is
/// `dir + k0 s + (k1 − k0) s² / 2L`.
pub fn clothoid(start: [f64; 2], dir: f64, len: f64, k0: f64, k1: f64, s: f64) -> ([f64; 2], f64) {
    let dk = if len > 0.0 { (k1 - k0) / len } else { 0.0 };
    let heading = |t: f64| dir + k0 * t + dk * t * t / 2.0;
    // Pieces short enough that the heading turns little in each.
    let turn = (k0.abs() + k1.abs()) * s.abs();
    let n =
        ((turn / 0.05).ceil() as usize).clamp(1, 20_000) + ((s.abs() / 50.0) as usize).min(1000);
    let h = s / n as f64;
    let (mut x, mut y) = (start[0], start[1]);
    for i in 0..n {
        let a = h * i as f64;
        for (xi, wi) in GL_X.iter().zip(GL_W) {
            let t = a + h * (xi + 1.0) / 2.0;
            let th = heading(t);
            x += wi * th.cos() * h / 2.0;
            y += wi * th.sin() * h / 2.0;
        }
    }
    ([x, y], heading(s))
}

impl Geom {
    /// Length along the element.
    pub fn length(&self) -> f64 {
        match *self {
            Geom::Line { start, end } => (end[0] - start[0]).hypot(end[1] - start[1]),
            Geom::Arc {
                start,
                end,
                center,
                ccw,
            } => {
                let r = (start[0] - center[0]).hypot(start[1] - center[1]);
                r * arc_sweep(start, end, center, ccw).abs()
            }
            Geom::Spiral { length, .. } => length,
        }
    }

    /// The point and heading at distance `s` along the element.
    pub fn at(&self, s: f64) -> ([f64; 2], f64) {
        match *self {
            Geom::Line { start, end } => {
                let len = self.length().max(1e-12);
                let t = s / len;
                (
                    [
                        start[0] + (end[0] - start[0]) * t,
                        start[1] + (end[1] - start[1]) * t,
                    ],
                    (end[1] - start[1]).atan2(end[0] - start[0]),
                )
            }
            Geom::Arc {
                start,
                end,
                center,
                ccw,
            } => {
                let r = (start[0] - center[0])
                    .hypot(start[1] - center[1])
                    .max(1e-12);
                let a0 = (start[1] - center[1]).atan2(start[0] - center[0]);
                let sweep = arc_sweep(start, end, center, ccw);
                let a = a0 + sweep.signum() * s / r;
                (
                    [center[0] + r * a.cos(), center[1] + r * a.sin()],
                    a + sweep.signum() * PI / 2.0,
                )
            }
            Geom::Spiral {
                start,
                dir,
                length,
                k0,
                k1,
                ..
            } => clothoid(start, dir, length, k0, k1, s),
        }
    }

    /// Where the element ends as computed, and where the file says it ends.
    pub fn ends(&self) -> ([f64; 2], Option<[f64; 2]>) {
        let computed = self.at(self.length()).0;
        let stated = match *self {
            Geom::Line { end, .. } | Geom::Arc { end, .. } => Some(end),
            Geom::Spiral { end, .. } => end,
        };
        (computed, stated)
    }
}

impl Alignment {
    /// Computed length of the elements.
    pub fn computed_length(&self) -> f64 {
        self.elements.iter().map(Geom::length).sum()
    }

    /// Points along the alignment at most `step` apart (and at every element end), with
    /// their stations.
    pub fn sample(&self, step: f64) -> Vec<(f64, [f64; 2])> {
        let mut out = Vec::new();
        let mut sta = self.sta_start;
        for e in &self.elements {
            let len = e.length();
            let curved = !matches!(e, Geom::Line { .. });
            let n = if curved {
                ((len / step.max(1e-9)).ceil() as usize).clamp(1, 100_000)
            } else {
                1
            };
            for i in 0..=n {
                if i == 0 && !out.is_empty() {
                    continue;
                }
                let s = len * i as f64 / n as f64;
                out.push((sta + s, e.at(s).0));
            }
            sta += len;
        }
        out
    }

    /// The point and heading at `station`.
    pub fn at_station(&self, station: f64) -> Option<([f64; 2], f64)> {
        let mut s = station - self.sta_start;
        if s < -1e-9 {
            return None;
        }
        for e in &self.elements {
            let len = e.length();
            if s <= len + 1e-9 {
                return Some(e.at(s.min(len)));
            }
            s -= len;
        }
        None
    }
}

/// A station as surveyors write it: `1+234.50` metric (thousands), `12+34.50` imperial
/// (hundreds).
pub fn station_label(sta: f64, imperial: bool, decimals: usize) -> String {
    let unit = if imperial { 100.0 } else { 1000.0 };
    let neg = sta < 0.0;
    let a = sta.abs();
    let mut major = (a / unit).floor();
    let mut minor = a - major * unit;
    // Rounding up to the next whole station.
    let p = 10f64.powi(decimals as i32);
    if (minor * p).round() / p >= unit {
        major += 1.0;
        minor = 0.0;
    }
    let width = if imperial { 2 } else { 3 } + if decimals > 0 { decimals + 1 } else { 0 };
    format!(
        "{}{}+{:0width$.decimals$}",
        if neg { "-" } else { "" },
        major as i64,
        minor,
        width = width,
        decimals = decimals
    )
}

/// The elevation of a design profile at `station`: straight grades between PVIs, a
/// parabola over each vertical curve's length (circular curves are drawn as the parabola
/// of the same length, within millimetres for road radii).
pub fn profile_elevation(pvis: &[Pvi], station: f64) -> Option<f64> {
    if pvis.is_empty() {
        return None;
    }
    let first = &pvis[0];
    let last = &pvis[pvis.len() - 1];
    if station < first.station - 1e-9 || station > last.station + 1e-9 {
        return None;
    }
    let grade = |a: &Pvi, b: &Pvi| {
        let d = b.station - a.station;
        if d.abs() < 1e-12 {
            0.0
        } else {
            (b.elevation - a.elevation) / d
        }
    };
    // A vertical curve around an inner PVI.
    for i in 1..pvis.len().saturating_sub(1) {
        let p = &pvis[i];
        let Some(l) = p.curve.filter(|l| *l > 0.0) else {
            continue;
        };
        let (g1, g2) = (grade(&pvis[i - 1], p), grade(p, &pvis[i + 1]));
        let bvc = p.station - l / 2.0;
        if station >= bvc && station <= p.station + l / 2.0 {
            let x = station - bvc;
            let e_bvc = p.elevation - g1 * l / 2.0;
            return Some(e_bvc + g1 * x + (g2 - g1) / (2.0 * l) * x * x);
        }
    }
    for w in pvis.windows(2) {
        if station >= w[0].station - 1e-9 && station <= w[1].station + 1e-9 {
            return Some(w[0].elevation + grade(&w[0], &w[1]) * (station - w[0].station));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clothoid_matches_fresnel_series() {
        // A clothoid from straight to radius R over L: with A² = R L, the end point is
        // x = L (1 − L²/(40 A⁴/L²)...) — compare with the Fresnel series to 1e-9.
        let (r, l) = (200.0, 80.0);
        let (p, h) = clothoid([0.0, 0.0], 0.0, l, 0.0, 1.0 / r, l);
        let a2: f64 = r * l;
        // Series: x = s − s⁵/(40 A⁴) + s⁹/(3456 A⁸), y = s³/(6A²) − s⁷/(336 A⁶) + s¹¹/(42240 A¹⁰).
        let s = l;
        let x = s - s.powi(5) / (40.0 * a2.powi(2)) + s.powi(9) / (3456.0 * a2.powi(4))
            - s.powi(13) / (599_040.0 * a2.powi(6));
        let y = s.powi(3) / (6.0 * a2) - s.powi(7) / (336.0 * a2.powi(3))
            + s.powi(11) / (42_240.0 * a2.powi(5))
            - s.powi(15) / (9_676_800.0 * a2.powi(7));
        assert!(
            (p[0] - x).abs() < 1e-9 && (p[1] - y).abs() < 1e-9,
            "{p:?} {x} {y}"
        );
        assert!((h - l / (2.0 * r)).abs() < 1e-12);
        // Turning right mirrors it.
        let (q, _) = clothoid([0.0, 0.0], 0.0, l, 0.0, -1.0 / r, l);
        assert!((q[1] + y).abs() < 1e-9);
    }

    #[test]
    fn arcs_and_stations() {
        let c = arc_center([0.0, 0.0], [10.0, 10.0], 10.0, true).unwrap();
        assert!(
            (c[0] - 0.0).abs() < 1e-9 && (c[1] - 10.0).abs() < 1e-9,
            "{c:?}"
        );
        let g = Geom::Arc {
            start: [0.0, 0.0],
            end: [10.0, 10.0],
            center: c,
            ccw: true,
        };
        assert!((g.length() - 10.0 * PI / 2.0).abs() < 1e-9);
        assert_eq!(station_label(1234.5, false, 2), "1+234.50");
        assert_eq!(station_label(1234.6, true, 0), "12+35");
        assert_eq!(station_label(999.999, false, 2), "1+000.00");
        assert_eq!(station_label(-50.0, false, 0), "-0+050");
    }

    #[test]
    fn vertical_curves() {
        let p = |s: f64, e: f64, l: Option<f64>| Pvi {
            station: s,
            elevation: e,
            curve: l,
            circular_radius: None,
        };
        let pvis = [
            p(0.0, 100.0, None),
            p(100.0, 102.0, Some(40.0)),
            p(200.0, 100.0, None),
        ];
        assert_eq!(profile_elevation(&pvis, 50.0), Some(101.0));
        // The curve's middle is (g2 − g1) L / 8 below the PVI.
        let mid = profile_elevation(&pvis, 100.0).unwrap();
        assert!((mid - (102.0 - 0.04 * 40.0 / 8.0)).abs() < 1e-9, "{mid}");
        assert_eq!(profile_elevation(&pvis, 250.0), None);
    }
}
