//! Old video codecs whose patents have long expired, decoded from their public descriptions:
//!
//! - QuickTime Animation (`rle `): run lengths per line, 8-bit palette or greyscale, RGB555,
//!   RGB and ARGB; frames that change some lines only.
//! - Cinepak (`cvid`): vector quantisation in strips with V1/V4 codebooks, colour or grey.
//! - Microsoft Video 1 (`CRAM`, `MSVC`): 4×4 blocks of one, two or eight colours, palette or
//!   RGB555.
//! - Microsoft RLE (BI_RLE8, BI_RLE4): run lengths with deltas, 8- and 4-bit palettes.
//! - Apple SMC (`smc `, QuickTime Graphics): 4×4 palette blocks with colour caches.
//! - Apple Video (`rpza`): 4×4 RGB555 blocks, one, four or sixteen colours.
//! - Flash Screen Video 1 and 2 (FLV codecs 3 and 6): zlib-compressed blocks of BGR or
//!   15/7-bit hybrid colours ([`flashsv`]).
//! - PlayStation MDEC pictures (`.str` movies, BS versions 1 to 3; [`mdec`]).
//!
//! Most frames change only part of the picture, so a decoder keeps its canvas between frames;
//! the canvas is in the codec's own pixel format ([`Pixels`]): palette indices, RGB555 or
//! 8-bit RGB.

mod cdg;
mod cinepak;
mod flashsv;
pub mod mdec;
mod msrle;
mod msvideo1;
mod qtrle;
mod rpza;
mod smc;

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Malformed(String),
    Unsupported(String),
    NeedsMemory(specular_core::NeedsMemory),
}

impl From<specular_core::NeedsMemory> for Error {
    fn from(n: specular_core::NeedsMemory) -> Self {
        Error::NeedsMemory(n)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Malformed(s) => write!(f, "malformed frame: {s}"),
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::NeedsMemory(n) => write!(f, "frame: {n}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn bad(s: impl Into<String>) -> Error {
    Error::Malformed(s.into())
}

/// The codecs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    QtRle,
    Cinepak,
    MsVideo1,
    MsRle,
    Smc,
    Rpza,
    FlashSv1,
    FlashSv2,
    /// CD+G karaoke graphics (300×216, 16 of 4096 colours).
    Cdg,
    /// PlayStation MDEC (BS) pictures, every frame a key frame.
    Mdec,
}

impl Codec {
    pub fn name(self) -> &'static str {
        match self {
            Codec::QtRle => "QuickTime Animation",
            Codec::Cinepak => "Cinepak",
            Codec::MsVideo1 => "Microsoft Video 1",
            Codec::MsRle => "Microsoft RLE",
            Codec::Smc => "QuickTime Graphics (SMC)",
            Codec::Rpza => "Apple Video (RPZA)",
            Codec::FlashSv1 => "Flash Screen Video",
            Codec::FlashSv2 => "Flash Screen Video 2",
            Codec::Cdg => "CD+G graphics",
            Codec::Mdec => "PlayStation MDEC",
        }
    }
}

/// A canvas in the codec's pixel format, rows top to bottom, `width` pixels each.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pixels {
    /// Palette indices.
    Pal8(Vec<u8>),
    /// `xRRRRRGGGGGBBBBB`.
    Rgb555(Vec<u16>),
    /// R, G, B bytes.
    Rgb24(Vec<u8>),
    /// A, R, G, B bytes.
    Argb(Vec<u8>),
}

/// The output format a codec at a depth decodes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Pal8,
    Rgb555,
    Rgb24,
    Argb,
}

impl Format {
    fn canvas(self, n: usize) -> Pixels {
        match self {
            Format::Pal8 => Pixels::Pal8(vec![0; n]),
            Format::Rgb555 => Pixels::Rgb555(vec![0; n]),
            Format::Rgb24 => Pixels::Rgb24(vec![0; 3 * n]),
            Format::Argb => Pixels::Argb(vec![0; 4 * n]),
        }
    }
}

/// The decoder of one track: the codec, the picture size and depth, and the canvas.
pub struct Decoder {
    codec: Codec,
    width: usize,
    height: usize,
    depth: u16,
    format: Format,
    canvas: Pixels,
    /// SMC's colour caches, Cinepak's codebooks.
    state: State,
}

enum State {
    None,
    Cinepak(Box<cinepak::Books>),
    Smc(Box<smc::Caches>),
    FlashSv(Box<flashsv::State>),
    Cdg(Box<cdg::Screen>),
}

/// The format a codec decodes to at a depth, or why it is refused.
fn format_of(codec: Codec, depth: u16) -> Result<Format> {
    let refuse = |what: &str| Err(Error::Unsupported(format!("{} {what}", codec.name())));
    Ok(match codec {
        Codec::QtRle => match depth {
            8 | 40 => Format::Pal8,
            16 => Format::Rgb555,
            24 => Format::Rgb24,
            32 => Format::Argb,
            1 | 2 | 4 | 33..=36 => return refuse(&format!("at {} bits", depth % 32)),
            _ => return refuse(&format!("at {depth} bits")),
        },
        // At 8 bits the codebooks hold palette indices.
        Codec::Cinepak => match depth {
            8 => Format::Pal8,
            _ => Format::Rgb24,
        },
        Codec::MsVideo1 => match depth {
            8 => Format::Pal8,
            16 | 15 | 0 => Format::Rgb555,
            _ => return refuse(&format!("at {depth} bits")),
        },
        Codec::MsRle => match depth {
            4 | 8 => Format::Pal8,
            _ => return refuse(&format!("at {depth} bits")),
        },
        Codec::Smc => match depth {
            8 | 40 => Format::Pal8,
            _ => return refuse(&format!("at {depth} bits")),
        },
        Codec::Rpza => Format::Rgb555,
        Codec::FlashSv1 | Codec::FlashSv2 | Codec::Cdg | Codec::Mdec => Format::Rgb24,
    })
}

/// Why a codec at a depth can't be decoded, in words.
pub fn refusal(codec: Codec, depth: u16) -> Option<String> {
    match format_of(codec, depth) {
        Err(Error::Unsupported(m)) => Some(m),
        _ => None,
    }
}

impl Decoder {
    /// A decoder for `width`×`height` pictures at `depth` bits (the container's bit count;
    /// QuickTime's 40 is 8-bit greyscale).
    pub fn new(codec: Codec, width: u32, height: u32, depth: u16) -> Result<Decoder> {
        if width == 0 || height == 0 || width > 16_384 || height > 16_384 {
            return Err(bad(format!("picture size {width}×{height}")));
        }
        let format = format_of(codec, depth)?;
        let (width, height) = (width as usize, height as usize);
        let _held = specular_core::budget::reserve((width * height * 4) as u64)?;
        Ok(Decoder {
            codec,
            width,
            height,
            depth,
            format,
            canvas: format.canvas(width * height),
            state: match codec {
                Codec::Cinepak => State::Cinepak(Box::default()),
                Codec::Smc => State::Smc(Box::default()),
                Codec::FlashSv1 | Codec::FlashSv2 => State::FlashSv(Box::default()),
                Codec::Cdg => State::Cdg(Box::default()),
                _ => State::None,
            },
        })
    }

    /// Back to a blank canvas (decoding goes on from a keyframe).
    pub fn reset(&mut self) {
        self.canvas = self.format.canvas(self.width * self.height);
        match &mut self.state {
            State::Cinepak(b) => **b = cinepak::Books::default(),
            State::Smc(c) => **c = smc::Caches::default(),
            State::FlashSv(s) => **s = flashsv::State::default(),
            State::Cdg(c) => **c = cdg::Screen::default(),
            State::None => {}
        }
    }

    /// Decodes one frame onto the canvas and gives the picture.
    pub fn decode(&mut self, data: &[u8]) -> Result<&Pixels> {
        let (w, h) = (self.width, self.height);
        match self.codec {
            Codec::QtRle => qtrle::decode(data, w, h, self.depth, &mut self.canvas)?,
            Codec::Cinepak => {
                let State::Cinepak(books) = &mut self.state else {
                    return Err(bad("state"));
                };
                cinepak::decode(data, w, h, books, &mut self.canvas)?
            }
            Codec::MsVideo1 => msvideo1::decode(data, w, h, &mut self.canvas)?,
            Codec::MsRle => msrle::decode(data, w, h, self.depth, &mut self.canvas)?,
            Codec::Smc => {
                let State::Smc(caches) = &mut self.state else {
                    return Err(bad("state"));
                };
                smc::decode(data, w, h, caches, &mut self.canvas)?
            }
            Codec::Rpza => rpza::decode(data, w, h, &mut self.canvas)?,
            Codec::FlashSv1 | Codec::FlashSv2 => {
                let State::FlashSv(st) = &mut self.state else {
                    return Err(bad("state"));
                };
                let v2 = self.codec == Codec::FlashSv2;
                flashsv::decode(data, w, h, v2, st, &mut self.canvas)?
            }
            Codec::Cdg => {
                let State::Cdg(screen) = &mut self.state else {
                    return Err(bad("state"));
                };
                cdg::decode(data, screen, &mut self.canvas)?
            }
            Codec::Mdec => {
                let Pixels::Rgb24(px) = &mut self.canvas else {
                    return Err(bad("canvas"));
                };
                mdec::decode(data, w, h, px)?
            }
        }
        Ok(&self.canvas)
    }

    pub fn width(&self) -> u32 {
        self.width as u32
    }

    pub fn height(&self) -> u32 {
        self.height as u32
    }
}

/// A byte reader that fails cleanly at the end.
pub(crate) struct Reader<'a> {
    pub data: &'a [u8],
    pub at: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Reader<'a> {
        Reader { data, at: 0 }
    }
    pub fn left(&self) -> usize {
        self.data.len().saturating_sub(self.at)
    }
    pub fn u8(&mut self) -> Result<u8> {
        let b = *self.data.get(self.at).ok_or_else(|| bad("truncated"))?;
        self.at += 1;
        Ok(b)
    }
    pub fn u16be(&mut self) -> Result<u16> {
        Ok(u16::from(self.u8()?) << 8 | u16::from(self.u8()?))
    }
    pub fn u16le(&mut self) -> Result<u16> {
        Ok(u16::from(self.u8()?) | u16::from(self.u8()?) << 8)
    }
    pub fn u24be(&mut self) -> Result<u32> {
        Ok(u32::from(self.u8()?) << 16 | u32::from(self.u16be()?))
    }
    pub fn u32be(&mut self) -> Result<u32> {
        Ok(u32::from(self.u16be()?) << 16 | u32::from(self.u16be()?))
    }
    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let s = self
            .data
            .get(self.at..self.at + n)
            .ok_or_else(|| bad("truncated"))?;
        self.at += n;
        Ok(s)
    }
    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.bytes(n).map(|_| ())
    }
}
