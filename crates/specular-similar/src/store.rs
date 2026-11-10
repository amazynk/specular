//! Signatures and decisions on disk, next to the thumbnail cache, behind an API shaped like
//! the catalogue's future `image_hashes` (per file: [`FileId`] → [`Record`]) and
//! `match_groups` (per member: category → [`Decision`]; skipped groups; split pairs) tables.
//!
//! Two files under the store directory, each rewritten atomically (temp file + rename) on
//! [`Store::flush`]:
//!
//! - `signatures.bin`: binary, a cache — dropped whole if it does not parse;
//! - `decisions.tsv`: text, the user's work — read line by line, bad lines skipped.
//!
//! Decisions are keyed by [`decision_key`] (content and path), so byte-identical copies are
//! decided apart. Next to them the store keeps each decided member's identity (content id and
//! path), and [`Store::follow`] moves a member's decisions, splits and skips to its new key
//! when a scan finds the same content at a new path and the old path is gone: decisions
//! survive renames and moves. Files the review itself trashed or moved away are marked
//! carried out and never followed, so a copy elsewhere does not inherit their discard.

use crate::focus::{FOCUS_BYTES, Focus, LEGACY_FOCUS_BYTES};
use crate::group::Category;
use crate::hints::Hints;
use crate::sig::{SIG_BYTES, Signature};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};

/// SPSIM2 adds the [`Focus`] of each record, SPSIM3 its measures at smaller sides
/// ([`Record::focus_at`]), SPSIM4 each measure's version and decode ([`Focus::version`]).
/// SPSIM3 and SPSIM2 files are still read, their measures as version 0 (the scan checks
/// those whose decode may have changed since, [`Focus::needs_check`]); SPSIM1 without them.
const MAGIC: &[u8; 8] = b"SPSIM4\0\0";
const MAGIC_3: &[u8; 8] = b"SPSIM3\0\0";
const MAGIC_2: &[u8; 8] = b"SPSIM2\0\0";
const MAGIC_1: &[u8; 8] = b"SPSIM1\0\0";
/// At most this many [`Record::focus_at`] measures are kept per file.
pub const FOCUS_AT_MAX: usize = 4;
const HINT_BYTES: usize = 2 + 8 + 7 * 8 + 16 + 4;

/// What identifies a file's content: its size and the thumbnail cache's content hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct FileId {
    pub len: u64,
    pub content: u64,
}

/// Everything matching needs of one file (the `image_hashes` row).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Record {
    pub sig: Signature,
    pub hints: Hints,
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub blake3: Option<[u8; 32]>,
    /// Sharpness and noise from full-resolution crops, measured for similar-shot members
    /// only ([`crate::focus`]).
    pub focus: Option<Focus>,
    /// The same measured at the scale where the longer side is the given px, for comparing
    /// with smaller members of a group (sorted by side, at most [`FOCUS_AT_MAX`]).
    pub focus_at: Vec<(u32, Focus)>,
}

/// The user's call on one member.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Decision {
    #[default]
    Undecided,
    Keep,
    Discard,
}

/// What discarding does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum DiscardAction {
    /// To the system trash (undoable). The default.
    #[default]
    Trash,
    /// Into a folder the user chose.
    Move,
    /// Only flag as rejected; the file stays.
    Reject,
}

impl DiscardAction {
    pub fn id(self) -> &'static str {
        match self {
            DiscardAction::Trash => "trash",
            DiscardAction::Move => "move",
            DiscardAction::Reject => "reject",
        }
    }
    pub fn from_id(s: &str) -> Option<DiscardAction> {
        match s {
            "trash" => Some(DiscardAction::Trash),
            "move" => Some(DiscardAction::Move),
            "reject" => Some(DiscardAction::Reject),
            _ => None,
        }
    }
}

/// Where a decided member was last seen.
#[derive(Clone, Debug, PartialEq)]
struct Ident {
    id: FileId,
    path: PathBuf,
    /// Discarded and carried out by the review (trashed or moved): never followed.
    carried: bool,
}

struct Entry {
    len: u64,
    mtime_ns: i64,
    id: FileId,
    rec: Record,
}

/// The key of a member's decision: its content and its path, so two byte-identical copies
/// are decided apart.
pub fn decision_key(id: FileId, path: &Path) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut eat = |b: &[u8]| {
        for &x in b {
            h = (h ^ u64::from(x)).wrapping_mul(0x100_0000_01b3);
        }
    };
    eat(&id.len.to_le_bytes());
    eat(&id.content.to_le_bytes());
    eat(path.to_string_lossy().as_bytes());
    h
}

/// The key of a group: its members' decision keys, in any order.
pub fn group_key(members: &[u64]) -> u64 {
    let mut v = members.to_vec();
    v.sort_unstable();
    v.iter().fold(0x9e37_79b9_7f4a_7c15u64, |h, &k| {
        (h ^ k).wrapping_mul(0x100_0000_01b3).rotate_left(17)
    })
}

/// Signatures, verify results and decisions.
pub struct Store {
    dir: Option<PathBuf>,
    files: HashMap<PathBuf, Entry>,
    verify: HashMap<(FileId, FileId), f32>,
    decisions: HashMap<(Category, u64), Decision>,
    skipped: HashSet<(Category, u64)>,
    /// The member keys of skipped groups (so a skip can follow a renamed member).
    members: HashMap<(Category, u64), Vec<u64>>,
    splits: HashSet<(u64, u64)>,
    /// Identities of decided members (persisted).
    idents: HashMap<u64, Ident>,
    /// Every member of the last scan (memory only): identities for new decisions.
    seen: HashMap<u64, (FileId, PathBuf)>,
    pub discard: DiscardAction,
    pub folder: Option<PathBuf>,
    /// Similar-shots sensitivity, 0 (tight) … 1 (loose).
    pub sensitivity: f32,
    dirty_sigs: bool,
    dirty_decisions: bool,
}

impl Default for Store {
    fn default() -> Store {
        Store {
            dir: None,
            files: HashMap::new(),
            verify: HashMap::new(),
            decisions: HashMap::new(),
            skipped: HashSet::new(),
            members: HashMap::new(),
            splits: HashSet::new(),
            idents: HashMap::new(),
            seen: HashMap::new(),
            discard: DiscardAction::Trash,
            folder: None,
            sensitivity: 0.5,
            dirty_sigs: false,
            dirty_decisions: false,
        }
    }
}

fn ordered(a: u64, b: u64) -> (u64, u64) {
    (a.min(b), a.max(b))
}

impl Store {
    /// A store kept in memory only (screenshots, tests).
    pub fn memory() -> Store {
        Store::default()
    }

    /// Opens (or starts) the store in `dir`; unreadable parts start empty.
    pub fn open(dir: &Path) -> Store {
        let mut s = Store {
            dir: Some(dir.to_path_buf()),
            ..Store::default()
        };
        if let Ok(b) = std::fs::read(dir.join("signatures.bin"))
            && s.read_signatures(&b).is_none()
        {
            s.files.clear();
            s.verify.clear();
        }
        if let Ok(t) = std::fs::read_to_string(dir.join("decisions.tsv")) {
            s.read_decisions(&t);
        }
        s
    }

    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    /// The record of `path` if its size and modification time are unchanged.
    pub fn lookup(&self, path: &Path, len: u64, mtime_ns: i64) -> Option<(FileId, &Record)> {
        self.files
            .get(path)
            .filter(|e| e.len == len && e.mtime_ns == mtime_ns)
            .map(|e| (e.id, &e.rec))
    }

    pub fn insert(&mut self, path: &Path, len: u64, mtime_ns: i64, id: FileId, rec: Record) {
        self.files.insert(
            path.to_path_buf(),
            Entry {
                len,
                mtime_ns,
                id,
                rec,
            },
        );
        self.dirty_sigs = true;
    }

    /// Sets a file's BLAKE3 (tier 0) once computed.
    pub fn set_blake3(&mut self, path: &Path, h: [u8; 32]) {
        if let Some(e) = self.files.get_mut(path) {
            e.rec.blake3 = Some(h);
            self.dirty_sigs = true;
        }
    }

    /// Sets a file's [`Focus`] once measured.
    pub fn set_focus(&mut self, path: &Path, f: Focus) {
        if let Some(e) = self.files.get_mut(path) {
            e.rec.focus = Some(f);
            self.dirty_sigs = true;
        }
    }

    /// Adds a file's [`Focus`] measured at longer side `side` (replacing one at that side;
    /// the largest sides go first once [`FOCUS_AT_MAX`] are kept).
    pub fn add_focus_at(&mut self, path: &Path, side: u32, f: Focus) {
        if let Some(e) = self.files.get_mut(path) {
            let at = &mut e.rec.focus_at;
            at.retain(|&(s, _)| s != side);
            at.push((side, f));
            at.sort_by_key(|&(s, _)| s);
            at.truncate(FOCUS_AT_MAX);
            self.dirty_sigs = true;
        }
    }

    /// Drops a file's [`Record::focus_at`] measures that `stale` picks (to be measured again).
    pub fn drop_focus_at(&mut self, path: &Path, stale: &dyn Fn(&Focus) -> bool) {
        if let Some(e) = self.files.get_mut(path) {
            let n = e.rec.focus_at.len();
            e.rec.focus_at.retain(|(_, f)| !stale(f));
            if e.rec.focus_at.len() != n {
                self.dirty_sigs = true;
            }
        }
    }

    /// Drops files not in `keep` (so the store does not grow without bound).
    pub fn retain(&mut self, keep: &dyn Fn(&Path) -> bool) {
        let n = self.files.len();
        self.files.retain(|p, _| keep(p));
        if self.files.len() != n {
            self.dirty_sigs = true;
        }
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// A cached SSIM verify result.
    pub fn verified(&self, a: FileId, b: FileId) -> Option<f32> {
        self.verify.get(&(a.min(b), a.max(b))).copied()
    }

    pub fn set_verified(&mut self, a: FileId, b: FileId, ssim: f32) {
        self.verify.insert((a.min(b), a.max(b)), ssim);
        self.dirty_sigs = true;
    }

    pub fn decision(&self, cat: Category, key: u64) -> Decision {
        self.decisions.get(&(cat, key)).copied().unwrap_or_default()
    }

    pub fn decide(&mut self, cat: Category, key: u64, d: Decision) {
        if d == Decision::Undecided {
            self.decisions.remove(&(cat, key));
            // Undone: the file is back.
            if let Some(i) = self.idents.get_mut(&key) {
                i.carried = false;
            }
        } else {
            self.decisions.insert((cat, key), d);
        }
        self.dirty_decisions = true;
    }

    /// Discarded in either category: out of every group.
    pub fn discarded(&self, key: u64) -> bool {
        [Category::Duplicates, Category::Similar]
            .iter()
            .any(|&c| self.decision(c, key) == Decision::Discard)
    }

    pub fn skip(&mut self, cat: Category, group: u64) {
        self.skipped.insert((cat, group));
        self.dirty_decisions = true;
    }

    /// Skips the group of these member keys, remembering them so the skip can follow a
    /// renamed member.
    pub fn skip_group(&mut self, cat: Category, members: &[u64]) {
        let g = group_key(members);
        self.skipped.insert((cat, g));
        self.members.insert((cat, g), members.to_vec());
        self.dirty_decisions = true;
    }

    /// Records the members of a group skipped before members were kept (a migration).
    pub fn note_skip_members(&mut self, cat: Category, members: &[u64]) {
        let g = group_key(members);
        if self.skipped.contains(&(cat, g)) && !self.members.contains_key(&(cat, g)) {
            self.members.insert((cat, g), members.to_vec());
            self.dirty_decisions = true;
        }
    }

    pub fn unskip_all(&mut self, cat: Category) {
        self.skipped.retain(|(c, _)| *c != cat);
        self.members.retain(|(c, _), _| *c != cat);
        self.dirty_decisions = true;
    }

    pub fn skipped(&self, cat: Category, group: u64) -> bool {
        self.skipped.contains(&(cat, group))
    }

    /// "Not similar": the two never group again (in either category).
    pub fn split(&mut self, a: u64, b: u64) {
        self.splits.insert(ordered(a, b));
        self.dirty_decisions = true;
    }

    pub fn is_split(&self, a: u64, b: u64) -> bool {
        self.splits.contains(&ordered(a, b))
    }

    /// The review carried out these members' discards (trashed or moved the files): their
    /// decisions are never followed to another file with the same content.
    pub fn carried_out(&mut self, keys: &[u64]) {
        for &k in keys {
            let Some(mut i) = self.ident(k) else { continue };
            i.carried = true;
            self.idents.insert(k, i);
            self.dirty_decisions = true;
        }
    }

    fn ident(&self, k: u64) -> Option<Ident> {
        self.idents.get(&k).cloned().or_else(|| {
            self.seen.get(&k).map(|(id, path)| Ident {
                id: *id,
                path: path.clone(),
                carried: false,
            })
        })
    }

    /// Keys something refers to: a decision, a split or a skipped group.
    fn referenced(&self) -> HashSet<u64> {
        let mut r: HashSet<u64> = self.decisions.keys().map(|&(_, k)| k).collect();
        for &(a, b) in &self.splits {
            r.insert(a);
            r.insert(b);
        }
        for (g, m) in &self.members {
            if self.skipped.contains(g) {
                r.extend(m.iter().copied());
            }
        }
        r
    }

    /// Takes a scan's members — (decision key, content id, path) — and moves the decisions,
    /// splits and skips of members that were renamed or moved to their new keys: a decided
    /// member whose path is gone, whose content shows up in exactly one undecided scanned
    /// file, and whose discard the review did not carry out itself. Decisions written
    /// before identities were kept are matched through the signature cache's paths. Returns
    /// how many members were followed.
    pub fn follow(&mut self, items: &[(u64, FileId, &Path)]) -> usize {
        // Members decided since the last scan: their identities from that scan.
        for k in self.referenced() {
            if !self.idents.contains_key(&k)
                && let Some(i) = self.ident(k)
            {
                self.idents.insert(k, i);
            }
        }
        self.seen = items
            .iter()
            .map(|&(k, id, p)| (k, (id, p.to_path_buf())))
            .collect();
        let refd = self.referenced();
        // Identities of decided members found where they were decided (this also migrates
        // decisions made before identities were kept).
        for &(k, id, p) in items {
            if !refd.contains(&k) {
                continue;
            }
            match self.idents.get_mut(&k) {
                // Back where it was (restored from the trash): followable again.
                Some(i) if i.carried => {
                    i.carried = false;
                    self.dirty_decisions = true;
                }
                Some(_) => {}
                None => {
                    self.idents.insert(
                        k,
                        Ident {
                            id,
                            path: p.to_path_buf(),
                            carried: false,
                        },
                    );
                    self.dirty_decisions = true;
                }
            }
        }
        // Older decisions of members that have moved since: the signature cache still
        // knows their old paths. A discarded one is most likely gone because the review
        // trashed it, so it counts as carried out.
        let unknown: HashSet<u64> = refd
            .iter()
            .copied()
            .filter(|k| !self.idents.contains_key(k))
            .collect();
        if !unknown.is_empty() {
            for (p, e) in &self.files {
                let k = decision_key(e.id, p);
                if unknown.contains(&k) {
                    let carried = [Category::Duplicates, Category::Similar]
                        .iter()
                        .any(|&c| self.decisions.get(&(c, k)) == Some(&Decision::Discard));
                    self.idents.insert(
                        k,
                        Ident {
                            id: e.id,
                            path: p.clone(),
                            carried,
                        },
                    );
                    self.dirty_decisions = true;
                }
            }
        }
        // Decided members not in this scan, by content.
        let mut orphans: HashMap<FileId, Vec<u64>> = HashMap::new();
        for (&k, i) in &self.idents {
            if refd.contains(&k) && !i.carried && !self.seen.contains_key(&k) {
                orphans.entry(i.id).or_default().push(k);
            }
        }
        if orphans.is_empty() {
            return 0;
        }
        let mut fresh: HashMap<FileId, Vec<(u64, &Path)>> = HashMap::new();
        for &(k, id, p) in items {
            if !refd.contains(&k) && orphans.contains_key(&id) {
                fresh.entry(id).or_default().push((k, p));
            }
        }
        let mut moves = Vec::new();
        for (id, f) in fresh {
            // Two new copies of one content: which one is the old file would be a guess.
            let [(new, path)] = f[..] else { continue };
            let gone: Vec<u64> = orphans[&id]
                .iter()
                .copied()
                .filter(|k| std::fs::symlink_metadata(&self.idents[k].path).is_err())
                .collect();
            if let [old] = gone[..] {
                moves.push((old, new, path.to_path_buf()));
            }
        }
        for (old, new, path) in &moves {
            self.rekey(*old, *new, path);
        }
        moves.len()
    }

    /// Moves everything recorded under member key `old` to `new` (the same content at `path`).
    fn rekey(&mut self, old: u64, new: u64, path: &Path) {
        for c in [Category::Duplicates, Category::Similar] {
            if let Some(d) = self.decisions.remove(&(c, old)) {
                self.decisions.insert((c, new), d);
            }
        }
        let sub = |k: u64| if k == old { new } else { k };
        self.splits = self
            .splits
            .drain()
            .map(|(a, b)| ordered(sub(a), sub(b)))
            .collect();
        let moved: Vec<(Category, u64)> = self
            .members
            .iter()
            .filter(|(_, m)| m.contains(&old))
            .map(|(g, _)| *g)
            .collect();
        for (c, g) in moved {
            let Some(m) = self.members.remove(&(c, g)) else {
                continue;
            };
            let m: Vec<u64> = m.into_iter().map(sub).collect();
            let ng = group_key(&m);
            if self.skipped.remove(&(c, g)) {
                self.skipped.insert((c, ng));
            }
            self.members.insert((c, ng), m);
        }
        if let Some(i) = self.idents.remove(&old) {
            self.idents.insert(
                new,
                Ident {
                    id: i.id,
                    path: path.to_path_buf(),
                    carried: false,
                },
            );
        }
        self.dirty_decisions = true;
    }

    /// Marks the options as changed (after setting `discard`, `folder` or `sensitivity`).
    pub fn options_changed(&mut self) {
        self.dirty_decisions = true;
    }

    pub fn dirty(&self) -> bool {
        self.dirty_sigs || self.dirty_decisions
    }

    /// Writes what changed; a memory store does nothing.
    pub fn flush(&mut self) -> std::io::Result<()> {
        let Some(dir) = self.dir.clone() else {
            self.dirty_sigs = false;
            self.dirty_decisions = false;
            return Ok(());
        };
        if !self.dirty() {
            return Ok(());
        }
        std::fs::create_dir_all(&dir)?;
        if self.dirty_decisions {
            write_atomic(&dir.join("decisions.tsv"), self.decisions_text().as_bytes())?;
            self.dirty_decisions = false;
        }
        if self.dirty_sigs {
            write_atomic(&dir.join("signatures.bin"), &self.signatures_bytes())?;
            self.dirty_sigs = false;
        }
        Ok(())
    }

    fn decisions_text(&self) -> String {
        let mut s = String::from("# Specular duplicates and similar shots: decisions\n");
        let mut d: Vec<_> = self.decisions.iter().collect();
        d.sort_by_key(|((c, k), _)| (*c, *k));
        for ((c, k), v) in d {
            let v = match v {
                Decision::Keep => "keep",
                Decision::Discard => "discard",
                Decision::Undecided => continue,
            };
            s += &format!("d\t{}\t{k:016x}\t{v}\n", c.id());
        }
        let mut sk: Vec<_> = self.skipped.iter().collect();
        sk.sort();
        for (c, g) in sk {
            s += &format!("s\t{}\t{g:016x}\n", c.id());
            if let Some(m) = self.members.get(&(*c, *g)) {
                let m: Vec<String> = m.iter().map(|k| format!("{k:016x}")).collect();
                s += &format!("m\t{}\t{g:016x}\t{}\n", c.id(), m.join(","));
            }
        }
        let mut sp: Vec<_> = self.splits.iter().collect();
        sp.sort();
        for (a, b) in sp {
            s += &format!("n\t{a:016x}\t{b:016x}\n");
        }
        let mut refd: Vec<u64> = self.referenced().into_iter().collect();
        refd.sort_unstable();
        for k in refd {
            let Some(i) = self.ident(k) else { continue };
            let p = i.path.to_string_lossy();
            if p.contains(['\n', '\r']) {
                continue;
            }
            let flag = if i.carried { "carried" } else { "-" };
            s += &format!(
                "i\t{k:016x}\t{:016x}\t{:016x}\t{flag}\t{p}\n",
                i.id.len, i.id.content
            );
        }
        s += &format!("o\tdiscard\t{}\n", self.discard.id());
        if let Some(f) = &self.folder {
            s += &format!("o\tfolder\t{}\n", f.to_string_lossy());
        }
        s += &format!("o\tsensitivity\t{}\n", self.sensitivity);
        s
    }

    fn read_decisions(&mut self, t: &str) {
        let hex = |s: &str| u64::from_str_radix(s, 16).ok();
        for line in t.lines() {
            // An identity: the path is the last field and may hold tabs.
            if let Some(rest) = line.strip_prefix("i\t") {
                let f: Vec<&str> = rest.splitn(5, '\t').collect();
                if let [k, len, content, flag, path] = f[..]
                    && let (Some(k), Some(len), Some(content)) = (hex(k), hex(len), hex(content))
                    && !path.is_empty()
                {
                    self.idents.insert(
                        k,
                        Ident {
                            id: FileId { len, content },
                            path: PathBuf::from(path),
                            carried: flag == "carried",
                        },
                    );
                }
                continue;
            }
            let f: Vec<&str> = line.split('\t').collect();
            match f.as_slice() {
                ["d", c, k, v] => {
                    let (Some(c), Some(k)) = (Category::from_id(c), hex(k)) else {
                        continue;
                    };
                    let v = match *v {
                        "keep" => Decision::Keep,
                        "discard" => Decision::Discard,
                        _ => continue,
                    };
                    self.decisions.insert((c, k), v);
                }
                ["s", c, g] => {
                    if let (Some(c), Some(g)) = (Category::from_id(c), hex(g)) {
                        self.skipped.insert((c, g));
                    }
                }
                ["m", c, g, m] => {
                    let (Some(c), Some(g)) = (Category::from_id(c), hex(g)) else {
                        continue;
                    };
                    let m: Option<Vec<u64>> = m.split(',').map(hex).collect();
                    if let Some(m) = m.filter(|m| group_key(m) == g) {
                        self.members.insert((c, g), m);
                    }
                }
                ["n", a, b] => {
                    if let (Some(a), Some(b)) = (hex(a), hex(b)) {
                        self.splits.insert(ordered(a, b));
                    }
                }
                ["o", "discard", v] => {
                    if let Some(a) = DiscardAction::from_id(v) {
                        self.discard = a;
                    }
                }
                ["o", "folder", v] if !v.is_empty() => self.folder = Some(PathBuf::from(v)),
                ["o", "sensitivity", v] => {
                    if let Ok(x) = v.parse::<f32>()
                        && x.is_finite()
                    {
                        self.sensitivity = x.clamp(0.0, 1.0);
                    }
                }
                _ => {}
            }
        }
    }

    fn signatures_bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(16 + self.files.len() * (96 + SIG_BYTES + HINT_BYTES));
        b.extend_from_slice(MAGIC);
        b.extend_from_slice(&(self.files.len() as u32).to_le_bytes());
        for (p, e) in &self.files {
            let ps = p.to_string_lossy();
            let pb = ps.as_bytes();
            let n = pb.len().min(u16::MAX as usize);
            b.extend_from_slice(&(n as u16).to_le_bytes());
            b.extend_from_slice(&pb[..n]);
            b.extend_from_slice(&e.len.to_le_bytes());
            b.extend_from_slice(&e.mtime_ns.to_le_bytes());
            b.extend_from_slice(&e.id.len.to_le_bytes());
            b.extend_from_slice(&e.id.content.to_le_bytes());
            b.extend_from_slice(&e.rec.width.to_le_bytes());
            b.extend_from_slice(&e.rec.height.to_le_bytes());
            b.push(e.rec.bit_depth);
            match e.rec.blake3 {
                Some(h) => {
                    b.push(1);
                    b.extend_from_slice(&h);
                }
                None => b.push(0),
            }
            b.extend_from_slice(&e.rec.sig.to_bytes());
            hints_bytes(&e.rec.hints, &mut b);
            match e.rec.focus {
                Some(f) => {
                    b.push(1);
                    b.extend_from_slice(&f.to_bytes());
                }
                None => b.push(0),
            }
            let at = &e.rec.focus_at[..e.rec.focus_at.len().min(FOCUS_AT_MAX)];
            b.push(at.len() as u8);
            for (side, f) in at {
                b.extend_from_slice(&side.to_le_bytes());
                b.extend_from_slice(&f.to_bytes());
            }
        }
        b.extend_from_slice(&(self.verify.len() as u32).to_le_bytes());
        for ((x, y), s) in &self.verify {
            for v in [x.len, x.content, y.len, y.content] {
                b.extend_from_slice(&v.to_le_bytes());
            }
            b.extend_from_slice(&s.to_le_bytes());
        }
        b
    }

    fn read_signatures(&mut self, b: &[u8]) -> Option<()> {
        let mut r = Reader { b, at: 0 };
        let version = match r.take(8)? {
            m if m == MAGIC => 4,
            m if m == MAGIC_3 => 3,
            m if m == MAGIC_2 => 2,
            m if m == MAGIC_1 => 1,
            _ => return None,
        };
        let fb = if version >= 4 {
            FOCUS_BYTES
        } else {
            LEGACY_FOCUS_BYTES
        };
        let n = r.u32()?;
        for _ in 0..n {
            let pl = usize::from(r.u16()?);
            let path = PathBuf::from(std::str::from_utf8(r.take(pl)?).ok()?);
            let len = r.u64()?;
            let mtime_ns = r.u64()? as i64;
            let id = FileId {
                len: r.u64()?,
                content: r.u64()?,
            };
            let width = r.u32()?;
            let height = r.u32()?;
            let bit_depth = r.take(1)?[0];
            let blake3 = match r.take(1)?[0] {
                0 => None,
                _ => Some(<[u8; 32]>::try_from(r.take(32)?).ok()?),
            };
            let sig = Signature::from_bytes(r.take(SIG_BYTES)?)?;
            let hints = read_hints(r.take(HINT_BYTES)?)?;
            let focus = match version >= 2 {
                true => match r.take(1)?[0] {
                    0 => None,
                    _ => Some(Focus::from_bytes(r.take(fb)?)?),
                },
                false => None,
            };
            let mut focus_at = Vec::new();
            if version >= 3 {
                for _ in 0..r.take(1)?[0] {
                    let side = r.u32()?;
                    focus_at.push((side, Focus::from_bytes(r.take(fb)?)?));
                }
                focus_at.truncate(FOCUS_AT_MAX);
            }
            self.files.insert(
                path,
                Entry {
                    len,
                    mtime_ns,
                    id,
                    rec: Record {
                        sig,
                        hints,
                        width,
                        height,
                        bit_depth,
                        blake3,
                        focus,
                        focus_at,
                    },
                },
            );
        }
        let m = r.u32()?;
        for _ in 0..m {
            let x = FileId {
                len: r.u64()?,
                content: r.u64()?,
            };
            let y = FileId {
                len: r.u64()?,
                content: r.u64()?,
            };
            let s = f32::from_le_bytes(r.take(4)?.try_into().ok()?);
            self.verify.insert((x, y), s);
        }
        Some(())
    }
}

fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.b.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(s)
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
}

fn hints_bytes(h: &Hints, b: &mut Vec<u8>) {
    let ids = [
        h.camera,
        h.serial,
        h.unique_id,
        h.document_id,
        h.original_document_id,
        h.derived_from,
        h.burst,
    ];
    let mut flags = 0u16;
    if h.time_ms.is_some() {
        flags |= 1;
    }
    if h.subsec {
        flags |= 2;
    }
    for (k, v) in ids.iter().enumerate() {
        if v.is_some() {
            flags |= 4 << k;
        }
    }
    if h.bracket {
        flags |= 1 << 9;
    }
    if h.gps.is_some() {
        flags |= 1 << 10;
    }
    if h.focal_mm.is_some() {
        flags |= 1 << 11;
    }
    if h.edited_by_software {
        flags |= 1 << 12;
    }
    if h.has_exif {
        flags |= 1 << 13;
    }
    b.extend_from_slice(&flags.to_le_bytes());
    b.extend_from_slice(&h.time_ms.unwrap_or(0).to_le_bytes());
    for v in ids {
        b.extend_from_slice(&v.unwrap_or(0).to_le_bytes());
    }
    let (la, lo) = h.gps.unwrap_or((0.0, 0.0));
    b.extend_from_slice(&la.to_le_bytes());
    b.extend_from_slice(&lo.to_le_bytes());
    b.extend_from_slice(&h.focal_mm.unwrap_or(0.0).to_le_bytes());
}

fn read_hints(b: &[u8]) -> Option<Hints> {
    let mut r = Reader { b, at: 0 };
    let flags = r.u16()?;
    let on = |bit: u16| flags & (1 << bit) != 0;
    let time = r.u64()? as i64;
    let mut ids = [None; 7];
    for (k, v) in ids.iter_mut().enumerate() {
        let x = r.u64()?;
        if on(2 + k as u16) {
            *v = Some(x);
        }
    }
    let la = f64::from_le_bytes(r.take(8)?.try_into().ok()?);
    let lo = f64::from_le_bytes(r.take(8)?.try_into().ok()?);
    let focal = f32::from_le_bytes(r.take(4)?.try_into().ok()?);
    Some(Hints {
        time_ms: on(0).then_some(time),
        subsec: on(1),
        camera: ids[0],
        serial: ids[1],
        unique_id: ids[2],
        document_id: ids[3],
        original_document_id: ids[4],
        derived_from: ids[5],
        burst: ids[6],
        bracket: on(9),
        gps: on(10).then_some((la, lo)),
        focal_mm: on(11).then_some(focal),
        edited_by_software: on(12),
        has_exif: on(13),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        // Scratch goes in the target dir the test binary runs from (target/…/debug/deps/x).
        let exe = std::env::current_exe().unwrap();
        let base = exe.ancestors().nth(3).unwrap().to_path_buf();
        let d = base.join("tmp").join("specular-similar-tests").join(name);
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn decisions_and_signatures_persist() {
        let dir = scratch("persist");
        let id = FileId {
            len: 1234,
            content: 99,
        };
        let p = Path::new("/photos/a.jpg");
        let k = decision_key(id, p);
        let k2 = decision_key(id, Path::new("/photos/copy/a.jpg"));
        assert_ne!(k, k2);
        let rec = Record {
            sig: Signature::of_rgba(4, 4, &[128u8; 64]),
            hints: Hints {
                time_ms: Some(1_700_000_000_123),
                subsec: true,
                serial: Some(7),
                gps: Some((48.2, 16.3)),
                focal_mm: Some(35.0),
                has_exif: true,
                ..Hints::default()
            },
            width: 4000,
            height: 3000,
            bit_depth: 8,
            blake3: Some([5; 32]),
            focus: Some(Focus {
                scale: 1,
                grid: [3.5; 16],
                detail: 3.5,
                anisotropy: 0.1,
                noise: 1.25,
                version: crate::focus::FOCUS_VERSION,
                decoded: 1,
                regions: true,
            }),
            focus_at: vec![(
                1000,
                Focus {
                    scale: 4,
                    grid: [2.5; 16],
                    detail: 2.5,
                    anisotropy: 0.2,
                    noise: 0.75,
                    version: crate::focus::FOCUS_VERSION,
                    decoded: 2,
                    regions: false,
                },
            )],
        };
        {
            let mut s = Store::open(&dir);
            s.insert(p, 1234, 42, id, rec.clone());
            s.set_verified(id, FileId { len: 1, content: 2 }, 0.93);
            s.decide(Category::Duplicates, k, Decision::Keep);
            s.decide(Category::Similar, k2, Decision::Discard);
            s.skip(Category::Similar, group_key(&[k, k2]));
            s.split(k2, k);
            s.discard = DiscardAction::Reject;
            s.sensitivity = 0.25;
            s.options_changed();
            s.flush().unwrap();
        }
        let s = Store::open(&dir);
        assert_eq!(
            s.lookup(p, 1234, 42).map(|(i, r)| (i, r.clone())),
            Some((id, rec))
        );
        assert!(
            s.lookup(p, 1234, 43).is_none(),
            "a changed file is not looked up"
        );
        assert_eq!(s.verified(FileId { len: 1, content: 2 }, id), Some(0.93));
        assert_eq!(s.decision(Category::Duplicates, k), Decision::Keep);
        assert_eq!(s.decision(Category::Similar, k), Decision::Undecided);
        assert!(s.discarded(k2) && !s.discarded(k));
        assert!(s.skipped(Category::Similar, group_key(&[k2, k])));
        assert!(s.is_split(k, k2));
        assert_eq!(s.discard, DiscardAction::Reject);
        assert_eq!(s.sensitivity, 0.25);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Decisions, splits and skips follow a renamed member; a copy next to a present member
    /// adopts nothing, and neither does a copy of a discard the review carried out.
    #[test]
    fn decisions_follow_renames() {
        let dir = scratch("follow");
        std::fs::create_dir_all(&dir).unwrap();
        let file = |n: &str| {
            let p = dir.join(n);
            std::fs::write(&p, n).unwrap();
            p
        };
        let (a, b, c) = (file("a.jpg"), file("b.jpg"), file("c.jpg"));
        let id = |n: u64| FileId { len: n, content: n };
        let (ka, kb, kc) = (
            decision_key(id(1), &a),
            decision_key(id(2), &b),
            decision_key(id(3), &c),
        );
        {
            let mut s = Store::open(&dir);
            assert_eq!(
                s.follow(&[(ka, id(1), &a), (kb, id(2), &b), (kc, id(3), &c)]),
                0
            );
            s.decide(Category::Similar, ka, Decision::Keep);
            s.decide(Category::Similar, kb, Decision::Discard);
            s.decide(Category::Duplicates, kc, Decision::Discard);
            s.split(ka, kc);
            s.skip_group(Category::Duplicates, &[ka, kb]);
            s.carried_out(&[kc]);
            s.flush().unwrap();
        }
        // a → sub/a2.jpg; b stays and gets a copy; c (trashed) reappears elsewhere.
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        let a2 = dir.join("sub/a2.jpg");
        std::fs::rename(&a, &a2).unwrap();
        let b2 = file("b copy.jpg");
        std::fs::remove_file(&c).unwrap();
        let c2 = file("c again.jpg");
        let (ka2, kb2, kc2) = (
            decision_key(id(1), &a2),
            decision_key(id(2), &b2),
            decision_key(id(3), &c2),
        );
        let mut s = Store::open(&dir);
        let n = s.follow(&[
            (ka2, id(1), &a2),
            (kb, id(2), &b),
            (kb2, id(2), &b2),
            (kc2, id(3), &c2),
        ]);
        assert_eq!(n, 1);
        assert_eq!(s.decision(Category::Similar, ka2), Decision::Keep);
        assert_eq!(s.decision(Category::Similar, ka), Decision::Undecided);
        assert!(s.is_split(kc, ka2) && !s.is_split(ka, kc));
        assert!(s.skipped(Category::Duplicates, group_key(&[kb, ka2])));
        assert!(!s.skipped(Category::Duplicates, group_key(&[kb, ka])));
        assert!(
            s.discarded(kb) && !s.discarded(kb2),
            "a copy adopts nothing"
        );
        assert!(!s.discarded(kc2), "a carried-out discard is not followed");
        // Persisted under the new key.
        s.flush().unwrap();
        let s = Store::open(&dir);
        assert_eq!(s.decision(Category::Similar, ka2), Decision::Keep);
        assert!(s.skipped(Category::Duplicates, group_key(&[kb, ka2])));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Decisions written before identities were kept: present members get theirs on the
    /// first scan; a member moved before it is found through the signature cache's path.
    #[test]
    fn path_keyed_decisions_migrate() {
        let dir = scratch("migrate");
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a.jpg"), dir.join("b.jpg"));
        let (ia, ib) = (FileId { len: 5, content: 1 }, FileId { len: 5, content: 2 });
        let (ka, kb) = (decision_key(ia, &a), decision_key(ib, &b));
        std::fs::write(
            dir.join("decisions.tsv"),
            format!(
                "d\tsimilar\t{ka:016x}\tkeep\nd\tsimilar\t{kb:016x}\tkeep\n\
                 s\tsimilar\t{:016x}\n",
                group_key(&[ka, kb])
            ),
        )
        .unwrap();
        // The cache saw both; b has since moved, a is still there.
        let mut s = Store::open(&dir);
        s.insert(&a, 5, 1, ia, Record::default());
        s.insert(&b, 5, 1, ib, Record::default());
        std::fs::write(&a, "aaaaa").unwrap();
        let b2 = dir.join("renamed.jpg");
        std::fs::write(&b2, "bbbbb").unwrap();
        let kb2 = decision_key(ib, &b2);
        // The group is seen once more under its old keys (legacy skip gets its members).
        s.note_skip_members(Category::Similar, &[ka, kb]);
        assert_eq!(s.follow(&[(ka, ia, &a), (kb2, ib, &b2)]), 1);
        assert_eq!(s.decision(Category::Similar, kb2), Decision::Keep);
        assert!(s.skipped(Category::Similar, group_key(&[ka, kb2])));
        s.flush().unwrap();
        let t = std::fs::read_to_string(dir.join("decisions.tsv")).unwrap();
        assert!(t.contains(&format!("i\t{ka:016x}\t")), "{t}");
        assert!(t.contains(&format!("i\t{kb2:016x}\t")), "{t}");
        assert!(t.contains("\tm\t") || t.contains("\nm\tsimilar\t"), "{t}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn damaged_files_start_empty() {
        let dir = scratch("damaged");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("signatures.bin"), b"SPSIM1\0\0\x05\0\0\0garbage").unwrap();
        std::fs::write(
            dir.join("decisions.tsv"),
            "d\tdupes\tzz\tkeep\nd\tsimilar\t00000000000000ff\tdiscard\nbad\n",
        )
        .unwrap();
        let s = Store::open(&dir);
        assert!(s.is_empty());
        assert!(s.discarded(0xff));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn signatures_before_focus_still_read() {
        let p = Path::new("/photos/b.jpg");
        let id = FileId { len: 9, content: 3 };
        let mut s = Store::default();
        s.insert(p, 9, 1, id, Record::default());
        // Without measures SPSIM3 differs from SPSIM4 by its magic only.
        let mut v3 = s.signatures_bytes();
        v3[..8].copy_from_slice(MAGIC_3);
        // SPSIM2: no count of focus_at measures (the last byte before the verify count).
        let mut v2 = v3.clone();
        v2[..8].copy_from_slice(MAGIC_2);
        v2.remove(v2.len() - 5);
        let mut two = Store::default();
        assert_eq!(two.read_signatures(&v2), Some(()));
        assert_eq!(
            two.lookup(p, 9, 1)
                .map(|(i, r)| (i, r.focus, r.focus_at.len())),
            Some((id, None, 0))
        );
        // SPSIM1: no focus flag after the hints either.
        let mut v1 = v2.clone();
        v1[..8].copy_from_slice(MAGIC_1);
        v1.remove(v1.len() - 5);
        let mut old = Store::default();
        assert_eq!(old.read_signatures(&v1), Some(()));
        assert_eq!(
            old.lookup(p, 9, 1).map(|(i, r)| (i, r.focus)),
            Some((id, None))
        );
        // Measured later: kept with the record.
        let f = Focus {
            scale: 2,
            ..Focus::UNAVAILABLE
        };
        old.set_focus(p, f);
        for side in [900, 300, 600, 1200, 300, 150] {
            let g = Focus {
                scale: side,
                ..Focus::UNAVAILABLE
            };
            old.add_focus_at(p, side, g);
        }
        let mut new = Store::default();
        assert_eq!(new.read_signatures(&old.signatures_bytes()), Some(()));
        let rec = new.lookup(p, 9, 1).unwrap().1;
        assert_eq!(rec.focus, Some(f));
        // Sorted, one per side, the smallest kept.
        let sides: Vec<u32> = rec.focus_at.iter().map(|&(s, _)| s).collect();
        assert_eq!(sides, [150, 300, 600, 900]);
    }

    #[test]
    fn spsim3_measures_read_as_legacy_and_written_with_versions() {
        let p = Path::new("/photos/c.arw");
        let id = FileId { len: 7, content: 4 };
        let f = Focus {
            scale: 2,
            grid: [4.0; 16],
            detail: 4.0,
            anisotropy: 0.2,
            noise: 1.5,
            version: crate::focus::FOCUS_VERSION,
            decoded: 2,
            regions: false,
        };
        let g = Focus {
            scale: 3,
            decoded: 2,
            ..f
        };
        let mut s = Store::default();
        s.insert(p, 7, 1, id, Record::default());
        s.set_focus(p, f);
        s.add_focus_at(p, 1500, g);
        let v4 = s.signatures_bytes();
        // SPSIM3: the same without each measure's last 4 bytes (own measure, count, side,
        // measure at the side, then the verify count).
        let mut v3 = v4.clone();
        v3[..8].copy_from_slice(MAGIC_3);
        let l = v3.len();
        v3.drain(l - 8..l - 4);
        v3.drain(l - 97..l - 93);
        let mut old = Store::default();
        assert_eq!(old.read_signatures(&v3), Some(()));
        let legacy = |x: Focus| Focus {
            version: 0,
            decoded: 0,
            regions: false,
            ..x
        };
        let rec = old.lookup(p, 7, 1).unwrap().1.clone();
        assert_eq!(rec.focus, Some(legacy(f)));
        assert_eq!(rec.focus_at, vec![(1500, legacy(g))]);
        // Written back as SPSIM4, version 0 kept until measured again.
        let mut again = Store::default();
        assert_eq!(again.read_signatures(&old.signatures_bytes()), Some(()));
        assert_eq!(again.lookup(p, 7, 1).unwrap().1, &rec);
        // Stale reduced measures dropped; a new measure replaces the legacy one.
        again.drop_focus_at(p, &|x| x.legacy_reduced_stale(true));
        again.set_focus(p, f);
        let mut new = Store::default();
        assert_eq!(new.read_signatures(&again.signatures_bytes()), Some(()));
        let rec = new.lookup(p, 7, 1).unwrap().1;
        assert_eq!((rec.focus, rec.focus_at.len()), (Some(f), 0));
    }
}
