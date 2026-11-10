//! Shorten (`.shn`): Tony Robinson's lossless sound coder (Cambridge University Engineering
//! Department technical report CUED/F-INFENG/TR.156, 1994), stream versions 1 to 3.
//!
//! Own code, from the report's description of the coder: a stream of commands, each coded
//! as a Rice-style variable-length integer; per channel, blocks of samples predicted by
//! fixed polynomials (`DIFF0`–`DIFF3`) or a quantised LPC filter (`QLPC`) about a running
//! mean of the last blocks, with the residual's energy sent per block. A `VERBATIM` command
//! carries the original file's header (a WAVE or AIFF header gives the sampling rate).
//!
//! Decoding is sequential (each block predicts from the last samples and means of the
//! blocks before), so [`scan`] decodes the whole stream once and keeps the decoder's state at
//! the start of every run of about [`CHUNK`] frames; [`Decoder::decode_chunk`] starts from
//! such a point.

use std::fmt;

/// Frames (samples a channel) in a run between two resume points, at least.
pub const CHUNK: u32 = 4096;
/// The most channels a stream may have.
pub const MAX_CHANNELS: usize = 8;

const FN_SIZE: u32 = 2;
const ENERGY_SIZE: u32 = 3;
const BITSHIFT_SIZE: u32 = 2;
const LPCQ_SIZE: u32 = 2;
const LPC_QUANT: u32 = 5;
const ULONG_SIZE: u32 = 2;
const VERBATIM_CK_SIZE: u32 = 5;
const VERBATIM_BYTE_SIZE: u32 = 8;
const NWRAP: usize = 3;
const MAX_BLOCK: u32 = 1 << 16;
const MAX_LPC: u32 = 1024;
const MAX_MEAN: u32 = 1024;

const FN_DIFF0: u32 = 0;
const FN_DIFF1: u32 = 1;
const FN_DIFF2: u32 = 2;
const FN_DIFF3: u32 = 3;
const FN_QUIT: u32 = 4;
const FN_BLOCKSIZE: u32 = 5;
const FN_BITSHIFT: u32 = 6;
const FN_QLPC: u32 = 7;
const FN_ZERO: u32 = 8;
const FN_VERBATIM: u32 = 9;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// A stream this decoder doesn't read (named).
    Unsupported(String),
    Malformed(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unsupported(m) => write!(f, "unsupported Shorten stream: {m}"),
            Error::Malformed(m) => write!(f, "damaged Shorten stream: {m}"),
        }
    }
}

impl std::error::Error for Error {}

fn bad(m: &str) -> Error {
    Error::Malformed(m.into())
}

/// The sample type a stream's header names (Shorten's `TYPE_*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleType {
    S8,
    U8,
    S16,
    U16,
}

impl SampleType {
    pub fn bits(self) -> u16 {
        match self {
            SampleType::S8 | SampleType::U8 => 8,
            SampleType::S16 | SampleType::U16 => 16,
        }
    }
}

/// What a stream's header and the original file's header say.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub version: u8,
    pub sample_type: SampleType,
    pub channels: usize,
    /// From the WAVE or AIFF header kept verbatim, when there is one.
    pub sample_rate: Option<u32>,
    /// Frames, when the original header says (its data size).
    pub frames: Option<u64>,
    /// The bytes of the original file's header (`VERBATIM` before the first block).
    pub original_header: Vec<u8>,
}

/// MSB-first bit reader.
#[derive(Clone)]
struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
}

impl<'a> Bits<'a> {
    fn bit(&mut self) -> Option<u32> {
        let b = *self.d.get(self.pos >> 3)?;
        let v = (b >> (7 - (self.pos & 7))) & 1;
        self.pos += 1;
        Some(u32::from(v))
    }

    fn bits(&mut self, n: u32) -> Option<u32> {
        let mut v = 0u32;
        for _ in 0..n {
            v = (v << 1) | self.bit()?;
        }
        Some(v)
    }

    /// Zeros up to a one, then `n` low bits.
    fn uvar(&mut self, n: u32) -> Option<u32> {
        let mut hi = 0u32;
        // Find the next set bit a byte at a time where we can.
        loop {
            if self.pos & 7 == 0 {
                let b = *self.d.get(self.pos >> 3)?;
                if b == 0 {
                    hi = hi.checked_add(8)?;
                    self.pos += 8;
                    if hi > 1 << 24 {
                        return None;
                    }
                    continue;
                }
            }
            if self.bit()? == 1 {
                break;
            }
            hi += 1;
            if hi > 1 << 24 {
                return None;
            }
        }
        if n > 31 {
            return None;
        }
        let lo = self.bits(n)?;
        hi.checked_shl(n).map(|h| h | lo)
    }

    fn svar(&mut self, n: u32) -> Option<i32> {
        let u = self.uvar(n + 1)?;
        Some(if u & 1 != 0 {
            !((u >> 1) as i32)
        } else {
            (u >> 1) as i32
        })
    }

    fn ulong(&mut self) -> Option<u32> {
        let n = self.uvar(ULONG_SIZE)?;
        self.uvar(n)
    }
}

/// The decoder's state between blocks: what a resume point keeps.
#[derive(Clone, Debug)]
struct State {
    /// Bit position of the next command.
    pos: usize,
    block: u32,
    bitshift: u32,
    /// Per channel: the last `wrap` samples (oldest first).
    history: Vec<Vec<i32>>,
    /// Per channel: the means of the last `nmean` blocks.
    means: Vec<Vec<i32>>,
}

/// A resume point: the state before the block that starts frame `frame`.
#[derive(Clone, Debug)]
pub struct Point {
    pub frame: u64,
    state: State,
}

/// A stream's facts and its resume points.
#[derive(Clone, Debug)]
pub struct Stream {
    pub info: Info,
    /// Resume points, the first at frame 0, then one after every `CHUNK` frames or more.
    pub points: Vec<Point>,
    /// Frames in the stream.
    pub frames: u64,
    /// Byte offset of the stream in the bytes given (a SPHERE header before it).
    offset: usize,
    params: Params,
}

#[derive(Clone, Copy, Debug)]
struct Params {
    version: u8,
    channels: usize,
    maxnlpc: u32,
    nmean: u32,
    wrap: usize,
}

/// Whether `d` starts a Shorten stream.
pub fn is_shorten(d: &[u8]) -> bool {
    d.len() >= 5 && d.starts_with(b"ajkg") && (1..=3).contains(&d[4])
}

fn type_of(t: u32) -> Result<SampleType, Error> {
    Ok(match t {
        1 => SampleType::S8,
        2 => SampleType::U8,
        3 | 5 => SampleType::S16,
        4 | 6 => SampleType::U16,
        0 | 7 | 8 | 9 | 10 => {
            return Err(Error::Unsupported("µ-law or A-law samples".into()));
        }
        t => return Err(Error::Unsupported(format!("sample type {t}"))),
    })
}

/// The sampling rate, channels and frames a WAVE or AIFF header says.
fn original(h: &[u8]) -> (Option<u32>, Option<u64>) {
    let le32 = |at: usize| {
        h.get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let be32 = |at: usize| {
        h.get(at..at + 4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    };
    if h.len() >= 12 && &h[..4] == b"RIFF" && &h[8..12] == b"WAVE" {
        let (mut rate, mut align, mut data) = (None, None, None);
        let mut at = 12;
        while at + 8 <= h.len() {
            let size = le32(at + 4).unwrap_or(0) as usize;
            match &h[at..at + 4] {
                b"fmt " => {
                    rate = le32(at + 12);
                    align = h
                        .get(at + 20..at + 22)
                        .map(|b| u16::from_le_bytes([b[0], b[1]]));
                }
                b"data" => data = Some(size as u64),
                _ => {}
            }
            at += 8 + size + (size & 1);
        }
        let frames = match (data, align) {
            (Some(d), Some(a)) if a > 0 && d != u64::from(u32::MAX) => Some(d / u64::from(a)),
            _ => None,
        };
        return (rate, frames);
    }
    if h.len() >= 12 && &h[..4] == b"FORM" && matches!(&h[8..12], b"AIFF" | b"AIFC") {
        let mut at = 12;
        while at + 8 <= h.len() {
            let size = be32(at + 4).unwrap_or(0) as usize;
            if &h[at..at + 4] == b"COMM" && h.len() >= at + 26 {
                let c = &h[at + 8..];
                let frames = u64::from(be32(at + 10).unwrap_or(0));
                // The 80-bit extended rate.
                let exp = i32::from(u16::from_be_bytes([c[8], c[9]]) & 0x7FFF);
                let mant = u64::from_be_bytes(c[10..18].try_into().unwrap_or([0; 8]));
                let rate = (mant as f64) * 2f64.powi(exp - 16383 - 63);
                return ((rate >= 1.0).then_some(rate.round() as u32), Some(frames));
            }
            at += 8 + size + (size & 1);
        }
    }
    (None, None)
}

/// Reads the stream header at `offset` of `d`, then decodes the whole stream once to find
/// its length and its resume points.
pub fn scan(d: &[u8], offset: usize) -> Result<Stream, Error> {
    let s = d.get(offset..).ok_or_else(|| bad("no stream"))?;
    if !is_shorten(s) {
        return Err(bad("no Shorten magic"));
    }
    let version = s[4];
    let mut b = Bits { d: s, pos: 40 };
    let cut = || bad("header cut short");
    let t = b.ulong().ok_or_else(cut)?;
    let sample_type = type_of(t)?;
    let channels = b.ulong().ok_or_else(cut)? as usize;
    if channels == 0 || channels > MAX_CHANNELS {
        return Err(Error::Unsupported(format!("{channels} channels")));
    }
    let block = b.ulong().ok_or_else(cut)?;
    let maxnlpc = b.ulong().ok_or_else(cut)?;
    let nmean = b.ulong().ok_or_else(cut)?;
    let nskip = b.ulong().ok_or_else(cut)?;
    if block == 0 || block > MAX_BLOCK || maxnlpc > MAX_LPC || nmean > MAX_MEAN {
        return Err(bad("block size, LPC order or mean count out of range"));
    }
    for _ in 0..nskip {
        b.uvar(8).ok_or_else(cut)?;
    }
    let wrap = NWRAP.max(maxnlpc as usize);
    let params = Params {
        version,
        channels,
        maxnlpc,
        nmean,
        wrap,
    };
    let start = State {
        pos: b.pos,
        block,
        bitshift: 0,
        history: vec![vec![0; wrap]; channels],
        means: vec![vec![0; nmean as usize]; channels],
    };
    let mut dec = Run::new(s, params, start.clone());
    let mut header = Vec::new();
    let mut points = vec![Point {
        frame: 0,
        state: start,
    }];
    let mut frames = 0u64;
    let mut since = 0u64;
    let mut sink = Vec::new();
    loop {
        let before = dec.state.clone();
        sink.clear();
        match dec.next_frames(&mut sink, Some(&mut header))? {
            0 => break,
            n => {
                if since >= u64::from(CHUNK) {
                    points.push(Point {
                        frame: frames,
                        state: before,
                    });
                    since = 0;
                }
                frames += n as u64;
                since += n as u64;
            }
        }
    }
    let (sample_rate, original_frames) = original(&header);
    Ok(Stream {
        info: Info {
            version,
            sample_type,
            channels,
            sample_rate,
            frames: original_frames,
            original_header: header,
        },
        points,
        frames,
        offset,
        params,
    })
}

/// Decoding from a state.
struct Run<'a> {
    bits: Bits<'a>,
    p: Params,
    state: State,
    /// Each channel's block, decoded until all channels have theirs.
    blocks: Vec<Vec<i32>>,
    done: bool,
}

impl<'a> Run<'a> {
    fn new(d: &'a [u8], p: Params, state: State) -> Run<'a> {
        Run {
            bits: Bits { d, pos: state.pos },
            p,
            state,
            blocks: vec![Vec::new(); p.channels],
            done: false,
        }
    }

    /// The mean of a channel's last blocks, the offset its samples are coded about.
    fn offset(&self, ch: usize) -> i32 {
        let n = self.p.nmean as i64;
        if n == 0 {
            return 0;
        }
        let m = &self.state.means[ch];
        if self.p.version < 2 {
            let sum: i64 = m.iter().map(|&v| i64::from(v)).sum();
            (sum / n) as i32
        } else {
            let sum: i64 = n / 2 + m.iter().map(|&v| i64::from(v)).sum::<i64>();
            let v = sum / n;
            let s = self.state.bitshift;
            if s == 0 {
                v as i32
            } else {
                ((v >> (s - 1)) >> 1) as i32
            }
        }
    }

    /// Decodes commands up to the end of the next block of every channel, appending its
    /// frames (interleaved) to `out`; the frames decoded (0 at the end). Verbatim bytes go to
    /// `header` when given.
    fn next_frames(
        &mut self,
        out: &mut Vec<i32>,
        mut header: Option<&mut Vec<u8>>,
    ) -> Result<usize, Error> {
        let cut = || bad("stream cut short");
        let ch_n = self.p.channels;
        let mut ch = 0;
        while !self.done {
            let cmd = self.bits.uvar(FN_SIZE).ok_or_else(cut)?;
            match cmd {
                FN_QUIT => {
                    self.done = true;
                    break;
                }
                FN_BLOCKSIZE => {
                    let n = self.bits.ulong().ok_or_else(cut)?;
                    if n == 0 || n > MAX_BLOCK {
                        return Err(bad("block size out of range"));
                    }
                    self.state.block = n;
                }
                FN_BITSHIFT => {
                    let s = self.bits.uvar(BITSHIFT_SIZE).ok_or_else(cut)?;
                    if s > 31 {
                        return Err(bad("bit shift out of range"));
                    }
                    self.state.bitshift = s;
                }
                FN_VERBATIM => {
                    let n = self.bits.uvar(VERBATIM_CK_SIZE).ok_or_else(cut)?;
                    for _ in 0..n {
                        let v = self.bits.uvar(VERBATIM_BYTE_SIZE).ok_or_else(cut)?;
                        if let Some(h) = header.as_deref_mut()
                            && h.len() < 1 << 20
                        {
                            h.push(v as u8);
                        }
                    }
                }
                FN_DIFF0 | FN_DIFF1 | FN_DIFF2 | FN_DIFF3 | FN_QLPC | FN_ZERO => {
                    self.block(cmd, ch)?;
                    ch += 1;
                    if ch == ch_n {
                        break;
                    }
                }
                c => return Err(bad(&format!("command {c}"))),
            }
        }
        if ch < ch_n {
            // The end of the stream (a channel's block missing is a cut stream).
            return if ch == 0 { Ok(0) } else { Err(cut()) };
        }
        let n = self.blocks.iter().map(Vec::len).min().unwrap_or(0);
        let shift = self.state.bitshift;
        out.reserve(n * ch_n);
        for i in 0..n {
            for b in &self.blocks {
                out.push(b[i].wrapping_shl(shift));
            }
        }
        self.state.pos = self.bits.pos;
        Ok(n)
    }

    /// One channel's block.
    fn block(&mut self, cmd: u32, ch: usize) -> Result<(), Error> {
        let cut = || bad("stream cut short");
        let n = self.state.block as usize;
        let wrap = self.p.wrap;
        let offset = self.offset(ch);
        // History then the block, so `x[wrap + i - k]` is k samples back.
        let mut x = std::mem::take(&mut self.blocks[ch]);
        x.clear();
        x.extend_from_slice(&self.state.history[ch]);
        x.resize(wrap + n, 0);
        match cmd {
            FN_ZERO => {}
            FN_QLPC => {
                let resn = self.bits.uvar(ENERGY_SIZE).ok_or_else(cut)?;
                let order = self.bits.uvar(LPCQ_SIZE).ok_or_else(cut)? as usize;
                if order > self.p.maxnlpc as usize || order > wrap {
                    return Err(bad("LPC order above the stream's"));
                }
                let mut q = [0i32; MAX_LPC as usize];
                for c in q.iter_mut().take(order) {
                    *c = self.bits.svar(LPC_QUANT).ok_or_else(cut)?;
                }
                for v in &mut x[wrap - order..wrap] {
                    *v = v.wrapping_sub(offset);
                }
                let round: i64 = if self.p.version < 2 {
                    0
                } else {
                    1 << LPC_QUANT
                };
                for i in wrap..wrap + n {
                    let mut sum = round;
                    for (k, c) in q[..order].iter().enumerate() {
                        sum += i64::from(*c) * i64::from(x[i - 1 - k]);
                    }
                    let r = self.bits.svar(resn).ok_or_else(cut)?;
                    x[i] = r.wrapping_add((sum >> LPC_QUANT) as i32);
                }
                for v in &mut x[wrap..] {
                    *v = v.wrapping_add(offset);
                }
            }
            _ => {
                let mut resn = self.bits.uvar(ENERGY_SIZE).ok_or_else(cut)?;
                if self.p.version == 0 {
                    resn = resn.saturating_sub(1);
                }
                for i in wrap..wrap + n {
                    let r = self.bits.svar(resn).ok_or_else(cut)?;
                    let (a, b, c) = (x[i - 1], x[i - 2], x[i - 3]);
                    x[i] = match cmd {
                        FN_DIFF0 => r.wrapping_add(offset),
                        FN_DIFF1 => r.wrapping_add(a),
                        FN_DIFF2 => r.wrapping_add(a.wrapping_mul(2).wrapping_sub(b)),
                        _ => r.wrapping_add(a.wrapping_sub(b).wrapping_mul(3).wrapping_add(c)),
                    };
                }
            }
        }
        // The running means.
        if self.p.nmean > 0 {
            let mut sum: i64 = if self.p.version < 2 { 0 } else { n as i64 / 2 };
            sum += x[wrap..].iter().map(|&v| i64::from(v)).sum::<i64>();
            let m = &mut self.state.means[ch];
            m.rotate_left(1);
            let mean = sum / n as i64;
            *m.last_mut().expect("nmean > 0") = if self.p.version < 2 {
                mean as i32
            } else {
                (mean as i32).wrapping_shl(self.state.bitshift)
            };
        }
        let h = &mut self.state.history[ch];
        h.copy_from_slice(&x[n..n + wrap]);
        x.drain(..wrap);
        self.blocks[ch] = x;
        Ok(())
    }
}

/// Decodes runs of a scanned stream.
pub struct Decoder {
    params: Params,
    offset: usize,
}

impl Decoder {
    pub fn new(s: &Stream) -> Decoder {
        Decoder {
            params: s.params,
            offset: s.offset,
        }
    }

    /// Decodes from resume point `p` up to the next one (or `max_frames` frames), appending
    /// interleaved samples in the stream's units (signed; unsigned types are centred) to
    /// `out`; the frames decoded.
    pub fn decode_chunk(
        &self,
        d: &[u8],
        p: &Point,
        max_frames: u64,
        out: &mut Vec<i32>,
    ) -> Result<u64, Error> {
        let s = d.get(self.offset..).ok_or_else(|| bad("no stream"))?;
        let mut run = Run::new(s, self.params, p.state.clone());
        let mut got = 0u64;
        while got < max_frames {
            match run.next_frames(out, None)? {
                0 => break,
                n => got += n as u64,
            }
        }
        Ok(got)
    }
}

/// The scale from a stream's samples to ±1.0, and the centre unsigned types are coded
/// about (Shorten keeps unsigned samples as written; their mean is the offset).
pub fn scale(t: SampleType) -> (f32, i32) {
    match t {
        SampleType::S8 => (1.0 / 128.0, 0),
        SampleType::U8 => (1.0 / 128.0, 128),
        SampleType::S16 => (1.0 / 32768.0, 0),
        SampleType::U16 => (1.0 / 32768.0, 32768),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variable_length_integers() {
        // uvar(2) of 6: a zero, a one, then "10"; svar(1) of −2: uvar(2) of 3 → "1", "11"
        let d = [0b0110_1110, 0];
        let mut b = Bits { d: &d, pos: 0 };
        assert_eq!(b.uvar(2), Some(6));
        assert_eq!(b.svar(1), Some(-2));
    }
}
