//! HEIC/HEIF in-process through the pure-Rust `specular-heic`: HEVC-coded images, and HEIF
//! images whose items are AVC, JPEG or uncompressed (`unci`), tiled (`tili`) or colour
//! format enhancement (`cfen`) images.
//!
//! - **HEVC.** Each coded picture (a grid tile, the alpha plane, a thumbnail) goes through the
//!   process's one [`specular_hwdec::heic::shared`] core: VA-API when the path selector
//!   (`Task::HeicDecode`) chooses the hardware for the image, else the software core
//!   (`specular_heic::DEFAULT_CORE`, heic-rs with Oxideav as its fallback). Pictures the
//!   hardware cannot take (mixed luma/chroma depths, over-size tiles) are decoded in software.
//! - **Levels and regions.** The decoded YCbCr image is kept by the session (one decode per
//!   session, single-flight). Each request converts only what it needs: a box-downscaled
//!   level (`1/scale`, averaged in YCbCr before conversion) and/or a window of it, so the
//!   coarse overview and visible-region requests cost a fraction of a full conversion.
//! - **Preview.** The file's `thmb` item (most phones store a ~320 px one) is decoded on its
//!   own as the embedded preview: the first pixels without decoding the primary image.
//! - **Colour.** The YCbCr planes themselves ([`PixelLayout::Yuv`]: the `nclx` / VUI matrix and
//!   range, the samples' bits, the chroma siting, an alpha plane), converted where they are
//!   shown with chroma interpolated at its siting; `specular_core::yuv::to_rgb` is the CPU
//!   path, to float rounding the same. PQ/HLG (`nclx` transfer 16/18) above 8 bits is labelled
//!   with its transfer and primaries. The ICC profile is passed on. Monochrome images come as
//!   RGBA (8 or 16 bits), pre-multiplied alpha as un-premultiplied RGBA from the CPU path.
//! - **Fallback.** Files `specular-heic` rejects (unsupported features, malformed streams) are
//!   handed to libheif in the sandboxed decode host (`specular-sandbox`, DESIGN.md §13) when
//!   it is installed and the fallback is on ([`set_libheif_fallback`],
//!   `SPECULAR_HEIC_LIBHEIF_FALLBACK=0` turns it off). Cancellation and size limits are never
//!   retried there.
//!
//! - **Collections.** The top-level images of a collection (not thumbnails, auxiliary images,
//!   grid tiles or gain maps) are pages, the primary first; each opens from the parsed
//!   container. libheif decodes only the primary image, so later pages have no fallback.
//!
//! - **Sequences.** A `.heics` image sequence (`moov` with an HEVC picture track) shows its
//!   primary item, or its first sample when it has none, and plays as an animation through
//!   [`Session::frames`] with the track's frame times. A track whose samples are all sync
//!   samples (each an intra picture, as `heif-enc -S` writes) decodes one sample at a time
//!   like a still; an inter-coded track (P and B pictures) goes through the HEVC video
//!   decoder ([`specular_heic::seq::SeqPlayer`]), pictures in display order.
//!
//! `SPECULAR_HEIC_HW=0` forces the software core, `=1` the hardware whenever it is usable
//! (benchmarks); unset, the path selector decides.

use crate::gainmap;
use crate::whole::{self, Levels};
use crate::{
    Alpha, AnimationInfo, DecodeError, DecodeRequest, Decoder, Format, Frame, Frames, ImageInfo,
    PageLabel, Result, Session,
};
use parking_lot::Mutex;
use specular_core::{
    CancelToken, GainMap, HdrEncoding, Orientation, PixelLayout, Primaries, Raster, Source,
    Transfer,
};
use specular_heic::seq::Sequence;
use specular_heic::{
    ConvertOptions, DecodeOptions, Error as HeicError, GainMapItem, GainMapKind, HeicFile,
    HevcCore, Image, Layout,
};
use specular_sandbox::{DecodedImage, Error as HostError, ErrorKind, Host, ImageLimits};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, OnceLock};

pub struct HeicDecoder;

/// Largest image opened, in pixels and per side (as the libheif host).
const MAX_PIXELS: u64 = 400_000_000;
const MAX_SIDE: u32 = 65_535;

pub(crate) fn too_large(w: u32, h: u32) -> bool {
    u64::from(w) * u64::from(h) > MAX_PIXELS || w > MAX_SIDE || h > MAX_SIDE
}

/// HEVC-coded HEIF brands.
const HEVC_BRANDS: &[&[u8; 4]] = &[
    b"heic", b"heix", b"heim", b"heis", b"hevc", b"hevx", b"hevm", b"hevs",
];
/// HEIF brands of the other codecs `specular-heic` reads: AVC, JPEG, VVC, and the generic
/// `heif` / uncompressed `unif` brands.
const OTHER_BRANDS: &[&[u8; 4]] = &[
    b"avci", b"avcs", b"jpeg", b"jpgs", b"vvic", b"vvis", b"unif", b"heif",
];
/// AV1-coded (AVIF), handled elsewhere.
const AV1_BRANDS: &[&[u8; 4]] = &[b"avif", b"avis"];

/// `....ftyp` with an HEVC brand (major, or compatible next to a generic `mif1`/`msf1`), or
/// another HEIF codec's brand ([`OTHER_BRANDS`]), and not AVIF.
fn is_heic(head: &[u8]) -> bool {
    if head.len() < 16 || &head[4..8] != b"ftyp" {
        return false;
    }
    let size = u32::from_be_bytes([head[0], head[1], head[2], head[3]]) as usize;
    let end = size.clamp(16, head.len().max(16)).min(head.len());
    let major = &head[8..12];
    let compatible = head[16..end].chunks_exact(4);
    let is = |set: &[&[u8; 4]], b: &[u8]| set.iter().any(|s| s.as_slice() == b);
    if is(HEVC_BRANDS, major) || (is(OTHER_BRANDS, major) && major != b"heif") {
        return true;
    }
    if major != b"mif1" && major != b"msf1" {
        return false;
    }
    let mut hevc = false;
    for b in compatible {
        if is(AV1_BRANDS, b) {
            return false;
        }
        hevc |= is(HEVC_BRANDS, b) || is(OTHER_BRANDS, b);
    }
    hevc
}

/// 0: not decided (environment), 1: on, 2: off.
static LIBHEIF_FALLBACK: AtomicU8 = AtomicU8::new(0);

/// Whether files the in-process decoder rejects are retried with libheif in the sandboxed
/// host (default on; `SPECULAR_HEIC_LIBHEIF_FALLBACK=0` turns it off at start).
pub fn set_libheif_fallback(on: bool) {
    LIBHEIF_FALLBACK.store(if on { 1 } else { 2 }, Ordering::Relaxed);
}

fn libheif_fallback() -> bool {
    match LIBHEIF_FALLBACK.load(Ordering::Relaxed) {
        1 => true,
        2 => false,
        _ => std::env::var_os("SPECULAR_HEIC_LIBHEIF_FALLBACK").is_none_or(|v| v != "0"),
    }
}

/// The HEVC core for every HEIC decode in the process (see the module docs).
fn core() -> &'static dyn HevcCore {
    static FORCED: OnceLock<Option<Box<dyn HevcCore>>> = OnceLock::new();
    let forced = FORCED.get_or_init(|| match std::env::var("SPECULAR_HEIC_HW").as_deref() {
        Ok("0") => Some(Box::new(specular_heic::DEFAULT_CORE)),
        Ok("1") => Some(Box::new(
            specular_hwdec::heic::VaapiCore::new()
                .with_policy(specular_hwdec::heic::Policy::Always),
        )),
        _ => None,
    });
    match forced {
        Some(c) => c.as_ref(),
        None => specular_hwdec::heic::shared(),
    }
}

fn map_heic(e: HeicError) -> DecodeError {
    match e {
        HeicError::Unsupported(m) => DecodeError::Unsupported(m),
        HeicError::Limit(_) => DecodeError::TooLarge,
        HeicError::NeedsMemory(n) => DecodeError::NeedsMemory(n),
        HeicError::Cancelled => DecodeError::Cancelled,
        e @ (HeicError::Heif(_) | HeicError::Hevc { .. }) => DecodeError::Malformed(e.to_string()),
    }
}

/// Whether libheif may succeed where `specular-heic` failed (not for cancellation, limits or
/// memory).
fn rejected(e: &HeicError) -> bool {
    !matches!(
        e,
        HeicError::Cancelled | HeicError::Limit(_) | HeicError::NeedsMemory(_)
    ) && libheif_fallback()
}

fn map_err(e: HostError) -> DecodeError {
    match e {
        HostError::NotInstalled(m) => {
            DecodeError::Unsupported(format!("HEIC support not installed: {m}"))
        }
        HostError::Unavailable(m) => {
            DecodeError::Unsupported(format!("HEIC support unavailable: {m}"))
        }
        HostError::Remote { kind, message } => match kind {
            ErrorKind::NotInstalled | ErrorKind::Unsupported | ErrorKind::Encrypted => {
                DecodeError::Unsupported(message)
            }
            ErrorKind::TooLarge => DecodeError::TooLarge,
            ErrorKind::Io => DecodeError::Io(std::io::Error::other(message)),
            _ => DecodeError::Malformed(message),
        },
        // A crash or a hang on a file is that file's fault as far as the viewer is concerned.
        e @ (HostError::Crashed(_) | HostError::Timeout(_) | HostError::Protocol(_)) => {
            DecodeError::Malformed(e.to_string())
        }
        HostError::Io(m) => DecodeError::Io(std::io::Error::other(m)),
    }
}

fn decode_in_host(source: &Source) -> Result<DecodedImage> {
    let t0 = std::time::Instant::now();
    let img = Host::global()
        .heif_decode(source.bytes(), ImageLimits::default())
        .map_err(map_err)?;
    specular_log::debug!(
        target: specular_log::DECODE,
        "heic: {}×{} decoded by libheif in the host in {:.1} ms",
        img.width,
        img.height,
        t0.elapsed().as_secs_f64() * 1e3
    );
    Ok(img)
}

fn host_raster(img: DecodedImage) -> Raster {
    whole::interleaved(img.width, img.height, PixelLayout::Rgba, img.rgba, false)
}

/// The libheif session of a file the in-process decoder rejected at open.
fn open_in_host(source: Arc<Source>, why: &HeicError) -> Result<Arc<dyn Session>> {
    specular_log::debug!(target: specular_log::DECODE, "heic: {why}; trying libheif");
    let mut img = decode_in_host(&source)?;
    let icc = std::mem::take(&mut img.icc);
    let info = ImageInfo {
        format: Format::Heic,
        width: img.width,
        height: img.height,
        // libheif applies the HEIF transformations; EXIF orientation is informational
        // in HEIF and must not be applied again.
        orientation: Orientation::NORMAL,
        icc_profile: (!icc.is_empty()).then(|| Arc::new(icc)),
        has_alpha: img.has_alpha,
        bit_depth: img.bit_depth,
        progressive: false,
        coding: format!(
            "{}, {}-bit{} (libheif, sandboxed)",
            specular_heic::header_coding(source.bytes())
                .and_then(|h| h.codec)
                .map_or("HEVC", |c| c.name()),
            img.bit_depth,
            if img.has_alpha { " + alpha" } else { "" }
        ),
        embedded_preview: None,
        fast_regions: false,
        scales: &[1, 2, 4, 8],
    };
    let first = host_raster(img);
    let levels = Levels::new();
    let header = specular_heic::header_coding(source.bytes());
    let s = HostSession {
        source,
        info,
        header,
        first: Mutex::new(Some(first)),
        levels,
    };
    Ok(Arc::new(s))
}

/// HDR labelling of high-bit-depth PQ/HLG images (as PNG `cICP`).
fn hdr_encoding(file: &HeicFile) -> Option<HdrEncoding> {
    let info = file.info();
    if info.bit_depth <= 8 {
        return None;
    }
    let (primaries, transfer) = match info.colour.nclx {
        Some(c) => (c.primaries, c.transfer),
        None => {
            let (p, t, _) = info.colour.vui?.description?;
            (u16::from(p), u16::from(t))
        }
    };
    let transfer = match transfer {
        16 => Transfer::Pq,
        18 => Transfer::Hlg,
        _ => return None,
    };
    let primaries = match primaries {
        1 => Primaries::Bt709,
        12 => Primaries::DisplayP3,
        _ => Primaries::Bt2020,
    };
    Some(HdrEncoding {
        transfer,
        primaries,
    })
}

impl Decoder for HeicDecoder {
    fn format(&self) -> Format {
        Format::Heic
    }
    fn probe(&self, head: &[u8]) -> bool {
        is_heic(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["heic", "heif", "hif", "heics"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let seq = specular_heic::seq::find_moov(source.bytes())
            .and_then(Sequence::parse)
            .map(SeqTrack::new);
        let file = match HeicFile::open(source.bytes()) {
            Ok(f) => f,
            // A sequence without a primary item: its first sample.
            Err(e) if seq.is_some() => {
                let first = seq.as_ref().and_then(|s| s.sample_file(source.bytes(), 0));
                match first.map(HeicFile::open) {
                    Some(Ok(f)) => f,
                    Some(Err(e)) => return Err(map_heic(e)),
                    None => return Err(map_heic(e)),
                }
            }
            // libheif in the host; when it cannot either, our refusal names the reason.
            Err(e) if rejected(&e) => {
                return match open_in_host(source, &e) {
                    Err(h)
                        if !matches!(h, DecodeError::Cancelled)
                            && matches!(e, HeicError::Unsupported(_)) =>
                    {
                        Err(map_heic(e))
                    }
                    r => r,
                };
            }
            Err(e) => return Err(map_heic(e)),
        };
        Ok(Arc::new(HeicSession::new(source, file, seq)?))
    }
}

impl HeicSession {
    fn new(source: Arc<Source>, file: HeicFile, seq: Option<Arc<SeqTrack>>) -> Result<HeicSession> {
        let i = file.info();
        if too_large(i.width, i.height) {
            return Err(DecodeError::TooLarge);
        }
        let layout = match i.layout {
            Layout::Single => String::new(),
            Layout::Grid { rows, columns, .. } => format!(", grid {columns}×{rows}"),
            Layout::Derived => match file.graph().kind {
                specular_heic::heif::ImageKind::Tiled(_) => ", tiled".into(),
                specular_heic::heif::ImageKind::ColourFormatEnhancement(_) => {
                    ", colour format enhancement".into()
                }
                _ => ", derived".into(),
            },
        };
        let gain_map = file.gain_map();
        let chroma = match i.chroma {
            _ if i.rgb => "RGB",
            specular_heic::heif::Chroma::Mono => "4:0:0",
            specular_heic::heif::Chroma::Yuv420 => "4:2:0",
            specular_heic::heif::Chroma::Yuv422 => "4:2:2",
            specular_heic::heif::Chroma::Yuv444 => "4:4:4",
        };
        let info = ImageInfo {
            format: Format::Heic,
            width: i.width,
            height: i.height,
            // The HEIF transformations (irot, imir, clap) are applied by the decoder; EXIF
            // orientation is informational in HEIF and must not be applied again.
            orientation: Orientation::NORMAL,
            icc_profile: i.colour.icc.clone().map(Arc::new),
            has_alpha: i.has_alpha,
            bit_depth: i.bit_depth,
            progressive: false,
            coding: format!(
                "{} {}-bit {chroma}{layout}{}{}{}{}",
                i.codec.name(),
                i.bit_depth,
                if i.has_alpha { " + alpha" } else { "" },
                match gain_map.as_ref().map(|g| &g.kind) {
                    Some(GainMapKind::Iso { .. }) => ", gain map (ISO 21496-1)",
                    Some(GainMapKind::Apple) => ", gain map (Apple)",
                    None => "",
                },
                match file.page_count() {
                    1 => String::new(),
                    n => format!(", collection of {n}"),
                },
                match seq.as_deref() {
                    None => String::new(),
                    Some(s) if s.seq.all_sync() => {
                        format!(", sequence of {} frames", s.seq.shown_count())
                    }
                    Some(s) =>
                        format!(", sequence of {} frames (inter-coded)", s.seq.shown_count()),
                }
            ),
            embedded_preview: None,
            // Regions are converted from the decoded image on their own.
            fast_regions: true,
            scales: &[1, 2, 4, 8],
        };
        let hdr = hdr_encoding(&file);
        let thumbnail = file.thumbnail();
        Ok(HeicSession {
            source,
            info,
            hdr,
            thumbnail,
            gain_map,
            file,
            image: Mutex::new(None),
            image_work: Mutex::new(()),
            host: OnceLock::new(),
            seq,
        })
    }
}

/// The picture track of a `.heics` sequence.
struct SeqTrack {
    seq: Sequence,
    /// Set when there are several samples.
    animation: Option<AnimationInfo>,
}

impl SeqTrack {
    fn new(seq: Sequence) -> Arc<SeqTrack> {
        let animation = (seq.shown_count() > 1).then(|| AnimationInfo {
            frame_count: u32::try_from(seq.shown_count()).ok(),
            loop_count: 0,
        });
        Arc::new(SeqTrack { seq, animation })
    }

    /// Sample `i` as a one-item HEIF file.
    fn sample_file(&self, file: &[u8], i: usize) -> Option<Vec<u8>> {
        self.seq.sample_heif(file, i)
    }

    fn delay(&self, i: usize) -> std::time::Duration {
        let d = self.seq.samples.get(i).map_or(0, |s| s.duration);
        std::time::Duration::from_secs_f64(f64::from(d) / f64::from(self.seq.timescale.max(1)))
    }
}

/// The samples of an all-intra sequence, one decoded per frame.
struct SeqFrames {
    source: Arc<Source>,
    track: Arc<SeqTrack>,
    scale: u32,
    hdr: Option<HdrEncoding>,
    next: usize,
}

impl SeqFrames {
    fn decode(&self, i: usize, cancel: &CancelToken) -> Result<Frame> {
        let heif = self
            .track
            .sample_file(self.source.bytes(), i)
            .ok_or_else(|| DecodeError::Truncated("sequence sample".into()))?;
        let file = HeicFile::open(heif).map_err(map_heic)?;
        let img = file
            .decode(&DecodeOptions {
                parallel: true,
                cancel: Some(cancel.flag()),
                core: Some(core()),
            })
            .map_err(map_heic)?;
        let raster = seq_raster(&img, self.scale, self.hdr, cancel)?;
        Ok(Frame {
            index: i as u32,
            raster,
            delay: self.track.delay(i),
        })
    }
}

/// A sequence picture at `1/scale`: its planes, or RGBA when it has alpha (animation strips
/// take three planes).
fn seq_raster(
    img: &Image,
    scale: u32,
    hdr: Option<HdrEncoding>,
    cancel: &CancelToken,
) -> Result<Raster> {
    let o = ConvertOptions {
        scale,
        window: None,
        wide: hdr.is_some(),
        parallel: true,
        cancel: Some(cancel.flag()),
    };
    let r = display_raster(img, &o, hdr)?;
    match r.layout {
        PixelLayout::Yuv(e) if e.alpha => specular_core::yuv::to_rgb(&r)
            .ok_or_else(|| DecodeError::Malformed("sequence picture planes".into())),
        _ => Ok(r),
    }
}

/// An image for display at `o.scale` and `o.window`: its YCbCr planes (see the module docs);
/// RGBA for monochrome images and un-premultiplied RGBA for pre-multiplied alpha.
fn display_raster(img: &Image, o: &ConvertOptions<'_>, hdr: Option<HdrEncoding>) -> Result<Raster> {
    let scale = o.scale.max(1);
    if let Some(r) = img
        .to_yuv(o, hdr.unwrap_or(HdrEncoding::SRGB))
        .map_err(map_heic)?
    {
        if !img.premultiplied_alpha || !matches!(r.layout, PixelLayout::Yuv(e) if e.alpha) {
            return Ok(r);
        }
        let mut rgb = specular_core::yuv::to_rgb(&r)
            .ok_or_else(|| DecodeError::Malformed("image planes".into()))?;
        unpremultiply(&mut rgb);
        return Ok(rgb);
    }
    let rgba = img.to_rgba(o).map_err(map_heic)?;
    let layout = match hdr {
        Some(h) => PixelLayout::Rgba16(h),
        None => PixelLayout::Rgba,
    };
    let mut r = whole::interleaved(rgba.width, rgba.height, layout, rgba.data, false);
    r.origin = rgba.origin;
    r.scale = scale;
    Ok(r)
}

/// Divides the colour of an RGBA8 or RGBA16 raster by its alpha (pre-multiplied → straight).
fn unpremultiply(r: &mut Raster) {
    let wide = r.layout != PixelLayout::Rgba;
    let Some(p) = r.planes.first_mut() else {
        return;
    };
    if wide {
        for px in p.data.chunks_exact_mut(8) {
            let a = u32::from(u16::from_ne_bytes([px[6], px[7]]));
            for c in 0..3 {
                let v = u32::from(u16::from_ne_bytes([px[2 * c], px[2 * c + 1]]));
                let s = (v * 65535 + a / 2)
                    .checked_div(a)
                    .map_or(0, |s| s.min(65535));
                px[2 * c..2 * c + 2].copy_from_slice(&(s as u16).to_ne_bytes());
            }
        }
    } else {
        for px in p.data.chunks_exact_mut(4) {
            let a = u32::from(px[3]);
            for c in &mut px[..3] {
                *c = (u32::from(*c) * 255 + a / 2)
                    .checked_div(a)
                    .map_or(0, |s| s.min(255) as u8);
            }
        }
    }
}

impl Frames for SeqFrames {
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame>> {
        // Samples that are never shown are skipped (each decodes on its own here).
        let samples = &self.track.seq.samples;
        while samples.get(self.next).is_some_and(|s| s.hidden) {
            self.next += 1;
        }
        let i = self.next;
        if i >= samples.len() {
            return None;
        }
        self.next += 1;
        let shown = samples[..i].iter().filter(|s| !s.hidden).count();
        Some(self.decode(i, cancel).map(|f| Frame {
            index: shown as u32,
            ..f
        }))
    }
    fn rewind(&mut self) -> Result<()> {
        self.next = 0;
        Ok(())
    }
}

/// The pictures of an inter-coded sequence, through the HEVC video decoder in display order.
struct InterFrames {
    source: Arc<Source>,
    player: specular_heic::seq::SeqPlayer,
    scale: u32,
    hdr: Option<HdrEncoding>,
}

impl Frames for InterFrames {
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame>> {
        let f = match self
            .player
            .next_frame(self.source.bytes(), Some(cancel.flag()))?
        {
            Ok(f) => f,
            Err(e) => return Some(Err(map_heic(e))),
        };
        Some(
            seq_raster(&f.image, self.scale, self.hdr, cancel).map(|raster| Frame {
                index: f.index as u32,
                raster,
                delay: f.delay,
            }),
        )
    }
    fn rewind(&mut self) -> Result<()> {
        self.player.rewind();
        Ok(())
    }
}

struct HeicSession {
    source: Arc<Source>,
    info: ImageInfo,
    hdr: Option<HdrEncoding>,
    /// `(item, width, height)` of the largest thumbnail.
    thumbnail: Option<(u32, u32, u32)>,
    gain_map: Option<GainMapItem>,
    file: HeicFile,
    /// The decoded primary image (YCbCr), decoded once by the first request that needs it.
    /// Held only to clone or swap the `Arc`, so [`Session::memory_bytes`],
    /// [`Session::decode_memory`] and [`Session::trim_memory`] (the app's memory accounting
    /// and pressure handling) never wait for a decode.
    image: Mutex<Option<Arc<Image>>>,
    /// Held while the primary image decodes, so concurrent level requests share one decode.
    image_work: Mutex<()>,
    /// libheif's levels, when the in-process decode of the primary image failed.
    host: OnceLock<Levels>,
    /// The `moov` picture track of an image sequence.
    seq: Option<Arc<SeqTrack>>,
}

impl HeicSession {
    /// The decoded image. `image_work` makes concurrent level requests wait for the one
    /// decode; a cancelled decode is not kept, the next request starts over.
    fn image(
        &self,
        cancel: &CancelToken,
        parallel: bool,
    ) -> std::result::Result<Arc<Image>, HeicError> {
        if let Some(img) = self.image.lock().clone() {
            return Ok(img);
        }
        let _work = self.image_work.lock();
        if let Some(img) = self.image.lock().clone() {
            return Ok(img);
        }
        if cancel.is_cancelled() {
            return Err(HeicError::Cancelled);
        }
        let t0 = std::time::Instant::now();
        let img = Arc::new(self.file.decode(&DecodeOptions {
            parallel,
            cancel: Some(cancel.flag()),
            core: Some(core()),
        })?);
        specular_log::debug!(
            target: specular_log::DECODE,
            "heic: {}×{} decoded in {:.1} ms ({})",
            img.width(),
            img.height(),
            t0.elapsed().as_secs_f64() * 1e3,
            core().name()
        );
        for w in &img.warnings {
            specular_log::debug!(target: specular_log::DECODE, "heic: {w}");
        }
        *self.image.lock() = Some(img.clone());
        Ok(img)
    }

    fn convert(&self, img: &Image, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        let o = ConvertOptions {
            scale: req.scale.max(1),
            window: req.region.map(|r| (r.x, r.y, r.w, r.h)),
            wide: self.hdr.is_some(),
            parallel: req.parallel,
            cancel: Some(cancel.flag()),
        };
        display_raster(img, &o, self.hdr)
    }

    /// Decodes the gain map image and turns its metadata into parameters: an ISO 21496-1
    /// `tmap` (a grid map is assembled like the primary), or an Apple auxiliary whose headroom
    /// is in its XMP or the primary's maker notes.
    fn decode_gain_map(&self, item: &GainMapItem, cancel: &CancelToken) -> Option<GainMap> {
        let params = match &item.kind {
            GainMapKind::Iso {
                metadata,
                alternate,
            } => {
                let base = &self.file.info().colour;
                let prim = |c: &specular_heic::Colour| {
                    c.nclx
                        .map(|n| n.primaries)
                        .or_else(|| c.vui?.description.map(|d| u16::from(d.0)))
                        .and_then(gainmap::cicp_primaries)
                };
                let p = gainmap::tmap_params(metadata, prim(base), prim(alternate))?;
                Some(p)
            }
            GainMapKind::Apple => None,
        };
        let t0 = std::time::Instant::now();
        let img = self
            .file
            .decode_image(
                item.map,
                &DecodeOptions {
                    parallel: true,
                    cancel: Some(cancel.flag()),
                    core: Some(core()),
                },
            )
            .ok()?;
        if img.width() > gainmap::MAX_MAP_SIDE || img.height() > gainmap::MAX_MAP_SIDE {
            return None;
        }
        let rgba = img
            .to_rgba(&ConvertOptions {
                cancel: Some(cancel.flag()),
                ..ConvertOptions::default()
            })
            .ok()?;
        let map = whole::interleaved(rgba.width, rgba.height, PixelLayout::Rgba, rgba.data, false);
        specular_log::debug!(
            target: specular_log::DECODE,
            "heic: gain map item {} {}×{} decoded in {:.1} ms",
            item.map,
            map.width,
            map.height,
            t0.elapsed().as_secs_f64() * 1e3
        );
        match params {
            Some(params) => Some(GainMap {
                map,
                params,
                source: gainmap::ISO_SOURCE,
            }),
            None => {
                let xmp = self.file.item_xmp(item.map);
                let exif = self.file.exif();
                let headroom = gainmap::apple_headroom(xmp.as_deref(), exif.as_deref())?;
                gainmap::apple_gain_map(&map, headroom)
            }
        }
    }

    fn decode_in_host(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        let levels = self.host.get_or_init(Levels::new);
        levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| decode_in_host(&self.source).map(host_raster),
            cancel,
        )
    }
}

impl Session for HeicSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    /// The `nclx` (or the HEVC VUI): PQ / HLG over a profile, its primaries without one.
    fn colour_space(&self) -> Option<&'static str> {
        let c = &self.file.info().colour;
        let (p, t) = match c.nclx {
            Some(n) => (n.primaries, n.transfer),
            None => {
                let (p, t, _) = c.vui?.description?;
                (u16::from(p), u16::from(t))
            }
        };
        if self.info.icc_profile.is_some() && !matches!(t, 16 | 18) {
            return None;
        }
        crate::colour_space::cicp_space(p, t)
    }
    fn colour_model(&self) -> Option<&'static str> {
        let i = self.file.info();
        let matrix = i
            .colour
            .nclx
            .map(|n| n.matrix)
            .or_else(|| i.colour.vui?.description.map(|d| u16::from(d.2)));
        let mono = i.chroma == specular_heic::heif::Chroma::Mono;
        Some(crate::cicp_colour_model(mono, matrix))
    }
    fn chroma_subsampling(&self) -> Option<&'static str> {
        use specular_heic::heif::Chroma;
        if !matches!(self.colour_model(), Some("YCbCr" | "YCgCo")) {
            return None;
        }
        Some(match self.file.info().chroma {
            Chroma::Mono => return None,
            Chroma::Yuv420 => "4:2:0",
            Chroma::Yuv422 => "4:2:2",
            Chroma::Yuv444 => "4:4:4",
        })
    }
    /// The alpha auxiliary image; premultiplied with a `prem` reference.
    fn alpha(&self) -> Option<Alpha> {
        let i = self.file.info();
        i.has_alpha.then_some(if i.premultiplied_alpha {
            Alpha::Premultiplied
        } else {
            Alpha::Straight
        })
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        let (id, ..) = self.thumbnail?;
        let r = self
            .file
            .decode_thumbnail(
                id,
                &DecodeOptions {
                    parallel: false,
                    cancel: None,
                    core: Some(core()),
                },
            )
            .map_err(map_heic)
            .and_then(|img| display_raster(&img, &ConvertOptions::default(), None));
        Some(r)
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        if self.host.get().is_some() {
            return self.decode_in_host(req, cancel);
        }
        match self.image(cancel, req.parallel) {
            Ok(img) => self.convert(&img, req, cancel),
            // libheif decodes the primary image only.
            Err(e) if rejected(&e) && self.file.page() == 0 => {
                specular_log::debug!(target: specular_log::DECODE, "heic: {e}; trying libheif");
                self.decode_in_host(req, cancel)
            }
            Err(e) => Err(map_heic(e)),
        }
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        let s = req.scale.max(1);
        // RGBA, or at most four planes of the samples (4:4:4 with alpha).
        let samples =
            (3 + u64::from(self.info.has_alpha)) * if self.info.bit_depth > 8 { 2 } else { 1 };
        let bpp = samples.max(if self.hdr.is_some() { 8 } else { 4 });
        let out = match req.region {
            Some(r) => u64::from(r.w) * u64::from(r.h) * bpp,
            None => whole::level_bytes(w, h, s, bpp),
        };
        if self.image.lock().is_some() {
            return out;
        }
        // The YCbCr image (luma + chroma + alpha, 1 or 2 bytes a sample) and, while it is
        // assembled, the decoded tiles.
        let sample = if self.info.bit_depth > 8 { 2 } else { 1 };
        let planes = 3 + u64::from(self.info.has_alpha);
        out + 2 * u64::from(w) * u64::from(h) * planes * sample
    }
    fn memory_bytes(&self) -> usize {
        let img = self.image.lock().as_deref().map_or(0, image_bytes);
        img + self.host.get().map_or(0, |l| l.memory_bytes())
    }
    /// Drops the decoded YCbCr image (levels and regions are converted from it) and libheif's
    /// levels. A decode in progress has nothing to drop yet and is not waited for (it keeps
    /// the image it makes).
    fn trim_memory(&self) -> usize {
        let img = self.image.lock().take().as_deref().map_or(0, image_bytes);
        img + self.host.get().map_or(0, Levels::clear)
    }
    fn scaled_decode_cheap(&self) -> bool {
        // The whole HEVC image is decoded at any scale.
        false
    }
    fn has_gain_map(&self) -> bool {
        self.gain_map.is_some()
    }
    fn gain_map(&self, cancel: &CancelToken) -> Option<GainMap> {
        self.decode_gain_map(self.gain_map.as_ref()?, cancel)
    }
    fn page_count(&self) -> u32 {
        self.file.page_count()
    }
    fn page_index(&self) -> u32 {
        self.file.page()
    }
    fn page_label(&self) -> Option<PageLabel> {
        (self.file.page_count() > 1).then_some(PageLabel::Image {
            width: self.info.width,
            height: self.info.height,
            bits: None,
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        let file = self.file.reopen_page(page).map_err(map_heic)?;
        Ok(Arc::new(HeicSession::new(self.source.clone(), file, None)?))
    }
    fn animation(&self) -> Option<&AnimationInfo> {
        self.seq.as_ref()?.animation.as_ref()
    }
    fn frames(&self, scale: u32) -> Result<Box<dyn Frames>> {
        let track = self
            .seq
            .clone()
            .filter(|s| s.animation.is_some())
            .ok_or_else(|| DecodeError::Unsupported("not animated".into()))?;
        if !track.seq.all_sync() {
            let player = specular_heic::seq::SeqPlayer::new(&track.seq, self.source.bytes())
                .map_err(map_heic)?;
            return Ok(Box::new(InterFrames {
                source: self.source.clone(),
                player,
                scale: scale.max(1),
                hdr: self.hdr,
            }));
        }
        Ok(Box::new(SeqFrames {
            source: self.source.clone(),
            track,
            scale: scale.max(1),
            hdr: self.hdr,
            next: 0,
        }))
    }
}

/// Bytes of a decoded YCbCr image's planes.
fn image_bytes(i: &Image) -> usize {
    i.frame.planes.iter().map(|p| p.data.len()).sum()
}

/// A file only libheif reads: decoded whole in the host at open.
struct HostSession {
    source: Arc<Source>,
    info: ImageInfo,
    /// What our container parser reads of the primary image, for the info panel.
    header: Option<specular_heic::HeaderCoding>,
    /// The decode done by `open`, until the level cache takes it.
    first: Mutex<Option<Raster>>,
    levels: Levels,
}

impl Session for HostSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    /// From the container: the `hvcC` chroma format and the `nclx` matrix, when our parser
    /// reads the item graph libheif decodes (another codec gives only the `nclx`).
    fn colour_model(&self) -> Option<&'static str> {
        let h = self.header?;
        let mono = h.chroma == Some(specular_heic::heif::Chroma::Mono);
        (h.chroma.is_some() || h.matrix.is_some()).then(|| crate::cicp_colour_model(mono, h.matrix))
    }
    fn chroma_subsampling(&self) -> Option<&'static str> {
        use specular_heic::heif::Chroma;
        if !matches!(self.colour_model(), Some("YCbCr" | "YCgCo")) {
            return None;
        }
        Some(match self.header?.chroma? {
            Chroma::Mono => return None,
            Chroma::Yuv420 => "4:2:0",
            Chroma::Yuv422 => "4:2:2",
            Chroma::Yuv444 => "4:4:4",
        })
    }
    fn alpha(&self) -> Option<Alpha> {
        let premultiplied = self.header.is_some_and(|h| h.premultiplied_alpha);
        self.info.has_alpha.then_some(if premultiplied {
            Alpha::Premultiplied
        } else {
            Alpha::Straight
        })
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| match self.first.lock().take() {
                Some(r) => Ok(r),
                None => decode_in_host(&self.source).map(host_raster),
            },
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        let s = req.scale.max(1);
        let mut m = whole::level_bytes(w, h, s, 4);
        if !self.levels.has(s) && !self.levels.has(1) {
            // The full image (received from the host) and its copy out of shared memory.
            m += whole::level_bytes(w, h, 1, 8);
        }
        m
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes() + self.first.lock().as_ref().map_or(0, |r| r.memory_bytes())
    }
    /// Drops the levels and the decode done at open (the next request asks libheif again).
    fn trim_memory(&self) -> usize {
        self.levels.clear() + self.first.lock().take().map_or(0, |r| r.memory_bytes())
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
mod tests {
    use super::*;

    const DATA: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../specular-decode-host/tests/data/"
    );

    #[test]
    fn probes_brands() {
        let ftyp = |major: &[u8; 4], compat: &[&[u8; 4]]| {
            let mut v = Vec::new();
            v.extend_from_slice(&((16 + 4 * compat.len()) as u32).to_be_bytes());
            v.extend_from_slice(b"ftyp");
            v.extend_from_slice(major);
            v.extend_from_slice(&[0; 4]);
            for c in compat {
                v.extend_from_slice(*c);
            }
            v.extend_from_slice(&[0, 0, 0, 8, b'm', b'e', b't', b'a']);
            v
        };
        assert!(is_heic(&ftyp(b"heic", &[b"mif1", b"heic"])));
        assert!(is_heic(&ftyp(b"heix", &[])));
        assert!(is_heic(&ftyp(b"mif1", &[b"mif1", b"heic"])));
        assert!(!is_heic(&ftyp(b"mif1", &[b"mif1", b"avif"])));
        assert!(!is_heic(&ftyp(b"mif1", &[b"heic", b"avif"])));
        assert!(!is_heic(&ftyp(b"avif", &[b"mif1", b"miaf"])));
        assert!(!is_heic(&ftyp(b"mif1", &[b"mif1"])));
        // AVC, JPEG, VVC and uncompressed HEIF files.
        assert!(is_heic(&ftyp(b"avci", &[b"mif1", b"avci"])));
        assert!(is_heic(&ftyp(b"mif1", &[b"mif1", b"jpeg"])));
        assert!(is_heic(&ftyp(b"mif1", &[b"mif1", b"heif"])));
        assert!(is_heic(&ftyp(b"msf1", &[b"msf1", b"avcs"])));
        assert!(is_heic(&ftyp(b"vvic", &[b"mif1", b"vvic", b"miaf"])));
        assert!(is_heic(&ftyp(b"mif1", &[b"mif1", b"vvic"])));
        assert!(!is_heic(&ftyp(b"mif1", &[b"jpeg", b"avif"])));
        assert!(!is_heic(&ftyp(b"isom", &[b"heic"])));
        assert!(!is_heic(b"\0\0\0\x18ftyphei"));
        let file = std::fs::read(format!("{DATA}grad.heic")).unwrap();
        assert!(is_heic(&file[..64]));
        assert!(
            crate::Registry::default()
                .probe(&file[..64])
                .is_some_and(|d| d.format() == Format::Heic)
        );
    }

    fn open(name: &str, bytes: Vec<u8>) -> Arc<dyn Session> {
        HeicDecoder
            .open(Arc::new(Source::from_vec(name, bytes)))
            .unwrap()
    }

    /// The RGBA8 bytes of `r`, YCbCr planes through the CPU conversion.
    fn rgba(r: &Raster) -> Vec<u8> {
        match r.layout {
            PixelLayout::Yuv(_) => specular_core::yuv::to_rgb(r).unwrap().planes[0]
                .data
                .clone(),
            _ => r.planes[0].data.clone(),
        }
    }

    /// `specular-heic`'s fixtures: AVC, JPEG, `tili` and `cfen` images, an inter-coded
    /// sequence.
    const HEIF_DATA: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../formats/specular-heic/tests/data/"
    );

    #[test]
    fn other_codecs_tiled_and_enhanced_images() {
        for (name, coding) in [
            ("avc.heif", "AVC 8-bit 4:2:0"),
            ("jpeg.heif", "JPEG 8-bit 4:2:0"),
            ("tili.heif", "HEVC 8-bit 4:2:0, tiled"),
            ("cfen.heif", "HEVC 8-bit 4:4:4, colour format enhancement"),
        ] {
            let file = std::fs::read(format!("{HEIF_DATA}{name}")).unwrap();
            assert!(is_heic(&file[..64]), "{name}");
            let s = open(name, file);
            assert_eq!(s.info().coding, coding, "{name}");
            let r = s
                .decode(&DecodeRequest::full(1), &CancelToken::new())
                .unwrap();
            assert_eq!(
                (r.width, r.height),
                (s.info().width, s.info().height),
                "{name}"
            );
        }
    }

    /// An inter-coded sequence (P and B pictures) plays every picture with the track's times.
    #[test]
    fn inter_coded_sequence_animates() {
        let file = std::fs::read(format!("{HEIF_DATA}inter.heics")).unwrap();
        let s = open("inter.heics", file);
        assert!(
            s.info()
                .coding
                .contains("sequence of 8 frames (inter-coded)"),
            "{}",
            s.info().coding
        );
        assert_eq!(s.animation().unwrap().frame_count, Some(8));
        let mut frames = s.frames(2).unwrap();
        let mut n = 0;
        while let Some(f) = frames.next_frame(&CancelToken::new()) {
            let f = f.unwrap();
            assert_eq!(f.index, n);
            assert_eq!((f.raster.width, f.raster.height), (32, 32));
            assert_eq!(f.delay, std::time::Duration::from_millis(100));
            n += 1;
        }
        assert_eq!(n, 8);
        frames.rewind().unwrap();
        let first = frames.next_frame(&CancelToken::new()).unwrap().unwrap();
        assert_eq!(first.index, 0);
    }

    /// An AVIF-branded file whose image is HEVC-coded opens through the HEIF decoder.
    #[test]
    fn avif_brand_with_hevc_image() {
        let mut file = std::fs::read(format!("{DATA}grad.heic")).unwrap();
        let size = u32::from_be_bytes(file[..4].try_into().unwrap()) as usize;
        for at in (8..size).step_by(4) {
            if &file[at..at + 4] == b"heic" {
                file[at..at + 4].copy_from_slice(b"avif");
            }
        }
        assert!(specular_avif::probe(&file[..64]));
        let s = crate::avif_dec::AvifDecoder
            .open(Arc::new(Source::from_vec("x.avif", file)))
            .unwrap();
        assert!(s.info().coding.starts_with("HEVC"), "{}", s.info().coding);
    }

    /// The images of a collection are pages, each with its own size, alpha and preview.
    #[test]
    fn collection_pages() {
        let s = open(
            "collection.heic",
            std::fs::read(format!("{DATA}collection.heic")).unwrap(),
        );
        assert_eq!((s.page_count(), s.page_index()), (3, 0));
        let size = |width, height| {
            Some(PageLabel::Image {
                width,
                height,
                bits: None,
            })
        };
        assert_eq!(s.page_label(), size(32, 24));
        assert!(
            s.info().coding.ends_with(", collection of 3"),
            "{}",
            s.info().coding
        );
        let p = s.open_page(2).unwrap();
        assert_eq!((p.page_index(), p.page_count()), (2, 3));
        assert_eq!(p.page_label(), size(40, 20));
        assert!(p.info().has_alpha);
        let preview = p.decode_preview().unwrap().unwrap();
        assert_eq!((preview.width, preview.height), (8, 4));
        let r = p
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert_eq!((r.width, r.height), (40, 20));
        assert_eq!(p.open_page(1).unwrap().info().width, 16);
        assert!(s.open_page(3).is_err());
        let one = open(
            "alpha.heic",
            std::fs::read(format!("{DATA}alpha.heic")).unwrap(),
        );
        assert_eq!((one.page_count(), one.page_label()), (1, None));
        assert!(one.open_page(1).is_err());
    }

    /// In-process: levels, regions (exact windows of the level), alpha, no libheif.
    #[test]
    fn decodes_in_process_with_levels_and_regions() {
        let s = open(
            "alpha.heic",
            std::fs::read(format!("{DATA}alpha.heic")).unwrap(),
        );
        let i = s.info();
        assert_eq!((i.width, i.height, i.has_alpha), (40, 30, true));
        assert!(!i.coding.contains("libheif"), "{}", i.coding);
        let c = CancelToken::new();
        let full = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!((full.width, full.height), (40, 30));
        assert!(
            matches!(full.layout, PixelLayout::Yuv(e) if e.alpha && e.bits == 8),
            "{:?}",
            full.layout
        );
        assert!(rgba(&full).chunks_exact(4).any(|p| p[3] < 255));
        let half = s.decode(&DecodeRequest::full(2), &c).unwrap();
        assert_eq!((half.width, half.height, half.scale), (20, 15, 2));
        let mut req = DecodeRequest::full(1);
        req.region = Some(crate::Rect {
            x: 10,
            y: 5,
            w: 8,
            h: 4,
        });
        // The window widened by a chroma sample each side: its pixels convert exactly like
        // the whole image's.
        let r = s.decode(&req, &c).unwrap();
        let (ox, oy) = (r.origin.0 as usize, r.origin.1 as usize);
        assert!(ox <= 10 && oy <= 5 && ox + r.width as usize >= 18 && oy + r.height as usize >= 9);
        let (part, whole) = (rgba(&r), rgba(&full));
        for y in 0..4 {
            let a = ((5 + y - oy) * r.width as usize + 10 - ox) * 4;
            let o = ((5 + y) * 40 + 10) * 4;
            assert_eq!(&part[a..a + 32], &whole[o..o + 32]);
        }
        assert!(s.memory_bytes() > 0);
        assert!(s.decode_memory(&DecodeRequest::full(1)) >= 40 * 30 * 4);
    }

    /// The calibration sample: an 8×… grid with a thumbnail, decoded as preview, a 1/8 level
    /// and full size; a raised cancel flag stops the decode and is not cached.
    #[test]
    fn grid_preview_and_cancel() {
        let s = open("calib.heic", crate::calibrate::HEIC_SAMPLE.to_vec());
        assert_eq!((s.info().width, s.info().height), (2048, 1024));
        let p = s.decode_preview().expect("thumbnail").unwrap();
        assert_eq!((p.width, p.height), (256, 128));
        let cancelled = CancelToken::new();
        cancelled.cancel();
        assert!(matches!(
            s.decode(&DecodeRequest::full(1), &cancelled),
            Err(DecodeError::Cancelled)
        ));
        assert_eq!(s.memory_bytes(), 0);
        let c = CancelToken::new();
        let eighth = s.decode(&DecodeRequest::full(8), &c).unwrap();
        assert_eq!((eighth.width, eighth.height), (256, 128));
        // The preview looks like the image.
        let (p, eighth) = (rgba(&p), rgba(&eighth));
        let diff = p
            .iter()
            .zip(&eighth)
            .map(|(&a, &b)| u64::from(a.abs_diff(b)))
            .sum::<u64>()
            / p.len() as u64;
        assert!(diff < 12, "mean preview difference {diff}");
        let full = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!((full.width, full.height), (2048, 1024));
    }

    /// `seq.heics`: three intra 32×24 frames at 10 per second (`heif-enc -S
    /// --max-keyframe-distance 1`), with a primary item; stripped of its `meta`, the first
    /// sample is shown.
    #[test]
    fn image_sequence() {
        let file = std::fs::read(format!("{DATA}seq.heics")).unwrap();
        assert!(is_heic(&file[..64]));
        for bytes in [file.clone(), {
            let mut f = file.clone();
            let at = f.windows(4).position(|w| w == b"meta").unwrap();
            f[at..at + 4].copy_from_slice(b"free");
            f
        }] {
            let s = open("seq.heics", bytes);
            assert_eq!((s.info().width, s.info().height), (32, 24));
            assert!(
                s.info().coding.contains("sequence of 3 frames"),
                "{}",
                s.info().coding
            );
            let a = s.animation().unwrap();
            assert_eq!((a.frame_count, a.loop_count), (Some(3), 0));
            let still = s
                .decode(&DecodeRequest::full(1), &CancelToken::new())
                .unwrap();
            let mut frames = s.frames(1).unwrap();
            let mut n = 0;
            while let Some(f) = frames.next_frame(&CancelToken::new()) {
                let f = f.unwrap();
                assert_eq!((f.raster.width, f.raster.height), (32, 24));
                assert_eq!(f.delay, std::time::Duration::from_millis(100));
                if n == 0 {
                    assert_eq!(rgba(&f.raster), rgba(&still));
                }
                n += 1;
            }
            assert_eq!(n, 3);
            frames.rewind().unwrap();
            assert_eq!(
                frames
                    .next_frame(&CancelToken::new())
                    .unwrap()
                    .unwrap()
                    .index,
                0
            );
        }
        // Truncated anywhere: an error or fewer frames, never a panic.
        for n in (0..file.len()).step_by(7) {
            let src = Arc::new(Source::from_vec("t.heics", file[..n].to_vec()));
            if let Ok(s) = HeicDecoder.open(src) {
                let _ = s.decode(&DecodeRequest::full(1), &CancelToken::new());
                if let Ok(mut f) = s.frames(2) {
                    while let Some(r) = f.next_frame(&CancelToken::new()) {
                        let _ = r;
                    }
                }
            }
        }
    }

    #[test]
    fn size_limits() {
        assert!(!too_large(20_000, 20_000));
        assert!(too_large(20_001, 20_000));
        assert!(too_large(65_536, 16));
        assert!(too_large(16, 65_536));
    }

    /// Trimming drops the decoded YCbCr image; the next request decodes it again.
    #[test]
    fn trims_decoded_image() {
        let s = open(
            "alpha.heic",
            std::fs::read(format!("{DATA}alpha.heic")).unwrap(),
        );
        crate::trim_check::assert_trims(&*s, &DecodeRequest::full(2));
    }

    /// The memory accounting and a trim do not wait for a decode in progress (one holding
    /// `image_work`): the app calls them from its UI thread under memory pressure.
    #[test]
    fn accounting_does_not_wait_for_a_decode() {
        let bytes = std::fs::read(format!("{DATA}alpha.heic")).unwrap();
        let file = HeicFile::open(bytes.clone()).unwrap();
        let s = Arc::new(
            HeicSession::new(Arc::new(Source::from_vec("a.heic", bytes)), file, None).unwrap(),
        );
        let c = CancelToken::new();
        s.decode(&DecodeRequest::full(1), &c).unwrap();
        let held = s.memory_bytes();
        assert!(held > 0);
        let work = s.image_work.lock();
        let (tx, rx) = std::sync::mpsc::channel();
        let t = {
            let s = s.clone();
            std::thread::spawn(move || {
                let m = s.memory_bytes();
                let d = s.decode_memory(&DecodeRequest::full(1));
                let t = s.trim_memory();
                let after = (s.memory_bytes(), s.decode_memory(&DecodeRequest::full(1)));
                tx.send((m, d, t, after)).unwrap();
            })
        };
        let got = rx.recv_timeout(std::time::Duration::from_secs(10));
        drop(work);
        t.join().unwrap();
        let (m, d, trimmed, (m2, d2)) = got.expect("accounting waited for the decode lock");
        assert_eq!((m, trimmed, m2), (held, held, 0));
        // With the image cached a level costs its output only; after the trim, the decode too.
        assert_eq!(d, 40 * 30 * 4);
        assert!(d2 > d, "{d2}");
        // The next decode makes the image again.
        s.decode(&DecodeRequest::full(2), &c).unwrap();
        assert_eq!(s.memory_bytes(), held);
    }
}
