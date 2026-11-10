//! MBTiles (1.0–1.3): an SQLite database of map tiles, `tiles(zoom_level, tile_column,
//! tile_row, tile_data)` with rows counted from the south (TMS), and a `metadata` table of
//! name/value pairs (format, bounds, zooms, attribution, vector layers as JSON).
//!
//! Read-only, through an [`SqlDb`] (SQLite runs in the sandboxed decode host, never here).
//! `tiles` and `metadata` may be views (many writers deduplicate tiles that way), so those
//! queries allow views; vector tiles are often gzip-compressed and come back uncompressed.

use crate::sql::{self, SqlDb, SqlLimits, SqlValue};
use crate::tiles::{self, TILE_LIMIT, TileFormat, TileMeta, TileStore};

/// Whether the first bytes are an MBTiles database (SQLite with the application id
/// "MPBX" that MBTiles 1.3 sets; older files are recognised by their name and tables).
pub fn sniff(head: &[u8]) -> bool {
    head.starts_with(b"SQLite format 3\0") && head.get(68..72) == Some(b"MPBX")
}

pub struct MbTiles {
    db: Box<dyn SqlDb>,
    meta: TileMeta,
}

/// Queries on `tiles` and `metadata`, which may be views.
fn viewable(max_rows: u32, max_bytes: u64) -> SqlLimits {
    SqlLimits {
        max_rows,
        max_bytes,
        max_value: TILE_LIMIT as u32,
        allow_views: true,
        millis: 10_000,
    }
}

impl MbTiles {
    pub fn open(db: Box<dyn SqlDb>) -> Result<MbTiles, String> {
        let has = |name: &str| -> bool {
            sql::row(
                db.as_ref(),
                "SELECT count(*) FROM sqlite_master WHERE name = ?1 AND type IN ('table','view')",
                &[SqlValue::Text(name.into())],
                sql::small(),
            )
            .ok()
            .flatten()
            .and_then(|r| r[0].as_i64())
            .is_some_and(|n| n > 0)
        };
        if !has("tiles") {
            return Err("not an MBTiles file: it has no tiles table".into());
        }
        let mut meta = TileMeta {
            container: "MBTiles",
            ..TileMeta::default()
        };
        let mut format = None;
        let mut json = None;
        if has("metadata") {
            let rows = sql::rows(
                db.as_ref(),
                "SELECT name, value FROM metadata",
                &[],
                viewable(1000, 64 << 20),
            )?;
            for r in rows {
                let (Some(k), Some(v)) = (sql::text(&r[0]), r[1].to_text()) else {
                    continue;
                };
                match k.as_str() {
                    "format" => format = Some(TileFormat::from_name(&v)),
                    "name" => meta.name = Some(v),
                    "description" => meta.description = Some(tiles::plain_text(&v)),
                    "attribution" => meta.attribution = Some(tiles::plain_text(&v)),
                    "minzoom" => meta.min_zoom = v.trim().parse().unwrap_or(0),
                    "maxzoom" => meta.max_zoom = v.trim().parse().unwrap_or(0),
                    "bounds" => {
                        let b: Vec<f64> =
                            v.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                        if b.len() == 4 && b.iter().all(|x| x.is_finite()) && b[0] < b[2] {
                            meta.bounds = [
                                b[0].max(-180.0),
                                b[1].max(tiles::WORLD[1]),
                                b[2].min(180.0),
                                b[3].min(tiles::WORLD[3]),
                            ];
                        }
                    }
                    "json" => json = Some(v),
                    _ => {
                        if v.len() < 500 {
                            meta.extra.push((k, v));
                        }
                    }
                }
            }
        }
        if let Some(j) = &json {
            meta.vector_layers = tiles::vector_layer_names(j);
        }
        // Zoom range from the tiles when the metadata leaves it out (an index makes it quick).
        let zooms = sql::row(
            db.as_ref(),
            "SELECT min(zoom_level), max(zoom_level) FROM tiles",
            &[],
            viewable(1, 1 << 10),
        )?
        .and_then(|r| Some((r[0].as_i64()?, r[1].as_i64()?)));
        let Some((zmin, zmax)) = zooms else {
            return Err("the MBTiles file has no tiles".into());
        };
        // Counting scans the whole table: a short deadline, and no count past it.
        meta.tiles = sql::row(
            db.as_ref(),
            "SELECT count(*) FROM tiles",
            &[],
            SqlLimits {
                millis: 2_000,
                ..viewable(1, 1 << 10)
            },
        )
        .ok()
        .flatten()
        .and_then(|r| r[0].as_i64())
        .map(|n| n.max(0) as u64);
        if meta.max_zoom == 0 || meta.max_zoom as i64 > zmax || (meta.min_zoom as i64) < zmin {
            meta.min_zoom = zmin.clamp(0, 30) as u8;
            meta.max_zoom = zmax.clamp(0, 30) as u8;
        }
        let mut mb = MbTiles { db, meta };
        mb.meta.format = match format {
            Some(f) if f != TileFormat::Unknown => f,
            _ => {
                // Older files leave `format` out: look at a tile.
                let first = sql::row(
                    mb.db.as_ref(),
                    "SELECT tile_data FROM tiles LIMIT 1",
                    &[],
                    viewable(1, TILE_LIMIT as u64 + 16),
                )
                .ok()
                .flatten()
                .and_then(|r| r.into_iter().next()?.into_bytes());
                match first {
                    Some(b) => match TileFormat::sniff(&b) {
                        TileFormat::Unknown if json.is_some() => TileFormat::Mvt,
                        f => f,
                    },
                    None => TileFormat::Unknown,
                }
            }
        };
        Ok(mb)
    }
}

fn unpack(fmt: TileFormat, b: Vec<u8>) -> Result<Vec<u8>, String> {
    if fmt == TileFormat::Mvt && (b.starts_with(&[0x1F, 0x8B]) || b.starts_with(&[0x78])) {
        tiles::gunzip(&b, TILE_LIMIT)
    } else {
        Ok(b)
    }
}

impl TileStore for MbTiles {
    fn meta(&self) -> &TileMeta {
        &self.meta
    }

    fn tile(&self, z: u8, x: u32, y: u32) -> Result<Option<Vec<u8>>, String> {
        let row = (1i64 << z.min(30)) - 1 - i64::from(y);
        let b = sql::row(
            self.db.as_ref(),
            "SELECT tile_data FROM tiles WHERE zoom_level = ?1 AND tile_column = ?2 AND tile_row = ?3",
            &[
                SqlValue::Int(i64::from(z)),
                SqlValue::Int(i64::from(x)),
                SqlValue::Int(row),
            ],
            viewable(1, TILE_LIMIT as u64 + 16),
        )?
        .and_then(|r| r.into_iter().next()?.into_bytes());
        b.map(|b| unpack(self.meta.format, b)).transpose()
    }

    fn tiles_in(
        &self,
        z: u8,
        (x0, y0, x1, y1): (u32, u32, u32, u32),
        max: usize,
    ) -> Result<Vec<(u32, u32, Vec<u8>)>, String> {
        let n = 1i64 << z.min(30);
        let max = u32::try_from(max).unwrap_or(u32::MAX);
        let rows = sql::rows(
            self.db.as_ref(),
            "SELECT tile_column, tile_row, tile_data FROM tiles WHERE zoom_level = ?1 \
             AND tile_column BETWEEN ?2 AND ?3 AND tile_row BETWEEN ?4 AND ?5 LIMIT ?6",
            &[
                SqlValue::Int(i64::from(z)),
                SqlValue::Int(i64::from(x0)),
                SqlValue::Int(i64::from(x1)),
                SqlValue::Int(n - 1 - i64::from(y1)),
                SqlValue::Int(n - 1 - i64::from(y0)),
                SqlValue::Int(i64::from(max)),
            ],
            viewable(max, 512 << 20),
        )?;
        let mut out = Vec::new();
        for r in rows {
            let mut it = r.into_iter();
            let (Some(c), Some(r), Some(b)) = (it.next(), it.next(), it.next()) else {
                continue;
            };
            let (Some(c), Some(r), Some(b)) = (c.as_i64(), r.as_i64(), b.into_bytes()) else {
                continue;
            };
            let (Ok(x), Ok(y)) = (u32::try_from(c), u32::try_from(n - 1 - r)) else {
                continue;
            };
            out.push((x, y, unpack(self.meta.format, b)?));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_small_file() {
        let dir = std::env::temp_dir().join(format!("specular-mbtiles-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.mbtiles");
        let _ = std::fs::remove_file(&path);
        crate::fixtures::mbtiles(&path, TileFormat::Png, 2);
        let mb = MbTiles::open(Box::new(crate::sql::local::LocalDb::open(&path))).unwrap();
        assert_eq!(mb.meta().format, TileFormat::Png);
        assert_eq!(mb.meta().tiles, Some(21));
        assert_eq!((mb.meta().min_zoom, mb.meta().max_zoom), (0, 2));
        assert_eq!(
            mb.meta().attribution.as_deref(),
            Some("© Test contributors")
        );
        // XYZ tile (2, 1, 0) is TMS row 3.
        let t = mb.tile(2, 1, 0).unwrap().unwrap();
        assert_eq!(t, crate::fixtures::tile_png(2, 1, 0));
        let all = mb.tiles_in(2, (0, 0, 3, 3), 100).unwrap();
        assert_eq!(all.len(), 16);
        assert!(
            all.iter()
                .any(|(x, y, b)| (*x, *y) == (3, 2) && *b == crate::fixtures::tile_png(2, 3, 2))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
