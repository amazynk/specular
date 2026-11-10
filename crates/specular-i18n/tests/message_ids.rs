//! Every message id used in the workspace's Rust code exists in the English catalogue, every
//! English message is translated in every locale, and every catalogue parses and uses the
//! same placeholders as English.
//!
//! Ids are found as string literals in `t!("…")`, `t!(handle => "…")`, `tr("…")` and
//! `tr_args("…", …)`. Code that builds ids at run time (the settings schema) has its own test.

use std::path::{Path, PathBuf};

use regex::Regex;

/// Crates with their own, separate catalogue (not specular-i18n's).
const OWN_CATALOGUE: &[&str] = &["crates/specular-metadata"];

fn workspace_root() -> PathBuf {
    // Canonicalise first: the crate may be built through a symlinked mirror workspace.
    let manifest = std::fs::canonicalize(env!("CARGO_MANIFEST_DIR")).expect("manifest dir");
    manifest
        .ancestors()
        .find(|p| p.join("Cargo.toml").is_file() && p.join("crates").is_dir())
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn all_used_ids_exist_in_english() {
    let root = workspace_root();
    let patterns = [
        Regex::new(r#"\bt!\(\s*(?:[^";]*?=>\s*)?"([^"\\]+)""#).unwrap(),
        Regex::new(r#"\btr(?:_args)?\(\s*"([^"\\]+)""#).unwrap(),
    ];
    let mut used = Vec::new();
    let mut files = 0;
    for entry in walkdir::WalkDir::new(&root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            let rel = e.path().strip_prefix(&root).unwrap_or(e.path());
            !(name.starts_with('.') && e.depth() > 0
                || name == "target"
                || OWN_CATALOGUE.iter().any(|c| rel == Path::new(c)))
        })
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "rs"))
    {
        let Ok(src) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        files += 1;
        for re in &patterns {
            for cap in re.captures_iter(&src) {
                let id = cap[1].to_owned();
                let start = cap.get(0).unwrap().start();
                let line_start = src[..start].rfind('\n').map_or(0, |i| i + 1);
                // Documentation comments describe the syntax with placeholder ids
                // (doc-tests with real ids are still compiled and run).
                if src[line_start..start].trim_start().starts_with("//") {
                    continue;
                }
                let line = src[..start].lines().count();
                used.push((
                    id,
                    entry
                        .path()
                        .strip_prefix(&root)
                        .unwrap()
                        .display()
                        .to_string(),
                    line,
                ));
            }
        }
    }
    assert!(files > 0, "no Rust files found under {}", root.display());
    assert!(!used.is_empty(), "the scanner found no ids at all");
    let missing: Vec<_> = used
        .iter()
        .filter(|(id, _, _)| {
            let msg = id.split('.').next().unwrap_or(id);
            !specular_i18n::english_has(msg)
        })
        .map(|(id, file, line)| format!("{file}:{line}: {id}"))
        .collect();
    assert!(
        missing.is_empty(),
        "message ids missing from i18n/en:\n{}",
        missing.join("\n")
    );
}

#[test]
fn catalogues_parse_and_placeholders_match() {
    let issues = specular_i18n::check_catalogues();
    assert!(issues.is_empty(), "{issues:#?}");
}

/// A new English id must arrive with its translations. The report names each id, its file
/// and the locales that lack it, so the fix is obvious.
#[test]
fn every_english_id_is_translated() {
    let gaps = specular_i18n::untranslated();
    if gaps.is_empty() {
        return;
    }
    let mut report = format!(
        "{} English message(s) have no translation. Add each one to \
         crates/specular-i18n/i18n/<locale>/<file> for the locales listed (draft translations \
         go under a `# machine-drafted, needs native review` comment):\n",
        gaps.len()
    );
    for g in &gaps {
        let locales: Vec<String> = g.missing_in.iter().map(ToString::to_string).collect();
        report.push_str(&format!(
            "  {:<12} {:<40} missing in: {}\n",
            g.file,
            g.id,
            locales.join(", ")
        ));
    }
    panic!("{report}");
}
