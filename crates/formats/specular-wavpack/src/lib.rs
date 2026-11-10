//! Specular's own WavPack decoder, written from the format's published description
//! (WavPack 5 File Format, David Bryant, BSD-licensed project): no code or tables copied.
//!
//! - Version 4 and 5 blocks (stream versions 0x402–0x410): lossless and hybrid (lossy, or
//!   lossless with the `.wvc` correction file), 8–32-bit integers, 32-bit float, mono,
//!   stereo, joint stereo and any number of channels (stereo/mono block pairs), extra
//!   decorrelation passes, noise shaping, bitrate balancing and the extended (`wvx`) bits of
//!   32-bit integer and float data.
//! - Every block's checksum is checked; a block that fails is an error (the caller puts
//!   silence in its place).
//! - DSD (1-bit) streams (WavPack 5): stored, "fast" and "high" blocks, decoded to DSD
//!   bytes bit-exact, and turned into PCM for playback by [`DsdToPcm`] (see [`dsd`]).
//! - Refused by name: WavPack 3 and older files.

#![deny(unsafe_code)]

pub mod dsd;
mod words;

pub use dsd::{DsdToPcm, FilterSpec};

use words::{Bits, Mode, Words, exp2s, restore_weight};

/// Why a stream or block can't be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Unsupported(String),
    Malformed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported(s) => write!(f, "unsupported WavPack: {s}"),
            Error::Malformed(s) => write!(f, "damaged WavPack: {s}"),
        }
    }
}

impl std::error::Error for Error {}

fn bad(s: &str) -> Error {
    Error::Malformed(s.into())
}

// Header flags.
const BYTES_STORED: u32 = 3;
const MONO_FLAG: u32 = 4;
const HYBRID_FLAG: u32 = 8;
const JOINT_STEREO: u32 = 0x10;
const CROSS_DECORR: u32 = 0x20;
const HYBRID_SHAPE: u32 = 0x40;
const FLOAT_DATA: u32 = 0x80;
const INT32_DATA: u32 = 0x100;
const HYBRID_BITRATE: u32 = 0x200;
const HYBRID_BALANCE: u32 = 0x400;
const INITIAL_BLOCK: u32 = 0x800;
const FINAL_BLOCK: u32 = 0x1000;
const SHIFT_LSB: u32 = 13;
const SRATE_LSB: u32 = 23;
const NEW_SHAPING: u32 = 0x2000_0000;
const FALSE_STEREO: u32 = 0x4000_0000;
const DSD_FLAG: u32 = 0x8000_0000;
const MONO_DATA: u32 = MONO_FLAG | FALSE_STEREO;

// Float flags.
const FLOAT_SHIFT_ONES: u8 = 1;
const FLOAT_SHIFT_SAME: u8 = 2;
const FLOAT_SHIFT_SENT: u8 = 4;
const FLOAT_ZEROS_SENT: u8 = 8;
const FLOAT_NEG_ZEROS: u8 = 0x10;

const SAMPLE_RATES: [u32; 15] = [
    6000, 8000, 9600, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000, 64000, 88200, 96000,
    192000,
];

/// The most samples a block may hold (the encoder writes at most 131072 a channel; more
/// would only be a damaged header asking for memory).
pub const MAX_BLOCK_SAMPLES: u32 = 1 << 18;
/// The most channels.
pub const MAX_CHANNELS: usize = 64;

/// A block's 32-byte header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// The whole block's length in bytes.
    pub size: usize,
    pub version: u16,
    /// Samples in the whole file (a channel), when the encoder knew.
    pub total_samples: Option<u64>,
    pub block_index: u64,
    pub block_samples: u32,
    pub flags: u32,
    pub crc: u32,
}

impl Header {
    /// Parses a header at the start of `b`.
    pub fn parse(b: &[u8]) -> Option<Header> {
        if b.len() < 32 || &b[..4] != b"wvpk" {
            return None;
        }
        let u32le = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        let ck = u32le(4) as usize;
        if !(24..=(1 << 24)).contains(&ck) || ck & 1 != 0 {
            return None;
        }
        let total = u32le(12);
        Some(Header {
            size: ck + 8,
            version: u16::from_le_bytes([b[8], b[9]]),
            total_samples: (total != u32::MAX)
                .then(|| u64::from(total) + (u64::from(b[11]) << 32) - u64::from(b[11])),
            block_index: u64::from(u32le(16)) + (u64::from(b[10]) << 32),
            block_samples: u32le(20),
            flags: u32le(24),
            crc: u32le(28),
        })
    }

    /// Whether this block's data is one channel (mono, or "false stereo": mono played twice).
    fn mono_data(&self) -> bool {
        self.flags & MONO_DATA != 0
    }
    /// Channels this block fills (1 or 2).
    fn channels(&self) -> usize {
        if self.flags & MONO_FLAG != 0 { 1 } else { 2 }
    }
}

/// A frame: the blocks of all channels for one stretch of samples, contiguous in the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub offset: usize,
    pub len: usize,
    /// The first sample's index (from the file's first block).
    pub index: u64,
    pub samples: u32,
}

/// Where the next block header starts at or after `at` (`None` at the end).
fn next_block(d: &[u8], mut at: usize) -> Option<(usize, Header)> {
    while at + 32 <= d.len() {
        at += memchr_wvpk(&d[at..])?;
        if let Some(h) = Header::parse(&d[at..])
            && at + h.size <= d.len()
        {
            return Some((at, h));
        }
        at += 1;
    }
    None
}

fn memchr_wvpk(d: &[u8]) -> Option<usize> {
    d.windows(4).position(|w| w == b"wvpk")
}

/// The file's frames in order (metadata-only blocks are skipped). Stops at `max` frames.
/// For DSD streams `index` and `samples` count PCM samples after decimation
/// ([`DsdToPcm`]): a frame of `n` bytes from byte `i` holds `⌊(i + n) / d⌋ − ⌊i / d⌋`.
pub fn frames(d: &[u8], max: usize) -> Vec<Frame> {
    let mut out = Vec::new();
    let mut at = 0;
    let mut first: Option<u64> = None;
    // DSD bytes a PCM sample (1 for PCM streams).
    let mut dec: Option<u64> = None;
    while out.len() < max {
        let Some((start, h)) = next_block(d, at) else {
            break;
        };
        at = start + h.size;
        if h.block_samples == 0 || h.flags & INITIAL_BLOCK == 0 {
            continue;
        }
        // Gather the blocks up to the final one of the same stretch.
        let mut end = at;
        let mut last = h;
        while last.flags & FINAL_BLOCK == 0 {
            match Header::parse(&d[end.min(d.len())..]) {
                Some(n)
                    if end + n.size <= d.len()
                        && n.block_index == h.block_index
                        && n.flags & INITIAL_BLOCK == 0 =>
                {
                    end += n.size;
                    last = n;
                }
                _ => break,
            }
        }
        at = end;
        let base = *first.get_or_insert(h.block_index);
        let dec = *dec.get_or_insert_with(|| {
            (h.flags & DSD_FLAG != 0)
                .then(|| dsd_byte_rate(&h, &d[start..start + h.size]))
                .flatten()
                .map_or(1, |r| u64::from(FilterSpec::new(r).decimation))
        });
        let i = h.block_index.saturating_sub(base);
        let e = i + u64::from(h.block_samples);
        out.push(Frame {
            offset: start,
            len: end - start,
            index: i / dec,
            samples: (e / dec - i / dec) as u32,
        });
    }
    out
}

/// What a WavPack file holds, from its first frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub channels: usize,
    /// The WAVE channel mask (0 when not given).
    pub channel_mask: u32,
    pub sample_rate: u32,
    /// Bytes a sample is stored in (1–4); integer output spans ±2^(8·bytes − 1).
    pub bytes: u8,
    /// Significant bits (bytes·8 less the shifted-out low bits).
    pub bits: u8,
    pub float: bool,
    /// Hybrid (lossy unless the correction file is used).
    pub hybrid: bool,
    pub total_samples: Option<u64>,
    pub version: u16,
    /// For DSD streams: the 1-bit stream's facts. `sample_rate` and `total_samples` are
    /// then those of the PCM made by [`DsdToPcm`], and `float` is set (it gives `f32`).
    pub dsd: Option<DsdInfo>,
}

/// A DSD stream's own rate and length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DsdInfo {
    /// One-bit samples a second a channel (2 822 400 for DSD64).
    pub rate: u32,
    /// One-bit samples a channel, when the encoder knew.
    pub samples: Option<u64>,
    /// How it is brought down to PCM.
    pub filter: FilterSpec,
}

/// A DSD block's byte rate a channel: the header's rate times the block's multiplier.
fn dsd_byte_rate(h: &Header, blk: &[u8]) -> Option<u32> {
    let mut rate = SAMPLE_RATES
        .get(((h.flags >> SRATE_LSB) & 0xf) as usize)
        .copied();
    let mut mult = None;
    for (id, data) in metadata(blk.get(32..)?) {
        match id {
            ID_SAMPLE_RATE => rate = sample_rate(data).or(rate),
            ID_DSD_BLOCK => mult = dsd::multiplier_log2(data),
            _ => {}
        }
    }
    rate?.checked_mul(1u32.checked_shl(mult?)?)
}

fn sample_rate(data: &[u8]) -> Option<u32> {
    if data.len() < 3 {
        return None;
    }
    let mut r = u32::from(data[0]) | u32::from(data[1]) << 8 | u32::from(data[2]) << 16;
    if data.len() == 4 {
        r |= u32::from(data[3] & 0x7f) << 24;
    }
    Some(r)
}

/// Reads the facts of the stream from its first frame.
pub fn info(d: &[u8]) -> Result<Info, Error> {
    if d.len() >= 12 && &d[..4] == b"RIFF" && &d[8..12] == b"WVPK" {
        return Err(Error::Unsupported("WavPack 3 and older files".into()));
    }
    let f = frames(d, 1)
        .into_iter()
        .next()
        .ok_or_else(|| bad("no audio blocks"))?;
    let frame = &d[f.offset..f.offset + f.len];
    let h = Header::parse(frame).ok_or_else(|| bad("block header"))?;
    check_version(h.version)?;
    let mut channels = 0usize;
    let mut mask = 0u32;
    let mut rate = None;
    let (mut wv, mut wvc) = (false, false);
    for (id, data) in metadata(&frame[32..h.size.min(frame.len())]) {
        match id {
            ID_WV_BITSTREAM => wv = true,
            ID_WVC_BITSTREAM => wvc = true,
            ID_CHANNEL_INFO => {
                if let Some((c, m)) = channel_info(data) {
                    channels = c;
                    mask = m;
                }
            }
            ID_SAMPLE_RATE if data.len() >= 3 => rate = sample_rate(data),
            _ => {}
        }
    }
    if wvc && !wv {
        return Err(Error::Unsupported(
            "a correction file (.wvc) only: open the .wv file beside it".into(),
        ));
    }
    if channels == 0 {
        // Without channel info: the block's own channels (a pair, or mono).
        channels = h.channels();
        mask = if channels == 1 { 4 } else { 3 };
    }
    if channels > MAX_CHANNELS {
        return Err(Error::Unsupported(format!("{channels} channels")));
    }
    let idx = ((h.flags >> SRATE_LSB) & 0xf) as usize;
    let sample_rate = rate
        .or_else(|| SAMPLE_RATES.get(idx).copied())
        .ok_or_else(|| bad("no sampling rate"))?;
    if h.flags & DSD_FLAG != 0 {
        let bytes =
            dsd_byte_rate(&h, &frame[..h.size.min(frame.len())]).ok_or_else(|| bad("DSD rate"))?;
        if bytes > dsd::MAX_DSD_RATE / 8 {
            return Err(Error::Unsupported(format!(
                "DSD at {} MHz (DSD512 is the most)",
                f64::from(bytes) * 8.0 / 1e6
            )));
        }
        let filter = FilterSpec::new(bytes);
        let dec = u64::from(filter.decimation);
        return Ok(Info {
            channels,
            channel_mask: mask,
            sample_rate: filter.pcm_rate,
            bytes: 4,
            bits: 32,
            float: true,
            hybrid: false,
            total_samples: h.total_samples.map(|n| n / dec),
            version: h.version,
            dsd: Some(DsdInfo {
                rate: bytes * 8,
                samples: h.total_samples.map(|n| n * 8),
                filter,
            }),
        });
    }
    let bytes = (h.flags & BYTES_STORED) as u8 + 1;
    let shift = ((h.flags >> SHIFT_LSB) & 0x1f) as u8;
    Ok(Info {
        channels,
        channel_mask: mask,
        sample_rate,
        bytes,
        bits: (bytes * 8).saturating_sub(shift),
        float: h.flags & FLOAT_DATA != 0,
        hybrid: h.flags & HYBRID_FLAG != 0,
        total_samples: h.total_samples,
        version: h.version,
        dsd: None,
    })
}

fn check_version(v: u16) -> Result<(), Error> {
    if v < 0x402 {
        return Err(Error::Unsupported("WavPack 3 and older files".into()));
    }
    if v > 0x410 {
        return Err(Error::Unsupported(format!("stream version {v:#x}")));
    }
    Ok(())
}

fn channel_info(d: &[u8]) -> Option<(usize, u32)> {
    if d.is_empty() || d.len() > 7 {
        return None;
    }
    if d.len() >= 6 {
        let ch = (usize::from(d[0]) | usize::from(d[2] & 0xf) << 8) + 1;
        let mut mask = u32::from(d[3]) | u32::from(d[4]) << 8 | u32::from(d[5]) << 16;
        if d.len() == 7 {
            mask |= u32::from(d[6]) << 24;
        }
        Some((ch, mask))
    } else {
        let mut mask = 0u32;
        for (i, &b) in d[1..].iter().enumerate() {
            mask |= u32::from(b) << (8 * i);
        }
        Some((usize::from(d[0]), mask))
    }
}

// Metadata ids.
const ID_DECORR_TERMS: u8 = 2;
const ID_DECORR_WEIGHTS: u8 = 3;
const ID_DECORR_SAMPLES: u8 = 4;
const ID_ENTROPY_VARS: u8 = 5;
const ID_HYBRID_PROFILE: u8 = 6;
const ID_SHAPING_WEIGHTS: u8 = 7;
const ID_FLOAT_INFO: u8 = 8;
const ID_INT32_INFO: u8 = 9;
const ID_WV_BITSTREAM: u8 = 0xa;
const ID_WVC_BITSTREAM: u8 = 0xb;
const ID_WVX_BITSTREAM: u8 = 0xc;
const ID_CHANNEL_INFO: u8 = 0xd;
const ID_DSD_BLOCK: u8 = 0xe;
const ID_OPTIONAL: u8 = 0x20;
const ID_SAMPLE_RATE: u8 = 0x27;
const ID_WVX_NEW_BITSTREAM: u8 = 0x2c;

/// A block body's metadata sub-blocks: (id without the size bits, data).
fn metadata(mut b: &[u8]) -> impl Iterator<Item = (u8, &[u8])> {
    std::iter::from_fn(move || {
        if b.len() < 2 {
            return None;
        }
        let id = b[0];
        let (words, head) = if id & 0x80 != 0 {
            if b.len() < 4 {
                return None;
            }
            (
                usize::from(b[1]) | usize::from(b[2]) << 8 | usize::from(b[3]) << 16,
                4,
            )
        } else {
            (usize::from(b[1]), 2)
        };
        let len = words * 2;
        let body = b.get(head..head + len)?;
        let data = if id & 0x40 != 0 && len > 0 {
            &body[..len - 1]
        } else {
            body
        };
        b = &b[head + len..];
        Some((id & 0x3f, data))
    })
}

#[derive(Clone, Copy, Default)]
struct Pass {
    term: i32,
    delta: i32,
    wa: i32,
    wb: i32,
    sa: [i32; 8],
    sb: [i32; 8],
}

#[inline]
fn apply(w: i32, s: i32) -> i32 {
    ((i64::from(w) * i64::from(s) + 512) >> 10) as i32
}

#[inline]
fn update(w: &mut i32, delta: i32, src: i32, res: i32) {
    if src != 0 && res != 0 {
        if (src ^ res) < 0 {
            *w = w.wrapping_sub(delta);
        } else {
            *w = w.wrapping_add(delta);
        }
    }
}

#[inline]
fn update_clip(w: &mut i32, delta: i32, src: i32, res: i32) {
    if src != 0 && res != 0 {
        if (src ^ res) < 0 {
            *w = (*w - delta).max(-1024);
        } else {
            *w = (*w + delta).min(1024);
        }
    }
}

/// The prediction of term 17/18 from the last two samples.
#[inline]
fn extrapolate(term: i32, s: &[i32; 8]) -> i32 {
    if term & 1 != 0 {
        s[0].wrapping_mul(2).wrapping_sub(s[1])
    } else {
        s[0].wrapping_mul(3).wrapping_sub(s[1]) >> 1
    }
}

/// One block's decoding state, from its metadata.
#[derive(Default)]
struct Block<'a> {
    passes: Vec<Pass>,
    words: Words,
    shaping_acc: [i32; 2],
    shaping_delta: [i32; 2],
    error: [i32; 2],
    float_flags: u8,
    float_shift: u8,
    float_max_exp: u8,
    float_norm_exp: u8,
    int32: [u8; 4],
    wv: Option<&'a [u8]>,
    wvc: Option<&'a [u8]>,
    wvx: Option<&'a [u8]>,
    wvx_new: bool,
    crc_wvx: u32,
}

fn le16(d: &[u8], at: usize) -> Option<i32> {
    Some(i32::from(i16::from_le_bytes([
        *d.get(at)?,
        *d.get(at + 1)?,
    ])))
}

impl<'a> Block<'a> {
    fn read(&mut self, h: &Header, body: &'a [u8]) -> Result<(), Error> {
        let mono = h.mono_data();
        let n = if mono { 1 } else { 2 };
        for (id, d) in metadata(body) {
            match id {
                0 | 1 => {}
                ID_DECORR_TERMS => {
                    if d.len() > 16 {
                        return Err(bad("too many decorrelation terms"));
                    }
                    self.passes = d
                        .iter()
                        .rev()
                        .map(|&b| Pass {
                            term: i32::from(b & 0x1f) - 5,
                            delta: i32::from((b >> 5) & 7),
                            ..Pass::default()
                        })
                        .collect();
                    for p in &self.passes {
                        let t = p.term;
                        if t == 0 || t < -3 || (8 < t && t < 17) || t > 18 || (mono && t < 0) {
                            return Err(bad("decorrelation term"));
                        }
                    }
                }
                ID_DECORR_WEIGHTS => {
                    let cnt = d.len() / n;
                    if cnt > self.passes.len() {
                        return Err(bad("decorrelation weights"));
                    }
                    let mut it = d.iter();
                    for p in self.passes.iter_mut().rev().take(cnt) {
                        p.wa = restore_weight(*it.next().unwrap_or(&0) as i8);
                        if !mono {
                            p.wb = restore_weight(*it.next().unwrap_or(&0) as i8);
                        }
                    }
                }
                ID_DECORR_SAMPLES => self.decorr_samples(h, d)?,
                ID_ENTROPY_VARS => {
                    if d.len() != 6 * n {
                        return Err(bad("entropy variables"));
                    }
                    for c in 0..n {
                        for k in 0..3 {
                            let v = u16::from_le_bytes([d[c * 6 + k * 2], d[c * 6 + k * 2 + 1]]);
                            self.words.c[c].median[k] = exp2s(i32::from(v)) as u32;
                        }
                    }
                }
                ID_HYBRID_PROFILE => self.hybrid_profile(h, d)?,
                ID_SHAPING_WEIGHTS => {
                    if d.len() == 2 {
                        self.shaping_acc[0] = restore_weight(d[0] as i8) << 16;
                        self.shaping_acc[1] = restore_weight(d[1] as i8) << 16;
                    } else if d.len() >= 4 * n {
                        for c in 0..n {
                            self.error[c] = exp2s(le16(d, 4 * c).unwrap_or(0));
                            self.shaping_acc[c] = exp2s(le16(d, 4 * c + 2).unwrap_or(0));
                        }
                        if d.len() == 6 * n {
                            for c in 0..n {
                                self.shaping_delta[c] = exp2s(le16(d, 4 * n + 2 * c).unwrap_or(0));
                            }
                        }
                    } else {
                        return Err(bad("shaping weights"));
                    }
                }
                ID_FLOAT_INFO => {
                    let [f, s, m, e] = <[u8; 4]>::try_from(d).map_err(|_| bad("float info"))?;
                    self.float_flags = f;
                    self.float_shift = s;
                    self.float_max_exp = m;
                    self.float_norm_exp = e;
                }
                ID_INT32_INFO => {
                    self.int32 = <[u8; 4]>::try_from(d).map_err(|_| bad("int32 info"))?;
                }
                ID_WV_BITSTREAM => self.wv = Some(d),
                ID_WVC_BITSTREAM => self.wvc = Some(d),
                ID_WVX_BITSTREAM | ID_WVX_NEW_BITSTREAM => {
                    if d.len() <= 4 {
                        return Err(bad("extended bits"));
                    }
                    self.crc_wvx = u32::from_le_bytes([d[0], d[1], d[2], d[3]]);
                    self.wvx = Some(&d[4..]);
                    self.wvx_new = id == ID_WVX_NEW_BITSTREAM;
                }
                ID_CHANNEL_INFO => {}
                id if id & ID_OPTIONAL != 0 => {}
                id => return Err(bad(&format!("unknown metadata {id:#x}"))),
            }
        }
        Ok(())
    }

    fn decorr_samples(&mut self, h: &Header, d: &[u8]) -> Result<(), Error> {
        let mono = h.mono_data();
        let n = if mono { 1 } else { 2 };
        let mut at = 0;
        if h.version == 0x402 && h.flags & HYBRID_FLAG != 0 {
            for c in 0..n {
                self.error[c] = exp2s(le16(d, at).ok_or_else(|| bad("decorr samples"))?);
                at += 2;
            }
        }
        let next = |at: &mut usize| -> Result<i32, Error> {
            let v = le16(d, *at).ok_or_else(|| bad("decorr samples"))?;
            *at += 2;
            Ok(exp2s(v))
        };
        for p in self.passes.iter_mut().rev() {
            if at >= d.len() {
                break;
            }
            if p.term > 8 {
                p.sa[0] = next(&mut at)?;
                p.sa[1] = next(&mut at)?;
                if !mono {
                    p.sb[0] = next(&mut at)?;
                    p.sb[1] = next(&mut at)?;
                }
            } else if p.term < 0 {
                p.sa[0] = next(&mut at)?;
                p.sb[0] = next(&mut at)?;
            } else {
                for m in 0..p.term as usize {
                    p.sa[m] = next(&mut at)?;
                    if !mono {
                        p.sb[m] = next(&mut at)?;
                    }
                }
            }
        }
        if at != d.len() {
            return Err(bad("decorr samples"));
        }
        Ok(())
    }

    fn hybrid_profile(&mut self, h: &Header, d: &[u8]) -> Result<(), Error> {
        let n = if h.mono_data() { 1 } else { 2 };
        let mut at = 0;
        let get = |at: &mut usize| -> Result<u16, Error> {
            let v = d.get(*at..*at + 2).ok_or_else(|| bad("hybrid profile"))?;
            *at += 2;
            Ok(u16::from_le_bytes([v[0], v[1]]))
        };
        if h.flags & HYBRID_BITRATE != 0 {
            for c in 0..n {
                self.words.c[c].slow_level = exp2s(i32::from(get(&mut at)?)) as u32;
            }
        }
        for c in 0..n {
            self.words.bitrate_acc[c] = u32::from(get(&mut at)?) << 16;
        }
        if at < d.len() {
            for c in 0..n {
                self.words.bitrate_delta[c] = exp2s(i32::from(get(&mut at)? as i16)) as u32;
            }
            if at < d.len() {
                return Err(bad("hybrid profile"));
            }
        }
        Ok(())
    }
}

/// The noise shaping filter of a hybrid block decoded with its correction: the output
/// from the reconstructed value `lossless` and the lossy one.
#[inline]
fn shape(b: &mut Block, flags: u32, c: usize, lossless: i32, lossy: i32) -> i32 {
    let corr = lossless.wrapping_sub(lossy);
    b.shaping_acc[c] = b.shaping_acc[c].wrapping_add(b.shaping_delta[c]);
    let w = b.shaping_acc[c] >> 16;
    let mut temp = apply(w, b.error[c]).wrapping_neg();
    if flags & NEW_SHAPING != 0 && w < 0 && temp != 0 {
        if temp == b.error[c] {
            temp = if temp < 0 { temp + 1 } else { temp - 1 };
        }
        b.error[c] = temp.wrapping_sub(corr);
    } else {
        b.error[c] = corr.wrapping_neg();
    }
    lossless.wrapping_sub(temp)
}

/// Decodes one block's `n` samples (1 or 2 channels interleaved) into `out`; `wvc` is the
/// matching correction block. Returns the block's channels (2 for false stereo).
fn decode_block(blk: &[u8], wvc: Option<&[u8]>, out: &mut Vec<i32>) -> Result<usize, Error> {
    let h = Header::parse(blk).ok_or_else(|| bad("block header"))?;
    check_version(h.version)?;
    if h.flags & MONO_DATA == MONO_DATA {
        return Err(bad("block flags"));
    }
    if h.block_samples > MAX_BLOCK_SAMPLES {
        return Err(bad("block too long"));
    }
    let flags = h.flags;
    let body = blk.get(32..h.size).ok_or_else(|| bad("block cut short"))?;
    if flags & DSD_FLAG != 0 {
        return dsd_block(&h, body, out);
    }
    let mut b = Block::default();
    b.read(&h, body)?;
    let mut crc_want = h.crc;
    let mut wvc_h = None;
    if let Some(c) = wvc
        && let Some(ch) = Header::parse(c)
        && ch.block_index == h.block_index
        && ch.block_samples == h.block_samples
        && h.flags & HYBRID_FLAG != 0
    {
        b.read(
            &h,
            c.get(32..ch.size)
                .ok_or_else(|| bad("correction block cut short"))?,
        )?;
        crc_want = ch.crc;
        wvc_h = Some(ch);
    }
    let corrected = wvc_h.is_some() && b.wvc.is_some();
    let wv = b.wv.ok_or_else(|| bad("no audio bits"))?;
    let mut bs = Bits::new(wv);
    let mut cbs = if corrected {
        b.wvc.map(Bits::new)
    } else {
        None
    };
    let mode = Mode {
        hybrid: flags & HYBRID_FLAG != 0,
        bitrate: flags & HYBRID_BITRATE != 0,
        balance: flags & HYBRID_BALANCE != 0,
        mono: h.mono_data(),
    };
    let n = h.block_samples as usize;
    let start = out.len();
    let mono = h.mono_data();
    out.reserve(if mono { n } else { 2 * n } * if flags & FALSE_STEREO != 0 { 2 } else { 1 });
    let mut crc: u32 = 0xffff_ffff;
    // A sample beyond the block's magnitude mutes the block (the reference decoder's rule).
    let mut limit = (1u64 << ((flags >> 18) & 0x1f)) + 2;
    if flags & HYBRID_FLAG != 0 && !corrected {
        limit = limit * 2 + 128;
    }
    let mut muted = false;
    let mut m = 0usize;
    let mut passes = std::mem::take(&mut b.passes);
    let eof = |_| bad("audio bits cut short");
    if mono {
        for _ in 0..n {
            let (mut w, corr) = b.words.word(mode, 0, &mut bs, cbs.as_mut()).map_err(eof)?;
            for p in passes.iter_mut() {
                let (sam, k);
                if p.term > 8 {
                    sam = extrapolate(p.term, &p.sa);
                    p.sa[1] = p.sa[0];
                    k = 0;
                } else {
                    sam = p.sa[m];
                    k = (m + p.term as usize) & 7;
                }
                let t = apply(p.wa, sam).wrapping_add(w);
                update(&mut p.wa, p.delta, sam, w);
                p.sa[k] = t;
                w = t;
            }
            m = (m + 1) & 7;
            if corrected {
                let lossless = w.wrapping_add(corr);
                w = if flags & HYBRID_SHAPE != 0 {
                    shape(&mut b, flags, 0, lossless, w)
                } else {
                    lossless
                };
                crc = crc.wrapping_mul(3).wrapping_add(w as u32);
            } else {
                crc = crc.wrapping_mul(3).wrapping_add(w as u32);
            }
            muted |= u64::from(w.unsigned_abs()) > limit;
            out.push(w);
        }
    } else {
        let cross = corrected && flags & CROSS_DECORR != 0;
        for i in 0..n {
            let (mut l, c0) = b.words.word(mode, 0, &mut bs, cbs.as_mut()).map_err(eof)?;
            let (mut r, c1) = b.words.word(mode, 1, &mut bs, cbs.as_mut()).map_err(eof)?;
            let (mut lc, mut rc) = (0i32, 0i32);
            if cross {
                // The lossless value's prediction before the passes update their state.
                lc = l.wrapping_add(c0);
                rc = r.wrapping_add(c1);
                for p in passes.iter() {
                    if p.term > 0 {
                        let (sa, sb) = if p.term > 8 {
                            (extrapolate(p.term, &p.sa), extrapolate(p.term, &p.sb))
                        } else {
                            (p.sa[m], p.sb[m])
                        };
                        lc = lc.wrapping_add(apply(p.wa, sa));
                        rc = rc.wrapping_add(apply(p.wb, sb));
                    } else if p.term == -1 {
                        lc = lc.wrapping_add(apply(p.wa, p.sa[0]));
                        rc = rc.wrapping_add(apply(p.wb, lc));
                    } else {
                        rc = rc.wrapping_add(apply(p.wb, p.sb[0]));
                        if p.term == -3 {
                            lc = lc.wrapping_add(apply(p.wa, p.sa[0]));
                        } else {
                            lc = lc.wrapping_add(apply(p.wa, rc));
                        }
                    }
                }
                if flags & JOINT_STEREO != 0 {
                    rc = rc.wrapping_sub(lc >> 1);
                    lc = lc.wrapping_add(rc);
                }
            }
            for p in passes.iter_mut() {
                if p.term > 0 {
                    let (sa, sb, k);
                    if p.term > 8 {
                        sa = extrapolate(p.term, &p.sa);
                        sb = extrapolate(p.term, &p.sb);
                        p.sa[1] = p.sa[0];
                        p.sb[1] = p.sb[0];
                        k = 0;
                    } else {
                        sa = p.sa[m];
                        sb = p.sb[m];
                        k = (m + p.term as usize) & 7;
                    }
                    let l2 = apply(p.wa, sa).wrapping_add(l);
                    let r2 = apply(p.wb, sb).wrapping_add(r);
                    update(&mut p.wa, p.delta, sa, l);
                    update(&mut p.wb, p.delta, sb, r);
                    p.sa[k] = l2;
                    p.sb[k] = r2;
                    l = l2;
                    r = r2;
                } else if p.term == -1 {
                    let l2 = l.wrapping_add(apply(p.wa, p.sa[0]));
                    update_clip(&mut p.wa, p.delta, p.sa[0], l);
                    l = l2;
                    let r2 = r.wrapping_add(apply(p.wb, l2));
                    update_clip(&mut p.wb, p.delta, l2, r);
                    r = r2;
                    p.sa[0] = r2;
                } else {
                    let r2 = r.wrapping_add(apply(p.wb, p.sb[0]));
                    update_clip(&mut p.wb, p.delta, p.sb[0], r);
                    r = r2;
                    let src = if p.term == -3 {
                        let s = p.sa[0];
                        p.sa[0] = r;
                        s
                    } else {
                        r2
                    };
                    let l2 = l.wrapping_add(apply(p.wa, src));
                    update_clip(&mut p.wa, p.delta, src, l);
                    l = l2;
                    p.sb[0] = l2;
                }
            }
            m = (m + 1) & 7;
            if corrected {
                if !cross {
                    lc = l.wrapping_add(c0);
                    rc = r.wrapping_add(c1);
                    if flags & JOINT_STEREO != 0 {
                        rc = rc.wrapping_sub(lc >> 1);
                        lc = lc.wrapping_add(rc);
                    }
                }
                if flags & JOINT_STEREO != 0 {
                    r = r.wrapping_sub(l >> 1);
                    l = l.wrapping_add(r);
                }
                if flags & HYBRID_SHAPE != 0 {
                    l = shape(&mut b, flags, 0, lc, l);
                    r = shape(&mut b, flags, 1, rc, r);
                } else {
                    l = lc;
                    r = rc;
                }
            } else if flags & JOINT_STEREO != 0 {
                r = r.wrapping_sub(l >> 1);
                l = l.wrapping_add(r);
            }
            crc = crc
                .wrapping_mul(9)
                .wrapping_add((l as u32).wrapping_mul(3))
                .wrapping_add(r as u32);
            // Without a correction file only every eighth pair is looked at.
            if corrected || i % 8 == 0 {
                muted |= u64::from(l.unsigned_abs().max(r.unsigned_abs())) > limit;
            }
            out.push(l);
            out.push(r);
        }
    }
    b.passes = passes;
    if bs.overrun || cbs.as_ref().is_some_and(|c| c.overrun) {
        return Err(bad("audio bits cut short"));
    }
    if muted {
        return Err(bad("a sample beyond the block's range"));
    }
    if crc != crc_want {
        return Err(bad("block checksum"));
    }
    let lossy = flags & HYBRID_FLAG != 0 && !corrected;
    fixup(&mut b, &h, &mut out[start..], lossy)?;
    if flags & FALSE_STEREO != 0 {
        let mono: Vec<i32> = out.drain(start..).collect();
        for v in mono {
            out.push(v);
            out.push(v);
        }
        return Ok(2);
    }
    Ok(if mono { 1 } else { 2 })
}

/// A DSD block: its bytes, checked against the block checksum.
fn dsd_block(h: &Header, body: &[u8], out: &mut Vec<i32>) -> Result<usize, Error> {
    let data = metadata(body)
        .find(|&(id, _)| id == ID_DSD_BLOCK)
        .map(|(_, d)| d)
        .ok_or_else(|| bad("no DSD bits"))?;
    let start = out.len();
    let (ch, crc) = dsd::decode(h, data, out)?;
    if crc != h.crc {
        out.truncate(start);
        return Err(bad("block checksum"));
    }
    if h.flags & FALSE_STEREO != 0 {
        let mono: Vec<i32> = out.drain(start..).collect();
        for v in mono {
            out.push(v);
            out.push(v);
        }
        return Ok(2);
    }
    Ok(ch)
}

/// Restores shifted-out bits, 32-bit integers and floats.
fn fixup(b: &mut Block, h: &Header, v: &mut [i32], lossy: bool) -> Result<(), Error> {
    let flags = h.flags;
    let mut shift = ((flags >> SHIFT_LSB) & 0x1f) as i32;
    if flags & FLOAT_DATA != 0 {
        return floats(b, v);
    }
    if flags & INT32_DATA != 0 {
        let [sent, mut zeros, mut ones, mut dups] = b.int32.map(|x| i32::from(x & 0x1f));
        if let Some(x) = b.wvx {
            let mut xb = Bits::new(x);
            let max_width = if b.wvx_new { xb.bits(5) as i32 } else { 0 };
            let mut crc: u32 = 0xffff_ffff;
            for s in v.iter_mut() {
                if sent != 0 {
                    if max_width != 0 {
                        let pv = if *s < 0 { !*s } else { *s };
                        let width = (32 - (pv as u32).leading_zeros()) as i32 + sent;
                        let mut read = sent;
                        if width <= max_width || {
                            read -= width - max_width;
                            read > 0
                        } {
                            let data = xb.bits(read as u32);
                            *s = (((*s as u32) << read) | data).wrapping_shl((sent - read) as u32)
                                as i32;
                        } else {
                            *s = ((*s as u32) << sent) as i32;
                        }
                    } else {
                        let data = xb.bits(sent as u32);
                        *s = (((*s as u32) << sent) | data) as i32;
                    }
                }
                fill(s, zeros, ones, dups);
                crc = crc
                    .wrapping_mul(9)
                    .wrapping_add((*s as u32 & 0xffff).wrapping_mul(3))
                    .wrapping_add((*s as u32 >> 16) & 0xffff);
            }
            if xb.overrun || crc != b.crc_wvx {
                return Err(bad("extended bits checksum"));
            }
        } else if sent == 0 && zeros + ones + dups != 0 {
            while lossy && flags & BYTES_STORED == 3 && shift < 8 {
                if zeros != 0 {
                    zeros -= 1;
                } else if ones != 0 {
                    ones -= 1;
                } else if dups != 0 {
                    dups -= 1;
                } else {
                    break;
                }
                shift += 1;
            }
            for s in v.iter_mut() {
                fill(s, zeros, ones, dups);
            }
        } else {
            shift += zeros + sent + ones + dups;
        }
    }
    let shift = (shift & 0x1f) as u32;
    if lossy {
        let (min, max): (i32, i32) = match flags & BYTES_STORED {
            0 => (-128 >> shift, 127 >> shift),
            1 => (-32768 >> shift, 32767 >> shift),
            2 => (-8_388_608 >> shift, 8_388_607 >> shift),
            _ => (i32::MIN >> shift, i32::MAX >> shift),
        };
        for s in v.iter_mut() {
            *s = ((*s).clamp(min, max) as u32).wrapping_shl(shift) as i32;
        }
    } else if shift != 0 {
        for s in v.iter_mut() {
            *s = (*s as u32).wrapping_shl(shift) as i32;
        }
    }
    Ok(())
}

#[inline]
fn fill(s: &mut i32, zeros: i32, ones: i32, dups: i32) {
    if zeros != 0 {
        *s = (*s as u32).wrapping_shl(zeros as u32) as i32;
    } else if ones != 0 {
        *s = ((s.wrapping_add(1) as u32).wrapping_shl(ones as u32) as i32).wrapping_sub(1);
    } else if dups != 0 {
        let lsb = *s & 1;
        *s = ((s.wrapping_add(lsb) as u32).wrapping_shl(dups as u32) as i32).wrapping_sub(lsb);
    }
}

fn f32_bits(sign: u32, exp: u32, mant: u32) -> u32 {
    (sign << 31) | ((exp & 0xff) << 23) | (mant & 0x7f_ffff)
}

/// Integer values to float bits (left in place as `i32`).
fn floats(b: &mut Block, v: &mut [i32]) -> Result<(), Error> {
    let fshift = u32::from(b.float_shift & 0x1f);
    let max_exp = i32::from(b.float_max_exp);
    let ff = b.float_flags;
    let Some(x) = b.wvx else {
        for s in v.iter_mut() {
            if *s == 0 {
                continue;
            }
            let mut exp = max_exp;
            let mut val = ((*s as u32) << fshift) as i32;
            let mut sign = 0;
            if val < 0 {
                val = val.wrapping_neg();
                sign = 1;
            }
            if val >= 0x100_0000 {
                while val & 0xf00_0000 != 0 {
                    val >>= 1;
                    exp += 1;
                }
            } else if exp != 0 {
                let mut shifted = 0;
                while val & 0x80_0000 == 0 {
                    exp -= 1;
                    if exp == 0 {
                        break;
                    }
                    shifted += 1;
                    val <<= 1;
                }
                shifted &= 0x1f;
                if shifted != 0 && ff & FLOAT_SHIFT_ONES != 0 {
                    val |= (1 << shifted) - 1;
                }
            }
            *s = f32_bits(sign, exp as u32, val as u32) as i32;
        }
        return Ok(());
    };
    let mut xb = Bits::new(x);
    let (min_zeros, max_ones) = if b.wvx_new {
        (xb.bits(5) as i32, xb.bits(5) as i32)
    } else {
        (0, 0)
    };
    let mut crc: u32 = 0xffff_ffff;
    for s in v.iter_mut() {
        let mut exp = max_exp;
        let (mut sign, mut e, mut mant) = (0u32, 0u32, 0u32);
        if *s == 0 {
            if ff & FLOAT_ZEROS_SENT != 0 {
                if xb.bit() == 1 {
                    mant = xb.bits(23);
                    if exp >= 25 {
                        e = xb.bits(8);
                    }
                    sign = xb.bit();
                } else if ff & FLOAT_NEG_ZEROS != 0 {
                    sign = xb.bit();
                }
            }
        } else {
            let mut val = ((*s as u32) << fshift) as i32;
            if val < 0 {
                val = val.wrapping_neg();
                sign = 1;
            }
            if val == 0x100_0000 {
                if xb.bit() == 1 {
                    mant = xb.bits(23);
                }
                e = 255;
            } else {
                let mut shifted = 0i32;
                if exp != 0 {
                    while val & 0x80_0000 == 0 {
                        exp -= 1;
                        if exp == 0 {
                            break;
                        }
                        shifted += 1;
                        val = ((val as u32) << 1) as i32;
                    }
                }
                shifted &= 0x1f;
                if shifted != 0 {
                    if ff & FLOAT_SHIFT_ONES != 0 || (ff & FLOAT_SHIFT_SAME != 0 && xb.bit() == 1) {
                        val |= ((1u32 << shifted) - 1) as i32;
                    } else if ff & FLOAT_SHIFT_SENT != 0 {
                        let mask = ((1u32 << shifted) - 1) as i32;
                        let mut zeros = 0;
                        if max_ones != 0 && shifted > max_ones {
                            zeros = shifted - max_ones;
                        }
                        if min_zeros > zeros {
                            zeros = min_zeros.min(shifted);
                        }
                        let n = shifted - zeros;
                        if n > 0 {
                            let t = xb.bits(n as u32);
                            val |= ((t << zeros) as i32) & mask;
                        }
                    }
                }
                mant = val as u32;
                e = exp as u32;
            }
        }
        let bits = f32_bits(sign, e, mant);
        crc = crc
            .wrapping_mul(27)
            .wrapping_add((bits & 0x7f_ffff).wrapping_mul(9))
            .wrapping_add(((bits >> 23) & 0xff).wrapping_mul(3))
            .wrapping_add(bits >> 31);
        *s = bits as i32;
    }
    if xb.overrun || crc != b.crc_wvx {
        return Err(bad("extended bits checksum"));
    }
    Ok(())
}

/// Decodes a frame (all its blocks) into `out` as interleaved samples in the file's channel
/// order: integers (spanning ±2^(8·bytes − 1)) or, for float streams, the IEEE bits of each
/// `f32`; for DSD streams, the DSD bytes (0–255, oldest bit on top) for [`DsdToPcm`].
/// `wvc` is the matching frame of the correction file. A block that fails its checks is
/// silent in its channels (zeros, or the DSD idle pattern 0x55), as the reference decoder
/// mutes it, and counted.
pub fn decode_frame(
    frame: &[u8],
    wvc: Option<&[u8]>,
    channels: usize,
    out: &mut Vec<i32>,
) -> Result<Decoded, Error> {
    let first = Header::parse(frame).ok_or_else(|| bad("block header"))?;
    let n = first.block_samples as usize;
    if first.block_samples > MAX_BLOCK_SAMPLES || channels == 0 || channels > MAX_CHANNELS {
        return Err(bad("frame size"));
    }
    let base = out.len();
    let quiet = if first.flags & DSD_FLAG != 0 { 0x55 } else { 0 };
    out.resize(base + n * channels, quiet);
    let mut tmp = Vec::with_capacity(2 * n);
    let mut at = 0;
    let mut cat = 0;
    let mut ch = 0;
    let mut muted = 0;
    while ch < channels && at < frame.len() {
        let h = Header::parse(&frame[at..]).ok_or_else(|| bad("block header"))?;
        if h.block_samples != first.block_samples || at + h.size > frame.len() {
            return Err(bad("blocks of a frame disagree"));
        }
        let blk = &frame[at..at + h.size];
        at += h.size;
        let cblk = wvc.and_then(|c| {
            let ch = Header::parse(c.get(cat..)?)?;
            let b = c.get(cat..cat + ch.size)?;
            cat += ch.size;
            Some(b)
        });
        tmp.clear();
        let got = match decode_block(blk, cblk, &mut tmp) {
            Ok(g) => g,
            Err(e @ Error::Unsupported(_)) => return Err(e),
            Err(_) => {
                muted += 1;
                ch += h.channels();
                if h.flags & FINAL_BLOCK != 0 {
                    break;
                }
                continue;
            }
        };
        let take = h.channels().min(channels - ch);
        for i in 0..n {
            for k in 0..take {
                out[base + i * channels + ch + k] = tmp[i * got + k];
            }
        }
        ch += h.channels();
        if h.flags & FINAL_BLOCK != 0 {
            break;
        }
    }
    if ch < channels {
        // A frame cut short (the end of a cut file): its missing channels are silent.
        muted += 1;
    }
    Ok(Decoded { samples: n, muted })
}

/// What [`decode_frame`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decoded {
    /// Samples a channel.
    pub samples: usize,
    /// Blocks muted for failing their checks.
    pub muted: usize,
}

/// The scale that maps a decoded integer to −1..1, or `None` for float streams.
pub fn int_scale(info: &Info) -> Option<f32> {
    (!info.float).then(|| 1.0 / (1u64 << (8 * u32::from(info.bytes) - 1)) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers() {
        let mut b = vec![0u8; 32];
        b[..4].copy_from_slice(b"wvpk");
        b[4..8].copy_from_slice(&24u32.to_le_bytes());
        b[8..10].copy_from_slice(&0x410u16.to_le_bytes());
        b[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        b[20..24].copy_from_slice(&100u32.to_le_bytes());
        let h = Header::parse(&b).unwrap();
        assert_eq!(h.size, 32);
        assert_eq!(h.total_samples, None);
        assert_eq!(h.block_samples, 100);
        b[4..8].copy_from_slice(&25u32.to_le_bytes());
        assert!(Header::parse(&b).is_none(), "odd sizes are not blocks");
    }

    #[test]
    fn old_files_are_refused_by_name() {
        let mut d = b"RIFF\0\0\0\0WVPKfmt ".to_vec();
        d.resize(64, 0);
        assert_eq!(
            info(&d),
            Err(Error::Unsupported("WavPack 3 and older files".into()))
        );
    }

    #[test]
    fn weights() {
        assert_eq!(restore_weight(0), 0);
        assert_eq!(restore_weight(127), 1016 + 8);
        assert_eq!(restore_weight(-128), -1024);
        let mut w = 1020;
        update_clip(&mut w, 8, 1, 1);
        assert_eq!(w, 1024);
        update_clip(&mut w, 8, 1, -1);
        assert_eq!(w, 1016);
    }
}
