//! "Open in Specular", "Compare in Specular" (several files: browse exactly those) and "Set as
//! wallpaper" in the context menus of the file managers found on this system.
//!
//! | File manager | What is written |
//! |---|---|
//! | Nautilus (GNOME Files) | scripts in `$XDG_DATA_HOME/nautilus/scripts/` (the Scripts submenu) |
//! | Dolphin (KDE) | service menus in `$XDG_DATA_HOME/kio/servicemenus/` (executable, as Plasma 6 requires) |
//! | Thunar (Xfce) | actions merged into `$XDG_CONFIG_HOME/Thunar/uca.xml`, see [`super::uca`] |
//! | Nemo (Cinnamon) | `$XDG_DATA_HOME/nemo/actions/specular-*.nemo_action` |

use std::fs;
use std::path::{Path, PathBuf};

use super::uca;
use super::{APP_ID, MARKER, Record, backup, desktop_quote, formats, install, is_ours};
use super::{shell_quote, uninstall, write_atomic};
use crate::integration::{Env, Error, Labels, OptionId, Result, State, io_err};

/// A supported file manager.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileManager {
    Nautilus,
    Dolphin,
    Thunar,
    Nemo,
}

impl FileManager {
    pub const ALL: [FileManager; 4] = [
        FileManager::Nautilus,
        FileManager::Dolphin,
        FileManager::Thunar,
        FileManager::Nemo,
    ];

    pub fn name(self) -> &'static str {
        match self {
            FileManager::Nautilus => "Nautilus",
            FileManager::Dolphin => "Dolphin",
            FileManager::Thunar => "Thunar",
            FileManager::Nemo => "Nemo",
        }
    }

    fn program(self) -> &'static str {
        match self {
            FileManager::Nautilus => "nautilus",
            FileManager::Dolphin => "dolphin",
            FileManager::Thunar => "thunar",
            FileManager::Nemo => "nemo",
        }
    }
}

/// The file managers installed (on `PATH`).
pub fn present(env: &Env) -> Vec<FileManager> {
    FileManager::ALL
        .into_iter()
        .filter(|f| env.find(f.program()).is_some())
        .collect()
}

/// One menu entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Entry {
    Open,
    Compare,
    Wallpaper,
}

const ENTRIES: [Entry; 3] = [Entry::Open, Entry::Compare, Entry::Wallpaper];

impl Entry {
    fn id(self) -> &'static str {
        match self {
            Entry::Open => "open",
            Entry::Compare => "compare",
            Entry::Wallpaper => "wallpaper",
        }
    }

    fn label(self, l: &Labels) -> String {
        match self {
            Entry::Open => l.open.clone(),
            Entry::Compare => l.compare.clone(),
            Entry::Wallpaper => l.wallpaper.clone(),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Entry::Wallpaper => "preferences-desktop-wallpaper",
            _ => APP_ID,
        }
    }

    /// Arguments after the program, before the file placeholder.
    fn flag(self) -> &'static str {
        match self {
            Entry::Wallpaper => "--set-wallpaper ",
            _ => "",
        }
    }
}

fn header(fm: FileManager) -> String {
    format!(
        "{MARKER}: installed by Specular (Settings > Integration > File-manager actions, {})",
        fm.name()
    )
}

/// A file-name-safe version of a menu label.
fn file_name(label: &str) -> String {
    let s: String = label
        .chars()
        .map(|c| if c == '/' || c.is_control() { '-' } else { c })
        .collect();
    let s = s.trim().trim_start_matches('.').to_owned();
    if s.is_empty() { "Specular".into() } else { s }
}

// ----- Nautilus ---------------------------------------------------------------------------

fn nautilus(env: &Env, labels: &Labels, rec: &mut Record) -> Result<()> {
    let dir = env.data_home.join("nautilus/scripts");
    for e in ENTRIES {
        let path = dir.join(file_name(&e.label(labels)));
        // Nautilus runs scripts in the folder with the selected names as arguments; make them
        // absolute (a name may start with `-`).
        let run = match e {
            Entry::Wallpaper => format!("exec {} --set-wallpaper \"$1\"", shell_quote(&env.exe)),
            _ => format!("exec {} \"$@\"", shell_quote(&env.exe)),
        };
        let script = format!(
            "#!/bin/sh\n\
             # {}\n\
             d=$(pwd)\n\
             for f do\n  \
               case $f in /*) ;; *) f=$d/$f ;; esac\n  \
               set -- \"$@\" \"$f\"\n  \
               shift\n\
             done\n\
             [ $# -gt 0 ] || exit 0\n\
             {run}\n",
            header(FileManager::Nautilus)
        );
        install(rec, &path, &script, 0o755)?;
    }
    Ok(())
}

// ----- Dolphin ----------------------------------------------------------------------------

fn dolphin(env: &Env, labels: &Labels, rec: &mut Record) -> Result<()> {
    let dir = env.data_home.join("kio/servicemenus");
    let types: Vec<&str> = formats::mime_types().collect();
    let mimes = formats::mime_list(&types);
    let exe = desktop_quote(&env.exe);
    let action = |e: Entry, key: &str| {
        format!(
            "\n[Desktop Action {key}]\nName={}\nIcon={}\nExec={exe} {}%{}\n",
            e.label(labels).replace(['\n', '\r'], " "),
            e.icon(),
            e.flag(),
            if e == Entry::Wallpaper { "f" } else { "F" },
        )
    };
    let single = format!(
        "# {}\n[Desktop Entry]\nType=Service\nMimeType={mimes}\nActions=open;wallpaper;\n\
         X-KDE-Submenu=\n{MARKER}=true\n{}{}",
        header(FileManager::Dolphin),
        action(Entry::Open, "open"),
        action(Entry::Wallpaper, "wallpaper"),
    );
    let compare = format!(
        "# {}\n[Desktop Entry]\nType=Service\nMimeType={mimes}\nActions=compare;\n\
         X-KDE-MinNumberOfUrls=2\n{MARKER}=true\n{}",
        header(FileManager::Dolphin),
        action(Entry::Compare, "compare"),
    );
    install(rec, &dir.join(format!("{APP_ID}.desktop")), &single, 0o755)?;
    install(
        rec,
        &dir.join(format!("{APP_ID}-compare.desktop")),
        &compare,
        0o755,
    )?;
    Ok(())
}

// ----- Nemo -------------------------------------------------------------------------------

fn nemo(env: &Env, labels: &Labels, rec: &mut Record) -> Result<()> {
    let dir = env.data_home.join("nemo/actions");
    let types: Vec<&str> = formats::mime_types().collect();
    let mimes = formats::mime_list(&types);
    for e in ENTRIES {
        let (selection, arg) = match e {
            Entry::Open => ("notnone", "%F"),
            Entry::Compare => ("m", "%F"),
            Entry::Wallpaper => ("s", "%F"),
        };
        let text = format!(
            "# {}\n[Nemo Action]\nActive=true\nName={}\nComment={}\nIcon-Name={}\n\
             Exec={} {}{arg}\nSelection={selection}\nMimetypes={mimes}\n{MARKER}=true\n",
            header(FileManager::Nemo),
            e.label(labels).replace(['\n', '\r'], " "),
            e.label(labels).replace(['\n', '\r'], " "),
            e.icon(),
            shell_quote(&env.exe),
            e.flag(),
        );
        install(
            rec,
            &dir.join(format!("specular-{}.nemo_action", e.id())),
            &text,
            0o644,
        )?;
    }
    Ok(())
}

// ----- Thunar -----------------------------------------------------------------------------

fn uca_path(env: &Env) -> PathBuf {
    env.config_home.join("Thunar/uca.xml")
}

fn read_text(p: &Path) -> Result<Option<String>> {
    match fs::read(p) {
        Ok(b) => String::from_utf8(b)
            .map(Some)
            .map_err(|_| Error::Unrecognized(p.to_path_buf(), "not UTF-8".into())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io_err(p)(e)),
    }
}

/// Mode of an existing file (kept when rewriting it).
fn mode_of(p: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(p).map_or(0o644, |m| m.permissions().mode() & 0o7777)
}

fn thunar_actions(env: &Env, labels: &Labels) -> Vec<uca::Action> {
    let patterns: Vec<String> = formats::IMAGE_TYPES
        .iter()
        .flat_map(|(_, exts)| exts.iter())
        .flat_map(|e| [format!("*.{e}"), format!("*.{}", e.to_uppercase())])
        .collect();
    let patterns = patterns.join(";");
    ENTRIES
        .into_iter()
        .map(|e| uca::Action {
            id: e.id(),
            name: e.label(labels),
            description: e.label(labels),
            icon: e.icon().into(),
            command: format!(
                "{} {}{}",
                shell_quote(&env.exe),
                e.flag(),
                if e == Entry::Wallpaper { "%f" } else { "%F" }
            ),
            patterns: patterns.clone(),
        })
        .collect()
}

/// Rewrites `uca.xml` with `actions` as ours (empty: none), backing it up first. Unchanged
/// files are not touched.
fn thunar_write(env: &Env, actions: &[uca::Action]) -> Result<()> {
    let path = uca_path(env);
    let old = read_text(&path)?;
    if old.is_none() && actions.is_empty() {
        return Ok(());
    }
    let new = if actions.is_empty() {
        uca::strip(old.as_deref().unwrap_or_default())
    } else {
        uca::merge(old.as_deref(), actions)
    }
    .map_err(|why| Error::Unrecognized(path.clone(), why))?;
    if old.as_deref() == Some(new.as_str()) {
        return Ok(());
    }
    backup(env, &path)?;
    write_atomic(&path, new.as_bytes(), mode_of(&path))?;
    specular_log::debug!(target: specular_log::SETTINGS, "integration: updated {}", path.display());
    Ok(())
}

// ----- The option -------------------------------------------------------------------------

pub(super) fn apply(env: &Env, labels: &Labels) -> Result<()> {
    let key = OptionId::FileManagerActions.key();
    let rec = Record::load(env, key).unwrap_or_default();
    // Labels may have changed since last time (another language): drop files we wrote
    // under other names first.
    let old_files: Vec<PathBuf> = rec.files().map(Path::to_path_buf).collect();
    let mut fresh = Record::default();
    let r = present(env).into_iter().try_for_each(|fm| match fm {
        FileManager::Nautilus => nautilus(env, labels, &mut fresh),
        FileManager::Dolphin => dolphin(env, labels, &mut fresh),
        FileManager::Nemo => nemo(env, labels, &mut fresh),
        FileManager::Thunar => thunar_write(env, &thunar_actions(env, labels)),
    });
    if let Err(e) = r {
        // Keep track of everything written so far, so removing still finds it.
        for f in old_files {
            fresh.add_file(&f);
        }
        fresh.save(env, key)?;
        return Err(e);
    }
    let stale = Record {
        rows: old_files
            .into_iter()
            .filter(|f| !fresh.files().any(|n| n == f))
            .map(|f| vec!["file".into(), f.to_string_lossy().into_owned()])
            .collect(),
    };
    uninstall(&stale);
    fresh.save(env, key)
}

pub(super) fn remove(env: &Env) -> Result<()> {
    let key = OptionId::FileManagerActions.key();
    let rec = Record::load(env, key).unwrap_or_default();
    uninstall(&rec);
    // Our Thunar actions go whether or not the record knows of them.
    let path = uca_path(env);
    if read_text(&path)?.is_some_and(|t| !uca::ours(&t).is_empty()) {
        thunar_write(env, &[])?;
    }
    Record::delete(env, key);
    Ok(())
}

/// Per file manager found: whether our entries are in place.
fn installed(env: &Env) -> Vec<(FileManager, bool)> {
    let rec = Record::load(env, OptionId::FileManagerActions.key()).unwrap_or_default();
    present(env)
        .into_iter()
        .map(|fm| {
            let ok = match fm {
                FileManager::Thunar => read_text(&uca_path(env))
                    .ok()
                    .flatten()
                    .is_some_and(|t| uca::ours(&t).len() == ENTRIES.len()),
                FileManager::Nautilus => dir_has(&rec, &env.data_home.join("nautilus/scripts"), 3),
                FileManager::Dolphin => dir_has(&rec, &env.data_home.join("kio/servicemenus"), 2),
                FileManager::Nemo => dir_has(&rec, &env.data_home.join("nemo/actions"), 3),
            };
            (fm, ok)
        })
        .collect()
}

fn dir_has(rec: &Record, dir: &Path, n: usize) -> bool {
    rec.files()
        .filter(|f| f.parent() == Some(dir) && is_ours(f))
        .count()
        >= n
}

pub(super) fn status(env: &Env) -> (State, Vec<String>) {
    let all = installed(env);
    let on: Vec<String> = all
        .iter()
        .filter(|(_, ok)| *ok)
        .map(|(fm, _)| fm.name().to_owned())
        .collect();
    let state = if on.is_empty() {
        State::Off
    } else if on.len() == all.len() {
        State::On
    } else {
        State::Partial
    };
    (state, on)
}

#[cfg(test)]
mod tests {
    use super::super::test_env;
    use super::*;

    /// An Env with fake file managers on its PATH.
    fn env_with(name: &str, fms: &[&str]) -> Env {
        let mut env = test_env::env(name);
        let bin = env.data_home.parent().unwrap().join("path");
        fs::create_dir_all(&bin).unwrap();
        for f in fms {
            fs::write(bin.join(f), "").unwrap();
        }
        env.path = vec![bin];
        env
    }

    #[test]
    fn all_file_managers_round_trip() {
        let env = env_with("fm", &["nautilus", "dolphin", "thunar", "nemo"]);
        assert_eq!(present(&env).len(), 4);
        let uca = uca_path(&env);
        fs::create_dir_all(uca.parent().unwrap()).unwrap();
        let user = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n<action>\n\
                    \t<name>Mine</name>\n\t<unique-id>1-1</unique-id>\n\t<command>x %f</command>\n\
                    </action>\n</actions>\n";
        fs::write(&uca, user).unwrap();
        assert_eq!(status(&env).0, State::Off);

        apply(&env, &Labels::default()).unwrap();
        let (state, on) = status(&env);
        assert_eq!(state, State::On, "{on:?}");
        assert_eq!(on.len(), 4);
        let script = env.data_home.join("nautilus/scripts/Open in Specular");
        let s = fs::read_to_string(&script).unwrap();
        assert!(
            s.contains("exec '") && s.contains("bin dir/specular' \"$@\""),
            "{s}"
        );
        let dolphin = fs::read_to_string(
            env.data_home
                .join("kio/servicemenus/app.specular.Specular.desktop"),
        )
        .unwrap();
        assert!(
            dolphin.contains("Exec=\"") && dolphin.contains("--set-wallpaper %f\n"),
            "{dolphin}"
        );
        let nemo = fs::read_to_string(
            env.data_home
                .join("nemo/actions/specular-compare.nemo_action"),
        )
        .unwrap();
        assert!(nemo.contains("Selection=m\n"));
        let merged = fs::read_to_string(&uca).unwrap();
        assert!(merged.starts_with(&user[..user.rfind("</actions>").unwrap()]));
        assert_eq!(uca::ours(&merged).len(), 3);
        // A backup of the user's file was made.
        let backups: Vec<_> = fs::read_dir(env.record_dir().join("backups"))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(fs::read_to_string(backups[0].path()).unwrap(), user);

        // Idempotent: no change, no new backup.
        apply(&env, &Labels::default()).unwrap();
        assert_eq!(fs::read_to_string(&uca).unwrap(), merged);
        assert_eq!(
            fs::read_dir(env.record_dir().join("backups"))
                .unwrap()
                .count(),
            1
        );

        // Another language: old Nautilus scripts are replaced, not left behind.
        let de = Labels {
            open: "In Specular öffnen".into(),
            ..Labels::default()
        };
        apply(&env, &de).unwrap();
        assert!(!script.exists());
        assert!(
            env.data_home
                .join("nautilus/scripts/In Specular öffnen")
                .exists()
        );

        // A user script of the same name is never overwritten.
        let mine = env.data_home.join("nautilus/scripts/Compare in Specular");
        fs::remove_file(&mine).unwrap();
        fs::write(&mine, "#!/bin/sh\necho mine\n").unwrap();
        assert!(matches!(apply(&env, &de), Err(Error::Unrecognized(..))));
        assert_eq!(fs::read_to_string(&mine).unwrap(), "#!/bin/sh\necho mine\n");

        remove(&env).unwrap();
        assert_eq!(fs::read_to_string(&uca).unwrap(), user);
        assert!(mine.exists(), "the user's script stays");
        assert!(
            !env.data_home
                .join("nautilus/scripts/In Specular öffnen")
                .exists()
        );
        assert!(
            !env.data_home
                .join("kio/servicemenus/app.specular.Specular.desktop")
                .exists()
        );
        assert_eq!(
            fs::read_dir(env.data_home.join("nemo/actions"))
                .unwrap()
                .count(),
            0
        );
        assert_eq!(status(&env).0, State::Off);
        test_env::cleanup(&env);
    }

    #[test]
    fn unreadable_uca_is_left_alone() {
        let env = env_with("fm-bad", &["thunar"]);
        let uca = uca_path(&env);
        fs::create_dir_all(uca.parent().unwrap()).unwrap();
        fs::write(&uca, "<actions>\n<action>\n").unwrap();
        assert!(matches!(
            apply(&env, &Labels::default()),
            Err(Error::Unrecognized(..))
        ));
        assert_eq!(fs::read_to_string(&uca).unwrap(), "<actions>\n<action>\n");
        assert!(!env.record_dir().join("backups").exists());
        test_env::cleanup(&env);
    }

    #[test]
    fn nothing_without_file_managers() {
        let env = env_with("fm-none", &[]);
        assert!(present(&env).is_empty());
        test_env::cleanup(&env);
    }

    #[test]
    fn script_runs_with_absolute_paths() {
        // The Nautilus script turns relative names into absolute paths and passes them on.
        let env = env_with("fm-script", &["nautilus"]);
        let root = env.data_home.parent().unwrap().to_path_buf();
        let out = root.join("args.txt");
        fs::write(
            &env.exe,
            format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\n", out.display()),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&env.exe, fs::Permissions::from_mode(0o755)).unwrap();
        apply(&env, &Labels::default()).unwrap();
        let script = env.data_home.join("nautilus/scripts/Open in Specular");
        let st = std::process::Command::new(&script)
            .args(["-a.jpg", "b c.png", "/abs/d.jpg"])
            .current_dir(&root)
            .status()
            .unwrap();
        assert!(st.success());
        let got = fs::read_to_string(&out).unwrap();
        let r = root.display();
        assert_eq!(got, format!("{r}/-a.jpg\n{r}/b c.png\n/abs/d.jpg\n"));
        test_env::cleanup(&env);
    }
}
