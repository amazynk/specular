//! Kodak sensor codecs.
//!
//! Compression 65000 (DCS Pro 14n/14nx/SLR, DCS760 and relatives): each row is cut into chunks
//! of up to 256 samples, each chunk independently coded. A chunk starts with one 4-bit length
//! per sample (the count rounded up to a multiple of 4); then follow the differences, read
//! LSB-first from pairs of big-endian 16-bit words. Even and odd samples are predicted
//! separately from the previous sample of the same parity, starting at zero. A length above 12
//! marks a chunk stored as plain 12-bit samples instead (eight samples in six 16-bit words). The
//! values index a linearisation curve. Files carry the end offset of every chunk (tag 0xfde9),
//! which lets rows decode in parallel.

use rayon::prelude::*;

use crate::error::Result;

/// Samples per coded chunk.
const CHUNK: usize = 256;

/// The parameters of a compression-65000 sensor.
#[derive(Clone, Debug)]
pub(crate) struct K65000 {
    /// End offset (relative to the data start) of each chunk, in row-major chunk order; empty
    /// when the file does not list them.
    pub ends: Vec<u32>,
    /// Byte order of the stored plain chunks.
    pub le: bool,
    /// Linearisation curve (empty = identity).
    pub curve: Vec<u16>,
}

/// LSB-first bit reader over big-endian 16-bit word pairs.
struct Bits<'a> {
    src: &'a [u8],
    pos: usize,
    buf: u64,
    n: u32,
    short: bool,
}

impl Bits<'_> {
    fn byte(&mut self) -> u64 {
        match self.src.get(self.pos) {
            Some(&b) => {
                self.pos += 1;
                b as u64
            }
            None => {
                self.pos += 1;
                self.short = true;
                0
            }
        }
    }

    #[inline(always)]
    fn take(&mut self, len: u32) -> u32 {
        if self.n < len {
            let (a, b, c, d) = (self.byte(), self.byte(), self.byte(), self.byte());
            self.buf |= ((a << 8) | b | (c << 24) | (d << 16)) << self.n;
            self.n += 32;
        }
        let v = (self.buf & ((1u64 << len) - 1)) as u32;
        self.buf >>= len;
        self.n -= len;
        v
    }
}

/// Decodes one chunk of `out.len()` samples (before the curve) from `src`. Returns the bytes
/// consumed and whether the data ran short.
fn chunk(src: &[u8], le: bool, out: &mut [i32; CHUNK], n: usize) -> (usize, bool, bool) {
    let padded = (n + 3) & !3;
    let mut lens = [0u8; CHUNK];
    let mut plain = false;
    for i in (0..padded).step_by(2) {
        let c = src.get(i / 2).copied().unwrap_or(0);
        lens[i] = c & 15;
        lens[i + 1] = c >> 4;
        if lens[i] > 12 || lens[i + 1] > 12 {
            plain = true;
            break;
        }
    }
    if plain {
        // Eight samples in six words: the low 12 bits of each word, then the top nibbles of
        // words 0, 2, 4 and 1, 3, 5 make two more.
        let mut short = false;
        let word = |k: usize, short: &mut bool| -> u32 {
            match src.get(k..k + 2) {
                Some(b) if le => u16::from_le_bytes([b[0], b[1]]) as u32,
                Some(b) => u16::from_be_bytes([b[0], b[1]]) as u32,
                None => {
                    *short = true;
                    0
                }
            }
        };
        let mut i = 0;
        let mut p = 0;
        while i < padded {
            let mut r = [0u32; 6];
            for (j, v) in r.iter_mut().enumerate() {
                *v = word(p + 2 * j, &mut short);
            }
            p += 12;
            let mut put = |k: usize, v: u32| {
                if k < CHUNK {
                    out[k] = v as i32;
                }
            };
            put(i, (r[0] >> 12) << 8 | (r[2] >> 12) << 4 | r[4] >> 12);
            put(i + 1, (r[1] >> 12) << 8 | (r[3] >> 12) << 4 | r[5] >> 12);
            for (j, &v) in r.iter().enumerate() {
                put(i + 2 + j, v & 0xfff);
            }
            i += 8;
        }
        return (p, short, true);
    }
    let mut b = Bits {
        src,
        pos: padded / 2,
        buf: 0,
        n: 0,
        short: false,
    };
    if padded & 7 == 4 {
        let (x, y) = (b.byte(), b.byte());
        b.buf = (x << 8) | y;
        b.n = 16;
    }
    for i in 0..n {
        let len = lens[i] as u32;
        if len == 0 {
            out[i] = 0;
            continue;
        }
        let mut v = b.take(len) as i32;
        if v & (1 << (len - 1)) == 0 {
            v -= (1 << len) - 1;
        }
        out[i] = v;
    }
    // The lengths of the padding samples still take their bits.
    for &len in &lens[n..padded] {
        if len > 0 {
            b.take(len as u32);
        }
    }
    (b.pos.min(src.len()), b.short, false)
}

/// Decodes one row (all its chunks) starting at `src`; returns bytes consumed and shortness.
fn row(src: &[u8], k: &K65000, out: &mut [u16], starts: Option<&[usize]>) -> (usize, bool) {
    let w = out.len();
    let max = k.curve.len().saturating_sub(1);
    let mut buf = [0i32; CHUNK];
    let mut pos = 0;
    let mut short = false;
    for (ci, col) in (0..w).step_by(CHUNK).enumerate() {
        let n = CHUNK.min(w - col);
        if let Some(s) = starts {
            pos = s.get(ci).copied().unwrap_or(usize::MAX);
        }
        let (used, sh, plain) = chunk(src.get(pos..).unwrap_or(&[]), k.le, &mut buf, n);
        short |= sh;
        pos = pos.saturating_add(used);
        let mut pred = [0i32; 2];
        for (i, o) in out[col..col + n].iter_mut().enumerate() {
            let v = if plain {
                buf[i]
            } else {
                pred[i & 1] += buf[i];
                pred[i & 1]
            };
            *o = if k.curve.is_empty() {
                v.clamp(0, 65535) as u16
            } else {
                k.curve[(v.max(0) as usize).min(max)]
            };
        }
    }
    (pos, short)
}

/// Decodes `w x h` samples. Returns whether the data ran short.
pub(crate) fn decode_65000(
    src: &[u8],
    w: usize,
    h: usize,
    k: &K65000,
    out: &mut [u16],
    parallel: bool,
) -> Result<bool> {
    let per_row = w.div_ceil(CHUNK);
    let out = &mut out[..w * h];
    if w == 0 || h == 0 {
        return Ok(false);
    }
    let listed = k.ends.len() >= per_row * h && k.ends.windows(2).all(|p| p[0] <= p[1]);
    if !listed {
        let mut pos = 0;
        let mut short = false;
        for r in out.chunks_exact_mut(w) {
            let (used, sh) = row(src.get(pos..).unwrap_or(&[]), k, r, None);
            short |= sh;
            pos = pos.saturating_add(used);
        }
        return Ok(short);
    }
    let work = |(y, r): (usize, &mut [u16])| -> bool {
        let first = y * per_row;
        let starts: Vec<usize> = (first..first + per_row)
            .map(|i| if i == 0 { 0 } else { k.ends[i - 1] as usize })
            .collect();
        row(src, k, r, Some(&starts)).1
    };
    let short = if parallel {
        out.par_chunks_mut(w)
            .enumerate()
            .map(work)
            .reduce(|| false, |a, b| a | b)
    } else {
        out.chunks_mut(w)
            .enumerate()
            .map(work)
            .fold(false, |a, b| a | b)
    };
    Ok(short)
}

/// DC120 sensor size: the TIFF tags give the 1280x960 output size instead.
pub(crate) const DC120: (usize, usize) = (848, 976);

/// Kodak DC120: 8-bit rows, each rotated left by an amount set by the row number.
pub(crate) fn decode_dc120(src: &[u8], out: &mut [u16]) -> bool {
    const MUL: [usize; 4] = [162, 192, 187, 92];
    const ADD: [usize; 4] = [0, 636, 424, 212];
    let (w, h) = DC120;
    for (row, dst) in out.chunks_exact_mut(w).take(h).enumerate() {
        let Some(line) = src.get(row * w..row * w + w) else {
            return true;
        };
        let shift = (row * MUL[row & 3] + ADD[row & 3]) % w;
        for (col, o) in dst.iter_mut().enumerate() {
            *o = line[(col + shift) % w] as u16;
        }
    }
    false
}

/// Kodak DC120 with JPEG compression: a baseline JPEG with the bytes of every 16-bit word
/// swapped, half the sensor's height, whose decoded RGB pixels each carry two rows of a
/// two-column Bayer group: green of both rows from G (doubled), red from the R of the pair,
/// blue from its B (each summed over the two pixels).
pub(crate) fn decode_dc120_jpeg(src: &[u8], out: &mut [u16]) -> crate::error::Result<bool> {
    let (w, h) = DC120;
    let mut buf = src.to_vec();
    for p in buf.chunks_exact_mut(2) {
        p.swap(0, 1);
    }
    // The size first: the frame header alone would size the decode.
    let info = specular_jpeg::read_info(&buf)?;
    if info.width as usize != w || info.height as usize * 2 != h {
        return Err(crate::error::RawError::Malformed("DC120 JPEG size"));
    }
    let planes = specular_jpeg::decode(&buf, &specular_jpeg::DecodeOptions::default())?;
    if planes.width as usize != w || planes.height as usize * 2 != h {
        return Err(crate::error::RawError::Malformed("DC120 JPEG size"));
    }
    let rgba = specular_jpeg::planes_to_rgba(&planes);
    for (r, line) in rgba.chunks_exact(w * 4).enumerate() {
        let (top, rest) = out[2 * r * w..].split_at_mut(w);
        let bottom = &mut rest[..w];
        for c in (0..w).step_by(2) {
            let (a, b) = (&line[c * 4..c * 4 + 4], &line[c * 4 + 4..c * 4 + 8]);
            top[c] = (a[1] as u16) << 1;
            bottom[c + 1] = (b[1] as u16) << 1;
            top[c + 1] = a[0] as u16 + b[0] as u16;
            bottom[c] = a[2] as u16 + b[2] as u16;
        }
    }
    Ok(planes.truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(curve: Vec<u16>) -> K65000 {
        K65000 {
            ends: vec![],
            le: false,
            curve,
        }
    }

    #[test]
    fn coded_chunk() {
        // Four samples: lengths 2,0,2,0 (bytes 0x02, 0x02), then the bits LSB-first from the
        // word pair: 0b11 (+3) then 0b01 (1 - 3 = -2).
        // Four lengths take two bytes, leaving the count at 4 mod 8, so the next two bytes
        // preload 16 bits (first byte high): 0b0111 in the low bits.
        let src = [0x02, 0x02, 0x00, 0x07, 0, 0];
        let mut out = [0u16; 4];
        assert!(!decode_65000(&src, 4, 1, &k(vec![]), &mut out, false).unwrap());
        // Even samples: 3, then 3 - 2 = 1; odd samples stay 0.
        assert_eq!(out, [3, 0, 1, 0]);
        // Through a curve.
        let curve: Vec<u16> = (0..16).map(|v| v * 10).collect();
        decode_65000(&src, 4, 1, &k(curve), &mut out, false).unwrap();
        assert_eq!(out, [30, 0, 10, 0]);
    }

    #[test]
    fn plain_chunk() {
        // A length nibble of 15 switches to plain 12-bit words (big-endian).
        // The first byte doubles as the first two length nibbles: 0xf0 reads as 0 and 15.
        let words: [u16; 6] = [0xf001, 0x2002, 0x3003, 0x4004, 0x5005, 0x6006];
        let src: Vec<u8> = words.iter().flat_map(|w| w.to_be_bytes()).collect();
        let mut out = [0u16; 8];
        decode_65000(&src, 8, 1, &k(vec![]), &mut out, false).unwrap();
        assert_eq!(out[0], 0xf35);
        assert_eq!(out[1], 0x246);
        assert_eq!(&out[2..], &[0x001, 0x002, 0x003, 0x004, 0x005, 0x006]);
    }

    /// A baseline grey JPEG frame header of 65535 × 65535 with 1 MiB of zero data (enough
    /// for the JPEG decoder's own data-size check: two bits per block): it was decoded before
    /// its size was compared with the DC120's, asking for 4 GiB.
    #[test]
    fn dc120_jpeg_of_the_wrong_size_is_refused_before_decoding() {
        let mut j = vec![0xff, 0xd8];
        j.extend_from_slice(&[0xff, 0xdb, 0, 67, 0]);
        j.extend_from_slice(&[1; 64]);
        j.extend_from_slice(&[0xff, 0xc0, 0, 11, 8, 0xff, 0xff, 0xff, 0xff, 1, 1, 0x11, 0]);
        // DC and AC tables with one 1-bit code each (DC category 0, AC end of block).
        let mut one = vec![1u8];
        one.extend_from_slice(&[0; 15]);
        j.extend_from_slice(&[0xff, 0xc4, 0, 20, 0x00]);
        j.extend_from_slice(&one);
        j.push(0);
        j.extend_from_slice(&[0xff, 0xc4, 0, 20, 0x10]);
        j.extend_from_slice(&one);
        j.push(0);
        j.extend_from_slice(&[0xff, 0xda, 0, 8, 1, 1, 0x00, 0, 63, 0]);
        j.resize(j.len() + (1 << 20), 0);
        j.extend_from_slice(&[0xff, 0xd9]);
        if j.len() % 2 == 1 {
            j.push(0);
        }
        let info = specular_jpeg::read_info(&j).unwrap();
        assert_eq!((info.width, info.height), (65535, 65535));
        // decode_dc120_jpeg swaps byte pairs back.
        for p in j.chunks_exact_mut(2) {
            p.swap(0, 1);
        }
        let mut out = vec![0u16; DC120.0 * DC120.1];
        assert!(decode_dc120_jpeg(&j, &mut out).is_err());
    }

    #[test]
    fn dc120_rotation() {
        let (w, h) = DC120;
        let src: Vec<u8> = (0..w * h).map(|i| (i % 251) as u8).collect();
        let mut out = vec![0u16; w * h];
        assert!(!decode_dc120(&src, &mut out));
        // Row 1 starts 192 + 636 samples in.
        assert_eq!(out[w], src[w + (192 + 636) % w] as u16);
        assert!(decode_dc120(&src[..w * 3], &mut out));
    }

    #[test]
    fn truncated_and_hostile() {
        let mut out = vec![0u16; 300 * 3];
        let src = [0x22u8; 40];
        assert!(decode_65000(&src, 300, 3, &k(vec![]), &mut out, true).unwrap());
        assert!(decode_65000(&[], 300, 3, &k(vec![]), &mut out, false).unwrap());
        let bad = K65000 {
            ends: vec![u32::MAX; 6],
            le: true,
            curve: vec![1],
        };
        let _ = decode_65000(&src, 300, 3, &bad, &mut out, true).unwrap();
    }
}
