//! Every `t!("id", name = value, …)` in the workspace passes the arguments its English message
//! uses, and no others: a missing one shows as `{$name}` on screen, and one the message does
//! not use (a misspelt name, a plural on another variable) is silently dropped. Arguments
//! only some variants of a selection use may be left out (the call selects another). The sources are read
//! with a small Rust lexer, so `t!` in comments and strings is not a call, and arguments
//! spanning lines, nested calls and closures are read whole.
//!
//! Ids with an attribute (`msg.attr`) and ids made at run time are not checked here
//! (`message_ids.rs` checks that every literal id exists).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Crates with their own catalogue (not specular-i18n's).
const OWN_CATALOGUE: &[&str] = &["crates/specular-metadata"];

/// Calls that pass other arguments on purpose (tests of the fallback itself).
const ALLOWED: &[(&str, &str)] = &[];

fn workspace_root() -> PathBuf {
    let manifest = std::fs::canonicalize(env!("CARGO_MANIFEST_DIR")).expect("manifest dir");
    manifest
        .ancestors()
        .find(|p| p.join("Cargo.toml").is_file() && p.join("crates").is_dir())
        .expect("workspace root")
        .to_path_buf()
}

/// One `t!` call: its id, the names of its arguments, and where it is.
#[derive(Debug, PartialEq, Eq)]
struct Call {
    id: String,
    args: BTreeSet<String>,
    /// Each `name = value` argument's value as written.
    values: Vec<(String, String)>,
    line: usize,
}

/// What a byte of the source is part of.
#[derive(Clone, Copy, PartialEq, Eq)]
enum In {
    Code,
    LineComment,
    /// Nested block comments, by depth.
    Block(u32),
    Str,
    /// A raw string closed by `"` and this many `#`.
    Raw(usize),
}

/// Marks each byte of `src` with whether it is code (not a comment, string or char literal).
fn code_mask(src: &[u8]) -> Vec<bool> {
    let mut mask = vec![false; src.len()];
    let mut st = In::Code;
    let mut i = 0;
    let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    while i < src.len() {
        let b = src[i];
        let next = src.get(i + 1).copied();
        match st {
            In::Code => {
                if b == b'/' && next == Some(b'/') {
                    st = In::LineComment;
                    i += 2;
                    continue;
                }
                if b == b'/' && next == Some(b'*') {
                    st = In::Block(1);
                    i += 2;
                    continue;
                }
                // Raw strings: r"…", r#"…"#, br"…".
                if (b == b'r' || b == b'b' && next == Some(b'r')) && (i == 0 || !ident(src[i - 1]))
                {
                    let start = if b == b'b' { i + 2 } else { i + 1 };
                    let hashes = src[start..].iter().take_while(|&&c| c == b'#').count();
                    if src.get(start + hashes) == Some(&b'"') {
                        mask[i] = true;
                        st = In::Raw(hashes);
                        i = start + hashes + 1;
                        continue;
                    }
                }
                if b == b'"' {
                    mask[i] = true;
                    st = In::Str;
                    i += 1;
                    continue;
                }
                // A char literal ('a', '\n', '\u{..}') rather than a lifetime ('a).
                if b == b'\'' {
                    if next == Some(b'\\') {
                        let end = src[i + 2..].iter().position(|&c| c == b'\'');
                        if let Some(e) = end {
                            i += 2 + e + 1;
                            continue;
                        }
                    } else if let Some(n) = next {
                        let w = utf8_width(n);
                        if src.get(i + 1 + w) == Some(&b'\'') {
                            i += 2 + w;
                            continue;
                        }
                    }
                }
                mask[i] = true;
                i += 1;
            }
            In::LineComment => {
                if b == b'\n' {
                    st = In::Code;
                    mask[i] = true;
                }
                i += 1;
            }
            In::Block(d) => {
                if b == b'*' && next == Some(b'/') {
                    st = if d == 1 { In::Code } else { In::Block(d - 1) };
                    i += 2;
                } else if b == b'/' && next == Some(b'*') {
                    st = In::Block(d + 1);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            In::Str => {
                if b == b'\\' {
                    i += 2;
                } else {
                    if b == b'"' {
                        mask[i] = true;
                        st = In::Code;
                    }
                    i += 1;
                }
            }
            In::Raw(h) => {
                if b == b'"' && src[i + 1..].iter().take(h).filter(|&&c| c == b'#').count() == h {
                    i += 1 + h;
                    mask[i - 1] = true;
                    st = In::Code;
                } else {
                    i += 1;
                }
            }
        }
    }
    mask
}

fn utf8_width(first: u8) -> usize {
    match first {
        0..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}

/// The `t!` calls of a source file.
fn calls(src: &str) -> Vec<Call> {
    let b = src.as_bytes();
    let mask = code_mask(b);
    let mut out = Vec::new();
    let mut i = 0;
    // Spelt apart, so `message_ids.rs`'s scanner does not take it for a call.
    const NEEDLE: &str = concat!("t", "!(");
    while let Some(k) = src[i..].find(NEEDLE) {
        let at = i + k;
        i = at + 3;
        if !mask[at] || at > 0 && (b[at - 1].is_ascii_alphanumeric() || b[at - 1] == b'_') {
            continue;
        }
        // The body, to the matching parenthesis (brackets in code only).
        let mut depth = 1;
        let mut j = at + 3;
        let mut parts: Vec<(usize, usize)> = Vec::new();
        let mut part = j;
        while j < b.len() && depth > 0 {
            if mask[j] {
                match b[j] {
                    b'(' | b'[' | b'{' => depth += 1,
                    b')' | b']' | b'}' => depth -= 1,
                    b',' if depth == 1 => {
                        parts.push((part, j));
                        part = j + 1;
                    }
                    _ => {}
                }
            }
            j += 1;
        }
        if depth != 0 {
            break;
        }
        parts.push((part, j - 1));
        let mut items = parts.iter().map(|&(s, e)| src[s..e].trim());
        let Some(first) = items.next() else { continue };
        // `t!(handle => "id", …)`.
        let first = first.rsplit_once("=>").map_or(first, |(_, id)| id.trim());
        let Some(id) = first
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .filter(|s| !s.contains(['"', '\\']))
        else {
            continue; // `$id` in a macro, or an id made at run time.
        };
        let mut args = BTreeSet::new();
        let mut values = Vec::new();
        for item in items.filter(|s| !s.is_empty()) {
            let split = item.split_once('=').filter(|(n, _)| {
                let n = n.trim();
                !n.is_empty() && n.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
            });
            match split {
                Some((n, v)) => {
                    args.insert(n.trim().to_owned());
                    values.push((n.trim().to_owned(), v.trim().to_owned()));
                }
                None => {
                    args.insert(format!("<not name = value: {item}>"));
                }
            }
        }
        out.push(Call {
            id: id.to_owned(),
            args,
            values,
            line: src[..at].lines().count().max(1),
        });
    }
    out
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            let rel = e.path().strip_prefix(root).unwrap_or(e.path());
            !(name.starts_with('.') && e.depth() > 0
                || name == "target"
                || name == "tmp" && e.depth() == 1
                || OWN_CATALOGUE.iter().any(|c| rel == Path::new(c)))
        })
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "rs"))
        .map(|e| e.into_path())
        .collect()
}

#[test]
fn every_t_call_passes_the_arguments_english_uses() {
    let root = workspace_root();
    let mut vars: BTreeMap<String, Option<specular_i18n::Arguments>> = BTreeMap::new();
    let mut problems = Vec::new();
    let mut checked = 0;
    for path in rust_files(&root) {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        let rel = path.strip_prefix(&root).unwrap().display().to_string();
        for c in calls(&src) {
            if c.id.contains('.') || ALLOWED.contains(&(rel.as_str(), c.id.as_str())) {
                continue;
            }
            let Some(want) = vars
                .entry(c.id.clone())
                .or_insert_with(|| specular_i18n::english_arguments(&c.id))
            else {
                // A missing id: `message_ids.rs` reports it.
                continue;
            };
            checked += 1;
            let missing: Vec<&String> = want.required.difference(&c.args).collect();
            let unused: Vec<&String> = c
                .args
                .iter()
                .filter(|a| !want.required.contains(*a) && !want.optional.contains(*a))
                .collect();
            if !missing.is_empty() || !unused.is_empty() {
                problems.push(format!(
                    "{rel}:{}: {}: missing {missing:?}, not in the message {unused:?}",
                    c.line, c.id
                ));
            }
        }
    }
    assert!(checked > 1000, "the scanner found only {checked} calls");
    assert!(
        problems.is_empty(),
        "{} t! call(s) don't pass what their English message uses:\n{}",
        problems.len(),
        problems.join("\n")
    );
}

#[test]
fn the_scanner_reads_calls_whole_and_skips_comments_and_strings() {
    // `T!` stands for the macro (see `NEEDLE`).
    let src = r##"
        // T!("in-a-comment", x = 1)
        /* T!("in /* nested */ block") */
        let s = "T!(\"in-a-string\")";
        let r = r#"T!("in-raw")"#;
        let c = '(';
        let a = T!("plain");
        let b = specular_i18n::T!("with-args", count = v.len() as i64, name = f(a, b),);
        let h = T!(&loc => "handle", n = [1, 2].len(), w = |x: u8| { x });
        macro_rules! m { ($id:literal) => { T!($id) }; }
        let lifetime: &'static str = T!(
            "multi-line",
            when = std::format!("{}, {}", a, b)
        );
    "##
    .replace("T!(", concat!("t", "!("));
    let got: Vec<(String, Vec<String>)> = calls(&src)
        .into_iter()
        .map(|c| (c.id, c.args.into_iter().collect()))
        .collect();
    let want: Vec<(String, Vec<String>)> = [
        ("plain", vec![]),
        ("with-args", vec!["count", "name"]),
        ("handle", vec!["n", "w"]),
        ("multi-line", vec!["when"]),
    ]
    .into_iter()
    .map(|(i, a)| (i.to_owned(), a.into_iter().map(str::to_owned).collect()))
    .collect();
    assert_eq!(got, want);
}

/// One `tr_args(id, args)` call: its arguments as written, the source before it, and where
/// it is.
struct TrCall<'a> {
    first: &'a str,
    second: &'a str,
    before: &'a str,
    line: usize,
}

/// The `tr_args` calls of a source file (methods, functions and helpers of that name), not
/// definitions and not forwards of an `args` parameter.
fn tr_calls(src: &str) -> Vec<TrCall<'_>> {
    let b = src.as_bytes();
    let mask = code_mask(b);
    // Spelt apart, so `message_ids.rs`'s scanner does not take it for a call.
    const NEEDLE: &str = concat!("tr", "_args(");
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(k) = src[i..].find(NEEDLE) {
        let at = i + k;
        i = at + NEEDLE.len();
        if !mask[at]
            || at > 0 && (b[at - 1].is_ascii_alphanumeric() || b[at - 1] == b'_')
            || src[..at].trim_end().ends_with("fn")
        {
            continue;
        }
        let mut depth = 1;
        let mut j = i;
        let mut parts: Vec<(usize, usize)> = Vec::new();
        let mut part = j;
        while j < b.len() && depth > 0 {
            if mask[j] {
                match b[j] {
                    b'(' | b'[' | b'{' => depth += 1,
                    b')' | b']' | b'}' => depth -= 1,
                    b',' if depth == 1 => {
                        parts.push((part, j));
                        part = j + 1;
                    }
                    _ => {}
                }
            }
            j += 1;
        }
        if depth != 0 {
            break;
        }
        parts.push((part, j - 1));
        let args: Vec<&str> = parts
            .iter()
            .map(|&(s, e)| src[s..e].trim())
            .filter(|s| !s.is_empty())
            .collect();
        // `I18n::tr_args(self, id, args)` and `.tr_args(id, args)`: a trait or handle
        // passing its caller's arguments on; `$id`: the `t!` macro.
        let last = args.last().copied().unwrap_or("");
        if args.len() < 2 || last == "args" || args.iter().any(|a| a.starts_with('$')) {
            continue;
        }
        out.push(TrCall {
            first: args[args.len() - 2],
            second: last,
            before: &src[..at],
            line: src[..at].lines().count().max(1),
        });
    }
    out
}

/// The first string of each `("name", …)` pair in `s`.
fn pair_names(s: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = s;
    while let Some(k) = rest.find('(') {
        rest = rest[k + 1..].trim_start();
        let Some(r) = rest.strip_prefix('"') else {
            continue;
        };
        rest = r;
        if let Some(e) = rest.find('"') {
            let name = &rest[..e];
            if rest[e + 1..].trim_start().starts_with(',')
                && name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
            {
                out.insert(name.to_owned());
            }
        }
    }
    out
}

/// The argument names a call passes, when the source says: `&[("name", …), …]`, or `&args`
/// made just before (`let mut args = FluentArgs::new()` or `vec![("name", …)]`, then
/// `args.set("name", …)` or `args.push(("name", …))`). None when they are made elsewhere.
fn passed(c: &TrCall<'_>) -> Option<BTreeSet<String>> {
    if let Some(list) = c.second.strip_prefix("&[") {
        return Some(pair_names(list));
    }
    if c.second.replace(' ', "") == "&FluentArgs::new()" {
        return Some(BTreeSet::new());
    }
    let var = c.second.strip_prefix('&')?;
    if !var.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        return None;
    }
    let made = c.before.rfind(&format!("let mut {var} = "))?;
    let after = &c.before[made..];
    let init_end = after.find(';')?;
    let init = &after[..init_end];
    if !(init.contains("FluentArgs::new()") || init.contains("vec![")) {
        return None;
    }
    let mut names = pair_names(init);
    for needle in [format!("{var}.set("), format!("{var}.push(")] {
        let mut rest = &after[init_end..];
        while let Some(k) = rest.find(&needle) {
            rest = &rest[k + needle.len()..];
            let r = rest.trim_start();
            let r = r.strip_prefix('(').unwrap_or(r).trim_start();
            let Some(r) = r.strip_prefix('"') else {
                continue;
            };
            let Some(e) = r.find('"') else { break };
            names.insert(r[..e].to_owned());
        }
    }
    Some(names)
}

/// `tr_args` calls whose id (or arguments) are chosen at run time: (file, the id as written,
/// the test that formats every case of it while [`specular_i18n::record_argument_problems`]
/// records, or "" for a test's own formatter).
const RUNTIME_TR_ARGS: &[(&str, &str, &str)] = &[
    (
        "app/specular/src/app/measure_tool/geometry.rs",
        "&id",
        "every_landmark_name_gets_its_number",
    ),
    (
        "crates/specular-settings/src/error.rs",
        "self.message_id()",
        "every_settings_message_gets_its_arguments",
    ),
    (
        "crates/specular-settings/src/error.rs",
        "reason.message_id()",
        "every_settings_message_gets_its_arguments",
    ),
    (
        "crates/specular-settings/src/lint.rs",
        "self.message_id()",
        "every_settings_message_gets_its_arguments",
    ),
    (
        "crates/specular-settings/src/lint.rs",
        "reason.message_id()",
        "every_settings_message_gets_its_arguments",
    ),
    (
        "crates/specular-settings/src/meta.rs",
        "id",
        "every_settings_message_gets_its_arguments",
    ),
    (
        "crates/specular-i18n/src/localizer.rs",
        "ids[i]",
        "byte_sizes_get_their_arguments",
    ),
    ("crates/specular-i18n/src/tests.rs", "id", ""),
    (
        "crates/specular-i18n/src/tests.rs",
        "\"status-image-count\"",
        "",
    ),
    (
        "crates/specular-input/src/names.rs",
        "id",
        "action_names_get_their_arguments",
    ),
    (
        "app/specular/src/app/fileops.rs",
        "id",
        "fileops_messages_get_their_arguments",
    ),
    (
        "app/specular/src/app/fileops.rs",
        "p.message_id()",
        "fileops_messages_get_their_arguments",
    ),
    (
        "app/specular/src/app/fileops.rs",
        "e.message_id()",
        "fileops_messages_get_their_arguments",
    ),
    (
        "app/specular/src/app/fileops.rs",
        "r.method.message_id()",
        "fileops_messages_get_their_arguments",
    ),
    (
        "app/specular/src/app/fileops.rs",
        "reason.message_id()",
        "fileops_messages_get_their_arguments",
    ),
    ("app/specular/src/app/cad.rs", "id", ""),
];

/// Calls that pass other arguments on purpose (tests of the check itself).
const DELIBERATE_TR_ARGS: &[(&str, &str)] =
    &[("crates/specular-i18n/src/tests.rs", "\"unit-percent\"")];

#[test]
fn every_tr_args_call_passes_the_arguments_english_uses() {
    let root = workspace_root();
    let mut problems = Vec::new();
    let mut checked = 0;
    let mut runtime_seen = BTreeSet::new();
    let mut sources = String::new();
    for path in rust_files(&root) {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        let rel = path.strip_prefix(&root).unwrap().display().to_string();
        for c in tr_calls(&src) {
            if DELIBERATE_TR_ARGS.contains(&(rel.as_str(), c.first)) {
                continue;
            }
            let literal = c
                .first
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .filter(|s| !s.contains(['"', '\\']));
            match (literal, passed(&c)) {
                (Some(id), Some(names)) => {
                    checked += 1;
                    if let Some(p) =
                        specular_i18n::argument_problem(id, names.iter().map(String::as_str))
                    {
                        problems.push(format!("{rel}:{}: {p}", c.line));
                    }
                }
                _ => match RUNTIME_TR_ARGS
                    .iter()
                    .find(|(f, first, _)| *f == rel && *first == c.first)
                {
                    Some(&(f, first, _)) => {
                        runtime_seen.insert((f, first));
                    }
                    None => problems.push(format!(
                        "{rel}:{}: tr_args({}, {}): the id or the arguments are made at run \
                         time; list it in RUNTIME_TR_ARGS with a test that formats every case",
                        c.line, c.first, c.second
                    )),
                },
            }
        }
        sources.push_str(&src);
    }
    assert!(checked > 20, "the scanner found only {checked} calls");
    for (f, first, test) in RUNTIME_TR_ARGS {
        if !runtime_seen.contains(&(*f, *first)) {
            problems.push(format!(
                "RUNTIME_TR_ARGS: no call tr_args({first}, …) in {f} any more"
            ));
        }
        if !test.is_empty() && !sources.contains(&format!("fn {test}()")) {
            problems.push(format!("RUNTIME_TR_ARGS: test {test} (for {f}) not found"));
        }
    }
    assert!(
        problems.is_empty(),
        "{} tr_args problem(s):\n{}",
        problems.len(),
        problems.join("\n")
    );
}

#[test]
fn the_tr_args_scanner_reads_ids_and_argument_names() {
    // `TR(` stands for the function (see `NEEDLE` in `tr_calls`).
    let src = r#"
        fn TR(id: &str, args: &[(&str, String)]) -> String { x.TR(id, args) }
        fn f() {
            let a = TR("one", &[("count", n.to_string()), ("name", s)]);
            let mut args = FluentArgs::new();
            args.set("value", 3);
            let b = loc.TR("two", &args);
            let mut v = vec![("file", f)];
            if x { v.push(("reason", r)); }
            let c = TR(e.message_id(), &v);
            // TR("in-a-comment", &[("no", 1)])
        }
    "#
    .replace("TR(", concat!("tr", "_args("));
    let got: Vec<(String, Option<Vec<String>>)> = tr_calls(&src)
        .iter()
        .map(|c| {
            (
                c.first.to_owned(),
                passed(c).map(|n| n.into_iter().collect()),
            )
        })
        .collect();
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert_eq!(
        got,
        vec![
            ("\"one\"".to_owned(), Some(s(&["count", "name"]))),
            ("\"two\"".to_owned(), Some(s(&["value"]))),
            ("e.message_id()".to_owned(), Some(s(&["file", "reason"]))),
        ]
    );
}

/// (file, message, argument) whose value is a number written by hand on purpose.
const HAND_NUMBERS_ALLOWED: &[(&str, &str, &str)] = &[
    // The decimal degrees written into the photos, as geotagging tools show them.
    (
        "app/specular/src/app/map/photos.rs",
        "map-geotag-will-write",
        "position",
    ),
];

/// Message arguments never write decimals by hand (`format!("{x:.2}")` always uses "." and
/// ASCII digits): they go through `specular_i18n::fixed`/`number`, the localizer's number
/// helpers, or are passed as numbers for `NUMBER()`.
#[test]
fn message_arguments_use_the_locale_number_format() {
    let root = workspace_root();
    let decimals = regex::Regex::new(r#"format!\("[^"]*\{[^{}"]*:[^{}"]*\.[0-9]+\}"#).unwrap();
    let mut problems = Vec::new();
    let mut checked = 0;
    for path in rust_files(&root) {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        let rel = path.strip_prefix(&root).unwrap().display().to_string();
        for c in calls(&src) {
            for (name, value) in &c.values {
                checked += 1;
                if decimals.is_match(value)
                    && !HAND_NUMBERS_ALLOWED.contains(&(rel.as_str(), c.id.as_str(), name.as_str()))
                {
                    problems.push(format!("{rel}:{}: {} {name} = {value}", c.line, c.id));
                }
            }
        }
    }
    assert!(checked > 1000, "the scanner found only {checked} arguments");
    assert!(
        problems.is_empty(),
        "{} message argument(s) with decimals written by hand (use specular_i18n::fixed or \
         number):\n{}",
        problems.len(),
        problems.join("\n")
    );
}

#[test]
fn hand_written_decimals_are_recognised() {
    let src =
        r#"let a = T!("x", v = format!("{v:.2}"), w = format!("{:.1} mm", d), n = fixed(v, 2));"#
            .replace("T!(", concat!("t", "!("));
    let decimals = regex::Regex::new(r#"format!\("[^"]*\{[^{}"]*:[^{}"]*\.[0-9]+\}"#).unwrap();
    let hits: Vec<_> = calls(&src)[0]
        .values
        .iter()
        .filter(|(_, v)| decimals.is_match(v))
        .map(|(n, _)| n.clone())
        .collect();
    assert_eq!(hits, ["v", "w"]);
}
