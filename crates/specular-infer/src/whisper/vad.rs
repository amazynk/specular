//! Which stretches of 16 kHz mono sound may hold speech, so silence is never read (Whisper
//! invents words in it, and it costs a window each 30 s), and the windows that cover them.
//!
//! Energy per 30 ms frame after a pre-emphasis (speech's consonants count; hum and rumble
//! less), against the stretch's noise floor (its 15th percentile): a frame is speech when it
//! is 9 dB above the floor and above −55 dBFS, or simply loud (−35 dBFS: speech over music
//! still reaches Whisper, which judges it). Speech frames are widened by 200 ms before and
//! 400 ms after, gaps under 600 ms are closed, and stretches with under 250 ms of speech are
//! dropped.
//!
//! Windows are at most 30 s: consecutive regions share one while they fit; a region longer
//! than a window is cut at its quietest frame in the window's last 5 s.

/// Samples per frame (30 ms).
pub const FRAME: usize = 480;
const RATE: usize = super::mel::SAMPLE_RATE;

/// A stretch of sound in samples, `start..end`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }
}

/// Each frame's energy in dB (pre-emphasised).
pub fn energies(audio: &[f32]) -> Vec<f32> {
    audio
        .chunks(FRAME)
        .enumerate()
        .map(|(f, c)| {
            let mut prev = if f == 0 { 0.0 } else { audio[f * FRAME - 1] };
            let mut sum = 0.0f64;
            for &x in c {
                let y = x - 0.97 * prev;
                prev = x;
                sum += f64::from(y * y);
            }
            (10.0 * (sum / c.len() as f64 + 1e-10).log10()) as f32
        })
        .collect()
}

/// The stretches of `audio` that may hold speech, in order.
pub fn speech(audio: &[f32]) -> Vec<Span> {
    let e = energies(audio);
    if e.is_empty() {
        return Vec::new();
    }
    let mut sorted = e.clone();
    sorted.sort_by(f32::total_cmp);
    let floor = sorted[sorted.len() * 15 / 100];
    let quiet = (floor + 9.0).max(-55.0);
    let on: Vec<bool> = e.iter().map(|&v| v > quiet || v > -35.0).collect();
    let ms = |m: usize| m * RATE / 1000 / FRAME;
    let (pre, post, gap, min) = (ms(200), ms(400), ms(600), ms(250).max(1));
    // Runs of speech frames, with how many frames were speech.
    let mut runs: Vec<(usize, usize, usize)> = Vec::new();
    for (i, &s) in on.iter().enumerate() {
        if !s {
            continue;
        }
        let (a, b) = (i.saturating_sub(pre), (i + 1 + post).min(e.len()));
        match runs.last_mut() {
            Some(r) if a <= r.1 + gap => {
                r.1 = r.1.max(b);
                r.2 += 1;
            }
            _ => runs.push((a, b, 1)),
        }
    }
    runs.into_iter()
        .filter(|r| r.2 >= min)
        .map(|(a, b, _)| Span {
            start: a * FRAME,
            end: (b * FRAME).min(audio.len()),
        })
        .collect()
}

/// Windows of at most `max` samples covering `regions` (in order) of `audio`.
pub fn windows(regions: &[Span], audio: &[f32], max: usize) -> Vec<Span> {
    let max = max.max(FRAME * 2);
    let mut out: Vec<Span> = Vec::new();
    let mut cur: Option<Span> = None;
    for r in regions {
        let mut r = *r;
        if let Some(c) = cur.as_mut() {
            if r.end - c.start <= max {
                c.end = r.end;
                continue;
            }
            out.push(*c);
        }
        // A region longer than a window: cut at its quietest frame near each window's end.
        while r.len() > max {
            let hi = r.start + max;
            let lo = hi.saturating_sub(5 * RATE).max(r.start + FRAME);
            let cut = quietest(audio, lo, hi).unwrap_or(hi);
            out.push(Span {
                start: r.start,
                end: cut,
            });
            r.start = cut;
        }
        cur = Some(r);
    }
    out.extend(cur);
    out.retain(|w| !w.is_empty());
    out
}

/// The start of the quietest frame in `lo..hi`, frame-aligned to `lo`.
fn quietest(audio: &[f32], lo: usize, hi: usize) -> Option<usize> {
    let mut best: Option<(f32, usize)> = None;
    let mut at = lo;
    while at + FRAME <= hi.min(audio.len()) {
        let e: f32 = audio[at..at + FRAME].iter().map(|x| x * x).sum();
        if best.is_none_or(|b| e < b.0) {
            best = Some((e, at + FRAME / 2));
        }
        at += FRAME / 2;
    }
    best.map(|b| b.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Noise at −60 dBFS with "speech" (a 300 Hz tone with harmonics) in some seconds.
    fn sound(secs: usize, talk: &[(f32, f32)]) -> Vec<f32> {
        let mut s = 0x1234_5678u32;
        (0..secs * RATE)
            .map(|i| {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                let noise = (s as f32 / u32::MAX as f32 - 0.5) * 0.002;
                let t = i as f32 / RATE as f32;
                let on = talk.iter().any(|&(a, b)| t >= a && t < b);
                let v = if on {
                    let w = std::f32::consts::TAU * 300.0 * t;
                    0.2 * w.sin() + 0.1 * (3.0 * w).sin() + 0.05 * (7.0 * w).sin()
                } else {
                    0.0
                };
                noise + v
            })
            .collect()
    }

    fn secs(s: &Span) -> (f32, f32) {
        (s.start as f32 / RATE as f32, s.end as f32 / RATE as f32)
    }

    #[test]
    fn finds_speech_and_pads_it() {
        let a = sound(20, &[(2.0, 4.0), (4.3, 5.0), (12.0, 13.5)]);
        let r = speech(&a);
        assert_eq!(r.len(), 2, "{:?}", r.iter().map(secs).collect::<Vec<_>>());
        let (s0, e0) = secs(&r[0]);
        assert!(
            (1.75..=2.0).contains(&s0) && (5.3..=5.5).contains(&e0),
            "{s0} {e0}"
        );
        let (s1, e1) = secs(&r[1]);
        assert!(
            (11.75..=12.0).contains(&s1) && (13.8..=14.0).contains(&e1),
            "{s1} {e1}"
        );
        // Silence: nothing. A click: too short.
        assert!(speech(&sound(5, &[])).is_empty());
        assert!(speech(&sound(5, &[(1.0, 1.1)])).is_empty());
        assert!(speech(&[]).is_empty());
    }

    #[test]
    fn windows_cover_regions_within_the_limit() {
        let a = sound(100, &[]);
        let sp = |a: f32, b: f32| Span {
            start: (a * RATE as f32) as usize,
            end: (b * RATE as f32) as usize,
        };
        let regions = [sp(1.0, 5.0), sp(8.0, 20.0), sp(25.0, 40.0), sp(41.0, 90.0)];
        let w = windows(&regions, &a, 30 * RATE);
        let s: Vec<(f32, f32)> = w.iter().map(secs).collect();
        assert_eq!(s[0], (1.0, 20.0), "{s:?}");
        assert_eq!(s[1], (25.0, 40.0), "{s:?}");
        // 41–90 s: 49 s, cut once within 66–71 s.
        assert_eq!(s.len(), 4, "{s:?}");
        assert!((66.0..=71.0).contains(&s[2].1) && s[2].0 == 41.0, "{s:?}");
        assert_eq!(s[3].1, 90.0);
        assert!(w.iter().all(|w| w.len() <= 30 * RATE));
        // Cut at a quiet moment when there is one.
        let mut loud = sound(40, &[(0.0, 40.0)]);
        for v in &mut loud[(27 * RATE)..(27 * RATE + 2000)] {
            *v = 0.0;
        }
        let w = windows(&[sp(0.0, 40.0)], &loud, 30 * RATE);
        assert!((27.0..27.13).contains(&secs(&w[0]).1), "{:?}", secs(&w[0]));
    }
}
