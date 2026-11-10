//! "Open with…": the applications the OS associates with a file's type, and launching one of
//! them with the file (DESIGN.md §11, "Integration and automation").
//!
//! * Linux and the BSDs: freedesktop desktop entries and `mimeapps.list`, parsed in pure Rust
//!   (`desktop.rs`).
//! * Windows: `SHAssocEnumHandlers` + `IAssocHandler::Invoke` (`windows.rs`).
//! * macOS: `NSWorkspace` `URLsForApplicationsToOpenURL:` (`macos.rs`), launched with `open -a`.
//!
//! [`handlers_for`] reads files or asks the OS and can take tens of milliseconds the first
//! time: call it off the UI thread.

use std::io;
use std::path::{Path, PathBuf};

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) mod desktop;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

/// An application that can open the file.
#[derive(Clone, Debug, PartialEq)]
pub struct Handler {
    /// What to show in the menu ("GIMP", "Photos").
    pub name: String,
    /// Stable identity: desktop file ID, executable path or app bundle path.
    pub id: String,
    /// The OS default for this type.
    pub is_default: bool,
    how: How,
}

#[derive(Clone, Debug, PartialEq)]
enum How {
    /// Freedesktop: the parsed entry (its `Exec` line, `Path`, icon).
    #[cfg(all(unix, not(target_os = "macos")))]
    Desktop(Box<desktop::DesktopEntry>),
    /// macOS: the app bundle.
    #[cfg(target_os = "macos")]
    Bundle(PathBuf),
    /// Windows: the handler's name (`IAssocHandler::GetName`), found again at launch.
    #[cfg(windows)]
    Assoc,
    #[allow(dead_code)]
    None(PathBuf),
}

/// Applications for `path`, the default first. `ui_lang` (a BCP-47 tag like `de` or
/// `zh-Hans`) picks localised names where the OS stores several.
pub fn handlers_for(path: &Path, ui_lang: &str) -> Vec<Handler> {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let data = desktop::data_dirs();
        let config = desktop::config_dirs();
        let desktops = desktop::current_desktops();
        let posix = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .find_map(|v| std::env::var(v).ok().filter(|s| !s.is_empty()));
        let langs = desktop::locale_candidates(ui_lang, posix.as_deref());
        let apps = desktop::scan_apps(&data, &langs);
        let lists = desktop::load_mimeapps(&config, &data, &desktops);
        let db = desktop::MimeDb::load(&data);
        let types = db.types_for(path);
        desktop::resolve(&types, &apps, &lists, &db, &desktops)
            .into_iter()
            .map(|(e, is_default)| Handler {
                name: e.name.clone(),
                id: e.id.clone(),
                is_default,
                how: How::Desktop(Box::new(e.clone())),
            })
            .collect()
    }
    #[cfg(target_os = "macos")]
    {
        let _ = ui_lang;
        macos::handlers_for(path)
    }
    #[cfg(windows)]
    {
        let _ = ui_lang;
        windows::handlers_for(path)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (path, ui_lang);
        Vec::new()
    }
}

/// Opens `path` with `h`. Returns once the program has started (or failed to).
pub fn launch(h: &Handler, path: &Path) -> io::Result<()> {
    match &h.how {
        #[cfg(all(unix, not(target_os = "macos")))]
        How::Desktop(e) => {
            let argv = desktop::expand_exec(e, path)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "bad Exec line"))?;
            let (prog, args) = argv
                .split_first()
                .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
            let cwd = e.path.as_deref().map(Path::new);
            crate::launch::spawn(prog, args, cwd, None)
        }
        #[cfg(target_os = "macos")]
        How::Bundle(app) => {
            let args = [
                "-a".into(),
                app.as_os_str().to_owned(),
                path.as_os_str().to_owned(),
            ];
            crate::launch::spawn("/usr/bin/open".as_ref(), &args, None, None)
        }
        #[cfg(windows)]
        How::Assoc => windows::launch(&h.id, path),
        How::None(_) => Err(io::ErrorKind::Unsupported.into()),
    }
}

/// Asks the OS to let the user pick any application ("Choose another app…"). Only Windows has
/// a system dialog for that; elsewhere this returns [`io::ErrorKind::Unsupported`].
pub fn choose_other(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        // The documented entry point of the "Open with" dialog; no shell, argv only.
        let mut arg = std::ffi::OsString::from("shell32.dll,OpenAs_RunDLL ");
        arg.push(path.as_os_str());
        crate::launch::spawn("rundll32.exe".as_ref(), &[arg], None, None)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err(io::ErrorKind::Unsupported.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listing_never_includes_specular_and_survives_odd_paths() {
        for p in ["/nonexistent/x.jpg", "noext", "/"] {
            let hs = handlers_for(Path::new(p), "en");
            assert!(
                hs.iter()
                    .all(|h| !h.id.to_ascii_lowercase().starts_with("app.specular."))
            );
            assert!(hs.iter().filter(|h| h.is_default).count() <= 1);
        }
    }
}
