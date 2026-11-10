//! `specular-jpeg`: a fast JPEG decoder for very large images.
//!
//! * **Planar output.** [`decode`] returns each component at its own (subsampled) resolution;
//!   colour conversion is left to the GPU (or [`planes_to_rgba`]).
//! * **DCT-domain scaling.** [`Scale::Half`], [`Scale::Quarter`] and [`Scale::Eighth`] use 4x4,
//!   2x2 and DC-only reduced IDCTs (bit-exact with libjpeg-turbo's `jidctred.c`); at 1/8 the AC
//!   coefficients are only skipped, and progressive AC scans are not read at all.
//! * **Parallelism.** Restart-marker segments are decoded concurrently. For files without
//!   restart markers, the entropy-coded data is split into chunks that are parsed speculatively
//!   in parallel (self-synchronising Huffman codes), giving one checkpoint per MCU row (and per
//!   block row for progressive AC first scans);
//!   if that fails (e.g. truncated data), entropy decoding is pipelined on one thread with IDCT
//!   on the others. A [`RowIndex`] makes any file restart-parallel and enables cheap region
//!   decoding. [`Parallelism::Auto`] stays single-threaded below about 2 MP.
//! * **Progressive pipeline.** Progressive files are decoded scan-parallel: the DC scans and
//!   each component's AC scans form independent pipelines, each scan is a stage that works on
//!   bands of MCU rows (in order for refinement scans, in parallel from checkpoints where the
//!   scan allows it), and dequantisation + IDCT of a band starts as soon as all of its scans
//!   are done, overlapping with entropy decoding of later bands.
//! * **SIMD.** IDCTs use portable `std::simd`; the crate is built for the workspace CPU baseline
//!   (x86-64-v3 on x86) and needs no runtime feature detection. The only `unsafe` in the crate is
//!   one BMI2 `pdep` call in progressive refinement decoding (see `UNSAFE.md` at the workspace
//!   root); other targets use a safe bit loop.
//! * **Robustness.** Corrupt or truncated data never panics; missing rows are filled with grey
//!   and flagged in [`DecodedPlanes::truncated`] (also set when a multi-scan file lacks EOI).
//!
//! # Plane geometry
//! Let a component have sampling factors `(h, v)`, with frame maxima `(Hmax, Vmax)`, at scale `s`.
//! At reduced scales, subsampled components may use a larger IDCT than `8 / s`, exactly as
//! libjpeg does, so that chroma is not needlessly shrunk. For example, at 1/2, 4:2:0 chroma is
//! decoded at 8x8, i.e. at the same resolution as luma. [`Plane::h_samp`]/[`Plane::v_samp`]
//! report the resulting *effective* factors, and Hmax/Vmax below are the maxima of those.
//! (At 1/8 every component is DC-only.)
//! * `plane.width  = ceil(W * h / (Hmax * s))` (for a region: the part of that inside the region),
//! * `plane.height = ceil(H * v / (Vmax * s))`,
//! * `plane.stride` is the MCU-padded row length (`>= width`); `data.len() == stride * height`.
//!
//! Chroma sample `i` is *centered* (JPEG/JFIF siting): it covers full-resolution samples
//! `[i * f, (i + 1) * f)` with `f = Hmax / h`, so its center is at `(i + 0.5) * f - 0.5`.
//!
//! # Memory
//! * **Baseline (sequential) files** are decoded straight into the output planes: the only
//!   large allocation is the output itself (`sum(plane.stride * plane.height)`, i.e. 1.5 B/px
//!   for 4:2:0 and 3 B/px for 4:4:4 at full scale, divided by `s * s` at scale `1/s`), plus
//!   transient per-thread scratch.
//! * **Progressive files** (and multi-scan sequential ones) need a coefficient buffer, because a
//!   block is only final after the last scan: 2 B (DC) + 128 B (AC) per 8x8 block over the
//!   decoded MCU rows, about 3.05 B/px for 4:2:0 and 6.1 B/px for 4:4:4 (e.g. ~150 MB for 50 MP
//!   4:2:0, ~1 GB for 160 MP 4:4:4), on top of the output. At [`Scale::Eighth`] only the 2 B DC
//!   part is allocated. With a [`RowIndex`] and a region, only the region's MCU rows (full
//!   width) are buffered, so a viewer can bound memory by decoding in horizontal stripes with
//!   [`decode_with_index`]. Building the index ([`build_index`]) keeps no coefficients: the
//!   bit-level parse of a refinement scan only depends on which coefficients are already
//!   nonzero, so it keeps one 64-bit nonzero mask per block (8 B, 1 bit per coefficient) for
//!   the components that still have refinement scans, and nothing for scans checkpointed
//!   speculatively. A 50 MP 4:2:0 progressive file is indexed in ~10 MB of heap (mostly masks)
//!   and then decoded in stripes of a few MB each.
//! * **Speculative parsing** keeps a record (12-28 B) of every 32nd MCU / block start, plus
//!   every one near the chunk boundaries where the parallel runs are matched up: typically 1-3%
//!   of a coefficient buffer.
//! * [`memory_estimate`] (headers only) and [`memory_estimate_for`] (actual scan script) predict
//!   the peak heap use of a decode, a region decode with an index, and an index build, to within
//!   about 10%, so a caller can choose a scale or striped decoding before allocating anything.
//! * **Index**: [`RowIndex::memory_bytes`], one 56-byte checkpoint per MCU row (per scan for
//!   progressive files); for example ~30 KB for a 100 MP 4:2:0 baseline file and ~0.3 MB for a
//!   50 MP 4:2:0 progressive one with 10 scans.
//! * Allocations of large buffers are fallible: if the allocator refuses, the decode returns
//!   [`JpegError::TooLarge`] instead of aborting.
//!
//! # Arithmetic coding
//! Sequential and progressive arithmetic-coded frames (SOF9, SOF10; 8 and 12 bits) are decoded
//! natively (module `arith`): every scan into a coefficient buffer on one thread (the coder's
//! adaptive statistics chain every block to the previous one), then the parallel scaled IDCT of
//! the coefficient path. Every entry point works for them, including region decodes and
//! [`RowIndex`] (whose checkpoints then also hold the statistics of the scan's tables, a few
//! hundred bytes per row). Arithmetic lossless frames (SOF11, T.81 Annex H) are decoded
//! natively too (`arith::lossless`, serial, no index) and share the Huffman lossless path's
//! 8-bit output rule.
//!
//! # Hierarchical mode and DNL
//! Hierarchical files (T.81 Annex J: DHP, EXP, differential frames; sequential, progressive
//! and arithmetic DCT frames, Huffman lossless frames) are decoded by module `hier`, serially
//! and whole for every call (see [`is_hierarchical`]). A height of 0 in the SOF is taken from
//! the DNL segment after the first scan, for every coding.
//!
//! # Not supported
//! * Arithmetic-coded lossless frames inside a hierarchical image.
//! * Fractional sampling ratios in DCT frames (where `Hmax % h != 0`, e.g. 3x1 luma with
//!   2x1 chroma): refused by name. T.81 allows them, but no encoder writes them, libjpeg and
//!   libjpeg-turbo refuse them ("Fractional sampling not implemented yet"), so there is no
//!   file to show and no oracle to check a decoder against; every plane consumer here (GPU
//!   upsampling, [`planes_to_rgba`]) also assumes whole ratios.
//!
//! # Precision
//! 12-bit frames are decoded natively (12-bit IDCT) and output as 8 bits; Huffman lossless frames (SOF3) are decoded
//! natively too (module `lossless`; restart intervals of whole rows in parallel on large
//! images; `jpeg-decoder` is its test oracle) and likewise reduced to 8 bits. [`decode_wide`] decodes
//! every process (sequential, progressive, arithmetic, lossless at 2..=16 bits, hierarchical)
//! to raw `u16` samples instead, for callers that need the stored values (DICOM).

#![feature(portable_simd)]

mod arith;
mod cmyk_hook;
mod coef;
mod color;
mod cols;
pub mod encode;
mod error;
mod fallback;
mod geom;
mod hier;
mod huffman;
mod idct;
mod lossless;
mod marker;
mod memory;
mod prog;
mod sequential;
mod spec;
pub mod transform;
mod wide;

use std::collections::HashMap;
use std::ops::Range;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

pub use cmyk_hook::{CmykHook, CmykRowConverter, set_cmyk_hook};
pub use color::{Upsampling, planes_to_rgba, planes_to_rgba_into};
pub use error::{JpegError, Result};
pub use transform::{
    Alignment, ComponentCoefficients, Transform, TransformOptions, read_coefficients, transform,
    transform_alignment,
};
pub use wide::{WideImage, WidePlane, decode_wide};

use geom::{OutGeom, alloc_zeroed};
use idct::Simd;
use marker::Parsed;

/// Entropy coding / process of the frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coding {
    /// SOF0.
    Baseline,
    /// SOF1 (8- or 12-bit sequential Huffman).
    ExtendedHuffman,
    /// SOF2 / SOF6 Huffman progressive.
    Progressive,
    /// Any arithmetic-coded frame (SOF9..SOF11).
    Arithmetic,
    /// SOF3 / SOF7 lossless.
    Lossless,
}

/// Colour model of the stored components, determined with libjpeg's rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorModel {
    Gray,
    YCbCr,
    Rgb,
    Cmyk,
    Ycck,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComponentInfo {
    pub id: u8,
    pub h_samp: u8,
    pub v_samp: u8,
    pub quant_table: u8,
}

/// Header information. Byte ranges index into the buffer passed to [`read_info`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JpegInfo {
    pub width: u32,
    pub height: u32,
    /// Sample precision in bits (8 or 12; 2..=16 for lossless).
    pub precision: u8,
    pub coding: Coding,
    pub color: ColorModel,
    pub components: Vec<ComponentInfo>,
    pub max_h: u8,
    pub max_v: u8,
    /// Restart interval (in MCUs) in effect for the first scan; 0 = none.
    pub restart_interval: u16,
    /// Reassembled ICC profile (APP2 chunks in sequence order).
    pub icc_profile: Option<Vec<u8>>,
    /// The TIFF structure of the Exif APP1 segment (after `Exif\0\0`).
    pub exif: Option<Range<usize>>,
    /// The XMP packet of the XMP APP1 segment.
    pub xmp: Option<Range<usize>>,
    /// Embedded JPEG thumbnail (Exif IFD1), validated to start with SOI.
    pub exif_thumbnail: Option<Range<usize>>,
    /// Adobe APP14 colour transform flag, if the marker is present.
    pub adobe_transform: Option<u8>,
    /// An APP11 JUMBF segment (C2PA manifest) is present.
    pub has_c2pa: bool,
    /// Offset of the first SOS marker (everything before is headers / metadata).
    pub header_len: usize,
    /// The SOFn marker code (0xC0..=0xCF).
    pub sof_marker: u8,
    /// A JFIF APP0 marker is present.
    pub jfif: bool,
    /// Number of components in the first scan. Fewer than `components.len()` in a sequential
    /// frame means a multi-scan (non-interleaved) file, which is decoded via a coefficient
    /// buffer like a progressive one.
    pub first_scan_components: u8,
}

/// Read header information only (stops at the first SOS; never touches entropy-coded data).
pub fn read_info(data: &[u8]) -> Result<JpegInfo> {
    Ok(logged("read_info", marker::parse(data, false))?.info)
}

/// Whether `data` is a hierarchical JPEG (a DHP segment before the first frame, T.81 Annex J).
/// [`read_info`] then describes the final image, with the coding and SOF marker of the first
/// frame; every decode entry point handles it (serially, by decoding the whole sequence of
/// frames), and the coefficient-level APIs ([`read_coefficients`], [`transform`],
/// [`decode_wide`]) refuse it.
pub fn is_hierarchical(data: &[u8]) -> bool {
    marker::parse(data, false).is_ok_and(|p| p.hierarchical)
}

/// Estimated heap memory of a decode, from [`memory_estimate`]. All values are in bytes and
/// exclude the JPEG data itself.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemoryEstimate {
    /// The returned [`DecodedPlanes`] (plane buffers including their MCU padding).
    pub output_bytes: u64,
    /// Peak temporary memory of a [`decode`] on top of `output_bytes` (coefficient buffers,
    /// speculative-parsing records, scratch). The peak of the whole call is
    /// `output_bytes + working_bytes` ([`MemoryEstimate::peak`]).
    pub working_bytes: u64,
    /// Like `working_bytes`, for [`decode_with_index`] with the same scale and region.
    pub working_bytes_with_index: u64,
    /// Size of the [`RowIndex`] ([`RowIndex::memory_bytes`]); 0 if no index can be built.
    pub index_bytes: u64,
    /// Peak memory of [`build_index`], including the returned index.
    pub index_build_bytes: u64,
}

impl MemoryEstimate {
    /// Peak heap memory of a [`decode`] (output plus working memory).
    pub fn peak(&self) -> u64 {
        self.output_bytes + self.working_bytes
    }
    /// Peak heap memory of a [`decode_with_index`] (output plus working memory).
    pub fn peak_with_index(&self) -> u64 {
        self.output_bytes + self.working_bytes_with_index
    }
}

/// Estimate the memory a decode of the image described by `info` at `scale` (optionally of
/// `region`) will need, from the headers alone. Lets a caller pick a scale or switch to striped
/// region decoding ([`decode_with_index`] over horizontal bands) before allocating anything.
///
/// Models a [`Parallelism::Auto`] decode; accurate to within about 10% (single-threaded decodes
/// need slightly less: no speculative parsing). Progressive files are assumed to use the
/// standard libjpeg scan script (`cjpeg`/`jpegtran -progressive`, most cameras and encoders).
/// That only matters for the index and for [`MemoryEstimate::index_build_bytes`], which for
/// scripts without refinement scans can be several times lower than estimated (no nonzero masks
/// are needed); the coefficient buffer, the dominant term of a decode, does not depend on the
/// script. [`memory_estimate_for`] reads the actual script. Lossless files (decoded by a
/// fallback decoder) are estimated roughly. An invalid region gives an all-zero estimate.
pub fn memory_estimate(info: &JpegInfo, scale: Scale, region: Option<Region>) -> MemoryEstimate {
    memory::estimate(info, scale, region, None)
}

/// Like [`memory_estimate`], but from the file: uses its actual scan script and scan sizes
/// (one pass over the markers, no entropy decoding).
pub fn memory_estimate_for(
    data: &[u8],
    scale: Scale,
    region: Option<Region>,
) -> Result<MemoryEstimate> {
    Ok(memory::estimate_parsed(
        &marker::parse(data, true)?,
        scale,
        region,
    ))
}

/// DCT-domain downscale factor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum Scale {
    #[default]
    Full = 1,
    Half = 2,
    Quarter = 4,
    Eighth = 8,
}

impl Scale {
    pub fn factor(self) -> u32 {
        self as u32
    }
}

/// Pick the smallest decode (largest downscale) that still covers a `target_w x target_h`
/// viewport when the image is *fit* into it (aspect preserved), i.e. the largest `s` with
/// `W >= s * target_w || H >= s * target_h`. The limiting dimension is thus never upsampled
/// for display, the other dimension may be smaller than its target. Returns [`Scale::Full`]
/// if even 1/2 would be too small; a zero target returns [`Scale::Eighth`].
pub fn choose_scale(info: &JpegInfo, target_w: u32, target_h: u32) -> Scale {
    if target_w == 0 || target_h == 0 {
        return Scale::Eighth;
    }
    for s in [Scale::Eighth, Scale::Quarter, Scale::Half] {
        let f = s as u64;
        if info.width as u64 >= f * target_w as u64 || info.height as u64 >= f * target_h as u64 {
            return s;
        }
    }
    Scale::Full
}

/// A decoded component plane (or an interleaved RGBA plane for [`PlanesLayout::Rgba`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plane {
    /// Visible width in samples.
    pub width: u32,
    pub height: u32,
    /// Row stride in bytes.
    pub stride: usize,
    pub data: Vec<u8>,
    /// Sampling factors of this plane relative to the largest factors among the planes (1 for
    /// RGBA). A plane with factors `(h, v)` is upsampled by `(max_h / h, max_v / v)` to the output
    /// size. At full scale these are the frame's factors; for scaled decodes subsampled
    /// components are decoded with a larger IDCT like libjpeg does (e.g. 4:2:0 chroma at
    /// [`Scale::Half`] comes out at full output resolution and reports the luma's factors).
    pub h_samp: u8,
    pub v_samp: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanesLayout {
    /// One plane.
    Gray,
    /// Y, Cb, Cr planes (BT.601 full range).
    YCbCr,
    /// R, G, B planes (and alpha, from [`decode_components`]).
    Rgb,
    /// One interleaved RGBA8 plane (CMYK / YCCK / fallback decodes).
    Rgba,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedPlanes {
    /// Size of the decoded area in output (scaled) pixels.
    pub width: u32,
    pub height: u32,
    /// Top-left of the decoded area in the scaled image (non-zero only for region decodes;
    /// MCU-aligned, so it may lie up/left of the requested region).
    pub origin: (u32, u32),
    pub layout: PlanesLayout,
    pub planes: Vec<Plane>,
    /// Entropy data ended early (or was corrupt); missing rows are mid-grey (or, for
    /// progressive files, lack the missing refinement).
    pub truncated: bool,
}

/// Rectangle in output (scaled) pixel coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Parallelism {
    /// Use the global rayon pool for images of at least ~2 MP, a single thread otherwise.
    #[default]
    Auto,
    Single,
    /// Use a dedicated (cached) pool with this many threads.
    Threads(usize),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DecodeOptions<'a> {
    pub scale: Scale,
    /// Decode only this rectangle (output coordinates). The result covers the MCU-aligned
    /// superset of the rectangle, see [`DecodedPlanes::origin`].
    pub region: Option<Region>,
    /// Checked at least once per MCU row; when set the decode returns [`JpegError::Cancelled`].
    pub cancel: Option<&'a AtomicBool>,
    pub parallelism: Parallelism,
}

/// Below this many source pixels `Parallelism::Auto` decodes on the calling thread.
pub const AUTO_PARALLEL_MIN_PIXELS: u64 = 2_000_000;

/// Saved entropy-decoder state at the start of every MCU row (per scan for progressive files),
/// allowing parallel and region decoding of files without restart markers.
///
/// Size: [`RowIndex::memory_bytes`] (one 56-byte checkpoint per MCU row and scan). For
/// progressive files, decoding with an index runs every scan from its row checkpoints, so a
/// region decode only entropy-decodes and buffers the region's MCU rows (see the crate docs,
/// *Memory*). Building one for a progressive file costs about one decode without the IDCT
/// (trailing AC scans that can be checkpointed speculatively are not decoded at all); it needs
/// no coefficient buffer, only an 8-byte nonzero mask per block of components with refinement
/// scans ([`MemoryEstimate::index_build_bytes`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowIndex {
    fingerprint: u64,
    kind: IndexKind,
    /// Column checkpoints recorded by region decodes (not part of equality; a clone starts
    /// empty).
    cols: ColCell,
}

struct ColCell(cols::ColCache);

impl Clone for ColCell {
    fn clone(&self) -> Self {
        ColCell(self.0.empty_like())
    }
}

impl PartialEq for ColCell {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for ColCell {}

impl std::fmt::Debug for ColCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ColCache({} checkpoints)", self.0.len())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum IndexKind {
    Seq(Vec<sequential::Checkpoint>),
    Coef(coef::ScanIndex),
    Arith(arith::ArithIndex),
    /// Hierarchical files: no checkpoints, every decode runs the whole sequence of frames.
    Hier,
}

impl RowIndex {
    fn new(p: &Parsed, fingerprint: u64, kind: IndexKind) -> RowIndex {
        let cols = match &kind {
            IndexKind::Seq(_) => cols::ColCache::sequential(p),
            IndexKind::Coef(v) => {
                cols::ColCache::scans(p, &v.iter().map(|s| s.len()).collect::<Vec<_>>())
            }
            // Arithmetic decodes do not record column checkpoints.
            IndexKind::Arith(_) | IndexKind::Hier => cols::ColCache::scans(p, &[]),
        };
        RowIndex {
            fingerprint,
            kind,
            cols: ColCell(cols),
        }
    }

    /// Number of row checkpoints stored.
    pub fn len(&self) -> usize {
        match &self.kind {
            IndexKind::Seq(v) => v.len(),
            IndexKind::Coef(v) => v.iter().map(|s| s.len()).sum(),
            IndexKind::Arith(v) => v.iter().map(|s| s.len()).sum(),
            IndexKind::Hier => 0,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Approximate heap size in bytes.
    pub fn memory_bytes(&self) -> usize {
        if let IndexKind::Arith(v) = &self.kind {
            return v.iter().flatten().map(|c| c.heap_bytes()).sum();
        }
        (self.len() + self.cols.0.len()) * std::mem::size_of::<sequential::Checkpoint>()
    }
}

fn fingerprint(data: &[u8], info: &JpegInfo) -> u64 {
    // FNV-1a over the length, the headers and the last 64 bytes.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: &[u8]| {
        for &x in b {
            h ^= x as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
    };
    eat(&(data.len() as u64).to_le_bytes());
    eat(&data[..info.header_len.min(data.len()).min(1 << 16)]);
    eat(&data[data.len().saturating_sub(64)..]);
    h
}

// ---------------------------------------------------------------------------------------------

fn pool(n: usize) -> Arc<rayon::ThreadPool> {
    static POOLS: Mutex<Option<HashMap<usize, Arc<rayon::ThreadPool>>>> = Mutex::new(None);
    let mut g = POOLS.lock().unwrap_or_else(|e| e.into_inner());
    g.get_or_insert_with(HashMap::new)
        .entry(n)
        .or_insert_with(|| {
            Arc::new(
                specular_thread::rayon_pool("dec-jpeg")
                    .num_threads(n)
                    .build()
                    .expect("rayon pool"),
            )
        })
        .clone()
}

/// Run `f(threads)` on the pool selected by `par`.
fn with_threads<T: Send>(par: Parallelism, pixels: u64, f: impl FnOnce(usize) -> T + Send) -> T {
    match par {
        Parallelism::Single => f(1),
        Parallelism::Threads(n) if n <= 1 => f(1),
        Parallelism::Threads(n) => pool(n).install(|| f(n)),
        Parallelism::Auto => {
            if pixels < AUTO_PARALLEL_MIN_PIXELS {
                f(1)
            } else {
                let n = rayon::current_num_threads();
                f(n)
            }
        }
    }
}

#[derive(Debug)]
enum Route {
    /// Hierarchical mode (`hier`): every frame serially, then the reconstruction.
    Hier,
    Streaming,
    Coef,
    Arith,
    Fallback,
}

/// Logs the chosen decode path and why (debug, target `jpeg`).
fn log_route(p: &Parsed, r: &Route) {
    let f = &p.frame;
    let why = match r {
        Route::Hier => "hierarchical: every frame serially, full-precision reconstruction",
        Route::Fallback if f.arithmetic => "arithmetic lossless: native, serial",
        Route::Fallback => "huffman lossless: native, restart intervals in parallel",
        Route::Arith => "arithmetic coding: coefficient buffers",
        Route::Streaming => "8-bit sequential, all components in one scan: streaming MCU decode",
        Route::Coef if f.progressive => "progressive: coefficient pipeline",
        Route::Coef if f.precision != 8 => "12-bit: coefficient buffers",
        Route::Coef => "multi-scan sequential: coefficient buffers",
    };
    tracing::debug!(target: "jpeg", route = ?r, why, "path");
}

/// Logs a failed public call with the error (debug, target `jpeg`); header errors are also
/// logged with their byte offset where they are detected.
fn logged<T>(what: &'static str, r: Result<T>) -> Result<T> {
    if let Err(e) = &r {
        tracing::debug!(target: "jpeg", error = %e, "{what} failed");
    }
    r
}

fn route(p: &Parsed) -> Result<Route> {
    let f = &p.frame;
    if p.hierarchical {
        return Ok(Route::Hier);
    }
    if f.lossless {
        return Ok(Route::Fallback);
    }
    // Like libjpeg ("Fractional sampling not implemented yet"): every component's sampling
    // factors must divide the maximum ones.
    if f.comps
        .iter()
        .any(|c| !f.max_h.is_multiple_of(c.h) || !f.max_v.is_multiple_of(c.v))
    {
        return Err(JpegError::Unsupported("fractional chroma sampling ratio"));
    }
    if f.arithmetic {
        return Ok(Route::Arith);
    }
    if f.precision == 8 && !f.progressive && p.scans[0].comps.len() == f.comps.len() {
        Ok(Route::Streaming)
    } else {
        Ok(Route::Coef)
    }
}

fn layout_for(color: ColorModel) -> PlanesLayout {
    match color {
        ColorModel::Gray => PlanesLayout::Gray,
        ColorModel::YCbCr => PlanesLayout::YCbCr,
        ColorModel::Rgb => PlanesLayout::Rgb,
        ColorModel::Cmyk | ColorModel::Ycck => PlanesLayout::Rgba,
    }
}

fn guard_size(p: &Parsed, data: &[u8], g: &OutGeom) -> Result<()> {
    let avail_bits = (data.len().saturating_sub(p.info.header_len) as u64) * 8;
    let blocks = p.frame.total_blocks();
    // Each Huffman-coded block costs at least 2 bits (1-bit DC code + 1-bit EOB). Refuse big
    // allocations for files that cannot possibly contain the declared image. Arithmetic coding
    // can spend far less than a bit on a flat block (two decisions at the most skewed
    // estimate), so its bound is only against absurdly short files.
    let per_bit: u64 = if p.frame.arithmetic { 1 << 20 } else { 8 };
    if g.total_bytes() > (64 << 20) && blocks > avail_bits.saturating_mul(per_bit) {
        return Err(JpegError::Truncated(
            "file far too short for the declared image size",
        ));
    }
    Ok(())
}

struct Ctx {
    want_index: bool,
    /// Leave four components as stored ([`decode_components`]).
    raw: bool,
}

fn decode_impl(
    data: &[u8],
    opts: &DecodeOptions,
    index: Option<&RowIndex>,
    cx: Ctx,
) -> Result<(DecodedPlanes, Option<RowIndex>)> {
    let mut parsed = marker::parse(data, false)?;
    let mut r = route(&parsed)?;
    if !matches!(r, Route::Streaming) {
        parsed = marker::reparse(data)?;
        r = route(&parsed)?;
    }
    let p = &parsed;
    log_route(p, &r);
    tracing::debug!(
        target: "jpeg",
        scale = ?opts.scale,
        region = ?opts.region,
        parallelism = ?opts.parallelism,
        with_index = index.is_some(),
        want_index = cx.want_index,
        "decode request"
    );
    if let Route::Hier = r {
        let fp = fingerprint(data, &p.info);
        if index.is_some_and(|ix| ix.fingerprint != fp) {
            return Err(JpegError::IndexMismatch);
        }
        let o = hier::decode(data, p, opts)?;
        let d = finish_planes(p, o.width, o.height, o.origin, o.planes, false, cx.raw);
        let ix = cx.want_index.then(|| RowIndex::new(p, fp, IndexKind::Hier));
        return Ok((d, ix));
    }
    if let Route::Fallback = r {
        // No index for lossless decodes; decode works regardless.
        if p.frame.arithmetic {
            return arith::lossless::decode(p, data, opts.scale, opts.region, opts.cancel)
                .map(|d| (d, None));
        }
        let pixels = p.frame.width as u64 * p.frame.height as u64;
        return with_threads(opts.parallelism, pixels, |threads| {
            lossless::decode(p, data, opts.scale, opts.region, opts.cancel, threads)
        })
        .map(|d| (d, None));
    }
    let fp = fingerprint(data, &p.info);
    if let Some(ix) = index
        && ix.fingerprint != fp
    {
        return Err(JpegError::IndexMismatch);
    }
    let g = OutGeom::new(&p.frame, opts.scale, opts.region)?;
    guard_size(p, data, &g)?;
    if tracing::enabled!(target: "jpeg", tracing::Level::DEBUG) {
        let m = memory::estimate_parsed(p, opts.scale, opts.region);
        tracing::debug!(
            target: "jpeg",
            output = g.total_bytes(),
            region_rows = ?(g.my0..g.my1),
            estimate = ?m,
            "memory"
        );
    }
    let mut planes: Vec<Vec<u8>> = Vec::with_capacity(g.comps.len());
    for c in &g.comps {
        planes.push(alloc_zeroed(c.stride * c.padded_h)?);
    }
    let simd = Simd::detect();
    let pixels = p.frame.width as u64 * p.frame.height as u64;

    let (truncated, new_index) = with_threads(
        opts.parallelism,
        pixels,
        |threads| -> Result<(bool, Option<RowIndex>)> {
            match r {
                Route::Streaming => {
                    let sc = sequential::SeqScan::new(p, data)?;
                    let ix = match index.map(|i| &i.kind) {
                        Some(IndexKind::Seq(v)) => Some(v.as_slice()),
                        Some(_) => return Err(JpegError::IndexMismatch),
                        None => None,
                    };
                    let sp = sequential::SeqParams {
                        simd,
                        threads,
                        cancel: opts.cancel,
                        index: ix,
                        want_index: cx.want_index,
                        cols: index.map(|i| &i.cols.0),
                    };
                    let out = sequential::decode(&sc, &g, &mut planes, &sp)?;
                    if let Some(t) = out.truncated_at {
                        sequential::fill_grey_from(&mut planes, &g, t);
                    }
                    Ok((
                        out.truncated_at.is_some(),
                        out.index.map(|v| RowIndex::new(p, fp, IndexKind::Seq(v))),
                    ))
                }
                Route::Coef => {
                    let ix = match index.map(|i| &i.kind) {
                        Some(IndexKind::Coef(v)) => Some(v),
                        Some(_) => return Err(JpegError::IndexMismatch),
                        None => None,
                    };
                    let cp = coef::CoefParams {
                        simd,
                        threads,
                        cancel: opts.cancel,
                        index: ix,
                        want_index: cx.want_index,
                        cols: index.map(|i| &i.cols.0),
                        no_output: false,
                        reuse: None,
                    };
                    let mut out: Vec<&mut [u8]> =
                        planes.iter_mut().map(|v| v.as_mut_slice()).collect();
                    let out = if ix.is_some() {
                        coef_striped(p, data, &g, opts.scale, &mut out, &cp)?
                    } else {
                        coef::decode(p, data, &g, &mut out, &cp)?
                    };
                    // Multi-scan files whose data ends before EOI are missing scans (lower quality).
                    Ok((
                        out.truncated || !p.saw_eoi,
                        out.index.map(|v| RowIndex::new(p, fp, IndexKind::Coef(v))),
                    ))
                }
                Route::Arith => {
                    let ix = match index.map(|i| &i.kind) {
                        Some(IndexKind::Arith(v)) => Some(v),
                        Some(_) => return Err(JpegError::IndexMismatch),
                        None => None,
                    };
                    let ap = arith::ArithParams {
                        simd,
                        threads,
                        cancel: opts.cancel,
                        index: ix,
                        want_index: cx.want_index,
                        no_output: false,
                    };
                    let mut out: Vec<&mut [u8]> =
                        planes.iter_mut().map(|v| v.as_mut_slice()).collect();
                    let out = arith::decode(p, data, &g, &mut out, &ap)?;
                    Ok((
                        out.truncated || !p.saw_eoi,
                        out.index.map(|v| RowIndex::new(p, fp, IndexKind::Arith(v))),
                    ))
                }
                Route::Fallback | Route::Hier => unreachable!(),
            }
        },
    )?;
    if truncated {
        tracing::debug!(target: "jpeg", eoi = p.saw_eoi, "data ends early: partial image");
    }
    Ok((finish(p, &g, planes, truncated, cx.raw), new_index))
}

/// Coefficient memory budget of one stripe of a progressive decode with an index. About the
/// size of a desktop L3 cache: smaller stripes add per-stripe overhead, larger ones were no
/// faster (34-100 MP files, 16 threads) and cost proportionally more memory.
pub(crate) const STRIPE_COEF_BYTES: u64 = 16 << 20;

/// MCU rows per stripe of a progressive decode with an index of `g`'s rows (`u32::MAX`: no
/// striping: the rows fit the budget, only DC coefficients are kept, or the region is narrow;
/// coefficient storage is full-width, but a narrow region only touches the pages of its own
/// columns, see [`crate::cols`]).
pub(crate) fn stripe_rows(f: &marker::Frame, g: &OutGeom) -> u32 {
    if !f.progressive || g.n == 1 || (g.mx1 - g.mx0) * 2 < f.mcus_x {
        return u32::MAX;
    }
    let row: u64 = f
        .comps
        .iter()
        .map(|c| c.v as u64 * c.bw_pad as u64 * memory::COEF_BLOCK)
        .sum();
    let rows = g.my1 - g.my0;
    if row * rows as u64 <= STRIPE_COEF_BYTES {
        return u32::MAX;
    }
    // Equal stripes (no short last one), each within the budget.
    let per = (STRIPE_COEF_BYTES / row.max(1)).max(1) as u32;
    rows.div_ceil(rows.div_ceil(per))
}

/// A coefficient-path decode with a row index. Progressive files buffer every coefficient of
/// the decoded MCU rows (full width) until their last scan; with an index each scan can start
/// at any row, so big decodes run as horizontal stripes of bounded coefficient memory
/// ([`STRIPE_COEF_BYTES`]), each decoded in parallel. Full-width stripes have the same plane
/// stride as the whole decode, so every stripe is one contiguous run of rows per plane and is
/// decoded straight into it.
fn coef_striped(
    p: &Parsed,
    data: &[u8],
    g: &OutGeom,
    scale: Scale,
    planes: &mut [&mut [u8]],
    cp: &coef::CoefParams,
) -> Result<coef::CoefOutcome> {
    let f = &p.frame;
    let per = stripe_rows(f, g);
    if cp.want_index || per >= g.my1 - g.my0 {
        return coef::decode(p, data, g, planes, cp);
    }
    let mcu_h = f.max_v * g.n as u32;
    let out_h = f.height.div_ceil(scale.factor());
    tracing::debug!(target: "jpeg", rows = g.my1 - g.my0, per, "progressive with index: striped");
    let reuse = std::sync::Mutex::new(Vec::new());
    let cps = coef::CoefParams {
        reuse: Some(&reuse),
        ..*cp
    };
    let mut truncated = false;
    let mut m = g.my0;
    while m < g.my1 {
        let m1 = (m + per).min(g.my1);
        let y = m * mcu_h;
        let r = Region {
            x: g.origin.0,
            y,
            w: g.width,
            h: (m1 * mcu_h).min(out_h) - y,
        };
        let gs = OutGeom::new(f, scale, Some(r))?;
        debug_assert!(gs.my0 == m && gs.my1 == m1 && gs.mx0 == g.mx0 && gs.mx1 == g.mx1);
        let mut sp: Vec<&mut [u8]> = Vec::with_capacity(gs.comps.len());
        for (ci, pl) in planes.iter_mut().enumerate() {
            let rb = g.mcu_row_bytes(ci);
            debug_assert_eq!(rb, gs.mcu_row_bytes(ci));
            let off = (m - g.my0) as usize * rb;
            let end = (off + (m1 - m) as usize * rb).min(pl.len());
            sp.push(&mut pl[off.min(end)..end]);
        }
        let out = coef::decode(p, data, &gs, &mut sp, &cps)?;
        truncated |= out.truncated;
        m = m1;
    }
    Ok(coef::CoefOutcome {
        truncated,
        index: None,
    })
}

fn finish(
    p: &Parsed,
    g: &OutGeom,
    planes: Vec<Vec<u8>>,
    truncated: bool,
    raw: bool,
) -> DecodedPlanes {
    let out: Vec<Plane> = planes
        .into_iter()
        .zip(&g.comps)
        .map(|(mut d, c)| {
            d.truncate(c.stride * c.h as usize);
            Plane {
                width: c.w,
                height: c.h,
                stride: c.stride,
                data: d,
                h_samp: c.h_samp as u8,
                v_samp: c.v_samp as u8,
            }
        })
        .collect();
    finish_planes(p, g.width, g.height, g.origin, out, truncated, raw)
}

/// [`DecodedPlanes`] from component planes: CMYK / YCCK converted to RGBA unless `raw`.
fn finish_planes(
    p: &Parsed,
    width: u32,
    height: u32,
    origin: (u32, u32),
    mut out: Vec<Plane>,
    truncated: bool,
    raw: bool,
) -> DecodedPlanes {
    let mut layout = layout_for(p.info.color);
    if layout == PlanesLayout::Rgba && raw {
        // Four components as stored: the caller says what they are.
        layout = PlanesLayout::Rgb;
    } else if layout == PlanesLayout::Rgba {
        let ycck = p.info.color == ColorModel::Ycck;
        let icc = p.info.icc_profile.as_deref();
        let rgba = color::cmyk_to_rgba(&out, width, height, ycck, icc);
        out = vec![rgba];
    }
    if p.frame.comps.len() == 1 {
        layout = PlanesLayout::Gray;
    }
    DecodedPlanes {
        width,
        height,
        origin,
        layout,
        planes: out,
        truncated,
    }
}

/// Decode `data` according to `opts`.
pub fn decode(data: &[u8], opts: &DecodeOptions) -> Result<DecodedPlanes> {
    logged(
        "decode",
        decode_impl(
            data,
            opts,
            None,
            Ctx {
                want_index: false,
                raw: false,
            },
        )
        .map(|r| r.0),
    )
}

/// [`decode`] without the colour model's conversion of four-component (CMYK / YCCK) images:
/// their four planes come back as stored, with [`PlanesLayout::Rgb`] (the caller sets the
/// layout it knows, e.g. TIFF's RGB or YCbCr plus alpha; [`planes_to_rgba_into`] takes a
/// fourth plane as alpha). Other images decode as with [`decode`].
pub fn decode_components(data: &[u8], opts: &DecodeOptions) -> Result<DecodedPlanes> {
    logged(
        "decode",
        decode_impl(
            data,
            opts,
            None,
            Ctx {
                want_index: false,
                raw: true,
            },
        )
        .map(|r| r.0),
    )
}

/// Decode and also return a [`RowIndex`] for later region decodes. For restart-interval files
/// the index is derived from the marker positions; otherwise it is recorded while decoding (a
/// region decode then also parses the rows below the region).
pub fn decode_and_index(data: &[u8], opts: &DecodeOptions) -> Result<(DecodedPlanes, RowIndex)> {
    let cx = Ctx {
        want_index: true,
        raw: false,
    };
    let r = decode_impl(data, opts, None, cx).and_then(|(d, ix)| match ix {
        Some(ix) => Ok((d, ix)),
        None if d.truncated => Err(JpegError::Truncated("cannot index truncated data")),
        None => Err(JpegError::Unsupported(
            "row index not available for this coding",
        )),
    });
    logged("decode_and_index", r)
}

/// Build a [`RowIndex`] (one checkpoint per MCU row, per scan for progressive files) without
/// producing pixels. Uses the restart markers if present (fast, parallel scan for markers),
/// otherwise parses all entropy-coded data.
pub fn build_index(data: &[u8]) -> Result<RowIndex> {
    build_index_with(data, &DecodeOptions::default())
}

/// [`build_index`] honouring `opts.cancel` and `opts.parallelism` (scale/region are ignored).
pub fn build_index_with(data: &[u8], opts: &DecodeOptions) -> Result<RowIndex> {
    let t0 = std::time::Instant::now();
    let r = logged("build_index", build_index_impl(data, opts));
    if let Ok(ix) = &r {
        tracing::debug!(
            target: "jpeg",
            bytes = ix.memory_bytes(),
            checkpoints = ix.len(),
            ms = t0.elapsed().as_secs_f64() * 1e3,
            "index built"
        );
    }
    r
}

fn build_index_impl(data: &[u8], opts: &DecodeOptions) -> Result<RowIndex> {
    if opts
        .cancel
        .is_some_and(|c| c.load(std::sync::atomic::Ordering::Relaxed))
    {
        return Err(JpegError::Cancelled);
    }
    let mut parsed = marker::parse(data, false)?;
    let mut r = route(&parsed)?;
    if !matches!(r, Route::Streaming) {
        parsed = marker::reparse(data)?;
        r = route(&parsed)?;
    }
    let p = &parsed;
    log_route(p, &r);
    tracing::debug!(target: "jpeg", parallelism = ?opts.parallelism, "index request");
    let fp = fingerprint(data, &p.info);
    let simd = Simd::detect();
    let pixels = p.frame.width as u64 * p.frame.height as u64;
    match r {
        Route::Hier => Ok(RowIndex::new(p, fp, IndexKind::Hier)),
        Route::Fallback => Err(JpegError::Unsupported(
            "row index not available for lossless",
        )),
        Route::Streaming => {
            let sc = sequential::SeqScan::new(p, data)?;
            let v = with_threads(opts.parallelism, pixels, |threads| {
                let sp = sequential::SeqParams {
                    simd,
                    threads,
                    cancel: opts.cancel,
                    index: None,
                    want_index: true,
                    cols: None,
                };
                sequential::build_index(&sc, &sp)
            })?;
            if v.len() != sc.mcus_y as usize {
                return Err(JpegError::Truncated("cannot index truncated data"));
            }
            Ok(RowIndex::new(p, fp, IndexKind::Seq(v)))
        }
        Route::Coef => {
            let g = OutGeom::new(&p.frame, Scale::Eighth, None)?;
            guard_size(p, data, &g)?;
            let out = with_threads(opts.parallelism, pixels, |threads| {
                let cp = coef::CoefParams {
                    simd,
                    threads,
                    cancel: opts.cancel,
                    index: None,
                    want_index: true,
                    cols: None,
                    no_output: true,
                    reuse: None,
                };
                coef::decode(p, data, &g, &mut [], &cp)
            })?;
            out.index
                .map(|v| RowIndex::new(p, fp, IndexKind::Coef(v)))
                .ok_or(JpegError::Truncated("cannot index truncated data"))
        }
        Route::Arith => {
            let g = OutGeom::new(&p.frame, Scale::Eighth, None)?;
            guard_size(p, data, &g)?;
            let ap = arith::ArithParams {
                simd,
                threads: 1,
                cancel: opts.cancel,
                index: None,
                want_index: true,
                no_output: true,
            };
            let out = arith::decode(p, data, &g, &mut [], &ap)?;
            out.index
                .map(|v| RowIndex::new(p, fp, IndexKind::Arith(v)))
                .ok_or(JpegError::Truncated("cannot index truncated data"))
        }
    }
}

/// Decode using a [`RowIndex`] built from the same `data`: rows are decoded in parallel from
/// their checkpoints, and with a region only the needed rows are entropy-decoded.
pub fn decode_with_index(
    data: &[u8],
    index: &RowIndex,
    opts: &DecodeOptions,
) -> Result<DecodedPlanes> {
    logged(
        "decode_with_index",
        decode_impl(
            data,
            opts,
            Some(index),
            Ctx {
                want_index: false,
                raw: false,
            },
        )
        .map(|r| r.0),
    )
}
