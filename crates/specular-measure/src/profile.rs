//! Line profiles: values along a path in the image, and values through frames (z or t).

use std::ops::Range;

use crate::calib::{Calibration, Source, Unit};
use crate::geom::{P, Shape};
use crate::raster::View;
use crate::stats;

/// Samples along a path.
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    /// Distance from the start in pixels.
    pub distance_px: Vec<f64>,
    /// Distance from the start in the calibration's unit.
    pub distance: Vec<f64>,
    /// Where each sample is.
    pub points: Vec<P>,
    /// `values[channel][sample]`, NaN outside the view.
    pub values: Vec<Vec<f64>>,
    pub unit: Unit,
    pub source: Source,
}

impl Profile {
    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

/// Bilinear samples (see [`View::bilinear`]) every `step` pixels of arc length along the
/// polyline `path`, from its start to exactly its end. With `width` > 1 each sample is the
/// mean of `width` samples one pixel apart across the path (perpendicular to the segment it
/// is on), as a wide line profile.
pub fn along(path: &[P], view: &View<'_>, cal: &Calibration, step: f64, width: u32) -> Profile {
    let ch = view.channels as usize;
    let mut out = Profile {
        distance_px: Vec::new(),
        distance: Vec::new(),
        points: Vec::new(),
        values: vec![Vec::new(); ch],
        unit: cal.unit,
        source: cal.source,
    };
    let step = if step.is_finite() && step > 0.0 {
        step
    } else {
        1.0
    };
    let width = width.max(1);
    let segs: Vec<(P, P, f64)> = path
        .windows(2)
        .map(|w| (w[0], w[1], (w[1] - w[0]).hypot()))
        .collect();
    let total: f64 = segs.iter().map(|s| s.2).sum();
    if path.is_empty() || !total.is_finite() {
        return out;
    }
    let mut at: Vec<f64> = (0..)
        .map(|k| f64::from(k) * step)
        .take_while(|&s| s <= total)
        .collect();
    if at.last().is_none_or(|&s| total - s > 1e-9 * total.max(1.0)) {
        at.push(total);
    }
    let (mut seg, mut seg_start, mut phys_start) = (0usize, 0.0f64, 0.0f64);
    for s in at {
        while seg + 1 < segs.len() && s > seg_start + segs[seg].2 {
            seg_start += segs[seg].2;
            phys_start += cal.distance(segs[seg].0, segs[seg].1);
            seg += 1;
        }
        let (a, b, len) = segs.get(seg).copied().unwrap_or((path[0], path[0], 0.0));
        let t = if len > 0.0 {
            ((s - seg_start) / len).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let p = P::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
        let normal = if len > 0.0 {
            P::new(-(b.y - a.y) / len, (b.x - a.x) / len)
        } else {
            P::new(0.0, 0.0)
        };
        out.distance_px.push(s);
        out.distance.push(phys_start + cal.distance(a, p));
        out.points.push(p);
        for (c, vals) in out.values.iter_mut().enumerate() {
            let (mut sum, mut n) = (0.0, 0u32);
            for k in 0..width {
                let o = f64::from(k) - f64::from(width - 1) / 2.0;
                if let Some(v) = view.bilinear(p.x + normal.x * o, p.y + normal.y * o, c as u32) {
                    sum += v;
                    n += 1;
                }
            }
            vals.push(if n > 0 { sum / f64::from(n) } else { f64::NAN });
        }
    }
    out
}

/// The profile of a line or polyline shape; `None` for other shapes.
pub fn of_shape(
    shape: &Shape,
    view: &View<'_>,
    cal: &Calibration,
    step: f64,
    width: u32,
) -> Option<Profile> {
    shape
        .is_path()
        .then(|| along(&shape.points(), view, cal, step, width))
}

/// Values through frames.
#[derive(Clone, Debug, PartialEq)]
pub struct Through {
    pub frames: Vec<u32>,
    /// `values[channel][i]` for `frames[i]`, NaN where the frame had no value.
    pub values: Vec<Vec<f64>>,
}

/// A profile through frames (z slices or t time points): `sample(frame)` gives the frame's
/// values per channel (from [`at`] or [`roi_mean`] on the frame's view), or `None`.
pub fn through(frames: Range<u32>, mut sample: impl FnMut(u32) -> Option<Vec<f64>>) -> Through {
    let mut out = Through {
        frames: Vec::new(),
        values: Vec::new(),
    };
    for (i, f) in frames.enumerate() {
        let v = sample(f);
        let ch = v.as_ref().map_or(0, Vec::len);
        while out.values.len() < ch {
            out.values.push(vec![f64::NAN; i]);
        }
        for (c, vals) in out.values.iter_mut().enumerate() {
            vals.push(
                v.as_ref()
                    .and_then(|v| v.get(c).copied())
                    .unwrap_or(f64::NAN),
            );
        }
        out.frames.push(f);
    }
    out
}

/// Every channel at a point, bilinearly.
pub fn at(view: &View<'_>, p: P) -> Option<Vec<f64>> {
    (0..view.channels)
        .map(|c| view.bilinear(p.x, p.y, c))
        .collect()
}

/// The mean of every channel over a shape's pixels.
pub fn roi_mean(view: &View<'_>, shape: &Shape) -> Option<Vec<f64>> {
    Some(
        stats::roi(shape, view, 1)?
            .into_iter()
            .map(|s| s.mean)
            .collect(),
    )
}
