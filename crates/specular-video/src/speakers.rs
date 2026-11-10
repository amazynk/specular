//! Speakers: how a sound's channels reach a device's.
//!
//! **The sound's side.** Decoders hand out their channels in the WAVE order (the
//! `WAVEFORMATEXTENSIBLE` mask order), and two layouts play as surround:
//!
//! - 6 channels, 5.1: L R C LFE Ls Rs ([`WAVE_51`]);
//! - 8 channels, 7.1: L R C LFE Lb Rb Ls Rs ([`WAVE_71`]; back pair, then side pair).
//!
//! Any other count of three or more folds to stereo. Every count up to eight has its
//! speakers in [`wave_layout`] (quad and 5.0 have no LFE): the one table the fold to
//! stereo, the sound page's channel names and loudness weights (BS.1770, LFE left out)
//! read.
//!
//! **The device's side.** cpal passes samples through in the platform's own order; it sets
//! no speaker positions itself ([`DeviceOrder`]):
//!
//! | Platform | 5.1 (6 channels) | 7.1 (8 channels) |
//! |---|---|---|
//! | Linux (ALSA, and PipeWire and PulseAudio through ALSA) | FL FR RL RR FC LFE | FL FR RL RR FC LFE SL SR |
//! | Windows (WASAPI, the mix format's mask) | FL FR FC LFE BL BR | FL FR FC LFE BL BR SL SR |
//! | macOS (Core Audio, the device's channels in order) | L R C LFE Ls Rs | L R C LFE Ls Rs Rls Rrs |
//!
//! A 5.1 surround pair goes to the device's rear pair on Linux and to BL BR on Windows (both
//! are what those systems call 5.1's surrounds) and to the side pair on a 7.1 device. macOS
//! takes the device's channels as Audio MIDI Setup's speaker configuration numbers them.
//!
//! **The mapping** ([`Mapping`]): a surround sound on a surround device goes channel to
//! channel (bit for bit, unless dialogue boost lifts the centre); 7.1 on a 5.1 device folds
//! each back speaker into the surround on its side; everything else folds to stereo on the
//! device's first two channels (ITU-R BS.775: centre and surrounds at −3 dB, no LFE), the
//! other channels silent.

use std::f32::consts::FRAC_1_SQRT_2 as M3;

/// A speaker position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Speaker {
    /// Front left and right.
    L,
    R,
    /// Front centre.
    C,
    Lfe,
    /// Surround (side) left and right: 5.1's surrounds.
    Ls,
    Rs,
    /// Back (rear) left and right: 7.1's extra pair.
    Lb,
    Rb,
    /// Back centre: 6.1's.
    Cb,
}

use Speaker::{C, Cb, L, Lb, Lfe, Ls, R, Rb, Rs};

impl Speaker {
    /// The speaker's weight in loudness (ITU-R BS.1770-4): the LFE is left out, the
    /// surround and back speakers count 1.41 (+1.5 dB).
    pub fn loudness_weight(self) -> f64 {
        match self {
            L | R | C => 1.0,
            Lfe => 0.0,
            Ls | Rs | Lb | Rb | Cb => 1.41,
        }
    }
}

/// 5.1 as decoders hand it out.
pub const WAVE_51: [Speaker; 6] = [L, R, C, Lfe, Ls, Rs];
/// 7.1 as decoders hand it out.
pub const WAVE_71: [Speaker; 8] = [L, R, C, Lfe, Lb, Rb, Ls, Rs];

/// The speakers of `ch` channels in the order decoders hand them out (the WAVE order, as
/// `WAVEFORMATEXTENSIBLE`'s default masks and ffmpeg's default layouts have them): mono is
/// the centre; 3.0 L R C; quad L R Ls Rs; 5.0 L R C Ls Rs; 5.1; 6.1 L R C LFE Cb Ls Rs; 7.1.
/// `None` past eight.
pub fn wave_layout(ch: usize) -> Option<&'static [Speaker]> {
    const MONO: [Speaker; 1] = [C];
    const STEREO: [Speaker; 2] = [L, R];
    const THREE: [Speaker; 3] = [L, R, C];
    const QUAD: [Speaker; 4] = [L, R, Ls, Rs];
    const FIVE: [Speaker; 5] = [L, R, C, Ls, Rs];
    const SIX_ONE: [Speaker; 7] = [L, R, C, Lfe, Cb, Ls, Rs];
    match ch {
        1 => Some(&MONO),
        2 => Some(&STEREO),
        3 => Some(&THREE),
        4 => Some(&QUAD),
        5 => Some(&FIVE),
        6 => Some(&WAVE_51),
        7 => Some(&SIX_ONE),
        8 => Some(&WAVE_71),
        _ => None,
    }
}

/// The speakers of a sound of `ch` channels, when it plays as surround.
pub fn source_layout(ch: usize) -> Option<&'static [Speaker]> {
    match ch {
        6 => Some(&WAVE_51),
        8 => Some(&WAVE_71),
        _ => None,
    }
}

/// The order a device takes its channels in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceOrder {
    /// Windows: the `WAVEFORMATEXTENSIBLE` mask order.
    Wave,
    /// Linux: ALSA's order (PipeWire's and PulseAudio's ALSA plugins keep it).
    Alsa,
    /// macOS: the device's channels in order, as Audio MIDI Setup's defaults name them.
    CoreAudio,
}

impl DeviceOrder {
    /// The order of this build's sound system.
    pub const fn native() -> DeviceOrder {
        if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
            DeviceOrder::CoreAudio
        } else if cfg!(target_os = "windows") {
            DeviceOrder::Wave
        } else {
            DeviceOrder::Alsa
        }
    }

    /// The speakers of a device playing `channels` channels, when it is a surround layout.
    pub fn speakers(self, channels: usize) -> Option<&'static [Speaker]> {
        const ALSA_51: [Speaker; 6] = [L, R, Ls, Rs, C, Lfe];
        const ALSA_71: [Speaker; 8] = [L, R, Lb, Rb, C, Lfe, Ls, Rs];
        const CORE_71: [Speaker; 8] = [L, R, C, Lfe, Ls, Rs, Lb, Rb];
        match (self, channels) {
            (DeviceOrder::Alsa, 6) => Some(&ALSA_51),
            (DeviceOrder::Alsa, 8) => Some(&ALSA_71),
            (DeviceOrder::Wave | DeviceOrder::CoreAudio, 6) => Some(&WAVE_51),
            (DeviceOrder::Wave, 8) => Some(&WAVE_71),
            (DeviceOrder::CoreAudio, 8) => Some(&CORE_71),
            _ => None,
        }
    }
}

/// How many channels the listener's speakers take (the `sound_channels` setting).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChannelMode {
    /// What the device says it has.
    #[default]
    Auto,
    Stereo,
    Surround51,
    Surround71,
}

/// The most channels a device may report and still be speakers: a sound server's mixer
/// takes any count (PipeWire up to 64, cpal lists up to 32) and hides how many speakers
/// it has.
pub const MAX_SPEAKERS: u16 = 8;

/// The channel count to open a device at, for a sound of `source` channels.
/// `supported`: the counts the device takes; `default`: its default count.
pub fn pick_channels(mode: ChannelMode, source: u16, supported: &[u16], default: u16) -> u16 {
    let surround = source_layout(usize::from(source)).is_some();
    let takes = |n: u16| supported.contains(&n);
    let fallback = if takes(2) || supported.is_empty() {
        2
    } else {
        default.max(1)
    };
    if !surround {
        return fallback;
    }
    let max = supported.iter().copied().max().unwrap_or(default);
    let want = match mode {
        ChannelMode::Stereo => return fallback,
        ChannelMode::Surround51 => 6,
        ChannelMode::Surround71 => 8,
        // The device's own count, when it is speakers: a 7.1 sound on 5.1 speakers plays
        // as 5.1, a 5.1 sound on 7.1 speakers as 5.1 with the back pair silent.
        ChannelMode::Auto if max > MAX_SPEAKERS => return fallback,
        ChannelMode::Auto if max >= 8 && source == 8 => 8,
        ChannelMode::Auto if max >= 6 => 6,
        ChannelMode::Auto => return fallback,
    };
    if takes(want) {
        want
    } else if want == 8 && takes(6) {
        6
    } else {
        fallback
    }
}

/// How sound of one channel count goes to a device's.
#[derive(Clone, Debug, PartialEq)]
pub struct Mapping {
    src: usize,
    out: usize,
    /// The device's speakers, when the sound goes to them as surround.
    dev: Option<&'static [Speaker]>,
    /// The centre lift the matrix is made for.
    centre: f32,
    how: How,
}

#[derive(Clone, Debug, PartialEq)]
enum How {
    /// To stereo on the first two channels (mono to both).
    Fold,
    /// Channel to channel: for each device channel, (source channel, gain) pairs.
    Matrix(Vec<Vec<(usize, f32)>>),
}

impl Mapping {
    /// Sound of `src` channels to a device of `out` channels in `order`. `centre` lifts
    /// the centre (dialogue boost; 1 leaves it).
    pub fn new(src: usize, out: usize, order: DeviceOrder, centre: f32) -> Mapping {
        let src = src.max(1);
        let (dev, how) = match (source_layout(src), order.speakers(out)) {
            (Some(s), Some(d)) => (Some(d), How::Matrix(matrix(s, d, centre))),
            _ => (None, How::Fold),
        };
        Mapping {
            src,
            out,
            dev,
            centre,
            how,
        }
    }

    /// The sound reaches fewer speakers than it has (folded to stereo, or 7.1 to 5.1).
    pub fn downmixes(&self) -> bool {
        match &self.how {
            How::Fold => self.src > 2 || self.out == 1 && self.src == 2,
            How::Matrix(_) => self.src > self.out,
        }
    }

    /// Each source channel goes to a device channel (surround as is).
    pub fn discrete(&self) -> bool {
        matches!(self.how, How::Matrix(_))
    }

    /// Maps interleaved `src`-channel frames, appending device frames to `out`, with the
    /// centre lifted by `centre`.
    pub fn run(&mut self, src: &[f32], centre: f32, out: &mut Vec<f32>) {
        if centre != self.centre
            && let (Some(s), Some(d)) = (source_layout(self.src), self.dev)
        {
            self.how = How::Matrix(matrix(s, d, centre));
        }
        self.centre = centre;
        match &self.how {
            How::Fold => fold(src, self.src, self.out, centre, out),
            How::Matrix(m) => {
                out.reserve(src.len() / self.src * self.out);
                for f in src.chunks_exact(self.src) {
                    for row in m {
                        out.push(match row.as_slice() {
                            [] => 0.0,
                            &[(k, 1.0)] => f[k],
                            r => r.iter().map(|&(k, g)| f[k] * g).sum(),
                        });
                    }
                }
            }
        }
    }
}

/// The gains of each device speaker from the source's.
fn matrix(src: &[Speaker], dev: &[Speaker], centre: f32) -> Vec<Vec<(usize, f32)>> {
    // A lifted centre turns the rest down instead: nothing can pass full scale.
    let norm = 1.0 / centre.max(1.0);
    let mut m: Vec<Vec<(usize, f32)>> = vec![Vec::new(); dev.len()];
    let fold_backs = !dev.contains(&Lb);
    for (k, &s) in src.iter().enumerate() {
        let (to, g) = match s {
            // 7.1's back pair on a 5.1 device joins the surrounds (both at −3 dB).
            Lb if fold_backs => (Ls, M3),
            Rb if fold_backs => (Rs, M3),
            Ls if fold_backs && src.contains(&Lb) => (Ls, M3),
            Rs if fold_backs && src.contains(&Rb) => (Rs, M3),
            C => (C, centre * norm),
            s => (s, norm),
        };
        if let Some(i) = dev.iter().position(|&d| d == to) {
            m[i].push((k, g));
        }
    }
    m
}

/// Each channel's gains into the left and right of a fold to stereo, by its speaker
/// ([`wave_layout`]; past eight channels, the first two only): the centre and surrounds at
/// −3 dB (ITU-R BS.775, no LFE), 7.1's back and side pairs each at −3 dB into the
/// surrounds, 6.1's back centre at −3 dB into both; `centre` lifts the centre, the rest
/// turned down so the sum keeps its level.
fn fold_gains(ch: usize, centre: f32) -> [(f32, f32); 8] {
    let norm = if centre == 1.0 {
        1.0 / (1.0 + 2.0 * M3)
    } else {
        1.0 / (1.0 + M3 * centre + M3)
    };
    let mut g = [(0.0, 0.0); 8];
    let Some(layout) = wave_layout(ch) else {
        g[0] = (norm, 0.0);
        g[1] = (0.0, norm);
        return g;
    };
    let backs = layout.contains(&Lb);
    for (k, s) in layout.iter().enumerate() {
        // A surround sharing its side with a back speaker goes in at −3 dB more.
        let pair = if backs { M3 * M3 } else { M3 };
        g[k] = match s {
            L => (norm, 0.0),
            R => (0.0, norm),
            C => (M3 * centre * norm, M3 * centre * norm),
            Lfe => (0.0, 0.0),
            Ls | Lb => (pair * norm, 0.0),
            Rs | Rb => (0.0, pair * norm),
            Cb => (M3 * M3 * norm, M3 * M3 * norm),
        };
    }
    g
}

/// Folds `ch` interleaved channels to `out_ch` device channels: mono goes to the first
/// two; stereo stays; more fold by their speakers ([`fold_gains`]).
fn fold(src: &[f32], ch: usize, out_ch: usize, centre: f32, out: &mut Vec<f32>) {
    let ch = ch.max(1);
    let gains = fold_gains(ch, centre);
    for f in src.chunks_exact(ch) {
        let (l, r) = match ch {
            1 => (f[0], f[0]),
            2 => (f[0], f[1]),
            _ => f
                .iter()
                .zip(&gains)
                .fold((0.0, 0.0), |(l, r), (&x, &(gl, gr))| {
                    (l + x * gl, r + x * gr)
                }),
        };
        match out_ch {
            0 => {}
            1 => out.push((l + r) * 0.5),
            n => {
                out.push(l);
                out.push(r);
                out.extend(std::iter::repeat_n(0.0, n - 2));
            }
        }
    }
}

/// Folds a device's interleaved frames (`ch` channels in `order`) to stereo, appended to
/// `out` (for the live displays): 5.1 and 7.1 as [`fold`] does from the WAVE order; other
/// counts give their first two channels (front left and right on every platform).
pub fn device_to_stereo(buf: &[f32], ch: usize, order: DeviceOrder, out: &mut Vec<f32>) {
    let ch = ch.max(1);
    out.reserve(buf.len() / ch * 2);
    match order.speakers(ch) {
        Some(dev) => {
            let wave: &[Speaker] = if ch == 8 { &WAVE_71 } else { &WAVE_51 };
            // For each WAVE slot, the device's channel holding that speaker.
            let mut at = [0usize; 8];
            for (k, s) in wave.iter().enumerate() {
                at[k] = dev.iter().position(|d| d == s).unwrap_or(0);
            }
            let mut w = [0.0f32; 8];
            for f in buf.chunks_exact(ch) {
                for k in 0..ch {
                    w[k] = f[at[k]];
                }
                fold(&w[..ch], ch, 2, 1.0, out);
            }
        }
        None if ch == 1 => {
            for &v in buf {
                out.extend_from_slice(&[v, v]);
            }
        }
        None => {
            for f in buf.chunks_exact(ch) {
                out.extend_from_slice(&f[..2]);
            }
        }
    }
}

/// Folds interleaved `ch`-channel sound to stereo (mono stays mono), as playback does.
pub(crate) fn fold_stereo(src: &[f32], ch: usize, out: &mut Vec<f32>) {
    if ch <= 2 {
        out.extend_from_slice(src);
    } else {
        fold(src, ch, 2, 1.0, out);
    }
}

/// Puts a WAVE-ordered frame of a coding mode (front channels, an optional centre and LFE,
/// zero, one or two surrounds) into 5.1 ([`WAVE_51`]); a single surround goes to both at
/// −3 dB. `front`: 1 (mono: the centre) or 2.
pub(crate) fn wave_to_51(
    src: &[f32],
    front: usize,
    centre: bool,
    lfe: bool,
    surrounds: usize,
    out: &mut Vec<f32>,
) {
    let n = front + usize::from(centre) + usize::from(lfe) + surrounds;
    if n == 0 {
        return;
    }
    for f in src.chunks_exact(n) {
        let mut k = 0;
        let mut next = || {
            k += 1;
            f[k - 1]
        };
        let (l, r, mut c) = if front == 1 {
            (0.0, 0.0, next())
        } else {
            (next(), next(), 0.0)
        };
        if centre {
            c = next();
        }
        let e = if lfe { next() } else { 0.0 };
        let (ls, rs) = match surrounds {
            1 => {
                let s = next() * M3;
                (s, s)
            }
            2 => (next(), next()),
            _ => (0.0, 0.0),
        };
        out.extend_from_slice(&[l, r, c, e, ls, rs]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(m: &Mapping, src: &[f32]) -> Vec<f32> {
        let mut out = Vec::new();
        m.clone().run(src, 1.0, &mut out);
        out
    }

    /// Each speaker of a 5.1 or 7.1 sound lands on that speaker of the device, in each
    /// platform's order.
    #[test]
    fn surround_lands_on_its_speakers() {
        let s51: Vec<f32> = (1..=6).map(|i| i as f32).collect();
        let s71: Vec<f32> = (1..=8).map(|i| i as f32).collect();
        // WAVE: 1 L, 2 R, 3 C, 4 LFE, 5 Ls, 6 Rs (5.1); 5 Lb, 6 Rb, 7 Ls, 8 Rs (7.1).
        let alsa = Mapping::new(6, 6, DeviceOrder::Alsa, 1.0);
        assert!(alsa.discrete() && !alsa.downmixes());
        assert_eq!(run(&alsa, &s51), [1.0, 2.0, 5.0, 6.0, 3.0, 4.0]);
        assert_eq!(
            run(&Mapping::new(8, 8, DeviceOrder::Alsa, 1.0), &s71),
            [1.0, 2.0, 5.0, 6.0, 3.0, 4.0, 7.0, 8.0]
        );
        assert_eq!(run(&Mapping::new(6, 6, DeviceOrder::Wave, 1.0), &s51), s51);
        assert_eq!(run(&Mapping::new(8, 8, DeviceOrder::Wave, 1.0), &s71), s71);
        assert_eq!(
            run(&Mapping::new(6, 6, DeviceOrder::CoreAudio, 1.0), &s51),
            s51
        );
        assert_eq!(
            run(&Mapping::new(8, 8, DeviceOrder::CoreAudio, 1.0), &s71),
            [1.0, 2.0, 3.0, 4.0, 7.0, 8.0, 5.0, 6.0]
        );
        // 5.1 on 7.1 speakers: the surrounds on the sides, the back pair silent.
        assert_eq!(
            run(&Mapping::new(6, 8, DeviceOrder::Wave, 1.0), &s51),
            [1.0, 2.0, 3.0, 4.0, 0.0, 0.0, 5.0, 6.0]
        );
        assert_eq!(
            run(&Mapping::new(6, 8, DeviceOrder::Alsa, 1.0), &s51),
            [1.0, 2.0, 0.0, 0.0, 3.0, 4.0, 5.0, 6.0]
        );
    }

    /// Each platform's 5.1 and 7.1 orders hold every speaker once.
    #[test]
    fn device_orders_are_complete() {
        for o in [DeviceOrder::Alsa, DeviceOrder::Wave, DeviceOrder::CoreAudio] {
            for (n, all) in [(6, &WAVE_51[..]), (8, &WAVE_71[..])] {
                let d = o.speakers(n).unwrap();
                assert_eq!(d.len(), n);
                for s in all {
                    assert_eq!(d.iter().filter(|x| *x == s).count(), 1, "{o:?} {n} {s:?}");
                }
            }
            assert!(o.speakers(2).is_none() && o.speakers(4).is_none());
        }
    }

    /// 7.1 on 5.1 speakers: each back speaker joins the surround on its side.
    #[test]
    fn seven_one_folds_to_five_one() {
        let m = Mapping::new(8, 6, DeviceOrder::Wave, 1.0);
        assert!(m.discrete() && m.downmixes());
        let out = run(&m, &[0.1, 0.2, 0.3, 0.4, 1.0, 0.0, 0.0, 1.0]);
        let want = [0.1, 0.2, 0.3, 0.4, M3, M3];
        for (a, b) in out.iter().zip(want) {
            assert!((a - b).abs() < 1e-6, "{out:?}");
        }
    }

    /// Stereo stays stereo; a surround sound on a stereo device (or one with a count that
    /// is no surround layout) folds to the first two channels; mono goes to both.
    #[test]
    fn folds_to_stereo() {
        let m = Mapping::new(2, 2, DeviceOrder::Alsa, 1.0);
        assert!(!m.discrete() && !m.downmixes());
        assert_eq!(run(&m, &[0.1, 0.2]), [0.1, 0.2]);
        assert_eq!(
            run(&Mapping::new(1, 2, DeviceOrder::Alsa, 1.0), &[0.5]),
            [0.5, 0.5]
        );
        assert_eq!(
            run(&Mapping::new(2, 1, DeviceOrder::Alsa, 1.0), &[0.2, 0.4]),
            [0.3f32]
        );
        // Stereo on a 5.1 device: the fronts, the rest silent.
        assert_eq!(
            run(&Mapping::new(2, 6, DeviceOrder::Alsa, 1.0), &[0.1, 0.2]),
            [0.1, 0.2, 0.0, 0.0, 0.0, 0.0]
        );
        let m = Mapping::new(6, 2, DeviceOrder::Alsa, 1.0);
        assert!(m.downmixes() && !m.discrete());
        // The centre alone: both sides at −3 dB, normalised.
        let out = run(&m, &[0.0, 0.0, 1.0, 0.0, 0.0, 0.0]);
        let c = M3 / (1.0 + 2.0 * M3);
        assert!((out[0] - c).abs() < 1e-6 && (out[1] - c).abs() < 1e-6);
        // Everything at full scale stays at full scale (the LFE left out).
        let out = run(&m, &[1.0; 6]);
        assert!((out[0] - 1.0).abs() < 1e-6 && (out[1] - 1.0).abs() < 1e-6);
        // 7.1: side and back pairs both reach the fold.
        let out = run(
            &Mapping::new(8, 2, DeviceOrder::Wave, 1.0),
            &[0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0],
        );
        assert!(out[0] > 0.4 && out[1] == 0.0, "{out:?}");
        // Four channels are no surround layout: folded.
        assert!(!Mapping::new(4, 6, DeviceOrder::Wave, 1.0).discrete());
    }

    /// Quad and 5.0 have no LFE: the fold hears every channel of theirs, and the loudness
    /// weights (the sound page) read the same table, so the two agree on which channel is
    /// the LFE for every count.
    #[test]
    fn one_layout_for_the_fold_and_the_loudness() {
        // Quad L R Ls Rs: the right surround reaches the right side.
        let out = run(
            &Mapping::new(4, 2, DeviceOrder::Wave, 1.0),
            &[0.0, 0.0, 0.0, 1.0],
        );
        assert!(out[0] == 0.0 && out[1] > 0.2, "{out:?}");
        // 5.0 L R C Ls Rs: the fourth channel is the left surround, not an LFE.
        let out = run(
            &Mapping::new(5, 2, DeviceOrder::Wave, 1.0),
            &[0.0, 0.0, 0.0, 1.0, 0.0],
        );
        assert!(out[0] > 0.2 && out[1] == 0.0, "{out:?}");
        // 6.1's back centre goes to both sides.
        let out = run(
            &Mapping::new(7, 2, DeviceOrder::Wave, 1.0),
            &[0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        );
        assert!(out[0] > 0.1 && (out[0] - out[1]).abs() < 1e-7, "{out:?}");
        for ch in 1..=8 {
            for k in 0..ch {
                let mut f = vec![0.0f32; ch];
                f[k] = 1.0;
                let out = run(&Mapping::new(ch, 2, DeviceOrder::Wave, 1.0), &f);
                let heard = out[0] != 0.0 || out[1] != 0.0;
                let weighed = crate::loudness::channel_weight(k, ch) > 0.0;
                assert_eq!(heard, weighed, "channel {k} of {ch}");
            }
        }
    }

    /// Dialogue boost lifts the centre against the rest, with nothing louder than before.
    #[test]
    fn centre_lift_keeps_the_ceiling() {
        let mut m = Mapping::new(6, 6, DeviceOrder::Wave, 1.0);
        let mut out = Vec::new();
        m.run(&[1.0; 6], 2.0, &mut out);
        assert_eq!(out, [0.5, 0.5, 1.0, 0.5, 0.5, 0.5]);
        out.clear();
        m.run(&[1.0; 6], 1.0, &mut out);
        assert_eq!(out, [1.0; 6]);
    }

    /// The channel count a device opens at, by the setting and what the device takes.
    #[test]
    fn channel_count_by_setting() {
        use ChannelMode::*;
        let speakers51 = [6];
        let speakers71 = [2, 6, 8];
        let stereo = [1, 2];
        let mixer: Vec<u16> = (1..=32).collect();
        // Stereo sound: stereo whatever the setting (the device's own count when it takes
        // no stereo).
        assert_eq!(pick_channels(Surround71, 2, &speakers71, 8), 2);
        assert_eq!(pick_channels(Auto, 2, &speakers51, 6), 6);
        // Automatic: the speakers there are.
        assert_eq!(pick_channels(Auto, 6, &speakers51, 6), 6);
        assert_eq!(pick_channels(Auto, 8, &speakers51, 6), 6);
        assert_eq!(pick_channels(Auto, 8, &speakers71, 8), 8);
        assert_eq!(pick_channels(Auto, 6, &speakers71, 8), 6);
        assert_eq!(pick_channels(Auto, 6, &stereo, 2), 2);
        // A sound server's mixer hides its speakers: stereo unless asked.
        assert_eq!(pick_channels(Auto, 6, &mixer, 2), 2);
        assert_eq!(pick_channels(Surround51, 8, &mixer, 2), 6);
        assert_eq!(pick_channels(Surround71, 8, &mixer, 2), 8);
        assert_eq!(pick_channels(Surround71, 6, &mixer, 2), 8);
        // Asked for more than the device takes: the most it has, else stereo.
        assert_eq!(pick_channels(Surround71, 8, &[2, 6], 2), 6);
        assert_eq!(pick_channels(Surround51, 6, &stereo, 2), 2);
        assert_eq!(pick_channels(Stereo, 8, &speakers71, 8), 2);
        // Nothing known about the device: stereo.
        assert_eq!(pick_channels(Auto, 6, &[], 0), 2);
    }

    /// The live displays' stereo view of a surround device: the same fold as from the
    /// WAVE order, whatever the platform's order.
    #[test]
    fn device_output_folds_for_the_displays() {
        let wave: Vec<f32> = [0.9, 0.1, 0.5, 0.7, 0.3, 0.2].repeat(3);
        let mut want = Vec::new();
        fold(&wave, 6, 2, 1.0, &mut want);
        for order in [DeviceOrder::Alsa, DeviceOrder::Wave, DeviceOrder::CoreAudio] {
            for n in [6, 8] {
                let src: Vec<f32> = if n == 6 {
                    wave.clone()
                } else {
                    // 7.1 with the back pair silent folds like the 5.1.
                    wave.chunks(6)
                        .flat_map(|f| [f[0], f[1], f[2], f[3], 0.0, 0.0, f[4], f[5]])
                        .collect()
                };
                let mut dev = Vec::new();
                Mapping::new(n, n, order, 1.0).run(&src, 1.0, &mut dev);
                let mut got = Vec::new();
                device_to_stereo(&dev, n, order, &mut got);
                let mut ref_ = Vec::new();
                fold(&src, n, 2, 1.0, &mut ref_);
                assert_eq!(got, ref_, "{order:?} {n}");
                if n == 6 {
                    assert_eq!(got, want, "{order:?}");
                }
            }
        }
        // Stereo on a four-channel device: its first two.
        let mut got = Vec::new();
        device_to_stereo(&[0.1, 0.2, 0.0, 0.0], 4, DeviceOrder::Alsa, &mut got);
        assert_eq!(got, [0.1, 0.2]);
    }

    /// AC-3 and DTS coding modes in the WAVE order become 5.1.
    #[test]
    fn coding_modes_to_five_one() {
        let mut out = Vec::new();
        // 3/2 with LFE: as is.
        wave_to_51(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 2, true, true, 2, &mut out);
        assert_eq!(out, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        // 2/1: one surround to both at −3 dB.
        out.clear();
        wave_to_51(&[1.0, 2.0, 1.0], 2, false, false, 1, &mut out);
        assert_eq!(out, [1.0, 2.0, 0.0, 0.0, M3, M3]);
        // 1/0 with LFE: the centre.
        out.clear();
        wave_to_51(&[0.5, 0.25], 1, false, true, 0, &mut out);
        assert_eq!(out, [0.0, 0.0, 0.5, 0.25, 0.0, 0.0]);
    }
}
