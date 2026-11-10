//! Volume levelling: one loudness for every file, from its tags or measured while it plays.
//!
//! - **ReplayGain 2** (`REPLAYGAIN_TRACK_GAIN`, `_ALBUM_GAIN`, `_TRACK_PEAK`, `_ALBUM_PEAK` in
//!   Vorbis comments, APEv2, ID3 `TXXX` frames, MP4 `----` items and Matroska tags): gains to
//!   a −18 LUFS reference, peaks as a share of full scale.
//! - **Opus R128** (`R128_TRACK_GAIN`, `R128_ALBUM_GAIN`): Q7.8 dB to −23 LUFS, on top of the
//!   header's output gain (which the decoder always applies); 5 dB are added to meet the
//!   ReplayGain reference.
//! - **iTunes Sound Check** (`iTunNORM`): ten hex words; the first two are the channels'
//!   loudness in thousandths (gain = −10·log10(max / 1000) dB), the 7th and 8th their peaks
//!   (of 32 768).
//!
//! Untagged files can be measured while they play ([`Meter`], ITU-R BS.1770-4: K-weighting,
//! 400 ms blocks overlapping by 75 %, the −70 LUFS absolute and −10 LU relative gates); the
//! gain follows the measurement slowly. Off by default.
//!
//! Written from the ReplayGain 2.0 specification (Hydrogenaudio wiki), RFC 7845 §5.2.1
//! (Opus), the tag format's public descriptions, and ITU-R BS.1770-4.

/// The loudness every file is brought to (ReplayGain 2's reference).
pub const TARGET_LUFS: f64 = -18.0;
/// The most a file is turned up or down by levelling.
const MAX_GAIN_DB: f64 = 24.0;

/// A file's levelling tags (dB and shares of full scale), as found.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GainTags {
    pub track_gain: Option<f32>,
    pub album_gain: Option<f32>,
    pub track_peak: Option<f32>,
    pub album_peak: Option<f32>,
}

impl GainTags {
    pub fn is_empty(&self) -> bool {
        self.track_gain.is_none() && self.album_gain.is_none()
    }

    /// Takes in one tag field (`key` in any case): the ReplayGain, R128 and iTunNORM fields;
    /// others are ignored. The first value of each field wins.
    pub fn field(&mut self, key: &str, value: &str) {
        let k = key.trim().to_ascii_uppercase();
        let v = value.trim().trim_end_matches('\0');
        match k.as_str() {
            "REPLAYGAIN_TRACK_GAIN" => set(&mut self.track_gain, db(v)),
            "REPLAYGAIN_ALBUM_GAIN" => set(&mut self.album_gain, db(v)),
            "REPLAYGAIN_TRACK_PEAK" => set(&mut self.track_peak, peak(v)),
            "REPLAYGAIN_ALBUM_PEAK" => set(&mut self.album_peak, peak(v)),
            "R128_TRACK_GAIN" => set(&mut self.track_gain, r128(v)),
            "R128_ALBUM_GAIN" => set(&mut self.album_gain, r128(v)),
            "ITUNNORM" => {
                if let Some((g, p)) = itunnorm(v) {
                    set(&mut self.track_gain, Some(g));
                    set(&mut self.track_peak, p);
                }
            }
            _ => {}
        }
    }
}

fn set(slot: &mut Option<f32>, v: Option<f32>) {
    if slot.is_none() {
        *slot = v;
    }
}

/// "−6.48 dB", "+1.2 dB", "-3.5".
fn db(v: &str) -> Option<f32> {
    let n = v
        .trim()
        .trim_end_matches(|c: char| c.is_ascii_alphabetic() || c.is_whitespace())
        .replace('\u{2212}', "-");
    n.trim()
        .parse::<f32>()
        .ok()
        .filter(|g| g.is_finite() && f64::from(g.abs()) <= 64.0)
}

fn peak(v: &str) -> Option<f32> {
    v.trim()
        .parse::<f32>()
        .ok()
        .filter(|p| p.is_finite() && *p > 0.0 && *p < 100.0)
}

/// An R128 gain: a signed Q7.8 integer, to −23 LUFS; as a ReplayGain gain (to −18 LUFS).
fn r128(v: &str) -> Option<f32> {
    let q = v.trim().parse::<i32>().ok()?;
    let q = i16::try_from(q).ok()?;
    Some(f32::from(q) / 256.0 + 5.0)
}

/// iTunNORM: the gain from the louder channel's figure, and the louder peak.
fn itunnorm(v: &str) -> Option<(f32, Option<f32>)> {
    let w: Vec<u32> = v
        .split_whitespace()
        .filter_map(|h| u32::from_str_radix(h, 16).ok())
        .collect();
    if w.len() < 2 {
        return None;
    }
    let loud = w[0].max(w[1]);
    if loud == 0 {
        return None;
    }
    let g = -10.0 * (f64::from(loud) / 1000.0).log10();
    let p = (w.len() >= 8)
        .then(|| w[6].max(w[7]))
        .filter(|&p| p > 0)
        .map(|p| (f64::from(p) / 32768.0) as f32);
    Some((g.clamp(-MAX_GAIN_DB, MAX_GAIN_DB) as f32, p))
}

/// Which gain levelling uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Off,
    /// Each file to the reference.
    Track,
    /// An album's files keep their loudness against each other.
    Album,
}

/// Levelling as set (the app turns "auto" into track or album).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Levelling {
    pub mode: Mode,
    /// Added to every levelled file (dB).
    pub preamp_db: f32,
    /// Never turn a file up past its tagged peak.
    pub prevent_clip: bool,
    /// Measure untagged files while they play.
    pub measure: bool,
}

impl Default for Levelling {
    fn default() -> Levelling {
        Levelling {
            mode: Mode::Off,
            preamp_db: 0.0,
            prevent_clip: true,
            measure: false,
        }
    }
}

/// Where a gain came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Track,
    Album,
    Measured,
}

/// The gain levelling applies to a file (dB) and where it came from; `None` when off or
/// nothing is known. `measured` is the loudness measured so far (LUFS), for untagged files.
pub fn gain_db(tags: &GainTags, l: &Levelling, measured: Option<f64>) -> Option<(f64, Source)> {
    let (g, p, src) = match l.mode {
        Mode::Off => return None,
        Mode::Track => match (tags.track_gain, tags.album_gain) {
            (Some(g), _) => (g, tags.track_peak, Source::Track),
            (None, Some(g)) => (g, tags.album_peak, Source::Album),
            _ => return measure(measured, l),
        },
        Mode::Album => match (tags.album_gain, tags.track_gain) {
            (Some(g), _) => (g, tags.album_peak.or(tags.track_peak), Source::Album),
            (None, Some(g)) => (g, tags.track_peak, Source::Track),
            _ => return measure(measured, l),
        },
    };
    let mut db = f64::from(g) + f64::from(l.preamp_db);
    if l.prevent_clip
        && let Some(p) = p.filter(|&p| p > 0.0)
    {
        // The peak, turned up by `db`, stays at full scale.
        db = db.min(-20.0 * f64::from(p).log10());
    }
    Some((db.clamp(-MAX_GAIN_DB, MAX_GAIN_DB), src))
}

fn measure(measured: Option<f64>, l: &Levelling) -> Option<(f64, Source)> {
    let m = measured.filter(|_| l.measure)?;
    let db = (TARGET_LUFS - m + f64::from(l.preamp_db)).clamp(-MAX_GAIN_DB, 12.0);
    Some((db, Source::Measured))
}

/// The linear gain of `db`.
pub fn linear(db: f64) -> f32 {
    10f64.powf(db / 20.0) as f32
}

/// Integrated loudness after ITU-R BS.1770-4, fed the source's interleaved frames as they
/// play (before any fold to the device's speakers): each channel weighed by its speaker
/// ([`crate::loudness::channel_weight`]: the LFE left out, surrounds +1.5 dB).
#[derive(Clone, Debug)]
pub struct Meter {
    ch: usize,
    weights: Vec<f64>,
    shelf: crate::eq::Section,
    high: crate::eq::Section,
    /// The current 100 ms step's sum of K-weighted squares, and its frames.
    step_sum: f64,
    step_n: usize,
    step_len: usize,
    /// The last four steps' sums (a 400 ms block is four steps).
    steps: [f64; 4],
    steps_seen: usize,
    /// Every gating block's mean square (above the absolute gate).
    blocks: Vec<f64>,
}

impl Meter {
    pub fn new(ch: usize, rate: u32) -> Meter {
        let (shelf, high) = k_weighting(f64::from(rate.max(1)));
        Meter {
            ch: ch.max(1),
            weights: (0..ch.max(1))
                .map(|i| crate::loudness::channel_weight(i, ch.max(1)))
                .collect(),
            shelf: crate::eq::Section::new(shelf, ch.max(1)),
            high: crate::eq::Section::new(high, ch.max(1)),
            step_sum: 0.0,
            step_n: 0,
            step_len: (rate as usize / 10).max(1),
            steps: [0.0; 4],
            steps_seen: 0,
            blocks: Vec::new(),
        }
    }

    pub fn feed(&mut self, buf: &[f32]) {
        for f in buf.chunks_exact(self.ch) {
            for (c, &s) in f.iter().enumerate() {
                let y = self.high.tick(c, self.shelf.tick(c, f64::from(s)));
                self.step_sum += self.weights[c] * y * y;
            }
            self.step_n += 1;
            if self.step_n == self.step_len {
                self.steps.rotate_left(1);
                self.steps[3] = self.step_sum;
                self.steps_seen += 1;
                self.step_sum = 0.0;
                self.step_n = 0;
                if self.steps_seen >= 4 {
                    let z = self.steps.iter().sum::<f64>() / (4 * self.step_len) as f64;
                    // The absolute gate: −70 LUFS.
                    if loudness(z) > -70.0 && self.blocks.len() < 1 << 20 {
                        self.blocks.push(z);
                    }
                }
            }
        }
    }

    /// Seconds measured (of blocks over the absolute gate).
    pub fn seconds(&self) -> f64 {
        self.blocks.len() as f64 * 0.1
    }

    /// The integrated loudness so far (LUFS), once there is a block.
    pub fn integrated(&self) -> Option<f64> {
        if self.blocks.is_empty() {
            return None;
        }
        let mean = |it: &mut dyn Iterator<Item = f64>| {
            let (s, n) = it.fold((0.0, 0usize), |(s, n), z| (s + z, n + 1));
            (n > 0).then(|| s / n as f64)
        };
        let all = mean(&mut self.blocks.iter().copied())?;
        let gate = loudness(all) - 10.0;
        let gated = mean(&mut self.blocks.iter().copied().filter(|&z| loudness(z) > gate))?;
        Some(loudness(gated))
    }
}

/// Loudness of a K-weighted mean square summed over channels.
fn loudness(z: f64) -> f64 {
    -0.691 + 10.0 * z.max(1e-20).log10()
}

/// BS.1770's two K-weighting stages for `rate`: the head's high shelf (+4 dB above
/// ~1.7 kHz) and the RLB high-pass (~38 Hz), designed here from their analogue
/// prototypes by the bilinear transform (the recommendation tabulates 48 kHz only).
fn k_weighting(rate: f64) -> (crate::eq::Biquad, crate::eq::Biquad) {
    use crate::eq::Biquad;
    // Stage 1: a high shelf, f0 1681.97 Hz, Q 0.7072, +3.99984 dB.
    let (f0, g, q) = (
        1_681.974_450_955_533,
        3.999_843_853_973_347,
        0.707_175_236_955_419_6,
    );
    let k = (std::f64::consts::PI * f0 / rate).tan();
    let vh = 10f64.powf(g / 20.0);
    let vb = vh.powf(0.499_666_774_154_541_6);
    let a0 = 1.0 + k / q + k * k;
    let shelf = Biquad {
        b: [
            (vh + vb * k / q + k * k) / a0,
            2.0 * (k * k - vh) / a0,
            (vh - vb * k / q + k * k) / a0,
        ],
        a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
    };
    // Stage 2: a high-pass, f0 38.1355 Hz, Q 0.5003.
    let (f0, q) = (38.135_470_876_024_44, 0.500_327_037_323_877_3);
    let k = (std::f64::consts::PI * f0 / rate).tan();
    let a0 = 1.0 + k / q + k * k;
    let high = Biquad {
        b: [1.0, -2.0, 1.0],
        a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
    };
    (shelf, high)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_read() {
        let mut t = GainTags::default();
        t.field("replaygain_track_gain", "-6.48 dB");
        t.field("REPLAYGAIN_TRACK_PEAK", "0.988525");
        t.field("REPLAYGAIN_ALBUM_GAIN", "\u{2212}7.10 dB");
        t.field("REPLAYGAIN_TRACK_GAIN", "+3 dB"); // the first value wins
        assert_eq!(t.track_gain, Some(-6.48));
        assert_eq!(t.album_gain, Some(-7.1));
        assert_eq!(t.track_peak, Some(0.988_525));
        // R128: −1536 / 256 = −6 dB to −23 LUFS, so −1 dB to −18 LUFS.
        let mut o = GainTags::default();
        o.field("R128_TRACK_GAIN", "-1536");
        o.field("R128_ALBUM_GAIN", "256");
        assert_eq!(o.track_gain, Some(-1.0));
        assert_eq!(o.album_gain, Some(6.0));
        // iTunNORM: 0x1F4 = 500 → +3.01 dB; peaks 0x4000 = half scale.
        let mut s = GainTags::default();
        s.field(
            "iTunNORM",
            " 000001F4 00000100 00000000 00000000 00000000 00000000 00004000 00002000 00000000 00000000",
        );
        assert!((s.track_gain.unwrap() - 3.0103).abs() < 1e-3);
        assert_eq!(s.track_peak, Some(0.5));
        t.field("COMMENT", "x");
        assert!(!t.is_empty());
        assert!(GainTags::default().is_empty());
    }

    #[test]
    fn gain_math() {
        let tags = GainTags {
            track_gain: Some(-6.0),
            album_gain: Some(-8.0),
            track_peak: Some(0.5),
            album_peak: Some(0.9),
        };
        let l = |mode, preamp, prevent_clip| Levelling {
            mode,
            preamp_db: preamp,
            prevent_clip,
            measure: false,
        };
        assert_eq!(gain_db(&tags, &l(Mode::Off, 0.0, true), None), None);
        assert_eq!(
            gain_db(&tags, &l(Mode::Track, 0.0, true), None),
            Some((-6.0, Source::Track))
        );
        assert_eq!(
            gain_db(&tags, &l(Mode::Album, 0.0, true), None),
            Some((-8.0, Source::Album))
        );
        // +10 dB preamp: track −6 + 10 = +4, but a 0.5 peak allows only +6.02.
        let (g, _) = gain_db(&tags, &l(Mode::Track, 10.0, true), None).unwrap();
        assert!((g - 4.0).abs() < 1e-9);
        let (g, _) = gain_db(&tags, &l(Mode::Track, 14.0, true), None).unwrap();
        assert!((g - 6.0206).abs() < 1e-3, "{g}");
        let (g, _) = gain_db(&tags, &l(Mode::Track, 12.0, false), None).unwrap();
        assert!((g - 6.0).abs() < 1e-9);
        // Album with a 0.9 peak: −8 + 12 = 4 → capped at 0.915 dB.
        let (g, _) = gain_db(&tags, &l(Mode::Album, 12.0, true), None).unwrap();
        assert!((g - 0.9151).abs() < 1e-3, "{g}");
        // No album gain: album mode uses the track's.
        let only = GainTags {
            track_gain: Some(2.5),
            ..GainTags::default()
        };
        assert_eq!(
            gain_db(&only, &l(Mode::Album, 0.0, true), None),
            Some((2.5, Source::Track))
        );
        // Untagged: measured only when asked.
        let none = GainTags::default();
        assert_eq!(
            gain_db(&none, &l(Mode::Track, 0.0, true), Some(-23.0)),
            None
        );
        let mut m = l(Mode::Track, 0.0, true);
        m.measure = true;
        assert_eq!(
            gain_db(&none, &m, Some(-23.0)),
            Some((5.0, Source::Measured))
        );
        assert!((linear(-6.0206) - 0.5).abs() < 1e-4);
    }

    /// BS.1770's calibration: a 0 dBFS 1 kHz sine on one channel reads −3.01 LUFS; at
    /// −20 dBFS on both channels, −20 LUFS.
    #[test]
    fn meter_reads_a_sine() {
        for rate in [44_100u32, 48_000] {
            let mut m = Meter::new(2, rate);
            let amp = 10f64.powf(-20.0 / 20.0);
            let buf: Vec<f32> = (0..rate as usize * 5)
                .flat_map(|i| {
                    let v = (amp
                        * (std::f64::consts::TAU * 997.0 * i as f64 / f64::from(rate)).sin())
                        as f32;
                    [v, v]
                })
                .collect();
            m.feed(&buf);
            let l = m.integrated().unwrap();
            assert!((l - (-20.0)).abs() < 0.5, "{rate}: {l}");
            assert!(m.seconds() > 4.0);
        }
        // Silence is below the absolute gate.
        let mut m = Meter::new(2, 48_000);
        m.feed(&vec![0.0; 96_000 * 2]);
        assert_eq!(m.integrated(), None);
    }

    /// 5.1 is measured by speaker (BS.1770): the LFE alone is not heard, a surround counts
    /// +1.5 dB over a front channel.
    #[test]
    fn surround_is_measured_by_speaker() {
        let rate = 48_000u32;
        let one = |k: usize| {
            let amp = 10f64.powf(-20.0 / 20.0);
            let mut m = Meter::new(6, rate);
            let buf: Vec<f32> = (0..rate as usize * 3)
                .flat_map(|i| {
                    let v = (amp
                        * (std::f64::consts::TAU * 997.0 * i as f64 / f64::from(rate)).sin())
                        as f32;
                    let mut f = [0.0f32; 6];
                    f[k] = v;
                    f
                })
                .collect();
            m.feed(&buf);
            m.integrated()
        };
        // L R C LFE Ls Rs.
        assert_eq!(one(3), None, "the LFE alone");
        let front = one(0).unwrap();
        let surround = one(4).unwrap();
        assert!((front - one(2).unwrap()).abs() < 1e-9);
        assert!(
            (surround - front - 10.0 * 1.41f64.log10()).abs() < 1e-6,
            "{front} {surround}"
        );
    }
}
