//! darktable sidecars: `IMG_1234.CR2.xmp` with a `darktable:history`.
//!
//! Only the module names are read: `darktable:operation` of each history item up to
//! `darktable:history_end` (older files list them in `darktable:history_operation`). The
//! parameter blobs are never decoded; Specular does not convert darktable edits.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use super::RDF;

/// darktable's XMP namespace.
pub const NS: &str = "http://darktable.sf.net/";

/// What a darktable sidecar says about a photo.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Darktable {
    /// The modules switched on in the edit, in the order they were first used, by
    /// darktable's own names (`exposure`, `sigmoid`, `colorbalancergb`).
    pub modules: Vec<String>,
}

/// The darktable sidecar of `file` (`IMG_1234.CR2` → `IMG_1234.CR2.xmp`).
pub fn sidecar_path(file: &Path) -> PathBuf {
    let mut s = file.as_os_str().to_owned();
    s.push(".xmp");
    PathBuf::from(s)
}

/// Reads `file`'s darktable sidecar, if it has one with a darktable history.
pub fn read_for(file: &Path) -> Option<Darktable> {
    let p = sidecar_path(file);
    let text = match std::fs::read_to_string(&p) {
        Ok(t) => t,
        // `IMG.CR2.XMP` on a case-sensitive disk.
        Err(_) => std::fs::read_to_string(p.with_extension("XMP")).ok()?,
    };
    detect(&text)
}

/// The darktable edit in XMP text, if it has one.
pub fn detect(text: &str) -> Option<Darktable> {
    let doc = super::parse(text)?;
    let desc = doc
        .descendants()
        .find(|n| n.has_tag_name((RDF, "Description")) && has_darktable(n))?;
    let prop = |n: roxmltree::Node, name: &str| -> Option<String> {
        n.attribute((NS, name)).map(str::to_owned).or_else(|| {
            n.children()
                .find(|c| c.has_tag_name((NS, name)))
                .and_then(|c| c.text())
                .map(|t| t.trim().to_owned())
        })
    };
    let end: Option<usize> = prop(desc, "history_end").and_then(|v| v.parse().ok());
    // (operation, instance) → on, in the order first used.
    let mut seen: Vec<(String, String, bool)> = Vec::new();
    let history = doc.descendants().find(|n| n.has_tag_name((NS, "history")));
    if let Some(h) = history {
        let items = h
            .descendants()
            .filter(|n| n.has_tag_name((RDF, "li")))
            .enumerate();
        for (i, li) in items {
            let num = prop(li, "num").and_then(|v| v.parse().ok()).unwrap_or(i);
            if end.is_some_and(|e| num >= e) {
                continue;
            }
            let Some(op) = prop(li, "operation").filter(|o| !o.is_empty()) else {
                continue;
            };
            let multi = prop(li, "multi_priority").unwrap_or_default();
            let on = prop(li, "enabled").is_none_or(|v| v != "0");
            match seen.iter_mut().find(|(o, m, _)| *o == op && *m == multi) {
                Some(s) => s.2 = on,
                None => seen.push((op, multi, on)),
            }
        }
    } else {
        // darktable before 0.9: parallel lists.
        let ops = super::seq_items(&doc, NS, "history_operation");
        let enabled = super::seq_items(&doc, NS, "history_enabled");
        for (i, op) in ops.iter().enumerate() {
            if end.is_some_and(|e| i >= e) || op.is_empty() {
                continue;
            }
            let on = enabled.get(i).is_none_or(|v| *v != "0");
            match seen.iter_mut().find(|(o, _, _)| o.as_str() == *op) {
                Some(s) => s.2 = on,
                None => seen.push(((*op).to_owned(), String::new(), on)),
            }
        }
        if ops.is_empty() {
            return None;
        }
    }
    let mut modules: Vec<String> = Vec::new();
    for (op, _, on) in seen {
        if on && !modules.contains(&op) {
            modules.push(op);
        }
    }
    Some(Darktable { modules })
}

fn has_darktable(n: &roxmltree::Node) -> bool {
    n.attributes().any(|a| a.namespace() == Some(NS))
        || n.children().any(|c| c.tag_name().namespace() == Some(NS))
}

/// The darktable program, if installed: on `PATH`, else in the usual places.
pub fn find_program() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    find_program_in(
        std::env::var_os("PATH").as_deref(),
        &usual_places(home.as_deref()),
    )
}

/// [`find_program`] with its inputs: the `PATH` value and the places to try after it.
pub fn find_program_in(path: Option<&OsStr>, usual: &[PathBuf]) -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) {
        &["darktable.exe"]
    } else {
        &["darktable"]
    };
    path.into_iter()
        .flat_map(std::env::split_paths)
        .flat_map(|d| names.iter().map(move |n| d.join(n)))
        .chain(usual.iter().cloned())
        .find(|p| p.is_file())
}

/// Where darktable usually installs, beyond `PATH`.
pub fn usual_places(home: Option<&Path>) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        v.push("/Applications/darktable.app/Contents/MacOS/darktable".into());
        if let Some(h) = home {
            v.push(h.join("Applications/darktable.app/Contents/MacOS/darktable"));
        }
    } else if cfg!(windows) {
        for var in ["ProgramFiles", "ProgramW6432", "LOCALAPPDATA"] {
            if let Some(d) = std::env::var_os(var) {
                let d = PathBuf::from(d);
                v.push(d.join("darktable").join("bin").join("darktable.exe"));
                v.push(
                    d.join("Programs")
                        .join("darktable")
                        .join("bin")
                        .join("darktable.exe"),
                );
            }
        }
    } else {
        for p in [
            "/usr/bin/darktable",
            "/usr/local/bin/darktable",
            "/opt/darktable/bin/darktable",
            "/snap/bin/darktable",
            "/var/lib/flatpak/exports/bin/org.darktable.Darktable",
        ] {
            v.push(p.into());
        }
        if let Some(h) = home {
            v.push(h.join(".local/share/flatpak/exports/bin/org.darktable.Darktable"));
            v.push(h.join(".local/bin/darktable"));
        }
    }
    v
}
