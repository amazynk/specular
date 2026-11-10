//! Which pixels a shape covers, as runs per row (the rules are in the crate docs).
//!
//! Each run is computed analytically from the row's centre line, then its ends are checked
//! against [`Shape::contains`], so the runs are exactly the pixels the point test accepts.

use crate::geom::{P, Shape, crossings, ellipse_contains};

/// Calls `run(y, x0, x1)` for each run of covered pixels `x0..x1` of row `y` inside `clip`
/// (`(x0, y0, x1, y1)`, exclusive ends), rows in increasing order and runs within a row
/// left to right without overlap. Lines, polylines and angles cover nothing.
pub fn spans(shape: &Shape, clip: (u32, u32, u32, u32), mut run: impl FnMut(u32, u32, u32)) {
    let (cx0, cy0, cx1, cy1) = clip;
    if cx0 >= cx1 || cy0 >= cy1 || !shape.is_valid() {
        return;
    }
    // Pixel indices clipped (as f64 first, so far-off shapes do not wrap).
    let clamp_x = |v: f64| v.clamp(f64::from(cx0), f64::from(cx1)) as u32;
    let clamp_y = |v: f64| v.clamp(f64::from(cy0), f64::from(cy1)) as u32;
    match *shape {
        Shape::Point(p) => {
            let (x, y) = (p.x.floor(), p.y.floor());
            if x >= f64::from(cx0)
                && x < f64::from(cx1)
                && y >= f64::from(cy0)
                && y < f64::from(cy1)
            {
                run(y as u32, x as u32, x as u32 + 1);
            }
        }
        Shape::Rect { .. } => {
            let (x0, y0, x1, y1) = shape.bounds();
            // Centre c = i + 0.5 with x0 <= c < x1  ⇔  ceil(x0 - 0.5) <= i < ceil(x1 - 0.5).
            let (a, b) = (clamp_x((x0 - 0.5).ceil()), clamp_x((x1 - 0.5).ceil()));
            let (ya, yb) = (clamp_y((y0 - 0.5).ceil()), clamp_y((y1 - 0.5).ceil()));
            if a < b {
                for y in ya..yb {
                    run(y, a, b);
                }
            }
        }
        Shape::Ellipse { cx, cy, rx, ry } => {
            let (rx, ry) = (rx.abs(), ry.abs());
            if rx == 0.0 || ry == 0.0 {
                return;
            }
            let ya = clamp_y((cy - ry - 0.5).floor());
            let yb = clamp_y((cy + ry + 0.5).ceil() + 1.0);
            let inside = |x: i64, yc: f64| ellipse_contains(cx, cy, rx, ry, x as f64 + 0.5, yc);
            for y in ya..yb {
                let yc = f64::from(y) + 0.5;
                let t = (yc - cy) / ry;
                if t * t > 1.0 {
                    continue;
                }
                let half = rx * (1.0 - t * t).sqrt();
                let mut lo = (cx - half - 0.5).ceil() as i64;
                let mut hi = (cx + half - 0.5).floor() as i64;
                // Exact ends by the point test (rounding moves them by at most one).
                while lo <= hi && !inside(lo, yc) {
                    lo += 1;
                }
                while inside(lo - 1, yc) {
                    lo -= 1;
                }
                while hi >= lo && !inside(hi, yc) {
                    hi -= 1;
                }
                while hi >= lo && inside(hi + 1, yc) {
                    hi += 1;
                }
                if lo > hi {
                    continue;
                }
                let (a, b) = (clamp_x(lo as f64), clamp_x(hi as f64 + 1.0));
                if a < b {
                    run(y, a, b);
                }
            }
        }
        Shape::Polygon(ref v) | Shape::Freehand(ref v) => polygon(v, clamp_x, clamp_y, run),
        Shape::Line(..) | Shape::Polyline(_) | Shape::Angle { .. } => {}
    }
}

fn polygon(
    v: &[P],
    clamp_x: impl Fn(f64) -> u32,
    clamp_y: impl Fn(f64) -> u32,
    mut run: impl FnMut(u32, u32, u32),
) {
    let (min_y, max_y) = v
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| {
            (a.min(p.y), b.max(p.y))
        });
    let ya = clamp_y((min_y - 0.5).floor());
    let yb = clamp_y((max_y + 0.5).ceil() + 1.0);
    let mut xs: Vec<f64> = Vec::new();
    for y in ya..yb {
        let yc = f64::from(y) + 0.5;
        xs.clear();
        xs.extend(crossings(v, yc));
        if xs.len() < 2 {
            continue;
        }
        xs.sort_unstable_by(f64::total_cmp);
        for pair in xs.chunks_exact(2) {
            // Centres with pair[0] <= c < pair[1].
            let (a, b) = (
                clamp_x((pair[0] - 0.5).ceil()),
                clamp_x((pair[1] - 0.5).ceil()),
            );
            if a < b {
                run(y, a, b);
            }
        }
    }
}

/// The number of pixels covered inside `clip`.
pub fn count(shape: &Shape, clip: (u32, u32, u32, u32)) -> u64 {
    let mut n = 0u64;
    spans(shape, clip, |_, a, b| n += u64::from(b - a));
    n
}
