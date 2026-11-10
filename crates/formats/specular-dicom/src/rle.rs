//! RLE Lossless (PS3.5 G): a 64-byte header of up to 15 segment offsets, each segment one
//! byte plane (most significant byte first per sample) compressed with PackBits.

use crate::pixel::{PixelInfo, alloc};
use crate::{Error, Result};

/// Decodes one RLE frame to native little-endian, colour-by-plane bytes (`truncated` when a
/// segment ended early). The caller unpacks it with planar configuration 1.
pub(crate) fn decode(data: &[u8], info: &PixelInfo) -> Result<(Vec<u8>, bool)> {
    let header = data.get(..64).ok_or(Error::Truncated)?;
    let word = |i: usize| {
        u32::from_le_bytes([
            header[i * 4],
            header[i * 4 + 1],
            header[i * 4 + 2],
            header[i * 4 + 3],
        ]) as usize
    };
    let count = word(0);
    let samples = info.samples as usize;
    let bps = (info.bits_allocated as usize / 8).max(1);
    if count == 0 || count > 15 || count != bps * samples {
        return Err(Error::Malformed(
            "RLE segment count does not match the image",
        ));
    }
    if info.bits_allocated == 1 {
        if samples != 1 {
            return Err(Error::Unsupported("1-bit colour RLE".into()));
        }
        return one_bit(data.get(word(1)..).unwrap_or(&[]), info.pixels() as usize);
    }
    let n = info.pixels() as usize;
    let mut out = alloc::<u8>(n * bps * samples)?;
    let mut truncated = false;
    for seg in 0..count {
        let start = word(seg + 1);
        let end = if seg + 1 < count {
            word(seg + 2)
        } else {
            data.len()
        };
        let src = data
            .get(start..end.max(start).min(data.len()))
            .unwrap_or(&[]);
        let (sample, byte) = (seg / bps, seg % bps);
        // Little-endian position of this byte plane: segment 0 holds the most significant.
        let lane = bps - 1 - byte;
        let base = sample * n * bps;
        let written = packbits(src, n, |i, b| out[base + i * bps + lane] = b);
        truncated |= written < n;
    }
    Ok((out, truncated))
}

/// A 1-bit frame's single segment. Encoders write either the packed bits (LSB first, as in
/// native data) or one byte per pixel; the decoded length tells them apart. Returns packed
/// bits.
fn one_bit(src: &[u8], pixels: usize) -> Result<(Vec<u8>, bool)> {
    let packed = pixels.div_ceil(8);
    let mut bytes = alloc::<u8>(pixels)?;
    let written = packbits(src, pixels, |i, b| bytes[i] = b);
    if written <= packed {
        bytes.truncate(packed);
        return Ok((bytes, written < packed));
    }
    let mut out = alloc::<u8>(packed)?;
    for (o, chunk) in out.iter_mut().zip(bytes.chunks(8)) {
        *o = chunk
            .iter()
            .enumerate()
            .fold(0, |acc, (k, &b)| acc | (u8::from(b != 0) << k));
    }
    Ok((out, written < pixels))
}

/// PackBits: `put(i, byte)` for output bytes `0..n`; returns how many were produced.
fn packbits(src: &[u8], n: usize, mut put: impl FnMut(usize, u8)) -> usize {
    let (mut i, mut o) = (0, 0);
    while o < n && i < src.len() {
        let h = src[i] as i8;
        i += 1;
        if h >= 0 {
            let len = h as usize + 1;
            let end = (i + len).min(src.len());
            for &b in &src[i..end] {
                if o >= n {
                    break;
                }
                put(o, b);
                o += 1;
            }
            i = end;
        } else if h != -128 {
            let Some(&b) = src.get(i) else { break };
            i += 1;
            for _ in 0..(1 - h as isize) as usize {
                if o >= n {
                    break;
                }
                put(o, b);
                o += 1;
            }
        }
    }
    o
}

/// Encodes byte planes with PackBits (literal runs only plus simple repeats): test fixtures.
#[cfg(test)]
pub(crate) fn encode(planes: &[Vec<u8>]) -> Vec<u8> {
    let mut segs: Vec<Vec<u8>> = Vec::new();
    for p in planes {
        let mut s = Vec::new();
        let mut i = 0;
        while i < p.len() {
            let mut run = 1;
            while i + run < p.len() && run < 128 && p[i + run] == p[i] {
                run += 1;
            }
            if run >= 3 {
                s.push((1 - run as i32) as i8 as u8);
                s.push(p[i]);
                i += run;
            } else {
                let len = (p.len() - i).min(128);
                let len = (1..len)
                    .find(|&k| {
                        i + k + 2 < p.len() && p[i + k] == p[i + k + 1] && p[i + k] == p[i + k + 2]
                    })
                    .unwrap_or(len);
                s.push((len - 1) as u8);
                s.extend(&p[i..i + len]);
                i += len;
            }
        }
        if s.len() % 2 == 1 {
            s.push(0x80);
        }
        segs.push(s);
    }
    let mut out = vec![0u8; 64];
    out[..4].copy_from_slice(&(segs.len() as u32).to_le_bytes());
    let mut off = 64u32;
    for (k, s) in segs.iter().enumerate() {
        out[4 + k * 4..8 + k * 4].copy_from_slice(&off.to_le_bytes());
        off += s.len() as u32;
    }
    for s in segs {
        out.extend(s);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pixel::Photometric;

    #[test]
    fn round_trips_16_bit() {
        let info = PixelInfo {
            rows: 3,
            columns: 5,
            samples: 1,
            bits_allocated: 16,
            bits_stored: 16,
            high_bit: 15,
            signed: false,
            planar: false,
            photometric: Photometric::Monochrome2,
            frames: 1,
            float: false,
        };
        let vals: Vec<u16> = (0..15u16)
            .map(|i| if i < 6 { 7 } else { i * 1000 })
            .collect();
        let hi: Vec<u8> = vals.iter().map(|v| (v >> 8) as u8).collect();
        let lo: Vec<u8> = vals.iter().map(|v| *v as u8).collect();
        let enc = encode(&[hi, lo]);
        let (out, t) = decode(&enc, &info).unwrap();
        assert!(!t);
        let back: Vec<u16> = out
            .chunks(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(back, vals);
        assert!(decode(&enc[..70], &info).unwrap().1);
        assert!(decode(&enc[..10], &info).is_err());
    }
}
