//! Validation errors and load warnings, each with a message id for the UI.

use std::fmt;
use std::path::PathBuf;

use specular_i18n::{FluentArgs, I18n, Localizer};

/// Why a value was rejected. Shown inline in the Settings window, by `specular --set`, and in
/// load warnings. Use [`SettingError::message`] for localised text; `Display` is English.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingError {
    UnknownKey {
        key: String,
        suggestion: Option<&'static str>,
    },
    NotBool,
    NotInteger,
    NotNumber,
    OutOfRange {
        min: String,
        max: String,
        /// 0 also allowed ("automatic").
        auto: bool,
    },
    UnknownOption {
        options: &'static [&'static str],
    },
    BadColour,
    BadLanguage,
    BadPath,
    /// The entry is an action (a button), not a value.
    NotSettable,
    /// `--set` text wasn't `key=value`.
    Syntax,
    /// Text with a tab, a line break or another control character.
    BadText,
    /// Refused by the check of whoever reads the setting ([`crate::set_text_check`]): its
    /// reason, already in the UI language.
    Invalid(String),
}

impl std::error::Error for SettingError {}

impl SettingError {
    /// The Fluent message id for this error.
    pub fn message_id(&self) -> &'static str {
        match self {
            SettingError::UnknownKey {
                suggestion: Some(_),
                ..
            } => "settings-error-unknown-key-suggestion",
            SettingError::UnknownKey { .. } => "settings-error-unknown-key",
            SettingError::NotBool => "settings-error-not-bool",
            SettingError::NotInteger => "settings-error-not-integer",
            SettingError::NotNumber => "settings-error-not-number",
            SettingError::OutOfRange { auto: true, .. } => "settings-error-out-of-range-auto",
            SettingError::OutOfRange { .. } => "settings-error-out-of-range",
            SettingError::UnknownOption { .. } => "settings-error-unknown-option",
            SettingError::BadColour => "settings-error-bad-colour",
            SettingError::BadLanguage => "settings-error-bad-language",
            SettingError::BadPath => "settings-error-bad-path",
            SettingError::NotSettable => "settings-error-not-settable",
            SettingError::Syntax => "settings-error-syntax",
            SettingError::BadText => "settings-error-bad-text",
            SettingError::Invalid(_) => "settings-error-invalid",
        }
    }

    /// The message arguments (keys, ranges and options are not translated).
    pub fn args(&self) -> FluentArgs<'static> {
        let mut args = FluentArgs::new();
        match self {
            SettingError::UnknownKey { key, suggestion } => {
                args.set("key", key.clone());
                if let Some(s) = suggestion {
                    args.set("suggestion", *s);
                }
            }
            SettingError::OutOfRange { min, max, .. } => {
                args.set("min", min.clone());
                args.set("max", max.clone());
            }
            SettingError::UnknownOption { options } => {
                args.set("options", options.join(", "));
            }
            SettingError::Invalid(reason) => args.set("reason", reason.clone()),
            _ => {}
        }
        args
    }

    /// Localised text.
    pub fn message(&self, i18n: &I18n) -> String {
        i18n.localizer().tr_args(self.message_id(), &self.args())
    }
}

impl fmt::Display for SettingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = Localizer::english().tr_args(self.message_id(), &self.args());
        f.write_str(&specular_i18n::strip_isolates(&s))
    }
}

/// A problem found while loading `settings.toml`. Loading never fails because of these:
/// the affected setting keeps its default and the file is left as it is.
#[derive(Debug, Clone, PartialEq)]
pub enum Warning {
    /// A key this version doesn't know. It stays in the file.
    UnknownKey { key: String },
    /// A value that failed validation; the default is used and the file is not changed.
    InvalidValue { key: String, reason: SettingError },
    /// The file was written by a newer schema version.
    NewerVersion { found: i64, supported: i64 },
    /// The file isn't valid TOML; defaults are used and it is not written until fixed.
    Syntax { line: usize, reason: String },
}

impl Warning {
    pub fn message_id(&self) -> &'static str {
        match self {
            Warning::UnknownKey { .. } => "settings-warning-unknown-key",
            Warning::InvalidValue { .. } => "settings-warning-invalid-value",
            Warning::NewerVersion { .. } => "settings-warning-newer-version",
            Warning::Syntax { .. } => "settings-warning-syntax",
        }
    }

    /// The setting key this warning is about, for inline display in the window.
    pub fn key(&self) -> Option<&str> {
        match self {
            Warning::UnknownKey { key } | Warning::InvalidValue { key, .. } => Some(key),
            _ => None,
        }
    }

    fn render(&self, loc: &Localizer) -> String {
        let mut args = FluentArgs::new();
        match self {
            Warning::UnknownKey { key } => args.set("key", key.clone()),
            Warning::InvalidValue { key, reason } => {
                args.set("key", key.clone());
                args.set("reason", loc.tr_args(reason.message_id(), &reason.args()));
            }
            Warning::NewerVersion { .. } => {}
            Warning::Syntax { line, reason } => {
                args.set("line", *line);
                args.set("reason", reason.clone());
            }
        }
        loc.tr_args(self.message_id(), &args)
    }

    /// Localised text.
    pub fn message(&self, i18n: &I18n) -> String {
        self.render(&i18n.localizer())
    }
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&specular_i18n::strip_isolates(
            &self.render(&Localizer::english()),
        ))
    }
}

/// I/O failures reading or writing settings files.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path} has errors and is not overwritten until they are fixed")]
    FileHasErrors { path: PathBuf },
    #[error("cannot import {path}: {reason}")]
    Import { path: PathBuf, reason: String },
    #[error("cannot watch {path}: {reason}")]
    Watch { path: PathBuf, reason: String },
}
