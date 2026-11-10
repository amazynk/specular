//! Specular's own AMR and AMR-WB speech decoders.
//!
//! - **AMR** (AMR-NB, 3GPP TS 26.071/26.090/26.073): 8 kHz, the eight modes 4.75 to
//!   12.2 kbit/s, SID frames and comfort noise, DTX, and concealment of lost and damaged
//!   frames.
//! - **AMR-WB** (3GPP TS 26.171/26.190/26.173, ITU-T G.722.2): 16 kHz, the nine modes
//!   6.60 to 23.85 kbit/s, SID frames and comfort noise, DTX and lost-frame concealment.
//!
//! Both follow the bit-exact fixed-point descriptions of the specifications (checked
//! against the 3GPP test sequences TS 26.074 and TS 26.174). The decoding logic is our
//! own; the tables are data transcribed from the specifications (see `nb/tables.rs`,
//! `wb/tables.rs`).
//!
//! Containers handled here: the AMR storage format of RFC 4867 §5 (`#!AMR\n`,
//! `#!AMR-WB\n` and their multichannel forms) and the same octet-aligned frames as stored
//! in 3GP/MP4 (`samr`/`sawb`) and Matroska (`A_AMR/NB`, `A_AMR/WB`) samples.
//!
//! Patents: AMR-NB's essential patents date from 1997–2000 and AMR-WB's from 1998–2001;
//! the last US one (US 7,280,959, AMR-WB) expired in May 2024.

#![deny(unsafe_code)]
#![allow(clippy::needless_range_loop)]

mod basic;
pub mod nb;
pub mod storage;
pub mod wb;

pub use storage::{Codec, FileInfo, Stream};

/// How a frame arrived (TS 26.093 / 26.193 receive frame types).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RxType {
    SpeechGood,
    SpeechDegraded,
    Onset,
    SpeechBad,
    SidFirst,
    SidUpdate,
    SidBad,
    NoData,
    /// AMR-WB only: speech whose bits were lost.
    SpeechLost,
}

/// Most bits in any frame (AMR-WB 23.85 kbit/s).
pub const MAX_BITS: usize = 477;

/// One received frame: its type, codec mode (for SID frames the mode indication) and its
/// bits in parameter order, one bit per byte.
#[derive(Clone, Debug)]
pub struct Frame {
    pub rx: RxType,
    pub mode: u8,
    pub bits: [u8; MAX_BITS],
}

impl Frame {
    pub fn empty(rx: RxType, mode: u8) -> Self {
        Frame {
            rx,
            mode,
            bits: [0; MAX_BITS],
        }
    }
}

/// Why something can't be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Unsupported(String),
    Malformed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Malformed(s) => write!(f, "damaged AMR: {s}"),
        }
    }
}

impl std::error::Error for Error {}

/// A decoder for either codec, frame by frame.
#[derive(Clone)]
pub enum Decoder {
    Nb(Box<nb::Decoder>),
    Wb(Box<wb::Decoder>),
}

impl Decoder {
    pub fn new(codec: Codec) -> Self {
        match codec {
            Codec::Nb => Decoder::Nb(Box::default()),
            Codec::Wb => Decoder::Wb(Box::default()),
        }
    }

    pub fn codec(&self) -> Codec {
        match self {
            Decoder::Nb(_) => Codec::Nb,
            Decoder::Wb(_) => Codec::Wb,
        }
    }

    pub fn reset(&mut self) {
        *self = Decoder::new(self.codec());
    }

    /// Samples per frame: 160 (AMR) or 320 (AMR-WB).
    pub fn frame_len(&self) -> usize {
        self.codec().frame_len()
    }

    /// Decodes one frame, appending its samples.
    pub fn decode_frame(&mut self, frame: &Frame, out: &mut Vec<i16>) {
        match self {
            Decoder::Nb(d) => {
                let mut s = [0i16; nb::L_FRAME];
                d.decode(frame, &mut s);
                out.extend_from_slice(&s);
            }
            Decoder::Wb(d) => {
                let mut s = [0i16; wb::L_FRAME16K];
                d.decode(frame, &mut s);
                out.extend_from_slice(&s);
            }
        }
    }

    /// Decodes the frames of one container sample (3GP/MP4 or Matroska storage-format
    /// frames, or an RTP octet-aligned payload); returns the number of frames.
    pub fn decode_packet(&mut self, data: &[u8], out: &mut Vec<i16>) -> usize {
        let frames = storage::packet_frames(self.codec(), data);
        for f in &frames {
            self.decode_frame(f, out);
        }
        frames.len()
    }
}
