//! Where the samples of an image are, and reading one band of them as values.

use rayon::prelude::*;

use crate::sample::Sample;

/// How bands are interleaved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interleave {
    /// Band sequential: each band's lines in turn.
    Bsq,
    /// Band interleaved by line.
    Bil,
    /// Band interleaved by pixel.
    Bip,
}

impl Interleave {
    pub fn name(self) -> &'static str {
        match self {
            Interleave::Bsq => "BSQ",
            Interleave::Bil => "BIL",
            Interleave::Bip => "BIP",
        }
    }
}

/// Values that are not data: equal to one of `values` (stored values, before scaling), or
/// stored below `below` (ISIS special pixels, under the valid minimum).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NoData {
    pub values: Vec<f64>,
    pub below: Option<f64>,
}

impl NoData {
    pub fn is_empty(&self) -> bool {
        self.values.is_empty() && self.below.is_none()
    }

    pub fn push(&mut self, v: f64) {
        if !self
            .values
            .iter()
            .any(|&x| x == v || (x.is_nan() && v.is_nan()))
        {
            self.values.push(v);
        }
    }

    #[inline]
    fn matches(&self, v: f64) -> bool {
        self.values
            .iter()
            .any(|&x| x == v || (x.is_nan() && v.is_nan()))
            || self.below.is_some_and(|b| v < b)
    }
}

/// The byte of band `b`, line `y`, sample `x` is
/// `offset + b·band_stride + y·line_stride + x·sample_stride`; values are
/// `stored · scale + add`. Lines and samples are flipped when the file stores them against
/// the display direction.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub width: u32,
    pub height: u32,
    pub bands: u32,
    pub sample: Sample,
    pub interleave: Interleave,
    pub offset: u64,
    pub band_stride: u64,
    pub line_stride: u64,
    pub sample_stride: u64,
    pub scale: f64,
    pub add: f64,
    pub nodata: NoData,
    /// Show the last stored line at the top.
    pub flip_y: bool,
    /// Show the last stored sample at the left.
    pub flip_x: bool,
}

impl Layout {
    /// The layout of an interleave with `prefix` and `suffix` bytes around each stored line
    /// (of one band, for BSQ; of all bands, for BIL and BIP).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        width: u32,
        height: u32,
        bands: u32,
        sample: Sample,
        interleave: Interleave,
        offset: u64,
        prefix: u64,
        suffix: u64,
    ) -> Layout {
        let s = sample.bytes() as u64;
        let (w, h, n) = (u64::from(width), u64::from(height), u64::from(bands));
        let line = |samples: u64| {
            prefix
                .saturating_add(suffix)
                .saturating_add(samples.saturating_mul(s))
        };
        // Saturating: a stride past u64 lies beyond any file, so reads come back truncated.
        let (band_stride, line_stride, sample_stride) = match interleave {
            Interleave::Bsq => (h.saturating_mul(line(w)), line(w), s),
            Interleave::Bil => (w.saturating_mul(s), line(w.saturating_mul(n)), s),
            Interleave::Bip => (s, line(w.saturating_mul(n)), n.saturating_mul(s)),
        };
        Layout {
            width,
            height,
            bands,
            sample,
            interleave,
            offset: offset.saturating_add(prefix),
            band_stride,
            line_stride,
            sample_stride,
            scale: 1.0,
            add: 0.0,
            nodata: NoData::default(),
            flip_y: false,
            flip_x: false,
        }
    }

    /// One past the last byte read (saturating).
    pub fn end(&self) -> u64 {
        if self.width == 0 || self.height == 0 || self.bands == 0 {
            return self.offset;
        }
        self.offset
            .saturating_add(u64::from(self.bands - 1).saturating_mul(self.band_stride))
            .saturating_add(u64::from(self.height - 1).saturating_mul(self.line_stride))
            .saturating_add(u64::from(self.width - 1).saturating_mul(self.sample_stride))
            .saturating_add(self.sample.bytes() as u64)
    }

    /// Every `step`-th line and sample (a quick rendition of a big image).
    pub fn subsampled(&self, step: u32) -> Layout {
        let step = step.max(1);
        let mut l = self.clone();
        l.width = self.width.div_ceil(step);
        l.height = self.height.div_ceil(step);
        l.line_stride = self.line_stride.saturating_mul(u64::from(step));
        l.sample_stride = self.sample_stride.saturating_mul(u64::from(step));
        l
    }

    /// Whether values are scaled.
    pub fn scaled(&self) -> bool {
        self.scale != 1.0 || self.add != 0.0
    }

    /// Band `band` as values in display order (top line first); no-data and non-finite values
    /// are NaN. Lines past the end of `data` are NaN too, and the result says whether any were.
    pub fn band(&self, data: &[u8], band: u32) -> (Vec<f32>, bool) {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut out = vec![f32::NAN; w * h];
        if band >= self.bands || w == 0 {
            return (out, true);
        }
        let s = self.sample.bytes();
        let base = u64::from(band)
            .checked_mul(self.band_stride)
            .and_then(|b| b.checked_add(self.offset));
        let span = (w as u64 - 1)
            .checked_mul(self.sample_stride)
            .and_then(|v| v.checked_add(s as u64));
        let ss = self.sample_stride as usize;
        let rows: Vec<bool> = out
            .par_chunks_exact_mut(w)
            .enumerate()
            .map(|(y, row)| {
                let stored = if self.flip_y { h - 1 - y } else { y };
                let range = (stored as u64)
                    .checked_mul(self.line_stride)
                    .zip(base)
                    .and_then(|(r, b)| r.checked_add(b))
                    .zip(span)
                    .and_then(|(a, n)| Some((a, a.checked_add(n)?)))
                    .filter(|&(_, last)| last <= data.len() as u64);
                let Some((start, last)) = range else {
                    return true;
                };
                let r = &data[start as usize..last as usize];
                for (x, v) in row.iter_mut().enumerate() {
                    let sx = if self.flip_x { w - 1 - x } else { x };
                    let o = sx * ss;
                    let f = self.sample.read(&r[o..o + s]);
                    *v = if self.nodata.matches(f) || !f.is_finite() {
                        f32::NAN
                    } else {
                        (f * self.scale + self.add) as f32
                    };
                }
                false
            })
            .collect();
        let truncated = rows.into_iter().any(|t| t);
        (out, truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interleaves_with_prefixes() {
        let s = Sample::Int {
            bytes: 2,
            signed: false,
            big_endian: true,
        };
        // 3 × 2, 2 bands, value = 100·band + 10·line + sample, 2 prefix and 1 suffix byte.
        for il in [Interleave::Bsq, Interleave::Bil, Interleave::Bip] {
            let l = Layout::new(3, 2, 2, s, il, 5, 2, 1);
            let mut d = vec![0xeeu8; l.end() as usize + 1];
            for b in 0..2u64 {
                for y in 0..2u64 {
                    for x in 0..3u64 {
                        let o = (l.offset
                            + b * l.band_stride
                            + y * l.line_stride
                            + x * l.sample_stride) as usize;
                        d[o..o + 2].copy_from_slice(&((100 * b + 10 * y + x) as u16).to_be_bytes());
                    }
                }
            }
            let (v, t) = l.band(&d, 1);
            assert!(!t, "{il:?}");
            assert_eq!(v, [100.0, 101.0, 102.0, 110.0, 111.0, 112.0], "{il:?}");
            let mut f = l.clone();
            f.flip_y = true;
            f.flip_x = true;
            f.scale = 2.0;
            f.add = 1.0;
            f.nodata.push(111.0);
            let (v, _) = f.band(&d, 1);
            assert_eq!(v[0], 225.0);
            assert!(v[1].is_nan());
            let (_, t) = l.band(&d[..12], 1);
            assert!(t);
        }
    }

    #[test]
    fn hostile_strides() {
        let mut l = Layout::new(4, 4, 2, Sample::U8, Interleave::Bsq, u64::MAX - 3, 0, 0);
        assert_eq!(l.end(), u64::MAX);
        let (v, t) = l.band(&[0u8; 64], 1);
        assert!(t && v.iter().all(|v| v.is_nan()));
        l.offset = 0;
        l.line_stride = u64::MAX / 2;
        assert!(l.band(&[0u8; 64], 0).1);
        let big = Layout::new(u32::MAX, 2, u32::MAX, Sample::U8, Interleave::Bip, 0, 0, 0);
        assert_eq!(big.end(), u64::MAX);
    }
}
