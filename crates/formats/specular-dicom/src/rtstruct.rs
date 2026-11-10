//! RT Structure Sets (PS3.3 A.19): each ROI's closed planar contours, read into [`Labels`].
//!
//! Contours are filled even-odd (an inner contour on the same plane is a hole) onto a grid of
//! their own: planes at the contour planes (gaps between them filled in empty), in-plane over
//! the contours' extent at no coarser than 1 mm (at most 512 voxels across). An ROI's volume is
//! the sum over its planes of the contour areas (holes, by containment parity, subtracted)
//! times the median distance between its planes, the usual reading of a structure set.

use std::collections::HashMap;

use crate::charset::Charset;
use crate::labels::{
    Builder, DEFAULT_COLOURS, LabelSource, Labels, Segment, along, distinct, numbers, plane_grid,
};
use crate::parse::DataSet;
use crate::volume::{cross, dot, normalise};
use crate::{Dicom, Error, Result, Values};

/// RT Structure Set Storage.
pub const RTSTRUCT_SOP_CLASS: &str = "1.2.840.10008.5.1.4.1.1.481.3";

const SOP_CLASS: u32 = 0x0008_0016;
const REFERENCED_FOR_SEQUENCE: u32 = 0x3006_0010;
const RT_REFERENCED_STUDY: u32 = 0x3006_0012;
const RT_REFERENCED_SERIES: u32 = 0x3006_0014;
const SERIES_UID: u32 = 0x0020_000E;
const FRAME_OF_REFERENCE: u32 = 0x0020_0052;
const STRUCTURE_SET_ROI: u32 = 0x3006_0020;
const ROI_NUMBER: u32 = 0x3006_0022;
const ROI_FOR: u32 = 0x3006_0024;
const ROI_NAME: u32 = 0x3006_0026;
const ROI_ALGORITHM: u32 = 0x3006_0036;
const ROI_CONTOUR: u32 = 0x3006_0039;
const ROI_COLOUR: u32 = 0x3006_002A;
const CONTOUR_SEQUENCE: u32 = 0x3006_0040;
const GEOMETRIC_TYPE: u32 = 0x3006_0042;
const CONTOUR_DATA: u32 = 0x3006_0050;
const REFERENCED_ROI: u32 = 0x3006_0084;
const STRUCTURE_SET_LABEL: u32 = 0x3006_0002;
const SERIES_DESCRIPTION: u32 = 0x0008_103E;

/// Largest in-plane grid side.
const MAX_SIDE: f64 = 512.0;

/// Whether `sop_class` is an RT Structure Set.
pub fn is_rtstruct(sop_class: &str) -> bool {
    sop_class == RTSTRUCT_SOP_CLASS
}

/// One closed planar contour: points in patient mm.
#[derive(Clone, Debug, PartialEq)]
pub struct Contour {
    pub points: Vec<[f64; 3]>,
}

/// One ROI with its contours.
#[derive(Clone, Debug, PartialEq)]
pub struct Roi {
    pub segment: Segment,
    pub contours: Vec<Contour>,
    /// Contours skipped: POINT, OPEN_PLANAR, OPEN_NONPLANAR or fewer than three points.
    pub skipped: usize,
}

/// A structure set's ROIs and references.
#[derive(Clone, Debug, PartialEq)]
pub struct StructureSet {
    pub rois: Vec<Roi>,
    pub frame_of_reference: String,
    pub referenced_series: Vec<String>,
    pub title: String,
}

fn text(ds: &DataSet, v: &Values, cs: &Charset, tag: u32) -> String {
    ds.get(tag)
        .map(|e| cs.decode(v.text_bytes(e), e.vr).trim().to_owned())
        .unwrap_or_default()
}

impl StructureSet {
    pub fn read(d: &Dicom, file: &[u8]) -> Result<StructureSet> {
        let ds = &d.dataset;
        let v = d.values(file);
        let cs = Charset::of(ds, &v);
        if !is_rtstruct(&text(ds, &v, &cs, SOP_CLASS)) {
            return Err(Error::Unsupported("not an RT structure set".into()));
        }
        let mut frame_of_reference = String::new();
        let mut referenced_series = Vec::new();
        for f in ds
            .get(REFERENCED_FOR_SEQUENCE)
            .map(|e| e.items.as_slice())
            .unwrap_or_default()
        {
            if frame_of_reference.is_empty() {
                frame_of_reference = text(f, &v, &cs, FRAME_OF_REFERENCE);
            }
            for st in f
                .get(RT_REFERENCED_STUDY)
                .map(|e| e.items.as_slice())
                .unwrap_or_default()
            {
                for se in st
                    .get(RT_REFERENCED_SERIES)
                    .map(|e| e.items.as_slice())
                    .unwrap_or_default()
                {
                    let uid = text(se, &v, &cs, SERIES_UID);
                    if !uid.is_empty() && !referenced_series.contains(&uid) {
                        referenced_series.push(uid);
                    }
                }
            }
        }
        let mut rois: Vec<Roi> = Vec::new();
        let mut by_number: HashMap<i64, usize> = HashMap::new();
        for (i, it) in ds
            .get(STRUCTURE_SET_ROI)
            .map(|e| e.items.as_slice())
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            let number = it
                .get(ROI_NUMBER)
                .and_then(|e| v.int(e))
                .unwrap_or(i as i64 + 1);
            if frame_of_reference.is_empty() {
                frame_of_reference = text(it, &v, &cs, ROI_FOR);
            }
            by_number.insert(number, rois.len());
            rois.push(Roi {
                segment: Segment {
                    number,
                    label: text(it, &v, &cs, ROI_NAME),
                    colour: DEFAULT_COLOURS[i % DEFAULT_COLOURS.len()],
                    algorithm: text(it, &v, &cs, ROI_ALGORITHM),
                    fractional: false,
                    ml: 0.0,
                    voxels: 0,
                },
                contours: Vec::new(),
                skipped: 0,
            });
        }
        for rc in ds
            .get(ROI_CONTOUR)
            .map(|e| e.items.as_slice())
            .unwrap_or_default()
        {
            let Some(number) = rc.get(REFERENCED_ROI).and_then(|e| v.int(e)) else {
                continue;
            };
            let at = match by_number.get(&number) {
                Some(&at) => at,
                None => {
                    // A contour for an ROI the structure set does not list: keep it, unnamed.
                    by_number.insert(number, rois.len());
                    let i = rois.len();
                    rois.push(Roi {
                        segment: Segment {
                            number,
                            label: String::new(),
                            colour: DEFAULT_COLOURS[i % DEFAULT_COLOURS.len()],
                            algorithm: String::new(),
                            fractional: false,
                            ml: 0.0,
                            voxels: 0,
                        },
                        contours: Vec::new(),
                        skipped: 0,
                    });
                    i
                }
            };
            let roi = &mut rois[at];
            if let Some(e) = rc.get(ROI_COLOUR) {
                let c: Vec<i64> = (0..3)
                    .filter_map(|k| v.number(e, k).map(|x| x as i64))
                    .collect();
                if c.len() == 3 {
                    roi.segment.colour = [0, 1, 2].map(|k| c[k].clamp(0, 255) as u8);
                }
            }
            for c in rc
                .get(CONTOUR_SEQUENCE)
                .map(|e| e.items.as_slice())
                .unwrap_or_default()
            {
                let kind = text(c, &v, &cs, GEOMETRIC_TYPE);
                let data = c
                    .get(CONTOUR_DATA)
                    .map(|e| numbers(&v, e))
                    .unwrap_or_default();
                let points: Vec<[f64; 3]> = data
                    .chunks_exact(3)
                    .map(|p| [p[0], p[1], p[2]])
                    .filter(|p| p.iter().all(|x| x.is_finite()))
                    .collect();
                if kind != "CLOSED_PLANAR" || points.len() < 3 {
                    roi.skipped += 1;
                    continue;
                }
                roi.contours.push(Contour { points });
            }
        }
        let title = Some(text(ds, &v, &cs, SERIES_DESCRIPTION))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| text(ds, &v, &cs, STRUCTURE_SET_LABEL));
        Ok(StructureSet {
            rois,
            frame_of_reference,
            referenced_series,
            title,
        })
    }

    /// The contours' common plane normal: the Newell normal of the largest contour.
    pub fn normal(&self) -> [f64; 3] {
        self.rois
            .iter()
            .flat_map(|r| &r.contours)
            .map(|c| newell(&c.points))
            .max_by(|a, b| norm(*a).total_cmp(&norm(*b)))
            .and_then(normalise)
            .map(|n| {
                // Point it the conventional way (+z, else +y, else +x), so planes increase.
                let s = if n[2].abs() > 1e-6 {
                    n[2]
                } else if n[1].abs() > 1e-6 {
                    n[1]
                } else {
                    n[0]
                };
                if s < 0.0 { n.map(|x| -x) } else { n }
            })
            .unwrap_or([0.0, 0.0, 1.0])
    }
}

/// Twice the vector area of a polygon (Newell): its normal × 2 × area.
fn newell(p: &[[f64; 3]]) -> [f64; 3] {
    let mut n = [0.0; 3];
    for (i, a) in p.iter().enumerate() {
        let b = p[(i + 1) % p.len()];
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    n
}

fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// Whether 2-D point `q` is inside polygon `p` (even-odd).
fn inside(p: &[[f64; 2]], q: [f64; 2]) -> bool {
    let mut c = false;
    for (i, a) in p.iter().enumerate() {
        let b = p[(i + 1) % p.len()];
        if (a[1] > q[1]) != (b[1] > q[1]) {
            let x = a[0] + (q[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]);
            if q[0] < x {
                c = !c;
            }
        }
    }
    c
}

/// In-plane axes for planes with `normal`.
fn axes(normal: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let x = [1.0, 0.0, 0.0];
    let r = normalise([0, 1, 2].map(|c| x[c] - dot(x, normal) * normal[c]))
        .or_else(|| {
            let y = [0.0, 1.0, 0.0];
            normalise([0, 1, 2].map(|c| y[c] - dot(y, normal) * normal[c]))
        })
        .unwrap_or(x);
    (r, cross(normal, r))
}

/// Contours of one ROI on one plane, in-plane (u, v) mm.
struct PlaneContours {
    along: f64,
    polys: Vec<Vec<[f64; 2]>>,
}

/// Groups an ROI's contours by plane (within `tol` mm), projected onto (`row`, `col`).
fn by_plane(roi: &Roi, normal: [f64; 3], row: [f64; 3], col: [f64; 3]) -> Vec<PlaneContours> {
    let mut out: Vec<PlaneContours> = Vec::new();
    for c in &roi.contours {
        let a = c.points.iter().map(|p| along(*p, normal)).sum::<f64>() / c.points.len() as f64;
        let poly: Vec<[f64; 2]> = c
            .points
            .iter()
            .map(|p| [dot(*p, row), dot(*p, col)])
            .collect();
        match out.iter_mut().find(|pc| (pc.along - a).abs() < PLANE_TOL) {
            Some(pc) => pc.polys.push(poly),
            None => out.push(PlaneContours {
                along: a,
                polys: vec![poly],
            }),
        }
    }
    out.sort_by(|a, b| a.along.total_cmp(&b.along));
    out
}

/// Contours this close along the normal share a plane, mm.
const PLANE_TOL: f64 = 0.05;

fn area2(p: &[[f64; 2]]) -> f64 {
    let mut s = 0.0;
    for (i, a) in p.iter().enumerate() {
        let b = p[(i + 1) % p.len()];
        s += a[0] * b[1] - b[0] * a[1];
    }
    s.abs() / 2.0
}

/// Area of a plane's contours, mm²: an inner contour (inside an odd number of others) is a
/// hole.
fn plane_area(polys: &[Vec<[f64; 2]>]) -> f64 {
    let mut total = 0.0;
    for (i, p) in polys.iter().enumerate() {
        let depth = polys
            .iter()
            .enumerate()
            .filter(|(j, o)| *j != i && inside(o, p[0]))
            .count();
        let a = area2(p);
        total += if depth % 2 == 1 { -a } else { a };
    }
    total.max(0.0)
}

fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(v[v.len() / 2])
}

/// An ROI's volume, mL: plane areas × the median spacing of its planes (the structure set's
/// spacing, `fallback`, when it has one plane).
pub fn roi_ml(roi: &Roi, normal: [f64; 3], fallback: f64) -> f64 {
    let (row, col) = axes(normal);
    let planes = by_plane(roi, normal, row, col);
    let gaps: Vec<f64> = planes
        .windows(2)
        .map(|w| w[1].along - w[0].along)
        .filter(|g| *g > PLANE_TOL)
        .collect();
    let step = median(gaps).unwrap_or(fallback);
    planes.iter().map(|p| plane_area(&p.polys)).sum::<f64>() * step / 1000.0
}

/// Reads a structure set's closed planar contours into a label volume, with each ROI's volume.
pub fn read(d: &Dicom, file: &[u8]) -> Result<Labels> {
    let set = StructureSet::read(d, file)?;
    labels(&set)
}

/// Fills `set`'s contours onto a grid of their own.
pub fn labels(set: &StructureSet) -> Result<Labels> {
    let normal = set.normal();
    let (row, col) = axes(normal);
    let mut alongs = Vec::new();
    let (mut u0, mut v0, mut u1, mut v1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for c in set.rois.iter().flat_map(|r| &r.contours) {
        let a = c.points.iter().map(|p| along(*p, normal)).sum::<f64>() / c.points.len() as f64;
        alongs.push(a);
        for p in &c.points {
            let (u, v) = (dot(*p, row), dot(*p, col));
            u0 = u0.min(u);
            u1 = u1.max(u);
            v0 = v0.min(v);
            v1 = v1.max(v);
        }
    }
    let planes = distinct(alongs, PLANE_TOL);
    if planes.is_empty() {
        return Err(Error::Unsupported(
            "structure set without closed planar contours".into(),
        ));
    }
    let px = ((u1 - u0).max(v1 - v0) / MAX_SIDE).max(1.0);
    // One voxel of margin around the contours; voxel 0's centre half a voxel inside.
    let w = ((u1 - u0) / px).ceil() as u32 + 3;
    let h = ((v1 - v0) / px).ceil() as u32 + 3;
    let (ou, ov) = (u0 - px, v0 - px);
    let origin = [0, 1, 2].map(|c| row[c] * ou + col[c] * ov + normal[c] * planes[0]);
    let rel: Vec<f64> = planes.iter().map(|a| a - planes[0]).collect();
    let (grid, index) = plane_grid(origin, row, col, [px, px], [w, h], &rel, None);
    let gaps: Vec<f64> = planes
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|g| *g > PLANE_TOL)
        .collect();
    let fallback = median(gaps).unwrap_or(grid.spacing().max(1.0));
    let plane_of = |a: f64| -> usize {
        let i = planes
            .iter()
            .enumerate()
            .min_by(|x, y| (x.1 - a).abs().total_cmp(&(y.1 - a).abs()))
            .map_or(0, |(i, _)| i);
        index[i]
    };
    let mut b = Builder::new(grid, set.rois.len());
    let (wu, hu) = (w as usize, h as usize);
    let mut segments = Vec::with_capacity(set.rois.len());
    for (s, roi) in set.rois.iter().enumerate() {
        let mut seg = roi.segment.clone();
        seg.ml = roi_ml(roi, normal, fallback);
        for pc in by_plane(roi, normal, row, col) {
            let m = b.mask(s, plane_of(pc.along));
            fill(m, wu, hu, &pc.polys, [ou, ov], px);
        }
        segments.push(seg);
    }
    for (seg, (n, _)) in segments.iter_mut().zip(b.counts()) {
        seg.voxels = n;
    }
    let layers = b.finish(&vec![false; segments.len()]);
    Ok(Labels {
        source: LabelSource::RtStruct,
        segments,
        layers,
        frame_of_reference: set.frame_of_reference.clone(),
        referenced_series: set.referenced_series.clone(),
        title: set.title.clone(),
    })
}

/// Even-odd scanline fill of `polys` into `mask` (`w`×`h`, voxel (i, j) centred at `origin` +
/// (i, j)·`px`).
fn fill(mask: &mut [u8], w: usize, h: usize, polys: &[Vec<[f64; 2]>], origin: [f64; 2], px: f64) {
    let mut xs: Vec<f64> = Vec::new();
    for j in 0..h {
        let y = origin[1] + j as f64 * px;
        xs.clear();
        for p in polys {
            for (i, a) in p.iter().enumerate() {
                let b = p[(i + 1) % p.len()];
                if (a[1] > y) != (b[1] > y) {
                    xs.push(a[0] + (y - a[1]) * (b[0] - a[0]) / (b[1] - a[1]));
                }
            }
        }
        xs.sort_by(f64::total_cmp);
        for pair in xs.chunks_exact(2) {
            // Voxel centres x with pair[0] <= x < pair[1].
            let i0 = ((pair[0] - origin[0]) / px).ceil().max(0.0) as usize;
            let i1 = ((pair[1] - origin[0]) / px).ceil().clamp(0.0, w as f64) as usize;
            for i in i0..i1.min(w) {
                mask[j * w + i] = 255;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(z: f64, x0: f64, y0: f64, s: f64) -> Contour {
        Contour {
            points: vec![
                [x0, y0, z],
                [x0 + s, y0, z],
                [x0 + s, y0 + s, z],
                [x0, y0 + s, z],
            ],
        }
    }

    fn roi(contours: Vec<Contour>) -> Roi {
        Roi {
            segment: Segment {
                number: 1,
                label: "box".into(),
                colour: [255, 0, 0],
                algorithm: "MANUAL".into(),
                fractional: false,
                ml: 0.0,
                voxels: 0,
            },
            contours,
            skipped: 0,
        }
    }

    #[test]
    fn cube_volume_and_voxels() {
        // A 20 mm square on 11 planes 2 mm apart: 400 mm² × 11 × 2 mm = 8.8 mL.
        let r = roi((0..11)
            .map(|k| square(k as f64 * 2.0, 0.0, 0.0, 20.0))
            .collect());
        assert!((roi_ml(&r, [0.0, 0.0, 1.0], 1.0) - 8.8).abs() < 1e-9);
        let set = StructureSet {
            rois: vec![r],
            frame_of_reference: "1.2".into(),
            referenced_series: vec![],
            title: String::new(),
        };
        let l = labels(&set).unwrap();
        assert!((l.segments[0].ml - 8.8).abs() < 1e-9);
        // 1 mm voxels: 20 × 20 centres inside per plane.
        assert_eq!(l.segments[0].voxels, 20 * 20 * 11);
        assert_eq!(l.at([10.0, 10.0, 10.0]), vec![0]);
        assert!(l.at([30.0, 10.0, 10.0]).is_empty());
        assert!(l.at([10.0, 10.0, 30.0]).is_empty());
    }

    #[test]
    fn holes_subtract() {
        let r = roi(vec![
            square(0.0, 0.0, 0.0, 20.0),
            square(0.0, 5.0, 5.0, 10.0),
        ]);
        // (400 − 100) mm² × 3 mm.
        assert!((roi_ml(&r, [0.0, 0.0, 1.0], 3.0) - 0.9).abs() < 1e-9);
        let set = StructureSet {
            rois: vec![r],
            frame_of_reference: String::new(),
            referenced_series: vec![],
            title: String::new(),
        };
        let l = labels(&set).unwrap();
        assert_eq!(l.segments[0].voxels, 300);
        assert!(l.at([10.0, 10.0, 0.0]).is_empty());
        assert_eq!(l.at([2.0, 2.0, 0.0]), vec![0]);
    }
}
