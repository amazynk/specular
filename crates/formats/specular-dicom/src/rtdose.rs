//! RT Dose (PS3.3 C.8.8.3): the dose grid, isodose levels and dose volume histograms.
//!
//! A dose grid is a multi-frame image whose stored values times Dose Grid Scaling are the dose
//! in Dose Units (Gy). Its frames sit at the Grid Frame Offset Vector: offsets along the slice
//! normal from Image Position (Patient) when the first offset is 0, else the frames' z.
//!
//! The cumulative DVH of a structure follows the usual planar method (as dicompyler-core
//! computes it, so the two agree bin for bin): for each contour plane of the ROI (planes at
//! the contours' z rounded to 0.01 mm), the dose plane at that z (a frame within 0.5 mm, else
//! interpolated linearly between the two nearest frames; planes outside the grid add volume
//! but no dose), the dose grid points inside the plane's contours (even-odd per contour,
//! contours combined by exclusive or), binned at 1 cGy from 0 to the grid maximum. Each point
//! stands for its pixel area times the ROI's plane thickness (the smallest distance between
//! its planes); the histogram is then scaled to the ROI's whole volume, including planes
//! outside the grid.

use crate::charset::Charset;
use crate::parse::DataSet;
use crate::pixel::Pixels;
use crate::rtstruct::{Roi, StructureSet};
use crate::volume::{Geometry, Refusal, Volume, VoxelKind, cross, dot, normalise};
use crate::{Dicom, Error, Result, Values};

/// RT Dose Storage.
pub const RTDOSE_SOP_CLASS: &str = "1.2.840.10008.5.1.4.1.1.481.2";
/// RT Plan Storage and Ion Plan Storage.
pub const RTPLAN_SOP_CLASSES: [&str; 2] = [
    "1.2.840.10008.5.1.4.1.1.481.5",
    "1.2.840.10008.5.1.4.1.1.481.8",
];

const SOP_CLASS: u32 = 0x0008_0016;
/// The most DVH bins (of 0.01 Gy each).
pub const MAX_DVH_BINS: usize = 1 << 20;
const SOP_INSTANCE: u32 = 0x0008_0018;
const REFERENCED_SOP_INSTANCE: u32 = 0x0008_1155;
const POSITION: u32 = 0x0020_0032;
const ORIENTATION: u32 = 0x0020_0037;
const FRAME_OF_REFERENCE: u32 = 0x0020_0052;
const PIXEL_SPACING: u32 = 0x0028_0030;
const DOSE_UNITS: u32 = 0x3004_0002;
const DOSE_TYPE: u32 = 0x3004_0004;
const SUMMATION_TYPE: u32 = 0x3004_000A;
const GRID_FRAME_OFFSETS: u32 = 0x3004_000C;
const DOSE_GRID_SCALING: u32 = 0x3004_000E;
const DOSE_REFERENCE_SEQUENCE: u32 = 0x300A_0010;
const DOSE_REFERENCE_TYPE: u32 = 0x300A_0020;
const DELIVERY_MAXIMUM_DOSE: u32 = 0x300A_0023;
const TARGET_PRESCRIPTION_DOSE: u32 = 0x300A_0026;
const PLAN_LABEL: u32 = 0x300A_0002;
const REFERENCED_PLAN_SEQUENCE: u32 = 0x300C_0002;

/// Most grid points read (frames × rows × columns).
const MAX_POINTS: usize = 1 << 28;
/// A structure plane uses a dose frame without interpolation within this distance (mm).
const PLANE_MATCH: f64 = 0.5;

/// Whether `sop_class` is RT Dose.
pub fn is_rtdose(sop_class: &str) -> bool {
    sop_class == RTDOSE_SOP_CLASS
}

/// Whether `sop_class` is an RT (Ion) Plan.
pub fn is_rtplan(sop_class: &str) -> bool {
    RTPLAN_SOP_CLASSES.contains(&sop_class)
}

/// A dose grid.
#[derive(Clone, Debug, PartialEq)]
pub struct DoseGrid {
    pub columns: usize,
    pub rows: usize,
    /// Image Position (Patient) of the first frame.
    pub position: [f64; 3],
    /// Image Orientation (Patient): row then column direction cosines.
    pub orientation: [f64; 6],
    /// Pixel Spacing: between rows, between columns (mm).
    pub spacing: [f64; 2],
    /// Grid Frame Offset Vector as written.
    pub offsets: Vec<f64>,
    pub scaling: f64,
    /// Dose Units (GY or RELATIVE).
    pub units: String,
    /// Dose Type (PHYSICAL, EFFECTIVE, ERROR).
    pub dose_type: String,
    /// Dose Summation Type (PLAN, BEAM, FRACTION…).
    pub summation: String,
    pub frame_of_reference: String,
    pub sop_instance: String,
    /// The RT Plan this dose was computed for.
    pub plan: Option<String>,
    /// Stored values, frame by frame, row by row.
    pub values: Vec<f64>,
    /// Largest stored value.
    pub max_stored: f64,
}

fn text(ds: &DataSet, v: &Values, cs: &Charset, tag: u32) -> String {
    ds.get(tag)
        .map(|e| cs.decode(v.text_bytes(e), e.vr).trim().to_owned())
        .unwrap_or_default()
}

fn nums(ds: &DataSet, v: &Values, tag: u32) -> Vec<f64> {
    ds.get(tag)
        .map(|e| (0..v.count(e)).filter_map(|i| v.number(e, i)).collect())
        .unwrap_or_default()
}

impl DoseGrid {
    /// Reads the dose grid of RT Dose `d` (parsed from `file`), decoding every frame.
    pub fn read(d: &Dicom, file: &[u8]) -> Result<DoseGrid> {
        let ds = &d.dataset;
        let v = d.values(file);
        let cs = Charset::of(ds, &v);
        if !is_rtdose(&text(ds, &v, &cs, SOP_CLASS)) {
            return Err(Error::Unsupported("not an RT dose".into()));
        }
        let pixel = d.pixel.as_ref().ok_or(Error::NoPixelData)?;
        let (columns, rows) = (pixel.columns as usize, pixel.rows as usize);
        let frames = d.frame_count() as usize;
        let total = columns
            .checked_mul(rows)
            .and_then(|n| n.checked_mul(frames))
            .filter(|&n| n <= MAX_POINTS)
            .ok_or(Error::TooLarge((columns * rows * frames * 8) as u64))?;
        let p = nums(ds, &v, POSITION);
        let o = nums(ds, &v, ORIENTATION);
        let s = nums(ds, &v, PIXEL_SPACING);
        if p.len() < 3 || o.len() < 6 || s.len() < 2 {
            return Err(Error::Malformed("RT dose without its plane geometry"));
        }
        let mut offsets = nums(ds, &v, GRID_FRAME_OFFSETS);
        if frames > 1 && offsets.len() != frames {
            return Err(Error::Malformed(
                "Grid Frame Offset Vector does not match the frames",
            ));
        }
        if offsets.is_empty() {
            offsets.push(0.0);
        }
        let scaling = ds
            .get(DOSE_GRID_SCALING)
            .and_then(|e| v.number(e, 0))
            .unwrap_or(1.0);
        // Number of Frames × Rows × Columns, all in memory at once.
        let _res = specular_core::budget::reserve_n(total as u64, 8)?;
        let mut values = Vec::with_capacity(total);
        for f in 0..frames {
            let frame = d.decode_frame(file, f as u32, None)?;
            let Pixels::Grey(g) = &frame.pixels else {
                return Err(Error::Malformed("RT dose in colour"));
            };
            values.extend((0..columns * rows).map(|i| g.get_f64(i).unwrap_or(0.0)));
        }
        let max_stored = values.iter().copied().fold(0.0, f64::max);
        Ok(DoseGrid {
            columns,
            rows,
            position: [p[0], p[1], p[2]],
            orientation: [o[0], o[1], o[2], o[3], o[4], o[5]],
            spacing: [s[0], s[1]],
            offsets,
            scaling,
            units: text(ds, &v, &cs, DOSE_UNITS),
            dose_type: text(ds, &v, &cs, DOSE_TYPE),
            summation: text(ds, &v, &cs, SUMMATION_TYPE),
            frame_of_reference: text(ds, &v, &cs, FRAME_OF_REFERENCE),
            sop_instance: text(ds, &v, &cs, SOP_INSTANCE),
            plan: ds
                .item(REFERENCED_PLAN_SEQUENCE)
                .map(|it| text(it, &v, &cs, REFERENCED_SOP_INSTANCE))
                .filter(|s| !s.is_empty()),
            values,
            max_stored,
        })
    }

    pub fn frames(&self) -> usize {
        self.offsets.len()
    }

    /// The largest dose (in Dose Units).
    pub fn max_dose(&self) -> f64 {
        self.max_stored * self.scaling
    }

    /// The slice normal (row × column direction).
    pub fn normal(&self) -> [f64; 3] {
        let o = self.orientation;
        normalise(cross([o[0], o[1], o[2]], [o[3], o[4], o[5]])).unwrap_or([0.0, 0.0, 1.0])
    }

    /// Patient position of frame `f`'s first pixel.
    pub fn frame_position(&self, f: usize) -> [f64; 3] {
        let p = self.position;
        let d = self.offsets.get(f).copied().unwrap_or(0.0);
        if self.offsets[0] == 0.0 {
            let n = self.normal();
            [p[0] + d * n[0], p[1] + d * n[1], p[2] + d * n[2]]
        } else {
            [p[0], p[1], d]
        }
    }

    /// The z of each frame.
    pub fn plane_z(&self) -> Vec<f64> {
        (0..self.frames())
            .map(|f| self.frame_position(f)[2])
            .collect()
    }

    /// The x, y of grid point (column `c`, row `r`).
    pub fn point_xy(&self, c: usize, r: usize) -> [f64; 2] {
        let o = self.orientation;
        let (dr, dc) = (self.spacing[0], self.spacing[1]);
        let p = self.position;
        [
            p[0] + c as f64 * dc * o[0] + r as f64 * dr * o[3],
            p[1] + c as f64 * dc * o[1] + r as f64 * dr * o[4],
        ]
    }

    /// Stored values of frame `f`.
    pub fn frame(&self, f: usize) -> &[f64] {
        let n = self.columns * self.rows;
        &self.values[f * n..(f + 1) * n]
    }

    /// The dose plane (stored values) at `z`: a frame within 0.5 mm, else interpolated
    /// between the two nearest frames; None outside the grid.
    pub fn plane_at(&self, z: f64) -> Option<Vec<f64>> {
        let planes = self.plane_z();
        let dist: Vec<f64> = planes.iter().map(|p| (p - z).abs()).collect();
        let nearest = argmin(&dist)?;
        if dist[nearest] < PLANE_MATCH {
            return Some(self.frame(nearest).to_vec());
        }
        let lo = planes.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = planes.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if z < lo || z > hi || planes.len() < 2 {
            return None;
        }
        let mut second = dist.clone();
        second[nearest] = dist.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let lb = argmin(&second)?;
        let fz = (z - planes[lb]) / (planes[nearest] - planes[lb]);
        let (u, l) = (self.frame(nearest), self.frame(lb));
        Some(
            u.iter()
                .zip(l)
                .map(|(&u, &l)| fz * u + (1.0 - fz) * l)
                .collect(),
        )
    }

    /// The grid as a volume of doses in Dose Units (f32), its slices in order along the
    /// normal (frames given in decreasing order are reversed).
    pub fn volume(&self) -> std::result::Result<Volume, Refusal> {
        let n = self.normal();
        let mut order: Vec<usize> = (0..self.frames()).collect();
        let along = |f: usize| dot(self.frame_position(f), n);
        order.sort_by(|&a, &b| along(a).total_cmp(&along(b)));
        let positions: Vec<[f64; 3]> = order.iter().map(|&f| self.frame_position(f)).collect();
        let o = self.orientation;
        let geometry = Geometry::new(
            &positions,
            [o[0], o[1], o[2]],
            [o[3], o[4], o[5]],
            [self.spacing[1], self.spacing[0]],
            [self.columns as u32, self.rows as u32],
        );
        let mut vol = Volume::new(geometry, VoxelKind::F32, 1)?;
        let mut slice = vec![0f32; self.columns * self.rows];
        for (k, &f) in order.iter().enumerate() {
            for (o, &s) in slice.iter_mut().zip(self.frame(f)) {
                *o = (s * self.scaling) as f32;
            }
            vol.put(k, &slice, self.columns as u32, self.rows as u32)?;
        }
        Ok(vol)
    }

    /// Area (mm²) a grid point stands for.
    pub fn pixel_area(&self) -> f64 {
        let a = self.point_xy(0, 0);
        let bx = self.point_xy(self.columns.saturating_sub(1), 0);
        let by = self.point_xy(0, self.rows.saturating_sub(1));
        // dicompyler-core's lookup tables: x across columns, y down rows (or swapped for
        // decubitus grids), spaced evenly from the first to the last grid point.
        let step = |a: f64, b: f64, n: usize| {
            if n > 1 {
                (b - a).abs() / (n - 1) as f64
            } else {
                0.0
            }
        };
        let x_across_columns = self.orientation[0].abs() >= self.orientation[1].abs();
        if x_across_columns {
            step(a[0], bx[0], self.columns) * step(a[1], by[1], self.rows)
        } else {
            step(a[1], bx[1], self.columns) * step(a[0], by[0], self.rows)
        }
    }

    /// The cumulative DVH of `roi`.
    ///
    /// Doses past [`MAX_DVH_BINS`] hundredths of a gray are left out.
    pub fn dvh(&self, roi: &Roi) -> Dvh {
        // 0.01 Gy bins up to the largest dose. The scaling is the file's: an absurd one (or
        // NaN) must not size the histogram, so it stops at MAX_DVH_BINS (over 10 kGy).
        let top = self.max_stored * self.scaling * 100.0;
        let max_bin = if top.is_finite() && top >= 0.0 {
            (top as usize).min(MAX_DVH_BINS - 1) + 1
        } else {
            1
        };
        let mut planes: Vec<(i64, Vec<&Vec<[f64; 3]>>)> = Vec::new();
        for c in &roi.contours {
            let Some(first) = c.points.first() else {
                continue;
            };
            let key = (first[2] * 100.0).round() as i64;
            match planes.iter_mut().find(|(k, _)| *k == key) {
                Some((_, v)) => v.push(&c.points),
                None => planes.push((key, vec![&c.points])),
            }
        }
        planes.sort_by_key(|(k, _)| *k);
        let thickness = planes
            .windows(2)
            .map(|w| (w[1].0 - w[0].0) as f64 / 100.0)
            .fold(None, |m: Option<f64>, d| Some(m.map_or(d, |m| m.min(d))))
            .unwrap_or(0.0);
        let area = self.pixel_area();
        let points: Vec<[f64; 2]> = (0..self.rows)
            .flat_map(|r| (0..self.columns).map(move |c| (c, r)))
            .map(|(c, r)| self.point_xy(c, r))
            .collect();
        let mut hist = vec![0.0f64; max_bin];
        let mut volume = 0.0;
        let mut any = false;
        for (key, contours) in &planes {
            let z = *key as f64 / 100.0;
            let mut mask = vec![false; points.len()];
            for c in contours {
                let poly: Vec<[f64; 2]> = c.iter().map(|p| [p[0], p[1]]).collect();
                for (m, p) in mask.iter_mut().zip(&points) {
                    if inside(&poly, *p) {
                        *m = !*m;
                    }
                }
            }
            let count = mask.iter().filter(|&&m| m).count();
            volume += count as f64 * area * thickness;
            let Some(plane) = self.plane_at(z) else {
                continue;
            };
            any = true;
            for (i, _) in mask.iter().enumerate().filter(|(_, m)| **m) {
                let d = plane[i] * self.scaling * 100.0;
                if (0.0..=max_bin as f64).contains(&d) {
                    hist[(d.floor() as usize).min(max_bin - 1)] += 1.0;
                }
            }
        }
        let volume = volume / 1000.0;
        let total: f64 = hist.iter().sum();
        if !any || total <= 0.0 {
            return Dvh {
                differential: Vec::new(),
                volume,
            };
        }
        for h in &mut hist {
            *h *= volume / total;
        }
        while hist.last() == Some(&0.0) {
            hist.pop();
        }
        Dvh {
            differential: hist,
            volume,
        }
    }
}

fn argmin(v: &[f64]) -> Option<usize> {
    let mut best: Option<usize> = None;
    for (i, x) in v.iter().enumerate() {
        if best.is_none_or(|b| *x < v[b]) {
            best = Some(i);
        }
    }
    best
}

/// Even-odd point in polygon (a point on the boundary falls on either side).
pub fn inside(poly: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut odd = false;
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if p[0] < x {
                odd = !odd;
            }
        }
        j = i;
    }
    odd
}

/// A dose volume histogram: 1 cGy bins from 0.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dvh {
    /// Volume (cc) per bin.
    pub differential: Vec<f64>,
    /// The structure's volume (cc), including any part outside the dose grid.
    pub volume: f64,
}

/// DVH bin width (Gy).
pub const BIN: f64 = 0.01;

impl Dvh {
    pub fn is_empty(&self) -> bool {
        self.differential.iter().all(|&v| v <= 0.0)
    }

    /// Volume (cc) receiving at least each bin's dose.
    pub fn cumulative(&self) -> Vec<f64> {
        let mut out = vec![0.0; self.differential.len()];
        let mut acc = 0.0;
        for i in (0..out.len()).rev() {
            acc += self.differential[i];
            out[i] = acc;
        }
        out
    }

    /// The highest dose bin's upper edge (Gy).
    pub fn max(&self) -> f64 {
        self.differential
            .iter()
            .rposition(|&v| v > 0.0)
            .map_or(0.0, |i| (i + 1) as f64 * BIN)
    }

    /// The lowest dose bin's upper edge (Gy).
    pub fn min(&self) -> f64 {
        self.differential
            .iter()
            .position(|&v| v > 0.0)
            .map_or(0.0, |i| (i + 1) as f64 * BIN)
    }

    /// The mean dose (Gy), at bin centres.
    pub fn mean(&self) -> f64 {
        let total: f64 = self.differential.iter().sum();
        if total <= 0.0 {
            return 0.0;
        }
        self.differential
            .iter()
            .enumerate()
            .map(|(i, v)| (i as f64 + 0.5) * BIN * v)
            .sum::<f64>()
            / total
    }

    /// The dose (Gy) received by at least `percent` % of the volume (D95…): the bin whose
    /// cumulative relative volume is nearest.
    pub fn dose_at(&self, percent: f64) -> f64 {
        let c = self.cumulative();
        let Some(&top) = c.first() else {
            return 0.0;
        };
        if top <= 0.0 || percent > 100.0 {
            return 0.0;
        }
        let rel: Vec<f64> = c.iter().map(|v| 100.0 * v / top).collect();
        let diff: Vec<f64> = rel.iter().map(|r| (r - percent).abs()).collect();
        let i = if percent == 100.0 {
            // The last of the nearest bins.
            let best = diff.iter().copied().fold(f64::INFINITY, f64::min);
            diff.iter().rposition(|&d| d == best).unwrap_or(0)
        } else {
            argmin(&diff).unwrap_or(0)
        };
        i as f64 * BIN
    }

    /// The volume (cc) receiving at least `gy` (V20Gy…), at the nearest bin edge.
    pub fn volume_at(&self, gy: f64) -> f64 {
        let c = self.cumulative();
        let edges = c.len() + 1;
        let i = (0..edges)
            .min_by(|&a, &b| {
                let da = (a as f64 * BIN - gy).abs();
                let db = (b as f64 * BIN - gy).abs();
                da.total_cmp(&db)
            })
            .unwrap_or(0);
        c.get(i).copied().unwrap_or(0.0)
    }
}

/// The DVH of every ROI of `set` that has contours: (ROI index, DVH).
pub fn dvhs(grid: &DoseGrid, set: &StructureSet) -> Vec<(usize, Dvh)> {
    set.rois
        .iter()
        .enumerate()
        .filter(|(_, r)| !r.contours.is_empty())
        .map(|(i, r)| (i, grid.dvh(r)))
        .collect()
}

/// A plan's prescription: the largest Target Prescription Dose (else Delivery Maximum Dose)
/// of its dose references, in Gy.
#[derive(Clone, Debug, PartialEq)]
pub struct Prescription {
    pub sop_instance: String,
    pub label: String,
    pub dose: f64,
}

impl Prescription {
    /// Reads the prescription of RT Plan `d`, if it states one.
    pub fn read(d: &Dicom, file: &[u8]) -> Option<Prescription> {
        let ds = &d.dataset;
        let v = d.values(file);
        let cs = Charset::of(ds, &v);
        if !is_rtplan(&text(ds, &v, &cs, SOP_CLASS)) {
            return None;
        }
        let refs = ds.get(DOSE_REFERENCE_SEQUENCE)?;
        let mut best: Option<f64> = None;
        for it in &refs.items {
            let target = text(it, &v, &cs, DOSE_REFERENCE_TYPE);
            let dose = it
                .get(TARGET_PRESCRIPTION_DOSE)
                .and_then(|e| v.number(e, 0))
                .or_else(|| it.get(DELIVERY_MAXIMUM_DOSE).and_then(|e| v.number(e, 0)));
            if let Some(dose) = dose.filter(|d| *d > 0.0)
                && (target.is_empty() || target == "TARGET")
            {
                best = Some(best.map_or(dose, |b: f64| b.max(dose)));
            }
        }
        Some(Prescription {
            sop_instance: text(ds, &v, &cs, SOP_INSTANCE),
            label: text(ds, &v, &cs, PLAN_LABEL),
            dose: best?,
        })
    }
}

/// The standard isodose levels (% of the reference dose) and their colours.
pub const ISODOSE_LEVELS: [(f64, [f32; 3]); 9] = [
    (107.0, [1.0, 0.0, 1.0]),
    (105.0, [1.0, 0.0, 0.0]),
    (100.0, [1.0, 0.5, 0.0]),
    (95.0, [1.0, 1.0, 0.0]),
    (90.0, [0.0, 1.0, 0.0]),
    (80.0, [0.0, 1.0, 1.0]),
    (70.0, [0.0, 0.6, 1.0]),
    (50.0, [0.0, 0.0, 1.0]),
    (30.0, [0.6, 0.3, 1.0]),
];

/// The isodose lines to draw: (dose in Gy, % of `reference`, colour), highest first.
pub fn isodose_lines(reference: f64) -> Vec<(f64, f64, [f32; 3])> {
    ISODOSE_LEVELS
        .iter()
        .map(|&(p, c)| (reference * p / 100.0, p, c))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::labels::Segment;
    use crate::rtstruct::Contour;

    fn grid(frames: usize) -> DoseGrid {
        let (columns, rows) = (10, 8);
        let mut values = Vec::new();
        for f in 0..frames {
            for r in 0..rows {
                for c in 0..columns {
                    values.push((100 * f + 10 * r + c) as f64);
                }
            }
        }
        let max_stored = values.iter().copied().fold(0.0, f64::max);
        DoseGrid {
            columns,
            rows,
            position: [-10.0, -8.0, 0.0],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            spacing: [2.0, 2.0],
            offsets: (0..frames).map(|f| f as f64 * 3.0).collect(),
            scaling: 0.01,
            units: "GY".into(),
            dose_type: "PHYSICAL".into(),
            summation: "PLAN".into(),
            frame_of_reference: "1.2.3".into(),
            sop_instance: "1.2.3.4".into(),
            plan: None,
            values,
            max_stored,
        }
    }

    /// An RT Dose of 8 frames of 4096 × 4096 (1 GiB of grid, under the cap) with almost no
    /// pixel data: refused under a 64 MiB memory budget before the grid is allocated.
    #[test]
    fn grid_within_memory_budget() {
        use crate::dict::Vr;
        use crate::write::{self, FileMeta, Object};
        let mut o = Object::default();
        o.put_text(SOP_CLASS, Vr::UI, RTDOSE_SOP_CLASS);
        o.put_text(SOP_INSTANCE, Vr::UI, "1.2.3.4");
        o.put_text(POSITION, Vr::DS, "0\\0\\0");
        o.put_text(ORIENTATION, Vr::DS, "1\\0\\0\\0\\1\\0");
        o.put_bytes(0x0028_0002, Vr::US, 1u16.to_le_bytes());
        o.put_text(0x0028_0004, Vr::CS, "MONOCHROME2");
        o.put_text(0x0028_0008, Vr::IS, "8");
        o.put_bytes(0x0028_0010, Vr::US, 4096u16.to_le_bytes());
        o.put_bytes(0x0028_0011, Vr::US, 4096u16.to_le_bytes());
        o.put_text(PIXEL_SPACING, Vr::DS, "1\\1");
        o.put_bytes(0x0028_0100, Vr::US, 16u16.to_le_bytes());
        o.put_bytes(0x0028_0101, Vr::US, 16u16.to_le_bytes());
        o.put_bytes(0x0028_0102, Vr::US, 15u16.to_le_bytes());
        o.put_bytes(0x0028_0103, Vr::US, 0u16.to_le_bytes());
        o.put_text(GRID_FRAME_OFFSETS, Vr::DS, "0\\1\\2\\3\\4\\5\\6\\7");
        o.put_bytes(0x7FE0_0010, Vr::OW, vec![0u8; 16]);
        let file = write::part10(&o, &FileMeta::of(&o, "1.2.840.10008.1.2.1")).unwrap();
        let d = Dicom::parse(&file).unwrap();
        let e =
            specular_core::budget::with_limit(64 << 20, || DoseGrid::read(&d, &file)).unwrap_err();
        let Error::NeedsMemory(n) = e else {
            panic!("{e:?}")
        };
        assert!(n.needed >= 1 << 30 && n.needed > n.free, "{n:?}");
        // One 32 MiB frame does not fit in 16 MiB either.
        let e = specular_core::budget::with_limit(16 << 20, || d.decode_frame(&file, 0, None))
            .unwrap_err();
        assert!(matches!(e, Error::NeedsMemory(_)), "{e:?}");
    }

    #[test]
    fn planes_interpolate_between_frames() {
        let g = grid(3);
        assert_eq!(g.plane_z(), vec![0.0, 3.0, 6.0]);
        assert_eq!(g.plane_at(3.2).unwrap()[0], 100.0);
        let mid = g.plane_at(1.5).unwrap();
        assert!((mid[0] - 50.0).abs() < 1e-9);
        assert!(g.plane_at(6.6).is_none());
        assert!(g.plane_at(-0.6).is_none());
        assert_eq!(g.point_xy(1, 2), [-8.0, -4.0]);
        assert_eq!(g.pixel_area(), 4.0);
    }

    #[test]
    fn grid_as_a_volume_in_gy() {
        let mut g = grid(3);
        // Frames written top down: the volume runs bottom up all the same.
        g.offsets = vec![0.0, -3.0, -6.0];
        let v = g.volume().unwrap();
        assert_eq!(v.geometry.dims, [10, 8, 3]);
        assert_eq!(v.geometry.origin, [-10.0, -8.0, -6.0]);
        assert!(v.complete());
        // Column 4, row 3 of frame 1 (z = −3): stored 134 × 0.01.
        let at = v.sample_patient([-2.0, -2.0, -3.0]).unwrap();
        assert!((at - 1.34).abs() < 1e-6, "{at}");
        // Halfway between frames 1 and 2 (z −3 and −6).
        let mid = v.sample_patient([-2.0, -2.0, -4.5]).unwrap();
        assert!((mid - 1.84).abs() < 1e-5, "{mid}");
    }

    #[test]
    fn point_in_polygon_is_even_odd() {
        let sq = [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]];
        assert!(inside(&sq, [1.0, 1.0]));
        assert!(!inside(&sq, [5.0, 1.0]));
        assert!(!inside(&sq[..2], [1.0, 1.0]));
    }

    #[test]
    fn dvh_with_absurd_scaling() {
        // Dose Grid Scaling of 1e12 (or NaN) asked for a histogram of 10^16 bins.
        for scaling in [1e12, f64::NAN, f64::INFINITY] {
            let mut g = grid(1);
            g.scaling = scaling;
            let roi = Roi {
                segment: Segment {
                    number: 1,
                    label: "PTV".into(),
                    colour: [255, 0, 0],
                    algorithm: String::new(),
                    fractional: false,
                    ml: 0.0,
                    voxels: 0,
                },
                contours: vec![Contour {
                    points: vec![[-9.0, -7.0, 0.0], [9.0, -7.0, 0.0], [9.0, 7.0, 0.0]],
                }],
                skipped: 0,
            };
            assert!(g.dvh(&roi).differential.len() <= MAX_DVH_BINS);
        }
    }

    #[test]
    fn dvh_statistics() {
        let g = grid(3);
        // A box around columns 2..=4, rows 1..=3 on planes 0 and 3.
        let square = |z: f64| Contour {
            points: vec![
                [-6.5, -6.5, z],
                [-1.5, -6.5, z],
                [-1.5, -1.5, z],
                [-6.5, -1.5, z],
            ],
        };
        let roi = Roi {
            segment: Segment {
                number: 1,
                label: "PTV".into(),
                colour: [255, 0, 0],
                algorithm: String::new(),
                fractional: false,
                ml: 0.0,
                voxels: 0,
            },
            contours: vec![square(0.0), square(3.0)],
            skipped: 0,
        };
        let d = g.dvh(&roi);
        // 9 points per plane, 4 mm² each, 3 mm thick: 2 × 108 mm³.
        assert!((d.volume - 0.216).abs() < 1e-12);
        let c = d.cumulative();
        assert!((c[0] - 0.216).abs() < 1e-12);
        // Highest point: plane 3, row 3, column 4: 134 → 1.34 Gy, in bin 134.
        assert!((d.max() - 1.35).abs() < 1e-12);
        assert!((d.min() - 0.13).abs() < 1e-12);
        assert!(d.mean() > 0.5 && d.mean() < 1.0);
        assert!(d.dose_at(100.0) >= 0.12);
        assert_eq!(d.volume_at(50.0), 0.0);
        assert!((d.volume_at(0.0) - 0.216).abs() < 1e-12);
        let lines = isodose_lines(2.0);
        assert_eq!(lines[2].0, 2.0);
        assert_eq!(lines.len(), 9);
    }
}
