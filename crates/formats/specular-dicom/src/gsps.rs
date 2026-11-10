//! Grayscale Softcopy Presentation States (PS3.3 A.33.1), read only: which images a state
//! applies to, its display pipeline (Modality LUT, Softcopy VOI LUT, Presentation LUT
//! shape or table, PS3.3 C.11), spatial transform (C.10.6), displayed area (C.10.4),
//! graphic annotations and graphic layers (C.10.5, C.10.7).
//!
//! [`ModalityFrame::presented`] puts a state's display pipeline in place of the image's;
//! the geometry (rotation, flip, displayed area, annotations) is for the viewer to apply.
//! Coordinates in PIXEL units are stored-image coordinates with the origin at the top-left
//! corner of the top-left pixel (pixel (i, j) covers i..i + 1 × j..j + 1); DISPLAY units are
//! fractions of the displayed area as shown (after rotation and flip), 0..1 across.

use crate::charset::Charset;
use crate::display::{LutTable, Mapping, Modality, ModalityFrame, Voi, VoiFunction};
use crate::graphics::{OverlayPlane, Shutter};
use crate::parse::{DataSet, Values};
use crate::pixel::{Grey, Pixels, read_lut};
use crate::{Dicom, Error, Result};

/// SOP Class UID of Grayscale Softcopy Presentation State Storage.
pub const GSPS_SOP_CLASS: &str = "1.2.840.10008.5.1.4.1.1.11.1";

const REFERENCED_SERIES: u32 = 0x0008_1115;
const REFERENCED_IMAGE: u32 = 0x0008_1140;
const REFERENCED_SOP_INSTANCE: u32 = 0x0008_1155;
const REFERENCED_FRAME_NUMBER: u32 = 0x0008_1160;

/// An image (and some of its frames) a presentation state or one of its parts applies to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageRef {
    pub sop_uid: String,
    /// Frame numbers (from 1); empty: every frame.
    pub frames: Vec<u32>,
}

/// Whether `refs` (empty: every image of the state) include frame `frame` (from 0) of
/// image `sop`.
fn applies(refs: &[ImageRef], sop: &str, frame: u32) -> bool {
    refs.is_empty()
        || refs
            .iter()
            .any(|r| r.sop_uid == sop && (r.frames.is_empty() || r.frames.contains(&(frame + 1))))
}

fn text(ds: &DataSet, v: &Values, tag: u32) -> Option<String> {
    ds.get(tag)
        .and_then(|e| std::str::from_utf8(v.text_bytes(e)).ok())
        .map(|s| s.trim_end_matches('\0').trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn image_refs(ds: &DataSet, v: &Values) -> Vec<ImageRef> {
    let Some(e) = ds.get(REFERENCED_IMAGE) else {
        return Vec::new();
    };
    e.items
        .iter()
        .filter_map(|it| {
            let sop_uid = text(it, v, REFERENCED_SOP_INSTANCE)?;
            let frames = it
                .get(REFERENCED_FRAME_NUMBER)
                .map(|f| {
                    (0..v.count(f).max(1))
                        .filter_map(|i| v.number(f, i))
                        .filter(|n| *n >= 1.0 && *n <= u32::MAX as f64)
                        .map(|n| n as u32)
                        .collect()
                })
                .unwrap_or_default();
            Some(ImageRef { sop_uid, frames })
        })
        .collect()
}

/// The images of a presentation state's Referenced Series Sequence.
fn series_refs(ds: &DataSet, v: &Values) -> Vec<ImageRef> {
    ds.get(REFERENCED_SERIES)
        .map(|e| e.items.iter().flat_map(|s| image_refs(s, v)).collect())
        .unwrap_or_default()
}

/// The SOP Instance UIDs a presentation state's data set references (for series scans).
pub(crate) fn referenced_sops(ds: &DataSet, v: &Values) -> Vec<String> {
    series_refs(ds, v).into_iter().map(|r| r.sop_uid).collect()
}

/// Units of annotation coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Units {
    /// Stored image pixels (also MATRIX, read as the image's pixels).
    Pixel,
    /// Fractions of the displayed area as shown.
    Display,
}

impl Units {
    fn read(ds: &DataSet, v: &Values, tag: u32) -> Units {
        match text(ds, v, tag).as_deref() {
            Some("DISPLAY") => Units::Display,
            _ => Units::Pixel,
        }
    }
}

/// Graphic Type (0070,0023).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphicKind {
    Point,
    Polyline,
    Interpolated,
    Circle,
    Ellipse,
}

/// A graphic object.
#[derive(Clone, Debug, PartialEq)]
pub struct Graphic {
    pub units: Units,
    pub kind: GraphicKind,
    /// (column, row) points.
    pub points: Vec<(f64, f64)>,
    pub filled: bool,
    pub style: Style,
    /// Compound Graphic Instance ID: part of the plain rendering of a compound graphic.
    pub compound: Option<u32>,
}

/// Line, fill or text style of an object (Line / Fill / Text Style Sequences).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Style {
    /// Pattern On Color (lines), Text Color (text), sRGB.
    pub colour: Option<[u8; 3]>,
    /// The fill's Pattern On Color, sRGB.
    pub fill: Option<[u8; 3]>,
    /// Line Thickness, in the object's units.
    pub thickness: Option<f64>,
    /// Line Dashing Style DASHED.
    pub dashed: bool,
    /// Fill Pattern On Opacity, 0..=1.
    pub opacity: Option<f64>,
}

impl Style {
    fn read(o: &DataSet, v: &Values) -> Style {
        let lab = |d: &DataSet, tag| -> Option<[u8; 3]> {
            let e = d.get(tag)?;
            let c: Vec<u16> = v.u16s(e).collect();
            match c[..] {
                [l, a, b] => Some(cielab_to_srgb(l, a, b)),
                _ => None,
            }
        };
        let num = |d: &DataSet, tag| d.get(tag).and_then(|e| v.number(e, 0));
        let mut s = Style::default();
        if let Some(l) = o.item(0x0070_0232) {
            s.colour = lab(l, 0x0070_0251);
            s.thickness = num(l, 0x0070_0253).filter(|t| *t > 0.0);
            s.dashed = text(l, v, 0x0070_0254).as_deref() == Some("DASHED");
        }
        if let Some(t) = o.item(0x0070_0231) {
            s.colour = lab(t, 0x0070_0241);
        }
        if let Some(f) = o.item(0x0070_0233) {
            s.fill = lab(f, 0x0070_0251);
            s.opacity = num(f, 0x0070_0284).map(|x| x.clamp(0.0, 1.0));
        }
        s
    }
}

impl Graphic {
    /// The outline in the graphic's units, and whether it is closed: circles and ellipses
    /// as polygons, interpolated curves as Catmull–Rom splines through their points, a
    /// polyline that ends where it starts as a closed polygon. Points are their own outline
    /// (not closed).
    pub fn outline(&self) -> (Vec<(f64, f64)>, bool) {
        let p = &self.points;
        let ring = |c: (f64, f64), a: f64, b: f64, angle: f64| {
            let (s, k) = angle.sin_cos();
            let n = 96;
            (0..n)
                .map(|i| {
                    let t = i as f64 / n as f64 * std::f64::consts::TAU;
                    let (x, y) = (a * t.cos(), b * t.sin());
                    (c.0 + x * k - y * s, c.1 + x * s + y * k)
                })
                .collect::<Vec<_>>()
        };
        match self.kind {
            GraphicKind::Point => (p.clone(), false),
            GraphicKind::Circle => match p[..] {
                [c, e, ..] => (ring(c, dist(c, e), dist(c, e), 0.0), true),
                _ => (Vec::new(), false),
            },
            GraphicKind::Ellipse => match p[..] {
                [a0, a1, b0, b1, ..] => {
                    let c = ((a0.0 + a1.0) / 2.0, (a0.1 + a1.1) / 2.0);
                    let angle = (a1.1 - a0.1).atan2(a1.0 - a0.0);
                    (ring(c, dist(a0, a1) / 2.0, dist(b0, b1) / 2.0, angle), true)
                }
                _ => (Vec::new(), false),
            },
            GraphicKind::Polyline | GraphicKind::Interpolated => {
                let closed = p.len() > 2 && p.first() == p.last();
                let mut pts = p.clone();
                if closed {
                    pts.pop();
                }
                if self.kind == GraphicKind::Interpolated && pts.len() > 2 {
                    pts = spline(&pts, closed);
                }
                (pts, closed)
            }
        }
    }
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// A centripetal-free (uniform) Catmull–Rom curve through `p`, 12 steps a segment.
fn spline(p: &[(f64, f64)], closed: bool) -> Vec<(f64, f64)> {
    let n = p.len();
    let at = |i: isize| -> (f64, f64) {
        if closed {
            p[i.rem_euclid(n as isize) as usize]
        } else {
            p[i.clamp(0, n as isize - 1) as usize]
        }
    };
    let segs = if closed { n } else { n - 1 };
    let steps = 12;
    let mut out = Vec::with_capacity(segs * steps + 1);
    for i in 0..segs as isize {
        let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        for k in 0..steps {
            let t = k as f64 / steps as f64;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f64, b: f64, c: f64, d: f64| {
                0.5 * (2.0 * b
                    + (c - a) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                    + (3.0 * b - a - 3.0 * c + d) * t3)
            };
            out.push((f(p0.0, p1.0, p2.0, p3.0), f(p0.1, p1.1, p2.1, p3.1)));
        }
    }
    if !closed {
        out.push(p[n - 1]);
    }
    out
}

/// Bounding Box Text Horizontal Justification (0070,0012).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Justify {
    Left,
    Right,
    Center,
}

/// Vertical Alignment (0070,0243) of text in its bounding box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VAlign {
    #[default]
    Top,
    Center,
    Bottom,
}

/// A point (column, row) in its units.
pub type Point = (f64, f64);

/// A text object: in a bounding box, at an anchor point, or both.
#[derive(Clone, Debug, PartialEq)]
pub struct TextObject {
    /// Lines of the Unformatted Text Value.
    pub lines: Vec<String>,
    /// Units, top-left and bottom-right corners.
    pub bbox: Option<(Units, Point, Point)>,
    pub justify: Justify,
    /// Units, the point, and whether a line to it is drawn.
    pub anchor: Option<(Units, Point, bool)>,
    /// Text Color CIELab Value, as sRGB.
    pub colour: Option<[u8; 3]>,
    pub valign: VAlign,
    /// Compound Graphic Instance ID: part of the plain rendering of a compound graphic.
    pub compound: Option<u32>,
}

/// One item of the Graphic Annotation Sequence.
#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    pub layer: String,
    pub refs: Vec<ImageRef>,
    pub graphics: Vec<Graphic>,
    pub texts: Vec<TextObject>,
    pub compounds: Vec<Compound>,
}

/// A graphic layer.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub name: String,
    pub order: i64,
    /// The recommended colour (from CIELab, or the retired RGB), sRGB.
    pub colour: Option<[u8; 3]>,
    /// The recommended grey, 0..=65535.
    pub grey: Option<u16>,
    pub description: Option<String>,
}

impl Layer {
    /// The layer's colour as sRGB: its colour, else its grey.
    pub fn srgb(&self) -> Option<[u8; 3]> {
        self.colour.or_else(|| {
            self.grey.map(|g| {
                let x = (g as u32 * 255 + 32767) / 65535;
                [x as u8; 3]
            })
        })
    }
}

/// Presentation Size Mode (0070,0100).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SizeMode {
    ScaleToFit,
    /// Physical size, with the Presentation Pixel Spacing (row, column spacing in mm).
    TrueSize(Option<(f64, f64)>),
    /// Screen pixels per image pixel.
    Magnify(f64),
}

/// A Displayed Area Selection item.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayedArea {
    pub refs: Vec<ImageRef>,
    /// Top-left and bottom-right hand corners, pixel numbers from 1 (column, row); the
    /// corners of the area as shown, so after a rotation the "top left" may be the bottom
    /// right of the stored image.
    pub tlhc: (i64, i64),
    pub brhc: (i64, i64),
    pub mode: SizeMode,
    /// Presentation Pixel Aspect Ratio: a pixel's height over its width (1 when absent).
    pub aspect: f64,
}

impl DisplayedArea {
    /// The area in stored coordinates: x0, y0, x1, y1 (pixel edges, x0 < x1, y0 < y1).
    pub fn stored_rect(&self) -> (f64, f64, f64, f64) {
        let (a, b) = (self.tlhc, self.brhc);
        (
            (a.0.min(b.0) - 1) as f64,
            (a.1.min(b.1) - 1) as f64,
            a.0.max(b.0) as f64,
            a.1.max(b.1) as f64,
        )
    }
}

/// The VOI of a Softcopy VOI LUT item.
#[derive(Clone, Debug, PartialEq)]
pub struct SoftcopyVoi {
    pub refs: Vec<ImageRef>,
    pub voi: Voi,
}

/// Presentation LUT Shape (2050,0020).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Identity,
    Inverse,
}

/// A Grayscale Softcopy Presentation State.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PresentationState {
    pub sop_uid: String,
    /// Content Label (0070,0080).
    pub label: String,
    /// Content Description (0070,0081).
    pub description: Option<String>,
    /// Presentation Creation Date and Time, `YYYYMMDDHHMMSS…` (sorts by time).
    pub created: String,
    pub references: Vec<ImageRef>,
    /// The state's modality transform; `None`: identity.
    pub modality: Option<Modality>,
    pub rescale_type: Option<String>,
    pub vois: Vec<SoftcopyVoi>,
    pub shape: Option<Shape>,
    pub plut: Option<LutTable>,
    /// Clockwise degrees (0, 90, 180, 270), applied after the flip.
    pub rotation: u16,
    pub flip: bool,
    pub areas: Vec<DisplayedArea>,
    pub layers: Vec<Layer>,
    pub annotations: Vec<Annotation>,
    /// Display shutter (the image's own is not shown under the state).
    pub shutter: Option<Shutter>,
    /// The state's own overlay planes.
    pub overlays: Vec<OverlayPlane>,
    /// Overlay Activation Layer: overlay group and the graphic layer it shows on.
    pub activations: Vec<(u16, String)>,
}

impl PresentationState {
    /// Parses a presentation state file. `Err(Unsupported)` for other SOP classes.
    pub fn parse(data: &[u8]) -> Result<PresentationState> {
        let d = Dicom::parse(data)?;
        let v = d.values(data);
        PresentationState::read(&d.dataset, &v)
    }

    /// Reads a presentation state from its data set.
    pub fn read(ds: &DataSet, v: &Values) -> Result<PresentationState> {
        let class = text(ds, v, 0x0008_0016).unwrap_or_default();
        if class != GSPS_SOP_CLASS {
            return Err(Error::Unsupported(format!(
                "presentation state class {class}"
            )));
        }
        let cs = Charset::of(ds, v);
        let num = |d: &DataSet, tag, i| d.get(tag).and_then(|e| v.number(e, i));
        let slope = num(ds, 0x0028_1053, 0).filter(|s| *s != 0.0);
        let intercept = num(ds, 0x0028_1052, 0);
        let modality = match (slope, intercept) {
            (None, None) => ds
                .item(0x0028_3000)
                .and_then(|m| read_lut(m, v, 0x0028_3002, 0x0028_3006, None, false))
                .map(|l| Modality::Lut(LutTable::from(l, None))),
            (s, i) => Some(Modality::Rescale {
                slope: s.unwrap_or(1.0),
                intercept: i.unwrap_or(0.0),
            }),
        };
        let rescale_type = text(ds, v, 0x0028_1054)
            .or_else(|| ds.item(0x0028_3000).and_then(|m| text(m, v, 0x0028_3004)));
        let vois = ds
            .get(0x0028_3110)
            .map(|e| {
                e.items
                    .iter()
                    .filter_map(|it| {
                        let refs = image_refs(it, v);
                        let function = match text(it, v, 0x0028_1056).as_deref() {
                            Some("LINEAR_EXACT") => VoiFunction::LinearExact,
                            Some("SIGMOID") => VoiFunction::Sigmoid,
                            _ => VoiFunction::Linear,
                        };
                        let voi = match (num(it, 0x0028_1050, 0), num(it, 0x0028_1051, 0)) {
                            (Some(center), Some(width)) if width > 0.0 => Voi::Window {
                                center,
                                width,
                                function,
                            },
                            _ => Voi::Lut(LutTable::from(
                                it.item(0x0028_3010).and_then(|m| {
                                    read_lut(m, v, 0x0028_3002, 0x0028_3006, None, false)
                                })?,
                                None,
                            )),
                        };
                        Some(SoftcopyVoi { refs, voi })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let shape = match text(ds, v, 0x2050_0020).as_deref() {
            Some("INVERSE") => Some(Shape::Inverse),
            Some("IDENTITY") => Some(Shape::Identity),
            _ => None,
        };
        let plut = ds
            .item(0x2050_0010)
            .and_then(|m| read_lut(m, v, 0x0028_3002, 0x0028_3006, None, false))
            .map(|l| LutTable::from(l, None));
        let rotation = ds
            .get(0x0070_0042)
            .and_then(|e| v.number(e, 0))
            .map_or(0, |r| ((r as i64).rem_euclid(360) / 90 * 90) as u16);
        let flip = text(ds, v, 0x0070_0041).as_deref() == Some("Y");
        let areas = ds
            .get(0x0070_005A)
            .map(|e| {
                e.items
                    .iter()
                    .filter_map(|it| {
                        let tl = it.get(0x0070_0052)?;
                        let br = it.get(0x0070_0053)?;
                        let pt = |e| Some((v.number(e, 0)? as i64, v.number(e, 1)? as i64));
                        let mode = match text(it, v, 0x0070_0100).as_deref() {
                            Some("TRUE SIZE") => SizeMode::TrueSize(
                                num(it, 0x0070_0101, 0).zip(num(it, 0x0070_0101, 1)),
                            ),
                            Some("MAGNIFY") => SizeMode::Magnify(
                                num(it, 0x0070_0103, 0).filter(|r| *r > 0.0).unwrap_or(1.0),
                            ),
                            _ => SizeMode::ScaleToFit,
                        };
                        // Vertical / horizontal; with a Presentation Pixel Spacing, its row /
                        // column spacing.
                        let aspect = match (num(it, 0x0070_0102, 0), num(it, 0x0070_0102, 1)) {
                            (Some(a), Some(b)) => Some((a, b)),
                            _ => num(it, 0x0070_0101, 0).zip(num(it, 0x0070_0101, 1)),
                        }
                        .filter(|(a, b)| *a > 0.0 && *b > 0.0)
                        .map_or(1.0, |(a, b)| a / b);
                        Some(DisplayedArea {
                            refs: image_refs(it, v),
                            tlhc: pt(tl)?,
                            brhc: pt(br)?,
                            mode,
                            aspect,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let layers = ds
            .get(0x0070_0060)
            .map(|e| {
                e.items
                    .iter()
                    .filter_map(|it| {
                        let us = |tag| -> Vec<u16> {
                            it.get(tag).map(|e| v.u16s(e).collect()).unwrap_or_default()
                        };
                        let lab = us(0x0070_0401);
                        let rgb = us(0x0070_0067);
                        let colour = if let [l, a, b] = lab[..] {
                            Some(cielab_to_srgb(l, a, b))
                        } else if let [r, g, b] = rgb[..] {
                            Some([r, g, b].map(|c| ((c as u32 * 255 + 32767) / 65535) as u8))
                        } else {
                            None
                        };
                        Some(Layer {
                            name: text(it, v, 0x0070_0002)?,
                            order: num(it, 0x0070_0062, 0).map_or(0, |o| o as i64),
                            colour,
                            grey: us(0x0070_0066).first().copied(),
                            description: text(it, v, 0x0070_0068),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let points = |it: &DataSet, tag| -> Option<(f64, f64)> {
            Some((num(it, tag, 0)?, num(it, tag, 1)?))
        };
        let annotations = ds
            .get(0x0070_0001)
            .map(|e| {
                e.items
                    .iter()
                    .map(|it| {
                        let graphics = it
                            .get(0x0070_0009)
                            .map(|g| g.items.iter().filter_map(|o| read_graphic(o, v)).collect())
                            .unwrap_or_default();
                        let texts = it
                            .get(0x0070_0008)
                            .map(|t| {
                                t.items
                                    .iter()
                                    .filter_map(|o| {
                                        let raw = o
                                            .get(0x0070_0006)
                                            .map(|e| cs.decode(v.bytes(e), e.vr))
                                            .unwrap_or_default();
                                        let lines: Vec<String> = raw
                                            .trim_end_matches(['\0', ' '])
                                            .replace("\r\n", "\n")
                                            .replace('\r', "\n")
                                            .lines()
                                            .map(|l| l.trim_end().to_owned())
                                            .collect();
                                        let bbox = points(o, 0x0070_0010)
                                            .zip(points(o, 0x0070_0011))
                                            .map(|(a, b)| (Units::read(o, v, 0x0070_0003), a, b));
                                        let anchor = points(o, 0x0070_0014).map(|p| {
                                            (
                                                Units::read(o, v, 0x0070_0004),
                                                p,
                                                text(o, v, 0x0070_0015).as_deref() == Some("Y"),
                                            )
                                        });
                                        let style = o.item(0x0070_0231);
                                        // Horizontal Alignment overrides the justification.
                                        let justify = match style
                                            .and_then(|t| text(t, v, 0x0070_0242))
                                            .or_else(|| text(o, v, 0x0070_0012))
                                            .as_deref()
                                        {
                                            Some("RIGHT") => Justify::Right,
                                            Some("CENTER") => Justify::Center,
                                            _ => Justify::Left,
                                        };
                                        let valign = match style
                                            .and_then(|t| text(t, v, 0x0070_0243))
                                            .as_deref()
                                        {
                                            Some("CENTER") => VAlign::Center,
                                            Some("BOTTOM") => VAlign::Bottom,
                                            _ => VAlign::Top,
                                        };
                                        (bbox.is_some() || anchor.is_some()).then(|| TextObject {
                                            lines,
                                            bbox,
                                            justify,
                                            anchor,
                                            colour: Style::read(o, v).colour,
                                            valign,
                                            compound: compound_id(o, v),
                                        })
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        let compounds = it
                            .get(0x0070_0209)
                            .map(|g| g.items.iter().filter_map(|o| read_compound(o, v)).collect())
                            .unwrap_or_default();
                        Annotation {
                            layer: text(it, v, 0x0070_0002).unwrap_or_default(),
                            refs: image_refs(it, v),
                            graphics,
                            texts,
                            compounds,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut annotations = annotations;
        // Graphics and texts that render a compound graphic the plain way give way to it.
        let ids: Vec<u32> = annotations
            .iter()
            .flat_map(|a| a.compounds.iter().filter_map(|c| c.id))
            .collect();
        for a in &mut annotations {
            a.graphics
                .retain(|g| g.compound.is_none_or(|i| !ids.contains(&i)));
            a.texts
                .retain(|t| t.compound.is_none_or(|i| !ids.contains(&i)));
        }
        let activations = (0x6000u32..=0x601E)
            .step_by(2)
            .filter_map(|g| Some((g as u16, text(ds, v, g << 16 | 0x1001)?)))
            .collect();
        Ok(PresentationState {
            sop_uid: text(ds, v, 0x0008_0018).unwrap_or_default(),
            label: ds
                .get(0x0070_0080)
                .map(|e| cs.decode(v.text_bytes(e), e.vr).trim().to_owned())
                .unwrap_or_default(),
            description: ds
                .get(0x0070_0081)
                .map(|e| cs.decode(v.text_bytes(e), e.vr).trim().to_owned())
                .filter(|s| !s.is_empty()),
            created: format!(
                "{}{}",
                text(ds, v, 0x0070_0082).unwrap_or_default(),
                text(ds, v, 0x0070_0083).unwrap_or_default()
            ),
            references: series_refs(ds, v),
            modality,
            rescale_type,
            vois,
            shape,
            plut,
            rotation,
            flip,
            areas,
            layers,
            annotations,
            shutter: Shutter::read(ds, v),
            overlays: crate::graphics::owned_planes(ds, v),
            activations,
        })
    }

    /// Whether the state applies to frame `frame` (from 0) of image `sop`.
    pub fn refers_to(&self, sop: &str, frame: u32) -> bool {
        self.references
            .iter()
            .any(|r| r.sop_uid == sop && (r.frames.is_empty() || r.frames.contains(&(frame + 1))))
    }

    /// The VOI for a frame; `None`: identity.
    pub fn voi_for(&self, sop: &str, frame: u32) -> Option<&Voi> {
        self.vois
            .iter()
            .find(|x| applies(&x.refs, sop, frame))
            .map(|x| &x.voi)
    }

    /// The displayed area for a frame.
    pub fn area_for(&self, sop: &str, frame: u32) -> Option<&DisplayedArea> {
        self.areas.iter().find(|a| applies(&a.refs, sop, frame))
    }

    /// The annotations shown on a frame, in layer order (bottom first).
    pub fn annotations_for<'a>(&'a self, sop: &'a str, frame: u32) -> Vec<&'a Annotation> {
        let mut out: Vec<&Annotation> = self
            .annotations
            .iter()
            .filter(|a| applies(&a.refs, sop, frame))
            .collect();
        out.sort_by_key(|a| self.layer(&a.layer).map_or(i64::MAX, |l| l.order));
        out
    }

    pub fn layer(&self, name: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.name == name)
    }

    /// Quarter turns clockwise and the horizontal flip (applied first).
    pub fn rotation_flip(&self) -> (u8, bool) {
        ((self.rotation / 90 % 4) as u8, self.flip)
    }
}

fn read_graphic(o: &DataSet, v: &Values) -> Option<Graphic> {
    let kind = match text(o, v, 0x0070_0023)?.as_str() {
        "POINT" => GraphicKind::Point,
        "POLYLINE" => GraphicKind::Polyline,
        "INTERPOLATED" => GraphicKind::Interpolated,
        "CIRCLE" => GraphicKind::Circle,
        "ELLIPSE" => GraphicKind::Ellipse,
        _ => return None,
    };
    let data = o.get(0x0070_0022)?;
    let n = data.value.len() / 4;
    let declared = o
        .get(0x0070_0021)
        .and_then(|e| v.int(e))
        .map_or(n / 2, |k| k.max(0) as usize);
    let mut points = Vec::with_capacity(declared.min(n / 2));
    for i in 0..declared.min(n / 2) {
        points.push((v.number(data, 2 * i)?, v.number(data, 2 * i + 1)?));
    }
    let need = match kind {
        GraphicKind::Point => 1,
        GraphicKind::Circle | GraphicKind::Polyline | GraphicKind::Interpolated => 2,
        GraphicKind::Ellipse => 4,
    };
    (points.len() >= need).then(|| Graphic {
        units: Units::read(o, v, 0x0070_0005),
        kind,
        points,
        filled: text(o, v, 0x0070_0024).as_deref() == Some("Y"),
        style: Style::read(o, v),
        compound: compound_id(o, v),
    })
}

fn compound_id(o: &DataSet, v: &Values) -> Option<u32> {
    o.get(0x0070_0226)
        .and_then(|e| v.int(e))
        .and_then(|i| u32::try_from(i).ok())
}

/// Compound Graphic Type (0070,0294, PS3.3 C.10.5.1.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompoundKind {
    Multiline,
    InfiniteLine,
    Cutline,
    RangeLine,
    Ruler,
    Axis,
    Crosshair,
    Arrow,
    Rectangle,
    Ellipse,
}

/// Tick Alignment (0070,0274): which side of the line ticks are on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TickAlign {
    Bottom,
    #[default]
    Center,
    Top,
}

/// An item of the Major Ticks Sequence.
#[derive(Clone, Debug, PartialEq)]
pub struct Tick {
    /// Tick Position, 0 (first point) to 1 (second point).
    pub at: f64,
    pub label: Option<String>,
}

/// An item of the Compound Graphic Sequence.
#[derive(Clone, Debug, PartialEq)]
pub struct Compound {
    pub units: Units,
    pub kind: CompoundKind,
    /// (column, row) points.
    pub points: Vec<Point>,
    /// Rotation Angle, degrees counter-clockwise (as stored: rows down) about the pivot.
    pub rotation: f64,
    /// Rotation Point.
    pub pivot: Option<Point>,
    /// Gap Length: diameter of the circle (about the pivot or crosshair origin) left out,
    /// in DISPLAY units.
    pub gap: Option<f64>,
    /// Diameter of Visibility of a crosshair, in DISPLAY units.
    pub visible: Option<f64>,
    pub ticks: Vec<Tick>,
    pub tick_align: TickAlign,
    /// Show Tick Label.
    pub show_labels: bool,
    /// Tick Label Alignment BOTTOM (else TOP).
    pub labels_below: bool,
    pub filled: bool,
    pub style: Style,
    /// Compound Graphic Instance ID.
    pub id: Option<u32>,
}

/// Sizes a compound graphic is drawn at, in its own units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sizes {
    /// One DISPLAY unit (Gap Length and Diameter of Visibility are in DISPLAY units).
    pub display: f64,
    /// How far an infinite line reaches either side of its midpoint (past the view).
    pub reach: f64,
    /// Arrowhead and tick length.
    pub head: f64,
}

/// A line or outline of a drawn compound graphic.
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub points: Vec<Point>,
    pub closed: bool,
    pub filled: bool,
}

/// A tick label of a drawn compound graphic: at `at`, the text above it (or below,
/// `below`).
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub at: Point,
    /// Where its tick meets the line: the text goes on the far side of `at` from it.
    pub tick: Point,
    pub text: String,
    pub below: bool,
}

fn add(a: Point, b: Point) -> Point {
    (a.0 + b.0, a.1 + b.1)
}

fn sub(a: Point, b: Point) -> Point {
    (a.0 - b.0, a.1 - b.1)
}

fn mul(a: Point, k: f64) -> Point {
    (a.0 * k, a.1 * k)
}

fn lerp(a: Point, b: Point, t: f64) -> Point {
    add(a, mul(sub(b, a), t))
}

/// Unit vector from `a` to `b` ((1, 0) when they meet).
fn unit(a: Point, b: Point) -> Point {
    let d = dist(a, b);
    if d > 0.0 {
        mul(sub(b, a), 1.0 / d)
    } else {
        (1.0, 0.0)
    }
}

/// The parameters (0..=1 along a→b) where the segment meets the circle about `c` of radius
/// `r`: entry and exit, when it crosses.
fn circle_hits(a: Point, b: Point, c: Point, r: f64) -> Option<(f64, f64)> {
    let d = sub(b, a);
    let f = sub(a, c);
    let qa = d.0 * d.0 + d.1 * d.1;
    let qb = 2.0 * (f.0 * d.0 + f.1 * d.1);
    let qc = f.0 * f.0 + f.1 * f.1 - r * r;
    let disc = qb * qb - 4.0 * qa * qc;
    if qa <= 0.0 || r <= 0.0 || disc <= 0.0 {
        return None;
    }
    let s = disc.sqrt();
    Some(((-qb - s) / (2.0 * qa), (-qb + s) / (2.0 * qa)))
}

/// The parts of segment a→b outside the circle about `c` of radius `r`.
fn outside(a: Point, b: Point, c: Point, r: f64) -> Vec<Stroke> {
    let line = |p: Point, q: Point| Stroke {
        points: vec![p, q],
        closed: false,
        filled: false,
    };
    let Some((t0, t1)) = circle_hits(a, b, c, r) else {
        return vec![line(a, b)];
    };
    let mut out = Vec::new();
    if t0 > 0.0 {
        out.push(line(a, lerp(a, b, t0.min(1.0))));
    }
    if t1 < 1.0 {
        out.push(line(lerp(a, b, t1.max(0.0)), b));
    }
    out
}

/// A filled arrowhead with its tip at `tip`, pointing away from `from`.
fn head(from: Point, tip: Point, size: f64) -> Stroke {
    let u = unit(from, tip);
    let n = (-u.1, u.0);
    let back = sub(tip, mul(u, size));
    Stroke {
        points: vec![
            add(back, mul(n, size * 0.4)),
            tip,
            sub(back, mul(n, size * 0.4)),
        ],
        closed: true,
        filled: true,
    }
}

impl Compound {
    /// The compound graphic drawn as strokes and tick labels in its units. RANGELINE, whose
    /// look the standard leaves to a figure, is drawn as the line with a bar across each end.
    pub fn draw(&self, s: &Sizes) -> (Vec<Stroke>, Vec<Label>) {
        let p = &self.points;
        let mut strokes = Vec::new();
        let mut labels = Vec::new();
        let line = |pts: Vec<Point>| Stroke {
            points: pts,
            closed: false,
            filled: false,
        };
        let gap = self.gap.unwrap_or(0.0) * s.display / 2.0;
        let (a, b) = (p[0], *p.get(1).unwrap_or(&p[0]));
        let mid = lerp(a, b, 0.5);
        let u = unit(a, b);
        // "Up" from the line, rows growing down.
        let up = (u.1, -u.0);
        match self.kind {
            CompoundKind::Multiline => strokes.push(line(p.clone())),
            CompoundKind::InfiniteLine => {
                let (x, y) = (sub(mid, mul(u, s.reach)), add(mid, mul(u, s.reach)));
                strokes.extend(outside(x, y, self.pivot.unwrap_or(mid), gap));
            }
            CompoundKind::Cutline => {
                strokes.extend(outside(a, b, self.pivot.unwrap_or(mid), gap));
                // Arrows in the right half-plane, ending at each half's midpoint.
                let right = mul(up, -1.0);
                for q in [lerp(a, b, 0.25), lerp(a, b, 0.75)] {
                    let tail = add(q, mul(right, s.head * 3.0));
                    strokes.push(line(vec![tail, q]));
                    strokes.push(head(tail, q, s.head));
                }
            }
            CompoundKind::RangeLine => {
                strokes.push(line(vec![a, b]));
                for q in [a, b] {
                    strokes.push(line(vec![add(q, mul(up, s.head)), sub(q, mul(up, s.head))]));
                }
            }
            CompoundKind::Ruler | CompoundKind::Axis => {
                strokes.push(line(vec![a, b]));
                let ticks: Vec<Tick> = if self.ticks.is_empty() && self.kind == CompoundKind::Ruler
                {
                    (0..=10)
                        .map(|i| Tick {
                            at: f64::from(i) / 10.0,
                            label: None,
                        })
                        .collect()
                } else {
                    self.ticks.clone()
                };
                for t in &ticks {
                    let q = lerp(a, b, t.at.clamp(0.0, 1.0));
                    let (from, to) = match self.tick_align {
                        TickAlign::Top => (q, add(q, mul(up, s.head))),
                        TickAlign::Bottom => (q, sub(q, mul(up, s.head))),
                        TickAlign::Center => {
                            (sub(q, mul(up, s.head / 2.0)), add(q, mul(up, s.head / 2.0)))
                        }
                    };
                    strokes.push(line(vec![from, to]));
                    if let Some(text) = t.label.clone().filter(|_| self.show_labels) {
                        let off = s.head * 1.2;
                        let at = if self.labels_below {
                            sub(q, mul(up, off))
                        } else {
                            add(q, mul(up, off))
                        };
                        labels.push(Label {
                            at,
                            tick: q,
                            text,
                            below: self.labels_below,
                        });
                    }
                }
            }
            CompoundKind::Crosshair => {
                let reach = self
                    .visible
                    .map_or(s.head * 3.0, |d| d * s.display / 2.0)
                    .max(gap);
                for d in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                    strokes.push(line(vec![add(a, mul(d, gap)), add(a, mul(d, reach))]));
                }
            }
            CompoundKind::Arrow => {
                strokes.push(line(vec![b, a]));
                strokes.push(head(b, a, s.head));
            }
            CompoundKind::Rectangle => strokes.push(Stroke {
                points: vec![a, (b.0, a.1), b, (a.0, b.1)],
                closed: true,
                filled: self.filled,
            }),
            CompoundKind::Ellipse => {
                let (rx, ry) = ((b.0 - a.0).abs() / 2.0, (b.1 - a.1).abs() / 2.0);
                strokes.push(Stroke {
                    points: (0..96)
                        .map(|i| {
                            let t = f64::from(i) / 96.0 * std::f64::consts::TAU;
                            (mid.0 + rx * t.cos(), mid.1 + ry * t.sin())
                        })
                        .collect(),
                    closed: true,
                    filled: self.filled,
                });
            }
        }
        if self.rotation != 0.0 {
            let c = self.pivot.unwrap_or_else(|| {
                let n = p.len() as f64;
                p.iter()
                    .fold((0.0, 0.0), |acc, q| add(acc, mul(*q, 1.0 / n)))
            });
            let (sn, cs) = self.rotation.to_radians().sin_cos();
            let turn = |q: Point| {
                let d = sub(q, c);
                (c.0 + d.0 * cs + d.1 * sn, c.1 - d.0 * sn + d.1 * cs)
            };
            for st in &mut strokes {
                st.points.iter_mut().for_each(|q| *q = turn(*q));
            }
            for l in &mut labels {
                l.at = turn(l.at);
                l.tick = turn(l.tick);
            }
        }
        (strokes, labels)
    }
}

fn read_compound(o: &DataSet, v: &Values) -> Option<Compound> {
    let kind = match text(o, v, 0x0070_0294)?.as_str() {
        "MULTILINE" => CompoundKind::Multiline,
        "INFINITELINE" => CompoundKind::InfiniteLine,
        "CUTLINE" => CompoundKind::Cutline,
        "RANGELINE" => CompoundKind::RangeLine,
        "RULER" => CompoundKind::Ruler,
        "AXIS" => CompoundKind::Axis,
        "CROSSHAIR" => CompoundKind::Crosshair,
        "ARROW" => CompoundKind::Arrow,
        "RECTANGLE" => CompoundKind::Rectangle,
        "ELLIPSE" => CompoundKind::Ellipse,
        _ => return None,
    };
    let data = o.get(0x0070_0022)?;
    let points: Vec<Point> = (0..4096)
        .map_while(|i| Some((v.number(data, 2 * i)?, v.number(data, 2 * i + 1)?)))
        .collect();
    let need = match kind {
        CompoundKind::Crosshair => 1,
        _ => 2,
    };
    if points.len() < need {
        return None;
    }
    let num = |tag| o.get(tag).and_then(|e| v.number(e, 0));
    let pivot = o
        .get(0x0070_0273)
        .and_then(|e| Some((v.number(e, 0)?, v.number(e, 1)?)));
    let ticks = o
        .get(0x0070_0287)
        .map(|e| {
            e.items
                .iter()
                .filter_map(|t| {
                    Some(Tick {
                        at: t.get(0x0070_0288).and_then(|e| v.number(e, 0))?,
                        label: text(t, v, 0x0070_0289),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(Compound {
        units: Units::read(o, v, 0x0070_0282),
        kind,
        points,
        rotation: num(0x0070_0230).unwrap_or(0.0),
        pivot,
        gap: num(0x0070_0261).filter(|g| *g > 0.0),
        visible: num(0x0070_0262).filter(|g| *g > 0.0),
        ticks,
        tick_align: match text(o, v, 0x0070_0274).as_deref() {
            Some("TOP") => TickAlign::Top,
            Some("BOTTOM") => TickAlign::Bottom,
            _ => TickAlign::Center,
        },
        show_labels: text(o, v, 0x0070_0278).as_deref() != Some("N"),
        labels_below: text(o, v, 0x0070_0279).as_deref() == Some("BOTTOM"),
        filled: text(o, v, 0x0070_0024).as_deref() == Some("Y"),
        style: Style::read(o, v),
        id: compound_id(o, v),
    })
}

/// DICOM's scaled CIELab (PS3.3 C.10.7.1.1: L 0..65535 for 0..100, a and b 0..65535 for
/// −128..127, D50) to 8-bit sRGB (Bradford-adapted to D65).
pub fn cielab_to_srgb(l: u16, a: u16, b: u16) -> [u8; 3] {
    let l = l as f64 * 100.0 / 65535.0;
    let a = a as f64 * 255.0 / 65535.0 - 128.0;
    let b = b as f64 * 255.0 / 65535.0 - 128.0;
    let fy = (l + 16.0) / 116.0;
    let (fx, fz) = (fy + a / 500.0, fy - b / 200.0);
    let inv = |t: f64| {
        if t > 6.0 / 29.0 {
            t * t * t
        } else {
            3.0 * (6.0f64 / 29.0).powi(2) * (t - 4.0 / 29.0)
        }
    };
    // D50 white.
    let (x, y, z) = (0.964_22 * inv(fx), inv(fy), 0.825_21 * inv(fz));
    // XYZ (D50) to linear sRGB, Bradford-adapted (Lindbloom).
    let r = 3.133_856_1 * x - 1.616_866_7 * y - 0.490_614_6 * z;
    let g = -0.978_768_4 * x + 1.916_141_5 * y + 0.033_454_0 * z;
    let bl = 0.071_945_3 * x - 0.228_991_4 * y + 1.405_242_7 * z;
    [r, g, bl].map(|c| {
        let c = c.clamp(0.0, 1.0);
        let e = if c <= 0.003_130_8 {
            12.92 * c
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        };
        (e * 255.0).round() as u8
    })
}

/// A LUT descriptor's first value read as US, as the SS it stands for when 0x8000 or more.
fn signed_first(first: i64) -> i64 {
    if (0x8000..0x1_0000).contains(&first) {
        first - 0x1_0000
    } else {
        first
    }
}

impl ModalityFrame {
    /// This frame (frame `frame` of its image) with `ps`'s display pipeline in place of the
    /// image's: its modality transform (identity when it has none), its VOI for the frame
    /// (identity over the possible modality values when it has none), and its Presentation
    /// LUT shape or table (MONOCHROME1 is no longer inverted on its own: the state's shape
    /// decides).
    pub fn presented(mut self, ps: &PresentationState, frame: u32) -> ModalityFrame {
        let sop = self.sop_uid.clone().unwrap_or_default();
        let ct = self.map.ct;
        let padding = (self.map.padding, self.map.float_padding);
        let mut modality = ps.modality.clone().unwrap_or(Modality::Rescale {
            slope: 1.0,
            intercept: 0.0,
        });
        // A LUT descriptor's first input value (US) is signed when its input can be
        // negative: stored values of a signed image, or modality values below zero.
        if let Modality::Lut(l) = &mut modality
            && self.frame.signed
        {
            l.first = signed_first(l.first);
        }
        let mut map = Mapping {
            modality,
            rescale_type: ps.rescale_type.clone(),
            windows: Vec::new(),
            explanations: Vec::new(),
            function: VoiFunction::LinearExact,
            voi_lut: None,
            invert: ps.shape == Some(Shape::Inverse),
            plut: ps.plut.clone(),
            padding: padding.0,
            float_padding: padding.1,
            ct,
        };
        match ps.voi_for(&sop, frame) {
            Some(Voi::Window {
                center,
                width,
                function,
            }) => {
                map.windows = vec![(*center, *width)];
                map.function = *function;
            }
            Some(Voi::Lut(l)) => {
                let mut l = l.clone();
                self.map = map.clone();
                if self.possible_range().0 < 0.0 {
                    l.first = signed_first(l.first);
                }
                map.voi_lut = Some(l);
            }
            None => {
                self.map = map.clone();
                let (lo, hi) = self.possible_range();
                map.windows = vec![((lo + hi) / 2.0, (hi - lo).max(f64::MIN_POSITIVE))];
            }
        }
        self.map = map;
        self.presentation = Some(ps.sop_uid.clone());
        self.graphics = crate::graphics::presented(
            self.graphics.as_ref(),
            ps,
            frame,
            self.frame.width,
            self.frame.height,
        );
        self
    }

    /// The modality values the frame's stored representation could hold (for an identity
    /// VOI): the stored bit range through the modality transform; for float data, the
    /// values' range.
    fn possible_range(&self) -> (f64, f64) {
        if matches!(
            &self.frame.pixels,
            Pixels::Grey(Grey::F32(_) | Grey::F64(_))
        ) {
            return self.value_range().unwrap_or((0.0, 1.0));
        }
        let b = self.frame.bits.clamp(1, 32) as u32;
        let (lo, hi) = if self.frame.signed {
            (-(1i64 << (b - 1)), (1i64 << (b - 1)) - 1)
        } else {
            (0, (1i64 << b) - 1)
        };
        match &self.map.modality {
            Modality::Rescale { .. } => {
                let (x, y) = (self.map.modality.apply(lo), self.map.modality.apply(hi));
                (x.min(y), x.max(y))
            }
            Modality::Lut(l) => (0.0, ((1u32 << l.bits) - 1) as f64),
        }
    }
}
