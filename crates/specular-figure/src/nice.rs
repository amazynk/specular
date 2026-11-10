//! Round numbers for scale bars and colour-bar ticks.

/// The largest 1, 2 or 5 × 10ⁿ at or below `v` (positive, finite); `None` otherwise.
pub fn floor_125(v: f64) -> Option<f64> {
    if !(v.is_finite() && v > 0.0) {
        return None;
    }
    let e = v.log10().floor();
    let base = 10f64.powf(e);
    let m = v / base;
    // Guard the edges of the decade against rounding (4.999999 → 2, 5.0000001 → 5).
    let k = if m >= 5.0 - 1e-9 {
        5.0
    } else if m >= 2.0 - 1e-9 {
        2.0
    } else {
        1.0
    };
    Some(k * base)
}

/// A length unit a scale bar can be labelled in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LengthUnit {
    pub symbol: &'static str,
    pub metres: f64,
}

/// SI lengths from nanometres to kilometres.
pub const METRIC: [LengthUnit; 6] = [
    LengthUnit {
        symbol: "nm",
        metres: 1e-9,
    },
    LengthUnit {
        symbol: "µm",
        metres: 1e-6,
    },
    LengthUnit {
        symbol: "mm",
        metres: 1e-3,
    },
    LengthUnit {
        symbol: "cm",
        metres: 1e-2,
    },
    LengthUnit {
        symbol: "m",
        metres: 1.0,
    },
    LengthUnit {
        symbol: "km",
        metres: 1e3,
    },
];

/// A scale bar's length: the round value and the unit it is written in, and its length in
/// the unit the calibration is in.
#[derive(Clone, Debug, PartialEq)]
pub struct BarLength {
    /// The number on the label (1, 2 or 5 × 10ⁿ).
    pub value: f64,
    pub unit: String,
    /// Bar length in figure px.
    pub px: f64,
}

/// The scale bar for a panel `panel_px` wide at `per_px` units of length per px, aimed at
/// about `fraction` of the panel. Metric lengths (`metres` per unit) pick the SI unit that keeps
/// the number between 1 and 999 (centimetres only for 1–9 cm); other units keep `unit`.
pub fn bar(
    per_px: f64,
    unit: &str,
    metres: Option<f64>,
    panel_px: f64,
    fraction: f64,
) -> Option<BarLength> {
    if !(per_px.is_finite() && per_px > 0.0 && panel_px > 0.0) {
        return None;
    }
    let target = panel_px * fraction * per_px;
    match metres {
        Some(m) => {
            let target_m = target * m;
            let len_m = floor_125(target_m)?;
            let u = METRIC
                .iter()
                .rev()
                .find(|u| {
                    let v = len_m / u.metres;
                    v >= 1.0 - 1e-9 && (u.symbol != "cm" || v < 10.0 - 1e-9)
                })
                .unwrap_or(&METRIC[0]);
            let value = round_sig(len_m / u.metres);
            Some(BarLength {
                value,
                unit: u.symbol.to_owned(),
                px: len_m / m / per_px,
            })
        }
        None => {
            let v = floor_125(target)?;
            Some(BarLength {
                value: v,
                unit: unit.to_owned(),
                px: v / per_px,
            })
        }
    }
}

/// `v` to 12 significant digits (drops binary noise such as 49.99999999).
fn round_sig(v: f64) -> f64 {
    if v == 0.0 || !v.is_finite() {
        return v;
    }
    let e = v.abs().log10().floor() as i32;
    let k = 10f64.powi(11 - e);
    (v * k).round() / k
}

/// A number as a label: no trailing zeros, no exponent for ordinary magnitudes.
pub fn label(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let a = v.abs();
    if !(1e-4..1e7).contains(&a) {
        return format!("{v:e}");
    }
    let mut s = format!("{:.6}", round_sig(v));
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" { "0".into() } else { s }
}

/// Tick values for a colour bar from `lo` to `hi` (either order): round steps (1, 2 or
/// 5 × 10ⁿ) giving at most `max` ticks inside the range, ends included when they are round.
pub fn ticks(lo: f64, hi: f64, max: usize) -> Vec<f64> {
    let (a, b) = if lo <= hi { (lo, hi) } else { (hi, lo) };
    if !(a.is_finite() && b.is_finite()) || max < 2 {
        return Vec::new();
    }
    if b - a <= 0.0 {
        return vec![a];
    }
    let mut step = floor_125((b - a) / (max - 1) as f64).unwrap_or(1.0);
    loop {
        let first = (a / step - 1e-9).ceil();
        let last = (b / step + 1e-9).floor();
        let n = (last - first) as i64 + 1;
        if n as usize <= max || n <= 2 {
            return (0..n.max(0))
                .map(|i| round_sig((first + i as f64) * step))
                .map(|v| if v == 0.0 { 0.0 } else { v })
                .collect();
        }
        step = next_125(step);
    }
}

/// The next 1–2–5 step up.
fn next_125(s: f64) -> f64 {
    let e = s.log10().floor();
    let base = 10f64.powf(e);
    let m = (s / base).round();
    if m < 2.0 {
        2.0 * base
    } else if m < 5.0 {
        5.0 * base
    } else {
        10.0 * base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_two_five() {
        assert_eq!(floor_125(7.3), Some(5.0));
        assert_eq!(floor_125(2.0), Some(2.0));
        assert_eq!(floor_125(0.0499), Some(0.02));
        assert_eq!(floor_125(1999.0), Some(1000.0));
        assert_eq!(floor_125(0.0), None);
        assert_eq!(floor_125(f64::NAN), None);
    }

    #[test]
    fn bars_pick_round_lengths_and_units() {
        // 0.5 µm per px, 1000 px: 20% is 100 µm.
        let b = bar(0.5e-3, "mm", Some(1e-3), 1000.0, 0.2).unwrap();
        assert_eq!((b.value, b.unit.as_str()), (100.0, "µm"));
        assert!((b.px - 200.0).abs() < 1e-9);
        // CT: 0.7 mm per px, 512 px: 20% is 71.7 mm → 50 mm.
        let b = bar(0.7, "mm", Some(1e-3), 512.0, 0.2).unwrap();
        assert_eq!((b.value, b.unit.as_str()), (5.0, "cm"));
        assert!((b.px - 50.0 / 0.7).abs() < 1e-9);
        // 2 mm per px, 1000 px: 400 mm → 20 cm is past centimetres' range: 200 mm.
        let b = bar(2.0, "mm", Some(1e-3), 1000.0, 0.2).unwrap();
        assert_eq!((b.value, b.unit.as_str()), (200.0, "mm"));
        // Arcseconds stay arcseconds.
        let b = bar(0.3, "″", None, 800.0, 0.25).unwrap();
        assert_eq!((b.value, b.unit.as_str()), (50.0, "″"));
        assert!((b.px - 50.0 / 0.3).abs() < 1e-9);
        assert_eq!(bar(0.0, "mm", Some(1e-3), 100.0, 0.2), None);
    }

    #[test]
    fn tick_values() {
        assert_eq!(
            ticks(0.0, 255.0, 6),
            vec![0.0, 50.0, 100.0, 150.0, 200.0, 250.0]
        );
        assert_eq!(ticks(-160.0, 240.0, 5), vec![-100.0, 0.0, 100.0, 200.0]);
        assert_eq!(ticks(240.0, -160.0, 5), vec![-100.0, 0.0, 100.0, 200.0]);
        assert_eq!(ticks(0.1, 0.35, 6), vec![0.1, 0.15, 0.2, 0.25, 0.3, 0.35]);
        assert_eq!(ticks(3.0, 3.0, 5), vec![3.0]);
        assert!(ticks(0.0, f64::INFINITY, 5).is_empty());
    }

    #[test]
    fn labels() {
        assert_eq!(label(50.0), "50");
        assert_eq!(label(0.15000000000000002), "0.15");
        assert_eq!(label(-0.0), "0");
        assert_eq!(label(1250.5), "1250.5");
        assert_eq!(label(2e-7), "2e-7");
    }
}
