//! `specular-exr`: an OpenEXR decoder with a plain decode API.
//!
//! * **Files.** Single-part and multi-part (parts are listed by [`read_info`]; one part is
//!   decoded per call), scanline and tiled storage, one-level / mipmap / ripmap tiles with both
//!   rounding modes, any line order, data windows anywhere (negative origins included), and
//!   long attribute names. Deep scanline and deep tiled parts are decoded flattened: each
//!   pixel's samples sorted by `Z` and composited front to back with "over" using `A` (without
//!   `A`, the front-most sample).
//! * **Channels.** `R`, `G`, `B`, `A` (optionally in a layer such as `diffuse.R`), luminance
//!   `Y` (replicated to RGB), and luminance/chroma `Y`+`RY`+`BY` (subsampled chroma is
//!   upsampled linearly between its samples, then converted with the file's chromaticities).
//!   Any channel can be routed to any output slot with [`ChannelSelect::Slots`]. `HALF`,
//!   `FLOAT` and `UINT` samples; x/y-subsampled channels in scanline files.
//! * **Compression.** NONE, RLE, ZIPS, ZIP, PIZ, PXR24, B44, B44A, DWAA, DWAB (lossy channels
//!   within a few half ulps of OpenEXR's own decode; see `tests/conformance.rs`), and
//!   HTJ2K256 / HTJ2K32 (lossless HTJ2K codestreams decoded by `specular-jp2`, bit-exact with
//!   OpenEXR) for HALF, FLOAT and UINT channels.
//! * **Levels.** [`LevelSelect`] picks the full level, any listed level, or the smallest level
//!   that still covers a target size: the viewer's fast preview path (a 1/4 mip level costs about
//!   1/16 of the full decode, since only that level's chunks are touched). [`RowReader`]
//!   reads any window of rows and columns of one level a chunk band at a time, for callers that
//!   reduce or crop while streaming instead of holding the whole level.
//! * **Output.** Interleaved RGBA, `f32` or `f16` bits ([`SampleFormat`]), covering the data
//!   window of the selected level; the display window and chromaticities are reported for the
//!   caller to composite and colour-manage.
//! * **Parallelism.** Chunks (scanline blocks, or rows of tiles) are decompressed and converted
//!   in parallel on the rayon pool, straight into the output buffer; per-thread scratch only.
//! * **Robustness.** Malformed input never panics. Header values are validated before use;
//!   the output is checked against [`Limits`] (including a cap on decoded pixels per file byte,
//!   so a tiny file cannot request a huge allocation) and allocated fallibly. Missing,
//!   truncated or corrupt chunks are left at zero (alpha one) and counted in
//!   [`ExrImage::damaged_chunks`]; a broken offset table is rebuilt by walking the chunks.
//!
//! # Not supported
//! * Compression ids above 13: [`ExrError::UnsupportedCompression`].

#![feature(f16, portable_simd)]

mod codec;
mod decode;
mod deep;
mod dwa;
mod error;
mod half;
mod header;
mod ht;
mod piz;
mod zstd;

pub use error::{ExrError, Result};
pub use header::{
    Attribute, Box2i, Channel, Chromaticities, Compression, ExrInfo, Level, LevelMode,
    LevelRounding, LineOrder, Part, PixelType, Storage, TileDesc, probe,
};

use decode::{Mapping, Plan};
use half::Sample;

/// Which resolution level to decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LevelSelect {
    /// Level 0 (full resolution).
    #[default]
    Full,
    /// An index into [`Part::levels`].
    Index(usize),
    /// The smallest level that is at least this large (see [`Part::level_for_size`]).
    MinSize { width: u32, height: u32 },
}

/// Which channels go to the R, G, B, A output slots.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum ChannelSelect {
    /// The root layer if it has colour or luminance channels, else the first layer that does,
    /// else the first channel as grey.
    #[default]
    Auto,
    /// `R`/`G`/`B`/`A` (or `Y`, `RY`, `BY`) of the named layer (`""` = root).
    Layer(String),
    /// Explicit channel names for R, G, B, A. Unset colour slots are zero; unset alpha is one.
    Slots([Option<String>; 4]),
}

/// Output sample type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SampleFormat {
    #[default]
    F32,
    /// `f16`, stored as its bit pattern in `u16`.
    F16,
}

/// Size limits, checked before allocating the output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_width: u32,
    pub max_height: u32,
    pub max_pixels: u64,
    /// Maximum decoded pixels per byte of input. No supported compression comes near 1024 on
    /// real data (ZIP of a constant single-channel image is ~500), so the default only rejects
    /// files whose header promises far more than the data can hold.
    pub max_pixels_per_byte: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_width: 1 << 20,
            max_height: 1 << 20,
            max_pixels: 1 << 28,
            max_pixels_per_byte: 1024,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodeOptions {
    /// Index into [`ExrInfo::parts`].
    pub part: usize,
    pub level: LevelSelect,
    pub channels: ChannelSelect,
    pub format: SampleFormat,
    pub limits: Limits,
    /// Decode on the rayon pool (`false`: on the calling thread).
    pub parallel: bool,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        DecodeOptions {
            part: 0,
            level: LevelSelect::Full,
            channels: ChannelSelect::Auto,
            format: SampleFormat::F32,
            limits: Limits::default(),
            parallel: true,
        }
    }
}

/// Interleaved RGBA samples.
#[derive(Clone, Debug, PartialEq)]
pub enum Pixels {
    F32(Vec<f32>),
    /// `f16` bit patterns.
    F16(Vec<u16>),
}

/// How the colour slots were filled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorKind {
    /// From R/G/B channels (or explicit slots).
    Rgb,
    /// From a luminance `Y` (or single) channel, replicated to R, G and B.
    Luminance,
    /// From `Y`, `RY`, `BY`, converted to RGB.
    LuminanceChroma,
}

/// A decoded level of one part.
#[derive(Clone, Debug)]
pub struct ExrImage {
    /// Size of the decoded level.
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` samples, top row first.
    pub pixels: Pixels,
    /// The level that was decoded.
    pub level: Level,
    /// The part's data window (full-resolution coordinates). Pixel `(0, 0)` of a level is at
    /// `(data_window.x_min, data_window.y_min)` in that level's coordinates.
    pub data_window: Box2i,
    /// The part's display window (full-resolution coordinates).
    pub display_window: Box2i,
    pub pixel_aspect_ratio: f32,
    /// The part's chromaticities attribute (OpenEXR's default is [`Chromaticities::REC709`]).
    pub chromaticities: Option<Chromaticities>,
    pub white_luminance: Option<f32>,
    /// Channel names used for R, G, B, A.
    pub channels: [Option<String>; 4],
    pub color: ColorKind,
    /// An alpha channel was decoded (otherwise alpha is one).
    pub has_alpha: bool,
    pub compression: Compression,
    /// Chunks that were missing, truncated or corrupt (their pixels are zero).
    pub damaged_chunks: usize,
}

/// How [`ChannelSelect::Auto`] would build colour for `part`: `None` when the part has no
/// R, G, B or Y channel (a depth or mask part shows its first channel as grey).
pub fn auto_color(part: &Part) -> Option<ColorKind> {
    let colour = part
        .channels
        .iter()
        .any(|c| matches!(c.base_name(), "R" | "G" | "B" | "Y"));
    if !colour {
        return None;
    }
    resolve_channels(part, &ChannelSelect::Auto)
        .ok()
        .map(|r| r.2)
}

/// Parse the headers of every part (no pixel data is touched).
pub fn read_info(data: &[u8]) -> Result<ExrInfo> {
    header::read_info(data)
}

/// Decode one part/level to interleaved RGBA.
pub fn decode(data: &[u8], opts: &DecodeOptions) -> Result<ExrImage> {
    let info = read_info(data)?;
    decode_with_info(data, &info, opts)
}

/// The part and level `opts` select, after checking that they can be decoded within
/// `opts.limits`.
fn select<'a>(data: &[u8], info: &'a ExrInfo, opts: &DecodeOptions) -> Result<(&'a Part, Level)> {
    let part = info
        .parts
        .get(opts.part)
        .ok_or(ExrError::NotFound("part"))?;
    match part.storage {
        Storage::DeepTiled(None) => return Err(ExrError::InvalidHeader("tiles")),
        Storage::DeepScanline | Storage::DeepTiled(Some(_)) => {
            if !matches!(
                part.compression,
                Compression::None
                    | Compression::Rle
                    | Compression::Zips
                    | Compression::Zip
                    | Compression::Zstd
            ) {
                return Err(ExrError::UnsupportedCompression(part.compression.name()));
            }
            if part
                .channels
                .iter()
                .any(|c| c.x_sampling != 1 || c.y_sampling != 1)
            {
                return Err(ExrError::InvalidHeader(
                    "subsampled channels in a deep part",
                ));
            }
        }
        Storage::Tiled(_) => {
            if part
                .channels
                .iter()
                .any(|c| c.x_sampling != 1 || c.y_sampling != 1)
            {
                return Err(ExrError::InvalidHeader(
                    "subsampled channels in a tiled part",
                ));
            }
        }
        Storage::Scanline => {}
    }
    if !part.compression.is_supported() {
        return Err(ExrError::UnsupportedCompression(part.compression.name()));
    }
    let li = match opts.level {
        LevelSelect::Full => 0,
        LevelSelect::Index(i) => i,
        LevelSelect::MinSize { width, height } => part.level_for_size(width, height),
    };
    let level = *part.levels.get(li).ok_or(ExrError::NotFound("level"))?;
    let (w, h) = (level.width as u64, level.height as u64);
    let lim = &opts.limits;
    if level.width > lim.max_width
        || level.height > lim.max_height
        || w * h > lim.max_pixels
        || w * h > (data.len() as u64).saturating_mul(lim.max_pixels_per_byte)
    {
        return Err(ExrError::TooLarge {
            width: w,
            height: h,
        });
    }
    Ok((part, level))
}

/// Like [`decode`] with headers already parsed by [`read_info`] on the same `data`.
pub fn decode_with_info(data: &[u8], info: &ExrInfo, opts: &DecodeOptions) -> Result<ExrImage> {
    let (part, level) = select(data, info, opts)?;
    let (w, h) = (level.width as u64, level.height as u64);
    let (map, names, color, has_alpha) = resolve_channels(part, &opts.channels)?;
    let plan = Plan::new(data, info, opts.part, level)?;
    let n = (w * h * 4) as usize;
    let size = match opts.format {
        SampleFormat::F32 => 4,
        SampleFormat::F16 => 2,
    };
    let _res = specular_core::budget::reserve_n(n as u64, size)?;
    let (pixels, damaged) = match opts.format {
        SampleFormat::F32 => {
            let mut v: Vec<f32> = alloc(n)?;
            let d = plan.decode(&map, &mut v, opts.parallel);
            if color == ColorKind::LuminanceChroma {
                yca_to_rgb(
                    &mut v,
                    part.chromaticities.unwrap_or(Chromaticities::REC709),
                    opts.parallel,
                );
            }
            (Pixels::F32(v), d)
        }
        SampleFormat::F16 => {
            let mut v: Vec<u16> = alloc(n)?;
            let d = plan.decode(&map, &mut v, opts.parallel);
            if color == ColorKind::LuminanceChroma {
                yca_to_rgb(
                    &mut v,
                    part.chromaticities.unwrap_or(Chromaticities::REC709),
                    opts.parallel,
                );
            }
            (Pixels::F16(v), d)
        }
    };
    Ok(ExrImage {
        width: level.width,
        height: level.height,
        pixels,
        level,
        data_window: part.data_window,
        display_window: part.display_window,
        pixel_aspect_ratio: part.pixel_aspect_ratio,
        chromaticities: part.chromaticities,
        white_luminance: part.white_luminance,
        channels: names,
        color,
        has_alpha,
        compression: part.compression,
        damaged_chunks: damaged,
    })
}

/// Random access to the rows of one part/level, for decoding a region or streaming a
/// reduced image without holding the whole level: [`RowReader::read_rows`] decodes only the
/// chunks (scanline blocks, or tiles) that cover the rows and columns asked for, as `f32`
/// RGBA with the same values [`decode`] gives (luminance/chroma converted, subsampled channels
/// filled in, missing alpha one).
pub struct RowReader<'a> {
    plan: Plan<'a>,
    map: Mapping,
    color: ColorKind,
    chroma: Chromaticities,
    has_alpha: bool,
    /// Largest vertical subsampling of a shown channel (1 for most files).
    max_ys: usize,
}

/// Per-thread buffers for [`RowReader::read_rows`].
#[derive(Default)]
pub struct RowScratch {
    st: (codec::Scratch, codec::BlockGeom),
    buf: Vec<f32>,
}

impl<'a> RowReader<'a> {
    /// Locates the chunks of the part and level `opts` selects (`opts.format` is ignored:
    /// rows are `f32`). Checks `opts.limits` as [`decode`] does; nothing is allocated for the
    /// pixels.
    pub fn new(data: &'a [u8], info: &'a ExrInfo, opts: &DecodeOptions) -> Result<Self> {
        let (part, level) = select(data, info, opts)?;
        let (map, _, color, has_alpha) = resolve_channels(part, &opts.channels)?;
        let max_ys = part
            .channels
            .iter()
            .zip(&map.slots_of)
            .filter(|(_, s)| !s.is_empty())
            .map(|(c, _)| c.y_sampling.max(1) as usize)
            .max()
            .unwrap_or(1);
        Ok(RowReader {
            plan: Plan::new(data, info, opts.part, level)?,
            map,
            color,
            chroma: part.chromaticities.unwrap_or(Chromaticities::REC709),
            has_alpha,
            max_ys,
        })
    }

    /// The selected level.
    pub fn level(&self) -> Level {
        self.plan.level
    }

    /// Rows that decode together (lines per chunk, or the tile height): reads aligned to it
    /// decode every chunk once.
    pub fn band_rows(&self) -> u32 {
        self.plan.band_rows() as u32
    }

    /// An alpha channel is read (otherwise alpha is one).
    pub fn has_alpha(&self) -> bool {
        self.has_alpha
    }

    /// Bytes of [`RowScratch`] one call of [`RowReader::read_rows`] for `rows` rows needs at
    /// most (the decoded bands, full level width, and one chunk's decompression buffers).
    pub fn scratch_bytes(&self, rows: u32) -> u64 {
        let p = &self.plan;
        let band = p.band_rows() as u64;
        let m = self.max_ys as u64 - 1;
        let window = (rows as u64 + 2 * m).div_ceil(band) * band + band;
        let chunk = band * p.width as u64 * p.part.channels.len() as u64 * 4;
        window * p.width as u64 * 16 + 3 * chunk
    }

    /// Decodes level rows `rows` and columns `cols` into `out` (`rows.len() * cols.len() * 4`
    /// interleaved RGBA `f32`, top row first). Returns the number of damaged or missing chunks
    /// met (their pixels are zero, alpha one).
    pub fn read_rows(
        &self,
        rows: std::ops::Range<u32>,
        cols: std::ops::Range<u32>,
        out: &mut [f32],
        s: &mut RowScratch,
    ) -> usize {
        let p = &self.plan;
        let (y0, y1) = (rows.start as usize, (rows.end as usize).min(p.height));
        let (x0, x1) = (cols.start as usize, (cols.end as usize).min(p.width));
        if y0 >= y1 || x0 >= x1 {
            return 0;
        }
        // Subsampled channels repeat the nearest sampled row: decode it too.
        let m = self.max_ys - 1;
        let (w0, w1) = (y0.saturating_sub(m), (y1 + m).min(p.height));
        let (first, damaged) = p.decode_window(&self.map, w0..w1, x0..x1, &mut s.buf, &mut s.st);
        if m > 0 {
            p.fill_subsampled_window(&self.map, &mut s.buf, first);
        }
        let (row_len, n) = (p.width * 4, (x1 - x0) * 4);
        for (y, o) in (y0..y1).zip(out.chunks_exact_mut(n)) {
            o.copy_from_slice(&s.buf[(y - first) * row_len + x0 * 4..][..n]);
        }
        if self.color == ColorKind::LuminanceChroma {
            let len = (y1 - y0) * n;
            yca_to_rgb(&mut out[..len], self.chroma, false);
        }
        damaged
    }
}

/// Zeroed buffer; fails instead of aborting if the allocator refuses.
fn alloc<T: Sample + Clone>(n: usize) -> Result<Vec<T>> {
    let bytes = (n as u64).saturating_mul(size_of::<T>() as u64);
    let mut probe: Vec<T> = Vec::new();
    probe
        .try_reserve_exact(n)
        .map_err(|_| ExrError::OutOfMemory(bytes))?;
    drop(probe);
    Ok(vec![T::ZERO; n])
}

type Resolved = (Mapping, [Option<String>; 4], ColorKind, bool);

fn resolve_channels(part: &Part, sel: &ChannelSelect) -> Result<Resolved> {
    let chans = &part.channels;
    let find_in = |layer: &str, base: &str| {
        chans
            .iter()
            .position(|c| c.layer() == layer && c.base_name() == base)
    };
    let has_color = |layer: &str| {
        ["R", "G", "B", "Y"]
            .iter()
            .any(|b| find_in(layer, b).is_some())
    };
    let mut slots: [Option<usize>; 4] = [None; 4];
    let mut color = ColorKind::Rgb;
    let layer_pick = |layer: &str, slots: &mut [Option<usize>; 4]| -> ColorKind {
        let (r, g, b, a) = (
            find_in(layer, "R"),
            find_in(layer, "G"),
            find_in(layer, "B"),
            find_in(layer, "A"),
        );
        slots[3] = a;
        if r.is_some() || g.is_some() || b.is_some() {
            slots[0] = r;
            slots[1] = g;
            slots[2] = b;
            return ColorKind::Rgb;
        }
        if let Some(y) = find_in(layer, "Y") {
            let (ry, by) = (find_in(layer, "RY"), find_in(layer, "BY"));
            if ry.is_some() || by.is_some() {
                *slots = [ry, Some(y), by, a];
                return ColorKind::LuminanceChroma;
            }
            *slots = [Some(y), Some(y), Some(y), a];
            return ColorKind::Luminance;
        }
        // Anything else (depth, masks...): the layer's first non-alpha channel as grey.
        if let Some(c) = chans
            .iter()
            .position(|c| c.layer() == layer && Some(c) != a.map(|a| &chans[a]))
        {
            *slots = [Some(c), Some(c), Some(c), a];
        }
        ColorKind::Luminance
    };
    match sel {
        ChannelSelect::Auto => {
            let layers = part.layers();
            let layer = if has_color("") {
                ""
            } else {
                layers
                    .iter()
                    .copied()
                    .find(|l| has_color(l))
                    .or_else(|| chans.first().map(|c| c.layer()))
                    .unwrap_or("")
            };
            color = layer_pick(layer, &mut slots);
        }
        ChannelSelect::Layer(l) => {
            if !chans.iter().any(|c| c.layer() == l) {
                return Err(ExrError::NotFound("layer"));
            }
            color = layer_pick(l, &mut slots);
        }
        ChannelSelect::Slots(names) => {
            for (s, n) in slots.iter_mut().zip(names) {
                if let Some(n) = n {
                    *s = Some(
                        chans
                            .iter()
                            .position(|c| &c.name == n)
                            .ok_or(ExrError::NotFound("channel"))?,
                    );
                }
            }
        }
    }
    let mut slots_of = vec![Vec::new(); chans.len()];
    for (s, c) in slots.iter().enumerate() {
        if let Some(c) = c {
            slots_of[*c].push(s);
        }
    }
    let names = slots.map(|c| c.map(|c| chans[c].name.clone()));
    let has_alpha = slots[3].is_some();
    Ok((
        Mapping {
            slots_of,
            alpha_one: !has_alpha,
        },
        names,
        color,
        has_alpha,
    ))
}

/// Luminance weights (the Y row of RGB->XYZ) for the given primaries.
///
/// Each primary's column is its (x, y, z) chromaticity scaled by s, with [r g b] * s the white
/// point's XYZ (Y = 1); the weights are then y * s. Nothing divides by a primary's y: XYZ
/// files (red (1, 0), green (0, 1), blue (0, 0)) fell back to Rec. 709 weights before, so
/// their green came out of luminance-chroma wrong (2-6 %, fidelity round 6).
fn luminance_weights(c: &Chromaticities) -> [f32; 3] {
    let xyz = |p: [f32; 2]| [p[0] as f64, p[1] as f64, 1.0 - p[0] as f64 - p[1] as f64];
    let (r, g, b) = (xyz(c.red), xyz(c.green), xyz(c.blue));
    let wy = c.white[1] as f64;
    let w = [
        c.white[0] as f64 / wy,
        1.0,
        (1.0 - c.white[0] as f64 - wy) / wy,
    ];
    // Solve [r g b] * s = w (Cramer's rule).
    let det = |a: [f64; 3], b: [f64; 3], c: [f64; 3]| {
        a[0] * (b[1] * c[2] - b[2] * c[1]) - b[0] * (a[1] * c[2] - a[2] * c[1])
            + c[0] * (a[1] * b[2] - a[2] * b[1])
    };
    let d = det(r, g, b);
    let s = [det(w, g, b) / d, det(r, w, b) / d, det(r, g, w) / d];
    let y = [r[1] * s[0], g[1] * s[1], b[1] * s[2]];
    let sum = y[0] + y[1] + y[2];
    if !(d.is_finite() && sum.is_finite() && y.iter().all(|v| v.is_finite()))
        || d == 0.0
        || sum == 0.0
    {
        return [0.2126, 0.7152, 0.0722];
    }
    [
        (y[0] / sum) as f32,
        (y[1] / sum) as f32,
        (y[2] / sum) as f32,
    ]
}

/// Convert (RY, Y, BY, A) to (R, G, B, A) in place, as OpenEXR's `RgbaYca::YCAtoRGB`.
fn yca_to_rgb<T: Sample>(px: &mut [T], c: Chromaticities, parallel: bool) {
    use rayon::prelude::*;
    let yw = luminance_weights(&c);
    let f = |p: &mut [T]| {
        let (ry, y, by) = (p[0].to_f32(), p[1].to_f32(), p[2].to_f32());
        if ry == 0.0 && by == 0.0 {
            p[0] = p[1];
            p[2] = p[1];
        } else {
            let r = (ry + 1.0) * y;
            let b = (by + 1.0) * y;
            let g = (y - r * yw[0] - b * yw[2]) / yw[1];
            p[0] = T::from_f32(r);
            p[1] = T::from_f32(g);
            p[2] = T::from_f32(b);
        }
    };
    if parallel {
        px.par_chunks_exact_mut(4).with_min_len(4096).for_each(f);
    } else {
        px.chunks_exact_mut(4).for_each(f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// XYZ primaries (red (1, 0), green (0, 1), blue (0, 0)): luminance is the green channel.
    #[test]
    fn xyz_weights() {
        let xyz = Chromaticities {
            red: [1.0, 0.0],
            green: [0.0, 1.0],
            blue: [0.0, 0.0],
            white: [1.0 / 3.0, 1.0 / 3.0],
        };
        let w = luminance_weights(&xyz);
        assert!(
            w[0].abs() < 1e-6 && (w[1] - 1.0).abs() < 1e-6 && w[2].abs() < 1e-6,
            "{w:?}"
        );
    }

    #[test]
    fn rec709_weights() {
        let w = luminance_weights(&Chromaticities::REC709);
        assert!(
            (w[0] - 0.2126).abs() < 1e-3
                && (w[1] - 0.7152).abs() < 1e-3
                && (w[2] - 0.0722).abs() < 1e-3,
            "{w:?}"
        );
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    fn attr(out: &mut Vec<u8>, name: &str, ty: &str, value: &[u8]) {
        out.extend_from_slice(name.as_bytes());
        out.push(0);
        out.extend_from_slice(ty.as_bytes());
        out.push(0);
        out.extend_from_slice(&(value.len() as i32).to_le_bytes());
        out.extend_from_slice(value);
    }

    /// A tiled mipmap of 1x1 tiles over a (2^32 - 1)-square data window and no chunkCount:
    /// summing the levels' tile counts overflowed u64, at open and again when locating a
    /// small level's chunks.
    #[test]
    fn mipmap_tile_count_overflow() {
        let mut f = header::MAGIC.to_vec();
        f.extend_from_slice(&(2u32 | 0x200).to_le_bytes());
        let mut ch = b"R\0".to_vec();
        ch.extend_from_slice(&1i32.to_le_bytes());
        ch.extend_from_slice(&[0; 4]);
        ch.extend_from_slice(&1i32.to_le_bytes());
        ch.extend_from_slice(&1i32.to_le_bytes());
        ch.push(0);
        attr(&mut f, "channels", "chlist", &ch);
        attr(&mut f, "compression", "compression", &[0]);
        let mut dw = Vec::new();
        for v in [i32::MIN, i32::MIN, i32::MAX - 1, i32::MAX - 1] {
            dw.extend_from_slice(&v.to_le_bytes());
        }
        attr(&mut f, "dataWindow", "box2i", &dw);
        let mut t = 1u32.to_le_bytes().to_vec();
        t.extend_from_slice(&1u32.to_le_bytes());
        t.push(1);
        attr(&mut f, "tiles", "tiledesc", &t);
        f.push(0);
        f.extend_from_slice(&[0xff; 64]);
        let info = read_info(&f).unwrap();
        assert_eq!(info.parts[0].chunk_count, u64::MAX);
        let last = info.parts[0].levels.len() - 1;
        let opts = DecodeOptions {
            level: LevelSelect::Index(last),
            parallel: false,
            ..DecodeOptions::default()
        };
        let img = decode(&f, &opts).unwrap();
        assert_eq!((img.width, img.height, img.damaged_chunks), (1, 1, 1));
        assert!(decode(&f, &DecodeOptions::default()).is_err());
    }

    /// 1x1 tiles over 1024 x 1024 pixels in a 2 KB file: the 24 MiB chunk table is refused
    /// under a 16 MiB memory budget before it is allocated, and located under 256 MiB.
    #[test]
    fn chunk_table_within_memory_budget() {
        use specular_core::budget::with_limit;
        let mut f = header::MAGIC.to_vec();
        f.extend_from_slice(&(2u32 | 0x200).to_le_bytes());
        let mut ch = b"R\0".to_vec();
        ch.extend_from_slice(&1i32.to_le_bytes());
        ch.extend_from_slice(&[0; 4]);
        ch.extend_from_slice(&1i32.to_le_bytes());
        ch.extend_from_slice(&1i32.to_le_bytes());
        ch.push(0);
        attr(&mut f, "channels", "chlist", &ch);
        attr(&mut f, "compression", "compression", &[0]);
        let mut dw = Vec::new();
        for v in [0i32, 0, 1023, 1023] {
            dw.extend_from_slice(&v.to_le_bytes());
        }
        attr(&mut f, "dataWindow", "box2i", &dw);
        let mut t = 1u32.to_le_bytes().to_vec();
        t.extend_from_slice(&1u32.to_le_bytes());
        t.push(0);
        attr(&mut f, "tiles", "tiledesc", &t);
        f.push(0);
        f.resize(2048, 0xff);
        let info = read_info(&f).unwrap();
        let opts = DecodeOptions::default();
        let r = with_limit(16 << 20, || RowReader::new(&f, &info, &opts).map(|_| ()));
        let Err(ExrError::NeedsMemory(n)) = r else {
            panic!("{r:?}")
        };
        assert!(n.needed >= 24 << 20 && n.needed > n.free, "{n:?}");
        assert!(with_limit(256 << 20, || RowReader::new(&f, &info, &opts).is_ok()));
    }

    /// 4096 FLOAT channels over 65536 x 16 pixels in one 8-byte ZIP chunk: 16 GiB of scratch
    /// was zero-filled before inflating.
    #[test]
    fn channel_bomb_chunk() {
        let mut f = header::MAGIC.to_vec();
        f.extend_from_slice(&2u32.to_le_bytes());
        let mut ch = Vec::new();
        for i in 0..4096 {
            ch.extend_from_slice(format!("c{i:04}\0").as_bytes());
            ch.extend_from_slice(&2i32.to_le_bytes());
            ch.extend_from_slice(&[0; 4]);
            ch.extend_from_slice(&1i32.to_le_bytes());
            ch.extend_from_slice(&1i32.to_le_bytes());
        }
        ch.push(0);
        attr(&mut f, "channels", "chlist", &ch);
        attr(&mut f, "compression", "compression", &[3]);
        let mut dw = Vec::new();
        for v in [0i32, 0, 65535, 15] {
            dw.extend_from_slice(&v.to_le_bytes());
        }
        attr(&mut f, "dataWindow", "box2i", &dw);
        f.push(0);
        let chunk = (f.len() + 8) as u64;
        f.extend_from_slice(&chunk.to_le_bytes());
        f.extend_from_slice(&0i32.to_le_bytes());
        f.extend_from_slice(&8i32.to_le_bytes());
        f.extend_from_slice(&[0x78, 1, 0, 0, 0, 0, 0, 0]);
        let opts = DecodeOptions {
            format: SampleFormat::F16,
            parallel: false,
            ..DecodeOptions::default()
        };
        let img = decode(&f, &opts).unwrap();
        assert_eq!((img.width, img.height, img.damaged_chunks), (65536, 16, 1));
    }
}
