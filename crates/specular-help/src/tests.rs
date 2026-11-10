use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::time::Instant;

use super::*;

mod drift;

fn en() -> Manual {
    Manual::load("en")
}

#[test]
fn every_page_parses() {
    for locale in locales() {
        for (file, src) in sources(locale) {
            if let Err(e) = doc::parse(src) {
                panic!("manual/{locale}/{file}: {e}");
            }
        }
    }
    assert_eq!(locales().first(), Some(&"en"));
    assert_eq!(en().pages().len(), sources("en").len());
}

#[test]
fn page_ids_are_unique_and_match_file_names() {
    let mut seen = HashSet::new();
    for (file, src) in sources("en") {
        let p = doc::parse(src).unwrap();
        assert!(seen.insert(p.id.clone()), "two pages are called {}", p.id);
        let stem = file.trim_end_matches(".md");
        let name = stem.split_once('-').map_or(stem, |(_, n)| n);
        assert_eq!(name, p.id, "{file} should be NN-{}.md", p.id);
        let mut anchors = HashSet::new();
        for a in p.anchors() {
            assert!(anchors.insert(a), "{}: anchor #{a} twice", p.id);
        }
    }
}

#[test]
fn contents_follow_sections() {
    let m = en();
    let sections: Vec<Section> = m.pages().iter().map(|p| p.section).collect();
    assert!(sections.is_sorted());
    for s in Section::ALL {
        assert!(sections.contains(&s), "no page in {}", s.id());
    }
    assert_eq!(m.pages()[0].id, WELCOME);
}

#[test]
fn links_resolve() {
    let m = en();
    for p in m.pages() {
        for (page, anchor) in p.links() {
            let Some(target) = m.page(&page) else {
                panic!("{} links to a missing page {page}", p.id);
            };
            assert!(
                anchor.is_empty() || target.anchors().any(|a| a == anchor),
                "{} links to {page}#{anchor}, which has no such section",
                p.id
            );
        }
    }
}

#[test]
fn key_references_name_actions() {
    for p in en().pages() {
        for id in p.key_refs() {
            assert!(
                Action::from_id(id).is_some(),
                "{}: [[key:{id}]] names no action",
                p.id
            );
        }
    }
}

fn assert_topic(m: &Manual, t: Topic, what: &str) {
    let Some(p) = m.page(t.page) else {
        panic!("{what}: no page {}", t.page);
    };
    assert!(
        t.anchor.is_empty() || p.anchors().any(|a| a == t.anchor),
        "{what}: {} has no #{}",
        t.page,
        t.anchor
    );
}

#[test]
fn every_disabled_reason_has_a_topic() {
    let m = en();
    for &d in Disabled::ALL {
        assert_topic(&m, disabled_topic(d), &format!("{d:?}"));
    }
}

#[test]
fn every_action_has_a_topic() {
    let m = en();
    for a in Action::BINDABLE {
        assert_topic(&m, action_topic(*a), &format!("{a:?}"));
    }
}

#[test]
fn every_context_has_a_page() {
    let m = en();
    for c in CONTEXTS {
        assert!(
            m.pages().iter().any(|p| p.contexts.iter().any(|x| x == c)),
            "no page for the context {c}"
        );
    }
    for mode in Mode::ALL {
        let c = context_page(mode);
        assert!(CONTEXTS.contains(&c), "{mode:?}");
    }
    // Contexts are unique, so F1 is unambiguous.
    let mut seen = HashSet::new();
    for p in m.pages() {
        for c in &p.contexts {
            assert!(seen.insert(c.clone()), "context {c} on two pages");
        }
    }
    assert_eq!(m.pages()[m.for_context("nowhere")].id, WELCOME);
    assert_eq!(m.pages()[m.for_context("medical")].id, "medical");
}

/// Each key table is in the manual: on its mode's page and on the keys page.
#[test]
fn every_key_table_is_in_the_manual() {
    let m = en();
    let keys = m.page("keys").unwrap();
    for t in key_tables() {
        assert!(
            keys.blocks.contains(&Block::Keys(t.to_owned())),
            "the keys page lacks ::: keys {t}"
        );
        let own = m.page(t).unwrap_or_else(|| panic!("no page {t}"));
        assert!(own.blocks.contains(&Block::Keys(t.to_owned())), "{t}");
    }
    for p in m.pages() {
        for b in &p.blocks {
            if let Block::Keys(t) = b {
                assert!(key_tables().contains(&t.as_str()), "{}: table {t}", p.id);
            }
        }
    }
}

/// Every mode's table lists every action of the mode (the table's rows are `Mode::actions`),
/// and every bindable action is in some mode's table.
#[test]
fn key_tables_cover_every_action() {
    let mut covered = Vec::new();
    for t in key_tables() {
        if let Some(mode) = Mode::from_id(t) {
            covered.extend(mode.actions());
        }
    }
    for a in Action::BINDABLE {
        assert!(covered.contains(a), "{a:?} is in no key table");
    }
}

fn hits(m: &Manual, q: &str) -> Vec<String> {
    m.search(q, 8)
        .into_iter()
        .map(|h| {
            let id = &m.pages()[h.page].id;
            if h.anchor.is_empty() {
                id.clone()
            } else {
                format!("{id}#{}", h.anchor)
            }
        })
        .collect()
}

#[test]
fn search_finds_the_right_sections() {
    let m = en();
    let top = |q: &str| hits(&m, q).into_iter().next().unwrap_or_default();
    assert_eq!(top("zoom"), "viewer#zoom");
    assert_eq!(top("window level"), "medical#window");
    assert_eq!(top("hillshade"), "maps#terrain");
    assert_eq!(top("crash report"), "diagnostics#crash");
    assert!(top("updates").starts_with("privacy"), "{}", top("updates"));
    // Typing: the last word is a prefix.
    assert!(hits(&m, "dupl").iter().any(|h| h.starts_with("duplicates")));
    assert!(
        hits(&m, "anonym")
            .iter()
            .any(|h| h.starts_with("howto-anonymise"))
    );
    // Case, accents and plurals don't matter.
    assert_eq!(top("ZOOM"), top("zoóm"));
    assert!(
        hits(&m, "thumbnails ")
            .iter()
            .any(|h| h.starts_with("gallery"))
    );
    assert!(
        hits(&m, "thumbnail ")
            .iter()
            .any(|h| h.starts_with("gallery"))
    );
    // Every word must match.
    assert!(hits(&m, "zoom xyzzy").is_empty());
    assert!(hits(&m, "").is_empty());
    assert!(hits(&m, "  ").is_empty());
    // Key tables are searchable by their actions' names.
    let key_names = Action::Cine.name();
    assert!(
        !m.search(&key_names, 20).is_empty(),
        "nothing found for {key_names}"
    );
    // Snippets come from the section and are short.
    for h in m.search("trash", 10) {
        assert!(h.snippet.chars().count() <= 141, "{}", h.snippet);
    }
}

#[test]
fn search_is_instant() {
    let m = en();
    let queries = [
        "z",
        "zo",
        "zoo",
        "zoom",
        "window level",
        "a",
        "e",
        "the image",
        "colour",
    ];
    let start = Instant::now();
    let mut n = 0;
    for _ in 0..20 {
        for q in queries {
            n += m.search(q, 50).len();
        }
    }
    let per = start.elapsed() / (20 * queries.len() as u32);
    assert!(n > 0);
    // Generous for a loaded debug build; it is ~10 µs in practice.
    assert!(per.as_millis() < 20, "{per:?} per query");
}

#[test]
fn loading_falls_back_to_english() {
    let m = Manual::load("xx-YY");
    assert_eq!(m.locale(), "xx-YY");
    assert_eq!(m.pages().len(), en().pages().len());
    assert_eq!(m.pages()[0].title, en().pages()[0].title);
}

#[test]
fn translations_keep_the_skeleton() {
    let english: BTreeMap<&str, Page> = sources("en")
        .iter()
        .map(|(f, s)| (*f, doc::parse(s).unwrap()))
        .collect();
    for locale in locales().into_iter().filter(|l| *l != "en") {
        for (file, src) in sources(locale) {
            let Some(e) = english.get(file) else {
                panic!("manual/{locale}/{file} has no English page");
            };
            let t = doc::parse(src).unwrap();
            assert_eq!(t.id, e.id, "{locale}/{file}");
            assert_eq!(t.skeleton(), e.skeleton(), "{locale}/{file}");
        }
    }
}

#[test]
fn every_section_has_a_heading_string() {
    for s in Section::ALL {
        let id = s.message_id();
        assert_ne!(I18n::global().tr(id), id, "{id} is not in help.ftl");
    }
}

#[test]
fn build_info_is_filled() {
    let b = build_info();
    assert!(!b.version.is_empty());
    assert!(b.target.contains('-'));
    assert_eq!(b.profile, "debug");
}

/// The crates of the app's dependency graph that are built into it (normal dependencies, not
/// build scripts or tests), from `cargo metadata`.
fn shipped_crates() -> BTreeSet<(String, String, String)> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml");
    let out = std::process::Command::new(cargo)
        .args([
            "metadata",
            "--offline",
            "--format-version",
            "1",
            "--manifest-path",
            root,
        ])
        .output()
        .expect("cargo metadata");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let members: HashSet<&str> = v["workspace_members"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m.as_str())
        .collect();
    let packages: BTreeMap<&str, &serde_json::Value> = v["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["id"].as_str().unwrap(), p))
        .collect();
    let nodes: BTreeMap<&str, &serde_json::Value> = v["resolve"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n["id"].as_str().unwrap(), n))
        .collect();
    let app = packages
        .values()
        .find(|p| p["name"] == "specular" && members.contains(p["id"].as_str().unwrap()))
        .map(|p| p["id"].as_str().unwrap())
        .expect("the specular package");
    let mut seen = HashSet::new();
    let mut todo = vec![app];
    let mut out = BTreeSet::new();
    while let Some(id) = todo.pop() {
        if !seen.insert(id) {
            continue;
        }
        let p = packages[id];
        // Procedural macros run in the compiler; they are not built into the app.
        let proc_macro = p["targets"].as_array().unwrap().iter().any(|t| {
            t["kind"]
                .as_array()
                .unwrap()
                .iter()
                .any(|k| k == "proc-macro")
        });
        if proc_macro {
            continue;
        }
        if !members.contains(id) {
            out.insert((
                p["name"].as_str().unwrap().to_owned(),
                p["version"].as_str().unwrap().to_owned(),
                p["license"]
                    .as_str()
                    .unwrap_or("see its repository")
                    .to_owned(),
            ));
        }
        for d in nodes[id]["deps"].as_array().unwrap() {
            let normal = d["dep_kinds"]
                .as_array()
                .unwrap()
                .iter()
                .any(|k| k["kind"].is_null());
            if normal {
                todo.push(d["pkg"].as_str().unwrap());
            }
        }
    }
    out
}

/// `data/third-party.tsv` lists exactly the crates built into the app. After changing
/// dependencies, run this test with `SPECULAR_UPDATE_LICENCES=1` to rewrite it.
#[test]
fn licences_in_sync() {
    let want = shipped_crates();
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/third-party.tsv");
    if std::env::var_os("SPECULAR_UPDATE_LICENCES").is_some() {
        let mut s = String::from(
            "# Third-party crates built into Specular: name, version, licence (SPDX).\n\
             # Generated by specular-help's `licences_in_sync` test; do not edit.\n",
        );
        for (n, v, l) in &want {
            s.push_str(&format!("{n}\t{v}\t{l}\n"));
        }
        std::fs::write(path, s).unwrap();
        return;
    }
    let have: BTreeSet<(String, String, String)> = third_party()
        .into_iter()
        .map(|l| (l.name.into(), l.version.into(), l.licence.into()))
        .collect();
    let missing: Vec<_> = want.difference(&have).collect();
    let extra: Vec<_> = have.difference(&want).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "data/third-party.tsv is out of date (run with SPECULAR_UPDATE_LICENCES=1): \
         missing {missing:?}, extra {extra:?}"
    );
    let groups = third_party_by_licence();
    assert_eq!(
        groups.iter().map(|(_, v)| v.len()).sum::<usize>(),
        have.len()
    );
    assert!(groups.windows(2).all(|w| w[0].1.len() >= w[1].1.len()));
}

/// Sentences in a paragraph: sentence ends followed by a space or the end ("e.g." and
/// "i.e." are not ends).
fn sentences(text: &str) -> usize {
    let t = text.replace("e.g.", "eg").replace("i.e.", "ie");
    let b: Vec<char> = t.trim().chars().collect();
    (0..b.len())
        .filter(|&i| {
            matches!(b[i], '.' | '?' | '!') && b.get(i + 1).is_none_or(|c| c.is_whitespace())
        })
        .count()
        .max(1)
}

/// The house style: short paragraphs (one idea, at most three sentences) and every page
/// broken into separate parts.
#[test]
fn pages_are_short_paragraphs_with_breaks() {
    assert_eq!(sentences("One. Two? Three! e.g. not four."), 4);
    assert_eq!(sentences("No end"), 1);
    for (file, src) in sources("en") {
        let p = doc::parse(src).unwrap();
        let parts = p
            .blocks
            .iter()
            .filter(|b| !matches!(b, Block::Heading { .. }))
            .count();
        assert!(
            parts >= 3,
            "{file}: {parts} parts; break it into paragraphs and lists"
        );
        for b in &p.blocks {
            if let Block::Para(s) | Block::Note(s) | Block::Bullet(s) | Block::Numbered(_, s) = b {
                let text = doc::plain(s, &|k| k.to_owned());
                let n = sentences(&text);
                assert!(n <= 3, "{file}: {n} sentences in one paragraph: {text}");
            }
        }
    }
}

/// Release builds are fat-LTO and profiling builds thin-LTO (a change once dropped
/// both by accident; the speed budgets assume them).
#[test]
fn release_profile_keeps_lto() {
    let toml = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml"));
    let section = |name: &str| -> String {
        let head = format!("[profile.{name}]\n");
        let start = toml
            .find(&head)
            .unwrap_or_else(|| panic!("no [profile.{name}]"))
            + head.len();
        let rest = &toml[start..];
        rest[..rest.find("\n[").unwrap_or(rest.len())].to_owned()
    };
    assert!(
        section("release")
            .lines()
            .any(|l| l.trim() == "lto = \"fat\""),
        "release lost fat LTO"
    );
    assert!(
        section("profiling")
            .lines()
            .any(|l| l.trim() == "lto = \"thin\""),
        "profiling lost thin LTO"
    );
}
