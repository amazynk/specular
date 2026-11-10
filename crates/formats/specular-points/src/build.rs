//! Turning decoded points into a [`PointCloud`]: colour depth, statistics for the info panel
//! and the view (classes, intensity range), and the octree.

use rayon::prelude::*;

use crate::{Bounds, CloudInfo, Error, PointCloud, Result, lod};

/// How colours are packed: LAS stores 16-bit channels, but many writers put 8-bit values in
/// them unscaled.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Colour {
    has: bool,
    shift: u32,
}

impl Colour {
    pub(crate) fn of(has: bool, sample: impl Iterator<Item = [u16; 3]>) -> Colour {
        let max = sample.flat_map(|c| c.into_iter()).max().unwrap_or(0);
        Colour {
            has,
            shift: if max > 255 { 8 } else { 0 },
        }
    }

    #[inline]
    pub(crate) fn pack(&self, c: [u16; 3]) -> u32 {
        if !self.has {
            return 0;
        }
        let s = self.shift;
        let ch = |v: u16| u32::from((v >> s).min(255) as u8);
        ch(c[0]) | (ch(c[1]) << 8) | (ch(c[2]) << 16)
    }
}

/// The 1st and 99th percentile of intensities (the colour ramp's ends).
pub(crate) fn intensity_range(values: impl Iterator<Item = u16>) -> [u16; 2] {
    let mut hist = vec![0u32; 65_536];
    let mut n = 0u64;
    for v in values {
        hist[usize::from(v)] += 1;
        n += 1;
    }
    range_of(&hist, n)
}

fn range_of(hist: &[u32], n: u64) -> [u16; 2] {
    if n == 0 {
        return [0, 65_535];
    }
    let (lo_at, hi_at) = (n / 100, n - n / 100 - 1);
    let (mut lo, mut hi) = (0u16, 65_535u16);
    let mut sum = 0u64;
    let mut found_lo = false;
    for (v, &c) in hist.iter().enumerate() {
        let next = sum + u64::from(c);
        if !found_lo && next > lo_at {
            lo = v as u16;
            found_lo = true;
        }
        if next > hi_at {
            hi = v as u16;
            break;
        }
        sum = next;
    }
    if hi <= lo {
        hi = lo.saturating_add(1);
    }
    [lo, hi]
}

/// The finished cloud from its points.
pub(crate) fn finish(
    mut positions: Vec<[f32; 3]>,
    mut attrs: Vec<[u32; 2]>,
    mut info: CloudInfo,
    origin: [f64; 3],
) -> Result<PointCloud> {
    if positions.is_empty() {
        return Err(Error::Empty);
    }
    let bounds = positions
        .par_chunks(1 << 16)
        .map(|c| {
            let mut b = Bounds::EMPTY;
            for &p in c {
                b.add(p);
            }
            b
        })
        .reduce(|| Bounds::EMPTY, |a, b| a.union(&b));
    let (classes, ihist) = attrs
        .par_chunks(1 << 16)
        .fold(
            || (vec![0u64; 256], vec![0u32; 65_536]),
            |(mut c, mut h), chunk| {
                for a in chunk {
                    c[(a[0] >> 24) as usize] += 1;
                    h[(a[1] & 0xFFFF) as usize] += 1;
                }
                (c, h)
            },
        )
        .reduce(
            || (vec![0u64; 256], vec![0u32; 65_536]),
            |(mut c, mut h), (c2, h2)| {
                c.iter_mut().zip(c2).for_each(|(a, b)| *a += b);
                h.iter_mut()
                    .zip(h2)
                    .for_each(|(a, b)| *a = a.saturating_add(b));
                (c, h)
            },
        );
    if info.channels.classification {
        info.classes = classes
            .iter()
            .enumerate()
            .filter(|(_, n)| **n > 0)
            .map(|(c, n)| (c as u8, *n))
            .collect();
    }
    if info.channels.intensity {
        info.intensity_range = range_of(&ihist, positions.len() as u64);
    }
    info.loaded = positions.len() as u64;
    let leaves = lod::build(&mut positions, &mut attrs);
    Ok(PointCloud {
        origin,
        positions,
        attrs,
        bounds,
        leaves,
        info,
    })
}

/// Collects points of formats that give coordinates as floating point, relative to the first
/// point, keeping an even subsample of at most about `max` points (halving the kept points
/// and doubling the stride whenever they reach twice `max`).
pub(crate) struct Collector {
    pub(crate) origin: Option<[f64; 3]>,
    pub(crate) positions: Vec<[f32; 3]>,
    pub(crate) attrs: Vec<[u32; 2]>,
    stride: u64,
    seen: u64,
    max: usize,
}

impl Collector {
    /// For about `expected` points.
    pub(crate) fn new(max: u64, expected: u64) -> Collector {
        let max = usize::try_from(max.max(1)).unwrap_or(usize::MAX);
        let stride = expected.div_ceil(max as u64).max(1);
        let cap = (expected / stride).min(max as u64).min(1 << 20) as usize;
        Collector {
            origin: None,
            positions: Vec::with_capacity(cap),
            attrs: Vec::with_capacity(cap),
            stride,
            seen: 0,
            max,
        }
    }

    /// Points offered so far (kept or not).
    #[cfg(test)]
    pub(crate) fn seen(&self) -> u64 {
        self.seen
    }

    #[inline]
    pub(crate) fn push(
        &mut self,
        p: [f64; 3],
        rgb: [u8; 3],
        class: u8,
        intensity: u16,
        returns: u8,
    ) {
        if !p.iter().all(|v| v.is_finite()) {
            return;
        }
        let keep = self.seen.is_multiple_of(self.stride);
        self.seen += 1;
        if !keep {
            return;
        }
        let o = *self
            .origin
            .get_or_insert([p[0].round(), p[1].round(), p[2].round()]);
        self.positions.push([
            (p[0] - o[0]) as f32,
            (p[1] - o[1]) as f32,
            (p[2] - o[2]) as f32,
        ]);
        self.attrs.push([
            u32::from(rgb[0])
                | (u32::from(rgb[1]) << 8)
                | (u32::from(rgb[2]) << 16)
                | (u32::from(class) << 24),
            u32::from(intensity) | (u32::from(returns) << 16),
        ]);
        if self.positions.len() >= self.max.saturating_mul(2) {
            let mut i = 0;
            self.positions.retain(|_| {
                i += 1;
                i % 2 == 1
            });
            let mut i = 0;
            self.attrs.retain(|_| {
                i += 1;
                i % 2 == 1
            });
            self.stride *= 2;
        }
    }

    /// The cloud, with `info`'s `total` set to the points offered.
    pub(crate) fn finish(self, mut info: CloudInfo) -> Result<PointCloud> {
        info.total = info.total.max(self.seen);
        build_finish(self, info)
    }
}

fn build_finish(c: Collector, info: CloudInfo) -> Result<PointCloud> {
    let origin = c.origin.unwrap_or_default();
    finish(c.positions, c.attrs, info, origin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles() {
        let r = intensity_range((0..1000u16).map(|i| i * 10));
        assert_eq!(r, [100, 9890]);
        assert_eq!(intensity_range(std::iter::empty()), [0, 65_535]);
        assert_eq!(intensity_range([7u16; 50].into_iter()), [7, 8]);
    }

    #[test]
    fn colour_depth() {
        let c = Colour::of(true, [[255u16, 3, 0]].into_iter());
        assert_eq!(c.pack([255, 3, 0]), 0x00_03_FF);
        let c = Colour::of(true, [[65535u16, 256, 0]].into_iter());
        assert_eq!(c.pack([65535, 256, 0]), 0x00_01_FF);
        assert_eq!(Colour::of(false, std::iter::empty()).pack([9, 9, 9]), 0);
    }

    #[test]
    fn collector_decimates_evenly() {
        let mut c = Collector::new(100, 10);
        for i in 0..1000 {
            c.push([f64::from(i), 0.0, 0.0], [0; 3], 0, 0, 0);
        }
        assert!(c.positions.len() < 200 && c.positions.len() >= 100);
        let step = c.positions[1][0] - c.positions[0][0];
        assert!(c.positions.windows(2).all(|w| w[1][0] - w[0][0] == step));
        assert_eq!(c.seen(), 1000);
    }
}
