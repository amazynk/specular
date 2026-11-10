//! Legacy `.vtk` files: a three-line header (version, title, ASCII or BINARY), then keyword
//! sections. Binary data is big-endian and follows its keyword line directly.

use crate::cells::{self, CellArray, Cells, Polyhedra};
use crate::num::{self, Ty};
use crate::{DataArray, Dataset, Error, Grid, Kind, MeshData, Result, Vtk, bad, limits};

struct Cur<'a> {
    d: &'a [u8],
    p: usize,
    binary: bool,
}

impl<'a> Cur<'a> {
    fn skip_ws(&mut self) {
        while self.p < self.d.len() && self.d[self.p].is_ascii_whitespace() {
            self.p += 1;
        }
    }

    fn word(&mut self) -> Option<&'a str> {
        self.skip_ws();
        let s = self.p;
        while self.p < self.d.len() && !self.d[self.p].is_ascii_whitespace() {
            self.p += 1;
        }
        if s == self.p {
            return None;
        }
        std::str::from_utf8(&self.d[s..self.p]).ok()
    }

    /// The next word on the current line, if any (the position stays when there is none).
    fn word_on_line(&mut self) -> Option<&'a str> {
        let save = self.p;
        while self.p < self.d.len() && matches!(self.d[self.p], b' ' | b'\t' | b'\r') {
            self.p += 1;
        }
        if self.p >= self.d.len() || self.d[self.p] == b'\n' {
            self.p = save;
            return None;
        }
        self.word()
    }

    fn need(&mut self, what: &str) -> Result<&'a str> {
        self.word()
            .ok_or_else(|| Error::Truncated(format!("{what} expected")))
    }

    fn int(&mut self, what: &str) -> Result<u64> {
        let w = self.need(what)?;
        w.parse::<u64>()
            .map_err(|_| Error::Malformed(format!("{what}: {w}")))
    }

    fn float(&mut self, what: &str) -> Result<f64> {
        let w = self.need(what)?;
        num::parse_f64(w).ok_or_else(|| Error::Malformed(format!("{what}: {w}")))
    }

    fn line(&mut self) -> &'a [u8] {
        let s = self.p;
        while self.p < self.d.len() && self.d[self.p] != b'\n' {
            self.p += 1;
        }
        let l = &self.d[s..self.p];
        if self.p < self.d.len() {
            self.p += 1;
        }
        l
    }

    /// Binary data starts after the end of the keyword line.
    fn data_start(&mut self) {
        if self.binary {
            self.line();
        }
    }

    fn bytes(&mut self, ty: Ty, n: u64) -> Result<&'a [u8]> {
        let len = ty.bytes_for(n).ok_or(Error::TooLarge)?;
        let rest = (self.d.len() - self.p) as u64;
        if len > rest {
            return Err(Error::Truncated("binary data".into()));
        }
        let s = &self.d[self.p..self.p + len as usize];
        self.p += len as usize;
        Ok(s)
    }

    /// Checks `n` ASCII values could be present (each takes two bytes at least, the last
    /// one).
    fn ascii_room(&self, n: u64) -> Result<()> {
        if n > limits::MAX_VALUES || n.saturating_mul(2) > (self.d.len() - self.p) as u64 + 1 {
            return Err(Error::Truncated("ASCII data".into()));
        }
        Ok(())
    }

    fn f32s(&mut self, ty: Ty, n: u64) -> Result<Vec<f32>> {
        self.data_start();
        let mut out = Vec::new();
        if self.binary {
            let b = self.bytes(ty, n)?;
            num::to_f32(ty, b, n as usize, true, &mut out)?;
        } else {
            self.ascii_room(n)?;
            num::reserve(&mut out, n)?;
            for _ in 0..n {
                let w = self.need("a value")?;
                out.push(
                    num::parse_f32(ty, w).ok_or_else(|| Error::Malformed(format!("value {w}")))?,
                );
            }
        }
        Ok(out)
    }

    fn f64s(&mut self, ty: Ty, n: u64) -> Result<Vec<f64>> {
        self.data_start();
        if self.binary {
            let b = self.bytes(ty, n)?;
            num::to_f64(ty, b, n as usize, true)
        } else {
            self.ascii_room(n)?;
            let mut out = Vec::new();
            num::reserve(&mut out, n)?;
            for _ in 0..n {
                let w = self.need("a value")?;
                out.push(num::parse_f64(w).ok_or_else(|| Error::Malformed(format!("value {w}")))?);
            }
            Ok(out)
        }
    }

    fn i64s(&mut self, ty: Ty, n: u64) -> Result<Vec<i64>> {
        self.data_start();
        if self.binary {
            let b = self.bytes(ty, n)?;
            num::to_i64(ty, b, n as usize, true)
        } else {
            self.ascii_room(n)?;
            let mut out = Vec::new();
            num::reserve(&mut out, n)?;
            for _ in 0..n {
                let w = self.need("an integer")?;
                let v = num::parse_int(w)
                    .and_then(|v| i64::try_from(v).ok())
                    .ok_or_else(|| Error::Malformed(format!("integer {w}")))?;
                out.push(v);
            }
            Ok(out)
        }
    }

    /// `METADATA` blocks (5.x): lines up to an empty one.
    fn skip_metadata(&mut self) {
        self.line();
        while self.p < self.d.len() {
            let l = self.line();
            if l.iter().all(|b| b.is_ascii_whitespace()) {
                break;
            }
        }
    }
}

/// `%XX` escapes (names with spaces in 5.x files).
fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Some(v) = std::str::from_utf8(&b[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[derive(Clone, Copy, PartialEq)]
enum Section {
    Dataset,
    Point(u64),
    Cell(u64),
}

#[derive(Default)]
struct State {
    structure: Option<String>,
    dims: Option<[usize; 3]>,
    origin: [f64; 3],
    spacing: [f64; 3],
    points: Option<Vec<[f32; 3]>>,
    coords: [Option<Vec<f64>>; 3],
    verts: CellArray,
    lines: CellArray,
    polys: CellArray,
    strips: CellArray,
    cells: CellArray,
    types: Vec<i64>,
    point_data: Vec<DataArray>,
    cell_data: Vec<DataArray>,
    field_data: Vec<DataArray>,
}

pub(crate) fn parse(data: &[u8]) -> Result<Vtk> {
    let mut c = Cur {
        d: data,
        p: 0,
        binary: false,
    };
    let first = String::from_utf8_lossy(c.line()).trim().to_owned();
    let version = first
        .split_whitespace()
        .last()
        .filter(|v| v.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
        .unwrap_or("")
        .to_owned();
    let title = String::from_utf8_lossy(c.line()).trim().to_owned();
    let enc = c.need("ASCII or BINARY")?.to_ascii_uppercase();
    c.binary = match enc.as_str() {
        "ASCII" => false,
        "BINARY" => true,
        _ => return bad(format!("data type {enc}")),
    };
    let mut st = State {
        spacing: [1.0; 3],
        ..State::default()
    };
    let mut section = Section::Dataset;
    while let Some(w) = c.word() {
        let kw = w.to_ascii_uppercase();
        match kw.as_str() {
            "DATASET" => st.structure = Some(c.need("dataset type")?.to_ascii_uppercase()),
            "DIMENSIONS" => {
                let mut d = [0usize; 3];
                for v in &mut d {
                    *v = usize::try_from(c.int("dimension")?).map_err(|_| Error::TooLarge)?;
                }
                st.dims = Some(d);
            }
            "ORIGIN" => {
                for k in 0..3 {
                    st.origin[k] = c.float("origin")?;
                }
            }
            "SPACING" | "ASPECT_RATIO" => {
                for k in 0..3 {
                    st.spacing[k] = c.float("spacing")?;
                }
            }
            "POINTS" => {
                let n = c.int("point count")?;
                let (ty, _) = Ty::legacy(c.need("point type")?)?;
                let v = c.f32s(ty, n.checked_mul(3).ok_or(Error::TooLarge)?)?;
                st.points = Some(v.chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect());
            }
            "X_COORDINATES" | "Y_COORDINATES" | "Z_COORDINATES" => {
                let k = usize::from(kw.as_bytes()[0] - b'X');
                let n = c.int("coordinate count")?;
                let (ty, _) = Ty::legacy(c.need("coordinate type")?)?;
                st.coords[k] = Some(c.f64s(ty, n)?);
            }
            "VERTICES" | "LINES" | "POLYGONS" | "TRIANGLE_STRIPS" | "CELLS" => {
                let arr = read_cells(&mut c)?;
                match kw.as_str() {
                    "VERTICES" => st.verts = arr,
                    "LINES" => st.lines = arr,
                    "POLYGONS" => st.polys = arr,
                    "TRIANGLE_STRIPS" => st.strips = arr,
                    _ => st.cells = arr,
                }
            }
            "CELL_TYPES" => {
                let n = c.int("cell type count")?;
                st.types = c.i64s(Ty::I32, n)?;
            }
            "METADATA" => c.skip_metadata(),
            "POINT_DATA" => section = Section::Point(c.int("point data count")?),
            "CELL_DATA" => section = Section::Cell(c.int("cell data count")?),
            "FIELD" => {
                let _name = c.need("field name")?;
                let n = c.int("array count")?;
                let arrays = read_field(&mut c, n)?;
                push_arrays(&mut st, section, arrays)?;
            }
            "LOOKUP_TABLE" => {
                // A table of its own: RGBA per entry.
                let _name = c.need("table name")?;
                let n = c.int("table size")?;
                let n4 = n.checked_mul(4).ok_or(Error::TooLarge)?;
                if c.binary {
                    c.data_start();
                    c.bytes(Ty::U8, n4)?;
                } else {
                    c.f32s(Ty::F32, n4)?;
                }
            }
            "SCALARS"
            | "COLOR_SCALARS"
            | "VECTORS"
            | "NORMALS"
            | "TEXTURE_COORDINATES"
            | "TENSORS"
            | "TENSORS6"
            | "GLOBAL_IDS"
            | "PEDIGREE_IDS"
            | "EDGE_FLAGS" => {
                let n = match section {
                    Section::Point(n) | Section::Cell(n) => n,
                    Section::Dataset => return bad(format!("{kw} outside POINT_DATA/CELL_DATA")),
                };
                let a = read_attribute(&mut c, &kw, n)?;
                push_arrays(&mut st, section, vec![a])?;
            }
            _ => return bad(format!("unknown keyword {w}")),
        }
    }
    finish(st, title, version, c.binary)
}

fn read_cells(c: &mut Cur<'_>) -> Result<CellArray> {
    let n = c.int("cell count")?;
    let size = c.int("cell list size")?;
    let save = c.p;
    if c.word().is_some_and(|w| w.eq_ignore_ascii_case("OFFSETS")) {
        let (ty, _) = Ty::legacy(c.need("offset type")?)?;
        let offsets = c.i64s(ty, n)?;
        if !c
            .word()
            .is_some_and(|w| w.eq_ignore_ascii_case("CONNECTIVITY"))
        {
            return bad("CONNECTIVITY expected");
        }
        let (ty, _) = Ty::legacy(c.need("connectivity type")?)?;
        let conn = c.i64s(ty, size)?;
        if offsets.is_empty() {
            return Ok(CellArray::default());
        }
        return CellArray::checked(offsets, conn);
    }
    c.p = save;
    let stream = c.i64s(Ty::I32, size)?;
    CellArray::from_counted(stream, usize::try_from(n).map_err(|_| Error::TooLarge)?)
}

fn read_field(c: &mut Cur<'_>, n: u64) -> Result<Vec<DataArray>> {
    if n as usize > limits::MAX_ARRAYS {
        return Err(Error::TooLarge);
    }
    let mut out = Vec::new();
    for _ in 0..n {
        let mut name = c.need("array name")?;
        // 5.x: each array may be followed by its METADATA block.
        while name.eq_ignore_ascii_case("METADATA") {
            c.skip_metadata();
            name = c.need("array name")?;
        }
        if name == "NULL_ARRAY" {
            continue;
        }
        let comps = c.int("component count")?;
        let tuples = c.int("tuple count")?;
        let tyname = c.need("array type")?;
        let total = comps.checked_mul(tuples).ok_or(Error::TooLarge)?;
        let (ty, dtype) = match Ty::legacy(tyname) {
            Ok(t) => t,
            Err(Error::Unsupported(_)) if !c.binary => {
                // A string array in an ASCII file: one value per line; skipped.
                c.line();
                for _ in 0..total {
                    c.line();
                }
                continue;
            }
            Err(e) => return Err(e),
        };
        let values = c.f32s(ty, total)?;
        out.push(DataArray {
            name: unescape(name),
            components: comps as usize,
            dtype,
            values,
            attribute: Some("FIELD"),
        });
    }
    Ok(out)
}

fn read_attribute(c: &mut Cur<'_>, kw: &str, n: u64) -> Result<DataArray> {
    let name = unescape(c.need("attribute name")?);
    let (role, comps, ty, dtype) = match kw {
        "SCALARS" => {
            let (ty, dtype) = Ty::legacy(c.need("scalar type")?)?;
            let comps = match c.word_on_line() {
                Some(w) => w
                    .parse::<u64>()
                    .map_err(|_| Error::Malformed(format!("component count {w}")))?,
                None => 1,
            };
            let save = c.p;
            match c.word() {
                Some(w) if w.eq_ignore_ascii_case("LOOKUP_TABLE") => {
                    c.need("table name")?;
                }
                _ => c.p = save,
            }
            ("SCALARS", comps, ty, dtype)
        }
        "COLOR_SCALARS" => {
            let comps = c.int("component count")?;
            let total = n.checked_mul(comps).ok_or(Error::TooLarge)?;
            let values = if c.binary {
                c.f32s(Ty::U8, total)?
            } else {
                let mut v = c.f32s(Ty::F32, total)?;
                for x in &mut v {
                    *x = f32::from((f64::from(*x) * 255.0 + 0.5) as u8);
                }
                v
            };
            return Ok(DataArray {
                name,
                components: comps as usize,
                dtype: "unsigned_char",
                values,
                attribute: Some("COLOR_SCALARS"),
            });
        }
        "TEXTURE_COORDINATES" => {
            let comps = c.int("dimension")?;
            let (ty, dtype) = Ty::legacy(c.need("type")?)?;
            ("TEXTURE_COORDINATES", comps, ty, dtype)
        }
        _ => {
            let (ty, dtype) = Ty::legacy(c.need("type")?)?;
            let (role, comps) = match kw {
                "VECTORS" => ("VECTORS", 3),
                "NORMALS" => ("NORMALS", 3),
                "TENSORS" => ("TENSORS", 9),
                "TENSORS6" => ("TENSORS", 6),
                "GLOBAL_IDS" => ("GLOBAL_IDS", 1),
                "PEDIGREE_IDS" => ("PEDIGREE_IDS", 1),
                _ => ("EDGE_FLAGS", 1),
            };
            (role, comps, ty, dtype)
        }
    };
    if comps == 0 || comps > 1 << 16 {
        return bad("component count");
    }
    let total = n.checked_mul(comps).ok_or(Error::TooLarge)?;
    let values = c.f32s(ty, total)?;
    Ok(DataArray {
        name,
        components: comps as usize,
        dtype,
        values,
        attribute: Some(role),
    })
}

fn push_arrays(st: &mut State, section: Section, arrays: Vec<DataArray>) -> Result<()> {
    let target = match section {
        Section::Dataset => &mut st.field_data,
        Section::Point(_) => &mut st.point_data,
        Section::Cell(_) => &mut st.cell_data,
    };
    if target.len() + arrays.len() > limits::MAX_ARRAYS {
        return Err(Error::TooLarge);
    }
    target.extend(arrays);
    Ok(())
}

fn check_tuples(arrays: &[DataArray], n: usize, what: &str) -> Result<()> {
    for a in arrays {
        if a.tuples() != n || a.values.len() % a.components.max(1) != 0 {
            return bad(format!(
                "{what} array {} has {} tuples, {n} expected",
                a.name,
                a.tuples()
            ));
        }
    }
    Ok(())
}

fn make_grid(s: &'static str, st: &mut State) -> Result<Grid> {
    let dims = st
        .dims
        .ok_or_else(|| Error::Malformed("no DIMENSIONS".into()))?
        .map(|d| d.max(1));
    let n = dims
        .iter()
        .try_fold(1usize, |a, &d| a.checked_mul(d))
        .ok_or(Error::TooLarge)?;
    let g = Grid {
        structure: s,
        dims,
        origin: st.origin,
        spacing: st.spacing,
        direction: None,
        coords: None,
        points: None,
        point_data: std::mem::take(&mut st.point_data),
        cell_data: std::mem::take(&mut st.cell_data),
    };
    check_tuples(&g.point_data, n, "point")?;
    let nc = g.cell_dims().iter().product::<usize>();
    check_tuples(&g.cell_data, nc, "cell")?;
    Ok(g)
}

fn finish(mut st: State, title: String, version: String, binary: bool) -> Result<Vtk> {
    let structure = st
        .structure
        .clone()
        .ok_or_else(|| Error::Malformed("no DATASET".into()))?;
    let dataset = match structure.as_str() {
        "STRUCTURED_POINTS" => Dataset::Grid(make_grid("STRUCTURED_POINTS", &mut st)?),
        "STRUCTURED_GRID" => {
            let mut g = make_grid("STRUCTURED_GRID", &mut st)?;
            let pts = st
                .points
                .ok_or_else(|| Error::Malformed("no POINTS".into()))?;
            if pts.len() != g.dims.iter().product::<usize>() {
                return bad("POINTS does not match DIMENSIONS");
            }
            g.points = Some(pts);
            Dataset::Grid(g)
        }
        "RECTILINEAR_GRID" => {
            let mut g = make_grid("RECTILINEAR_GRID", &mut st)?;
            let [x, y, z] = std::mem::take(&mut st.coords);
            let (Some(x), Some(y), Some(z)) = (x, y, z) else {
                return bad("missing coordinates");
            };
            if [x.len(), y.len(), z.len()] != g.dims {
                return bad("coordinates do not match DIMENSIONS");
            }
            g.coords = Some([x, y, z]);
            Dataset::Grid(g)
        }
        "POLYDATA" | "UNSTRUCTURED_GRID" => {
            let points = st.points.unwrap_or_default();
            let polyhedra = Polyhedra::InCells;
            let t = if structure == "POLYDATA" {
                cells::triangulate(
                    points.len(),
                    Cells::Poly {
                        verts: &st.verts,
                        lines: &st.lines,
                        polys: &st.polys,
                        strips: &st.strips,
                    },
                )?
            } else {
                cells::triangulate(
                    points.len(),
                    Cells::Unstructured {
                        cells: &st.cells,
                        types: &st.types,
                        polyhedra: &polyhedra,
                    },
                )?
            };
            check_tuples(&st.point_data, points.len(), "point")?;
            check_tuples(&st.cell_data, t.cells as usize, "cell")?;
            Dataset::Mesh(MeshData {
                points,
                triangles: t.triangles,
                point_data: st.point_data,
                cell_data: st.cell_data,
                triangle_cell: t.triangle_cell,
                cells: t.cells,
                vertices: t.vertices,
                lines: t.lines,
                notes: t.notes,
            })
        }
        "FIELD" => {
            return Err(Error::Unsupported(
                "this VTK file holds only field data, no grid or mesh".into(),
            ));
        }
        s => {
            return Err(Error::Unsupported(format!(
                "VTK {s} datasets are not supported"
            )));
        }
    };
    Ok(Vtk {
        kind: Kind::Legacy,
        title: Some(title).filter(|t| !t.is_empty()),
        version,
        encoding: if binary { "binary" } else { "ASCII" }.to_owned(),
        dataset,
        field_data: st.field_data,
    })
}
