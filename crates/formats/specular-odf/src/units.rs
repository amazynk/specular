//! Lengths, angles, percentages and colours as OpenDocument writes them.

use specular_drawpage::Rgba;

/// A length (`2.5cm`, `10mm`, `1in`, `12pt`, `1pc`, `96px`, a bare number as points) in
/// points.
pub fn length(s: &str) -> Option<f64> {
    let s = s.trim();
    let split = s
        .find(|c: char| c.is_ascii_alphabetic() || c == '%')
        .unwrap_or(s.len());
    let v: f64 = s[..split].trim().parse().ok()?;
    let k = match s[split..].trim().to_ascii_lowercase().as_str() {
        "" | "pt" => 1.0,
        "cm" => 72.0 / 2.54,
        "mm" => 72.0 / 25.4,
        "in" | "inch" => 72.0,
        "pc" => 12.0,
        "px" => 0.75,
        _ => return None,
    };
    let v = v * k;
    v.is_finite().then_some(v)
}

/// A percentage (`60%`) as a fraction, or a bare number in 0–1.
pub fn percent(s: &str) -> Option<f64> {
    let s = s.trim();
    let v = if let Some(p) = s.strip_suffix('%') {
        p.trim().parse::<f64>().ok()? / 100.0
    } else {
        s.parse::<f64>().ok()?
    };
    v.is_finite().then_some(v)
}

/// An angle (`30deg`, `0.5rad`, `100grad`, a bare number as degrees) in degrees.
pub fn angle_deg(s: &str) -> Option<f64> {
    let s = s.trim();
    let (num, k) = if let Some(n) = s.strip_suffix("deg") {
        (n, 1.0)
    } else if let Some(n) = s.strip_suffix("grad") {
        (n, 0.9)
    } else if let Some(n) = s.strip_suffix("rad") {
        (n, 180.0 / std::f64::consts::PI)
    } else {
        (s, 1.0)
    };
    let v = num.trim().parse::<f64>().ok()? * k;
    v.is_finite().then_some(v)
}

/// `#rrggbb` (alpha 255).
pub fn color(s: &str) -> Option<Rgba> {
    specular_drawpage::hex_color(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units() {
        assert!((length("2.54cm").unwrap() - 72.0).abs() < 1e-9);
        assert!((length("25.4mm").unwrap() - 72.0).abs() < 1e-9);
        assert_eq!(length("1in"), Some(72.0));
        assert_eq!(length("12pt"), Some(12.0));
        assert_eq!(length("-1pc"), Some(-12.0));
        assert_eq!(length("4px"), Some(3.0));
        assert_eq!(length("abc"), None);
        assert_eq!(length("1furlong"), None);
        assert_eq!(percent("60%"), Some(0.6));
        assert_eq!(angle_deg("90deg"), Some(90.0));
        assert_eq!(angle_deg("450"), Some(450.0));
        assert!((angle_deg("100grad").unwrap() - 90.0).abs() < 1e-9);
    }
}
