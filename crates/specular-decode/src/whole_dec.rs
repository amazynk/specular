//! Formats read whole by in-tree crates that share one shape (`probe`, `info` and `decode` per
//! page, header facts beside them): DPX and Cineon (specular-film); XWD, MIFF and PlayStation
//! TIM (specular-legacy2); FBI WSQ (specular-wsq); JBIG (specular-jbig); JBIG2
//! (specular-jbig2); JPEG XS (specular-jpegxs); GIMP and Adobe palettes, gradients, brushes
//! and patterns, Microsoft Paint, GD/GD2, Pixar, LabEye IM, IM Tools, XV thumbnails, McIDAS
//! AREA and PGX (specular-legacy2); CALS Type 1 rasters (G4 from specular-tiff).
//!
//! Each page is decoded whole once, to 8-bit or 16-bit display-coded samples or to linear
//! light (DPX and Cineon film scans in printing density or linear); coarser levels are
//! box-filtered from it and regions cropped. Images in a file (MIFF images, DPX image
//! elements, JBIG2 pages) are pages.

use crate::whole::{self, Levels};
use crate::{
    Confidence, DecodeError, DecodeRequest, Decoder, Format, HeaderFacts, ImageInfo, PageLabel,
    Result, Session,
};
use specular_core::hdr::{HdrEncoding, Primaries};
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use std::sync::Arc;

/// How many bytes the content probes look at.
const PROBE_BYTES: usize = 512;
/// JPEG XS in HEIF is recognised by its `meta` box (the primary item's type), which can sit
/// further in.
const PROBE_BYTES_HEIF: usize = 64 << 10;

/// Header facts in the crates' common shape.
struct Facts {
    width: u32,
    height: u32,
    pages: u32,
    bit_depth: u8,
    model: &'static str,
    has_alpha: bool,
    orientation: u8,
    coding: String,
    header: HeaderFacts,
}

/// Decoded pixels in the crates' common shape.
enum Px {
    Gray8(Vec<u8>),
    Rgba8(Vec<u8>),
    Gray16(Vec<u16>),
    Rgba16(Vec<u16>),
    /// Linear light, 1.0 = reference white.
    RgbaF32(Vec<f32>),
}

struct Img {
    width: u32,
    height: u32,
    px: Px,
    truncated: bool,
}

/// One format module: `$m` holds `probe`, `info` and `decode`; `$root` the `Error`, `Model`
/// and `Pixels` types.
macro_rules! adapter {
    // `; NeedsMemory` for a crate whose errors carry the memory budget's refusals.
    ($name:ident, $($m:ident)::+ ; $($root:ident)::+ $(; $nm:ident)?) => {
        mod $name {
            use super::{Facts, Img, Px};
            use crate::{DecodeError, HeaderFacts, Result};
            use $($m)::+ as m;
            use $($root)::+ as root;

            fn err(e: root::Error) -> DecodeError {
                match e {
                    root::Error::NotRecognised => DecodeError::UnsupportedFormat,
                    root::Error::Unsupported(s) => DecodeError::Unsupported(s),
                    root::Error::Truncated(s) => DecodeError::Truncated(s),
                    root::Error::Malformed(s) => DecodeError::Malformed(s),
                    root::Error::TooLarge => DecodeError::TooLarge,
                    $(root::Error::$nm(n) => DecodeError::NeedsMemory(n),)?
                }
            }

            pub(super) fn probe(head: &[u8]) -> bool {
                m::probe(head)
            }

            pub(super) fn info(data: &[u8], page: u32) -> Result<Facts> {
                let i = m::info(data, page).map_err(err)?;
                Ok(Facts {
                    width: i.width,
                    height: i.height,
                    pages: i.pages.max(1),
                    bit_depth: i.bit_depth,
                    model: match i.model {
                        root::Model::Grey => "Greyscale",
                        root::Model::Rgb => "RGB",
                        root::Model::Palette => "Palette",
                        root::Model::YCbCr => "YCbCr",
                        // "1-bit Greyscale" in the info panel.
                        root::Model::Bilevel => "Greyscale",
                    },
                    has_alpha: i.has_alpha,
                    orientation: i.orientation,
                    coding: i.coding,
                    header: HeaderFacts {
                        summary: i.summary,
                        fields: i.fields,
                    },
                })
            }

            pub(super) fn decode(data: &[u8], page: u32) -> Result<Img> {
                let i = m::decode(data, page).map_err(err)?;
                Ok(Img {
                    width: i.width,
                    height: i.height,
                    px: match i.pixels {
                        root::Pixels::Gray8(v) => Px::Gray8(v),
                        root::Pixels::Rgba8(v) => Px::Rgba8(v),
                        root::Pixels::Gray16(v) => Px::Gray16(v),
                        root::Pixels::Rgba16(v) => Px::Rgba16(v),
                        root::Pixels::RgbaF32(v) => Px::RgbaF32(v),
                    },
                    truncated: i.truncated,
                })
            }
        }
    };
}

adapter!(dpx, specular_film::dpx; specular_film; NeedsMemory);
adapter!(cineon, specular_film::cineon; specular_film; NeedsMemory);
adapter!(xwd, specular_legacy2::xwd; specular_legacy2; NeedsMemory);
adapter!(miff, specular_legacy2::miff; specular_legacy2; NeedsMemory);
adapter!(tim, specular_legacy2::tim; specular_legacy2; NeedsMemory);
adapter!(wsq, specular_wsq; specular_wsq; NeedsMemory);
adapter!(jbig, specular_jbig; specular_jbig; NeedsMemory);
adapter!(jbig2, specular_jbig2; specular_jbig2);
adapter!(jpegxs, specular_jpegxs; specular_jpegxs; NeedsMemory);
adapter!(gpl, specular_legacy2::gpl; specular_legacy2; NeedsMemory);
adapter!(aco, specular_legacy2::aco; specular_legacy2; NeedsMemory);
adapter!(act, specular_legacy2::act; specular_legacy2; NeedsMemory);
adapter!(ase, specular_legacy2::ase; specular_legacy2; NeedsMemory);
adapter!(ggr, specular_legacy2::ggr; specular_legacy2; NeedsMemory);
adapter!(gbr, specular_legacy2::gimp::gbr; specular_legacy2; NeedsMemory);
adapter!(gih, specular_legacy2::gimp::gih; specular_legacy2; NeedsMemory);
adapter!(pat, specular_legacy2::gimp::pat; specular_legacy2; NeedsMemory);
adapter!(msp, specular_legacy2::msp; specular_legacy2; NeedsMemory);
adapter!(gd1, specular_legacy2::gd::image; specular_legacy2; NeedsMemory);
adapter!(gd2, specular_legacy2::gd::gd2; specular_legacy2; NeedsMemory);
adapter!(pxr, specular_legacy2::pxr; specular_legacy2; NeedsMemory);
adapter!(labeye, specular_legacy2::labeye; specular_legacy2; NeedsMemory);
adapter!(imt, specular_legacy2::imt; specular_legacy2; NeedsMemory);
adapter!(xvthumb, specular_legacy2::xvthumb; specular_legacy2; NeedsMemory);
adapter!(mcidas, specular_legacy2::mcidas; specular_legacy2; NeedsMemory);
adapter!(pgx, specular_legacy2::pgx; specular_legacy2; NeedsMemory);

/// CALS Type 1 rasters (MIL-R-28002): a 2048-byte header of 128-byte `key: value` records
/// (`rtype`, `rorient`, `rpelcnt`, `rdensty`…), then one CCITT Group 4 (T.6) stream.
/// `rorient` gives the pel path and line progression: 000,270 is the usual left to right,
/// top to bottom; other angles become the image's orientation. Type 2 (tiled) is refused.
mod cals {
    use super::{Facts, Img, Px};
    use crate::{DecodeError, HeaderFacts, Result};

    const HEADER: usize = 2048;
    const KEYS: &[&str] = &[
        "srcdocid:",
        "dstdocid:",
        "txtfilid:",
        "figid:",
        "srcgph:",
        "doccls:",
        "rtype:",
        "rorient:",
        "rpelcnt:",
        "rdensty:",
        "notes:",
    ];

    fn records(d: &[u8]) -> Vec<(String, String)> {
        d[..d.len().min(HEADER)]
            .chunks(128)
            .filter_map(|r| {
                let t = String::from_utf8_lossy(r);
                let (k, v) = t.split_once(':')?;
                let k = k.trim();
                (!k.is_empty() && k.bytes().all(|c| c.is_ascii_alphanumeric()))
                    .then(|| (k.to_ascii_lowercase(), v.trim().to_owned()))
            })
            .collect()
    }

    /// The first record is a CALS key and `rpelcnt` is among the records.
    pub(super) fn probe(head: &[u8]) -> bool {
        let first = head.get(..16).unwrap_or(head);
        KEYS.iter().any(|k| first.starts_with(k.as_bytes()))
            && head
                .chunks(128)
                .take(16)
                .any(|r| r.starts_with(b"rpelcnt:"))
    }

    struct Header {
        width: u32,
        height: u32,
        orientation: u8,
        fields: Vec<(String, String)>,
    }

    fn header(d: &[u8]) -> Result<Header> {
        if !probe(d) {
            return Err(DecodeError::UnsupportedFormat);
        }
        let recs = records(d);
        let get = |k: &str| recs.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str());
        match get("rtype").map(str::trim) {
            Some("1") | None => {}
            Some("2") => {
                return Err(DecodeError::Unsupported(
                    "CALS Type 2 (tiled) rasters are not supported (Type 1 is)".into(),
                ));
            }
            Some(t) => {
                return Err(DecodeError::Unsupported(format!(
                    "CALS raster type {t} is not supported (Type 1 is)"
                )));
            }
        }
        let pair = |v: &str| -> Option<(u32, u32)> {
            let (a, b) = v.split_once(',')?;
            Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
        };
        let (width, height) = get("rpelcnt")
            .and_then(pair)
            .ok_or_else(|| DecodeError::Malformed("no rpelcnt size".into()))?;
        if width == 0 || height == 0 || width > 1 << 17 || height > 1 << 17 {
            return Err(DecodeError::Malformed(format!("size {width}×{height}")));
        }
        // Pel path and line progression (relative), as EXIF orientations.
        let orientation = match get("rorient").and_then(pair) {
            None | Some((0, 270)) => 1,
            Some((90, 270)) => 6,
            Some((180, 270)) => 3,
            Some((270, 270)) => 8,
            Some((0, 90)) => 4,
            Some((90, 90)) => 5,
            Some((180, 90)) => 2,
            Some((270, 90)) => 7,
            Some((p, l)) => {
                return Err(DecodeError::Unsupported(format!(
                    "CALS orientation {p:03},{l:03} is not supported"
                )));
            }
        };
        let fields = recs
            .into_iter()
            .filter(|(_, v)| !v.is_empty() && !v.eq_ignore_ascii_case("NONE"))
            .collect();
        Ok(Header {
            width,
            height,
            orientation,
            fields,
        })
    }

    pub(super) fn info(data: &[u8], _page: u32) -> Result<Facts> {
        let h = header(data)?;
        Ok(Facts {
            width: h.width,
            height: h.height,
            pages: 1,
            bit_depth: 1,
            model: "Greyscale",
            has_alpha: false,
            orientation: h.orientation,
            coding: "CCITT Group 4".into(),
            header: HeaderFacts {
                summary: Vec::new(),
                fields: h.fields,
            },
        })
    }

    pub(super) fn decode(data: &[u8], _page: u32) -> Result<Img> {
        let h = header(data)?;
        let (w, rows) = (h.width as usize, h.height as usize);
        let src = data.get(HEADER..).unwrap_or(&[]);
        let (bits, done, damaged) = specular_tiff::decode_g4(src, h.width, rows);
        let rb = w.div_ceil(8);
        let mut px = vec![128u8; w * rows];
        for (y, row) in bits.chunks_exact(rb).take(done).enumerate() {
            for (x, p) in px[y * w..(y + 1) * w].iter_mut().enumerate() {
                *p = if (row[x / 8] >> (7 - x % 8)) & 1 == 1 {
                    0
                } else {
                    255
                };
            }
        }
        Ok(Img {
            width: h.width,
            height: h.height,
            px: Px::Gray8(px),
            truncated: damaged || done < rows,
        })
    }
}

fn probe(kind: Format, head: &[u8]) -> bool {
    match kind {
        Format::Dpx => dpx::probe(head),
        Format::Cineon => cineon::probe(head),
        Format::Xwd => xwd::probe(head),
        Format::Miff => miff::probe(head),
        Format::Tim => tim::probe(head),
        Format::Wsq => wsq::probe(head),
        Format::Jbig => jbig::probe(head),
        Format::Jbig2 => jbig2::probe(head),
        Format::JpegXs => jpegxs::probe(head),
        Format::GimpPalette => gpl::probe(head),
        Format::SwatchExchange => ase::probe(head),
        Format::GimpGradient => ggr::probe(head),
        Format::GimpBrush => gbr::probe(head) && !specular_legacy2::gimp::gbr::is_weak(head),
        Format::GimpBrushPipe => gih::probe(head),
        Format::GimpPattern => pat::probe(head),
        Format::MsPaint => msp::probe(head),
        Format::Gd => gd1::probe(head),
        Format::Gd2 => gd2::probe(head),
        Format::PixarPicture => pxr::probe(head),
        Format::LabEyeIm => labeye::probe(head),
        Format::XvThumbnail => xvthumb::probe(head),
        Format::Pgx => pgx::probe(head),
        Format::Cals => cals::probe(head),
        // No signature: see `weak`.
        _ => false,
    }
}

fn info(kind: Format, data: &[u8], page: u32) -> Result<Facts> {
    match kind {
        Format::Dpx => dpx::info(data, page),
        Format::Cineon => cineon::info(data, page),
        Format::Xwd => xwd::info(data, page),
        Format::Miff => miff::info(data, page),
        Format::Tim => tim::info(data, page),
        Format::Wsq => wsq::info(data, page),
        Format::Jbig => jbig::info(data, page),
        Format::Jbig2 => jbig2::info(data, page),
        Format::JpegXs => jpegxs::info(data, page),
        Format::GimpPalette => gpl::info(data, page),
        Format::PhotoshopSwatches => aco::info(data, page),
        Format::AdobeColorTable => act::info(data, page),
        Format::SwatchExchange => ase::info(data, page),
        Format::GimpGradient => ggr::info(data, page),
        Format::GimpBrush => gbr::info(data, page),
        Format::GimpBrushPipe => gih::info(data, page),
        Format::GimpPattern => pat::info(data, page),
        Format::MsPaint => msp::info(data, page),
        Format::Gd => gd1::info(data, page),
        Format::Gd2 => gd2::info(data, page),
        Format::PixarPicture => pxr::info(data, page),
        Format::LabEyeIm => labeye::info(data, page),
        Format::ImTools => imt::info(data, page),
        Format::XvThumbnail => xvthumb::info(data, page),
        Format::McIdasArea => mcidas::info(data, page),
        Format::Pgx => pgx::info(data, page),
        Format::Cals => cals::info(data, page),
        _ => Err(DecodeError::UnsupportedFormat),
    }
}

fn decode(kind: Format, data: &[u8], page: u32) -> Result<Img> {
    match kind {
        Format::Dpx => dpx::decode(data, page),
        Format::Cineon => cineon::decode(data, page),
        Format::Xwd => xwd::decode(data, page),
        Format::Miff => miff::decode(data, page),
        Format::Tim => tim::decode(data, page),
        Format::Wsq => wsq::decode(data, page),
        Format::Jbig => jbig::decode(data, page),
        Format::Jbig2 => jbig2::decode(data, page),
        Format::JpegXs => jpegxs::decode(data, page),
        Format::GimpPalette => gpl::decode(data, page),
        Format::PhotoshopSwatches => aco::decode(data, page),
        Format::AdobeColorTable => act::decode(data, page),
        Format::SwatchExchange => ase::decode(data, page),
        Format::GimpGradient => ggr::decode(data, page),
        Format::GimpBrush => gbr::decode(data, page),
        Format::GimpBrushPipe => gih::decode(data, page),
        Format::GimpPattern => pat::decode(data, page),
        Format::MsPaint => msp::decode(data, page),
        Format::Gd => gd1::decode(data, page),
        Format::Gd2 => gd2::decode(data, page),
        Format::PixarPicture => pxr::decode(data, page),
        Format::LabEyeIm => labeye::decode(data, page),
        Format::ImTools => imt::decode(data, page),
        Format::XvThumbnail => xvthumb::decode(data, page),
        Format::McIdasArea => mcidas::decode(data, page),
        Format::Pgx => pgx::decode(data, page),
        Format::Cals => cals::decode(data, page),
        _ => Err(DecodeError::UnsupportedFormat),
    }
}

/// Content checks without a signature (the whole file at hand): likely, never certain, and
/// only for the extensions the decoder claims.
fn weak(kind: Format, data: &[u8]) -> bool {
    match kind {
        Format::PhotoshopSwatches => aco::probe(data),
        Format::AdobeColorTable => act::probe(data),
        // GD 1.x: no id, the exact size.
        Format::Gd => specular_legacy2::gd::image::probe_v1(data),
        // Version 1 brushes have no magic.
        Format::GimpBrush => {
            specular_legacy2::gimp::gbr::probe(data) && specular_legacy2::gimp::gbr::is_weak(data)
        }
        Format::ImTools => imt::probe(&data[..data.len().min(PROBE_BYTES)]),
        Format::McIdasArea => mcidas::probe(data) && data.len() >= 256,
        _ => false,
    }
}

fn ne_bytes(v: &[u16]) -> Vec<u8> {
    v.iter().flat_map(|s| s.to_ne_bytes()).collect()
}

fn to_raster(img: Img, parallel: bool) -> Result<Raster> {
    let (w, h) = (img.width, img.height);
    let n = w as usize * h as usize;
    let short = |len: usize, per: usize| {
        if len < n * per {
            Err(DecodeError::Internal(format!(
                "decoder gave {len} samples for {w}×{h}×{per}"
            )))
        } else {
            Ok(())
        }
    };
    Ok(match img.px {
        Px::Gray8(v) => {
            short(v.len(), 1)?;
            whole::interleaved(w, h, PixelLayout::Gray, v, img.truncated)
        }
        Px::Rgba8(v) => {
            short(v.len(), 4)?;
            whole::interleaved(w, h, PixelLayout::Rgba, v, img.truncated)
        }
        Px::Gray16(v) => {
            short(v.len(), 1)?;
            whole::interleaved(w, h, PixelLayout::Gray16, ne_bytes(&v), img.truncated)
        }
        Px::Rgba16(v) => {
            short(v.len(), 4)?;
            whole::interleaved(
                w,
                h,
                PixelLayout::Rgba16(HdrEncoding::SRGB),
                ne_bytes(&v),
                img.truncated,
            )
        }
        Px::RgbaF32(v) => {
            short(v.len(), 4)?;
            crate::linear::f32_to_raster(
                w,
                h,
                4,
                &v,
                None,
                Primaries::Bt709,
                img.truncated,
                parallel,
            )
        }
    })
}

/// Bytes per decoded pixel (the raster kept, not the decoder's working buffers).
fn raster_bpp(kind: Format, f: &Facts) -> u64 {
    match kind {
        // Film scans keep 16-bit or half-float samples.
        Format::Dpx | Format::Cineon if f.model == "Greyscale" && f.bit_depth > 8 => 2,
        Format::Dpx | Format::Cineon => 8,
        Format::Miff if f.bit_depth > 8 => 8,
        // JPEG XS keeps 16-bit RGBA past 8 bits (greyscale below).
        Format::JpegXs if f.bit_depth > 8 && f.model != "Greyscale" => 8,
        _ if f.model == "Greyscale" && !f.has_alpha => {
            if f.bit_depth > 8 {
                2
            } else {
                1
            }
        }
        _ => 4,
    }
}

pub struct WholeDecoder(pub Format);

impl Decoder for WholeDecoder {
    fn format(&self) -> Format {
        self.0
    }
    fn probe(&self, head: &[u8]) -> bool {
        probe(self.0, head)
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        let n = if self.0 == Format::JpegXs {
            PROBE_BYTES_HEIF
        } else {
            PROBE_BYTES
        };
        probe(self.0, &data[..data.len().min(n)])
    }
    fn confidence(&self, data: &[u8]) -> Option<Confidence> {
        if self.probe_file(data) {
            Some(Confidence::Certain)
        } else {
            weak(self.0, data).then_some(Confidence::Likely)
        }
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Format::Dpx => &["dpx"],
            Format::Cineon => &["cin"],
            Format::Xwd => &["xwd"],
            Format::Miff => &["miff"],
            Format::Tim => &["tim"],
            Format::Wsq => &["wsq"],
            Format::Jbig => &["jbg", "jbig", "bie"],
            Format::Jbig2 => &["jb2", "jbig2"],
            Format::JpegXs => &["jxs", "jxsi"],
            Format::GimpPalette => &["gpl"],
            Format::PhotoshopSwatches => &["aco"],
            Format::AdobeColorTable => &["act"],
            Format::SwatchExchange => &["ase"],
            Format::GimpGradient => &["ggr"],
            Format::GimpBrush => &["gbr"],
            Format::GimpBrushPipe => &["gih"],
            Format::GimpPattern => &["pat"],
            Format::MsPaint => &["msp"],
            Format::Gd => &["gd"],
            Format::Gd2 => &["gd2"],
            Format::PixarPicture => &["pxr", "picio"],
            Format::LabEyeIm => &["im"],
            Format::ImTools => &["imt"],
            // XV keeps its thumbnails under the original's name in `.xvpics`.
            Format::XvThumbnail => &[],
            Format::McIdasArea => &["area", "ara"],
            Format::Pgx => &["pgx"],
            Format::Cals => &["cal", "cals", "ct1"],
            _ => &[],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        open_page(self.0, source, 0)
    }
}

fn open_page(kind: Format, source: Arc<Source>, page: u32) -> Result<Arc<dyn Session>> {
    let f = info(kind, source.bytes(), page)?;
    if f.width == 0 || f.height == 0 {
        return Err(DecodeError::Malformed("empty image".into()));
    }
    let info = ImageInfo {
        format: kind,
        width: f.width,
        height: f.height,
        orientation: Orientation::from_exif(f.orientation),
        icc_profile: None,
        has_alpha: f.has_alpha,
        bit_depth: f.bit_depth,
        progressive: false,
        coding: f.coding.clone(),
        embedded_preview: None,
        fast_regions: false,
        scales: &[1, 2, 4, 8],
    };
    let bpp = raster_bpp(kind, &f);
    Ok(Arc::new(WholeSession {
        kind,
        page,
        pages: f.pages,
        source,
        info,
        model: f.model,
        header: f.header,
        bpp,
        levels: Levels::new(),
    }))
}

struct WholeSession {
    kind: Format,
    page: u32,
    pages: u32,
    source: Arc<Source>,
    info: ImageInfo,
    model: &'static str,
    header: HeaderFacts,
    /// Bytes per pixel of the decoded raster.
    bpp: u64,
    levels: Levels,
}

impl Session for WholeSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(self.model)
    }
    fn header(&self) -> Option<&HeaderFacts> {
        (!self.header.summary.is_empty() || !self.header.fields.is_empty()).then_some(&self.header)
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| {
                let img = decode(self.kind, self.source.bytes(), self.page)?;
                if (img.width, img.height) != (self.info.width, self.info.height) {
                    return Err(DecodeError::Internal(format!(
                        "{} decoder gave {}×{} for a {}×{} image",
                        self.kind.name(),
                        img.width,
                        img.height,
                        self.info.width,
                        self.info.height
                    )));
                }
                to_raster(img, req.parallel)
            },
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        let s = req.scale.max(1);
        let mut m = whole::level_bytes(w, h, s, self.bpp);
        if !self.levels.has(s) && !self.levels.has(1) {
            // The full image, plus the decoder's own output before conversion (float
            // RGBA for film scans, the same samples elsewhere).
            let work = match self.kind {
                Format::Dpx | Format::Cineon => 16,
                // JPEG XS: 32-bit coefficients of up to four components, then 16-bit samples.
                Format::JpegXs => 24,
                _ => self.bpp,
            };
            m += whole::level_bytes(w, h, 1, self.bpp) + whole::level_bytes(w, h, 1, work);
        }
        m
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear()
    }
    fn page_count(&self) -> u32 {
        self.pages
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        (self.pages > 1).then_some(PageLabel::Image {
            width: self.info.width,
            height: self.info.height,
            bits: None,
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page >= self.pages {
            return Err(DecodeError::Malformed("no such page".into()));
        }
        open_page(self.kind, self.source.clone(), page)
    }
}
