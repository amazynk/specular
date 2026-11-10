//! How a drawing writes its measurements: number formats (ISO 32000 §12.9, the number format
//! dictionaries of a PDF measure; CAD linear unit styles), drawing ratios ("1:50") and the
//! round lengths of a scale bar.
//!
//! A chain of number formats writes one value in several units, largest first ("5 ft 3 in"):
//! the value in the first unit is the input times the first factor, each further unit gets the
//! remainder of the one before times its own factor, and the last one is written whole
//! (decimals, a fraction, rounded or truncated).

use crate::calib::Unit;

/// How the last unit of a chain writes the part below one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fraction {
    /// Decimals: `precision` is a power of ten (100: two decimals).
    Decimal,
    /// A fraction with `precision` as the denominator ("3 1/2").
    Fraction,
    /// Rounded to a whole number.
    Round,
    /// Truncated to a whole number.
    Truncate,
}

/// One unit of a chain (a PDF NumberFormat dictionary).
#[derive(Clone, Debug, PartialEq)]
pub struct NumberFormat {
    /// The label (`U`), e.g. `m`, `ft`, `'`.
    pub unit: String,
    /// The factor from the previous unit (or from the input, for the first) to this one (`C`).
    pub factor: f64,
    /// `F`.
    pub fraction: Fraction,
    /// `D`: decimals as a power of ten, or the fraction's denominator.
    pub precision: u32,
    /// Fractions are reduced (`FD` false).
    pub reduce: bool,
    /// Between thousands (`RT`).
    pub thousands: String,
    /// The decimal mark (`RD`).
    pub decimal: String,
    /// Before the label (`PS`) and after it (`SS`).
    pub before: String,
    pub after: String,
    /// The label goes before the number (`O` = `P`).
    pub prefix: bool,
}

impl Default for NumberFormat {
    /// ISO 32000's defaults: two decimals, `,` and `.`, a space either side of the label.
    fn default() -> NumberFormat {
        NumberFormat {
            unit: String::new(),
            factor: 1.0,
            fraction: Fraction::Decimal,
            precision: 100,
            reduce: true,
            thousands: ",".into(),
            decimal: ".".into(),
            before: " ".into(),
            after: " ".into(),
            prefix: false,
        }
    }
}

impl NumberFormat {
    /// A unit with the default style.
    pub fn new(unit: &str, factor: f64) -> NumberFormat {
        NumberFormat {
            unit: unit.into(),
            factor,
            ..NumberFormat::default()
        }
    }

    fn label(&self, number: &str) -> String {
        if self.prefix {
            format!("{}{}{}{number}", self.before, self.unit, self.after)
        } else {
            format!("{number}{}{}{}", self.before, self.unit, self.after)
        }
    }

    /// Digits of a whole number with the thousands mark.
    fn whole(&self, v: u64) -> String {
        let digits = v.to_string();
        let mut out = String::new();
        for (i, c) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i).is_multiple_of(3) {
                out.push_str(&self.thousands);
            }
            out.push(c);
        }
        out
    }

    /// The last unit's number (non-negative `v`).
    fn number(&self, v: f64) -> String {
        let d = self.precision.max(1);
        match self.fraction {
            Fraction::Round => self.whole(v.round() as u64),
            Fraction::Truncate => self.whole(v.trunc() as u64),
            Fraction::Decimal => {
                let decimals = (f64::from(d).log10().round() as usize).min(9);
                let scaled = (v * 10f64.powi(decimals as i32)).round();
                let p = 10f64.powi(decimals as i32);
                let int = (scaled / p).trunc();
                let frac = scaled - int * p;
                let mut s = self.whole(int as u64);
                if decimals > 0 {
                    s.push_str(&self.decimal);
                    s.push_str(&format!("{:0decimals$}", frac as u64));
                }
                s
            }
            Fraction::Fraction => {
                let mut int = v.trunc();
                let mut num = ((v - int) * f64::from(d)).round() as u64;
                let mut den = u64::from(d);
                if num >= den {
                    int += 1.0;
                    num = 0;
                }
                if self.reduce && num > 0 {
                    let g = gcd(num, den);
                    num /= g;
                    den /= g;
                }
                match (int as u64, num) {
                    (i, 0) => self.whole(i),
                    (0, n) => format!("{n}/{den}"),
                    (i, n) => format!("{} {n}/{den}", self.whole(i)),
                }
            }
        }
    }
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// `value` written through `chain` (see the module docs); `None` for an empty chain or a
/// value that is not finite.
pub fn format_chain(value: f64, chain: &[NumberFormat]) -> Option<String> {
    let (last, head) = chain.split_last()?;
    if !value.is_finite() {
        return None;
    }
    let sign = if value < 0.0 { "-" } else { "" };
    let mut v = value.abs() * chain[0].factor;
    let mut out = String::new();
    for (i, f) in head.iter().enumerate() {
        // The last unit's rounding may carry into this one ("5 ft 12 in" is "6 ft").
        let next = &chain[i + 1];
        let rest_units = (v - v.trunc()) * chain[i + 1..].iter().map(|f| f.factor).product::<f64>();
        let whole_rest: f64 = chain[i + 1..].iter().map(|f| f.factor).product();
        let mut int = v.trunc();
        if carries(rest_units, whole_rest, last) {
            int += 1.0;
            v = int;
        }
        if int > 0.0 {
            out.push_str(&f.label(&f.whole(int as u64)));
        }
        v = (v - int).max(0.0) * next.factor;
    }
    out.push_str(&last.label(&last.number(v)));
    let s = out.trim();
    Some(format!("{sign}{s}"))
}

/// Whether `rest` (in the last unit, out of `full` per unit above) rounds up to a whole unit
/// above it.
fn carries(rest: f64, full: f64, last: &NumberFormat) -> bool {
    let d = f64::from(last.precision.max(1));
    let rounded = match last.fraction {
        Fraction::Decimal => (rest * d).round() / d,
        Fraction::Fraction => (rest * d).round() / d,
        Fraction::Round => rest.round(),
        Fraction::Truncate => rest.trunc(),
    };
    rounded >= full - 1e-9
}

/// A unit's chain in plain decimals (`decimals` places, no thousands mark).
pub fn decimal(unit: &str, factor: f64, decimals: u32) -> Vec<NumberFormat> {
    vec![NumberFormat {
        precision: 10u32.saturating_pow(decimals.min(9)),
        thousands: String::new(),
        ..NumberFormat::new(unit, factor)
    }]
}

/// A CAD linear unit style (DXF `$LUNITS` 1–5 with `$LUPREC` places) for values in `unit`:
/// architectural and engineering (feet and inches, `5'-3 1/2"`) only for inches and feet,
/// fractional as a fraction of the unit, decimal and scientific as decimals.
pub fn cad_formats(lunits: i16, luprec: i16, unit: Unit) -> Vec<NumberFormat> {
    let prec = luprec.clamp(0, 8) as u32;
    let symbol = unit.symbol();
    // Feet and inches from a value in `unit`.
    let to_ft = match unit {
        Unit::Inch => Some(1.0 / 12.0),
        Unit::Ft => Some(1.0),
        _ => None,
    };
    let ft = |f: f64| NumberFormat {
        before: String::new(),
        after: "-".into(),
        thousands: String::new(),
        ..NumberFormat::new("'", f)
    };
    let inch = |fraction: Fraction, precision: u32| NumberFormat {
        before: String::new(),
        after: String::new(),
        thousands: String::new(),
        fraction,
        precision,
        ..NumberFormat::new("\"", 12.0)
    };
    match (lunits, to_ft) {
        (4, Some(f)) => vec![ft(f), inch(Fraction::Fraction, 1 << prec)],
        (3, Some(f)) => vec![ft(f), inch(Fraction::Decimal, 10u32.pow(prec))],
        (5, _) => vec![NumberFormat {
            fraction: Fraction::Fraction,
            precision: 1 << prec,
            thousands: String::new(),
            ..NumberFormat::new(symbol, 1.0)
        }],
        _ => decimal(symbol, 1.0, prec),
    }
}

/// A drawing ratio as written on drawings: `1:50`, `1:2.5`, `10:1` for enlargements. `n` is
/// real size over drawn size.
pub fn ratio_text(n: f64) -> String {
    if !n.is_finite() || n <= 0.0 {
        return String::new();
    }
    let tidy = |v: f64| {
        let r = v.round();
        if (v - r).abs() <= r.abs() * 0.005 + 1e-9 {
            format!("{r:.0}")
        } else {
            let s = crate::calib::format_sig(v, 3);
            if s.contains('.') {
                s.trim_end_matches('0').trim_end_matches('.').to_owned()
            } else {
                s
            }
        }
    };
    if n >= 1.0 - 1e-9 {
        format!("1:{}", tidy(n))
    } else {
        format!("{}:1", tidy(1.0 / n))
    }
}

/// The largest round length (1, 2 or 5 × 10ⁿ) not above `max`.
pub fn round_length(max: f64) -> f64 {
    if !max.is_finite() || max <= 0.0 {
        return 0.0;
    }
    let p = 10f64.powf(max.log10().floor());
    [5.0, 2.0, 1.0]
        .into_iter()
        .map(|k| k * p)
        .find(|v| *v <= max * (1.0 + 1e-12))
        .unwrap_or(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain(v: f64, c: &[NumberFormat]) -> String {
        format_chain(v, c).unwrap()
    }

    #[test]
    fn decimals_and_thousands() {
        let m = vec![NumberFormat::new("m", 1.0)];
        assert_eq!(chain(1234.567, &m), "1,234.57 m");
        assert_eq!(chain(0.004, &m), "0.00 m");
        let de = vec![NumberFormat {
            thousands: ".".into(),
            decimal: ",".into(),
            precision: 1000,
            ..NumberFormat::new("m", 0.5)
        }];
        assert_eq!(chain(5000.0, &de), "2.500,000 m");
        let pre = vec![NumberFormat {
            prefix: true,
            before: String::new(),
            unit: "≈".into(),
            ..NumberFormat::new("", 1.0)
        }];
        assert_eq!(chain(3.0, &pre), "≈ 3.00");
    }

    #[test]
    fn feet_and_inches() {
        // The input in feet: 5.25 ft is 5 ft 3 in.
        let c = vec![
            NumberFormat::new("ft", 1.0),
            NumberFormat {
                fraction: Fraction::Fraction,
                precision: 8,
                ..NumberFormat::new("in", 12.0)
            },
        ];
        assert_eq!(chain(5.25, &c), "5 ft 3 in");
        assert_eq!(chain(5.0 + 3.5 / 12.0, &c), "5 ft 3 1/2 in");
        // Rounding carries into feet.
        assert_eq!(chain(5.0 + 11.99 / 12.0, &c), "6 ft 0 in");
        assert_eq!(chain(0.5 / 12.0, &c), "1/2 in");
        // Unreduced eighths.
        let mut c2 = c.clone();
        c2[1].reduce = false;
        assert_eq!(chain(5.0 + 3.5 / 12.0, &c2), "5 ft 3 4/8 in");
    }

    #[test]
    fn cad_styles() {
        let arch = cad_formats(4, 4, Unit::Inch);
        assert_eq!(chain(63.5, &arch), "5'-3 1/2\"");
        let eng = cad_formats(3, 2, Unit::Inch);
        assert_eq!(chain(63.5, &eng), "5'-3.50\"");
        let dec = cad_formats(2, 3, Unit::Mm);
        assert_eq!(chain(1234.5678, &dec), "1234.568 mm");
        let frac = cad_formats(5, 3, Unit::Mm);
        assert_eq!(chain(2.25, &frac), "2 1/4 mm");
        // Architectural on a metric drawing is decimals.
        assert_eq!(chain(2.25, &cad_formats(4, 2, Unit::M)), "2.25 m");
    }

    #[test]
    fn ratios_and_round_lengths() {
        assert_eq!(ratio_text(50.0), "1:50");
        assert_eq!(ratio_text(49.9), "1:50");
        assert_eq!(ratio_text(48.0), "1:48");
        assert_eq!(ratio_text(2.5), "1:2.5");
        assert_eq!(ratio_text(0.1), "10:1");
        assert_eq!(ratio_text(1.0), "1:1");
        assert_eq!(round_length(7.3), 5.0);
        assert_eq!(round_length(0.03), 0.02);
        assert_eq!(round_length(100.0), 100.0);
        assert_eq!(round_length(0.0), 0.0);
    }
}
