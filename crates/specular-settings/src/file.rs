//! `settings.toml`: loading with diagnostics, schema migrations, comment-preserving updates
//! and atomic writes.

use std::io::Write as _;
use std::ops::Range;
use std::path::Path;

use toml_edit::{DocumentMut, Item, Table};

use crate::error::{SettingError, Warning};
use crate::meta::{self, SettingMeta};
use crate::schema::{Section, Settings};
use crate::value::Value;

/// Current schema version, written as the top-level `version` key.
pub const SCHEMA_VERSION: i64 = 5;

/// Upgrades a document written by an older schema version.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    /// Version the migration upgrades from (to `from + 1`).
    pub from: i64,
    /// Edits the document in place (rename keys, convert values). Comments on untouched keys
    /// are kept by `toml_edit`.
    pub migrate: fn(&mut DocumentMut),
}

/// Migrations of the shipped schema, oldest first.
pub static MIGRATIONS: &[Migration] = &[
    Migration {
        from: 1,
        migrate: path_overrides,
    },
    Migration {
        from: 2,
        migrate: start_screen,
    },
    Migration {
        from: 3,
        migrate: maps_profile,
    },
    Migration {
        from: 4,
        migrate: optional_decoders,
    },
];

/// Version 5: the DWG converter and OpenCASCADE are optional decoders.
/// `viewing.dwg_converter`, `viewing.dwg_converter_path`, `viewing.occt` and
/// `viewing.occt_path` become `decoders.dwg`, `decoders.dwg_path`, `decoders.occt` and
/// `decoders.occt_path`. A choice written in the file is kept (the converter was off by
/// default, so a written `false` was a choice); without one each is used when found.
fn optional_decoders(doc: &mut DocumentMut) {
    let Some(viewing) = doc.get_mut("viewing").and_then(Item::as_table_like_mut) else {
        return;
    };
    let moved: Vec<(&str, Item)> = [
        ("dwg_converter", "dwg"),
        ("dwg_converter_path", "dwg_path"),
        ("occt", "occt"),
        ("occt_path", "occt_path"),
    ]
    .into_iter()
    .filter_map(|(old, new)| viewing.remove(old).map(|v| (new, v)))
    .collect();
    if moved.is_empty() {
        return;
    }
    if doc.get("decoders").is_none() {
        doc.insert("decoders", Item::Table(Table::new()));
    }
    let Some(decoders) = doc.get_mut("decoders").and_then(Item::as_table_like_mut) else {
        return;
    };
    for (key, v) in moved {
        if decoders.get(key).is_none() {
            decoders.insert(key, v);
        }
    }
}

/// Version 4: maps have a profile of their own. Until now vector maps were Engineering's
/// drawings, so a profile with Engineering also gets Maps and nothing it had disappears.
fn maps_profile(doc: &mut DocumentMut) {
    let Some(profile) = doc.get_mut("profile").and_then(Item::as_table_like_mut) else {
        return;
    };
    let Some(list) = profile
        .get_mut("capabilities")
        .and_then(Item::as_value_mut)
        .and_then(toml_edit::Value::as_array_mut)
    else {
        return;
    };
    let has = |id: &str| list.iter().any(|v| v.as_str() == Some(id));
    if has("engineering") && !has("maps") {
        list.push("maps");
    }
}

/// Version 3: `general.on_start` chooses between the start screen, the gallery of the last
/// folder and the library. Its old choices (an empty window, the last image, the Open dialog)
/// were never acted on, so they become the start screen, the window they all showed.
fn start_screen(doc: &mut DocumentMut) {
    let Some(general) = doc.get_mut("general").and_then(Item::as_table_like_mut) else {
        return;
    };
    let Some(v) = general.get_mut("on_start").and_then(Item::as_value_mut) else {
        return;
    };
    if matches!(v.as_str(), Some("empty" | "last_image" | "open_dialog")) {
        let decor = v.decor().clone();
        *v = toml_edit::Value::from("start_screen");
        *v.decor_mut() = decor;
    }
}

/// Version 2: `performance.fast_yuv_upload` and `performance.hardware_decode` became
/// overrides of the automatic CPU/GPU path choice ("auto" / "on" / "off"). On stays on for
/// hardware decoding (it was opt-in); colour conversion on the GPU was the default, so only
/// "off" is kept.
fn path_overrides(doc: &mut DocumentMut) {
    let Some(perf) = doc.get_mut("performance").and_then(Item::as_table_like_mut) else {
        return;
    };
    for (key, was_on, was_off) in [
        ("fast_yuv_upload", "auto", "off"),
        ("hardware_decode", "on", "auto"),
    ] {
        let Some(v) = perf.get_mut(key).and_then(Item::as_value_mut) else {
            continue;
        };
        if let Some(b) = v.as_bool() {
            let decor = v.decor().clone();
            *v = toml_edit::Value::from(if b { was_on } else { was_off });
            *v.decor_mut() = decor;
        }
    }
}

/// The result of reading a settings document.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub settings: Settings,
    /// The document, with migrations applied. `None` when it didn't parse.
    pub doc: Option<DocumentMut>,
    pub warnings: Vec<Warning>,
    /// The document was migrated and should be written back.
    pub migrated: bool,
    /// Where in the text each of [`warnings`](Self::warnings) is (same order; byte ranges),
    /// when known. For editors that underline problems in place.
    pub spans: Vec<Option<Range<usize>>>,
}

/// The byte range of an item in a parsed document.
pub(crate) fn item_span(item: &Item) -> Option<Range<usize>> {
    match item {
        Item::None => None,
        Item::Value(v) => v.span(),
        Item::Table(t) => t.span(),
        Item::ArrayOfTables(a) => a.span(),
    }
}

/// The span of `top` (a top-level key) or of `top.name`: the key itself, or its value when
/// `value` is set (falling back to the key).
pub(crate) fn find_span(
    root: &Table,
    top: &str,
    name: Option<&str>,
    value: bool,
) -> Option<Range<usize>> {
    let (key, item) = root.get_key_value(top)?;
    let (key, item) = match name {
        None => (key, item),
        Some(n) => item.as_table_like()?.get_key_value(n)?,
    };
    let k = key.span().or_else(|| item_span(item));
    if value { item_span(item).or(k) } else { k }
}

const HEADER: &str = "\
# Specular settings. You can edit this file while Specular is running; changes are picked up
# when you save. Settings left out use their defaults. Comments and order are kept.
";

/// A new, empty settings document.
pub fn new_document() -> DocumentMut {
    let mut doc = DocumentMut::new();
    doc.insert("version", toml_edit::value(SCHEMA_VERSION));
    if let Some(mut key) = doc.key_mut("version") {
        key.leaf_decor_mut().set_prefix(HEADER);
    }
    doc
}

/// Sets the top-level `version`, keeping the comments around it.
fn set_version(doc: &mut DocumentMut, version: i64) {
    match doc.get_mut("version").and_then(Item::as_value_mut) {
        Some(v) => {
            let decor = v.decor().clone();
            *v = toml_edit::Value::from(version);
            *v.decor_mut() = decor;
        }
        None => {
            doc.insert("version", toml_edit::value(version));
        }
    }
}

/// Reads settings from TOML text with the shipped migrations.
pub fn load_str(text: &str) -> Loaded {
    load_str_with(text, MIGRATIONS, SCHEMA_VERSION)
}

/// Reads settings from TOML text, upgrading from older versions with `migrations` up to
/// `current`. Never fails: problems become [`Warning`]s and the defaults are used.
pub fn load_str_with(text: &str, migrations: &[Migration], current: i64) -> Loaded {
    let mut settings = Settings::default();
    let mut warnings = Vec::new();
    let mut spans = Vec::new();
    // Parsed once with spans (for diagnostics), then turned into an editable document.
    let raw = match toml_edit::Document::parse(text) {
        Ok(d) => d,
        Err(e) => {
            let start = e.span().map_or(0, |s| s.start.min(text.len()));
            let line = text.get(..start).map_or(0, |t| t.matches('\n').count()) + 1;
            warnings.push(Warning::Syntax {
                line,
                reason: e.message().trim().to_owned(),
            });
            spans.push(e.span());
            return Loaded {
                settings,
                doc: None,
                warnings,
                migrated: false,
                spans,
            };
        }
    };
    let root = raw.as_table();
    let mut doc: DocumentMut = raw.clone().into_mut();

    // Schema version and migrations.
    let mut migrated = false;
    let version = match doc.get("version") {
        None => current,
        Some(item) => match item.as_integer() {
            Some(v) => v,
            None => {
                warnings.push(Warning::InvalidValue {
                    key: "version".into(),
                    reason: SettingError::NotInteger,
                });
                spans.push(find_span(root, "version", None, true));
                current
            }
        },
    };
    if version > current {
        warnings.push(Warning::NewerVersion {
            found: version,
            supported: current,
        });
        spans.push(find_span(root, "version", None, true));
    } else if version < current {
        let mut v = version;
        for m in migrations
            .iter()
            .filter(|m| m.from >= version && m.from < current)
        {
            if m.from != v {
                log::warn!(
                    "settings migration gap: at version {v}, next migrates from {}",
                    m.from
                );
            }
            (m.migrate)(&mut doc);
            v = m.from + 1;
        }
        log::info!("settings.toml migrated from version {version} to {current}");
        set_version(&mut doc, current);
        migrated = true;
    }

    // Values.
    for (top_key, item) in doc.iter() {
        // `[[commands]]` and `[[hooks]]` are read separately (`commands::parse_commands`,
        // `commands::parse_hooks`).
        if top_key == "version"
            || top_key == crate::commands::COMMANDS_KEY
            || top_key == crate::commands::HOOKS_KEY
        {
            continue;
        }
        let Some(section) = Section::from_key(top_key) else {
            warnings.push(Warning::UnknownKey {
                key: top_key.to_owned(),
            });
            spans.push(find_span(root, top_key, None, false));
            continue;
        };
        let Some(table) = item.as_table_like() else {
            warnings.push(Warning::UnknownKey {
                key: top_key.to_owned(),
            });
            spans.push(find_span(root, top_key, None, false));
            continue;
        };
        for (name, item) in table.iter() {
            let key = format!("{}.{name}", section.key());
            let Some(m) = meta::find(&key) else {
                warnings.push(Warning::UnknownKey { key });
                spans.push(find_span(root, top_key, Some(name), false));
                continue;
            };
            if !m.stored {
                warnings.push(Warning::InvalidValue {
                    key,
                    reason: SettingError::NotSettable,
                });
                spans.push(find_span(root, top_key, Some(name), false));
                continue;
            }
            let parsed = match item.as_value() {
                Some(v) => m.kind.from_toml(v),
                None => Err(type_error(m)),
            };
            match parsed {
                Ok(v) => {
                    settings.put(m.key, &v);
                }
                Err(reason) => {
                    warnings.push(Warning::InvalidValue { key, reason });
                    spans.push(find_span(root, top_key, Some(name), true));
                }
            }
        }
    }
    debug_assert_eq!(warnings.len(), spans.len());
    Loaded {
        settings,
        doc: Some(doc),
        warnings,
        migrated,
        spans,
    }
}

fn type_error(m: &SettingMeta) -> SettingError {
    m.kind
        .from_toml(&toml_edit::Value::from(toml_edit::InlineTable::new()))
        .err()
        .unwrap_or(SettingError::Syntax)
}

/// Writes `keys` of `settings` into `doc`, keeping comments, order and formatting of
/// everything else. A key whose value equals the default is written only if the file
/// already has it (so files stay short, and a user's commented line keeps its comment).
pub fn update_document(doc: &mut DocumentMut, settings: &Settings, keys: &[&str]) {
    for key in keys {
        let Some(m) = meta::find(key) else { continue };
        if !m.stored {
            continue;
        }
        let Some(value) = settings.get(m.key) else {
            continue;
        };
        let new = m.kind.to_toml(&value);
        let section = m.section.key();
        let present = doc
            .get(section)
            .and_then(Item::as_table_like)
            .is_some_and(|t| t.contains_key(m.name));
        if !present && value == m.default {
            continue;
        }
        if !doc.contains_key(section) || !doc[section].is_table_like() {
            let mut t = Table::new();
            t.set_implicit(false);
            doc.insert(section, Item::Table(t));
        }
        let table = doc[section]
            .as_table_like_mut()
            .expect("section is a table");
        match table.get_mut(m.name).and_then(Item::as_value_mut) {
            Some(existing) => {
                let decor = existing.decor().clone();
                *existing = new;
                *existing.decor_mut() = decor;
            }
            None => {
                table.insert(m.name, Item::Value(new));
            }
        }
    }
}

/// A complete settings file with every stored setting (for export).
pub fn to_full_document(settings: &Settings) -> DocumentMut {
    let mut doc = new_document();
    for section in Section::ALL {
        let mut table = Table::new();
        for m in meta::in_section(*section).filter(|m| m.stored) {
            let value = settings.get(m.key).unwrap_or(Value::None);
            table.insert(m.name, Item::Value(m.kind.to_toml(&value)));
        }
        if !table.is_empty() {
            doc.insert(section.key(), Item::Table(table));
        }
    }
    doc
}

/// Writes `text` to `path` atomically: a temporary file in the same folder is written,
/// flushed to disk and renamed over the target, so readers see the old or the new file,
/// never a partial one.
pub fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let mut tmp = tempfile::Builder::new()
        .prefix(".settings-")
        .suffix(".tmp")
        .tempfile_in(dir)?;
    tmp.write_all(text.as_bytes())?;
    tmp.as_file().sync_all()?;
    if let Ok(meta) = std::fs::metadata(path) {
        // Keep the user's permissions (e.g. a read-only-for-others file).
        let _ = std::fs::set_permissions(tmp.path(), meta.permissions());
    }
    // std's rename, not `persist`: on Windows it replaces with POSIX semantics, so readers
    // that have the file open (the settings watcher, another window) keep the old file
    // instead of making the replace fail with "Access is denied", as MoveFileEx does.
    let tmp = tmp.into_temp_path();
    std::fs::rename(&tmp, path)?;
    // Renamed away: nothing left for the guard to delete.
    let _ = tmp.keep();
    Ok(())
}
