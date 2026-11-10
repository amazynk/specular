//! Hierarchical grid clustering of photo positions.
//!
//! Points are sorted once by their Morton (z-order) code. At zoom level `L` the world is cut
//! into square cells of [`CELL_PX`] logical px; the points of one cell are a contiguous run
//! of the sorted array, so a level's clusters come from one linear pass and a cluster's
//! members are a slice. Levels are built when first asked for and kept; cells nest, so a
//! cluster splits into the clusters of the next level as the map zooms in.

use std::sync::OnceLock;

use crate::camera::Camera;
use crate::{MAX_ZOOM, TILE};

/// A cluster cell's side in logical px at its own level.
pub const CELL_PX: f64 = 64.0;
/// Bits per axis of the Morton codes (enough for the cells of every level).
const BITS: u32 = 24;
/// Levels 0..=LEVELS-1.
const LEVELS: usize = MAX_ZOOM as usize + 1;
/// log2(TILE / CELL_PX): the cells at level 0 are 2^SHIFT a side.
const SHIFT: u32 = ((TILE / CELL_PX) as u32).ilog2();

/// The points of one cell at one level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cluster {
    /// The members' mean position, world units.
    pub x: f64,
    pub y: f64,
    /// The members' run in [`Clusters::members`] order.
    pub start: u32,
    pub count: u32,
}

#[derive(Debug, Default)]
pub struct Clusters {
    /// Positions in Morton order.
    pts: Vec<(f64, f64)>,
    /// The caller's index of each sorted point.
    ids: Vec<u32>,
    codes: Vec<u64>,
    levels: [OnceLock<Vec<Cluster>>; LEVELS],
}

fn spread(v: u32) -> u64 {
    let mut v = u64::from(v);
    v = (v | (v << 16)) & 0x0000_ffff_0000_ffff;
    v = (v | (v << 8)) & 0x00ff_00ff_00ff_00ff;
    v = (v | (v << 4)) & 0x0f0f_0f0f_0f0f_0f0f;
    v = (v | (v << 2)) & 0x3333_3333_3333_3333;
    v = (v | (v << 1)) & 0x5555_5555_5555_5555;
    v
}

fn morton(x: f64, y: f64) -> u64 {
    let q = f64::from(1u32 << BITS);
    let max = (1u32 << BITS) - 1;
    let xi = ((x.rem_euclid(1.0) * q) as u32).min(max);
    let yi = ((y.clamp(0.0, 1.0) * q) as u32).min(max);
    spread(xi) | (spread(yi) << 1)
}

impl Clusters {
    /// Clusters `points` (world units, as from [`crate::project`]) given with the caller's
    /// ids.
    pub fn new(points: impl IntoIterator<Item = (u32, (f64, f64))>) -> Clusters {
        let mut v: Vec<(u64, u32, (f64, f64))> = points
            .into_iter()
            .filter(|(_, p)| p.0.is_finite() && p.1.is_finite())
            .map(|(id, p)| {
                (
                    morton(p.0, p.1),
                    id,
                    (p.0.rem_euclid(1.0), p.1.clamp(0.0, 1.0)),
                )
            })
            .collect();
        v.sort_unstable_by_key(|&(c, id, _)| (c, id));
        Clusters {
            codes: v.iter().map(|e| e.0).collect(),
            ids: v.iter().map(|e| e.1).collect(),
            pts: v.iter().map(|e| e.2).collect(),
            levels: Default::default(),
        }
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// The clusters at zoom level `level` (clamped to the deepest), in Morton order.
    pub fn level(&self, level: u8) -> &[Cluster] {
        let l = usize::from(level).min(LEVELS - 1);
        self.levels[l].get_or_init(|| self.build(l as u32))
    }

    fn build(&self, l: u32) -> Vec<Cluster> {
        let shift = 2 * (BITS - (l + SHIFT));
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.codes.len() {
            let key = self.codes[i] >> shift;
            let mut j = i + 1;
            while j < self.codes.len() && self.codes[j] >> shift == key {
                j += 1;
            }
            let (mut sx, mut sy) = (0.0, 0.0);
            for p in &self.pts[i..j] {
                sx += p.0;
                sy += p.1;
            }
            let n = (j - i) as f64;
            out.push(Cluster {
                x: sx / n,
                y: sy / n,
                start: i as u32,
                count: (j - i) as u32,
            });
            i = j;
        }
        out
    }

    /// The caller's ids of a cluster's members (the first is the cluster's face).
    pub fn members(&self, c: &Cluster) -> &[u32] {
        &self.ids[c.start as usize..(c.start + c.count) as usize]
    }

    /// The world rectangle around a cluster's members.
    pub fn bounds(&self, c: &Cluster) -> (f64, f64, f64, f64) {
        bounds(&self.pts[c.start as usize..(c.start + c.count) as usize])
    }

    /// The world rectangle around every point (x1 may pass 1 when the points straddle the
    /// antimeridian); `None` without points.
    pub fn all_bounds(&self) -> Option<(f64, f64, f64, f64)> {
        (!self.pts.is_empty()).then(|| bounds(&self.pts))
    }

    /// The clusters of the camera's level on screen (with `margin` logical px around the
    /// view), each with its screen position; a cluster shows once per copy of the world in
    /// view.
    pub fn on_screen(&self, cam: &Camera, margin: f64) -> Vec<(Cluster, (f64, f64))> {
        let (x0, y0, x1, y1) = cam.visible();
        let m = margin / cam.scale();
        let (x0, y0, x1, y1) = (x0 - m, y0 - m, x1 + m, y1 + m);
        let s = cam.scale();
        let (hw, hh) = (cam.size.0 / 2.0, cam.size.1 / 2.0);
        let mut out = Vec::new();
        let level = self.level(cam.level());
        for k in (x0.floor() as i64)..=(x1.floor() as i64) {
            let k = k as f64;
            for c in level {
                let x = c.x + k;
                if x >= x0 && x <= x1 && c.y >= y0 && c.y <= y1 {
                    out.push((
                        *c,
                        ((x - cam.center.0) * s + hw, (c.y - cam.center.1) * s + hh),
                    ));
                }
            }
        }
        out
    }
}

/// Grid cells can put two clusters side by side, so their markers overlap: this joins each
/// marker to a bigger one it would overlap. `markers` are counts and screen positions;
/// `radius` is a marker's radius (px) for its count. Returns, for each marker, the one it
/// joins (its own index when it stays). The biggest stay first, so the joined ones are the
/// small ones next to them; one pass (a marker only joins one that stays).
pub fn declutter(markers: &[(u32, (f64, f64))], radius: impl Fn(u32) -> f64) -> Vec<usize> {
    let n = markers.len();
    let mut parent: Vec<usize> = (0..n).collect();
    if n < 2 {
        return parent;
    }
    let r: Vec<f64> = markers.iter().map(|m| radius(m.0)).collect();
    let cell = 2.0 * r.iter().copied().fold(1.0, f64::max);
    let key = |p: (f64, f64)| ((p.0 / cell).floor() as i64, (p.1 / cell).floor() as i64);
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| (std::cmp::Reverse(markers[i].0), i));
    let mut grid: std::collections::HashMap<(i64, i64), Vec<usize>> =
        std::collections::HashMap::with_capacity(n);
    for i in order {
        let p = markers[i].1;
        let (cx, cy) = key(p);
        let mut join = None;
        'look: for gx in cx - 1..=cx + 1 {
            for gy in cy - 1..=cy + 1 {
                for &j in grid.get(&(gx, gy)).into_iter().flatten() {
                    let q = markers[j].1;
                    let d = (p.0 - q.0).hypot(p.1 - q.1);
                    // Touching is fine; a real overlap joins.
                    if d < (r[i] + r[j]) * 0.85 {
                        join = Some(j);
                        break 'look;
                    }
                }
            }
        }
        match join {
            Some(j) => parent[i] = j,
            None => grid.entry((cx, cy)).or_default().push(i),
        }
    }
    parent
}

/// The bounding box of points in the unit square, taking the narrower way round the world:
/// when the widest east–west gap between points spans the antimeridian or not.
fn bounds(pts: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    let (mut y0, mut y1) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut xs: Vec<f64> = Vec::with_capacity(pts.len());
    for p in pts {
        y0 = y0.min(p.1);
        y1 = y1.max(p.1);
        xs.push(p.0);
    }
    xs.sort_unstable_by(f64::total_cmp);
    let (first, last) = (xs[0], xs[xs.len() - 1]);
    // The gap across the antimeridian, then the widest gap inside.
    let mut gap = (first + 1.0 - last, last, first + 1.0);
    for w in xs.windows(2) {
        if w[1] - w[0] > gap.0 {
            gap = (w[1] - w[0], w[0], w[1]);
        }
    }
    if gap.2 > 1.0 {
        // The widest gap is across the antimeridian: no wrap needed.
        (first, y0, last, y1)
    } else {
        // Start east of the gap, end west of it one world later.
        (gap.2, y0, gap.1 + 1.0, y1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proj::project;

    #[test]
    fn overlapping_markers_join_the_biggest() {
        let r = |n: u32| {
            if n == 1 {
                6.0
            } else {
                15.0 + 3.0 * f64::from(n).log10()
            }
        };
        let m = [
            (3, (100.0, 100.0)),
            (8, (110.0, 95.0)),
            (1, (300.0, 300.0)),
            (1, (305.0, 300.0)),
            (2, (140.0, 100.0)),
            (5, (500.0, 100.0)),
        ];
        let p = declutter(&m, r);
        assert_eq!(p[1], 1, "the biggest stays");
        assert_eq!(p[0], 1, "the 3 under the 8 joins it");
        assert_eq!(p[4], 4, "the 2 only touches the 8");
        assert_eq!(
            (p[2], p[3]),
            (2, 2),
            "two pins on each other: the first stays"
        );
        assert_eq!(p[5], 5);
        assert!(declutter(&[], r).is_empty());
    }

    fn pseudo(n: usize) -> Vec<(u32, (f64, f64))> {
        let mut s = 0x9e37_79b9_7f4a_7c15u64;
        let mut r = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 11) as f64 / (1u64 << 53) as f64
        };
        (0..n as u32).map(|i| (i, (r(), r()))).collect()
    }

    #[test]
    fn every_level_holds_every_point_once() {
        let pts = pseudo(5000);
        let c = Clusters::new(pts.clone());
        for l in 0..=MAX_ZOOM as u8 {
            let level = c.level(l);
            assert_eq!(level.iter().map(|k| k.count as usize).sum::<usize>(), 5000);
            let mut seen: Vec<u32> = level.iter().flat_map(|k| c.members(k).to_vec()).collect();
            seen.sort_unstable();
            assert_eq!(seen, (0..5000).collect::<Vec<_>>());
            // A cluster's members share its cell.
            let cell = 1.0 / f64::from(1u32 << (u32::from(l) + SHIFT));
            for k in level {
                let (x0, y0, x1, y1) = c.bounds(k);
                assert!(x1 - x0 < cell && y1 - y0 < cell, "level {l}");
                assert!((x0..=x1).contains(&k.x) && (y0..=y1).contains(&k.y));
            }
        }
        // Deeper levels never have fewer clusters, and nest.
        for l in 1..=MAX_ZOOM as u8 {
            assert!(c.level(l).len() >= c.level(l - 1).len());
        }
        assert_eq!(c.level(0).len(), 16, "4 × 4 cells of 64 px at zoom 0");
        assert_eq!(c.level(MAX_ZOOM as u8).len(), 5000);
    }

    #[test]
    fn towns_split_as_the_map_zooms_in() {
        let lisbon = project(38.7223, -9.1393);
        let porto = project(41.1579, -8.6291);
        let mut pts = Vec::new();
        for i in 0..30u32 {
            let d = f64::from(i) * 1e-7;
            pts.push((i, (lisbon.0 + d, lisbon.1 + d)));
            pts.push((100 + i, (porto.0 - d, porto.1)));
        }
        let c = Clusters::new(pts);
        // Lisbon and Porto (~270 km apart) share a cell at zoom 0 and part by zoom 6.
        assert_eq!(c.level(0).len(), 1);
        assert_eq!(c.level(0)[0].count, 60);
        let l6 = c.level(6);
        assert_eq!(l6.len(), 2);
        assert!(l6.iter().all(|k| k.count == 30));
        let lis = l6.iter().find(|k| c.members(k)[0] < 100).unwrap();
        assert!((lis.x - lisbon.0).abs() < 1e-5 && (lis.y - lisbon.1).abs() < 1e-5);
    }

    #[test]
    fn on_screen_culls_and_repeats_the_world() {
        let c = Clusters::new([(0, (0.5, 0.5)), (1, (0.9, 0.5)), (2, (0.02, 0.5))]);
        let mut cam = Camera::new((800.0, 600.0));
        cam.zoom = 4.0;
        cam.center = (0.5, 0.5);
        let on = c.on_screen(&cam, 0.0);
        assert_eq!(on.len(), 1);
        assert_eq!(on[0].1, (400.0, 300.0));
        // Across the antimeridian: points on both sides of it.
        cam.center = (0.999, 0.5);
        cam.zoom = 3.0; // 2048 px a world: 0.39 of it in view
        let on = c.on_screen(&cam, 0.0);
        let ids: Vec<u32> = on.iter().map(|o| c.members(&o.0)[0]).collect();
        assert_eq!(ids.len(), 2, "{ids:?}");
        assert!(ids.contains(&1) && ids.contains(&2));
        let x2 = on.iter().find(|o| c.members(&o.0)[0] == 2).unwrap().1.0;
        assert!((x2 - (400.0 + 0.021 * cam.scale())).abs() < 1e-6);
        // A view wider than the world shows each point once per copy.
        let mut wide = Camera::new((2000.0, 300.0));
        wide.zoom = wide.min_zoom();
        let n = c.on_screen(&wide, 0.0).len();
        assert!(n >= 3 * 6, "{n}");
    }

    #[test]
    fn bounds_take_the_short_way_round() {
        let b = bounds(&[(0.98, 0.4), (0.01, 0.5), (0.99, 0.45)]);
        assert!(
            (b.0 - 0.98).abs() < 1e-12 && (b.2 - 1.01).abs() < 1e-12,
            "{b:?}"
        );
        let b = bounds(&[(0.2, 0.4), (0.3, 0.5)]);
        assert_eq!(b, (0.2, 0.4, 0.3, 0.5));
        let b = bounds(&[(0.7, 0.4)]);
        assert_eq!(b, (0.7, 0.4, 0.7, 0.4));
    }

    #[test]
    fn non_finite_points_are_dropped() {
        let c = Clusters::new([(0, (f64::NAN, 0.5)), (1, (0.5, 0.5))]);
        assert_eq!(c.len(), 1);
        assert!(Clusters::new([]).all_bounds().is_none());
    }
}
