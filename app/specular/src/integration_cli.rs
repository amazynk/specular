//! Desktop-integration command line (DESIGN.md §11, "Deep desktop integration"):
//!
//! * `specular --integration-status` lists the options and their state;
//! * `specular --integrate OPTION|all` and `--unintegrate OPTION|all` apply and remove them;
//! * `specular --set-wallpaper FILE` (the file-manager action);
//! * `specular --search-provider` (started by the session bus for GNOME Shell and KRunner);
//! * `specular FILE…` hands the files to a running viewer when the single-instance setting is
//!   on: over D-Bus on Linux (with the integration option on too), and `--gapplication-service`
//!   marks a start by the bus; over the remote-control socket on Windows;
//! * `specular specular:URL` opens a link: plain local files like file arguments, anything
//!   else in a running viewer through its socket (single-instance setting on), else in a new
//!   window (see `remote` for the order).
//!
//! None of these open a window or touch the GPU.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use specular_i18n::t;
use specular_platform::integration::{self as int, Env, Labels, OptionId, Status};

/// Files given on the command line when there were several (the first instance browses
/// exactly those, like "Compare" from a file manager).
static STARTUP_FILES: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// This process was started by the bus to serve `org.freedesktop.Application`.
static SERVICE_LAUNCH: AtomicBool = AtomicBool::new(false);

pub fn take_startup_files() -> Vec<PathBuf> {
    std::mem::take(&mut *STARTUP_FILES.lock().unwrap_or_else(|e| e.into_inner()))
}

pub fn service_launch() -> bool {
    SERVICE_LAUNCH.load(Ordering::Relaxed)
}

/// Menu and `.desktop` texts in the current language.
pub fn labels() -> Labels {
    Labels {
        open: t!("integration-label-open"),
        compare: t!("integration-label-compare"),
        wallpaper: t!("integration-label-wallpaper"),
        generic_name: t!("integration-label-generic-name"),
        comment: t!("integration-label-comment"),
        new_window: t!("integration-label-new-window"),
        library: t!("integration-label-library"),
        slideshow: t!("integration-label-slideshow"),
    }
}

pub const USAGE: &str = "Desktop integration (per user, undone by --unintegrate):\n  \
     --integration-status   list the integration options and their state\n  \
     --integrate OPTION     apply OPTION (or all): default-viewer, thumbnailer,\n                         \
     single-instance, file-manager-actions, search-provider,\n                         \
     url-scheme, recent-files\n  \
     --unintegrate OPTION   remove OPTION (or all), restoring what it changed\n  \
     --set-wallpaper FILE   set FILE as the desktop background and exit";

/// Handles the integration arguments. `Some(code)`: done, exit with `code`.
pub fn from_args() -> Option<i32> {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let Some(first) = args.first() else {
        // Windows: a start without files (the Start menu, the taskbar) raises the running
        // window, or becomes the one later files go to. Linux leaves this to D-Bus activation.
        #[cfg(windows)]
        forward_or_continue(&args);
        return None;
    };
    let first = first.to_str().unwrap_or("");
    let one_value = |what: &str| -> Result<&std::ffi::OsString, i32> {
        match args.as_slice() {
            [_, v] => Ok(v),
            _ => {
                eprintln!("usage: specular {first} {what}");
                Err(2)
            }
        }
    };
    match first {
        "--integration-status" => Some(status_cmd()),
        "--integrate" | "--unintegrate" => Some(match one_value("OPTION|all") {
            Ok(v) => change_cmd(first == "--integrate", &v.to_string_lossy()),
            Err(c) => c,
        }),
        "--set-wallpaper" => Some(match one_value("FILE") {
            Ok(v) => wallpaper_cmd(&path_arg(v)),
            Err(c) => c,
        }),
        #[cfg(all(unix, not(target_os = "macos")))]
        int::linux::search::SERVICE_ARG => Some(search_cmd()),
        _ => {
            forward_or_continue(&args);
            None
        }
    }
}

fn path_arg(a: &std::ffi::OsStr) -> PathBuf {
    a.to_str()
        .filter(|s| s.starts_with("file:"))
        .and_then(specular_platform::launch::path_from_uri)
        .unwrap_or_else(|| PathBuf::from(a))
}

fn print_status(s: &Status) {
    let detail = match &s.state {
        int::State::Unavailable(why) => format!(" ({why})"),
        _ if !s.targets.is_empty() => format!(" ({})", s.targets.join(", ")),
        _ => String::new(),
    };
    println!("{:<22} {}{detail}", s.id.key(), s.state.key());
}

/// This process's environment with the user's extra default-viewer types (Settings →
/// Integration → "More file types").
pub(crate) fn detect_env() -> Env {
    let extra = specular_settings::SettingsStore::open_default()
        .map(|s| s.snapshot().integration.extra_default_types.clone())
        .unwrap_or_default();
    Env::detect().with_extra_default_types(&extra)
}

fn status_cmd() -> i32 {
    let env = detect_env();
    if int::options().is_empty() {
        println!("no desktop integration options on this platform yet");
        return 0;
    }
    for id in int::options() {
        print_status(&int::status(&env, *id));
    }
    0
}

fn change_cmd(on: bool, which: &str) -> i32 {
    specular_log::init(None);
    let env = detect_env();
    let ids: Vec<OptionId> = if which == "all" {
        int::options().to_vec()
    } else {
        match OptionId::from_key(which) {
            Some(id) => vec![id],
            None => {
                eprintln!(
                    "specular: unknown integration option {which:?} (try --integration-status)"
                );
                return 2;
            }
        }
    };
    let labels = labels();
    let mut code = 0;
    for id in ids {
        if which == "all" && on {
            // `all` skips what cannot work here instead of failing.
            if let int::State::Unavailable(_) = int::status(&env, id).state {
                continue;
            }
        }
        let r = if on {
            int::apply(&env, id, &labels)
        } else {
            int::remove(&env, id)
        };
        match r {
            Ok(s) => print_status(&s),
            Err(e) => {
                eprintln!("specular: {}: {e}", id.key());
                code = 1;
            }
        }
    }
    code
}

fn wallpaper_cmd(file: &std::path::Path) -> i32 {
    specular_log::init(None);
    let env = Env::detect();
    match int::set_wallpaper(&env, file) {
        Ok(b) => {
            println!("wallpaper set ({})", b.name());
            0
        }
        Err(e) => {
            eprintln!("specular: wallpaper: {e}");
            1
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn search_cmd() -> i32 {
    use int::linux::search;
    specular_log::init(None);
    let env = Env::detect();
    let source = Box::new(crate::desktop_search::LibrarySource::detect());
    let texts = search::Texts {
        open_folder: t!("search-provider-open-folder"),
    };
    match search::serve(&env, source, texts, std::time::Duration::from_secs(60)) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("specular: search provider: {e}");
            1
        }
    }
}

/// Options of `main::parse_args` whose value is the next argument (unless given as `=VALUE`).
const VALUE_OPTIONS: &[&str] = &[
    "--bench-open",
    "--set",
    "--lang",
    "--log",
    "--then-open",
    "--screenshot",
    "--show",
    "--query",
    "--zoom",
    "--bench-hold",
    "--bench-repeat",
    "--bench-pan",
    "--bench-loupe",
    "--size",
    "--files-from",
];

/// A plain start: remember several files for the window; with the single-instance option on
/// and a viewer running, hand the files to it and exit.
fn forward_or_continue(args: &[std::ffi::OsString]) {
    let mut files = Vec::new();
    let mut links = Vec::new();
    let mut only_paths = false;
    let mut plain = true;
    let mut action = None;
    let mut query = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let s = a.to_string_lossy();
        if !only_paths && s == "--" {
            only_paths = true;
        } else if !only_paths && (s == "--library" || s == "--slideshow") {
            // Launcher-menu actions: the running viewer does them when there is one.
            action = Some(if s == "--library" {
                LauncherAction::Library
            } else {
                LauncherAction::Slideshow
            });
        } else if !only_paths && s == "--search" {
            // The desktop search's "more results": the Library with the query.
            action = Some(LauncherAction::Library);
            query = it.next().map(|q| q.to_string_lossy().into_owned());
        } else if !only_paths && s == "--gapplication-service" {
            SERVICE_LAUNCH.store(true, Ordering::Relaxed);
        } else if !only_paths && specular_platform::url_scheme::is_url(&s) {
            match specular_platform::url_scheme::parse(&s) {
                Ok(specular_platform::url_scheme::Target::Local(paths)) => files.extend(paths),
                _ => links.push(s.into_owned()),
            }
        } else if !only_paths && s == "-" {
            // A file list on standard input: this process reads it.
            plain = false;
        } else if only_paths || !s.starts_with('-') {
            files.push(path_arg(a));
        } else {
            // Any other option (benchmarks, screenshots, --set …): a separate instance.
            plain = false;
            if VALUE_OPTIONS.contains(&&*s) {
                it.next();
            }
        }
    }
    if files.len() > 1 {
        *STARTUP_FILES.lock().unwrap_or_else(|e| e.into_inner()) = files.clone();
    }
    if !plain || service_launch() {
        return;
    }
    if let Some(action) = action {
        // `--library` with files, or `--slideshow` with several: a separate instance.
        let folder = match (action, files.as_slice()) {
            (LauncherAction::Library, []) => None,
            (LauncherAction::Slideshow, []) => Some(int::user_pictures_dir()),
            (LauncherAction::Slideshow, [one]) => {
                Some(std::path::absolute(one).unwrap_or(one.clone()))
            }
            _ => return,
        };
        if links.is_empty() {
            forward_action_to_running(action, folder, query);
        }
        return;
    }
    if links.is_empty() {
        forward_to_running(files);
    } else if files.is_empty()
        && single_instance()
        && links.iter().all(|l| crate::remote::forward_url(l))
    {
        // Links that need a question (or a refusal) go where the user already looks.
        std::process::exit(0);
    }
}

/// The single-instance setting (on unless the settings say otherwise).
pub fn single_instance_on() -> bool {
    single_instance()
}

fn single_instance() -> bool {
    specular_settings::SettingsStore::open_default()
        .map(|s| s.snapshot().general.single_instance)
        .unwrap_or(true)
}

/// Hands `files` to a running viewer over D-Bus and exits, when the single-instance option is
/// on and one is running.
#[cfg(all(unix, not(target_os = "macos")))]
fn forward_to_running(files: Vec<PathBuf>) {
    use int::linux::dbus;
    let env = Env::detect();
    if !dbus::enabled(&env) {
        return;
    }
    if !single_instance() {
        return;
    }
    let files: Vec<PathBuf> = files
        .into_iter()
        .map(|f| std::path::absolute(&f).unwrap_or(f))
        .collect();
    match dbus::forward(&files) {
        Ok(true) => std::process::exit(0),
        Ok(false) => {}
        Err(e) => eprintln!("specular: single instance: {e}"),
    }
}

/// Hands `files` to this copy's running viewer over its socket and exits, when the
/// single-instance setting is on and one runs. Several processes started at once (files
/// selected together in Explorer, each opened by its own process) agree through a named mutex:
/// the first opens the window, the others wait for it to listen, up to [`WAIT_FOR_VIEWER`].
#[cfg(windows)]
fn forward_to_running(files: Vec<PathBuf>) {
    use std::time::Instant;
    if !single_instance() {
        return;
    }
    let files: Vec<PathBuf> = files
        .into_iter()
        .map(|f| std::path::absolute(&f).unwrap_or(f))
        .collect();
    let name = instance_name(&crate::remote::client::fallback_dir());
    let until = Instant::now() + WAIT_FOR_VIEWER;
    loop {
        // The first viewer of this copy (or the one before it has exited): open the window.
        if specular_platform::instance::claim(&name) {
            return;
        }
        // Another one runs, or is starting and does not listen yet.
        if let Some(socket) = crate::remote::client::socket_path(None)
            && crate::remote::forward_files(&socket, &files)
        {
            std::process::exit(0);
        }
        if Instant::now() >= until {
            eprintln!("specular: the running viewer did not answer; opening another window");
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// How long a start waits for a viewer that is starting to listen on its socket.
#[cfg(windows)]
const WAIT_FOR_VIEWER: std::time::Duration = std::time::Duration::from_secs(10);

/// The mutex name of the viewers of one copy of Specular: from its socket folder (a portable
/// copy's own, or the profile's for an installed one), without case as Windows compares paths.
#[cfg(windows)]
fn instance_name(socket_folder: &std::path::Path) -> String {
    // FNV-1a: stable across runs and builds (unlike the standard library's hasher).
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in socket_folder.to_string_lossy().to_lowercase().bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("app.specular.Specular-{h:016x}")
}

/// macOS: no command-line hand-over to a running viewer yet.
#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
fn forward_to_running(_files: Vec<PathBuf>) {}

/// `--library` and `--slideshow` on the command line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LauncherAction {
    Library,
    Slideshow,
}

/// Asks a running viewer to do `action` (over D-Bus `ActivateAction`, like the launcher menu)
/// and exits, when the single-instance option is on and one is running.
#[cfg(all(unix, not(target_os = "macos")))]
fn forward_action_to_running(
    action: LauncherAction,
    folder: Option<PathBuf>,
    query: Option<String>,
) {
    use int::linux::{DesktopAction, dbus};
    let env = Env::detect();
    if !dbus::enabled(&env) || !single_instance() {
        return;
    }
    let action = match action {
        LauncherAction::Library => DesktopAction::Library,
        LauncherAction::Slideshow => DesktopAction::Slideshow,
    };
    match dbus::forward_action(action, folder.as_deref(), query.as_deref()) {
        Ok(true) => std::process::exit(0),
        Ok(false) => {}
        Err(e) => eprintln!("specular: single instance: {e}"),
    }
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
fn forward_action_to_running(
    _action: LauncherAction,
    _folder: Option<PathBuf>,
    _query: Option<String>,
) {
}

#[cfg(test)]
mod tests {
    /// One name per copy: the same folder in any case gives the same name, another copy another.
    #[cfg(windows)]
    #[test]
    fn instance_names_follow_the_copy() {
        use std::path::Path;
        let installed =
            super::instance_name(Path::new(r"C:\Users\Me\AppData\Local\Specular\cache\run"));
        let same = super::instance_name(Path::new(r"c:\users\me\appdata\local\specular\cache\RUN"));
        let portable = super::instance_name(Path::new(r"D:\Tools\Specular\cache\run"));
        assert_eq!(installed, same);
        assert_ne!(installed, portable);
        assert!(installed.starts_with("app.specular.Specular-") && !installed.contains('\\'));
    }

    /// The platform crate's MIME table (the `.desktop` `MimeType=`, the MIME package, the
    /// thumbnailer list) and the Windows installer's file associations (`associations.wxi`, the
    /// winget manifest's `FileExtensions`) are generated copies of the decoder registry's
    /// types; this fails when a decoder gains an extension they lack.
    /// `SPECULAR_UPDATE_MIME=1` rewrites them.
    #[test]
    fn mime_table_in_sync() {
        use specular_platform::integration::mime;
        let groups = specular_decode::Registry::new().mime_groups();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let winget = std::fs::read_to_string(root.join(mime::WINGET_PATH)).unwrap();
        let files = [
            (mime::TABLE_PATH, mime::render(&groups)),
            (mime::WIX_PATH, mime::render_wix(&groups)),
            (mime::WINGET_PATH, mime::render_winget(&winget, &groups)),
        ];
        let update = std::env::var_os("SPECULAR_UPDATE_MIME").is_some();
        let mut stale = Vec::new();
        for (rel, want) in files {
            let path = root.join(rel);
            let have = std::fs::read_to_string(&path).unwrap_or_default();
            if have.replace("\r\n", "\n") == want {
                continue;
            }
            if update {
                std::fs::write(&path, want).unwrap();
                eprintln!("rewrote {}", path.display());
            } else {
                stale.push(rel);
            }
        }
        assert!(
            stale.is_empty(),
            "{stale:?} out of date with the decoder registry; rerun this test with \
             SPECULAR_UPDATE_MIME=1"
        );
    }
}
