//! Hatch patterns: families of parallel dashed lines clipped to the boundary by even-odd
//! scanline crossing (the boundary flattened to line segments).

use kurbo::{BezPath, PathEl, Point, Rect, Vec2};

/// One pattern line family as the HATCH stores it (already scaled and rotated).
pub(crate) struct PatternLine {
    pub angle: f64,
    pub base: Point,
    pub offset: Vec2,
    pub dashes: Vec<f64>,
}

/// Flattens closed boundary paths into segments.
pub(crate) fn segments(paths: &[BezPath], tolerance: f64) -> Vec<(Point, Point)> {
    let mut segs = Vec::new();
    for p in paths {
        let mut start = None;
        let mut last = None;
        kurbo::flatten(p.iter(), tolerance, |el| match el {
            PathEl::MoveTo(q) => {
                if let (Some(s), Some(l)) = (start, last)
                    && s != l
                {
                    segs.push((l, s));
                }
                start = Some(q);
                last = Some(q);
            }
            PathEl::LineTo(q) => {
                if let Some(l) = last {
                    segs.push((l, q));
                }
                last = Some(q);
            }
            PathEl::ClosePath => {
                if let (Some(s), Some(l)) = (start, last)
                    && s != l
                {
                    segs.push((l, s));
                }
                last = start;
            }
            _ => {}
        });
        if let (Some(s), Some(l)) = (start, last)
            && s != l
        {
            segs.push((l, s));
        }
    }
    segs
}

/// Hatch segments one file may generate in all: each hatch is capped, but thousands of
/// tiny hatch entities would otherwise each take that cap.
pub(crate) const MAX_TOTAL: usize = 8_000_000;

/// The pattern's dashes inside the boundary, as one path of line segments; at most
/// `cap` segments (returns whether it was cut short).
pub(crate) fn pattern(
    segs: &[(Point, Point)],
    bounds: Rect,
    lines: &[PatternLine],
    cap: usize,
) -> (BezPath, bool) {
    let mut out = BezPath::new();
    let mut count = 0usize;
    let corners = [
        Point::new(bounds.x0, bounds.y0),
        Point::new(bounds.x1, bounds.y0),
        Point::new(bounds.x0, bounds.y1),
        Point::new(bounds.x1, bounds.y1),
    ];
    let mut ts: Vec<f64> = Vec::new();
    for line in lines {
        let d = Vec2::new(line.angle.cos(), line.angle.sin());
        let n = Vec2::new(-d.y, d.x);
        let step = n.dot(line.offset);
        if !step.is_finite() || step.abs() < 1e-9 * bounds.width().max(bounds.height()).max(1e-9) {
            continue;
        }
        let nb = n.dot(line.base.to_vec2());
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for c in corners {
            let v = (n.dot(c.to_vec2()) - nb) / step;
            lo = lo.min(v);
            hi = hi.max(v);
        }
        let (k0, k1) = (lo.floor(), hi.ceil());
        // Bounded work: lines × boundary segments.
        if !(k0.is_finite() && k1.is_finite())
            || k1 - k0 > cap as f64
            || (k1 - k0) * segs.len() as f64 > 60e6
        {
            return (out, true);
        }
        let pattern_len: f64 = line.dashes.iter().map(|x| x.abs()).sum();
        let mut k = k0;
        while k <= k1 {
            let origin = line.base + line.offset * k;
            let off = n.dot(origin.to_vec2());
            // Crossings of the line {p : n·p = off} with every segment, as t along d.
            ts.clear();
            for &(a, b) in segs {
                let sa = n.dot(a.to_vec2()) - off;
                let sb = n.dot(b.to_vec2()) - off;
                if (sa < 0.0) != (sb < 0.0) {
                    let f = sa / (sa - sb);
                    let p = a.lerp(b, f);
                    ts.push(d.dot(p - origin));
                }
            }
            ts.sort_by(|a, b| a.total_cmp(b));
            for pair in ts.chunks_exact(2) {
                let (t0, t1) = (pair[0], pair[1]);
                let reps = (t1 - t0) / pattern_len.max(1e-300) + 1.0;
                if line.dashes.is_empty()
                    || pattern_len <= 1e-12
                    || reps * line.dashes.len() as f64 > (cap - count.min(cap)) as f64 * 4.0
                {
                    // No pattern, or far too many dashes to walk: a solid line.
                    out.move_to(origin + d * t0);
                    out.line_to(origin + d * t1);
                    count += 1;
                } else {
                    // Walk the dash pattern from the repetition before t0.
                    let mut t = (t0 / pattern_len).floor() * pattern_len;
                    'walk: loop {
                        for &dash in &line.dashes {
                            let len = dash.abs();
                            if t >= t1 {
                                break 'walk;
                            }
                            if dash >= 0.0 {
                                let a = t.max(t0);
                                let b = (t + len).min(t1);
                                if b > a || (dash == 0.0 && t >= t0) {
                                    out.move_to(origin + d * a);
                                    out.line_to(origin + d * b.max(a));
                                    count += 1;
                                }
                            }
                            t += len;
                            if count >= cap {
                                return (out, true);
                            }
                        }
                    }
                }
                if count >= cap {
                    return (out, true);
                }
            }
            k += 1.0;
        }
    }
    (out, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_lines_in_square() {
        let mut sq = BezPath::new();
        sq.move_to((0.0, 0.0));
        sq.line_to((10.0, 0.0));
        sq.line_to((10.0, 10.0));
        sq.line_to((0.0, 10.0));
        sq.close_path();
        let segs = segments(&[sq], 0.01);
        assert_eq!(segs.len(), 4);
        let lines = [PatternLine {
            angle: 0.0,
            base: Point::new(0.0, 0.5),
            offset: Vec2::new(0.0, 1.0),
            dashes: vec![],
        }];
        let (p, cut) = pattern(&segs, Rect::new(0.0, 0.0, 10.0, 10.0), &lines, 1000);
        assert!(!cut);
        assert_eq!(p.elements().len(), 20);
    }
}
