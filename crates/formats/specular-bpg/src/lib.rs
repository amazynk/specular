//! BPG (Better Portable Graphics) decoder for Specular, over the in-tree HEVC decoder.
//!
//! | | Supported |
//! |---|---|
//! | Pixel formats | grey, 4:2:0, 4:2:2, 4:4:4; JPEG and MPEG-2 chroma siting |
//! | Colour | YCbCr BT.601/709/2020 (non-constant luminance), RGB (GBR), YCgCo; full and limited range |
//! | Depth | 8 to 14 bits for stills, 8 to 12 bits in animations (the HEVC decoder's inter limit) |
//! | Alpha | straight and premultiplied (a second, monochrome HEVC stream) |
//! | CMYK | the W plane multiplied into the colour (the specification's sRGB rendering without a profile) |
//! | Animation | P-frame sequences, frame durations (SEI payload 257 × the frame period), loop count |
//! | Extensions | EXIF, ICC, XMP, thumbnail (a nested BPG), animation control |
//!
//! The stream leaves out the VPS and SPS; they are rebuilt from the compact
//! `hevc_header()` ([`ps`]). Colour conversion follows the reference decoder (libbpg 0.9.8)
//! bit for bit: its chroma interpolation filters, fixed-point matrices and alpha division.
//! Output above 8 bits is scaled to the full 16-bit range.
//!
//! Refused, by name:
//! * BT.2020 constant luminance (colour space 5) and more than 14 bits: the BPG specification
//!   reserves both ("not supported in this version"; `bit_depth_minus_8` ≤ 6), libbpg refuses
//!   them and no file uses them;
//! * animations above 12 bits and pictures wider or taller than 65 535 samples: limits of the
//!   shared HEVC decoder (`specular-hevc`: inter prediction up to 12 bits, 16-bit picture
//!   sizes), lifted there or not at all;
//! * reserved pixel formats, and HEVC tools the decoder does not implement (its message is
//!   passed on).
//!
//! Robustness: no input panics; the picture is capped at [`MAX_PIXELS`] and its buffers are
//! reserved from the memory budget ([`specular_core::budget`]). Damaged slices are concealed
//! by the HEVC decoder and reported through [`Image::truncated`].

mod convert;
mod header;
mod ps;
mod stream;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// Largest picture accepted (256 MP).
pub const MAX_PIXELS: u64 = 1 << 28;
/// Largest picture side the HEVC decoder takes.
pub const MAX_SIDE: u32 = 65_535;
/// Largest frame count read from an animation.
pub const MAX_FRAMES: usize = 100_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColourSpace {
    YCbCr601,
    Rgb,
    YCgCo,
    YCbCr709,
    YCbCr2020,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chroma {
    Gray,
    C420,
    C422,
    C444,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaKind {
    None,
    Straight,
    Premultiplied,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Animation {
    /// Times to play; 0 plays forever.
    pub loop_count: u32,
    /// The frame period, `frame_period_num / frame_period_den` seconds.
    pub frame_period_num: u32,
    pub frame_period_den: u32,
}

/// What the header and the extensions say.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    pub chroma: Chroma,
    /// MPEG-2 chroma siting (pixel formats 4 and 5) rather than JPEG's.
    pub video_siting: bool,
    pub bit_depth: u8,
    pub colour_space: ColourSpace,
    pub limited_range: bool,
    pub alpha: AlphaKind,
    /// CMYK: the alpha plane holds W = 1 − K.
    pub cmyk: bool,
    pub animation: Option<Animation>,
    pub icc: Option<Vec<u8>>,
    pub exif: Option<Vec<u8>>,
    pub xmp: Option<Vec<u8>>,
    pub has_thumbnail: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Gray8,
    Rgba8,
    /// Native-endian `u16`, full 16-bit range.
    Gray16,
    Rgba16,
}

impl PixelFormat {
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            PixelFormat::Gray8 => 1,
            PixelFormat::Gray16 => 2,
            PixelFormat::Rgba8 => 4,
            PixelFormat::Rgba16 => 8,
        }
    }
}

/// A decoded frame, straight alpha.
#[derive(Clone, Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub data: Vec<u8>,
    /// The HEVC decoder concealed damaged coding units, or the picture data is cut short.
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Options<'a> {
    /// Worker threads for the HEVC decoder (0 and 1: the calling thread).
    pub threads: usize,
    pub cancel: Option<&'a AtomicBool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Not a BPG file.
    NotBpg,
    /// A valid file using a feature this decoder does not implement.
    Unsupported(String),
    /// The structure needed to reach the pixels is cut short.
    Truncated(String),
    Malformed(String),
    /// Dimensions over the caps.
    TooLarge,
    /// Under the caps, but more than the memory budget has free now.
    NeedsMemory(specular_core::NeedsMemory),
    Cancelled,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotBpg => write!(f, "not a BPG file"),
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Truncated(s) => write!(f, "truncated: {s}"),
            Error::Malformed(s) => write!(f, "malformed: {s}"),
            Error::TooLarge => write!(f, "image too large"),
            Error::NeedsMemory(n) => write!(f, "{n}"),
            Error::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::error::Error for Error {}

impl From<specular_core::NeedsMemory> for Error {
    fn from(n: specular_core::NeedsMemory) -> Self {
        Error::NeedsMemory(n)
    }
}

fn hevc_err(e: hevc::Error) -> Error {
    match e {
        hevc::Error::Cancelled => Error::Cancelled,
        hevc::Error::TooLarge => Error::TooLarge,
        hevc::Error::NeedsMemory(n) => Error::NeedsMemory(n),
        hevc::Error::Unsupported(m) => Error::Unsupported(format!("BPG using HEVC {m}")),
        hevc::Error::Truncated | hevc::Error::NoSlices => {
            Error::Truncated("BPG picture data".into())
        }
        e => Error::Malformed(format!("BPG picture data: {e}")),
    }
}

/// Whether `head` starts with the BPG magic.
pub fn probe(head: &[u8]) -> bool {
    head.starts_with(header::MAGIC)
}

/// The header and extensions.
pub fn read_info(data: &[u8]) -> Result<Info, Error> {
    header::parse(data).map(|h| h.info)
}

/// The parsed parts of a file every frame decode needs.
struct Prepared {
    header: header::Header,
    /// VPS and SPS of the colour stream, and of the alpha stream.
    colour_ps: [Vec<u8>; 2],
    alpha_ps: Option<[Vec<u8>; 2]>,
    units: Vec<stream::AccessUnit>,
}

fn prepare(data: &[u8]) -> Result<Prepared, Error> {
    let header = header::parse(data)?;
    let i = &header.info;
    if i.width > MAX_SIDE || i.height > MAX_SIDE {
        return Err(Error::Unsupported(format!(
            "BPG wider or taller than {MAX_SIDE} pixels"
        )));
    }
    if u64::from(i.width) * u64::from(i.height) > MAX_PIXELS {
        return Err(Error::TooLarge);
    }
    let has_alpha = i.alpha != AlphaKind::None || i.cmyk;
    let mut at = header.hevc.start;
    let data_hevc = &data[..header.hevc.end];
    let alpha_ps = if has_alpha {
        let (h, end) = ps::parse_compact(data_hevc, at)?;
        at = end;
        Some([ps::vps(), ps::sps(&h, i.width, i.height, 0, i.bit_depth)])
    } else {
        None
    };
    let (h, end) = ps::parse_compact(data_hevc, at)?;
    at = end;
    let chroma_idc = match i.chroma {
        Chroma::Gray => 0,
        Chroma::C420 => 1,
        Chroma::C422 => 2,
        Chroma::C444 => 3,
    };
    let colour_ps = [
        ps::vps(),
        ps::sps(&h, i.width, i.height, chroma_idc, i.bit_depth),
    ];
    if at >= header.hevc.end {
        return Err(Error::Truncated("BPG picture data".into()));
    }
    let mut units = stream::access_units(data, at..header.hevc.end, has_alpha);
    if units.is_empty() {
        return Err(Error::Truncated("BPG picture data".into()));
    }
    if i.animation.is_none() {
        units.truncate(1);
    }
    units.truncate(MAX_FRAMES);
    Ok(Prepared {
        header,
        colour_ps,
        alpha_ps,
        units,
    })
}

impl Prepared {
    fn params(&self) -> convert::Params {
        let i = &self.header.info;
        convert::Params {
            width: i.width,
            height: i.height,
            chroma: i.chroma,
            video_siting: i.video_siting,
            bit_depth: i.bit_depth,
            colour_space: i.colour_space,
            limited_range: i.limited_range,
            alpha: if i.cmyk {
                Some(convert::AlphaUse::White)
            } else {
                match i.alpha {
                    AlphaKind::None => None,
                    AlphaKind::Straight => Some(convert::AlphaUse::Straight),
                    AlphaKind::Premultiplied => Some(convert::AlphaUse::Premultiplied),
                }
            },
        }
    }

    /// The NAL units of access unit `u` for one stream (alpha ones with layer id 0).
    fn nals<'d>(
        &self,
        data: &'d [u8],
        u: &stream::AccessUnit,
        alpha: bool,
    ) -> Vec<std::borrow::Cow<'d, [u8]>> {
        u.nals
            .iter()
            .filter(|n| n.alpha == alpha)
            .map(|n| {
                let b = &data[n.range.clone()];
                if alpha {
                    let mut v = b.to_vec();
                    v[0] &= 0xfe;
                    v[1] &= 0x07;
                    std::borrow::Cow::Owned(v)
                } else {
                    std::borrow::Cow::Borrowed(b)
                }
            })
            .collect()
    }

    /// Converts decoded frames to an image.
    fn finish(
        &self,
        colour: &hevc::Frame,
        alpha: Option<&hevc::Frame>,
        truncated: bool,
    ) -> Result<Image, Error> {
        let p = self.params();
        check_frame(colour, p.width, p.height, p.chroma)?;
        if let Some(a) = alpha {
            check_frame(a, p.width, p.height, Chroma::Gray)?;
        }
        let planes = [
            convert::Plane {
                data: &colour.y,
                stride: colour.y_stride as usize,
            },
            convert::Plane {
                data: &colour.cb,
                stride: colour.c_stride as usize,
            },
            convert::Plane {
                data: &colour.cr,
                stride: colour.c_stride as usize,
            },
        ];
        let a = alpha.map(|a| convert::Plane {
            data: &a.y,
            stride: a.y_stride as usize,
        });
        let bpp = if p.bit_depth > 8 { 8 } else { 4 };
        let _r = specular_core::budget::reserve(u64::from(p.width) * u64::from(p.height) * bpp)?;
        let mut data = Vec::new();
        let format = convert::to_pixels(&p, &planes, a.as_ref(), &mut data);
        Ok(Image {
            width: p.width,
            height: p.height,
            format,
            data,
            truncated: truncated || self.header.cut_short,
        })
    }
}

/// The decoded picture covers the BPG picture, at the bit depth it declares.
fn check_frame(f: &hevc::Frame, w: u32, h: u32, chroma: Chroma) -> Result<(), Error> {
    f.validate().map_err(hevc_err)?;
    let want = match chroma {
        Chroma::Gray => hevc::ChromaFormat::Monochrome,
        Chroma::C420 => hevc::ChromaFormat::Yuv420,
        Chroma::C422 => hevc::ChromaFormat::Yuv422,
        Chroma::C444 => hevc::ChromaFormat::Yuv444,
    };
    if f.width < w || f.height < h || f.chroma != want {
        return Err(Error::Malformed(
            "BPG picture data does not match its header".into(),
        ));
    }
    Ok(())
}

fn decode_still(pre: &Prepared, data: &[u8], opts: &Options) -> Result<Image, Error> {
    let u = &pre.units[0];
    let still = |ps: &[Vec<u8>; 2], alpha: bool| -> Result<hevc::Decoded, Error> {
        let nals = pre.nals(data, u, alpha);
        let mut sets: Vec<&[u8]> = vec![&ps[0], &ps[1]];
        let mut slices: Vec<&[u8]> = Vec::new();
        for n in &nals {
            let nut = (n[0] >> 1) & 0x3f;
            if (32..=34).contains(&nut) {
                sets.push(n);
            } else {
                slices.push(n);
            }
        }
        hevc::decode_still_with(
            &sets,
            &slices,
            &hevc::Options {
                threads: opts.threads,
                cancel: opts.cancel,
            },
        )
        .map_err(hevc_err)
    };
    let alpha = match &pre.alpha_ps {
        Some(ps) => Some(still(ps, true)?),
        None => None,
    };
    let colour = still(&pre.colour_ps, false)?;
    let damaged = colour.damaged > 0 || alpha.as_ref().is_some_and(|a| a.damaged > 0);
    pre.finish(&colour.frame, alpha.as_ref().map(|a| &a.frame), damaged)
}

/// Decodes the picture, or an animation's first frame.
pub fn decode(data: &[u8], opts: &Options) -> Result<Image, Error> {
    let pre = prepare(data)?;
    decode_still(&pre, data, opts)
}

/// The thumbnail extension's picture, when the file has one.
pub fn decode_thumbnail(data: &[u8]) -> Option<Result<Image, Error>> {
    let h = header::parse(data).ok()?;
    let r = h.thumbnail?;
    let thumb = &data[r];
    // A thumbnail's own thumbnail is not looked at.
    Some(decode(thumb, &Options::default()))
}

/// The frames of an animation (or the single frame of a still), in order.
pub struct Frames {
    data: Arc<[u8]>,
    pre: Prepared,
    next: usize,
    colour: Option<hevc::VideoDecoder>,
    alpha: Option<hevc::VideoDecoder>,
}

impl Frames {
    pub fn new(data: Arc<[u8]>, threads: usize) -> Result<Frames, Error> {
        let _ = threads; // Frames decode on the calling thread, one picture after another.
        let pre = prepare(&data)?;
        let i = &pre.header.info;
        if i.animation.is_some() && i.bit_depth > 12 {
            return Err(Error::Unsupported(format!(
                "BPG animation at {} bits per sample",
                i.bit_depth
            )));
        }
        let mut f = Frames {
            data,
            pre,
            next: 0,
            colour: None,
            alpha: None,
        };
        f.rewind()?;
        Ok(f)
    }

    pub fn frame_count(&self) -> u32 {
        self.pre.units.len() as u32
    }

    pub fn rewind(&mut self) -> Result<(), Error> {
        let new = |ps: &[Vec<u8>; 2]| {
            hevc::VideoDecoder::new(&[ps[0].as_slice(), ps[1].as_slice()], 1).map_err(hevc_err)
        };
        self.colour = Some(new(&self.pre.colour_ps)?);
        self.alpha = match &self.pre.alpha_ps {
            Some(ps) => Some(new(ps)?),
            None => None,
        };
        self.next = 0;
        Ok(())
    }

    /// The duration of access unit `u`.
    fn duration(&self, u: &stream::AccessUnit) -> Duration {
        match &self.pre.header.info.animation {
            Some(a) => Duration::from_secs_f64(
                f64::from(a.frame_period_num) * f64::from(u.duration)
                    / f64::from(a.frame_period_den),
            ),
            None => Duration::ZERO,
        }
    }

    pub fn next_frame(
        &mut self,
        cancel: Option<&AtomicBool>,
    ) -> Option<Result<(Image, Duration), Error>> {
        let u = self.pre.units.get(self.next)?.clone();
        self.next += 1;
        Some(self.decode_unit(&u, cancel))
    }

    fn decode_unit(
        &mut self,
        u: &stream::AccessUnit,
        cancel: Option<&AtomicBool>,
    ) -> Result<(Image, Duration), Error> {
        let run = |dec: &mut hevc::VideoDecoder, alpha: bool| -> Result<hevc::Decoded, Error> {
            let nals = self.pre.nals(&self.data, u, alpha);
            let refs: Vec<&[u8]> = nals.iter().map(|n| n.as_ref()).collect();
            dec.decode(&refs, cancel).map_err(hevc_err)?;
            match dec.next_output() {
                Some(d) => Ok(d),
                None => {
                    dec.flush();
                    dec.next_output()
                        .ok_or_else(|| Error::Malformed("BPG frame without a picture".into()))
                }
            }
        };
        let alpha = match self.alpha.as_mut() {
            Some(d) => Some(run(d, true)?),
            None => None,
        };
        let colour = run(self.colour.as_mut().expect("rewound"), false)?;
        let damaged = colour.damaged > 0 || alpha.as_ref().is_some_and(|a| a.damaged > 0);
        let img = self
            .pre
            .finish(&colour.frame, alpha.as_ref().map(|a| &a.frame), damaged)?;
        Ok((img, self.duration(u)))
    }
}

#[cfg(test)]
mod tests;
