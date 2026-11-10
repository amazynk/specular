//! Text checks over every catalogue (specular-i18n's and specular-metadata's):
//!
//! - **English left behind**: a translation identical to English may only hold names, units,
//!   codes and words the language shares with English. Its words (three letters or more, not
//!   all capitals) must be listed in `tests/same-as-english.txt`, for every language (`*`),
//!   for Latin-script languages (`latn`) or for that language.
//! - **No literal percent sign**: percentages go through `PERCENT()` (or a formatted
//!   argument), so each language gets its own pattern ("45%", "45 %", "%45", "٪٤٥").
//! - **Plural forms**: where English picks a plural form by a count, every language with
//!   plural forms does too wherever it shows that count next to a word.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use fluent_syntax::ast;

/// Catalogue folders, relative to the workspace root.
const ROOTS: &[&str] = &["crates/specular-i18n/i18n", "crates/specular-metadata/i18n"];

/// Messages whose `%` is not a percentage: strftime codes in name patterns, what the user
/// types into Go to time, and the unit of a chart axis.
const PERCENT_SIGN_ALLOWED: &[&str] = &[
    "settings-convert-pattern-help",
    "settings-import-rename-pattern-help",
    "batch-rename-tokens",
    "palette-go-to-time-help",
    "dvh-axis-volume",
];

/// Locales written in Latin script (the `latn` allowlist line applies to them).
const LATIN: &[&str] = &[
    "cs", "da", "de", "es", "fi", "fr", "it", "nl", "pl", "pt-BR", "pt-PT", "sv", "tr",
];

fn workspace_root() -> PathBuf {
    let manifest = std::fs::canonicalize(env!("CARGO_MANIFEST_DIR")).expect("manifest dir");
    manifest
        .ancestors()
        .find(|p| p.join("Cargo.toml").is_file() && p.join("crates").is_dir())
        .expect("workspace root")
        .to_path_buf()
}

/// One catalogue file: (locale, file name, messages by id and attribute).
type Messages = BTreeMap<String, Flat>;

/// A pattern flattened: `shape` holds its text with placeables written out (equal shapes are
/// equal messages), `text` only the text a reader sees.
#[derive(Default, PartialEq)]
struct Flat {
    shape: String,
    text: String,
}

fn flatten_pattern(p: &ast::Pattern<&str>, out: &mut Flat) {
    for el in &p.elements {
        match el {
            ast::PatternElement::TextElement { value } => {
                out.shape.push_str(value);
                out.text.push_str(value);
            }
            ast::PatternElement::Placeable { expression } => {
                out.text.push(' ');
                flatten_expression(expression, out);
                out.text.push(' ');
            }
        }
    }
}

fn flatten_expression(e: &ast::Expression<&str>, out: &mut Flat) {
    match e {
        ast::Expression::Select { selector, variants } => {
            out.shape.push('{');
            flatten_inline(selector, out);
            out.shape.push_str("->");
            for v in variants {
                let key = match v.key {
                    ast::VariantKey::Identifier { name } => name,
                    ast::VariantKey::NumberLiteral { value } => value,
                };
                out.shape.push_str(if v.default { "*[" } else { "[" });
                out.shape.push_str(key);
                out.shape.push(']');
                flatten_pattern(&v.value, out);
                out.text.push('\n');
            }
            out.shape.push('}');
        }
        ast::Expression::Inline(i) => {
            out.shape.push('{');
            flatten_inline(i, out);
            out.shape.push('}');
        }
    }
}

fn flatten_inline(i: &ast::InlineExpression<&str>, out: &mut Flat) {
    match i {
        ast::InlineExpression::StringLiteral { value } => {
            // `{ "{" }` and friends are text the reader sees.
            out.shape.push('"');
            out.shape.push_str(value);
            out.shape.push('"');
            out.text.push_str(value);
        }
        ast::InlineExpression::NumberLiteral { value } => out.shape.push_str(value),
        ast::InlineExpression::FunctionReference { id, arguments } => {
            out.shape.push_str(id.name);
            out.shape.push('(');
            for a in &arguments.positional {
                flatten_inline(a, out);
                out.shape.push(',');
            }
            for a in &arguments.named {
                out.shape.push_str(a.name.name);
                out.shape.push(':');
                flatten_inline(&a.value, out);
                out.shape.push(',');
            }
            out.shape.push(')');
        }
        ast::InlineExpression::MessageReference { id, attribute } => {
            out.shape.push_str(id.name);
            if let Some(a) = attribute {
                out.shape.push('.');
                out.shape.push_str(a.name);
            }
        }
        ast::InlineExpression::TermReference { id, .. } => {
            out.shape.push('-');
            out.shape.push_str(id.name);
        }
        ast::InlineExpression::VariableReference { id } => {
            out.shape.push('$');
            out.shape.push_str(id.name);
        }
        ast::InlineExpression::Placeable { expression } => flatten_expression(expression, out),
    }
}

fn messages(src: &str) -> Messages {
    let resource = match fluent_syntax::parser::parse(src) {
        Ok(r) => r,
        Err((r, _)) => r,
    };
    let mut out = Messages::new();
    for entry in &resource.body {
        let (id, value, attributes) = match entry {
            ast::Entry::Message(m) => (m.id.name.to_owned(), m.value.as_ref(), &m.attributes),
            ast::Entry::Term(t) => (format!("-{}", t.id.name), Some(&t.value), &t.attributes),
            _ => continue,
        };
        if let Some(v) = value {
            let mut f = Flat::default();
            flatten_pattern(v, &mut f);
            out.insert(id.clone(), f);
        }
        for a in attributes {
            let mut f = Flat::default();
            flatten_pattern(&a.value, &mut f);
            out.insert(format!("{id}.{}", a.id.name), f);
        }
    }
    out
}

/// Every catalogue file: (locale, file name, path).
fn catalogue_files(root: &Path) -> Vec<(String, String, PathBuf)> {
    let mut out = Vec::new();
    for dir in ROOTS {
        let dir = root.join(dir);
        let mut locales: Vec<_> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .flatten()
            .filter(|e| e.path().is_dir())
            .collect();
        locales.sort_by_key(|e| e.file_name());
        for l in locales {
            let locale = l.file_name().to_string_lossy().into_owned();
            let mut files: Vec<_> = std::fs::read_dir(l.path())
                .expect("locale folder")
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "ftl"))
                .collect();
            files.sort();
            for f in files {
                let name = f.file_name().unwrap().to_string_lossy().into_owned();
                out.push((locale.clone(), name, f));
            }
        }
    }
    out
}

/// The words of `text` a translator would translate: three letters or more, not all
/// capitals (acronyms, codes and unit symbols such as "SUV", "mm" or "px" are left out).
fn words(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = None;
    let is_word = |c: char| c.is_alphabetic() || c == '\'' || c == '’' || c == '-';
    for (i, c) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ' ')))
    {
        match (start, is_word(c) && (start.is_some() || c.is_alphabetic())) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                let w = text[s..i].trim_end_matches(['\'', '’', '-']);
                start = None;
                let letters = w.chars().filter(|c| c.is_alphabetic()).count();
                let all_caps = !w.chars().any(|c| c.is_lowercase());
                if letters >= 3 && !all_caps {
                    out.push(w);
                }
            }
            _ => {}
        }
    }
    out
}

/// `tests/same-as-english.txt`: `<scope> word word …` lines; scope is `*`, `latn` or a locale.
fn allowlist() -> BTreeMap<String, BTreeSet<String>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/same-as-english.txt");
    let text = std::fs::read_to_string(&path).expect("tests/same-as-english.txt");
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let scope = parts.next().unwrap().to_owned();
        out.entry(scope)
            .or_default()
            .extend(parts.map(str::to_owned));
    }
    out
}

#[test]
fn no_english_left_behind() {
    let root = workspace_root();
    let files = catalogue_files(&root);
    let allow = allowlist();
    let empty = BTreeSet::new();
    let english: BTreeMap<&str, Messages> = files
        .iter()
        .filter(|(l, _, _)| l == "en")
        .map(|(_, name, p)| {
            (
                name.as_str(),
                messages(&std::fs::read_to_string(p).unwrap()),
            )
        })
        .collect();
    let mut problems = Vec::new();
    let mut checked = 0;
    for (locale, name, path) in &files {
        if locale == "en" {
            continue;
        }
        let Some(en) = english.get(name.as_str()) else {
            continue;
        };
        let everywhere = allow.get("*").unwrap_or(&empty);
        let latin = if LATIN.contains(&locale.as_str()) {
            allow.get("latn").unwrap_or(&empty)
        } else {
            &empty
        };
        let own = allow.get(locale.as_str()).unwrap_or(&empty);
        for (id, flat) in messages(&std::fs::read_to_string(path).unwrap()) {
            checked += 1;
            if en.get(&id) != Some(&flat) {
                continue;
            }
            let left: Vec<_> = words(&flat.text)
                .into_iter()
                .filter(|w| !everywhere.contains(*w) && !latin.contains(*w) && !own.contains(*w))
                .collect();
            if !left.is_empty() {
                problems.push(format!("{locale}/{name}: {id}: {}", left.join(" ")));
            }
        }
    }
    assert!(checked > 50_000, "only {checked} messages checked");
    assert!(
        problems.is_empty(),
        "translations identical to English (translate them, or list words that are the \
         same in the language in tests/same-as-english.txt):\n{}",
        problems.join("\n")
    );
}

#[test]
fn no_literal_percent_signs() {
    let root = workspace_root();
    let mut problems = Vec::new();
    let files = catalogue_files(&root);
    assert!(
        files.iter().any(|(_, n, _)| n == "develop-retouch.ftl"),
        "Develop's catalogues are checked"
    );
    for (locale, name, path) in files {
        for (id, flat) in messages(&std::fs::read_to_string(&path).unwrap()) {
            let base = id.split('.').next().unwrap();
            if flat.text.contains(['%', '٪']) && !PERCENT_SIGN_ALLOWED.contains(&base) {
                problems.push(format!("{locale}/{name}: {id}"));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "literal percent signs (use PERCENT($value) or PERCENT(100)):\n{}",
        problems.join("\n")
    );
}

#[test]
fn words_are_split_as_a_translator_sees_them() {
    assert_eq!(words("Zoom px"), ["Zoom"]);
    assert_eq!(words("SUVmax, TLG and mm"), ["SUVmax", "and"]);
    assert_eq!(words("Lagret’s well-known"), ["Lagret’s", "well-known"]);
}

/// Variables a pattern uses where a word may agree with them, and those it picks a plural
/// form by (a select with a CLDR category key such as `[one]`, on `$var` or `NUMBER($var)`).
/// A value standing alone after a label ("Files: { $count }", "Files: { $count }, …") has no
/// word to agree with and is not counted as used.
#[derive(Default)]
struct Counts<'a> {
    used: BTreeSet<&'a str>,
    plural: BTreeSet<&'a str>,
}

fn count_pattern<'a>(p: &'a ast::Pattern<&'a str>, out: &mut Counts<'a>) {
    let text = |i: Option<&'a ast::PatternElement<&'a str>>| -> Option<&'a str> {
        match i {
            Some(ast::PatternElement::TextElement { value }) => Some(*value),
            _ => None,
        }
    };
    for (i, el) in p.elements.iter().enumerate() {
        let ast::PatternElement::Placeable { expression } = el else {
            continue;
        };
        let after_label = i
            .checked_sub(1)
            .and_then(|j| text(p.elements.get(j)))
            .is_some_and(|t| t.trim_end().ends_with([':', '：']));
        let no_word_after = text(p.elements.get(i + 1))
            .and_then(|t| t.trim_start().chars().next())
            .is_none_or(|c| !c.is_alphabetic());
        let alone = after_label
            && no_word_after
            && matches!(
                expression,
                ast::Expression::Inline(
                    ast::InlineExpression::VariableReference { .. }
                        | ast::InlineExpression::FunctionReference { .. }
                )
            );
        if alone {
            let mut inner = Counts::default();
            count_expression(expression, &mut inner);
            out.plural.extend(inner.plural);
        } else {
            count_expression(expression, out);
        }
    }
}

fn count_expression<'a>(e: &'a ast::Expression<&'a str>, out: &mut Counts<'a>) {
    match e {
        ast::Expression::Select { selector, variants } => {
            let plural = variants.iter().any(|v| {
                matches!(v.key, ast::VariantKey::Identifier { name }
                    if matches!(name, "zero" | "one" | "two" | "few" | "many"))
            });
            let var = match selector {
                ast::InlineExpression::VariableReference { id } => Some(id.name),
                ast::InlineExpression::FunctionReference { arguments, .. } => {
                    match arguments.positional.first() {
                        Some(ast::InlineExpression::VariableReference { id }) => Some(id.name),
                        _ => None,
                    }
                }
                _ => None,
            };
            if plural && let Some(v) = var {
                out.plural.insert(v);
            }
            // Selecting on a count does not show it.
            for v in variants {
                count_pattern(&v.value, out);
            }
        }
        ast::Expression::Inline(i) => count_inline(i, out),
    }
}

fn count_inline<'a>(i: &'a ast::InlineExpression<&'a str>, out: &mut Counts<'a>) {
    match i {
        ast::InlineExpression::VariableReference { id } => {
            out.used.insert(id.name);
        }
        ast::InlineExpression::FunctionReference { arguments, .. }
        | ast::InlineExpression::TermReference {
            arguments: Some(arguments),
            ..
        } => {
            for a in &arguments.positional {
                count_inline(a, out);
            }
            for a in &arguments.named {
                count_inline(&a.value, out);
            }
        }
        ast::InlineExpression::Placeable { expression } => count_expression(expression, out),
        _ => {}
    }
}

fn parse(src: &str) -> ast::Resource<&str> {
    match fluent_syntax::parser::parse(src) {
        Ok(r) => r,
        Err((r, _)) => r,
    }
}

/// Each message's and attribute's [`Counts`], by id (`msg` or `msg.attr`).
fn counts<'a>(r: &'a ast::Resource<&'a str>) -> BTreeMap<String, Counts<'a>> {
    let mut out = BTreeMap::new();
    for entry in &r.body {
        let ast::Entry::Message(m) = entry else {
            continue;
        };
        if let Some(v) = &m.value {
            let mut c = Counts::default();
            count_pattern(v, &mut c);
            out.insert(m.id.name.to_owned(), c);
        }
        for a in &m.attributes {
            let mut c = Counts::default();
            count_pattern(&a.value, &mut c);
            out.insert(format!("{}.{}", m.id.name, a.id.name), c);
        }
    }
    out
}

/// Languages whose nouns don't change with a count: Japanese and Chinese (CLDR has only
/// `other`) and Turkish (a noun after a number stays singular: "5 dosya").
const NO_PLURALS: &[&str] = &["ja", "zh-Hans", "tr"];

/// (locales, message, variable) whose translation may show a count English picks a plural
/// form by without one, because no word in it agrees with the count.
const FLAT_COUNT_ALLOWED: &[(&str, &str, &str)] = &[
    // English picks the wording by the number of series, not a noun form.
    ("*", "dicom-series-toast", "series"),
    // "★ 1 or more" is English's own wording, not agreement.
    ("*", "gallery-filter-rating", "stars"),
    // A noun before the number, as a label ("hodnot 5", "values: 5", "loading slices: 3 of 9").
    ("ar cs pl uk", "map-table-stats-number", "count"),
    ("ar cs pl uk", "map-table-stats-text", "count"),
    ("ar cs pl uk", "map-table-stats-text-top", "count"),
    ("ar", "seg-stl-detail", "count"),
    ("ar cs pl uk", "survey-surface", "points"),
    ("ar cs pl uk", "survey-surface", "triangles"),
    // A unit symbol after the number ("1 234 B").
    ("cs pl", "maker-note-bytes", "num"),
    ("ar cs fi he pl uk", "mpr-loading", "count"),
    ("ar", "similar-confirm", "groups"),
    ("ar", "similar-confirm-yes", "files"),
    // The verb does not agree with a number in Danish, Swedish and Finnish.
    ("da fi sv", "import-done-existed", "count"),
    // English's verb agrees with the count ("1 … was", "3 … were"); these translations name
    // the delay as the subject ("for 3 of the last 9 images, reading the file held it up"),
    // or use a verb that does not change.
    (
        "ar cs da fi pl sv uk",
        "open-report-advice-archive",
        "count",
    ),
    (
        "ar cs da fi pl sv uk",
        "open-report-advice-slow-storage",
        "count",
    ),
    (
        "ar cs da fi pl sv uk",
        "open-report-advice-slow-storage-no-speed",
        "count",
    ),
    (
        "ar cs da fi pl sv uk",
        "open-report-advice-software",
        "count",
    ),
    (
        "ar cs da fi pl sv uk",
        "open-report-advice-thumbnails-archive",
        "count",
    ),
    (
        "ar cs da fi pl sv uk",
        "open-report-advice-thumbnails-slow-storage",
        "count",
    ),
    (
        "ar cs da fi pl sv uk",
        "open-report-advice-thumbnails-slow-storage-no-speed",
        "count",
    ),
    (
        "ar cs da fi pl sv uk",
        "open-report-advice-thumbnails-software",
        "count",
    ),
];

fn flat_count_allowed(locale: &str, id: &str, var: &str) -> bool {
    let base = id.split('.').next().unwrap_or(id);
    FLAT_COUNT_ALLOWED.iter().any(|(locales, m, v)| {
        *m == base && *v == var && (*locales == "*" || locales.split(' ').any(|l| l == locale))
    })
}

/// Where English picks a plural form by a count, every language with plural forms does too
/// whenever it shows that count: "{ $count } Dateien" reads "1 Dateien".
#[test]
fn counts_english_pluralises_are_pluralised_everywhere() {
    let root = workspace_root();
    let files = catalogue_files(&root);
    let sources: Vec<(String, String, String)> = files
        .iter()
        .map(|(l, n, p)| (l.clone(), n.clone(), std::fs::read_to_string(p).unwrap()))
        .collect();
    // English plural selectors per (file, message): their variables.
    let mut english: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    for (_, name, src) in sources.iter().filter(|(l, _, _)| l == "en") {
        let r = parse(src);
        for (id, c) in counts(&r) {
            if !c.plural.is_empty() {
                english.insert(
                    (name.clone(), id),
                    c.plural.iter().map(|s| (*s).to_owned()).collect(),
                );
            }
        }
    }
    assert!(
        english.len() > 250,
        "only {} English plurals",
        english.len()
    );
    let mut problems = Vec::new();
    for (locale, name, src) in &sources {
        if locale == "en" || NO_PLURALS.contains(&locale.as_str()) {
            continue;
        }
        let r = parse(src);
        for (id, c) in counts(&r) {
            let Some(vars) = english.get(&(name.clone(), id.clone())) else {
                continue;
            };
            for v in vars {
                if c.used.contains(v.as_str())
                    && !c.plural.contains(v.as_str())
                    && !flat_count_allowed(locale, &id, v)
                {
                    problems.push(format!("{locale}/{name}: {id}: ${v}"));
                }
            }
        }
    }
    assert!(
        problems.is_empty(),
        "{} counts English pluralises are flat here (give the message its plural forms, or \
         list a count that stands alone in FLAT_COUNT_ALLOWED):\n{}",
        problems.len(),
        problems.join("\n")
    );
}
