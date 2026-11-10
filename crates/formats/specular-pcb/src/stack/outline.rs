//! The board's shape from its outline layer: the outline's draws joined end to end into
//! closed loops (cut-outs included), or its regions.

use specular_vector::kurbo::{Arc, BezPath, Point, Rect, Shape, Vec2};

use crate::geom;
use crate::gerber::{Gerber, ObjectKind, Segment};

/// Ends closer than this join (mm).
const JOIN: f64 = 0.02;
/// Outlines with more draws than this are not joined (the bounds stand in).
const MAX_SEGMENTS: usize = 50_000;

/// The board: closed loops to fill even-odd, and their bounds.
#[derive(Clone, Debug)]
pub(crate) struct BoardShape {
    pub path: BezPath,
    pub bounds: Rect,
}

fn reversed(s: Segment) -> Segment {
    match s {
        Segment::Line(a, b) => Segment::Line(b, a),
        Segment::Arc {
            center,
            radius,
            start,
            sweep,
        } => Segment::Arc {
            center,
            radius,
            start: start + sweep,
            sweep: -sweep,
        },
    }
}

fn append(p: &mut BezPath, s: Segment) {
    match s {
        Segment::Line(_, b) => p.line_to(b),
        Segment::Arc {
            center,
            radius,
            start,
            sweep,
        } => {
            let arc = Arc::new(center, Vec2::new(radius, radius), start, sweep, 0.0);
            for el in arc.append_iter(geom::FLAT * 0.1) {
                p.push(el);
            }
        }
    }
}

/// Joins `segs` into closed loops.
pub(crate) fn loops(segs: &[Segment]) -> Vec<BezPath> {
    let mut used = vec![false; segs.len()];
    let mut out = Vec::new();
    // Ends on a coarse grid: candidates are found in the 3×3 cells around a point.
    let cell = |p: Point| ((p.x / JOIN).floor() as i64, (p.y / JOIN).floor() as i64);
    let mut grid: std::collections::HashMap<(i64, i64), Vec<usize>> = Default::default();
    for (i, s) in segs.iter().enumerate() {
        for p in [s.start(), s.end()] {
            grid.entry(cell(p)).or_default().push(i);
        }
    }
    let near = |p: Point, used: &[bool]| -> Option<(usize, bool)> {
        let (cx, cy) = cell(p);
        let mut best: Option<(f64, usize, bool)> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for &i in grid.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                    if used[i] {
                        continue;
                    }
                    for (q, rev) in [(segs[i].start(), false), (segs[i].end(), true)] {
                        let d = q.distance(p);
                        if d <= JOIN && best.is_none_or(|b| d < b.0) {
                            best = Some((d, i, rev));
                        }
                    }
                }
            }
        }
        best.map(|(_, i, r)| (i, r))
    };
    for first in 0..segs.len() {
        if used[first] {
            continue;
        }
        used[first] = true;
        let start = segs[first].start();
        let mut p = BezPath::new();
        p.move_to(start);
        append(&mut p, segs[first]);
        let mut end = segs[first].end();
        let mut n = 1;
        let closed = loop {
            if n > 1 && end.distance(start) <= JOIN {
                break true;
            }
            match near(end, &used) {
                Some((i, rev)) => {
                    used[i] = true;
                    let s = if rev { reversed(segs[i]) } else { segs[i] };
                    append(&mut p, s);
                    end = s.end();
                    n += 1;
                }
                None => break end.distance(start) <= JOIN && n > 0,
            }
        };
        // A lone full circle closes on itself.
        let circle = n == 1
            && matches!(segs[first], Segment::Arc { sweep, .. } if sweep.abs() >= std::f64::consts::TAU - 1e-6);
        if closed && (n > 1 || circle) {
            p.close_path();
            if p.area().abs() > 1e-6 {
                out.push(p);
            }
        }
    }
    out
}

/// The board's shape from an outline layer, `None` when nothing closes.
pub(crate) fn board_shape(g: &Gerber) -> Option<BoardShape> {
    let mut path = BezPath::new();
    let segs: Vec<Segment> = g
        .objects
        .iter()
        .filter(|o| o.dark && matches!(o.kind, ObjectKind::Draw | ObjectKind::Hairline))
        .filter_map(|o| o.segment)
        .collect();
    if segs.len() <= MAX_SEGMENTS {
        for l in loops(&segs) {
            for el in l.elements() {
                path.push(*el);
            }
        }
    }
    for o in &g.objects {
        if o.dark && o.kind == ObjectKind::Region {
            for el in o.path.elements() {
                path.push(*el);
            }
        }
    }
    if path.elements().is_empty() {
        return None;
    }
    let bounds = path.bounding_box();
    Some(BoardShape { path, bounds })
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;

    #[test]
    fn joins_a_square_with_a_rounded_corner_and_a_cutout() {
        let p = |x: f64, y: f64| Point::new(x, y);
        let segs = vec![
            Segment::Line(p(0.0, 0.0), p(10.0, 0.0)),
            // Out of order and reversed.
            Segment::Line(p(0.0, 10.0), p(0.0, 0.0)),
            Segment::Line(p(10.0, 0.0), p(10.0, 8.0)),
            Segment::Arc {
                center: p(8.0, 8.0),
                radius: 2.0,
                start: 0.0,
                sweep: PI / 2.0,
            },
            Segment::Line(p(0.0, 10.0), p(8.0, 10.0 + 0.005)),
            // A round cut-out.
            Segment::Arc {
                center: p(5.0, 5.0),
                radius: 1.0,
                start: 0.0,
                sweep: 2.0 * PI,
            },
            // A dangling line.
            Segment::Line(p(20.0, 20.0), p(30.0, 20.0)),
        ];
        let l = loops(&segs);
        assert_eq!(l.len(), 2);
        let a = l[0].area().abs();
        assert!((a - (100.0 - 4.0 + PI)).abs() < 0.05, "{a}");
    }
}
