//! Imaging mass spectrometry: imzML 1.1.
//!
//! An imzML data set is two files with the same stem: the `.imzML` XML (an mzML document
//! whose spectra carry pixel positions and, for each binary array, its offset, length and
//! encoded size in the other file) and the `.ibd` binary (a 16-byte UUID, which the XML
//! repeats, then the arrays, little-endian). In *continuous* mode every spectrum shares
//! one m/z array; in *processed* mode each has its own.
//!
//! The XML is parsed once, keeping only where each spectrum's arrays are; the arrays are
//! read from the `.ibd` (normally memory-mapped by the caller) when a spectrum or an image
//! asks for them. Arrays may be 32 or 64-bit floats or integers; compressed arrays are
//! refused, by name.
//!
//! Images: the total ion current (the sum of each spectrum's intensities) and the ion
//! image of an m/z window (the sum of intensities whose m/z lies in the window, found by
//! binary search in each sorted m/z array), both computed over all spectra on several
//! threads.

mod parse;

use std::sync::Mutex;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("not an imzML file")]
    NotImzml,
    #[error("malformed imzML: {0}")]
    Malformed(String),
    #[error("imzML file has no spectra")]
    NoSpectra,
    #[error(
        "compressed imzML arrays are not supported (the {array} array uses {compression}); only uncompressed .ibd data can be read"
    )]
    Compressed { array: String, compression: String },
    #[error("unsupported imzML array data type: {0}")]
    UnsupportedType(String),
    #[error("the .ibd file belongs to another data set (its UUID is {ibd}, the .imzML says {xml})")]
    UuidMismatch { xml: String, ibd: String },
    #[error(
        "the .ibd file is truncated: spectrum {spectrum} needs {needed} bytes, the file has {have}"
    )]
    Truncated {
        spectrum: usize,
        needed: u64,
        have: u64,
    },
    #[error("imzML image too large: {0}")]
    TooLarge(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// How spectra share their m/z values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// One m/z array for all spectra.
    Continuous,
    /// An m/z array per spectrum.
    Processed,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Continuous => "continuous",
            Mode::Processed => "processed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Polarity {
    Positive,
    Negative,
    /// Both, in different spectra.
    Mixed,
}

/// A binary array's sample type (always little-endian in the `.ibd`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataType {
    F32,
    F64,
    I32,
    I64,
}

impl DataType {
    pub fn size(self) -> usize {
        match self {
            DataType::F32 | DataType::I32 => 4,
            DataType::F64 | DataType::I64 => 8,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            DataType::F32 => "32-bit float",
            DataType::F64 => "64-bit float",
            DataType::I32 => "32-bit integer",
            DataType::I64 => "64-bit integer",
        }
    }

    /// Value `i` of the array whose bytes are `b`.
    #[inline]
    pub fn at(self, b: &[u8], i: usize) -> f64 {
        match self {
            DataType::F32 => {
                let o = i * 4;
                f64::from(f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]))
            }
            DataType::I32 => {
                let o = i * 4;
                f64::from(i32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]))
            }
            DataType::F64 => {
                let o = i * 8;
                f64::from_le_bytes(b[o..o + 8].try_into().unwrap_or([0; 8]))
            }
            DataType::I64 => {
                let o = i * 8;
                i64::from_le_bytes(b[o..o + 8].try_into().unwrap_or([0; 8])) as f64
            }
        }
    }

    /// The sum of all values of `b`.
    pub fn sum(self, b: &[u8]) -> f64 {
        match self {
            DataType::F32 => b
                .chunks_exact(4)
                .map(|c| f64::from(f32::from_le_bytes([c[0], c[1], c[2], c[3]])))
                .sum(),
            DataType::I32 => b
                .chunks_exact(4)
                .map(|c| f64::from(i32::from_le_bytes([c[0], c[1], c[2], c[3]])))
                .sum(),
            DataType::F64 => b
                .chunks_exact(8)
                .map(|c| f64::from_le_bytes(c.try_into().unwrap_or([0; 8])))
                .sum(),
            DataType::I64 => b
                .chunks_exact(8)
                .map(|c| i64::from_le_bytes(c.try_into().unwrap_or([0; 8])) as f64)
                .sum(),
        }
    }
}

/// Where a binary array is in the `.ibd`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Array {
    pub offset: u64,
    /// Number of values.
    pub len: u64,
    pub dtype: DataType,
}

impl Array {
    pub fn bytes(&self) -> u64 {
        self.len.saturating_mul(self.dtype.size() as u64)
    }

    fn end(&self) -> Option<u64> {
        self.offset
            .checked_add(self.len.checked_mul(self.dtype.size() as u64)?)
    }

    /// The array's bytes (empty if out of `ibd`, which [`ImzMl::check_ibd`] rules out).
    fn slice<'a>(&self, ibd: &'a [u8]) -> &'a [u8] {
        match self.end() {
            Some(e) if e <= ibd.len() as u64 => &ibd[self.offset as usize..e as usize],
            _ => &[],
        }
    }
}

/// One spectrum: its pixel (1-based, as the file gives it) and its arrays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub x: u32,
    pub y: u32,
    pub z: u32,
    pub mz: Array,
    pub intensity: Array,
}

/// A spectrum's values.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Spectrum {
    pub mz: Vec<f64>,
    pub intensity: Vec<f32>,
}

/// An m/z tolerance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tolerance {
    /// Parts per million of the centre.
    Ppm(f64),
    /// Daltons (absolute m/z units).
    Da(f64),
}

/// An m/z window: `mz ± tolerance`, both ends included.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Window {
    pub mz: f64,
    pub tolerance: Tolerance,
}

impl Window {
    /// The half-width in m/z units.
    pub fn half_width(&self) -> f64 {
        match self.tolerance {
            Tolerance::Ppm(p) => self.mz.abs() * p.abs() * 1e-6,
            Tolerance::Da(d) => d.abs(),
        }
    }

    /// The inclusive bounds.
    pub fn bounds(&self) -> (f64, f64) {
        let h = self.half_width();
        (self.mz - h, self.mz + h)
    }
}

/// The index of the first value of the sorted array `b` (of `dtype`, `n` values) that is not
/// below `v` (`n` if none).
pub fn lower_bound(b: &[u8], dtype: DataType, n: usize, v: f64) -> usize {
    let (mut lo, mut hi) = (0, n);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if dtype.at(b, mid) < v {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// The index of the first value of the sorted array `b` that is above `v` (`n` if none).
pub fn upper_bound(b: &[u8], dtype: DataType, n: usize, v: f64) -> usize {
    let (mut lo, mut hi) = (0, n);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if dtype.at(b, mid) <= v {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// The index range of the m/z array `mz` (bytes of `ibd`) inside `window`'s bounds.
pub fn window_range(ibd: &[u8], mz: &Array, (lo, hi): (f64, f64)) -> std::ops::Range<usize> {
    let b = mz.slice(ibd);
    let n = (b.len() / mz.dtype.size()).min(mz.len as usize);
    let a = lower_bound(b, mz.dtype, n, lo);
    let e = upper_bound(b, mz.dtype, n, hi).max(a);
    a..e
}

/// A parsed imzML document: the spectra's places and the image they tile.
#[derive(Debug)]
pub struct ImzMl {
    pub mode: Mode,
    /// Whether the document named its mode (otherwise it was inferred from the arrays).
    pub mode_declared: bool,
    pub uuid: Option<[u8; 16]>,
    /// The UUID as the document writes it.
    pub uuid_text: Option<String>,
    pub entries: Vec<Entry>,
    /// The smallest x and y (image column 0 and row 0).
    pub origin: (u32, u32),
    pub width: u32,
    pub height: u32,
    /// The distinct z positions, ascending.
    pub zs: Vec<u32>,
    /// Pixel size in µm (x, y), when given (y = x when only x is).
    pub pixel_size: Option<(f64, f64)>,
    /// The scan settings' "max count of pixels" (x, y).
    pub max_count: (Option<u32>, Option<u32>),
    pub instrument: Option<String>,
    pub software: Option<String>,
    pub polarity: Option<Polarity>,
    /// `Some(true)` when the spectra are centroided (peak lists), `Some(false)` profile.
    pub centroid: Option<bool>,
    /// The declared lowest and highest observed m/z, when every spectrum says them.
    pub declared_range: Option<(f64, f64)>,
    /// Every spectrum's m/z array is the same one.
    pub shared_mz: bool,
    /// Grid cell (`(zi·height + row)·width + column`) → entry, or `u32::MAX` for none.
    grid: Vec<u32>,
}

/// Offsets written as signed 32-bit numbers (negative past 2 GiB): each switch from a
/// non-negative to a negative offset, in file order, adds another 2³², per array kind.
fn unwrap_offsets(entries: &mut [Entry]) {
    let negative = |o: u64| (o as i64) < 0;
    if !entries
        .iter()
        .any(|e| negative(e.mz.offset) || negative(e.intensity.offset))
    {
        return;
    }
    for pick in [
        (|e: &mut Entry| &mut e.mz.offset) as fn(&mut Entry) -> &mut u64,
        |e: &mut Entry| &mut e.intensity.offset,
    ] {
        let (mut delta, mut prev_negative) = (0u64, true);
        for e in entries.iter_mut() {
            let o = pick(e);
            let raw = *o as i64;
            if raw < 0 && !prev_negative {
                delta = delta.wrapping_add(1 << 32);
            }
            prev_negative = raw < 0;
            // A huge offset wraps here and is then refused by `check_ibd`.
            *o = raw.wrapping_add(delta as i64) as u64;
        }
    }
}

/// No spectrum at a grid cell.
const NONE: u32 = u32::MAX;

impl ImzMl {
    /// Parses the `.imzML` document `xml`.
    pub fn parse(xml: &[u8]) -> Result<ImzMl> {
        let mut p = parse::parse(xml)?;
        let polarity = p.polarity();
        let mut entries = std::mem::take(&mut p.entries);
        unwrap_offsets(&mut entries);
        if entries.len() >= NONE as usize {
            return Err(Error::TooLarge(format!("{} spectra", entries.len())));
        }
        let first_mz = entries[0].mz;
        let shared_mz = entries.iter().all(|e| e.mz == first_mz);
        let (mode, mode_declared) = match p.mode {
            Some(m) => (m, true),
            None if shared_mz => (Mode::Continuous, false),
            None => (Mode::Processed, false),
        };
        let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
        let mut zs: Vec<u32> = Vec::with_capacity(entries.len());
        for e in &entries {
            x0 = x0.min(e.x);
            y0 = y0.min(e.y);
            x1 = x1.max(e.x);
            y1 = y1.max(e.y);
            zs.push(e.z);
        }
        zs.sort_unstable();
        zs.dedup();
        let (width, height) = (x1 - x0 + 1, y1 - y0 + 1);
        let cells = u64::from(width) * u64::from(height) * zs.len() as u64;
        // The grid holds an index per pixel: a few spectra at far-apart positions must not
        // ask for gigabytes (a sparse grid of 1 in 64 pixels is plenty).
        let sparse = cells > (entries.len() as u64 * 64).max(1 << 24);
        if cells > 1 << 31 || sparse {
            return Err(Error::TooLarge(format!(
                "{width} × {height} pixels × {} planes",
                zs.len()
            )));
        }
        let mut grid = vec![NONE; cells as usize];
        for (i, e) in entries.iter().enumerate() {
            let zi = zs.binary_search(&e.z).unwrap_or(0);
            let c =
                (zi * height as usize + (e.y - y0) as usize) * width as usize + (e.x - x0) as usize;
            grid[c] = i as u32;
        }
        let pixel_size = match p.pixel_size {
            (Some(x), Some(y)) => Some((x, y)),
            (Some(x), None) => Some((x, x)),
            (None, Some(y)) => Some((y, y)),
            (None, None) => None,
        }
        .filter(|&(x, y)| x > 0.0 && y > 0.0 && x.is_finite() && y.is_finite());
        Ok(ImzMl {
            mode,
            mode_declared,
            uuid: p.uuid,
            uuid_text: p.uuid_text,
            origin: (x0, y0),
            width,
            height,
            zs,
            pixel_size,
            max_count: p.max_count,
            instrument: p.instrument,
            software: p.software,
            polarity,
            centroid: p.centroid.or(p.file_centroid),
            declared_range: p.lowest.zip(p.highest),
            shared_mz,
            entries,
            grid,
        })
    }

    /// Checks that `ibd` is this document's binary: the UUID it starts with (when the
    /// document gives one) and that every array lies inside it.
    pub fn check_ibd(&self, ibd: &[u8]) -> Result<()> {
        if let Some(u) = self.uuid {
            let have: Option<[u8; 16]> = ibd.get(..16).and_then(|b| b.try_into().ok());
            if have != Some(u) {
                let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02X}")).collect::<String>();
                return Err(Error::UuidMismatch {
                    xml: hex(&u),
                    ibd: have.map_or_else(|| "missing".into(), |h| hex(&h)),
                });
            }
        }
        let have = ibd.len() as u64;
        for (i, e) in self.entries.iter().enumerate() {
            for a in [e.mz, e.intensity] {
                match a.end() {
                    Some(n) if n <= have => {}
                    n => {
                        return Err(Error::Truncated {
                            spectrum: i,
                            needed: n.unwrap_or(u64::MAX),
                            have,
                        });
                    }
                }
            }
        }
        Ok(())
    }

    /// The image's planes (distinct z positions).
    pub fn depth(&self) -> usize {
        self.zs.len()
    }

    /// The spectrum at image column `x`, row `y` of plane `zi`.
    pub fn index_at(&self, x: u32, y: u32, zi: usize) -> Option<usize> {
        if x >= self.width || y >= self.height || zi >= self.zs.len() {
            return None;
        }
        let c = (zi * self.height as usize + y as usize) * self.width as usize + x as usize;
        self.grid
            .get(c)
            .copied()
            .filter(|&i| i != NONE)
            .map(|i| i as usize)
    }

    /// Spectrum `i`'s values.
    pub fn spectrum(&self, ibd: &[u8], i: usize) -> Spectrum {
        let Some(e) = self.entries.get(i) else {
            return Spectrum::default();
        };
        let n = e.mz.len as usize;
        let (mb, ib) = (e.mz.slice(ibd), e.intensity.slice(ibd));
        if mb.is_empty() && n > 0 || ib.is_empty() && n > 0 {
            return Spectrum::default();
        }
        Spectrum {
            mz: (0..n).map(|k| e.mz.dtype.at(mb, k)).collect(),
            intensity: (0..n).map(|k| e.intensity.dtype.at(ib, k) as f32).collect(),
        }
    }

    /// The m/z range of all spectra: the declared one, else the first and last value of
    /// every m/z array (which are sorted).
    pub fn mz_range(&self, ibd: &[u8]) -> Option<(f64, f64)> {
        if let Some(r) = self.declared_range {
            return Some(r);
        }
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        let one = |a: &Array, lo: &mut f64, hi: &mut f64| {
            let b = a.slice(ibd);
            if a.len > 0 && !b.is_empty() {
                *lo = lo.min(a.dtype.at(b, 0));
                *hi = hi.max(a.dtype.at(b, a.len as usize - 1));
            }
        };
        if self.shared_mz {
            one(&self.entries[0].mz, &mut lo, &mut hi);
        } else {
            for e in &self.entries {
                one(&e.mz, &mut lo, &mut hi);
            }
        }
        (lo <= hi).then_some((lo, hi))
    }

    /// `f(entry)` for every spectrum, on several threads; `None` when `cancel` said so.
    pub fn per_spectrum(
        &self,
        f: impl Fn(&Entry) -> f32 + Sync,
        cancel: &(dyn Fn() -> bool + Sync),
    ) -> Option<Vec<f32>> {
        const CHUNK: usize = 512;
        let n = self.entries.len();
        let mut out = vec![f32::NAN; n];
        let threads = if n > 2 * CHUNK {
            std::thread::available_parallelism()
                .map_or(1, |n| n.get())
                .min(16)
                .min(n.div_ceil(CHUNK))
        } else {
            1
        };
        let work = |(ci, c): (usize, &mut [f32])| {
            for (k, v) in c.iter_mut().enumerate() {
                *v = f(&self.entries[ci * CHUNK + k]);
            }
        };
        if threads <= 1 {
            for item in out.chunks_mut(CHUNK).enumerate() {
                if cancel() {
                    return None;
                }
                work(item);
            }
            return Some(out);
        }
        let queue = Mutex::new(out.chunks_mut(CHUNK).enumerate());
        let stopped = std::sync::atomic::AtomicBool::new(false);
        std::thread::scope(|s| {
            for wi in 0..threads {
                let (queue, stopped, work) = (&queue, &stopped, &work);
                specular_thread::spawn_scoped(s, format!("dec-imzml-{wi}"), move || {
                    loop {
                        if stopped.load(std::sync::atomic::Ordering::Relaxed) {
                            return;
                        }
                        if cancel() {
                            stopped.store(true, std::sync::atomic::Ordering::Relaxed);
                            return;
                        }
                        let Some(item) = queue.lock().ok().and_then(|mut q| q.next()) else {
                            return;
                        };
                        work(item);
                    }
                });
            }
        });
        if stopped.into_inner() {
            return None;
        }
        Some(out)
    }

    /// Per-spectrum values laid out as plane `zi` (row 0 = the smallest y); pixels without a
    /// spectrum are NaN.
    pub fn image(&self, per_spectrum: &[f32], zi: usize) -> Vec<f32> {
        let cells = self.width as usize * self.height as usize;
        let start = zi * cells;
        self.grid
            .get(start..start + cells)
            .unwrap_or(&[])
            .iter()
            .map(|&i| {
                if i == NONE {
                    f32::NAN
                } else {
                    per_spectrum.get(i as usize).copied().unwrap_or(f32::NAN)
                }
            })
            .collect()
    }

    /// Each spectrum's total ion current (the sum of its intensities).
    pub fn tic(&self, ibd: &[u8], cancel: &(dyn Fn() -> bool + Sync)) -> Option<Vec<f32>> {
        self.per_spectrum(
            |e| e.intensity.dtype.sum(e.intensity.slice(ibd)) as f32,
            cancel,
        )
    }

    /// Each spectrum's summed intensity in `window`.
    pub fn ion(
        &self,
        ibd: &[u8],
        window: Window,
        cancel: &(dyn Fn() -> bool + Sync),
    ) -> Option<Vec<f32>> {
        let bounds = window.bounds();
        let shared = self
            .shared_mz
            .then(|| window_range(ibd, &self.entries[0].mz, bounds));
        self.per_spectrum(
            |e| {
                let r = shared
                    .clone()
                    .unwrap_or_else(|| window_range(ibd, &e.mz, bounds));
                if r.is_empty() {
                    return 0.0;
                }
                let s = e.intensity.dtype.size();
                let b = e.intensity.slice(ibd);
                b.get(r.start * s..r.end * s)
                    .map_or(f32::NAN, |b| e.intensity.dtype.sum(b) as f32)
            },
            cancel,
        )
    }
}

#[cfg(test)]
mod tests;
