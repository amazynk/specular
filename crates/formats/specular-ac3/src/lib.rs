//! Specular's own AC-3 and E-AC-3 decoder (ATSC A/52:2012 with the alternate bit stream
//! syntax of Annex D and Enhanced AC-3 of Annex E): the sound of DVDs, camcorder AVCHD
//! (MTS/M2TS), broadcast transport streams, Blu-ray, and Matroska and MP4 movies.
//!
//! - AC-3: every audio coding mode (1+1 dual mono to 3/2) with LFE; coupling with phase
//!   flags, rematrixing, block switching, delta bit allocation, dither; `bsid` 9 and 10
//!   (half and quarter rate AC-3).
//! - E-AC-3: 1, 2, 3 or 6 blocks per frame, reduced sample rates, frame exponent
//!   strategies, spectral extension, the adaptive hybrid transform (vector and gain-adaptive
//!   quantization). Dependent substreams of the first programme (Annex E §2.3.1.1: 7.1
//!   and other extension channels) are decoded and mixed in: their channels go where
//!   their custom channel map (Table E2.5) or coding mode puts them, replacing the
//!   independent frame's at the same 5.1 locations; back channels (Lrs/Rrs, Cs) make 7.1,
//!   the rest (Lc/Rc, wide, height, top, LFE2) fold into the nearest 7.1 speakers. Further
//!   independent programmes are skipped. Enhanced coupling is refused in words (no known
//!   encoder uses it); transient pre-noise processing (optional) is not applied.
//! - Output: stereo (Lo/Ro) with the stream's centre, surround and LFE mix levels, dynamic
//!   range control and dialogue normalisation to the −31 dBFS line level (back channels
//!   count as surrounds); or every channel in the WAVE order (with dependent substreams:
//!   5.1, or 7.1 as L R C LFE Lb Rb Ls Rs).
//!
//! A frame with dependent substreams is put out once they are in: when the bytes given
//! end (a container sample holds the whole frame), when the next programme frame starts,
//! or at [`Decoder::flush`].
//!
//! Input is the byte stream in any pieces: frames are found by their sync word, length and
//! CRC. A frame failing its CRC between two good sync words is concealed as silence.
#![deny(unsafe_code)]
#![allow(clippy::needless_range_loop)]

mod alloc;
mod bits;
mod frame;
mod header;
mod mdct;
mod tables;

pub use header::FrameInfo;

use header::{NFCHANS, location_channels, locations};

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
            Error::Malformed(s) => write!(f, "damaged AC-3: {s}"),
        }
    }
}

impl std::error::Error for Error {}

/// How the PCM is made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// Fold to two channels (mono stays mono only when `stereo` is off).
    pub stereo: bool,
    /// Apply dialogue normalisation (to −31 dBFS) and E-AC-3's programme scale factors.
    pub levels: bool,
    /// The share of the stream's dynamic range control to apply (0 none, 1 all).
    pub drc: f32,
    /// Dither for zero-bit mantissas when the stream asks for it.
    pub dither: bool,
    /// Mix the LFE into a stereo fold at the level an E-AC-3 stream gives (Annex E §3.9).
    pub lfe_mix: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            stereo: true,
            levels: true,
            drc: 1.0,
            dither: true,
            lfe_mix: true,
        }
    }
}

/// What a stream holds, from its first frames.
#[derive(Clone, Debug, PartialEq)]
pub struct StreamInfo {
    /// The first programme's independent frame.
    pub first: FrameInfo,
    /// Channels of the programme, dependent substreams' extension channels included (8 for
    /// 7.1).
    pub channels: usize,
    /// Whether the programme has dependent substreams (extension channels).
    pub dependents: bool,
    /// The programme's Table E2.5 locations (bit 0, the left, in the top bit).
    pub locations: u16,
}

impl StreamInfo {
    /// The channels [`Decoder`] puts out with `stereo` off: the frame's own (in the WAVE
    /// order) or, with dependent substreams, 8 (7.1) when a back channel is among them,
    /// else 6 (5.1).
    pub fn output_channels(&self) -> usize {
        if !self.dependents {
            self.first.channels()
        } else if self.locations & (loc::LRS_RRS | loc::CS) != 0 {
            8
        } else {
            6
        }
    }
}

impl StreamInfo {
    /// "AC-3" or "E-AC-3".
    pub fn name(&self) -> &'static str {
        if self.first.eac3 { "E-AC-3" } else { "AC-3" }
    }
}

/// Finds the first whole frame with a good CRC in `data`.
fn next_frame(data: &[u8], mut from: usize) -> Option<(usize, FrameInfo)> {
    while from + 8 <= data.len() {
        let p = from + data[from..].windows(2).position(|w| w == [0x0B, 0x77])?;
        if let Some(info) = FrameInfo::parse(&data[p..])
            && p + info.frame_bytes <= data.len()
            && bits::crc16(&data[p + 2..p + info.frame_bytes]) == 0
        {
            return Some((p, info));
        }
        from = p + 1;
    }
    None
}

/// The stream in `data` with every dither flag cleared (each frame's CRC made again), so
/// zero-bit mantissas decode as zero in any decoder: an oracle's output can then be compared
/// sample for sample, with no random noise in it. For tests; E-AC-3 frames that dither
/// without flags (`dithflage` off) and frames that don't decode stay as they are.
#[doc(hidden)]
pub fn without_dither(data: &[u8]) -> Vec<u8> {
    let mut out = data.to_vec();
    let mut st = frame::State::new();
    let p = frame::Process {
        dither: false,
        drc: 1.0,
    };
    let mut at = 0;
    while let Some(k) = out[at..].windows(2).position(|w| w == [0x0B, 0x77]) {
        let s = at + k;
        let Some(n) = FrameInfo::parse(&out[s..])
            .map(|i| i.frame_bytes)
            .filter(|&n| n >= 8 && s + n <= out.len() && bits::crc16(&out[s + 2..s + n]) == 0)
        else {
            at = s + 1;
            continue;
        };
        let frame = &mut out[s..s + n];
        if let Some(bsi) = header::parse_bsi(frame)
            && frame::decode(frame, &bsi, &mut st, p).is_ok()
            && !st.dith_at.is_empty()
        {
            for &b in &st.dith_at {
                frame[b >> 3] &= !(0x80 >> (b & 7));
            }
            if !bsi.info.eac3 {
                // CRC1 again: the first 5/8 of the frame (after the sync word) leave no
                // remainder.
                let words = n / 2;
                let end = 2 * ((words >> 1) + (words >> 3));
                let c = crc_at_start(&frame[4..end]);
                frame[2..4].copy_from_slice(&c.to_be_bytes());
            }
            // CRC2 again, so the whole frame's remainder is zero.
            let c = bits::crc16(&frame[2..n - 2]);
            frame[n - 2..].copy_from_slice(&c.to_be_bytes());
        }
        at = s + n;
    }
    out
}

/// An independent E-AC-3 frame rewritten as dependent substream `substreamid` (Annex E
/// §2.3.1.1): `strmtyp` 1, the custom channel map `chanmap` (Table E2.5, bit 0 in the top
/// bit) when given, the fields only an independent frame carries taken out (programme
/// scale factors and the rest of its mixing metadata, `convsync`, `convexpstr`,
/// `convsnroffst`), the frame size and CRC made again. The audio data stays bit for bit.
/// For tests: no encoder at hand writes dependent substreams. None if `frame` is not a
/// whole independent E-AC-3 frame that decodes.
#[doc(hidden)]
pub fn as_dependent(frame: &[u8], substreamid: u8, chanmap: Option<u16>) -> Option<Vec<u8>> {
    let bsi = header::parse_bsi(frame)?;
    let n = bsi.info.frame_bytes;
    if !bsi.info.eac3 || bsi.info.strmtyp != 0 || frame.len() < n || substreamid > 7 {
        return None;
    }
    let mut st = frame::State::new();
    let p = frame::Process {
        dither: false,
        drc: 1.0,
    };
    frame::decode(&frame[..n], &bsi, &mut st, p).ok()?;
    let mut gone: Vec<(usize, usize)> = bsi.indep_only.clone();
    gone.extend(st.indep_at.iter().copied());
    gone.sort_unstable();
    let bit = |i: usize| (frame[i >> 3] >> (7 - (i & 7))) & 1 == 1;
    let mut w = bits::Writer::default();
    w.put(0x0B77, 16);
    w.put(1, 2); // strmtyp: dependent
    w.put(u32::from(substreamid), 3);
    w.put(0, 11); // frmsiz, below
    for i in 32..bsi.chanmap_at {
        w.put(u32::from(bit(i)), 1);
    }
    match chanmap {
        Some(m) => {
            w.put(1, 1);
            w.put(u32::from(m), 16);
        }
        None => w.put(0, 1),
    }
    // everything up to auxdatae, encinfo and crc2 (the last 18 bits), less what goes
    let end = n * 8 - 18;
    let mut g = gone.iter().peekable();
    let mut i = bsi.chanmap_at;
    while i < end {
        if let Some(&&(s, l)) = g.peek()
            && s <= i
        {
            i = i.max(s + l);
            g.next();
            continue;
        }
        w.put(u32::from(bit(i)), 1);
        i += 1;
    }
    while (w.len() + 18) % 16 != 0 {
        w.put(0, 1);
    }
    w.put(0, 2); // auxdatae, encinfo
    w.put(0, 16);
    let mut out = w.into_bytes();
    let words = out.len() / 2;
    if !(1..=2048).contains(&words) {
        return None;
    }
    let sz = (words - 1) as u16;
    out[2] = (out[2] & 0xF8) | (sz >> 8) as u8;
    out[3] = sz as u8;
    let m = out.len();
    let c = bits::crc16(&out[2..m - 2]);
    out[m - 2..].copy_from_slice(&c.to_be_bytes());
    Some(out)
}

/// The 16 bits that, put before `rest`, leave the CRC no remainder (AC-3's CRC1 leads the
/// bytes it covers). The CRC is linear: the remainder of the bits before `rest` is solved
/// for from each bit's own.
fn crc_at_start(rest: &[u8]) -> u16 {
    let rem = |lead: u16| {
        let mut v = lead.to_be_bytes().to_vec();
        v.extend_from_slice(rest);
        bits::crc16(&v)
    };
    let target = rem(0);
    // rows: the remainder each lead bit adds (linear over GF(2)), with that bit
    let mut rows: Vec<(u16, u16)> = (0..16).map(|k| (rem(1 << k) ^ target, 1u16 << k)).collect();
    let (mut want, mut x) = (target, 0u16);
    // Gaussian elimination: each remainder bit from the highest
    for bit in (0..16).rev() {
        let m = 1u16 << bit;
        let Some(i) = rows.iter().position(|r| r.0 & m != 0) else {
            continue;
        };
        let pivot = rows.swap_remove(i);
        for r in &mut rows {
            if r.0 & m != 0 {
                r.0 ^= pivot.0;
                r.1 ^= pivot.1;
            }
        }
        if want & m != 0 {
            want ^= pivot.0;
            x ^= pivot.1;
        }
    }
    x
}

/// The stream in `data` (a few frames from its start), or None if no AC-3 frame is found.
pub fn probe(data: &[u8]) -> Option<StreamInfo> {
    let (mut at, first) = next_frame(data, 0)?;
    let mut info = first;
    let mut loc = locations(first.acmod, first.lfe);
    let mut seen = 0;
    let mut indep = None;
    let mut in_first = false;
    let mut dependents = false;
    while let Some((p, f)) = next_frame(data, at) {
        if f.strmtyp != 1 && f.substreamid == 0 {
            if indep.is_some() {
                break;
            }
            indep = Some(f);
            info = f;
            loc = locations(f.acmod, f.lfe);
            in_first = true;
        } else if f.strmtyp == 1 && in_first {
            dependents = true;
            loc |= f.chanmap.unwrap_or_else(|| locations(f.acmod, f.lfe));
        } else if f.strmtyp != 1 {
            in_first = false;
        }
        at = p + f.frame_bytes;
        seen += 1;
        if seen > 16 {
            break;
        }
    }
    Some(StreamInfo {
        first: info,
        channels: location_channels(loc),
        dependents,
        locations: loc,
    })
}

/// Decodes a byte stream of AC-3 or E-AC-3 syncframes.
pub struct Decoder {
    opts: Options,
    st: Box<frame::State>,
    buf: Vec<u8>,
    info: Option<FrameInfo>,
    out_ch: Option<usize>,
    /// Whether the programme's dependent substreams are mixed in (decided by the first
    /// frame put out, or by the caller).
    extended: Option<bool>,
    /// The programme's independent frame, decoded, with its dependent frames still to come.
    pending: Option<Pending>,
    /// Inside programme 0 (its dependent substreams follow).
    in_first: bool,
    /// One decoder state per dependent substream (0..8).
    deps: Vec<Box<frame::State>>,
    /// The programme's channels as [`BED`], once dependent substreams are in.
    bed: Vec<[f32; 1536]>,
}

/// An independent frame decoded, waiting for its dependent substreams.
struct Pending {
    bsi: header::Bsi,
    /// Dependent frames mixed in, and the locations they and the frame cover.
    deps: usize,
    locations: u16,
}

/// The programme's channels with dependent substreams: 7.1 in the WAVE order (L R C LFE,
/// back pair, side pair); other Table E2.5 locations are folded in.
const BED: usize = 8;
const B_L: usize = 0;
const B_R: usize = 1;
const B_C: usize = 2;
const B_LFE: usize = 3;
const B_LB: usize = 4;
const B_RB: usize = 5;
const B_LS: usize = 6;
const B_RS: usize = 7;

/// Table E2.5 locations (bit 0, the left, in the top bit).
mod loc {
    pub const L: u16 = 1 << 15;
    pub const C: u16 = 1 << 14;
    pub const R: u16 = 1 << 13;
    pub const LS: u16 = 1 << 12;
    pub const RS: u16 = 1 << 11;
    pub const LC_RC: u16 = 1 << 10;
    pub const LRS_RRS: u16 = 1 << 9;
    pub const CS: u16 = 1 << 8;
    pub const TS: u16 = 1 << 7;
    pub const LSD_RSD: u16 = 1 << 6;
    pub const LW_RW: u16 = 1 << 5;
    pub const VHL_VHR: u16 = 1 << 4;
    pub const VHC: u16 = 1 << 3;
    pub const LTS_RTS: u16 = 1 << 2;
    pub const LFE2: u16 = 1 << 1;
    pub const LFE: u16 = 1;
    /// Locations that are a pair of channels.
    pub const PAIRS: u16 = LC_RC | LRS_RRS | LSD_RSD | LW_RW | VHL_VHR | LTS_RTS;
}

/// One channel's place: (bed slot, gain) pairs, and whether it replaces what the
/// independent frame put there (the 5.1 locations) or adds to it.
fn placement(location: u16, right: bool) -> (&'static [(usize, f32)], bool) {
    const H: f32 = std::f32::consts::FRAC_1_SQRT_2;
    let side = |l: &'static [(usize, f32)], r: &'static [(usize, f32)]| if right { r } else { l };
    match location {
        loc::L => (&[(B_L, 1.0)], true),
        loc::C => (&[(B_C, 1.0)], true),
        loc::R => (&[(B_R, 1.0)], true),
        loc::LS => (&[(B_LS, 1.0)], true),
        loc::RS => (&[(B_RS, 1.0)], true),
        loc::LFE => (&[(B_LFE, 1.0)], true),
        loc::LRS_RRS => (side(&[(B_LB, 1.0)], &[(B_RB, 1.0)]), true),
        // between front and centre: panned at equal power
        loc::LC_RC => (side(&[(B_L, H), (B_C, H)], &[(B_R, H), (B_C, H)]), false),
        loc::CS => (&[(B_LB, H), (B_RB, H)], true),
        loc::TS => (&[(B_LS, H), (B_RS, H)], false),
        loc::LSD_RSD | loc::LTS_RTS => (side(&[(B_LS, 1.0)], &[(B_RS, 1.0)]), false),
        loc::LW_RW | loc::VHL_VHR => (side(&[(B_L, 1.0)], &[(B_R, 1.0)]), false),
        loc::VHC => (&[(B_C, 1.0)], false),
        loc::LFE2 => (&[(B_LFE, 1.0)], false),
        _ => (&[], false),
    }
}

/// The locations of a frame's coded channels in coding order (the LFE last): from its
/// custom channel map, else its coding mode. Each entry: (location, right of a pair).
fn channel_locations(info: &FrameInfo) -> Vec<(u16, bool)> {
    let mut v = Vec::with_capacity(8);
    match info.chanmap {
        Some(m) => {
            // the LFE is the last coded channel; its bit is the map's last
            let fbw = m & !loc::LFE;
            for b in (0..16).rev().map(|k| 1u16 << k).filter(|b| fbw & b != 0) {
                v.push((b, false));
                if b & loc::PAIRS != 0 {
                    v.push((b, true));
                }
            }
            v.truncate(NFCHANS[usize::from(info.acmod & 7)]);
        }
        None => {
            use loc::*;
            let m: &[u16] = match info.acmod {
                0 | 2 => &[L, R],
                1 => &[C],
                3 => &[L, C, R],
                4 => &[L, R, CS],
                5 => &[L, C, R, CS],
                6 => &[L, R, LS, RS],
                _ => &[L, C, R, LS, RS],
            };
            v.extend(m.iter().map(|&l| (l, false)));
        }
    }
    if info.lfe {
        v.push((loc::LFE, false));
    }
    v
}

/// The bed (cleared) with an independent frame's channels from its decoder state.
fn fill_bed(bed: &mut Vec<[f32; 1536]>, st: &frame::State, info: &FrameInfo) {
    let ns = info.samples();
    bed.resize(BED, [0.0; 1536]);
    for b in bed.iter_mut() {
        b[..ns].fill(0.0);
    }
    let coded = channel_locations(&FrameInfo {
        chanmap: None,
        ..*info
    });
    let nfch = NFCHANS[usize::from(info.acmod)];
    for (k, &(l, right)) in coded.iter().enumerate() {
        let slot = if k < nfch { k } else { frame::LFE };
        for &(t, g) in placement(l, right).0 {
            for (o, &v) in bed[t][..ns].iter_mut().zip(&st.pcm[slot][..ns]) {
                *o += g * v;
            }
        }
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Decoder::new(Options::default())
    }
}

impl Decoder {
    pub fn new(opts: Options) -> Decoder {
        Decoder {
            opts,
            st: Box::new(frame::State::new()),
            buf: Vec::new(),
            info: None,
            out_ch: None,
            extended: None,
            pending: None,
            in_first: false,
            deps: Vec::new(),
            bed: Vec::new(),
        }
    }

    /// Says up front whether the stream has dependent substreams ([`StreamInfo::dependents`]
    /// from [`probe`]), so the first frame needs no look at the next. Without it the
    /// decoder decides from the first frame and what follows it.
    pub fn with_dependents(mut self, yes: bool) -> Decoder {
        self.extended = Some(yes);
        self
    }

    /// Whether dependent substreams are mixed in: the output is then 5.1 or 7.1 in the
    /// WAVE order (stereo folded from it), not the independent frame's own channels.
    pub fn extended(&self) -> bool {
        self.extended == Some(true)
    }

    /// The last decoded frame's header.
    pub fn info(&self) -> Option<FrameInfo> {
        self.info
    }

    /// Output channels: 2 when folding to stereo, else the first frame's (0 before it).
    pub fn channels(&self) -> usize {
        if self.opts.stereo {
            2
        } else {
            self.out_ch.unwrap_or(0)
        }
    }

    /// Forgets buffered bytes and the transform overlap (after a seek).
    pub fn reset(&mut self) {
        self.buf.clear();
        self.st.reset();
        for d in &mut self.deps {
            d.reset();
        }
        self.pending = None;
        self.in_first = false;
    }

    /// Decodes the frames completed by `data`, appending interleaved PCM.
    pub fn decode(&mut self, data: &[u8], out: &mut Vec<f32>) -> Result<(), Error> {
        self.buf.extend_from_slice(data);
        self.run(out, false)
    }

    /// Decodes what is left at the end of the stream.
    pub fn flush(&mut self, out: &mut Vec<f32>) -> Result<(), Error> {
        let r = self.run(out, true);
        self.buf.clear();
        r
    }

    fn run(&mut self, out: &mut Vec<f32>, end: bool) -> Result<(), Error> {
        let mut at = 0;
        let mut first_err = None;
        loop {
            let Some(p) = self.buf[at..]
                .windows(2)
                .position(|w| w == [0x0B, 0x77])
                .map(|p| p + at)
            else {
                // keep a last 0x0B: it may start a sync word
                at = if self.buf.last() == Some(&0x0B) {
                    self.buf.len() - 1
                } else {
                    self.buf.len()
                };
                break;
            };
            at = p;
            let Some(info) = FrameInfo::parse(&self.buf[p..]) else {
                if self.buf.len() - p < 8 && !end {
                    break;
                }
                at = p + 1;
                continue;
            };
            let n = info.frame_bytes;
            if self.buf.len() - p < n {
                if end {
                    // a frame cut off at the end of the stream: its length in silence
                    self.conceal(&info, out);
                    at = self.buf.len();
                }
                break;
            }
            if bits::crc16(&self.buf[p + 2..p + n]) != 0 {
                // a damaged frame where the next one starts right after it, else a false sync
                let next = self.buf.get(p + n..p + n + 2);
                match next {
                    Some(s) if s == [0x0B, 0x77] => {
                        self.conceal(&info, out);
                        at = p + n;
                        if first_err.is_none() {
                            first_err = Some(Error::Malformed("CRC mismatch".into()));
                        }
                    }
                    None if !end => break,
                    _ => at = p + 1,
                }
                continue;
            }
            let frame = self.buf[p..p + n].to_vec();
            at = p + n;
            if let Err(e) = self.frame(&frame, out) {
                if let Some(i) = self.info {
                    self.conceal(&i, out);
                }
                if matches!(e, Error::Unsupported(_)) {
                    self.buf.drain(..at);
                    return Err(e);
                }
                first_err.get_or_insert(e);
            }
        }
        self.buf.drain(..at.min(self.buf.len()));
        // A container sample holds a whole frame with its dependent frames: what was given
        // is used up, so the frame is complete. Streams cut anywhere put it out when the
        // next programme frame starts, or at the end.
        // (a frame without dependent frames yet waits for them, or for the next frame)
        let complete = self.pending.as_ref().is_some_and(|p| p.deps > 0);
        if end || (self.buf.is_empty() && complete) {
            self.put_out(out);
        }
        match first_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Silence for a frame that could not be decoded, keeping the sound's length.
    fn conceal(&mut self, info: &FrameInfo, out: &mut Vec<f32>) {
        if info.strmtyp == 1 || info.substreamid != 0 {
            return;
        }
        self.put_out(out);
        self.in_first = false;
        self.st.reset();
        for d in &mut self.deps {
            d.reset();
        }
        if self.out_ch.is_none() && !self.opts.stereo {
            // nothing put out yet: the frame's own channels (or the bed)
            self.out_ch = Some(if self.extended() {
                BED
            } else {
                wave_order(info.acmod, info.lfe).len()
            });
        }
        let ch = self.channels().max(1);
        out.extend(std::iter::repeat_n(0.0, info.samples() * ch));
    }

    fn frame(&mut self, frame: &[u8], out: &mut Vec<f32>) -> Result<(), Error> {
        let Some(bsi) = header::parse_bsi(frame) else {
            return Err(Error::Malformed("frame header".into()));
        };
        let info = bsi.info;
        let p = frame::Process {
            dither: self.opts.dither,
            drc: self.opts.drc,
        };
        if info.strmtyp == 1 {
            // a dependent substream: of programme 0, unless decided against
            if self.in_first && self.extended != Some(false) {
                self.dependent(frame, &bsi, p);
            }
            return Ok(());
        }
        // an independent frame ends the frame before it
        self.put_out(out);
        if info.substreamid != 0 {
            // further programmes are skipped, with their dependent substreams
            self.in_first = false;
            return Ok(());
        }
        self.info = Some(info);
        let r = frame::decode(frame, &bsi, &mut self.st, p);
        self.in_first = r.is_ok();
        r?;
        let pending = Pending {
            locations: locations(info.acmod, info.lfe),
            bsi,
            deps: 0,
        };
        if self.extended == Some(false) {
            self.emit(&pending, out);
        } else {
            self.pending = Some(pending);
        }
        Ok(())
    }

    /// Decodes a dependent frame of the programme waiting and puts its channels in the bed.
    fn dependent(&mut self, frame: &[u8], bsi: &header::Bsi, p: frame::Process) {
        let Some(pend) = self.pending.as_mut() else {
            return;
        };
        let ns = bsi.info.samples();
        if ns != pend.bsi.info.samples() {
            return;
        }
        let id = usize::from(bsi.info.substreamid);
        while self.deps.len() <= id {
            self.deps.push(Box::new(frame::State::new()));
        }
        let st = &mut self.deps[id];
        if frame::decode(frame, bsi, st, p).is_err() {
            st.reset();
            return;
        }
        if pend.deps == 0 {
            // the independent frame's channels first
            fill_bed(&mut self.bed, &self.st, &pend.bsi.info);
        }
        pend.deps += 1;
        let nfch = NFCHANS[usize::from(bsi.info.acmod)];
        let coded = channel_locations(&bsi.info);
        // the 5.1 locations it carries replace the independent frame's
        let mut cleared = [false; BED];
        for &(l, right) in &coded {
            let (to, replace) = placement(l, right);
            if replace {
                for &(b, _) in to {
                    if !cleared[b] {
                        self.bed[b][..ns].fill(0.0);
                        cleared[b] = true;
                    }
                }
            }
        }
        for (k, &(l, right)) in coded.iter().enumerate() {
            let slot = if k < nfch { k } else { frame::LFE };
            pend.locations |= l;
            for &(b, g) in placement(l, right).0 {
                for (o, &v) in self.bed[b][..ns].iter_mut().zip(&st.pcm[slot][..ns]) {
                    *o += g * v;
                }
            }
        }
    }

    /// Puts out the frame waiting, deciding the output's channels at the first.
    fn put_out(&mut self, out: &mut Vec<f32>) {
        let Some(p) = self.pending.take() else {
            return;
        };
        if self.extended.is_none() {
            self.extended = Some(p.deps > 0);
        }
        self.emit(&p, out);
    }

    /// The programme gain: dialogue normalisation and programme scale factor `prog`.
    fn level(&self, bsi: &header::Bsi, prog: usize) -> f32 {
        let mut g = 1.0;
        if self.opts.levels {
            g *= 10f32.powf((f32::from(bsi.dialnorm[prog]) - 31.0) / 20.0);
            g *= bsi.pgmscl[prog];
        }
        g
    }

    fn emit(&mut self, p: &Pending, out: &mut Vec<f32>) {
        if self.extended() {
            self.emit_bed(p, out);
        } else {
            self.emit_own(&p.bsi, out);
        }
    }

    /// The bed (7.1, or 5.1 when no back channel was ever there), or its stereo fold.
    fn emit_bed(&mut self, p: &Pending, out: &mut Vec<f32>) {
        let b = &p.bsi;
        let ns = b.info.samples();
        if p.deps == 0 {
            // no dependent frame came: the independent frame's channels alone
            fill_bed(&mut self.bed, &self.st, &b.info);
        }
        let g = self.level(b, 0);
        let back = p.locations & (loc::LRS_RRS | loc::CS) != 0;
        let ch = *self.out_ch.get_or_insert(if back { 8 } else { 6 });
        let bed = &self.bed;
        if self.opts.stereo {
            // Lo/Ro: back channels as surrounds
            let (c, s) = (b.loro_cmix, b.loro_surmix);
            let e = match b.lfe_mix {
                Some(e) if self.opts.lfe_mix => e,
                _ => 0.0,
            };
            let norm = (1.0 + c + 2.0 * s + e).max(1.0);
            let k = g / norm;
            out.reserve(ns * 2);
            for i in 0..ns {
                let common = c * bed[B_C][i] + e * bed[B_LFE][i];
                let l = bed[B_L][i] + common + s * (bed[B_LS][i] + bed[B_LB][i]);
                let r = bed[B_R][i] + common + s * (bed[B_RS][i] + bed[B_RB][i]);
                out.push((l * k).clamp(-1.0, 1.0));
                out.push((r * k).clamp(-1.0, 1.0));
            }
        } else if ch == 8 {
            out.reserve(ns * 8);
            for i in 0..ns {
                for x in bed.iter() {
                    out.push((x[i] * g).clamp(-1.0, 1.0));
                }
            }
        } else {
            // 5.1: a back channel (never there so far) into the surround on its side
            out.reserve(ns * 6);
            for i in 0..ns {
                for t in [B_L, B_R, B_C, B_LFE] {
                    out.push((bed[t][i] * g).clamp(-1.0, 1.0));
                }
                out.push(((bed[B_LS][i] + bed[B_LB][i]) * g).clamp(-1.0, 1.0));
                out.push(((bed[B_RS][i] + bed[B_RB][i]) * g).clamp(-1.0, 1.0));
            }
        }
    }

    /// The independent frame's own channels (no dependent substreams), or their fold.
    fn emit_own(&mut self, bsi: &header::Bsi, out: &mut Vec<f32>) {
        let info = bsi.info;
        let nfch = NFCHANS[usize::from(info.acmod)];
        let ns = info.samples();
        let slots: Vec<usize> = (0..nfch).chain(info.lfe.then_some(frame::LFE)).collect();
        let (g0, g1) = (self.level(bsi, 0), self.level(bsi, 1));
        let pcm = &self.st.pcm;
        if self.opts.stereo {
            let m = stereo_matrix(bsi, self.opts.lfe_mix);
            out.reserve(ns * 2);
            for i in 0..ns {
                let (mut l, mut r) = (0f32, 0f32);
                for (k, &s) in slots.iter().enumerate() {
                    let v = pcm[s][i];
                    l += m[0][k] * v;
                    r += m[1][k] * v;
                }
                if info.acmod == 0 {
                    l *= g0;
                    r *= g1;
                } else {
                    l *= g0;
                    r *= g0;
                }
                out.push(l.clamp(-1.0, 1.0));
                out.push(r.clamp(-1.0, 1.0));
            }
        } else {
            let order = wave_order(info.acmod, info.lfe);
            self.out_ch.get_or_insert(order.len());
            out.reserve(ns * order.len());
            for i in 0..ns {
                for &s in &order {
                    out.push((pcm[s][i] * g0).clamp(-1.0, 1.0));
                }
            }
        }
    }
}

/// The slots of a coding mode in the WAVE order (L R C LFE, then surrounds).
fn wave_order(acmod: u8, lfe: bool) -> Vec<usize> {
    let lf = frame::LFE;
    let mut v: Vec<usize> = match acmod {
        0 | 2 => vec![0, 1],
        1 => vec![0],
        3 => vec![0, 2, 1],
        4 => vec![0, 1, 2],
        5 => vec![0, 2, 1, 3],
        6 => vec![0, 1, 2, 3],
        _ => vec![0, 2, 1, 3, 4],
    };
    if lfe {
        let at = match acmod {
            1 => 1,
            0 | 2 | 4 | 6 => 2,
            _ => 3,
        };
        v.insert(at, lf);
    }
    v
}

/// Left and right weights of each coded slot (fbw channels in coding order, then the LFE)
/// for the Lo/Ro stereo fold (A/52 §7.8.2, Annex D §3.1), scaled so no output can overload.
fn stereo_matrix(b: &header::Bsi, lfe_mix: bool) -> [[f32; 6]; 2] {
    const M3: f32 = std::f32::consts::FRAC_1_SQRT_2;
    let info = &b.info;
    // Lo/Ro: the stereo for listening (Lt/Rt is for matrix surround decoders), with the
    // stream's Lo/Ro levels
    let (c, s) = (b.loro_cmix, b.loro_surmix);
    let mut m = [[0f32; 6]; 2];
    // (left, right) weights per coded channel
    let w: Vec<(f32, f32)> = match info.acmod {
        0 => vec![(1.0, 0.0), (0.0, 1.0)],
        1 => vec![(M3, M3)],
        2 => vec![(1.0, 0.0), (0.0, 1.0)],
        3 => vec![(1.0, 0.0), (c, c), (0.0, 1.0)],
        4 => vec![(1.0, 0.0), (0.0, 1.0), (s * M3, s * M3)],
        5 => vec![(1.0, 0.0), (c, c), (0.0, 1.0), (s * M3, s * M3)],
        6 => vec![(1.0, 0.0), (0.0, 1.0), (s, 0.0), (0.0, s)],
        _ => vec![(1.0, 0.0), (c, c), (0.0, 1.0), (s, 0.0), (0.0, s)],
    };
    let n = w.len();
    for (k, &(l, r)) in w.iter().enumerate() {
        m[0][k] = l;
        m[1][k] = r;
    }
    if info.lfe
        && lfe_mix
        && let Some(g) = b.lfe_mix
    {
        m[0][n] = g;
        m[1][n] = g;
    }
    let norm = m
        .iter()
        .map(|row| row.iter().map(|v| v.abs()).sum::<f32>())
        .fold(0f32, f32::max);
    if norm > 1.0 {
        for row in m.iter_mut() {
            for v in row.iter_mut() {
                *v /= norm;
            }
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_sizes_follow_table_5_18() {
        // 48 kHz 192 kbit/s: 384 words; 44.1 kHz 640 kbit/s odd: 1394 words
        let h = [0x0B, 0x77, 0, 0, 0x14, 0x40, 0xE0, 0];
        let i = FrameInfo::parse(&h).unwrap();
        assert_eq!((i.frame_bytes, i.sample_rate, i.kbps), (768, 48_000, 192));
        let h = [0x0B, 0x77, 0, 0, 0x65, 0x40, 0x40, 0];
        let i = FrameInfo::parse(&h).unwrap();
        assert_eq!((i.frame_bytes, i.sample_rate), (2788, 44_100));
        assert_eq!(i.layout(), "stereo");
    }

    #[test]
    fn stereo_fold_never_overloads() {
        for acmod in 0..8u8 {
            let order = wave_order(acmod, true);
            assert_eq!(order.len(), NFCHANS[usize::from(acmod)] + 1);
        }
        assert_eq!(location_channels(locations(7, true)), 6);
        // 7.1: 5.1 plus a back pair
        assert_eq!(location_channels(locations(7, true) | (1 << 9)), 8);
    }

    /// A reset (a seek) starts the dither again: the sound after it is the same however
    /// much was decoded before (a player's decoder may have run ahead before the seek).
    #[test]
    fn a_reset_decodes_the_same_whatever_came_before() {
        let data = include_bytes!("../tests/data/c51.ac3");
        let mut fresh = Decoder::new(Options::default());
        let mut want = Vec::new();
        fresh.decode(data, &mut want).unwrap();
        let mut d = Decoder::new(Options::default());
        let mut before = Vec::new();
        d.decode(&data[..data.len() / 2], &mut before).unwrap();
        assert!(!before.is_empty());
        d.reset();
        let mut got = Vec::new();
        d.decode(data, &mut got).unwrap();
        assert_eq!(got, want);
    }

    /// Clearing the dither flags keeps both of an AC-3 frame's CRCs right: the first 5/8
    /// (CRC1 leading it) and the whole frame (CRC2 ending it) leave no remainder.
    #[test]
    fn without_dither_keeps_the_crcs() {
        let data = include_bytes!("../tests/data/c51.ac3");
        let nd = without_dither(data);
        assert_ne!(&nd[..], &data[..]);
        let mut at = 0;
        let mut frames = 0;
        while at + 8 <= nd.len() {
            let n = FrameInfo::parse(&nd[at..]).expect("a frame").frame_bytes;
            let f = &nd[at..at + n];
            let words = n / 2;
            let five_eighths = 2 * ((words >> 1) + (words >> 3));
            assert_eq!(
                bits::crc16(&f[2..five_eighths]),
                0,
                "CRC1 of frame {frames}"
            );
            assert_eq!(bits::crc16(&f[2..]), 0, "CRC2 of frame {frames}");
            at += n;
            frames += 1;
        }
        assert!(frames > 5);
        // The bits before any bytes that leave no remainder: found for every pattern.
        for rest in [&[0u8; 0][..], b"\x01", b"AC-3 frame body"] {
            let c = crc_at_start(rest);
            let mut v = c.to_be_bytes().to_vec();
            v.extend_from_slice(rest);
            assert_eq!(bits::crc16(&v), 0);
        }
    }
}
