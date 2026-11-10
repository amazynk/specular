//! Shared helpers of the AFM readers: SI units from the prefixed units instruments write,
//! pixel-centre calibrations and Latin-1 text.

use specular_em::dataset::Lateral;

/// Base units an SI prefix may precede.
const BASES: &[&str] = &["m", "V", "A", "N", "Hz", "s", "Pa", "W", "F", "S", "T", "C"];

/// Splits a unit into its base and the factor to it: "nm" → ("m", 1e-9), "~m" (Bruker's
/// micro) → ("m", 1e-6), "Å" → ("m", 1e-10), "deg" → ("deg", 1). Units it does not know come
/// back unchanged with factor 1.
pub(crate) fn si(unit: &str) -> (String, f64) {
    let u = unit.trim();
    if BASES.contains(&u) {
        return (u.to_string(), 1.0);
    }
    match u {
        "Å" | "Angstrom" | "angstrom" => return ("m".into(), 1e-10),
        "°" | "º" | "deg" | "degree" | "degrees" => return ("deg".into(), 1.0),
        "um" => return ("m".into(), 1e-6),
        _ => {}
    }
    let mut chars = u.chars();
    let Some(p) = chars.next() else {
        return (String::new(), 1.0);
    };
    let rest = chars.as_str();
    if !BASES.contains(&rest) {
        return (u.to_string(), 1.0);
    }
    let f = match p {
        'f' => 1e-15,
        'p' => 1e-12,
        'n' => 1e-9,
        'u' | 'µ' | 'μ' | '~' => 1e-6,
        'm' => 1e-3,
        'c' if rest == "m" => 1e-2,
        'k' => 1e3,
        'M' => 1e6,
        'G' => 1e9,
        _ => return (u.to_string(), 1.0),
    };
    (rest.to_string(), f)
}

/// The calibration of `n` pixels spanning `range` from `origin` (edges), at pixel centres.
pub(crate) fn centres(origin: f64, range: f64, n: usize, unit: &str) -> Option<Lateral> {
    if n == 0 {
        return None;
    }
    let step = range / n as f64;
    let l = Lateral::new(origin + step / 2.0, step, unit);
    l.is_usable().then_some(l)
}

/// Latin-1 bytes as text (every byte one character).
pub(crate) fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| char::from(c)).collect()
}

/// Mirrors each row of a `width`-wide plane left to right (a backward scan line).
pub(crate) fn mirror_rows(v: &mut [f32], width: usize) {
    if width > 0 {
        for row in v.chunks_exact_mut(width) {
            row.reverse();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes() {
        assert_eq!(si("nm"), ("m".into(), 1e-9));
        assert_eq!(si("~m"), ("m".into(), 1e-6));
        assert_eq!(si("µm"), ("m".into(), 1e-6));
        assert_eq!(si("mV"), ("V".into(), 1e-3));
        assert_eq!(si("m"), ("m".into(), 1.0));
        assert_eq!(si("pA"), ("A".into(), 1e-12));
        assert_eq!(si("kHz"), ("Hz".into(), 1e3));
        assert_eq!(si("º"), ("deg".into(), 1.0));
        assert_eq!(si("Arb"), ("Arb".into(), 1.0));
        assert_eq!(si(""), (String::new(), 1.0));
        let c = centres(1.0, 4.0, 4, "m").unwrap();
        assert_eq!((c.offset, c.step), (1.5, 1.0));
        assert!(centres(0.0, 0.0, 4, "m").is_none());
    }
}
