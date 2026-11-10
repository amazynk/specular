//! A movie's sound: the first audio track decoded on its own thread (PCM here; AAC through
//! `specular-aac`, Opus through `specular-opus`, AC-3 and E-AC-3 through `specular-ac3`,
//! Vorbis and FLAC through `specular-vorbis` and `specular-flac`, MPEG audio and MP3 through
//! `specular-mpa`, Apple Lossless through `specular-alac`, G.711 and ADPCM here), mapped to the
//! device's channels ([`crate::speakers`]: 5.1 and 7.1 as is on a device with those
//! speakers, else folded to stereo), resampled to the device's rate and played through an [`AudioOutput`] (cpal in the app,
//! [`FakeOutput`] in tests).
//!
//! The sound is the master clock of playback: [`AudioPlayer::clock`] says which media time
//! is being heard now, from the instant each device buffer will reach the speakers, and the
//! animation player follows it (`specular_anim::Player::set_clock`). The clock stops (`None`)
//! when there is nothing to hear — paused, before the first buffer after a seek, past the end
//! of the sound — and the picture then keeps time by the wall clock. Muting keeps the clock:
//! the device plays silence at the same pace.
//!
//! Away from 1× ([`AudioPlayer::set_speed`]) the sound is time-stretched in the device
//! callback (`crate::stretch`, pitch kept) and the clock runs at the speed; the speed takes
//! effect from the next device buffer. At 1× the queued frames go out untouched.
//!
//! Without a device, or when it goes away, playback goes on silently: [`AudioPlayer::lost`]
//! says so for the UI to tell once.
//!
//! **The chain**, in the order the sound goes through it:
//!
//! 1. decode (each file's encoder delay and padding trimmed);
//! 2. channel map ([`crate::speakers::Mapping`]: surround as is on a surround device, else
//!    folded to stereo; dialogue boost lifts the centre, [`crate::dialogue::centre_gain`]);
//! 3. resample to the device's rate;
//! 4. levelling gain ([`crate::levelling`]: tags, or measured), per file;
//! 5. crossfade or gapless splice into the next file ([`AudioPlayer::set_next`]);
//!
//! — the queue (about [`AHEAD`] of sound) —
//!
//! 6. time-stretch away from 1× ([`crate::stretch`]);
//! 7. equaliser ([`crate::eq`]);
//! 8. dialogue boost: speech emphasis and compression ([`crate::dialogue`]);
//! 9. volume;
//! 10. mono, then balance;
//! 11. limiter (when anything may pass full scale);
//! 12. fade in or out (a device change, [`AudioPlayer::fade_in`], [`AudioPlayer::fade_out`]);
//! 13. output.
//!
//! Steps 1 to 5 run on the decoding thread (per file: a splice or crossfade mixes two
//! files' levelled sound); 6 to 11 in the device callback ([`crate::dsp::Chain`]), so the
//! equaliser, dialogue boost, balance and mono answer from the next buffer. The resampler
//! sits before the stretcher (which works at the device's rate) and the equaliser after it:
//! a linear filter and the stretch commute closely enough, and the filters are then
//! designed once, at the device's rate. With everything off and the volume at most 100 %,
//! the sound is bit-identical to the decoder's (no stage runs).

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use parking_lot::{Condvar, Mutex};
use specular_core::Source;

use crate::dsp::{Chain, SoundSettings};
use crate::levelling::{self, GainTags, Levelling, Meter, Source as LevelSource};
use crate::mp4::{AudioCodec, AudioTrack, Movie};
use crate::speakers::{DeviceOrder, Mapping};
use crate::stretch::Stretcher;

/// What a device plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceFormat {
    pub rate: u32,
    pub channels: u16,
}

/// Fills one device buffer with interleaved frames; the instant is when its first frame will
/// be heard (the callback's time plus the output latency).
pub type Render = Box<dyn FnMut(&mut [f32], Instant) + Send>;

/// A sound device.
pub trait AudioOutput: Send {
    /// Starts the device, which calls `render` from its own thread for every buffer. The
    /// sound has `source_channels` channels (6 and 8: 5.1 and 7.1); the device picks its
    /// own count.
    fn start(&mut self, render: Render, source_channels: u16) -> Result<DeviceFormat, String>;
    /// The device has gone away since it started (unplugged, sound server gone).
    fn lost(&self) -> bool;
    /// The order the device takes more than two channels in.
    fn order(&self) -> DeviceOrder {
        DeviceOrder::native()
    }
    /// The device opened (empty before it starts, or unknown).
    fn device(&self) -> OutputDevice {
        OutputDevice::default()
    }
}

/// A device as opened.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutputDevice {
    /// The sound system's id for it (cpal's: `alsa:default`, `wasapi:{…}`).
    pub id: String,
    /// Its name for the listener.
    pub name: String,
    /// It was opened as the system's default.
    pub default: bool,
}

/// What plays where: the device and its format, the sound's own, and what was done to fit
/// it ([`AudioPlayer::output`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputInfo {
    pub device: OutputDevice,
    pub format: DeviceFormat,
    /// The heard file's channels and rate as decoded.
    pub source_channels: u16,
    pub source_rate: u32,
    /// The rate was converted to the device's.
    pub resampled: bool,
    /// The sound reaches fewer speakers than it has (folded to stereo, or 7.1 to 5.1).
    pub downmixed: bool,
}

/// Why a movie plays without sound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Silence {
    /// The movie has no sound track.
    NoTrack,
    /// The sound's coding can't be decoded (named).
    Unsupported(String),
    /// There is no sound device, or it would not start.
    NoDevice(String),
    /// The sound's decoder stopped part way (a bug on this file: the panic's brief).
    Failed(String),
}

impl std::fmt::Display for Silence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Silence::NoTrack => f.write_str("no sound track"),
            Silence::Unsupported(s) => write!(f, "{s} sound can't be played"),
            Silence::NoDevice(s) => write!(f, "no sound device ({s})"),
            Silence::Failed(s) => write!(f, "the sound stopped ({s})"),
        }
    }
}

/// Decodes one coded sample to interleaved f32.
pub(crate) trait SoundDecoder: Send {
    fn decode(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String>;
    /// Decodes sample `index` of the track (decoders that read other tracks beside it need
    /// the index).
    fn decode_at(
        &mut self,
        _index: usize,
        sample: &[u8],
        out: &mut Vec<f32>,
    ) -> Result<(), String> {
        self.decode(sample, out)
    }
    /// Sample `index` is missing or would not decode: rebuilds it from `next` (the sample
    /// after it) where the codec carries a copy there (Opus in-band FEC), else conceals it.
    /// None: the decoder has no such way (a missing sample is then decoded as empty).
    fn decode_lost(
        &mut self,
        _index: usize,
        _next: Option<&[u8]>,
        _out: &mut Vec<f32>,
    ) -> Option<Result<(), String>> {
        None
    }
    fn reset(&mut self);
    /// The end of the track: what the decoder still holds (a last frame without a header
    /// after it).
    fn flush(&mut self, _out: &mut Vec<f32>) {}
    /// Coded samples to decode before the one wanted after a seek (overlap of transforms).
    fn preroll(&self) -> usize {
        0
    }
}

/// DV sound: the PCM in a DV frame's audio blocks, at the track's channel count (a frame
/// with fewer channels is padded with silence).
struct DvSound {
    channels: usize,
}

impl SoundDecoder for DvSound {
    fn decode(&mut self, frame: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        let a = specular_dv::audio(frame).ok_or("a DV frame without sound")?;
        let ch = usize::from(a.channels.max(1));
        for f in a.samples.chunks_exact(ch) {
            for c in 0..self.channels {
                out.push(f.get(c).map_or(0.0, |&s| f32::from(s) / 32768.0));
            }
        }
        Ok(())
    }

    fn reset(&mut self) {}
}

struct Pcm {
    bits: u16,
    float: bool,
    little_endian: bool,
}

impl SoundDecoder for Pcm {
    fn decode(&mut self, b: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        let n = usize::from(self.bits / 8).max(1);
        let le = self.little_endian;
        out.reserve(b.len() / n);
        for s in b.chunks_exact(n) {
            let v = match (self.bits, self.float) {
                // "Little-endian" 8-bit samples are the unsigned ones (`raw `).
                (8, _) if le => (f32::from(s[0]) - 128.0) / 128.0,
                (8, _) => f32::from(s[0] as i8) / 128.0,
                (16, _) => {
                    let a = [s[0], s[1]];
                    f32::from(if le {
                        i16::from_le_bytes(a)
                    } else {
                        i16::from_be_bytes(a)
                    }) / 32768.0
                }
                (24, _) => {
                    let (hi, mid, lo) = if le {
                        (s[2], s[1], s[0])
                    } else {
                        (s[0], s[1], s[2])
                    };
                    (i32::from_be_bytes([hi, mid, lo, 0]) >> 8) as f32 / 8_388_608.0
                }
                (32, false) => {
                    let a = [s[0], s[1], s[2], s[3]];
                    let v = if le {
                        i32::from_le_bytes(a)
                    } else {
                        i32::from_be_bytes(a)
                    };
                    (f64::from(v) / 2_147_483_648.0) as f32
                }
                (32, true) => {
                    let a = [s[0], s[1], s[2], s[3]];
                    if le {
                        f32::from_le_bytes(a)
                    } else {
                        f32::from_be_bytes(a)
                    }
                }
                (64, true) => {
                    let a: [u8; 8] = s.try_into().map_err(|_| "short sample")?;
                    (if le {
                        f64::from_le_bytes(a)
                    } else {
                        f64::from_be_bytes(a)
                    }) as f32
                }
                _ => return Err(format!("{}-bit PCM", self.bits)),
            };
            out.push(if v.is_finite() {
                v.clamp(-1.0, 1.0)
            } else {
                0.0
            });
        }
        Ok(())
    }
    fn reset(&mut self) {}
}

/// MPEG audio (`specular-mpa`): frames are found across samples. `preroll` is how many
/// samples to decode before a seek's target.
struct Mpa(specular_mpa::Decoder, usize);

impl SoundDecoder for Mpa {
    fn decode(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        self.0.decode(sample, out).map_err(|e| e.to_string())
    }
    fn reset(&mut self) {
        self.0.reset();
    }
    fn flush(&mut self, out: &mut Vec<f32>) {
        let _ = self.0.flush(out);
    }
    fn preroll(&self) -> usize {
        self.1
    }
}

/// Fills in a sound track's facts the container leaves out, from its configuration or first
/// samples: MPEG audio's layer, AC-3 against E-AC-3 and their channels (7.1 with E-AC-3's
/// extension), DTS's channels and rate (and DTS carried as 16-bit PCM), FLAC's and
/// Vorbis's channels and rate.
pub fn complete_sound(data: &[u8], t: &mut AudioTrack) {
    match &t.codec {
        AudioCodec::Ac3 | AudioCodec::Eac3 => crate::ac3_sound::complete(data, t),
        c if crate::dts_sound::is_dts(c) => crate::dts_sound::complete(data, t),
        c if crate::amr_sound::is_amr(c) => crate::amr_sound::complete(data, t),
        AudioCodec::Pcm { bits: 16, .. } => crate::dts_sound::complete(data, t),
        AudioCodec::Flac(c) => {
            let c = c.clone();
            crate::xiph_sound::complete_flac(data, t, &c);
        }
        AudioCodec::Alac(c) => {
            if let Some((ch, rate)) = crate::alac_sound::facts(c) {
                t.channels = ch;
                if rate > 0 {
                    t.sample_rate = rate;
                }
            }
        }
        AudioCodec::Vorbis(h) => {
            if let Some((ch, rate)) = crate::xiph_sound::vorbis_facts(h) {
                t.channels = ch;
                t.sample_rate = rate;
            }
        }
        _ => complete_mpa(data, t),
    }
}

/// Fills in an MPEG audio track's layer, channels and rate from its first samples (MP4 and
/// transport streams say "MPEG audio" without the layer).
fn complete_mpa(data: &[u8], t: &mut AudioTrack) {
    if !matches!(t.codec, AudioCodec::Mpa(_)) {
        return;
    }
    let mut head = Vec::new();
    for s in t.samples.iter().take(8) {
        if let Some(b) = t.sample_data(data, s) {
            head.extend_from_slice(&b);
        }
        if head.len() > 8192 {
            break;
        }
    }
    if let Some(h) = specular_mpa::probe(&head) {
        t.codec = AudioCodec::Mpa(h.layer);
        t.channels = h.channels() as u16;
        t.sample_rate = h.sample_rate;
    }
}

/// The decoder of a sound track with its output channels and rate, or why there is none.
/// `data` is the movie's bytes (some decoders read the first samples to set up). `multi`:
/// surround comes out as 5.1 or 7.1 ([`crate::speakers::source_layout`]) for a device
/// that plays it; otherwise decoders that fold with the stream's own mix levels (AC-3,
/// DTS) fold to stereo.
fn decoder_for(
    t: &AudioTrack,
    data: &[u8],
    multi: bool,
) -> Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
    let (dec, ch, rate) = any_decoder_for(t, data, multi)?;
    plausible(ch, rate)?;
    if !t.extra.companions.is_empty() {
        return Merged::wrap(t, dec, ch, rate);
    }
    Ok((dec, ch, rate))
}

/// A track and its companions (MXF: one mono track per channel) played as one: sample `k`
/// of each, side by side.
struct Merged {
    main: Box<dyn SoundDecoder>,
    main_ch: usize,
    others: Vec<Companion>,
    scratch: Vec<Vec<f32>>,
}

/// A companion track: its file, the track, its decoder and channels.
type Companion = (Arc<Source>, AudioTrack, Box<dyn SoundDecoder>, usize);

impl Merged {
    fn wrap(
        t: &AudioTrack,
        main: Box<dyn SoundDecoder>,
        ch: u16,
        rate: u32,
    ) -> Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
        let mut others = Vec::new();
        let mut total = ch;
        for c in &t.extra.companions {
            // Companions are opened with their file by [`AudioPlayer::open`].
            let Some(src) = c.extra.source.as_ref() else {
                continue;
            };
            let Ok((d, cch, crate_)) = any_decoder_for(c, src.0.bytes(), false) else {
                continue;
            };
            if crate_ != rate || total + cch > MAX_CHANNELS {
                continue;
            }
            total += cch;
            others.push((Arc::clone(&src.0), c.clone(), d, usize::from(cch)));
        }
        let n = others.len() + 1;
        Ok((
            Box::new(Merged {
                main,
                main_ch: usize::from(ch),
                others,
                scratch: vec![Vec::new(); n],
            }),
            total,
            rate,
        ))
    }
}

impl SoundDecoder for Merged {
    fn decode(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        self.main.decode(sample, out)
    }
    fn decode_at(&mut self, index: usize, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        for s in self.scratch.iter_mut() {
            s.clear();
        }
        self.main.decode_at(index, sample, &mut self.scratch[0])?;
        for (k, (src, t, d, _)) in self.others.iter_mut().enumerate() {
            let bytes = t
                .samples
                .get(index)
                .and_then(|s| t.sample_data(src.bytes(), s))
                .unwrap_or_default();
            // A companion that fails is silent beside the others.
            let _ = d.decode_at(index, &bytes, &mut self.scratch[k + 1]);
        }
        let mut chans = vec![self.main_ch];
        chans.extend(self.others.iter().map(|o| o.3));
        let frames = self.scratch[0].len() / self.main_ch.max(1);
        for i in 0..frames {
            for (k, &c) in chans.iter().enumerate() {
                for j in 0..c {
                    out.push(self.scratch[k].get(i * c + j).copied().unwrap_or(0.0));
                }
            }
        }
        Ok(())
    }
    fn reset(&mut self) {
        self.main.reset();
        for o in self.others.iter_mut() {
            o.2.reset();
        }
    }
    fn flush(&mut self, out: &mut Vec<f32>) {
        self.main.flush(out);
    }
    fn preroll(&self) -> usize {
        self.main.preroll()
    }
}

/// The track with its companions given the files their samples are in (the movie's
/// `source` unless they say).
fn with_sources(t: &AudioTrack, source: &Arc<Source>) -> AudioTrack {
    let mut t = t.clone();
    for c in t.extra.companions.iter_mut() {
        if c.extra.source.is_none() {
            c.extra.source = Some(crate::mp4::External(Arc::clone(source)));
        }
    }
    t
}

/// The lowest and highest sample rates and the most channels played: a file's rate of 1 Hz
/// would make the resampler write 48 000 samples for each one decoded, and 65 535 channels
/// a gigabyte of silence before the sound starts.
const MIN_RATE: u32 = 1000;
const MAX_RATE: u32 = 768_000;
const MAX_CHANNELS: u16 = 64;

fn plausible(ch: u16, rate: u32) -> Result<(), Silence> {
    if (MIN_RATE..=MAX_RATE).contains(&rate) && (1..=MAX_CHANNELS).contains(&ch) {
        Ok(())
    } else {
        Err(Silence::Unsupported(format!("{ch}-channel {rate} Hz")))
    }
}

fn any_decoder_for(
    t: &AudioTrack,
    data: &[u8],
    multi: bool,
) -> Result<(Box<dyn SoundDecoder>, u16, u32), Silence> {
    let rate = t.sample_rate;
    if rate == 0 || t.samples.is_empty() {
        return Err(Silence::NoTrack);
    }
    match &t.codec {
        AudioCodec::Pcm {
            bits,
            float,
            little_endian,
        } => Ok((
            Box::new(Pcm {
                bits: *bits,
                float: *float,
                little_endian: *little_endian,
            }),
            t.channels.max(1),
            rate,
        )),
        AudioCodec::Aac(asc) => crate::aac_sound::decoder(t, asc, data),
        AudioCodec::Opus(head) => crate::opus_sound::decoder(t, head, multi),
        AudioCodec::Ac3 | AudioCodec::Eac3 => crate::ac3_sound::decoder(t, data, multi),
        c if crate::dts_sound::is_dts(c) => crate::dts_sound::decoder(t, data, multi),
        c if crate::amr_sound::is_amr(c) => crate::amr_sound::decoder(t, data),
        c if crate::qcelp_sound::is_speech(c) => crate::qcelp_sound::decoder(t),
        c if crate::mpc_sound::is_mpc(c) => crate::mpc_sound::decoder(t, data),
        c if crate::tta_sound::is_tta(c) => crate::tta_sound::decoder(t),
        c if crate::shn_sound::is_shn(c) => crate::shn_sound::decoder(t, data),
        c if crate::dsd_files::is_dsd(c) => crate::dsd_files::decoder(t),
        c if crate::pcm_files::is_svx(c) => crate::pcm_files::svx_decoder(t, data),
        c if crate::tracker_sound::is_tracker(c) => crate::tracker_sound::decoder(t, data),
        c if crate::speech_files::is_gsm(c) => crate::speech_files::gsm_decoder(t),
        c if crate::qoa_sound::is_qoa(c) => crate::qoa_sound::decoder(t),
        c if crate::synth_sound::is_midi(c) => crate::synth_sound::midi_decoder(t, data),
        c if crate::synth_sound::is_bank(c) => crate::synth_sound::bank_decoder(t, data),
        AudioCodec::Vorbis(headers) => crate::xiph_sound::vorbis(headers, multi),
        AudioCodec::Flac(config) => crate::xiph_sound::flac(t, config, multi),
        AudioCodec::Dv => Ok((
            Box::new(DvSound {
                channels: usize::from(t.channels.max(1)),
            }),
            t.channels.max(1),
            rate,
        )),
        AudioCodec::Alac(config) => crate::alac_sound::decoder(t, config),
        AudioCodec::WavPack { .. } => crate::lossless_sound::wavpack(t, data),
        AudioCodec::Ape { .. } => crate::lossless_sound::ape(t, data),
        AudioCodec::Other([b'D', b'V', b'D', b'L'], bits @ (20 | 24)) => Ok((
            Box::new(crate::wave_sound::DvdLpcm::new(
                u16::from(*bits),
                t.channels,
                t.samples.iter().map(|s| s.size),
            )),
            t.channels.max(1),
            rate,
        )),
        AudioCodec::Speex(header) => crate::speex_sound::decoder(t, header),
        AudioCodec::Other([b'n', b'm', b'o', b's'], _) => {
            Ok((Box::new(crate::nelly_sound::Nellymoser::new()), 1, rate))
        }
        AudioCodec::Other([b's', b'w', b'f', b'a'], _) => Ok((
            Box::new(crate::wave_sound::SwfAdpcm {
                channels: usize::from(t.channels.clamp(1, 2)),
            }),
            t.channels.clamp(1, 2),
            rate,
        )),
        AudioCodec::G711 { alaw } => Ok((
            Box::new(crate::wave_sound::G711 { alaw: *alaw }),
            t.channels.max(1),
            rate,
        )),
        AudioCodec::Adpcm { kind, block } => {
            let d = crate::wave_sound::Adpcm::new(kind.clone(), t.channels, *block)
                .map_err(|e| Silence::Unsupported(format!("{} ({e})", t.codec.name())))?;
            Ok((Box::new(d), t.channels.max(1), rate))
        }
        // Layers I and II need the filter bank's memory and a frame begun in the sample
        // before; MP3 its bit reservoir too, up to 511 bytes back (10 frames at 8 kbit/s).
        AudioCodec::Mpa(layer @ 1..=3) => Ok((
            Box::new(Mpa(
                // The track's channels whatever a frame's mode (MP3 streams switch).
                specular_mpa::Decoder::with_channels(usize::from(t.channels.clamp(1, 2))),
                if *layer == 3 { 10 } else { 1 },
            )),
            t.channels.clamp(1, 2),
            rate,
        )),
        other => Err(Silence::Unsupported(other.name())),
    }
}

/// Decodes a whole sound track in order without a device: interleaved samples at the
/// decoder's channel count and rate (for oracle tests and tools; playback goes through
/// [`AudioPlayer`]). Damaged samples decode as the decoder's silence.
pub fn decode_track(t: &AudioTrack, source: &Arc<Source>) -> Result<(Vec<f32>, u16, u32), Silence> {
    decode_whole(t, source, false)
}

/// [`decode_track`] as a device with surround speakers gets it: 5.1 and 7.1 as such (the
/// WAVE order, [`crate::speakers::source_layout`]) instead of folded by the stream's own
/// mix levels.
pub fn decode_track_surround(
    t: &AudioTrack,
    source: &Arc<Source>,
) -> Result<(Vec<f32>, u16, u32), Silence> {
    decode_whole(t, source, true)
}

/// Sample `k` of `t` (in `data`) decoded into `out`: one that is missing (`bytes` None or
/// empty) or will not decode is rebuilt from the sample after it or concealed where the
/// decoder can ([`SoundDecoder::decode_lost`]); otherwise a missing sample is decoded as
/// empty when `empty` (else adds nothing), and a damaged one gives its error.
fn decode_sample(
    dec: &mut dyn SoundDecoder,
    t: &AudioTrack,
    data: &[u8],
    k: usize,
    bytes: Option<&[u8]>,
    empty: bool,
    out: &mut Vec<f32>,
) -> Result<(), String> {
    let at = out.len();
    let lost = |dec: &mut dyn SoundDecoder, out: &mut Vec<f32>| {
        let next = t
            .samples
            .get(k + 1)
            .and_then(|s| t.sample_data(data, s))
            .filter(|b| !b.is_empty());
        dec.decode_lost(k, next.as_deref(), out)
    };
    match bytes.filter(|b| !b.is_empty()) {
        Some(b) => dec.decode_at(k, b, out).or_else(|e| {
            out.truncate(at);
            match lost(dec, out) {
                Some(Ok(())) => {
                    specular_log::debug!(target: specular_log::DECODE, "video: sound sample {k} rebuilt: {e}");
                    Ok(())
                }
                _ => {
                    out.truncate(at);
                    Err(e)
                }
            }
        }),
        None => match lost(dec, out) {
            Some(r) => r,
            None if empty => dec.decode_at(k, &[], out),
            None => Ok(()),
        },
    }
}

fn decode_whole(
    t: &AudioTrack,
    source: &Arc<Source>,
    multi: bool,
) -> Result<(Vec<f32>, u16, u32), Silence> {
    let source = t.extra.source.as_ref().map_or(source, |e| &e.0);
    let t = &with_sources(t, source);
    let data = source.bytes();
    let (mut dec, ch, rate) = decoder_for(t, data, multi)?;
    let mut out = Vec::new();
    for (k, s) in t.samples.iter().enumerate() {
        let b = t.sample_data(data, s);
        decode_sample(&mut *dec, t, data, k, b.as_deref(), b.is_some(), &mut out)
            .map_err(Silence::Unsupported)?;
    }
    dec.flush(&mut out);
    Ok((out, ch, rate))
}

/// A track's decoder driven sample by sample, for analysis (a sound file's waveform and
/// spectrogram): decoded channels and rate, and seeks by reset and pre-roll.
pub(crate) struct TrackDecoder {
    dec: Box<dyn SoundDecoder>,
    pub ch: u16,
    pub rate: u32,
    track: AudioTrack,
    source: Arc<Source>,
}

impl TrackDecoder {
    pub fn new(t: &AudioTrack, source: &Arc<Source>) -> Result<TrackDecoder, Silence> {
        let source = Arc::clone(t.extra.source.as_ref().map_or(source, |e| &e.0));
        let track = with_sources(t, &source);
        let (dec, ch, rate) = decoder_for(&track, source.bytes(), false)?;
        Ok(TrackDecoder {
            dec,
            ch,
            rate,
            track,
            source,
        })
    }

    pub fn len(&self) -> usize {
        self.track.samples.len()
    }

    pub fn preroll(&self) -> usize {
        self.dec.preroll()
    }

    pub fn reset(&mut self) {
        self.dec.reset();
    }

    /// Appends sample `k`'s sound (the decoder's remainder after the last one); a damaged
    /// sample adds nothing.
    pub fn decode(&mut self, k: usize, out: &mut Vec<f32>) {
        let Some(s) = self.track.samples.get(k) else {
            return;
        };
        let data = self.source.bytes();
        let b = self.track.sample_data(data, s);
        let at = out.len();
        if decode_sample(
            &mut *self.dec,
            &self.track,
            data,
            k,
            b.as_deref(),
            b.is_some(),
            out,
        )
        .is_err()
        {
            out.truncate(at);
        }
        if k + 1 == self.track.samples.len() {
            self.dec.flush(out);
        }
    }
}

/// Whether a track's sound can be decoded, or why not (for the info panel, without opening a
/// device).
pub fn playable(t: &AudioTrack) -> Result<(), Silence> {
    if t.sample_rate == 0 || t.samples.is_empty() {
        return Err(Silence::NoTrack);
    }
    match &t.codec {
        AudioCodec::Aac(asc) => crate::aac_sound::check(asc),
        _ => decoder_for(t, &[], false).map(|_| ()),
    }
}

/// Rate conversion by 4-point cubic (Catmull-Rom) interpolation, carried across chunks.
struct Resampler {
    ch: usize,
    /// Input frames per output frame.
    step: f64,
    /// Position of the next output frame, in input frames from the start of `hist`.
    pos: f64,
    /// Input not yet passed (interleaved), starting one frame before the interpolation point.
    hist: Vec<f32>,
}

impl Resampler {
    fn new(ch: usize, from: u32, to: u32) -> Resampler {
        Resampler {
            ch: ch.max(1),
            step: f64::from(from) / f64::from(to.max(1)),
            pos: 1.0,
            hist: Vec::new(),
        }
    }

    fn reset(&mut self) {
        self.pos = 1.0;
        self.hist.clear();
    }

    fn run(&mut self, input: &[f32], out: &mut Vec<f32>) {
        if self.step == 1.0 {
            out.extend_from_slice(input);
            return;
        }
        let ch = self.ch;
        if self.hist.is_empty() {
            // Repeat the first frame so interpolation starts on it.
            self.hist.extend_from_slice(&input[..ch.min(input.len())]);
        }
        self.hist.extend_from_slice(input);
        let frames = self.hist.len() / ch;
        let at = |i: usize, c: usize| self.hist[i.min(frames - 1) * ch + c];
        while self.pos + 2.0 < frames as f64 {
            let i = self.pos as usize;
            let t = (self.pos - i as f64) as f32;
            for c in 0..ch {
                let (p0, p1, p2, p3) = (at(i - 1, c), at(i, c), at(i + 1, c), at(i + 2, c));
                let a = -0.5 * p0 + 1.5 * p1 - 1.5 * p2 + 0.5 * p3;
                let b = p0 - 2.5 * p1 + 2.0 * p2 - 0.5 * p3;
                let cc = -0.5 * p0 + 0.5 * p2;
                out.push(((a * t + b) * t + cc) * t + p1);
            }
            self.pos += self.step;
        }
        // Keep from one frame before the next interpolation point.
        let keep = (self.pos as usize).saturating_sub(1).min(frames);
        self.hist.drain(..keep * ch);
        self.pos -= keep as f64;
    }
}

/// Seconds of decoded sound kept ahead of the device.
const AHEAD: Duration = Duration::from_millis(300);

/// Slowest and fastest sound speeds.
pub const MIN_SPEED: f64 = 0.25;
pub const MAX_SPEED: f64 = 4.0;

/// Levelled-in gain changes of measured files: at most this many dB per second.
const MEASURED_SLEW_DB: f64 = 1.0;
/// Seconds measured before a measured gain is used.
const MEASURE_FIRST_S: f64 = 3.0;

struct Feed {
    /// Device-format frames, interleaved; the first is frame `base` from the seek target
    /// `origin_ns` (media time).
    queue: VecDeque<f32>,
    origin_ns: i64,
    base: u64,
    /// The frame (counted like `base`) the file being heard started at: media time is
    /// `origin_ns` plus the time since it.
    seg_start: u64,
    /// The frame the next file is heard from (it is in the queue), and whether that file
    /// carries its own dynamic range control.
    splice: Option<(u64, bool)>,
    /// Files spliced in and heard since asked ([`AudioPlayer::take_advanced`]).
    advanced: u32,
    /// Heard files the decoder still holds (it lets them go).
    retired: u32,
    /// The next file asked for, not yet taken by the decoder (`Some(None)`: none).
    next_req: Option<Option<Arc<Source>>>,
    /// The next file asked for is the next piece of one stream ([`AudioPlayer::set_next_piece`]).
    next_piece: bool,
    /// The decoder holds a next file (opened and primed).
    has_next: bool,
    /// Media seconds per second heard.
    speed: f64,
    /// The time-stretcher (made once the device's format is known), and whether it is
    /// producing what is heard (otherwise queued frames go out as they are).
    stretch: Option<Stretcher>,
    stretching: bool,
    /// The sound as set, and the device-side stages (made with the device's format).
    sound: SoundSettings,
    chain: Option<Chain>,
    /// The file heard carries its own dynamic range control (AC-3, E-AC-3).
    stream_drc: bool,
    /// The levelling gain of the file heard (dB) and its source, and whether any queued
    /// sound was turned up (the limiter runs).
    applied: Option<(f64, LevelSource)>,
    boosted: bool,
    /// Bumped by each seek; the decoder repositions when it sees a new one.
    generation: u64,
    target_ns: i64,
    running: bool,
    volume: f32,
    muted: bool,
    /// The limiter's gain (1: not limiting), for volumes over 100 %.
    limit: f32,
    /// The last buffer's first frame: heard at this instant, media time this, the clock
    /// running at this speed.
    anchor: Option<(Instant, i64, f64)>,
    /// The decoder has passed the end of the sound (for this generation).
    ended: bool,
    quit: bool,
    /// The decoder panicked (its brief), not said yet.
    failed: Option<String>,
    /// The heard file's decoded channels and rate, and whether it is downmixed.
    source: (u16, u32, bool),
    /// A fade under way, and a fade in armed for the next start.
    fade: Option<Fade>,
    fade_in: bool,
    /// The heard buffer folded to stereo for the live tap (surround output).
    tap_stereo: Vec<f32>,
}

/// How long a fade in or out takes (a device change).
pub const FADE: Duration = Duration::from_millis(30);

/// A fade in or out, `done` frames into it.
#[derive(Clone, Copy, Debug)]
struct Fade {
    out: bool,
    done: usize,
}

impl Fade {
    /// Applies the fade to `buf` (`ch` interleaved channels at `rate`); false once a fade
    /// in is complete (it goes).
    fn apply(&mut self, buf: &mut [f32], ch: usize, rate: u32) -> bool {
        let len = (FADE.as_secs_f64() * f64::from(rate)).max(1.0) as usize;
        for f in buf.chunks_exact_mut(ch.max(1)) {
            let p = (self.done as f32 / len as f32).min(1.0);
            let g = if self.out { 1.0 - p } else { p };
            if g != 1.0 {
                for s in f {
                    *s *= g;
                }
            }
            self.done = (self.done + 1).min(len);
        }
        self.out || self.done < len
    }
}

struct Shared {
    feed: Mutex<Feed>,
    /// Wakes the decoder (room in the queue, a seek, quitting).
    wake: Condvar,
    /// What is heard (after the whole chain), for the sound page's live displays.
    tap: Arc<crate::live_tap::Tap>,
}

/// Plays a movie's sound track.
pub struct AudioPlayer {
    shared: Arc<Shared>,
    out: Box<dyn AudioOutput>,
    format: DeviceFormat,
    thread: Option<JoinHandle<()>>,
    lost_said: AtomicBool,
}

/// Where the decoder is.
#[derive(Clone, Copy, Debug, Default)]
struct Cursor {
    sample: usize,
    /// Track-rate frames still to drop from the decoded output (landing mid-sample).
    skip: u64,
    /// Coded samples whose output is dropped entirely (decoder pre-roll).
    drop: usize,
    /// Track-rate frames of silence before the sound starts (an edit's empty lead-in).
    lead: u64,
}

/// One file's sound in the decoder: the heard one, and the next one primed for gapless play.
struct Segment {
    source: Arc<Source>,
    track: AudioTrack,
    dec: Box<dyn SoundDecoder>,
    /// Decoded channels and rate.
    ch: usize,
    rate: u32,
    video_first_us: i64,
    /// The decoded channels to the device's.
    map: Mapping,
    /// The channels the sound has (more than `ch` when the decoder folds it itself).
    surround: usize,
    resampler: Resampler,
    cur: Cursor,
    /// The tags levelling reads, and the album the file is from (album and album artist).
    tags: GainTags,
    album: Option<String>,
    /// The stream has its own dynamic range control.
    stream_drc: bool,
    /// Loudness measured as it plays (for untagged files, when asked).
    meter: Option<Meter>,
    /// The levelling gain last applied (linear) and its dB.
    gain: f32,
    gain_set: bool,
    gain_db: Option<f64>,
    applied: Option<(f64, LevelSource)>,
    /// Decoded from since the last seek.
    started: bool,
    /// The next file is the next piece of this one's stream, coded alike: the decoder goes
    /// on into it ([`AudioPlayer::set_next_piece`]).
    continues: bool,
    /// Device frames from the start of the file to the decoder's place, and in all
    /// (estimated from the track's length).
    pos: u64,
    len: u64,
}

impl Segment {
    fn new(
        source: Arc<Source>,
        movie: &Movie,
        video_first_us: i64,
        format: DeviceFormat,
        order: DeviceOrder,
        multi: bool,
    ) -> Result<Segment, Silence> {
        let track = movie.audio.as_ref().ok_or(Silence::NoTrack)?;
        // An OP-Atom MXF's sound is in a file of its own.
        let source = track
            .extra
            .source
            .as_ref()
            .map_or(source, |e| Arc::clone(&e.0));
        let track = with_sources(track, &source);
        let (dec, ch, rate) = decoder_for(&track, source.bytes(), multi)?;
        let tags = crate::sound_tags::read(source.bytes());
        let album = tags.get("album").map(|a| {
            format!(
                "{a}\u{1f}{}",
                tags.get("album-artist")
                    .or_else(|| tags.get("artist"))
                    .unwrap_or_default()
            )
        });
        let ts = u128::from(track.timescale.max(1));
        let len = match track.edit.length_us {
            Some(us) => u128::from(us) * u128::from(format.rate) / 1_000_000,
            None => {
                let end = track
                    .samples
                    .last()
                    .map_or(0, |s| s.dts.saturating_add(u64::from(s.duration)))
                    .saturating_sub(track.edit.start.max(0) as u64);
                u128::from(end) * u128::from(format.rate) / ts
            }
        } as u64;
        Ok(Segment {
            stream_drc: matches!(track.codec, AudioCodec::Ac3 | AudioCodec::Eac3),
            source,
            dec,
            ch: usize::from(ch),
            rate,
            video_first_us,
            map: Mapping::new(usize::from(ch), usize::from(format.channels), order, 1.0),
            surround: usize::from(ch),
            resampler: Resampler::new(usize::from(format.channels), rate, format.rate),
            cur: Cursor::default(),
            tags: tags.gain,
            album,
            meter: None,
            gain: 1.0,
            gain_set: false,
            gain_db: None,
            applied: None,
            started: false,
            continues: false,
            pos: 0,
            len,
            track,
        })
    }

    /// The file's sound for a device of `format` and `order`: as 5.1 or 7.1 when the
    /// device plays it so, else folded (by the stream's own mix levels where it has them).
    fn for_device(
        source: Arc<Source>,
        movie: &Movie,
        video_first_us: i64,
        format: DeviceFormat,
        order: DeviceOrder,
    ) -> Result<Segment, Silence> {
        let multi = Segment::new(
            Arc::clone(&source),
            movie,
            video_first_us,
            format,
            order,
            true,
        )?;
        if multi.map.discrete() {
            return Ok(multi);
        }
        let mut s = Segment::new(source, movie, video_first_us, format, order, false)?;
        s.surround = multi.ch;
        Ok(s)
    }

    /// The sound's channels and rate, and whether it reaches fewer speakers than it has.
    fn heard_as(&self) -> (u16, u32, bool) {
        (
            self.surround as u16,
            self.rate,
            self.map.downmixes() || self.surround > self.ch,
        )
    }

    /// Takes over the decoder and resampler of `prev`, the piece before it in one stream,
    /// at their state after its last sample, and starts at its own first sample (nothing
    /// dropped, no pre-roll, no lead-in).
    fn go_on_from(&mut self, prev: &mut Segment) {
        std::mem::swap(&mut self.dec, &mut prev.dec);
        std::mem::swap(&mut self.resampler, &mut prev.resampler);
        prev.continues = false;
        self.cur = Cursor::default();
        self.started = false;
        self.pos = 0;
    }

    /// Past the last sample (and any lead-in).
    fn at_end(&self) -> bool {
        self.cur.sample >= self.track.samples.len() && self.cur.lead == 0
    }

    /// Moves to `target_ns` from the first video frame.
    fn seek(&mut self, target_ns: i64, device_rate: u32) {
        self.cur = self.locate(target_ns);
        self.started = false;
        self.dec.reset();
        self.resampler.reset();
        self.pos = (i128::from(target_ns.max(0)) * i128::from(device_rate) / 1_000_000_000) as u64;
    }

    /// Decodes the next coded sample (or a tenth of a second of lead-in silence) into
    /// `pcm` at the track's channels and rate; false when nothing was made for the output
    /// (pre-roll).
    fn decode_next(&mut self, pcm: &mut Vec<f32>) -> bool {
        pcm.clear();
        if self.cur.lead > 0 {
            let n = self.cur.lead.min(u64::from(self.rate / 10).max(1));
            self.cur.lead -= n;
            pcm.resize(n as usize * self.ch, 0.0);
            return true;
        }
        let Some(s) = self.track.samples.get(self.cur.sample) else {
            return true;
        };
        self.cur.sample += 1;
        let data = self.source.bytes();
        let bytes = self.track.sample_data(data, s);
        let k = self.cur.sample - 1;
        if let Err(e) = decode_sample(
            &mut *self.dec,
            &self.track,
            data,
            k,
            bytes.as_deref(),
            true,
            pcm,
        ) {
            specular_log::debug!(target: specular_log::DECODE, "video: sound sample {}: {e}", self.cur.sample - 1);
            pcm.clear();
        }
        if self.cur.sample == self.track.samples.len() && !self.continues {
            // (A decoder going on into the next piece keeps what it holds for it.)
            self.dec.flush(pcm);
        }
        if self.cur.drop > 0 {
            self.cur.drop -= 1;
            return false;
        }
        let skip = (self.cur.skip as usize * self.ch).min(pcm.len());
        self.cur.skip -= (skip / self.ch) as u64;
        pcm.drain(..skip);
        true
    }

    /// Where to decode from to be heard at `target_ns` from the first video frame.
    fn locate(&self, target_ns: i64) -> Cursor {
        let t = &self.track;
        let ts = i128::from(t.timescale.max(1));
        let movie_us = i128::from(target_ns / 1000) + i128::from(self.video_first_us);
        let delay = i128::from(t.edit.delay_us);
        let rate = i128::from(self.rate);
        if movie_us < delay {
            // Before the sound starts: silence until it does.
            let lead = ((delay - movie_us) * rate / 1_000_000) as u64;
            return Cursor {
                sample: t
                    .samples
                    .partition_point(|s| (s.dts as i128) < i128::from(t.edit.start)),
                skip: 0,
                drop: 0,
                lead,
            }
            .with_start(self, i128::from(t.edit.start));
        }
        let ticks = (movie_us - delay) * ts / 1_000_000 + i128::from(t.edit.start);
        let k = t
            .samples
            .partition_point(|s| (s.dts as i128) <= ticks)
            .saturating_sub(1);
        let from = k.saturating_sub(self.dec.preroll());
        let into = (ticks - t.samples.get(k).map_or(0, |s| s.dts as i128)).max(0);
        Cursor {
            sample: from,
            skip: (into * rate / ts) as u64,
            drop: k - from,
            lead: 0,
        }
    }

    /// The levelling gain for what comes next (linear), measured loudness moving slowly
    /// over `secs` of sound.
    fn level(&mut self, l: &Levelling, secs: f64) -> (f32, Option<(f64, LevelSource)>) {
        let measured = self
            .meter
            .as_ref()
            .filter(|m| m.seconds() >= MEASURE_FIRST_S)
            .and_then(Meter::integrated);
        let Some((db, src)) = levelling::gain_db(&self.tags, l, measured) else {
            self.gain_db = None;
            self.applied = None;
            return (1.0, None);
        };
        let db = match (src, self.gain_db) {
            (LevelSource::Measured, Some(had)) => {
                let step = MEASURED_SLEW_DB * secs;
                had + (db - had).clamp(-step, step)
            }
            (LevelSource::Measured, None) => {
                let step = MEASURED_SLEW_DB * secs;
                db.clamp(-step, step)
            }
            _ => db,
        };
        self.gain_db = Some(db);
        self.applied = Some((db, src));
        (levelling::linear(db), Some((db, src)))
    }
}

impl AudioPlayer {
    /// The sound of `movie` (parsed with samples, from `source`'s bytes) for a video whose
    /// first frame is presented at `video_first_us` on the movie's timeline. Starts `out`
    /// paused (it plays silence).
    pub fn open(
        source: Arc<Source>,
        movie: &Movie,
        video_first_us: i64,
        out: Box<dyn AudioOutput>,
    ) -> Result<AudioPlayer, Silence> {
        AudioPlayer::open_with(
            source,
            movie,
            video_first_us,
            out,
            &SoundSettings::default(),
        )
    }

    /// [`AudioPlayer::open`] with the sound set from the first frame decoded.
    pub fn open_with(
        source: Arc<Source>,
        movie: &Movie,
        video_first_us: i64,
        mut out: Box<dyn AudioOutput>,
        sound: &SoundSettings,
    ) -> Result<AudioPlayer, Silence> {
        // The track's decoder first: a track that can't play doesn't open the device.
        movie.audio.as_ref().ok_or(Silence::NoTrack)?;
        let probe = DeviceFormat {
            rate: 48_000,
            channels: 2,
        };
        // Decoded as surround first: its channel count is what the device is asked for.
        let first = Segment::new(
            Arc::clone(&source),
            movie,
            video_first_us,
            probe,
            DeviceOrder::native(),
            true,
        )?;
        let stream_drc = first.stream_drc;
        let shared = Arc::new(Shared {
            feed: Mutex::new(Feed {
                queue: VecDeque::new(),
                origin_ns: 0,
                base: 0,
                seg_start: 0,
                splice: None,
                advanced: 0,
                retired: 0,
                next_req: None,
                next_piece: false,
                has_next: false,
                speed: 1.0,
                stretch: None,
                stretching: false,
                sound: sound.clone(),
                chain: None,
                stream_drc,
                applied: None,
                boosted: false,
                generation: 1,
                target_ns: 0,
                running: false,
                volume: 1.0,
                muted: false,
                limit: 1.0,
                anchor: None,
                ended: false,
                failed: None,
                quit: false,
                source: (first.ch as u16, first.rate, false),
                fade: None,
                fade_in: false,
                tap_stereo: Vec::new(),
            }),
            wake: Condvar::new(),
            tap: Arc::new(crate::live_tap::Tap::new()),
        });
        // The device's format is known once it starts; until then buffers are silence.
        let fmt = Arc::new(std::sync::OnceLock::<(usize, u32, DeviceOrder)>::new());
        let render: Render = {
            let shared = Arc::clone(&shared);
            let fmt = Arc::clone(&fmt);
            Box::new(move |buf: &mut [f32], heard: Instant| match fmt.get() {
                Some(&(ch, rate, order)) => fill_buffer(&shared, buf, heard, ch, rate, order),
                None => buf.fill(0.0),
            })
        };
        let format = out
            .start(render, first.ch as u16)
            .map_err(Silence::NoDevice)
            .and_then(|f| {
                if f.rate == 0 || f.channels == 0 {
                    Err(Silence::NoDevice("no output format".into()))
                } else {
                    Ok(f)
                }
            })?;
        let order = out.order();
        let mut first = first;
        first.map = Mapping::new(first.ch, usize::from(format.channels), order, 1.0);
        if !first.map.discrete() && first.ch > 2 {
            // Folded after all: by the stream's own mix levels where it has them.
            let surround = first.ch;
            first = Segment::new(
                Arc::clone(&source),
                movie,
                video_first_us,
                format,
                order,
                false,
            )?;
            first.surround = surround;
        } else {
            first.resampler = Resampler::new(usize::from(format.channels), first.rate, format.rate);
            first.len = first.len * u64::from(format.rate) / u64::from(probe.rate);
        }
        shared.feed.lock().source = first.heard_as();
        let _ = fmt.set((usize::from(format.channels), format.rate, order));
        let worker = Decoding {
            shared: Arc::clone(&shared),
            format,
            order,
            segs: VecDeque::from([first]),
            at: 0,
        };
        let sh = Arc::clone(&shared);
        let thread = specular_thread::builder("anim-audio")
            .spawn(move || {
                // A panic (a decoder bug on this file) silences the movie and says so; the
                // pictures play on.
                if let Err(p) = specular_core::isolate::run("movie sound", || worker.run()) {
                    let mut f = sh.feed.lock();
                    f.failed = Some(p.brief());
                    f.ended = true;
                    drop(f);
                    sh.wake.notify_all();
                }
            })
            .map_err(|e| Silence::NoDevice(e.to_string()))?;
        Ok(AudioPlayer {
            shared,
            out,
            format,
            thread: Some(thread),
            lost_said: AtomicBool::new(false),
        })
    }

    pub fn format(&self) -> DeviceFormat {
        self.format
    }

    /// The device, its format, and how the heard file's sound was fitted to it.
    pub fn output(&self) -> OutputInfo {
        let (source_channels, source_rate, downmixed) = self.shared.feed.lock().source;
        OutputInfo {
            device: self.out.device(),
            format: self.format,
            source_channels,
            source_rate,
            resampled: source_rate != self.format.rate,
            downmixed,
        }
    }

    /// The next start (play or seek) fades in over [`FADE`] instead of starting at full
    /// level: the sound moved to another device goes on without a click.
    pub fn fade_in(&self) {
        self.shared.feed.lock().fade_in = true;
    }

    /// Fades the sound out over [`FADE`], then plays silence: the player can then go
    /// without a click. The clock runs on.
    pub fn fade_out(&self) {
        let mut f = self.shared.feed.lock();
        if !f.fade.is_some_and(|x| x.out) {
            f.fade = Some(Fade { out: true, done: 0 });
        }
    }

    /// Plays from `at` (time from the first video frame).
    pub fn play(&self, at: Duration) {
        let mut f = self.shared.feed.lock();
        reposition(&mut f, at);
        f.running = true;
        drop(f);
        self.shared.wake.notify_all();
    }

    /// Stops; the device plays silence and the clock stops.
    pub fn pause(&self) {
        let mut f = self.shared.feed.lock();
        f.running = false;
        f.anchor = None;
    }

    /// Moves to `at` (playing on if playing).
    pub fn seek(&self, at: Duration) {
        let mut f = self.shared.feed.lock();
        reposition(&mut f, at);
        drop(f);
        self.shared.wake.notify_all();
    }

    pub fn is_playing(&self) -> bool {
        self.shared.feed.lock().running
    }

    /// Volume 0..=2 (up to 1 applied as its square: even steps sound even; above, a boost
    /// up to twice as loud, with a limiter so loud passages don't clip).
    pub fn set_volume(&self, v: f32) {
        self.shared.feed.lock().volume = if v.is_finite() {
            v.clamp(0.0, MAX_VOLUME)
        } else {
            1.0
        };
    }

    pub fn set_muted(&self, muted: bool) {
        self.shared.feed.lock().muted = muted;
    }

    /// The sound as the listener sets it: the equaliser, dialogue boost, balance and mono
    /// apply from the next device buffer; levelling and the centre lift from what is decoded
    /// next (within [`AHEAD`]).
    pub fn set_sound(&self, s: &SoundSettings) {
        let mut f = self.shared.feed.lock();
        if f.sound == *s {
            return;
        }
        f.sound = s.clone();
        if let Some(c) = &mut f.chain {
            c.set(s);
        }
    }

    /// The file to play after this one without a gap (`None`: none): opened and primed
    /// now, spliced at the sample where this one ends (each file's encoder delay and
    /// padding trimmed), through the resampler when the rates differ; crossfaded when
    /// [`SoundSettings::crossfade`] is set, unless both files are from one album.
    /// [`AudioPlayer::take_advanced`] says when it is heard; the clock then runs from the
    /// next file's start.
    pub fn set_next(&self, next: Option<Arc<Source>>) {
        self.ask_next(next, false);
    }

    /// [`AudioPlayer::set_next`] for the next piece of one stream (HLS, DASH, a web file in
    /// pieces): when its sound is coded as this one's (same codec, configuration, channels
    /// and rate), the decoder goes on into it with its state, so the sound runs on across
    /// the boundary as one stream (no restart, no priming, no gap); otherwise as
    /// [`AudioPlayer::set_next`].
    pub fn set_next_piece(&self, next: Option<Arc<Source>>) {
        self.ask_next(next, true);
    }

    fn ask_next(&self, next: Option<Arc<Source>>, piece: bool) {
        let mut f = self.shared.feed.lock();
        f.next_piece = piece;
        // A next file already in the queue and not yet heard goes.
        if let Some((at, _)) = f.splice.take() {
            let keep = (at.saturating_sub(f.base) as usize * self.format.channels as usize)
                .min(f.queue.len());
            f.queue.truncate(keep);
        }
        f.next_req = Some(next);
        drop(f);
        self.shared.wake.notify_all();
    }

    /// A next file is primed and the app has not moved on to it.
    pub fn has_next(&self) -> bool {
        let f = self.shared.feed.lock();
        // A next file heard but not yet taken ([`AudioPlayer::take_advanced`]) counts: the
        // app has not moved on to it yet.
        f.has_next && f.next_req.is_none() || f.advanced > 0 || matches!(f.next_req, Some(Some(_)))
    }

    /// How many next files have started to be heard since last asked (the app moves on to
    /// them, keeping this player).
    pub fn take_advanced(&self) -> u32 {
        std::mem::take(&mut self.shared.feed.lock().advanced)
    }

    /// The levelling gain of the file heard (dB) and where it came from.
    pub fn levelling(&self) -> Option<(f64, LevelSource)> {
        self.shared.feed.lock().applied
    }

    /// Plays at `speed` (media seconds per second, [`MIN_SPEED`]..=[`MAX_SPEED`]) with the
    /// pitch kept, from the next device buffer; the clock runs at the speed. 1 plays the
    /// sound exactly as decoded.
    pub fn set_speed(&self, speed: f64) {
        let speed = if speed.is_finite() {
            speed.clamp(MIN_SPEED, MAX_SPEED)
        } else {
            1.0
        };
        let mut f = self.shared.feed.lock();
        if f.speed != speed {
            f.speed = speed;
            drop(f);
            // A faster speed keeps more ahead.
            self.shared.wake.notify_all();
        }
    }

    pub fn speed(&self) -> f64 {
        self.shared.feed.lock().speed
    }

    /// The media time being heard (from the first video frame), for
    /// `specular_anim::Player::set_clock`.
    pub fn clock(&self) -> Arc<dyn Fn(Instant) -> Option<Duration> + Send + Sync> {
        let shared = Arc::clone(&self.shared);
        Arc::new(move |now| heard_at(&shared.feed.lock(), now))
    }

    /// The samples being heard, as the device gets them (after the limiter).
    pub fn tap(&self) -> Arc<crate::live_tap::Tap> {
        Arc::clone(&self.shared.tap)
    }

    /// The media time being heard now (see [`AudioPlayer::clock`]).
    pub fn heard(&self, now: Instant) -> Option<Duration> {
        heard_at(&self.shared.feed.lock(), now)
    }

    /// The device has gone away. True once per player from [`AudioPlayer::take_lost`].
    pub fn lost(&self) -> bool {
        self.out.lost()
    }

    /// Waits until `frames` device frames are decoded ahead of the device, or the sound
    /// has ended; whether they are there. For tests that pull a [`FakeOutput`] themselves:
    /// buffers then never run short, however loaded the machine. `frames` must be under
    /// what the decoder keeps ahead (300 ms of sound at 1× and slower, more when faster).
    pub fn wait_queued(&self, frames: usize) -> bool {
        let want = frames * usize::from(self.format.channels);
        let mut f = self.shared.feed.lock();
        loop {
            if f.queue.len() >= want {
                return true;
            }
            if f.ended || f.quit {
                return false;
            }
            self.shared.wake.wait(&mut f);
        }
    }

    /// Why the sound's decoder stopped, the first time it is asked (to say so once).
    pub fn take_failed(&self) -> Option<String> {
        self.shared.feed.lock().failed.take()
    }

    /// True the first time the device is found gone (to say so once).
    pub fn take_lost(&self) -> bool {
        self.out.lost() && !self.lost_said.swap(true, Ordering::Relaxed)
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        self.shared.feed.lock().quit = true;
        self.shared.wake.notify_all();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// The loudest volume setting: twice as loud as the sound as made.
pub const MAX_VOLUME: f32 = 2.0;

/// The amplitude gain of a volume setting: its square up to 1, then linear up to 2.
pub fn gain_of(v: f32) -> f32 {
    if v <= 1.0 { v * v } else { v }
}

/// The limiter's ceiling (just under full scale).
const CEILING: f32 = 0.98;
/// How fast the limiter lets go, per frame (about 50 ms at 48 kHz).
const RELEASE: f32 = 1.0 / 2400.0;

/// Holds boosted sound under full scale: each frame whose peak would pass the ceiling is
/// turned down at once (no clipping), and the gain comes back slowly after.
pub fn limit(buf: &mut [f32], ch: usize, gain: &mut f32) {
    for frame in buf.chunks_exact_mut(ch.max(1)) {
        let peak = frame.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        *gain = (*gain + (1.0 - *gain) * RELEASE).min(1.0);
        if peak * *gain > CEILING {
            *gain = CEILING / peak;
        }
        for s in frame.iter_mut() {
            *s *= *gain;
        }
    }
}

fn reposition(f: &mut Feed, at: Duration) {
    f.generation += 1;
    if std::mem::take(&mut f.fade_in) {
        f.fade = Some(Fade {
            out: false,
            done: 0,
        });
    }
    f.target_ns = i64::try_from(at.as_nanos()).unwrap_or(i64::MAX);
    f.queue.clear();
    f.origin_ns = f.target_ns;
    f.base = 0;
    f.seg_start = 0;
    f.splice = None;
    f.stretching = false;
    f.anchor = None;
    f.ended = false;
    if let Some(c) = &mut f.chain {
        c.reset();
    }
}

fn heard_at(f: &Feed, now: Instant) -> Option<Duration> {
    if !f.running {
        return None;
    }
    let (at, media, speed) = f.anchor?;
    let scaled = |d: Duration| {
        let ns = if speed == 1.0 {
            d.as_nanos()
        } else {
            (d.as_nanos() as f64 * speed) as u128
        };
        i64::try_from(ns).unwrap_or(i64::MAX)
    };
    let t = if now >= at {
        media.saturating_add(scaled(now - at))
    } else {
        media.saturating_sub(scaled(at - now))
    };
    u64::try_from(t).ok().map(Duration::from_nanos)
}

/// Media time of input frame `pos` (counted like `base`) in the file heard.
fn media_ns(f: &Feed, pos: f64, rate: u32) -> i64 {
    let pos = pos - f.seg_start as f64;
    let ns = if pos.fract() == 0.0 {
        (pos.max(0.0) as u128 * 1_000_000_000 / u128::from(rate.max(1))) as i64
    } else {
        (pos * 1e9 / f64::from(rate.max(1))) as i64
    };
    f.origin_ns.saturating_add(ns)
}

/// The device callback: hands out queued frames (time-stretched away from 1×; silence when
/// paused or starved), runs the device-side stages and moves the clock's anchor to this
/// buffer.
fn fill_buffer(
    shared: &Shared,
    buf: &mut [f32],
    heard: Instant,
    ch: usize,
    rate: u32,
    order: DeviceOrder,
) {
    let mut guard = shared.feed.lock();
    let f = &mut *guard;
    if !f.running {
        buf.fill(0.0);
        f.anchor = None;
        shared.tap.idle();
        return;
    }
    let frames = buf.len() / ch;
    let mut done = 0;
    // The media time of the buffer's first frame and the speed it plays at.
    let mut first: Option<(f64, f64)> = None;
    let st = f.stretch.get_or_insert_with(|| Stretcher::new(ch, rate));
    while done < frames {
        if f.stretching && st.pending() > 0 {
            first.get_or_insert((st.position(), st.hop_speed()));
            done += st.take(&mut buf[done * ch..]);
            continue;
        }
        if f.speed == 1.0 {
            if f.stretching {
                // Plain playback goes on where the last segment would.
                let skip = st
                    .next
                    .saturating_sub(f.base)
                    .min((f.queue.len() / ch) as u64);
                f.queue.drain(..skip as usize * ch);
                f.base += skip;
                f.stretching = false;
            }
            let n = (frames - done).min(f.queue.len() / ch);
            if n > 0 {
                first.get_or_insert((f.base as f64, 1.0));
                for (d, s) in buf[done * ch..].iter_mut().zip(f.queue.drain(..n * ch)) {
                    *d = s;
                }
                f.base += n as u64;
                done += n;
            }
            break;
        }
        if !f.stretching {
            st.start(f.base);
            f.stretching = true;
        }
        if !st.make_hop(&f.queue, f.base, f.ended, f.speed) {
            break;
        }
        let used = st
            .keep_from()
            .saturating_sub(f.base)
            .min((f.queue.len() / ch) as u64);
        f.queue.drain(..used as usize * ch);
        f.base += used;
    }
    let Some((pos, speed)) = first else {
        buf.fill(0.0);
        f.anchor = None;
        shared.tap.idle();
        drop(guard);
        shared.wake.notify_all();
        return;
    };
    // The next file is heard from this buffer on: its clock starts at its own start.
    if let Some((at, drc)) = f.splice
        && pos >= at as f64
    {
        f.splice = None;
        f.seg_start = at;
        f.origin_ns = 0;
        f.advanced += 1;
        f.retired += 1;
        f.stream_drc = drc;
        if let Some(c) = &mut f.chain {
            c.set_stream_drc(drc);
        }
    }
    f.anchor = Some((heard, media_ns(f, pos, rate), speed));
    let out = &mut buf[..done * ch];
    let chain = f.chain.get_or_insert_with(|| {
        let mut c = Chain::new(ch, rate, order);
        c.set_stream_drc(f.stream_drc);
        c.set(&f.sound);
        c
    });
    chain.shape(out);
    let gain = if f.muted { 0.0 } else { gain_of(f.volume) };
    for d in out.iter_mut() {
        *d *= gain;
    }
    chain.place(out);
    if gain > 1.0 || f.boosted || chain.may_boost() {
        let mut lim = f.limit;
        limit(out, ch, &mut lim);
        f.limit = lim;
    } else {
        f.limit = 1.0;
    }
    if let Some(fade) = &mut f.fade
        && !fade.apply(out, ch, rate)
    {
        f.fade = None;
    }
    // The live displays read stereo: surround is folded from the device's order.
    let at = media_ns(f, pos, rate);
    if ch > 2 {
        let mut st = std::mem::take(&mut f.tap_stereo);
        st.clear();
        crate::speakers::device_to_stereo(out, ch, order, &mut st);
        shared.tap.write(&st, 2, rate, at, speed);
        f.tap_stereo = st;
    } else {
        shared.tap.write(out, ch, rate, at, speed);
    }
    buf[done * ch..].fill(0.0);
    drop(guard);
    shared.wake.notify_all();
}

/// The decoding thread's state: the file heard first in `segs`, then the next one primed.
struct Decoding {
    shared: Arc<Shared>,
    format: DeviceFormat,
    order: DeviceOrder,
    segs: VecDeque<Segment>,
    /// The segment being decoded.
    at: usize,
}

/// What the decoder makes of one step, before it goes in the queue.
struct Step {
    out: Vec<f32>,
    /// The next file starts at this frame of `out` (it is heard from there).
    splice_at: Option<usize>,
    /// Some of `out` is the next file's.
    mixed: bool,
    /// Nothing more is coming.
    end: bool,
}

impl Decoding {
    fn run(mut self) {
        let ahead_frames = AHEAD.as_millis() as usize * self.format.rate as usize / 1000;
        let ch_out = usize::from(self.format.channels);
        // Faster playback uses the queue up faster: keep as much time ahead.
        let ahead = |speed: f64| (ahead_frames as f64 * speed.max(1.0)) as usize * ch_out;
        let mut seen = 0;
        let mut scratch = Scratch::default();
        // Frames of the next file decoded during a crossfade, not yet in the queue.
        let mut carry: Vec<f32> = Vec::new();
        loop {
            let (generation, sound, req) = {
                let mut f = self.shared.feed.lock();
                loop {
                    if f.quit {
                        return;
                    }
                    // Files whose next one is heard go once they are decoded to their end
                    // (in a crossfade the next file is heard before), or at a seek.
                    while f.retired > 0
                        && self.segs.len() > 1
                        && (self.at > 0 || f.generation != seen)
                    {
                        f.retired -= 1;
                        self.segs.pop_front();
                        self.at = self.at.saturating_sub(1);
                    }
                    if let Some(req) = f.next_req.take() {
                        break (f.generation, f.sound.clone(), Some((req, f.next_piece)));
                    }
                    if f.generation != seen {
                        seen = f.generation;
                        // Seeks are in the file heard; a next one starts again from its start.
                        self.at = 0;
                        carry.clear();
                        let rate = self.format.rate;
                        self.segs[0].seek(f.target_ns, rate);
                        for s in self.segs.iter_mut().skip(1) {
                            s.seek(0, rate);
                        }
                        if self.segs[0].at_end() && self.segs.len() == 1 {
                            f.ended = true;
                        }
                    }
                    if !f.ended && f.queue.len() < ahead(f.speed) {
                        break (f.generation, f.sound.clone(), None);
                    }
                    self.shared.wake.wait(&mut f);
                }
            };
            if let Some((req, piece)) = req {
                self.take_next(req, piece, &mut carry);
                continue;
            }
            let step = self.step(&sound, &mut scratch, &mut carry);
            let mut f = self.shared.feed.lock();
            if f.generation != generation {
                continue;
            }
            // A new next file was asked for meanwhile: only the heard file's sound goes in.
            let out = match (f.next_req.is_some(), step.splice_at) {
                (false, _) => &step.out[..],
                (true, Some(k)) => &step.out[..k * ch_out],
                (true, None) if !step.mixed => &step.out[..],
                (true, None) => &[],
            };
            if let (Some(k), false) = (step.splice_at, f.next_req.is_some()) {
                let at = f.base + (f.queue.len() / ch_out) as u64 + k as u64;
                let drc = self.segs.get(self.at).is_some_and(|s| s.stream_drc);
                f.splice = Some((at, drc));
            }
            f.queue.extend(out.iter().copied());
            f.ended = step.end;
            f.has_next = self.segs.len().saturating_sub(f.retired as usize) > 1;
            f.applied = self.segs[0].applied;
            f.source = self.segs[0].heard_as();
            f.boosted = self.segs.iter().any(|s| s.gain > 1.0);
            drop(f);
            // For [`AudioPlayer::wait_queued`].
            self.shared.wake.notify_all();
        }
    }

    /// Takes a new next file (or none): opened and primed here, off the device's thread.
    fn take_next(&mut self, req: Option<Arc<Source>>, piece: bool, carry: &mut Vec<f32>) {
        // The decoder may have moved into the old next file; it comes back to the end of
        // the heard one (the queue was cut at the splice).
        self.segs.truncate(1);
        self.segs[0].continues = false;
        carry.clear();
        if self.at > 0 {
            self.at = 0;
            let s = &mut self.segs[0];
            s.cur.sample = s.track.samples.len();
            s.cur.lead = 0;
        }
        let next = req.and_then(|src| match crate::session::sound_movie(&src) {
            Ok((movie, first)) => {
                Segment::for_device(src, &movie, first, self.format, self.order).ok()
            }
            Err(_) => None,
        });
        if let Some(n) = &next {
            self.segs[0].continues = piece && continuable(&self.segs[0], n);
        }
        let mut f = self.shared.feed.lock();
        f.has_next = next.is_some();
        if let Some(n) = next {
            self.segs.push_back(n);
            // The heard file had ended: the next one goes on from there.
            f.ended = false;
        } else if self.segs[0].at_end() {
            f.ended = true;
        }
        drop(f);
        self.shared.wake.notify_all();
    }

    /// Decodes one step of the segment being decoded (mixing in the next one during a
    /// crossfade) as device-format frames with the levelling applied.
    fn step(&mut self, sound: &SoundSettings, sc: &mut Scratch, carry: &mut Vec<f32>) -> Step {
        let ch_out = usize::from(self.format.channels);
        let rate = self.format.rate;
        let centre = sound.dialogue.map_or(1.0, crate::dialogue::centre_gain);
        let fmt = self.format;
        let mut out = Vec::new();
        let made = produce(&mut self.segs[self.at], sound, centre, fmt, sc, &mut out);
        let seg = &self.segs[self.at];
        let ended = seg.at_end();
        let Some(next) = self.segs.get(self.at + 1) else {
            return Step {
                out,
                splice_at: None,
                mixed: false,
                end: ended,
            };
        };
        // Crossfade: the next file starts under the end of this one (never within an
        // album: its files are made to follow each other).
        let xf = (sound.crossfade.as_secs_f64() * f64::from(rate)) as u64;
        let same_album = seg.album.is_some() && seg.album == next.album;
        let (seg_len, seg_pos) = (seg.len, seg.pos);
        let remaining = seg_len.saturating_sub(seg_pos);
        let started = next.started;
        // Pieces of one stream are one sound: never faded into each other.
        let one_stream = seg.continues;
        if xf > 0 && !same_album && !one_stream && (started || (made > 0 && remaining < xf)) {
            let next = &mut self.segs[self.at + 1];
            if !started {
                next.seek(0, rate);
            }
            let n = out.len() / ch_out;
            // The fade starts `xf` frames before the end, part way into this step.
            let from = if started {
                0
            } else {
                ((seg_len.saturating_sub(xf)).saturating_sub(seg_pos - n as u64) as usize).min(n)
            };
            let need = (n - from) * ch_out;
            while carry.len() < need && !next.at_end() {
                produce(next, sound, centre, fmt, sc, carry);
            }
            for i in from..n {
                // Equal power: the old file's share falls as the new one's rises.
                let left = (seg_len.saturating_sub(seg_pos - (n - i) as u64)) as f64 / xf as f64;
                let th = (1.0 - left.clamp(0.0, 1.0)) * std::f64::consts::FRAC_PI_2;
                let (a, b) = (th.cos() as f32, th.sin() as f32);
                for c in 0..ch_out {
                    let k = i * ch_out + c;
                    let j = (i - from) * ch_out + c;
                    out[k] = out[k] * a + carry.get(j).copied().unwrap_or(0.0) * b;
                }
            }
            carry.drain(..need.min(carry.len()));
            if ended {
                // The rest of what was decoded of the next file follows as it is.
                self.at += 1;
                out.append(carry);
            }
            return Step {
                out,
                splice_at: (!started).then_some(from),
                mixed: true,
                end: false,
            };
        }
        if !ended {
            return Step {
                out,
                splice_at: None,
                mixed: false,
                end: false,
            };
        }
        // Gapless: the next file's first frames follow this one's last.
        self.at += 1;
        let k = out.len() / ch_out;
        if one_stream {
            // The next piece of one stream: the decoder (and the resampler) go on into it
            // with their state, from its first coded sample.
            let (a, b) = self.segs.make_contiguous().split_at_mut(self.at);
            b[0].go_on_from(&mut a[self.at - 1]);
        } else {
            self.segs[self.at].seek(0, rate);
        }
        let next = &mut self.segs[self.at];
        let mut first = Vec::new();
        while first.is_empty() && !next.at_end() {
            produce(next, sound, centre, fmt, sc, &mut first);
        }
        out.append(&mut first);
        Step {
            out,
            splice_at: Some(k),
            mixed: true,
            end: false,
        }
    }
}

/// Whether `next`'s sound is coded as `heard`'s, so one decoder can go on from one into the
/// other: the same codec and configuration, channels and rate, for codecs whose decoders keep
/// no per-file tables (AAC, MPEG audio, AC-3, E-AC-3, Opus).
fn continuable(heard: &Segment, next: &Segment) -> bool {
    let (a, b) = (&heard.track, &next.track);
    matches!(
        a.codec,
        AudioCodec::Aac(_)
            | AudioCodec::Mpa(_)
            | AudioCodec::Ac3
            | AudioCodec::Eac3
            | AudioCodec::Opus(_)
    ) && a.codec == b.codec
        && a.channels == b.channels
        && a.sample_rate == b.sample_rate
        && (heard.ch, heard.rate, heard.surround) == (next.ch, next.rate, next.surround)
        && a.extra.companions.is_empty()
        && b.extra.companions.is_empty()
}

/// Buffers reused between steps.
#[derive(Default)]
struct Scratch {
    pcm: Vec<f32>,
    mapped: Vec<f32>,
}

/// Decodes the next coded sample of `seg` into `out` (appended, device format), levelled;
/// how many frames it added.
fn produce(
    seg: &mut Segment,
    sound: &SoundSettings,
    centre: f32,
    format: DeviceFormat,
    sc: &mut Scratch,
    out: &mut Vec<f32>,
) -> usize {
    let ch_out = usize::from(format.channels);
    if !seg.decode_next(&mut sc.pcm) {
        return 0;
    }
    sc.mapped.clear();
    seg.map.run(&sc.pcm, centre, &mut sc.mapped);
    let from = out.len();
    seg.resampler.run(&sc.mapped, out);
    let made = (out.len() - from) / ch_out;
    seg.pos += made as u64;
    seg.started = true;
    let lev = &sound.levelling;
    if lev.measure && lev.mode != levelling::Mode::Off && seg.tags.is_empty() {
        // Measured on the sound's own channels (BS.1770: by speaker, no LFE), not on the
        // device's, which may be folded or in another order.
        seg.meter
            .get_or_insert_with(|| Meter::new(seg.ch, seg.rate))
            .feed(&sc.pcm);
    }
    let (g, _) = seg.level(lev, made as f64 / f64::from(format.rate.max(1)));
    // The first gain applies at once; later ones ramp.
    let had = if seg.gain_set { seg.gain } else { g };
    seg.gain = g;
    seg.gain_set = true;
    if g != 1.0 || had != 1.0 {
        // A ramp from the last gain over the step: no click when it changes.
        let n = made.max(1) as f32;
        for (i, f) in out[from..].chunks_exact_mut(ch_out).enumerate() {
            let k = had + (g - had) * ((i + 1) as f32 / n);
            for s in f {
                *s *= k;
            }
        }
    }
    made
}

impl Cursor {
    /// Starts decoding at media time `start` (the edit's): pre-roll and the part of the
    /// first sample before it.
    fn with_start(mut self, d: &Segment, start: i128) -> Cursor {
        let t = &d.track;
        let k = self.sample.min(t.samples.len().saturating_sub(1));
        let k = if t.samples.get(k).is_some_and(|s| s.dts as i128 > start) {
            k.saturating_sub(1)
        } else {
            k
        };
        let from = k.saturating_sub(d.dec.preroll());
        let into = (start - t.samples.get(k).map_or(0, |s| s.dts as i128)).max(0);
        self.sample = from;
        self.drop = k - from;
        self.skip = (into * i128::from(d.rate) / i128::from(t.timescale.max(1))) as u64;
        self
    }
}

/// A sound device for tests: nothing plays; the test pulls buffers with
/// [`FakeOutput::pull`], saying when each is heard, and reads what was played.
#[derive(Clone)]
pub struct FakeOutput {
    format: DeviceFormat,
    order: DeviceOrder,
    /// The channel count each start was asked for.
    pub asked: Arc<Mutex<Vec<u16>>>,
    render: Arc<Mutex<Option<Render>>>,
    /// Everything played, interleaved.
    pub played: Arc<Mutex<Vec<f32>>>,
    lost: Arc<AtomicBool>,
    fail: Option<String>,
}

impl FakeOutput {
    pub fn new(rate: u32, channels: u16) -> FakeOutput {
        FakeOutput {
            format: DeviceFormat { rate, channels },
            order: DeviceOrder::Wave,
            asked: Arc::new(Mutex::new(Vec::new())),
            render: Arc::new(Mutex::new(None)),
            played: Arc::new(Mutex::new(Vec::new())),
            lost: Arc::new(AtomicBool::new(false)),
            fail: None,
        }
    }

    /// A device that will not start.
    pub fn failing(why: &str) -> FakeOutput {
        FakeOutput {
            fail: Some(why.into()),
            ..FakeOutput::new(48_000, 2)
        }
    }

    /// Plays one buffer of `frames`, heard at `heard`. False when the device is not started.
    pub fn pull(&self, frames: usize, heard: Instant) -> bool {
        let mut buf = vec![0.0; frames * usize::from(self.format.channels)];
        let mut r = self.render.lock();
        let Some(render) = r.as_mut() else {
            return false;
        };
        render(&mut buf, heard);
        drop(r);
        self.played.lock().extend_from_slice(&buf);
        true
    }

    /// A device taking more than two channels in `order`.
    pub fn with_order(mut self, order: DeviceOrder) -> FakeOutput {
        self.order = order;
        self
    }

    /// Simulates the device going away.
    pub fn unplug(&self) {
        self.lost.store(true, Ordering::Relaxed);
        *self.render.lock() = None;
    }
}

impl AudioOutput for FakeOutput {
    fn start(&mut self, render: Render, source_channels: u16) -> Result<DeviceFormat, String> {
        self.asked.lock().push(source_channels);
        if let Some(why) = &self.fail {
            return Err(why.clone());
        }
        *self.render.lock() = Some(render);
        Ok(self.format)
    }
    fn lost(&self) -> bool {
        self.lost.load(Ordering::Relaxed)
    }
    fn order(&self) -> DeviceOrder {
        self.order
    }
    fn device(&self) -> OutputDevice {
        OutputDevice {
            id: "test".into(),
            name: "Test device".into(),
            default: true,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn boost_is_limited_not_clipped() {
        assert_eq!(super::gain_of(0.5), 0.25);
        assert_eq!(super::gain_of(2.0), 2.0);
        // A 0.8 sine boosted to 1.6: never past the ceiling, and quiet parts keep the boost.
        let mut buf: Vec<f32> = (0..9600)
            .flat_map(|i| {
                let v = 0.8 * (i as f32 * 0.05).sin() * super::gain_of(2.0);
                [v, v]
            })
            .collect();
        let mut g = 1.0;
        super::limit(&mut buf, 2, &mut g);
        assert!(buf.iter().all(|s| s.abs() <= 0.98 + 1e-6));
        let mut quiet = vec![0.2f32; 2 * 48_000];
        super::limit(&mut quiet, 2, &mut g);
        assert!(
            (quiet[quiet.len() - 1] - 0.2).abs() < 0.01,
            "the gain comes back"
        );
    }
    use super::*;

    #[test]
    fn pcm_formats_decode() {
        let mut out = Vec::new();
        let mut d = Pcm {
            bits: 16,
            float: false,
            little_endian: true,
        };
        d.decode(&[0x00, 0x40, 0x00, 0xC0], &mut out).unwrap();
        assert_eq!(out, [0.5, -0.5]);
        out.clear();
        let mut d = Pcm {
            bits: 24,
            float: false,
            little_endian: false,
        };
        d.decode(&[0x40, 0, 0, 0xC0, 0, 0], &mut out).unwrap();
        assert_eq!(out, [0.5, -0.5]);
        out.clear();
        let mut d = Pcm {
            bits: 8,
            float: false,
            little_endian: true,
        };
        d.decode(&[128, 192, 64], &mut out).unwrap();
        assert_eq!(out, [0.0, 0.5, -0.5]);
        out.clear();
        let mut d = Pcm {
            bits: 32,
            float: true,
            little_endian: false,
        };
        d.decode(&0.25f32.to_be_bytes(), &mut out).unwrap();
        assert_eq!(out, [0.25]);
    }

    /// PCM declared at 1 Hz (48 000 output samples per input one) or with 65 535 channels
    /// (a gigabyte of silence a tenth of a second): refused, not resampled.
    #[test]
    fn implausible_rate_or_channels_refused() {
        let track = |channels, sample_rate| AudioTrack {
            codec: AudioCodec::Pcm {
                bits: 16,
                float: false,
                little_endian: true,
            },
            channels,
            sample_rate,
            timescale: sample_rate.max(1),
            duration: 1,
            sample_count: 1,
            samples: vec![crate::mp4::Sample {
                offset: 0,
                size: 2,
                dts: 0,
                cts: 0,
                duration: 1,
                sync: true,
            }],
            edit: Default::default(),
            layout: Default::default(),
            extra: Default::default(),
        };
        assert!(playable(&track(2, 48_000)).is_ok());
        assert!(matches!(
            playable(&track(2, 1)),
            Err(Silence::Unsupported(_))
        ));
        assert!(matches!(
            playable(&track(65_535, 48_000)),
            Err(Silence::Unsupported(_))
        ));
    }

    /// A 1 kHz sine resampled 44.1 → 48 kHz in uneven chunks stays a clean 1 kHz sine of the
    /// right length.
    #[test]
    fn resampling_keeps_a_sine_clean() {
        let sine = |rate: f64, n: usize| -> Vec<f32> {
            (0..n)
                .map(|i| (2.0 * std::f64::consts::PI * 1000.0 * i as f64 / rate).sin() as f32)
                .collect()
        };
        let input = sine(44_100.0, 44_100);
        let mut r = Resampler::new(1, 44_100, 48_000);
        let mut out = Vec::new();
        let mut at = 0;
        for (k, n) in [1usize, 7, 1000, 333, 4410].iter().cycle().enumerate() {
            if at >= input.len() {
                break;
            }
            let end = (at + n + k % 3).min(input.len());
            r.run(&input[at..end], &mut out);
            at = end;
        }
        assert!(out.len().abs_diff(48_000) <= 3, "{} frames", out.len());
        let want = sine(48_000.0, out.len());
        let err = out
            .iter()
            .zip(&want)
            .skip(4)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(err < 2e-3, "worst error {err}");
    }
}
