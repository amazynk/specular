//! FlatGeobuf (version 3): a magic, a FlatBuffers header (name, geometry type, columns,
//! CRS, feature count), an optional packed Hilbert R-tree of feature boxes, then the
//! features, each a size-prefixed FlatBuffers table holding a geometry and its properties.
//!
//! [`read`] reads every feature in file order; [`read_bbox`] walks the R-tree and reads
//! only the features whose boxes meet a rectangle (a linear scan when there is no index).
//! Curves are stroked into lines; Z, M and time values are not kept.

use crate::crs;
use crate::flatbuf::Table;
use crate::model::{
    Dataset, Geometry, Layer, MAX_FEATURES, MAX_VERTICES, Pt, Value, stroke_circular,
};

const MAGIC: &[u8; 3] = b"fgb";
/// One R-tree node: min x, min y, max x, max y and an offset.
const NODE: usize = 40;

/// The magic is "fgb", the major version 3, "fgb" and a patch version; like GDAL only the
/// first four bytes are checked.
pub fn sniff(head: &[u8]) -> bool {
    head.len() >= 8 && &head[..3] == MAGIC && head[3] == 3
}

/// GeometryType from the FlatGeobuf schema.
fn type_name(t: u8) -> &'static str {
    match t {
        1 => "Point",
        2 => "LineString",
        3 => "Polygon",
        4 => "MultiPoint",
        5 => "MultiLineString",
        6 => "MultiPolygon",
        7 => "GeometryCollection",
        8 => "CircularString",
        9 => "CompoundCurve",
        10 => "CurvePolygon",
        11 => "MultiCurve",
        12 => "MultiSurface",
        13 => "Curve",
        14 => "Surface",
        15 => "PolyhedralSurface",
        16 => "TIN",
        17 => "Triangle",
        _ => "Unknown",
    }
}

#[derive(Clone, Debug)]
struct Column {
    name: String,
    ty: u8,
}

fn columns(v: Option<crate::flatbuf::Vector<'_>>) -> Vec<Column> {
    v.map(|v| {
        v.tables()
            .map(|c| Column {
                name: c.str(0).unwrap_or_default(),
                ty: c.u8(1, 0),
            })
            .collect()
    })
    .unwrap_or_default()
}

/// An opened FlatGeobuf file.
pub struct Fgb<'a> {
    data: &'a [u8],
    pub name: Option<String>,
    pub geometry_type: u8,
    columns: Vec<Column>,
    pub features_count: u64,
    node_size: u16,
    /// Where the R-tree starts (when there is one) and where the features start.
    index_at: Option<usize>,
    features_at: usize,
    pub crs: Option<crs::Crs>,
    pub crs_note: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub has_z: bool,
    pub has_m: bool,
}

/// The (start, end) node index of each R-tree level, leaves first.
fn level_bounds(items: u64, node_size: u64) -> Option<Vec<(u64, u64)>> {
    if node_size < 2 || items == 0 {
        return None;
    }
    let mut n = items;
    let mut nodes = n;
    let mut per_level = vec![n];
    loop {
        n = n.div_ceil(node_size);
        nodes = nodes.checked_add(n)?;
        per_level.push(n);
        if n == 1 {
            break;
        }
    }
    let mut out = Vec::with_capacity(per_level.len());
    let mut end = nodes;
    for size in per_level {
        out.push((end - size, end));
        end -= size;
    }
    Some(out)
}

fn tree_bytes(items: u64, node_size: u16) -> Option<u64> {
    let levels = level_bounds(items, u64::from(node_size))?;
    levels.first()?.1.checked_mul(NODE as u64)
}

impl<'a> Fgb<'a> {
    pub fn open(data: &'a [u8]) -> Result<Fgb<'a>, String> {
        if !sniff(data) {
            return Err("not a FlatGeobuf file".into());
        }
        let size = u32::from_le_bytes(
            data.get(8..12)
                .ok_or("truncated")?
                .try_into()
                .unwrap_or_default(),
        ) as usize;
        let hbuf = data
            .get(12..12usize.checked_add(size).ok_or("bad header size")?)
            .ok_or("the header is truncated")?;
        let h = Table::root(hbuf).ok_or("the header is damaged")?;
        let geometry_type = h.u8(2, 0);
        let features_count = h.u64(8, 0);
        let node_size = h.u16(9, 16);
        let mut at = 12 + size;
        let mut index_at = None;
        if node_size > 0 && features_count > 0 {
            let t = tree_bytes(features_count, node_size).ok_or("bad index size")?;
            let t = usize::try_from(t).map_err(|_| "index too large")?;
            if at.checked_add(t).is_none_or(|e| e > data.len()) {
                return Err("the spatial index is truncated".into());
            }
            index_at = Some(at);
            at += t;
        }
        let (crs, crs_note) = match h.table(10) {
            None => (None, None),
            Some(c) => {
                let org = c.str(0).unwrap_or_default();
                let code = c.i32(1, 0);
                let wkt = c.str(4).filter(|w| !w.trim().is_empty());
                let code_string = c.str(5);
                let by_code = (code > 0 && (org.is_empty() || org.eq_ignore_ascii_case("EPSG")))
                    .then(|| crs::from_epsg(code as u32))
                    .flatten();
                let found = by_code
                    .or_else(|| wkt.as_deref().and_then(crate::wkt::crs_from_wkt))
                    .or_else(|| {
                        code_string
                            .as_deref()
                            .and_then(crs::epsg_from_ref)
                            .and_then(crs::from_epsg)
                    });
                let note = found.is_none().then(|| {
                    if code > 0 {
                        format!("{} {code}", if org.is_empty() { "EPSG" } else { &org })
                    } else {
                        c.str(2).or(code_string).unwrap_or_else(|| "unknown".into())
                    }
                });
                (found, note)
            }
        };
        Ok(Fgb {
            data,
            name: h.str(0).filter(|s| !s.is_empty()),
            geometry_type,
            columns: columns(h.vector(7)),
            features_count,
            node_size,
            index_at,
            features_at: at,
            crs,
            crs_note,
            title: h.str(11).filter(|s| !s.is_empty()),
            description: h.str(12).filter(|s| !s.is_empty()),
            has_z: h.bool(3, false),
            has_m: h.bool(4, false),
        })
    }

    pub fn has_index(&self) -> bool {
        self.index_at.is_some()
    }

    /// The declared geometry type's name.
    pub fn type_name(&self) -> &'static str {
        type_name(self.geometry_type)
    }

    /// Offsets (from the start of the features) of every feature whose box meets `bbox`
    /// (min x, min y, max x, max y), by walking the R-tree; `None` without an index.
    pub fn search(&self, bbox: [f64; 4]) -> Option<Vec<u64>> {
        let start = self.index_at?;
        let levels = level_bounds(self.features_count, u64::from(self.node_size))?;
        let node = |i: u64| -> Option<([f64; 4], u64)> {
            let at = start.checked_add(usize::try_from(i).ok()?.checked_mul(NODE)?)?;
            let b = self.data.get(at..at + NODE)?;
            let f =
                |k: usize| f64::from_le_bytes(b[k * 8..k * 8 + 8].try_into().unwrap_or_default());
            Some((
                [f(0), f(1), f(2), f(3)],
                u64::from_le_bytes(b[32..40].try_into().unwrap_or_default()),
            ))
        };
        let mut out = Vec::new();
        let top = levels.len() - 1;
        let mut queue = vec![(0u64, top)];
        let mut visits = 0u64;
        let budget = levels[0].1.saturating_mul(2);
        while let Some((first, level)) = queue.pop() {
            let end = first
                .saturating_add(u64::from(self.node_size))
                .min(levels[level].1);
            for i in first..end {
                visits += 1;
                if visits > budget {
                    return None; // a cycle in a damaged index
                }
                let (b, off) = node(i)?;
                if b[0] > bbox[2] || b[1] > bbox[3] || b[2] < bbox[0] || b[3] < bbox[1] {
                    continue;
                }
                if level == 0 {
                    out.push(off);
                } else {
                    let (lo, hi) = levels[level - 1];
                    if off < lo || off >= hi {
                        return None;
                    }
                    queue.push((off, level - 1));
                }
            }
        }
        out.sort_unstable();
        Some(out)
    }

    /// The feature at `offset` from the start of the features, and its size with prefix.
    fn feature_at(&self, offset: usize) -> Option<(Table<'a>, usize)> {
        let at = self.features_at.checked_add(offset)?;
        let len = u32::from_le_bytes(self.data.get(at..at + 4)?.try_into().ok()?) as usize;
        let buf = self.data.get(at + 4..(at + 4).checked_add(len)?)?;
        Some((Table::root(buf)?, len + 4))
    }

    fn layer(&self, fallback: &str) -> Layer {
        let mut l = Layer::new(self.name.clone().unwrap_or_else(|| fallback.to_string()));
        for c in &self.columns {
            l.field(&c.name);
        }
        l.declared_type = (self.geometry_type != 0).then(|| self.type_name().to_string());
        l
    }

    fn dataset(&self, layer: Layer, warnings: Vec<String>) -> Dataset {
        let mut ds = Dataset {
            format: "FlatGeobuf",
            layers: vec![layer],
            crs: self.crs.clone(),
            crs_note: self.crs_note.clone(),
            ..Dataset::default()
        };
        for w in warnings {
            ds.warn(w);
        }
        ds
    }

    /// Decodes one feature into the layer; `index` is its position in the file.
    fn push(&self, f: Table<'_>, index: usize, layer: &mut Layer, st: &mut State) {
        let geometry = f.table(0).and_then(|g| {
            // `geometry` counts the vertices as it reads them.
            geometry(g, self.geometry_type, 0, st)
        });
        let own = columns(f.vector(2));
        let cols = if own.is_empty() { &self.columns } else { &own };
        let mut values: Vec<(String, Value)> = Vec::new();
        if let Some(p) = f.bytes(1)
            && let Err(e) = properties(p, cols, &mut values)
        {
            st.warn(e);
        }
        layer.push(Some(index.to_string()), geometry, values);
    }

    /// Every feature, in file order.
    pub fn read_all(&self, fallback_name: &str) -> Result<Dataset, String> {
        let mut layer = self.layer(fallback_name);
        let mut st = State::default();
        let mut off = 0usize;
        let mut i = 0usize;
        while self.features_at + off < self.data.len() {
            if self.features_count > 0 && i as u64 >= self.features_count {
                break;
            }
            if i >= MAX_FEATURES || st.vertices > MAX_VERTICES {
                st.warn("too many features; the rest are not shown".into());
                break;
            }
            let Some((f, len)) = self.feature_at(off) else {
                st.warn(format!(
                    "feature {} is damaged; the rest are not shown",
                    i + 1
                ));
                break;
            };
            self.push(f, i, &mut layer, &mut st);
            off += len;
            i += 1;
        }
        if self.features_count > 0 && (i as u64) < self.features_count && st.warnings.is_empty() {
            st.warn(format!(
                "the header promises {} features, the file holds {i}",
                self.features_count
            ));
        }
        Ok(self.dataset(layer, st.warnings))
    }

    /// The features whose boxes meet `bbox`, through the index (a linear scan without one).
    pub fn read_bbox(&self, fallback_name: &str, bbox: [f64; 4]) -> Result<Dataset, String> {
        let Some(offsets) = self.search(bbox) else {
            let mut ds = self.read_all(fallback_name)?;
            for l in &mut ds.layers {
                l.features.retain(|f| {
                    f.geometry
                        .as_ref()
                        .and_then(Geometry::bounds)
                        .is_some_and(|b| {
                            b[0] <= bbox[2] && b[1] <= bbox[3] && b[2] >= bbox[0] && b[3] >= bbox[1]
                        })
                });
            }
            return Ok(ds);
        };
        let mut layer = self.layer(fallback_name);
        let mut st = State::default();
        for (n, off) in offsets.into_iter().enumerate() {
            if n >= MAX_FEATURES {
                st.warn("too many features; the rest are not shown".into());
                break;
            }
            let Some((f, _)) = usize::try_from(off).ok().and_then(|o| self.feature_at(o)) else {
                st.warn("the index points outside the features".into());
                continue;
            };
            self.push(f, n, &mut layer, &mut st);
        }
        Ok(self.dataset(layer, st.warnings))
    }
}

#[derive(Default)]
struct State {
    vertices: usize,
    warnings: Vec<String>,
    curves: bool,
}

impl State {
    fn warn(&mut self, w: String) {
        if !self.warnings.contains(&w) && self.warnings.len() < 16 {
            self.warnings.push(w);
        }
    }
}

fn split(xy: &[Pt], ends: &[u32]) -> Vec<Vec<Pt>> {
    if ends.is_empty() {
        return vec![xy.to_vec()];
    }
    let mut out = Vec::with_capacity(ends.len());
    let mut s = 0usize;
    for &e in ends {
        let e = (e as usize).min(xy.len());
        if e < s {
            break;
        }
        out.push(xy[s..e].to_vec());
        s = e;
    }
    out
}

/// A curve part (LineString, CircularString or CompoundCurve) as points.
fn curve(g: Table<'_>, ty: u8, depth: usize, st: &mut State) -> Option<Vec<Pt>> {
    match geometry(g, ty, depth, st)? {
        Geometry::LineString(l) => Some(l),
        Geometry::MultiLineString(ls) => Some(ls.into_iter().flatten().collect()),
        _ => None,
    }
}

fn geometry(g: Table<'_>, parent: u8, depth: usize, st: &mut State) -> Option<Geometry> {
    if depth > 16 {
        return None;
    }
    let ty = match g.u8(6, 0) {
        0 => parent,
        t => t,
    };
    let xy: Vec<Pt> = g
        .vector(1)
        .and_then(|v| v.f64s())
        .map(|it| {
            let v: Vec<f64> = it.collect();
            v.chunks_exact(2).map(|c| [c[0], c[1]]).collect()
        })
        .unwrap_or_default();
    // Counted here, per part read: parts may be shared, so a small file could otherwise
    // repeat one part's points 2^16 times before the total is checked.
    st.vertices = st.vertices.saturating_add(xy.len().max(1));
    if st.vertices > MAX_VERTICES {
        return None;
    }
    let ends: Vec<u32> = g
        .vector(0)
        .and_then(|v| v.u32s())
        .map(Iterator::collect)
        .unwrap_or_default();
    let parts: Vec<Table<'_>> = g
        .vector(7)
        .map(|v| v.tables().collect())
        .unwrap_or_default();
    let sub = |st: &mut State, want: u8| -> Vec<Geometry> {
        parts
            .iter()
            .filter_map(|p| geometry(*p, want, depth + 1, st))
            .collect()
    };
    Some(match ty {
        1 => Geometry::Point(*xy.first()?),
        2 => Geometry::LineString(xy),
        3 | 17 => Geometry::Polygon(split(&xy, &ends)),
        4 => Geometry::MultiPoint(xy),
        5 => Geometry::MultiLineString(split(&xy, &ends)),
        6 | 15 | 16 => {
            if parts.is_empty() {
                // Written as one polygon.
                return Some(Geometry::MultiPolygon(vec![split(&xy, &ends)]));
            }
            Geometry::MultiPolygon(
                sub(st, 3)
                    .into_iter()
                    .flat_map(|g| match g {
                        Geometry::Polygon(p) => vec![p],
                        Geometry::MultiPolygon(m) => m,
                        _ => Vec::new(),
                    })
                    .collect(),
            )
        }
        7 => Geometry::Collection(sub(st, 0)),
        8 => {
            st.curves = true;
            Geometry::LineString(stroke_circular(&xy))
        }
        9 => {
            let mut line: Vec<Pt> = Vec::new();
            for p in &parts {
                let Some(c) = curve(*p, 2, depth + 1, st) else {
                    continue;
                };
                let skip = usize::from(line.last().is_some() && line.last() == c.first());
                line.extend_from_slice(&c[skip.min(c.len())..]);
            }
            Geometry::LineString(line)
        }
        10 => Geometry::Polygon(
            parts
                .iter()
                .filter_map(|p| curve(*p, 2, depth + 1, st))
                .collect(),
        ),
        11 => Geometry::MultiLineString(
            parts
                .iter()
                .filter_map(|p| curve(*p, 2, depth + 1, st))
                .collect(),
        ),
        12 => Geometry::MultiPolygon(
            sub(st, 3)
                .into_iter()
                .filter_map(|g| match g {
                    Geometry::Polygon(p) => Some(p),
                    _ => None,
                })
                .collect(),
        ),
        _ => {
            st.warn(format!("{} geometries are not shown", type_name(ty)));
            return None;
        }
    })
}

/// Property values: (u16 column, value) pairs in the column's binary type.
fn properties(p: &[u8], cols: &[Column], out: &mut Vec<(String, Value)>) -> Result<(), String> {
    let mut i = 0usize;
    let take = |i: &mut usize, n: usize| -> Result<&[u8], String> {
        let s = p
            .get(*i..i.checked_add(n).ok_or("bad property")?)
            .ok_or("a feature's properties are truncated")?;
        *i += n;
        Ok(s)
    };
    while i < p.len() {
        let c = u16::from_le_bytes(take(&mut i, 2)?.try_into().unwrap_or_default()) as usize;
        let col = cols
            .get(c)
            .ok_or("a property names a column the file does not declare")?;
        macro_rules! num {
            ($t:ty, $n:expr) => {
                <$t>::from_le_bytes(take(&mut i, $n)?.try_into().unwrap_or_default())
            };
        }
        let v = match col.ty {
            0 => Value::Int(i64::from(num!(i8, 1))),
            1 => Value::Int(i64::from(num!(u8, 1))),
            2 => Value::Bool(num!(u8, 1) != 0),
            3 => Value::Int(i64::from(num!(i16, 2))),
            4 => Value::Int(i64::from(num!(u16, 2))),
            5 => Value::Int(i64::from(num!(i32, 4))),
            6 => Value::Int(i64::from(num!(u32, 4))),
            7 => Value::Int(num!(i64, 8)),
            8 => {
                let u = num!(u64, 8);
                i64::try_from(u).map_or(Value::Float(u as f64), Value::Int)
            }
            9 => Value::Float(f64::from(num!(f32, 4))),
            10 => Value::Float(num!(f64, 8)),
            11..=14 => {
                let n = num!(u32, 4) as usize;
                let b = take(&mut i, n)?;
                match col.ty {
                    13 => Value::Date(String::from_utf8_lossy(b).into_owned()),
                    14 => Value::Blob(n),
                    _ => Value::Text(String::from_utf8_lossy(b).into_owned()),
                }
            }
            t => return Err(format!("unknown column type {t}")),
        };
        out.push((col.name.clone(), v));
    }
    Ok(())
}

/// Reads a FlatGeobuf file: every feature, in file order.
pub fn read(data: &[u8], name: &str) -> Result<Dataset, String> {
    let f = Fgb::open(data)?;
    let mut ds = f.read_all(name)?;
    if f.has_z || f.has_m {
        ds.warn("heights and measures are not shown");
    }
    Ok(ds)
}

/// The features of a FlatGeobuf file whose boxes meet `bbox`, through its R-tree.
pub fn read_bbox(data: &[u8], name: &str, bbox: [f64; 4]) -> Result<Dataset, String> {
    Fgb::open(data)?.read_bbox(name, bbox)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::flatbuf::build::{Builder, F};

    /// Geometry collections listing one part twice, 16 levels down, over a line of 1000
    /// points: 65 million points without the per-part vertex count.
    #[test]
    fn shared_parts_bounded() {
        let mut b = Builder::default();
        b.buf.extend_from_slice(&[0; 4]);
        let mut slots = vec![0];
        for _ in 0..16 {
            let (t, offs) = b.table(&[
                None,
                None,
                None,
                None,
                None,
                None,
                Some(F::U8(7)),
                Some(F::Off(0)),
            ]);
            for s in slots {
                b.patch(s, t);
            }
            let parts = b.offsets_n(2);
            b.patch(offs[0].0, parts);
            slots = vec![parts + 4, parts + 8];
        }
        let (leaf, offs) = b.table(&[
            None,
            Some(F::Off(0)),
            None,
            None,
            None,
            None,
            Some(F::U8(2)),
        ]);
        for s in slots {
            b.patch(s, leaf);
        }
        let xy = b.f64s(&[0.5; 2000]);
        b.patch(offs[0].0, xy);
        let mut st = State {
            vertices: MAX_VERTICES - 100_000,
            ..State::default()
        };
        let _ = geometry(Table::root(&b.buf).unwrap(), 0, 0, &mut st);
        assert!(st.vertices <= MAX_VERTICES + 1000);
    }

    /// A FlatGeobuf file of points (one per entry, with an Int "n" and a String "s"
    /// column) in EPSG:4326, with an R-tree of `node_size` (0: none), features in order.
    pub fn points_fgb(pts: &[Pt], node_size: u16) -> Vec<u8> {
        // Header.
        let mut b = Builder::default();
        let (root, offs) = b.table(&[
            Some(F::Off(0)), // name
            None,
            Some(F::U8(1)), // Point
            None,
            None,
            None,
            None,
            Some(F::Off(0)), // columns
            Some(F::U64(pts.len() as u64)),
            Some(F::U16(node_size)),
            Some(F::Off(0)), // crs
        ]);
        let name = b.bytes(b"pts");
        b.patch(offs[0].0, name);
        let cols = b.offsets_n(2);
        b.patch(offs[1].0, cols);
        let (c1, o1) = b.table(&[Some(F::Off(0)), Some(F::U8(5))]);
        b.patch(cols + 4, c1);
        let n1 = b.bytes(b"n");
        b.patch(o1[0].0, n1);
        let (c2, o2) = b.table(&[Some(F::Off(0)), Some(F::U8(11))]);
        b.patch(cols + 8, c2);
        let n2 = b.bytes(b"s");
        b.patch(o2[0].0, n2);
        let (crs, _) = b.table(&[None, Some(F::I32(4326))]);
        b.patch(offs[2].0, crs);
        let mut header = vec![0u8; 4];
        header.extend_from_slice(&b.buf);
        // The root offset counts from the start of the header buffer (4 bytes added).
        header[..4].copy_from_slice(&((root + 4) as u32).to_le_bytes());
        // Shift every absolute offset: they are relative, so only the root needed fixing.
        let mut out = b"fgb\x03fgb\x00".to_vec();
        out.extend_from_slice(&(header.len() as u32).to_le_bytes());
        out.extend_from_slice(&header);
        // Features.
        let mut feats: Vec<Vec<u8>> = Vec::new();
        for (k, p) in pts.iter().enumerate() {
            let mut b = Builder::default();
            let (root, offs) = b.table(&[Some(F::Off(0)), Some(F::Off(0))]);
            let (g, go) = b.table(&[None, Some(F::Off(0))]);
            b.patch(offs[0].0, g);
            let xy = b.f64s(&[p[0], p[1]]);
            b.patch(go[0].0, xy);
            let mut props = Vec::new();
            props.extend_from_slice(&0u16.to_le_bytes());
            props.extend_from_slice(&(k as i32).to_le_bytes());
            props.extend_from_slice(&1u16.to_le_bytes());
            let s = format!("p{k}");
            props.extend_from_slice(&(s.len() as u32).to_le_bytes());
            props.extend_from_slice(s.as_bytes());
            let pr = b.bytes(&props);
            b.patch(offs[1].0, pr);
            let mut f = vec![0u8; 4];
            f.extend_from_slice(&b.buf);
            f[..4].copy_from_slice(&((root + 4) as u32).to_le_bytes());
            let mut sized = (f.len() as u32).to_le_bytes().to_vec();
            sized.extend_from_slice(&f);
            feats.push(sized);
        }
        if node_size > 0 && !pts.is_empty() {
            let levels = level_bounds(pts.len() as u64, u64::from(node_size)).unwrap();
            let total = levels[0].1 as usize;
            let mut nodes = vec![([0f64; 4], 0u64); total];
            let mut off = 0u64;
            let (l0s, _) = levels[0];
            for (k, p) in pts.iter().enumerate() {
                nodes[l0s as usize + k] = ([p[0], p[1], p[0], p[1]], off);
                off += feats[k].len() as u64;
            }
            for lv in 1..levels.len() {
                let (s, e) = levels[lv];
                let (cs, ce) = levels[lv - 1];
                for (j, i) in (s..e).enumerate() {
                    let first = cs + j as u64 * u64::from(node_size);
                    let last = (first + u64::from(node_size)).min(ce);
                    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
                    for c in first..last {
                        let n = nodes[c as usize].0;
                        b = [
                            b[0].min(n[0]),
                            b[1].min(n[1]),
                            b[2].max(n[2]),
                            b[3].max(n[3]),
                        ];
                    }
                    nodes[i as usize] = (b, first);
                }
            }
            for (b, o) in nodes {
                for v in b {
                    out.extend_from_slice(&v.to_le_bytes());
                }
                out.extend_from_slice(&o.to_le_bytes());
            }
        }
        for f in feats {
            out.extend_from_slice(&f);
        }
        out
    }

    #[test]
    fn levels() {
        assert_eq!(level_bounds(1, 16).unwrap(), vec![(1, 2), (0, 1)]);
        assert_eq!(level_bounds(40, 16).unwrap(), vec![(4, 44), (1, 4), (0, 1)]);
    }

    #[test]
    fn reads_points_and_searches_the_index() {
        let pts: Vec<Pt> = (0..100)
            .map(|k| [f64::from(k % 10), f64::from(k / 10)])
            .collect();
        for node_size in [0u16, 2, 16] {
            let data = points_fgb(&pts, node_size);
            assert!(sniff(&data));
            let ds = read(&data, "x").unwrap();
            assert_eq!(ds.format, "FlatGeobuf");
            assert_eq!(ds.crs.as_ref().and_then(|c| c.epsg), Some(4326));
            let l = &ds.layers[0];
            assert_eq!(l.name, "pts");
            assert_eq!(l.fields(), vec!["n", "s"]);
            assert_eq!(l.features.len(), 100);
            assert_eq!(l.features[37].geometry, Some(Geometry::Point([7.0, 3.0])));
            assert_eq!(
                l.features[37].values,
                vec![Value::Int(37), Value::Text("p37".into())]
            );
            let bb = read_bbox(&data, "x", [2.5, 2.5, 4.5, 3.5]).unwrap();
            let got: Vec<_> = bb.layers[0]
                .features
                .iter()
                .map(|f| f.values[0].clone())
                .collect();
            assert_eq!(
                got,
                vec![Value::Int(33), Value::Int(34)],
                "node size {node_size}"
            );
            assert_eq!(Fgb::open(&data).unwrap().has_index(), node_size > 0);
        }
    }

    #[test]
    fn damaged_files_fail_cleanly() {
        let data = points_fgb(&[[1.0, 2.0], [3.0, 4.0]], 16);
        for cut in [5, 12, 40, data.len() - 3] {
            let _ = read(&data[..cut], "x");
        }
        let mut bad = data.clone();
        for b in bad.iter_mut().skip(12).step_by(7) {
            *b ^= 0x5A;
        }
        let _ = read(&bad, "x");
    }
}
