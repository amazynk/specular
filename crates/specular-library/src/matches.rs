//! Duplicates and similar shots across the catalogue: `dupes:` and `similar:` in Library
//! search.
//!
//! The groups are the gallery's ([`specular_similar`]), found the same way over every
//! location at once and kept current as files arrive, change and leave
//! ([`specular_similar::live::Live`]): a file's candidates are looked up in a perceptual-hash
//! index and its capture-time neighbourhood when its signature is written, never by comparing
//! all pairs, and only the groups an arrival or departure touches are formed again. The
//! matcher lives on the indexer's thread. Its links and roles are kept in the catalogue
//! (`match_links`, `match_groups`, `info.match_state`): a start with nothing changed loads
//! and matches nothing; the first change loads the stored state (items and links, no
//! candidate looked up: about a second at 100k) and matches only around it. Only a catalogue
//! without a state takes in every signature. Its memory is reserved in the process's budget
//! (`specular_core::budget`); refused, it lets go and pauses.
//!
//! What a role means here, against the gallery's duplicate finder:
//!
//! * **Exact duplicates** need the same bytes: equal size and partial content hash (the
//!   thumbnail key's), then equal BLAKE3, read only for files that collide. Pixel-equal and
//!   visually equal candidates are checked with SSIM on the cached 256 px thumbnails, as the
//!   gallery checks them; a pair that cannot be checked (offline and not cached) is no
//!   duplicate.
//! * **Similar shots** are grouped at the gallery's default sensitivity
//!   ([`specular_similar::live::DEFAULT_SENSITIVITY`]): the gallery's slider is a view of one
//!   folder's scan, while the catalogue keeps one answer for every location. Capture time,
//!   camera and focal length come from the catalogue's facts (seconds, no sub-seconds, burst
//!   ids or positions), so a burst inside one second is grouped by looks alone.
//! * **Keep** is the suggestion: protected files (rated, picked, labelled, or tagged in a
//!   sidecar or in Specular) are always kept, as in the gallery. The gallery's own decisions
//!   (kept, discarded, "not duplicates") live in its per-folder store and are not applied
//!   here; the gallery's groups and roles are unchanged by any of this.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use rusqlite::{Connection, OptionalExtension, Row, params};
use specular_core::Reservation;
use specular_similar::hints::fnv_str;
use specular_similar::live::{DEFAULT_SENSITIVITY, Link, Live, RULES_ID, Role};
use specular_similar::scan::SIG_DIGEST_ID;
use specular_similar::{Category, Hints, Item, Kind, Luma, Protect, Signature, Tier};

use crate::Config;
use crate::hashes::{Signer, key_of, signature_of};

/// After the memory budget refused the matcher, this long before it tries again.
const STARVED: Duration = Duration::from_secs(60);

/// The catalogue's groups, as the indexer keeps them.
///
/// A sync is a pass over the signatures written up to a point (the newest when the pass
/// began), taken in a part at a time so that the indexer answers commands between parts
/// ([`Budget`]). The roles are written only when the pass is through, in one transaction: a
/// search sees the groups of the last finished pass, never half-formed ones. Many roles (a
/// first pass) are staged in a temporary table a part at a time first, and then go into
/// `match_groups` with two set-based statements in that one transaction.
///
/// With the roles go the links that changed (`match_links`) and the state
/// (`info.match_state`: the rules and the last signature taken in), so the next run starts
/// where this one ended: with nothing changed it loads nothing and matches nothing; with
/// changes it loads the stored links and items a part at a time ([`Live::restore`], no
/// candidate looked up) and forms again only the groups around them. The first pass on a
/// catalogue without a state (or with other rules) builds it from every signature.
pub(crate) struct Matches {
    /// Not loaded while nothing changed since the stored state.
    live: Option<Live>,
    /// The newest `image_hashes.seq` taken in.
    seq: i64,
    /// The pass under way: signatures up to this `seq` are taken in before roles are written.
    target: Option<i64>,
    /// `match_groups` and `match_links` agree with what is taken in up to `seq` (a stored
    /// state was found, or a pass is done).
    loaded: bool,
    /// The `seq` of the stored state.
    saved_seq: Option<i64>,
    /// A first pass (no stored state): `match_groups` as found, to drop what it no longer
    /// holds; (file, category) → (group, keep).
    stored: Option<HashMap<(i64, u8), (i64, bool)>>,
    /// Roles that moved since they were last written.
    moved: HashSet<(i64, u8)>,
    /// Stored roles of files the links were loaded without: dropped with the next write.
    orphans: Vec<(i64, u8)>,
    protect: HashMap<i64, Protect>,
    /// The marks counter (`temp.match_marks`) when the protections were last read.
    marks_seen: Option<i64>,
    /// Times the protections were read (an idle sync reads none).
    pub protect_reads: usize,
    /// Files whose protection was read (all of them only on the first read).
    pub protect_rows: usize,
    /// The roles of a finished pass on their way into `temp.match_stage`.
    stage: Option<Stage>,
    /// Parts spent staging roles, and how long the last swap into `match_groups` took.
    pub staged_parts: usize,
    pub last_swap: Option<Duration>,
    /// What the swap's part spent after its commit (the stage cleared).
    pub last_settle: Option<Duration>,
    /// Files may have changed or left (walks, removed locations): check every signature
    /// taken in is still the file's.
    pub check_gone: bool,
    /// Loading the stored state, a part at a time.
    restore: Option<Step>,
    /// Times the stored state was loaded.
    pub restores: usize,
    /// Matching operations done ([`Live::work`]), across loads.
    pub work: u64,
    /// The matcher's memory, reserved in the process's budget.
    mem: Option<Reservation>,
    /// The budget refused: nothing is matched until then.
    starved_until: Option<Instant>,
}

/// Where loading the stored state is: items after a file id, links after a file id, roles
/// after a (file, category).
#[derive(Clone, Copy, Debug)]
enum Step {
    Items(i64),
    Links(i64),
    Roles(i64, i64),
}

/// How much one [`Matches::sync_some`] may do before it returns to the indexer's loop.
pub(crate) struct Budget<'a> {
    /// No part is started after this long.
    pub time: Duration,
    /// Signatures taken in per part (one read and one regrouping).
    pub rows: usize,
    /// No part is started once this says so (a command is waiting).
    pub yield_now: &'a dyn Fn() -> bool,
}

#[cfg(test)]
impl Budget<'_> {
    /// No limit: a whole pass at once.
    pub const ALL: Budget<'static> = Budget {
        time: Duration::MAX,
        rows: 2048,
        yield_now: &|| false,
    };
}

impl Budget<'_> {
    /// Roles written to `match_groups` row by row, or staged per part: more than this many
    /// are staged (a row is a few microseconds; a signature taken in is far more).
    fn stage_rows(&self) -> usize {
        self.rows.max(1).saturating_mul(16)
    }
}

/// A role to write: (file, category, Some(group, keep) or None to drop it).
type RoleRow = (i64, u8, Option<(i64, bool)>);

/// Roles staged so far, and what goes with them when they are swapped in.
struct Stage {
    rows: Vec<RoleRow>,
    /// How many of `rows` are in `temp.match_stage`.
    at: usize,
    /// Files whose stored links are written again.
    owners: Vec<i64>,
    /// `match_links` is written from nothing (a first pass).
    fresh: bool,
    /// The pass's last signature.
    target: i64,
}

/// What one [`Matches::sync_some`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Synced {
    /// `match_groups` rows changed (only when a pass ends).
    pub written: usize,
    /// The pass is through (else call again).
    pub done: bool,
}

const DONE: Synced = Synced {
    written: 0,
    done: true,
};
const MORE: Synced = Synced {
    written: 0,
    done: false,
};

/// What the catalogue's writers tell the matcher, through temporary triggers on the
/// indexer's connection (they see every write made on that connection): a counter bumped
/// whenever a mark that protects a file may have changed (rating, pick, label, a hand-made
/// tag, a protected file arriving or leaving) with the files it may have changed for, and
/// the files whose name changed (the "copy of" rule reads it). The same files go into the
/// catalogue's logs (`match_mark_log`, `match_rename_log`), cleared only with the roles
/// they changed, so a restart in between forms their groups again. `match_stage` holds the
/// roles of a finished pass on their way into `match_groups`.
const WATCH: &str = "
CREATE TEMP TABLE IF NOT EXISTS match_marks (n INTEGER NOT NULL);
INSERT INTO temp.match_marks (n) SELECT 0 WHERE NOT EXISTS (SELECT 1 FROM temp.match_marks);
CREATE TEMP TABLE IF NOT EXISTS match_marked (id INTEGER PRIMARY KEY);
CREATE TEMP TABLE IF NOT EXISTS match_renamed (id INTEGER PRIMARY KEY);
CREATE TEMP TABLE IF NOT EXISTS match_stage (
  file_id INTEGER NOT NULL, cat INTEGER NOT NULL, grp INTEGER, keep INTEGER,
  PRIMARY KEY (file_id, cat)) WITHOUT ROWID;
CREATE TEMP TRIGGER IF NOT EXISTS match_marks_upd AFTER UPDATE OF rating, flag, label ON files
  WHEN (old.rating, old.flag, old.label) IS NOT (new.rating, new.flag, new.label)
  BEGIN UPDATE match_marks SET n = n + 1;
    INSERT OR IGNORE INTO match_marked (id) VALUES (new.id);
    INSERT OR IGNORE INTO match_mark_log (id) VALUES (new.id); END;
CREATE TEMP TRIGGER IF NOT EXISTS match_marks_ins AFTER INSERT ON files
  WHEN new.rating > 0 OR new.flag = 1 OR new.label > 0
  BEGIN UPDATE match_marks SET n = n + 1;
    INSERT OR IGNORE INTO match_marked (id) VALUES (new.id);
    INSERT OR IGNORE INTO match_mark_log (id) VALUES (new.id); END;
CREATE TEMP TRIGGER IF NOT EXISTS match_marks_del AFTER DELETE ON files
  WHEN old.rating > 0 OR old.flag = 1 OR old.label > 0
  BEGIN UPDATE match_marks SET n = n + 1;
    INSERT OR IGNORE INTO match_marked (id) VALUES (old.id);
    INSERT OR IGNORE INTO match_mark_log (id) VALUES (old.id); END;
CREATE TEMP TRIGGER IF NOT EXISTS match_tags_ins AFTER INSERT ON file_tags
  WHEN new.source >= 1
  BEGIN UPDATE match_marks SET n = n + 1;
    INSERT OR IGNORE INTO match_marked (id) VALUES (new.file_id);
    INSERT OR IGNORE INTO match_mark_log (id) VALUES (new.file_id); END;
CREATE TEMP TRIGGER IF NOT EXISTS match_tags_del AFTER DELETE ON file_tags
  WHEN old.source >= 1
  BEGIN UPDATE match_marks SET n = n + 1;
    INSERT OR IGNORE INTO match_marked (id) VALUES (old.file_id);
    INSERT OR IGNORE INTO match_mark_log (id) VALUES (old.file_id); END;
CREATE TEMP TRIGGER IF NOT EXISTS match_tags_upd AFTER UPDATE ON file_tags
  WHEN old.source >= 1 OR new.source >= 1
  BEGIN UPDATE match_marks SET n = n + 1;
    INSERT OR IGNORE INTO match_marked (id) VALUES (old.file_id);
    INSERT OR IGNORE INTO match_marked (id) VALUES (new.file_id);
    INSERT OR IGNORE INTO match_mark_log (id) VALUES (old.file_id);
    INSERT OR IGNORE INTO match_mark_log (id) VALUES (new.file_id); END;
CREATE TEMP TRIGGER IF NOT EXISTS match_renamed_upd AFTER UPDATE OF name ON files
  WHEN old.name IS NOT new.name
  BEGIN INSERT OR IGNORE INTO match_renamed (id) VALUES (new.id);
    INSERT OR IGNORE INTO match_rename_log (id) VALUES (new.id); END;
";

/// One duplicate check, written with the groups.
struct Verified {
    a: i64,
    b: i64,
    tier: Tier,
    score: f32,
}

fn tier_id(t: Tier) -> i64 {
    match t {
        Tier::Bytes => 0,
        Tier::Pixels => 1,
        Tier::Visual => 2,
    }
}

fn cat_id(c: Category) -> u8 {
    match c {
        Category::Duplicates => 0,
        Category::Similar => 1,
    }
}

/// Where a file is and how its thumbnail is keyed.
struct Place {
    path: PathBuf,
    size: i64,
    mtime_ns: i64,
    content: Option<i64>,
    blake3: Option<[u8; 32]>,
}

/// The `files` columns an [`Item`] is made of ([`item_of`] reads them in this order).
macro_rules! item_cols {
    () => {
        "f.name, f.ext, f.size, f.mtime_ns, f.width, f.height, f.bits, f.taken, f.camera, f.focal"
    };
}

/// An item from the columns of [`item_cols`] starting at `at`.
fn item_of(
    r: &Row<'_>,
    at: usize,
    sig: Signature,
    protect: Option<Protect>,
) -> rusqlite::Result<Item> {
    let ext: String = r.get(at + 1)?;
    let size = r.get::<_, i64>(at + 2)?.max(0) as u64;
    let taken: Option<i64> = r.get(at + 7)?;
    let camera: Option<String> = r.get(at + 8)?;
    Ok(Item {
        name: r.get(at)?,
        kind: Kind::from_ext(&ext),
        width: r.get::<_, Option<u32>>(at + 4)?.unwrap_or(0),
        height: r.get::<_, Option<u32>>(at + 5)?.unwrap_or(0),
        bit_depth: r.get::<_, Option<u8>>(at + 6)?.unwrap_or(0),
        bytes: size,
        len: size,
        modified: r.get::<_, i64>(at + 3)?.div_euclid(1_000_000_000),
        sig,
        hints: Hints {
            time_ms: taken.map(|t| t.saturating_mul(1000)),
            camera: camera.as_deref().map(fnv_str),
            focal_mm: r.get::<_, Option<f64>>(at + 9)?.map(|f| f as f32),
            has_exif: taken.is_some() || camera.is_some(),
            ..Hints::default()
        },
        protect,
        ..Item::default()
    })
}

/// The signature in column `at` when it is of this build's version (column `at + 1`).
fn sig_at(r: &Row<'_>, at: usize) -> rusqlite::Result<Option<Signature>> {
    if r.get::<_, i64>(at + 1)? != i64::from(SIG_DIGEST_ID) {
        return Ok(None);
    }
    Ok(r.get_ref(at)?.as_blob_or_null()?.and_then(signature_of))
}

/// Full items for the groups formed again, one point lookup each.
fn fetch(c: &Connection, keys: &[i64]) -> Vec<Option<Item>> {
    let Ok(mut st) = c.prepare_cached(concat!(
        "SELECT h.sig, h.version, ",
        item_cols!(),
        " FROM image_hashes h JOIN files f ON f.id = h.file_id WHERE h.file_id = ?1"
    )) else {
        return vec![None; keys.len()];
    };
    keys.iter()
        .map(|&k| {
            st.query_row([k], |r| match sig_at(r, 0)? {
                Some(sig) => item_of(r, 2, sig, None).map(Some),
                None => Ok(None),
            })
            .optional()
            .ok()
            .flatten()
            .flatten()
        })
        .collect()
}

/// A file's links to larger file ids as stored: varints of (step << 2 | kind).
fn encode_links(key: i64, links: &[(i64, Link)]) -> Vec<u8> {
    let mut out = Vec::with_capacity(links.len() * 3);
    let mut prev = key;
    for &(o, l) in links {
        let mut v = ((o - prev).max(0) as u64) << 2 | u64::from(l.id());
        prev = o;
        loop {
            let b = (v & 0x7F) as u8;
            v >>= 7;
            if v == 0 {
                out.push(b);
                break;
            }
            out.push(b | 0x80);
        }
    }
    out
}

/// The links [`encode_links`] wrote (as far as they read: a damaged blob ends early).
fn decode_links(key: i64, b: &[u8]) -> Vec<(i64, Link)> {
    let mut out = Vec::new();
    let mut prev = key;
    let (mut v, mut shift) = (0u64, 0u32);
    for &x in b {
        if shift > 56 {
            break;
        }
        v |= u64::from(x & 0x7F) << shift;
        shift += 7;
        if x & 0x80 != 0 {
            continue;
        }
        let Some(link) = Link::from_id((v & 3) as u8) else {
            break;
        };
        prev = prev.saturating_add((v >> 2) as i64);
        out.push((prev, link));
        (v, shift) = (0, 0);
    }
    out
}

/// What `info.match_state` says for these rules: the last signature its links agree with.
fn state_id(seq: i64) -> String {
    format!("{RULES_ID} s={DEFAULT_SENSITIVITY} seq={seq}")
}

fn read_state(c: &Connection) -> rusqlite::Result<Option<i64>> {
    let v = crate::store::info_get(c, "match_state")?;
    let prefix = format!("{RULES_ID} s={DEFAULT_SENSITIVITY} seq=");
    Ok(v.and_then(|v| v.strip_prefix(&prefix)?.parse().ok()))
}

/// Writes the stored links of `owners` again (all of them from nothing when `fresh`), the
/// state, and clears the logs it took in (inside the caller's transaction).
fn write_state(
    c: &Connection,
    live: &Live,
    owners: &[i64],
    fresh: bool,
    seq: i64,
) -> rusqlite::Result<()> {
    if fresh {
        c.execute("DELETE FROM match_links", [])?;
    }
    let mut put =
        c.prepare_cached("INSERT OR REPLACE INTO match_links (file_id, links) VALUES (?1, ?2)")?;
    let mut drop = c.prepare_cached("DELETE FROM match_links WHERE file_id = ?1")?;
    for &k in owners {
        let v = live.links_of(k);
        if v.is_empty() {
            if !fresh {
                drop.execute([k])?;
            }
        } else {
            put.execute(params![k, encode_links(k, &v)])?;
        }
    }
    crate::store::info_set(c, "match_state", &state_id(seq))?;
    c.prepare_cached("DELETE FROM match_mark_log")?
        .execute([])?;
    c.prepare_cached("DELETE FROM match_rename_log")?
        .execute([])?;
    Ok(())
}

fn logs_waiting(c: &Connection) -> rusqlite::Result<bool> {
    c.prepare_cached(
        "SELECT EXISTS (SELECT 1 FROM match_mark_log) OR EXISTS (SELECT 1 FROM match_rename_log)",
    )?
    .query_row([], |r| r.get(0))
}

impl Matches {
    pub fn new() -> Matches {
        Matches {
            live: None,
            seq: 0,
            target: None,
            loaded: false,
            saved_seq: None,
            stored: None,
            moved: HashSet::new(),
            orphans: Vec::new(),
            protect: HashMap::new(),
            marks_seen: None,
            protect_reads: 0,
            protect_rows: 0,
            stage: None,
            staged_parts: 0,
            last_swap: None,
            last_settle: None,
            check_gone: false,
            restore: None,
            restores: 0,
            work: 0,
            mem: None,
            starved_until: None,
        }
    }

    /// A pass is under way: its roles are not written yet.
    pub fn part_way(&self) -> bool {
        self.target.is_some()
    }

    /// The matcher's groups are in memory (not while nothing changed since the stored state).
    #[cfg(test)]
    pub fn resident(&self) -> bool {
        self.live.is_some()
    }

    /// A whole pass at once: takes in what changed since the last pass and writes the roles
    /// that moved. Returns how many `match_groups` rows changed.
    #[cfg(test)]
    pub fn sync(
        &mut self,
        c: &Connection,
        cfg: &Config,
        signer: &dyn Signer,
    ) -> rusqlite::Result<usize> {
        let mut n = 0;
        loop {
            let s = self.sync_some(c, cfg, signer, &Budget::ALL)?;
            n += s.written;
            if s.done {
                return Ok(n);
            }
        }
    }

    /// Keeps the matcher's memory reserved in the process's budget. When the budget refuses,
    /// the matcher lets go of everything in memory (the stored state stays) and waits.
    fn reserve(&mut self) -> bool {
        let Some(live) = &self.live else {
            return true;
        };
        let want = live.approx_bytes();
        let r = match self.mem.as_mut() {
            Some(m) => m.grow_to(want),
            None => specular_core::budget::reserve(want).map(|m| {
                self.mem = Some(m);
            }),
        };
        match r {
            Ok(()) => true,
            Err(e) => {
                specular_log::warn!(
                    target: specular_log::FOLDER,
                    "library: duplicates and similar shots paused: matching {} files {e}",
                    live.len()
                );
                let (reads, rows) = (self.protect_reads, self.protect_rows);
                *self = Matches::new();
                (self.protect_reads, self.protect_rows) = (reads, rows);
                self.starved_until = Some(Instant::now() + STARVED);
                false
            }
        }
    }

    /// Goes on with the pass under way (or begins one) for at most about `budget`, and
    /// writes the roles when the pass is through.
    pub fn sync_some(
        &mut self,
        c: &Connection,
        cfg: &Config,
        signer: &dyn Signer,
        budget: &Budget,
    ) -> rusqlite::Result<Synced> {
        let start = Instant::now();
        if self.stage.is_some() {
            return self.stage_some(c, budget, start, true);
        }
        if self.target.is_none() {
            if self.starved_until.is_some_and(|t| Instant::now() < t) {
                return Ok(DONE);
            }
            self.starved_until = None;
            // Every pass, as a reopened connection or a reset loses them (cheap: all
            // IF NOT EXISTS). Marks made while they were missing were not logged: read
            // them all.
            let watched: bool = c
                .prepare_cached(
                    "SELECT EXISTS (SELECT 1 FROM sqlite_temp_master
                                    WHERE type = 'table' AND name = 'match_marked')",
                )?
                .query_row([], |r| r.get(0))?;
            if !watched {
                self.marks_seen = None;
            }
            c.execute_batch(WATCH)?;
            if !self.loaded && self.live.is_none() {
                self.saved_seq = read_state(c)?;
                match self.saved_seq {
                    Some(s) => {
                        self.loaded = true;
                        self.seq = s;
                    }
                    None => {
                        // No state: everything is taken in, and rows written before that
                        // the groups no longer hold are dropped.
                        let mut stored = HashMap::new();
                        let mut st =
                            c.prepare("SELECT file_id, cat, grp, keep FROM match_groups")?;
                        let mut q = st.query([])?;
                        while let Some(r) = q.next()? {
                            stored.insert(
                                (r.get(0)?, r.get(1)?),
                                (r.get(2)?, r.get::<_, i64>(3)? != 0),
                            );
                        }
                        self.stored = Some(stored);
                        self.live = Some(Live::new(DEFAULT_SENSITIVITY));
                        self.seq = 0;
                    }
                }
            }
            let t: i64 =
                c.query_row("SELECT COALESCE(MAX(seq), 0) FROM image_hashes", [], |r| {
                    r.get(0)
                })?;
            if self.live.is_none() {
                // Nothing new since the stored state: nothing to load or match.
                if t <= self.seq && !self.check_gone && !logs_waiting(c)? {
                    return Ok(DONE);
                }
                self.begin_restore(c)?;
            }
            self.target = Some(t);
        }
        if self.restore.is_some() {
            self.restore_some(c, budget, start)?;
            return Ok(if self.live.is_none() { DONE } else { MORE });
        }
        let Some(target) = self.target else {
            unreachable!("a pass without a target");
        };
        self.follow_marks(c)?;
        self.follow_renames(c)?;
        if self.check_gone {
            self.check_gone = false;
            let mut valid: HashSet<i64> = HashSet::new();
            let mut st = c.prepare_cached(
                "SELECT h.file_id FROM image_hashes h JOIN files f ON f.id = h.file_id
                 WHERE h.sig IS NOT NULL AND h.version = ?1
                   AND h.size = f.size AND h.mtime_ns = f.mtime_ns",
            )?;
            let mut q = st.query([i64::from(SIG_DIGEST_ID)])?;
            while let Some(r) = q.next()? {
                valid.insert(r.get(0)?);
            }
            if let Some(live) = self.live.as_mut() {
                let gone: Vec<i64> = live.keys().filter(|k| !valid.contains(k)).collect();
                for k in gone {
                    live.remove(k);
                }
            }
        }

        let mut checks = Checks {
            c,
            cfg,
            signer,
            lumas: HashMap::new(),
            places: HashMap::new(),
            verified: Vec::new(),
            hashed: Vec::new(),
        };
        let rows = budget.rows.max(1);
        let work0 = self.live.as_ref().map_or(0, Live::work);
        // The groups are formed once the pass has taken everything in, not after each part:
        // they are written only then, and a large group touched by every part (thousands of
        // copies of one image) was formed again each time.
        let done = loop {
            let n = self.take_in(c, target, rows)?;
            let Some(live) = self.live.as_mut() else {
                unreachable!("a pass without its groups");
            };
            live.apply(&mut |a, b, t| checks.check(a, b, t));
            if n < rows {
                let changes = live.regroup(&mut |keys: &[i64]| fetch(c, keys));
                self.moved
                    .extend(changes.iter().map(|ch| (ch.key, cat_id(ch.cat))));
                if !self.reserve() {
                    return Ok(DONE);
                }
                break true;
            }
            if !self.reserve() {
                return Ok(DONE);
            }
            if start.elapsed() >= budget.time || (budget.yield_now)() {
                break false;
            }
        };
        let Checks {
            verified, hashed, ..
        } = checks;
        let Some(live) = self.live.as_mut() else {
            unreachable!("a pass without its groups");
        };
        crate::work_counts::match_ops(live.work() - work0);
        self.work += live.work() - work0;
        let roles = if done {
            self.changed_roles()
        } else {
            Vec::new()
        };
        // Files whose stored links changed: taken only once their transaction has begun (a
        // busy catalogue leaves them for the next try; a failed write starts over from the
        // stored state).
        let owners_waiting = done && self.live.as_ref().is_some_and(Live::link_owners_waiting);
        let fresh = self.stored.is_some();
        // The state is written when the pass is through and anything it stands for moved.
        let state = done
            && (fresh
                || self.saved_seq != Some(target)
                || owners_waiting
                || !roles.is_empty()
                || logs_waiting(c)?);
        // Many roles are staged a part at a time and swapped in at the end.
        let staged = roles.len() > budget.stage_rows();
        let direct = !roles.is_empty() && !staged;
        let mut written = 0;
        if !verified.is_empty() || !hashed.is_empty() || direct || (state && !staged) {
            c.execute_batch("BEGIN IMMEDIATE")?;
            let r = Self::write(c, &verified, &hashed, if direct { &roles } else { &[] }).and_then(
                |n| {
                    if state && !staged {
                        let live = self.live.as_mut().expect("a pass with its groups");
                        let owners = live.take_link_owners();
                        write_state(c, live, &owners, fresh, target)?;
                    }
                    Ok(n)
                },
            );
            match r {
                Ok(n) => {
                    if let Err(e) = c.execute_batch("COMMIT") {
                        let _ = c.execute_batch("ROLLBACK");
                        *self = Matches::new();
                        return Err(e);
                    }
                    written = n;
                }
                Err(e) => {
                    let _ = c.execute_batch("ROLLBACK");
                    // Read everything again next time.
                    *self = Matches::new();
                    return Err(e);
                }
            }
        }
        if staged {
            c.prepare_cached("DELETE FROM temp.match_stage")?
                .execute([])?;
            let owners = self
                .live
                .as_mut()
                .map(Live::take_link_owners)
                .unwrap_or_default();
            self.stage = Some(Stage {
                rows: roles,
                at: 0,
                owners,
                fresh,
                target,
            });
            return self.stage_some(c, budget, start, false);
        }
        if done {
            self.finish(target);
        }
        Ok(Synced { written, done })
    }

    /// The pass is through and its roles, links and state are in the catalogue.
    fn finish(&mut self, target: i64) {
        self.loaded = true;
        self.target = None;
        self.saved_seq = Some(target);
        self.stored = None;
        self.moved.clear();
        self.orphans.clear();
    }

    /// Starts loading the stored state: the protections first (all of them, once).
    fn begin_restore(&mut self, c: &Connection) -> rusqlite::Result<()> {
        let marks: i64 = c
            .prepare_cached("SELECT n FROM temp.match_marks")?
            .query_row([], |r| r.get(0))?;
        c.prepare_cached("DELETE FROM temp.match_marked")?
            .execute([])?;
        c.prepare_cached("DELETE FROM temp.match_renamed")?
            .execute([])?;
        self.protect = protections(c)?;
        self.protect_reads += 1;
        self.protect_rows += self.protect.len();
        self.marks_seen = Some(marks);
        self.live = Some(Live::new(DEFAULT_SENSITIVITY));
        self.restore = Some(Step::Items(i64::MIN));
        self.restores += 1;
        Ok(())
    }

    /// Loads the stored state a part at a time: every item with a signature (no candidate
    /// looked up), then the links, then the roles; then the files logged since are formed
    /// again and those whose signature no longer holds are taken out. Nothing is matched.
    fn restore_some(
        &mut self,
        c: &Connection,
        budget: &Budget,
        start: Instant,
    ) -> rusqlite::Result<()> {
        let rows = budget.rows.max(1).saturating_mul(16) as i64;
        let mut stale: Vec<i64> = Vec::new();
        while let Some(step) = self.restore {
            let Some(live) = self.live.as_mut() else {
                self.restore = None;
                return Ok(());
            };
            let mut n = 0;
            let next = match step {
                Step::Items(after) => {
                    let mut st = c.prepare_cached(concat!(
                        "SELECT h.file_id, h.sig, h.version,
                                h.size = f.size AND h.mtime_ns = f.mtime_ns, ",
                        item_cols!(),
                        ", th.content
                         FROM image_hashes h JOIN files f ON f.id = h.file_id
                         LEFT JOIN thumbs th ON th.file_id = f.id
                         WHERE h.file_id > ?1 ORDER BY h.file_id LIMIT ?2"
                    ))?;
                    let mut q = st.query(params![after, rows])?;
                    let mut last = after;
                    while let Some(r) = q.next()? {
                        n += 1;
                        let id: i64 = r.get(0)?;
                        last = id;
                        let Some(sig) = sig_at(r, 1)? else {
                            continue;
                        };
                        let item = item_of(r, 4, sig, self.protect.get(&id).copied())?;
                        let quick = r.get::<_, Option<i64>>(14)?.map(|q| q as u64);
                        live.restore(id, &item, quick);
                        if !r.get::<_, bool>(3)? {
                            stale.push(id);
                        }
                    }
                    if n < rows {
                        Step::Links(i64::MIN)
                    } else {
                        Step::Items(last)
                    }
                }
                Step::Links(after) => {
                    let mut st = c.prepare_cached(
                        "SELECT file_id, links FROM match_links WHERE file_id > ?1
                         ORDER BY file_id LIMIT ?2",
                    )?;
                    let mut q = st.query(params![after, rows])?;
                    let mut last = after;
                    while let Some(r) = q.next()? {
                        n += 1;
                        let a: i64 = r.get(0)?;
                        last = a;
                        let blob = r.get_ref(1)?.as_blob_or_null()?.unwrap_or_default();
                        for (b, link) in decode_links(a, blob) {
                            live.restore_link(a, b, link);
                        }
                    }
                    if n < rows {
                        Step::Roles(i64::MIN, 0)
                    } else {
                        Step::Links(last)
                    }
                }
                Step::Roles(after, after_cat) => {
                    let mut st = c.prepare_cached(
                        "SELECT file_id, cat, grp, keep FROM match_groups
                         WHERE (file_id, cat) > (?1, ?2) ORDER BY file_id, cat LIMIT ?3",
                    )?;
                    let mut q = st.query(params![after, after_cat, rows])?;
                    let mut last = (after, after_cat);
                    while let Some(r) = q.next()? {
                        n += 1;
                        let (k, cat): (i64, i64) = (r.get(0)?, r.get(1)?);
                        last = (k, cat);
                        let category = if cat == 0 {
                            Category::Duplicates
                        } else {
                            Category::Similar
                        };
                        if live.contains(k) {
                            let role = Role {
                                group: r.get(2)?,
                                keep: r.get::<_, i64>(3)? != 0,
                            };
                            live.restore_role(k, category, Some(role));
                        } else {
                            self.orphans.push((k, cat_id(category)));
                        }
                    }
                    if n < rows {
                        // The files logged since the state was written: formed again.
                        let mut st = c.prepare_cached(
                            "SELECT id FROM match_mark_log UNION SELECT id FROM match_rename_log",
                        )?;
                        let mut q = st.query([])?;
                        while let Some(r) = q.next()? {
                            live.touch(r.get(0)?);
                        }
                        self.restore = None;
                        break;
                    }
                    Step::Roles(last.0, last.1)
                }
            };
            self.restore = Some(next);
            if !self.reserve() {
                return Ok(());
            }
            if start.elapsed() >= budget.time || (budget.yield_now)() {
                break;
            }
        }
        // Signatures that no longer hold (the file changed since): taken out, as a pass
        // that reads them does.
        if let Some(live) = self.live.as_mut() {
            for k in stale {
                live.remove(k);
            }
        }
        Ok(())
    }

    /// Stages roles a part at a time (only the temporary database is written: no lock on the
    /// catalogue), then swaps them all into `match_groups` in one transaction, with the
    /// pass's links and state. `fresh`: no part was done yet in this call.
    fn stage_some(
        &mut self,
        c: &Connection,
        budget: &Budget,
        start: Instant,
        mut fresh: bool,
    ) -> rusqlite::Result<Synced> {
        let chunk = budget.stage_rows();
        loop {
            if !fresh && (start.elapsed() >= budget.time || (budget.yield_now)()) {
                return Ok(MORE);
            }
            fresh = false;
            let Some(st) = self.stage.as_mut() else {
                unreachable!("staging without a stage");
            };
            if st.at < st.rows.len() {
                let end = (st.at + chunk).min(st.rows.len());
                let r = stage_rows(c, &st.rows[st.at..end]);
                if let Err(e) = r {
                    *self = Matches::new();
                    return Err(e);
                }
                st.at = end;
                self.staged_parts += 1;
                continue;
            }
            let t = Instant::now();
            let n = {
                let live = self.live.as_ref().expect("a pass with its groups");
                let extra = || write_state(c, live, &st.owners, st.fresh, st.target);
                match swap_staged(c, &extra) {
                    Ok(n) => n,
                    Err(e) => {
                        if !is_busy(&e) {
                            *self = Matches::new();
                        }
                        return Err(e);
                    }
                }
            };
            self.last_swap = Some(t.elapsed());
            let t = Instant::now();
            let Some(st) = self.stage.take() else {
                unreachable!("staging without a stage");
            };
            c.prepare_cached("DELETE FROM temp.match_stage")?
                .execute([])?;
            self.finish(st.target);
            self.last_settle = Some(t.elapsed());
            return Ok(Synced {
                written: n,
                done: true,
            });
        }
    }

    /// Takes in up to `limit` signatures written after the last taken in and up to `target`.
    /// Returns how many rows were read. (`seq` is unique: the indexer numbers the rows it
    /// writes one by one.)
    fn take_in(&mut self, c: &Connection, target: i64, limit: usize) -> rusqlite::Result<usize> {
        let Some(live) = self.live.as_mut() else {
            return Ok(0);
        };
        let mut st = c.prepare_cached(concat!(
            "SELECT h.file_id, h.seq, h.sig, h.version, h.size = f.size AND h.mtime_ns = f.mtime_ns, ",
            item_cols!(),
            ", th.content
             FROM image_hashes h JOIN files f ON f.id = h.file_id
             LEFT JOIN thumbs th ON th.file_id = f.id
             WHERE h.seq > ?1 AND h.seq <= ?2 ORDER BY h.seq LIMIT ?3"
        ))?;
        let mut q = st.query(params![self.seq, target, limit as i64])?;
        let mut n = 0;
        while let Some(r) = q.next()? {
            n += 1;
            let id: i64 = r.get(0)?;
            self.seq = self.seq.max(r.get(1)?);
            let sig = sig_at(r, 2)?.filter(|_| r.get::<_, bool>(4).unwrap_or(false));
            let Some(sig) = sig else {
                if live.contains(id) {
                    live.remove(id);
                }
                continue;
            };
            let item = item_of(r, 5, sig, self.protect.get(&id).copied())?;
            let quick = r.get::<_, Option<i64>>(15)?.map(|q| q as u64);
            live.put(id, item, quick);
        }
        Ok(n)
    }

    /// Reads the protections again when a mark may have changed since they were read.
    fn follow_marks(&mut self, c: &Connection) -> rusqlite::Result<()> {
        let marks: i64 = c
            .prepare_cached("SELECT n FROM temp.match_marks")?
            .query_row([], |r| r.get(0))?;
        if self.marks_seen == Some(marks) {
            return Ok(());
        }
        let Some(live) = self.live.as_mut() else {
            return Ok(());
        };
        self.protect_reads += 1;
        if self.marks_seen.is_none() {
            // Everything, once: the log starts now.
            c.prepare_cached("DELETE FROM temp.match_marked")?
                .execute([])?;
            let protect = protections(c)?;
            self.protect_rows += protect.len();
            for (&k, &p) in &protect {
                if self.protect.get(&k) != Some(&p) {
                    live.set_protect(k, Some(p));
                }
            }
            for k in self.protect.keys() {
                if !protect.contains_key(k) {
                    live.set_protect(*k, None);
                }
            }
            self.protect = protect;
        } else {
            // Only the files the triggers logged.
            let mut st = c.prepare_cached(
                "SELECT m.id, f.rating, f.flag, f.label,
                        EXISTS (SELECT 1 FROM file_tags t WHERE t.file_id = m.id AND t.source >= 1)
                 FROM temp.match_marked m LEFT JOIN files f ON f.id = m.id",
            )?;
            let mut q = st.query([])?;
            while let Some(r) = q.next()? {
                self.protect_rows += 1;
                let id: i64 = r.get(0)?;
                let p = protection(
                    r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    r.get(4)?,
                );
                if self.protect.get(&id).copied() != p {
                    live.set_protect(id, p);
                    match p {
                        Some(p) => self.protect.insert(id, p),
                        None => self.protect.remove(&id),
                    };
                }
            }
            drop(q);
            c.prepare_cached("DELETE FROM temp.match_marked")?
                .execute([])?;
        }
        self.marks_seen = Some(marks);
        Ok(())
    }

    /// Forms the groups of renamed files again with their new names (the signature still
    /// holds: same bytes).
    fn follow_renames(&mut self, c: &Connection) -> rusqlite::Result<()> {
        let Some(live) = self.live.as_mut() else {
            return Ok(());
        };
        let mut st = c.prepare_cached("SELECT id FROM temp.match_renamed")?;
        let mut q = st.query([])?;
        let mut any = false;
        while let Some(r) = q.next()? {
            any = true;
            live.touch(r.get(0)?);
        }
        drop(q);
        if any {
            c.prepare_cached("DELETE FROM temp.match_renamed")?
                .execute([])?;
        }
        Ok(())
    }

    /// The roles to write, in key order: those that moved, and on a first pass those the
    /// groups no longer hold, minus what `match_groups` holds already.
    fn changed_roles(&self) -> Vec<RoleRow> {
        let Some(live) = self.live.as_ref() else {
            return Vec::new();
        };
        let mut want: Vec<(i64, u8)> = self.moved.iter().copied().collect();
        want.extend(self.orphans.iter().copied());
        if let Some(stored) = &self.stored {
            // Rows written before this run that the groups no longer hold.
            for &(k, cat) in stored.keys() {
                if live.roles(k)[cat as usize].is_none() {
                    want.push((k, cat));
                }
            }
        }
        want.sort_unstable();
        want.dedup();
        want.into_iter()
            .filter_map(|(k, cat)| {
                let now = live.roles(k)[cat as usize].map(|r| (r.group, r.keep));
                match &self.stored {
                    Some(stored) => {
                        (stored.get(&(k, cat)).copied() != now).then_some((k, cat, now))
                    }
                    None => Some((k, cat, now)),
                }
            })
            .collect()
    }

    /// Writes the checks and BLAKE3 values, and `roles` row by row. Returns how many
    /// `match_groups` rows changed.
    fn write(
        c: &Connection,
        verified: &[Verified],
        hashed: &[(i64, [u8; 32])],
        roles: &[RoleRow],
    ) -> rusqlite::Result<usize> {
        for v in verified {
            c.prepare_cached(
                "INSERT OR REPLACE INTO match_verify (a, b, tier, score)
                 SELECT ?1, ?2, ?3, ?4 WHERE EXISTS (SELECT 1 FROM files WHERE id = ?1)
                   AND EXISTS (SELECT 1 FROM files WHERE id = ?2)",
            )?
            .execute(params![v.a, v.b, tier_id(v.tier), f64::from(v.score)])?;
        }
        for (id, h) in hashed {
            c.prepare_cached("UPDATE image_hashes SET blake3 = ?2 WHERE file_id = ?1")?
                .execute(params![id, &h[..]])?;
        }
        let mut n = 0;
        for &(k, cat, now) in roles {
            match now {
                Some((g, keep)) => {
                    // A file that left meanwhile has no row to point at.
                    n += c
                        .prepare_cached(
                            "INSERT INTO match_groups (file_id, cat, grp, keep)
                             SELECT ?1, ?2, ?3, ?4 WHERE EXISTS (SELECT 1 FROM files WHERE id = ?1)
                             ON CONFLICT(file_id, cat) DO UPDATE SET grp = excluded.grp, keep = excluded.keep",
                        )?
                        .execute(params![k, cat, g, keep])?;
                }
                None => {
                    n += c
                        .prepare_cached("DELETE FROM match_groups WHERE file_id = ?1 AND cat = ?2")?
                        .execute(params![k, cat])?;
                }
            }
        }
        Ok(n)
    }
}

/// Puts `rows` in `temp.match_stage`, in one transaction of the temporary database alone.
fn stage_rows(c: &Connection, rows: &[RoleRow]) -> rusqlite::Result<()> {
    c.execute_batch("BEGIN")?;
    let r = (|| {
        let mut st = c.prepare_cached(
            "INSERT INTO temp.match_stage (file_id, cat, grp, keep) VALUES (?1, ?2, ?3, ?4)",
        )?;
        for &(k, cat, now) in rows {
            st.execute(params![k, cat, now.map(|r| r.0), now.map(|r| r.1)])?;
        }
        Ok(())
    })();
    match r {
        Ok(()) => c.execute_batch("COMMIT"),
        Err(e) => {
            let _ = c.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

fn is_busy(e: &rusqlite::Error) -> bool {
    matches!(e, rusqlite::Error::SqliteFailure(f, _) if f.code == rusqlite::ErrorCode::DatabaseBusy)
}

/// Moves the staged roles into `match_groups` in one transaction: what a reader sees is the
/// last pass's roles or this one's, never a mix. Returns how many rows changed.
///
/// Foreign keys are off meanwhile (a quarter to a third of the time at 100k rows: each row's
/// file was looked up twice); every row inserted is checked against `files` in the same
/// transaction instead, and they are on again whatever happens. `extra` writes the rest of
/// the pass (its links and state) in the same transaction.
fn swap_staged(
    c: &Connection,
    extra: &dyn Fn() -> rusqlite::Result<()>,
) -> rusqlite::Result<usize> {
    c.execute_batch("PRAGMA foreign_keys = OFF")?;
    let r = (|| {
        c.execute_batch("BEGIN IMMEDIATE")?;
        let r = move_staged(c)
            .and_then(|n| extra().map(|()| n))
            .and_then(|n| c.execute_batch("COMMIT").map(|()| n));
        if r.is_err() {
            let _ = c.execute_batch("ROLLBACK");
        }
        r
    })();
    // Outside any transaction now (inside one the pragma would do nothing).
    c.execute_batch("PRAGMA foreign_keys = ON")?;
    r
}

/// Drops the staged roles that went, then inserts or replaces the rest, in key order
/// (inside the caller's transaction). Returns how many rows changed.
fn move_staged(c: &Connection) -> rusqlite::Result<usize> {
    let gone = c
        .prepare_cached(
            "DELETE FROM match_groups WHERE (file_id, cat) IN
               (SELECT file_id, cat FROM temp.match_stage WHERE grp IS NULL)",
        )?
        .execute([])?;
    // A file that left meanwhile has no row to point at.
    let put = c
        .prepare_cached(
            "INSERT INTO match_groups (file_id, cat, grp, keep)
             SELECT s.file_id, s.cat, s.grp, s.keep FROM temp.match_stage s
             WHERE s.grp IS NOT NULL AND EXISTS (SELECT 1 FROM files f WHERE f.id = s.file_id)
             ORDER BY s.file_id, s.cat
             ON CONFLICT(file_id, cat) DO UPDATE SET grp = excluded.grp, keep = excluded.keep",
        )?
        .execute([])?;
    Ok(gone + put)
}

/// What one file's marks protect it as, strongest first.
fn protection(rating: i64, flag: i64, label: i64, tagged: bool) -> Option<Protect> {
    if rating > 0 {
        Some(Protect::Rated)
    } else if flag == 1 {
        Some(Protect::Flagged)
    } else if label > 0 {
        Some(Protect::Labelled)
    } else if tagged {
        Some(Protect::Tagged)
    } else {
        None
    }
}

/// What the user's marks protect: rated, picked, labelled, or tagged by hand (a sidecar's
/// keywords or Specular's own), as the gallery's marks and sidecars protect.
fn protections(c: &Connection) -> rusqlite::Result<HashMap<i64, Protect>> {
    let mut out = HashMap::new();
    let mut st = c.prepare_cached(
        "SELECT id, rating, flag, label FROM files WHERE rating > 0 OR flag = 1 OR label > 0",
    )?;
    let mut q = st.query([])?;
    while let Some(r) = q.next()? {
        if let Some(p) = protection(r.get(1)?, r.get(2)?, r.get(3)?, false) {
            out.insert(r.get(0)?, p);
        }
    }
    let mut st = c.prepare_cached("SELECT DISTINCT file_id FROM file_tags WHERE source >= 1")?;
    let mut q = st.query([])?;
    while let Some(r) = q.next()? {
        out.entry(r.get(0)?).or_insert(Protect::Tagged);
    }
    Ok(out)
}

/// Duplicate checks of one sync: remembered ones from `match_verify`, new ones made with the
/// signer (and written afterwards with the groups).
struct Checks<'a> {
    c: &'a Connection,
    cfg: &'a Config,
    signer: &'a dyn Signer,
    lumas: HashMap<i64, Option<std::sync::Arc<Luma>>>,
    places: HashMap<i64, Option<Place>>,
    verified: Vec<Verified>,
    hashed: Vec<(i64, [u8; 32])>,
}

/// Thumbnails kept for the checks of one sync.
const LUMAS: usize = 256;

impl Checks<'_> {
    fn check(&mut self, a: i64, b: i64, tier: Tier) -> Option<f32> {
        let known: Option<f64> = self
            .c
            .prepare_cached("SELECT score FROM match_verify WHERE a = ?1 AND b = ?2 AND tier = ?3")
            .and_then(|mut st| {
                st.query_row(params![a, b, tier_id(tier)], |r| r.get(0))
                    .optional()
            })
            .ok()
            .flatten();
        if let Some(s) = known {
            return Some(s as f32);
        }
        let score = match tier {
            Tier::Bytes => {
                let (x, y) = (self.file_hash(a)?, self.file_hash(b)?);
                if x == y { 1.0 } else { 0.0 }
            }
            Tier::Pixels | Tier::Visual => {
                let (x, y) = (self.luma(a)?, self.luma(b)?);
                specular_similar::ssim::ssim(&x, &y)
            }
        };
        self.verified.push(Verified { a, b, tier, score });
        Some(score)
    }

    fn place(&mut self, id: i64) -> Option<&Place> {
        if !self.places.contains_key(&id) {
            let p = self
                .c
                .prepare_cached(
                    "SELECT d.path, f.name, f.size, f.mtime_ns, th.content, h.blake3
                     FROM files f JOIN dirs d ON d.id = f.dir_id
                     LEFT JOIN thumbs th ON th.file_id = f.id
                     LEFT JOIN image_hashes h ON h.file_id = f.id WHERE f.id = ?1",
                )
                .and_then(|mut st| {
                    st.query_row([id], |r| {
                        Ok(Place {
                            path: PathBuf::from(r.get::<_, String>(0)?)
                                .join(r.get::<_, String>(1)?),
                            size: r.get(2)?,
                            mtime_ns: r.get(3)?,
                            content: r.get(4)?,
                            blake3: r
                                .get_ref(5)?
                                .as_blob_or_null()?
                                .and_then(|b| <[u8; 32]>::try_from(b).ok()),
                        })
                    })
                    .optional()
                })
                .ok()
                .flatten();
            self.places.insert(id, p);
        }
        self.places.get(&id).and_then(Option::as_ref)
    }

    fn file_hash(&mut self, id: i64) -> Option<[u8; 32]> {
        let signer = self.signer;
        let p = self.place(id)?;
        if let Some(h) = p.blake3 {
            return Some(h);
        }
        let h = signer.file_hash(&p.path)?;
        if let Some(Some(p)) = self.places.get_mut(&id) {
            p.blake3 = Some(h);
        }
        self.hashed.push((id, h));
        Some(h)
    }

    fn luma(&mut self, id: i64) -> Option<std::sync::Arc<Luma>> {
        if let Some(l) = self.lumas.get(&id) {
            return l.clone();
        }
        let (signer, cache, flags) = (self.signer, self.cfg.cache.clone(), self.cfg.cache_flags);
        let l = self.place(id).and_then(|p| {
            let key = key_of(p.size, p.mtime_ns, p.content?);
            signer.luma(&p.path, &key, cache.as_deref(), flags)
        });
        let l = l.map(std::sync::Arc::new);
        if self.lumas.len() >= LUMAS {
            self.lumas.clear();
        }
        self.lumas.insert(id, l.clone());
        l
    }
}

/// The roles of every file in a group, for search: file → [duplicates, similar] as Some(keep).
pub fn roles(c: &Connection) -> rusqlite::Result<HashMap<i64, [Option<bool>; 2]>> {
    let mut out: HashMap<i64, [Option<bool>; 2]> = HashMap::new();
    let mut st = c.prepare_cached("SELECT file_id, cat, keep FROM match_groups")?;
    let mut q = st.query([])?;
    while let Some(r) = q.next()? {
        let cat: i64 = r.get(1)?;
        if let Some(slot) = out.entry(r.get(0)?).or_default().get_mut(cat as usize) {
            *slot = Some(r.get::<_, i64>(2)? != 0);
        }
    }
    Ok(out)
}
