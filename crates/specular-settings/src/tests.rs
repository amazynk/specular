use crate::undo::{Edit, UndoStack};
use crate::value::Value;

fn edit(key: &'static str, before: i64, after: i64) -> Edit {
    Edit {
        key,
        before: Value::Int(before),
        after: Value::Int(after),
    }
}

#[test]
fn undo_coalesces_same_key() {
    let mut u = UndoStack::default();
    u.record(edit("a", 1, 2));
    u.record(edit("a", 2, 3));
    u.record(edit("a", 3, 4));
    let step = u.pop_undo().unwrap();
    assert_eq!(step.edits, vec![edit("a", 1, 4)]);
    assert!(!u.can_undo());
    assert!(u.can_redo());
}

#[test]
fn undo_drops_steps_that_return_to_start() {
    let mut u = UndoStack::default();
    u.record(edit("a", 1, 2));
    u.record(edit("a", 2, 1));
    assert!(!u.can_undo());
}

#[test]
fn undo_keeps_different_keys_apart_and_clears_redo() {
    let mut u = UndoStack::default();
    u.record(edit("a", 1, 2));
    u.record(edit("b", 1, 2));
    assert_eq!(u.pop_undo().unwrap().edits[0].key, "b");
    assert!(u.can_redo());
    u.record(edit("c", 1, 2));
    assert!(!u.can_redo());
}

#[test]
fn undo_groups() {
    let mut u = UndoStack::default();
    u.begin_group();
    u.record(edit("a", 1, 2));
    u.record(edit("b", 1, 2));
    u.record(edit("a", 2, 5));
    u.end_group();
    let step = u.pop_undo().unwrap();
    assert_eq!(step.edits, vec![edit("a", 1, 5), edit("b", 1, 2)]);
    assert!(!u.can_undo());
    let redo = u.pop_redo().unwrap();
    assert_eq!(redo.edits.len(), 2);
    assert!(u.can_undo());
    u.clear();
    assert!(!u.can_undo() && !u.can_redo());
}

/// Every error, warning and editor problem, and every value with a unit, is formatted with
/// the arguments its English message uses (their ids are picked at run time, so
/// `message_args.rs` cannot read them from the source).
#[test]
fn every_settings_message_gets_its_arguments() {
    use crate::error::{SettingError, Warning};
    use crate::lint::{Diagnostic, Problem, Severity};
    use crate::value::Unit;
    use specular_i18n::{I18n, Localizer};

    let errors = vec![
        SettingError::UnknownKey {
            key: "a.b".into(),
            suggestion: Some("a.c"),
        },
        SettingError::UnknownKey {
            key: "a.b".into(),
            suggestion: None,
        },
        SettingError::NotBool,
        SettingError::NotInteger,
        SettingError::NotNumber,
        SettingError::OutOfRange {
            min: "1".into(),
            max: "9".into(),
            auto: true,
        },
        SettingError::OutOfRange {
            min: "1".into(),
            max: "9".into(),
            auto: false,
        },
        SettingError::UnknownOption {
            options: &["x", "y"],
        },
        SettingError::BadColour,
        SettingError::BadLanguage,
        SettingError::BadPath,
        SettingError::NotSettable,
        SettingError::Syntax,
        SettingError::BadText,
        SettingError::Invalid("Not like that.".into()),
    ];
    let i18n = I18n::global();
    specular_i18n::record_argument_problems(true);
    for e in &errors {
        let _ = e.message(i18n);
        let _ = e.to_string();
        let _ = Warning::InvalidValue {
            key: "a.b".into(),
            reason: e.clone(),
        }
        .message(i18n);
        let _ = Diagnostic {
            span: 0..1,
            severity: Severity::Error,
            problem: Problem::Invalid {
                key: "a.b".into(),
                reason: e.clone(),
            },
        }
        .message(i18n);
    }
    for w in [
        Warning::UnknownKey { key: "a.b".into() },
        Warning::NewerVersion {
            found: 3,
            supported: 2,
        },
        Warning::Syntax {
            line: 4,
            reason: "bad".into(),
        },
    ] {
        let _ = w.message(i18n);
    }
    for problem in [
        Problem::Syntax {
            reason: "bad".into(),
        },
        Problem::Duplicate { key: "a.b".into() },
        Problem::UnknownSection {
            key: "a".into(),
            suggestion: Some("b"),
        },
        Problem::UnknownSection {
            key: "a".into(),
            suggestion: None,
        },
        Problem::UnknownKey {
            key: "a.b".into(),
            suggestion: Some("a.c"),
        },
        Problem::UnknownKey {
            key: "a.b".into(),
            suggestion: None,
        },
        Problem::Migrated { key: "a.b".into() },
        Problem::NewerVersion {
            found: 3,
            supported: 2,
        },
        Problem::Command {
            index: 1,
            reason: "bad".into(),
        },
    ] {
        let _ = Diagnostic {
            span: 0..1,
            severity: Severity::Warning,
            problem,
        }
        .message(i18n);
    }
    // Values with units (`$value`): every setting's default, and each unit.
    let loc = Localizer::english();
    for m in crate::meta::all() {
        let _ = crate::meta::display_value(m, &m.default, &loc);
    }
    let mut problems = specular_i18n::argument_problems();
    specular_i18n::record_argument_problems(false);
    for u in [
        Unit::None,
        Unit::Percent,
        Unit::Milliseconds,
        Unit::Seconds,
        Unit::Mebibytes,
        Unit::Images,
        Unit::Threads,
        Unit::Luminance,
        Unit::Decibels,
    ] {
        if let Some(id) = u.message_id() {
            problems.extend(specular_i18n::argument_problem(id, ["value"]));
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

/// A text setting's check (registered by its reader) refuses values through `set`,
/// `set_text` and `--set` with its own reason; other settings and other values pass, and
/// control characters are refused by the kind itself.
#[test]
fn text_checks_refuse_with_their_reason() {
    use crate::{Origin, SettingError, SettingsStore, parse_set_arg, set_text_check};
    fn no_braces(s: &str) -> Result<(), String> {
        if s.contains('{') {
            Err("No braces.".into())
        } else {
            Ok(())
        }
    }
    set_text_check("import.folder_pattern", no_braces);
    let store = SettingsStore::in_memory();
    let o = Origin::SettingsWindow;
    let bad = SettingError::Invalid("No braces.".into());
    assert_eq!(
        store.set_text("import.folder_pattern", "{year}", o),
        Err(bad.clone())
    );
    assert_eq!(
        store.set("import.folder_pattern", Value::Text("{x}".into()), o),
        Err(bad.clone())
    );
    assert_eq!(parse_set_arg("import.folder_pattern={y}").unwrap_err(), bad);
    assert_eq!(
        store.set_text("import.folder_pattern", "photos", o),
        Ok(true)
    );
    assert_eq!(
        store.get("import.folder_pattern").unwrap(),
        Value::Text("photos".into())
    );
    // Other settings are not its.
    assert_eq!(store.set_text("convert.subfolder", "{x}", o), Ok(true));
    assert_eq!(
        store.set("general.your_name", Value::Text("a\tb".into()), o),
        Err(SettingError::BadText)
    );
    assert_eq!(bad.to_string(), "No braces.");
}
