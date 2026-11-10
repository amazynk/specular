//! Measurements in medical mode: length, angle, and elliptical / rectangular regions with the
//! statistics of their modality values. Points are kept in stored pixel coordinates (pixel
//! (i, j) covers i..i + 1 × j..j + 1) on the frame they were made on, so they follow zoom,
//! pan, rotation and flips, and show again when that frame does.
//!
//! Geometry, coverage and statistics are the measurement layer's ([`specular_measure`]), the
//! same as the viewer's measuring tool. Sizes go through the frame's calibration
//! ([`specular_dicom::calib`]): millimetres when the file states a spacing, with a caveat
//! when that spacing is at the detector or a magnification estimate; pixels, with the reason,
//! when it states none or the points leave a calibrated ultrasound region.

use specular_decode::ModalityFrame;
use specular_dicom::Uncalibrated;
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::t;
use specular_input::Action;
use specular_measure::{Calibration, Quantity, Shape, Source};
use specular_theme::Color;
use specular_theme::metrics::{FONT_BODY, RADIUS_SM};

use super::number;

/// A point in stored pixel coordinates.
pub(crate) type P = (f64, f64);

/// A measuring tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tool {
    Length,
    /// Three points: the angle is at the second.
    Angle,
    /// The bounding box's corners.
    Ellipse,
    Rect,
}

impl Tool {
    pub fn of(a: Action) -> Option<Tool> {
        Some(match a {
            Action::MeasureLength => Tool::Length,
            Action::MeasureAngle => Tool::Angle,
            Action::MeasureEllipse => Tool::Ellipse,
            Action::MeasureRectangle => Tool::Rect,
            _ => return None,
        })
    }

    /// Points that complete a measurement.
    pub fn points(self) -> usize {
        if self == Tool::Angle { 3 } else { 2 }
    }

    /// Fluent id of the hint shown when the tool is picked.
    pub fn hint_id(self) -> &'static str {
        match self {
            Tool::Length => "measure-hint-length",
            Tool::Angle => "measure-hint-angle",
            Tool::Ellipse => "measure-hint-ellipse",
            Tool::Rect => "measure-hint-rectangle",
        }
    }
}

/// A finished measurement.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Measurement {
    pub tool: Tool,
    pub points: Vec<P>,
    /// The frame it was made on.
    pub frame: u32,
    /// Its label, first line the size.
    pub lines: Vec<String>,
}

fn p((x, y): P) -> specular_measure::P {
    specular_measure::P::new(x, y)
}

/// The shape of a measurement (or a draft) through `pts`, once it has all its points.
pub(crate) fn shape_of(tool: Tool, pts: &[P]) -> Option<Shape> {
    Some(match (tool, pts) {
        (Tool::Length, &[a, b]) => Shape::Line(p(a), p(b)),
        (Tool::Angle, &[a, v, b]) => Shape::Angle {
            a: p(a),
            vertex: p(v),
            b: p(b),
        },
        (Tool::Ellipse, &[a, b]) => Shape::ellipse_in(p(a), p(b)),
        (Tool::Rect, &[a, b]) => Shape::rect(p(a), p(b)),
        _ => return None,
    })
}

/// The region of an ellipse or rectangle from two opposite corners.
pub(crate) fn roi_of(tool: Tool, a: P, b: P) -> Option<Shape> {
    match tool {
        Tool::Ellipse | Tool::Rect => shape_of(tool, &[a, b]),
        Tool::Length | Tool::Angle => None,
    }
}

/// A size: calibrated, or in pixels with the reason there is no physical size.
pub(crate) type Size = Result<Quantity, (f64, Uncalibrated)>;

/// The size of `shape` (its length, or its area when `area`) through the frame calibration.
fn size(shape: &Shape, cal: &Result<Calibration, Uncalibrated>, area: bool) -> Option<Size> {
    let measure = |c: &Calibration| {
        if area { shape.area(c) } else { shape.length(c) }
    };
    Some(match cal {
        Ok(c) => Ok(measure(c)?),
        Err(why) => Err((measure(&Calibration::uncalibrated())?.value, *why)),
    })
}

/// A size and the caveat that goes with it: `mm` or `px`, squared for areas.
pub(crate) fn size_text(s: Size, area: bool) -> (String, Option<String>) {
    let caveat = |id: &str| Some(specular_i18n::I18n::global().tr(id));
    match s {
        Ok(q) => {
            let value = specular_i18n::fixed(q.value, 1).replace('-', "\u{2212}");
            let text = if area {
                t!("measure-area-mm", value = value)
            } else {
                t!("measure-length-mm", value = value)
            };
            let c = match q.source {
                Source::DicomImagerPixelSpacing => caveat("measure-caveat-detector"),
                Source::DicomMagnification => caveat("measure-caveat-magnification"),
                _ => None,
            };
            (text, c)
        }
        Err((v, why)) => {
            let value = specular_i18n::fixed(v, 1);
            let text = if area {
                t!("measure-area-px", value = value)
            } else {
                t!("measure-length-px", value = value)
            };
            let c = match why {
                Uncalibrated::None => caveat("measure-caveat-uncalibrated"),
                Uncalibrated::OutsideRegion => caveat("measure-caveat-outside-region"),
            };
            (text, c)
        }
    }
}

/// The label of a measurement through `pts` on `frame`, values in `unit`.
pub(crate) fn label(
    tool: Tool,
    pts: &[P],
    frame: &ModalityFrame,
    unit: Option<&str>,
) -> Vec<String> {
    let mut lines = Vec::new();
    let Some(shape) = shape_of(tool, pts) else {
        return lines;
    };
    // An area is calibrated by its bounding box (an ultrasound region must hold all of it).
    let cal = frame.calibration().measure(&shape.points());
    let push_size = |area: bool, lines: &mut Vec<String>| {
        if let Some(s) = size(&shape, &cal, area) {
            let (text, caveat) = size_text(s, area);
            lines.push(text);
            lines.extend(caveat);
        }
    };
    match tool {
        Tool::Length => push_size(false, &mut lines),
        Tool::Angle => {
            // In physical space when the pixels have a known (possibly non-square) spacing.
            let c = cal.as_ref().map_or(Calibration::uncalibrated(), |c| *c);
            if let Some(q) = shape.angle(&c) {
                lines.push(t!(
                    "measure-angle",
                    value = specular_i18n::fixed(q.value, 1)
                ));
            }
        }
        Tool::Ellipse | Tool::Rect => {
            push_size(true, &mut lines);
            // No pixel centre inside: the area alone.
            if let Some(s) = frame.roi_stats(&shape) {
                let unit = unit.unwrap_or("");
                lines.push(
                    t!(
                        "measure-mean",
                        mean = number(s.mean),
                        sd = number(s.sd),
                        unit = unit
                    )
                    .trim_end()
                    .to_owned(),
                );
                lines.push(
                    t!(
                        "measure-range",
                        min = number(s.min),
                        max = number(s.max),
                        count = s.count as i64
                    )
                    .trim_end()
                    .to_owned(),
                );
            }
        }
    }
    lines
}

/// The outline of a measurement (or a draft) through `pts`, in stored coordinates, and
/// whether it is closed.
pub(crate) fn outline(tool: Tool, pts: &[P]) -> (Vec<P>, bool) {
    match (tool, shape_of(tool, pts)) {
        (Tool::Ellipse | Tool::Rect, Some(s)) => {
            (s.outline().into_iter().map(|q| (q.x, q.y)).collect(), true)
        }
        _ => (pts.to_vec(), false),
    }
}

/// A measurement's handles: a length's or angle's points; a region's four corners (the two
/// placed, then the other two: `(a.x, b.y)` and `(b.x, a.y)`).
pub(crate) fn handles(tool: Tool, pts: &[P]) -> Vec<P> {
    match (tool, pts) {
        (Tool::Ellipse | Tool::Rect, &[a, b]) => vec![a, b, (a.0, b.1), (b.0, a.1)],
        _ => pts.to_vec(),
    }
}

/// Moves handle `e` (see [`handles`]) of a measurement to `q`: a region's corner keeps the
/// opposite corner where it is.
pub(crate) fn move_handle(tool: Tool, pts: &mut [P], e: usize, q: P) {
    match (tool, &mut *pts) {
        (Tool::Ellipse | Tool::Rect, [a, b]) => match e {
            0 => *a = q,
            1 => *b = q,
            2 => (a.0, b.1) = q,
            3 => (b.0, a.1) = q,
            _ => {}
        },
        _ => {
            if let Some(p) = pts.get_mut(e) {
                *p = q;
            }
        }
    }
}

/// The handle of a measurement on `frame` drawn under screen point `p` (a length's or
/// angle's point, a region's corner): the nearest within `radius`, as (measurement, handle).
/// `to_screen` maps stored points.
pub(crate) fn end_at(
    items: &[Measurement],
    frame: u32,
    to_screen: impl Fn(P) -> P,
    p: P,
    radius: f64,
) -> Option<(usize, usize)> {
    let mut best: Option<(f64, (usize, usize))> = None;
    for (i, m) in items.iter().enumerate() {
        if m.frame != frame {
            continue;
        }
        for (e, &q) in handles(m.tool, &m.points).iter().enumerate() {
            let s = to_screen(q);
            let d = (s.0 - p.0).hypot(s.1 - p.1);
            if d <= radius && best.is_none_or(|(b, _)| d < b) {
                best = Some((d, (i, e)));
            }
        }
    }
    best.map(|(_, at)| at)
}

/// Whether point `e` of `pts` was dropped onto another of its points (within `gone` on
/// screen): the measurement is removed.
pub(crate) fn dropped_on_another(
    pts: &[P],
    e: usize,
    to_screen: impl Fn(P) -> P,
    gone: f64,
) -> bool {
    let Some(&q) = pts.get(e) else {
        return false;
    };
    let a = to_screen(q);
    pts.iter().enumerate().any(|(k, &o)| {
        let b = to_screen(o);
        k != e && (a.0 - b.0).hypot(a.1 - b.1) < gone
    })
}

/// A measurement on screen (logical pixels).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Drawn {
    pub outline: Vec<(f32, f32)>,
    pub closed: bool,
    /// The points placed (handles).
    pub handles: Vec<(f32, f32)>,
    pub lines: Vec<String>,
    /// Still being placed.
    pub draft: bool,
}

const INK: Color = Color::rgba(1.0, 0.84, 0.2, 1.0);

/// Draws the measurements: outlines over a dark halo, handles, and labels next to the
/// rightmost (leftmost in right-to-left) point.
pub(crate) fn draw(dl: &mut DrawList, text: &mut Text, items: &[Drawn], rtl: bool, ts: f32) {
    let halo = Color::rgba(0.0, 0.0, 0.0, 0.55);
    for m in items {
        let ink = if m.draft { INK.with_alpha(0.75) } else { INK };
        let segs: Vec<((f32, f32), (f32, f32))> = m
            .outline
            .windows(2)
            .map(|w| (w[0], w[1]))
            .chain(
                m.closed
                    .then(|| Some((*m.outline.last()?, *m.outline.first()?)))
                    .flatten(),
            )
            .collect();
        for &(a, b) in &segs {
            dl.line(a, b, 3.0, halo);
        }
        for &(a, b) in &segs {
            dl.line(a, b, 1.5, ink);
        }
        for &h in &m.handles {
            dl.circle(h, 3.5, halo);
            dl.circle(h, 2.5, ink);
        }
        if m.lines.is_empty() {
            continue;
        }
        let st = TextStyle::new(FONT_BODY * ts).tabular();
        let blocks: Vec<_> = m
            .lines
            .iter()
            .map(|l| text.layout(l, st, None, Align::Start))
            .collect();
        let pad = 4.0 * ts;
        let w = blocks.iter().map(|b| b.width).fold(0.0, f32::max) + 2.0 * pad;
        let h = blocks.iter().map(|b| b.height).sum::<f32>() + 2.0 * pad;
        let pts = m.handles.iter().chain(&m.outline);
        let anchor = if rtl {
            pts.copied()
                .fold((f32::MAX, 0.0), |a, p| if p.0 < a.0 { p } else { a })
        } else {
            pts.copied()
                .fold((f32::MIN, 0.0), |a, p| if p.0 > a.0 { p } else { a })
        };
        let gap = 10.0 * ts;
        let x = if rtl {
            anchor.0 - gap - w
        } else {
            anchor.0 + gap
        };
        let y = anchor.1 - h / 2.0;
        dl.rect(
            Rect::new(x, y, w, h),
            RADIUS_SM,
            Color::rgba(0.0, 0.0, 0.0, 0.6),
        );
        let mut ty = y + pad;
        for (i, b) in blocks.iter().enumerate() {
            let c = if i == 0 {
                INK
            } else {
                Color::WHITE.with_alpha(0.85)
            };
            text.draw(dl, b, x + pad, ty, c);
            ty += b.height;
        }
    }
}
