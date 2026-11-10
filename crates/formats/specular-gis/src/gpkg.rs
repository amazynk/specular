//! OGC GeoPackage (an SQLite database): feature tables with their GeoPackage geometry
//! blobs (ISO/OGC WKB, extended WKB) and attributes, the spatial reference table (WKT1 and
//! WKT2 definitions), and tile pyramids (their matrices and the tile blobs; the images are
//! decoded by the caller). Read through an [`SqlDb`] (SQLite runs in the sandboxed decode
//! host, never here); the file is never written. A feature layer may be a view (read with the
//! host's view checks, paged by its first integer column); virtual tables are not read.

use crate::crs::{self, Crs, CrsKind};
use crate::model::{
    Dataset, Geometry, Layer, MAX_FEATURES, MAX_VERTICES, Pt, TileLevel, TileSet, Value,
};
use crate::sql::{self, SqlDb, SqlLimits, SqlValue};
use crate::wkt;

/// SQLite's header, and a GeoPackage application id ("GPKG" or the 1.0 "GP10"/"GP11").
pub fn sniff(head: &[u8]) -> bool {
    head.starts_with(b"SQLite format 3\0")
        && head
            .get(68..72)
            .is_some_and(|id| matches!(id, b"GPKG" | b"GP10" | b"GP11"))
}

/// Rows of a feature table per query (paged by rowid).
const PAGE_ROWS: u32 = 50_000;
/// Bytes of one page of features (the host refuses a page past it).
const PAGE_BYTES: u64 = 512 << 20;

fn quote(id: &str) -> String {
    format!("\"{}\"", id.replace('"', "\"\""))
}

fn int(v: &SqlValue) -> Option<i64> {
    v.as_i64()
}

/// The CRS of an `srs_id`, from `gpkg_spatial_ref_sys`.
fn srs(db: &dyn SqlDb, id: i64) -> (Option<Crs>, Option<String>) {
    let has_12_063 = db
        .query(
            "SELECT definition_12_063 FROM gpkg_spatial_ref_sys LIMIT 0",
            &[],
            sql::small(),
        )
        .is_ok();
    let q = if has_12_063 {
        "SELECT srs_name, organization, organization_coordsys_id, definition, definition_12_063 FROM gpkg_spatial_ref_sys WHERE srs_id = ?1"
    } else {
        "SELECT srs_name, organization, organization_coordsys_id, definition, NULL FROM gpkg_spatial_ref_sys WHERE srs_id = ?1"
    };
    let Ok(Some(r)) = sql::row(db, q, &[SqlValue::Int(id)], sql::small()) else {
        return (None, Some(format!("srs_id {id}")));
    };
    let (name, org, code, def, def2) = (
        sql::text(&r[0]),
        sql::text(&r[1]),
        int(&r[2]),
        sql::text(&r[3]),
        sql::text(&r[4]),
    );
    let wkt2 = def2.filter(|d| !d.trim().is_empty() && d.trim() != "undefined");
    let wkt1 = def.filter(|d| !d.trim().is_empty() && d.trim() != "undefined");
    let epsg = org
        .as_deref()
        .filter(|o| o.eq_ignore_ascii_case("EPSG"))
        .and(code)
        .and_then(|c| u32::try_from(c).ok());
    let mut c = epsg
        .and_then(crs::from_epsg)
        .or_else(|| wkt2.as_deref().and_then(wkt::crs_from_wkt))
        .or_else(|| wkt1.as_deref().and_then(wkt::crs_from_wkt));
    if let Some(c) = &mut c {
        if c.wkt.is_none() {
            c.wkt = wkt2.or(wkt1);
        }
        return (Some(c.clone()), None);
    }
    match id {
        -1 => (
            Some(Crs::named("Undefined Cartesian".into(), CrsKind::Other)),
            None,
        ),
        0 => (
            Some(Crs::named(
                "Undefined geographic".into(),
                CrsKind::Geographic,
            )),
            None,
        ),
        _ => (None, name.or(Some(format!("srs_id {id}")))),
    }
}

/// Reads every feature table and lists the tile pyramids.
pub fn read(db: &dyn SqlDb) -> Result<Dataset, String> {
    let mut ds = Dataset {
        format: "GeoPackage",
        ..Dataset::default()
    };
    let contents = sql::rows(
        db,
        "SELECT table_name, data_type, identifier, srs_id FROM gpkg_contents ORDER BY rowid",
        &[],
        sql::small(),
    )
    .map_err(|e| format!("not a GeoPackage (gpkg_contents: {e})"))?;
    let mut vertices = 0usize;
    let mut first_srs: Option<i64> = None;
    for r in contents {
        let (Some(table), Some(kind)) = (sql::text(&r[0]), sql::text(&r[1])) else {
            continue;
        };
        let (ident, srs_id) = (sql::text(&r[2]), int(&r[3]));
        match kind.as_str() {
            "features" => {
                let g = sql::row(
                    db,
                    "SELECT column_name, geometry_type_name, srs_id FROM gpkg_geometry_columns WHERE table_name = ?1",
                    &[SqlValue::Text(table.clone())],
                    sql::small(),
                )
                .map_err(|e| format!("{table}: no geometry column ({e})"))?;
                let Some((col, gtype, gsrs)) =
                    g.and_then(|g| Some((sql::text(&g[0])?, sql::text(&g[1])?, int(&g[2])?)))
                else {
                    return Err(format!("{table}: no geometry column"));
                };
                if first_srs.is_none() {
                    first_srs = Some(gsrs);
                } else if first_srs != Some(gsrs) {
                    ds.warn(
                        "feature tables use different coordinate systems; the first one's is shown",
                    );
                }
                let mut layer =
                    Layer::new(ident.filter(|s| !s.is_empty()).unwrap_or(table.clone()));
                layer.declared_type = Some(gtype);
                if is_view(db, &table) {
                    // A view the host refuses (too complex, or reading what is not read
                    // for safety) leaves out that layer, not the file.
                    match read_table(db, &table, &col, &mut layer, &mut vertices, &mut ds, true) {
                        Ok(()) => ds.layers.push(layer),
                        Err(e) => ds.warn(format!("layer {} is not shown: {e}", layer.name)),
                    }
                } else {
                    read_table(db, &table, &col, &mut layer, &mut vertices, &mut ds, false)?;
                    ds.layers.push(layer);
                }
            }
            "tiles" | "2d-gridded-coverage" => {
                if let Some(t) = tile_set(db, &table, ident, srs_id) {
                    if first_srs.is_none() {
                        first_srs = Some(t.srs_id);
                    }
                    ds.tiles.push(t);
                }
            }
            _ => {}
        }
    }
    if let Some(id) = first_srs {
        let (c, note) = srs(db, id);
        ds.crs = c;
        ds.crs_note = note;
    }
    Ok(ds)
}

/// Whether `table` is a view.
fn is_view(db: &dyn SqlDb, table: &str) -> bool {
    sql::row(
        db,
        "SELECT type FROM sqlite_master WHERE name = ?1 COLLATE NOCASE AND type IN ('table', 'view')",
        &[SqlValue::Text(table.to_string())],
        sql::small(),
    )
    .ok()
    .flatten()
    .and_then(|r| sql::text(&r[0]))
    .is_some_and(|t| t == "view")
}

fn read_table(
    db: &dyn SqlDb,
    table: &str,
    geom_col: &str,
    layer: &mut Layer,
    vertices: &mut usize,
    ds: &mut Dataset,
    view: bool,
) -> Result<(), String> {
    let small = SqlLimits {
        allow_views: view,
        ..sql::small()
    };
    // Columns, their declared types and the integer primary key (the feature id).
    let cols: Vec<(String, String, bool)> = sql::rows(
        db,
        &format!("PRAGMA table_info({})", quote(table)),
        &[],
        small,
    )
    .map_err(|e| format!("{table}: {e}"))?
    .into_iter()
    .filter_map(|r| {
        Some((
            sql::text(&r[1])?,
            sql::text(&r[2]).unwrap_or_default(),
            int(&r[5]).is_some_and(|p| p > 0),
        ))
    })
    .collect();
    if cols.is_empty() {
        return Err(format!("{table}: no such table"));
    }
    // A view has no primary key: GeoPackage gives it an integer id column, first.
    let pk = if view {
        cols.iter()
            .position(|(_, t, _)| t.eq_ignore_ascii_case("INTEGER"))
    } else {
        cols.iter()
            .position(|(_, t, pk)| *pk && t.eq_ignore_ascii_case("INTEGER"))
    };
    let gi = cols
        .iter()
        .position(|(n, _, _)| n.eq_ignore_ascii_case(geom_col));
    let attr_cols: Vec<usize> = (0..cols.len())
        .filter(|&i| Some(i) != pk && Some(i) != gi)
        .collect();
    for &i in &attr_cols {
        layer.field(&cols[i].0);
    }
    let types: Vec<String> = cols.iter().map(|c| c.1.to_ascii_uppercase()).collect();
    let select: Vec<String> = cols.iter().map(|c| quote(&c.0)).collect();
    // Paged by rowid (an INTEGER PRIMARY KEY is the rowid), so no page rereads the ones
    // before it; the rowid comes first. A view has no rowid: paged by its id column, or,
    // without one, read as one page.
    let key = match (view, pk) {
        (false, _) => Some("rowid".to_string()),
        (true, Some(p)) => Some(quote(&cols[p].0)),
        (true, None) => None,
    };
    let sql = match &key {
        Some(k) => format!(
            "SELECT {k}, {} FROM {} WHERE {k} > ?1 ORDER BY {k} LIMIT ?2",
            select.join(", "),
            quote(table)
        ),
        None => format!(
            "SELECT 0, {} FROM {} WHERE ?1 IS NOT NULL LIMIT ?2",
            select.join(", "),
            quote(table)
        ),
    };
    let limits = SqlLimits {
        max_rows: PAGE_ROWS,
        max_bytes: PAGE_BYTES,
        max_value: 256 << 20,
        allow_views: view,
        millis: 25_000,
    };
    let mut after = i64::MIN;
    let mut n = 0usize;
    'pages: loop {
        let page = db
            .query(
                &sql,
                &[SqlValue::Int(after), SqlValue::Int(i64::from(PAGE_ROWS))],
                limits,
            )
            .map_err(|e| format!("{table}: {e}"))?;
        let rows = page.len();
        for mut r in page.into_rows() {
            let Some(rowid) = int(&r[0]) else {
                break 'pages;
            };
            after = rowid;
            let r = &mut r[1..];
            if layer.features.len() >= MAX_FEATURES {
                ds.warn("too many features; the rest are not shown");
                break 'pages;
            }
            let id = match pk.map(|p| &r[p]) {
                Some(SqlValue::Int(i)) => i.to_string(),
                _ => n.to_string(),
            };
            let geometry = match gi.map(|g| &r[g]) {
                Some(SqlValue::Blob(b)) => match gpkg_geometry(b) {
                    Ok(g) => g,
                    Err(e) => {
                        ds.warn(format!("a geometry is unreadable: {e}"));
                        None
                    }
                },
                _ => None,
            };
            if let Some(g) = &geometry {
                *vertices += g.vertex_count();
                if *vertices > MAX_VERTICES {
                    ds.warn("too many vertices; the rest are not shown");
                    break 'pages;
                }
            }
            let values = attr_cols
                .iter()
                .map(|&i| {
                    let ty = &types[i];
                    match std::mem::replace(&mut r[i], SqlValue::Null) {
                        SqlValue::Null => Value::Null,
                        SqlValue::Int(v) if ty == "BOOLEAN" => Value::Bool(v != 0),
                        SqlValue::Int(v) => Value::Int(v),
                        SqlValue::Real(v) => Value::Float(v),
                        SqlValue::Text(s) => {
                            if ty == "DATE" || ty == "DATETIME" {
                                Value::Date(s)
                            } else {
                                Value::Text(s)
                            }
                        }
                        SqlValue::Blob(b) => Value::Blob(b.len()),
                    }
                })
                .collect();
            layer.features.push(crate::model::Feature {
                id: Some(id),
                geometry,
                values,
            });
            n += 1;
        }
        if rows < PAGE_ROWS as usize {
            break;
        }
        if key.is_none() {
            ds.warn(format!(
                "{table}: a view without an id column; only its first {PAGE_ROWS} features are shown"
            ));
            break;
        }
    }
    Ok(())
}

fn tile_set(
    db: &dyn SqlDb,
    table: &str,
    ident: Option<String>,
    srs_id: Option<i64>,
) -> Option<TileSet> {
    let name = [SqlValue::Text(table.to_string())];
    let r = sql::row(
        db,
        "SELECT srs_id, min_x, min_y, max_x, max_y FROM gpkg_tile_matrix_set WHERE table_name = ?1",
        &name,
        sql::small(),
    )
    .ok()??;
    let srs = int(&r[0])?;
    let b = [
        r[1].as_f64()?,
        r[2].as_f64()?,
        r[3].as_f64()?,
        r[4].as_f64()?,
    ];
    let levels = sql::rows(
        db,
        "SELECT zoom_level, matrix_width, matrix_height, tile_width, tile_height, pixel_x_size, pixel_y_size FROM gpkg_tile_matrix WHERE table_name = ?1 ORDER BY zoom_level",
        &name,
        sql::small(),
    )
    .ok()?
    .into_iter()
    .map(|r| {
        Some(TileLevel {
            zoom: int(&r[0])?,
            matrix_w: int(&r[1])?,
            matrix_h: int(&r[2])?,
            tile_w: int(&r[3])?.clamp(1, 65536) as u32,
            tile_h: int(&r[4])?.clamp(1, 65536) as u32,
            pixel_x: r[5].as_f64()?,
            pixel_y: r[6].as_f64()?,
        })
    })
    .collect::<Option<Vec<_>>>()?;
    Some(TileSet {
        table: table.to_string(),
        identifier: ident,
        srs_id: srs_id.unwrap_or(srs),
        bounds: b,
        levels,
    })
}

/// The tiles of one zoom level: (column, row, encoded image), row 0 at the top; at most
/// `max_bytes` of them together.
pub fn tiles(
    db: &dyn SqlDb,
    table: &str,
    zoom: i64,
    max_bytes: u64,
) -> Result<Vec<(i64, i64, Vec<u8>)>, String> {
    let q = format!(
        "SELECT tile_column, tile_row, tile_data FROM {} WHERE zoom_level = ?1",
        quote(table)
    );
    let limits = SqlLimits {
        max_rows: 4_000_000,
        max_bytes,
        max_value: crate::tiles::TILE_LIMIT as u32,
        allow_views: false,
        millis: 25_000,
    };
    Ok(sql::rows(db, &q, &[SqlValue::Int(zoom)], limits)?
        .into_iter()
        .filter_map(|r| {
            let mut it = r.into_iter();
            let (c, row, data) = (it.next()?, it.next()?, it.next()?);
            Some((c.as_i64()?, row.as_i64()?, data.into_bytes()?))
        })
        .collect())
}

/// How many tiles one zoom level has.
pub fn tile_count(db: &dyn SqlDb, table: &str, zoom: i64) -> Result<usize, String> {
    let q = format!(
        "SELECT count(*) FROM {} WHERE zoom_level = ?1",
        quote(table)
    );
    let r = sql::row(db, &q, &[SqlValue::Int(zoom)], sql::small())?;
    Ok(r.and_then(|r| int(&r[0])).unwrap_or(0).max(0) as usize)
}

/// A GeoPackage geometry blob: the `GP` header (with an optional envelope), then WKB.
pub fn gpkg_geometry(b: &[u8]) -> Result<Option<Geometry>, String> {
    if b.len() < 8 || &b[..2] != b"GP" {
        // Some writers store plain WKB.
        return Wkb::new(b).geometry(0).map(Some);
    }
    let flags = b[3];
    if flags & 0x20 != 0 {
        return Err("extended GeoPackage geometry".into());
    }
    if flags & 0x10 != 0 {
        return Ok(None);
    }
    let env = match (flags >> 1) & 7 {
        0 => 0,
        1 => 32,
        2 | 3 => 48,
        4 => 64,
        _ => return Err("bad envelope code".into()),
    };
    let wkb = b.get(8 + env..).ok_or("truncated")?;
    let g = Wkb::new(wkb).geometry(0)?;
    Ok(Some(g))
}

/// A WKB reader: OGC/ISO (Z, M, ZM by thousands) and PostGIS extended WKB (flag bits).
pub struct Wkb<'a> {
    b: &'a [u8],
    o: usize,
    le: bool,
}

impl<'a> Wkb<'a> {
    pub fn new(b: &'a [u8]) -> Wkb<'a> {
        Wkb { b, o: 0, le: true }
    }

    fn u8(&mut self) -> Result<u8, String> {
        let v = *self.b.get(self.o).ok_or("truncated WKB")?;
        self.o += 1;
        Ok(v)
    }

    fn u32(&mut self) -> Result<u32, String> {
        let s: [u8; 4] = self
            .b
            .get(self.o..self.o + 4)
            .ok_or("truncated WKB")?
            .try_into()
            .map_err(|_| "truncated WKB")?;
        self.o += 4;
        Ok(if self.le {
            u32::from_le_bytes(s)
        } else {
            u32::from_be_bytes(s)
        })
    }

    fn f64(&mut self) -> Result<f64, String> {
        let s: [u8; 8] = self
            .b
            .get(self.o..self.o + 8)
            .ok_or("truncated WKB")?
            .try_into()
            .map_err(|_| "truncated WKB")?;
        self.o += 8;
        Ok(if self.le {
            f64::from_le_bytes(s)
        } else {
            f64::from_be_bytes(s)
        })
    }

    fn count(&mut self, item: usize) -> Result<usize, String> {
        let n = self.u32()? as usize;
        if n.saturating_mul(item) > self.b.len().saturating_sub(self.o) {
            return Err("WKB count beyond the data".into());
        }
        Ok(n)
    }

    fn pt(&mut self, dims: usize) -> Result<Pt, String> {
        let x = self.f64()?;
        let y = self.f64()?;
        for _ in 2..dims {
            self.f64()?;
        }
        Ok([x, y])
    }

    fn line(&mut self, dims: usize) -> Result<Vec<Pt>, String> {
        let n = self.count(dims * 8)?;
        (0..n).map(|_| self.pt(dims)).collect()
    }

    fn rings(&mut self, dims: usize) -> Result<Vec<Vec<Pt>>, String> {
        let n = self.count(4)?;
        (0..n).map(|_| self.line(dims)).collect()
    }

    pub fn geometry(&mut self, depth: usize) -> Result<Geometry, String> {
        if depth > 32 {
            return Err("WKB nested too deep".into());
        }
        self.le = match self.u8()? {
            0 => false,
            1 => true,
            _ => return Err("bad WKB byte order".into()),
        };
        let raw = self.u32()?;
        let (mut z, mut m) = (raw & 0x8000_0000 != 0, raw & 0x4000_0000 != 0);
        if raw & 0x2000_0000 != 0 {
            self.u32()?; // EWKB SRID
        }
        let t = raw & 0x0FFF_FFFF;
        let base = t % 1000;
        match t / 1000 {
            1 => z = true,
            2 => m = true,
            3 => (z, m) = (true, true),
            _ => {}
        }
        let dims = 2 + usize::from(z) + usize::from(m);
        let sub = |s: &mut Self, d| -> Result<Vec<Geometry>, String> {
            let n = s.count(5)?;
            (0..n).map(|_| s.geometry(d)).collect()
        };
        Ok(match base {
            1 => {
                let p = self.pt(dims)?;
                if p[0].is_nan() && p[1].is_nan() {
                    Geometry::MultiPoint(Vec::new())
                } else {
                    Geometry::Point(p)
                }
            }
            2 => Geometry::LineString(self.line(dims)?),
            3 | 17 => Geometry::Polygon(self.rings(dims)?),
            4 => Geometry::MultiPoint(
                sub(self, depth + 1)?
                    .into_iter()
                    .filter_map(|g| match g {
                        Geometry::Point(p) => Some(p),
                        _ => None,
                    })
                    .collect(),
            ),
            5 | 11 => Geometry::MultiLineString(
                sub(self, depth + 1)?
                    .into_iter()
                    .filter_map(|g| match g {
                        Geometry::LineString(l) => Some(l),
                        _ => None,
                    })
                    .collect(),
            ),
            6 | 12 | 15 | 16 => Geometry::MultiPolygon(
                sub(self, depth + 1)?
                    .into_iter()
                    .filter_map(|g| match g {
                        Geometry::Polygon(p) => Some(p),
                        _ => None,
                    })
                    .collect(),
            ),
            7 => Geometry::Collection(sub(self, depth + 1)?),
            // Curves: their control points as lines (an approximation, warned by type).
            8 => Geometry::LineString(self.line(dims)?),
            9 | 10 => Geometry::Collection(sub(self, depth + 1)?),
            _ => return Err(format!("unsupported WKB type {raw}")),
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::path::Path;

    pub fn wkb_polygon(rings: &[&[Pt]], z: bool) -> Vec<u8> {
        let mut b = vec![1u8];
        b.extend_from_slice(&(if z { 1003u32 } else { 3u32 }).to_le_bytes());
        b.extend_from_slice(&(rings.len() as u32).to_le_bytes());
        for r in rings {
            b.extend_from_slice(&(r.len() as u32).to_le_bytes());
            for p in *r {
                b.extend_from_slice(&p[0].to_le_bytes());
                b.extend_from_slice(&p[1].to_le_bytes());
                if z {
                    b.extend_from_slice(&7.0f64.to_le_bytes());
                }
            }
        }
        b
    }

    pub fn gp_blob(srs: i32, wkb: &[u8]) -> Vec<u8> {
        let mut b = b"GP\0\x03".to_vec(); // envelope [minx,maxx,miny,maxy], little-endian
        b.extend_from_slice(&srs.to_le_bytes());
        for v in [0.0f64, 1.0, 0.0, 1.0] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(wkb);
        b
    }

    /// A small GeoPackage with one feature table, written with rusqlite.
    pub fn build(path: &Path) {
        let _ = std::fs::remove_file(path);
        let c = Connection::open(path).unwrap();
        c.execute_batch(
            r#"PRAGMA application_id = 1196444487;
            CREATE TABLE gpkg_spatial_ref_sys (srs_name TEXT NOT NULL, srs_id INTEGER PRIMARY KEY, organization TEXT NOT NULL, organization_coordsys_id INTEGER NOT NULL, definition TEXT NOT NULL, description TEXT);
            INSERT INTO gpkg_spatial_ref_sys VALUES ('WGS 84 / UTM zone 32N', 32632, 'EPSG', 32632, 'undefined', NULL);
            CREATE TABLE gpkg_contents (table_name TEXT NOT NULL PRIMARY KEY, data_type TEXT NOT NULL, identifier TEXT, description TEXT DEFAULT '', last_change DATETIME, min_x DOUBLE, min_y DOUBLE, max_x DOUBLE, max_y DOUBLE, srs_id INTEGER);
            INSERT INTO gpkg_contents (table_name, data_type, identifier, srs_id) VALUES ('parcels', 'features', 'Parcels', 32632);
            CREATE TABLE gpkg_geometry_columns (table_name TEXT NOT NULL, column_name TEXT NOT NULL, geometry_type_name TEXT NOT NULL, srs_id INTEGER NOT NULL, z TINYINT NOT NULL, m TINYINT NOT NULL);
            INSERT INTO gpkg_geometry_columns VALUES ('parcels', 'geom', 'POLYGON', 32632, 1, 0);
            CREATE TABLE parcels (fid INTEGER PRIMARY KEY AUTOINCREMENT, geom POLYGON, name TEXT, ok BOOLEAN, area REAL, day DATE);"#,
        )
        .unwrap();
        let ring: &[Pt] = &[
            [500000.0, 5000000.0],
            [500100.0, 5000000.0],
            [500100.0, 5000100.0],
            [500000.0, 5000000.0],
        ];
        let blob = gp_blob(32632, &wkb_polygon(&[ring], true));
        c.execute(
            "INSERT INTO parcels (fid, geom, name, ok, area, day) VALUES (5, ?1, 'Ä', 1, 2.5, '2024-02-03')",
            [&blob],
        )
        .unwrap();
        c.execute(
            "INSERT INTO parcels (fid, geom, name) VALUES (9, NULL, NULL)",
            [],
        )
        .unwrap();
    }

    /// A feature view is read as a layer, paged by its id column (more rows than one page);
    /// a view that cannot be read leaves out that layer with a warning, not the file.
    #[test]
    fn feature_view() {
        let p = std::env::temp_dir().join(format!("gis-view-{}.gpkg", std::process::id()));
        crate::fixtures::geopackage_with_view(&p, 2 * PAGE_ROWS as usize + 10);
        let d = read(&crate::sql::local::LocalDb::open(&p)).unwrap();
        assert_eq!(d.layers.len(), 2);
        let v = &d.layers[1];
        assert_eq!(v.name, "Even places");
        assert_eq!(v.features.len(), PAGE_ROWS as usize + 5);
        assert_eq!(v.fields(), ["name"]);
        assert_eq!(v.features[0].id.as_deref(), Some("2"));
        assert!(v.features.iter().all(|f| f.geometry.is_some()));
        // The view's text broken: the table layer still reads.
        let c = rusqlite::Connection::open(&p).unwrap();
        c.execute_batch("DROP VIEW even; CREATE VIEW even AS SELECT * FROM missing;")
            .unwrap();
        drop(c);
        let d = read(&crate::sql::local::LocalDb::open(&p)).unwrap();
        let _ = std::fs::remove_file(&p);
        assert_eq!(d.layers.len(), 1);
        assert!(
            d.warnings.iter().any(|w| w.contains("Even places")),
            "{:?}",
            d.warnings
        );
    }

    #[test]
    fn feature_table() {
        // TMPDIR points into the build tree for tests.
        let p = std::env::temp_dir().join(format!("gis-test-{}.gpkg", std::process::id()));
        build(&p);
        let head = std::fs::read(&p).unwrap();
        assert!(sniff(&head));
        let d = read(&crate::sql::local::LocalDb::open(&p)).unwrap();
        let _ = std::fs::remove_file(&p);
        assert_eq!(d.crs.as_ref().unwrap().epsg, Some(32632));
        let l = &d.layers[0];
        assert_eq!(l.name, "Parcels");
        assert_eq!(l.fields(), ["name", "ok", "area", "day"]);
        assert_eq!(l.features[0].id.as_deref(), Some("5"));
        assert_eq!(
            l.features[0].values,
            [
                Value::Text("Ä".into()),
                Value::Bool(true),
                Value::Float(2.5),
                Value::Date("2024-02-03".into())
            ]
        );
        match l.features[0].geometry.as_ref().unwrap() {
            Geometry::Polygon(r) => assert_eq!(r[0][1], [500100.0, 5000000.0]),
            g => panic!("{g:?}"),
        }
        assert_eq!(l.features[1].geometry, None);
    }
}
