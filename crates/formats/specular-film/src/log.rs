//! Code-value lookup tables: printing-density log to linear, linear, and display-coded.

/// The printing-density conversion's parameters, in 10-bit code values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LogParams {
    /// Reference white and black code values (685 and 95 by default).
    pub white: f32,
    pub black: f32,
    /// Negative film gamma (0.6).
    pub gamma: f32,
    /// Density per 10-bit code value (0.002).
    pub density: f32,
}

impl Default for LogParams {
    fn default() -> Self {
        LogParams {
            white: 685.0,
            black: 95.0,
            gamma: 0.6,
            density: 0.002,
        }
    }
}

impl LogParams {
    /// Linear light of 10-bit code value `c` (fractional for other depths).
    pub fn linear(&self, c: f32) -> f32 {
        let k = self.density / self.gamma;
        let offset = 10f32.powf((self.black - self.white) * k);
        let v = 10f32.powf((c - self.white) * k);
        (v - offset) / (1.0 - offset)
    }
}

/// A table of every code value of `bits` (≤ 16): log to linear `f32`.
pub(crate) fn log_table(bits: u8, p: &LogParams) -> Vec<f32> {
    let max = ((1u32 << bits) - 1) as f32;
    (0..1u32 << bits)
        .map(|c| p.linear(c as f32 * 1023.0 / max))
        .collect()
}

/// Code value → `(c − lo) / (hi − lo)` as `f32` (not clamped).
pub(crate) fn linear_table(bits: u8, lo: f32, hi: f32) -> Vec<f32> {
    let span = (hi - lo).max(1e-6);
    (0..1u32 << bits).map(|c| (c as f32 - lo) / span).collect()
}

/// Code value → display 16-bit, stretched so `lo` is 0 and `hi` 65535 (clamped).
pub(crate) fn display_table(bits: u8, lo: f32, hi: f32) -> Vec<u16> {
    let span = (hi - lo).max(1e-6);
    (0..1u32 << bits)
        .map(|c| (((c as f32 - lo) / span).clamp(0.0, 1.0) * 65535.0 + 0.5) as u16)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_points() {
        let p = LogParams::default();
        assert!(p.linear(95.0).abs() < 1e-6);
        assert!((p.linear(685.0) - 1.0).abs() < 1e-5);
        let top = p.linear(1023.0);
        assert!(top > 13.0 && top < 14.0, "{top}");
        let t = log_table(10, &p);
        assert_eq!(t.len(), 1024);
        assert!((t[685] - 1.0).abs() < 1e-5);
    }
}
