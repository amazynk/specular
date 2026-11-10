//! "Open with…" on Linux and the BSDs: the freedesktop.org specifications, in pure Rust.
//!
//! * Desktop Entry Specification 1.5: `applications/*.desktop` under `$XDG_DATA_HOME` and
//!   `$XDG_DATA_DIRS`, desktop file IDs, localised `Name`, `Exec` quoting and field codes,
//!   `Hidden`, `NoDisplay`, `TryExec`, `OnlyShowIn`/`NotShowIn`.
//! * Association between MIME types and applications 1.0.1: `mimeapps.list` (and the
//!   desktop-specific `$desktop-mimeapps.list`) in `$XDG_CONFIG_HOME`, `$XDG_CONFIG_DIRS` and
//!   the data directories; default, added and removed associations; legacy `defaults.list`.
//! * Shared MIME-info database: `mime/globs2`, `mime/aliases` and `mime/subclasses` to name the
//!   file's type from its extension (with a built-in table when the database is missing).
//!
//! Everything here reads plain files: no GIO, no D-Bus. A scan of ~500 desktop files takes a
//! few milliseconds and runs on a background thread.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// One parsed `[Desktop Entry]` of `Type=Application`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DesktopEntry {
    /// Desktop file ID (`org.gnome.eog.desktop`, `kde4-gwenview.desktop`).
    pub id: String,
    /// Localised name.
    pub name: String,
    pub exec: String,
    pub mime: Vec<String>,
    pub icon: Option<String>,
    pub no_display: bool,
    pub hidden: bool,
    pub terminal: bool,
    pub try_exec: Option<String>,
    /// Working directory (`Path=`).
    pub path: Option<String>,
    pub only_show_in: Vec<String>,
    pub not_show_in: Vec<String>,
    /// The `.desktop` file itself (for `%k`).
    pub file: PathBuf,
}

/// Unescapes a desktop-file string value (`\s`, `\n`, `\t`, `\r`, `\\`).
fn unescape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut it = v.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(o) => {
                out.push('\\');
                out.push(o);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Splits a `;`-separated list (`\;` is a literal semicolon).
fn list(v: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut it = v.chars();
    while let Some(c) = it.next() {
        match c {
            '\\' => match it.next() {
                Some(';') => cur.push(';'),
                Some(o) => {
                    cur.push('\\');
                    cur.push(o);
                }
                None => cur.push('\\'),
            },
            ';' => {
                let s = unescape(cur.trim());
                if !s.is_empty() {
                    out.push(s);
                }
                cur.clear();
            }
            c => cur.push(c),
        }
    }
    let s = unescape(cur.trim());
    if !s.is_empty() {
        out.push(s);
    }
    out
}

fn bool_value(v: &str) -> bool {
    v.trim() == "true"
}

/// Locale names to try for localised keys, best first, for the UI language (`de`, `zh-Hans`,
/// `pt-BR`) and the POSIX locale (`LC_ALL`, `LC_MESSAGES`, `LANG`, e.g. `de_AT.UTF-8@euro`).
pub(crate) fn locale_candidates(ui_lang: &str, posix: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |s: String| {
        if !s.is_empty() && !out.contains(&s) {
            out.push(s);
        }
    };
    let ui = ui_lang.trim();
    let ui_base = ui
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    // A POSIX locale of the same language is more specific (de_AT), so it goes first.
    if let Some(p) = posix.filter(|p| !p.is_empty() && *p != "C" && *p != "POSIX") {
        let (main, modifier) = match p.split_once('@') {
            Some((m, md)) => (m, Some(md)),
            None => (p, None),
        };
        let main = main.split('.').next().unwrap_or(main);
        let (lang, country) = match main.split_once('_') {
            Some((l, c)) => (l, Some(c)),
            None => (main, None),
        };
        if ui_base.is_empty() || lang.eq_ignore_ascii_case(&ui_base) {
            if let (Some(c), Some(m)) = (country, modifier) {
                add(format!("{lang}_{c}@{m}"));
            }
            if let Some(c) = country {
                add(format!("{lang}_{c}"));
            }
            if let Some(m) = modifier {
                add(format!("{lang}@{m}"));
            }
            add(lang.to_owned());
        }
    }
    match ui {
        "zh-Hans" | "zh-CN" | "zh_CN" => add("zh_CN".into()),
        "zh-Hant" | "zh-TW" | "zh_TW" => add("zh_TW".into()),
        // Bare `pt` is the Brazilian catalogue; desktop files use `pt` for European Portuguese.
        "pt" => add("pt_BR".into()),
        _ => {
            if let Some((l, c)) = ui.split_once(['-', '_']).filter(|(_, c)| c.len() == 2) {
                add(format!(
                    "{}_{}",
                    l.to_ascii_lowercase(),
                    c.to_ascii_uppercase()
                ));
            }
        }
    }
    add(ui_base);
    out
}

/// Parses one desktop file. `None` unless it is a valid `Type=Application` entry with `Exec`.
pub(crate) fn parse_entry(
    text: &str,
    id: &str,
    file: &Path,
    langs: &[String],
) -> Option<DesktopEntry> {
    let mut in_main = false;
    let mut seen_main = false;
    let mut kind = None;
    let mut name: Option<String> = None;
    let mut name_rank = usize::MAX;
    let mut e = DesktopEntry {
        id: id.to_owned(),
        name: String::new(),
        exec: String::new(),
        mime: Vec::new(),
        icon: None,
        no_display: false,
        hidden: false,
        terminal: false,
        try_exec: None,
        path: None,
        only_show_in: Vec::new(),
        not_show_in: Vec::new(),
        file: file.to_path_buf(),
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            if seen_main && in_main {
                break; // later groups (actions) do not matter
            }
            in_main = line == "[Desktop Entry]";
            seen_main |= in_main;
            continue;
        }
        if !in_main {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim());
        if let Some(loc) = k.strip_prefix("Name[").and_then(|r| r.strip_suffix(']')) {
            if let Some(rank) = langs.iter().position(|l| l == loc)
                && rank < name_rank
            {
                name_rank = rank;
                name = Some(unescape(v));
            }
            continue;
        }
        match k {
            "Type" => kind = Some(v.to_owned()),
            "Name" => e.name = unescape(v),
            "Exec" => e.exec = unescape(v),
            "TryExec" => e.try_exec = Some(unescape(v)),
            "Path" => e.path = Some(unescape(v)).filter(|p| !p.is_empty()),
            "Icon" => e.icon = Some(unescape(v)).filter(|p| !p.is_empty()),
            "MimeType" => e.mime = list(v),
            "NoDisplay" => e.no_display = bool_value(v),
            "Hidden" => e.hidden = bool_value(v),
            "Terminal" => e.terminal = bool_value(v),
            "OnlyShowIn" => e.only_show_in = list(v),
            "NotShowIn" => e.not_show_in = list(v),
            _ => {}
        }
    }
    if let Some(n) = name {
        e.name = n;
    }
    (seen_main
        && kind.as_deref() == Some("Application")
        && !e.exec.is_empty()
        && !e.name.is_empty())
    .then_some(e)
}

/// Splits an (already unescaped) `Exec` value into arguments, following the spec's quoting:
/// double quotes group, and inside them `\"`, `` \` ``, `\$` and `\\` are escapes. `None` for
/// an unterminated quote.
pub(crate) fn split_exec(exec: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut has = false;
    let mut it = exec.chars();
    while let Some(c) = it.next() {
        match c {
            '"' => {
                has = true;
                loop {
                    match it.next()? {
                        '"' => break,
                        '\\' => match it.next()? {
                            e @ ('"' | '`' | '$' | '\\') => cur.push(e),
                            o => {
                                cur.push('\\');
                                cur.push(o);
                            }
                        },
                        o => cur.push(o),
                    }
                }
            }
            ' ' | '\t' | '\n' => {
                if has {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            c => {
                cur.push(c);
                has = true;
            }
        }
    }
    if has {
        out.push(cur);
    }
    Some(out)
}

/// Builds the command line for opening `file` with `e`: field codes replaced (`%f %F` → the
/// path, `%u %U` → its URI, `%i` → `--icon NAME`, `%c` → the name, `%k` → the desktop file,
/// `%%` → `%`; deprecated and unknown codes dropped). When `Exec` has no file code the path is
/// appended, as KDE does: the user explicitly asked to open this file with that program.
pub(crate) fn expand_exec(e: &DesktopEntry, file: &Path) -> Option<Vec<OsString>> {
    let args = split_exec(&e.exec)?;
    let uri = crate::launch::file_uri(file);
    let mut out: Vec<OsString> = Vec::with_capacity(args.len() + 1);
    let mut used_file = false;
    for a in &args {
        match a.as_str() {
            "%f" | "%F" => {
                out.push(file.as_os_str().to_owned());
                used_file = true;
                continue;
            }
            "%u" | "%U" => {
                out.push(uri.clone().into());
                used_file = true;
                continue;
            }
            "%i" => {
                if let Some(i) = &e.icon {
                    out.push("--icon".into());
                    out.push(i.into());
                }
                continue;
            }
            "%d" | "%D" | "%n" | "%N" | "%v" | "%m" => continue,
            _ => {}
        }
        let mut s = OsString::new();
        let mut it = a.chars();
        while let Some(c) = it.next() {
            if c != '%' {
                let mut b = [0; 4];
                s.push(&*c.encode_utf8(&mut b));
                continue;
            }
            match it.next() {
                Some('%') => s.push("%"),
                Some('f' | 'F') => {
                    s.push(file.as_os_str());
                    used_file = true;
                }
                Some('u' | 'U') => {
                    s.push(&uri);
                    used_file = true;
                }
                Some('c') => s.push(&e.name),
                Some('k') => s.push(e.file.as_os_str()),
                Some('i') => {}
                _ => {}
            }
        }
        out.push(s);
    }
    if out.is_empty() {
        return None;
    }
    if !used_file {
        out.push(file.as_os_str().to_owned());
    }
    Some(out)
}

/// `$XDG_DATA_HOME` then `$XDG_DATA_DIRS`, most important first.
pub(crate) fn data_dirs() -> Vec<PathBuf> {
    xdg_dirs(
        "XDG_DATA_HOME",
        ".local/share",
        "XDG_DATA_DIRS",
        "/usr/local/share:/usr/share",
    )
}

/// `$XDG_CONFIG_HOME` then `$XDG_CONFIG_DIRS`, most important first.
pub(crate) fn config_dirs() -> Vec<PathBuf> {
    xdg_dirs("XDG_CONFIG_HOME", ".config", "XDG_CONFIG_DIRS", "/etc/xdg")
}

fn xdg_dirs(
    home_var: &str,
    home_default: &str,
    dirs_var: &str,
    dirs_default: &str,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let abs = |p: PathBuf| p.is_absolute().then_some(p);
    if let Some(h) = std::env::var_os(home_var).map(PathBuf::from).and_then(abs) {
        out.push(h);
    } else if let Some(h) = std::env::var_os("HOME").filter(|h| !h.is_empty()) {
        out.push(PathBuf::from(h).join(home_default));
    }
    let dirs = std::env::var_os(dirs_var)
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| dirs_default.into());
    for d in std::env::split_paths(&dirs).filter_map(abs) {
        if !out.contains(&d) {
            out.push(d);
        }
    }
    out
}

/// `$XDG_CURRENT_DESKTOP` as a list (`KDE`, `GNOME`, `ubuntu:GNOME`).
pub(crate) fn current_desktops() -> Vec<String> {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|v| {
            v.split(':')
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Every application in `data_dirs/applications`, by desktop file ID. A file in a more
/// important directory hides one with the same ID further down the list.
pub(crate) fn scan_apps(data_dirs: &[PathBuf], langs: &[String]) -> HashMap<String, DesktopEntry> {
    let mut apps = HashMap::new();
    for d in data_dirs {
        let root = d.join("applications");
        let mut stack = vec![(root.clone(), 0u32)];
        while let Some((dir, depth)) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else {
                continue;
            };
            for ent in rd.flatten() {
                let p = ent.path();
                let Ok(ft) = std::fs::metadata(&p).map(|m| m.file_type()) else {
                    continue;
                };
                if ft.is_dir() {
                    if depth < 4 {
                        stack.push((p, depth + 1));
                    }
                    continue;
                }
                if p.extension().is_none_or(|e| e != "desktop") {
                    continue;
                }
                let Some(id) = desktop_id(&root, &p) else {
                    continue;
                };
                if apps.contains_key(&id) {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&p) else {
                    continue;
                };
                match parse_entry(&text, &id, &p, langs) {
                    Some(e) => {
                        apps.insert(id, e);
                    }
                    None => {
                        // An invalid or non-application file still hides lower ones (that
                        // is how users delete system entries).
                        apps.insert(
                            id.clone(),
                            DesktopEntry {
                                id,
                                name: String::new(),
                                exec: String::new(),
                                mime: Vec::new(),
                                icon: None,
                                no_display: true,
                                hidden: true,
                                terminal: false,
                                try_exec: None,
                                path: None,
                                only_show_in: Vec::new(),
                                not_show_in: Vec::new(),
                                file: p,
                            },
                        );
                    }
                }
            }
        }
    }
    apps
}

/// Desktop file ID: the path below `applications/` with `/` replaced by `-`.
fn desktop_id(root: &Path, file: &Path) -> Option<String> {
    let rel = file.strip_prefix(root).ok()?;
    let parts: Option<Vec<&str>> = rel.components().map(|c| c.as_os_str().to_str()).collect();
    Some(parts?.join("-"))
}

/// The three groups of one `mimeapps.list` (or `defaults.list`).
#[derive(Debug, Default)]
pub(crate) struct MimeApps {
    pub default: HashMap<String, Vec<String>>,
    pub added: HashMap<String, Vec<String>>,
    pub removed: HashMap<String, Vec<String>>,
}

pub(crate) fn parse_mimeapps(text: &str) -> MimeApps {
    let mut m = MimeApps::default();
    let mut group: Option<&mut HashMap<String, Vec<String>>> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            group = match line {
                "[Default Applications]" => Some(&mut m.default),
                "[Added Associations]" => Some(&mut m.added),
                "[Removed Associations]" => Some(&mut m.removed),
                _ => None,
            };
            continue;
        }
        if let (Some(g), Some((k, v))) = (group.as_deref_mut(), line.split_once('=')) {
            g.entry(k.trim().to_owned()).or_default().extend(list(v));
        }
    }
    m
}

/// `mimeapps.list` files in the spec's precedence order, read and parsed (missing ones
/// skipped).
pub(crate) fn load_mimeapps(
    config_dirs: &[PathBuf],
    data_dirs: &[PathBuf],
    desktops: &[String],
) -> Vec<MimeApps> {
    let mut files = Vec::new();
    let mut push_dir = |dir: PathBuf, legacy: bool| {
        for d in desktops {
            files.push(dir.join(format!("{}-mimeapps.list", d.to_ascii_lowercase())));
        }
        files.push(dir.join("mimeapps.list"));
        if legacy {
            files.push(dir.join("defaults.list"));
        }
    };
    for c in config_dirs {
        push_dir(c.clone(), false);
    }
    for d in data_dirs {
        push_dir(d.join("applications"), true);
    }
    files
        .iter()
        .filter_map(|f| std::fs::read_to_string(f).ok())
        .map(|t| parse_mimeapps(&t))
        .collect()
}

/// The shared MIME-info database (just what extension lookup needs).
#[derive(Debug, Default)]
pub(crate) struct MimeDb {
    /// Lower-case extension → (weight, MIME type), best first.
    ext: HashMap<String, Vec<(u32, String)>>,
    aliases: HashMap<String, String>,
    parents: HashMap<String, Vec<String>>,
}

impl MimeDb {
    pub fn load(data_dirs: &[PathBuf]) -> MimeDb {
        let mut db = MimeDb::default();
        // Least important first, so more important directories win.
        for d in data_dirs.iter().rev() {
            if let Ok(t) = std::fs::read_to_string(d.join("mime/globs2")) {
                db.add_globs2(&t);
            }
            if let Ok(t) = std::fs::read_to_string(d.join("mime/aliases")) {
                for (a, c) in t.lines().filter_map(|l| l.split_once(' ')) {
                    db.aliases.insert(a.trim().to_owned(), c.trim().to_owned());
                }
            }
            if let Ok(t) = std::fs::read_to_string(d.join("mime/subclasses")) {
                for (c, p) in t.lines().filter_map(|l| l.split_once(' ')) {
                    let v = db.parents.entry(c.trim().to_owned()).or_default();
                    if !v.iter().any(|x| x == p.trim()) {
                        v.push(p.trim().to_owned());
                    }
                }
            }
        }
        for v in db.ext.values_mut() {
            v.sort_by_key(|b| std::cmp::Reverse(b.0));
        }
        db
    }

    pub fn add_globs2(&mut self, text: &str) {
        for line in text.lines() {
            if line.starts_with('#') {
                continue;
            }
            let mut f = line.split(':');
            let (Some(w), Some(mime), Some(glob)) = (f.next(), f.next(), f.next()) else {
                continue;
            };
            let Ok(w) = w.parse() else { continue };
            // Only the simple "*.ext" form; enough for images.
            let Some(ext) = glob.strip_prefix("*.") else {
                continue;
            };
            if ext.contains(['*', '?', '[']) {
                continue;
            }
            let v = self.ext.entry(ext.to_ascii_lowercase()).or_default();
            if !v.iter().any(|(_, m)| m == mime) {
                v.push((w, mime.to_owned()));
            }
        }
    }

    pub fn canonical<'a>(&'a self, mime: &'a str) -> &'a str {
        self.aliases.get(mime).map_or(mime, String::as_str)
    }

    /// The file's MIME type followed by its ancestors (`image/x-canon-cr2`, `image/tiff`, …),
    /// from the extension. Empty when unknown.
    pub fn types_for(&self, path: &Path) -> Vec<String> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        let first = self
            .ext
            .get(&ext)
            .and_then(|v| v.first())
            .map(|(_, m)| m.clone())
            .or_else(|| builtin_type(&ext).map(str::to_owned));
        let mut out = Vec::new();
        let mut queue: Vec<String> = first.into_iter().collect();
        while let Some(m) = queue.pop() {
            let m = self.canonical(&m).to_owned();
            if out.contains(&m) || out.len() > 8 {
                continue;
            }
            if let Some(ps) = self.parents.get(&m) {
                queue.extend(ps.iter().rev().cloned());
            }
            out.push(m);
        }
        // Programs that take any image list `image/*` (rare, but allowed).
        if let Some(top) = out.first().and_then(|m| m.split_once('/')).map(|(t, _)| t)
            && top == "image"
        {
            out.push("image/*".into());
        }
        out
    }
}

/// Image types by extension when the MIME database is missing.
fn builtin_type(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "jpg" | "jpeg" | "jpe" | "jfif" => "image/jpeg",
        "png" => "image/png",
        "apng" => "image/apng",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "heic" | "heif" => "image/heif",
        "jxl" => "image/jxl",
        "tif" | "tiff" => "image/tiff",
        "bmp" => "image/bmp",
        "ico" => "image/vnd.microsoft.icon",
        "svg" => "image/svg+xml",
        "exr" => "image/x-exr",
        "hdr" => "image/vnd.radiance",
        "psd" => "image/vnd.adobe.photoshop",
        "jp2" | "j2k" => "image/jp2",
        "qoi" => "image/qoi",
        "tga" => "image/x-tga",
        "dng" => "image/x-adobe-dng",
        _ => return None,
    })
}

/// Rules that hide an application from "Open with…".
fn usable(e: &DesktopEntry, desktops: &[String], explicit: bool) -> bool {
    if e.hidden || e.terminal || e.exec.is_empty() {
        return false;
    }
    if e.no_display && !explicit {
        return false;
    }
    let on = |l: &[String]| {
        l.iter()
            .any(|x| desktops.iter().any(|d| d.eq_ignore_ascii_case(x)))
    };
    if !e.only_show_in.is_empty() && !on(&e.only_show_in) {
        return false;
    }
    if on(&e.not_show_in) {
        return false;
    }
    if let Some(t) = &e.try_exec
        && crate::launch::find_program(t).is_none()
    {
        return false;
    }
    !is_specular(e)
}

fn is_specular(e: &DesktopEntry) -> bool {
    if e.id.to_ascii_lowercase().starts_with("app.specular.") {
        return true;
    }
    split_exec(&e.exec)
        .and_then(|a| a.into_iter().next())
        .is_some_and(|p| Path::new(&p).file_name().is_some_and(|n| n == "specular"))
}

/// Applications for a file of `types` (most specific first), in menu order: the default, then
/// associations users added, then every other installed application that declares the type,
/// by name. Returns `(entry, is_default)`.
pub(crate) fn resolve<'a>(
    types: &[String],
    apps: &'a HashMap<String, DesktopEntry>,
    lists: &[MimeApps],
    db: &MimeDb,
    desktops: &[String],
) -> Vec<(&'a DesktopEntry, bool)> {
    let mut out: Vec<(&DesktopEntry, bool)> = Vec::new();
    let mut seen = HashSet::new();
    let mut removed: HashSet<&str> = HashSet::new();
    let mut default: Option<&DesktopEntry> = None;
    let mut added: Vec<&DesktopEntry> = Vec::new();
    for t in types {
        for l in lists {
            if default.is_none()
                && let Some(ids) = l.default.get(t)
            {
                default = ids
                    .iter()
                    .filter(|id| !removed.contains(id.as_str()))
                    .filter_map(|id| apps.get(id))
                    .find(|e| usable(e, desktops, true));
            }
            for id in l.added.get(t).into_iter().flatten() {
                if !removed.contains(id.as_str())
                    && let Some(e) = apps.get(id)
                    && usable(e, desktops, true)
                {
                    added.push(e);
                }
            }
            for id in l.removed.get(t).into_iter().flatten() {
                removed.insert(id);
            }
        }
    }
    if let Some(d) = default {
        seen.insert(d.id.as_str());
        out.push((d, true));
    }
    for e in added {
        if seen.insert(e.id.as_str()) {
            out.push((e, false));
        }
    }
    let mut declared: Vec<&DesktopEntry> = apps
        .values()
        .filter(|e| !seen.contains(e.id.as_str()) && !removed.contains(e.id.as_str()))
        .filter(|e| {
            e.mime
                .iter()
                .any(|m| types.iter().any(|t| t == db.canonical(m)))
        })
        .filter(|e| usable(e, desktops, false))
        .collect();
    declared.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.id.cmp(&b.id))
    });
    out.extend(declared.into_iter().map(|e| (e, false)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/test-tmp/open-with"
        ))
        .join(name);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(p: &Path, text: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    #[test]
    fn entries_parse_with_localised_names() {
        let text = "\
# comment
[Desktop Entry]
Type=Application
Name=Image Viewer
Name[de]=Bildbetrachter
Name[de_AT]=Bildanzeiger
Name[fr]=Visionneuse\\sd'images
Exec=eog %U
MimeType=image/jpeg;image/png;image/x\\;odd;
NoDisplay=false
Icon=eog
OnlyShowIn=GNOME;

[Desktop Action new-window]
Name=Ignored
Exec=ignored
";
        let f = Path::new("/x/eog.desktop");
        let de = parse_entry(
            text,
            "eog.desktop",
            f,
            &locale_candidates("de", Some("de_AT.UTF-8")),
        )
        .unwrap();
        assert_eq!(de.name, "Bildanzeiger");
        let de = parse_entry(
            text,
            "eog.desktop",
            f,
            &locale_candidates("de", Some("en_US.UTF-8")),
        )
        .unwrap();
        assert_eq!(de.name, "Bildbetrachter");
        let fr = parse_entry(text, "eog.desktop", f, &locale_candidates("fr", None)).unwrap();
        assert_eq!(fr.name, "Visionneuse d'images");
        let en = parse_entry(text, "eog.desktop", f, &locale_candidates("ja", None)).unwrap();
        assert_eq!(en.name, "Image Viewer");
        assert_eq!(en.exec, "eog %U");
        assert_eq!(en.mime, ["image/jpeg", "image/png", "image/x;odd"]);
        assert_eq!(en.only_show_in, ["GNOME"]);
        assert_eq!(en.icon.as_deref(), Some("eog"));
        // Not applications, or broken.
        assert!(parse_entry("[Desktop Entry]\nType=Link\nName=x\nURL=y", "l", f, &[]).is_none());
        assert!(parse_entry("[Desktop Entry]\nType=Application\nName=x", "l", f, &[]).is_none());
        assert!(parse_entry("[Other]\nType=Application\nName=x\nExec=y", "l", f, &[]).is_none());
    }

    #[test]
    fn locales() {
        assert_eq!(
            locale_candidates("de", Some("de_AT.UTF-8@euro")),
            ["de_AT@euro", "de_AT", "de@euro", "de"]
        );
        assert_eq!(locale_candidates("zh-Hans", Some("C")), ["zh_CN", "zh"]);
        assert_eq!(
            locale_candidates("pt-BR", Some("en_GB.UTF-8")),
            ["pt_BR", "pt"]
        );
        assert_eq!(locale_candidates("pt", None), ["pt_BR", "pt"]);
        assert_eq!(
            locale_candidates("pt-PT", Some("pt_PT.UTF-8")),
            ["pt_PT", "pt"]
        );
        assert_eq!(
            locale_candidates("sv", Some("sv_FI.UTF-8")),
            ["sv_FI", "sv"]
        );
        assert_eq!(locale_candidates("he", None), ["he"]);
    }

    #[test]
    fn exec_quoting_and_field_codes() {
        assert_eq!(
            split_exec(r#"foo "a b" c\ d "x\"y\$z\\w" "" e"#).unwrap(),
            ["foo", "a b", r"c\", "d", r#"x"y$z\w"#, "", "e"]
        );
        assert!(split_exec(r#"foo "open"#).is_none());

        let e = DesktopEntry {
            id: "v.desktop".into(),
            name: "Viewer".into(),
            exec: "viewer --name=%c %i --title=%%x %F %d".into(),
            mime: vec![],
            icon: Some("vic".into()),
            no_display: false,
            hidden: false,
            terminal: false,
            try_exec: None,
            path: None,
            only_show_in: vec![],
            not_show_in: vec![],
            file: "/apps/v.desktop".into(),
        };
        let f = Path::new("/p/a b.jpg");
        let got = expand_exec(&e, f).unwrap();
        assert_eq!(
            got,
            [
                "viewer",
                "--name=Viewer",
                "--icon",
                "vic",
                "--title=%x",
                "/p/a b.jpg"
            ]
            .map(OsString::from)
        );
        let u = DesktopEntry {
            exec: "viewer %u".into(),
            ..e.clone()
        };
        assert_eq!(
            expand_exec(&u, f).unwrap(),
            ["viewer", "file:///p/a%20b.jpg"].map(OsString::from)
        );
        // No field code: the file is appended.
        let n = DesktopEntry {
            exec: "\"/opt/My App/run\" -x".into(),
            ..e
        };
        assert_eq!(
            expand_exec(&n, f).unwrap(),
            ["/opt/My App/run", "-x", "/p/a b.jpg"].map(OsString::from)
        );
    }

    #[test]
    fn associations_follow_the_spec() {
        let root = tmp("assoc");
        let home = root.join("home");
        let sys = root.join("sys");
        let cfg = root.join("cfg");
        let app = |dir: &Path, rel: &str, name: &str, extra: &str| {
            write(
                &dir.join("applications").join(rel),
                &format!(
                    "[Desktop Entry]\nType=Application\nName={name}\nExec={name} %f\n{extra}\n"
                ),
            );
        };
        app(
            &sys,
            "gimp.desktop",
            "gimp",
            "MimeType=image/jpeg;image/png;",
        );
        app(
            &sys,
            "kde/gwenview.desktop",
            "gwenview",
            "MimeType=image/jpeg;",
        );
        app(&sys, "eog.desktop", "eog", "MimeType=image/jpg;"); // an alias
        app(&sys, "hidden.desktop", "hiddenapp", "MimeType=image/jpeg;");
        app(&home, "hidden.desktop", "hiddenapp", "Hidden=true"); // user deleted it
        app(
            &sys,
            "nodisp.desktop",
            "nodisp",
            "MimeType=image/jpeg;\nNoDisplay=true",
        );
        app(&sys, "added.desktop", "addedapp", "NoDisplay=true");
        app(
            &sys,
            "term.desktop",
            "termapp",
            "MimeType=image/jpeg;\nTerminal=true",
        );
        app(
            &sys,
            "gnomeonly.desktop",
            "gnomeapp",
            "MimeType=image/jpeg;\nOnlyShowIn=GNOME;",
        );
        app(
            &sys,
            "tryexec.desktop",
            "missing",
            "MimeType=image/jpeg;\nTryExec=surely-not-installed-xyz",
        );
        app(
            &sys,
            "removed.desktop",
            "removedapp",
            "MimeType=image/jpeg;",
        );
        app(
            &sys,
            "app.specular.Specular.desktop",
            "specular",
            "MimeType=image/jpeg;",
        );
        write(
            &sys.join("mime/globs2"),
            "50:image/jpeg:*.jpg\n50:image/jpeg:*.jpeg\n50:image/png:*.png\n",
        );
        write(&sys.join("mime/aliases"), "image/jpg image/jpeg\n");
        write(
            &cfg.join("kde-mimeapps.list"),
            "[Default Applications]\nimage/jpeg=notinstalled.desktop;kde-gwenview.desktop;\n",
        );
        write(
            &cfg.join("mimeapps.list"),
            "[Default Applications]\nimage/jpeg=gimp.desktop\n\
             [Added Associations]\nimage/jpeg=added.desktop;\n\
             [Removed Associations]\nimage/jpeg=removed.desktop;\n",
        );

        let data = [home, sys];
        let apps = scan_apps(&data, &[]);
        assert!(apps.contains_key("kde-gwenview.desktop"));
        let desktops = vec!["KDE".to_string()];
        let lists = load_mimeapps(std::slice::from_ref(&cfg), &data, &desktops);
        let db = MimeDb::load(&data);
        let types = db.types_for(Path::new("/x/photo.JPG"));
        assert_eq!(types, ["image/jpeg", "image/*"]);
        let got: Vec<(&str, bool)> = resolve(&types, &apps, &lists, &db, &desktops)
            .into_iter()
            .map(|(e, d)| (e.name.as_str(), d))
            .collect();
        assert_eq!(
            got,
            [
                ("gwenview", true),
                ("addedapp", false),
                ("eog", false),
                ("gimp", false)
            ]
        );
        // Without the KDE-specific list, mimeapps.list's default applies.
        let lists = load_mimeapps(std::slice::from_ref(&cfg), &data, &[]);
        let got = resolve(&types, &apps, &lists, &db, &[]);
        assert_eq!((got[0].0.name.as_str(), got[0].1), ("gimp", true));
        assert!(!got.iter().any(|(e, _)| e.name == "gnomeapp"));
    }

    #[test]
    fn builtin_types_without_a_database() {
        let db = MimeDb::default();
        assert_eq!(db.types_for(Path::new("a.webp")), ["image/webp", "image/*"]);
        assert!(db.types_for(Path::new("a.unknownext")).is_empty());
        let mut db = MimeDb::default();
        db.add_globs2("50:image/x-canon-cr2:*.cr2\n");
        db.parents
            .insert("image/x-canon-cr2".into(), vec!["image/tiff".into()]);
        assert_eq!(
            db.types_for(Path::new("a.CR2")),
            ["image/x-canon-cr2", "image/tiff", "image/*"]
        );
    }
}
