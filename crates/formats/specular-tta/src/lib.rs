//! TTA (True Audio, `.tta`): the lossless coder of Aleksander Djuric's open format
//! description (TTA1: 8-, 16- and 24-bit integer PCM, up to 8 channels).
//!
//! Own code, from the published format: a 22-byte header ("TTA1", format, channels, bits,
//! rate, frames a channel, CRC-32), a seek table of frame sizes, then frames of
//! `256·rate/245` samples a channel. Each frame starts every channel's coder afresh: an
//! adaptive Rice code (two parameters following running sums), an 8-tap sign-sign adaptive
//! filter, a fixed first-order predictor, and channels coded as differences from the
//! next, the last about half the one before it. Frames end on a byte boundary with their
//! CRC-32.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Unsupported(String),
    Malformed(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unsupported(m) => write!(f, "unsupported TTA file: {m}"),
            Error::Malformed(m) => write!(f, "damaged TTA file: {m}"),
        }
    }
}

impl std::error::Error for Error {}

fn bad(m: &str) -> Error {
    Error::Malformed(m.into())
}

pub const MAX_CHANNELS: u16 = 8;

/// A TTA file's header and frames.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub channels: u16,
    pub bits: u16,
    pub sample_rate: u32,
    /// Samples a channel.
    pub samples: u64,
    /// Samples a channel in a frame (the last may be shorter).
    pub frame_len: u32,
    /// Each frame's (offset, length in bytes, samples a channel).
    pub frames: Vec<Frame>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub offset: usize,
    pub len: usize,
    pub samples: u32,
}

fn le16(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}
fn le32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

/// CRC-32 (IEEE, reflected), as the header, seek table and frames carry it.
pub fn crc32(d: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in d {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0xEDB8_8320
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// Bytes of ID3v2 tags before the stream.
pub fn skip_id3v2(d: &[u8]) -> usize {
    let mut at = 0;
    while let Some(h) = d.get(at..at + 10) {
        if &h[..3] != b"ID3" || h[6..10].iter().any(|b| b & 0x80 != 0) {
            break;
        }
        let size = h[6..10]
            .iter()
            .fold(0usize, |a, &b| (a << 7) | usize::from(b));
        at += 10 + size + if h[5] & 0x10 != 0 { 10 } else { 0 };
    }
    at.min(d.len())
}

/// Whether `d` (after any ID3v2 tag) starts a TTA1 stream.
pub fn is_tta(d: &[u8]) -> bool {
    d.get(skip_id3v2(d)..)
        .is_some_and(|s| s.starts_with(b"TTA1"))
}

/// Reads the header and seek table.
pub fn info(d: &[u8]) -> Result<Info, Error> {
    let start = skip_id3v2(d);
    let h = d
        .get(start..start + 22)
        .ok_or_else(|| bad("header cut short"))?;
    if &h[..4] != b"TTA1" {
        return Err(bad("no TTA1 header"));
    }
    let format = le16(h, 4).unwrap_or(0);
    let channels = le16(h, 6).unwrap_or(0);
    let bits = le16(h, 8).unwrap_or(0);
    let sample_rate = le32(h, 10).unwrap_or(0);
    let samples = u64::from(le32(h, 14).unwrap_or(0));
    match format {
        1 => {}
        2 => return Err(Error::Unsupported("password-protected stream".into())),
        3 => return Err(Error::Unsupported("floating-point samples".into())),
        f => return Err(Error::Unsupported(format!("format {f}"))),
    }
    if crc32(&h[..18]) != le32(h, 18).unwrap_or(0) {
        return Err(bad("header checksum"));
    }
    if channels == 0 || channels > MAX_CHANNELS {
        return Err(Error::Unsupported(format!("{channels} channels")));
    }
    if !matches!(bits, 8 | 16 | 24) {
        return Err(Error::Unsupported(format!("{bits}-bit samples")));
    }
    if sample_rate == 0 || sample_rate > 1_000_000 {
        return Err(bad("sampling rate"));
    }
    let frame_len = (256 * u64::from(sample_rate) / 245) as u32;
    let n = samples.div_ceil(u64::from(frame_len)) as usize;
    let table = start + 22;
    let first = table + 4 * n + 4;
    if first > d.len() {
        return Err(bad("seek table cut short"));
    }
    let mut frames = Vec::with_capacity(n);
    let mut at = first;
    let mut left = samples;
    for k in 0..n {
        let len = le32(d, table + 4 * k).unwrap_or(0) as usize;
        let s = left.min(u64::from(frame_len)) as u32;
        left -= u64::from(s);
        if at + len > d.len() {
            // A cut file: the frames that are there.
            break;
        }
        frames.push(Frame {
            offset: at,
            len,
            samples: s,
        });
        at += len;
    }
    Ok(Info {
        channels,
        bits,
        sample_rate,
        samples,
        frame_len,
        frames,
    })
}

/// LSB-first bit reader.
struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
}

impl Bits<'_> {
    #[inline]
    fn bit(&mut self) -> Option<u32> {
        let b = *self.d.get(self.pos >> 3)?;
        let v = (b >> (self.pos & 7)) & 1;
        self.pos += 1;
        Some(u32::from(v))
    }

    #[inline]
    fn bits(&mut self, n: u32) -> Option<u32> {
        let mut v = 0u32;
        for i in 0..n {
            v |= self.bit()? << i;
        }
        Some(v)
    }

    /// Ones up to a zero.
    #[inline]
    fn unary(&mut self) -> Option<u32> {
        let mut n = 0u32;
        loop {
            if self.pos & 7 == 0 && self.d.get(self.pos >> 3) == Some(&0xFF) {
                n += 8;
                self.pos += 8;
            } else if self.bit()? == 1 {
                n += 1;
            } else {
                return Some(n);
            }
            if n > 1 << 24 {
                return None;
            }
        }
    }
}

/// One channel's adaptive Rice code.
#[derive(Clone, Copy)]
struct Rice {
    k0: u32,
    k1: u32,
    sum0: u32,
    sum1: u32,
}

/// `1 << (k + 4)`: the running sum's bounds for parameter `k`.
#[inline]
fn shift16(k: u32) -> u32 {
    1u32 << (k + 4).min(31)
}

impl Rice {
    fn new() -> Rice {
        Rice {
            k0: 10,
            k1: 10,
            sum0: shift16(10),
            sum1: shift16(10),
        }
    }

    fn read(&mut self, b: &mut Bits) -> Option<i32> {
        let mut unary = b.unary()?;
        let (depth, k) = if unary == 0 {
            (0, self.k0)
        } else {
            unary -= 1;
            (1, self.k1)
        };
        let mut value = if k > 0 {
            (unary.checked_shl(k)?).wrapping_add(b.bits(k)?)
        } else {
            unary
        };
        if depth == 1 {
            self.sum1 = self.sum1.wrapping_add(value.wrapping_sub(self.sum1 >> 4));
            if self.k1 > 0 && self.sum1 < shift16(self.k1) {
                self.k1 -= 1;
            } else if self.sum1 > shift16(self.k1 + 1) {
                self.k1 += 1;
            }
            value = value.wrapping_add(1u32 << self.k0.min(31));
        }
        self.sum0 = self.sum0.wrapping_add(value.wrapping_sub(self.sum0 >> 4));
        if self.k0 > 0 && self.sum0 < shift16(self.k0) {
            self.k0 -= 1;
        } else if self.sum0 > shift16(self.k0 + 1) {
            self.k0 += 1;
        }
        if self.k0 > 27 || self.k1 > 27 {
            return None;
        }
        // Odd values are positive.
        Some(if value & 1 != 0 {
            ((value >> 1) + 1) as i32
        } else {
            -((value >> 1) as i32)
        })
    }
}

/// One channel's sign-sign adaptive filter.
#[derive(Clone, Copy)]
struct Filter {
    shift: u32,
    round: i32,
    error: i32,
    qm: [i32; 8],
    dx: [i32; 8],
    dl: [i32; 8],
}

impl Filter {
    fn new(bits: u16) -> Filter {
        let shift = match bits {
            8 => 10,
            16 => 9,
            _ => 10,
        };
        Filter {
            shift,
            round: 1 << (shift - 1),
            error: 0,
            qm: [0; 8],
            dx: [0; 8],
            dl: [0; 8],
        }
    }

    #[inline]
    fn run(&mut self, v: i32) -> i32 {
        let (a, b, m) = (&mut self.dl, &mut self.qm, &mut self.dx);
        if self.error < 0 {
            for i in 0..8 {
                b[i] = b[i].wrapping_sub(m[i]);
            }
        } else if self.error > 0 {
            for i in 0..8 {
                b[i] = b[i].wrapping_add(m[i]);
            }
        }
        let mut sum = self.round;
        for i in 0..8 {
            sum = sum.wrapping_add(a[i].wrapping_mul(b[i]));
        }
        m.copy_within(1..5, 0);
        a.copy_within(1..5, 0);
        m[4] = (a[4] >> 30) | 1;
        m[5] = ((a[5] >> 28) | 2) & !1;
        m[6] = ((a[6] >> 28) | 2) & !1;
        m[7] = ((a[7] >> 27) | 4) & !3;
        self.error = v;
        let out = v.wrapping_add(sum >> self.shift);
        a[4] = a[5].wrapping_neg();
        a[5] = a[6].wrapping_neg();
        a[6] = out.wrapping_sub(a[7]);
        a[7] = out;
        a[5] = a[5].wrapping_add(a[6]);
        a[4] = a[4].wrapping_add(a[5]);
        out
    }
}

/// The decoder of one file's frames.
pub struct Decoder {
    channels: usize,
    bits: u16,
}

impl Decoder {
    pub fn new(info: &Info) -> Decoder {
        Decoder {
            channels: usize::from(info.channels),
            bits: info.bits,
        }
    }

    /// Decodes one frame of `samples` samples a channel, appending interleaved integers to
    /// `out`. A frame whose checksum doesn't match is still decoded (the error says so after).
    pub fn decode(&self, frame: &[u8], samples: u32, out: &mut Vec<i32>) -> Result<(), Error> {
        let ch = self.channels;
        let body = frame.len().saturating_sub(4);
        let crc_ok = frame.len() >= 4 && crc32(&frame[..body]) == le32(frame, body).unwrap_or(0);
        let mut b = Bits {
            d: &frame[..body],
            pos: 0,
        };
        let mut rice = vec![Rice::new(); ch];
        let mut filt = vec![Filter::new(self.bits); ch];
        let mut prev = vec![0i32; ch];
        let k = if self.bits == 8 { 4 } else { 5 };
        let base = out.len();
        out.reserve(samples as usize * ch);
        let mut cur = vec![0i32; ch];
        for _ in 0..samples {
            for c in 0..ch {
                let Some(r) = rice[c].read(&mut b) else {
                    out.resize(base + samples as usize * ch, 0);
                    return Err(bad("frame cut short"));
                };
                let mut v = filt[c].run(r);
                let p = prev[c];
                v = v.wrapping_add(p.wrapping_shl(k).wrapping_sub(p) >> k);
                prev[c] = v;
                cur[c] = v;
            }
            if ch > 1 {
                let l = ch - 1;
                cur[l] = cur[l].wrapping_add(cur[l - 1] / 2);
                for i in (0..l).rev() {
                    cur[i] = cur[i + 1].wrapping_sub(cur[i]);
                }
            }
            out.extend_from_slice(&cur);
        }
        if !crc_ok {
            return Err(bad("frame checksum"));
        }
        Ok(())
    }

    /// The scale from samples to ±1.0.
    pub fn scale(&self) -> f32 {
        1.0 / (1u32 << (self.bits - 1)) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_of_check_string() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }
}
