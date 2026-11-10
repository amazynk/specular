//! Label volumes over images: a NIfTI (or MGH, NRRD, MetaImage…) volume of integer labels
//! placed in patient space by its own world affine, as [`Labels`] the segmentation overlays
//! of MPR and the 2D medical view draw (the same renderer as DICOM SEG).
//!
//! Placement:
//! - NIfTI's affine is voxel → RAS; patient space here is DICOM's LPS, so x and y are
//!   negated (`specular_medvol` does this when it reads the header).
//! - The sform is used when `sform_code` > 0, else the qform, else the voxel sizes alone
//!   (nibabel's precedence, which `specular_medvol` follows).
//! - An oblique affine keeps its own grid: the overlay renderer samples the label volume
//!   by patient position, so it lines up with any series however both are tilted.
//! - A sheared affine (axes not perpendicular), which a grid of row, column and slice
//!   directions cannot hold, is resampled (nearest neighbour, by the exact inverse affine)
//!   onto a perpendicular grid around it.
//!
//! Labels are the volume's distinct non-zero values, sorted; each becomes a segment named and
//! coloured by a colour table ([`lut`]) when one is given, else coloured by [`lut::palette`].
//! Nothing is detected or classified (DESIGN §7): the file's values, shown.

pub mod lut;
#[doc(hidden)]
pub mod testing;

use std::collections::HashMap;

use specular_dicom::labels::{LabelSource, Labels, Layer, Segment};
use specular_dicom::volume::{Geometry, Volume, Voxels};
use specular_medvol::{MedVolume, Plane};

pub use lut::{Lut, palette};

/// Most distinct labels (a 16-bit layer holds index + 1).
pub const MAX_LABELS: usize = 65_534;
/// Most voxels a sheared volume is resampled to.
pub const MAX_RESAMPLED: u64 = 1 << 29;

/// Why a volume cannot be shown as labels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// RGB voxels.
    Colour,
    /// Values that are not whole numbers.
    NotWhole,
    /// More distinct labels than [`MAX_LABELS`].
    TooManyLabels(usize),
    /// Every voxel is 0.
    Empty,
    /// A sheared volume whose perpendicular grid would exceed [`MAX_RESAMPLED`] voxels.
    TooLarge(u64),
    /// The voxel axes are degenerate (a zero or parallel column in the affine).
    Degenerate,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::Colour => f.write_str("colour voxels"),
            Refusal::NotWhole => f.write_str("values that are not whole numbers"),
            Refusal::TooManyLabels(n) => write!(f, "{n} labels, over {MAX_LABELS}"),
            Refusal::Empty => f.write_str("no labelled voxels"),
            Refusal::TooLarge(n) => write!(f, "{n} voxels after resampling"),
            Refusal::Degenerate => f.write_str("a degenerate affine"),
        }
    }
}

/// A plane's values as whole numbers.
fn whole(p: Plane) -> Result<Vec<i64>, Refusal> {
    fn f<T: Copy + Into<f64>>(v: Vec<T>) -> Result<Vec<i64>, Refusal> {
        v.into_iter()
            .map(|x| {
                let x: f64 = x.into();
                if x.is_finite() && x.fract() == 0.0 && x.abs() < 9.0e15 {
                    Ok(x as i64)
                } else {
                    Err(Refusal::NotWhole)
                }
            })
            .collect()
    }
    Ok(match p {
        Plane::U8(v) => v.into_iter().map(i64::from).collect(),
        Plane::I8(v) => v.into_iter().map(i64::from).collect(),
        Plane::U16(v) => v.into_iter().map(i64::from).collect(),
        Plane::I16(v) => v.into_iter().map(i64::from).collect(),
        Plane::U32(v) => v.into_iter().map(i64::from).collect(),
        Plane::I32(v) => v.into_iter().map(i64::from).collect(),
        Plane::U64(v) => v
            .into_iter()
            .map(|x| i64::try_from(x).unwrap_or(i64::MAX))
            .collect(),
        Plane::I64(v) => v,
        Plane::F32(v) => f(v)?,
        Plane::F64(v) => f(v)?,
        Plane::Rgb8(_) => return Err(Refusal::Colour),
    })
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// A label grid being filled: the full grid's values (as label indices + 1) are not kept;
/// only counts, the bounding box and, in the second pass, the cropped layer.
struct Census {
    counts: HashMap<i64, u64>,
    lo: [usize; 3],
    hi: [usize; 3],
}

impl Census {
    fn new() -> Census {
        Census {
            counts: HashMap::new(),
            lo: [usize::MAX; 3],
            hi: [0; 3],
        }
    }

    fn add(&mut self, v: i64, at: [usize; 3]) {
        *self.counts.entry(v).or_insert(0) += 1;
        self.lo = [0, 1, 2].map(|a| self.lo[a].min(at[a]));
        self.hi = [0, 1, 2].map(|a| self.hi[a].max(at[a]));
    }
}

/// The geometry of `g`'s sub-grid from voxel `start`, `size` voxels.
fn crop(g: &Geometry, start: [usize; 3], size: [usize; 3]) -> Geometry {
    let positions: Vec<[f64; 3]> = (start[2]..start[2] + size[2])
        .map(|k| g.to_patient([start[0] as f64, start[1] as f64, k as f64]))
        .collect();
    Geometry::new(
        &positions,
        g.row,
        g.col,
        g.pixel,
        [size[0] as u32, size[1] as u32],
    )
}

/// A source of label values on a grid: `plane(k)` gives grid plane `k` (row-major).
fn build(
    grid: &Geometry,
    voxel_ml: f64,
    mut plane: impl FnMut(usize) -> Result<Vec<i64>, Refusal>,
    lut: Option<&Lut>,
    title: &str,
) -> Result<Labels, Refusal> {
    let [w, h, d] = grid.dims.map(|x| x as usize);
    let mut census = Census::new();
    for k in 0..d {
        let p = plane(k)?;
        for (idx, &v) in p.iter().enumerate().take(w * h) {
            if v != 0 {
                census.add(v, [idx % w, idx / w, k]);
            }
        }
        if census.counts.len() > MAX_LABELS {
            return Err(Refusal::TooManyLabels(census.counts.len()));
        }
    }
    if census.counts.is_empty() {
        return Err(Refusal::Empty);
    }
    let mut values: Vec<i64> = census.counts.keys().copied().collect();
    values.sort_unstable();
    let index: HashMap<i64, u16> = values
        .iter()
        .enumerate()
        .map(|(i, &v)| (v, (i + 1) as u16))
        .collect();
    // One voxel of margin, so outlines close at the crop's edge.
    let lo = [0, 1, 2].map(|a| census.lo[a].saturating_sub(1));
    let hi = [0, 1, 2].map(|a| (census.hi[a] + 1).min(grid.dims[a] as usize - 1));
    let size = [0, 1, 2].map(|a| hi[a] - lo[a] + 1);
    let mut buf = vec![0u16; size[0] * size[1] * size[2]];
    let mut last = (0i64, 0u16);
    for k in lo[2]..=hi[2] {
        let p = plane(k)?;
        let base = (k - lo[2]) * size[0] * size[1];
        for j in lo[1]..=hi[1] {
            for i in lo[0]..=hi[0] {
                let v = p.get(j * w + i).copied().unwrap_or(0);
                if v == 0 {
                    continue;
                }
                if v != last.0 {
                    last = (v, index[&v]);
                }
                buf[base + (j - lo[1]) * size[0] + (i - lo[0])] = last.1;
            }
        }
    }
    let geometry = crop(grid, lo, size);
    let volume = Volume {
        loaded: vec![true; size[2]],
        voxels: Voxels::U16(buf),
        geometry,
        step_xy: 1,
    };
    let segments = values
        .iter()
        .map(|&v| {
            let e = lut.and_then(|l| l.get(v));
            let n = census.counts[&v];
            Segment {
                number: v,
                label: e.map(|e| e.name.clone()).unwrap_or_default(),
                colour: e.map_or_else(|| palette(v), |e| e.colour),
                algorithm: String::new(),
                fractional: false,
                ml: n as f64 * voxel_ml,
                voxels: n,
            }
        })
        .collect();
    Ok(Labels {
        source: LabelSource::Volume,
        segments,
        layers: vec![Layer {
            volume,
            fraction: None,
        }],
        frame_of_reference: String::new(),
        referenced_series: Vec::new(),
        title: title.to_owned(),
    })
}

/// The labels of `vol` (its first volume), named and coloured by `lut` where it has the
/// value.
pub fn labels(vol: &MedVolume, lut: Option<&Lut>, title: &str) -> Result<Labels, Refusal> {
    if vol.header.is_colour() {
        return Err(Refusal::Colour);
    }
    let a = vol.header.affine;
    let col = |c: usize| [a[0][c], a[1][c], a[2][c]];
    let det = dot(col(0), cross(col(1), col(2)));
    if !det.is_finite() || det.abs() < 1e-12 {
        return Err(Refusal::Degenerate);
    }
    let voxel_ml = det.abs() / 1000.0;
    let c = vol.canonical();
    if c.skewed {
        return sheared(vol, lut, title, voxel_ml);
    }
    let [w, h, d] = c.dims;
    let normal = cross(c.row, c.col);
    // Slices in increasing order along the normal (the grid's convention).
    let reversed = dot(c.step, normal) < 0.0;
    let order = |k: usize| if reversed { d - 1 - k } else { k };
    let positions: Vec<[f64; 3]> = (0..d)
        .map(|k| c.position(0.0, 0.0, order(k) as f64))
        .collect();
    let grid = Geometry::new(
        &positions,
        c.row,
        c.col,
        [c.pixel[0], c.pixel[1]],
        [w as u32, h as u32],
    );
    build(
        &grid,
        voxel_ml,
        |k| whole(vol.plane(&c, order(k), 0)),
        lut,
        title,
    )
}

/// A sheared volume resampled onto a perpendicular grid around it.
fn sheared(
    vol: &MedVolume,
    lut: Option<&Lut>,
    title: &str,
    voxel_ml: f64,
) -> Result<Labels, Refusal> {
    let a = vol.header.affine;
    let size = vol.header.layout.size;
    let c = vol.canonical();
    let (row, colv) = (c.row, c.col);
    let normal = cross(row, colv);
    let t = [a[0][3], a[1][3], a[2][3]];
    let m = [
        [a[0][0], a[0][1], a[0][2]],
        [a[1][0], a[1][1], a[1][2]],
        [a[2][0], a[2][1], a[2][2]],
    ];
    let inv = invert(&m).ok_or(Refusal::Degenerate)?;
    // Spacing: the smallest voxel edge, on every axis.
    let step = vol
        .header
        .spacing
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min)
        .max(1e-6);
    // The source's corners (voxel edges) in the grid's frame.
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for corner in 0..8 {
        let v = [0, 1, 2].map(|ax| {
            if corner >> ax & 1 == 1 {
                size[ax] as f64 - 0.5
            } else {
                -0.5
            }
        });
        let p = [0, 1, 2].map(|r| dot(m[r], v) + t[r]);
        let q = [dot(p, row), dot(p, colv), dot(p, normal)];
        for ax in 0..3 {
            lo[ax] = lo[ax].min(q[ax]);
            hi[ax] = hi[ax].max(q[ax]);
        }
    }
    let dims = [0, 1, 2].map(|ax| (((hi[ax] - lo[ax]) / step).ceil() as u64).max(1));
    let total = dims[0] * dims[1] * dims[2];
    if total > MAX_RESAMPLED || dims.iter().any(|&x| x > u64::from(u32::MAX)) {
        return Err(Refusal::TooLarge(total));
    }
    let origin_q = [0, 1, 2].map(|ax| lo[ax] + step / 2.0);
    let to_lps = |q: [f64; 3]| [0, 1, 2].map(|r| row[r] * q[0] + colv[r] * q[1] + normal[r] * q[2]);
    let positions: Vec<[f64; 3]> = (0..dims[2])
        .map(|k| to_lps([origin_q[0], origin_q[1], origin_q[2] + k as f64 * step]))
        .collect();
    let grid = Geometry::new(
        &positions,
        row,
        colv,
        [step, step],
        [dims[0] as u32, dims[1] as u32],
    );
    let (w, h) = (dims[0] as usize, dims[1] as usize);
    // Source values by stored voxel, one stored slice at a time as the grid reaches it.
    let read = |k: usize| -> Result<Vec<i64>, Refusal> {
        let mut all = Vec::with_capacity(size[0] * size[1]);
        for j in 0..size[1] {
            for i in 0..size[0] {
                let x = vol.value(i, j, k, 0, 0);
                if !(x.is_finite() && x.fract() == 0.0) {
                    return Err(Refusal::NotWhole);
                }
                all.push(x as i64);
            }
        }
        Ok(all)
    };
    let mut slices: Vec<Option<Vec<i64>>> = vec![None; size[2]];
    build(
        &grid,
        voxel_ml,
        |kk| {
            let mut out = vec![0i64; w * h];
            for j in 0..h {
                for i in 0..w {
                    let p = grid.to_patient([i as f64, j as f64, kk as f64]);
                    let v = [0, 1, 2].map(|r| dot(inv[r], sub(p, t)));
                    let s = v.map(f64::round);
                    if (0..3).any(|ax| s[ax] < 0.0 || s[ax] >= size[ax] as f64) {
                        continue;
                    }
                    let (si, sj, sk) = (s[0] as usize, s[1] as usize, s[2] as usize);
                    if slices[sk].is_none() {
                        slices[sk] = Some(read(sk)?);
                    }
                    out[j * w + i] = slices[sk].as_ref().map_or(0, |p| p[sj * size[0] + si]);
                }
            }
            Ok(out)
        },
        lut,
        title,
    )
}

fn invert(m: &[[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let [a, b, c] = *m;
    let det = dot(a, cross(b, c));
    if det.abs() < 1e-12 {
        return None;
    }
    let (x, y, z) = (cross(b, c), cross(c, a), cross(a, b));
    Some([
        [x[0] / det, y[0] / det, z[0] / det],
        [x[1] / det, y[1] / det, z[1] / det],
        [x[2] / det, y[2] / det, z[2] / det],
    ])
}

/// The label value at patient position `p` (0 when none).
pub fn value_at(l: &Labels, p: [f64; 3]) -> i64 {
    l.at(p).first().map_or(0, |&s| l.segments[s].number)
}

/// The patient-space bounding box (min, max) of a grid's voxel centres.
fn bounds(g: &Geometry) -> ([f64; 3], [f64; 3]) {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in g.corners() {
        for a in 0..3 {
            lo[a] = lo[a].min(p[a]);
            hi[a] = hi[a].max(p[a]);
        }
    }
    (lo, hi)
}

/// The labels' grid and `g` overlap in patient space (their bounding boxes meet).
pub fn overlaps(l: &Labels, g: &Geometry) -> bool {
    let (b0, b1) = bounds(g);
    l.layers.iter().any(|layer| {
        let (a0, a1) = bounds(&layer.volume.geometry);
        (0..3).all(|ax| a0[ax] <= b1[ax] + 1e-6 && b0[ax] <= a1[ax] + 1e-6)
    })
}

#[cfg(test)]
mod tests;
