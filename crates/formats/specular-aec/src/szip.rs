//! The SZIP library's interface on top of AEC, as HDF5's SZIP filter (id 4) stores it.
//!
//! SZIP describes a stream by an options mask, bits per pixel, pixels per block and pixels
//! per scanline. Each scanline is one reference sample interval; a scanline that doesn't fill
//! its last block is padded with samples to whole blocks (the bits run on unpadded). 32 and
//! 64-bit pixels are coded as bytes, one byte plane after another.

use crate::{Error, MSB, PREPROCESS, Params, decode_bytes};

/// Larger codes allowed (always the case for libaec).
pub const ALLOW_K13: u32 = 1;
/// Entropy coding without the predictor.
pub const EC: u32 = 4;
/// Little-endian pixels.
pub const LSB: u32 = 8;
/// Big-endian pixels.
pub const MSB_OPTION: u32 = 16;
/// The nearest-neighbour predictor.
pub const NN: u32 = 32;
/// No SZIP header in the stream (HDF5 always sets it).
pub const RAW: u32 = 128;

/// SZIP's description of a stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SzParams {
    /// Options mask ([`NN`], [`MSB_OPTION`], ...).
    pub options: u32,
    /// Bits per pixel: 1 to 32, or 64.
    pub bits_per_pixel: u32,
    /// Pixels per block, even, up to 32.
    pub pixels_per_block: u32,
    /// Pixels per scanline.
    pub pixels_per_scanline: u32,
}

fn pixel_bytes(bits: u32) -> usize {
    match bits {
        0..=8 => 1,
        9..=16 => 2,
        17..=32 => 4,
        _ => 8,
    }
}

/// Decodes a raw SZIP stream into `out_len` bytes of pixels in the mask's byte order.
pub fn decode(input: &[u8], p: &SzParams, out_len: usize) -> Result<Vec<u8>, Error> {
    if p.pixels_per_block == 0 || p.pixels_per_scanline == 0 {
        return Err(Error::Params("szip block or scanline"));
    }
    if p.options & RAW == 0 {
        return Err(Error::Params("szip header (non-raw) streams"));
    }
    let interleave = matches!(p.bits_per_pixel, 32 | 64);
    let pb = pixel_bytes(p.bits_per_pixel);
    let (bps, sample_bytes) = if interleave {
        (8, 1)
    } else {
        (p.bits_per_pixel, pb)
    };
    let ppb = p.pixels_per_block;
    let pps = p.pixels_per_scanline;
    let mut flags = 0u8;
    if p.options & MSB_OPTION != 0 {
        flags |= MSB;
    }
    if p.options & NN != 0 {
        flags |= PREPROCESS;
    }
    let rsi = pps.div_ceil(ppb);
    let padded = !pps.is_multiple_of(ppb);
    let params = Params {
        bits_per_sample: bps,
        block_size: ppb,
        rsi,
        flags,
    };
    // Bounds block size and interval before the padded-line arithmetic below.
    params.check()?;
    let samples = out_len / sample_bytes;
    let raw = if padded {
        // Decode whole padded scanlines, then drop each one's padding.
        let lines = samples.div_ceil(pps as usize);
        let line = (rsi * ppb) as usize;
        let full = decode_bytes(input, &params, lines * line)?;
        let mut v = Vec::with_capacity(samples * sample_bytes);
        for l in full.chunks(line * sample_bytes) {
            let take = (pps as usize * sample_bytes).min(samples * sample_bytes - v.len());
            v.extend_from_slice(&l[..take]);
            if v.len() >= samples * sample_bytes {
                break;
            }
        }
        v
    } else {
        decode_bytes(input, &params, samples)?
    };
    if !interleave {
        return Ok(raw);
    }
    // Byte planes back into pixels.
    let n = out_len / pb;
    let mut out = vec![0u8; n * pb];
    for (j, plane) in raw.chunks(n).take(pb).enumerate() {
        for (i, &b) in plane.iter().enumerate() {
            out[i * pb + j] = b;
        }
    }
    Ok(out)
}
