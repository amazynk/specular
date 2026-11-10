//! The little JSON writing the exporters need (reading is serde_json's).

use std::fmt::Write as _;

/// A JSON string literal.
pub fn string(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// A number: shortest round-tripping decimal, `null` when not finite.
pub fn num(v: f64) -> String {
    if v.is_finite() {
        // Display never uses an exponent and round-trips; -0 is written 0.
        if v == 0.0 {
            "0".to_owned()
        } else {
            format!("{v}")
        }
    } else {
        "null".to_owned()
    }
}

/// `v` rounded to `places` decimals (coordinates: 1/1000 px is plenty and keeps files small).
pub fn round(v: f64, places: i32) -> f64 {
    let f = 10f64.powi(places);
    (v * f).round() / f
}

/// An sRGB colour as `[r, g, b]`.
pub fn rgb(c: [u8; 3]) -> String {
    format!("[{},{},{}]", c[0], c[1], c[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_and_numbers() {
        assert_eq!(string("a\"b\\c\n\u{1}"), "\"a\\\"b\\\\c\\n\\u0001\"");
        assert_eq!(num(1.5), "1.5");
        assert_eq!(num(-0.0), "0");
        assert_eq!(num(f64::NAN), "null");
        assert_eq!(num(1e21), "1000000000000000000000");
        assert_eq!(round(1.23456, 3), 1.235);
    }
}
