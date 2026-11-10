//! Specular's own MIDI player and sample-bank readers, written from the formats' public
//! descriptions (the MIDI 1.0 and Standard MIDI File specifications, SoundFont 2.04, DLS
//! levels 1 and 2, the SFZ format pages, FastTracker 2's XI layout):
//!
//! - [`smf`]: Standard MIDI Files (`.mid`, `.midi`, `.kar`) and RIFF MIDI (`.rmi`), read into
//!   timed events with their text (title, copyright, karaoke lyrics);
//! - [`sf2`], [`dls`], [`xi`]: SoundFont 2, Downloadable Sounds and FastTracker 2 instruments,
//!   read into one [`Bank`] model (presets of key/velocity zones over samples);
//! - [`sfz`]: SFZ instrument text (regions over sample files beside it);
//! - [`synth`]: a small synthesiser that plays a song through a bank, or through a built-in
//!   General MIDI approximation (oscillators, plucked strings and noise drums) when there is
//!   no bank; [`synth::SampleList`] plays a bank's samples one after another.
//!
//! Nothing is bundled: a SoundFont is used only when the user has one.

#![deny(unsafe_code)]
#![allow(clippy::needless_range_loop)]

use std::sync::Arc;

pub mod dls;
pub mod sf2;
pub mod sfz;
pub mod smf;
pub mod synth;
pub mod xi;

/// What went wrong reading a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A kind of file this crate does not read, in words.
    Unsupported(String),
    /// A damaged or cut file.
    Malformed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::Unsupported(m) | Error::Malformed(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn bad(m: &str) -> Error {
    Error::Malformed(m.into())
}

/// A bank's bytes, shared with whoever holds the file (a memory map).
pub type Bytes = Arc<dyn AsRef<[u8]> + Send + Sync>;

/// How a sample's frames are stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleFormat {
    /// Signed 16-bit little-endian (SoundFont, DLS, decoded XI).
    I16,
    /// Unsigned 8-bit (DLS, WAVE).
    U8,
}

impl SampleFormat {
    pub fn bytes(self) -> usize {
        match self {
            SampleFormat::I16 => 2,
            SampleFormat::U8 => 1,
        }
    }
}

/// One mono sample of a bank.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    pub name: String,
    pub rate: u32,
    pub format: SampleFormat,
    /// Byte offset of the first frame in the bank's data.
    pub offset: usize,
    /// Frames.
    pub len: usize,
    /// The loop, in frames from the first (`loop_end` past the last looped frame).
    pub loop_start: usize,
    pub loop_end: usize,
    /// The MIDI key the sample sounds at when played at its rate.
    pub root: u8,
    /// Cents to add to reach that key exactly.
    pub cents: i16,
}

/// How a zone's sample loops.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LoopMode {
    /// Played once.
    #[default]
    None,
    /// Looped while it sounds.
    Continuous,
    /// Looped until the key is released, then played to its end.
    UntilRelease,
}

/// A volume envelope: times in seconds, the sustain level as a linear gain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Envelope {
    pub delay: f32,
    pub attack: f32,
    pub hold: f32,
    pub decay: f32,
    /// Sustain attenuation in decibels (0: full level).
    pub sustain_db: f32,
    pub release: f32,
}

impl Default for Envelope {
    fn default() -> Self {
        Envelope {
            delay: 0.0,
            attack: 0.001,
            hold: 0.0,
            decay: 0.001,
            sustain_db: 0.0,
            release: 0.01,
        }
    }
}

/// A key and velocity range of a preset that plays one sample.
#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    pub keys: (u8, u8),
    pub vels: (u8, u8),
    pub sample: usize,
    /// The key the sample sounds at (the sample's own unless the zone overrides it).
    pub root: u8,
    /// Tuning in cents (coarse and fine together, plus the sample's correction).
    pub tune: f32,
    /// Cents per key (100: the sample follows the keyboard).
    pub scale: f32,
    pub loop_mode: LoopMode,
    /// Frame offsets added to the sample's start, end and loop.
    pub start_off: i64,
    pub end_off: i64,
    pub loop_start_off: i64,
    pub loop_end_off: i64,
    /// Attenuation in decibels (positive: quieter).
    pub atten_db: f32,
    /// Pan from -1 (left) to 1 (right).
    pub pan: f32,
    pub env: Envelope,
    /// A low-pass filter's cut-off in Hz (`None`: open).
    pub cutoff: Option<f32>,
    /// The filter's resonance in decibels.
    pub q_db: f32,
    /// Notes of one class on a channel cut each other (hi-hats).
    pub exclusive: u16,
    /// Keys fixed by the zone (drum kits playing one pitch whatever the key).
    pub fixed_key: Option<u8>,
}

impl Zone {
    /// A zone playing `sample` over every key at its own pitch.
    pub fn whole(sample: usize, root: u8) -> Zone {
        Zone {
            keys: (0, 127),
            vels: (0, 127),
            sample,
            root,
            tune: 0.0,
            scale: 100.0,
            loop_mode: LoopMode::None,
            start_off: 0,
            end_off: 0,
            loop_start_off: 0,
            loop_end_off: 0,
            atten_db: 0.0,
            pan: 0.0,
            env: Envelope::default(),
            cutoff: None,
            q_db: 0.0,
            exclusive: 0,
            fixed_key: None,
        }
    }
}

/// A playable instrument: its bank and program numbers and zones.
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    pub name: String,
    /// The bank number (MIDI bank select; 128 for drum kits).
    pub bank: u16,
    pub program: u8,
    pub zones: Vec<Zone>,
}

/// The kind of file a bank was read from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BankKind {
    SoundFont,
    Dls,
    Xi,
}

impl BankKind {
    pub fn name(self) -> &'static str {
        match self {
            BankKind::SoundFont => "SoundFont 2",
            BankKind::Dls => "DLS",
            BankKind::Xi => "FastTracker 2 instrument",
        }
    }
}

/// Instruments over samples, whatever file they came from.
pub struct Bank {
    pub kind: BankKind,
    /// Text fields by the sound tags' keys ("title", "copyright", "comment", "encoder",
    /// "date").
    pub info: Vec<(&'static str, String)>,
    pub presets: Vec<Preset>,
    pub samples: Vec<Sample>,
    /// The bytes the samples are in.
    pub data: Bytes,
}

impl std::fmt::Debug for Bank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bank")
            .field("kind", &self.kind)
            .field("presets", &self.presets.len())
            .field("samples", &self.samples.len())
            .finish()
    }
}

impl Bank {
    /// Frame `i` of sample `s` as -1..1 (0 outside it).
    #[inline]
    pub fn frame(&self, s: &Sample, i: usize) -> f32 {
        if i >= s.len {
            return 0.0;
        }
        let d = (*self.data).as_ref();
        match s.format {
            SampleFormat::I16 => {
                let at = s.offset + 2 * i;
                match d.get(at..at + 2) {
                    Some(b) => f32::from(i16::from_le_bytes([b[0], b[1]])) / 32768.0,
                    None => 0.0,
                }
            }
            SampleFormat::U8 => d
                .get(s.offset + i)
                .map_or(0.0, |&b| (f32::from(b) - 128.0) / 128.0),
        }
    }

    /// The preset for a bank and program, falling back as General MIDI players do: the same
    /// program in bank 0 (or the first kit for drums), then the first preset.
    pub fn find(&self, bank: u16, program: u8) -> Option<&Preset> {
        let exact = |b: u16, p: u8| self.presets.iter().find(|x| x.bank == b && x.program == p);
        exact(bank, program)
            .or_else(|| {
                if bank == 128 {
                    exact(128, 0).or_else(|| self.presets.iter().find(|x| x.bank == 128))
                } else {
                    exact(0, program)
                }
            })
            .or_else(|| (bank != 128).then(|| self.presets.first()).flatten())
    }

    /// Reads a SoundFont 2, DLS or XI file's tables only: the samples' bytes are not
    /// attached (an XI file's, decoded, are its own).
    pub fn parse(d: &[u8]) -> Result<Bank> {
        if sf2::is_sf2(d) {
            sf2::read(d)
        } else if dls::is_dls(d) {
            dls::read(d)
        } else if xi::is_xi(d) {
            xi::read(d)
        } else {
            Err(Error::Unsupported("not a sample bank".into()))
        }
    }

    /// Reads a SoundFont 2, DLS or XI file, its samples played from `data`.
    pub fn read(data: Bytes) -> Result<Bank> {
        let b = Bank::parse((*data).as_ref())?;
        Ok(b.with_data(data))
    }

    /// The bank with its samples in `data` (the file it was read from); an XI bank keeps its
    /// own decoded samples.
    pub fn with_data(mut self, data: Bytes) -> Bank {
        if self.kind != BankKind::Xi {
            self.data = data;
        }
        self
    }
}

/// RIFF chunks of `d` from `at` to `end`: (fourcc, body range).
pub(crate) fn chunks(
    d: &[u8],
    mut at: usize,
    end: usize,
) -> Vec<([u8; 4], std::ops::Range<usize>)> {
    let end = end.min(d.len());
    let mut out = Vec::new();
    while at + 8 <= end {
        let id: [u8; 4] = d[at..at + 4].try_into().unwrap_or([0; 4]);
        let size = u32::from_le_bytes(d[at + 4..at + 8].try_into().unwrap_or([0; 4])) as usize;
        let body = at + 8;
        let stop = body.saturating_add(size).min(end);
        out.push((id, body..stop));
        at = stop + (size & 1);
        if out.len() > 1 << 20 {
            break;
        }
    }
    out
}

/// A LIST chunk's type and its sub-chunks.
pub(crate) fn list(
    cs: &[([u8; 4], std::ops::Range<usize>)],
    d: &[u8],
    kind: &[u8; 4],
) -> Option<std::ops::Range<usize>> {
    cs.iter()
        .find(|(id, r)| id == b"LIST" && d.get(r.start..r.start + 4) == Some(&kind[..]))
        .map(|(_, r)| r.start + 4..r.end)
}

/// Text of a fixed or chunk field: up to the first NUL, Latin-1 unless it is valid UTF-8.
pub(crate) fn text(b: &[u8]) -> String {
    let b = b.split(|&c| c == 0).next().unwrap_or(b);
    match std::str::from_utf8(b) {
        Ok(s) => s.trim().to_owned(),
        Err(_) => b
            .iter()
            .map(|&c| char::from(c))
            .collect::<String>()
            .trim()
            .to_owned(),
    }
}

/// A RIFF INFO list's fields by the sound tags' keys.
pub(crate) fn info_fields(d: &[u8], r: std::ops::Range<usize>) -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = Vec::new();
    for (id, b) in chunks(d, r.start, r.end) {
        let key = match &id {
            b"INAM" => "title",
            b"ICOP" => "copyright",
            b"ICMT" => "comment",
            b"ISFT" => "encoder",
            b"ICRD" => "date",
            b"IENG" | b"IART" => "artist",
            b"IPRD" => "album",
            b"IGNR" => "genre",
            _ => continue,
        };
        let v = text(&d[b]);
        if !v.is_empty() && !out.iter().any(|(k, _)| *k == key) {
            out.push((key, v));
        }
    }
    out
}

pub(crate) fn u16le(d: &[u8], at: usize) -> u16 {
    d.get(at..at + 2)
        .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]))
}
pub(crate) fn u32le(d: &[u8], at: usize) -> u32 {
    d.get(at..at + 4)
        .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
pub(crate) fn i32le(d: &[u8], at: usize) -> i32 {
    u32le(d, at) as i32
}
