//! `specular-radiance`: decoders for Radiance RGBE (`.hdr`, `.pic`) and Portable Float Map (`.pfm`).
//!
//! * **Radiance RGBE.** `#?RADIANCE`/`#?RGBE` headers with `FORMAT` (`32-bit_rle_rgbe` or
//!   `32-bit_rle_xyze`), `EXPOSURE` (multiplied over every occurrence), `PRIMARIES`, `COLORCORR`
//!   and `PIXASPECT`; all eight resolution-string orientations (`-Y N +X M`, `+Y N +X M`,
//!   `+X M -Y N`, ...); new-style (per-component) RLE, old-style RLE (`1 1 1 n` repeat pixels) and
//!   flat scanlines, freely mixed per scanline. Scanline starts are found in a cheap sequential
//!   skip pass (only run headers are touched), then scanlines are decoded and converted to `f32`
//!   in parallel. Pixel values are returned as stored (`mantissa * 2^(exp - 136)`, as in Greg
//!   Ward's `rgbe.c` and stb_image); `EXPOSURE` and the primaries are only reported in [`Info`].
//!   XYZE files are returned as XYZ ([`ColorSpace::Xyz`]). [`RowIndex`] keeps the scanline
//!   starts so row-major files can be read a window of rows and columns at a time (streamed
//!   levels and regions; PFM needs no index).
//! * **PFM.** `PF` (RGB), `Pf` (grey) and the common `PF4` (RGBA) extension, both endians
//!   (negative scale = little-endian), bottom-up rows flipped to top-down, parallel conversion.
//! * **Output.** Interleaved `f32` RGB or RGBA, top-down, left-to-right ([`OutputChannels`]).
//!   Grey PFM is replicated to RGB; missing alpha is `1.0`.
//! * **Robustness.** Malformed input never panics. Every dimension is checked against
//!   [`Limits`] before anything is allocated, and the output allocation is fallible.
//!   Truncated or corrupt pixel data does not fail the decode: undecodable scanlines are left
//!   at zero and [`Image::truncated`] is set.

mod hdr;
mod pfm;

use rayon::prelude::*;

/// Errors from [`read_info`] and [`decode`].
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("not a Radiance HDR or PFM file")]
    NotRecognized,
    #[error("invalid header: {0}")]
    InvalidHeader(&'static str),
    #[error("unsupported: {0}")]
    Unsupported(&'static str),
    #[error("image of {width}x{height} exceeds the configured limits")]
    TooLarge { width: u64, height: u64 },
    #[error("out of memory allocating {0} bytes")]
    OutOfMemory(u64),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Container format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// Radiance RGBE / XYZE (`.hdr`, `.pic`).
    Radiance,
    /// Portable Float Map.
    Pfm,
}

/// Colour space of the decoded values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorSpace {
    /// RGB; for Radiance files with the primaries in [`Info::primaries`] (Radiance's default
    /// primaries if absent), for PFM unspecified (usually linear sRGB/Rec.709).
    Rgb,
    /// CIE XYZ (`FORMAT=32-bit_rle_xyze`).
    Xyz,
}

/// How pixels are stored on disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    /// Radiance RGBE/XYZE scanlines (new RLE, old RLE or flat; may vary per scanline).
    Rgbe,
    /// Little-endian `f32` samples (PFM).
    F32Le,
    /// Big-endian `f32` samples (PFM).
    F32Be,
}

/// Radiance scan order, i.e. the resolution string. `x_major` files store columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Orientation {
    /// The first (slow) axis is X: each stored scanline is a column of the image.
    pub x_major: bool,
    /// Rows are stored top to bottom (`-Y`), as in the standard `-Y N +X M`.
    pub top_down: bool,
    /// Columns are stored left to right (`+X`).
    pub left_to_right: bool,
}

impl Orientation {
    /// The standard orientation `-Y N +X M` (also PFM after flipping).
    pub const STANDARD: Orientation = Orientation {
        x_major: false,
        top_down: true,
        left_to_right: true,
    };
}

/// Header information (no pixel data is decoded).
#[derive(Clone, Debug, PartialEq)]
pub struct Info {
    pub format: Format,
    /// Width of the decoded image (after applying the orientation).
    pub width: u32,
    /// Height of the decoded image.
    pub height: u32,
    /// Channels stored in the file: 1 (grey PFM), 3 (RGB/XYZ) or 4 (`PF4`).
    pub channels: u8,
    pub encoding: Encoding,
    pub color_space: ColorSpace,
    /// Radiance `EXPOSURE`: product of all values in the header (1.0 if absent). Physical
    /// radiance is `value / exposure`.
    pub exposure: f32,
    /// Radiance `COLORCORR` (product of all values; `[1, 1, 1]` if absent).
    pub color_correction: [f32; 3],
    /// Radiance `PRIMARIES`: `[rx, ry, gx, gy, bx, by, wx, wy]`.
    pub primaries: Option<[f32; 8]>,
    /// Radiance `PIXASPECT` (product; 1.0 if absent).
    pub pixel_aspect: f32,
    /// Radiance scan order (`Orientation::STANDARD` for PFM).
    pub orientation: Orientation,
    /// PFM scale field (its absolute value; 1.0 for Radiance).
    pub scale: f32,
    /// Offset of the pixel data in the file.
    pub data_offset: usize,
}

/// Size limits checked before any allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_width: u32,
    pub max_height: u32,
    pub max_pixels: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_width: 1 << 20,
            max_height: 1 << 20,
            max_pixels: 1 << 28,
        }
    }
}

/// Output channel layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OutputChannels {
    /// 3 samples per pixel.
    Rgb,
    /// 4 samples per pixel, alpha 1.0 unless the file has alpha (`PF4`).
    #[default]
    Rgba,
}

impl OutputChannels {
    pub fn count(self) -> usize {
        match self {
            OutputChannels::Rgb => 3,
            OutputChannels::Rgba => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecodeOptions {
    pub channels: OutputChannels,
    pub limits: Limits,
    /// Decode on the rayon pool (`false`: on the calling thread only).
    pub parallel: bool,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        DecodeOptions {
            channels: OutputChannels::Rgba,
            limits: Limits::default(),
            parallel: true,
        }
    }
}

/// A decoded image: `width * height * channels.count()` interleaved `f32`, top-down.
#[derive(Clone, Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub channels: OutputChannels,
    pub data: Vec<f32>,
    pub info: Info,
    /// Some pixel data was missing or corrupt; those pixels are zero.
    pub truncated: bool,
}

/// Is this a Radiance or PFM file (by its magic bytes)?
pub fn probe(data: &[u8]) -> bool {
    detect(data).is_some()
}

/// Identify the format from the first bytes.
pub fn detect(data: &[u8]) -> Option<Format> {
    if hdr::probe(data) {
        Some(Format::Radiance)
    } else if pfm::probe(data) {
        Some(Format::Pfm)
    } else {
        None
    }
}

/// Parse the header only.
pub fn read_info(data: &[u8]) -> Result<Info> {
    match detect(data) {
        Some(Format::Radiance) => hdr::read_info(data),
        Some(Format::Pfm) => pfm::read_info(data),
        None => Err(Error::NotRecognized),
    }
}

/// Decode the whole image.
pub fn decode(data: &[u8], opts: &DecodeOptions) -> Result<Image> {
    let info = read_info(data)?;
    check_limits(&info, &opts.limits)?;
    let nch = opts.channels.count();
    let len = info.width as u64 * info.height as u64 * nch as u64;
    let mut out = alloc_f32(len)?;
    let truncated = match info.format {
        Format::Radiance => hdr::decode_into(data, &info, nch, opts.parallel, &mut out),
        Format::Pfm => pfm::decode_into(data, &info, nch, opts.parallel, &mut out),
    };
    Ok(Image {
        width: info.width,
        height: info.height,
        channels: opts.channels,
        data: out,
        info,
        truncated,
    })
}

/// Random access to the rows of a file, for decoding a region or streaming a reduced image
/// without holding the whole `f32` image: PFM rows are read in place; Radiance scanlines are
/// located once by [`RowIndex::new`] (a skip pass that touches only run headers) and decoded
/// one at a time. Files stored column by column (x-major Radiance) have no row access.
#[derive(Clone, Debug)]
pub struct RowIndex {
    /// Radiance: where each stored scanline starts. Empty for PFM.
    starts: Vec<usize>,
    truncated: bool,
}

impl RowIndex {
    /// Indexes the rows of `data`, whose header is `info` ([`read_info`]). `None` for x-major
    /// Radiance files. Checks `limits` first.
    pub fn new(data: &[u8], info: &Info, limits: &Limits) -> Result<Option<RowIndex>> {
        check_limits(info, limits)?;
        Ok(match info.format {
            Format::Radiance if info.orientation.x_major => None,
            Format::Radiance => {
                let (starts, truncated) = hdr::index(data, info);
                Some(RowIndex { starts, truncated })
            }
            Format::Pfm => {
                let row = info.width as u64 * info.channels as u64 * 4;
                let have = data.len().saturating_sub(info.data_offset) as u64;
                Some(RowIndex {
                    starts: Vec::new(),
                    truncated: have < row * info.height as u64,
                })
            }
        })
    }

    /// Some pixel data is missing or corrupt (as [`Image::truncated`] would say).
    pub fn truncated(&self) -> bool {
        self.truncated
    }

    /// Heap bytes held by the index.
    pub fn memory_bytes(&self) -> usize {
        self.starts.capacity() * size_of::<usize>()
    }

    /// Decodes rows `rows` and columns `cols` (top-down, left to right, after the file's
    /// orientation) into `out`: `rows.len() * cols.len() * 4` interleaved RGBA `f32`, the
    /// values [`decode`] gives (as stored; grey replicated; alpha one unless the file has it;
    /// pixels past the end of a truncated file as [`decode`] leaves them). `line` is a reusable
    /// scanline buffer.
    pub fn read_rows(
        &self,
        data: &[u8],
        info: &Info,
        rows: std::ops::Range<u32>,
        cols: std::ops::Range<u32>,
        out: &mut [f32],
        line: &mut Vec<u8>,
    ) {
        let rows = rows.start.min(info.height) as usize..rows.end.min(info.height) as usize;
        let cols = cols.start.min(info.width) as usize..cols.end.min(info.width) as usize;
        let n = cols.len() * 4;
        if n == 0 {
            return;
        }
        let table = (info.format == Format::Radiance).then(hdr::exp_table);
        for (y, o) in rows.zip(out.chunks_exact_mut(n)) {
            match &table {
                Some(t) => hdr::read_row(data, info, &self.starts, y, cols.clone(), o, line, t),
                None => pfm::read_row(data, info, y, cols.clone(), o),
            }
        }
    }
}

fn check_limits(info: &Info, limits: &Limits) -> Result<()> {
    let (w, h) = (info.width as u64, info.height as u64);
    if info.width > limits.max_width || info.height > limits.max_height || w * h > limits.max_pixels
    {
        return Err(Error::TooLarge {
            width: w,
            height: h,
        });
    }
    Ok(())
}

/// Zeroed `f32` buffer, fallible.
fn alloc_f32(len: u64) -> Result<Vec<f32>> {
    let bytes = len.saturating_mul(4);
    let n = usize::try_from(len).map_err(|_| Error::OutOfMemory(bytes))?;
    // Probe the allocator first, then take zeroed memory (calloc: untouched pages stay free).
    let mut probe: Vec<f32> = Vec::new();
    probe
        .try_reserve_exact(n)
        .map_err(|_| Error::OutOfMemory(bytes))?;
    drop(probe);
    Ok(vec![0.0; n])
}

/// Run `f(row_index, row)` over the rows of `out`, in parallel if asked.
fn for_rows(
    out: &mut [f32],
    row_len: usize,
    parallel: bool,
    f: impl Fn(usize, &mut [f32]) + Sync + Send,
) {
    if row_len == 0 {
        return;
    }
    if parallel {
        out.par_chunks_mut(row_len)
            .enumerate()
            .with_min_len(8)
            .for_each(|(y, row)| f(y, row));
    } else {
        out.chunks_mut(row_len)
            .enumerate()
            .for_each(|(y, row)| f(y, row));
    }
}
