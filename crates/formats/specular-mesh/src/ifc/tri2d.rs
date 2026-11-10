//! Triangulation of plane polygons with holes: each hole is joined to the outer ring by a
//! bridge from its rightmost vertex to a visible vertex of the ring, then the one ring is
//! cut into ears. Quadratic in the corner count, which is small for profiles and faces;
//! very large rings fall back to a fan.

use super::geom::{P2, ring_area};

/// Corners beyond which ear clipping is not attempted.
const MAX_EAR_CORNERS: usize = 20_000;

fn cross(a: P2, b: P2, c: P2) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/// Triangles over the corners of `outer` followed by those of each hole, in that order, as
/// indices; counter-clockwise in the plane's own orientation whatever the rings' turning.
pub(crate) fn triangulate(outer: &[P2], holes: &[Vec<P2>]) -> Vec<[u32; 3]> {
    let mut out = Vec::new();
    if outer.len() < 3 {
        return out;
    }
    // Turned by an odd angle: rays along +x then rarely run along the rings' own edges
    // (rows of holes, axis-aligned walls), which ear clipping handles badly.
    let (sn, cs) = 0.371_234_f64.sin_cos();
    let turn = |p: &P2| -> P2 { [p[0] * cs - p[1] * sn, p[0] * sn + p[1] * cs] };
    let outer: Vec<P2> = outer.iter().map(turn).collect();
    let holes: Vec<Vec<P2>> = holes.iter().map(|h| h.iter().map(turn).collect()).collect();
    let outer = &outer[..];
    let holes = &holes[..];
    let mut pts: Vec<P2> = outer.to_vec();
    let mut ring: Vec<u32> = (0..outer.len() as u32).collect();
    if ring_area(outer) < 0.0 {
        ring.reverse();
    }
    // Holes clockwise, then bridged in order of their rightmost x, largest first.
    let mut hs: Vec<(f64, Vec<u32>)> = Vec::new();
    for h in holes {
        if h.len() < 3 {
            continue;
        }
        let base = pts.len() as u32;
        pts.extend_from_slice(h);
        let mut idx: Vec<u32> = (base..base + h.len() as u32).collect();
        if ring_area(h) > 0.0 {
            idx.reverse();
        }
        let maxx = h.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
        hs.push((maxx, idx));
    }
    hs.sort_by(|a, b| b.0.total_cmp(&a.0));
    let n_all = pts.len();
    if n_all > MAX_EAR_CORNERS {
        fan(&ring, &mut out);
        return out;
    }
    for (_, h) in hs {
        bridge(&pts, &mut ring, &h);
    }
    ear_clip(&pts, &ring, &mut out);
    out
}

fn fan(ring: &[u32], out: &mut Vec<[u32; 3]>) {
    for k in 1..ring.len().saturating_sub(1) {
        out.push([ring[0], ring[k], ring[k + 1]]);
    }
}

/// Joins hole `h` (clockwise) into `ring` (counter-clockwise).
fn bridge(pts: &[P2], ring: &mut Vec<u32>, h: &[u32]) {
    // The hole's rightmost vertex.
    let (mk, &m) = h
        .iter()
        .enumerate()
        .max_by(|a, b| pts[*a.1 as usize][0].total_cmp(&pts[*b.1 as usize][0]))
        .expect("hole has corners");
    let mp = pts[m as usize];
    // The nearest ring edge crossed by the ray to +x.
    let n = ring.len();
    let mut best: Option<(f64, usize)> = None;
    for k in 0..n {
        let (a, b) = (pts[ring[k] as usize], pts[ring[(k + 1) % n] as usize]);
        if (a[1] > mp[1]) == (b[1] > mp[1]) && a[1] != mp[1] && b[1] != mp[1] {
            continue;
        }
        if a[1] == b[1] {
            // Horizontal edge on the ray: its nearer end.
            if a[1] == mp[1] {
                for (x, kk) in [(a[0], k), (b[0], (k + 1) % n)] {
                    if x >= mp[0] && best.is_none_or(|(bx, _)| x < bx) {
                        best = Some((x, kk));
                    }
                }
            }
            continue;
        }
        let t = (mp[1] - a[1]) / (b[1] - a[1]);
        if !(0.0..=1.0).contains(&t) {
            continue;
        }
        let x = a[0] + t * (b[0] - a[0]);
        if x < mp[0] {
            continue;
        }
        // The candidate is the edge's end with the larger x.
        let kk = if a[0] > b[0] { k } else { (k + 1) % n };
        if best.is_none_or(|(bx, _)| x < bx) {
            best = Some((x, kk));
        }
    }
    let Some((ix, mut pk)) = best else {
        // Not inside: drawn as a separate ring.
        return;
    };
    let p = pts[ring[pk] as usize];
    let i = [ix, mp[1]];
    // A reflex ring vertex inside the triangle (M, I, P) blocks the view: take the one
    // making the smallest angle with the ray.
    if p != i {
        let tri = |q: P2| {
            let (d1, d2, d3) = (cross(mp, i, q), cross(i, p, q), cross(p, mp, q));
            let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
            let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
            !(neg && pos)
        };
        let mut best_angle = f64::INFINITY;
        for k in 0..n {
            let q = pts[ring[k] as usize];
            if k == pk || q == mp {
                continue;
            }
            let prev = pts[ring[(k + n - 1) % n] as usize];
            let next = pts[ring[(k + 1) % n] as usize];
            let reflex = cross(prev, q, next) <= 0.0;
            if reflex && tri(q) && q[0] >= mp[0] {
                let ang = (q[1] - mp[1]).abs().atan2(q[0] - mp[0]);
                if ang < best_angle {
                    best_angle = ang;
                    pk = k;
                }
            }
        }
    }
    // A corner the ring passes more than once (an earlier bridge's end): join at the pass
    // whose corner opens towards the hole, else the bridges cross there.
    let at = pts[ring[pk] as usize];
    if let Some(k) = (0..n).find(|&k| {
        pts[ring[k] as usize] == at
            && opens_towards(
                pts[ring[(k + n - 1) % n] as usize],
                at,
                pts[ring[(k + 1) % n] as usize],
                mp,
            )
    }) {
        pk = k;
    }
    splice(ring, h, mk, pk);
}

/// Whether `m` lies in the inside corner at `v` of a counter-clockwise ring running
/// `a` → `v` → `b`.
fn opens_towards(a: P2, v: P2, b: P2, m: P2) -> bool {
    if cross(a, v, b) >= 0.0 {
        cross(a, v, m) > 0.0 && cross(v, b, m) > 0.0
    } else {
        cross(a, v, m) > 0.0 || cross(v, b, m) > 0.0
    }
}

/// `ring[..=pk]`, the hole from its corner `mk` around back to it, then `ring[pk..]`.
fn splice(ring: &mut Vec<u32>, h: &[u32], mk: usize, pk: usize) {
    let mut merged = Vec::with_capacity(ring.len() + h.len() + 2);
    merged.extend_from_slice(&ring[..=pk]);
    for j in 0..=h.len() {
        merged.push(h[(mk + j) % h.len()]);
    }
    merged.extend_from_slice(&ring[pk..]);
    *ring = merged;
}

fn ear_clip(pts: &[P2], ring: &[u32], out: &mut Vec<[u32; 3]>) {
    let n = ring.len();
    if n < 3 {
        return;
    }
    let mut next: Vec<usize> = (0..n).map(|k| (k + 1) % n).collect();
    let mut prev: Vec<usize> = (0..n).map(|k| (k + n - 1) % n).collect();
    let p = |k: usize| pts[ring[k] as usize];
    let scale = ring
        .iter()
        .map(|&i| pts[i as usize][0].abs().max(pts[i as usize][1].abs()))
        .fold(0.0f64, f64::max)
        .max(1e-30);
    let eps = scale * scale * 1e-14;
    let mut left = n;
    let mut cur = 0usize;
    let mut stall = 0usize;
    // Only reflex (and flat) corners can lie inside an ear: they are all the ear test
    // looks at. A corner never turns reflex as ears are cut.
    let mut reflex_flag: Vec<bool> = (0..n)
        .map(|k| cross(p(prev[k]), p(k), p(next[k])) <= eps)
        .collect();
    let mut reflex: Vec<usize> = (0..n).filter(|&k| reflex_flag[k]).collect();
    let mut alive = vec![true; n];
    // `touch`: corners on an ear's edge block it (the first rounds); once no ear is left
    // they do not (a corner touching the cut diagonal leaves the rest weakly simple).
    let mut touch = true;
    let is_ear = |a: usize,
                  b: usize,
                  c: usize,
                  reflex: &[usize],
                  flag: &[bool],
                  alive: &[bool],
                  touch: bool|
     -> bool {
        let (pa, pb, pc) = (p(a), p(b), p(c));
        if cross(pa, pb, pc) <= eps {
            return false;
        }
        for &k in reflex {
            if !alive[k] || !flag[k] || k == a || k == b || k == c {
                continue;
            }
            let q = p(k);
            if q != pa && q != pb && q != pc {
                let e = if touch { -eps } else { eps };
                let inside = cross(pa, pb, q) > e && cross(pb, pc, q) > e && cross(pc, pa, q) > e;
                if inside {
                    return false;
                }
            }
        }
        true
    };
    while left > 3 {
        let (a, b, c) = (prev[cur], cur, next[cur]);
        if is_ear(a, b, c, &reflex, &reflex_flag, &alive, touch) {
            out.push([ring[a], ring[b], ring[c]]);
            next[a] = c;
            prev[c] = a;
            alive[b] = false;
            for k in [a, c] {
                if reflex_flag[k] && cross(p(prev[k]), p(k), p(next[k])) > eps {
                    reflex_flag[k] = false;
                }
            }
            if reflex.len() > 64 && reflex.len() > 2 * left {
                reflex.retain(|&k| alive[k] && reflex_flag[k]);
            }
            left -= 1;
            cur = c;
            stall = 0;
            continue;
        }
        cur = next[cur];
        stall += 1;
        if stall > left {
            // No proper ear: cut a degenerate (collinear) corner if there is one, else the
            // rest as a fan of what remains.
            let mut k = cur;
            let mut cut = false;
            let rounds = left;
            for _ in 0..rounds {
                let (a, b, c) = (prev[k], k, next[k]);
                if cross(p(a), p(b), p(c)).abs() <= eps {
                    next[a] = c;
                    prev[c] = a;
                    alive[k] = false;
                    left -= 1;
                    cur = c;
                    cut = true;
                    break;
                }
                k = next[k];
            }
            if !cut && touch {
                touch = false;
                stall = 0;
                continue;
            }
            if !cut {
                let start = cur;
                let mut k = next[start];
                while next[k] != start {
                    out.push([ring[start], ring[k], ring[next[k]]]);
                    k = next[k];
                }
                return;
            }
            stall = 0;
        }
    }
    let (a, b, c) = (prev[cur], cur, next[cur]);
    if cross(p(a), p(b), p(c)) > 0.0 {
        out.push([ring[a], ring[b], ring[c]]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(pts: &[P2], t: &[[u32; 3]]) -> f64 {
        t.iter()
            .map(|t| {
                let a = cross(pts[t[0] as usize], pts[t[1] as usize], pts[t[2] as usize]) / 2.0;
                // Clockwise triangles would be overlaps: counted against.
                if a < -1e-9 { f64::NAN } else { a }
            })
            .sum()
    }

    #[test]
    fn square_with_hole() {
        let outer = vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]];
        let hole = vec![[1.0, 1.0], [1.0, 3.0], [3.0, 3.0], [3.0, 1.0]];
        let t = triangulate(&outer, std::slice::from_ref(&hole));
        let mut all = outer.clone();
        all.extend(hole);
        assert!((area(&all, &t) - 12.0).abs() < 1e-9, "{}", area(&all, &t));
        assert!(t.iter().all(|t| cross(
            all[t[0] as usize],
            all[t[1] as usize],
            all[t[2] as usize]
        ) > 0.0));
    }

    /// A beam's flange with four bolt holes, two at each end, a pair on one line along the
    /// beam (Holter tower, ISSUE_053): bridges must not cross.
    #[test]
    fn bolt_holes_in_pairs() {
        let outer = vec![
            [5720.0, -150.0],
            [380.0, -150.0],
            [380.0, 150.0],
            [5720.0, 150.0],
        ];
        let circle = |cx: f64, cy: f64| -> Vec<P2> {
            (0..16)
                .map(|k| {
                    let a = k as f64 * std::f64::consts::TAU / 16.0;
                    [cx + 21.0 * a.cos(), cy + 21.0 * a.sin()]
                })
                .collect()
        };
        let holes = vec![
            circle(475.0, 80.0),
            circle(475.0, -80.0),
            circle(5625.0, -80.0),
            circle(5625.0, 80.0),
        ];
        let t = triangulate(&outer, &holes);
        let mut all = outer.clone();
        for h in &holes {
            all.extend_from_slice(h);
        }
        let hole = 8.0 * 21.0 * 21.0 * (std::f64::consts::TAU / 16.0).sin();
        let want = 5340.0 * 300.0 - 4.0 * hole;
        assert!(
            (area(&all, &t) - want).abs() < 1e-6,
            "{} vs {want}",
            area(&all, &t)
        );
    }

    #[test]
    fn two_holes_and_clockwise_outer() {
        let outer = vec![[0.0, 0.0], [0.0, 2.0], [6.0, 2.0], [6.0, 0.0]];
        let h1 = vec![[1.0, 0.5], [2.0, 0.5], [2.0, 1.5], [1.0, 1.5]];
        let h2 = vec![[4.0, 0.5], [5.0, 0.5], [5.0, 1.5], [4.0, 1.5]];
        let t = triangulate(&outer, &[h1.clone(), h2.clone()]);
        let mut all = outer.clone();
        all.extend(h1);
        all.extend(h2);
        assert!((area(&all, &t) - 10.0).abs() < 1e-9, "{}", area(&all, &t));
    }

    /// A wall face with a notch from its left edge and five window holes (web-ifc's
    /// "noisy face" fixture).
    #[test]
    fn notched_face_with_holes() {
        let outer = vec![
            [24.99998093, 980.0],
            [1775.0, 980.0],
            [1775.0, 3380.0],
            [24.99998093, 3380.0],
            [25.0, 3550.0],
            [10109.824, 3550.0],
            [10109.824, 250.0],
            [25.0, 250.0],
        ];
        let holes: Vec<Vec<P2>> = [2243.0, 3743.0, 5243.0, 6743.0, 8243.0]
            .iter()
            .map(|&x| {
                vec![
                    [x, 3380.0],
                    [x, 980.0],
                    [x + 1032.0, 980.0],
                    [x + 1032.0, 3380.0],
                ]
            })
            .collect();
        let t = triangulate(&outer, &holes);
        let mut all = outer.clone();
        for h in &holes {
            all.extend_from_slice(h);
        }
        let want = 10084.824 * 3300.0 - 1750.0 * 2400.0 - 5.0 * 1032.0 * 2400.0;
        let got = area(&all, &t);
        assert!((got - want).abs() < 1.0, "{got} {want}");
        // The same with the axes swapped (bridges run the other way).
        let sw = |r: &[P2]| -> Vec<P2> { r.iter().map(|p| [p[1], p[0]]).collect() };
        let holes2: Vec<Vec<P2>> = holes.iter().map(|h| sw(h)).collect();
        let t = triangulate(&sw(&outer), &holes2);
        let got = area(&sw(&all), &t).abs();
        assert!((got - want).abs() < 1.0, "swapped: {got} {want}");
        // As a face's plane frame gives it: from the first corner, with noise.
        let o = [980.0, 24.99998093];
        let fr = |r: &[P2]| -> Vec<P2> {
            r.iter()
                .map(|p| [p[1] - o[0] + 1e-26, p[0] - o[1] + 1e-31])
                .collect()
        };
        let holes3: Vec<Vec<P2>> = holes.iter().map(|h| fr(h)).collect();
        let t = triangulate(&fr(&outer), &holes3);
        let got = area(&fr(&all), &t).abs();
        assert!((got - want).abs() < 1.0, "framed: {got} {want}");
    }

    #[test]
    fn concave_and_degenerate() {
        // A U shape with a collinear corner.
        let outer = vec![
            [0.0, 0.0],
            [3.0, 0.0],
            [3.0, 3.0],
            [2.0, 3.0],
            [2.0, 1.0],
            [1.0, 1.0],
            [1.0, 3.0],
            [0.0, 3.0],
            [0.0, 1.5],
        ];
        let t = triangulate(&outer, &[]);
        assert!(
            (area(&outer, &t) - 7.0).abs() < 1e-9,
            "{}",
            area(&outer, &t)
        );
    }
}
