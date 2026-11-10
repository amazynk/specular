//! Cells into triangles: polygons fanned, strips split, 3D cells reduced to the faces no
//! other cell shares.

use std::collections::HashMap;

use crate::{Error, Result, bad};

/// Cells as VTK stores them: `offsets` has one entry per cell plus a leading 0, each cell's
/// point ids are `conn[offsets[i]..offsets[i + 1]]`.
#[derive(Clone, Debug, Default)]
pub(crate) struct CellArray {
    pub offsets: Vec<i64>,
    pub conn: Vec<i64>,
}

impl CellArray {
    pub fn len(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    pub fn cell(&self, i: usize) -> Option<&[i64]> {
        let a = usize::try_from(*self.offsets.get(i)?).ok()?;
        let b = usize::try_from(*self.offsets.get(i + 1)?).ok()?;
        self.conn.get(a..b)
    }

    /// From the classic layout: a count, then the ids, cell after cell.
    pub fn from_counted(stream: Vec<i64>, cells: usize) -> Result<CellArray> {
        if cells > stream.len() {
            return Err(Error::Truncated("cells".into()));
        }
        let mut offsets = Vec::new();
        crate::num::reserve(&mut offsets, cells as u64 + 1)?;
        let mut conn = Vec::new();
        crate::num::reserve(&mut conn, stream.len() as u64)?;
        offsets.push(0);
        let mut i = 0usize;
        for _ in 0..cells {
            let n = *stream
                .get(i)
                .ok_or_else(|| Error::Truncated("cells".into()))?;
            let n = usize::try_from(n).map_err(|_| Error::Malformed("cell size".into()))?;
            let ids = stream
                .get(i + 1..i + 1 + n)
                .ok_or_else(|| Error::Truncated("cells".into()))?;
            conn.extend_from_slice(ids);
            offsets.push(conn.len() as i64);
            i += 1 + n;
        }
        Ok(CellArray { offsets, conn })
    }

    /// From XML's end offsets (no leading 0).
    pub fn from_ends(ends: Vec<i64>, conn: Vec<i64>) -> Result<CellArray> {
        let mut offsets = Vec::new();
        crate::num::reserve(&mut offsets, ends.len() as u64 + 1)?;
        offsets.push(0);
        offsets.extend(ends);
        CellArray::checked(offsets, conn)
    }

    /// From offsets with the leading 0 (legacy 5.1), checked to grow within `conn`.
    pub fn checked(offsets: Vec<i64>, conn: Vec<i64>) -> Result<CellArray> {
        if offsets.first().is_some_and(|&o| o != 0)
            || offsets.windows(2).any(|w| w[1] < w[0])
            || offsets
                .last()
                .is_some_and(|&o| o as u64 > conn.len() as u64 || o < 0)
        {
            return bad("cell offsets");
        }
        Ok(CellArray { offsets, conn })
    }
}

/// Faces of polyhedron cells.
#[derive(Clone, Debug, Default)]
pub(crate) enum Polyhedra {
    /// No polyhedra, or their faces are streamed in the cell connectivity itself (legacy:
    /// a face count, then per face its size and ids).
    #[default]
    InCells,
    /// XML before VTK 9.4: per cell a stream in `faces` ending at `ends[cell]` (−1 for cells
    /// that are not polyhedra).
    Stream { faces: Vec<i64>, ends: Vec<i64> },
    /// XML from VTK 9.4: faces as cells of their own, and per cell the faces it has.
    Lists {
        faces: CellArray,
        cell_faces: CellArray,
    },
}

/// What the triangles came from.
pub(crate) enum Cells<'a> {
    Poly {
        verts: &'a CellArray,
        lines: &'a CellArray,
        polys: &'a CellArray,
        strips: &'a CellArray,
    },
    Unstructured {
        cells: &'a CellArray,
        types: &'a [i64],
        polyhedra: &'a Polyhedra,
    },
}

#[derive(Default)]
pub(crate) struct Triangles {
    pub triangles: Vec<[u32; 3]>,
    pub triangle_cell: Vec<u32>,
    pub cells: u64,
    pub vertices: u64,
    pub lines: u64,
    pub notes: Vec<String>,
}

const TETRA: &[&[usize]] = &[&[0, 1, 3], &[1, 2, 3], &[2, 0, 3], &[0, 2, 1]];
const HEXA: &[&[usize]] = &[
    &[0, 4, 7, 3],
    &[1, 2, 6, 5],
    &[0, 1, 5, 4],
    &[3, 7, 6, 2],
    &[0, 3, 2, 1],
    &[4, 5, 6, 7],
];
const VOXEL: &[&[usize]] = &[
    &[0, 4, 6, 2],
    &[1, 3, 7, 5],
    &[0, 1, 5, 4],
    &[2, 6, 7, 3],
    &[0, 2, 3, 1],
    &[4, 5, 7, 6],
];
const WEDGE: &[&[usize]] = &[
    &[0, 1, 2],
    &[3, 5, 4],
    &[0, 3, 4, 1],
    &[1, 4, 5, 2],
    &[2, 5, 3, 0],
];
const PYRAMID: &[&[usize]] = &[
    &[0, 3, 2, 1],
    &[0, 1, 4],
    &[1, 2, 4],
    &[2, 3, 4],
    &[3, 0, 4],
];
const PENTA_PRISM: &[&[usize]] = &[
    &[0, 4, 3, 2, 1],
    &[5, 6, 7, 8, 9],
    &[0, 1, 6, 5],
    &[1, 2, 7, 6],
    &[2, 3, 8, 7],
    &[3, 4, 9, 8],
    &[4, 0, 5, 9],
];
const HEXA_PRISM: &[&[usize]] = &[
    &[0, 5, 4, 3, 2, 1],
    &[6, 7, 8, 9, 10, 11],
    &[0, 1, 7, 6],
    &[1, 2, 8, 7],
    &[2, 3, 9, 8],
    &[3, 4, 10, 9],
    &[4, 5, 11, 10],
    &[5, 0, 6, 11],
];

enum Shape {
    Skip,
    Vertex,
    Line,
    /// A polygon of the first `n` points (0: all of them).
    Polygon(usize),
    /// A quadratic polygon: the first half of its points.
    HalfPolygon,
    Pixel,
    Strip,
    Solid(&'static [&'static [usize]], usize),
    Polyhedron,
    Unknown,
}

fn shape(t: i64) -> Shape {
    match t {
        0 => Shape::Skip,
        1 | 2 => Shape::Vertex,
        3 | 4 | 21 | 35 | 68 | 75 => Shape::Line,
        5 | 22 | 34 | 69 | 76 => Shape::Polygon(3),
        6 => Shape::Strip,
        7 => Shape::Polygon(0),
        8 => Shape::Pixel,
        9 | 23 | 28 | 30 | 70 | 77 => Shape::Polygon(4),
        36 => Shape::HalfPolygon,
        10 | 24 | 71 | 78 => Shape::Solid(TETRA, 4),
        11 => Shape::Solid(VOXEL, 8),
        12 | 25 | 29 | 33 | 72 | 79 => Shape::Solid(HEXA, 8),
        13 | 26 | 31 | 32 | 73 | 80 => Shape::Solid(WEDGE, 6),
        14 | 27 | 37 | 74 | 81 => Shape::Solid(PYRAMID, 5),
        15 => Shape::Solid(PENTA_PRISM, 10),
        16 => Shape::Solid(HEXA_PRISM, 12),
        42 => Shape::Polyhedron,
        _ => Shape::Unknown,
    }
}

/// A face's key: its sorted point ids (short faces inline).
#[derive(Clone, PartialEq, Eq, Hash)]
enum Key {
    Short([u32; 4]),
    Long(Box<[u32]>),
}

fn key(ids: &[u32]) -> Key {
    if ids.len() <= 4 {
        let mut k = [u32::MAX; 4];
        k[..ids.len()].copy_from_slice(ids);
        k.sort_unstable();
        Key::Short(k)
    } else {
        let mut v = ids.to_vec();
        v.sort_unstable();
        Key::Long(v.into_boxed_slice())
    }
}

struct Builder {
    out: Triangles,
    points: usize,
    invalid: u64,
    unknown: u64,
    /// Boundary candidates: key → (uses, first face's cell, its ids' start in `face_ids`, len).
    faces: HashMap<Key, (u32, u32, usize, usize)>,
    face_order: Vec<Key>,
    face_ids: Vec<u32>,
}

impl Builder {
    fn ids(&self, cell: &[i64]) -> Option<Vec<u32>> {
        cell.iter()
            .map(|&i| {
                u32::try_from(i)
                    .ok()
                    .filter(|&i| (i as usize) < self.points)
            })
            .collect()
    }

    fn fan(&mut self, ids: &[u32], cell: u32) -> Result<()> {
        if ids.len() < 3 {
            return Ok(());
        }
        if self.out.triangles.len() as u64 + ids.len() as u64 > crate::limits::MAX_VALUES {
            return Err(Error::TooLarge);
        }
        for i in 1..ids.len() - 1 {
            self.out.triangles.push([ids[0], ids[i], ids[i + 1]]);
            self.out.triangle_cell.push(cell);
        }
        Ok(())
    }

    fn strip(&mut self, ids: &[u32], cell: u32) {
        for i in 0..ids.len().saturating_sub(2) {
            let t = if i % 2 == 0 {
                [ids[i], ids[i + 1], ids[i + 2]]
            } else {
                [ids[i + 1], ids[i], ids[i + 2]]
            };
            self.out.triangles.push(t);
            self.out.triangle_cell.push(cell);
        }
    }

    fn face(&mut self, ids: &[u32], cell: u32) {
        if ids.len() < 3 {
            return;
        }
        let k = key(ids);
        match self.faces.get_mut(&k) {
            Some(e) => e.0 += 1,
            None => {
                let start = self.face_ids.len();
                self.face_ids.extend_from_slice(ids);
                self.faces.insert(k.clone(), (1, cell, start, ids.len()));
                self.face_order.push(k);
            }
        }
    }

    fn finish(mut self) -> Result<Triangles> {
        let order = std::mem::take(&mut self.face_order);
        let faces = std::mem::take(&mut self.faces);
        let ids = std::mem::take(&mut self.face_ids);
        for k in &order {
            if let Some(&(1, cell, s, n)) = faces.get(k) {
                self.fan(&ids[s..s + n], cell)?;
            }
        }
        if self.invalid > 0 {
            self.out.notes.push(format!(
                "{} cells with point ids out of range skipped",
                self.invalid
            ));
        }
        if self.unknown > 0 {
            self.out.notes.push(format!(
                "{} cells of unsupported types skipped",
                self.unknown
            ));
        }
        if self.out.vertices > 0 || self.out.lines > 0 {
            self.out.notes.push(format!(
                "{} vertex and {} line cells not drawn",
                self.out.vertices, self.out.lines
            ));
        }
        Ok(self.out)
    }
}

/// The triangles of `cells` over `points` points.
pub(crate) fn triangulate(points: usize, cells: Cells<'_>) -> Result<Triangles> {
    let mut b = Builder {
        out: Triangles::default(),
        points,
        invalid: 0,
        unknown: 0,
        faces: HashMap::new(),
        face_order: Vec::new(),
        face_ids: Vec::new(),
    };
    match cells {
        Cells::Poly {
            verts,
            lines,
            polys,
            strips,
        } => {
            let mut cell = 0u32;
            b.out.vertices = verts.len() as u64;
            b.out.lines = lines.len() as u64;
            cell = cell.saturating_add((verts.len() + lines.len()) as u32);
            for (arr, is_strip) in [(polys, false), (strips, true)] {
                for i in 0..arr.len() {
                    let c = arr.cell(i).unwrap_or(&[]);
                    match b.ids(c) {
                        Some(ids) if is_strip => b.strip(&ids, cell),
                        Some(ids) => b.fan(&ids, cell)?,
                        None => b.invalid += 1,
                    }
                    cell = cell.saturating_add(1);
                }
            }
            b.out.cells = (verts.len() + lines.len() + polys.len() + strips.len()) as u64;
        }
        Cells::Unstructured {
            cells,
            types,
            polyhedra,
        } => {
            if types.len() < cells.len() {
                return Err(Error::Truncated("cell types".into()));
            }
            b.out.cells = cells.len() as u64;
            for (i, &ty) in types.iter().enumerate().take(cells.len()) {
                let cell = u32::try_from(i).map_err(|_| Error::TooLarge)?;
                let c = cells.cell(i).unwrap_or(&[]);
                match shape(ty) {
                    Shape::Skip => {}
                    Shape::Vertex => b.out.vertices += 1,
                    Shape::Line => b.out.lines += 1,
                    Shape::Unknown => b.unknown += 1,
                    Shape::Polyhedron => {
                        if !polyhedron(&mut b, polyhedra, c, i, cell) {
                            b.invalid += 1;
                        }
                    }
                    s => {
                        let Some(ids) = b.ids(c) else {
                            b.invalid += 1;
                            continue;
                        };
                        match s {
                            Shape::Polygon(n) => {
                                let n = if n == 0 { ids.len() } else { n };
                                match ids.get(..n) {
                                    Some(p) => b.fan(p, cell)?,
                                    None => b.invalid += 1,
                                }
                            }
                            Shape::HalfPolygon => b.fan(&ids[..ids.len() / 2], cell)?,
                            Shape::Pixel if ids.len() >= 4 => {
                                b.fan(&[ids[0], ids[1], ids[3], ids[2]], cell)?
                            }
                            Shape::Strip => b.strip(&ids, cell),
                            Shape::Solid(faces, n) if ids.len() >= n => {
                                for f in faces {
                                    let face: Vec<u32> = f.iter().map(|&k| ids[k]).collect();
                                    b.face(&face, cell);
                                }
                            }
                            _ => b.invalid += 1,
                        }
                    }
                }
            }
        }
    }
    b.finish()
}

/// Adds a polyhedron's faces; false when they cannot be read.
fn polyhedron(b: &mut Builder, p: &Polyhedra, cell_ids: &[i64], i: usize, cell: u32) -> bool {
    let add_stream = |b: &mut Builder, s: &[i64]| -> bool {
        let Some(&n) = s.first() else {
            return false;
        };
        let mut at = 1usize;
        for _ in 0..n.max(0) {
            let Some(&m) = s.get(at) else {
                return false;
            };
            let Ok(m) = usize::try_from(m) else {
                return false;
            };
            let Some(face) = s.get(at + 1..at + 1 + m) else {
                return false;
            };
            let Some(ids) = b.ids(face) else {
                return false;
            };
            b.face(&ids, cell);
            at += 1 + m;
        }
        true
    };
    match p {
        Polyhedra::InCells => add_stream(b, cell_ids),
        Polyhedra::Stream { faces, ends } => {
            let Some(&end) = ends.get(i) else {
                return false;
            };
            let start = ends[..i]
                .iter()
                .rev()
                .find(|&&e| e >= 0)
                .copied()
                .unwrap_or(0);
            match (usize::try_from(start), usize::try_from(end)) {
                (Ok(s), Ok(e)) if s <= e => match faces.get(s..e) {
                    Some(st) => add_stream(b, st),
                    None => false,
                },
                _ => false,
            }
        }
        Polyhedra::Lists { faces, cell_faces } => {
            let Some(fs) = cell_faces.cell(i) else {
                return false;
            };
            for &f in fs {
                let Some(face) = usize::try_from(f).ok().and_then(|f| faces.cell(f)) else {
                    return false;
                };
                let Some(ids) = b.ids(face) else {
                    return false;
                };
                b.face(&ids, cell);
            }
            true
        }
    }
}
