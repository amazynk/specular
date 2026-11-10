//! What the client sends back: `picks.json` or `picks.txt` from the gallery page, or a list
//! pasted from a message (one name per line, or separated by commas or semicolons), and which
//! photos those names are.
//!
//! A name matches a photo by its file name, ignoring case. A name with no such photo matches
//! by its stem instead (`IMG_1.jpg` finds `IMG_1.CR3`, and both halves of a raw + JPEG pair).
//! The page names a photo whose name another photo of the gallery shares as `folder/name`;
//! the folder then picks among the photos of that name. A name photos in different folders
//! still answer to is not applied: it is reported as ambiguous.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// The names read, in the client's order, without repeats.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Picks {
    pub names: Vec<String>,
    /// The gallery's title, when the file says it.
    pub gallery: Option<String>,
}

/// Reads picks from the text of a `picks.json`, a `picks.txt` or a pasted list.
pub fn parse(text: &str) -> Picks {
    let text = text.trim_start_matches('\u{feff}');
    let t = text.trim_start();
    if (t.starts_with('{') || t.starts_with('['))
        && let Ok(v) = serde_json::from_str::<serde_json::Value>(t)
    {
        return from_json(&v);
    }
    let mut out = Picks::default();
    let mut seen = HashSet::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(c) = line.strip_prefix('#') {
            if let Some(g) = c.trim().strip_prefix("gallery:") {
                let g = g.trim();
                if !g.is_empty() {
                    out.gallery = Some(g.to_owned());
                }
            }
            continue;
        }
        for part in line.split([',', ';', '\t']) {
            push(&mut out.names, &mut seen, part);
        }
    }
    out
}

fn push(names: &mut Vec<String>, seen: &mut HashSet<String>, s: &str) {
    let s = s
        .trim()
        .trim_matches(|c| matches!(c, '"' | '\'' | '“' | '”' | '‘' | '’'))
        .trim();
    // A path: its file name and the folder it is in (`folder/name`, as the page names a
    // photo whose name another photo has too).
    let parts: Vec<&str> = s
        .split(['/', '\\'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    let s = match parts.as_slice() {
        [] => return,
        [.., dir, name] => format!("{dir}/{name}"),
        [name] => (*name).to_owned(),
    };
    if seen.insert(s.to_lowercase()) {
        names.push(s);
    }
}

fn from_json(v: &serde_json::Value) -> Picks {
    let mut out = Picks::default();
    let mut seen = HashSet::new();
    let list = match v {
        serde_json::Value::Array(a) => Some(a),
        serde_json::Value::Object(o) => {
            out.gallery = o
                .get("gallery")
                .and_then(|g| g.as_str())
                .map(str::trim)
                .filter(|g| !g.is_empty())
                .map(str::to_owned);
            o.get("picks").and_then(|p| p.as_array())
        }
        _ => None,
    };
    for item in list.into_iter().flatten() {
        match item {
            serde_json::Value::String(s) => push(&mut out.names, &mut seen, s),
            serde_json::Value::Object(o) => {
                if let Some(s) = o.get("name").and_then(|n| n.as_str()) {
                    push(&mut out.names, &mut seen, s);
                }
            }
            _ => {}
        }
    }
    out
}

/// Which photos the names are.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Matched {
    /// Each name that matched, with its photos (one, or a raw + JPEG pair).
    pub found: Vec<(String, Vec<PathBuf>)>,
    /// Names no photo answers to.
    pub missing: Vec<String>,
    /// Names photos in more than one folder answer to (not applied).
    pub ambiguous: Vec<String>,
}

impl Matched {
    /// Every matched photo, once each, in the client's order.
    pub fn photos(&self) -> Vec<PathBuf> {
        let mut seen = HashSet::new();
        self.found
            .iter()
            .flat_map(|(_, p)| p.iter())
            .filter(|p| seen.insert((*p).clone()))
            .cloned()
            .collect()
    }
}

fn lower_name(p: &Path) -> Option<String> {
    p.file_name().map(|n| n.to_string_lossy().to_lowercase())
}

fn lower_stem(s: &str) -> String {
    match s.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem.to_lowercase(),
        _ => s.to_lowercase(),
    }
}

/// Matches `picks` to `photos` (the photos Specular lists).
pub fn match_photos(picks: &Picks, photos: &[PathBuf]) -> Matched {
    let mut by_name: HashMap<String, Vec<&PathBuf>> = HashMap::new();
    let mut by_stem: HashMap<String, Vec<&PathBuf>> = HashMap::new();
    for p in photos {
        if let Some(n) = lower_name(p) {
            by_stem.entry(lower_stem(&n)).or_default().push(p);
            by_name.entry(n).or_default().push(p);
        }
    }
    let one_folder = |v: &[&PathBuf]| {
        let mut dirs = v.iter().map(|p| p.parent());
        let first = dirs.next();
        dirs.all(|d| d == first.flatten())
    };
    let mut out = Matched::default();
    for name in &picks.names {
        let key = name.to_lowercase();
        let (dir, file) = match key.rsplit_once('/') {
            Some((d, f)) => (Some(d), f),
            None => (None, key.as_str()),
        };
        let hit = by_name
            .get(file)
            .filter(|v| !v.is_empty())
            .or_else(|| by_stem.get(&lower_stem(file)));
        // Several photos of that name: the folder named picks among them.
        let hit: Option<Vec<&PathBuf>> = hit.map(|v| {
            let in_dir: Vec<&PathBuf> = v
                .iter()
                .copied()
                .filter(|p| {
                    dir.is_some_and(|d| {
                        p.parent()
                            .and_then(|q| q.file_name())
                            .is_some_and(|n| n.to_string_lossy().to_lowercase() == d)
                    })
                })
                .collect();
            if v.len() > 1 && !in_dir.is_empty() {
                in_dir
            } else {
                v.clone()
            }
        });
        match hit {
            None => out.missing.push(name.clone()),
            Some(v) if v.len() > 1 && !one_folder(&v) => out.ambiguous.push(name.clone()),
            Some(v) => out
                .found
                .push((name.clone(), v.iter().map(|p| (*p).clone()).collect())),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_pages_files_and_pasted_lists() {
        let txt = "# Specular client picks\n# gallery: Smith wedding\n# id: 1f\nIMG_1.CR3\nIMG_7.CR3\n\nimg_1.cr3\n";
        let p = parse(txt);
        assert_eq!(p.names, ["IMG_1.CR3", "IMG_7.CR3"]);
        assert_eq!(p.gallery.as_deref(), Some("Smith wedding"));

        let json = r#"{"format":"specular-picks","version":1,"gallery":"Smith","id":"1f","picks":["a.jpg","b.jpg"]}"#;
        let p = parse(json);
        assert_eq!(p.names, ["a.jpg", "b.jpg"]);
        assert_eq!(p.gallery.as_deref(), Some("Smith"));
        assert_eq!(
            parse(r#"["x.jpg", {"name": "y.jpg"}, 3]"#).names,
            ["x.jpg", "y.jpg"]
        );

        let pasted = "\u{feff}\"IMG_2.jpg\", IMG_3.jpg; C:\\Photos\\IMG_4.jpg\t/home/a/IMG_5.jpg";
        assert_eq!(
            parse(pasted).names,
            ["IMG_2.jpg", "IMG_3.jpg", "Photos/IMG_4.jpg", "a/IMG_5.jpg"]
        );
        assert!(parse("").names.is_empty());
        // Broken JSON is read as text.
        assert_eq!(parse("{a.jpg").names, ["{a.jpg"]);
    }

    #[test]
    fn matches_names_stems_pairs_and_reports_the_rest() {
        let photos: Vec<PathBuf> = [
            "/s/IMG_1.CR3",
            "/s/IMG_1.JPG",
            "/s/IMG_2.CR3",
            "/s/IMG_3.jpg",
            "/t/IMG_3.jpg",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        let picks = Picks {
            names: [
                "img_2.cr3",
                "IMG_1.webp",
                "IMG_1.CR3",
                "IMG_3.jpg",
                "IMG_9.jpg",
                // The page names a shared name with its folder; a folder no photo of
                // that name is in is ignored.
                "T/img_3.jpg",
                "Downloads/IMG_2.CR3",
            ]
            .map(String::from)
            .to_vec(),
            gallery: None,
        };
        let m = match_photos(&picks, &photos);
        assert_eq!(m.found.len(), 5);
        assert_eq!(m.found[3].1, [PathBuf::from("/t/IMG_3.jpg")]);
        assert_eq!(m.found[4].1, [PathBuf::from("/s/IMG_2.CR3")]);
        assert_eq!(m.found[0].1, [PathBuf::from("/s/IMG_2.CR3")]);
        // By stem: the pair.
        assert_eq!(m.found[1].1.len(), 2);
        // By name: just that file.
        assert_eq!(m.found[2].1, [PathBuf::from("/s/IMG_1.CR3")]);
        assert_eq!(m.ambiguous, ["IMG_3.jpg"]);
        assert_eq!(m.missing, ["IMG_9.jpg"]);
        assert_eq!(m.photos().len(), 4);
    }
}
