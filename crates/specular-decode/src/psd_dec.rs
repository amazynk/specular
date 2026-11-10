//! Photoshop PSD / PSB through the in-tree `specular-psd` reader.
//!
//! The merged composite is decoded once (row-parallel) to 8-bit grey or RGBA, or, for 16-bit
//! RGB and grey-like files, kept at 16 bits ([`PixelLayout::Gray16`],
//! [`PixelLayout::GrayAlpha16`] or sRGB-coded [`PixelLayout::Rgba16`]) so the inspector reads
//! the file's values and grey stays compact; coarser levels are box-filtered from it and
//! regions are cropped. The embedded JPEG thumbnail is the quick
//! first picture. CMYK and Lab composites are colour-managed while decoding (`colour.rs`; the
//! embedded CMYK profile, else a generic one); only RGB files pass their ICC profile on. Files
//! saved without a merged image ("Maximize compatibility" off) show the composite of their
//! layers (`specular_psd::Psd::composites_layers`: pixel, fill and shape layers with their
//! masks; the encoding row says so and how many adjustment layers it leaves out); with nothing
//! to draw they show their thumbnail and fail to decode with a message saying why.
//!
//! Pages: page 0 is the merged image, then each artboard (the merged image cropped to it,
//! named after it), then the layers with pixels of their own, top-most first as the Layers
//! panel lists them (hidden ones too; groups, adjustment and fill layers are not pages). The
//! info panel's header group lists the artboards and, for a composite built from the
//! layers, what it leaves out or draws only approximately. A layer page is the layer alone on the document canvas, transparent around
//! its rectangle, with its transparency, user mask, opacity and fill opacity; blend mode,
//! clipping, effects and vector masks are not applied. The layer records are parsed once at
//! open and shared by every page ([`Session::open_page`] neither re-reads nor re-parses); a
//! page decodes only its own layer's channels, through the same colour conversion as the
//! merged image, and stops between row bands when cancelled.

use crate::whole::{self, Levels};
use crate::{
    Alpha, DecodeError, DecodeRequest, Decoder, Format, HeaderFacts, ImageInfo, PageLabel, Result,
    Session,
};
use specular_core::{CancelToken, HdrEncoding, Orientation, PixelLayout, Raster, Source};
use specular_jpeg as sj;
use specular_psd as sp;
use std::sync::Arc;

pub struct PsdDecoder;

fn layout(f: sp::PixelFormat) -> PixelLayout {
    match f {
        sp::PixelFormat::Gray8 => PixelLayout::Gray,
        sp::PixelFormat::Rgba8 => PixelLayout::Rgba,
        sp::PixelFormat::Gray16 => PixelLayout::Gray16,
        sp::PixelFormat::GrayAlpha16 => PixelLayout::GrayAlpha16,
        sp::PixelFormat::Rgba16 => PixelLayout::Rgba16(HdrEncoding::SRGB),
    }
}

fn raster(img: sp::Image) -> Raster {
    whole::interleaved(
        img.width,
        img.height,
        layout(img.format),
        img.data,
        img.truncated,
    )
}

/// Native-depth planes to a raster: CMYK and Lab colour-managed (`icc`, else a generic CMYK
/// profile), 16-bit RGB and grey-like kept at 16 bits, everything else 8-bit.
fn planes_raster(planes: &sp::Planes, icc: Option<&[u8]>, parallel: bool) -> Raster {
    if matches!(planes.mode, sp::ColorMode::Cmyk | sp::ColorMode::Lab) {
        if let Some(rgba) = crate::colour::psd_rgba(planes, icc) {
            return whole::interleaved(
                planes.width,
                planes.height,
                PixelLayout::Rgba,
                rgba,
                planes.truncated,
            );
        }
        return raster(planes.to_rgba8());
    }
    if let Some(img) = planes.to_wide(parallel) {
        return raster(img);
    }
    raster(planes.to_rgba8())
}

/// The profile handed on with an RGB composite. A 32-bit document holds linear light, which
/// the decoder puts through the sRGB curve, while its profile describes the linear values
/// (Photoshop's "Linear RGB Profile"): handing that on would apply the curve twice. Its
/// colorants travel with the sRGB curve instead, or nothing when they are sRGB's; a 32-bit
/// profile that is not a matrix/curves one is dropped (shown as sRGB).
fn shown_icc(icc: &[u8], depth: u8) -> Option<Vec<u8>> {
    if depth != 32 {
        return Some(icc.to_vec());
    }
    let p = specular_color::icc::Profile::parse(icc).ok()?;
    let m = p.matrix.filter(|_| p.is_matrix_shaper())?;
    if m.max_diff(&specular_color::math::bt709_to_pcs()) < 2e-3 {
        return None;
    }
    Some(specular_color::profiles::matrix_srgb_curve_icc(
        &m,
        "32-bit document colorants, sRGB curve",
    ))
}

fn map_err(e: sp::Error) -> DecodeError {
    match e {
        sp::Error::NotPsd => DecodeError::Malformed("not a PSD/PSB file".into()),
        sp::Error::Unsupported(s) => DecodeError::Unsupported(s),
        sp::Error::Truncated(s) => DecodeError::Truncated(s),
        sp::Error::Malformed(s) => DecodeError::Malformed(s),
        sp::Error::TooLarge => DecodeError::TooLarge,
        sp::Error::Cancelled => DecodeError::Cancelled,
        sp::Error::NoComposite(sp::Composite::NotMaximized) => DecodeError::Unsupported(
            "this Photoshop file has no merged image: it was saved with \"Maximize \
             compatibility\" off, and it has no layers to build one from"
                .into(),
        ),
        sp::Error::NoComposite(_) => {
            DecodeError::Unsupported("this Photoshop file has no merged image".into())
        }
    }
}

impl Decoder for PsdDecoder {
    fn format(&self) -> Format {
        Format::Psd
    }
    fn probe(&self, head: &[u8]) -> bool {
        sp::probe(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        // PhotoDeluxe (`.pdd`) and Photoshop templates (`.psdt`) are plain PSD.
        &["psd", "psb", "pdd", "psdt"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let data = source.bytes();
        let psd = sp::parse(data).map_err(map_err)?;
        let i = psd.info();
        let mut coding = format!(
            "{} {:?} {}-bit",
            match i.version {
                sp::Version::Psd => "PSD",
                sp::Version::Psb => "PSB",
            },
            i.mode,
            i.depth
        );
        if let Some(c) = i.compression {
            coding += match c {
                sp::Compression::Raw => ", uncompressed",
                sp::Compression::Rle => ", RLE",
                sp::Compression::Zip => ", ZIP",
                sp::Compression::ZipPrediction => ", ZIP with prediction",
            };
        }
        if i.layer_count > 0 {
            coding += &format!(
                ", {} layer{}",
                i.layer_count,
                if i.layer_count == 1 { "" } else { "s" }
            );
        }
        if psd.composites_layers() {
            coding += if i.composite == sp::Composite::Blank {
                ", blank merged image: composited from its layers"
            } else {
                ", no merged image: composited from its layers"
            };
            match psd.undrawn_layers(data) {
                0 => {}
                1 => coding += " (1 adjustment layer not drawn)",
                n => coding += &format!(" ({n} adjustment layers not drawn)"),
            }
        } else if i.composite != sp::Composite::Present {
            coding += ", no merged image";
        }
        let thumbnail = psd.resources().thumbnail.clone();
        let info = ImageInfo {
            format: Format::Psd,
            width: i.width,
            height: i.height,
            orientation: Orientation::NORMAL,
            icc_profile: (i.mode == sp::ColorMode::Rgb)
                .then(|| psd.icc(data).and_then(|b| shown_icc(b, i.depth)))
                .flatten()
                .map(Arc::new),
            has_alpha: i.is_transparent(),
            bit_depth: i.depth,
            progressive: false,
            coding,
            embedded_preview: thumbnail
                .as_ref()
                .filter(|t| t.format == sp::ThumbnailFormat::Jpeg)
                .map(|t| t.data.clone()),
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        let layers = psd.pixel_layers().into();
        let artboards: Arc<[sp::Artboard]> = psd.artboards(data).into();
        // A stored merged image next to layers: page 1 shows the layers composited.
        let composed_page = !psd.composites_layers()
            && i.composite == sp::Composite::Present
            && i.layer_count > 0
            && psd.can_compose_layers();
        let composed_header = composed_page
            .then(|| header_facts(&psd, data, &artboards, true))
            .flatten();
        let header = header_facts(&psd, data, &artboards, false);
        Ok(Arc::new(PsdSession {
            file: Arc::new(PsdFile {
                source,
                psd,
                thumbnail,
                layers,
                artboards,
                composed_page,
                header,
                composed_header,
                merged: info.clone(),
            }),
            page: 0,
            info,
            levels: Levels::new(),
        }))
    }
}

/// What every page of one file shares: the bytes and the records parsed at open.
struct PsdFile {
    source: Arc<Source>,
    psd: sp::Psd,
    thumbnail: Option<sp::Thumbnail>,
    /// Indices into the layer records of the layer pages, the first after the artboards.
    layers: Arc<[usize]>,
    /// The artboard pages, the first after the merged image (and the composed page).
    artboards: Arc<[sp::Artboard]>,
    /// Page 1 is the composite built from the layers, beside a stored merged image.
    composed_page: bool,
    /// The info panel's header facts (artboards, what a composite leaves out).
    header: Option<HeaderFacts>,
    /// The composed page's: what the composite of the layers leaves out too.
    composed_header: Option<HeaderFacts>,
    /// The merged image's facts; a layer page's are derived from them.
    merged: ImageInfo,
}

struct PsdSession {
    file: Arc<PsdFile>,
    /// 0 for the merged image, then the composite built from the layers (when the file
    /// stores a merged image), the artboards, then the layer pages.
    page: u32,
    info: ImageInfo,
    levels: Levels,
}

/// The info panel's header facts: the artboards, and for a composite built from the layers
/// what it leaves out or draws only approximately (Photoshop's names for them).
/// `composed`: for the page that composites the layers beside a stored merged image.
fn header_facts(
    psd: &sp::Psd,
    data: &[u8],
    boards: &[sp::Artboard],
    composed: bool,
) -> Option<HeaderFacts> {
    let mut summary = Vec::new();
    if !boards.is_empty() {
        let list: Vec<String> = boards
            .iter()
            .map(|a| {
                format!(
                    "{} ({} × {})",
                    a.name,
                    a.right.saturating_sub(a.left),
                    a.bottom.saturating_sub(a.top)
                )
            })
            .collect();
        summary.push(("artboards", list.join(", ")));
    }
    if psd.composites_layers() || composed {
        let notes = psd.composite_notes(data);
        if !notes.not_drawn.is_empty() {
            summary.push(("not-drawn", notes.not_drawn.join(", ")));
        }
        if !notes.approximate.is_empty() {
            summary.push(("approximated", notes.approximate.join(", ")));
        }
    }
    (!summary.is_empty()).then_some(HeaderFacts {
        summary,
        fields: Vec::new(),
    })
}

impl PsdFile {
    /// The first artboard page.
    fn first_board(&self) -> usize {
        1 + usize::from(self.composed_page)
    }
}

impl PsdSession {
    /// Whether this page is the composite built from the layers beside a stored merged
    /// image.
    fn composed(&self) -> bool {
        self.file.composed_page && self.page == 1
    }

    /// The artboard this page shows, if it is an artboard page.
    fn artboard(&self) -> Option<&sp::Artboard> {
        self.file
            .artboards
            .get((self.page as usize).checked_sub(self.file.first_board())?)
    }

    /// The layer record this page shows, None on the merged image and artboards.
    fn layer(&self) -> Option<&sp::Layer> {
        let first = self.file.first_board() + self.file.artboards.len();
        let i = *self
            .file
            .layers
            .get((self.page as usize).checked_sub(first)?)?;
        self.file.psd.layers().get(i)
    }

    /// The layer record index this page shows.
    fn layer_index(&self) -> Option<usize> {
        let first = self.file.first_board() + self.file.artboards.len();
        self.file
            .layers
            .get((self.page as usize).checked_sub(first)?)
            .copied()
    }

    fn decode_thumbnail(&self, t: &sp::Thumbnail) -> Result<Raster> {
        let bytes = self
            .file
            .source
            .bytes()
            .get(t.data.clone())
            .ok_or_else(|| DecodeError::Truncated("thumbnail".into()))?;
        let mut rgba = match t.format {
            sp::ThumbnailFormat::Jpeg => {
                let d = sj::decode(bytes, &sj::DecodeOptions::default())
                    .map_err(crate::jpeg::map_err)?;
                if !t.bgr {
                    return Ok(crate::jpeg::to_raster(d, 0));
                }
                let (w, h) = (d.width, d.height);
                (w, h, sj::planes_to_rgba(&d))
            }
            sp::ThumbnailFormat::RawRgb { row_bytes } => {
                let (w, h) = (t.width as usize, t.height as usize);
                let rb = (row_bytes as usize).max(w * 3);
                if bytes.len() < rb * h.saturating_sub(1) + w * 3 || w == 0 || h == 0 {
                    return Err(DecodeError::Truncated("thumbnail".into()));
                }
                let mut out = Vec::with_capacity(w * h * 4);
                for y in 0..h {
                    for p in bytes[y * rb..y * rb + w * 3].chunks_exact(3) {
                        out.extend_from_slice(&[p[0], p[1], p[2], 255]);
                    }
                }
                (t.width, t.height, out)
            }
        };
        if t.bgr {
            for p in rgba.2.chunks_exact_mut(4) {
                p.swap(0, 2);
            }
        }
        let mut r = whole::interleaved(rgba.0, rgba.1, PixelLayout::Rgba, rgba.2, false);
        r.scale = 0;
        Ok(r)
    }

    fn decode_full(&self, parallel: bool, cancel: &CancelToken) -> Result<Raster> {
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let psd = &self.file.psd;
        let data = self.file.source.bytes();
        let opts = sp::Options { parallel };
        // Both canvases are alive at once: the native-depth planes and the converted page. A
        // header alone can ask for 2^29 pixels of each.
        let _canvases = specular_core::budget::reserve(self.canvases_bytes())?;
        if let Some(a) = self.artboard() {
            let planes = psd
                .decode_artboard_planes(data, a, &opts)
                .map_err(map_err)?;
            if cancel.is_cancelled() {
                return Err(DecodeError::Cancelled);
            }
            return Ok(planes_raster(&planes, psd.icc(data), parallel));
        }
        if let Some(index) = self.layer_index() {
            let planes = psd
                .decode_layer_planes(data, index, &opts, Some(cancel.flag()))
                .map_err(map_err)?;
            if cancel.is_cancelled() {
                return Err(DecodeError::Cancelled);
            }
            return Ok(planes_raster(&planes, psd.icc(data), parallel));
        }
        if self.composed() {
            let planes = psd.compose_layers(data, &opts).map_err(map_err)?;
            if cancel.is_cancelled() {
                return Err(DecodeError::Cancelled);
            }
            return Ok(planes_raster(&planes, psd.icc(data), parallel));
        }
        if matches!(psd.info().mode, sp::ColorMode::Cmyk | sp::ColorMode::Lab) {
            // Colour-managed through the embedded (or generic CMYK) profile.
            let planes = psd.decode_planes(data, &opts).map_err(map_err)?;
            return Ok(planes_raster(&planes, psd.icc(data), parallel));
        }
        if let Some(img) = psd.decode_wide(data, &opts).map_err(map_err)? {
            return Ok(raster(img));
        }
        Ok(raster(psd.decode_rgba8(data, &opts).map_err(map_err)?))
    }

    /// The page at full size plus its native-depth planes (a layer: its colour channels and
    /// transparency over the whole canvas): what a full decode holds at its peak.
    fn canvases_bytes(&self) -> u64 {
        let i = self.file.psd.info();
        let cc = i.mode.color_channels(i.channels) as u64;
        let whole = self.layer_index().is_none();
        let channels = if whole { u64::from(i.channels) } else { cc + 1 };
        let mut planes = channels * u64::from(i.depth.max(8) / 8);
        if whole && (self.file.psd.composites_layers() || self.composed()) {
            // Built from the layers: a float canvas, a float layer and its native planes
            // (two of each inside an isolated group).
            planes = (cc + 1) * (4 * 4 + 2 * u64::from(i.depth.max(8) / 8));
        }
        // An artboard is cut from the whole canvas.
        whole::level_bytes(i.width, i.height, 1, self.pixel_bytes() + planes)
    }

    /// Bytes per pixel of the decoded page.
    fn pixel_bytes(&self) -> u64 {
        let i = self.file.psd.info();
        if self.layer_index().is_none() && !self.composed() {
            return self
                .file
                .psd
                .wide_format()
                .map_or(4, |f| f.bytes_per_pixel() as u64);
        }
        // Layers always carry alpha: 16-bit RGB → RGBA16, 16-bit grey-like → grey + alpha.
        match (i.depth, i.mode) {
            (16, sp::ColorMode::Rgb) => 8,
            _ => 4,
        }
    }
}

impl Session for PsdSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(match self.file.psd.info().mode {
            // 1-bit black and white: the bit depth says the rest.
            sp::ColorMode::Bitmap | sp::ColorMode::Grayscale => "Greyscale",
            sp::ColorMode::Indexed => "Palette",
            sp::ColorMode::Rgb => "RGB",
            sp::ColorMode::Cmyk => "CMYK",
            sp::ColorMode::Multichannel => "Multichannel",
            sp::ColorMode::Duotone => "Duotone",
            sp::ColorMode::Lab => "Lab",
        })
    }
    /// The merged image's alpha channel (straight), else an indexed transparent entry; a
    /// layer page's transparency is straight.
    fn alpha(&self) -> Option<Alpha> {
        if self.layer_index().is_some() || self.composed() {
            return Some(Alpha::Straight);
        }
        let i = self.file.psd.info();
        if i.has_alpha {
            Some(Alpha::Straight)
        } else {
            i.is_transparent().then_some(Alpha::Transparency)
        }
    }
    fn source(&self) -> &Arc<Source> {
        &self.file.source
    }
    /// The embedded thumbnail shows the merged image, so layer pages have no preview.
    fn decode_preview(&self) -> Option<Result<Raster>> {
        if self.page > 0 {
            return None;
        }
        Some(self.decode_thumbnail(self.file.thumbnail.as_ref()?))
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| self.decode_full(req.parallel, cancel),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        let s = req.scale.max(1);
        let bpp = self.pixel_bytes();
        if self.levels.has(s) {
            return whole::level_bytes(w, h, s, bpp);
        }
        let mut m = whole::level_bytes(w, h, s, bpp);
        if !self.levels.has(1) {
            m += self.canvases_bytes();
        }
        m
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
    }
    /// Drops the cached levels (the next request composites the image again).
    fn trim_memory(&self) -> usize {
        self.levels.clear()
    }
    fn page_count(&self) -> u32 {
        self.file.first_board() as u32
            + self.file.artboards.len() as u32
            + self.file.layers.len() as u32
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        if self.page_count() == 1 {
            return None;
        }
        if self.composed() {
            return Some(PageLabel::FromLayers);
        }
        if let Some(a) = self.artboard() {
            return Some(PageLabel::Sheet {
                name: a.name.as_str().into(),
                model: false,
            });
        }
        Some(match self.layer() {
            None => PageLabel::Merged,
            Some(l) => PageLabel::DocumentLayer {
                name: l.name.as_str().into(),
                index: self.page + 1
                    - self.file.first_board() as u32
                    - self.file.artboards.len() as u32,
                hidden: !l.visible,
            },
        })
    }
    fn header(&self) -> Option<&HeaderFacts> {
        if self.composed() {
            return self.file.composed_header.as_ref();
        }
        self.file.header.as_ref()
    }
    /// Page 0 is the merged image, then the artboards, then the pixel layers top-most
    /// first: the parsed records are shared, nothing is read or parsed again.
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page >= self.page_count() {
            return Err(DecodeError::Malformed("no such page".into()));
        }
        let merged = &self.file.merged;
        let boards = self.file.artboards.len() as u32;
        let first = self.file.first_board();
        let info = if page == 0 {
            merged.clone()
        } else if self.file.composed_page && page == 1 {
            ImageInfo {
                has_alpha: true,
                embedded_preview: None,
                coding: format!("{}, composited from its layers", merged.coding),
                ..merged.clone()
            }
        } else if let Some(a) = (page as usize)
            .checked_sub(first)
            .and_then(|k| self.file.artboards.get(k))
        {
            let (w, h) = (merged.width as i64, merged.height as i64);
            let span = |a0: i32, a1: i32, n: i64| {
                let (x0, x1) = (i64::from(a0).clamp(0, n), i64::from(a1).clamp(0, n));
                (x1 - x0).max(1) as u32
            };
            ImageInfo {
                width: span(a.left, a.right, w),
                height: span(a.top, a.bottom, h),
                embedded_preview: None,
                coding: format!(
                    "{}, artboard {} of {boards}",
                    merged.coding,
                    page as usize + 1 - first
                ),
                ..merged.clone()
            }
        } else {
            ImageInfo {
                has_alpha: true,
                embedded_preview: None,
                coding: format!(
                    "{}, layer {} of {}",
                    merged.coding,
                    page as usize + 1 - first - boards as usize,
                    self.file.layers.len()
                ),
                ..merged.clone()
            }
        };
        Ok(Arc::new(PsdSession {
            file: self.file.clone(),
            page,
            info,
            levels: Levels::new(),
        }))
    }
}
