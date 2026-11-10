//! DICOM Segmentation objects (PS3.3 A.51): binary, fractional (probability / occupancy) and
//! labelmap segmentations, read into [`Labels`].
//!
//! The segments come from the Segment Sequence (number, label, recommended CIELab colour,
//! algorithm type). Each frame's segment, plane position and source image come from the
//! Per-frame Functional Groups (Segment Identification, Plane Position, Derivation Image);
//! orientation and pixel measures from the Shared ones (or the frame's own). Frames are laid
//! on a grid of planes along the slice normal; missing planes between frames are filled in
//! empty (so a gap never shows the neighbouring frame's mask).

use std::sync::atomic::AtomicBool;

use crate::charset::Charset;
use crate::labels::{
    Builder, DEFAULT_COLOURS, LabelSource, Labels, Segment, along, distinct, lab_to_srgb,
    plane_grid, voxel_ml,
};
use crate::parse::DataSet;
use crate::pixel::{Grey, Pixels};
use crate::volume::{cross, dot, normalise};
use crate::{Dicom, Error, Result, Values};

/// Segmentation Storage.
pub const SEG_SOP_CLASS: &str = "1.2.840.10008.5.1.4.1.1.66.4";
/// Label Map Segmentation Storage.
pub const LABELMAP_SOP_CLASS: &str = "1.2.840.10008.5.1.4.1.1.66.7";

const SOP_CLASS: u32 = 0x0008_0016;
const SEGMENTATION_TYPE: u32 = 0x0062_0001;
const SEGMENT_SEQUENCE: u32 = 0x0062_0002;
const SEGMENT_NUMBER: u32 = 0x0062_0004;
const SEGMENT_LABEL: u32 = 0x0062_0005;
const SEGMENT_ALGORITHM_TYPE: u32 = 0x0062_0008;
const SEGMENT_IDENTIFICATION: u32 = 0x0062_000A;
const REFERENCED_SEGMENT_NUMBER: u32 = 0x0062_000B;
const RECOMMENDED_CIELAB: u32 = 0x0062_000D;
const MAX_FRACTIONAL: u32 = 0x0062_000E;
const PER_FRAME: u32 = 0x5200_9230;
const SHARED: u32 = 0x5200_9229;
const PLANE_POSITION: u32 = 0x0020_9113;
const PLANE_ORIENTATION: u32 = 0x0020_9116;
const PIXEL_MEASURES: u32 = 0x0028_9110;
const POSITION: u32 = 0x0020_0032;
const ORIENTATION: u32 = 0x0020_0037;
const PIXEL_SPACING: u32 = 0x0028_0030;
const SPACING_BETWEEN: u32 = 0x0018_0088;
const SLICE_THICKNESS: u32 = 0x0018_0050;
const FRAME_OF_REFERENCE: u32 = 0x0020_0052;
const REFERENCED_SERIES: u32 = 0x0008_1115;
const SERIES_UID: u32 = 0x0020_000E;
const CONTENT_LABEL: u32 = 0x0070_0080;
const SERIES_DESCRIPTION: u32 = 0x0008_103E;
const PIXEL_PADDING: u32 = 0x0028_0120;

/// What the Segmentation Type says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SegType {
    Binary,
    /// Fractions of `max` (Maximum Fractional Value).
    Fractional {
        max: f64,
    },
    /// Pixel values are segment numbers.
    Labelmap,
}

/// Whether `sop_class` is a segmentation.
pub fn is_seg(sop_class: &str) -> bool {
    sop_class == SEG_SOP_CLASS || sop_class == LABELMAP_SOP_CLASS
}

fn text(ds: &DataSet, v: &Values, cs: &Charset, tag: u32) -> String {
    ds.get(tag)
        .map(|e| cs.decode(v.text_bytes(e), e.vr).trim().to_owned())
        .unwrap_or_default()
}

fn nums<const N: usize>(ds: &DataSet, v: &Values, tag: u32) -> Option<[f64; N]> {
    let e = ds.get(tag)?;
    let mut out = [0.0; N];
    for (i, o) in out.iter_mut().enumerate() {
        *o = v.number(e, i)?;
    }
    Some(out)
}

/// A functional group macro of frame `f`: the frame's own, else the shared one.
fn group(ds: &DataSet, f: usize, tag: u32) -> Option<&DataSet> {
    ds.get(PER_FRAME)
        .and_then(|e| e.items.get(f))
        .and_then(|it| it.item(tag))
        .or_else(|| ds.item(SHARED).and_then(|s| s.item(tag)))
}

/// One frame's place: its segment (Segment Number; 0 for labelmaps) and plane position.
#[derive(Clone, Debug, PartialEq)]
pub struct SegFrame {
    pub segment: i64,
    pub position: [f64; 3],
}

/// The header of a segmentation: segments, type and frame geometry, without pixel data.
#[derive(Clone, Debug, PartialEq)]
pub struct SegHeader {
    pub kind: SegType,
    pub segments: Vec<Segment>,
    pub frames: Vec<SegFrame>,
    pub row: [f64; 3],
    pub col: [f64; 3],
    /// mm between columns, between rows.
    pub pixel: [f64; 2],
    pub spacing: Option<f64>,
    pub size: [u32; 2],
    pub frame_of_reference: String,
    pub referenced_series: Vec<String>,
    pub title: String,
    /// Labelmaps: the Pixel Padding Value (no segment).
    pub padding: Option<i64>,
}

impl SegHeader {
    pub fn read(d: &Dicom, file: &[u8]) -> Result<SegHeader> {
        let ds = &d.dataset;
        let v = d.values(file);
        let cs = Charset::of(ds, &v);
        let class = text(ds, &v, &cs, SOP_CLASS);
        if !is_seg(&class) {
            return Err(Error::Unsupported("not a segmentation".into()));
        }
        let info = d.pixel.as_ref().ok_or(Error::NoPixelData)?;
        let kind = match text(ds, &v, &cs, SEGMENTATION_TYPE).as_str() {
            "FRACTIONAL" => SegType::Fractional {
                max: ds
                    .get(MAX_FRACTIONAL)
                    .and_then(|e| v.number(e, 0))
                    .filter(|m| *m > 0.0)
                    .unwrap_or(255.0),
            },
            "LABELMAP" => SegType::Labelmap,
            _ if class == LABELMAP_SOP_CLASS => SegType::Labelmap,
            _ => SegType::Binary,
        };
        let mut segments = Vec::new();
        for (i, it) in ds
            .get(SEGMENT_SEQUENCE)
            .map(|e| e.items.as_slice())
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            let number = it
                .get(SEGMENT_NUMBER)
                .and_then(|e| v.int(e))
                .unwrap_or(i as i64 + 1);
            let colour = it
                .get(RECOMMENDED_CIELAB)
                .map(|e| v.u16s(e).collect::<Vec<_>>())
                .filter(|c| c.len() == 3)
                .map_or(DEFAULT_COLOURS[i % DEFAULT_COLOURS.len()], |c| {
                    lab_to_srgb([c[0], c[1], c[2]])
                });
            segments.push(Segment {
                number,
                label: text(it, &v, &cs, SEGMENT_LABEL),
                colour,
                algorithm: text(it, &v, &cs, SEGMENT_ALGORITHM_TYPE),
                fractional: matches!(kind, SegType::Fractional { .. }),
                ml: 0.0,
                voxels: 0,
            });
        }
        if kind == SegType::Labelmap {
            // Segment 0 of a labelmap is the background, not a region.
            segments.retain(|s| s.number != 0);
        }
        if segments.is_empty() {
            return Err(Error::Malformed("segmentation without segments"));
        }
        let orientation = group(ds, 0, PLANE_ORIENTATION)
            .and_then(|g| nums::<6>(g, &v, ORIENTATION))
            .or_else(|| nums::<6>(ds, &v, ORIENTATION))
            .ok_or(Error::Unsupported(
                "segmentation without plane orientation".into(),
            ))?;
        let row = normalise([orientation[0], orientation[1], orientation[2]])
            .ok_or(Error::Malformed("orientation"))?;
        let col = normalise([orientation[3], orientation[4], orientation[5]])
            .ok_or(Error::Malformed("orientation"))?;
        let measures = group(ds, 0, PIXEL_MEASURES);
        let spacing2 = measures
            .and_then(|g| nums::<2>(g, &v, PIXEL_SPACING))
            .or_else(|| nums::<2>(ds, &v, PIXEL_SPACING))
            .ok_or(Error::Unsupported(
                "segmentation without pixel spacing".into(),
            ))?;
        // Pixel Spacing is row spacing (between rows) then column spacing.
        let pixel = [spacing2[1], spacing2[0]];
        let spacing = measures
            .and_then(|g| g.get(SPACING_BETWEEN).and_then(|e| v.number(e, 0)))
            .or_else(|| measures.and_then(|g| g.get(SLICE_THICKNESS).and_then(|e| v.number(e, 0))))
            .filter(|s| *s > 0.0);
        // Number of Frames may say a billion, all placed by the shared groups: a frame takes at
        // least a bit of the data, and only the per-frame items are worth reserving for.
        let count = (info.frames as usize).min(d.buf(file).len().saturating_mul(8));
        let items = ds.get(PER_FRAME).map_or(1, |e| e.items.len().max(1));
        let mut frames = Vec::with_capacity(count.min(items));
        for f in 0..count {
            let position = group(ds, f, PLANE_POSITION)
                .and_then(|g| nums::<3>(g, &v, POSITION))
                .or_else(|| {
                    (info.frames == 1)
                        .then(|| nums::<3>(ds, &v, POSITION))
                        .flatten()
                })
                .ok_or(Error::Unsupported(
                    "segmentation frame without plane position".into(),
                ))?;
            let segment = match kind {
                SegType::Labelmap => 0,
                _ => group(ds, f, SEGMENT_IDENTIFICATION)
                    .and_then(|g| g.get(REFERENCED_SEGMENT_NUMBER))
                    .and_then(|e| v.int(e))
                    .ok_or(Error::Malformed("segmentation frame without segment"))?,
            };
            frames.push(SegFrame { segment, position });
        }
        let referenced_series = ds
            .get(REFERENCED_SERIES)
            .map(|e| {
                e.items
                    .iter()
                    .map(|it| text(it, &v, &cs, SERIES_UID))
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let title = Some(text(ds, &v, &cs, SERIES_DESCRIPTION))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| text(ds, &v, &cs, CONTENT_LABEL));
        Ok(SegHeader {
            kind,
            segments,
            frames,
            row,
            col,
            pixel,
            spacing,
            size: [info.columns, info.rows],
            frame_of_reference: text(ds, &v, &cs, FRAME_OF_REFERENCE),
            referenced_series,
            title,
            padding: ds.get(PIXEL_PADDING).and_then(|e| v.int(e)),
        })
    }

    fn normal(&self) -> [f64; 3] {
        normalise(cross(self.row, self.col)).unwrap_or([0.0, 0.0, 1.0])
    }
}

/// Reads a segmentation's segments into label layers, with each segment's volume.
pub fn read(d: &Dicom, file: &[u8], cancel: Option<&AtomicBool>) -> Result<Labels> {
    let h = SegHeader::read(d, file)?;
    let normal = h.normal();
    let first = h.frames.first().ok_or(Error::NoPixelData)?.position;
    let tol = h.spacing.map_or(0.01, |s| (s * 0.01).max(1e-3));
    let planes = distinct(
        h.frames.iter().map(|f| along(f.position, normal)).collect(),
        tol,
    );
    let a0 = along(first, normal);
    let rel: Vec<f64> = planes.iter().map(|a| a - a0).collect();
    // The grid's origin: the first frame's position moved to the lowest plane.
    let origin = [0, 1, 2].map(|c| first[c] + normal[c] * (planes[0] - a0));
    let (grid, index) = plane_grid(
        origin,
        h.row,
        h.col,
        h.pixel,
        h.size,
        &rel.iter().map(|a| a - rel[0]).collect::<Vec<_>>(),
        h.spacing,
    );
    let plane_of = |p: [f64; 3]| -> usize {
        let a = along(p, normal);
        let i = planes
            .iter()
            .enumerate()
            .min_by(|x, y| (x.1 - a).abs().total_cmp(&(y.1 - a).abs()))
            .map_or(0, |(i, _)| i);
        index[i]
    };
    let number_to_index = |n: i64| h.segments.iter().position(|s| s.number == n);
    let mut b = Builder::new(grid, h.segments.len());
    let (w, hh) = (h.size[0] as usize, h.size[1] as usize);
    for (f, fr) in h.frames.iter().enumerate() {
        let frame = d.decode_stored(file, f as u32, cancel)?;
        let Pixels::Grey(g) = &frame.pixels else {
            return Err(Error::Unsupported("colour segmentation frames".into()));
        };
        if frame.width as usize != w || frame.height as usize != hh {
            return Err(Error::Malformed("segmentation frame size"));
        }
        let plane = plane_of(fr.position);
        // Frames may sit shifted in-plane (whole pixels) from the first.
        let d = [0, 1, 2].map(|c| fr.position[c] - first[c]);
        let di = (dot(d, h.row) / h.pixel[0]).round() as i64;
        let dj = (dot(d, h.col) / h.pixel[1]).round() as i64;
        let put = |mask: &mut Vec<u8>, x: usize, y: usize, value: u8| {
            let (gx, gy) = (x as i64 + di, y as i64 + dj);
            if gx >= 0 && gy >= 0 && (gx as usize) < w && (gy as usize) < hh {
                let o = gy as usize * w + gx as usize;
                mask[o] = mask[o].max(value);
            }
        };
        match h.kind {
            SegType::Labelmap => {
                for y in 0..hh {
                    for x in 0..w {
                        let s = g.get(y * w + x).unwrap_or(0);
                        if s > 0
                            && Some(s) != h.padding
                            && let Some(si) = number_to_index(s)
                        {
                            put(b.mask(si, plane), x, y, 255);
                        }
                    }
                }
            }
            SegType::Binary => {
                let Some(si) = number_to_index(fr.segment) else {
                    continue;
                };
                if !any_set(g) {
                    continue;
                }
                let m = b.mask(si, plane);
                for y in 0..hh {
                    for x in 0..w {
                        if g.get(y * w + x).unwrap_or(0) != 0 {
                            put(m, x, y, 255);
                        }
                    }
                }
            }
            SegType::Fractional { max } => {
                let Some(si) = number_to_index(fr.segment) else {
                    continue;
                };
                if !any_set(g) {
                    continue;
                }
                let m = b.mask(si, plane);
                for y in 0..hh {
                    for x in 0..w {
                        let v = g.get_f64(y * w + x).unwrap_or(0.0);
                        if v > 0.0 {
                            let q = ((v / max).clamp(0.0, 1.0) * 255.0).round().max(1.0);
                            put(m, x, y, q as u8);
                        }
                    }
                }
            }
        }
    }
    let ml = voxel_ml(&b.grid);
    let mut segments = h.segments.clone();
    for (s, (n, sum)) in segments.iter_mut().zip(b.counts()) {
        s.voxels = n;
        s.ml = if s.fractional {
            sum * ml
        } else {
            n as f64 * ml
        };
    }
    let fractional: Vec<bool> = segments.iter().map(|s| s.fractional).collect();
    let layers = b.finish(&fractional);
    Ok(Labels {
        source: LabelSource::Seg,
        segments,
        layers,
        frame_of_reference: h.frame_of_reference,
        referenced_series: h.referenced_series,
        title: h.title,
    })
}

fn any_set(g: &Grey) -> bool {
    match g {
        Grey::U16(v) => v.iter().any(|&x| x != 0),
        Grey::I16(v) => v.iter().any(|&x| x != 0),
        Grey::I32(v) => v.iter().any(|&x| x != 0),
        Grey::F32(v) => v.iter().any(|&x| x != 0.0),
        Grey::F64(v) => v.iter().any(|&x| x != 0.0),
    }
}
