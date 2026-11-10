//! Specular's own software H.264 / AVC decoder (ITU-T H.264), the fallback for motion photos
//! and Live Photos when there is no hardware decoder or it refuses the stream.
//!
//! Decodes streams of the Constrained Baseline, Baseline (with FMO, ASO and redundant
//! slices, the redundant ones dropped), Main, Extended (with data partitioning), High, High 10, High 4:2:2, High 4:4:4 Predictive and the matching intra
//! profiles: 4:0:0, 4:2:0, 4:2:2 and 4:4:4 chroma at 8 to 14 bits (4:4:4 with separate
//! colour planes too: three monochrome decoders, one per colour_plane_id, joined), lossless
//! coding (transform bypass), CAVLC and CABAC, I/P/B slices, several slices per picture,
//! interlaced coding (field pictures and MBAFF frames), weighted prediction, spatial and
//! temporal direct, the 8x8 transform and scaling matrices, all three picture order count
//! types, long-term references and memory management operations, and output in display
//! (bumping) or decoding order. [`random_access`] tells which samples decoding can start at
//! (IDR pictures and open-GOP recovery points).
//!
//! Not supported: SP/SI slices (refused with [`Error::Unsupported`]) and the SVC and MVC
//! extensions (the base layer or view decodes on its own).
//!
//! Field pairs are output as interleaved frames; a field without its opposite field is output
//! with each of its lines repeated. [`FieldScan`] tells from the slice headers alone which MP4
//! samples carry a second field (one field per sample: two composition times per frame).
//!
//! Pictures decode on a pool of worker threads (frame threading): the main thread parses
//! headers, keeps the decoded picture buffer and builds the reference lists, and each worker
//! decodes one picture, publishing it one deblocked macroblock row at a time so a later
//! picture can motion-compensate from the finished top of its references.
//!
//! Output is bit-exact with the reference decoder for conforming streams; damaged slices are
//! concealed (copied from a reference, or grey) rather than stopping the stream.
#![deny(unsafe_code)]
// Cache hints for the reference samples of motion compensation (`picture::View::prefetch`).
#![feature(hint_prefetch)]
#![feature(portable_simd)]
// Block loops index several per-block tables at once, in the standard's terms.
#![allow(clippy::needless_range_loop)]

mod bits;
mod cabac;
mod cavlc;
mod deblock;
mod decoder;
mod dpb;
mod fields;
#[cfg(test)]
mod hostile;
mod inter;
mod intra;
mod mb;
mod params;
mod picture;
mod pixel;
mod sei;
mod slice;
mod slicedec;
mod spec_tables;
mod spec_tables_444;
#[cfg(test)]
mod split_tests;
mod tables;
mod transform;

pub use decoder::{
    Chroma, Decoder, Options, OutputOrder, Picture, Signal, Snapshot, parse_avcc, split_annexb,
};
pub use fields::{FieldScan, SampleFields, VuiSignal};
pub use sei::{random_access, sei_recovery_frame_cnt};

/// Why a stream can't be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A valid stream using something this decoder doesn't implement (the message names it,
    /// e.g. "H.264 4:4:4 chroma").
    Unsupported(String),
    /// The bitstream breaks the syntax or its limits.
    Malformed(String),
    /// The picture buffers fit the limits but not the memory budget's free memory now.
    NeedsMemory(specular_core::NeedsMemory),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Malformed(s) => write!(f, "malformed H.264: {s}"),
            Error::NeedsMemory(n) => write!(f, "{n}"),
        }
    }
}

impl From<specular_core::NeedsMemory> for Error {
    fn from(n: specular_core::NeedsMemory) -> Self {
        Error::NeedsMemory(n)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn malformed(s: &str) -> Error {
    Error::Malformed(s.into())
}
