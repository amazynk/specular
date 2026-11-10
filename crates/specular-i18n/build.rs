//! Generates the table of embedded catalogues from `i18n/<locale>/*.ftl`, so adding a
//! locale or a file needs no code change. Every file is embedded with `include_str!`.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;
use std::{env, fs};

fn main() {
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let root = Path::new(&manifest).join("i18n");
    println!("cargo:rerun-if-changed=i18n");

    let mut locales: Vec<_> = fs::read_dir(&root)
        .expect("i18n directory")
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    // English is the source catalogue and must come first.
    locales.sort_by_key(|l| (l != "en", l.clone()));
    assert_eq!(
        locales.first().map(String::as_str),
        Some("en"),
        "i18n/en is required"
    );

    let mut out = String::from(
        "/// Embedded catalogues: (locale, [(file name, FTL source)]), English first.\n\
         pub(crate) const CATALOGUES: &[(&str, &[(&str, &str)])] = &[\n",
    );
    let mut id_sets: Vec<BTreeSet<String>> = Vec::new();
    for locale in &locales {
        let dir = root.join(locale);
        println!("cargo:rerun-if-changed=i18n/{locale}");
        let mut files: Vec<_> = fs::read_dir(&dir)
            .expect("locale directory")
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "ftl"))
            .collect();
        files.sort();
        let _ = writeln!(out, "    ({locale:?}, &[");
        let mut ids = BTreeSet::new();
        for f in files {
            let name = f.file_name().unwrap().to_string_lossy().into_owned();
            let abs = f.canonicalize().expect("canonical path");
            let _ = writeln!(out, "        ({name:?}, include_str!({:?})),", abs);
            ids.extend(message_ids(&fs::read_to_string(&f).expect("read .ftl")));
        }
        out.push_str("    ]),\n");
        id_sets.push(ids);
    }
    out.push_str("];\n");
    // How many English message ids each catalogue defines (English first), so the language
    // list needs no catalogue parsed; a test checks these against the parsed catalogues.
    let en = &id_sets[0];
    out.push_str(
        "/// English message ids each catalogue defines, in `CATALOGUES` order.\n\
         pub(crate) const TRANSLATED: [usize; CATALOGUES.len()] = [",
    );
    for ids in &id_sets {
        let _ = write!(out, "{}, ", ids.intersection(en).count());
    }
    out.push_str("];\n");

    let dest = Path::new(&env::var("OUT_DIR").expect("OUT_DIR")).join("catalogues.rs");
    fs::write(dest, out).expect("write catalogues.rs");
}

/// The message ids an FTL source defines: entries start at the line's first column, and a
/// message's id (unlike a `-term`'s) starts with a letter.
fn message_ids(src: &str) -> impl Iterator<Item = String> + '_ {
    src.lines().filter_map(|l| {
        let (id, _) = l.split_once('=')?;
        let id = id.trim_end();
        let ok = id.starts_with(|c: char| c.is_ascii_alphabetic())
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        ok.then(|| id.to_owned())
    })
}
