//! Linux and BSD desktop integration (freedesktop.org specifications).
//!
//! | Option | What is written (under `$XDG_DATA_HOME` = `~/.local/share` unless noted) |
//! |---|---|
//! | Default viewer | `applications/app.specular.Specular.desktop` (not for packaged builds), `mime/packages/app.specular.Specular.xml`, and the image types in `$XDG_CONFIG_HOME/mimeapps.list` via `xdg-mime default` |
//! | Thumbnailer | `thumbnailers/specular.thumbnailer` for types no installed thumbnailer handles |
//! | Single instance | `dbus-1/services/app.specular.Specular.service` and `DBusActivatable=true` in the `.desktop` file |
//! | File-manager actions | Nautilus scripts, Dolphin service menus, Thunar `uca.xml` entries, Nemo actions |
//! | Search provider | GNOME Shell search provider, KRunner D-Bus plugin, and their D-Bus services |
//! | `specular://` links | `x-scheme-handler/specular` in the `.desktop` file's `MimeType=` and in `$XDG_CONFIG_HOME/mimeapps.list` |
//! | Recent files | our `<bookmark:application>` entries in `recently-used.xbel` (removal takes them out again) |
//!
//! The `.desktop` file (whenever one of ours is installed) carries the launcher actions
//! [`DESKTOP_ACTIONS`] (New Window, Library, Slideshow of Pictures) and a `MimeType=` list
//! generated from the decoder registry ([`crate::integration::mime`]).
//!
//! Every file we write carries the [`MARKER`]; removal deletes only files that still carry it
//! and are listed in the option's record (`$XDG_STATE_HOME/specular/integration/<option>.record`).
//! Shared configuration files (`mimeapps.list`, Thunar's `uca.xml`) are edited line by line,
//! backed up first (`…/integration/backups/`), and written atomically.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use specular_log::{SETTINGS, debug, info, warn};

use super::{Env, Error, Labels, OptionId, Result, State, Status, Unavailable, io_err};

pub mod dbus;
mod default_viewer;
mod file_manager;
pub mod formats;
pub mod keyfile;
pub mod recent;
pub mod search;
mod thumbnailer;
pub mod uca;
mod url_scheme;
pub mod wallpaper;

/// The application id: desktop file name, D-Bus name and icon name.
pub const APP_ID: &str = "app.specular.Specular";
/// Text in every file we write; a file without it is not ours and is never deleted.
pub const MARKER: &str = "X-Specular-Integration";
/// Backups kept per configuration file.
const KEEP_BACKUPS: usize = 10;

pub(super) fn status(env: &Env, id: OptionId) -> Status {
    let (state, targets) = match unavailable(env, id) {
        Some(u) => (State::Unavailable(u), Vec::new()),
        None => match id {
            OptionId::DefaultViewer => default_viewer::status(env),
            OptionId::Thumbnailer => thumbnailer::status(env),
            OptionId::DbusActivation => dbus::status(env),
            OptionId::FileManagerActions => file_manager::status(env),
            OptionId::SearchProvider => search::status(env),
            OptionId::UrlScheme => url_scheme::status(env),
            OptionId::RecentFiles => recent::status(env),
        },
    };
    Status { id, state, targets }
}

pub(super) fn apply(env: &Env, id: OptionId, labels: &Labels) -> Result<Status> {
    if let Some(u) = unavailable(env, id) {
        return Err(Error::Unavailable(u));
    }
    info!(target: SETTINGS, "integration: applying {}", id.key());
    match id {
        OptionId::DefaultViewer => default_viewer::apply(env, labels)?,
        OptionId::Thumbnailer => thumbnailer::apply(env)?,
        OptionId::DbusActivation => dbus::apply(env, labels)?,
        OptionId::FileManagerActions => file_manager::apply(env, labels)?,
        OptionId::SearchProvider => search::apply(env)?,
        OptionId::UrlScheme => url_scheme::apply(env, labels)?,
        OptionId::RecentFiles => recent::apply(env)?,
    }
    Ok(status(env, id))
}

pub(super) fn remove(env: &Env, id: OptionId) -> Result<Status> {
    info!(target: SETTINGS, "integration: removing {}", id.key());
    match id {
        OptionId::DefaultViewer => default_viewer::remove(env)?,
        OptionId::Thumbnailer => thumbnailer::remove(env)?,
        OptionId::DbusActivation => dbus::remove(env)?,
        OptionId::FileManagerActions => file_manager::remove(env)?,
        OptionId::SearchProvider => search::remove(env)?,
        OptionId::UrlScheme => url_scheme::remove(env)?,
        OptionId::RecentFiles => recent::remove(env)?,
    }
    Ok(status(env, id))
}

fn unavailable(env: &Env, id: OptionId) -> Option<Unavailable> {
    if env.sandboxed {
        return Some(Unavailable::Sandboxed);
    }
    match id {
        OptionId::DbusActivation if !dbus::session_bus_exists() && env.run_tools => {
            Some(Unavailable::NoSessionBus)
        }
        OptionId::FileManagerActions if file_manager::present(env).is_empty() => {
            Some(Unavailable::NoFileManager)
        }
        OptionId::SearchProvider if search::front_ends(env).is_empty() => {
            Some(Unavailable::NoSearch)
        }
        _ => None,
    }
}

// ----- Records: what an option made -------------------------------------------------------

/// What one option wrote and changed, one tab-separated record per line:
/// `file <path>` (a file we created), `prev <mime> <value>` (a `mimeapps.list` value we
/// replaced; `-` when there was none).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Record {
    pub rows: Vec<Vec<String>>,
}

impl Record {
    fn path(env: &Env, name: &str) -> PathBuf {
        env.record_dir().join(format!("{name}.record"))
    }

    /// The record `name`, if the option was applied.
    pub fn load(env: &Env, name: &str) -> Option<Record> {
        let text = fs::read_to_string(Self::path(env, name)).ok()?;
        let rows = text
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| l.split('\t').map(str::to_owned).collect())
            .collect();
        Some(Record { rows })
    }

    pub fn save(&self, env: &Env, name: &str) -> Result<()> {
        let mut s = format!(
            "# {MARKER}: what Specular changed for this integration option. Removed with it.\n"
        );
        for r in &self.rows {
            s.push_str(&r.join("\t"));
            s.push('\n');
        }
        write_atomic(&Self::path(env, name), s.as_bytes(), 0o644)
    }

    pub fn delete(env: &Env, name: &str) {
        let _ = fs::remove_file(Self::path(env, name));
    }

    pub fn files(&self) -> impl Iterator<Item = &Path> {
        self.rows
            .iter()
            .filter(|r| r.len() == 2 && r[0] == "file")
            .map(|r| Path::new(&r[1]))
    }

    pub fn add_file(&mut self, p: &Path) {
        let s = p.to_string_lossy().into_owned();
        if !self.files().any(|f| f == p) && !s.contains(['\t', '\n']) {
            self.rows.push(vec!["file".into(), s]);
        }
    }
}

// ----- Files ------------------------------------------------------------------------------

/// Writes `bytes` to `path` through a temporary file in the same folder and a rename, so a
/// crash never leaves half a file. A symlinked file is written through its target (dotfile
/// managers link `mimeapps.list`).
pub(crate) fn write_atomic(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let path = if path.is_symlink() {
        fs::canonicalize(path).map_err(io_err(path))?
    } else {
        path.to_path_buf()
    };
    let dir = path.parent().unwrap_or(Path::new("/"));
    fs::create_dir_all(dir).map_err(io_err(dir))?;
    let name = path
        .file_name()
        .map_or_else(Default::default, |n| n.to_string_lossy());
    let tmp = dir.join(format!(".{name}.specular-{}", std::process::id()));
    let r = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.set_permissions(fs::Permissions::from_mode(mode))?;
        f.sync_all()?;
        fs::rename(&tmp, &path)
    })();
    if let Err(e) = r {
        let _ = fs::remove_file(&tmp);
        return Err(Error::Io(path, e));
    }
    Ok(())
}

/// Writes one of our files (it must contain [`MARKER`]) and notes it in `rec`. Refuses to
/// replace a file of the same name that is not ours.
pub(crate) fn install(rec: &mut Record, path: &Path, text: &str, mode: u32) -> Result<()> {
    debug_assert!(text.contains(MARKER), "{} lacks the marker", path.display());
    if let Ok(old) = fs::read(path) {
        if !contains(&old, MARKER) {
            return Err(Error::Unrecognized(
                path.to_path_buf(),
                "a file of that name exists and was not made by Specular".into(),
            ));
        }
        if old == text.as_bytes() {
            rec.add_file(path);
            return Ok(());
        }
    }
    write_atomic(path, text.as_bytes(), mode)?;
    debug!(target: SETTINGS, "integration: wrote {}", path.display());
    rec.add_file(path);
    Ok(())
}

fn contains(hay: &[u8], needle: &str) -> bool {
    hay.windows(needle.len()).any(|w| w == needle.as_bytes())
}

/// Whether `path` is one of our files.
pub(crate) fn is_ours(path: &Path) -> bool {
    fs::read(path).is_ok_and(|b| contains(&b, MARKER))
}

/// Deletes the files of `rec` that are still ours; others are left (and logged).
pub(crate) fn uninstall(rec: &Record) {
    for f in rec.files() {
        if !f.exists() {
            continue;
        }
        if is_ours(f) {
            match fs::remove_file(f) {
                Ok(()) => debug!(target: SETTINGS, "integration: removed {}", f.display()),
                Err(e) => {
                    warn!(target: SETTINGS, "integration: cannot remove {}: {e}", f.display())
                }
            }
        } else {
            warn!(target: SETTINGS, "integration: {} was changed by someone else; left in place", f.display());
        }
    }
}

/// Copies `path` (if it exists) to the backup folder before we change it; keeps the newest
/// [`KEEP_BACKUPS`] copies per file name. Returns the copy.
pub(crate) fn backup(env: &Env, path: &Path) -> Result<Option<PathBuf>> {
    let Ok(bytes) = fs::read(path) else {
        return Ok(None);
    };
    let dir = env.record_dir().join("backups");
    fs::create_dir_all(&dir).map_err(io_err(&dir))?;
    let name = path
        .file_name()
        .map_or_else(|| "file".into(), |n| n.to_string_lossy().into_owned());
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let mut dest = dir.join(format!("{name}.{ms}"));
    let mut n = 0;
    while dest.exists() {
        n += 1;
        dest = dir.join(format!("{name}.{ms}-{n}"));
    }
    write_atomic(&dest, &bytes, 0o600)?;
    info!(target: SETTINGS, "integration: backed up {} to {}", path.display(), dest.display());
    let prefix = format!("{name}.");
    let mut old: Vec<PathBuf> = fs::read_dir(&dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .and_then(|n| n.strip_prefix(&prefix))
                        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
                })
                .collect()
        })
        .unwrap_or_default();
    if old.len() > KEEP_BACKUPS {
        // Names sort by time (same-length millisecond stamps).
        old.sort();
        for p in &old[..old.len() - KEEP_BACKUPS] {
            let _ = fs::remove_file(p);
        }
    }
    Ok(Some(dest))
}

// ----- Quoting ----------------------------------------------------------------------------

/// `Exec=` quoting of the desktop entry specification: plain when safe, else double quotes
/// with `"`, `` ` ``, `$` and `\` escaped, and then the key-file escape of `\`.
pub(crate) fn desktop_quote(p: &Path) -> String {
    let s = p.to_string_lossy();
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-+,@:".contains(c))
    {
        return s.into_owned();
    }
    let mut q = String::from("\"");
    for c in s.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            q.push('\\');
        }
        q.push(c);
    }
    q.push('"');
    q.replace('\\', "\\\\")
}

/// A key-file string value: `\\` and line breaks escaped (`TryExec=` takes the plain path).
pub(crate) fn keyfile_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// POSIX shell quoting (scripts, D-Bus service and thumbnailer `Exec=` lines, which are split
/// like a shell does).
pub(crate) fn shell_quote(p: &Path) -> String {
    let s = p.to_string_lossy();
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "/._-+,@:".contains(c))
    {
        return s.into_owned();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

// ----- Helper programs --------------------------------------------------------------------

/// Runs a helper program with the Env's folders in its environment. `Ok(false)` when helper
/// programs are off or the program is missing.
pub(crate) fn run_tool(env: &Env, program: &str, args: &[&std::ffi::OsStr]) -> Result<bool> {
    if !env.run_tools {
        return Ok(false);
    }
    let Some(exe) = env.find(program) else {
        return Ok(false);
    };
    let join = |v: &[PathBuf]| std::env::join_paths(v).unwrap_or_default();
    let out = Command::new(&exe)
        .args(args)
        .env("XDG_DATA_HOME", &env.data_home)
        .env("XDG_CONFIG_HOME", &env.config_home)
        .env("XDG_STATE_HOME", &env.state_home)
        .env("XDG_DATA_DIRS", join(&env.data_dirs))
        .env("XDG_CONFIG_DIRS", join(&env.config_dirs))
        .env("XDG_CURRENT_DESKTOP", env.desktops.join(":"))
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| Error::Tool(program.into(), e.to_string()))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        return Err(Error::Tool(
            program.into(),
            format!("{}: {err}", out.status),
        ));
    }
    debug!(target: SETTINGS, "integration: ran {program} {args:?}");
    Ok(true)
}

/// Runs a helper whose failure only costs freshness (cache rebuilds).
pub(crate) fn refresh(env: &Env, program: &str, dir: &Path) {
    if dir.is_dir()
        && let Err(e) = run_tool(env, program, &[dir.as_os_str()])
    {
        warn!(target: SETTINGS, "integration: {e}");
    }
}

// ----- The .desktop file ------------------------------------------------------------------

/// `~/.local/share/applications/app.specular.Specular.desktop`.
pub(crate) fn local_desktop_file(env: &Env) -> PathBuf {
    env.data_home
        .join("applications")
        .join(format!("{APP_ID}.desktop"))
}

/// A system-wide `.desktop` file of ours that starts this very program (a distribution
/// package): then no local copy is needed for the default viewer.
pub(crate) fn packaged(env: &Env) -> bool {
    let me = env.exe.canonicalize().unwrap_or_else(|_| env.exe.clone());
    env.data_dirs.iter().any(|d| {
        let f = d.join("applications").join(format!("{APP_ID}.desktop"));
        let Ok(text) = fs::read_to_string(&f) else {
            return false;
        };
        let kf = keyfile::KeyFile::parse(&text);
        let Some(exec) = kf.get("Desktop Entry", "Exec") else {
            return false;
        };
        let prog = exec
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_matches('"');
        let prog = if prog.starts_with('/') {
            Some(PathBuf::from(prog))
        } else {
            env.find(prog)
        };
        prog.and_then(|p| p.canonicalize().ok()) == Some(me.clone())
    })
}

/// Our `.desktop` file for this program. `dbus`: launch through D-Bus activation; `scheme`:
/// also handle `specular://` links.
pub(crate) fn desktop_entry(env: &Env, labels: &Labels, dbus: bool, scheme: bool) -> String {
    let exe = desktop_quote(&env.exe);
    let mut types: Vec<&str> = formats::mime_types().collect();
    if scheme {
        types.push(url_scheme::SCHEME_TYPE);
    }
    let esc = |s: &str| s.replace(['\n', '\r'], " ");
    let mut s = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Specular\n\
         GenericName={generic}\n\
         Comment={comment}\n\
         Exec={exe} %U\n\
         TryExec={try_exec}\n\
         Icon={APP_ID}\n\
         Terminal=false\n\
         Categories=Graphics;Viewer;2DGraphics;RasterGraphics;\n\
         MimeType={mimes}\n\
         Keywords=image;photo;picture;viewer;\n\
         StartupNotify=true\n\
         StartupWMClass={APP_ID}\n\
         DBusActivatable={dbus}\n\
         Actions={actions}\n\
         {MARKER}=true\n",
        generic = esc(&labels.generic_name),
        comment = esc(&labels.comment),
        mimes = formats::mime_list(&types),
        try_exec = keyfile_escape(&env.exe.to_string_lossy()),
        actions = DESKTOP_ACTIONS
            .iter()
            .map(|a| format!("{};", a.id()))
            .collect::<String>(),
    );
    // Launcher and dock menus (GNOME, KDE, Cinnamon…): the freedesktop counterpart of jump
    // lists. With D-Bus activation the shell calls `ActivateAction` on the running viewer
    // instead of running `Exec`.
    for a in DESKTOP_ACTIONS {
        s.push_str(&format!(
            "\n[Desktop Action {}]\nName={}\nExec={exe} {}\n",
            a.id(),
            esc(a.label(labels)),
            a.flag()
        ));
    }
    s
}

/// The actions of our `.desktop` file (the launcher's right-click menu), shared with the other
/// platforms' launcher menus.
pub use crate::integration::shell::{DESKTOP_ACTIONS, DesktopAction};

/// Makes the local `.desktop` file match what the options need: present when the default
/// viewer needs it (no package installed it), D-Bus activation or the URL scheme is on, with
/// `DBusActivatable` and the scheme type set accordingly; gone otherwise.
pub(crate) fn sync_desktop_file(env: &Env, labels: &Labels) -> Result<()> {
    let viewer = Record::load(env, OptionId::DefaultViewer.key()).is_some();
    let dbus = Record::load(env, OptionId::DbusActivation.key()).is_some();
    let scheme = Record::load(env, OptionId::UrlScheme.key()).is_some();
    let path = local_desktop_file(env);
    let mut rec = Record::load(env, "desktop-file").unwrap_or_default();
    if dbus || scheme || (viewer && !packaged(env)) {
        install(
            &mut rec,
            &path,
            &desktop_entry(env, labels, dbus, scheme),
            0o644,
        )?;
        rec.save(env, "desktop-file")?;
    } else {
        uninstall(&rec);
        Record::delete(env, "desktop-file");
    }
    let apps = env.data_home.join("applications");
    refresh(env, "update-desktop-database", &apps);
    // Our file was the last one: the cache `update-desktop-database` made describes nothing.
    let no_entries = fs::read_dir(&apps).is_ok_and(|rd| {
        !rd.flatten()
            .any(|e| e.path().extension().is_some_and(|x| x == "desktop"))
    });
    if no_entries {
        let _ = fs::remove_file(apps.join("mimeinfo.cache"));
    }
    Ok(())
}

/// Removal path of [`sync_desktop_file`] (labels are not needed to delete).
pub(crate) fn sync_desktop_file_after_removal(env: &Env) -> Result<()> {
    let labels = existing_labels(env);
    sync_desktop_file(env, &labels)
}

/// The labels already in our desktop file (so turning one option off does not re-translate
/// the file the other option still needs).
fn existing_labels(env: &Env) -> Labels {
    let mut l = Labels::default();
    if let Ok(t) = fs::read_to_string(local_desktop_file(env)) {
        let kf = keyfile::KeyFile::parse(&t);
        if let Some(g) = kf.get("Desktop Entry", "GenericName") {
            l.generic_name = g.to_owned();
        }
        if let Some(c) = kf.get("Desktop Entry", "Comment") {
            l.comment = c.to_owned();
        }
        for a in DESKTOP_ACTIONS {
            if let Some(n) = kf.get(&format!("Desktop Action {}", a.id()), "Name") {
                a.set_label(&mut l, n);
            }
        }
    }
    l
}

#[cfg(test)]
pub(crate) mod test_env {
    use super::*;

    /// A fresh Env under `target/agent-app/itest-unit/<name>-<pid>` (never the real home).
    pub fn env(name: &str) -> Env {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/agent-app/itest-unit")
            .join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let exe = root.join("bin dir/specular");
        fs::create_dir_all(exe.parent().unwrap()).unwrap();
        fs::write(&exe, "#!/bin/sh\n").unwrap();
        Env::under(&root, &exe)
    }

    pub fn cleanup(env: &Env) {
        if let Some(root) = env.data_home.parent() {
            let _ = fs::remove_dir_all(root);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting() {
        assert_eq!(
            desktop_quote(Path::new("/usr/bin/specular")),
            "/usr/bin/specular"
        );
        assert_eq!(
            desktop_quote(Path::new("/opt/My Apps/spe$cula")),
            "\"/opt/My Apps/spe\\\\$cula\""
        );
        assert_eq!(
            shell_quote(Path::new("/usr/bin/specular")),
            "/usr/bin/specular"
        );
        assert_eq!(shell_quote(Path::new("/a b/it's")), "'/a b/it'\\''s'");
    }

    #[test]
    fn desktop_entry_has_launcher_actions() {
        let env = test_env::env("actions");
        let labels = Labels {
            library: "Bibliothek".into(),
            ..Labels::default()
        };
        let text = desktop_entry(&env, &labels, true, false);
        let kf = keyfile::KeyFile::parse(&text);
        assert_eq!(
            kf.get("Desktop Entry", "Actions"),
            Some("new-window;library;slideshow;")
        );
        let exe = desktop_quote(&env.exe);
        for a in DESKTOP_ACTIONS {
            let group = format!("Desktop Action {}", a.id());
            assert_eq!(
                kf.get(&group, "Exec").map(str::to_owned),
                Some(format!("{exe} {}", a.flag()))
            );
            assert!(kf.get(&group, "Name").is_some_and(|n| !n.is_empty()));
            assert_eq!(DesktopAction::from_id(a.id()), Some(a));
        }
        assert_eq!(kf.get("Desktop Action library", "Name"), Some("Bibliothek"));
        // Every registry type is offered; the main group still carries the marker.
        let mimes = kf.get("Desktop Entry", "MimeType").unwrap();
        for (m, _) in formats::IMAGE_TYPES {
            assert!(mimes.split(';').any(|x| x == *m), "{m} missing");
        }
        assert_eq!(kf.get("Desktop Entry", MARKER), Some("true"));
        // Turning one option off keeps the labels another one still needs.
        let p = local_desktop_file(&env);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, &text).unwrap();
        let back = existing_labels(&env);
        assert_eq!(back.library, "Bibliothek");
        assert_eq!(back.new_window, labels.new_window);
        test_env::cleanup(&env);
    }

    #[test]
    fn install_refuses_foreign_files_and_uninstall_keeps_them() {
        let env = test_env::env("install");
        let mut rec = Record::default();
        let p = env.data_home.join("x/ours.desktop");
        install(&mut rec, &p, &format!("{MARKER}=1\n"), 0o644).unwrap();
        install(&mut rec, &p, &format!("{MARKER}=1\n"), 0o644).unwrap();
        assert_eq!(rec.files().count(), 1);
        let foreign = env.data_home.join("x/theirs.desktop");
        fs::write(&foreign, "user's own").unwrap();
        assert!(matches!(
            install(&mut rec, &foreign, &format!("{MARKER}=1\n"), 0o644),
            Err(Error::Unrecognized(..))
        ));
        assert_eq!(fs::read_to_string(&foreign).unwrap(), "user's own");
        // A file of ours later replaced by the user stays.
        rec.add_file(&foreign);
        uninstall(&rec);
        assert!(!p.exists());
        assert!(foreign.exists());
        test_env::cleanup(&env);
    }

    #[test]
    fn records_and_backups() {
        let env = test_env::env("records");
        let mut r = Record::default();
        r.add_file(Path::new("/a/b"));
        r.rows
            .push(vec!["prev".into(), "image/png".into(), "-".into()]);
        r.save(&env, "x").unwrap();
        assert_eq!(Record::load(&env, "x").unwrap(), r);
        let f = env.config_home.join("mimeapps.list");
        assert_eq!(backup(&env, &f).unwrap(), None);
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(&f, "[Default Applications]\n").unwrap();
        for _ in 0..KEEP_BACKUPS + 3 {
            let b = backup(&env, &f).unwrap().unwrap();
            assert_eq!(fs::read(&b).unwrap(), b"[Default Applications]\n");
        }
        let n = fs::read_dir(env.record_dir().join("backups"))
            .unwrap()
            .count();
        assert_eq!(n, KEEP_BACKUPS);
        test_env::cleanup(&env);
    }

    #[test]
    fn symlinked_files_are_written_through() {
        let env = test_env::env("symlink");
        let real = env.data_home.join("dotfiles/mimeapps.list");
        fs::create_dir_all(real.parent().unwrap()).unwrap();
        fs::write(&real, "old").unwrap();
        fs::create_dir_all(&env.config_home).unwrap();
        let link = env.config_home.join("mimeapps.list");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        write_atomic(&link, b"new", 0o644).unwrap();
        assert!(link.is_symlink());
        assert_eq!(fs::read_to_string(&real).unwrap(), "new");
        test_env::cleanup(&env);
    }
}
