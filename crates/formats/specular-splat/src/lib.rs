//! 3D Gaussian splats: own readers for antimatter15's `.splat` (32-byte records), 3DGS PLY
//! (`f_dc_*`, `f_rest_*`, `opacity`, `scale_*`, `rot_*`), SuperSplat's compressed PLY (chunked
//! 11-10-11 positions, smallest-three rotations) and Niantic's `.spz` (gzip v1–3, Zstandard
//! v4), all into one [`Splats`]: positions and 3D covariances in the viewer's z-up frame, a
//! base colour and opacity per splat and, when the file has them, the higher spherical
//! harmonics (degree 1–3) as half floats for view-dependent colour.
//!
//! **Which PLY is a splat.** A PLY whose `vertex` element has the 3DGS properties (a DC colour
//! `f_dc_0`, `opacity`, `scale_0` and `rot_0`), or a compressed PLY (a `chunk` element and
//! `packed_position`), is splats ([`ply_is_splats`]); any other PLY is a mesh or a point cloud.
//!
//! **Frames.** 3DGS PLY, `.splat` and compressed PLY are in the COLMAP camera frame (x right,
//! y down, z forward): up is −y. `.spz` is x right, y up, z back: up is +y. Both are turned to
//! z up at load ([`Splats::to_file`] maps a view direction back for the harmonics).
//!
//! **Safety of input.** Counts are checked against the bytes present before anything is
//! allocated, every size is capped by [`Limits`], non-finite values drop their splat, and no
//! input panics. [`preview`] renders splats on the CPU for thumbnails.

mod half;
mod math;
mod ply;
pub mod preview;
mod splat;
mod spz;

#[doc(hidden)]
pub mod fixtures;

#[cfg(test)]
mod tests;

pub use half::{f16_to_f32, f32_to_f16};
pub use math::{SH_C0, covariance, sh_basis};
pub use specular_mesh::Bounds;

/// What went wrong reading splats.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("malformed: {0}")]
    Malformed(String),
    #[error("not supported: {0}")]
    Unsupported(String),
    #[error("too large: {0}")]
    TooLarge(String),
    #[error("no splats")]
    Empty,
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn bad<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Malformed(msg.into()))
}

/// The file formats read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SplatFormat {
    /// antimatter15's `.splat`: 32-byte records, no harmonics.
    Splat,
    /// A 3DGS PLY (the reference trainer's output).
    Ply,
    /// SuperSplat's compressed PLY.
    CompressedPly,
    /// Niantic's `.spz`.
    Spz,
}

impl SplatFormat {
    pub fn name(self) -> &'static str {
        match self {
            SplatFormat::Splat => "SPLAT",
            SplatFormat::Ply => "3DGS PLY",
            SplatFormat::CompressedPly => "Compressed PLY",
            SplatFormat::Spz => "SPZ",
        }
    }
}

/// Which way the file's up axis points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileUp {
    /// COLMAP's camera frame: y down.
    NegY,
    /// OpenGL's: y up.
    PosY,
}

impl FileUp {
    pub fn label(self) -> &'static str {
        match self {
            FileUp::NegY => "−Y",
            FileUp::PosY => "Y",
        }
    }

    /// Rows of the map from the file's frame to the viewer's (z up).
    pub fn to_canonical(self) -> [[f32; 3]; 3] {
        match self {
            // (x, y, z) → (x, z, −y)
            FileUp::NegY => [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]],
            // (x, y, z) → (x, −z, y)
            FileUp::PosY => [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]],
        }
    }
}

/// Splats read from one file, in the viewer's z-up frame.
#[derive(Clone, Debug)]
pub struct Splats {
    pub format: SplatFormat,
    /// Splats the file holds (more than [`Splats::len`] when some were dropped or the cap
    /// subsampled them).
    pub in_file: u64,
    pub positions: Vec<[f32; 3]>,
    /// The 3D covariance of each, upper triangle: xx, xy, xz, yy, yz, zz.
    pub covariances: Vec<[f32; 6]>,
    /// Base colour (the DC term, display-referred, may leave 0–1) and opacity (0–1).
    pub colors: Vec<[f32; 4]>,
    /// Degree of the higher harmonics kept (0–3).
    pub sh_degree: u8,
    /// Higher harmonics as half floats: per splat `sh_coeffs() * 3` values, coefficient-major
    /// with r, g, b interleaved, in the file's frame.
    pub sh: Vec<u16>,
    /// The file's up axis.
    pub up: FileUp,
    /// Rows of the map from the viewer's frame back to the file's (the transpose of
    /// [`FileUp::to_canonical`]), for evaluating the harmonics.
    pub to_file: [[f32; 3]; 3],
    /// The box holding all but the outermost half percent of splats on each axis (the
    /// floaters of a capture would otherwise dwarf the scene).
    pub bounds: Bounds,
    /// The box of every splat centre.
    pub full_bounds: Bounds,
    /// Trained with anti-aliasing (`.spz` flag): opacity is compensated for the screen filter.
    pub antialiased: bool,
    /// What was dropped or approximated, for the info panel.
    pub notes: Vec<String>,
}

impl Splats {
    pub fn len(&self) -> usize {
        self.positions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Higher-harmonic coefficients per colour channel: 0, 3, 8 or 15.
    pub fn sh_coeffs(&self) -> usize {
        sh_coeffs(self.sh_degree)
    }

    /// Bytes held.
    pub fn memory_bytes(&self) -> usize {
        self.positions.len() * (12 + 24 + 16) + self.sh.len() * 2
    }

    /// The colour of splat `i` seen along `dir` (unit, viewer's frame, from the eye towards
    /// the splat): base colour plus the harmonics, clamped at 0.
    pub fn color_towards(&self, i: usize, dir: [f32; 3]) -> [f32; 3] {
        let c = self.colors[i];
        let k = self.sh_coeffs();
        if k == 0 {
            return [c[0].max(0.0), c[1].max(0.0), c[2].max(0.0)];
        }
        let d = math::mul(&self.to_file, dir);
        let basis = sh_basis(d);
        let sh = &self.sh[i * k * 3..(i + 1) * k * 3];
        let mut out = [c[0], c[1], c[2]];
        for (j, b) in basis.iter().enumerate().take(k) {
            for (ch, o) in out.iter_mut().enumerate() {
                *o += b * f16_to_f32(sh[j * 3 + ch]);
            }
        }
        out.map(|v| v.max(0.0))
    }
}

/// Higher-harmonic coefficients per channel for `degree`.
pub fn sh_coeffs(degree: u8) -> usize {
    match degree {
        0 => 0,
        1 => 3,
        2 => 8,
        _ => 15,
    }
}

/// Caps on what a file may make us allocate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Splats kept at most; files with more are evenly subsampled.
    pub max_splats: usize,
    /// Splats a file may declare at all.
    pub max_declared: u64,
    /// Bytes a compressed `.spz` may unpack to.
    pub max_unpacked: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_splats: 12_000_000,
            max_declared: 1 << 31,
            max_unpacked: 4 << 30,
        }
    }
}

/// Whether the first bytes are a splat format's signature: a gzip stream or "NGSP" (`.spz`
/// is the only splat format with one; `.splat` has none, PLY is told apart by
/// [`ply_is_splats`]).
pub fn sniff_spz(head: &[u8]) -> bool {
    head.starts_with(b"NGSP") || head.starts_with(&[0x1f, 0x8b])
}

/// Whether a PLY file holds Gaussian splats (by its property names).
pub fn ply_is_splats(bytes: &[u8]) -> bool {
    ply::is_splats(bytes)
}

/// Reads a `.splat` file.
pub fn read_splat(bytes: &[u8], limits: &Limits) -> Result<Splats> {
    finish(splat::read(bytes, limits)?)
}

/// Reads a 3DGS or compressed PLY.
pub fn read_ply(bytes: &[u8], limits: &Limits) -> Result<Splats> {
    finish(ply::read(bytes, limits)?)
}

/// Reads a `.spz` file.
pub fn read_spz(bytes: &[u8], limits: &Limits) -> Result<Splats> {
    finish(spz::read(bytes, limits)?)
}

/// What a reader collects, in the file's frame.
pub(crate) struct Raw {
    pub format: SplatFormat,
    pub in_file: u64,
    pub up: FileUp,
    pub positions: Vec<[f32; 3]>,
    /// Linear scales.
    pub scales: Vec<[f32; 3]>,
    /// Unit quaternions w, x, y, z.
    pub rotations: Vec<[f32; 4]>,
    pub colors: Vec<[f32; 4]>,
    pub sh_degree: u8,
    pub sh: Vec<u16>,
    pub antialiased: bool,
    pub notes: Vec<String>,
}

impl Raw {
    pub(crate) fn new(format: SplatFormat, in_file: u64, up: FileUp, n: usize, k: usize) -> Raw {
        Raw {
            format,
            in_file,
            up,
            positions: Vec::with_capacity(n),
            scales: Vec::with_capacity(n),
            rotations: Vec::with_capacity(n),
            colors: Vec::with_capacity(n),
            sh_degree: 0,
            sh: Vec::with_capacity(n * k * 3),
            antialiased: false,
            notes: Vec::new(),
        }
    }
}

/// The indices of `n` splats to keep under `max`: all, or an even stride.
pub(crate) fn keep(n: u64, max: usize) -> (usize, u64) {
    if n <= max as u64 {
        (n as usize, 1)
    } else {
        let stride = n.div_ceil(max as u64);
        (n.div_ceil(stride) as usize, stride)
    }
}

/// Turns what a reader collected into the viewer's frame: drops non-finite splats, computes
/// covariances, the bounds.
fn finish(raw: Raw) -> Result<Splats> {
    use rayon::prelude::*;
    let m = raw.up.to_canonical();
    let k = sh_coeffs(raw.sh_degree) * 3;
    let n = raw.positions.len();
    let ok: Vec<bool> = (0..n)
        .into_par_iter()
        .map(|i| {
            raw.positions[i].iter().all(|v| v.is_finite())
                && raw.scales[i].iter().all(|v| v.is_finite())
                && raw.rotations[i].iter().all(|v| v.is_finite())
                && raw.colors[i].iter().all(|v| v.is_finite())
        })
        .collect();
    let dropped = ok.iter().filter(|o| !**o).count();
    let mut notes = raw.notes;
    if dropped > 0 {
        notes.push(format!("{dropped} splats with non-finite values left out"));
    }
    let idx: Vec<usize> = (0..n).filter(|&i| ok[i]).collect();
    if idx.is_empty() {
        return Err(Error::Empty);
    }
    let positions: Vec<[f32; 3]> = idx
        .par_iter()
        .map(|&i| math::mul(&m, raw.positions[i]))
        .collect();
    let covariances: Vec<[f32; 6]> = idx
        .par_iter()
        .map(|&i| math::rotate_cov(&m, covariance(raw.scales[i], raw.rotations[i])))
        .collect();
    let colors: Vec<[f32; 4]> = idx.iter().map(|&i| raw.colors[i]).collect();
    let sh = if k == 0 || dropped == 0 {
        raw.sh
    } else {
        idx.iter()
            .flat_map(|&i| raw.sh[i * k..(i + 1) * k].iter().copied())
            .collect()
    };
    let mut full = Bounds::EMPTY;
    for p in &positions {
        full.add(*p);
    }
    let bounds = robust_bounds(&positions).unwrap_or(full);
    Ok(Splats {
        format: raw.format,
        in_file: raw.in_file,
        positions,
        covariances,
        colors,
        sh_degree: raw.sh_degree,
        sh,
        up: raw.up,
        to_file: math::transpose(&m),
        bounds,
        full_bounds: full,
        antialiased: raw.antialiased,
        notes,
    })
}

/// The box of the 0.5th to 99.5th percentile on each axis, from at most 200k samples.
fn robust_bounds(p: &[[f32; 3]]) -> Option<Bounds> {
    if p.len() < 20 {
        return None;
    }
    let step = p.len().div_ceil(200_000);
    let mut b = Bounds::EMPTY;
    for axis in 0..3 {
        let mut v: Vec<f32> = p.iter().step_by(step).map(|q| q[axis]).collect();
        let lo = v.len() / 200;
        let hi = v.len() - 1 - lo;
        let a = *v.select_nth_unstable_by(lo, f32::total_cmp).1;
        let z = *v.select_nth_unstable_by(hi, f32::total_cmp).1;
        b.min[axis] = a;
        b.max[axis] = z;
    }
    (!b.is_empty()).then_some(b)
}
