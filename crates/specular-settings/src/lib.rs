//! Specular's settings.
//!
//! * **One schema.** [`schema`] declares every setting once with the [`settings!`] macro:
//!   name, type, default, and optionally kind (range, unit), restart requirement and
//!   whether it is stored. The macro generates the typed [`Settings`] struct (one field per
//!   section), the [`SettingMeta`] table used by the UI, the palette, `--set` and the file
//!   loader, and get/set by key with validation.
//! * **Change notification.** [`SettingsStore`] publishes each batch of changes with a new
//!   [`generation`](SettingsStore::generation); [`changes_since`](SettingsStore::changes_since)
//!   gives the changed keys. Poll the counter once per frame; no callbacks, no locks held.
//! * **`settings.toml`.** Edited with `toml_edit`, so comments, order and formatting survive.
//!   Unknown keys are kept and reported as [`Warning`]s, invalid values are reported with the
//!   key and reason and the default is used, the file is never replaced by a broken or
//!   default one, and writes are atomic (temporary file + rename). A top-level `version`
//!   drives [`Migration`]s.
//! * **Live reload.** [`SettingsStore::watch`] debounces file events and reloads.
//! * **Location.** The platform config folder, or next to the executable when a file named
//!   `portable` or `specular.toml` is there ([`Locations`]).
//! * **User commands.** `[[commands]]` tables (external programs run with the current file)
//!   are kept out of the typed schema and read with [`SettingsStore::user_commands`].
//! * **Undo** for Settings-window changes, **search** over localised and English labels
//!   ([`search`]), **import/export** of one TOML file, and **`--set key=value`**
//!   ([`parse_set_arg`]).
//!
//! Message ids: `settings-<section>-<name>` (label), `settings-<section>-<name>-help`,
//! `settings-<section>-<name>-<option>` (choices), `settings-section-<section>`, with `_`
//! written as `-`. They live in `specular-i18n`'s catalogues.

#[doc(hidden)]
pub use log as __log;

#[macro_use]
mod value;
#[macro_use]
mod macros;
pub mod bundle;
pub mod commands;
mod error;
mod file;
pub mod lint;
mod meta;
pub mod paths;
pub mod policy;
pub mod schema;
mod search;
mod store;
mod undo;

pub use bundle::{ImportChange, ImportPreview, export_bundle, is_secret, preview_import};
pub use commands::{CommandProblem, Hook, HookProblem, Load, Recipe, UserCommand, WhenValue};
pub use error::{SettingError, SettingsError, Warning};
pub use file::{
    Loaded, MIGRATIONS, Migration, SCHEMA_VERSION, load_str, load_str_with, new_document,
    to_full_document, update_document, write_atomic,
};
pub use lint::{Diagnostic, Problem, Severity, lint};
pub use meta::{
    SettingMeta, TextCheck, all, find, in_section, levenshtein, set_text_check, suggest,
};
pub use paths::{Locations, PORTABLE_MARKER, PORTABLE_SETTINGS_FILE, SETTINGS_FILE};
pub use schema::*;
pub use search::search;
pub use store::{Change, Origin, ReloadHook, SettingsStore, parse_set_arg};
pub use undo::Edit;
pub use value::{
    Action, Color, Kind, MonitorChoice, RegionFormats, SettingEnum, SettingValue, Shortcut, Unit,
    Value,
};

#[cfg(test)]
mod tests;
