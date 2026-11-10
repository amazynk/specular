//! Lengths and areas in metric or imperial units (international foot and statute mile), and
//! the round lengths a scale bar shows.

/// The unit system for display.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Units {
    Metric,
    Imperial,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LengthUnit {
    M,
    Km,
    Ft,
    Mi,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AreaUnit {
    M2,
    Ha,
    Km2,
    Ft2,
    Ac,
    Mi2,
}

const FOOT: f64 = 0.3048;
const MILE: f64 = 1609.344;

impl LengthUnit {
    pub fn symbol(self) -> &'static str {
        match self {
            LengthUnit::M => "m",
            LengthUnit::Km => "km",
            LengthUnit::Ft => "ft",
            LengthUnit::Mi => "mi",
        }
    }

    /// Metres in one unit.
    pub fn metres(self) -> f64 {
        match self {
            LengthUnit::M => 1.0,
            LengthUnit::Km => 1000.0,
            LengthUnit::Ft => FOOT,
            LengthUnit::Mi => MILE,
        }
    }
}

impl AreaUnit {
    pub fn symbol(self) -> &'static str {
        match self {
            AreaUnit::M2 => "m²",
            AreaUnit::Ha => "ha",
            AreaUnit::Km2 => "km²",
            AreaUnit::Ft2 => "ft²",
            AreaUnit::Ac => "ac",
            AreaUnit::Mi2 => "mi²",
        }
    }

    /// Square metres in one unit.
    pub fn square_metres(self) -> f64 {
        match self {
            AreaUnit::M2 => 1.0,
            AreaUnit::Ha => 10_000.0,
            AreaUnit::Km2 => 1.0e6,
            AreaUnit::Ft2 => FOOT * FOOT,
            AreaUnit::Ac => 4_046.856_422_4,
            AreaUnit::Mi2 => MILE * MILE,
        }
    }
}

/// A scale bar's length: in metres, and as the number and unit written under it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScaleChoice {
    pub metres: f64,
    pub value: f64,
    pub unit: LengthUnit,
}

/// The largest of 1, 2, 5 × 10ⁿ not above `max`.
fn nice_below(max: f64) -> f64 {
    let p = 10f64.powf(max.log10().floor());
    let m = max / p;
    // Guard the rounding of log10 at exact powers.
    let k = if m >= 5.0 - 1e-9 {
        5.0
    } else if m >= 2.0 - 1e-9 {
        2.0
    } else if m >= 1.0 - 1e-9 {
        1.0
    } else {
        0.5
    };
    k * p
}

/// The longest round length not above `max_metres`: 1, 2 or 5 × 10ⁿ metres (kilometres from
/// 1 km); in imperial, feet up to 1000 ft, then ¼ and ½ mile, then 1, 2, 5 × 10ⁿ miles.
pub fn scale_bar(max_metres: f64, units: Units) -> Option<ScaleChoice> {
    if !(max_metres.is_finite() && max_metres > 0.0) {
        return None;
    }
    match units {
        Units::Metric => {
            let m = nice_below(max_metres);
            Some(if m >= 1000.0 {
                ScaleChoice {
                    metres: m,
                    value: m / 1000.0,
                    unit: LengthUnit::Km,
                }
            } else {
                ScaleChoice {
                    metres: m,
                    value: m,
                    unit: LengthUnit::M,
                }
            })
        }
        Units::Imperial => {
            let mi = max_metres / MILE;
            if mi >= 0.25 {
                let v = if mi >= 1.0 {
                    nice_below(mi)
                } else if mi >= 0.5 {
                    0.5
                } else {
                    0.25
                };
                Some(ScaleChoice {
                    metres: v * MILE,
                    value: v,
                    unit: LengthUnit::Mi,
                })
            } else {
                let ft = nice_below(max_metres / FOOT).min(1000.0);
                Some(ScaleChoice {
                    metres: ft * FOOT,
                    value: ft,
                    unit: LengthUnit::Ft,
                })
            }
        }
    }
}

/// A length for display: metres below 1 km, else kilometres; feet below 0.1 mile (528 ft),
/// else miles.
pub fn length(metres: f64, units: Units) -> (f64, LengthUnit) {
    match units {
        Units::Metric if metres.abs() < 1000.0 => (metres, LengthUnit::M),
        Units::Metric => (metres / 1000.0, LengthUnit::Km),
        Units::Imperial if metres.abs() / FOOT < 528.0 => (metres / FOOT, LengthUnit::Ft),
        Units::Imperial => (metres / MILE, LengthUnit::Mi),
    }
}

/// An area for display: m² below 1 ha, hectares below 100 ha, then km²; ft² below an
/// acre, acres below 640 (a square mile), then mi².
pub fn area(m2: f64, units: Units) -> (f64, AreaUnit) {
    let pick = |u: AreaUnit| (m2 / u.square_metres(), u);
    match units {
        Units::Metric if m2.abs() < 1.0e4 => pick(AreaUnit::M2),
        Units::Metric if m2.abs() < 1.0e6 => pick(AreaUnit::Ha),
        Units::Metric => pick(AreaUnit::Km2),
        Units::Imperial if m2.abs() < AreaUnit::Ac.square_metres() => pick(AreaUnit::Ft2),
        Units::Imperial if m2.abs() < 640.0 * AreaUnit::Ac.square_metres() => pick(AreaUnit::Ac),
        Units::Imperial => pick(AreaUnit::Mi2),
    }
}

/// A value with three significant digits (whole numbers from 100 up), no exponent and no
/// trailing zeros: "1.25", "12.5", "125", "1253", "0.05".
pub fn format_value(v: f64) -> String {
    if !v.is_finite() {
        return String::new();
    }
    if v == 0.0 {
        return "0".into();
    }
    let mag = v.abs().log10().floor() as i32;
    let decimals = (2 - mag).clamp(0, 12) as usize;
    // Rounding can carry into the next magnitude (9.996 → 10.0): fewer decimals then.
    let mut s = format!("{v:.decimals$}");
    if decimals > 0 {
        let r: f64 = s.parse().unwrap_or(v);
        let mag2 = r.abs().log10().floor() as i32;
        if mag2 > mag {
            let d2 = (2 - mag2).clamp(0, 12) as usize;
            s = format!("{v:.d2$}");
        }
    }
    if s.contains('.') {
        let t = s.trim_end_matches('0').trim_end_matches('.');
        s = t.to_owned();
    }
    if s == "-0" { "0".into() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_bars_are_round() {
        let c = scale_bar(130.0, Units::Metric).unwrap();
        assert_eq!((c.value, c.unit), (100.0, LengthUnit::M));
        let c = scale_bar(4_900.0, Units::Metric).unwrap();
        assert_eq!((c.value, c.unit, c.metres), (2.0, LengthUnit::Km, 2000.0));
        let c = scale_bar(5_000.0, Units::Metric).unwrap();
        assert_eq!(c.value, 5.0);
        let c = scale_bar(0.37, Units::Metric).unwrap();
        assert_eq!((c.value, c.unit), (0.2, LengthUnit::M));
        let c = scale_bar(100.0, Units::Imperial).unwrap();
        assert_eq!((c.value, c.unit), (200.0, LengthUnit::Ft));
        let c = scale_bar(350.0, Units::Imperial).unwrap();
        assert_eq!((c.value, c.unit), (1000.0, LengthUnit::Ft));
        let c = scale_bar(500.0, Units::Imperial).unwrap();
        assert_eq!((c.value, c.unit), (0.25, LengthUnit::Mi));
        let c = scale_bar(1000.0, Units::Imperial).unwrap();
        assert_eq!((c.value, c.unit), (0.5, LengthUnit::Mi));
        let c = scale_bar(9000.0, Units::Imperial).unwrap();
        assert_eq!((c.value, c.unit), (5.0, LengthUnit::Mi));
        assert!((c.metres - 5.0 * MILE).abs() < 1e-9);
        assert!(scale_bar(0.0, Units::Metric).is_none());
        assert!(scale_bar(f64::NAN, Units::Metric).is_none());
    }

    #[test]
    fn display_units() {
        assert_eq!(length(999.0, Units::Metric).1, LengthUnit::M);
        assert_eq!(length(1500.0, Units::Metric), (1.5, LengthUnit::Km));
        assert_eq!(length(100.0, Units::Imperial).1, LengthUnit::Ft);
        assert_eq!(length(MILE, Units::Imperial), (1.0, LengthUnit::Mi));
        assert_eq!(area(500.0, Units::Metric).1, AreaUnit::M2);
        assert_eq!(area(50_000.0, Units::Metric), (5.0, AreaUnit::Ha));
        assert_eq!(area(3.0e6, Units::Metric), (3.0, AreaUnit::Km2));
        assert_eq!(area(100.0, Units::Imperial).1, AreaUnit::Ft2);
        assert_eq!(area(8094.0, Units::Imperial).1, AreaUnit::Ac);
        assert_eq!(area(1.0e7, Units::Imperial).1, AreaUnit::Mi2);
    }

    #[test]
    fn values() {
        assert_eq!(format_value(1.2534), "1.25");
        assert_eq!(format_value(12.534), "12.5");
        assert_eq!(format_value(125.34), "125");
        assert_eq!(format_value(1253.4), "1253");
        assert_eq!(format_value(1.20), "1.2");
        assert_eq!(format_value(2.0), "2");
        assert_eq!(format_value(0.05), "0.05");
        assert_eq!(format_value(0.001234), "0.00123");
        assert_eq!(format_value(9.996), "10");
        assert_eq!(format_value(-3.24159), "-3.24");
        assert_eq!(format_value(0.0), "0");
    }
}
