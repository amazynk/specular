//! What the exposure properties' values mean (ISO 15740 §13.5), for showing them.

use crate::codes::prop;
use crate::data::{Form, PropDesc, Value};

/// White balance settings (§13.5.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhiteBalance {
    Manual,
    Auto,
    OnePush,
    Daylight,
    Fluorescent,
    Tungsten,
    Flash,
}

/// Exposure programs (§13.5.14).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Program {
    Manual,
    Auto,
    Aperture,
    Shutter,
    Creative,
    Action,
    Portrait,
}

/// A property value, ready to show. Words are left to the app to translate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shown {
    /// Already readable: `1/250`, `2.5″`, `f/2.8`, `ISO 400`, `+0.7 EV`.
    Text(String),
    /// Automatic (ISO).
    Auto,
    /// The shutter stays open while the button is held.
    Bulb,
    WhiteBalance(WhiteBalance),
    Program(Program),
    /// A value the standard doesn't define (a vendor's).
    Other(i64),
}

/// `v` of property `code`.
pub fn show(code: u16, v: &Value) -> Shown {
    if let Value::Str(s) = v {
        return Shown::Text(s.clone());
    }
    let Some(x) = v.as_i64() else {
        return Shown::Other(0);
    };
    match code {
        prop::EXPOSURE_TIME => shutter(x),
        prop::F_NUMBER if x > 0 => Shown::Text(format!("f/{}", one_decimal(x as f64 / 100.0))),
        prop::EXPOSURE_INDEX => match x {
            0xFFFF => Shown::Auto,
            x if x > 0 => Shown::Text(format!("ISO {x}")),
            _ => Shown::Other(x),
        },
        prop::EXPOSURE_BIAS_COMPENSATION => {
            // Thousandths of a stop (−333 is −⅓ EV).
            let ev = x as f64 / 1000.0;
            let t = one_decimal(ev.abs());
            Shown::Text(if x == 0 {
                "0 EV".into()
            } else if x > 0 {
                format!("+{t} EV")
            } else {
                format!("\u{2212}{t} EV")
            })
        }
        prop::WHITE_BALANCE => match x {
            1 => Shown::WhiteBalance(WhiteBalance::Manual),
            2 => Shown::WhiteBalance(WhiteBalance::Auto),
            3 => Shown::WhiteBalance(WhiteBalance::OnePush),
            4 => Shown::WhiteBalance(WhiteBalance::Daylight),
            5 => Shown::WhiteBalance(WhiteBalance::Fluorescent),
            6 => Shown::WhiteBalance(WhiteBalance::Tungsten),
            7 => Shown::WhiteBalance(WhiteBalance::Flash),
            _ => Shown::Other(x),
        },
        prop::EXPOSURE_PROGRAM_MODE => match x {
            1 => Shown::Program(Program::Manual),
            2 => Shown::Program(Program::Auto),
            3 => Shown::Program(Program::Aperture),
            4 => Shown::Program(Program::Shutter),
            5 => Shown::Program(Program::Creative),
            6 => Shown::Program(Program::Action),
            7 => Shown::Program(Program::Portrait),
            _ => Shown::Other(x),
        },
        prop::FOCAL_LENGTH if x > 0 => Shown::Text(format!("{} mm", one_decimal(x as f64 / 100.0))),
        _ => Shown::Other(x),
    }
}

/// Exposure time in units of 0.1 ms (§13.5.13).
fn shutter(x: i64) -> Shown {
    if x == 0xFFFF_FFFF || x == -1 {
        return Shown::Bulb;
    }
    if x <= 0 {
        return Shown::Other(x);
    }
    if x >= 10_000 {
        // Whole and part seconds: 2″, 2.5″.
        Shown::Text(format!("{}\u{2033}", one_decimal(x as f64 / 10_000.0)))
    } else {
        let d = 10_000.0 / x as f64;
        // 1/3 (3333), 1/8000 (1 → 1/10000 at this resolution), 0.3″ for slow fractions
        // that aren't 1/n.
        let n = d.round();
        if (d - n).abs() / d < 0.02 || d >= 4.0 {
            Shown::Text(format!("1/{}", n as i64))
        } else {
            Shown::Text(format!("{}\u{2033}", one_decimal(x as f64 / 10_000.0)))
        }
    }
}

/// `x` with one decimal, the `.0` left off.
fn one_decimal(x: f64) -> String {
    let r = (x * 10.0).round() / 10.0;
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r:.1}")
    }
}

/// The battery level as a percentage of the range the camera gives (0–100 when it gives
/// none).
pub fn battery_percent(d: &PropDesc) -> Option<u8> {
    let v = d.current.as_i64()?;
    let (lo, hi) = match &d.form {
        Form::Range { min, max, .. } => (min.as_i64()?, max.as_i64()?),
        Form::Enum(e) => (
            e.iter().filter_map(Value::as_i64).min()?,
            e.iter().filter_map(Value::as_i64).max()?,
        ),
        Form::None => (0, 100),
    };
    if hi <= lo {
        return None;
    }
    Some((((v - lo).clamp(0, hi - lo) * 100) / (hi - lo)) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codes::ty;

    fn t(code: u16, x: i64) -> Shown {
        show(code, &Value::Int(x))
    }

    fn txt(s: &str) -> Shown {
        Shown::Text(s.into())
    }

    #[test]
    fn shutter_speeds() {
        assert_eq!(t(prop::EXPOSURE_TIME, 40), txt("1/250"));
        assert_eq!(t(prop::EXPOSURE_TIME, 3333), txt("1/3"));
        assert_eq!(t(prop::EXPOSURE_TIME, 1), txt("1/10000"));
        assert_eq!(t(prop::EXPOSURE_TIME, 10_000), txt("1\u{2033}"));
        assert_eq!(t(prop::EXPOSURE_TIME, 25_000), txt("2.5\u{2033}"));
        assert_eq!(t(prop::EXPOSURE_TIME, 4000), txt("0.4\u{2033}"));
        assert_eq!(t(prop::EXPOSURE_TIME, 5000), txt("1/2"));
        assert_eq!(t(prop::EXPOSURE_TIME, 0xFFFF_FFFF), Shown::Bulb);
    }

    #[test]
    fn aperture_iso_bias() {
        assert_eq!(t(prop::F_NUMBER, 280), txt("f/2.8"));
        assert_eq!(t(prop::F_NUMBER, 800), txt("f/8"));
        assert_eq!(t(prop::EXPOSURE_INDEX, 400), txt("ISO 400"));
        assert_eq!(t(prop::EXPOSURE_INDEX, 0xFFFF), Shown::Auto);
        assert_eq!(
            t(prop::EXPOSURE_BIAS_COMPENSATION, -333),
            txt("\u{2212}0.3 EV")
        );
        assert_eq!(t(prop::EXPOSURE_BIAS_COMPENSATION, 667), txt("+0.7 EV"));
        assert_eq!(t(prop::EXPOSURE_BIAS_COMPENSATION, 0), txt("0 EV"));
        assert_eq!(
            t(prop::WHITE_BALANCE, 6),
            Shown::WhiteBalance(WhiteBalance::Tungsten)
        );
        assert_eq!(t(prop::WHITE_BALANCE, 0x8012), Shown::Other(0x8012));
        assert_eq!(
            t(prop::EXPOSURE_PROGRAM_MODE, 3),
            Shown::Program(Program::Aperture)
        );
    }

    #[test]
    fn battery_scales_to_the_range() {
        let d = PropDesc {
            code: prop::BATTERY_LEVEL,
            data_type: ty::UINT8,
            writable: false,
            default: Value::UInt(0),
            current: Value::UInt(2),
            form: Form::Range {
                min: Value::UInt(0),
                max: Value::UInt(4),
                step: Value::UInt(1),
            },
        };
        assert_eq!(battery_percent(&d), Some(50));
        let e = PropDesc {
            form: Form::Enum([0, 25, 50, 75, 100].map(Value::UInt).to_vec()),
            current: Value::UInt(75),
            ..d.clone()
        };
        assert_eq!(battery_percent(&e), Some(75));
        let n = PropDesc {
            form: Form::None,
            current: Value::UInt(140),
            ..d
        };
        assert_eq!(battery_percent(&n), Some(100));
    }
}
