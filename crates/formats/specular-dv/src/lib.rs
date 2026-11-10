//! Specular's own DV video decoder: IEC 61834 consumer DV and SMPTE 314M DV-based video
//! (DVCPRO, DVCPRO50), written from the free SMPTE ST 314:2005 text.
//!
//! - DV25 525/60 4:1:1 (NTSC DV and DVCPRO), 625/50 4:2:0 (PAL consumer DV), 625/50 4:1:1
//!   (DVCPRO), and DV50 4:2:2 in both systems (DVCPRO50, two DIF channels).
//! - Every frame is intra coded: 8-8 and 2-4-8 (field) DCT, four classes, the area/QNO
//!   quantisation of Table 23, weighting, and the three-pass distribution of the code words
//!   over a video segment. DIF sequences decode in parallel.
//! - Header, VAUX and subcode packs give the system, the 16:9 flag, the field order and the
//!   recording date and time; the audio section gives 16-bit PCM (48, 44.1 or 32 kHz) and
//!   12-bit nonlinear sound (32 kHz; the first channel pair), expanded to 16 bits.
//!
//! Refused: DVCPRO HD (SMPTE 370M: its quantiser, weighting and macro block order tables are
//! not in the free SMPTE 314 text this decoder follows).
//!
//! Damaged macro blocks (error status, video error code, invalid code words) decode as far as
//! their data goes, missing ones are mid-grey; `Picture::damaged_blocks` counts them.
//!
//! The inverse DCT follows the floating-point definition of 5.2.1, so samples may differ from
//! other decoders' integer transforms by a step (the standard leaves the arithmetic open
//! within the tolerances of 5.2.4).
#![deny(unsafe_code)]
#![allow(clippy::needless_range_loop)]

mod audio;
mod bits;
mod packs;
#[cfg(test)]
mod synth_tests;
mod tables;
mod video;
mod vlc;

use rayon::prelude::*;

pub use audio::{Audio, audio};

/// Why a frame can't be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A valid frame using something this decoder doesn't do.
    Unsupported(String),
    /// Not DV, or damaged beyond use.
    Malformed(String),
    /// The memory budget cannot hold the frame's planes now.
    NeedsMemory(specular_core::NeedsMemory),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Malformed(s) => write!(f, "damaged DV video: {s}"),
            Error::NeedsMemory(n) => write!(f, "this video {n}"),
        }
    }
}

impl std::error::Error for Error {}

fn malformed(s: &str) -> Error {
    Error::Malformed(s.into())
}

/// Chroma sampling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chroma {
    /// Chroma planes a quarter of the width, full height (NTSC DV, DVCPRO).
    C411,
    /// Half width, half height (PAL consumer DV).
    C420,
    /// Half width, full height (DVCPRO50).
    C422,
}

/// What a frame's headers say.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    pub chroma: Chroma,
    /// Bytes per frame (120,000 or 144,000; twice that for DVCPRO50).
    pub frame_bytes: usize,
    /// Frames per second as a fraction (30000/1001 or 25/1).
    pub frame_rate: (u32, u32),
    /// 16:9 (anamorphic) pictures.
    pub wide: bool,
    /// "DV", "DVCPRO" or "DVCPRO50".
    pub profile: &'static str,
    /// Interlaced with the bottom field (field 1, the odd lines) first; DV's usual order.
    pub bottom_field_first: bool,
    /// The camera's recording date and time, "YYYY-MM-DD HH:MM:SS", when the frame has it.
    pub recorded: Option<String>,
}

/// A decoded frame: planes Y, Cb, Cr, tightly packed (chroma `width / 4` wide for 4:1:1,
/// `width / 2` for 4:2:0 and 4:2:2; half height for 4:2:0). BT.601, studio range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub chroma: Chroma,
    pub planes: [Vec<u8>; 3],
    /// Macro blocks with an error status and blocks whose data was damaged or missing.
    pub damaged_blocks: u32,
}

/// Bytes of one DIF sequence.
pub(crate) const SEQ: usize = 150 * 80;

/// The frame structure the decoder walks.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Layout {
    pub chroma: Chroma,
    /// DIF sequences per channel (10 or 12).
    pub seqs: usize,
    /// DIF channels (2 for 50 Mb/s).
    pub channels: usize,
    pub width: u32,
    pub height: u32,
}

/// The first DIF sequence whose header block looks intact (some captures zero the ID bytes,
/// and a tape dropout can hit the first header), so its payload says the system.
pub(crate) fn header(frame: &[u8]) -> Option<&[u8]> {
    (0..12)
        .map(|s| s * SEQ)
        .take_while(|&at| at + SEQ <= frame.len())
        .map(|at| &frame[at..at + SEQ])
        .find(|seq| seq[0] >> 5 == 0 && seq[2] == 0 && seq[80] >> 5 == 1 && seq[240] >> 5 == 2)
}

fn layout(frame: &[u8]) -> Result<(Layout, Info), Error> {
    if frame.len() < SEQ {
        return Err(malformed("shorter than one DIF sequence"));
    }
    let h = header(frame).ok_or_else(|| malformed("not a DV frame"))?;
    let pal = h[3] & 0x80 != 0;
    let apt = h[4] & 7;
    let seqs = if pal { 12 } else { 10 };
    let p = packs::scan(frame, seqs);
    let stype = p.vs.map(|v| v[3] & 0x1F);
    let (chroma, channels, profile) = match stype {
        Some(0x14 | 0x15 | 0x18 | 0x19) => {
            return Err(Error::Unsupported("DVCPRO HD video".into()));
        }
        Some(0x04) => (Chroma::C422, 2, "DVCPRO50"),
        Some(0) | None => match (pal, apt) {
            (true, 0) => (Chroma::C420, 1, "DV"),
            (_, 0) => (Chroma::C411, 1, "DV"),
            _ => (Chroma::C411, 1, "DVCPRO"),
        },
        Some(s) => {
            return Err(Error::Unsupported(format!("DV video of signal type {s}")));
        }
    };
    let (width, height) = (720, if pal { 576 } else { 480 });
    let l = Layout {
        chroma,
        seqs,
        channels,
        width,
        height,
    };
    let info = Info {
        width,
        height,
        chroma,
        frame_bytes: SEQ * seqs * channels,
        frame_rate: if pal { (25, 1) } else { (30000, 1001) },
        wide: p.wide,
        profile,
        bottom_field_first: p.bottom_field_first,
        recorded: p.recorded,
    };
    Ok((l, info))
}

/// The frame's facts from its first DIF sequence (header and VAUX packs).
pub fn probe(frame: &[u8]) -> Result<Info, Error> {
    layout(frame).map(|(_, i)| i)
}

/// Super block positions of the five macro blocks of segment `k` of DIF sequence `dseq` in
/// channel `fsc` (4.7.2.1 and 5.1.6).
pub(crate) fn segment_places(l: &Layout, dseq: usize, fsc: usize, k: u32) -> [video::MbPos; 5] {
    const J: [u32; 5] = [2, 1, 3, 0, 4];
    const OFF: [usize; 5] = [2, 6, 8, 0, 4];
    std::array::from_fn(|q| {
        let i = (dseq + OFF[q]) % l.seqs;
        let row = i * l.channels + fsc;
        video::mb_pos(l.chroma, row as u32, J[q], k)
    })
}

/// Decodes one whole frame (every DIF sequence).
pub fn decode(frame: &[u8]) -> Result<Picture, Error> {
    let (l, info) = layout(frame)?;
    let nseq = l.seqs * l.channels;
    if frame.len() < info.frame_bytes {
        return Err(malformed("the frame is cut short"));
    }
    let (cw, ch) = match l.chroma {
        Chroma::C411 => (l.width / 4, l.height),
        Chroma::C420 => (l.width / 2, l.height / 2),
        Chroma::C422 => (l.width / 2, l.height),
    };
    let luma = (l.width * l.height) as usize;
    let chroma = (cw * ch) as usize;
    // The planes, and per sequence the decoded blocks before they are placed.
    let blocks_per_seq = 135 * 6 * std::mem::size_of::<video::Placed>();
    let _res = specular_core::budget::reserve((luma + 2 * chroma + nseq * blocks_per_seq) as u64)
        .map_err(Error::NeedsMemory)?;
    let results: Vec<(Vec<video::Placed>, u32)> = (0..nseq)
        .into_par_iter()
        .map(|t| decode_sequence(&l, t % l.seqs, t / l.seqs, &frame[t * SEQ..(t + 1) * SEQ]))
        .collect();
    let mut planes = [vec![128u8; luma], vec![128u8; chroma], vec![128u8; chroma]];
    let mut damaged = 0;
    let widths = [l.width as usize, cw as usize, cw as usize];
    for (placed, d) in results {
        damaged += d;
        for b in &placed {
            let p = usize::from(b.plane);
            let w = widths[p];
            let plane = &mut planes[p];
            let (x0, y0) = (usize::from(b.x), usize::from(b.y));
            if b.folded {
                for y in 0..8 {
                    for x in 0..8 {
                        let (xx, yy) = if x < 4 { (x, y) } else { (x - 4, y + 8) };
                        if let Some(s) = plane.get_mut((y0 + yy) * w + x0 + xx) {
                            *s = b.px[y * 8 + x];
                        }
                    }
                }
            } else {
                for y in 0..8 {
                    let at = (y0 + y) * w + x0;
                    if let Some(row) = plane.get_mut(at..at + 8) {
                        row.copy_from_slice(&b.px[y * 8..y * 8 + 8]);
                    }
                }
            }
        }
    }
    Ok(Picture {
        width: l.width,
        height: l.height,
        chroma: l.chroma,
        planes,
        damaged_blocks: damaged,
    })
}

/// The 135 video DIF blocks of sequence `dseq` of channel `fsc`, decoded segment by segment.
/// Blocks are found by position: the ID bytes are not always kept by capture tools.
fn decode_sequence(l: &Layout, dseq: usize, fsc: usize, seq: &[u8]) -> (Vec<video::Placed>, u32) {
    let mut out = Vec::with_capacity(135 * 6);
    let mut scratch = video::Scratch::default();
    let mut damaged = 0;
    for k in 0..27u32 {
        let cms: [Option<&[u8]>; 5] = std::array::from_fn(|q| {
            let b = k as usize * 5 + q;
            let at = (7 + (b / 15) * 16 + b % 15) * 80;
            Some(&seq[at..at + 80])
        });
        let places = segment_places(l, dseq, fsc, k);
        damaged += video::segment(l, cms, places, &mut scratch, &mut out);
    }
    (out, damaged)
}
