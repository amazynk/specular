//! Specular's own Vorbis I audio decoder (the Xiph.Org Vorbis I specification): the sound of
//! WebM, Matroska and Ogg movies.
//!
//! - Every Vorbis I feature: all codebook kinds (ordered, sparse, lookup types 0, 1 and 2),
//!   floor 0 (LSP) and floor 1, residue 0, 1 and 2, channel coupling, submaps, short and long
//!   blocks with their mixed windows.
//! - 1 to 255 channels in the Vorbis channel order, any sample rate.
//! - Floor 0 follows the specification but has no encoder left to test it against.
//! - Refused in words: reserved codebook, floor, residue, mapping, window and transform types
//!   (no known stream uses them) and setups whose codebooks would need more than about
//!   64 MB.
//! - The first packet after [`Decoder::new`] or [`Decoder::reset`] only primes the overlap;
//!   trimming the last packet to the container's end position is the caller's job.
//!
//! Input is the three header packets (or Matroska's laced `CodecPrivate`, see
//! [`split_xiph_laced`]) and then the audio packets one by one. Output is interleaved `f32`.
#![deny(unsafe_code)]
#![allow(clippy::needless_range_loop)]

mod bits;
mod codebook;
mod floor;
mod mdct;
mod residue;
mod tables;

use bits::{BitReader, ilog};
use codebook::Codebook;
use floor::{FLOOR0_MAX_ORDER, FLOOR1_MAX_VALUES, Floor};
use mdct::Imdct;
use residue::{Bundle, Residue};

/// Why a stream or packet can't be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A valid but unsupported variant, named in plain words.
    Unsupported(String),
    /// Damaged data.
    Malformed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Malformed(s) => write!(f, "damaged Vorbis audio: {s}"),
        }
    }
}

impl std::error::Error for Error {}

fn malformed(s: &str) -> Error {
    Error::Malformed(s.to_string())
}

/// Splits Matroska's CodecPrivate (Xiph lacing of the three headers) into the headers.
pub fn split_xiph_laced(private: &[u8]) -> Option<[&[u8]; 3]> {
    let (&count, mut rest) = private.split_first()?;
    if count != 2 {
        return None;
    }
    let mut sizes = [0usize; 2];
    for s in &mut sizes {
        loop {
            let (&b, r) = rest.split_first()?;
            rest = r;
            *s += usize::from(b);
            if b != 255 {
                break;
            }
        }
    }
    if sizes[0].checked_add(sizes[1])? > rest.len() {
        return None;
    }
    let (a, rest) = rest.split_at(sizes[0]);
    let (b, c) = rest.split_at(sizes[1]);
    Some([a, b, c])
}

/// Checks a header packet's type byte and "vorbis" signature; returns the body.
fn header_body<'a>(packet: &'a [u8], kind: u8, name: &str) -> Result<&'a [u8], Error> {
    match packet {
        [k, b'v', b'o', b'r', b'b', b'i', b's', body @ ..] if *k == kind => Ok(body),
        _ => Err(malformed(&format!("not a Vorbis {name} header"))),
    }
}

/// The identification header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ident {
    pub channels: u8,
    pub sample_rate: u32,
    pub bitrate_nominal: i32,
    /// The short and long block sizes.
    pub blocksize: [u16; 2],
}

impl Ident {
    /// Parses the identification header packet.
    pub fn parse(packet: &[u8]) -> Result<Ident, Error> {
        let body = header_body(packet, 1, "identification")?;
        let mut br = BitReader::new(body);
        let eop = || malformed("the identification header is cut short");
        let version = br.read(32).ok_or_else(eop)?;
        if version != 0 {
            return Err(Error::Unsupported(format!("Vorbis version {version}")));
        }
        let channels = br.read(8).ok_or_else(eop)? as u8;
        let sample_rate = br.read(32).ok_or_else(eop)?;
        br.read(32).ok_or_else(eop)?;
        let bitrate_nominal = br.read(32).ok_or_else(eop)? as i32;
        br.read(32).ok_or_else(eop)?;
        let b0 = br.read(4).ok_or_else(eop)?;
        let b1 = br.read(4).ok_or_else(eop)?;
        let framing = br.bit().ok_or_else(eop)?;
        if channels == 0 || sample_rate == 0 {
            return Err(malformed("no channels or no sample rate"));
        }
        if !(6..=13).contains(&b0) || !(6..=13).contains(&b1) || b0 > b1 {
            return Err(malformed("bad block sizes"));
        }
        if !framing {
            return Err(malformed("the identification header has no framing bit"));
        }
        Ok(Ident {
            channels,
            sample_rate,
            bitrate_nominal,
            blocksize: [1 << b0, 1 << b1],
        })
    }
}

struct Mapping {
    /// (magnitude, angle) channel pairs.
    coupling: Vec<(u8, u8)>,
    /// The submap of each channel.
    mux: Vec<u8>,
    /// (floor, residue) of each submap.
    submaps: Vec<(u8, u8)>,
}

struct Mode {
    long: bool,
    mapping: u8,
}

/// Codebook entries plus VQ scalars all books together may hold: at most about 64 MB
/// (13 bytes per entry, 4 per scalar). Real streams use well under 1% of it.
const CODEBOOK_BUDGET: u64 = 1 << 22;

/// A Vorbis I decoder.
pub struct Decoder {
    ident: Ident,
    books: Vec<Codebook>,
    floors: Vec<Floor>,
    residues: Vec<Residue>,
    mappings: Vec<Mapping>,
    modes: Vec<Mode>,
    mode_bits: u32,
    imdct: [Imdct; 2],
    /// Window slopes for the short and long half blocks, rising.
    slopes: [Vec<f32>; 2],
    /// The previous block's size; None after new/reset.
    prev_n: Option<usize>,
    /// Per channel, the previous block's windowed right half (bs1/2 stride).
    tail: Vec<f32>,
    /// Per channel spectra (bs1/2 stride).
    spec: Vec<f32>,
    /// Scratch: one block's DCT-IV output and its windowed left half.
    u: Vec<f32>,
    left: Vec<f32>,
    floor1_y: Vec<[i32; FLOOR1_MAX_VALUES]>,
    floor0: Vec<(u64, [f32; FLOOR0_MAX_ORDER])>,
    floor_used: Vec<bool>,
    no_residue: Vec<bool>,
    classes: Vec<u8>,
}

impl Decoder {
    /// From the identification and setup header packets (the comment header is not needed).
    pub fn new(ident: &[u8], setup: &[u8]) -> Result<Decoder, Error> {
        let id = Ident::parse(ident)?;
        let body = header_body(setup, 5, "setup")?;
        let mut br = BitReader::new(body);
        let eop = || malformed("the setup header ends early");
        let channels = usize::from(id.channels);
        let halves = [
            usize::from(id.blocksize[0]) / 2,
            usize::from(id.blocksize[1]) / 2,
        ];

        let mut budget = CODEBOOK_BUDGET;
        let count = br.read(8).ok_or_else(eop)? + 1;
        let mut books = Vec::with_capacity(count as usize);
        for _ in 0..count {
            books.push(Codebook::parse(&mut br, &mut budget)?);
        }

        let times = br.read(6).ok_or_else(eop)? + 1;
        for _ in 0..times {
            if br.read(16).ok_or_else(eop)? != 0 {
                return Err(Error::Unsupported(
                    "Vorbis with a reserved time transform".to_string(),
                ));
            }
        }

        let count = br.read(6).ok_or_else(eop)? + 1;
        let mut floors = Vec::with_capacity(count as usize);
        for _ in 0..count {
            floors.push(Floor::parse(&mut br, &books, halves)?);
        }

        let count = br.read(6).ok_or_else(eop)? + 1;
        let mut residues = Vec::with_capacity(count as usize);
        for _ in 0..count {
            residues.push(Residue::parse(&mut br, &books)?);
        }

        let count = br.read(6).ok_or_else(eop)? + 1;
        let mut mappings = Vec::with_capacity(count as usize);
        for _ in 0..count {
            if br.read(16).ok_or_else(eop)? != 0 {
                return Err(Error::Unsupported(
                    "Vorbis with a reserved channel mapping type".to_string(),
                ));
            }
            let nsub = if br.bit().ok_or_else(eop)? {
                br.read(4).ok_or_else(eop)? + 1
            } else {
                1
            };
            let mut coupling = Vec::new();
            if br.bit().ok_or_else(eop)? {
                let steps = br.read(8).ok_or_else(eop)? + 1;
                let bits = ilog(channels as u32 - 1);
                for _ in 0..steps {
                    let m = br.read(bits).ok_or_else(eop)? as usize;
                    let a = br.read(bits).ok_or_else(eop)? as usize;
                    if m == a || m >= channels || a >= channels {
                        return Err(malformed("bad channel coupling"));
                    }
                    coupling.push((m as u8, a as u8));
                }
            }
            if br.read(2).ok_or_else(eop)? != 0 {
                return Err(malformed("a mapping's reserved bits are set"));
            }
            let mut mux = vec![0u8; channels];
            if nsub > 1 {
                for m in &mut mux {
                    *m = br.read(4).ok_or_else(eop)? as u8;
                    if u32::from(*m) >= nsub {
                        return Err(malformed("a channel names a missing submap"));
                    }
                }
            }
            let mut submaps = Vec::with_capacity(nsub as usize);
            for _ in 0..nsub {
                br.read(8).ok_or_else(eop)?;
                let f = br.read(8).ok_or_else(eop)?;
                let r = br.read(8).ok_or_else(eop)?;
                if f as usize >= floors.len() || r as usize >= residues.len() {
                    return Err(malformed("a submap names a missing floor or residue"));
                }
                submaps.push((f as u8, r as u8));
            }
            mappings.push(Mapping {
                coupling,
                mux,
                submaps,
            });
        }

        let count = br.read(6).ok_or_else(eop)? + 1;
        let mut modes = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let long = br.bit().ok_or_else(eop)?;
            let window = br.read(16).ok_or_else(eop)?;
            let transform = br.read(16).ok_or_else(eop)?;
            let mapping = br.read(8).ok_or_else(eop)?;
            if window != 0 || transform != 0 {
                return Err(Error::Unsupported(
                    "Vorbis with a reserved window or transform type".to_string(),
                ));
            }
            if mapping as usize >= mappings.len() {
                return Err(malformed("a mode names a missing mapping"));
            }
            modes.push(Mode {
                long,
                mapping: mapping as u8,
            });
        }
        if !br.bit().ok_or_else(eop)? {
            return Err(malformed("the setup header has no framing bit"));
        }

        let slope = |h: usize| -> Vec<f32> {
            let pi = std::f64::consts::PI;
            (0..h)
                .map(|i| {
                    let s = ((i as f64 + 0.5) / h as f64 * pi / 2.0).sin();
                    (pi / 2.0 * s * s).sin() as f32
                })
                .collect()
        };
        let half = halves[1];
        Ok(Decoder {
            ident: id,
            books,
            floors,
            residues,
            mappings,
            modes,
            mode_bits: ilog(count - 1),
            imdct: [
                Imdct::new(usize::from(id.blocksize[0])),
                Imdct::new(usize::from(id.blocksize[1])),
            ],
            slopes: [slope(halves[0]), slope(halves[1])],
            prev_n: None,
            tail: vec![0.0; channels * half],
            spec: vec![0.0; channels * half],
            u: vec![0.0; half],
            left: vec![0.0; half],
            floor1_y: vec![[0; FLOOR1_MAX_VALUES]; channels],
            floor0: vec![(0, [0.0; FLOOR0_MAX_ORDER]); channels],
            floor_used: vec![false; channels],
            no_residue: vec![false; channels],
            classes: Vec::new(),
        })
    }

    pub fn channels(&self) -> usize {
        usize::from(self.ident.channels)
    }

    pub fn sample_rate(&self) -> u32 {
        self.ident.sample_rate
    }

    /// Forgets the overlap (after a seek).
    pub fn reset(&mut self) {
        self.prev_n = None;
    }

    /// Reads the packet type and mode: (mode, block size). None if not an audio packet.
    fn packet_mode(&self, br: &mut BitReader) -> Option<(usize, usize)> {
        if br.bit()? {
            return None;
        }
        let mode = br.read(self.mode_bits)? as usize;
        let long = self.modes.get(mode)?.long;
        Some((mode, usize::from(self.ident.blocksize[usize::from(long)])))
    }

    /// The block size of audio packet `packet` (from its mode), without decoding: None if not
    /// an audio packet. A packet after one of block size `p` produces `p / 4 + n / 4` frames.
    pub fn packet_blocksize(&self, packet: &[u8]) -> Option<usize> {
        self.packet_mode(&mut BitReader::new(packet))
            .map(|(_, n)| n)
    }

    /// The number of frames packet `packet` will produce given the previous packet's block size
    /// (for container timing), without decoding: None if not an audio packet.
    pub fn packet_frames(&self, packet: &[u8]) -> Option<usize> {
        if packet.is_empty() {
            return Some(0);
        }
        let (_, n) = self.packet_mode(&mut BitReader::new(packet))?;
        Some(self.prev_n.map_or(0, |p| p / 4 + n / 4))
    }

    /// Decodes one audio packet, appending its finished frames interleaved, in the Vorbis channel
    /// order, as f32 in -1..=1 (clamped). Returns the frames appended. The first packet after
    /// new/reset appends none (it only primes the overlap), as the spec says.
    pub fn decode(&mut self, packet: &[u8], out: &mut Vec<f32>) -> Result<usize, Error> {
        if packet.is_empty() {
            return Ok(0);
        }
        let mut br = BitReader::new(packet);
        let Some((mode, n)) = self.packet_mode(&mut br) else {
            return Err(malformed("not an audio packet"));
        };
        let long = self.modes[mode].long;
        let (prev_flag, next_flag) = if long {
            match (br.bit(), br.bit()) {
                (Some(p), Some(q)) => (p, q),
                _ => return Err(malformed("an audio packet is cut short")),
            }
        } else {
            (false, false)
        };
        let channels = self.channels();
        let n2 = n / 2;
        let stride = usize::from(self.ident.blocksize[1]) / 2;
        let mapping = &self.mappings[usize::from(self.modes[mode].mapping)];

        // Floors, in channel order.
        for c in 0..channels {
            let (fi, _) = mapping.submaps[usize::from(mapping.mux[c])];
            let used = match &self.floors[usize::from(fi)] {
                Floor::Zero(f) => {
                    let (amp, coefs) = &mut self.floor0[c];
                    match f.decode(&mut br, &self.books, coefs)? {
                        Some(a) => {
                            *amp = a;
                            true
                        }
                        None => false,
                    }
                }
                Floor::One(f) => f.decode(&mut br, &self.books, &mut self.floor1_y[c]),
            };
            self.floor_used[c] = used;
            self.no_residue[c] = !used;
        }
        // Nonzero propagation through coupling.
        for &(m, a) in &mapping.coupling {
            let (m, a) = (usize::from(m), usize::from(a));
            if !self.no_residue[m] || !self.no_residue[a] {
                self.no_residue[m] = false;
                self.no_residue[a] = false;
            }
        }
        // Residues, by submap.
        for c in 0..channels {
            self.spec[c * stride..c * stride + n2].fill(0.0);
        }
        let mut chans = Vec::with_capacity(channels);
        let mut skip = Vec::with_capacity(channels);
        for (s, &(_, ri)) in mapping.submaps.iter().enumerate() {
            chans.clear();
            skip.clear();
            for c in 0..channels {
                if usize::from(mapping.mux[c]) == s {
                    chans.push(c);
                    skip.push(self.no_residue[c]);
                }
            }
            let mut bundle = Bundle {
                buf: &mut self.spec,
                stride,
                chans: &chans,
                skip: &skip,
                n: n2,
            };
            self.residues[usize::from(ri)].decode(
                &mut br,
                &self.books,
                &mut bundle,
                &mut self.classes,
            );
        }
        // Inverse coupling.
        for &(m, a) in mapping.coupling.iter().rev() {
            let (m, a) = (usize::from(m) * stride, usize::from(a) * stride);
            for k in 0..n2 {
                let (mv, av) = (self.spec[m + k], self.spec[a + k]);
                let (nm, na) = if mv > 0.0 {
                    if av > 0.0 {
                        (mv, mv - av)
                    } else {
                        (mv + av, mv)
                    }
                } else if av > 0.0 {
                    (mv, mv + av)
                } else {
                    (mv - av, mv)
                };
                self.spec[m + k] = nm;
                self.spec[a + k] = na;
            }
        }
        // Floor curves times residues.
        for c in 0..channels {
            let v = &mut self.spec[c * stride..c * stride + n2];
            if !self.floor_used[c] {
                v.fill(0.0);
                continue;
            }
            let (fi, _) = mapping.submaps[usize::from(mapping.mux[c])];
            match &self.floors[usize::from(fi)] {
                Floor::Zero(f) => {
                    let (amp, coefs) = &self.floor0[c];
                    f.apply(*amp, coefs, v, long);
                }
                Floor::One(f) => f.apply(&self.floor1_y[c], v),
            }
        }

        // Window shape.
        let bs0 = usize::from(self.ident.blocksize[0]);
        let (left_n, right_n) = (
            if long && !prev_flag { bs0 / 2 } else { n2 },
            if long && !next_flag { bs0 / 2 } else { n2 },
        );
        let left_start = n / 4 - left_n / 2;
        let right_start = n * 3 / 4 - right_n / 2;
        let slope_l = &self.slopes[usize::from(left_n != bs0 / 2)];
        let slope_r = &self.slopes[usize::from(right_n != bs0 / 2)];

        // Output range: the previous block's centre to this block's centre.
        let frames = self.prev_n.map_or(0, |p| p / 4 + n / 4);
        let base = out.len();
        out.resize(base + frames * channels, 0.0);
        let shift = n / 4;
        let prev_half = self.prev_n.map_or(0, |p| p / 2);
        let prev_q = self.prev_n.map_or(0, |p| p / 4);

        let tr = &mut self.imdct[usize::from(long)];
        for c in 0..channels {
            let spec = &self.spec[c * stride..c * stride + n2];
            let u = &mut self.u[..n2];
            if self.floor_used[c] {
                tr.dct4(spec, u);
            } else {
                u.fill(0.0);
            }
            let m = n2;
            let y = |i: usize| -> f32 {
                if i < m / 2 {
                    u[m / 2 + i]
                } else if i < 3 * m / 2 {
                    -u[3 * m / 2 - 1 - i]
                } else {
                    -u[i - 3 * m / 2]
                }
            };
            // Windowed left half.
            let left = &mut self.left[..n2];
            left[..left_start].fill(0.0);
            for i in 0..left_n {
                left[left_start + i] = y(left_start + i) * slope_l[i];
            }
            for i in left_start + left_n..n2 {
                left[i] = y(i);
            }
            // Overlap-add with the previous right half.
            let tail = &mut self.tail[c * stride..c * stride + stride];
            for k in 0..frames {
                let mut s = if k < prev_half { tail[k] } else { 0.0 };
                let j = k + shift;
                if j >= prev_q && j - prev_q < n2 {
                    s += left[j - prev_q];
                }
                out[base + k * channels + c] = if s.is_nan() { 0.0 } else { s.clamp(-1.0, 1.0) };
            }
            // New windowed right half.
            let r0 = right_start - n2;
            for i in 0..r0 {
                tail[i] = y(n2 + i);
            }
            for i in 0..right_n {
                tail[r0 + i] = y(right_start + i) * slope_r[right_n - 1 - i];
            }
            tail[r0 + right_n..n2].fill(0.0);
        }
        self.prev_n = Some(n);
        Ok(frames)
    }
}
