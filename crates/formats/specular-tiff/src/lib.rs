//! TIFF decoder for Specular.
//!
//! Own container parser and decode pipeline (no libtiff, no `tiff` crate at run time):
//!
//! - classic TIFF and BigTIFF, either byte order; IFD chains and SubIFDs, with loop and size
//!   guards;
//! - strips and tiles, chunky and planar; compression none, PackBits, LZW (own decoder), Deflate
//!   (`zlib-rs`), JPEG (`specular-jpeg`, with `JPEGTables`), old-style JPEG (compression 6:
//!   whole streams, `JPEGInterchangeFormat` streams or per-component table tags, rebuilt for
//!   `specular-jpeg`), CCITT RLE / Group 3 (1D and 2D) / Group 4 (own table-driven decoder,
//!   either `FillOrder`), Zstandard (`ruzstd`), LZMA (own xz container parser, Delta filter,
//!   `lzma-rust2` for LZMA2), LERC v2 (own decoder, v2-v6, optionally Deflate- or
//!   Zstandard-wrapped), and whole-image codecs per unit: WebP (`specular-webp`), JPEG XL
//!   (`specular-jxl`; 16/32-bit float samples as stored) and JPEG 2000 (`specular-jp2`,
//!   signed samples too, including Aperio SVS 33003 / 33005);
//!   horizontal predictor at any depth up to 64 bits (sub-byte and odd depths too, which
//!   libtiff refuses: the samples summed modulo `2^bits`) and the floating-point predictor
//!   (16/24/32/64-bit), either byte order;
//! - 1–64-bit integer (signed too) and 16/24/32/64-bit float samples, grey, RGB(A), palette
//!   (up to 22 bits), CMYK, YCbCr; L*a*b* (CIELab, ICCLab, ITULab with its `Decode` ranges) of
//!   any integer depth or float, with or without alpha, in any of the compressions (JPEG:
//!   8-bit);
//! - separated images with named inks (InkSet 2, InkNames): process inks by name in any order,
//!   spot inks tinted by the colour word in their name ("Orange"; mid grey without one), each
//!   ink filtering the light by its amount (the naive CMYK formula generalised);
//! - **pages**: every top-level IFD that is not a reduced-resolution image or a mask is a page.
//!   Reduced-resolution IFDs (`NewSubfileType` bit 0) that follow a page, and its SubIFDs, become
//!   that page's pyramid *levels*;
//! - decodes any power-of-two scale and any region: only the strips / tiles that intersect the
//!   region are read, units are decoded in parallel (rayon), the coarsest pyramid level that is
//!   at least as fine as the request is used, and the remaining factor is a gamma-correct box
//!   filter (or the JPEG DCT scaling, for JPEG tiles). A reduced-scale decode of one large strip
//!   or tile converts and reduces it in bands of rows (in parallel), never holding the whole
//!   converted unit.
//!
//! Output is 8 bits per sample, grey or RGBA (straight alpha), unless the caller asks for
//! [`DecodeOptions::wide`] output, which keeps integer grey and RGB(A) deeper than 8 bits at 16
//! bits per sample ([`PixelFormat::Gray16`], [`PixelFormat::GrayAlpha16`] or
//! [`PixelFormat::Rgba16`]; [`Page::wide_format`] says which):
//! - 16/32-bit integers are rounded to 8 bits (`v * 255 / max`), or to 16 bits for wide output
//!   (`v * 65535 / max`); signed integers are offset to unsigned first;
//! - floating-point samples are taken as linear light (NaN as 0): clamped to 0..1 and
//!   sRGB-encoded for 8-bit output, or kept unclamped as [`PixelFormat::Rgba16F`] (values above
//!   1.0 kept) for wide output;
//! - CMYK is converted naively, `R = (1 - C)(1 - K)` etc. (no ICC), unless the caller registers
//!   a colour-managed converter with [`set_cmyk_hook`];
//! - L*a*b* (D50) goes to sRGB through a scalar formula (relative colorimetric, Bradford), unless
//!   the caller registers a faster one with [`set_lab_hook`], which gets the samples where they
//!   lie (chunky rows or planes) and their [`LabCoding`]. An embedded ICC profile does not change
//!   it: L*a*b* is device-independent. 16-bit ICCLab a*/b* use the ICC v4 coding (÷ 257 − 128).
//!   Other integer depths are scaled to 16 bits first (CIELab a*/b* then read as 16-bit two's
//!   complement, as ImageMagick writes them); float L*a*b* holds the values themselves
//!   (libvips), or the 16-bit coding over 0–1 when ImageMagick wrote it (its name in Software,
//!   or an SMaxSampleValue above 100);
//! - associated (premultiplied) alpha is un-premultiplied.
//!
//! Rarer layouts go through `variants`, which rewrites a decompressed unit into one of the
//! layouts above: integers of 17–31, 24 and 33–64 bits (read as 32-bit), L*a*b* of other
//! depths (read as 16-bit), SGI LogL, LogLuv32
//! and LogLuv24 (34676, 34677, read as linear float grey / RGB), YCbCr with chroma subsampling outside JPEG (any
//! compression, 1–4 × 1–4), ThunderScan 4-bit (32809), NeXT 2-bit (32766) and PixarLog
//! (32909, strips and tiles, read as linear float, so values above 1.0 survive); JBIG1
//! strips (34661) go
//! through `specular-jbig`. Old-style (LSB-first) LZW, LERC 6
//! lossless float (FPL) and tiled files with their offsets in the strip tags are read too.
//!
//! Not supported (decode returns [`Error::Unsupported`] or a truncated unit): palettes deeper
//! than 22 bits (their ColorMap would exceed the IFD value limit), planar SGI LogLuv (libtiff
//! refuses it and no writer makes it), xz filters other than Delta and LZMA2, and CCITT
//! uncompressed mode (no encoder writes it and no reader decodes it: libtiff, Ghostscript,
//! MuPDF and pdfium refuse or skip it, so there is nothing to check a decoder against),
//! PixarFilm (32908: no description, sample or libtiff codec) and float JPEG 2000 (the
//! codestream holds integers only).

mod ccitt;
mod cmyk_hook;
mod codec;
mod convert;
mod decode;
mod ifd;
mod imgcodec;
mod lab_hook;
mod lerc;
mod logluv_uv;
mod ojpeg;
pub mod science;
mod variants;

use std::ops::Range;
use std::sync::atomic::AtomicBool;

pub use cmyk_hook::{CmykHook, CmykRowConverter, set_cmyk_hook};
pub use convert::downscale;
pub use lab_hook::{LabCoding, LabHook, LabRowConverter, LabSamples, set_lab_hook};

/// Decodes a CCITT Group 4 (T.6) stream of `rows` rows of `width` pixels, as used outside
/// TIFF (CALS rasters): the rows packed most significant bit first, a set bit for each
/// black pixel, then the number of complete rows and whether the stream was damaged.
pub fn decode_g4(input: &[u8], width: u32, rows: usize) -> (Vec<u8>, usize, bool) {
    let rb = (width as usize).div_ceil(8);
    let mut out = vec![0u8; rb * rows];
    let (n, damaged) = ccitt::decode(input, ccitt::Mode::G4, width, rows, &mut out);
    (out, n / rb.max(1), damaged)
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("truncated: {0}")]
    Truncated(&'static str),
    #[error("malformed: {0}")]
    Malformed(&'static str),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("too large")]
    TooLarge,
    #[error("cancelled")]
    Cancelled,
    #[error("no such page")]
    NoPage,
    #[error("region outside the image")]
    BadRegion,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Largest width or height accepted.
pub const MAX_DIMENSION: u32 = 1 << 22;
/// Largest decode output in bytes.
pub const MAX_OUTPUT_BYTES: u64 = 4 << 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    None,
    Lzw,
    Deflate,
    PackBits,
    Jpeg,
    /// CCITT Modified Huffman (2: rows byte-aligned; 32771: rows word-aligned).
    CcittRle {
        word_aligned: bool,
    },
    /// CCITT Group 3 (T.4, compression 3).
    Group3,
    /// CCITT Group 4 (T.6, compression 4).
    Group4,
    /// Old-style (TIFF 6.0) JPEG, compression 6.
    OldJpeg,
    /// LZMA2 in an xz stream (34925).
    Lzma,
    /// Zstandard (50000).
    Zstd,
    /// WebP (50001).
    Webp,
    /// JPEG XL (50002, or the draft code 52546).
    Jxl,
    /// JPEG 2000 codestreams: Aperio YCbCr (33003), Aperio RGB (33005), 33004 and 34712.
    Jpeg2000(u16),
    /// Esri LERC v2 (34887), optionally Deflate- or Zstandard-wrapped (LercParameters).
    Lerc,
    /// SGI LogL / LogLuv32 run-length coding (34676).
    SgiLog,
    /// SGI LogLuv24 (34677).
    SgiLog24,
    /// ThunderScan 4-bit (32809).
    ThunderScan,
    /// NeXT 2-bit (32766).
    Next,
    /// Pixar's 11-bit log coding, Deflate-compressed (PixarLog, 32909).
    PixarLog,
    /// JBIG1 (T.82) bi-level images, one per unit (34661).
    Jbig,
    /// JPEG (compression 7) with 16-bit samples: lossless frames (SOF3), decoded at full
    /// precision into the raw samples like the other codecs, not through the 8-bit JPEG path.
    Jpeg16,
    Other(u16),
}

impl Compression {
    fn from_tag(v: u64) -> Compression {
        match v {
            1 => Compression::None,
            2 => Compression::CcittRle {
                word_aligned: false,
            },
            32771 => Compression::CcittRle { word_aligned: true },
            3 => Compression::Group3,
            4 => Compression::Group4,
            5 => Compression::Lzw,
            6 => Compression::OldJpeg,
            8 | 32946 => Compression::Deflate,
            32773 => Compression::PackBits,
            7 => Compression::Jpeg,
            34925 => Compression::Lzma,
            50000 => Compression::Zstd,
            50001 => Compression::Webp,
            50002 | 52546 => Compression::Jxl,
            33003 | 33004 | 33005 | 34712 => Compression::Jpeg2000(v as u16),
            34887 => Compression::Lerc,
            34676 => Compression::SgiLog,
            34677 => Compression::SgiLog24,
            32809 => Compression::ThunderScan,
            32766 => Compression::Next,
            32909 => Compression::PixarLog,
            34661 => Compression::Jbig,
            v => Compression::Other(v.min(u16::MAX as u64) as u16),
        }
    }
    pub fn name(self) -> String {
        match self {
            Compression::None => "uncompressed".into(),
            Compression::Lzw => "LZW".into(),
            Compression::Deflate => "Deflate".into(),
            Compression::PackBits => "PackBits".into(),
            Compression::Jpeg => "JPEG".into(),
            Compression::OldJpeg => "old-style JPEG".into(),
            Compression::CcittRle { .. } => "CCITT RLE".into(),
            Compression::Group3 => "CCITT Group 3".into(),
            Compression::Group4 => "CCITT Group 4".into(),
            Compression::Lerc => "LERC".into(),
            Compression::SgiLog => "SGI LogLuv".into(),
            Compression::SgiLog24 => "SGI LogLuv24".into(),
            Compression::ThunderScan => "ThunderScan".into(),
            Compression::Next => "NeXT".into(),
            Compression::PixarLog => "PixarLog".into(),
            Compression::Jbig => "JBIG".into(),
            Compression::Jpeg16 => "16-bit lossless JPEG".into(),
            Compression::Lzma => "LZMA".into(),
            Compression::Zstd => "ZSTD".into(),
            Compression::Webp => "WebP".into(),
            Compression::Jxl => "JPEG XL".into(),
            Compression::Jpeg2000(_) => "JPEG 2000".into(),
            // Pixar's 10-bit film coding: no description or sample exists, and libtiff has
            // no codec for it either.
            Compression::Other(32908) => "PixarFilm (no public description)".into(),
            Compression::Other(v) => format!("compression {v}"),
        }
    }

    /// Decoded through `specular-jpeg` (new- or old-style JPEG).
    pub fn is_jpeg(self) -> bool {
        matches!(self, Compression::Jpeg | Compression::OldJpeg)
    }

    /// CCITT bilevel coding.
    pub(crate) fn is_ccitt(self) -> bool {
        matches!(
            self,
            Compression::CcittRle { .. } | Compression::Group3 | Compression::Group4
        )
    }

    /// Each unit is a whole image in another format (decoded to raw samples).
    pub(crate) fn is_image_codec(self) -> bool {
        matches!(
            self,
            Compression::Webp | Compression::Jxl | Compression::Jpeg2000(_)
        )
    }
}

/// The inks of a separated image ([`Ifd::inks`]): `None` for C, M, Y, K in that order (the
/// default). InkSet 2 or InkNames name them; process inks by name, spot inks by a colour word
/// in their name ("Orange", "PANTONE Green"), others as a mid grey. Unnamed InkSet 2 inks:
/// four are taken as CMYK, other counts as grey inks.
fn inks(
    ink_set: u64,
    count: Option<u64>,
    names: Option<&[u8]>,
    samples: u16,
    extra: usize,
) -> Option<Box<[[u8; 3]]>> {
    let names: Vec<String> = names
        .unwrap_or(&[])
        .split(|&b| b == 0)
        .filter(|n| !n.is_empty())
        .map(|n| String::from_utf8_lossy(n).trim().to_ascii_lowercase())
        .collect();
    if ink_set != 2 && names.is_empty() {
        return None;
    }
    let n = count
        .map(|c| c as usize)
        .unwrap_or_else(|| usize::from(samples).saturating_sub(extra))
        .clamp(1, usize::from(samples.max(1)));
    if names.is_empty() && n == 4 {
        return None;
    }
    let colour = |name: Option<&String>| -> [u8; 3] {
        const WORDS: [(&str, [u8; 3]); 17] = [
            ("cyan", [0, 255, 255]),
            ("magenta", [255, 0, 255]),
            ("yellow", [255, 255, 0]),
            ("black", [0, 0, 0]),
            ("key", [0, 0, 0]),
            ("orange", [255, 128, 0]),
            ("green", [0, 166, 81]),
            ("violet", [128, 0, 160]),
            ("purple", [128, 0, 128]),
            ("blue", [0, 71, 187]),
            ("red", [237, 28, 36]),
            ("pink", [255, 105, 180]),
            ("brown", [120, 72, 36]),
            ("gold", [212, 175, 55]),
            ("silver", [192, 192, 192]),
            ("white", [255, 255, 255]),
            ("gray", [128, 128, 128]),
        ];
        let Some(name) = name else {
            return [128; 3];
        };
        let name = name.replace("grey", "gray");
        WORDS
            .iter()
            .find(|(w, _)| {
                name.split(|c: char| !c.is_ascii_alphabetic())
                    .any(|t| t == *w)
            })
            .map_or([128; 3], |&(_, c)| c)
    };
    let inks: Vec<[u8; 3]> = (0..n).map(|i| colour(names.get(i))).collect();
    if inks == [[0, 255, 255], [255, 0, 255], [255, 255, 0], [0, 0, 0]] {
        return None;
    }
    Some(inks.into_boxed_slice())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Photometric {
    MinIsWhite,
    MinIsBlack,
    Rgb,
    Palette,
    Mask,
    Cmyk,
    YCbCr,
    /// L*a*b* (D50) with signed a*/b* (TIFF 6.0).
    CieLab,
    /// L*a*b* in the ICC encoding: a*/b* unsigned, offset by half the range.
    IccLab,
    /// L*a*b* in the ITU-T T.42 encoding (ranges from the Decode tag or the defaults).
    ItuLab,
    Other(u16),
}

impl Photometric {
    fn from_tag(v: u64) -> Photometric {
        match v {
            0 => Photometric::MinIsWhite,
            1 => Photometric::MinIsBlack,
            2 => Photometric::Rgb,
            3 => Photometric::Palette,
            4 => Photometric::Mask,
            5 => Photometric::Cmyk,
            6 => Photometric::YCbCr,
            8 => Photometric::CieLab,
            9 => Photometric::IccLab,
            10 => Photometric::ItuLab,
            v => Photometric::Other(v.min(u16::MAX as u64) as u16),
        }
    }
    pub fn name(self) -> String {
        match self {
            Photometric::MinIsWhite | Photometric::MinIsBlack => "grey".into(),
            Photometric::Rgb => "RGB".into(),
            Photometric::Palette => "palette".into(),
            Photometric::Mask => "mask".into(),
            Photometric::Cmyk => "CMYK".into(),
            Photometric::YCbCr => "YCbCr".into(),
            Photometric::CieLab => "CIELab".into(),
            Photometric::IccLab => "ICCLab".into(),
            Photometric::ItuLab => "ITULab".into(),
            Photometric::Other(32844) => "LogL".into(),
            Photometric::Other(32845) => "LogLuv".into(),
            Photometric::Other(v) => format!("photometric {v}"),
        }
    }

    /// One of the three L*a*b* encodings.
    pub fn is_lab(self) -> bool {
        matches!(
            self,
            Photometric::CieLab | Photometric::IccLab | Photometric::ItuLab
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleFormat {
    Uint,
    Int,
    Float,
}

/// Output pixel format of a page. The 16-bit formats hold native-endian `u16` samples
/// (0..=65535) and come only from [`DecodeOptions::wide`] decodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Gray8,
    /// Interleaved RGBA, straight alpha.
    Rgba8,
    Gray16,
    /// Grey and straight alpha.
    GrayAlpha16,
    /// Interleaved RGBA, straight alpha (65535 when the image has none).
    Rgba16,
    /// Interleaved RGBA of native-endian IEEE half floats: linear light (1.0 = reference
    /// white, values above kept), straight alpha (1.0 when the image has none). Float images'
    /// [`DecodeOptions::wide`] decodes.
    Rgba16F,
    /// One sample per pixel as a native-endian `f32`, the value as stored (integers unscaled,
    /// NaN kept, missing data and no-data NaN): [`Tiff::decode_values`] only.
    GrayF32,
}

impl PixelFormat {
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            PixelFormat::Gray8 => 1,
            PixelFormat::Gray16 => 2,
            PixelFormat::Rgba8 | PixelFormat::GrayAlpha16 | PixelFormat::GrayF32 => 4,
            PixelFormat::Rgba16 | PixelFormat::Rgba16F => 8,
        }
    }

    /// 16 bits per sample.
    pub fn is_wide(self) -> bool {
        matches!(
            self,
            PixelFormat::Gray16 | PixelFormat::GrayAlpha16 | PixelFormat::Rgba16
        )
    }
}

/// One stored raster (an IFD): a page or one of its pyramid levels.
#[derive(Clone, Debug)]
pub struct Ifd {
    pub width: u32,
    pub height: u32,
    pub bits: u16,
    pub samples: u16,
    pub sample_format: SampleFormat,
    /// SampleFormat 5 or 6: complex values, each stored as two samples (real then
    /// imaginary) that `samples`, `bits` and `sample_format` describe.
    pub complex: bool,
    pub photometric: Photometric,
    pub compression: Compression,
    pub predictor: u16,
    /// PlanarConfiguration = 2 (one plane per sample).
    pub planar: bool,
    /// FillOrder = 2 (LSB first).
    pub(crate) lsb_fill: bool,
    pub tiled: bool,
    /// Tile size, or (width, rows per strip) for strips.
    pub unit_w: u32,
    pub unit_h: u32,
    pub(crate) offsets: Vec<u64>,
    pub(crate) counts: Vec<u64>,
    pub(crate) colormap: Option<Vec<u16>>,
    /// Index of the alpha sample and whether it is associated (premultiplied).
    pub alpha: Option<(usize, bool)>,
    pub(crate) jpeg_tables: Option<Range<usize>>,
    pub(crate) subsampling: (u16, u16),
    /// Separated images with inks other than C, M, Y, K in that order (InkSet 2, or InkNames
    /// in another order): the colour of each ink on white paper, in sample order.
    pub(crate) inks: Option<Box<[[u8; 3]]>>,
    /// The Decode tag (ITULab ranges: L min/max, a* min/max, b* min/max).
    pub(crate) decode: Option<[f64; 6]>,
    pub orientation: u8,
    pub icc: Option<Range<usize>>,
    pub(crate) subfile_type: u64,
    pub(crate) sub_ifds: Vec<u64>,
    /// T4Options (Group 3) / T6Options (Group 4).
    pub(crate) fax_options: u32,
    /// LercParameters: version and the extra compression (0 none, 1 Deflate, 2 Zstandard).
    pub(crate) lerc: [u32; 2],
    /// Old-style JPEG tables and stream (compression 6).
    pub(crate) ojpeg: Option<Box<ojpeg::Tags>>,
    /// GeoTIFF and GDAL tags, as stored (interpreted by the caller).
    pub geo: Option<Box<GeoTags>>,
    /// How the samples are stored when that differs from the fields above (which then say
    /// how they are read): wide integers, LogL/LogLuv, subsampled YCbCr.
    pub(crate) stored: Option<variants::Stored>,
    /// The stored bits per sample and photometric interpretation, for descriptions.
    pub(crate) stored_as: (u16, Photometric),
}

/// The GeoTIFF tags (ModelPixelScale 33550, ModelTiepoint 33922, ModelTransformation 34264,
/// GeoKeyDirectory 34735, GeoDoubleParams 34736, GeoAsciiParams 34737) and GDAL's
/// GDAL_METADATA 42112 and GDAL_NODATA 42113, as stored.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GeoTags {
    pub pixel_scale: Vec<f64>,
    pub tiepoints: Vec<f64>,
    pub transformation: Vec<f64>,
    pub key_directory: Vec<u16>,
    pub double_params: Vec<f64>,
    pub ascii_params: String,
    pub gdal_metadata: Option<String>,
    pub gdal_nodata: Option<String>,
}

impl GeoTags {
    fn parse(r: &ifd::Reader, entries: &[ifd::Entry]) -> Option<Box<GeoTags>> {
        use ifd::find;
        const CAP: usize = 1 << 16;
        let reals = |tag| {
            find(entries, tag)
                .and_then(|e| r.reals(e).ok())
                .map(|mut v| {
                    v.truncate(CAP);
                    v
                })
                .unwrap_or_default()
        };
        let ascii = |tag| {
            let e = find(entries, tag)?;
            let b = r.data.get(r.range(e).ok()?)?;
            let b = &b[..b.len().min(1 << 20)];
            let end = b.iter().rposition(|&c| c != 0).map_or(0, |i| i + 1);
            Some(String::from_utf8_lossy(&b[..end]).into_owned())
        };
        let g = GeoTags {
            pixel_scale: reals(33550),
            tiepoints: reals(33922),
            transformation: reals(34264),
            key_directory: find(entries, 34735)
                .and_then(|e| r.uints(e).ok())
                .map(|v| v.into_iter().take(CAP).map(|x| x as u16).collect())
                .unwrap_or_default(),
            double_params: reals(34736),
            ascii_params: ascii(34737).unwrap_or_default(),
            gdal_metadata: ascii(42112),
            gdal_nodata: ascii(42113),
        };
        (g != GeoTags::default()).then(|| Box::new(g))
    }

    /// The GeoTIFF tags of the first IFD of `data`, whether or not it holds a readable
    /// image: a GeoJP2 box's degenerate TIFF (often 1×1 with no strips).
    pub fn of_first_ifd(data: &[u8]) -> Option<GeoTags> {
        let (r, first) = ifd::Reader::new(data).ok()?;
        let (entries, _) = r.ifd(first).ok()?;
        GeoTags::parse(&r, &entries).map(|g| *g)
    }
}

impl Ifd {
    fn parse(r: &ifd::Reader, entries: &[ifd::Entry]) -> Result<Ifd> {
        use ifd::find;
        let get = |tag: u16| -> Result<Option<u64>> {
            match find(entries, tag) {
                Some(e) => r.uint(e).map(Some),
                None => Ok(None),
            }
        };
        let width = get(256)?.ok_or(Error::Malformed("no ImageWidth"))?;
        let height = get(257)?.ok_or(Error::Malformed("no ImageLength"))?;
        if width == 0 || height == 0 {
            return Err(Error::Malformed("zero size"));
        }
        if width > MAX_DIMENSION as u64 || height > MAX_DIMENSION as u64 {
            return Err(Error::TooLarge);
        }
        let (width, height) = (width as u32, height as u32);
        let samples = get(277)?.unwrap_or(1);
        if samples == 0 || samples > 64 {
            return Err(Error::Malformed("SamplesPerPixel"));
        }
        let samples = samples as u16;
        let bits = match find(entries, 258) {
            Some(e) => {
                let mut v = r.uints(e)?;
                // Old Zeiss LSM writers store two BitsPerSample values out of line although
                // they fit in the entry: the entry then holds an offset.
                if e.typ == 3
                    && e.count == 2
                    && (v[0] != v[1] || v[0] > 64)
                    && let Some(at) = r.data.get(e.pos as usize..e.pos as usize + 4)
                {
                    let off = if r.le {
                        u32::from_le_bytes([at[0], at[1], at[2], at[3]])
                    } else {
                        u32::from_be_bytes([at[0], at[1], at[2], at[3]])
                    } as usize;
                    if let Some(b) = r.data.get(off..off.saturating_add(4)) {
                        let get = |i: usize| {
                            u64::from(if r.le {
                                u16::from_le_bytes([b[i], b[i + 1]])
                            } else {
                                u16::from_be_bytes([b[i], b[i + 1]])
                            })
                        };
                        let out = vec![get(0), get(2)];
                        if out[0] == out[1] && (1..=64).contains(&out[0]) {
                            v = out;
                        }
                    }
                }
                // LSM writers also give one value more than there are samples (a zero).
                v.truncate(usize::from(samples).max(1));
                let b = *v.first().ok_or(Error::Malformed("BitsPerSample"))?;
                if v.iter().any(|&x| x != b) {
                    return Err(Error::Unsupported("different bit depths per sample".into()));
                }
                b
            }
            None => 1,
        };
        let format_code = match find(entries, 339) {
            Some(e) => r.uints(e)?.first().copied(),
            None => None,
        };
        // Complex integers (5) and floats (6): each sample is a (real, imaginary) pair,
        // read as two samples of half the bits.
        let complex = matches!(format_code, Some(5 | 6));
        if bits == 0 || bits > if complex { 128 } else { 64 } {
            return Err(Error::Malformed("BitsPerSample"));
        }
        let mut bits = bits as u16;
        let sample_format = match format_code {
            Some(2 | 5) => SampleFormat::Int,
            Some(3 | 6) => SampleFormat::Float,
            _ => SampleFormat::Uint,
        };
        let samples = if complex {
            if samples > 1 && get(284)? == Some(2) {
                return Err(Error::Unsupported("planar complex samples".into()));
            }
            if !matches!(
                (sample_format, bits),
                (SampleFormat::Int, 32 | 64) | (SampleFormat::Float, 64 | 128)
            ) {
                return Err(Error::Unsupported(format!("{bits}-bit complex samples")));
            }
            bits /= 2;
            samples * 2
        } else {
            samples
        };
        let compression = match Compression::from_tag(get(259)?.unwrap_or(1)) {
            Compression::Jpeg if bits == 16 => Compression::Jpeg16,
            c => c,
        };
        // Old-style JPEG YCbCr without SamplesPerPixel (libtiff assumes three).
        let samples = if compression == Compression::OldJpeg
            && find(entries, 277).is_none()
            && get(262)? == Some(6)
        {
            3
        } else {
            samples
        };
        let photometric = match get(262)? {
            // Zeiss LSM says RGB for any number of channels: fewer than three are grey bands.
            Some(2) if samples < 3 && find(entries, 34412).is_some() => Photometric::MinIsBlack,
            Some(v) => Photometric::from_tag(v),
            None if samples >= 3 => Photometric::Rgb,
            None => Photometric::MinIsBlack,
        };
        let planar = get(284)?.unwrap_or(1) == 2 && samples > 1 && !complex;
        let predictor = get(317)?.unwrap_or(1).min(u16::MAX as u64) as u16;
        let lsb_fill = get(266)? == Some(2);

        let subsampling = match find(entries, 530) {
            Some(e) => {
                let v = r.uints(e)?;
                (
                    v.first().copied().unwrap_or(2) as u16,
                    v.get(1).copied().unwrap_or(2) as u16,
                )
            }
            None => (2, 2),
        };
        // Subsampled YCbCr outside JPEG is stored in blocks, not rows.
        let blocked =
            photometric == Photometric::YCbCr && !compression.is_jpeg() && subsampling != (1, 1);

        let tiled = find(entries, 322).is_some() || find(entries, 324).is_some();
        let (unit_w, unit_h, off_tag, cnt_tag) = if tiled {
            let tw = get(322)?.ok_or(Error::Malformed("no TileWidth"))?;
            let th = get(323)?.ok_or(Error::Malformed("no TileLength"))?;
            if tw == 0 || th == 0 || tw > 1 << 16 || th > 1 << 16 || tw * th > 1 << 26 {
                return Err(Error::Malformed("tile size"));
            }
            // Early tiled writers kept the tile offsets and counts in the strip tags.
            if find(entries, 324).is_none() && find(entries, 273).is_some() {
                (tw as u32, th as u32, 273, 279)
            } else {
                (tw as u32, th as u32, 324, 325)
            }
        } else {
            let rps = get(278)?.unwrap_or(height as u64).clamp(1, height as u64);
            (width, rps as u32, 273, 279)
        };
        let units_x = width.div_ceil(unit_w) as u64;
        let units_y = height.div_ceil(unit_h) as u64;
        let planes = if planar { samples as u64 } else { 1 };
        let n_units = units_x * units_y * planes;
        if n_units > ifd::MAX_VALUES {
            return Err(Error::TooLarge);
        }
        let ojpeg = if compression == Compression::OldJpeg {
            Some(Box::new(ojpeg::Tags::parse(r, entries)?))
        } else {
            None
        };
        let mut unit_h = unit_h;
        let (offsets, mut counts) = match (find(entries, off_tag), &ojpeg) {
            (Some(e), _) => (
                r.uints(e)?,
                match find(entries, cnt_tag) {
                    Some(e) => r.uints(e)?,
                    None => Vec::new(),
                },
            ),
            // Old-style JPEG with only a JPEGInterchangeFormat stream: one strip.
            (None, Some(oj)) if !tiled && oj.jif.is_some() => {
                let j = oj.jif.clone().unwrap_or_default();
                unit_h = height;
                (vec![j.start as u64], vec![j.len() as u64])
            }
            (None, _) => return Err(Error::Malformed("no strip/tile offsets")),
        };
        if counts.is_empty() {
            if compression != Compression::None {
                return Err(Error::Malformed("no strip/tile byte counts"));
            }
            // Uncompressed without byte counts: the expected size of each unit.
            let row = row_bytes(unit_w, bits, if planar { 1 } else { samples });
            r.charge(offsets.len() as u64)?;
            counts = vec![row * unit_h as u64; offsets.len()];
        }
        if (offsets.len() as u64) < n_units || (counts.len() as u64) < n_units {
            // Missing units decode as truncated; but nothing at all is malformed.
            if offsets.is_empty() {
                return Err(Error::Malformed("no strip/tile offsets"));
            }
        }
        let mut offsets = offsets;
        // Separate planes stored back to back under one plane's strips (old ImageJ): each
        // later plane follows the previous one.
        if planar
            && !tiled
            && compression == Compression::None
            && !blocked
            && offsets.len() as u64 == units_x * units_y
            && counts.len() == offsets.len()
        {
            let row = row_bytes(unit_w, bits, 1);
            let plane = row * u64::from(height);
            // Saturating: BigTIFF byte counts are arbitrary 64-bit values.
            let stored = counts.iter().fold(0u64, |a, &c| a.saturating_add(c));
            if stored >= plane * planes {
                r.charge(2 * planes * offsets.len() as u64)?;
                let strip = |s: u64| row * u64::from((height - s as u32 * unit_h).min(unit_h));
                let per = offsets.len() as u64;
                let (o, c): (Vec<u64>, Vec<u64>) = (0..planes)
                    .flat_map(|p| (0..per).map(move |s| (p, s)))
                    .map(|(p, s)| (offsets[s as usize].saturating_add(p * plane), strip(s)))
                    .unzip();
                (offsets, counts) = (o, c);
            }
        }
        if !tiled && compression == Compression::None && !blocked {
            let row = row_bytes(unit_w, bits, if planar { 1 } else { samples });
            if let Some(split) = split_strips(r, &offsets, &counts, height, unit_h, planes, row) {
                (offsets, counts, unit_h) = split;
            }
        }
        let colormap = match find(entries, 320) {
            Some(e) => Some(r.uints(e)?.into_iter().map(|v| v as u16).collect()),
            None => None,
        };
        let alpha = match find(entries, 338) {
            Some(e) => {
                let ex = r.uints(e)?;
                let base = samples as usize - ex.len().min(samples as usize);
                ex.iter()
                    .position(|&v| v == 1 || v == 2)
                    .map(|i| (base + i, ex[i] == 1))
            }
            None => None,
        };
        let jpeg_tables = match find(entries, 347) {
            Some(e) => Some(r.range(e)?),
            None => None,
        };
        let icc = match find(entries, 34675) {
            Some(e) => r.range(e).ok(),
            None => None,
        };
        let decode = match find(entries, 433) {
            Some(e) => r.reals(e).ok().and_then(|v| <[f64; 6]>::try_from(v).ok()),
            None => None,
        };
        let sub_ifds = match find(entries, 330) {
            Some(e) => r.uints(e).unwrap_or_default(),
            None => Vec::new(),
        };
        // Layouts read through `variants`: the fields below say how they are read.
        let stored_as = (bits, photometric);
        let (stored, bits, samples, sample_format, photometric) = match (compression, photometric) {
            (Compression::SgiLog24, Photometric::Other(32845)) if !planar => (
                Some(variants::Stored::LogLuv24),
                32,
                3,
                SampleFormat::Float,
                Photometric::Rgb,
            ),
            // PixarLog's 11-bit log tokens decode to linear light: read as float, whatever
            // depth the file claims (libtiff writes 8 for every PixarLog file).
            (Compression::PixarLog, p) if !matches!(p, Photometric::Palette) => (
                Some(variants::Stored::PixarLog {
                    restart: width.saturating_mul(if planar { 1 } else { u32::from(samples) }),
                }),
                32,
                samples,
                SampleFormat::Float,
                p,
            ),
            // LogL keeps its 16-bit run-length coding under 34677 too (as libtiff reads it).
            (Compression::SgiLog | Compression::SgiLog24, Photometric::Other(32844)) => (
                Some(variants::Stored::LogL),
                32,
                1,
                SampleFormat::Float,
                Photometric::MinIsBlack,
            ),
            (Compression::SgiLog, Photometric::Other(32845)) if !planar => (
                Some(variants::Stored::LogLuv),
                32,
                3,
                SampleFormat::Float,
                Photometric::Rgb,
            ),
            (_, Photometric::YCbCr)
                if blocked
                    && bits == 8
                    && samples == 3
                    && !planar
                    && matches!(subsampling.0, 1 | 2 | 4)
                    && matches!(subsampling.1, 1 | 2 | 4) =>
            {
                (
                    Some(variants::Stored::Subsampled {
                        h: subsampling.0,
                        v: subsampling.1,
                    }),
                    8,
                    3,
                    sample_format,
                    photometric,
                )
            }
            // L*a*b* of other depths (and float): read as 16-bit L*a*b*.
            (_, p)
                if p.is_lab()
                    && !(matches!(bits, 8 | 16) && sample_format != SampleFormat::Float)
                    && (sample_format != SampleFormat::Float
                        || matches!(bits, 16 | 24 | 32 | 64))
                    && !compression.is_ccitt()
                    && !compression.is_jpeg()
                    && !compression.is_image_codec()
                    && compression != Compression::Lerc =>
            {
                let float = sample_format == SampleFormat::Float;
                // ImageMagick writes float L*a*b* as the 16-bit coding over 0–1, and says so
                // by its name or an SMaxSampleValue above any L* (its quantum range);
                // libvips writes the values themselves.
                let unit = float
                    && (find(entries, 305)
                        .and_then(|e| r.range(e).ok())
                        .is_some_and(|g| r.data[g].starts_with(b"ImageMagick"))
                        || find(entries, 341)
                            .and_then(|e| r.reals(e).ok())
                            .is_some_and(|v| v.first().is_some_and(|&m| m > 100.0)));
                let p = if float && !unit {
                    Photometric::CieLab
                } else {
                    p
                };
                (
                    Some(variants::Stored::Lab { bits, float, unit }),
                    16,
                    samples,
                    SampleFormat::Uint,
                    p,
                )
            }
            _ if sample_format != SampleFormat::Float
                && bits > 16
                && bits != 32
                && !compression.is_ccitt()
                && !compression.is_jpeg()
                && !compression.is_image_codec()
                && compression != Compression::Lerc =>
            {
                (
                    Some(variants::Stored::WideInt { bits }),
                    32,
                    samples,
                    sample_format,
                    photometric,
                )
            }
            _ => (None, bits, samples, sample_format, photometric),
        };
        Ok(Ifd {
            width,
            height,
            bits,
            samples,
            sample_format,
            complex,
            photometric,
            compression,
            predictor,
            planar,
            lsb_fill,
            tiled,
            unit_w,
            unit_h,
            offsets,
            counts,
            colormap,
            alpha,
            jpeg_tables,
            subsampling,
            inks: if photometric == Photometric::Cmyk {
                inks(
                    get(332)?.unwrap_or(1),
                    get(334)?,
                    find(entries, 333)
                        .and_then(|e| r.range(e).ok())
                        .map(|g| &r.data[g]),
                    samples,
                    find(entries, 338).map_or(Ok(0), |e| r.uints(e).map(|v| v.len()))?,
                )
            } else {
                None
            },
            decode,
            orientation: get(274)?.unwrap_or(1).clamp(1, 8) as u8,
            icc,
            subfile_type: get(254)?.unwrap_or(0),
            sub_ifds,
            fax_options: get(if compression == Compression::Group4 {
                293
            } else {
                292
            })?
            .unwrap_or(0) as u32,
            lerc: match find(entries, 50674) {
                Some(e) => {
                    let v = r.uints(e)?;
                    [
                        v.first().copied().unwrap_or(2) as u32,
                        v.get(1).copied().unwrap_or(0) as u32,
                    ]
                }
                None => [2, 0],
            },
            ojpeg,
            geo: GeoTags::parse(r, entries),
            stored,
            stored_as,
        })
    }

    /// File offsets of the strips or tiles.
    pub fn unit_offsets(&self) -> &[u64] {
        &self.offsets
    }

    /// Byte counts of the strips or tiles.
    pub fn unit_counts(&self) -> &[u64] {
        &self.counts
    }

    /// The same raster stored `delta` bytes further on: ImageJ and MetaMorph stacks keep
    /// their further planes back to back after the one their only IFD describes.
    pub fn shifted(&self, delta: u64) -> Ifd {
        let mut i = self.clone();
        for o in &mut i.offsets {
            *o = o.saturating_add(delta);
        }
        i
    }

    /// Bytes the raster's strips or tiles take in the file (their byte counts' sum).
    pub fn stored_bytes(&self) -> u64 {
        self.counts.iter().fold(0u64, |a, &c| a.saturating_add(c))
    }

    /// Whether the strips or tiles lie back to back in the file, in order.
    pub fn contiguous(&self) -> bool {
        self.offsets
            .windows(2)
            .zip(&self.counts)
            .all(|(o, &c)| o[0].checked_add(c) == Some(o[1]))
    }

    /// The ColorMap of a palette image: red, then green, then blue, `2^bits` 16-bit entries
    /// each.
    pub fn colormap(&self) -> Option<&[u16]> {
        self.colormap.as_deref()
    }

    /// NewSubfileType (bit 0 reduced resolution, bit 2 mask).
    pub fn subfile_type(&self) -> u64 {
        self.subfile_type
    }

    /// Byte range of the JPEGTables tag.
    pub fn jpeg_tables(&self) -> Option<Range<usize>> {
        self.jpeg_tables.clone()
    }

    /// Output format: grey for single-channel grey images, RGBA otherwise.
    pub fn output_format(&self) -> PixelFormat {
        match self.photometric {
            Photometric::MinIsBlack | Photometric::MinIsWhite if self.alpha.is_none() => {
                PixelFormat::Gray8
            }
            _ => PixelFormat::Rgba8,
        }
    }

    /// The 16-bit output format of integer grey and RGB(A) deeper than 8 bits, or the
    /// half-float HDR format of float grey and RGB(A); `None` for everything else (8-bit and
    /// shallower, palette, CMYK, YCbCr, JPEG).
    pub fn wide_format(&self) -> Option<PixelFormat> {
        if self.sample_format == SampleFormat::Float {
            return match self.photometric {
                Photometric::MinIsBlack | Photometric::MinIsWhite | Photometric::Rgb => {
                    Some(PixelFormat::Rgba16F)
                }
                _ => None,
            };
        }
        if self.bits <= 8 || self.compression.is_jpeg() {
            return None;
        }
        let alpha = self.alpha.is_some_and(|(i, _)| i < self.samples as usize);
        match self.photometric {
            Photometric::MinIsBlack | Photometric::MinIsWhite if alpha => {
                Some(PixelFormat::GrayAlpha16)
            }
            Photometric::MinIsBlack | Photometric::MinIsWhite => Some(PixelFormat::Gray16),
            Photometric::Rgb => Some(PixelFormat::Rgba16),
            _ => None,
        }
    }

    /// The format a decode with `wide` set to `wide` gives.
    pub fn format_for(&self, wide: bool) -> PixelFormat {
        match self.wide_format() {
            Some(f) if wide => f,
            _ => self.output_format(),
        }
    }

    /// How the L*a*b* samples of an 8/16-bit L*a*b* image (or its JPEG units, 8-bit, in
    /// native order) are coded; `le` is the file's byte order.
    pub fn lab_coding(&self, le: bool) -> Option<LabCoding> {
        let wide = self.bits == 16 && !self.compression.is_jpeg();
        let max = if wide { 65535.0 } else { 255.0 };
        let mut c = LabCoding {
            wide,
            big_endian: !le,
            signed: [false; 3],
            scale: [100.0 / max, 1.0, 1.0],
            offset: [0.0; 3],
        };
        match self.photometric {
            // TIFF 6.0: a*/b* signed, 16-bit in 1/256 units.
            Photometric::CieLab => {
                c.signed = [false, true, true];
                if wide {
                    c.scale[1..].fill(1.0 / 256.0);
                }
            }
            // Adobe's ICCLab: a*/b* offset by 128 (8-bit) or 32768/257 (16-bit, ICC v4).
            Photometric::IccLab => {
                c.offset[1..].fill(-128.0);
                if wide {
                    c.scale[1..].fill(1.0 / 257.0);
                }
            }
            // ITU-T T.42 / RFC 2301: linear ranges, by default L 0..100, a* −85..85,
            // b* −75..125 (in 1/255 steps).
            Photometric::ItuLab => {
                let d = self.decode.unwrap_or([
                    0.0,
                    100.0,
                    -21760.0 / 255.0,
                    21590.0 / 255.0,
                    -19200.0 / 255.0,
                    31800.0 / 255.0,
                ]);
                if d.iter().any(|v| !v.is_finite() || v.abs() > 1e4) {
                    return None;
                }
                for k in 0..3 {
                    c.scale[k] = ((d[2 * k + 1] - d[2 * k]) / max as f64) as f32;
                    c.offset[k] = d[2 * k] as f32;
                }
            }
            _ => return None,
        }
        Some(c)
    }

    /// Number of colour samples the photometric interpretation needs.
    fn color_samples(&self) -> u16 {
        match self.photometric {
            Photometric::Rgb | Photometric::YCbCr => 3,
            p if p.is_lab() => 3,
            Photometric::Cmyk => self.inks.as_ref().map_or(4, |i| i.len() as u16),
            _ => 1,
        }
    }

    /// Bits per sample as stored in the file. [`Ifd::bits`] says how the samples are read,
    /// which differs for integers wider than 16 bits (other than 32: read as 32) and SGI
    /// LogL/LogLuv (16 as stored, read as 32-bit float).
    pub fn stored_bits(&self) -> u16 {
        self.stored_as.0
    }

    /// Samples per pixel as stored (LogL/LogLuv pack a pixel into one value).
    pub(crate) fn stored_samples(&self) -> u16 {
        match self.stored {
            Some(variants::Stored::LogL | variants::Stored::LogLuv) => 1,
            _ => self.samples,
        }
    }

    /// Whether this IFD can be decoded; the error says why not.
    pub fn check_supported(&self) -> Result<()> {
        let unsup = |s: String| Err(Error::Unsupported(s));
        let c = self.compression;
        match c {
            Compression::Other(_) => return unsup(c.name()),
            Compression::Jpeg | Compression::OldJpeg => {
                // 12-bit JPEG units are shown at 8 bits.
                if self.bits != 8 && !(self.bits == 12 && c == Compression::Jpeg) {
                    return unsup(format!("{}-bit {}", self.bits, c.name()));
                }
                // Planar: one single-component JPEG per sample (RGB or grey, alpha too).
                if self.planar
                    && (c != Compression::Jpeg
                        || !matches!(
                            self.photometric,
                            Photometric::Rgb | Photometric::MinIsBlack | Photometric::MinIsWhite
                        )
                        || self.samples > 4)
                {
                    return unsup(format!("planar {} {}", c.name(), self.photometric.name()));
                }
                if let Some(oj) = &self.ojpeg
                    && oj.process != 1
                {
                    return unsup(format!("old-style JPEG process {}", oj.process));
                }
            }
            _ if c.is_ccitt() => {
                if self.bits != 1 || self.samples != 1 {
                    return unsup(format!("{}-bit {}", self.bits, c.name()));
                }
                if !matches!(
                    self.photometric,
                    Photometric::MinIsWhite | Photometric::MinIsBlack
                ) {
                    return unsup(format!("{} {}", c.name(), self.photometric.name()));
                }
                // Uncompressed mode is not decoded: no encoder writes it, no file uses it, and
                // no reader to check against decodes it (libtiff, Ghostscript, MuPDF and
                // pdfium refuse or skip it).
                if self.fax_options & 2 != 0 {
                    return unsup(format!("{} uncompressed mode", c.name()));
                }
            }
            Compression::Webp => {
                if self.bits != 8 || !matches!(self.samples, 3 | 4) || self.planar {
                    return unsup(format!(
                        "{}-bit WebP with {} samples",
                        self.bits, self.samples
                    ));
                }
            }
            Compression::Jxl | Compression::Jpeg2000(_) => {
                let float = self.sample_format == SampleFormat::Float;
                if float && c != Compression::Jxl {
                    return unsup(
                        "JPEG 2000 with floating-point samples (its codestreams hold integers)"
                            .into(),
                    );
                }
                if float && !(matches!(self.bits, 16 | 32) && matches!(self.samples, 1 | 3)) {
                    return unsup(format!(
                        "{}-bit floating-point JPEG XL with {} samples",
                        self.bits, self.samples
                    ));
                }
                let int_ok = matches!(self.bits, 8 | 16)
                    && (self.sample_format == SampleFormat::Uint
                        || (self.sample_format == SampleFormat::Int && c != Compression::Jxl));
                if (!float && !int_ok) || self.samples > 4 || (self.planar && self.samples > 1) {
                    return unsup(format!(
                        "{}-bit {} {} with {} samples",
                        self.bits,
                        match self.sample_format {
                            SampleFormat::Int => "signed",
                            SampleFormat::Float => "floating-point",
                            _ => "unsigned",
                        },
                        c.name(),
                        self.samples
                    ));
                }
            }
            Compression::Lerc if !matches!(self.lerc[1], 0..=2) => {
                return unsup(format!("LERC with extra compression {}", self.lerc[1]));
            }
            Compression::SgiLog24 if self.stored.is_none() => {
                return unsup(format!(
                    "SGI LogLuv24 with {}{}",
                    self.photometric.name(),
                    if self.planar { ", planar" } else { "" }
                ));
            }
            Compression::SgiLog if self.stored.is_none() => {
                return unsup(format!(
                    "SGI LogLuv with {}{}",
                    self.photometric.name(),
                    if self.planar { ", planar" } else { "" }
                ));
            }
            Compression::ThunderScan => {
                if self.bits != 4 || self.samples != 1 {
                    return unsup(format!(
                        "{}-bit ThunderScan with {} samples",
                        self.bits, self.samples
                    ));
                }
            }
            Compression::Jpeg16 => {
                if self.samples > 4 || self.sample_format != SampleFormat::Uint {
                    return unsup(format!(
                        "16-bit JPEG with {} {:?} samples",
                        self.samples, self.sample_format
                    ));
                }
                if self.predictor != 1 {
                    return unsup(format!("predictor {} with JPEG", self.predictor));
                }
            }
            Compression::Next | Compression::Jbig => {
                let bits = if c == Compression::Next { 2 } else { 1 };
                if self.bits != bits || self.samples != 1 {
                    return unsup(format!(
                        "{}-bit {} with {} samples",
                        self.bits,
                        c.name(),
                        self.samples
                    ));
                }
                if self.predictor != 1 {
                    return unsup(format!("predictor {} with {}", self.predictor, c.name()));
                }
            }
            Compression::PixarLog => {
                if self.stored.is_none() {
                    return unsup(format!("PixarLog {}", self.photometric.name()));
                }
                // libtiff ignores the tag; no writer sets it.
                if self.predictor != 1 {
                    return unsup(format!("predictor {} with PixarLog", self.predictor));
                }
            }
            _ if matches!(self.stored_as.1, Photometric::Other(32844 | 32845))
                && !matches!(c, Compression::SgiLog | Compression::SgiLog24) =>
            {
                return unsup(format!(
                    "{} without SGI LogLuv coding",
                    self.stored_as.1.name()
                ));
            }
            _ => {}
        }
        if self.samples < self.color_samples() {
            return Err(Error::Malformed("too few samples for the colour model"));
        }
        match self.photometric {
            Photometric::MinIsBlack | Photometric::MinIsWhite | Photometric::Rgb => {}
            Photometric::Palette => {
                // ColorMap holds 3 · 2^bits values: deeper palettes would not fit the IFD
                // value limit (and no writer makes them).
                let bits = self.stored_bits();
                if bits > 22 || self.sample_format != SampleFormat::Uint {
                    return unsup(format!("{bits}-bit palette"));
                }
                let need = 3usize << bits;
                if self.colormap.as_ref().is_none_or(|c| c.len() < need) {
                    return Err(Error::Malformed("ColorMap missing or short"));
                }
            }
            Photometric::Cmyk => {}
            Photometric::YCbCr => {
                if !self.compression.is_jpeg()
                    && self.subsampling != (1, 1)
                    && self.stored.is_none()
                {
                    return unsup(format!(
                        "{}-bit YCbCr subsampled {}×{}{}",
                        self.bits,
                        self.subsampling.0,
                        self.subsampling.1,
                        if self.planar { ", planar" } else { "" }
                    ));
                }
                if self.stored.is_some() && self.predictor != 1 {
                    return unsup("subsampled YCbCr with a predictor".into());
                }
            }
            p if p.is_lab() => {
                if !matches!(self.bits, 8 | 16) || self.sample_format == SampleFormat::Float {
                    return unsup(format!("{}-bit {}", self.bits, p.name()));
                }
                if self.compression.is_jpeg() && self.samples != 3 {
                    return unsup(format!("JPEG {} with extra samples", p.name()));
                }
                if self.lab_coding(true).is_none() {
                    return Err(Error::Malformed("Decode ranges"));
                }
            }
            p => return unsup(format!("{} colour", p.name())),
        }
        let ok_bits = match self.sample_format {
            SampleFormat::Float => matches!(self.bits, 16 | 24 | 32 | 64),
            _ => self.bits <= 16 || self.bits == 32,
        };
        if matches!(
            self.stored,
            Some(variants::Stored::LogL | variants::Stored::LogLuv)
        ) && self.predictor != 1
        {
            return unsup(format!("predictor {} with SGI LogLuv", self.predictor));
        }
        if !ok_bits {
            return unsup(format!(
                "{}-bit {:?} samples",
                self.bits, self.sample_format
            ));
        }
        let stored_bits = self.stored_bits();
        match self.predictor {
            1 => {}
            2 if stored_bits <= 64 => {}
            3 if self.sample_format == SampleFormat::Float
                || matches!(self.stored, Some(variants::Stored::Lab { float: true, .. })) => {}
            p => return unsup(format!("predictor {p}")),
        }
        Ok(())
    }

    /// Short technical description, e.g. "tiled 256×256, LZW, RGB 16-bit".
    pub fn description(&self) -> String {
        let layout = if self.tiled {
            format!("tiled {}×{}", self.unit_w, self.unit_h)
        } else {
            "strips".to_string()
        };
        let fmt = match self.sample_format {
            SampleFormat::Float => " float",
            SampleFormat::Int => " signed",
            SampleFormat::Uint => "",
        };
        let (bits, photometric) = if self.stored.is_some() {
            self.stored_as
        } else {
            (self.bits, self.photometric)
        };
        let fmt = if matches!(photometric, Photometric::Other(32844 | 32845)) {
            ""
        } else {
            fmt
        };
        format!(
            "{layout}, {}, {}{} {}-bit{fmt}{}",
            self.compression.name(),
            photometric.name(),
            if self.alpha.is_some() { "A" } else { "" },
            bits,
            if self.planar { ", planar" } else { "" }
        )
    }
}

/// Bytes in one row of `w` pixels with `spp` samples of `bits` bits (rows are byte-aligned).
/// Uncompressed strips taller than this are split into virtual sub-strips.
const SPLIT_MIN_ROWS: u32 = 256;
/// Target size of one virtual sub-strip.
const SPLIT_TARGET_BYTES: u64 = 1 << 20;

/// Re-describes tall uncompressed strips (e.g. one 675 MB strip for a whole image) as short
/// virtual strips over the same bytes, so regions read only their rows and scaled decodes stream
/// in small pieces. Rows are independent in uncompressed data (predictors work per row too).
/// Missing parts become missing units (offset past any file), never zero-filled ones.
/// The virtual strips are charged to `r` (a tiny IFD can describe 2^24 of them); past the
/// file's budget the strips stay as stored.
fn split_strips(
    r: &ifd::Reader,
    offsets: &[u64],
    counts: &[u64],
    height: u32,
    unit_h: u32,
    planes: u64,
    row: u64,
) -> Option<(Vec<u64>, Vec<u64>, u32)> {
    if unit_h <= SPLIT_MIN_ROWS || row == 0 {
        return None;
    }
    let units_y = height.div_ceil(unit_h);
    // A power of two around 1 MiB per sub-strip, aligning with power-of-two scales; it must
    // divide the strip height unless the whole plane is one strip.
    let want = (SPLIT_TARGET_BYTES / row).clamp(8, SPLIT_MIN_ROWS as u64) as u32;
    let rows = (3..=8)
        .rev()
        .map(|k| 1u32 << k)
        .find(|&r| r <= want && (units_y == 1 || unit_h.is_multiple_of(r)))?;
    let per = unit_h.div_ceil(rows);
    let n = (height.div_ceil(rows) as u64).checked_mul(planes)?;
    if n > ifd::MAX_VALUES {
        return None;
    }
    r.charge(2 * n).ok()?;
    let sub = row * rows as u64;
    let (mut o, mut c) = (
        Vec::with_capacity(n as usize),
        Vec::with_capacity(n as usize),
    );
    for p in 0..planes {
        for s in 0..units_y as u64 {
            let idx = (p * units_y as u64 + s) as usize;
            let first = s as u32 * unit_h;
            let subs = (height - first).min(unit_h).div_ceil(rows).min(per);
            let (off, cnt) = match (offsets.get(idx), counts.get(idx)) {
                (Some(&o), Some(&c)) => (o, c),
                _ => (u64::MAX, 1),
            };
            for k in 0..subs as u64 {
                let at = k * sub;
                if cnt == 0 {
                    // Sparse strip: every part is sparse.
                    o.push(off);
                    c.push(0);
                } else if at < cnt && off != u64::MAX {
                    o.push(off.saturating_add(at));
                    c.push((cnt - at).min(sub));
                } else {
                    o.push(u64::MAX);
                    c.push(1);
                }
            }
        }
    }
    Some((o, c, rows))
}

pub(crate) fn row_bytes(w: u32, bits: u16, spp: u16) -> u64 {
    (w as u64 * bits as u64 * spp as u64).div_ceil(8)
}

/// One page of a (possibly multi-page) TIFF with its pyramid levels.
#[derive(Clone, Debug)]
pub struct Page {
    /// Index 0 is the full-resolution image; others are reduced-resolution levels, largest
    /// first, all decodable and of the same output format.
    levels: Vec<Ifd>,
}

impl Page {
    pub fn ifd(&self) -> &Ifd {
        &self.levels[0]
    }
    pub fn width(&self) -> u32 {
        self.levels[0].width
    }
    pub fn height(&self) -> u32 {
        self.levels[0].height
    }
    /// EXIF-style orientation (1..=8).
    pub fn orientation(&self) -> u8 {
        self.levels[0].orientation
    }
    pub fn icc_profile<'a>(&self, data: &'a [u8]) -> Option<&'a [u8]> {
        self.levels[0].icc.clone().and_then(|r| data.get(r))
    }
    pub fn output_format(&self) -> PixelFormat {
        self.levels[0].output_format()
    }
    /// The format of [`DecodeOptions::wide`] decodes, if the page is deeper than 8 bits.
    pub fn wide_format(&self) -> Option<PixelFormat> {
        self.levels[0].wide_format()
    }
    /// The format a decode with these options gives.
    pub fn format_for(&self, opts: &DecodeOptions) -> PixelFormat {
        self.levels[0].format_for(opts.wide)
    }
    /// ExtraSamples marks a sample as alpha. A palette alone is not alpha: TIFF colour maps
    /// hold none.
    pub fn has_alpha(&self) -> bool {
        self.levels[0].alpha.is_some()
    }
    pub fn tiled(&self) -> bool {
        self.levels[0].tiled
    }
    /// Sizes of the stored levels (full resolution first).
    pub fn level_sizes(&self) -> Vec<(u32, u32)> {
        self.levels.iter().map(|l| (l.width, l.height)).collect()
    }
    pub fn check_supported(&self) -> Result<()> {
        self.levels[0].check_supported()
    }

    /// A page made of `levels` chosen by the caller (a whole-slide image's pyramid, whose
    /// levels the file does not mark as reduced): the first is the full-resolution image; the
    /// others are kept when smaller, decodable and of the same output format, largest first.
    /// `None` without levels.
    pub fn from_levels(mut levels: Vec<Ifd>) -> Option<Page> {
        if levels.is_empty() {
            return None;
        }
        let base = levels.remove(0);
        let (fmt, wide, w, h) = (
            base.output_format(),
            base.wide_format(),
            base.width,
            base.height,
        );
        levels.retain(|l| {
            l.width < w
                && l.height < h
                && l.output_format() == fmt
                && l.wide_format() == wide
                && l.check_supported().is_ok()
        });
        levels.sort_by_key(|l| std::cmp::Reverse(l.width));
        levels.dedup_by_key(|l| l.width);
        levels.insert(0, base);
        Some(Page { levels })
    }

    /// The stored levels, full resolution first.
    pub fn levels(&self) -> &[Ifd] {
        &self.levels
    }

    /// Decode (part of) the page at a scale, like [`Tiff::decode`]; `little_endian` is the
    /// file's byte order.
    pub fn decode(
        &self,
        data: &[u8],
        little_endian: bool,
        opts: &DecodeOptions,
    ) -> Result<Decoded> {
        decode::decode_page(self, data, little_endian, opts)
    }

    /// Estimated peak heap memory of [`Page::decode`].
    pub fn memory_estimate(&self, opts: &DecodeOptions) -> u64 {
        decode::memory_estimate(self, opts)
    }

    /// Decode (part of) one band of the page as values, like [`Tiff::decode_values`];
    /// `little_endian` is the file's byte order.
    pub fn decode_values(
        &self,
        data: &[u8],
        little_endian: bool,
        band: usize,
        nodata: Option<f64>,
        opts: &DecodeOptions,
    ) -> Result<Decoded> {
        if self.ifd().compression.is_jpeg() {
            return Err(Error::Unsupported("values of JPEG-compressed data".into()));
        }
        if band >= self.ifd().samples as usize {
            return Err(Error::Unsupported(format!("band {band}")));
        }
        decode::decode_page_values(self, data, little_endian, band, nodata, opts)
    }
}

/// Where a tag's value lies, as the IFD entry states it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TagEntry {
    /// The TIFF field type (1 BYTE, 2 ASCII, 3 SHORT, 4 LONG, 5 RATIONAL…).
    pub typ: u16,
    pub count: u64,
    /// File offset of the value (inline values point into the entry itself).
    pub offset: u64,
}

/// One IFD as stored, for readers of conventions built on TIFF (whole-slide images, OME-TIFF)
/// that group the directories themselves.
#[derive(Clone, Debug)]
pub struct Directory {
    /// File offset of the IFD.
    pub offset: u64,
    /// For a SubIFD: the index (in [`Directories::dirs`]) of the main-chain IFD that lists it.
    pub parent: Option<usize>,
    /// The raster, when the IFD describes one this crate can parse.
    pub ifd: Option<Ifd>,
    entries: Vec<ifd::Entry>,
}

/// Every IFD of a file: the main chain in order, each followed by its SubIFDs.
#[derive(Clone, Debug)]
pub struct Directories {
    pub big_tiff: bool,
    pub little_endian: bool,
    /// The file uses NDPI's 64-bit offset extension.
    pub ndpi_offsets: bool,
    pub dirs: Vec<Directory>,
}

/// Upper bound on SubIFDs read per main-chain IFD.
const MAX_SUB_IFDS: usize = 64;

impl Directories {
    /// Parse the header and list every IFD (main chain and SubIFDs) without grouping them.
    /// Fails only when the first IFD cannot be read.
    pub fn read(data: &[u8]) -> Result<Directories> {
        Self::read_limited(data, usize::MAX)
    }

    /// Like [`Directories::read`], stopping after `max_main` main-chain IFDs (a cheap look at
    /// the first ones).
    pub fn read_limited(data: &[u8], max_main: usize) -> Result<Directories> {
        let (r, first) = ifd::Reader::new(data)?;
        let mut visited = std::collections::HashSet::new();
        let mut dirs: Vec<Directory> = Vec::new();
        let mut main = Vec::new();
        ifd::walk_chain(&r, first, &mut visited, |off, entries| {
            if main.len() >= max_main {
                return Err(Error::Cancelled);
            }
            main.push((off, entries.to_vec()));
            Ok(())
        })
        .or_else(|e| match e {
            Error::Cancelled if !main.is_empty() => Ok(()),
            e => Err(e),
        })?;
        for (off, entries) in main {
            let ifd = Ifd::parse(&r, &entries).ok();
            let subs = match ifd::find(&entries, 330) {
                Some(e) => r.uints(e).unwrap_or_default(),
                None => Vec::new(),
            };
            let parent = dirs.len();
            dirs.push(Directory {
                offset: off,
                parent: None,
                ifd,
                entries,
            });
            for sub in subs.into_iter().take(MAX_SUB_IFDS) {
                if visited.len() >= 65_536 || !visited.insert(sub) {
                    continue;
                }
                if let Ok((entries, _)) = r.ifd(sub) {
                    dirs.push(Directory {
                        offset: sub,
                        parent: Some(parent),
                        ifd: Ifd::parse(&r, &entries).ok(),
                        entries,
                    });
                }
            }
        }
        Ok(Directories {
            big_tiff: r.big,
            little_endian: r.le,
            ndpi_offsets: r.ndpi,
            dirs,
        })
    }

    /// Indices (in [`Directories::dirs`]) of the main-chain IFDs, in chain order.
    pub fn main_chain(&self) -> Vec<usize> {
        (0..self.dirs.len())
            .filter(|&i| self.dirs[i].parent.is_none())
            .collect()
    }

    /// The parsed SubIFDs of directory `d`, in the order listed.
    pub fn sub_ifds(&self, d: usize) -> impl Iterator<Item = &Ifd> + '_ {
        self.dirs
            .iter()
            .filter(move |x| x.parent == Some(d))
            .filter_map(|x| x.ifd.as_ref())
    }

    fn reader<'a>(&self, data: &'a [u8]) -> ifd::Reader<'a> {
        ifd::Reader {
            data,
            le: self.little_endian,
            big: self.big_tiff,
            ndpi: self.ndpi_offsets,
            spent: std::cell::Cell::new(0),
        }
    }

    /// Whether directory `d` has `tag`.
    pub fn has(&self, d: usize, tag: u16) -> bool {
        self.dirs
            .get(d)
            .is_some_and(|d| ifd::find(&d.entries, tag).is_some())
    }

    /// An ASCII tag's text (up to the first NUL, invalid UTF-8 replaced).
    pub fn ascii(&self, data: &[u8], d: usize, tag: u16) -> Option<String> {
        let b = self.bytes(data, d, tag)?;
        let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
        Some(String::from_utf8_lossy(&b[..end]).into_owned())
    }

    /// A tag's value bytes as stored.
    pub fn bytes<'a>(&self, data: &'a [u8], d: usize, tag: u16) -> Option<&'a [u8]> {
        let e = ifd::find(&self.dirs.get(d)?.entries, tag)?;
        let r = self.reader(data);
        r.range(e).ok().and_then(|rg| data.get(rg))
    }

    /// An integer tag's values.
    pub fn uints(&self, data: &[u8], d: usize, tag: u16) -> Option<Vec<u64>> {
        let e = ifd::find(&self.dirs.get(d)?.entries, tag)?;
        self.reader(data).uints(e).ok()
    }

    /// A rational (or integer) tag's values.
    pub fn reals(&self, data: &[u8], d: usize, tag: u16) -> Option<Vec<f64>> {
        let e = ifd::find(&self.dirs.get(d)?.entries, tag)?;
        self.reader(data).reals(e).ok()
    }

    /// A tag's type, count and value offset as stored, for conventions whose values are not
    /// what the type says (MetaMorph's UIC tags count planes, not values).
    pub fn entry(&self, d: usize, tag: u16) -> Option<TagEntry> {
        let e = ifd::find(&self.dirs.get(d)?.entries, tag)?;
        Some(TagEntry {
            typ: e.typ,
            count: e.count,
            offset: e.pos,
        })
    }

    /// The tags of directory `d`, in ascending order.
    pub fn tags(&self, d: usize) -> impl Iterator<Item = u16> + '_ {
        self.dirs
            .get(d)
            .into_iter()
            .flat_map(|d| d.entries.iter().map(|e| e.tag))
    }

    /// A 32-bit float tag's values (NDPI stores some properties as FLOAT).
    pub fn floats(&self, data: &[u8], d: usize, tag: u16) -> Option<Vec<f64>> {
        let e = ifd::find(&self.dirs.get(d)?.entries, tag)?;
        let r = self.reader(data);
        match e.typ {
            11 => {
                let raw = r.bytes(e.pos, 4 * e.count.min(ifd::MAX_VALUES)).ok()?;
                Some(
                    raw.chunks_exact(4)
                        .map(|c| {
                            let b = [c[0], c[1], c[2], c[3]];
                            f64::from(if r.le {
                                f32::from_le_bytes(b)
                            } else {
                                f32::from_be_bytes(b)
                            })
                        })
                        .collect(),
                )
            }
            12 => {
                let raw = r.bytes(e.pos, 8 * e.count.min(ifd::MAX_VALUES)).ok()?;
                Some(
                    raw.chunks_exact(8)
                        .map(|c| {
                            let mut b = [0u8; 8];
                            b.copy_from_slice(c);
                            if r.le {
                                f64::from_le_bytes(b)
                            } else {
                                f64::from_be_bytes(b)
                            }
                        })
                        .collect(),
                )
            }
            _ => r.reals(e).ok(),
        }
    }
}

/// A parsed TIFF file (headers only; pixel data is read from the caller's buffer on decode).
#[derive(Clone, Debug)]
pub struct Tiff {
    pub big_tiff: bool,
    pub little_endian: bool,
    pages: Vec<Page>,
}

/// Rectangle in output (scaled) pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct DecodeOptions<'a> {
    /// Downscale factor, a power of two (1, 2, 4, 8, ...). The output has
    /// `ceil(width / scale)` × `ceil(height / scale)` pixels.
    pub scale: u32,
    /// Decode only this rectangle of the scaled image (clipped to it).
    pub region: Option<Region>,
    /// Checked before every strip / tile.
    pub cancel: Option<&'a AtomicBool>,
    /// Decode strips / tiles on the rayon pool.
    pub parallel: bool,
    /// Keep integer grey and RGB(A) deeper than 8 bits at 16 bits per sample
    /// ([`Page::wide_format`]) instead of rounding to 8.
    pub wide: bool,
}

impl Default for DecodeOptions<'_> {
    fn default() -> Self {
        DecodeOptions {
            scale: 1,
            region: None,
            cancel: None,
            parallel: true,
            wide: false,
        }
    }
}

/// Decoded pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    /// Top-left of this raster in the scaled image.
    pub origin: (u32, u32),
    pub format: PixelFormat,
    /// Row stride in bytes (`width * bytes_per_pixel`).
    pub stride: usize,
    pub data: Vec<u8>,
    /// Some strip or tile was missing or damaged (shown grey).
    pub truncated: bool,
}

impl Tiff {
    /// Parse the header and every IFD.
    pub fn open(data: &[u8]) -> Result<Tiff> {
        let (r, first) = ifd::Reader::new(data)?;
        let mut visited = std::collections::HashSet::new();
        let mut pages: Vec<Page> = Vec::new();
        let mut first_err = None;
        ifd::walk_chain(&r, first, &mut visited, |_, entries| {
            let im = match Ifd::parse(&r, entries) {
                Ok(im) => im,
                Err(e) => {
                    if pages.is_empty() && first_err.is_none() {
                        first_err = Some(e);
                    }
                    return Ok(());
                }
            };
            if im.subfile_type & 4 != 0 {
                return Ok(()); // transparency mask
            }
            if im.subfile_type & 1 != 0 {
                if let Some(p) = pages.last_mut() {
                    p.levels.push(im);
                }
                return Ok(());
            }
            pages.push(Page { levels: vec![im] });
            Ok(())
        })?;
        if pages.is_empty() {
            return Err(first_err.unwrap_or(Error::Malformed("no image")));
        }
        // SubIFDs of each page: reduced-resolution versions.
        for p in &mut pages {
            let subs = p.levels[0].sub_ifds.clone();
            for off in subs.into_iter().take(64) {
                if !visited.insert(off) {
                    continue;
                }
                if let Ok((entries, _)) = r.ifd(off)
                    && let Ok(im) = Ifd::parse(&r, &entries)
                    && im.subfile_type & 4 == 0
                {
                    p.levels.push(im);
                }
            }
            let base = &p.levels[0];
            let (fmt, wide, w, h) = (
                base.output_format(),
                base.wide_format(),
                base.width,
                base.height,
            );
            let mut levels = std::mem::take(&mut p.levels);
            let base = levels.remove(0);
            levels.retain(|l| {
                l.width < w
                    && l.height < h
                    && l.output_format() == fmt
                    && l.wide_format() == wide
                    && l.check_supported().is_ok()
            });
            levels.sort_by_key(|l| std::cmp::Reverse(l.width));
            levels.dedup_by_key(|l| l.width);
            levels.insert(0, base);
            p.levels = levels;
        }
        Ok(Tiff {
            big_tiff: r.big,
            little_endian: r.le,
            pages,
        })
    }

    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    /// Decode (part of) a page at a scale.
    pub fn decode(&self, data: &[u8], page: usize, opts: &DecodeOptions) -> Result<Decoded> {
        let p = self.pages.get(page).ok_or(Error::NoPage)?;
        decode::decode_page(p, data, self.little_endian, opts)
    }

    /// Decode (part of) one band of a page as values ([`PixelFormat::GrayF32`]): sample
    /// `band` of every pixel as stored, `nodata` and missing data as NaN; reduced scales are
    /// the mean of the finite values. Not for JPEG-compressed pages.
    pub fn decode_values(
        &self,
        data: &[u8],
        page: usize,
        band: usize,
        nodata: Option<f64>,
        opts: &DecodeOptions,
    ) -> Result<Decoded> {
        let p = self.pages.get(page).ok_or(Error::NoPage)?;
        if p.ifd().compression.is_jpeg() {
            return Err(Error::Unsupported("values of JPEG-compressed data".into()));
        }
        if band >= p.ifd().samples as usize {
            return Err(Error::Unsupported(format!("band {band}")));
        }
        decode::decode_page_values(p, data, self.little_endian, band, nodata, opts)
    }

    /// Estimated peak heap memory of [`Tiff::decode`] (output plus working buffers).
    pub fn memory_estimate(&self, page: usize, opts: &DecodeOptions) -> u64 {
        match self.pages.get(page) {
            Some(p) => decode::memory_estimate(p, opts),
            None => 0,
        }
    }
}
