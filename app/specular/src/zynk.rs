//! Hands files to the Zynk desktop app, a separate file-sending product. "Send with Zynk"
//! gives them to Zynk's picker of contacts and the user's devices; "Share with Zynk" opens
//! Zynk's web share form with them, which makes a web link anyone can download them from.
//!
//! | | Send | Share |
//! |---|---|---|
//! | macOS | the app's "Send with Zynk" Service | its "Share Link with Zynk" Service |
//! | Windows | the app's `ZynkShare` pipe, or `Zynk.exe --share` | its `ZynkWebShare` pipe, or `Zynk.exe --web-share` |
//! | Linux | `zynk --share` | `zynk --web-share` |
//!
//! On macOS the files go through Services because Zynk is sandboxed: it can read files handed
//! over that way and no others. On Windows the pipes reach a running Zynk; the app is started
//! when they are not there (and a second start passes the files on to a running one). The
//! Store's Zynk takes nothing on its command line: it is started bare, and the files go down
//! its pipe once it is up. On Linux Zynk is one instance per user: starting it again passes
//! the files on, with the activation token its window comes forward with.
//!
//! `SPECULAR_ZYNK_APP` names the app to use (`Zynk.app` on macOS, `Zynk.exe` on Windows, the
//! program or AppImage on Linux), for development builds and installs found nowhere else.
//! Without Zynk, the app says what it is and offers [`DOWNLOAD_URL`].
//!
//! [`available`], [`running`] and [`hand_over`] look on disk, and [`hand_over`] blocks (it may
//! start the app and wait for it): callers run them on a background thread.

use std::io;
use std::path::PathBuf;
#[cfg(not(target_os = "macos"))]
use std::process::{Command, Stdio};
#[cfg(not(target_os = "macos"))]
use std::time::{Duration, Instant};

/// What the files are handed to Zynk for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// To a contact or one of the user's devices, picked in Zynk.
    Send,
    /// A web link to the files, made in Zynk's web share form.
    Share,
}

/// Why the Zynk desktop app here can't take files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unavailable {
    /// It is not installed, or not anywhere it is looked for.
    NotFound,
    /// Windows before 10 version 2004, where Zynk takes no files to send.
    #[cfg(windows)]
    OldWindows,
}

/// The environment variable naming the desktop app to use.
const APP_ENV: &str = "SPECULAR_ZYNK_APP";

/// Where to get Zynk: its page in the Mac App Store.
#[cfg(target_os = "macos")]
pub const DOWNLOAD_URL: &str = "https://apps.apple.com/app/id1665078215";

/// Where to get Zynk: its page in the Microsoft Store.
#[cfg(windows)]
pub const DOWNLOAD_URL: &str = "https://apps.microsoft.com/store/detail/zynk/9PP9CNT87P3F";

/// Where to get Zynk: its download page, which has the AppImage.
#[cfg(not(any(target_os = "macos", windows)))]
pub const DOWNLOAD_URL: &str = "https://zynk.it/download/";

/// Whether the Zynk desktop app here can take files for MODE.
///
/// macOS: the app is installed, has been built and run (a development build), or is running.
/// Both modes are offered; a Zynk too old for web shares fails when asked.
#[cfg(target_os = "macos")]
pub fn available(_mode: Mode) -> Result<(), Unavailable> {
    let found = macos::app_path().is_some()
        || specular_platform::share::app_running(macos::BUNDLE_ID).is_some();
    found.then_some(()).ok_or(Unavailable::NotFound)
}

/// Whether the Zynk desktop app here can take files for MODE: it is installed (and on Windows,
/// for files to send, the system is new enough for Zynk to take them). A Zynk too old for
/// MODE fails when asked (Windows, for a web share) or ignores the files.
#[cfg(not(target_os = "macos"))]
pub fn available(mode: Mode) -> Result<(), Unavailable> {
    #[cfg(windows)]
    {
        if windows::app_exe().is_none() && windows::package_family().is_none() {
            return Err(Unavailable::NotFound);
        }
        if mode == Mode::Send && !windows::takes_sends() {
            return Err(Unavailable::OldWindows);
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = mode;
        linux::app_exe().map(|_| ()).ok_or(Unavailable::NotFound)
    }
}

/// Whether the Zynk app is up and can take files at once (no start to wait for).
#[cfg(target_os = "macos")]
pub fn running() -> bool {
    specular_platform::share::app_running(macos::BUNDLE_ID) == Some(true)
}

/// Whether the Zynk app is up and can take files at once: it runs, or is started with them
/// (not waited for). Only the Store's Zynk, which is waited for, may not be.
#[cfg(windows)]
pub fn running() -> bool {
    windows::takes_files_at_once()
}

/// Whether the Zynk app is up and can take files at once. Starting it here is quick (only its
/// first moment is waited for, in case it ends at once).
#[cfg(not(any(target_os = "macos", windows)))]
pub fn running() -> bool {
    true
}

/// Hands FILES to the Zynk desktop app for MODE. ACTIVATION is a token Zynk's window may come
/// to the front with (Linux; the others need none).
pub fn hand_over(mode: Mode, files: &[PathBuf], activation: Option<&str>) -> io::Result<()> {
    if files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "no files given",
        ));
    }
    #[cfg(any(target_os = "macos", windows))]
    let _ = activation;
    #[cfg(target_os = "macos")]
    return macos::hand_over(mode, files);
    #[cfg(windows)]
    return windows::hand_over(mode, files);
    #[cfg(not(any(target_os = "macos", windows)))]
    return linux::hand_over(mode, files, activation);
}

/// The flag the app's command line takes FILES after, for MODE.
#[cfg(not(target_os = "macos"))]
fn command_flag(mode: Mode) -> &'static str {
    match mode {
        Mode::Send => "--share",
        Mode::Share => "--web-share",
    }
}

/// How long a started app is watched for ending at once with a failure (it could not start,
/// or could not pass the files on), which is then the hand-over's error.
#[cfg(not(target_os = "macos"))]
const EARLY_EXIT: Duration = Duration::from_millis(1500);

/// Starts EXE with FILES for MODE, and ACTIVATION as the token its window comes forward with.
/// Only its first moment is waited for ([`EARLY_EXIT`]): a second start ends there, having
/// passed the files on, and the app itself runs on. Its output is dropped: Zynk keeps its own
/// log.
#[cfg(not(target_os = "macos"))]
fn launch(
    exe: &std::path::Path,
    mode: Mode,
    files: &[PathBuf],
    activation: Option<&str>,
) -> io::Result<()> {
    let mut command = Command::new(exe);
    specular_platform::launch::scrub_env(&mut command);
    command
        .arg(command_flag(mode))
        .args(files)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // The variables GTK reads the token from (Wayland, X11). One Specular was started with is
    // spent, so it is not passed on.
    for var in ["XDG_ACTIVATION_TOKEN", "DESKTOP_STARTUP_ID"] {
        match activation {
            Some(token) => command.env(var, token),
            None => command.env_remove(var),
        };
    }
    let mut child = command.spawn()?;
    let start = Instant::now();
    while start.elapsed() < EARLY_EXIT {
        if let Some(status) = child.try_wait()? {
            return if status.success() {
                Ok(())
            } else {
                Err(io::Error::other(format!(
                    "Zynk ended as it started ({status})"
                )))
            };
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    // Reaped once it ends, so it does not linger as a zombie.
    specular_thread::builder("zynk-child").spawn(move || {
        let _ = child.wait();
    })?;
    Ok(())
}

/// The path in the environment variable VAR, when it is set and not empty.
fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(target_os = "macos")]
mod macos {
    use std::io;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::{Duration, Instant};

    use specular_platform::share;

    use super::{APP_ENV, Mode, env_path};

    /// The app's bundle id.
    pub(super) const BUNDLE_ID: &str = "it.zynk";

    /// How long the app may take to start before handing over gives up.
    const LAUNCH_TIMEOUT: Duration = Duration::from_secs(30);

    /// How long the app is given after it starts to be ready for the files. A Zynk that holds
    /// files until its UI is up takes them at once; an older one drops what comes before.
    const SETTLE: Duration = Duration::from_secs(2);

    /// How often a starting app is checked on.
    const POLL_INTERVAL: Duration = Duration::from_millis(50);

    /// The pasteboard server, which keeps the list of Services.
    const PBS: &str = "/System/Library/CoreServices/pbs";

    /// The app's Service for MODE (`NSServices` in its Info.plist).
    fn service(mode: Mode) -> &'static str {
        match mode {
            Mode::Send => "Send with Zynk",
            Mode::Share => "Share Link with Zynk",
        }
    }

    /// A running Zynk gets the files; otherwise the app (`SPECULAR_ZYNK_APP` if set) is started
    /// first, so the system does not pick some other build that has the same bundle id.
    pub(super) fn hand_over(mode: Mode, files: &[PathBuf]) -> io::Result<()> {
        let running = share::app_running(BUNDLE_ID);
        if running.is_none() {
            let app = app_path()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Zynk is not installed"))?;
            let status = Command::new("/usr/bin/open")
                .arg("-a")
                .arg(&app)
                .stdin(Stdio::null())
                .status()?;
            if !status.success() {
                return Err(io::Error::other("Could not open Zynk"));
            }
        }
        if running != Some(true) {
            let start = Instant::now();
            while share::app_running(BUNDLE_ID) != Some(true) {
                if start.elapsed() > LAUNCH_TIMEOUT {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "Zynk did not start",
                    ));
                }
                thread::sleep(POLL_INTERVAL);
            }
            thread::sleep(SETTLE);
        }
        let name = service(mode);
        if share::perform_service(name, files).is_ok() {
            return Ok(());
        }
        // The system reads an app's Services when it is installed; a development build, or one
        // that has gained a Service since, may not be known yet. Have it look again, once.
        let _ = Command::new(PBS)
            .arg("-update")
            .stdin(Stdio::null())
            .status();
        share::perform_service(name, files).map_err(|_| {
            io::Error::other(format!(
                "this Zynk has no \u{201c}{name}\u{201d} service; update Zynk"
            ))
        })
    }

    /// `SPECULAR_ZYNK_APP` when set, else an install in an Applications folder, else what
    /// Launch Services has for the bundle id (a development build that has been run).
    pub(super) fn app_path() -> Option<PathBuf> {
        if let Some(app) = env_path(APP_ENV) {
            return app.exists().then_some(app);
        }
        let home = std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Applications"));
        [Some(PathBuf::from("/Applications")), home]
            .into_iter()
            .flatten()
            .map(|dir| dir.join("Zynk.app"))
            .find(|app| app.exists())
            .or_else(|| share::app_for_bundle_id(BUNDLE_ID))
    }
}

#[cfg(windows)]
mod windows {
    use std::fs::File;
    use std::io::{self, Write};
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::{Duration, Instant};

    use super::{APP_ENV, Mode, env_path, launch};

    /// The Store package's name; its family name adds the publisher's hash after a `_`.
    const PACKAGE_NAME: &str = "GlobalZoneLimited.Zynk";

    /// The app's id in its package (the Flutter project's name, as the msix tool sets it).
    const PACKAGE_APP_ID: &str = "zynk";

    /// How long the Store's app may take to start before handing over gives up.
    const LAUNCH_TIMEOUT: Duration = Duration::from_secs(30);

    /// How long a started app's pipe for web shares may lag its pipe for files to send before
    /// the app counts as too old to have it.
    const WEB_SHARE_GRACE: Duration = Duration::from_secs(3);

    /// How often a starting app is checked on.
    const POLL_INTERVAL: Duration = Duration::from_millis(100);

    /// The named pipe a running app takes files to send on.
    const SEND_PIPE: &str = r"\\.\pipe\ZynkShare";

    /// The named pipe a running app takes files to share as a web link on.
    const WEB_SHARE_PIPE: &str = r"\\.\pipe\ZynkWebShare";

    /// How long a running app may be without a pipe between two of its instances (it makes
    /// the next once one has been read).
    const PIPE_GAP: Duration = Duration::from_millis(250);

    /// Writes the paths to the app's pipe for MODE, or starts the app with them when that is
    /// not there (not running, or busy: a second start waits for the pipe and passes them on).
    /// The app then asks who gets them, or opens its web share form.
    pub(super) fn hand_over(mode: Mode, files: &[PathBuf]) -> io::Result<()> {
        // The app comes forward with them, which Windows lets it do only when the app in
        // front (this one) allows.
        specular_platform::share::allow_foreground_handoff();
        let name = match mode {
            Mode::Send => SEND_PIPE,
            Mode::Share => WEB_SHARE_PIPE,
        };
        let pipe = match open_pipe(name) {
            Ok(pipe) => pipe,
            // Zynk runs (its pipe for files to send is there) but this one is not: it is
            // between two of the pipe's instances, or too old to take web shares.
            Err(err)
                if mode == Mode::Share
                    && err.kind() == io::ErrorKind::NotFound
                    && pipe_there(SEND_PIPE) =>
            {
                thread::sleep(PIPE_GAP);
                match open_pipe(name) {
                    Ok(pipe) => pipe,
                    Err(err) if err.kind() == io::ErrorKind::NotFound => {
                        return Err(io::Error::other(
                            "this Zynk cannot make web links; update Zynk",
                        ));
                    }
                    Err(_) => return launch_app(mode, files),
                }
            }
            Err(_) => return launch_app(mode, files),
        };
        write_paths(pipe, files)
    }

    /// Writes FILES to PIPE, as the pipes take them.
    fn write_paths(mut pipe: File, files: &[PathBuf]) -> io::Result<()> {
        pipe.write_all(&utf16_message(files))?;
        pipe.flush()
    }

    /// Whether files handed over now are taken at once: the app runs, or an install is started
    /// with them. The Store's app, started bare, is waited for.
    pub(super) fn takes_files_at_once() -> bool {
        app_exe().is_some() || pipe_there(SEND_PIPE) || pipe_there(WEB_SHARE_PIPE)
    }

    /// Whether this Windows lets Zynk take files to send: they go to the picker of its tray,
    /// which Zynk has from Windows 10 version 2004 (build 19041). Assumed when unknown.
    pub(super) fn takes_sends() -> bool {
        specular_platform::windows_build().is_none_or(|build| build >= 19041)
    }

    /// Opens the pipe NAME to write to.
    fn open_pipe(name: &str) -> io::Result<File> {
        std::fs::OpenOptions::new().write(true).open(name)
    }

    /// Whether the pipe NAME is there (busy counts). Opening it and closing it at once hands
    /// the app nothing.
    fn pipe_there(name: &str) -> bool {
        !matches!(open_pipe(name), Err(err) if err.kind() == io::ErrorKind::NotFound)
    }

    /// Starts the app with FILES for MODE; a running app's second start passes them on. The
    /// Store's app is started bare, and gets them down its pipe once it is up.
    fn launch_app(mode: Mode, files: &[PathBuf]) -> io::Result<()> {
        if let Some(exe) = app_exe() {
            return launch(&exe, mode, files, None);
        }
        let family = package_family()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Zynk is not installed"))?;
        // Explorer starts it and ends at once (its exit status says nothing).
        Command::new("explorer.exe")
            .arg(format!(r"shell:AppsFolder\{family}!{PACKAGE_APP_ID}"))
            .stdin(Stdio::null())
            .status()?;
        write_paths(wait_for_pipe(mode)?, files)
    }

    /// Waits for a starting app's pipe for MODE.
    fn wait_for_pipe(mode: Mode) -> io::Result<File> {
        let name = match mode {
            Mode::Send => SEND_PIPE,
            Mode::Share => WEB_SHARE_PIPE,
        };
        let start = Instant::now();
        let mut send_pipe_since = None;
        loop {
            // Busy (another program writing) is waited out as well.
            if let Ok(pipe) = open_pipe(name) {
                return Ok(pipe);
            }
            if mode == Mode::Share && pipe_there(SEND_PIPE) {
                let since = *send_pipe_since.get_or_insert_with(Instant::now);
                if since.elapsed() > WEB_SHARE_GRACE {
                    return Err(io::Error::other(
                        "this Zynk cannot make web links; update Zynk",
                    ));
                }
            }
            if start.elapsed() > LAUNCH_TIMEOUT {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Zynk did not start",
                ));
            }
            thread::sleep(POLL_INTERVAL);
        }
    }

    /// The Store package's family name, when it is installed for this user: the name of its
    /// folder under `%LOCALAPPDATA%\Packages`.
    pub(super) fn package_family() -> Option<String> {
        let packages = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("Packages");
        let prefix = format!("{PACKAGE_NAME}_");
        std::fs::read_dir(packages)
            .ok()?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .find(|name| name.starts_with(&prefix))
    }

    /// The paths as UTF-16LE code units, each path ended by a NUL unit, as the pipes take them.
    fn utf16_message(files: &[PathBuf]) -> Vec<u8> {
        use std::os::windows::ffi::OsStrExt;
        let mut message = Vec::new();
        for file in files {
            for unit in file.as_os_str().encode_wide().chain(std::iter::once(0u16)) {
                message.extend_from_slice(&unit.to_le_bytes());
            }
        }
        message
    }

    /// `SPECULAR_ZYNK_APP` when set, else the first install of the app that exists: per user,
    /// or the installer's, under Program Files (in its publisher's folder).
    pub(super) fn app_exe() -> Option<PathBuf> {
        if let Some(exe) = env_path(APP_ENV) {
            return exe.is_file().then_some(exe);
        }
        let mut candidates = Vec::new();
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            candidates.push(PathBuf::from(local).join("Programs").join("Zynk"));
        }
        for var in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(dir) = std::env::var_os(var) {
                let dir = PathBuf::from(dir);
                candidates.push(dir.join("Global Zone Limited").join("Zynk"));
                candidates.push(dir.join("Zynk"));
            }
        }
        candidates
            .into_iter()
            .map(|dir| dir.join("Zynk.exe"))
            .find(|path| path.is_file())
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod linux {
    use std::io;
    use std::path::{Path, PathBuf};

    use super::{APP_ENV, Mode, env_path, launch};

    /// Starts the app with the files; when Zynk runs, that start passes them on to it, with
    /// ACTIVATION, and ends. Zynk then asks who gets them, or opens its web share form.
    pub(super) fn hand_over(
        mode: Mode,
        files: &[PathBuf],
        activation: Option<&str>,
    ) -> io::Result<()> {
        let exe = app_exe()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Zynk is not installed"))?;
        launch(&exe, mode, files, activation)
    }

    /// `SPECULAR_ZYNK_APP` when set, else the app's `zynk` on the PATH (an install, the
    /// snap), else an AppImage in `~/Applications`, `~/.local/bin` or `~/AppImages`, else the
    /// one Zynk's desktop entry starts, else the AppImage or bundle of a Zynk that runs (it
    /// may be anywhere). Not a flatpak: Zynk is not packaged as one, and a sandboxed one could
    /// not open the paths given it.
    pub(super) fn app_exe() -> Option<PathBuf> {
        if let Some(exe) = env_path(APP_ENV) {
            return exe.is_file().then_some(exe);
        }
        // Every `zynk` on the PATH, not the first: the command-line one may come before.
        let on_path = std::env::var_os("PATH").and_then(|path| {
            std::env::split_paths(&path)
                .map(|dir| dir.join("zynk"))
                .find(|exe| exe.is_file() && is_desktop_app(exe))
        });
        if let Some(exe) = on_path {
            return Some(exe);
        }
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let in_folders = home.as_ref().and_then(|home| {
            [
                home.join("Applications"),
                home.join(".local/bin"),
                home.join("AppImages"),
            ]
            .iter()
            .find_map(|dir| app_image_in(dir))
        });
        in_folders
            .or_else(|| desktop_entry_exe(home.as_deref()))
            .or_else(running_exe)
    }

    /// The program Zynk's desktop entry starts (Zynk writes `it.zynk.desktop` when it runs),
    /// when that is there and not inside a running AppImage, whose files go when it ends.
    fn desktop_entry_exe(home: Option<&Path>) -> Option<PathBuf> {
        let data = env_path("XDG_DATA_HOME").or_else(|| Some(home?.join(".local/share")))?;
        let entry = std::fs::read_to_string(data.join("applications/it.zynk.desktop")).ok()?;
        let exe = exec_program(entry_exec(&entry)?)?;
        let runs = is_executable(&exe) && (is_app_image(&exe) || is_desktop_app(&exe));
        (runs && !in_app_image(&exe)).then_some(exe)
    }

    /// The `Exec` value of a desktop entry's `[Desktop Entry]` group.
    fn entry_exec(entry: &str) -> Option<&str> {
        entry
            .lines()
            .map(str::trim)
            .skip_while(|line| *line != "[Desktop Entry]")
            .skip(1)
            .take_while(|line| !line.starts_with('['))
            .find_map(|line| {
                let (key, value) = line.split_once('=')?;
                (key.trim_end() == "Exec").then(|| value.trim_start())
            })
    }

    /// The program an `Exec` value starts: its first argument, with the string's escapes
    /// (`\s`, `\\` and the like) undone, then the key's quoting (in double quotes, `\"`,
    /// `` \` ``, `\$` and `\\`), and `%%` read as `%` (Desktop Entry Specification, "The Exec
    /// key"). An unknown escape is kept as it is, so a `\"` written with one backslash, as
    /// some do, still reads as a quote.
    fn exec_program(exec: &str) -> Option<PathBuf> {
        let mut value = String::with_capacity(exec.len());
        let mut chars = exec.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                value.push(c);
                continue;
            }
            match chars.next() {
                Some('s') => value.push(' '),
                Some('n') => value.push('\n'),
                Some('t') => value.push('\t'),
                Some('r') => value.push('\r'),
                Some('\\') => value.push('\\'),
                Some(other) => value.extend(['\\', other]),
                None => value.push('\\'),
            }
        }
        let value = value.trim_start();
        let mut program = String::new();
        if let Some(quoted) = value.strip_prefix('"') {
            let mut chars = quoted.chars();
            loop {
                // Not closed: not an entry to trust.
                match chars.next()? {
                    '"' => break,
                    '\\' => match chars.next()? {
                        c @ ('"' | '`' | '$' | '\\') => program.push(c),
                        c => program.extend(['\\', c]),
                    },
                    c => program.push(c),
                }
            }
        } else {
            program.extend(value.chars().take_while(|c| !c.is_whitespace()));
        }
        let program = program.replace("%%", "%");
        (!program.is_empty()).then(|| PathBuf::from(program))
    }

    /// Whether PATH is inside a running AppImage (its files are mounted at a `.mount_…`
    /// folder for as long as it runs).
    fn in_app_image(path: &Path) -> bool {
        path.components().any(|c| {
            c.as_os_str()
                .to_str()
                .is_some_and(|name| name.starts_with(".mount_"))
        })
    }

    /// The AppImage of a Zynk that runs, from wherever it was started: the AppImage's own
    /// process serves the app's files while it runs, and names the AppImage as its program
    /// (`/proc/PID/exe`; only the user's own processes can be read). Else the program of a
    /// Zynk that runs from an unpacked bundle.
    fn running_exe() -> Option<PathBuf> {
        let programs: Vec<PathBuf> = std::fs::read_dir("/proc")
            .ok()?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                let name = entry.file_name();
                let name = name.as_encoded_bytes();
                !name.is_empty() && name.iter().all(u8::is_ascii_digit)
            })
            .filter_map(|entry| std::fs::read_link(entry.path().join("exe")).ok())
            .collect();
        programs
            .iter()
            .find(|exe| is_app_image(exe) && exe.is_file())
            .or_else(|| {
                programs.iter().find(|exe| {
                    exe.file_name().is_some_and(|name| name == "zynk")
                        && !in_app_image(exe)
                        && is_desktop_app(exe)
                })
            })
            .cloned()
    }

    /// Whether EXE, a `zynk` on the PATH, is the desktop app rather than Zynk's command-line
    /// program of the same name: (followed through links) a Flutter bundle's program, with the
    /// bundle's `data/flutter_assets` beside it, or a snap's command whose snap holds such a
    /// bundle.
    fn is_desktop_app(exe: &Path) -> bool {
        if exe.starts_with("/snap/bin") {
            // A link to `snap` itself; the snap's files are under /snap/NAME/current.
            return exe.file_name().is_some_and(|name| {
                holds_flutter_bundle(&Path::new("/snap").join(name).join("current"), 2)
            });
        }
        std::fs::canonicalize(exe)
            .ok()
            .and_then(|real| real.parent().map(|dir| holds_flutter_bundle(dir, 0)))
            .unwrap_or(false)
    }

    /// Whether DIR, or a folder at most DEPTH levels inside it, is a Flutter bundle (has
    /// `data/flutter_assets`).
    fn holds_flutter_bundle(dir: &Path, depth: usize) -> bool {
        if dir.join("data").join("flutter_assets").is_dir() {
            return true;
        }
        depth > 0
            && std::fs::read_dir(dir).is_ok_and(|entries| {
                entries
                    .filter_map(|entry| entry.ok())
                    .map(|entry| entry.path())
                    .any(|path| path.is_dir() && holds_flutter_bundle(&path, depth - 1))
            })
    }

    /// The Zynk AppImage in DIR (`Zynk-1.2.3-x86_64.AppImage` and the like), the last by name
    /// when there are several. One not yet made executable is passed over (it could not be
    /// started): without another, the user is told how to set it up.
    fn app_image_in(dir: &Path) -> Option<PathBuf> {
        std::fs::read_dir(dir)
            .ok()?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| is_app_image(path) && is_executable(path))
            .max()
    }

    /// Whether PATH is named as Zynk's AppImage is.
    fn is_app_image(path: &Path) -> bool {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        name.starts_with("zynk") && name.ends_with(".appimage")
    }

    /// Whether PATH is a file that may be run.
    fn is_executable(path: &Path) -> bool {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    }

    #[cfg(test)]
    mod tests {
        use std::path::PathBuf;

        use super::{entry_exec, exec_program};

        #[test]
        fn exec_in_the_desktop_entry_group() {
            let entry = "[Desktop Entry]\nName=Zynk\nTryExec=/x\nExec = /a/zynk %U\n\
                         [Desktop Action New]\nExec=/b\n";
            assert_eq!(entry_exec(entry), Some("/a/zynk %U"));
            let other = "[Desktop Action New]\nExec=/b\n[Desktop Entry]\nName=Zynk\n";
            assert_eq!(entry_exec(other), None);
        }

        #[test]
        fn exec_program_undoes_escapes_and_quoting() {
            let cases = [
                (
                    "/home/a/Applications/Zynk-1.4.AppImage %U",
                    Some("/home/a/Applications/Zynk-1.4.AppImage"),
                ),
                (
                    r#""/home/a/My Apps/zynk.AppImage" %U"#,
                    Some("/home/a/My Apps/zynk.AppImage"),
                ),
                // As Zynk writes it: the quoting's backslashes doubled by the string's.
                (
                    r#""/home/a/x\\"y\\$z\\\\w.AppImage" %U"#,
                    Some(r#"/home/a/x"y$z\w.AppImage"#),
                ),
                // One backslash, as some write it.
                (r#""/home/a/x\"y" %U"#, Some(r#"/home/a/x"y"#)),
                (
                    r#""/home/a/My\sApps/zynk" %U"#,
                    Some("/home/a/My Apps/zynk"),
                ),
                ("/home/a/100%%/zynk %U", Some("/home/a/100%/zynk")),
                (r#""/home/a/unclosed %U"#, None),
                ("", None),
            ];
            for (exec, want) in cases {
                assert_eq!(exec_program(exec), want.map(PathBuf::from), "{exec}");
            }
        }
    }
}
