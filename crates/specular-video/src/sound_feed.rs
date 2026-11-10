//! What the sound page's live displays show at a moment of the sound: the spectrum, the
//! levels of each channel and the loudness, from a [`Feed`], with meter ballistics.
//!
//! - [`Feed`] is the input: the whole-file [`Analysis`] looked up at the playing position
//!   ([`AnalysisFeed`]), or the samples being heard ([`LiveFeed`], from the live tap,
//!   `crate::live_tap`), which falls back to the analysis while nothing is heard (paused,
//!   seeking, no sound device).
//! - [`Ballistics`] turns snapshots into what is drawn: bars rise at once and fall at a set
//!   rate, peaks hold then fall, a clip stays lit until cleared. With reduce motion the
//!   falls are slower; the values stay live (they are information, like a clock).

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use parking_lot::Mutex;

use crate::live_tap::Tap;
use crate::loudness;
use crate::sound_view::{Analysis, BANDS, BINS, MAX_CH, Spectrum};
use crate::stereo;

/// The floor of every level shown (dBFS).
pub const FLOOR_DB: f32 = -100.0;
/// The span a peak and a true peak are taken over, and an RMS level's (seconds).
pub const PEAK_S: f64 = 0.05;
pub const RMS_S: f64 = 0.3;

/// The sound around one moment.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// The spectrogram's bins (dBFS), lowest frequency first.
    pub bins: [f32; BINS],
    /// Third-octave bands (dBFS).
    pub bands: [f32; BANDS],
    pub channels: usize,
    /// Per channel over the last [`PEAK_S`]: sample peak (dBFS) and true peak (dBTP); over
    /// the last [`RMS_S`]: RMS (dBFS, a full-scale sine reads −3).
    pub peak: [f32; MAX_CH],
    pub true_peak: [f32; MAX_CH],
    pub rms: [f32; MAX_CH],
    /// Momentary and short-term loudness (LUFS), when measured.
    pub momentary: Option<f32>,
    pub short_term: Option<f32>,
    /// The phase correlation of left and right over [`stereo::CORRELATION_S`] (−1…+1);
    /// `None` for mono or silence.
    pub correlation: Option<f32>,
    /// Taken from the sound being heard (else from the whole-file analysis).
    pub live: bool,
}

impl Default for Snapshot {
    fn default() -> Self {
        Snapshot {
            bins: [FLOOR_DB; BINS],
            bands: [FLOOR_DB; BANDS],
            channels: 0,
            peak: [FLOOR_DB; MAX_CH],
            true_peak: [FLOOR_DB; MAX_CH],
            rms: [FLOOR_DB; MAX_CH],
            momentary: None,
            short_term: None,
            correlation: None,
            live: false,
        }
    }
}

/// Where the live displays' sound comes from.
pub trait Feed: Send + Sync {
    /// Fills `out` with the sound heard at `t` seconds; false when there is none to show.
    fn snapshot(&self, t: f64, out: &mut Snapshot) -> bool;

    /// The last `frames` frames heard up to `t` (interleaved into `out`), for the
    /// goniometer; their channel count, or `None` when no samples are at hand.
    fn samples(&self, _t: f64, _frames: usize, _out: &mut Vec<f32>) -> Option<usize> {
        None
    }
}

fn to_db(level: f32) -> f32 {
    if level > 0.0 {
        (20.0 * level.log10()).max(FLOOR_DB)
    } else {
        FLOOR_DB
    }
}

fn u8_db(v: u8) -> f32 {
    f32::from(v) / 255.0 * 100.0 - 100.0
}

/// The whole-file analysis read at the playing position, with its loudness (from the
/// analysis, or measured on its own for long sound).
pub struct AnalysisFeed {
    pub analysis: std::sync::Arc<Analysis>,
    pub loudness: Option<std::sync::Arc<loudness::Report>>,
}

impl Feed for AnalysisFeed {
    fn snapshot(&self, t: f64, out: &mut Snapshot) -> bool {
        let a = &self.analysis;
        if a.cols == 0 || a.duration_s <= 0.0 {
            return false;
        }
        let per_s = a.cols as f64 / a.duration_s;
        // Spectra: between the two columns whose centres are either side of `t`.
        let x = (t * per_s - 0.5).clamp(0.0, (a.cols - 1) as f64);
        let c0 = x.floor() as usize;
        let c1 = (c0 + 1).min(a.cols - 1);
        let f = (x - c0 as f64) as f32;
        let mix = |v0: u8, v1: u8| u8_db(v0) * (1.0 - f) + u8_db(v1) * f;
        for (b, o) in out.bins.iter_mut().enumerate() {
            *o = mix(a.spec[c0 * BINS + b], a.spec[c1 * BINS + b]);
        }
        for (b, o) in out.bands.iter_mut().enumerate() {
            *o = mix(a.bands[c0 * BANDS + b], a.bands[c1 * BANDS + b]);
        }
        // Levels: the columns of the span ending at `t`.
        let ch = a.channels;
        out.channels = ch;
        let end = ((t * per_s).ceil() as usize).clamp(1, a.cols);
        let span = |secs: f64| {
            let n = ((secs * per_s).ceil() as usize).max(1);
            end.saturating_sub(n)..end
        };
        for i in 0..MAX_CH {
            out.peak[i] = FLOOR_DB;
            out.true_peak[i] = FLOOR_DB;
            out.rms[i] = FLOOR_DB;
        }
        for i in 0..ch {
            let (mut p, mut tp) = (0.0f32, 0.0f32);
            for c in span(PEAK_S) {
                let m = a.meters[c * ch + i];
                p = p.max(m[0]);
                tp = tp.max(m[2]);
            }
            let r = span(RMS_S);
            let n = r.len().max(1);
            let ms: f32 = r.map(|c| a.meters[c * ch + i][1]).sum::<f32>() / n as f32;
            out.peak[i] = to_db(p);
            out.true_peak[i] = to_db(tp);
            out.rms[i] = to_db(ms.sqrt());
        }
        // Correlation: the stereo columns of its span.
        out.correlation = None;
        if a.stereo.len() == a.cols {
            let (mut lr, mut ll, mut rr) = (0.0f64, 0.0f64, 0.0f64);
            for c in span(stereo::CORRELATION_S) {
                let [x, l, r] = a.stereo[c];
                lr += f64::from(x);
                ll += f64::from(l);
                rr += f64::from(r);
            }
            out.correlation = stereo::correlation(lr, ll, rr);
        }
        out.live = false;
        // Loudness: the 100 ms step ending at or before `t`.
        out.momentary = None;
        out.short_term = None;
        if let Some(l) = &self.loudness {
            let k = (t * f64::from(loudness::STEPS_PER_S)).floor() as usize;
            if let Some(k) = k.checked_sub(1).or((t > 0.0).then_some(0)) {
                let k = k.min(l.momentary.len().saturating_sub(1));
                let finite = |v: f32| v.is_finite().then_some(v.max(FLOOR_DB));
                out.momentary = l.momentary.get(k).copied().and_then(finite);
                out.short_term = l.short_term.get(k).copied().and_then(finite);
            }
        }
        true
    }
}

/// The sound being heard, from the live tap; the whole-file analysis where the tap has
/// nothing (paused, seeking, no device) and for what the tap does not give: the loudness
/// numbers (the file's own), and the meters when the device's channels are not the sound's
/// (they show the sound's channels).
pub struct LiveFeed {
    pub tap: Arc<Tap>,
    pub fallback: AnalysisFeed,
    /// The sound plays this much later than the clock asked for (µs; the player's sound
    /// delay): what is heard at `t` is the sound of `t − delay`.
    pub delay_us: Option<Arc<AtomicI64>>,
    scratch: Mutex<Scratch>,
}

#[derive(Default)]
struct Scratch {
    spectrum: Option<Spectrum>,
    pcm: Vec<f32>,
    mono: Vec<f32>,
}

impl LiveFeed {
    pub fn new(tap: Arc<Tap>, fallback: AnalysisFeed) -> LiveFeed {
        LiveFeed {
            tap,
            fallback,
            delay_us: None,
            scratch: Mutex::new(Scratch::default()),
        }
    }

    /// The sound's time heard at clock time `t`.
    fn heard(&self, t: f64) -> f64 {
        let d = self
            .delay_us
            .as_ref()
            .map_or(0, |d| d.load(Ordering::Relaxed));
        t - d as f64 / 1e6
    }

    /// Replaces what `out` has from the analysis with what the tap heard; whether it had it.
    fn live(&self, t: f64, out: &mut Snapshot) -> bool {
        let mut guard = self.scratch.lock();
        let sc = &mut *guard;
        let Some((_, rate)) = self.tap.format() else {
            return false;
        };
        if sc.spectrum.as_ref().is_none_or(|s| s.rate() != rate) {
            sc.spectrum = Some(Spectrum::new(rate));
        }
        let Some(spectrum) = sc.spectrum.as_mut() else {
            return false;
        };
        let n = spectrum.n;
        let rms_n = (RMS_S * f64::from(rate)).ceil() as usize;
        let Some(h) = self.tap.read_at(t, n.max(rms_n), &mut sc.pcm) else {
            return false;
        };
        if h.rate != rate {
            return false;
        }
        let ch = h.channels;
        let pcm = &sc.pcm;
        // The spectrum: the channels' mean over the newest window.
        sc.mono.clear();
        sc.mono.extend(
            pcm[(h.frames - n) * ch..]
                .chunks_exact(ch)
                .map(|f| f.iter().sum::<f32>() / ch as f32),
        );
        spectrum.levels_db(&sc.mono, FLOOR_DB, &mut out.bins, &mut out.bands);
        // The meters, when the device plays the sound's own channels.
        if ch == out.channels {
            let peak_n = ((PEAK_S * f64::from(rate)).ceil() as usize).min(h.frames);
            for i in 0..ch.min(MAX_CH) {
                let mut tp = loudness::TruePeak::default();
                let (mut p, mut t_p, mut sq) = (0.0f32, 0.0f32, 0.0f64);
                // The true-peak filter runs in over the frames before the span.
                let lead = 64.min(h.frames - peak_n);
                for (k, f) in pcm[(h.frames - peak_n - lead) * ch..]
                    .chunks_exact(ch)
                    .enumerate()
                {
                    let v = f[i];
                    let x = tp.push(v);
                    if k >= lead {
                        p = p.max(v.abs());
                        t_p = t_p.max(x);
                    }
                }
                let r0 = h.frames - rms_n.min(h.frames);
                for f in pcm[r0 * ch..].chunks_exact(ch) {
                    sq += f64::from(f[i]) * f64::from(f[i]);
                }
                let ms = sq / (h.frames - r0).max(1) as f64;
                out.peak[i] = to_db(p);
                out.true_peak[i] = to_db(t_p.max(p));
                out.rms[i] = to_db(ms.sqrt() as f32);
            }
        }
        // The correlation, of a stereo sound heard in stereo.
        if ch >= 2 && self.fallback.analysis.lanes == 2 {
            let cn = ((stereo::CORRELATION_S * f64::from(rate)).ceil() as usize).min(h.frames);
            out.correlation = stereo::correlation_of(&pcm[(h.frames - cn) * ch..], ch);
        }
        out.live = true;
        true
    }
}

impl Feed for LiveFeed {
    fn snapshot(&self, t: f64, out: &mut Snapshot) -> bool {
        let had = self.fallback.snapshot(t, out);
        if !had {
            *out = Snapshot::default();
        }
        self.live(self.heard(t), out) || had
    }

    fn samples(&self, t: f64, frames: usize, out: &mut Vec<f32>) -> Option<usize> {
        self.tap
            .read_at(self.heard(t), frames, out)
            .map(|h| h.channels)
    }
}

/// How fast things fall (dB a second) and how long peaks hold (seconds).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rates {
    pub bar_fall: f32,
    pub peak_hold: f32,
    pub peak_fall: f32,
    pub meter_fall: f32,
    /// The RMS reading's extra smoothing (time constant, seconds; 0: none).
    pub rms_smooth: f32,
}

impl Rates {
    /// Ordinary motion, or reduced.
    pub fn new(reduce_motion: bool) -> Rates {
        if reduce_motion {
            Rates {
                bar_fall: 8.0,
                peak_hold: 2.5,
                peak_fall: 4.0,
                meter_fall: 4.0,
                rms_smooth: 0.5,
            }
        } else {
            // Bars fall 24 dB/s (falls slower than 300 ms over a typical range); peak
            // meters 20 dB in 1.7 s (IEC 60268-18).
            Rates {
                bar_fall: 24.0,
                peak_hold: 1.0,
                peak_fall: 12.0,
                meter_fall: 11.8,
                rms_smooth: 0.0,
            }
        }
    }
}

/// A level with a held peak above it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Held {
    pub level: f32,
    pub peak: f32,
    /// Seconds the peak has held.
    held: f32,
}

impl Default for Held {
    fn default() -> Self {
        Held {
            level: FLOOR_DB,
            peak: FLOOR_DB,
            held: 0.0,
        }
    }
}

impl Held {
    fn set(&mut self, v: f32) {
        *self = Held {
            level: v,
            peak: v,
            held: 0.0,
        };
    }

    fn update(&mut self, v: f32, dt: f32, fall: f32, r: &Rates) {
        self.level = v.max(self.level - fall * dt).max(FLOOR_DB);
        if v >= self.peak {
            self.peak = v;
            self.held = 0.0;
        } else {
            self.held += dt;
            if self.held > r.peak_hold {
                self.peak = (self.peak - r.peak_fall * dt).max(self.level);
            }
        }
    }
}

/// What the live displays draw.
#[derive(Clone, Debug, PartialEq)]
pub struct Ballistics {
    pub bands: [Held; BANDS],
    pub bins: [Held; BINS],
    pub channels: usize,
    /// Per channel: the peak meter, the RMS reading, the true peak (held) and whether it
    /// clipped (over 0 dBFS / dBTP; lit until cleared).
    pub peak: [Held; MAX_CH],
    pub rms: [f32; MAX_CH],
    pub true_peak: [Held; MAX_CH],
    pub clip: [bool; MAX_CH],
    /// The highest true peak since the clip lights were cleared (dBTP).
    pub max_true_peak: [f32; MAX_CH],
    pub momentary: Option<f32>,
    pub short_term: Option<f32>,
    /// The phase correlation as measured, and its slow average ([`stereo::AVERAGE_S`]).
    pub correlation: Option<f32>,
    pub correlation_avg: Option<f32>,
    /// Set once a snapshot came in.
    pub live: bool,
}

impl Default for Ballistics {
    fn default() -> Self {
        Ballistics {
            bands: [Held::default(); BANDS],
            bins: [Held::default(); BINS],
            channels: 0,
            peak: [Held::default(); MAX_CH],
            rms: [FLOOR_DB; MAX_CH],
            true_peak: [Held::default(); MAX_CH],
            clip: [false; MAX_CH],
            max_true_peak: [FLOOR_DB; MAX_CH],
            momentary: None,
            short_term: None,
            correlation: None,
            correlation_avg: None,
            live: false,
        }
    }
}

impl Ballistics {
    /// Takes `s` as it is (a seek, a pause, the first snapshot): nothing falls into it.
    pub fn jump(&mut self, s: &Snapshot) {
        for (h, &v) in self.bands.iter_mut().zip(&s.bands) {
            h.set(v);
        }
        for (h, &v) in self.bins.iter_mut().zip(&s.bins) {
            h.set(v);
        }
        for i in 0..MAX_CH {
            self.peak[i].set(s.peak[i]);
            self.true_peak[i].set(s.true_peak[i]);
            self.rms[i] = s.rms[i];
        }
        self.correlation_avg = s.correlation;
        self.take_common(s);
    }

    /// Moves on by `dt` seconds of playing towards `s`.
    pub fn update(&mut self, s: &Snapshot, dt: f32, r: &Rates) {
        if !self.live {
            self.jump(s);
            return;
        }
        let dt = dt.clamp(0.0, 0.25);
        for (h, &v) in self.bands.iter_mut().zip(&s.bands) {
            h.update(v, dt, r.bar_fall, r);
        }
        for (h, &v) in self.bins.iter_mut().zip(&s.bins) {
            h.update(v, dt, r.bar_fall, r);
        }
        for i in 0..MAX_CH {
            self.peak[i].update(s.peak[i], dt, r.meter_fall, r);
            self.true_peak[i].update(s.true_peak[i], dt, r.meter_fall, r);
            self.rms[i] = if r.rms_smooth > 0.0 {
                let k = 1.0 - (-dt / r.rms_smooth).exp();
                self.rms[i] + (s.rms[i] - self.rms[i]) * k
            } else {
                s.rms[i]
            };
        }
        self.correlation_avg = match (self.correlation_avg, s.correlation) {
            (Some(avg), Some(c)) => {
                let k = 1.0 - (-dt / stereo::AVERAGE_S).exp();
                Some(avg + (c - avg) * k)
            }
            (avg, None) => avg,
            (None, c) => c,
        };
        self.take_common(s);
    }

    fn take_common(&mut self, s: &Snapshot) {
        self.channels = s.channels;
        for i in 0..s.channels.min(MAX_CH) {
            // A sample at full scale, or a true peak over it.
            if s.peak[i] >= -0.001 || s.true_peak[i] > 0.0 {
                self.clip[i] = true;
            }
            self.max_true_peak[i] = self.max_true_peak[i].max(s.true_peak[i]);
        }
        self.momentary = s.momentary;
        self.short_term = s.short_term;
        self.correlation = s.correlation;
        self.live = true;
    }

    /// Clears the clip lights and the highest true peaks.
    pub fn clear_clips(&mut self) {
        self.clip = [false; MAX_CH];
        self.max_true_peak = [FLOOR_DB; MAX_CH];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars_rise_at_once_and_fall_at_their_rate() {
        let r = Rates::new(false);
        let mut b = Ballistics::default();
        let mut s = Snapshot {
            channels: 2,
            ..Snapshot::default()
        };
        s.bands[5] = -10.0;
        b.update(&s, 0.016, &r);
        assert_eq!(b.bands[5].level, -10.0);
        s.bands[5] = -60.0;
        b.update(&s, 0.5, &r.clone());
        // 0.25 s at most a step: 6 dB.
        assert!((b.bands[5].level + 16.0).abs() < 1e-4, "{:?}", b.bands[5]);
        // The peak holds, then falls.
        assert_eq!(b.bands[5].peak, -10.0);
        for _ in 0..10 {
            b.update(&s, 0.1, &r);
        }
        assert!(b.bands[5].peak < -10.0);
        assert!(b.bands[5].peak >= b.bands[5].level);
        // Reduced motion falls slower.
        let slow = Rates::new(true);
        let mut c = Ballistics::default();
        s.bands[5] = -10.0;
        c.update(&s, 0.016, &slow);
        s.bands[5] = -60.0;
        c.update(&s, 0.1, &slow);
        assert!((c.bands[5].level + 10.8).abs() < 1e-4);
    }

    #[test]
    fn clips_stay_lit_until_cleared() {
        let r = Rates::new(false);
        let mut b = Ballistics::default();
        let mut s = Snapshot {
            channels: 2,
            ..Snapshot::default()
        };
        s.true_peak[1] = 0.4;
        b.update(&s, 0.016, &r);
        assert_eq!(
            b.clip,
            [false, true, false, false, false, false, false, false]
        );
        s.true_peak[1] = -20.0;
        for _ in 0..100 {
            b.update(&s, 0.1, &r);
        }
        assert!(b.clip[1]);
        assert_eq!(b.max_true_peak[1], 0.4);
        b.clear_clips();
        assert!(!b.clip[1]);
    }
}

#[cfg(test)]
#[path = "sound_feed_live_tests.rs"]
mod live_tests;
