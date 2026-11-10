//! The cell hierarchy: references resolved, cycles cut, top cells, children-first order,
//! exact bounds and flat counts computed per cell once (never by expanding), and a
//! bounded flattening for tests and exports.

use std::collections::HashMap;

use specular_vector::kurbo::{Affine, Point, Rect, Vec2};

use crate::geom::{self, Ends};
use crate::{Cell, Library, Shape, ShapeKind, warn};

/// Resolves reference names, cuts reference cycles and finds the top cells.
pub fn resolve(lib: &mut Library) {
    let mut names: HashMap<&str, u32> = HashMap::with_capacity(lib.cells.len());
    let mut dups: Vec<String> = Vec::new();
    for (i, c) in lib.cells.iter().enumerate() {
        if names.insert(&c.name, i as u32).is_some() {
            dups.push(c.name.clone());
        }
    }
    // The last definition wins, as in other readers.
    let names: HashMap<String, u32> = names.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    for d in dups {
        warn(
            &mut lib.warnings,
            format!("cell {d} is defined more than once; the last definition is used"),
        );
    }
    let mut undefined: Vec<String> = Vec::new();
    for c in &mut lib.cells {
        for r in &mut c.refs {
            r.cell = names.get(&r.name).copied();
            if r.cell.is_none() && !undefined.contains(&r.name) {
                undefined.push(r.name.clone());
            }
        }
    }
    for u in undefined {
        warn(
            &mut lib.warnings,
            format!("references to cell {u}, which is not defined, are not drawn"),
        );
    }
    cut_cycles(lib);
    let mut referenced = vec![false; lib.cells.len()];
    for c in &lib.cells {
        for r in &c.refs {
            if let Some(i) = r.cell {
                referenced[i as usize] = true;
            }
        }
    }
    let dup_of = |i: usize| names.get(&lib.cells[i].name) != Some(&(i as u32));
    lib.top = (0..lib.cells.len())
        .filter(|&i| !referenced[i] && !dup_of(i))
        .map(|i| i as u32)
        .collect();
}

/// Depth-first search; a reference to a cell on the stack closes a cycle and is dropped.
fn cut_cycles(lib: &mut Library) {
    let n = lib.cells.len();
    let mut state = vec![0u8; n];
    let mut cut: Vec<(usize, usize)> = Vec::new();
    for root in 0..n {
        if state[root] != 0 {
            continue;
        }
        state[root] = 1;
        let mut stack: Vec<(usize, usize)> = vec![(root, 0)];
        while let Some(top) = stack.last_mut() {
            let (c, k) = *top;
            let Some(r) = lib.cells[c].refs.get(k) else {
                state[c] = 2;
                stack.pop();
                continue;
            };
            top.1 += 1;
            let Some(child) = r.cell.map(|x| x as usize) else {
                continue;
            };
            match state[child] {
                0 => {
                    state[child] = 1;
                    stack.push((child, 0));
                }
                1 => cut.push((c, k)),
                _ => {}
            }
        }
    }
    for (c, k) in cut {
        let child = lib.cells[c].refs[k].name.clone();
        lib.cells[c].refs[k].cell = None;
        let parent = lib.cells[c].name.clone();
        warn(
            &mut lib.warnings,
            format!(
                "reference cycle: {parent} places {child}, which contains {parent}; that reference is not drawn"
            ),
        );
    }
}

/// Cells children first (every cell after the cells it places).
pub fn order(lib: &Library) -> Vec<u32> {
    let n = lib.cells.len();
    let mut done = vec![false; n];
    let mut out = Vec::with_capacity(n);
    for root in 0..n {
        if done[root] {
            continue;
        }
        done[root] = true;
        let mut stack: Vec<(usize, usize)> = vec![(root, 0)];
        while let Some(top) = stack.last_mut() {
            let (c, k) = *top;
            match lib.cells[c].refs.get(k) {
                Some(r) => {
                    top.1 += 1;
                    if let Some(ch) = r.cell.map(|x| x as usize)
                        && !done[ch]
                    {
                        done[ch] = true;
                        stack.push((ch, 0));
                    }
                }
                None => {
                    out.push(c as u32);
                    stack.pop();
                }
            }
        }
    }
    out
}

fn pt(p: [i32; 2]) -> Point {
    Point::new(f64::from(p[0]), f64::from(p[1]))
}

/// The placement of a reference (absolute flags taken relative to `parent`).
pub fn ref_transform(parent: Affine, r: &crate::Ref) -> Affine {
    geom::absolute(parent, r.origin, &r.strans)
}

/// The offsets of an array's elements.
pub fn array_offsets(a: &crate::Array) -> impl Iterator<Item = Vec2> + '_ {
    (0..a.rows).flat_map(move |j| {
        (0..a.cols).map(move |i| {
            Vec2::new(
                a.col[0] * f64::from(i) + a.row[0] * f64::from(j),
                a.col[1] * f64::from(i) + a.row[1] * f64::from(j),
            )
        })
    })
}

/// The ends of a path at a scale (database units of the drawing per unit of the cell).
pub fn path_ends(pathtype: i16, width: f64, bgn: f64, end: f64) -> Ends {
    match pathtype {
        0 => Ends::Extended(0.0, 0.0),
        1 => Ends::Round,
        2 => Ends::Extended(width / 2.0, width / 2.0),
        _ => Ends::Extended(bgn, end),
    }
}

/// The polygon of a shape under `xf` (paths expanded; an absolute width is not scaled).
pub fn shape_polygon(cell: &Cell, s: &Shape, xf: Affine) -> Vec<Point> {
    let pts = cell.shape_points(s);
    match s.kind {
        ShapeKind::Boundary | ShapeKind::Box => pts.iter().map(|&p| xf * pt(p)).collect(),
        ShapeKind::Path {
            width,
            pathtype,
            bgn,
            end,
        } => {
            let spine: Vec<Point> = pts.iter().map(|&p| xf * pt(p)).collect();
            if width == 0 {
                return zero_width(&spine);
            }
            let scale = if width < 0 { 1.0 } else { geom::scale_of(xf) };
            let w = f64::from(width).abs() * scale;
            let ends = path_ends(pathtype, w, f64::from(bgn) * scale, f64::from(end) * scale);
            geom::path_outline(&spine, w, ends)
        }
    }
}

/// A zero-width path as a polygon of no area: along the spine and back (the way gdstk
/// reads it, and a line when drawn); empty when it has no length.
fn zero_width(spine: &[Point]) -> Vec<Point> {
    let mut pts: Vec<Point> = Vec::with_capacity(spine.len() * 2);
    for &p in spine {
        if pts.last() != Some(&p) {
            pts.push(p);
        }
    }
    if pts.len() < 2 {
        return Vec::new();
    }
    let back: Vec<Point> = pts[1..pts.len() - 1].iter().rev().copied().collect();
    pts.extend(back);
    pts
}

/// The convex hull of every cell's shapes, references included (database units, cell
/// coordinates); empty for a cell with nothing drawn. Each cell is computed once from its
/// children's hulls, so it costs the size of the file, not of the flat layout.
pub fn hulls(lib: &Library) -> Vec<Vec<Point>> {
    let mut out: Vec<Vec<Point>> = vec![Vec::new(); lib.cells.len()];
    for c in order(lib) {
        let cell = &lib.cells[c as usize];
        let mut pts: Vec<Point> = Vec::new();
        for s in &cell.shapes {
            pts.extend(shape_polygon(cell, s, Affine::IDENTITY));
        }
        let mut own = geom::hull(pts);
        for r in &cell.refs {
            let Some(ch) = r.cell else {
                continue;
            };
            let child = &out[ch as usize];
            if child.is_empty() {
                continue;
            }
            let t = ref_transform(Affine::IDENTITY, r);
            let placed: Vec<Point> = child.iter().map(|&p| t * p).collect();
            match &r.array {
                None => own.extend(placed),
                Some(a) => {
                    let last_c = Vec2::new(a.col[0], a.col[1]) * f64::from(a.cols - 1);
                    let last_r = Vec2::new(a.row[0], a.row[1]) * f64::from(a.rows - 1);
                    for off in [Vec2::ZERO, last_c, last_r, last_c + last_r] {
                        own.extend(placed.iter().map(|&p| p + off));
                    }
                }
            }
            if own.len() > 4096 {
                own = geom::hull(own);
            }
        }
        out[c as usize] = geom::hull(own);
    }
    out
}

/// Bounds of a hull.
pub fn hull_bounds(h: &[Point]) -> Option<Rect> {
    specular_vector::bounds_of(h.iter().copied())
}

/// Shapes of every cell with its references expanded (saturating).
pub fn flat_counts(lib: &Library) -> Vec<u64> {
    let mut out = vec![0u64; lib.cells.len()];
    for c in order(lib) {
        let cell = &lib.cells[c as usize];
        let mut n = cell.shapes.len() as u64;
        for r in &cell.refs {
            if let Some(ch) = r.cell {
                n = n.saturating_add(out[ch as usize].saturating_mul(r.count()));
            }
        }
        out[c as usize] = n;
    }
    out
}

/// Shapes per layer of `top` with its references expanded (saturating).
pub fn flat_layer_counts(lib: &Library, top: u32) -> Vec<u64> {
    let nl = lib.layers.len();
    let mut memo: HashMap<u32, Vec<u64>> = HashMap::new();
    for c in order(lib) {
        let cell = &lib.cells[c as usize];
        let mut v = vec![0u64; nl];
        for s in &cell.shapes {
            v[s.layer as usize] += 1;
        }
        for r in &cell.refs {
            if let Some(child) = r.cell.and_then(|ch| memo.get(&ch)) {
                for (a, b) in v.iter_mut().zip(child) {
                    *a = a.saturating_add(b.saturating_mul(r.count()));
                }
            }
        }
        memo.insert(c, v);
        if c == top {
            break;
        }
    }
    memo.remove(&top).unwrap_or_else(|| vec![0; nl])
}

/// A shape of a flattened cell: its layer and polygon (database units).
#[derive(Clone, Debug, PartialEq)]
pub struct FlatShape {
    pub layer: u32,
    pub points: Vec<Point>,
}

/// Expands `top` into its shapes, at most `max` of them; `Err` carries the shapes so far
/// when the cap is hit.
pub fn flatten(lib: &Library, top: u32, max: usize) -> Result<Vec<FlatShape>, Vec<FlatShape>> {
    let mut out = Vec::new();
    // Cells placed, shapes or not: nested arrays of cells that draw nothing multiply
    // without adding shapes.
    let mut visits = max.saturating_mul(2);
    let ok = flatten_into(lib, top, Affine::IDENTITY, max, &mut out, 0, &mut visits);
    if ok { Ok(out) } else { Err(out) }
}

fn flatten_into(
    lib: &Library,
    c: u32,
    xf: Affine,
    max: usize,
    out: &mut Vec<FlatShape>,
    depth: u32,
    visits: &mut usize,
) -> bool {
    let Some(cell) = lib.cells.get(c as usize) else {
        return true;
    };
    let Some(left) = visits.checked_sub(1) else {
        return false;
    };
    *visits = left;
    if depth > 256 {
        return true;
    }
    for s in &cell.shapes {
        if out.len() >= max {
            return false;
        }
        let points = shape_polygon(cell, s, xf);
        // A path of one point (or of one point repeated) has no outline.
        if !points.is_empty() {
            out.push(FlatShape {
                layer: s.layer,
                points,
            });
        }
    }
    for r in &cell.refs {
        let Some(ch) = r.cell else {
            continue;
        };
        let local = ref_transform(xf, r);
        match &r.array {
            None => {
                if !flatten_into(lib, ch, xf * local, max, out, depth + 1, visits) {
                    return false;
                }
            }
            Some(a) => {
                for off in array_offsets(a) {
                    let te = xf * Affine::translate(off) * local;
                    if !flatten_into(lib, ch, te, max, out, depth + 1, visits) {
                        return false;
                    }
                }
            }
        }
    }
    true
}
