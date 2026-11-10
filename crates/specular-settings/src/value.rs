//! Setting kinds, dynamic values, validation and conversion to and from TOML and text.

use std::fmt;
use std::path::PathBuf;

use specular_i18n::{LanguageIdentifier, LanguagePreference};

use crate::error::SettingError;

/// Unit shown after a numeric value. Each maps to a `unit-*` message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unit {
    None,
    /// "800%".
    Percent,
    /// "1500 ms".
    Milliseconds,
    /// "1.5 s".
    Seconds,
    /// "4096 MiB".
    Mebibytes,
    /// "2 images".
    Images,
    /// "8 threads".
    Threads,
    /// "350 cd/m²".
    Luminance,
    /// "−6 dB".
    Decibels,
}

impl Unit {
    /// The Fluent message that formats a value with this unit (`$value`).
    pub fn message_id(self) -> Option<&'static str> {
        match self {
            Unit::None => None,
            Unit::Percent => Some("unit-percent"),
            Unit::Milliseconds => Some("unit-milliseconds"),
            Unit::Seconds => Some("unit-seconds"),
            Unit::Mebibytes => Some("unit-mebibytes"),
            Unit::Images => Some("unit-images"),
            Unit::Threads => Some("unit-threads"),
            Unit::Luminance => Some("unit-luminance"),
            Unit::Decibels => Some("unit-decibels"),
        }
    }
}

/// What kind of control a setting gets and how its values are validated.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// On/off switch.
    Bool,
    /// One of a fixed set of options (TOML strings). Labels are `<label_id>-<option>`.
    Enum { options: &'static [&'static str] },
    /// Whole number in `min..=max`, moved in `step`s. With `auto`, 0 also means "automatic".
    Int {
        min: i64,
        max: i64,
        step: i64,
        unit: Unit,
        auto: bool,
    },
    /// Number in `min..=max`, moved in `step`s.
    Float {
        min: f64,
        max: f64,
        step: f64,
        unit: Unit,
    },
    /// sRGB colour, `#RRGGBB` or `#RRGGBBAA`.
    Color,
    /// File or folder path; empty means "automatic/default".
    Path { directory: bool },
    /// UI language: a tag or "system".
    Language,
    /// Locale for region formats: a tag or "language" (follow the interface language).
    Locale,
    /// A key combination (Keys section; Stage 1 reads them from keymap.toml).
    Shortcut,
    /// One line of free text (a file-name pattern), shown as is and edited where it is used
    /// or in the file.
    Text,
    /// A display (monitor), by the name the system gives it, or "auto". The choices are the
    /// displays connected now, which only the application knows.
    Monitor,
    /// Ordered subset of `options` (TOML array of strings). The options in `repeat` may
    /// appear any number of times (toolbar spacers); the others at most once.
    List {
        options: &'static [&'static str],
        repeat: &'static [&'static str],
    },
    /// A command shown as a button; has no value and is never stored.
    Action,
    /// One of the names the application offers now (an OCIO config's displays, views or
    /// colour spaces), as text; empty for the default.
    Pick,
}

/// A setting value, independent of its Rust type.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    /// Enum option, language tag, locale or shortcut.
    Text(String),
    Color(Color),
    Path(Option<PathBuf>),
    List(Vec<String>),
    /// Placeholder value of an [`Kind::Action`].
    None,
}

impl fmt::Display for Value {
    /// The canonical text form, as accepted by `--set` and written to TOML strings.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(i) => write!(f, "{i}"),
            Value::Float(x) => write!(f, "{x}"),
            Value::Text(s) => f.write_str(s),
            Value::Color(c) => write!(f, "{c}"),
            Value::Path(Some(p)) => write!(f, "{}", p.display()),
            Value::Path(None) => Ok(()),
            Value::List(l) => f.write_str(&l.join(",")),
            Value::None => Ok(()),
        }
    }
}

/// An sRGB colour with alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Parses `#RGB`, `#RRGGBB` or `#RRGGBBAA` (the `#` is optional).
    pub fn parse(s: &str) -> Option<Self> {
        let h = s.trim().strip_prefix('#').unwrap_or(s.trim());
        if !h.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
        match h.len() {
            3 => {
                let n = |i: usize| u8::from_str_radix(&h[i..i + 1], 16).ok().map(|v| v * 17);
                Some(Color::rgb(n(0)?, n(1)?, n(2)?))
            }
            6 => Some(Color::rgb(byte(0)?, byte(2)?, byte(4)?)),
            8 => Some(Color {
                r: byte(0)?,
                g: byte(2)?,
                b: byte(4)?,
                a: byte(6)?,
            }),
            _ => None,
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02X}{:02X}{:02X}", self.r, self.g, self.b)?;
        if self.a != 255 {
            write!(f, "{:02X}", self.a)?;
        }
        Ok(())
    }
}

/// Locale for dates, numbers and units.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum RegionFormats {
    /// Follow the interface language.
    #[default]
    FollowLanguage,
    /// A specific locale (`en-GB`, `de-CH`).
    Locale(LanguageIdentifier),
}

impl RegionFormats {
    /// The locale to pass to [`specular_i18n::I18n::set_region_formats`].
    pub fn locale(&self) -> Option<LanguageIdentifier> {
        match self {
            RegionFormats::FollowLanguage => None,
            RegionFormats::Locale(l) => Some(l.clone()),
        }
    }
}

/// Which display something goes to (`slideshow.monitor`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum MonitorChoice {
    /// Another display than the main window's when there is one, else the main window's.
    #[default]
    Auto,
    /// The display with this name (as the system reports it: "HDMI-A-1", "DELL U2720Q").
    Named(String),
}

impl MonitorChoice {
    /// From its text form: "auto" (any case) or empty is [`MonitorChoice::Auto`], anything
    /// else a display name (trimmed).
    pub fn parse(s: &str) -> MonitorChoice {
        let t = s.trim();
        if t.is_empty() || t.eq_ignore_ascii_case("auto") {
            MonitorChoice::Auto
        } else {
            MonitorChoice::Named(t.to_owned())
        }
    }

    /// The text form ("auto" or the name).
    pub fn to_text(&self) -> String {
        match self {
            MonitorChoice::Auto => "auto".into(),
            MonitorChoice::Named(n) => n.clone(),
        }
    }
}

impl SettingValue for MonitorChoice {
    fn default_kind() -> Kind {
        Kind::Monitor
    }
    fn to_value(&self) -> Value {
        Value::Text(self.to_text())
    }
    fn from_value(v: &Value) -> Option<Self> {
        match v {
            Value::Text(s) => Some(MonitorChoice::parse(s)),
            _ => None,
        }
    }
}

/// Marker type for action entries (buttons such as "Export settings…").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Action;

/// A key combination as text ("Ctrl+Shift+S"); validated by the keymap, not here.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Shortcut(pub String);

// ----- Validation and parsing -------------------------------------------

const TRUE: &[&str] = &["true", "on", "yes", "1"];
const FALSE: &[&str] = &["false", "off", "no", "0"];

impl Kind {
    /// Checks and normalises `v` for this kind (range, options, types).
    pub fn validate(&self, v: Value) -> Result<Value, SettingError> {
        match (self, v) {
            (Kind::Bool, v @ Value::Bool(_)) => Ok(v),
            (Kind::Bool, _) => Err(SettingError::NotBool),
            (Kind::Int { min, max, auto, .. }, Value::Int(i)) => {
                if (*min..=*max).contains(&i) || (*auto && i == 0) {
                    Ok(Value::Int(i))
                } else {
                    Err(SettingError::OutOfRange {
                        min: min.to_string(),
                        max: max.to_string(),
                        auto: *auto,
                    })
                }
            }
            (Kind::Int { .. }, Value::Float(x)) if x.fract() == 0.0 && x.abs() < 9e15 => {
                self.validate(Value::Int(x as i64))
            }
            (Kind::Int { .. }, _) => Err(SettingError::NotInteger),
            (Kind::Float { min, max, .. }, Value::Float(x)) => {
                if x.is_finite() && (*min..=*max).contains(&x) {
                    Ok(Value::Float(x))
                } else {
                    Err(SettingError::OutOfRange {
                        min: min.to_string(),
                        max: max.to_string(),
                        auto: false,
                    })
                }
            }
            (Kind::Float { .. }, Value::Int(i)) => self.validate(Value::Float(i as f64)),
            (Kind::Float { .. }, _) => Err(SettingError::NotNumber),
            (Kind::Enum { options }, Value::Text(s)) => {
                let s = s.trim().to_ascii_lowercase().replace('-', "_");
                if options.contains(&s.as_str()) || crate::schema::legacy_option(options, &s) {
                    Ok(Value::Text(s))
                } else {
                    Err(SettingError::UnknownOption { options })
                }
            }
            (Kind::Enum { options }, _) => Err(SettingError::UnknownOption { options }),
            (Kind::Color, v @ Value::Color(_)) => Ok(v),
            (Kind::Color, Value::Text(s)) => Color::parse(&s)
                .map(Value::Color)
                .ok_or(SettingError::BadColour),
            (Kind::Color, _) => Err(SettingError::BadColour),
            (Kind::Path { .. }, v @ Value::Path(_)) => Ok(v),
            (Kind::Path { .. }, Value::Text(s)) => {
                let s = s.trim();
                Ok(Value::Path((!s.is_empty()).then(|| PathBuf::from(s))))
            }
            (Kind::Path { .. }, _) => Err(SettingError::BadPath),
            (Kind::Language, Value::Text(s)) => s
                .parse::<LanguagePreference>()
                .map(|p| Value::Text(p.to_string()))
                .map_err(|_| SettingError::BadLanguage),
            (Kind::Locale, Value::Text(s)) => {
                let t = s.trim();
                if t.is_empty() || t.eq_ignore_ascii_case("language") {
                    Ok(Value::Text("language".into()))
                } else {
                    specular_i18n::parse_locale(t)
                        .map(|l| Value::Text(l.to_string()))
                        .map_err(|_| SettingError::BadLanguage)
                }
            }
            (Kind::Language | Kind::Locale, _) => Err(SettingError::BadLanguage),
            (Kind::Shortcut, Value::Text(s)) => Ok(Value::Text(s.trim().to_owned())),
            (Kind::Shortcut, _) => Err(SettingError::Syntax),
            (Kind::Text, Value::Text(s)) if !s.chars().any(char::is_control) => Ok(Value::Text(s)),
            (Kind::Text, Value::Text(_)) => Err(SettingError::BadText),
            (Kind::Text, _) => Err(SettingError::Syntax),
            (Kind::Monitor, Value::Text(s)) => Ok(Value::Text(MonitorChoice::parse(&s).to_text())),
            (Kind::Monitor, _) => Err(SettingError::Syntax),
            (Kind::Pick, Value::Text(s)) if !s.chars().any(char::is_control) => {
                Ok(Value::Text(s.trim().to_owned()))
            }
            (Kind::Pick, Value::Text(_)) => Err(SettingError::BadText),
            (Kind::Pick, _) => Err(SettingError::Syntax),
            (Kind::List { options, repeat }, Value::List(items)) => {
                let mut out: Vec<String> = Vec::with_capacity(items.len());
                for item in items {
                    let item = item.trim().to_ascii_lowercase().replace('-', "_");
                    if !options.contains(&item.as_str()) {
                        return Err(SettingError::UnknownOption { options });
                    }
                    if repeat.contains(&item.as_str()) || !out.contains(&item) {
                        out.push(item);
                    }
                }
                Ok(Value::List(out))
            }
            (Kind::List { .. }, Value::Text(s)) => self.validate(Value::List(
                s.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect(),
            )),
            (Kind::List { options, .. }, _) => Err(SettingError::UnknownOption { options }),
            (Kind::Action, _) => Err(SettingError::NotSettable),
        }
    }

    /// Parses command-line / palette text (`--set key=value`) for this kind.
    pub fn parse(&self, text: &str) -> Result<Value, SettingError> {
        let t = text.trim();
        let v = match self {
            Kind::Bool => {
                let l = t.to_ascii_lowercase();
                if TRUE.contains(&l.as_str()) {
                    Value::Bool(true)
                } else if FALSE.contains(&l.as_str()) {
                    Value::Bool(false)
                } else {
                    return Err(SettingError::NotBool);
                }
            }
            Kind::Int { auto, .. } => {
                if *auto && t.eq_ignore_ascii_case("auto") {
                    Value::Int(0)
                } else {
                    let digits = t.trim_end_matches(['%', ' ']).replace('_', "");
                    Value::Int(digits.parse().map_err(|_| SettingError::NotInteger)?)
                }
            }
            Kind::Float { .. } => Value::Float(t.parse().map_err(|_| SettingError::NotNumber)?),
            Kind::Action => return Err(SettingError::NotSettable),
            _ => Value::Text(t.to_owned()),
        };
        self.validate(v)
    }

    /// Reads a TOML value for this kind.
    pub fn from_toml(&self, item: &toml_edit::Value) -> Result<Value, SettingError> {
        use toml_edit::Value as T;
        let v = match item {
            T::Boolean(b) => Value::Bool(*b.value()),
            T::Integer(i) => Value::Int(*i.value()),
            T::Float(f) => Value::Float(*f.value()),
            T::String(s) => {
                // Strings are accepted for numbers and booleans too ("auto", "on").
                return match self {
                    Kind::Bool | Kind::Int { .. } | Kind::Float { .. } => self.parse(s.value()),
                    _ => self.validate(Value::Text(s.value().clone())),
                };
            }
            T::Array(a) => Value::List(
                a.iter()
                    .map(|v| v.as_str().map(str::to_owned).ok_or(SettingError::Syntax))
                    .collect::<Result<_, _>>()
                    .map_err(|_| match self {
                        Kind::List { options, .. } => SettingError::UnknownOption { options },
                        _ => SettingError::Syntax,
                    })?,
            ),
            _ => {
                return Err(match self {
                    Kind::Bool => SettingError::NotBool,
                    Kind::Int { .. } => SettingError::NotInteger,
                    Kind::Float { .. } => SettingError::NotNumber,
                    Kind::Color => SettingError::BadColour,
                    Kind::Path { .. } => SettingError::BadPath,
                    Kind::Language | Kind::Locale => SettingError::BadLanguage,
                    Kind::Enum { options } | Kind::List { options, .. } => {
                        SettingError::UnknownOption { options }
                    }
                    Kind::Shortcut | Kind::Text | Kind::Monitor | Kind::Pick => {
                        SettingError::Syntax
                    }
                    Kind::Action => SettingError::NotSettable,
                });
            }
        };
        self.validate(v)
    }

    /// Converts a valid value to TOML.
    pub fn to_toml(&self, v: &Value) -> toml_edit::Value {
        match v {
            Value::Bool(b) => (*b).into(),
            Value::Int(i) => (*i).into(),
            Value::Float(x) => (*x).into(),
            Value::Text(s) => s.as_str().into(),
            Value::Color(c) => c.to_string().into(),
            Value::Path(p) => p
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default()
                .into(),
            Value::List(l) => {
                let mut a = toml_edit::Array::new();
                for s in l {
                    a.push(s.as_str());
                }
                toml_edit::Value::Array(a)
            }
            Value::None => "".into(),
        }
    }
}

// ----- Typed values -------------------------------------------------------

/// Conversion between a typed setting field and [`Value`].
pub trait SettingValue: Clone + PartialEq + Sized {
    /// The kind used when the schema doesn't give one.
    fn default_kind() -> Kind;
    fn to_value(&self) -> Value;
    /// Converts a value that already passed [`Kind::validate`].
    fn from_value(v: &Value) -> Option<Self>;
}

impl SettingValue for bool {
    fn default_kind() -> Kind {
        Kind::Bool
    }
    fn to_value(&self) -> Value {
        Value::Bool(*self)
    }
    fn from_value(v: &Value) -> Option<Self> {
        match v {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

impl SettingValue for i64 {
    fn default_kind() -> Kind {
        Kind::Int {
            min: i64::MIN,
            max: i64::MAX,
            step: 1,
            unit: Unit::None,
            auto: false,
        }
    }
    fn to_value(&self) -> Value {
        Value::Int(*self)
    }
    fn from_value(v: &Value) -> Option<Self> {
        match v {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }
}

impl SettingValue for f64 {
    fn default_kind() -> Kind {
        Kind::Float {
            min: f64::MIN,
            max: f64::MAX,
            step: 0.1,
            unit: Unit::None,
        }
    }
    fn to_value(&self) -> Value {
        Value::Float(*self)
    }
    fn from_value(v: &Value) -> Option<Self> {
        match v {
            Value::Float(x) => Some(*x),
            Value::Int(i) => Some(*i as f64),
            _ => None,
        }
    }
}

impl SettingValue for Color {
    fn default_kind() -> Kind {
        Kind::Color
    }
    fn to_value(&self) -> Value {
        Value::Color(*self)
    }
    fn from_value(v: &Value) -> Option<Self> {
        match v {
            Value::Color(c) => Some(*c),
            _ => None,
        }
    }
}

impl SettingValue for Option<PathBuf> {
    fn default_kind() -> Kind {
        Kind::Path { directory: false }
    }
    fn to_value(&self) -> Value {
        Value::Path(self.clone())
    }
    fn from_value(v: &Value) -> Option<Self> {
        match v {
            Value::Path(p) => Some(p.clone()),
            _ => None,
        }
    }
}

impl SettingValue for LanguagePreference {
    fn default_kind() -> Kind {
        Kind::Language
    }
    fn to_value(&self) -> Value {
        Value::Text(self.to_string())
    }
    fn from_value(v: &Value) -> Option<Self> {
        match v {
            Value::Text(s) => s.parse().ok(),
            _ => None,
        }
    }
}

impl SettingValue for RegionFormats {
    fn default_kind() -> Kind {
        Kind::Locale
    }
    fn to_value(&self) -> Value {
        Value::Text(match self {
            RegionFormats::FollowLanguage => "language".into(),
            RegionFormats::Locale(l) => l.to_string(),
        })
    }
    fn from_value(v: &Value) -> Option<Self> {
        match v {
            Value::Text(s) if s == "language" => Some(RegionFormats::FollowLanguage),
            Value::Text(s) => s.parse().ok().map(RegionFormats::Locale),
            _ => None,
        }
    }
}

impl SettingValue for Shortcut {
    fn default_kind() -> Kind {
        Kind::Shortcut
    }
    fn to_value(&self) -> Value {
        Value::Text(self.0.clone())
    }
    fn from_value(v: &Value) -> Option<Self> {
        match v {
            Value::Text(s) => Some(Shortcut(s.clone())),
            _ => None,
        }
    }
}

impl SettingValue for String {
    fn default_kind() -> Kind {
        Kind::Text
    }
    fn to_value(&self) -> Value {
        Value::Text(self.clone())
    }
    fn from_value(v: &Value) -> Option<Self> {
        match v {
            Value::Text(s) => Some(s.clone()),
            _ => None,
        }
    }
}

impl SettingValue for Action {
    fn default_kind() -> Kind {
        Kind::Action
    }
    fn to_value(&self) -> Value {
        Value::None
    }
    fn from_value(_: &Value) -> Option<Self> {
        Some(Action)
    }
}

/// A Rust enum whose variants are a setting's options. Implemented by [`setting_enum!`].
pub trait SettingEnum: Copy + PartialEq + Sized + 'static {
    /// TOML names of the variants, in display order.
    const OPTIONS: &'static [&'static str];
    /// All variants, in the same order as [`OPTIONS`](Self::OPTIONS).
    const ALL: &'static [Self];
    fn as_str(self) -> &'static str;
    fn from_name(s: &str) -> Option<Self> {
        Self::OPTIONS
            .iter()
            .position(|o| *o == s)
            .map(|i| Self::ALL[i])
    }
}

impl<T: SettingEnum> SettingValue for Vec<T> {
    fn default_kind() -> Kind {
        Kind::List {
            options: T::OPTIONS,
            repeat: &[],
        }
    }
    fn to_value(&self) -> Value {
        Value::List(self.iter().map(|v| v.as_str().to_owned()).collect())
    }
    fn from_value(v: &Value) -> Option<Self> {
        match v {
            Value::List(l) => l.iter().map(|s| T::from_name(s)).collect(),
            _ => None,
        }
    }
}

/// Declares a setting enum: `snake_case` TOML names, [`SettingEnum`], [`SettingValue`],
/// `Display` and `FromStr`.
///
/// ```
/// specular_settings::setting_enum! {
///     /// Example.
///     pub enum Speed { Slow = "slow", Fast = "fast" }
/// }
/// assert_eq!("fast".parse::<Speed>().unwrap(), Speed::Fast);
/// ```
#[macro_export]
macro_rules! setting_enum {
    ($(#[$meta:meta])* $vis:vis enum $Name:ident { $($Variant:ident = $str:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $vis enum $Name { $($Variant),+ }

        impl $crate::SettingEnum for $Name {
            const OPTIONS: &'static [&'static str] = &[$($str),+];
            const ALL: &'static [Self] = &[$(Self::$Variant),+];
            fn as_str(self) -> &'static str {
                match self { $(Self::$Variant => $str),+ }
            }
        }

        impl $crate::SettingValue for $Name {
            fn default_kind() -> $crate::Kind {
                $crate::Kind::Enum { options: <Self as $crate::SettingEnum>::OPTIONS }
            }
            fn to_value(&self) -> $crate::Value {
                $crate::Value::Text($crate::SettingEnum::as_str(*self).to_owned())
            }
            fn from_value(v: &$crate::Value) -> Option<Self> {
                match v {
                    $crate::Value::Text(s) => <Self as $crate::SettingEnum>::from_name(s),
                    _ => None,
                }
            }
        }

        impl ::std::fmt::Display for $Name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str($crate::SettingEnum::as_str(*self))
            }
        }

        impl ::std::str::FromStr for $Name {
            type Err = $crate::SettingError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                <Self as $crate::SettingEnum>::from_name(&s.trim().to_ascii_lowercase().replace('-', "_"))
                    .ok_or($crate::SettingError::UnknownOption {
                        options: <Self as $crate::SettingEnum>::OPTIONS,
                    })
            }
        }
    };
}
