//! The package as ebooklib reads it (`tools/corpus/epub-oracle.py` → `oracle.tsv`): title,
//! creators, languages, identifier, spine, table of contents and cover of the fixtures and
//! the corpus (`tools/corpus/fetch-epub.sh`). Skipped when the oracle file is missing; the
//! fixtures must match exactly, corpus mismatches are counted and printed.

use std::collections::BTreeMap;
use std::path::PathBuf;

use specular_epub::{Book, Error, Options};

fn oracle_path() -> PathBuf {
    std::env::var_os("SPECULAR_EPUB_ORACLE").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/epub/oracle.tsv"),
        PathBuf::from,
    )
}

fn ours(b: &Book) -> BTreeMap<&'static str, Vec<String>> {
    let m = b.metadata();
    let mut f: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    f.insert("title", m.title.iter().cloned().collect());
    f.insert(
        "creator",
        m.creators.iter().map(|c| c.name.clone()).collect(),
    );
    f.insert("language", m.languages.clone());
    f.insert("identifier", m.identifier.iter().cloned().collect());
    f.insert("spine", b.spine().iter().map(|s| s.to_string()).collect());
    f.insert(
        "toc",
        // ebooklib drops headings without a link that head nothing.
        b.toc()
            .iter()
            .enumerate()
            .filter(|(i, t)| {
                !t.href.is_empty() || b.toc().get(i + 1).is_some_and(|n| n.depth > t.depth)
            })
            .map(|(_, t)| {
                let target = match &t.fragment {
                    Some(fr) => format!("{}#{fr}", t.href),
                    None => t.href.clone(),
                };
                format!("{}\t{target}", t.title)
            })
            .collect(),
    );
    f
}

#[test]
fn package_matches_ebooklib() {
    let path = oracle_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("skipped: no oracle at {}", path.display());
        return;
    };
    let mut books: BTreeMap<String, BTreeMap<String, Vec<String>>> = BTreeMap::new();
    for line in text.lines() {
        let mut p = line.splitn(3, '\t');
        let (Some(file), Some(key), Some(val)) = (p.next(), p.next(), p.next()) else {
            continue;
        };
        books
            .entry(file.to_owned())
            .or_default()
            .entry(key.to_owned())
            .or_default()
            .push(val.to_owned());
    }
    let mut mismatches: BTreeMap<&str, usize> = BTreeMap::new();
    let mut compared = 0;
    let mut refused = 0;
    let mut fixture_bad = Vec::new();
    for (file, facts) in &books {
        if facts.contains_key("error") {
            continue;
        }
        let Ok(data) = std::fs::read(file) else {
            continue;
        };
        let fixture = file.contains("specular-epub/fixtures");
        let b = match Book::open(data, &Options::default()) {
            Ok(b) => b,
            Err(Error::Drm(_)) => {
                refused += 1;
                continue;
            }
            Err(e) => {
                println!("{file}: open failed: {e}");
                *mismatches.entry("open").or_default() += 1;
                continue;
            }
        };
        compared += 1;
        let mine = ours(&b);
        let get = |k: &str| facts.get(k).cloned().unwrap_or_default();
        // ebooklib keeps the spine's file order; ours puts non-linear documents last.
        let spine: Vec<(String, bool)> = get("spine")
            .iter()
            .map(|s| {
                let (h, l) = s.split_once('\t').unwrap_or((s, "yes"));
                (h.to_owned(), l == "yes")
            })
            .collect();
        let mut want_spine: Vec<String> =
            spine.iter().filter(|s| s.1).map(|s| s.0.clone()).collect();
        want_spine.extend(spine.iter().filter(|s| !s.1).map(|s| s.0.clone()));
        let mut check = |key: &'static str, want: Vec<String>, have: Vec<String>| {
            if want != have {
                *mismatches.entry(key).or_default() += 1;
                println!(
                    "{}: {key}\n  ebooklib {want:?}\n  ours     {have:?}",
                    file.rsplit('/').next().unwrap_or(file)
                );
                if fixture {
                    fixture_bad.push(format!("{file} {key}"));
                }
            }
        };
        check("title", get("title"), mine["title"].clone());
        check("creator", get("creator"), mine["creator"].clone());
        check("language", get("language"), mine["language"].clone());
        check("identifier", get("identifier"), mine["identifier"].clone());
        check("spine", want_spine, mine["spine"].clone());
        check("toc", get("toc"), mine["toc"].clone());
        let covers = get("cover");
        if !covers.is_empty() {
            let have = b.cover_path().unwrap_or_default();
            check(
                "cover",
                vec![covers.contains(&have).to_string()],
                vec!["true".into()],
            );
        }
    }
    println!(
        "epub oracle: {compared} books compared, {refused} refused (DRM), mismatches by field: {mismatches:?}"
    );
    assert!(fixture_bad.is_empty(), "fixtures differ: {fixture_bad:?}");
}
