//! A sound file (or a movie with sound and no picture) as an image that plays: its cover
//! above its waveform and spectrogram, timed as frames the animation player steps through
//! as the sound plays (the sound is the clock, as for movies). The app's sound page draws
//! the playhead on the GPU; the viewer's player takes frames without pictures
//! ([`SoundSession::clock_frames`]).
//!
//! - The waveform is each column's peak and RMS level of every decoded sample, per channel
//!   (more than two folded to stereo as playback does); the spectrogram is a Hann-windowed
//!   FFT at each column's centre on a logarithmic frequency axis, −100 to 0 dBFS.
//! - Files longer than [`EXACT_MAX_US`] are sampled: one short window a column (peaks then
//!   come from the windows only). Gallery thumbnails are the cover, else a sampled waveform.
//! - A track whose sound can't be decoded shows its cover and empty lanes; why is said by
//!   the application, as for a movie's.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, RasterPlane, Source};
use specular_decode::{
    AnimationInfo, DecodeError, DecodeRequest, Format, Frame, Frames, ImageInfo, Result, Session,
    SoundFacts, VideoFacts,
};

use crate::audio::TrackDecoder;
use crate::loudness;
use crate::lyrics::{self, Lyrics};
use crate::mp4::{AudioCodec, AudioTrack, Movie};
use crate::sound_tags::{self, Tags};
use crate::speakers::fold_stereo;

/// The silent reason of DRM-protected sound (FairPlay M4P): never decrypted.
pub const DRM: &str = "DRM-protected";

/// The picture's size.
pub const WIDTH: u32 = 1600;
pub const HEIGHT: u32 = 1000;
/// Sound longer than this is sampled for the picture (one window a column).
pub const EXACT_MAX_US: u64 = 30 * 60 * 1_000_000;
/// The playhead moves ten times a second.
const FRAME: Duration = Duration::from_millis(100);
/// The most frames a timeline has (longer sound moves the playhead less often).
const MAX_FRAMES: u64 = 200_000;
/// Frequency bins of the spectrogram, logarithmically spaced.
pub const BINS: usize = 256;
/// Third-octave bands of the spectrum analyser (20 Hz to 20 kHz).
pub const BANDS: usize = 31;
/// Channels metered at most.
pub const MAX_CH: usize = 8;
/// The sound page's time step (one column), and its most columns.
pub const COL_US: u64 = 10_000;
pub const MAX_COLS: usize = 32_768;
/// Windows read for the sound page of sound longer than [`EXACT_MAX_US`].
const SAMPLED_COLS: usize = 4096;
/// The lowest frequency shown.
const F_MIN: f64 = 20.0;
/// Windows decoded for a sampled thumbnail.
const THUMB_WINDOWS: usize = 160;

/// Colours (RGB).
const BACKGROUND: [u8; 3] = [20, 22, 26];
const LANE: [u8; 3] = [28, 31, 36];
const AXIS: [u8; 3] = [58, 62, 70];
const WAVE: [u8; 3] = [86, 128, 178];
const WAVE_RMS: [u8; 3] = [140, 182, 230];
const PLAYED: [u8; 3] = [214, 150, 52];
const PLAYED_RMS: [u8; 3] = [250, 206, 120];
/// The spectrogram's colour ramp from silence to full scale: black, violet, crimson,
/// orange, pale yellow.
const RAMP: [[f32; 3]; 5] = [
    [2.0, 2.0, 8.0],
    [70.0, 20.0, 110.0],
    [185.0, 45.0, 85.0],
    [245.0, 135.0, 40.0],
    [252.0, 248.0, 185.0],
];

/// Levels, spectra and loudness by column: the picture's and the sound page's data. Column
/// `c` is the sound from `c / cols` to `(c + 1) / cols` of its length.
#[derive(Debug)]
pub struct Analysis {
    pub cols: usize,
    /// Waveform lanes: 1 (mono) or 2 (more channels folded to stereo, as playback does).
    pub lanes: usize,
    /// Meter channels: the sound's own, up to [`MAX_CH`].
    pub channels: usize,
    /// The length (seconds).
    pub duration_s: f64,
    /// Per column, per lane: lowest, highest and RMS level (−1…1; RMS ≥ 0).
    pub wave: Vec<[f32; 3]>,
    /// Per column, `BINS` levels 0…255 (−100…0 dBFS), lowest frequency first.
    pub spec: Vec<u8>,
    /// Per column, [`BANDS`] third-octave levels 0…255 (−100…0 dBFS).
    pub bands: Vec<u8>,
    /// Per column, per channel: sample peak, mean square and true peak (linear).
    pub meters: Vec<[f32; 3]>,
    /// Per column of a stereo sound (the waveform's two lanes): the means of L·R, L² and
    /// R², for the phase correlation; empty for mono.
    pub stereo: Vec<[f32; 3]>,
    /// The spectrogram's frequency axis: its lowest and highest frequency (Hz).
    pub f_lo: f32,
    pub f_hi: f32,
    /// Every sample was read (else one short window a column).
    pub exact: bool,
    /// The loudness of the whole sound, when every sample was read.
    pub loudness: Option<Arc<loudness::Report>>,
}

impl Analysis {
    /// Bytes held.
    pub fn bytes(&self) -> usize {
        self.wave.len() * 12
            + self.spec.len()
            + self.bands.len()
            + self.meters.len() * 12
            + self.stereo.len() * 12
            + self.loudness.as_ref().map_or(0, |l| l.momentary.len() * 8)
    }

    /// Column `c`'s spectrogram level of bin `b` (0…1).
    pub fn level(&self, c: usize, b: usize) -> f32 {
        f32::from(self.spec[c * BINS + b]) / 255.0
    }
}

/// A decoded cover picture.
struct Picture {
    w: u32,
    h: u32,
    rgba: Vec<u8>,
}

/// A sound file as an image.
pub struct SoundSession {
    source: Arc<Source>,
    movie: Movie,
    info: ImageInfo,
    anim: Option<AnimationInfo>,
    facts: VideoFacts,
    tags: Tags,
    cover: Option<Arc<Picture>>,
    duration_us: u64,
    frames: u32,
    analysis: Mutex<Option<Arc<Analysis>>>,
    loudness: Mutex<Option<Arc<loudness::Report>>>,
    /// Held while the analysis or the loudness is worked out (one at a time).
    working: Mutex<()>,
    lyrics: std::sync::OnceLock<Option<Arc<Lyrics>>>,
}

/// Why a track's sound can't be heard, said for DRM in words of its own.
fn drm(t: &AudioTrack) -> bool {
    matches!(t.codec, AudioCodec::Other(c, _) if matches!(&c, b"drms" | b"drmi" | b"enca" | b"encr"))
}

impl SoundSession {
    /// The session of `movie` (parsed from `source`, with samples), which has a sound track
    /// and no picture.
    pub fn new(source: Arc<Source>, format: Format, movie: Movie) -> Result<SoundSession> {
        let track = movie
            .audio
            .as_ref()
            .ok_or_else(|| DecodeError::Malformed("no sound track".into()))?;
        let tags = sound_tags::read(source.bytes());
        let cover = tags.cover.as_ref().and_then(|c| {
            specular_decode::decode_rgba8(&c.data)
                .map(|(w, h, rgba)| Arc::new(Picture { w, h, rgba }))
        });
        let duration_us = movie
            .duration_us
            .filter(|&d| d > 0)
            .or_else(|| {
                (track.timescale > 0)
                    .then(|| track.us(track.duration as i64).max(0) as u64)
                    .filter(|&d| d > 0)
            })
            .unwrap_or(0);
        let frame_us = (FRAME.as_micros() as u64).max(duration_us.div_ceil(MAX_FRAMES));
        let frames = duration_us.div_ceil(frame_us).clamp(1, MAX_FRAMES) as u32;
        let mut facts = crate::session::facts(&movie, frames, None, false);
        facts.duration_us = (duration_us > 0).then_some(duration_us);
        facts.codec = String::new();
        facts.frame_rate = None;
        if drm(track) {
            facts.silent_reason = Some(DRM.into());
        }
        let bytes: u64 = track.samples.iter().map(|s| u64::from(s.size)).sum();
        let kbps = (duration_us > 0 && bytes > 0)
            .then(|| (bytes as u128 * 8_000 / u128::from(duration_us)) as u32);
        let bits = match &track.codec {
            AudioCodec::Pcm { bits, .. } => Some(*bits),
            AudioCodec::Flac(c) => specular_flac::StreamInfo::from_config(c)
                .ok()
                .map(|i| u16::from(i.bits)),
            AudioCodec::Alac(c) if c.len() > 5 => Some(u16::from(c[5])),
            AudioCodec::WavPack { bits, .. } | AudioCodec::Ape { bits, .. } => Some(*bits),
            _ => None,
        };
        let cover_text = tags.cover.as_ref().map(|c| {
            let kind = c
                .mime
                .strip_prefix("image/")
                .unwrap_or(&c.mime)
                .to_ascii_uppercase();
            match &cover {
                Some(p) => format!("{kind} {} × {}", p.w, p.h),
                None => kind,
            }
        });
        facts.sound = Some(SoundFacts {
            fields: tags.fields.clone(),
            tag_kinds: tags.kinds.clone(),
            cover: cover_text,
            cover_shown: cover.is_some(),
            kbps,
            bits,
        });
        let coding = facts.audio.clone().unwrap_or_default();
        let info = ImageInfo {
            format,
            width: WIDTH,
            height: HEIGHT,
            orientation: Orientation(1),
            icc_profile: None,
            has_alpha: false,
            bit_depth: 8,
            progressive: false,
            coding,
            embedded_preview: None,
            fast_regions: false,
            scales: &[1],
        };
        let anim = (duration_us > 0 && !track.samples.is_empty()).then_some(AnimationInfo {
            frame_count: Some(frames),
            loop_count: 1,
        });
        Ok(SoundSession {
            source,
            movie,
            info,
            anim,
            facts,
            tags,
            cover,
            duration_us,
            frames,
            analysis: Mutex::new(None),
            loudness: Mutex::new(None),
            working: Mutex::new(()),
            lyrics: std::sync::OnceLock::new(),
        })
    }

    pub fn movie(&self) -> &Movie {
        &self.movie
    }

    /// The tags read from the file.
    pub fn tags(&self) -> &Tags {
        &self.tags
    }

    /// The analysis of the whole sound for the picture and the sound page (cached; one
    /// thread works it out, others wait for it); `None` when its sound can't be decoded.
    pub fn analysis(&self, cancel: &CancelToken) -> Result<Option<Arc<Analysis>>> {
        if let Some(a) = self.analysis.lock().as_ref() {
            return Ok(Some(Arc::clone(a)));
        }
        let Some(t) = self.movie.audio.as_ref() else {
            return Ok(None);
        };
        let _working = self.working.lock();
        if let Some(a) = self.analysis.lock().as_ref() {
            return Ok(Some(Arc::clone(a)));
        }
        let plot = Layout::new(WIDTH, HEIGHT, self.cover.is_some()).plot_w as usize;
        let (cols, mode) = if self.duration_us > EXACT_MAX_US {
            (SAMPLED_COLS, Mode::Sampled(SAMPLED_COLS, 0))
        } else {
            (page_cols(self.duration_us, plot), Mode::Exact)
        };
        let Some(a) = analyse(t, &self.source, self.duration_us, cols, mode, cancel)? else {
            return Ok(None);
        };
        if let Some(l) = &a.loudness {
            *self.loudness.lock() = Some(Arc::clone(l));
        }
        let a = Arc::new(a);
        *self.analysis.lock() = Some(Arc::clone(&a));
        Ok(Some(a))
    }

    /// The analysis of `span_us` of the sound from `from_us`, `cols` columns of one short
    /// window each (not cached; for the sound page zoomed into a sampled sound). Runs beside
    /// a whole-file loudness measure. `None` when its sound can't be decoded.
    pub fn analysis_of(
        &self,
        from_us: u64,
        span_us: u64,
        cols: usize,
        cancel: &CancelToken,
    ) -> Result<Option<Analysis>> {
        let Some(t) = self.movie.audio.as_ref() else {
            return Ok(None);
        };
        let from_us = from_us.min(self.duration_us);
        let span_us = span_us.clamp(1, (self.duration_us - from_us).max(1));
        let cols = cols.clamp(1, MAX_COLS);
        analyse(
            t,
            &self.source,
            span_us,
            cols,
            Mode::Sampled(cols, from_us),
            cancel,
        )
    }

    /// The loudness of the whole sound (cached): from the analysis when it read every
    /// sample, else measured on its own. `None` when its sound can't be decoded.
    pub fn loudness(&self, cancel: &CancelToken) -> Result<Option<Arc<loudness::Report>>> {
        if let Some(l) = self.loudness.lock().as_ref() {
            return Ok(Some(Arc::clone(l)));
        }
        if self.duration_us <= EXACT_MAX_US {
            return Ok(self.analysis(cancel)?.and_then(|a| a.loudness.clone()));
        }
        let Some(t) = self.movie.audio.as_ref() else {
            return Ok(None);
        };
        let _working = self.working.lock();
        if let Some(l) = self.loudness.lock().as_ref() {
            return Ok(Some(Arc::clone(l)));
        }
        let Some(r) = measure_loudness(t, &self.source, cancel)? else {
            return Ok(None);
        };
        let r = Arc::new(r);
        *self.loudness.lock() = Some(Arc::clone(&r));
        Ok(Some(r))
    }

    /// The lyrics: a sidecar `.lrc` beside the file, else the tags' (read once; the sidecar
    /// is a file read, so off the UI thread).
    pub fn lyrics(&self) -> Option<Arc<Lyrics>> {
        self.lyrics
            .get_or_init(|| {
                let path = self.source.path();
                let side = if path.is_absolute() {
                    lyrics::read_sidecar(path)
                } else {
                    None
                };
                side.or_else(|| self.tags.lyrics()).map(Arc::new)
            })
            .clone()
    }

    /// The cover picture (RGBA8, its width and height), when it decoded.
    pub fn cover(&self) -> Option<(u32, u32, &[u8])> {
        self.cover.as_ref().map(|c| (c.w, c.h, c.rgba.as_slice()))
    }

    /// The length (microseconds).
    pub fn duration_us(&self) -> u64 {
        self.duration_us
    }

    /// The timeline's frames without pictures (each a transparent pixel): for the viewer's
    /// player, whose sound page draws the page and its playhead on the GPU. Nothing is
    /// drawn on the CPU per frame.
    pub fn clock_frames(&self) -> Box<dyn Frames> {
        self.page_frames(None)
    }

    fn page_frames(&self, picture: Option<Arc<Vec<u8>>>) -> Box<dyn Frames> {
        let delay = Duration::from_micros(
            (FRAME.as_micros() as u64).max(self.duration_us.div_ceil(MAX_FRAMES)),
        );
        Box::new(PageFrames {
            picture,
            count: self.frames,
            delay,
            duration_us: self.duration_us,
            next: 0,
        })
    }

    fn picture(&self, cancel: &CancelToken, played_to: Option<f64>) -> Result<Raster> {
        let a = self.analysis(cancel)?;
        let px = render(
            WIDTH,
            HEIGHT,
            a.as_deref(),
            self.cover.as_deref(),
            played_to,
        );
        Ok(rgba_raster(WIDTH, HEIGHT, px))
    }
}

/// How the sound is read for the picture.
#[derive(Clone, Copy)]
enum Mode {
    /// Every sample.
    Exact,
    /// This many short windows spread over the sound, or over `duration_us` of it from
    /// this time (µs).
    Sampled(usize, u64),
}

/// The FFT length for a sampling rate: about 43 ms.
fn fft_len(rate: u32) -> usize {
    match rate {
        0..=24_000 => 1024,
        24_001..=48_000 => 2048,
        _ => 4096,
    }
}

/// The centre of third-octave band `k` (ISO 266 base-2 series, 1 kHz at band 17; nominal
/// 20 Hz to 20 kHz).
pub fn band_centre(k: usize) -> f64 {
    1000.0 * 2f64.powf((k as f64 - 17.0) / 3.0)
}

/// The sound page's columns for sound `duration_us` long: one per [`COL_US`], at least
/// `min`, at most [`MAX_COLS`].
pub fn page_cols(duration_us: u64, min: usize) -> usize {
    ((duration_us / COL_US) as usize).clamp(min.max(1), MAX_COLS.max(min))
}

/// A level 0…255 (−100…0 dBFS) of a power (1 = a full-scale sine).
fn level_u8(power: f64) -> u8 {
    let db = 10.0 * (power + 1e-12).log10();
    (((db + 100.0) / 100.0).clamp(0.0, 1.0) * 255.0).round() as u8
}

/// One window's spectrum: `BINS` log-spaced levels and [`BANDS`] third-octave levels from
/// `win` (mono, `n` samples; missing ones silent). Also the live displays' (the live tap's
/// sound, `crate::sound_feed::LiveFeed`).
pub(crate) struct Spectrum {
    fft: specular_fft::Fft,
    pub(crate) n: usize,
    rate: u32,
    hann: Vec<f32>,
    data: Vec<specular_fft::Complex>,
    scratch: Vec<specular_fft::Complex>,
    power: Vec<f64>,
    /// The last window's powers (1 = a full-scale sine).
    bin_p: [f64; BINS],
    band_p: [f64; BANDS],
}

impl Spectrum {
    pub(crate) fn new(rate: u32) -> Spectrum {
        let n = fft_len(rate);
        let fft = specular_fft::Fft::new(n);
        let scratch = vec![specular_fft::Complex::ZERO; fft.scratch_len()];
        let hann = (0..n)
            .map(|i| {
                let x = std::f64::consts::PI * 2.0 * i as f64 / n as f64;
                (0.5 - 0.5 * x.cos()) as f32
            })
            .collect();
        Spectrum {
            fft,
            n,
            rate,
            hann,
            data: vec![specular_fft::Complex::ZERO; n],
            scratch,
            power: vec![0.0; n / 2 + 1],
            bin_p: [0.0; BINS],
            band_p: [0.0; BANDS],
        }
    }

    pub(crate) fn rate(&self) -> u32 {
        self.rate
    }

    /// The spectrogram's lowest and highest frequency.
    fn range(&self) -> (f64, f64) {
        let nyquist = f64::from(self.rate) / 2.0;
        let f_min = F_MIN
            .max(f64::from(self.rate) / self.n as f64)
            .min(nyquist / 2.0);
        (f_min, nyquist)
    }

    fn levels(&mut self, win: &[f32], bins: &mut Vec<u8>, bands: &mut Vec<u8>) {
        self.powers(win);
        bins.extend(self.bin_p.iter().map(|&p| level_u8(p)));
        bands.extend(self.band_p.iter().map(|&p| level_u8(p)));
    }

    /// The levels of `win` in dBFS, no lower than `floor`.
    pub(crate) fn levels_db(
        &mut self,
        win: &[f32],
        floor: f32,
        bins: &mut [f32; BINS],
        bands: &mut [f32; BANDS],
    ) {
        self.powers(win);
        let db = |p: f64| ((10.0 * (p + 1e-12).log10()) as f32).clamp(floor, 0.0);
        for (o, &p) in bins.iter_mut().zip(&self.bin_p) {
            *o = db(p);
        }
        for (o, &p) in bands.iter_mut().zip(&self.band_p) {
            *o = db(p);
        }
    }

    fn powers(&mut self, win: &[f32]) {
        let n = self.n;
        for (i, d) in self.data.iter_mut().enumerate() {
            *d = specular_fft::Complex::new(win.get(i).copied().unwrap_or(0.0) * self.hann[i], 0.0);
        }
        self.fft.forward(&mut self.data, &mut self.scratch);
        // A full-scale sine's peak through a Hann window: n/4.
        let norm = 1.0 / (n as f64 / 4.0).powi(2);
        let half = n / 2;
        for (k, p) in self.power.iter_mut().enumerate() {
            *p = f64::from(self.data[k].norm_sqr()) * norm;
        }
        let power = |k: usize| self.power[k.min(half)];
        let (f_min, nyquist) = self.range();
        let ratio = nyquist / f_min;
        let hz_per_bin = f64::from(self.rate) / n as f64;
        // Interpolated at a fractional bin.
        let at = |c: f64| {
            let k = c.floor().max(0.0) as usize;
            let t = c - k as f64;
            power(k) * (1.0 - t) + power(k + 1) * t
        };
        for b in 0..BINS {
            let lo = f_min * ratio.powf(b as f64 / BINS as f64) / hz_per_bin;
            let hi = f_min * ratio.powf((b + 1) as f64 / BINS as f64) / hz_per_bin;
            self.bin_p[b] = if hi - lo < 1.0 {
                // Narrower than an FFT bin: interpolated at its centre.
                at((lo + hi) / 2.0)
            } else {
                (lo.floor() as usize..hi.ceil() as usize)
                    .map(power)
                    .fold(0.0, f64::max)
            };
        }
        // Third-octave bands: the power of the FFT bins inside, over the Hann window's
        // noise bandwidth (1.5 bins), so a full-scale sine reads 0 dBFS.
        for k in 0..BANDS {
            let c = band_centre(k);
            let (lo, hi) = (c * 2f64.powf(-1.0 / 6.0), c * 2f64.powf(1.0 / 6.0));
            if lo >= nyquist {
                self.band_p[k] = 0.0;
                continue;
            }
            let (kl, kh) = (
                (lo / hz_per_bin).ceil() as usize,
                (hi.min(nyquist) / hz_per_bin).floor() as usize,
            );
            self.band_p[k] = if kh < kl + 1 {
                // Narrower than the bins: the level at its centre.
                at(c / hz_per_bin)
            } else {
                (kl..=kh).map(power).sum::<f64>() / 1.5
            };
        }
    }
}

/// Per-column accumulation of levels: the waveform's lanes and the meters' channels.
struct Levels {
    cols: usize,
    lanes: usize,
    /// min, max, sum of squares, count.
    acc: Vec<(f32, f32, f64, u64)>,
    channels: usize,
    /// Per column and channel: peak, sum of squares, true peak, count.
    meter: Vec<(f32, f64, f32, u64)>,
    /// Per column of a stereo sound: sums of L·R, L² and R², count.
    pairs: Vec<(f64, f64, f64, u64)>,
}

impl Levels {
    fn new(cols: usize, lanes: usize, channels: usize) -> Levels {
        Levels {
            cols,
            lanes,
            acc: vec![(0.0, 0.0, 0.0, 0); cols * lanes],
            channels,
            meter: vec![(0.0, 0.0, 0.0, 0); cols * channels],
            pairs: if lanes == 2 {
                vec![(0.0, 0.0, 0.0, 0); cols]
            } else {
                Vec::new()
            },
        }
    }

    /// A stereo frame (the lanes), for the phase correlation.
    #[inline]
    fn pair(&mut self, col: usize, f: &[f32]) {
        if let (Some(p), [l, r]) = (self.pairs.get_mut(col.min(self.cols - 1)), f) {
            let (l, r) = (f64::from(*l), f64::from(*r));
            p.0 += l * r;
            p.1 += l * l;
            p.2 += r * r;
            p.3 += 1;
        }
    }

    #[inline]
    fn add(&mut self, col: usize, lane: usize, v: f32) {
        let a = &mut self.acc[col.min(self.cols - 1) * self.lanes + lane];
        if a.3 == 0 {
            a.0 = v;
            a.1 = v;
        } else {
            a.0 = a.0.min(v);
            a.1 = a.1.max(v);
        }
        a.2 += f64::from(v) * f64::from(v);
        a.3 += 1;
    }

    /// A frame of the sound's own channels, with their true peaks.
    #[inline]
    fn meter(&mut self, col: usize, frame: &[f32], tp: &[f32]) {
        let at = col.min(self.cols - 1) * self.channels;
        for (i, m) in self.meter[at..at + self.channels].iter_mut().enumerate() {
            let v = frame.get(i).copied().unwrap_or(0.0);
            m.0 = m.0.max(v.abs());
            m.1 += f64::from(v) * f64::from(v);
            m.2 = m.2.max(tp.get(i).copied().unwrap_or(0.0));
            m.3 += 1;
        }
    }

    /// The waveform's, the meters' and the stereo columns' values.
    fn finish(self) -> [Vec<[f32; 3]>; 3] {
        let wave = self
            .acc
            .into_iter()
            .map(|(lo, hi, sq, n)| {
                if n == 0 {
                    [0.0; 3]
                } else {
                    [lo, hi, (sq / n as f64).sqrt() as f32]
                }
            })
            .collect();
        let meters = self
            .meter
            .into_iter()
            .map(|(p, sq, tp, n)| [p, if n == 0 { 0.0 } else { (sq / n as f64) as f32 }, tp])
            .collect();
        let pairs = self
            .pairs
            .into_iter()
            .map(|(lr, ll, rr, n)| {
                let n = n.max(1) as f64;
                [(lr / n) as f32, (ll / n) as f32, (rr / n) as f32]
            })
            .collect();
        [wave, meters, pairs]
    }
}

/// Decodes `t` (all of it, or windows) into levels, spectra and (reading all of it)
/// loudness for `cols` columns. `None` when the sound can't be decoded.
fn analyse(
    t: &AudioTrack,
    source: &Arc<Source>,
    duration_us: u64,
    cols: usize,
    mode: Mode,
    cancel: &CancelToken,
) -> Result<Option<Analysis>> {
    let Ok(mut td) = TrackDecoder::new(t, source) else {
        return Ok(None);
    };
    let cols = cols.max(1);
    let ch = usize::from(td.ch.max(1));
    let lanes = if ch == 1 { 1 } else { 2 };
    let channels = ch.min(MAX_CH);
    let rate = td.rate;
    let total = (u128::from(duration_us) * u128::from(rate) / 1_000_000).max(1) as u64;
    let mut levels = Levels::new(cols, lanes, channels);
    let mut spectrum = Spectrum::new(rate);
    let (f_lo, f_hi) = spectrum.range();
    let n = spectrum.n;
    let mut spec: Vec<u8> = Vec::with_capacity(cols * BINS);
    let mut bands: Vec<u8> = Vec::with_capacity(cols * BANDS);
    let (mut pcm, mut folded) = (Vec::new(), Vec::new());
    let cancelled = || DecodeError::Cancelled;
    let mut report = None;
    match mode {
        Mode::Exact => {
            // Window `c` is centred on its column's middle.
            let start = |c: usize| -> i64 {
                ((c as u128 * 2 + 1) * u128::from(total) / (cols as u128 * 2)) as i64 - n as i64 / 2
            };
            let mut meter = loudness::Meter::new(rate, ch);
            let mut tp = vec![0.0f32; ch];
            let mut buf: Vec<f32> = Vec::new();
            let mut buf_start: i64 = 0;
            let mut pos: u64 = 0;
            let mut col = 0;
            let mut window = vec![0.0f32; n];
            let take = |buf: &[f32], buf_start: i64, from: i64, window: &mut Vec<f32>| {
                for (i, w) in window.iter_mut().enumerate() {
                    let at = from + i as i64 - buf_start;
                    *w = if at >= 0 {
                        buf.get(at as usize).copied().unwrap_or(0.0)
                    } else {
                        0.0
                    };
                }
            };
            for k in 0..td.len() {
                if k % 64 == 0 && cancel.is_cancelled() {
                    return Err(cancelled());
                }
                pcm.clear();
                td.decode(k, &mut pcm);
                folded.clear();
                fold_stereo(&pcm, ch, &mut folded);
                let fch = ch.min(2);
                for (f, src) in folded.chunks_exact(fch).zip(pcm.chunks_exact(ch)) {
                    let c = (u128::from(pos) * cols as u128 / u128::from(total)) as usize;
                    let mut mono = 0.0;
                    for (lane, &v) in f.iter().enumerate() {
                        levels.add(c, lane, v);
                        mono += v;
                    }
                    levels.pair(c, f);
                    meter.push_frame(src, &mut tp);
                    levels.meter(c, src, &tp);
                    buf.push(mono / fch as f32);
                    pos += 1;
                }
                while col < cols && start(col) + n as i64 <= buf_start + buf.len() as i64 {
                    take(&buf, buf_start, start(col), &mut window);
                    spectrum.levels(&window, &mut spec, &mut bands);
                    col += 1;
                    let keep = if col < cols { start(col) } else { i64::MAX };
                    let drop = (keep - buf_start).clamp(0, buf.len() as i64) as usize;
                    buf.drain(..drop);
                    buf_start += drop as i64;
                }
            }
            while col < cols {
                take(&buf, buf_start, start(col), &mut window);
                spectrum.levels(&window, &mut spec, &mut bands);
                col += 1;
            }
            report = Some(Arc::new(meter.report()));
        }
        Mode::Sampled(windows, from_us) => {
            let first_frame = u128::from(from_us) * u128::from(rate) / 1_000_000;
            let windows = windows.clamp(1, cols);
            let ts = u128::from(t.timescale.max(1));
            let mut window = Vec::with_capacity(n);
            let (mut one, mut one_bands) = (Vec::with_capacity(BINS), Vec::with_capacity(BANDS));
            let mut tps = vec![loudness::TruePeak::default(); ch];
            let mut tp = vec![0.0f32; ch];
            for w in 0..windows {
                if cancel.is_cancelled() {
                    return Err(cancelled());
                }
                // The window's first sample, in the track's ticks, and the coded sample
                // holding it.
                let centre =
                    first_frame + (w as u128 * 2 + 1) * u128::from(total) / (windows as u128 * 2);
                let first = centre.saturating_sub(n as u128 / 2);
                let tick =
                    (first * ts / u128::from(rate.max(1))) as u64 + t.edit.start.max(0) as u64;
                let k = t
                    .samples
                    .partition_point(|s| s.dts <= tick)
                    .saturating_sub(1);
                let from = k.saturating_sub(td.preroll());
                td.reset();
                pcm.clear();
                for j in from..k {
                    td.decode(j, &mut pcm);
                }
                pcm.clear();
                // Into the sample: the frames before the window's start.
                let into = t.samples.get(k).map_or(0, |s| {
                    (u128::from(tick.saturating_sub(s.dts)) * u128::from(rate) / ts) as usize
                });
                window.clear();
                let mut j = k;
                let mut skip = into;
                let c0 = w * cols / windows;
                let c1 = ((w + 1) * cols / windows).max(c0 + 1).min(cols);
                while window.len() < n && j < td.len() {
                    pcm.clear();
                    td.decode(j, &mut pcm);
                    j += 1;
                    folded.clear();
                    fold_stereo(&pcm, ch, &mut folded);
                    let fch = ch.min(2);
                    for (f, src) in folded.chunks_exact(fch).zip(pcm.chunks_exact(ch)) {
                        if skip > 0 {
                            skip -= 1;
                            continue;
                        }
                        if window.len() >= n {
                            break;
                        }
                        let mut mono = 0.0;
                        for (lane, &v) in f.iter().enumerate() {
                            for c in c0..c1 {
                                levels.add(c, lane, v);
                            }
                            mono += v;
                        }
                        for c in c0..c1 {
                            levels.pair(c, f);
                        }
                        for (p, (o, &v)) in tps.iter_mut().zip(tp.iter_mut().zip(src)) {
                            *o = p.push(v);
                        }
                        for c in c0..c1 {
                            levels.meter(c, src, &tp);
                        }
                        window.push(mono / fch as f32);
                    }
                }
                one.clear();
                one_bands.clear();
                spectrum.levels(&window, &mut one, &mut one_bands);
                for _ in c0..c1 {
                    spec.extend_from_slice(&one);
                    bands.extend_from_slice(&one_bands);
                }
            }
            spec.resize(cols * BINS, 0);
            bands.resize(cols * BANDS, 0);
        }
    }
    let [wave, meters, stereo] = levels.finish();
    Ok(Some(Analysis {
        cols,
        lanes,
        channels,
        duration_s: duration_us as f64 / 1e6,
        wave,
        spec,
        bands,
        meters,
        stereo,
        f_lo: f_lo as f32,
        f_hi: f_hi as f32,
        exact: matches!(mode, Mode::Exact),
        loudness: report,
    }))
}

/// The loudness of a movie's sound track `track` (`None`: the one played by default), every
/// sample read (for Info, on a worker). `None` without a track that can be decoded.
pub fn movie_loudness(
    source: &Arc<Source>,
    track: Option<u64>,
    cancel: &CancelToken,
) -> Result<Option<loudness::Report>> {
    let Ok(movie) = crate::parse_movie_with(source.bytes(), true, track) else {
        return Ok(None);
    };
    let Some(t) = movie.audio.as_ref() else {
        return Ok(None);
    };
    measure_loudness(t, source, cancel)
}

/// Measures the loudness of all of `t` (for sound too long to read every sample of for the
/// picture). `None` when the sound can't be decoded.
fn measure_loudness(
    t: &AudioTrack,
    source: &Arc<Source>,
    cancel: &CancelToken,
) -> Result<Option<loudness::Report>> {
    let Ok(mut td) = TrackDecoder::new(t, source) else {
        return Ok(None);
    };
    let mut meter = loudness::Meter::new(td.rate, usize::from(td.ch.max(1)));
    let mut pcm = Vec::new();
    for k in 0..td.len() {
        if k % 64 == 0 && cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        pcm.clear();
        td.decode(k, &mut pcm);
        meter.push(&pcm);
    }
    Ok(Some(meter.report()))
}

/// Where things are on a picture of `w × h`.
#[derive(Clone, Copy, Debug)]
struct Layout {
    margin: u32,
    /// The cover's box (x, y, width, height), when there is a cover.
    cover: Option<(u32, u32, u32, u32)>,
    plot_x: u32,
    plot_w: u32,
    wave_y: u32,
    wave_h: u32,
    spec_y: u32,
    spec_h: u32,
}

impl Layout {
    fn new(w: u32, h: u32, cover: bool) -> Layout {
        let margin = (w / 64).max(2);
        let inner_h = h.saturating_sub(margin * 2);
        let (cover_box, top) = if cover {
            let ch = inner_h / 2;
            (
                Some((margin, margin, w.saturating_sub(margin * 2), ch)),
                margin + ch + margin,
            )
        } else {
            (None, margin)
        };
        let rest = (h - margin).saturating_sub(top);
        let gap = margin / 2;
        let wave_h = rest.saturating_sub(gap) / 2;
        let spec_h = rest.saturating_sub(gap + wave_h);
        Layout {
            margin,
            cover: cover_box,
            plot_x: margin,
            plot_w: w.saturating_sub(margin * 2).max(1),
            wave_y: top,
            wave_h,
            spec_y: top + wave_h + gap,
            spec_h,
        }
    }
}

/// Where in the sound a point of the picture is (0 start, 1 end), when it is on the
/// waveform or the spectrogram. `x`, `y` are picture pixels; `cover`: the cover is shown
/// ([`SoundFacts::cover_shown`]).
pub fn time_at(cover: bool, x: f32, y: f32) -> Option<f64> {
    let l = Layout::new(WIDTH, HEIGHT, cover);
    let (x0, w) = (l.plot_x as f32, l.plot_w as f32);
    let on =
        (x0..=x0 + w).contains(&x) && (l.wave_y as f32..=(l.spec_y + l.spec_h) as f32).contains(&y);
    on.then(|| f64::from((x - x0) / w).clamp(0.0, 1.0))
}

fn put(px: &mut [u8], w: u32, x: u32, y: u32, c: [u8; 3]) {
    let i = (y as usize * w as usize + x as usize) * 4;
    if let Some(p) = px.get_mut(i..i + 4) {
        p.copy_from_slice(&[c[0], c[1], c[2], 255]);
    }
}

fn ramp(v: f32) -> [u8; 3] {
    let x = v.clamp(0.0, 1.0) * (RAMP.len() - 1) as f32;
    let i = (x as usize).min(RAMP.len() - 2);
    let t = x - i as f32;
    let (a, b) = (RAMP[i], RAMP[i + 1]);
    [0, 1, 2].map(|k| (a[k] + (b[k] - a[k]) * t).round() as u8)
}

/// Draws the picture as RGBA8: the cover, the waveform (played up to `played_to`, a
/// fraction of the length, in the played colours) and the spectrogram.
fn render(
    w: u32,
    h: u32,
    a: Option<&Analysis>,
    cover: Option<&Picture>,
    played_to: Option<f64>,
) -> Vec<u8> {
    let l = Layout::new(w, h, cover.is_some());
    let mut px = vec![0u8; w as usize * h as usize * 4];
    for p in px.chunks_exact_mut(4) {
        p.copy_from_slice(&[BACKGROUND[0], BACKGROUND[1], BACKGROUND[2], 255]);
    }
    if let (Some(c), Some((bx, by, bw, bh))) = (cover, l.cover) {
        draw_fitted(&mut px, w, c, bx, by, bw, bh);
    }
    // The column range of pixel x.
    let cols_of = |x: u32, cols: usize| {
        let c0 = x as usize * cols / l.plot_w as usize;
        let c1 = ((x as usize + 1) * cols / l.plot_w as usize)
            .max(c0 + 1)
            .min(cols);
        c0..c1
    };
    let played_x = played_to.map(|f| (f.clamp(0.0, 1.0) * f64::from(l.plot_w)) as u32);
    // Waveform lanes.
    let lanes = a.map_or(1, |a| a.lanes) as u32;
    let gap = (l.margin / 4).max(1);
    let lane_h = l.wave_h.saturating_sub(gap * (lanes - 1)) / lanes;
    for lane in 0..lanes {
        let y0 = l.wave_y + lane * (lane_h + gap);
        let mid = y0 + lane_h / 2;
        let half = (lane_h / 2).max(1) as f32;
        for x in 0..l.plot_w {
            let played = played_x.is_some_and(|p| x < p);
            let (wave, rms) = if played {
                (PLAYED, PLAYED_RMS)
            } else {
                (WAVE, WAVE_RMS)
            };
            for y in y0..y0 + lane_h {
                put(&mut px, w, l.plot_x + x, y, LANE);
            }
            put(&mut px, w, l.plot_x + x, mid, AXIS);
            let Some(a) = a else { continue };
            let (mut lo, mut hi, mut r) = (0.0f32, 0.0f32, 0.0f32);
            for c in cols_of(x, a.cols) {
                let v = a.wave[c * a.lanes + lane as usize];
                lo = lo.min(v[0]);
                hi = hi.max(v[1]);
                r = r.max(v[2]);
            }
            let to_y = |v: f32| (mid as f32 - v.clamp(-1.0, 1.0) * half).round() as u32;
            let (top, bottom) = (to_y(hi), to_y(lo));
            for y in top.max(y0)..=bottom.min(y0 + lane_h - 1) {
                put(&mut px, w, l.plot_x + x, y, wave);
            }
            let (rt, rb) = (to_y(r.min(hi.max(0.0))), to_y(-r.min((-lo).max(0.0))));
            for y in rt.max(y0)..=rb.min(y0 + lane_h - 1) {
                put(&mut px, w, l.plot_x + x, y, rms);
            }
        }
    }
    // Spectrogram: high frequencies at the top.
    for x in 0..l.plot_w {
        for y in 0..l.spec_h {
            let v = a.map_or(0.0, |a| {
                let b = (1.0 - (y as f32 + 0.5) / l.spec_h as f32) * BINS as f32 - 0.5;
                let b0 = (b.floor().max(0.0) as usize).min(BINS - 1);
                let b1 = (b0 + 1).min(BINS - 1);
                let t = (b - b0 as f32).clamp(0.0, 1.0);
                cols_of(x, a.cols)
                    .map(|c| a.level(c, b0) * (1.0 - t) + a.level(c, b1) * t)
                    .fold(0.0, f32::max)
            });
            put(&mut px, w, l.plot_x + x, l.spec_y + y, ramp(v));
        }
    }
    px
}

/// Draws `c` scaled to fit the box (aspect kept, centred): area-averaged when shrinking,
/// bilinear when enlarging.
fn draw_fitted(px: &mut [u8], w: u32, c: &Picture, bx: u32, by: u32, bw: u32, bh: u32) {
    if c.w == 0 || c.h == 0 || bw == 0 || bh == 0 {
        return;
    }
    let s = (f64::from(bw) / f64::from(c.w)).min(f64::from(bh) / f64::from(c.h));
    let dw = ((f64::from(c.w) * s).round() as u32).clamp(1, bw);
    let dh = ((f64::from(c.h) * s).round() as u32).clamp(1, bh);
    let fitted = scale_rgba(&c.rgba, c.w, c.h, dw, dh);
    let (ox, oy) = (bx + (bw - dw) / 2, by + (bh - dh) / 2);
    for y in 0..dh {
        for x in 0..dw {
            let i = (y as usize * dw as usize + x as usize) * 4;
            let p = &fitted[i..i + 4];
            // Over the background by the picture's alpha.
            let al = f32::from(p[3]) / 255.0;
            let mix = |k: usize| {
                (f32::from(p[k]) * al + f32::from(BACKGROUND[k]) * (1.0 - al)).round() as u8
            };
            put(px, w, ox + x, oy + y, [mix(0), mix(1), mix(2)]);
        }
    }
}

/// Scales RGBA8 rows: area averaging down, bilinear up.
fn scale_rgba(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32) -> Vec<u8> {
    let (sw_, sh_) = (sw as usize, sh as usize);
    let mut out = vec![0u8; dw as usize * dh as usize * 4];
    let fx = f64::from(sw) / f64::from(dw);
    let fy = f64::from(sh) / f64::from(dh);
    let at = |x: usize, y: usize, k: usize| {
        f64::from(src[(y.min(sh_ - 1) * sw_ + x.min(sw_ - 1)) * 4 + k])
    };
    for y in 0..dh as usize {
        for x in 0..dw as usize {
            let o = (y * dw as usize + x) * 4;
            if fx >= 1.0 && fy >= 1.0 {
                let x0 = (x as f64 * fx) as usize;
                let x1 = (((x + 1) as f64 * fx) as usize).clamp(x0 + 1, sw_.max(x0 + 1));
                let y0 = (y as f64 * fy) as usize;
                let y1 = (((y + 1) as f64 * fy) as usize).clamp(y0 + 1, sh_.max(y0 + 1));
                let n = ((x1 - x0) * (y1 - y0)) as f64;
                for k in 0..4 {
                    let mut sum = 0.0;
                    for yy in y0..y1 {
                        for xx in x0..x1 {
                            sum += at(xx, yy, k);
                        }
                    }
                    out[o + k] = (sum / n).round() as u8;
                }
            } else {
                let sx = ((x as f64 + 0.5) * fx - 0.5).max(0.0);
                let sy = ((y as f64 + 0.5) * fy - 0.5).max(0.0);
                let (ix, iy) = (sx as usize, sy as usize);
                let (tx, ty) = (sx - ix as f64, sy - iy as f64);
                for k in 0..4 {
                    let top = at(ix, iy, k) * (1.0 - tx) + at(ix + 1, iy, k) * tx;
                    let bot = at(ix, iy + 1, k) * (1.0 - tx) + at(ix + 1, iy + 1, k) * tx;
                    out[o + k] = (top * (1.0 - ty) + bot * ty).round() as u8;
                }
            }
        }
    }
    out
}

fn rgba_raster(w: u32, h: u32, data: Vec<u8>) -> Raster {
    Raster {
        width: w,
        height: h,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Rgba,
        planes: vec![RasterPlane {
            width: w,
            height: h,
            stride: w as usize * 4,
            data,
            h_div: 1,
            v_div: 1,
        }],
        truncated: false,
    }
}

/// The timeline of a sound file as frames (one per [`FRAME`], at most [`MAX_FRAMES`]): each
/// the page's picture as it stands (the app's sound page draws the playhead itself, on the
/// GPU), or, for a player that only needs the times ([`SoundSession::clock_frames`]), a
/// transparent pixel.
struct PageFrames {
    /// The page's picture; None: a transparent pixel.
    picture: Option<Arc<Vec<u8>>>,
    count: u32,
    delay: Duration,
    duration_us: u64,
    next: u32,
}

impl Frames for PageFrames {
    fn next_frame(&mut self, _cancel: &CancelToken) -> Option<Result<Frame>> {
        if self.next >= self.count {
            return None;
        }
        let i = self.next;
        self.next += 1;
        let t_us = u64::from(i) * self.delay.as_micros() as u64;
        let last = i + 1 == self.count;
        let delay = if last {
            Duration::from_micros(self.duration_us.saturating_sub(t_us).max(1))
        } else {
            self.delay
        };
        let raster = match &self.picture {
            Some(px) => rgba_raster(WIDTH, HEIGHT, px.as_ref().clone()),
            None => rgba_raster(1, 1, vec![0; 4]),
        };
        Some(Ok(Frame {
            index: i,
            raster,
            delay,
        }))
    }

    fn rewind(&mut self) -> Result<()> {
        self.next = 0;
        Ok(())
    }

    fn seek(&mut self, index: u32) -> Result<bool> {
        self.next = index.min(self.count);
        Ok(true)
    }

    fn delays(&self) -> Option<Vec<Duration>> {
        let mut d = vec![self.delay; self.count as usize];
        if let Some(last) = d.last_mut() {
            let before = u64::from(self.count - 1) * self.delay.as_micros() as u64;
            *last = Duration::from_micros(self.duration_us.saturating_sub(before).max(1));
        }
        Some(d)
    }
}

impl Session for SoundSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode_sampled(&self, long_side: u32, cancel: &CancelToken) -> Option<Result<Raster>> {
        // The cover, else a sampled waveform.
        if let Some(c) = &self.cover {
            let s = (f64::from(long_side.max(1)) / f64::from(c.w.max(c.h))).min(1.0);
            let (w, h) = (
                ((f64::from(c.w) * s).round() as u32).max(1),
                ((f64::from(c.h) * s).round() as u32).max(1),
            );
            let mut px = scale_rgba(&c.rgba, c.w, c.h, w, h);
            for p in px.chunks_exact_mut(4) {
                p[3] = 255;
            }
            return Some(Ok(rgba_raster(w, h, px)));
        }
        let w = long_side.clamp(64, WIDTH);
        let h = w * HEIGHT / WIDTH;
        let l = Layout::new(w, h, false);
        let a = self.movie.audio.as_ref().map(|t| {
            analyse(
                t,
                &self.source,
                self.duration_us,
                l.plot_w as usize,
                Mode::Sampled(THUMB_WINDOWS, 0),
                cancel,
            )
        });
        let a = match a {
            Some(Err(e)) => return Some(Err(e)),
            Some(Ok(a)) => a,
            None => None,
        };
        Some(Ok(rgba_raster(w, h, render(w, h, a.as_ref(), None, None))))
    }
    fn decode(&self, _req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.picture(cancel, None)
    }
    fn decode_memory(&self, _req: &DecodeRequest) -> u64 {
        u64::from(WIDTH) * u64::from(HEIGHT) * 4 * 3
    }
    fn animation(&self) -> Option<&AnimationInfo> {
        self.anim.as_ref()
    }
    fn frames(&self, _scale: u32) -> Result<Box<dyn Frames>> {
        let cancel = CancelToken::new();
        let a = self.analysis(&cancel)?;
        let picture = render(WIDTH, HEIGHT, a.as_deref(), self.cover.as_deref(), None);
        Ok(self.page_frames(Some(Arc::new(picture))))
    }
    fn video(&self) -> Option<&VideoFacts> {
        Some(&self.facts)
    }
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn search_text(&self) -> Option<String> {
        self.lyrics().map(|l| l.text()).filter(|t| !t.is_empty())
    }
    fn memory_bytes(&self) -> usize {
        let a = self.analysis.lock().as_ref().map_or(0, |a| a.bytes());
        a + self.cover.as_ref().map_or(0, |c| c.rgba.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_fit() {
        for (w, h, c) in [
            (1600, 1000, true),
            (1600, 1000, false),
            (64, 40, false),
            (256, 160, true),
        ] {
            let l = Layout::new(w, h, c);
            assert!(l.spec_y + l.spec_h <= h, "{l:?}");
            assert!(l.plot_x + l.plot_w <= w);
            assert!(l.wave_y + l.wave_h <= l.spec_y);
        }
    }

    #[test]
    fn clicks_on_the_graphs_give_their_time() {
        let l = Layout::new(WIDTH, HEIGHT, true);
        let mid_y = (l.spec_y + l.spec_h / 2) as f32;
        let x = |f: f32| l.plot_x as f32 + f * l.plot_w as f32;
        assert_eq!(time_at(true, x(0.0), mid_y), Some(0.0));
        assert!((time_at(true, x(0.25), l.wave_y as f32 + 1.0).unwrap() - 0.25).abs() < 1e-6);
        assert_eq!(time_at(true, x(1.0), mid_y), Some(1.0));
        // The cover and the margins are not the graphs.
        assert_eq!(time_at(true, x(0.5), (l.margin + 10) as f32), None);
        assert_eq!(time_at(true, 0.5, mid_y), None);
        // Without a cover the graphs fill the picture.
        assert!(time_at(false, x(0.5), (l.margin + 10) as f32).is_some());
    }

    #[test]
    fn ramp_ends() {
        assert_eq!(ramp(0.0), [2, 2, 8]);
        assert_eq!(ramp(1.0), [252, 248, 185]);
        assert_eq!(ramp(2.0), [252, 248, 185]);
    }

    #[test]
    fn scaling_keeps_flat_colour() {
        let src = [10u8, 20, 30, 255].repeat(7 * 5);
        for (dw, dh) in [(3, 2), (14, 10), (7, 5)] {
            let out = scale_rgba(&src, 7, 5, dw, dh);
            assert!(out.chunks_exact(4).all(|p| p == [10, 20, 30, 255]));
        }
    }

    #[test]
    fn a_sine_peaks_at_its_bin() {
        let mut s = Spectrum::new(48_000);
        let n = s.n;
        let win: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48_000.0).sin())
            .collect();
        let (mut out, mut bands) = (Vec::new(), Vec::new());
        s.levels(&win, &mut out, &mut bands);
        let peak = out.iter().enumerate().max_by_key(|a| *a.1).unwrap();
        // Full scale (0 dBFS within a dB) at 1 kHz on the log axis.
        assert!(*peak.1 >= 252, "{peak:?}");
        // The third-octave band of 1 kHz (band 17) reads full scale; its neighbours far less.
        assert_eq!(bands.len(), BANDS);
        assert!(bands[17] >= 252, "{bands:?}");
        assert!(bands[14] < 200 && bands[20] < 200, "{bands:?}");
        let f = F_MIN * (24_000.0 / F_MIN).powf((peak.0 as f64 + 0.5) / BINS as f64);
        assert!((f / 1000.0 - 1.0).abs() < 0.05, "{f}");
    }

    /// 70 s of mono 8 kHz: silence, then a 0.5 tone from 40 s.
    fn step_wave() -> Vec<u8> {
        let rate = 8_000u32;
        let n = 70 * rate as usize;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + n as u32 * 2).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&rate.to_le_bytes());
        b.extend_from_slice(&(rate * 2).to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(n as u32 * 2).to_le_bytes());
        for i in 0..n {
            let v = if i >= 40 * rate as usize {
                0.5 * (2.0 * std::f64::consts::PI * 1000.0 * i as f64 / f64::from(rate)).sin()
            } else {
                0.0
            };
            b.extend_from_slice(&((v * 32767.0) as i16).to_le_bytes());
        }
        b
    }

    /// A span's analysis puts the tone's start where it is in the span.
    #[test]
    fn a_span_is_analysed_where_it_is() {
        let src = Arc::new(Source::from_vec("span.wav", step_wave()));
        let s = crate::session::open_any(src, Format::Wave).unwrap();
        let snd = s.as_any().unwrap().downcast_ref::<SoundSession>().unwrap();
        let cancel = CancelToken::new();
        let a = snd
            .analysis_of(35_000_000, 10_000_000, 100, &cancel)
            .unwrap()
            .unwrap();
        assert_eq!(a.cols, 100);
        assert!((a.duration_s - 10.0).abs() < 1e-6);
        assert!(!a.exact);
        // Column c covers 35 + c/10 s: silent before 40 s, the tone after.
        let peak = |c: usize| a.wave[c][1];
        for c in 0..48 {
            assert!(peak(c) < 0.01, "column {c}: {}", peak(c));
        }
        for c in 52..100 {
            assert!(peak(c) > 0.45, "column {c}: {}", peak(c));
        }
        // The spectrogram agrees: the tone's bin is dark, then bright.
        let k = (0..BINS).max_by_key(|&b| a.spec[90 * BINS + b]).unwrap();
        assert!(a.spec[90 * BINS + k] > 200 && a.spec[10 * BINS + k] < 60);
    }
}
