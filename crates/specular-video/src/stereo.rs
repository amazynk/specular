//! The stereo panel's measures: the phase correlation of left and right, and the
//! goniometer (vectorscope) picture of them.
//!
//! - Correlation: Σ L·R / √(Σ L² · Σ R²) over a short span, +1 for the same sound in both
//!   (mono), 0 for unrelated sound (wide stereo, or one side silent), −1 for one side the
//!   other turned over (out of phase: it cancels when summed to mono).
//! - Goniometer: each frame a point at x = (R − L)/√2, y = (L + R)/√2, so mono is a
//!   vertical line, left only leans up-left, right only up-right, and out-of-phase sound is
//!   flat. Points add into a grid that fades (persistence); a slow automatic gain keeps quiet
//!   sound readable. Reduce motion: longer persistence and fewer redraws.

/// The correlation from the sums (or means) of L·R, L² and R²; `None` in silence.
pub fn correlation(lr: f64, ll: f64, rr: f64) -> Option<f32> {
    const QUIET: f64 = 1e-10;
    if ll <= QUIET && rr <= QUIET {
        return None;
    }
    if ll <= QUIET || rr <= QUIET {
        return Some(0.0);
    }
    Some((lr / (ll * rr).sqrt()).clamp(-1.0, 1.0) as f32)
}

/// The correlation of interleaved stereo `frames` (`ch` channels: the first two are left
/// and right).
pub fn correlation_of(pcm: &[f32], ch: usize) -> Option<f32> {
    if ch < 2 {
        return None;
    }
    let (mut lr, mut ll, mut rr) = (0.0f64, 0.0f64, 0.0f64);
    for f in pcm.chunks_exact(ch) {
        let (l, r) = (f64::from(f[0]), f64::from(f[1]));
        lr += l * r;
        ll += l * l;
        rr += r * r;
    }
    correlation(lr, ll, rr)
}

/// The span the correlation is measured over (seconds).
pub const CORRELATION_S: f64 = 0.2;
/// The slow average's time constant (seconds).
pub const AVERAGE_S: f32 = 3.0;

/// The goniometer's fading picture.
#[derive(Clone, Debug)]
pub struct Goniometer {
    size: usize,
    grid: Vec<f32>,
    /// The automatic gain (1: full scale reaches the edge).
    gain: f32,
}

/// How long a point stays visible (seconds to fade to 1/e).
pub fn persistence(reduce_motion: bool) -> f32 {
    if reduce_motion { 0.6 } else { 0.15 }
}

/// The least time between redraws of the picture (seconds).
pub fn redraw_every(reduce_motion: bool) -> f32 {
    if reduce_motion { 1.0 / 12.0 } else { 0.0 }
}

impl Goniometer {
    /// A picture `size` × `size`.
    pub fn new(size: usize) -> Goniometer {
        let size = size.max(8);
        Goniometer {
            size,
            grid: vec![0.0; size * size],
            gain: 1.0,
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn gain(&self) -> f32 {
        self.gain
    }

    /// Empties the picture.
    pub fn clear(&mut self) {
        self.grid.fill(0.0);
        self.gain = 1.0;
    }

    /// Fades what is there by `dt` seconds of persistence `tau`.
    pub fn fade(&mut self, dt: f32, tau: f32) {
        let k = (-dt.max(0.0) / tau.max(1e-3)).exp();
        for v in &mut self.grid {
            *v *= k;
        }
    }

    /// The point of frame (l, r), −1…1 each way (y up), before the gain.
    pub fn point(l: f32, r: f32) -> (f32, f32) {
        let k = std::f32::consts::FRAC_1_SQRT_2;
        ((r - l) * k, (l + r) * k)
    }

    /// Adds the frames of interleaved `pcm` (`ch` channels, the first two left and right);
    /// `dt` seconds of sound, for the gain's ballistics.
    pub fn add(&mut self, pcm: &[f32], ch: usize, dt: f32) {
        if ch < 2 || pcm.len() < ch {
            return;
        }
        // The gain follows the level: its peak radius reaches 0.8 of the edge, at most 16×
        // up; it comes down at once and goes up slowly.
        let mut peak = 0.0f32;
        for f in pcm.chunks_exact(ch) {
            let (x, y) = Goniometer::point(f[0], f[1]);
            peak = peak.max(x.abs()).max(y.abs());
        }
        if peak > 1e-5 {
            let want = (0.8 / peak).clamp(0.5, 16.0);
            if want < self.gain {
                self.gain = want;
            } else {
                let k = 1.0 - (-dt.max(0.0) / 1.5).exp();
                self.gain += (want - self.gain) * k;
            }
        }
        let n = self.size as f32;
        let frames = pcm.len() / ch;
        // Each point's weight: a picture of any rate and span sums alike.
        let w = (n * n * 0.05) / frames.max(1) as f32;
        for f in pcm.chunks_exact(ch) {
            let (x, y) = Goniometer::point(f[0], f[1]);
            let px = ((x * self.gain * 0.5 + 0.5) * n).floor();
            let py = ((0.5 - y * self.gain * 0.5) * n).floor();
            if px < 0.0 || py < 0.0 || px >= n || py >= n {
                continue;
            }
            self.grid[py as usize * self.size + px as usize] += w;
        }
    }

    /// The picture over `bg` in `fg` (sRGB bytes), opaque RGBA rows.
    pub fn rgba(&self, bg: [u8; 3], fg: [u8; 3], out: &mut Vec<u8>) {
        out.clear();
        out.reserve(self.grid.len() * 4);
        for &v in &self.grid {
            let a = 1.0 - (-v).exp();
            for c in 0..3 {
                let b = f32::from(bg[c]);
                out.push((b + (f32::from(fg[c]) - b) * a).round() as u8);
            }
            out.push(255);
        }
    }

    /// The amount at cell (x, y), for tests.
    pub fn at(&self, x: usize, y: usize) -> f32 {
        self.grid[y * self.size + x]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stereo(n: usize, f: impl Fn(f64) -> (f64, f64)) -> Vec<f32> {
        (0..n)
            .flat_map(|i| {
                let (l, r) = f(i as f64 / 48_000.0);
                [l as f32, r as f32]
            })
            .collect()
    }

    fn sine(f: f64, t: f64) -> f64 {
        (std::f64::consts::TAU * f * t).sin()
    }

    #[test]
    fn correlation_of_generated_signals() {
        let n = 9600;
        // The same in both: +1.
        let mono = stereo(n, |t| (0.5 * sine(440.0, t), 0.5 * sine(440.0, t)));
        assert!((correlation_of(&mono, 2).unwrap() - 1.0).abs() < 1e-6);
        // One side turned over: −1.
        let inv = stereo(n, |t| (0.5 * sine(440.0, t), -0.5 * sine(440.0, t)));
        assert!((correlation_of(&inv, 2).unwrap() + 1.0).abs() < 1e-6);
        // A quarter period apart (90°): 0.
        let quad = stereo(n, |t| (sine(500.0, t), sine(500.0, t + 0.0005)));
        assert!(correlation_of(&quad, 2).unwrap().abs() < 1e-3);
        // Unrelated noise: near 0.
        let mut seed = 7u32;
        let mut noise = move || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            f64::from(seed >> 8) / f64::from(1u32 << 24) * 2.0 - 1.0
        };
        let pcm: Vec<f32> = (0..n * 2).map(|_| noise() as f32).collect();
        assert!(correlation_of(&pcm, 2).unwrap().abs() < 0.05);
        // Different levels, same sound: still +1; 60° apart: cos 60° = 0.5.
        let lev = stereo(n, |t| (0.9 * sine(300.0, t), 0.1 * sine(300.0, t)));
        assert!((correlation_of(&lev, 2).unwrap() - 1.0).abs() < 1e-5);
        let sixty = stereo(n, |t| (sine(500.0, t), sine(500.0, t + 1.0 / 3000.0)));
        assert!((correlation_of(&sixty, 2).unwrap() - 0.5).abs() < 1e-3);
        // Silence: none; one side silent: 0; mono sound: none.
        assert_eq!(correlation_of(&vec![0.0; 200], 2), None);
        let left = stereo(n, |t| (sine(440.0, t), 0.0));
        assert_eq!(correlation_of(&left, 2), Some(0.0));
        assert_eq!(correlation_of(&mono, 1), None);
        // Other channels after the first two are left out.
        let six: Vec<f32> = mono
            .chunks_exact(2)
            .flat_map(|f| [f[0], f[1], 0.3, -0.3, 0.1, 0.2])
            .collect();
        assert!((correlation_of(&six, 6).unwrap() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn goniometer_draws_where_the_sound_is() {
        let mut g = Goniometer::new(64);
        // Mono: on the vertical line.
        let mono = stereo(4800, |t| (0.7 * sine(440.0, t), 0.7 * sine(440.0, t)));
        g.add(&mono, 2, 0.1);
        let c = 32;
        let on: f32 = (0..64).map(|y| g.at(c, y) + g.at(c - 1, y)).sum();
        let total: f32 = (0..64)
            .flat_map(|y| (0..64).map(move |x| (x, y)))
            .map(|(x, y)| g.at(x, y))
            .sum();
        assert!(on / total > 0.99, "{on} of {total}");
        // Out of phase: on the horizontal line.
        g.clear();
        let inv = stereo(4800, |t| (0.7 * sine(440.0, t), -0.7 * sine(440.0, t)));
        g.add(&inv, 2, 0.1);
        let on: f32 = (0..64).map(|x| g.at(x, 32) + g.at(x, 31)).sum();
        assert!(on > 0.0);
        let row_sum: f32 = (0..64)
            .flat_map(|y| (0..64).map(move |x| (x, y)))
            .filter(|&(_, y)| y == 31 || y == 32)
            .map(|(x, y)| g.at(x, y))
            .sum();
        assert!((row_sum - on).abs() < 1e-3);
        // Left only leans up-left.
        let (x, y) = Goniometer::point(1.0, 0.0);
        assert!(x < 0.0 && y > 0.0);
        // It fades.
        let before = g.at(10, 32) + g.at(10, 31);
        g.fade(0.15, persistence(false));
        let after = g.at(10, 32) + g.at(10, 31);
        assert!((after / before - (-1.0f32).exp()).abs() < 1e-4);
        // The gain lifts quiet sound, no more than 16×; loud sound is not lifted.
        let mut q = Goniometer::new(64);
        let quiet = stereo(4800, |t| (0.001 * sine(440.0, t), 0.001 * sine(440.0, t)));
        for _ in 0..100 {
            q.add(&quiet, 2, 0.1);
        }
        assert!((q.gain() - 16.0).abs() < 0.1);
        q.add(&mono, 2, 0.1);
        assert!(q.gain() <= 1.0);
        // The picture: background where nothing is.
        let mut px = Vec::new();
        g.clear();
        g.rgba([10, 20, 30], [200, 200, 200], &mut px);
        assert_eq!(px.len(), 64 * 64 * 4);
        assert_eq!(&px[..4], &[10, 20, 30, 255]);
    }
}
