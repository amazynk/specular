//! Privacy redaction for crash reports, logs and settings.
//!
//! Always: the home folder becomes `~` and the user name, where it is a path component,
//! `<user>`. By default also file and folder names: paths the app knows of (the open files,
//! the library's locations) and anything that looks like a path or a `name.ext` file name
//! become `dir-N` and `file-N.ext`, numbered consistently so one file keeps one name across
//! the whole bundle. Source paths in backtraces (`.rs`, the toolchain, cargo's registry) and
//! web addresses stay readable.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

/// Settings keys whose values are never included (passwords, tokens, servers, accounts).
const SENSITIVE: &[&str] = &[
    "password", "passwd", "token", "secret", "key", "user", "login", "account", "url", "server",
    "host", "email", "path", "folder", "dir", "location", "recent",
];

/// Extensions of files that are not the user's (sources, libraries, config) and words with a
/// dot that are not files.
const KEEP_EXT: &[&str] = &[
    "rs", "so", "dll", "dylib", "exe", "toml", "ftl", "com", "org", "net", "io", "md", "wgsl",
];

/// Folders kept as they are: system locations say nothing about the user.
const KEEP_DIR: &[&str] = &[
    "~",
    "<user>",
    "",
    ".",
    "..",
    "usr",
    "lib",
    "lib64",
    "bin",
    "etc",
    "opt",
    "var",
    "tmp",
    "proc",
    "media",
    "mnt",
    "Volumes",
    "dev",
    "sys",
    "run",
    "home",
    "Users",
    "Windows",
    "System32",
    "Program Files",
    "AppData",
    "Library",
    "Applications",
    "nix",
    "store",
    "snap",
    "flatpak",
    "app",
];

#[derive(Default)]
struct Names {
    files: HashMap<String, usize>,
    dirs: HashMap<String, usize>,
}

impl Names {
    fn file(&mut self, name: &str) -> String {
        let n = self.files.len() + 1;
        let n = *self.files.entry(name.to_owned()).or_insert(n);
        match name.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() && is_ext(ext) => {
                format!("file-{n}.{}", ext.to_ascii_lowercase())
            }
            _ => format!("file-{n}"),
        }
    }

    fn dir(&mut self, name: &str) -> String {
        let n = self.dirs.len() + 1;
        let n = *self.dirs.entry(name.to_owned()).or_insert(n);
        format!("dir-{n}")
    }
}

/// Removes personal details from text. Cheap to share: numbering is kept per redactor.
pub struct Redactor {
    home: Option<String>,
    user: Option<String>,
    hide_names: bool,
    /// Paths the app knows (longest first), replaced whole even when they contain spaces.
    known: Vec<String>,
    names: Mutex<Names>,
}

impl std::fmt::Debug for Redactor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Redactor")
            .field("hide_names", &self.hide_names)
            .field("known", &self.known.len())
            .finish_non_exhaustive()
    }
}

impl Redactor {
    /// `home` and `user` as found on this computer; file names hidden.
    pub fn new(home: Option<&Path>, user: Option<&str>) -> Redactor {
        let home = home
            .map(|h| h.to_string_lossy().trim_end_matches(['/', '\\']).to_owned())
            .filter(|h| h.len() > 1);
        Redactor {
            home,
            user: user.map(str::to_owned).filter(|u| u.len() > 1),
            hide_names: true,
            known: Vec::new(),
            names: Mutex::new(Names::default()),
        }
    }

    /// For this computer: `HOME`/`USERPROFILE` and `USER`/`USERNAME`.
    pub fn from_env() -> Redactor {
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
        let user = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .ok();
        Redactor::new(home.as_deref().map(Path::new), user.as_deref())
    }

    /// Whether file and folder names are hidden (the default) or kept.
    pub fn hide_file_names(mut self, hide: bool) -> Redactor {
        self.hide_names = hide;
        self
    }

    pub fn hides_file_names(&self) -> bool {
        self.hide_names
    }

    /// Paths the app knows of (open files, library locations), hidden whole.
    pub fn known_paths(mut self, paths: impl IntoIterator<Item = String>) -> Redactor {
        self.known.extend(paths.into_iter().filter(|p| p.len() > 1));
        self.known.sort_by_key(|p| std::cmp::Reverse(p.len()));
        self.known.dedup();
        self
    }

    /// `text` with personal details removed.
    pub fn redact(&self, text: &str) -> String {
        let mut s = text.to_owned();
        let mut names = self
            .names
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.hide_names {
            for p in &self.known {
                if s.contains(p.as_str()) {
                    let r = self.hide_path(&self.home_to_tilde(p), &mut names, true);
                    s = s.replace(p.as_str(), &r);
                }
            }
        }
        s = self.home_to_tilde(&s);
        if let Some(u) = &self.user {
            s = replace_component(&s, u, "<user>");
        }
        if self.hide_names {
            s = self.hide_tokens(&s, &mut names);
        }
        s
    }

    fn home_to_tilde(&self, s: &str) -> String {
        let Some(h) = &self.home else {
            return s.to_owned();
        };
        let mut out = s.replace(h.as_str(), "~");
        let other = if h.contains('\\') {
            h.replace('\\', "/")
        } else {
            h.replace('/', "\\")
        };
        if other != *h {
            out = out.replace(other.as_str(), "~");
        }
        out
    }

    /// Every path-like or file-name-like token in `s`, hidden.
    fn hide_tokens(&self, s: &str, names: &mut Names) -> String {
        let mut out = String::with_capacity(s.len());
        let mut tok = String::new();
        for c in s.chars() {
            if c.is_whitespace()
                || matches!(c, '"' | '\'' | '(' | ')' | '[' | ']' | ',' | '=' | '`')
            {
                out.push_str(&self.hide_token(&tok, names));
                tok.clear();
                out.push(c);
            } else {
                tok.push(c);
            }
        }
        out.push_str(&self.hide_token(&tok, names));
        out
    }

    fn hide_token(&self, tok: &str, names: &mut Names) -> String {
        // Trailing punctuation and `:line:col` stay outside the name.
        let core = tok.trim_end_matches(['.', ';', ':', '!', '?']);
        let (core, _) = split_line_col(core);
        let tail = &tok[core.len()..];
        if core.is_empty() || keep_token(core) || is_placeholder(core) {
            return tok.to_owned();
        }
        let is_path = core.contains('/') || core.contains('\\');
        if is_path {
            let hidden = self.hide_path(core, names, false);
            return format!("{hidden}{tail}");
        }
        if looks_like_file(core) {
            return format!("{}{tail}", names.file(core));
        }
        tok.to_owned()
    }

    /// Hides each folder and the file name of `path`, keeping system folders and the
    /// separators. `whole`: the path is known to be a file or folder of the user's.
    fn hide_path(&self, path: &str, names: &mut Names, whole: bool) -> String {
        let sep = if path.contains('\\') && !path.contains('/') {
            '\\'
        } else {
            '/'
        };
        let parts: Vec<&str> = path.split(sep).collect();
        let last = parts.len() - 1;
        let mut out = Vec::with_capacity(parts.len());
        for (i, p) in parts.iter().enumerate() {
            let drive = i == 0 && p.len() == 2 && p.ends_with(':');
            if drive || KEEP_DIR.contains(p) || is_placeholder(p) {
                out.push((*p).to_owned());
            } else if i == last && (whole || looks_like_file(p)) {
                out.push(names.file(p));
            } else {
                out.push(names.dir(p));
            }
        }
        out.join(&sep.to_string())
    }
}

/// A name this redactor gave: `dir-N`, `file-N`, `file-N.ext`.
fn is_placeholder(s: &str) -> bool {
    let s = s.split_once('.').map_or(s, |(stem, _)| stem);
    let n = s.strip_prefix("dir-").or_else(|| s.strip_prefix("file-"));
    n.is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// `a/b.rs:12:5` → (`a/b.rs`, `:12:5`).
fn split_line_col(s: &str) -> (&str, &str) {
    let mut end = s.len();
    for _ in 0..2 {
        let head = &s[..end];
        match head.rfind(':') {
            Some(i) if i + 1 < head.len() && head[i + 1..].bytes().all(|b| b.is_ascii_digit()) => {
                end = i;
            }
            _ => break,
        }
    }
    (&s[..end], &s[end..])
}

fn is_ext(e: &str) -> bool {
    (1..=5).contains(&e.len())
        && e.bytes().all(|b| b.is_ascii_alphanumeric())
        && e.bytes().any(|b| b.is_ascii_alphabetic())
}

/// `name.ext` with a plausible extension (not a version number, not a Rust path).
fn looks_like_file(s: &str) -> bool {
    match s.rsplit_once('.') {
        Some((stem, ext)) => {
            !stem.is_empty()
                && is_ext(ext)
                && !KEEP_EXT.contains(&ext.to_ascii_lowercase().as_str())
                && !stem.bytes().all(|b| b.is_ascii_digit() || b == b'.')
        }
        None => false,
    }
}

/// Tokens that are not the user's: web addresses, Rust paths, sources and the toolchain.
fn keep_token(t: &str) -> bool {
    t.contains("://")
        || t.contains("::")
        || t.starts_with('<')
        || t.contains(".cargo")
        || t.contains("rustc")
        || t.contains("/rustlib/")
        || t.starts_with("specular")
        || t.rsplit_once('.')
            .is_some_and(|(_, e)| KEEP_EXT.contains(&e))
}

/// `name` replaced where it is a whole path component (between separators, or after one at
/// the end of a token).
fn replace_component(s: &str, name: &str, with: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(name) {
        let before = rest[..i].chars().next_back();
        let after = rest[i + name.len()..].chars().next();
        let sep = |c: Option<char>| matches!(c, Some('/' | '\\'));
        let end = |c: Option<char>| {
            sep(c)
                || c.is_none_or(|c| c.is_whitespace() || matches!(c, '"' | '\'' | ')' | ':' | ','))
        };
        out.push_str(&rest[..i]);
        if sep(before) && end(after) {
            out.push_str(with);
        } else {
            out.push_str(name);
        }
        rest = &rest[i + name.len()..];
    }
    out.push_str(rest);
    out
}

/// A settings file (TOML) with the values of sensitive keys removed, then redacted.
pub fn redact_settings(toml: &str, r: &Redactor) -> String {
    let mut out = String::new();
    for line in toml.lines() {
        let l = line.trim_start();
        let redact = !l.starts_with('#')
            && !l.starts_with('[')
            && l.split_once('=').is_some_and(|(k, _)| {
                let k = k.trim().trim_matches('"').to_ascii_lowercase();
                SENSITIVE.iter().any(|s| k.contains(s))
            });
        if redact {
            let (k, _) = line.split_once('=').unwrap_or((line, ""));
            out.push_str(k.trim_end());
            out.push_str(" = \"<redacted>\"");
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    r.redact(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r() -> Redactor {
        Redactor::new(Some(Path::new("/home/ada")), Some("ada"))
    }

    #[test]
    fn home_and_user_always_go() {
        let r = r().hide_file_names(false);
        assert_eq!(
            r.redact("opened /home/ada/Pictures/cat.jpg"),
            "opened ~/Pictures/cat.jpg"
        );
        assert_eq!(
            r.redact("/mnt/ada/x and C:\\Users\\ada\\y"),
            "/mnt/<user>/x and C:\\Users\\<user>\\y"
        );
        // Not a path component: kept (the name may be a word).
        assert_eq!(r.redact("adaptive ada"), "adaptive ada");
        let w = Redactor::new(Some(Path::new("C:\\Users\\Ada")), None).hide_file_names(false);
        assert_eq!(
            w.redact("C:\\Users\\Ada\\x.png and C:/Users/Ada/y.png"),
            "~\\x.png and ~/y.png"
        );
    }

    #[test]
    fn file_names_are_hidden_consistently() {
        let r = r();
        let a = r.redact("decode /home/ada/Pictures/Holiday/cat.JPG failed; retry cat.JPG");
        assert_eq!(
            a,
            "decode ~/dir-1/dir-2/file-1.jpg failed; retry file-1.jpg"
        );
        // Same names, same numbers, across calls.
        assert_eq!(
            r.redact("again: ~/Pictures/dog.png"),
            "again: ~/dir-1/file-2.png"
        );
        // Known paths with spaces go whole.
        let r = Redactor::new(Some(Path::new("/home/ada")), Some("ada"))
            .known_paths(["/media/disk/My Scans/page 1.tif".to_owned()]);
        assert_eq!(
            r.redact("open \"/media/disk/My Scans/page 1.tif\""),
            "open \"/media/dir-1/dir-2/file-1.tif\""
        );
    }

    #[test]
    fn backtraces_versions_and_addresses_stay_readable() {
        let r = r();
        for keep in [
            "at crates/specular-decode/src/jpeg.rs:12:5",
            "at /home/ada/.cargo/registry/src/x/lib.rs:3",
            "specular_core::isolate::run",
            "version 0.4.1 on Vulkan 1.3.2",
            "see https://example.com/a/b.html",
            "specular.log",
        ] {
            let got = r.redact(keep);
            assert!(
                !got.contains("file-") && !got.contains("dir-"),
                "{keep} → {got}"
            );
        }
        assert_eq!(r.redact("at /home/ada/src/main.rs:5"), "at ~/src/main.rs:5");
        // A file name before a position keeps the position.
        assert_eq!(
            r.redact("bad marker in photo.jpg:3."),
            "bad marker in file-1.jpg:3."
        );
    }

    #[test]
    fn crash_report_redaction() {
        let report = "Specular 1.0 crashed\nmessage:  index out of range in /home/ada/Scans/x-ray 4.dcm\n\
                      backtrace:\n  0: specular_decode::dicom::read\n     at ./crates/specular-decode/src/dicom.rs:88:9\n\
                      last 2 log lines:\n  12.0 main io opened /home/ada/Scans/a.dcm\n  13.0 main ui ok\n";
        let r = r().known_paths(["/home/ada/Scans/x-ray 4.dcm".to_owned()]);
        let out = r.redact(report);
        assert!(!out.contains("ada"), "{out}");
        assert!(!out.contains("Scans") && !out.contains("x-ray"), "{out}");
        assert!(out.contains("specular_decode::dicom::read"));
        assert!(out.contains("dicom.rs:88:9"));
        assert!(
            out.contains("~/dir-1/file-1.dcm") && out.contains("~/dir-1/file-2.dcm"),
            "{out}"
        );
    }

    #[test]
    fn settings_lose_secrets() {
        let toml = "[remote]\nserver_url = \"https://pacs.example/\"\npassword = \"hunter2\"\n\
                    [viewing]\nzoom = \"fit\"\n# user = comment kept\nlast_folder = \"/home/ada/Pictures\"\n";
        let out = redact_settings(toml, &r());
        assert!(out.contains("server_url = \"<redacted>\""));
        assert!(out.contains("password = \"<redacted>\""));
        assert!(out.contains("last_folder = \"<redacted>\""));
        assert!(out.contains("zoom = \"fit\""));
        assert!(!out.contains("hunter2") && !out.contains("pacs.example"));
    }
}
