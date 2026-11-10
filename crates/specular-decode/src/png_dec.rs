//! PNG / APNG through `specular-png` (own inflate, fused pipeline, scaled decode).
//!
//! 16-bit images (and APNG frames) keep their 16-bit samples: colour (and APNG frames) as
//! [`PixelLayout::Rgba16`], 8 bytes per pixel, sRGB-coded unless an HDR cICP says PQ/HLG; grey
//! as [`PixelLayout::Gray16`] (2 bytes per pixel) and grey + alpha (or grey with a tRNS key) as
//! [`PixelLayout::GrayAlpha16`] (4 bytes per pixel). The embedded profile (iCCP, or
//! gAMA/cHRM) is applied to the 16-bit samples by the renderer. The interlaced quick preview
//! stays 8-bit. Other depths decode to 8-bit layouts.

use crate::{
    Alpha, AnimationInfo, DecodeError, DecodeRequest, Decoder, Format, Frame, Frames, ImageInfo,
    Result, Session,
};
use parking_lot::{Condvar, Mutex};
use specular_color::math::{D65_XY, rgb_to_pcs};
use specular_color::profiles::matrix_shaper_icc;
use specular_core::paths::{Choice, Engine, Request, Task, Trust, selector};
use specular_core::{
    CancelToken, HdrEncoding, Orientation, PixelLayout, Primaries, Raster, RasterPlane, Source,
    Transfer,
};
use specular_png::{
    Animator, ColorType, DecodeOptions, Image, Layout, Output, Parallelism, PngError, PngInfo,
    Wavefront,
};
use std::sync::Arc;
use std::time::Duration;

pub struct PngDecoder;

const SIG: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Interlaced images at least this large offer their first Adam7 pass as a quick preview.
const PREVIEW_MIN_PIXELS: u64 = 2_000_000;

/// Still images at least this large decode the levels announced by
/// [`Session::want_levels`] together, in one pass over the data (every scale costs a full
/// inflate and unfilter).
const PASS_MIN_PIXELS: u64 = 4_000_000;

/// The full-size level joins a pass (kept for region requests) when it needs at most this
/// many bytes. Writing a larger one delays the first level noticeably (36000×18000 RGB, 1.9
/// GB: +1 s on a 16-core machine), so it is decoded when the view zooms in instead.
const FULL_IN_PASS_BYTES: u64 = 768 << 20;

fn map_err(e: PngError) -> DecodeError {
    match e {
        PngError::NotPng => DecodeError::UnsupportedFormat,
        PngError::Truncated(s) => DecodeError::Truncated(s.into()),
        PngError::Unsupported(s) => DecodeError::Unsupported(s.into()),
        PngError::TooLarge(_) => DecodeError::TooLarge,
        PngError::NeedsMemory(n) | PngError::Data(specular_png::InflateError::NeedsMemory(n)) => {
            DecodeError::NeedsMemory(n)
        }
        PngError::Cancelled => DecodeError::Cancelled,
        other => DecodeError::Malformed(other.to_string()),
    }
}

impl Decoder for PngDecoder {
    fn format(&self) -> Format {
        Format::Png
    }
    fn probe(&self, head: &[u8]) -> bool {
        head.starts_with(SIG)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["png", "apng"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let png = specular_png::read_info(source.bytes()).map_err(map_err)?;
        let orientation = png
            .exif(source.bytes())
            .and_then(crate::exif::exif_orientation)
            .map(Orientation::from_exif)
            .unwrap_or(Orientation::NORMAL);
        let animation = (png.animation.is_some() && png.frames.len() > 1).then(|| AnimationInfo {
            frame_count: Some(png.frames.len() as u32),
            loop_count: png.animation.map_or(0, |a| a.num_plays),
        });
        let hdr = hdr_encoding(&png);
        let info = ImageInfo {
            format: Format::Png,
            width: png.width,
            height: png.height,
            orientation,
            // cICP takes precedence over iCCP; an HDR cICP is converted in the shader.
            icc_profile: if hdr.is_some() {
                None
            } else {
                png.icc_profile
                    .clone()
                    .or_else(|| gama_chrm_profile(&png))
                    .map(Arc::new)
            },
            has_alpha: png.has_alpha(),
            bit_depth: png.bit_depth,
            progressive: png.interlaced,
            coding: coding(&png),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(Arc::new(PngSession {
            source,
            info,
            wide: hdr.is_some() || png.bit_depth == 16,
            png,
            animation,
            hdr,
            levels: Mutex::new(Vec::new()),
            pass: Mutex::new(Pass::default()),
            pass_done: Condvar::new(),
        }))
    }
}

/// "RGB 8-bit, interlaced" plus colour / HDR signalling the viewer does not apply.
fn coding(p: &PngInfo) -> String {
    let mut s = p.coding();
    if let Some(c) = p.cicp {
        s += &format!(
            ", cICP {}/{}/{}{}",
            c.color_primaries,
            c.transfer_function,
            c.matrix_coefficients,
            if c.full_range { " full" } else { " narrow" }
        );
        match c.transfer_function {
            16 => s += " (PQ HDR)",
            18 => s += " (HLG HDR)",
            _ => {}
        }
    }
    if p.mastering_display.is_some() {
        s += ", mDCv";
    }
    if let Some(l) = p.content_light_level {
        s += &format!(", cLLi {}/{} nits", l.max_cll / 10_000, l.max_fall / 10_000);
    }
    if p.icc_profile.is_none() && p.cicp.is_none() {
        if let Some(i) = p.srgb_intent {
            s += &format!(", sRGB (intent {i})");
        } else if let Some(g) = p.gamma {
            s += &format!(", gAMA 1/{:.2}", 100_000.0 / f64::from(g.max(1)));
        }
    }
    if p.idot.is_some() {
        s += ", iDOT";
    }
    s
}

/// gAMA (and cHRM) as an embedded profile, when they are the file's only colour description
/// (no cICP, iCCP or sRGB chunk, which take precedence in that order). gAMA alone means
/// BT.709/sRGB primaries; cHRM without gAMA is ignored (the transfer is unknown). A gAMA of
/// about 1/2.2 with sRGB (or no) primaries is left as sRGB: writers use it to approximate the
/// sRGB curve, and a pure 2.2 power would darken shadows the encoder meant as sRGB.
fn gama_chrm_profile(p: &PngInfo) -> Option<Vec<u8>> {
    if p.cicp.is_some() || p.icc_profile.is_some() || p.srgb_intent.is_some() {
        return None;
    }
    let g = p.gamma.filter(|&g| g > 0)?;
    // Decoding exponent.
    let gamma = 100_000.0 / f64::from(g);
    if !(0.1..=10.0).contains(&gamma) {
        return None;
    }
    let xy = |v: (u32, u32)| [f64::from(v.0) / 100_000.0, f64::from(v.1) / 100_000.0];
    let srgb = ([0.64, 0.33], [0.30, 0.60], [0.15, 0.06], D65_XY);
    let prim = p
        .chromaticities
        .map(|c| (xy(c.red), xy(c.green), xy(c.blue), xy(c.white)))
        .filter(|(r, g, b, w)| [r, g, b, w].iter().all(|v| v[1] > 0.0 && v[0] >= 0.0))
        .unwrap_or(srgb);
    let near =
        |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).abs() < 0.002 && (a[1] - b[1]).abs() < 0.002;
    let srgb_prim = near(prim.0, srgb.0)
        && near(prim.1, srgb.1)
        && near(prim.2, srgb.2)
        && near(prim.3, srgb.3);
    if srgb_prim && (gamma - 2.2).abs() < 0.05 {
        return None;
    }
    let m = rgb_to_pcs(prim.0, prim.1, prim.2, prim.3);
    m.inverse()?;
    Some(matrix_shaper_icc(&m, gamma, "PNG gAMA/cHRM"))
}

/// PNG 3rd edition cICP with a PQ (16) or HLG (18) transfer: decoded to 16 bits and
/// linearised in the shader. Primaries 1 = BT.709, 9 = BT.2020, 12 = Display P3 (others are
/// treated as BT.2020, the usual HDR container). Narrow range is not allowed in PNG.
fn hdr_encoding(p: &PngInfo) -> Option<HdrEncoding> {
    let c = p.cicp?;
    let transfer = match c.transfer_function {
        16 => Transfer::Pq,
        18 => Transfer::Hlg,
        _ => return None,
    };
    let primaries = match c.color_primaries {
        1 => Primaries::Bt709,
        12 => Primaries::DisplayP3,
        _ => Primaries::Bt2020,
    };
    Some(HdrEncoding {
        transfer,
        primaries,
    })
}

/// `hdr` labels 16-bit output: PQ/HLG for an HDR cICP, else sRGB-coded samples that the
/// embedded profile (iCCP, gAMA/cHRM) describes like 8-bit ones.
fn to_raster(img: Image, scale: u32, hdr: Option<HdrEncoding>) -> Raster {
    let layout = match img.layout {
        Layout::Gray8 => PixelLayout::Gray,
        Layout::Rgb8Planar => PixelLayout::Rgb,
        Layout::Rgba8 => PixelLayout::Rgba,
        Layout::Rgba16 => PixelLayout::Rgba16(hdr.unwrap_or(HdrEncoding::SRGB)),
        Layout::Gray16 => PixelLayout::Gray16,
        Layout::GrayAlpha16 => PixelLayout::GrayAlpha16,
    };
    Raster {
        width: img.width,
        height: img.height,
        origin: (0, 0),
        scale,
        layout,
        planes: img
            .planes
            .into_iter()
            .map(|p| RasterPlane {
                width: p.width,
                height: p.height,
                stride: p.stride,
                data: p.data,
                h_div: 1,
                v_div: 1,
            })
            .collect(),
        truncated: img.truncated,
    }
}

struct PngSession {
    source: Arc<Source>,
    info: ImageInfo,
    png: PngInfo,
    animation: Option<AnimationInfo>,
    /// PQ/HLG cICP (decoded to 16 bits).
    hdr: Option<HdrEncoding>,
    /// Decoded to 16-bit layouts ([`Output::Auto16`], [`Output::Rgba16`] for an HDR cICP and
    /// for animation frames): 16-bit samples (kept at full precision, so a source transform
    /// works on them rather than on 8-bit roundings) or an HDR cICP.
    wide: bool,
    /// Decoded whole-image levels, kept for region requests only.
    levels: Mutex<Vec<Arc<Raster>>>,
    pass: Mutex<Pass>,
    /// Signalled when a pass ends.
    pass_done: Condvar,
}

/// Whole levels decoded together (see [`PASS_MIN_PIXELS`]).
#[derive(Default)]
struct Pass {
    /// Announced levels no pass has started on yet; 1 means "keep for regions".
    wanted: Vec<u32>,
    /// Scales of the passes in progress.
    running: Vec<u32>,
    /// Levels a pass produced for another request, until that request picks them up.
    ready: Vec<Raster>,
}

impl PngSession {
    /// Wavefront or row-by-row unfiltering for a threaded decode of this image: the path
    /// selector's [`Task::PngUnfilter`] (DESIGN.md §12.3.1). The wavefront costs about 50 %
    /// more CPU time for about half the wall time on large, highly compressed images, so the
    /// image the user waits for takes it where it is faster and background work where it is
    /// cheap enough. `None` (no threads, interlaced, or the selector has not checked the
    /// wavefront on this machine yet): `specular-png`'s fixed gate, untimed.
    fn wavefront(&self, parallel: bool) -> Option<Choice> {
        let px = u64::from(self.png.width) * u64::from(self.png.height);
        if !parallel || self.png.interlaced || px < specular_png::AUTO_PARALLEL_MIN_PIXELS {
            return None;
        }
        let sel = selector();
        sel.set_available(Task::PngUnfilter, Engine::CpuParallel);
        if sel.trust(Task::PngUnfilter, 1) != Trust::Trusted {
            return None;
        }
        let channels: u64 = match self.png.color_type {
            ColorType::Gray | ColorType::Indexed => 1,
            ColorType::GrayAlpha => 2,
            ColorType::Rgb => 3,
            ColorType::Rgba => 4,
        };
        let raw = (u64::from(self.png.width) * channels * u64::from(self.png.bit_depth))
            .div_ceil(8)
            .saturating_add(1)
            * u64::from(self.png.height);
        let variant = specular_core::paths::png_variant(raw, self.source.bytes().len() as u64);
        Some(sel.choose(Task::PngUnfilter, &Request::new(px, variant)))
    }

    /// The still image's output: the smallest 16-bit layout for 16-bit SDR images (grey stays
    /// grey), RGBA16 for an HDR cICP.
    fn output(&self) -> Output {
        match (self.wide, self.hdr) {
            (false, _) => Output::Auto8,
            (true, None) => Output::Auto16,
            (true, Some(_)) => Output::Rgba16,
        }
    }

    fn options<'a>(
        &self,
        scale: u32,
        parallel: bool,
        wavefront: Option<&Choice>,
        cancel: &'a CancelToken,
    ) -> DecodeOptions<'a> {
        DecodeOptions {
            output: self.output(),
            scale,
            parallelism: if parallel {
                Parallelism::Auto
            } else {
                Parallelism::Single
            },
            wavefront: match wavefront {
                None => Wavefront::Auto,
                Some(c) if c.engine == Engine::CpuParallel => Wavefront::On,
                Some(_) => Wavefront::Off,
            },
            verify_checksums: false,
            cancel: Some(cancel.flag()),
        }
    }

    /// The cached whole level at 1/`scale`, for region requests.
    fn level(&self, scale: u32, parallel: bool, cancel: &CancelToken) -> Result<Arc<Raster>> {
        if let Some(r) = self.cached(scale) {
            return Ok(r);
        }
        // A pass that will cache this level: wait for it rather than decode it twice.
        self.wait_pass(scale, cancel)?;
        if let Some(r) = self.cached(scale) {
            return Ok(r);
        }
        let r = Arc::new(self.decode_level(scale, parallel, cancel)?);
        let mut g = self.levels.lock();
        if !g.iter().any(|l| l.scale == scale) {
            g.push(r.clone());
        }
        Ok(r)
    }

    fn cached(&self, scale: u32) -> Option<Arc<Raster>> {
        self.levels
            .lock()
            .iter()
            .find(|r| r.scale == scale)
            .cloned()
    }

    /// Still and large enough for one-pass multi-level decoding.
    fn pass_eligible(&self) -> bool {
        self.animation.is_none()
            && u64::from(self.png.width) * u64::from(self.png.height) >= PASS_MIN_PIXELS
    }

    /// Whether the full-size level is small enough to be decoded along with the others.
    fn full_in_pass(&self) -> bool {
        self.out_bytes(1) <= FULL_IN_PASS_BYTES
    }

    /// Waits while a pass that includes `scale` is running.
    fn wait_pass(&self, scale: u32, cancel: &CancelToken) -> Result<()> {
        let mut g = self.pass.lock();
        while g.running.contains(&scale) {
            if cancel.is_cancelled() {
                return Err(DecodeError::Cancelled);
            }
            self.pass_done.wait_for(&mut g, Duration::from_millis(20));
        }
        Ok(())
    }

    /// The whole level at 1/`scale` from a pass: produced by one already (or by the one in
    /// progress), or by a new pass over all announced levels. `None` when no pass covers it.
    fn level_from_pass(
        &self,
        scale: u32,
        parallel: bool,
        cancel: &CancelToken,
    ) -> Result<Option<Raster>> {
        let mut g = self.pass.lock();
        loop {
            if let Some(i) = g.ready.iter().position(|r| r.scale == scale) {
                return Ok(Some(g.ready.swap_remove(i)));
            }
            if !g.running.contains(&scale) {
                break;
            }
            if cancel.is_cancelled() {
                return Err(DecodeError::Cancelled);
            }
            self.pass_done.wait_for(&mut g, Duration::from_millis(20));
        }
        if !g.wanted.contains(&scale) {
            return Ok(None);
        }
        let mut scales = std::mem::take(&mut g.wanted);
        scales.sort_unstable();
        g.running.extend_from_slice(&scales);
        drop(g);
        let t = std::time::Instant::now();
        let data = self.source.bytes();
        let choice = self.wavefront(parallel);
        let opts = self.options(1, parallel, choice.as_ref(), cancel);
        let run = choice.as_ref().map(|c| selector().start(c));
        let _load = run.is_none().then(|| selector().busy());
        let res = specular_png::decode_scales(data, &opts, &scales);
        if let (Some(run), Ok(_)) = (run, &res) {
            run.finish();
        }
        let mut g = self.pass.lock();
        for s in &scales {
            if let Some(i) = g.running.iter().position(|r| r == s) {
                g.running.swap_remove(i);
            }
        }
        let out = match res {
            Ok(images) => {
                specular_log::debug!(target: specular_log::DECODE, "png: levels {scales:?} in one pass, {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
                let mut mine = None;
                for (img, &s) in images.into_iter().zip(&scales) {
                    let r = to_raster(img, s, self.hdr);
                    if s == 1 {
                        // Kept for regions (and copied when it is also the request).
                        if scale == 1 {
                            mine = Some(r.clone());
                        }
                        let mut l = self.levels.lock();
                        if !l.iter().any(|x| x.scale == 1) {
                            l.push(Arc::new(r));
                        }
                    } else if s == scale {
                        mine = Some(r);
                    } else {
                        g.ready.push(r);
                    }
                }
                Ok(mine)
            }
            Err(e) => Err(map_err(e)),
        };
        drop(g);
        self.pass_done.notify_all();
        out
    }

    /// The whole level at 1/`scale` box-filtered from the cached full-size level, which
    /// costs a fraction of a decode.
    fn level_from_full(&self, scale: u32, parallel: bool) -> Option<Raster> {
        if scale <= 1 {
            return None;
        }
        let full = self.cached(1)?;
        let layout = match full.layout {
            PixelLayout::Gray => Layout::Gray8,
            PixelLayout::Rgb => Layout::Rgb8Planar,
            PixelLayout::Rgba => Layout::Rgba8,
            PixelLayout::Rgba16(_) => Layout::Rgba16,
            PixelLayout::Gray16 => Layout::Gray16,
            PixelLayout::GrayAlpha16 => Layout::GrayAlpha16,
            _ => return None,
        };
        let planes: Vec<(&[u8], usize)> = full
            .planes
            .iter()
            .map(|p| (p.data.as_slice(), p.stride))
            .collect();
        let workers = if parallel {
            std::thread::available_parallelism().map_or(1, |n| n.get().min(8))
        } else {
            1
        };
        let img = specular_png::downscale(layout, full.width, full.height, &planes, scale, workers)
            .ok()?;
        let mut r = to_raster(img, scale, self.hdr);
        r.truncated = full.truncated;
        Some(r)
    }

    /// Decodes the whole image at 1/`scale` (not cached).
    fn decode_level(&self, scale: u32, parallel: bool, cancel: &CancelToken) -> Result<Raster> {
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let data = self.source.bytes();
        // `decode` shows the first animation frame; when the default image is not part of the
        // animation that is frame 0 (which covers the whole canvas), not the IDAT image.
        let frame = self.animation.is_some() && !self.png.default_image_is_frame;
        let choice = if frame {
            None
        } else {
            self.wavefront(parallel)
        };
        let opts = self.options(scale, parallel, choice.as_ref(), cancel);
        let run = choice.as_ref().map(|c| selector().start(c));
        let _load = run.is_none().then(|| selector().busy());
        let img = if frame {
            specular_png::decode_frame(data, &self.png, 0, &opts)
        } else {
            specular_png::decode(data, &opts)
        }
        .map_err(map_err)?;
        if let Some(run) = run {
            run.finish();
        }
        Ok(to_raster(img, scale, self.hdr))
    }

    fn bytes_per_pixel(&self) -> u64 {
        let layout = match self.output() {
            Output::Auto8 => self.png.auto_layout(),
            Output::Auto16 => self.png.auto_layout16(),
            _ => Layout::Rgba16,
        };
        match layout {
            Layout::Gray8 => 1,
            Layout::Gray16 => 2,
            Layout::Rgb8Planar => 3,
            Layout::Rgba8 | Layout::GrayAlpha16 => 4,
            Layout::Rgba16 => 8,
        }
    }

    fn out_bytes(&self, scale: u32) -> u64 {
        let s = scale.max(1);
        u64::from(self.info.width.div_ceil(s))
            * u64::from(self.info.height.div_ceil(s))
            * self.bytes_per_pixel()
    }
}

impl Session for PngSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    /// `cICP` over `iCCP`; then `sRGB`; a profile made from `gAMA`/`cHRM` has no name.
    fn colour_space(&self) -> Option<&'static str> {
        let p = &self.png;
        if let Some(c) = p.cicp
            && let Some(w) = crate::colour_space::cicp_space(
                u16::from(c.color_primaries),
                u16::from(c.transfer_function),
            )
        {
            return Some(w);
        }
        if p.icc_profile.is_some() {
            return None;
        }
        if p.srgb_intent.is_some() {
            return Some("srgb");
        }
        self.info.icc_profile.is_some().then_some("")
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(match self.png.color_type {
            ColorType::Gray | ColorType::GrayAlpha => "Greyscale",
            ColorType::Rgb | ColorType::Rgba => "RGB",
            ColorType::Indexed => "Palette",
        })
    }
    /// An alpha channel, or tRNS: transparent palette entries or a grey/RGB colour key.
    fn alpha(&self) -> Option<Alpha> {
        match self.png.color_type {
            ColorType::GrayAlpha | ColorType::Rgba => Some(Alpha::Straight),
            _ => self.png.has_alpha().then_some(Alpha::Transparency),
        }
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    /// For large interlaced files, the first Adam7 pass: a 1/8 image from 1/64 of the data.
    fn decode_preview(&self) -> Option<Result<Raster>> {
        let px = u64::from(self.png.width) * u64::from(self.png.height);
        // (Not for HDR: the 8-bit preview would show raw PQ/HLG codes. 16-bit SDR images get
        // an 8-bit preview; their levels are 16-bit.)
        if !self.png.interlaced
            || px < PREVIEW_MIN_PIXELS
            || self.animation.is_some()
            || self.hdr.is_some()
        {
            return None;
        }
        let opts = DecodeOptions {
            scale: 8,
            parallelism: Parallelism::Single,
            ..Default::default()
        };
        Some(
            specular_png::decode_adam7_preview(self.source.bytes(), &opts)
                .map(|img| to_raster(img, 8, None))
                .map_err(map_err),
        )
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        let scale = req.scale.max(1);
        match req.region {
            None => {
                specular_log::debug!(target: specular_log::DECODE, "png: decode 1/{scale} whole image")
            }
            Some(r) => {
                specular_log::trace!(target: specular_log::DECODE, "png: 1/{scale} region {}×{}+{}+{}", r.w, r.h, r.x, r.y)
            }
        }
        match req.region {
            // Whole levels are handed over as decoded: caching them would cost a copy of the
            // image per request (as long as the decode itself for large RGB files) and keep a
            // second copy alive for the session. Only region requests use the cache.
            None => {
                let cached = self
                    .levels
                    .lock()
                    .iter()
                    .find(|r| r.scale == scale)
                    .cloned();
                if let Some(r) = cached {
                    return Ok((*r).clone());
                }
                if let Some(r) = self.level_from_pass(scale, req.parallel, cancel)? {
                    return Ok(r);
                }
                // A pass decoding the full-size level: derive this one from it.
                self.wait_pass(1, cancel)?;
                if let Some(r) = self.level_from_full(scale, req.parallel) {
                    specular_log::debug!(target: specular_log::DECODE, "png: 1/{scale} from the full-size level");
                    return Ok(r);
                }
                self.decode_level(scale, req.parallel, cancel)
            }
            Some(r) => {
                let level = self.level(scale, req.parallel, cancel)?;
                Ok(crate::jpeg::crop(&level, r))
            }
        }
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let scale = req.scale.max(1);
        // The level (cached for region requests, which return a crop of it), an inflate
        // window, and for Adam7 at reduced scale the full-resolution work canvas.
        let mut m = self.out_bytes(scale) + (1 << 20);
        if let Some(r) = req.region {
            // The returned crop (`r` is in level pixels).
            m += u64::from(r.w) * u64::from(r.h) * self.bytes_per_pixel();
        }
        if self.png.interlaced && scale > 1 {
            m += self.out_bytes(1) * 4 / 3;
        }
        m
    }
    fn open_memory(&self, levels: &[u32]) -> u64 {
        let whole = |scale| {
            self.decode_memory(&DecodeRequest {
                scale,
                region: None,
                parallel: true,
                prepare_regions: true,
            })
        };
        if !self.pass_eligible() {
            return levels.iter().map(|&s| whole(s)).sum();
        }
        // One pass: every level (and the full-size one when it joins) plus one work canvas.
        let mut m: u64 = levels.iter().map(|&s| self.out_bytes(s)).sum::<u64>() + (1 << 20);
        if self.full_in_pass() && !levels.contains(&1) {
            m += self.out_bytes(1);
        }
        if self.png.interlaced && levels.iter().any(|&s| s > 1) {
            m += self.out_bytes(1) * 4 / 3;
        }
        m
    }
    fn scaled_decode_cheap(&self) -> bool {
        // Every scale inflates and unfilters all of the data.
        false
    }
    fn want_levels(&self, scales: &[u32]) {
        if !self.pass_eligible() {
            return;
        }
        let full = self.full_in_pass();
        let have: Vec<u32> = self.levels.lock().iter().map(|r| r.scale).collect();
        let mut g = self.pass.lock();
        for &s in scales {
            let s = s.max(1);
            if (s > 1 || full)
                && !have.contains(&s)
                && !g.wanted.contains(&s)
                && !g.running.contains(&s)
                && !g.ready.iter().any(|r| r.scale == s)
            {
                g.wanted.push(s);
            }
        }
    }
    fn memory_bytes(&self) -> usize {
        let levels: usize = self.levels.lock().iter().map(|r| r.memory_bytes()).sum();
        let ready: usize = self
            .pass
            .lock()
            .ready
            .iter()
            .map(|r| r.memory_bytes())
            .sum();
        levels + ready
    }
    /// Drops the cached levels. Levels a pass made for a request still on its way stay: that
    /// request takes them shortly.
    fn trim_memory(&self) -> usize {
        crate::whole::drop_levels(&self.levels)
    }
    fn animation(&self) -> Option<&AnimationInfo> {
        self.animation.as_ref()
    }
    fn frames(&self, scale: u32) -> Result<Box<dyn Frames>> {
        if self.animation.is_none() {
            return Err(DecodeError::Unsupported("not animated".into()));
        }
        let a = Animator::with_info(
            SourceBytes(self.source.clone()),
            self.png.clone(),
            if self.wide {
                Output::Rgba16
            } else {
                Output::Rgba8
            },
            scale.max(1),
        )
        .map_err(map_err)?;
        Ok(Box::new(PngFrames {
            a,
            scale: scale.max(1),
            hdr: self.hdr,
        }))
    }
}

struct SourceBytes(Arc<Source>);

impl AsRef<[u8]> for SourceBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.bytes()
    }
}

/// Sequential APNG frames: one canvas (plus a saved region for "previous" disposal).
struct PngFrames {
    a: Animator<SourceBytes>,
    scale: u32,
    /// Labels 16-bit frames (see [`to_raster`]).
    hdr: Option<HdrEncoding>,
}

impl Frames for PngFrames {
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame>> {
        let r = self.a.next_frame(Some(cancel.flag()))?;
        Some(r.map_err(map_err).map(|f| {
            let den = if f.delay_den == 0 { 100 } else { f.delay_den };
            Frame {
                index: f.index as u32,
                raster: to_raster(f.image, self.scale, self.hdr),
                delay: Duration::from_nanos(
                    u64::from(f.delay_num) * 1_000_000_000 / u64::from(den),
                ),
            }
        }))
    }
    fn rewind(&mut self) -> Result<()> {
        self.a.rewind();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(out: &mut Vec<u8>, ty: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let start = out.len();
        out.extend_from_slice(ty);
        out.extend_from_slice(data);
        let mut c = !0u32;
        for &b in &out[start..] {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        out.extend_from_slice(&(!c).to_be_bytes());
    }

    /// Stored-block zlib stream (no compressor needed).
    fn zlib_stored(raw: &[u8]) -> Vec<u8> {
        let mut z = vec![0x78, 0x01, 1];
        let n = raw.len() as u16;
        z.extend_from_slice(&n.to_le_bytes());
        z.extend_from_slice(&(!n).to_le_bytes());
        z.extend_from_slice(raw);
        let (mut a, mut b) = (1u32, 0u32);
        for &x in raw {
            a = (a + u32::from(x)) % 65521;
            b = (b + a) % 65521;
        }
        z.extend_from_slice(&((b << 16) | a).to_be_bytes());
        z
    }

    fn fctl(out: &mut Vec<u8>, seq: u32, w: u32, h: u32, delay: (u16, u16)) {
        let mut d = seq.to_be_bytes().to_vec();
        for v in [w, h, 0, 0] {
            d.extend_from_slice(&v.to_be_bytes());
        }
        d.extend_from_slice(&delay.0.to_be_bytes());
        d.extend_from_slice(&delay.1.to_be_bytes());
        d.extend_from_slice(&[0, 0]);
        chunk(out, b"fcTL", &d);
    }

    /// 2x1 RGB APNG: two frames (red, then green), 1/10 s and 1/4 s, 2 plays.
    fn apng() -> Vec<u8> {
        let mut f = SIG.to_vec();
        let mut ihdr = 2u32.to_be_bytes().to_vec();
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(&mut f, b"IHDR", &ihdr);
        let mut actl = 2u32.to_be_bytes().to_vec();
        actl.extend_from_slice(&2u32.to_be_bytes());
        chunk(&mut f, b"acTL", &actl);
        fctl(&mut f, 0, 2, 1, (1, 10));
        chunk(&mut f, b"IDAT", &zlib_stored(&[0, 255, 0, 0, 255, 0, 0]));
        fctl(&mut f, 1, 2, 1, (1, 4));
        let mut fd = 2u32.to_be_bytes().to_vec();
        fd.extend_from_slice(&zlib_stored(&[0, 0, 255, 0, 0, 255, 0]));
        chunk(&mut f, b"fdAT", &fd);
        chunk(&mut f, b"IEND", &[]);
        f
    }

    /// 1×1 grey RGB PNG with the given colour chunks before IDAT.
    fn tagged(chunks: &[(&[u8; 4], Vec<u8>)]) -> Option<Arc<Vec<u8>>> {
        let mut f = SIG.to_vec();
        let mut ihdr = 1u32.to_be_bytes().to_vec();
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(&mut f, b"IHDR", &ihdr);
        for (ty, data) in chunks {
            chunk(&mut f, ty, data);
        }
        chunk(&mut f, b"IDAT", &zlib_stored(&[0, 128, 128, 128]));
        chunk(&mut f, b"IEND", &[]);
        let s = PngDecoder
            .open(Arc::new(Source::from_vec("a.png", f)))
            .unwrap();
        s.info().icc_profile.clone()
    }

    fn be(vals: &[u32]) -> Vec<u8> {
        vals.iter().flat_map(|v| v.to_be_bytes()).collect()
    }

    /// gAMA/cHRM become a matrix/shaper profile only when nothing else describes the colour,
    /// and gAMA ≈ 1/2.2 on sRGB primaries stays sRGB.
    #[test]
    fn gama_chrm_as_profile() {
        let adobe = be(&[
            31_270, 32_900, 64_000, 33_000, 21_000, 71_000, 15_000, 6_000,
        ]);
        let icc = tagged(&[(b"gAMA", be(&[55_556])), (b"cHRM", adobe.clone())])
            .expect("gAMA 1/1.8 + cHRM: a profile");
        let p = specular_color::icc::Profile::parse(&icc).unwrap();
        let want = rgb_to_pcs([0.64, 0.33], [0.21, 0.71], [0.15, 0.06], [0.3127, 0.329]);
        assert!(p.matrix.unwrap().max_diff(&want) < 1e-3);
        assert!(tagged(&[(b"gAMA", be(&[55_556]))]).is_some(), "gAMA alone");
        assert!(
            tagged(&[(b"gAMA", be(&[45_455]))]).is_none(),
            "1/2.2 stays sRGB"
        );
        assert!(
            tagged(&[(b"gAMA", be(&[45_455])), (b"cHRM", adobe.clone())]).is_some(),
            "1/2.2 on other primaries"
        );
        assert!(tagged(&[(b"cHRM", adobe)]).is_none(), "cHRM alone");
        assert!(
            tagged(&[(b"sRGB", vec![0]), (b"gAMA", be(&[55_556]))]).is_none(),
            "sRGB wins"
        );
        assert!(tagged(&[(b"gAMA", be(&[0]))]).is_none(), "invalid gAMA");
    }

    /// Whole levels are returned without being cached; regions crop a cached level.
    #[test]
    fn whole_levels_not_cached() {
        let s = PngDecoder
            .open(Arc::new(Source::from_vec("a.png", apng())))
            .unwrap();
        let c = CancelToken::new();
        let whole = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!(s.memory_bytes(), 0);
        let req = DecodeRequest {
            region: Some(crate::Rect {
                x: 0,
                y: 0,
                w: 2,
                h: 1,
            }),
            ..DecodeRequest::full(1)
        };
        let region = s.decode(&req, &c).unwrap();
        assert!(s.memory_bytes() > 0);
        assert_eq!(region.planes[0].data, whole.planes[0].data);
        let again = s.decode(&DecodeRequest::full(1), &c).unwrap();
        assert_eq!(again.planes[0].data, whole.planes[0].data);
    }

    /// Announced levels come from one pass and match separate decodes; the full-size level
    /// is kept for regions and later levels are derived from it.
    #[test]
    fn levels_in_one_pass() {
        let (w, h) = (2400u32, 1700u32);
        let rgb: Vec<u8> = (0..w * h)
            .flat_map(|i| {
                let (x, y) = (i % w, i / w);
                [(x * 7 + y) as u8, (x ^ y) as u8, (y * 3) as u8]
            })
            .collect();
        let png = specular_png::encode(w, h, specular_png::ColorType::Rgb, &rgb).unwrap();
        let open = || {
            PngDecoder
                .open(Arc::new(Source::from_vec("big.png", png.clone())))
                .unwrap()
        };
        let c = CancelToken::new();
        let plain = open();
        assert!(!plain.scaled_decode_cheap());
        let want = |s: u32| data(&plain.decode(&DecodeRequest::full(s), &c).unwrap());
        fn data(r: &Raster) -> Vec<Vec<u8>> {
            r.planes.iter().map(|p| p.data.clone()).collect()
        }
        let s = open();
        s.want_levels(&[8, 4, 1]);
        let l4 = s.decode(&DecodeRequest::full(4), &c).unwrap();
        // 1/8 was decoded in the same pass and waits for its request; 1/1 is cached.
        assert!(s.memory_bytes() > (w * h * 3) as usize);
        let l8 = s.decode(&DecodeRequest::full(8), &c).unwrap();
        assert_eq!(data(&l4), want(4));
        assert_eq!(data(&l8), want(8));
        assert_eq!(s.memory_bytes(), (w * h * 3) as usize);
        let req = DecodeRequest {
            region: Some(crate::Rect {
                x: 100,
                y: 50,
                w: 300,
                h: 200,
            }),
            ..DecodeRequest::full(1)
        };
        let region = s.decode(&req, &c).unwrap();
        assert_eq!(data(&region), data(&plain.decode(&req, &c).unwrap()));
        // Not announced: box-filtered from the full-size level, identical to a decode.
        let l2 = s.decode(&DecodeRequest::full(2), &c).unwrap();
        assert_eq!(data(&l2), want(2));
        assert_eq!((l2.width, l2.height, l2.scale), (1200, 850, 2));
    }

    /// A 2×1 16-bit PNG (or a two-frame APNG whose second frame is `second`) of colour type
    /// `ct` with big-endian `samples` per row.
    fn png16(ct: u8, first: &[u16], second: Option<&[u16]>) -> Vec<u8> {
        let row = |s: &[u16]| {
            let mut r = vec![0u8];
            r.extend(s.iter().flat_map(|v| v.to_be_bytes()));
            zlib_stored(&r)
        };
        let mut f = SIG.to_vec();
        let mut ihdr = 2u32.to_be_bytes().to_vec();
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&[16, ct, 0, 0, 0]);
        chunk(&mut f, b"IHDR", &ihdr);
        if second.is_some() {
            chunk(&mut f, b"acTL", &be(&[2, 0]));
            fctl(&mut f, 0, 2, 1, (1, 10));
        }
        chunk(&mut f, b"IDAT", &row(first));
        if let Some(s) = second {
            fctl(&mut f, 1, 2, 1, (1, 10));
            let mut fd = 2u32.to_be_bytes().to_vec();
            fd.extend_from_slice(&row(s));
            chunk(&mut f, b"fdAT", &fd);
        }
        chunk(&mut f, b"IEND", &[]);
        f
    }

    fn ne16(r: &Raster) -> Vec<u16> {
        r.planes[0]
            .data
            .chunks_exact(2)
            .map(|b| u16::from_ne_bytes([b[0], b[1]]))
            .collect()
    }

    /// 16-bit grey and grey + alpha reach the renderer as compact grey layouts, RGB and RGBA
    /// as sRGB-coded RGBA16, with every sample intact, at every scale and in regions.
    #[test]
    fn sixteen_bit_kept() {
        let (a, b) = (0x1234u16, 0xFEDCu16);
        let rgba16 = PixelLayout::Rgba16(HdrEncoding::SRGB);
        let cases: [(u8, Vec<u16>, PixelLayout, Vec<u16>); 4] = [
            (0, vec![a, b], PixelLayout::Gray16, vec![a, b]),
            (
                4,
                vec![a, 0x8001, b, 3],
                PixelLayout::GrayAlpha16,
                vec![a, 0x8001, b, 3],
            ),
            (
                2,
                vec![a, 1, b, b, 2, a],
                rgba16,
                vec![a, 1, b, 0xFFFF, b, 2, a, 0xFFFF],
            ),
            (
                6,
                vec![a, 1, b, 7, b, 2, a, 9],
                rgba16,
                vec![a, 1, b, 7, b, 2, a, 9],
            ),
        ];
        let c = CancelToken::new();
        for (ct, samples, layout, want) in cases {
            let s = PngDecoder
                .open(Arc::new(Source::from_vec(
                    "a.png",
                    png16(ct, &samples, None),
                )))
                .unwrap();
            assert_eq!(s.info().bit_depth, 16);
            let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
            assert_eq!(r.layout, layout, "type {ct}");
            assert_eq!(r.planes[0].stride, 2 * layout.pixel_bytes(), "type {ct}");
            assert_eq!(ne16(&r), want, "type {ct}");
            let req = DecodeRequest {
                region: Some(crate::Rect {
                    x: 1,
                    y: 0,
                    w: 1,
                    h: 1,
                }),
                ..DecodeRequest::full(1)
            };
            let region = s.decode(&req, &c).unwrap();
            // Regions are widened to 16-pixel alignment: here the whole row.
            assert_eq!((region.layout, region.origin), (r.layout, (0, 0)));
            assert_eq!(ne16(&region), want, "type {ct} region");
            let half = s.decode(&DecodeRequest::full(2), &c).unwrap();
            assert_eq!(half.layout, r.layout, "type {ct} 1/2");
            assert_eq!(
                half.planes[0].data.len(),
                layout.pixel_bytes(),
                "type {ct} 1/2"
            );
        }
    }

    /// 16-bit APNG frames are composited and handed on at 16 bits.
    #[test]
    fn sixteen_bit_frames() {
        let (f0, f1) = ([0x0101u16, 2, 3, 0x7FFF, 5, 6], [9u16, 8, 7, 6, 5, 0xABCD]);
        let src = Arc::new(Source::from_vec("a.png", png16(2, &f0, Some(&f1))));
        let s = PngDecoder.open(src).unwrap();
        let mut fr = s.frames(1).unwrap();
        let c = CancelToken::new();
        for f in [f0, f1] {
            let r = fr.next_frame(&c).unwrap().unwrap().raster;
            assert_eq!(r.layout, PixelLayout::Rgba16(HdrEncoding::SRGB));
            let want = [f[0], f[1], f[2], 0xFFFF, f[3], f[4], f[5], 0xFFFF];
            assert_eq!(ne16(&r), want);
        }
    }

    #[test]
    fn animation_api() {
        let src = Arc::new(Source::from_vec("a.png", apng()));
        let s = PngDecoder.open(src).unwrap();
        let a = s.animation().unwrap();
        assert_eq!((a.frame_count, a.loop_count), (Some(2), 2));
        let still = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert_eq!(still.layout, PixelLayout::Rgb);
        assert_eq!(still.planes[0].data, vec![255, 255]);
        let mut fr = s.frames(1).unwrap();
        let c = CancelToken::new();
        for pass in 0..2 {
            let f0 = fr.next_frame(&c).unwrap().unwrap();
            assert_eq!(f0.delay, Duration::from_millis(100));
            assert_eq!(&f0.raster.planes[0].data[..4], &[255, 0, 0, 255]);
            let f1 = fr.next_frame(&c).unwrap().unwrap();
            assert_eq!((f1.index, f1.delay), (1, Duration::from_millis(250)));
            assert_eq!(&f1.raster.planes[0].data[..4], &[0, 255, 0, 255]);
            assert!(fr.next_frame(&c).is_none());
            if pass == 0 {
                fr.rewind().unwrap();
            }
        }
    }

    /// Trimming drops the level kept for regions; the next region decodes it again.
    #[test]
    fn trims_region_level() {
        let s = PngDecoder
            .open(Arc::new(Source::from_vec("a.png", apng())))
            .unwrap();
        let req = DecodeRequest {
            region: Some(crate::Rect {
                x: 0,
                y: 0,
                w: 2,
                h: 1,
            }),
            ..DecodeRequest::full(1)
        };
        crate::trim_check::assert_trims(&*s, &req);
    }
}
