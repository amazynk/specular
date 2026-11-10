//! Specular's own Monkey's Audio (`.ape`) decoder. Monkey's Audio has no written
//! specification; this follows the behaviour of the reference SDK (3-clause BSD since
//! version 10.18), reimplemented: only the range coder's two 64-entry width tables are
//! copied, as data (see `tables.PROVENANCE.md`).
//!
//! - File versions 3950 and later (Monkey's Audio 3.95, 2002, onwards; the descriptor
//!   header from 3.98): every compression level (fast, normal, high, extra high, insane),
//!   8/16/24/32-bit integers and 32-bit float, mono, stereo and up to 32 channels.
//! - File versions 3800–3949 (Monkey's Audio 3.80–3.94, 2000–2002; see [`old`]): the normal
//!   and extra high levels, all four levels from 3.93; 8/16/24-bit mono and stereo.
//! - Each frame's CRC is checked (before 3.82 by default only a sum of magnitudes); a frame
//!   that fails is an error (the caller plays silence).
//! - Refused by name: files from versions before 3.80, and the fast and high levels of
//!   3.80–3.92 (no sample of either to check them against).

#![deny(unsafe_code)]

mod old;
mod predict;
mod range;
mod tables;

use predict::Predictor;
use range::{Reader, State};

/// Why a file or frame can't be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Unsupported(String),
    Malformed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported(s) => write!(f, "unsupported Monkey's Audio: {s}"),
            Error::Malformed(s) => write!(f, "damaged Monkey's Audio: {s}"),
        }
    }
}

impl std::error::Error for Error {}

fn bad(s: &str) -> Error {
    Error::Malformed(s.into())
}

const FLAG_8_BIT: u16 = 1;
const FLAG_CRC: u16 = 1 << 1;
const FLAG_HAS_PEAK_LEVEL: u16 = 1 << 2;
const FLAG_24_BIT: u16 = 1 << 3;
const FLAG_HAS_SEEK_ELEMENTS: u16 = 1 << 4;
const FLAG_CREATE_WAV_HEADER: u16 = 1 << 5;
const FLAG_FLOATING_POINT: u16 = 1 << 12;

/// The most channels the format allows.
pub const MAX_CHANNELS: u16 = 32;
/// The most blocks (samples a channel) in a frame accepted.
pub const MAX_FRAME_BLOCKS: u32 = 10_000_000;

/// One frame's place in the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    /// Where the frame's 32-bit-aligned data starts, and its bytes (with 4 to spare).
    pub offset: usize,
    pub len: usize,
    /// Bits before the frame's first bit, within its first word.
    pub skip: usize,
    /// Samples a channel.
    pub blocks: u32,
}

/// A Monkey's Audio file's facts and frames.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    /// The encoder's file version (3990 is current).
    pub version: u16,
    /// 1000 (fast) … 5000 (insane).
    pub level: u16,
    pub channels: u16,
    pub sample_rate: u32,
    pub bits: u16,
    pub float: bool,
    pub total_blocks: u64,
    pub frames: Vec<Frame>,
    /// Frames carry a CRC-32 of their samples (always from 3.82 on; before that only when
    /// the header's flag says so, else a sum of magnitudes).
    pub crc: bool,
}

impl Info {
    /// The compression level's name.
    pub fn level_name(&self) -> &'static str {
        match self.level {
            1000 => "fast",
            2000 => "normal",
            3000 => "high",
            4000 => "extra high",
            5000 => "insane",
            _ => "unknown",
        }
    }
}

fn u16le(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}
fn u32le(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

/// Where the descriptor starts: at 0, after an ID3v2 tag, or within the first 1 MB.
fn find_start(d: &[u8]) -> Option<usize> {
    let mut at = 0;
    if d.len() >= 10 && &d[..3] == b"ID3" {
        let size = d[6..10]
            .iter()
            .fold(0usize, |a, &b| (a << 7) | usize::from(b & 0x7f));
        at = 10 + size + if d[5] & 0x10 != 0 { 10 } else { 0 };
    }
    let end = d.len().min(at + (1 << 20));
    let win = d.get(at..end)?;
    let k = win.windows(4).position(|w| w == b"MAC " || w == b"MACF")?;
    Some(at + k)
}

/// The length of tags at the end of the file (APEv2 with or without header, ID3v1).
fn tag_bytes(d: &[u8]) -> usize {
    let mut end = d.len();
    if end >= 128 && &d[end - 128..end - 125] == b"TAG" {
        end -= 128;
    }
    if end >= 32 && &d[end - 32..end - 24] == b"APETAGEX" {
        let size = u32le(d, end - 20).unwrap_or(0) as usize;
        let header = u32le(d, end - 12).is_some_and(|f| f & 0x8000_0000 != 0);
        let total = size + if header { 32 } else { 0 };
        if total <= end {
            end -= total;
        }
    }
    d.len() - end
}

/// Reads the headers and seek table.
pub fn info(d: &[u8]) -> Result<Info, Error> {
    let start = find_start(d).ok_or_else(|| bad("no Monkey's Audio header"))?;
    let h = &d[start..];
    let version = u16le(h, 4).ok_or_else(|| bad("header cut short"))?;
    if version < 3800 {
        return Err(Error::Unsupported(format!(
            "files from versions before 3.80 (this one is {}.{:02})",
            version / 1000,
            (version % 1000) / 10
        )));
    }
    let short = || bad("header cut short");
    let (level, flags, bpf, final_blocks, total_frames, bits, channels, rate);
    let seek_at;
    let seek_count;
    let terminating;
    if version >= 3980 {
        let desc = u32le(h, 8).ok_or_else(short)? as usize;
        let head = u32le(h, 12).ok_or_else(short)? as usize;
        let seek_bytes = u32le(h, 16).ok_or_else(short)? as usize;
        terminating = u32le(h, 32).ok_or_else(short)? as usize;
        let a = h.get(desc..desc + 24).ok_or_else(short)?;
        level = u16le(a, 0).ok_or_else(short)?;
        flags = u16le(a, 2).ok_or_else(short)?;
        bpf = u32le(a, 4).ok_or_else(short)?;
        final_blocks = u32le(a, 8).ok_or_else(short)?;
        total_frames = u32le(a, 12).ok_or_else(short)?;
        bits = u16le(a, 16).ok_or_else(short)?;
        channels = u16le(a, 18).ok_or_else(short)?;
        rate = u32le(a, 20).ok_or_else(short)?;
        seek_at = start + desc + head;
        seek_count = seek_bytes / 4;
    } else {
        level = u16le(h, 6).ok_or_else(short)?;
        flags = u16le(h, 8).ok_or_else(short)?;
        channels = u16le(h, 10).ok_or_else(short)?;
        rate = u32le(h, 12).ok_or_else(short)?;
        let wav_header = u32le(h, 16).ok_or_else(short)? as usize;
        terminating = u32le(h, 20).ok_or_else(short)? as usize;
        total_frames = u32le(h, 24).ok_or_else(short)?;
        final_blocks = u32le(h, 28).ok_or_else(short)?;
        let mut at = 32;
        if flags & FLAG_HAS_PEAK_LEVEL != 0 {
            at += 4;
        }
        if flags & FLAG_HAS_SEEK_ELEMENTS != 0 {
            seek_count = u32le(h, at).ok_or_else(short)? as usize;
            at += 4;
        } else {
            seek_count = total_frames as usize;
        }
        if flags & FLAG_CREATE_WAV_HEADER == 0 {
            at += wav_header;
        }
        seek_at = start + at;
        bpf = if version >= 3950 {
            73728 * 4
        } else if version >= 3900 || (version >= 3800 && level == 4000) {
            73728
        } else {
            9216
        };
        bits = if flags & FLAG_8_BIT != 0 {
            8
        } else if flags & FLAG_24_BIT != 0 {
            24
        } else {
            16
        };
    }
    if !(1..=MAX_CHANNELS).contains(&channels) {
        return Err(Error::Unsupported(format!("{channels} channels")));
    }
    if !matches!(bits, 8 | 16 | 24 | 32) {
        return Err(Error::Unsupported(format!("{bits}-bit samples")));
    }
    if !matches!(level, 1000 | 2000 | 3000 | 4000 | 5000) {
        return Err(Error::Unsupported(format!("compression level {level}")));
    }
    if version < 3950 && (channels > 2 || bits > 24) {
        return Err(bad("channels or sample size of an old file"));
    }
    if version < 3950 && level == 5000 {
        return Err(bad("insane level in a file from before 3.95"));
    }
    if version < 3930 && matches!(level, 1000 | 3000) {
        return Err(Error::Unsupported(format!(
            "the {} level of files from before 3.93",
            if level == 1000 { "fast" } else { "high" }
        )));
    }
    if bpf == 0 || bpf > MAX_FRAME_BLOCKS || final_blocks > bpf || rate == 0 {
        return Err(bad("frame size"));
    }
    let float = flags & FLAG_FLOATING_POINT != 0;
    if float && bits != 32 {
        return Err(bad("floating point without 32 bits"));
    }
    let n = total_frames as usize;
    if seek_count < n || seek_at + 4 * n > d.len() {
        return Err(bad("seek table"));
    }
    // Offsets wrap at 4 GB in the 32-bit table.
    let mut seeks = Vec::with_capacity(n);
    let (mut add, mut prev) = (0u64, 0u32);
    for i in 0..n {
        let v = u32le(d, seek_at + 4 * i).ok_or_else(|| bad("seek table"))?;
        if v < prev {
            add += 1 << 32;
        }
        prev = v;
        seeks.push(add + u64::from(v) + start as u64);
    }
    let data_end = d
        .len()
        .saturating_sub(tag_bytes(d))
        .saturating_sub(terminating) as u64;
    // Up to 3.80 frames start at any bit: a byte of bit offsets follows the seek table.
    let seek_bits = if version <= 3800 {
        let at = seek_at + 4 * seek_count;
        Some(d.get(at..at + n).ok_or_else(|| bad("seek table"))?)
    } else {
        None
    };
    let first = seeks.first().copied().unwrap_or(0);
    let mut frames = Vec::with_capacity(n);
    for i in 0..n {
        let s = seeks[i];
        let e = if i + 1 < n { seeks[i + 1] } else { data_end };
        if s >= d.len() as u64 {
            // A cut file: the frames past its end are gone.
            break;
        }
        if e < s {
            return Err(bad("seek table"));
        }
        let (skip_bytes, skip) = match seek_bits {
            Some(b) => (0, usize::from(b[i])),
            None => {
                let k = ((s - first) % 4) as usize;
                (k, 8 * k)
            }
        };
        let offset = s as usize - skip_bytes;
        // a frame that starts mid-byte may end in the next frame's first word
        let spare = if seek_bits.is_some() { 8 } else { 4 };
        let len = ((e.min(d.len() as u64) - s) as usize + skip_bytes + spare).min(d.len() - offset);
        frames.push(Frame {
            offset,
            len,
            skip,
            blocks: if i + 1 == n { final_blocks } else { bpf },
        });
    }
    let total_blocks = if n == 0 {
        0
    } else {
        (n as u64 - 1) * u64::from(bpf) + u64::from(final_blocks)
    };
    Ok(Info {
        version,
        level,
        channels,
        sample_rate: rate,
        bits,
        float,
        total_blocks,
        frames,
        crc: version >= 3930 || flags & FLAG_CRC != 0,
    })
}

enum Preds {
    Narrow(Vec<Predictor<i32, i16>>),
    Wide(Vec<Predictor<i64, i32>>),
    /// Files 3930–3949.
    Old3930(Vec<old::Pred3930>),
    /// Files before 3930: whole-frame anti-predictors, no state between frames.
    Frames,
}

impl Preds {
    fn flush(&mut self) {
        match self {
            Preds::Narrow(p) => p.iter_mut().for_each(|p| p.flush()),
            Preds::Wide(p) => p.iter_mut().for_each(|p| p.flush()),
            Preds::Old3930(p) => p.iter_mut().for_each(|p| p.flush()),
            Preds::Frames => {}
        }
    }
    fn get(&mut self, c: usize, a: i64, b: i32) -> i32 {
        match self {
            Preds::Narrow(p) => p[c].decompress(a, b),
            Preds::Wide(p) => p[c].decompress(a, b),
            Preds::Old3930(p) => p[c].decompress(a),
            Preds::Frames => 0,
        }
    }
}

/// Decodes a file's frames.
pub struct Decoder {
    version: u16,
    level: u16,
    channels: usize,
    bits: u16,
    float: bool,
    crc: bool,
    preds: Preds,
    values: Vec<i32>,
}

const SPECIAL_LEFT_SILENCE: u32 = 1;
const SPECIAL_RIGHT_SILENCE: u32 = 2;
const SPECIAL_PSEUDO_STEREO: u32 = 4;

impl Decoder {
    pub fn new(info: &Info) -> Result<Decoder, Error> {
        let n = usize::from(info.channels);
        let unsupported = || Error::Unsupported(format!("compression level {}", info.level));
        let preds = if info.version < 3930 {
            Preds::Frames
        } else if info.version < 3950 {
            Preds::Old3930(
                (0..n)
                    .map(|_| old::Pred3930::new(info.level, info.version))
                    .collect::<Option<_>>()
                    .ok_or_else(unsupported)?,
            )
        } else if info.bits < 32 {
            Preds::Narrow(
                (0..n)
                    .map(|_| Predictor::new(info.level, info.version, info.bits))
                    .collect::<Option<_>>()
                    .ok_or_else(unsupported)?,
            )
        } else {
            Preds::Wide(
                (0..n)
                    .map(|_| Predictor::new(info.level, info.version, info.bits))
                    .collect::<Option<_>>()
                    .ok_or_else(unsupported)?,
            )
        };
        Ok(Decoder {
            version: info.version,
            level: info.level,
            channels: n,
            bits: info.bits,
            float: info.float,
            crc: info.crc,
            preds,
            values: vec![0; n],
        })
    }

    /// Bytes the decoder keeps besides its output (the filters' windows).
    pub fn buffer_bytes(info: &Info) -> u64 {
        let per = match info.level {
            5000 => 1552,
            4000 => 288,
            3000 => 64,
            2000 => 16,
            _ => 0,
        } + 3 * 512;
        u64::from(info.channels) * per as u64 * 8
    }

    /// Decodes frame `f` (its bytes: `data[f.offset..][..f.len]`) into `out`: interleaved
    /// samples in the file's channel order as integers (8-bit centred on 0), or for float
    /// files the IEEE bits of each `f32`.
    pub fn decode(
        &mut self,
        frame: &[u8],
        skip: usize,
        blocks: u32,
        out: &mut Vec<i32>,
    ) -> Result<(), Error> {
        if blocks > MAX_FRAME_BLOCKS {
            return Err(bad("frame too long"));
        }
        let start = out.len();
        let r = self.decode_inner(frame, skip, blocks as usize, out);
        if r.is_err() {
            out.truncate(start);
        }
        r
    }

    fn decode_inner(
        &mut self,
        frame: &[u8],
        skip: usize,
        n: usize,
        out: &mut Vec<i32>,
    ) -> Result<(), Error> {
        let mut rd = Reader::new(frame, skip, self.version);
        if self.version < 3930 {
            return self.decode_old(&mut rd, n, out);
        }
        let mut stored = rd.bits(32);
        let mut special = 0;
        if stored & 0x8000_0000 != 0 {
            special = rd.bits(32);
        }
        stored &= 0x7fff_ffff;
        self.preds.flush();
        let mut states = vec![State::default(); self.channels];
        rd.start_range();
        let err = |_| bad("frame data");
        let ch = self.channels;
        let start = out.len();
        out.reserve(n * ch);
        let mut crc = Crc::new();
        let mut bytes = Vec::with_capacity(ch * 4);
        let mut last_x = 0i32;
        for _ in 0..n {
            let v = &mut self.values;
            if ch > 2 {
                for c in 0..ch {
                    let a = rd.value(&mut states[c]).map_err(err)?;
                    v[c] = self.preds.get(c, a, 0);
                }
            } else if ch == 2 {
                if special & SPECIAL_LEFT_SILENCE != 0 && special & SPECIAL_RIGHT_SILENCE != 0 {
                    v[0] = 0;
                    v[1] = 0;
                } else if special & SPECIAL_PSEUDO_STEREO != 0 {
                    let a = rd.value(&mut states[0]).map_err(err)?;
                    v[0] = self.preds.get(0, a, 0);
                    v[1] = 0;
                } else if self.version < 3950 {
                    // before 3.95 the channels are predicted on their own, X first
                    let nx = rd.value(&mut states[0]).map_err(err)?;
                    v[0] = self.preds.get(0, nx, 0);
                    let ny = rd.value(&mut states[1]).map_err(err)?;
                    v[1] = self.preds.get(1, ny, 0);
                } else {
                    let ny = rd.value(&mut states[1]).map_err(err)?;
                    let nx = rd.value(&mut states[0]).map_err(err)?;
                    let y = self.preds.get(1, ny, last_x);
                    let x = self.preds.get(0, nx, y);
                    last_x = x;
                    v[0] = x;
                    v[1] = y;
                }
            } else if special & 1 != 0 {
                v[0] = 0;
            } else {
                let a = rd.value(&mut states[0]).map_err(err)?;
                v[0] = self.preds.get(0, a, 0);
            }
            unprepare(v, self.bits, out)?;
            bytes.clear();
            for &s in &out[out.len() - ch..] {
                match self.bits {
                    8 => bytes.push((s + 128) as u8),
                    16 => bytes.extend_from_slice(&(s as i16).to_le_bytes()),
                    24 => bytes.extend_from_slice(&s.to_le_bytes()[..3]),
                    _ => bytes.extend_from_slice(&s.to_le_bytes()),
                }
            }
            crc.update(&bytes);
        }
        if (crc.0 ^ 0xffff_ffff) >> 1 != stored {
            return Err(bad(if rd.overrun {
                "frame cut short"
            } else {
                "frame checksum"
            }));
        }
        if self.float {
            for s in out[start..].iter_mut() {
                *s = float_bits(*s as u32) as i32;
            }
        }
        Ok(())
    }

    /// A frame of a file from before 3.93 ([`old`]): each channel's residuals for the whole
    /// frame, then its anti-predictor, then the stereo pair undone.
    fn decode_old(&mut self, rd: &mut Reader, n: usize, out: &mut Vec<i32>) -> Result<(), Error> {
        let err = |_| bad("frame data");
        let v = self.version;
        let (stored, special) = if !self.crc {
            let s = old::old_checksum(rd).map_err(err)?;
            (s, if s == 0 { 3 } else { 0 })
        } else {
            let mut s = rd.bits(32);
            let mut special = 0;
            if v > 3820 {
                if s & 0x8000_0000 != 0 {
                    special = rd.bits(32);
                }
                s &= 0x7fff_ffff;
            }
            (s, special)
        };
        let ch = self.channels;
        let mut res = vec![0i32; n];
        let mut chans = vec![vec![0i32; n]; ch];
        let silent = if ch == 2 {
            special & SPECIAL_LEFT_SILENCE != 0 && special & SPECIAL_RIGHT_SILENCE != 0
        } else {
            special & SPECIAL_LEFT_SILENCE != 0
        };
        let coded = if silent {
            0
        } else if ch == 2 && special & SPECIAL_PSEUDO_STEREO != 0 {
            1
        } else {
            ch
        };
        for data in chans.iter_mut().take(coded) {
            old::residuals(rd, v, &mut res).map_err(err)?;
            match self.level {
                2000 => old::normal_3800(&res, data),
                4000 => old::extra_high_3800(&mut res, data, v),
                _ => {
                    return Err(Error::Unsupported(format!(
                        "compression level {}",
                        self.level
                    )));
                }
            }
        }
        // the range coder reads a few bytes ahead, past the end of a frame at the end of a
        // cut file: the checksum decides
        let start = out.len();
        out.reserve(n * ch);
        // 8-bit samples were stored unsigned until 3.83
        let offset = if self.bits == 8 && v <= 3830 { 128 } else { 0 };
        let mut sum = 0u32;
        let mut vals = vec![0i32; ch];
        for i in 0..n {
            for (v, c) in vals.iter_mut().zip(&chans) {
                *v = c[i];
            }
            if ch == 2 {
                let r = chans[0][i].wrapping_sub(chans[1][i] / 2);
                let l = r.wrapping_add(chans[1][i]);
                sum = sum.wrapping_add(r.unsigned_abs().wrapping_add(l.unsigned_abs()));
            } else {
                sum = sum.wrapping_add(chans[0][i].unsigned_abs());
            }
            unprepare(&vals, self.bits, out)?;
            if offset != 0 {
                let k = out.len() - ch;
                for s in &mut out[k..] {
                    *s = i32::from(*s as u8) - offset;
                }
            }
        }
        let ok = if self.crc {
            let mut crc = Crc::new();
            let mut bytes = Vec::with_capacity(4 * ch);
            for &s in &out[start..] {
                bytes.clear();
                match self.bits {
                    8 => bytes.push((s + 128) as u8),
                    16 => bytes.extend_from_slice(&(s as i16).to_le_bytes()),
                    _ => bytes.extend_from_slice(&s.to_le_bytes()[..3]),
                }
                crc.update(&bytes);
            }
            let c = crc.0 ^ 0xffff_ffff;
            (if v > 3820 { c >> 1 } else { c }) == stored
        } else {
            sum == stored
        };
        if !ok {
            out.truncate(start);
            return Err(bad("frame checksum"));
        }
        Ok(())
    }
}

/// The stored 32-bit value of a float sample to its IEEE bits.
fn float_bits(x: u32) -> u32 {
    let mut o = x & 0xC3FF_FFFF;
    o |= !(x & 0x3C00_0000) ^ 0xC3FF_FFFF;
    if o & 0x8000_0000 != 0 {
        o = !o | 0x8000_0000;
    }
    o
}

/// Undoes the channel decorrelation: middle/side pairs (stereo, and in 4- and 6–8-channel
/// files at 16 and 24 bits), and wraps each value to its sample width.
fn unprepare(v: &[i32], bits: u16, out: &mut Vec<i32>) -> Result<(), Error> {
    let ch = v.len();
    let pair = |x: i32, y: i32| -> (i32, i32) {
        let r = x.wrapping_sub(y / 2);
        (r, r.wrapping_add(y))
    };
    let wrap = |s: i32| -> i32 {
        match bits {
            8 => ((s + 128) as u8) as i32 - 128,
            16 => i32::from(s as i16),
            24 => (s << 8) >> 8,
            _ => s,
        }
    };
    let check16 = |a: i32, b: i32| -> Result<(), Error> {
        if bits == 16 && (!(-32768..=32767).contains(&a) || !(-32768..=32767).contains(&b)) {
            Err(bad("sample out of range"))
        } else {
            Ok(())
        }
    };
    if ch == 2 {
        let (a, b) = pair(v[0], v[1]);
        check16(a, b)?;
        out.push(wrap(a));
        out.push(wrap(b));
    } else if ch > 2 && (bits == 16 || bits == 24) && (ch == 4 || ch >= 6) {
        // Pairs (0,1), (2,3) for four channels; (0,1), (4,5) and (6,7) for six to eight, the
        // pair (2,3) (centre, LFE) left as it is.
        let mut c = 0;
        while c < ch {
            let joint = c % 2 == 0 && c + 1 < ch && c < 8 && (ch == 4 || c != 2);
            if joint {
                let (a, b) = pair(v[c], v[c + 1]);
                check16(a, b)?;
                out.push(wrap(a));
                out.push(wrap(b));
                c += 2;
            } else {
                out.push(wrap(v[c]));
                c += 1;
            }
        }
    } else {
        for &s in v {
            out.push(wrap(s));
        }
    }
    Ok(())
}

struct Crc(u32);

static CRC_TABLE: [u32; 256] = {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
};

impl Crc {
    fn new() -> Crc {
        Crc(0xffff_ffff)
    }
    fn update(&mut self, b: &[u8]) {
        for &x in b {
            self.0 = CRC_TABLE[((self.0 ^ u32::from(x)) & 0xff) as usize] ^ (self.0 >> 8);
        }
    }
}

/// The scale mapping decoded integers to −1..1 (`None` for float files).
pub fn int_scale(info: &Info) -> Option<f32> {
    (!info.float).then(|| 1.0 / (1u64 << (info.bits - 1)) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_is_ieee() {
        let mut c = Crc::new();
        c.update(b"123456789");
        assert_eq!(c.0 ^ 0xffff_ffff, 0xCBF4_3926);
    }

    #[test]
    fn old_versions_are_refused_by_name() {
        let mut d = b"MAC ".to_vec();
        d.extend_from_slice(&3790u16.to_le_bytes());
        d.resize(64, 0);
        match info(&d) {
            Err(Error::Unsupported(m)) => assert!(m.contains("before 3.80"), "{m}"),
            r => panic!("{r:?}"),
        }
    }

    #[test]
    fn six_channel_pairs() {
        let mut out = Vec::new();
        unprepare(&[10, 4, 7, 8, 1, 2], 16, &mut out).unwrap();
        assert_eq!(out, vec![8, 12, 7, 8, 0, 2]);
    }
}
