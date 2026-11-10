//! Vector layers (PSP 6 and later): the layer's vector extension block holds one shape block
//! per shape. A shape block starts with an attributes chunk (name, shape type, flags: bit 0
//! visible) and a definition chunk (stroked, filled, antialiased, stroke width), then a paint
//! style block for the stroke and one for the fill (type 1: an RGB colour), a line style block,
//! and a node chunk: each node is a point with the control points before and after it (cubic
//! Béziers; straight segments repeat the point) and flags (bit 0 of the first byte starts a
//! path; on that start node, bit 1 of the second byte closes the path).
//!
//! Polyline shapes are drawn (filled, then stroked with round joins and caps); text shapes are
//! not (no font engine here) and are named in the notes. Checked against the composite image
//! PaintShop Pro stored (tests/vector_oracle.rs).

use kurbo::{BezPath, Cap, Join, PathEl, Point, Stroke, StrokeOpts};

const SIG: &[u8; 4] = b"~BK\0";
const SHAPE: u16 = 14;
const PAINT: u16 = 15;
/// Shape types.
const TEXT: u16 = 1;
/// Most shapes and nodes read from one layer (bounds hostile files).
const MAX_SHAPES: usize = 100_000;
const MAX_NODES: usize = 1_000_000;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct VShape {
    pub fill: Option<[u8; 3]>,
    pub stroke: Option<([u8; 3], f64)>,
    pub path: Vec<PathEl>,
}

/// The shapes of a vector extension block body, and whether some (text, unknown) are left
/// out.
#[derive(Clone, Debug, Default)]
pub(crate) struct Shapes {
    pub list: Vec<VShape>,
    pub text: bool,
}

fn u16_at(d: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(i..i + 2)?.try_into().ok()?))
}

fn u32_at(d: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(i..i + 4)?.try_into().ok()?))
}

fn f64_at(d: &[u8], i: usize) -> Option<f64> {
    let v = f64::from_le_bytes(d.get(i..i + 8)?.try_into().ok()?);
    v.is_finite().then_some(v)
}

/// A block header at `at` (v4+): id and body range, within `end`.
fn block(d: &[u8], at: usize, end: usize) -> Option<(u16, usize, usize)> {
    if d.get(at..at + 4)? != SIG {
        return None;
    }
    let id = u16_at(d, at + 4)?;
    let len = u32_at(d, at + 6)? as usize;
    let start = at + 10;
    Some((id, start, start.checked_add(len)?.min(end)))
}

/// Reads the shapes of vector extension body `d[start..end]`.
pub(crate) fn read(d: &[u8], start: usize, end: usize) -> Shapes {
    let mut out = Shapes::default();
    let end = end.min(d.len());
    // The extension's own chunk (size, shape count), then the shape blocks.
    let Some(sz) = u32_at(d, start) else {
        return out;
    };
    let mut at = start.saturating_add(sz as usize);
    while at < end && out.list.len() < MAX_SHAPES {
        let Some((id, s, e)) = block(d, at, end) else {
            break;
        };
        if id == SHAPE {
            shape(d, s, e, &mut out);
        }
        at = e;
    }
    out
}

fn shape(d: &[u8], s: usize, e: usize, out: &mut Shapes) {
    let Some(attr) = u32_at(d, s) else { return };
    let Some(nl) = u16_at(d, s + 4) else { return };
    let t = s + 6 + nl as usize;
    let (Some(ty), Some(flags)) = (u16_at(d, t), u32_at(d, t + 2)) else {
        return;
    };
    if flags & 1 == 0 {
        return;
    }
    if ty == TEXT {
        out.text = true;
        return;
    }
    let def = s.saturating_add(attr as usize);
    let Some(dl) = u32_at(d, def) else { return };
    let stroked = d.get(def + 4).copied().unwrap_or(0) != 0;
    let filled = d.get(def + 5).copied().unwrap_or(0) != 0;
    let width = f64_at(d, def + 7).unwrap_or(1.0).clamp(0.0, 10_000.0);
    // Paint blocks: the stroke's, then the fill's; then the line style.
    let mut at = def.saturating_add(dl as usize);
    let mut paints = Vec::new();
    while let Some((id, bs, be)) = block(d, at, e) {
        if id == PAINT {
            paints.push(paint(d, bs, be));
        }
        at = be;
    }
    let colour = |i: usize| paints.get(i).copied().flatten();
    // The node chunk.
    let Some(nsz) = u32_at(d, at) else { return };
    let Some(count) = u32_at(d, at + 4) else {
        return;
    };
    let mut p = at.saturating_add(nsz as usize);
    let mut path = Vec::new();
    let mut prev: Option<(Point, Point)> = None; // the last point and its outgoing control
    // The start of a closed path and its incoming control (open paths: None).
    let mut first: Option<(Point, Point)> = None;
    for _ in 0..(count as usize).min(MAX_NODES) {
        let Some(ns) = u32_at(d, p) else { break };
        let v: Option<Vec<f64>> = (0..6).map(|k| f64_at(d, p + 4 + 8 * k)).collect();
        let Some(v) = v else { break };
        let f0 = d.get(p + 52).copied().unwrap_or(0);
        let f1 = d.get(p + 53).copied().unwrap_or(0);
        let (pt, cin, cout) = (
            Point::new(v[0], v[1]),
            Point::new(v[2], v[3]),
            Point::new(v[4], v[5]),
        );
        if f0 & 1 != 0 || prev.is_none() {
            close(&mut path, prev, first);
            path.push(PathEl::MoveTo(pt));
            // The start node says whether its path is closed.
            first = (f1 & 2 != 0).then_some((pt, cin));
        } else if let Some((q, qc)) = prev {
            path.push(seg(q, qc, cin, pt));
        }
        prev = Some((pt, cout));
        if ns < 52 {
            break;
        }
        p += ns as usize;
    }
    close(&mut path, prev, first);
    out.list.push(VShape {
        fill: if filled { colour(1) } else { None },
        stroke: if stroked && width > 0.0 {
            colour(0).map(|c| (c, width))
        } else {
            None
        },
        path,
    });
}

/// A segment from `a` (outgoing control `ac`) to `b` (incoming control `bc`): a line when the
/// controls sit on the points.
fn seg(a: Point, ac: Point, bc: Point, b: Point) -> PathEl {
    if ac == a && bc == b {
        PathEl::LineTo(b)
    } else {
        PathEl::CurveTo(ac, bc, b)
    }
}

/// Closes the path being built when `first` is set (back to its start, along its controls).
fn close(path: &mut Vec<PathEl>, prev: Option<(Point, Point)>, first: Option<(Point, Point)>) {
    if let (Some((q, qc)), Some((f, fc))) = (prev, first) {
        if q != f {
            path.push(seg(q, qc, fc, f));
        }
        path.push(PathEl::ClosePath);
    }
}

/// A paint style block: type 1 is a colour (R, G, B, then a byte and a palette index).
fn paint(d: &[u8], s: usize, e: usize) -> Option<[u8; 3]> {
    let sz = u32_at(d, s)? as usize;
    let ty = u16_at(d, s + 4)?;
    if ty & 1 == 0 {
        return None;
    }
    let c = s.checked_add(sz)?;
    if c + 7 > e {
        return None;
    }
    Some([d[c + 4], d[c + 5], d[c + 6]])
}

/// Coverage of a path over a `w × h` grid (non-zero), by signed area accumulation.
fn coverage(path: &BezPath, w: usize, h: usize) -> Vec<f32> {
    let mut acc = vec![0f32; (w + 2) * h];
    let stride = w + 2;
    let mut line = |p0: Point, p1: Point| {
        let (x0, y0, x1, y1) = (p0.x, p0.y, p1.x, p1.y);
        if (y0 - y1).abs() < 1e-12 {
            return;
        }
        let dir = if y1 > y0 { 1.0 } else { -1.0 };
        let (xa, ya, xb, yb) = if y0 < y1 {
            (x0, y0, x1, y1)
        } else {
            (x1, y1, x0, y0)
        };
        let dxdy = (xb - xa) / (yb - ya);
        let ys = ya.max(0.0);
        let ye = yb.min(h as f64);
        if ys >= ye {
            return;
        }
        let mut y = ys;
        while y < ye {
            let row = y.floor() as usize;
            let yn = ((row + 1) as f64).min(ye);
            let dy = yn - y;
            let xs = xa + (y - ya) * dxdy;
            let xe = xa + (yn - ya) * dxdy;
            // Split the slice where it crosses pixel columns; each piece adds its height to
            // the cell it is in (the part right of it) and the next (the rest), so the
            // running sum along the row gives the exact area.
            let r = &mut acc[row * stride..(row + 1) * stride];
            let (lo, hi) = (xs.min(xe), xs.max(xe));
            let mut cuts = vec![0.0f64, 1.0];
            if hi - lo > 1e-12 {
                let mut c = lo.floor() + 1.0;
                while c < hi && cuts.len() < w + 4 {
                    cuts.push((c - xs) / (xe - xs));
                    c += 1.0;
                }
                cuts.sort_by(f64::total_cmp);
            }
            for k in cuts.windows(2) {
                let (t0, t1) = (k[0], k[1]);
                let pdy = dy * (t1 - t0);
                if pdy <= 0.0 {
                    continue;
                }
                let xm = (xs + (xe - xs) * (t0 + t1) / 2.0).clamp(-1.0, w as f64);
                let cx = xm.floor();
                let frac = (xm - cx) as f32;
                let ci = (cx + 1.0) as usize;
                let a = (pdy * dir) as f32;
                if ci < stride {
                    r[ci] += a * (1.0 - frac);
                }
                if ci + 1 < stride {
                    r[ci + 1] += a * frac;
                }
            }
            y = yn;
        }
    };
    let mut start = Point::ZERO;
    let mut last = Point::ZERO;
    kurbo::flatten(path.iter(), 0.1, |el| match el {
        PathEl::MoveTo(p) => {
            line(last, start);
            start = p;
            last = p;
        }
        PathEl::LineTo(p) => {
            line(last, p);
            last = p;
        }
        PathEl::ClosePath => {
            line(last, start);
            last = start;
        }
        _ => {}
    });
    line(last, start);
    let mut out = vec![0f32; w * h];
    for y in 0..h {
        let mut s = 0f32;
        for x in 0..w + 1 {
            s += acc[y * stride + x];
            if x >= 1 {
                out[y * w + x - 1] = s.abs().min(1.0);
            }
        }
    }
    out
}

/// Draws `shapes` (bottom first) onto a transparent `w × h` canvas of straight RGBA16.
pub(crate) fn raster(shapes: &[VShape], w: usize, h: usize) -> Vec<u16> {
    let mut px = vec![0u16; w * h * 4];
    let mut over = |cov: &[f32], c: [u8; 3]| {
        for (p, &a) in px.chunks_exact_mut(4).zip(cov) {
            if a <= 0.0 {
                continue;
            }
            let ba = f32::from(p[3]) / 65535.0;
            let oa = a + ba * (1.0 - a);
            for k in 0..3 {
                let s = f32::from(c[k]) * 257.0;
                let b = f32::from(p[k]);
                p[k] = ((s * a + b * ba * (1.0 - a)) / oa).round() as u16;
            }
            p[3] = (oa * 65535.0).round() as u16;
        }
    };
    for s in shapes {
        let path = BezPath::from_vec(s.path.clone());
        if let Some(c) = s.fill {
            over(&coverage(&path, w, h), c);
        }
        if let Some((c, width)) = s.stroke {
            let st = Stroke::new(width)
                .with_join(Join::Round)
                .with_caps(Cap::Round);
            let outline = kurbo::stroke(path.iter(), &st, &StrokeOpts::default(), 0.05);
            over(&coverage(&outline, w, h), c);
        }
    }
    px
}

/// The bounding box of all shapes, for tests.
#[cfg(test)]
fn bounds(shapes: &[VShape]) -> Option<kurbo::Rect> {
    use kurbo::Shape as _;
    shapes
        .iter()
        .map(|s| BezPath::from_vec(s.path.clone()).bounding_box())
        .reduce(|a, b| a.union(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_square_covers_its_area() {
        let mut p = BezPath::new();
        p.move_to((2.0, 2.0));
        p.line_to((6.0, 2.0));
        p.line_to((6.0, 6.0));
        p.line_to((2.0, 6.0));
        p.close_path();
        let c = coverage(&p, 8, 8);
        assert_eq!(c[3 * 8 + 3], 1.0);
        assert_eq!(c[8 + 1], 0.0);
        assert_eq!(c[3 * 8 + 6], 0.0);
        let total: f32 = c.iter().sum();
        assert!((total - 16.0).abs() < 1e-3, "{total}");
    }

    fn node(v: &mut Vec<u8>, p: (f64, f64), f0: u8, f1: u8) {
        v.extend_from_slice(&55u32.to_le_bytes());
        for _ in 0..3 {
            v.extend_from_slice(&p.0.to_le_bytes());
            v.extend_from_slice(&p.1.to_le_bytes());
        }
        v.extend_from_slice(&[f0, f1, 0]);
    }

    fn blk(v: &mut Vec<u8>, id: u16, body: &[u8]) {
        v.extend_from_slice(SIG);
        v.extend_from_slice(&id.to_le_bytes());
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
    }

    /// A vector extension body with one filled triangle and one text shape.
    pub(crate) fn sample() -> Vec<u8> {
        let mut shape = Vec::new();
        let name = b"tri";
        shape.extend_from_slice(&(4 + 2 + 3 + 2 + 4 + 8u32).to_le_bytes());
        shape.extend_from_slice(&3u16.to_le_bytes());
        shape.extend_from_slice(name);
        shape.extend_from_slice(&2u16.to_le_bytes());
        shape.extend_from_slice(&5u32.to_le_bytes());
        shape.extend_from_slice(&[0; 8]);
        let mut def = 60u32.to_le_bytes().to_vec();
        def.extend_from_slice(&[0, 1, 1]);
        def.extend_from_slice(&1.0f64.to_le_bytes());
        def.resize(60, 0);
        shape.extend(def);
        blk(&mut shape, PAINT, &[6, 0, 0, 0, 0, 0]);
        let mut fill = vec![6, 0, 0, 0, 1, 0];
        fill.extend_from_slice(&12u32.to_le_bytes());
        fill.extend_from_slice(&[255, 0, 0, 0, 255, 255, 255, 255]);
        blk(&mut shape, PAINT, &fill);
        blk(&mut shape, 19, &[0; 45]);
        shape.extend_from_slice(&8u32.to_le_bytes());
        shape.extend_from_slice(&3u32.to_le_bytes());
        node(&mut shape, (1.0, 1.0), 1, 0x43);
        node(&mut shape, (9.0, 1.0), 0, 0x43);
        node(&mut shape, (1.0, 9.0), 0, 0x43);
        let mut text = Vec::new();
        text.extend_from_slice(&(4 + 2 + 2 + 4u32).to_le_bytes());
        text.extend_from_slice(&0u16.to_le_bytes());
        text.extend_from_slice(&TEXT.to_le_bytes());
        text.extend_from_slice(&1u32.to_le_bytes());
        let mut v = 8u32.to_le_bytes().to_vec();
        v.extend_from_slice(&2u32.to_le_bytes());
        blk(&mut v, SHAPE, &shape);
        blk(&mut v, SHAPE, &text);
        v
    }

    #[test]
    fn shapes_are_read_and_drawn() {
        let v = sample();
        let s = read(&v, 0, v.len());
        assert!(s.text);
        assert_eq!(s.list.len(), 1);
        assert_eq!(s.list[0].fill, Some([255, 0, 0]));
        assert_eq!(s.list[0].stroke, None);
        let b = bounds(&s.list).unwrap();
        assert_eq!((b.x0, b.y0, b.x1, b.y1), (1.0, 1.0, 9.0, 9.0));
        let px = raster(&s.list, 10, 10);
        let at = |x: usize, y: usize| &px[(y * 10 + x) * 4..(y * 10 + x) * 4 + 4];
        assert_eq!(at(2, 2), [65535, 0, 0, 65535]);
        assert_eq!(at(8, 8)[3], 0);
        // Cut short anywhere: no panic.
        for n in 0..v.len() {
            let _ = read(&v[..n], 0, n);
        }
    }
}
