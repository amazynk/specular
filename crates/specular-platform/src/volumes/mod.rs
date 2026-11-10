//! Removable volumes, for importing from cameras, phones and memory cards.
//!
//! * [`list`]: the removable volumes mounted now (cards and USB cameras in mass-storage mode,
//!   and on Linux phones and cameras mounted by gvfs over MTP/PTP), each with a display name.
//!   Callers poll it (cheaply gated by [`stamp`]); there is no arrival callback to register.
//! * [`eject`]: unmount and eject a volume, where the platform lets us.
//! * [`free_space`]: bytes available to an unprivileged writer on the volume holding a path.
//!
//! Linux asks udisks2 over D-Bus which mounts are removable (falling back to the
//! `/run/media/$USER` and `/media/$USER` conventions without it) and lists gvfs's FUSE mounts
//! under `/run/user/$UID/gvfs`. macOS lists `/Volumes`, Windows the drive letters other than the
//! system drive; both are compiled but not exercised on real hardware.

use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
mod linux;

/// What kind of mount a volume is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VolumeKind {
    /// A block device: memory card, USB stick, camera in mass-storage mode.
    Removable,
    /// A phone or camera over MTP/PTP, mounted by gvfs as a folder tree.
    Device,
}

/// One mounted removable volume.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Volume {
    /// Where its files are.
    pub mount: PathBuf,
    /// A name to show: the file-system label, the device's name, or the mount's folder name.
    pub name: String,
    pub kind: VolumeKind,
    /// The udisks2 block and drive object paths (Linux), for eject.
    pub(crate) udisks: Option<(String, String)>,
}

impl Volume {
    /// A volume at `mount`, for callers that found it themselves (and for tests).
    pub fn at(mount: impl Into<PathBuf>, name: impl Into<String>, kind: VolumeKind) -> Volume {
        Volume {
            mount: mount.into(),
            name: name.into(),
            kind,
            udisks: None,
        }
    }

    /// Whether [`eject`] can do anything for it.
    pub fn can_eject(&self) -> bool {
        if cfg!(target_os = "linux") {
            self.udisks.is_some()
        } else {
            cfg!(target_os = "macos") && self.kind == VolumeKind::Removable
        }
    }
}

/// Why a volume was not ejected.
#[derive(Debug)]
pub enum EjectError {
    /// The platform (or this kind of mount) offers no way to eject it from here.
    Unsupported,
    /// The system refused, typically because a file on it is still open.
    Failed(String),
}

impl std::fmt::Display for EjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EjectError::Unsupported => write!(f, "cannot be ejected from here"),
            EjectError::Failed(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for EjectError {}

/// The removable volumes mounted now. Cheap enough to call every few seconds, but callers that
/// poll should compare [`stamp`] first: it only reads the mount table.
pub fn list() -> Vec<Volume> {
    #[cfg(target_os = "linux")]
    {
        linux::list()
    }
    #[cfg(target_os = "macos")]
    {
        macos_list()
    }
    #[cfg(windows)]
    {
        windows_list()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        Vec::new()
    }
}

/// Changes whenever the set of mounts may have changed (a hash of the mount table and of the
/// gvfs folder on Linux, of the volume list elsewhere).
pub fn stamp() -> u64 {
    #[cfg(target_os = "linux")]
    {
        linux::stamp()
    }
    #[cfg(not(target_os = "linux"))]
    {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        list().hash(&mut h);
        h.finish()
    }
}

/// Unmounts and ejects `v` (powers it off where it cannot be ejected).
pub fn eject(v: &Volume) -> Result<(), EjectError> {
    #[cfg(target_os = "linux")]
    {
        linux::eject(v)
    }
    #[cfg(target_os = "macos")]
    {
        if v.kind != VolumeKind::Removable {
            return Err(EjectError::Unsupported);
        }
        let out = std::process::Command::new("/usr/sbin/diskutil")
            .arg("eject")
            .arg(&v.mount)
            .output()
            .map_err(|e| EjectError::Failed(e.to_string()))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(EjectError::Failed(
                String::from_utf8_lossy(&out.stderr).trim().to_owned(),
            ))
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = v;
        Err(EjectError::Unsupported)
    }
}

/// Bytes available to this user on the file system holding `path` (the nearest existing
/// ancestor of it), or `None` when the system does not say.
pub fn free_space(path: &Path) -> Option<u64> {
    let mut p = path;
    while !p.exists() {
        p = p.parent()?;
    }
    #[cfg(unix)]
    {
        let s = rustix::fs::statvfs(p).ok()?;
        #[allow(clippy::unnecessary_cast)] // u32 or u64 by target.
        Some((s.f_bavail as u64).saturating_mul(s.f_frsize as u64))
    }
    #[cfg(windows)]
    {
        // The longest mount point holding `p`.
        let p = std::fs::canonicalize(p).ok()?;
        let disks = sysinfo::Disks::new_with_refreshed_list();
        disks
            .list()
            .iter()
            .filter(|d| {
                let m = d.mount_point().to_string_lossy().to_lowercase();
                p.to_string_lossy()
                    .to_lowercase()
                    .trim_start_matches(r"\\?\")
                    .starts_with(&m)
            })
            .max_by_key(|d| d.mount_point().as_os_str().len())
            .map(|d| d.available_space())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = p;
        None
    }
}

/// macOS: everything in `/Volumes` except the start-up disk (a link to `/`). Unverified on
/// hardware: network shares and disk images show too, which is harmless for import (they are
/// only offered when they hold a DCIM folder).
#[cfg(target_os = "macos")]
fn macos_list() -> Vec<Volume> {
    let Ok(rd) = std::fs::read_dir("/Volumes") else {
        return Vec::new();
    };
    let mut v: Vec<Volume> = rd
        .flatten()
        .filter(|e| {
            std::fs::canonicalize(e.path()).is_ok_and(|c| c != Path::new("/")) && e.path().is_dir()
        })
        .map(|e| {
            Volume::at(
                e.path(),
                e.file_name().to_string_lossy().into_owned(),
                VolumeKind::Removable,
            )
        })
        .collect();
    v.sort_by(|a, b| a.mount.cmp(&b.mount));
    v
}

/// Windows: drive letters other than the system drive that are present. Without unsafe Win32
/// calls the drive type is not asked; import only offers those with a DCIM folder, which is
/// what a card or camera has. Unverified on hardware.
#[cfg(windows)]
fn windows_list() -> Vec<Volume> {
    let system = std::env::var("SystemDrive")
        .unwrap_or_else(|_| "C:".into())
        .to_ascii_uppercase();
    (b'D'..=b'Z')
        .chain(*b"AB")
        .map(|l| format!("{}:", l as char))
        .filter(|d| *d != system)
        .filter_map(|d| {
            let root = PathBuf::from(format!("{d}\\"));
            std::fs::metadata(&root)
                .is_ok()
                .then(|| Volume::at(root, d, VolumeKind::Removable))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_space_walks_up_to_an_existing_folder() {
        let dir = std::env::temp_dir();
        let a = free_space(&dir);
        let b = free_space(&dir.join("not/yet/made"));
        assert!(a.is_some());
        assert!(b.is_some());
    }
}
