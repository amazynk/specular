//! Specular's own DTS Coherent Acoustics core decoder, written from ETSI TS 102 114 V1.6.1
//! (clause 5 and Annexes C and D; a free download from etsi.org). The core DTS patents
//! (US 5,974,380, 5,978,762 and 6,487,535, priority 1995-12-01) expired on 2016-05-02.
//!
//! Decodes the core of DTS and DTS-HD streams: up to 5.1 channels at 8 to 48 kHz, all
//! subframe layouts, Huffman, block and plain coded subband samples, joint intensity,
//! sum/difference, both synthesis filter banks and the LFE channel. Raw 16-bit and 14-bit
//! streams in either byte order (as on DTS CDs and in WAVE files) are read as well as the
//! big-endian frames of MP4, Matroska, MPEG-TS and DVD; extension substreams (DTS-HD) and
//! the core's extensions (XCh, X96, XXCh) are skipped.
//!
//! Refused with [`Error::Unsupported`]: frames that use ADPCM prediction or high-frequency
//! VQ, whose two code books the specification leaves out, and channel arrangements beyond
//! 5.1 (AMODE 10 and up). Dynamic range coefficients are not applied.

#![deny(unsafe_code)]
#![allow(clippy::needless_range_loop)]

mod bits;
mod core;
mod tables;

pub use crate::core::FrameInfo;

/// The core sync word (16-bit big-endian form).
pub(crate) const SYNC_CORE: u32 = 0x7FFE_8001;
/// The extension substream sync word.
const SYNC_EXSS: u32 = 0x6458_2025;

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
            Error::Malformed(s) => write!(f, "damaged DTS: {s}"),
        }
    }
}

impl std::error::Error for Error {}

/// How the frames are packed in the bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Packing {
    /// 16-bit words, big-endian (MP4, Matroska, transport streams, DVD).
    Be16,
    /// 16-bit words, little-endian.
    Le16,
    /// 14 bits in each big-endian 16-bit word (DTS CDs, WAVE files).
    Be14,
    /// 14 bits in each little-endian 16-bit word.
    Le14,
}

/// The packing of a sync word at the start of `b`, if one is there.
fn packing_at(b: &[u8]) -> Option<Packing> {
    match b {
        [0x7F, 0xFE, 0x80, 0x01, ..] => Some(Packing::Be16),
        [0xFE, 0x7F, 0x01, 0x80, ..] => Some(Packing::Le16),
        [0x1F, 0xFF, 0xE8, 0x00, 0x07, x, ..] if x & 0xF0 == 0xF0 => Some(Packing::Be14),
        [0xFF, 0x1F, 0x00, 0xE8, x, 0x07, ..] if x & 0xF0 == 0xF0 => Some(Packing::Le14),
        _ => None,
    }
}

/// The first core sync word in `b`: its offset and packing.
fn find_sync(b: &[u8]) -> Option<(usize, Packing)> {
    (0..b.len().saturating_sub(5)).find_map(|i| packing_at(&b[i..]).map(|p| (i, p)))
}

/// Rewrites `raw` (whole 16-bit words of `p`) as 16-bit big-endian bytes onto `out`;
/// returns the raw bytes used. 14-bit words leave up to 13 bits for the next call in
/// `acc`/`nacc`.
fn normalize(p: Packing, raw: &[u8], out: &mut Vec<u8>, acc: &mut u32, nacc: &mut u32) -> usize {
    let n = raw.len() & !1;
    match p {
        Packing::Be16 => out.extend_from_slice(&raw[..n]),
        Packing::Le16 => {
            for w in raw[..n].chunks_exact(2) {
                out.extend_from_slice(&[w[1], w[0]]);
            }
        }
        Packing::Be14 | Packing::Le14 => {
            for w in raw[..n].chunks_exact(2) {
                let v = if p == Packing::Be14 {
                    u16::from_be_bytes([w[0], w[1]])
                } else {
                    u16::from_le_bytes([w[0], w[1]])
                };
                *acc = (*acc << 14) | u32::from(v & 0x3FFF);
                *nacc += 14;
                while *nacc >= 8 {
                    *nacc -= 8;
                    out.push((*acc >> *nacc) as u8);
                }
                *acc &= (1 << *nacc) - 1;
            }
        }
    }
    n
}

/// What the first frames of a stream say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamInfo {
    pub first: FrameInfo,
    pub packing: Packing,
}

/// The stream's first core frame header, from its first bytes.
pub fn probe(data: &[u8]) -> Option<StreamInfo> {
    let mut at = 0;
    while let Some((i, p)) = find_sync(&data[at..]) {
        let mut b = Vec::new();
        let (mut acc, mut nacc) = (0, 0);
        let end = (at + i + 64).min(data.len());
        normalize(p, &data[at + i..end], &mut b, &mut acc, &mut nacc);
        if let Some(first) = FrameInfo::parse(&b) {
            return Some(StreamInfo { first, packing: p });
        }
        at += i + 1;
    }
    None
}

/// How the PCM is made.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Options {
    /// Fold to two channels (L + 0.71 C + 0.71 surrounds, scaled to stay in range; LFE
    /// left out; mono to both sides at −3 dB).
    pub stereo: bool,
    /// Decode frames that use ADPCM prediction or high-frequency VQ without them (those
    /// subbands silent, no prediction) instead of refusing them. Wrong sound: only for
    /// checking the rest of the decoder against an oracle.
    pub partial: bool,
}

/// A DTS core decoder fed with bytes cut anywhere.
pub struct Decoder {
    opts: Options,
    packing: Option<Packing>,
    raw: Vec<u8>,
    buf: Vec<u8>,
    acc: u32,
    nacc: u32,
    state: core::State,
    last: Option<FrameInfo>,
    pcm: Vec<f32>,
}

impl Decoder {
    pub fn new(opts: Options) -> Decoder {
        Decoder {
            opts,
            packing: None,
            raw: Vec::new(),
            buf: Vec::new(),
            acc: 0,
            nacc: 0,
            state: core::State::default(),
            last: None,
            pcm: Vec::new(),
        }
    }

    /// The last frame decoded.
    pub fn info(&self) -> Option<FrameInfo> {
        self.last
    }

    /// Channels of the output: 2 when folding, else those of the last frame.
    pub fn channels(&self) -> usize {
        if self.opts.stereo {
            2
        } else {
            self.last.map_or(2, |f| f.channels)
        }
    }

    /// Forgets the buffered bytes and the filter banks' history (after a seek).
    pub fn reset(&mut self) {
        let opts = self.opts;
        let packing = self.packing;
        *self = Decoder::new(opts);
        self.packing = packing;
    }

    /// Decodes the frames completed by `data`, appending interleaved samples to `out`. A
    /// damaged frame is skipped (its length in silence, when its header is readable) and
    /// reported after the rest of `data` is decoded.
    pub fn decode(&mut self, data: &[u8], out: &mut Vec<f32>) -> Result<(), Error> {
        self.raw.extend_from_slice(data);
        let p = match self.packing {
            Some(p) => p,
            None => match find_sync(&self.raw) {
                Some((i, p)) => {
                    self.raw.drain(..i);
                    self.packing = Some(p);
                    p
                }
                None => {
                    let keep = self.raw.len().min(5);
                    self.raw.drain(..self.raw.len() - keep);
                    return Ok(());
                }
            },
        };
        let used = normalize(p, &self.raw, &mut self.buf, &mut self.acc, &mut self.nacc);
        self.raw.drain(..used);
        let mut first_err = None;
        let mut at = 0usize;
        loop {
            // Find a core frame (skipping extension substreams by their size).
            let Some(rel) = self.buf[at..].windows(4).position(|w| {
                let v = u32::from_be_bytes([w[0], w[1], w[2], w[3]]);
                v == SYNC_CORE || v == SYNC_EXSS
            }) else {
                at = self.buf.len().saturating_sub(3);
                break;
            };
            at += rel;
            if self.buf.len() - at < 16 {
                break;
            }
            let sync = u32::from_be_bytes([
                self.buf[at],
                self.buf[at + 1],
                self.buf[at + 2],
                self.buf[at + 3],
            ]);
            if sync == SYNC_EXSS {
                match exss_size(&self.buf[at..]) {
                    Some(n) if at + n <= self.buf.len() => at += n,
                    Some(_) => break,
                    None => at += 1,
                }
                continue;
            }
            let Some(info) = FrameInfo::parse(&self.buf[at..]) else {
                at += 1;
                continue;
            };
            if at + info.size > self.buf.len() {
                break;
            }
            self.pcm.clear();
            match core::decode(
                &self.buf[at..at + info.size],
                &info,
                &mut self.state,
                &mut self.pcm,
                self.opts.partial,
            ) {
                Ok(()) => {}
                Err(e @ Error::Unsupported(_)) => {
                    self.buf.drain(..at + info.size);
                    return Err(e);
                }
                Err(e) => {
                    // Its length in silence keeps the sound in time.
                    self.pcm.clear();
                    self.pcm.resize(info.samples * info.channels, 0.0);
                    first_err.get_or_insert(e);
                }
            }
            self.last = Some(info);
            self.emit(&info, out);
            at += info.size;
        }
        self.buf.drain(..at.min(self.buf.len()));
        first_err.map_or(Ok(()), Err)
    }

    /// Appends the frame in `self.pcm` to `out`, folded when asked.
    fn emit(&self, info: &FrameInfo, out: &mut Vec<f32>) {
        let ch = info.channels;
        if !self.opts.stereo {
            out.extend_from_slice(&self.pcm);
            return;
        }
        let lfe = info.lfe > 0;
        let h = std::f32::consts::FRAC_1_SQRT_2;
        // (gain to L, gain to R) for each output channel in WAVE order.
        let mut g: Vec<(f32, f32)> = match info.amode {
            0 => vec![(h, h)],
            1..=4 => vec![(1.0, 0.0), (0.0, 1.0)],
            5 => vec![(1.0, 0.0), (0.0, 1.0), (h, h)],
            6 => vec![(1.0, 0.0), (0.0, 1.0), (0.5, 0.5)],
            7 => vec![(1.0, 0.0), (0.0, 1.0), (h, h), (0.5, 0.5)],
            8 => vec![(1.0, 0.0), (0.0, 1.0), (h, 0.0), (0.0, h)],
            _ => vec![(1.0, 0.0), (0.0, 1.0), (h, h), (h, 0.0), (0.0, h)],
        };
        if lfe {
            let at = if matches!(info.amode, 5 | 7 | 9) {
                3
            } else {
                2.min(g.len())
            };
            g.insert(at, (0.0, 0.0));
        }
        let norm = 1.0 / g.iter().map(|x| x.0).sum::<f32>().max(1.0);
        for f in self.pcm.chunks_exact(ch) {
            let (mut l, mut r) = (0.0, 0.0);
            for (v, (a, b)) in f.iter().zip(&g) {
                l += v * a;
                r += v * b;
            }
            out.push(l * norm);
            out.push(r * norm);
        }
    }
}

/// Bytes of the extension substream at the start of `b` (Table 7-2).
fn exss_size(b: &[u8]) -> Option<usize> {
    let mut r = bits::Bits::new(b.get(..12)?);
    r.u(32);
    r.u(8);
    r.u(2);
    let long = r.u(1) != 0;
    let (hb, fb) = if long { (12, 20) } else { (8, 16) };
    r.u(hb);
    Some(r.u(fb) as usize + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct BitWriter(Vec<bool>);

    impl BitWriter {
        fn put(&mut self, n: u32, v: u32) {
            for i in (0..n).rev() {
                self.0.push(v >> i & 1 != 0);
            }
        }
        fn bytes(&self) -> Vec<u8> {
            self.0
                .chunks(8)
                .map(|c| {
                    c.iter()
                        .enumerate()
                        .fold(0u8, |a, (i, &b)| a | (u8::from(b) << (7 - i)))
                })
                .collect()
        }
    }

    /// A core header: 5.1 with LFE at 48 kHz, 512 samples, 1024 bytes.
    fn header() -> Vec<u8> {
        let mut w = BitWriter::default();
        w.put(32, SYNC_CORE);
        w.put(1, 1); // FTYPE
        w.put(5, 31); // SHORT
        w.put(1, 0); // CPF
        w.put(7, 15); // NBLKS
        w.put(14, 1023); // FSIZE
        w.put(6, 9); // AMODE
        w.put(4, 13); // SFREQ
        w.put(5, 15); // RATE
        w.put(10, 0); // flags
        w.put(2, 2); // LFF
        w.put(1, 0); // HFLAG
        w.put(24, 0); // the rest
        w.put(16, 0);
        w.put(32, 0);
        w.bytes()
    }

    #[test]
    fn header_fields() {
        let f = FrameInfo::parse(&header()).unwrap();
        assert_eq!((f.size, f.samples, f.sample_rate), (1024, 512, 48000));
        assert_eq!((f.amode, f.lfe, f.channels), (9, 2, 6));
    }

    /// The same header found in every packing.
    #[test]
    fn packings() {
        let be = header();
        let le: Vec<u8> = be.chunks(2).flat_map(|w| [w[1], w[0]]).collect();
        // 14 bits a word: the header's bits regrouped, sign-extended into the top two.
        let bits: Vec<bool> = be
            .iter()
            .flat_map(|&b| (0..8).rev().map(move |i| b >> i & 1 != 0))
            .collect();
        let mut be14 = Vec::new();
        for c in bits.chunks(14) {
            let mut v = c.iter().fold(0u16, |a, &b| a << 1 | u16::from(b)) << (14 - c.len());
            if v & 0x2000 != 0 {
                v |= 0xC000;
            }
            be14.extend_from_slice(&v.to_be_bytes());
        }
        let le14: Vec<u8> = be14.chunks(2).flat_map(|w| [w[1], w[0]]).collect();
        for (bytes, p) in [
            (&be, Packing::Be16),
            (&le, Packing::Le16),
            (&be14, Packing::Be14),
            (&le14, Packing::Le14),
        ] {
            let mut data = vec![0u8; 6];
            data.extend_from_slice(bytes);
            let s = probe(&data).unwrap_or_else(|| panic!("{p:?}: {:02x?}", &data[..14]));
            assert_eq!(s.packing, p);
            assert_eq!(s.first.channels, 6);
        }
    }

    #[test]
    fn exss_size_short_and_long() {
        let mut w = BitWriter::default();
        w.put(32, SYNC_EXSS);
        w.put(8, 0);
        w.put(2, 0);
        w.put(1, 0);
        w.put(8, 15);
        w.put(16, 999);
        w.put(32, 0);
        assert_eq!(exss_size(&w.bytes()), Some(1000));
        let mut w = BitWriter::default();
        w.put(32, SYNC_EXSS);
        w.put(8, 0);
        w.put(2, 0);
        w.put(1, 1);
        w.put(12, 15);
        w.put(20, 99_999);
        w.put(32, 0);
        assert_eq!(exss_size(&w.bytes()), Some(100_000));
    }

    /// Bytes without a frame give nothing and no error; a damaged frame its length in
    /// silence.
    #[test]
    fn garbage_and_damage() {
        // (zeros would ask for high-frequency VQ: left out)
        let mut d = Decoder::new(Options {
            partial: true,
            ..Options::default()
        });
        let mut out = Vec::new();
        d.decode(&[0x55; 5000], &mut out).unwrap();
        assert!(out.is_empty());
        let mut frame = header();
        frame.resize(1024, 0);
        frame.extend_from_slice(&header());
        assert!(d.decode(&frame, &mut out).is_err());
        assert_eq!(out.len(), 512 * 6);
    }
}
