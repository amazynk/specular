//! SMPTE VC-2 (ST 2042-1) video, the intra-only professional subset of BBC Dirac, decoded from
//! the public Dirac specification (version 2.2.3, BBC 2008) and the VC-2 additions:
//!
//! - low-delay pictures (parse codes 0xC8, 0xCC): fixed-size slices of exp-Golomb coefficients
//!   under a quantisation matrix, the DC band intra-predicted (§13.3);
//! - high-quality pictures (0xE8, 0xEC): slices with prefix bytes and per-component lengths
//!   scaled by the slice size scaler;
//! - all seven wavelet filters (Deslauriers-Dubuc 9/7 and 13/7, LeGall 5/3, Haar with and
//!   without shift, Fidelity, Daubechies 9/7), any transform depth with a custom matrix, the
//!   default matrices of Annex E.1 up to four levels;
//! - 4:4:4, 4:2:2 and 4:2:0 at 1 to 16 bits; frame and field coding (two fields are woven
//!   into one frame);
//! - the predefined video formats of Annex C as the base for every sequence header.
//!
//! A stream is a sequence of data units, each behind a 13-byte parse info header ("BBCD", the
//! parse code, the offsets to the next and previous headers). Refused by name: the Dirac core
//! syntax (arithmetic-coded and motion-compensated pictures, codes 0x08–0x0E and 0x48/0x4C;
//! no encoder is available to check a decoder against) and VC-2 version 3 asymmetric
//! transforms.

mod bits;
mod picture;
mod seq;
mod wavelet;

use std::fmt;

use seq::Sequence;

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
            Error::Malformed(s) => write!(f, "malformed VC-2 stream: {s}"),
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::NeedsMemory(n) => write!(f, "VC-2 picture: {n}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn bad(s: impl Into<String>) -> Error {
    Error::Malformed(s.into())
}

/// Chroma sampling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chroma {
    C444,
    C422,
    C420,
}

/// The colour description of a sequence as H.273 code points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Signal {
    pub full_range: bool,
    pub primaries: Option<u8>,
    pub transfer: Option<u8>,
    pub matrix: Option<u8>,
}

/// A decoded frame. Planes are tightly packed Y, Cb, Cr as unsigned samples of `bit_depth`
/// bits (the luma depth; chroma is at its own depth, which differs only for YCoCg ranges).
/// Chroma planes are `ceil(width / 2)` wide for 4:2:2 and 4:2:0 (and `ceil(height / 2)` high
/// for 4:2:0): a coded chroma plane is rounded down, so its last column or row repeats.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub chroma: Chroma,
    pub bit_depth: u8,
    pub planes: [Vec<u16>; 3],
    pub signal: Signal,
    /// The picture number (of the first field, for field coding).
    pub number: u32,
}

/// What a stream's sequence header (and first picture) say.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    pub chroma: Chroma,
    pub bit_depth: u8,
    /// "VC-2 high quality", "VC-2 low delay", "Dirac core"...
    pub profile: String,
    pub interlaced: bool,
    /// Frames per second as a ratio.
    pub frame_rate: (u32, u32),
}

const PREFIX: &[u8; 4] = b"BBCD";

/// The parse codes (Table 9.1 and SMPTE ST 2042-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unit {
    Sequence,
    End,
    LowDelay,
    HighQuality,
    Core,
    Other,
}

fn unit(code: u8) -> Unit {
    match code {
        0x00 => Unit::Sequence,
        0x10 => Unit::End,
        _ if code & 0x08 == 0 => Unit::Other,
        _ if code & 0xF8 == 0xC8 => Unit::LowDelay,
        _ if code & 0xF8 == 0xE8 => Unit::HighQuality,
        _ if code & 0x88 == 0x08 => Unit::Core,
        _ => Unit::Other,
    }
}

/// The data units of a buffer: (parse code, body after the parse info header). Each runs to
/// the next parse info header (by its offset, else by the next prefix found), or to the end.
fn units(d: &[u8]) -> Vec<(u8, &[u8])> {
    let mut v = Vec::new();
    let mut at = match find(d, 0) {
        Some(a) => a,
        None => return v,
    };
    while at + 13 <= d.len() {
        let code = d[at + 4];
        let next = u32::from_be_bytes([d[at + 5], d[at + 6], d[at + 7], d[at + 8]]) as usize;
        let end = if next >= 13
            && at + next <= d.len()
            && (at + next == d.len() || d[at + next..].starts_with(PREFIX))
        {
            at + next
        } else {
            find(d, at + 13).unwrap_or(d.len())
        };
        v.push((code, &d[at + 13..end]));
        if end >= d.len() {
            break;
        }
        at = end;
    }
    v
}

fn find(d: &[u8], from: usize) -> Option<usize> {
    d.get(from..)?
        .windows(4)
        .position(|w| w == PREFIX)
        .map(|p| p + from)
}

const CORE: &str = "Dirac core syntax (arithmetic-coded or motion-compensated pictures)";

/// What the first sequence header in `stream` says; the profile is named after the first
/// picture's syntax when one follows.
pub fn probe(stream: &[u8]) -> Option<Info> {
    let us = units(stream);
    let seq = us
        .iter()
        .find(|(c, _)| *c == 0)
        .and_then(|(_, b)| Sequence::parse(b).ok())?;
    let syntax = us
        .iter()
        .map(|(c, _)| unit(*c))
        .find(|u| matches!(u, Unit::LowDelay | Unit::HighQuality | Unit::Core));
    let profile = match (syntax, seq.profile) {
        (Some(Unit::LowDelay), _) | (None, 0) => "VC-2 low delay",
        (Some(Unit::HighQuality), _) | (None, 3) => "VC-2 high quality",
        (None, 1) => "Dirac simple",
        (None, 2) => "Dirac main (intra)",
        _ => "Dirac core",
    };
    Some(Info {
        width: seq.width,
        height: seq.height,
        chroma: seq.chroma(),
        bit_depth: seq.luma_depth() as u8,
        profile: profile.into(),
        interlaced: seq.interlaced,
        frame_rate: seq.frame_rate,
    })
}

/// Why `stream` can't be decoded, in words; `None` when nothing in it is refused.
pub fn refusal(stream: &[u8]) -> Option<String> {
    let mut seq = None;
    for (code, body) in units(stream) {
        match unit(code) {
            Unit::Sequence => match Sequence::parse(body) {
                Ok(s) => seq = Some(s),
                Err(Error::Unsupported(m)) => return Some(m),
                Err(_) => {}
            },
            Unit::Core => return Some(CORE.into()),
            u @ (Unit::LowDelay | Unit::HighQuality) => {
                if let Some(s) = &seq
                    && let Err(Error::Unsupported(m)) =
                        picture::header(body, s, u == Unit::HighQuality)
                {
                    return Some(m);
                }
            }
            _ => {}
        }
    }
    None
}

/// A field waiting for its partner.
struct Field {
    number: u32,
    planes: [(Vec<i32>, usize); 3],
}

/// A stream's decoder: the sequence header in force and, for field coding, the first field.
#[derive(Default)]
pub struct Decoder {
    seq: Option<Sequence>,
    field: Option<Field>,
}

impl Decoder {
    pub fn new() -> Decoder {
        Decoder::default()
    }

    /// Forgets the held field (the sequence header stays: containers give it once).
    pub fn reset(&mut self) {
        self.field = None;
    }

    /// Decodes the data units of one container sample or Ogg packet; the last frame it
    /// completes, if any.
    pub fn decode(&mut self, data: &[u8]) -> Result<Option<Picture>> {
        let mut out = None;
        for (code, body) in units(data) {
            match unit(code) {
                Unit::Sequence => {
                    let s = Sequence::parse(body)?;
                    if self.seq.as_ref() != Some(&s) {
                        self.field = None;
                    }
                    self.seq = Some(s);
                }
                Unit::End => self.field = None,
                Unit::Core => return Err(Error::Unsupported(CORE.into())),
                u @ (Unit::LowDelay | Unit::HighQuality) => {
                    let seq = self
                        .seq
                        .clone()
                        .ok_or_else(|| bad("a picture before any sequence header"))?;
                    let h = picture::header(body, &seq, u == Unit::HighQuality)?;
                    let planes = picture::decode(body, &h, &seq)?;
                    if !seq.fields {
                        out = Some(frame(&seq, h.number, &planes, None));
                    } else if h.number % 2 == 0 {
                        self.field = Some(Field {
                            number: h.number,
                            planes,
                        });
                    } else if let Some(f) = self.field.take()
                        && f.number.wrapping_add(1) == h.number
                    {
                        out = Some(frame(&seq, f.number, &f.planes, Some(&planes)));
                    }
                }
                Unit::Other => {}
            }
        }
        Ok(out)
    }
}

/// Clips, offsets and crops the decoded components into a frame; with a second field, the two
/// are woven (the first, even-numbered field on the top line when the format says top field
/// first).
fn frame(
    seq: &Sequence,
    number: u32,
    first: &[(Vec<i32>, usize); 3],
    second: Option<&[(Vec<i32>, usize); 3]>,
) -> Picture {
    let (fw, fh) = (seq.width as usize, seq.height as usize);
    let chroma = seq.chroma();
    let (cw, ch) = match chroma {
        Chroma::C444 => (fw, fh),
        Chroma::C422 => (fw.div_ceil(2), fh),
        Chroma::C420 => (fw.div_ceil(2), fh.div_ceil(2)),
    };
    let (lw, lh) = seq.luma_size();
    let (pcw, pch) = seq.chroma_size();
    let planes = std::array::from_fn(|c| {
        let depth = if c == 0 {
            seq.luma_depth()
        } else {
            seq.chroma_depth()
        };
        let (ow, oh, iw, ih) = if c == 0 {
            (fw, fh, lw, lh)
        } else {
            (cw, ch, pcw, pch)
        };
        let half = 1i32 << (depth - 1);
        let put = |v: i32| (v.clamp(-half, half - 1) + half) as u16;
        let mut out = vec![0u16; ow * oh];
        if iw == 0 || ih == 0 {
            return out;
        }
        for y in 0..oh {
            // The coded row this output row comes from.
            let (src, row) = match second {
                None => (&first[c], y.min(ih - 1)),
                Some(s) => {
                    // Field rows: the output frame's lines alternate between the fields.
                    let parity = y % 2;
                    let from_first = (parity == 0) == seq.top_field_first;
                    (
                        if from_first { &first[c] } else { &s[c] },
                        (y / 2).min(ih - 1),
                    )
                }
            };
            let (data, stride) = (&src.0, src.1);
            let line = &data[row * stride..];
            let o = &mut out[y * ow..][..ow];
            for (x, v) in o.iter_mut().enumerate() {
                *v = put(line[x.min(iw - 1)]);
            }
        }
        out
    });
    Picture {
        width: seq.width,
        height: seq.height,
        chroma,
        bit_depth: seq.luma_depth() as u8,
        planes,
        signal: seq.signal(),
        number,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_codes() {
        assert_eq!(unit(0x00), Unit::Sequence);
        assert_eq!(unit(0x10), Unit::End);
        assert_eq!(unit(0x20), Unit::Other);
        assert_eq!(unit(0x30), Unit::Other);
        assert_eq!(unit(0xC8), Unit::LowDelay);
        assert_eq!(unit(0xCC), Unit::LowDelay);
        assert_eq!(unit(0xE8), Unit::HighQuality);
        assert_eq!(unit(0xEC), Unit::HighQuality);
        for c in [0x08, 0x0C, 0x48, 0x4C, 0x09, 0x0A, 0x0D, 0x0E] {
            assert_eq!(unit(c), Unit::Core, "{c:#x}");
        }
    }

    #[test]
    fn hostile_input_does_not_panic() {
        let mut d = Decoder::new();
        for junk in [
            &b""[..],
            b"BBCD",
            b"BBCD\x00\x00\x00\x00\x00\x00\x00\x00\x00",
            b"BBCD\xe8\x00\x00\x00\x20\x00\x00\x00\x00\x00\x00\x00\x00",
            b"BBCD\x00\xff\xff\xff\xff\x00\x00\x00\x00\xff\xff\xff\xff\xff",
        ] {
            let _ = d.decode(junk);
            let _ = probe(junk);
            let _ = refusal(junk);
        }
    }
}
