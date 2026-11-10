//! Specular's own VP8 video decoder (RFC 6386), for WebM playback.
//!
//! Decodes key frames and interframes of bitstream versions 0 to 3: golden and altref
//! references with sign bias and copy/refresh flags, segmentation, the normal and simple
//! loop filters with mode and reference deltas, probability updates (kept or discarded per
//! frame), all intra modes, split motion vectors, six-tap and bilinear (and full-pixel
//! chroma) motion compensation, and up to eight token partitions. Hidden frames
//! (`show_frame` 0) update the references without returning a picture. The scale bits are
//! reported by [`probe`]; pictures are returned at the coded size.
//!
//! Macroblock rows decode on worker threads in a wavefront: each row reads its token
//! partition, reconstructs and loop-filters a few macroblocks behind the row above.
//!
//! Output is bit-exact with libvpx. Damaged frames return an error, after which the
//! decoder waits for the next key frame.
#![deny(unsafe_code)]
#![feature(portable_simd)]
// Block loops index several per-block tables at once, in the specification's terms.
#![allow(clippy::needless_range_loop)]

mod boolcoder;
mod decoder;
mod frame;
mod header;
mod idct;
mod inter;
mod intra;
mod loopfilter;
mod modes;
mod tables;
mod tokens;

/// Why a frame can't be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A valid stream using something this decoder doesn't do.
    Unsupported(String),
    /// The bitstream is damaged or breaks the format.
    Malformed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Malformed(s) => write!(f, "damaged VP8 video: {s}"),
        }
    }
}

impl std::error::Error for Error {}

pub(crate) fn malformed(s: &str) -> Error {
    Error::Malformed(s.into())
}

/// A decoded frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    /// Y, U, V, rows tightly packed (Y: width × height; U, V: (width+1)/2 × (height+1)/2), 8-bit.
    pub planes: [Vec<u8>; 3],
    /// The key frame's color_space bit is 1 (reserved) rather than 0 (BT.601).
    pub reserved_color_space: bool,
}

/// A VP8 decoder for one stream.
pub struct Decoder {
    state: decoder::State,
}

impl Decoder {
    /// `threads`: worker threads (0 = available parallelism, capped at 8; 1 = none).
    pub fn new(threads: usize) -> Decoder {
        Decoder {
            state: decoder::State::new(threads),
        }
    }

    /// Decodes one frame; returns the picture when it is shown (show_frame), None for hidden ones.
    /// After an error the decoder needs a keyframe.
    pub fn decode(&mut self, data: &[u8]) -> Result<Option<Picture>, Error> {
        self.state.decode(data)
    }

    /// Forgets all references; the next frame must be a key frame.
    pub fn reset(&mut self) {
        self.state.reset();
    }
}

/// A key frame's size and scaling bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameInfo {
    pub width: u32,
    pub height: u32,
    pub h_scale: u8,
    pub v_scale: u8,
}

/// Reads a key frame's header; None for interframes and damaged data.
pub fn probe(data: &[u8]) -> Option<FrameInfo> {
    let t = header::parse_tag(data).ok()?;
    t.key.then_some(FrameInfo {
        width: t.width,
        height: t.height,
        h_scale: t.h_scale,
        v_scale: t.v_scale,
    })
}

/// The frame is a key frame (frame type bit and start code).
pub fn is_keyframe(data: &[u8]) -> bool {
    probe(data).is_some()
}
