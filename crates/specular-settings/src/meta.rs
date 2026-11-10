//! Per-setting metadata derived from the schema: keys, kinds, defaults and message ids.

use std::sync::{LazyLock, RwLock};

use specular_i18n::{FluentArgs, FluentValue, I18n, LanguageIdentifier, ListKind, Localizer};

use crate::error::SettingError;
use crate::schema::{Section, build_metas};
use crate::value::{Kind, Unit, Value};

/// Everything the UI, the palette, the CLI and the file loader need to know about a setting.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingMeta {
    /// Full key, `"viewing.initial_zoom"` (TOML table and key, and the `--set` name).
    pub key: &'static str,
    pub section: Section,
    /// Key within its section, `"initial_zoom"`.
    pub name: &'static str,
    pub kind: Kind,
    pub default: Value,
    /// `settings-viewing-initial-zoom`.
    pub label_id: &'static str,
    /// `settings-viewing-initial-zoom-help`.
    pub help_id: &'static str,
    /// The change applies only after a restart (the window offers "Restart now").
    pub needs_restart: bool,
    /// Written to `settings.toml`. False for actions and for settings stored elsewhere
    /// (portable mode is the presence of a marker file).
    pub stored: bool,
    /// The operating systems (`std::env::consts::OS` names) the setting applies to; empty
    /// for all. Others neither show nor search it.
    pub os: &'static [&'static str],
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

pub(crate) fn kebab(s: &str) -> String {
    s.replace('_', "-")
}

impl SettingMeta {
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)] // called only by `settings!`, one argument per field
    pub fn new(
        section: Section,
        section_key: &'static str,
        name: &'static str,
        kind: Kind,
        default: Value,
        needs_restart: bool,
        stored: bool,
        os: &'static [&'static str],
    ) -> Self {
        let label = format!("settings-{}-{}", kebab(section_key), kebab(name));
        let stored = stored && kind != Kind::Action;
        Self {
            key: leak(format!("{section_key}.{name}")),
            section,
            name,
            help_id: leak(format!("{label}-help")),
            label_id: leak(label),
            kind,
            default,
            needs_restart,
            stored,
            os,
        }
    }

    /// Whether the setting applies on this operating system.
    pub fn applies_here(&self) -> bool {
        self.os.is_empty() || self.os.contains(&std::env::consts::OS)
    }

    /// True for buttons (`Kind::Action`).
    pub fn is_action(&self) -> bool {
        self.kind == Kind::Action
    }

    /// Message id of one option of an enum or list setting:
    /// `settings-viewing-initial-zoom-fit-if-larger`.
    pub fn option_id(&self, option: &str) -> String {
        format!("{}-{}", self.label_id, kebab(option))
    }

    /// The options of an enum or list setting.
    pub fn options(&self) -> &'static [&'static str] {
        match self.kind {
            Kind::Enum { options } | Kind::List { options, .. } => options,
            _ => &[],
        }
    }

    /// Localised label.
    pub fn label(&self, i18n: &I18n) -> String {
        tr_for_os(i18n, self.label_id)
    }

    /// Localised help text.
    pub fn help(&self, i18n: &I18n) -> String {
        tr_for_os(i18n, self.help_id)
    }

    /// Validates `value` for this setting.
    pub fn validate(&self, value: Value) -> Result<Value, SettingError> {
        self.kind.validate(value)
    }

    /// Parses text (`--set`, the palette, the window's fields) for this setting, then checks
    /// it ([`check`](Self::check)).
    pub fn parse(&self, text: &str) -> Result<Value, SettingError> {
        let v = self.kind.parse(text)?;
        self.check(&v)?;
        Ok(v)
    }

    /// The check of whoever reads this setting, when it has one ([`set_text_check`]): a
    /// pattern's placeholders, a tile server's address. Values from `settings.toml` are not
    /// checked here (a value the reader refuses falls back there, and the file stays as
    /// written).
    pub fn check(&self, v: &Value) -> Result<(), SettingError> {
        match (v, text_check(self.key)) {
            (Value::Text(s), Some(check)) => check(s).map_err(SettingError::Invalid),
            _ => Ok(()),
        }
    }

    /// Human-readable, localised value: "On", "Fit to window", "1500 ms", "Automatic",
    /// "Deutsch". Used for "Default: …", the palette ("Set: … → …") and screen readers.
    pub fn display_value(&self, value: &Value, i18n: &I18n) -> String {
        display_value(self, value, &i18n.localizer())
    }
}

/// The message `<id>-<os>` (`settings-viewing-wheel-behaviour-help-macos`) when it exists,
/// else `id`: text that differs by platform (⌘ against Ctrl) without naming the others.
fn tr_for_os(i18n: &I18n, id: &str) -> String {
    let os = format!("{id}-{}", std::env::consts::OS);
    if i18n.localizer().has_message(&os) {
        i18n.tr(&os)
    } else {
        i18n.tr(id)
    }
}

pub(crate) fn display_value(meta: &SettingMeta, value: &Value, loc: &Localizer) -> String {
    let with_unit = |unit: Unit, v: FluentValue<'static>, plain: String| match unit.message_id() {
        Some(id) => {
            let mut args = FluentArgs::new();
            args.set("value", v);
            loc.tr_args(id, &args)
        }
        None => plain,
    };
    match (&meta.kind, value) {
        (Kind::Bool, Value::Bool(b)) => loc.tr(if *b {
            "settings-value-on"
        } else {
            "settings-value-off"
        }),
        // An old value the app has yet to migrate has no label: shown as written.
        (Kind::Enum { options }, Value::Text(o)) if crate::schema::legacy_option(options, o) => {
            specular_i18n::isolate_ltr(o)
        }
        (Kind::Enum { .. }, Value::Text(o)) => loc.tr(&meta.option_id(o)),
        (Kind::Int { auto: true, .. }, Value::Int(0)) => loc.tr("settings-value-auto"),
        (Kind::Int { unit, .. }, Value::Int(i)) => {
            with_unit(*unit, FluentValue::from(*i), loc.integer(*i))
        }
        (Kind::Float { unit, .. }, Value::Float(x)) => {
            with_unit(*unit, FluentValue::from(*x), loc.number(*x, 2))
        }
        (Kind::Color, Value::Color(c)) => c.to_string(),
        (Kind::Path { directory }, Value::Path(p)) => match p {
            Some(p) => specular_i18n::isolate_ltr(&p.display().to_string()),
            // A folder that may stay unset says what that means (`<label>-unset`).
            None if loc.has_message(&format!("{}-unset", meta.label_id)) => {
                loc.tr(&format!("{}-unset", meta.label_id))
            }
            None if *directory => loc.tr("settings-value-default-location"),
            None => loc.tr("settings-value-auto"),
        },
        (Kind::Language, Value::Text(s)) if s == "system" => {
            loc.tr("settings-value-system-language")
        }
        (Kind::Locale, Value::Text(s)) if s == "language" => {
            loc.tr("settings-value-follow-language")
        }
        (Kind::Language | Kind::Locale, Value::Text(s)) => match s.parse::<LanguageIdentifier>() {
            Ok(l) => specular_i18n::language_name(&l, loc.language()),
            Err(_) => s.clone(),
        },
        (Kind::Monitor, Value::Text(s)) if s == "auto" => loc.tr(&meta.option_id("auto")),
        (Kind::Monitor, Value::Text(s)) => specular_i18n::isolate_ltr(s),
        (Kind::Pick, Value::Text(s)) if s.is_empty() => loc.tr(&meta.option_id("default")),
        (Kind::Pick, Value::Text(s)) => specular_i18n::isolate_ltr(s),
        (Kind::List { .. }, Value::List(items)) => {
            let labels: Vec<String> = items.iter().map(|o| loc.tr(&meta.option_id(o))).collect();
            loc.list(&labels, ListKind::Unit)
        }
        (Kind::Action, _) => loc.tr(meta.label_id),
        (Kind::Text, Value::Text(s)) => specular_i18n::isolate_ltr(s),
        (_, v) => v.to_string(),
    }
}

static METAS: LazyLock<Vec<SettingMeta>> = LazyLock::new(build_metas);

/// A check of a text setting's value beyond its kind: `Err` with the reason, in the UI
/// language, when the value can't be used.
pub type TextCheck = fn(&str) -> Result<(), String>;

static CHECKS: RwLock<Vec<(&'static str, TextCheck)>> = RwLock::new(Vec::new());

/// Sets the check of text setting `key` (the crate that reads it knows its rules: the
/// application registers the rename patterns' and the tile address's). Replaces an earlier
/// one.
pub fn set_text_check(key: &'static str, check: TextCheck) {
    let mut all = CHECKS.write().unwrap_or_else(|e| e.into_inner());
    all.retain(|(k, _)| *k != key);
    all.push((key, check));
}

fn text_check(key: &str) -> Option<TextCheck> {
    let all = CHECKS.read().unwrap_or_else(|e| e.into_inner());
    all.iter().find(|(k, _)| *k == key).map(|(_, c)| *c)
}

/// Every setting, in Settings-window order.
pub fn all() -> &'static [SettingMeta] {
    &METAS
}

/// The setting with full key `key` (`"viewing.initial_zoom"`).
pub fn find(key: &str) -> Option<&'static SettingMeta> {
    METAS.iter().find(|m| m.key == key)
}

/// The settings of one section.
pub fn in_section(section: Section) -> impl Iterator<Item = &'static SettingMeta> {
    METAS.iter().filter(move |m| m.section == section)
}

impl Section {
    /// Message id of the section title (`settings-section-viewing`).
    pub fn label_id(self) -> &'static str {
        static IDS: LazyLock<Vec<(Section, &'static str)>> = LazyLock::new(|| {
            Section::ALL
                .iter()
                .map(|s| (*s, leak(format!("settings-section-{}", kebab(s.key())))))
                .collect()
        });
        IDS.iter()
            .find(|(s, _)| *s == self)
            .map(|(_, id)| *id)
            .unwrap_or("settings-title")
    }

    /// Localised section title.
    pub fn label(self, i18n: &I18n) -> String {
        i18n.tr(self.label_id())
    }
}

/// Finds the closest known key to a mistyped one, for "Did you mean …?".
pub fn suggest(key: &str) -> Option<&'static str> {
    let key = key.trim().to_ascii_lowercase().replace('-', "_");
    // The section was left out: "initial_zoom" → "viewing.initial_zoom".
    if let Some(m) = METAS.iter().find(|m| m.name == key) {
        return Some(m.key);
    }
    METAS
        .iter()
        .map(|m| {
            let full = levenshtein(&key, m.key);
            let name = key
                .rsplit_once('.')
                .map_or(usize::MAX, |(_, n)| levenshtein(n, m.name));
            (full.min(name), m.key)
        })
        .filter(|(d, k)| *d <= (k.len() / 4).clamp(2, 4))
        .min_by_key(|(d, _)| *d)
        .map(|(_, k)| k)
}

/// Edit distance between two names, for "Did you mean …?" suggestions.
pub fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + usize::from(ca != *cb))
                .min(prev[j + 1] + 1)
                .min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}
