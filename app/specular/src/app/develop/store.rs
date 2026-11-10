//! Develop's edits on disk: the catalogue (`specular_library::edits`)
//! for catalogued files, and the `<file>.<ext>.specular.xmp` sidecar (on by
//! default, `metadata.develop_sidecar`).
//!
//! - **Load** (background, on opening an image): the sidecar and the catalogue's edit; the
//!   newer of the two wins, except that a sidecar newer than the catalogue's copy with a
//!   different edit is a [`Conflict`]: the user picks. A load that migrated module
//!   params starts a fresh history. The photo's versions (virtual copies) and other tools'
//!   edits come with it.
//! - **Save** (background): one second after the last change, and when the image is left.
//!   The sidecar is written atomically; the catalogue write goes through the indexer.
//! - **Versions:** each has its own edit, history and sidecar (`IMG.CR2.v2.specular.xmp`;
//!   the first keeps the plain name) and catalogue row (with its name).
//! - **Batch** ([`Store::apply`]): a paste, a style, sync or match on photos that are not
//!   open: each one's first version is read, changed by one history step and saved, on a
//!   background thread. Photos whose sidecar changed outside Specular ([`Store::scan`]
//!   finds them first) take the file's edit, the library's, or are skipped
//!   ([`OnConflict`]), as asked once for the batch.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use crossbeam_channel::{Receiver, Sender, unbounded};
use specular_develop::model::migrate::MigrationReport;
use specular_develop::model::{Edit, History, Registry, codec, hash};
use specular_develop::sidecar;
use specular_library::Library;
use specular_library::edits::StoredEdit;

use crate::loader::Loader;

/// Seconds without a change before an edit is saved.
pub(super) const SAVE_AFTER_S: f64 = 1.0;

use specular_develop::model::migrate::NOTICE_NEWER;
/// The edit was made with a newer process version: it may look different here.
pub(super) const NOTICE_NEWER_PROCESS: &str = "toast-develop-edit-newer-process";

/// What a load found.
pub(super) struct Found {
    pub edit: Edit,
    pub history: History,
    /// Fluent ids of the notices to show (module params were updated).
    pub notices: Vec<(&'static str, usize)>,
    /// The version's name (empty: none).
    pub name: String,
}

/// A load's answer: the edit of `version` (if saved) and the photo's versions.
pub(super) struct Loaded {
    pub path: PathBuf,
    pub version: u32,
    /// The library's copy while `conflict` asks.
    pub found: Option<Found>,
    /// Every version with its name (empty: none), in order; at least the first.
    pub versions: Vec<(u32, String)>,
    /// The sidecar changed outside this run: which to keep is asked.
    pub conflict: Option<Conflict>,
    /// Other tools' edits of the photo (version 1 only).
    pub others: super::interop::Others,
    /// The sidecar cannot be fully read (a newer Specular's, or damaged): the notice to
    /// show; nothing of this version is saved while it is open.
    pub locked: Option<&'static str>,
}

/// Notices of a sidecar that is kept as it is (see [`Loaded::locked`]).
pub(super) const NOTICE_PARTIAL: &str = "toast-develop-edit-partial";
pub(super) const NOTICE_UNREADABLE: &str = "toast-develop-sidecar-unreadable";

/// Why the sidecar of `version` must not be written over, if it must not.
pub(super) fn locked(path: &Path, version: u32) -> Option<&'static str> {
    let text = match std::fs::read(sidecar::path_for_version(path, version)) {
        Ok(b) => String::from_utf8(b).ok(),
        Err(_) => return None,
    };
    let Some(text) = text else {
        return Some(NOTICE_UNREADABLE);
    };
    if sidecar::keepable(&text) {
        return None;
    }
    match sidecar::from_xmp(&text, Registry::builtin()) {
        Ok(_) => Some(NOTICE_PARTIAL),
        Err(_) => Some(NOTICE_UNREADABLE),
    }
}

/// A sidecar newer than the library's copy with a different edit.
pub(super) struct Conflict {
    /// When each was saved (seconds since 1970).
    pub file_s: i64,
    pub library_s: i64,
    /// What wrote the sidecar, when it was another Specular.
    pub writer: Option<String>,
    pub file: Found,
}

/// What a batch does with a photo whose sidecar changed outside Specular (a
/// [`Conflict`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum OnConflict {
    /// The change goes on top of the sidecar's edit (the outside change is kept).
    #[default]
    File,
    /// The change goes on top of the library's copy (the outside change is replaced).
    Library,
    /// The photo is left as it is.
    Skip,
}

/// A change to apply to a photo that is not open: `true` when it changed the edit (one
/// history step).
pub(super) type Change = Arc<dyn Fn(&Path, &mut Edit, &mut History) -> bool + Send + Sync>;

/// Loads and saves in the background.
pub(super) struct Store {
    tx: Sender<Loaded>,
    rx: Receiver<Loaded>,
    done_tx: Sender<(u64, usize)>,
    done_rx: Receiver<(u64, usize)>,
    scan_tx: Sender<(u64, Vec<PathBuf>)>,
    scan_rx: Receiver<(u64, Vec<PathBuf>)>,
}

impl Default for Store {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        let (done_tx, done_rx) = unbounded();
        let (scan_tx, scan_rx) = unbounded();
        Store {
            tx,
            rx,
            done_tx,
            done_rx,
            scan_tx,
            scan_rx,
        }
    }
}

impl Store {
    /// Reads version `version` of `path`'s saved edit, and its versions, in the background.
    pub fn load(&self, path: &Path, version: u32, lib: Option<Arc<Library>>, loader: &Loader) {
        let tx = self.tx.clone();
        let p = path.to_path_buf();
        loader.background("develop-load", move |l| {
            let (found, conflict) = load_checked(&p, version, lib.as_deref());
            let versions = versions_of(&p, lib.as_deref());
            let locked = locked(&p, version);
            let others = if version == 0 {
                super::interop::Others::of(&p)
            } else {
                super::interop::Others::default()
            };
            let _ = tx.send(Loaded {
                path: p,
                version,
                found,
                versions,
                conflict,
                others,
                locked,
            });
            l.waker()();
        });
    }

    /// A load that finished.
    pub fn take(&self) -> Option<Loaded> {
        self.rx.try_recv().ok()
    }

    /// Saves `edit` and `history` of version `version` of `path` in the background.
    #[allow(clippy::too_many_arguments)]
    pub fn save(
        &self,
        path: &Path,
        version: u32,
        name: &str,
        edit: &Edit,
        history: &History,
        lib: Option<Arc<Library>>,
        sidecar_on: bool,
        loader: &Loader,
    ) {
        let (p, e, h, n) = (
            path.to_path_buf(),
            edit.clone(),
            history.clone(),
            name.to_owned(),
        );
        loader.background("develop-save", move |l| {
            let side = save_version(&p, version, &n, &e, &h, lib.as_deref(), sidecar_on);
            saved(p, version, side, None);
            l.waker()();
        });
    }

    /// Forgets version `version` of `path` (its sidecar and catalogue row) in the
    /// background. The image is never touched.
    pub fn delete(&self, path: &Path, version: u32, lib: Option<Arc<Library>>, loader: &Loader) {
        let p = path.to_path_buf();
        loader.background("develop-delete", move |_| {
            if let Err(e) = sidecar::remove_version(&p, version) {
                specular_log::warn!(target: specular_log::FOLDER, "develop: cannot remove a version sidecar of {}: {e}", p.display());
            }
            if let Some(l) = lib {
                set_pending(&p, version, None);
                l.delete_edit(p.clone(), version);
            }
            super::edited::changed(&p);
        });
    }

    /// Applies `change` to the first version of each of `paths` in the background (one
    /// history step each) and saves them; [`Store::applied`] reports `(token, changed)`
    /// when all are done.
    #[allow(clippy::too_many_arguments)]
    pub fn apply(
        &self,
        token: u64,
        paths: Vec<PathBuf>,
        change: Change,
        on_conflict: OnConflict,
        lib: Option<Arc<Library>>,
        sidecar_on: bool,
        loader: &Loader,
    ) {
        let tx = self.done_tx.clone();
        loader.background("develop-batch", move |l| {
            let mut changed = 0;
            for p in &paths {
                let found = match load_checked(p, 0, lib.as_deref()) {
                    (found, None) => found,
                    (found, Some(c)) => match on_conflict {
                        OnConflict::File => Some(c.file),
                        OnConflict::Library => found,
                        OnConflict::Skip => continue,
                    },
                };
                let (mut e, mut h, name) = match found {
                    Some(f) => (f.edit, f.history, f.name),
                    None => (
                        super::controls::new_edit(p),
                        History::default(),
                        String::new(),
                    ),
                };
                if change(p, &mut e, &mut h) {
                    let side = save_version(p, 0, &name, &e, &h, lib.as_deref(), sidecar_on);
                    saved(p.clone(), 0, side, Some(token));
                    changed += 1;
                }
            }
            let _ = tx.send((token, changed));
            l.waker()();
        });
    }

    /// Finds which of `paths` have a sidecar that changed outside Specular (their first
    /// version), in the background; [`Store::scanned`] gives them with `token`.
    pub fn scan(
        &self,
        token: u64,
        paths: Vec<PathBuf>,
        lib: Option<Arc<Library>>,
        loader: &Loader,
    ) {
        let tx = self.scan_tx.clone();
        loader.background("develop-scan", move |l| {
            let _ = tx.send((token, conflicts(&paths, lib.as_deref())));
            l.waker()();
        });
    }

    /// A scan that finished: its token and the photos in conflict.
    pub fn scanned(&self) -> Option<(u64, Vec<PathBuf>)> {
        self.scan_rx.try_recv().ok()
    }

    /// A batch that finished: its token and how many photos changed.
    pub fn applied(&self) -> Option<(u64, usize)> {
        self.done_rx.try_recv().ok()
    }
}

/// Those of `paths` whose first version's sidecar changed outside Specular.
pub(super) fn conflicts(paths: &[PathBuf], lib: Option<&Library>) -> Vec<PathBuf> {
    // Without a library there is no second copy to differ from.
    if lib.is_none() {
        return Vec::new();
    }
    paths
        .iter()
        .filter(|p| load_checked(p, 0, lib).1.is_some())
        .cloned()
        .collect()
}

fn now_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// The newer of the sidecar's and the catalogue's edit of `path` (its first version).
pub(super) fn load(path: &Path, lib: Option<&Library>) -> Option<Found> {
    load_version(path, 0, lib)
}

/// The newer of the sidecar's and the catalogue's edit of version `version` of `path`.
/// The newest of the sidecar and the library's copy (batch work, which asks nothing).
pub(super) fn load_version(path: &Path, version: u32, lib: Option<&Library>) -> Option<Found> {
    let (found, conflict) = load_checked(path, version, lib);
    match conflict {
        Some(c) => Some(c.file),
        None => found,
    }
}

/// The edit of `version`: the library's copy and the sidecar's, the newer one, unless the
/// sidecar is newer with a different edit; then the library's, and the [`Conflict`].
pub(super) fn load_checked(
    path: &Path,
    version: u32,
    lib: Option<&Library>,
) -> (Option<Found>, Option<Conflict>) {
    let reg = Registry::builtin();
    let side = match sidecar::read_version(path, version, reg) {
        Ok(Some(l)) => {
            let t = std::fs::metadata(sidecar::path_for_version(path, version))
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs() as i64);
            // Modules of a newer Specular are kept in the file but not shown.
            let mut extra = Vec::new();
            if !l.unknown_modules.is_empty() {
                extra.push((NOTICE_NEWER, l.unknown_modules.len()));
            }
            if l.newer_process {
                extra.push((NOTICE_NEWER_PROCESS, 1));
            }
            Some((
                t,
                l.edit,
                l.history,
                l.report,
                l.name.unwrap_or_default(),
                l.writer,
                extra,
            ))
        }
        Ok(None) => None,
        Err(e) => {
            specular_log::warn!(target: specular_log::FOLDER, "develop: sidecar of {}: {e}", path.display());
            None
        }
    };
    let cat = lib
        .and_then(|l| pending_or(path, version, l.load_edit(path, version).ok().flatten()))
        .and_then(|s| {
            let (edit, report) = codec::decode_edit(&s.edit, reg).ok()?;
            let steps: Option<Vec<_>> = s
                .steps
                .iter()
                .map(|b| codec::decode_step(b, reg).ok())
                .collect();
            let history = match steps {
                Some(steps) if report.migrated.is_empty() => History {
                    position: (s.position as usize).min(steps.len()),
                    steps,
                },
                _ => History::default(),
            };
            Some((s.updated, edit, history, report, s.name, None, Vec::new()))
        });
    type Side = (
        i64,
        Edit,
        History,
        MigrationReport,
        String,
        Option<String>,
        Vec<(&'static str, usize)>,
    );
    let found = |(_, edit, history, report, name, _, extra): Side| {
        let mut notices: Vec<(&'static str, usize)> = report
            .notices()
            .into_iter()
            .map(|n| (n.id, n.count))
            .collect();
        for (id, n) in extra {
            match notices.iter_mut().find(|x| x.0 == id) {
                Some(x) => x.1 += n,
                None => notices.push((id, n)),
            }
        }
        Found {
            edit,
            history,
            notices,
            name,
        }
    };
    match (side, cat) {
        (Some(a), Some(b)) => {
            if a.0 > b.0 && hash::edit_hash(&a.1) != hash::edit_hash(&b.1) {
                let (file_s, library_s, writer) = (a.0, b.0, a.5.clone());
                (
                    Some(found(b)),
                    Some(Conflict {
                        file_s,
                        library_s,
                        writer: writer.filter(|w| w.as_str() != sidecar::WRITER),
                        file: found(a),
                    }),
                )
            } else if a.0 > b.0 {
                (Some(found(a)), None)
            } else {
                (Some(found(b)), None)
            }
        }
        (Some(a), None) | (None, Some(a)) => (Some(found(a)), None),
        (None, None) => (None, None),
    }
}

/// The versions of `path`: the first always, then those with a sidecar or a catalogue row,
/// with their names.
pub(super) fn versions_of(path: &Path, lib: Option<&Library>) -> Vec<(u32, String)> {
    let mut v: Vec<(u32, String)> = lib
        .and_then(|l| l.edit_versions(path).ok())
        .unwrap_or_default();
    // Saves and deletions on their way to the catalogue.
    if lib.is_some()
        && let Ok(p) = PENDING.lock()
        && let Some(map) = p.as_ref()
    {
        for ((at, n), e) in map {
            if at != path {
                continue;
            }
            v.retain(|(k, _)| k != n);
            if let Some(e) = e {
                v.push((*n, e.name.clone()));
            }
        }
    }
    for n in sidecar::versions_on_disk(path) {
        if !v.iter().any(|(k, _)| *k == n) {
            let name = sidecar::read_version(path, n, Registry::builtin())
                .ok()
                .flatten()
                .and_then(|l| l.name)
                .unwrap_or_default();
            v.push((n, name));
        }
    }
    if !v.iter().any(|(k, _)| *k == 0) {
        v.push((0, String::new()));
    }
    v.sort_by_key(|x| x.0);
    v
}

/// Catalogue writes on their way (they go through the indexer, which may be busy): each
/// version's last save (None: deleted). Reads take them until the catalogue has them, so
/// a photo read again after a save (its edited thumbnail) never gets the older copy, also
/// with sidecars off.
type Pending = std::collections::HashMap<(PathBuf, u32), Option<StoredEdit>>;
static PENDING: std::sync::Mutex<Option<Pending>> = std::sync::Mutex::new(None);

fn set_pending(path: &Path, version: u32, e: Option<StoredEdit>) {
    if let Ok(mut p) = PENDING.lock() {
        p.get_or_insert_with(Default::default)
            .insert((path.to_path_buf(), version), e);
    }
}

/// The catalogue's copy `cat` of `path`'s version, or the save on its way when the
/// catalogue doesn't have it yet (the pending entry goes once it has).
fn pending_or(path: &Path, version: u32, cat: Option<StoredEdit>) -> Option<StoredEdit> {
    let Ok(mut guard) = PENDING.lock() else {
        return cat;
    };
    let Some(map) = guard.as_mut() else {
        return cat;
    };
    let key = (path.to_path_buf(), version);
    let Some(want) = map.get(&key) else {
        return cat;
    };
    let arrived = match (want, &cat) {
        (Some(w), Some(c)) => w.hash == c.hash && w.updated <= c.updated,
        (None, None) => true,
        _ => false,
    };
    if arrived {
        map.remove(&key);
        cat
    } else {
        want.clone()
    }
}

/// Version `version` of an edit as the catalogue stores it.
fn stored(edit: &Edit, history: &History, name: &str, updated: i64) -> StoredEdit {
    let (modules, styles) = super::edited::summary_of(edit, history);
    StoredEdit {
        edit: codec::encode_edit(edit),
        hash: hash::edit_hash(edit).to_vec(),
        steps: history.steps.iter().map(codec::encode_step).collect(),
        position: history.position as u32,
        updated,
        name: name.to_owned(),
        modules: Some(modules),
        styles: Some(styles),
    }
}

/// How the Library's walks read Develop sidecars it has no edit for (so its `edited:`
/// then agrees with the folder view's): the sidecar's version, timed by the file.
pub(in crate::app) fn edit_import() -> specular_library::EditImport {
    specular_library::EditImport {
        suffix: sidecar::SUFFIX,
        read: Arc::new(|path: &Path, version: u32| {
            let l = sidecar::read_version(path, version, Registry::builtin()).ok()??;
            let t = std::fs::metadata(sidecar::path_for_version(path, version))
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs() as i64);
            Some(stored(
                &l.edit,
                &l.history,
                l.name.as_deref().unwrap_or_default(),
                t,
            ))
        }),
    }
}

/// A Develop save that was written, for the `develop-saved` event (`events.rs`).
pub(in crate::app) struct Saved {
    pub path: PathBuf,
    pub version: u32,
    /// The sidecar written (none: off, or waiting for an offline original).
    pub sidecar: Option<PathBuf>,
    /// The batch it belongs to ([`Store::apply`]'s token).
    pub token: Option<u64>,
}

/// Saves written and not yet told, oldest first (at most [`MAX_SAVED`]).
static SAVED: parking_lot::Mutex<Vec<Saved>> = parking_lot::Mutex::new(Vec::new());
const MAX_SAVED: usize = 100_000;

fn saved(path: PathBuf, version: u32, sidecar: Option<PathBuf>, token: Option<u64>) {
    let mut s = SAVED.lock();
    if s.len() < MAX_SAVED {
        s.push(Saved {
            path,
            version,
            sidecar,
            token,
        });
    }
}

/// The saves written since the last call.
pub(in crate::app) fn take_saved() -> Vec<Saved> {
    std::mem::take(&mut *SAVED.lock())
}

/// Writes the sidecar (when on) and the catalogue's copy of version `version`; the sidecar
/// written, if one was.
pub(super) fn save_version(
    path: &Path,
    version: u32,
    name: &str,
    edit: &Edit,
    history: &History,
    lib: Option<&Library>,
    sidecar_on: bool,
) -> Option<PathBuf> {
    let named = (!name.is_empty()).then_some(name);
    // Offline (a local copy is being edited): the sidecar waits for the original; the
    // catalogue's copy below is what it is written from when the original is back.
    let waits = sidecar_on && crate::app::local_copies::queue_edit(path, version);
    let mut side = None;
    if sidecar_on && !waits {
        match sidecar::write_version(path, version, named, edit, Some(history)) {
            Ok(_) => side = Some(sidecar::path_for_version(path, version)),
            Err(e) => {
                specular_log::warn!(target: specular_log::FOLDER, "develop: cannot write the sidecar of {}: {e}", path.display());
            }
        }
    }
    if let Some(l) = lib {
        let e = stored(edit, history, name, now_s());
        set_pending(path, version, Some(e.clone()));
        l.save_edit(path.to_path_buf(), version, e);
    }
    // Its tile re-develops.
    super::edited::changed(path);
    side
}

/// The catalogue's copy of version `version` of `path` (local copies: what an offline edit
/// saved), as its edit, history and name.
pub(in crate::app) fn library_copy(
    path: &Path,
    version: u32,
    lib: &Library,
) -> Option<(Edit, History, String)> {
    let reg = Registry::builtin();
    let s = pending_or(path, version, lib.load_edit(path, version).ok().flatten())?;
    let (edit, _) = codec::decode_edit(&s.edit, reg).ok()?;
    let steps: Option<Vec<_>> = s
        .steps
        .iter()
        .map(|b| codec::decode_step(b, reg).ok())
        .collect();
    let history = steps.map_or_else(History::default, |steps| History {
        position: (s.position as usize).min(steps.len()),
        steps,
    });
    Some((edit, history, s.name))
}

/// Writes the sidecar of version `version` of `path` from the catalogue's copy (a change
/// that waited while the original was offline). False when the catalogue has none.
pub(in crate::app) fn write_sidecar_from_library(
    path: &Path,
    version: u32,
    lib: &Library,
) -> std::io::Result<bool> {
    let Some((edit, history, name)) = library_copy(path, version, lib) else {
        return Ok(false);
    };
    let named = (!name.is_empty()).then_some(name.as_str());
    sidecar::write_version(path, version, named, &edit, Some(&history))?;
    Ok(true)
}

/// The sidecar's edit of version `version` of `path` becomes the catalogue's (local copies:
/// the user kept the file's edit over one made offline). False when it cannot be read.
pub(in crate::app) fn adopt_sidecar(path: &Path, version: u32, lib: &Library) -> bool {
    let Ok(Some(l)) = sidecar::read_version(path, version, Registry::builtin()) else {
        return false;
    };
    let e = stored(&l.edit, &l.history, &l.name.unwrap_or_default(), now_s());
    set_pending(path, version, Some(e.clone()));
    lib.save_edit(path.to_path_buf(), version, e);
    super::edited::changed(path);
    true
}

/// When the sidecar of version `version` of `path` was last written (seconds since 1970),
/// for local copies' conflict question.
pub(in crate::app) fn sidecar_saved(path: &Path, version: u32) -> Option<i64> {
    let p = sidecar::path_for_version(path, version);
    let t = std::fs::metadata(p).and_then(|m| m.modified()).ok()?;
    t.duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs() as i64)
}

/// [`save_version`] for the first version.
#[cfg(test)]
pub(super) fn save(
    path: &Path,
    edit: &Edit,
    history: &History,
    lib: Option<&Library>,
    sidecar_on: bool,
) {
    let _ = save_version(path, 0, "", edit, history, lib, sidecar_on);
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_develop::model::{ModuleKind, Value};

    #[test]
    fn a_saved_edit_loads_back_from_the_sidecar() {
        let dir = std::env::temp_dir().join(format!("specular-dv5-store-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let img = dir.join("IMG_0001.JPG");
        assert!(load(&img, None).is_none());
        let mut e = super::super::controls::new_edit(&img);
        let m = e.module_mut(ModuleKind::EXPOSURE, 0).unwrap();
        let mut f = m.params.to_fields();
        f.set("exposure_ev", Value::Float(0.75));
        m.params.set_fields(f);
        save(&img, &e, &History::default(), None, true);
        let back = load(&img, None).unwrap();
        assert_eq!(back.edit, e);
        assert!(back.notices.is_empty());
        // Off: nothing is written.
        let other = dir.join("IMG_0002.JPG");
        save(&other, &e, &History::default(), None, false);
        assert!(!sidecar::path_for(&other).exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn versions_load_with_their_names() {
        let dir = std::env::temp_dir().join(format!("specular-dv16-store-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let img = dir.join("IMG_0003.JPG");
        assert_eq!(versions_of(&img, None), vec![(0, String::new())]);
        let e = super::super::controls::new_edit(&img);
        save_version(&img, 2, "Square crop", &e, &History::default(), None, true);
        assert_eq!(
            versions_of(&img, None),
            vec![(0, String::new()), (2, "Square crop".into())]
        );
        let f = load_version(&img, 2, None).unwrap();
        assert_eq!(f.name, "Square crop");
        assert!(load_version(&img, 1, None).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// With sidecars off, a read right after a save gets the
    /// saved edit even while the catalogue's write waits on a busy indexer (the edited
    /// thumbnail used to stay on the older edit); the save on its way goes once it lands.
    #[test]
    fn reads_after_a_save_never_get_the_older_catalogue_copy() {
        let edit = |ev: f32| {
            let mut e = super::super::controls::new_edit(Path::new("late.jpg"));
            let m = e.module_mut(ModuleKind::EXPOSURE, 0).unwrap();
            let mut f = m.params.to_fields();
            f.set("exposure_ev", Value::Float(ev));
            m.params.set_fields(f);
            e
        };
        let p = Path::new("/nowhere/late.jpg");
        let (old, new) = (
            stored(&edit(0.5), &History::default(), "", 100),
            stored(&edit(1.0), &History::default(), "", 200),
        );
        // Nothing on its way: the catalogue's copy.
        assert_eq!(pending_or(p, 0, Some(old.clone())), Some(old.clone()));
        set_pending(p, 0, Some(new.clone()));
        // The indexer hasn't written it: the save, not the older copy (twice: it stays).
        assert_eq!(pending_or(p, 0, Some(old.clone())), Some(new.clone()));
        assert_eq!(pending_or(p, 0, None), Some(new.clone()));
        // Written: the catalogue's, and the entry goes.
        assert_eq!(pending_or(p, 0, Some(new.clone())), Some(new.clone()));
        assert_eq!(pending_or(p, 0, Some(old.clone())), Some(old.clone()));
        // A deletion on its way hides the row until it goes.
        set_pending(p, 3, None);
        assert_eq!(pending_or(p, 3, Some(old.clone())), None);
        assert_eq!(pending_or(p, 3, None), None);
        assert_eq!(pending_or(p, 3, Some(old.clone())), Some(old));

        // End to end with a Library and sidecars off: the photo's facts (its thumbnail's
        // hash) follow each save at once.
        let root = std::env::temp_dir().join(format!("specular-late-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("p")).unwrap();
        // The Library keeps canonical paths (/var is /private/var on macOS).
        let root = specular_platform::path::canonicalize(&root).unwrap();
        let img = root.join("p/late.jpg");
        std::fs::write(&img, b"x").unwrap();
        let mut cfg = specular_library::Config::new(root.join("library.sqlite3"));
        cfg.watch = false;
        cfg.is_image = Arc::new(|p: &Path| p.extension().is_some_and(|e| e == "jpg"));
        let lib = Library::open(cfg).unwrap();
        lib.add_location(&root.join("p"));
        assert!(lib.wait_idle(std::time::Duration::from_secs(20)));
        for ev in [0.25f32, 0.5, 0.75, 1.0] {
            let e = edit(ev);
            save(&img, &e, &History::default(), Some(&lib), false);
            let f = super::super::edited::facts_of(&img, Some(&lib));
            assert_eq!(f.len(), 1);
            assert_eq!(f[0].hash, hash::edit_hash(&e), "{ev}");
        }
        assert!(lib.wait_idle(std::time::Duration::from_secs(20)));
        assert_eq!(
            lib.load_edit(&img, 0).unwrap().map(|s| s.hash),
            Some(hash::edit_hash(&edit(1.0)).to_vec())
        );
        drop(lib);
        let _ = std::fs::remove_dir_all(&root);
    }
}
