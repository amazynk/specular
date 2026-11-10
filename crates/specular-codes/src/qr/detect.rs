//! Finding QR symbols in a binarised image: finder patterns (dark–light–dark–light–dark
//! runs in the ratio 1:1:3:1:1 across and down), triples of them forming a right angle, the
//! bottom-right alignment pattern, and the perspective grid sampled module by module.

use super::matrix::Grid;
use crate::image::{Binary, Gray, Homography, Runs};

/// A finder pattern's centre and module size, and how many scan lines confirmed it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Finder {
    pub x: f32,
    pub y: f32,
    pub module: f32,
    pub count: u32,
}

#[inline(always)]
fn ratio_ok(c: &[u32; 5]) -> bool {
    let total: u32 = c.iter().sum();
    if total < 7 || c.contains(&0) {
        return false;
    }
    // In units of a seventh: each side run within two thirds of a module of one module, the
    // centre within two modules of three.
    let t2 = 2 * total;
    3 * (7 * c[0]).abs_diff(total) < t2
        && 3 * (7 * c[4]).abs_diff(total) < t2
        && (7 * c[2]).abs_diff(3 * total) < t2
        && 3 * (7 * c[1]).abs_diff(total) < t2
        && 3 * (7 * c[3]).abs_diff(total) < t2
}

/// Runs through (x, y) along direction (dx, dy): the five runs centred on the dark centre,
/// each limited to `max` pixels. Returns the runs and the centre's offset along the line.
fn cross(b: &Binary, x: f32, y: f32, dx: f32, dy: f32, max: u32) -> Option<([u32; 5], f32)> {
    let at = |k: i32| b.at(x + dx * k as f32, y + dy * k as f32);
    if at(0) != Some(true) {
        return None;
    }
    let mut c = [0u32; 5];
    // backwards: centre, light, dark
    let mut k = 0i32;
    while at(k) == Some(true) {
        c[2] += 1;
        k -= 1;
        if c[2] > max * 3 {
            return None;
        }
    }
    while at(k) == Some(false) {
        c[1] += 1;
        k -= 1;
        if c[1] > max {
            return None;
        }
    }
    while at(k) == Some(true) {
        c[0] += 1;
        k -= 1;
        if c[0] > max {
            return None;
        }
    }
    let start = k + 1;
    k = 1;
    while at(k) == Some(true) {
        c[2] += 1;
        k += 1;
        if c[2] > max * 3 {
            return None;
        }
    }
    while at(k) == Some(false) {
        c[3] += 1;
        k += 1;
        if c[3] > max {
            return None;
        }
    }
    while at(k) == Some(true) {
        c[4] += 1;
        k += 1;
        if c[4] > max {
            return None;
        }
    }
    if !ratio_ok(&c) {
        return None;
    }
    // centre of the middle run, relative to (x, y)
    let mid_start = start + (c[0] + c[1]) as i32;
    let centre = mid_start as f32 + c[2] as f32 / 2.0;
    Some((c, centre))
}

/// Confirms a candidate from a horizontal scan: across it vertically, again horizontally
/// at the new centre, and diagonally.
fn confirm(b: &Binary, runs: &[u32; 5], x_end: usize, y: usize) -> Option<Finder> {
    let total: u32 = runs.iter().sum();
    let cx = x_end as f32 - runs[4] as f32 - runs[3] as f32 - runs[2] as f32 / 2.0;
    let max = runs[2];
    let (vc, vy) = cross(b, cx, y as f32 + 0.5, 0.0, 1.0, max)?;
    let vt: u32 = vc.iter().sum();
    if 5 * vt.abs_diff(total) >= 2 * total {
        return None;
    }
    let cy = y as f32 + vy;
    let (hc, hx) = cross(b, cx, cy, 1.0, 0.0, max)?;
    let cx = cx.floor() + hx;
    let ht: u32 = hc.iter().sum();
    let module = (ht + vt) as f32 / 14.0;
    Some(Finder {
        x: cx,
        y: cy,
        module,
        count: 1,
    })
}

fn add(found: &mut Vec<Finder>, f: Finder) {
    for g in found.iter_mut() {
        if (g.x - f.x).abs() <= g.module.max(f.module)
            && (g.y - f.y).abs() <= g.module.max(f.module)
            && (g.module - f.module).abs() <= g.module.max(1.0)
        {
            let n = g.count as f32;
            g.x = (g.x * n + f.x) / (n + 1.0);
            g.y = (g.y * n + f.y) / (n + 1.0);
            g.module = (g.module * n + f.module) / (n + 1.0);
            g.count += 1;
            return;
        }
    }
    found.push(f);
}

/// All finder pattern candidates in the image: every three dark runs in a row with the two
/// light runs between them in the ratio 1:1:3:1:1.
pub(crate) fn finders(b: &Binary, runs: &Runs) -> Vec<Finder> {
    let mut found = Vec::new();
    for y in 0..b.h {
        for w in runs.row(y).windows(3) {
            let [(a0, a1), (b0, b1), (c0, c1)] = [w[0], w[1], w[2]];
            let c = [
                a1 - a0 + 1,
                b0 - a1 - 1,
                b1 - b0 + 1,
                c0 - b1 - 1,
                c1 - c0 + 1,
            ];
            if ratio_ok(&c)
                && let Some(f) = confirm(b, &c, c1 as usize + 1, y)
            {
                add(&mut found, f);
            }
        }
    }
    found
}

/// The best dark square of about one module ringed by light near (x, y): the alignment
/// pattern's centre.
fn alignment(b: &Binary, x: f32, y: f32, module: f32, radius: f32) -> Option<(f32, f32)> {
    let r = radius.max(module * 4.0);
    let (x0, x1) = (
        (x - r).max(0.0) as usize,
        ((x + r) as usize).min(b.w.saturating_sub(1)),
    );
    let (y0, y1) = (
        (y - r).max(0.0) as usize,
        ((y + r) as usize).min(b.h.saturating_sub(1)),
    );
    let mut best: Option<(f32, f32, f32)> = None;
    let max = (module * 2.5).ceil() as u32 + 1;
    let fits = |n: u32| (n as f32 - module).abs() < module * 0.7;
    for yy in y0..=y1 {
        let mut xx = x0;
        while xx <= x1 {
            if !b.get(xx, yy) {
                xx += 1;
                continue;
            }
            // a dark run from xx
            let s = xx;
            while xx <= x1 && b.get(xx, yy) {
                xx += 1;
            }
            let len = (xx - s) as u32;
            if !fits(len) {
                continue;
            }
            let cx = s as f32 + len as f32 / 2.0;
            // light on both sides about a module wide, then dark
            let side = |dir: i32| -> bool {
                let mut k = 0u32;
                let mut p = if dir < 0 { s as i32 - 1 } else { xx as i32 };
                while p >= 0 && (p as usize) < b.w && !b.get(p as usize, yy) && k <= max {
                    k += 1;
                    p += dir;
                }
                fits(k)
            };
            if !side(-1) || !side(1) {
                continue;
            }
            // vertically: dark centre, light ring above and below
            let col = cx as usize;
            let mut up = yy as i32;
            while up >= 0 && b.get(col, up as usize) {
                up -= 1;
            }
            let mut down = yy;
            while down < b.h && b.get(col, down) {
                down += 1;
            }
            let vlen = (down as i32 - up - 1) as u32;
            if !fits(vlen) {
                continue;
            }
            let cy = (up + 1) as f32 + vlen as f32 / 2.0;
            let d = (cx - x).hypot(cy - y);
            if best.is_none_or(|b| d < b.2) {
                best = Some((cx, cy, d));
            }
        }
    }
    best.map(|(a, b, _)| (a, b))
}

/// Where the timing patterns put the columns and rows ([`timing_warp`]'s edges across and
/// down), and how the finders' columns and rows outside them follow (`bent`: see [`warp`]).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Warps {
    pub across: Vec<f64>,
    pub down: Vec<f64>,
    pub bent: bool,
}

/// A located symbol: its grid and the transform from module coordinates to the image.
pub(crate) struct Located {
    pub grid: Grid,
    pub to_image: Homography,
}

impl Located {
    /// The grid again, each module read from its centre pixel alone.
    pub(crate) fn centres(&self, b: &Binary) -> Grid {
        let n = self.grid.size;
        let mut g = Grid::new(n);
        for y in 0..n {
            for x in 0..n {
                let (ix, iy) = self.to_image.map(x as f64 + 0.5, y as f64 + 0.5);
                g.set(x, y, b.at(ix as f32, iy as f32).unwrap_or(false));
            }
        }
        g
    }
}

impl Located {
    /// The grid again with each column and row moved to where the timing patterns put it:
    /// a label bent round a bottle or a sheet that sags stretches the modules unevenly, the
    /// same along every row (or column), which one homography can't follow. `None` when
    /// either timing pattern doesn't show every module.
    pub(crate) fn timed(&self, b: &Binary, w: &Warps) -> Option<Grid> {
        let (fx, fy, bent) = (&w.across, &w.down, w.bent);
        let n = self.grid.size;
        let h = self.to_image;
        let mut grid = Grid::new(n);
        for y in 0..n {
            for x in 0..n {
                let mut dark = 0;
                for (ox, oy) in [(0.5, 0.5), (0.3, 0.3), (0.7, 0.3), (0.3, 0.7), (0.7, 0.7)] {
                    let (ix, iy) =
                        h.map(warp(fx, x as f64 + ox, bent), warp(fy, y as f64 + oy, bent));
                    dark += u32::from(b.at(ix as f32, iy as f32)?);
                }
                grid.set(x, y, dark >= 3);
            }
        }
        Some(grid)
    }

    /// The timing patterns' edges across and down ([`timing_warp`]) to try: each read
    /// along the row or column the homography puts it on; then each read where the other
    /// puts it (a strong bend moves the column off the homography's).
    pub(crate) fn warps(&self, b: &Binary) -> Vec<Warps> {
        let (n, h) = (self.grid.size, self.to_image);
        let fx = timing_warp(b, h, n, true, None);
        let fy = timing_warp(b, h, n, false, None);
        let mut out = Vec::new();
        let mut add = |(across, down): (Vec<f64>, Vec<f64>)| {
            for bent in [false, true] {
                let w = Warps {
                    across: across.clone(),
                    down: down.clone(),
                    bent,
                };
                if !out.contains(&w) {
                    out.push(w);
                }
            }
        };
        if let (Some(x), Some(y)) = (&fx, &fy) {
            add((x.clone(), y.clone()));
        }
        if let Some(x) = &fx
            && let Some(y) = timing_warp(b, h, n, false, Some(x))
        {
            add((x.clone(), y));
        }
        if let Some(y) = &fy
            && let Some(x) = timing_warp(b, h, n, true, Some(y))
        {
            add((x, y.clone()));
        }
        out
    }

    /// The grid read from the grey picture `g` (the one binarised; light on dark when
    /// `inverted`): each module's centre value, interpolated, dark on the side of the
    /// threshold that best splits the symbol's values (Otsu). Small blurred modules keep
    /// their centre's shade where a binarisation loses them. Through the timing patterns'
    /// `warps` when given.
    pub(crate) fn grey_grid(
        &self,
        g: &Gray,
        inverted: bool,
        warps: Option<&Warps>,
    ) -> Option<Grid> {
        let n = self.grid.size;
        let mut v = Vec::with_capacity(n * n);
        for y in 0..n {
            for x in 0..n {
                let (u, w) = (x as f64 + 0.5, y as f64 + 0.5);
                let (u, w) = warps.map_or((u, w), |p| {
                    (warp(&p.across, u, p.bent), warp(&p.down, w, p.bent))
                });
                let (ix, iy) = self.to_image.map(u, w);
                v.push(bilinear(g, ix - 0.5, iy - 0.5)?);
            }
        }
        let thr = otsu(&v);
        let mut grid = Grid::new(n);
        for y in 0..n {
            for x in 0..n {
                grid.set(x, y, (v[y * n + x] < thr) != inverted);
            }
        }
        Some(grid)
    }
}

/// `g` at (x, y) in pixel centres, interpolated; `None` outside.
fn bilinear(g: &Gray, x: f64, y: f64) -> Option<f64> {
    if x < 0.0 || y < 0.0 {
        return None;
    }
    let (x0, y0) = (x as usize, y as usize);
    if x0 + 1 >= g.w || y0 + 1 >= g.h {
        return None;
    }
    let (fx, fy) = (x - x0 as f64, y - y0 as f64);
    let p = |x: usize, y: usize| f64::from(g.px[y * g.w + x]);
    let top = p(x0, y0) * (1.0 - fx) + p(x0 + 1, y0) * fx;
    let bottom = p(x0, y0 + 1) * (1.0 - fx) + p(x0 + 1, y0 + 1) * fx;
    Some(top * (1.0 - fy) + bottom * fy)
}

/// The threshold that best splits `v` in two (Otsu: largest between-class variance).
fn otsu(v: &[f64]) -> f64 {
    let mut s: Vec<f64> = v.to_vec();
    s.sort_by(f64::total_cmp);
    let total: f64 = s.iter().sum();
    let (mut best, mut thr, mut below) = (-1.0, s[s.len() / 2], 0.0);
    for i in 1..s.len() {
        below += s[i - 1];
        let (n0, n1) = (i as f64, (s.len() - i) as f64);
        let (m0, m1) = (below / n0, (total - below) / n1);
        let between = n0 * n1 * (m0 - m1) * (m0 - m1);
        if between > best && s[i] > s[i - 1] {
            best = between;
            thr = (s[i - 1] + s[i]) / 2.0;
        }
    }
    thr
}

/// What [`Located::timed`] finds (study aid): edges across and down, and whether the
/// grid decodes.
#[cfg(test)]
pub(crate) fn debug_timed(b: &Binary, l: &Located) -> (Option<usize>, Option<usize>, bool) {
    let n = l.grid.size;
    let fx = timing_warp(b, l.to_image, n, true, None).map(|e| e.len());
    let fy = timing_warp(b, l.to_image, n, false, None).map(|e| e.len());
    let ok =
        l.warps(b).iter().filter_map(|w| l.timed(b, w)).any(|g| {
            super::matrix::read(&g).is_ok() || super::matrix::read(&g.transposed()).is_ok()
        });
    (fx, fy, ok)
}

/// Where the edges between a timing pattern's modules are, in the module coordinates of
/// `h`: index `k` is the edge the symbol has at `7 + k` (from the separator to the last
/// finder). Read along the middle of row 6 (`along_x`) or column 6, a little above and
/// below too, each sample the majority of the three.
fn timing_warp(
    b: &Binary,
    h: Homography,
    dim: usize,
    along_x: bool,
    cross: Option<&[f64]>,
) -> Option<Vec<f64>> {
    const STEP: f64 = 0.1;
    // A change of colour shorter than this is a speck (modules).
    const MIN_RUN: f64 = 0.25;
    // How far an edge may be from where one homography puts it (modules).
    const SLACK: f64 = 1.5;
    let want = dim - 13;
    let at = |t: f64| -> Option<bool> {
        let mut dark = 0;
        for off in [6.3, 6.5, 6.7] {
            let off = cross.map_or(off, |e| warp(e, off, true));
            let (u, v) = if along_x { (t, off) } else { (off, t) };
            let (ix, iy) = h.map(u, v);
            dark += u32::from(b.at(ix as f32, iy as f32)?);
        }
        Some(dark >= 2)
    };
    // Dark finder ring, then light separator, alternating, light separator, dark finder.
    let mut cur = true;
    if at(6.5)? != cur {
        return None;
    }
    let mut edges = Vec::with_capacity(want);
    // A change of colour seen at `t0`, not yet long enough to count.
    let mut change: Option<f64> = None;
    let mut t = 6.5 + STEP;
    let end = dim as f64 - 6.5;
    while t <= end {
        let v = at(t)?;
        if v == cur {
            change = None;
        } else {
            let t0 = *change.get_or_insert(t);
            if t - t0 + STEP >= MIN_RUN {
                let e = t0 - STEP / 2.0;
                if edges.len() >= want || (e - (7 + edges.len()) as f64).abs() > SLACK {
                    return None;
                }
                edges.push(e);
                cur = v;
                change = None;
            }
        }
        // The next edge is overdue: not a timing pattern (or bent past reading).
        if edges.len() < want && t > (7 + edges.len()) as f64 + SLACK + MIN_RUN {
            return None;
        }
        t += STEP;
    }
    (edges.len() == want).then_some(edges)
}

/// Module coordinate `t` moved as the measured `edges` (from [`timing_warp`]) say:
/// linear between them. Outside them (the finders' columns), shifted with the nearest edge;
/// or (`bent`: a bend stretches those most) on the parabola through the finder's centre
/// (where the homography puts it, 3.5 modules in) and the two nearest edges two modules
/// apart.
fn warp(edges: &[f64], t: f64, bent: bool) -> f64 {
    let first = 7.0;
    let n = edges.len();
    let last = first + (n - 1) as f64;
    // Lagrange through (x0, y0), (x1, y1), (x2, y2) at t.
    let para = |p: [(f64, f64); 3]| {
        let [(x0, y0), (x1, y1), (x2, y2)] = p;
        y0 * (t - x1) * (t - x2) / ((x0 - x1) * (x0 - x2))
            + y1 * (t - x0) * (t - x2) / ((x1 - x0) * (x1 - x2))
            + y2 * (t - x0) * (t - x1) / ((x2 - x0) * (x2 - x1))
    };
    if t <= first {
        if n < 3 || !bent {
            return t + edges[0] - first;
        }
        return para([(3.5, 3.5), (first, edges[0]), (first + 2.0, edges[2])]);
    }
    if t >= last {
        if n < 3 || !bent {
            return t + edges[n - 1] - last;
        }
        return para([
            (last + 3.5, last + 3.5),
            (last, edges[n - 1]),
            (last - 2.0, edges[n - 3]),
        ]);
    }
    let k = (t - first).floor() as usize;
    let f = t - first - k as f64;
    edges[k] + f * (edges[k + 1] - edges[k])
}

/// Samples the grid of a `dim`-module symbol whose finders are `tl`, `tr`, `bl`.
pub(crate) fn locate(
    b: &Binary,
    tl: &Finder,
    tr: &Finder,
    bl: &Finder,
    dim: usize,
    use_alignment: bool,
) -> Option<Located> {
    locate_nudged(b, tl, tr, bl, dim, use_alignment, (0.0, 0.0))
}

/// As [`locate`], with the bottom-right corner estimate (no alignment pattern) moved by
/// `nudge` modules along the symbol's axes: perspective bends it off the parallelogram.
pub(crate) fn locate_nudged(
    b: &Binary,
    tl: &Finder,
    tr: &Finder,
    bl: &Finder,
    dim: usize,
    use_alignment: bool,
    nudge: (f64, f64),
) -> Option<Located> {
    place_nudged(b, tl, tr, bl, dim, use_alignment, nudge, false)
}

#[allow(clippy::too_many_arguments)]
fn place_nudged(
    b: &Binary,
    tl: &Finder,
    tr: &Finder,
    bl: &Finder,
    dim: usize,
    use_alignment: bool,
    nudge: (f64, f64),
    ungated: bool,
) -> Option<Located> {
    let d = dim as f64;
    let module = f64::from(tl.module + tr.module + bl.module) / 3.0;
    let p = |f: &Finder| (f64::from(f.x), f64::from(f.y));
    let (ptl, ptr, pbl) = (p(tl), p(tr), p(bl));
    let mut br = (ptr.0 + pbl.0 - ptl.0, ptr.1 + pbl.1 - ptl.1);
    let mut br_mod = (d - 3.5, d - 3.5);
    let (ux, uy) = ((ptr.0 - ptl.0) / (d - 7.0), (ptr.1 - ptl.1) / (d - 7.0));
    let (vx, vy) = ((pbl.0 - ptl.0) / (d - 7.0), (pbl.1 - ptl.1) / (d - 7.0));
    br.0 += nudge.0 * ux + nudge.1 * vx;
    br.1 += nudge.0 * uy + nudge.1 * vy;
    if use_alignment && dim > 21 {
        // the bottom-right alignment pattern, 3 modules in from the corner finders' lines
        let k = 1.0 - 3.0 / (d - 7.0);
        let gx = ptl.0 + k * (br.0 - ptl.0);
        let gy = ptl.1 + k * (br.1 - ptl.1);
        let found = alignment(
            b,
            gx as f32,
            gy as f32,
            module as f32,
            (module * 6.0) as f32,
        )?;
        br = (f64::from(found.0), f64::from(found.1));
        br_mod = (d - 6.5, d - 6.5);
    }
    let from = [(3.5, 3.5), (d - 3.5, 3.5), br_mod, (3.5, d - 3.5)];
    let to = [ptl, ptr, br, pbl];
    let h = Homography::quad_to_quad(from, to);
    if ungated {
        return Some(Located {
            grid: Grid::new(dim),
            to_image: h,
        });
    }
    sample(b, h, dim)
}

/// Where [`locate`] puts a `dim`-module symbol, without reading it or checking its timing
/// patterns (a bent symbol fails that check; [`Located::timed`] reads it).
pub(crate) fn place(
    b: &Binary,
    tl: &Finder,
    tr: &Finder,
    bl: &Finder,
    dim: usize,
    use_alignment: bool,
) -> Option<Located> {
    place_nudged(b, tl, tr, bl, dim, use_alignment, (0.0, 0.0), true)
}

/// The grid of a `dim`-module symbol through `h` (module coordinates to the image), when
/// its timing patterns are there.
fn sample(b: &Binary, h: Homography, dim: usize) -> Option<Located> {
    // The timing patterns first: a false triple of finders seldom has them.
    let tap = |x: f64, y: f64| {
        let (ix, iy) = h.map(x + 0.5, y + 0.5);
        b.at(ix as f32, iy as f32)
    };
    let (mut good, mut all) = (0, 0);
    for i in 8..dim - 8 {
        let want = i % 2 == 0;
        for v in [tap(i as f64, 6.0), tap(6.0, i as f64)] {
            all += 1;
            good += usize::from(v == Some(want));
        }
    }
    if good * 10 < all * 7 {
        return None;
    }
    let mut grid = Grid::new(dim);
    for y in 0..dim {
        for x in 0..dim {
            // five taps: the centre and four a quarter module away; most decide
            let mut dark = 0;
            for (ox, oy) in [(0.5, 0.5), (0.3, 0.3), (0.7, 0.3), (0.3, 0.7), (0.7, 0.7)] {
                let (ix, iy) = h.map(x as f64 + ox, y as f64 + oy);
                dark += u32::from(b.at(ix as f32, iy as f32)?);
            }
            grid.set(x, y, dark >= 3);
        }
    }
    Some(Located { grid, to_image: h })
}

/// A straight line: a point on it and its unit direction.
#[derive(Clone, Copy, Debug)]
struct Line {
    p: (f64, f64),
    d: (f64, f64),
}

/// The total-least-squares line through `pts` (three or more).
fn fit_line(pts: &[(f64, f64)]) -> Option<Line> {
    if pts.len() < 3 {
        return None;
    }
    let n = pts.len() as f64;
    let (cx, cy) = pts
        .iter()
        .fold((0.0, 0.0), |a, p| (a.0 + p.0 / n, a.1 + p.1 / n));
    let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
    for &(x, y) in pts {
        let (dx, dy) = (x - cx, y - cy);
        sxx += dx * dx;
        sxy += dx * dy;
        syy += dy * dy;
    }
    // the principal axis of the scatter
    let angle = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    Some(Line {
        p: (cx, cy),
        d: (angle.cos(), angle.sin()),
    })
}

/// Where two lines cross, unless they are nearly parallel.
fn meet(a: Line, b: Line) -> Option<(f64, f64)> {
    let den = a.d.0 * b.d.1 - a.d.1 * b.d.0;
    if den.abs() < 0.2 {
        return None;
    }
    let (wx, wy) = (b.p.0 - a.p.0, b.p.1 - a.p.1);
    let t = (wx * b.d.1 - wy * b.d.0) / den;
    Some((a.p.0 + t * a.d.0, a.p.1 + t * a.d.1))
}

/// Points on the outer edge of finder `f`'s dark ring on the side facing `n` (a unit
/// vector in the image): rays from the centre fanned ±30° about `n`, each ending where it
/// leaves the ring (dark centre, light ring, dark ring, light).
fn edge_points(b: &Binary, f: &Finder, n: (f64, f64), module: f64, out: &mut Vec<(f64, f64)>) {
    let (x0, y0) = (f64::from(f.x), f64::from(f.y));
    for k in -3i32..=3 {
        let a = f64::from(k) * 0.17;
        let (c, s) = (a.cos(), a.sin());
        let d = (n.0 * c - n.1 * s, n.0 * s + n.1 * c);
        let reach = 5.0 * module / c;
        let (mut prev, mut t, mut leaves) = (true, 0.0, 0);
        while t < reach {
            t += 0.5;
            let Some(dark) = b.at((x0 + d.0 * t) as f32, (y0 + d.1 * t) as f32) else {
                break;
            };
            if prev && !dark {
                leaves += 1;
                if leaves == 2 {
                    // the edge is between this sample and the one before, along `n`
                    let along = (t - 0.25) * c;
                    if (2.5 * module..=4.5 * module).contains(&along) {
                        out.push((x0 + d.0 * (t - 0.25), y0 + d.1 * (t - 0.25)));
                    }
                    break;
                }
            }
            prev = dark;
        }
    }
}

/// A symbol outlined by its outer edges, for pictures in strong perspective: the top edge
/// along the outside of the top-left and top-right finders, the left one along the
/// top-left and bottom-left, the right and bottom edges along the top-right's and
/// bottom-left's outsides; their crossings are the symbol's corners.
pub(crate) struct Outline {
    dim: usize,
    /// Top-left, top-right, bottom-right, bottom-left.
    corners: [(f64, f64); 4],
    /// The finders' centres, top-left, top-right, bottom-left.
    centres: [(f64, f64); 3],
    /// The bottom-right alignment pattern's centre, where the corners put one.
    alignment: Option<(f64, f64)>,
    module: f64,
}

/// The outline of a `dim`-module symbol whose finders are `tl`, `tr`, `bl`.
pub(crate) fn outline(
    b: &Binary,
    tl: &Finder,
    tr: &Finder,
    bl: &Finder,
    dim: usize,
) -> Option<Outline> {
    let d = dim as f64;
    let module = f64::from(tl.module + tr.module + bl.module) / 3.0;
    let unit = |a: &Finder, z: &Finder| {
        let (x, y) = (f64::from(z.x - a.x), f64::from(z.y - a.y));
        let l = x.hypot(y).max(1e-9);
        (x / l, y / l)
    };
    let (u, v) = (unit(tl, tr), unit(tl, bl));
    let neg = |p: (f64, f64)| (-p.0, -p.1);
    let mut pts = Vec::with_capacity(14);
    let mut side = |parts: &[(&Finder, (f64, f64))]| {
        pts.clear();
        for (f, n) in parts {
            edge_points(b, f, *n, module, &mut pts);
        }
        fit_line(&pts)
    };
    let top = side(&[(tl, neg(v)), (tr, neg(v))])?;
    let left = side(&[(tl, neg(u)), (bl, neg(u))])?;
    let right = side(&[(tr, u)])?;
    let bottom = side(&[(bl, v)])?;
    let corners = [
        meet(top, left)?,
        meet(top, right)?,
        meet(right, bottom)?,
        meet(left, bottom)?,
    ];
    let alignment = (dim > 21)
        .then(|| {
            let h = Homography::quad_to_quad(square(d), corners);
            let (gx, gy) = h.map(d - 6.5, d - 6.5);
            alignment(
                b,
                gx as f32,
                gy as f32,
                module as f32,
                (module * 2.0) as f32,
            )
        })
        .flatten()
        .map(|(x, y)| (f64::from(x), f64::from(y)));
    let c = |f: &Finder| (f64::from(f.x), f64::from(f.y));
    Some(Outline {
        dim,
        corners,
        centres: [c(tl), c(tr), c(bl)],
        alignment,
        module,
    })
}

/// The corners of a `d`-module square.
fn square(d: f64) -> [(f64, f64); 4] {
    [(0.0, 0.0), (d, 0.0), (d, d), (0.0, d)]
}

impl Outline {
    /// The grid through the corners.
    pub(crate) fn cornered(&self, b: &Binary) -> Option<Located> {
        sample(
            b,
            Homography::quad_to_quad(square(self.dim as f64), self.corners),
            self.dim,
        )
    }

    /// The grid through the finders' centres and the alignment patterns: the bottom-right
    /// one found where the corners put it, then (larger symbols) the others where that
    /// grid puts them, all fitted together.
    pub(crate) fn aligned(&self, b: &Binary) -> Option<Located> {
        let d = self.dim as f64;
        let at = [(3.5, 3.5), (d - 3.5, 3.5), (3.5, d - 3.5)];
        let mut pairs: Vec<crate::image::PointPair> = at.into_iter().zip(self.centres).collect();
        pairs.push(((d - 6.5, d - 6.5), self.alignment?));
        let mut h = Homography::fit(&pairs)?;
        let pos = super::tables::alignment((self.dim - 17) / 4);
        if pos.len() > 2 {
            let last = pos[pos.len() - 1];
            for &y in &pos {
                for &x in &pos {
                    // none under the finders; the bottom-right one is in already
                    if (x == 6 && (y == 6 || y == last))
                        || (y == 6 && x == last)
                        || x == last && y == last
                    {
                        continue;
                    }
                    let m = (x as f64 + 0.5, y as f64 + 0.5);
                    let (gx, gy) = h.map(m.0, m.1);
                    let r = (self.module * 1.5) as f32;
                    if let Some((ax, ay)) =
                        alignment(b, gx as f32, gy as f32, self.module as f32, r)
                    {
                        pairs.push((m, (f64::from(ax), f64::from(ay))));
                    }
                }
            }
            h = Homography::fit(&pairs)?;
        }
        sample(b, h, self.dim)
    }
}

/// A finder's width along the line to another finder's centre, over 7.
fn module_along(img: &Binary, f: &Finder, to: &Finder) -> Option<f32> {
    let (dx, dy) = (to.x - f.x, to.y - f.y);
    let len = dx.hypot(dy);
    if len < 1.0 {
        return None;
    }
    let (ux, uy) = (dx / len, dy / len);
    let max = (f.module * 3.0).ceil() as u32 + 2;
    let (runs, _) = cross(img, f.x, f.y, ux, uy, max)?;
    Some(runs.iter().sum::<u32>() as f32 / 7.0)
}

/// The three finders arranged as top-left, top-right, bottom-left, when they make a
/// plausible symbol, with its estimated size in modules.
pub(crate) fn arrange(
    img: &Binary,
    a: &Finder,
    b: &Finder,
    c: &Finder,
) -> Option<([Finder; 3], usize)> {
    let ms = [a.module, b.module, c.module];
    let (mn, mx) = ms
        .iter()
        .fold((f32::MAX, 0f32), |(l, h), &m| (l.min(m), h.max(m)));
    if mx > mn * 1.6 {
        return None;
    }
    let d = |p: &Finder, q: &Finder| (p.x - q.x).hypot(p.y - q.y);
    let (ab, bc, ca) = (d(a, b), d(b, c), d(c, a));
    // the corner is opposite the longest side
    let (tl, p, q) = if bc >= ab && bc >= ca {
        (a, b, c)
    } else if ca >= ab && ca >= bc {
        (b, c, a)
    } else {
        (c, a, b)
    };
    let (s1, s2) = (d(tl, p), d(tl, q));
    let hyp = d(p, q);
    if s1.min(s2) < 7.0 * mn || (s1 - s2).abs() > 0.45 * s1.max(s2) {
        return None;
    }
    // right angle: hyp² ≈ s1² + s2²
    if (hyp * hyp - s1 * s1 - s2 * s2).abs() > 0.35 * (s1 * s1 + s2 * s2) {
        return None;
    }
    let cross = (p.x - tl.x) * (q.y - tl.y) - (p.y - tl.y) * (q.x - tl.x);
    let (tr, bl) = if cross > 0.0 { (p, q) } else { (q, p) };
    // The module along the symbol's own axes (runs across a turned finder are longer).
    let along = [
        module_along(img, tl, tr),
        module_along(img, tr, tl),
        module_along(img, tl, bl),
        module_along(img, bl, tl),
    ];
    let got: Vec<f32> = along.iter().flatten().copied().collect();
    let module = if got.is_empty() {
        (tl.module + tr.module + bl.module) / 3.0
    } else {
        got.iter().sum::<f32>() / got.len() as f32
    };
    let mut fs = [*tl, *tr, *bl];
    for f in &mut fs {
        f.module = module;
    }
    let est = ((d(tl, tr) + d(tl, bl)) / 2.0 / module).round() as i32 + 7;
    // nearest 17 + 4v
    let v = ((est - 17) as f32 / 4.0).round() as i32;
    if !(1..=40).contains(&v) {
        return None;
    }
    Some((fs, (17 + 4 * v) as usize))
}
