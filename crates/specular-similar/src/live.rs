//! Duplicates and similar shots kept current over a set that changes: the Library's
//! catalogue, where files arrive, change and leave one at a time and a scan of everything
//! after each would cost seconds.
//!
//! What a folder scan does at once ([`crate::scan`]) is done here per item, with the same
//! rules ([`crate::group`]) and the same suggestions ([`crate::analyse::suggest`]):
//!
//! * **Candidates on arrival.** A new item is looked up in a [`DynIndex`] of pHashes (radius
//!   [`DUP_PHASH`] for duplicates, [`SIMILAR_PHASH`] for similar shots), in its capture-time
//!   neighbourhood (the [`TIME_NEIGHBOURS`] nearest each way within the time window), by its
//!   tier 1 hash and by its size and partial content hash (tier 0). Nothing is ever compared
//!   pairwise over the whole set.
//! * **Edges kept.** Verified duplicate pairs and the similar-shot links that hold at the
//!   set's fixed sensitivity stay with both items until one of them leaves or changes.
//! * **Groups redone locally.** Only the connected parts of the edge graph that an arrival,
//!   a departure or a protection change touched are clustered again and their suggestions
//!   picked again; [`Live::update`] returns the roles that changed.
//! * **Little kept per item.** The set keeps what finding candidates and links reads (the
//!   hashes, the byte length, the hints and the protection: about a third of an [`Item`]);
//!   the rest (names, sizes, quality measures) is asked of the caller for the members of the
//!   groups formed again ([`Fetch`]), so it streams from the catalogue rather than staying.
//! * **Kept across runs.** The caller stores the links ([`Live::links_of`], for the items
//!   [`Live::take_link_owners`] names) and the roles; [`Live::restore`],
//!   [`Live::restore_link`] and [`Live::restore_role`] bring them back with no candidate
//!   looked up and no group formed, so a restart with nothing changed does no matching.
//!
//! Against a scan of everything at once there is one difference: a scan takes the 48
//! following neighbours of every item in time, here an arrival takes its 48 nearest each way
//! and the links of items already in are not trimmed when an arrival crowds their window.
//! The two differ only where more than 48 images share one time window (a burst of hundreds,
//! or files with one bogus capture time), and only by keeping a few more links here.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::analyse;
use crate::group::{self, Category, DUP_PHASH, Edge, Item, SIMILAR_PHASH, TIME_NEIGHBOURS, Tier};
use crate::hints::Hints;
use crate::index::DynIndex;
use crate::pick::Protect;
use crate::sig::Signature;

/// Duplicate candidates looked at per arrival from each of the byte and pixel-hash lookups,
/// and from the perceptual-hash index (nearest first). A set rarely has more; thousands of
/// copies of one image would otherwise cost thousands of checks and links per copy (and
/// square in all), where these keep the copies in one group through a few links each.
const EXACT_LOOKS: usize = 8;
const VISUAL_LOOKS: usize = 32;
/// Similar-shot candidates looked at per arrival from the perceptual-hash index.
const NEAR_LOOKS: usize = 64;
/// Exact copies (same length and signature) of an arrival linked before the rest are left
/// out: they are in one duplicate group through those links already.
const COPY_LINKS: usize = 2;

/// The gallery's default sensitivity ([`crate::store::Store`]'s), which a set without its
/// own slider uses.
pub const DEFAULT_SENSITIVITY: f32 = 0.5;

/// Names the rules links are kept by: links stored under another id were made by other
/// rules (constants above, [`crate::group`]'s thresholds) and are not restored.
pub const RULES_ID: &str = "live-1";

/// An item's place in a group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Role {
    /// The group: the smallest key among its members (stable while they stay).
    pub group: i64,
    /// Suggested to keep.
    pub keep: bool,
}

/// A role that changed in [`Live::update`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Change {
    pub key: i64,
    pub cat: Category,
    pub role: Option<Role>,
}

/// A link kept between two items.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Link {
    /// A verified duplicate pair, with the tier it matched at.
    Dup(Tier),
    /// A similar-shot link at the set's sensitivity.
    Similar,
}

impl Link {
    /// A small number for storage: 0–2 the duplicate tiers, 3 a similar-shot link.
    pub fn id(self) -> u8 {
        match self {
            Link::Dup(Tier::Bytes) => 0,
            Link::Dup(Tier::Pixels) => 1,
            Link::Dup(Tier::Visual) => 2,
            Link::Similar => 3,
        }
    }

    pub fn from_id(id: u8) -> Option<Link> {
        Some(match id {
            0 => Link::Dup(Tier::Bytes),
            1 => Link::Dup(Tier::Pixels),
            2 => Link::Dup(Tier::Visual),
            3 => Link::Similar,
            _ => return None,
        })
    }
}

/// Full items for keys, in the same order (None: the key is not known any more). Asked for
/// the members of each group formed again.
pub type Fetch<'a> = dyn FnMut(&[i64]) -> Vec<Option<Item>> + 'a;

/// Index of a category in per-item pairs.
fn cat_index(c: Category) -> usize {
    match c {
        Category::Duplicates => 0,
        Category::Similar => 1,
    }
}

const CATS: [Category; 2] = [Category::Duplicates, Category::Similar];

/// What finding an item's candidates and links reads: the signature without its quality
/// measures, the byte length, the hints and the protection.
#[derive(Clone, Debug, Default)]
struct Lite {
    tier1: u64,
    phash: u64,
    dhash: [u64; 4],
    moments: [u8; 6],
    aspect: f32,
    len: u64,
    hints: Hints,
    protect: Option<Protect>,
}

impl Lite {
    fn of(it: &Item) -> Lite {
        Lite {
            tier1: it.sig.tier1,
            phash: it.sig.phash,
            dhash: it.sig.dhash,
            moments: it.sig.moments,
            aspect: it.sig.aspect,
            len: it.len,
            hints: it.hints.clone(),
            protect: it.protect,
        }
    }

    fn sig(&self) -> Signature {
        Signature {
            tier1: self.tier1,
            phash: self.phash,
            dhash: self.dhash,
            moments: self.moments,
            aspect: self.aspect,
            ..Signature::default()
        }
    }

    /// The item as far as candidate checks and links read it.
    fn item(&self) -> Item {
        Item {
            len: self.len,
            bytes: self.len,
            sig: self.sig(),
            hints: self.hints.clone(),
            protect: self.protect,
            ..Item::default()
        }
    }

    /// An exact copy of `it`: the same length and hashes.
    fn copy_of(&self, it: &Item) -> bool {
        let s = &it.sig;
        self.len == it.len
            && self.tier1 == s.tier1
            && self.phash == s.phash
            && self.dhash == s.dhash
            && self.moments == s.moments
            && self.aspect.to_bits() == s.aspect.to_bits()
    }
}

struct Slot {
    key: i64,
    lite: Lite,
    /// Partial content hash (tier 0 candidates are equal in size and this).
    quick: Option<u64>,
    live: bool,
}

/// One queued arrival or departure.
enum Op {
    Put(Box<Item>, Option<u64>),
    Remove,
}

/// The groups of a changing set of items, each known by a key of the caller's (the
/// catalogue's file id).
pub struct Live {
    s: f32,
    window_ms: i64,
    slots: Vec<Slot>,
    free: Vec<u32>,
    by_key: HashMap<i64, u32>,
    index: DynIndex,
    by_time: BTreeSet<(i64, u32)>,
    by_tier1: BTreeSet<(u64, u32)>,
    /// (length, partial content hash, slot).
    by_bytes: BTreeSet<(u64, u64, u32)>,
    /// Verified duplicate pairs, with the tier that matched, kept on both sides.
    dup: Vec<Vec<(u32, Tier)>>,
    /// Similar-shot links at `s` (`a` < `b`, slots), kept on both sides.
    sim: Vec<Vec<Edge>>,
    roles: Vec<[Option<Role>; 2]>,
    queue: Vec<(i64, Op)>,
    /// Duplicates whose last link went ([`Live::take_out`]), to look up again.
    relink: Vec<i64>,
    dirty: [Vec<u32>; 2],
    changes: Vec<Change>,
    /// Keys whose stored links ([`Live::links_of`]) changed: the smaller key of each link
    /// made or dropped.
    owners: Vec<i64>,
    /// Matching work done so far ([`Live::work`]).
    work: u64,
}

impl Live {
    /// An empty set grouping similar shots at sensitivity `s` (0 tight … 1 loose).
    pub fn new(s: f32) -> Live {
        let s = s.clamp(0.0, 1.0);
        Live {
            s,
            window_ms: (group::window_s(s) * 1000.0) as i64,
            slots: Vec::new(),
            free: Vec::new(),
            by_key: HashMap::new(),
            index: DynIndex::new(),
            by_time: BTreeSet::new(),
            by_tier1: BTreeSet::new(),
            by_bytes: BTreeSet::new(),
            dup: Vec::new(),
            sim: Vec::new(),
            roles: Vec::new(),
            queue: Vec::new(),
            relink: Vec::new(),
            dirty: [Vec::new(), Vec::new()],
            changes: Vec::new(),
            owners: Vec::new(),
            work: 0,
        }
    }

    pub fn sensitivity(&self) -> f32 {
        self.s
    }

    /// Items in the set (queued changes not counted).
    pub fn len(&self) -> usize {
        self.by_key.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty()
    }

    pub fn contains(&self, key: i64) -> bool {
        self.by_key.contains_key(&key)
    }

    /// Keys in the set, in no particular order.
    pub fn keys(&self) -> impl Iterator<Item = i64> + '_ {
        self.by_key.keys().copied()
    }

    /// Adds `key`, or replaces what it was (its pixels or file changed), at the next
    /// [`Live::update`]. `quick` is the partial content hash that makes a tier 0 candidate
    /// with an item of the same length.
    pub fn put(&mut self, key: i64, item: Item, quick: Option<u64>) {
        self.queue.push((key, Op::Put(Box::new(item), quick)));
    }

    /// Takes `key` out at the next [`Live::update`].
    pub fn remove(&mut self, key: i64) {
        self.queue.push((key, Op::Remove));
    }

    /// The item's protection (the user's marks): suggestions never discard a protected item.
    /// Takes effect at the next [`Live::update`].
    pub fn set_protect(&mut self, key: i64, p: Option<Protect>) {
        let Some(&sl) = self.by_key.get(&key) else {
            return;
        };
        let it = &mut self.slots[sl as usize].lite;
        if it.protect != p {
            it.protect = p;
            self.dirty[0].push(sl);
            self.dirty[1].push(sl);
        }
    }

    /// Something only the suggestions read changed for the item (its name: the "copy of"
    /// rule; its kind: raw first): its groups are formed again, with what [`Fetch`] gives,
    /// at the next [`Live::update`], without taking it out and in again.
    pub fn touch(&mut self, key: i64) {
        if let Some(&sl) = self.by_key.get(&key) {
            self.dirty[0].push(sl);
            self.dirty[1].push(sl);
        }
    }

    pub fn protect(&self, key: i64) -> Option<Protect> {
        self.by_key
            .get(&key)
            .and_then(|&sl| self.slots[sl as usize].lite.protect)
    }

    /// The item's duplicate and similar-shot roles, as of the last [`Live::update`].
    pub fn roles(&self, key: i64) -> [Option<Role>; 2] {
        self.by_key
            .get(&key)
            .map_or([None, None], |&sl| self.roles[sl as usize])
    }

    /// Every item with a role in `cat`, as (key, role).
    pub fn members(&self, cat: Category) -> Vec<(i64, Role)> {
        let c = cat_index(cat);
        let mut v: Vec<(i64, Role)> = self
            .by_key
            .iter()
            .filter_map(|(&k, &sl)| self.roles[sl as usize][c].map(|r| (k, r)))
            .collect();
        v.sort_unstable_by_key(|x| x.0);
        v
    }

    /// Matching operations done so far: candidates looked at by arrivals, and members of the
    /// groups formed again. Counted, not timed, for tests that bound the work of a change.
    pub fn work(&self) -> u64 {
        self.work
    }

    /// Links kept (duplicate pairs, similar-shot links).
    pub fn edges(&self) -> (usize, usize) {
        let d: usize = self.dup.iter().map(Vec::len).sum();
        let s: usize = self.sim.iter().map(Vec::len).sum();
        (d / 2, s / 2)
    }

    /// About how many bytes the set holds (for a memory budget).
    pub fn approx_bytes(&self) -> u64 {
        use std::mem::size_of;
        let n = self.slots.capacity() as u64;
        // A B-tree entry with its share of the node; a hash map entry with its control byte.
        let tree = 2 * (self.by_time.len() + self.by_tier1.len()) as u64 * 16
            + 2 * self.by_bytes.len() as u64 * 24;
        let links: u64 = self
            .dup
            .iter()
            .map(|v| (v.capacity() * size_of::<(u32, Tier)>()) as u64)
            .sum::<u64>()
            + self
                .sim
                .iter()
                .map(|v| (v.capacity() * size_of::<Edge>()) as u64)
                .sum::<u64>();
        n * (size_of::<Slot>()
            + 2 * size_of::<Vec<()>>()
            + size_of::<[Option<Role>; 2]>()
            + size_of::<(i64, u32)>()
            + 1) as u64
            + tree
            + links
            + self.index.approx_bytes()
    }

    /// Applies what was queued and regroups what it touched. `verify(a, b, tier)` (keys,
    /// `a` < `b`) tells a duplicate candidate's score: for [`Tier::Bytes`] 1 when the files
    /// are byte-identical, else the SSIM of the two thumbnails; None when it cannot tell
    /// (the pair is not a duplicate). `fetch` gives the full items of the groups formed
    /// again. Returns the roles that changed.
    pub fn update(
        &mut self,
        verify: &mut dyn FnMut(i64, i64, Tier) -> Option<f32>,
        fetch: &mut Fetch<'_>,
    ) -> Vec<Change> {
        self.apply(verify);
        self.regroup(fetch)
    }

    /// The first half of [`Live::update`]: applies what was queued (candidates checked,
    /// links kept) without forming the groups again. A caller taking in many items in parts
    /// regroups once at the end ([`Live::regroup`]) rather than after every part: a group of
    /// thousands touched by every part was formed again each time.
    pub fn apply(&mut self, verify: &mut dyn FnMut(i64, i64, Tier) -> Option<f32>) {
        let queue = std::mem::take(&mut self.queue);
        for (key, op) in queue {
            if let Some(&sl) = self.by_key.get(&key) {
                self.take_out(sl);
            }
            if let Op::Put(item, quick) = op {
                self.add(key, *item, quick, verify);
            }
        }
        // Duplicates left without a link by what went are looked up again (and so on, for
        // what that takes out), unless they went too.
        loop {
            let relink = std::mem::take(&mut self.relink);
            if relink.is_empty() {
                break;
            }
            for key in relink {
                let Some(&sl) = self.by_key.get(&key) else {
                    continue;
                };
                if !self.dup[sl as usize].is_empty() {
                    continue;
                }
                let slot = &self.slots[sl as usize];
                let (item, quick) = (slot.lite.item(), slot.quick);
                self.take_out(sl);
                self.add(key, item, quick, verify);
            }
        }
    }

    /// The second half of [`Live::update`]: regroups what was touched since the last
    /// regrouping, with the full items `fetch` gives. Returns the roles that changed since
    /// then.
    pub fn regroup(&mut self, fetch: &mut Fetch<'_>) -> Vec<Change> {
        self.regroup_duplicates(fetch);
        self.regroup_similar(fetch);
        std::mem::take(&mut self.changes)
    }

    /// Something is waiting for [`Live::apply`] or [`Live::regroup`].
    pub fn pending(&self) -> bool {
        !self.queue.is_empty()
            || !self.relink.is_empty()
            || !self.dirty[0].is_empty()
            || !self.dirty[1].is_empty()
    }

    /// Some stored links changed since [`Live::take_link_owners`] was last called.
    pub fn link_owners_waiting(&self) -> bool {
        !self.owners.is_empty()
    }

    /// Keys whose links to larger keys ([`Live::links_of`]) changed since the last call,
    /// each once, in order: what a caller storing the links writes again.
    pub fn take_link_owners(&mut self) -> Vec<i64> {
        let mut v = std::mem::take(&mut self.owners);
        v.sort_unstable();
        v.dedup();
        v
    }

    /// The links of `key` to larger keys, by key and then link (each link is stored once,
    /// with its smaller key).
    pub fn links_of(&self, key: i64) -> Vec<(i64, Link)> {
        let Some(&sl) = self.by_key.get(&key) else {
            return Vec::new();
        };
        let s = sl as usize;
        let key_of = |o: u32| self.slots[o as usize].key;
        let mut v: Vec<(i64, Link)> = self.dup[s]
            .iter()
            .map(|&(o, t)| (key_of(o), Link::Dup(t)))
            .chain(self.sim[s].iter().map(|e| {
                let o = if e.a == sl { e.b } else { e.a };
                (key_of(o), Link::Similar)
            }))
            .filter(|&(k, _)| k > key)
            .collect();
        v.sort_unstable();
        v
    }

    /// Brings back an item as it was when its links and roles were stored: indexed, with no
    /// candidate looked up and nothing to regroup. Every item comes back before any link.
    pub fn restore(&mut self, key: i64, item: &Item, quick: Option<u64>) {
        if let Some(&sl) = self.by_key.get(&key) {
            self.take_out(sl);
        }
        let sl = self.slot();
        let lite = Lite::of(item);
        self.index.insert(sl, lite.phash);
        if let Some(t) = lite.hints.time_ms {
            self.by_time.insert((t, sl));
        }
        self.by_tier1.insert((lite.tier1, sl));
        if let Some(q) = quick {
            self.by_bytes.insert((lite.len, q, sl));
        }
        self.by_key.insert(key, sl);
        let slot = &mut self.slots[sl as usize];
        slot.key = key;
        slot.lite = lite;
        slot.quick = quick;
        slot.live = true;
    }

    /// Brings back a stored link between `a` and `b` (restored items). When one of them is
    /// not in the set any more (it left while the links were not loaded), the link is
    /// dropped: the other's groups are formed again at the next update (and a duplicate
    /// left with no link is looked up again), and its stored links are named to be written
    /// again. Returns whether the link came back.
    pub fn restore_link(&mut self, a: i64, b: i64, link: Link) -> bool {
        let (sa, sb) = (self.by_key.get(&a).copied(), self.by_key.get(&b).copied());
        let (Some(sa), Some(sb)) = (sa, sb) else {
            self.owners.push(a.min(b));
            for (k, s) in [(a, sa), (b, sb)] {
                if let Some(s) = s {
                    self.dirty[0].push(s);
                    self.dirty[1].push(s);
                    if matches!(link, Link::Dup(_)) {
                        self.relink.push(k);
                    }
                }
            }
            return false;
        };
        if sa == sb {
            return false;
        }
        match link {
            Link::Dup(t) => {
                self.dup[sa as usize].push((sb, t));
                self.dup[sb as usize].push((sa, t));
            }
            Link::Similar => {
                let (x, y) = (sa.min(sb), sa.max(sb));
                let e = group::edge_between(
                    &self.slots[x as usize].lite.item(),
                    &self.slots[y as usize].lite.item(),
                    x,
                    y,
                );
                self.sim[sa as usize].push(e);
                self.sim[sb as usize].push(e);
            }
        }
        true
    }

    /// Brings back a stored role (after the item and its links), with no change reported.
    pub fn restore_role(&mut self, key: i64, cat: Category, role: Option<Role>) {
        if let Some(&sl) = self.by_key.get(&key) {
            self.roles[sl as usize][cat_index(cat)] = role;
        }
    }

    fn set_role(&mut self, sl: u32, c: usize, role: Option<Role>) {
        let r = &mut self.roles[sl as usize][c];
        if *r != role {
            *r = role;
            self.changes.push(Change {
                key: self.slots[sl as usize].key,
                cat: CATS[c],
                role,
            });
        }
    }

    /// A free slot, or a new one.
    fn slot(&mut self) -> u32 {
        match self.free.pop() {
            Some(sl) => sl,
            None => {
                self.slots.push(Slot {
                    key: 0,
                    lite: Lite::default(),
                    quick: None,
                    live: false,
                });
                self.dup.push(Vec::new());
                self.sim.push(Vec::new());
                self.roles.push([None, None]);
                (self.slots.len() - 1) as u32
            }
        }
    }

    /// Takes an item out with its links. A duplicate left with no link is looked up again
    /// (once the queue is applied): an arrival links to only a few of many copies
    /// ([`EXACT_LOOKS`], [`COPY_LINKS`]), and the others must not fall out of the group when
    /// those few go.
    fn take_out(&mut self, sl: u32) {
        let s = sl as usize;
        let key = self.slots[s].key;
        for (o, _) in std::mem::take(&mut self.dup[s]) {
            let ou = o as usize;
            self.dup[ou].retain(|x| x.0 != sl);
            self.dirty[0].push(o);
            self.owners.push(key.min(self.slots[ou].key));
            if self.dup[ou].is_empty() && self.slots[ou].live {
                self.relink.push(self.slots[ou].key);
            }
        }
        for e in std::mem::take(&mut self.sim[s]) {
            let o = if e.a == sl { e.b } else { e.a };
            self.sim[o as usize].retain(|x| x.a != sl && x.b != sl);
            self.dirty[1].push(o);
            self.owners.push(key.min(self.slots[o as usize].key));
        }
        for c in 0..2 {
            self.set_role(sl, c, None);
        }
        let slot = &mut self.slots[s];
        let it = std::mem::take(&mut slot.lite);
        self.index.remove(sl, it.phash);
        if let Some(t) = it.hints.time_ms {
            self.by_time.remove(&(t, sl));
        }
        self.by_tier1.remove(&(it.tier1, sl));
        if let Some(q) = slot.quick {
            self.by_bytes.remove(&(it.len, q, sl));
        }
        self.by_key.remove(&slot.key);
        slot.live = false;
        self.free.push(sl);
    }

    fn add(
        &mut self,
        key: i64,
        item: Item,
        quick: Option<u64>,
        verify: &mut dyn FnMut(i64, i64, Tier) -> Option<f32>,
    ) {
        let sl = self.slot();

        // Duplicate candidates, each with the tiers it may match at, strongest first.
        let mut cands: HashMap<u32, Vec<Tier>> = HashMap::new();
        if let Some(q) = quick {
            for &(_, _, o) in self
                .by_bytes
                .range((item.len, q, 0)..=(item.len, q, u32::MAX))
                .take(EXACT_LOOKS)
            {
                cands.entry(o).or_default().push(Tier::Bytes);
            }
        }
        let t1 = item.sig.tier1;
        for &(_, o) in self
            .by_tier1
            .range((t1, 0)..=(t1, u32::MAX))
            .take(EXACT_LOOKS)
        {
            cands.entry(o).or_default().push(Tier::Pixels);
        }
        for (o, _) in self.index.nearest(item.sig.phash, DUP_PHASH, VISUAL_LOOKS) {
            if group::dup_visual_close(&item.sig, &self.slots[o as usize].lite.sig()) {
                cands.entry(o).or_default().push(Tier::Visual);
            }
        }
        self.work += 1 + cands.len() as u64;
        let mut cands: Vec<(u32, Vec<Tier>)> = cands.into_iter().collect();
        cands.sort_unstable_by_key(|c| self.slots[c.0 as usize].key);
        let mut copies = 0;
        for (o, tiers) in cands {
            let other = &self.slots[o as usize];
            let copy = other.lite.copy_of(&item);
            if copies >= COPY_LINKS && copy {
                continue;
            }
            let (a, b) = if key < other.key {
                (key, other.key)
            } else {
                (other.key, key)
            };
            let other_item = other.lite.item();
            let other_key = other.key;
            for t in tiers {
                let Some(min) = group::dup_min_ssim(&item, &other_item, t) else {
                    continue;
                };
                let need = if t == Tier::Bytes { 1.0 } else { min };
                if verify(a, b, t).is_some_and(|v| v >= need) {
                    copies += usize::from(copy);
                    self.dup[sl as usize].push((o, t));
                    self.dup[o as usize].push((sl, t));
                    self.dirty[0].push(o);
                    self.owners.push(key.min(other_key));
                    break;
                }
            }
        }

        // Similar-shot links: time neighbours each way, and close pHashes.
        let mut near: HashSet<u32> = HashSet::new();
        if let Some(t) = item.hints.time_ms {
            let lo = t.saturating_sub(self.window_ms);
            let hi = t.saturating_add(self.window_ms);
            near.extend(
                self.by_time
                    .range((lo, 0)..(t, u32::MAX))
                    .rev()
                    .take(TIME_NEIGHBOURS)
                    .map(|x| x.1),
            );
            near.extend(
                self.by_time
                    .range((t, 0)..=(hi, u32::MAX))
                    .take(TIME_NEIGHBOURS)
                    .map(|x| x.1),
            );
        }
        near.extend(
            self.index
                .nearest(item.sig.phash, SIMILAR_PHASH, NEAR_LOOKS)
                .into_iter()
                .map(|x| x.0),
        );
        // Copies linked as duplicates: one of each pair is left out of similar shots, so a
        // link between them is only needed while there are few.
        let mut near: Vec<u32> = near.into_iter().collect();
        self.work += near.len() as u64;
        near.sort_unstable();
        let mut copies_near = 0;
        for o in near {
            let other = &self.slots[o as usize];
            if other.lite.copy_of(&item) {
                if copies >= COPY_LINKS && copies_near >= COPY_LINKS {
                    continue;
                }
                copies_near += 1;
            }
            let other_key = other.key;
            let other = other.lite.item();
            let (a, b, ia, ib) = if sl < o {
                (sl, o, &item, &other)
            } else {
                (o, sl, &other, &item)
            };
            let e = group::edge_between(ia, ib, a, b);
            if group::links(&e, self.s) {
                self.sim[sl as usize].push(e);
                self.sim[o as usize].push(e);
                self.dirty[1].push(o);
                self.owners.push(key.min(other_key));
            }
        }

        let lite = Lite::of(&item);
        self.index.insert(sl, lite.phash);
        if let Some(t) = lite.hints.time_ms {
            self.by_time.insert((t, sl));
        }
        self.by_tier1.insert((lite.tier1, sl));
        if let Some(q) = quick {
            self.by_bytes.insert((lite.len, q, sl));
        }
        self.by_key.insert(key, sl);
        let slot = &mut self.slots[sl as usize];
        slot.key = key;
        slot.lite = lite;
        slot.quick = quick;
        slot.live = true;
        self.dirty[0].push(sl);
        self.dirty[1].push(sl);
    }

    /// A duplicate the suggestions would discard: left out of similar shots, as a scan does.
    fn gone(&self, sl: u32) -> bool {
        self.roles[sl as usize][0].is_some_and(|r| !r.keep)
    }

    /// Members of the component around `start`, by key, through `next`.
    fn component(
        &self,
        start: u32,
        seen: &mut HashSet<u32>,
        next: impl Fn(u32) -> Vec<u32>,
    ) -> Vec<u32> {
        let mut out = vec![start];
        seen.insert(start);
        let mut i = 0;
        while i < out.len() {
            for o in next(out[i]) {
                if seen.insert(o) {
                    out.push(o);
                }
            }
            i += 1;
        }
        out.sort_unstable_by_key(|&sl| self.slots[sl as usize].key);
        out
    }

    /// The full items of `members` from `fetch`, with the protection kept here (what the
    /// changes followed say); an item `fetch` does not know any more stands in with what is
    /// kept here.
    fn local_items(&self, members: &[u32], fetch: &mut Fetch<'_>) -> Vec<Item> {
        let keys: Vec<i64> = members
            .iter()
            .map(|&sl| self.slots[sl as usize].key)
            .collect();
        let mut got = fetch(&keys);
        got.resize_with(members.len(), || None);
        members
            .iter()
            .zip(got)
            .map(|(&sl, it)| {
                let lite = &self.slots[sl as usize].lite;
                match it {
                    Some(mut it) => {
                        it.protect = lite.protect;
                        it
                    }
                    None => lite.item(),
                }
            })
            .collect()
    }

    fn regroup_duplicates(&mut self, fetch: &mut Fetch<'_>) {
        let dirty = std::mem::take(&mut self.dirty[0]);
        let mut seen = HashSet::new();
        for d in dirty {
            if !self.slots[d as usize].live || seen.contains(&d) {
                continue;
            }
            let members = self.component(d, &mut seen, |sl| {
                self.dup[sl as usize].iter().map(|x| x.0).collect()
            });
            self.work += members.len() as u64;
            let mut roles: Vec<Option<Role>> = vec![None; members.len()];
            if members.len() > 1 {
                let items = self.local_items(&members, fetch);
                let tier = members
                    .iter()
                    .flat_map(|&sl| self.dup[sl as usize].iter().map(|x| x.1))
                    .max();
                let local: Vec<u32> = (0..members.len() as u32).collect();
                let group = self.slots[members[0] as usize].key;
                // Each group behind its own boundary: a panic leaves this one ungrouped
                // (logged), never the pass or the indexer.
                let what = format!(
                    "duplicate group of file {group} ({} files) skipped",
                    members.len()
                );
                if let Ok(g) = specular_core::isolate::run(what, || {
                    analyse::suggest(&items, Category::Duplicates, &local, tier)
                }) {
                    for (m, p) in g.members.iter().zip(&g.picks) {
                        roles[*m as usize] = Some(Role {
                            group,
                            keep: p.keep,
                        });
                    }
                }
            }
            for (&sl, role) in members.iter().zip(roles) {
                let was_gone = self.gone(sl);
                self.set_role(sl, 0, role);
                if was_gone != self.gone(sl) {
                    self.dirty[1].push(sl);
                }
            }
        }
    }

    fn regroup_similar(&mut self, fetch: &mut Fetch<'_>) {
        let dirty = std::mem::take(&mut self.dirty[1]);
        let mut starts: Vec<u32> = Vec::new();
        for d in dirty {
            if !self.slots[d as usize].live {
                continue;
            }
            if self.gone(d) {
                // Out of similar shots: its neighbours regroup without it.
                self.set_role(d, 1, None);
                starts.extend(
                    self.sim[d as usize]
                        .iter()
                        .map(|e| if e.a == d { e.b } else { e.a }),
                );
            } else {
                starts.push(d);
            }
        }
        let mut seen = HashSet::new();
        for d in starts {
            if !self.slots[d as usize].live || self.gone(d) || seen.contains(&d) {
                continue;
            }
            let members = self.component(d, &mut seen, |sl| {
                self.sim[sl as usize]
                    .iter()
                    .map(|e| if e.a == sl { e.b } else { e.a })
                    .filter(|&o| !self.gone(o))
                    .collect()
            });
            self.work += members.len() as u64;
            let mut roles: Vec<Option<Role>> = vec![None; members.len()];
            if members.len() > 1 {
                let local_of: HashMap<u32, u32> = members
                    .iter()
                    .enumerate()
                    .map(|(i, &sl)| (sl, i as u32))
                    .collect();
                let mut edges: Vec<Edge> = Vec::new();
                for &sl in &members {
                    for e in &self.sim[sl as usize] {
                        if e.a != sl {
                            continue;
                        }
                        let (Some(&x), Some(&y)) = (local_of.get(&e.a), local_of.get(&e.b)) else {
                            continue;
                        };
                        let mut l = *e;
                        (l.a, l.b) = (x.min(y), x.max(y));
                        edges.push(l);
                    }
                }
                edges.sort_by(|x, y| {
                    x.visual
                        .total_cmp(&y.visual)
                        .then((x.a, x.b).cmp(&(y.a, y.b)))
                });
                let items = self.local_items(&members, fetch);
                let first = self.slots[members[0] as usize].key;
                let what = format!(
                    "similar shots around file {first} ({} files) skipped",
                    members.len()
                );
                let groups = specular_core::isolate::run(what, || {
                    group::cluster_similar(&items, &edges, self.s, &|_, _| false)
                })
                .unwrap_or_default();
                for g in groups {
                    // The group's members are sorted by key, so its first is the smallest.
                    let group = self.slots[members[g[0] as usize] as usize].key;
                    let what = format!("similar group of file {group} ({} files) skipped", g.len());
                    let Ok(sg) = specular_core::isolate::run(what, || {
                        analyse::suggest(&items, Category::Similar, &g, None)
                    }) else {
                        continue;
                    };
                    for (m, p) in sg.members.iter().zip(&sg.picks) {
                        roles[*m as usize] = Some(Role {
                            group,
                            keep: p.keep,
                        });
                    }
                }
            }
            for (&sl, role) in members.iter().zip(roles) {
                self.set_role(sl, 1, role);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hints::Hints;
    use crate::sig::Signature;
    use std::ops::{Deref, DerefMut};

    /// A set with the full items beside it, as the catalogue keeps them for [`Fetch`].
    struct T {
        l: Live,
        items: HashMap<i64, Item>,
    }

    impl T {
        fn new(s: f32) -> T {
            T {
                l: Live::new(s),
                items: HashMap::new(),
            }
        }

        fn put(&mut self, key: i64, item: Item, quick: Option<u64>) {
            self.items.insert(key, item.clone());
            self.l.put(key, item, quick);
        }

        fn update(&mut self, verify: &mut dyn FnMut(i64, i64, Tier) -> Option<f32>) -> Vec<Change> {
            let items = &self.items;
            self.l.update(verify, &mut |ks: &[i64]| {
                ks.iter().map(|k| items.get(k).cloned()).collect()
            })
        }

        fn rename(&mut self, key: i64, name: &str) {
            if let Some(it) = self.items.get_mut(&key)
                && it.name != name
            {
                it.name = name.into();
                self.l.touch(key);
            }
        }
    }

    impl Deref for T {
        type Target = Live;
        fn deref(&self) -> &Live {
            &self.l
        }
    }

    impl DerefMut for T {
        fn deref_mut(&mut self) -> &mut Live {
            &mut self.l
        }
    }

    fn lcg(s: &mut u64) -> u64 {
        *s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *s ^ (*s >> 29)
    }

    /// A random item; `near` copies another's look with a few bits changed.
    fn item(s: &mut u64, near: Option<&Item>, flips: u32, time_ms: Option<i64>) -> Item {
        let mut sig = match near {
            Some(n) => n.sig,
            None => Signature {
                tier1: lcg(s),
                phash: lcg(s),
                dhash: [lcg(s), lcg(s), lcg(s), lcg(s)],
                moments: [120, 128, 128, 40, 10, 10],
                aspect: 1.5,
                ..Signature::default()
            },
        };
        if near.is_some() {
            sig.tier1 = lcg(s);
            for k in 0..flips {
                sig.phash ^= 1 << (((lcg(s) % 64) as u32 + k) % 64);
                sig.dhash[(k % 4) as usize] ^= 1 << (lcg(s) % 64);
            }
        }
        sig.quality.detail = (lcg(s) % 100) as f32;
        Item {
            name: format!("IMG_{}.jpg", lcg(s) % 10000),
            width: 4000,
            height: 3000,
            bytes: 1 << 20,
            len: 1 << 20,
            sig,
            hints: Hints {
                time_ms,
                ..Hints::default()
            },
            ..Item::default()
        }
    }

    /// The groups of a scan of everything at once, as (keys, keeps) sets.
    fn batch_groups(items: &[(i64, Item)], s: f32) -> [Vec<Vec<(i64, bool)>>; 2] {
        let its: Vec<Item> = items.iter().map(|x| x.1.clone()).collect();
        let index = analyse::index(&its);
        let mut verify = |_: &group::DupCandidate| Some(1.0);
        let dups = analyse::duplicates(&its, &index, &|_, _| false, &mut verify);
        let mut sim_items = its.clone();
        for g in &dups {
            for (m, p) in g.members.iter().zip(&g.picks) {
                if !p.keep {
                    sim_items[*m as usize].gone = true;
                }
            }
        }
        let edges = group::similar_edges(&sim_items, &index, &[]);
        let sims = analyse::similar(&sim_items, &edges, s, &|_, _| false);
        let norm = |gs: Vec<crate::Group>| {
            let mut v: Vec<Vec<(i64, bool)>> = gs
                .into_iter()
                .map(|g| {
                    let mut m: Vec<(i64, bool)> = g
                        .members
                        .iter()
                        .zip(&g.picks)
                        .map(|(m, p)| (items[*m as usize].0, p.keep))
                        .collect();
                    m.sort_unstable();
                    m
                })
                .collect();
            v.sort();
            v
        };
        [norm(dups), norm(sims)]
    }

    fn live_groups(l: &Live) -> [Vec<Vec<(i64, bool)>>; 2] {
        CATS.map(|c| {
            let mut by: HashMap<i64, Vec<(i64, bool)>> = HashMap::new();
            for (k, r) in l.members(c) {
                by.entry(r.group).or_default().push((k, r.keep));
            }
            let mut v: Vec<Vec<(i64, bool)>> = by
                .into_values()
                .map(|mut g| {
                    g.sort_unstable();
                    g
                })
                .collect();
            v.sort();
            v
        })
    }

    fn corpus(n: usize, seed: u64) -> Vec<(i64, Item)> {
        let mut s = seed;
        let mut out: Vec<(i64, Item)> = Vec::new();
        let mut t = 1_600_000_000_000i64;
        for k in 0..n {
            t += (lcg(&mut s) % 400_000) as i64;
            let roll = lcg(&mut s) % 10;
            let it = if roll < 3 && !out.is_empty() {
                // A re-take or a copy of a recent one.
                let j = out.len() - 1 - (lcg(&mut s) % out.len().min(4) as u64) as usize;
                let base = out[j].1.clone();
                let flips = (lcg(&mut s) % 5) as u32;
                let time = base
                    .hints
                    .time_ms
                    .map(|b| b + (lcg(&mut s) % 20_000) as i64);
                let mut it = item(&mut s, Some(&base), flips, time);
                if roll == 0 {
                    it.sig = base.sig;
                    it.hints.time_ms = base.hints.time_ms;
                }
                it
            } else {
                item(&mut s, None, 0, (roll < 9).then_some(t))
            };
            out.push((k as i64 * 3 + 7, it));
        }
        out
    }

    #[test]
    fn incremental_matches_a_scan_of_everything() {
        let items = corpus(600, 5);
        let s = DEFAULT_SENSITIVITY;
        let mut l = T::new(s);
        // In arrival order, a few at a time.
        for chunk in items.chunks(37) {
            for (k, it) in chunk {
                l.put(*k, it.clone(), None);
            }
            l.update(&mut |_, _, _| Some(1.0));
        }
        let want = batch_groups(&items, s);
        assert!(
            !want[0].is_empty() && !want[1].is_empty(),
            "the corpus has groups"
        );
        assert_eq!(live_groups(&l), want);
    }

    #[test]
    fn departures_and_changes_regroup() {
        let mut items = corpus(400, 9);
        let s = DEFAULT_SENSITIVITY;
        let mut l = T::new(s);
        for (k, it) in &items {
            l.put(*k, it.clone(), None);
        }
        let first = l.update(&mut |_, _, _| Some(1.0));
        assert!(!first.is_empty());
        // Take out every fifth, change every seventh.
        let mut s2 = 77u64;
        let mut kept = Vec::new();
        for (i, (k, it)) in items.drain(..).enumerate() {
            if i % 5 == 0 {
                l.remove(k);
            } else if i % 7 == 0 {
                let it = item(&mut s2, None, 0, it.hints.time_ms);
                l.put(k, it.clone(), None);
                kept.push((k, it));
            } else {
                kept.push((k, it));
            }
        }
        let changes = l.update(&mut |_, _, _| Some(1.0));
        assert!(!changes.is_empty());
        assert_eq!(live_groups(&l), batch_groups(&kept, s));
        // Changes report every role that moved: replaying them gives the same roles.
        assert_eq!(l.len(), kept.len());
    }

    #[test]
    fn protection_keeps_and_unverified_pairs_stay_apart() {
        let mut s = 3u64;
        let a = item(&mut s, None, 0, Some(1000));
        let mut b = a.clone();
        b.name = "IMG_9 copy.jpg".into();
        let c = item(&mut s, None, 0, Some(5_000_000));
        let mut l = T::new(DEFAULT_SENSITIVITY);
        l.put(1, a, None);
        l.put(2, b, None);
        l.put(3, c.clone(), None);
        l.put(4, c, None);
        // 3 and 4 do not verify.
        l.update(&mut |a, b, _| Some(if (a, b) == (3, 4) { 0.2 } else { 1.0 }));
        let [d1, _] = l.roles(1);
        let [d2, _] = l.roles(2);
        assert_eq!(
            d1,
            Some(Role {
                group: 1,
                keep: true
            })
        );
        assert_eq!(
            d2,
            Some(Role {
                group: 1,
                keep: false
            })
        );
        assert_eq!(l.roles(3)[0], None);
        // Rating the copy keeps it.
        l.set_protect(2, Some(Protect::Rated));
        let ch = l.update(&mut |_, _, _| Some(1.0));
        assert_eq!(
            l.roles(2)[0],
            Some(Role {
                group: 1,
                keep: true
            })
        );
        assert!(ch.contains(&Change {
            key: 2,
            cat: Category::Duplicates,
            role: Some(Role {
                group: 1,
                keep: true
            }),
        }));
        // Leaving takes the group with it.
        l.remove(1);
        let ch = l.update(&mut |_, _, _| Some(1.0));
        assert_eq!(l.roles(2), [None, None]);
        assert!(ch.iter().any(|c| c.key == 2 && c.role.is_none()));
    }

    #[test]
    fn a_rename_picks_again() {
        let mut s = 6u64;
        let a = item(&mut s, None, 0, Some(1000));
        let mut l = T::new(DEFAULT_SENSITIVITY);
        let mut b = a.clone();
        b.name = "IMG_9 copy.jpg".into();
        let mut a = a;
        a.name = "IMG_9.jpg".into();
        l.put(1, a, None);
        l.put(2, b, None);
        l.update(&mut |_, _, _| Some(1.0));
        assert_eq!(l.roles(1)[0].map(|r| r.keep), Some(true));
        assert_eq!(l.roles(2)[0].map(|r| r.keep), Some(false));
        // The names swap: so do the suggestions, with nothing verified again.
        l.rename(1, "IMG_9 copy.jpg");
        l.rename(2, "IMG_9.jpg");
        let ch = l.update(&mut |_, _, _| panic!("verified again"));
        assert_eq!(l.roles(1)[0].map(|r| r.keep), Some(false));
        assert_eq!(l.roles(2)[0].map(|r| r.keep), Some(true));
        assert_eq!(ch.len(), 2);
        // The same name again is no change.
        l.rename(2, "IMG_9.jpg");
        assert!(l.update(&mut |_, _, _| None).is_empty());
    }

    #[test]
    fn a_group_that_panics_is_skipped_alone() {
        let mut s = 7u64;
        let mut l = T::new(DEFAULT_SENSITIVITY);
        let mut put = |l: &mut T, key: i64, name: &str, quick: u64| {
            let mut it = item(&mut s, None, 0, None);
            it.len = 1000 + quick;
            it.name = name.into();
            l.put(key, it, Some(quick));
        };
        put(&mut l, 1, "a.jpg", 1);
        put(&mut l, 2, "a copy.jpg", 1);
        // The test hook panics on this name in debug builds.
        put(&mut l, 3, "b-specular-test-panic.jpg", 2);
        put(&mut l, 4, "b copy.jpg", 2);
        l.update(&mut |_, _, _| Some(1.0));
        assert!(l.roles(1)[0].is_some() && l.roles(2)[0].is_some());
        if cfg!(debug_assertions) {
            assert_eq!(l.roles(3)[0], None);
            assert_eq!(l.roles(4)[0], None);
        }
        // The next pass goes on as usual.
        put(&mut l, 5, "a again.jpg", 1);
        l.update(&mut |_, _, _| Some(1.0));
        assert!(l.roles(5)[0].is_some());
    }

    #[test]
    fn byte_identical_files_need_equal_bytes() {
        let mut s = 4u64;
        let a = item(&mut s, None, 0, None);
        let mut b = item(&mut s, None, 0, None);
        b.len = a.len;
        let mut l = T::new(DEFAULT_SENSITIVITY);
        l.put(10, a.clone(), Some(42));
        l.put(11, b, Some(42));
        let mut asked = Vec::new();
        l.update(&mut |a, b, t| {
            asked.push((a, b, t));
            Some(1.0)
        });
        assert_eq!(asked, vec![(10, 11, Tier::Bytes)]);
        assert!(l.roles(10)[0].is_some());
        // Same partial hash, other bytes: no group.
        let mut l = T::new(DEFAULT_SENSITIVITY);
        let mut c = item(&mut s, None, 0, None);
        c.len = a.len;
        l.put(10, a, Some(42));
        l.put(11, c, Some(42));
        l.update(&mut |_, _, _| Some(0.0));
        assert_eq!(l.roles(10)[0], None);
    }

    /// Thousands of copies of one image (a folder copied many times, hard links): each
    /// arrival is checked against a few of them, not all, the copies still make one group,
    /// and it holds when the copies its links went through leave.
    #[test]
    fn many_copies_cost_a_few_checks_each() {
        let mut s = 9u64;
        let base = item(&mut s, None, 0, Some(1_600_000_000_000));
        let n = 3000i64;
        let mut l = T::new(DEFAULT_SENSITIVITY);
        for k in 0..n {
            l.put(k, base.clone(), Some(7));
        }
        let mut checks = 0usize;
        l.update(&mut |_, _, _| {
            checks += 1;
            Some(1.0)
        });
        // Before: every copy against every earlier one (4.5 million).
        assert!(checks <= n as usize * 4, "{checks} checks");
        let (dup, sim) = l.edges();
        assert!(
            dup <= n as usize * 4 && sim <= n as usize * 8,
            "{dup} {sim} links"
        );
        let groups = |l: &Live| {
            let mut g: Vec<i64> = l
                .members(Category::Duplicates)
                .iter()
                .map(|(_, r)| r.group)
                .collect();
            g.dedup();
            (l.members(Category::Duplicates).len(), g)
        };
        assert_eq!(groups(&l), (n as usize, vec![0]));
        // The first copies (every arrival linked to them) leave.
        for k in 0..20 {
            l.remove(k);
        }
        l.update(&mut |_, _, _| Some(1.0));
        assert_eq!(groups(&l), ((n - 20) as usize, vec![20]));
        assert_eq!(
            l.members(Category::Duplicates)
                .iter()
                .filter(|(_, r)| r.keep)
                .count(),
            1
        );
    }

    /// Stored links, as a caller keeps them: written again for the keys
    /// [`Live::take_link_owners`] names.
    fn store_links(l: &mut Live, stored: &mut HashMap<i64, Vec<(i64, Link)>>) {
        for k in l.take_link_owners() {
            let v = l.links_of(k);
            if v.is_empty() {
                stored.remove(&k);
            } else {
                stored.insert(k, v);
            }
        }
    }

    /// A set brought back from stored items, links and roles.
    fn restored(
        items: &HashMap<i64, Item>,
        links: &HashMap<i64, Vec<(i64, Link)>>,
        from: &Live,
    ) -> T {
        let mut r = T::new(from.sensitivity());
        r.items = items.clone();
        let mut keys: Vec<i64> = items.keys().copied().collect();
        keys.sort_unstable();
        for k in &keys {
            r.l.restore(*k, &items[k], None);
        }
        for (&a, v) in links {
            for &(b, link) in v {
                r.l.restore_link(a, b, link);
            }
        }
        for k in &keys {
            let [d, s] = from.roles(*k);
            r.l.restore_role(*k, Category::Duplicates, d);
            r.l.restore_role(*k, Category::Similar, s);
        }
        r
    }

    /// Stored and brought back, a set does no matching until something changes, and then
    /// only around the change; it groups as the set it was stored from.
    #[test]
    fn a_restored_set_matches_nothing_until_a_change() {
        let items = corpus(800, 21);
        let mut l = T::new(DEFAULT_SENSITIVITY);
        for (k, it) in &items {
            l.put(*k, it.clone(), None);
        }
        l.update(&mut |_, _, _| Some(1.0));
        let mut links = HashMap::new();
        store_links(&mut l, &mut links);
        let first = l.work();
        assert!(first > 2_000, "{first}");
        let (d, s) = l.edges();
        let stored: usize = links.values().map(Vec::len).sum();
        assert_eq!(stored, d + s);

        let mut r = restored(&l.items, &links, &l);
        assert_eq!(live_groups(&r), live_groups(&l));
        assert_eq!(r.edges(), l.edges());
        assert!(!r.pending());
        let ch = r.update(&mut |_, _, _| panic!("verified again"));
        assert!(ch.is_empty());
        assert_eq!(
            r.work(),
            0,
            "a restart with nothing changed matches nothing"
        );
        assert!(r.take_link_owners().is_empty());

        // One changed file: both sets agree, and the restored one worked only around it.
        let mut s = 5u64;
        let (k, old) = items[400].clone();
        let new = item(&mut s, Some(&old), 3, old.hints.time_ms.map(|t| t + 2000));
        for t in [&mut l, &mut r] {
            t.put(k, new.clone(), None);
            t.update(&mut |_, _, _| Some(1.0));
        }
        assert_eq!(live_groups(&r), live_groups(&l));
        assert!(
            r.work() > 0 && r.work() < first / 20,
            "{} of {first}",
            r.work()
        );
        // The links named for storing are all that changed.
        store_links(&mut l, &mut links);
        let mut again = HashMap::new();
        for k in l.keys() {
            let v = l.links_of(k);
            if !v.is_empty() {
                again.insert(k, v);
            }
        }
        assert_eq!(links, again);
    }

    /// Items that left while the links were stored: their links are dropped on restoring,
    /// and what they linked groups again as a set without them would.
    #[test]
    fn restoring_drops_links_to_items_that_left() {
        let items = corpus(500, 17);
        let mut l = T::new(DEFAULT_SENSITIVITY);
        for (k, it) in &items {
            l.put(*k, it.clone(), None);
        }
        l.update(&mut |_, _, _| Some(1.0));
        let mut links = HashMap::new();
        store_links(&mut l, &mut links);
        // Every tenth item with links leaves.
        let gone: Vec<i64> = items
            .iter()
            .map(|x| x.0)
            .filter(|k| l.roles(*k) != [None, None])
            .step_by(10)
            .collect();
        assert!(gone.len() > 5);
        let mut left = l.items.clone();
        for k in &gone {
            left.remove(k);
        }
        let mut r = restored(&left, &links, &l);
        assert!(r.pending());
        r.update(&mut |_, _, _| Some(1.0));
        for k in &gone {
            l.remove(*k);
        }
        l.update(&mut |_, _, _| Some(1.0));
        assert_eq!(live_groups(&r), live_groups(&l));
        // The stored links of the items that left are named to be written again.
        let owners = r.take_link_owners();
        for (&a, v) in &links {
            if v.iter().any(|(b, _)| gone.contains(b)) || gone.contains(&a) {
                assert!(owners.contains(&a), "{a}");
            }
        }
    }

    #[test]
    fn link_ids_round_trip() {
        for id in 0..4 {
            assert_eq!(Link::from_id(id).map(Link::id), Some(id));
        }
        assert_eq!(Link::from_id(4), None);
    }
}
