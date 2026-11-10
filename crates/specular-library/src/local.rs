//! Local copies for offline editing: photos on a slow or removable location (a network
//! share, a USB disk, an SD card) kept in the app's data folder, so the gallery, the viewer,
//! culling, keywords and Develop go on while the location is offline.
//!
//! * **Copies:** per scope (a location, an album, a selection) the user picks the original,
//!   or a compact proxy of each raw ([`Mode::Proxies`]: made by the app's [`ProxyMaker`],
//!   a linear DNG at 2560 px; other files are copied whole). They live under the store
//!   folder (`<data>/local-copies`), never beside the originals, named by content and
//!   catalogue id. A copy is held by every scope that asked for it and goes when none does.
//!   The store has a size limit: a keep stops when the next copy would pass it.
//! * **Reading:** [`LocalCopies::resolve`] answers from memory which copy stands for an
//!   original; the app reads it only when the original cannot be read.
//! * **Changes while offline** ([`LocalCopies::enqueue`]) wait in the catalogue against the
//!   original's identity (path, size, time stamp, partial content hash) and its sidecars as
//!   they were when copied (else the catalogue's sidecar time stamp). One row per path,
//!   kind and Develop version: a later change replaces the payload and keeps the base.
//! * **Coming back:** [`LocalCopies::check`] tells for each waiting change whether the
//!   original or a sidecar it touches changed meanwhile ([`Check::Conflict`]); the app writes
//!   clean ones through its own writers and asks about conflicts, then [`LocalCopies::done`]
//!   drops them. Nothing is overwritten here: this module never writes outside its store.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::{Mutex, RwLock};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{Error, edits, now_s, scan, schema, store};

/// The store's folder name in the data folder.
pub const DIR_NAME: &str = "local-copies";
/// The long edge of a proxy, in pixels.
pub const PROXY_LONG_EDGE: u32 = 2560;
/// The default size limit (20 GiB).
pub const DEFAULT_LIMIT: u64 = 20 << 30;

/// What a scope keeps of each file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Each file whole.
    #[default]
    Originals,
    /// A compact proxy of each raw; other files whole.
    Proxies,
}

impl Mode {
    fn code(self) -> i64 {
        match self {
            Mode::Originals => 0,
            Mode::Proxies => 1,
        }
    }

    fn of(code: i64) -> Mode {
        if code == 1 {
            Mode::Proxies
        } else {
            Mode::Originals
        }
    }
}

/// What a copy is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CopyKind {
    Original,
    Proxy,
}

impl CopyKind {
    fn code(self) -> i64 {
        match self {
            CopyKind::Original => 0,
            CopyKind::Proxy => 1,
        }
    }

    fn of(code: i64) -> CopyKind {
        if code == 1 {
            CopyKind::Proxy
        } else {
            CopyKind::Original
        }
    }
}

/// What the user asked to keep.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    /// A watched location (`folders.id`).
    Location(i64),
    /// An album (`collections.id`).
    Album(i64),
    /// Files picked in the grid.
    Selection,
}

impl Scope {
    fn code(&self) -> (i64, Option<i64>) {
        match self {
            Scope::Location(id) => (0, Some(*id)),
            Scope::Album(id) => (1, Some(*id)),
            Scope::Selection => (2, None),
        }
    }

    fn of(kind: i64, target: Option<i64>) -> Scope {
        match (kind, target) {
            (0, Some(id)) => Scope::Location(id),
            (1, Some(id)) => Scope::Album(id),
            _ => Scope::Selection,
        }
    }
}

/// A kept scope, as listed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kept {
    pub id: i64,
    pub scope: Scope,
    /// What the user knows it by (the location's folder, the album's name, "12 photos").
    pub name: String,
    pub mode: Mode,
    /// Copies it holds, and their bytes (copies shared with other scopes counted in each).
    pub files: u64,
    pub bytes: u64,
}

/// A file's identity: what the Library recognises it by besides its path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Identity {
    pub size: u64,
    pub mtime_ns: i64,
    /// [`specular_thumbs::key::content_hash`] of the file.
    pub content: u64,
}

impl Identity {
    /// The identity of the file at `path` as it is now (three small reads).
    pub fn of_file(path: &Path) -> io::Result<Identity> {
        let k = specular_thumbs::ThumbKey::of_file(path)?;
        Ok(Identity {
            size: k.stamp.len,
            mtime_ns: k.stamp.mtime_ns.clamp(i64::MIN as i128, i64::MAX as i128) as i64,
            content: k.content,
        })
    }
}

/// One sidecar of an image, as it was: its file name, time stamp and content hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SidecarState {
    pub name: String,
    pub mtime_ns: i64,
    pub hash: u64,
}

/// Every sidecar of `image` as it is now: in its folder, `.xmp` files (either case) named
/// `<stem>.xmp` or starting with `<name>.` (`IMG.CR3.xmp`, `IMG.CR3.specular.xmp`,
/// `IMG.CR3.v2.specular.xmp`). Sorted by name.
pub fn sidecars_of(image: &Path) -> Vec<SidecarState> {
    let (Some(dir), Some(name)) = (
        image.parent(),
        image.file_name().map(|n| n.to_string_lossy().into_owned()),
    ) else {
        return Vec::new();
    };
    let stem = image
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if !belongs(&n, &name, &stem) {
            continue;
        }
        let Ok(m) = e.metadata() else { continue };
        if !m.is_file() {
            continue;
        }
        let Ok(bytes) = std::fs::read(e.path()) else {
            continue;
        };
        out.push(SidecarState {
            name: n,
            mtime_ns: scan::mtime_ns(m.modified().ok()),
            hash: specular_thumbs::key::content_hash(&bytes),
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Whether sidecar file name `n` belongs to the image named `name` (stem `stem`).
fn belongs(n: &str, name: &str, stem: &str) -> bool {
    let lower = n.to_ascii_lowercase();
    if !lower.ends_with(".xmp") {
        return false;
    }
    let base = &n[..n.len() - 4];
    base == stem || base == name || n.starts_with(&format!("{name}."))
}

/// Whether sidecar `name` holds what a change of `kind` (Develop version `version`) writes:
/// Develop edits live in `<name>[.vN].specular.xmp`, everything else in the plain sidecar.
fn touches(sidecar: &str, kind: ChangeKind, version: u32) -> bool {
    let lower = sidecar.to_ascii_lowercase();
    let develop = lower.ends_with(".specular.xmp");
    match kind {
        ChangeKind::Edit => {
            if !develop {
                return false;
            }
            let base = &lower[..lower.len() - ".specular.xmp".len()];
            let v = base
                .rsplit_once('.')
                .and_then(|(_, t)| t.strip_prefix('v'))
                .and_then(|t| t.parse::<u32>().ok());
            // `IMG.CR3.v2.specular.xmp` is version 2 (`sidecar::path_for_version`).
            match v {
                Some(n) if n >= 1 => version == n,
                _ => version == 0,
            }
        }
        _ => !develop,
    }
}

fn encode_sidecars(v: &[SidecarState]) -> String {
    let mut s = String::new();
    for c in v {
        if c.name.contains(['\t', '\n']) {
            continue;
        }
        s.push_str(&format!("{}\t{}\t{:x}\n", c.name, c.mtime_ns, c.hash));
    }
    s
}

fn decode_sidecars(s: &str) -> Vec<SidecarState> {
    s.lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            let name = f.next()?.to_owned();
            let mtime_ns = f.next()?.parse().ok()?;
            let hash = u64::from_str_radix(f.next()?, 16).ok()?;
            Some(SidecarState {
                name,
                mtime_ns,
                hash,
            })
        })
        .collect()
}

/// What changed while the original was offline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChangeKind {
    /// Rating, colour label, pick/reject (payload: [`encode_marks`]).
    Marks,
    /// Keywords (payload: one per line).
    Keywords,
    /// IPTC fields (payload: `field TAB value` lines).
    Iptc,
    /// A Develop edit (the catalogue holds it; the payload is empty). `version` names it.
    Edit,
}

impl ChangeKind {
    fn code(self) -> i64 {
        match self {
            ChangeKind::Marks => 0,
            ChangeKind::Keywords => 1,
            ChangeKind::Iptc => 2,
            ChangeKind::Edit => 3,
        }
    }

    fn of(code: i64) -> Option<ChangeKind> {
        Some(match code {
            0 => ChangeKind::Marks,
            1 => ChangeKind::Keywords,
            2 => ChangeKind::Iptc,
            3 => ChangeKind::Edit,
            _ => return None,
        })
    }
}

/// The payload of a [`ChangeKind::Marks`] change.
pub fn encode_marks(m: crate::Mark) -> Vec<u8> {
    format!("{} {} {}", m.rating, m.label, m.flag).into_bytes()
}

/// The marks of a [`ChangeKind::Marks`] payload.
pub fn decode_marks(b: &[u8]) -> Option<crate::Mark> {
    let s = std::str::from_utf8(b).ok()?;
    let mut f = s.split(' ');
    Some(crate::Mark {
        rating: f.next()?.parse::<u8>().ok()?.min(5),
        label: f.next()?.parse::<u8>().ok()?.min(4),
        flag: f.next()?.parse::<i8>().ok()?.clamp(-1, 1),
    })
}

/// The payload of a [`ChangeKind::Keywords`] change.
pub fn encode_keywords(k: &[String]) -> Vec<u8> {
    k.iter()
        .filter(|k| !k.contains('\n'))
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join("\n")
        .into_bytes()
}

pub fn decode_keywords(b: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(b)
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The payload of a [`ChangeKind::Iptc`] change.
pub fn encode_fields(f: &[(String, String)]) -> Vec<u8> {
    let mut s = String::new();
    for (k, v) in f {
        if k.contains(['\t', '\n']) {
            continue;
        }
        s.push_str(k);
        s.push('\t');
        s.push_str(&v.replace('\\', "\\\\").replace('\n', "\\n"));
        s.push('\n');
    }
    s.into_bytes()
}

pub fn decode_fields(b: &[u8]) -> Vec<(String, String)> {
    String::from_utf8_lossy(b)
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(k, v)| {
            let mut out = String::new();
            let mut esc = false;
            for c in v.chars() {
                match (esc, c) {
                    (true, 'n') => out.push('\n'),
                    (true, c) => out.push(c),
                    (false, '\\') => {
                        esc = true;
                        continue;
                    }
                    (false, c) => out.push(c),
                }
                esc = false;
            }
            (k.to_owned(), out)
        })
        .collect()
}

/// A change waiting for its original.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub id: i64,
    /// The original.
    pub path: PathBuf,
    /// The original as it was when the change was made against it.
    pub identity: Identity,
    /// Its sidecars as they were (None: only `xmp_mtime` was known).
    pub sidecars: Option<Vec<SidecarState>>,
    /// The catalogue's time stamp of its plain sidecar then (None: it had none).
    pub xmp_mtime: Option<i64>,
    pub kind: ChangeKind,
    pub version: u32,
    pub payload: Vec<u8>,
    /// When it was first queued (seconds since the epoch).
    pub queued: i64,
}

/// Whether a waiting change can be written as it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    /// Nothing it touches changed: write it.
    Clean,
    /// The original or a sidecar the change touches changed meanwhile: ask.
    Conflict { original: bool, sidecar: bool },
    /// The original is not there (still offline, or moved or deleted): leave it waiting.
    Missing,
}

/// Makes a proxy of a raw: its bytes (a linear DNG), or None when it cannot (the file is
/// then copied whole). Arguments: the original's path and bytes, and the long edge.
pub type ProxyMaker = Arc<dyn Fn(&Path, &[u8], u32) -> Option<Vec<u8>> + Send + Sync>;

/// What a keep did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeepReport {
    /// The scope's id.
    pub scope: i64,
    /// Copies made (or made again because the original changed).
    pub copied: u64,
    /// Of those, proxies.
    pub proxies: u64,
    /// Files already copied for another scope (now held by this one too).
    pub shared: u64,
    /// Files whose original could not be read (offline, unreadable).
    pub unreachable: u64,
    /// Paths the catalogue does not hold.
    pub not_catalogued: u64,
    /// The size limit was reached: this many files were left out.
    pub over_limit: u64,
    /// Bytes added to the store.
    pub bytes: u64,
    /// The keep was cancelled before the end.
    pub cancelled: bool,
}

/// Where a keep is (for the progress line).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeepProgress {
    pub done: u64,
    pub total: u64,
    pub bytes: u64,
}

/// One copy, as the index holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalCopy {
    /// The copy's file.
    pub path: PathBuf,
    pub kind: CopyKind,
}

/// The local copies and the changes that wait for their originals. Thread-safe; every call
/// but [`Self::resolve`], [`Self::copy_of`] and [`Self::used`] touches the catalogue, so
/// never on the UI thread.
pub struct LocalCopies {
    conn: Mutex<Connection>,
    root: PathBuf,
    /// Original → copy, for [`Self::resolve`] without a query.
    index: RwLock<HashMap<PathBuf, LocalCopy>>,
    used: AtomicU64,
    limit: AtomicU64,
    /// Changes waiting (cheap to read for the banner).
    waiting: AtomicU64,
}

impl LocalCopies {
    /// Opens the local copies of catalogue `catalogue` (opened and brought up to date by
    /// [`crate::Library::open`] first), with the store at `root`. Copies whose file is gone
    /// leave the catalogue, and files in the store no copy names are removed.
    pub fn open(catalogue: &Path, root: PathBuf) -> Result<LocalCopies, Error> {
        specular_platform::assert_not_ui_thread("LocalCopies::open");
        let mut conn = Connection::open(catalogue)?;
        schema::configure(&conn)?;
        let found = schema::migrate_checked(&mut conn)?;
        if found > schema::VERSION {
            return Err(Error::Newer { found });
        }
        std::fs::create_dir_all(&root)?;
        let me = LocalCopies {
            conn: Mutex::new(conn),
            root,
            index: RwLock::new(HashMap::new()),
            used: AtomicU64::new(0),
            limit: AtomicU64::new(DEFAULT_LIMIT),
            waiting: AtomicU64::new(0),
        };
        me.tidy()?;
        me.reload()?;
        Ok(me)
    }

    /// The store folder.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The size limit of the store, in bytes.
    pub fn set_limit(&self, bytes: u64) {
        self.limit.store(bytes, Ordering::Relaxed);
    }

    pub fn limit(&self) -> u64 {
        self.limit.load(Ordering::Relaxed)
    }

    /// Bytes the copies take (no query).
    pub fn used(&self) -> u64 {
        self.used.load(Ordering::Relaxed)
    }

    /// Changes waiting for their originals (no query).
    pub fn waiting(&self) -> u64 {
        self.waiting.load(Ordering::Relaxed)
    }

    /// The copy standing for `original`, if one is kept (no query, no I/O).
    pub fn copy_of(&self, original: &Path) -> Option<LocalCopy> {
        self.index.read().get(original).cloned()
    }

    /// The copy to read instead of `original`, if one is kept and its file is there.
    pub fn resolve(&self, original: &Path) -> Option<PathBuf> {
        let c = self.copy_of(original)?;
        c.path.is_file().then_some(c.path)
    }

    /// How many originals have a copy.
    pub fn count(&self) -> usize {
        self.index.read().len()
    }

    /// Of `originals`, those with a copy and its kind.
    pub fn kinds(&self, originals: &[PathBuf]) -> Vec<(PathBuf, CopyKind)> {
        let ix = self.index.read();
        originals
            .iter()
            .filter_map(|p| ix.get(p).map(|c| (p.clone(), c.kind)))
            .collect()
    }

    /// Rebuilds the in-memory index, the bytes used and the waiting count.
    fn reload(&self) -> Result<(), Error> {
        let c = self.conn.lock();
        let mut st = c.prepare(
            "SELECT d.path, f.name, l.local, l.kind, l.bytes FROM local_copies l
             JOIN files f ON f.id = l.file_id JOIN dirs d ON d.id = f.dir_id",
        )?;
        let mut ix = HashMap::new();
        let mut used = 0u64;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })?;
        for row in rows {
            let (dir, name, local, kind, bytes) = row?;
            used += bytes.max(0) as u64;
            ix.insert(
                Path::new(&dir).join(name),
                LocalCopy {
                    path: self.root.join(local),
                    kind: CopyKind::of(kind),
                },
            );
        }
        *self.index.write() = ix;
        self.used.store(used, Ordering::Relaxed);
        let n: i64 = c.query_row("SELECT COUNT(*) FROM local_changes", [], |r| r.get(0))?;
        self.waiting.store(n.max(0) as u64, Ordering::Relaxed);
        Ok(())
    }

    /// Copies whose file is gone leave the catalogue; store files no copy names go.
    fn tidy(&self) -> Result<(), Error> {
        let c = self.conn.lock();
        let rows: Vec<(i64, String)> = c
            .prepare("SELECT file_id, local FROM local_copies")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut known = std::collections::HashSet::new();
        for (id, local) in rows {
            if self.root.join(&local).is_file() {
                known.insert(local);
            } else {
                c.execute("DELETE FROM local_copies WHERE file_id = ?1", [id])?;
            }
        }
        // Copies no scope holds (a scope's location or album went with its rows).
        let orphans: Vec<(i64, String)> = c
            .prepare(
                "SELECT file_id, local FROM local_copies
                 WHERE file_id NOT IN (SELECT file_id FROM local_scope_files)",
            )?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (id, local) in orphans {
            c.execute("DELETE FROM local_copies WHERE file_id = ?1", [id])?;
            known.remove(&local);
        }
        c.execute(
            "DELETE FROM local_scopes WHERE id NOT IN (SELECT scope_id FROM local_scope_files)",
            [],
        )?;
        drop(c);
        let Ok(rd) = std::fs::read_dir(&self.root) else {
            return Ok(());
        };
        for shard in rd.flatten() {
            let Ok(files) = std::fs::read_dir(shard.path()) else {
                continue;
            };
            for f in files.flatten() {
                let rel = format!(
                    "{}/{}",
                    shard.file_name().to_string_lossy(),
                    f.file_name().to_string_lossy()
                );
                if !known.contains(&rel) {
                    let _ = std::fs::remove_file(f.path());
                }
            }
        }
        Ok(())
    }

    /// Every kept scope, newest first.
    pub fn kept(&self) -> Result<Vec<Kept>, Error> {
        let c = self.conn.lock();
        let mut st = c.prepare(
            "SELECT s.id, s.kind, s.target, s.name, s.mode,
                    COUNT(l.file_id), COALESCE(SUM(l.bytes), 0)
             FROM local_scopes s
             LEFT JOIN local_scope_files sf ON sf.scope_id = s.id
             LEFT JOIN local_copies l ON l.file_id = sf.file_id
             GROUP BY s.id ORDER BY s.created DESC, s.id DESC",
        )?;
        let rows = st.query_map([], |r| {
            Ok(Kept {
                id: r.get(0)?,
                scope: Scope::of(r.get(1)?, r.get(2)?),
                name: r.get(3)?,
                mode: Mode::of(r.get(4)?),
                files: r.get::<_, i64>(5)?.max(0) as u64,
                bytes: r.get::<_, i64>(6)?.max(0) as u64,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The kept scope for `scope` (a location's or album's; selections are each their own).
    pub fn kept_for(&self, scope: &Scope) -> Result<Option<Kept>, Error> {
        if *scope == Scope::Selection {
            return Ok(None);
        }
        Ok(self.kept()?.into_iter().find(|k| k.scope == *scope))
    }

    /// The catalogued images of a location or manual album, for a keep.
    pub fn files_of(&self, scope: &Scope) -> Result<Vec<PathBuf>, Error> {
        let c = self.conn.lock();
        let sql = match scope {
            Scope::Location(_) => {
                "SELECT d.path, f.name FROM files f JOIN dirs d ON d.id = f.dir_id
                 WHERE d.folder_id = ?1 AND f.state <> 2 ORDER BY d.path, f.name"
            }
            Scope::Album(_) => {
                "SELECT d.path, f.name FROM collection_files cf JOIN files f ON f.id = cf.file_id
                 JOIN dirs d ON d.id = f.dir_id WHERE cf.collection_id = ?1
                 ORDER BY d.path, f.name"
            }
            Scope::Selection => return Ok(Vec::new()),
        };
        let id = scope.code().1.unwrap_or(0);
        let rows: Vec<PathBuf> = c
            .prepare(sql)?
            .query_map([id], |r| {
                Ok(Path::new(&r.get::<_, String>(0)?).join(r.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    /// Keeps a local copy of each of `files` for `scope` (named `name`), as `mode` asks.
    /// Runs on the calling thread (copying takes as long as the drive takes): `progress` is
    /// told after each file, and `cancelled` is asked before each. A file already kept is
    /// held by this scope too (a proxy becomes the original when originals are asked for,
    /// and a copy whose original changed is made again). Stops when the next copy would
    /// pass the size limit.
    #[allow(clippy::too_many_arguments)]
    pub fn keep(
        &self,
        scope: &Scope,
        name: &str,
        mode: Mode,
        files: &[PathBuf],
        proxy: Option<&ProxyMaker>,
        cancelled: &dyn Fn() -> bool,
        progress: &dyn Fn(KeepProgress),
    ) -> Result<KeepReport, Error> {
        specular_platform::assert_not_ui_thread("LocalCopies::keep");
        let scope_id = self.scope_row(scope, name, mode)?;
        let mut rep = KeepReport {
            scope: scope_id,
            ..KeepReport::default()
        };
        let total = files.len() as u64;
        for (k, original) in files.iter().enumerate() {
            if cancelled() {
                rep.cancelled = true;
                break;
            }
            self.keep_one(scope_id, mode, original, proxy, &mut rep)?;
            progress(KeepProgress {
                done: k as u64 + 1,
                total,
                bytes: rep.bytes,
            });
        }
        // A scope that kept nothing (all offline, all over the limit) is not listed.
        let c = self.conn.lock();
        c.execute(
            "DELETE FROM local_scopes WHERE id = ?1
             AND NOT EXISTS (SELECT 1 FROM local_scope_files WHERE scope_id = ?1)",
            [scope_id],
        )?;
        Ok(rep)
    }

    /// The scope's row: the existing one for a location or album (its mode updated), a new
    /// one otherwise.
    fn scope_row(&self, scope: &Scope, name: &str, mode: Mode) -> Result<i64, Error> {
        let c = self.conn.lock();
        let (kind, target) = scope.code();
        if let Some(t) = target
            && let Some(id) = c
                .query_row(
                    "SELECT id FROM local_scopes WHERE kind = ?1 AND target = ?2",
                    params![kind, t],
                    |r| r.get::<_, i64>(0),
                )
                .optional()?
        {
            c.execute(
                "UPDATE local_scopes SET mode = ?2, name = ?3 WHERE id = ?1",
                params![id, mode.code(), name],
            )?;
            return Ok(id);
        }
        c.execute(
            "INSERT INTO local_scopes (kind, target, name, mode, created) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![kind, target, name, mode.code(), now_s()],
        )?;
        Ok(c.last_insert_rowid())
    }

    fn keep_one(
        &self,
        scope_id: i64,
        mode: Mode,
        original: &Path,
        proxy: Option<&ProxyMaker>,
        rep: &mut KeepReport,
    ) -> Result<(), Error> {
        let file_id = {
            let c = self.conn.lock();
            edits::file_id(&c, original)?
        };
        let Some(file_id) = file_id else {
            rep.not_catalogued += 1;
            return Ok(());
        };
        let existing = self.row(file_id)?;
        let now = Identity::of_file(original).ok();
        if let Some(ex) = &existing {
            let fresh = now.is_none_or(|n| n == ex.identity);
            let good_enough = mode == Mode::Proxies || ex.kind == CopyKind::Original;
            if fresh && good_enough && self.root.join(&ex.local).is_file() {
                self.link(scope_id, file_id)?;
                rep.shared += 1;
                return Ok(());
            }
        }
        let Some(identity) = now else {
            if existing.is_some() {
                // Offline: the copy there is the best there is.
                self.link(scope_id, file_id)?;
                rep.shared += 1;
            } else {
                rep.unreachable += 1;
            }
            return Ok(());
        };
        let ext = store::ext_of(&original.file_name().unwrap_or_default().to_string_lossy());
        let want_proxy = mode == Mode::Proxies && store::is_raw_ext(&ext);
        let made = if want_proxy && let Some(make) = proxy {
            match std::fs::read(original) {
                Ok(bytes) => make(original, &bytes, PROXY_LONG_EDGE),
                Err(_) => {
                    rep.unreachable += 1;
                    return Ok(());
                }
            }
        } else {
            None
        };
        let (kind, size) = match &made {
            Some(b) => (CopyKind::Proxy, b.len() as u64),
            None => (CopyKind::Original, identity.size),
        };
        let freed = existing.as_ref().map_or(0, |e| e.bytes);
        if self.used().saturating_sub(freed) + size > self.limit() {
            rep.over_limit += 1;
            return Ok(());
        }
        let shard = format!("{:02x}", identity.content & 0xff);
        let file_ext = match kind {
            CopyKind::Proxy => "dng".to_string(),
            CopyKind::Original if ext.is_empty() => "bin".to_string(),
            CopyKind::Original => ext.clone(),
        };
        let rel = format!(
            "{shard}/{:016x}-{file_id}-{}.{file_ext}",
            identity.content,
            if kind == CopyKind::Proxy { "p" } else { "o" }
        );
        let dest = self.root.join(&rel);
        let written = match &made {
            Some(b) => write_new(&dest, |f| {
                use std::io::Write;
                f.write_all(b)
            }),
            None => write_new(&dest, |f| {
                let mut src = std::fs::File::open(original)?;
                io::copy(&mut src, f).map(|_| ())
            }),
        };
        match written {
            Ok(n) if kind == CopyKind::Proxy || n == identity.size => {}
            Ok(_) => {
                // The original changed while it was copied: not a faithful copy.
                let _ = std::fs::remove_file(&dest);
                rep.unreachable += 1;
                return Ok(());
            }
            Err(e) => {
                specular_log::warn!(target: specular_log::FOLDER, "local copies: cannot copy {}: {e}", original.display());
                rep.unreachable += 1;
                return Ok(());
            }
        }
        let sidecars = encode_sidecars(&sidecars_of(original));
        {
            let c = self.conn.lock();
            c.execute(
                "INSERT INTO local_copies (file_id, kind, local, bytes, size, mtime_ns, content, sidecars, made)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(file_id) DO UPDATE SET kind = excluded.kind, local = excluded.local,
                   bytes = excluded.bytes, size = excluded.size, mtime_ns = excluded.mtime_ns,
                   content = excluded.content, sidecars = excluded.sidecars, made = excluded.made",
                params![
                    file_id,
                    kind.code(),
                    rel,
                    size as i64,
                    identity.size as i64,
                    identity.mtime_ns,
                    identity.content as i64,
                    sidecars,
                    now_s()
                ],
            )?;
            c.execute(
                "INSERT OR IGNORE INTO local_scope_files (scope_id, file_id) VALUES (?1, ?2)",
                params![scope_id, file_id],
            )?;
        }
        if let Some(ex) = existing
            && ex.local != rel
        {
            let _ = std::fs::remove_file(self.root.join(&ex.local));
        }
        self.refresh_used()?;
        self.index
            .write()
            .insert(original.to_path_buf(), LocalCopy { path: dest, kind });
        rep.copied += 1;
        rep.proxies += u64::from(kind == CopyKind::Proxy);
        rep.bytes += size;
        Ok(())
    }

    fn refresh_used(&self) -> Result<(), Error> {
        let n: i64 = self.conn.lock().query_row(
            "SELECT COALESCE(SUM(bytes), 0) FROM local_copies",
            [],
            |r| r.get(0),
        )?;
        self.used.store(n.max(0) as u64, Ordering::Relaxed);
        Ok(())
    }

    fn link(&self, scope_id: i64, file_id: i64) -> Result<(), Error> {
        self.conn.lock().execute(
            "INSERT OR IGNORE INTO local_scope_files (scope_id, file_id) VALUES (?1, ?2)",
            params![scope_id, file_id],
        )?;
        Ok(())
    }

    fn row(&self, file_id: i64) -> Result<Option<CopyRow>, Error> {
        let c = self.conn.lock();
        Ok(c.query_row(
            "SELECT kind, local, bytes, size, mtime_ns, content, sidecars FROM local_copies WHERE file_id = ?1",
            [file_id],
            |r| {
                Ok(CopyRow {
                    kind: CopyKind::of(r.get(0)?),
                    local: r.get(1)?,
                    bytes: r.get::<_, i64>(2)?.max(0) as u64,
                    identity: Identity {
                        size: r.get::<_, i64>(3)?.max(0) as u64,
                        mtime_ns: r.get(4)?,
                        content: r.get::<_, i64>(5)? as u64,
                    },
                    sidecars: r.get(6)?,
                })
            },
        )
        .optional()?)
    }

    /// Removes kept scope `id`: copies no other scope holds are deleted from the store. The
    /// originals are never touched, and waiting changes stay. Returns the bytes freed.
    pub fn remove(&self, id: i64) -> Result<u64, Error> {
        specular_platform::assert_not_ui_thread("LocalCopies::remove");
        let gone: Vec<(i64, String, i64)> = {
            let c = self.conn.lock();
            c.execute("DELETE FROM local_scopes WHERE id = ?1", [id])?;
            let rows = c
                .prepare(
                    "SELECT file_id, local, bytes FROM local_copies
                     WHERE file_id NOT IN (SELECT file_id FROM local_scope_files)",
                )?
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            for (f, _, _) in &rows {
                c.execute("DELETE FROM local_copies WHERE file_id = ?1", [f])?;
            }
            rows
        };
        let mut freed = 0;
        for (_, local, bytes) in gone {
            let _ = std::fs::remove_file(self.root.join(local));
            freed += bytes.max(0) as u64;
        }
        self.reload()?;
        Ok(freed)
    }

    /// Queues a change of `original` made while it is offline. Its base is the original's
    /// copy as made (identity and sidecars), else the catalogue's identity and sidecar time
    /// stamp. A change of the same kind (and version) already waiting keeps its base and
    /// takes the new payload. False when the catalogue does not hold `original`.
    pub fn enqueue(
        &self,
        original: &Path,
        kind: ChangeKind,
        version: u32,
        payload: &[u8],
    ) -> Result<bool, Error> {
        specular_platform::assert_not_ui_thread("LocalCopies::enqueue");
        let Some(p) = store::text(original) else {
            return Ok(false);
        };
        let c = self.conn.lock();
        let updated = c.execute(
            "UPDATE local_changes SET payload = ?4 WHERE path = ?1 AND kind = ?2 AND version = ?3",
            params![p, kind.code(), version, payload],
        )?;
        if updated > 0 {
            return Ok(true);
        }
        let Some(file_id) = edits::file_id(&c, original)? else {
            return Ok(false);
        };
        let base: Option<(i64, i64, i64, String)> = c
            .query_row(
                "SELECT size, mtime_ns, content, sidecars FROM local_copies WHERE file_id = ?1",
                [file_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        let (size, mtime, xmp_mtime, content): (i64, i64, Option<i64>, Option<i64>) = c.query_row(
            "SELECT f.size, f.mtime_ns, f.xmp_mtime, t.content FROM files f
             LEFT JOIN thumbs t ON t.file_id = f.id WHERE f.id = ?1",
            [file_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
        let (size, mtime, content, sidecars) = match base {
            Some((s, m, h, sc)) => (s, m, h, Some(sc)),
            // No copy: the catalogue's identity (a content hash of 0 when never read).
            None => (size, mtime, content.unwrap_or(0), None),
        };
        c.execute(
            "INSERT INTO local_changes
             (path, size, mtime_ns, content, sidecars, xmp_mtime, kind, version, payload, queued)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                p,
                size,
                mtime,
                content,
                sidecars,
                xmp_mtime,
                kind.code(),
                version,
                payload,
                now_s()
            ],
        )?;
        self.waiting.fetch_add(1, Ordering::Relaxed);
        Ok(true)
    }

    /// The keywords of `original` as the catalogue holds them, a waiting keyword change
    /// winning; None when it is not catalogued.
    pub fn keywords_of(&self, original: &Path) -> Result<Option<Vec<String>>, Error> {
        let Some(p) = store::text(original) else {
            return Ok(None);
        };
        let c = self.conn.lock();
        let queued: Option<Vec<u8>> = c
            .query_row(
                "SELECT payload FROM local_changes WHERE path = ?1 AND kind = ?2",
                params![p, ChangeKind::Keywords.code()],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(q) = queued {
            return Ok(Some(decode_keywords(&q)));
        }
        let Some(id) = edits::file_id(&c, original)? else {
            return Ok(None);
        };
        let v: Vec<String> = c
            .prepare_cached(
                "SELECT t.name FROM file_tags ft JOIN tags t ON t.id = ft.tag_id
                 WHERE ft.file_id = ?1 ORDER BY t.name",
            )?
            .query_map([id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Some(v))
    }

    /// Every waiting change, oldest first.
    pub fn changes(&self) -> Result<Vec<Change>, Error> {
        self.changes_where("1", [])
    }

    /// The waiting changes of originals under `dir`.
    pub fn changes_under(&self, dir: &Path) -> Result<Vec<Change>, Error> {
        let Some(d) = store::text(dir) else {
            return Ok(Vec::new());
        };
        let d = d.trim_end_matches('/');
        let mut v = self.changes()?;
        v.retain(|c| {
            c.path.to_str().is_some_and(|p| {
                p.strip_prefix(d)
                    .is_some_and(|r| r.starts_with('/') || r.starts_with('\\'))
            })
        });
        Ok(v)
    }

    fn changes_where<P: rusqlite::Params>(&self, cond: &str, p: P) -> Result<Vec<Change>, Error> {
        let c = self.conn.lock();
        let sql = format!(
            "SELECT id, path, size, mtime_ns, content, sidecars, xmp_mtime, kind, version, payload, queued
             FROM local_changes WHERE {cond} ORDER BY queued, id"
        );
        let mut st = c.prepare(&sql)?;
        let rows = st.query_map(p, |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<i64>>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, i64>(8)?,
                r.get::<_, Vec<u8>>(9)?,
                r.get::<_, i64>(10)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, path, size, mtime, content, sc, xmp, kind, version, payload, queued) = row?;
            let Some(kind) = ChangeKind::of(kind) else {
                continue;
            };
            out.push(Change {
                id,
                path: PathBuf::from(path),
                identity: Identity {
                    size: size.max(0) as u64,
                    mtime_ns: mtime,
                    content: content as u64,
                },
                sidecars: sc.as_deref().map(decode_sidecars),
                xmp_mtime: xmp,
                kind,
                version: version.max(0) as u32,
                payload,
                queued,
            });
        }
        Ok(out)
    }

    /// Whether `change` can be written now (reads the original's identity and its sidecars).
    pub fn check(&self, change: &Change) -> Check {
        check(change)
    }

    /// Drops waiting changes (written, or the user kept the file's version).
    pub fn done(&self, ids: &[i64]) -> Result<(), Error> {
        let c = self.conn.lock();
        for id in ids {
            c.execute("DELETE FROM local_changes WHERE id = ?1", [id])?;
        }
        let n: i64 = c.query_row("SELECT COUNT(*) FROM local_changes", [], |r| r.get(0))?;
        self.waiting.store(n.max(0) as u64, Ordering::Relaxed);
        Ok(())
    }

    /// After a change was written to the original's sidecars: the copy's base becomes the
    /// sidecars as they are now, so the next offline change is checked against them.
    pub fn rebase(&self, original: &Path) -> Result<(), Error> {
        let c = self.conn.lock();
        let Some(file_id) = edits::file_id(&c, original)? else {
            return Ok(());
        };
        let sc = encode_sidecars(&sidecars_of(original));
        let mut sets = "sidecars = ?2".to_string();
        let id = Identity::of_file(original).ok();
        if id.is_some() {
            sets.push_str(", size = ?3, mtime_ns = ?4, content = ?5");
        }
        let id = id.unwrap_or_default();
        let sql = format!("UPDATE local_copies SET {sets} WHERE file_id = ?1");
        if sets.contains("?3") {
            c.execute(
                &sql,
                params![file_id, sc, id.size as i64, id.mtime_ns, id.content as i64],
            )?;
        } else {
            c.execute(&sql, params![file_id, sc])?;
        }
        Ok(())
    }
}

/// A copy's row.
struct CopyRow {
    kind: CopyKind,
    local: String,
    bytes: u64,
    identity: Identity,
    #[allow(dead_code)]
    sidecars: String,
}

/// Whether `change` can be written to its original as it is now ([`LocalCopies::check`]).
pub fn check(change: &Change) -> Check {
    let Ok(now) = Identity::of_file(&change.path) else {
        return Check::Missing;
    };
    // A content hash of 0: the catalogue never read it; size and time stamp must do.
    let original = now.size != change.identity.size
        || now.mtime_ns != change.identity.mtime_ns
        || (change.identity.content != 0 && now.content != change.identity.content);
    let relevant = |v: &[SidecarState]| -> Vec<SidecarState> {
        v.iter()
            .filter(|s| touches(&s.name, change.kind, change.version))
            .cloned()
            .collect()
    };
    let current = sidecars_of(&change.path);
    let sidecar = match &change.sidecars {
        Some(base) => relevant(base) != relevant(&current),
        None => {
            // Only the catalogue's time stamp of the plain sidecar is known.
            let plain: Vec<&SidecarState> = current
                .iter()
                .filter(|s| touches(&s.name, ChangeKind::Marks, 0))
                .collect();
            match change.kind {
                ChangeKind::Edit => false,
                _ => match (change.xmp_mtime, plain.as_slice()) {
                    (None, []) => false,
                    (None, _) => true,
                    (Some(_), []) => true,
                    (Some(t), s) => s.iter().all(|s| s.mtime_ns != t),
                },
            }
        }
    };
    if original || sidecar {
        Check::Conflict { original, sidecar }
    } else {
        Check::Clean
    }
}

/// Writes a new file at `dest` through a temporary beside it; its length.
fn write_new(
    dest: &Path,
    fill: impl FnOnce(&mut std::fs::File) -> io::Result<()>,
) -> io::Result<u64> {
    let dir = dest.parent().ok_or_else(|| io::Error::other("no folder"))?;
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".{}.{}.part",
        dest.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ));
    let r = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        fill(&mut f)?;
        f.sync_all()?;
        let n = f.metadata()?.len();
        drop(f);
        std::fs::rename(&tmp, dest)?;
        Ok(n)
    })();
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn sidecar_names_belong_to_their_image() {
        let b = |n| belongs(n, "IMG_1.CR3", "IMG_1");
        assert!(b("IMG_1.xmp"));
        assert!(b("IMG_1.XMP"));
        assert!(b("IMG_1.CR3.xmp"));
        assert!(b("IMG_1.CR3.specular.xmp"));
        assert!(b("IMG_1.CR3.v2.specular.xmp"));
        assert!(!b("IMG_10.xmp"));
        assert!(!b("IMG_1.JPG.xmp"));
        assert!(!b("IMG_1.CR3"));
    }

    #[test]
    fn changes_touch_their_own_sidecars() {
        assert!(touches("IMG_1.xmp", ChangeKind::Marks, 0));
        assert!(touches("IMG_1.CR3.xmp", ChangeKind::Keywords, 0));
        assert!(!touches("IMG_1.CR3.specular.xmp", ChangeKind::Marks, 0));
        assert!(touches("IMG_1.CR3.specular.xmp", ChangeKind::Edit, 0));
        assert!(!touches("IMG_1.CR3.specular.xmp", ChangeKind::Edit, 1));
        assert!(touches("IMG_1.CR3.v2.specular.xmp", ChangeKind::Edit, 2));
        assert!(!touches("IMG_1.CR3.v2.specular.xmp", ChangeKind::Edit, 1));
        assert!(!touches("IMG_1.CR3.v2.specular.xmp", ChangeKind::Edit, 0));
        assert!(!touches("IMG_1.xmp", ChangeKind::Edit, 0));
    }

    #[test]
    fn payloads_round_trip() {
        let m = crate::Mark {
            rating: 4,
            label: 2,
            flag: -1,
        };
        assert_eq!(decode_marks(&encode_marks(m)), Some(m));
        assert_eq!(decode_marks(b"x"), None);
        let k = vec!["Places|Beach".to_string(), "sun".into()];
        assert_eq!(decode_keywords(&encode_keywords(&k)), k);
        let f = vec![
            (
                "Iptc4xmpCore:Location".to_string(),
                "Pier\\3\nwest".to_string(),
            ),
            ("dc:title".into(), String::new()),
        ];
        assert_eq!(decode_fields(&encode_fields(&f)), f);
        let s = vec![SidecarState {
            name: "IMG.xmp".into(),
            mtime_ns: -5,
            hash: u64::MAX,
        }];
        assert_eq!(decode_sidecars(&encode_sidecars(&s)), s);
    }
}
