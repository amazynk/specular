//! A movie's or sound file's sound as speech recognition reads it: 16 kHz mono, any stretch
//! of it on demand (generated subtitles start near the playhead, then fill the rest).
//!
//! The track is decoded by the same decoders playback uses ([`crate::audio`]), from the
//! coded sample holding the start (after the decoder's pre-roll), its channels averaged,
//! then resampled with a windowed-sinc low-pass (32 taps, cut-off 7.6 kHz, 512 phases) so
//! nothing above 8 kHz folds back into the speech band.
//!
//! Times are the sound's own (its first sample is 0), plus [`SpeechSound::offset`]: where
//! that sample sits on the movie's timeline (an edit list's start), the timeline subtitle
//! cues use.

use std::sync::Arc;

use specular_core::Source;

use crate::audio::{Silence, TrackDecoder};
use crate::mp4::AudioTrack;

/// The rate speech is read at.
pub const RATE: u32 = 16_000;
/// Taps of the resampling filter (each side of the centre: half).
const TAPS: usize = 32;
/// Filter phases between two input samples.
const PHASES: usize = 512;
/// Cut-off, as a share of the output's Nyquist frequency.
const CUTOFF: f64 = 0.95;

/// A sound track open for speech reading.
pub struct SpeechSound {
    dec: TrackDecoder,
    track: AudioTrack,
    duration: f64,
    /// `PHASES + 1` rows of `TAPS` filter weights.
    filter: Vec<f32>,
    /// Input samples per output sample.
    step: f64,
}

impl SpeechSound {
    /// The sound of the movie or sound file in `source` (track `track`, an id from
    /// [`crate::extras::read`], or the default one).
    pub fn open(source: Arc<Source>, track: Option<u64>) -> Result<SpeechSound, Silence> {
        let movie =
            crate::parse_movie_with(source.bytes(), true, track).map_err(|_| Silence::NoTrack)?;
        let t = movie.audio.ok_or(Silence::NoTrack)?;
        crate::audio::playable(&t)?;
        let dec = TrackDecoder::new(&t, &source)?;
        let ts = f64::from(t.timescale.max(1));
        let duration = t
            .samples
            .last()
            .map_or(0.0, |s| (s.dts + u64::from(s.duration)) as f64 / ts)
            - (t.edit.start.max(0) as f64 / ts);
        let step = f64::from(dec.rate.max(1)) / f64::from(RATE);
        Ok(SpeechSound {
            filter: filter(step),
            dec,
            track: t,
            duration: duration.max(0.0),
            step,
        })
    }

    /// Seconds of sound.
    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// Seconds from the movie timeline's 0 to the sound's first sample.
    pub fn offset(&self) -> f64 {
        let ts = f64::from(self.track.timescale.max(1));
        self.track.edit.delay_us as f64 / 1e6 - self.track.edit.start.min(0) as f64 / ts
    }

    /// `secs` seconds of 16 kHz mono from `start` (sound time), shorter at the end.
    pub fn read(&mut self, start: f64, secs: f64) -> Vec<f32> {
        let rate = f64::from(self.dec.rate.max(1));
        let ch = usize::from(self.dec.ch.max(1));
        let ts = u128::from(self.track.timescale.max(1));
        let start = start.clamp(0.0, self.duration);
        let want = ((secs.max(0.0).min(self.duration - start)) * f64::from(RATE)) as usize;
        if want == 0 {
            return Vec::new();
        }
        // The filter reaches TAPS/2 input samples back: start that much early.
        let lead = TAPS / 2 + 1;
        let first = ((start * rate) as u64).saturating_sub(lead as u64);
        let tick = (u128::from(first) * ts / u128::from(self.dec.rate.max(1))) as u64
            + self.track.edit.start.max(0) as u64;
        let samples = &self.track.samples;
        let k = samples.partition_point(|s| s.dts <= tick).saturating_sub(1);
        let from = k.saturating_sub(self.dec.preroll());
        self.dec.reset();
        let mut pcm = Vec::new();
        for j in from..k {
            self.dec.decode(j, &mut pcm);
        }
        // Frames of sample k before `first`.
        let mut skip = samples.get(k).map_or(0, |s| {
            (u128::from(tick.saturating_sub(s.dts)) * u128::from(self.dec.rate) / ts) as usize
        });
        // Input frames the output needs, mono.
        let need = ((want as f64) * self.step) as usize + TAPS + lead + 2;
        let mut mono: Vec<f32> = Vec::with_capacity(need);
        let mut j = k;
        while mono.len() < need && j < self.dec.len() {
            pcm.clear();
            self.dec.decode(j, &mut pcm);
            j += 1;
            for f in pcm.chunks_exact(ch) {
                if skip > 0 {
                    skip -= 1;
                    continue;
                }
                mono.push(f.iter().sum::<f32>() / ch as f32);
            }
        }
        // Output sample i is at input position (start·rate − first) + i·step.
        let base = start * rate - first as f64;
        resample(&mono, base, self.step, want, &self.filter)
    }
}

/// The resampling filter for `step` input samples per output sample: a Hann-windowed sinc
/// cutting at the output's band (or the input's, when upsampling), per phase.
fn filter(step: f64) -> Vec<f32> {
    let cut = CUTOFF / step.max(1.0);
    let half = (TAPS / 2) as f64;
    let mut out = vec![0f32; (PHASES + 1) * TAPS];
    for p in 0..=PHASES {
        let frac = p as f64 / PHASES as f64;
        let mut sum = 0.0;
        let row = &mut out[p * TAPS..(p + 1) * TAPS];
        let mut w = [0f64; TAPS];
        for (t, v) in w.iter_mut().enumerate() {
            // Tap t is input sample floor(pos) − TAPS/2 + 1 + t.
            let x = t as f64 - half + 1.0 - frac;
            let sinc = if x.abs() < 1e-9 {
                1.0
            } else {
                (std::f64::consts::PI * x * cut).sin() / (std::f64::consts::PI * x * cut)
            };
            let win = 0.5 + 0.5 * (std::f64::consts::PI * x / (half + 1.0)).cos();
            *v = sinc * win.max(0.0);
            sum += *v;
        }
        for (r, v) in row.iter_mut().zip(w) {
            *r = (v / sum) as f32;
        }
    }
    out
}

/// `n` samples of `src` at positions `base + i·step`, filtered.
fn resample(src: &[f32], base: f64, step: f64, n: usize, filter: &[f32]) -> Vec<f32> {
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let pos = base + i as f64 * step;
        let whole = pos.floor();
        let p = (((pos - whole) * PHASES as f64).round() as usize).min(PHASES);
        let row = &filter[p * TAPS..(p + 1) * TAPS];
        let at = whole as isize - (TAPS / 2) as isize + 1;
        let mut acc = 0.0f32;
        for (t, w) in row.iter().enumerate() {
            let j = at + t as isize;
            if j >= 0
                && let Some(v) = src.get(j as usize)
            {
                acc += v * w;
            }
        }
        out.push(acc);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: f64, hz: f64, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (std::f64::consts::TAU * hz * i as f64 / rate).sin() as f32)
            .collect()
    }

    fn rms(v: &[f32]) -> f32 {
        (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
    }

    #[test]
    fn keeps_speech_and_drops_what_would_alias() {
        let f = filter(3.0);
        // 1 kHz at 48 kHz passes; 12 kHz (would fold to 4 kHz) is gone.
        let pass = resample(&tone(48_000.0, 1000.0, 48_000), 0.0, 3.0, 16_000, &f);
        let stop = resample(&tone(48_000.0, 12_000.0, 48_000), 0.0, 3.0, 16_000, &f);
        let mid = |v: &[f32]| rms(&v[100..v.len() - 100]);
        assert!(
            (mid(&pass) - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.02,
            "{}",
            mid(&pass)
        );
        assert!(mid(&stop) < 0.01, "{}", mid(&stop));
        // The tone stays in place: the output follows sin(2π·1000·i/16000).
        let want = tone(16_000.0, 1000.0, 16_000);
        let err = pass[200..15_800]
            .iter()
            .zip(&want[200..15_800])
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f32::max);
        assert!(err < 0.03, "{err}");
    }

    /// A 16-bit PCM WAV of interleaved `ch`-channel samples.
    fn wav(rate: u32, ch: u16, samples: &[f32]) -> Vec<u8> {
        let data_len = samples.len() as u32 * 2;
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36 + data_len).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&ch.to_le_bytes());
        v.extend_from_slice(&rate.to_le_bytes());
        v.extend_from_slice(&(rate * u32::from(ch) * 2).to_le_bytes());
        v.extend_from_slice(&(ch * 2).to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            let q = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
            v.extend_from_slice(&q.to_le_bytes());
        }
        v
    }

    #[test]
    fn reads_any_stretch_as_16k_mono() {
        // 3 s at 44.1 kHz stereo: a 500 Hz tone left, its negative plus 0.4 right after 1 s
        // (so the mono mix is 0 for the first second, 0.2 after).
        let rate = 44_100;
        let n = 3 * rate as usize;
        let mut s = Vec::with_capacity(n * 2);
        for i in 0..n {
            let t = (std::f64::consts::TAU * 500.0 * i as f64 / f64::from(rate)).sin() as f32 * 0.5;
            s.push(t);
            s.push(if i >= rate as usize { -t + 0.4 } else { -t });
        }
        let src = Arc::new(Source::from_vec("speech.wav", wav(rate, 2, &s)));
        let mut sp = SpeechSound::open(src, None).unwrap();
        assert!((sp.duration() - 3.0).abs() < 1e-3, "{}", sp.duration());
        assert_eq!(sp.offset(), 0.0);
        let a = sp.read(0.25, 0.5);
        assert_eq!(a.len(), 8_000);
        assert!(a.iter().all(|v| v.abs() < 0.01));
        let b = sp.read(1.5, 1.0);
        assert_eq!(b.len(), 16_000);
        assert!(b.iter().all(|v| (v - 0.2).abs() < 0.01));
        // Past the end: what is left.
        assert_eq!(sp.read(2.75, 1.0).len(), 4_000);
        assert!(sp.read(3.5, 1.0).is_empty());
    }

    #[test]
    fn same_rate_is_nearly_a_copy() {
        let f = filter(1.0);
        let x = tone(16_000.0, 440.0, 4_000);
        let y = resample(&x, 0.0, 1.0, 4_000, &f);
        let err = x[100..3_900]
            .iter()
            .zip(&y[100..3_900])
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f32::max);
        assert!(err < 0.01, "{err}");
    }
}
