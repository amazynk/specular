//! Terrain from an elevation grid: slope and aspect (Horn's 3×3 weights, as `gdaldem`
//! computes them) and contour lines (marching squares over the cell centres, as
//! `gdal_contour` traces them), with a "nice" interval and index contours for labels.
//!
//! Grids are row-major, rows top-down, NaN (any non-finite value) for no data. Positions
//! are stored pixels: (0, 0) the top-left corner, the centre of cell (x, y) at
//! (x + 0.5, y + 0.5).

use std::collections::HashMap;

/// An elevation grid with its ground step.
#[derive(Clone, Copy, Debug)]
pub struct Dem<'a> {
    pub values: &'a [f32],
    pub w: usize,
    pub h: usize,
    /// Ground metres from one cell to the next: east along a row (x), and north down a
    /// column (y; negative when rows run south, as in a north-up map).
    pub step: [f64; 2],
    /// Metres per value unit (0.3048 for heights in feet), times any exaggeration.
    pub z: f64,
}

impl Dem<'_> {
    fn at(&self, x: usize, y: usize) -> f32 {
        self.values[y * self.w + x]
    }

    /// The gradient at cell (`x`, `y`): metres of height per metre east and per metre
    /// north, from the 3×3 neighbours with Horn's weights. Neighbours past the edges are
    /// the edge's own; neighbours without data take the centre's value (`gdaldem
    /// -compute_edges` does the same for no data). `None` where the cell has none.
    pub fn gradient(&self, x: usize, y: usize) -> Option<(f64, f64)> {
        if x >= self.w || y >= self.h || self.values.len() < self.w * self.h {
            return None;
        }
        let c = self.at(x, y);
        if !c.is_finite() {
            return None;
        }
        let n = |dx: isize, dy: isize| {
            let xi = (x as isize + dx).clamp(0, self.w as isize - 1) as usize;
            let yi = (y as isize + dy).clamp(0, self.h as isize - 1) as usize;
            let v = self.at(xi, yi);
            f64::from(if v.is_finite() { v } else { c })
        };
        let ex = (n(1, -1) + 2.0 * n(1, 0) + n(1, 1)) - (n(-1, -1) + 2.0 * n(-1, 0) + n(-1, 1));
        let ey = (n(-1, 1) + 2.0 * n(0, 1) + n(1, 1)) - (n(-1, -1) + 2.0 * n(0, -1) + n(1, -1));
        let [sx, sy] = self.step;
        if sx == 0.0 || sy == 0.0 || !sx.is_finite() || !sy.is_finite() {
            return None;
        }
        Some((ex * self.z / (8.0 * sx), ey * self.z / (8.0 * sy)))
    }

    /// The slope at cell (`x`, `y`) in degrees from the horizontal.
    pub fn slope(&self, x: usize, y: usize) -> Option<f64> {
        let (gx, gy) = self.gradient(x, y)?;
        Some(gx.hypot(gy).atan().to_degrees())
    }

    /// The aspect at cell (`x`, `y`): the compass direction the slope faces (downhill),
    /// degrees clockwise from north in 0..360. `None` on flat ground and without data.
    pub fn aspect(&self, x: usize, y: usize) -> Option<f64> {
        let (gx, gy) = self.gradient(x, y)?;
        aspect_of(gx, gy)
    }
}

/// The aspect of a gradient (metres per metre east, north): degrees clockwise from north
/// that the ground falls towards, 0..360; `None` when flat.
pub fn aspect_of(gx: f64, gy: f64) -> Option<f64> {
    if gx == 0.0 && gy == 0.0 {
        return None;
    }
    let a = (-gx).atan2(-gy).to_degrees().rem_euclid(360.0);
    Some(if a >= 360.0 { 0.0 } else { a })
}

/// The compass point (0 north, 1 north-east … 7 north-west) nearest `degrees`.
pub fn compass_point(degrees: f64) -> usize {
    ((degrees.rem_euclid(360.0) / 45.0).round() as usize) % 8
}

/// A contour interval for heights between `lo` and `hi`: 1, 2, 2.5 or 5 times a power of
/// ten, the largest giving at least `lines` contours (`None` for an empty range).
pub fn nice_interval(lo: f64, hi: f64, lines: usize) -> Option<f64> {
    let span = hi - lo;
    if !(span.is_finite() && span > 0.0) || lines == 0 {
        return None;
    }
    let raw = span / lines as f64;
    let p = 10f64.powf(raw.log10().floor());
    [5.0, 2.5, 2.0, 1.0]
        .into_iter()
        .map(|m| m * p)
        .find(|&s| s <= raw)
        .or(Some(p))
}

/// Contours `interval` apart from `base` (index contours every `index_every`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Levels {
    pub base: f64,
    pub interval: f64,
    pub index_every: u32,
}

impl Levels {
    /// The levels between `lo` and `hi` (inclusive), lowest first.
    pub fn between(&self, lo: f64, hi: f64) -> Vec<f64> {
        if !(self.interval > 0.0 && lo.is_finite() && hi.is_finite()) || hi < lo {
            return Vec::new();
        }
        let k0 = ((lo - self.base) / self.interval).ceil() as i64;
        let k1 = ((hi - self.base) / self.interval).floor() as i64;
        if k1 - k0 > 100_000 {
            return Vec::new();
        }
        (k0..=k1)
            .map(|k| self.base + k as f64 * self.interval)
            .collect()
    }

    /// Whether `level` is an index contour (drawn bolder and labelled).
    pub fn is_index(&self, level: f64) -> bool {
        let n = u64::from(self.index_every.max(1)) as f64;
        let k = ((level - self.base) / self.interval).round();
        (k / n - (k / n).round()).abs() < 1e-6
    }
}

/// One contour line in stored pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Contour {
    pub level: f64,
    pub index: bool,
    /// The line through the cell edges it crosses; the last point repeats the first when
    /// it closes.
    pub points: Vec<(f32, f32)>,
    pub closed: bool,
}

/// Contour lines of `values` (`w`×`h`) at `levels`, in stored pixels, by marching squares
/// over the cell centres. As `gdal_contour` draws them, lines reach the edges of the data:
/// past the outer cells and next to cells without data, the line runs on to the cell's
/// edge (half a cell). A square with a corner without data is split into quarters around
/// its valid corners, edge midpoints and centre taking the mean of the valid values. Values
/// equal to a level count as above it; saddles are split by the square's mean.
pub fn contours(values: &[f32], w: usize, h: usize, levels: &Levels) -> Vec<Contour> {
    if w == 0 || h == 0 || values.len() < w * h {
        return Vec::new();
    }
    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
    for &v in &values[..w * h] {
        if v.is_finite() {
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }
    let all = levels.between(f64::from(lo), f64::from(hi));
    if all.is_empty() {
        return Vec::new();
    }
    let v = |x: isize, y: isize| -> f32 {
        if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
            return f32::NAN;
        }
        let a = values[y as usize * w + x as usize];
        if a.is_finite() { a } else { f32::NAN }
    };
    let lattice = Lattice { lw: 2 * w + 3 };
    let mut segs: Vec<Vec<Seg>> = vec![Vec::new(); all.len()];
    let first_at = |v: f32| all.partition_point(|&l| (l as f32) <= v);
    for y in -1..h as isize {
        for x in -1..w as isize {
            let c = [v(x, y), v(x + 1, y), v(x + 1, y + 1), v(x, y + 1)];
            let valid = c.iter().filter(|a| a.is_finite()).count();
            if valid == 0 {
                continue;
            }
            let (mn, mx) = c
                .iter()
                .filter(|a| a.is_finite())
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), &x| {
                    (a.min(x), b.max(x))
                });
            let k0 = first_at(mn);
            if k0 >= all.len() || (all[k0] as f32) > mx {
                continue;
            }
            let (lx, ly) = (2 * (x + 1) as usize, 2 * (y + 1) as usize);
            let mut squares: Vec<Sq> = Vec::with_capacity(4);
            if valid == 4 {
                squares.push(Sq {
                    lx,
                    ly,
                    size: 2,
                    v: c,
                });
            } else {
                // Midpoints of the edges and the centre, from the valid corners.
                let mid = |a: f32, b: f32| match (a.is_finite(), b.is_finite()) {
                    (true, true) => (a + b) / 2.0,
                    (true, false) => a,
                    (false, true) => b,
                    _ => f32::NAN,
                };
                let (top, right, bottom, left) = (
                    mid(c[0], c[1]),
                    mid(c[1], c[2]),
                    mid(c[3], c[2]),
                    mid(c[0], c[3]),
                );
                let centre = c.iter().filter(|a| a.is_finite()).sum::<f32>() / valid as f32;
                if c[0].is_finite() {
                    squares.push(Sq {
                        lx,
                        ly,
                        size: 1,
                        v: [c[0], top, centre, left],
                    });
                }
                if c[1].is_finite() {
                    squares.push(Sq {
                        lx: lx + 1,
                        ly,
                        size: 1,
                        v: [top, c[1], right, centre],
                    });
                }
                if c[2].is_finite() {
                    squares.push(Sq {
                        lx: lx + 1,
                        ly: ly + 1,
                        size: 1,
                        v: [centre, right, c[2], bottom],
                    });
                }
                if c[3].is_finite() {
                    squares.push(Sq {
                        lx,
                        ly: ly + 1,
                        size: 1,
                        v: [left, centre, bottom, c[3]],
                    });
                }
            }
            let mut k = k0;
            while k < all.len() && (all[k] as f32) <= mx {
                let level = all[k] as f32;
                for s in &squares {
                    s.march(level, &lattice, &mut segs[k]);
                }
                k += 1;
            }
        }
    }
    let mut out = Vec::new();
    for (k, segs) in segs.into_iter().enumerate() {
        if segs.is_empty() {
            continue;
        }
        let level = all[k];
        let index = levels.is_index(level);
        let mut point: HashMap<usize, (f32, f32)> = HashMap::with_capacity(segs.len() * 2);
        let ids: Vec<[usize; 2]> = segs
            .iter()
            .map(|s| {
                for (id, p) in s {
                    point.entry(*id).or_insert(*p);
                }
                [s[0].0, s[1].0]
            })
            .collect();
        for (pts, closed) in chain(&ids) {
            out.push(Contour {
                level,
                index,
                points: pts.into_iter().map(|e| point[&e]).collect(),
                closed,
            });
        }
    }
    out
}

/// A segment of a contour: the unit edges it joins and where it crosses them.
type Seg = [(usize, (f32, f32)); 2];

/// The half-cell lattice the squares' edges lie on: point (i, j) is at stored pixel
/// (i/2 − 0.5, j/2 − 0.5); `lw` points a row.
struct Lattice {
    lw: usize,
}

impl Lattice {
    /// The id of the unit edge from lattice point (i, j) east (`down` false) or south.
    fn edge(&self, i: usize, j: usize, down: bool) -> usize {
        2 * (j * self.lw + i) + usize::from(down)
    }
}

/// A square of the lattice with all four values: top-left at (`lx`, `ly`), `size` lattice
/// units a side (2: between four cell centres; 1: a quarter of one), values top-left,
/// top-right, bottom-right, bottom-left.
struct Sq {
    lx: usize,
    ly: usize,
    size: usize,
    v: [f32; 4],
}

impl Sq {
    /// The crossing of `level` on the edge from corner `p` to corner `q` (both of the
    /// square, in the order east or south): its unit edge's id and position.
    fn cross(&self, lat: &Lattice, level: f32, p: usize, q: usize) -> (usize, (f32, f32)) {
        let corner = |k: usize| match k {
            0 => (self.lx, self.ly),
            1 => (self.lx + self.size, self.ly),
            2 => (self.lx + self.size, self.ly + self.size),
            _ => (self.lx, self.ly + self.size),
        };
        let ((pi, pj), (qi, qj)) = (corner(p), corner(q));
        let (a, b) = (self.v[p], self.v[q]);
        let t = if a != b {
            ((level - a) / (b - a)).clamp(0.0, 1.0)
        } else {
            0.5
        };
        let down = qj > pj;
        let half = usize::from(self.size == 2 && t >= 0.5);
        let id = if down {
            lat.edge(pi, pj + half, true)
        } else {
            lat.edge(pi + half, pj, false)
        };
        let pos = |l: usize| l as f32 / 2.0 - 0.5;
        let at = (
            pos(pi) + t * (pos(qi) - pos(pi)),
            pos(pj) + t * (pos(qj) - pos(pj)),
        );
        (id, at)
    }

    /// The square's segments at `level`.
    fn march(&self, level: f32, lat: &Lattice, out: &mut Vec<Seg>) {
        let [a, b, c, d] = self.v;
        let code = usize::from(a >= level)
            | usize::from(b >= level) << 1
            | usize::from(c >= level) << 2
            | usize::from(d >= level) << 3;
        if code == 0 || code == 15 {
            return;
        }
        let top = || self.cross(lat, level, 0, 1);
        let right = || self.cross(lat, level, 1, 2);
        let bottom = || self.cross(lat, level, 3, 2);
        let left = || self.cross(lat, level, 0, 3);
        let centre = (a + b + c + d) / 4.0 >= level;
        match code {
            1 | 14 => out.push([left(), top()]),
            2 | 13 => out.push([top(), right()]),
            3 | 12 => out.push([left(), right()]),
            4 | 11 => out.push([right(), bottom()]),
            6 | 9 => out.push([top(), bottom()]),
            7 | 8 => out.push([left(), bottom()]),
            5 if centre => out.extend([[left(), bottom()], [top(), right()]]),
            5 => out.extend([[left(), top()], [right(), bottom()]]),
            10 if centre => out.extend([[left(), top()], [right(), bottom()]]),
            _ => out.extend([[left(), bottom()], [top(), right()]]),
        }
    }
}

/// Chains segments (pairs of edge ids) through their shared edges into lines.
fn chain(segs: &[[usize; 2]]) -> Vec<(Vec<usize>, bool)> {
    let mut at: HashMap<usize, [usize; 2]> = HashMap::with_capacity(segs.len() * 2);
    const NONE: usize = usize::MAX;
    for (i, s) in segs.iter().enumerate() {
        for e in s {
            let slot = at.entry(*e).or_insert([NONE, NONE]);
            if slot[0] == NONE {
                slot[0] = i;
            } else {
                slot[1] = i;
            }
        }
    }
    let other = |e: usize, s: usize| -> Option<usize> {
        let [p, q] = at[&e];
        if p == s {
            (q != NONE).then_some(q)
        } else {
            (p != NONE).then_some(p)
        }
    };
    let mut used = vec![false; segs.len()];
    let mut lines = Vec::new();
    // Walks from segment `s` leaving through edge `e`, appending the edges reached.
    let walk = |mut s: usize, mut e: usize, used: &mut Vec<bool>, out: &mut Vec<usize>| {
        loop {
            used[s] = true;
            e = if segs[s][0] == e {
                segs[s][1]
            } else {
                segs[s][0]
            };
            out.push(e);
            match other(e, s) {
                Some(n) if !used[n] => s = n,
                _ => break,
            }
        }
    };
    for i in 0..segs.len() {
        if used[i] {
            continue;
        }
        let start = segs[i][0];
        let mut fwd = vec![start];
        walk(i, start, &mut used, &mut fwd);
        let closed = fwd.len() > 2 && fwd.last() == Some(&start);
        if closed {
            lines.push((fwd, true));
            continue;
        }
        let mut back = Vec::new();
        if let Some(n) = other(start, i).filter(|&n| !used[n]) {
            walk(n, start, &mut used, &mut back);
        }
        back.reverse();
        back.extend(fwd);
        lines.push((back, false));
    }
    lines
}

/// A grid averaged down by `f` in each direction (no-data cells left out of each mean), for
/// contours of a grid too large to trace whole: (values, width, height).
pub fn reduce(values: &[f32], w: usize, h: usize, f: usize) -> (Vec<f32>, usize, usize) {
    let f = f.max(1);
    let (rw, rh) = (w.div_ceil(f), h.div_ceil(f));
    let mut out = vec![f32::NAN; rw * rh];
    for ry in 0..rh {
        for rx in 0..rw {
            let (mut s, mut n) = (0.0f64, 0u32);
            for y in ry * f..((ry + 1) * f).min(h) {
                for x in rx * f..((rx + 1) * f).min(w) {
                    let v = values[y * w + x];
                    if v.is_finite() {
                        s += f64::from(v);
                        n += 1;
                    }
                }
            }
            if n > 0 {
                out[ry * rw + rx] = (s / f64::from(n)) as f32;
            }
        }
    }
    (out, rw, rh)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plane rising `gx` metres per metre east and `gy` north, 30 m cells, north up.
    fn plane(gx: f64, gy: f64) -> Vec<f32> {
        let mut v = Vec::new();
        for y in 0..5 {
            for x in 0..5 {
                let (e, n) = (f64::from(x) * 30.0, -f64::from(y) * 30.0);
                v.push((100.0 + gx * e + gy * n) as f32);
            }
        }
        v
    }

    #[test]
    fn slope_and_aspect_of_planes() {
        // Rising to the east: 45° falling west.
        let v = plane(1.0, 0.0);
        let d = Dem {
            values: &v,
            w: 5,
            h: 5,
            step: [30.0, -30.0],
            z: 1.0,
        };
        assert!((d.slope(2, 2).unwrap() - 45.0).abs() < 1e-4);
        assert!((d.aspect(2, 2).unwrap() - 270.0).abs() < 1e-4);
        // Rising to the north: falls south.
        let v = plane(0.0, 0.5);
        let d = Dem { values: &v, ..d };
        assert!((d.slope(2, 2).unwrap() - 0.5f64.atan().to_degrees()).abs() < 1e-4);
        assert!((d.aspect(2, 2).unwrap() - 180.0).abs() < 1e-4);
        // Rising to the south-west: faces north-east.
        let v = plane(-0.2, -0.2);
        let d = Dem { values: &v, ..d };
        assert!((d.aspect(2, 2).unwrap() - 45.0).abs() < 1e-3);
        assert_eq!(compass_point(d.aspect(2, 2).unwrap()), 1);
        // Flat: no aspect.
        let v = vec![5.0f32; 25];
        let d = Dem { values: &v, ..d };
        assert_eq!(d.slope(2, 2), Some(0.0));
        assert_eq!(d.aspect(2, 2), None);
        assert_eq!(compass_point(359.0), 0);
        assert_eq!(compass_point(200.0), 4);
    }

    #[test]
    fn intervals_are_round() {
        assert_eq!(nice_interval(0.0, 1000.0, 10), Some(100.0));
        assert_eq!(nice_interval(0.0, 1000.0, 15), Some(50.0));
        assert_eq!(nice_interval(312.0, 377.0, 12), Some(5.0));
        assert_eq!(nice_interval(0.0, 26.0, 10), Some(2.5));
        assert_eq!(nice_interval(5.0, 5.0, 10), None);
        let l = Levels {
            base: 0.0,
            interval: 10.0,
            index_every: 5,
        };
        assert_eq!(l.between(3.0, 31.0), vec![10.0, 20.0, 30.0]);
        assert!(l.is_index(50.0) && l.is_index(-100.0) && !l.is_index(20.0));
    }

    #[test]
    fn a_cone_gives_closed_circles() {
        let n = 41;
        let mut v = Vec::new();
        for y in 0..n {
            for x in 0..n {
                let (dx, dy) = (x as f32 - 20.0, y as f32 - 20.0);
                v.push(100.0 - (dx * dx + dy * dy).sqrt());
            }
        }
        let l = Levels {
            base: 0.0,
            interval: 5.0,
            index_every: 2,
        };
        let c = contours(&v, n, n, &l);
        let at90: Vec<_> = c.iter().filter(|c| c.level == 90.0).collect();
        assert_eq!(at90.len(), 1);
        assert!(at90[0].closed);
        for &(x, y) in &at90[0].points {
            let r = (x - 20.5).hypot(y - 20.5);
            assert!((r - 10.0).abs() < 0.1, "{r}");
        }
        assert!(at90[0].index);
        // A hole of no data opens the ring.
        let mut v2 = v.clone();
        v2[20 * n + 30] = f32::NAN;
        let c = contours(&v2, n, n, &l);
        let at90: Vec<_> = c.iter().filter(|c| c.level == 90.0).collect();
        assert_eq!(at90.len(), 1);
        assert!(!at90[0].closed);
    }

    #[test]
    fn reduce_averages_known_cells() {
        let v = [1.0, 3.0, f32::NAN, 5.0, 2.0, 2.0];
        let (r, w, h) = reduce(&v, 3, 2, 2);
        assert_eq!((w, h), (2, 1));
        assert_eq!(r, vec![2.75, 2.0]);
    }
}
