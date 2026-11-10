//! Finding Aztec symbols: see the module notes in `aztec`.

use super::content::Content;
use super::{
    Grid, Symbol, decode_mode, decode_rune, mode_positions, orientation_marks, read, size_of,
};
use crate::dm::detect::{corners, hull_rows};
use crate::image::{Binary, Homography, Runs};

type P = (f64, f64);

/// A bull's-eye's centre and module size, and how many rows found it.
#[derive(Clone, Copy, Debug)]
struct Eye {
    x: f64,
    y: f64,
    module: f64,
    count: u32,
}

/// Whether seven run lengths are about equal: each within half its mean.
fn even(v: &[f64; 7]) -> Option<f64> {
    let m = v.iter().sum::<f64>() / 7.0;
    (m >= 1.0 && v.iter().all(|&x| x >= 0.5 * m && x <= 1.5 * m)).then_some(m)
}

/// Runs through the dark pixel (x, y) along (dx, dy): the light, dark and light runs before
/// and after the dark centre, each at most `max` steps, with dark beyond them on both sides.
/// Returns the seven inner runs (in steps) and the centre's offset along the line.
fn rings(b: &Binary, x: f64, y: f64, dx: i32, dy: i32, max: u32) -> Option<([f64; 7], f64)> {
    let (x, y) = (x as i32, y as i32);
    let (w, h) = (b.w as i32, b.h as i32);
    let at = |k: i32| {
        let (px, py) = (x + dx * k, y + dy * k);
        (px >= 0 && py >= 0 && px < w && py < h).then(|| b.dark[(py * w + px) as usize])
    };
    if at(0) != Some(true) {
        return None;
    }
    let mut side = [[0u32; 4]; 2];
    let mut centre = [0i32; 2];
    for (s, dir) in [(0usize, -1i32), (1, 1)] {
        let mut k = 0i32;
        while at(k) == Some(true) {
            k += dir;
            if k.unsigned_abs() > 2 * max {
                return None;
            }
        }
        centre[s] = k.abs();
        // light, dark, light, then some dark
        for (i, want) in [false, true, false, true].into_iter().enumerate() {
            let mut n = 0;
            while at(k) == Some(want) {
                n += 1;
                k += dir;
                if n > max {
                    if i == 3 {
                        break;
                    }
                    return None;
                }
            }
            if n == 0 {
                return None;
            }
            side[s][i] = n;
        }
    }
    let c = (centre[0] + centre[1] - 1) as f64;
    let f = |v: u32| f64::from(v);
    let v = [
        f(side[0][2]),
        f(side[0][1]),
        f(side[0][0]),
        c,
        f(side[1][0]),
        f(side[1][1]),
        f(side[1][2]),
    ];
    // the centre run spans -centre[0]+1 ..= centre[1]-1
    let off = f64::from(centre[1] - centre[0]) / 2.0;
    Some((v, off))
}

/// Bull's-eye candidates: rows with five dark runs whose middle seven runs are even.
fn eyes(b: &Binary, runs: &Runs) -> Vec<Eye> {
    let mut found: Vec<Eye> = Vec::new();
    for y in 0..b.h {
        for w in runs.row(y).windows(5) {
            // quick: the seven middle runs within a factor three of each other, and wide
            // enough to read (rings of a pixel or so are noise)
            let v = [
                w[1].0 - w[0].1 - 1,
                w[1].1 - w[1].0 + 1,
                w[2].0 - w[1].1 - 1,
                w[2].1 - w[2].0 + 1,
                w[3].0 - w[2].1 - 1,
                w[3].1 - w[3].0 + 1,
                w[4].0 - w[3].1 - 1,
            ];
            let (lo, hi) = v
                .iter()
                .fold((u32::MAX, 0), |a, &x| (a.0.min(x), a.1.max(x)));
            if hi > 3 * lo || v.iter().sum::<u32>() < 11 {
                continue;
            }
            let len = |i: usize| f64::from(w[i].1 - w[i].0 + 1);
            let gap = |i: usize| f64::from(w[i + 1].0 - w[i].1 - 1);
            let inner = [gap(0), len(1), gap(1), len(2), gap(2), len(3), gap(3)];
            let Some(m) = even(&inner) else { continue };
            if len(0) < 0.5 * m || len(4) < 0.5 * m {
                continue;
            }
            let cx = f64::from(w[2].0 + w[2].1 + 1) / 2.0;
            if let Some(e) = confirm(b, cx, y as f64 + 0.5, m) {
                add(&mut found, e);
            }
        }
    }
    found.sort_by_key(|e| std::cmp::Reverse(e.count));
    found
}

/// Checks a row's candidate down, across again and along a diagonal; its centre.
fn confirm(b: &Binary, cx: f64, cy: f64, m: f64) -> Option<Eye> {
    let max = (2.0 * m).ceil() as u32 + 1;
    let (v, off) = rings(b, cx, cy, 0, 1, max)?;
    let mv = even(&v)?;
    if !(0.6..=1.6).contains(&(mv / m)) {
        return None;
    }
    let cy = cy.floor() + 0.5 + off;
    let (h, off) = rings(b, cx, cy, 1, 0, max)?;
    let mh = even(&h)?;
    let cx = cx.floor() + 0.5 + off;
    // Along a diagonal the rings cross at their corners, which blur rounds: only all there
    // and of like widths.
    let (d, _) = rings(b, cx, cy, 1, 1, 2 * max)?;
    let (lo, hi) = d
        .iter()
        .fold((f64::MAX, 0.0f64), |a, &v| (a.0.min(v), a.1.max(v)));
    if hi > 4.0 * lo {
        return None;
    }
    Some(Eye {
        x: cx,
        y: cy,
        module: (mv + mh) / 2.0,
        count: 1,
    })
}

fn add(found: &mut Vec<Eye>, e: Eye) {
    for g in found.iter_mut() {
        if (g.x - e.x).abs() <= g.module.max(e.module)
            && (g.y - e.y).abs() <= g.module.max(e.module)
        {
            let n = f64::from(g.count);
            g.x = (g.x * n + e.x) / (n + 1.0);
            g.y = (g.y * n + e.y) / (n + 1.0);
            g.module = (g.module * n + e.module) / (n + 1.0);
            g.count += 1;
            return;
        }
    }
    found.push(e);
}

/// The region of `want` pixels through pixel (x, y), as rows (y, first x, last x), if it
/// stays inside
/// the box (x0, y0)–(x1, y1) (exclusive ends).
fn flood(
    b: &Binary,
    x: usize,
    y: usize,
    want: bool,
    bx: [usize; 4],
) -> Option<Vec<(usize, usize, usize)>> {
    let [x0, y0, x1, y1] = bx;
    if b.get(x, y) != want || x <= x0 || y <= y0 || x + 1 >= x1 || y + 1 >= y1 {
        return None;
    }
    let (w, h) = (x1 - x0, y1 - y0);
    let mut seen = vec![false; w * h];
    let mut stack = vec![(x, y)];
    seen[(y - y0) * w + x - x0] = true;
    let mut ext: Vec<(usize, usize)> = vec![(usize::MAX, 0); h];
    while let Some((px, py)) = stack.pop() {
        let e = &mut ext[py - y0];
        e.0 = e.0.min(px);
        e.1 = e.1.max(px);
        for (dx, dy) in [
            (-1i32, -1i32),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ] {
            let (nx, ny) = (px as i32 + dx, py as i32 + dy);
            let (nx, ny) = (nx as usize, ny as usize);
            if b.get(nx, ny) != want {
                continue;
            }
            // touching the box: not a closed ring
            if nx <= x0 || ny <= y0 || nx + 1 >= x1 || ny + 1 >= y1 {
                return None;
            }
            let i = (ny - y0) * w + nx - x0;
            if !seen[i] {
                seen[i] = true;
                stack.push((nx, ny));
            }
        }
    }
    Some(
        ext.iter()
            .enumerate()
            .filter(|(_, e)| e.0 != usize::MAX)
            .map(|(i, e)| (y0 + i, e.0, e.1))
            .collect(),
    )
}

/// The outline of the ring `n` modules out from the centre (light for odd `n`), found
/// `n` runs out along `dir` (a unit step): clockwise on screen, the square [-n, n + 1) in
/// modules.
fn ring(b: &Binary, e: &Eye, n: usize, dir: P) -> Option<[P; 4]> {
    // Runs along the ray, in quarter pixels; runs under a third of a module merged away.
    let at = |t: f64| b.at((e.x + dir.0 * t) as f32, (e.y + dir.1 * t) as f32);
    let end = (2 * n + 4) as f64 * e.module;
    let mut runs: Vec<(bool, f64, f64)> = Vec::new();
    let mut t = 0.0;
    while t < end {
        let v = at(t)?;
        match runs.last_mut() {
            Some(r) if r.0 == v => r.2 = t,
            _ => runs.push((v, t, t)),
        }
        t += 0.25;
    }
    let mut clean: Vec<(bool, f64, f64)> = Vec::new();
    for r in runs {
        if r.2 - r.1 + 0.25 < e.module / 3.0 && !clean.is_empty() {
            continue;
        }
        match clean.last_mut() {
            Some(l) if l.0 == r.0 => l.2 = r.2,
            _ => clean.push(r),
        }
    }
    let &(want, t0, t1) = clean.get(n)?;
    if want != n.is_multiple_of(2) {
        return None;
    }
    let t = (t0 + t1) / 2.0;
    let (x, y) = ((e.x + dir.0 * t) as usize, (e.y + dir.1 * t) as usize);
    let r = (1.5 * n as f64 + 3.0) * e.module;
    let bx = [
        (e.x - r).max(0.0) as usize,
        (e.y - r).max(0.0) as usize,
        ((e.x + r) as usize).min(b.w),
        ((e.y + r) as usize).min(b.h),
    ];
    let rows = flood(b, x, y, n.is_multiple_of(2), bx)?;
    let q = corners(&hull_rows(&rows))?;
    // about the size it should be
    let side = (q[0].0 - q[2].0).hypot(q[0].1 - q[2].1) / std::f64::consts::SQRT_2;
    let want = (2 * n + 1) as f64 * e.module;
    (side > 0.6 * want && side < 1.5 * want).then_some(q)
}

/// The quadrilateral `q` (clockwise on screen) as the square of half-width `r` modules
/// around the centre module, turned by `k` and mirrored or not: four point pairs.
fn square(q: &[P; 4], r: f64, k: usize, mirrored: bool) -> [(P, P); 4] {
    let module = [(-r, -r), (r + 1.0, -r), (r + 1.0, r + 1.0), (-r, r + 1.0)];
    std::array::from_fn(|i| {
        let at = if mirrored {
            q[(k + 4 - i) % 4]
        } else {
            q[(k + i) % 4]
        };
        (module[i], at)
    })
}

/// Module (u, v) (relative to the centre module) dark, by a vote of five taps.
fn sample(b: &Binary, h: &Homography, u: f64, v: f64) -> Option<bool> {
    let mut dark = 0;
    for (dx, dy) in [(0.5, 0.5), (0.3, 0.3), (0.7, 0.3), (0.3, 0.7), (0.7, 0.7)] {
        let (x, y) = h.map(u + dx, v + dy);
        dark += usize::from(b.at(x as f32, y as f32)?);
    }
    Some(dark >= 3)
}

/// The share of the ring `d` modules out that is `want`.
fn ring_share(b: &Binary, h: &Homography, d: i32, want: bool) -> f64 {
    let (mut ok, mut n) = (0, 0);
    for t in -d..d {
        for (u, v) in [(t, -d), (d, t), (-t, d), (-d, -t)] {
            n += 1;
            ok += usize::from(sample(b, h, f64::from(u), f64::from(v)) == Some(want));
        }
    }
    ok as f64 / f64::from(n.max(1))
}

/// The pixel point of the reference grid crossing at module (u, v) near where `h` puts it,
/// or `None` when the grid's pattern is not there.
fn crossing(b: &Binary, h: &Homography, u: f64, v: f64) -> Option<P> {
    let taps = [
        (0.0, 0.0, true),
        (1.0, 0.0, false),
        (-1.0, 0.0, false),
        (0.0, 1.0, false),
        (0.0, -1.0, false),
        (2.0, 0.0, true),
        (-2.0, 0.0, true),
        (0.0, 2.0, true),
        (0.0, -2.0, true),
    ];
    let mut best = (0, 0.0, 0.0, 0.0);
    for i in -4..=4 {
        for j in -4..=4 {
            let (du, dv) = (f64::from(i) / 8.0, f64::from(j) / 8.0);
            let mut s = 0;
            for &(a, c, want) in &taps {
                let (x, y) = h.map(u + 0.5 + du + a, v + 0.5 + dv + c);
                s += usize::from(b.at(x as f32, y as f32) == Some(want));
            }
            if s > best.0 {
                best = (s, du, dv, 1.0);
            } else if s == best.0 {
                // the middle of the best offsets
                best.1 += du;
                best.2 += dv;
                best.3 += 1.0;
            }
        }
    }
    if best.0 < taps.len() {
        return None;
    }
    let (du, dv) = (best.1 / best.3, best.2 / best.3);
    Some(h.map(u + 0.5 + du, v + 0.5 + dv))
}

/// The symbol's outline from its pixels near where `h` puts it: corners matched to the
/// predicted ones, as point pairs.
fn outline(b: &Binary, h: &Homography, lo: f64, hi: f64, module: f64) -> Option<[(P, P); 4]> {
    let want = [(lo, lo), (hi, lo), (hi, hi), (lo, hi)];
    let grow = 1.5;
    let q: [P; 4] = [
        (lo - grow, lo - grow),
        (hi + grow, lo - grow),
        (hi + grow, hi + grow),
        (lo - grow, hi + grow),
    ]
    .map(|(u, v)| h.map(u, v));
    let ys = q.iter().map(|p| p.1);
    let y0 = ys.clone().fold(f64::INFINITY, f64::min).max(0.0) as usize;
    let y1 = (ys.fold(f64::NEG_INFINITY, f64::max).ceil() as usize).min(b.h.saturating_sub(1));
    let mut rows = Vec::new();
    for y in y0..=y1 {
        let Some((a, c)) = crate::dm::detect::span(&q, y as f64 + 0.5) else {
            continue;
        };
        if c < 0.5 || a - 0.5 >= b.w as f64 {
            continue;
        }
        let xa = (a - 0.5).ceil().max(0.0) as usize;
        let xb = ((c - 0.5).floor() as usize).min(b.w - 1);
        if xa > xb {
            continue;
        }
        let row = &b.dark[y * b.w + xa..=y * b.w + xb];
        if let Some(f) = row.iter().position(|&d| d) {
            let l = row.iter().rposition(|&d| d).unwrap_or(f);
            rows.push((y, xa + f, xa + l));
        }
    }
    let got = corners(&hull_rows(&rows))?;
    let mut out = [((0.0, 0.0), (0.0, 0.0)); 4];
    for (i, &m) in want.iter().enumerate() {
        let p = h.map(m.0, m.1);
        let near = got.iter().copied().min_by(|a, c| {
            let da = (a.0 - p.0).hypot(a.1 - p.1);
            let dc = (c.0 - p.0).hypot(c.1 - p.1);
            da.total_cmp(&dc)
        })?;
        if (near.0 - p.0).hypot(near.1 - p.1) > 1.5 * module {
            return None;
        }
        out[i] = (m, near);
    }
    Some(out)
}

/// The symbol's grid sampled through `h`, `size` modules a side.
fn grid(b: &Binary, h: &Homography, size: usize) -> Grid {
    let c = (size / 2) as f64;
    let mut g = Grid {
        size,
        dark: vec![false; size * size],
    };
    for y in 0..size {
        for x in 0..size {
            g.dark[y * size + x] = sample(b, h, x as f64 - c, y as f64 - c).unwrap_or(false);
        }
    }
    g
}

/// Rays out from the centre, for the ring edges.
const RAYS: usize = 32;

/// Along RAYS rays out from `o`, the first `bounds` changes between dark and light (runs
/// under a third of a module left out): for each j = 1..=bounds the pixel points of the
/// j-th change.
fn ray_edges(b: &Binary, o: P, module: f64, bounds: usize) -> Vec<Vec<P>> {
    let mut out = vec![Vec::new(); bounds];
    for i in 0..RAYS {
        let a = i as f64 * std::f64::consts::TAU / RAYS as f64;
        let dir = (a.cos(), a.sin());
        let at = |t: f64| b.at((o.0 + dir.0 * t) as f32, (o.1 + dir.1 * t) as f32);
        let end = (bounds as f64 + 1.0) * module * 1.6;
        // runs along the ray in quarter pixels
        let mut runs: Vec<(bool, f64, f64)> = Vec::new();
        let mut t = 0.0;
        while t < end {
            let Some(v) = at(t) else { break };
            match runs.last_mut() {
                Some(r) if r.0 == v => r.2 = t,
                _ => runs.push((v, t, t)),
            }
            t += 0.25;
        }
        let mut clean: Vec<(bool, f64, f64)> = Vec::new();
        for r in runs {
            if r.2 - r.1 + 0.25 < module / 3.0 && !clean.is_empty() {
                continue;
            }
            match clean.last_mut() {
                Some(l) if l.0 == r.0 => l.2 = r.2,
                _ => clean.push(r),
            }
        }
        if clean.first().is_none_or(|r| !r.0) {
            continue;
        }
        for j in 1..=bounds.min(clean.len() - 1) {
            let t = (clean[j - 1].2 + clean[j].1) / 2.0;
            out[j - 1].push((o.0 + dir.0 * t, o.1 + dir.1 * t));
        }
    }
    out
}

/// The outline of the third ring edge (the square [-2, 3) in modules) from rays out of the
/// eye: for when the ring's pixels do not stand apart (small modules).
fn ray_ring(b: &Binary, e: &Eye) -> Option<[P; 4]> {
    let mut pts = ray_edges(b, (e.x, e.y), e.module, 3).pop()?;
    if pts.len() < RAYS * 3 / 4 {
        return None;
    }
    pts.sort_by(|a, c| a.1.total_cmp(&c.1).then(a.0.total_cmp(&c.0)));
    let q = corners(&crate::dm::detect::chain(&pts))?;
    // a square five modules a side
    let side = |i: usize| (q[i].0 - q[(i + 1) % 4].0).hypot(q[i].1 - q[(i + 1) % 4].1);
    let want = 5.0 * e.module;
    (0..4)
        .all(|i| side(i) > 0.6 * want && side(i) < 1.5 * want)
        .then_some(q)
}

/// Refines `h` (relative module coordinates to pixels) with the bull's-eye's ring edges:
/// along rays out from the centre, the first `bounds` changes between dark and light are
/// the edges of the squares [-j + 1, j) in modules, j = 1..=bounds.
fn ring_fit(b: &Binary, h: &Homography, module: f64, bounds: usize) -> Option<Homography> {
    let mut h = *h;
    for _ in 0..3 {
        let inv = h.inverse();
        let mut pairs = Vec::new();
        for (j, pts) in ray_edges(b, h.map(0.5, 0.5), module, bounds)
            .iter()
            .enumerate()
        {
            let half = j as f64 + 0.5;
            for &p in pts {
                let q = inv.map(p.0, p.1);
                // the nearest point of the square's edge, about the centre (0.5, 0.5)
                let (x, y) = (q.0 - 0.5, q.1 - 0.5);
                let s = if x.abs() > y.abs() {
                    (half * x.signum(), y.clamp(-half, half))
                } else {
                    (x.clamp(-half, half), half * y.signum())
                };
                if (s.0 - x).hypot(s.1 - y) < 0.4 {
                    pairs.push(((s.0 + 0.5, s.1 + 0.5), p));
                }
            }
        }
        // most rays cross every edge where it should be
        if pairs.len() < RAYS * bounds / 2 {
            return None;
        }
        h = Homography::fit(&pairs)?;
    }
    Some(h)
}

/// The eight symmetries of the square about the centre module: turned `k` quarters,
/// mirrored or not.
fn symmetry(k: usize, mirrored: bool) -> Homography {
    let (c, s) = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)][k];
    let m = if mirrored { -1.0 } else { 1.0 };
    // p - 0.5 → R (mirror (p - 0.5)) + 0.5
    let (a, b, d, e) = (c * m, -s, s * m, c);
    Homography::affine(a, b, 0.5 - 0.5 * (a + b), d, e, 0.5 - 0.5 * (d + e))
}

/// Reads the symbol around one bull's-eye.
fn read_eye(b: &Binary, e: &Eye) -> Option<Symbol> {
    let inner = ring(b, e, 2, (1.0, 0.0)).or_else(|| ray_ring(b, e))?;
    let h0 = Homography::fit(&square(&inner, 2.0, 0, false))?;
    // compact's closed rings reach four modules out, full-range's six
    let hc = ring_fit(b, &h0, e.module, 4)?;
    let full = ring_share(b, &hc, 5, false) >= 0.75 && ring_share(b, &hc, 6, true) >= 0.75;
    let hf = if full {
        ring_fit(b, &hc, e.module, 6)
    } else {
        None
    };
    // Which turns and mirrorings the orientation marks allow, best first, for each kind.
    let mut cands: Vec<(usize, bool, bool, Homography)> = Vec::new();
    for (compact, h) in [(true, Some(hc)), (false, hf)] {
        let Some(h) = h else { continue };
        for k in 0..4 {
            for mirrored in [false, true] {
                let h = symmetry(k, mirrored).then(&h);
                let score = orientation_marks(compact)
                    .iter()
                    .filter(|&&(u, v, want)| {
                        sample(b, &h, f64::from(u), f64::from(v)) == Some(want)
                    })
                    .count();
                if score >= 10 {
                    cands.push((score, compact, mirrored, h));
                }
            }
        }
    }
    cands.sort_by_key(|c| std::cmp::Reverse(c.0));
    let corners_of = |h: &Homography, size: usize| {
        let (lo, hi) = (-((size / 2) as f64), (size - size / 2) as f64);
        [(lo, lo), (hi, lo), (hi, hi), (lo, hi)].map(|(u, v)| {
            let p = h.map(u, v);
            (p.0 as f32, p.1 as f32)
        })
    };
    for (score, compact, mirrored, h) in cands {
        let bits: Option<Vec<bool>> = mode_positions(compact)
            .iter()
            .map(|&(u, v)| sample(b, &h, f64::from(u), f64::from(v)))
            .collect();
        let Some(bits) = bits else { continue };
        if let Some((layers, words, fixed)) = decode_mode(&bits, compact) {
            let size = size_of(compact, layers);
            for h in refined(b, &h, compact, size, e.module) {
                let g = grid(b, &h, size);
                if let Ok((content, corrected)) = read(&g, compact, layers, words) {
                    return Some(Symbol {
                        compact,
                        layers,
                        size,
                        rune: None,
                        content,
                        corners: corners_of(&h, size),
                        mirrored,
                        corrected: corrected + fixed,
                    });
                }
            }
        }
        // A rune: its marks all there and light around it (a bull's-eye in another
        // symbol's data is not one).
        if compact
            && score == 12
            && ring_share(b, &h, 6, false) >= 0.75
            && let Some((v, fixed)) = decode_rune(&bits)
        {
            return Some(Symbol {
                compact,
                layers: 0,
                size: 11,
                rune: Some(v),
                content: Content {
                    text: v.to_string(),
                    bytes: vec![v],
                    ..Content::default()
                },
                corners: corners_of(&h, 11),
                mirrored,
                corrected: fixed,
            });
        }
    }
    None
}

/// Transforms to try for a symbol of `size`: refined with its outline and, full-range, the
/// reference grid's crossings, then the bull's-eye's own.
fn refined(b: &Binary, h: &Homography, compact: bool, size: usize, module: f64) -> Vec<Homography> {
    let c = (size / 2) as f64;
    let r = if compact { 3.0 } else { 5.0 };
    // the bull's-eye's corners as `h` has them (its own fit)
    let mut pairs: Vec<(P, P)> = [(-r, -r), (r + 1.0, -r), (r + 1.0, r + 1.0), (-r, r + 1.0)]
        .iter()
        .map(|&(u, v)| ((u, v), h.map(u, v)))
        .collect();
    let mut cur = *h;
    if !compact {
        // crossings ring by ring outwards, refitting after each
        let n = (size / 2) as i32 / 16;
        for ringno in 1..=n {
            for a in -ringno..=ringno {
                for bb in -ringno..=ringno {
                    if a.abs().max(bb.abs()) != ringno {
                        continue;
                    }
                    let (u, v) = (f64::from(16 * a), f64::from(16 * bb));
                    if let Some(p) = crossing(b, &cur, u, v) {
                        pairs.push(((u + 0.5, v + 0.5), p));
                    }
                }
            }
            if let Some(f) = Homography::fit(&pairs) {
                cur = f;
            }
        }
    }
    let mut out = Vec::new();
    if let Some(o) = outline(b, &cur, -c, size as f64 - c, module) {
        pairs.extend(o);
        if let Some(f) = Homography::fit(&pairs) {
            out.push(f);
        }
    }
    out.push(cur);
    if !compact {
        out.push(*h);
    }
    out
}

pub(crate) fn find(b: &Binary, runs: &Runs, skip: &[[P; 4]]) -> Vec<Symbol> {
    let mut out: Vec<Symbol> = Vec::new();
    let mut taken: Vec<[P; 4]> = skip.to_vec();
    for e in eyes(b, runs) {
        if taken.iter().any(|q| inside(q, (e.x, e.y))) {
            continue;
        }
        if let Some(s) = read_eye(b, &e) {
            taken.push(s.corners.map(|(x, y)| (f64::from(x), f64::from(y))));
            out.push(s);
        }
    }
    out
}

fn inside(q: &[P; 4], p: P) -> bool {
    let cross = |o: P, a: P| (a.0 - o.0) * (p.1 - o.1) - (a.1 - o.1) * (p.0 - o.0);
    let s: Vec<f64> = (0..4).map(|i| cross(q[i], q[(i + 1) % 4])).collect();
    s.iter().all(|&v| v >= 0.0) || s.iter().all(|&v| v <= 0.0)
}
