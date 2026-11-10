//! AV1 in a video container (MP4 `av01` tracks): a frame-by-frame decoder over the same
//! backend as stills, giving each picture's planes as they were coded (no RGB conversion,
//! which the GPU does at display time).

use crate::Error;
use crate::backend::{Av1Decoder, Input, new_decoder};

/// One decoded picture, planes copied out tightly (Y, then Cb and Cr; above 8 bits each
/// sample is a native-endian `u16`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoPicture {
    pub width: u32,
    pub height: u32,
    /// Significant bits per sample (8, 10 or 12).
    pub bits: u8,
    /// Y, Cb, Cr; a monochrome picture gets neutral chroma planes.
    pub planes: [Vec<u8>; 3],
    /// Width and height of each plane in samples.
    pub sizes: [(u32, u32); 3],
    /// Chroma halved horizontally / vertically.
    pub ssx: bool,
    pub ssy: bool,
    /// Subsampled chroma is co-sited with the even luma columns.
    pub cosited_x: bool,
    /// ... and with the even luma rows (AV1 "colocated").
    pub cosited_y: bool,
    /// H.273 colour primaries, transfer characteristics and matrix coefficients.
    pub primaries: u16,
    pub transfer: u16,
    pub matrix: u16,
    pub full_range: bool,
}

/// An AV1 decoder for a video track's samples, fed in decode order.
pub struct Av1Video {
    dec: Box<dyn Av1Decoder>,
}

impl Av1Video {
    /// `threads`: 0 for automatic.
    pub fn new(threads: u32) -> Result<Av1Video, Error> {
        Ok(Av1Video {
            dec: new_decoder(threads)?,
        })
    }

    /// Decodes one sample (a temporal unit). `None` when it yields no picture to show (a
    /// frame that is not shown, or a stream that has not started).
    pub fn decode(&mut self, sample: &[u8]) -> Result<Option<VideoPicture>, Error> {
        let Some(pic) = self.dec.decode(Input::Owned(sample.to_vec()))? else {
            return Ok(None);
        };
        let p = pic.planes();
        let bps = if p.hbd { 2 } else { 1 };
        let cw = if p.ssx { p.width.div_ceil(2) } else { p.width };
        let ch = if p.ssy {
            p.height.div_ceil(2)
        } else {
            p.height
        };
        let sizes = [(p.width, p.height), (cw, ch), (cw, ch)];
        let copy = |i: usize| -> Vec<u8> {
            let (w, h) = sizes[i];
            let row = w as usize * bps;
            let mut out = Vec::with_capacity(row * h as usize);
            if p.mono && i > 0 {
                let mid = 1u16 << (p.bits.max(8) - 1);
                for _ in 0..w as usize * h as usize {
                    if p.hbd {
                        out.extend_from_slice(&mid.to_ne_bytes());
                    } else {
                        out.push(128);
                    }
                }
                return out;
            }
            for y in 0..h as usize {
                let start = y * p.strides[i];
                match p.planes[i].get(start..start + row) {
                    Some(s) => out.extend_from_slice(s),
                    None => out.resize(out.len() + row, 0),
                }
            }
            out
        };
        Ok(Some(VideoPicture {
            width: p.width,
            height: p.height,
            bits: p.bits,
            planes: [copy(0), copy(1), copy(2)],
            sizes,
            ssx: p.ssx,
            ssy: p.ssy,
            cosited_x: p.cosited_x,
            cosited_y: p.cosited_y,
            primaries: p.cicp.primaries,
            transfer: p.cicp.transfer,
            matrix: p.cicp.matrix,
            full_range: p.cicp.full_range,
        }))
    }

    /// Forgets all state (before decoding from another keyframe).
    pub fn reset(&mut self) {
        self.dec.reset();
    }
}
