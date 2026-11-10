//! Time-intensity curves of dynamic series: an ROI's mean and max at each time point, and
//! their CSV.
//!
//! A dynamic series is either one multi-frame file (frame times from the Frame Time (Vector))
//! or many single-frame files repeating slice positions (one per time point, ordered by
//! Temporal Position Index, Trigger Time / Frame Reference Time, Acquisition Time, Instance
//! Number). Read-only analysis (DESIGN §7): the numbers, no fitted model or verdict.

use std::fmt::Write;

use crate::ModalityFrame;
use crate::series::{Instance, Series};
use specular_measure::{ChannelStats, Shape};

/// One time point.
#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    /// Seconds from the first time point.
    pub t: f64,
    pub mean: f64,
    pub max: f64,
    pub sd: f64,
    pub count: u64,
}

/// The ROI's statistics on each frame, at `times` (s).
pub fn curve(frames: &[(f64, &ModalityFrame)], roi: &Shape) -> Vec<Point> {
    let t0 = frames.first().map_or(0.0, |f| f.0);
    frames
        .iter()
        .filter_map(|(t, f)| {
            let ChannelStats {
                count,
                mean,
                sd,
                max,
                ..
            } = f.roi_stats(roi)?;
            Some(Point {
                t: t - t0,
                mean,
                max,
                sd,
                count,
            })
        })
        .collect()
}

/// Seconds of each frame of a multi-frame file from its frame durations (starting at 0).
pub fn frame_times(durations: &[std::time::Duration]) -> Vec<f64> {
    let mut t = 0.0;
    let mut out = Vec::with_capacity(durations.len());
    for d in durations {
        out.push(t);
        t += d.as_secs_f64();
    }
    out
}

/// The time points of a series at the slice position of item `at`: the items sharing its
/// position, in time order, with each one's time (s, from the first). `None` when only one
/// item lies there (not dynamic).
pub fn time_points<K>(series: &Series<K>, at: usize) -> Option<Vec<(usize, f64)>> {
    let here = series.items.get(at)?;
    let same = |a: Option<f64>| match (a, here.along) {
        (Some(a), Some(b)) => (a - b).abs() < 0.01,
        (None, None) => true,
        _ => false,
    };
    let mut items: Vec<usize> = (0..series.items.len())
        .filter(|&i| same(series.items[i].along))
        .collect();
    if items.len() < 2 {
        return None;
    }
    let inst = |i: usize| &series.items[i].instance;
    items.sort_by(|&a, &b| order(inst(a), inst(b)));
    let time = |i: &Instance| -> Option<f64> {
        i.trigger_time
            .map(|t| t / 1000.0)
            .or(i.acquisition_time)
            .or(i.temporal_position.map(|p| p as f64))
    };
    let t0 = time(inst(items[0]));
    Some(
        items
            .iter()
            .enumerate()
            .map(|(n, &i)| {
                let t = match (time(inst(i)), t0) {
                    (Some(t), Some(t0)) => t - t0,
                    _ => n as f64,
                };
                (i, t)
            })
            .collect(),
    )
}

fn order(a: &Instance, b: &Instance) -> std::cmp::Ordering {
    let f = |x: Option<f64>, y: Option<f64>| match (x, y) {
        (Some(x), Some(y)) => x.total_cmp(&y),
        // Missing times last: "unknown equals anything" is not transitive, and a sort with
        // such a comparator may panic.
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    };
    a.temporal_position
        .cmp(&b.temporal_position)
        .then_with(|| f(a.trigger_time, b.trigger_time))
        .then_with(|| f(a.acquisition_time, b.acquisition_time))
        .then_with(|| a.instance_number.cmp(&b.instance_number))
}

/// The curve as CSV: a header naming `unit`, then time (s), mean, max, SD and pixel count.
pub fn csv(points: &[Point], unit: &str) -> String {
    let unit = unit.replace([',', '"', '\n', '\r'], " ");
    let mut s = format!("time_s,mean_{unit},max_{unit},sd_{unit},pixels\n");
    for p in points {
        let _ = writeln!(s, "{},{},{},{},{}", p.t, p.mean, p.max, p.sd, p.count);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The phase order is total with missing, NaN and near-equal times mixed (a perfusion
    /// series with 40+ phases at one position used to be able to make the sort panic).
    #[test]
    fn phase_order_is_total() {
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let time = |r: u64| match r % 6 {
            0 => None,
            1 => Some(f64::NAN),
            2 => Some(1.0 + (r % 5) as f64 * 1e-12),
            _ => Some((r % 7) as f64),
        };
        for _ in 0..200 {
            let v: Vec<Instance> = (0..24)
                .map(|_| Instance {
                    temporal_position: (next() % 3 == 0).then(|| (next() % 3) as i64),
                    trigger_time: time(next()),
                    acquisition_time: time(next()),
                    instance_number: (next() % 4 != 0).then(|| (next() % 5) as i64),
                    ..Instance::default()
                })
                .collect();
            for a in &v {
                for b in &v {
                    assert_eq!(order(a, b), order(b, a).reverse());
                    for c in &v {
                        if order(a, b).is_le() && order(b, c).is_le() {
                            assert!(order(a, c).is_le());
                        }
                    }
                }
            }
            let mut idx: Vec<usize> = (0..v.len()).collect();
            idx.sort_by(|&a, &b| order(&v[a], &v[b]));
        }
    }

    #[test]
    fn csv_and_times() {
        let pts = vec![
            Point {
                t: 0.0,
                mean: 10.5,
                max: 20.0,
                sd: 1.0,
                count: 4,
            },
            Point {
                t: 2.5,
                mean: 11.0,
                max: 22.0,
                sd: 0.5,
                count: 4,
            },
        ];
        assert_eq!(
            csv(&pts, "HU"),
            "time_s,mean_HU,max_HU,sd_HU,pixels\n0,10.5,20,1,4\n2.5,11,22,0.5,4\n"
        );
        let d = [1.0, 1.5, 2.0].map(std::time::Duration::from_secs_f64);
        assert_eq!(frame_times(&d), vec![0.0, 1.0, 2.5]);
    }
}
