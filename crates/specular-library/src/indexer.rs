//! The indexer thread: takes commands, follows the watchers, walks, reads facts.
//!
//! One loop, in order of urgency: commands from the app (add, remove, rescan, marks), settled
//! watcher events (walks of just the folders they name), the periodic checks (offline
//! locations coming back, rescans of unwatched ones), queued walks, and finally one batch of
//! the facts pass, which yields to everything above between batches and stops within a file
//! when the user browses ([`crate::Library::hold`]). After the facts come the signatures
//! ([`crate::hashes`]), the same way, and the duplicate and similar-shot groups
//! ([`crate::matches`]), formed again from what changed when the signatures are done (and
//! every few seconds during a long signature pass). Both run under [`Phase::Idle`]. A
//! full-text index in another fold (a new catalogue, or the app's fold changed) is rebuilt
//! before all of that, a part at a time between commands.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use rusqlite::{Connection, OptionalExtension, params};
use specular_thumbs::watch::FolderWatch;

use crate::matches::{Budget, Matches};
use crate::scan::{self, Walker};
use crate::skip::Skip;
use crate::{Cmd, Config, Halt, Phase, Queue, Shared, facts, hashes, now_s, store};
use specular_core::work::{self, Kind, Pause, Step, Work};

/// A walk waiting to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub folder: i64,
    /// The whole location; else just `paths` (from the watcher).
    pub full: bool,
    pub paths: Vec<PathBuf>,
}

/// How often the UI is woken while counts move.
const WAKE_EVERY: Duration = Duration::from_millis(200);
/// How often location counts are refreshed while reading.
const COUNTS_EVERY: Duration = Duration::from_secs(2);
/// A full count of the files waits at least this many times what the last one took.
const COUNTS_SHARE: u32 = 20;
/// How often the groups are brought up to date during a long signature pass.
const MATCH_EVERY: Duration = Duration::from_secs(10);
/// How long a part of a matching pass may keep commands waiting.
const MATCH_PART: Duration = Duration::from_millis(30);
/// Signatures a part of a matching pass takes in at a time.
const MATCH_ROWS: usize = 128;
/// Files (or folders) a part of a full-text rebuild rewrites between looks at the clock.
const FTS_ROWS: usize = 256;

struct Ix {
    conn: Connection,
    cfg: Config,
    shared: Arc<Shared>,
    poke: Sender<Cmd>,
    watches: HashMap<i64, FolderWatch>,
    last_full: HashMap<i64, Instant>,
    queue: Queue,
    next_check: Instant,
    /// Facts may be waiting.
    maybe_pending: bool,
    reading: bool,
    last_wake: Instant,
    last_counts: Instant,
    /// What the last full count took ([`Ix::refresh_locations_soon`]).
    count_cost: Duration,
    /// A full count is owed (files were added or went since the last).
    counts_due: bool,
    /// Signatures may be waiting.
    hash_pending: bool,
    /// The newest signature row's sequence number.
    seq: i64,
    /// Where the signature pass is ([`hashes::batch`]).
    hash_from: i64,
    matches: Matches,
    /// Signatures, marks or files changed since the groups were last formed.
    match_dirty: bool,
    last_match: Instant,
    /// When the matching pass under way began.
    match_began: Instant,
    /// Stopped by [`crate::Library::cancel`]: no full walks, facts or signatures until a
    /// rescan or an added location.
    paused: bool,
    /// The full-text index is being rebuilt for another fold, a part at a time before
    /// anything but commands; searches don't use it meanwhile.
    fts: Option<store::FtsRebuild>,
    /// The indexer in the background-work panel while it works ([`Ix::report`]).
    work: Option<Work>,
    /// Signatures made since the signature pass began (for the panel).
    signed: u64,
}

pub fn run(
    conn: Connection,
    cfg: Config,
    shared: Arc<Shared>,
    rx: Receiver<Cmd>,
    poke: Sender<Cmd>,
) {
    if let Some(f) = cfg.on_thread_start {
        f();
    }
    let mut ix = fresh(conn, cfg, shared, poke);
    loop {
        // Files are read behind their own boundaries (facts, signatures) and groups are
        // formed behind theirs; a panic here is in the indexer itself, which stops like on a
        // database error: the viewer and the catalogue as it stands go on, and the user is
        // told ("Indexing stopped: …", with Retry).
        let r = specular_core::isolate::run("library indexer", || {
            ix.prepare()?;
            // Catch up with what changed while Specular was not running.
            let ids: Vec<i64> = ix.shared.locations.lock().iter().map(|l| l.id).collect();
            for id in ids {
                ix.queue_full(id);
            }
            ix.run(&rx)
        });
        let halt = match r {
            Ok(Ok(())) => return,
            Ok(Err(e)) => {
                let e = crate::Error::from(e);
                Halt::Failed {
                    class: e.class(),
                    io: e.io().map(std::io::Error::kind),
                    detail: e.to_string(),
                }
            }
            Err(p) => Halt::Failed {
                class: crate::ErrorClass::Other,
                io: None,
                detail: p.to_string(),
            },
        };
        if let Halt::Failed { detail, .. } = &halt {
            specular_log::error!(target: specular_log::FOLDER, "library: indexer stopped: {detail}");
        }
        ix.work = None;
        {
            let mut p = ix.shared.progress.lock();
            p.phase = Phase::Idle;
            p.location = None;
            p.halt = Some(halt);
        }
        ix.shared.idle.store(true, Ordering::Release);
        (ix.cfg.wake)();
        // Parked until Retry; what the user asked meanwhile is kept and follows the restart.
        let mut kept = Vec::new();
        loop {
            match rx.recv() {
                Ok(Cmd::Retry) => break,
                Ok(Cmd::Stop) | Err(_) => return,
                Ok(Cmd::Poke) => {}
                Ok(c) => kept.push(c),
            }
        }
        let Ix {
            conn,
            cfg,
            shared,
            poke,
            ..
        } = ix;
        shared.queued.store(0, Ordering::Release);
        shared.progress.lock().halt = None;
        shared.beat();
        specular_log::info!(target: specular_log::FOLDER, "library: indexer started again");
        for c in kept {
            shared.queued.fetch_add(1, Ordering::AcqRel);
            let _ = poke.send(c);
        }
        ix = fresh(conn, cfg, shared, poke);
    }
}

/// An indexer that has done nothing yet (also after a restart).
fn fresh(conn: Connection, cfg: Config, shared: Arc<Shared>, poke: Sender<Cmd>) -> Ix {
    let now = Instant::now();
    Ix {
        conn,
        next_check: now + cfg.offline_every,
        cfg,
        shared,
        poke,
        watches: HashMap::new(),
        last_full: HashMap::new(),
        queue: Queue::new(),
        maybe_pending: true,
        reading: false,
        last_wake: now,
        last_counts: now,
        count_cost: Duration::ZERO,
        counts_due: false,
        hash_pending: true,
        seq: 0,
        hash_from: 0,
        matches: Matches::new(),
        match_dirty: true,
        last_match: now,
        match_began: now,
        paused: false,
        fts: None,
        work: None,
        signed: 0,
    }
}

impl Ix {
    fn prepare(&mut self) -> rusqlite::Result<()> {
        if store::info_get(&self.conn, "fold")?.as_deref() != Some(self.cfg.fold_id) {
            // Searches stop trusting the index (an older build with the old fold too) until
            // the last part of the rebuild names the new fold.
            self.conn
                .execute("DELETE FROM info WHERE key = 'fold'", [])?;
            self.fts = Some(store::FtsRebuild::START);
        }
        self.seq =
            self.conn
                .query_row("SELECT COALESCE(MAX(seq), 0) FROM image_hashes", [], |r| {
                    r.get(0)
                })?;
        Ok(())
    }

    fn stopping(&self) -> bool {
        self.shared.stop.load(Ordering::Relaxed)
    }

    fn run(&mut self, rx: &Receiver<Cmd>) -> rusqlite::Result<()> {
        loop {
            self.shared.beat();
            while let Ok(c) = rx.try_recv() {
                self.handle(c)?;
            }
            if self.stopping() {
                self.work = None;
                return Ok(());
            }
            self.report();
            if self.fts.is_some() {
                self.fts_part()?;
                continue;
            }
            let now = Instant::now();
            if self.counts_due {
                self.refresh_locations_soon()?;
            }
            self.poll_watches(now);
            if now >= self.next_check {
                self.next_check = now + self.cfg.offline_every;
                self.periodic(now)?;
            }
            if let Some(job) = self.queue.pop_front() {
                self.shared.idle.store(false, Ordering::Release);
                self.walk(job)?;
                continue;
            }
            // Stopped: only the watchers' walks of what changed and the groups of what is
            // signed already; nothing is read. Paused from the background-work panel: like
            // browsing, but until resumed.
            let held = self.shared.held() || work::user_paused(Kind::Library);
            if self.maybe_pending && !held && !self.paused {
                self.read_batch()?;
                continue;
            }
            let signing = self.hash_pending && self.cfg.signer.is_some() && !self.paused;
            if signing && !held {
                self.hash_batch()?;
                continue;
            }
            if self.match_dirty && !signing && !held {
                self.sync_matches()?;
                continue;
            }
            let timeout = if (self.maybe_pending && !self.paused) || signing || self.match_dirty {
                Duration::from_millis(30)
            } else {
                if self.shared.progress.lock().phase != Phase::Idle {
                    self.reading = false;
                    self.refresh_locations()?;
                    self.set_phase(Phase::Idle);
                }
                let mut next = self.next_check;
                for w in self.watches.values() {
                    if let Some(d) = w.due() {
                        next = next.min(d);
                    }
                }
                // An owed count is work still to do: not idle until it is done.
                if self.counts_due {
                    next = next.min(self.last_counts + self.count_cost * COUNTS_SHARE);
                }
                let t = next
                    .saturating_duration_since(Instant::now())
                    .clamp(Duration::from_millis(10), Duration::from_secs(1));
                if !self.counts_due && self.watches.values().all(|w| w.due().is_none()) {
                    self.shared.idle.store(true, Ordering::Release);
                }
                t
            };
            match rx.recv_timeout(timeout) {
                Ok(c) => self.handle(c)?,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            }
        }
    }

    fn handle(&mut self, c: Cmd) -> rusqlite::Result<()> {
        self.shared.idle.store(false, Ordering::Release);
        if !matches!(c, Cmd::Poke) {
            let _ = self
                .shared
                .queued
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |q| q.checked_sub(1));
        }
        match c {
            Cmd::Add(p) => {
                self.resume();
                self.add(&p)?;
            }
            Cmd::Remove(id) => self.remove(id)?,
            Cmd::Rescan(id) => {
                self.resume();
                let ids: Vec<i64> = match id {
                    Some(id) => vec![id],
                    None => self.shared.locations.lock().iter().map(|l| l.id).collect(),
                };
                for folder in ids {
                    self.queue_full(folder);
                }
            }
            Cmd::Marks(m) => {
                if store::set_marks(&self.conn, &m)? > 0 {
                    // Marks protect files from being suggested for discarding.
                    self.match_dirty = true;
                    self.changed(1);
                    self.wake(true);
                }
            }
            Cmd::Keywords(k) => {
                if store::set_keywords(&self.conn, &k, self.cfg.fold)? > 0 {
                    self.changed(1);
                    self.wake(true);
                }
            }
            Cmd::Iptc(f) => {
                if store::set_iptc(&self.conn, &f)? > 0 {
                    self.changed(1);
                    self.wake(true);
                }
            }
            Cmd::Edit(path, version, e) => {
                if let Some(id) = crate::edits::file_id(&self.conn, &path)? {
                    crate::edits::save(&mut self.conn, id, version, &e)?;
                }
            }
            Cmd::DeleteEdit(path, version) => {
                if let Some(id) = crate::edits::file_id(&self.conn, &path)? {
                    crate::edits::delete(&self.conn, id, version)?;
                }
            }
            Cmd::Skip => {
                // Stopped: the next rescan walks everything with the new rules.
                if !self.paused {
                    let ids: Vec<i64> = self.shared.locations.lock().iter().map(|l| l.id).collect();
                    for folder in ids {
                        self.queue_full(folder);
                    }
                }
            }
            Cmd::PreferForeign(folders) => {
                if store::read_foreign_again(&self.conn, &folders)? > 0 {
                    self.maybe_pending = true;
                    self.changed(1);
                    self.wake(true);
                }
            }
            Cmd::Cancel => self.cancel()?,
            Cmd::Poke => {}
            // Running (it was stalled and moves again): walk everything once more.
            Cmd::Retry => {
                self.resume();
                let ids: Vec<i64> = self.shared.locations.lock().iter().map(|l| l.id).collect();
                for folder in ids {
                    self.queue_full(folder);
                }
            }
            Cmd::Stop => self.shared.stop.store(true, Ordering::Relaxed),
        }
        Ok(())
    }

    /// Stops the walks and the reading (the walk or batch under way has already returned:
    /// the flag is checked at each folder and file). Whatever was written stays; files not
    /// read yet stay pending, and full walks not begun are dropped.
    fn cancel(&mut self) -> rusqlite::Result<()> {
        self.queue.retain(|j| !j.full);
        self.paused = true;
        self.reading = false;
        self.shared.cancel.store(false, Ordering::Relaxed);
        self.refresh_locations()?;
        {
            let mut p = self.shared.progress.lock();
            p.stopped = true;
            p.to_read = 0;
            p.read = 0;
            p.walked = 0;
        }
        specular_log::debug!(target: specular_log::FOLDER, "library: indexing stopped");
        self.set_phase(Phase::Idle);
        self.wake(true);
        Ok(())
    }

    /// A rescan or an added location after [`Self::cancel`]: walking and reading go on.
    fn resume(&mut self) {
        if self.paused {
            self.paused = false;
            self.maybe_pending = true;
            self.hash_pending = true;
            self.shared.progress.lock().stopped = false;
        }
    }

    fn queue_full(&mut self, folder: i64) {
        self.queue.retain(|j| j.folder != folder);
        self.queue.push_back(Job {
            folder,
            full: true,
            paths: Vec::new(),
        });
    }

    fn add(&mut self, p: &Path) -> rusqlite::Result<()> {
        // Test hook (debug builds): a location named `*specular-test-panic*` panics the
        // indexer itself.
        specular_core::isolate::trip(p);
        // Without Windows's `\\?\` prefix, so locations compare with (and read like) the
        // user's paths.
        let p = specular_platform::path::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        let Some(ps) = store::text(&p) else {
            specular_log::warn!(target: specular_log::FOLDER, "library: a location's path must be UTF-8");
            return Ok(());
        };
        let locs = self.shared.locations.lock().clone();
        if locs.iter().any(|l| p.starts_with(&l.path)) {
            specular_log::info!(target: specular_log::FOLDER, "library: already watched");
            return Ok(());
        }
        let inner: Vec<i64> = locs
            .iter()
            .filter(|l| l.path.starts_with(&p))
            .map(|l| l.id)
            .collect();
        let dev = std::fs::metadata(&p).ok().and_then(|m| scan::device(&m));
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        self.conn.execute(
            "INSERT INTO folders (path, added, dev) VALUES (?1, ?2, ?3)",
            params![ps, now_s(), dev],
        )?;
        let id = self.conn.last_insert_rowid();
        // Locations inside the new one become part of it, with their rows.
        for old in &inner {
            self.conn.execute(
                "UPDATE dirs SET folder_id = ?1 WHERE folder_id = ?2",
                [id, *old],
            )?;
            self.conn
                .execute("DELETE FROM folders WHERE id = ?1", [*old])?;
            self.watches.remove(old);
            self.queue.retain(|j| j.folder != *old);
        }
        self.conn.execute_batch("COMMIT")?;
        self.queue.push_front(Job {
            folder: id,
            full: true,
            paths: Vec::new(),
        });
        self.maybe_pending = true;
        self.refresh_locations()?;
        self.changed(1);
        self.wake(true);
        Ok(())
    }

    fn remove(&mut self, id: i64) -> rusqlite::Result<()> {
        self.watches.remove(&id);
        self.shared.progress.lock().out_of_watches = self.watches.values().any(|w| w.missed());
        self.queue.retain(|j| j.folder != id);
        self.last_full.remove(&id);
        // Cascades to its folders, files, thumbnail references and tags (and, by trigger,
        // their full-text rows).
        let conn = &self.conn;
        let done = (|| {
            conn.execute_batch("BEGIN IMMEDIATE")?;
            conn.execute("DELETE FROM folders WHERE id = ?1", [id])?;
            conn.execute(
                "DELETE FROM tags WHERE id NOT IN (SELECT tag_id FROM file_tags)",
                [],
            )?;
            conn.execute_batch("COMMIT")
        })();
        // Done or failed, the removal no longer waits (other work would stop forever).
        self.shared.removing.lock().remove(&id);
        if done.is_err() {
            let _ = self.conn.execute_batch("ROLLBACK");
        }
        done?;
        self.matches.check_gone = true;
        self.match_dirty = true;
        self.refresh_locations()?;
        self.changed(1);
        self.wake(true);
        Ok(())
    }

    fn poll_watches(&mut self, now: Instant) {
        let mut jobs = Vec::new();
        for (&id, w) in &self.watches {
            if let Some(ch) = w.take(now) {
                if ch.rescan {
                    jobs.push(Job {
                        folder: id,
                        full: true,
                        paths: Vec::new(),
                    });
                } else if !ch.paths.is_empty() {
                    jobs.push(Job {
                        folder: id,
                        full: false,
                        paths: ch.paths,
                    });
                }
            }
        }
        for j in jobs {
            if j.full {
                self.queue_full(j.folder);
            } else if let Some(q) = self
                .queue
                .iter_mut()
                .find(|q| q.folder == j.folder && !q.full)
            {
                q.paths.extend(j.paths);
            } else if !self.queue.iter().any(|q| q.folder == j.folder && q.full) {
                self.queue.push_back(j);
            }
        }
    }

    /// Offline locations coming back, online ones going, unwatched ones due a walk.
    fn periodic(&mut self, now: Instant) -> rusqlite::Result<()> {
        let locs = self.shared.locations.lock().clone();
        for l in locs {
            if l.offline {
                if !self.paused && self.root_state(l.id, &l.path)?.is_ok() {
                    self.queue_full(l.id);
                }
                continue;
            }
            if let Err(why) = self.root_state(l.id, &l.path)? {
                self.go_offline(l.id, why)?;
                continue;
            }
            let watched = self.watches.get(&l.id).is_some_and(|w| !w.missed());
            let due = self
                .last_full
                .get(&l.id)
                .is_none_or(|t| now.duration_since(*t) >= self.cfg.rescan_every);
            if !watched && due && !self.paused && !self.queue.iter().any(|j| j.folder == l.id) {
                self.queue_full(l.id);
            }
        }
        Ok(())
    }

    /// Whether the location's root is there (and is the volume it was).
    fn root_state(&self, id: i64, root: &Path) -> rusqlite::Result<Result<(), &'static str>> {
        let Ok(m) = std::fs::metadata(root) else {
            return Ok(Err("its folder is missing"));
        };
        if !m.is_dir() {
            return Ok(Err("its folder is missing"));
        }
        // A root the walk cannot list (its permissions were taken away) would otherwise be an
        // empty location with no word why; offline keeps its files and says so.
        if std::fs::read_dir(root).is_err_and(|e| e.kind() == std::io::ErrorKind::PermissionDenied)
        {
            return Ok(Err("its folder cannot be read"));
        }
        let dev: Option<i64> = self
            .conn
            .query_row("SELECT dev FROM folders WHERE id = ?1", [id], |r| r.get(0))
            .optional()?
            .flatten();
        let empty = || {
            std::fs::read_dir(root)
                .map(|mut d| d.next().is_none())
                .unwrap_or(true)
        };
        if dev.is_some() && dev != scan::device(&m) && empty() {
            // An empty mount point: the volume is not mounted.
            return Ok(Err("its volume is not mounted"));
        }
        Ok(Ok(()))
    }

    fn go_offline(&mut self, id: i64, why: &str) -> rusqlite::Result<()> {
        self.watches.remove(&id);
        self.queue.retain(|j| j.folder != id);
        let n = self.conn.execute(
            "UPDATE folders SET offline = 1 WHERE id = ?1 AND offline = 0",
            [id],
        )?;
        if n > 0 {
            specular_log::info!(target: specular_log::FOLDER, "library: a location is offline ({why})");
            self.refresh_locations()?;
            self.changed(1);
            self.wake(true);
        }
        Ok(())
    }

    fn walk(&mut self, job: Job) -> rusqlite::Result<()> {
        let Some(root) = self
            .conn
            .query_row(
                "SELECT path FROM folders WHERE id = ?1",
                [job.folder],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .map(PathBuf::from)
        else {
            return Ok(());
        };
        if let Err(why) = self.root_state(job.folder, &root)? {
            return self.go_offline(job.folder, why);
        }
        if self.conn.execute(
            "UPDATE folders SET offline = 0 WHERE id = ?1 AND offline = 1",
            [job.folder],
        )? > 0
        {
            specular_log::info!(target: specular_log::FOLDER, "library: a location is back online");
            self.changed(1);
        }
        let full = job.full || job.paths.contains(&root);
        let skip = self.shared.skip.lock().clone();
        let starts = if full {
            vec![(root.clone(), true)]
        } else {
            starts(&root, job.paths.clone(), &skip)
        };
        if starts.is_empty() {
            return Ok(());
        }
        {
            let mut p = self.shared.progress.lock();
            p.phase = Phase::Walking;
            p.location = Some(root.clone());
            p.walked = 0;
        }
        self.report();
        self.wake(true);
        // The walk watches each folder it lists (and only those: not hidden or tool folders,
        // not through symbolic links), so a large tree is never walked twice.
        if self.cfg.watch && !self.watches.contains_key(&job.folder) {
            let poke = self.poke.clone();
            let poke = std::sync::Mutex::new(poke);
            let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
                if let Ok(p) = poke.lock() {
                    let _ = p.send(Cmd::Poke);
                }
            });
            match FolderWatch::folders(&root, wake) {
                Ok(w) => {
                    self.watches.insert(job.folder, w);
                }
                Err(e) => specular_log::warn!(
                    target: specular_log::FOLDER,
                    "library: cannot watch a location, it is walked every {} min instead: {e}",
                    self.cfg.rescan_every.as_secs() / 60
                ),
            }
        }
        let mut watch = self.watches.get_mut(&job.folder);
        let missed = watch.as_ref().is_some_and(|w| w.missed());
        let shared = self.shared.clone();
        let wake = self.cfg.wake.clone();
        let mut last = Instant::now();
        let work = &self.work;
        let mut progress = |n: u64| {
            shared.beat();
            shared.progress.lock().walked = n;
            if last.elapsed() >= WAKE_EVERY {
                last = Instant::now();
                if let Some(w) = work {
                    w.set_progress(n, None);
                }
                wake();
            }
        };
        let out = Walker::new(
            &self.conn,
            self.cfg.is_image.as_ref(),
            self.cfg.fold,
            &self.shared,
            &mut progress,
            job.folder,
            root.clone(),
            &skip,
        )
        .watching(watch.as_deref_mut())
        .importing(self.cfg.edit_import.as_ref())
        .run(starts)?;
        if !missed && watch.is_some_and(|w| w.missed()) {
            self.shared.progress.lock().out_of_watches = true;
            specular_log::warn!(
                target: specular_log::FOLDER,
                "library: out of folder watches; a location is also walked every {} min",
                self.cfg.rescan_every.as_secs() / 60
            );
        }
        if out.skipped > 0 {
            specular_log::warn!(
                target: specular_log::FOLDER,
                "library: {} folder(s) skipped after an internal error; the rest was walked",
                out.skipped
            );
        }
        // Stopped for another location's removal: walked again once it is done.
        if out.aborted
            && !self.shared.halting()
            && !self.shared.removing.lock().contains(&job.folder)
        {
            self.queue.push_front(job.clone());
        }
        if out.root_gone {
            self.go_offline(job.folder, "its folder went missing during a walk")?;
        } else if full && !out.aborted {
            let dev = std::fs::metadata(&root).ok().and_then(|m| scan::device(&m));
            self.conn.execute(
                "UPDATE folders SET last_scan = ?2, dev = ?3 WHERE id = ?1",
                params![job.folder, now_s(), dev],
            )?;
            self.last_full.insert(job.folder, Instant::now());
            specular_log::debug!(
                target: specular_log::FOLDER,
                "library: walked a location: {} images, {} changes",
                out.walked,
                out.changes
            );
        }
        if out.changes > 0 {
            self.maybe_pending = true;
            self.hash_pending = true;
            self.matches.check_gone = true;
            self.match_dirty = true;
            self.changed(out.changes);
        }
        {
            let mut p = self.shared.progress.lock();
            p.location = None;
            if self.reading {
                p.phase = Phase::Reading;
                p.to_read += out.changes;
            }
        }
        if full {
            self.refresh_locations()?;
        } else if out.changes > 0 {
            self.refresh_locations_soon()?;
        }
        self.wake(true);
        Ok(())
    }

    fn read_batch(&mut self) -> rusqlite::Result<()> {
        if !self.reading {
            let n = facts::pending(&self.conn)?;
            if n == 0 {
                self.maybe_pending = false;
                return Ok(());
            }
            self.reading = true;
            let mut p = self.shared.progress.lock();
            p.phase = Phase::Reading;
            p.to_read = n;
            p.read = 0;
        }
        match facts::batch(&self.conn, &self.cfg, &self.shared)? {
            None => {
                self.maybe_pending = false;
                self.reading = false;
                self.refresh_locations()?;
                self.set_phase(Phase::Idle);
            }
            Some(0) => {}
            Some(n) => {
                self.hash_pending = true;
                {
                    let mut p = self.shared.progress.lock();
                    p.read += n as u64;
                    p.to_read = p.to_read.max(p.read);
                }
                self.changed(n as u64);
                if self.last_counts.elapsed() >= COUNTS_EVERY {
                    self.refresh_pending()?;
                }
                self.wake(false);
            }
        }
        Ok(())
    }

    /// One batch of signatures; the groups are brought up to date now and then meanwhile.
    fn hash_batch(&mut self) -> rusqlite::Result<()> {
        let Some(signer) = self.cfg.signer.clone() else {
            self.hash_pending = false;
            return Ok(());
        };
        match hashes::batch(
            &self.conn,
            &self.cfg,
            signer.as_ref(),
            &self.shared,
            &mut self.seq,
            &mut self.hash_from,
        )? {
            None => self.hash_pending = false,
            Some(0) => {}
            Some(n) => {
                self.signed += n as u64;
                specular_log::trace!(target: specular_log::FOLDER, "library: {n} signatures");
                self.match_dirty = true;
                // A pass under way goes on a part per batch; a new one begins now and then.
                if self.matches.part_way() || self.last_match.elapsed() >= MATCH_EVERY {
                    self.sync_matches()?;
                }
            }
        }
        Ok(())
    }

    /// Forms the duplicate and similar-shot groups again where something changed, a part at
    /// a time: commands are answered between parts, and a search sees the groups of the last
    /// finished pass until this one is through.
    fn sync_matches(&mut self) -> rusqlite::Result<()> {
        self.match_dirty = false;
        let Some(signer) = self.cfg.signer.clone() else {
            return Ok(());
        };
        if !self.matches.part_way() {
            self.last_match = Instant::now();
            self.match_began = Instant::now();
        }
        let shared = self.shared.clone();
        let waiting = move || shared.queued.load(Ordering::Acquire) > 0 || shared.held();
        let budget = Budget {
            time: MATCH_PART,
            rows: MATCH_ROWS,
            yield_now: &waiting,
        };
        match self
            .matches
            .sync_some(&self.conn, &self.cfg, signer.as_ref(), &budget)
        {
            Ok(s) => {
                if !s.done {
                    self.match_dirty = true;
                    return Ok(());
                }
                specular_log::debug!(
                    target: specular_log::FOLDER,
                    "library: groups updated: {} roles changed in {:.0?}",
                    s.written,
                    self.match_began.elapsed()
                );
                if s.written > 0 {
                    self.changed(1);
                    self.wake(true);
                }
                Ok(())
            }
            // Busy (another writer): try again on the next turn.
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == rusqlite::ErrorCode::DatabaseBusy =>
            {
                self.match_dirty = true;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// One part of the full-text rebuild, in one transaction: up to [`MATCH_PART`], or until
    /// a command waits. The last part names the new fold, so searches use the index again.
    fn fts_part(&mut self) -> rusqlite::Result<()> {
        let Some(mut at) = self.fts else {
            return Ok(());
        };
        let start = Instant::now();
        let (c, cfg, shared) = (&self.conn, &self.cfg, &self.shared);
        let mut part = || -> rusqlite::Result<Option<store::FtsRebuild>> {
            loop {
                match store::fts_rebuild_some(c, at, FTS_ROWS, cfg.fold)? {
                    Some(next) => {
                        at = next;
                        if start.elapsed() >= MATCH_PART
                            || shared.queued.load(Ordering::Acquire) > 0
                        {
                            return Ok(Some(at));
                        }
                    }
                    None => {
                        store::info_set(c, "fold", cfg.fold_id)?;
                        return Ok(None);
                    }
                }
            }
        };
        let r = c.execute_batch("BEGIN IMMEDIATE").and_then(|()| {
            match part().and_then(|next| c.execute_batch("COMMIT").map(|()| next)) {
                Ok(next) => Ok(next),
                Err(e) => {
                    let _ = c.execute_batch("ROLLBACK");
                    Err(e)
                }
            }
        });
        match r {
            Ok(next) => {
                self.fts = next;
                if next.is_none() {
                    specular_log::debug!(target: specular_log::FOLDER, "library: full-text index rebuilt for another fold");
                }
                Ok(())
            }
            // Busy (another writer): this part again on the next turn.
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == rusqlite::ErrorCode::DatabaseBusy =>
            {
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// The indexer as the background-work panel shows it ([`specular_core::work`]): what
    /// it does now, how far, what follows, and why it waits; gone when there is nothing to
    /// do. Cheap (a lock or two), once per turn of the loop and per walk.
    fn report(&mut self) {
        let (phase, location, walked, read, to_read) = {
            let p = self.shared.progress.lock();
            (
                p.phase,
                p.location
                    .as_ref()
                    .and_then(|l| l.file_name())
                    .map(|n| n.to_string_lossy().into_owned()),
                p.walked,
                p.read,
                p.to_read,
            )
        };
        let signing = self.hash_pending && self.cfg.signer.is_some();
        let now = match phase {
            Phase::Walking => Some((Step::Finding, walked, None)),
            Phase::Reading => Some((Step::Reading, read, Some(to_read))),
            Phase::Idle if self.fts.is_some() => Some((Step::TextIndex, 0, None)),
            Phase::Idle if signing => Some((Step::Signatures, self.signed, None)),
            Phase::Idle if self.match_dirty && self.cfg.signer.is_some() => {
                Some((Step::Grouping, 0, None))
            }
            Phase::Idle => None,
        };
        if !signing {
            self.signed = 0;
        }
        let Some((step, done, total)) = now else {
            self.work = None;
            return;
        };
        let w = self.work.get_or_insert_with(|| {
            let w = Work::start(Kind::Library, step);
            w.set_pausable(true);
            w
        });
        w.set_step(step, total);
        w.set_progress(done, total);
        w.set_item(if step == Step::Finding {
            location.as_deref()
        } else {
            None
        });
        let then: &[Step] = match (step, self.cfg.signer.is_some()) {
            (Step::Finding, true) => &[Step::Reading, Step::Signatures, Step::Grouping],
            (Step::Finding, false) => &[Step::Reading],
            (Step::Reading, true) => &[Step::Signatures, Step::Grouping],
            (Step::Signatures, _) => &[Step::Grouping],
            _ => &[],
        };
        w.set_then(then);
        let paused = if work::user_paused(Kind::Library) {
            Some(Pause::User)
        } else if self.shared.held() && step != Step::Finding {
            Some(Pause::Browsing)
        } else {
            None
        };
        w.set_paused(paused);
    }

    fn refresh_locations(&mut self) -> rusqlite::Result<()> {
        let t = Instant::now();
        self.shared.set_locations(store::locations(&self.conn)?);
        self.last_counts = Instant::now();
        self.count_cost = t.elapsed();
        self.counts_due = false;
        Ok(())
    }

    /// The counts again now, or once a full count costs at most a twentieth of the time
    /// since the last ([`COUNTS_SHARE`]): it reads every file's row (a third of a second at
    /// a million), and a watcher's walk of one folder after another must not do that each
    /// time.
    fn refresh_locations_soon(&mut self) -> rusqlite::Result<()> {
        if self.last_counts.elapsed() >= self.count_cost * COUNTS_SHARE {
            self.refresh_locations()
        } else {
            self.counts_due = true;
            Ok(())
        }
    }

    /// While the facts are read only the files waiting change: their counts come from the
    /// index of what waits (shrinking as the pass goes), not from every file.
    fn refresh_pending(&mut self) -> rusqlite::Result<()> {
        let pending = store::pending_by_location(&self.conn)?;
        let mut locs = self.shared.locations.lock().clone();
        for l in &mut locs {
            l.pending = pending.get(&l.id).copied().unwrap_or(0);
        }
        self.shared.set_locations(locs);
        self.last_counts = Instant::now();
        Ok(())
    }

    fn changed(&self, n: u64) {
        if n > 0 {
            self.shared.progress.lock().changes += 1;
        }
    }

    fn set_phase(&mut self, phase: Phase) {
        {
            let mut p = self.shared.progress.lock();
            if p.phase == phase {
                return;
            }
            p.phase = phase;
            if phase == Phase::Idle {
                p.location = None;
            }
        }
        self.wake(true);
    }

    fn wake(&mut self, force: bool) {
        if force || self.last_wake.elapsed() >= WAKE_EVERY {
            self.last_wake = Instant::now();
            (self.cfg.wake)();
        }
    }
}

/// Where to walk for the changed `paths` below `root`: a folder that is there is walked
/// whole; for anything else, its nearest folder that is there is listed (new folders in it
/// are walked whole, vanished ones go). Hidden and skipped folders ([`Skip::name`]) are not
/// followed.
pub fn starts(root: &Path, paths: Vec<PathBuf>, skip: &Skip) -> Vec<(PathBuf, bool)> {
    let mut want: HashMap<PathBuf, bool> = HashMap::new();
    for p in paths {
        let Ok(rel) = p.strip_prefix(root) else {
            continue;
        };
        if rel
            .components()
            .any(|c| c.as_os_str().to_str().is_some_and(|n| skip.name(n)))
        {
            continue;
        }
        // The deepest folder on the way down from the root that is a folder itself, not a
        // link to one (nor below one): a folder replaced by a link to `/` must not have the
        // walk follow it there. The path itself, when it is such a folder, is walked whole.
        let mut dir = root.to_path_buf();
        let mut whole = true;
        for c in rel.components() {
            let next = dir.join(c);
            if !std::fs::symlink_metadata(&next).is_ok_and(|m| m.is_dir()) {
                whole = false;
                break;
            }
            dir = next;
        }
        let rec = whole;
        let e = want.entry(dir).or_insert(rec);
        *e |= rec;
    }
    let mut v: Vec<(PathBuf, bool)> = want.into_iter().collect();
    v.sort();
    let mut out: Vec<(PathBuf, bool)> = Vec::new();
    let mut covered: HashSet<PathBuf> = HashSet::new();
    for (d, rec) in v {
        if d.ancestors().skip(1).any(|a| covered.contains(a)) {
            continue;
        }
        if rec {
            covered.insert(d.clone());
        }
        out.push((d, rec));
    }
    out
}
