//! The walk: lists folders and brings the catalogue's file rows in line with them.
//!
//! Each folder is compared with its rows: unchanged files cost a `stat` and nothing else,
//! changed ones (size, time stamp, identity or sidecar) are marked for the facts pass, new
//! ones are added and missing ones set aside. Only once the walk is over are the missing
//! matched against the new by identity (device and inode, with the size or the name agreeing;
//! size, time and name where there is no inode): a match is a move or rename, and the old row
//! (its marks, tags and thumbnail reference) takes the new place. What is left is deleted,
//! unless the location's root has gone meanwhile (an unmounted volume), which makes it
//! offline instead.

use std::collections::{HashMap, HashSet};
use std::path::{MAIN_SEPARATOR, Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use rusqlite::{Connection, OptionalExtension, params};
use specular_thumbs::watch::FolderWatch;

use crate::skip::{Skip, hidden};
use crate::store;
use crate::{Fold, Shared};

/// Commit after this many changed rows…
const COMMIT_EVERY: usize = 2000;
/// …or this long after the first, so the grid fills while a long walk goes on.
const COMMIT_AFTER: Duration = Duration::from_millis(500);
/// Entries of a folder between updates of the running count (a folder of 100 000 files
/// still shows the count moving).
const COUNT_EVERY: usize = 512;

/// What a file is recognised by.
#[derive(Clone, Debug)]
struct Ident {
    id: i64,
    dir_id: i64,
    name: String,
    size: i64,
    mtime: i64,
    dev: Option<i64>,
    ino: Option<i64>,
    xmp: Option<String>,
    /// Every sidecar's name and time stamp ([`sides_signature`]).
    sides: Option<String>,
}

/// What a walk did.
#[derive(Debug, Default)]
pub struct Outcome {
    /// Rows added, changed, moved or deleted.
    pub changes: u64,
    /// Images listed.
    pub walked: u64,
    /// Stopped before the end (shutdown): nothing was deleted.
    pub aborted: bool,
    /// The root was missing at the end: nothing was deleted.
    pub root_gone: bool,
    /// Folders left as they were because walking them panicked (a bug, logged with the
    /// folder); the rest of the walk went on.
    pub skipped: u64,
}

pub struct Walker<'a> {
    pub conn: &'a Connection,
    pub is_image: &'a (dyn Fn(&Path) -> bool + Send + Sync),
    pub fold: Fold,
    pub shared: &'a Shared,
    /// Called with the running count of images listed (throttled by the caller).
    pub progress: &'a mut dyn FnMut(u64),
    pub folder: i64,
    pub root: PathBuf,
    /// Folders passed over.
    skip: &'a Skip,
    /// Watches each folder listed ([`Walker::watching`]).
    watch: Option<&'a mut FolderWatch>,
    /// Reads Develop sidecars the catalogue has no edit for ([`Walker::importing`]).
    import: Option<&'a crate::EditImport>,
    /// Develop sidecars seen: (folder id, photo path, version).
    sidecars: Vec<(i64, PathBuf, u32)>,
    gone: Vec<Ident>,
    new: Vec<Ident>,
    vanished: Vec<i64>,
    out: Outcome,
    open_tx: bool,
    in_tx: usize,
    /// When the open transaction began.
    tx_at: Instant,
    /// `out.changes` when the catalogue's readers were last told of new rows.
    told: u64,
}

pub fn mtime_ns(t: Option<SystemTime>) -> i64 {
    t.map_or(0, |t| {
        specular_thumbs::key::time_ns(t).clamp(i64::MIN as i128, i64::MAX as i128) as i64
    })
}

#[cfg(unix)]
fn identity(m: &std::fs::Metadata) -> (Option<i64>, Option<i64>) {
    use std::os::unix::fs::MetadataExt;
    (Some(m.dev() as i64), Some(m.ino() as i64))
}

/// NTFS's device and inode: the volume serial number and the file index. Only metadata read
/// through a handle (`fs::metadata`) has them; a directory listing's has neither.
#[cfg(windows)]
fn identity(m: &std::fs::Metadata) -> (Option<i64>, Option<i64>) {
    use std::os::windows::fs::MetadataExt;
    (
        m.volume_serial_number().map(i64::from),
        m.file_index().map(|i| i as i64),
    )
}

#[cfg(not(any(unix, windows)))]
fn identity(_: &std::fs::Metadata) -> (Option<i64>, Option<i64>) {
    (None, None)
}

/// The root's device (to tell an empty mount point from the mounted volume).
pub fn device(m: &std::fs::Metadata) -> Option<i64> {
    identity(m).0
}

/// `[lo, hi)` bounds of the paths strictly below `dir`.
pub fn below(dir: &str) -> (String, String) {
    let sep = MAIN_SEPARATOR;
    let next = char::from_u32(sep as u32 + 1).unwrap_or('0');
    let base = dir.trim_end_matches(sep);
    (format!("{base}{sep}"), format!("{base}{next}"))
}

fn is_xmp(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() > 4 && b[b.len() - 4..].eq_ignore_ascii_case(b".xmp")
}

/// The `files.sides` of a file whose sidecars are `names` (in their order of weight), with
/// each one's time stamp from `mtime`: one `name \t mtime_ns` line each; None for none.
pub fn sides_signature(
    names: impl IntoIterator<Item = String>,
    mtime: &dyn Fn(&str) -> i64,
) -> Option<String> {
    let mut out = String::new();
    for n in names {
        if n.contains(['\t', '\n']) {
            continue;
        }
        out.push_str(&format!("{n}\t{}\n", mtime(&n)));
    }
    (!out.is_empty()).then_some(out)
}

/// The names and time stamps of a `files.sides` value, in its order.
pub fn sides_names(sides: &str) -> impl Iterator<Item = (&str, i64)> {
    sides.lines().filter_map(|l| {
        let (n, t) = l.split_once('\t')?;
        Some((n, t.parse().ok()?))
    })
}

/// Whether a name is lyrics for a sound file beside it (`specular_video::lyrics` reads
/// `stem.lrc`, `.LRC` or `.Lrc`).
fn is_lrc(name: &str) -> bool {
    [".lrc", ".LRC", ".Lrc"].iter().any(|e| name.ends_with(e))
}

/// The modification time of `name`'s lyrics sidecar among the folder's `.lrc` files, in the
/// order the sound reader looks for them.
fn lrc_of(name: &str, lrc: &HashMap<String, i64>) -> Option<i64> {
    if lrc.is_empty() {
        return None;
    }
    let stem = match name.rsplit_once('.') {
        Some((s, _)) if !s.is_empty() => s,
        _ => name,
    };
    ["lrc", "LRC", "Lrc"]
        .iter()
        .find_map(|e| lrc.get(&format!("{stem}.{e}")).copied())
}

struct Listed {
    name: String,
    size: i64,
    mtime: i64,
    dev: Option<i64>,
    ino: Option<i64>,
}

impl<'a> Walker<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        conn: &'a Connection,
        is_image: &'a (dyn Fn(&Path) -> bool + Send + Sync),
        fold: Fold,
        shared: &'a Shared,
        progress: &'a mut dyn FnMut(u64),
        folder: i64,
        root: PathBuf,
        skip: &'a Skip,
    ) -> Walker<'a> {
        Walker {
            conn,
            is_image,
            fold,
            shared,
            progress,
            folder,
            root,
            skip,
            watch: None,
            import: None,
            sidecars: Vec::new(),
            gone: Vec::new(),
            new: Vec::new(),
            vanished: Vec::new(),
            out: Outcome::default(),
            open_tx: false,
            in_tx: 0,
            tx_at: Instant::now(),
            told: 0,
        }
    }

    /// Watches the folders walked (and lets go of the ones that went).
    pub fn watching(mut self, watch: Option<&'a mut FolderWatch>) -> Walker<'a> {
        self.watch = watch;
        self
    }

    /// Takes the edits of Develop sidecars the catalogue has none for (edits saved while
    /// the photo was outside the Library, or by another copy of Specular), so the Library's
    /// `edited:` agrees with the folder view's. A catalogued edit is never replaced: a newer
    /// sidecar with another edit is Develop's conflict to ask about.
    pub fn importing(mut self, import: Option<&'a crate::EditImport>) -> Walker<'a> {
        self.import = import;
        self
    }

    /// The photo and version a Develop sidecar's name is of (`IMG.CR2.v2<suffix>` →
    /// `IMG.CR2`, 2).
    fn sidecar_of<'n>(name: &'n str, suffix: &str) -> Option<(&'n str, u32)> {
        let rest = name.strip_suffix(suffix)?;
        let (photo, version) = match rest.rsplit_once(".v") {
            Some((p, v)) if !p.is_empty() && v.parse::<u32>().is_ok_and(|v| v > 0) => {
                (p, v.parse().ok()?)
            }
            _ => (rest, 0),
        };
        (!photo.is_empty()).then_some((photo, version))
    }

    /// Imports the sidecars seen whose photo is catalogued without that version's edit.
    fn import_sidecars(&mut self) -> rusqlite::Result<()> {
        let Some(import) = self.import else {
            return Ok(());
        };
        for (dir_id, photo, version) in std::mem::take(&mut self.sidecars) {
            let Some(name) = photo.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let Some(id) = self
                .conn
                .prepare_cached("SELECT id FROM files WHERE dir_id = ?1 AND name = ?2")?
                .query_row(params![dir_id, name], |r| r.get::<_, i64>(0))
                .optional()?
            else {
                continue;
            };
            let has = self
                .conn
                .prepare_cached("SELECT 1 FROM edits WHERE file_id = ?1 AND version = ?2")?
                .exists(params![id, version])?;
            if has {
                continue;
            }
            let Some(e) = (import.read)(&photo, version) else {
                continue;
            };
            self.touch(1)?;
            crate::edits::insert(self.conn, id, version, &e)?;
            self.out.changes += 1;
        }
        self.commit()
    }

    fn touch(&mut self, n: usize) -> rusqlite::Result<()> {
        if !self.open_tx {
            self.conn.execute_batch("BEGIN IMMEDIATE")?;
            self.open_tx = true;
            self.tx_at = Instant::now();
        }
        self.in_tx += n;
        Ok(())
    }

    fn commit(&mut self) -> rusqlite::Result<()> {
        if self.open_tx {
            self.conn.execute_batch("COMMIT")?;
            self.open_tx = false;
            self.in_tx = 0;
            // A long walk's rows are searchable as they come.
            if self.out.changes > self.told {
                self.told = self.out.changes;
                self.shared.progress.lock().changes += 1;
            }
        }
        Ok(())
    }

    /// Walks `starts` (each with whether to go below it; new folders are always walked
    /// whole), then settles moves and deletions.
    pub fn run(mut self, starts: Vec<(PathBuf, bool)>) -> rusqlite::Result<Outcome> {
        let r = self.walk(starts);
        if r.is_err() && self.open_tx {
            let _ = self.conn.execute_batch("ROLLBACK");
            self.open_tx = false;
        }
        r?;
        Ok(self.out)
    }

    fn walk(&mut self, starts: Vec<(PathBuf, bool)>) -> rusqlite::Result<()> {
        let mut stack: Vec<(PathBuf, bool, Option<i64>)> = Vec::new();
        for (dir, recursive) in starts.into_iter().rev() {
            let parent = if dir == self.root {
                None
            } else {
                match dir.parent().and_then(store::text) {
                    Some(p) => self.dir_id(p)?,
                    None => None,
                }
            };
            stack.push((dir, recursive, parent));
        }
        while let Some((dir, recursive, parent)) = stack.pop() {
            // Stopped, closing, or a location's removal waits (this one's or another's: the
            // indexer walks this one again after it).
            let shared = self.shared;
            let stop = || shared.halting() || shared.removal_waits();
            if stop() {
                self.out.aborted = true;
                return self.commit();
            }
            // Browsing slows the walk ([`Shared::held`] lets a quarter of the time through).
            while self.shared.held() && !stop() {
                self.commit()?;
                std::thread::sleep(Duration::from_millis(25));
            }
            // A panic while listing one folder (a name or file the walk trips on) skips that
            // folder and what is below it; the rest of the location is walked. Rows it wrote
            // stay in the open transaction, as a database error's would not.
            let what = format_args!("library walk of {}", dir.display());
            match specular_core::isolate::run(what, || {
                self.dir(&dir, recursive, parent, &mut stack)
            }) {
                Ok(r) => r?,
                Err(_) => self.out.skipped += 1,
            }
            if self.in_tx >= COMMIT_EVERY || self.open_tx && self.tx_at.elapsed() >= COMMIT_AFTER {
                self.commit()?;
            }
        }
        self.commit()?;
        if !self.root.is_dir() {
            self.out.root_gone = true;
            return Ok(());
        }
        self.settle()?;
        self.import_sidecars()
    }

    fn dir_id(&self, path: &str) -> rusqlite::Result<Option<i64>> {
        self.conn
            .prepare_cached("SELECT id FROM dirs WHERE path = ?1")?
            .query_row([path], |r| r.get(0))
            .optional()
    }

    /// The folder's row, added or brought under this location.
    fn ensure_dir(&mut self, path: &str, parent: Option<i64>, mtime: i64) -> rusqlite::Result<i64> {
        let found: Option<(i64, i64, Option<i64>, i64)> = self
            .conn
            .prepare_cached("SELECT id, folder_id, parent, mtime_ns FROM dirs WHERE path = ?1")?
            .query_row([path], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .optional()?;
        match found {
            Some((id, f, p, m)) => {
                if f != self.folder || p != parent || m != mtime {
                    self.touch(1)?;
                    self.conn
                        .prepare_cached(
                            "UPDATE dirs SET folder_id = ?2, parent = ?3, mtime_ns = ?4 WHERE id = ?1",
                        )?
                        .execute(params![id, self.folder, parent, mtime])?;
                }
                Ok(id)
            }
            None => {
                self.touch(1)?;
                self.conn
                    .prepare_cached(
                        "INSERT INTO dirs (folder_id, parent, path, mtime_ns) VALUES (?1, ?2, ?3, ?4)",
                    )?
                    .execute(params![self.folder, parent, path, mtime])?;
                let id = self.conn.last_insert_rowid();
                self.conn
                    .prepare_cached("INSERT INTO fts_dirs (rowid, path) VALUES (?1, ?2)")?
                    .execute(params![id, (self.fold)(path)])?;
                Ok(id)
            }
        }
    }

    /// The files of a folder that is no longer there (and of the folders below it) are set
    /// aside; its rows go once moves are settled.
    fn vanish(&mut self, id: i64, path: &str) -> rusqlite::Result<()> {
        let (lo, hi) = below(path);
        let mut st = self.conn.prepare_cached(
            "SELECT f.id, f.dir_id, f.name, f.size, f.mtime_ns, f.dev, f.inode, f.xmp, f.sides
             FROM files f JOIN dirs d ON d.id = f.dir_id
             WHERE d.id = ?1 OR (d.path >= ?2 AND d.path < ?3)",
        )?;
        let rows = st.query_map(params![id, lo, hi], ident)?;
        for r in rows {
            self.gone.push(r?);
        }
        self.vanished.push(id);
        if let Some(w) = self.watch.as_deref_mut() {
            w.remove_below(Path::new(path));
        }
        Ok(())
    }

    fn dir(
        &mut self,
        dir: &Path,
        recursive: bool,
        parent: Option<i64>,
        stack: &mut Vec<(PathBuf, bool, Option<i64>)>,
    ) -> rusqlite::Result<()> {
        specular_core::isolate::trip_sniff(dir);
        let Some(dir_s) = store::text(dir) else {
            return Ok(());
        };
        // Watched before it is listed, so nothing written meanwhile is missed.
        if let Some(w) = self.watch.as_deref_mut() {
            w.add(dir);
        }
        // A folder that became a link since it was listed (to `/`, say) is not followed: it
        // is gone, like a folder that became a file.
        let linked = dir != self.root
            && std::fs::symlink_metadata(dir).is_ok_and(|m| m.file_type().is_symlink());
        let listed = if linked {
            Err(std::io::Error::from(std::io::ErrorKind::NotADirectory))
        } else {
            std::fs::read_dir(dir)
        };
        let rd = match listed {
            Ok(rd) => rd,
            Err(e) => {
                // Gone since its folder was listed, or a file of its name now.
                let gone = matches!(
                    e.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                );
                if gone
                    && let Some(id) = self.dir_id(dir_s)?
                    && dir != self.root
                {
                    self.vanish(id, dir_s)?;
                } else {
                    specular_log::debug!(target: specular_log::FOLDER, "library: cannot list a folder: {e}");
                }
                return Ok(());
            }
        };
        let dir_mtime = mtime_ns(std::fs::metadata(dir).ok().and_then(|m| m.modified().ok()));
        let mut files: Vec<Listed> = Vec::new();
        let mut xmp: HashMap<String, i64> = HashMap::new();
        let mut lrc: HashMap<String, i64> = HashMap::new();
        // Other apps' sidecars that are not XMP (RawTherapee's `.pp3`, Capture One's `.cos`).
        let mut others: HashMap<String, i64> = HashMap::new();
        let mut capture_one = false;
        let mut subdirs: Vec<String> = Vec::new();
        let mut tool_dir = false;
        for (k, e) in rd.flatten().enumerate() {
            if k % COUNT_EVERY == COUNT_EVERY - 1 {
                (self.progress)(self.out.walked + files.len() as u64);
            }
            let Ok(name) = e.file_name().into_string() else {
                continue;
            };
            // A folder holding a mark (`.git`, `pyvenv.cfg`…) is a tool's, unless it is the
            // location itself.
            if dir != self.root.as_path() && self.skip.mark(&name, dir) {
                tool_dir = true;
                break;
            }
            if hidden(&name) {
                continue;
            }
            // Symbolic links to folders are not followed (`file_type` does not follow them).
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                capture_one |= name == specular_metadata::sidecars::CAPTURE_ONE_DIR;
                if !self.skip.name(&name) {
                    subdirs.push(name);
                }
                continue;
            }
            let path = e.path();
            if is_xmp(&name) {
                let m = std::fs::metadata(&path).ok();
                xmp.insert(name, mtime_ns(m.and_then(|m| m.modified().ok())));
                continue;
            }
            if is_lrc(&name) {
                let m = std::fs::metadata(&path).ok();
                lrc.insert(name, mtime_ns(m.and_then(|m| m.modified().ok())));
                continue;
            }
            if specular_metadata::sidecars::Kind::of_name(&name)
                == Some(specular_metadata::sidecars::Kind::Pp3)
            {
                let m = std::fs::metadata(&path).ok();
                others.insert(name, mtime_ns(m.and_then(|m| m.modified().ok())));
                continue;
            }
            if !(self.is_image)(&path) {
                continue;
            }
            // Symbolic links to files count (their target's facts); to folders, not. On Windows
            // a listing's metadata has no file identity, so it is read by path there.
            let meta = if ft.is_symlink() || cfg!(windows) {
                std::fs::metadata(&path)
            } else {
                e.metadata()
            };
            let Ok(m) = meta else { continue };
            if !m.is_file() {
                continue;
            }
            let (dev, ino) = identity(&m);
            files.push(Listed {
                name,
                size: m.len() as i64,
                mtime: mtime_ns(m.modified().ok()),
                dev,
                ino,
            });
        }
        if tool_dir {
            match self.dir_id(dir_s)? {
                Some(id) => self.vanish(id, dir_s)?,
                None => {
                    if let Some(w) = self.watch.as_deref_mut() {
                        w.remove_below(dir);
                    }
                }
            }
            return Ok(());
        }
        let dir_id = self.ensure_dir(dir_s, parent, dir_mtime)?;
        self.out.walked += files.len() as u64;
        (self.progress)(self.out.walked);

        // The sidecar rule needs the stems of the folder's images.
        let mut stems: HashMap<String, (u32, u32)> = HashMap::new();
        for f in &files {
            let (stem, ext) = match f.name.rsplit_once('.') {
                Some((s, e)) if !s.is_empty() => (s, e),
                _ => (f.name.as_str(), ""),
            };
            let e = stems.entry(stem.to_owned()).or_default();
            e.0 += 1;
            e.1 += u32::from(store::is_raw_ext(ext));
        }
        let xmp_names: HashSet<String> = xmp.keys().cloned().collect();
        // Capture One's settings, in its folder beside the images.
        let cos_dirs = if capture_one {
            capture_one_listing(dir, &mut others)
        } else {
            Vec::new()
        };
        // darktable's duplicates (`IMG_01.CR2.xmp`) of the images listed, by image.
        let mut duplicates: HashMap<String, Vec<(u32, String)>> = HashMap::new();
        {
            let names: HashSet<&str> = files.iter().map(|f| f.name.as_str()).collect();
            for n in &xmp_names {
                if let Some((photo, k)) = specular_metadata::sidecars::duplicate_of(n)
                    && names.contains(photo.as_str())
                    && !names.contains(&n[..n.len() - 4])
                {
                    duplicates.entry(photo).or_default().push((k, n.clone()));
                }
            }
        }
        if let Some(import) = self.import {
            for n in &xmp_names {
                if let Some((photo, version)) = Self::sidecar_of(n, import.suffix) {
                    self.sidecars.push((dir_id, dir.join(photo), version));
                }
            }
        }

        type Old = (Ident, Option<i64>, Option<i64>);
        let mut old: HashMap<String, Old> = HashMap::new();
        {
            let mut st = self.conn.prepare_cached(
                "SELECT id, dir_id, name, size, mtime_ns, dev, inode, xmp, sides, xmp_mtime,
                 lrc_mtime FROM files WHERE dir_id = ?1",
            )?;
            let rows = st.query_map([dir_id], |r| {
                Ok((
                    ident(r)?,
                    r.get::<_, Option<i64>>(9)?,
                    r.get::<_, Option<i64>>(10)?,
                ))
            })?;
            for r in rows {
                let (i, xm, lm) = r?;
                old.insert(i.name.clone(), (i, xm, lm));
            }
        }
        for f in files {
            let side = store::sidecar_for(&f.name, &stems, &xmp_names).map(str::to_owned);
            let side_mtime = side.as_ref().and_then(|s| xmp.get(s)).copied();
            let lrc_mtime = lrc_of(&f.name, &lrc);
            let sides = {
                let has = |n: &str| xmp.contains_key(n) || others.contains_key(n);
                let own = store::stem_is_own(&f.name, &stems);
                let mut c: Vec<String> =
                    specular_metadata::sidecars::candidates(&f.name, own, &cos_dirs, &has)
                        .into_iter()
                        .map(|c| c.name)
                        .collect();
                if let Some(d) = duplicates.get_mut(&f.name) {
                    d.sort();
                    c.extend(d.iter().map(|(_, n)| n.clone()));
                }
                let at = |n: &str| xmp.get(n).or_else(|| others.get(n)).copied().unwrap_or(0);
                sides_signature(c, &at)
            };
            match old.remove(&f.name) {
                Some((o, o_xm, o_lm)) => {
                    let same = o.size == f.size
                        && o.mtime == f.mtime
                        && o.dev == f.dev
                        && o.ino == f.ino
                        && o.xmp == side
                        && (side.is_none() || o_xm == side_mtime)
                        && o.sides == sides;
                    // a new, changed or removed lyrics sidecar: the words are read again
                    let same_lrc = o_lm == lrc_mtime;
                    if !same || !same_lrc {
                        self.touch(1)?;
                        self.conn
                            .prepare_cached(
                                "UPDATE files SET size = ?2, mtime_ns = ?3, dev = ?4, inode = ?5,
                                 xmp = ?6, lrc_mtime = ?7, sides = ?8, state = 0 WHERE id = ?1",
                            )?
                            .execute(params![
                                o.id, f.size, f.mtime, f.dev, f.ino, side, lrc_mtime, sides
                            ])?;
                        self.out.changes += 1;
                    }
                }
                None => {
                    self.touch(1)?;
                    let ext = store::ext_of(&f.name);
                    self.conn
                        .prepare_cached(
                            "INSERT INTO files (dir_id, name, ext, size, mtime_ns, dev, inode, xmp,
                             lrc_mtime, sides, fts)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0)",
                        )?
                        .execute(params![
                            dir_id, f.name, ext, f.size, f.mtime, f.dev, f.ino, side, lrc_mtime,
                            sides
                        ])?;
                    // Its full-text row comes with its facts ([`store::fts_first`]).
                    let id = self.conn.last_insert_rowid();
                    self.out.changes += 1;
                    self.new.push(Ident {
                        id,
                        dir_id,
                        name: f.name,
                        size: f.size,
                        mtime: f.mtime,
                        dev: f.dev,
                        ino: f.ino,
                        xmp: side,
                        sides,
                    });
                }
            }
        }
        self.gone.extend(old.into_values().map(|(i, _, _)| i));

        // Folders below: vanished ones go, new ones are walked whole.
        let known: Vec<(i64, String)> = self
            .conn
            .prepare_cached("SELECT id, path FROM dirs WHERE parent = ?1")?
            .query_map([dir_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        let listed: HashSet<&str> = subdirs.iter().map(String::as_str).collect();
        let mut known_names = HashSet::new();
        for (id, p) in known {
            let name = Path::new(&p)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_owned();
            if listed.contains(name.as_str()) {
                known_names.insert(name);
            } else {
                self.vanish(id, &p)?;
            }
        }
        for name in subdirs.into_iter().rev() {
            if recursive || !known_names.contains(&name) {
                stack.push((dir.join(&name), true, Some(dir_id)));
            }
        }
        Ok(())
    }

    /// Moves and renames keep their rows; the rest of what was not found goes.
    fn settle(&mut self) -> rusqlite::Result<()> {
        if self.gone.is_empty() && self.vanished.is_empty() {
            return Ok(());
        }
        let mut by_ino: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        let mut by_stamp: HashMap<(i64, i64, &str), Vec<usize>> = HashMap::new();
        for (i, n) in self.new.iter().enumerate() {
            match (n.dev, n.ino) {
                (Some(d), Some(ino)) => by_ino.entry((d, ino)).or_default().push(i),
                _ => by_stamp
                    .entry((n.size, n.mtime, n.name.as_str()))
                    .or_default()
                    .push(i),
            }
        }
        let mut taken = vec![false; self.new.len()];
        let mut moves: Vec<(usize, usize)> = Vec::new();
        let mut gone_ids: HashSet<i64> = HashSet::new();
        for (gi, g) in self.gone.iter().enumerate() {
            if !gone_ids.insert(g.id) {
                continue;
            }
            let cands = match (g.dev, g.ino) {
                (Some(d), Some(ino)) => by_ino.get(&(d, ino)),
                _ => by_stamp.get(&(g.size, g.mtime, g.name.as_str())),
            };
            let pick = cands.and_then(|c| {
                c.iter().copied().find(|&ni| {
                    let n = &self.new[ni];
                    !taken[ni] && (n.size == g.size || n.name == g.name)
                })
            });
            if let Some(ni) = pick {
                taken[ni] = true;
                moves.push((gi, ni));
            }
        }
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let moved: HashSet<i64> = moves.iter().map(|&(gi, _)| self.gone[gi].id).collect();
        for &(gi, ni) in &moves {
            let (g, n) = (&self.gone[gi], &self.new[ni]);
            self.conn
                .prepare_cached("DELETE FROM files WHERE id = ?1")?
                .execute([n.id])?;
            self.conn
                .prepare_cached(
                    "UPDATE files SET dir_id = ?2, name = ?3, ext = ?4, size = ?5, mtime_ns = ?6,
                     dev = ?7, inode = ?8, xmp = ?9, sides = ?10,
                     state = CASE WHEN size = ?5 AND mtime_ns = ?6 AND xmp IS ?9 AND sides IS ?10
                         THEN state ELSE 0 END
                     WHERE id = ?1",
                )?
                .execute(params![
                    g.id,
                    n.dir_id,
                    n.name,
                    store::ext_of(&n.name),
                    n.size,
                    n.mtime,
                    n.dev,
                    n.ino,
                    n.xmp,
                    n.sides
                ])?;
            store::fts_refresh(self.conn, g.id, self.fold)?;
        }
        let mut del = self
            .conn
            .prepare_cached("DELETE FROM files WHERE id = ?1")?;
        for g in &self.gone {
            if !moved.contains(&g.id) {
                self.out.changes += del.execute([g.id])? as u64;
            }
        }
        drop(del);
        for id in &self.vanished {
            self.conn
                .prepare_cached("DELETE FROM dirs WHERE id = ?1")?
                .execute([id])?;
        }
        self.conn.execute_batch("COMMIT")?;
        // A move is one change: its new row's insert, counted when it was listed (the row
        // it was folded into is not counted again).
        if !moves.is_empty() {
            specular_log::debug!(target: specular_log::FOLDER, "library: {} files moved or renamed", moves.len());
        }
        Ok(())
    }
}

fn ident(r: &rusqlite::Row<'_>) -> rusqlite::Result<Ident> {
    Ok(Ident {
        id: r.get(0)?,
        dir_id: r.get(1)?,
        name: r.get(2)?,
        size: r.get(3)?,
        mtime: r.get(4)?,
        dev: r.get(5)?,
        ino: r.get(6)?,
        xmp: r.get(7)?,
        sides: r.get(8)?,
    })
}

/// The `.cos` files in Capture One's settings folders of `dir` (`CaptureOne/Settings153`),
/// added to `others` by their path from `dir` with their time stamps. Returns the folders,
/// newest settings version first.
fn capture_one_listing(dir: &Path, others: &mut HashMap<String, i64>) -> Vec<String> {
    let dirs = specular_metadata::sidecars::capture_one_dirs(dir);
    for d in &dirs {
        let Ok(rd) = std::fs::read_dir(dir.join(d)) else {
            continue;
        };
        for e in rd.flatten() {
            let Ok(name) = e.file_name().into_string() else {
                continue;
            };
            if specular_metadata::sidecars::Kind::of_name(&name)
                != Some(specular_metadata::sidecars::Kind::Cos)
                || !e.file_type().is_ok_and(|t| t.is_file())
            {
                continue;
            }
            let m = e.metadata().ok().and_then(|m| m.modified().ok());
            others.insert(format!("{d}/{name}"), mtime_ns(m));
        }
    }
    dirs
}
