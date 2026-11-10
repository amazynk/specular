//! Shapes: fill and line style arrays, then shape records (style changes, moves, straight
//! and curved edges). Each edge has a fill on its left (fill style 0) and its right (fill
//! style 1): the edges of one fill are gathered (left ones reversed) and joined end to end
//! into closed contours; lines are stroked in drawing order. New style arrays start a new
//! layer drawn above the previous ones.

use std::collections::HashMap;

use specular_drawpage::{Affine, BezPath, Cap, Join, Point, Stop, Stroke};

use crate::bits::{Bits, Bytes};

/// A fill style, in twips.
#[derive(Clone, Debug)]
pub(crate) enum Fill {
    Solid([u8; 4]),
    /// Linear (`radial` false) or radial gradient: the gradient square's matrix and stops
    /// (ratio 0–255, colour). Focal gradients are drawn centred.
    Gradient {
        radial: bool,
        matrix: Affine,
        stops: Vec<(u8, [u8; 4])>,
    },
    /// A bitmap by character id, its matrix (bitmap pixels to twips), repeated or clipped.
    Bitmap {
        id: u16,
        matrix: Affine,
        repeat: bool,
    },
}

/// A line style.
#[derive(Clone, Debug)]
pub(crate) struct Line {
    pub width: f64,
    pub fill: Fill,
    pub cap: Cap,
    pub join: Join,
}

/// One drawn part of a shape, in twips.
#[derive(Clone, Debug)]
pub(crate) enum Part {
    Fill(BezPath, Fill),
    Line(BezPath, Line),
}

/// Which shape tag: 1–4 (DefineShape to DefineShape4), 0 for a font glyph, or a morph
/// shape's edges (styles given apart).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Version(pub u8);

impl Version {
    fn rgba(self) -> bool {
        self.0 >= 3
    }
}

fn gradient(r: &mut Bytes, v: Version, focal: bool) -> Option<Vec<(u8, [u8; 4])>> {
    let head = r.u8()?;
    let n = usize::from(head & 15);
    let mut stops = Vec::with_capacity(n);
    for _ in 0..n {
        let ratio = r.u8()?;
        let c = if v.rgba() { r.rgba()? } else { r.rgb()? };
        stops.push((ratio, c));
    }
    if focal {
        r.i16()?;
    }
    Some(stops)
}

pub(crate) fn fill_style(r: &mut Bytes, v: Version) -> Option<Fill> {
    let t = r.u8()?;
    Some(match t {
        0x00 => Fill::Solid(if v.rgba() { r.rgba()? } else { r.rgb()? }),
        0x10 | 0x12 | 0x13 => {
            let matrix = r.matrix()?;
            let stops = gradient(r, v, t == 0x13)?;
            Fill::Gradient {
                radial: t != 0x10,
                matrix,
                stops,
            }
        }
        0x40..=0x43 => {
            let id = r.u16()?;
            let matrix = r.matrix()?;
            Fill::Bitmap {
                id,
                matrix,
                repeat: t == 0x40 || t == 0x42,
            }
        }
        _ => return None,
    })
}

fn styles(r: &mut Bytes, v: Version) -> Option<(Vec<Fill>, Vec<Line>)> {
    let mut n = usize::from(r.u8()?);
    if n == 0xFF && v.0 >= 2 {
        n = usize::from(r.u16()?);
    }
    let mut fills = Vec::with_capacity(n);
    for _ in 0..n {
        fills.push(fill_style(r, v)?);
    }
    let mut n = usize::from(r.u8()?);
    if n == 0xFF && v.0 >= 2 {
        n = usize::from(r.u16()?);
    }
    let mut lines = Vec::with_capacity(n);
    for _ in 0..n {
        let width = f64::from(r.u16()?);
        if v.0 == 4 {
            // Two flag bytes read from their top bits: start cap, join, has-fill, …
            let (f1, _f2) = (r.u8()?, r.u8()?);
            let cap = match f1 >> 6 {
                1 => Cap::Butt,
                2 => Cap::Square,
                _ => Cap::Round,
            };
            let join = match (f1 >> 4) & 3 {
                1 => Join::Bevel,
                2 => Join::Miter,
                _ => Join::Round,
            };
            if (f1 >> 4) & 3 == 2 {
                r.u16()?;
            }
            let fill = if f1 & 0x08 != 0 {
                fill_style(r, v)?
            } else {
                Fill::Solid(r.rgba()?)
            };
            lines.push(Line {
                width,
                fill,
                cap,
                join,
            });
        } else {
            let c = if v.rgba() { r.rgba()? } else { r.rgb()? };
            lines.push(Line {
                width,
                fill: Fill::Solid(c),
                cap: Cap::Round,
                join: Join::Round,
            });
        }
    }
    Some((fills, lines))
}

/// An edge: from, control (curves), to.
#[derive(Clone, Copy, Debug)]
struct Edge {
    from: (i32, i32),
    ctrl: Option<(i32, i32)>,
    to: (i32, i32),
}

impl Edge {
    fn reversed(self) -> Edge {
        Edge {
            from: self.to,
            ctrl: self.ctrl,
            to: self.from,
        }
    }
}

fn pt(p: (i32, i32)) -> Point {
    Point::new(f64::from(p.0), f64::from(p.1))
}

/// Joins a fill's edges into closed contours.
fn contours(edges: &[Edge]) -> BezPath {
    let mut by_start: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (i, e) in edges.iter().enumerate() {
        by_start.entry(e.from).or_default().push(i);
    }
    let mut used = vec![false; edges.len()];
    let mut path = BezPath::new();
    for i in 0..edges.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        let start = edges[i].from;
        path.move_to(pt(start));
        let mut cur = i;
        loop {
            let e = edges[cur];
            match e.ctrl {
                Some(c) => path.quad_to(pt(c), pt(e.to)),
                None => path.line_to(pt(e.to)),
            }
            if e.to == start {
                break;
            }
            let next = by_start
                .get(&e.to)
                .and_then(|v| v.iter().copied().find(|&j| !used[j]));
            match next {
                Some(j) => {
                    used[j] = true;
                    cur = j;
                }
                None => break,
            }
        }
        path.close_path();
    }
    path
}

/// Reads a shape's records (after any style arrays) into parts; `fills` and `lines` are the
/// styles in force at the start.
fn records(
    r: &mut Bytes,
    v: Version,
    mut fills: Vec<Fill>,
    mut lines: Vec<Line>,
    out: &mut Vec<Part>,
) -> Option<()> {
    let mut b: Bits = r.bits();
    let mut fill_bits = b.u(4)?;
    let mut line_bits = b.u(4)?;
    let (mut x, mut y) = (0i32, 0i32);
    let (mut f0, mut f1, mut ln) = (0usize, 0usize, 0usize);
    let mut fill_edges: HashMap<usize, Vec<Edge>> = HashMap::new();
    let mut line_paths: HashMap<usize, BezPath> = HashMap::new();
    let mut line_order: Vec<usize> = Vec::new();
    let mut pen_moved = true;
    let flush = |fe: &mut HashMap<usize, Vec<Edge>>,
                 lp: &mut HashMap<usize, BezPath>,
                 lo: &mut Vec<usize>,
                 fills: &[Fill],
                 lines: &[Line],
                 out: &mut Vec<Part>| {
        let mut keys: Vec<usize> = fe.keys().copied().collect();
        keys.sort_unstable();
        for k in keys {
            if let (Some(edges), Some(f)) = (fe.get(&k), fills.get(k.wrapping_sub(1))) {
                out.push(Part::Fill(contours(edges), f.clone()));
            }
        }
        for k in lo.iter() {
            if let (Some(p), Some(l)) = (lp.remove(k), lines.get(k.wrapping_sub(1))) {
                out.push(Part::Line(p, l.clone()));
            }
        }
        fe.clear();
        lo.clear();
    };
    let mut count = 0usize;
    loop {
        count += 1;
        if count > 1 << 21 {
            return None;
        }
        if b.u(1)? == 0 {
            let flags = b.u(5)?;
            if flags == 0 {
                break;
            }
            if flags & 1 != 0 {
                let n = b.u(5)?;
                x = b.s(n)?;
                y = b.s(n)?;
                pen_moved = true;
            }
            if flags & 2 != 0 {
                f0 = b.u(fill_bits)? as usize;
            }
            if flags & 4 != 0 {
                f1 = b.u(fill_bits)? as usize;
            }
            if flags & 8 != 0 {
                ln = b.u(line_bits)? as usize;
                pen_moved = true;
            }
            if flags & 16 != 0 && v.0 >= 2 {
                flush(
                    &mut fill_edges,
                    &mut line_paths,
                    &mut line_order,
                    &fills,
                    &lines,
                    out,
                );
                // New style arrays, byte-aligned.
                let mut rr = Bytes {
                    d: r.d,
                    at: b.done(),
                };
                (fills, lines) = styles(&mut rr, v)?;
                b = rr.bits();
                fill_bits = b.u(4)?;
                line_bits = b.u(4)?;
                (f0, f1, ln) = (0, 0, 0);
            }
            continue;
        }
        let edge = if b.u(1)? == 1 {
            let n = b.u(4)? + 2;
            let (dx, dy) = if b.u(1)? == 1 {
                (b.s(n)?, b.s(n)?)
            } else if b.u(1)? == 1 {
                (0, b.s(n)?)
            } else {
                (b.s(n)?, 0)
            };
            let e = Edge {
                from: (x, y),
                ctrl: None,
                to: (x + dx, y + dy),
            };
            (x, y) = e.to;
            e
        } else {
            let n = b.u(4)? + 2;
            let (cx, cy) = (x + b.s(n)?, y + b.s(n)?);
            let (ax, ay) = (cx + b.s(n)?, cy + b.s(n)?);
            let e = Edge {
                from: (x, y),
                ctrl: Some((cx, cy)),
                to: (ax, ay),
            };
            (x, y) = e.to;
            e
        };
        if f1 > 0 {
            fill_edges.entry(f1).or_default().push(edge);
        }
        if f0 > 0 {
            fill_edges.entry(f0).or_default().push(edge.reversed());
        }
        if ln > 0 {
            let p = line_paths.entry(ln).or_insert_with(|| {
                line_order.push(ln);
                BezPath::new()
            });
            let last = p.elements().last().and_then(|e| e.end_point());
            if pen_moved || last != Some(pt(edge.from)) {
                p.move_to(pt(edge.from));
            }
            match edge.ctrl {
                Some(c) => p.quad_to(pt(c), pt(edge.to)),
                None => p.line_to(pt(edge.to)),
            }
            pen_moved = false;
        }
    }
    flush(
        &mut fill_edges,
        &mut line_paths,
        &mut line_order,
        &fills,
        &lines,
        out,
    );
    r.at = b.done();
    Some(())
}

/// A DefineShape body after the id and bounds (and DefineShape4's edge bounds and flags).
pub(crate) fn shape_with_style(r: &mut Bytes, v: Version) -> Option<Vec<Part>> {
    let (fills, lines) = styles(r, v)?;
    let mut out = Vec::new();
    records(r, v, fills, lines, &mut out)?;
    Some(out)
}

/// A font glyph or morph edges: records only, with the given styles.
pub(crate) fn shape_records(
    r: &mut Bytes,
    v: Version,
    fills: Vec<Fill>,
    lines: Vec<Line>,
) -> Option<Vec<Part>> {
    let mut out = Vec::new();
    records(r, v, fills, lines, &mut out)?;
    Some(out)
}

/// Gradient stops as a paint's (offsets 0–1, colours through `cx`).
pub(crate) fn stops(s: &[(u8, [u8; 4])], cx: &crate::bits::Cxform) -> Vec<Stop> {
    s.iter()
        .map(|(r, c)| Stop {
            offset: f32::from(*r) / 255.0,
            color: cx.apply(*c),
        })
        .collect()
}

/// A line's stroke in twips (hairlines a twentieth of a pixel at least one pixel wide).
pub(crate) fn stroke(l: &Line) -> Stroke {
    Stroke::new(l.width.max(20.0))
        .with_caps(l.cap)
        .with_join(l.join)
}
