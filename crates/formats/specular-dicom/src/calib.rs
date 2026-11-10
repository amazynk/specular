//! Physical calibration of a frame for measurements, and frame timing for cine.
//!
//! Spacing (PS3.3 10.7.1.3): Pixel Spacing (0028,0030) is in the patient for cross-sectional
//! images, or — for projection radiography — calibrated when Pixel Spacing Calibration Type
//! (0028,0A02) says so, and otherwise only the detector spacing when it equals Imager Pixel
//! Spacing (0018,1164). Imager Pixel Spacing alone is at the detector plane, so lengths
//! through it are magnified by the geometry; with an Estimated Radiographic Magnification
//! Factor (0018,1114) it is divided out, as an estimate. Enhanced multi-frame images take the
//! frame's Pixel Measures (0028,9110). Ultrasound images carry spacing per Sequence of
//! Ultrasound Regions (0018,6011) item instead; a measurement must lie in one region whose
//! physical units are centimetres in both directions.
//!
//! The geometry, coverage and statistics of measurements are specular-measure's: a spacing
//! becomes its [`specular_measure::Calibration`] (with the matching DICOM source) through
//! [`Calibration::measure`].

use std::time::Duration;

use crate::display::functional;
use crate::parse::{DataSet, Values};
use specular_measure::{self as measure, P, Source, Unit};

/// Where a spacing comes from, which decides the caveat a measurement carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpacingKind {
    /// In the patient (cross-sectional images, or Pixel Spacing without a detector spacing).
    Patient,
    /// Calibrated by the modality or an operator (Pixel Spacing Calibration Type).
    Calibrated,
    /// At the detector (or film) plane: lengths in the patient are smaller by the
    /// magnification.
    Detector,
    /// Corrected by the estimated radiographic magnification: an estimate.
    Magnification,
    /// A calibrated ultrasound region.
    Region,
}

/// Millimetres between the centres of adjacent pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spacing {
    /// Between columns (along x).
    pub x: f64,
    /// Between rows (along y).
    pub y: f64,
    pub kind: SpacingKind,
}

/// An ultrasound region (inclusive pixel bounds) with physical spacing in millimetres, or
/// `None` when its units are not spatial.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UsRegion {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
    pub spacing: Option<(f64, f64)>,
    /// Region Spatial Format (1 = 2D, 2 = M-mode, 3 = spectral, ...).
    pub format: u16,
}

impl UsRegion {
    fn contains(&self, (x, y): (f64, f64)) -> bool {
        x >= f64::from(self.x0)
            && x < f64::from(self.x1) + 1.0
            && y >= f64::from(self.y0)
            && y < f64::from(self.y1) + 1.0
    }
}

/// How to turn a frame's pixel distances into millimetres.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Calibration {
    /// The frame-wide spacing (none for ultrasound regions or uncalibrated images).
    pub spacing: Option<Spacing>,
    pub regions: Vec<UsRegion>,
}

/// Why a measurement has no physical size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Uncalibrated {
    /// The image states no spacing.
    None,
    /// An ultrasound image, but the points are not inside one spatially calibrated region.
    OutsideRegion,
}

impl Calibration {
    /// The spacing for a measurement through `points` (stored pixel coordinates).
    pub fn for_points(&self, points: &[(f64, f64)]) -> Result<Spacing, Uncalibrated> {
        if let Some(s) = self.spacing {
            return Ok(s);
        }
        if self.regions.is_empty() {
            return Err(Uncalibrated::None);
        }
        self.regions
            .iter()
            .filter(|r| points.iter().all(|&p| r.contains(p)))
            .find_map(|r| r.spacing)
            .map(|(x, y)| Spacing {
                x,
                y,
                kind: SpacingKind::Region,
            })
            .ok_or(Uncalibrated::OutsideRegion)
    }

    pub(crate) fn read(ds: &DataSet, v: &Values, frame: u32) -> Calibration {
        let pair = |d: &DataSet, tag: u32| {
            let e = d.get(tag)?;
            let (a, b) = (v.number(e, 0)?, v.number(e, 1).or(v.number(e, 0))?);
            (a > 0.0 && b > 0.0).then_some((a, b))
        };
        let measures = functional(ds, frame, 0x0028_9110);
        let pixel = measures
            .and_then(|m| pair(m, 0x0028_0030))
            .or_else(|| pair(ds, 0x0028_0030));
        let imager = pair(ds, 0x0018_1164).or_else(|| {
            // Enhanced DX / XA keep it in a functional group.
            functional(ds, frame, 0x0018_9412)
                .or_else(|| functional(ds, frame, 0x0028_9110))
                .and_then(|m| pair(m, 0x0018_1164))
        });
        let calibrated = ds.get(0x0028_0A02).and_then(|e| v.text_n(e, 0)).is_some();
        let ermf = ds
            .get(0x0018_1114)
            .and_then(|e| v.number(e, 0))
            .filter(|m| *m >= 0.1 && *m <= 10.0);
        let close = |a: (f64, f64), b: (f64, f64)| {
            (a.0 - b.0).abs() <= 1e-4 * a.0 && (a.1 - b.1).abs() <= 1e-4 * a.1
        };
        // (row spacing, column spacing) → x = column, y = row.
        let spacing = |(r, c): (f64, f64), kind| Spacing { x: c, y: r, kind };
        let s = match (pixel, imager) {
            (Some(p), _) if calibrated => Some(spacing(p, SpacingKind::Calibrated)),
            (Some(p), Some(i)) if close(p, i) => Some(spacing(p, SpacingKind::Detector)),
            (Some(p), Some(_)) => Some(spacing(p, SpacingKind::Magnification)),
            (Some(p), None) => Some(spacing(p, SpacingKind::Patient)),
            (None, Some(i)) => Some(match ermf {
                Some(m) => spacing((i.0 / m, i.1 / m), SpacingKind::Magnification),
                None => spacing(i, SpacingKind::Detector),
            }),
            (None, None) => pair(ds, 0x0018_2010).map(|n| spacing(n, SpacingKind::Detector)),
        };
        let regions = us_regions(ds, v);
        Calibration {
            spacing: if regions.is_empty() { s } else { None },
            regions,
        }
    }
}

fn us_regions(ds: &DataSet, v: &Values) -> Vec<UsRegion> {
    let Some(e) = ds.get(0x0018_6011) else {
        return Vec::new();
    };
    let int = |d: &DataSet, tag| d.get(tag).and_then(|e| v.int(e));
    let num = |d: &DataSet, tag| d.get(tag).and_then(|e| v.number(e, 0));
    e.items
        .iter()
        .take(64)
        .filter_map(|r| {
            let x0 = int(r, 0x0018_6018)?;
            let y0 = int(r, 0x0018_601A)?;
            let x1 = int(r, 0x0018_601C)?;
            let y1 = int(r, 0x0018_601E)?;
            if x1 < x0 || y1 < y0 || x0 < 0 || y0 < 0 {
                return None;
            }
            // Units 3: centimetres.
            let cm = int(r, 0x0018_6024) == Some(3) && int(r, 0x0018_6026) == Some(3);
            let spacing = match (num(r, 0x0018_602C), num(r, 0x0018_602E)) {
                (Some(dx), Some(dy)) if cm && dx > 0.0 && dy > 0.0 => {
                    Some((dx.abs() * 10.0, dy.abs() * 10.0))
                }
                _ => None,
            };
            Some(UsRegion {
                x0: x0 as u32,
                y0: y0 as u32,
                x1: x1.min(u32::MAX as i64) as u32,
                y1: y1.min(u32::MAX as i64) as u32,
                spacing,
                format: int(r, 0x0018_6012).unwrap_or(0) as u16,
            })
        })
        .collect()
}

/// How long each of `frames` frames shows, from Frame Time Vector (0018,1065), Frame Time
/// (0018,1063), Cine Rate (0018,0040) or Recommended Display Frame Rate (0008,2144), in that
/// order; `None` when the file states no timing.
pub(crate) fn frame_durations(ds: &DataSet, v: &Values, frames: u32) -> Option<Vec<Duration>> {
    let ms = |x: f64| Duration::from_secs_f64((x / 1000.0).clamp(0.001, 10.0));
    // Number of Frames may claim a billion: a frame takes at least a byte of the file.
    let n = (frames.max(1) as usize).min(v.buf.len().max(1));
    if let Some(e) = ds.get(0x0018_1065) {
        // Entry i is the time from frame i − 1 to frame i (the first is 0): frame i shows
        // for entry i + 1; the last for the mean.
        let t: Vec<f64> = (0..v.count(e).min(n + 1))
            .filter_map(|i| v.number(e, i))
            .collect();
        if t.len() >= 2 && t[1..].iter().all(|x| *x > 0.0) {
            let mean = t[1..].iter().sum::<f64>() / (t.len() - 1) as f64;
            return Some(
                (0..n)
                    .map(|i| ms(t.get(i + 1).copied().unwrap_or(mean)))
                    .collect(),
            );
        }
    }
    let num = |tag: u32| {
        ds.get(tag)
            .and_then(|e| v.number(e, 0))
            .filter(|x| *x > 0.0)
    };
    let per = num(0x0018_1063)
        .or_else(|| num(0x0018_0040).map(|fps| 1000.0 / fps))
        .or_else(|| num(0x0008_2144).map(|fps| 1000.0 / fps))?;
    Some(vec![ms(per); n])
}

impl SpacingKind {
    /// The measurement layer's source for this kind: its badge, and whether the sizes are
    /// approximate.
    pub fn source(self) -> Source {
        match self {
            SpacingKind::Patient => Source::DicomPixelSpacing,
            SpacingKind::Calibrated => Source::DicomCalibrated,
            SpacingKind::Detector => Source::DicomImagerPixelSpacing,
            SpacingKind::Magnification => Source::DicomMagnification,
            SpacingKind::Region => Source::DicomUltrasoundRegion,
        }
    }
}

impl Spacing {
    /// As a measurement-layer calibration in millimetres (`None` for a spacing that is not
    /// positive and finite).
    pub fn calibration(&self) -> Option<measure::Calibration> {
        measure::Calibration::spacing(self.x, self.y, Unit::Mm, self.kind.source()).ok()
    }
}

impl Calibration {
    /// The measurement-layer calibration for a measurement through `points` (stored pixel
    /// coordinates): the frame's spacing, or the ultrasound region holding every point.
    pub fn measure(&self, points: &[P]) -> Result<measure::Calibration, Uncalibrated> {
        let pts: Vec<(f64, f64)> = points.iter().map(|p| (p.x, p.y)).collect();
        let s = self.for_points(&pts)?;
        s.calibration().ok_or(match s.kind {
            SpacingKind::Region => Uncalibrated::OutsideRegion,
            _ => Uncalibrated::None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_measure::Shape;

    fn cal(x: f64, y: f64) -> Calibration {
        Calibration {
            spacing: Some(Spacing {
                x,
                y,
                kind: SpacingKind::Patient,
            }),
            regions: Vec::new(),
        }
    }

    fn length(c: &Calibration, a: P, b: P) -> Result<(f64, Source), Uncalibrated> {
        let m = c.measure(&[a, b])?;
        let q = Shape::Line(a, b).length(&m).unwrap();
        Ok((q.value, q.source))
    }

    #[test]
    fn spacing_becomes_a_measure_calibration() {
        let c = cal(0.5, 2.0);
        let m = c.measure(&[P::new(0.0, 0.0)]).unwrap();
        assert_eq!(m.unit, Unit::Mm);
        assert_eq!(m.source, Source::DicomPixelSpacing);
        assert_eq!(m.axis_spacing(), (0.5, 2.0));
        assert_eq!(
            length(&c, P::new(0.0, 0.0), P::new(6.0, 2.0)),
            Ok((5.0, Source::DicomPixelSpacing))
        );
        assert_eq!(
            Calibration::default().measure(&[P::new(0.0, 0.0)]),
            Err(Uncalibrated::None)
        );
        for (kind, source) in [
            (SpacingKind::Calibrated, Source::DicomCalibrated),
            (SpacingKind::Detector, Source::DicomImagerPixelSpacing),
            (SpacingKind::Magnification, Source::DicomMagnification),
            (SpacingKind::Region, Source::DicomUltrasoundRegion),
        ] {
            assert_eq!(kind.source(), source);
        }
        assert!(SpacingKind::Magnification.source().is_approximate());
    }

    #[test]
    fn regions_calibrate_only_inside() {
        let c = Calibration {
            spacing: None,
            regions: vec![
                UsRegion {
                    x0: 10,
                    y0: 10,
                    x1: 109,
                    y1: 109,
                    spacing: Some((0.2, 0.2)),
                    format: 1,
                },
                UsRegion {
                    x0: 200,
                    y0: 0,
                    x1: 299,
                    y1: 99,
                    spacing: None,
                    format: 3,
                },
            ],
        };
        let (mm, source) = length(&c, P::new(10.0, 10.0), P::new(60.0, 10.0)).unwrap();
        assert!((mm - 10.0).abs() < 1e-9);
        assert_eq!(source, Source::DicomUltrasoundRegion);
        assert_eq!(
            length(&c, P::new(10.0, 10.0), P::new(150.0, 10.0)),
            Err(Uncalibrated::OutsideRegion)
        );
        assert_eq!(
            length(&c, P::new(210.0, 10.0), P::new(220.0, 10.0)),
            Err(Uncalibrated::OutsideRegion)
        );
    }
}
