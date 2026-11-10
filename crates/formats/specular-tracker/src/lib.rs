//! Tracker modules: ProTracker-style MOD (4 to 32 channels), Scream Tracker 3 (S3M),
//! FastTracker 2 (XM) and Impulse Tracker (IT, also its compressed samples), and the older
//! Composer 669 / UNIS 669, MultiTracker (MTM), Scream Tracker 2 (STM), Oktalyzer (OKT),
//! MED / OctaMED (MMD0 to MMD3), UltraTracker (ULT) and Farandole Composer (FAR), loaded into
//! one model and rendered to stereo PCM by an own player and mixer.
//!
//! Own code, from the formats' public descriptions (the ProTracker module layout, Scream
//! Tracker 3's TECH.DOC, FastTracker 2's XM.TXT, Impulse Tracker's ITTECH.TXT, Tran's 669
//! notes, the MultiTracker, Scream Tracker 2, Oktalyzer and UltraTracker write-ups, Teijo
//! Kinnunen's MMD documentation, Daniel Potter's Farandole specification). The player
//! keeps one pitch model for all four: a note of 60 plays a sample at its own rate (C-5 in
//! Impulse Tracker's numbering); Amiga-style "periods" (four to ProTracker's unit) or linear
//! ones (1/64 semitone) carry the slides. Effects are kept in Scream Tracker's letters, the
//! MOD and XM ones translated on load.
//!
//! What is left out: Impulse Tracker's new-note actions (a new note cuts the one before),
//! resonant filters and pitch envelopes, MIDI macros, tremor and panbrello, 15-sample
//! Soundtracker modules (refused: they carry no signature).

mod load_669;
mod load_far;
mod load_it;
mod load_med;
mod load_mod;
mod load_mtm;
mod load_okt;
mod load_s3m;
mod load_stm;
mod load_ult;
mod load_xm;
mod player;

use std::fmt;

pub use player::Player;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Unsupported(String),
    Malformed(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unsupported(m) => write!(f, "unsupported module: {m}"),
            Error::Malformed(m) => write!(f, "damaged module: {m}"),
        }
    }
}

impl std::error::Error for Error {}

pub(crate) fn bad(m: &str) -> Error {
    Error::Malformed(m.into())
}

/// The module formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Mod,
    S3m,
    Xm,
    It,
    /// Composer 669 and UNIS 669.
    Six69,
    Mtm,
    Stm,
    Okt,
    /// MED / OctaMED, MMD0 to MMD3.
    Med,
    Ult,
    Far,
}

const KINDS: [Kind; 11] = [
    Kind::Mod,
    Kind::S3m,
    Kind::Xm,
    Kind::It,
    Kind::Six69,
    Kind::Mtm,
    Kind::Stm,
    Kind::Okt,
    Kind::Med,
    Kind::Ult,
    Kind::Far,
];

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Mod => "ProTracker module",
            Kind::S3m => "Scream Tracker 3 module",
            Kind::Xm => "FastTracker 2 module",
            Kind::It => "Impulse Tracker module",
            Kind::Six69 => "Composer 669 module",
            Kind::Mtm => "MultiTracker module",
            Kind::Stm => "Scream Tracker 2 module",
            Kind::Okt => "Oktalyzer module",
            Kind::Med => "OctaMED module",
            Kind::Ult => "UltraTracker module",
            Kind::Far => "Farandole Composer module",
        }
    }

    pub fn code(self) -> u8 {
        KINDS.iter().position(|&k| k == self).unwrap_or(0) as u8
    }

    pub fn from_code(c: u8) -> Option<Kind> {
        KINDS.get(usize::from(c)).copied()
    }

    /// The player rules a kind follows: the older formats play as MOD or S3M do.
    pub(crate) fn style(self) -> Kind {
        match self {
            Kind::Mod | Kind::Mtm | Kind::Okt | Kind::Med | Kind::Ult => Kind::Mod,
            Kind::S3m | Kind::Six69 | Kind::Stm | Kind::Far => Kind::S3m,
            k => k,
        }
    }
}

/// The channels a MOD's tag at byte 1080 says (none: not a MOD with a tag).
pub(crate) fn mod_channels(tag: &[u8]) -> Option<usize> {
    let digit = |b: u8| b.is_ascii_digit().then(|| usize::from(b - b'0'));
    match tag {
        b"M.K." | b"M!K!" | b"M&K!" | b"FLT4" | b"4CHN" | b"N.T." => Some(4),
        b"FLT8" | b"OKTA" | b"OCTA" | b"CD81" | b"8CHN" => Some(8),
        [c, b'C', b'H', b'N'] => digit(*c).filter(|&n| n > 0),
        [a, b, b'C', b'H'] | [a, b, b'C', b'N'] => {
            Some(digit(*a)? * 10 + digit(*b)?).filter(|&n| (1..=32).contains(&n))
        }
        [b'T', b'D', b'Z', c] => digit(*c).filter(|&n| n > 0),
        _ => None,
    }
}

/// Which module `d` is, by its signature (never by name: `.mod` is also a camcorder's
/// MPEG programme stream, which starts with a pack header and is left to the video path).
pub fn sniff(d: &[u8]) -> Option<Kind> {
    if d.starts_with(&[0, 0, 1, 0xBA]) {
        return None;
    }
    if d.len() >= 0x40 && d.starts_with(b"IMPM") {
        return Some(Kind::It);
    }
    if d.len() >= 80 && d.starts_with(b"Extended Module: ") {
        return Some(Kind::Xm);
    }
    if d.len() >= 96 && d.get(44..48) == Some(b"SCRM") && d[29] == 16 {
        return Some(Kind::S3m);
    }
    if d.len() >= 66 && d.starts_with(b"MTM") && d[3] >> 4 == 1 {
        return Some(Kind::Mtm);
    }
    if d.len() >= 48 + 31 * 32 + 128
        && d[28] == 0x1A
        && d[29] == 2
        && d[20..28].iter().all(|c| c.is_ascii_graphic())
        && d[30] == 2
    {
        return Some(Kind::Stm);
    }
    if d.len() >= 16 && d.starts_with(b"OKTASONG") && &d[8..12] == b"CMOD" {
        return Some(Kind::Okt);
    }
    if d.len() >= 52 && matches!(&d[..4], b"MMD0" | b"MMD1" | b"MMD2" | b"MMD3") {
        return Some(Kind::Med);
    }
    if d.len() >= 64 && d.starts_with(b"MAS_UTrack_V00") && (b'1'..=b'4').contains(&d[14]) {
        return Some(Kind::Ult);
    }
    if d.len() >= 98 && d.starts_with(b"FAR\xFE") && d[44..47] == [13, 10, 26] {
        return Some(Kind::Far);
    }
    if load_669::sniff(d) {
        return Some(Kind::Six69);
    }
    if d.len() >= 1084 + 64 * 4 && mod_channels(&d[1080..1084]).is_some() {
        // The song length and the orders must be sane.
        let len = usize::from(d[950]);
        if (1..=128).contains(&len) && d[952..952 + len].iter().all(|&o| o < 128) {
            return Some(Kind::Mod);
        }
    }
    None
}

/// How a sample loops.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LoopKind {
    #[default]
    None,
    Forward,
    PingPong,
}

#[derive(Clone, Debug, Default)]
pub struct Sample {
    pub name: String,
    /// Mono samples scaled to ±1.0 (a stereo sample's channels are averaged).
    pub data: Vec<f32>,
    pub loop_kind: LoopKind,
    pub loop_start: usize,
    pub loop_end: usize,
    /// Impulse Tracker's sustain loop (played until the note is released).
    pub sustain: Option<(usize, usize, LoopKind)>,
    /// The rate the sample plays at for note 60.
    pub c_speed: f64,
    /// Default volume, 0–64.
    pub volume: u8,
    /// Sample global volume (IT), 0–64.
    pub global_volume: u8,
    /// Default panning (0–255), if the sample sets one.
    pub pan: Option<u8>,
    /// Auto-vibrato (XM, IT): type, sweep, depth, rate.
    pub vibrato: (u8, u8, u8, u8),
}

#[derive(Clone, Debug, Default)]
pub struct Envelope {
    pub enabled: bool,
    /// (tick, value): volume 0–64, panning −32…32.
    pub points: Vec<(u16, i16)>,
    pub sustain: Option<(usize, usize)>,
    pub looped: Option<(usize, usize)>,
}

#[derive(Clone, Debug, Default)]
pub struct Instrument {
    pub name: String,
    /// The sample (1-based, 0 none) and the note it plays for each of the 120 notes.
    pub keymap: Vec<(u8, u8)>,
    pub volume: Envelope,
    pub panning: Envelope,
    /// Volume lost a tick after the note is released, in 1/65536 (XM's ×2, IT's as is ÷2…).
    pub fadeout: u32,
    /// Instrument global volume (IT), 0–128.
    pub global_volume: u8,
    pub pan: Option<u8>,
}

/// The volume column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum VolCmd {
    #[default]
    None,
    Set(u8),
    SlideUp(u8),
    SlideDown(u8),
    FineUp(u8),
    FineDown(u8),
    Pan(u8),
    PanLeft(u8),
    PanRight(u8),
    PortaUp(u8),
    PortaDown(u8),
    TonePorta(u8),
    Vibrato(u8),
    VibratoSpeed(u8),
}

/// Notes in a cell beside 0–119.
pub const NOTE_NONE: u8 = 255;
pub const NOTE_OFF: u8 = 254;
pub const NOTE_CUT: u8 = 253;
pub const NOTE_FADE: u8 = 252;

/// Effects beside Scream Tracker's letters (b'A'…b'Z').
pub const FX_SET_VOLUME: u8 = b'v';
pub const FX_KEY_OFF: u8 = b'k';
pub const FX_MOD_VOLSLIDE: u8 = b'd';
pub const FX_ENVELOPE_POS: u8 = b'l';
pub const FX_FINE_VOL_UP: u8 = b'f';
pub const FX_FINE_VOL_DOWN: u8 = b'g';

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub note: u8,
    pub instr: u8,
    pub vol: VolCmd,
    pub fx: u8,
    pub param: u8,
}

impl Default for Cell {
    fn default() -> Cell {
        Cell {
            note: NOTE_NONE,
            instr: 0,
            vol: VolCmd::None,
            fx: 0,
            param: 0,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Pattern {
    pub rows: usize,
    /// `rows × channels` cells.
    pub cells: Vec<Cell>,
}

/// A loaded module.
#[derive(Clone, Debug)]
pub struct Module {
    pub kind: Kind,
    pub title: String,
    pub channels: usize,
    /// Pattern numbers in play order (254: skip, 255: end).
    pub orders: Vec<u8>,
    pub patterns: Vec<Pattern>,
    pub samples: Vec<Sample>,
    pub instruments: Vec<Instrument>,
    pub speed: u8,
    pub tempo: u8,
    /// 0–128.
    pub global_volume: u8,
    /// Overall loudness the format asks for (S3M master, IT mixing volume), 0–128.
    pub mix_volume: u8,
    pub pans: Vec<u8>,
    /// Channel volumes (IT), 0–64.
    pub channel_volumes: Vec<u8>,
    pub linear: bool,
    /// Whether instruments (rather than samples) are what cells name (XM, IT in that mode).
    pub use_instruments: bool,
    pub restart: usize,
    /// The format's rule that an effect without a parameter repeats the last one (all but
    /// MOD's volume slides and porta).
    pub fx_memory: bool,
}

impl Module {
    /// Loads a module of any of the four kinds.
    pub fn load(d: &[u8]) -> Result<Module, Error> {
        let m = match sniff(d) {
            Some(Kind::Mod) => load_mod::load(d)?,
            Some(Kind::S3m) => load_s3m::load(d)?,
            Some(Kind::Xm) => load_xm::load(d)?,
            Some(Kind::It) => load_it::load(d)?,
            Some(Kind::Six69) => load_669::load(d)?,
            Some(Kind::Mtm) => load_mtm::load(d)?,
            Some(Kind::Stm) => load_stm::load(d)?,
            Some(Kind::Okt) => load_okt::load(d)?,
            Some(Kind::Med) => load_med::load(d)?,
            Some(Kind::Ult) => load_ult::load(d)?,
            Some(Kind::Far) => load_far::load(d)?,
            None => return Err(bad("no module signature")),
        };
        if m.channels == 0 || m.channels > 64 {
            return Err(Error::Unsupported(format!("{} channels", m.channels)));
        }
        if !m.orders.iter().any(|&o| usize::from(o) < m.patterns.len()) {
            return Err(bad("no pattern in the order list"));
        }
        Ok(m)
    }

    /// Frames the song plays at `rate` (to its end or until it would repeat), at most an
    /// hour.
    pub fn length(&self, rate: u32) -> u64 {
        let mut p = Player::new(std::sync::Arc::new(self.clone()), rate);
        p.skip(u64::from(rate) * 3600)
    }
}

/// Text from fixed-size fields: up to the first NUL, control characters dropped.
pub(crate) fn text(b: &[u8]) -> String {
    let b = b.split(|&c| c == 0).next().unwrap_or_default();
    b.iter()
        .map(|&c| {
            if (32..127).contains(&c) {
                c as char
            } else {
                ' '
            }
        })
        .collect::<String>()
        .trim()
        .to_owned()
}

pub(crate) fn le16(d: &[u8], at: usize) -> u16 {
    d.get(at..at + 2)
        .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]))
}
pub(crate) fn le32(d: &[u8], at: usize) -> u32 {
    d.get(at..at + 4)
        .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
pub(crate) fn be16(d: &[u8], at: usize) -> u16 {
    d.get(at..at + 2)
        .map_or(0, |b| u16::from_be_bytes([b[0], b[1]]))
}
pub(crate) fn be32(d: &[u8], at: usize) -> u32 {
    d.get(at..at + 4)
        .map_or(0, |b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// Signed 8-bit sample bytes (unsigned ones flipped first) scaled to ±1.
pub(crate) fn pcm8(b: &[u8], unsigned: bool) -> Vec<f32> {
    let flip = if unsigned { 0x80 } else { 0 };
    b.iter()
        .take(MAX_SAMPLE)
        .map(|&v| f32::from((v ^ flip) as i8) / 128.0)
        .collect()
}

/// Signed 16-bit little-endian sample bytes scaled to ±1.
pub(crate) fn pcm16le(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(2)
        .take(MAX_SAMPLE)
        .map(|w| f32::from(i16::from_le_bytes([w[0], w[1]])) / 32768.0)
        .collect()
}

/// A sample from its data and loop (frames; an empty or out-of-range loop plays once).
pub(crate) fn sample(
    name: String,
    data: Vec<f32>,
    lp: Option<(usize, usize)>,
    c_speed: f64,
    volume: u8,
) -> Sample {
    let (ls, le) = lp.unwrap_or((0, 0));
    let le = le.min(data.len());
    let looped = lp.is_some() && le > ls + 1;
    Sample {
        name,
        loop_kind: if looped {
            LoopKind::Forward
        } else {
            LoopKind::None
        },
        loop_start: if looped { ls } else { 0 },
        loop_end: if looped { le } else { 0 },
        data,
        c_speed,
        volume: volume.min(64),
        global_volume: 64,
        ..Sample::default()
    }
}

/// A module of `channels` channels in MOD's defaults (tempo 125, no instruments).
pub(crate) fn module(kind: Kind, title: String, channels: usize) -> Module {
    Module {
        kind,
        title,
        channels,
        orders: Vec::new(),
        patterns: Vec::new(),
        samples: Vec::new(),
        instruments: Vec::new(),
        speed: 6,
        tempo: 125,
        global_volume: 128,
        mix_volume: 128,
        pans: (0..channels)
            .map(|c| if c % 4 == 0 || c % 4 == 3 { 0 } else { 255 })
            .collect(),
        channel_volumes: vec![64; channels],
        linear: false,
        use_instruments: false,
        restart: 0,
        fx_memory: false,
    }
}

/// The most sample frames loaded (a damaged length can't ask for gigabytes).
pub(crate) const MAX_SAMPLE: usize = 1 << 24;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_tags() {
        assert_eq!(mod_channels(b"M.K."), Some(4));
        assert_eq!(mod_channels(b"6CHN"), Some(6));
        assert_eq!(mod_channels(b"16CH"), Some(16));
        assert_eq!(mod_channels(b"32CN"), Some(32));
        assert_eq!(mod_channels(b"ABCD"), None);
    }

    #[test]
    fn older_signatures() {
        let mut d = vec![0u8; 4096];
        d[..4].copy_from_slice(b"MTM\x10");
        assert_eq!(sniff(&d), Some(Kind::Mtm));
        d[..8].copy_from_slice(b"OKTASONG");
        d[8..12].copy_from_slice(b"CMOD");
        assert_eq!(sniff(&d), Some(Kind::Okt));
        d[..4].copy_from_slice(b"MMD2");
        assert_eq!(sniff(&d), Some(Kind::Med));
        d[..15].copy_from_slice(b"MAS_UTrack_V004");
        assert_eq!(sniff(&d), Some(Kind::Ult));
        let mut f = vec![0u8; 4096];
        f[..4].copy_from_slice(b"FAR\xFE");
        f[44..47].copy_from_slice(&[13, 10, 26]);
        assert_eq!(sniff(&f), Some(Kind::Far));
        let mut s = vec![0u8; 4096];
        s[20..28].copy_from_slice(b"!Scream!");
        s[28..31].copy_from_slice(&[0x1A, 2, 2]);
        assert_eq!(sniff(&s), Some(Kind::Stm));
        for k in KINDS {
            assert_eq!(Kind::from_code(k.code()), Some(k));
        }
    }

    #[test]
    fn mpeg_pack_is_not_a_module() {
        let mut d = vec![0u8; 2048];
        d[..4].copy_from_slice(&[0, 0, 1, 0xBA]);
        d[1080..1084].copy_from_slice(b"M.K.");
        d[950] = 1;
        assert_eq!(sniff(&d), None);
    }
}
