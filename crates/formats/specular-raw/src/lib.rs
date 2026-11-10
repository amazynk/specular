//! `specular-raw`: camera RAW reader for Specular.
//!
//! Two speeds, matching DESIGN.md principle 1 ("first pixel fast, then refine"):
//!
//! 1. **Embedded previews (the viewer fast path).** [`parse`] reads only container headers
//!    (TIFF IFDs, maker notes, ISO-BMFF boxes, the RAF header) and returns every embedded
//!    JPEG/RGB preview with its byte range, pixel size and orientation, plus camera make/model
//!    and exposure basics. No sensor data is touched; a typical call costs microseconds.
//!    The preview JPEGs are ordinary baseline JPEGs that the caller feeds to `specular-jpeg`
//!    (or [`decode_preview`] does it for convenience).
//! 2. **Sensor decode.** [`RawFile::decode_raw`] unpacks the sensor data with our own decoders
//!    (uncompressed/packed, lossless JPEG "LJ92" incl. DNG tiles and Canon CR2 slices, Canon
//!    CRX, Nikon Huffman, Sony ARW2 and lossless (LJ92 tiles), Fujifilm compressed, Olympus, Panasonic, Pentax Huffman)
//!    into a [`RawImage`] (16-bit CFA or linear-raw samples plus black/white levels, white
//!    balance and a camera-to-Rec.709 matrix). [`develop`] turns that into a linear 16-bit
//!    Rec.709 [`RgbImage`] in parallel row bands: Bayer at full size (Malvar-He-Cutler
//!    gradient-corrected demosaic) or half size (2x2 binning, the fast path), X-Trans and other
//!    3x3 RGB patterns through their own develop path (gradient-weighted green, colour
//!    difference for red and blue; 3x3 means at half size).
//!
//! # Supported formats
//! | Format | Previews + metadata | Sensor decode |
//! |---|---|---|
//! | DNG (incl. phones) | yes | uncompressed (8-16 bit, packed), LJ92 strips/tiles, VC-5 (GoPro GPR, one or several tiles); Bayer, X-Trans (6x6, Adobe DNG Converter) and other 3x3 RGB CFAs, LinearRaw (3 samples, or 1: monochrome, shown grey) |
//! | Canon CR2 | yes | LJ92 with slices |
//! | Canon CR3 | yes (THMB, PRVW, full JPEG track) | CRX lossless ("RAW") and lossy wavelet ("C-RAW") |
//! | Canon Cinema RAW Light (`.crm`, video) | yes (THMB, CMT1-4) | every frame, CRX ([`RawFile::frame`]) |
//! | Apple ProRes RAW (QuickTime, video) | movie `meta` keys | every frame, through `specular-prores` |
//! | Nikon NEF/NRW | yes | uncompressed, lossless/lossy Huffman, High Efficiency HE/HE* (intoPIX TicoRAW, own decoder over the JPEG XS crate) |
//! | Sony ARW/SR2 | yes | uncompressed, ARW2 compressed, lossless compressed (LJ92 tiles, tile-parallel; L, M and S sizes), ARW 1.0 |
//! | Fujifilm RAF | yes | uncompressed (16-bit or bit-packed), compressed lossless and lossy; X-Trans and Bayer |
//! | Olympus ORF | yes | uncompressed (12/16-bit), Olympus compressed (12-bit, one strip) |
//! | Panasonic RW2 / Leica RWL | yes | raw formats 1-4 (Panasonic bit stream), 5 and 6 (12/14-bit blocks), 7 (blocks per row) and 8 (strip-parallel prefix code) |
//! | Pentax PEF | yes | uncompressed, Pentax Huffman |
//! | Samsung SRW | yes | uncompressed, compressed versions 1, 2 and 3 |
//! | Hasselblad 3FR/FFF | yes | Hasselblad lossless (`.fff` thermal files from FLIR are another format) |
//! | Phase One IIQ | yes | IIQ L, IIQ S, uncompressed; IIQ L and S with the back's calibration: black per row and column, defects, flat fields, curves (`phase_one_cal`) |
//! | Leaf MOS, Mamiya MEF | yes | uncompressed (TIFF strips or tiles) |
//! | Kodak DCR/KDC/DCS | yes | uncompressed, compression 65000, DC120 (plain and JPEG), RADC (DC40, DC50) |
//! | Epson ERF | yes | uncompressed, 12-bit packed |
//! | Minolta MRW | yes | packed and unpacked 12-bit |
//! | Canon CRW (CIFF) | yes | Canon CRW lossless (10 and 12 bit), plain 10-bit (Pro70); CMYG filters (Pro70, Pro90 IS, G1) |
//! | Sigma X3F (Foveon) | yes | TRUE (DP1-DP3, SD14/SD15), Merrill, Quattro (with AF-pixel correction), Huffman 10-bit (SD9/SD10); three samples per pixel, colour and lens shading (not Quattro) from the file's own calibration; SD9/SD10 thumbnails (plain and Huffman-coded) |
//! | ARRI ARI | metadata (no previews) | 12-bit Bayer frames; colour from ARRI's ACES input transforms |
//! | Sony SRF (DSC-F828) | yes | encrypted 14-bit red, green, blue and emerald photosites |
//! | Other TIFF-based | where the TIFF exposes them | uncompressed 2x2 Bayer only |
//!
//! Also decoded: DNG lossy JPEG, JPEG XL and deflate (integer and floating point) blocks, DNG
//! dual- and multi-pixel data (several samples per photosite, added up), four-colour (CYGM,
//! RGBE) and other RGB patterns the develop cannot demosaic and staggered DNG layouts
//! (demosaiced to camera RGB on decode), Canon CR2
//! sRAW/mRAW, Sony ARW1 and lossless M/S sizes (YCbCr tiles), Nikon Coolpix NEFs (rows stored
//! as two fields) and Fujifilm SuperCCD (turned upright to camera RGB on decode; on SuperCCD SR
//! sensors the high-sensitivity S photosites, as the camera's JPEG).
//!
//! DNG CFA data as the DNG specification (1.7) defines it beyond the rectangular RGB mosaic
//! is decoded too: staggered `CFALayout`s 2-9 (each colour estimated at the photosite's true
//! position by a local plane fit, on the stored grid), colour planes other than red, green
//! and blue (four planes converted by the file's own 4 x 3 `ColorMatrix` and four-value
//! `AsShotNeutral` when present, else by ideal filter responses), and up to 16 samples per
//! photosite (added up). `DefaultScale` (non-square pixels) stretches the whole picture
//! [`RawFile::decode`] gives to square pixels; [`develop`] and regions keep the stored grid.
//!
//! Not supported for sensor decode (previews and metadata still work where the container is
//! read), each refused by name:
//! - Canon CR3 sRAW/mRAW: no CR3 body writes them (C-RAW replaced them); a CRX track with an
//!   unknown plane layout is refused. Roll-burst frames (encoding 3, four planes) take the
//!   CRX path, unchecked: no sample file is public.
//! - The CRX signed-sample variant (encoding 1): no camera file and no description of where
//!   black lies.
//! - Sony SRF from bodies other than the DSC-F828 (the DSC-V3): no sample file to check a
//!   decoder against.
//! - DNG CFA data whose pattern points outside `CFAPlaneColor`, whose colours cannot give red,
//!   green and blue, with a `CFALayout` outside 1-9, or with more than 16 samples per pixel.
//!
//! [`SensorInfo::unsupported_reason`] names the reason for a given file. RED R3D and
//! Blackmagic RAW video are refused by [`video_raw_refusal`].
//!
//! # Colour
//! [`develop`] outputs **linear** light with Rec.709/sRGB primaries (D65), 0..=65535 = 0..1,
//! after black subtraction, white-level scaling, white balance and highlight clipping. DNG uses
//! `ForwardMatrix` (preferred) or `ColorMatrix` interpolated by the as-shot colour temperature;
//! other cameras use, in order, the Adobe DNG Converter `ColorMatrix` for the model (generated
//! table, `tools/gen-adobe-coeff.py`), a published third-party table for bodies Adobe's lacks
//! (`tools/gen-extra-coeff.py`: LibRaw's numbers, ARRI's ACES transforms), the matrix embedded
//! in the file (Sony; Phase One's camera-to-ROMM matrix), or a generic matrix (see
//! [`MatrixSource`]). Sigma X3F files carry their own calibration (white-balance gains and
//! colour corrections per preset, or a camera-to-XYZ matrix), used instead. Four-colour
//! sensors (CMYG, RGBE) are converted by their published 4 x 3 matrix on decode.
//!
//! # Robustness
//! Corrupt or truncated files never panic. Sensor buffers are only allocated after the
//! dimensions pass [`DecodeOptions::max_pixels`] and a plausibility check against the file
//! size; missing data is filled with zeros and flagged in [`RawImage::truncated`].

mod adobe_coeff;
mod ari;
mod bits;
mod bmff;
mod cfa;
mod cfagen;
mod color;
mod container;
mod crw;
mod crx;
pub mod dcp;
mod decode;
mod develop;
mod dngtile;
mod error;
mod extra_coeff;
mod fuji;
mod hasselblad;
mod huffman;
mod kodak;
mod leaf;
mod lj92tile;
pub mod ljpeg;
mod makernote;
mod nikon_he;
mod olympus;
mod opcode;
mod pana;
mod pana8;
mod phase_one;
mod phase_one_cal;
mod profile;
mod radc;
mod rd175;
mod region;
mod samsung;
mod scale;
pub mod sensor;
mod srf;
mod superccd;
mod tiff;
mod vc5;
mod vendor;
mod x3f;
mod xtrans;
mod ycc;

use std::ops::Range;
use std::sync::atomic::AtomicBool;

pub use cfa::Cfa;
pub use color::{as_shot_xy, camera_key};
pub use dcp::{Dcp, DcpError};
pub use develop::{DevelopStats, develop, develop_with};
pub use error::{RawError, Result};
pub use opcode::DngOpcodes;
pub use profile::ProfileTransform;
pub use region::TileCache;
pub use sensor::{SensorData, SensorMeta};

/// RAW container family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RawFormat {
    Dng,
    Cr2,
    Cr3,
    Nef,
    Nrw,
    Arw,
    Sr2,
    Raf,
    Orf,
    Rw2,
    Pef,
    /// Samsung SRW.
    Srw,
    /// Hasselblad 3FR and FFF.
    Hasselblad,
    /// Phase One IIQ.
    Iiq,
    /// Leaf MOS.
    Mos,
    /// Mamiya MEF.
    Mef,
    /// Kodak DCR, KDC and DCS (TIFF-based).
    Kodak,
    /// Epson ERF.
    Erf,
    /// Minolta MRW.
    Mrw,
    /// Canon CRW (CIFF).
    Crw,
    /// Sigma X3F (Foveon).
    X3f,
    /// ARRI ARI.
    Ari,
    /// Sony SRF (DSC-F828).
    Srf,
    /// Sinarback STI (TIFF-based).
    Sinar,
    /// Apple QuickTake 100, 150 and 200 (`.qtk`).
    QuickTake,
    /// Minolta RD-175 (`.mdc`): three 8-bit CCDs.
    Rd175,
    /// Lytro light-field pictures (`.lfp`, `.lfr`).
    Lytro,
    /// Ricoh J6I: a JPEG behind the camera's header.
    J6i,
    /// A headerless sensor dump of a known size (old compacts, action cameras).
    Headerless,
    /// Another TIFF-based RAW.
    TiffRaw,
    /// Canon Cinema RAW Light video (`.crm`): CRX frames in the CR3 container.
    Crm,
    /// Apple ProRes RAW video (QuickTime `aprn`, `aprh`).
    ProResRaw,
}

impl RawFormat {
    pub fn name(self) -> &'static str {
        match self {
            RawFormat::Dng => "DNG",
            RawFormat::Cr2 => "Canon CR2",
            RawFormat::Cr3 => "Canon CR3",
            RawFormat::Crm => "Canon Cinema RAW Light",
            RawFormat::ProResRaw => "Apple ProRes RAW",
            RawFormat::Nef => "Nikon NEF",
            RawFormat::Nrw => "Nikon NRW",
            RawFormat::Arw => "Sony ARW",
            RawFormat::Sr2 => "Sony SR2",
            RawFormat::Raf => "Fujifilm RAF",
            RawFormat::Orf => "Olympus ORF",
            RawFormat::Rw2 => "Panasonic RW2",
            RawFormat::Pef => "Pentax PEF",
            RawFormat::Srw => "Samsung SRW",
            RawFormat::Hasselblad => "Hasselblad 3FR/FFF",
            RawFormat::Iiq => "Phase One IIQ",
            RawFormat::Mos => "Leaf MOS",
            RawFormat::Mef => "Mamiya MEF",
            RawFormat::Kodak => "Kodak DCR/KDC",
            RawFormat::Erf => "Epson ERF",
            RawFormat::Mrw => "Minolta MRW",
            RawFormat::Crw => "Canon CRW",
            RawFormat::X3f => "Sigma X3F",
            RawFormat::Ari => "ARRI ARI",
            RawFormat::Srf => "Sony SRF",
            RawFormat::Sinar => "Sinar STI",
            RawFormat::QuickTake => "Apple QuickTake",
            RawFormat::Rd175 => "Minolta RD-175 MDC",
            RawFormat::Lytro => "Lytro light field",
            RawFormat::J6i => "Ricoh J6I",
            RawFormat::Headerless => "headerless camera RAW",
            RawFormat::TiffRaw => "TIFF-based RAW",
        }
    }
}

/// How an embedded preview is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PreviewKind {
    /// A complete baseline/progressive JPEG stream (starts with SOI).
    Jpeg,
    /// Uncompressed interleaved 8-bit RGB, rows `range.len() / height` bytes apart (at least
    /// `width * 3`; any padding follows each row).
    Rgb8,
    /// Sigma X3F Huffman-coded 8-bit RGB (format 11 of SD9/SD10 files), decoded by
    /// [`decode_preview`].
    X3fCoded,
    /// 4-bit grey, two pixels a byte (high nibble first), rows packed (QuickTake 100).
    Grey4,
}

/// An embedded preview image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preview {
    pub kind: PreviewKind,
    /// Byte range inside the file.
    pub range: Range<usize>,
    /// Stored size (before orientation).
    pub width: u32,
    pub height: u32,
    /// EXIF orientation (1..=8) to apply when displaying it.
    pub orientation: u16,
    /// Where it was found, e.g. `"IFD0 JPEG"`, `"MakerNote PreviewImage"`, `"CR3 PRVW"`.
    pub source: &'static str,
}

impl Preview {
    pub fn pixels(&self) -> u64 {
        self.width as u64 * self.height as u64
    }
}

/// Rectangle in sensor sample coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// Sensor data compression.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SensorCompression {
    Uncompressed,
    LosslessJpeg,
    NikonHuffman,
    SonyArw2,
    Panasonic,
    PentaxHuffman,
    CanonCrx,
    /// Apple ProRes RAW.
    ProResRaw,
    Fujifilm,
    Olympus,
    LossyJpeg,
    /// Nikon High Efficiency (intoPIX TicoRAW); `star` for HE* (the higher quality).
    NikonHighEfficiency {
        star: bool,
    },
    Other(u32),
}

/// Facts about the sensor data, from headers only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SensorInfo {
    /// Stored raw size in pixels (including masked borders).
    pub width: u32,
    pub height: u32,
    /// Visible area that [`develop`] outputs.
    pub crop: Rect,
    /// Samples per pixel: 1 for CFA data, 3 for DNG LinearRaw.
    pub samples_per_pixel: u32,
    pub bits: u32,
    pub compression: SensorCompression,
    pub cfa: Option<Cfa>,
    /// [`RawFile::decode_raw`] supports this layout.
    pub decodable: bool,
    /// Why not, if `!decodable`.
    pub unsupported_reason: Option<&'static str>,
    /// DNG opcodes the decode applies (`OpcodeList1..3`, names in list order, each once).
    pub opcodes: Vec<&'static str>,
    /// DNG opcodes without the "optional" flag that the decode does not apply (unknown,
    /// unsupported at their stage or malformed); the image may then differ from the intended
    /// rendering.
    pub opcodes_skipped: Vec<&'static str>,
    /// The crop's size with square pixels: `crop` stretched by DNG `DefaultScale` along the
    /// axis it makes longer (never shrunk), else `crop`'s own size. [`RawFile::decode`]
    /// outputs this; [`develop`] keeps the stored grid ([`resample_rgb`] resamples it).
    pub square_size: (u32, u32),
    /// The decode turns the samples into camera RGB itself (four-colour and other patterns
    /// the develop cannot demosaic, staggered layouts): [`RawFile::decode_raw`] and
    /// [`RawFile::decode_sensor`] give three samples per pixel and no CFA, as LinearRaw.
    pub demosaiced: bool,
}

/// Header-level facts of a RAW file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawInfo {
    pub format: Option<RawFormat>,
    pub make: String,
    pub model: String,
    /// EXIF orientation 1..=8 of the raw data (1 if absent).
    pub orientation: u16,
    pub iso: Option<u32>,
    /// Seconds.
    pub exposure_time: Option<f64>,
    pub f_number: Option<f64>,
    /// Millimetres.
    pub focal_length: Option<f64>,
    /// EV.
    pub exposure_bias: Option<f64>,
    /// `YYYY:MM:DD HH:MM:SS` as stored.
    pub date_time: Option<String>,
    pub lens: Option<String>,
    pub software: Option<String>,
    /// DNG `BaselineExposure` (EV) â a renderer may want to apply it.
    pub baseline_exposure: Option<f64>,
    /// All embedded previews, largest first.
    pub previews: Vec<Preview>,
    pub sensor: Option<SensorInfo>,
    /// TIFF-structured metadata blocks (each starting with a TIFF header) for a full metadata
    /// view: the whole file for TIFF-based RAWs, the CMT1..CMT4 boxes for CR3, the embedded
    /// JPEG's Exif block for RAF.
    pub tiff_blocks: Vec<Range<usize>>,
    /// A raw video clip's frames and facts (Canon Cinema RAW Light, Apple ProRes RAW); the
    /// sensor is the first frame's ([`RawFile::frame`] gives the others).
    pub clip: Option<Clip>,
}

/// A raw video clip: every frame is a sensor image of the same layout.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clip {
    /// The coding ("Canon Cinema RAW Light", "Apple ProRes RAW HQ").
    pub codec: String,
    /// Each frame's bytes, in display order: the frames wholly in the file.
    pub frames: Vec<Range<usize>>,
    /// Frames the clip's sample table lists (more than `frames` when the file was cut short).
    pub listed: u32,
    /// Frames per second (over the whole track).
    pub frame_rate: Option<f64>,
    pub duration_us: Option<u64>,
    /// The sound, described ("PCM, 4 × 1 ch, 48 kHz"); not played.
    pub audio: Option<String>,
    /// Further facts the clip states, by name ("white balance", "shutter angle", "gain",
    /// "encoder", "coding"), as written.
    pub facts: Vec<(&'static str, String)>,
}

impl RawInfo {
    /// The largest embedded preview.
    pub fn largest_preview(&self) -> Option<&Preview> {
        self.previews.first()
    }
    /// The smallest embedded preview (EXIF-style thumbnail).
    pub fn thumbnail(&self) -> Option<&Preview> {
        self.previews.last()
    }
    /// The smallest preview whose longer side covers `min_long_side` pixels, else the largest.
    /// JPEG previews are preferred over RGB ones of similar size.
    pub fn preview_for(&self, min_long_side: u32) -> Option<&Preview> {
        self.previews
            .iter()
            .rev()
            .find(|p| p.width.max(p.height) >= min_long_side)
            .or(self.previews.first())
    }
    /// "Make Model" without repeating the make (e.g. "Canon EOS 5D" not "Canon Canon EOS 5D").
    pub fn camera_name(&self) -> String {
        let make = self.make.split_whitespace().next().unwrap_or("");
        if make.is_empty() || self.model.to_lowercase().starts_with(&make.to_lowercase()) {
            self.model.clone()
        } else {
            format!("{} {}", make, self.model).trim().to_string()
        }
    }
}

/// The DNG colour tags a converter would write for a camera without DNG colour data of its
/// own: the built-in profile of the body (ColorMatrix1/2, ForwardMatrix1/2 and their
/// illuminants), else the Adobe table's ColorMatrix (D65, illuminant 21), else the published
/// table's. `None` for bodies in neither (their colour then comes from the file alone). For
/// writing derived DNGs, such as an HDR merge, that keep the camera's colour.
pub fn dng_colour_for(make: &str, model: &str) -> Option<sensor::DngColour> {
    if let Some(p) = color::camera_profile(make, model) {
        return Some(sensor::DngColour {
            color_matrix: p.cm,
            forward_matrix: p.fm,
            illuminants: p.illuminant,
            ..Default::default()
        });
    }
    let e = color::adobe_entry(make, model).or_else(|| color::extra_entry(make, model))?;
    let m = e.matrix?;
    Some(sensor::DngColour {
        color_matrix: [
            Some(std::array::from_fn(|i| f64::from(m[i]) / 10000.0)),
            None,
        ],
        illuminants: [21, 0],
        ..Default::default()
    })
}

/// Where the camera-to-RGB matrix of a [`RawImage`] came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MatrixSource {
    /// DNG `ForwardMatrix` (with D50 â D65 Bradford adaptation).
    DngForwardMatrix,
    /// DNG `ColorMatrix` (inverted, white-preserving).
    DngColorMatrix,
    /// DNG camera profile with its look tables: `ForwardMatrix` (or `ColorMatrix`) followed by
    /// `ProfileHueSatMap` and `ProfileLookTable` in ProPhoto RGB, see [`RawImage::profile`].
    DngProfile,
    /// Adobe DNG Converter colour data for this camera model (built-in, see `adobe_coeff.rs`
    /// for provenance): the dcraw-era ColorMatrix table, or the profile of a DNG converted from
    /// a raw file of the same body.
    Adobe,
    /// Adobe DNG Converter ColorMatrix of another body with the same sensor (built-in list of
    /// siblings, see `adobe_coeff.rs`).
    AdobeSibling,
    /// Built-in DNG colour profile (ColorMatrix1/2, ForwardMatrix1/2) for this camera model,
    /// read from a camera-written DNG of the same body (see `adobe_coeff.rs` for provenance).
    CameraProfile,
    /// Colour calibration the camera stores in the file: Sigma X3F white-balance gains and
    /// colour-correction (or camera-to-XYZ) matrices.
    CameraCalibration,
    /// A published third-party table for bodies without Adobe colour data (see
    /// `extra_coeff.rs` for provenance): LibRaw's colour data (numbers only), ARRI's ACES input
    /// transforms for ALEXA.
    Published,
    /// A generic average-camera matrix (no colour data for this camera).
    Generic,
    /// Identity (LinearRaw without colour data, monochrome).
    Identity,
}

/// Where the white balance came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WbSource {
    /// As-shot multipliers stored in the file.
    AsShot,
    /// Estimated by [`develop`] (grey world), because the file has none.
    Estimated,
}

/// Decoded sensor data, ready for [`develop`].
#[derive(Clone, Debug)]
pub struct RawImage {
    pub width: u32,
    pub height: u32,
    /// Samples per pixel (1 = CFA, 3 = linear RGB).
    pub cpp: u32,
    /// `window.w * window.h * cpp` samples, row-major, as stored (before linearization).
    pub data: Vec<u16>,
    /// The part of the sensor `data` holds: all of it (`0, 0, width, height`) unless a
    /// region was decoded (see [`DecodeOptions::region`]).
    pub window: Rect,
    pub cfa: Option<Cfa>,
    /// Visible area.
    pub crop: Rect,
    /// Black level per 2x2 position `(y & 1) * 2 + (x & 1)` (CFA, raw coordinates) or per
    /// channel (cpp = 3), in linearized units.
    pub black: [f32; 4],
    /// White (saturation) level in linearized units.
    pub white: f32,
    /// Maps stored values to linear values (DNG `LinearizationTable`).
    pub linearization: Option<Vec<u16>>,
    /// White balance multipliers for R, G, B (G = 1); `None` = unknown, estimated by [`develop`].
    pub wb: Option<[f32; 3]>,
    /// White-balanced camera RGB â linear Rec.709, rows sum to 1. With a [`Self::profile`] this
    /// is the profile's matrix alone (an approximation [`develop`] does not use).
    pub rgb_cam: [[f32; 3]; 3],
    pub matrix_source: MatrixSource,
    /// DNG camera profile with look tables, applied by [`develop`] instead of `rgb_cam`.
    pub profile: Option<std::sync::Arc<ProfileTransform>>,
    /// EXIF orientation of the image.
    pub orientation: u16,
    /// Some sensor data was missing or corrupt (filled with zeros).
    pub truncated: bool,
    /// DNG opcode lists. [`RawFile::decode_raw`] has applied `OpcodeList1` and `OpcodeList2`
    /// to `data` already (black, white and linearization then describe the corrected,
    /// normalised samples); [`develop`] applies `OpcodeList3` after the demosaic.
    pub opcodes: Option<std::sync::Arc<DngOpcodes>>,
}

/// Output size of [`develop`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum OutputSize {
    /// One output pixel per sensor pixel (demosaiced).
    #[default]
    Full,
    /// One output pixel per 2x2 sensor block (no interpolation; ~4-8x faster).
    Half,
}

/// Options for [`RawFile::decode_raw`], [`RawFile::decode`] and [`develop`].
#[derive(Clone, Copy, Debug)]
pub struct DecodeOptions<'a> {
    pub size: OutputSize,
    /// Use the global rayon pool (tiles, row bands). `false` decodes on the calling thread.
    pub parallel: bool,
    /// Checked between tiles / row bands.
    pub cancel: Option<&'a AtomicBool>,
    /// Refuse sensors with more samples than this (default 1 G samples).
    pub max_pixels: u64,
    /// Only this part of the output (pixels of the developed image at `size`, relative to
    /// the crop). [`develop`] outputs exactly this region, equal to the same pixels of the
    /// whole develop. [`RawFile::decode_raw`] then decodes only the sensor tiles the region
    /// needs when [`RawFile::region_decodable`], else the whole sensor.
    pub region: Option<Rect>,
    /// Decoded sensor tiles reused between region decodes of the same file.
    pub tiles: Option<&'a TileCache>,
}

impl Default for DecodeOptions<'_> {
    fn default() -> Self {
        DecodeOptions {
            size: OutputSize::Full,
            parallel: true,
            cancel: None,
            max_pixels: 1 << 30,
            region: None,
            tiles: None,
        }
    }
}

/// Developed image: interleaved linear RGB, Rec.709 primaries, 0..=65535.
#[derive(Clone, Debug)]
pub struct RgbImage {
    pub width: u32,
    pub height: u32,
    /// `width * height * 3` samples.
    pub data: Vec<u16>,
    /// EXIF orientation to apply for display.
    pub orientation: u16,
    pub matrix_source: MatrixSource,
    pub wb_source: WbSource,
    pub truncated: bool,
}

/// A parsed RAW file: header facts plus the private sensor layout. Holds no reference to the
/// file data; pass the same bytes to the decode methods.
#[derive(Clone, Debug)]
pub struct RawFile {
    info: RawInfo,
    sensor: Option<container::Sensor>,
}

/// Cheap magic-number check (reads at most the first IFD for TIFF-based files).
pub fn detect(data: &[u8]) -> Option<RawFormat> {
    container::detect(data)
}

/// [`detect`] for a file whose extension (any case, without the dot) is known: also the
/// formats recognised by name and content together (headerless sensor dumps of a known size).
pub fn detect_named(data: &[u8], extension: &str) -> Option<RawFormat> {
    container::detect_named(data, extension)
}

/// Whether a `.raw` file (`extension` any case) is a headerless camera sensor dump this crate
/// reads: exact size of a known layout and samples that fit it. A content check of its own,
/// so a registry choosing among the decoders that claim `.raw` can ask it.
pub fn is_headerless_camera_raw(data: &[u8], extension: &str) -> bool {
    container::is_headerless_camera_raw(data, extension)
}

/// [`parse`] for a file whose extension (any case, without the dot) is known: also the
/// formats [`detect_named`] recognises by name.
pub fn parse_named(data: &[u8], extension: &str) -> Result<RawFile> {
    let (info, mut sensor) = container::parse_named(data, extension)?;
    if let Some(s) = sensor.as_mut() {
        color::apply_adobe(&info.make, &info.model, s);
    }
    Ok(RawFile { info, sensor })
}

/// Why a RAW is refused, in plain words: RED R3D (by its `RED1`/`RED2` signature or the
/// `.r3d` extension) and Blackmagic RAW (`.braw`, a QuickTime file), whose codecs are only
/// published as their makers' SDKs; Rawzor (`.rwz`), whose compression is not published
/// (Rawzor by its signature; other `.rwz` files are Outlook rules, not pictures).
/// `extension` is the file's, any case.
pub fn video_raw_refusal(data: &[u8], extension: &str) -> Option<&'static str> {
    let red = matches!(data.get(4..8), Some(b"RED1" | b"RED2"));
    if red || extension.eq_ignore_ascii_case("r3d") {
        return Some(
            "RED R3D video can’t be opened: RED’s licence lets only programs built with its SDK, \
             under RED’s own terms, read it",
        );
    }
    if extension.eq_ignore_ascii_case("braw") {
        return Some(
            "Blackmagic RAW video is not supported: Blackmagic publishes its codec only as an SDK",
        );
    }
    // Rawzor files start with "rawzor" (ExifTool's Rawzor page). Outlook's Rules Wizard
    // exports share `.rwz`; they are told apart by that signature.
    if data.starts_with(b"rawzor") {
        return Some(
            "Rawzor files are not supported: the Rawzor compression has no public description. \
             Rawzor's own tool restores the original RAW file",
        );
    }
    if extension.eq_ignore_ascii_case("rwz") {
        return Some(
            "This .rwz file is not a picture: it has no Rawzor signature. \
             Outlook also uses .rwz for its saved mail rules",
        );
    }
    None
}

/// Parse container headers: previews, metadata and sensor layout. Never decodes pixel data.
pub fn parse(data: &[u8]) -> Result<RawFile> {
    let (info, mut sensor) = container::parse(data)?;
    if let Some(s) = sensor.as_mut() {
        color::apply_adobe(&info.make, &info.model, s);
    }
    Ok(RawFile { info, sensor })
}

impl RawFile {
    pub fn info(&self) -> &RawInfo {
        &self.info
    }

    /// The same file developed with white-balance multipliers `wb` (R, G, B; G 1) instead of
    /// what it states or the develop's estimate: a clip's frames share one, so the picture
    /// does not flicker.
    pub fn with_white_balance(&self, wb: [f32; 3]) -> RawFile {
        let mut f = self.clone();
        if let Some(s) = f.sensor.as_mut()
            && wb.iter().all(|v| v.is_finite() && *v > 0.0)
        {
            s.wb = Some(wb);
        }
        f
    }

    /// Frame `index` of a raw video clip ([`RawInfo::clip`]): the same file with that
    /// frame's sensor data. `None` for a still or past the last frame present.
    pub fn frame(&self, index: usize) -> Option<RawFile> {
        let range = self.info.clip.as_ref()?.frames.get(index)?.clone();
        let mut f = self.clone();
        match &mut f.sensor.as_mut()?.storage {
            container::Storage::Crx { range: r, .. }
            | container::Storage::ProResRaw { range: r } => {
                *r = range;
            }
            _ => return None,
        }
        Some(f)
    }
    pub fn into_info(self) -> RawInfo {
        self.info
    }
    /// The sensor data can be decoded by [`RawFile::decode_raw`].
    pub fn can_decode_sensor(&self) -> bool {
        self.info.sensor.as_ref().is_some_and(|s| s.decodable)
    }
    /// Whether [`RawFile::decode_raw`] with a [`DecodeOptions::region`] decodes only the
    /// sensor tiles or rows behind it: tiled or striped sensor data (lossless ARW, DNG,
    /// uncompressed TIFF-based raws) with a 2x2 pattern or linear samples, and black, white
    /// and white balance known without looking at the data.
    pub fn region_decodable(&self) -> bool {
        self.sensor
            .as_ref()
            .is_some_and(|s| s.unsupported.is_none() && decode::region_ok(s))
    }
    /// Estimated peak bytes of [`RawFile::decode`] for output `region` at `size` when
    /// [`RawFile::region_decodable`]: the developed region, the sensor window behind it and
    /// the tiles it touches. `None` otherwise.
    pub fn region_memory(&self, region: Rect, size: OutputSize) -> Option<u64> {
        let s = self.sensor.as_ref()?;
        if !(s.unsupported.is_none() && decode::region_ok(s)) {
            return None;
        }
        Some(decode::region_memory(s, region, size))
    }
    /// Decode the sensor data (no demosaic). `data` must be the bytes given to [`parse`].
    pub fn decode_raw(&self, data: &[u8], opts: &DecodeOptions) -> Result<RawImage> {
        let s = self
            .sensor
            .as_ref()
            .ok_or(RawError::Unsupported("no sensor data found"))?;
        decode::decode_raw(data, s, self.info.orientation, opts)
    }
    /// The sensor samples as stored with their metadata kept apart (black levels, white,
    /// linearization, CFA layout, DNG opcode lists, white balance, matrices, DNG profile), for
    /// a scene-referred develop on the GPU: see [`sensor`]. Always the whole sensor
    /// ([`DecodeOptions::region`] is ignored); nothing is clipped.
    pub fn decode_sensor(&self, data: &[u8], opts: &DecodeOptions) -> Result<SensorData> {
        let s = self
            .sensor
            .as_ref()
            .ok_or(RawError::Unsupported("no sensor data found"))?;
        let opts = DecodeOptions {
            region: None,
            ..*opts
        };
        // Floating point DNGs (HDR merges): the samples at full precision as well, decoded
        // once (the 16-bit samples are made from them).
        let float = decode::decode_float(data, s, &opts)?;
        let raw = match &float {
            Some((f, short)) => decode::decode_unprocessed_from_float(
                data,
                s,
                self.info.orientation,
                &opts,
                f,
                *short,
            )?,
            None => decode::decode_unprocessed(data, s, self.info.orientation, &opts)?,
        };
        let mut out = sensor::build(raw, s)?;
        out.float = float.map(|(f, _)| std::sync::Arc::from(f));
        Ok(out)
    }

    /// [`RawFile::decode_raw`] without the DNG opcode lists and the vendors' sensor
    /// calibrations (Phase One black tables and calibration block, Sigma X3F lens shading):
    /// the samples as stored, black, white and linearization as the file gives them, no
    /// `OpcodeList3` in the develop (and no `TrimBounds` in the crop).
    pub fn decode_raw_as_stored(&self, data: &[u8], opts: &DecodeOptions) -> Result<RawImage> {
        let s = self
            .sensor
            .as_ref()
            .ok_or(RawError::Unsupported("no sensor data found"))?;
        let mut s = s.without_calibration();
        if let Some(ops) = s.opcodes.take() {
            s.crop = ops.untrimmed_crop;
        }
        decode::decode_raw(data, &s, self.info.orientation, opts)
    }
    /// The sensor samples as [`RawFile::decode_raw`] decodes them but before patterns the
    /// develop cannot demosaic (four-colour filters, some RGB layouts) are turned into camera
    /// RGB. For comparisons with other readers' raw values.
    #[doc(hidden)]
    pub fn decode_raw_mosaic(&self, data: &[u8], opts: &DecodeOptions) -> Result<RawImage> {
        let s = self
            .sensor
            .as_ref()
            .ok_or(RawError::Unsupported("no sensor data found"))?;
        decode::decode_mosaic(data, s, self.info.orientation, opts)
    }
    /// [`RawFile::decode_raw`] followed by [`develop`]; a whole image of a sensor with
    /// non-square pixels (DNG `DefaultScale`) is then resampled to square pixels
    /// ([`SensorInfo::square_size`], halved for [`OutputSize::Half`]). Regions keep the
    /// stored grid.
    pub fn decode(&self, data: &[u8], opts: &DecodeOptions) -> Result<RgbImage> {
        let raw = self.decode_raw(data, opts)?;
        let img = develop(&raw, opts)?;
        match self.sensor.as_ref().and_then(|s| s.default_scale) {
            Some(k) if opts.region.is_none() => Ok(scale::square_pixels(&img, k)),
            _ => Ok(img),
        }
    }
}

pub use scale::resample_rgb;

/// A decoded preview: interleaved RGBA8.
#[derive(Clone, Debug)]
pub struct PreviewImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub orientation: u16,
}

/// Decode an embedded preview to RGBA8 with `specular-jpeg`, using DCT scaling so that the
/// result's longer side is at least `min_long_side` (0 = full size).
pub fn decode_preview(data: &[u8], preview: &Preview, min_long_side: u32) -> Result<PreviewImage> {
    let bytes = data
        .get(preview.range.clone())
        .ok_or(RawError::Malformed("preview out of bounds"))?;
    match preview.kind {
        PreviewKind::Jpeg => {
            let info = specular_jpeg::read_info(bytes)?;
            let scale = if min_long_side == 0 {
                specular_jpeg::Scale::Full
            } else {
                specular_jpeg::choose_scale(&info, min_long_side, min_long_side)
            };
            let planes = specular_jpeg::decode(
                bytes,
                &specular_jpeg::DecodeOptions {
                    scale,
                    ..Default::default()
                },
            )?;
            Ok(PreviewImage {
                width: planes.width,
                height: planes.height,
                rgba: specular_jpeg::planes_to_rgba(&planes),
                orientation: preview.orientation,
            })
        }
        PreviewKind::Rgb8 => {
            let (w, h) = (preview.width as usize, preview.height as usize);
            let stride = bytes.len() / h.max(1);
            if w == 0 || stride < w * 3 {
                return Err(RawError::Malformed("RGB preview truncated"));
            }
            let mut rgba = Vec::with_capacity(w * h * 4);
            for row in bytes.chunks_exact(stride).take(h) {
                for p in row[..w * 3].chunks_exact(3) {
                    rgba.extend_from_slice(&[p[0], p[1], p[2], 255]);
                }
            }
            Ok(PreviewImage {
                width: preview.width,
                height: preview.height,
                rgba,
                orientation: preview.orientation,
            })
        }
        PreviewKind::Grey4 => {
            let n = preview.width as usize * preview.height as usize;
            if bytes.len() * 2 < n {
                return Err(RawError::Malformed("grey preview truncated"));
            }
            let mut rgba = Vec::with_capacity(n * 4);
            for i in 0..n {
                let b = bytes[i / 2];
                let v = if i % 2 == 0 { b >> 4 } else { b & 15 } * 17;
                rgba.extend_from_slice(&[v, v, v, 255]);
            }
            Ok(PreviewImage {
                width: preview.width,
                height: preview.height,
                rgba,
                orientation: preview.orientation,
            })
        }
        PreviewKind::X3fCoded => {
            let rgb =
                x3f::decode_coded_thumb(bytes, preview.width as usize, preview.height as usize)
                    .ok_or(RawError::Malformed("X3F thumbnail"))?;
            let mut rgba = Vec::with_capacity(rgb.len() / 3 * 4);
            for p in rgb.chunks_exact(3) {
                rgba.extend_from_slice(&[p[0], p[1], p[2], 255]);
            }
            Ok(PreviewImage {
                width: preview.width,
                height: preview.height,
                rgba,
                orientation: preview.orientation,
            })
        }
    }
}
