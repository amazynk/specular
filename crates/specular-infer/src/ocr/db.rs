//! Text boxes from a DB (differentiable binarisation) probability map: threshold, 8-connected
//! regions, the minimum-area rectangle of each, its mean probability as a score, and the
//! rectangle grown by `area · ratio / perimeter` on every side ("unclip": the shrunk text
//! kernel the network predicts back to the text's own size).

/// A point (x, y) in pixels.
pub type Pt = [f32; 2];

/// Post-processing settings (the model's `inference.yml` values).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DbParams {
    /// Pixels above this are text.
    pub thresh: f32,
    /// Regions whose mean probability is below this are dropped.
    pub box_thresh: f32,
    pub unclip_ratio: f32,
    /// Regions considered at most.
    pub max_candidates: usize,
    /// Shortest side kept, in map pixels.
    pub min_size: f32,
}

impl Default for DbParams {
    /// PP-OCRv6 small det's values.
    fn default() -> DbParams {
        DbParams {
            thresh: 0.2,
            box_thresh: 0.45,
            unclip_ratio: 1.4,
            max_candidates: 3000,
            min_size: 3.0,
        }
    }
}

/// A text box: four corners clockwise from the top left, in source-image pixels, and its
/// score.
#[derive(Clone, Debug, PartialEq)]
pub struct TextBox {
    pub pts: [Pt; 4],
    pub score: f32,
}

/// Convex hull of integer points (monotone chain), counter-clockwise in y-down terms not
/// relied on; collinear points dropped.
fn hull(mut p: Vec<[i32; 2]>) -> Vec<[f32; 2]> {
    p.sort_unstable();
    p.dedup();
    if p.len() < 3 {
        return p.iter().map(|q| [q[0] as f32, q[1] as f32]).collect();
    }
    let cross = |o: [i32; 2], a: [i32; 2], b: [i32; 2]| -> i64 {
        i64::from(a[0] - o[0]) * i64::from(b[1] - o[1])
            - i64::from(a[1] - o[1]) * i64::from(b[0] - o[0])
    };
    let mut h: Vec<[i32; 2]> = Vec::with_capacity(2 * p.len());
    for &q in &p {
        while h.len() >= 2 && cross(h[h.len() - 2], h[h.len() - 1], q) <= 0 {
            h.pop();
        }
        h.push(q);
    }
    let lower = h.len() + 1;
    for &q in p.iter().rev().skip(1) {
        while h.len() >= lower && cross(h[h.len() - 2], h[h.len() - 1], q) <= 0 {
            h.pop();
        }
        h.push(q);
    }
    h.pop();
    h.iter().map(|q| [q[0] as f32, q[1] as f32]).collect()
}

/// A rotated rectangle: centre, half extents along the unit axis `u` and its normal.
#[derive(Clone, Copy, Debug)]
struct Rect {
    c: Pt,
    u: Pt,
    hw: f32,
    hh: f32,
}

impl Rect {
    fn corners(&self) -> [Pt; 4] {
        let v = [-self.u[1], self.u[0]];
        let p = |a: f32, b: f32| {
            [
                self.c[0] + a * self.hw * self.u[0] + b * self.hh * v[0],
                self.c[1] + a * self.hw * self.u[1] + b * self.hh * v[1],
            ]
        };
        [p(-1.0, -1.0), p(1.0, -1.0), p(1.0, 1.0), p(-1.0, 1.0)]
    }

    fn short_side(&self) -> f32 {
        2.0 * self.hw.min(self.hh)
    }
}

/// The minimum-area rectangle around the hull points (rotating calipers over its edges).
fn min_area_rect(h: &[Pt]) -> Rect {
    if h.len() == 1 {
        return Rect {
            c: h[0],
            u: [1.0, 0.0],
            hw: 0.0,
            hh: 0.0,
        };
    }
    let mut best: Option<(f32, Rect)> = None;
    for i in 0..h.len() {
        let a = h[i];
        let b = h[(i + 1) % h.len()];
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dy * dy).sqrt();
        if len == 0.0 {
            continue;
        }
        let u = [dx / len, dy / len];
        let v = [-u[1], u[0]];
        let (mut u0, mut u1, mut v0, mut v1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for p in h {
            let pu = p[0] * u[0] + p[1] * u[1];
            let pv = p[0] * v[0] + p[1] * v[1];
            u0 = u0.min(pu);
            u1 = u1.max(pu);
            v0 = v0.min(pv);
            v1 = v1.max(pv);
        }
        let area = (u1 - u0) * (v1 - v0);
        if best.as_ref().is_none_or(|(a, _)| area < *a) {
            let (cu, cv) = ((u0 + u1) / 2.0, (v0 + v1) / 2.0);
            best = Some((
                area,
                Rect {
                    c: [cu * u[0] + cv * v[0], cu * u[1] + cv * v[1]],
                    u,
                    hw: (u1 - u0) / 2.0,
                    hh: (v1 - v0) / 2.0,
                },
            ));
        }
    }
    best.map(|(_, r)| r).unwrap_or(Rect {
        c: h[0],
        u: [1.0, 0.0],
        hw: 0.0,
        hh: 0.0,
    })
}

/// Orders four corners as top-left, top-right, bottom-right, bottom-left: the two
/// leftmost (by x) are the left pair, each pair split by y.
pub fn order_corners(p: [Pt; 4]) -> [Pt; 4] {
    let mut s = p;
    s.sort_by(|a, b| a[0].total_cmp(&b[0]));
    let (tl, bl) = if s[1][1] > s[0][1] {
        (s[0], s[1])
    } else {
        (s[1], s[0])
    };
    let (tr, br) = if s[3][1] > s[2][1] {
        (s[2], s[3])
    } else {
        (s[3], s[2])
    };
    [tl, tr, br, bl]
}

/// Mean of `map` over the pixels inside the quad `q` (integer corners, edges included),
/// within its bounding box.
fn quad_score(map: &[f32], w: usize, h: usize, q: &[Pt; 4]) -> f32 {
    let qi: Vec<[f32; 2]> = q.iter().map(|p| [p[0].trunc(), p[1].trunc()]).collect();
    let clampx = |v: f32| (v.max(0.0) as usize).min(w - 1);
    let clampy = |v: f32| (v.max(0.0) as usize).min(h - 1);
    let x0 = clampx(q.iter().map(|p| p[0]).fold(f32::MAX, f32::min).floor());
    let x1 = clampx(q.iter().map(|p| p[0]).fold(f32::MIN, f32::max).ceil());
    let y0 = clampy(q.iter().map(|p| p[1]).fold(f32::MAX, f32::min).floor());
    let y1 = clampy(q.iter().map(|p| p[1]).fold(f32::MIN, f32::max).ceil());
    // Orientation of the quad, for a side test that works either way round.
    let area2: f32 = (0..4)
        .map(|i| {
            let (a, b) = (qi[i], qi[(i + 1) % 4]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum();
    let sign = if area2 >= 0.0 { 1.0 } else { -1.0 };
    let (mut sum, mut n) = (0f64, 0u32);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let (px, py) = (x as f32, y as f32);
            let inside = (0..4).all(|i| {
                let (a, b) = (qi[i], qi[(i + 1) % 4]);
                sign * ((b[0] - a[0]) * (py - a[1]) - (b[1] - a[1]) * (px - a[0])) >= -0.5
            });
            if inside {
                sum += f64::from(map[y * w + x]);
                n += 1;
            }
        }
    }
    if n == 0 {
        0.0
    } else {
        (sum / f64::from(n)) as f32
    }
}

/// Finds text boxes in the probability map `map` (`w × h`, row-major), scaled to a source
/// image of `src_w × src_h`.
pub fn boxes(
    map: &[f32],
    w: usize,
    h: usize,
    src_w: usize,
    src_h: usize,
    p: &DbParams,
) -> Vec<TextBox> {
    if w == 0 || h == 0 || map.len() != w * h {
        return Vec::new();
    }
    let on: Vec<bool> = map.iter().map(|&v| v > p.thresh).collect();
    let mut label = vec![0u32; w * h];
    let mut out = Vec::new();
    let mut next = 0u32;
    let mut stack = Vec::new();
    for start in 0..w * h {
        if !on[start] || label[start] != 0 {
            continue;
        }
        next += 1;
        if next as usize > p.max_candidates {
            break;
        }
        // Flood fill; keep the boundary pixels (a 4-neighbour is off or outside).
        let mut edge = Vec::new();
        label[start] = next;
        stack.push(start);
        while let Some(i) = stack.pop() {
            let (x, y) = ((i % w) as i32, (i / w) as i32);
            let mut boundary = false;
            for dy in -1..=1i32 {
                for dx in -1..=1i32 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        boundary |= dx == 0 || dy == 0;
                        continue;
                    }
                    let j = ny as usize * w + nx as usize;
                    if !on[j] {
                        boundary |= dx == 0 || dy == 0;
                        continue;
                    }
                    if label[j] == 0 {
                        label[j] = next;
                        stack.push(j);
                    }
                }
            }
            if boundary {
                edge.push([x, y]);
            }
        }
        let hp = hull(edge);
        let r = min_area_rect(&hp);
        if r.short_side() < p.min_size {
            continue;
        }
        let quad = order_corners(r.corners());
        let score = quad_score(map, w, h, &quad);
        if score < p.box_thresh {
            continue;
        }
        // Unclip: grow by area · ratio / perimeter.
        let (bw, bh) = (2.0 * r.hw, 2.0 * r.hh);
        let d = bw * bh * p.unclip_ratio / (2.0 * (bw + bh));
        let grown = Rect {
            hw: r.hw + d,
            hh: r.hh + d,
            ..r
        };
        if grown.short_side() < p.min_size + 2.0 {
            continue;
        }
        let sx = src_w as f32 / w as f32;
        let sy = src_h as f32 / h as f32;
        let pts = order_corners(grown.corners()).map(|q| {
            [
                (q[0] * sx).round().clamp(0.0, src_w as f32),
                (q[1] * sy).round().clamp(0.0, src_h as f32),
            ]
        });
        let side = |a: Pt, b: Pt| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
        if side(pts[0], pts[1]) as i32 <= 3 || side(pts[0], pts[3]) as i32 <= 3 {
            continue;
        }
        out.push(TextBox { pts, score });
    }
    sort_reading_order(&mut out);
    out
}

/// Top to bottom, then left to right; boxes whose tops are within 10 px count as one row.
pub fn sort_reading_order(b: &mut [TextBox]) {
    b.sort_by(|x, y| {
        (x.pts[0][1], x.pts[0][0])
            .partial_cmp(&(y.pts[0][1], y.pts[0][0]))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for i in 0..b.len().saturating_sub(1) {
        for j in (0..=i).rev() {
            if (b[j + 1].pts[0][1] - b[j].pts[0][1]).abs() < 10.0
                && b[j + 1].pts[0][0] < b[j].pts[0][0]
            {
                b.swap(j, j + 1);
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_rotated_bar() {
        let (w, h) = (64, 48);
        let mut m = vec![0f32; w * h];
        // A bar from (10, 20) to (50, 28), tilted a little.
        for y in 0..h {
            for x in 0..w {
                let (fx, fy) = (x as f32, y as f32);
                let yc = 24.0 + (fx - 30.0) * 0.1;
                if (10.0..=50.0).contains(&fx) && (fy - yc).abs() <= 3.0 {
                    m[y * w + x] = 0.9;
                }
            }
        }
        let b = boxes(&m, w, h, w * 2, h * 2, &DbParams::default());
        assert_eq!(b.len(), 1);
        let p = b[0].pts;
        assert!(p[0][0] < p[1][0] && p[3][1] > p[0][1], "{p:?}");
        assert!(b[0].score > 0.75, "{}", b[0].score);
        // Scaled ×2 and grown: wider than the bar itself.
        assert!(p[1][0] - p[0][0] > 80.0, "{p:?}");
    }

    #[test]
    fn small_and_faint_regions_are_dropped() {
        let (w, h) = (32, 32);
        let mut m = vec![0f32; w * h];
        m[5 * w + 5] = 0.9;
        for y in 10..20 {
            for x in 4..28 {
                m[y * w + x] = 0.3;
            }
        }
        assert!(boxes(&m, w, h, w, h, &DbParams::default()).is_empty());
    }

    #[test]
    fn reading_order() {
        let tb = |x: f32, y: f32| TextBox {
            pts: [[x, y], [x + 10.0, y], [x + 10.0, y + 5.0], [x, y + 5.0]],
            score: 1.0,
        };
        let mut v = vec![tb(50.0, 3.0), tb(0.0, 30.0), tb(0.0, 0.0)];
        sort_reading_order(&mut v);
        assert_eq!(v[0].pts[0], [0.0, 0.0]);
        assert_eq!(v[1].pts[0], [50.0, 3.0]);
        assert_eq!(v[2].pts[0], [0.0, 30.0]);
    }
}
