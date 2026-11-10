//! Where settings and caches live: the platform config folder, or next to the executable in
//! portable mode.
//!
//! | Platform | Settings folder |
//! |---|---|
//! | Linux | `$XDG_CONFIG_HOME/specular` (`~/.config/specular`) |
//! | macOS | `~/Library/Application Support/Specular` |
//! | Windows | `%APPDATA%\Specular\config` |
//!
//! Portable mode: a file named `portable`, or a settings file named `specular.toml`, next to
//! the executable keeps everything next to the executable instead:
//!
//! | What | Portable location |
//! |---|---|
//! | Settings | `specular.toml` if present, else `settings.toml` |
//! | Caches (display cache, pasted images) | `cache/` |
//! | Data (the Library's catalogue) | `data/` |
//! | Logs (`SPECULAR_LOG_FILE` relative names, crash notes) | `logs/` |
//!
//! Nothing is written outside that folder, so the whole thing can live on a USB stick.

use std::path::{Path, PathBuf};

/// Name of the portable-mode marker file next to the executable.
pub const PORTABLE_MARKER: &str = "portable";
/// Name of the settings file.
pub const SETTINGS_FILE: &str = "settings.toml";
/// A settings file with this name next to the executable also turns on portable mode, and is
/// then the settings file.
pub const PORTABLE_SETTINGS_FILE: &str = "specular.toml";

/// Folder containing the running executable (symbolic links resolved). On Windows without
/// the `\\?\` prefix canonicalising adds, so portable paths read like the user's (About).
pub fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| specular_platform::path::canonicalize(&p).ok().or(Some(p)))
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

/// Resolved locations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locations {
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
    /// Data that is not a cache and not a setting: the Library's catalogue
    /// (`$XDG_DATA_HOME/specular` on Linux).
    pub data_dir: PathBuf,
    /// Where log files go (created on first use).
    pub log_dir: PathBuf,
    /// The settings file (normally `config_dir/settings.toml`).
    pub settings_file: PathBuf,
    pub portable: bool,
}

impl Locations {
    /// Locations for this process.
    pub fn detect() -> Self {
        Self::for_exe_dir(exe_dir().as_deref())
    }

    /// Locations given the executable's folder (testable).
    pub fn for_exe_dir(exe_dir: Option<&Path>) -> Self {
        if let Some(dir) = exe_dir {
            let named = dir.join(PORTABLE_SETTINGS_FILE);
            if named.is_file() {
                return Self::portable_in(dir, named);
            }
            if dir.join(PORTABLE_MARKER).is_file() {
                return Self::portable_in(dir, dir.join(SETTINGS_FILE));
            }
        }
        match directories::ProjectDirs::from("", "", "Specular") {
            Some(p) => {
                let config_dir = p.config_dir().to_path_buf();
                let cache_dir = p.cache_dir().to_path_buf();
                // Linux: $XDG_STATE_HOME/specular/logs; elsewhere next to the cache.
                let log_dir = p
                    .state_dir()
                    .map_or_else(|| cache_dir.join("logs"), |s| s.join("logs"));
                Self {
                    settings_file: config_dir.join(SETTINGS_FILE),
                    data_dir: p.data_dir().to_path_buf(),
                    config_dir,
                    cache_dir,
                    log_dir,
                    portable: false,
                }
            }
            // No home folder (rare: system services). Fall back to the executable's folder.
            None => {
                let dir = exe_dir.map_or_else(|| PathBuf::from("."), Path::to_path_buf);
                Self::portable_in(&dir, dir.join(SETTINGS_FILE))
            }
        }
    }

    fn portable_in(dir: &Path, settings_file: PathBuf) -> Self {
        Self {
            config_dir: dir.to_path_buf(),
            cache_dir: dir.join("cache"),
            data_dir: dir.join("data"),
            log_dir: dir.join("logs"),
            settings_file,
            portable: true,
        }
    }

    /// Where a crash report goes when the UI thread panics: next to the log folder
    /// (`$XDG_STATE_HOME/specular/crashes` on Linux). Reports stay on this computer.
    pub fn crash_dir(&self) -> PathBuf {
        self.log_dir
            .parent()
            .map_or_else(|| self.log_dir.join("crashes"), |p| p.join("crashes"))
    }

    /// The settings file.
    pub fn settings_path(&self) -> PathBuf {
        self.settings_file.clone()
    }
}

/// Whether this process runs in portable mode.
pub fn is_portable() -> bool {
    Locations::detect().portable
}

/// `settings.toml` for this process.
pub fn settings_path() -> PathBuf {
    Locations::detect().settings_path()
}

/// Turns portable mode on or off by creating or removing the marker next to the executable.
/// When turning it on, the current settings file is copied next to the executable (if none is
/// there yet), so nothing is lost. Takes effect after a restart.
pub fn set_portable(enabled: bool) -> std::io::Result<()> {
    let dir = exe_dir().ok_or_else(|| std::io::Error::other("executable folder unknown"))?;
    set_portable_in(&dir, enabled)
}

pub(crate) fn set_portable_in(exe_dir: &Path, enabled: bool) -> std::io::Result<()> {
    let marker = exe_dir.join(PORTABLE_MARKER);
    if !enabled && exe_dir.join(PORTABLE_SETTINGS_FILE).is_file() {
        // `specular.toml` is both the marker and the settings; removing it would lose them.
        return Err(std::io::Error::other(format!(
            "portable mode is set by {PORTABLE_SETTINGS_FILE} next to the executable; move or rename it to turn portable mode off"
        )));
    }
    if enabled {
        let current = Locations::for_exe_dir(Some(exe_dir)).settings_path();
        let target = exe_dir.join(SETTINGS_FILE);
        if current != target && current.is_file() && !target.exists() {
            std::fs::copy(&current, &target)?;
        }
        std::fs::write(
            &marker,
            b"Specular portable mode: settings are kept in this folder.\n",
        )
    } else {
        match std::fs::remove_file(&marker) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/test-tmp/paths"
        ))
        .join(name);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Portable paths (shown in About) read like the user's: no verbatim prefix.
    #[cfg(windows)]
    #[test]
    fn exe_dir_has_no_verbatim_prefix() {
        let d = exe_dir().unwrap();
        assert!(!d.to_string_lossy().starts_with(r"\\?\"), "{d:?}");
        assert!(d.is_dir());
    }

    #[test]
    fn specular_toml_is_a_portable_settings_file() {
        let dir = scratch("specular-toml");
        let normal = Locations::for_exe_dir(Some(&dir));
        assert!(!normal.portable);
        assert!(!normal.log_dir.starts_with(&dir));
        assert!(!normal.data_dir.starts_with(&dir));

        std::fs::write(dir.join(PORTABLE_SETTINGS_FILE), "").unwrap();
        let p = Locations::for_exe_dir(Some(&dir));
        assert!(p.portable);
        assert_eq!(p.settings_path(), dir.join("specular.toml"));
        assert_eq!(p.cache_dir, dir.join("cache"));
        assert_eq!(p.data_dir, dir.join("data"));
        assert_eq!(p.log_dir, dir.join("logs"));
        assert_eq!(p.crash_dir(), dir.join("crashes"));
        // It wins over the plain marker.
        std::fs::write(dir.join(PORTABLE_MARKER), "").unwrap();
        assert_eq!(
            Locations::for_exe_dir(Some(&dir)).settings_path(),
            dir.join("specular.toml")
        );
        // Turning portable mode off cannot silently drop the user's settings file.
        assert!(set_portable_in(&dir, false).is_err());
        assert!(dir.join(PORTABLE_SETTINGS_FILE).is_file());

        let m = scratch("marker");
        std::fs::write(m.join(PORTABLE_MARKER), "").unwrap();
        let p = Locations::for_exe_dir(Some(&m));
        assert_eq!(p.settings_path(), m.join("settings.toml"));
        assert_eq!(p.log_dir, m.join("logs"));
    }
}
