//! Live folder updates: a `notify` watcher whose events are collected and handed to the
//! gallery once they settle (a camera import writes hundreds of files; the grid relists once).
//!
//! Symbolic links to folders are never followed. Where the platform has no native recursive
//! watch (inotify, kqueue), `notify` recurses by walking the whole tree and adding a watch per
//! folder, synchronously, hidden and cache folders included; the Library therefore uses
//! [`FolderWatch::folders`], which watches just the folders its walk indexes, one at a time.
//! The gallery and cull use [`TreeWatch`], which sets such a watch up on its own thread.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use notify::{RecursiveMode, Watcher as _};
use parking_lot::Mutex;

/// Changes are handed out once no new event arrived for this long.
pub const SETTLE: Duration = Duration::from_millis(250);

/// The platform watches a whole tree natively with one cheap watch (FSEvents,
/// ReadDirectoryChangesW), so [`FolderWatch::folders`] watches the root recursively.
const NATIVE_RECURSIVE: bool = cfg!(any(target_os = "macos", target_os = "windows"));

/// Per-folder watches of all [`FolderWatch::folders`] watchers together.
static PER_FOLDER: AtomicUsize = AtomicUsize::new(0);

/// At most this many per-folder watches: half the user's inotify limit (other programs need
/// theirs too), and never more than 65 536 (each costs the kernel about 1 KiB).
fn per_folder_cap() -> usize {
    static CAP: OnceLock<usize> = OnceLock::new();
    *CAP.get_or_init(|| {
        std::fs::read_to_string("/proc/sys/fs/inotify/max_user_watches")
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .map_or(8192, |n| n / 2)
            .min(PER_FOLDER_MAX)
    })
}

/// The most per-folder watches Specular takes ([`per_folder_cap`]), and the system's limit
/// it comes from (Linux's `fs.inotify.max_user_watches`; `None` where there is none to
/// read). Raising the system's limit helps only while the first is below
/// [`PER_FOLDER_MAX`].
pub fn per_folder_limit() -> (usize, Option<usize>) {
    let system = std::fs::read_to_string("/proc/sys/fs/inotify/max_user_watches")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok());
    (per_folder_cap(), system)
}

/// Per-folder watches are never more than this, whatever the system allows.
pub const PER_FOLDER_MAX: usize = 65_536;

/// A folder's identity: a folder renamed and listed again under its new name shares the
/// kernel watch of its old name.
type Ident = Option<(u64, u64)>;

#[cfg(unix)]
fn ident(dir: &Path) -> Ident {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(dir).ok().map(|m| (m.dev(), m.ino()))
}

#[cfg(not(unix))]
fn ident(_: &Path) -> Ident {
    None
}

#[derive(Default)]
struct Pending {
    paths: HashSet<PathBuf>,
    last: Option<Instant>,
    /// Something happened that needs a full relist (rename of the folder, overflow).
    rescan: bool,
}

/// The folders of a per-folder watch.
#[derive(Default)]
struct Folders {
    dirs: BTreeMap<PathBuf, Ident>,
    /// How many of `dirs` share each identity.
    idents: HashMap<(u64, u64), u32>,
    /// A folder went unwatched for want of watches.
    missed: bool,
}

pub struct FolderWatch {
    watcher: notify::RecommendedWatcher,
    pending: Arc<Mutex<Pending>>,
    pub dir: PathBuf,
    /// Per-folder: the folders are added one at a time ([`FolderWatch::add`]).
    folders: Option<Folders>,
}

/// What changed, once settled.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub paths: Vec<PathBuf>,
    pub rescan: bool,
}

impl FolderWatch {
    /// Watches `dir` (and below when `recursive`). `wake` is called from the watcher thread on
    /// every event batch.
    pub fn new(
        dir: &Path,
        recursive: bool,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> notify::Result<FolderWatch> {
        let mut w = FolderWatch::unwatched(dir, wake, Arc::default(), None)?;
        w.watcher.watch(
            dir,
            if recursive {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            },
        )?;
        Ok(w)
    }

    /// Watches the tree at `root` a folder at a time: nothing is watched until folders are
    /// [added](FolderWatch::add). Where the platform watches trees natively, the whole tree
    /// is watched at once instead, and adding does nothing.
    pub fn folders(root: &Path, wake: Arc<dyn Fn() + Send + Sync>) -> notify::Result<FolderWatch> {
        if NATIVE_RECURSIVE {
            return FolderWatch::new(root, true, wake);
        }
        let mut w = FolderWatch::unwatched(root, wake, Arc::default(), None)?;
        w.folders = Some(Folders::default());
        Ok(w)
    }

    /// A watcher that watches nothing yet, collecting into `pending`; with `tap`, events that
    /// may add or remove folders are also sent there (see [`TreeWatch`]).
    fn unwatched(
        dir: &Path,
        wake: Arc<dyn Fn() + Send + Sync>,
        pending: Arc<Mutex<Pending>>,
        tap: Option<Sender<Tap>>,
    ) -> notify::Result<FolderWatch> {
        use notify::EventKind;
        let p = pending.clone();
        // FSEvents reports paths with symlinks resolved (/var is /private/var): put them back
        // under `dir` as it was given, which is how the callers know the files.
        let alias = std::fs::canonicalize(dir)
            .ok()
            .filter(|real| cfg!(target_os = "macos") && real.as_path() != dir)
            .map(|real| (real, dir.to_path_buf()));
        let watcher = notify::RecommendedWatcher::new(
            move |ev: notify::Result<notify::Event>| {
                let mut st = p.lock();
                let mut tapped = None;
                match ev {
                    Ok(mut ev) => {
                        if let Some((real, given)) = &alias {
                            for path in &mut ev.paths {
                                if let Ok(rel) = path.strip_prefix(real) {
                                    *path = given.join(rel);
                                }
                            }
                        }
                        if ev.need_rescan() {
                            st.rescan = true;
                            tapped = Some(Tap::Rewalk);
                        }
                        if matches!(ev.kind, EventKind::Access(_)) {
                            return;
                        }
                        if tapped.is_none()
                            && matches!(
                                ev.kind,
                                EventKind::Create(_)
                                    | EventKind::Remove(_)
                                    | EventKind::Modify(notify::event::ModifyKind::Name(_))
                            )
                        {
                            tapped = Some(Tap::Paths(ev.paths.clone()));
                        }
                        st.paths.extend(ev.paths);
                    }
                    Err(_) => st.rescan = true,
                }
                st.last = Some(Instant::now());
                drop(st);
                if let (Some(tap), Some(t)) = (&tap, tapped) {
                    let _ = tap.send(t);
                }
                wake();
            },
            notify::Config::default().with_follow_symlinks(false),
        )?;
        Ok(FolderWatch {
            watcher,
            pending,
            dir: dir.to_path_buf(),
            folders: None,
        })
    }

    /// Watches folder `dir` (not below it) of a per-folder watch. A folder that cannot be
    /// watched is skipped; once the watches run out, [`FolderWatch::missed`] says so. Returns
    /// whether `dir` was unwatched before (and may have changed unseen).
    pub fn add(&mut self, dir: &Path) -> bool {
        let Some(f) = &mut self.folders else {
            return false;
        };
        if f.dirs.contains_key(dir) {
            return false;
        }
        if PER_FOLDER.fetch_add(1, Ordering::Relaxed) >= per_folder_cap() {
            PER_FOLDER.fetch_sub(1, Ordering::Relaxed);
            f.missed = true;
            return true;
        }
        let id = ident(dir);
        match self.watcher.watch(dir, RecursiveMode::NonRecursive) {
            Ok(()) => {
                f.dirs.insert(dir.to_path_buf(), id);
                if let Some(id) = id {
                    *f.idents.entry(id).or_default() += 1;
                }
            }
            Err(e) => {
                PER_FOLDER.fetch_sub(1, Ordering::Relaxed);
                if matches!(e.kind, notify::ErrorKind::MaxFilesWatch) {
                    f.missed = true;
                } else {
                    specular_log::debug!(target: specular_log::FOLDER, "watch: a folder cannot be watched: {e}");
                }
            }
        }
        true
    }

    /// Stops watching folder `dir` and the folders below it (they are gone).
    pub fn remove_below(&mut self, dir: &Path) {
        let Some(f) = &mut self.folders else {
            return;
        };
        let gone: Vec<(PathBuf, Ident)> = f
            .dirs
            .range(dir.to_path_buf()..)
            .take_while(|(p, _)| p.starts_with(dir))
            .map(|(p, i)| (p.clone(), *i))
            .collect();
        for (p, id) in gone {
            f.dirs.remove(&p);
            PER_FOLDER.fetch_sub(1, Ordering::Relaxed);
            if let Some(id) = id
                && let Some(n) = f.idents.get_mut(&id)
            {
                *n -= 1;
                if *n > 0 {
                    // Renamed, and already watched under its new name: the same watch.
                    continue;
                }
                f.idents.remove(&id);
            }
            // Fails when the kernel already dropped the watch of a deleted folder.
            let _ = self.watcher.unwatch(&p);
        }
    }

    /// Folders watched one at a time.
    pub fn watched(&self) -> usize {
        self.folders.as_ref().map_or(0, |f| f.dirs.len())
    }

    /// Some folder went unwatched for want of watches: the tree needs walking now and then.
    pub fn missed(&self) -> bool {
        self.folders.as_ref().is_some_and(|f| f.missed)
    }

    /// The settled changes, or `None` while events are still arriving (then call again after
    /// [`FolderWatch::due`]).
    pub fn take(&self, now: Instant) -> Option<Changes> {
        take(&self.pending, now)
    }

    /// When the pending changes settle.
    pub fn due(&self) -> Option<Instant> {
        self.pending.lock().last.map(|l| l + SETTLE)
    }
}

impl Drop for FolderWatch {
    fn drop(&mut self) {
        PER_FOLDER.fetch_sub(self.watched(), Ordering::Relaxed);
    }
}

/// The settled changes of `pending` (see [`FolderWatch::take`]).
fn take(pending: &Mutex<Pending>, now: Instant) -> Option<Changes> {
    let mut st = pending.lock();
    let last = st.last?;
    if now.duration_since(last) < SETTLE {
        return None;
    }
    st.last = None;
    let mut paths: Vec<PathBuf> = st.paths.drain().collect();
    paths.sort();
    Some(Changes {
        paths,
        rescan: std::mem::take(&mut st.rescan),
    })
}

/// What a [`TreeWatch`]'s watcher hands its keeper thread.
enum Tap {
    /// Folders may have appeared, gone or moved here.
    Paths(Vec<PathBuf>),
    /// Events were lost (queue overflow): every folder is looked at again.
    Rewalk,
    /// The [`TreeWatch`] was dropped.
    Stop,
}

/// Folders over the watch cap are walked this often, to notice changes in them.
pub const REWALK: Duration = Duration::from_secs(15 * 60);

/// A folder changed this close before its watch started counts as changed after it: file
/// times come from a coarse clock, and some file systems keep them to a second or two.
const SLACK: Duration = Duration::from_secs(1);

#[derive(Default)]
struct TreeState {
    cancel: AtomicBool,
    ready: AtomicBool,
    failed: AtomicBool,
    watched: AtomicUsize,
}

/// A folder watch (with a depth, of its subfolders too) set up on a thread of its own, so
/// that opening a folder never waits for it: [`TreeWatch::start`] returns at once.
///
/// - Recursion uses the per-folder scheme of [`FolderWatch::folders`]: a walk adds one watch
///   per folder down to `depth` levels below the root (hidden folders only with
///   `show_hidden`), never through symbolic links; folders created, moved in, moved out or
///   deleted later are followed. Folders over the watch cap are walked every [`REWALK`].
/// - Nothing is lost while the walk runs: events in folders already watched are collected as
///   usual, and a folder that changed after `start` but before its own watch began is reported
///   as a rescan (with the files written into it since).
/// - Dropping the `TreeWatch` cancels a walk still running; the watches are let go on the
///   keeper thread, not the caller's.
pub struct TreeWatch {
    dir: PathBuf,
    depth: u32,
    show_hidden: bool,
    pending: Arc<Mutex<Pending>>,
    state: Arc<TreeState>,
    tx: Sender<Tap>,
}

impl TreeWatch {
    /// Starts watching `dir` and the folders up to `depth` levels below it, on a thread named
    /// `thread` (see `specular-thread`). `wake` is called on every event batch, and once more
    /// when the walk found changes it could not have seen as events.
    pub fn start(
        thread: &str,
        dir: &Path,
        depth: u32,
        show_hidden: bool,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> TreeWatch {
        let since = SystemTime::now();
        let pending: Arc<Mutex<Pending>> = Arc::default();
        let state: Arc<TreeState> = Arc::default();
        let (tx, rx) = std::sync::mpsc::channel();
        let keeper = Keeper {
            root: dir.to_path_buf(),
            depth,
            show_hidden,
            pending: pending.clone(),
            state: state.clone(),
            wake,
        };
        let tap = tx.clone();
        let name = thread.to_owned();
        let spawned = specular_thread::builder(thread).spawn(move || {
            // A panic costs the watches of that run: the owner relists (anything may have
            // changed meanwhile) and the watch is set up again after a back-off.
            let since = std::cell::Cell::new(since);
            specular_log::service::supervise(
                &name,
                specular_log::service::Policy::RESTART,
                || keeper.state.cancel.load(Ordering::Relaxed),
                || keeper.clone().run(&rx, &tap, since.get()),
                |_| {
                    since.set(SystemTime::now());
                    keeper.rescan();
                },
            );
        });
        if let Err(e) = spawned {
            specular_log::debug!(target: specular_log::FOLDER, "watch: no thread: {e}");
            state.failed.store(true, Ordering::Release);
        }
        TreeWatch {
            dir: dir.to_path_buf(),
            depth,
            show_hidden,
            pending,
            state,
            tx,
        }
    }

    /// Whether this watches `dir` as asked (else a new one is needed).
    pub fn is(&self, dir: &Path, depth: u32, show_hidden: bool) -> bool {
        self.dir == dir && self.depth == depth && (depth == 0 || self.show_hidden == show_hidden)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The walk is done: every folder is watched (or over the cap).
    pub fn ready(&self) -> bool {
        self.state.ready.load(Ordering::Acquire)
    }

    /// The folder cannot be watched at all.
    pub fn failed(&self) -> bool {
        self.state.failed.load(Ordering::Acquire)
    }

    /// Folders watched one at a time so far (0 for a single or native recursive watch).
    pub fn watched(&self) -> usize {
        self.state.watched.load(Ordering::Relaxed)
    }

    /// See [`FolderWatch::take`].
    pub fn take(&self, now: Instant) -> Option<Changes> {
        take(&self.pending, now)
    }

    /// See [`FolderWatch::due`].
    pub fn due(&self) -> Option<Instant> {
        self.pending.lock().last.map(|l| l + SETTLE)
    }
}

impl Drop for TreeWatch {
    fn drop(&mut self) {
        self.state.cancel.store(true, Ordering::Relaxed);
        let _ = self.tx.send(Tap::Stop);
    }
}

/// How a walk tells that a folder it starts watching may hold changes no event reported.
#[derive(Clone, Copy)]
enum Unseen {
    /// Modified since then.
    Since(SystemTime),
    /// Not empty (a folder that just appeared).
    Entries,
}

/// The [`TreeWatch`] thread: sets the watch up, then follows folders coming and going.
#[derive(Clone)]
struct Keeper {
    root: PathBuf,
    depth: u32,
    show_hidden: bool,
    pending: Arc<Mutex<Pending>>,
    state: Arc<TreeState>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl Keeper {
    fn run(self, rx: &Receiver<Tap>, tap: &Sender<Tap>, since: SystemTime) {
        specular_log::service::trip();
        let t0 = Instant::now();
        let per_folder = self.depth > 0 && !NATIVE_RECURSIVE;
        let made = FolderWatch::unwatched(
            &self.root,
            self.wake.clone(),
            self.pending.clone(),
            per_folder.then(|| tap.clone()),
        )
        .and_then(|mut w| {
            if per_folder {
                w.folders = Some(Folders::default());
                Ok(w)
            } else {
                let mode = if self.depth > 0 {
                    RecursiveMode::Recursive
                } else {
                    RecursiveMode::NonRecursive
                };
                w.watcher.watch(&self.root, mode).map(|()| w)
            }
        });
        let mut w = match made {
            Ok(w) => w,
            Err(e) => return self.fail(&e.to_string()),
        };
        let unseen = if per_folder {
            if self.state.cancel.load(Ordering::Relaxed) {
                return;
            }
            // The root first: a root that cannot be watched is a failed watch.
            w.add(&self.root);
            if w.watched() == 0 {
                return self.fail("cannot watch the folder");
            }
            let root_unseen = self.unseen(&self.root, Unseen::Since(since));
            match self.walk(&mut w, &self.root, 0, Unseen::Since(since)) {
                Some(u) => u || root_unseen,
                None => return,
            }
        } else {
            self.unseen(&self.root, Unseen::Since(since))
        };
        self.state.ready.store(true, Ordering::Release);
        specular_log::debug!(target: specular_log::FOLDER, "watch: {} folders of {} watched in {:.1} ms{}", w.watched(), self.root.display(), t0.elapsed().as_secs_f64() * 1e3, if w.missed() { " (some over the cap)" } else { "" });
        if unseen {
            self.rescan();
        }
        let mut walked = SystemTime::now();
        loop {
            let msg = if w.missed() {
                rx.recv_timeout(REWALK)
            } else {
                rx.recv().map_err(|_| RecvTimeoutError::Disconnected)
            };
            let unseen = match msg {
                Ok(Tap::Stop) | Err(RecvTimeoutError::Disconnected) => break,
                Ok(Tap::Paths(paths)) => {
                    let mut unseen = false;
                    for p in paths {
                        unseen |= self.follow(&mut w, &p);
                    }
                    unseen
                }
                Ok(Tap::Rewalk) => self.walk(&mut w, &self.root, 0, Unseen::Entries) == Some(true),
                Err(RecvTimeoutError::Timeout) => {
                    let since = std::mem::replace(&mut walked, SystemTime::now());
                    self.walk(&mut w, &self.root, 0, Unseen::Since(since)) == Some(true)
                }
            };
            if self.state.cancel.load(Ordering::Relaxed) {
                break;
            }
            if unseen {
                self.rescan();
            }
        }
        // `w`, and with it every kernel watch, is let go here, on this thread.
    }

    fn fail(&self, why: &str) {
        specular_log::debug!(target: specular_log::FOLDER, "watch: no watch on {}: {why}", self.root.display());
        self.state.failed.store(true, Ordering::Release);
        (self.wake)();
    }

    /// Something changed that no event reported: the owner relists.
    fn rescan(&self) {
        let mut st = self.pending.lock();
        st.rescan = true;
        st.last = Some(Instant::now());
        drop(st);
        (self.wake)();
    }

    /// A folder event at `p`: a folder that appeared is watched (with the folders below it),
    /// one that went is let go. Whether it may hold changes no event reported.
    fn follow(&self, w: &mut FolderWatch, p: &Path) -> bool {
        let Ok(rel) = p.strip_prefix(&self.root) else {
            return false;
        };
        let level = u32::try_from(rel.components().count()).unwrap_or(u32::MAX);
        if level == 0 {
            return false;
        }
        match std::fs::symlink_metadata(p) {
            Ok(m) if m.is_dir() => {
                let hidden = !self.show_hidden
                    && rel
                        .components()
                        .any(|c| c.as_os_str().to_string_lossy().starts_with('.'));
                level <= self.depth
                    && !hidden
                    && self.walk(w, p, level, Unseen::Entries) == Some(true)
            }
            Ok(_) => false,
            Err(_) => {
                w.remove_below(p);
                self.state.watched.store(w.watched(), Ordering::Relaxed);
                false
            }
        }
    }

    /// Watches the folders from `top` (`level` below the root) down to the depth limit.
    /// Whether a folder not watched before may hold changes no event reported, or `None` when
    /// cancelled. Each folder is watched before it is read, so a subfolder made meanwhile
    /// comes as an event.
    fn walk(&self, w: &mut FolderWatch, top: &Path, level: u32, how: Unseen) -> Option<bool> {
        let mut unseen = false;
        let mut stack = vec![(top.to_path_buf(), level)];
        let mut n = 0usize;
        while let Some((d, lv)) = stack.pop() {
            if self.state.cancel.load(Ordering::Relaxed) {
                return None;
            }
            if w.add(&d) && self.unseen(&d, how) {
                unseen = true;
            }
            n += 1;
            if n.is_multiple_of(256) {
                self.state.watched.store(w.watched(), Ordering::Relaxed);
            }
            if lv >= self.depth {
                continue;
            }
            let Ok(rd) = std::fs::read_dir(&d) else {
                continue;
            };
            for e in rd.flatten() {
                // Not followed: a symbolic link is not a folder here.
                if !e.file_type().is_ok_and(|t| t.is_dir()) {
                    continue;
                }
                if !self.show_hidden && e.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                stack.push((e.path(), lv + 1));
            }
        }
        self.state.watched.store(w.watched(), Ordering::Relaxed);
        Some(unseen)
    }

    /// Whether folder `d`, just watched, may hold changes no event reported. With a time,
    /// the files written into it since are added to the pending paths.
    fn unseen(&self, d: &Path, how: Unseen) -> bool {
        match how {
            Unseen::Entries => std::fs::read_dir(d).is_ok_and(|mut rd| rd.next().is_some()),
            Unseen::Since(since) => {
                let from = since.checked_sub(SLACK).unwrap_or(since);
                let recent = |m: std::io::Result<std::fs::Metadata>| {
                    m.and_then(|m| m.modified()).is_ok_and(|t| t >= from)
                };
                if !recent(std::fs::metadata(d)) {
                    return false;
                }
                let fresh: Vec<PathBuf> = std::fs::read_dir(d)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter(|e| recent(e.metadata()))
                    .map(|e| e.path())
                    .collect();
                self.pending.lock().paths.extend(fresh);
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn sees_new_and_removed_files() {
        let dir = crate::cache::test_dir("watch");
        let woken = Arc::new(AtomicUsize::new(0));
        let w2 = woken.clone();
        let Ok(w) = FolderWatch::new(
            &dir,
            false,
            Arc::new(move || {
                w2.fetch_add(1, Ordering::Relaxed);
            }),
        ) else {
            // No inotify (sandboxed CI): nothing to test.
            return;
        };
        std::fs::write(dir.join("new.jpg"), b"x").unwrap();
        let t = Instant::now();
        let changes = loop {
            if let Some(c) = w.take(Instant::now()) {
                break c;
            }
            assert!(t.elapsed() < Duration::from_secs(10), "no event");
            std::thread::sleep(Duration::from_millis(20));
        };
        assert!(
            changes.paths.iter().any(|p| p.ends_with("new.jpg")),
            "{changes:?}"
        );
        assert!(woken.load(Ordering::Relaxed) > 0);
        // Not settled yet right after an event.
        std::fs::remove_file(dir.join("new.jpg")).unwrap();
        let t = Instant::now();
        while w.due().is_none() {
            assert!(t.elapsed() < Duration::from_secs(10));
            std::thread::sleep(Duration::from_millis(5));
        }
        let first = w.due().unwrap();
        assert!(w.take(first - SETTLE).is_none());
        drop(w);
        std::fs::remove_dir_all(&dir).ok();
    }

    fn settled(w: &FolderWatch, within: Duration) -> Option<Changes> {
        let t = Instant::now();
        while t.elapsed() < within {
            if let Some(c) = w.take(Instant::now()) {
                return Some(c);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        None
    }

    #[test]
    fn per_folder_watches_only_the_folders_added() {
        if NATIVE_RECURSIVE {
            return;
        }
        let dir = crate::cache::test_dir("watch-folders");
        for d in ["a/deep", "b", "outside"] {
            std::fs::create_dir_all(dir.join(d)).unwrap();
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.join("outside"), dir.join("a/link")).unwrap();
        let Ok(mut w) = FolderWatch::folders(&dir, Arc::new(|| {})) else {
            return;
        };
        assert_eq!(w.watched(), 0, "nothing is watched until added");
        for d in ["", "a", "a/deep", "b"] {
            w.add(&dir.join(d));
        }
        w.add(&dir.join("a"));
        assert_eq!(w.watched(), 4);
        std::fs::write(dir.join("a/deep/new.jpg"), b"x").unwrap();
        let ch = settled(&w, Duration::from_secs(10)).expect("an added folder is watched");
        assert!(
            ch.paths.iter().any(|p| p.ends_with("a/deep/new.jpg")),
            "{ch:?}"
        );
        // Not added, and behind a symbolic link: not watched.
        std::fs::write(dir.join("outside/x.jpg"), b"x").unwrap();
        assert_eq!(settled(&w, Duration::from_millis(600)), None);
        // A folder and the folders below it go together; its siblings stay.
        w.remove_below(&dir.join("a"));
        assert_eq!(w.watched(), 2);
        std::fs::write(dir.join("a/deep/gone.jpg"), b"x").unwrap();
        assert_eq!(settled(&w, Duration::from_millis(600)), None);
        std::fs::write(dir.join("b/new.jpg"), b"x").unwrap();
        assert!(settled(&w, Duration::from_secs(10)).is_some());
        assert!(!w.missed());
        drop(w);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_renamed_folder_keeps_its_watch() {
        if NATIVE_RECURSIVE {
            return;
        }
        let dir = crate::cache::test_dir("watch-rename");
        std::fs::create_dir_all(dir.join("old")).unwrap();
        let Ok(mut w) = FolderWatch::folders(&dir, Arc::new(|| {})) else {
            return;
        };
        w.add(&dir.join("old"));
        std::fs::rename(dir.join("old"), dir.join("new")).unwrap();
        // Listed again under its new name before the old name is let go: one kernel watch.
        w.add(&dir.join("new"));
        w.remove_below(&dir.join("old"));
        assert_eq!(w.watched(), 1);
        let _ = settled(&w, Duration::from_millis(400));
        std::fs::write(dir.join("new/x.jpg"), b"x").unwrap();
        assert!(settled(&w, Duration::from_secs(10)).is_some());
        drop(w);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A fresh folder under TMPDIR (the build sets it inside the work tree).
    fn tmp_tree(name: &str) -> PathBuf {
        static N: AtomicUsize = AtomicUsize::new(0);
        let d = std::env::temp_dir().join(format!(
            "specular-{name}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// `root/aNN/bNN`: `outer` × `inner` leaf folders.
    fn make_tree(root: &Path, outer: usize, inner: usize) {
        for i in 0..outer {
            for j in 0..inner {
                std::fs::create_dir_all(root.join(format!("a{i:03}/b{j:03}"))).unwrap();
            }
        }
    }

    /// Changes until `done` says all were seen (or `within` passed).
    fn collect(
        w: &TreeWatch,
        within: Duration,
        done: impl Fn(&HashSet<PathBuf>, bool) -> bool,
    ) -> (HashSet<PathBuf>, bool) {
        let (mut paths, mut rescan) = (HashSet::new(), false);
        let t = Instant::now();
        while t.elapsed() < within && !(w.ready() && done(&paths, rescan)) {
            if let Some(c) = w.take(Instant::now()) {
                paths.extend(c.paths);
                rescan |= c.rescan;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        (paths, rescan)
    }

    /// A panic in the watch thread: the owner is told to relist, and after the restart new
    /// files are reported again.
    #[test]
    fn tree_watch_restarts_after_a_panic() {
        let root = tmp_tree("treewatch-panic");
        specular_log::service::arm("test-watch-pn", 1);
        let w = TreeWatch::start("test-watch-pn", &root, 1, false, Arc::new(|| {}));
        let (_, rescan) = collect(&w, Duration::from_secs(10), |_, r| r);
        if w.failed() {
            std::fs::remove_dir_all(&root).ok();
            return;
        }
        assert!(rescan, "the owner relists after the panic");
        assert_eq!(specular_log::service::deaths("test-watch-pn"), 1);
        let file = root.join("after.jpg");
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut seen = false;
        while !seen && Instant::now() < deadline {
            // Written again until the restarted watch reports it.
            std::fs::write(&file, b"x").unwrap();
            let (paths, _) = collect(&w, Duration::from_millis(500), |p, _| p.contains(&file));
            seen = paths.contains(&file);
        }
        assert!(seen, "the restarted watch reports new files");
        drop(w);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn tree_watch_starts_at_once_and_misses_nothing_during_setup() {
        if NATIVE_RECURSIVE {
            return;
        }
        let (outer, inner) = (60, 50);
        let root = tmp_tree("treewatch");
        make_tree(&root, outer, inner);
        // Not watched: below the depth limit, hidden, or behind a symbolic link.
        std::fs::create_dir_all(root.join("a000/b000/too-deep")).unwrap();
        std::fs::create_dir_all(root.join(".hidden/x")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("a001"), root.join("link")).unwrap();

        let t = Instant::now();
        let w = TreeWatch::start("test-watch", &root, 2, false, Arc::new(|| {}));
        let start = t.elapsed();
        // Written while the walk runs: some folders are watched by then, some not yet.
        let mut written = Vec::new();
        for i in (0..outer).step_by(7) {
            let p = root.join(format!("a{i:03}/b{:03}/during.jpg", i % inner));
            std::fs::write(&p, b"x").unwrap();
            written.push(p);
        }
        std::fs::create_dir(root.join("a002/made")).unwrap();
        std::fs::write(root.join("a002/made/in-new.jpg"), b"x").unwrap();
        let ready_after = {
            let t = Instant::now();
            while !w.ready() && !w.failed() && t.elapsed() < Duration::from_secs(60) {
                std::thread::sleep(Duration::from_millis(1));
            }
            t.elapsed() + start
        };
        if w.failed() {
            // No inotify (sandboxed CI): nothing to test.
            std::fs::remove_dir_all(&root).ok();
            return;
        }
        eprintln!(
            "{} folders: start() {:.3} ms, watched after {:.1} ms",
            w.watched(),
            start.as_secs_f64() * 1e3,
            ready_after.as_secs_f64() * 1e3
        );
        assert!(
            start < Duration::from_millis(50),
            "start() blocked for {start:?}"
        );
        assert!(w.ready());
        // Root, the a and b folders and the one made during setup (which, made in a folder
        // already watched, comes as an event, maybe once the walk is done).
        let t = Instant::now();
        while w.watched() < 1 + outer + outer * inner + 1 && t.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(w.watched(), 1 + outer + outer * inner + 1);

        let (paths, rescan) = collect(&w, Duration::from_secs(10), |p, r| {
            written.iter().all(|f| p.contains(f))
                && (r || p.contains(&root.join("a002/made/in-new.jpg")))
        });
        for f in &written {
            assert!(paths.contains(f), "{} not seen", f.display());
        }
        assert!(rescan || paths.contains(&root.join("a002/made/in-new.jpg")));

        // After setup: new folders are followed, gone ones let go.
        std::fs::create_dir_all(root.join("a003/later")).unwrap();
        let t = Instant::now();
        while w.watched() < 1 + outer + outer * inner + 2 {
            assert!(
                t.elapsed() < Duration::from_secs(10),
                "new folder not watched"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = collect(&w, Duration::from_millis(400), |_, _| false);
        let later = root.join("a003/later/after.jpg");
        std::fs::write(&later, b"x").unwrap();
        let (paths, _) = collect(&w, Duration::from_secs(10), |p, _| p.contains(&later));
        assert!(paths.contains(&later));
        std::fs::remove_dir_all(root.join("a004")).unwrap();
        let t = Instant::now();
        while w.watched() > 1 + outer + outer * inner + 2 - (1 + inner) {
            assert!(
                t.elapsed() < Duration::from_secs(10),
                "gone folder still watched"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        // Not watched: a hidden folder, one below the depth limit.
        let _ = collect(&w, Duration::from_millis(400), |_, _| false);
        std::fs::write(root.join(".hidden/x/no.jpg"), b"x").unwrap();
        std::fs::write(root.join("a000/b000/too-deep/no.jpg"), b"x").unwrap();
        assert_eq!(w.take(Instant::now() + SETTLE), None);
        std::thread::sleep(Duration::from_millis(400));
        assert_eq!(w.take(Instant::now()), None);
        drop(w);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_dropped_tree_watch_stops_its_thread() {
        let root = tmp_tree("treewatch-drop");
        make_tree(&root, 20, 50);
        let w = TreeWatch::start("test-watch", &root, 3, false, Arc::new(|| {}));
        let state = w.state.clone();
        drop(w);
        let t = Instant::now();
        // The keeper holds the other reference until it ends.
        while Arc::strong_count(&state) > 1 {
            assert!(
                t.elapsed() < Duration::from_secs(10),
                "keeper still running"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_tree_watch_without_depth_watches_one_folder() {
        let root = tmp_tree("treewatch-flat");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        let w = TreeWatch::start("test-watch", &root, 0, false, Arc::new(|| {}));
        assert!(w.is(&root, 0, true));
        assert!(!w.is(&root, 1, false));
        let t = Instant::now();
        while !w.ready() && !w.failed() {
            assert!(t.elapsed() < Duration::from_secs(10));
            std::thread::sleep(Duration::from_millis(5));
        }
        if w.failed() {
            std::fs::remove_dir_all(&root).ok();
            return;
        }
        std::fs::write(root.join("new.jpg"), b"x").unwrap();
        std::fs::write(root.join("sub/below.jpg"), b"x").unwrap();
        let (paths, _) = collect(&w, Duration::from_secs(10), |p, _| !p.is_empty());
        assert!(paths.contains(&root.join("new.jpg")), "{paths:?}");
        assert!(!paths.contains(&root.join("sub/below.jpg")));
        drop(w);
        std::fs::remove_dir_all(&root).ok();
    }

    /// The UI-thread time of watching a 20 000-folder tree, before (`FolderWatch::new`, one
    /// recursive watch made by the caller) and after (`TreeWatch::start`).
    /// `cargo test -p specular-thumbs --lib -- --ignored --nocapture watch_setup_timing`
    #[test]
    #[ignore = "timing; builds a 20 000-folder tree"]
    fn watch_setup_timing() {
        let root = tmp_tree("treewatch-20k");
        make_tree(&root, 100, 200);
        for round in 0..3 {
            let t = Instant::now();
            let before = FolderWatch::new(&root, true, Arc::new(|| {}));
            let old = t.elapsed();
            let ok = before.is_ok();
            let t = Instant::now();
            drop(before);
            let old_drop = t.elapsed();
            let t = Instant::now();
            let w = TreeWatch::start("test-watch", &root, 16, false, Arc::new(|| {}));
            let new = t.elapsed();
            while !w.ready() && !w.failed() {
                std::thread::sleep(Duration::from_micros(200));
            }
            let ready = t.elapsed();
            let watched = w.watched();
            let t = Instant::now();
            drop(w);
            let new_drop = t.elapsed();
            eprintln!(
                "round {round}: before: FolderWatch::new {:.1} ms on the caller (ok {ok}), drop {:.1} ms; \
                 after: TreeWatch::start {:.3} ms on the caller, drop {:.3} ms, {watched} folders watched \
                 off-thread in {:.1} ms",
                old.as_secs_f64() * 1e3,
                old_drop.as_secs_f64() * 1e3,
                new.as_secs_f64() * 1e3,
                new_drop.as_secs_f64() * 1e3,
                ready.as_secs_f64() * 1e3,
            );
            // Let the keeper let go of its watches before the next round.
            std::thread::sleep(Duration::from_millis(500));
        }
        std::fs::remove_dir_all(&root).ok();
    }
}
