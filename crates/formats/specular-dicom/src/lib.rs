//! DICOM Part 10 reader for Specular.
//!
//! [`Dicom::parse`] reads the file meta information and the data set (implicit VR little
//! endian, explicit VR little / big endian, deflated explicit VR little endian), with
//! sequences and items of defined and undefined length and encapsulated pixel data (Basic and
//! Extended Offset Tables). Values stay in the file buffer (or the inflated data set) and are
//! read on demand, so callers pass the file bytes back in.
//!
//! Pixel data: native 1/8/16/32-bit samples (BitsStored / HighBit masking, two's complement,
//! planar configuration), MONOCHROME1/2, RGB, PALETTE COLOR (including segmented palettes),
//! YBR_FULL, YBR_FULL_422 and YBR_PARTIAL_*; RLE Lossless; JPEG baseline, extended (12-bit),
//! lossless and the retired processes (progressive, arithmetic, hierarchical; all but
//! arithmetic lossless hierarchical, .66) through `specular-jpeg`; JPEG-LS lossless and near-lossless through
//! `specular-jpegls`; JPEG 2000 and HTJ2K through `specular-jp2`; MPEG-2, H.264 and HEVC video
//! through `specular-mpeg2`, `specular-h264` and `specular-hevc` ([`video`]). SMPTE ST 2110
//! video is refused by name. Encapsulated documents (PDF, CDA, STL / OBJ / MTL) are
//! found by [`document`].
//!
//! Display: [`ModalityFrame`] keeps a frame's stored values with its modality transform
//! (rescale slope / intercept or Modality LUT: Hounsfield units for CT) and renders through a
//! VOI transform (window, VOI LUT, or the value range) to 16-bit grey.
//!
//! Never panics on malformed input; allocations are bounded by the file size or the declared
//! image size (capped at [`pixel::MAX_FRAME_PIXELS`] per frame).

use std::borrow::Cow;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub mod anon;
pub mod calib;
pub mod charset;
mod charset_table;
mod codec;
mod deid_table;
pub mod detect;
pub mod dict;
mod dict_table;
pub mod display;
pub mod document;
#[doc(hidden)]
pub mod fixture;
pub mod graphics;
pub mod gsdf;
pub mod gsps;
pub mod hash;
pub mod kos;
pub mod labels;
pub mod linked;
pub mod mask;
pub mod media;
mod mp4;
pub mod oct;
pub mod orient;
pub mod parse;
pub mod pdi;
pub mod pixel;
mod rle;
pub mod rtdose;
pub mod rtstruct;
pub mod rwvm;
pub mod seg;
pub mod series;
pub mod sr;
pub mod sr_write;
mod summary;
pub mod suv;
pub mod tags;
pub mod tic;
pub mod tmtv;
pub mod ts;
pub mod video;
pub mod volume;
pub mod waveform;
pub mod write;

pub use calib::{Calibration, Spacing, SpacingKind, Uncalibrated, UsRegion};
pub use charset::Charset;
pub use display::{Image, Modality, ModalityFrame, Rendered, Voi, VoiFunction, VoiSource};
pub use graphics::Graphics;
pub use orient::{Anatomy, PatientAxes, Region};
pub use parse::{DataSet, Element, Fragments, Values, sniff};
pub use pixel::{Frame, Grey, Photometric, PixelInfo, Pixels};
pub use rwvm::{RealWorld, RealWorldFunction, RealWorldValue};
pub use series::{Instance, Series, SeriesItem, SeriesOrder};
pub use summary::{Nde, Summary};
pub use tags::{TagRow, TagTree, TagValue};
pub use ts::{Compression, Encoding, TransferSyntax};
pub use volume::{Geometry, Volume, VoxelKind, Voxels};

use display::Mapping;
use parse::PIXEL_DATA;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("not a DICOM file")]
    NotDicom,
    #[error("truncated DICOM data")]
    Truncated,
    #[error("malformed DICOM data: {0}")]
    Malformed(&'static str),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("no image in this DICOM file")]
    NoPixelData,
    #[error("image too large ({0} bytes)")]
    TooLarge(u64),
    /// Under the caps, but more than the memory budget has free now.
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
    #[error("pixel data codec: {0}")]
    Codec(String),
    #[error("decode cancelled")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Largest inflated data set (Deflated Explicit VR Little Endian).
const MAX_INFLATED: usize = 1 << 30;

const TRANSFER_SYNTAX_UID: u32 = 0x0002_0010;
const EXTENDED_OFFSET_TABLE: u32 = 0x7FE0_0001;
const ICON_IMAGE_SEQUENCE: u32 = 0x0088_0200;
const NUMBER_OF_FRAMES: u32 = 0x0028_0008;

/// A parsed DICOM file.
#[derive(Clone, Debug)]
pub struct Dicom {
    /// The file meta information (empty for files without one).
    pub meta: DataSet,
    pub dataset: DataSet,
    pub transfer_syntax: TransferSyntax,
    /// The image, when the data set has pixel data.
    pub pixel: Option<PixelInfo>,
    /// The data set ended early.
    pub truncated: bool,
    inflated: Option<Vec<u8>>,
    /// The memory budget's share of `inflated`, held as long as it (shared by clones).
    _inflated_res: Option<Arc<specular_core::Reservation>>,
    /// Fragment index range of each frame (encapsulated pixel data).
    frame_map: Vec<Range<usize>>,
    palette: Option<[pixel::Lut; 3]>,
    /// The indexed stream of H.264 / HEVC pixel data, or why it cannot be decoded.
    video: Option<std::result::Result<Arc<video::Video>, Error>>,
}

/// A deflated data set that does not inflate, or does not fit the memory budget.
pub(crate) fn inflate_err(e: specular_png::InflateError) -> Error {
    match e {
        specular_png::InflateError::NeedsMemory(n) => Error::NeedsMemory(n),
        _ => Error::Malformed("deflated data set does not inflate"),
    }
}

fn cancelled(c: Option<&AtomicBool>) -> Result<()> {
    if c.is_some_and(|c| c.load(Ordering::Relaxed)) {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

impl Dicom {
    /// Parses the file. Fails only when it is not DICOM at all or the data set cannot be read;
    /// a file without pixel data parses with `pixel: None`.
    pub fn parse(data: &[u8]) -> Result<Dicom> {
        let layout = parse::split(data)?;
        let meta_v = Values {
            buf: data,
            big: false,
        };
        let uid = layout
            .meta
            .get(TRANSFER_SYNTAX_UID)
            .and_then(|e| std::str::from_utf8(meta_v.text_bytes(e)).ok());
        let mut ts = uid.map_or_else(TransferSyntax::implicit_little, TransferSyntax::from_uid);
        let big = ts.encoding == Encoding::ExplicitBig;
        let (dataset, truncated, inflated) = match ts.encoding {
            Encoding::DeflatedLittle => {
                let body = data.get(layout.body..).unwrap_or(&[]);
                let cap = body.len().saturating_mul(1100).min(MAX_INFLATED);
                let inflated = specular_png::inflate_raw(body, cap).map_err(inflate_err)?;
                let (ds, t, _) = parse::body(&inflated, 0, true, false)?;
                (ds, t, Some(inflated))
            }
            enc => {
                let explicit = enc != Encoding::ImplicitLittle;
                let (ds, t, used_explicit) = parse::body(data, layout.body, explicit, big)?;
                if uid.is_none() && used_explicit {
                    ts = TransferSyntax::from_uid("1.2.840.10008.1.2.1");
                }
                (ds, t, None)
            }
        };
        if dataset.elements.is_empty() && layout.meta.elements.is_empty() {
            return Err(Error::NotDicom);
        }
        // The inflater's reservation ended with it; the data set lives on in `inflated`.
        let inflated_res = match &inflated {
            Some(b) => Some(Arc::new(specular_core::budget::reserve(b.len() as u64)?)),
            None => None,
        };
        let v = Values {
            buf: inflated.as_deref().unwrap_or(data),
            big,
        };
        let (mut pixel, mut palette, mut map, mut vid) = (None, None, Vec::new(), None);
        if let Some((pd, float)) = pixel_element(&dataset) {
            let mut info = PixelInfo::read_as(&dataset, &v, float)?;
            if info.photometric == Photometric::Palette {
                palette = Some(pixel::palette(&dataset, &v, info.signed)?);
            }
            if let Some(codec) = video::codec(&ts) {
                let indexed = match &pd.fragments {
                    Some(f) => video::Video::index(codec, f, &v, info.frames).map(Arc::new),
                    None => Err(Error::Malformed("video syntax with native pixel data")),
                };
                if let Ok(s) = &indexed {
                    // Number of Frames is required, but the stream is what plays.
                    let n = s.frames().max(1);
                    if dataset.get(NUMBER_OF_FRAMES).is_none() || info.frames > n {
                        info.frames = n;
                    }
                }
                vid = Some(indexed);
            } else if let Some(f) = &pd.fragments {
                let eot = dataset
                    .get(EXTENDED_OFFSET_TABLE)
                    .map(|e| v.u64s(e).collect::<Vec<_>>());
                map = frame_map(f, &v, eot.as_deref(), info.frames);
            }
            pixel = Some(info);
        }
        let d = Dicom {
            meta: layout.meta,
            dataset,
            transfer_syntax: ts,
            pixel,
            truncated,
            inflated,
            _inflated_res: inflated_res,
            frame_map: map,
            palette,
            video: vid,
        };
        Ok(d)
    }

    /// The buffer the data set's values are in: the file, or the inflated data set.
    pub fn buf<'a>(&'a self, file: &'a [u8]) -> &'a [u8] {
        self.inflated.as_deref().unwrap_or(file)
    }

    /// Value access for [`Dicom::dataset`].
    pub fn values<'a>(&'a self, file: &'a [u8]) -> Values<'a> {
        Values {
            buf: self.buf(file),
            big: self.transfer_syntax.encoding == Encoding::ExplicitBig,
        }
    }

    /// Number of frames (pages).
    pub fn frame_count(&self) -> u32 {
        self.pixel.as_ref().map_or(0, |p| p.frames)
    }

    /// Whether the pixel data can be decoded (`Err` names the reason it cannot).
    pub fn check_supported(&self) -> Result<()> {
        if self.pixel.is_none() {
            return Err(Error::NoPixelData);
        }
        match self.transfer_syntax.compression {
            Compression::Video => match &self.video {
                Some(Ok(_)) => Ok(()),
                Some(Err(e)) => Err(e.clone()),
                None => Err(Error::Unsupported(video::refusal(&self.transfer_syntax))),
            },
            Compression::Other => Err(Error::Unsupported(format!(
                "{} pixel data",
                self.transfer_syntax.name
            ))),
            _ => Ok(()),
        }
    }

    /// Decodes frame `index` to stored values (grey) or RGB.
    pub fn decode_frame(
        &self,
        file: &[u8],
        index: u32,
        cancel: Option<&AtomicBool>,
    ) -> Result<Frame> {
        let mut f = self.decode_stored(file, index, cancel)?;
        if let Some(p) = &self.palette {
            pixel::apply_palette(&mut f, p)?;
        }
        Ok(f)
    }

    /// Decodes frame `index` like [`Dicom::decode_frame`], but leaves PALETTE COLOR images as
    /// their stored indices (grey).
    pub fn decode_stored(
        &self,
        file: &[u8],
        index: u32,
        cancel: Option<&AtomicBool>,
    ) -> Result<Frame> {
        self.check_supported()?;
        let info = self.pixel.as_ref().ok_or(Error::NoPixelData)?;
        if index >= info.frames {
            return Err(Error::Malformed("frame index out of range"));
        }
        // The frame's samples, through the decode.
        let _res = specular_core::budget::reserve_n(
            info.pixels(),
            u64::from(info.samples.max(1)) * u64::from(info.bits_allocated.div_ceil(8).max(1)),
        )?;
        if let Some(Ok(vid)) = &self.video {
            cancelled(cancel)?;
            return vid.frame(self.buf(file), info, index, cancel);
        }
        let (e, _) = pixel_element(&self.dataset).ok_or(Error::NoPixelData)?;
        let v = self.values(file);
        decode_pixels(
            info,
            e,
            &v,
            &self.transfer_syntax,
            &self.frame_map,
            index,
            cancel,
        )
    }

    /// Decodes frame `index` with its display mapping.
    pub fn modality_frame(
        &self,
        file: &[u8],
        index: u32,
        cancel: Option<&AtomicBool>,
    ) -> Result<ModalityFrame> {
        let frame = self.decode_frame(file, index, cancel)?;
        self.modality_of(file, index, frame)
    }

    /// Frame `index`, already decoded (as [`Dicom::decode_frame`] gives it), with its display
    /// mapping.
    pub fn modality_of(&self, file: &[u8], index: u32, frame: Frame) -> Result<ModalityFrame> {
        let info = self.pixel.as_ref().ok_or(Error::NoPixelData)?;
        let grey = matches!(frame.pixels, Pixels::Grey(_));
        let v = self.values(file);
        let map = Mapping::read(&self.dataset, &v, index, info.signed, grey, info.float);
        let graphics = graphics::read(&self.dataset, &v, info, index, frame.width, frame.height);
        let calibration = Calibration::read(&self.dataset, &v, index);
        let axes = PatientAxes::read(&self.dataset, &v, index);
        let sop_uid = self
            .dataset
            .get(0x0008_0018)
            .and_then(|e| std::str::from_utf8(v.text_bytes(e)).ok())
            .map(|s| s.trim_end_matches('\0').trim().to_owned())
            .filter(|s| !s.is_empty());
        let rwvm = if grey {
            rwvm::read(&self.dataset, &v, index, info.signed)
        } else {
            Vec::new()
        };
        Ok(ModalityFrame {
            frame,
            map,
            graphics,
            calibration,
            axes,
            sop_uid,
            presentation: None,
            rwvm,
        })
    }

    /// The indexed H.264 / HEVC stream, when the pixel data is decodable video.
    pub fn video(&self) -> Option<&Arc<video::Video>> {
        self.video.as_ref()?.as_ref().ok()
    }

    /// Front-to-back decoding of the video frames (with its own decoder), for cine.
    pub fn video_sequence(&self) -> Option<video::Sequence> {
        let v = self.video()?.clone();
        Some(video::Sequence::new(v, self.pixel.clone()?))
    }

    /// Bytes of decoded video frames kept for stepping.
    pub fn video_memory(&self) -> usize {
        self.video().map_or(0, |v| v.memory_bytes())
    }

    /// Drops the decoded video frames kept for stepping and the decoder; returns their bytes.
    pub fn trim_video(&self) -> usize {
        self.video().map_or(0, |v| v.trim())
    }

    /// How long each frame shows in cine (Frame Time Vector, Frame Time, Cine Rate or
    /// Recommended Display Frame Rate); `None` when the file states no timing.
    pub fn frame_durations(&self, file: &[u8]) -> Option<Vec<std::time::Duration>> {
        calib::frame_durations(&self.dataset, &self.values(file), self.frame_count())
    }

    /// The Icon Image Sequence's image, rendered with its own (or the value range) VOI.
    pub fn icon(&self, file: &[u8]) -> Option<Result<Image>> {
        self.icon_in(&self.dataset, file)
    }

    /// The image of the Icon Image Sequence in `parent` (the data set or one of its items, such
    /// as a DICOMDIR record).
    pub fn icon_in(&self, parent: &DataSet, file: &[u8]) -> Option<Result<Image>> {
        let item = parent.item(ICON_IMAGE_SEQUENCE)?;
        let e = item.get(PIXEL_DATA)?;
        let v = self.values(file);
        Some((|| {
            let info = PixelInfo::read(item, &v)?;
            let map_frames = match &e.fragments {
                Some(f) => frame_map(f, &v, None, 1),
                None => Vec::new(),
            };
            let ts = if e.fragments.is_some() {
                self.transfer_syntax.clone()
            } else {
                TransferSyntax::from_uid("1.2.840.10008.1.2.1")
            };
            let mut frame = decode_pixels(&info, e, &v, &ts, &map_frames, 0, None)?;
            if info.photometric == Photometric::Palette {
                pixel::apply_palette(&mut frame, &pixel::palette(item, &v, info.signed)?)?;
            }
            let grey = matches!(frame.pixels, Pixels::Grey(_));
            let map = Mapping::read(item, &v, 0, info.signed, grey, info.float);
            ModalityFrame {
                frame,
                map,
                graphics: None,
                calibration: Calibration::default(),
                axes: None,
                sop_uid: None,
                presentation: None,
                rwvm: Vec::new(),
            }
            .render_default()
        })())
    }

    /// The info panel summary (no patient-identifying attributes).
    pub fn summary(&self, file: &[u8]) -> Summary {
        let v = self.values(file);
        let r = summary::Reader {
            ds: &self.dataset,
            v,
            cs: Charset::of(&self.dataset, &v),
        };
        let mut s = summary::read(&r, None);
        if let Some(p) = &self.pixel {
            s.columns = p.columns;
            s.rows = p.rows;
            s.frames = p.frames;
            s.bits_stored = p.bits_stored;
            s.photometric = p.photometric.name().to_owned();
        }
        s.transfer_syntax = self.transfer_syntax.name.clone();
        s.lossy |= self.transfer_syntax.lossy;
        s
    }

    /// Peak bytes of decoding and rendering one frame (stored values, codec output and the
    /// rendered image).
    pub fn frame_memory(&self) -> u64 {
        let Some(p) = &self.pixel else { return 0 };
        let n = p.pixels();
        let stored = if p.is_colour() {
            n * 3
                * if p.bits_stored > 8 || p.photometric == Photometric::Palette {
                    2
                } else {
                    1
                }
        } else if p.bits_stored > 16 {
            n * 4
        } else {
            n * 2
        };
        let rendered = if p.is_colour() { n * 8 } else { n * 2 };
        let codec = match self.transfer_syntax.compression {
            Compression::Native => 0,
            Compression::Rle | Compression::EncapsulatedNative | Compression::DeflatedFrame => {
                p.frame_bits() / 8
            }
            _ => n * 4 * p.samples as u64,
        };
        stored + rendered + codec
    }
}

/// Bytes of encapsulated frame `index`: one fragment borrowed, several joined.
fn frame_bytes<'a>(
    f: &Fragments,
    v: &Values<'a>,
    map: &[Range<usize>],
    index: u32,
) -> Result<Cow<'a, [u8]>> {
    let r = map.get(index as usize).cloned().ok_or(Error::Truncated)?;
    let frags = f.items.get(r).ok_or(Error::Truncated)?;
    match frags {
        [] => Err(Error::Truncated),
        [one] => Ok(Cow::Borrowed(v.buf.get(one.clone()).unwrap_or(&[]))),
        many => {
            let mut out = Vec::new();
            for r in many {
                out.extend_from_slice(v.buf.get(r.clone()).unwrap_or(&[]));
            }
            Ok(Cow::Owned(out))
        }
    }
}

/// The pixel data element: Pixel Data, else Float or Double Float Pixel Data (`true`).
fn pixel_element(ds: &DataSet) -> Option<(&Element, bool)> {
    if let Some(e) = ds.get(PIXEL_DATA) {
        return Some((e, false));
    }
    ds.get(parse::FLOAT_PIXEL_DATA)
        .or_else(|| ds.get(parse::DOUBLE_FLOAT_PIXEL_DATA))
        .map(|e| (e, true))
}

fn decode_pixels(
    info: &PixelInfo,
    e: &Element,
    v: &Values,
    ts: &TransferSyntax,
    map: &[Range<usize>],
    index: u32,
    cancel: Option<&AtomicBool>,
) -> Result<Frame> {
    cancelled(cancel)?;
    let Some(frags) = &e.fragments else {
        if ts.encapsulated() && ts.compression != Compression::Other {
            return Err(Error::Malformed("compressed syntax with native pixel data"));
        }
        let src = pixel::native_frame(info, e, v, index)?;
        return pixel::unpack(info, &src);
    };
    let data = frame_bytes(frags, v, map, index)?;
    let f = match ts.compression {
        Compression::Native | Compression::EncapsulatedNative => pixel::unpack(
            info,
            &pixel::Native {
                data: &data,
                bit_offset: 0,
                big: false,
                swap8: false,
                planar: info.planar,
            },
        )?,
        Compression::DeflatedFrame => {
            let want = info.frame_bits().div_ceil(8) as usize;
            let bytes = specular_png::inflate_raw(&data, want).map_err(|e| match e {
                specular_png::InflateError::NeedsMemory(n) => Error::NeedsMemory(n),
                e => Error::Codec(format!("deflated frame: {e}")),
            })?;
            let mut f = pixel::unpack(
                info,
                &pixel::Native {
                    data: &bytes,
                    bit_offset: 0,
                    big: false,
                    swap8: false,
                    planar: info.planar,
                },
            )?;
            f.truncated |= bytes.len() < want;
            f
        }
        Compression::Rle => {
            let (bytes, truncated) = rle::decode(&data, info)?;
            let mut f = pixel::unpack(
                info,
                &pixel::Native {
                    data: &bytes,
                    bit_offset: 0,
                    big: false,
                    swap8: false,
                    planar: true,
                },
            )?;
            f.truncated |= truncated;
            f
        }
        Compression::JpegDct | Compression::JpegLossless | Compression::JpegRetired => {
            codec::jpeg(&data, info, cancel)?
        }
        Compression::Jpeg2000 | Compression::Htj2k => codec::jpeg2000(&data, info, cancel)?,
        Compression::JpegLs => codec::jpegls(&data, info, cancel)?,
        Compression::JpegXl => codec::jpegxl(&data, info, cancel)?,
        _ => return Err(Error::Unsupported(format!("{} pixel data", ts.name))),
    };
    Ok(f)
}

/// Starts of codestreams: JPEG SOI, JPEG 2000 SOC + SIZ, a JP2 signature box, a JPEG XL
/// codestream or container.
fn starts_codestream(b: &[u8]) -> bool {
    b.starts_with(&[0xFF, 0xD8, 0xFF])
        || b.starts_with(&[0xFF, 0x4F, 0xFF, 0x51])
        || b.get(4..8) == Some(b"jP  ")
        || b.starts_with(&[0xFF, 0x0A])
        || b.get(4..8) == Some(b"JXL ")
}

/// Maps frames to fragments: the Extended Offset Table, else the Basic Offset Table, else one
/// frame for everything, one fragment per frame, or codestream starts.
fn frame_map(f: &Fragments, v: &Values, eot: Option<&[u64]>, frames: u32) -> Vec<Range<usize>> {
    let n = f.items.len();
    let frames = frames.max(1) as usize;
    let from_offsets = |offs: &[u64]| -> Option<Vec<Range<usize>>> {
        if offs.len() != frames || !offs.windows(2).all(|w| w[0] < w[1]) {
            return None;
        }
        let starts: Vec<usize> = offs
            .iter()
            .map(|&o| f.item_offsets.partition_point(|&x| x < o))
            .collect();
        if starts.iter().any(|&s| s >= n) || !starts.windows(2).all(|w| w[0] < w[1]) {
            return None;
        }
        Some(
            starts
                .iter()
                .enumerate()
                .map(|(i, &s)| s..starts.get(i + 1).copied().unwrap_or(n))
                .collect(),
        )
    };
    if let Some(m) = eot.and_then(from_offsets) {
        return m;
    }
    let bot = v.buf.get(f.offset_table.clone()).unwrap_or(&[]);
    if !bot.is_empty() {
        let offs: Vec<u64> = bot
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as u64)
            .collect();
        // The first offset is 0; tables of a single 0 are common for single frames.
        if let Some(m) = from_offsets(&offs) {
            return m;
        }
    }
    if frames == 1 {
        #[allow(clippy::single_range_in_vec_init)] // one frame of all `n` fragments
        return vec![0..n];
    }
    if n == frames {
        return (0..n).map(|i| i..i + 1).collect();
    }
    let starts: Vec<usize> = (0..n)
        .filter(|&i| starts_codestream(v.buf.get(f.items[i].clone()).unwrap_or(&[])))
        .collect();
    if !starts.is_empty() && starts[0] == 0 {
        return starts
            .iter()
            .enumerate()
            .map(|(i, &s)| s..starts.get(i + 1).copied().unwrap_or(n))
            .take(frames)
            .collect();
    }
    (0..n.min(frames)).map(|i| i..i + 1).collect()
}

#[cfg(test)]
mod hostile_tests;
#[cfg(test)]
mod series_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod video_tests;
