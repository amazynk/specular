//! The managed-install policy file: what an administrator sets for every user of a machine.
//!
//! One system-wide TOML file, read at start-up, never written by the app:
//!
//! | System  | Path |
//! |---------|------|
//! | Linux   | `/etc/specular/policy.toml` |
//! | macOS   | `/Library/Application Support/Specular/policy.toml` |
//! | Windows | `%ProgramData%\Specular\policy.toml` |
//!
//! ```toml
//! # Nothing in Specular opens a network connection; users cannot turn it back on.
//! no_network = true
//! # Local AI models placed here by IT are installed from here (no download).
//! model_folder = "/srv/specular/models"
//! ```
//!
//! Unknown keys are ignored (a newer policy on an older app); a file that cannot be read or
//! parsed is logged and treated as no policy.

use std::path::{Path, PathBuf};

/// The policy's file name.
pub const FILE_NAME: &str = "policy.toml";

/// What the policy file sets.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Policy {
    /// The file it came from (`None`: no policy file).
    pub file: Option<PathBuf>,
    /// Turn the no-network switch on, for good.
    pub no_network: bool,
    /// A folder of Local AI model files to install from (instead of downloading).
    pub model_folder: Option<PathBuf>,
}

impl Policy {
    /// The policy of this machine ([`system_file`]); none when there is no file.
    pub fn load() -> Policy {
        match system_file() {
            Some(p) => Policy::load_from(&p),
            None => Policy::default(),
        }
    }

    /// The policy in `path` (none when it does not exist or cannot be read).
    pub fn load_from(path: &Path) -> Policy {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let mut p = Policy::parse(&text).unwrap_or_else(|e| {
                    specular_log::warn!(target: specular_log::SETTINGS, "policy {}: {e}", path.display());
                    Policy::default()
                });
                p.file = Some(path.to_owned());
                p
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Policy::default(),
            Err(e) => {
                specular_log::warn!(target: specular_log::SETTINGS, "policy {}: {e}", path.display());
                Policy::default()
            }
        }
    }

    /// Parses a policy file's text.
    pub fn parse(text: &str) -> Result<Policy, String> {
        let doc = text
            .parse::<toml_edit::DocumentMut>()
            .map_err(|e| e.to_string())?;
        let mut p = Policy::default();
        if let Some(v) = doc.get("no_network") {
            p.no_network = v
                .as_bool()
                .ok_or_else(|| "no_network: expected true or false".to_owned())?;
        }
        if let Some(v) = doc.get("model_folder") {
            let s = v
                .as_str()
                .ok_or_else(|| "model_folder: expected a path".to_owned())?;
            if !s.trim().is_empty() {
                p.model_folder = Some(PathBuf::from(s));
            }
        }
        Ok(p)
    }
}

/// Where the system-wide policy file is looked for on this system.
pub fn system_file() -> Option<PathBuf> {
    system_dir().map(|d| d.join(FILE_NAME))
}

/// The system-wide folder of the policy file (and of [`system_model_folder`]).
pub fn system_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("ProgramData").map(|d| PathBuf::from(d).join("Specular"))
    } else if cfg!(target_os = "macos") {
        Some(PathBuf::from("/Library/Application Support/Specular"))
    } else {
        Some(PathBuf::from("/etc/specular"))
    }
}

/// The folder where administrators may place Local AI model files without a policy:
/// `models` next to the policy file (`/etc/specular/models` on Linux).
pub fn system_model_folder() -> Option<PathBuf> {
    system_dir().map(|d| d.join("models"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_keys_and_ignores_unknown_ones() {
        let p = Policy::parse("no_network = true\nmodel_folder = \"/srv/m\"\nlater = 3\n").unwrap();
        assert!(p.no_network);
        assert_eq!(p.model_folder.as_deref(), Some(Path::new("/srv/m")));
        assert_eq!(Policy::parse("").unwrap(), Policy::default());
        assert!(Policy::parse("no_network = \"yes\"").is_err());
        assert!(Policy::parse("model_folder = 1").is_err());
        assert!(Policy::parse("no_network = [").is_err());
    }

    #[test]
    fn a_missing_or_broken_file_is_no_policy() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        assert_eq!(
            Policy::load_from(&dir.join("absent.toml")),
            Policy::default()
        );
        let broken = dir.join("broken.toml");
        std::fs::write(&broken, "no_network = [").unwrap();
        let p = Policy::load_from(&broken);
        assert!(!p.no_network);
        let good = dir.join("policy.toml");
        std::fs::write(&good, "no_network = true").unwrap();
        let p = Policy::load_from(&good);
        assert!(p.no_network);
        assert_eq!(p.file.as_deref(), Some(good.as_path()));
        assert!(system_file().is_some_and(|f| f.ends_with(FILE_NAME)));
    }
}
