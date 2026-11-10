//! Bracket detection: which raws were shot as one exposure bracket.
//!
//! A bracket is a run of frames in capture order from one camera, close in time, whose
//! exposures differ. The run breaks where the time gap grows past what the exposures explain,
//! where the camera, aperture or focal length changes, or where an exposure repeats (the next
//! bracket of a series shot with the same steps). Exif `ExposureMode` 2 (auto bracket)
//! marks a frame as bracketed for sure; without it a run still counts when its exposures span
//! at least 2/3 EV.

use std::path::PathBuf;

/// What detection and merging know of a frame (from its metadata).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shot {
    pub path: PathBuf,
    pub make: String,
    pub model: String,
    /// Capture time in seconds (any epoch; sub-seconds when the file has them).
    pub taken: Option<f64>,
    /// Seconds.
    pub exposure_time: Option<f64>,
    pub f_number: Option<f64>,
    pub iso: Option<f64>,
    /// Exposure compensation, EV.
    pub bias: Option<f64>,
    pub focal_length: Option<f64>,
    /// Exif ExposureMode 2: the camera bracketed this frame itself.
    pub auto_bracket: bool,
    /// Focus distance in metres (Exif SubjectDistance), when the file says.
    pub focus_distance: Option<f64>,
}

impl Shot {
    /// The exposure the sensor saw, up to a constant: time × ISO / f-number². `None` when
    /// the time is unknown.
    pub fn exposure(&self) -> Option<f64> {
        let t = self.exposure_time.filter(|t| *t > 0.0 && t.is_finite())?;
        let iso = self.iso.filter(|v| *v > 0.0).unwrap_or(100.0);
        let n = self.f_number.filter(|v| *v > 0.0).unwrap_or(1.0);
        Some(t * iso / (n * n))
    }

    /// [`Shot::exposure`] in EV (log2).
    pub fn ev(&self) -> Option<f64> {
        self.exposure().map(f64::log2)
    }

    /// Reads the facts of a raw file (its head is enough; `data` may be the whole file).
    pub fn of_raw(path: PathBuf, data: &[u8]) -> Option<Shot> {
        let f = specular_raw::parse(data).ok()?;
        let i = f.info();
        let mut s = Shot {
            path,
            make: i.make.clone(),
            model: i.model.clone(),
            taken: i.date_time.as_deref().and_then(parse_time),
            exposure_time: i.exposure_time,
            f_number: i.f_number,
            iso: i.iso.map(f64::from),
            bias: i.exposure_bias,
            focal_length: i.focal_length,
            auto_bracket: false,
            focus_distance: None,
        };
        // ExposureMode and sub-seconds from the Exif directory.
        if let Some(e) = specular_convert::source_meta(data).exif {
            let find = |tag: u16| e.exif.iter().find(|x| x.tag == tag);
            if let Some(m) = find(0xA402)
                && m.data.len() >= 2
            {
                s.auto_bracket = u16::from_le_bytes([m.data[0], m.data[1]]) == 2;
            }
            // SubjectDistance: a rational in metres; 0 is unknown, 0xFFFFFFFF infinity.
            if let Some(m) = find(0x9206)
                && m.data.len() >= 8
            {
                let n = u32::from_le_bytes([m.data[0], m.data[1], m.data[2], m.data[3]]);
                let d = u32::from_le_bytes([m.data[4], m.data[5], m.data[6], m.data[7]]);
                s.focus_distance = match (n, d) {
                    (0, _) | (_, 0) => None,
                    (u32::MAX, _) => Some(f64::INFINITY),
                    (n, d) => Some(f64::from(n) / f64::from(d)),
                };
            }
            if let (Some(t), Some(sub)) = (s.taken, find(0x9291)) {
                let digits: String = sub
                    .data
                    .iter()
                    .take_while(|b| b.is_ascii_digit())
                    .map(|&b| b as char)
                    .collect();
                if !digits.is_empty()
                    && let Ok(v) = format!("0.{digits}").parse::<f64>()
                {
                    s.taken = Some(t + v);
                }
            }
        }
        Some(s)
    }
}

/// `YYYY:MM:DD HH:MM:SS` → seconds since 1970 (no time zone; only differences matter).
pub fn parse_time(s: &str) -> Option<f64> {
    let b = s.trim().as_bytes();
    if b.len() < 19 {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        std::str::from_utf8(b.get(r)?).ok()?.trim().parse().ok()
    };
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, se) = (num(11..13)?, num(14..16)?, num(17..19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    // Days from civil (proleptic Gregorian).
    let (y2, m2) = if mo <= 2 {
        (y - 1, mo + 9)
    } else {
        (y, mo - 3)
    };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * m2 + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some((days * 86_400 + h * 3600 + mi * 60 + se) as f64)
}

/// The most frames a set holds (longer runs split).
pub const MAX_FRAMES: usize = 9;
/// Seconds between frames beyond their own exposure times that still join a run.
const GAP_S: f64 = 2.0;
/// Exposures closer than this (EV) are the same step.
const SAME_EV: f64 = 0.15;

/// A detected bracket: indices into the shots given, in capture order.
#[derive(Clone, Debug, PartialEq)]
pub struct BracketSet {
    pub frames: Vec<usize>,
    /// From the cameras' own bracketing flag rather than inferred from the exposures.
    pub auto: bool,
    /// Lowest to highest exposure, EV.
    pub span_ev: f64,
}

/// The bracket sets among `shots` (any order). Frames without a capture time or exposure
/// never join one.
pub fn bracket_sets(shots: &[Shot]) -> Vec<BracketSet> {
    let mut order: Vec<usize> = (0..shots.len())
        .filter(|&i| shots[i].taken.is_some() && shots[i].ev().is_some())
        .collect();
    order.sort_by(|&a, &b| {
        shots[a]
            .taken
            .partial_cmp(&shots[b].taken)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(shots[a].path.cmp(&shots[b].path))
    });
    let mut out = Vec::new();
    let mut run: Vec<usize> = Vec::new();
    let close = |run: &mut Vec<usize>, out: &mut Vec<BracketSet>| {
        if let Some(set) = finish(shots, run) {
            out.push(set);
        }
        run.clear();
    };
    for &i in &order {
        if let Some(&last) = run.last() {
            let (a, b) = (&shots[last], &shots[i]);
            let gap = b.taken.unwrap_or(0.0) - a.taken.unwrap_or(0.0);
            // Times are often whole seconds: allow the previous frame's exposure on top.
            let allowed = GAP_S + a.exposure_time.unwrap_or(0.0) + b.exposure_time.unwrap_or(0.0);
            let same_camera = a.make == b.make && a.model == b.model;
            let same_optics =
                near(a.f_number, b.f_number, 0.02) && near(a.focal_length, b.focal_length, 0.02);
            let ev = b.ev().unwrap_or(0.0);
            let repeats = run
                .iter()
                .any(|&j| (shots[j].ev().unwrap_or(f64::NAN) - ev).abs() < SAME_EV);
            if gap > allowed || !same_camera || !same_optics || repeats || run.len() >= MAX_FRAMES {
                close(&mut run, &mut out);
            }
        }
        run.push(i);
    }
    close(&mut run, &mut out);
    out
}

fn near(a: Option<f64>, b: Option<f64>, rel: f64) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a - b).abs() <= rel * a.abs().max(b.abs()),
        _ => true,
    }
}

fn finish(shots: &[Shot], run: &[usize]) -> Option<BracketSet> {
    if run.len() < 2 {
        return None;
    }
    let evs: Vec<f64> = run.iter().filter_map(|&i| shots[i].ev()).collect();
    let lo = evs.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = evs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let auto = run.iter().all(|&i| shots[i].auto_bracket);
    let span = hi - lo;
    (auto || span >= 2.0 / 3.0 - 1e-6).then(|| BracketSet {
        frames: run.to_vec(),
        auto,
        span_ev: span,
    })
}

/// The set `index` belongs to, if any.
pub fn set_of(sets: &[BracketSet], index: usize) -> Option<&BracketSet> {
    sets.iter().find(|s| s.frames.contains(&index))
}

/// The most frames a focus stack takes.
pub const MAX_FOCUS_FRAMES: usize = 200;
/// The most frames a panorama takes.
pub const MAX_PANO_FRAMES: usize = 60;
/// Seconds between panorama frames that still join a run (turning the camera by hand).
const PANO_GAP_S: f64 = 30.0;

/// Shots with capture times, in capture order (then by path).
fn timed(shots: &[Shot]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..shots.len())
        .filter(|&i| shots[i].taken.is_some())
        .collect();
    order.sort_by(|&a, &b| {
        shots[a]
            .taken
            .partial_cmp(&shots[b].taken)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(shots[a].path.cmp(&shots[b].path))
    });
    order
}

/// Runs of `order` where `joins(previous, next, run)` holds, at most `max` long; runs of at
/// least `min` frames are kept.
fn runs(
    order: &[usize],
    min: usize,
    max: usize,
    joins: &dyn Fn(usize, usize, &[usize]) -> bool,
) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    let mut run: Vec<usize> = Vec::new();
    for &i in order {
        if let Some(&last) = run.last()
            && (run.len() >= max || !joins(last, i, &run))
        {
            if run.len() >= min {
                out.push(std::mem::take(&mut run));
            }
            run.clear();
        }
        run.push(i);
    }
    if run.len() >= min {
        out.push(run);
    }
    out
}

fn same_camera(a: &Shot, b: &Shot) -> bool {
    a.make == b.make && a.model == b.model
}

/// Whether the run's known focus distances, with `next`'s, keep moving one way (unknown
/// distances never break a run; a repeated one does).
fn monotonic(shots: &[Shot], run: &[usize], next: usize) -> bool {
    let known: Vec<f64> = run
        .iter()
        .chain(std::iter::once(&next))
        .filter_map(|&i| shots[i].focus_distance)
        .collect();
    if known.len() < 2 {
        return true;
    }
    known.windows(2).all(|w| w[1] > w[0]) || known.windows(2).all(|w| w[1] < w[0])
}

/// The focus stacks among `shots` (any order): runs from one camera at one exposure and
/// focal length, shot as a burst (at most 2 s apart beyond the exposure times), at least
/// three frames, whose focus distance (when the files give it) moves one way. Capture
/// order; at most [`MAX_FOCUS_FRAMES`] frames.
pub fn focus_sets(shots: &[Shot]) -> Vec<BracketSet> {
    let order: Vec<usize> = timed(shots)
        .into_iter()
        .filter(|&i| shots[i].ev().is_some())
        .collect();
    let joins = |a: usize, b: usize, run: &[usize]| {
        let (x, y) = (&shots[a], &shots[b]);
        let gap = y.taken.unwrap_or(0.0) - x.taken.unwrap_or(0.0);
        let allowed = GAP_S + x.exposure_time.unwrap_or(0.0) + y.exposure_time.unwrap_or(0.0);
        gap <= allowed
            && same_camera(x, y)
            && near(x.focal_length, y.focal_length, 0.02)
            && (x.ev().unwrap_or(0.0) - y.ev().unwrap_or(0.0)).abs() < SAME_EV
            && monotonic(shots, run, b)
    };
    runs(&order, 3, MAX_FOCUS_FRAMES, &joins)
        .into_iter()
        .filter(|r| {
            // Distances known but all the same: a plain burst, not a focus bracket.
            let d: Vec<f64> = r.iter().filter_map(|&i| shots[i].focus_distance).collect();
            d.len() < 2 || d.iter().any(|v| (v - d[0]).abs() > 1e-9)
        })
        .map(|frames| BracketSet {
            frames,
            auto: false,
            span_ev: 0.0,
        })
        .collect()
}

/// The panoramas among `shots` (any order): runs from one camera at one focal length and
/// about one exposure (within 1/3 EV), at most 30 s apart, 2 to [`MAX_PANO_FRAMES`] frames.
/// Capture order.
pub fn pano_sets(shots: &[Shot]) -> Vec<BracketSet> {
    let order = timed(shots);
    let joins = |a: usize, b: usize, _: &[usize]| {
        let (x, y) = (&shots[a], &shots[b]);
        let gap = y.taken.unwrap_or(0.0) - x.taken.unwrap_or(0.0);
        let ev = match (x.ev(), y.ev()) {
            (Some(p), Some(q)) => (p - q).abs() <= 1.0 / 3.0 + 1e-6,
            _ => true,
        };
        gap <= PANO_GAP_S + y.exposure_time.unwrap_or(0.0)
            && same_camera(x, y)
            && near(x.focal_length, y.focal_length, 0.02)
            && ev
    };
    runs(&order, 2, MAX_PANO_FRAMES, &joins)
        .into_iter()
        .map(|frames| BracketSet {
            frames,
            auto: false,
            span_ev: 0.0,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shot(name: &str, t: f64, exp: f64, d: Option<f64>) -> Shot {
        Shot {
            path: PathBuf::from(name),
            make: "OM Digital".into(),
            model: "OM-1".into(),
            taken: Some(t),
            exposure_time: Some(exp),
            f_number: Some(5.6),
            iso: Some(200.0),
            focal_length: Some(60.0),
            focus_distance: d,
            ..Shot::default()
        }
    }

    #[test]
    fn focus_sets_are_bursts_with_moving_focus() {
        let at = |i: i32, t0: f64| t0 + 0.2 * f64::from(i);
        let mut s: Vec<Shot> = (0..5)
            .map(|i| {
                let d = 0.3 + 0.01 * f64::from(i);
                shot(&format!("a{i}"), at(i, 100.0), 0.01, Some(d))
            })
            .collect();
        // A gap, then a burst at one distance (not a stack), then one without distances.
        s.extend((0..4).map(|i| shot(&format!("b{i}"), at(i, 200.0), 0.01, Some(0.5))));
        s.extend((0..3).map(|i| shot(&format!("c{i}"), at(i, 300.0), 0.01, None)));
        // An exposure step breaks a run.
        s.push(shot("c3", 300.8, 0.04, None));
        let sets = focus_sets(&s);
        assert_eq!(sets.len(), 2, "{sets:?}");
        assert_eq!(sets[0].frames, vec![0, 1, 2, 3, 4]);
        assert_eq!(sets[1].frames, vec![9, 10, 11]);
        // Focus turning back breaks the run.
        let mut t: Vec<Shot> = (0..6)
            .map(|i| {
                let d = 1.0 + 0.1 * f64::from(i);
                shot(&format!("d{i}"), at(i, 0.0), 0.01, Some(d))
            })
            .collect();
        t[4].focus_distance = Some(0.5);
        t[5].focus_distance = Some(0.4);
        let sets = focus_sets(&t);
        assert_eq!(sets.len(), 1, "{sets:?}");
        assert_eq!(sets[0].frames, vec![0, 1, 2, 3]);
    }

    #[test]
    fn pano_sets_join_frames_turned_by_hand() {
        let mut s: Vec<Shot> = (0..4)
            .map(|i| shot(&format!("p{i}"), 10.0 * f64::from(i), 0.01, None))
            .collect();
        // Auto exposure drifts a little; a change of lens and a long pause break the run.
        s[2].exposure_time = Some(0.012);
        s.push(shot("q0", 100.0, 0.01, None));
        s[4].focal_length = Some(24.0);
        s.push(shot("q1", 110.0, 0.01, None));
        s[5].focal_length = Some(24.0);
        s.push(shot("r0", 500.0, 0.01, None));
        let sets = pano_sets(&s);
        assert_eq!(sets.len(), 2, "{sets:?}");
        assert_eq!(sets[0].frames, vec![0, 1, 2, 3]);
        assert_eq!(sets[1].frames, vec![4, 5]);
    }
}
