//! Level of detail: the points sorted into the leaves of an octree (built by halving boxes
//! along x, y and z in turn, in place), each leaf's points shuffled, so drawing the first `k`
//! points of a leaf draws an even `k`-point subsample of it. [`select`] picks how many of each
//! leaf a view needs under a point budget.

use crate::{Bounds, Leaf};

/// Points per leaf at most (unless they all share one spot).
pub const LEAF_POINTS: usize = 16_384;

/// Splits at most this deep (20 octree levels).
const MAX_DEPTH: u32 = 60;

/// Below this many points a split runs on this thread.
const PARALLEL: usize = 1 << 17;

/// Sorts `positions` (and `attrs` alike) into leaves, returned in order.
pub(crate) fn build(positions: &mut [[f32; 3]], attrs: &mut [[u32; 2]]) -> Vec<Leaf> {
    let mut b = Bounds::EMPTY;
    for &p in positions.iter() {
        b.add(p);
    }
    if b.is_empty() {
        return Vec::new();
    }
    // A cube, so splits halve every axis alike.
    let side = b
        .size()
        .into_iter()
        .fold(0.0f32, f32::max)
        .max(f32::MIN_POSITIVE);
    let cube = Cell {
        min: b.min,
        side: [side; 3],
    };
    let mut leaves = Vec::new();
    split(positions, attrs, cube, 0, 0, &mut leaves);
    leaves
}

#[derive(Clone, Copy)]
struct Cell {
    min: [f32; 3],
    side: [f32; 3],
}

fn split(
    pos: &mut [[f32; 3]],
    attrs: &mut [[u32; 2]],
    cell: Cell,
    depth: u32,
    first: u32,
    out: &mut Vec<Leaf>,
) {
    if pos.is_empty() {
        return;
    }
    if pos.len() <= LEAF_POINTS || depth >= MAX_DEPTH {
        shuffle(pos, attrs, first);
        let mut b = Bounds::EMPTY;
        for &p in pos.iter() {
            b.add(p);
        }
        out.push(Leaf {
            first,
            count: pos.len() as u32,
            bounds: b,
        });
        return;
    }
    let axis = (depth % 3) as usize;
    let mid = cell.min[axis] + cell.side[axis] * 0.5;
    // Partition: points below the middle first.
    let mut lo = 0;
    let mut hi = pos.len();
    while lo < hi {
        if pos[lo][axis] < mid {
            lo += 1;
        } else {
            hi -= 1;
            pos.swap(lo, hi);
            attrs.swap(lo, hi);
        }
    }
    let (p0, p1) = pos.split_at_mut(lo);
    let (a0, a1) = attrs.split_at_mut(lo);
    let mut c0 = cell;
    c0.side[axis] *= 0.5;
    let mut c1 = c0;
    c1.min[axis] = mid;
    let f1 = first + lo as u32;
    if p0.len() + p1.len() > PARALLEL {
        let mut out1 = Vec::new();
        rayon::join(
            || split(p0, a0, c0, depth + 1, first, out),
            || split(p1, a1, c1, depth + 1, f1, &mut out1),
        );
        out.append(&mut out1);
    } else {
        split(p0, a0, c0, depth + 1, first, out);
        split(p1, a1, c1, depth + 1, f1, out);
    }
}

/// A Fisher–Yates shuffle seeded by the leaf's position (the same file shuffles the same).
fn shuffle(pos: &mut [[f32; 3]], attrs: &mut [[u32; 2]], seed: u32) {
    let mut s = u64::from(seed) ^ 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = s;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    for i in (1..pos.len()).rev() {
        let j = (next() % (i as u64 + 1)) as usize;
        pos.swap(i, j);
        attrs.swap(i, j);
    }
}

/// A view for [`select`]: clip-space rows (`clip = rows · [p, 1]`, w the depth along the
/// view for perspective, 1 for orthographic) and the viewport in pixels.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub rows: [[f32; 4]; 4],
    pub width: f32,
    pub height: f32,
    /// Spacing between points on screen that looks filled, in pixels.
    pub spacing_px: f32,
}

/// How many points of each leaf to draw (`(first, count)` runs, in leaf order) so the view
/// looks filled, with at most `budget` points in all: leaves outside the view get none, the
/// others points for their area on screen, all scaled down together past the budget.
pub fn select(leaves: &[Leaf], view: &View, budget: u64) -> Vec<(u32, u32)> {
    let r = &view.rows;
    let px_per_unit =
        0.5 * view.width * (r[0][0] * r[0][0] + r[0][1] * r[0][1] + r[0][2] * r[0][2]).sqrt();
    let spacing = view.spacing_px.max(0.5);
    let ortho = r[3][0] == 0.0 && r[3][1] == 0.0 && r[3][2] == 0.0;
    let mut want: Vec<(usize, f64)> = Vec::with_capacity(leaves.len());
    let mut sum = 0.0f64;
    for (i, l) in leaves.iter().enumerate() {
        if l.bounds.is_empty() {
            continue;
        }
        let c = [0, 1, 2].map(|k| (l.bounds.min[k] + l.bounds.max[k]) * 0.5);
        let s = l.bounds.size();
        let radius = 0.5 * (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
        let clip = [0, 1, 2, 3].map(|k| r[k][0] * c[0] + r[k][1] * c[1] + r[k][2] * c[2] + r[k][3]);
        let w = clip[3];
        // Orthographic views have no eye plane: every leaf is in front, at w = 1.
        let straddles = !ortho && w <= radius;
        // Behind the eye, wholly.
        if !ortho && w < -radius {
            continue;
        }
        let r_px = radius * px_per_unit / w.max(1e-6);
        // In front: cull by the projected circle against the viewport (in NDC).
        if !straddles {
            let rx = r_px / (0.5 * view.width);
            let ry = r_px / (0.5 * view.height);
            let (x, y) = (clip[0] / w, clip[1] / w);
            if x + rx < -1.0 || x - rx > 1.0 || y + ry < -1.0 || y - ry > 1.0 {
                continue;
            }
        }
        // Straddling the eye plane: wanted in full.
        let need = if straddles {
            f64::from(l.count)
        } else {
            let area = std::f64::consts::PI * f64::from(r_px) * f64::from(r_px);
            (area / f64::from(spacing * spacing)).clamp(1.0, f64::from(l.count))
        };
        sum += need;
        want.push((i, need));
    }
    let scale = if sum > budget as f64 {
        budget as f64 / sum
    } else {
        1.0
    };
    // Shares carried leaf to leaf, so the total stays within the budget and small leaves
    // still get their points now and then.
    let mut carried = 0.0f64;
    let mut given = 0u64;
    want.into_iter()
        .filter_map(|(i, n)| {
            carried += n * scale;
            let k = (carried.round() as u64)
                .saturating_sub(given)
                .min(u64::from(u32::MAX)) as u32;
            given += u64::from(k);
            (k > 0).then(|| (leaves[i].first, k.min(leaves[i].count)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cloud(n: usize) -> (Vec<[f32; 3]>, Vec<[u32; 2]>) {
        let mut s = 1u32;
        let mut r = || {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            (s % 10_000) as f32 / 100.0
        };
        let p: Vec<[f32; 3]> = (0..n).map(|_| [r(), r(), r() * 0.1]).collect();
        let a = (0..n as u32).map(|i| [i, 0]).collect();
        (p, a)
    }

    #[test]
    fn leaves_cover_and_contain() {
        let (mut p, mut a) = cloud(100_000);
        let orig: Vec<[f32; 3]> = p.clone();
        let leaves = build(&mut p, &mut a);
        let mut next = 0;
        for l in &leaves {
            assert_eq!(l.first, next);
            assert!(l.count as usize <= LEAF_POINTS);
            for i in l.first..l.first + l.count {
                let q = p[i as usize];
                assert!((0..3).all(|k| q[k] >= l.bounds.min[k] && q[k] <= l.bounds.max[k]));
            }
            next += l.count;
        }
        assert_eq!(next as usize, p.len());
        // Attributes travelled with their points.
        for (q, at) in p.iter().zip(&a) {
            assert_eq!(*q, orig[at[0] as usize]);
        }
    }

    #[test]
    fn identical_points_stop_splitting() {
        let mut p = vec![[1.0f32, 2.0, 3.0]; 40_000];
        let mut a = vec![[0u32; 2]; 40_000];
        let leaves = build(&mut p, &mut a);
        assert_eq!(leaves.iter().map(|l| l.count).sum::<u32>(), 40_000);
    }

    #[test]
    fn select_respects_budget_and_culls() {
        let (mut p, mut a) = cloud(200_000);
        let leaves = build(&mut p, &mut a);
        // Orthographic, looking down z, 100 units across 1000 px, centred on the cloud.
        let s = 2.0 / 100.0;
        let view = View {
            rows: [
                [s, 0.0, 0.0, -1.0],
                [0.0, s, 0.0, -1.0],
                [0.0, 0.0, 0.0, 0.5],
                [0.0, 0.0, 0.0, 1.0],
            ],
            width: 1000.0,
            height: 1000.0,
            spacing_px: 1.0,
        };
        let all: u64 = select(&leaves, &view, u64::MAX)
            .iter()
            .map(|r| u64::from(r.1))
            .sum();
        assert!(all > 150_000, "{all}");
        let some: u64 = select(&leaves, &view, 10_000)
            .iter()
            .map(|r| u64::from(r.1))
            .sum();
        assert!((9_000..=11_000).contains(&some), "{some}");
        // Shifted off screen: nothing.
        let mut off = view;
        off.rows[0][3] = 5.0;
        assert!(select(&leaves, &off, u64::MAX).is_empty());
    }
}
