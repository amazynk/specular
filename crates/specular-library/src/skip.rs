//! Folders of tools, not of pictures, that the walk passes over: hidden ones always, folders
//! with one of the [`Skip`] names (`target`, `node_modules`…), and folders holding one of its
//! marks (a `.git` checkout, a Python environment's `pyvenv.cfg`, a `CACHEDIR.TAG`). The
//! user edits both lists (Settings → Library); a location's own folder is never passed over.

use std::path::Path;

/// Folders with these names are not walked (Settings → Library, "Skip folders named").
pub const DEFAULT_NAMES: &[&str] = &[
    "node_modules",
    "target",
    "build",
    "__pycache__",
    "site-packages",
    "corpus",
    "corpora",
];

/// Folders holding a file or folder of one of these names are not walked (Settings → Library,
/// "Skip folders holding").
pub const DEFAULT_MARKS: &[&str] = &[".git", "pyvenv.cfg", CACHE_TAG];

/// A cache directory tag (<https://bford.info/cachedir/>); it counts only with its signature.
const CACHE_TAG: &str = "CACHEDIR.TAG";

/// What the walk passes over (besides hidden folders).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skip {
    names: Vec<String>,
    marks: Vec<String>,
}

impl Default for Skip {
    fn default() -> Skip {
        Skip::new(DEFAULT_NAMES, DEFAULT_MARKS)
    }
}

impl Skip {
    pub fn new<S: AsRef<str>>(names: &[S], marks: &[S]) -> Skip {
        let clean = |v: &[S]| -> Vec<String> {
            let mut out: Vec<String> = Vec::new();
            for s in v {
                let s = s.as_ref().trim().trim_end_matches(['/', '\\']).trim();
                if !s.is_empty() && !out.iter().any(|o| o == s) {
                    out.push(s.to_owned());
                }
            }
            out
        };
        Skip {
            names: clean(names),
            marks: clean(marks),
        }
    }

    /// From the settings' text: names separated by commas or new lines.
    pub fn parse(names: &str, marks: &str) -> Skip {
        Skip::new(&list(names), &list(marks))
    }

    /// Nothing is passed over but hidden folders.
    pub fn none() -> Skip {
        Skip::new::<&str>(&[], &[])
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }

    pub fn marks(&self) -> &[String] {
        &self.marks
    }

    /// Whether a folder of this name is passed over (hidden, or one of the names).
    pub fn name(&self, name: &str) -> bool {
        hidden(name) || self.names.iter().any(|n| n == name)
    }

    /// Whether an entry `name` in folder `dir` makes the folder one to pass over.
    pub(crate) fn mark(&self, name: &str, dir: &Path) -> bool {
        self.marks.iter().any(|m| m == name) && (name != CACHE_TAG || cache_tag(&dir.join(name)))
    }
}

/// The names in a settings text: separated by commas or new lines, trimmed, empty ones left
/// out.
pub fn list(text: &str) -> Vec<String> {
    text.split([',', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

pub(crate) fn hidden(name: &str) -> bool {
    name.starts_with('.')
}

/// Whether `path` is a cache directory tag: a file named `CACHEDIR.TAG` that begins with the
/// signature.
fn cache_tag(path: &Path) -> bool {
    const SIGNATURE: &[u8] = b"Signature: 8a477f597d28d172789f06886806bc55";
    let mut head = [0u8; SIGNATURE.len()];
    specular_core::open_regular(path)
        .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut head))
        .is_ok_and(|()| head == SIGNATURE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_text_is_split_trimmed_and_deduplicated() {
        let s = Skip::parse(
            " target, build/ ,\n\nnode_modules,target",
            ".git,, pyvenv.cfg ",
        );
        assert_eq!(s.names(), ["target", "build", "node_modules"]);
        assert_eq!(s.marks(), [".git", "pyvenv.cfg"]);
        assert!(s.name("target") && s.name(".cache") && !s.name("Target"));
        assert!(!Skip::none().name("target") && Skip::none().name(".git"));
    }

    #[test]
    fn the_defaults_cover_developer_folders() {
        let s = Skip::default();
        for n in ["target", "node_modules", "build", "__pycache__", "corpus"] {
            assert!(s.name(n), "{n}");
        }
        for m in [".git", "pyvenv.cfg"] {
            assert!(s.mark(m, Path::new("/nonexistent")), "{m}");
        }
        // A cache tag counts only with its signature.
        assert!(!s.mark(CACHE_TAG, Path::new("/nonexistent")));
        assert!(!s.mark("photo.jpg", Path::new("/nonexistent")));
    }
}
