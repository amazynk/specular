//! Freedesktop thumbnailer: `~/.local/share/thumbnailers/specular.thumbnailer` running this
//! very program (`specular --thumbnail %i %o %s`), for the image types no other installed
//! thumbnailer handles (JPEG XL, AVIF, HEIC, raw, JPEG 2000, EXR, PSD… depending on the
//! system). GNOME's, Cinnamon's and MATE's file managers read user thumbnailers directly;
//! Thunar reads them through tumbler's desktop-thumbnailer plugin.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use super::keyfile::{KeyFile, split_list};
use super::{MARKER, Record, formats, install, is_ours, keyfile_escape, shell_quote, uninstall};
use crate::integration::{Env, OptionId, Result, State};

fn path(env: &Env) -> PathBuf {
    env.data_home.join("thumbnailers/specular.thumbnailer")
}

/// MIME types other thumbnailers (installed for the user or system-wide) already handle.
pub fn covered(env: &Env) -> HashSet<String> {
    let mine = path(env);
    let mut out = HashSet::new();
    for dir in std::iter::once(&env.data_home).chain(&env.data_dirs) {
        let Ok(rd) = fs::read_dir(dir.join("thumbnailers")) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p == mine || p.extension().is_none_or(|x| x != "thumbnailer") || is_ours(&p) {
                continue;
            }
            let Ok(text) = fs::read_to_string(&p) else {
                continue;
            };
            let kf = KeyFile::parse(&text);
            if let Some(v) = kf.get("Thumbnailer Entry", "MimeType") {
                out.extend(split_list(v).into_iter().map(str::to_owned));
            }
        }
    }
    out
}

/// The types we would thumbnail here.
pub fn wanted(env: &Env) -> Vec<&'static str> {
    let covered = covered(env);
    formats::mime_types()
        .filter(|m| !covered.contains(*m))
        .collect()
}

fn entry(env: &Env, types: &[&str]) -> String {
    format!(
        "# {MARKER}: installed by Specular (Settings > Integration > Thumbnailer); removed with it.\n\
         [Thumbnailer Entry]\n\
         TryExec={}\n\
         Exec={} --thumbnail %i %o %s\n\
         MimeType={}\n",
        keyfile_escape(&env.exe.to_string_lossy()),
        shell_quote(&env.exe),
        formats::mime_list(types)
    )
}

pub(super) fn apply(env: &Env) -> Result<()> {
    let types = wanted(env);
    let mut rec = Record::load(env, OptionId::Thumbnailer.key()).unwrap_or_default();
    install(&mut rec, &path(env), &entry(env, &types), 0o644)?;
    rec.save(env, OptionId::Thumbnailer.key())
}

pub(super) fn remove(env: &Env) -> Result<()> {
    let mut rec = Record::load(env, OptionId::Thumbnailer.key()).unwrap_or_default();
    rec.add_file(&path(env));
    uninstall(&rec);
    Record::delete(env, OptionId::Thumbnailer.key());
    Ok(())
}

pub(super) fn status(env: &Env) -> (State, Vec<String>) {
    let p = path(env);
    if !is_ours(&p) {
        return (State::Off, Vec::new());
    }
    let text = fs::read_to_string(&p).unwrap_or_default();
    let kf = KeyFile::parse(&text);
    let n = kf
        .get("Thumbnailer Entry", "MimeType")
        .map_or(0, |v| split_list(v).len());
    // It must still run this program (an AppImage that moved needs applying again).
    let exe_ok = kf.get("Thumbnailer Entry", "TryExec").map(PathBuf::from) == Some(env.exe.clone())
        && env.exe.is_file();
    let state = if exe_ok { State::On } else { State::Partial };
    (state, vec![n.to_string()])
}

#[cfg(test)]
mod tests {
    use super::super::test_env;
    use super::*;

    #[test]
    fn only_uncovered_types() {
        let env = test_env::env("thumb");
        let sys = env.data_dirs[0].join("thumbnailers");
        fs::create_dir_all(&sys).unwrap();
        fs::write(
            sys.join("gdk-pixbuf-thumbnailer.thumbnailer"),
            "[Thumbnailer Entry]\nExec=gdk-pixbuf-thumbnailer -s %s %u %o\n\
             MimeType=image/jpeg;image/png;image/gif;image/webp;image/tiff;image/bmp;\n",
        )
        .unwrap();
        let w = wanted(&env);
        assert!(w.contains(&"image/jxl") && w.contains(&"image/x-canon-cr3"));
        assert!(!w.contains(&"image/jpeg") && !w.contains(&"image/png"));
        assert_eq!(status(&env).0, State::Off);

        apply(&env).unwrap();
        let text = fs::read_to_string(path(&env)).unwrap();
        assert!(text.contains("Exec='"), "spaced path quoted: {text}");
        assert!(text.contains("/bin dir/specular' --thumbnail %i %o %s\n"));
        assert!(!text.contains("image/jpeg;"));
        let (state, n) = status(&env);
        assert_eq!(state, State::On);
        assert_eq!(n, vec![w.len().to_string()]);
        // Our own file does not count as "covered" when applying again.
        apply(&env).unwrap();
        assert_eq!(fs::read_to_string(path(&env)).unwrap(), text);

        remove(&env).unwrap();
        assert!(!path(&env).exists());
        assert_eq!(status(&env).0, State::Off);
        test_env::cleanup(&env);
    }
}
