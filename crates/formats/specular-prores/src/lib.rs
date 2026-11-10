//! Apple ProRes (SMPTE RDD 36): every profile, 422 Proxy, LT, 422 and HQ at 10 bits, 4444
//! and 4444 XQ at 12 bits with an optional 8- or 16-bit alpha channel, progressive and
//! interlaced (two field pictures per frame).
//!
//! A frame is independent of every other. Its pictures are cut into slices of 1 to 8
//! macroblocks along a macroblock row, each with its own size in the picture's index, so
//! the macroblock rows decode in parallel on the rayon pool.
//!
//! The output is planar: 10-bit (422) or 12-bit (4444) samples as little-endian `u16`,
//! chroma half width for 4:2:2, alpha at the same depth as the colour.

#![feature(portable_simd)]

mod bits;
mod idct;
pub mod raw;
mod slice;

use std::fmt;

pub use slice::PROGRESSIVE_SCAN;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Not a ProRes frame, or one that contradicts itself.
    Malformed(String),
    /// A valid frame this decoder does not read.
    Unsupported(String),
    /// The memory budget cannot hold the frame's planes.
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
            Error::Malformed(s) => write!(f, "malformed ProRes frame: {s}"),
            Error::Unsupported(s) => write!(f, "unsupported ProRes frame: {s}"),
            Error::NeedsMemory(n) => write!(f, "ProRes frame: {n}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T, E = Error> = std::result::Result<T, E>;

fn bad(s: impl Into<String>) -> Error {
    Error::Malformed(s.into())
}

/// Chroma sampling of a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chroma {
    Yuv422,
    Yuv444,
}

/// How the frame is scanned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interlace {
    Progressive,
    /// Two field pictures, the top field (even lines) first.
    TopFirst,
    /// Two field pictures, the bottom field first.
    BottomFirst,
}

/// The alpha channel's coding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alpha {
    None,
    Bits8,
    Bits16,
}

/// The frame header (RDD 36 §5.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub version: u8,
    /// The encoder's four-character identifier (`apl0` for Apple's, `Lavc`...).
    pub encoder: [u8; 4],
    pub width: u32,
    pub height: u32,
    pub chroma: Chroma,
    pub interlace: Interlace,
    /// `aspect_ratio_information` (0 unknown, 1 square, 2 4:3, 3 16:9).
    pub aspect: u8,
    /// `frame_rate_code` (0 unknown).
    pub frame_rate: u8,
    /// H.273 code points (0 and 2 mean unspecified).
    pub primaries: u8,
    pub transfer: u8,
    pub matrix: u8,
    pub alpha: Alpha,
    luma_q: [u8; 64],
    chroma_q: [u8; 64],
    /// Offset of the first picture from the start of the frame.
    pictures_at: usize,
}

impl Header {
    /// Bits per output sample: 10 for 4:2:2, 12 for 4:4:4.
    pub fn bit_depth(&self) -> u8 {
        match self.chroma {
            Chroma::Yuv422 => 10,
            Chroma::Yuv444 => 12,
        }
    }
}

/// Reads the frame header of a coded frame (a whole movie sample: `frame_size`, `icpf`,
/// header, pictures).
/// Matroska stores frames without the first 8 bytes (`frame_size`, `icpf`): those start with
/// the frame header itself.
pub fn header(frame: &[u8]) -> Result<Header> {
    /// The largest frame side read.
    const MAX_SIDE: u32 = 8192;
    let base = if frame.get(4..8) == Some(b"icpf") {
        8
    } else if frame.len() >= 20 && frame[2] == 0 && frame[3] <= 1 && frame[12] >> 6 >= 2 {
        0
    } else {
        return Err(bad("no frame identifier"));
    };
    if frame.len() < base + 20 {
        return Err(bad("no frame header"));
    }
    let h = &frame[base..];
    let size = usize::from(u16::from_be_bytes([h[0], h[1]]));
    if size < 20 || size > h.len() {
        return Err(bad("frame header size"));
    }
    let version = h[3];
    if h[2] != 0 || version > 1 {
        return Err(Error::Unsupported(format!("bitstream version {version}")));
    }
    let width = u32::from(u16::from_be_bytes([h[8], h[9]]));
    let height = u32::from(u16::from_be_bytes([h[10], h[11]]));
    if width == 0 || height == 0 {
        return Err(bad("empty frame"));
    }
    // ProRes goes to 8K; larger declared sizes cost gigabytes from a frame of empty slices.
    if width > MAX_SIDE || height > MAX_SIDE {
        return Err(Error::Unsupported(format!("{width}×{height} frame")));
    }
    let chroma = match h[12] >> 6 {
        2 => Chroma::Yuv422,
        3 => Chroma::Yuv444,
        c => return Err(bad(format!("chroma format {c}"))),
    };
    let interlace = match (h[12] >> 2) & 3 {
        0 => Interlace::Progressive,
        1 => Interlace::TopFirst,
        2 => Interlace::BottomFirst,
        _ => return Err(bad("interlace mode 3")),
    };
    let alpha = match h[17] & 15 {
        0 => Alpha::None,
        1 => Alpha::Bits8,
        2 => Alpha::Bits16,
        a => return Err(Error::Unsupported(format!("alpha channel type {a}"))),
    };
    let flags = h[19];
    let mut at = 20;
    let mut matrix = |load: bool| -> Result<[u8; 64]> {
        if !load {
            return Ok([4; 64]);
        }
        let m: [u8; 64] = h
            .get(at..at + 64)
            .filter(|_| at + 64 <= size)
            .and_then(|m| m.try_into().ok())
            .ok_or_else(|| bad("quantisation matrix"))?;
        at += 64;
        if m.contains(&0) {
            return Err(bad("zero weight"));
        }
        Ok(m)
    };
    let luma_q = matrix(flags & 2 != 0)?;
    let chroma_q = if flags & 1 != 0 {
        matrix(true)?
    } else {
        luma_q
    };
    Ok(Header {
        version,
        encoder: [h[4], h[5], h[6], h[7]],
        width,
        height,
        chroma,
        interlace,
        aspect: h[13] >> 4,
        frame_rate: h[13] & 15,
        primaries: h[14],
        transfer: h[15],
        matrix: h[16],
        alpha,
        luma_q,
        chroma_q,
        pictures_at: base + size,
    })
}

/// A decoded frame.
#[derive(Clone, Debug)]
pub struct Frame {
    pub header: Header,
    pub width: u32,
    pub height: u32,
    /// 10 or 12.
    pub bit_depth: u8,
    pub chroma: Chroma,
    /// Y, Cb, Cr: tightly packed little-endian `u16` rows (chroma `ceil(width/2)` wide for
    /// 4:2:2).
    pub planes: [Vec<u8>; 3],
    /// The alpha plane (luma size and depth), when the frame has one.
    pub alpha: Option<Vec<u8>>,
    /// Slices that failed to decode (their area is left mid-grey).
    pub damaged_slices: usize,
}

/// Decoding choices.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Macroblock rows in parallel on the current rayon pool.
    pub parallel: bool,
    /// Decode the alpha channel (when the frame has one).
    pub alpha: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            parallel: true,
            alpha: true,
        }
    }
}

/// Decodes one frame.
pub fn decode(frame: &[u8], o: &Options) -> Result<Frame> {
    let header = header(frame)?;
    slice::decode_frame(frame, header, o)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_other_data() {
        assert!(matches!(
            header(b"not prores at all, really no"),
            Err(Error::Malformed(_))
        ));
        let mut f = vec![0u8; 8 + 20];
        f[4..8].copy_from_slice(b"icpf");
        f[9] = 20;
        f[8 + 9] = 16;
        f[8 + 11] = 16;
        f[8 + 12] = 0x80;
        let h = header(&f).unwrap();
        assert_eq!((h.width, h.height, h.chroma), (16, 16, Chroma::Yuv422));
        assert_eq!(h.bit_depth(), 10);
        f[8 + 3] = 2;
        assert!(matches!(header(&f), Err(Error::Unsupported(_))));
    }

    /// A 28-byte frame declaring 65 535 × 65 535 (gigabytes of planes): refused.
    #[test]
    fn huge_frame_refused() {
        let mut f = vec![0u8; 8 + 20];
        f[4..8].copy_from_slice(b"icpf");
        f[9] = 20;
        f[8 + 8..8 + 12].copy_from_slice(&[0xff; 4]);
        f[8 + 12] = 0x80;
        assert!(matches!(header(&f), Err(Error::Unsupported(_))));
        assert!(matches!(
            decode(&f, &Options::default()),
            Err(Error::Unsupported(_))
        ));
    }

    /// A `side`² 4:4:4 frame with 16-bit alpha whose slices (8 macroblocks each) are all
    /// empty: 8 bytes a slice header plus 2 a slice.
    fn empty_444(side: u16) -> Vec<u8> {
        let mut f = vec![0u8; 8 + 20];
        f[4..8].copy_from_slice(b"icpf");
        f[9] = 20;
        f[8 + 8..8 + 10].copy_from_slice(&side.to_be_bytes());
        f[8 + 10..8 + 12].copy_from_slice(&side.to_be_bytes());
        f[8 + 12] = 0xc0;
        f[8 + 17] = 2;
        let mb = u32::from(side).div_ceil(16);
        let count = mb * mb.div_ceil(8);
        let size = 8 + 2 * count;
        f.push(8 << 3);
        f.extend_from_slice(&size.to_be_bytes());
        f.extend_from_slice(&(count as u16).to_be_bytes());
        f.push(3 << 4);
        f.resize(f.len() + 2 * count as usize, 0);
        f
    }

    /// The planes are reserved from the budget, sized from the header, before they are
    /// allocated: 512 MiB from a 64 KiB frame is refused under 64 MiB.
    #[test]
    fn planes_reserved() {
        use specular_core::budget::with_limit;
        let o = Options {
            parallel: false,
            alpha: true,
        };
        let f = empty_444(8192);
        assert!(f.len() < 70 << 10);
        let r = with_limit(64 << 20, || decode(&f, &o));
        assert!(matches!(r, Err(Error::NeedsMemory(_))), "{r:?}");
        // 256²: four 16-bit planes of 256² are 512 KiB, and no crop.
        let f = empty_444(256);
        let r = with_limit((512 << 10) - 1, || decode(&f, &o));
        assert!(matches!(r, Err(Error::NeedsMemory(_))), "{r:?}");
        let r = with_limit(512 << 10, || decode(&f, &o)).unwrap();
        assert_eq!(r.planes[0].len(), 256 * 256 * 2);
        assert!(r.alpha.is_some());
    }
}
