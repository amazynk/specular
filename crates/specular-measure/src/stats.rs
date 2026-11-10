//! Region statistics per channel: count, mean, standard deviation, minimum, maximum, sum,
//! median and histogram, over the pixels a shape covers ([`crate::cover`]).
//!
//! 8- and 16-bit data are counted exactly (a count per value), so every statistic is exact:
//! the sums are integers and the variance is `(n·Σv² − (Σv)²) / n²` in 128-bit integers.
//! Floating-point data is summed with compensation (Neumaier) and the variance is a second
//! pass about the mean. Its median is exact: a radix selection on the samples' total order,
//! 16 bits per pass, so nothing is gathered or sorted. NaN samples are left out and counted
//! in [`ChannelStats::nan`].
//!
//! Large regions run in parallel on rayon's global pool (named by `specular-thread`). The
//! work is cut into pieces of a fixed number of pixels and partial sums are combined in
//! order, so the results never depend on the number of threads.
//!
//! The median of an even count is the mean of the two middle values.

use std::ops::Range;

use rayon::prelude::*;

use crate::cover;
use crate::geom::Shape;
use crate::raster::{Samples, View};

/// The default number of histogram bins.
pub const DEFAULT_BINS: usize = 256;

/// Pixels per work item of the parallel passes; regions of at most this many pixels run on
/// the calling thread.
const CHUNK: usize = 1 << 18;

/// Bits of a key resolved per radix-selection pass.
const DIGIT: u32 = 16;
const DIGITS: usize = 1 << DIGIT;

/// Bin `i` counts values in `lo + i·width .. lo + (i + 1)·width`; the last bin also counts
/// its upper end. Integer data has whole-number bins starting at the minimum (one value per
/// bin when the range fits); a constant region has one bin of width 0.
#[derive(Clone, Debug, PartialEq)]
pub struct Histogram {
    pub lo: f64,
    pub width: f64,
    pub bins: Vec<u64>,
}

impl Histogram {
    /// The upper end of the last bin.
    pub fn hi(&self) -> f64 {
        self.lo + self.width * self.bins.len() as f64
    }
}

/// Statistics of one channel.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelStats {
    pub count: u64,
    pub sum: f64,
    pub mean: f64,
    /// Population standard deviation (divides by n; the DICOM viewer's convention).
    pub sd: f64,
    /// Sample standard deviation (divides by n − 1; ImageJ's convention). 0 for one pixel.
    pub sd_sample: f64,
    pub min: f64,
    pub max: f64,
    pub median: f64,
    pub histogram: Histogram,
    /// NaN samples left out.
    pub nan: u64,
}

impl ChannelStats {
    /// No values (only NaN ones, `nan` of them).
    fn empty(nan: u64) -> ChannelStats {
        ChannelStats {
            count: 0,
            sum: 0.0,
            mean: f64::NAN,
            sd: f64::NAN,
            sd_sample: f64::NAN,
            min: f64::NAN,
            max: f64::NAN,
            median: f64::NAN,
            histogram: Histogram {
                lo: 0.0,
                width: 0.0,
                bins: Vec::new(),
            },
            nan,
        }
    }
}

/// Statistics of each channel of `view` over the pixels `shape` covers within the view, or
/// `None` when it covers none (or is not an area or point).
pub fn roi(shape: &Shape, view: &View<'_>, bins: usize) -> Option<Vec<ChannelStats>> {
    let bins = bins.max(1);
    let ch = view.channels as usize;
    let clip = view.extent();
    let (ox, oy) = (view.origin.0 as usize, view.origin.1 as usize);
    let mut work = Work::new(ch);
    cover::spans(shape, clip, |y, a, b| {
        let start = (y as usize - oy) * view.stride + (a as usize - ox) * ch;
        work.push(start..start + (b - a) as usize * ch);
    });
    let work = work.finish();
    let out = match view.data {
        Samples::U8(d) => counted(d, ch, 256, &work, bins),
        Samples::U16(d) => counted(d, ch, 65536, &work, bins),
        Samples::F32(d) => floats(d, ch, &work, bins),
        Samples::F64(d) => floats(d, ch, &work, bins),
    }?;
    (out.first()?.count + out.first()?.nan > 0).then_some(out)
}

/// Statistics of `value(x, y)` over the pixels of a `width`×`height` image that `shape`
/// covers, `None` values left out (as the modality values of a DICOM frame are read).
pub fn roi_with(
    shape: &Shape,
    width: u32,
    height: u32,
    bins: usize,
    mut value: impl FnMut(u32, u32) -> Option<f64>,
) -> Option<ChannelStats> {
    let mut vals = Vec::new();
    cover::spans(shape, (0, 0, width, height), |y, a, b| {
        vals.extend((a..b).filter_map(|x| value(x, y)));
    });
    of_values(vals, bins)
}

/// Statistics of a list of values.
pub fn of_values(values: Vec<f64>, bins: usize) -> Option<ChannelStats> {
    let mut work = Work::new(1);
    work.push(0..values.len());
    floats(&values, 1, &work.finish(), bins.max(1))?.pop()
}

/// The covered samples as ranges of the data, cut into work items of [`CHUNK`] pixels.
struct Work {
    ch: usize,
    pieces: Vec<Range<usize>>,
    items: Vec<Range<usize>>,
    /// Pixels in the open item.
    open: usize,
}

impl Work {
    fn new(ch: usize) -> Work {
        Work {
            ch: ch.max(1),
            pieces: Vec::new(),
            items: Vec::new(),
            open: 0,
        }
    }

    /// Adds the samples `r` (whole pixels).
    fn push(&mut self, mut r: Range<usize>) {
        while !r.is_empty() {
            let px = (CHUNK - self.open).min(r.len() / self.ch);
            if px == 0 {
                break;
            }
            let end = r.start + px * self.ch;
            self.pieces.push(r.start..end);
            r.start = end;
            self.open += px;
            if self.open == CHUNK {
                self.close();
            }
        }
    }

    fn close(&mut self) {
        let start = self.items.last().map_or(0, |i| i.end);
        if start < self.pieces.len() {
            self.items.push(start..self.pieces.len());
        }
        self.open = 0;
    }

    fn finish(mut self) -> Work {
        self.close();
        self
    }

    fn item(&self, i: usize) -> &[Range<usize>] {
        &self.pieces[self.items[i].clone()]
    }

    /// Folds `step` over every item, in parallel when there are several; `merge` must not
    /// depend on the order (integer counts, or partials tagged with their item).
    fn fold<A: Send>(
        &self,
        init: impl Fn() -> A + Sync + Send,
        step: impl Fn(&mut A, usize) + Sync + Send,
        merge: impl Fn(&mut A, A) + Sync + Send,
    ) -> A {
        let n = self.items.len();
        if n <= 1 {
            let mut a = init();
            (0..n).for_each(|i| step(&mut a, i));
            return a;
        }
        (0..n)
            .into_par_iter()
            .fold(&init, |mut a, i| {
                step(&mut a, i);
                a
            })
            .reduce(&init, |mut a, b| {
                merge(&mut a, b);
                a
            })
    }
}

/// Adds `b` into `a` element by element.
fn add_counts(a: &mut [u64], b: &[u64]) {
    for (x, y) in a.iter_mut().zip(b) {
        *x += y;
    }
}

/// Exact statistics of integer samples with `values` possible values.
fn counted<T: Copy + Into<usize> + Sync>(
    d: &[T],
    ch: usize,
    values: usize,
    work: &Work,
    bins: usize,
) -> Option<Vec<ChannelStats>> {
    let counts = work.fold(
        || vec![0u64; values * ch],
        |c, i| {
            for r in work.item(i) {
                let s = &d[r.clone()];
                if ch == 1 {
                    for &v in s {
                        c[v.into()] += 1;
                    }
                } else {
                    for px in s.chunks_exact(ch) {
                        for (k, &v) in px.iter().enumerate() {
                            c[k * values + v.into()] += 1;
                        }
                    }
                }
            }
        },
        |a, b| add_counts(a, &b),
    );
    counts
        .chunks_exact(values)
        .map(|c| from_counts(c, bins))
        .collect()
}

/// Exact statistics from a count per integer value.
fn from_counts(counts: &[u64], bins: usize) -> Option<ChannelStats> {
    let n: u64 = counts.iter().sum();
    if n == 0 {
        return Some(ChannelStats::empty(0));
    }
    let mut stats = ChannelStats {
        count: n,
        ..ChannelStats::empty(0)
    };
    let (mut sum, mut sum2) = (0u128, 0u128);
    for (v, &c) in counts.iter().enumerate() {
        let (v, c) = (v as u128, u128::from(c));
        sum += v * c;
        sum2 += v * v * c;
    }
    let lo = counts.iter().position(|&c| c > 0)?;
    let hi = counts.iter().rposition(|&c| c > 0)?;
    let nn = u128::from(n);
    // n·Σv² − (Σv)² ≥ 0 exactly.
    let spread = (nn * sum2 - sum * sum) as f64;
    let nf = n as f64;
    let kth = |k: u64| {
        let mut seen = 0u64;
        counts
            .iter()
            .position(|&c| {
                seen += c;
                seen > k
            })
            .unwrap_or(hi) as f64
    };
    stats.sum = sum as f64;
    stats.mean = sum as f64 / nf;
    stats.sd = (spread / (nf * nf)).sqrt();
    stats.sd_sample = if n > 1 {
        (spread / (nf * (nf - 1.0))).sqrt()
    } else {
        0.0
    };
    stats.min = lo as f64;
    stats.max = hi as f64;
    stats.median = if n % 2 == 1 {
        kth(n / 2)
    } else {
        (kth(n / 2 - 1) + kth(n / 2)) / 2.0
    };
    let span = hi - lo + 1;
    let width = span.div_ceil(bins);
    let mut hist = vec![0u64; span.div_ceil(width)];
    for (i, &c) in counts[lo..=hi].iter().enumerate() {
        hist[i / width] += c;
    }
    stats.histogram = Histogram {
        lo: lo as f64,
        width: width as f64,
        bins: hist,
    };
    Some(stats)
}

/// A floating-point sample type.
trait Float: Copy + Send + Sync {
    /// Bits of its key.
    const BITS: u32;
    fn f64(self) -> f64;
    fn nan(self) -> bool;
    /// An unsigned key in the order of `total_cmp`.
    fn key(self) -> u64;
    /// The value of a key.
    fn of_key(k: u64) -> f64;
}

impl Float for f32 {
    const BITS: u32 = 32;
    #[inline]
    fn f64(self) -> f64 {
        f64::from(self)
    }
    #[inline]
    fn nan(self) -> bool {
        self.is_nan()
    }
    #[inline]
    fn key(self) -> u64 {
        let b = self.to_bits();
        u64::from(if b >> 31 == 1 { !b } else { b | 1 << 31 })
    }
    fn of_key(k: u64) -> f64 {
        let k = k as u32;
        let b = if k >> 31 == 1 { k & !(1 << 31) } else { !k };
        f64::from(f32::from_bits(b))
    }
}

impl Float for f64 {
    const BITS: u32 = 64;
    #[inline]
    fn f64(self) -> f64 {
        self
    }
    #[inline]
    fn nan(self) -> bool {
        self.is_nan()
    }
    #[inline]
    fn key(self) -> u64 {
        let b = self.to_bits();
        if b >> 63 == 1 { !b } else { b | 1 << 63 }
    }
    fn of_key(k: u64) -> f64 {
        f64::from_bits(if k >> 63 == 1 { k & !(1 << 63) } else { !k })
    }
}

/// One item's first-pass partial of one channel.
#[derive(Clone, Copy, Debug)]
struct Part {
    n: u64,
    nan: u64,
    lo: f64,
    hi: f64,
    sum: f64,
    comp: f64,
}

impl Part {
    const ZERO: Part = Part {
        n: 0,
        nan: 0,
        lo: f64::INFINITY,
        hi: f64::NEG_INFINITY,
        sum: 0.0,
        comp: 0.0,
    };
}

/// Neumaier's compensated `sum += v`.
#[inline]
fn neumaier(sum: &mut f64, comp: &mut f64, v: f64) {
    let t = *sum + v;
    *comp += if sum.abs() >= v.abs() {
        (*sum - t) + v
    } else {
        (v - t) + *sum
    };
    *sum = t;
}

/// The values of channel `c` of the interleaved samples `s`.
#[inline]
fn channel<T: Copy>(s: &[T], ch: usize, c: usize) -> impl Iterator<Item = T> + '_ {
    s.iter().skip(c).step_by(ch).copied()
}

/// A median rank being resolved, 16 bits of its key per pass.
#[derive(Clone, Copy, Debug)]
struct Target {
    /// The key's top `16 · level` bits.
    prefix: u64,
    /// Its rank among the keys with that prefix.
    rank: u64,
    level: u32,
}

impl Target {
    /// Whether `key` has this prefix, and the next digit of the key.
    #[inline]
    fn digit<T: Float>(&self, key: u64) -> Option<usize> {
        let below = T::BITS - DIGIT * self.level;
        let matches = self.level == 0 || key >> below == self.prefix;
        matches.then(|| ((key >> (below - DIGIT)) as usize) & (DIGITS - 1))
    }

    /// Takes the next digit from the counts of the keys with this prefix.
    fn descend(&mut self, counts: &[u64]) {
        let mut seen = 0u64;
        for (d, &c) in counts.iter().enumerate() {
            if seen + c > self.rank {
                self.rank -= seen;
                self.prefix = (self.prefix << DIGIT) | d as u64;
                self.level += 1;
                return;
            }
            seen += c;
        }
        debug_assert!(false, "rank {} beyond the counts", self.rank);
        self.level += 1;
    }

    fn done<T: Float>(&self) -> bool {
        self.level * DIGIT >= T::BITS
    }
}

/// Statistics of floating-point samples.
fn floats<T: Float>(d: &[T], ch: usize, work: &Work, bins: usize) -> Option<Vec<ChannelStats>> {
    // Pass 1: count, NaNs, range and sum per item, and the keys' first digits.
    struct Acc1 {
        parts: Vec<(usize, Vec<Part>)>,
        counts: Vec<u64>,
    }
    let first = Target {
        prefix: 0,
        rank: 0,
        level: 0,
    };
    let acc = work.fold(
        || Acc1 {
            parts: Vec::new(),
            counts: vec![0u64; DIGITS * ch],
        },
        |a, i| {
            let mut parts = vec![Part::ZERO; ch];
            for r in work.item(i) {
                let s = &d[r.clone()];
                for (c, p) in parts.iter_mut().enumerate() {
                    let counts = &mut a.counts[c * DIGITS..(c + 1) * DIGITS];
                    let mut q = *p;
                    for v in channel(s, ch, c) {
                        if v.nan() {
                            q.nan += 1;
                            continue;
                        }
                        if let Some(dg) = first.digit::<T>(v.key()) {
                            counts[dg] += 1;
                        }
                        let x = v.f64();
                        q.n += 1;
                        q.lo = q.lo.min(x);
                        q.hi = q.hi.max(x);
                        neumaier(&mut q.sum, &mut q.comp, x);
                    }
                    *p = q;
                }
            }
            a.parts.push((i, parts));
        },
        |a, b| {
            a.parts.extend(b.parts);
            add_counts(&mut a.counts, &b.counts);
        },
    );
    let Acc1 { mut parts, counts } = acc;
    parts.sort_unstable_by_key(|p| p.0);

    struct Chan {
        n: u64,
        nan: u64,
        lo: f64,
        hi: f64,
        sum: f64,
        mean: f64,
        width: f64,
        bins: usize,
        targets: Vec<Target>,
    }
    let mut chans: Vec<Chan> = (0..ch)
        .map(|c| {
            let (mut sum, mut comp) = (0.0, 0.0);
            let mut t = Part::ZERO;
            for (_, p) in &parts {
                let p = p[c];
                t.n += p.n;
                t.nan += p.nan;
                t.lo = t.lo.min(p.lo);
                t.hi = t.hi.max(p.hi);
                neumaier(&mut sum, &mut comp, p.sum);
                comp += p.comp;
            }
            let sum = sum + comp;
            let n = t.n;
            let (width, bins) = if t.hi > t.lo {
                ((t.hi - t.lo) / bins as f64, bins)
            } else {
                (0.0, 1)
            };
            let ranks: &[u64] = match n {
                0 => &[],
                n if n % 2 == 1 => &[n / 2],
                n => &[n / 2 - 1, n / 2],
            };
            let targets = ranks
                .iter()
                .map(|&rank| {
                    let mut t = Target {
                        prefix: 0,
                        rank,
                        level: 0,
                    };
                    t.descend(&counts[c * DIGITS..(c + 1) * DIGITS]);
                    t
                })
                .collect();
            Chan {
                n,
                nan: t.nan,
                lo: t.lo,
                hi: t.hi,
                sum,
                mean: sum / n as f64,
                width,
                bins,
                targets,
            }
        })
        .collect();
    drop(counts);

    // Pass 2: squared deviations, the histogram and the median's next digits.
    struct Acc2 {
        ss: Vec<(usize, Vec<(f64, f64)>)>,
        hist: Vec<Vec<u64>>,
        sel: Vec<Vec<u64>>,
    }
    let ntargets = |chans: &[Chan]| chans.iter().map(|c| c.targets.len()).sum::<usize>();
    let nt = ntargets(&chans);
    let acc = work.fold(
        || Acc2 {
            ss: Vec::new(),
            hist: chans.iter().map(|c| vec![0u64; c.bins]).collect(),
            sel: vec![vec![0u64; DIGITS]; nt],
        },
        |a, i| {
            let mut ss = vec![(0.0, 0.0); ch];
            let mut t0 = 0;
            for (c, k) in chans.iter().enumerate() {
                let (mut s, mut comp) = (0.0, 0.0);
                let hist = &mut a.hist[c];
                let sel = &mut a.sel[t0..t0 + k.targets.len()];
                for r in work.item(i) {
                    for v in channel(&d[r.clone()], ch, c) {
                        if v.nan() {
                            continue;
                        }
                        let x = v.f64();
                        let dv = x - k.mean;
                        neumaier(&mut s, &mut comp, dv * dv);
                        let b = if k.width > 0.0 {
                            (((x - k.lo) / k.width) as usize).min(k.bins - 1)
                        } else {
                            0
                        };
                        hist[b] += 1;
                        let key = v.key();
                        for (t, counts) in k.targets.iter().zip(sel.iter_mut()) {
                            if let Some(dg) = t.digit::<T>(key) {
                                counts[dg] += 1;
                            }
                        }
                    }
                }
                ss[c] = (s, comp);
                t0 += k.targets.len();
            }
            a.ss.push((i, ss));
        },
        |a, b| {
            a.ss.extend(b.ss);
            for (x, y) in a.hist.iter_mut().zip(&b.hist) {
                add_counts(x, y);
            }
            for (x, y) in a.sel.iter_mut().zip(&b.sel) {
                add_counts(x, y);
            }
        },
    );
    let Acc2 { mut ss, hist, sel } = acc;
    ss.sort_unstable_by_key(|p| p.0);
    let mut it = sel.iter();
    for k in &mut chans {
        for t in &mut k.targets {
            if let Some(counts) = it.next() {
                t.descend(counts);
            }
        }
    }

    // Further passes for 64-bit keys: only the median's digits.
    while chans
        .iter()
        .any(|k| k.targets.iter().any(|t| !t.done::<T>()))
    {
        let nt = ntargets(&chans);
        let sel = work.fold(
            || vec![vec![0u64; DIGITS]; nt],
            |a, i| {
                let mut t0 = 0;
                for (c, k) in chans.iter().enumerate() {
                    let sel = &mut a[t0..t0 + k.targets.len()];
                    for r in work.item(i) {
                        for v in channel(&d[r.clone()], ch, c) {
                            if v.nan() {
                                continue;
                            }
                            let key = v.key();
                            for (t, counts) in k.targets.iter().zip(sel.iter_mut()) {
                                if let Some(dg) = t.digit::<T>(key) {
                                    counts[dg] += 1;
                                }
                            }
                        }
                    }
                    t0 += k.targets.len();
                }
            },
            |a, b| {
                for (x, y) in a.iter_mut().zip(&b) {
                    add_counts(x, y);
                }
            },
        );
        let mut it = sel.iter();
        for k in &mut chans {
            for t in &mut k.targets {
                if let Some(counts) = it.next() {
                    t.descend(counts);
                }
            }
        }
    }

    chans
        .into_iter()
        .zip(hist)
        .enumerate()
        .map(|(c, (k, hist))| {
            if k.n == 0 {
                return (k.nan > 0).then(|| ChannelStats::empty(k.nan));
            }
            let (mut s, mut comp) = (0.0, 0.0);
            for (_, p) in &ss {
                neumaier(&mut s, &mut comp, p[c].0);
                comp += p[c].1;
            }
            let ss = s + comp;
            let nf = k.n as f64;
            let median = match k.targets[..] {
                [t] => T::of_key(t.prefix),
                [a, b] => (T::of_key(a.prefix) + T::of_key(b.prefix)) / 2.0,
                _ => f64::NAN,
            };
            Some(ChannelStats {
                count: k.n,
                sum: k.sum,
                mean: k.mean,
                sd: (ss / nf).sqrt(),
                sd_sample: if k.n > 1 {
                    (ss / (nf - 1.0)).sqrt()
                } else {
                    0.0
                },
                min: k.lo,
                max: k.hi,
                median,
                histogram: Histogram {
                    lo: k.lo,
                    width: k.width,
                    bins: hist,
                },
                nan: k.nan,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The statistics as gathered, sorted and summed one by one.
    fn reference(vals: &[f64], bins: usize) -> Option<ChannelStats> {
        let nan = vals.iter().filter(|v| v.is_nan()).count() as u64;
        let mut v: Vec<f64> = vals.iter().copied().filter(|v| !v.is_nan()).collect();
        if v.is_empty() {
            return (nan > 0).then(|| ChannelStats::empty(nan));
        }
        v.sort_by(f64::total_cmp);
        let n = v.len();
        let (mut sum, mut comp) = (0.0, 0.0);
        v.iter().for_each(|&x| neumaier(&mut sum, &mut comp, x));
        let sum = sum + comp;
        let mean = sum / n as f64;
        let (mut ss, mut comp) = (0.0, 0.0);
        v.iter()
            .for_each(|&x| neumaier(&mut ss, &mut comp, (x - mean) * (x - mean)));
        let ss = ss + comp;
        let (lo, hi) = (v[0], v[n - 1]);
        let median = if n % 2 == 1 {
            v[n / 2]
        } else {
            (v[n / 2 - 1] + v[n / 2]) / 2.0
        };
        let (width, count) = if hi > lo {
            ((hi - lo) / bins as f64, bins)
        } else {
            (0.0, 1)
        };
        let mut h = vec![0u64; count];
        for x in &v {
            let i = if width > 0.0 {
                (((x - lo) / width) as usize).min(count - 1)
            } else {
                0
            };
            h[i] += 1;
        }
        Some(ChannelStats {
            count: n as u64,
            sum,
            mean,
            sd: (ss / n as f64).sqrt(),
            sd_sample: if n > 1 {
                (ss / (n as f64 - 1.0)).sqrt()
            } else {
                0.0
            },
            min: lo,
            max: hi,
            median,
            histogram: Histogram { lo, width, bins: h },
            nan,
        })
    }

    fn same(a: &ChannelStats, b: &ChannelStats) {
        let close = |x: f64, y: f64| x == y || (x - y).abs() <= 1e-9 * x.abs().max(y.abs());
        assert_eq!((a.count, a.nan), (b.count, b.nan));
        assert_eq!((a.min, a.max, a.median), (b.min, b.max, b.median));
        assert!(
            close(a.sum, b.sum) && close(a.mean, b.mean),
            "{a:?} vs {b:?}"
        );
        assert!(
            close(a.sd, b.sd) && close(a.sd_sample, b.sd_sample),
            "{a:?} vs {b:?}"
        );
        assert_eq!(a.histogram, b.histogram);
    }

    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 11
        }
    }

    /// Exact against sorting, for both key widths, odd and even counts, ties, signed zeros,
    /// NaNs, several channels, and regions spanning several work items.
    #[test]
    fn floats_match_the_sorted_reference() {
        let mut g = Lcg(7);
        for (case, n) in [1usize, 2, 3, 10, 1001, CHUNK + 17, 3 * CHUNK + 2]
            .into_iter()
            .enumerate()
        {
            for ch in [1usize, 3] {
                let vals: Vec<f64> = (0..n * ch)
                    .map(|i| match (g.next() % 23, case % 3) {
                        (0, _) => f64::NAN,
                        (1, _) => -0.0,
                        (2, _) => 0.0,
                        (_, 0) => (g.next() % 7) as f64 - 3.0,
                        (_, 1) => (g.next() as f64 / 9.0e15 - 0.5) * 1e6,
                        _ => f64::from((i % 97) as f32 * -1.5e-3),
                    })
                    .collect();
                let f32s: Vec<f32> = vals.iter().map(|&v| v as f32).collect();
                let mut work = Work::new(ch);
                // Uneven pieces, like rows of a shape.
                let mut at = 0;
                while at < n {
                    let len = (1 + g.next() as usize % 5000).min(n - at);
                    work.push(at * ch..(at + len) * ch);
                    at += len;
                }
                let work = work.finish();
                let a = floats(&vals, ch, &work, 64);
                let b = floats(&f32s, ch, &work, 64);
                for c in 0..ch {
                    let col: Vec<f64> = vals.iter().skip(c).step_by(ch).copied().collect();
                    let col32: Vec<f64> = f32s
                        .iter()
                        .skip(c)
                        .step_by(ch)
                        .map(|&v| f64::from(v))
                        .collect();
                    let (ra, rb) = (reference(&col, 64), reference(&col32, 64));
                    match (&a, ra) {
                        (Some(a), Some(r)) => same(&a[c], &r),
                        (None, None) => {}
                        (a, r) => panic!("{a:?} vs {r:?}"),
                    }
                    match (&b, rb) {
                        (Some(b), Some(r)) => same(&b[c], &r),
                        (None, None) => {}
                        (b, r) => panic!("{b:?} vs {r:?}"),
                    }
                }
            }
        }
    }

    #[test]
    fn keys_keep_the_total_order() {
        let vals = [
            f32::NEG_INFINITY,
            -3.5,
            -f32::MIN_POSITIVE,
            -0.0,
            0.0,
            1e-45,
            2.0,
            f32::INFINITY,
        ];
        for w in vals.windows(2) {
            assert!(w[0].key() < w[1].key(), "{w:?}");
        }
        for v in vals {
            assert_eq!(f32::of_key(v.key()).to_bits(), f64::from(v).to_bits());
            let d = f64::from(v);
            assert_eq!(f64::of_key(d.key()).to_bits(), d.to_bits());
        }
    }

    #[test]
    fn work_items_cover_every_sample_once() {
        let mut w = Work::new(3);
        w.push(0..3 * (CHUNK - 1));
        w.push(3 * CHUNK..3 * (2 * CHUNK + 5));
        let w = w.finish();
        let px: Vec<usize> = (0..w.items.len())
            .map(|i| w.item(i).iter().map(|r| r.len() / 3).sum())
            .collect();
        assert_eq!(px, vec![CHUNK, CHUNK, 4]);
        let total: usize = w.pieces.iter().map(|r| r.len()).sum();
        assert_eq!(total, 3 * (CHUNK - 1 + CHUNK + 5));
    }
}
