//! The histogram of the visible image: 256 bins per channel over sRGB-encoded values, plus
//! the counts of clipped pixels.

use crate::colour::{bin, luma, srgb_encode};

pub const BINS: usize = 256;

/// A histogram channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Channel {
    Red,
    Green,
    Blue,
    /// Relative luminance, sRGB-encoded (so it lines up with the colour channels).
    Luma,
}

impl Channel {
    pub const ALL: [Channel; 4] = [Channel::Red, Channel::Green, Channel::Blue, Channel::Luma];
    pub const RGB: [Channel; 3] = [Channel::Red, Channel::Green, Channel::Blue];

    fn index(self) -> usize {
        self as usize
    }
}

/// Summary of one channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stats {
    pub count: u64,
    /// Mean 8-bit code.
    pub mean: f32,
    pub median: u8,
    pub min: u8,
    pub max: u8,
}

/// Bins of the four channels and the clipping counts.
#[derive(Clone, PartialEq, Eq)]
pub struct Histogram {
    bins: [[u32; BINS]; 4],
    /// Pixels counted (the visible, non-transparent ones).
    pub total: u32,
    /// Pixels with any colour channel at or above the highlight threshold.
    pub clipped_high: u32,
    /// Pixels with any colour channel at or below the shadow threshold.
    pub clipped_low: u32,
}

impl std::fmt::Debug for Histogram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Histogram")
            .field("total", &self.total)
            .field("clipped_high", &self.clipped_high)
            .field("clipped_low", &self.clipped_low)
            .finish_non_exhaustive()
    }
}

impl Default for Histogram {
    fn default() -> Self {
        Histogram {
            bins: [[0; BINS]; 4],
            total: 0,
            clipped_high: 0,
            clipped_low: 0,
        }
    }
}

impl Histogram {
    /// Words the GPU writes: 4 × 256 bins (R, G, B, luma), then total, clipped high,
    /// clipped low and one pad word.
    pub const WORDS: usize = 4 * BINS + 4;

    pub fn new() -> Histogram {
        Histogram::default()
    }

    /// From the GPU's words (see [`Histogram::WORDS`]); `None` when there are too few.
    pub fn from_words(words: &[u32]) -> Option<Histogram> {
        if words.len() < Self::WORDS {
            return None;
        }
        let mut h = Histogram::new();
        for (c, bins) in h.bins.iter_mut().enumerate() {
            bins.copy_from_slice(&words[c * BINS..(c + 1) * BINS]);
        }
        h.total = words[4 * BINS];
        h.clipped_high = words[4 * BINS + 1];
        h.clipped_low = words[4 * BINS + 2];
        Some(h)
    }

    /// The layout of [`Histogram::from_words`].
    pub fn to_words(&self) -> Vec<u32> {
        let mut w = Vec::with_capacity(Self::WORDS);
        for b in &self.bins {
            w.extend_from_slice(b);
        }
        w.extend_from_slice(&[self.total, self.clipped_high, self.clipped_low, 0]);
        w
    }

    /// Counts one pixel of straight (not premultiplied) linear RGB, exactly as the shader
    /// does. `low`/`high` are the clipping thresholds (8-bit codes, inclusive).
    pub fn add_linear(&mut self, lin: [f32; 3], low: u8, high: u8) {
        let mut lo = 255usize;
        let mut hi = 0usize;
        for (c, v) in lin.iter().enumerate() {
            let b = bin(srgb_encode(*v));
            self.bins[c][b] += 1;
            lo = lo.min(b);
            hi = hi.max(b);
        }
        self.bins[3][bin(srgb_encode(luma(lin)))] += 1;
        self.total += 1;
        if hi >= high as usize {
            self.clipped_high += 1;
        }
        if lo <= low as usize {
            self.clipped_low += 1;
        }
    }

    pub fn bins(&self, c: Channel) -> &[u32; BINS] {
        &self.bins[c.index()]
    }

    pub fn is_empty(&self) -> bool {
        self.total == 0
    }

    /// Fraction (0‥1) of counted pixels clipped in the highlights.
    pub fn high_fraction(&self) -> f32 {
        fraction(self.clipped_high, self.total)
    }

    /// Fraction (0‥1) of counted pixels clipped in the shadows.
    pub fn low_fraction(&self) -> f32 {
        fraction(self.clipped_low, self.total)
    }

    pub fn stats(&self, c: Channel) -> Stats {
        let b = self.bins(c);
        let count: u64 = b.iter().map(|&n| n as u64).sum();
        if count == 0 {
            return Stats {
                count: 0,
                mean: 0.0,
                median: 0,
                min: 0,
                max: 0,
            };
        }
        let sum: u64 = b
            .iter()
            .enumerate()
            .map(|(i, &n)| i as u64 * n as u64)
            .sum();
        let min = b.iter().position(|&n| n > 0).unwrap_or(0) as u8;
        let max = b.iter().rposition(|&n| n > 0).unwrap_or(0) as u8;
        // Lower median: the first bin where the running count reaches half.
        let half = count.div_ceil(2);
        let mut run = 0u64;
        let mut median = 0u8;
        for (i, &n) in b.iter().enumerate() {
            run += n as u64;
            if run >= half {
                median = i as u8;
                break;
            }
        }
        Stats {
            count,
            mean: sum as f32 / count as f32,
            median,
            min,
            max,
        }
    }

    /// The height the curves of `channels` are normalised to: the tallest bin, leaving out
    /// the two end bins (clipped pixels pile up there and would flatten everything else;
    /// their bars are capped at full height instead). With `log`, heights are `ln(1 + n)`.
    pub fn peak(&self, channels: &[Channel], log: bool) -> f32 {
        let mut m = 0u32;
        for &c in channels {
            let b = self.bins(c);
            m = m.max(b[1..BINS - 1].iter().copied().max().unwrap_or(0));
        }
        if m == 0 {
            // Only the end bins are used (a two-tone image): normalise to them.
            for &c in channels {
                let b = self.bins(c);
                m = m.max(b[0]).max(b[BINS - 1]);
            }
        }
        height(m, log)
    }

    /// Heights (0‥1) of `columns` bars for channel `c`, normalised to `peak` (from
    /// [`Histogram::peak`] with the same `log`). Each bar shows the tallest bin it covers.
    pub fn curve(&self, c: Channel, log: bool, peak: f32, columns: usize) -> Vec<f32> {
        let b = self.bins(c);
        let columns = columns.clamp(1, BINS);
        (0..columns)
            .map(|i| {
                let lo = i * BINS / columns;
                let hi = ((i + 1) * BINS / columns).max(lo + 1);
                let n = b[lo..hi].iter().copied().max().unwrap_or(0);
                if peak <= 0.0 {
                    0.0
                } else {
                    (height(n, log) / peak).min(1.0)
                }
            })
            .collect()
    }
}

fn height(n: u32, log: bool) -> f32 {
    if log { (n as f32).ln_1p() } else { n as f32 }
}

fn fraction(n: u32, total: u32) -> f32 {
    if total == 0 {
        0.0
    } else {
        n as f32 / total as f32
    }
}
