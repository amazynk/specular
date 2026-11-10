//! GeoParquet (1.0 and 1.1) and GeoArrow in Parquet and in Arrow IPC files: the primary
//! geometry column as WKB, WKT or a native encoding, the flat columns as attributes.
//!
//! The geometry column comes from the `geo` metadata (GeoParquet, also written by GDAL
//! into Arrow files), else from a GeoArrow extension type on a field (`ARROW:schema` in
//! Parquet). Without either the file is refused: it has no map. A missing `crs` means
//! OGC:CRS84 (longitude/latitude on WGS 84), as the specification says.

use serde_json::Value as J;

use crate::arrow::{self, Arr, Encoding, Nested};
use crate::crs::{self, Crs};
use crate::model::{Dataset, Layer, MAX_FEATURES, MAX_VERTICES, Value};
use crate::parquet::{self, File, Logical, Physical, Values};

/// The most bytes one column chunk or Arrow buffer may decompress to.
const LIMIT: usize = 1 << 31;

/// The geometry column a file declares.
struct GeoColumn {
    name: String,
    encoding: Encoding,
    crs: Option<Crs>,
    crs_note: Option<String>,
    types: Vec<String>,
}

fn crs_of(v: Option<&J>) -> (Option<Crs>, Option<String>) {
    match v {
        // Absent: OGC:CRS84.
        None => (crs::from_epsg(4326), None),
        Some(J::Null) => (None, Some("undefined (crs is null)".into())),
        Some(o @ J::Object(_)) => {
            let text = o.to_string();
            match crate::wkt::crs_from_projjson(&text) {
                Some(c) => (Some(c), None),
                None => {
                    let id = o
                        .get("id")
                        .map(|i| {
                            format!(
                                "{}:{}",
                                i.get("authority").and_then(J::as_str).unwrap_or("?"),
                                i.get("code")
                                    .map(|c| c.to_string().trim_matches('"').to_string())
                                    .unwrap_or_default()
                            )
                        })
                        .or_else(|| o.get("name").and_then(J::as_str).map(str::to_string));
                    let by_id = id
                        .as_deref()
                        .and_then(crs::epsg_from_ref)
                        .and_then(crs::from_epsg);
                    let note = by_id
                        .is_none()
                        .then(|| id.unwrap_or_else(|| "PROJJSON".into()));
                    (by_id, note)
                }
            }
        }
        Some(J::String(s)) => {
            let c = crs::epsg_from_ref(s)
                .and_then(crs::from_epsg)
                .or_else(|| crate::wkt::crs_from_wkt(s))
                .or_else(|| crate::wkt::crs_from_projjson(s));
            let note = c.is_none().then(|| s.chars().take(80).collect());
            (c, note)
        }
        Some(other) => (None, Some(other.to_string())),
    }
}

/// From the GeoParquet `geo` metadata.
fn from_geo_meta(text: &str) -> Result<Option<GeoColumn>, String> {
    let j: J =
        serde_json::from_str(text).map_err(|e| format!("the geo metadata is not JSON: {e}"))?;
    let cols = j.get("columns").and_then(J::as_object);
    let primary = j
        .get("primary_column")
        .and_then(J::as_str)
        .map(str::to_string)
        .or_else(|| cols.and_then(|c| c.keys().next().cloned()));
    let Some(name) = primary else {
        return Ok(None);
    };
    let c = cols.and_then(|c| c.get(&name));
    let enc_name = c
        .and_then(|c| c.get("encoding"))
        .and_then(J::as_str)
        .unwrap_or("WKB");
    let encoding = Encoding::parse(enc_name)
        .ok_or_else(|| format!("the geometry encoding {enc_name:?} is not supported"))?;
    let (crs, crs_note) = crs_of(c.and_then(|c| c.get("crs")));
    let types = c
        .and_then(|c| c.get("geometry_types"))
        .and_then(J::as_array)
        .map(|a| a.iter().filter_map(J::as_str).map(str::to_string).collect())
        .unwrap_or_default();
    Ok(Some(GeoColumn {
        name,
        encoding,
        crs,
        crs_note,
        types,
    }))
}

/// From a GeoArrow extension type on a top-level field.
fn from_extension(fields: &[arrow::Field]) -> Result<Option<GeoColumn>, String> {
    for f in fields {
        let Some(ext) = f.extension() else { continue };
        if !ext.starts_with("geoarrow.") {
            continue;
        }
        let encoding = Encoding::parse(ext)
            .ok_or_else(|| format!("the GeoArrow type {ext:?} is not supported"))?;
        let meta: Option<J> = f
            .meta("ARROW:extension:metadata")
            .filter(|m| !m.trim().is_empty())
            .and_then(|m| serde_json::from_str(m).ok());
        let (crs, crs_note) = match meta.as_ref().and_then(|m| m.get("crs")) {
            // GeoArrow without a CRS says nothing: unknown, unlike GeoParquet.
            None => (None, Some("not given".into())),
            v => crs_of(v),
        };
        return Ok(Some(GeoColumn {
            name: f.name.clone(),
            encoding,
            crs,
            crs_note,
            types: Vec::new(),
        }));
    }
    Ok(None)
}

fn dataset(format: &'static str, g: &GeoColumn) -> Dataset {
    Dataset {
        format,
        crs: g.crs.clone(),
        crs_note: g.crs_note.clone(),
        ..Dataset::default()
    }
}

// ------------------------------------------------------------------------------ Parquet

fn pq_value(v: &Values, i: usize, phys: Physical, logical: Logical, tz: Option<i32>) -> Value {
    match (v, logical) {
        (Values::Bool(b), _) => Value::Bool(b[i]),
        (Values::I32(x), Logical::Date) => Value::Date(arrow::date_text(i64::from(x[i]))),
        (Values::I64(x), Logical::Date) => Value::Date(arrow::date_text(x[i])),
        (Values::I64(x), Logical::Timestamp(u)) => Value::Date(arrow::timestamp_tz(x[i], u, tz)),
        (Values::I32(x), Logical::Time(u)) | (Values::I32(x), Logical::Timestamp(u)) => {
            let t = arrow::timestamp_text(i64::from(x[i]), u);
            Value::Date(t.split_once('T').map_or(t.clone(), |(_, b)| b.to_string()))
        }
        (Values::I64(x), Logical::Time(u)) => {
            let t = arrow::timestamp_text(x[i], u);
            Value::Date(t.split_once('T').map_or(t.clone(), |(_, b)| b.to_string()))
        }
        (Values::I32(x), Logical::Decimal(s)) => Value::Float(f64::from(x[i]) / 10f64.powi(s)),
        (Values::I64(x), Logical::Decimal(s)) => Value::Float(x[i] as f64 / 10f64.powi(s)),
        (Values::I32(x), Logical::Unsigned) => Value::Int(i64::from(x[i] as u32)),
        (Values::I64(x), Logical::Unsigned) => {
            let u = x[i] as u64;
            i64::try_from(u).map_or(Value::Float(u as f64), Value::Int)
        }
        (Values::I32(x), _) => Value::Int(i64::from(x[i])),
        (Values::I64(x), _) => Value::Int(x[i]),
        (Values::I96(x), _) => {
            let (nanos, jd) = x[i];
            let days = i64::from(jd) - 2_440_588;
            Value::Date(arrow::timestamp_text(
                days * 86_400_000_000 + (nanos / 1000) as i64,
                1_000_000,
            ))
        }
        (Values::F32(x), _) => Value::Float(f64::from(x[i])),
        (Values::F64(x), _) => Value::Float(x[i]),
        (b @ Values::Bytes(..), l) => {
            let s = b.bytes(i).unwrap_or(&[]);
            match (l, phys) {
                (Logical::String | Logical::Json, _) => {
                    Value::Text(String::from_utf8_lossy(s).into_owned())
                }
                (Logical::Decimal(sc), _) => Value::Float(arrow::decimal_be(s, sc)),
                _ => Value::Blob(s.len()),
            }
        }
    }
}

/// A flat column's values, one per row (nulls where undefined).
fn flat(leaf: &parquet::Leaf, col: &parquet::Column, tz: Option<i32>) -> Vec<Value> {
    let mut out = Vec::with_capacity(col.entries);
    let mut vi = 0usize;
    for e in 0..col.entries {
        let defined = leaf.max_def == 0 || col.def.get(e) == Some(&leaf.max_def);
        if defined && vi < col.values.len() {
            out.push(pq_value(&col.values, vi, leaf.physical, leaf.logical, tz));
            vi += 1;
        } else {
            out.push(Value::Null);
        }
    }
    out
}

/// Prefix sums of list lengths: offsets with one more entry.
fn offsets(lens: &[usize]) -> Vec<usize> {
    let mut o = Vec::with_capacity(lens.len() + 1);
    let mut s = 0usize;
    o.push(0);
    for &l in lens {
        s += l;
        o.push(s);
    }
    o
}

/// A native geometry column of one row group as [`Nested`].
fn pq_native(f: &File<'_>, g: usize, leaves: &[usize], enc: Encoding) -> Result<Nested, String> {
    let depth = enc.depth();
    let names: Vec<&str> = leaves
        .iter()
        .map(|&l| f.leaves[l].path.last().map_or("", String::as_str))
        .collect();
    let xi = names.iter().position(|n| *n == "x");
    let yi = names.iter().position(|n| *n == "y");
    if let (Some(xi), Some(yi)) = (xi, yi) {
        let (lx, ly) = (&f.leaves[leaves[xi]], &f.leaves[leaves[yi]]);
        let cx = f.column(g, leaves[xi], LIMIT)?;
        let cy = f.column(g, leaves[yi], LIMIT)?;
        let n = parquet::nest(f, lx, &cx)?;
        if n.lists.len() != depth {
            return Err("the geometry column is not nested as its encoding says".into());
        }
        let mut coords = Vec::with_capacity(n.present.len());
        let mut vi = 0usize;
        // The y column has the same layout; its values line up with x's.
        let _ = ly;
        for &p in &n.present {
            if p {
                coords.push([
                    cx.values.f64(vi).unwrap_or(f64::NAN),
                    cy.values.f64(vi).unwrap_or(f64::NAN),
                ]);
                vi += 1;
            } else {
                coords.push([f64::NAN; 2]);
            }
        }
        return Ok(Nested {
            levels: n.lists.iter().map(|l| offsets(l)).collect(),
            coords,
            row_valid: n.row_null.iter().map(|&b| !b).collect(),
        });
    }
    // Interleaved: one leaf, an extra list level holding each coordinate's values.
    let &[leaf] = leaves else {
        return Err("the geometry column has no x and y".into());
    };
    let l = &f.leaves[leaf];
    let c = f.column(g, leaf, LIMIT)?;
    let n = parquet::nest(f, l, &c)?;
    if n.lists.len() != depth + 1 {
        return Err("the geometry column is not nested as its encoding says".into());
    }
    let mut coords = Vec::new();
    let (mut slot, mut vi) = (0usize, 0usize);
    for &len in &n.lists[depth] {
        let mut v = [f64::NAN; 2];
        for k in 0..len {
            if n.present.get(slot).copied().unwrap_or(false) {
                if let Some(x) = v.get_mut(k) {
                    *x = c.values.f64(vi).unwrap_or(f64::NAN);
                }
                vi += 1;
            }
            slot += 1;
        }
        coords.push(v);
    }
    Ok(Nested {
        levels: n.lists[..depth].iter().map(|l| offsets(l)).collect(),
        coords,
        row_valid: n.row_null.iter().map(|&b| !b).collect(),
    })
}

/// Without GeoParquet metadata or a GeoArrow type, a binary column named like a geometry
/// ("geometry", "geom", "wkb_geometry") is read as WKB, as GDAL does; its CRS is unknown.
fn by_name(f: &File<'_>) -> Option<GeoColumn> {
    let leaf = f.leaves.iter().find(|l| {
        l.path.len() == 1
            && l.max_rep == 0
            && l.physical == Physical::ByteArray
            && matches!(
                l.path[0].to_ascii_lowercase().as_str(),
                "geometry" | "geom" | "wkb_geometry"
            )
    })?;
    Some(GeoColumn {
        name: leaf.path[0].clone(),
        encoding: Encoding::Wkb,
        crs: None,
        crs_note: Some("not given (no GeoParquet metadata)".into()),
        types: Vec::new(),
    })
}

pub fn sniff(head: &[u8]) -> bool {
    parquet::sniff(head)
}

/// Reads a GeoParquet file.
pub fn read(data: &[u8], name: &str) -> Result<Dataset, String> {
    let f = File::open(data)?;
    let g = match f.meta("geo") {
        Some(m) => from_geo_meta(m)?,
        None => None,
    };
    let g = match g {
        Some(g) => g,
        None => {
            let ext = f
                .meta("ARROW:schema")
                .and_then(arrow::schema_from_base64)
                .map(|s| from_extension(&s.fields))
                .transpose()?
                .flatten();
            match ext {
                Some(e) => e,
                None => by_name(&f)
                    .ok_or("this Parquet file has no geometry column (no GeoParquet metadata)")?,
            }
        }
    };
    // Time zones of timestamp columns, from the Arrow schema a writer embedded.
    let zones: Vec<(String, i32)> = f
        .meta("ARROW:schema")
        .and_then(arrow::schema_from_base64)
        .map(|s| {
            s.fields
                .iter()
                .filter_map(|fl| match fl.ty {
                    arrow::Type::Timestamp(_, Some(z)) => Some((fl.name.clone(), z)),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    let gleaves = f.leaves_of(&g.name);
    if gleaves.is_empty() {
        return Err(format!("the geometry column {:?} is missing", g.name));
    }
    let mut ds = dataset("GeoParquet", &g);
    let mut layer = Layer::new(name);
    layer.declared_type = match g.types.as_slice() {
        [t] => Some(t.clone()),
        _ => None,
    };
    // Flat top-level columns are attributes; nested ones are listed as skipped.
    let mut attrs: Vec<(String, usize)> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    for &c in &f.nodes[0].children {
        let n = &f.nodes[c];
        if n.name == g.name {
            continue;
        }
        let ls = f.leaves_of(&n.name);
        match ls.as_slice() {
            [l] if f.leaves[*l].path.len() == 1 && f.leaves[*l].max_rep == 0 => {
                attrs.push((n.name.clone(), *l));
                layer.field(&n.name);
            }
            _ => skipped.push(n.name.clone()),
        }
    }
    // GeoParquet 1.1 bounding-box covering columns are not attributes.
    skipped.retain(|s| s != "bbox");
    if !skipped.is_empty() {
        ds.warn(format!(
            "nested columns are not shown: {}",
            skipped.join(", ")
        ));
    }
    let mut vertices = 0usize;
    'groups: for (gi, rg) in f.row_groups.iter().enumerate() {
        if rg.rows == 0 {
            continue;
        }
        let (geoms, _rows): (Vec<Option<crate::Geometry>>, usize) = match g.encoding {
            Encoding::Wkb | Encoding::Wkt => {
                let leaf = &f.leaves[gleaves[0]];
                let col = f.column(gi, gleaves[0], LIMIT)?;
                let mut out = Vec::with_capacity(col.entries);
                let mut vi = 0usize;
                for e in 0..col.entries {
                    let defined = leaf.max_def == 0 || col.def.get(e) == Some(&leaf.max_def);
                    if !defined {
                        out.push(None);
                        continue;
                    }
                    let b = col.values.bytes(vi).unwrap_or(&[]);
                    vi += 1;
                    out.push(if g.encoding == Encoding::Wkb {
                        crate::gpkg::Wkb::new(b).geometry(0).ok()
                    } else {
                        std::str::from_utf8(b)
                            .ok()
                            .and_then(|s| crate::wkt_geom::parse(s).ok())
                    });
                }
                let n = out.len();
                (out, n)
            }
            enc => {
                let n = pq_native(&f, gi, &gleaves, enc)?;
                let rows = n.row_valid.len();
                ((0..rows).map(|r| n.geometry(enc, r)).collect(), rows)
            }
        };
        // A column that fails keeps its error once (not once per row: the row count is
        // the file's word), shown in every row read.
        let cols: Vec<Result<Vec<Value>, Value>> = attrs
            .iter()
            .map(|(_, l)| {
                f.column(gi, *l, LIMIT)
                    .map(|c| {
                        let leaf = &f.leaves[*l];
                        let tz = zones
                            .iter()
                            .find(|(n, _)| leaf.path.first() == Some(n))
                            .map(|(_, z)| *z);
                        flat(leaf, &c, tz)
                    })
                    .map_err(|e| Value::Text(format!("({e})")))
            })
            .collect();
        for (r, geom) in geoms.into_iter().enumerate() {
            if layer.features.len() >= MAX_FEATURES || vertices > MAX_VERTICES {
                ds.warn("too many features; the rest are not shown");
                break 'groups;
            }
            vertices += geom.as_ref().map_or(0, crate::Geometry::vertex_count);
            let values = cols
                .iter()
                .map(|c| match c {
                    Ok(c) => c.get(r).cloned().unwrap_or(Value::Null),
                    Err(e) => e.clone(),
                })
                .collect();
            let id = layer.features.len().to_string();
            layer.features.push(crate::model::Feature {
                id: Some(id),
                geometry: geom,
                values,
            });
        }
    }
    ds.layers.push(layer);
    Ok(ds)
}

// ------------------------------------------------------------------------- Arrow IPC

/// Reads an Arrow IPC file or stream with a GeoArrow (or `geo`-described) geometry.
pub fn read_ipc(data: &[u8], name: &str) -> Result<Dataset, String> {
    let ipc = arrow::read_ipc(data, LIMIT)?;
    let mut g = match ipc.schema.meta("geo") {
        Some(m) => from_geo_meta(m)?,
        None => None,
    };
    if let Some(gc) = &mut g
        && let Some(f) = ipc.schema.fields.iter().find(|f| f.name == gc.name)
        && let Some(e) = f.extension().and_then(Encoding::parse)
    {
        gc.encoding = e;
    }
    let g = match g {
        Some(g) => g,
        None => from_extension(&ipc.schema.fields)?
            .ok_or("this Arrow file has no geometry column (no GeoArrow type)")?,
    };
    let gi = ipc
        .schema
        .fields
        .iter()
        .position(|f| f.name == g.name)
        .ok_or_else(|| format!("the geometry column {:?} is missing", g.name))?;
    let mut ds = dataset("GeoArrow", &g);
    let mut layer = Layer::new(name);
    let mut attrs = Vec::new();
    let mut skipped = Vec::new();
    for (i, f) in ipc.schema.fields.iter().enumerate() {
        if i == gi {
            continue;
        }
        match f.ty {
            arrow::Type::List
            | arrow::Type::LargeList
            | arrow::Type::FixedList(_)
            | arrow::Type::Struct
            | arrow::Type::Map
            | arrow::Type::Other(_) => skipped.push(f.name.clone()),
            _ => {
                attrs.push(i);
                layer.field(&f.name);
            }
        }
    }
    skipped.retain(|s| s != "bbox");
    if !skipped.is_empty() {
        ds.warn(format!(
            "nested columns are not shown: {}",
            skipped.join(", ")
        ));
    }
    let mut vertices = 0usize;
    'batches: for batch in &ipc.batches {
        let a: &Arr = &batch[gi];
        let nested = match g.encoding {
            Encoding::Wkb | Encoding::Wkt => None,
            e => Some(arrow::nested(a, e.depth())?),
        };
        for r in 0..a.len() {
            if layer.features.len() >= MAX_FEATURES || vertices > MAX_VERTICES {
                ds.warn("too many features; the rest are not shown");
                break 'batches;
            }
            let geom = arrow::geometry(a, g.encoding, nested.as_ref(), r);
            vertices += geom.as_ref().map_or(0, crate::Geometry::vertex_count);
            let values = attrs
                .iter()
                .map(|&i| arrow::value(&batch[i], &ipc.schema.fields[i].ty, r))
                .collect();
            let id = layer.features.len().to_string();
            layer.features.push(crate::model::Feature {
                id: Some(id),
                geometry: geom,
                values,
            });
        }
    }
    ds.layers.push(layer);
    Ok(ds)
}
