//! Colour bars and the HiLo indicator (DESIGN.md §6.2): the numbers behind a false-colour
//! legend (its range in the data's own units and "nice" tick values) and the bit-depth limits
//! HiLo marks.

/// What a colour bar shows: the map from `lo` (map start) to `hi` (map end), in `unit`
/// (HU, counts, a physical unit when the file states one; empty for raw sample values).
#[derive(Clone, Debug, PartialEq)]
pub struct Legend {
    pub map: crate::ColourMap,
    pub lo: f64,
    pub hi: f64,
    pub unit: String,
}

impl Legend {
    /// The map position (0‥1) of `v`.
    pub fn position(&self, v: f64) -> f64 {
        if self.hi == self.lo {
            return 0.0;
        }
        ((v - self.lo) / (self.hi - self.lo)).clamp(0.0, 1.0)
    }

    /// Tick values (at most `max`, at least the two ends' neighbourhood) with their labels.
    pub fn ticks(&self, max: usize) -> Vec<(f64, String)> {
        let step = tick_step(self.lo, self.hi, max);
        ticks(self.lo, self.hi, max)
            .into_iter()
            .map(|v| (v, format_value(v, step)))
            .collect()
    }
}

/// A "nice" step (1, 2 or 5 × 10ⁿ) that puts at most `max` ticks between `lo` and `hi`.
pub fn tick_step(lo: f64, hi: f64, max: usize) -> f64 {
    let span = (hi - lo).abs();
    if !span.is_finite() || span == 0.0 || max < 2 {
        return 0.0;
    }
    let raw = span / (max - 1) as f64;
    let p = 10f64.powf(raw.log10().floor());
    for m in [1.0, 2.0, 5.0, 10.0] {
        let s = m * p;
        if ((span / s + 1e-9).floor() as usize) < max {
            return s;
        }
    }
    10.0 * p
}

/// The multiples of [`tick_step`] within `lo..=hi`, ascending. A degenerate range gives its
/// single value.
pub fn ticks(lo: f64, hi: f64, max: usize) -> Vec<f64> {
    if !lo.is_finite() || !hi.is_finite() {
        return Vec::new();
    }
    let (a, b) = (lo.min(hi), lo.max(hi));
    let step = tick_step(a, b, max);
    if step == 0.0 {
        return vec![a];
    }
    let first = (a / step - 1e-9).ceil() as i64;
    let last = (b / step + 1e-9).floor() as i64;
    (first..=last)
        .map(|k| {
            let v = k as f64 * step;
            // Exact zero, not −0 or 1e-17.
            if v.abs() < step * 1e-9 { 0.0 } else { v }
        })
        .collect()
}

/// `v` with as many decimals as `step` needs (none for integral steps).
pub fn format_value(v: f64, step: f64) -> String {
    let decimals = if step <= 0.0 || step >= 1.0 {
        0
    } else {
        (-step.log10().floor()) as usize
    };
    let s = format!("{v:.decimals$}");
    // No "-0".
    if s.trim_start_matches('-')
        .chars()
        .all(|c| c == '0' || c == '.')
    {
        s.trim_start_matches('-').to_owned()
    } else {
        s
    }
}

/// The real bit depth's limits for HiLo (DICOM BitsStored, a TIFF's BitsPerSample, PNG's
/// sBIT): `bits` significant bits, unsigned or two's complement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BitRange {
    pub bits: u8,
    pub signed: bool,
}

impl BitRange {
    pub fn new(bits: u8, signed: bool) -> Self {
        BitRange {
            bits: bits.clamp(1, 32),
            signed,
        }
    }

    /// The smallest stored value (0, or −2^(bits−1)).
    pub fn min(self) -> i64 {
        if self.signed {
            -(1i64 << (self.bits - 1))
        } else {
            0
        }
    }

    /// The largest stored value (2^bits − 1, or 2^(bits−1) − 1): 4095 for 12 bits.
    pub fn max(self) -> i64 {
        if self.signed {
            (1i64 << (self.bits - 1)) - 1
        } else {
            (1i64 << self.bits) - 1
        }
    }

    /// The right shift that leaves a 16-bit container code's significant bits. Decoders
    /// widen deeper-than-8-bit samples to 16 bits (by scaling or by shifting left), so a
    /// sample is at the maximum exactly when its top `bits` bits are all ones and at the
    /// minimum when they are all zero ([`BitRange::is_high16`], the convert shader's rule).
    pub fn shift16(self) -> u32 {
        16 - self.bits.min(16) as u32
    }

    /// Whether a widened unsigned 16-bit code is at the bit depth's maximum.
    pub fn is_high16(self, v: u16) -> bool {
        let top = (1u32 << self.bits.min(16)) - 1;
        (v as u32 >> self.shift16()) == top
    }

    /// Whether a widened unsigned 16-bit code is at the bit depth's minimum.
    pub fn is_low16(self, v: u16) -> bool {
        (v as u32 >> self.shift16()) == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ColourMap;

    #[test]
    fn ticks_are_nice_and_inside() {
        assert_eq!(
            ticks(0.0, 4095.0, 6),
            vec![0.0, 1000.0, 2000.0, 3000.0, 4000.0]
        );
        assert_eq!(
            ticks(-1000.0, 3000.0, 6),
            vec![-1000.0, 0.0, 1000.0, 2000.0, 3000.0]
        );
        assert_eq!(ticks(-160.0, 240.0, 5), vec![-100.0, 0.0, 100.0, 200.0]);
        assert_eq!(
            ticks(0.0, 1.0, 6),
            vec![0.0, 0.2, 0.4, 0.6000000000000001, 0.8, 1.0]
        );
        assert_eq!(
            ticks(0.0, 255.0, 6),
            vec![0.0, 50.0, 100.0, 150.0, 200.0, 250.0]
        );
        assert_eq!(ticks(7.0, 7.0, 6), vec![7.0]);
        assert!(ticks(f64::NAN, 1.0, 6).is_empty());
        for (lo, hi, n) in [(3.0, 97.0, 5), (-0.013, 0.021, 4), (12.0, 65535.0, 7)] {
            let t = ticks(lo, hi, n);
            assert!(!t.is_empty() && t.len() <= n, "{lo} {hi}: {t:?}");
            assert!(t.iter().all(|&v| v >= lo - 1e-12 && v <= hi + 1e-12));
        }
    }

    #[test]
    fn values_format_to_the_step() {
        assert_eq!(format_value(1000.0, 1000.0), "1000");
        assert_eq!(format_value(0.6000000000000001, 0.2), "0.6");
        assert_eq!(format_value(-0.0, 0.5), "0.0");
        assert_eq!(format_value(0.025, 0.005), "0.025");
        let l = Legend {
            map: ColourMap::Viridis,
            lo: -1000.0,
            hi: 3000.0,
            unit: "HU".into(),
        };
        let t = l.ticks(6);
        assert_eq!(t[0], (-1000.0, "-1000".into()));
        assert_eq!(l.position(1000.0), 0.5);
        assert_eq!(l.position(-5000.0), 0.0);
    }

    #[test]
    fn bit_ranges() {
        let b12 = BitRange::new(12, false);
        assert_eq!((b12.min(), b12.max()), (0, 4095));
        let s16 = BitRange::new(16, true);
        assert_eq!((s16.min(), s16.max()), (-32768, 32767));
        assert_eq!(BitRange::new(8, false).max(), 255);
        // Widened 12-bit samples: scaled (4095 → 65535) or shifted (4095 → 65520).
        assert!(b12.is_high16(65535) && b12.is_high16(65520));
        assert!(!b12.is_high16(65519) && !b12.is_high16((65535u32 * 4094 / 4095) as u16));
        assert!(b12.is_low16(0) && b12.is_low16(15) && !b12.is_low16(16));
        let b16 = BitRange::new(16, false);
        assert!(b16.is_high16(65535) && !b16.is_high16(65534));
        assert!(b16.is_low16(0) && !b16.is_low16(1));
    }
}
