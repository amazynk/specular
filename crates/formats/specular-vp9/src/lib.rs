//! Specular's own VP9 decoder, written from the VP9 Bitstream & Decoding Process
//! Specification (v0.6/v0.7).
//!
//! Decodes profiles 0 to 3: 8, 10 and 12 bits, 4:2:0, 4:2:2, 4:4:0 and 4:4:4 chroma (and RGB,
//! [`ColorSpace::Srgb`], whose planes come out G, B, R as coded), with every
//! intra and inter mode, compound prediction, switchable interpolation filters, all transform
//! sizes and types (and lossless), segmentation, loop filter deltas, scaled references,
//! intra-only and hidden frames, show_existing_frame, superframes and backward probability
//! adaptation.
//!
//! Tile columns decode in parallel, then the loop filter runs as a wavefront of superblock
//! rows, on a pool of worker threads owned by the [`Decoder`]. Output is bit-exact with the
//! reference decoder for conforming streams.
#![deny(unsafe_code)]
#![feature(portable_simd)]
// Block loops index several per-block tables at once, in the specification's terms.
#![allow(clippy::needless_range_loop)]
// The probability tables keep the specification's dimensions.
#![allow(clippy::type_complexity)]

mod bits;
mod decoder;
mod frame;
mod header;
mod inter;
mod intra;
mod itx;
mod lf;
mod probs;
mod tables;
mod tile;
mod txfm_gen;

/// Why a stream can't be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A valid stream using something this decoder doesn't implement (the message says what).
    Unsupported(String),
    /// The bitstream breaks the syntax or its limits.
    Malformed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported(s) => write!(f, "{s}"),
            Error::Malformed(s) => write!(f, "damaged VP9 video: {s}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn malformed(s: &str) -> Error {
    Error::Malformed(s.into())
}

/// The colour space signalled in the stream (color_space).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSpace {
    Unknown,
    Bt601,
    Bt709,
    Smpte170,
    Smpte240,
    Bt2020,
    Reserved,
    Srgb,
}

impl ColorSpace {
    pub(crate) fn from_code(c: u32) -> ColorSpace {
        match c {
            0 => ColorSpace::Unknown,
            1 => ColorSpace::Bt601,
            2 => ColorSpace::Bt709,
            3 => ColorSpace::Smpte170,
            4 => ColorSpace::Smpte240,
            5 => ColorSpace::Bt2020,
            6 => ColorSpace::Reserved,
            _ => ColorSpace::Srgb,
        }
    }
}

/// A decoded frame.
#[derive(Debug, Clone)]
pub struct Picture {
    /// The decoded frame size.
    pub width: u32,
    pub height: u32,
    /// render_size() from the header.
    pub render_width: u32,
    pub render_height: u32,
    /// 8, 10 or 12.
    pub bit_depth: u8,
    /// Chroma subsampling.
    pub ss_x: bool,
    pub ss_y: bool,
    /// Y, U, V planes (G, B, R for [`ColorSpace::Srgb`]), rows tightly packed (plane width x
    /// plane height); samples are u8 for 8-bit, u16 little-endian above 8 bits. Chroma planes are (w+ss_x)>>ss_x x (h+ss_y)>>ss_y.
    pub planes: [Vec<u8>; 3],
    pub color_space: ColorSpace,
    /// color_range.
    pub full_range: bool,
}

/// Header facts of a keyframe or intra-only frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameInfo {
    pub profile: u8,
    pub width: u32,
    pub height: u32,
    pub render_width: u32,
    pub render_height: u32,
    pub bit_depth: u8,
    pub ss_x: bool,
    pub ss_y: bool,
    pub color_space: ColorSpace,
    pub full_range: bool,
}

/// Header facts of a keyframe (or intra-only frame) without decoding it; `None` for inter
/// frames and garbage. For a superframe, the first frame is probed.
pub fn probe(data: &[u8]) -> Option<FrameInfo> {
    let first = *header::split_superframe(data).first()?;
    let (profile, c, width, height, render_width, render_height) = header::probe_intra(first)?;
    Some(FrameInfo {
        profile,
        width,
        height,
        render_width,
        render_height,
        bit_depth: c.bit_depth,
        ss_x: c.ss_x,
        ss_y: c.ss_y,
        color_space: c.color_space,
        full_range: c.full_range,
    })
}

/// Whether the block (the first frame of a superframe) is a keyframe.
pub fn is_keyframe(data: &[u8]) -> bool {
    header::split_superframe(data)
        .first()
        .is_some_and(|f| header::frame_is_key(f))
}

/// A VP9 decoder with its reference frames, probability contexts and worker threads.
pub struct Decoder {
    state: decoder::State,
}

impl Decoder {
    /// `threads`: worker threads for tiles and the loop filter (0 = available parallelism,
    /// capped at 8; 1 = decode on the calling thread).
    pub fn new(threads: usize) -> Decoder {
        Decoder {
            state: decoder::State::new(threads),
        }
    }

    /// Decodes one container block (a frame or a superframe); pushes every shown frame to
    /// `out` (0, 1 or more). After an error the decoder needs a keyframe (it returns errors
    /// until then).
    pub fn decode(&mut self, data: &[u8], out: &mut Vec<Picture>) -> Result<()> {
        self.state.decode(data, out)
    }

    /// Forgets all state (before decoding from a keyframe after a seek).
    pub fn reset(&mut self) {
        self.state.reset();
    }
}
