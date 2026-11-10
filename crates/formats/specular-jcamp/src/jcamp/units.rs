//! Units as JCAMP-DX writes them, mapped to axis titles; numbers of header values.

use specular_spectra::{Axis, Technique};

/// A header number: `1.5`, `1.5E3`, `+2`, or `1,5` with a decimal comma.
pub(crate) fn number(s: &str) -> Option<f64> {
    let t = s.trim();
    let t = t.split_whitespace().next().unwrap_or("");
    t.parse::<f64>()
        .ok()
        .or_else(|| t.replace(',', ".").parse().ok())
        .filter(|v: &f64| v.is_finite())
}

/// A value for a label: up to four decimals, trailing zeros dropped.
pub(crate) fn short(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    if s == "-0" { "0".into() } else { s }
}

fn key(u: &str) -> String {
    u.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_uppercase)
        .collect()
}

/// What was measured, from `##DATA TYPE=` (upper case), else from the abscissa's unit.
pub(crate) fn technique(data_type: &str, x_units: &str) -> Technique {
    let d = data_type;
    if d.contains("RAMAN") {
        Technique::Raman
    } else if d.contains("INFRARED")
        || d.contains("FTIR")
        || d.starts_with("IR")
        || d.contains("NIR")
    {
        Technique::Infrared
    } else if d.contains("UV") || d.contains("VIS") {
        Technique::UvVis
    } else if d.contains("FLUORESCENCE") {
        Technique::Fluorescence
    } else if d.contains("NMR") {
        Technique::Nmr
    } else if d.contains("MASS") {
        Technique::Mass
    } else if d.contains("CHROMATOGRA") {
        Technique::Chromatogram
    } else {
        match key(x_units).as_str() {
            "1/CM" | "CM-1" | "CM^-1" => Technique::Infrared,
            "M/Z" => Technique::Mass,
            "PPM" => Technique::Nmr,
            _ => Technique::Other,
        }
    }
}

/// The abscissa's axis, and the divisor its values need (Hz to ppm on an NMR spectrum).
pub(crate) fn x_axis(
    units: &str,
    t: Technique,
    fid: bool,
    observe_mhz: Option<f64>,
) -> (Axis, Option<f64>) {
    let k = key(units);
    let a = match k.as_str() {
        "1/CM" | "CM-1" | "CM^-1" | "WAVENUMBERS" | "WAVENUMBER" | "1/CENTIMETERS" => {
            if t == Technique::Raman {
                Axis::new("Raman shift", "cm⁻¹", false)
            } else {
                Axis::new("Wavenumber", "cm⁻¹", true)
            }
        }
        "NANOMETERS" | "NM" => Axis::new("Wavelength", "nm", false),
        "MICROMETERS" | "MICRONS" | "UM" | "µM" => Axis::new("Wavelength", "µm", false),
        "HZ" | "HERTZ" if t == Technique::Nmr && !fid => match observe_mhz {
            Some(f) => return (Axis::new("Chemical shift", "ppm", true), Some(f)),
            None => Axis::new("Frequency", "Hz", true),
        },
        "HZ" | "HERTZ" => Axis::new("Frequency", "Hz", false),
        "PPM" => Axis::new("Chemical shift", "ppm", true),
        "SECONDS" | "SECOND" | "SEC" | "S" => Axis::new("Time", "s", false),
        "MINUTES" | "MINUTE" | "MIN" => Axis::new("Time", "min", false),
        "MILLISECONDS" | "MSEC" => Axis::new("Time", "ms", false),
        "M/Z" | "MZ" => Axis::new("m/z", "", false),
        "MASSUNITS" | "MASS" | "AMU" | "DA" => Axis::new("m/z", "", false),
        "" | "ARBITRARYUNITS" | "ARBITRARY" if t == Technique::Mass => Axis::new("m/z", "", false),
        "" | "ARBITRARYUNITS" | "ARBITRARY" => Axis::new("X", "", false),
        k if k.contains("1/CM") || k.contains("CM-1") => {
            return x_axis("1/CM", t, fid, observe_mhz);
        }
        k if k.contains("NANOMETER") || k.ends_with("(NM)") => Axis::new("Wavelength", "nm", false),
        _ => Axis::new("X", units.trim(), false),
    };
    (a, None)
}

/// The ordinate's axis.
pub(crate) fn y_axis(units: &str) -> Axis {
    let k = key(units);
    match k.as_str() {
        "TRANSMITTANCE" | "TRANSMISSION" | "%T" => Axis::new("Transmittance", "", false),
        "ABSORBANCE" | "A" => Axis::new("Absorbance", "", false),
        "KUBELKA-MUNK" | "KUBELKAMUNK" => Axis::new("Kubelka-Munk", "", false),
        "REFLECTANCE" | "%R" => Axis::new("Reflectance", "", false),
        "LOG(1/R)" => Axis::new("log(1/R)", "", false),
        "RELATIVEABUNDANCE" => Axis::new("Relative abundance", "", false),
        "COUNTS" => Axis::new("Counts", "", false),
        "" | "ARBITRARYUNITS" | "ARBITRARY" => Axis::new("Intensity", "", false),
        _ => Axis::new("Intensity", units.trim(), false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axes() {
        assert_eq!(number(" 1,5 "), Some(1.5));
        assert_eq!(number("2.5E3 $$x"), Some(2500.0));
        assert_eq!(short(1715.30000), "1715.3");
        let (a, d) = x_axis("1/CM", Technique::Infrared, false, None);
        assert!(a.reversed && d.is_none());
        let (a, d) = x_axis("HZ", Technique::Nmr, false, Some(400.0));
        assert_eq!((a.unit.as_str(), d), ("ppm", Some(400.0)));
        assert_eq!(technique("INFRARED SPECTRUM", ""), Technique::Infrared);
        assert_eq!(technique("", "m/z"), Technique::Mass);
    }
}
