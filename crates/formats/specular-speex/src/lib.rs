//! Specular's own Speex decoder: narrowband (8 kHz), wideband (16 kHz) and ultra-wideband
//! (32 kHz) CELP, every sub-mode, the decoder's perceptual enhancement (on, as libspeex has
//! it by default), the high-pass output filter and intensity stereo from in-band messages.
//! Output is `f32` in ±1, interleaved for stereo.
//!
//! The arithmetic follows libspeex's floating-point build, the reference every Speex stream
//! was made against; the codebooks and filter tables are libspeex's data (BSD, see
//! `PROVENANCE.md`), the decoding is this crate's own.
//!
//! Not supported: packet loss concealment beyond silence, and DTX comfort noise beyond the
//! sub-mode 0 noise (lost frames decode as silence of the frame's length).
#![deny(unsafe_code)]
#![allow(clippy::needless_range_loop)]

mod nb;
mod sb;
#[rustfmt::skip]
mod tables;

/// Why a stream or packet can't be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Unsupported(String),
    Malformed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported(s) => write!(f, "{s} can't be played yet"),
            Error::Malformed(s) => write!(f, "damaged Speex audio: {s}"),
        }
    }
}

impl std::error::Error for Error {}

/// Reads bits most significant first, as libspeex packs them.
pub(crate) struct Bits<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Bits<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Bits<'a> {
        Bits { data, at: 0 }
    }

    pub(crate) fn remaining(&self) -> isize {
        (self.data.len() * 8) as isize - self.at as isize
    }

    /// `n` bits; zeros past the end (libspeex's overflow).
    pub(crate) fn read(&mut self, n: u32) -> u32 {
        let mut v = 0u32;
        for _ in 0..n {
            let bit = self
                .data
                .get(self.at / 8)
                .map_or(0, |b| (b >> (7 - self.at % 8)) & 1);
            v = (v << 1) | u32::from(bit);
            self.at += 1;
        }
        v
    }

    pub(crate) fn peek(&self) -> u32 {
        self.data
            .get(self.at / 8)
            .map_or(0, |b| u32::from((b >> (7 - self.at % 8)) & 1))
    }

    pub(crate) fn skip(&mut self, n: usize) {
        self.at += n;
    }
}

/// The Ogg Speex header (the 80-byte first packet).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub rate: u32,
    /// 0 narrowband, 1 wideband, 2 ultra-wideband.
    pub mode: u8,
    pub channels: u8,
    pub frames_per_packet: u32,
    /// Header packets after the comment packet.
    pub extra_headers: u32,
}

impl Header {
    pub fn parse(b: &[u8]) -> Result<Header, Error> {
        if b.len() < 80 || &b[..8] != b"Speex   " {
            return Err(Error::Malformed("the header is missing".into()));
        }
        let le = |at: usize| u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]);
        let mode = le(40);
        if mode > 2 {
            return Err(Error::Unsupported(format!("Speex mode {mode}")));
        }
        Ok(Header {
            rate: le(36),
            mode: mode as u8,
            channels: le(48).clamp(1, 2) as u8,
            frames_per_packet: le(64).max(1),
            extra_headers: le(68).min(16),
        })
    }
}

/// The intensity stereo state (libspeex's in-band stereo messages).
struct Stereo {
    balance: f32,
    e_ratio: f32,
    smooth_left: f32,
    smooth_right: f32,
}

impl Stereo {
    fn new() -> Stereo {
        Stereo {
            balance: 1.0,
            e_ratio: 0.5,
            smooth_left: 1.0,
            smooth_right: 1.0,
        }
    }

    /// Reads the message after its 4-bit id 9.
    fn read(&mut self, b: &mut Bits) {
        let sign = if b.read(1) != 0 { -1.0f32 } else { 1.0 };
        let dexp = b.read(5) as f32;
        self.balance = (sign * 0.25 * dexp).exp();
        self.e_ratio = [0.25, 0.315, 0.397, 0.5][b.read(2) as usize];
    }

    /// One mono frame as two channels.
    fn spread(&mut self, mono: &[f32], out: &mut Vec<f32>) {
        let e_right = 1.0 / (self.e_ratio * (1.0 + self.balance)).sqrt();
        let e_left = self.balance.sqrt() * e_right;
        let at = out.len();
        out.resize(at + 2 * mono.len(), 0.0);
        // libspeex walks the frame backwards (it spreads in place).
        for i in (0..mono.len()).rev() {
            self.smooth_left = self.smooth_left * 0.98 + e_left * 0.02;
            self.smooth_right = self.smooth_right * 0.98 + e_right * 0.02;
            out[at + 2 * i] = self.smooth_left * mono[i];
            out[at + 2 * i + 1] = self.smooth_right * mono[i];
        }
    }
}

/// What a frame's decode found.
pub(crate) enum Frame {
    Decoded,
    /// No more frames in the packet (a terminator, or too few bits left).
    End,
}

/// The decoder for one band layout.
enum Core {
    Nb(Box<nb::Nb>),
    Sb(Box<sb::Sb>),
}

impl Core {
    fn new(mode: u8) -> Core {
        match mode {
            0 => Core::Nb(Box::new(nb::Nb::new(false))),
            1 => Core::Sb(Box::new(sb::Sb::wideband())),
            _ => Core::Sb(Box::new(sb::Sb::ultra())),
        }
    }

    fn frame_size(&self) -> usize {
        match self {
            Core::Nb(_) => nb::FRAME,
            Core::Sb(s) => s.full_frame(),
        }
    }

    fn frame(
        &mut self,
        b: &mut Bits,
        stereo: &mut Stereo,
        out: &mut [f32],
    ) -> Result<Frame, Error> {
        match self {
            Core::Nb(n) => n.decode(b, stereo, out, None),
            Core::Sb(s) => s.decode(b, stereo, out),
        }
    }
}

/// A Speex stream's decoder.
pub struct Decoder {
    core: Core,
    mode: u8,
    channels: usize,
    stereo: Stereo,
    frame: Vec<f32>,
}

impl Decoder {
    /// `mode`: 0 narrowband, 1 wideband, 2 ultra-wideband; `channels` 1 or 2.
    pub fn new(mode: u8, channels: u8) -> Decoder {
        let core = Core::new(mode.min(2));
        let n = core.frame_size();
        Decoder {
            core,
            mode: mode.min(2),
            channels: usize::from(channels.clamp(1, 2)),
            stereo: Stereo::new(),
            frame: vec![0.0; n],
        }
    }

    /// Samples a channel in a frame (160, 320 or 640).
    pub fn frame_size(&self) -> usize {
        self.core.frame_size()
    }

    pub fn sample_rate(&self) -> u32 {
        [8000, 16000, 32000][usize::from(self.mode)]
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Decodes a packet's frames (all of them when `frames` is None, else at most that many)
    /// and appends them; returns the frames decoded. A frame that fails ends the packet.
    pub fn decode(
        &mut self,
        packet: &[u8],
        frames: Option<usize>,
        out: &mut Vec<f32>,
    ) -> Result<usize, Error> {
        let mut b = Bits::new(packet);
        let mut n = 0;
        while frames.is_none_or(|f| n < f) {
            match self.core.frame(&mut b, &mut self.stereo, &mut self.frame)? {
                Frame::End => break,
                Frame::Decoded => {}
            }
            n += 1;
            let scale = 1.0 / 32768.0;
            if self.channels == 2 {
                let mono: Vec<f32> = self.frame.iter().map(|v| v * scale).collect();
                self.stereo.spread(&mono, out);
            } else {
                out.extend(self.frame.iter().map(|v| v * scale));
            }
        }
        Ok(n)
    }

    /// Forgets the decoder's state (after a seek).
    pub fn reset(&mut self) {
        self.core = Core::new(self.mode);
        self.stereo = Stereo::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header() {
        let mut h = b"Speex   ".to_vec();
        h.resize(80, 0);
        h[36..40].copy_from_slice(&16000u32.to_le_bytes());
        h[40..44].copy_from_slice(&1u32.to_le_bytes());
        h[48..52].copy_from_slice(&1u32.to_le_bytes());
        h[64..68].copy_from_slice(&2u32.to_le_bytes());
        let p = Header::parse(&h).unwrap();
        assert_eq!(
            (p.rate, p.mode, p.channels, p.frames_per_packet),
            (16000, 1, 1, 2)
        );
        h[40] = 3;
        assert!(Header::parse(&h).is_err());
    }

    #[test]
    fn junk_never_panics() {
        for mode in 0..3 {
            let mut d = Decoder::new(mode, 2);
            let mut seed = 7u32;
            let mut out = Vec::new();
            for _ in 0..200 {
                let p: Vec<u8> = (0..70)
                    .map(|_| {
                        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                        (seed >> 24) as u8
                    })
                    .collect();
                let _ = d.decode(&p, None, &mut out);
            }
            assert!(out.iter().all(|v| v.is_finite()));
        }
    }
}
