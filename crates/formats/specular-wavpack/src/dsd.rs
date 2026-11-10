//! DSD (1-bit) WavPack: the three block codings (stored, "fast" and "high"), and the
//! decimating low-pass filter that turns the 1-bit stream into PCM for playback.
//!
//! A DSD block's audio is one `ID_DSD_BLOCK` sub-block: a byte with the log2 of the rate
//! multiplier (the header's rate × multiplier is the byte rate), a mode byte, then the
//! coded bytes. Each output value is one byte of eight 1-bit samples, oldest bit on top
//! (DSDIFF order), channels interleaved by byte. Blocks are independent: every coder
//! starts afresh, and the block checksum (`crc = crc · 3 + byte`) covers the coded bytes.
//!
//! Own code, written from what the reference decoder does (WavPack 5.8.1, BSD licence),
//! studied for its rules only.

use crate::{Error, Header, bad};

/// "Fast" mode: at most this many history bits pick a probability table.
const MAX_HISTORY_BITS: u8 = 5;
/// "Fast" mode: at most this many symbols summed in a table, on average.
const MAX_BYTES_PER_BIN: usize = 1280;

/// Decodes a DSD block's sub-block `d` (`samples` bytes a channel; 1 or 2 channels) into
/// `out` as bytes (0–255). Returns the coded channels (1 or 2) and the checksum.
pub(crate) fn decode(h: &Header, d: &[u8], out: &mut Vec<i32>) -> Result<(usize, u32), Error> {
    let ch = if h.mono_data() { 1 } else { 2 };
    let n = h.block_samples as usize * ch;
    if d.len() < 2 || d[0] > 31 {
        return Err(bad("DSD block"));
    }
    let start = out.len();
    out.reserve(n);
    let body = &d[2..];
    let ok = match d[1] {
        0 => {
            if body.len() != n {
                return Err(bad("DSD block length"));
            }
            out.extend(body.iter().map(|&b| i32::from(b)));
            true
        }
        1 => fast(body, ch, n, out)?,
        3 => high(body, ch, n, out)?,
        m => return Err(Error::Unsupported(format!("DSD coding mode {m}"))),
    };
    if !ok || out.len() != start + n {
        return Err(bad("DSD bits cut short"));
    }
    let crc = out[start..].iter().fold(0xffff_ffffu32, |c, &b| {
        c.wrapping_mul(3).wrapping_add(b as u32)
    });
    Ok((ch, crc))
}

/// The byte rate multiplier's log2 (the first byte of a DSD sub-block).
pub(crate) fn multiplier_log2(d: &[u8]) -> Option<u32> {
    d.first().map(|&b| u32::from(b)).filter(|&b| b <= 31)
}

/// The usual name of a 1-bit rate: "DSD64" for 64 × 44.1 kHz (or 64 × 48 kHz), else "DSD".
pub fn name(rate: u32) -> String {
    [44_100, 48_000]
        .into_iter()
        .find(|&b| rate.is_multiple_of(b) && (rate / b).is_power_of_two())
        .map_or_else(|| "DSD".into(), |b| format!("DSD{}", rate / b))
}

/// A range decoder's state, shared by both coded modes.
struct Range<'a> {
    d: &'a [u8],
    at: usize,
    low: u32,
    high: u32,
    value: u32,
}

impl<'a> Range<'a> {
    fn new(d: &'a [u8]) -> Option<Range<'a>> {
        let v = d.get(..4)?;
        Some(Range {
            d,
            at: 4,
            low: 0,
            high: u32::MAX,
            value: u32::from_be_bytes([v[0], v[1], v[2], v[3]]),
        })
    }

    /// Shifts in bytes while the top bytes of `low` and `high` agree (and input is left).
    #[inline]
    fn normalize(&mut self) {
        while (self.low ^ self.high) & 0xff00_0000 == 0 && self.at < self.d.len() {
            self.value = self.value << 8 | u32::from(self.d[self.at]);
            self.at += 1;
            self.high = self.high << 8 | 0xff;
            self.low <<= 8;
        }
    }
}

/// "Fast" mode: each byte is coded with a probability table chosen by the channel's
/// previous byte (its low `history_bits`). Returns false when the coded bits fail.
fn fast(d: &[u8], ch: usize, n: usize, out: &mut Vec<i32>) -> Result<bool, Error> {
    let (&hb, mut d) = d.split_first().ok_or_else(|| bad("DSD block"))?;
    if hb > MAX_HISTORY_BITS {
        return Err(bad("DSD history"));
    }
    let bins = 1usize << hb;
    let (&maxp, rest) = d.split_first().ok_or_else(|| bad("DSD block"))?;
    d = rest;
    let mut probs = vec![0u8; bins * 256];
    if maxp < 0xff {
        // Run-length coded: a code above `maxp` is that many more zeros, 0 ends the list.
        let mut o = 0;
        let mut at = 0;
        while o < probs.len() && at < d.len() {
            let code = d[at];
            at += 1;
            if code > maxp {
                let z = usize::from(code - maxp);
                o = (o + z).min(probs.len());
            } else if code != 0 {
                probs[o] = code;
                o += 1;
            } else {
                break;
            }
        }
        if o < probs.len() {
            return Err(bad("DSD probabilities"));
        }
        if at < d.len() {
            if d[at] != 0 {
                return Err(bad("DSD probabilities"));
            }
            at += 1;
        }
        d = &d[at..];
    } else {
        if d.len() <= probs.len() {
            return Err(bad("DSD probabilities"));
        }
        probs.copy_from_slice(&d[..bins * 256]);
        d = &d[bins * 256..];
    }
    // Cumulative sums, and each bin's symbols spelled out for a direct lookup.
    let mut sums = vec![0u32; bins * 256];
    let mut lookup: Vec<u8> = Vec::new();
    let mut first = vec![0usize; bins];
    for b in 0..bins {
        let mut s = 0u32;
        for i in 0..256 {
            s += u32::from(probs[b * 256 + i]);
            sums[b * 256 + i] = s;
        }
        first[b] = lookup.len();
        for i in 0..256 {
            let c = probs[b * 256 + i];
            lookup.extend(std::iter::repeat_n(i as u8, usize::from(c)));
        }
        if lookup.len() > bins * MAX_BYTES_PER_BIN {
            return Err(bad("DSD probabilities"));
        }
    }
    let mut r = Range::new(d).ok_or_else(|| bad("DSD bits cut short"))?;
    let (mut p0, mut p1) = (0usize, 0usize);
    let mask = bins - 1;
    for _ in 0..n {
        let total = sums[p0 * 256 + 255];
        if total == 0 {
            return Ok(false);
        }
        let mut mult = (r.high.wrapping_sub(r.low)) / total;
        if mult == 0 {
            if r.d.len() - r.at >= 4 {
                for _ in 0..4 {
                    r.value = r.value << 8 | u32::from(r.d[r.at]);
                    r.at += 1;
                }
            }
            r.low = 0;
            r.high = u32::MAX;
            mult = r.high / total;
            if mult == 0 {
                return Ok(false);
            }
        }
        let index = r.value.wrapping_sub(r.low) / mult;
        if index >= total {
            return Ok(false);
        }
        let code = lookup[first[p0] + index as usize];
        if code != 0 {
            r.low = r
                .low
                .wrapping_add(sums[p0 * 256 + usize::from(code) - 1].wrapping_mul(mult));
        }
        r.high = r
            .low
            .wrapping_add(u32::from(probs[p0 * 256 + usize::from(code)]).wrapping_mul(mult))
            .wrapping_sub(1);
        out.push(i32::from(code));
        if ch == 1 {
            p0 = usize::from(code) & mask;
        } else {
            p0 = p1;
            p1 = usize::from(code) & mask;
        }
        r.normalize();
    }
    Ok(true)
}

// "High" mode: each bit is coded with an adaptive probability picked by a prediction of
// the analog signal from a small bank of one-pole filters.
const PRECISION: u32 = 20;
const VALUE_ONE: i32 = 1 << PRECISION;
const PRECISION_USE: u32 = 12;
const UP: i32 = 0x0100_00fe;
const DOWN: i32 = 0x0001_0000;
const DECAY: u32 = 8;
const RATE_S: u8 = 20;

/// The 256 adaptive probabilities' starting values: halves mirrored around 0x808000,
/// falling towards `DOWN` at a rate set by `rate_i` that grows by `rate_s`/256 a step.
fn ptable(rate_i: u8, rate_s: u8) -> [i32; 256] {
    let mut t = [0i32; 256];
    let mut value: i64 = 0x80_8000;
    let mut rate = i64::from(rate_i) << 8;
    let down = i64::from(DOWN);
    for _ in 0..(rate + 128) >> 8 {
        value += (down - value) >> DECAY;
    }
    for i in 0..128 {
        t[i] = value as i32;
        t[255 - i] = (0x0100_ffff - value) as i32;
        if value > 0x1_0000 {
            rate += (rate * i64::from(rate_s) + 128) >> 8;
            let mut c = (rate + 64) >> 7;
            // The value settles on `DOWN` long before the count runs out.
            while c > 0 && value > down {
                value += (down - value) >> DECAY;
                c -= 1;
            }
        }
    }
    t
}

#[derive(Clone, Copy, Default)]
struct Filters {
    f1: i32,
    f2: i32,
    f3: i32,
    f4: i32,
    f5: i32,
    f6: i32,
    factor: i32,
    value: i32,
    byte: u32,
}

impl Filters {
    #[inline]
    fn predict(&self) -> i32 {
        self.f1
            .wrapping_sub(self.f5)
            .wrapping_add(self.f6.wrapping_mul(self.factor) >> 2)
    }

    /// Codes one bit: returns it after updating the shared probability and the filters.
    #[inline]
    fn bit(&mut self, r: &mut Range, t: &mut [i32; 256]) {
        let p = &mut t[((self.value >> (PRECISION - PRECISION_USE)) & 0xff) as usize];
        let split = r
            .low
            .wrapping_add(((r.high.wrapping_sub(r.low)) >> 8).wrapping_mul((*p >> 16) as u32));
        // `f0` is −1 for a one bit, 0 for a zero bit.
        let f0: i32 = if r.value <= split {
            r.high = split;
            *p += (UP - *p) >> DECAY;
            -1
        } else {
            r.low = split.wrapping_add(1);
            *p += (DOWN - *p) >> DECAY;
            0
        };
        r.normalize();
        self.value = self.value.wrapping_add(self.f6.wrapping_mul(8));
        self.byte = self.byte << 1 | (f0 & 1) as u32;
        let moved = (self.value ^ self.value.wrapping_sub(self.f6.wrapping_mul(16))) >> 31;
        self.factor = self
            .factor
            .wrapping_add((((self.value ^ f0) >> 31) | 1) & moved);
        let one = f0 & VALUE_ONE;
        self.f1 = self.f1.wrapping_add((one.wrapping_sub(self.f1)) >> 6);
        self.f2 = self.f2.wrapping_add((one.wrapping_sub(self.f2)) >> 4);
        self.f3 = self.f3.wrapping_add((self.f2.wrapping_sub(self.f3)) >> 4);
        self.f4 = self.f4.wrapping_add((self.f3.wrapping_sub(self.f4)) >> 4);
        let v = (self.f4.wrapping_sub(self.f5)) >> 4;
        self.f5 = self.f5.wrapping_add(v);
        self.f6 = self.f6.wrapping_add((v.wrapping_sub(self.f6)) >> 3);
        self.value = self.predict();
    }
}

fn high(d: &[u8], ch: usize, n: usize, out: &mut Vec<i32>) -> Result<bool, Error> {
    if d.len() < if ch == 1 { 13 } else { 20 } {
        return Err(bad("DSD block"));
    }
    let (rate_i, rate_s) = (d[0], d[1]);
    if rate_s != RATE_S {
        return Err(bad("DSD rate"));
    }
    let mut t = ptable(rate_i, rate_s);
    let mut f = [Filters::default(); 2];
    let mut at = 2;
    for s in f.iter_mut().take(ch) {
        let b = &d[at..at + 7];
        s.f1 = i32::from(b[0]) << (PRECISION - 8);
        s.f2 = i32::from(b[1]) << (PRECISION - 8);
        s.f3 = i32::from(b[2]) << (PRECISION - 8);
        s.f4 = i32::from(b[3]) << (PRECISION - 8);
        s.f5 = i32::from(b[4]) << (PRECISION - 8);
        s.factor = i32::from(i16::from_le_bytes([b[5], b[6]]));
        at += 7;
    }
    let mut r = Range::new(&d[at..]).ok_or_else(|| bad("DSD bits cut short"))?;
    let frames = n / ch;
    for _ in 0..frames {
        for s in f.iter_mut().take(ch) {
            s.value = s.predict();
        }
        for _ in 0..8 {
            for s in f.iter_mut().take(ch) {
                s.bit(&mut r, &mut t);
            }
        }
        for s in f.iter_mut().take(ch) {
            out.push((s.byte & 0xff) as i32);
            s.factor -= (s.factor + 512) >> 10;
        }
    }
    Ok(true)
}

/// The decimating low-pass filter's design: a Kaiser-windowed sinc, its taps a whole number
/// of bytes so each byte of history is one table lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilterSpec {
    /// DSD bytes (8 one-bit samples) a PCM sample.
    pub decimation: u32,
    /// The PCM rate.
    pub pcm_rate: u32,
    /// Passband edge and stopband start (Hz).
    pub passband: u32,
    pub stopband: u32,
    /// Stopband attenuation (dB) the window is designed for.
    pub attenuation: u32,
    /// Taps (1-bit samples), a multiple of 8.
    pub taps: usize,
}

/// Stopband attenuation of the decimation filter (dB); the passband ripple is the same
/// fraction (±0.00003 dB).
pub const ATTENUATION: u32 = 110;
/// The PCM rate DSD is brought down to, at least (88.2 kHz for 44.1 kHz-based DSD, 96 kHz
/// for 48 kHz-based).
pub const MIN_PCM_RATE: u32 = 88_200;
/// The highest DSD rate played: DSD512 (22.6 or 24.6 MHz).
pub const MAX_DSD_RATE: u32 = 512 * 48_000;

impl FilterSpec {
    /// The filter for a DSD stream of `byte_rate` bytes a second a channel.
    pub fn new(byte_rate: u32) -> FilterSpec {
        let mut decimation = 1u32;
        let mut pcm = byte_rate;
        while pcm.is_multiple_of(2) && pcm / 2 >= MIN_PCM_RATE {
            pcm /= 2;
            decimation *= 2;
        }
        let fs = f64::from(byte_rate) * 8.0;
        let stopband = pcm / 2;
        let passband = 20_000.min(stopband / 10 * 9);
        let df = f64::from(stopband - passband) / fs;
        let n = ((f64::from(ATTENUATION) - 7.95) / (14.36 * df) + 1.0).ceil() as usize;
        FilterSpec {
            decimation,
            pcm_rate: pcm,
            passband,
            stopband,
            attenuation: ATTENUATION,
            taps: n.div_ceil(8) * 8,
        }
    }

    /// The taps (sum 1: a steady stream of ones is 1.0), oldest-last order irrelevant as
    /// the filter is symmetric.
    pub fn coefficients(&self) -> Vec<f64> {
        let n = self.taps;
        let fs = f64::from(self.pcm_rate) * f64::from(self.decimation) * 8.0;
        let fc = f64::from(self.passband + self.stopband) / 2.0 / fs;
        let beta = 0.1102 * (f64::from(self.attenuation) - 8.7);
        let i0b = bessel_i0(beta);
        let mid = (n - 1) as f64 / 2.0;
        let mut h: Vec<f64> = (0..n)
            .map(|i| {
                let x = i as f64 - mid;
                let sinc = if x == 0.0 {
                    2.0 * fc
                } else {
                    (2.0 * std::f64::consts::PI * fc * x).sin() / (std::f64::consts::PI * x)
                };
                let r = 2.0 * i as f64 / (n - 1) as f64 - 1.0;
                sinc * bessel_i0(beta * (1.0 - r * r).max(0.0).sqrt()) / i0b
            })
            .collect();
        let s: f64 = h.iter().sum();
        for v in &mut h {
            *v /= s;
        }
        h
    }
}

/// The modified Bessel function of the first kind, order 0 (power series).
fn bessel_i0(x: f64) -> f64 {
    let q = x * x / 4.0;
    let mut term = 1.0;
    let mut sum = 1.0;
    for k in 1..200 {
        term *= q / (k * k) as f64;
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
    }
    sum
}

/// Turns DSD bytes into PCM: a FIR low-pass applied to the ±1 stream and kept every
/// `decimation` bytes, by one table lookup a byte of history (each table holds the sum of
/// eight taps for every byte value). Keeps its history between consecutive blocks; a jump
/// (a seek) starts again from silence.
pub struct DsdToPcm {
    spec: FilterSpec,
    channels: usize,
    tables: Vec<[f32; 256]>,
    /// Per channel: the last `tables.len() − 1` bytes, then the block's bytes.
    hist: Vec<Vec<u8>>,
    /// The byte index the next block should start at.
    next: Option<u64>,
}

/// The DSD idle pattern: alternating bits, nothing after the low-pass.
const SILENCE: u8 = 0x55;

impl DsdToPcm {
    /// A converter for `channels` channels of DSD at `byte_rate` bytes a second.
    pub fn new(byte_rate: u32, channels: usize) -> DsdToPcm {
        let spec = FilterSpec::new(byte_rate);
        let h = spec.coefficients();
        // Table t covers the bits of the byte t bytes back: its bit j (value 1 << j) is
        // 8t + j one-bit samples old, the newest bit being the lowest.
        let tables: Vec<[f32; 256]> = h
            .chunks_exact(8)
            .map(|c| {
                let mut t = [0f32; 256];
                for (v, e) in t.iter_mut().enumerate() {
                    let s: f64 = (0..8)
                        .map(|j| if v >> j & 1 != 0 { c[j] } else { -c[j] })
                        .sum();
                    *e = s as f32;
                }
                t
            })
            .collect();
        let keep = tables.len() - 1;
        DsdToPcm {
            spec,
            channels,
            tables,
            hist: vec![vec![SILENCE; keep]; channels],
            next: None,
        }
    }

    pub fn spec(&self) -> FilterSpec {
        self.spec
    }

    /// Forgets the history (after a seek).
    pub fn reset(&mut self) {
        let keep = self.tables.len() - 1;
        for h in &mut self.hist {
            h.clear();
            h.resize(keep, SILENCE);
        }
        self.next = None;
    }

    /// Filters a block's interleaved bytes (as [`crate::decode_frame`] gives them) that
    /// start at byte `index` of the stream, appending interleaved PCM to `out`: a sample
    /// after every byte whose index + 1 is a multiple of the decimation, so a block yields
    /// `⌊(index + n) / d⌋ − ⌊index / d⌋` samples.
    pub fn run(&mut self, index: u64, bytes: &[i32], out: &mut Vec<f32>) {
        if self.next != Some(index) {
            self.reset();
        }
        let ch = self.channels;
        let n = bytes.len() / ch;
        self.next = Some(index + n as u64);
        let keep = self.tables.len() - 1;
        let d = u64::from(self.spec.decimation);
        // The first byte (in the block) a sample follows.
        let first = ((d - 1 + d - index % d) % d) as usize;
        let count = if first < n {
            (n - 1 - first) / d as usize + 1
        } else {
            0
        };
        let base = out.len();
        out.resize(base + count * ch, 0.0);
        for c in 0..ch {
            let h = &mut self.hist[c];
            h.extend(bytes[c..].iter().step_by(ch).map(|&b| b as u8));
            for k in 0..count {
                // The newest byte is at `keep + first + k·d` in `h`.
                let p = keep + first + k * d as usize;
                let win = &h[p - keep..=p];
                out[base + k * ch + c] = dot(&self.tables, win);
            }
            h.drain(..h.len() - keep);
        }
    }
}

/// Σ tables[t][win[last − t]].
#[inline]
fn dot(tables: &[[f32; 256]], win: &[u8]) -> f32 {
    let mut acc = [0f32; 4];
    let last = win.len() - 1;
    let mut t = 0;
    while t + 4 <= tables.len() {
        acc[0] += tables[t][usize::from(win[last - t])];
        acc[1] += tables[t + 1][usize::from(win[last - t - 1])];
        acc[2] += tables[t + 2][usize::from(win[last - t - 2])];
        acc[3] += tables[t + 3][usize::from(win[last - t - 3])];
        t += 4;
    }
    while t < tables.len() {
        acc[0] += tables[t][usize::from(win[last - t])];
        t += 1;
    }
    (acc[0] + acc[1]) + (acc[2] + acc[3])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_for_the_usual_rates() {
        for (rate, d, pcm) in [
            (352_800, 4, 88_200),
            (705_600, 8, 88_200),
            (1_411_200, 16, 88_200),
            (384_000, 4, 96_000),
            (2_822_400, 32, 88_200),
        ] {
            let s = FilterSpec::new(rate);
            assert_eq!((s.decimation, s.pcm_rate), (d, pcm), "{rate}");
            assert_eq!(s.taps % 8, 0);
        }
        assert_eq!(FilterSpec::new(352_800).taps, 840);
    }

    #[test]
    fn steady_streams_are_dc() {
        let mut f = DsdToPcm::new(352_800, 1);
        let mut out = Vec::new();
        f.run(0, &vec![0xff; 4096], &mut out);
        assert!((out.last().unwrap() - 1.0).abs() < 1e-5);
        out.clear();
        f.run(4096, &vec![0x00; 4096], &mut out);
        assert!((out.last().unwrap() + 1.0).abs() < 1e-5);
        out.clear();
        f.run(8192, &vec![i32::from(SILENCE); 4096], &mut out);
        assert!(out.last().unwrap().abs() < 1e-4);
    }

    #[test]
    fn sample_counts_follow_the_byte_index() {
        let mut f = DsdToPcm::new(352_800, 2);
        let mut out = Vec::new();
        // 4 per sample; bytes 0..10: samples after bytes 3 and 7.
        f.run(0, &[0x55; 20], &mut out);
        assert_eq!(out.len(), 4);
        out.clear();
        // Bytes 10..13: one after byte 11.
        f.run(10, &[0x55; 6], &mut out);
        assert_eq!(out.len(), 2);
        out.clear();
        f.run(1000, &[0x55; 2], &mut out);
        assert_eq!(out.len(), 0);
    }

    #[test]
    fn ptable_is_mirrored() {
        let t = ptable(10, RATE_S);
        for i in 0..128 {
            assert_eq!(t[i] + t[255 - i], 0x0100_ffff);
        }
        assert!(t[0] > t[127]);
    }
}
