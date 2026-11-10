//! "Export settings…" and "Import settings…": one TOML file with every stored setting and the
//! keymap's changes (`[keymap]`, the `keymap.toml` text moved under one table).
//!
//! Importing never applies anything blind: [`preview_import`] reads the file into a list of
//! the settings that would change (from what to what), what was left out and why, and the
//! keymap text; the app shows that and applies it with [`SettingsStore::apply_import`] only
//! when the user confirms. Secrets are never exported or imported: any key whose name looks
//! like a password, token or other credential is dropped wherever it is, and user commands
//! and hooks (`[[commands]]`, `[[hooks]]`: programs the app would start) are not imported
//! either.

use toml_edit::{DocumentMut, Item, Table};

use crate::commands::{COMMANDS_KEY, HOOKS_KEY};
use crate::error::Warning;
use crate::file::{load_str, to_full_document};
use crate::meta::{self, SettingMeta};
use crate::schema::Settings;
use crate::store::{Origin, SettingsStore};
use crate::value::Value;

/// The table the keymap's changes travel in.
pub const KEYMAP_KEY: &str = "keymap";

/// Name parts that mark a secret. Matched case-insensitively against every key, at any depth.
const SECRET_WORDS: &[&str] = &[
    "password",
    "passwd",
    "passphrase",
    "secret",
    "token",
    "api_key",
    "apikey",
    "credential",
    "private_key",
    "cookie",
];

/// Whether a key name looks like it holds a secret.
pub fn is_secret(name: &str) -> bool {
    let n = name.to_ascii_lowercase().replace('-', "_");
    SECRET_WORDS.iter().any(|w| n.contains(w))
}

/// Removes secret-looking keys from `table` and its subtables; returns their dotted paths.
fn strip_secrets(table: &mut dyn toml_edit::TableLike, path: &str, out: &mut Vec<String>) {
    let names: Vec<String> = table.iter().map(|(k, _)| k.to_owned()).collect();
    for name in names {
        let full = if path.is_empty() {
            name.clone()
        } else {
            format!("{path}.{name}")
        };
        if is_secret(&name) {
            table.remove(&name);
            out.push(full);
            continue;
        }
        match table.get_mut(&name) {
            Some(Item::Table(t)) => strip_secrets(t, &full, out),
            Some(Item::Value(toml_edit::Value::InlineTable(t))) => strip_secrets(t, &full, out),
            Some(Item::ArrayOfTables(a)) => {
                for t in a.iter_mut() {
                    strip_secrets(t, &full, out);
                }
            }
            _ => {}
        }
    }
}

/// The export file: every stored setting, then the keymap's changes when there are any
/// (`keymap` is the text of `keymap.toml`; `None` or a file with no bindings leaves the table
/// out). Secret-looking keys are left out.
pub fn export_bundle(settings: &Settings, keymap: Option<&str>) -> String {
    let mut doc = to_full_document(settings);
    if let Some(k) = keymap
        && let Ok(kd) = k.parse::<DocumentMut>()
        && !kd.as_table().is_empty()
    {
        let mut t = Table::new();
        for (name, item) in kd.as_table().iter() {
            let mut item = item.clone();
            if let Item::Table(sub) = &mut item {
                sub.decor_mut().clear();
            }
            t.insert(name, item);
        }
        doc.insert(KEYMAP_KEY, Item::Table(t));
    }
    let mut dropped = Vec::new();
    strip_secrets(doc.as_table_mut(), "", &mut dropped);
    doc.to_string()
}

/// One setting an import would change.
#[derive(Debug, Clone)]
pub struct ImportChange {
    pub meta: &'static SettingMeta,
    pub before: Value,
    pub after: Value,
}

/// What an import file would do, for the preview.
#[derive(Debug, Clone, Default)]
pub struct ImportPreview {
    /// Settings whose value would change, in schema order.
    pub changes: Vec<ImportChange>,
    /// Entries left out: unknown keys, invalid values (with the reason), a newer version.
    pub problems: Vec<Warning>,
    /// Secret-looking keys that were in the file and are never imported (dotted paths).
    pub secrets: Vec<String>,
    /// User commands and hooks in the file (not imported: they start programs).
    pub commands: usize,
    /// The keymap's changes, as `keymap.toml` text, when the file has a `[keymap]` table.
    pub keymap: Option<String>,
}

impl ImportPreview {
    /// Nothing would change.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty() && self.keymap.is_none()
    }
}

/// Reads an export file against the settings in effect. `Err` when the text is not TOML at
/// all (the warning says where); everything else is reported in the preview.
pub fn preview_import(current: &Settings, text: &str) -> Result<ImportPreview, Warning> {
    let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| {
        let start = e.span().map_or(0, |s| s.start.min(text.len()));
        Warning::Syntax {
            line: text.get(..start).map_or(0, |t| t.matches('\n').count()) + 1,
            reason: e.message().trim().to_owned(),
        }
    })?;
    let mut p = ImportPreview::default();
    strip_secrets(doc.as_table_mut(), "", &mut p.secrets);
    for key in [COMMANDS_KEY, HOOKS_KEY] {
        if let Some(item) = doc.remove(key) {
            p.commands += item.as_array_of_tables().map_or(1, |a| a.len());
        }
    }
    if let Some(item) = doc.remove(KEYMAP_KEY) {
        match item.as_table_like() {
            Some(t) => {
                let mut kd = DocumentMut::new();
                for (name, item) in t.iter() {
                    kd.insert(name, item.clone());
                }
                p.keymap = Some(kd.to_string());
            }
            None => p.problems.push(Warning::UnknownKey {
                key: KEYMAP_KEY.to_owned(),
            }),
        }
    }
    let loaded = load_str(&doc.to_string());
    p.problems.extend(loaded.warnings);
    for m in meta::all().iter().filter(|m| m.stored) {
        let present = doc
            .get(m.section.key())
            .and_then(Item::as_table_like)
            .is_some_and(|t| t.contains_key(m.name));
        if !present || p.problems.iter().any(|w| w.key() == Some(m.key)) {
            continue;
        }
        let (Some(before), Some(after)) = (current.get(m.key), loaded.settings.get(m.key)) else {
            continue;
        };
        if before != after {
            p.changes.push(ImportChange {
                meta: m,
                before,
                after,
            });
        }
    }
    Ok(p)
}

impl SettingsStore {
    /// Applies the settings of a confirmed import preview as one undoable step (the keymap
    /// is the app's to apply). Returns the keys that changed.
    pub fn apply_import(&self, p: &ImportPreview) -> Vec<&'static str> {
        let edits = p
            .changes
            .iter()
            .map(|c| (c.meta, c.after.clone()))
            .collect();
        self.apply(edits, Origin::Import)
    }

    /// The bundle of the settings in effect and `keymap` (see [`export_bundle`]).
    pub fn export_bundle(&self, keymap: Option<&str>) -> String {
        export_bundle(&self.snapshot(), keymap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYMAP: &str =
        "preset = \"vim\"\n\n[viewer]\n\"F2\" = \"none\"\n\"Ctrl+e\" = \"rename\"\n";

    #[test]
    fn round_trip_with_keymap() {
        let mut s = Settings::default();
        s.viewing.zoom_animation = false;
        s.gallery.thumbnail_size = 256;
        s.convert.pattern = "{name}-{n:3}".into();
        let text = export_bundle(&s, Some(KEYMAP));
        assert!(text.contains("[keymap.viewer]"), "{text}");
        let p = preview_import(&Settings::default(), &text).unwrap();
        assert!(p.problems.is_empty(), "{:?}", p.problems);
        let keys: Vec<&str> = p.changes.iter().map(|c| c.meta.key).collect();
        assert_eq!(
            keys,
            [
                "viewing.zoom_animation",
                "gallery.thumbnail_size",
                "convert.pattern"
            ]
        );
        assert_eq!(p.changes[1].before, Value::Int(160));
        assert_eq!(p.changes[1].after, Value::Int(256));
        let km = p.keymap.unwrap();
        let back: DocumentMut = km.parse().unwrap();
        let want: DocumentMut = KEYMAP.parse().unwrap();
        assert_eq!(back.to_string().trim(), want.to_string().trim());

        // Applied through a store, then exported again: the same file.
        let store = SettingsStore::in_memory();
        let p = preview_import(&store.snapshot(), &text).unwrap();
        let changed = store.apply_import(&p);
        assert_eq!(changed.len(), 3);
        assert_eq!(store.snapshot().gallery.thumbnail_size, 256);
        assert_eq!(store.export_bundle(Some(KEYMAP)), text);
        // Importing what is already in effect changes nothing.
        let again = preview_import(&store.snapshot(), &text).unwrap();
        assert!(again.changes.is_empty());
        // One undo step takes the whole import back.
        assert!(store.can_undo());
        store.undo();
        assert_eq!(store.snapshot().gallery.thumbnail_size, 160);
    }

    #[test]
    fn secrets_and_commands_are_never_imported() {
        let text = "version = 2\n\
            [viewing]\nzoom_animation = false\npassword = \"hunter2\"\n\
            [dicomweb]\nserver_token = \"abc\"\n[dicomweb.auth]\nApi-Key = \"x\"\n\
            [[commands]]\nname = \"Wipe\"\nprogram = \"rm\"\n\
            [[hooks]]\nname = \"Send\"\non = [\"marked\"]\nprogram = \"curl\"\n\
            [keymap.viewer]\n\"x\" = \"trash\"\nsecret = \"no\"\n";
        let p = preview_import(&Settings::default(), text).unwrap();
        let mut secrets = p.secrets.clone();
        secrets.sort();
        assert_eq!(
            secrets,
            [
                "dicomweb.auth.Api-Key",
                "dicomweb.server_token",
                "keymap.viewer.secret",
                "viewing.password"
            ]
        );
        assert_eq!(p.commands, 2, "the command and the hook");
        assert_eq!(p.changes.len(), 1);
        assert!(!p.keymap.as_deref().unwrap().contains("secret"));
        assert!(!format!("{p:?}").contains("hunter2"));
        // The unknown `dicomweb` table is reported, not applied.
        assert!(p.problems.iter().any(|w| w.key() == Some("dicomweb")));
    }

    #[test]
    fn invalid_values_are_reported_not_applied() {
        let text = "[gallery]\nthumbnail_size = 9999\nlayout = \"sideways\"\ncaptions = false\n";
        let p = preview_import(&Settings::default(), text).unwrap();
        assert_eq!(p.changes.len(), 1);
        assert_eq!(p.changes[0].meta.key, "gallery.captions");
        let bad: Vec<_> = p.problems.iter().filter_map(Warning::key).collect();
        assert_eq!(bad, ["gallery.thumbnail_size", "gallery.layout"]);
        assert!(matches!(
            preview_import(&Settings::default(), "[gallery\n"),
            Err(Warning::Syntax { line: 1, .. })
        ));
    }

    #[test]
    fn export_never_writes_secrets() {
        let text = export_bundle(&Settings::default(), Some("[viewer]\ntoken = \"t\"\n"));
        assert!(!text.contains("token"));
        assert!(is_secret("smtp_password") && is_secret("ApiKey") && !is_secret("key_sequence"));
    }
}
