//! Vector masks (`vmsk` / `vsms`): Bézier paths in fractions of the document, rasterized to
//! a coverage plane (area of the path inside each pixel).
//!
//! A mask is a list of path components. A subpath whose operation is −1 joins the component
//! before it; the subpaths of one component are filled together by the even-odd rule (a
//! subpath inside another cuts a hole). Components combine in order: exclude (0), union
//! (1), subtract (2), intersect (3); a first component that subtracts or intersects starts
//! from everything. A mask with an "initial fill" record and no path shows everything.
//!
//! Coverage comes from an accumulation buffer: each edge adds its signed area to the cells
//! it crosses, and a running sum along each row gives the winding coverage, folded to the
//! even-odd rule.

use crate::parse::Rd;

/// Records past this many are ignored.
const MAX_RECORDS: usize = 1 << 16;
/// Rows of edges rasterized per component, at most (a hostile path cannot take forever;
/// edges past it are left out).
const ROW_BUDGET: usize = 1 << 26;

#[derive(Clone, Copy, Debug)]
struct Knot {
    before: (f64, f64),
    anchor: (f64, f64),
    after: (f64, f64),
}

#[derive(Debug, Default)]
struct Subpath {
    closed: bool,
    operation: i16,
    knots: Vec<Knot>,
}

/// A parsed vector mask.
#[derive(Debug, Default)]
pub(crate) struct VectorMask {
    pub invert: bool,
    pub disabled: bool,
    initial_fill: bool,
    paths: Vec<Subpath>,
}

fn fixed(r: &mut Rd) -> Option<f64> {
    Some(f64::from(r.i32()?) / f64::from(1u32 << 24))
}

/// A `vmsk` / `vsms` block body.
pub(crate) fn parse(b: &[u8]) -> Option<VectorMask> {
    let mut r = Rd::new(b, 0);
    let _version = r.u32()?;
    let flags = r.u32()?;
    let mut vm = VectorMask {
        invert: flags & 1 != 0,
        disabled: flags & 4 != 0,
        ..VectorMask::default()
    };
    let mut left = 0usize;
    for _ in 0..MAX_RECORDS {
        let Some(sel) = r.u16() else { break };
        let Some(rec) = r.bytes(24) else { break };
        let mut q = Rd::new(rec, 0);
        match sel {
            0 | 3 => {
                left = q.u16()? as usize;
                let operation = q.i16()?;
                vm.paths.push(Subpath {
                    closed: sel == 0,
                    operation,
                    knots: Vec::new(),
                });
            }
            1 | 2 | 4 | 5 => {
                let mut pt = || -> Option<(f64, f64)> {
                    let y = fixed(&mut q)?;
                    let x = fixed(&mut q)?;
                    Some((x, y))
                };
                let k = Knot {
                    before: pt()?,
                    anchor: pt()?,
                    after: pt()?,
                };
                if left > 0 {
                    if let Some(p) = vm.paths.last_mut() {
                        p.knots.push(k);
                    }
                    left -= 1;
                }
            }
            8 => vm.initial_fill = q.u16()? != 0,
            _ => {}
        }
    }
    Some(vm)
}

/// Accumulation buffer for one coverage plane.
struct Acc {
    w: usize,
    h: usize,
    /// `w + 2` cells per row.
    a: Vec<f32>,
    budget: usize,
}

impl Acc {
    fn new(w: usize, h: usize) -> Acc {
        Acc {
            w,
            h,
            a: vec![0.0; (w + 2) * h],
            budget: ROW_BUDGET,
        }
    }

    /// Adds the edge `p0`→`p1` (pixel coordinates).
    fn line(&mut self, p0: (f64, f64), p1: (f64, f64)) {
        if p0.1 == p1.1 || !(p0.0.is_finite() && p0.1.is_finite() && p1.0.is_finite()) {
            return;
        }
        let (dir, a, b) = if p0.1 < p1.1 {
            (1.0f32, p0, p1)
        } else {
            (-1.0f32, p1, p0)
        };
        let hf = self.h as f64;
        if b.1 <= 0.0 || a.1 >= hf {
            return;
        }
        let dxdy = (b.0 - a.0) / (b.1 - a.1);
        let wmax = self.w as f64;
        let stride = self.w + 2;
        let ystart = a.1.max(0.0);
        let mut x = a.0 + (ystart - a.1) * dxdy;
        let y0 = ystart.floor() as usize;
        let y1 = (b.1.min(hf)).ceil() as usize;
        let rows = y1.saturating_sub(y0);
        if rows > self.budget {
            self.budget = 0;
            return;
        }
        self.budget -= rows;
        for y in y0..y1.min(self.h) {
            let yf = y as f64;
            let dy = (yf + 1.0).min(b.1) - yf.max(a.1);
            let xnext = x + dxdy * dy;
            let d = dy as f32 * dir;
            let (lo, hi) = if x < xnext { (x, xnext) } else { (xnext, x) };
            let lo = lo.clamp(0.0, wmax);
            let hi = hi.clamp(0.0, wmax);
            let row = &mut self.a[y * stride..(y + 1) * stride];
            let l0 = lo.floor();
            let i0 = l0 as usize;
            let i1 = hi.ceil() as usize;
            if i1 <= i0 + 1 {
                let xm = ((lo + hi) * 0.5 - l0) as f32;
                row[i0] += d - d * xm;
                row[i0 + 1] += d * xm;
            } else {
                let s = (1.0 / (hi - lo)) as f32;
                let f0 = (lo - l0) as f32;
                let a0 = 0.5 * s * (1.0 - f0) * (1.0 - f0);
                let f1 = (hi - hi.ceil() + 1.0) as f32;
                let am = 0.5 * s * f1 * f1;
                row[i0] += d * a0;
                if i1 == i0 + 2 {
                    row[i0 + 1] += d * (1.0 - a0 - am);
                } else {
                    let a1 = s * (1.5 - f0);
                    row[i0 + 1] += d * (a1 - a0);
                    for c in &mut row[i0 + 2..i1 - 1] {
                        *c += d * s;
                    }
                    let a2 = a1 + (i1 - i0 - 3) as f32 * s;
                    row[i1 - 1] += d * (1.0 - a2 - am);
                }
                row[i1] += d * am;
            }
            x = xnext;
        }
    }

    /// Coverage of the union of what was drawn (all wound one way), `w * h`.
    fn union_coverage(&self) -> Vec<f32> {
        let stride = self.w + 2;
        let mut out = vec![0f32; self.w * self.h];
        for (y, o) in out.chunks_mut(self.w).enumerate() {
            let mut s = 0f32;
            for (x, v) in o.iter_mut().enumerate() {
                s += self.a[y * stride + x];
                *v = s.abs().min(1.0);
            }
        }
        out
    }

    /// Even-odd coverage, `w * h`.
    fn coverage(&self) -> Vec<f32> {
        let stride = self.w + 2;
        let mut out = vec![0f32; self.w * self.h];
        for (y, o) in out.chunks_mut(self.w).enumerate() {
            let mut s = 0f32;
            for (x, v) in o.iter_mut().enumerate() {
                s += self.a[y * stride + x];
                let m = s.abs() % 2.0;
                *v = if m > 1.0 { 2.0 - m } else { m };
            }
        }
        out
    }
}

/// Points along the cubic `p0 c0 c1 p1`, `p0` left out.
fn cubic(
    out: &mut Vec<(f64, f64)>,
    p0: (f64, f64),
    c0: (f64, f64),
    c1: (f64, f64),
    p1: (f64, f64),
) {
    let d = |a: (f64, f64), b: (f64, f64)| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
    let len = d(p0, c0) + d(c0, c1) + d(c1, p1);
    let n = if c0 == p0 && c1 == p1 {
        1
    } else {
        (len / 2.0).ceil().clamp(1.0, 512.0) as usize
    };
    for i in 1..=n {
        let t = i as f64 / n as f64;
        let u = 1.0 - t;
        let (a, b, c, e) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        out.push((
            a * p0.0 + b * c0.0 + c * c1.0 + e * p1.0,
            a * p0.1 + b * c0.1 + c * c1.1 + e * p1.1,
        ));
    }
}

impl VectorMask {
    /// Coverage over a `w`×`h` document, 0..1.
    pub fn raster(&self, w: usize, h: usize) -> Vec<f32> {
        let n = w * h;
        if self.paths.is_empty() {
            let v = if self.initial_fill { 1.0 } else { 0.0 };
            return self.finish(vec![v; n]);
        }
        let (sw, sh) = (w as f64, h as f64);
        let mut mask = vec![0f32; n];
        let mut first = true;
        let mut k = 0;
        while k < self.paths.len() {
            let mut end = k + 1;
            while end < self.paths.len() && self.paths[end].operation == -1 {
                end += 1;
            }
            let mut acc = Acc::new(w, h);
            for p in &self.paths[k..end] {
                if p.knots.len() < 2 {
                    continue;
                }
                let s = |q: (f64, f64)| (q.0 * sw, q.1 * sh);
                let mut pts = vec![s(p.knots[0].anchor)];
                let m = p.knots.len();
                let segs = if p.closed { m } else { m - 1 };
                for i in 0..segs {
                    let (a, b) = (p.knots[i], p.knots[(i + 1) % m]);
                    cubic(&mut pts, s(a.anchor), s(a.after), s(b.before), s(b.anchor));
                }
                // Filling closes an open subpath too.
                for i in 0..pts.len() {
                    acc.line(pts[i], pts[(i + 1) % pts.len()]);
                }
            }
            let plane = acc.coverage();
            let op = self.paths[k].operation;
            if first && matches!(op, 2 | 3) {
                mask.iter_mut().for_each(|v| *v = 1.0 - *v);
            }
            for (m, &p) in mask.iter_mut().zip(&plane) {
                *m = match op {
                    0 => *m + p - 2.0 * *m * p,
                    2 => (*m - p).max(0.0),
                    3 => *m * p,
                    _ => *m + p - *m * p,
                };
            }
            first = false;
            k = end;
        }
        self.finish(mask)
    }

    fn finish(&self, mut m: Vec<f32>) -> Vec<f32> {
        for v in &mut m {
            *v = v.clamp(0.0, 1.0);
            if self.invert {
                *v = 1.0 - *v;
            }
        }
        m
    }

    /// The bounds of the anchors in document pixels (left, top, right, bottom), if any.
    pub fn bounds(&self, w: usize, h: usize) -> Option<(f64, f64, f64, f64)> {
        let mut b: Option<(f64, f64, f64, f64)> = None;
        for k in self.paths.iter().flat_map(|p| &p.knots) {
            let (x, y) = (k.anchor.0 * w as f64, k.anchor.1 * h as f64);
            b = Some(match b {
                None => (x, y, x, y),
                Some((l, t, r, bt)) => (l.min(x), t.min(y), r.max(x), bt.max(y)),
            });
        }
        b
    }
}

/// Where a stroke lies on its path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Align {
    Inside,
    Center,
    Outside,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cap {
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Join {
    Miter,
    Round,
    Bevel,
}

/// A shape's stroke (`vstk`) geometry.
#[derive(Clone, Debug)]
pub(crate) struct StrokeStyle {
    /// Width in pixels.
    pub width: f64,
    pub align: Align,
    pub cap: Cap,
    pub join: Join,
    pub miter_limit: f64,
    /// Dash and gap lengths in widths; empty: solid.
    pub dashes: Vec<f64>,
    /// Where the dash pattern starts, in widths.
    pub dash_offset: f64,
}

/// Sides of the polygon standing for a round join or cap.
const ROUND_SIDES: usize = 24;

/// Polygons whose union is a stroke, wound one way.
struct Outline {
    polys: Vec<Vec<(f64, f64)>>,
}

impl Outline {
    fn push(&mut self, mut p: Vec<(f64, f64)>) {
        if p.len() < 3 {
            return;
        }
        let area: f64 = (0..p.len())
            .map(|i| {
                let (a, b) = (p[i], p[(i + 1) % p.len()]);
                a.0 * b.1 - b.0 * a.1
            })
            .sum();
        if area < 0.0 {
            p.reverse();
        }
        self.polys.push(p);
    }

    fn disc(&mut self, c: (f64, f64), r: f64) {
        let p = (0..ROUND_SIDES)
            .map(|i| {
                let t = i as f64 / ROUND_SIDES as f64 * std::f64::consts::TAU;
                (c.0 + r * t.cos(), c.1 + r * t.sin())
            })
            .collect();
        self.push(p);
    }

    /// The stroke of polyline `pts` (closed or open) at half-width `hw`.
    fn polyline(&mut self, pts: &[(f64, f64)], closed: bool, hw: f64, st: &StrokeStyle) {
        let pts: Vec<(f64, f64)> = {
            let mut v: Vec<(f64, f64)> = Vec::with_capacity(pts.len());
            for &p in pts {
                if v.last().is_none_or(|q| (q.0 - p.0).hypot(q.1 - p.1) > 1e-6) {
                    v.push(p);
                }
            }
            if closed && v.len() > 2 && {
                let (a, b) = (v[0], v[v.len() - 1]);
                (a.0 - b.0).hypot(a.1 - b.1) <= 1e-6
            } {
                v.pop();
            }
            v
        };
        let n = pts.len();
        if n == 0 {
            return;
        }
        if n == 1 {
            if !closed && st.cap == Cap::Round {
                self.disc(pts[0], hw);
            } else if !closed && st.cap == Cap::Square {
                let (x, y) = pts[0];
                self.push(vec![
                    (x - hw, y - hw),
                    (x + hw, y - hw),
                    (x + hw, y + hw),
                    (x - hw, y + hw),
                ]);
            }
            return;
        }
        let segs = if closed { n } else { n - 1 };
        let normal = |a: (f64, f64), b: (f64, f64)| {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let l = dx.hypot(dy).max(1e-12);
            ((-dy / l * hw, dx / l * hw), (dx / l, dy / l))
        };
        for i in 0..segs {
            let (mut a, mut b) = (pts[i], pts[(i + 1) % n]);
            let (nv, d) = normal(a, b);
            if !closed && st.cap == Cap::Square {
                if i == 0 {
                    a = (a.0 - d.0 * hw, a.1 - d.1 * hw);
                }
                if i == segs - 1 {
                    b = (b.0 + d.0 * hw, b.1 + d.1 * hw);
                }
            }
            self.push(vec![
                (a.0 + nv.0, a.1 + nv.1),
                (b.0 + nv.0, b.1 + nv.1),
                (b.0 - nv.0, b.1 - nv.1),
                (a.0 - nv.0, a.1 - nv.1),
            ]);
        }
        // Joins at the inner vertices (every vertex of a closed path).
        let joins: Vec<usize> = if closed {
            (0..n).collect()
        } else {
            (1..n - 1).collect()
        };
        for i in joins {
            let (p, v, q) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
            let (n1, d1) = normal(p, v);
            let (n2, d2) = normal(v, q);
            let turn = d1.0 * d2.1 - d1.1 * d2.0;
            if turn.abs() < 1e-9 && d1.0 * d2.0 + d1.1 * d2.1 > 0.0 {
                continue;
            }
            // The outer side of the turn.
            let s = if turn > 0.0 { -1.0 } else { 1.0 };
            let o1 = (v.0 + s * n1.0, v.1 + s * n1.1);
            let o2 = (v.0 + s * n2.0, v.1 + s * n2.1);
            match st.join {
                Join::Round => self.disc(v, hw),
                Join::Bevel => self.push(vec![v, o1, o2]),
                Join::Miter => {
                    // The tip where the two outer edges meet.
                    let cos = (d1.0 * d2.0 + d1.1 * d2.1).clamp(-1.0, 1.0);
                    let half = ((1.0 + cos) / 2.0).sqrt().max(1e-9);
                    let ratio = 1.0 / half;
                    if ratio <= st.miter_limit.max(1.0) {
                        let m = ((o1.0 + o2.0) / 2.0 - v.0, (o1.1 + o2.1) / 2.0 - v.1);
                        let ml = m.0.hypot(m.1).max(1e-12);
                        let tip = (v.0 + m.0 / ml * hw * ratio, v.1 + m.1 / ml * hw * ratio);
                        self.push(vec![v, o1, tip, o2]);
                    } else {
                        self.push(vec![v, o1, o2]);
                    }
                }
            }
        }
        if !closed && st.cap == Cap::Round {
            self.disc(pts[0], hw);
            self.disc(pts[n - 1], hw);
        }
    }
}

/// `pts` cut into dashes: the pattern `dash` (lengths in pixels: on, off, on…) from
/// `offset`.
fn dashed(pts: &[(f64, f64)], closed: bool, dash: &[f64], offset: f64) -> Vec<Vec<(f64, f64)>> {
    let total: f64 = dash.iter().sum();
    if total <= 1e-6 || pts.len() < 2 {
        return vec![pts.to_vec()];
    }
    let mut path = pts.to_vec();
    if closed {
        path.push(pts[0]);
    }
    let mut out = Vec::new();
    let mut k = 0usize;
    let mut left = dash[0];
    // Skip the offset into the pattern.
    let mut skip = offset.rem_euclid(total);
    while skip > 0.0 {
        if skip >= left {
            skip -= left;
            k = (k + 1) % dash.len();
            left = dash[k];
        } else {
            left -= skip;
            skip = 0.0;
        }
    }
    let mut cur: Vec<(f64, f64)> = if k.is_multiple_of(2) {
        vec![path[0]]
    } else {
        Vec::new()
    };
    let mut guard = 0usize;
    for w in path.windows(2) {
        let (mut a, b) = (w[0], w[1]);
        let mut len = (b.0 - a.0).hypot(b.1 - a.1);
        while len > 1e-9 && guard < 1 << 20 {
            guard += 1;
            let step = left.min(len);
            let t = step / len;
            let p = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            if k.is_multiple_of(2) {
                cur.push(p);
            }
            len -= step;
            left -= step;
            a = p;
            if left <= 1e-9 {
                if k.is_multiple_of(2) && cur.len() > 1 {
                    out.push(std::mem::take(&mut cur));
                }
                cur.clear();
                k = (k + 1) % dash.len();
                left = dash[k].max(1e-3);
                if k.is_multiple_of(2) {
                    cur.push(a);
                }
            }
        }
    }
    if k.is_multiple_of(2) && cur.len() > 1 {
        out.push(cur);
    }
    out
}

impl VectorMask {
    /// The flattened subpaths in pixels, with whether each is closed.
    fn polylines(&self, w: usize, h: usize) -> Vec<(Vec<(f64, f64)>, bool)> {
        let (sw, sh) = (w as f64, h as f64);
        let s = |q: (f64, f64)| (q.0 * sw, q.1 * sh);
        self.paths
            .iter()
            .filter(|p| !p.knots.is_empty())
            .map(|p| {
                let mut pts = vec![s(p.knots[0].anchor)];
                let m = p.knots.len();
                let segs = if p.closed { m } else { m - 1 };
                for i in 0..segs {
                    let (a, b) = (p.knots[i], p.knots[(i + 1) % m]);
                    cubic(&mut pts, s(a.anchor), s(a.after), s(b.before), s(b.anchor));
                }
                (pts, p.closed)
            })
            .collect()
    }

    /// The coverage of the stroke `st` along the paths over a `w`×`h` document, 0..1.
    pub fn stroke_raster(&self, w: usize, h: usize, st: &StrokeStyle) -> Vec<f32> {
        let hw = match st.align {
            Align::Center => st.width / 2.0,
            _ => st.width,
        }
        .max(0.0);
        let mut outline = Outline { polys: Vec::new() };
        for (pts, closed) in self.polylines(w, h) {
            if st.dashes.is_empty() {
                outline.polyline(&pts, closed, hw, st);
            } else {
                let dash: Vec<f64> = st.dashes.iter().map(|d| d * st.width).collect();
                for piece in dashed(&pts, closed, &dash, st.dash_offset * st.width) {
                    outline.polyline(&piece, false, hw, st);
                }
            }
        }
        let mut acc = Acc::new(w, h);
        for p in &outline.polys {
            for i in 0..p.len() {
                acc.line(p[i], p[(i + 1) % p.len()]);
            }
        }
        let mut cov = acc.union_coverage();
        if st.align != Align::Center {
            let fill = self.raster(w, h);
            for (c, f) in cov.iter_mut().zip(&fill) {
                *c *= if st.align == Align::Inside {
                    *f
                } else {
                    1.0 - f
                };
            }
        }
        cov
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect_mask(l: f64, t: f64, r: f64, b: f64, w: f64, h: f64) -> VectorMask {
        let k = |x: f64, y: f64| Knot {
            before: (x / w, y / h),
            anchor: (x / w, y / h),
            after: (x / w, y / h),
        };
        VectorMask {
            paths: vec![Subpath {
                closed: true,
                operation: 1,
                knots: vec![k(l, t), k(r, t), k(r, b), k(l, b)],
            }],
            ..VectorMask::default()
        }
    }

    #[test]
    fn rectangle_coverage_is_its_area_per_pixel() {
        let m = rect_mask(1.5, 1.0, 4.0, 3.25, 6.0, 4.0).raster(6, 4);
        let at = |x: usize, y: usize| m[y * 6 + x];
        assert!((at(0, 1)).abs() < 1e-6);
        assert!((at(1, 1) - 0.5).abs() < 1e-5);
        assert!((at(2, 2) - 1.0).abs() < 1e-5);
        assert!((at(3, 3) - 0.25).abs() < 1e-5);
        assert!((at(4, 2)).abs() < 1e-6);
        let total: f32 = m.iter().sum();
        assert!((total - 2.5 * 2.25).abs() < 1e-4, "{total}");
    }

    #[test]
    fn subtract_first_starts_from_everything_and_invert_flips() {
        let mut v = rect_mask(0.0, 0.0, 2.0, 2.0, 4.0, 4.0);
        v.paths[0].operation = 2;
        let m = v.raster(4, 4);
        assert_eq!(m[0], 0.0);
        assert_eq!(m[15], 1.0);
        v.invert = true;
        let m = v.raster(4, 4);
        assert_eq!(m[0], 1.0);
        assert_eq!(m[15], 0.0);
    }

    #[test]
    fn inner_subpath_of_one_component_is_a_hole() {
        let mut v = rect_mask(0.0, 0.0, 8.0, 8.0, 8.0, 8.0);
        let inner = rect_mask(2.0, 2.0, 6.0, 6.0, 8.0, 8.0);
        let mut p = inner.paths.into_iter().next().unwrap();
        p.operation = -1;
        v.paths.push(p);
        let m = v.raster(8, 8);
        assert_eq!(m[0], 1.0);
        assert!(m[4 * 8 + 4] < 1e-6);
    }

    #[test]
    fn parses_records() {
        // version 3, flags 0, one closed length record (2 knots), two knots, fill rule.
        let mut b = vec![0, 0, 0, 3, 0, 0, 0, 0];
        let rec = |sel: u16, body: &[u8]| {
            let mut r = sel.to_be_bytes().to_vec();
            r.extend_from_slice(body);
            r.resize(26, 0);
            r
        };
        b.extend(rec(6, &[]));
        b.extend(rec(0, &[0, 2, 0, 1]));
        let half = (1i32 << 23).to_be_bytes();
        let knot: Vec<u8> = (0..6).flat_map(|_| half).collect();
        b.extend(rec(1, &knot));
        b.extend(rec(2, &knot));
        let v = parse(&b).unwrap();
        assert_eq!(v.paths.len(), 1);
        assert_eq!(v.paths[0].knots.len(), 2);
        assert!((v.paths[0].knots[1].anchor.0 - 0.5).abs() < 1e-9);
    }
}
