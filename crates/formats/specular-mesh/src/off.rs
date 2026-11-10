//! Geomview Object File Format (`.off`), the text form: a keyword (`OFF`, with prefixes
//! `ST` texture coordinates, `C` vertex colours, `N` vertex normals, `4` homogeneous
//! coordinates), the vertex, face and edge counts, the vertices, then each face as its corner
//! count, the corner indices and an optional colour (RGB or RGBA, 0‥1 or 0‥255; a single
//! colour-map index is ignored). `#` starts a comment. Faces are fanned into triangles; a
//! file with face colours gets its own vertices per coloured face.
//!
//! Refused by name: binary OFF and `nOFF` (any dimension).

use crate::builder::Builder;
use crate::text::{f32_of, i64_of, lines, tokens};
use crate::{Limits, Mesh, Result, bad};

/// Geomview's default face colour (when some faces have one and others not).
const DEFAULT_GREY: [u8; 4] = [170, 170, 170, 255];

/// Whether the first line is an OFF keyword.
pub(crate) fn sniff(head: &[u8]) -> bool {
    let Some(first) = lines(head).map(strip).find(|l| !l.is_empty()) else {
        return false;
    };
    tokens(first).next().and_then(keyword).is_some()
}

fn strip(l: &[u8]) -> &[u8] {
    let l = match l.iter().position(|&b| b == b'#') {
        Some(p) => &l[..p],
        None => l,
    };
    crate::text::trim(l)
}

#[derive(Default, Clone, Copy)]
struct Kw {
    st: bool,
    c: bool,
    n: bool,
    four: bool,
}

fn keyword(t: &[u8]) -> Option<Kw> {
    let mut rest = t.strip_suffix(b"OFF")?;
    let mut k = Kw::default();
    if let Some(r) = rest.strip_prefix(b"ST") {
        k.st = true;
        rest = r;
    }
    if let Some(r) = rest.strip_prefix(b"C") {
        k.c = true;
        rest = r;
    }
    if let Some(r) = rest.strip_prefix(b"N") {
        k.n = true;
        rest = r;
    }
    if let Some(r) = rest.strip_prefix(b"4") {
        k.four = true;
        rest = r;
    }
    rest.is_empty().then_some(k)
}

fn colour(vals: &[f32]) -> Option<[u8; 4]> {
    if vals.len() < 3 {
        return None;
    }
    let ints = vals.iter().any(|&v| v > 1.0);
    let c = |v: f32| {
        if ints {
            v.clamp(0.0, 255.0).round() as u8
        } else {
            (v.clamp(0.0, 1.0) * 255.0).round() as u8
        }
    };
    Some([
        c(vals[0]),
        c(vals[1]),
        c(vals[2]),
        vals.get(3).map_or(255, |&a| c(a)),
    ])
}

pub(crate) fn read(bytes: &[u8], limits: &Limits) -> Result<Mesh> {
    let mut ls = lines(bytes).map(strip).filter(|l| !l.is_empty());
    let first = ls.next().ok_or(crate::Error::Empty)?;
    let mut toks = tokens(first);
    let kw_tok = toks.next().unwrap_or_default();
    if kw_tok.ends_with(b"nOFF") {
        return bad("nOFF (files of any dimension) is not supported");
    }
    let Some(kw) = keyword(kw_tok) else {
        return bad("not an OFF file");
    };
    let mut counts: Vec<i64> = toks.filter_map(i64_of).collect();
    if counts.is_empty() && tokens(first).nth(1).is_some_and(|t| t == b"BINARY") {
        return bad("binary OFF is not supported");
    }
    while counts.len() < 3 {
        let Some(l) = ls.next() else {
            return bad("OFF counts missing");
        };
        if l == b"BINARY" || tokens(l).next() == Some(b"BINARY".as_slice()) {
            return bad("binary OFF is not supported");
        }
        counts.extend(tokens(l).filter_map(i64_of));
    }
    let (nv, nf) = (counts[0], counts[1]);
    if nv < 0 || nf < 0 {
        return bad("negative OFF counts");
    }
    let (nv, nf) = (nv as usize, nf as usize);
    if nv > limits.max_vertices {
        return Err(crate::Error::TooLarge(format!(
            "more than {} vertices",
            limits.max_vertices
        )));
    }
    let mut pos: Vec<[f32; 3]> = Vec::with_capacity(nv.min(1 << 20));
    let mut norms: Vec<Option<[f32; 3]>> = Vec::new();
    let mut vcols: Vec<Option<[u8; 4]>> = Vec::new();
    for _ in 0..nv {
        let Some(l) = ls.next() else {
            return bad("OFF file ends inside its vertices");
        };
        let v: Vec<f32> = tokens(l).filter_map(f32_of).collect();
        let dim = if kw.four { 4 } else { 3 };
        if v.len() < dim {
            return bad("OFF vertex with too few coordinates");
        }
        let w = if kw.four && v[3] != 0.0 { v[3] } else { 1.0 };
        pos.push([v[0] / w, v[1] / w, v[2] / w]);
        let mut at = dim;
        norms.push(if kw.n && v.len() >= at + 3 {
            let n = [v[at], v[at + 1], v[at + 2]];
            at += 3;
            Some(n)
        } else {
            None
        });
        vcols.push(if kw.c {
            // RGBA when the line has room for it besides texture coordinates.
            let rest = v.len() - at - if kw.st { 2 } else { 0 };
            colour(&v[at..at + rest.min(4)])
        } else {
            None
        });
    }
    // Faces: (corners, colour).
    let mut faces: Vec<(Vec<u32>, Option<[u8; 4]>)> = Vec::with_capacity(nf.min(1 << 20));
    for _ in 0..nf {
        let Some(l) = ls.next() else {
            break;
        };
        let t: Vec<&[u8]> = tokens(l).collect();
        let Some(n) = t.first().and_then(|t| i64_of(t)) else {
            continue;
        };
        let n = n.max(0) as usize;
        if t.len() < 1 + n {
            continue;
        }
        let idx: Vec<u32> = t[1..1 + n]
            .iter()
            .filter_map(|t| i64_of(t))
            .filter(|&i| i >= 0 && (i as usize) < nv)
            .map(|i| i as u32)
            .collect();
        if idx.len() != n || n < 3 {
            continue;
        }
        let c: Vec<f32> = t[1 + n..].iter().filter_map(|t| f32_of(t)).collect();
        faces.push((idx, colour(&c)));
    }
    let face_colours = faces.iter().any(|(_, c)| c.is_some());
    let vertex_colours = vcols.iter().any(Option::is_some);
    let mut b = Builder::new(limits);
    if face_colours {
        // Each face gets its own corners in its colour.
        for (idx, c) in &faces {
            let c = c.unwrap_or(DEFAULT_GREY);
            let first = b.vertex_count();
            b.reserve_vertices(idx.len())?;
            for &i in idx {
                b.vertex(pos[i as usize], None, Some(c), None)?;
            }
            for k in 1..idx.len() as u32 - 1 {
                b.triangle(first, first + k, first + k + 1)?;
            }
        }
    } else {
        b.reserve_vertices(nv)?;
        for i in 0..nv {
            let c = vertex_colours.then(|| vcols[i].unwrap_or(DEFAULT_GREY));
            b.vertex(pos[i], norms[i], c, None)?;
        }
        for (idx, _) in &faces {
            for k in 1..idx.len() - 1 {
                b.triangle(idx[0], idx[k], idx[k + 1])?;
            }
        }
    }
    if faces.len() < nf {
        b.note(format!("{} of {nf} faces read", faces.len()));
    }
    b.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_with_face_colours() {
        let src = b"OFF # a cube\n8 6 12\n\
            0 0 0\n1 0 0\n1 1 0\n0 1 0\n0 0 1\n1 0 1\n1 1 1\n0 1 1\n\
            4 0 3 2 1 255 0 0\n4 4 5 6 7 0 1 0\n4 0 1 5 4\n4 1 2 6 5\n4 2 3 7 6\n4 3 0 4 7\n";
        assert!(sniff(src));
        let m = read(src, &Limits::default()).unwrap();
        assert_eq!(m.triangle_count(), 12);
        assert_eq!(m.vertex_count(), 24);
    }

    #[test]
    fn coff_shared_vertices() {
        let src = b"COFF\n3 1 0\n0 0 0 1 0 0 1\n1 0 0 0 1 0 1\n0 1 0 0 0 1 1\n3 0 1 2\n";
        let m = read(src, &Limits::default()).unwrap();
        assert_eq!((m.vertex_count(), m.triangle_count()), (3, 1));
        assert!(!sniff(b"ply\nformat ascii 1.0\n"));
        assert!(read(b"OFF BINARY\n", &Limits::default()).is_err());
        assert!(read(b"nOFF\n3\n", &Limits::default()).is_err());
    }
}
