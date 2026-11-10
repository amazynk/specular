//! Scientific array files: NumPy `.npy` and `.npz`, MATLAB `.mat` (v4 and v5; v7.3 is HDF5
//! and read through `specular-hdf5` by the caller) and plain Zarr v2 / v3 stores (folders and
//! zips).
//!
//! Every reader describes its arrays by element type, logical shape (slowest dimension first,
//! the way NumPy prints it) and where the elements lie; [`read_plane`] reads one 2-D plane (or a
//! row of a 1-D array) of any of them as `f32`, whatever the storage order. Complex arrays
//! read as one [`Part`] at a time.
//!
//! **Safety of input.** Counts are checked against the bytes present before anything is
//! allocated, decompression is bounded by the size the metadata promises, and no input
//! panics.

pub mod mat;
pub mod npy;
pub mod npz;
pub mod zarr;
mod zip;

pub use zip::{Entry as ZipEntry, Zip};

/// Values of one plane at most.
pub const MAX_PLANE: u64 = 1 << 28;
/// Dimensions of an array at most.
pub const MAX_DIMS: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not an array file of this kind")]
    NotArray,
    #[error("truncated: {0}")]
    Truncated(String),
    #[error("malformed: {0}")]
    Malformed(String),
    /// A variant the readers refuse, with a plain-language reason.
    #[error("{0}")]
    Unsupported(String),
    #[error("too large")]
    TooLarge,
    /// A remote store could not be read (offline, refused, timed out), worded for the user.
    #[error("{0}")]
    Io(String),
    /// Under the caps, but more than the memory budget has free now.
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn bad<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Malformed(msg.into()))
}

/// An element type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Elem {
    Bool,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F16,
    F32,
    F64,
    /// Two 32-bit floats, real first.
    C64,
    /// Two 64-bit floats, real first.
    C128,
}

impl Elem {
    pub fn size(self) -> usize {
        match self {
            Elem::Bool | Elem::I8 | Elem::U8 => 1,
            Elem::I16 | Elem::U16 | Elem::F16 => 2,
            Elem::I32 | Elem::U32 | Elem::F32 => 4,
            Elem::I64 | Elem::U64 | Elem::F64 | Elem::C64 => 8,
            Elem::C128 => 16,
        }
    }

    pub fn is_complex(self) -> bool {
        matches!(self, Elem::C64 | Elem::C128)
    }

    pub fn is_float(self) -> bool {
        matches!(
            self,
            Elem::F16 | Elem::F32 | Elem::F64 | Elem::C64 | Elem::C128
        )
    }

    /// Bits per value (per part for complex).
    pub fn bits(self) -> u32 {
        match self {
            Elem::Bool => 1,
            Elem::C64 => 32,
            Elem::C128 => 64,
            e => e.size() as u32 * 8,
        }
    }

    /// "16-bit unsigned", "64-bit complex float"…
    pub fn describe(self) -> &'static str {
        match self {
            Elem::Bool => "boolean",
            Elem::I8 => "8-bit signed",
            Elem::U8 => "8-bit unsigned",
            Elem::I16 => "16-bit signed",
            Elem::U16 => "16-bit unsigned",
            Elem::I32 => "32-bit signed",
            Elem::U32 => "32-bit unsigned",
            Elem::I64 => "64-bit signed",
            Elem::U64 => "64-bit unsigned",
            Elem::F16 => "16-bit float",
            Elem::F32 => "32-bit float",
            Elem::F64 => "64-bit float",
            Elem::C64 => "complex 32-bit float",
            Elem::C128 => "complex 64-bit float",
        }
    }

    /// The real type of one part of a complex element (itself for real types).
    pub fn real(self) -> Elem {
        match self {
            Elem::C64 => Elem::F32,
            Elem::C128 => Elem::F64,
            e => e,
        }
    }
}

/// An element type and its byte order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Dtype {
    pub elem: Elem,
    pub big: bool,
}

impl Dtype {
    pub const fn le(elem: Elem) -> Dtype {
        Dtype { elem, big: false }
    }

    pub fn size(self) -> usize {
        self.elem.size()
    }
}

/// Which value of a complex element a plane shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Part {
    #[default]
    Magnitude,
    Real,
    Imaginary,
    /// The argument, radians.
    Phase,
}

impl Part {
    /// The four parts, in the order offered.
    pub const ALL: [Part; 4] = [Part::Magnitude, Part::Real, Part::Imaginary, Part::Phase];

    pub fn name(self) -> &'static str {
        match self {
            Part::Magnitude => "Magnitude",
            Part::Real => "Real",
            Part::Imaginary => "Imaginary",
            Part::Phase => "Phase",
        }
    }

    #[inline]
    pub fn of(self, re: f64, im: f64) -> f32 {
        (match self {
            Part::Magnitude => re.hypot(im),
            Part::Real => re,
            Part::Imaginary => im,
            Part::Phase => im.atan2(re),
        }) as f32
    }
}

/// Element strides (in elements) of `shape` stored with dimension `perm[0]` slowest and
/// `perm[last]` fastest (C order is `0, 1, …`; Fortran order its reverse).
pub fn strides_perm(shape: &[u64], perm: &[usize]) -> Vec<u64> {
    let mut s = vec![0u64; shape.len()];
    let mut n = 1u64;
    for &d in perm.iter().rev() {
        if let Some(v) = s.get_mut(d) {
            *v = n;
            n = n.saturating_mul(shape[d].max(1));
        }
    }
    s
}

/// Element strides of `shape` in C (`fortran` false) or Fortran order.
pub fn strides(shape: &[u64], fortran: bool) -> Vec<u64> {
    let perm: Vec<usize> = if fortran {
        (0..shape.len()).rev().collect()
    } else {
        (0..shape.len()).collect()
    };
    strides_perm(shape, &perm)
}

/// Elements of `shape`, `None` on overflow.
pub fn count(shape: &[u64]) -> Option<u64> {
    shape.iter().try_fold(1u64, |n, &d| n.checked_mul(d))
}

/// One plane of an N-D array: the dimension along x, the one down y (`None` for a single row
/// of a 1-D array), and a position on every other dimension (entries for x and y ignored).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaneSpec {
    pub x_dim: usize,
    pub y_dim: Option<usize>,
    pub index: Vec<u64>,
}

impl PlaneSpec {
    /// Width and height of the plane of an array of `shape`.
    pub fn size(&self, shape: &[u64]) -> (usize, usize) {
        let w = shape.get(self.x_dim).copied().unwrap_or(1) as usize;
        let h = self.y_dim.and_then(|d| shape.get(d)).copied().unwrap_or(1) as usize;
        (w, h)
    }

    /// Checks the plane against `shape`: dimensions in range and distinct, positions in
    /// range, and at most [`MAX_PLANE`] values.
    pub fn check(&self, shape: &[u64]) -> Result<(usize, usize)> {
        let n = shape.len();
        if self.x_dim >= n.max(1) || self.y_dim.is_some_and(|y| y >= n || y == self.x_dim) {
            return bad("no such plane");
        }
        for (d, &len) in shape.iter().enumerate() {
            if d == self.x_dim || Some(d) == self.y_dim {
                continue;
            }
            if self.index.get(d).copied().unwrap_or(0) >= len {
                return bad("no such plane");
            }
        }
        let (w, h) = self.size(shape);
        if (w as u64).saturating_mul(h as u64) > MAX_PLANE {
            return Err(Error::TooLarge);
        }
        Ok((w, h))
    }

    /// The element offset of the plane's first value under `strides`.
    pub fn base(&self, strides: &[u64]) -> u64 {
        strides
            .iter()
            .enumerate()
            .filter(|&(d, _)| d != self.x_dim && Some(d) != self.y_dim)
            .map(|(d, s)| self.index.get(d).copied().unwrap_or(0).saturating_mul(*s))
            .fold(0u64, u64::saturating_add)
    }
}

#[inline]
fn arr<const N: usize>(b: &[u8], big: bool) -> [u8; N] {
    let mut a = [0u8; N];
    a.copy_from_slice(&b[..N]);
    if big {
        a.reverse();
    }
    a
}

/// Reads elements `base + y·sy + x·sx` (in elements) of `bytes` into `out` (`w` per row)
/// through `f`; elements past the end are NaN. Returns whether any were.
#[inline]
#[allow(clippy::too_many_arguments)]
fn gather_with<F: Fn(&[u8]) -> f32>(
    bytes: &[u8],
    size: usize,
    base: u64,
    sx: u64,
    sy: u64,
    w: usize,
    out: &mut [f32],
    f: F,
) -> bool {
    let mut truncated = false;
    let size64 = size as u64;
    for (y, row) in out.chunks_mut(w.max(1)).enumerate() {
        let start = base.saturating_add((y as u64).saturating_mul(sy));
        // A contiguous row entirely present: one slice, no per-element checks.
        let end = start
            .saturating_add((w as u64).saturating_mul(sx).saturating_sub(sx))
            .saturating_add(1)
            .saturating_mul(size64);
        if sx == 1 && end <= bytes.len() as u64 {
            let s = (start * size64) as usize;
            for (o, b) in row.iter_mut().zip(bytes[s..].chunks_exact(size)) {
                *o = f(b);
            }
            continue;
        }
        for (x, o) in row.iter_mut().enumerate() {
            let e = start.saturating_add((x as u64).saturating_mul(sx));
            let at = e.saturating_mul(size64);
            match usize::try_from(at)
                .ok()
                .and_then(|a| bytes.get(a..a.checked_add(size)?))
            {
                Some(b) => *o = f(b),
                None => {
                    *o = f32::NAN;
                    truncated = true;
                }
            }
        }
    }
    truncated
}

/// Reads rows of an array stored in `bytes` into `out`: `w` values per row, rows of `out` at element stride
/// `sy`, values at stride `sx`, from element `base`.
#[allow(clippy::too_many_arguments)]
pub fn gather_rows(
    bytes: &[u8],
    dtype: Dtype,
    base: u64,
    sx: u64,
    sy: u64,
    w: usize,
    part: Part,
    out: &mut [f32],
) -> bool {
    let big = dtype.big;
    let size = dtype.size();
    macro_rules! go {
        ($f:expr) => {
            gather_with(bytes, size, base, sx, sy, w, out, $f)
        };
    }
    match dtype.elem {
        Elem::Bool => go!(|b: &[u8]| f32::from(u8::from(b[0] != 0))),
        Elem::U8 => go!(|b: &[u8]| f32::from(b[0])),
        Elem::I8 => go!(|b: &[u8]| f32::from(b[0] as i8)),
        Elem::U16 => go!(|b: &[u8]| f32::from(u16::from_le_bytes(arr(b, big)))),
        Elem::I16 => go!(|b: &[u8]| f32::from(i16::from_le_bytes(arr(b, big)))),
        Elem::U32 => go!(|b: &[u8]| u32::from_le_bytes(arr(b, big)) as f32),
        Elem::I32 => go!(|b: &[u8]| i32::from_le_bytes(arr(b, big)) as f32),
        Elem::U64 => go!(|b: &[u8]| u64::from_le_bytes(arr(b, big)) as f32),
        Elem::I64 => go!(|b: &[u8]| i64::from_le_bytes(arr(b, big)) as f32),
        Elem::F16 => go!(|b: &[u8]| specular_hdf5::f16_to_f32(u16::from_le_bytes(arr(b, big)))),
        Elem::F32 => go!(|b: &[u8]| f32::from_le_bytes(arr(b, big))),
        Elem::F64 => go!(|b: &[u8]| f64::from_le_bytes(arr(b, big)) as f32),
        Elem::C64 => go!(|b: &[u8]| {
            let re = f32::from_le_bytes(arr(b, big));
            let im = f32::from_le_bytes(arr(&b[4..], big));
            part.of(f64::from(re), f64::from(im))
        }),
        Elem::C128 => go!(|b: &[u8]| {
            let re = f64::from_le_bytes(arr(b, big));
            let im = f64::from_le_bytes(arr(&b[8..], big));
            part.of(re, im)
        }),
    }
}

/// Reads a whole plane of an array: [`PlaneSpec::check`], then [`gather_rows`]. `out` is
/// resized to width × height.
pub fn read_plane(
    bytes: &[u8],
    dtype: Dtype,
    shape: &[u64],
    strides: &[u64],
    spec: &PlaneSpec,
    part: Part,
    out: &mut Vec<f32>,
) -> Result<bool> {
    let (w, h) = spec.check(shape)?;
    alloc(out, w * h)?;
    let base = spec.base(strides);
    let sx = strides.get(spec.x_dim).copied().unwrap_or(1);
    let sy = spec
        .y_dim
        .and_then(|d| strides.get(d))
        .copied()
        .unwrap_or(0);
    Ok(gather_par(bytes, dtype, base, sx, sy, w, part, out))
}

/// Values gathered per parallel task at least.
const PAR_VALUES: usize = 1 << 16;

/// [`gather_rows`] over bands of rows in parallel, for large planes.
#[allow(clippy::too_many_arguments)]
pub fn gather_par(
    bytes: &[u8],
    dtype: Dtype,
    base: u64,
    sx: u64,
    sy: u64,
    w: usize,
    part: Part,
    out: &mut [f32],
) -> bool {
    use rayon::prelude::*;
    let h = out.len() / w.max(1);
    if out.len() < 2 * PAR_VALUES || h < 2 {
        return gather_rows(bytes, dtype, base, sx, sy, w, part, out);
    }
    let rows = PAR_VALUES.div_ceil(w.max(1)).max(1);
    out.par_chunks_mut(rows * w)
        .enumerate()
        .map(|(k, band)| {
            let b = base.saturating_add(((k * rows) as u64).saturating_mul(sy));
            gather_rows(bytes, dtype, b, sx, sy, w, part, band)
        })
        .reduce(|| false, |a, b| a | b)
}

/// Combines a split complex array's real plane (`re`, replaced) with its imaginary plane.
pub fn combine(re: &mut [f32], im: &[f32], part: Part) {
    for (r, &i) in re.iter_mut().zip(im) {
        *r = part.of(f64::from(*r), f64::from(i));
    }
}

/// Resizes `v` to `n` zeros, refusing what cannot be allocated.
pub(crate) fn alloc(v: &mut Vec<f32>, n: usize) -> Result<()> {
    v.clear();
    v.try_reserve_exact(n).map_err(|_| Error::TooLarge)?;
    v.resize(n, 0.0);
    Ok(())
}

pub(crate) fn le_u16(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}

pub(crate) fn le_u32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

pub(crate) fn le_u64(d: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(d.get(at..at + 8)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strides_c_and_f() {
        assert_eq!(strides(&[2, 3, 4], false), vec![12, 4, 1]);
        assert_eq!(strides(&[2, 3, 4], true), vec![1, 2, 6]);
        assert_eq!(strides_perm(&[2, 3, 4], &[1, 2, 0]), vec![1, 8, 2]);
    }

    #[test]
    fn gathers_c_and_f_planes() {
        // 2 × 3 int16, C order: [[0, 1, 2], [3, 4, 5]].
        let c: Vec<u8> = (0..6i16).flat_map(i16::to_le_bytes).collect();
        let shape = [2u64, 3];
        let spec = PlaneSpec {
            x_dim: 1,
            y_dim: Some(0),
            index: vec![0, 0],
        };
        let mut out = Vec::new();
        let t = read_plane(
            &c,
            Dtype::le(Elem::I16),
            &shape,
            &strides(&shape, false),
            &spec,
            Part::Real,
            &mut out,
        )
        .unwrap();
        assert!(!t);
        assert_eq!(out, [0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
        // The same values in Fortran order: 0, 3, 1, 4, 2, 5.
        let f: Vec<u8> = [0i16, 3, 1, 4, 2, 5]
            .iter()
            .flat_map(|v| v.to_be_bytes())
            .collect();
        read_plane(
            &f,
            Dtype {
                elem: Elem::I16,
                big: true,
            },
            &shape,
            &strides(&shape, true),
            &spec,
            Part::Real,
            &mut out,
        )
        .unwrap();
        assert_eq!(out, [0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
        // Truncated: the last element missing.
        let t = read_plane(
            &c[..10],
            Dtype::le(Elem::I16),
            &shape,
            &strides(&shape, false),
            &spec,
            Part::Real,
            &mut out,
        )
        .unwrap();
        assert!(t);
        assert!(out[5].is_nan());
    }

    #[test]
    fn complex_parts() {
        let b: Vec<u8> = [3.0f32, 4.0].iter().flat_map(|v| v.to_le_bytes()).collect();
        let spec = PlaneSpec {
            x_dim: 0,
            y_dim: None,
            index: vec![0],
        };
        let mut out = Vec::new();
        for (p, want) in [
            (Part::Magnitude, 5.0f32),
            (Part::Real, 3.0),
            (Part::Imaginary, 4.0),
            (Part::Phase, 4f32.atan2(3.0)),
        ] {
            read_plane(&b, Dtype::le(Elem::C64), &[1], &[1], &spec, p, &mut out).unwrap();
            assert_eq!(out, [want]);
        }
    }
}
