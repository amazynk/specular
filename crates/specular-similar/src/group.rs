//! Duplicates (strict pairs, verified) and similar shots (clusters of a graph mixing visual,
//! time and place distance, regrouped live by a sensitivity).
//!
//! An [`Item`] is a unit: a RAW+JPEG(+XMP) pair is one item, so its halves never match each
//! other and are scored and discarded together.

use crate::hints::Hints;
use crate::index::HashIndex;
use crate::pick::Protect;
use crate::sig::Signature;

/// The two categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    Duplicates,
    Similar,
}

impl Category {
    pub fn id(self) -> &'static str {
        match self {
            Category::Duplicates => "dupes",
            Category::Similar => "similar",
        }
    }
    pub fn from_id(s: &str) -> Option<Category> {
        match s {
            "dupes" => Some(Category::Duplicates),
            "similar" => Some(Category::Similar),
            _ => None,
        }
    }
}

/// Container family, for "RAW over JPEG" and "less compression".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Kind {
    Raw,
    Jpeg,
    Heic,
    Avif,
    Jxl,
    WebP,
    Png,
    Tiff,
    Gif,
    #[default]
    Other,
}

impl Kind {
    pub fn from_ext(ext: &str) -> Kind {
        match ext.to_ascii_lowercase().as_str() {
            "jpg" | "jpeg" | "jpe" | "jfif" => Kind::Jpeg,
            "heic" | "heif" | "hif" => Kind::Heic,
            "avif" => Kind::Avif,
            "jxl" => Kind::Jxl,
            "webp" => Kind::WebP,
            "png" => Kind::Png,
            "tif" | "tiff" => Kind::Tiff,
            "gif" => Kind::Gif,
            "dng" | "cr2" | "cr3" | "crw" | "nef" | "nrw" | "arw" | "srf" | "sr2" | "orf"
            | "rw2" | "raf" | "pef" | "srw" | "x3f" | "3fr" | "fff" | "iiq" | "mos" | "mrw"
            | "erf" | "kdc" | "dcr" | "rwl" | "mef" | "ari" | "gpr" => Kind::Raw,
            _ => Kind::Other,
        }
    }

    /// Lossless containers (as usually written): a RAW, PNG, TIFF.
    pub fn lossless(self) -> bool {
        matches!(self, Kind::Raw | Kind::Png | Kind::Tiff)
    }
}

/// One unit to match.
#[derive(Clone, Debug, Default)]
pub struct Item {
    /// File name of the main file (for the "copy of" heuristics).
    pub name: String,
    pub kind: Kind,
    /// Full image size (oriented), 0 when unknown.
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    /// Bytes of the unit (all its files): what discarding it reclaims.
    pub bytes: u64,
    /// Bytes of the main file (tier 0 compares these).
    pub len: u64,
    /// Modification time, seconds since 1970.
    pub modified: i64,
    pub sig: Signature,
    pub hints: Hints,
    /// The user's own data: always kept, never suggested for discarding.
    pub protect: Option<Protect>,
    /// Tier 0, when computed (only for sizes that collide).
    pub blake3: Option<[u8; 32]>,
    /// Already discarded (reject only): left out of matching.
    pub gone: bool,
    /// Sharpness and noise at full resolution, measured for similar-shot members
    /// ([`crate::focus`]); best-pick falls back to the thumbnail's measures without it.
    pub focus: Option<crate::focus::Focus>,
    /// The same at smaller longer sides (px), for groups of mixed sizes
    /// ([`crate::store::Record::focus_at`]).
    pub focus_at: Vec<(u32, crate::focus::Focus)>,
}

impl Item {
    pub fn pixels(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    /// Looks like a derivative: an XMP DerivedFrom, an editor's Software tag, or a name like
    /// "IMG_1 copy", "IMG_1 (2)", "IMG_1-edited", "IMG_1_small".
    pub fn derivative(&self) -> bool {
        if self.hints.derived_from.is_some() || self.hints.edited_by_software {
            return true;
        }
        let stem = self
            .name
            .rsplit_once('.')
            .map_or(self.name.as_str(), |(s, _)| s)
            .to_lowercase();
        const MARKS: [&str; 10] = [
            " copy", "-copy", "_copy", "-edited", "_edited", "-export", "_export", "_small",
            "-small", "_web",
        ];
        MARKS.iter().any(|m| stem.contains(m))
            || stem.ends_with(')')
                && stem.rfind(" (").is_some_and(|i| {
                    stem[i + 2..stem.len() - 1]
                        .bytes()
                        .all(|b| b.is_ascii_digit())
                })
    }
}

/// Which tier a duplicate pair matched at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tier {
    /// Byte-identical (size + BLAKE3): no verify needed.
    Bytes,
    /// Same decoded pixels at the canonical size.
    Pixels,
    /// Perceptual hashes within the strict threshold.
    Visual,
}

/// A duplicate candidate; all but [`Tier::Bytes`] need the SSIM verify.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DupCandidate {
    pub a: u32,
    pub b: u32,
    pub tier: Tier,
    /// The SSIM a verify must reach to accept the pair (hints move it).
    pub min_ssim: f32,
}

/// pHash radius of duplicate candidates.
pub const DUP_PHASH: u32 = 7;
/// dHash (256-bit) distance limit of duplicates.
pub const DUP_DHASH: u32 = 48;
/// The default SSIM a duplicate must reach.
pub const DUP_SSIM: f32 = 0.90;

/// Capture times that tell two shots apart: both known and further apart than the clock's
/// resolution (1 ms with sub-seconds, else a second).
fn different_moments(a: &Hints, b: &Hints) -> bool {
    match (a.time_ms, b.time_ms) {
        (Some(x), Some(y)) => {
            let res = if a.subsec && b.subsec { 1 } else { 1000 };
            (x - y).abs() >= res
        }
        _ => false,
    }
}

/// The SSIM a duplicate candidate of `tier` must reach, or None when the metadata rules the
/// pair out (two capture moments or two bodies: shots of a burst look the same but are
/// similar, not duplicates). Byte-identical pairs are never ruled out.
pub fn dup_min_ssim(a: &Item, b: &Item, tier: Tier) -> Option<f32> {
    if tier == Tier::Bytes {
        return Some(DUP_SSIM);
    }
    let (ha, hb) = (&a.hints, &b.hints);
    if different_moments(ha, hb) {
        return None;
    }
    if let (Some(x), Some(y)) = (ha.serial, hb.serial)
        && x != y
    {
        return None;
    }
    let linked = |p: &Hints, q: &Hints| {
        p.document_id.is_some()
            && (q.derived_from == p.document_id || q.original_document_id == p.document_id)
    };
    if linked(ha, hb) || linked(hb, ha) || ha.unique_id.is_some() && ha.unique_id == hb.unique_id {
        // The metadata says one came from the other.
        return Some(0.80);
    }
    Some(DUP_SSIM)
}

/// Whether two signatures within [`DUP_PHASH`] are close enough everywhere else to be a
/// tier 2 duplicate candidate.
pub fn dup_visual_close(sa: &Signature, sb: &Signature) -> bool {
    sa.dhash_distance(sb) <= DUP_DHASH
        && sa.aspect_difference(sb) <= 0.05
        && sa.colour_distance(sb) <= 0.15
}

/// Duplicate candidates: byte-identical sets, equal tier 1 hashes and close tier 2 hashes,
/// minus pairs the metadata rules out (different capture moments or bodies) and pairs for
/// which `excluded` says the user split them.
pub fn duplicate_candidates(
    items: &[Item],
    index: &HashIndex,
    excluded: &dyn Fn(u32, u32) -> bool,
) -> Vec<DupCandidate> {
    let mut out = Vec::new();
    let push = |a: u32, b: u32, tier: Tier, out: &mut Vec<DupCandidate>| {
        let (ia, ib) = (&items[a as usize], &items[b as usize]);
        if ia.gone || ib.gone || excluded(a, b) {
            return;
        }
        if let Some(min_ssim) = dup_min_ssim(ia, ib, tier) {
            out.push(DupCandidate {
                a,
                b,
                tier,
                min_ssim,
            });
        }
    };
    // Tier 0: equal size and BLAKE3.
    let mut by_hash: Vec<(u64, [u8; 32], u32)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, it)| it.blake3.map(|h| (it.len, h, i as u32)))
        .collect();
    by_hash.sort_unstable();
    let mut seen = std::collections::HashSet::new();
    for w in by_hash.windows(2) {
        if w[0].0 == w[1].0 && w[0].1 == w[1].1 {
            push(w[0].2, w[1].2, Tier::Bytes, &mut out);
            seen.insert((w[0].2, w[1].2));
        }
    }
    // Tier 1: equal pixel hashes.
    let mut by_t1: Vec<(u64, u32)> = items
        .iter()
        .enumerate()
        .map(|(i, it)| (it.sig.tier1, i as u32))
        .collect();
    by_t1.sort_unstable();
    for w in by_t1.windows(2) {
        if w[0].0 == w[1].0 && !seen.contains(&(w[0].1, w[1].1)) {
            push(w[0].1, w[1].1, Tier::Pixels, &mut out);
            seen.insert((w[0].1, w[1].1));
        }
    }
    // Tier 2.
    for (a, b, _) in index.pairs(DUP_PHASH) {
        if seen.contains(&(a, b)) {
            continue;
        }
        if dup_visual_close(&items[a as usize].sig, &items[b as usize].sig) {
            push(a, b, Tier::Visual, &mut out);
        }
    }
    out
}

/// Union-find with path halving.
pub struct Sets {
    parent: Vec<u32>,
    size: Vec<u32>,
}

impl Sets {
    pub fn new(n: usize) -> Sets {
        Sets {
            parent: (0..n as u32).collect(),
            size: vec![1; n],
        }
    }
    pub fn find(&mut self, mut x: u32) -> u32 {
        while self.parent[x as usize] != x {
            let p = self.parent[self.parent[x as usize] as usize];
            self.parent[x as usize] = p;
            x = p;
        }
        x
    }
    pub fn union(&mut self, a: u32, b: u32) -> u32 {
        let (mut a, mut b) = (self.find(a), self.find(b));
        if a == b {
            return a;
        }
        if self.size[a as usize] < self.size[b as usize] {
            std::mem::swap(&mut a, &mut b);
        }
        self.parent[b as usize] = a;
        self.size[a as usize] += self.size[b as usize];
        a
    }
    /// Sets of two or more, each sorted, largest first.
    pub fn groups(&mut self) -> Vec<Vec<u32>> {
        let n = self.parent.len();
        let mut by_root: std::collections::HashMap<u32, Vec<u32>> = Default::default();
        for i in 0..n as u32 {
            let r = self.find(i);
            if self.size[r as usize] > 1 {
                by_root.entry(r).or_default().push(i);
            }
        }
        let mut g: Vec<Vec<u32>> = by_root.into_values().collect();
        g.sort_by(|a, b| b.len().cmp(&a.len()).then(a[0].cmp(&b[0])));
        g
    }
}

/// Duplicate groups from verified pairs.
pub fn cluster_pairs(n: usize, pairs: &[(u32, u32)]) -> Vec<Vec<u32>> {
    let mut s = Sets::new(n);
    for &(a, b) in pairs {
        s.union(a, b);
    }
    s.groups()
}

/// A possible similar-shot link, with what the sensitivity needs to decide it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edge {
    pub a: u32,
    pub b: u32,
    /// [`Signature::visual_distance`], or a tier 3 distance when that is closer.
    pub visual: f32,
    /// Seconds between the captures (NaN: unknown).
    pub dt: f32,
    /// Metres between the GPS positions (NaN: unknown).
    pub dist: f32,
    /// Same body (serial, else camera model), when both say.
    pub same_body: Option<bool>,
    /// Same burst id, or both from one bracketing sequence.
    pub burst: bool,
    /// Ratio of focal lengths (≥ 1; 1 when unknown).
    pub zoom: f32,
}

/// Neighbours in capture time considered per image.
pub const TIME_NEIGHBOURS: usize = 48;
/// The widest time window of the slider (s).
pub const MAX_WINDOW_S: f32 = 3600.0;
/// pHash radius of visual-only similar candidates.
pub const SIMILAR_PHASH: u32 = 11;

fn haversine_m(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (la1, lo1) = (a.0.to_radians(), a.1.to_radians());
    let (la2, lo2) = (b.0.to_radians(), b.1.to_radians());
    let h = ((la2 - la1) / 2.0).sin().powi(2)
        + la1.cos() * la2.cos() * ((lo2 - lo1) / 2.0).sin().powi(2);
    2.0 * 6_371_000.0 * h.sqrt().asin()
}

fn edge_of(items: &[Item], a: u32, b: u32) -> Edge {
    edge_between(&items[a as usize], &items[b as usize], a, b)
}

/// The possible link between items `ia` (index `a`) and `ib` (index `b`).
pub fn edge_between(ia: &Item, ib: &Item, a: u32, b: u32) -> Edge {
    let (ha, hb) = (&ia.hints, &ib.hints);
    let dt = match (ha.time_ms, hb.time_ms) {
        (Some(x), Some(y)) => (x - y).abs() as f32 / 1000.0,
        _ => f32::NAN,
    };
    let dist = match (ha.gps, hb.gps) {
        (Some(x), Some(y)) => haversine_m(x, y) as f32,
        _ => f32::NAN,
    };
    let same_body = match (ha.serial, hb.serial) {
        (Some(x), Some(y)) => Some(x == y),
        _ => match (ha.camera, hb.camera) {
            (Some(x), Some(y)) => Some(x == y),
            _ => None,
        },
    };
    let burst = ha.burst.is_some() && ha.burst == hb.burst
        || ha.bracket && hb.bracket && dt.is_finite() && dt <= 3.0;
    let zoom = match (ha.focal_mm, hb.focal_mm) {
        (Some(x), Some(y)) if x > 0.0 && y > 0.0 => (x / y).max(y / x),
        _ => 1.0,
    };
    Edge {
        a,
        b,
        visual: ia.sig.visual_distance(&ib.sig),
        dt,
        dist,
        same_body,
        burst,
        zoom,
    }
}

/// Every pair that the loosest sensitivity could link: neighbours in capture time and close
/// perceptual hashes, plus `extra` pairs (tier 3) with their own distance.
pub fn similar_edges(items: &[Item], index: &HashIndex, extra: &[(u32, u32, f32)]) -> Vec<Edge> {
    let loosest = max_threshold();
    // (a, b, tier-3 distance or NaN); sorted and merged below instead of a hash map.
    let mut pairs: Vec<(u32, u32, f32)> = Vec::new();
    // Time neighbours.
    let mut timed: Vec<(i64, u32)> = items
        .iter()
        .enumerate()
        .filter(|(_, it)| !it.gone)
        .filter_map(|(i, it)| it.hints.time_ms.map(|t| (t, i as u32)))
        .collect();
    timed.sort_unstable();
    let window_ms = (MAX_WINDOW_S * 1000.0) as i64;
    for (k, &(t, a)) in timed.iter().enumerate() {
        for &(u, b) in timed[k + 1..].iter().take(TIME_NEIGHBOURS) {
            if u - t > window_ms {
                break;
            }
            pairs.push((a.min(b), a.max(b), f32::NAN));
        }
    }
    pairs.extend(
        index
            .pairs(SIMILAR_PHASH)
            .into_iter()
            .map(|(a, b, _)| (a, b, f32::NAN)),
    );
    pairs.extend(extra.iter().map(|&(a, b, d)| (a.min(b), a.max(b), d)));
    pairs.sort_unstable_by_key(|x| (x.0, x.1));
    let mut merged: Vec<(u32, u32, f32)> = Vec::with_capacity(pairs.len());
    for p in pairs {
        match merged.last_mut() {
            Some(m) if (m.0, m.1) == (p.0, p.1) => {
                if m.2.is_nan() || p.2 < m.2 {
                    m.2 = p.2;
                }
            }
            _ => merged.push(p),
        }
    }
    let mut out: Vec<Edge> = merged
        .into_iter()
        .filter(|&(a, b, _)| a != b && !items[a as usize].gone && !items[b as usize].gone)
        .map(|(a, b, d3)| {
            let mut e = edge_of(items, a, b);
            if !d3.is_nan() {
                e.visual = e.visual.min(d3);
            }
            e
        })
        .filter(|e| e.visual <= loosest)
        .collect();
    out.sort_by(|x, y| {
        x.visual
            .total_cmp(&y.visual)
            .then((x.a, x.b).cmp(&(y.a, y.b)))
    });
    out
}

/// Visual threshold at sensitivity `s` (0 tight … 1 loose).
pub fn threshold(s: f32) -> f32 {
    0.16 + 0.30 * s.clamp(0.0, 1.0)
}

/// Time window at sensitivity `s` (s): 10 s … 1 h, geometric.
pub fn window_s(s: f32) -> f32 {
    10.0 * 360f32.powf(s.clamp(0.0, 1.0))
}

fn max_threshold() -> f32 {
    threshold(1.0) * 1.6
}

/// Whether `e` links its two images at sensitivity `s`.
pub fn links(e: &Edge, s: f32) -> bool {
    let mut t = threshold(s);
    let w = window_s(s);
    if e.dt.is_finite() {
        if e.dt > w && !e.burst {
            return false;
        }
        // Closer in time, looser on looks: bursts move.
        t *= 1.0 + 0.5 * (1.0 - (e.dt / w).min(1.0));
    } else {
        // Looks only: stricter.
        t *= 0.75;
    }
    if e.burst {
        t = t.max(threshold(s) * 1.6);
    }
    if e.same_body == Some(false) {
        t *= 0.6;
    }
    if e.dist.is_finite() && e.dist > 100.0 + 1900.0 * s {
        return false;
    }
    if e.zoom > 1.6 {
        t *= 0.75;
    }
    e.visual <= t
}

/// Similar-shot clusters at sensitivity `s`: edges strongest first, merged unless the
/// cluster would span more than three time windows (no chaining a whole day together).
/// `excluded(a, b)` is true for pairs the user split ("not similar").
pub fn cluster_similar(
    items: &[Item],
    edges: &[Edge],
    s: f32,
    excluded: &dyn Fn(u32, u32) -> bool,
) -> Vec<Vec<u32>> {
    let n = items.len();
    let mut sets = Sets::new(n);
    let mut span: Vec<Option<(i64, i64)>> = items
        .iter()
        .map(|it| it.hints.time_ms.map(|t| (t, t)))
        .collect();
    let max_span = (window_s(s) * 3.0 * 1000.0) as i64;
    for e in edges {
        if !links(e, s) || excluded(e.a, e.b) {
            continue;
        }
        let (ra, rb) = (sets.find(e.a), sets.find(e.b));
        if ra == rb {
            continue;
        }
        let merged = match (span[ra as usize], span[rb as usize]) {
            (Some(x), Some(y)) => Some((x.0.min(y.0), x.1.max(y.1))),
            (x, y) => x.or(y),
        };
        if let Some((lo, hi)) = merged
            && hi - lo > max_span
            && !e.burst
        {
            continue;
        }
        let r = sets.union(ra, rb);
        span[r as usize] = merged;
    }
    sets.groups()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derivative_names() {
        let it = |n: &str| Item {
            name: n.into(),
            ..Item::default()
        };
        assert!(it("IMG_1 copy.jpg").derivative());
        assert!(it("IMG_1 (2).jpg").derivative());
        assert!(it("IMG_1-edited.jpg").derivative());
        assert!(!it("IMG_1.jpg").derivative());
        assert!(!it("Holiday (Rome).jpg").derivative());
    }

    #[test]
    fn sensitivity_is_monotonic() {
        let e = Edge {
            a: 0,
            b: 1,
            visual: 0.3,
            dt: 40.0,
            dist: f32::NAN,
            same_body: Some(true),
            burst: false,
            zoom: 1.0,
        };
        let mut was = false;
        for k in 0..=20 {
            let now = links(&e, k as f32 / 20.0);
            assert!(!was || now, "looser must keep links");
            was = now;
        }
        assert!(was);
        assert!(!links(&e, 0.0));
    }

    #[test]
    fn union_find_groups() {
        let g = cluster_pairs(6, &[(0, 1), (1, 2), (4, 5)]);
        assert_eq!(g, vec![vec![0, 1, 2], vec![4, 5]]);
    }
}
