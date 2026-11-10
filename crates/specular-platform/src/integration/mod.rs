//! Opt-in desktop integration (DESIGN.md §11, "Deep desktop integration").
//!
//! Settings → Integration and `specular --integrate/--unintegrate/--integration-status` list the
//! options of the current platform ([`options`]), show their [`Status`] and [`apply`] or
//! [`remove`] them. Every option is per-user, needs no administrator rights and can be undone:
//! removing it takes away every file Specular made and restores every setting it changed.
//!
//! * Linux ([`linux`]): default viewer (`.desktop` file, MIME types, `xdg-mime default`),
//!   freedesktop thumbnailer, D-Bus activation (`org.freedesktop.Application`, single
//!   instance), file-manager actions (Nautilus, Dolphin, Thunar, Nemo), GNOME Shell and KRunner
//!   search providers, `specular://` links, the desktop's recent files. "Set as wallpaper"
//!   ([`set_wallpaper`]) is an action, not an option. The `.desktop` file carries the
//!   launcher actions ([`shell::DESKTOP_ACTIONS`]) and the MIME types of [`mime`].
//! * Windows ([`windows`]): the recent files (`SHAddToRecentDocs`); design notes and the
//!   [`shell`] seams for the rest (jump list tasks, thumbnail and preview handlers, search
//!   properties, WIC codec), each returning [`Unavailable::Platform`].
//! * macOS ([`macos`]): design notes and the [`shell`] seams; [`options`] is empty there and
//!   the Integration page is hidden.
//!
//! Everything works on an explicit [`Env`] (the XDG folders, the executable, the desktop), so
//! tests run against throw-away folders and never touch the real user's configuration.

use std::fmt;
use std::path::{Path, PathBuf};

#[cfg(all(unix, not(target_os = "macos")))]
pub mod linux;
pub mod macos;
pub mod mime;
pub mod shell;
pub mod windows;

/// One integration option.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OptionId {
    /// Specular as the default image viewer.
    DefaultViewer,
    /// File managers show previews of formats the desktop cannot thumbnail.
    Thumbnailer,
    /// Opening files from the desktop reuses the running window (single instance).
    DbusActivation,
    /// "Open in Specular", "Compare in Specular", "Set as wallpaper" in file managers.
    FileManagerActions,
    /// The desktop search finds images in the current folder and recent files.
    SearchProvider,
    /// `specular://` links open in Specular (`x-scheme-handler/specular`).
    UrlScheme,
    /// Viewed images appear among the desktop's recent files (GNOME Files "Recent", KDE
    /// "Recent Files", file choosers).
    RecentFiles,
}

impl OptionId {
    pub const ALL: [OptionId; 7] = [
        OptionId::DefaultViewer,
        OptionId::Thumbnailer,
        OptionId::DbusActivation,
        OptionId::FileManagerActions,
        OptionId::SearchProvider,
        OptionId::UrlScheme,
        OptionId::RecentFiles,
    ];

    /// The name used on the command line and in state files (`default-viewer`).
    pub fn key(self) -> &'static str {
        match self {
            OptionId::DefaultViewer => "default-viewer",
            OptionId::Thumbnailer => "thumbnailer",
            OptionId::DbusActivation => "single-instance",
            OptionId::FileManagerActions => "file-manager-actions",
            OptionId::SearchProvider => "search-provider",
            OptionId::UrlScheme => "url-scheme",
            OptionId::RecentFiles => "recent-files",
        }
    }

    /// The settings key of its row on the Integration page (`integration.default_viewer`).
    pub fn setting_key(self) -> &'static str {
        match self {
            OptionId::DefaultViewer => "integration.default_viewer",
            OptionId::Thumbnailer => "integration.thumbnailer",
            OptionId::DbusActivation => "integration.single_instance",
            OptionId::FileManagerActions => "integration.file_manager_actions",
            OptionId::SearchProvider => "integration.search_provider",
            OptionId::UrlScheme => "integration.url_scheme",
            OptionId::RecentFiles => "integration.recent_files",
        }
    }

    /// Looks an option up by [`key`](Self::key) (also accepts `dbus-activation`).
    pub fn from_key(s: &str) -> Option<OptionId> {
        if s == "dbus-activation" {
            return Some(OptionId::DbusActivation);
        }
        OptionId::ALL.into_iter().find(|o| o.key() == s)
    }

    /// Looks an option up by [`setting_key`](Self::setting_key).
    pub fn from_setting_key(s: &str) -> Option<OptionId> {
        OptionId::ALL.into_iter().find(|o| o.setting_key() == s)
    }
}

/// Why an option cannot be used here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unavailable {
    /// Not implemented on this operating system yet.
    Platform,
    /// Running in a sandbox (Flatpak) that manages this itself.
    Sandboxed,
    /// A program the option needs is not installed.
    MissingTool(&'static str),
    /// No D-Bus session bus.
    NoSessionBus,
    /// None of the supported file managers is installed.
    NoFileManager,
    /// None of the supported desktop search front ends is installed.
    NoSearch,
}

impl fmt::Display for Unavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unavailable::Platform => f.write_str("not available on this platform"),
            Unavailable::Sandboxed => f.write_str("managed by the sandbox"),
            Unavailable::MissingTool(t) => write!(f, "{t} is not installed"),
            Unavailable::NoSessionBus => f.write_str("no D-Bus session bus"),
            Unavailable::NoFileManager => f.write_str("no supported file manager"),
            Unavailable::NoSearch => f.write_str("no supported desktop search"),
        }
    }
}

/// Whether an option is in effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    On,
    Off,
    /// Applied, but part of it was undone from outside (another default viewer chosen for some
    /// types, a file deleted). Applying again repairs it; removing cleans up.
    Partial,
    Unavailable(Unavailable),
}

impl State {
    /// Command-line spelling.
    pub fn key(&self) -> &'static str {
        match self {
            State::On => "on",
            State::Off => "off",
            State::Partial => "partial",
            State::Unavailable(_) => "unavailable",
        }
    }
}

/// An option's state and what it covers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub id: OptionId,
    pub state: State,
    /// What it applies to, as proper names: file managers ("Nautilus"), search front ends
    /// ("GNOME Shell"), or for the default viewer the number of MIME types ("12/30").
    pub targets: Vec<String>,
}

impl Status {
    pub fn is_applied(&self) -> bool {
        matches!(self.state, State::On | State::Partial)
    }
}

/// Texts written into file-manager menus, in the user interface language at the time the
/// option is applied (they are files on disk, not live translations).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Labels {
    pub open: String,
    pub compare: String,
    pub wallpaper: String,
    /// Generic name for the `.desktop` file ("Image Viewer").
    pub generic_name: String,
    /// One-line description for the `.desktop` file.
    pub comment: String,
    /// The `.desktop` file's launcher-menu actions ("New Window", "Library", "Slideshow").
    pub new_window: String,
    pub library: String,
    pub slideshow: String,
}

impl Default for Labels {
    fn default() -> Labels {
        Labels {
            open: "Open in Specular".into(),
            compare: "Compare in Specular".into(),
            wallpaper: "Set as Wallpaper".into(),
            generic_name: "Image Viewer".into(),
            comment: "Open and flip through large photos instantly".into(),
            new_window: "New Window".into(),
            library: "Library".into(),
            slideshow: "Slideshow of Pictures".into(),
        }
    }
}

/// An integration failure.
#[derive(Debug)]
pub enum Error {
    Unavailable(Unavailable),
    Io(PathBuf, std::io::Error),
    /// A configuration file is not in a form we can edit safely; it was left alone.
    Unrecognized(PathBuf, String),
    /// A helper program failed.
    Tool(String, String),
    /// D-Bus failed.
    Bus(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unavailable(u) => u.fmt(f),
            Error::Io(p, e) => write!(f, "{}: {e}", p.display()),
            Error::Unrecognized(p, why) => {
                write!(
                    f,
                    "{}: not changed, unexpected content ({why})",
                    p.display()
                )
            }
            Error::Tool(t, e) => write!(f, "{t}: {e}"),
            Error::Bus(e) => write!(f, "D-Bus: {e}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn io_err(p: &Path) -> impl FnOnce(std::io::Error) -> Error + '_ {
    move |e| Error::Io(p.to_path_buf(), e)
}

/// Where integration reads and writes: the freedesktop base directories, the program to
/// register, the desktop, and whether helper programs (`xdg-mime`, `update-desktop-database`)
/// may run. [`Env::detect`] for real use; tests build one over a temporary folder.
#[derive(Clone, Debug)]
pub struct Env {
    /// `$XDG_DATA_HOME` (`~/.local/share`).
    pub data_home: PathBuf,
    /// `$XDG_CONFIG_HOME` (`~/.config`).
    pub config_home: PathBuf,
    /// `$XDG_STATE_HOME` (`~/.local/state`): our records of what was changed, and backups.
    pub state_home: PathBuf,
    /// `$XDG_DATA_DIRS`.
    pub data_dirs: Vec<PathBuf>,
    /// `$XDG_CONFIG_DIRS`.
    pub config_dirs: Vec<PathBuf>,
    /// The program registrations point at: the AppImage file when running from one, else the
    /// running executable.
    pub exe: PathBuf,
    /// `$XDG_CURRENT_DESKTOP`, split (`["ubuntu", "GNOME"]`).
    pub desktops: Vec<String>,
    /// `$PATH`, for finding helper programs and file managers.
    pub path: Vec<PathBuf>,
    /// Inside a Flatpak sandbox.
    pub sandboxed: bool,
    /// Run helper programs (with this Env's folders in their environment). Off in unit tests:
    /// then every file is written by our own code.
    pub run_tools: bool,
    /// Types the user added to the default-viewer claim (`integration.extra_default_types`;
    /// see [`mime::ExtraGroup`]). Types outside the groups are ignored.
    pub extra_default_types: Vec<String>,
}

impl Env {
    /// The environment of this process.
    pub fn detect() -> Env {
        let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from);
        let dir = |var: &str, fallback: &str| {
            std::env::var_os(var)
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .unwrap_or_else(|| home.join(fallback))
        };
        let dirs = |var: &str, fallback: &str| -> Vec<PathBuf> {
            let v = std::env::var(var).ok().filter(|s| !s.is_empty());
            v.as_deref()
                .unwrap_or(fallback)
                .split(':')
                .filter(|s| s.starts_with('/'))
                .map(PathBuf::from)
                .collect()
        };
        let exe = std::env::var_os("APPIMAGE")
            .map(PathBuf::from)
            .filter(|p| p.is_file())
            .or_else(|| std::env::current_exe().ok())
            .map(|p| p.canonicalize().unwrap_or(p))
            .unwrap_or_else(|| PathBuf::from("specular"));
        #[cfg(windows)]
        let (data_home, config_home, state_home) = {
            // No XDG folders: roaming application data for data and configuration, local for
            // Specular's own records (%LOCALAPPDATA%\Specular\integration).
            let _ = dir;
            let base = directories::BaseDirs::new();
            let or_home = |p: Option<&Path>| p.map_or_else(|| home.clone(), Path::to_path_buf);
            (
                or_home(base.as_ref().map(directories::BaseDirs::data_dir)),
                or_home(base.as_ref().map(directories::BaseDirs::config_dir)),
                or_home(base.as_ref().map(directories::BaseDirs::data_local_dir)),
            )
        };
        #[cfg(not(windows))]
        let (data_home, config_home, state_home) = (
            dir("XDG_DATA_HOME", ".local/share"),
            dir("XDG_CONFIG_HOME", ".config"),
            dir("XDG_STATE_HOME", ".local/state"),
        );
        Env {
            data_home,
            config_home,
            state_home,
            data_dirs: dirs("XDG_DATA_DIRS", "/usr/local/share:/usr/share"),
            config_dirs: dirs("XDG_CONFIG_DIRS", "/etc/xdg"),
            exe,
            desktops: std::env::var("XDG_CURRENT_DESKTOP")
                .unwrap_or_default()
                .split(':')
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
            path: std::env::var_os("PATH")
                .map(|p| std::env::split_paths(&p).collect())
                .unwrap_or_default(),
            sandboxed: Path::new("/.flatpak-info").exists(),
            run_tools: true,
            extra_default_types: Vec::new(),
        }
    }

    /// An environment entirely under `root` (tests): no helper programs, nothing on `PATH`,
    /// no system data folders.
    pub fn under(root: &Path, exe: &Path) -> Env {
        Env {
            data_home: root.join("data"),
            config_home: root.join("config"),
            state_home: root.join("state"),
            data_dirs: vec![root.join("sys/share")],
            config_dirs: vec![root.join("sys/xdg")],
            exe: exe.to_path_buf(),
            desktops: Vec::new(),
            path: Vec::new(),
            sandboxed: false,
            run_tools: false,
            extra_default_types: Vec::new(),
        }
    }

    /// The same environment with the user's extra default-viewer types (the setting's
    /// comma-separated value).
    pub fn with_extra_default_types(mut self, setting: &str) -> Env {
        self.extra_default_types = mime::parse_extra_types(setting)
            .into_iter()
            .map(str::to_owned)
            .collect();
        self
    }

    /// Every type the default-viewer option claims here: the raster image types, then the
    /// user's extra types, in table order.
    pub fn default_viewer_types(&self) -> Vec<&'static str> {
        mime::IMAGE_TYPES
            .iter()
            .map(|(m, _)| *m)
            .filter(|m| {
                mime::is_default_viewer_type(m)
                    || (mime::extra_group(m).is_some()
                        && self.extra_default_types.iter().any(|t| t == m))
            })
            .collect()
    }

    /// Whether `XDG_CURRENT_DESKTOP` names `name` (case-insensitive).
    pub fn is_desktop(&self, name: &str) -> bool {
        self.desktops.iter().any(|d| d.eq_ignore_ascii_case(name))
    }

    /// A program on [`path`](Self::path).
    pub fn find(&self, program: &str) -> Option<PathBuf> {
        self.path
            .iter()
            .map(|d| d.join(program))
            .find(|p| p.is_file())
    }

    /// Specular's own integration folder: records of what each option changed, and backups.
    pub fn record_dir(&self) -> PathBuf {
        self.state_home.join("specular/integration")
    }
}

/// The user's Pictures folder: `XDG_PICTURES_DIR` from `user-dirs.dirs` in `config_home`
/// (Linux, BSD), else `home/Pictures` when it exists, else `home`. The launcher's
/// "Slideshow" action shows it.
pub fn pictures_dir(config_home: &Path, home: &Path) -> PathBuf {
    let from_user_dirs = std::fs::read_to_string(config_home.join("user-dirs.dirs"))
        .ok()
        .and_then(|text| {
            text.lines().find_map(|l| {
                let v = l.trim().strip_prefix("XDG_PICTURES_DIR=")?;
                let v = v.trim().trim_matches('"');
                let p = match v.strip_prefix("$HOME") {
                    Some(rest) => home.join(rest.trim_start_matches('/')),
                    None if v.starts_with('/') => PathBuf::from(v),
                    None => return None,
                };
                // `XDG_PICTURES_DIR="$HOME/"` means "disabled".
                (p != home).then_some(p)
            })
        });
    from_user_dirs
        .filter(|p| p.is_dir())
        .or_else(|| Some(home.join("Pictures")).filter(|p| p.is_dir()))
        .unwrap_or_else(|| home.to_path_buf())
}

/// [`pictures_dir`] of this process (`$HOME`, `$XDG_CONFIG_HOME`).
#[cfg(not(windows))]
pub fn user_pictures_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from);
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home.join(".config"));
    pictures_dir(&config, &home)
}

/// The user's Pictures folder: Windows' Pictures known folder, wherever the user moved it
/// (OneDrive included), else the profile folder. The launcher's "Slideshow" action shows it.
#[cfg(windows)]
pub fn user_pictures_dir() -> PathBuf {
    let dirs = directories::UserDirs::new();
    dirs.as_ref()
        .and_then(directories::UserDirs::picture_dir)
        .filter(|p| p.is_dir())
        .map(Path::to_path_buf)
        .or_else(|| dirs.as_ref().map(|d| d.home_dir().to_path_buf()))
        .or_else(std::env::home_dir)
        .unwrap_or_else(|| PathBuf::from(r"C:\"))
}

/// The options of this platform, in display order (empty where nothing is implemented).
pub fn options() -> &'static [OptionId] {
    #[cfg(all(unix, not(target_os = "macos")))]
    return &OptionId::ALL;
    #[cfg(windows)]
    return &[OptionId::RecentFiles];
    #[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
    &[]
}

/// The state of `id`. Reads files only (no programs run): cheap enough for every repaint of
/// the settings page, but better called off the UI thread.
pub fn status(env: &Env, id: OptionId) -> Status {
    #[cfg(all(unix, not(target_os = "macos")))]
    return linux::status(env, id);
    #[cfg(windows)]
    return windows::status(env, id);
    #[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
    {
        let _ = env;
        Status {
            id,
            state: State::Unavailable(Unavailable::Platform),
            targets: Vec::new(),
        }
    }
}

/// Turns `id` on (idempotent: applying again repairs a partial state).
pub fn apply(env: &Env, id: OptionId, labels: &Labels) -> Result<Status> {
    #[cfg(all(unix, not(target_os = "macos")))]
    return linux::apply(env, id, labels);
    #[cfg(windows)]
    {
        let _ = labels;
        windows::apply(env, id)
    }
    #[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
    {
        let _ = (env, labels, id);
        Err(Error::Unavailable(Unavailable::Platform))
    }
}

/// Turns `id` off, removing what [`apply`] made and restoring what it changed.
pub fn remove(env: &Env, id: OptionId) -> Result<Status> {
    #[cfg(all(unix, not(target_os = "macos")))]
    return linux::remove(env, id);
    #[cfg(windows)]
    return windows::remove(env, id);
    #[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
    {
        let _ = (env, id);
        Err(Error::Unavailable(Unavailable::Platform))
    }
}

/// How the wallpaper was set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WallpaperBackend {
    /// xdg-desktop-portal's Wallpaper portal.
    Portal,
    Gnome,
    Cinnamon,
    Mate,
    Kde,
    Xfce,
    Sway,
    /// Windows: `IDesktopWallpaper` or `SystemParametersInfoW`.
    Windows(&'static str),
    /// macOS: `NSWorkspace`.
    MacOs,
}

impl WallpaperBackend {
    pub fn name(self) -> &'static str {
        match self {
            WallpaperBackend::Portal => "xdg-desktop-portal",
            WallpaperBackend::Gnome => "GNOME",
            WallpaperBackend::Cinnamon => "Cinnamon",
            WallpaperBackend::Mate => "MATE",
            WallpaperBackend::Kde => "KDE Plasma",
            WallpaperBackend::Xfce => "Xfce",
            WallpaperBackend::Sway => "sway",
            WallpaperBackend::Windows(how) => how,
            WallpaperBackend::MacOs => "NSWorkspace",
        }
    }
}

/// Sets `image` as the desktop background: the Wallpaper portal when there is one, else the
/// desktop's own mechanism. Blocks (the portal may ask the user); call off the UI thread.
pub fn set_wallpaper(env: &Env, image: &Path) -> Result<WallpaperBackend> {
    #[cfg(all(unix, not(target_os = "macos")))]
    return linux::wallpaper::set(env, image);
    #[cfg(windows)]
    {
        let _ = env;
        crate::share::windows::set_wallpaper(image)
            .map(WallpaperBackend::Windows)
            .map_err(|e| Error::Io(image.to_path_buf(), e))
    }
    #[cfg(target_os = "macos")]
    {
        let _ = env;
        crate::share::macos::set_wallpaper(image)
            .map(|_| WallpaperBackend::MacOs)
            .map_err(|e| Error::Io(image.to_path_buf(), e))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (env, image);
        Err(Error::Unavailable(Unavailable::Platform))
    }
}

/// Whether "Set as wallpaper" can work here (a cheap guess; the call itself decides).
pub fn wallpaper_available(env: &Env) -> bool {
    #[cfg(all(unix, not(target_os = "macos")))]
    return linux::wallpaper::available(env);
    #[cfg(any(windows, target_os = "macos"))]
    {
        let _ = env;
        true
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = env;
        false
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// Windows' Pictures folder, not `$HOME` (unset there unless a Unix-style shell set it).
    #[test]
    fn user_pictures_dir_is_the_known_folder() {
        let p = user_pictures_dir();
        assert!(p.is_absolute() && p.is_dir(), "{}", p.display());
        assert!(p.parent().is_some(), "a drive root: {}", p.display());
        let known =
            directories::UserDirs::new().and_then(|d| d.picture_dir().map(Path::to_path_buf));
        if let Some(known) = known.filter(|k| k.is_dir()) {
            assert_eq!(p, known);
        }
    }
}
