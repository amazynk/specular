//! Finding Data Matrix symbols: see the module notes in `dm`.

use super::tables::{SIZES, Size};
use super::{Grid, Symbol, read};
use crate::image::{Binary, Gray, Homography, Runs};

type P = (f64, f64);

/// Dark pixels a candidate region has at least.
const MIN_PIXELS: usize = 30;
/// Candidate regions looked at, largest first, at most.
const MAX_REGIONS: usize = 400;
/// A size is read when this share of its finder and timing modules match.
const MIN_SCORE: f64 = 0.8;

/// A dark region: its pixel count and each row's leftmost and rightmost pixel.
struct Region {
    pixels: usize,
    rows: Vec<(usize, usize, usize)>,
}

fn root(parent: &mut [u32], mut i: u32) -> u32 {
    while parent[i as usize] != i {
        parent[i as usize] = parent[parent[i as usize] as usize];
        i = parent[i as usize];
    }
    i
}

/// The span of row `y` (pixel centres) inside the convex quadrilateral `q`, if any.
pub(crate) fn span(q: &[P; 4], y: f64) -> Option<(f64, f64)> {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for i in 0..4 {
        let (a, c) = (q[i], q[(i + 1) % 4]);
        if (a.1 <= y && y <= c.1) || (c.1 <= y && y <= a.1) {
            let x = if (c.1 - a.1).abs() < 1e-9 {
                lo = lo.min(a.0.min(c.0));
                a.0.max(c.0)
            } else {
                a.0 + (y - a.1) / (c.1 - a.1) * (c.0 - a.0)
            };
            lo = lo.min(x);
            hi = hi.max(x);
        }
    }
    (lo < hi).then_some((lo, hi))
}

/// Dark regions (8-connected), from runs of dark pixels joined row to row, leaving out the
/// runs wholly inside the `skip` quadrilaterals (codes already read); and the number of
/// specks (regions of at most four pixels).
fn regions(b: &Binary, all: &Runs, skip: &[[P; 4]]) -> (Vec<Region>, usize) {
    let (w, h) = (b.w, b.h);
    // Without the runs wholly inside a code already read.
    let kept;
    let runs = if skip.is_empty() {
        all
    } else {
        let mut k = Runs::default();
        let mut spans: Vec<(u32, u32)> = Vec::with_capacity(skip.len());
        for y in 0..h {
            k.row.push(k.runs.len() as u32);
            spans.clear();
            spans.extend(
                skip.iter()
                    .filter_map(|q| span(q, y as f64 + 0.5))
                    .filter(|&(lo, hi)| hi >= 0.5 && lo - 0.5 < w as f64)
                    .map(|(lo, hi)| {
                        let lo = (lo - 0.5).ceil().max(0.0) as u32;
                        (lo, ((hi - 0.5).floor() as u32).min(w as u32))
                    })
                    .filter(|&(lo, hi)| lo <= hi),
            );
            k.runs.extend(
                all.row(y)
                    .iter()
                    .filter(|&&(a, c)| !spans.iter().any(|&(lo, hi)| lo <= a && c <= hi)),
            );
        }
        k.row.push(k.runs.len() as u32);
        kept = k;
        &kept
    };
    let n = runs.runs.len();
    let mut parent: Vec<u32> = (0..n as u32).collect();
    for y in 1..h {
        let (mut i, prev_end) = (runs.row[y - 1] as usize, runs.row[y] as usize);
        for k in runs.row[y] as usize..runs.row[y + 1] as usize {
            let (a, c) = runs.runs[k];
            // Runs of the row above touching this one (diagonals count).
            while i < prev_end && runs.runs[i].1 + 1 < a {
                i += 1;
            }
            let mut j = i;
            while j < prev_end && runs.runs[j].0 <= c + 1 {
                let rj = root(&mut parent, j as u32);
                if parent[k] == k as u32 {
                    // the first: this run joins its region
                    parent[k] = rj;
                } else {
                    let rk = root(&mut parent, k as u32);
                    if rk != rj {
                        parent[rk.max(rj) as usize] = rk.min(rj);
                    }
                }
                j += 1;
            }
        }
    }
    // Pixels of each region (at its root), then rows only for the regions big enough.
    let mut pixels = vec![0u32; n];
    for k in 0..n {
        let r = root(&mut parent, k as u32);
        parent[k] = r;
        pixels[r as usize] += runs.runs[k].1 - runs.runs[k].0 + 1;
    }
    let mut specks = 0;
    let mut index = vec![u32::MAX; n];
    let mut out: Vec<Region> = Vec::new();
    for y in 0..h {
        for k in runs.row[y] as usize..runs.row[y + 1] as usize {
            let r = parent[k] as usize;
            let n = pixels[r] as usize;
            if n < MIN_PIXELS {
                specks += usize::from(n <= 4 && r == k);
                continue;
            }
            if index[r] == u32::MAX {
                index[r] = out.len() as u32;
                out.push(Region {
                    pixels: n,
                    rows: Vec::new(),
                });
            }
            let (a, c) = (runs.runs[k].0 as usize, runs.runs[k].1 as usize);
            let reg = &mut out[index[r] as usize];
            match reg.rows.last_mut() {
                Some(last) if last.0 == y => {
                    last.1 = last.1.min(a);
                    last.2 = last.2.max(c);
                }
                _ => reg.rows.push((y, a, c)),
            }
        }
    }
    out.retain(|r| r.rows.len() >= 6);
    out.sort_by_key(|r| std::cmp::Reverse(r.pixels));
    out.truncate(MAX_REGIONS);
    (out, specks)
}

fn cross(o: P, a: P, b: P) -> f64 {
    (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
}

/// Convex hull (monotone chain) of points sorted by (y, x).
pub(crate) fn chain(pts: &[P]) -> Vec<P> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut lower: Vec<P> = Vec::with_capacity(pts.len());
    for &p in pts {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0 {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<P> = Vec::with_capacity(pts.len());
    for &p in pts.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0.0 {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// The convex hull of the pixels of rows (y, first x, last x) sorted by y: only each pixel
/// row's ends matter, and they come already in order, so no sorting.
pub(crate) fn hull_rows(rows: &[(usize, usize, usize)]) -> Vec<P> {
    // Leftmost and rightmost pixel corner on each corner row.
    let mut ext: Vec<(usize, usize, usize)> = Vec::with_capacity(rows.len() + 1);
    for &(y, a, b) in rows {
        for yy in [y, y + 1] {
            match ext.last_mut() {
                Some(l) if l.0 == yy => {
                    l.1 = l.1.min(a);
                    l.2 = l.2.max(b + 1);
                }
                _ => ext.push((yy, a, b + 1)),
            }
        }
    }
    let mut pts = Vec::with_capacity(ext.len() * 2);
    for (y, a, b) in ext {
        pts.push((a as f64, y as f64));
        if b != a {
            pts.push((b as f64, y as f64));
        }
    }
    // Sorted by (y, x): the chain is the hull of the mirror image, so the hull's own.
    chain(&pts)
}

/// The smallest rectangle around a hull: (unit u, unit v, [min u, max u, min v, max v]).
fn min_rect(h: &[P]) -> Option<(P, P, [f64; 4])> {
    let mut best: Option<(f64, P, P, [f64; 4])> = None;
    for i in 0..h.len() {
        let (a, b) = (h[i], h[(i + 1) % h.len()]);
        let l = (b.0 - a.0).hypot(b.1 - a.1);
        if l < 1e-9 {
            continue;
        }
        let u = ((b.0 - a.0) / l, (b.1 - a.1) / l);
        let v = (-u.1, u.0);
        let mut e = [
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ];
        for p in h {
            let (pu, pv) = (p.0 * u.0 + p.1 * u.1, p.0 * v.0 + p.1 * v.1);
            e[0] = e[0].min(pu);
            e[1] = e[1].max(pu);
            e[2] = e[2].min(pv);
            e[3] = e[3].max(pv);
        }
        let area = (e[1] - e[0]) * (e[3] - e[2]);
        if best.as_ref().is_none_or(|b| area < b.0) {
            best = Some((area, u, v, e));
        }
    }
    best.map(|(_, u, v, e)| (u, v, e))
}

/// A line as a point and a unit direction.
type Line = (P, P);

/// The line through hull edges, each weighted by its length (principal direction); `None`
/// without any.
fn fit(edges: &[(P, P)]) -> Option<Line> {
    let mut w = 0.0;
    let (mut cx, mut cy) = (0.0, 0.0);
    for &(a, b) in edges {
        let l = (b.0 - a.0).hypot(b.1 - a.1);
        w += l;
        cx += l * (a.0 + b.0) / 2.0;
        cy += l * (a.1 + b.1) / 2.0;
    }
    if w < 1e-9 {
        return None;
    }
    let (cx, cy) = (cx / w, cy / w);
    let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
    for &(a, b) in edges {
        let l = (b.0 - a.0).hypot(b.1 - a.1);
        // A segment's second moment about the centre: its ends' and its midpoint's.
        for (p, k) in [
            (a, 1.0 / 6.0),
            (b, 1.0 / 6.0),
            (((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0), 4.0 / 6.0),
        ] {
            let (dx, dy) = (p.0 - cx, p.1 - cy);
            sxx += l * k * dx * dx;
            sxy += l * k * dx * dy;
            syy += l * k * dy * dy;
        }
    }
    let a = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    Some(((cx, cy), (a.cos(), a.sin())))
}

fn meet(a: Line, b: Line) -> Option<P> {
    let ((p, d), (q, e)) = (a, b);
    let den = d.0 * e.1 - d.1 * e.0;
    if den.abs() < 1e-9 {
        return None;
    }
    let t = ((q.0 - p.0) * e.1 - (q.1 - p.1) * e.0) / den;
    Some((p.0 + t * d.0, p.1 + t * d.1))
}

/// The symbol's four corners, clockwise on screen, from a hull.
pub(crate) fn corners(h: &[P]) -> Option<[P; 4]> {
    let (u, v, e) = min_rect(h)?;
    let (lu, lv) = (e[1] - e[0], e[3] - e[2]);
    if lu < 6.0 || lv < 6.0 || lu.max(lv) / lu.min(lv) > 7.0 {
        return None;
    }
    let tol = 0.08 * lu.min(lv) + 1.0;
    // Sides: (outward normal, its extreme projection, the rectangle's own line).
    let pt = |a: f64, b: f64| (a * u.0 + b * v.0, a * u.1 + b * v.1);
    let sides = [
        ((-v.0, -v.1), -e[2], (pt(e[0], e[2]), u)),
        ((u.0, u.1), e[1], (pt(e[1], e[2]), v)),
        ((v.0, v.1), e[3], (pt(e[1], e[3]), (-u.0, -u.1))),
        ((-u.0, -u.1), -e[0], (pt(e[0], e[3]), (-v.0, -v.1))),
    ];
    let mut lines = Vec::with_capacity(4);
    for (n, d, own) in sides {
        // Hull edges along this side (within 25° of it) and near it.
        let near: Vec<(P, P)> = (0..h.len())
            .map(|i| (h[i], h[(i + 1) % h.len()]))
            .filter(|&(a, b)| {
                let l = (b.0 - a.0).hypot(b.1 - a.1);
                let along = ((b.0 - a.0) * own.1.0 + (b.1 - a.1) * own.1.1).abs() / l.max(1e-9);
                along > 0.9
                    && d - (a.0 * n.0 + a.1 * n.1) <= tol
                    && d - (b.0 * n.0 + b.1 * n.1) <= tol
            })
            .collect();
        let l = fit(&near).unwrap_or(own);
        // A fit turned far from the side (few points, all near one corner) is not trusted.
        let along = (l.1.0 * own.1.0 + l.1.1 * own.1.1).abs();
        lines.push(if along > 0.9 { l } else { own });
    }
    let mut q = [(0.0, 0.0); 4];
    for i in 0..4 {
        q[i] = meet(lines[(i + 3) % 4], lines[i])?;
    }
    // Clockwise on screen (y down): positive shoelace sum.
    let area: f64 = (0..4)
        .map(|i| q[i].0 * q[(i + 1) % 4].1 - q[(i + 1) % 4].0 * q[i].1)
        .sum();
    if area < 0.0 {
        q.reverse();
    }
    Some(q)
}

/// Module (`r`, `c`) of a symbol mapped by `h`: dark by a vote of five taps.
fn sample(b: &Binary, h: &Homography, r: usize, c: usize) -> Option<bool> {
    let mut dark = 0;
    for (dx, dy) in [(0.5, 0.5), (0.3, 0.3), (0.7, 0.3), (0.3, 0.7), (0.7, 0.7)] {
        let (x, y) = h.map(c as f64 + dx, r as f64 + dy);
        dark += usize::from(b.at(x as f32, y as f32)?);
    }
    Some(dark >= 3)
}

/// The share of `size`'s finder and timing modules that match.
fn score(b: &Binary, h: &Homography, s: &Size) -> f64 {
    // The outer ring first: most wrong guesses fail there, cheaply.
    let (mut ok, mut n) = (0usize, 0usize);
    let ring = (0..s.cols)
        .flat_map(|c| [(0, c), (s.rows - 1, c)])
        .chain((1..s.rows - 1).flat_map(|r| [(r, 0), (r, s.cols - 1)]));
    for (r, c) in ring {
        let Some(want) = s.fixed(r, c) else { continue };
        n += 1;
        match sample(b, h, r, c) {
            Some(v) if v == want => ok += 1,
            Some(_) => {}
            None => return 0.0,
        }
    }
    if (ok as f64) < 0.7 * n as f64 {
        return 0.0;
    }
    let (mut ok, mut n) = (0usize, 0usize);
    for r in 0..s.rows {
        for c in 0..s.cols {
            let Some(want) = s.fixed(r, c) else { continue };
            n += 1;
            match sample(b, h, r, c) {
                Some(v) if v == want => ok += 1,
                Some(_) => {}
                None => return 0.0,
            }
        }
    }
    ok as f64 / n.max(1) as f64
}

/// Specks (dark regions of at most four pixels) per 100 000 pixels from which a picture is
/// so noisy that outlines are looked for again in a cleaned copy of all of it.
const SPECKLED: usize = 2500;

/// A 3 × 3 majority vote inside the windows `(x0, y0, x1, y1)` (exclusive ends): specks
/// of noise go, module edges stay. Light elsewhere.
fn majority(b: &Binary, windows: &[[usize; 4]]) -> Binary {
    let (w, h) = (b.w, b.h);
    let mut dark = vec![false; w * h];
    if w < 3 || h < 3 {
        return Binary { w, h, dark };
    }
    let mut col = vec![0u8; w];
    for &[x0, y0, x1, y1] in windows {
        let (x0, x1) = (x0.max(1), x1.min(w - 1));
        let (y0, y1) = (y0.max(1), y1.min(h - 1));
        if x0 >= x1 {
            continue;
        }
        // Each column's count over three rows, then three columns of those.
        for y in y0..y1 {
            for x in x0 - 1..=x1 {
                col[x] = u8::from(b.dark[(y - 1) * w + x])
                    + u8::from(b.dark[y * w + x])
                    + u8::from(b.dark[(y + 1) * w + x]);
            }
            let out = &mut dark[y * w..(y + 1) * w];
            for x in x0..x1 {
                out[x] = col[x - 1] + col[x] + col[x + 1] >= 5;
            }
        }
    }
    Binary { w, h, dark }
}

/// Every symbol in `b` outside the quadrilaterals `skip` (codes already read there).
pub(crate) fn find(b: &Binary, runs: &Runs, grey: &Gray, skip: &[[P; 4]]) -> (Vec<Symbol>, bool) {
    let (out, mut likely, specks) = find_with(b, runs, b, grey, skip);
    if !out.is_empty() {
        return (out, true);
    }
    if specks * 100_000 >= SPECKLED * b.w * b.h {
        likely = vec![[0, 0, b.w, b.h]];
    }
    if likely.is_empty() {
        return (out, false);
    }
    // An L that could not be read, or a very noisy picture (noise breaks outlines):
    // outlines again from a cleaned copy around it, modules still read from the original.
    let m = majority(b, &likely);
    (find_with(&m, &Runs::of(&m), b, grey, skip).0, true)
}

/// The symbols read with outlines from `shape` and modules from `b`; the surroundings
/// (x0, y0, x1, y1) of the regions with a solid L; and the number of specks.
fn find_with(
    shape: &Binary,
    runs: &Runs,
    b: &Binary,
    grey: &Gray,
    skip: &[[P; 4]],
) -> (Vec<Symbol>, Vec<[usize; 4]>, usize) {
    let mut likely = Vec::new();
    let mut out: Vec<Symbol> = Vec::new();
    let mut taken: Vec<[P; 4]> = skip.to_vec();
    let (regs, specks) = regions(shape, runs, skip);
    for reg in regs {
        let h0 = hull_rows(&reg.rows);
        let Some((u, v, e)) = min_rect(&h0) else {
            continue;
        };
        // Skip regions inside a symbol already read.
        let mid = {
            let (a, c) = ((e[0] + e[1]) / 2.0, (e[2] + e[3]) / 2.0);
            (a * u.0 + c * v.0, a * u.1 + c * v.1)
        };
        if taken.iter().any(|q| inside(q, mid)) {
            continue;
        }
        // Cheap first: a region without a solid L along two adjacent sides is no symbol.
        let Some(q0) = corners(&h0) else {
            continue;
        };
        let d0 = sides_dark(shape, &q0);
        if !(0..4).any(|j| d0[(j + 3) % 4].0 >= SOLID && d0[j].0 >= SOLID) {
            continue;
        }
        // Again with every dark pixel near the rectangle (timing modules not touching the
        // region), so the open sides are found.
        likely.push(around(&q0, b.w, b.h));
        let grow = 0.04 * (e[1] - e[0]).max(e[3] - e[2]) + 1.0;
        let h1 = hull_rows(&near_rows(shape, u, v, e, grow));
        let q = corners(&h1).unwrap_or(q0);
        if let Some(sym) = read_quad(b, grey, q) {
            taken.push(sym.corners.map(|(x, y)| (f64::from(x), f64::from(y))));
            out.push(sym);
        }
    }
    (out, likely, specks)
}

/// The box around a quadrilateral, grown by a quarter of its size and clipped.
fn around(q: &[P; 4], w: usize, h: usize) -> [usize; 4] {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in q {
        x0 = x0.min(p.0);
        y0 = y0.min(p.1);
        x1 = x1.max(p.0);
        y1 = y1.max(p.1);
    }
    let g = 0.25 * (x1 - x0).max(y1 - y0) + 4.0;
    [
        (x0 - g).max(0.0) as usize,
        (y0 - g).max(0.0) as usize,
        ((x1 + g).ceil().max(0.0) as usize).min(w),
        ((y1 + g).ceil().max(0.0) as usize).min(h),
    ]
}

fn inside(q: &[P; 4], p: P) -> bool {
    let s: Vec<f64> = (0..4).map(|i| cross(q[i], q[(i + 1) % 4], p)).collect();
    s.iter().all(|&v| v >= 0.0) || s.iter().all(|&v| v <= 0.0)
}

/// Rows of the dark pixels within `grow` px of the rectangle (u, v, e).
fn near_rows(b: &Binary, u: P, v: P, e: [f64; 4], grow: f64) -> Vec<(usize, usize, usize)> {
    let pt = |a: f64, c: f64| (a * u.0 + c * v.0, a * u.1 + c * v.1);
    let cs = [
        pt(e[0] - grow, e[2] - grow),
        pt(e[1] + grow, e[2] - grow),
        pt(e[1] + grow, e[3] + grow),
        pt(e[0] - grow, e[3] + grow),
    ];
    let y0 = cs
        .iter()
        .map(|p| p.1)
        .fold(f64::INFINITY, f64::min)
        .max(0.0) as usize;
    let y1 = (cs
        .iter()
        .map(|p| p.1)
        .fold(f64::NEG_INFINITY, f64::max)
        .ceil() as usize)
        .min(b.h.saturating_sub(1));
    let mut rows = Vec::new();
    for y in y0..=y1 {
        // The pixels of this row whose centres are inside: the first and last dark one.
        let Some((lo, hi)) = span(&cs, y as f64 + 0.5) else {
            continue;
        };
        if hi < 0.5 || lo - 0.5 >= b.w as f64 {
            continue;
        }
        let xa = (lo - 0.5).ceil().max(0.0) as usize;
        let xb = ((hi - 0.5).floor() as usize).min(b.w - 1);
        if xa > xb {
            continue;
        }
        let row = &b.dark[y * b.w + xa..=y * b.w + xb];
        if let Some(first) = row.iter().position(|&d| d) {
            let last = row.iter().rposition(|&d| d).unwrap_or(first);
            rows.push((y, xa + first, xa + last));
        }
    }
    rows
}

/// The share of dark pixels just inside side `a`–`c` of the quadrilateral around `mid`
/// at a few depths, so modules from 1.2 px to large ones are followed: the darkest and the
/// lightest share.
fn side_dark(b: &Binary, a: P, c: P, mid: P) -> P {
    let len = (c.0 - a.0).hypot(c.1 - a.1);
    if len < 1.0 {
        return (0.0, 0.0);
    }
    let d = ((c.0 - a.0) / len, (c.1 - a.1) / len);
    // inward normal: towards the middle
    let mut n = (-d.1, d.0);
    if (mid.0 - a.0) * n.0 + (mid.1 - a.1) * n.1 < 0.0 {
        n = (-n.0, -n.1);
    }
    let steps = ((len / 2.0) as usize).clamp(12, 48);
    let (mut most, mut least) = (0.0f64, 1.0f64);
    for depth in [0.75, 1.5, 3.0] {
        // deeper than a tenth of the side is past the border of any size
        if depth > 0.75 && depth > 0.1 * len {
            break;
        }
        let mut dark = 0usize;
        for i in 0..steps {
            let t = 0.1 + 0.8 * (i as f64 + 0.5) / steps as f64;
            let x = a.0 + d.0 * t * len + n.0 * depth;
            let y = a.1 + d.1 * t * len + n.1 * depth;
            dark += usize::from(b.at(x as f32, y as f32) == Some(true));
        }
        let share = dark as f64 / steps as f64;
        most = most.max(share);
        least = least.min(share);
    }
    (most, least)
}

/// A side whose inner edge is this dark (at some depth) is part of the solid L.
const SOLID: f64 = 0.8;
/// A side of the timing pattern is lighter than this at some depth.
const NOT_SOLID: f64 = 0.92;

/// The dark shares of each side of the clockwise quadrilateral `q` (side i from corner i to
/// corner i + 1).
fn sides_dark(b: &Binary, q: &[P; 4]) -> [P; 4] {
    let mid = (
        q.iter().map(|p| p.0).sum::<f64>() / 4.0,
        q.iter().map(|p| p.1).sum::<f64>() / 4.0,
    );
    [0, 1, 2, 3].map(|i| side_dark(b, q[i], q[(i + 1) % 4], mid))
}

/// Whether corner `j` could be the corner of the L: its two sides solid, the other two not.
fn l_corner(s: &[P; 4], j: usize) -> bool {
    s[(j + 3) % 4].0 >= SOLID
        && s[j].0 >= SOLID
        && s[(j + 1) % 4].1 < NOT_SOLID
        && s[(j + 2) % 4].1 < NOT_SOLID
}

/// The mean grey of module (`r`, `c`): an `n` × `n` lattice over its middle.
fn grey_mean(g: &Gray, h: &Homography, r: usize, c: usize, n: usize) -> Option<f64> {
    let mut sum = 0u32;
    for i in 0..n {
        for j in 0..n {
            let (fx, fy) = (
                0.2 + 0.6 * (j as f64 + 0.5) / n as f64,
                0.2 + 0.6 * (i as f64 + 0.5) / n as f64,
            );
            let (x, y) = h.map(c as f64 + fx, r as f64 + fy);
            if !(x >= 0.0 && y >= 0.0) || x as usize >= g.w || y as usize >= g.h {
                return None;
            }
            sum += u32::from(g.px[y as usize * g.w + x as usize]);
        }
    }
    Some(f64::from(sum) / (n * n) as f64)
}

/// The modules of `s` mapped by `h` from their mean grey, against a threshold halfway between
/// the finder and timing modules' dark and light means (which also tell the polarity).
/// `None` when the symbol leaves the picture or its fixed modules show no contrast.
fn grey_grid(g: &Gray, h: &Homography, s: &Size, n: usize) -> Option<Grid> {
    let mut means = vec![0.0; s.rows * s.cols];
    let (mut dark, mut light) = ((0.0, 0usize), (0.0, 0usize));
    for r in 0..s.rows {
        for c in 0..s.cols {
            let m = grey_mean(g, h, r, c, n)?;
            means[r * s.cols + c] = m;
            match s.fixed(r, c) {
                Some(true) => dark = (dark.0 + m, dark.1 + 1),
                Some(false) => light = (light.0 + m, light.1 + 1),
                None => {}
            }
        }
    }
    let (d, l) = (
        dark.0 / dark.1.max(1) as f64,
        light.0 / light.1.max(1) as f64,
    );
    if (d - l).abs() < 24.0 {
        return None;
    }
    let mid = (d + l) / 2.0;
    Some(Grid {
        rows: s.rows,
        cols: s.cols,
        dark: means.iter().map(|&m| (m < mid) == (d < l)).collect(),
    })
}

/// How far apart the mean greys of `s`'s dark and light finder and timing modules are.
fn fixed_contrast(g: &Gray, h: &Homography, s: &Size, n: usize) -> f64 {
    let (mut dark, mut light) = ((0.0, 0usize), (0.0, 0usize));
    for r in 0..s.rows {
        for c in 0..s.cols {
            let Some(want) = s.fixed(r, c) else { continue };
            let Some(m) = grey_mean(g, h, r, c, n) else {
                return 0.0;
            };
            if want {
                dark = (dark.0 + m, dark.1 + 1);
            } else {
                light = (light.0 + m, light.1 + 1);
            }
        }
    }
    (light.0 / light.1.max(1) as f64 - dark.0 / dark.1.max(1) as f64).abs()
}

/// `quad` (top left, top right, bottom right, bottom left of `s`) with its corners moved, a
/// quarter and an eighth of a module at a time, to where the finder and timing modules
/// stand out most: outlines from noisy pixels are off by up to a module, too much for
/// small modules.
fn refine(g: &Gray, quad: [P; 4], s: &Size, module: f64, n: usize) -> [P; 4] {
    let (c, r) = (s.cols as f64, s.rows as f64);
    let h = |q: [P; 4]| Homography::quad_to_quad([(0.0, 0.0), (c, 0.0), (c, r), (0.0, r)], q);
    let mut q = quad;
    let mut best = fixed_contrast(g, &h(q), s, n);
    for step in [0.25 * module, 0.125 * module] {
        for _ in 0..3 {
            let mut moved = false;
            for k in 0..4 {
                for (dx, dy) in [(step, 0.0), (-step, 0.0), (0.0, step), (0.0, -step)] {
                    let mut t = q;
                    t[k] = (q[k].0 + dx, q[k].1 + dy);
                    let v = fixed_contrast(g, &h(t), s, n);
                    if v > best {
                        (best, q, moved) = (v, t, true);
                    }
                }
            }
            if !moved {
                break;
            }
        }
    }
    q
}

/// Tries every turn, mirroring and size on the quadrilateral `q` (clockwise).
fn read_quad(b: &Binary, grey: &Gray, q: [P; 4]) -> Option<Symbol> {
    let side = |a: P, c: P| (a.0 - c.0).hypot(a.1 - c.1);
    let dark = sides_dark(b, &q);
    let mut cands: Vec<(f64, [P; 4], bool, Size)> = Vec::new();
    for k in 0..4 {
        for mirrored in [false, true] {
            // The L's corner (bottom left as printed) is corner k + 3 or, mirrored, k + 1.
            if !l_corner(&dark, if mirrored { (k + 1) % 4 } else { (k + 3) % 4 }) {
                continue;
            }
            let at = |i: usize| {
                if mirrored {
                    q[(k + 4 - i) % 4]
                } else {
                    q[(k + i) % 4]
                }
            };
            let (tl, tr, br, bl) = (at(0), at(1), at(2), at(3));
            let width = (side(tl, tr) + side(bl, br)) / 2.0;
            let height = (side(tl, bl) + side(tr, br)) / 2.0;
            for s in SIZES {
                let want = s.cols as f64 / s.rows as f64;
                let have = width / height.max(1e-9);
                if !(have / want).is_finite() || (have / want).max(want / have) > 1.6 {
                    continue;
                }
                if width / (s.cols as f64) < 1.2 || height / (s.rows as f64) < 1.2 {
                    continue;
                }
                let (c, r) = (s.cols as f64, s.rows as f64);
                let h = Homography::quad_to_quad(
                    [(0.0, 0.0), (c, 0.0), (c, r), (0.0, r)],
                    [tl, tr, br, bl],
                );
                let sc = score(b, &h, &s);
                if sc >= MIN_SCORE {
                    cands.push((sc, [tl, tr, br, bl], mirrored, s));
                }
            }
        }
    }
    cands.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    cands.truncate(8);
    // Five taps of the binary picture a module first; then (noise) each module's mean grey.
    for from_grey in [false, true] {
        for &(_, quad, mirrored, s) in &cands {
            let (c, r) = (s.cols as f64, s.rows as f64);
            let module = (side(quad[0], quad[1]) / c).min(side(quad[0], quad[3]) / r);
            let n = (module * 0.6).ceil().clamp(2.0, 5.0) as usize;
            let quad = if from_grey {
                refine(grey, quad, &s, module, n)
            } else {
                quad
            };
            let h = Homography::quad_to_quad([(0.0, 0.0), (c, 0.0), (c, r), (0.0, r)], quad);
            let g = if from_grey {
                let Some(g) = grey_grid(grey, &h, &s, n) else {
                    continue;
                };
                g
            } else {
                let mut g = Grid {
                    rows: s.rows,
                    cols: s.cols,
                    dark: vec![false; s.rows * s.cols],
                };
                for rr in 0..s.rows {
                    for cc in 0..s.cols {
                        g.dark[rr * s.cols + cc] = sample(b, &h, rr, cc).unwrap_or(false);
                    }
                }
                g
            };
            if let Ok((content, corrected)) = read(&g, &s) {
                return Some(Symbol {
                    size: s,
                    content,
                    corners: quad.map(|(x, y)| (x as f32, y as f32)),
                    mirrored,
                    corrected,
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hull_of_rows_is_the_hull_of_their_pixels() {
        let mut s = 7u32;
        let mut rnd = |n: usize| {
            s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
            (s >> 16) as usize % n
        };
        for _ in 0..200 {
            let y0 = rnd(20);
            let rows: Vec<(usize, usize, usize)> = (0..1 + rnd(30))
                .map(|i| {
                    let a = rnd(40);
                    (y0 + i, a, a + rnd(40))
                })
                .collect();
            // Every pixel corner, sorted by (x, y) the usual way.
            let mut pts: Vec<P> = rows
                .iter()
                .flat_map(|&(y, a, b)| {
                    let (y, a, b) = (y as f64, a as f64, b as f64 + 1.0);
                    [(a, y), (a, y + 1.0), (b, y), (b, y + 1.0)]
                })
                .collect();
            pts.sort_by(|a, b| a.partial_cmp(b).unwrap());
            pts.dedup();
            let mut want = chain(&pts);
            let mut got = hull_rows(&rows);
            want.sort_by(|a, b| a.partial_cmp(b).unwrap());
            got.sort_by(|a, b| a.partial_cmp(b).unwrap());
            assert_eq!(got, want, "{rows:?}");
        }
    }
}
