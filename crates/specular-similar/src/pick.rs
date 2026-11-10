//! Best-pick suggestions: which members of a group to keep, each with a reason the UI shows.
//!
//! Members the user has touched (rated, flagged, labelled, tagged, edited) are always kept:
//! a suggestion never discards the user's own work.

use crate::focus::Focus;
use crate::group::{Item, Kind};
use crate::hints::UserFacts;
use std::cmp::Ordering;

/// Why a member is protected from discarding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Protect {
    Rated,
    Flagged,
    Labelled,
    Tagged,
    Edited,
}

impl Protect {
    /// The first protecting fact, if any (the order is the one the reason names).
    pub fn of(f: &UserFacts) -> Option<Protect> {
        if f.rated {
            Some(Protect::Rated)
        } else if f.flagged {
            Some(Protect::Flagged)
        } else if f.labelled {
            Some(Protect::Labelled)
        } else if f.tagged {
            Some(Protect::Tagged)
        } else if f.edited {
            Some(Protect::Edited)
        } else {
            None
        }
    }
}

/// The reason of one suggestion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Reason {
    // Keep.
    Protected(Protect),
    /// Duplicates: the original (others are derivatives: exports, "copy" names).
    Original,
    /// Duplicates: the RAW (others are JPEGs or other renderings).
    RawOriginal,
    HighestResolution,
    DeeperColour,
    LeastCompressed,
    HasMetadata,
    /// Duplicates: all else equal, the oldest file.
    Oldest,
    /// Similar shots: the sharpest where the subject is.
    Sharpest,
    /// Similar shots: the one without (or with least) motion blur.
    Steadiest,
    BestExposure,
    LeastNoise,
    /// Similar shots: as good as the best (within a hair), kept too.
    AlsoGood,
    // Discard.
    /// Duplicates: an exact copy of the kept file.
    SameFile,
    Derivative,
    /// Duplicates: a rendering of the kept RAW.
    RenderingOfRaw,
    SmallerCopy,
    ShallowerColour,
    MoreCompressed,
    NoMetadata,
    NewerCopy,
    Blurrier,
    MotionBlur,
    Overexposed,
    Underexposed,
    Noisier,
    LowerResolution,
}

/// The suggestion for one member.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pick {
    pub keep: bool,
    pub reason: Reason,
    /// Similar shots: the score (0–1, higher better); duplicates: rank order (0 best).
    pub score: f32,
}

/// The ordered criteria of duplicates, best first. Each returns a value where greater is
/// better, so the comparison is lexicographic and the first difference is the reason.
fn dup_key(it: &Item) -> [f64; 7] {
    let bpp = if it.pixels() > 0 {
        it.len as f64 / it.pixels() as f64
    } else {
        0.0
    };
    [
        f64::from(u8::from(!it.derivative())),
        f64::from(u8::from(it.kind == Kind::Raw)),
        it.pixels() as f64,
        f64::from(it.bit_depth),
        // Lossless beats any lossy file; among lossy ones more bytes per pixel.
        if it.kind.lossless() { 1e9 } else { bpp },
        f64::from(u8::from(it.hints.has_exif)),
        -(it.modified as f64),
    ]
}

const DUP_KEEP: [Reason; 7] = [
    Reason::Original,
    Reason::RawOriginal,
    Reason::HighestResolution,
    Reason::DeeperColour,
    Reason::LeastCompressed,
    Reason::HasMetadata,
    Reason::Oldest,
];
const DUP_DISCARD: [Reason; 7] = [
    Reason::Derivative,
    Reason::RenderingOfRaw,
    Reason::SmallerCopy,
    Reason::ShallowerColour,
    Reason::MoreCompressed,
    Reason::NoMetadata,
    Reason::NewerCopy,
];

/// The first criterion where `a` beats `b` (with a little slack for the measured ones).
fn first_win(a: &[f64; 7], b: &[f64; 7]) -> Option<usize> {
    for k in 0..7 {
        let (x, y) = (a[k], b[k]);
        let better = match k {
            // Resolutions within 1 % and bytes per pixel within 3 % are "the same".
            2 => x > y * 1.01,
            4 => x > y * 1.03,
            _ => x > y,
        };
        if better {
            return Some(k);
        }
        let worse = match k {
            2 => y > x * 1.01,
            4 => y > x * 1.03,
            _ => y > x,
        };
        if worse {
            return None;
        }
    }
    None
}

/// Slack of the measured criteria: resolutions within 1 % and bytes per pixel within 3 % are
/// "the same" ([`first_win`]).
const SLACK: [(usize, f64); 2] = [(2, 1.01), (4, 1.03)];

/// Makes the keys of one group comparable with a total order. The slack of [`first_win`] is
/// not transitive (a chain of values each within 1 % of the next), and a sort with such a
/// comparator may panic; so each measured criterion is snapped to the best value it is "the
/// same" as, walking down from the best, and NaN (never better) becomes the worst value.
fn snap_keys(keys: &mut [[f64; 7]]) {
    for k in keys.iter_mut().flatten() {
        if k.is_nan() {
            *k = f64::NEG_INFINITY;
        }
    }
    for (c, slack) in SLACK {
        let mut order: Vec<usize> = (0..keys.len()).collect();
        order.sort_by(|&a, &b| keys[b][c].total_cmp(&keys[a][c]).then(a.cmp(&b)));
        let mut top: Option<f64> = None;
        for i in order {
            let v = keys[i][c];
            match top {
                Some(t) if t <= v * slack => keys[i][c] = t,
                _ => top = Some(v),
            }
        }
    }
}

/// Lexicographic and total (after [`snap_keys`]): greater is better.
fn cmp_key(a: &[f64; 7], b: &[f64; 7]) -> Ordering {
    a.iter()
        .zip(b)
        .map(|(x, y)| x.total_cmp(y))
        .find(|o| o.is_ne())
        .unwrap_or(Ordering::Equal)
}

/// Suggestions for a duplicate group (indices into `items`): keep the best by the ordered
/// criteria, and every protected member; discard the rest.
pub fn pick_duplicates(items: &[Item], group: &[u32]) -> Vec<Pick> {
    let mut keys: Vec<[f64; 7]> = group.iter().map(|&i| dup_key(&items[i as usize])).collect();
    snap_keys(&mut keys);
    let mut order: Vec<usize> = (0..group.len()).collect();
    // Best first by the key (ties keep the input order); protected members are kept besides.
    order.sort_by(|&x, &y| cmp_key(&keys[y], &keys[x]));
    let best = order[0];
    let mut out = vec![
        Pick {
            keep: false,
            reason: Reason::SameFile,
            score: 0.0,
        };
        group.len()
    ];
    for (rank, &m) in order.iter().enumerate() {
        let it = &items[group[m] as usize];
        out[m].score = rank as f32;
        if let Some(p) = it.protect {
            out[m].keep = true;
            out[m].reason = Reason::Protected(p);
            continue;
        }
        if m == best {
            out[m].keep = true;
            // What sets it apart from the runner-up.
            out[m].reason = order
                .get(1)
                .and_then(|&r| first_win(&keys[m], &keys[r]))
                .map_or(Reason::Oldest, |k| DUP_KEEP[k]);
            continue;
        }
        let kept = &items[group[best] as usize];
        out[m].reason = if kept.blake3.is_some() && kept.blake3 == it.blake3 {
            Reason::SameFile
        } else {
            first_win(&keys[best], &keys[m]).map_or(Reason::NewerCopy, |k| DUP_DISCARD[k])
        };
    }
    out
}

/// What best-pick compares of each member, all on one footing: the full-resolution crops
/// ([`crate::focus::Focus`]) when every member has them at a comparable sampling, else the
/// signature thumbnail's measures.
struct Measures {
    /// Detail per oriented 4×4 cell.
    grid: [f32; 16],
    /// Detail over the frame.
    detail: f32,
    anisotropy: f32,
    /// Noise sigma; `None` when it cannot be told apart at this footing (the thumbnail of a
    /// large image: the reduction averages the noise away, and blur lowers what is left).
    noise: Option<f32>,
    /// From the crops.
    fine: bool,
}

/// The longest side above which the thumbnail's noise estimate is not compared: past 2:1 the
/// reduction averages most of the noise away, and what is left measures blur as much.
const THUMB_NOISE_MAX_SIDE: u32 = 2 * crate::sig::SIG_CLASS;

fn measures(items: &[Item], group: &[u32]) -> Vec<Measures> {
    let foci = fine_foci(items, group);
    let fine = foci.iter().all(Option::is_some);
    group
        .iter()
        .zip(&foci)
        .map(|(&i, f)| {
            let it = &items[i as usize];
            match f {
                Some(f) if fine => Measures {
                    grid: f.grid,
                    detail: f.detail,
                    anisotropy: f.anisotropy,
                    noise: Some(f.noise),
                    fine: true,
                },
                _ => {
                    let q = &it.sig.quality;
                    let long = it.width.max(it.height);
                    Measures {
                        grid: q.grid,
                        detail: q.detail,
                        anisotropy: q.anisotropy,
                        noise: (long == 0 || long <= THUMB_NOISE_MAX_SIDE).then_some(q.noise),
                        fine: false,
                    }
                }
            }
        })
        .collect()
}

/// The full-resolution measures of a group that compare, one per member, or none: the crops
/// must cover the same share of each frame at the same sampling (within 3 %). Members larger
/// than the smallest use their measure at its side ([`Item::focus_at`]) when there is one.
fn fine_foci(items: &[Item], group: &[u32]) -> Vec<Option<Focus>> {
    let none = vec![None; group.len()];
    let native: Vec<Focus> = match group
        .iter()
        .map(|&i| items[i as usize].focus.filter(|f| f.ok()))
        .collect::<Option<Vec<_>>>()
    {
        Some(f) => f,
        None => return none,
    };
    let long = |m: usize| {
        let it = &items[group[m] as usize];
        it.width.max(it.height)
    };
    if (0..group.len()).any(|m| long(m) == 0) {
        // Sizes unknown: the same measuring scale at least.
        return if native.iter().all(|f| f.scale == native[0].scale) {
            native.into_iter().map(Some).collect()
        } else {
            none
        };
    }
    let sides: Vec<f32> = (0..group.len())
        .map(|m| long(m) as f32 / native[m].scale.max(1) as f32)
        .collect();
    let target = sides.iter().copied().fold(f32::MAX, f32::min);
    let near = |s: f32| s <= target * 1.03 && s * 1.03 >= target;
    let chosen: Option<Vec<Option<Focus>>> = (0..group.len())
        .map(|m| {
            if near(sides[m]) {
                return Some(Some(native[m]));
            }
            items[group[m] as usize]
                .focus_at
                .iter()
                .find(|(s, f)| f.ok() && near(*s as f32))
                .map(|&(_, f)| Some(f))
        })
        .collect();
    chosen.unwrap_or(none)
}

/// The 4×4 cell the group's subject is in: where detail peaks, agreed over the members, with
/// a mild preference for the centre (photographers frame the subject inward).
pub fn subject_cell(items: &[Item], group: &[u32]) -> usize {
    cell_of(&measures(items, group))
}

fn cell_of(ms: &[Measures]) -> usize {
    let mut votes = [0f32; 16];
    for m in ms {
        let peak = m.grid.iter().copied().fold(1e-6f32, f32::max);
        for (c, v) in votes.iter_mut().enumerate() {
            *v += m.grid[c] / peak;
        }
    }
    let centre = |c: usize| {
        let (x, y) = (c % 4, c / 4);
        if (1..=2).contains(&x) && (1..=2).contains(&y) {
            1.15
        } else {
            1.0
        }
    };
    (0..16)
        .max_by(|&a, &b| (votes[a] * centre(a)).total_cmp(&(votes[b] * centre(b))))
        .unwrap_or(5)
}

/// Weights of the similar-shot score components.
const W: [f32; 5] = [0.50, 0.15, 0.15, 0.10, 0.10];

/// A member this much less sharp than the sharpest (sharpness component) gains nothing on
/// noise: blur smooths noise away, so it would otherwise buy back what it loses.
const NOISE_GATE: f32 = 0.10;

/// Component scores (0–1, greater better): subject sharpness, steadiness, exposure, low
/// noise, resolution — each relative to the group.
fn components(items: &[Item], group: &[u32], ms: &[Measures], cell: usize) -> Vec<[f32; 5]> {
    let fine = ms.iter().all(|m| m.fine);
    let max_sharp = ms.iter().map(|m| m.grid[cell]).fold(1e-6, f32::max);
    let max_detail = ms.iter().map(|m| m.detail).fold(1e-6, f32::max);
    let min_aniso = ms.iter().map(|m| m.anisotropy).fold(1.0, f32::min);
    let all_noise = ms.iter().all(|m| m.noise.is_some());
    let max_noise = ms.iter().filter_map(|m| m.noise).fold(1e-6, f32::max);
    let max_px = group
        .iter()
        .map(|&i| items[i as usize].pixels() as f32)
        .fold(1.0, f32::max);
    let mut out: Vec<[f32; 5]> = group
        .iter()
        .zip(ms)
        .map(|(&i, m)| {
            let q = &items[i as usize].sig.quality;
            let sharp = if fine {
                // The subject first, the frame as a whole too (a subject cell that misses
                // the subject in one member still counts the rest).
                0.75 * m.grid[cell] / max_sharp + 0.25 * m.detail / max_detail
            } else {
                m.grid[cell] / max_sharp
            };
            // Anisotropy beyond the steadiest member's is smear.
            let steady = (1.0 - 2.5 * (m.anisotropy - min_aniso).max(0.0)).clamp(0.0, 1.0);
            let clip = (4.0 * (q.clip_lo + q.clip_hi)).min(1.0);
            let level = (2.0 * (q.mean - 0.46).abs()).min(1.0);
            let exposure = 1.0 - 0.6 * clip - 0.4 * level;
            let noise = match m.noise {
                Some(n) if all_noise => 1.0 - n / max_noise,
                // Not comparable: the same for everyone.
                _ => 1.0,
            };
            let px = items[i as usize].pixels() as f32;
            let res = if px > 0.0 { px / max_px } else { 1.0 };
            [sharp, steady, exposure, noise, res]
        })
        .collect();
    // Blur never buys noise points: the clearly blurrier score at most the sharp ones' worst.
    let top = out.iter().map(|c| c[0]).fold(0.0, f32::max);
    let sharp_floor = out
        .iter()
        .filter(|c| c[0] >= top - NOISE_GATE)
        .map(|c| c[3])
        .fold(f32::MAX, f32::min);
    for c in &mut out {
        if c[0] < top - NOISE_GATE {
            c[3] = c[3].min(sharp_floor);
        }
    }
    out
}

/// Suggestions for a group of similar shots: keep the best by sharpness on the subject, then
/// motion blur, exposure, noise and resolution (plus any within a hair of it, and every
/// protected member); discard the rest, naming what each loses on most.
pub fn pick_similar(items: &[Item], group: &[u32]) -> Vec<Pick> {
    let ms = measures(items, group);
    let cell = cell_of(&ms);
    let comps = components(items, group, &ms, cell);
    let scores: Vec<f32> = comps
        .iter()
        .map(|c| c.iter().zip(W).map(|(v, w)| v * w).sum())
        .collect();
    let best = (0..group.len())
        .max_by(|&a, &b| scores[a].total_cmp(&scores[b]).then(b.cmp(&a)))
        .unwrap_or(0);
    let runner = (0..group.len())
        .filter(|&m| m != best)
        .max_by(|&a, &b| scores[a].total_cmp(&scores[b]));
    const KEEP: [Reason; 5] = [
        Reason::Sharpest,
        Reason::Steadiest,
        Reason::BestExposure,
        Reason::LeastNoise,
        Reason::HighestResolution,
    ];
    let biggest = |a: usize, b: usize| {
        (0..5)
            .max_by(|&x, &y| {
                let dx = W[x] * (comps[a][x] - comps[b][x]);
                let dy = W[y] * (comps[a][y] - comps[b][y]);
                dx.total_cmp(&dy)
            })
            .unwrap_or(0)
    };
    (0..group.len())
        .map(|m| {
            let it = &items[group[m] as usize];
            let score = scores[m];
            if let Some(p) = it.protect {
                return Pick {
                    keep: true,
                    reason: Reason::Protected(p),
                    score,
                };
            }
            if m == best {
                let reason = runner.map_or(Reason::Sharpest, |r| KEEP[biggest(m, r)]);
                return Pick {
                    keep: true,
                    reason,
                    score,
                };
            }
            if scores[best] - score <= 0.02 {
                return Pick {
                    keep: true,
                    reason: Reason::AlsoGood,
                    score,
                };
            }
            let k = biggest(best, m);
            let q = &it.sig.quality;
            let reason = match k {
                0 => Reason::Blurrier,
                1 => Reason::MotionBlur,
                2 if q.clip_hi > q.clip_lo || q.mean > 0.5 => Reason::Overexposed,
                2 => Reason::Underexposed,
                3 => Reason::Noisier,
                _ => Reason::LowerResolution,
            };
            Pick {
                keep: false,
                reason,
                score,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::focus::Focus;
    use crate::sig::{Quality, Signature};

    fn item(name: &str, kind: Kind, w: u32, len: u64) -> Item {
        Item {
            name: name.into(),
            kind,
            width: w,
            height: w * 2 / 3,
            bit_depth: 8,
            len,
            bytes: len,
            ..Item::default()
        }
    }

    /// Near-equal and NaN criteria: the snapped keys are totally ordered (antisymmetric and
    /// transitive), so the sort cannot panic on a chain of values each within the slack.
    mod total_order {
        use super::*;
        use proptest::prelude::*;

        fn value() -> impl Strategy<Value = f64> {
            prop_oneof![
                Just(f64::NAN),
                Just(f64::INFINITY),
                Just(0.0),
                // A chain: 1000 px stepped by up to 1.2 % (each within the slack of the next).
                (0u32..60).prop_map(|i| 1000.0 * 1.006f64.powi(i as i32)),
                -1e6f64..1e6,
            ]
        }

        fn keys() -> impl Strategy<Value = Vec<[f64; 7]>> {
            proptest::collection::vec(proptest::array::uniform7(value()), 1..32)
        }

        proptest! {
            #[test]
            fn snapped_keys_are_a_total_order(mut ks in keys()) {
                snap_keys(&mut ks);
                for a in &ks {
                    prop_assert_eq!(cmp_key(a, a), Ordering::Equal);
                    for b in &ks {
                        prop_assert_eq!(cmp_key(a, b), cmp_key(b, a).reverse());
                        for c in &ks {
                            if cmp_key(a, b).is_le() && cmp_key(b, c).is_le() {
                                prop_assert!(cmp_key(a, c).is_le());
                            }
                        }
                    }
                }
            }

            #[test]
            fn near_equal_duplicates_never_panic(steps in proptest::collection::vec(0u32..40, 2..64)) {
                let items: Vec<Item> = steps
                    .iter()
                    .enumerate()
                    .map(|(i, &s)| {
                        let w = (3000.0 * 1.004f64.powi(s as i32)) as u32;
                        let len = (w as u64 * w as u64 * 2 / 3) / 3 + s as u64 * 977;
                        item(&format!("{i}.jpg"), Kind::Jpeg, w, len)
                    })
                    .collect();
                let group: Vec<u32> = (0..items.len() as u32).collect();
                let p = pick_duplicates(&items, &group);
                prop_assert_eq!(p.iter().filter(|p| p.keep).count(), 1);
            }
        }
    }

    #[test]
    fn duplicates_prefer_raw_then_resolution() {
        let items = vec![
            item("a.jpg", Kind::Jpeg, 2000, 900_000),
            item("a.dng", Kind::Raw, 2000, 20_000_000),
            item("a_small.jpg", Kind::Jpeg, 800, 100_000),
        ];
        let p = pick_duplicates(&items, &[0, 1, 2]);
        assert!(p[1].keep && p[1].reason == Reason::RawOriginal);
        assert!(!p[0].keep && p[0].reason == Reason::RenderingOfRaw);
        assert!(!p[2].keep && p[2].reason == Reason::Derivative);
    }

    #[test]
    fn protected_are_never_discarded() {
        let mut items = vec![
            item("a.png", Kind::Png, 4000, 9_000_000),
            item("b.jpg", Kind::Jpeg, 1000, 200_000),
            item("c.jpg", Kind::Jpeg, 1000, 100_000),
        ];
        items[2].protect = Some(Protect::Flagged);
        let p = pick_duplicates(&items, &[0, 1, 2]);
        assert!(p[2].keep && p[2].reason == Reason::Protected(Protect::Flagged));
        assert!(p[0].keep);
        assert!(!p[1].keep);
        // Similar shots too, even the blurriest.
        for (k, it) in items.iter_mut().enumerate() {
            it.sig = Signature {
                quality: Quality {
                    grid: [10.0 * (k + 1) as f32; 16],
                    mean: 0.45,
                    ..Quality::default()
                },
                ..Signature::default()
            };
        }
        items[2].protect = None;
        items[0].protect = Some(Protect::Rated);
        let p = pick_similar(&items, &[0, 1, 2]);
        assert!(p[0].keep && p[0].reason == Reason::Protected(Protect::Rated));
        assert!(p[2].keep && p[2].reason == Reason::Sharpest);
        assert!(!p[1].keep && p[1].reason == Reason::Blurrier);
    }

    /// A burst of 12 MP frames where the last is defocused: alike on the thumbnail (blur of a
    /// few pixels is under its resolving limit) except that the blur smooths the noise there.
    fn burst() -> Vec<Item> {
        (0..4)
            .map(|k| {
                let mut it = item(&format!("b{k}.jpg"), Kind::Jpeg, 4000, 5_000_000);
                it.sig.quality = Quality {
                    grid: [20.0; 16],
                    detail: 20.0,
                    mean: 0.45,
                    noise: if k == 3 { 1.5 } else { 3.0 },
                    ..Quality::default()
                };
                it
            })
            .collect()
    }

    #[test]
    fn blur_never_buys_the_keep_on_noise() {
        let mut items = burst();
        // Thumbnails only: the noise there is not compared.
        let p = pick_similar(&items, &[0, 1, 2, 3]);
        let best = p
            .iter()
            .position(|p| p.keep && p.reason != Reason::AlsoGood);
        assert_ne!(best, Some(3), "{p:?}");
        assert!(p.iter().all(|p| p.reason != Reason::LeastNoise), "{p:?}");
        // At full resolution the defocus shows: it goes, for blur, though cleaner.
        for (k, it) in items.iter_mut().enumerate() {
            it.focus = Some(Focus {
                scale: 1,
                grid: [if k == 3 { 8.0 } else { 19.5 + k as f32 * 0.1 }; 16],
                detail: if k == 3 { 8.0 } else { 19.5 },
                anisotropy: 0.01,
                noise: if k == 3 { 0.4 } else { 0.75 },
                ..Focus::default()
            });
        }
        let p = pick_similar(&items, &[0, 1, 2, 3]);
        assert!(p[2].keep && p[2].reason == Reason::Sharpest, "{p:?}");
        assert!(p[0].keep && p[1].keep, "{p:?}");
        assert!(!p[3].keep && p[3].reason == Reason::Blurrier, "{p:?}");
        // One member not measured: all fall back to the thumbnails together.
        items[0].focus = None;
        let p = pick_similar(&items, &[0, 1, 2, 3]);
        assert!(p.iter().all(|p| p.reason != Reason::LeastNoise), "{p:?}");
        // Small images: the thumbnail's noise is the image's, and counts.
        let mut small = burst();
        for it in &mut small {
            it.width = 400;
            it.height = 300;
        }
        let p = pick_similar(&small, &[0, 1, 2, 3]);
        assert!(p[3].keep && p[3].reason == Reason::LeastNoise, "{p:?}");
    }

    #[test]
    fn mixed_sizes_use_the_measure_at_the_smallest_side() {
        let f = |scale: u32, detail: f32| Focus {
            scale,
            grid: [detail; 16],
            detail,
            anisotropy: 0.0,
            noise: 1.0,
            ..Focus::default()
        };
        let mut items = vec![
            item("big.jpg", Kind::Jpeg, 4000, 1),
            item("small.jpg", Kind::Jpeg, 2000, 1),
            item("small2.jpg", Kind::Jpeg, 2040, 1),
        ];
        items[0].focus = Some(f(1, 9.0));
        items[1].focus = Some(f(1, 5.0));
        items[2].focus = Some(f(1, 5.5));
        // Sides 4000 and 2000: nothing compares yet.
        assert_eq!(fine_foci(&items, &[0, 1]), vec![None, None]);
        // Within 3 %: as they are.
        assert_eq!(
            fine_foci(&items, &[1, 2]),
            vec![Some(f(1, 5.0)), Some(f(1, 5.5))]
        );
        // The big one measured at 2000 (an UNAVAILABLE one at 3000 is no use).
        items[0].focus_at = vec![(2000, f(2, 6.0)), (3000, Focus::UNAVAILABLE)];
        assert_eq!(
            fine_foci(&items, &[0, 1, 2]),
            vec![Some(f(2, 6.0)), Some(f(1, 5.0)), Some(f(1, 5.5))]
        );
        // A member measured natively at half scale samples at 1000 px: 2000 is not near.
        items[1].focus = Some(f(2, 5.0));
        assert_eq!(fine_foci(&items, &[0, 1]), vec![None, None]);
        items[0].focus_at.push((1010, f(4, 3.0)));
        assert_eq!(
            fine_foci(&items, &[0, 1]),
            vec![Some(f(4, 3.0)), Some(f(2, 5.0))]
        );
        // Measures picked this way make the picks fine (full-resolution noise compared).
        let ms = measures(&items, &[0, 1]);
        assert!(ms.iter().all(|m| m.fine));
    }
}
