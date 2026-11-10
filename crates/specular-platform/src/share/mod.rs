//! Sharing files with other applications: the system's share sheet, a new email with the
//! files attached, and (Windows, macOS) setting the desktop picture.
//!
//! | | Share | Send by email |
//! |---|---|---|
//! | Linux | the OpenURI portal's application chooser (`OpenFile`, `ask`) | `xdg-email --attach` |
//! | Windows | `DataTransferManager` (the Share flyout) for the window | not offered |
//! | macOS | `NSSharingServicePicker` under the window's view | the Mail compose service |
//!
//! Linux calls block (D-Bus, a helper program) and belong on a background thread; the
//! Windows and macOS share sheets need the UI thread and the window.

use std::io;
use std::path::{Path, PathBuf};

#[cfg(all(unix, not(target_os = "macos")))]
mod linux;
#[cfg(target_os = "macos")]
pub(crate) mod macos;
#[cfg(windows)]
pub(crate) mod windows;

/// Whether "Share…" can work here (a cheap guess; the call itself decides).
pub fn share_available() -> bool {
    #[cfg(all(unix, not(target_os = "macos")))]
    return linux::portal_available();
    #[cfg(any(windows, target_os = "macos"))]
    return true;
    #[cfg(not(any(unix, windows)))]
    return false;
}

/// Whether "Send by email" can work here.
pub fn email_available() -> bool {
    #[cfg(all(unix, not(target_os = "macos")))]
    return crate::launch::find_program("xdg-email").is_some();
    #[cfg(target_os = "macos")]
    return true;
    #[cfg(not(unix))]
    return false;
}

/// Linux: lets the user pick an application to open `file` with, through the OpenURI
/// portal's chooser (works in sandboxes). Blocks until the portal answers.
#[cfg(all(unix, not(target_os = "macos")))]
pub fn open_with_chooser(file: &Path) -> io::Result<()> {
    linux::open_file(file)
}

/// Windows and macOS: the system share sheet for `files`, attached to `window`. Call on the
/// UI thread.
#[cfg(not(all(unix, not(target_os = "macos"))))]
pub fn share(files: &[PathBuf], window: &dyn raw_window_handle::HasWindowHandle) -> io::Result<()> {
    let files = absolute(files)?;
    #[cfg(windows)]
    return windows::share(&files, window);
    #[cfg(target_os = "macos")]
    return macos::share(&files, window);
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (files, window);
        Err(io::ErrorKind::Unsupported.into())
    }
}

/// Opens a new message in the user's mail program with `files` attached. Blocks (Linux:
/// until `xdg-email` has handed it over); call off the UI thread.
pub fn send_by_email(files: &[PathBuf]) -> io::Result<()> {
    let files = absolute(files)?;
    #[cfg(all(unix, not(target_os = "macos")))]
    return linux::email(&files);
    #[cfg(target_os = "macos")]
    return macos::email(&files);
    #[cfg(not(unix))]
    {
        let _ = files;
        Err(io::ErrorKind::Unsupported.into())
    }
}

/// macOS: the application Launch Services has for `bundle_id` (an install, or a build that
/// has been run once).
#[cfg(target_os = "macos")]
pub fn app_for_bundle_id(bundle_id: &str) -> Option<PathBuf> {
    macos::app_for_bundle_id(bundle_id)
}

/// macOS: None when no application with `bundle_id` runs; otherwise whether one has finished
/// launching.
#[cfg(target_os = "macos")]
pub fn app_running(bundle_id: &str) -> Option<bool> {
    macos::app_running(bundle_id)
}

/// macOS: hands `files` to the Service whose menu item is `name` (another application's
/// "Services" entry). Blocks until the provider has taken them; call off the UI thread.
#[cfg(target_os = "macos")]
pub fn perform_service(name: &str, files: &[PathBuf]) -> io::Result<()> {
    let files = absolute(files)?;
    macos::perform_service(name, &files)
}

/// Windows: lets the application Specular hands files to next come to the front with them.
/// Windows keeps other applications from taking the foreground unless the one in front
/// allows it.
#[cfg(windows)]
pub fn allow_foreground_handoff() {
    windows::allow_foreground_handoff();
}

/// The files as absolute paths that exist (a helper or another process gets them).
fn absolute(files: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
    if files.is_empty() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    files
        .iter()
        .map(|f| {
            let p = std::path::absolute(f)?;
            if p.is_file() {
                Ok(p)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    p.display().to_string(),
                ))
            }
        })
        .collect()
}

/// The arguments `xdg-email` gets for `files`: one `--attach` each.
pub fn email_args(files: &[&Path]) -> Vec<std::ffi::OsString> {
    let mut v = Vec::new();
    for f in files {
        v.push("--attach".into());
        v.push(f.as_os_str().to_owned());
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_attaches_every_file() {
        let a = Path::new("/p/a b.jpg");
        let b = Path::new("/p/c.png");
        let args = email_args(&[a, b]);
        assert_eq!(
            args,
            ["--attach", "/p/a b.jpg", "--attach", "/p/c.png"]
                .map(std::ffi::OsString::from)
                .to_vec()
        );
        assert!(absolute(&[]).is_err());
        assert_eq!(
            absolute(&[PathBuf::from("/no/such/file.jpg")])
                .unwrap_err()
                .kind(),
            io::ErrorKind::NotFound
        );
    }
}
