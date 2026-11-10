//! Own FLAC decoder (RFC 9639) for the sound of Matroska, MP4, Ogg and native FLAC files.
//!
//! Supported: every frame the RFC defines. Fixed and variable block sizes from 1 to 65535,
//! every sample rate code, 1 to 8 channels with independent, left/side, side/right and
//! mid/side coding, bit depths 4 to 32 (the 33-bit side channel of 32-bit stereo included),
//! constant, verbatim, fixed (orders 0 to 4) and linear predictive (orders 1 to 32) subframes
//! with wasted bits, and Rice partitions with 4- and 5-bit parameters and escape codes.
//! The frame header CRC-8 and the frame CRC-16 are checked.
//!
//! Refused as [`Error::Malformed`]: reserved or forbidden codes (block size 65536, negative
//! LPC shift, precision code 15, reserved channel layouts and subframe types), checksum
//! mismatches and frames cut short. Metadata other than STREAMINFO is not read; seeking and
//! MD5 checking are left to the caller.

#![deny(unsafe_code)]

mod bits;
mod frame;

use std::fmt;

use frame::{Assign, Header};

/// Why a stream or frame could not be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A valid variant this decoder does not handle (a plain name of it).
    Unsupported(String),
    /// Damaged or invalid data.
    Malformed(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unsupported(what) => write!(f, "unsupported FLAC: {what}"),
            Error::Malformed(what) => write!(f, "damaged FLAC: {what}"),
        }
    }
}

impl std::error::Error for Error {}

fn bad(what: &str) -> Error {
    Error::Malformed(what.to_string())
}

/// The STREAMINFO metadata block. Zero sizes, total and MD5 mean "unknown".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamInfo {
    pub min_block: u16,
    pub max_block: u16,
    pub min_frame: u32,
    pub max_frame: u32,
    pub sample_rate: u32,
    pub channels: u8,
    pub bits: u8,
    pub total_samples: u64,
    pub md5: [u8; 16],
}

impl StreamInfo {
    /// The 34-byte STREAMINFO block body.
    pub fn parse(body: &[u8]) -> Result<StreamInfo, Error> {
        let b = body
            .get(..34)
            .ok_or_else(|| bad("STREAMINFO shorter than 34 bytes"))?;
        let be = |r: std::ops::Range<usize>| b[r].iter().fold(0u64, |a, &x| a << 8 | u64::from(x));
        let packed = be(10..18);
        let mut md5 = [0u8; 16];
        md5.copy_from_slice(&b[18..34]);
        let info = StreamInfo {
            min_block: be(0..2) as u16,
            max_block: be(2..4) as u16,
            min_frame: be(4..7) as u32,
            max_frame: be(7..10) as u32,
            sample_rate: (packed >> 44) as u32,
            channels: ((packed >> 41) & 7) as u8 + 1,
            bits: ((packed >> 36) & 31) as u8 + 1,
            total_samples: packed & ((1 << 36) - 1),
            md5,
        };
        if info.bits < 4 {
            return Err(bad("bit depth below 4"));
        }
        Ok(info)
    }

    /// From the bytes a container stores ahead of the frames: "fLaC" followed by metadata blocks
    /// (Matroska CodecPrivate, a native file's start), an MP4 `dfLa` box body (4 bytes version+flags,
    /// then metadata blocks), or Ogg FLAC's first packet (0x7F "FLAC" major minor count "fLaC" STREAMINFO block).
    pub fn from_config(b: &[u8]) -> Result<StreamInfo, Error> {
        if let Some(rest) = b.strip_prefix(b"fLaC") {
            return Self::from_blocks(rest);
        }
        if let Some(rest) = b.strip_prefix(b"\x7fFLAC") {
            let major = *rest
                .first()
                .ok_or_else(|| bad("Ogg FLAC header cut short"))?;
            if major != 1 {
                return Err(Error::Unsupported(format!(
                    "Ogg FLAC mapping version {major}"
                )));
            }
            let blocks = rest
                .get(4..)
                .and_then(|r| r.strip_prefix(b"fLaC"))
                .ok_or_else(|| bad("Ogg FLAC header without fLaC"))?;
            return Self::from_blocks(blocks);
        }
        if b.len() == 34 {
            return Self::parse(b);
        }
        if let Some(rest) = b.strip_prefix(&[0, 0, 0, 0]) {
            return Self::from_blocks(rest);
        }
        if b.first().is_some_and(|&t| t & 0x7F == 0) {
            return Self::from_blocks(b);
        }
        Err(bad("unrecognised FLAC configuration"))
    }

    /// Finds STREAMINFO among metadata blocks.
    fn from_blocks(mut d: &[u8]) -> Result<StreamInfo, Error> {
        loop {
            let h = d.get(..4).ok_or_else(|| bad("no STREAMINFO block"))?;
            let len = (usize::from(h[1]) << 16) | (usize::from(h[2]) << 8) | usize::from(h[3]);
            let body = d
                .get(4..4 + len)
                .ok_or_else(|| bad("metadata block cut short"))?;
            if h[0] & 0x7F == 0 {
                return Self::parse(body);
            }
            if h[0] & 0x80 != 0 {
                return Err(bad("no STREAMINFO block"));
            }
            d = &d[4 + len..];
        }
    }
}

/// Where decoded frames go: interleaved integers or floats.
trait Out {
    fn push(&mut self, chans: &[Vec<i32>], n: usize, bits: u8);
}

impl Out for Vec<i32> {
    fn push(&mut self, chans: &[Vec<i32>], n: usize, _: u8) {
        self.reserve(n * chans.len());
        match chans {
            [a] => self.extend_from_slice(&a[..n]),
            [a, b] => {
                for (&l, &r) in a[..n].iter().zip(&b[..n]) {
                    self.extend_from_slice(&[l, r]);
                }
            }
            _ => {
                for i in 0..n {
                    self.extend(chans.iter().map(|c| c[i]));
                }
            }
        }
    }
}

impl Out for Vec<f32> {
    fn push(&mut self, chans: &[Vec<i32>], n: usize, bits: u8) {
        let scale = 1.0 / (1u64 << (bits - 1)) as f32;
        self.reserve(n * chans.len());
        match chans {
            [a] => self.extend(a[..n].iter().map(|&s| s as f32 * scale)),
            [a, b] => {
                for (&l, &r) in a[..n].iter().zip(&b[..n]) {
                    self.extend_from_slice(&[l as f32 * scale, r as f32 * scale]);
                }
            }
            _ => {
                for i in 0..n {
                    self.extend(chans.iter().map(|c| c[i] as f32 * scale));
                }
            }
        }
    }
}

/// FLAC frame decoder. Frames are independent; the decoder only remembers the stream format.
pub struct Decoder {
    info: Option<StreamInfo>,
    channels: usize,
    rate: u32,
    bits: u8,
    /// Decoded samples per channel of the last frame.
    chans: Vec<Vec<i32>>,
    /// The two channels of 32-bit stereo with a 33-bit side channel.
    wide: [Vec<i64>; 2],
}

impl Decoder {
    /// With the STREAMINFO when the container gives it (frames may still say rate/bits themselves).
    pub fn new(info: Option<StreamInfo>) -> Decoder {
        let mut d = Decoder {
            info,
            channels: 0,
            rate: 0,
            bits: 0,
            chans: Vec::new(),
            wide: [Vec::new(), Vec::new()],
        };
        d.reset();
        d
    }

    /// Decodes whole frames in `data` (one or more, back to back: Matroska blocks, MP4 samples and
    /// Ogg packets hold one), appending interleaved f32 in -1..=1 (sample / 2^(bits-1)), channel order
    /// as coded (FLAC's order = WAVE order). Returns frames (sample instants) appended.
    ///
    /// On an error the frames before the bad one stay appended. A frame whose CRC-16 does not
    /// match is appended too, then `Err(Malformed)` is returned; the decoder stays usable.
    /// Zero bytes after the last frame are ignored.
    pub fn decode(&mut self, data: &[u8], out: &mut Vec<f32>) -> Result<usize, Error> {
        self.run(data, out)
    }

    /// The same, as integers (for exactness tests and MD5): appends interleaved i32.
    pub fn decode_i32(&mut self, data: &[u8], out: &mut Vec<i32>) -> Result<usize, Error> {
        self.run(data, out)
    }

    /// Channel count from STREAMINFO or the last frame.
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Sample rate from STREAMINFO or the last frame (0 when neither said).
    pub fn sample_rate(&self) -> u32 {
        self.rate
    }

    /// Bits per sample from STREAMINFO or the last frame.
    pub fn bits(&self) -> u8 {
        self.bits
    }

    /// Forgets what frames said about the format (back to STREAMINFO's).
    pub fn reset(&mut self) {
        let i = self.info;
        self.channels = i.map_or(0, |i| usize::from(i.channels));
        self.rate = i.map_or(0, |i| i.sample_rate);
        self.bits = i.map_or(0, |i| i.bits);
    }

    fn run<O: Out>(&mut self, data: &[u8], out: &mut O) -> Result<usize, Error> {
        let mut pos = 0;
        let mut total = 0;
        while pos < data.len() {
            let rest = &data[pos..];
            if !frame::is_sync(rest) && rest.iter().all(|&b| b == 0) {
                break;
            }
            let (h, len, crc_ok) = self.frame(rest)?;
            out.push(&self.chans[..h.channels], h.block, self.bits);
            total += h.block;
            pos += len;
            if !crc_ok {
                return Err(bad("frame checksum mismatch"));
            }
        }
        Ok(total)
    }

    /// Decodes one frame into `self.chans`: (header, bytes used, CRC-16 matched).
    fn frame(&mut self, data: &[u8]) -> Result<(Header, usize, bool), Error> {
        let h = frame::parse_header(data)?;
        let bits = match (h.bits, self.info) {
            (0, Some(i)) => i.bits,
            (0, None) => return Err(bad("frame takes its bit depth from a missing STREAMINFO")),
            (b, _) => b,
        };
        if !(4..=32).contains(&bits) {
            return Err(bad("bit depth outside 4 to 32"));
        }
        let n = h.block;
        if self.chans.len() < h.channels {
            self.chans.resize_with(h.channels, Vec::new);
        }
        for c in &mut self.chans[..h.channels] {
            c.resize(n, 0);
        }
        let body = &data[h.len..];
        let mut br = bits::Bits::new(body);
        let wide = bits == 32 && h.assign != Assign::Independent;
        for ch in 0..h.channels {
            let side = matches!(
                (h.assign, ch),
                (Assign::LeftSide, 1) | (Assign::SideRight, 0) | (Assign::MidSide, 1)
            );
            let sbits = u32::from(bits) + u32::from(side);
            if wide {
                let w = &mut self.wide[ch];
                w.resize(n, 0);
                frame::subframe(&mut br, sbits, &mut w[..n])?;
            } else {
                frame::subframe(&mut br, sbits, &mut self.chans[ch][..n])?;
            }
        }
        br.align();
        let crc_at = br.bit_pos() / 8;
        let crc = br.read(16) as u16;
        if br.overrun() {
            return Err(bad("frame cut short"));
        }
        let crc_ok = frame::crc16(&data[..h.len + crc_at]) == crc;
        if wide {
            let [a, b] = &self.wide;
            let (l, r) = self.chans.split_at_mut(1);
            decorrelate(h.assign, &a[..n], &b[..n], &mut l[0], &mut r[0]);
        } else if h.assign != Assign::Independent {
            let (l, r) = self.chans.split_at_mut(1);
            let (l, r) = (&mut l[0][..n], &mut r[0][..n]);
            stereo_i32(h.assign, l, r);
        }
        self.channels = h.channels;
        self.bits = bits;
        if h.rate != 0 {
            self.rate = h.rate;
        }
        Ok((h, h.len + crc_at + 2, crc_ok))
    }
}

/// Undoes stereo decorrelation in place on 32-bit storage (bit depths up to 31).
fn stereo_i32(assign: Assign, l: &mut [i32], r: &mut [i32]) {
    match assign {
        Assign::LeftSide => {
            for (l, r) in l.iter().zip(r.iter_mut()) {
                *r = l.wrapping_sub(*r);
            }
        }
        Assign::SideRight => {
            for (l, r) in l.iter_mut().zip(r.iter()) {
                *l = l.wrapping_add(*r);
            }
        }
        Assign::MidSide => {
            for (m, s) in l.iter_mut().zip(r.iter_mut()) {
                let side = i64::from(*s);
                let mid = (i64::from(*m) << 1) | (side & 1);
                *m = ((mid + side) >> 1) as i32;
                *s = ((mid - side) >> 1) as i32;
            }
        }
        Assign::Independent => {}
    }
}

/// Undoes stereo decorrelation from 64-bit storage (32-bit audio).
fn decorrelate(assign: Assign, a: &[i64], b: &[i64], l: &mut [i32], r: &mut [i32]) {
    for (i, (&x, &y)) in a.iter().zip(b).enumerate() {
        let (left, right) = match assign {
            Assign::LeftSide => (x, x.wrapping_sub(y)),
            Assign::SideRight => (x.wrapping_add(y), y),
            Assign::MidSide => {
                let mid = x.wrapping_shl(1) | (y & 1);
                (mid.wrapping_add(y) >> 1, mid.wrapping_sub(y) >> 1)
            }
            Assign::Independent => (x, y),
        };
        l[i] = left as i32;
        r[i] = right as i32;
    }
}

/// Splits a native FLAC stream's frame bytes (after the metadata) into frames by sync code + header CRC-8 (test helper also useful to callers).
///
/// A frame ends where the next valid header starts and the bytes before it pass the CRC-16,
/// so sync-like patterns inside frame data are skipped. Bytes before the first header are
/// skipped; the last frame runs to the end of `data`.
pub fn frames(data: &[u8]) -> impl Iterator<Item = &[u8]> {
    Frames { data, pos: 0 }
}

struct Frames<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Iterator for Frames<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<&'a [u8]> {
        let d = self.data;
        let header_at = |i: usize| frame::is_sync(&d[i..]) && frame::parse_header(&d[i..]).is_ok();
        let start = (self.pos..d.len()).find(|&i| header_at(i))?;
        let mut crc = 0u16;
        let mut j = start;
        while j < d.len() {
            if j >= start + 2 && crc == 0 && header_at(j) {
                self.pos = j;
                return Some(&d[start..j]);
            }
            crc = frame::crc16_byte(crc, d[j]);
            j += 1;
        }
        self.pos = d.len();
        Some(&d[start..])
    }
}

/// The samples per channel of the frame starting at `frame` (from its header), for timing
/// a native FLAC stream's frames without decoding them.
pub fn frame_block_size(frame: &[u8]) -> Option<usize> {
    frame::parse_header(frame).ok().map(|h| h.block)
}
