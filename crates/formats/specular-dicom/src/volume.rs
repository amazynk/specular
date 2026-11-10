//! Volumes from series (DESIGN.md §7.3): the slices of a series stacked along their normal,
//! their values in modality units (Hounsfield units for CT), and the geometry between voxel
//! and patient coordinates.
//!
//! * [`stack`] orders the slices of a [`Series`] (single-frame files, or one enhanced
//!   multi-frame file with its per-frame positions) and refuses, with a [`Refusal`], what
//!   would not make a faithful volume: slices that are not parallel, several images at one
//!   position (phases, echoes), missing slices.
//! * [`Geometry`] keeps every slice's own position. A gantry tilt (the slices shifted
//!   in-plane from one to the next) and uneven spacing are therefore exact: going from a
//!   patient point to voxel coordinates looks the slice up by its distance along the normal
//!   and removes that slice's in-plane shift (a shear), so nothing is resampled.
//!   [`Geometry::affine`] states the usual 4×4 voxel-to-patient matrix (its slice column is
//!   the mean slice step, sheared under a tilt), exact when the spacing is even.
//! * [`Plan`] fits the volume to the GPU's 3D texture size and the memory budget by taking
//!   every n-th pixel or slice (box-averaged in-plane).
//! * [`Volume`] holds the voxels (i16, u16 or f32, promoted to f32 when a slice needs it),
//!   filled slice by slice in [`load_order`] (every n-th slice first, then the gaps), and
//!   samples them trilinearly ([`Volume::sample`]), across slices not loaded yet as the GPU
//!   does, so the CPU and the GPU agree while a volume arrives.

use crate::display::{Modality, ModalityFrame};
use crate::pixel::{Grey, Pixels};
use crate::series::Series;

/// One slice of a stack: a frame of a file and its Image Position (Patient).
#[derive(Clone, Debug, PartialEq)]
pub struct Slice<K> {
    pub key: K,
    pub frame: u32,
    /// Patient position of the centre of the slice's first pixel, mm (LPS).
    pub position: [f64; 3],
}

/// The slices of a series in order along their normal.
#[derive(Clone, Debug, PartialEq)]
pub struct Stack<K> {
    pub slices: Vec<Slice<K>>,
    /// Direction of increasing column index (unit, LPS).
    pub row: [f64; 3],
    /// Direction of increasing row index.
    pub col: [f64; 3],
    /// `row × col`: the direction the slices are stacked in (positions increase along it).
    pub normal: [f64; 3],
    /// Typical distance between slices along the normal, mm.
    pub spacing: f64,
    /// The spacing varies by more than 10 % (kept exact, see [`Geometry`]).
    pub irregular: bool,
    /// Gantry tilt: the angle between the stacking direction and the normal, degrees, when
    /// above 0.5°. The in-plane shift is corrected (see [`Geometry`]).
    pub tilt: Option<f64>,
}

/// Why a series does not make a volume.
#[derive(Clone, Debug, PartialEq)]
pub enum Refusal {
    /// Fewer slices than a volume needs (the count).
    TooFew(usize),
    /// A slice states no position or orientation.
    NoGeometry,
    /// The slices are not parallel (or their rows point different ways).
    NotParallel,
    /// Several images share a position (phases, echoes, a time series): how many positions.
    SharedPosition(usize),
    /// Slices are missing between others (the estimated count).
    Missing(u32),
    /// A slice differs from the first in size or pixel spacing.
    Mismatch,
    /// A slice is colour, not grey values.
    NotGrey,
    /// Not even a downsampled volume fits (bytes needed, budget).
    TooLarge { need: u64, budget: u64 },
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::TooFew(n) => write!(f, "{n} slices are too few for a volume"),
            Refusal::NoGeometry => write!(f, "a slice states no position or orientation"),
            Refusal::NotParallel => write!(f, "the slices are not parallel"),
            Refusal::SharedPosition(n) => write!(f, "{n} positions hold several images"),
            Refusal::Missing(n) => write!(f, "about {n} slices are missing"),
            Refusal::Mismatch => write!(f, "the slices differ in size or pixel spacing"),
            Refusal::NotGrey => write!(f, "the slices are not grey values"),
            Refusal::TooLarge { need, budget } => {
                write!(f, "{} MiB needed, {} MiB allowed", need >> 20, budget >> 20)
            }
        }
    }
}

/// The fewest slices a volume is made of.
pub const MIN_SLICES: usize = 3;

pub(crate) fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(crate) fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(crate) fn normalise(v: [f64; 3]) -> Option<[f64; 3]> {
    let l = dot(v, v).sqrt();
    (l.is_finite() && l > 1e-9).then(|| [v[0] / l, v[1] / l, v[2] / l])
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// Orders the slices of `series` into a stack, or says why they do not make a volume.
pub fn stack<K: Clone>(series: &Series<K>) -> Result<Stack<K>, Refusal> {
    let mut slices: Vec<Slice<K>> = Vec::new();
    let mut orientation: Option<[f64; 6]> = None;
    for item in &series.items {
        let inst = &item.instance;
        let o = inst.orientation.ok_or(Refusal::NoGeometry)?;
        match orientation {
            None => orientation = Some(o),
            Some(first) => {
                let same = |a: &[f64], b: &[f64]| {
                    let (Some(a), Some(b)) =
                        (normalise([a[0], a[1], a[2]]), normalise([b[0], b[1], b[2]]))
                    else {
                        return false;
                    };
                    dot(a, b) > 0.9999
                };
                if !same(&first[..3], &o[..3]) || !same(&first[3..], &o[3..]) {
                    return Err(Refusal::NotParallel);
                }
            }
        }
        if inst.frames > 1 {
            // Enhanced multi-frame: every frame its own Plane Position.
            if inst.frame_positions.len() != inst.frames as usize {
                return Err(Refusal::NoGeometry);
            }
            for (f, p) in inst.frame_positions.iter().enumerate() {
                slices.push(Slice {
                    key: item.key.clone(),
                    frame: f as u32,
                    position: *p,
                });
            }
        } else {
            slices.push(Slice {
                key: item.key.clone(),
                frame: 0,
                position: inst.position.ok_or(Refusal::NoGeometry)?,
            });
        }
    }
    let o = orientation.ok_or(Refusal::TooFew(0))?;
    let (Some(row), Some(col)) = (normalise([o[0], o[1], o[2]]), normalise([o[3], o[4], o[5]]))
    else {
        return Err(Refusal::NoGeometry);
    };
    let normal = normalise(cross(row, col)).ok_or(Refusal::NoGeometry)?;
    stack_of(slices, row, col, normal)
}

/// [`stack`] from slices already gathered (any order).
pub fn stack_of<K>(
    mut slices: Vec<Slice<K>>,
    row: [f64; 3],
    col: [f64; 3],
    normal: [f64; 3],
) -> Result<Stack<K>, Refusal> {
    if slices.len() < MIN_SLICES {
        return Err(Refusal::TooFew(slices.len()));
    }
    slices.sort_by(|a, b| dot(a.position, normal).total_cmp(&dot(b.position, normal)));
    let along: Vec<f64> = slices.iter().map(|s| dot(s.position, normal)).collect();
    let diffs: Vec<f64> = along.windows(2).map(|w| w[1] - w[0]).collect();
    let shared = diffs.iter().filter(|d| **d < 1e-2).count();
    if shared > 0 {
        return Err(Refusal::SharedPosition(shared));
    }
    let mut sorted = diffs.clone();
    sorted.sort_by(f64::total_cmp);
    let spacing = sorted[sorted.len() / 2];
    let mut missing = 0;
    let mut irregular = false;
    for d in &diffs {
        let r = d / spacing;
        if r > 1.5 {
            missing += (r.round() as u32).saturating_sub(1).max(1);
        } else if !(0.9..=1.1).contains(&r) {
            irregular = true;
        }
    }
    if missing > 0 {
        return Err(Refusal::Missing(missing));
    }
    let first = slices[0].position;
    let last = slices[slices.len() - 1].position;
    let tilt = normalise(sub(last, first))
        .map(|d| dot(d, normal).abs().min(1.0).acos().to_degrees())
        .filter(|a| *a > 0.5);
    Ok(Stack {
        slices,
        row,
        col,
        normal,
        spacing,
        irregular,
        tilt,
    })
}

impl<K> Stack<K> {
    /// The geometry of the slices `pick` (indices into `slices`, increasing) at `cols` ×
    /// `rows` pixels `pixel` mm apart (x between columns, y between rows).
    pub fn geometry(&self, pick: &[usize], cols: u32, rows: u32, pixel: [f64; 2]) -> Geometry {
        let positions: Vec<[f64; 3]> = pick.iter().map(|&i| self.slices[i].position).collect();
        Geometry::new(&positions, self.row, self.col, pixel, [cols, rows])
    }

    /// The geometry of the volume `plan` makes of `cols` × `rows` slices: its slices, and
    /// under an in-plane step each voxel at the centre of the block it averages.
    pub fn planned(&self, plan: &Plan, cols: u32, rows: u32, pixel: [f64; 2]) -> Geometry {
        let s = f64::from(plan.step_xy.max(1));
        let half = (s - 1.0) / 2.0;
        let positions: Vec<[f64; 3]> = plan
            .pick(self.slices.len())
            .iter()
            .map(|&i| {
                let p = self.slices[i].position;
                [0, 1, 2]
                    .map(|c| p[c] + self.row[c] * half * pixel[0] + self.col[c] * half * pixel[1])
            })
            .collect();
        let step = plan.step_xy.max(1);
        Geometry::new(
            &positions,
            self.row,
            self.col,
            [pixel[0] * s, pixel[1] * s],
            [cols.div_ceil(step), rows.div_ceil(step)],
        )
    }
}

/// Voxel ↔ patient coordinates of a stack of slices.
///
/// Voxel coordinates `(i, j, s)`: `i` the column, `j` the row (integers at pixel centres),
/// `s` the slice (fractional between slices). Slice `k` lies at `origin + along[k]·normal`
/// plus its in-plane `shift[k]` (in voxels; non-zero under a gantry tilt).
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    /// Columns, rows, slices.
    pub dims: [u32; 3],
    pub row: [f64; 3],
    pub col: [f64; 3],
    pub normal: [f64; 3],
    /// mm between columns (x) and between rows (y).
    pub pixel: [f64; 2],
    /// The centre of voxel (0, 0, 0): slice 0's position.
    pub origin: [f64; 3],
    /// Each slice's distance from slice 0 along the normal, mm (increasing).
    pub along: Vec<f64>,
    /// Each slice's in-plane offset of its first pixel from the origin, in voxels.
    pub shift: Vec<[f64; 2]>,
}

impl Geometry {
    /// From the slices' positions (in order along the normal).
    pub fn new(
        positions: &[[f64; 3]],
        row: [f64; 3],
        col: [f64; 3],
        pixel: [f64; 2],
        size: [u32; 2],
    ) -> Geometry {
        let normal = normalise(cross(row, col)).unwrap_or([0.0, 0.0, 1.0]);
        let origin = positions.first().copied().unwrap_or_default();
        let mut along = Vec::with_capacity(positions.len());
        let mut shift = Vec::with_capacity(positions.len());
        for p in positions {
            let d = sub(*p, origin);
            along.push(dot(d, normal));
            shift.push([dot(d, row) / pixel[0], dot(d, col) / pixel[1]]);
        }
        Geometry {
            dims: [size[0], size[1], positions.len() as u32],
            row,
            col,
            normal,
            pixel,
            origin,
            along,
            shift,
        }
    }

    /// Slices.
    pub fn slices(&self) -> usize {
        self.along.len()
    }

    /// Mean distance between slices along the normal, mm.
    pub fn spacing(&self) -> f64 {
        let n = self.along.len();
        if n < 2 {
            return 1.0;
        }
        (self.along[n - 1] - self.along[0]) / (n - 1) as f64
    }

    /// mm per voxel step along i, j and k (k: the mean spacing).
    pub fn voxel_mm(&self) -> [f64; 3] {
        [self.pixel[0], self.pixel[1], self.spacing()]
    }

    /// The slice coordinate and in-plane shift at `a` mm along the normal from slice 0,
    /// extrapolated past the ends with the end slices' spacing.
    pub fn slice_at(&self, a: f64) -> (f64, [f64; 2]) {
        let n = self.along.len();
        if n < 2 {
            return (0.0, self.shift.first().copied().unwrap_or_default());
        }
        // The segment [k, k+1] holding `a` (the first or last one when outside).
        let k = match self.along.partition_point(|&x| x <= a) {
            0 => 0,
            p if p >= n => n - 2,
            p => p - 1,
        };
        let (a0, a1) = (self.along[k], self.along[k + 1]);
        let t = (a - a0) / (a1 - a0);
        let (s0, s1) = (self.shift[k], self.shift[k + 1]);
        (
            k as f64 + t,
            [s0[0] + (s1[0] - s0[0]) * t, s0[1] + (s1[1] - s0[1]) * t],
        )
    }

    /// The patient position (mm, LPS) of voxel coordinates `v`.
    pub fn to_patient(&self, v: [f64; 3]) -> [f64; 3] {
        let n = self.along.len();
        let (a, sh) = if n < 2 {
            (0.0, [0.0, 0.0])
        } else {
            let k = (v[2].floor().max(0.0) as usize).min(n - 2);
            let t = v[2] - k as f64;
            let (s0, s1) = (self.shift[k], self.shift[k + 1]);
            (
                self.along[k] + (self.along[k + 1] - self.along[k]) * t,
                [s0[0] + (s1[0] - s0[0]) * t, s0[1] + (s1[1] - s0[1]) * t],
            )
        };
        let x = (v[0] + sh[0]) * self.pixel[0];
        let y = (v[1] + sh[1]) * self.pixel[1];
        [0, 1, 2].map(|c| self.origin[c] + x * self.row[c] + y * self.col[c] + a * self.normal[c])
    }

    /// The voxel coordinates of patient position `p` (the inverse of [`Geometry::to_patient`]).
    pub fn to_voxel(&self, p: [f64; 3]) -> [f64; 3] {
        let d = sub(p, self.origin);
        let (s, sh) = self.slice_at(dot(d, self.normal));
        [
            dot(d, self.row) / self.pixel[0] - sh[0],
            dot(d, self.col) / self.pixel[1] - sh[1],
            s,
        ]
    }

    /// The 4×4 voxel-to-patient matrix (row-major; columns i, j, k, translation). The k
    /// column is the mean step from slice to slice, so it is sheared under a gantry tilt; it
    /// is exact for evenly spaced slices ([`Geometry::to_patient`] is exact always).
    pub fn affine(&self) -> [[f64; 4]; 4] {
        let n = self.along.len();
        let k = if n < 2 {
            self.normal
        } else {
            let last = self.to_patient([0.0, 0.0, (n - 1) as f64]);
            sub(last, self.origin).map(|c| c / (n - 1) as f64)
        };
        let mut m = [[0.0; 4]; 4];
        for r in 0..3 {
            m[r][0] = self.row[r] * self.pixel[0];
            m[r][1] = self.col[r] * self.pixel[1];
            m[r][2] = k[r];
            m[r][3] = self.origin[r];
        }
        m[3][3] = 1.0;
        m
    }

    /// The patient position of the volume's centre.
    pub fn centre(&self) -> [f64; 3] {
        self.to_patient([
            (self.dims[0] as f64 - 1.0) / 2.0,
            (self.dims[1] as f64 - 1.0) / 2.0,
            (self.dims[2] as f64 - 1.0) / 2.0,
        ])
    }

    /// The eight corners of the voxel box (outer edges, half a voxel past the centres).
    pub fn corners(&self) -> [[f64; 3]; 8] {
        let e = [
            self.dims[0] as f64 - 0.5,
            self.dims[1] as f64 - 0.5,
            self.dims[2] as f64 - 0.5,
        ];
        let mut out = [[0.0; 3]; 8];
        for (n, c) in out.iter_mut().enumerate() {
            let pick = |bit: usize, hi: f64| if n >> bit & 1 == 1 { hi } else { -0.5 };
            *c = self.to_patient([pick(0, e[0]), pick(1, e[1]), pick(2, e[2])]);
        }
        out
    }

    /// The slice lookup for the GPU: `nodes` evenly spaced entries over the slices' extent
    /// (half a slice past each end), each `[s, shift_i, shift_j, 0]` at that distance along
    /// the normal; linear between entries. Exact for evenly spaced slices.
    pub fn table(&self, nodes: usize) -> SliceTable {
        let n = self.along.len();
        let nodes = nodes.max(2);
        let (lo, hi) = if n < 2 {
            (-0.5, 0.5)
        } else {
            let first = self.along[1] - self.along[0];
            let last = self.along[n - 1] - self.along[n - 2];
            (-0.5 * first, self.along[n - 1] + 0.5 * last)
        };
        let step = (hi - lo) / (nodes - 1) as f64;
        let entries = (0..nodes)
            .map(|i| {
                let (s, sh) = self.slice_at(lo + step * i as f64);
                [s as f32, sh[0] as f32, sh[1] as f32, 0.0]
            })
            .collect();
        SliceTable {
            start: lo,
            step,
            entries,
        }
    }

    /// Entries for [`Geometry::table`]: four per slice (enough for the uneven spacing CT
    /// produces), at least 64, at most 8192.
    pub fn table_nodes(&self) -> usize {
        (self.along.len() * 4).clamp(64, 8192)
    }
}

/// See [`Geometry::table`].
#[derive(Clone, Debug, PartialEq)]
pub struct SliceTable {
    /// mm along the normal (from slice 0) of the first entry.
    pub start: f64,
    /// mm between entries.
    pub step: f64,
    pub entries: Vec<[f32; 4]>,
}

/// How voxels are stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoxelKind {
    I16,
    U16,
    F32,
}

impl VoxelKind {
    pub fn bytes(self) -> u64 {
        match self {
            VoxelKind::I16 | VoxelKind::U16 => 2,
            VoxelKind::F32 => 4,
        }
    }

    /// The narrowest kind holding every modality value of `f` exactly: its stored range (or,
    /// when that is wider, its values) through an integer rescale or a Modality LUT as i16 or
    /// u16, else f32.
    pub fn of(f: &ModalityFrame) -> VoxelKind {
        let Pixels::Grey(g) = &f.frame.pixels else {
            return VoxelKind::F32;
        };
        if matches!(g, Grey::F32(_) | Grey::F64(_)) {
            return VoxelKind::F32;
        }
        let bits = u32::from(f.frame.bits.clamp(1, 32));
        let (lo, hi) = if f.frame.signed {
            (-(1i64 << (bits - 1)), (1i64 << (bits - 1)) - 1)
        } else {
            (0, (1i64 << bits) - 1)
        };
        let (a, b) = match &f.map.modality {
            Modality::Rescale { slope, intercept } => {
                if slope.fract() != 0.0 || intercept.fract() != 0.0 {
                    return VoxelKind::F32;
                }
                let (x, y) = (f.map.modality.apply(lo), f.map.modality.apply(hi));
                (x.min(y), x.max(y))
            }
            Modality::Lut(l) => (0.0, ((1u32 << l.bits.min(16)) - 1) as f64),
        };
        let pick = |a: f64, b: f64| {
            if a >= -32768.0 && b <= 32767.0 {
                Some(VoxelKind::I16)
            } else if a >= 0.0 && b <= 65535.0 {
                Some(VoxelKind::U16)
            } else {
                None
            }
        };
        // The stored range rarely fills 16 bits: the frame's own values decide then (a later
        // slice outside them promotes the volume to f32).
        pick(a, b)
            .or_else(|| f.value_range().and_then(|(a, b)| pick(a, b)))
            .unwrap_or(VoxelKind::F32)
    }
}

/// Voxel values in modality units, slice after slice, row after row.
#[derive(Clone, Debug, PartialEq)]
pub enum Voxels {
    I16(Vec<i16>),
    U16(Vec<u16>),
    F32(Vec<f32>),
}

impl Voxels {
    pub fn kind(&self) -> VoxelKind {
        match self {
            Voxels::I16(_) => VoxelKind::I16,
            Voxels::U16(_) => VoxelKind::U16,
            Voxels::F32(_) => VoxelKind::F32,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Voxels::I16(v) => v.len(),
            Voxels::U16(v) => v.len(),
            Voxels::F32(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn get(&self, i: usize) -> f32 {
        match self {
            Voxels::I16(v) => f32::from(v[i]),
            Voxels::U16(v) => f32::from(v[i]),
            Voxels::F32(v) => v[i],
        }
    }

    fn zeroed(kind: VoxelKind, n: usize) -> Result<Voxels, Refusal> {
        let too_large = || Refusal::TooLarge {
            need: n as u64 * kind.bytes(),
            budget: 0,
        };
        Ok(match kind {
            VoxelKind::I16 => Voxels::I16(alloc(n).ok_or_else(too_large)?),
            VoxelKind::U16 => Voxels::U16(alloc(n).ok_or_else(too_large)?),
            VoxelKind::F32 => Voxels::F32(alloc(n).ok_or_else(too_large)?),
        })
    }
}

fn alloc<T: Clone + Default>(n: usize) -> Option<Vec<T>> {
    let mut v = Vec::new();
    v.try_reserve_exact(n).ok()?;
    v.resize(n, T::default());
    Some(v)
}

/// How a series becomes a volume within the limits: every `step_xy`-th pixel (box-averaged)
/// and every `step_z`-th slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub step_xy: u32,
    pub step_z: u32,
    /// The volume's columns, rows and slices.
    pub dims: [u32; 3],
    pub bytes: u64,
}

impl Plan {
    /// For `size` = (columns, rows, slices) of `kind` voxels, `max_dim` the largest 3D
    /// texture side, `budget` bytes. In-plane pixels are halved while they are finer than
    /// the slice spacing (`pixel` and `spacing`, mm), else every other slice is dropped.
    pub fn new(
        size: [u32; 3],
        kind: VoxelKind,
        pixel: f64,
        spacing: f64,
        max_dim: u32,
        budget: u64,
    ) -> Result<Plan, Refusal> {
        let max_dim = max_dim.max(1);
        let full = size[0] as u64 * size[1] as u64 * size[2] as u64 * kind.bytes();
        let dims = |xy: u32, z: u32| {
            [
                size[0].div_ceil(xy).max(1),
                size[1].div_ceil(xy).max(1),
                size[2].div_ceil(z).max(1),
            ]
        };
        let bytes = |d: [u32; 3]| d[0] as u64 * d[1] as u64 * d[2] as u64 * kind.bytes();
        let (mut xy, mut z) = (1u32, 1u32);
        while size[0].div_ceil(xy) > max_dim || size[1].div_ceil(xy) > max_dim {
            xy *= 2;
        }
        while size[2].div_ceil(z) > max_dim {
            z += 1;
        }
        while bytes(dims(xy, z)) > budget {
            let d = dims(xy, z);
            if d[0] <= 16 && d[1] <= 16 && d[2] <= MIN_SLICES as u32 {
                return Err(Refusal::TooLarge { need: full, budget });
            }
            let finer_in_plane = pixel * xy as f64 <= spacing * z as f64;
            if (finer_in_plane || d[2] <= MIN_SLICES as u32) && d[0] > 16 && d[1] > 16 {
                xy *= 2;
            } else if d[2] > MIN_SLICES as u32 {
                z += 1;
            } else {
                xy *= 2;
            }
        }
        let d = dims(xy, z);
        Ok(Plan {
            step_xy: xy,
            step_z: z,
            dims: d,
            bytes: bytes(d),
        })
    }

    /// The stack slices this plan keeps (every `step_z`-th, from the first).
    pub fn pick(&self, slices: usize) -> Vec<usize> {
        (0..slices).step_by(self.step_z as usize).collect()
    }
}

/// The order slices load in: every n-th first (the first and last included), so the whole
/// volume shows coarsely within a few slices, then the slices halfway between, and so on.
pub fn load_order(n: usize) -> Vec<usize> {
    if n == 0 {
        return Vec::new();
    }
    let mut stride = 1usize;
    while stride * 16 < n {
        stride *= 2;
    }
    let mut done = vec![false; n];
    let mut out = Vec::with_capacity(n);
    let mut push = |i: usize, out: &mut Vec<usize>| {
        if !done[i] {
            done[i] = true;
            out.push(i);
        }
    };
    for i in (0..n).step_by(stride) {
        push(i, &mut out);
    }
    push(n - 1, &mut out);
    while stride > 1 {
        let half = stride / 2;
        for i in (half..n).step_by(stride) {
            push(i, &mut out);
        }
        stride = half;
    }
    out
}

/// A volume of modality values.
#[derive(Clone, Debug, PartialEq)]
pub struct Volume {
    pub geometry: Geometry,
    pub voxels: Voxels,
    /// Slices filled in so far.
    pub loaded: Vec<bool>,
    /// The in-plane step the voxels were made with (1: full resolution).
    pub step_xy: u32,
}

impl Volume {
    /// An empty volume of `kind` voxels.
    pub fn new(geometry: Geometry, kind: VoxelKind, step_xy: u32) -> Result<Volume, Refusal> {
        let [w, h, d] = geometry.dims;
        let n = w as usize * h as usize * d as usize;
        Ok(Volume {
            voxels: Voxels::zeroed(kind, n)?,
            loaded: vec![false; d as usize],
            geometry,
            step_xy: step_xy.max(1),
        })
    }

    /// Voxels in one slice.
    pub fn slice_len(&self) -> usize {
        self.geometry.dims[0] as usize * self.geometry.dims[1] as usize
    }

    /// Slices filled in.
    pub fn loaded_count(&self) -> usize {
        self.loaded.iter().filter(|l| **l).count()
    }

    pub fn complete(&self) -> bool {
        self.loaded.iter().all(|l| *l)
    }

    /// Bytes of the voxels.
    pub fn bytes(&self) -> u64 {
        self.voxels.len() as u64 * self.voxels.kind().bytes()
    }

    /// Fills slice `k` from a frame's modality values (`width` × `height`, row-major),
    /// box-averaging `step_xy` × `step_xy` pixels. Returns whether the voxels were promoted to
    /// f32 (a value that the integer kind does not hold), which re-uploads the whole volume.
    pub fn put(
        &mut self,
        k: usize,
        values: &[f32],
        width: u32,
        height: u32,
    ) -> Result<bool, Refusal> {
        let [w, h, d] = self.geometry.dims;
        if k >= d as usize {
            return Err(Refusal::Mismatch);
        }
        let step = self.step_xy;
        if width.div_ceil(step) != w || height.div_ceil(step) != h {
            return Err(Refusal::Mismatch);
        }
        if values.len() < width as usize * height as usize {
            return Err(Refusal::Mismatch);
        }
        let reduced;
        let src: &[f32] = if step == 1 {
            values
        } else {
            reduced = box_reduce(values, width, height, step);
            &reduced
        };
        let exact = step == 1;
        let base = k * self.slice_len();
        let n = self.slice_len();
        let mut promoted = false;
        let fits = match &self.voxels {
            Voxels::I16(_) => src[..n].iter().all(|&v| {
                let r = if exact { v } else { v.round() };
                (r as i16) as f32 == r
            }),
            Voxels::U16(_) => src[..n].iter().all(|&v| {
                let r = if exact { v } else { v.round() };
                (r as u16) as f32 == r
            }),
            Voxels::F32(_) => true,
        };
        if !fits {
            self.promote();
            promoted = true;
        }
        match &mut self.voxels {
            Voxels::I16(v) => {
                for (o, &x) in v[base..base + n].iter_mut().zip(src) {
                    *o = x.round() as i16;
                }
            }
            Voxels::U16(v) => {
                for (o, &x) in v[base..base + n].iter_mut().zip(src) {
                    *o = x.round() as u16;
                }
            }
            Voxels::F32(v) => v[base..base + n].copy_from_slice(&src[..n]),
        }
        self.loaded[k] = true;
        Ok(promoted)
    }

    /// Fills slice `k` from a decoded frame (see [`Volume::put`]).
    pub fn put_frame(&mut self, k: usize, f: &ModalityFrame) -> Result<bool, Refusal> {
        let values = f.values_f32().ok_or(Refusal::NotGrey)?;
        self.put(k, &values, f.frame.width, f.frame.height)
    }

    /// Converts the voxels to f32.
    pub fn promote(&mut self) {
        let v: Vec<f32> = match &self.voxels {
            Voxels::I16(v) => v.iter().map(|&x| f32::from(x)).collect(),
            Voxels::U16(v) => v.iter().map(|&x| f32::from(x)).collect(),
            Voxels::F32(_) => return,
        };
        self.voxels = Voxels::F32(v);
    }

    /// The value of voxel (i, j, k).
    #[inline]
    pub fn value(&self, i: u32, j: u32, k: u32) -> f32 {
        let [w, h, _] = self.geometry.dims;
        self.voxels
            .get((k as usize * h as usize + j as usize) * w as usize + i as usize)
    }

    /// For every slice, the nearest loaded slice at or below it and at or above it (`-1` and
    /// `slices` when there is none): what the GPU blends while the volume loads.
    pub fn neighbours(&self) -> Vec<[i32; 2]> {
        let n = self.loaded.len();
        let mut out = vec![[-1, n as i32]; n];
        let mut last = -1;
        for (k, o) in out.iter_mut().enumerate() {
            if self.loaded[k] {
                last = k as i32;
            }
            o[0] = last;
        }
        let mut next = n as i32;
        for k in (0..n).rev() {
            if self.loaded[k] {
                next = k as i32;
            }
            out[k][1] = next;
        }
        out
    }

    /// The value at voxel coordinates `v`: bilinear in-plane (clamped to the edge pixels),
    /// linear between the nearest loaded slices. `None` outside the volume (more than half a
    /// voxel past the edge centres) or before any slice loaded.
    pub fn sample(&self, v: [f64; 3]) -> Option<f32> {
        let [w, h, d] = self.geometry.dims;
        let inside = |x: f64, n: u32| x >= -0.5 && x <= n as f64 - 0.5;
        if !inside(v[0], w) || !inside(v[1], h) || !inside(v[2], d) {
            return None;
        }
        let s = v[2].clamp(0.0, d as f64 - 1.0);
        let k0 = (s.floor() as usize).min(d as usize - 1);
        let k1 = (k0 + 1).min(d as usize - 1);
        let below = (0..=k0).rev().find(|&k| self.loaded[k]);
        let above = (k1..d as usize).find(|&k| self.loaded[k]);
        let (a, b, t) = match (below, above) {
            (Some(a), Some(b)) if b > a => (a, b, ((s - a as f64) / (b - a) as f64) as f32),
            (Some(a), _) => (a, a, 0.0),
            (None, Some(b)) => (b, b, 0.0),
            (None, None) => return None,
        };
        let plane = |k: usize| {
            let x = v[0].clamp(0.0, w as f64 - 1.0);
            let y = v[1].clamp(0.0, h as f64 - 1.0);
            let (x0, y0) = (x.floor() as u32, y.floor() as u32);
            let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
            let (fx, fy) = ((x - x0 as f64) as f32, (y - y0 as f64) as f32);
            let k = k as u32;
            let top = self.value(x0, y0, k) * (1.0 - fx) + self.value(x1, y0, k) * fx;
            let bot = self.value(x0, y1, k) * (1.0 - fx) + self.value(x1, y1, k) * fx;
            top * (1.0 - fy) + bot * fy
        };
        let pa = plane(a);
        Some(if b == a { pa } else { pa + (plane(b) - pa) * t })
    }

    /// The value at patient position `p` (mm, LPS); see [`Volume::sample`].
    pub fn sample_patient(&self, p: [f64; 3]) -> Option<f32> {
        self.sample(self.geometry.to_voxel(p))
    }
}

/// Averages `step` × `step` blocks (partial blocks at the right and bottom edges average
/// what they hold).
fn box_reduce(values: &[f32], width: u32, height: u32, step: u32) -> Vec<f32> {
    let (w, h) = (width.div_ceil(step), height.div_ceil(step));
    let mut out = vec![0f32; w as usize * h as usize];
    for y in 0..h {
        let y0 = y * step;
        let y1 = (y0 + step).min(height);
        for x in 0..w {
            let x0 = x * step;
            let x1 = (x0 + step).min(width);
            let mut sum = 0f64;
            for yy in y0..y1 {
                let row = &values[(yy * width) as usize..][..width as usize];
                for &v in &row[x0 as usize..x1 as usize] {
                    sum += f64::from(v);
                }
            }
            out[(y * w + x) as usize] = (sum / ((y1 - y0) * (x1 - x0)) as f64) as f32;
        }
    }
    out
}

#[doc(hidden)]
pub mod phantom;

#[cfg(test)]
#[path = "volume_tests.rs"]
mod tests;
