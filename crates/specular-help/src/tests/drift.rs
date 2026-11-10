//! The manual names what the interface shows: every "Settings › Section › Setting" path and
//! every bold or quoted label exists in the English catalogues, so a renamed menu, button or
//! setting can't leave the manual behind silently.

use std::collections::HashSet;

use specular_i18n::{I18n, Localizer, langid, strip_isolates};
use specular_settings::{Section as SettingsSection, in_section};

use super::*;

/// Bold text that is not a label of the interface: lead words, formats, kinds of error.
const BOLD_PROSE: &[&str] = &[
    // Lead words of definition lists.
    "darktable:",
    "Lightroom and Camera Raw:",
    // Formats and decoders, as the Optional decoders page groups them.
    "Zeiss CZI",
    "RED R3D",
    "MrSID",
    "ECW",
    "DWG",
    "Blackmagic RAW",
    "STEP and IGES",
    // Kinds of error on the troubleshooting page (their messages name the file).
    "The image needs more memory than is free.",
    "The format or variant is not supported.",
    "The file is password-protected.",
    "The file is damaged or incomplete",
    // What the privacy page says goes over the network.
    "Update checks",
    "Local AI downloads",
    "Servers you add",
    "Base maps and map services",
    "Network streams",
];

/// Quoted text that is an example or a typed value, not a label.
const QUOTE_PROSE: &[&str] = &["audio description", "dog on a beach", "For Teaching"];

/// What the comparison ignores: isolation marks, the spelling of the ellipsis and
/// apostrophes, a trailing colon, case.
fn norm(s: &str) -> String {
    strip_isolates(s)
        .replace("...", "…")
        .replace('\'', "’")
        .trim()
        .trim_end_matches(':')
        .trim()
        .to_lowercase()
}

/// The English catalogues' texts: every message's value and every variant of a selector, as
/// literal pieces between placeholders (`Cloud { $count } changes` → `cloud `, ` changes`).
struct Catalogue {
    exact: HashSet<String>,
    patterns: Vec<Vec<String>>,
}

impl Catalogue {
    fn english() -> Catalogue {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
        let mut texts = Vec::new();
        for dir in ["specular-i18n/i18n/en", "specular-metadata/i18n/en"] {
            let mut files: Vec<_> = std::fs::read_dir(format!("{root}/{dir}"))
                .unwrap_or_else(|e| panic!("{dir}: {e}"))
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "ftl"))
                .collect();
            files.sort();
            assert!(!files.is_empty(), "no catalogue in {dir}");
            for f in files {
                let src = std::fs::read_to_string(&f).unwrap();
                texts.extend(src.lines().filter_map(ftl_text).map(str::to_owned));
            }
        }
        let mut exact = HashSet::new();
        let mut patterns = Vec::new();
        for t in texts {
            let t = norm(&t.replace("{ -app-name }", "Specular"));
            if t.contains('{') {
                let pieces = literal_pieces(&t);
                // A pattern of nothing but placeholders and punctuation would match anything.
                let letters: usize = pieces
                    .iter()
                    .map(|p| p.chars().filter(|c| c.is_alphabetic()).count())
                    .sum();
                if letters >= 4 {
                    patterns.push(pieces);
                }
            } else if !t.is_empty() {
                exact.insert(t);
            }
        }
        Catalogue { exact, patterns }
    }

    fn has(&self, label: &str) -> bool {
        let l = norm(label);
        self.exact.contains(&l) || self.patterns.iter().any(|p| matches_pieces(p, &l))
    }
}

/// The text a line of a `.ftl` file contributes: a message's value, a variant, or a
/// continuation line; not comments, attributes, selectors or ids alone.
fn ftl_text(line: &str) -> Option<&str> {
    if line.starts_with(|c: char| c.is_ascii_lowercase()) {
        return line.split_once(" =").map(|(_, v)| v.trim());
    }
    if !line.starts_with(' ') {
        return None;
    }
    let t = line.trim_start();
    let t = t.strip_prefix('*').unwrap_or(t);
    if t.starts_with('[') {
        return t.split_once("] ").map(|(_, v)| v.trim());
    }
    if t.starts_with('.') || t.starts_with('}') || t.ends_with("->") {
        return None;
    }
    Some(t.trim())
}

/// `a { $x } b` → `["a ", " b"]` (an empty piece where a placeholder starts or ends it).
fn literal_pieces(t: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = t;
    loop {
        match rest.find('{') {
            Some(i) => {
                out.push(rest[..i].to_owned());
                rest = rest[i..].find('}').map_or("", |j| &rest[i + j + 1..]);
            }
            None => {
                out.push(rest.to_owned());
                return out;
            }
        }
    }
}

/// Whether `s` is the pieces with something in place of each placeholder.
fn matches_pieces(pieces: &[String], s: &str) -> bool {
    let (first, rest) = pieces.split_first().expect("at least one piece");
    let Some(mut s) = s.strip_prefix(first.as_str()) else {
        return false;
    };
    for (i, p) in rest.iter().enumerate() {
        let last = i + 1 == rest.len();
        if last {
            return s.len() > p.len() && s.ends_with(p.as_str());
        }
        // Each placeholder stands for at least one character.
        let skip = s.chars().next().map_or(0, char::len_utf8);
        match s.get(skip..).and_then(|t| t.find(p.as_str())) {
            Some(j) => s = &s[skip + j + p.len()..],
            None => return false,
        }
    }
    s.is_empty()
}

/// A key as the manual writes it: "Ctrl+Shift+T", "Esc", "Arrow keys", "F2", "M".
fn is_key(s: &str) -> bool {
    const NAMED: &[&str] = &[
        "Ctrl",
        "Shift",
        "Alt",
        "Cmd",
        "Enter",
        "Esc",
        "Tab",
        "Space",
        "Backspace",
        "Delete",
        "Home",
        "End",
        "Page Up",
        "Page Down",
        "Arrow keys",
        "Left",
        "Right",
        "Up",
        "Down",
    ];
    s.split('+').all(|k| {
        NAMED.contains(&k)
            || (k.chars().count() == 1 && !k.starts_with(' '))
            || k.strip_prefix('F')
                .is_some_and(|n| n.parse::<u8>().is_ok_and(|n| (1..=24).contains(&n)))
    })
}

/// A setting's label (with its per-system variants) and its options' labels.
struct SettingLabels {
    names: Vec<String>,
    options: Vec<String>,
}

/// The Settings window's labels: section titles and their settings.
struct SettingsLabels {
    sections: Vec<(String, Vec<SettingLabels>)>,
}

/// The label at the start of `text` that is followed by the end, punctuation, or prose
/// going on in lower case ("Sound language picks a track"), the longest if several.
fn label_at<'a>(labels: impl Iterator<Item = &'a String>, text: &str) -> Option<&'a String> {
    labels
        .filter(|l| {
            text.strip_prefix(l.as_str()).is_some_and(|after| {
                let mut c = after.chars();
                match c.next() {
                    None => true,
                    Some(' ') => c.next().is_none_or(|n| !n.is_uppercase()),
                    Some(ch) => !ch.is_alphanumeric(),
                }
            })
        })
        .max_by_key(|l| l.len())
}

impl SettingsLabels {
    fn english() -> SettingsLabels {
        let i18n = I18n::for_language(langid!("en"));
        let en = Localizer::english();
        let with_os = |id: &str| -> Vec<String> {
            let mut v = vec![en.tr(id)];
            for os in ["linux", "windows", "macos"] {
                let o = format!("{id}-{os}");
                if specular_i18n::english_has(&o) {
                    v.push(en.tr(&o));
                }
            }
            v
        };
        let sections = SettingsSection::ALL
            .iter()
            .map(|&s| {
                let settings = in_section(s)
                    .map(|m| SettingLabels {
                        names: with_os(m.label_id),
                        options: m.options().iter().map(|o| en.tr(&m.option_id(o))).collect(),
                    })
                    .collect();
                (s.label(&i18n), settings)
            })
            .collect();
        SettingsLabels { sections }
    }

    /// Checks the path at the start of `rest` (the text after "Settings › "): its section,
    /// the setting after a second "›", its option after a third. Prose may follow.
    fn check(&self, rest: &str) -> Result<(), String> {
        let titles: Vec<&String> = self.sections.iter().map(|(t, _)| t).collect();
        let Some(title) = label_at(titles.into_iter(), rest) else {
            return Err(format!("no section “{}”", head(rest)));
        };
        let (_, settings) = self.sections.iter().find(|(t, _)| t == title).unwrap();
        let Some(after) = rest[title.len()..].strip_prefix(" › ") else {
            return Ok(());
        };
        let Some((setting, name)) = settings
            .iter()
            .filter_map(|s| label_at(s.names.iter(), after).map(|n| (s, n)))
            .max_by_key(|(_, n)| n.len())
        else {
            return Err(format!("{title} has no setting “{}”", head(after)));
        };
        let Some(option) = after[name.len()..].strip_prefix(" › ") else {
            return Ok(());
        };
        match label_at(setting.options.iter(), option) {
            Some(_) => Ok(()),
            None => Err(format!("{title} › {name} has no option “{}”", head(option))),
        }
    }
}

fn head(s: &str) -> String {
    s.chars().take(48).collect()
}

/// The manual's "Settings › …" paths name sections, settings and options that exist, and are
/// written with "›".
#[test]
fn settings_paths_exist() {
    let labels = SettingsLabels::english();
    let mut problems = Vec::new();
    for p in en().pages() {
        for b in &p.blocks {
            let text = doc::plain(doc::spans_of(b), &|k| k.to_owned());
            for (i, _) in text.match_indices("Settings ") {
                let rest = &text[i + "Settings ".len()..];
                if rest.starts_with('→') || rest.starts_with("> ") {
                    problems.push(format!("{}: write “Settings › …”: {}", p.id, head(rest)));
                } else if let Some(path) = rest.strip_prefix("› ")
                    && let Err(e) = labels.check(path)
                {
                    problems.push(format!("{}: {e}", p.id));
                }
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Bold text names a label (a menu item, button, setting or message, in either English
/// catalogue) or a key, else is listed in `BOLD_PROSE`; quoted text likewise, else is in
/// `QUOTE_PROSE`. The lists hold only what is used and is no label.
#[test]
fn quoted_labels_exist() {
    let cat = Catalogue::english();
    let mut problems = Vec::new();
    let mut used = HashSet::new();
    let mut check = |page: &str, s: &str, prose: &[&str], what: &str| {
        if prose.contains(&s) {
            used.insert(s.to_owned());
        } else if !cat.has(s) && !is_key(s) {
            problems.push(format!("{page}: {what} “{s}” is no label"));
        }
    };
    for p in en().pages() {
        for b in &p.blocks {
            for sp in doc::spans_of(b) {
                match sp {
                    Span::Strong(s) => check(&p.id, s, BOLD_PROSE, "bold"),
                    Span::Text(t) => quotes(t)
                        .into_iter()
                        .for_each(|q| check(&p.id, q, QUOTE_PROSE, "quoted")),
                    _ => {}
                }
            }
        }
    }
    for s in BOLD_PROSE.iter().chain(QUOTE_PROSE) {
        if !used.contains(*s) {
            problems.push(format!("“{s}” is allowed as prose but no page uses it"));
        } else if cat.has(s) {
            problems.push(format!("“{s}” is allowed as prose but is a label"));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The quoted pieces of a text: between straight double quotes or curly ones.
fn quotes(t: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = t;
    while let Some(i) = rest.find(['"', '“']) {
        let close = if rest[i..].starts_with('“') {
            '”'
        } else {
            '"'
        };
        let start = i + rest[i..].chars().next().map_or(1, char::len_utf8);
        let Some(j) = rest[start..].find(close) else {
            break;
        };
        out.push(&rest[start..start + j]);
        rest = &rest[start + j + close.len_utf8()..];
    }
    out
}

#[test]
fn the_checks_check() {
    assert_eq!(quotes("a \"b\" c “d e” f"), ["b", "d e"]);
    assert!(is_key("Ctrl+Shift+T") && is_key("F2") && is_key("Esc") && !is_key("Hello"));
    assert_eq!(norm("Save..."), norm("save…"));
    assert_eq!(
        literal_pieces("cloud { $n } changes"),
        ["cloud ", " changes"]
    );
    assert!(matches_pieces(
        &literal_pieces("cloud { $n } changes"),
        "cloud 3 changes"
    ));
    assert!(!matches_pieces(
        &literal_pieces("cloud { $n } changes"),
        "cloud changes"
    ));
    assert!(matches_pieces(
        &literal_pieces("{ $a } needs { $b }"),
        "… needs …"
    ));
    assert!(!matches_pieces(
        &literal_pieces("{ $a } needs { $b }"),
        "x needs "
    ));
    assert_eq!(ftl_text("a-b = Open…"), Some("Open…"));
    assert_eq!(ftl_text("   *[other] { $n } files"), Some("{ $n } files"));
    assert_eq!(ftl_text("# a comment"), None);
    let cat = Catalogue::english();
    assert!(cat.has("Cloud 3 changes") && cat.has("Substituted fonts"));
    assert!(!cat.has("Cloud changes"));
    let s = SettingsLabels::english();
    assert!(s.check("Viewing › Sound language picks a track.").is_ok());
    assert!(s.check("Viewing › Open STEP and IGES").is_err());
    assert!(
        s.check("Metadata and privacy › Base map › My own map file.")
            .is_ok()
    );
    assert!(s.check("Metadata › Base map").is_err());
}
