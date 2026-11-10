//! GoPro CineForm (`CFHD`), decoded from its tag stream. CineForm is the wavelet codec VC-5
//! (SMPTE ST 2073) grew out of; its codebook 17 is the VC-5 codebook GoPro GPR raws use.
//!
//! A sample is a sequence of big-endian 32-bit segments: a 16-bit tag (negative = optional)
//! and a 16-bit value. Header tags give the transform, the channel count, the picture size,
//! the source precision and the encoded format; then each channel follows: its lowpass band
//! (plain 16-bit values after a `0x0F0F` marker), then each wavelet's highpass bands, smallest
//! first, as run-length codes (run, magnitude, sign bit) up to a band end code. Rows are coded
//! at a pitch of whole multiples of eight values. Magnitudes are expanded (cubic companding in
//! codebook 17, the older two-step expansion in codebook 9) and multiplied by the band's
//! quantisation; a band may be difference-coded along its rows, and large values may come from
//! a peak table.
//!
//! Transforms: spatial (three 2/6 wavelets per channel; the largest is a frame wavelet over
//! the two fields when the picture is interlaced) and field+ (two-frame groups: frame wavelets
//! for both frames, a temporal wavelet between them, spatial wavelets on its sum and
//! difference; the second frame's sample only points back into the group).
//!
//! Encoded formats: YUV 4:2:2 (Y, Cr, Cb channels, 8 to 12 bits), RGB 4:4:4 (G, R, B at 12
//! bits), RGBA 4:4:4:4 (alpha companded) and Bayer (green sum, red and blue differences, green
//! difference; shown through a bilinear demosaic). Refused by name: the field transform without
//! the extra wavelet, YUVA 4:4:4:4, uncompressed samples and band codings other than run
//! lengths.
//!
//! Tag numbers, the codebooks (`tables.rs`), the companding curves, the default prescale tables
//! and the alpha decompanding are from GoPro's CineForm SDK (Apache-2.0 / MIT), see
//! `tables.PROVENANCE.md`; the code is our own.

mod tables;
mod transform;

/// Codebook 17 as (code length in bits, code, run length, magnitude), the band end code
/// (run length 0) last. It is also the VC-5 codebook (SMPTE ST 2073-1 codeset 17) of GoPro
/// GPR raws, which `specular-raw` decodes with it.
pub fn vc5_codebook() -> &'static [(u8, u32, u16, u16)] {
    &tables::CS17
}

use std::fmt;

use rayon::prelude::*;
use transform::{Bands, Scale, sat};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Malformed(String),
    Unsupported(String),
    NeedsMemory(specular_core::NeedsMemory),
}

impl From<specular_core::NeedsMemory> for Error {
    fn from(n: specular_core::NeedsMemory) -> Self {
        Error::NeedsMemory(n)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Malformed(s) => write!(f, "malformed CineForm frame: {s}"),
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::NeedsMemory(n) => write!(f, "CineForm frame: {n}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T, E = Error> = std::result::Result<T, E>;

fn bad(s: impl Into<String>) -> Error {
    Error::Malformed(s.into())
}

fn refuse(s: impl Into<String>) -> Error {
    Error::Unsupported(s.into())
}

/// The encoded formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Yuv422,
    Bayer,
    Rgb444,
    Rgba4444,
    Yuva4444,
}

impl Layout {
    pub fn name(self) -> &'static str {
        match self {
            Layout::Yuv422 => "YUV 4:2:2",
            Layout::Bayer => "Bayer RAW",
            Layout::Rgb444 => "RGB 4:4:4",
            Layout::Rgba4444 => "RGBA 4:4:4:4",
            Layout::Yuva4444 => "YUVA 4:4:4:4",
        }
    }

    fn of(v: u16) -> Option<Layout> {
        Some(match v {
            0 | 1 => Layout::Yuv422,
            2 => Layout::Bayer,
            3 => Layout::Rgb444,
            4 => Layout::Rgba4444,
            5 => Layout::Yuva4444,
            _ => return None,
        })
    }
}

/// The wavelet transforms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transform {
    /// Every sample a picture.
    Spatial,
    /// Two-frame groups (the older field transform).
    Field,
    /// Two-frame groups with a wavelet on the temporal difference.
    FieldPlus,
}

/// What a sample's header says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    pub layout: Layout,
    /// Bits of the source (8, 10 or 12).
    pub precision: u8,
    pub transform: Transform,
    pub interlaced: bool,
    pub channels: u8,
}

impl Info {
    /// Why such a stream can't be decoded, in words.
    pub fn refusal(&self) -> Option<String> {
        match (self.transform, self.layout) {
            (Transform::Field, _) => Some("CineForm field transform".into()),
            (_, Layout::Yuva4444) => Some("CineForm YUVA 4:4:4:4".into()),
            _ => None,
        }
    }

    /// "YUV 4:2:2, 10-bit, interlaced, two-frame groups".
    pub fn describe(&self) -> String {
        let mut s = format!("{}, {}-bit", self.layout.name(), self.precision);
        if self.interlaced {
            s.push_str(", interlaced");
        }
        if self.transform != Transform::Spatial {
            s.push_str(", two-frame groups");
        }
        s
    }
}

/// A decoded picture. YUV 4:2:2: Y, Cb, Cr planes (chroma `ceil(width / 2)` wide). RGB, RGBA
/// and Bayer (demosaiced): R, G, B and, for RGBA, A planes. Samples are `bits` deep.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub layout: Layout,
    pub bits: u8,
    pub planes: Vec<Vec<u16>>,
}

// Tags (the SDK's `codec.h`).
const T_SAMPLE: i32 = 1;
const T_INDEX: i32 = 2;
const T_MARKER: i32 = 4;
const T_TRANSFORM: i32 = 10;
const T_FRAMES: i32 = 11;
const T_CHANNELS: i32 = 12;
const T_WAVELETS: i32 = 13;
const T_WIDTH: i32 = 20;
const T_HEIGHT: i32 = 21;
const T_LOW_W: i32 = 27;
const T_LOW_H: i32 = 28;
const T_LEVELS: i32 = 26;
const T_DEPTH: i32 = 35;
const T_WAVE_NUM: i32 = 38;
const T_HIGH_W: i32 = 41;
const T_HIGH_H: i32 = 42;
const T_BAND: i32 = 48;
const T_BAND_W: i32 = 49;
const T_BAND_H: i32 = 50;
const T_BAND_SUB: i32 = 51;
const T_BAND_CODING: i32 = 52;
const T_BAND_QUANT: i32 = 53;
const T_BAND_HEADER: i32 = 55;
const T_CHANNEL: i32 = 62;
const T_FLAGS: i32 = 68;
const T_PRECISION: i32 = 70;
const T_CODEBOOK: i32 = 72;
const T_PEAK_LEVEL: i32 = 74;
const T_PEAK_LOW: i32 = 75;
const T_PEAK_HIGH: i32 = 76;
const T_PRESCALE: i32 = 83;
const T_FORMAT: i32 = 84;
const T_DISPLAY_H: i32 = 85;

const LOWPASS_MARKER: u16 = 0x0F0F;
const SAMPLE_FRAME: u16 = 1;
const SAMPLE_GROUP: u16 = 2;
const SAMPLE_CHANNEL: u16 = 3;
const SAMPLE_INTRA: u16 = 9;
const MAX_DIM: usize = 16_384;
const MAX_WAVELETS: usize = 8;

/// The sample header (tags before the first channel's data).
#[derive(Clone, Debug, Default)]
struct Header {
    sample: u16,
    transform: u16,
    frames: u16,
    channels: usize,
    wavelets: usize,
    width: usize,
    height: usize,
    display_h: usize,
    precision: u16,
    prescale: Option<[u8; MAX_WAVELETS]>,
    format: u16,
    progressive: bool,
}

impl Header {
    fn info(&self) -> Result<Info> {
        if self.width == 0 || self.height == 0 || self.width > MAX_DIM || self.height > MAX_DIM {
            return Err(bad(format!("picture size {}×{}", self.width, self.height)));
        }
        let layout = Layout::of(self.format)
            .ok_or_else(|| refuse(format!("CineForm encoded format {}", self.format)))?;
        let transform = match self.transform {
            0 => Transform::Spatial,
            1 => Transform::Field,
            2 => Transform::FieldPlus,
            t => return Err(refuse(format!("CineForm transform type {t}"))),
        };
        let precision = match self.precision {
            0 => 8,
            p @ (8 | 10 | 12) => p as u8,
            p => return Err(refuse(format!("CineForm at {p} bits"))),
        };
        let (w, h) = if layout == Layout::Bayer {
            (2 * self.width, 2 * self.display_rows())
        } else {
            (self.width, self.display_rows())
        };
        Ok(Info {
            width: w as u32,
            height: h as u32,
            layout,
            precision,
            transform,
            interlaced: !self.progressive,
            channels: self.channels as u8,
        })
    }

    fn display_rows(&self) -> usize {
        if self.display_h > 0 && self.display_h <= self.height {
            self.display_h
        } else {
            self.height
        }
    }

    /// The prescale shift of each wavelet: the stream's table, else the defaults.
    fn prescale(&self, transform: Transform, precision: u8) -> [u8; MAX_WAVELETS] {
        if let Some(p) = self.prescale {
            return p;
        }
        match (precision, transform) {
            (10, Transform::Spatial) => [0, 2, 0, 0, 0, 0, 0, 0],
            (10, _) => [0, 0, 0, 0, 2, 0, 0, 0],
            (12, Transform::Spatial) => [0, 2, 2, 0, 0, 0, 0, 0],
            (12, _) => [0, 0, 0, 2, 2, 2, 0, 0],
            _ => [0; MAX_WAVELETS],
        }
    }
}

fn segment(d: &[u8], pos: usize) -> Option<(i32, u16)> {
    let s = d.get(pos..pos + 4)?;
    Some((
        i32::from(i16::from_be_bytes([s[0], s[1]])),
        u16::from_be_bytes([s[2], s[3]]),
    ))
}

/// Reads the header of a sample (up to the first channel's lowpass band).
fn header(d: &[u8]) -> Result<(Header, usize)> {
    let mut h = Header {
        frames: 1,
        format: 1,
        ..Header::default()
    };
    let mut pos = 0;
    let mut start = None;
    let (mut lw, mut lh, mut levels) = (0usize, 0usize, 0u32);
    while let Some((tag, v)) = segment(d, pos) {
        let t = tag.abs();
        if start.is_none() && matches!(t, T_MARKER | T_LOW_W | T_LOW_H | T_LEVELS) {
            start = Some(pos);
        }
        match t {
            T_SAMPLE => h.sample = v,
            T_INDEX => {
                pos += 4 * usize::from(v);
            }
            T_MARKER if v == LOWPASS_MARKER => {
                // Groups may leave out the frame size: the lowpass band is the frame scaled
                // down once per level (the temporal level keeps the size).
                let shift = levels.saturating_sub(u32::from(h.transform != 0)).min(8);
                if h.width == 0 {
                    h.width = lw << shift;
                }
                if h.height == 0 {
                    h.height = lh << shift;
                }
                return Ok((h, start.unwrap_or(pos)));
            }
            T_MARKER => {}
            T_LOW_W => lw = usize::from(v),
            T_LOW_H => lh = usize::from(v),
            T_LEVELS => levels = u32::from(v),
            T_TRANSFORM => h.transform = v,
            T_FRAMES => h.frames = v,
            T_CHANNELS => h.channels = usize::from(v),
            T_WAVELETS => h.wavelets = usize::from(v),
            T_WIDTH => h.width = usize::from(v),
            T_HEIGHT => h.height = usize::from(v),
            T_DISPLAY_H => h.display_h = usize::from(v),
            T_PRECISION => h.precision = v,
            T_FORMAT => h.format = v,
            T_FLAGS => h.progressive = v & 1 != 0,
            T_PRESCALE => {
                h.prescale = Some(std::array::from_fn(|i| ((v >> (14 - 2 * i)) & 3) as u8));
            }
            _ => {
                if let Some(n) = chunk_words(t, v) {
                    pos += 4 * n;
                }
            }
        }
        pos += 4;
    }
    if h.sample == SAMPLE_FRAME {
        return Ok((h, pos));
    }
    Err(bad("no channel data"))
}

/// Words a sized chunk tag skips.
fn chunk_words(t: i32, v: u16) -> Option<usize> {
    if t & 0x4000 == 0 {
        return None;
    }
    let mut n = usize::from(v);
    if t & 0x2000 != 0 {
        n += ((t & 0xFF) as usize) << 16;
    }
    Some(n)
}

/// What a sample says about the stream (a first sample of a group or an intra-frame sample).
pub fn probe(sample: &[u8]) -> Result<Info> {
    let (h, _) = header(sample)?;
    if h.sample == SAMPLE_FRAME {
        return Err(bad("the second frame of a group has no header"));
    }
    h.info()
}

/// Why a stream starting with `sample` can't be decoded, in words.
pub fn refusal(sample: &[u8]) -> Option<String> {
    match probe(sample) {
        Ok(i) => i.refusal(),
        Err(Error::Unsupported(m)) => Some(m),
        Err(_) => None,
    }
}

/// Bits from big-endian bytes.
struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
    acc: u64,
    n: u32,
    over: bool,
}

impl<'a> Bits<'a> {
    fn at(d: &'a [u8], pos: usize) -> Self {
        Bits {
            d,
            pos,
            acc: 0,
            n: 0,
            over: false,
        }
    }

    #[inline(always)]
    fn peek(&mut self, k: u32) -> u32 {
        while self.n < k {
            let b = match self.d.get(self.pos) {
                Some(&b) => b,
                None => {
                    self.over = true;
                    0
                }
            };
            self.pos += 1;
            self.acc |= u64::from(b) << (56 - self.n);
            self.n += 8;
        }
        (self.acc >> (64 - k)) as u32
    }

    #[inline(always)]
    fn skip(&mut self, k: u32) {
        self.acc <<= k;
        self.n -= k;
    }

    /// The byte position after the bits read so far, rounded up to a whole segment.
    fn end(&self) -> usize {
        let used = self.pos * 8 - self.n as usize;
        used.div_ceil(32) * 4
    }
}

/// A codebook: codes up to [`FAST`] bits through a table, longer ones by search. Run length 0
/// is the band end code.
struct Codebook {
    fast: Vec<(u8, u16, u16)>,
    long: Vec<(u8, u32, u16, u16)>,
}

const FAST: u32 = 12;

impl Codebook {
    fn new(rows: &[(u8, u32, u16, u16)]) -> Codebook {
        let mut fast = vec![(0u8, 0u16, 0u16); 1 << FAST];
        let mut long = Vec::new();
        for &(len, code, run, mag) in rows {
            if u32::from(len) <= FAST {
                let first = (code << (FAST - u32::from(len))) as usize;
                for e in &mut fast[first..first + (1 << (FAST - u32::from(len)))] {
                    *e = (len, run, mag);
                }
            } else {
                long.push((len, code, run, mag));
            }
        }
        long.sort_by_key(|e| e.0);
        Codebook { fast, long }
    }

    #[inline(always)]
    fn next(&self, br: &mut Bits) -> Option<(u16, u16)> {
        let (len, run, mag) = self.fast[br.peek(FAST) as usize];
        if len != 0 {
            br.skip(u32::from(len));
            return Some((run, mag));
        }
        for &(len, code, run, mag) in &self.long {
            if br.peek(u32::from(len)) == code {
                br.skip(u32::from(len));
                return Some((run, mag));
            }
        }
        None
    }
}

fn books() -> &'static [Codebook; 2] {
    static B: std::sync::OnceLock<[Codebook; 2]> = std::sync::OnceLock::new();
    B.get_or_init(|| [Codebook::new(&tables::CS9), Codebook::new(&tables::CS17)])
}

/// A coded magnitude expanded by the codebook's companding.
fn expand(m: i32, codebook: u16) -> i32 {
    match codebook {
        0 => {
            let mut m = m;
            if m >= 40 {
                if m >= 54 {
                    m = ((m - 54) << 2) + 54;
                }
                m = ((m - 40) << 2) + 40;
            }
            m
        }
        1 => m + ((i64::from(m).pow(3) * 768) >> 24) as i32,
        _ => m,
    }
}

/// A band's coding state.
struct BandCoding<'a> {
    codebook: u16,
    difference: bool,
    quant: i32,
    /// Values above this (after companding, before quantisation) come from the peak table.
    peaks: Option<(i32, &'a [u8])>,
}

/// Decodes a run-length band of `w`×`h` from byte `pos`; gives the values and the byte
/// position after the band (a whole segment).
fn band(d: &[u8], pos: usize, w: usize, h: usize, c: &BandCoding) -> Result<(Vec<i16>, usize)> {
    let book = &books()[usize::from(c.codebook.min(1))];
    let pitch = w.div_ceil(8) * 8;
    let total = pitch * h;
    let mut vals = vec![0i16; w * h];
    let mut br = Bits::at(d, pos);
    let mut i = 0usize;
    let mut peak = 0usize;
    loop {
        let (run, mag) = book.next(&mut br).ok_or_else(|| bad("unknown code"))?;
        if run == 0 {
            break;
        }
        let mut v = expand(i32::from(mag), c.codebook);
        if mag != 0 {
            if br.peek(1) == 1 {
                v = -v;
            }
            br.skip(1);
        }
        if let Some((level, table)) = c.peaks
            && v.abs() > level
        {
            let p = table
                .get(2 * peak..2 * peak + 2)
                .map_or(0, |b| i32::from(i16::from_le_bytes([b[0], b[1]])));
            peak += 1;
            v = p / c.quant.max(1);
        }
        let q = sat(v.saturating_mul(c.quant));
        if q != 0 {
            for k in i..(i + usize::from(run)).min(total) {
                let (y, x) = (k / pitch, k % pitch);
                if x < w {
                    vals[y * w + x] = q;
                }
            }
        }
        i += usize::from(run);
        if br.over {
            return Err(bad("band runs past the data"));
        }
    }
    if c.difference {
        for row in vals.chunks_mut(w.max(1)) {
            for x in 1..row.len() {
                row[x] = row[x].wrapping_add(row[x - 1]);
            }
        }
    }
    Ok((vals, br.end()))
}

/// One wavelet's bands as they arrive.
#[derive(Default)]
struct Wavelet {
    w: usize,
    h: usize,
    bands: [Option<Vec<i16>>; 4],
}

#[derive(Default)]
struct Channel {
    lowpass: Option<(usize, usize, Vec<i16>)>,
    wavelets: [Option<Wavelet>; MAX_WAVELETS],
}

/// Parses every channel of a sample.
fn parse(d: &[u8], h: &Header, mut pos: usize) -> Result<Vec<Channel>> {
    if h.channels == 0 || h.channels > 4 {
        return Err(bad(format!("{} channels", h.channels)));
    }
    let mut chans: Vec<Channel> = (0..h.channels).map(|_| Channel::default()).collect();
    let mut cur = 0usize;
    let (mut lw, mut lh, mut depth) = (0usize, 0usize, 16u16);
    let mut wnum = 0usize;
    let (mut band_no, mut bw, mut bh) = (0usize, 0usize, 0usize);
    let (mut coding, mut quant, mut codebook) = (3u16, 1i32, 0u16);
    let mut peak_off = 0usize;
    let mut peak_base = 0usize;
    let mut peak_level = 0i32;
    let mut sample = 0u16;
    let mut subband = 0u16;
    while let Some((tag, v)) = segment(d, pos) {
        pos += 4;
        let t = tag.abs();
        match t {
            T_SAMPLE => sample = v,
            T_CHANNEL if sample == SAMPLE_CHANNEL => {
                cur = usize::from(v);
                if cur >= chans.len() {
                    return Err(bad("channel out of range"));
                }
            }
            T_INDEX => pos += 4 * usize::from(v),
            T_LOW_W => lw = usize::from(v),
            T_LOW_H => lh = usize::from(v),
            T_DEPTH => depth = v,
            T_MARKER if v == LOWPASS_MARKER => {
                if depth != 16 {
                    return Err(refuse(format!("CineForm lowpass band at {depth} bits")));
                }
                if lw == 0 || lh == 0 || lw > MAX_DIM || lh > MAX_DIM {
                    return Err(bad("lowpass band size"));
                }
                let n = lw * lh;
                let raw = d.get(pos..pos + 2 * n).ok_or_else(|| bad("lowpass band"))?;
                let vals = raw
                    .chunks_exact(2)
                    .map(|b| i16::from_be_bytes([b[0], b[1]]))
                    .collect();
                chans[cur].lowpass = Some((lw, lh, vals));
                pos += (2 * n).div_ceil(4) * 4;
            }
            T_WAVE_NUM => {
                wnum = usize::from(v);
                if wnum == 0 || wnum > MAX_WAVELETS {
                    return Err(bad("wavelet number"));
                }
                if chans[cur].wavelets[wnum - 1].is_none() {
                    chans[cur].wavelets[wnum - 1] = Some(Wavelet::default());
                }
            }
            T_HIGH_W | T_HIGH_H if wnum > 0 => {
                if let Some(wv) = chans[cur].wavelets[wnum - 1].as_mut() {
                    let n = usize::from(v).min(MAX_DIM);
                    if t == T_HIGH_W { wv.w = n } else { wv.h = n }
                }
            }
            T_BAND => band_no = usize::from(v),
            T_BAND_W => bw = usize::from(v),
            T_BAND_H => bh = usize::from(v),
            T_BAND_SUB => subband = v,
            T_BAND_CODING => coding = v,
            T_BAND_QUANT => quant = i32::from(v),
            T_CODEBOOK => codebook = v,
            T_PEAK_LOW => {
                peak_off = (peak_off & !0xFFFF) | usize::from(v);
                peak_base = pos;
            }
            T_PEAK_HIGH => peak_off = (peak_off & 0xFFFF) | usize::from(v) << 16,
            T_PEAK_LEVEL => peak_level = i32::from(v),
            // Subband 255 is an empty placeholder (the temporal difference of a field+
            // group, which its own wavelet carries).
            T_BAND_HEADER if subband == 255 => {}
            T_BAND_HEADER => {
                if coding != 3 {
                    return Err(refuse(format!("CineForm band coding {coding}")));
                }
                if bw == 0 || bh == 0 || bw > MAX_DIM || bh > MAX_DIM || band_no > 3 {
                    return Err(bad("band size"));
                }
                if (codebook & 15) > 2 {
                    return Err(refuse(format!("CineForm codebook {}", codebook & 15)));
                }
                let peaks = (peak_level > 0).then(|| {
                    let at = peak_base.saturating_add(peak_off).min(d.len());
                    let level = if quant > 1 {
                        peak_level / quant
                    } else {
                        peak_level
                    };
                    (level, &d[at..])
                });
                let c = BandCoding {
                    codebook: codebook & 15,
                    difference: codebook >> 4 & 1 == 1,
                    quant,
                    peaks,
                };
                let (vals, end) = band(d, pos, bw, bh, &c).map_err(|e| match e {
                    Error::Malformed(m) => bad(format!(
                        "{m} (channel {cur}, wavelet {wnum}, band {band_no} at {pos})"
                    )),
                    e => e,
                })?;
                pos = end;
                // The codebook, difference coding and peak level hold for one band.
                peak_level = 0;
                codebook = 0;
                if wnum == 0 {
                    return Err(bad("band before its wavelet"));
                }
                if let Some(wv) = chans[cur].wavelets[wnum - 1].as_mut() {
                    wv.w = bw;
                    wv.h = bh;
                    wv.bands[band_no] = Some(vals);
                }
            }
            _ => {
                if let Some(n) = chunk_words(t, v) {
                    pos += 4 * n;
                }
            }
        }
    }
    Ok(chans)
}

/// The band `i` of a wavelet, zeros when missing (a damaged sample).
fn get(wv: &Wavelet, i: usize) -> std::borrow::Cow<'_, [i16]> {
    match &wv.bands[i] {
        Some(b) if b.len() == wv.w * wv.h => std::borrow::Cow::Borrowed(b),
        _ => std::borrow::Cow::Owned(vec![0; wv.w * wv.h]),
    }
}

fn scale(prescale: u8, precision: u8) -> Scale {
    match (prescale >= 2, precision >= 10) {
        (false, _) => Scale::Half,
        (true, false) => Scale::Keep,
        (true, true) => Scale::Double,
    }
}

/// Inverts wavelet `k` (with `low` as its lowpass band) into `ow`×`oh`.
fn invert(wv: &Wavelet, low: &[i16], ow: usize, oh: usize, sc: Scale) -> Vec<i16> {
    let (b1, b2, b3) = (get(wv, 1), get(wv, 2), get(wv, 3));
    let s = Bands {
        w: wv.w,
        h: wv.h,
        b: [low, &b1, &b2, &b3],
    };
    transform::spatial(&s, ow, oh, sc)
}

fn invert_frame(wv: &Wavelet, low: &[i16], ow: usize, oh: usize, max: i32) -> Vec<i16> {
    let (b1, b2, b3) = (get(wv, 1), get(wv, 2), get(wv, 3));
    let s = Bands {
        w: wv.w,
        h: wv.h,
        b: [low, &b1, &b2, &b3],
    };
    transform::frame(&s, ow, oh, max)
}

/// A lowpass band resized to `w`×`h` (cropped, or padded by repetition).
fn fit(v: Vec<i16>, vw: usize, vh: usize, w: usize, h: usize) -> Vec<i16> {
    if vw == w && vh == h {
        return v;
    }
    let mut out = vec![0i16; w * h];
    if vw == 0 || vh == 0 {
        return out;
    }
    for y in 0..h {
        for x in 0..w {
            out[y * w + x] = v[y.min(vh - 1) * vw + x.min(vw - 1)];
        }
    }
    out
}

/// The wavelet at index `i`, or an error.
fn wavelet(ch: &Channel, i: usize) -> Result<&Wavelet> {
    ch.wavelets
        .get(i)
        .and_then(|w| w.as_ref())
        .filter(|w| w.w > 0 && w.h > 0)
        .ok_or_else(|| bad(format!("wavelet {} missing", i + 1)))
}

/// A spatial-transform channel: the picture `cw`×`ch` (samples, not clipped).
fn spatial_channel(
    c: &Channel,
    n: usize,
    ps: &[u8; MAX_WAVELETS],
    precision: u8,
    progressive: bool,
    cw: usize,
    chh: usize,
) -> Result<Vec<i16>> {
    if !(1..=MAX_WAVELETS).contains(&n) {
        return Err(bad("wavelet count"));
    }
    let (lw, lh, low) = c.lowpass.clone().ok_or_else(|| bad("no lowpass band"))?;
    let top = wavelet(c, n - 1)?;
    let mut low = fit(low, lw, lh, top.w, top.h);
    for k in (1..n).rev() {
        let wv = wavelet(c, k)?;
        let below = wavelet(c, k - 1)?;
        low = invert(wv, &low, below.w, below.h, scale(ps[k], precision));
    }
    let w0 = wavelet(c, 0)?;
    let max = (1i32 << precision) - 1;
    Ok(if progressive {
        invert(w0, &low, cw, chh, scale(ps[0], precision))
    } else {
        invert_frame(w0, &low, cw, chh, max)
    })
}

/// A field+ channel: both frames of the group, `cw`×`ch` each.
fn fieldplus_channel(
    c: &Channel,
    ps: &[u8; MAX_WAVELETS],
    precision: u8,
    cw: usize,
    chh: usize,
) -> Result<[Vec<i16>; 2]> {
    let (lw, lh, low) = c.lowpass.clone().ok_or_else(|| bad("no lowpass band"))?;
    let w5 = wavelet(c, 5)?;
    let w4 = wavelet(c, 4)?;
    let w3 = wavelet(c, 3)?;
    let w2 = wavelet(c, 2)?;
    let (f0, f1) = (wavelet(c, 0)?, wavelet(c, 1)?);
    let low = fit(low, lw, lh, w5.w, w5.h);
    let l4 = invert(w5, &low, w4.w, w4.h, scale(ps[5], precision));
    // The temporal sum and difference, at the temporal wavelet's size.
    let sum = invert(w4, &l4, w2.w, w2.h, scale(ps[4], precision));
    let diff_low = get(w3, 0);
    let p3 = if precision >= 10 { ps[3] } else { 0 };
    let diff = invert(w3, &diff_low, w2.w, w2.h, scale(p3, precision));
    let (a, b) = transform::temporal(&sum, &diff, w2.w, precision == 8);
    let max = (1i32 << precision) - 1;
    let a = fit(a, w2.w, w2.h, f0.w, f0.h);
    let b = fit(b, w2.w, w2.h, f1.w, f1.h);
    Ok([
        invert_frame(f0, &a, cw, chh, max),
        invert_frame(f1, &b, cw, chh, max),
    ])
}

/// The size of channel `i`.
fn channel_size(h: &Header, layout: Layout, i: usize) -> (usize, usize) {
    if layout == Layout::Yuv422 && i > 0 {
        (h.width.div_ceil(2), h.height)
    } else {
        (h.width, h.height)
    }
}

/// Decoded channels to a picture.
fn assemble(h: &Header, info: &Info, chans: Vec<Vec<i16>>) -> Result<Picture> {
    let rows = h.display_rows();
    let max = (1i32 << info.precision) - 1;
    let clip = |v: &[i16], w: usize| -> Vec<u16> {
        v[..w * rows]
            .iter()
            .map(|&s| i32::from(s).clamp(0, max) as u16)
            .collect()
    };
    let (w, _) = (h.width, h.height);
    let planes = match info.layout {
        Layout::Yuv422 => {
            if chans.len() < 3 {
                return Err(bad("YUV needs three channels"));
            }
            let cw = w.div_ceil(2);
            vec![clip(&chans[0], w), clip(&chans[2], cw), clip(&chans[1], cw)]
        }
        Layout::Rgb444 | Layout::Rgba4444 => {
            if chans.len() < 3 {
                return Err(bad("RGB needs three channels"));
            }
            let mut p = vec![clip(&chans[1], w), clip(&chans[0], w), clip(&chans[2], w)];
            if info.layout == Layout::Rgba4444 {
                let a = chans
                    .get(3)
                    .ok_or_else(|| bad("RGBA needs four channels"))?;
                p.push(
                    a[..w * rows]
                        .iter()
                        .map(|&s| (((i32::from(s) - 256) * 9400) >> 13).clamp(0, 4095) as u16)
                        .collect(),
                );
            }
            p
        }
        Layout::Bayer => {
            if chans.len() < 4 {
                return Err(bad("Bayer needs four channels"));
            }
            demosaic(&chans, w, rows)
        }
        Layout::Yuva4444 => return Err(refuse("CineForm YUVA 4:4:4:4")),
    };
    Ok(Picture {
        width: info.width,
        height: info.height,
        layout: info.layout,
        bits: if info.layout == Layout::Bayer {
            12
        } else {
            info.precision
        },
        planes,
    })
}

/// The Bayer mosaic (RGGB) from the four component arrays (`cw`×`ch` each), demosaiced
/// bilinearly to full-size R, G, B planes of 12 bits.
fn demosaic(c: &[Vec<i16>], cw: usize, ch: usize) -> Vec<Vec<u16>> {
    let (w, h) = (2 * cw, 2 * ch);
    let mut m = vec![0u16; w * h];
    for y in 0..ch {
        for x in 0..cw {
            let i = y * cw + x;
            let gs = i32::from(c[0][i]);
            let rg = i32::from(c[1][i]) - 2048;
            let bg = i32::from(c[2][i]) - 2048;
            let gd = i32::from(c[3][i]) - 2048;
            let v = |s: i32| s.clamp(0, 4095) as u16;
            m[2 * y * w + 2 * x] = v((rg << 1) + gs);
            m[2 * y * w + 2 * x + 1] = v(gs + gd);
            m[(2 * y + 1) * w + 2 * x] = v(gs - gd);
            m[(2 * y + 1) * w + 2 * x + 1] = v((bg << 1) + gs);
        }
    }
    // Which colour a site holds: 0 red, 1 green, 2 blue.
    let site = |x: usize, y: usize| match (y & 1, x & 1) {
        (0, 0) => 0,
        (1, 1) => 2,
        _ => 1,
    };
    let mut planes = vec![vec![0u16; w * h]; 3];
    let [r, g, b] = &mut planes[..] else {
        return planes;
    };
    let px = |x: isize, y: isize| -> u32 {
        let x = x.clamp(0, w as isize - 1) as usize;
        let y = y.clamp(0, h as isize - 1) as usize;
        u32::from(m[y * w + x])
    };
    let rows = r
        .par_chunks_mut(w)
        .zip(g.par_chunks_mut(w))
        .zip(b.par_chunks_mut(w));
    rows.enumerate().for_each(|(y, ((rr, gg), bb))| {
        let yi = y as isize;
        for x in 0..w {
            let xi = x as isize;
            let here = px(xi, yi);
            let cross = (px(xi - 1, yi) + px(xi + 1, yi) + px(xi, yi - 1) + px(xi, yi + 1)) / 4;
            let diag =
                (px(xi - 1, yi - 1) + px(xi + 1, yi - 1) + px(xi - 1, yi + 1) + px(xi + 1, yi + 1))
                    / 4;
            let horiz = (px(xi - 1, yi) + px(xi + 1, yi)) / 2;
            let vert = (px(xi, yi - 1) + px(xi, yi + 1)) / 2;
            let (rv, gv, bv) = match site(x, y) {
                0 => (here, cross, diag),
                2 => (diag, cross, here),
                // Green on a red row: red left and right, blue above and below.
                _ if y & 1 == 0 => (horiz, here, vert),
                _ => (vert, here, horiz),
            };
            rr[x] = rv as u16;
            gg[x] = gv as u16;
            bb[x] = bv as u16;
        }
    });
    planes
}

/// A stream's decoder: the second frame of the last group waits here for its sample.
#[derive(Default)]
pub struct Decoder {
    pending: Option<Picture>,
    info: Option<Info>,
}

impl Decoder {
    pub fn new() -> Decoder {
        Decoder::default()
    }

    /// Forgets the waiting frame (decoding goes on from a group's first sample).
    pub fn reset(&mut self) {
        self.pending = None;
    }

    /// What the last header said.
    pub fn info(&self) -> Option<&Info> {
        self.info.as_ref()
    }

    /// Decodes one sample: an intra frame, a group's first frame, or the group's second frame.
    pub fn decode(&mut self, sample: &[u8]) -> Result<Picture> {
        let (h, pos) = header(sample)?;
        if h.sample == SAMPLE_FRAME {
            return self
                .pending
                .take()
                .ok_or_else(|| bad("second frame without its group"));
        }
        if h.sample != SAMPLE_INTRA && h.sample != SAMPLE_GROUP && h.sample != 0 {
            return Err(refuse(format!("CineForm sample type {}", h.sample)));
        }
        let info = h.info()?;
        if let Some(why) = info.refusal() {
            return Err(refuse(why));
        }
        let px = (h.width * h.height) as u64;
        let _held = specular_core::budget::reserve(px * 2 * 16)?;
        let chans = parse(sample, &h, pos)?;
        let ps = h.prescale(info.transform, info.precision);
        let n = if h.wavelets > 0 { h.wavelets } else { 3 };
        self.info = Some(info.clone());
        match info.transform {
            Transform::Spatial => {
                let out: Vec<Vec<i16>> = chans
                    .par_iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let (cw, chh) = channel_size(&h, info.layout, i);
                        spatial_channel(c, n, &ps, info.precision, h.progressive, cw, chh)
                    })
                    .collect::<Result<_>>()?;
                self.pending = None;
                assemble(&h, &info, out)
            }
            Transform::FieldPlus => {
                let out: Vec<[Vec<i16>; 2]> = chans
                    .par_iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let (cw, chh) = channel_size(&h, info.layout, i);
                        fieldplus_channel(c, &ps, info.precision, cw, chh)
                    })
                    .collect::<Result<_>>()?;
                let (first, second): (Vec<_>, Vec<_>) =
                    out.into_iter().map(|[a, b]| (a, b)).unzip();
                self.pending = Some(assemble(&h, &info, second)?);
                assemble(&h, &info, first)
            }
            Transform::Field => Err(refuse("CineForm field transform")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codebooks_are_prefix_free() {
        for rows in [&tables::CS9[..], &tables::CS17[..]] {
            for (i, a) in rows.iter().enumerate() {
                for b in &rows[i + 1..] {
                    let (s, l) = if a.0 <= b.0 { (a, b) } else { (b, a) };
                    assert_ne!(s.1, l.1 >> (l.0 - s.0), "{a:?} prefixes {b:?}");
                }
            }
            assert_eq!(rows.last().unwrap().2, 0);
        }
    }

    #[test]
    fn companding() {
        assert_eq!(expand(39, 0), 39);
        assert_eq!(expand(41, 0), 44);
        assert_eq!(expand(54, 0), 96);
        assert_eq!(expand(55, 0), 112);
        assert_eq!(
            expand(255, 1),
            255 + (255i64 * 255 * 255 * 768 / (1 << 24)) as i32
        );
        assert_eq!(expand(300, 2), 300);
    }

    #[test]
    fn hostile_samples() {
        assert!(probe(&[]).is_err());
        assert!(Decoder::new().decode(&[0, 1, 0, 9]).is_err());
        // A header claiming a huge picture, then nothing.
        let mut s = Vec::new();
        for (t, v) in [(1i16, 9u16), (12, 3), (20, 16000), (21, 16000), (4, 0x1A4A)] {
            s.extend_from_slice(&t.to_be_bytes());
            s.extend_from_slice(&v.to_be_bytes());
        }
        assert!(Decoder::new().decode(&s).is_err());
        // Random bytes after a valid-looking header.
        let mut x = 0x1234_5678u32;
        for _ in 0..4096 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            s.push(x as u8);
        }
        let _ = Decoder::new().decode(&s);
        assert_eq!(
            refusal(&{
                let mut v = Vec::new();
                for (t, val) in [
                    (1i16, 2u16),
                    (10, 1),
                    (12, 3),
                    (20, 64),
                    (21, 48),
                    (4, 0x0F0F),
                ] {
                    v.extend_from_slice(&t.to_be_bytes());
                    v.extend_from_slice(&val.to_be_bytes());
                }
                v
            })
            .as_deref(),
            Some("CineForm field transform")
        );
    }
}
