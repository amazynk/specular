//! Default viewer: our `.desktop` file and MIME globs in `~/.local/share` (unless a package
//! installed them), and Specular as the default application for the image types in
//! `$XDG_CONFIG_HOME/mimeapps.list` (through `xdg-mime default` when installed, else our own
//! editor). The values replaced are recorded, and removal puts them back, unless the user
//! has picked another application since.

use std::fs;
use std::path::{Path, PathBuf};

use specular_log::{SETTINGS, info, warn};

use super::keyfile::{KeyFile, split_list};
use super::{
    APP_ID, Record, backup, formats, install, refresh, run_tool, sync_desktop_file,
    sync_desktop_file_after_removal, uninstall, write_atomic,
};
use crate::integration::{Env, Error, Labels, OptionId, Result, State};

pub(super) const DEFAULTS: &str = "Default Applications";

pub(super) fn desktop_id() -> String {
    format!("{APP_ID}.desktop")
}

fn record_name() -> &'static str {
    OptionId::DefaultViewer.key()
}

/// The user's `mimeapps.list` (the file `xdg-mime default` writes).
pub(crate) fn mimeapps_path(env: &Env) -> PathBuf {
    env.config_home.join("mimeapps.list")
}

pub(super) fn read_keyfile(path: &Path) -> Result<(KeyFile, u32)> {
    use std::os::unix::fs::PermissionsExt;
    match fs::read(path) {
        Ok(bytes) => {
            let text = String::from_utf8(bytes)
                .map_err(|_| Error::Unrecognized(path.into(), "not UTF-8 text".into()))?;
            let mode = fs::metadata(path).map_or(0o644, |m| m.permissions().mode() & 0o7777);
            Ok((KeyFile::parse(&text), mode))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok((KeyFile::parse(""), 0o644)),
        Err(e) => Err(Error::Io(path.into(), e)),
    }
}

fn mime_package_path(env: &Env) -> PathBuf {
    env.data_home
        .join("mime/packages")
        .join(format!("{APP_ID}.xml"))
}

/// Previous values recorded for each type (`None`: the key was absent).
fn recorded(rec: &Record) -> Vec<(String, Option<String>)> {
    rec.rows
        .iter()
        .filter(|r| r.len() == 3 && r[0] == "prev")
        .map(|r| (r[1].clone(), (r[2] != "-").then(|| r[2].clone())))
        .collect()
}

/// The value with our id first and the previous entries kept as fallbacks.
pub(super) fn ours_first(prev: Option<&str>) -> String {
    let me = desktop_id();
    let mut v = format!("{me};");
    for e in prev.map(split_list).unwrap_or_default() {
        if e != me {
            v.push_str(e);
            v.push(';');
        }
    }
    v
}

pub(super) fn first_is_ours(v: Option<&str>) -> bool {
    v.and_then(|v| split_list(v).first().copied()) == Some(desktop_id().as_str())
}

pub(super) fn apply(env: &Env, labels: &Labels) -> Result<()> {
    let path = mimeapps_path(env);
    let (kf, mode) = read_keyfile(&path)?;
    let mut rec = Record::load(env, record_name()).unwrap_or_default();
    let types = env.default_viewer_types();
    // Extra types claimed before and unticked since: given back below, forgotten here so
    // ticking them again records the value of that time.
    let dropped: Vec<(String, Option<String>)> = recorded(&rec)
        .into_iter()
        .filter(|(m, _)| !types.contains(&m.as_str()))
        .collect();
    rec.rows
        .retain(|r| !(r.len() == 3 && r[0] == "prev" && dropped.iter().any(|(m, _)| *m == r[1])));
    let known: Vec<String> = recorded(&rec).into_iter().map(|(m, _)| m).collect();
    for mime in &types {
        if known.iter().any(|k| k == mime) {
            continue; // Applied before: the first recorded value is the one to restore.
        }
        let prev = kf.get(DEFAULTS, mime).filter(|v| !first_is_ours(Some(v)));
        let prev = prev.filter(|v| !v.contains(['\t', '\n'])).unwrap_or("-");
        rec.rows
            .push(vec!["prev".into(), (*mime).into(), prev.into()]);
    }
    // Recorded before anything changes, so an interrupted apply can still be undone (the
    // dropped types' previous values stay in hand until the file is written).
    rec.save(env, record_name())?;

    install(
        &mut rec,
        &mime_package_path(env),
        &formats::mime_package(),
        0o644,
    )?;
    rec.save(env, record_name())?;
    refresh(env, "update-mime-database", &env.data_home.join("mime"));
    sync_desktop_file(env, labels)?;

    let all_ours = types.iter().all(|m| first_is_ours(kf.get(DEFAULTS, m)));
    let give_back = dropped
        .iter()
        .any(|(m, _)| first_is_ours(kf.get(DEFAULTS, m)));
    // Already the default for every type: leave the file (and its backups) alone.
    if all_ours && !give_back {
        return Ok(());
    }
    backup(env, &path)?;
    if !all_ours {
        let mut args: Vec<&std::ffi::OsStr> = vec!["default".as_ref()];
        let id = desktop_id();
        args.push(id.as_ref());
        args.extend(types.iter().map(|t| std::ffi::OsStr::new(*t)));
        match run_tool(env, "xdg-mime", &args) {
            Ok(true) => {
                let n = types.len();
                info!(target: SETTINGS, "integration: xdg-mime default {id} for {n} types")
            }
            Ok(false) => {}
            Err(e) => {
                warn!(target: SETTINGS, "integration: {e}; editing {} directly", path.display())
            }
        }
    }
    // Whatever xdg-mime did (or if it is missing), make sure every type is ours, keeping the
    // previous applications as fallbacks, and give the unticked types back.
    let (mut kf, _) = read_keyfile(&path)?;
    let before = kf.to_text();
    for mime in &types {
        let cur = kf.get(DEFAULTS, mime).map(str::to_owned);
        if !first_is_ours(cur.as_deref()) {
            kf.set(DEFAULTS, mime, &ours_first(cur.as_deref()));
        }
    }
    for (mime, prev) in &dropped {
        restore(&mut kf, mime, prev.as_deref());
    }
    if kf.to_text() != before {
        write_atomic(&path, kf.to_text().as_bytes(), mode)?;
    }
    Ok(())
}

/// Puts back the value `mime` had before we claimed it (`None`: there was none), unless the
/// user has chosen another application since: then the entry is theirs and stays.
fn restore(kf: &mut KeyFile, mime: &str, prev: Option<&str>) {
    if !first_is_ours(kf.get(DEFAULTS, mime)) {
        return;
    }
    match prev {
        Some(prev) => kf.set(DEFAULTS, mime, prev),
        None => {
            kf.remove(DEFAULTS, mime);
        }
    }
}

pub(super) fn remove(env: &Env) -> Result<()> {
    let rec = Record::load(env, record_name()).unwrap_or_default();
    let prevs = recorded(&rec);
    let path = mimeapps_path(env);
    let (mut kf, mode) = read_keyfile(&path)?;
    let before = kf.to_text();
    let mut types: Vec<String> = env
        .default_viewer_types()
        .into_iter()
        .map(str::to_owned)
        .collect();
    for (m, _) in &prevs {
        if !types.contains(m) {
            types.push(m.clone());
        }
    }
    for mime in &types {
        let prev = prevs
            .iter()
            .find(|(m, _)| m == mime)
            .and_then(|(_, p)| p.clone());
        restore(&mut kf, mime, prev.as_deref());
    }
    if kf.to_text() != before {
        backup(env, &path)?;
        write_atomic(&path, kf.to_text().as_bytes(), mode)?;
        info!(target: SETTINGS, "integration: restored the previous defaults in {}", path.display());
    }
    uninstall(&rec);
    refresh(env, "update-mime-database", &env.data_home.join("mime"));
    super::Record::delete(env, record_name());
    sync_desktop_file_after_removal(env)?;
    Ok(())
}

pub(super) fn status(env: &Env) -> (State, Vec<String>) {
    let applied = Record::load(env, record_name()).is_some();
    let me = desktop_id();
    let types = env.default_viewer_types();
    let total = types.len();
    let ours = types
        .iter()
        .filter(|m| effective_default(env, m).as_deref() == Some(me.as_str()))
        .count();
    let state = if ours == total {
        State::On
    } else if ours > 0 || applied {
        State::Partial
    } else {
        State::Off
    };
    (state, vec![format!("{ours}/{total}")])
}

/// Whether a desktop file with this id is installed.
fn installed(env: &Env, id: &str) -> bool {
    std::iter::once(&env.data_home)
        .chain(&env.data_dirs)
        .any(|d| d.join("applications").join(id).is_file())
}

/// The default application of `mime` as the MIME applications specification finds it (what
/// `xdg-mime query default` answers): desktop-specific lists before generic ones, the user's
/// before the system's, and only installed applications count.
pub fn effective_default(env: &Env, mime: &str) -> Option<String> {
    let mut dirs: Vec<PathBuf> = vec![env.config_home.clone()];
    dirs.extend(env.config_dirs.iter().cloned());
    dirs.push(env.data_home.join("applications"));
    dirs.extend(env.data_dirs.iter().map(|d| d.join("applications")));
    let mut names: Vec<String> = env
        .desktops
        .iter()
        .map(|d| format!("{}-mimeapps.list", d.to_lowercase()))
        .collect();
    names.push("mimeapps.list".into());
    for dir in &dirs {
        for name in &names {
            let Ok(text) = fs::read_to_string(dir.join(name)) else {
                continue;
            };
            let kf = KeyFile::parse(&text);
            if let Some(v) = kf.get(DEFAULTS, mime)
                && let Some(id) = split_list(v).into_iter().find(|id| installed(env, id))
            {
                return Some(id.to_owned());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::test_env;
    use super::*;

    const USER_LIST: &str = "# keep me\n\
        [Default Applications]\n\
        image/jpeg=org.gnome.Loupe.desktop;\n\
        text/plain=org.gnome.TextEditor.desktop\n\
        \n\
        [Added Associations]\n\
        image/png=gimp.desktop;\n";

    fn setup(name: &str) -> Env {
        let env = test_env::env(name);
        let apps = env.data_dirs[0].join("applications");
        fs::create_dir_all(&apps).unwrap();
        for id in ["org.gnome.Loupe.desktop", "gimp.desktop", "eog.desktop"] {
            fs::write(apps.join(id), "[Desktop Entry]\n").unwrap();
        }
        // A system default the user list does not override for PNG.
        fs::write(
            apps.join("mimeapps.list"),
            "[Default Applications]\nimage/png=eog.desktop\n",
        )
        .unwrap();
        fs::create_dir_all(&env.config_home).unwrap();
        fs::write(mimeapps_path(&env), USER_LIST).unwrap();
        env
    }

    #[test]
    fn apply_and_remove_restore_the_file() {
        let env = setup("viewer");
        assert_eq!(status(&env).0, State::Off);
        assert_eq!(
            effective_default(&env, "image/jpeg").as_deref(),
            Some("org.gnome.Loupe.desktop")
        );
        assert_eq!(
            effective_default(&env, "image/png").as_deref(),
            Some("eog.desktop")
        );

        apply(&env, &Labels::default()).unwrap();
        let after = fs::read_to_string(mimeapps_path(&env)).unwrap();
        assert!(after.starts_with("# keep me\n[Default Applications]\n"));
        assert!(
            after.contains("image/jpeg=app.specular.Specular.desktop;org.gnome.Loupe.desktop;\n")
        );
        assert!(after.contains("text/plain=org.gnome.TextEditor.desktop\n"));
        assert!(after.contains("[Added Associations]\nimage/png=gimp.desktop;\n"));
        assert!(super::super::local_desktop_file(&env).is_file());
        assert!(mime_package_path(&env).is_file());
        let (state, targets) = status(&env);
        assert_eq!(state, State::On, "{targets:?}");
        assert_eq!(
            effective_default(&env, "image/png").as_deref(),
            Some("app.specular.Specular.desktop")
        );
        // A backup of the original was made.
        let backups: Vec<_> = fs::read_dir(env.record_dir().join("backups"))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(fs::read_to_string(backups[0].path()).unwrap(), USER_LIST);

        // Idempotent: applying again changes nothing and keeps the first recorded values.
        apply(&env, &Labels::default()).unwrap();
        assert_eq!(fs::read_to_string(mimeapps_path(&env)).unwrap(), after);

        // The user picks another viewer for GIF meanwhile: removal leaves that alone.
        let mut kf = KeyFile::parse(&after);
        kf.set(DEFAULTS, "image/gif", "eog.desktop;");
        fs::write(mimeapps_path(&env), kf.to_text()).unwrap();
        assert_eq!(status(&env).0, State::Partial);

        remove(&env).unwrap();
        let restored = fs::read_to_string(mimeapps_path(&env)).unwrap();
        assert_eq!(
            restored,
            USER_LIST.replace(
                "text/plain=org.gnome.TextEditor.desktop\n",
                "text/plain=org.gnome.TextEditor.desktop\nimage/gif=eog.desktop;\n"
            )
        );
        assert!(!super::super::local_desktop_file(&env).exists());
        assert!(!mime_package_path(&env).exists());
        assert_eq!(status(&env).0, State::Off);
        // Removing twice is harmless.
        remove(&env).unwrap();
        assert_eq!(fs::read_to_string(mimeapps_path(&env)).unwrap(), restored);
        test_env::cleanup(&env);
    }

    /// Extra types the user ticks are claimed like the image types; unticking one gives back
    /// only that entry (and only while it is still ours), leaving everything else.
    #[test]
    fn extra_types_are_claimed_and_given_back() {
        let env = setup("viewer-extra");
        let mut kf = KeyFile::parse(USER_LIST);
        kf.set(DEFAULTS, "application/x-krita", "org.kde.krita.desktop;");
        kf.set(DEFAULTS, "model/stl", "org.freecad.FreeCAD.desktop;");
        let original = kf.to_text();
        fs::write(mimeapps_path(&env), &original).unwrap();
        let base = status(&env).1;

        let env = env.with_extra_default_types("application/x-krita,model/stl,application/dicom");
        apply(&env, &Labels::default()).unwrap();
        let text = fs::read_to_string(mimeapps_path(&env)).unwrap();
        assert!(text.contains(
            "application/x-krita=app.specular.Specular.desktop;org.kde.krita.desktop;\n"
        ));
        assert!(
            text.contains("model/stl=app.specular.Specular.desktop;org.freecad.FreeCAD.desktop;\n")
        );
        assert!(text.contains("application/dicom=app.specular.Specular.desktop;\n"));
        // Not ticked: untouched.
        assert!(!text.contains("image/svg+xml="));
        let (_, targets) = status(&env);
        let n = |t: &[String]| -> usize { t[0].split('/').nth(1).unwrap().parse().unwrap() };
        assert_eq!(n(&targets), n(&base) + 3, "{targets:?} vs {base:?}");

        // The user picks FreeCAD again for STL by hand; then unticks STL and DICOM.
        let mut kf = KeyFile::parse(&text);
        kf.set(DEFAULTS, "model/stl", "org.freecad.FreeCAD.desktop;");
        fs::write(mimeapps_path(&env), kf.to_text()).unwrap();
        let env = env.with_extra_default_types("application/x-krita");
        apply(&env, &Labels::default()).unwrap();
        let text = fs::read_to_string(mimeapps_path(&env)).unwrap();
        assert!(text.contains(
            "application/x-krita=app.specular.Specular.desktop;org.kde.krita.desktop;\n"
        ));
        assert!(text.contains("model/stl=org.freecad.FreeCAD.desktop;\n"));
        assert!(
            !text.contains("application/dicom="),
            "ours, with nothing before: removed"
        );
        assert_eq!(status(&env).0, State::On);

        // Ticking DICOM again records afresh; turning the option off restores the file.
        let env = env.with_extra_default_types("application/x-krita,application/dicom");
        apply(&env, &Labels::default()).unwrap();
        assert!(
            fs::read_to_string(mimeapps_path(&env))
                .unwrap()
                .contains("application/dicom=app.specular.Specular.desktop;\n")
        );
        remove(&env).unwrap();
        assert_eq!(fs::read_to_string(mimeapps_path(&env)).unwrap(), original);
        test_env::cleanup(&env);
    }

    #[test]
    fn missing_file_is_created_and_removed_cleanly() {
        let env = test_env::env("viewer-new");
        apply(&env, &Labels::default()).unwrap();
        let text = fs::read_to_string(mimeapps_path(&env)).unwrap();
        assert!(text.starts_with("[Default Applications]\n"));
        assert!(text.contains("\nimage/jpeg=app.specular.Specular.desktop;\n"));
        assert_eq!(status(&env).0, State::On);
        remove(&env).unwrap();
        assert_eq!(
            fs::read_to_string(mimeapps_path(&env)).unwrap(),
            "[Default Applications]\n"
        );
        test_env::cleanup(&env);
    }

    #[test]
    fn binary_file_is_left_alone() {
        let env = test_env::env("viewer-binary");
        fs::create_dir_all(&env.config_home).unwrap();
        fs::write(mimeapps_path(&env), b"\xff\xfe[Default").unwrap();
        assert!(matches!(
            apply(&env, &Labels::default()),
            Err(Error::Unrecognized(..))
        ));
        assert_eq!(fs::read(mimeapps_path(&env)).unwrap(), b"\xff\xfe[Default");
        test_env::cleanup(&env);
    }
}
