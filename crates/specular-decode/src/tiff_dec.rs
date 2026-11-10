//! TIFF / BigTIFF through the in-tree `specular-tiff` decoder.
//!
//! The session shows the first page; [`open_tiff_page`] opens any other page and
//! [`Session::page_count`] tells how many there are. Tiled images and images with small strips
//! decode regions directly (only the tiles / strips that intersect are read, in parallel);
//! images with large strips decode a whole level once, cache it and crop regions from it.
//! Pyramid levels (reduced-resolution IFDs and SubIFDs) serve the downscaled requests.
//!
//! Integer grey and RGB(A) deeper than 8 bits stay 16-bit ([`PixelLayout::Gray16`],
//! [`PixelLayout::GrayAlpha16`] or sRGB-coded [`PixelLayout::Rgba16`]), so the inspector reads
//! the file's values and grey stays compact. Float grey and RGB(A) (16, 24, 32 and 64-bit) is
//! HDR: linear half-float RGBA ([`PixelLayout::Rgba16F`], 1.0 = reference white, not clamped)
//! like OpenEXR, so exposure and tone mapping apply. Everything else is 8-bit grey or RGBA:
//! CMYK goes through the colour-managed hook
//! (`crate::colour`, the embedded profile or a generic one) and so does L*a*b* (CIELab, ICCLab,
//! ITULab; D50 to sRGB). A Lab page's embedded profile is not passed on: the samples are
//! device-independent and already converted, so an RGB profile wrongly attached to them must not
//! be applied again (the info panel still lists it from the file).

use crate::geo::{GeoFacts, GeoRef};
use crate::science::{NdPlane, PlaneCache, ScienceSummary};
use crate::{Alpha, DecodeError, DecodeRequest, Decoder, Format, ImageInfo, Rect, Result, Session};
use parking_lot::Mutex;
use specular_core::nd::Limits;
use specular_core::{
    CancelToken, HdrEncoding, Orientation, PixelLayout, Primaries, Raster, RasterPlane, Source,
};
use specular_tiff as tf;
use std::sync::Arc;

/// Whole levels cached for strip images above this size are not kept (decoded per request).
const LEVEL_CACHE_MAX: u64 = 768 << 20;
/// Strips this tall or shorter are read directly for regions.
const DIRECT_STRIP_ROWS: u32 = 256;
/// Georeferenced single-band images up to this many pixels show their values through the
/// science view (the full-resolution plane is held as `f32`).
const VALUES_MAX_PIXELS: u64 = 1 << 27;

pub struct TiffDecoder;

pub(crate) fn map_err(e: tf::Error) -> DecodeError {
    match e {
        tf::Error::Truncated(s) => DecodeError::Truncated(s.into()),
        tf::Error::Malformed(s) => DecodeError::Malformed(s.into()),
        tf::Error::Unsupported(s) => DecodeError::Unsupported(s),
        tf::Error::TooLarge => DecodeError::TooLarge,
        tf::Error::Cancelled => DecodeError::Cancelled,
        tf::Error::NoPage => DecodeError::Malformed("no such page".into()),
        tf::Error::BadRegion => DecodeError::Unsupported("region outside the image".into()),
    }
}

/// Whether the bytes start with a TIFF or BigTIFF header.
pub(crate) fn probe(head: &[u8]) -> bool {
    matches!(
        head.get(..4),
        Some(b"II*\0" | b"MM\0*" | b"II+\0" | b"MM\0+")
    )
}

impl Decoder for TiffDecoder {
    fn format(&self) -> Format {
        Format::Tiff
    }
    fn probe(&self, head: &[u8]) -> bool {
        probe(head)
    }
    /// Camera RAWs in TIFF containers belong to the RAW decoder.
    fn probe_file(&self, data: &[u8]) -> bool {
        probe(data) && !crate::raw_dec::is_raw(data)
    }
    fn extensions(&self) -> &'static [&'static str] {
        // Aperio SVS slides are tiled TIFF (JPEG or JPEG 2000 tiles, pyramid IFDs).
        // Zeiss LSM, MetaMorph STK and Molecular Dynamics GEL are TIFF too.
        &["tif", "tiff", "lsm", "stk", "gel"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        open_tiff_page(source, 0)
    }
}

/// Open page `page` (0-based) of a TIFF file.
pub fn open_tiff_page(source: Arc<Source>, page: usize) -> Result<Arc<dyn Session>> {
    let tiff = tf::Tiff::open(source.bytes()).map_err(map_err)?;
    open_parsed(source, tiff, page)
}

/// Page `page` of an already parsed file, for [`crate::scitiff_dec`]'s sessions.
pub(crate) fn open_page_of(
    source: Arc<Source>,
    tiff: tf::Tiff,
    page: usize,
) -> Result<Arc<dyn Session>> {
    open_parsed(source, tiff, page)
}

/// Page `page` of an already parsed file (the IFD chain is not walked again). Scientific
/// pages and stacks open in [`crate::scitiff_dec`]'s session.
fn open_parsed(source: Arc<Source>, tiff: tf::Tiff, page: usize) -> Result<Arc<dyn Session>> {
    let data = source.bytes();
    let pages = tiff.pages().len();
    let p = tiff
        .pages()
        .get(page)
        .ok_or(DecodeError::Malformed("no such page".into()))?;
    if p.ifd().complex {
        return crate::sar::open_tiff(source, tiff, page);
    }
    p.check_supported().map_err(map_err)?;
    let facts = match crate::scitiff_dec::open(&source, &tiff, page)? {
        crate::scitiff_dec::Opened::Science(s) => return Ok(s),
        crate::scitiff_dec::Opened::Picture(f) => f.map(|f| *f),
    };
    let ifd = p.ifd();
    let mut coding = ifd.description();
    let levels = p.level_sizes().len();
    if levels > 1 {
        coding += &format!(", {levels} levels");
    }
    if pages > 1 {
        coding += &format!(", page {} of {pages}", page + 1);
    }
    if tiff.big_tiff {
        coding = format!("BigTIFF {coding}");
    }
    let colour_model = colour_model(ifd);
    // ExtraSamples: 1 associated (premultiplied), 2 unassociated.
    let alpha = ifd.alpha.map(|(_, premultiplied)| {
        if premultiplied {
            Alpha::Premultiplied
        } else {
            Alpha::Straight
        }
    });
    let direct = ifd.tiled || ifd.unit_h <= DIRECT_STRIP_ROWS;
    let mut geo = ifd
        .geo
        .as_deref()
        .and_then(|g| crate::geo::from_geotiff(g, p.width(), p.height(), p.level_sizes()))
        // No GeoTIFF transform: a world file, `.prj` or `.aux.xml` beside the file.
        .or_else(|| {
            (page == 0 && crate::geo::on_disk(&source))
                .then(|| {
                    crate::geo::from_sidecars(
                        source.path(),
                        p.width(),
                        p.height(),
                        p.level_sizes(),
                        true,
                    )
                })
                .flatten()
                .map(|g| crate::geo::with_gdal_tags(g, ifd.geo.as_deref()))
        });
    let small = u64::from(p.width()) * u64::from(p.height()) <= VALUES_MAX_PIXELS;
    let values = geo.is_some() && shows_values(ifd) && small;
    // A plain page of measurements (a 16-bit camera frame, a float map): its values, unless
    // too large to hold, when only its facts stay.
    let mut facts = facts;
    let measured = match facts.take() {
        Some(f) if !f.raster_only && !values && shows_values(ifd) && small => Some(f),
        Some(mut f) => {
            f.raster_only = true;
            facts = Some(f);
            None
        }
        None => None,
    };
    let mut nodata = None;
    let mut scale_offset = None;
    if let Some(GeoRef {
        facts: GeoFacts::Raster(r),
        ..
    }) = geo.as_mut()
    {
        r.values = values;
        nodata = r.nodata;
        scale_offset = r.scale_offset;
    }
    let geo = geo.map(Arc::new);
    let science = measured.or_else(|| {
        let r = match geo.as_deref() {
            Some(GeoRef {
                facts: GeoFacts::Raster(r),
                ..
            }) => Some(r),
            _ => None,
        };
        values.then(|| ScienceSummary {
            width: p.width(),
            height: p.height(),
            sample: sample_name(ifd),
            unit: r.and_then(|r| r.unit.clone()),
            title: r.and_then(|r| r.band_name.clone()),
            geo: geo.clone(),
            ..Default::default()
        })
    });
    let info = ImageInfo {
        // GeoKeys make it a GeoTIFF (a sidecar georeference does not).
        format: if ifd
            .geo
            .as_deref()
            .is_some_and(|g| !g.key_directory.is_empty())
        {
            Format::GeoTiff
        } else {
            Format::Tiff
        },
        width: p.width(),
        height: p.height(),
        orientation: Orientation::from_exif(p.orientation()),
        icc_profile: (!ifd.photometric.is_lab())
            .then(|| p.icc_profile(data))
            .flatten()
            .map(|b| Arc::new(b.to_vec())),
        has_alpha: p.has_alpha(),
        bit_depth: ifd.stored_bits().min(255) as u8,
        progressive: false,
        coding,
        embedded_preview: None,
        fast_regions: direct,
        scales: &[1, 2, 4, 8],
    };
    Ok(Arc::new(TiffSession {
        source,
        tiff,
        page,
        pages: pages as u32,
        colour_model,
        alpha,
        info,
        levels: Mutex::new(Vec::new()),
        geo,
        values: science.map(|summary| Values {
            summary,
            nodata,
            scale_offset,
            planes: PlaneCache::new(),
            limits: Mutex::new(None),
            stretched: values,
        }),
        facts,
    }))
}

/// Whether a page holds one band of measurements (an elevation model, a temperature grid):
/// integer samples of 16 bits or more, or floats, not JPEG-compressed.
fn shows_values(ifd: &tf::Ifd) -> bool {
    use tf::Photometric as P;
    ifd.samples == 1
        && ifd.alpha.is_none()
        && !ifd.compression.is_jpeg()
        && matches!(ifd.photometric, P::MinIsBlack | P::MinIsWhite)
        && (ifd.sample_format == tf::SampleFormat::Float || ifd.bits >= 16)
}

/// "32-bit float", "16-bit signed", "16-bit unsigned".
pub(crate) fn sample_name(ifd: &tf::Ifd) -> String {
    let kind = match ifd.sample_format {
        tf::SampleFormat::Float => "float",
        tf::SampleFormat::Int => "signed",
        tf::SampleFormat::Uint => "unsigned",
    };
    // Integers by their stored width (17, 24, 64 bits are read as 32); floats as read
    // (SGI LogL is a 16-bit log code read as 32-bit float).
    let bits = if ifd.sample_format == tf::SampleFormat::Float {
        ifd.bits
    } else {
        ifd.stored_bits()
    };
    format!("{bits}-bit {kind}")
}

/// The stored colour model, as the info panel names it.
fn colour_model(ifd: &tf::Ifd) -> Option<&'static str> {
    use tf::Photometric as P;
    Some(match ifd.photometric {
        P::MinIsWhite | P::MinIsBlack | P::Mask => "Greyscale",
        P::Rgb => "RGB",
        P::Palette => "Palette",
        P::Cmyk => "CMYK",
        P::YCbCr => "YCbCr",
        P::CieLab | P::IccLab | P::ItuLab => "Lab",
        P::Other(_) => return None,
    })
}

struct TiffSession {
    source: Arc<Source>,
    tiff: tf::Tiff,
    page: usize,
    pages: u32,
    colour_model: Option<&'static str>,
    alpha: Option<Alpha>,
    info: ImageInfo,
    /// Whole levels of large-strip images.
    levels: Mutex<Vec<Arc<Raster>>>,
    /// The GeoTIFF georeference.
    geo: Option<Arc<GeoRef>>,
    /// Shown as values (a georeferenced single-band image).
    values: Option<Values>,
    /// A calibrated picture's pixel size and instrument facts (an 8-bit SEM image).
    facts: Option<ScienceSummary>,
}

/// Values the display limits are taken from at least (zscale samples 1000).
const LIMIT_SAMPLES: u64 = 4096;

/// A page shown as real values: the science view's planes, and the auto-stretched grey the
/// session decodes (thumbnails, the first frame, the CPU path).
struct Values {
    summary: ScienceSummary,
    nodata: Option<f64>,
    /// GDAL scale and offset.
    scale_offset: Option<(f64, f64)>,
    planes: PlaneCache,
    /// zscale limits from a reduced decode, for the grey.
    limits: Mutex<Option<Option<(f32, f32)>>>,
    /// Whether the session decodes the stretched grey (georeferenced values) rather than the
    /// samples themselves (a plain page of measurements keeps its 16-bit raster).
    stretched: bool,
}

fn to_raster(d: tf::Decoded, scale: u32) -> Raster {
    let layout = match d.format {
        tf::PixelFormat::Gray8 => PixelLayout::Gray,
        tf::PixelFormat::Rgba8 => PixelLayout::Rgba,
        tf::PixelFormat::Gray16 => PixelLayout::Gray16,
        tf::PixelFormat::GrayAlpha16 => PixelLayout::GrayAlpha16,
        tf::PixelFormat::Rgba16 => PixelLayout::Rgba16(HdrEncoding::SRGB),
        tf::PixelFormat::Rgba16F => PixelLayout::Rgba16F(Primaries::Bt709),
        // Only from `decode_values`, which never comes here.
        tf::PixelFormat::GrayF32 => PixelLayout::Rgba,
    };
    Raster {
        width: d.width,
        height: d.height,
        origin: d.origin,
        scale,
        layout,
        planes: vec![RasterPlane {
            width: d.width,
            height: d.height,
            stride: d.stride,
            data: d.data,
            h_div: 1,
            v_div: 1,
        }],
        truncated: d.truncated,
    }
}

impl TiffSession {
    fn opts<'a>(
        scale: u32,
        region: Option<Rect>,
        parallel: bool,
        cancel: &'a CancelToken,
    ) -> tf::DecodeOptions<'a> {
        tf::DecodeOptions {
            scale,
            region: region.map(|r| tf::Region {
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h,
            }),
            cancel: Some(cancel.flag()),
            parallel,
            wide: true,
        }
    }

    fn direct(
        &self,
        scale: u32,
        region: Option<Rect>,
        parallel: bool,
        cancel: &CancelToken,
    ) -> Result<Raster> {
        let d = self
            .tiff
            .decode(
                self.source.bytes(),
                self.page,
                &Self::opts(scale, region, parallel, cancel),
            )
            .map_err(map_err)?;
        Ok(to_raster(d, scale))
    }

    /// Values of band 1 at `scale` (a region of the scaled image, or all of it), physical
    /// (GDAL scale and offset applied), no-data as NaN.
    fn values(
        &self,
        v: &Values,
        scale: u32,
        region: Option<Rect>,
        parallel: bool,
        cancel: &CancelToken,
    ) -> Result<(tf::Decoded, Vec<f32>)> {
        let mut d = self
            .tiff
            .decode_values(
                self.source.bytes(),
                self.page,
                0,
                v.nodata,
                &Self::opts(scale, region, parallel, cancel),
            )
            .map_err(map_err)?;
        let data = std::mem::take(&mut d.data);
        let mut vals: Vec<f32> = data
            .chunks_exact(4)
            .map(|b| f32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        if let Some((k, o)) = v.scale_offset {
            for x in &mut vals {
                *x = (f64::from(*x) * k + o) as f32;
            }
        }
        Ok((d, vals))
    }

    /// zscale limits of the whole image, from a decode at 1/8 (an overview when there is one),
    /// or larger where 1/8 would give fewer than [`LIMIT_SAMPLES`] values: zscale fits a line
    /// to the sorted samples, and nine of a 20×20 grid clipped half of it to black or white.
    fn value_limits(&self, v: &Values, cancel: &CancelToken) -> Result<Option<(f32, f32)>> {
        if let Some(l) = *v.limits.lock() {
            return Ok(l);
        }
        let (w, h) = (u64::from(self.info.width), u64::from(self.info.height));
        let mut scale = 8u32;
        while scale > 1 && (w / u64::from(scale)) * (h / u64::from(scale)) < LIMIT_SAMPLES {
            scale /= 2;
        }
        let (d, vals) = self.values(v, scale, None, true, cancel)?;
        let l = Limits::ZScale.compute(&vals, d.width as usize);
        *v.limits.lock() = Some(l);
        Ok(l)
    }

    /// The values as grey at zscale limits, no-data transparent.
    fn decode_values_grey(
        &self,
        v: &Values,
        req: &DecodeRequest,
        cancel: &CancelToken,
    ) -> Result<Raster> {
        let scale = req.scale.max(1);
        let (lo, hi) = self.value_limits(v, cancel)?.unwrap_or((0.0, 1.0));
        let (d, vals) = self.values(v, scale, req.region, req.parallel, cancel)?;
        let k = if hi > lo { 255.0 / (hi - lo) } else { 0.0 };
        let mut rgba = vec![0u8; vals.len() * 4];
        for (px, &x) in rgba.chunks_exact_mut(4).zip(&vals) {
            if x.is_finite() {
                let g = ((x - lo) * k).clamp(0.0, 255.0).round() as u8;
                px.copy_from_slice(&[g, g, g, 255]);
            }
        }
        Ok(Raster {
            width: d.width,
            height: d.height,
            origin: d.origin,
            scale,
            layout: PixelLayout::Rgba,
            planes: vec![RasterPlane {
                width: d.width,
                height: d.height,
                stride: d.width as usize * 4,
                data: rgba,
                h_div: 1,
                v_div: 1,
            }],
            truncated: d.truncated,
        })
    }

    fn level_bytes(&self, scale: u32) -> u64 {
        let p = &self.tiff.pages()[self.page];
        let bpp = p
            .wide_format()
            .unwrap_or(p.output_format())
            .bytes_per_pixel();
        u64::from(self.info.width.div_ceil(scale))
            * u64::from(self.info.height.div_ceil(scale))
            * bpp as u64
    }
}

impl Session for TiffSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn colour_model(&self) -> Option<&'static str> {
        self.colour_model
    }
    fn alpha(&self) -> Option<Alpha> {
        self.alpha
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }

    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        if let Some(v) = self.values.as_ref().filter(|v| v.stretched) {
            return self.decode_values_grey(v, req, cancel);
        }
        let scale = req.scale.max(1);
        if self.info.fast_regions
            || req.region.is_none()
            || self.level_bytes(scale) > LEVEL_CACHE_MAX
        {
            return self.direct(scale, req.region, req.parallel, cancel);
        }
        if let Some(l) = self.levels.lock().iter().find(|l| l.scale == scale) {
            return Ok(req
                .region
                .map_or_else(|| (**l).clone(), |r| crate::jpeg::crop(l, r)));
        }
        let level = Arc::new(self.direct(scale, None, req.parallel, cancel)?);
        {
            let mut ls = self.levels.lock();
            if !ls.iter().any(|l| l.scale == scale) {
                ls.push(level.clone());
            }
        }
        Ok(req
            .region
            .map_or_else(|| (*level).clone(), |r| crate::jpeg::crop(&level, r)))
    }

    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let scale = req.scale.max(1);
        let token = CancelToken::new();
        let mut m = self.tiff.memory_estimate(
            self.page,
            &Self::opts(scale, req.region, req.parallel, &token),
        );
        if !self.info.fast_regions && req.region.is_some() {
            m += self.level_bytes(scale);
        }
        m
    }

    fn memory_bytes(&self) -> usize {
        self.levels
            .lock()
            .iter()
            .map(|l| l.planes.iter().map(|p| p.data.len()).sum::<usize>())
            .sum::<usize>()
            + self.values.as_ref().map_or(0, |v| v.planes.memory_bytes())
    }

    /// Drops the whole levels of a large-strip image (tiled ones decode regions directly and
    /// keep none) and the value planes.
    fn trim_memory(&self) -> usize {
        crate::whole::drop_levels(&self.levels)
            + self.values.as_ref().map_or(0, |v| v.planes.clear())
    }

    fn geo(&self) -> Option<Arc<GeoRef>> {
        self.geo.clone()
    }

    fn science(&self) -> Option<&ScienceSummary> {
        self.values
            .as_ref()
            .map(|v| &v.summary)
            .or(self.facts.as_ref())
    }

    fn nd_plane(&self, index: &[u32], cancel: &CancelToken) -> Option<Result<Arc<NdPlane>>> {
        let v = self.values.as_ref()?;
        Some(v.planes.get_or(index, cancel, || {
            let (d, values) = self.values(v, 1, None, true, cancel)?;
            Ok(NdPlane {
                width: d.width,
                height: d.height,
                values,
                truncated: d.truncated,
            })
        }))
    }

    fn page_count(&self) -> u32 {
        self.pages
    }
    fn page_index(&self) -> u32 {
        self.page as u32
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        open_parsed(self.source.clone(), self.tiff.clone(), page as usize)
    }
}
