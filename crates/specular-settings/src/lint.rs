//! Checking the text of `settings.toml` the way the loader reads it, with the place of each
//! problem: for the built-in editor, which underlines problems as you type and refuses to
//! save while errors remain.

use std::ops::Range;

use specular_i18n::{FluentArgs, I18n, Localizer};
use toml_edit::Item;

use crate::commands::{COMMANDS_KEY, HOOKS_KEY, parse_commands, parse_hooks};
use crate::error::{SettingError, Warning};
use crate::file::{find_span, load_str};
use crate::meta::suggest;
use crate::schema::Section;

/// How serious a problem is. Only errors stop a save.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// What is wrong.
#[derive(Debug, Clone, PartialEq)]
pub enum Problem {
    /// Not valid TOML. `reason` is the parser's (English) explanation.
    Syntax { reason: String },
    /// A key given twice in the same table.
    Duplicate { key: String },
    /// A top-level table (or key) that is not a section.
    UnknownSection {
        key: String,
        suggestion: Option<&'static str>,
    },
    /// A key no setting has.
    UnknownKey {
        key: String,
        suggestion: Option<&'static str>,
    },
    /// A value of the wrong type, out of range, or not one of the choices.
    Invalid { key: String, reason: SettingError },
    /// Written by an older version; converted on load.
    Migrated { key: String },
    /// Written by a newer version.
    NewerVersion { found: i64, supported: i64 },
    /// A `[[commands]]` entry that cannot be used (1-based).
    Command { index: usize, reason: String },
    /// A `[[hooks]]` entry that cannot be used, or a note on one (1-based).
    Hook { index: usize, reason: String },
}

/// One problem, with where it is in the text.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    /// Byte range in the text (may be empty: a position).
    pub span: Range<usize>,
    pub severity: Severity,
    pub problem: Problem,
}

impl Diagnostic {
    /// The Fluent message id.
    pub fn message_id(&self) -> &'static str {
        match &self.problem {
            Problem::Syntax { .. } => "config-editor-problem-syntax",
            Problem::Duplicate { .. } => "config-editor-problem-duplicate",
            Problem::UnknownSection {
                suggestion: Some(_),
                ..
            } => "config-editor-problem-unknown-section-suggestion",
            Problem::UnknownSection { .. } => "config-editor-problem-unknown-section",
            Problem::UnknownKey {
                suggestion: Some(_),
                ..
            } => "config-editor-problem-unknown-key-suggestion",
            Problem::UnknownKey { .. } => "config-editor-problem-unknown-key",
            Problem::Invalid { .. } => "config-editor-problem-invalid",
            Problem::Migrated { .. } => "config-editor-problem-migrated",
            Problem::NewerVersion { .. } => "config-editor-problem-newer-version",
            Problem::Command { .. } => "config-editor-problem-command",
            Problem::Hook { .. } => "config-editor-problem-hook",
        }
    }

    fn render(&self, loc: &Localizer) -> String {
        let mut args = FluentArgs::new();
        match &self.problem {
            Problem::Syntax { reason } => args.set("reason", reason.clone()),
            Problem::Duplicate { key } | Problem::Migrated { key } => args.set("key", key.clone()),
            Problem::UnknownSection { key, suggestion }
            | Problem::UnknownKey { key, suggestion } => {
                args.set("key", key.clone());
                if let Some(s) = suggestion {
                    args.set("suggestion", *s);
                }
            }
            Problem::Invalid { key, reason } => {
                args.set("key", key.clone());
                args.set("reason", loc.tr_args(reason.message_id(), &reason.args()));
            }
            Problem::NewerVersion { found, supported } => {
                args.set("found", *found);
                args.set("supported", *supported);
            }
            Problem::Command { index, reason } | Problem::Hook { index, reason } => {
                args.set("index", *index);
                args.set("reason", reason.clone());
            }
        }
        loc.tr_args(self.message_id(), &args)
    }

    /// Localised text.
    pub fn message(&self, i18n: &I18n) -> String {
        self.render(&i18n.localizer())
    }

    /// English text without bidi isolation marks (tests, logs).
    pub fn english(&self) -> String {
        specular_i18n::strip_isolates(&self.render(&Localizer::english()))
    }
}

/// Checks settings text with the loader's parser and schema. Problems come in text order.
/// Errors: syntax, duplicate keys, unknown sections and keys, invalid values. Warnings: a
/// newer schema version, unusable `[[commands]]` entries. Info: values converted from an
/// older version.
pub fn lint(text: &str) -> Vec<Diagnostic> {
    let loaded = load_str(text);
    let mut out = Vec::new();
    let at = |span: Option<Range<usize>>| {
        let s = span.unwrap_or(0..0);
        s.start.min(text.len())..s.end.min(text.len())
    };
    for (w, span) in loaded.warnings.iter().zip(&loaded.spans) {
        let span = at(span.clone());
        let (severity, problem) = match w {
            Warning::Syntax { reason, .. } => {
                let p = match duplicate_key(reason) {
                    Some(key) => Problem::Duplicate {
                        key: if key.is_empty() {
                            key_text(&text[span.clone()])
                        } else {
                            key
                        },
                    },
                    None => Problem::Syntax {
                        reason: reason.clone(),
                    },
                };
                (Severity::Error, p)
            }
            Warning::UnknownKey { key } if !key.contains('.') => (
                Severity::Error,
                Problem::UnknownSection {
                    key: key.clone(),
                    suggestion: suggest_section(key),
                },
            ),
            Warning::UnknownKey { key } => (
                Severity::Error,
                Problem::UnknownKey {
                    key: key.clone(),
                    suggestion: suggest(key),
                },
            ),
            Warning::InvalidValue { key, reason } => (
                Severity::Error,
                Problem::Invalid {
                    key: key.clone(),
                    reason: reason.clone(),
                },
            ),
            Warning::NewerVersion { found, supported } => (
                Severity::Warning,
                Problem::NewerVersion {
                    found: *found,
                    supported: *supported,
                },
            ),
        };
        out.push(Diagnostic {
            span,
            severity,
            problem,
        });
    }
    if let (Some(doc), Ok(raw)) = (&loaded.doc, toml_edit::Document::parse(text)) {
        let root = raw.as_table();
        // `[[commands]]`.
        let (_, problems) = parse_commands(doc);
        for p in problems {
            let item = root.get(COMMANDS_KEY);
            let span = match item {
                Some(Item::ArrayOfTables(a)) if p.index > 0 => {
                    a.get(p.index - 1).and_then(|t| t.span())
                }
                Some(Item::Value(toml_edit::Value::Array(a))) if p.index > 0 => {
                    a.get(p.index - 1).and_then(toml_edit::Value::span)
                }
                _ => find_span(root, COMMANDS_KEY, None, false),
            };
            out.push(Diagnostic {
                span: at(span),
                severity: Severity::Warning,
                problem: Problem::Command {
                    index: p.index,
                    reason: p.reason,
                },
            });
        }
        // `[[hooks]]`.
        let (_, problems) = parse_hooks(doc);
        for p in problems {
            let span = match root.get(HOOKS_KEY) {
                Some(Item::ArrayOfTables(a)) if p.index > 0 => {
                    a.get(p.index - 1).and_then(|t| t.span())
                }
                Some(Item::Value(toml_edit::Value::Array(a))) if p.index > 0 => {
                    a.get(p.index - 1).and_then(toml_edit::Value::span)
                }
                _ => find_span(root, HOOKS_KEY, None, false),
            };
            out.push(Diagnostic {
                span: at(span),
                severity: Severity::Warning,
                problem: Problem::Hook {
                    index: p.index,
                    reason: p.reason,
                },
            });
        }
        // Values a migration converted.
        if loaded.migrated {
            for (top, item) in doc.iter() {
                let (Some(section), Some(table)) = (Section::from_key(top), item.as_table_like())
                else {
                    continue;
                };
                let Some(old) = root.get(top).and_then(Item::as_table_like) else {
                    continue;
                };
                for (name, new) in table.iter() {
                    let before = old.get(name).and_then(Item::as_value).map(value_text);
                    let after = new.as_value().map(value_text);
                    if before != after {
                        out.push(Diagnostic {
                            span: at(find_span(root, top, Some(name), false)),
                            severity: Severity::Info,
                            problem: Problem::Migrated {
                                key: format!("{}.{name}", section.key()),
                            },
                        });
                    }
                }
            }
        }
    }
    out.sort_by_key(|d| (d.span.start, d.severity));
    out
}

fn value_text(v: &toml_edit::Value) -> String {
    v.to_string().trim().to_owned()
}

/// The key named in toml_edit's "duplicate key `x` in table `y`" message.
fn duplicate_key(reason: &str) -> Option<String> {
    let rest = reason.strip_prefix("duplicate key")?;
    let key = rest.split('`').nth(1).unwrap_or("").to_owned();
    Some(key)
}

/// A key as written (`"a b"`, `[x.y]`) without brackets, quotes or the value.
fn key_text(s: &str) -> String {
    let s = s.split('=').next().unwrap_or("").trim();
    s.trim_matches(|c| c == '[' || c == ']')
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .to_owned()
}

/// The closest section name to `key` (`[veiwing]` → `viewing`).
fn suggest_section(key: &str) -> Option<&'static str> {
    let k = key.trim().to_ascii_lowercase().replace('-', "_");
    Section::ALL
        .iter()
        .map(|s| (crate::meta::levenshtein(&k, s.key()), s.key()))
        .filter(|(d, s)| *d <= (s.len() / 3).clamp(1, 3))
        .min_by_key(|(d, _)| *d)
        .map(|(_, s)| s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(text: &str) -> Diagnostic {
        let d = lint(text);
        assert_eq!(d.len(), 1, "{d:#?}");
        d.into_iter().next().unwrap()
    }

    fn at<'a>(text: &'a str, d: &Diagnostic) -> &'a str {
        &text[d.span.clone()]
    }

    #[test]
    fn clean() {
        assert!(lint("version = 2\n[viewing]\ninitial_zoom = \"fit\"\n").is_empty());
        assert!(lint("").is_empty());
    }

    #[test]
    fn syntax() {
        let t = "version = 2\n[viewing\n";
        let d = one(t);
        assert_eq!(d.severity, Severity::Error);
        assert!(matches!(d.problem, Problem::Syntax { .. }));
        assert!(d.span.start >= 12, "{:?}", d.span);
        assert!(d.english().starts_with("Not valid TOML"), "{}", d.english());
    }

    #[test]
    fn duplicate() {
        let t = "[viewing]\ninitial_zoom = \"fit\"\ninitial_zoom = \"fill\"\n";
        let d = one(t);
        assert_eq!(
            d.problem,
            Problem::Duplicate {
                key: "initial_zoom".into()
            }
        );
        assert!(d.span.start >= t.find("\ninitial_zoom = \"fill").unwrap());
        assert!(d.english().contains("initial_zoom"));
    }

    #[test]
    fn unknown_section_and_key() {
        let t = "[veiwing]\nx = 1\n";
        let d = one(t);
        assert_eq!(
            d.problem,
            Problem::UnknownSection {
                key: "veiwing".into(),
                suggestion: Some("viewing")
            }
        );
        assert_eq!(at(t, &d), "veiwing");
        assert!(d.english().contains("Did you mean"), "{}", d.english());

        let t = "[viewing]\ninitial_zom = \"fit\"\n";
        let d = one(t);
        assert_eq!(
            d.problem,
            Problem::UnknownKey {
                key: "viewing.initial_zom".into(),
                suggestion: Some("viewing.initial_zoom")
            }
        );
        assert_eq!(at(t, &d), "initial_zom");
        assert!(d.english().contains("viewing.initial_zoom"));
    }

    #[test]
    fn wrong_type_range_and_enum() {
        let t = "[viewing]\ninitial_zoom = 3\n";
        let d = one(t);
        assert_eq!(at(t, &d), "3");
        assert!(matches!(d.problem, Problem::Invalid { .. }));

        let t = "[viewing]\ninitial_zoom = \"huge\"\n";
        let d = one(t);
        assert_eq!(at(t, &d), "\"huge\"");
        let m = d.english();
        assert!(m.contains("Expected one of") && m.contains("fit"), "{m}");

        // Out of range: the first ranged integer setting.
        let m = crate::meta::all()
            .iter()
            .find(|m| matches!(m.kind, crate::Kind::Int { .. }) && m.stored)
            .expect("an integer setting");
        let crate::Kind::Int { max, .. } = m.kind else {
            unreachable!()
        };
        let t = format!(
            "[{}]\n{} = {}\n",
            m.section.key(),
            m.name,
            max as i128 + 1000
        );
        let d = one(&t);
        assert!(d.english().contains("between"), "{}", d.english());

        let t = "[viewing]\ninitial_zoom = true\n";
        assert!(matches!(one(t).problem, Problem::Invalid { .. }));
    }

    #[test]
    fn newer_version_and_commands_are_warnings() {
        let t = "version = 99\n";
        let d = one(t);
        assert_eq!(d.severity, Severity::Warning);
        assert_eq!(at(t, &d), "99");

        let t = "[[commands]]\nname = \"x\"\n";
        let d = one(t);
        assert_eq!(d.severity, Severity::Warning);
        assert!(matches!(d.problem, Problem::Command { index: 1, .. }));
        assert!(at(t, &d).starts_with("[[commands]]"));
    }

    #[test]
    fn migrated_is_info() {
        let t = "version = 1\n[performance]\nhardware_decode = true\n";
        let d = one(t);
        assert_eq!(d.severity, Severity::Info);
        assert_eq!(at(t, &d), "hardware_decode");
    }

    #[test]
    fn several_in_order() {
        let t = "[viewing]\nbogus = 1\ninitial_zoom = 2\n[nope]\n";
        let d = lint(t);
        assert_eq!(d.len(), 3);
        assert!(d.windows(2).all(|w| w[0].span.start <= w[1].span.start));
    }
}
