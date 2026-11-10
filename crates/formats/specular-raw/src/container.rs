//! Container parsing: format detection, metadata, embedded previews and the sensor layout for
//! every supported RAW family. Reads headers only.

use std::ops::Range;

mod ari;
mod ciff;
mod clip;
mod iiq;
mod mrw;
mod oldcam;
mod x3f;

use crate::bmff;
use crate::cfa::{BLUE, Cfa, RED};
use crate::error::{RawError, Result};
use crate::makernote::{self, Vendor};
use crate::tiff::{self, Endian, Ifd, Visited, be_u16, be_u32, tag};
use crate::{
    MatrixSource, Preview, PreviewKind, RawFormat, RawInfo, Rect, SensorCompression, SensorInfo,
};

// ---------------------------------------------------------------------------------------------
// Private sensor description

/// How black levels are obtained.
#[derive(Clone, Debug)]
pub(crate) enum Black {
    /// Per 2x2 position (CFA) or per channel (cpp = 3).
    Levels([f32; 4]),
    /// Measured from masked (optical black) sensor pixels at decode time: `area`, and
    /// `also` where the sensor has a second masked strip.
    Masked {
        area: Rect,
        also: Option<Rect>,
        fallback: [f32; 4],
    },
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DngColor {
    pub cm: [Option<[f64; 9]>; 2],
    pub fm: [Option<[f64; 9]>; 2],
    pub cc: [Option<[f64; 9]>; 2],
    pub ab: Option<[f64; 3]>,
    pub illuminant: [u32; 2],
    pub neutral: Option<[f64; 3]>,
    pub white_xy: Option<(f64, f64)>,
    /// ProfileHueSatMapData1/2 (validated).
    pub hue_sat_map: [Option<crate::profile::HsvTable>; 2],
    /// ProfileLookTableData (validated).
    pub look_table: Option<crate::profile::HsvTable>,
    /// ProfileToneCurve (validated; `None` when absent, malformed or the identity).
    pub tone_curve: Option<crate::profile::ToneCurve>,
    /// A table tag is present but unusable: the profile cannot be rendered as designed.
    pub tables_malformed: bool,
    /// The profile was designed to be used with its tables (an Adobe profile such as
    /// "Adobe Standard"): its ForwardMatrix alone is not a finished rendering.
    pub needs_tables: bool,
    /// High dynamic range profile (ProfileDynamicRange).
    pub hdr: bool,
    /// Exposure applied after developing, in EV (BaselineExposure + BaselineExposureOffset).
    pub exposure: f64,
    /// NoiseProfile: (scale, offset) pairs, one for all planes or one per colour plane.
    pub noise: Vec<[f64; 2]>,
}

#[derive(Clone, Debug)]
pub(crate) enum ColorSource {
    Dng(Box<DngColor>),
    Matrix([[f32; 3]; 3], MatrixSource),
    Generic,
    Identity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Packing {
    U8,
    U16Le,
    U16Be,
    /// Big-endian bit order, `bits` per sample, rows padded to whole bytes.
    Msb(u32),
    /// 12-bit little-endian packing (two samples in three bytes, low nibble first).
    Le12,
    /// Big-endian bit order over little-endian 32-bit words, `bits` per sample, rows padded to
    /// whole words (Fujifilm X-A bodies).
    Msb32Le(u32),
    /// Big-endian bit order over little-endian 16-bit words, `bits` per sample, rows padded
    /// to whole words (Canon PowerShot Pro70).
    Msb16Le(u32),
    /// 12-bit big-endian bit order with one padding byte after every 15 bytes (ten samples):
    /// Epson ERF.
    Msb12Pad10,
    /// 12-bit little-endian packing ([`Packing::Le12`]) with one padding byte after every 15
    /// bytes: Panasonic's older `.RAW` (FZ8).
    Le12Pad10,
    /// 12-bit samples in the high bits of 16-bit words, little or big endian (`true`: little):
    /// Olympus's uncompressed ORF (E-1; dcraw's `load_flags = 4`).
    U16Hi12(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Codec {
    /// `row_bytes`: stride of one row inside a block (0 = tightly packed).
    Raw {
        packing: Packing,
        row_bytes: usize,
    },
    Lj92,
    /// Sony lossless ARW: each tile is a 4-component frame of half the tile's width and
    /// height, one MCU per 2x2 CFA block (components 0 and 1 on the even row, 2 and 3 on the
    /// odd row).
    Lj92Quad,
    /// DNG deflate, floating point, lossy JPEG and JPEG XL blocks.
    Dng(crate::dngtile::DngTile),
    /// Sony lossless M/S sizes: subsampled YCbCr lossless JPEG tiles (chroma offset `half`),
    /// converted to camera RGB (cpp 3).
    Ycc {
        half: u32,
    },
}

#[derive(Clone, Debug)]
pub(crate) enum Storage {
    /// TIFF strips or tiles: `blocks` in row-major tile order, each `tile_w x tile_h` pixels.
    Blocks {
        blocks: Vec<Range<usize>>,
        tile_w: u32,
        tile_h: u32,
        codec: Codec,
    },
    /// Canon CR2 lossless JPEG with optional vertical slices `[n, width, last_width]`.
    Cr2 {
        range: Range<usize>,
        slices: Option<[u32; 3]>,
    },
    /// Nikon Huffman (`table` = NEFLinearizationTable).
    Nikon {
        range: Range<usize>,
        table: Range<usize>,
        le: bool,
    },
    SonyArw2 {
        range: Range<usize>,
        curve: Vec<u16>,
    },
    /// Canon sRAW / mRAW: subsampled luma/chroma lossless JPEG, sliced like other CR2 data
    /// (slice widths in MCU samples), converted to camera RGB (cpp 3).
    Cr2Ycc {
        range: Range<usize>,
        slices: Option<[u32; 3]>,
        conv: crate::ycc::Conv,
        /// Per-channel gain in 1/1024 back to sensor RGB.
        mul: [u32; 3],
    },
    /// Sony ARW 1.0: one Huffman-coded stream of 12-bit differences, column by column from the
    /// right, even rows then odd rows; `rows` coded rows per column (at least the height).
    SonyArw1 {
        range: Range<usize>,
        rows: u32,
    },
    Panasonic {
        range: Range<usize>,
        load_flags: usize,
    },
    Pentax {
        range: Range<usize>,
        table: Option<Range<usize>>,
        le: bool,
    },
    /// Fujifilm compressed RAF: `range` starts after the 16-byte header; `cfa[r % 6][c % 6]`.
    Fuji {
        range: Range<usize>,
        header: crate::fuji::Header,
        cfa: [[u8; 6]; 6],
    },
    /// Panasonic RW2 raw formats 5 and 6 (16-byte blocks).
    PanaBlocks {
        range: Range<usize>,
        version: u32,
        bits: u32,
    },
    /// Olympus compressed ORF: the single strip.
    Olympus {
        range: Range<usize>,
    },
    /// Fujifilm SuperCCD: stored photosites on a 45 degree lattice, turned upright into camera
    /// RGB (cpp 3).
    SuperCcd {
        range: Range<usize>,
        geom: crate::superccd::Geometry,
    },
    /// Canon CRX (CR3): `range` is the whole first sample of the raw track.
    Crx {
        range: Range<usize>,
        header: crate::crx::Header,
    },
    /// Apple ProRes RAW: `range` is one frame (a movie sample).
    ProResRaw {
        range: Range<usize>,
    },
    /// A vendor codec over one byte range (see [`crate::vendor`]).
    Vendor {
        range: Range<usize>,
        codec: crate::vendor::VendorCodec,
    },
    None,
}

#[derive(Clone, Debug)]
pub(crate) struct Sensor {
    pub width: u32,
    pub height: u32,
    pub cpp: u32,
    pub bits: u32,
    pub compression: SensorCompression,
    pub storage: Storage,
    pub cfa: Option<Cfa>,
    pub crop: Rect,
    pub black: Black,
    /// None: nominal `(1 << bits) - 1`, adjusted to the observed maximum unless
    /// `white_nominal`.
    pub white: Option<f32>,
    /// With no white level: the nominal `(1 << bits) - 1` as it is (Canon, as LibRaw); the
    /// develop still lowers it to a clip the data shows.
    pub white_nominal: bool,
    pub linearization: Option<Vec<u16>>,
    pub wb: Option<[f32; 3]>,
    pub color: ColorSource,
    pub unsupported: Option<&'static str>,
    /// DNG opcode lists (`crop` already trimmed by `TrimBounds`).
    pub opcodes: Option<std::sync::Arc<crate::DngOpcodes>>,
    /// Rows stored as two fields: the even rows, then the odd rows (Nikon Coolpix).
    pub fields: bool,
    /// Colour data of a four-colour sensor with a published matrix (see [`crate::cfagen`]).
    pub four: Option<Box<crate::cfagen::FourColor>>,
    /// A staggered (non-rectangular) DNG CFA layout: demosaiced on decode.
    pub stagger: Option<crate::cfagen::Stagger>,
    /// DNG black levels as the file gives them (whole repeat pattern, per-row and per-column
    /// deltas), for [`crate::sensor`]; `black` keeps the 2x2 summary the develop uses.
    pub black_area: Option<Box<crate::sensor::BlackLevels>>,
    /// The file's own lens correction tables (Sony, Fujifilm maker data).
    pub lens: Option<std::sync::Arc<crate::makernote::LensCorrection>>,
    /// Horizontal and vertical stretch to square pixels (DNG `DefaultScale` divided by its
    /// smaller value, so at least 1 each, one of them above 1); `None` for square pixels.
    pub default_scale: Option<[f32; 2]>,
}

impl Sensor {
    fn new(width: u32, height: u32, bits: u32, compression: SensorCompression) -> Sensor {
        Sensor {
            width,
            height,
            cpp: 1,
            bits,
            compression,
            storage: Storage::None,
            cfa: None,
            crop: Rect {
                x: 0,
                y: 0,
                w: width,
                h: height,
            },
            black: Black::Levels([0.0; 4]),
            white: None,
            white_nominal: false,
            linearization: None,
            wb: None,
            color: ColorSource::Generic,
            unsupported: None,
            opcodes: None,
            fields: false,
            four: None,
            stagger: None,
            black_area: None,
            lens: None,
            default_scale: None,
        }
    }

    fn info(&self) -> SensorInfo {
        SensorInfo {
            width: self.width,
            height: self.height,
            crop: self.crop,
            samples_per_pixel: self.cpp,
            bits: self.bits,
            compression: self.compression,
            cfa: self.cfa.clone(),
            decodable: self.unsupported.is_none() && !matches!(self.storage, Storage::None),
            unsupported_reason: if matches!(self.storage, Storage::None)
                && self.unsupported.is_none()
            {
                Some("sensor data layout not supported")
            } else {
                self.unsupported
            },
            opcodes: self
                .opcodes
                .as_ref()
                .map_or_else(Vec::new, |o| o.applied().to_vec()),
            opcodes_skipped: self
                .opcodes
                .as_ref()
                .map_or_else(Vec::new, |o| o.skipped().to_vec()),
            square_size: crate::scale::square_size(self.crop.w, self.crop.h, self.default_scale),
            demosaiced: self
                .cfa
                .as_ref()
                .is_some_and(|c| self.stagger.is_some() || crate::cfagen::supported(c)),
        }
    }

    fn black_is_zero(&self) -> bool {
        matches!(self.black, Black::Levels(b) if b.iter().all(|&v| v == 0.0))
    }

    /// The same sensor without the vendor calibrations the decode applies (Phase One black
    /// tables and calibration block: the black level back as a level; X3F lens shading).
    pub(crate) fn without_calibration(&self) -> Sensor {
        let mut s = self.clone();
        if let Storage::Vendor { codec, .. } = &mut s.storage {
            match codec {
                crate::vendor::VendorCodec::PhaseOne { cal, .. }
                | crate::vendor::VendorCodec::PhaseOnePlain { cal, .. } => {
                    let black = cal.black as f32;
                    **cal = crate::phase_one_cal::Calibration::default();
                    s.black = Black::Levels([black; 4]);
                    s.white = s.white.map(|w| w + black);
                }
                crate::vendor::VendorCodec::X3fTrue { gain, .. }
                | crate::vendor::VendorCodec::X3fHuffman { gain, .. } => *gain = None,
                _ => {}
            }
        }
        s
    }

    fn unsupported(mut self, why: &'static str) -> Sensor {
        self.unsupported = Some(why);
        self
    }
}

// ---------------------------------------------------------------------------------------------
// Detection

const CANON_UUID: [u8; 16] = [
    0x85, 0xc0, 0xb6, 0x87, 0x82, 0x0f, 0x11, 0xe0, 0x81, 0x11, 0xf4, 0xce, 0x46, 0x2b, 0x6a, 0x48,
];
const PRVW_UUID: [u8; 16] = [
    0xea, 0xf4, 0x2b, 0x5e, 0x1c, 0x98, 0x4b, 0x88, 0xb9, 0xfb, 0xb7, 0xdc, 0x40, 0x6e, 0x4d, 0x16,
];

fn is_cr3(d: &[u8]) -> bool {
    d.get(4..8) == Some(b"ftyp") && d.get(8..12) == Some(b"crx ")
}

fn is_raf(d: &[u8]) -> bool {
    d.starts_with(b"FUJIFILMCCD-RAW")
}

pub(crate) fn detect(d: &[u8]) -> Option<RawFormat> {
    if is_raf(d) {
        return Some(RawFormat::Raf);
    }
    if let Some(f) = foreign_format(d) {
        return Some(f);
    }
    if clip::is_crm(d) {
        return Some(RawFormat::Crm);
    }
    if is_cr3(d) {
        return Some(RawFormat::Cr3);
    }
    if clip::is_prores_raw(d) {
        return Some(RawFormat::ProResRaw);
    }
    let (le, first) = tiff::header(d, 0)?;
    let walk = Walk::new(d, le, first);
    classify(d, &walk)
}

// ---------------------------------------------------------------------------------------------
// TIFF walk

struct Walk {
    le: bool,
    /// IFD0, IFD1, ... (main chain).
    main: Vec<Ifd>,
    /// SubIFDs of the main IFDs (one level, plus their own SubIFDs).
    subs: Vec<Ifd>,
    exif: Option<Ifd>,
    maker: Option<Ifd>,
    vendor: Vendor,
}

impl Walk {
    fn new(d: &[u8], le: bool, first: usize) -> Walk {
        let mut v = Visited::default();
        let main = tiff::chain(d, first, 0, le, &mut v);
        let mut subs = Vec::new();
        let mut queue: Vec<usize> = main
            .iter()
            .flat_map(|i| i.offsets(d, tag::SUB_IFDS, 32))
            .collect();
        while let Some(off) = queue.pop() {
            if !v.insert(off) {
                continue;
            }
            if let Some(ifd) = tiff::read_ifd(d, off, 0, le) {
                queue.extend(ifd.offsets(d, tag::SUB_IFDS, 32));
                subs.push(ifd);
            }
        }
        subs.sort_by_key(|i| i.offset);
        let ifd0 = main.first();
        let exif = ifd0
            .and_then(|i| i.offsets(d, tag::EXIF_IFD, 1).first().copied())
            .filter(|&o| v.insert(o))
            .and_then(|o| tiff::read_ifd(d, o, 0, le));
        let make = ifd0
            .and_then(|i| i.string(d, tag::MAKE))
            .unwrap_or_default();
        let vendor = makernote::vendor(&make);
        let maker = exif
            .as_ref()
            .and_then(|e| e.get(tag::MAKER_NOTE))
            .and_then(|e| makernote::maker_ifd(d, e, 0, le, vendor));
        Walk {
            le,
            main,
            subs,
            exif,
            maker,
            vendor,
        }
    }

    fn ifd0(&self) -> Option<&Ifd> {
        self.main.first()
    }

    fn image_ifds(&self) -> impl Iterator<Item = &Ifd> {
        self.main.iter().chain(self.subs.iter())
    }
}

/// Leaf's private metadata tag (PKTS records).
const LEAF_PKTS: u16 = 0x8606;

fn is_rawish(d: &[u8], ifd: &Ifd) -> bool {
    let photometric = ifd.uint(d, tag::PHOTOMETRIC).unwrap_or(0);
    let compression = ifd.uint(d, tag::COMPRESSION).unwrap_or(1);
    let bps = ifd.uint(d, tag::BITS_PER_SAMPLE).unwrap_or(0);
    let spp = ifd.uint(d, tag::SAMPLES_PER_PIXEL).unwrap_or(1);
    matches!(photometric, 32803 | 34892)
        || matches!(
            compression,
            34713 | 32767 | 32769 | 32770 | 32772 | 65535 | 34316 | 65000 | 32867 | 34892
        )
        || (compression == 7 || compression == 32773) && bps > 8
        || (10..=16).contains(&bps) && spp == 1 && ifd.get(tag::IMAGE_WIDTH).is_some()
        || ifd.get(KODAK_KDC_STRIPS).is_some()
}

/// The strip table of Kodak's P-series KDC sensor IFD.
const KODAK_KDC_STRIPS: u16 = 0xfd04;

fn classify(d: &[u8], w: &Walk) -> Option<RawFormat> {
    let ifd0 = w.ifd0()?;
    if ifd0.get(tag::DNG_VERSION).is_some() {
        return Some(RawFormat::Dng);
    }
    if d.get(8..10) == Some(b"CR") {
        return Some(RawFormat::Cr2);
    }
    let magic = Endian { le: w.le }.u16(d, 2)?;
    if magic == 0x4f52 || magic == 0x5352 {
        return Some(RawFormat::Orf);
    }
    if magic == 0x55 {
        return Some(RawFormat::Rw2);
    }
    if a100(d, ifd0).is_some() {
        return Some(RawFormat::Arw);
    }
    let model = ifd0
        .string(d, tag::MODEL)
        .unwrap_or_default()
        .to_ascii_uppercase();
    // The F828's sensor data is not described by any IFD.
    if w.vendor == Vendor::Sony && model == "DSC-F828" {
        return Some(RawFormat::Srf);
    }
    if !w.image_ifds().any(|i| is_rawish(d, i)) {
        return None;
    }
    if ifd0.get(LEAF_PKTS).is_some() {
        return Some(RawFormat::Mos);
    }
    Some(match w.vendor {
        Vendor::Nikon if model.contains("COOLPIX") => RawFormat::Nrw,
        Vendor::Nikon => RawFormat::Nef,
        Vendor::Sony if model.starts_with("DSC-R1") => RawFormat::Sr2,
        Vendor::Sony => RawFormat::Arw,
        Vendor::Pentax => RawFormat::Pef,
        Vendor::Olympus => RawFormat::Orf,
        _ => tiff_vendor_format(d, ifd0).unwrap_or(RawFormat::TiffRaw),
    })
}

/// The family of a TIFF-based RAW of a maker without its own signature, from `Make`.
fn tiff_vendor_format(d: &[u8], ifd0: &Ifd) -> Option<RawFormat> {
    let make = ifd0.string(d, tag::MAKE)?.to_ascii_lowercase();
    let make = make.trim();
    Some(if make.starts_with("samsung") {
        RawFormat::Srw
    } else if make.starts_with("hasselblad") {
        RawFormat::Hasselblad
    } else if make.starts_with("leaf") {
        RawFormat::Mos
    } else if make.starts_with("mamiya") {
        RawFormat::Mef
    } else if make.starts_with("kodak") || make.starts_with("eastman kodak") {
        RawFormat::Kodak
    } else if make.contains("epson") {
        RawFormat::Erf
    } else if make.starts_with("phase one") {
        RawFormat::Iiq
    } else if make.starts_with("sinar") {
        RawFormat::Sinar
    } else {
        return None;
    })
}

// ---------------------------------------------------------------------------------------------
// Metadata

fn fill_basic(info: &mut RawInfo, d: &[u8], ifd0: &Ifd) {
    if info.make.is_empty() {
        info.make = ifd0.string(d, tag::MAKE).unwrap_or_default();
    }
    if info.model.is_empty() {
        info.model = ifd0.string(d, tag::MODEL).unwrap_or_default();
    }
    if info.orientation == 0 {
        info.orientation = ifd0
            .uint(d, tag::ORIENTATION)
            .filter(|o| (1..=8).contains(o))
            .unwrap_or(0) as u16;
    }
    if info.software.is_none() {
        info.software = ifd0.string(d, tag::SOFTWARE);
    }
    if info.date_time.is_none() {
        info.date_time = ifd0
            .string(d, tag::DATE_TIME_ORIGINAL)
            .or_else(|| ifd0.string(d, tag::DATE_TIME));
    }
    // TIFF/EP files may carry Exif tags in IFD0.
    fill_exif(info, d, ifd0);
}

fn fill_exif(info: &mut RawInfo, d: &[u8], exif: &Ifd) {
    if info.iso.is_none() {
        info.iso = exif.uint(d, tag::ISO).filter(|&v| v > 0 && v < 10_000_000);
    }
    if info.exposure_time.is_none() {
        info.exposure_time = exif.float(d, tag::EXPOSURE_TIME).filter(|&v| v > 0.0);
    }
    if info.f_number.is_none() {
        info.f_number = exif.float(d, tag::F_NUMBER).filter(|&v| v > 0.0);
    }
    if info.focal_length.is_none() {
        info.focal_length = exif.float(d, tag::FOCAL_LENGTH).filter(|&v| v > 0.0);
    }
    if info.exposure_bias.is_none() {
        info.exposure_bias = exif.float(d, tag::EXPOSURE_BIAS);
    }
    if info.lens.is_none() {
        info.lens = exif
            .string(d, tag::LENS_MODEL)
            .filter(|s| s.chars().any(|c| c.is_alphanumeric()));
    }
    if let Some(dt) = exif.string(d, tag::DATE_TIME_ORIGINAL) {
        info.date_time = Some(dt);
    }
}

/// Fill missing metadata from a TIFF-structured block (e.g. the Exif of an embedded JPEG).
fn fill_from_tiff_block(info: &mut RawInfo, d: &[u8], block: Range<usize>) {
    let Some(b) = d.get(block.clone()) else {
        return;
    };
    let Some((le, first)) = tiff::header(b, 0) else {
        return;
    };
    let mut v = Visited::default();
    let chain = tiff::chain(b, first, 0, le, &mut v);
    let Some(ifd0) = chain.first() else { return };
    fill_basic(info, b, ifd0);
    if let Some(o) = ifd0.offsets(b, tag::EXIF_IFD, 1).first()
        && v.insert(*o)
        && let Some(e) = tiff::read_ifd(b, *o, 0, le)
    {
        fill_exif(info, b, &e);
    }
}

// ---------------------------------------------------------------------------------------------
// Previews

#[derive(Default)]
struct Previews {
    list: Vec<Preview>,
}

impl Previews {
    fn seen(&self, start: usize) -> bool {
        self.list.iter().any(|p| p.range.start == start)
    }

    /// Validate and add a JPEG at `range` (clamped to the file).
    fn jpeg(
        &mut self,
        d: &[u8],
        range: Range<usize>,
        source: &'static str,
    ) -> Option<specular_jpeg::JpegInfo> {
        let end = range.end.min(d.len());
        if range.start.checked_add(4).is_none_or(|e| e > end) || self.seen(range.start) {
            return None;
        }
        let b = &d[range.start..end];
        if b[0] != 0xff || b[1] != 0xd8 {
            return None;
        }
        let ji = specular_jpeg::read_info(b).ok()?;
        if ji.precision != 8
            || matches!(ji.coding, specular_jpeg::Coding::Lossless)
            || ji.width == 0
            || ji.height == 0
        {
            return None;
        }
        self.list.push(Preview {
            kind: PreviewKind::Jpeg,
            range: range.start..end,
            width: ji.width,
            height: ji.height,
            orientation: 1,
            source,
        });
        Some(ji)
    }

    /// A JPEG preview plus the Exif thumbnail inside it.
    fn jpeg_and_thumb(&mut self, d: &[u8], range: Range<usize>, source: &'static str) {
        let start = range.start;
        if let Some(ji) = self.jpeg(d, range, source)
            && let Some(t) = ji.exif_thumbnail
        {
            self.jpeg(
                d,
                start + t.start..start + t.end,
                "embedded JPEG Exif thumbnail",
            );
        }
    }

    fn rgb(&mut self, d: &[u8], start: usize, w: u32, h: u32, source: &'static str) {
        self.rgb_rows(d, start, w, h, w as usize * 3, source);
    }

    /// Uncompressed RGB8 rows `stride` bytes apart (at least `3 * w`).
    fn rgb_rows(
        &mut self,
        d: &[u8],
        start: usize,
        w: u32,
        h: u32,
        stride: usize,
        source: &'static str,
    ) {
        if w == 0 || h == 0 || w > 16384 || h > 16384 || stride < w as usize * 3 {
            return;
        }
        if stride > w as usize * 3 + 64 || self.seen(start) {
            return;
        }
        let n = stride * h as usize;
        if start.checked_add(n).is_none_or(|e| e > d.len()) {
            return;
        }
        self.list.push(Preview {
            kind: PreviewKind::Rgb8,
            range: start..start + n,
            width: w,
            height: h,
            orientation: 1,
            source,
        });
    }

    /// A Sigma X3F Huffman-coded thumbnail (format 11) in `range`.
    fn x3f_coded(&mut self, range: Range<usize>, w: u32, h: u32, source: &'static str) {
        if w == 0 || h == 0 || w > 16384 || h > 16384 || self.seen(range.start) {
            return;
        }
        if range.len() < 1024 + h as usize * 4 {
            return;
        }
        // Every pixel costs at least a bit per channel: a large thumbnail needs its data.
        if w as usize * h as usize > range.len().saturating_mul(8) {
            return;
        }
        self.list.push(Preview {
            kind: PreviewKind::X3fCoded,
            range,
            width: w,
            height: h,
            orientation: 1,
            source,
        });
    }

    /// Generic previews of one IFD: JPEGInterchangeFormat, single-strip JPEG, uncompressed RGB8.
    fn add_ifd(&mut self, d: &[u8], ifd: &Ifd, source: &'static str) {
        if let (Some(o), Some(l)) = (
            ifd.uint(d, tag::JPEG_IF_OFFSET),
            ifd.uint(d, tag::JPEG_IF_LENGTH),
        ) {
            let s = ifd.base.saturating_add(o as usize);
            self.jpeg_and_thumb(d, s..s.saturating_add(l as usize), source);
        }
        let photometric = ifd.uint(d, tag::PHOTOMETRIC);
        if matches!(photometric, Some(32803 | 34892)) {
            return;
        }
        let offs = ifd.uints(d, tag::STRIP_OFFSETS, 2);
        let lens = ifd.uints(d, tag::STRIP_BYTE_COUNTS, 2);
        if offs.len() != 1 || lens.len() != 1 {
            return;
        }
        let s = ifd.base.saturating_add(offs[0] as usize);
        let l = lens[0] as usize;
        if d.get(s..s + 2) == Some(&[0xff, 0xd8]) {
            self.jpeg_and_thumb(d, s..s.saturating_add(l), source);
            return;
        }
        let w = ifd.uint(d, tag::IMAGE_WIDTH).unwrap_or(0);
        let h = ifd.uint(d, tag::IMAGE_LENGTH).unwrap_or(0);
        let bps = ifd.uints(d, tag::BITS_PER_SAMPLE, 4);
        let spp = ifd
            .uint(d, tag::SAMPLES_PER_PIXEL)
            .unwrap_or(bps.len() as u32);
        let compression = ifd.uint(d, tag::COMPRESSION).unwrap_or(1);
        if spp == 3
            && bps.iter().all(|&b| b == 8)
            && matches!(compression, 1 | 6)
            && matches!(photometric, None | Some(2))
            && l >= (w as usize).saturating_mul(h as usize).saturating_mul(3)
        {
            self.rgb(d, s, w, h, source);
        }
    }

    fn finish(mut self, orientation: u16) -> Vec<Preview> {
        for p in &mut self.list {
            p.orientation = orientation;
        }
        // Largest first; JPEG before RGB at equal size.
        self.list.sort_by(|a, b| {
            b.pixels()
                .cmp(&a.pixels())
                .then((a.kind == PreviewKind::Rgb8).cmp(&(b.kind == PreviewKind::Rgb8)))
        });
        self.list
    }
}

// ---------------------------------------------------------------------------------------------
// Entry point

/// Containers that are not TIFF-based (or not only).
fn foreign_format(d: &[u8]) -> Option<RawFormat> {
    if mrw::is_mrw(d) {
        return Some(RawFormat::Mrw);
    }
    if x3f::is_x3f(d) {
        return Some(RawFormat::X3f);
    }
    if ari::is_ari(d) {
        return Some(RawFormat::Ari);
    }
    if ciff::is_crw(d) {
        return Some(RawFormat::Crw);
    }
    if iiq::find(d).is_some() {
        return Some(RawFormat::Iiq);
    }
    oldcam::detect(d)
}

pub(crate) use oldcam::is_headerless_camera_raw;

/// [`detect`] with the file's extension: also the formats recognised by name and content.
pub(crate) fn detect_named(d: &[u8], ext: &str) -> Option<RawFormat> {
    detect(d).or_else(|| oldcam::detect_named(d, ext))
}

/// [`parse`] with the file's extension.
pub(crate) fn parse_named(d: &[u8], ext: &str) -> Result<(RawInfo, Option<Sensor>)> {
    match parse(d) {
        Err(RawError::NotRaw) => match oldcam::detect_named(d, ext) {
            Some(f) => oldcam::parse_ext(d, f, ext),
            None => Err(RawError::NotRaw),
        },
        r => r,
    }
}

pub(crate) fn parse(d: &[u8]) -> Result<(RawInfo, Option<Sensor>)> {
    if is_raf(d) {
        return parse_raf(d);
    }
    match foreign_format(d) {
        Some(RawFormat::Mrw) => return mrw::parse(d),
        Some(RawFormat::Crw) => return ciff::parse(d),
        Some(RawFormat::X3f) => return x3f::parse(d),
        Some(RawFormat::Ari) => return ari::parse(d),
        Some(f @ (RawFormat::QuickTake | RawFormat::J6i | RawFormat::Lytro)) => {
            return oldcam::parse(d, f);
        }
        Some(RawFormat::Iiq) => {
            if let Some((base, le)) = iiq::find(d) {
                return iiq::parse(d, base, le);
            }
        }
        _ => {}
    }
    if clip::is_crm(d) {
        return clip::parse_crm(d);
    }
    if is_cr3(d) {
        return parse_cr3(d);
    }
    if clip::is_prores_raw(d) {
        return clip::parse_prores_raw(d);
    }
    let (le, first) = tiff::header(d, 0).ok_or(RawError::NotRaw)?;
    let w = Walk::new(d, le, first);
    let format = classify(d, &w).ok_or(RawError::NotRaw)?;
    let ifd0 = w.ifd0().ok_or(RawError::Malformed("no IFD0"))?;
    let mut info = RawInfo {
        format: Some(format),
        tiff_blocks: std::iter::once(0..d.len()).collect(),
        ..Default::default()
    };
    fill_basic(&mut info, d, ifd0);
    if let Some(e) = &w.exif {
        fill_exif(&mut info, d, e);
    }
    info.baseline_exposure = baseline_exposure(d, ifd0);

    // Previews.
    let mut pv = Previews::default();
    for (i, ifd) in w.main.iter().enumerate() {
        pv.add_ifd(d, ifd, if i == 0 { "IFD0" } else { "IFD chain" });
    }
    for ifd in &w.subs {
        pv.add_ifd(d, ifd, "SubIFD");
    }
    let vendor = w.vendor;
    let mut olympus = None;
    let mut pentax = None;
    let mut nikon = None;
    let mut canon = None;
    if let Some(mn) = &w.maker {
        pv.add_ifd(d, mn, "MakerNote");
        match vendor {
            Vendor::Nikon => {
                let n = makernote::nikon(d, mn);
                if let Some(p) = &n.preview_ifd {
                    pv.add_ifd(d, p, "MakerNote PreviewIFD");
                }
                nikon = Some(n);
            }
            Vendor::Olympus => {
                let o = makernote::olympus(d, mn);
                if let Some(r) = o.preview.clone() {
                    pv.jpeg_and_thumb(d, r, "MakerNote PreviewImage");
                }
                olympus = Some(o);
            }
            Vendor::Pentax => {
                let p = makernote::pentax(d, mn);
                if let Some(r) = p.preview.clone() {
                    pv.jpeg_and_thumb(d, r, "MakerNote PreviewImage");
                }
                pentax = Some(p);
            }
            Vendor::Canon => {
                if info.lens.is_none() {
                    info.lens = makernote::canon_lens(d, mn);
                }
                canon = Some(makernote::canon(d, mn));
            }
            _ => {}
        }
    }
    if format == RawFormat::Rw2
        && let Some(e) = ifd0.get(0x002e)
    {
        pv.jpeg_and_thumb(d, e.pos..e.pos + e.byte_len(), "RW2 JpgFromRaw");
    }
    let mut leaf = None;
    if format == RawFormat::Mos
        && let Some(e) = ifd0.get(LEAF_PKTS)
    {
        let l = crate::leaf::pkts(d, e.pos..e.pos.saturating_add(e.byte_len()));
        if info.make.is_empty() {
            info.make = "Leaf".into();
        }
        if info.model.is_empty()
            && let Some(b) = l.back
        {
            info.model = b.into();
        }
        if info.orientation == 0
            && let Some(r) = l.rotation
        {
            info.orientation = match r.rem_euclid(360) {
                90 => 6,
                180 => 3,
                270 => 8,
                _ => 1,
            };
        }
        if let Some(r) = l.preview.clone() {
            pv.jpeg_and_thumb(d, r, "Leaf JPEG preview");
        }
        leaf = Some(l);
    }
    // Metadata the TIFF structure lacks (RW2 keeps Exif in its JpgFromRaw).
    if info.iso.is_none() || info.exposure_time.is_none() {
        let jpegs: Vec<Range<usize>> = pv
            .list
            .iter()
            .filter(|p| p.kind == PreviewKind::Jpeg)
            .map(|p| p.range.clone())
            .collect();
        for r in jpegs {
            if let Ok(ji) = specular_jpeg::read_info(&d[r.clone()])
                && let Some(ex) = ji.exif
            {
                fill_from_tiff_block(&mut info, d, r.start + ex.start..r.start + ex.end);
                break;
            }
        }
    }
    if info.orientation == 0 {
        info.orientation = 1;
    }
    info.previews = pv.finish(info.orientation);

    let sensor = match format {
        RawFormat::Dng => dng_sensor(d, &w),
        RawFormat::Cr2 => cr2_sensor(d, &w, canon.unwrap_or_default()),
        RawFormat::Nef | RawFormat::Nrw => {
            nef_sensor(d, &w, nikon.unwrap_or_default(), &info.model)
        }
        RawFormat::Arw | RawFormat::Sr2 => arw_sensor(d, &w),
        RawFormat::Rw2 => rw2_sensor(d, &w),
        RawFormat::Pef => pef_sensor(d, &w, pentax.unwrap_or_default()),
        RawFormat::Orf => orf_sensor(d, &w, olympus.unwrap_or_default()),
        RawFormat::Hasselblad => hasselblad_sensor(d, &w),
        RawFormat::Mos => mos_sensor(d, &w, leaf),
        RawFormat::Mef => tiff_bayer_sensor(d, &w, Some("RGGB")),
        RawFormat::Erf => erf_sensor(d, &w),
        RawFormat::Kodak => kodak_sensor(d, &w),
        RawFormat::Srw => srw_sensor(d, &w),
        RawFormat::Srf => Some(srf_sensor(d, &info.model)),
        RawFormat::Sinar => sinar_sensor(d, &w),
        _ => generic_sensor(d, &w),
    };
    info.sensor = sensor.as_ref().map(Sensor::info);
    Ok((info, sensor))
}

/// Why a CRX track that parses cannot be shown: encoding 1 stores signed samples, and no
/// camera file or public description says where black sits, so the values cannot be placed
/// (LibRaw only clamps them).
fn crx_refusal(h: &crate::crx::Header) -> Option<&'static str> {
    (h.enc_type == 1)
        .then_some("Canon CR3: signed CRX samples (no camera file or specification known)")
}

/// Sony SRF (DSC-F828): encrypted 14-bit red, green, blue and emerald photosites at a fixed
/// place, black from the masked columns on the right.
fn srf_sensor(d: &[u8], model: &str) -> Sensor {
    let (w, h) = (3360u32, 2460u32);
    let mut s = Sensor::new(w, h, 14, SensorCompression::Other(0));
    if !model.contains("DSC-F828") {
        // The other SRF body, the DSC-V3, has no public sample file to check a decoder against.
        return s
            .unsupported("Sony SRF from a camera other than the DSC-F828 (no sample file known)");
    }
    let Some(key) = crate::srf::key(d) else {
        return s.unsupported("Sony SRF: the key block is missing (the file is damaged)");
    };
    // Emerald red / blue green from the sensor origin.
    s.cfa = Cfa::new(2, 2, vec![crate::cfagen::EMERALD, RED, BLUE, 1]);
    s.crop = Rect {
        x: 6,
        y: 0,
        w: 3287,
        h,
    };
    s.black = Black::Masked {
        area: Rect {
            x: 3294,
            y: 0,
            w: 48,
            h,
        },
        also: None,
        fallback: [492.0; 4],
    };
    s.white = Some(16368.0);
    s.four = crate::color::four_color_entry("Sony", model)
        .and_then(|e| crate::cfagen::FourColor::new(e.colors, &e.matrix, None))
        .map(Box::new);
    let start = crate::srf::DATA;
    s.storage = Storage::Vendor {
        range: start..start + (w * h * 2) as usize,
        codec: crate::vendor::VendorCodec::SonySrf { key },
    };
    s
}

// ---------------------------------------------------------------------------------------------
// Shared sensor helpers

fn dims(d: &[u8], ifd: &Ifd) -> (u32, u32) {
    (
        ifd.uint(d, tag::IMAGE_WIDTH).unwrap_or(0),
        ifd.uint(d, tag::IMAGE_LENGTH).unwrap_or(0),
    )
}

fn bps(d: &[u8], ifd: &Ifd) -> u32 {
    ifd.uint(d, tag::BITS_PER_SAMPLE).unwrap_or(0)
}

/// The largest raw-looking IFD.
fn largest_raw_ifd<'a>(d: &[u8], w: &'a Walk) -> Option<&'a Ifd> {
    w.image_ifds().filter(|i| is_rawish(d, i)).max_by_key(|i| {
        let (a, b) = dims(d, i);
        a as u64 * b as u64
    })
}

fn clamp_range(d: &[u8], start: usize, len: usize) -> Range<usize> {
    let s = start.min(d.len());
    s..start.saturating_add(len).min(d.len())
}

/// Strip/tile layout of an IFD.
fn blocks(d: &[u8], ifd: &Ifd, width: u32, height: u32) -> Option<(Vec<Range<usize>>, u32, u32)> {
    const MAX_BLOCKS: usize = 1 << 20;
    if let (Some(tw), Some(th)) = (ifd.uint(d, tag::TILE_WIDTH), ifd.uint(d, tag::TILE_LENGTH)) {
        let offs = ifd.uints(d, tag::TILE_OFFSETS, MAX_BLOCKS);
        let lens = ifd.uints(d, tag::TILE_BYTE_COUNTS, MAX_BLOCKS);
        // Tiles are at least 16 wide and long (TIFF requires multiples of 16): 1 × 1 tiles
        // would cost a tile record per sensor pixel.
        if tw < 16 || th < 16 || offs.is_empty() {
            return None;
        }
        let r = offs
            .iter()
            .zip(lens.iter().chain(std::iter::repeat(&0)))
            .map(|(&o, &l)| clamp_range(d, ifd.base + o as usize, l as usize))
            .collect();
        return Some((r, tw, th));
    }
    let offs = ifd.uints(d, tag::STRIP_OFFSETS, MAX_BLOCKS);
    let lens = ifd.uints(d, tag::STRIP_BYTE_COUNTS, MAX_BLOCKS);
    if offs.is_empty() {
        return None;
    }
    let rps = ifd
        .uint(d, tag::ROWS_PER_STRIP)
        .filter(|&r| r > 0)
        .unwrap_or(height)
        .min(height.max(1));
    let r: Vec<Range<usize>> = offs
        .iter()
        .enumerate()
        .map(|(i, &o)| {
            // A missing byte count: up to the next strip / end of file.
            let l = lens.get(i).copied().map(|l| l as usize).unwrap_or_else(|| {
                offs.get(i + 1)
                    .map_or(d.len(), |&n| n as usize)
                    .saturating_sub(o as usize)
            });
            clamp_range(d, ifd.base + o as usize, l)
        })
        .collect();
    let _ = width;
    Some((r, width, rps))
}

/// Raw packing of an uncompressed block from its size.
fn packing_for(
    bits: u32,
    le: bool,
    w: u32,
    h: u32,
    cpp: u32,
    bytes: usize,
    dng: bool,
) -> Option<(Packing, usize)> {
    let rowsamp = (w as usize).saturating_mul(cpp as usize);
    let samples = rowsamp.saturating_mul(h as usize);
    if bits == 8 {
        return Some((Packing::U8, rowsamp));
    }
    if bits == 16 || !dng && bytes >= samples.saturating_mul(2) && h > 0 {
        let row = (bytes / h.max(1) as usize).max(rowsamp.saturating_mul(2));
        let row = if bytes >= row.saturating_mul(h as usize) {
            row
        } else {
            rowsamp.saturating_mul(2)
        };
        return Some((if le { Packing::U16Le } else { Packing::U16Be }, row));
    }
    if (1..16).contains(&bits) {
        let min_row = rowsamp.saturating_mul(bits as usize).div_ceil(8);
        let row = if h > 0 && bytes / h as usize > min_row && bytes.is_multiple_of(h as usize) {
            bytes / h as usize
        } else {
            min_row
        };
        return Some((Packing::Msb(bits), row));
    }
    let _ = samples;
    None
}

#[allow(clippy::too_many_arguments)]
fn uncompressed_storage(
    d: &[u8],
    ifd: &Ifd,
    w: u32,
    h: u32,
    cpp: u32,
    bits: u32,
    le: bool,
    dng: bool,
) -> Option<Storage> {
    let (blocks, tw, th) = blocks(d, ifd, w, h)?;
    let first = blocks.first()?.len();
    let declared = ifd.uints(
        d,
        if ifd.get(tag::TILE_WIDTH).is_some() {
            tag::TILE_BYTE_COUNTS
        } else {
            tag::STRIP_BYTE_COUNTS
        },
        1,
    );
    let bytes = declared.first().map_or(first, |&b| b as usize);
    let bh = th.min(h);
    let (packing, row_bytes) = packing_for(bits, le, tw, bh, cpp, bytes, dng)?;
    Some(Storage::Blocks {
        blocks,
        tile_w: tw,
        tile_h: th,
        codec: Codec::Raw { packing, row_bytes },
    })
}

/// CFA from TIFF/EP tags (CFARepeatPatternDim + CFAPattern) or the Exif CFAPattern.
fn cfa_from_tags(d: &[u8], ifds: &[&Ifd]) -> Option<Cfa> {
    for ifd in ifds {
        let dim = ifd.uints(d, tag::CFA_REPEAT_DIM, 2);
        if let Some(p) = ifd.get(tag::CFA_PATTERN).and_then(|e| e.bytes(d)) {
            let (r, c) = if dim.len() == 2 {
                (dim[0], dim[1])
            } else {
                (2, 2)
            };
            if r.checked_mul(c).is_some_and(|n| n as usize == p.len())
                && let Some(cfa) = Cfa::new(c, r, p.to_vec())
            {
                return Some(cfa);
            }
        }
        if let Some(e) = ifd.get(tag::EXIF_CFA_PATTERN)
            && let Some(b) = e.bytes(d)
            && b.len() >= 4
        {
            // Two 16-bit dims (byte order varies; try both) then the pattern.
            for le in [ifd.le, !ifd.le] {
                let en = Endian { le };
                let (c, r) = (en.u16(b, 0)? as u32, en.u16(b, 2)? as u32);
                if r * c > 0
                    && b.len() == 4 + (r * c) as usize
                    && let Some(cfa) = Cfa::new(c, r, b[4..].to_vec())
                {
                    return Some(cfa);
                }
            }
        }
    }
    None
}

/// Per-position black levels from per-colour R, G1, G2, B values.
fn black_from_rggb(cfa: Option<&Cfa>, b: [f32; 4]) -> [f32; 4] {
    let Some(p) = cfa.and_then(Cfa::bayer2x2) else {
        let m = (b[0] + b[1] + b[2] + b[3]) / 4.0;
        return [m; 4];
    };
    let mut out = [0.0; 4];
    let mut g = 0;
    for (i, &c) in p.iter().enumerate() {
        out[i] = match c {
            RED => b[0],
            BLUE => b[3],
            _ => {
                g += 1;
                if g == 1 { b[1] } else { b[2] }
            }
        };
    }
    out
}

fn wb_from_rggb(v: [f64; 4]) -> Option<[f32; 3]> {
    let g = (v[1] + v[2]) / 2.0;
    (g > 0.0 && v[0] > 0.0 && v[3] > 0.0).then(|| [(v[0] / g) as f32, 1.0, (v[3] / g) as f32])
}

// ---------------------------------------------------------------------------------------------
// DNG

fn dng_matrix(d: &[u8], ifd: &Ifd, t: u16, n: usize) -> Option<[f64; 9]> {
    let v = ifd.floats(d, t, n);
    if v.len() < 9 {
        return None;
    }
    let mut m = [0.0; 9];
    m.copy_from_slice(&v[..9]);
    Some(m)
}

/// DNG BaselineExposure plus the default profile's BaselineExposureOffset.
fn baseline_exposure(d: &[u8], ifd0: &Ifd) -> Option<f64> {
    let base = ifd0.float(d, tag::BASELINE_EXPOSURE);
    let off = ifd0
        .float(d, tag::BASELINE_EXPOSURE_OFFSET)
        .filter(|v| v.abs() <= 16.0);
    match (base, off) {
        (None, None) => None,
        (b, o) => Some(b.unwrap_or(0.0) + o.unwrap_or(0.0)),
    }
}

/// The camera profile tables of IFD 0: (HueSatMap1/2, LookTable, malformed). A table whose
/// tags are present but inconsistent (dimensions, count, values) marks the profile malformed.
#[allow(clippy::type_complexity)]
fn dng_tables(
    d: &[u8],
    ifd0: &Ifd,
) -> (
    [Option<crate::profile::HsvTable>; 2],
    Option<crate::profile::HsvTable>,
    bool,
) {
    use crate::profile::HsvTable;
    let mut bad = false;
    let mut table = |dims: u16, data: u16, enc: u16| -> Option<HsvTable> {
        let e = ifd0.get(data)?;
        let dims = ifd0.uints(d, dims, 4);
        // Bound the read by the declared dimensions before touching the data.
        let n = dims
            .iter()
            .take(3)
            .try_fold(3usize, |a, &v| a.checked_mul(v as usize));
        let t = n
            .filter(|&n| n <= 3 << 16 && e.count as usize == n)
            .and_then(|n| {
                let enc = ifd0.uint(d, enc).unwrap_or(0);
                HsvTable::new(&dims, &e.floats(d, n), enc)
            });
        bad |= t.is_none();
        t
    };
    let hsm = [
        table(
            tag::PROFILE_HUE_SAT_MAP_DIMS,
            tag::PROFILE_HUE_SAT_MAP_DATA1,
            tag::PROFILE_HUE_SAT_MAP_ENCODING,
        ),
        table(
            tag::PROFILE_HUE_SAT_MAP_DIMS,
            tag::PROFILE_HUE_SAT_MAP_DATA2,
            tag::PROFILE_HUE_SAT_MAP_ENCODING,
        ),
    ];
    let look = table(
        tag::PROFILE_LOOK_TABLE_DIMS,
        tag::PROFILE_LOOK_TABLE_DATA,
        tag::PROFILE_LOOK_TABLE_ENCODING,
    );
    (hsm, look, bad)
}

/// DNG-style colour tags (ColorMatrix, ForwardMatrix, AnalogBalance, AsShotNeutral, profile
/// tables) of IFD 0 and the raw IFD; also used by Hasselblad files, which carry them.
pub(crate) fn dng_color(d: &[u8], ifd0: &Ifd, raw: &Ifd) -> DngColor {
    let illum = [
        ifd0.uint(d, tag::CALIBRATION_ILLUMINANT1).unwrap_or(0),
        ifd0.uint(d, tag::CALIBRATION_ILLUMINANT2).unwrap_or(0),
    ];
    // CameraCalibration belongs to the reference camera of its signature: use it only with a
    // profile of the same signature (both absent counts as a match).
    let use_cc = ifd0.string(d, tag::CAMERA_CALIBRATION_SIGNATURE)
        == ifd0.string(d, tag::PROFILE_CALIBRATION_SIGNATURE);
    let (hue_sat_map, look_table, tables_malformed) = dng_tables(d, ifd0);
    let adobe = |t: u16| {
        ifd0.string(d, t)
            .is_some_and(|s| s.to_ascii_lowercase().contains("adobe"))
    };
    let mut col = DngColor {
        cm: [
            dng_matrix(d, ifd0, tag::COLOR_MATRIX1, 12),
            dng_matrix(d, ifd0, tag::COLOR_MATRIX2, 12),
        ],
        fm: [
            dng_matrix(d, ifd0, tag::FORWARD_MATRIX1, 12),
            dng_matrix(d, ifd0, tag::FORWARD_MATRIX2, 12),
        ],
        cc: [
            dng_matrix(d, raw, tag::CAMERA_CALIBRATION1, 16)
                .or_else(|| dng_matrix(d, ifd0, tag::CAMERA_CALIBRATION1, 16)),
            dng_matrix(d, raw, tag::CAMERA_CALIBRATION2, 16)
                .or_else(|| dng_matrix(d, ifd0, tag::CAMERA_CALIBRATION2, 16)),
        ],
        ab: {
            let v = ifd0.floats(d, tag::ANALOG_BALANCE, 3);
            (v.len() == 3 && v.iter().all(|&x| x > 0.0)).then(|| [v[0], v[1], v[2]])
        },
        illuminant: illum,
        neutral: {
            let v = ifd0.floats(d, tag::AS_SHOT_NEUTRAL, 3);
            (v.len() == 3 && v.iter().all(|&x| x > 0.0)).then(|| [v[0], v[1], v[2]])
        },
        white_xy: {
            let v = ifd0.floats(d, tag::AS_SHOT_WHITE_XY, 2);
            (v.len() == 2 && v[0] > 0.0 && v[1] > 0.0).then(|| (v[0], v[1]))
        },
        hue_sat_map,
        look_table,
        tone_curve: crate::profile::ToneCurve::new(&ifd0.floats(d, tag::PROFILE_TONE_CURVE, 16384)),
        tables_malformed,
        needs_tables: adobe(tag::PROFILE_NAME) || adobe(tag::PROFILE_COPYRIGHT),
        // ProfileDynamicRange (UNDEFINED x 8, file byte order): version 1, dynamic range
        // (1 = HDR), hint for the maximum output value.
        hdr: ifd0
            .get(tag::PROFILE_DYNAMIC_RANGE)
            .and_then(|e| e.bytes(d).filter(|b| b.len() == 8))
            .is_some_and(|b| {
                let e = Endian { le: ifd0.le };
                e.u16(b, 0) == Some(1) && e.u16(b, 2) == Some(1)
            }),
        exposure: baseline_exposure(d, ifd0).unwrap_or(0.0),
        noise: noise_profile(d, raw)
            .or_else(|| noise_profile(d, ifd0))
            .unwrap_or_default(),
    };
    if !use_cc {
        col.cc = [None, None];
    }
    col
}

/// DNG NoiseProfile (DNG 1.3 §6, tag 51041): variance = scale·x + offset for signal x in
/// 0..1, as one pair or one pair per colour plane. Pairs with a non-positive scale or a
/// negative offset are invalid, and so is the whole tag then.
fn noise_profile(d: &[u8], ifd: &Ifd) -> Option<Vec<[f64; 2]>> {
    let v = ifd.floats(d, tag::NOISE_PROFILE, 8);
    if v.len() < 2 || !v.len().is_multiple_of(2) {
        return None;
    }
    let pairs: Vec<[f64; 2]> = v.chunks_exact(2).map(|c| [c[0], c[1]]).collect();
    pairs
        .iter()
        .all(|[s, o]| s.is_finite() && o.is_finite() && *s > 0.0 && *o >= 0.0)
        .then_some(pairs)
}

/// Most samples per photosite a DNG CFA may carry (added up on decode).
const MAX_CFA_SAMPLES: u32 = 16;

/// A four-plane DNG's colour from its own tags: the plane colours (`CFAPlaneColor`, distinct
/// and defined by the DNG specification), the 4 x 3 `ColorMatrix` of the daylight
/// calibration (else the other one), and `AsShotNeutral` (four values) as multipliers.
fn dng_four_color(d: &[u8], ifd0: &Ifd, planes: &[u8]) -> Option<crate::cfagen::FourColor> {
    let colors: [u8; 4] = planes.try_into().ok()?;
    if colors.iter().any(|&c| c > 6) || (1..4).any(|i| colors[..i].contains(&colors[i])) {
        return None;
    }
    // EXIF light sources of daylight-like illuminants (daylight, fine weather, flash, standard
    // B and C, D55, D65, D75, D50).
    let daylight = |t: u16| {
        ifd0.uint(d, t)
            .is_some_and(|v| matches!(v, 1 | 4 | 9 | 10 | 18 | 19 | 20 | 21 | 22 | 23))
    };
    let matrix = |t: u16| {
        let v = ifd0.floats(d, t, 12);
        (v.len() == 12 && v.iter().all(|x| x.is_finite())).then_some(v)
    };
    let m = match (matrix(tag::COLOR_MATRIX1), matrix(tag::COLOR_MATRIX2)) {
        (Some(a), Some(b)) => {
            if daylight(tag::CALIBRATION_ILLUMINANT1) && !daylight(tag::CALIBRATION_ILLUMINANT2) {
                a
            } else {
                b
            }
        }
        (a, b) => a.or(b)?,
    };
    let n = ifd0.floats(d, tag::AS_SHOT_NEUTRAL, 4);
    let mul = (n.len() == 4 && n.iter().all(|&v| v.is_finite() && v > 0.0))
        .then(|| std::array::from_fn(|i| (1.0 / n[i]) as f32));
    Some(crate::cfagen::FourColor {
        colors,
        cam_xyz: std::array::from_fn(|i| std::array::from_fn(|j| m[i * 3 + j])),
        mul,
    })
}

fn dng_sensor(d: &[u8], w: &Walk) -> Option<Sensor> {
    let ifd0 = w.ifd0()?;
    let raw = w
        .image_ifds()
        .filter(|i| i.uint(d, tag::NEW_SUBFILE_TYPE).unwrap_or(0) & 1 == 0)
        .filter(|i| matches!(i.uint(d, tag::PHOTOMETRIC), Some(32803 | 34892)))
        .max_by_key(|i| {
            let (a, b) = dims(d, i);
            a as u64 * b as u64
        })?;
    let (width, height) = dims(d, raw);
    let bits = bps(d, raw);
    let photometric = raw.uint(d, tag::PHOTOMETRIC).unwrap_or(0);
    let cpp = raw.uint(d, tag::SAMPLES_PER_PIXEL).unwrap_or(1);
    let compression = raw.uint(d, tag::COMPRESSION).unwrap_or(1);
    let comp = match compression {
        1 => SensorCompression::Uncompressed,
        7 => SensorCompression::LosslessJpeg,
        34892 => SensorCompression::LossyJpeg,
        c => SensorCompression::Other(c),
    };
    let mut s = Sensor::new(width, height, bits, comp);
    s.cpp = cpp;
    if photometric == 32803 {
        // CFA pattern, mapped through CFAPlaneColor.
        let planes = raw
            .get(tag::CFA_PLANE_COLOR)
            .and_then(|e| e.bytes(d))
            .map(<[u8]>::to_vec)
            .unwrap_or(vec![0, 1, 2]);
        s.cfa = cfa_from_tags(d, &[raw]).map(|c| {
            let colors = c
                .colors
                .iter()
                .map(|&i| planes.get(i as usize).copied().unwrap_or(9))
                .collect();
            Cfa { colors, ..c }
        });
        // Several samples per photosite (the parts of a dual- or quad-pixel sensor): added up
        // on decode.
        if !(1..=MAX_CFA_SAMPLES).contains(&cpp) {
            return Some(s.unsupported("DNG CFA with more than 16 samples per pixel"));
        }
        let layout = raw.uint(d, tag::CFA_LAYOUT).unwrap_or(1);
        if !(1..=9).contains(&layout) {
            return Some(s.unsupported("DNG CFA layout the DNG specification does not define"));
        }
        // Staggered layouts: demosaiced on decode by the photosites' true positions.
        let scale = raw.floats(d, tag::DEFAULT_SCALE, 2);
        let aspect = match scale[..] {
            [h, v] if h > 0.0 && v > 0.0 => (h / v) as f32,
            _ => 1.0,
        };
        let aa = raw.uints(d, tag::ACTIVE_AREA, 4);
        let (top, left) = if aa.len() == 4 {
            (aa[0], aa[1])
        } else {
            (0, 0)
        };
        s.stagger = crate::cfagen::Stagger::new(layout, top, left, aspect);
        // Four colour planes with the file's own 4 x 3 ColorMatrix: converted by it.
        if planes.len() == 4 {
            s.four = dng_four_color(d, ifd0, &planes).map(Box::new);
        }
    } else if cpp != 3 && cpp != 1 {
        return Some(s.unsupported("DNG LinearRaw with other than 1 or 3 samples"));
    }
    // LinearRaw with one sample: a monochrome sensor (Leica Monochrom, Pentax Monochrome) or a
    // film scanner's grey channel, shown as grey.
    let mono = photometric != 32803 && cpp == 1;

    // Crop: ActiveArea, then DefaultCrop inside it.
    let aa = raw.uints(d, tag::ACTIVE_AREA, 4);
    let (mut top, mut left, mut bottom, mut right) = (0, 0, height, width);
    if aa.len() == 4 && aa[0] < aa[2] && aa[1] < aa[3] && aa[2] <= height && aa[3] <= width {
        (top, left, bottom, right) = (aa[0], aa[1], aa[2], aa[3]);
    }
    let mut crop = Rect {
        x: left,
        y: top,
        w: right - left,
        h: bottom - top,
    };
    let co = raw.floats(d, tag::DEFAULT_CROP_ORIGIN, 2);
    let cs = raw.floats(d, tag::DEFAULT_CROP_SIZE, 2);
    if co.len() == 2 && cs.len() == 2 {
        let (x, y) = (co[0].max(0.0) as u32, co[1].max(0.0) as u32);
        let (cw, ch) = (cs[0].max(0.0) as u32, cs[1].max(0.0) as u32);
        if cw > 0
            && ch > 0
            && x.checked_add(cw).is_some_and(|e| e <= crop.w)
            && y.checked_add(ch).is_some_and(|e| e <= crop.h)
        {
            crop = Rect {
                x: crop.x + x,
                y: crop.y + y,
                w: cw,
                h: ch,
            };
        }
    }
    s.crop = crop;
    s.default_scale = crate::scale::default_scale(&raw.floats(d, tag::DEFAULT_SCALE, 2));
    // OpcodeList1..3; TrimBounds narrows the crop.
    let active = Rect {
        x: left,
        y: top,
        w: right - left,
        h: bottom - top,
    };
    let lists = crate::opcode::TAGS.map(|t| raw.get(t).and_then(|e| e.bytes(d)));
    if let Some(mut ops) = crate::DngOpcodes::parse(lists, width, height, active, cpp) {
        ops.untrimmed_crop = crop;
        if let Some(t) = ops.trim {
            let c = crate::opcode::intersect(crop, t);
            if c.w > 0 && c.h > 0 {
                s.crop = c;
            }
        }
        s.opcodes = Some(std::sync::Arc::new(ops));
    }
    // CFAPattern and BlackLevel repeat from the ActiveArea's top-left corner; `Cfa` is anchored
    // at the stored image's.
    if let Some(c) = &s.cfa {
        s.cfa = Some(c.shifted(
            (c.width - left % c.width) % c.width,
            (c.height - top % c.height) % c.height,
        ));
    }

    // Black / white / linearization.
    let rd = raw.uints(d, tag::BLACK_LEVEL_REPEAT_DIM, 2);
    let (br, bc) = if rd.len() == 2 && rd[0] > 0 && rd[1] > 0 {
        (rd[0] as usize, rd[1] as usize)
    } else {
        (1, 1)
    };
    let bl = raw.floats(d, tag::BLACK_LEVEL, 4096);
    let n = br.saturating_mul(bc).saturating_mul(cpp as usize);
    let mut black = [0f32; 4];
    if !bl.is_empty() {
        if bl.len() >= n {
            if cpp == 1 && (br <= 2 && bc <= 2) {
                for (p, b) in black.iter_mut().enumerate() {
                    let y = (p / 2 + br - top as usize % br) % br;
                    let x = (p % 2 + bc - left as usize % bc) % bc;
                    *b = bl[y * bc + x] as f32;
                }
            } else {
                for (c, b) in black.iter_mut().enumerate().take(cpp.min(4) as usize) {
                    let vals: Vec<f64> = (0..br * bc)
                        .map(|i| bl[i * cpp as usize + c.min(cpp as usize - 1)])
                        .collect();
                    *b = (vals.iter().sum::<f64>() / vals.len() as f64) as f32;
                }
                if cpp == 1 {
                    black = [black[0]; 4];
                }
            }
        } else {
            black = [bl[0] as f32; 4];
        }
    }
    // Several samples per photosite, added up on decode: one level per sample, the same at
    // every position (their mean; the sum scales it).
    if photometric == 32803 && cpp > 1 && bl.len() >= n && n > 0 {
        black = [(bl[..n].iter().sum::<f64>() / n as f64) as f32; 4];
    }
    s.black = Black::Levels(black);
    if !bl.is_empty() {
        let full = bl.len() >= n && n > 0;
        let dh = raw.floats(d, tag::BLACK_LEVEL_DELTA_H, 1 << 17);
        let dv = raw.floats(d, tag::BLACK_LEVEL_DELTA_V, 1 << 17);
        s.black_area = Some(Box::new(crate::sensor::BlackLevels {
            repeat: if full { (br as u32, bc as u32) } else { (1, 1) },
            cpp: cpp.max(1),
            levels: if full {
                bl[..n].iter().map(|&v| v as f32).collect()
            } else {
                vec![bl[0] as f32; cpp.max(1) as usize]
            },
            origin: (left, top),
            per_column: if dh.len() as u32 == right - left {
                dh.iter().map(|&v| v as f32).collect()
            } else {
                Vec::new()
            },
            per_row: if dv.len() as u32 == bottom - top {
                dv.iter().map(|&v| v as f32).collect()
            } else {
                Vec::new()
            },
        }));
    }
    let lin = raw.uints(d, tag::LINEARIZATION_TABLE, 65536);
    if lin.len() >= 2 {
        s.linearization = Some(lin.iter().map(|&v| v.min(65535) as u16).collect());
    }
    let white = raw.uint(d, tag::WHITE_LEVEL).filter(|&v| v > 0);
    s.white = Some(white.unwrap_or_else(|| {
        s.linearization
            .as_ref()
            .map_or((1u32 << bits.clamp(1, 16)) - 1, |l| {
                l.iter().copied().max().unwrap_or(65535) as u32
            })
    }) as f32);

    let col = dng_color(d, ifd0, raw);
    // Colour planes other than red, green, blue in that order: the colour tags describe those
    // planes (n x 3 matrices in plane order), not the RGB the decode makes of them. Four planes
    // use `four` when the file has a matrix for them; others the generic matrix.
    let three_planes = photometric != 32803
        || raw
            .get(tag::CFA_PLANE_COLOR)
            .and_then(|e| e.bytes(d))
            .is_none_or(|p| p == [0, 1, 2]);
    s.color = if mono {
        s.wb = Some([1.0; 3]);
        ColorSource::Identity
    } else if !three_planes {
        ColorSource::Generic
    } else if col.cm[0].is_some() || col.cm[1].is_some() {
        ColorSource::Dng(Box::new(col))
    } else if cpp == 3 && photometric != 32803 {
        ColorSource::Identity
    } else {
        ColorSource::Generic
    };

    // Storage.
    let le = raw.le;
    let float = raw.uint(d, tag::SAMPLE_FORMAT) == Some(3);
    let tile = |kind| {
        // Floating point: the white level (1.0 unless given) maps to 65535.
        let white = raw
            .floats(d, tag::WHITE_LEVEL, 1)
            .first()
            .copied()
            .filter(|&v| v > 0.0 && v.is_finite())
            .unwrap_or(1.0);
        crate::dngtile::DngTile {
            kind,
            bits: bits.min(255) as u8,
            float,
            predictor: raw.uint(d, tag::PREDICTOR).unwrap_or(1).min(65535) as u16,
            le,
            scale: if float {
                (65535.0 / white) as f32
            } else {
                1.0f32
            }
            .to_bits(),
        }
    };
    if float {
        let t = tile(crate::dngtile::Kind::Raw);
        let scale = f32::from_bits(t.scale);
        if let Black::Levels(b) = &mut s.black {
            *b = b.map(|v| v * scale);
        }
        s.white = Some(65535.0);
        s.linearization = None;
        s.bits = 16;
    }
    // Compression 7 normally holds lossless JPEG; some cinema cameras (Blackmagic) store
    // DCT (lossy) frames under it instead.
    let dct7 = compression == 7
        && blocks(d, raw, width, height)
            .and_then(|(b, _, _)| b.first().cloned())
            .is_some_and(|r| crate::dngtile::is_dct_jpeg(&d[r]));
    let codec = match compression {
        1 if float => Some(crate::dngtile::Kind::Raw),
        7 if dct7 => Some(crate::dngtile::Kind::Jpeg),
        8 => Some(crate::dngtile::Kind::Deflate),
        34892 => Some(crate::dngtile::Kind::Jpeg),
        52546 => Some(crate::dngtile::Kind::Jxl),
        _ => None,
    };
    if let Some(kind) = codec {
        let t = tile(kind);
        if !t.valid() {
            return Some(s.unsupported("DNG: unsupported sample format or predictor"));
        }
        match blocks(d, raw, width, height) {
            // A block is never much wider than the image.
            Some((b, tw, th)) if tw <= width.saturating_add(1024) && th > 0 => {
                s.storage = Storage::Blocks {
                    blocks: b,
                    tile_w: tw,
                    tile_h: th.min(height.saturating_add(1024)),
                    codec: Codec::Dng(t),
                };
            }
            _ => return Some(s.unsupported("DNG: bad strip/tile layout")),
        }
    } else if float {
        return Some(s.unsupported("DNG: floating point samples with this compression"));
    }
    match compression {
        _ if codec.is_some() => {}
        1 => match uncompressed_storage(d, raw, width, height, cpp, bits, le, true) {
            Some(st) => s.storage = st,
            None => return Some(s.unsupported("DNG: bad strip/tile layout")),
        },
        7 => match blocks(d, raw, width, height) {
            Some((b, tw, th)) => {
                s.storage = Storage::Blocks {
                    blocks: b,
                    tile_w: tw,
                    tile_h: th,
                    codec: Codec::Lj92,
                }
            }
            None => return Some(s.unsupported("DNG: bad strip/tile layout")),
        },
        // GoPro GPR: one VC-5 bitstream for the whole image, decoded to 12 or 14 bits
        // (by the white level) in the Bayer order of the CFA pattern.
        9 => match blocks(d, raw, width, height) {
            Some((b, tw, th)) if b.len() == 1 && tw >= width && th >= height && cpp == 1 => {
                let white = raw.uint(d, tag::WHITE_LEVEL).unwrap_or(4095);
                let gbrg = s.cfa.as_ref().is_some_and(|c| c.colors[..] != [0, 1, 1, 2]);
                s.bits = if white > 4095 { 14 } else { 12 };
                s.storage = Storage::Vendor {
                    range: b[0].clone(),
                    codec: crate::vendor::VendorCodec::Vc5(crate::vc5::Params {
                        bits: s.bits,
                        gbrg,
                    }),
                };
            }
            Some((b, tw, th)) if cpp == 1 && !b.is_empty() => {
                // Several tiles (or strips), each its own VC-5 bitstream; missing ones are
                // left empty and flagged as truncated.
                let white = raw.uint(d, tag::WHITE_LEVEL).unwrap_or(4095);
                let gbrg = s.cfa.as_ref().is_some_and(|c| c.colors[..] != [0, 1, 1, 2]);
                s.bits = if white > 4095 { 14 } else { 12 };
                s.storage = Storage::Vendor {
                    range: 0..d.len(),
                    codec: crate::vendor::VendorCodec::Vc5Tiles {
                        p: crate::vc5::Params { bits: s.bits, gbrg },
                        tiles: b,
                        tw: tw as usize,
                        th: th as usize,
                    },
                };
            }
            _ => return Some(s.unsupported("GoPro GPR: the VC-5 data does not cover the image")),
        },
        _ => return Some(s.unsupported("DNG compression")),
    }
    if !(1..=16).contains(&s.bits) {
        return Some(s.unsupported("DNG bit depth"));
    }
    // 2x2 Bayer, X-Trans and other RGB patterns the X-Trans develop handles.
    match &s.cfa {
        _ if photometric != 32803 => {}
        None => return Some(s.unsupported("DNG CFA without a valid CFAPattern")),
        Some(c) if s.stagger.is_some() && crate::cfagen::convertible(c) => {}
        Some(c) if s.stagger.is_none() && crate::cfagen::supported(c) => {}
        Some(c) if c.colors.iter().any(|&v| v > BLUE) => {
            return Some(s.unsupported("DNG CFA with colours other than red, green and blue"));
        }
        Some(c) if !c.developable() => {
            return Some(
                s.unsupported("DNG CFA pattern without red, green and blue in every 3x3 window"),
            );
        }
        Some(_) => {}
    }
    Some(s)
}

// ---------------------------------------------------------------------------------------------
// Canon CR2

fn cr2_sensor(d: &[u8], w: &Walk, canon: makernote::CanonInfo) -> Option<Sensor> {
    let raw = w.main.get(3)?;
    let off = raw.uint(d, tag::STRIP_OFFSETS)? as usize;
    let len = raw.uint(d, tag::STRIP_BYTE_COUNTS)? as usize;
    let range = clamp_range(d, off, len);
    let h = crate::ljpeg::read_header(&d[range.clone()]).ok()?;
    let slices = raw.uints(d, tag::CR2_SLICE, 3);
    let slices = (slices.len() == 3 && slices[0] > 0 || slices.len() == 3 && slices[2] > 0)
        .then(|| [slices[0], slices[1], slices[2]]);
    let total = h.width as u64 * h.components as u64 * h.height as u64;
    let raw_w = match slices {
        Some([n, sw, lw]) => n as u64 * sw as u64 + lw as u64,
        None => h.width as u64 * h.components as u64,
    };
    if raw_w == 0 || raw_w > 65535 {
        return None;
    }
    let raw_h = total / raw_w;
    let mut s = Sensor::new(
        raw_w as u32,
        raw_h as u32,
        h.precision as u32,
        SensorCompression::LosslessJpeg,
    );
    s.storage = Storage::Cr2 { range, slices };
    if h.subsampled {
        return Some(cr2_ycc(&h, s, &canon));
    }
    // CR2CFAPattern in the raw IFD; most bodies are RGGB but e.g. the 5D Mark II and 1300D
    // are GBRG at the sensor origin.
    let cfa = Cfa::bayer(cr2_cfa_name(raw.uint(d, 0xc5e0)))?;
    if let Some([sw, sh, l, t, r, b]) = canon.sensor
        && sw <= s.width.saturating_add(64)
        && sh <= s.height.saturating_add(64)
        && r < s.width
        && b < s.height
    {
        // Odd crop origins shift the pattern; keep the crop even so the visible area starts on
        // a full 2x2 cell like other converters.
        let (x, y) = ((l + 1) & !1, (t + 1) & !1);
        let (cw, ch) = ((r + 1 - x) & !1, (b + 1 - y) & !1);
        s.crop = Rect { x, y, w: cw, h: ch };
        if l >= 20 {
            // Columns just left of the border can still be lit (the 1300D's image reaches about
            // ten columns into its border of 152, which raised the black by 50; the 5D Mark
            // III's 18 into its 140, +9): stop 32 short where the border is wide enough.
            let short = if l >= 64 { 32 } else { 16 };
            s.black = Black::Masked {
                area: Rect {
                    x: 2,
                    y: t,
                    w: l.saturating_sub(short + 2) & !1,
                    h: (b + 1 - t) & !1,
                },
                also: None,
                fallback: [0.0; 4],
            };
        }
    }
    // ColorData levels, where the body records them, beat the masked border: on some
    // PowerShots (G7 X Mark II) the border right of the crop edge is not optically black.
    if let Some(b) = canon.black {
        s.black = Black::Levels(b.map(|v| v as f32));
    }
    s.white_nominal = true;
    s.cfa = Some(cfa);
    s.wb = canon.wb_rggb.and_then(wb_from_rggb);
    Some(s)
}

/// Canon sRAW / mRAW: demosaiced in camera, stored as luma/chroma.
fn cr2_ycc(h: &crate::ljpeg::Lj92Header, mut s: Sensor, canon: &makernote::CanonInfo) -> Sensor {
    let Storage::Cr2 { range, slices } = std::mem::replace(&mut s.storage, Storage::None) else {
        return s;
    };
    let Some(lay) = crate::ljpeg::subsampled_layout(h) else {
        return s.unsupported("Canon sRAW/mRAW: subsampling layout");
    };
    // The slices cover the MCU sample stream: their total width is one MCU line.
    let line = lay.cols as u64 * lay.clrs() as u64;
    if let Some([n, sw, lw]) = slices
        && n as u64 * sw as u64 + lw as u64 != line
    {
        return s.unsupported("Canon sRAW/mRAW: slices do not match the frame");
    }
    s.width = h.width;
    s.height = h.height;
    s.crop = Rect {
        x: 0,
        y: 0,
        w: h.width,
        h: h.height,
    };
    s.cpp = 3;
    s.cfa = None;
    s.bits = 16;
    s.black = Black::Levels([0.0; 4]);
    // The camera white-balances the luma/chroma to its sRAW reference; these levels undo that
    // (each in 1/1024, scaled by the reference green).
    let mul = canon
        .sraw_levels
        .map_or([1024; 3], |l| l.map(|v| (v * l[1]) >> 10));
    // Luma saturates at half the sample range (the other half is chroma headroom); the
    // channel with the largest gain clips last.
    let luma_white = (1u32 << (h.precision.clamp(2, 16) - 1)) - 1;
    s.white = Some(((luma_white * mul.iter().copied().max().unwrap_or(1024)) >> 10) as f32);
    s.wb = canon.wb_rggb.and_then(wb_from_rggb);
    let id = canon.model_id.unwrap_or(0);
    let conv = match id {
        0x80000218 | 0x80000250 | 0x80000261 | 0x80000281 | 0x80000287 => {
            let firmware_1_0_0 = canon
                .firmware
                .as_deref()
                .is_some_and(|f| f.contains("1.0.0"));
            let hue = if id >= 0x80000281 || id == 0x80000218 && firmware_1_0_0 {
                (lay.clrs() as i32 - 3) << 1
            } else {
                (lay.clrs() as i32 - 2) << 2
            };
            crate::ycc::Conv::CanonMatrix { hue }
        }
        id if id < 0x80000218 => crate::ycc::Conv::CanonEarly,
        _ => crate::ycc::Conv::CanonPlain,
    };
    s.storage = Storage::Cr2Ycc {
        range,
        slices,
        conv,
        mul,
    };
    s
}

/// Canon's CR2CFAPattern value (tag 0xc5e0 in the raw IFD) as a 2x2 pattern name.
fn cr2_cfa_name(v: Option<u32>) -> &'static str {
    match v {
        Some(2) => "BGGR",
        Some(3) => "GBRG",
        Some(4) => "GRBG",
        _ => "RGGB",
    }
}

// ---------------------------------------------------------------------------------------------
// Nikon NEF / NRW

fn nef_sensor(d: &[u8], w: &Walk, nikon: makernote::NikonInfo, model: &str) -> Option<Sensor> {
    let raw = largest_raw_ifd(d, w)?;
    let (width, height) = dims(d, raw);
    let bits = bps(d, raw);
    let compression = raw.uint(d, tag::COMPRESSION).unwrap_or(1);
    let comp = match compression {
        34713 => SensorCompression::NikonHuffman,
        1 => SensorCompression::Uncompressed,
        c => SensorCompression::Other(c),
    };
    let mut s = Sensor::new(width, height, bits, comp);
    let mut ifds = vec![raw];
    if let Some(e) = &w.exif {
        ifds.push(e);
    }
    s.cfa = cfa_from_tags(d, &ifds).or_else(|| Cfa::bayer("RGGB"));
    if let Some(b) = nikon.black {
        let b = b.map(|v| v as f32);
        // Some bodies (all Nikon Z) store 14-bit black levels for 12-bit files: 1008 where
        // the 12-bit level is 252. Real 12-bit blacks stay well under 512.
        let scale = if bits == 12 && b.iter().any(|&v| v >= 512.0) {
            0.25
        } else {
            1.0
        };
        s.black = Black::Levels(black_from_rggb(s.cfa.as_ref(), b.map(|v| v * scale)));
    }
    s.wb = nikon.wb_rb.map(|(r, b)| [r as f32, 1.0, b as f32]);
    let offs = raw.uints(d, tag::STRIP_OFFSETS, 2);
    let lens = raw.uints(d, tag::STRIP_BYTE_COUNTS, 2);
    match compression {
        34713 => {
            // High Efficiency (NEFCompression 13 = HE, 14 = HE*) is an intoPIX TicoRAW
            // codestream (JPEG XS with changes of its own): no linearization table, black and
            // white as the lossless files of these bodies.
            let he = matches!(nikon.compression, Some(n) if n > 4 && n != 7);
            let start = offs.first().copied().unwrap_or(0) as usize;
            if he || d.get(start..).is_some_and(crate::nikon_he::probe) {
                if offs.len() != 1 || !d.get(start..).is_some_and(crate::nikon_he::probe) {
                    return Some(s.unsupported(
                        "Nikon High Efficiency raw: this variant of the codec is not supported",
                    ));
                }
                // The curve, black and white were fitted on 14-bit, one-sample-per-pixel
                // files only; anything else would decode with silently wrong levels.
                let cpp = raw.uint(d, tag::SAMPLES_PER_PIXEL).unwrap_or(1);
                if cpp != 1 || !matches!(bits, 0 | 14) {
                    return Some(s.unsupported(
                        "Nikon High Efficiency raw: only 14-bit files are supported so far",
                    ));
                }
                s.compression = SensorCompression::NikonHighEfficiency {
                    star: nikon.compression == Some(14),
                };
                // Adobe DNG Converter's white for every body tested; the curve clips at
                // 16383 above it.
                s.white = Some(15892.0);
                s.storage = Storage::Vendor {
                    range: clamp_range(
                        d,
                        start,
                        lens.first().copied().unwrap_or(u32::MAX) as usize,
                    ),
                    codec: crate::vendor::VendorCodec::NikonHe,
                };
                return Some(s);
            }
            let Some(table) = nikon.lin_table else {
                return Some(s.unsupported("NEF: missing NEFLinearizationTable"));
            };
            if offs.len() != 1 {
                return Some(s.unsupported("NEF: compressed data in several strips"));
            }
            s.storage = Storage::Nikon {
                range: clamp_range(
                    d,
                    offs[0] as usize,
                    lens.first().copied().unwrap_or(u32::MAX) as usize,
                ),
                table,
                le: w.maker.as_ref().map_or(raw.le, |m| m.le),
            };
        }
        1 if coolpix_p_packed(model, width, height, bits, lens.first().copied()) => {
            // The white is the 12-bit full scale, as LibRaw; the develop finds the clip (3840
            // on the corpus file).
            s.white_nominal = true;
            // Coolpix P NRW (P7000, P7100): 12 bits MSB-first over little-endian 32-bit
            // words, rows back to back (checked bit-exact against LibRaw on a P7000).
            s.storage = Storage::Blocks {
                blocks: vec![clamp_range(
                    d,
                    offs.first().copied().unwrap_or(0) as usize,
                    lens.first().copied().unwrap_or(0) as usize,
                )],
                tile_w: width,
                tile_h: height,
                codec: Codec::Raw {
                    packing: Packing::Msb32Le(12),
                    row_bytes: 0,
                },
            };
        }
        1 => match uncompressed_storage(d, raw, width, height, 1, bits, raw.le, false) {
            Some(st) => {
                s.storage = st;
                // Coolpix bodies ("E5700") store the even rows first, then the odd rows.
                let m = model.trim();
                let m = m.strip_prefix("NIKON").map_or(m, str::trim_start);
                s.fields = m.starts_with('E') && m[1..].starts_with(|c: char| c.is_ascii_digit());
                // Their four-colour CFAPattern starts one row late (checked against LibRaw on
                // an E5700).
                if s.fields
                    && s.cfa
                        .as_ref()
                        .is_some_and(|c| c.colors.iter().any(|&v| v > BLUE))
                {
                    s.cfa = s.cfa.as_ref().map(|c| c.shifted(0, 1));
                }
            }
            None => return Some(s.unsupported("NEF: bad strip layout")),
        },
        _ => return Some(s.unsupported("NEF compression")),
    }
    Some(s)
}

/// Whether an uncompressed Coolpix P NRW packs its 12-bit samples over little-endian 32-bit
/// words. The 4032-wide bodies (P6000) do not; rows must end on a word, as the packing
/// pads each row to one.
fn coolpix_p_packed(model: &str, w: u32, h: u32, bits: u32, bytes: Option<u32>) -> bool {
    let m = model.trim();
    let m = m.strip_prefix("NIKON").map_or(m, str::trim_start);
    let row_bits = u64::from(w) * 12;
    m.starts_with("COOLPIX P")
        && bits == 12
        && w != 4032
        && row_bits.is_multiple_of(32)
        && bytes.is_some_and(|b| u64::from(b) >= row_bits / 8 * u64::from(h))
}

// ---------------------------------------------------------------------------------------------
// Sony ARW / SR2

/// The Sony DSLR-A100 layout (ARW 1.0): IFD 0 describes the preview, its SubIFDs entry holds
/// the offset of the sensor data instead of an IFD, and DNGPrivateData points to a Minolta-style
/// block list ("\0MRI" + byte order; blocks "\0PRD" with the sensor size and "\0WBG" with
/// the white balance).
struct A100 {
    data: usize,
    width: u32,
    height: u32,
    wb: Option<[f32; 3]>,
}

fn a100(d: &[u8], ifd0: &Ifd) -> Option<A100> {
    let make = ifd0.string(d, tag::MAKE)?;
    let model = ifd0.string(d, tag::MODEL)?;
    if !make.to_ascii_uppercase().starts_with("SONY") || model.trim() != "DSLR-A100" {
        return None;
    }
    let data = ifd0.uints(d, tag::SUB_IFDS, 1).first().copied()? as usize;
    // A LONG, or UNDEFINED x 4 holding one in the file's byte order.
    let e = ifd0.get(tag::DNG_PRIVATE_DATA)?;
    let mri = if e.count == 4 && e.byte_len() == 4 {
        Endian { le: e.le }.u32(d, e.pos)?
    } else {
        e.uint(d, 0)?
    } as usize;
    let head = d.get(mri..mri.checked_add(8)?)?;
    if &head[..3] != b"\0MR" {
        return None;
    }
    let en = Endian {
        le: head[3] == b'I',
    };
    let end = mri
        .checked_add(8)?
        .checked_add(en.u32(d, mri + 4)? as usize)?;
    let end = end.min(d.len());
    let (mut size, mut wb) = (None, None);
    let mut p = mri + 8;
    while p.checked_add(8).is_some_and(|q| q <= end) {
        let len = en.u32(d, p + 4)? as usize;
        match &d[p..p + 4] {
            b"\0PRD" => {
                size = Some((en.u16(d, p + 16)? as u32, en.u16(d, p + 18)? as u32));
            }
            b"\0WBG" => {
                let v: Option<Vec<f64>> = (0..4)
                    .map(|i| en.u16(d, p + 12 + 2 * i).map(f64::from))
                    .collect();
                wb = v.and_then(|v| wb_from_rggb([v[0], v[1], v[2], v[3]]));
            }
            _ => {}
        }
        p = p.checked_add(8)?.checked_add(len)?;
    }
    let (height, width) = size?;
    // The compressed variant (3880 wide) codes one column more than the PRD states.
    (width == 3880 && height > 1).then_some(A100 {
        data,
        width: width + 1,
        height,
        wb,
    })
}

fn arw_sensor(d: &[u8], w: &Walk) -> Option<Sensor> {
    let ifd0 = w.ifd0()?;
    if let Some(a) = a100(d, ifd0) {
        // The last coded (odd) row is not part of the image.
        let mut s = Sensor::new(a.width, a.height - 1, 12, SensorCompression::Other(32767));
        s.cfa = Cfa::bayer("GRBG");
        s.wb = a.wb;
        s.storage = Storage::SonyArw1 {
            range: a.data.min(d.len())..d.len(),
            rows: a.height,
        };
        return Some(s);
    }
    let raw = largest_raw_ifd(d, w)?;
    let (width, height) = dims(d, raw);
    let bits = bps(d, raw);
    let compression = raw.uint(d, tag::COMPRESSION).unwrap_or(1);
    let offs = raw.uints(d, tag::STRIP_OFFSETS, 2);
    let lens = raw.uints(d, tag::STRIP_BYTE_COUNTS, 2);
    let bytes = lens.first().copied().unwrap_or(0) as u64;
    let px = width as u64 * height as u64;
    let arw2 = compression == 32767 && bytes == px;
    // Lossless compressed RAW (A1 and later): LJ92 tiles.
    let lossless = compression == 7;
    let comp = if lossless {
        SensorCompression::LosslessJpeg
    } else if arw2 {
        SensorCompression::SonyArw2
    } else if compression == 1 || px.checked_mul(2) == Some(bytes) {
        SensorCompression::Uncompressed
    } else {
        SensorCompression::Other(compression)
    };
    let mut s = Sensor::new(width, height, if arw2 { 14 } else { bits }, comp);
    s.cfa = cfa_from_tags(d, &[raw]).or_else(|| Cfa::bayer("RGGB"));
    let r1 = ifd0
        .string(d, tag::MODEL)
        .is_some_and(|m| m.trim().starts_with("DSC-R1"));
    s.lens = makernote::sony_lens(d, raw).map(std::sync::Arc::new);
    let sr2 = makernote::sony_sr2(d, ifd0);
    if let Some(sr2) = &sr2 {
        if let Some(b) = sr2.black {
            s.black = Black::Levels(black_from_rggb(s.cfa.as_ref(), b.map(|v| v as f32)));
        }
        s.wb = sr2.wb_rggb.and_then(wb_from_rggb);
        if let Some(wl) = sr2.white {
            s.white = Some(wl as f32);
        }
    }
    // The raw IFD's WhiteLevel, where newer files have it, is where the data clips; the SR2
    // WhiteLevel sits lower (15360 against 16380 on the RX100 VII, whose samples run evenly
    // up to about 16300) and clipped real highlight detail, as LibRaw does not.
    if let Some(wl) = raw
        .uint(d, tag::WHITE_LEVEL)
        .filter(|&v| v > 0 && v < 1 << s.bits.clamp(1, 16))
        .filter(|&v| s.white.is_none_or(|w| v as f32 > w))
    {
        s.white = Some(wl as f32);
    }
    if s.black_is_zero()
        && let b = raw.uints(d, 0x7310, 4)
        && b.len() == 4
    {
        // BlackLevel is also in the raw IFD of newer files.
        s.black = Black::Levels(black_from_rggb(
            s.cfa.as_ref(),
            [b[0], b[1], b[2], b[3]].map(|v| v as f32),
        ));
    }
    let co = raw.uints(d, tag::DEFAULT_CROP_ORIGIN, 2);
    let cs = raw.uints(d, tag::DEFAULT_CROP_SIZE, 2);
    if co.len() == 2
        && cs.len() == 2
        && co[0] as u64 + cs[0] as u64 <= width as u64
        && co[1] as u64 + cs[1] as u64 <= height as u64
        && cs[0] > 0
        && cs[1] > 0
    {
        s.crop = Rect {
            x: co[0],
            y: co[1],
            w: cs[0],
            h: cs[1],
        };
    }
    if lossless {
        return Some(arw_lossless(d, raw, s));
    }
    if offs.len() != 1 {
        return Some(s.unsupported("ARW: data in several strips"));
    }
    let range = clamp_range(d, offs[0] as usize, bytes as usize);
    if arw2 {
        // Tone curve (0x7010): four knots (14-bit, >> 2) splitting 0..4095 into segments with
        // slopes 1, 2, 4, 8, 16.
        let knots = raw.uints(d, 0x7010, 4);
        let mut sc = [0u32, 0, 0, 0, 0, 4095];
        if knots.len() == 4 {
            for (c, k) in knots.iter().enumerate() {
                sc[c + 1] = (k >> 2) & 0xfff;
            }
        }
        let mut curve: Vec<u16> = (0..0x4001u32).map(|i| i as u16).collect();
        for i in 0..5 {
            for j in sc[i] + 1..=sc[i + 1] {
                let j = j as usize;
                if j < curve.len() {
                    curve[j] = curve[j - 1].saturating_add(1 << i);
                }
            }
        }
        // Without a raw-IFD WhiteLevel the white is the 14-bit full scale (or the curve's
        // top, if lower), as LibRaw states it, not a lower SR2 WhiteLevel: the NEX-5N file
        // (SR2 15360) runs evenly through 15360 and piles up at 16404. The
        // develop finds the clip in the data.
        let top = (curve[4095] as f32).min(16383.0);
        if raw.uint(d, tag::WHITE_LEVEL).is_none_or(|v| v == 0) {
            s.white = Some(s.white.map_or(top, |w| w.max(top)));
        }
        s.storage = Storage::SonyArw2 { range, curve };
    } else if px.checked_mul(2) == Some(bytes) {
        s.bits = 14;
        // The R1 states no white: its full scale, as LibRaw (the develop finds the clip, 16368
        // on the corpus file).
        s.white_nominal |= r1;
        s.storage = Storage::Blocks {
            blocks: vec![range],
            tile_w: width,
            tile_h: height,
            codec: Codec::Raw {
                // The DSC-R1 (SR2) writes big-endian samples in a little-endian TIFF
                // (checked bit-exact against LibRaw).
                packing: if raw.le && !r1 {
                    Packing::U16Le
                } else {
                    Packing::U16Be
                },
                row_bytes: width as usize * 2,
            },
        };
    } else if px.checked_mul(12) == Some(bytes * 8) && compression != 32767 {
        s.storage = Storage::Blocks {
            blocks: vec![range],
            tile_w: width,
            tile_h: height,
            codec: Codec::Raw {
                packing: Packing::Le12,
                row_bytes: width as usize * 3 / 2,
            },
        };
    } else if compression == 32767 {
        // ARW 1.0: eight more coded rows than the image has.
        s.bits = 12;
        s.storage = Storage::SonyArw1 {
            range: offs[0] as usize..d.len(),
            rows: height.saturating_add(8),
        };
    } else {
        return Some(s.unsupported("ARW: unknown compression"));
    }
    Some(s)
}

/// Sony lossless compressed ARW: 14-bit LJ92 tiles (512x512 on the bodies seen so far), each
/// coded as a frame of half the tile's width and height with four components, one per position
/// of a 2x2 CFA block.
fn arw_lossless(d: &[u8], raw: &Ifd, mut s: Sensor) -> Sensor {
    // The raw IFD's WhiteLevel is where the data clips (16383 on 14-bit bodies, as observed in
    // the samples); the SR2 WhiteLevel (0x787f, e.g. 15360) is lower and would clip real
    // highlight detail.
    if let Some(wl) = raw
        .uint(d, tag::WHITE_LEVEL)
        .filter(|&v| v > 0 && v < 1 << s.bits.clamp(1, 16))
    {
        s.white = Some(wl as f32);
    }
    if raw.get(tag::TILE_WIDTH).is_none() {
        return s.unsupported("ARW lossless: data not tiled");
    }
    let Some((blocks, tw, th)) = blocks(d, raw, s.width, s.height) else {
        return s.unsupported("ARW lossless: bad tile layout");
    };
    if blocks.len() != s.width.div_ceil(tw) as usize * s.height.div_ceil(th) as usize {
        return s.unsupported("ARW lossless: tile count does not match the image size");
    }
    // Tiles are decoded at their declared size: one far larger than the image would size the
    // decode from the header alone.
    if tw > s.width.saturating_add(1024) || th > s.height.saturating_add(1024) {
        return s.unsupported("ARW lossless: tiles larger than the image");
    }
    // The first tile's frame header tells how its samples map to sensor pixels.
    let Some(h) = blocks
        .first()
        .and_then(|r| d.get(r.clone()))
        .and_then(|t| crate::ljpeg::read_header(t).ok())
    else {
        return s.unsupported("ARW lossless: unreadable lossless JPEG tile header");
    };
    if h.subsampled {
        // M and S sizes: demosaiced in camera, luma/chroma tiles of the full tile size.
        if crate::ljpeg::subsampled_layout(&h).is_none() || h.width != tw || h.height != th {
            return s.unsupported("ARW lossless: unknown YCbCr tile layout");
        }
        // ReferenceBlackWhite: luma spans [0, white] above the black level; chroma is centred
        // on its second pair's black.
        let refbw = raw.floats(d, 0x0214, 6);
        let level = |i: usize, default: f64| {
            refbw
                .get(i)
                .copied()
                .filter(|v| (1.0..65536.0).contains(v))
                .unwrap_or(default)
        };
        let half = level(2, 16384.0) as u32;
        // The black level (0x7310) is given for 14-bit data; these tiles carry more bits.
        let bps = raw
            .uint(d, tag::BITS_PER_SAMPLE)
            .unwrap_or(14)
            .clamp(14, 16);
        let scale = (1u32 << (bps - 14)) as f32;
        if let Black::Levels(l) = &mut s.black {
            l.iter_mut().for_each(|v| *v *= scale);
        }
        let black = match s.black {
            Black::Levels(l) => l[0],
            Black::Masked { fallback, .. } => fallback[0],
        };
        s.white = Some(level(1, 16383.0) as f32 + black);
        s.cpp = 3;
        s.cfa = None;
        s.bits = 16;
        s.storage = Storage::Blocks {
            blocks,
            tile_w: tw,
            tile_h: th,
            codec: Codec::Ycc { half },
        };
        return s;
    }
    if s.cfa.as_ref().and_then(Cfa::bayer2x2).is_none() {
        return s.unsupported("ARW lossless: non-Bayer CFA");
    }
    let codec = if h.components == 4 && h.width * 2 == tw && h.height * 2 == th {
        Codec::Lj92Quad
    } else if h.width * h.components == tw && h.height == th {
        Codec::Lj92
    } else {
        return s.unsupported("ARW lossless: unknown tile frame layout");
    };
    if !(8..=16).contains(&h.precision) {
        return s.unsupported("ARW lossless: sample precision");
    }
    s.bits = h.precision as u32;
    s.storage = Storage::Blocks {
        blocks,
        tile_w: tw,
        tile_h: th,
        codec,
    };
    s
}

// ---------------------------------------------------------------------------------------------
// Panasonic RW2

fn rw2_sensor(d: &[u8], w: &Walk) -> Option<Sensor> {
    let ifd0 = w.ifd0()?;
    let width = ifd0.uint(d, 0x0002)?;
    let height = ifd0.uint(d, 0x0003)?;
    let bits = ifd0.uint(d, 0x000a).unwrap_or(12);
    let compression = ifd0.uint(d, 0x000b).unwrap_or(34316);
    let raw_format = ifd0.uint(d, 0x002d).unwrap_or(0);
    let comp = if compression == 34316 {
        SensorCompression::Panasonic
    } else {
        SensorCompression::Other(compression)
    };
    let mut s = Sensor::new(width, height, bits, comp);
    s.cfa = match ifd0.uint(d, 0x0009) {
        Some(2) => Cfa::bayer("GRBG"),
        Some(3) => Cfa::bayer("GBRG"),
        Some(4) => Cfa::bayer("BGGR"),
        _ => Cfa::bayer("RGGB"),
    };
    let (t, l, b, r) = (
        ifd0.uint(d, 0x0004).unwrap_or(0),
        ifd0.uint(d, 0x0005).unwrap_or(0),
        ifd0.uint(d, 0x0006).unwrap_or(height),
        ifd0.uint(d, 0x0007).unwrap_or(width),
    );
    if l < r && t < b && r <= width && b <= height {
        s.crop = Rect {
            x: l,
            y: t,
            w: r - l,
            h: b - t,
        };
    }
    let bl: Vec<u32> = (0x1c..=0x1e).filter_map(|t| ifd0.uint(d, t)).collect();
    if bl.len() == 3 {
        let b = [bl[0] as f32, bl[1] as f32, bl[1] as f32, bl[2] as f32];
        s.black = Black::Levels(black_from_rggb(s.cfa.as_ref(), b));
    }
    let wb: Vec<u32> = (0x24..=0x26).filter_map(|t| ifd0.uint(d, t)).collect();
    if wb.len() == 3 && wb.iter().all(|&v| v > 0) {
        s.wb = Some([
            wb[0] as f32 / wb[1] as f32,
            1.0,
            wb[2] as f32 / wb[1] as f32,
        ]);
    } else if let (Some(r), Some(b)) = (ifd0.uint(d, 0x0011), ifd0.uint(d, 0x0012))
        && r > 0
        && b > 0
    {
        s.wb = Some([r as f32 / 256.0, 1.0, b as f32 / 256.0]);
    }
    let off = ifd0
        .uint(d, 0x0118)
        .or_else(|| ifd0.uint(d, tag::STRIP_OFFSETS).filter(|&o| o != u32::MAX))?
        as usize;
    let range = clamp_range(d, off, usize::MAX);
    if compression != 34316 {
        // The older `.RAW` bodies (FZ30, L1, Digilux 2, V-LUX 1): compressions 34826, 34828
        // and 34830 store the 12-bit samples unpacked, in the high bits of little-endian
        // 16-bit words.
        let need = width as usize * height as usize * 2;
        if matches!(compression, 34826 | 34828 | 34830)
            && range.len() >= need
            && width > 0
            && height > 0
        {
            s.compression = SensorCompression::Uncompressed;
            let lim = (0x0e..=0x10)
                .filter_map(|t| ifd0.uint(d, t))
                .filter(|&v| v > 0)
                .min()
                .unwrap_or(4095);
            s.bits = 12;
            s.white = Some(lim.min(4095) as f32);
            s.storage = Storage::Blocks {
                blocks: std::iter::once(range.start..range.start + need).collect(),
                tile_w: width,
                tile_h: height,
                codec: Codec::Raw {
                    packing: Packing::U16Hi12(true),
                    row_bytes: 0,
                },
            };
            return Some(s);
        }
        return Some(s.unsupported("RW2 compression"));
    }
    if raw_format == 8 {
        let Some(t) = crate::pana8::tags(d, ifd0, width as usize, height as usize) else {
            return Some(s.unsupported("RW2 raw format 8 without valid strip tags"));
        };
        let p = crate::pana8::Params::new(t);
        let lim = (0x0e..=0x10)
            .filter_map(|t| ifd0.uint(d, t))
            .filter(|&v| v > 0)
            .min()
            .unwrap_or(u32::MAX);
        s.white = Some(p.white(lim).max(1) as f32);
        s.storage = Storage::Vendor {
            range: p.span(),
            codec: crate::vendor::VendorCodec::Pana8(Box::new(p)),
        };
        return Some(s);
    }
    // The FZ8 and its time: 12-bit little-endian packing, 16 bytes per 10 samples, not the
    // bit stream (the strip's size tells).
    let packed = (width as usize).div_ceil(10) * 16 * height as usize;
    if raw_format < 5
        && bits == 12
        && ifd0.uint(d, tag::STRIP_BYTE_COUNTS) == Some(packed as u32)
        && range.len() >= packed
    {
        s.compression = SensorCompression::Uncompressed;
        if s.white.is_none() {
            s.white = Some(4095.0);
        }
        let need = packed;
        s.storage = Storage::Blocks {
            blocks: std::iter::once(range.start..range.start + need).collect(),
            tile_w: width,
            tile_h: height,
            codec: Codec::Raw {
                packing: Packing::Le12Pad10,
                row_bytes: 0,
            },
        };
        return Some(s);
    }
    if raw_format >= 5 {
        if !crate::pana::supported(raw_format, bits) {
            return Some(s.unsupported("RW2 raw format / bit depth"));
        }
        // Saturation: the smallest per-channel linearity limit; format 6 values carry a +15
        // offset that the decoder removes.
        if let Some(lim) = (0x0e..=0x10).filter_map(|t| ifd0.uint(d, t)).min()
            && lim > 0
        {
            let off = if raw_format == 6 { 15 } else { 0 };
            s.white = Some(lim.saturating_sub(off).max(1) as f32);
        }
        s.storage = Storage::PanaBlocks {
            range,
            version: raw_format,
            bits,
        };
        return Some(s);
    }
    // Formats up to 4: the 12-bit full scale, as LibRaw states it. A few samples run past
    // it (the D-LUX 4 file: 2651 at 4096, 1126 at 4097 over a pile of 181 921 at 4095); the
    // develop follows them.
    if s.white.is_none() && bits <= 16 {
        s.white = Some(((1u32 << bits.max(1)) - 1) as f32);
    }
    s.storage = Storage::Panasonic {
        range,
        load_flags: 0x2008,
    };
    Some(s)
}

// ---------------------------------------------------------------------------------------------
// Pentax PEF

fn pef_sensor(d: &[u8], w: &Walk, pentax: makernote::PentaxInfo) -> Option<Sensor> {
    let raw = largest_raw_ifd(d, w)?;
    let (width, height) = dims(d, raw);
    let bits = bps(d, raw);
    let compression = raw.uint(d, tag::COMPRESSION).unwrap_or(1);
    let comp = match compression {
        65535 => SensorCompression::PentaxHuffman,
        1 => SensorCompression::Uncompressed,
        c => SensorCompression::Other(c),
    };
    let mut s = Sensor::new(width, height, bits, comp);
    let mut ifds = vec![raw];
    if let Some(e) = &w.exif {
        ifds.push(e);
    }
    s.cfa = cfa_from_tags(d, &ifds).or_else(|| Cfa::bayer("RGGB"));
    if let Some(b) = pentax.black {
        s.black = Black::Levels(black_from_rggb(s.cfa.as_ref(), b.map(|v| v as f32)));
    }
    s.wb = pentax.wb_rggb.and_then(wb_from_rggb);
    // The picture area from the makernote: the K-x codes 43 columns of junk past it (samples
    // up to 3872 in a frame whose picture peaks at 903), which set the white level and the
    // demosaic's right edge.
    if let Some([x, y, cw, ch]) = pentax.area
        && u64::from(x) + u64::from(cw) <= u64::from(width)
        && u64::from(y) + u64::from(ch) <= u64::from(height)
    {
        s.crop = Rect { x, y, w: cw, h: ch };
    }
    let offs = raw.uints(d, tag::STRIP_OFFSETS, 2);
    let lens = raw.uints(d, tag::STRIP_BYTE_COUNTS, 2);
    match compression {
        65535 => {
            if offs.len() != 1 {
                return Some(s.unsupported("PEF: compressed data in several strips"));
            }
            s.storage = Storage::Pentax {
                range: clamp_range(
                    d,
                    offs[0] as usize,
                    lens.first().copied().unwrap_or(u32::MAX) as usize,
                ),
                table: pentax.huff_table,
                le: w.maker.as_ref().map_or(raw.le, |m| m.le),
            };
        }
        1 => match uncompressed_storage(d, raw, width, height, 1, bits, raw.le, false) {
            Some(st) => s.storage = st,
            None => return Some(s.unsupported("PEF: bad strip layout")),
        },
        _ => return Some(s.unsupported("PEF compression")),
    }
    Some(s)
}

// ---------------------------------------------------------------------------------------------
// Olympus ORF

fn orf_sensor(d: &[u8], w: &Walk, olympus: makernote::OlympusInfo) -> Option<Sensor> {
    let raw = w.ifd0()?;
    let (width, height) = dims(d, raw);
    let bits = bps(d, raw);
    let mut s = Sensor::new(width, height, bits, SensorCompression::Olympus);
    // The white is the full scale, as LibRaw states it, and the develop lowers it to a clip
    // the data shows: the E-PL1's samples clip spread over 3928–3958 (makernote 3932), so
    // the largest sample was no level of its own.
    s.white_nominal = true;
    // The E-1 says GRBG in the Exif CFAPattern only (the raw IFD has none): RGGB swapped
    // red and green.
    let exif: Vec<&Ifd> = w.exif.iter().collect();
    s.cfa = cfa_from_tags(d, &[raw])
        .or_else(|| cfa_from_tags(d, &exif))
        .or_else(|| Cfa::bayer("RGGB"));
    if let Some(b) = olympus.black {
        s.black = Black::Levels(black_from_rggb(s.cfa.as_ref(), b.map(|v| v as f32)));
    }
    s.wb = olympus.wb_rb.map(|(r, b)| [r as f32, 1.0, b as f32]);
    let lens = raw.uints(d, tag::STRIP_BYTE_COUNTS, 1024);
    let bytes: u64 = lens.iter().map(|&l| l as u64).sum();
    let px = width as u64 * height as u64;
    if bytes >= px.saturating_mul(2) {
        s.compression = SensorCompression::Uncompressed;
        // 12 bits in the high end of each word (the makernote's ValidBits; LibRaw shifts them
        // down): read as 16 the levels were 16 times the black and white from the makernote.
        s.storage = match uncompressed_storage(d, raw, width, height, 1, 16, raw.le, false) {
            Some(Storage::Blocks {
                blocks,
                tile_w,
                tile_h,
                codec: Codec::Raw { packing, row_bytes },
            }) if matches!(packing, Packing::U16Le | Packing::U16Be) => Storage::Blocks {
                blocks,
                tile_w,
                tile_h,
                codec: Codec::Raw {
                    packing: Packing::U16Hi12(packing == Packing::U16Le),
                    row_bytes,
                },
            },
            other => other.unwrap_or(Storage::None),
        };
        s.bits = if matches!(
            s.storage,
            Storage::Blocks {
                codec: Codec::Raw {
                    packing: Packing::U16Hi12(_),
                    ..
                },
                ..
            }
        ) {
            12
        } else {
            bits.min(16)
        };
    } else if px.checked_mul(12) == Some(bytes * 8) {
        s.compression = SensorCompression::Uncompressed;
        s.storage = uncompressed_storage(d, raw, width, height, 1, 12, raw.le, false)
            .unwrap_or(Storage::None);
    } else {
        let offs = raw.uints(d, tag::STRIP_OFFSETS, 2);
        if offs.len() != 1 || lens.len() != 1 {
            return Some(s.unsupported("ORF: compressed data in several strips"));
        }
        s.bits = 12;
        s.storage = Storage::Olympus {
            range: clamp_range(d, offs[0] as usize, lens[0] as usize),
        };
    }
    Some(s)
}

// ---------------------------------------------------------------------------------------------
// Hasselblad 3FR / FFF

fn hasselblad_sensor(d: &[u8], w: &Walk) -> Option<Sensor> {
    let ifd0 = w.ifd0()?;
    let raw = w
        .image_ifds()
        .filter(|i| i.uint(d, tag::PHOTOMETRIC) == Some(32803))
        .max_by_key(|i| {
            let (a, b) = dims(d, i);
            a as u64 * b as u64
        })
        .or_else(|| largest_raw_ifd(d, w))?;
    let (width, height) = dims(d, raw);
    let bits = bps(d, raw);
    let compression = raw.uint(d, tag::COMPRESSION).unwrap_or(1);
    let comp = match compression {
        1 => SensorCompression::Uncompressed,
        7 => SensorCompression::LosslessJpeg,
        c => SensorCompression::Other(c),
    };
    let mut s = Sensor::new(width, height, bits, comp);
    s.cfa = cfa_from_tags(d, &[raw]).or_else(|| Cfa::bayer("RGGB"));
    crop_from_default_crop(d, raw, &mut s);
    if let Some(b) = raw.float(d, tag::BLACK_LEVEL) {
        s.black = Black::Levels([b as f32; 4]);
    }
    if let Some(v) = raw.uint(d, tag::WHITE_LEVEL).filter(|&v| v > 0) {
        s.white = Some(v as f32);
    }
    let col = dng_color(d, ifd0, raw);
    if col.cm[0].is_some() || col.cm[1].is_some() {
        s.color = ColorSource::Dng(Box::new(col));
    }
    if raw.uint(d, tag::SAMPLES_PER_PIXEL).unwrap_or(1) != 1 {
        return Some(s.unsupported("Hasselblad multi-shot (several samples per pixel)"));
    }
    match compression {
        7 => {
            let offs = raw.uints(d, tag::STRIP_OFFSETS, 2);
            let lens = raw.uints(d, tag::STRIP_BYTE_COUNTS, 2);
            let Some(&off) = offs.first() else {
                return Some(s.unsupported("Hasselblad: no sensor data"));
            };
            s.storage = Storage::Vendor {
                range: clamp_range(
                    d,
                    off as usize,
                    lens.first().copied().unwrap_or(u32::MAX) as usize,
                ),
                codec: crate::vendor::VendorCodec::Hasselblad { base: 0 },
            };
        }
        1 => match uncompressed_storage(d, raw, width, height, 1, bits, raw.le, false) {
            Some(st) => s.storage = st,
            None => return Some(s.unsupported("Hasselblad: bad strip layout")),
        },
        _ => return Some(s.unsupported("Hasselblad compression")),
    }
    Some(s)
}

/// DefaultCropOrigin / DefaultCropSize as the visible area, when they fit.
fn crop_from_default_crop(d: &[u8], raw: &Ifd, s: &mut Sensor) {
    let co = raw.floats(d, tag::DEFAULT_CROP_ORIGIN, 2);
    let cs = raw.floats(d, tag::DEFAULT_CROP_SIZE, 2);
    if co.len() == 2 && cs.len() == 2 {
        let (x, y) = (co[0].max(0.0) as u32, co[1].max(0.0) as u32);
        let (cw, ch) = (cs[0].max(0.0) as u32, cs[1].max(0.0) as u32);
        if cw > 0
            && ch > 0
            && x.checked_add(cw).is_some_and(|e| e <= s.width)
            && y.checked_add(ch).is_some_and(|e| e <= s.height)
        {
            s.crop = Rect { x, y, w: cw, h: ch };
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Other TIFF-based RAWs: uncompressed CFA data only.

fn generic_sensor(d: &[u8], w: &Walk) -> Option<Sensor> {
    tiff_bayer_sensor(d, w, None)
}

/// Sinarback `.sti`: 16-bit words holding 14-bit samples, a GRBG mosaic without CFA tags.
fn sinar_sensor(d: &[u8], w: &Walk) -> Option<Sensor> {
    let mut s = tiff_bayer_sensor(d, w, Some("GRBG"))?;
    if s.unsupported.is_none() && s.bits == 16 {
        s.white = Some(16383.0);
    }
    Some(s)
}

/// The largest raw IFD as uncompressed or lossless JPEG (compression 7 or Leaf's 99) Bayer
/// data; `default_cfa` when the IFD has no CFA tags.
fn tiff_bayer_sensor(d: &[u8], w: &Walk, default_cfa: Option<&str>) -> Option<Sensor> {
    let raw = largest_raw_ifd(d, w)?;
    let (width, height) = dims(d, raw);
    let bits = bps(d, raw);
    let compression = raw.uint(d, tag::COMPRESSION).unwrap_or(1);
    let mut s = Sensor::new(
        width,
        height,
        bits,
        match compression {
            1 => SensorCompression::Uncompressed,
            7 | 99 => SensorCompression::LosslessJpeg,
            c => SensorCompression::Other(c),
        },
    );
    let mut ifds = vec![raw];
    if let Some(e) = &w.exif {
        ifds.push(e);
    }
    s.cfa = cfa_from_tags(d, &ifds).or_else(|| default_cfa.and_then(Cfa::bayer));
    if s.cfa.as_ref().and_then(Cfa::bayer2x2).is_none() {
        return Some(s.unsupported("unknown RAW layout: no Bayer CFA pattern"));
    }
    match compression {
        1 => {
            s.storage = uncompressed_storage(d, raw, width, height, 1, bits, raw.le, false)
                .unwrap_or(Storage::None);
        }
        7 | 99 => match blocks(d, raw, width, height) {
            Some((b, tw, th)) => {
                s.storage = Storage::Blocks {
                    blocks: b,
                    tile_w: tw,
                    tile_h: th,
                    codec: Codec::Lj92,
                }
            }
            None => return Some(s.unsupported("bad strip/tile layout")),
        },
        _ => return Some(s.unsupported("unknown RAW layout: vendor compression")),
    }
    Some(s)
}

// ---------------------------------------------------------------------------------------------
// Kodak DCR / KDC

/// Kodak's private IFD (TIFF tag 0x8290 in IFD0).
fn kodak_ifd(d: &[u8], w: &Walk) -> Option<Ifd> {
    let ifd0 = w.ifd0()?;
    let off = ifd0.uint(d, 0x8290)? as usize;
    tiff::read_ifd(d, off, 0, w.le)
}

/// Kodak DCS sensors: compression 65000 or uncompressed, with the linearisation curve and
/// white balance from the Kodak IFD.
/// Kodak DC25 (`.k25`): the CFA IFD gives the picture's display size, but the strip holds
/// 244 rows of 8-bit samples as wide as the strip allows (512 in high, 256 in standard
/// resolution), under the file's 4 x 2 cyan, magenta, yellow and green pattern. The last 3
/// rows and the right 12 (high) or 8 (standard) columns hold no picture (measured on sample
/// files); the rest is stretched to the stated display size.
fn dc25_sensor(d: &[u8], w: &Walk, raw: &Ifd) -> Option<Sensor> {
    let model = w.ifd0()?.string(d, tag::MODEL)?;
    if !model.contains("DC25") || raw.uint(d, tag::COMPRESSION).unwrap_or(1) != 1 {
        return None;
    }
    const ROWS: u32 = 244;
    let off = raw.uint(d, tag::STRIP_OFFSETS)? as usize;
    let len = raw.uint(d, tag::STRIP_BYTE_COUNTS)? as usize;
    let width = (len / ROWS as usize) as u32;
    if !matches!(width, 256 | 512) || off.checked_add(len)? > d.len() {
        return None;
    }
    let mut s = Sensor::new(width, ROWS, 8, SensorCompression::Uncompressed);
    s.cfa = cfa_from_tags(d, &[raw]);
    s.crop = Rect {
        x: 0,
        y: 0,
        w: width - if width == 512 { 12 } else { 8 },
        h: ROWS - 3,
    };
    s.white = Some(255.0);
    let (dw, dh) = dims(d, raw);
    if dw > 0 && dh > 0 {
        s.default_scale = crate::scale::default_scale(&[
            dw as f64 / s.crop.w as f64,
            dh as f64 / s.crop.h as f64,
        ]);
    }
    s.storage = Storage::Blocks {
        blocks: std::iter::once(off..off + len).collect(),
        tile_w: width,
        tile_h: ROWS,
        codec: Codec::Raw {
            packing: Packing::U8,
            row_bytes: 0,
        },
    };
    Some(s)
}

fn kodak_sensor(d: &[u8], w: &Walk) -> Option<Sensor> {
    if let Some(sub) = w.image_ifds().find(|i| i.get(KODAK_KDC_STRIPS).is_some()) {
        return kdc_sensor(d, w, sub);
    }
    let raw = w
        .image_ifds()
        .filter(|i| i.uint(d, tag::PHOTOMETRIC) == Some(32803))
        .max_by_key(|i| {
            let (a, b) = dims(d, i);
            a as u64 * b as u64
        })
        .or_else(|| largest_raw_ifd(d, w))?;
    if let Some(s) = dc25_sensor(d, w, raw) {
        return Some(s);
    }
    let (width, height) = dims(d, raw);
    let compression = raw.uint(d, tag::COMPRESSION).unwrap_or(1);
    let kifd = kodak_ifd(d, w);
    let mut s = Sensor::new(
        width,
        height,
        bps(d, raw),
        match compression {
            1 => SensorCompression::Uncompressed,
            c => SensorCompression::Other(c),
        },
    );
    s.cfa = cfa_from_tags(d, &[raw]).or_else(|| Cfa::bayer("GRBG"));
    if s.cfa.as_ref().and_then(Cfa::bayer2x2).is_none() {
        return Some(s.unsupported("unknown RAW layout: no Bayer CFA pattern"));
    }
    let mut curve = Vec::new();
    if let Some(k) = &kifd {
        // Linearisation table: the white level is its value at 4095 once extended.
        let t = k.uints(d, 0x090d, 0x10000);
        if t.len() > 1 {
            curve = t.iter().map(|&v| v.min(65535) as u16).collect();
            let last = *curve.last().unwrap_or(&0);
            curve.resize(curve.len().max(4096), last);
            s.white = Some(curve[4095] as f32);
        }
        // White balance: the preset index (tag 1020, or 64013) selects one of the triples at
        // 2120.., stored as the inverse multipliers.
        let wbi = k.uint(d, 0xfa0d).or_else(|| k.uint(d, 0x03fc)).unwrap_or(0) as u16;
        let m = k.floats(d, 0x0848u16.saturating_add(wbi), 3);
        if wbi < 64 && m.len() == 3 && m.iter().all(|&v| v > 0.0) {
            s.wb = Some([(m[1] / m[0]) as f32, 1.0, (m[1] / m[2]) as f32]);
        }
    }
    let model = w
        .ifd0()
        .and_then(|i| i.string(d, tag::MODEL))
        .unwrap_or_default();
    let (dw, dh) = crate::kodak::DC120;
    if compression == 1 && model.contains("DC120") {
        let off = raw.uint(d, tag::STRIP_OFFSETS)? as usize;
        let mut s = Sensor::new(dw as u32, dh as u32, 8, SensorCompression::Uncompressed);
        s.cfa = cfa_from_tags(d, &[raw]).or_else(|| Cfa::bayer("GRBG"));
        s.white = Some(255.0);
        s.storage = Storage::Vendor {
            range: clamp_range(d, off, dw * dh),
            codec: crate::vendor::VendorCodec::KodakDc120,
        };
        return Some(s);
    }
    if compression == 7 && model.contains("DC120") {
        let off = raw.uint(d, tag::STRIP_OFFSETS)? as usize;
        let len = raw.uint(d, tag::STRIP_BYTE_COUNTS)? as usize;
        let mut s = Sensor::new(dw as u32, dh as u32, 9, SensorCompression::LossyJpeg);
        s.cfa = Cfa::bayer("GRBG");
        s.white = Some(510.0);
        s.storage = Storage::Vendor {
            range: clamp_range(d, off, len),
            codec: crate::vendor::VendorCodec::KodakDc120Jpeg,
        };
        return Some(s);
    }
    match compression {
        32867 => {
            // RADC (DC40, DC50): a 768 x 512 sensor whatever the TIFF says, 14-bit after the
            // codec's curve, coloured as dcraw and LibRaw colour every RADC camera.
            let off = raw.uint(d, tag::STRIP_OFFSETS)? as usize;
            let len = raw
                .uint(d, tag::STRIP_BYTE_COUNTS)
                .map_or(usize::MAX, |v| v as usize);
            let cbpp = raw
                .get(37122)
                .and_then(|e| tiff::Endian { le: e.le }.u32(d, e.pos))
                .unwrap_or(0);
            let mut s = Sensor::new(768, 512, 14, SensorCompression::Other(32867));
            s.cfa = cfa_from_tags(d, &[raw]).or_else(|| Cfa::bayer("GRBG"));
            s.white = Some(crate::radc::WHITE as f32);
            if let Some(m) = crate::color::extra_entry("Apple", "QuickTake")
                .and_then(|e| e.matrix.as_ref())
                .and_then(crate::color::adobe_rgb_cam)
            {
                s.color = ColorSource::Matrix(m, MatrixSource::Published);
            }
            s.storage = Storage::Vendor {
                range: clamp_range(d, off, len),
                codec: crate::vendor::VendorCodec::KodakRadc { cbpp },
            };
            return Some(s);
        }
        65000 => {
            let off = raw.uint(d, tag::STRIP_OFFSETS)? as usize;
            let len = raw
                .uint(d, tag::STRIP_BYTE_COUNTS)
                .map_or(usize::MAX, |v| v as usize);
            let per_row = (width as usize).div_ceil(256);
            let ends = raw.uints(
                d,
                0xfde9,
                per_row.saturating_mul(height as usize).min(1 << 24),
            );
            s.bits = 12;
            s.storage = Storage::Vendor {
                range: clamp_range(d, off, len),
                codec: crate::vendor::VendorCodec::Kodak65000(Box::new(crate::kodak::K65000 {
                    ends,
                    le: raw.le,
                    curve,
                })),
            };
        }
        1 => match uncompressed_storage(d, raw, width, height, 1, bps(d, raw), raw.le, false) {
            Some(st) => s.storage = st,
            None => return Some(s.unsupported("Kodak: bad strip layout")),
        },
        _ => return Some(s.unsupported("unknown RAW layout: Kodak compression")),
    }
    Some(s)
}

/// The maker note IFD of Kodak's P-series and EasyShare bodies: either a bare IFD or one behind
/// a two-byte byte-order mark.
fn kodak_maker_ifd(d: &[u8], w: &Walk) -> Option<Ifd> {
    let e = w.exif.as_ref()?.get(tag::MAKER_NOTE)?;
    let p = e.pos;
    match d.get(p..p + 2)? {
        b"II" => tiff::read_ifd(d, p + 2, 0, true),
        b"MM" => tiff::read_ifd(d, p + 2, 0, false),
        _ => tiff::read_ifd(d, p, 0, w.le),
    }
}

/// Kodak P-series / EasyShare KDC (P712, P850, P880, Z1015 ...): 12-bit big-endian packed rows
/// after a 64-byte header at the first listed strip; size, CFA and white level in the maker
/// note's 0xff00 IFD, white balance in the 0xfe00 IFD.
fn kdc_sensor(d: &[u8], w: &Walk, sub: &Ifd) -> Option<Sensor> {
    let ifd0 = w.ifd0()?;
    let info = kodak_maker_ifd(d, w)
        .and_then(|m| m.uint(d, 0xff00))
        .and_then(|o| tiff::read_ifd(d, o as usize, 0, w.le));
    let extra = ifd0
        .uint(d, 0xfe00)
        .and_then(|o| tiff::read_ifd(d, o as usize, 0, w.le));
    let pick = |a: Option<&Ifd>, ta: u16, b: Option<&Ifd>, tb: u16| {
        a.and_then(|i| i.uint(d, ta))
            .or_else(|| b.and_then(|i| i.uint(d, tb)))
            .filter(|&v| v > 0)
    };
    let width = pick(info.as_ref(), 0xf900, extra.as_ref(), 0xfa13)
        .or_else(|| sub.uint(d, 0xfd00))
        .unwrap_or(0);
    let height = pick(info.as_ref(), 0xf901, extra.as_ref(), 0xfa14)
        .or_else(|| sub.uint(d, 0xfd01))
        .unwrap_or(0);
    let mut s = Sensor::new(width, height, 12, SensorCompression::Uncompressed);
    let cfa = info
        .as_ref()
        .and_then(|i| i.string(d, 0xf902))
        .or_else(|| extra.as_ref().and_then(|i| i.string(d, 0xfa15)));
    s.cfa = cfa.as_deref().and_then(Cfa::bayer);
    if s.cfa.as_ref().and_then(Cfa::bayer2x2).is_none() {
        return Some(s.unsupported("unknown RAW layout: no Bayer CFA pattern"));
    }
    if let Some(v) = info
        .as_ref()
        .and_then(|i| i.uint(d, 0xf90c))
        .filter(|&v| v > 0 && v < 4096)
    {
        s.white = Some(v as f32);
    }
    if let Some(e) = extra.as_ref() {
        let m = e.uints(d, 0xfa12, 3);
        if m.len() == 3 && m.iter().all(|&v| v > 0) {
            s.wb = Some([m[0] as f32 / m[1] as f32, 1.0, m[2] as f32 / m[1] as f32]);
        }
    }
    let strips = sub.uints(d, KODAK_KDC_STRIPS, 17);
    let row_bytes = width as usize * 3 / 2;
    let (Some(&first), true) = (strips.get(16), width > 0 && height > 0 && width % 2 == 0) else {
        return Some(s.unsupported("unknown RAW layout: Kodak KDC strips"));
    };
    s.storage = Storage::Blocks {
        blocks: vec![clamp_range(
            d,
            (first as usize).saturating_add(64),
            row_bytes.saturating_mul(height as usize),
        )],
        tile_w: width,
        tile_h: height,
        codec: Codec::Raw {
            packing: Packing::Msb(12),
            row_bytes,
        },
    };
    Some(s)
}

// ---------------------------------------------------------------------------------------------
// Epson ERF

/// Epson ERF (R-D1, R-D1s/x): 12-bit data packed with a padding byte after every ten samples
/// (compression 32769), black levels and white balance in the maker note.
fn erf_sensor(d: &[u8], w: &Walk) -> Option<Sensor> {
    let raw = largest_raw_ifd(d, w)?;
    let (width, height) = dims(d, raw);
    let compression = raw.uint(d, tag::COMPRESSION).unwrap_or(1);
    if compression == 1 {
        return tiff_bayer_sensor(d, w, Some("RGGB"));
    }
    let mut s = Sensor::new(width, height, 12, SensorCompression::Other(compression));
    s.cfa = cfa_from_tags(d, &[raw]).or_else(|| Cfa::bayer("RGGB"));
    s.white = Some(4095.0);
    if let Some(mn) = &w.maker {
        // Black per 2x2 position (raster order).
        let b = mn.uints(d, 0x0401, 4);
        if b.len() == 4 && b.iter().all(|&v| v < 4095) {
            s.black = Black::Levels([b[0] as f32, b[1] as f32, b[2] as f32, b[3] as f32]);
        }
        // White balance: two 16-bit coefficients 48 bytes into the 256-byte block, times fixed
        // per-channel gains: 567 and 431 (/ 65536), as LibRaw 0.22 reads them. The older dcraw
        // gains (508 × 1.078, 382 × 1.173) left the R-D1s sample's develop 4 % off the camera's
        // own JPEG in R/G and 9 % in B/G; these are within 0.3 % and 2.6 %.
        if let Some(e) = mn.get(0x0e80).filter(|e| e.byte_len() == 256)
            && let Some(b) = e.bytes(d)
        {
            let le = mn.le;
            let g = |i: usize| {
                let v = [b[i], b[i + 1]];
                (if le {
                    u16::from_le_bytes(v)
                } else {
                    u16::from_be_bytes(v)
                }) as f32
            };
            let (r, bl) = (g(48), g(50));
            if r > 0.0 && bl > 0.0 {
                s.wb = Some([r * 567.0 / 65536.0, 1.0, bl * 431.0 / 65536.0]);
            }
        }
    }
    let offs = raw.uints(d, tag::STRIP_OFFSETS, 2);
    let lens = raw.uints(d, tag::STRIP_BYTE_COUNTS, 2);
    let row_bytes = (width as usize).div_ceil(10) * 16;
    match (compression, offs.first()) {
        (32769, Some(&off)) if offs.len() == 1 => {
            s.storage = Storage::Blocks {
                blocks: vec![clamp_range(
                    d,
                    off as usize,
                    lens.first()
                        .map(|&l| l as usize)
                        .unwrap_or(row_bytes * height as usize),
                )],
                tile_w: width,
                tile_h: height,
                codec: Codec::Raw {
                    packing: Packing::Msb12Pad10,
                    row_bytes,
                },
            };
            Some(s)
        }
        _ => Some(s.unsupported("unknown RAW layout: Epson compression")),
    }
}

// ---------------------------------------------------------------------------------------------
// Samsung SRW

/// Samsung's maker note: an IFD at its start, offsets relative to it.
fn samsung_maker_ifd(d: &[u8], w: &Walk) -> Option<Ifd> {
    let e = w.exif.as_ref()?.get(tag::MAKER_NOTE)?;
    let p = e.pos;
    tiff::read_ifd(d, p, p, w.le)
}

fn srw_sensor(d: &[u8], w: &Walk) -> Option<Sensor> {
    let raw = largest_raw_ifd(d, w)?;
    let (width, height) = dims(d, raw);
    let bits = bps(d, raw);
    let compression = raw.uint(d, tag::COMPRESSION).unwrap_or(1);
    let mut s = Sensor::new(width, height, bits, SensorCompression::Other(compression));
    if !(8..=16).contains(&bits) {
        return Some(s.unsupported("unknown RAW layout: Samsung sample size"));
    }
    s.white = Some(((1u32 << bits) - 1) as f32);
    // Sizes the tags do not give, by the stored size (as LibRaw places them).
    let (rw, rh) = (width, height);
    let mut cfa = cfa_from_tags(d, &[raw]);
    let crop = |x: u32, y: u32, cw: u32, ch: u32| Rect { x, y, w: cw, h: ch };
    if rw == 4704 && rh > 8 {
        s.crop = crop(8, 8, rw - 16, rh - 8);
        // Both masked strips, left and right (as LibRaw measures them: the NX100's left
        // strip alone read 1.2 above its level).
        s.black = Black::Masked {
            area: crop(0, 8, 8, rh - 8),
            also: Some(crop(rw - 8, 8, 8, rh - 8)),
            fallback: [0.0; 4],
        };
    } else if rh == 3714 && rw >= 5536 {
        // The CFA tags of these bodies are wrong.
        cfa = Cfa::bayer("GRBG");
        s.crop = if rw == 5600 {
            crop(rw - 5536, 18, 5536, rh - 18)
        } else {
            crop(0, 0, 5536, rh - 18)
        };
    } else if rw == 5632 && rh >= 3696 {
        let left = 32 + bits;
        s.crop = crop(left, 2, 5574 - left, 3694);
    } else if rw == 5664 && rh > 18 {
        cfa = Cfa::bayer("RGGB");
        s.crop = crop(96, 18, 5544, rh - 18);
    } else if rw == 6496 {
        s.black = Black::Levels([(1u32 << (bits - 7)) as f32; 4]);
    }
    s.cfa = cfa;
    if s.cfa.as_ref().and_then(Cfa::bayer2x2).is_none() {
        return Some(s.unsupported("unknown RAW layout: no Bayer CFA pattern"));
    }
    if let Some(mn) = samsung_maker_ifd(d, w) {
        // White balance levels R, G, G, B, stored with Samsung's offsets (the first four
        // values of tag 0xa020) added.
        let lv = mn.uints(d, 0xa021, 4);
        let mut key = mn.uints(d, 0xa020, 4);
        key.resize(4, 0);
        if lv.len() == 4 {
            let v: Vec<f32> = lv
                .iter()
                .zip(&key)
                .map(|(&l, &k)| l.wrapping_sub(k) as i32 as f32)
                .collect();
            if v.iter().all(|&x| x > 0.0) {
                s.wb = Some([v[0] / v[1], 1.0, v[3] / v[1]]);
            }
        }
    }
    let off = raw.uint(d, tag::STRIP_OFFSETS)? as usize;
    let len = raw.uint(d, tag::STRIP_BYTE_COUNTS).unwrap_or(u32::MAX) as usize;
    let (w_, h_) = (width as usize, height as usize);
    let vendor = |codec| Storage::Vendor {
        range: clamp_range(d, off, len),
        codec,
    };
    s.storage = match compression {
        32769 | 32770 if len >= w_ * h_ * bits as usize / 8 => {
            // Plain packed rows: little-endian 12-bit for the 5632-wide sensors (whatever
            // the file's byte order), big-endian bits otherwise.
            let (packing, row_bytes) = if rw == 5632 && bits == 12 {
                (Packing::Le12, w_ * 3 / 2)
            } else {
                (Packing::Msb(bits), (w_ * bits as usize).div_ceil(8))
            };
            Storage::Blocks {
                blocks: vec![clamp_range(d, off, row_bytes * h_)],
                tile_w: width,
                tile_h: height,
                codec: Codec::Raw { packing, row_bytes },
            }
        }
        32770 => match raw.uint(d, 0xa010) {
            Some(rows) => vendor(crate::vendor::VendorCodec::SamsungV1 {
                rows: clamp_range(d, rows as usize, h_.saturating_mul(4)),
            }),
            None => return Some(s.unsupported("Samsung: no row offsets")),
        },
        32772 => vendor(crate::vendor::VendorCodec::SamsungV2),
        32773 => vendor(crate::vendor::VendorCodec::SamsungV3),
        1 => uncompressed_storage(d, raw, width, height, 1, bits, raw.le, false)
            .unwrap_or(Storage::None),
        _ => return Some(s.unsupported("unknown RAW layout: Samsung compression")),
    };
    Some(s)
}

// ---------------------------------------------------------------------------------------------
// Leaf MOS

fn mos_sensor(d: &[u8], w: &Walk, leaf: Option<crate::leaf::LeafInfo>) -> Option<Sensor> {
    let leaf = leaf.unwrap_or_default();
    let mut s = tiff_bayer_sensor(d, w, Some(leaf_cfa(leaf.red_at.unwrap_or(0))))?;
    s.wb = leaf.wb;
    if s.bits == 16 && s.white.is_none() {
        // Leaf backs store 14-bit data in 16-bit samples.
        s.white = Some(16383.0);
    }
    Some(s)
}

fn leaf_cfa(red_at: usize) -> &'static str {
    match red_at {
        1 => "GRBG",
        2 => "GBRG",
        3 => "BGGR",
        _ => "RGGB",
    }
}

// ---------------------------------------------------------------------------------------------
// Canon CR3 (ISO-BMFF): previews and metadata only.

fn parse_cr3(d: &[u8]) -> Result<(RawInfo, Option<Sensor>)> {
    let mut info = RawInfo {
        format: Some(RawFormat::Cr3),
        ..Default::default()
    };
    let mut pv = Previews::default();
    let top = bmff::boxes(d, 0..d.len());
    let moov = top
        .iter()
        .find(|b| &b.typ == b"moov")
        .ok_or(RawError::Malformed("CR3 without moov"))?;
    let mut cmt: [Option<Range<usize>>; 4] = [None, None, None, None];
    for b in bmff::boxes(d, moov.payload.clone()) {
        if b.uuid == Some(CANON_UUID) {
            for c in bmff::boxes(d, b.payload.clone()) {
                match &c.typ {
                    b"CMT1" => cmt[0] = Some(c.payload.clone()),
                    b"CMT2" => cmt[1] = Some(c.payload.clone()),
                    b"CMT3" => cmt[2] = Some(c.payload.clone()),
                    b"CMT4" => cmt[3] = Some(c.payload.clone()),
                    b"THMB" => {
                        if let Some(r) = find_soi(d, c.payload.clone(), 32) {
                            pv.jpeg(d, r, "CR3 THMB");
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    // Metadata: CMT1 = IFD0, CMT2 = Exif IFD, CMT3 = Canon maker note.
    if let Some(r) = &cmt[0] {
        fill_from_tiff_block(&mut info, d, r.clone());
    }
    if let Some(r) = &cmt[1]
        && let Some(b) = d.get(r.clone())
        && let Some((le, first)) = tiff::header(b, 0)
        && let Some(e) = tiff::read_ifd(b, first, 0, le)
    {
        fill_exif(&mut info, b, &e);
    }
    // The lens from the maker note when CMT2 names none (offsets from CMT3's own header).
    if info.lens.is_none()
        && let Some(r) = &cmt[2]
        && let Some(b) = d.get(r.clone())
        && let Some((le, first)) = tiff::header(b, 0)
        && let Some(mn) = tiff::read_ifd(b, first, 0, le)
    {
        info.lens = makernote::canon_lens(b, &mn);
    }
    info.tiff_blocks = cmt.iter().flatten().cloned().collect();
    for b in &top {
        if b.uuid == Some(PRVW_UUID)
            && let Some(p) = bmff::boxes(d, b.payload.start.saturating_add(8)..b.payload.end)
                .into_iter()
                .find(|c| &c.typ == b"PRVW")
            && let Some(r) = find_soi(d, p.payload.clone(), 32)
        {
            pv.jpeg(d, r, "CR3 PRVW");
        }
    }
    // Tracks: JPEG tracks are previews; the largest CRAW track with a CMP1 header is the raw.
    let mut raw = None::<(Range<usize>, crate::crx::Header)>;
    // A CRAW track this decoder refuses (sRAW/mRAW, unknown header), reported when no
    // Bayer track exists so the file still says why it cannot be developed.
    let mut refused = None::<crate::crx::Refused>;
    let mut ctmd = None::<makernote::CanonInfo>;
    for t in bmff::boxes(d, moov.payload.clone())
        .into_iter()
        .filter(|b| &b.typ == b"trak")
    {
        let Some(stbl) = bmff::path(d, t.payload.clone(), &[b"mdia", b"minf", b"stbl"]) else {
            continue;
        };
        let Some((off, size)) = track_first_sample(d, stbl.payload.clone()) else {
            continue;
        };
        if off.checked_add(2).and_then(|e| d.get(off..e)) == Some(&[0xff, 0xd8]) {
            pv.jpeg(d, off..off.saturating_add(size), "CR3 full-size JPEG track");
            continue;
        }
        let entry = bmff::find(d, stbl.payload.clone(), b"stsd").and_then(|s| {
            bmff::boxes(d, s.payload.start + 8..s.payload.end)
                .into_iter()
                .next()
        });
        if let Some(e) = &entry
            && &e.typ == b"CTMD"
        {
            ctmd = ctmd.or_else(|| ctmd_canon(d, off..off.saturating_add(size)));
            continue;
        }
        let Some(e) = entry.filter(|e| &e.typ == b"CRAW") else {
            continue;
        };
        // CRAW sample entry: 82 bytes of fixed fields, then child boxes (CMP1, CDI1, ...).
        let cmp1 = bmff::find(
            d,
            e.payload.start.saturating_add(82)..e.payload.end,
            b"CMP1",
        )
        .and_then(|c| d.get(c.payload.clone()));
        let hdr = cmp1.and_then(crate::crx::parse_cmp1);
        if refused.is_none()
            && hdr.is_none_or(|h| h.n_planes != 4)
            && let Some(r) = cmp1.and_then(crate::crx::refused)
        {
            refused = Some(r);
        }
        if let Some(h) = hdr
            && h.n_planes == 4
            && raw.as_ref().is_none_or(|(_, o)| {
                h.width as u64 * h.height as u64 > o.width as u64 * o.height as u64
            })
        {
            raw = Some((off..off.saturating_add(size), h));
        }
    }
    if info.orientation == 0 {
        info.orientation = 1;
    }
    info.previews = pv.finish(info.orientation);
    let canon = cmt[2]
        .as_ref()
        .and_then(|r| d.get(r.clone()))
        .and_then(|b| {
            let (le, first) = tiff::header(b, 0)?;
            let mn = tiff::read_ifd(b, first, 0, le)?;
            Some(makernote::canon(b, &mn))
        })
        .unwrap_or_default();
    // Newer bodies keep ColorData (WB, black and white levels) in the timed-metadata track.
    let canon = match ctmd {
        Some(c) => makernote::CanonInfo {
            sensor: canon.sensor.or(c.sensor),
            wb_rggb: canon.wb_rggb.or(c.wb_rggb),
            black: canon.black.or(c.black),
            sraw_levels: canon.sraw_levels.or(c.sraw_levels),
            model_id: canon.model_id.or(c.model_id),
            firmware: canon.firmware.clone().or(c.firmware),
        },
        None => canon,
    };
    let sensor = raw.map(|(range, h)| {
        let mut s = Sensor::new(h.width, h.height, h.bits(), SensorCompression::CanonCrx);
        s.cfa = Cfa::bayer(h.cfa_name());
        if let Some([sw, sh, l, t, r, b]) = canon.sensor
            && sw <= s.width.saturating_add(64)
            && sh <= s.height.saturating_add(64)
            && r < s.width
            && b < s.height
        {
            let (x, y) = ((l + 1) & !1, (t + 1) & !1);
            let (cw, ch) = ((r + 1 - x) & !1, (b + 1 - y) & !1);
            s.crop = Rect { x, y, w: cw, h: ch };
        }
        if let Some(b) = canon.black {
            s.black = Black::Levels(b.map(|v| v as f32));
        }
        s.white_nominal = true;
        s.wb = canon.wb_rggb.and_then(wb_from_rggb);
        s.storage = Storage::Crx { range, header: h };
        if let Some(why) = crx_refusal(&h) {
            s.unsupported = Some(why);
        }
        s
    });
    let sensor = sensor.or_else(|| {
        let r = refused?;
        Some(Sensor::new(r.width, r.height, r.bits, SensorCompression::CanonCrx).unsupported(r.why))
    });
    info.sensor = sensor.as_ref().map(Sensor::info);
    Ok((info, sensor))
}

/// Canon maker-note fields from a CR3 timed-metadata (CTMD) sample: records of
/// `u32 size, u16 type, ...`; types 7-9 carry a maker-note TIFF block after a 20-byte header.
fn ctmd_canon(d: &[u8], sample: Range<usize>) -> Option<makernote::CanonInfo> {
    let b = d.get(sample.start..sample.end.min(d.len()))?;
    let mut p = 0usize;
    let mut out: Option<makernote::CanonInfo> = None;
    while p + 8 <= b.len() {
        let size = u32::from_le_bytes(b[p..p + 4].try_into().ok()?) as usize;
        let typ = u16::from_le_bytes(b[p + 4..p + 6].try_into().ok()?);
        if size < 8 || size > b.len() - p {
            break;
        }
        if (7..=9).contains(&typ)
            && size > 28
            && let Some(t) = b.get(p + 20..p + size)
            && let Some((le, first)) = tiff::header(t, 0)
            && let Some(mn) = tiff::read_ifd(t, first, 0, le)
            && mn.get(0x4001).is_some()
        {
            let c = makernote::canon(t, &mn);
            out = Some(match out {
                Some(o) => makernote::CanonInfo {
                    sensor: o.sensor.or(c.sensor),
                    wb_rggb: o.wb_rggb.or(c.wb_rggb),
                    black: o.black.or(c.black),
                    sraw_levels: o.sraw_levels.or(c.sraw_levels),
                    model_id: o.model_id.or(c.model_id),
                    firmware: o.firmware.or(c.firmware),
                },
                None => c,
            });
        }
        p += size;
    }
    out
}

/// The JPEG inside a preview box: first SOI within `scan` bytes of the payload start.
fn find_soi(d: &[u8], payload: Range<usize>, scan: usize) -> Option<Range<usize>> {
    let end = payload.end.min(d.len());
    let p = payload.start;
    (p..(p + scan).min(end.saturating_sub(3)))
        .find(|&i| d[i] == 0xff && d[i + 1] == 0xd8 && d[i + 2] == 0xff)
        .map(|s| s..end)
}

/// Offset and size of the first sample of a track (stco/co64 + stsz).
fn track_first_sample(d: &[u8], stbl: Range<usize>) -> Option<(usize, usize)> {
    let stsz = bmff::find(d, stbl.clone(), b"stsz")?;
    let mut size = be_u32(d, stsz.payload.start + 4)? as usize;
    if size == 0 {
        size = be_u32(d, stsz.payload.start + 12)? as usize;
    }
    let off = if let Some(co) = bmff::find(d, stbl.clone(), b"co64") {
        crate::tiff::be_u64(d, co.payload.start + 8)? as usize
    } else {
        let co = bmff::find(d, stbl, b"stco")?;
        be_u32(d, co.payload.start + 8)? as usize
    };
    Some((off, size))
}

// ---------------------------------------------------------------------------------------------
// Fujifilm RAF

fn parse_raf(d: &[u8]) -> Result<(RawInfo, Option<Sensor>)> {
    let jpeg_off = be_u32(d, 84).ok_or(RawError::Malformed("RAF header"))? as usize;
    let jpeg_len = be_u32(d, 88).ok_or(RawError::Malformed("RAF header"))? as usize;
    let mut info = RawInfo {
        format: Some(RawFormat::Raf),
        make: "FUJIFILM".into(),
        ..Default::default()
    };
    if let Some(m) = d.get(28..60) {
        let end = m.iter().position(|&c| c == 0).unwrap_or(m.len());
        info.model = String::from_utf8_lossy(&m[..end]).trim().to_string();
    }
    let mut pv = Previews::default();
    let jr = clamp_range(d, jpeg_off, jpeg_len);
    if let Some(ji) = pv.jpeg(d, jr.clone(), "RAF JPEG") {
        if let Some(ex) = ji.exif {
            let block = jr.start + ex.start..jr.start + ex.end;
            info.make.clear();
            info.model.clear();
            fill_from_tiff_block(&mut info, d, block.clone());
            if info.make.is_empty() {
                info.make = "FUJIFILM".into();
            }
            info.tiff_blocks.push(block);
        }
        if let Some(t) = ji.exif_thumbnail {
            pv.jpeg(
                d,
                jr.start + t.start..jr.start + t.end,
                "embedded JPEG Exif thumbnail",
            );
        }
    }
    if info.orientation == 0 {
        info.orientation = 1;
    }
    info.previews = pv.finish(info.orientation);
    let sensor = raf_sensor(d, &info.model);
    info.sensor = sensor.as_ref().map(Sensor::info);
    Ok((info, sensor))
}

/// RAF sensor description: the CFA header directory (big-endian `tag, size, value` records)
/// and the TIFF-like raw directory (tag 0xF000 sub-IFD with 0xF001.. Fuji tags).
fn raf_sensor(d: &[u8], model: &str) -> Option<Sensor> {
    let mut dims = None;
    let mut crop_tl = None;
    let mut crop_wh = None;
    let mut xtrans: Option<[[u8; 6]; 6]> = None;
    let mut wb_dir = None;
    let mut rotated = false;
    let mut frame = None;
    let mut layout = (false, false);
    if let (Some(dir), Some(_)) = (be_u32(d, 92), be_u32(d, 96)) {
        let dir = dir as usize;
        let n = be_u32(d, dir).unwrap_or(0).min(256) as usize;
        let mut p = dir + 4;
        for _ in 0..n {
            let (Some(t), Some(sz)) = (be_u16(d, p), be_u16(d, p + 2)) else {
                break;
            };
            let v = p + 4;
            let pair = || Some((be_u16(d, v)? as u32, be_u16(d, v + 2)? as u32));
            match t {
                0x100 => dims = pair(),
                0x110 => crop_tl = pair(),
                0x111 => crop_wh = pair(),
                0x121 => frame = pair(),
                0x130 => {
                    if let Some(b) = d.get(v..v + 2) {
                        // (one diagonal per stored row, photosites on a 45 degree lattice)
                        layout = (b[0] >> 7 != 0, b[1] & 8 == 0);
                        rotated = layout.0 || layout.1;
                    }
                }
                0x131 if sz == 36 => {
                    if let Some(b) = d.get(v..v + 36) {
                        // Stored last cell first.
                        xtrans = Some(std::array::from_fn(|r| {
                            std::array::from_fn(|c| b[35 - (r * 6 + c)].min(2))
                        }));
                    }
                }
                0x2ff0 if sz >= 8 => {
                    // G, R, G, B.
                    let g = (0..4)
                        .map(|i| be_u16(d, v + 2 * i))
                        .collect::<Option<Vec<_>>>();
                    if let Some(g) = g {
                        wb_dir = Some([g[1] as f32, g[0] as f32, g[3] as f32]);
                    }
                }
                _ => {}
            }
            p = v + sz as usize;
        }
    }
    // These bodies record red and blue levels that need a fixed correction to balance the
    // sensor data (LibRaw's published factors: 10/17/0.652941 and 2/3/(3/4 + 1/300)).
    if ["S2Pro", "S20Pro", "F700", "S5000", "S7000"]
        .iter()
        .any(|m| model.contains(m))
        && let Some(wb) = wb_dir.as_mut()
    {
        wb[0] *= (10.0 / 17.0 / 0.652941) as f32;
        wb[2] *= (2.0 / 3.0 / (3.0 / 4.0 + 1.0 / 300.0)) as f32;
    }
    let (h0, w0) = dims?;
    let mut s = Sensor::new(w0, h0, 14, SensorCompression::Fujifilm);
    if rotated {
        let crop = crop_tl.zip(crop_wh);
        return Some(superccd_sensor(
            d,
            s,
            model,
            (w0, h0),
            frame,
            crop,
            layout,
            wb_dir,
        ));
    }
    // Raw directory: a little TIFF at the CFA offset.
    let base = be_u32(d, 100)? as usize;
    let raw_ifd = tiff::header(d, base)
        .and_then(|(le, off)| tiff::read_ifd(d, off, base, le))
        .and_then(|root| {
            let sub = root.offsets(d, 0xf000, 1).into_iter().next()?;
            tiff::read_ifd(d, sub, base, root.le)
        });
    let Some(ifd) = raw_ifd else {
        return Some(s.unsupported("Fujifilm RAF without raw directory"));
    };
    let w = ifd.uint(d, 0xf001).unwrap_or(w0);
    let h = ifd.uint(d, 0xf002).unwrap_or(h0);
    let bits = ifd.uint(d, 0xf003).unwrap_or(14).clamp(8, 16);
    s.lens = makernote::fuji_lens(d, &ifd).map(std::sync::Arc::new);
    let off = base.saturating_add(ifd.uint(d, 0xf007)? as usize);
    let len = ifd.uint(d, 0xf008)? as usize;
    let range = clamp_range(d, off, len);
    s.width = w;
    s.height = h;
    s.bits = bits;
    // CFA: X-Trans layout from 0x131 (absolute sensor coordinates), else RGGB Bayer.
    let pattern: [[u8; 6]; 6] = xtrans.unwrap_or_else(|| {
        std::array::from_fn(|r| std::array::from_fn(|c| [[0, 1], [1, 2]][r & 1][c & 1]))
    });
    s.cfa = if xtrans.is_some() {
        Cfa::new(6, 6, pattern.iter().flatten().copied().collect())
    } else {
        Cfa::bayer("RGGB")
    };
    // Black: per CFA position (2x2 or 6x6 repeat), averaged per colour.
    let bl = ifd.uints(d, 0xf00a, 36);
    if !bl.is_empty() {
        let dim = if bl.len() >= 36 {
            6
        } else if bl.len() >= 4 {
            2
        } else {
            1
        };
        let mut sum = [0f32; 3];
        let mut cnt = [0f32; 3];
        for r in 0..dim {
            for c in 0..dim {
                let col = pattern[r][c] as usize;
                sum[col] += bl[(r * dim + c).min(bl.len() - 1)] as f32;
                cnt[col] += 1.0;
            }
        }
        let avg: [f32; 3] = std::array::from_fn(|i| {
            if cnt[i] > 0.0 {
                sum[i] / cnt[i]
            } else {
                sum.iter().sum::<f32>() / cnt.iter().sum::<f32>()
            }
        });
        s.black = Black::Levels(if xtrans.is_some() {
            [(avg[0] + avg[1] + avg[2]) / 3.0; 4]
        } else {
            [avg[0], avg[1], avg[1], avg[2]]
        });
    }
    // White: Bayer GFX / non-X bodies clip at 0x3E00 in 14-bit mode.
    let white = if xtrans.is_none() && bits == 14 && !model.starts_with("X-") {
        0x3e00
    } else {
        (1u32 << bits) - 1
    };
    s.white = Some(white as f32);
    // As-shot white balance: 0xF00E (G, R, B), else 0xF00D, else the CFA directory.
    let wb = [0xf00e, 0xf00d]
        .iter()
        .map(|&t| ifd.uints(d, t, 3))
        .find(|v| v.len() == 3 && v.iter().all(|&x| x > 0))
        .map(|v| [v[1] as f32, v[0] as f32, v[2] as f32])
        .or(wb_dir.filter(|v| v.iter().all(|&x| x > 0.0)));
    s.wb = wb.map(|v| [v[0] / v[1], 1.0, v[2] / v[1]]);
    // Crop: the camera's own default crop (RawImageCropTopLeft / RawImageCroppedSize).
    if let (Some((ct, cl)), Some((ch, cw))) = (crop_tl, crop_wh)
        && cw > 0
        && ch > 0
        && cl + cw <= w
        && ct + ch <= h
    {
        s.crop = Rect {
            x: cl,
            y: ct,
            w: cw,
            h: ch,
        };
    } else {
        s.crop = Rect { x: 0, y: 0, w, h };
    }
    let px = (w as usize).saturating_mul(h as usize);
    if let Some(fh) = d.get(range.clone()).and_then(crate::fuji::parse_header) {
        if fh.width > w || fh.height > h {
            return Some(s.unsupported("Fujifilm compressed header does not match the raw size"));
        }
        if fh.xtrans != xtrans.is_some() {
            return Some(s.unsupported("Fujifilm CFA type does not match the compressed data"));
        }
        s.bits = fh.bits;
        s.width = fh.width;
        s.height = fh.height;
        s.crop.w = s.crop.w.min(fh.width - s.crop.x.min(fh.width));
        s.crop.h = s.crop.h.min(fh.height - s.crop.y.min(fh.height));
        s.storage = Storage::Fuji {
            range: range.start + crate::fuji::HEADER_LEN..range.end,
            header: fh,
            cfa: pattern,
        };
    } else if len >= px.saturating_mul(2) {
        s.compression = SensorCompression::Uncompressed;
        s.storage = Storage::Blocks {
            blocks: vec![range],
            tile_w: w,
            tile_h: h,
            codec: Codec::Raw {
                packing: if ifd.le {
                    Packing::U16Le
                } else {
                    Packing::U16Be
                },
                row_bytes: 0,
            },
        };
    } else if (10..=16).contains(&s.bits)
        && len
            >= (w as usize * s.bits as usize)
                .div_ceil(32)
                .saturating_mul(4)
                .saturating_mul(h as usize)
    {
        // Bit-packed rows (X-A3/X-A5/...): MSB-first within little-endian 32-bit words.
        s.compression = SensorCompression::Uncompressed;
        s.storage = Storage::Blocks {
            blocks: vec![range],
            tile_w: w,
            tile_h: h,
            codec: Codec::Raw {
                packing: Packing::Msb32Le(s.bits),
                row_bytes: 0,
            },
        };
    } else {
        return Some(s.unsupported("Fujifilm packed or unknown sensor data layout"));
    }
    Some(s)
}

/// Fujifilm SuperCCD RAF: photosites on a 45 degree lattice, 16-bit little-endian, turned
/// upright while decoding (camera RGB, see [`crate::superccd`]).
#[allow(clippy::too_many_arguments)]
fn superccd_sensor(
    d: &[u8],
    mut s: Sensor,
    model: &str,
    (raw_w, raw_h): (u32, u32),
    frame: Option<(u32, u32)>,
    crop: Option<((u32, u32), (u32, u32))>,
    (one_diagonal, lattice): (bool, bool),
    wb: Option<[f32; 3]>,
) -> Sensor {
    if !lattice {
        return s.unsupported("Fujifilm SuperCCD: unknown photosite layout");
    }
    // SR sensors that store a low-sensitivity (R) photosite row after every S row: the S
    // rows make the image (as the camera's own JPEG); the R rows are skipped.
    let sr_rows = model.contains("F700") || model.contains("F710");
    // Two diagonals per row: the camera's crop (0x110 top/left, 0x111 height/width) frames
    // the photosites; one diagonal per row: the frame size (0x121) in unturned coordinates.
    let geom = match (one_diagonal, crop, frame) {
        (false, Some(((t, l), (h, w))), _) => {
            crate::superccd::Geometry::with_margins(raw_w, raw_h, t, l, w, h, false)
        }
        (_, _, Some((h, w))) => crate::superccd::Geometry::new(raw_w, raw_h, w, h, one_diagonal),
        _ => None,
    };
    let geom = geom.and_then(|g| g.with_row_step(if sr_rows { 2 } else { 1 }));
    let Some(geom) = geom else {
        return s.unsupported("Fujifilm SuperCCD: frame size does not fit the raw data");
    };
    let (Some(off), Some(len)) = (be_u32(d, 100), be_u32(d, 104)) else {
        return s.unsupported("Fujifilm SuperCCD: no raw data");
    };
    // The length given is the S image's; with interleaved R rows the data spans twice that.
    let len = if sr_rows {
        (geom.data_len() as u32).max(len)
    } else {
        len
    };
    if (len as usize) < geom.data_len() {
        return s.unsupported("Fujifilm SuperCCD: raw data smaller than the frame");
    }
    s.width = geom.out_w;
    s.height = geom.out_h;
    s.crop = Rect {
        x: 0,
        y: 0,
        w: geom.out_w,
        h: geom.out_h,
    };
    s.cpp = 3;
    s.cfa = None;
    s.bits = 14;
    s.compression = SensorCompression::Uncompressed;
    s.black = Black::Levels([0.0; 4]);
    s.white = Some(f32::from(0x3e00u16));
    s.wb = wb
        .filter(|v| v.iter().all(|&x| x > 0.0))
        .map(|v| [v[0] / v[1], 1.0, v[2] / v[1]]);
    s.storage = Storage::SuperCcd {
        range: clamp_range(d, off as usize, len as usize),
        geom,
    };
    s
}

#[cfg(test)]
mod hostile {
    use super::*;

    /// Header-sized strips (u32::MAX x u32::MAX samples) must not overflow the row arithmetic.
    #[test]
    fn packing_for_huge_dims() {
        let m = u32::MAX;
        for bits in [8, 12, 14, 16] {
            let _ = packing_for(bits, true, m, m, 1, 1 << 20, false);
            let _ = packing_for(bits, true, m, m, m, 1 << 20, true);
        }
    }

    /// A 16384 × 16384 X3F coded thumbnail in 66 KB: decoding it took 0.8 GB, then 1 GB more
    /// for RGBA. Its pixels cannot be in so few bytes, so it is not listed.
    #[test]
    fn x3f_thumbnail_larger_than_its_data_is_not_listed() {
        let mut pv = Previews::default();
        pv.x3f_coded(0..66_000, 16384, 16384, "test");
        assert!(pv.list.is_empty());
        pv.x3f_coded(100_000..166_000, 160, 120, "test");
        assert_eq!(pv.list.len(), 1);
    }

    /// A JPEG at the end of the address space is ignored rather than overflowing its range.
    #[test]
    fn preview_jpeg_at_usize_max() {
        let d = [0u8; 64];
        let mut pv = Previews::default();
        assert!(pv.jpeg(&d, usize::MAX - 1..usize::MAX, "test").is_none());
        assert!(pv.list.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A four-plane CRX `CMP1` header of 6000×4000, 14 bits, encoding `enc`.
    fn cmp1(enc: u8) -> Vec<u8> {
        let mut b = vec![0u8; 40];
        b[4..6].copy_from_slice(&0x100u16.to_be_bytes());
        b[8..12].copy_from_slice(&6000u32.to_be_bytes());
        b[12..16].copy_from_slice(&4000u32.to_be_bytes());
        b[16..20].copy_from_slice(&6000u32.to_be_bytes());
        b[20..24].copy_from_slice(&4000u32.to_be_bytes());
        b[24] = 14;
        b[25] = 4 << 4;
        b[26] = enc << 4;
        b[28..32].copy_from_slice(&64u32.to_be_bytes());
        b
    }

    /// Regression guards for the raw variants refused by name (igapC): signed CRX samples
    /// (encoding 1), and SRF bodies other than the DSC-F828. (Unknown CRX plane layouts are
    /// covered in `crx.rs`, Rawzor in specular-decode's `raw_old_cameras.rs`.)
    #[test]
    fn raw_variants_refused_by_name() {
        let h = crate::crx::parse_cmp1(&cmp1(1)).expect("encoding 1 parses");
        assert!(crx_refusal(&h).unwrap().contains("signed CRX samples"));
        let h = crate::crx::parse_cmp1(&cmp1(0)).expect("encoding 0 parses");
        assert_eq!(crx_refusal(&h), None);
        let s = srf_sensor(&[], "DSC-V3");
        assert!(s.unsupported.unwrap().contains("other than the DSC-F828"));
        // The F828 path is taken (and fails only for the missing key block).
        let s = srf_sensor(&[], "DSC-F828");
        assert!(s.unsupported.unwrap().contains("key block"));
    }

    #[test]
    fn coolpix_p_word_packing_bodies() {
        let full = Some(3664 * 2742 * 3 / 2);
        assert!(coolpix_p_packed("COOLPIX P7000", 3664, 2742, 12, full));
        assert!(coolpix_p_packed(
            "NIKON COOLPIX P7100",
            3664,
            2742,
            12,
            full
        ));
        // The 4032-wide P6000, other Coolpix, 14 bits, rows off a word, short strips.
        assert!(!coolpix_p_packed(
            "COOLPIX P6000",
            4032,
            3024,
            12,
            Some(1 << 30)
        ));
        assert!(!coolpix_p_packed("COOLPIX A", 3664, 2742, 12, full));
        assert!(!coolpix_p_packed("E5700", 3664, 2742, 12, full));
        assert!(!coolpix_p_packed("COOLPIX P7000", 3664, 2742, 14, full));
        assert!(!coolpix_p_packed("COOLPIX P7000", 3662, 2742, 12, full));
        assert!(!coolpix_p_packed(
            "COOLPIX P7000",
            3664,
            2742,
            12,
            Some(1000)
        ));
    }

    #[test]
    fn cr2_cfa_pattern_values() {
        // exiftool's CR2CFAPattern table: 1 RGGB, 2 BGGR, 3 GBRG, 4 GRBG.
        for (v, want) in [
            (None, "RGGB"),
            (Some(1), "RGGB"),
            (Some(2), "BGGR"),
            (Some(3), "GBRG"),
            (Some(4), "GRBG"),
            (Some(9), "RGGB"),
        ] {
            assert_eq!(cr2_cfa_name(v), want);
            assert!(Cfa::bayer(cr2_cfa_name(v)).is_some());
        }
    }
}
