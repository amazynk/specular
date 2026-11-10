//! Import sources: mounted volumes holding a DCIM folder, and a watcher that reports them as
//! they come and go.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use specular_platform::volumes::{self, Volume, VolumeKind};

/// Replaces volume detection with these folders (an OS path list, `:`-separated on Unix), each
/// treated as a mounted card. For tests and GUI checks: nothing real is looked at.
pub const MOUNTS_ENV: &str = "SPECULAR_IMPORT_MOUNTS";

/// A volume with DCIM folders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    /// What to call it: the card's label, the phone's or camera's name.
    pub name: String,
    pub mount: PathBuf,
    /// The DCIM folders to import from (one on a card; one per storage on a phone).
    pub roots: Vec<PathBuf>,
    pub volume: Volume,
}

impl Source {
    pub fn kind(&self) -> VolumeKind {
        self.volume.kind
    }

    pub fn can_eject(&self) -> bool {
        self.volume.can_eject()
    }
}

/// `dir`'s child named `name` (ASCII case-insensitively), if it is a folder.
fn child_dir(dir: &Path, name: &str) -> Option<PathBuf> {
    let exact = dir.join(name);
    if exact.is_dir() {
        return Some(exact);
    }
    std::fs::read_dir(dir).ok()?.flatten().find_map(|e| {
        (e.file_name().to_string_lossy().eq_ignore_ascii_case(name) && e.path().is_dir())
            .then(|| e.path())
    })
}

/// The DCIM folders of a mount: `DCIM` at the top (cards, cameras), or one level down for
/// phones and cameras over MTP/PTP (`Internal shared storage/DCIM`, `store_00010001/DCIM`).
pub fn dcim_roots(mount: &Path, kind: VolumeKind) -> Vec<PathBuf> {
    if let Some(d) = child_dir(mount, "DCIM") {
        return vec![d];
    }
    if kind != VolumeKind::Device {
        return Vec::new();
    }
    let Ok(rd) = std::fs::read_dir(mount) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = rd
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| child_dir(&e.path(), "DCIM"))
        .collect();
    v.sort();
    v
}

/// The sources among `volumes`.
pub fn from_volumes(volumes: Vec<Volume>) -> Vec<Source> {
    volumes
        .into_iter()
        .filter_map(|v| {
            let roots = dcim_roots(&v.mount, v.kind);
            (!roots.is_empty()).then(|| Source {
                name: v.name.clone(),
                mount: v.mount.clone(),
                roots,
                volume: v,
            })
        })
        .collect()
}

/// The sources mounted now ([`MOUNTS_ENV`] when set).
pub fn sources() -> Vec<Source> {
    from_volumes(volumes_now())
}

fn volumes_now() -> Vec<Volume> {
    match std::env::var_os(MOUNTS_ENV) {
        Some(list) => std::env::split_paths(&list)
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| {
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| p.display().to_string());
                Volume::at(p, name, VolumeKind::Removable)
            })
            .collect(),
        None => volumes::list(),
    }
}

/// Polls for sources on its own thread (`io-cards`) and reports the full list whenever it
/// changes (including once at the start). Dropping it stops the thread at its next poll.
pub struct Watcher {
    stop: Arc<AtomicBool>,
}

impl Watcher {
    /// Looks every `every` (cheaply: the mount table first) and calls `changed` with the
    /// sources, once at the start and then whenever they change.
    pub fn start(every: Duration, changed: impl Fn(Vec<Source>) + Send + 'static) -> Watcher {
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        specular_thread::spawn("io-cards", move || {
            // A panic (a strange mount, or `changed`) costs one poll: the thread restarts
            // after a back-off and reports the full list again.
            let quit = || s.load(Ordering::Relaxed);
            specular_log::service::supervise(
                "io-cards",
                specular_log::service::Policy::RESTART,
                quit,
                || {
                    let fake = std::env::var_os(MOUNTS_ENV).is_some();
                    let mut stamp = None;
                    let mut last: Option<Vec<Source>> = None;
                    while !s.load(Ordering::Relaxed) {
                        specular_log::service::trip();
                        let now = if fake { 0 } else { volumes::stamp() };
                        // The fake list has no mount table to watch; a card folder appearing in it is
                        // what the GUI checks simulate, so look every time.
                        if fake || stamp != Some(now) {
                            stamp = Some(now);
                            let found = sources();
                            if last.as_ref() != Some(&found) {
                                changed(found.clone());
                                last = Some(found);
                            }
                        }
                        std::thread::sleep(every);
                    }
                },
                |_| {},
            );
        });
        Watcher { stop }
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{card, fresh, write};

    /// A panic while polling costs that poll: the watcher restarts and reports the sources
    /// again.
    #[test]
    fn card_watch_restarts_after_a_panic() {
        specular_log::service::arm("io-cards", 1);
        let (tx, rx) = std::sync::mpsc::channel();
        let tx = std::sync::Mutex::new(tx);
        let w = Watcher::start(Duration::from_millis(10), move |s| {
            let _ = tx.lock().unwrap().send(s.len());
        });
        rx.recv_timeout(Duration::from_secs(10))
            .expect("sources reported after the restart");
        assert_eq!(specular_log::service::deaths("io-cards"), 1);
        drop(w);
    }

    #[test]
    fn dcim_on_cards_and_phones() {
        let root = fresh("dcim");
        let c = card(&root);
        assert_eq!(dcim_roots(&c, VolumeKind::Removable), [c.join("DCIM")]);

        // As gvfs mounts a phone; `:` cannot be in a Windows file name, and nothing reads it.
        let phone = root.join(if cfg!(windows) {
            "mtp_host=Pixel"
        } else {
            "mtp:host=Pixel"
        });
        write(
            &phone.join("Internal shared storage/DCIM/Camera/PXL_1.jpg"),
            b"x",
        );
        write(&phone.join("SD card/dcim/100ANDRO/DSC_1.jpg"), b"x");
        // On a case-insensitive file system (Windows, macOS) `dcim` is found as `DCIM`: the same folder.
        let key = |v: &[PathBuf]| -> Vec<String> {
            v.iter()
                .map(|p| {
                    let s = p.to_string_lossy().replace('\\', "/");
                    if cfg!(any(windows, target_os = "macos")) {
                        s.to_lowercase()
                    } else {
                        s
                    }
                })
                .collect()
        };
        assert_eq!(
            key(&dcim_roots(&phone, VolumeKind::Device)),
            key(&[
                phone.join("Internal shared storage/DCIM"),
                phone.join("SD card/dcim")
            ])
        );
        assert!(
            dcim_roots(&phone, VolumeKind::Removable).is_empty(),
            "a card's DCIM is at the top"
        );

        let usb = root.join("STICK");
        std::fs::create_dir_all(usb.join("Documents")).unwrap();
        let s = from_volumes(vec![
            Volume::at(&c, "EOS_DIGITAL", VolumeKind::Removable),
            Volume::at(&usb, "STICK", VolumeKind::Removable),
            Volume::at(&phone, "Pixel", VolumeKind::Device),
        ]);
        let names: Vec<_> = s.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            ["EOS_DIGITAL", "Pixel"],
            "a stick without DCIM is no source"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
