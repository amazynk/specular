//! Tempo change with the pitch kept, for playing sound faster or slower: WSOLA
//! (waveform-similarity overlap-add, Verhelst and Roelands, ICASSP 1993).
//!
//! Output is made a hop (12.5 ms) at a time. Each hop cross-fades (raised cosine, the two
//! halves of a Hann window at 50 % overlap) from the natural continuation of the previous
//! segment to a new segment of the input taken near the nominal position, which advances by
//! `speed` hops of input per hop of output. The new segment is the one, within ±6 ms of the
//! nominal position, most like that natural continuation (normalised cross-correlation of
//! the channels' sum, coarse then fine), so the cross-fade joins like with like and the
//! waveform's period, its pitch, is kept. All channels take the same segment: the stereo
//! image stays put.
//!
//! The state is only two positions (the nominal one and the continuation), so the stretcher
//! starts from plain playback without a gap (both at the next unplayed frame), and plain
//! playback picks up after any hop at the continuation.

use std::collections::VecDeque;

/// Hop length, seconds.
const HOP_S: f64 = 0.0125;

/// Coarse search: every this many lags and samples.
const COARSE: usize = 4;

pub(crate) struct Stretcher {
    ch: usize,
    /// Output frames per hop; the window is twice this.
    hop: usize,
    /// How far (frames) a segment may move from its nominal position.
    tol: usize,
    /// The cross-fade into the new segment (rising), `hop` long.
    fade: Vec<f32>,
    /// Nominal input position of the next segment (frames from the start of the input).
    pub(crate) nominal: f64,
    /// Where the previous segment would naturally go on (frames from the start).
    pub(crate) next: u64,
    /// The hop being handed out, interleaved, and how many frames of it are out.
    out: Vec<f32>,
    pub(crate) read: usize,
    /// Media position (input frames) of `out`'s first frame, and its input frames per frame.
    hop_at: f64,
    hop_speed: f64,
    /// Scratch: the input a hop looks at, and its channels' sum.
    frames: Vec<f32>,
    mono: Vec<f32>,
}

impl Stretcher {
    pub(crate) fn new(ch: usize, rate: u32) -> Stretcher {
        let hop = ((f64::from(rate) * HOP_S).round() as usize).max(8);
        let fade = (0..hop)
            .map(|i| {
                let x = std::f64::consts::PI * (i as f64 + 0.5) / hop as f64;
                (0.5 - 0.5 * x.cos()) as f32
            })
            .collect();
        Stretcher {
            ch: ch.max(1),
            hop,
            tol: hop / 2,
            fade,
            nominal: 0.0,
            next: 0,
            out: Vec::new(),
            read: 0,
            hop_at: 0.0,
            hop_speed: 1.0,
            frames: Vec::new(),
            mono: Vec::new(),
        }
    }

    /// Starts at input frame `at` (nothing pending).
    pub(crate) fn start(&mut self, at: u64) {
        self.nominal = at as f64;
        self.next = at;
        self.out.clear();
        self.read = 0;
    }

    /// Frames of the current hop not handed out yet.
    pub(crate) fn pending(&self) -> usize {
        self.out.len() / self.ch - self.read
    }

    /// The media position (input frames) of the next frame handed out, while one is pending.
    pub(crate) fn position(&self) -> f64 {
        self.hop_at + self.read as f64 * self.hop_speed
    }

    /// The input speed of the pending hop.
    pub(crate) fn hop_speed(&self) -> f64 {
        self.hop_speed
    }

    /// Hands out up to `dst.len() / ch` pending frames; how many.
    pub(crate) fn take(&mut self, dst: &mut [f32]) -> usize {
        let n = self.pending().min(dst.len() / self.ch);
        let from = self.read * self.ch;
        dst[..n * self.ch].copy_from_slice(&self.out[from..from + n * self.ch]);
        self.read += n;
        n
    }

    /// The first input frame still needed (frames before it may be dropped).
    pub(crate) fn keep_from(&self) -> u64 {
        let lo = (self.nominal.floor() as u64).saturating_sub(self.tol as u64);
        lo.min(self.next)
    }

    /// Makes the next hop at `speed` from `input` (interleaved, its first frame being input
    /// frame `base`). Past the end of the input (`ended`) it reads silence. False when the
    /// input does not reach far enough yet, or (ended) the input is used up.
    pub(crate) fn make_hop(
        &mut self,
        input: &VecDeque<f32>,
        base: u64,
        ended: bool,
        speed: f64,
    ) -> bool {
        let (ch, hop, tol) = (self.ch, self.hop, self.tol as u64);
        let have = base + (input.len() / ch) as u64;
        let nom = self.nominal.floor() as u64;
        let win = 2 * hop as u64;
        let need = (nom + tol + win).max(self.next + win);
        if need > have && !ended {
            return false;
        }
        if ended && nom >= have {
            return false;
        }
        let lo = nom.saturating_sub(tol).max(base);
        let hi = nom + tol;
        let len = win as usize;
        // The input this hop looks at, contiguous (silence past the end), and its channels'
        // sum for the similarity measure.
        let from = lo.min(self.next);
        let to = (hi + win).max(self.next + win);
        let end = to.min(have).max(from);
        let mut frames = std::mem::take(&mut self.frames);
        frames.clear();
        frames.extend(input.range((from - base) as usize * ch..(end - base) as usize * ch));
        frames.resize((to - from) as usize * ch, 0.0);
        let mut mono = std::mem::take(&mut self.mono);
        mono.clear();
        mono.extend(frames.chunks_exact(ch).map(|f| f.iter().sum::<f32>()));
        let target = &mono[(self.next - from) as usize..][..len];
        let cand = &mono[(lo - from) as usize..];
        let score = |off: usize, step: usize| -> f32 {
            let (mut xy, mut yy) = (0.0f32, 1e-9f32);
            let c = &cand[off..off + len];
            for j in (0..len).step_by(step) {
                xy += target[j] * c[j];
                yy += c[j] * c[j];
            }
            xy / yy.sqrt()
        };
        let span = (hi - lo) as usize;
        // Ties (silence) go to the nominal position.
        let mid = (nom - lo) as usize;
        let mut best = (mid, score(mid, COARSE));
        for off in (0..=span).step_by(COARSE) {
            let s = score(off, COARSE);
            if s > best.1 {
                best = (off, s);
            }
        }
        let (a, b) = (
            best.0.saturating_sub(COARSE - 1),
            (best.0 + COARSE - 1).min(span),
        );
        let mut fine = (best.0, score(best.0, 1));
        for off in a..=b {
            let s = score(off, 1);
            if s > fine.1 {
                fine = (off, s);
            }
        }
        let seg = lo + fine.0 as u64;
        let old = &frames[(self.next - from) as usize * ch..][..hop * ch];
        let new = &frames[(seg - from) as usize * ch..][..hop * ch];
        self.out.clear();
        for ((o, n), &w) in old
            .chunks_exact(ch)
            .zip(new.chunks_exact(ch))
            .zip(&self.fade)
        {
            self.out
                .extend(o.iter().zip(n).map(|(&o, &n)| o + (n - o) * w));
        }
        self.frames = frames;
        self.mono = mono;
        self.read = 0;
        // Within the hop the sound runs at 1×; its middle is heard at nominal + hop / 2.
        self.hop_at = self.nominal + hop as f64 * 0.5 * (1.0 - speed);
        self.hop_speed = speed;
        self.next = seg + hop as u64;
        self.nominal += hop as f64 * speed;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stretches all of `x` at `speed`.
    fn stretch(x: &[f32], ch: usize, rate: u32, speed: f64) -> Vec<f32> {
        let mut s = Stretcher::new(ch, rate);
        s.start(0);
        let input: VecDeque<f32> = x.iter().copied().collect();
        let mut out = Vec::new();
        let mut buf = vec![0.0; 512 * ch];
        while s.make_hop(&input, 0, true, speed) {
            while s.pending() > 0 {
                let n = s.take(&mut buf);
                out.extend_from_slice(&buf[..n * ch]);
            }
        }
        out
    }

    #[test]
    fn stereo_image_kept() {
        // Left a sine, right the same sine at half level: every output frame keeps the ratio.
        let x: Vec<f32> = (0..48_000)
            .flat_map(|i| {
                let v = (i as f32 * 0.05).sin();
                [v, 0.5 * v]
            })
            .collect();
        let y = stretch(&x, 2, 48_000, 1.5);
        for f in y.chunks_exact(2) {
            assert!((f[1] - 0.5 * f[0]).abs() < 1e-6);
        }
    }

    /// Share of one core the stretcher takes at 2× on 48 kHz stereo and 7.1 (noise-like
    /// content in every channel). Prints; the budget is checked loosely (dev builds, loaded
    /// machines).
    #[test]
    fn cpu_load_at_2x() {
        for ch in [2usize, 8] {
            let secs = 10.0;
            let n = (48_000.0 * secs) as usize;
            let mut seed = 0x1234_5678u32;
            let x: Vec<f32> = (0..n * ch)
                .map(|i| {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    let noise = (seed as f32 / u32::MAX as f32 - 0.5) * 0.2;
                    noise + 0.3 * ((i / ch) as f32 * 0.03).sin()
                })
                .collect();
            let t = std::time::Instant::now();
            let y = stretch(&x, ch, 48_000, 2.0);
            let took = t.elapsed().as_secs_f64();
            // Seconds of sound heard: half the input at 2×.
            let heard = y.len() as f64 / ch as f64 / 48_000.0;
            let load = took / heard * 100.0;
            eprintln!("{ch} channels at 2×: {load:.2} % of a core");
            assert!(load < 25.0, "{ch} channels: {load:.1} % of a core");
        }
    }

    #[test]
    fn silence_stays_put() {
        let y = stretch(&vec![0.0; 9600], 1, 48_000, 2.0);
        assert!(y.iter().all(|&v| v == 0.0));
        assert!((y.len() as i64 - 4800).abs() <= 600, "{}", y.len());
    }
}
