//! Specular's own MPEG audio decoder for Layers I, II and III (ISO/IEC 11172-3, and the lower
//! sampling rates of ISO/IEC 13818-3): the sound of MPEG-1/2 programme streams, DVDs, DVB and
//! HDV transport streams, VCDs, and MP3 in AVI, MP4, Matroska and Flash files.
//!
//! - MPEG-1 (32, 44.1, 48 kHz), MPEG-2 (16, 22.05, 24 kHz) and MPEG-2.5 (8, 11.025, 12 kHz).
//! - Mono, stereo, dual channel and joint stereo (intensity; middle/side for Layer III);
//!   CRC-protected frames; Layer III's bit reservoir, block switching and mixed blocks.
//! - Free-format bit rates: the frame length is the distance to the next header, found once
//!   and kept for the stream.
//!
//! Input is the byte stream in any pieces; frames are found by their headers (a frame must
//! be followed by another frame header, or be the last). Output is interleaved `f32`.
#![deny(unsafe_code)]
#![allow(clippy::needless_range_loop)]

mod huffman_data;
mod layer12;
mod layer3;
mod synth;
mod window;

/// Why a stream can't be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Unsupported(String),
    Malformed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Malformed(s) => write!(f, "damaged MPEG audio: {s}"),
        }
    }
}

impl std::error::Error for Error {}

/// MPEG audio version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Version {
    Mpeg1,
    Mpeg2,
    Mpeg25,
}

/// A frame header (§2.4.2.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub version: Version,
    /// 1, 2 or 3.
    pub layer: u8,
    pub crc: bool,
    /// kbit/s; 0 for free format.
    pub bitrate: u32,
    pub sample_rate: u32,
    pub padding: bool,
    /// 0 stereo, 1 joint stereo, 2 dual channel, 3 mono.
    pub mode: u8,
    pub mode_extension: u8,
    /// Free format: the frame's length in bytes without padding, once known (else 0).
    pub free_len: usize,
}

impl Header {
    /// Parses four bytes; `None` unless they are a valid header.
    pub fn parse(b: &[u8]) -> Option<Header> {
        let h = u32::from_be_bytes(b.get(..4)?.try_into().ok()?);
        if h >> 21 != 0x7FF {
            return None;
        }
        let version = match (h >> 19) & 3 {
            3 => Version::Mpeg1,
            2 => Version::Mpeg2,
            0 => Version::Mpeg25,
            _ => return None,
        };
        let layer = match (h >> 17) & 3 {
            3 => 1,
            2 => 2,
            1 => 3,
            _ => return None,
        };
        let br_index = ((h >> 12) & 15) as usize;
        let sr_index = ((h >> 10) & 3) as usize;
        if br_index == 15 || sr_index == 3 {
            return None;
        }
        const BR: [[u16; 15]; 5] = [
            [
                0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448,
            ],
            [
                0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384,
            ],
            [
                0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
            ],
            [
                0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256,
            ],
            [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160],
        ];
        let row = match (version, layer) {
            (Version::Mpeg1, l) => usize::from(l) - 1,
            (_, 1) => 3,
            _ => 4,
        };
        let base = [44100, 48000, 32000][sr_index];
        let sample_rate = match version {
            Version::Mpeg1 => base,
            Version::Mpeg2 => base / 2,
            Version::Mpeg25 => base / 4,
        };
        Some(Header {
            version,
            layer,
            crc: (h >> 16) & 1 == 0,
            bitrate: u32::from(BR[row][br_index]),
            sample_rate,
            padding: (h >> 9) & 1 != 0,
            mode: ((h >> 6) & 3) as u8,
            mode_extension: ((h >> 4) & 3) as u8,
            free_len: 0,
        })
    }

    pub fn channels(&self) -> usize {
        if self.mode == 3 { 1 } else { 2 }
    }

    /// Samples per channel in a frame.
    pub fn samples(&self) -> usize {
        match (self.layer, self.version) {
            (1, _) => 384,
            (3, Version::Mpeg2 | Version::Mpeg25) => 576,
            _ => 1152,
        }
    }

    /// The frame's length in bytes (`None` for free format until [`Header::free_len`] is
    /// known).
    pub fn frame_len(&self) -> Option<usize> {
        if self.bitrate == 0 {
            let pad = if self.layer == 1 { 4 } else { 1 };
            return (self.free_len > 0).then(|| self.free_len + pad * usize::from(self.padding));
        }
        let br = self.bitrate as usize * 1000;
        let fs = self.sample_rate as usize;
        Some(match self.layer {
            1 => (12 * br / fs + usize::from(self.padding)) * 4,
            3 if self.version != Version::Mpeg1 => 72 * br / fs + usize::from(self.padding),
            _ => 144 * br / fs + usize::from(self.padding),
        })
    }

    /// The bit rate in kbit/s, worked out from the frame length for free format (0 while
    /// that is unknown).
    pub fn kbps(&self) -> u32 {
        if self.bitrate > 0 {
            return self.bitrate;
        }
        let slots_per_bit = match self.layer {
            1 => 48,
            3 if self.version != Version::Mpeg1 => 72,
            _ => 144,
        };
        (self.free_len as u64 * u64::from(self.sample_rate) / slots_per_bit / 1000) as u32
    }

    /// Whether `n` can be the next frame of the stream this header starts.
    fn continues(&self, n: &Header) -> bool {
        same_stream(self, n) && (n.bitrate == 0) == (self.bitrate == 0)
    }

    /// "MP1", "MP2", "MP3".
    pub fn layer_name(&self) -> &'static str {
        match self.layer {
            1 => "MP1",
            2 => "MP2",
            _ => "MP3",
        }
    }
}

/// The longest free-format frame looked for: 640 kbit/s Layer III at 32 kHz is 2880 bytes;
/// Layer I and II may go somewhat higher.
const MAX_FREE: usize = 4096;

/// A free-format frame's length without padding: the distance from the header at the start
/// of `data` to the next one of the stream, confirmed by the header after that (or the end
/// of `data`). `None` when no such header is in `data`.
pub fn free_len(data: &[u8], h: &Header) -> Option<usize> {
    let pad = |h: &Header| usize::from(h.padding) * if h.layer == 1 { 4 } else { 1 };
    let parse = |at: usize| {
        data.get(at..)
            .and_then(Header::parse)
            .filter(|n| h.continues(n))
    };
    (21..MAX_FREE.min(data.len().saturating_sub(3)))
        .filter(|&i| data[i] == 0xFF)
        .find_map(|i| {
            let len = i.checked_sub(pad(h)).filter(|&l| l >= 21)?;
            let n = parse(i)?;
            let next = i + len + pad(&n);
            (next + 4 > data.len() || parse(next).is_some()).then_some(len)
        })
}

/// The first frame header in `data` that another header follows (or that ends the data);
/// a free-format one has its [`Header::free_len`].
pub fn probe(data: &[u8]) -> Option<Header> {
    let mut i = 0;
    while i + 4 <= data.len() {
        if data[i] == 0xFF
            && let Some(mut h) = Header::parse(&data[i..])
            && (h.bitrate > 0 || {
                h.free_len = free_len(&data[i..], &h).unwrap_or(0);
                h.free_len > 0
            })
            && let Some(len) = h.frame_len()
        {
            let next = i + len;
            if next + 4 > data.len()
                || Header::parse(&data[next..]).is_some_and(|n| h.continues(&n))
            {
                return Some(h);
            }
        }
        i += 1;
    }
    None
}

fn same_stream(a: &Header, b: &Header) -> bool {
    a.version == b.version && a.layer == b.layer && a.sample_rate == b.sample_rate
}

/// An MPEG audio decoder.
pub struct Decoder {
    buf: Vec<u8>,
    /// Layer III's bit reservoir and IMDCT overlap.
    l3: Box<layer3::State>,
    /// The stream's first header (later frames must agree in version, layer and rate).
    first: Option<Header>,
    synth: [synth::Synth; 2],
    /// Frames decoded since the last reset.
    frames: u64,
    /// Output channels when fixed (a stream switching between mono and stereo keeps them).
    channels: Option<usize>,
    /// A free-format stream's frame length without padding, once found.
    free: Option<usize>,
}

impl Default for Decoder {
    fn default() -> Decoder {
        Decoder::new()
    }
}

impl Decoder {
    pub fn new() -> Decoder {
        Decoder {
            buf: Vec::new(),
            l3: Box::new(layer3::State::new()),
            first: None,
            synth: [synth::Synth::new(), synth::Synth::new()],
            frames: 0,
            channels: None,
            free: None,
        }
    }

    /// A decoder whose output has `channels` channels (1 or 2) whatever each frame's mode:
    /// mono frames go to both sides, stereo frames are averaged for mono. Without it the
    /// first frame's channels are kept.
    pub fn with_channels(channels: usize) -> Decoder {
        Decoder {
            channels: Some(channels.clamp(1, 2)),
            ..Decoder::new()
        }
    }

    /// The header of the stream (after the first frame).
    pub fn header(&self) -> Option<Header> {
        self.first
    }

    /// Takes a piece of the stream and appends every frame it completes to `out`
    /// (interleaved, the stream's channel count). Damaged frames are skipped.
    pub fn decode(&mut self, data: &[u8], out: &mut Vec<f32>) -> Result<(), Error> {
        self.buf.extend_from_slice(data);
        self.frames_in(out, false)
    }

    /// The end of the stream: a last frame without a header after it is decoded too.
    pub fn flush(&mut self, out: &mut Vec<f32>) -> Result<(), Error> {
        self.frames_in(out, true)
    }

    /// Forgets buffered input and the filter bank's history (after a seek).
    pub fn reset(&mut self) {
        self.buf.clear();
        self.synth = [synth::Synth::new(), synth::Synth::new()];
        *self.l3 = layer3::State::new();
    }

    fn frames_in(&mut self, out: &mut Vec<f32>, last: bool) -> Result<(), Error> {
        let mut at = 0;
        let mut result = Ok(());
        while at + 4 <= self.buf.len() {
            let Some(mut h) = Header::parse(&self.buf[at..])
                .filter(|h| self.first.is_none_or(|f| f.continues(h)))
            else {
                at += 1;
                continue;
            };
            if h.bitrate == 0 {
                match self.free.or_else(|| free_len(&self.buf[at..], &h)) {
                    Some(f) => {
                        self.free = Some(f);
                        h.free_len = f;
                    }
                    // The next header isn't in yet.
                    None if !last && self.buf.len() - at < MAX_FREE + 8 => break,
                    // The stream's only frame: all that is left.
                    None if last && self.buf.len() - at > 21 + 4 => {
                        let pad = if h.layer == 1 { 4 } else { 1 };
                        h.free_len = self.buf.len() - at - pad * usize::from(h.padding);
                    }
                    None => {
                        at += 1;
                        continue;
                    }
                }
            }
            let Some(len) = h.frame_len() else {
                result = Err(Error::Unsupported("free-format MPEG audio".into()));
                break;
            };
            let mut cut = false;
            if at + len > self.buf.len() {
                if !last || self.first.is_none() {
                    if last {
                        at = self.buf.len();
                    }
                    break;
                }
                // The stream's last frame, cut short: decoded with the missing bytes as
                // zeros (its spectrum is usually all there; the tail is padding).
                self.buf.resize(at + len, 0);
                cut = true;
            }
            // Before the first frame is trusted, the next one must confirm it.
            if self.first.is_none() && !last {
                match self.buf.get(at + len..at + len + 4) {
                    None => break,
                    Some(n) if Header::parse(n).is_none_or(|n| !h.continues(&n)) => {
                        at += 1;
                        continue;
                    }
                    _ => {}
                }
            }
            let frame = &self.buf[at..at + len];
            let start = out.len();
            let want = self
                .channels
                .unwrap_or_else(|| self.first.map_or(h.channels(), |f| f.channels()));
            let decoded = if h.layer == 3 {
                layer3::decode_frame(&h, frame, &mut self.l3, &mut self.synth, want, out)
            } else {
                layer12::decode_frame(&h, frame, &mut self.synth, out).map(|()| {
                    if h.channels() != want {
                        let pcm = out.split_off(start);
                        respread(&pcm, h.channels(), want, out);
                    }
                })
            };
            match decoded {
                Ok(()) => {
                    self.first.get_or_insert(h);
                    self.frames += 1;
                }
                Err(_) => {
                    // A damaged frame: silence in its place keeps the timing.
                    out.truncate(start);
                    out.resize(start + h.samples() * want, 0.0);
                }
            }
            at += len;
            if cut {
                break;
            }
        }
        self.buf.drain(..at.min(self.buf.len()));
        result
    }
}

/// Interleaved frames of `from` channels as `to` (mono to both sides, stereo averaged).
fn respread(pcm: &[f32], from: usize, to: usize, out: &mut Vec<f32>) {
    for f in pcm.chunks_exact(from.max(1)) {
        match (from, to) {
            (1, _) => out.extend(std::iter::repeat_n(f[0], to)),
            (_, 1) => out.push((f[0] + f[1]) * 0.5),
            _ => out.extend_from_slice(&f[..to.min(f.len())]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers() {
        // MPEG-1 Layer II, 192 kbit/s, 48 kHz, stereo.
        let h = Header::parse(&[0xFF, 0xFD, 0xA4, 0x00]).unwrap();
        assert_eq!(
            (h.version, h.layer, h.bitrate, h.sample_rate),
            (Version::Mpeg1, 2, 192, 48000)
        );
        assert_eq!(h.frame_len(), Some(576));
        assert!(!h.crc);
        assert_eq!(h.channels(), 2);
        assert!(Header::parse(&[0xFF, 0xFD, 0xF4, 0x00]).is_none());
        assert!(Header::parse(&[0xFF, 0xF9, 0xA4, 0x00]).is_none());
    }

    #[test]
    fn junk_never_panics() {
        let mut d = Decoder::new();
        let mut out = Vec::new();
        let mut seed = 99u32;
        let mut junk: Vec<u8> = (0..50_000)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 24) as u8
            })
            .collect();
        for k in 0..40 {
            junk[k * 1000..k * 1000 + 4].copy_from_slice(&[0xFF, 0xFD, 0xA4, 0x00]);
        }
        for c in junk.chunks(333) {
            let _ = d.decode(c, &mut out);
        }
        let _ = d.flush(&mut out);
        assert!(out.iter().all(|v| v.is_finite()));
    }
}
