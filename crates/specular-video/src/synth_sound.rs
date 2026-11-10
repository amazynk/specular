//! MIDI songs and sample banks (`specular-synth`) for [`crate::audio`], rendered at 44.1 kHz
//! stereo.
//!
//! - A MIDI file (`Other(b"midi", 0)`) plays through the SoundFont the session found
//!   ([`crate::mp4::SoundExtra::bank`]: one beside the song or installed on the system), a
//!   RIFF MIDI file's own DLS bank, else the built-in General MIDI sounds. Its samples are
//!   runs of 4096 frames with no bytes of their own; the decoder keeps a copy of the
//!   synthesiser every 32 runs, so a seek replays at most about three seconds silently.
//! - A SoundFont, DLS or XI bank (`Other(b"bank", kind)`) plays its samples one after another
//!   at their own pitch ([`specular_synth::synth::SampleList`]).
//! - An SFZ instrument's samples are other files: the session decodes them and the track is
//!   their PCM ([`sfz_track`]).

use std::path::Path;
use std::sync::Arc;

use specular_core::Source;
use specular_synth::synth::{SampleList, Synth};
use specular_synth::{Bank, BankKind, smf};

use crate::audio::{Silence, SoundDecoder};
use crate::mp4::{AudioCodec, AudioTrack, Error, External, Result};
use crate::sound_file::{MAX_SAMPLES, PCM_FRAMES, sample, track};

/// The rate songs and banks are rendered at.
pub(crate) const RATE: u32 = 44_100;
/// Runs between two kept copies of the synthesiser.
const KEEP_EVERY: usize = 32;

pub(crate) fn is_midi(c: &AudioCodec) -> bool {
    matches!(c, AudioCodec::Other(t, _) if t == b"midi")
}

pub(crate) fn is_bank(c: &AudioCodec) -> bool {
    matches!(c, AudioCodec::Other(t, _) if t == b"bank")
}

fn kind_code(k: BankKind) -> u8 {
    match k {
        BankKind::SoundFont => 0,
        BankKind::Dls => 1,
        BankKind::Xi => 2,
    }
}

/// The display name of a song's or bank's track.
pub(crate) fn name(tag: &[u8; 4], code: u8) -> String {
    match (tag, code) {
        (b"midi", 1) => "MIDI (SoundFont)".into(),
        (b"midi", 2) => "MIDI (its own DLS bank)".into(),
        (b"midi", _) => "MIDI (built-in sounds)".into(),
        (_, 1) => "DLS samples".into(),
        (_, 2) => "FastTracker 2 instrument samples".into(),
        _ => "SoundFont samples".into(),
    }
}

fn map(e: specular_synth::Error) -> Error {
    match e {
        specular_synth::Error::Unsupported(m) => Error::Unsupported(m),
        specular_synth::Error::Malformed(m) => Error::Malformed(m),
    }
}

/// Runs of `PCM_FRAMES` over `frames`.
fn runs(frames: u64) -> (Vec<crate::mp4::Sample>, u64) {
    let mut samples = Vec::new();
    let mut t = 0;
    while t < frames && samples.len() < MAX_SAMPLES {
        let n = PCM_FRAMES.min(frames - t);
        samples.push(sample(0, 0, t, n));
        t += n;
    }
    (samples, t)
}

/// A MIDI file as a sound track.
pub(crate) fn midi_track(d: &[u8]) -> Result<AudioTrack> {
    let song = smf::parse(d).map_err(map)?;
    if song.notes == 0 {
        return Err(Error::Malformed("a MIDI file without notes".into()));
    }
    let (samples, t) = runs(Synth::length(&song, RATE));
    let own_bank = smf::unwrap(d).1.is_some();
    let mut tr = track(
        AudioCodec::Other(*b"midi", if own_bank { 2 } else { 0 }),
        2,
        RATE,
        samples,
    );
    tr.duration = t;
    Ok(tr)
}

/// A SoundFont, DLS or XI file as a sound track of its samples.
pub(crate) fn bank_track(d: &[u8]) -> Result<AudioTrack> {
    let bank = Arc::new(Bank::parse(d).map_err(map)?);
    let list = SampleList::new(Arc::clone(&bank), RATE);
    if list.length() == 0 {
        return Err(Error::Malformed(format!(
            "a {} without samples",
            bank.kind.name()
        )));
    }
    let (samples, t) = runs(list.length());
    let mut tr = track(
        AudioCodec::Other(*b"bank", kind_code(bank.kind)),
        2,
        RATE,
        samples,
    );
    tr.duration = t;
    Ok(tr)
}

/// A file's bytes shared with the synthesiser.
struct SourceBytes(Arc<Source>);

impl AsRef<[u8]> for SourceBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.bytes()
    }
}

/// The bytes a track's decoder plays from: the file the session attached, else a copy of
/// `data`.
fn bytes_of(source: Option<&External>, data: &[u8]) -> specular_synth::Bytes {
    match source {
        Some(e) => Arc::new(SourceBytes(Arc::clone(&e.0))),
        None => Arc::new(data.to_vec()),
    }
}

struct Song {
    synth: Synth,
    at: usize,
    kept: Vec<Synth>,
}

impl SoundDecoder for Song {
    fn decode(&mut self, _: &[u8], _: &mut Vec<f32>) -> std::result::Result<(), String> {
        Err("needs the run's index".into())
    }
    fn decode_at(
        &mut self,
        index: usize,
        _: &[u8],
        out: &mut Vec<f32>,
    ) -> std::result::Result<(), String> {
        if index != self.at {
            let k = (index / KEEP_EVERY).min(self.kept.len().saturating_sub(1));
            if index < self.at || k * KEEP_EVERY > self.at {
                self.synth = self.kept[k].clone();
                self.at = k * KEEP_EVERY;
            }
            self.synth.skip((index - self.at) as u64 * PCM_FRAMES);
            self.at = index;
        }
        if self.at.is_multiple_of(KEEP_EVERY) && self.at / KEEP_EVERY == self.kept.len() {
            self.kept.push(self.synth.clone());
        }
        self.synth.render(PCM_FRAMES as usize, out);
        self.at += 1;
        Ok(())
    }
    fn reset(&mut self) {}
}

struct Samples {
    list: SampleList,
}

impl SoundDecoder for Samples {
    fn decode(&mut self, _: &[u8], _: &mut Vec<f32>) -> std::result::Result<(), String> {
        Err("needs the run's index".into())
    }
    fn decode_at(
        &mut self,
        index: usize,
        _: &[u8],
        out: &mut Vec<f32>,
    ) -> std::result::Result<(), String> {
        self.list
            .render(index as u64 * PCM_FRAMES, PCM_FRAMES as usize, out);
        Ok(())
    }
    fn reset(&mut self) {}
}

struct Silent;

impl SoundDecoder for Silent {
    fn decode(&mut self, _: &[u8], _: &mut Vec<f32>) -> std::result::Result<(), String> {
        Err("no data".into())
    }
    fn reset(&mut self) {}
}

type Opened = std::result::Result<(Box<dyn SoundDecoder>, u16, u32), Silence>;

/// The decoder of a MIDI file's track.
pub(crate) fn midi_decoder(t: &AudioTrack, data: &[u8]) -> Opened {
    if data.is_empty() {
        // Asked only whether it plays.
        return Ok((Box::new(Silent), 2, RATE));
    }
    let song = smf::parse(data).map_err(|e| Silence::Unsupported(e.to_string()))?;
    let bank = match &t.extra.bank {
        Some(b) => Bank::read(Arc::new(SourceBytes(Arc::clone(&b.0)))).ok(),
        None => smf::unwrap(data)
            .1
            .and_then(|r| Bank::read(Arc::new(data[r].to_vec())).ok()),
    };
    let synth = Synth::new(Arc::new(song), bank.map(Arc::new), RATE);
    Ok((
        Box::new(Song {
            kept: vec![synth.clone()],
            synth,
            at: 0,
        }),
        2,
        RATE,
    ))
}

/// The decoder of a bank's track.
pub(crate) fn bank_decoder(t: &AudioTrack, data: &[u8]) -> Opened {
    if data.is_empty() {
        return Ok((Box::new(Silent), 2, RATE));
    }
    let bank = Bank::read(bytes_of(t.extra.source.as_ref(), data))
        .map_err(|e| Silence::Unsupported(e.to_string()))?;
    Ok((
        Box::new(Samples {
            list: SampleList::new(Arc::new(bank), RATE),
        }),
        2,
        RATE,
    ))
}

/// Where SoundFonts are looked for besides the song's folder: the user's and the system's
/// sound-font folders (as FluidSynth and TiMidity++ packages install them).
fn soundfont_dirs() -> Vec<std::path::PathBuf> {
    let mut v = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        let h = Path::new(&home);
        v.push(h.join(".local/share/sounds/sf2"));
        v.push(h.join(".local/share/soundfonts"));
    }
    if cfg!(target_os = "macos")
        && let Some(home) = std::env::var_os("HOME")
    {
        v.push(Path::new(&home).join("Library/Audio/Sounds/Banks"));
    }
    for d in [
        "/usr/share/sounds/sf2",
        "/usr/share/soundfonts",
        "/usr/local/share/soundfonts",
    ] {
        v.push(d.into());
    }
    v
}

/// The SoundFont a MIDI file plays through: `SPECULAR_SOUNDFONT` when set, else one named
/// like the song beside it, else the first `.sf2` beside it, else the system's General MIDI
/// font (`default-GM.sf2`, `FluidR3_GM.sf2`…), else the first one installed.
pub fn find_soundfont(song: &Path) -> Option<std::path::PathBuf> {
    let is_sf2 =
        |p: &Path| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("sf2")) && p.is_file();
    if let Some(p) = std::env::var_os("SPECULAR_SOUNDFONT") {
        let p = std::path::PathBuf::from(p);
        return is_sf2(&p).then_some(p);
    }
    let same = song.with_extension("sf2");
    if is_sf2(&same) {
        return Some(same);
    }
    let in_dir = |d: &Path| -> Vec<std::path::PathBuf> {
        let mut v: Vec<_> = std::fs::read_dir(d)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| is_sf2(p))
            .collect();
        v.sort();
        v
    };
    if let Some(dir) = song.parent()
        && let Some(p) = in_dir(dir).into_iter().next()
    {
        return Some(p);
    }
    let installed: Vec<_> = soundfont_dirs().iter().flat_map(|d| in_dir(d)).collect();
    let preferred = [
        "default-gm.sf2",
        "fluidr3_gm.sf2",
        "generaluser",
        "timgm6mb.sf2",
    ];
    preferred
        .iter()
        .find_map(|n| {
            installed.iter().find(|p| {
                p.file_name()
                    .and_then(|f| f.to_str())
                    .is_some_and(|f| f.to_ascii_lowercase().starts_with(n))
            })
        })
        .or(installed.first())
        .cloned()
}

/// Attaches what a song's or bank's track plays from: a bank its own file, a MIDI song the
/// SoundFont found for it (the codec then says so).
pub(crate) fn attach(source: &Arc<Source>, a: &mut AudioTrack) {
    if is_bank(&a.codec) {
        a.extra.source = Some(External(Arc::clone(source)));
    }
    if is_midi(&a.codec) && !matches!(a.codec, AudioCodec::Other(_, 2)) {
        let font = find_soundfont(source.path()).and_then(|p| Source::open(p).ok());
        if let Some(f) = font.filter(|f| specular_synth::sf2::is_sf2(f.bytes())) {
            a.extra.bank = Some(External(Arc::new(f)));
            a.codec = AudioCodec::Other(*b"midi", 1);
        }
    }
}

/// The most sound an SFZ instrument's samples make in all (a long library is cut).
const SFZ_MAX_SECONDS: f64 = 20.0 * 60.0;
/// Silence between two samples.
const SFZ_GAP: f64 = 0.25;

/// An `#include`d file of an SFZ instrument, relative to it, read only when it lies in the
/// instrument's folder tree.
fn sfz_include(source: &Source, rel: &str) -> Option<Vec<u8>> {
    let dir = source.path().parent()?;
    let base = dir.canonicalize().ok()?;
    let path = dir.join(rel.replace('\\', "/")).canonicalize().ok()?;
    if !path.starts_with(&base) || std::fs::metadata(&path).ok()?.len() > 1 << 20 {
        return None;
    }
    std::fs::read(path).ok()
}

/// An SFZ instrument's samples, decoded from the files it names and played one after
/// another at 44.1 kHz stereo: a track of 16-bit PCM in memory. Samples that are missing
/// or don't decode are skipped.
pub(crate) fn sfz_track(source: &Arc<Source>) -> Result<AudioTrack> {
    let inst = specular_synth::sfz::parse_with(source.bytes(), &mut |p| sfz_include(source, p));
    let names = inst.samples();
    if names.is_empty() {
        return Err(Error::Malformed("an SFZ file without samples".into()));
    }
    let dir = source.path().parent().unwrap_or(Path::new("."));
    let mut pcm: Vec<u8> = Vec::new();
    let gap = (SFZ_GAP * f64::from(RATE)) as usize;
    let max_frames = (SFZ_MAX_SECONDS * f64::from(RATE)) as usize;
    let mut played = 0;
    for n in &names {
        if pcm.len() / 4 >= max_frames {
            break;
        }
        let Ok(src) = Source::open(dir.join(n)) else {
            continue;
        };
        let src = Arc::new(src);
        let Ok(movie) = crate::parse_movie(src.bytes(), true) else {
            continue;
        };
        let Some(t) = movie.audio.as_ref() else {
            continue;
        };
        let Ok((f, ch, rate)) = crate::audio::decode_track(t, &src) else {
            continue;
        };
        let ch = usize::from(ch.max(1));
        let frames = f.len() / ch;
        let step = f64::from(rate) / f64::from(RATE);
        let out_frames = ((frames as f64 / step) as usize).min(max_frames - pcm.len() / 4);
        let at = |i: usize, c: usize| f.get(i * ch + c.min(ch - 1)).copied().unwrap_or(0.0);
        for o in 0..out_frames {
            let pos = o as f64 * step;
            let i = pos as usize;
            let fr = (pos - i as f64) as f32;
            for c in 0..2 {
                let x = at(i, c) + (at(i + 1, c) - at(i, c)) * fr;
                let v = (x.clamp(-1.0, 1.0) * 32767.0) as i16;
                pcm.extend_from_slice(&v.to_le_bytes());
            }
        }
        pcm.resize(pcm.len() + gap * 4, 0);
        played += 1;
    }
    if played == 0 {
        return Err(Error::Unsupported(format!(
            "SFZ samples that are missing or can't be read ({} named)",
            names.len()
        )));
    }
    let len = pcm.len();
    let mem = Arc::new(Source::from_vec(
        source.path().with_extension("sfz.pcm"),
        pcm,
    ));
    let mut t = crate::sound_file::plain_track(
        AudioCodec::Pcm {
            bits: 16,
            float: false,
            little_endian: true,
        },
        2,
        RATE,
        16,
        0,
        len,
    )?;
    t.extra.source = Some(External(mem));
    Ok(t)
}

/// What a bank or a song says about itself, for the info panel: its own text fields, then
/// the instruments and samples it lists ("instruments", "samples") or a song's tracks
/// and notes ("comment").
pub fn facts(d: &[u8], sfz_names: bool) -> Vec<(&'static str, String)> {
    const MAX_LISTED: usize = 40;
    let list = |names: Vec<String>, total: usize| {
        let shown: Vec<String> = names.into_iter().take(MAX_LISTED).collect();
        let mut s = shown.join(", ");
        if total > MAX_LISTED {
            s += &format!(", … ({total} in all)");
        }
        s
    };
    if smf::is_midi(d) {
        let Ok(song) = smf::parse(d) else {
            return Vec::new();
        };
        let mut f = song.info.clone();
        let channels = song.channels_used.count_ones();
        f.push((
            "midi",
            format!(
                "format {}, {} track{}, {} notes on {} channel{}",
                song.format,
                song.tracks,
                if song.tracks == 1 { "" } else { "s" },
                song.notes,
                channels,
                if channels == 1 { "" } else { "s" }
            ),
        ));
        return f;
    }
    if sfz_names && specular_synth::sfz::is_sfz(d) {
        let i = specular_synth::sfz::parse(d);
        let s = i.samples();
        let mut f = vec![(
            "instruments",
            format!("{} regions in {} groups", i.regions.len(), i.groups),
        )];
        let total = s.len();
        f.push(("samples", list(s, total)));
        return f;
    }
    let Ok(bank) = Bank::parse(d) else {
        return Vec::new();
    };
    let mut f = bank.info.clone();
    let mut presets: Vec<String> = bank
        .presets
        .iter()
        .map(|p| {
            if bank.kind == BankKind::Xi {
                p.name.clone()
            } else if p.bank == 128 {
                format!("{} (drums {})", p.name, p.program)
            } else {
                format!("{} ({}:{})", p.name, p.bank, p.program)
            }
        })
        .collect();
    presets.retain(|p| !p.is_empty());
    let np = presets.len();
    if np > 0 {
        f.push(("instruments", list(presets, np)));
    }
    let names: Vec<String> = bank.samples.iter().map(|s| s.name.clone()).collect();
    let ns = names.len();
    if ns > 0 {
        f.push(("samples", list(names, ns)));
    }
    f
}
