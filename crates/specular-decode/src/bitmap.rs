//! Adapters for the formats decoded by `specular-bitmap` (own decoders): GIF (animated), BMP,
//! ICO/CUR, Windows animated cursors (.ani), TGA, QOI and PNM/PAM.
//!
//! Random-access formats (uncompressed BMP/TGA, binary PNM, and GIF once its first frame's
//! indices are cached) decode every scale and region directly from the file. Sequential ones
//! (RLE, QOI, ASCII PNM) decode a whole level once, cache it (within a memory bound) and crop
//! regions from it. ICO entries that are PNG streams are handed to the PNG decoder. Every
//! usable entry of an ICO/CUR file is a page, largest first (the default image), labelled
//! with its size and bit depth; an animated cursor's frames are not pages. The images of a
//! multi-image PNM/PAM file are pages too, in file order, labelled with their size; their
//! offsets come from a scan of the headers at open, and a page parses only its own header.
//!
//! PNM samples deeper than 8 bits stay 16-bit ([`PixelLayout::Gray16`],
//! [`PixelLayout::GrayAlpha16`] or sRGB-coded [`PixelLayout::Rgba16`]).

use crate::{
    Alpha, AnimationInfo, DecodeError, DecodeRequest, Decoder, Format, Frame, Frames, ImageInfo,
    PageLabel, Rect, Result, Session,
};
use parking_lot::Mutex;
use specular_bitmap as bm;
use specular_core::{
    CancelToken, HdrEncoding, Orientation, PixelLayout, Raster, RasterPlane, Source,
};
use std::sync::Arc;
use std::time::Duration;

/// Cached whole levels of sequential formats are kept only below this size; above it every
/// request decodes again (bounded memory, slower).
const LEVEL_CACHE_MAX: usize = 768 << 20;

/// Which `specular-bitmap` format a [`BitmapDecoder`] handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Gif,
    Bmp,
    Ico,
    Ani,
    Tga,
    Qoi,
    Pnm,
}

pub struct BitmapDecoder(pub Kind);

fn map_err(e: bm::Error) -> DecodeError {
    match e {
        bm::Error::Unsupported(s) => DecodeError::Unsupported(s),
        bm::Error::Truncated(s) => DecodeError::Truncated(s),
        bm::Error::Malformed(s) => DecodeError::Malformed(s),
        bm::Error::TooLarge => DecodeError::TooLarge,
        bm::Error::BadRequest(s) => DecodeError::Unsupported(format!("request: {s}")),
        bm::Error::Cancelled => DecodeError::Cancelled,
    }
}

impl Decoder for BitmapDecoder {
    fn format(&self) -> Format {
        match self.0 {
            Kind::Gif => Format::Gif,
            Kind::Bmp => Format::Bmp,
            Kind::Ico | Kind::Ani => Format::Ico,
            Kind::Tga => Format::Tga,
            Kind::Qoi => Format::Qoi,
            Kind::Pnm => Format::Pnm,
        }
    }

    fn probe(&self, head: &[u8]) -> bool {
        match self.0 {
            Kind::Gif => bm::gif::probe(head),
            Kind::Bmp => bm::bmp::probe(head),
            Kind::Ico => bm::ico::probe(head),
            Kind::Ani => bm::ani::probe(head),
            // TGA has no signature: it is opened by extension only (see `Registry::open`).
            Kind::Tga => false,
            Kind::Qoi => bm::qoi::probe(head),
            // "P7 332" is an XV thumbnail (`whole_dec`), not a PAM file.
            Kind::Pnm => bm::pnm::probe(head) && !head.starts_with(b"P7 332"),
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Kind::Gif => &["gif"],
            Kind::Bmp => &["bmp", "dib"],
            Kind::Ico => &["ico", "cur"],
            Kind::Ani => &["ani"],
            Kind::Tga => &["tga", "icb", "vda", "vst"],
            Kind::Qoi => &["qoi"],
            Kind::Pnm => &["pnm", "pbm", "pgm", "ppm", "pam"],
        }
    }

    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let data = source.bytes();
        let format = self.format();
        let (inner, anim, extra) = match self.0 {
            Kind::Gif => {
                let g = Arc::new(bm::Gif::parse(data).map_err(map_err)?);
                let a = g.animation();
                let anim = (a.shown.len() > 1).then(|| AnimationInfo {
                    frame_count: Some(a.shown.len() as u32),
                    loop_count: match a.loop_count {
                        Some(0) => 0,
                        Some(n) => n as u32 + 1,
                        None => 1,
                    },
                });
                let extra = if a.shown.len() > 1 {
                    format!(", {} frames", a.shown.len())
                } else {
                    String::new()
                };
                (Inner::Gif(g), anim, extra)
            }
            Kind::Bmp => match bm::bmp::Bmp::parse(data) {
                Ok(b) => (Inner::Plain(Box::new(b)), None, String::new()),
                // BI_JPEG / BI_PNG (printer DIBs, BMP Suite's rgb24jpeg/rgb24png): the
                // pixels are a whole JPEG or PNG file, shown through its own decoder.
                Err(bm::Error::Unsupported(s)) if s == "JPEG in BMP" || s == "PNG in BMP" => {
                    let off = data
                        .get(10..14)
                        .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
                    let inner = data.get(off..).unwrap_or(&[]).to_vec();
                    let what = if s.starts_with("JPEG") {
                        "BMP with embedded JPEG"
                    } else {
                        "BMP with embedded PNG"
                    };
                    return crate::embedded_dec::open_embedded(
                        source.clone(),
                        Source::from_vec("embedded", inner).within(&source),
                        format,
                        what,
                    );
                }
                Err(e) => return Err(map_err(e)),
            },
            Kind::Tga => {
                let t = bm::tga::Tga::parse(data).map_err(map_err)?;
                (Inner::Plain(Box::new(t)), None, String::new())
            }
            Kind::Qoi => (
                Inner::Plain(Box::new(bm::qoi::Qoi::parse(data).map_err(map_err)?)),
                None,
                String::new(),
            ),
            Kind::Ico => {
                let ico = Arc::new(bm::ico::Ico::parse(data).map_err(map_err)?);
                return open_ico_page(source, ico, 0);
            }
            Kind::Pnm => {
                let offsets = bm::pnm::image_offsets(data);
                if offsets.len() > 1 {
                    return open_pnm_page(source, Arc::new(offsets), 0);
                }
                (
                    Inner::Plain(Box::new(bm::pnm::Pnm::parse(data).map_err(map_err)?)),
                    None,
                    String::new(),
                )
            }
            Kind::Ani => {
                let ani = Arc::new(bm::ani::Ani::parse(data).map_err(map_err)?);
                let first = open_ico(&source, ani.frame_data(data, ani.steps[0].frame))?;
                let anim = (ani.steps.len() > 1).then_some(AnimationInfo {
                    frame_count: Some(ani.steps.len() as u32),
                    loop_count: 0,
                });
                let extra = format!(
                    " animated cursor, {} frames, {} steps",
                    ani.frames.len(),
                    ani.steps.len()
                );
                (Inner::Ani(ani, first), anim, extra)
            }
        };
        finish(source, format, inner, anim, extra, None)
    }
}

/// A session over `inner`, with `extra` added to the coding line.
fn finish(
    source: Arc<Source>,
    format: Format,
    inner: Inner,
    anim: Option<AnimationInfo>,
    extra: String,
    page: Option<Paged>,
) -> Result<Arc<dyn Session>> {
    let info = match &inner {
        Inner::Plain(d) => image_info(format, d.info(), &extra),
        Inner::Gif(g) => {
            let mut i = image_info(format, bm::Decode::info(g.as_ref()), &extra);
            // Regions come from the cached first-frame indices once the first decode ran.
            i.fast_regions = true;
            i
        }
        Inner::Session(s) | Inner::Ani(_, s) => {
            let mut i = s.info().clone();
            i.format = format;
            i.coding = format!("{}{extra}", i.coding);
            i.fast_regions = false;
            i
        }
    };
    Ok(Arc::new(BitmapSession {
        source,
        info,
        inner,
        anim,
        levels: Mutex::new(Vec::new()),
        page,
    }))
}

/// Page `page` of a parsed icon file: its entries, largest first, are the pages.
fn open_ico_page(
    source: Arc<Source>,
    ico: Arc<bm::ico::Ico>,
    page: u32,
) -> Result<Arc<dyn Session>> {
    let p = ico
        .pages
        .get(page as usize)
        .ok_or(DecodeError::Malformed("no such page".into()))?;
    let extra = format!(
        ", {} image{}",
        ico.count,
        if ico.count == 1 { "" } else { "s" }
    );
    let what = if ico.cursor { "cursor" } else { "icon" };
    let (inner, extra) = match &p.entry {
        bm::ico::Entry::Bmp(b) => (Inner::Plain(b.clone()), format!(" {what}{extra}")),
        bm::ico::Entry::Png { range, .. } => {
            let png = open_png(&source, source.bytes()[range.clone()].to_vec())?;
            (Inner::Session(png), format!(" {what} (PNG){extra}"))
        }
    };
    let format = BitmapDecoder(Kind::Ico).format();
    finish(
        source,
        format,
        inner,
        None,
        extra,
        Some(Paged::Ico(IcoPage { ico, index: page })),
    )
}

/// Image `page` of a multi-image PNM/PAM file, its header at `offsets[page]`.
fn open_pnm_page(
    source: Arc<Source>,
    offsets: Arc<Vec<usize>>,
    page: u32,
) -> Result<Arc<dyn Session>> {
    let off = *offsets
        .get(page as usize)
        .ok_or(DecodeError::Malformed("no such page".into()))?;
    let pnm = bm::pnm::Pnm::parse_at(source.bytes(), off).map_err(map_err)?;
    let extra = format!(", image {} of {}", page + 1, offsets.len());
    let format = BitmapDecoder(Kind::Pnm).format();
    finish(
        source,
        format,
        Inner::Plain(Box::new(pnm)),
        None,
        extra,
        Some(Paged::Pnm {
            offsets,
            index: page,
        }),
    )
}

/// Which image of a multi-image file a session shows.
enum Paged {
    Ico(IcoPage),
    /// A multi-image PNM/PAM file: where each image's header starts.
    Pnm {
        offsets: Arc<Vec<usize>>,
        index: u32,
    },
}

/// Where an ICO/CUR session's image sits in its file.
struct IcoPage {
    ico: Arc<bm::ico::Ico>,
    index: u32,
}

fn image_info(format: Format, i: &bm::Info, extra: &str) -> ImageInfo {
    ImageInfo {
        format,
        width: i.width,
        height: i.height,
        orientation: Orientation::NORMAL,
        icc_profile: None,
        has_alpha: i.has_alpha,
        bit_depth: i.bit_depth,
        progressive: false,
        coding: format!("{}{extra}", i.coding),
        embedded_preview: None,
        fast_regions: i.random_access,
        scales: &[1, 2, 4, 8],
    }
}

fn sub_source(parent: &Source, bytes: Vec<u8>) -> Arc<Source> {
    Arc::new(Source::from_vec(parent.path().to_path_buf(), bytes))
}

fn open_png(parent: &Source, bytes: Vec<u8>) -> Result<Arc<dyn Session>> {
    crate::png_dec::PngDecoder.open(sub_source(parent, bytes))
}

/// Open one ICO/CUR image (an ANI frame).
fn open_ico(parent: &Source, bytes: &[u8]) -> Result<Arc<dyn Session>> {
    BitmapDecoder(Kind::Ico).open(sub_source(parent, bytes.to_vec()))
}

enum Inner {
    Plain(Box<dyn bm::Decode>),
    Gif(Arc<bm::Gif>),
    /// A PNG inside an ICO.
    Session(Arc<dyn Session>),
    /// An animated cursor and its first frame.
    Ani(Arc<bm::ani::Ani>, Arc<dyn Session>),
}

struct BitmapSession {
    source: Arc<Source>,
    info: ImageInfo,
    inner: Inner,
    anim: Option<AnimationInfo>,
    /// Whole levels of sequential formats.
    levels: Mutex<Vec<Arc<Raster>>>,
    /// The icon file and entry (ICO/CUR) or the image of a multi-image PNM; `None` for other
    /// formats, ANI and single-image PNM.
    page: Option<Paged>,
}

fn to_raster(img: bm::Image, scale: u32, origin: (u32, u32)) -> Raster {
    let layout = match img.format {
        bm::PixelFormat::Gray8 => PixelLayout::Gray,
        bm::PixelFormat::Rgba8 => PixelLayout::Rgba,
        bm::PixelFormat::Gray16 => PixelLayout::Gray16,
        bm::PixelFormat::GrayAlpha16 => PixelLayout::GrayAlpha16,
        bm::PixelFormat::Rgba16 => PixelLayout::Rgba16(HdrEncoding::SRGB),
    };
    let bpp = img.format.bytes_per_pixel();
    Raster {
        width: img.width,
        height: img.height,
        origin,
        scale,
        layout,
        planes: vec![RasterPlane {
            width: img.width,
            height: img.height,
            stride: img.width as usize * bpp,
            data: img.data,
            h_div: 1,
            v_div: 1,
        }],
        truncated: img.truncated,
    }
}

impl BitmapSession {
    fn decoder(&self) -> Option<&dyn bm::Decode> {
        match &self.inner {
            Inner::Plain(d) => Some(d.as_ref()),
            Inner::Gif(g) => Some(g.as_ref()),
            Inner::Session(_) | Inner::Ani(..) => None,
        }
    }

    fn direct(
        &self,
        d: &dyn bm::Decode,
        scale: u32,
        region: Option<Rect>,
        parallel: bool,
        cancel: &CancelToken,
    ) -> Result<Raster> {
        let opts = bm::Options {
            scale,
            region: region.map(|r| bm::Region {
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h,
            }),
            parallel,
        };
        let img = d
            .decode_wide(self.source.bytes(), &opts, &|| cancel.is_cancelled())
            .map_err(map_err)?;
        let origin = region.map_or((0, 0), |r| (r.x, r.y));
        Ok(to_raster(img, scale, origin))
    }

    /// Bytes per output pixel.
    fn pixel_bytes(&self) -> usize {
        self.decoder().map_or(4, |d| match d.wide_format() {
            Some(f) => f.bytes_per_pixel(),
            None if d.info().gray && !d.info().has_alpha => 1,
            None => 4,
        })
    }

    fn level_bytes(&self, scale: u32) -> usize {
        self.info.width.div_ceil(scale) as usize
            * self.info.height.div_ceil(scale) as usize
            * self.pixel_bytes()
    }
}

impl Session for BitmapSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn colour_model(&self) -> Option<&'static str> {
        match &self.inner {
            Inner::Session(s) | Inner::Ani(_, s) => s.colour_model(),
            _ => Some(match self.decoder()?.info().model {
                bm::Model::Grey => "Greyscale",
                bm::Model::Rgb => "RGB",
                bm::Model::Palette => "Palette",
            }),
        }
    }
    /// An alpha channel (TGA's premultiplied when its extension area says so), or a
    /// transparent index, AND mask or palette alpha.
    fn alpha(&self) -> Option<Alpha> {
        let i = match &self.inner {
            Inner::Session(s) | Inner::Ani(_, s) => return s.alpha(),
            _ => self.decoder()?.info(),
        };
        if !i.has_alpha {
            return None;
        }
        Some(match i.alpha {
            bm::AlphaKind::Key => Alpha::Transparency,
            _ if i.model == bm::Model::Palette => Alpha::Transparency,
            bm::AlphaKind::Premultiplied => Alpha::Premultiplied,
            bm::AlphaKind::Straight => Alpha::Straight,
        })
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }

    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        let scale = req.scale.clamp(1, 8);
        let d = match &self.inner {
            Inner::Session(s) | Inner::Ani(_, s) => return s.decode(req, cancel),
            _ => self.decoder().expect("bitmap decoder"),
        };
        if self.info.fast_regions {
            return self.direct(d, scale, req.region, req.parallel, cancel);
        }
        // Sequential: decode the whole level once and crop regions from it.
        if let Some(l) = self.levels.lock().iter().find(|l| l.scale == scale) {
            return Ok(match req.region {
                Some(r) => crate::jpeg::crop(l, r),
                None => (**l).clone(),
            });
        }
        if self.level_bytes(scale) > LEVEL_CACHE_MAX {
            return self.direct(d, scale, req.region, req.parallel, cancel);
        }
        let level = Arc::new(self.direct(d, scale, None, req.parallel, cancel)?);
        {
            let mut ls = self.levels.lock();
            if !ls.iter().any(|l| l.scale == scale) {
                ls.push(level.clone());
            }
        }
        Ok(match req.region {
            Some(r) => crate::jpeg::crop(&level, r),
            None => (*level).clone(),
        })
    }

    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let scale = req.scale.clamp(1, 8);
        let out = match req.region {
            Some(r) => r.w as u64 * r.h as u64 * self.pixel_bytes().max(4) as u64,
            None => self.level_bytes(scale) as u64,
        };
        let mut m = out;
        if !self.info.fast_regions {
            // The cached level plus the decoder's working state (RLE index buffers, or the
            // full-resolution samples of plain 16-bit PNM).
            let wide = self.decoder().and_then(|d| d.wide_format()).is_some();
            let work = if wide { self.pixel_bytes() as u64 } else { 1 };
            m += self.level_bytes(scale) as u64
                + self.info.width as u64 * self.info.height as u64 * work;
        }
        if let Inner::Gif(_) = self.inner {
            // First-frame palette indices.
            m += self.info.width as u64 * self.info.height as u64;
        }
        m
    }

    fn memory_bytes(&self) -> usize {
        let levels: usize = self.levels.lock().iter().map(|l| l.memory_bytes()).sum();
        levels
            + match &self.inner {
                Inner::Gif(g) => g.memory_bytes(),
                Inner::Session(s) | Inner::Ani(_, s) => s.memory_bytes(),
                Inner::Plain(_) => 0,
            }
    }

    /// Drops the cached levels, what an embedded PNG's session keeps (ICO, ANI), and a GIF's
    /// first-frame indices. Those are 1 byte per pixel (23 MiB for 24 MP) and every level and
    /// region renders from them (a 512-px tile in ~0.3 ms), but they are re-created by one LZW
    /// pass over the frame (~0.2 s for 24 MP), so under memory pressure they go like the other
    /// decoders' whole levels. Playback ([`Session::frames`]) decodes its own frames.
    fn trim_memory(&self) -> usize {
        crate::whole::drop_levels(&self.levels)
            + match &self.inner {
                Inner::Gif(g) => g.trim(),
                Inner::Session(s) | Inner::Ani(_, s) => s.trim_memory(),
                Inner::Plain(_) => 0,
            }
    }

    fn animation(&self) -> Option<&AnimationInfo> {
        self.anim.as_ref()
    }

    fn page_count(&self) -> u32 {
        match &self.page {
            Some(Paged::Ico(p)) => p.ico.pages.len() as u32,
            Some(Paged::Pnm { offsets, .. }) => offsets.len() as u32,
            None => 1,
        }
    }
    fn page_index(&self) -> u32 {
        match &self.page {
            Some(Paged::Ico(p)) => p.index,
            Some(Paged::Pnm { index, .. }) => *index,
            None => 0,
        }
    }
    fn page_label(&self) -> Option<PageLabel> {
        match self.page.as_ref()? {
            Paged::Ico(p) => {
                let e = &p.ico.pages[p.index as usize];
                Some(PageLabel::Image {
                    width: e.width,
                    height: e.height,
                    bits: Some(e.bits),
                })
            }
            Paged::Pnm { .. } => Some(PageLabel::Image {
                width: self.info.width,
                height: self.info.height,
                bits: None,
            }),
        }
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        match &self.page {
            Some(Paged::Ico(p)) => open_ico_page(self.source.clone(), p.ico.clone(), page),
            Some(Paged::Pnm { offsets, .. }) => {
                open_pnm_page(self.source.clone(), offsets.clone(), page)
            }
            None => Err(DecodeError::Unsupported("no pages".into())),
        }
    }

    fn frames(&self, scale: u32) -> Result<Box<dyn Frames>> {
        let scale = scale.clamp(1, 8);
        match &self.inner {
            Inner::Gif(g) => Ok(Box::new(GifFrames {
                source: self.source.clone(),
                anim: bm::gif::Animator::new(g.clone()),
                width: self.info.width,
                height: self.info.height,
                scale,
            })),
            Inner::Ani(ani, _) => Ok(Box::new(AniFrames {
                source: self.source.clone(),
                ani: ani.clone(),
                width: self.info.width.div_ceil(scale),
                height: self.info.height.div_ceil(scale),
                scale,
                step: 0,
                cache: vec![None; ani.frames.len()],
            })),
            _ => Err(DecodeError::Unsupported("not animated".into())),
        }
    }
}

struct GifFrames {
    source: Arc<Source>,
    anim: bm::gif::Animator,
    width: u32,
    height: u32,
    scale: u32,
}

impl Frames for GifFrames {
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame>> {
        let index = self.anim.position() as u32;
        let c = &|| cancel.is_cancelled();
        let (canvas, delay_cs, truncated) = match self.anim.next_frame(self.source.bytes(), c)? {
            Ok(f) => f,
            Err(e) => return Some(Err(map_err(e))),
        };
        let opts = bm::Options {
            scale: self.scale,
            region: None,
            parallel: true,
        };
        let r = bm::gif::scale_rgba(canvas, self.width, self.height, &opts, c).map_err(map_err);
        Some(r.map(|mut img| {
            img.truncated |= truncated;
            Frame {
                index,
                raster: to_raster(img, self.scale, (0, 0)),
                delay: Duration::from_millis(delay_cs as u64 * 10),
            }
        }))
    }

    fn rewind(&mut self) -> Result<()> {
        self.anim.rewind();
        Ok(())
    }
}

struct AniFrames {
    source: Arc<Source>,
    ani: Arc<bm::ani::Ani>,
    /// Canvas size at the scale (the first frame's size; other frames are placed top-left).
    width: u32,
    height: u32,
    scale: u32,
    step: usize,
    /// Decoded frames (cursor frames are small; an ANI rarely has more than a few dozen).
    cache: Vec<Option<Arc<Raster>>>,
}

impl AniFrames {
    fn frame(&mut self, i: usize, cancel: &CancelToken) -> Result<Arc<Raster>> {
        if let Some(r) = &self.cache[i] {
            return Ok(r.clone());
        }
        let s = open_ico(&self.source, self.ani.frame_data(self.source.bytes(), i))?;
        let r = s.decode(&DecodeRequest::full(self.scale), cancel)?;
        let r = Arc::new(fit_rgba(&r, self.width, self.height));
        self.cache[i] = Some(r.clone());
        Ok(r)
    }
}

/// `r` as RGBA on a transparent canvas of `w`×`h` (top-left aligned, clipped).
fn fit_rgba(r: &Raster, w: u32, h: u32) -> Raster {
    let p = &r.planes[0];
    let mut data = vec![0u8; w as usize * h as usize * 4];
    for y in 0..h.min(r.height) as usize {
        let src = &p.data[y * p.stride..];
        let dst = &mut data[y * w as usize * 4..];
        for x in 0..w.min(r.width) as usize {
            let px = match r.layout {
                PixelLayout::Gray => [src[x], src[x], src[x], 255],
                _ => [src[x * 4], src[x * 4 + 1], src[x * 4 + 2], src[x * 4 + 3]],
            };
            dst[x * 4..x * 4 + 4].copy_from_slice(&px);
        }
    }
    Raster {
        width: w,
        height: h,
        origin: (0, 0),
        scale: r.scale,
        layout: PixelLayout::Rgba,
        planes: vec![RasterPlane {
            width: w,
            height: h,
            stride: w as usize * 4,
            data,
            h_div: 1,
            v_div: 1,
        }],
        truncated: r.truncated,
    }
}

impl Frames for AniFrames {
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame>> {
        let step = *self.ani.steps.get(self.step)?;
        if cancel.is_cancelled() {
            return Some(Err(DecodeError::Cancelled));
        }
        let index = self.step as u32;
        self.step += 1;
        Some(self.frame(step.frame, cancel).map(|r| Frame {
            index,
            raster: (*r).clone(),
            // Jiffies are 1/60 s.
            delay: Duration::from_micros(step.jiffies as u64 * 1_000_000 / 60),
        }))
    }

    fn rewind(&mut self) -> Result<()> {
        self.step = 0;
        Ok(())
    }
}
