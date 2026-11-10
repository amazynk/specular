//! Recent files on Windows: images viewed in Specular go into the user's recent items
//! (`SHAddToRecentDocs`): File Explorer's "Recent" (Home, Quick access), the Start menu's
//! recommendations and, for types whose ProgID names Specular (the installer's associations),
//! the "Recent" list of Specular's taskbar jump list.
//!
//! Applying only records that the option is on. Each viewed file is then handed to the shell
//! (which adds a shortcut to the file and one to its folder) and kept in the record (newest
//! last, at most [`MAX_FILES`]). Removing deletes the recent-items shortcuts that point at the
//! recorded files or their folders (found by the target inside each `.lnk`: their names drop
//! the extension when Explorer hides known ones, and get numbered on clashes), clears
//! Specular's jump list
//! (`IApplicationDestinations::RemoveAllDestinations`) and deletes the record. The recent items
//! folder is shared with every program and File Explorer adds its own shortcuts, so a file or
//! folder opened from Explorer as well loses its shortcut there too.

use std::fs;
use std::path::{Path, PathBuf};

use specular_log::{SETTINGS, info};

use crate::integration::{Env, Error, OptionId, Result, State};

/// Files kept in the record, at most (the oldest go first).
const MAX_FILES: usize = 200;

/// The record: one viewed file per line. Its presence means the option is on.
fn record_path(env: &Env) -> PathBuf {
    env.record_dir()
        .join(format!("{}.txt", OptionId::RecentFiles.key()))
}

/// The user's recent items folder (`FOLDERID_Recent`) under `env`'s roaming application data.
fn recent_folder(env: &Env) -> PathBuf {
    env.data_home
        .join("Microsoft")
        .join("Windows")
        .join("Recent")
}

/// Whether the option is on.
pub fn enabled(env: &Env) -> bool {
    record_path(env).is_file()
}

fn load(env: &Env) -> Vec<PathBuf> {
    fs::read_to_string(record_path(env))
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn save(env: &Env, files: &[PathBuf]) -> Result<()> {
    let path = record_path(env);
    let dir = env.record_dir();
    fs::create_dir_all(&dir).map_err(|e| Error::Io(dir.clone(), e))?;
    let mut text = String::new();
    for f in files {
        // A path with a line break cannot be recorded (nor easily typed): left out.
        if let Some(s) = f.to_str().filter(|s| !s.contains(['\n', '\r'])) {
            text.push_str(s);
            text.push('\n');
        }
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, text).map_err(|e| Error::Io(tmp.clone(), e))?;
    fs::rename(&tmp, &path).map_err(|e| Error::Io(path.clone(), e))
}

pub(crate) fn apply(env: &Env) -> Result<()> {
    if !enabled(env) {
        save(env, &[])?;
    }
    Ok(())
}

pub(crate) fn remove(env: &Env) -> Result<()> {
    let files = load(env);
    // Paths compare without case, as Windows does.
    let ours: Vec<String> = files
        .iter()
        .map(PathBuf::as_path)
        .chain(files.iter().filter_map(|f| f.parent()))
        .map(|p| p.to_string_lossy().to_lowercase())
        .collect();
    let mut removed = 0;
    let links = fs::read_dir(recent_folder(env))
        .into_iter()
        .flatten()
        .flatten();
    for link in links {
        let path = link.path();
        let is_link = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("lnk"));
        // Shell links are a few kilobytes; anything far larger is not one.
        let small = link.metadata().is_ok_and(|m| m.len() < 1 << 20);
        let target = (is_link && small)
            .then(|| fs::read(&path).ok())
            .flatten()
            .and_then(|b| link_target(&b));
        if target.is_some_and(|t| ours.contains(&t.to_string_lossy().to_lowercase()))
            && fs::remove_file(&path).is_ok()
        {
            removed += 1;
        }
    }
    if env.run_tools {
        shell::clear_jump_list();
    }
    match fs::remove_file(record_path(env)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            return Err(Error::Io(record_path(env), e));
        }
        _ => {}
    }
    info!(target: SETTINGS, "integration: recent files off ({removed} recent items removed)");
    Ok(())
}

/// The local file or folder a shell link (`.lnk`, MS-SHLLINK) points at, from its LinkInfo:
/// the local base path and the common path suffix, in UTF-16 when the link has them, else in
/// the ANSI code page (taken only when ASCII). None for links without a local path.
fn link_target(b: &[u8]) -> Option<PathBuf> {
    const HAS_ID_LIST: usize = 0x1;
    const HAS_LINK_INFO: usize = 0x2;
    const VOLUME_ID_AND_LOCAL_BASE_PATH: usize = 0x1;
    let u16_at = |b: &[u8], o: usize| Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?));
    let u32_at =
        |b: &[u8], o: usize| Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?) as usize);
    if u32_at(b, 0)? != 0x4C {
        return None;
    }
    let flags = u32_at(b, 0x14)?;
    let mut at = 0x4C;
    if flags & HAS_ID_LIST != 0 {
        at += 2 + usize::from(u16_at(b, at)?);
    }
    if flags & HAS_LINK_INFO == 0 {
        return None;
    }
    let info = b.get(at..at + u32_at(b, at)?)?;
    let header = u32_at(info, 4)?;
    if u32_at(info, 8)? & VOLUME_ID_AND_LOCAL_BASE_PATH == 0 {
        return None;
    }
    let wide = |o: usize| {
        let units: Vec<u16> = info
            .get(o..)?
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&u| u != 0)
            .collect();
        String::from_utf16(&units).ok()
    };
    let narrow = |o: usize| {
        let bytes: Vec<u8> = info
            .get(o..)?
            .iter()
            .copied()
            .take_while(|&c| c != 0)
            .collect();
        bytes
            .is_ascii()
            .then(|| String::from_utf8(bytes).ok())
            .flatten()
    };
    let (base, suffix) = if header >= 0x24 {
        (wide(u32_at(info, 0x1C)?)?, wide(u32_at(info, 0x20)?)?)
    } else {
        (narrow(u32_at(info, 0x10)?)?, narrow(u32_at(info, 0x18)?)?)
    };
    Some(PathBuf::from(base + &suffix))
}

pub(crate) fn status(env: &Env) -> (State, Vec<String>) {
    if !enabled(env) {
        return (State::Off, Vec::new());
    }
    (State::On, vec![load(env).len().to_string()])
}

/// Adds `file` to the recent items, when the option is on. Blocks briefly (the shell writes a
/// shortcut); call off the UI thread.
pub fn note(env: &Env, file: &Path) -> Result<()> {
    if !enabled(env) {
        return Ok(());
    }
    if env.run_tools {
        shell::add(file);
    }
    let mut files = load(env);
    files.retain(|f| f != file);
    files.push(file.to_path_buf());
    let over = files.len().saturating_sub(MAX_FILES);
    files.drain(..over);
    save(env, &files)
}

/// The shell calls, each `unsafe` with its reasoning (UNSAFE.md).
mod shell {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use specular_log::{SETTINGS, warn};
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
    use windows::Win32::UI::Shell::{
        ApplicationDestinations, IApplicationDestinations, SHARD_PATHW, SHAddToRecentDocs,
    };

    use crate::share::windows::Com;

    pub(super) fn add(file: &Path) {
        let _com = Com::init();
        let wide: Vec<u16> = file
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        #[allow(unsafe_code)]
        // SAFETY: SHARD_PATHW takes a NUL-terminated UTF-16 path; `wide` ends in NUL and
        // outlives the call, which only reads it.
        unsafe {
            SHAddToRecentDocs(SHARD_PATHW.0 as u32, Some(wide.as_ptr().cast()));
        }
    }

    /// Empties the "Recent" and "Frequent" lists of Specular's jump list.
    pub(super) fn clear_jump_list() {
        let _com = Com::init();
        #[allow(unsafe_code)]
        // SAFETY: creates the shell's in-process ApplicationDestinations object on this thread,
        // where COM was just initialised; the interface is released on drop.
        let d: windows::core::Result<IApplicationDestinations> =
            unsafe { CoCreateInstance(&ApplicationDestinations, None, CLSCTX_INPROC_SERVER) };
        let r = d.and_then(|d| {
            #[allow(unsafe_code)]
            // SAFETY: a method without arguments on the live interface; for this process's
            // application id, the one `SHAddToRecentDocs` filed the items under.
            unsafe {
                d.RemoveAllDestinations()
            }
        });
        if let Err(e) = r {
            warn!(target: SETTINGS, "recent files: clearing the jump list: {}", e.message());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shell link to `target` as the shell writes one for a local file: header, LinkInfo with
    /// the ANSI and UTF-16 paths (no ID list: optional, and not read).
    fn lnk(target: &Path) -> Vec<u8> {
        let path = target.to_string_lossy();
        let ansi: Vec<u8> = path
            .bytes()
            .map(|c| if c.is_ascii() { c } else { b'?' })
            .collect();
        let wide: Vec<u8> = path.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let header = 0x24usize;
        let base = header;
        let suffix = base + ansi.len() + 1;
        let base_w = suffix + 1;
        let suffix_w = base_w + wide.len() + 2;
        let size = suffix_w + 2;
        let mut info = Vec::new();
        for v in [size, header, 1, 0, base, 0, suffix, base_w, suffix_w] {
            info.extend_from_slice(&(v as u32).to_le_bytes());
        }
        info.extend_from_slice(&ansi);
        info.extend_from_slice(&[0, 0]);
        info.extend_from_slice(&wide);
        info.extend_from_slice(&[0, 0, 0, 0]);
        let mut b = vec![0u8; 0x4C];
        b[0] = 0x4C;
        b[0x14] = 0x02 | 0x80;
        b.extend_from_slice(&info);
        b
    }

    #[test]
    fn reads_link_targets() {
        let t = Path::new("C:\\Users\\Me\\Bilder \u{fc}\\\u{7167}\u{7247}.jpg");
        assert_eq!(link_target(&lnk(t)).as_deref(), Some(t));
        assert_eq!(link_target(b"not a link"), None);
        let mut cut = lnk(t);
        cut.truncate(0x60);
        assert_eq!(link_target(&cut), None, "truncated");
    }

    fn env(name: &str) -> (Env, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("specular-recent-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let env = Env::under(&root, Path::new("specular.exe"));
        (env, root)
    }

    #[test]
    fn notes_only_while_on_and_cleans_up() {
        let (env, root) = env("cycle");
        let a = root.join("photos").join("a.jpg");
        note(&env, &a).unwrap();
        assert_eq!(status(&env).0, State::Off);
        assert!(!record_path(&env).exists(), "nothing kept while off");

        apply(&env).unwrap();
        assert_eq!(status(&env), (State::On, vec!["0".to_owned()]));
        let b = root.join("photos").join("b.png");
        note(&env, &a).unwrap();
        note(&env, &b).unwrap();
        note(&env, &a).unwrap();
        assert_eq!(
            load(&env),
            vec![b.clone(), a.clone()],
            "newest last, once each"
        );
        apply(&env).unwrap();
        assert_eq!(load(&env).len(), 2, "applying again keeps the list");

        // The shell's shortcuts (made by hand here: run_tools is off), named as Explorer
        // names them with and without known extensions, and one of another program's.
        let folder = recent_folder(&env);
        fs::create_dir_all(&folder).unwrap();
        let other = root.join("docs").join("other.docx");
        let links = [
            ("a.jpg.lnk", a.clone()),
            ("b.lnk", b.clone()),
            ("photos.lnk", root.join("PHOTOS")),
            ("other.lnk", other),
        ];
        for (n, t) in &links {
            fs::write(folder.join(n), lnk(t)).unwrap();
        }
        fs::write(folder.join("broken.lnk"), b"").unwrap();
        remove(&env).unwrap();
        assert_eq!(status(&env).0, State::Off);
        assert!(!folder.join("a.jpg.lnk").exists() && !folder.join("b.lnk").exists());
        assert!(
            !folder.join("photos.lnk").exists(),
            "the folder's shortcut too, any case"
        );
        assert!(
            folder.join("other.lnk").exists(),
            "other programs' items stay"
        );
        assert!(folder.join("broken.lnk").exists(), "unreadable links stay");
        remove(&env).unwrap();
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn keeps_the_newest() {
        let (env, root) = env("bounded");
        apply(&env).unwrap();
        for i in 0..MAX_FILES + 5 {
            note(&env, &root.join(format!("{i}.jpg"))).unwrap();
        }
        let files = load(&env);
        assert_eq!(files.len(), MAX_FILES);
        assert_eq!(files[0], root.join("5.jpg"));
        let _ = fs::remove_dir_all(&root);
    }

    /// The real shell: a viewed file gets a shortcut in this user's recent items, and turning
    /// the option off takes it away again. Writes to the real recent items folder (and leaves
    /// it as it was), so it only runs when asked: `cargo test -p specular-platform -- --ignored
    /// real_recent_items`. Needs "Show recently opened items" on (Settings → Personalization →
    /// Start).
    #[test]
    #[ignore = "writes to this user's recent items"]
    fn real_recent_items() {
        let (mut env, _) = env("real");
        env.run_tools = true;
        env.data_home = Env::detect().data_home;
        // A plain absolute path (no `..`), as the viewer passes; not under %TEMP%, which the
        // shell may leave out.
        // A new name each run: the shell keeps its own list of recent names and does not make
        // a shortcut again for one it has seen.
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let name = format!("specular-recent-{stamp}");
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(&name);
        fs::create_dir_all(&root).unwrap();
        let root = crate::path::canonicalize(&root).unwrap();
        let file = root.join(format!("{name}.png"));
        fs::write(&file, b"x").unwrap();
        // The shortcuts pointing at the file and at its folder, whatever their names.
        let pointing_at = |target: &Path| {
            fs::read_dir(recent_folder(&env))
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .find(|p| fs::read(p).ok().and_then(|b| link_target(&b)).as_deref() == Some(target))
        };
        apply(&env).unwrap();
        note(&env, &file).unwrap();
        let appeared = (0..50).any(|_| {
            std::thread::sleep(std::time::Duration::from_millis(100));
            pointing_at(&file).is_some()
        });
        let folder_link = pointing_at(&root);
        remove(&env).unwrap();
        let left = (pointing_at(&file), pointing_at(&root));
        let _ = fs::remove_dir_all(&root);
        assert!(appeared, "no shortcut to {}", file.display());
        assert!(folder_link.is_some(), "no shortcut to {}", root.display());
        assert_eq!(left, (None, None), "removing left them");
    }
}
