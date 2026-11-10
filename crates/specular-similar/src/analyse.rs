//! From items to the groups the UI shows: candidates, verify, clusters and suggestions.

use crate::group::{self, Category, DupCandidate, Edge, Item, Tier};
use crate::index::HashIndex;
use crate::pick::{self, Pick};

/// One group, with a suggestion per member.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub cat: Category,
    /// Indices into the items, best suggestion first.
    pub members: Vec<u32>,
    pub picks: Vec<Pick>,
    /// Bytes the suggested discards would free.
    pub reclaim: u64,
    /// Duplicates: the weakest tier that joined the group (Bytes: all byte-identical).
    pub tier: Option<Tier>,
}

/// Builds the group of `members` with its suggestions, best first.
pub fn suggest(items: &[Item], cat: Category, members: &[u32], tier: Option<Tier>) -> Group {
    // Test hook (debug builds only): a member named `*specular-test-panic*` panics here.
    for &m in members {
        specular_core::isolate::trip(std::path::Path::new(&items[m as usize].name));
    }
    let picks = match cat {
        Category::Duplicates => pick::pick_duplicates(items, members),
        Category::Similar => pick::pick_similar(items, members),
    };
    let mut order: Vec<usize> = (0..members.len()).collect();
    order.sort_by(|&a, &b| {
        let (pa, pb) = (&picks[a], &picks[b]);
        pb.keep.cmp(&pa.keep).then(match cat {
            Category::Duplicates => pa.score.total_cmp(&pb.score),
            Category::Similar => pb.score.total_cmp(&pa.score),
        })
    });
    let reclaim = order
        .iter()
        .filter(|&&m| !picks[m].keep)
        .map(|&m| items[members[m] as usize].bytes)
        .sum();
    Group {
        cat,
        members: order.iter().map(|&m| members[m]).collect(),
        picks: order.iter().map(|&m| picks[m]).collect(),
        reclaim,
        tier,
    }
}

/// Duplicate groups. `verify` gives the SSIM of a candidate pair (None: cannot tell, the pair
/// is dropped); byte-identical pairs need none.
pub fn duplicates(
    items: &[Item],
    index: &HashIndex,
    excluded: &dyn Fn(u32, u32) -> bool,
    verify: &mut dyn FnMut(&DupCandidate) -> Option<f32>,
) -> Vec<Group> {
    let cands = group::duplicate_candidates(items, index, excluded);
    let mut pairs = Vec::new();
    let mut tiers: std::collections::HashMap<(u32, u32), Tier> = Default::default();
    for c in &cands {
        let ok = c.tier == Tier::Bytes || verify(c).is_some_and(|s| s >= c.min_ssim);
        if ok {
            pairs.push((c.a, c.b));
            tiers.insert((c.a, c.b), c.tier);
        }
    }
    let groups = group::cluster_pairs(items.len(), &pairs);
    groups
        .into_iter()
        .filter_map(|g| {
            let tier = tiers
                .iter()
                .filter(|((a, _), _)| g.binary_search(a).is_ok())
                .map(|(_, t)| *t)
                .max();
            guarded(items, Category::Duplicates, &g, tier)
        })
        .collect()
}

/// Similar-shot groups at sensitivity `s` from precomputed [`group::similar_edges`].
pub fn similar(
    items: &[Item],
    edges: &[Edge],
    s: f32,
    excluded: &dyn Fn(u32, u32) -> bool,
) -> Vec<Group> {
    group::cluster_similar(items, edges, s, excluded)
        .into_iter()
        .filter_map(|g| guarded(items, Category::Similar, &g, None))
        .collect()
}

/// [`suggest`] behind its own boundary: a panic leaves this one group out (logged), never
/// the pass.
fn guarded(items: &[Item], cat: Category, members: &[u32], tier: Option<Tier>) -> Option<Group> {
    let what = format!("{cat:?} group of {} files skipped", members.len());
    specular_core::isolate::run(what, || suggest(items, cat, members, tier)).ok()
}

/// The perceptual hash index over the items.
pub fn index(items: &[Item]) -> HashIndex {
    HashIndex::new(items.iter().map(|it| it.sig.phash).collect())
}
