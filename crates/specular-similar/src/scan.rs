//! The background scan: signatures and hints per unit, tier 0 hashes for colliding sizes,
//! candidate verify, and the duplicate clusters and similar-shot edges the UI regroups live.
//!
//! Signatures are made inside the thumbnail engine ([`digester`]): an engine that carries the
//! digester, the gallery's or the scan's, hashes every class-[`SIG_CLASS`] thumbnail it
//! generates from its fresh pixels, on its worker, and stores the signature beside the cache
//! entry. A scan reads those first without decoding anything, and fetches thumbnails only for
//! files that have none: a miss is generated (and hashed) on an engine worker and cached for
//! the gallery too; an entry cached before digests (or by an engine without the digester) is
//! decoded and hashed here, from its cached pixels, as before.
//!
//! The scan asks for its thumbnails at prefetch priority (nothing counts as visible), so the
//! pool runs the gallery's visible tiles and the viewer's decodes first.

use crate::analyse::{self, Group};
use crate::focus::Focus;
use crate::group::{DupCandidate, Edge, Item, Kind};
use crate::hints::{self, Hints};
use crate::pick::Protect;
use crate::sig::{Luma, SIG_CLASS, Signature};
use crate::store::{FileId, Record, Store, decision_key};
use specular_thumbs::pixels::Pixels;
use specular_thumbs::{Digester, DiskCache, Engine, KeyMemo, Request, engine::Outcome};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Bytes of the file head read for metadata hints.
const HEAD: usize = 256 << 10;
/// Thumbnails asked for at once.
const BATCH: usize = 48;
/// Decoded thumbnails kept for the verify step.
const VERIFY_LRU: usize = 160;

/// One unit to scan: a file, or a RAW with its JPEG (and sidecars) as one.
#[derive(Clone, Debug, PartialEq)]
pub struct Unit {
    /// The file that represents the unit (the RAW of a pair).
    pub main: PathBuf,
    /// Every file of the unit (main first): what discarding moves.
    pub files: Vec<PathBuf>,
    /// The file the thumbnail and signature come from (the JPEG of a pair: cheaper).
    pub sig_path: PathBuf,
}

impl Unit {
    /// Whether [`Unit::sig_path`] (what focus measures) is a camera RAW.
    fn measures_raw(&self) -> bool {
        let ext = self.sig_path.extension().map(|e| e.to_string_lossy());
        Kind::from_ext(ext.as_deref().unwrap_or("")) == Kind::Raw
    }
}

/// Groups `paths` into units: same folder and stem with a RAW and a rendered file → one
/// unit; XMP and edit sidecars join their image.
pub fn units(paths: &[PathBuf]) -> Vec<Unit> {
    let mut by_stem: HashMap<(PathBuf, String), Vec<&PathBuf>> = HashMap::new();
    let mut order = Vec::new();
    for p in paths {
        let dir = p.parent().map(Path::to_path_buf).unwrap_or_default();
        let stem = p
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let k = (dir, stem);
        let e = by_stem.entry(k.clone()).or_default();
        if e.is_empty() {
            order.push(k);
        }
        e.push(p);
    }
    let kind = |p: &Path| {
        Kind::from_ext(
            &p.extension()
                .map(|e| e.to_string_lossy())
                .unwrap_or_default(),
        )
    };
    let mut out = Vec::new();
    for k in order {
        let files = &by_stem[&k];
        let raws: Vec<&PathBuf> = files
            .iter()
            .copied()
            .filter(|p| kind(p) == Kind::Raw)
            .collect();
        let rendered: Vec<&PathBuf> = files
            .iter()
            .copied()
            .filter(|p| matches!(kind(p), Kind::Jpeg | Kind::Heic))
            .collect();
        if raws.len() == 1 && rendered.len() == 1 {
            out.push(Unit {
                main: raws[0].clone(),
                files: vec![raws[0].clone(), rendered[0].clone()],
                sig_path: rendered[0].clone(),
            });
            for p in files {
                if *p != raws[0] && *p != rendered[0] {
                    out.push(single(p));
                }
            }
        } else {
            out.extend(files.iter().map(|p| single(p)));
        }
    }
    out
}

fn single(p: &Path) -> Unit {
    Unit {
        main: p.to_path_buf(),
        files: vec![p.to_path_buf()],
        sig_path: p.to_path_buf(),
    }
}

/// Sidecars of an image that exist: `name.ext.xmp`, `name.xmp`, and edit files.
pub fn sidecars(p: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let name = p.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    let mut cands = Vec::new();
    for ext in ["xmp", "XMP", "pp3", "dop"] {
        let mut n = name.clone();
        n.push(".");
        n.push(ext);
        cands.push(p.with_file_name(n));
    }
    cands.push(p.with_extension("xmp"));
    cands.push(p.with_extension("XMP"));
    for c in cands {
        if !out.contains(&c) && c.is_file() {
            out.push(c);
        }
    }
    out
}

/// What the user did to the unit, from its sidecars: rated, flagged, labelled, tagged,
/// edited (an XMP with develop settings, a RawTherapee or DxO edit file).
pub fn sidecar_protect(files: &[PathBuf]) -> Option<Protect> {
    let mut facts = hints::UserFacts::default();
    for f in files {
        for s in sidecars(f) {
            let ext = s.extension().map(|e| e.to_ascii_lowercase());
            if ext.as_deref() != Some(std::ffi::OsStr::new("xmp")) {
                facts.edited = true;
                continue;
            }
            if let Ok(b) = std::fs::read(&s)
                && b.len() < 8 << 20
            {
                let x = hints::user_facts(&String::from_utf8_lossy(&b));
                facts.rated |= x.rated;
                facts.flagged |= x.flagged;
                facts.labelled |= x.labelled;
                facts.tagged |= x.tagged;
                facts.edited |= x.edited;
            }
        }
    }
    Protect::of(&facts)
}

/// How the scan reaches thumbnails.
pub struct ScanSetup {
    /// An engine for the scan alone (sharing the gallery's registry, pool and cache).
    pub engine: Engine,
    /// Wakes when `engine` has results.
    pub wake: crossbeam_channel::Receiver<()>,
    pub store: Arc<Mutex<Store>>,
    pub cancel: Arc<AtomicBool>,
    /// The file keys `engine` reads (from [`engine`]): the scan's cache look-up, the engine
    /// and the content id share them, so each file's samples are read once.
    pub keys: KeyMemo,
    /// Tier 3 (AI pack embeddings): by default, not installed.
    pub tier3: crate::embed::Tier3,
}

/// What a scan is busy with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    /// Signatures and hints per unit (then grouping, which reports nothing).
    #[default]
    Signatures,
    /// Best-pick's full-resolution measures ([`run_focus`]): the groups are known.
    Focus,
}

/// Progress, from the scan thread: `done` of `total` of the current phase.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub phase: Phase,
}

/// Timings of one scan.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Timings {
    /// Signatures computed on the scan thread (store misses whose thumbnail came without
    /// one) and the time spent hashing them.
    pub hashed: usize,
    pub hash_time: Duration,
    /// Store misses whose signature came with the thumbnail (made by an engine worker), and
    /// those of them read from the thumbnail cache without decoding; and the time spent looking
    /// there (every store miss, found or not).
    pub digests: usize,
    pub digests_cached: usize,
    pub digest_time: Duration,
    /// Hint reads (file heads) and their time.
    pub hint_time: Duration,
    /// Store hits.
    pub reused: usize,
    /// Decided members found again at a new path (renamed or moved) and their decisions
    /// moved there.
    pub followed: usize,
    /// Files BLAKE3-hashed for tier 0 and the time.
    pub tier0_files: usize,
    pub tier0_time: Duration,
    /// SSIM verifies run (not cached) and their time, including fetching the thumbnails.
    pub verified: usize,
    pub verify_time: Duration,
    /// Candidates, clustering and edges.
    pub group_time: Duration,
    /// Similar-shot members measured at full resolution for best-pick ([`crate::focus`]),
    /// and the time (wall clock, [`FOCUS_THREADS`] at a time).
    pub focused: usize,
    pub focus_time: Duration,
    /// Until the groups were known (everything but the focus pass).
    pub groups: Duration,
    pub total: Duration,
}

/// The scan's findings: the items (index = unit), the duplicate clusters, and the
/// similar-shot edges, regrouped live by [`Results::similar`].
#[derive(Default)]
pub struct Results {
    pub units: Vec<Unit>,
    pub items: Vec<Item>,
    pub ids: Vec<FileId>,
    /// Decision keys, one per item.
    pub keys: Vec<u64>,
    pub duplicates: Vec<Group>,
    pub edges: Vec<Edge>,
    /// What tier 3 (AI pack embeddings) did: "needs the AI pack" without it.
    pub tier3: crate::embed::Tier3Status,
    pub failed: usize,
    pub timings: Timings,
}

impl Results {
    /// Similar shots at sensitivity `s`, with the items the scan left out of similar
    /// matching (duplicate non-keepers) and the user's splits excluded.
    pub fn similar(&self, s: f32, excluded: &dyn Fn(u32, u32) -> bool) -> Vec<Group> {
        analyse::similar(&self.items, &self.edges, s, excluded)
    }

    pub fn find(&self, path: &Path) -> Option<usize> {
        self.units
            .iter()
            .position(|u| u.main == path || u.files.iter().any(|f| f == path))
    }
}

fn mtime_ns(m: &std::fs::Metadata) -> i64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos().min(i64::MAX as u128) as i64)
}

struct Pending {
    unit: usize,
    len: u64,
    mtime: i64,
}

impl Pending {
    fn id(&self, content: u64) -> FileId {
        FileId {
            len: self.len,
            content,
        }
    }
}

/// A unit's store record: its signature, the image size from the thumbnail's meta, and the
/// hints read from the file heads (timed into `t`).
fn record(u: &Unit, sig: Signature, meta: &specular_thumbs::Meta, t: &mut Timings) -> Record {
    let h1 = Instant::now();
    let mut hints = hints_of(&u.main);
    if u.sig_path != u.main {
        let other = hints_of(&u.sig_path);
        merge_hints(&mut hints, &other);
    }
    t.hint_time += h1.elapsed();
    Record {
        sig,
        hints,
        width: meta.width,
        height: meta.height,
        bit_depth: meta.bit_depth,
        blake3: None,
        focus: None,
        focus_at: Vec::new(),
    }
}

/// The digest id of [`SigDigester`]: the signature layout and algorithm version. Bump it
/// whenever [`Signature::of_rgba`] or [`Signature::to_bytes`] changes, so digests stored by
/// an older build are ignored (and recomputed) rather than mixed with new ones.
pub const SIG_DIGEST_ID: u32 = u32::from_be_bytes(*b"SIG1");

/// Computes the [`Signature`] of every class-[`SIG_CLASS`] thumbnail inside the thumbnail
/// engine, from the pixels as generated (before the cache's lossy coding), on the engine's
/// worker; see [`digester`].
#[derive(Debug)]
pub struct SigDigester;

impl Digester for SigDigester {
    fn id(&self) -> u32 {
        SIG_DIGEST_ID
    }
    fn class(&self) -> u32 {
        SIG_CLASS
    }
    fn digest(&self, px: &Pixels) -> Vec<u8> {
        Signature::of_rgba(px.width, px.height, &px.rgba)
            .to_bytes()
            .to_vec()
    }
}

/// The digester for [`specular_thumbs::EngineConfig::digester`]: every engine whose
/// class-[`SIG_CLASS`] thumbnails a scan may read (the gallery's and the scan's) should carry
/// it, so those thumbnails carry their signatures.
pub fn digester() -> Arc<dyn Digester> {
    Arc::new(SigDigester)
}

fn signature_of_digest(b: &[u8]) -> Option<Signature> {
    (b.len() == crate::sig::SIG_BYTES)
        .then(|| Signature::from_bytes(b))
        .flatten()
}

/// The signature stored with `path`'s class-[`SIG_CLASS`] cache entry, with the file's
/// content id and the entry's meta; reads the entry's head only, decodes no pixels.
fn cached_signature(
    setup: &ScanSetup,
    path: &Path,
) -> Option<(u64, specular_thumbs::Meta, Signature)> {
    let cache = setup.engine.cache()?;
    let flags = setup.engine.config().flags;
    // Kept for the engine and the content id when the entry has no signature.
    let key = setup.keys.key(path).ok()?;
    let (meta, d) = cache.get_digest(&key, SIG_CLASS, flags, SIG_DIGEST_ID)?;
    let sig = signature_of_digest(&d.bytes)?;
    setup.keys.forget(path);
    Some((key.content, meta, sig))
}

/// Fetches class-[`SIG_CLASS`] thumbnails through the engine, `paths` at a time.
fn fetch(
    setup: &ScanSetup,
    paths: &[PathBuf],
    mut got: impl FnMut(&Path, Option<&specular_thumbs::Ready>),
) {
    let mut left: HashMap<PathBuf, ()> = paths.iter().map(|p| (p.clone(), ())).collect();
    let wish: Vec<Request> = paths
        .iter()
        .map(|p| Request {
            path: p.clone(),
            class: SIG_CLASS,
        })
        .collect();
    // Already finished in the engine's memory: `want` skips those.
    for p in paths {
        if let Some((c, px, avg)) = setup.engine.mem_get(p, SIG_CLASS)
            && c == SIG_CLASS
            && let Some(meta) = setup.engine.meta(p)
        {
            let r = specular_thumbs::Ready {
                path: p.clone(),
                class: c,
                pixels: Some(px),
                average: avg,
                meta,
                sharp: true,
                from_cache: true,
                digest: None,
            };
            got(p, Some(&r));
            left.remove(p);
        }
    }
    if left.is_empty() {
        return;
    }
    setup.engine.want(
        wish.into_iter()
            .filter(|r| left.contains_key(&r.path))
            .collect(),
        0,
    );
    let mut quiet = 0;
    while !left.is_empty() && !setup.cancel.load(Ordering::Relaxed) {
        let r = setup.wake.recv_timeout(Duration::from_millis(250));
        let outs = setup.engine.take_ready();
        if outs.is_empty() && r.is_err() {
            quiet += 1;
            // The engine forgot an item (cancelled, evicted): ask again, then give up.
            if setup.engine.idle() {
                if quiet > 8 {
                    break;
                }
                setup.engine.want(
                    left.keys()
                        .map(|p| Request {
                            path: p.clone(),
                            class: SIG_CLASS,
                        })
                        .collect(),
                    0,
                );
            }
            continue;
        }
        quiet = 0;
        for o in outs {
            match o {
                Outcome::Ready(r) if r.sharp && r.class == SIG_CLASS && r.pixels.is_some() => {
                    if left.remove(&r.path).is_some() {
                        got(&r.path, Some(&r));
                    }
                }
                Outcome::Failed { path, .. } if left.remove(&path).is_some() => {
                    got(&path, None);
                }
                _ => {}
            }
        }
    }
    for p in left.keys() {
        got(p, None);
    }
    setup.engine.want(Vec::new(), 0);
}

/// Runs a scan over `units`, focus pass included ([`run_groups`] then [`run_focus`]);
/// `progress` is called now and then. Returns None if cancelled.
pub fn run(
    setup: &ScanSetup,
    units: Vec<Unit>,
    protect: &dyn Fn(&Unit) -> Option<Protect>,
    progress: &mut dyn FnMut(Progress),
) -> Option<Results> {
    let started = Instant::now();
    let mut res = run_groups(setup, units, protect, progress)?;
    let f0 = Instant::now();
    let mut input = FocusInput::of(&res);
    let mut measured = Vec::new();
    let n = run_focus(
        setup,
        &mut input,
        &FocusWish::default(),
        &mut |m| measured.extend_from_slice(m),
        progress,
    )?;
    res.apply_focus(&measured);
    res.timings.focused = n;
    res.timings.focus_time = f0.elapsed();
    res.timings.total = started.elapsed();
    Some(res)
}

/// The scan up to the groups: signatures, tier 0, verify, duplicates and similar-shot edges.
/// Best-pick then uses the thumbnails' measures until [`run_focus`] measures the similar
/// shots' members at full resolution. Returns None if cancelled.
pub fn run_groups(
    setup: &ScanSetup,
    units: Vec<Unit>,
    protect: &dyn Fn(&Unit) -> Option<Protect>,
    progress: &mut dyn FnMut(Progress),
) -> Option<Results> {
    let started = Instant::now();
    let mut t = Timings::default();
    let n = units.len();
    let mut records: Vec<Option<(FileId, Record)>> = vec![None; n];
    let mut stat: Vec<(u64, i64, u64)> = vec![(0, 0, 0); n];
    let mut todo = Vec::new();
    {
        let store = setup.store.lock().ok()?;
        for (i, u) in units.iter().enumerate() {
            let Ok(m) = std::fs::metadata(&u.sig_path) else {
                continue;
            };
            let bytes: u64 = u
                .files
                .iter()
                .map(|f| std::fs::metadata(f).map_or(0, |m| m.len()))
                .sum();
            let main_len = std::fs::metadata(&u.main).map_or(m.len(), |m| m.len());
            stat[i] = (main_len, mtime_ns(&m), bytes);
            match store.lookup(&u.sig_path, m.len(), mtime_ns(&m)) {
                Some((id, rec)) => {
                    records[i] = Some((id, rec.clone()));
                    t.reused += 1;
                }
                None => todo.push(Pending {
                    unit: i,
                    len: m.len(),
                    mtime: mtime_ns(&m),
                }),
            }
        }
    }
    let mut done = n - todo.len();
    progress(Progress {
        done,
        total: n,
        phase: Phase::Signatures,
    });
    let mut failed = 0;
    for chunk in todo.chunks(BATCH) {
        if setup.cancel.load(Ordering::Relaxed) {
            return None;
        }
        let mut fresh = Vec::new();
        // Signatures the thumbnail cache already holds: no pixels decoded.
        let mut paths: Vec<PathBuf> = Vec::new();
        for p in chunk {
            let path = &units[p.unit].sig_path;
            let d0 = Instant::now();
            let cached = cached_signature(setup, path);
            t.digest_time += d0.elapsed();
            match cached {
                Some((content, meta, sig)) => {
                    t.digests += 1;
                    t.digests_cached += 1;
                    let rec = record(&units[p.unit], sig, &meta, &mut t);
                    fresh.push((p.unit, p.len, p.mtime, p.id(content), rec));
                }
                None => paths.push(path.clone()),
            }
        }
        fetch(setup, &paths, |path, ready| {
            let Some(p) = chunk.iter().find(|p| units[p.unit].sig_path == path) else {
                return;
            };
            let Some(r) = ready else {
                failed += 1;
                return;
            };
            let Some(px) = &r.pixels else { return };
            // Made by the engine worker from the fresh pixels; else (an entry cached
            // without one, or the engine's memory) hashed here from the pixels at hand.
            let sig = match r.digest.as_deref().and_then(signature_of_digest) {
                Some(sig) => {
                    t.digests += 1;
                    sig
                }
                None => {
                    let h0 = Instant::now();
                    let sig = Signature::of_rgba(px.width, px.height, &px.rgba);
                    t.hash_time += h0.elapsed();
                    t.hashed += 1;
                    sig
                }
            };
            let rec = record(&units[p.unit], sig, &r.meta, &mut t);
            let content = setup.keys.take(path).map_or(0, |k| k.content);
            fresh.push((p.unit, p.len, p.mtime, p.id(content), rec));
        });
        if let Ok(mut store) = setup.store.lock() {
            for (u, len, mtime, id, rec) in fresh {
                store.insert(&units[u].sig_path, len, mtime, id, rec.clone());
                records[u] = Some((id, rec));
            }
        }
        done += chunk.len();
        progress(Progress {
            done,
            total: n,
            phase: Phase::Signatures,
        });
    }
    // The keys of files that failed.
    setup.keys.clear();
    if setup.cancel.load(Ordering::Relaxed) {
        return None;
    }

    // Items.
    let g0 = Instant::now();
    let mut res = Results {
        failed,
        ..Results::default()
    };
    let mut stale_at = Vec::new();
    for (i, u) in units.into_iter().enumerate() {
        let Some((id, rec)) = records[i].take() else {
            continue;
        };
        let (len, mtime, bytes) = stat[i];
        let name = u
            .main
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let kind = Kind::from_ext(
            &u.main
                .extension()
                .map(|e| e.to_string_lossy())
                .unwrap_or_default(),
        );
        let key = decision_key(id, &u.main);
        let raw = u.measures_raw();
        if raw
            && rec
                .focus_at
                .iter()
                .any(|(_, f)| f.legacy_reduced_stale(true))
        {
            stale_at.push(u.sig_path.clone());
        }
        res.items.push(Item {
            name,
            kind,
            width: rec.width,
            height: rec.height,
            bit_depth: rec.bit_depth.max(8),
            bytes,
            len,
            modified: mtime.div_euclid(1_000_000_000),
            sig: rec.sig,
            hints: rec.hints,
            protect: protect(&u),
            blake3: rec.blake3,
            gone: false,
            // Measures of an older method are taken again, as are a RAW's legacy reduced
            // ones; its legacy own measure is checked by the focus pass (`Focus::needs_check`).
            focus: rec.focus.filter(|f| !f.outdated()),
            focus_at: rec
                .focus_at
                .iter()
                .copied()
                .filter(|(_, f)| !f.outdated() && !f.legacy_reduced_stale(raw))
                .collect(),
        });
        res.ids.push(id);
        res.keys.push(key);
        res.units.push(u);
    }
    // Decisions follow renamed and moved members, then discards apply.
    if let Ok(mut s) = setup.store.lock() {
        let members: Vec<(u64, FileId, &Path)> = (0..res.keys.len())
            .map(|i| (res.keys[i], res.ids[i], res.units[i].main.as_path()))
            .collect();
        t.followed = s.follow(&members);
        for p in &stale_at {
            s.drop_focus_at(p, &|f| f.legacy_reduced_stale(true));
        }
        for (it, &k) in res.items.iter_mut().zip(&res.keys) {
            it.gone = s.discarded(k);
        }
    }

    // Tier 0 for colliding sizes.
    let t0 = Instant::now();
    let mut by_len: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, it) in res.items.iter().enumerate() {
        by_len.entry(it.len).or_default().push(i);
    }
    for idx in by_len.values().filter(|v| v.len() > 1) {
        for &i in idx {
            if res.items[i].blake3.is_none()
                && let Ok(h) = crate::file_hash(&res.units[i].main)
            {
                res.items[i].blake3 = Some(h);
                t.tier0_files += 1;
                if let Ok(mut s) = setup.store.lock() {
                    s.set_blake3(&res.units[i].sig_path, h);
                }
            }
        }
    }
    t.tier0_time = t0.elapsed();

    // Duplicates, verified.
    let index = analyse::index(&res.items);
    let split = |a: u32, b: u32| {
        setup
            .store
            .lock()
            .is_ok_and(|s| s.is_split(res.keys[a as usize], res.keys[b as usize]))
    };
    let cands = crate::group::duplicate_candidates(&res.items, &index, &split);
    let mut ssim_of: HashMap<(u32, u32), f32> = HashMap::new();
    let mut need: Vec<(u32, u32)> = Vec::new();
    if let Ok(s) = setup.store.lock() {
        for c in &cands {
            if c.tier == crate::group::Tier::Bytes {
                continue;
            }
            match s.verified(res.ids[c.a as usize], res.ids[c.b as usize]) {
                Some(v) => {
                    ssim_of.insert((c.a, c.b), v);
                }
                None => need.push((c.a, c.b)),
            }
        }
    }
    let v0 = Instant::now();
    verify_pairs(setup, &res, &need, &mut ssim_of);
    t.verified = need.len();
    t.verify_time = v0.elapsed();
    if let Ok(mut s) = setup.store.lock() {
        for &(a, b) in &need {
            if let Some(&v) = ssim_of.get(&(a, b)) {
                s.set_verified(res.ids[a as usize], res.ids[b as usize], v);
            }
        }
    }
    let mut verify = |c: &DupCandidate| ssim_of.get(&(c.a, c.b)).copied();
    res.duplicates = analyse::duplicates(&res.items, &index, &split, &mut verify);
    // Similar shots leave out the copies a duplicate group would discard.
    let mut similar_items = res.items.clone();
    for g in &res.duplicates {
        for (m, p) in g.members.iter().zip(&g.picks) {
            if !p.keep {
                similar_items[*m as usize].gone = true;
            }
        }
    }
    let sig_paths: Vec<&Path> = res.units.iter().map(|u| u.sig_path.as_path()).collect();
    let (extra, tier3) = crate::embed::tier3_pairs(&setup.tier3, &sig_paths);
    res.tier3 = tier3;
    res.edges = crate::group::similar_edges(&similar_items, &index, &extra);
    t.group_time = g0.elapsed() - t.tier0_time - t.verify_time;
    if let Ok(mut s) = setup.store.lock() {
        let _ = s.flush();
    }
    if setup.cancel.load(Ordering::Relaxed) {
        return None;
    }
    t.groups = started.elapsed();
    t.total = t.groups;
    res.timings = t;
    Some(res)
}

impl Results {
    /// Takes in measures from [`run_focus`] (in any order: the outcome is the same).
    pub fn apply_focus(&mut self, measured: &[Measured]) {
        for m in measured {
            if let Some(it) = self.items.get_mut(m.item as usize) {
                match m.side {
                    None => it.focus = Some(m.focus),
                    Some(side) => add_at(&mut it.focus_at, side, m.focus),
                }
            }
        }
    }
}

fn add_at(at: &mut Vec<(u32, Focus)>, side: u32, f: Focus) {
    at.retain(|&(s, _)| s != side);
    at.push((side, f));
    at.sort_by_key(|&(s, _)| s);
    at.truncate(crate::store::FOCUS_AT_MAX);
}

/// Files measured at once by [`run_focus`] on threads of its own; more run as idle jobs on
/// the engine's pool while it has nothing queued.
pub const FOCUS_THREADS: usize = 2;
/// How often [`run_focus`] hands over what it measured.
const FOCUS_BATCH: Duration = Duration::from_millis(100);
/// Members at sides within this factor of each other compare at full resolution as they are.
const SIDE_TOLERANCE: f32 = 1.03;
/// Smaller sides each larger member of a group is measured at, at most.
const SMALLER_SIDES: usize = 2;

/// One full-resolution measure: an item's own (`side` None) or at longer side `side`
/// ([`crate::focus::measure_at`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FocusJob {
    pub item: u32,
    pub side: Option<u32>,
}

/// A measure taken by [`run_focus`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measured {
    pub item: u32,
    pub side: Option<u32>,
    pub focus: Focus,
}

/// The items [`run_focus`] measures first, set by the UI (the visible groups' members), most
/// wanted first. Shared: setting it while the pass runs reorders what is left.
#[derive(Clone, Debug, Default)]
pub struct FocusWish(Arc<Mutex<Vec<u32>>>);

impl FocusWish {
    pub fn set(&self, items: Vec<u32>) {
        if let Ok(mut w) = self.0.lock()
            && *w != items
        {
            *w = items;
        }
    }

    /// The items wished for, most wanted first.
    pub fn get(&self) -> Vec<u32> {
        self.0.lock().map(|w| w.clone()).unwrap_or_default()
    }
}

#[derive(Clone, Debug)]
struct FocusState {
    path: PathBuf,
    long: u32,
    focus: Option<Focus>,
    /// `focus` is a legacy record to check ([`Focus::needs_check`]).
    check: bool,
    at: Vec<u32>,
}

/// What the focus pass needs of a scan's [`Results`]: the similar-shot edges' endpoints
/// (any sensitivity: the edges are the loosest), their sizes and measures so far, and the
/// connected sets of them (every group at any sensitivity lies within one).
#[derive(Clone, Debug, Default)]
pub struct FocusInput {
    items: HashMap<u32, FocusState>,
    /// Endpoints in item order.
    order: Vec<u32>,
    comps: Vec<Vec<u32>>,
}

fn root(p: &mut [usize], mut x: usize) -> usize {
    while p[x] != x {
        p[x] = p[p[x]];
        x = p[x];
    }
    x
}

impl FocusInput {
    pub fn of(res: &Results) -> FocusInput {
        let mut order: Vec<u32> = res.edges.iter().flat_map(|e| [e.a, e.b]).collect();
        order.sort_unstable();
        order.dedup();
        let items = order
            .iter()
            .map(|&i| {
                let it = &res.items[i as usize];
                let st = FocusState {
                    path: res.units[i as usize].sig_path.clone(),
                    long: it.width.max(it.height),
                    focus: it.focus,
                    check: it
                        .focus
                        .is_some_and(|f| f.needs_check(res.units[i as usize].measures_raw())),
                    at: it.focus_at.iter().map(|&(s, _)| s).collect(),
                };
                (i, st)
            })
            .collect();
        // Connected sets (union-find over the endpoints).
        let pos: HashMap<u32, usize> = order.iter().enumerate().map(|(k, &i)| (i, k)).collect();
        let mut parent: Vec<usize> = (0..order.len()).collect();
        for e in &res.edges {
            let (a, b) = (root(&mut parent, pos[&e.a]), root(&mut parent, pos[&e.b]));
            if a != b {
                parent[a.max(b)] = a.min(b);
            }
        }
        let mut by_root: HashMap<usize, Vec<u32>> = HashMap::new();
        for (k, &i) in order.iter().enumerate() {
            let r = root(&mut parent, k);
            by_root.entry(r).or_default().push(i);
        }
        let mut comps: Vec<Vec<u32>> = by_root.into_values().collect();
        comps.sort_unstable();
        FocusInput {
            items,
            order,
            comps,
        }
    }

    /// The measures still to take: every endpoint's own first; once a connected set has
    /// them, its larger members at the sides of its smaller ones (so groups of mixed sizes
    /// compare at a common scale, [`crate::pick`]). `estimate` counts members not measured
    /// yet as measured at full resolution (for the progress total).
    pub fn jobs(&self, estimate: bool) -> Vec<FocusJob> {
        let mut out: Vec<FocusJob> = self
            .order
            .iter()
            .filter(|i| self.items[i].focus.is_none() || self.items[i].check)
            .map(|&item| FocusJob { item, side: None })
            .collect();
        if !out.is_empty() && !estimate {
            return out;
        }
        let near = |a: f32, b: f32| a <= b * SIDE_TOLERANCE && b <= a * SIDE_TOLERANCE;
        for comp in &self.comps {
            // The side each member's own measure samples at.
            let side = |i: &u32| {
                let st = &self.items[i];
                match st.focus.filter(|_| !st.check) {
                    Some(f) if f.ok() && st.long > 0 => {
                        Some(st.long as f32 / f.scale.max(1) as f32)
                    }
                    None if estimate && st.long > 0 => Some(st.long as f32),
                    _ => None,
                }
            };
            let mut sides: Vec<f32> = comp.iter().filter_map(side).collect();
            sides.sort_by(f32::total_cmp);
            // Sides within the tolerance of the smallest of a run measure as one.
            let mut reps: Vec<f32> = Vec::new();
            for s in sides {
                if reps.last().is_none_or(|&r| !near(r, s)) {
                    reps.push(s);
                }
            }
            if reps.len() < 2 {
                continue;
            }
            for i in comp {
                let Some(s) = side(i) else { continue };
                let st = &self.items[i];
                for &r in reps
                    .iter()
                    .filter(|&&r| r < s && !near(r, s))
                    .take(SMALLER_SIDES)
                {
                    let at = r.round() as u32;
                    if at >= 8
                        && f64::from(st.long) / f64::from(at) <= crate::focus::MAX_REDUCTION
                        && !st.at.iter().any(|&a| near(a as f32, r))
                    {
                        out.push(FocusJob {
                            item: *i,
                            side: Some(at),
                        });
                    }
                }
            }
        }
        out
    }

    fn apply(&mut self, m: &Measured) {
        if let Some(st) = self.items.get_mut(&m.item) {
            match m.side {
                None => {
                    st.focus = Some(m.focus);
                    st.check = false;
                }
                Some(s) => st.at.push(s),
            }
        }
    }
}

type Outcome1 = (FocusJob, Option<Focus>);

/// The focus pass's shared queue.
struct Pass {
    /// The jobs (taken ones None) and the first that may be left.
    jobs: Mutex<(Vec<Option<FocusJob>>, usize)>,
    paths: HashMap<u32, PathBuf>,
    /// Legacy own measures kept if today's plan matches ([`crate::focus::keep_or_measure`]).
    legacy: HashMap<u32, Focus>,
    wish: FocusWish,
    registry: Arc<specular_decode::Registry>,
    cancel: Arc<AtomicBool>,
    tx: crossbeam_channel::Sender<Outcome1>,
    helpers: std::sync::atomic::AtomicUsize,
    /// Jobs measuring now.
    busy: std::sync::atomic::AtomicUsize,
}

impl Pass {
    /// The next job: a wished item's first, else the next in order.
    fn take(&self) -> Option<FocusJob> {
        let wish = self.wish.get();
        let mut q = self.jobs.lock().ok()?;
        let (jobs, next) = &mut *q;
        for w in wish {
            if let Some(j) = jobs[*next..]
                .iter_mut()
                .find(|j| j.is_some_and(|j| j.item == w))
            {
                return j.take();
            }
        }
        while *next < jobs.len() {
            *next += 1;
            if let Some(j) = jobs[*next - 1].take() {
                return Some(j);
            }
        }
        None
    }

    fn left(&self) -> bool {
        self.jobs
            .lock()
            .is_ok_and(|q| q.0[q.1..].iter().any(Option::is_some))
    }

    /// Measures one job; false when none is left (or cancelled).
    fn work(&self) -> bool {
        if self.cancel.load(Ordering::Relaxed) {
            return false;
        }
        let Some(j) = self.take() else {
            return false;
        };
        let legacy = self.legacy.get(&j.item).copied();
        let path = &self.paths[&j.item];
        // Files measure side by side on one thread each (the least CPU per file); the last
        // ones, with nothing left to start beside them, share the idle cores among their
        // crops (a lone iPhone DNG on 8 threads: about 4x sooner for about 1.5x the CPU).
        let busy = self.busy.fetch_add(1, Ordering::Relaxed) + 1;
        let threads = if self.left() {
            1
        } else {
            let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
            (cores / busy).clamp(1, crate::focus::MAX_CROP_THREADS)
        };
        // A decoder panic on this file leaves it unmeasured (and the pass waiting for no
        // reply that never comes would hang until cancelled).
        let f = specular_core::isolate::run(format!("focus of {}", path.display()), || {
            crate::focus::keep_or_measure(
                &self.registry,
                path,
                j.side,
                legacy,
                threads,
                &self.cancel,
            )
        })
        .unwrap_or(Some(crate::focus::Focus::UNAVAILABLE));
        self.busy.fetch_sub(1, Ordering::Relaxed);
        let _ = self.tx.send((j, f));
        f.is_some()
    }
}

/// A helper job's share of [`Pass::helpers`], given back when the job ends or is dropped.
struct Helper(Arc<Pass>);

impl Drop for Helper {
    fn drop(&mut self) {
        self.0.helpers.fetch_sub(1, Ordering::Relaxed);
    }
}

/// The focus pass: measures what [`FocusInput::jobs`] lists, the [`FocusWish`]'s items first,
/// into the store and `input`, handing each batch to `on` (about every [`FOCUS_BATCH`]) and
/// reporting [`Phase::Focus`] progress. Runs [`FOCUS_THREADS`] files at a time, plus idle
/// jobs on the engine's pool while it has nothing queued. Returns how many were measured,
/// None if cancelled (what was measured is stored and handed over).
pub fn run_focus(
    setup: &ScanSetup,
    input: &mut FocusInput,
    wish: &FocusWish,
    on: &mut dyn FnMut(&[Measured]),
    progress: &mut dyn FnMut(Progress),
) -> Option<usize> {
    let mut done = 0;
    let mut total = input.jobs(true).len();
    let mut cancelled = false;
    while !cancelled {
        let jobs = input.jobs(false);
        if jobs.is_empty() {
            break;
        }
        total = total.max(done + jobs.len());
        progress(Progress {
            done,
            total,
            phase: Phase::Focus,
        });
        let (round, stopped) = focus_round(setup, input, &jobs, wish, on, &mut |n| {
            progress(Progress {
                done: done + n,
                total,
                phase: Phase::Focus,
            })
        });
        done += round;
        cancelled = stopped;
        // What the next round adds is known now.
        total = done + input.jobs(false).len();
    }
    if let Ok(mut s) = setup.store.lock() {
        let _ = s.flush();
    }
    if cancelled || setup.cancel.load(Ordering::Relaxed) {
        return None;
    }
    if done > 0 {
        progress(Progress {
            done,
            total: done,
            phase: Phase::Focus,
        });
    }
    Some(done)
}

/// One round of [`run_focus`]: `jobs`, returning how many were measured and whether it was
/// cancelled.
fn focus_round(
    setup: &ScanSetup,
    input: &mut FocusInput,
    jobs: &[FocusJob],
    wish: &FocusWish,
    on: &mut dyn FnMut(&[Measured]),
    progress: &mut dyn FnMut(usize),
) -> (usize, bool) {
    let (tx, rx) = crossbeam_channel::unbounded();
    let pass = Arc::new(Pass {
        jobs: Mutex::new((jobs.iter().copied().map(Some).collect(), 0)),
        paths: jobs
            .iter()
            .map(|j| (j.item, input.items[&j.item].path.clone()))
            .collect(),
        legacy: jobs
            .iter()
            .filter_map(|j| {
                let st = &input.items[&j.item];
                st.focus
                    .filter(|_| st.check && j.side.is_none())
                    .map(|f| (j.item, f))
            })
            .collect(),
        wish: wish.clone(),
        registry: setup.engine.registry().clone(),
        cancel: setup.cancel.clone(),
        tx,
        helpers: Default::default(),
        busy: Default::default(),
    });
    let pool = setup.engine.pool();
    let max_helpers = pool.threads().saturating_sub(1);
    let mut got = 0;
    let mut measured = 0;
    let mut cancelled = false;
    let mut batch: Vec<Measured> = Vec::new();
    let mut last = Instant::now();
    std::thread::scope(|sc| {
        for wi in 0..FOCUS_THREADS.min(jobs.len()) {
            let p = &pass;
            specular_thread::spawn_scoped(sc, format!("sim-focus-{wi}"), move || while p.work() {});
        }
        while got < jobs.len() {
            // Idle cores lend a hand, one file per job so thumbnails never wait long.
            while pass.helpers.load(Ordering::Relaxed) < max_helpers
                && pool.queued() == 0
                && pool.running() < pool.threads()
                && pass.left()
            {
                pass.helpers.fetch_add(1, Ordering::Relaxed);
                let h = Helper(pass.clone());
                pool.submit_labeled(
                    specular_core::Priority::Idle,
                    specular_core::CancelToken::new(),
                    "focus",
                    move |_| {
                        h.0.work();
                    },
                );
            }
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok((j, Some(focus))) => {
                    got += 1;
                    batch.push(Measured {
                        item: j.item,
                        side: j.side,
                        focus,
                    });
                }
                Ok((_, None)) => {
                    got += 1;
                    cancelled = true;
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
            }
            if setup.cancel.load(Ordering::Relaxed) {
                cancelled = true;
            }
            if !batch.is_empty()
                && (last.elapsed() >= FOCUS_BATCH || got == jobs.len() || cancelled)
            {
                measured += batch.len();
                hand_over(setup, input, &mut batch, on);
                progress(measured);
                last = Instant::now();
            }
            if cancelled {
                // Helpers still queued find nothing left and end.
                if let Ok(mut q) = pass.jobs.lock() {
                    q.0.clear();
                    q.1 = 0;
                }
                break;
            }
        }
    });
    // Results that came in after the loop ended (cancelled while the threads finished).
    while let Ok((j, Some(focus))) = rx.try_recv() {
        batch.push(Measured {
            item: j.item,
            side: j.side,
            focus,
        });
    }
    measured += batch.len();
    hand_over(setup, input, &mut batch, on);
    (measured, cancelled)
}

/// Stores a batch of measures, notes them in `input` and hands them to `on`.
fn hand_over(
    setup: &ScanSetup,
    input: &mut FocusInput,
    batch: &mut Vec<Measured>,
    on: &mut dyn FnMut(&[Measured]),
) {
    if batch.is_empty() {
        return;
    }
    if let Ok(mut s) = setup.store.lock() {
        for m in batch.iter() {
            let path = &input.items[&m.item].path;
            match m.side {
                None => s.set_focus(path, m.focus),
                Some(side) => s.add_focus_at(path, side, m.focus),
            }
        }
    }
    for m in batch.iter() {
        input.apply(m);
    }
    on(batch);
    batch.clear();
}

fn verify_pairs(
    setup: &ScanSetup,
    res: &Results,
    need: &[(u32, u32)],
    out: &mut HashMap<(u32, u32), f32>,
) {
    let mut lru: VecDeque<(u32, Arc<Luma>)> = VecDeque::new();
    let mut i = 0;
    while i < need.len() {
        if setup.cancel.load(Ordering::Relaxed) {
            return;
        }
        // The next run of pairs whose thumbnails fit beside what is held.
        let mut want: Vec<u32> = Vec::new();
        let mut j = i;
        while j < need.len() {
            let (a, b) = need[j];
            for x in [a, b] {
                if !lru.iter().any(|(k, _)| *k == x) && !want.contains(&x) {
                    want.push(x);
                }
            }
            j += 1;
            if want.len() >= BATCH {
                break;
            }
        }
        let paths: Vec<PathBuf> = want
            .iter()
            .map(|&x| res.units[x as usize].sig_path.clone())
            .collect();
        fetch(setup, &paths, |path, ready| {
            let Some(k) = want
                .iter()
                .find(|&&x| res.units[x as usize].sig_path == path)
            else {
                return;
            };
            if let Some(px) = ready.and_then(|r| r.pixels.as_ref()) {
                lru.push_back((*k, Arc::new(Luma::from_rgba(px.width, px.height, &px.rgba))));
            }
        });
        for &(a, b) in &need[i..j] {
            let la = lru.iter().find(|(k, _)| *k == a).map(|x| x.1.clone());
            let lb = lru.iter().find(|(k, _)| *k == b).map(|x| x.1.clone());
            if let (Some(la), Some(lb)) = (la, lb) {
                out.insert((a, b), crate::ssim::ssim(&la, &lb));
            }
        }
        while lru.len() > VERIFY_LRU {
            lru.pop_front();
        }
        i = j;
    }
}

/// Hints of one file from its head.
pub fn hints_of(p: &Path) -> Hints {
    crate::file_head(p, HEAD).map_or_else(|_| Hints::default(), |h| hints::read_head(&h))
}

fn merge_hints(h: &mut Hints, o: &Hints) {
    if h.time_ms.is_none() || !h.subsec && o.subsec && o.time_ms.is_some() {
        h.time_ms = o.time_ms;
        h.subsec = o.subsec;
    }
    h.camera = h.camera.or(o.camera);
    h.serial = h.serial.or(o.serial);
    h.unique_id = h.unique_id.or(o.unique_id);
    h.burst = h.burst.or(o.burst);
    h.bracket |= o.bracket;
    h.gps = h.gps.or(o.gps);
    h.focal_mm = h.focal_mm.or(o.focal_mm);
    h.has_exif |= o.has_exif;
}

/// A scan engine beside the gallery's: same registry, pool and cache (so thumbnails are
/// shared both ways), two workers, a small memory budget, cache writes on.
pub fn engine(
    registry: Arc<specular_decode::Registry>,
    pool: Arc<specular_core::JobPool>,
    cache: Option<Arc<DiskCache>>,
    flags: u32,
    prefer_embedded: bool,
    keys: KeyMemo,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
) -> (Engine, crossbeam_channel::Receiver<()>) {
    let (tx, rx) = crossbeam_channel::bounded::<()>(1);
    let mut cfg = specular_thumbs::EngineConfig {
        workers: 2,
        memory_budget: 24 << 20,
        flags,
        digester: Some(digester()),
        ..Default::default()
    };
    cfg.generate.prefer_embedded = prefer_embedded;
    let e = Engine::new(
        cfg,
        registry,
        pool,
        Arc::new(keys),
        cache,
        Arc::new(move || {
            let _ = tx.try_send(());
            if let Some(w) = &wake {
                w();
            }
        }),
    );
    (e, rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_jpeg_pairs_are_one_unit() {
        let p = |s: &str| PathBuf::from(s);
        let u = units(&[
            p("/a/IMG_1.CR3"),
            p("/a/IMG_1.JPG"),
            p("/a/IMG_2.jpg"),
            p("/b/IMG_1.jpg"),
        ]);
        assert_eq!(u.len(), 3);
        assert_eq!(u[0].main, p("/a/IMG_1.CR3"));
        assert_eq!(u[0].sig_path, p("/a/IMG_1.JPG"));
        assert_eq!(u[0].files.len(), 2);
    }

    fn write_png(path: &Path, w: u32, h: u32, seed: u32) {
        let rgba: Vec<u8> = (0..w * h)
            .flat_map(|i| {
                let (x, y) = (i % w, i / w);
                let v = (x * 7 + y * 3 + seed * 41) ^ (x * y / 97 + seed);
                [v as u8, (v >> 2) as u8 ^ (y as u8), (x + seed) as u8, 255]
            })
            .collect();
        let mut out = Vec::new();
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(&rgba).unwrap();
        std::fs::write(path, out).unwrap();
    }

    fn setup(dir: &Path, store: &str, digests: bool) -> ScanSetup {
        let cache = Arc::new(DiskCache::new(dir.join("cache"), 64 << 20));
        let registry = Arc::new(specular_decode::Registry::new());
        let pool = Arc::new(specular_core::JobPool::new(2, "thumb-test"));
        let keys = KeyMemo::default();
        let (engine, wake) = engine(registry, pool, Some(cache), 0, true, keys.clone(), None);
        if !digests {
            let mut cfg = engine.config();
            cfg.digester = None;
            engine.set_config(cfg);
        }
        ScanSetup {
            engine,
            wake,
            store: Arc::new(Mutex::new(Store::open(&dir.join(store)))),
            cancel: Arc::new(AtomicBool::new(false)),
            keys,
            tier3: Default::default(),
        }
    }

    fn scan(s: &ScanSetup, paths: &[PathBuf]) -> Results {
        run(s, units(paths), &|_| None, &mut |_| {}).unwrap()
    }

    fn sigs(r: &Results) -> Vec<Signature> {
        r.items.iter().map(|i| i.sig).collect()
    }

    #[test]
    fn signatures_come_from_the_thumbnail_engine_and_its_cache() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/similar-test")
            .join(format!("scan-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let paths: Vec<PathBuf> = (0..3)
            .map(|k| {
                let p = dir.join(format!("p{k}.png"));
                write_png(&p, 600 + 40 * k, 400, k);
                p
            })
            .collect();
        // Before: no cache entries, hashed on the scan thread from the fresh pixels.
        let s = setup(&dir, "s0", false);
        let r = scan(&s, &paths);
        assert_eq!((r.timings.hashed, r.timings.digests), (3, 0));
        let fresh = sigs(&r);
        // Content ids as the files' keys give them, however the signature came; the keys the
        // scan, its engine and the ids shared are forgotten once used.
        let ids = r.ids.clone();
        for (id, u) in ids.iter().zip(&r.units) {
            let k = specular_thumbs::ThumbKey::of_file(&u.sig_path).unwrap();
            assert_eq!(
                *id,
                FileId {
                    len: k.stamp.len,
                    content: k.content
                }
            );
        }
        assert!(s.keys.is_empty());
        // Those entries carry no digest: a scan reads their (lossy) pixels as before.
        let s = setup(&dir, "s1", true);
        let r = scan(&s, &paths);
        assert_eq!((r.timings.hashed, r.timings.digests), (3, 0));
        // Generated by a digesting engine: the worker's signature, equal to the fresh hash.
        s.engine.cache().unwrap().clear().unwrap();
        let s = setup(&dir, "s2", true);
        let r = scan(&s, &paths);
        assert_eq!((r.timings.hashed, r.timings.digests), (0, 3));
        assert_eq!(r.timings.digests_cached, 0);
        assert_eq!(sigs(&r), fresh);
        assert_eq!(r.ids, ids);
        assert!(s.keys.is_empty());
        // From the cache, without decoding: the same signatures again.
        let s = setup(&dir, "s3", true);
        let r = scan(&s, &paths);
        assert_eq!((r.timings.hashed, r.timings.digests_cached), (0, 3));
        assert_eq!(sigs(&r), fresh);
        assert_eq!(s.engine.stats().from_cache, 0);
        assert_eq!(r.ids, ids);
        assert!(s.keys.is_empty());
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A renamed file keeps its decision: the next scan finds it under its new key.
    #[test]
    fn decisions_survive_a_rename() {
        use crate::group::Category;
        use crate::store::Decision;
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/similar-test")
            .join(format!("rename-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a.png"), dir.join("b.png"));
        write_png(&a, 300, 200, 1);
        write_png(&b, 300, 200, 2);
        let s = setup(&dir, "store", true);
        let r = scan(&s, &[a.clone(), b.clone()]);
        let ka = r.keys[r.units.iter().position(|u| u.main == a).unwrap()];
        s.store
            .lock()
            .unwrap()
            .decide(Category::Similar, ka, Decision::Discard);
        let a2 = dir.join("renamed.png");
        std::fs::rename(&a, &a2).unwrap();
        let r = scan(&s, &[a2.clone(), b.clone()]);
        assert_eq!(r.timings.followed, 1);
        let i = r.units.iter().position(|u| u.main == a2).unwrap();
        assert_ne!(r.keys[i], ka);
        assert!(r.items[i].gone, "the discard followed the rename");
        let st = s.store.lock().unwrap();
        assert_eq!(st.decision(Category::Similar, r.keys[i]), Decision::Discard);
        drop(st);
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }

    fn edge(a: u32, b: u32) -> Edge {
        Edge {
            a,
            b,
            visual: 0.0,
            dt: f32::NAN,
            dist: f32::NAN,
            same_body: None,
            burst: false,
            zoom: 1.0,
        }
    }

    /// Two similar-shot sets: 600, 600 and 1200 px wide (mixed sizes), and two of 300 px.
    fn focus_fixture(name: &str) -> (PathBuf, Results) {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/similar-test")
            .join(format!("{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let sizes = [(600, 400), (600, 400), (1200, 800), (300, 200), (300, 200)];
        let paths: Vec<PathBuf> = sizes
            .iter()
            .enumerate()
            .map(|(k, &(w, h))| {
                let p = dir.join(format!("f{k}.png"));
                write_png(&p, w, h, k as u32);
                p
            })
            .collect();
        let res = Results {
            units: units(&paths),
            items: sizes
                .iter()
                .map(|&(width, height)| Item {
                    width,
                    height,
                    ..Item::default()
                })
                .collect(),
            edges: vec![edge(0, 1), edge(1, 2), edge(3, 4)],
            ..Results::default()
        };
        (dir, res)
    }

    fn focus_pass(
        s: &ScanSetup,
        res: &mut Results,
        wish: &FocusWish,
    ) -> (Option<usize>, Vec<Progress>) {
        let mut input = FocusInput::of(res);
        let mut got = Vec::new();
        let mut seen = Vec::new();
        let n = run_focus(
            s,
            &mut input,
            wish,
            &mut |m| got.extend_from_slice(m),
            &mut |p| seen.push(p),
        );
        res.apply_focus(&got);
        (n, seen)
    }

    #[test]
    fn the_focus_pass_measures_mixed_sizes_and_reports_progress() {
        let (dir, mut res) = focus_fixture("focus");
        let input = FocusInput::of(&res);
        // Each member's own first; the 1200 px one at 600 once its set has those.
        let natives: Vec<FocusJob> = (0..5).map(|item| FocusJob { item, side: None }).collect();
        assert_eq!(input.jobs(false), natives);
        let mut all = natives.clone();
        all.push(FocusJob {
            item: 2,
            side: Some(600),
        });
        assert_eq!(input.jobs(true), all);

        let s = setup(&dir, "store", true);
        let (n, seen) = focus_pass(&s, &mut res, &FocusWish::default());
        assert_eq!(n, Some(6));
        assert!(seen.iter().all(|p| p.phase == Phase::Focus), "{seen:?}");
        assert!(seen.windows(2).all(|w| w[0].done <= w[1].done), "{seen:?}");
        assert_eq!(seen.first().map(|p| (p.done, p.total)), Some((0, 6)));
        assert_eq!(seen.last().map(|p| (p.done, p.total)), Some((6, 6)));
        assert!(res.items.iter().all(|it| it.focus.is_some_and(|f| f.ok())));
        let at = &res.items[2].focus_at;
        assert_eq!(at.len(), 1);
        assert!(at[0].0 == 600 && at[0].1.ok(), "{at:?}");
        assert!(res.items[0].focus_at.is_empty() && res.items[3].focus_at.is_empty());
        // Done: nothing left.
        assert!(FocusInput::of(&res).jobs(true).is_empty());
        let (n, _) = focus_pass(&s, &mut res, &FocusWish::default());
        assert_eq!(n, Some(0));

        // Another order (the UI's wish) ends the same.
        let (_, mut again) = focus_fixture("focus-wish");
        let s2 = setup(&dir, "store2", true);
        let wish = FocusWish::default();
        wish.set(vec![4, 2, 3]);
        let (n, _) = focus_pass(&s2, &mut again, &wish);
        assert_eq!(n, Some(6));
        for (a, b) in res.items.iter().zip(&again.items) {
            assert_eq!((a.focus, &a.focus_at), (b.focus, &b.focus_at));
        }
        drop((s, s2));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::remove_dir_all(dir.with_file_name(format!("focus-wish-{}", std::process::id())))
            .ok();
    }

    /// Measures stored before versions (SPSIM3) are kept, except a RAW's whose decode may have
    /// changed: those are checked and measured again where it did.
    #[test]
    fn legacy_measures_are_measured_again_only_where_the_decode_changed() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/similar-test")
            .join(format!("focus-legacy-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        // The decoders go by content: the ".arw" files are PNGs, measured as RAWs would be.
        let names = ["a.png", "b.png", "c.arw", "d.arw", "e.arw"];
        let paths: Vec<PathBuf> = names
            .iter()
            .enumerate()
            .map(|(k, n)| {
                let p = dir.join(n);
                write_png(&p, 600, 400, k as u32);
                p
            })
            .collect();
        let legacy = |scale: u32| Focus {
            scale,
            grid: [1.0; 16],
            detail: 1.0,
            noise: 0.5,
            ..Focus::default()
        };
        let stored = [
            // Not a RAW: kept as it is (any scale; at 1 here so that its group needs no
            // measures at a common side).
            Some(legacy(1)),
            None,
            // A RAW measured at full resolution: any plan decodes at 1, kept.
            Some(legacy(1)),
            // A RAW at 2 (whole decode over the pixel budget before regions) and one that
            // could not be measured: today's plan decodes at 1, so both are measured again.
            Some(legacy(2)),
            Some(Focus {
                version: 0,
                ..Focus::UNAVAILABLE
            }),
        ];
        let mut res = Results {
            units: units(&paths),
            items: stored
                .iter()
                .map(|&focus| Item {
                    width: 600,
                    height: 400,
                    focus,
                    ..Item::default()
                })
                .collect(),
            edges: vec![edge(0, 1), edge(2, 3), edge(3, 4)],
            ..Results::default()
        };
        let input = FocusInput::of(&res);
        let own = |item| FocusJob { item, side: None };
        assert_eq!(input.jobs(false), vec![own(1), own(3), own(4)]);
        let s = setup(&dir, "store", true);
        let (n, _) = focus_pass(&s, &mut res, &FocusWish::default());
        assert_eq!(n, Some(3));
        assert_eq!(res.items[0].focus, stored[0]);
        assert_eq!(res.items[2].focus, stored[2]);
        for k in [1, 3, 4] {
            let f = res.items[k].focus.unwrap();
            assert!(f.ok() && f.scale == 1 && f.decoded == 1, "{k}: {f:?}");
            assert_eq!(f.version, crate::focus::FOCUS_VERSION);
            assert_ne!(Some(f), stored[k]);
        }
        // Once measured, nothing is left to check.
        assert!(FocusInput::of(&res).jobs(true).is_empty());
        let (n, _) = focus_pass(&s, &mut res, &FocusWish::default());
        assert_eq!(n, Some(0));
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn wished_items_are_measured_first() {
        let (tx, _rx) = crossbeam_channel::unbounded();
        let wish = FocusWish::default();
        let job = |item| FocusJob { item, side: None };
        let pass = Pass {
            jobs: Mutex::new(((0..5).map(|i| Some(job(i))).collect(), 0)),
            paths: HashMap::new(),
            legacy: HashMap::new(),
            wish: wish.clone(),
            registry: Arc::new(specular_decode::Registry::new()),
            cancel: Arc::new(AtomicBool::new(false)),
            tx,
            helpers: Default::default(),
            busy: Default::default(),
        };
        assert_eq!(pass.take(), Some(job(0)));
        wish.set(vec![9, 3]);
        assert_eq!(pass.take(), Some(job(3)));
        assert_eq!(pass.take(), Some(job(1)));
        // Changed while running: what is left reorders.
        wish.set(vec![4]);
        assert_eq!(pass.take(), Some(job(4)));
        assert_eq!(pass.take(), Some(job(2)));
        assert!(!pass.left());
        assert_eq!(pass.take(), None);
    }

    #[test]
    fn a_cancelled_focus_pass_stops() {
        let (dir, mut res) = focus_fixture("focus-cancel");
        // Cancelled before: nothing measured.
        let s = setup(&dir, "store", true);
        s.cancel.store(true, Ordering::Relaxed);
        let (n, _) = focus_pass(&s, &mut res, &FocusWish::default());
        assert_eq!(n, None);
        assert!(res.items.iter().all(|it| it.focus.is_none()));
        // Cancelled once it reports: it stops early, keeping what it measured.
        let s = setup(&dir, "store2", true);
        let cancel = s.cancel.clone();
        let mut input = FocusInput::of(&res);
        let mut got = Vec::new();
        let n = run_focus(
            &s,
            &mut input,
            &FocusWish::default(),
            &mut |m| got.extend_from_slice(m),
            &mut |_| cancel.store(true, Ordering::Relaxed),
        );
        assert_eq!(n, None);
        assert!(got.len() < 6, "{got:?}");
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Stopping the signature pass (the gallery's Cancel): it returns at the next chunk of
    /// [`BATCH`] images, no groups; the signatures made so far stay in the store, so the next
    /// scan starts from them.
    #[test]
    fn a_cancelled_signature_pass_stops_within_a_chunk_and_keeps_its_signatures() {
        const N: usize = BATCH + 12;
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/similar-test")
            .join(format!("sig-cancel-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let paths: Vec<PathBuf> = (0..N)
            .map(|k| {
                let p = dir.join(format!("s{k:03}.png"));
                write_png(&p, 48, 32, k as u32);
                p
            })
            .collect();
        let s = setup(&dir, "store", true);
        let cancel = s.cancel.clone();
        let mut last = 0;
        let r = run_groups(&s, units(&paths), &|_| None, &mut |p| {
            last = p.done;
            if p.done > 0 {
                cancel.store(true, Ordering::Relaxed);
            }
        });
        assert!(r.is_none(), "no groups from a stopped pass");
        assert_eq!(last, BATCH, "stopped after the first chunk");
        let kept = s.store.lock().unwrap().len();
        assert!(kept > 0 && kept <= BATCH, "{kept} signatures kept");
        // Scanning again: the kept signatures are not made again.
        s.cancel.store(false, Ordering::Relaxed);
        let mut first = None;
        let r = run_groups(&s, units(&paths), &|_| None, &mut |p| {
            first.get_or_insert(p.done);
        })
        .expect("a full pass");
        assert_eq!(first, Some(kept), "resumed from the kept signatures");
        assert_eq!(r.items.len(), N);
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_scan_reports_its_phases() {
        let (dir, res) = focus_fixture("phases");
        let paths: Vec<PathBuf> = res.units.iter().map(|u| u.main.clone()).collect();
        let s = setup(&dir, "store", true);
        let mut seen = Vec::new();
        let r = run(&s, units(&paths), &|_| None, &mut |p| seen.push(p)).unwrap();
        assert_eq!(seen.first().map(|p| p.phase), Some(Phase::Signatures));
        let last_sig = seen
            .iter()
            .rposition(|p| p.phase == Phase::Signatures)
            .unwrap();
        assert_eq!(seen[last_sig].done, 5);
        // Any focus progress comes after the signatures, and ends complete.
        assert!(
            seen[last_sig..]
                .iter()
                .skip(1)
                .all(|p| p.phase == Phase::Focus)
        );
        if let Some(p) = seen.last().filter(|p| p.phase == Phase::Focus) {
            assert_eq!(p.done, p.total);
            assert_eq!(p.done, r.timings.focused);
        }
        assert!(r.timings.groups <= r.timings.total);
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }
}
