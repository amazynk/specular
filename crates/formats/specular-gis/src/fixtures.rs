//! Synthetic geodata for tests, goldens and timings: a tiled, deflated Float32 GeoTIFF with
//! overviews (a cloud-optimised layout) and a polygon Shapefile with its sidecars.

use std::io::Write;

/// A north-up georeference: the map coordinates of the top-left corner of the top-left
/// pixel and the pixel size, in the units of `epsg`.
#[derive(Clone, Copy, Debug)]
pub struct GeoGrid {
    pub epsg: u16,
    pub origin: (f64, f64),
    pub pixel: (f64, f64),
}

struct Entry {
    tag: u16,
    typ: u16,
    count: u32,
    data: Vec<u8>,
}

fn shorts(tag: u16, v: &[u16]) -> Entry {
    Entry {
        tag,
        typ: 3,
        count: v.len() as u32,
        data: v.iter().flat_map(|x| x.to_le_bytes()).collect(),
    }
}

fn longs(tag: u16, v: &[u32]) -> Entry {
    Entry {
        tag,
        typ: 4,
        count: v.len() as u32,
        data: v.iter().flat_map(|x| x.to_le_bytes()).collect(),
    }
}

fn doubles(tag: u16, v: &[f64]) -> Entry {
    Entry {
        tag,
        typ: 12,
        count: v.len() as u32,
        data: v.iter().flat_map(|x| x.to_le_bytes()).collect(),
    }
}

fn ascii(tag: u16, s: &str) -> Entry {
    let mut data = s.as_bytes().to_vec();
    data.push(0);
    Entry {
        tag,
        typ: 2,
        count: data.len() as u32,
        data,
    }
}

/// Writes one IFD at the end of `f` (word aligned) with its out-of-line values after it;
/// returns the position of its next-IFD pointer.
fn write_ifd(f: &mut Vec<u8>, mut entries: Vec<Entry>) -> usize {
    if f.len() % 2 == 1 {
        f.push(0);
    }
    entries.sort_by_key(|e| e.tag);
    let start = f.len();
    let extra_at = start + 2 + 12 * entries.len() + 4;
    let mut extra = Vec::new();
    f.extend((entries.len() as u16).to_le_bytes());
    for e in &entries {
        f.extend(e.tag.to_le_bytes());
        f.extend(e.typ.to_le_bytes());
        f.extend(e.count.to_le_bytes());
        if e.data.len() <= 4 {
            let mut v = e.data.clone();
            v.resize(4, 0);
            f.extend(v);
        } else {
            f.extend(((extra_at + extra.len()) as u32).to_le_bytes());
            extra.extend(&e.data);
            if extra.len() % 2 == 1 {
                extra.push(0);
            }
        }
    }
    let next = f.len();
    f.extend([0; 4]);
    f.extend(extra);
    next
}

/// Half size, the mean of the finite values of each 2×2 block (NaN when none).
fn halve(v: &[f32], w: usize, h: usize) -> (Vec<f32>, usize, usize) {
    let (ow, oh) = (w.div_ceil(2), h.div_ceil(2));
    let mut out = vec![f32::NAN; ow * oh];
    for y in 0..oh {
        for x in 0..ow {
            let (mut s, mut n) = (0f64, 0u32);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (sx, sy) = (2 * x + dx, 2 * y + dy);
                if sx < w && sy < h {
                    let q = v[sy * w + sx];
                    if q.is_finite() {
                        s += f64::from(q);
                        n += 1;
                    }
                }
            }
            if n > 0 {
                out[y * ow + x] = (s / f64::from(n)) as f32;
            }
        }
    }
    (out, ow, oh)
}

/// A Float32 GeoTIFF of `w` × `h` values (row-major, top row first): deflated `tile` ×
/// `tile` tiles, overviews halving down to one tile, the GeoKeys of `grid` and, when given,
/// GDAL_NODATA (NaN values are written as that value).
pub fn geotiff_f32(
    values: &[f32],
    w: usize,
    h: usize,
    grid: GeoGrid,
    nodata: Option<f32>,
    tile: usize,
) -> Vec<u8> {
    assert_eq!(values.len(), w * h);
    let mut levels: Vec<(Vec<f32>, usize, usize)> = vec![(values.to_vec(), w, h)];
    while {
        let (_, lw, lh) = levels.last().unwrap();
        *lw > tile || *lh > tile
    } {
        let (v, lw, lh) = levels.last().unwrap();
        let next = halve(v, *lw, *lh);
        levels.push(next);
    }
    let mut f = b"II*\0\0\0\0\0".to_vec();
    let mut tiles_of = Vec::new();
    for (v, lw, lh) in &levels {
        let (tx, ty) = (lw.div_ceil(tile), lh.div_ceil(tile));
        let mut offsets = Vec::with_capacity(tx * ty);
        let mut counts = Vec::with_capacity(tx * ty);
        let mut raw = vec![0u8; tile * tile * 4];
        for r in 0..ty {
            for c in 0..tx {
                raw.fill(0);
                for y in 0..tile {
                    let sy = r * tile + y;
                    if sy >= *lh {
                        break;
                    }
                    for x in 0..tile {
                        let sx = c * tile + x;
                        if sx >= *lw {
                            break;
                        }
                        let mut q = v[sy * lw + sx];
                        if let (Some(nd), false) = (nodata, q.is_finite()) {
                            q = nd;
                        }
                        let o = (y * tile + x) * 4;
                        raw[o..o + 4].copy_from_slice(&q.to_le_bytes());
                    }
                }
                let mut z =
                    flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
                z.write_all(&raw).expect("in memory");
                let data = z.finish().expect("in memory");
                offsets.push(f.len() as u32);
                counts.push(data.len() as u32);
                f.extend(data);
            }
        }
        tiles_of.push((offsets, counts));
    }
    let mut prev: Option<usize> = None;
    for (i, ((_, lw, lh), (offsets, counts))) in levels.iter().zip(tiles_of).enumerate() {
        let mut e = vec![
            longs(254, &[u32::from(i > 0)]),
            longs(256, &[*lw as u32]),
            longs(257, &[*lh as u32]),
            shorts(258, &[32]),
            shorts(259, &[8]),
            shorts(262, &[1]),
            shorts(277, &[1]),
            shorts(284, &[1]),
            shorts(322, &[tile as u16]),
            shorts(323, &[tile as u16]),
            longs(324, &offsets),
            longs(325, &counts),
            shorts(339, &[3]),
        ];
        if i == 0 {
            e.push(doubles(33550, &[grid.pixel.0, grid.pixel.1, 0.0]));
            e.push(doubles(
                33922,
                &[0.0, 0.0, 0.0, grid.origin.0, grid.origin.1, 0.0],
            ));
            let geographic = grid.epsg == 4326 || (4000..5000).contains(&grid.epsg);
            let model = if geographic { 2 } else { 1 };
            let crs_key = if geographic { 2048 } else { 3072 };
            e.push(shorts(
                34735,
                &[
                    1, 1, 0, 3, 1024, 0, 1, model, 1025, 0, 1, 1, crs_key, 0, 1, grid.epsg,
                ],
            ));
            if let Some(nd) = nodata {
                e.push(ascii(42113, &format!("{nd}")));
            }
        }
        let start = f.len() + f.len() % 2;
        match prev {
            None => f[4..8].copy_from_slice(&(start as u32).to_le_bytes()),
            Some(p) => f[p..p + 4].copy_from_slice(&(start as u32).to_le_bytes()),
        }
        prev = Some(write_ifd(&mut f, e));
    }
    f
}

/// An elevation surface for fixtures: hills over a slope, metres; `nan_corner` leaves the
/// top-left eighth undefined.
pub fn dem_values(w: usize, h: usize, nan_corner: bool) -> Vec<f32> {
    let mut v = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let (u, t) = (x as f64 / w as f64, y as f64 / h as f64);
            if nan_corner && u + t < 0.25 {
                v.push(f32::NAN);
                continue;
            }
            let z = 400.0
                + 600.0 * (1.0 - t)
                + 250.0 * (u * 9.0).sin() * (t * 7.0).cos()
                + 120.0 * ((u * 31.0).sin() + (t * 23.0).sin());
            v.push(z as f32);
        }
    }
    v
}

/// One polygon feature: rings (the first the outer one, the rest holes) and two
/// attributes.
pub struct PolygonFeature {
    pub rings: Vec<Vec<[f64; 2]>>,
    pub id: i64,
    pub name: String,
}

/// The four files of a polygon Shapefile.
pub struct ShapefileParts {
    pub shp: Vec<u8>,
    pub shx: Vec<u8>,
    pub dbf: Vec<u8>,
    pub prj: String,
}

fn ring_area2(r: &[[f64; 2]]) -> f64 {
    r.windows(2)
        .map(|w| w[0][0] * w[1][1] - w[1][0] * w[0][1])
        .sum()
}

/// A Shapefile of polygons with an `ID` (numeric) and `NAME` (text) attribute; outer rings
/// are written clockwise and holes counter-clockwise, as the format wants. `prj` is the WKT.
pub fn shapefile_polygons(features: &[PolygonFeature], prj: &str) -> ShapefileParts {
    let mut bbox = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    let mut records = Vec::new();
    for f in features {
        let mut rings = f.rings.clone();
        for (i, r) in rings.iter_mut().enumerate() {
            if r.first() != r.last() {
                let p = r[0];
                r.push(p);
            }
            let clockwise = ring_area2(r) < 0.0;
            if clockwise != (i == 0) {
                r.reverse();
            }
        }
        let pts: Vec<[f64; 2]> = rings.iter().flatten().copied().collect();
        let mut b = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        for p in &pts {
            b = [
                b[0].min(p[0]),
                b[1].min(p[1]),
                b[2].max(p[0]),
                b[3].max(p[1]),
            ];
        }
        bbox = [
            bbox[0].min(b[0]),
            bbox[1].min(b[1]),
            bbox[2].max(b[2]),
            bbox[3].max(b[3]),
        ];
        let mut c = Vec::new();
        c.extend(5i32.to_le_bytes());
        for v in b {
            c.extend(v.to_le_bytes());
        }
        c.extend((rings.len() as i32).to_le_bytes());
        c.extend((pts.len() as i32).to_le_bytes());
        let mut start = 0i32;
        for r in &rings {
            c.extend(start.to_le_bytes());
            start += r.len() as i32;
        }
        for p in &pts {
            c.extend(p[0].to_le_bytes());
            c.extend(p[1].to_le_bytes());
        }
        records.push(c);
    }
    let header = |words: usize| {
        let mut h = vec![0u8; 100];
        h[0..4].copy_from_slice(&9994i32.to_be_bytes());
        h[24..28].copy_from_slice(&(words as i32).to_be_bytes());
        h[28..32].copy_from_slice(&1000i32.to_le_bytes());
        h[32..36].copy_from_slice(&5i32.to_le_bytes());
        for (i, v) in bbox.iter().enumerate() {
            h[36 + 8 * i..44 + 8 * i].copy_from_slice(&v.to_le_bytes());
        }
        h
    };
    let shp_len: usize = 100 + records.iter().map(|r| 8 + r.len()).sum::<usize>();
    let mut shp = header(shp_len / 2);
    let mut shx = header((100 + 8 * records.len()) / 2);
    for (i, r) in records.iter().enumerate() {
        shx.extend(((shp.len() / 2) as i32).to_be_bytes());
        shx.extend(((r.len() / 2) as i32).to_be_bytes());
        shp.extend((i as i32 + 1).to_be_bytes());
        shp.extend(((r.len() / 2) as i32).to_be_bytes());
        shp.extend(r);
    }
    // DBF: ID N(10), NAME C(40).
    let fields: [(&str, u8, u8); 2] = [("ID", b'N', 10), ("NAME", b'C', 40)];
    let rec_len = 1 + fields.iter().map(|f| usize::from(f.2)).sum::<usize>();
    let mut dbf = vec![0x03, 126, 10, 11];
    dbf.extend((features.len() as u32).to_le_bytes());
    dbf.extend(((32 + 32 * fields.len() + 1) as u16).to_le_bytes());
    dbf.extend((rec_len as u16).to_le_bytes());
    dbf.extend([0u8; 20]);
    for (name, typ, len) in fields {
        let mut d = [0u8; 32];
        d[..name.len()].copy_from_slice(name.as_bytes());
        d[11] = typ;
        d[16] = len;
        dbf.extend(d);
    }
    dbf.push(0x0D);
    for f in features {
        dbf.push(b' ');
        dbf.extend(format!("{:>10}", f.id).bytes().take(10));
        let mut name: Vec<u8> = f.name.bytes().take(40).collect();
        name.resize(40, b' ');
        dbf.extend(name);
    }
    dbf.push(0x1A);
    ShapefileParts {
        shp,
        shx,
        dbf,
        prj: prj.to_string(),
    }
}

/// The WKT1 (Esri flavour) of WGS 84 / UTM zone 33N.
pub const UTM33N_WKT: &str = r#"PROJCS["WGS_1984_UTM_Zone_33N",GEOGCS["GCS_WGS_1984",DATUM["D_WGS_1984",SPHEROID["WGS_1984",6378137.0,298.257223563]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]],PROJECTION["Transverse_Mercator"],PARAMETER["False_Easting",500000.0],PARAMETER["False_Northing",0.0],PARAMETER["Central_Meridian",15.0],PARAMETER["Scale_Factor",0.9996],PARAMETER["Latitude_Of_Origin",0.0],UNIT["Meter",1.0]]"#;

/// `n` × `n` square parcels of `size` metres from `origin` (UTM), every third with a
/// square courtyard (a hole), each outline with `detail` points per side: a cadastre-like
/// polygon layer.
pub fn parcels(n: usize, size: f64, detail: usize, origin: (f64, f64)) -> Vec<PolygonFeature> {
    let mut out = Vec::with_capacity(n * n);
    let side = |x0: f64, y0: f64, s: f64| {
        let mut r = Vec::with_capacity(4 * detail + 1);
        let k = detail.max(1);
        for (ax, ay, bx, by) in [
            (0., 0., 1., 0.),
            (1., 0., 1., 1.),
            (1., 1., 0., 1.),
            (0., 1., 0., 0.),
        ] {
            for i in 0..k {
                let t = i as f64 / k as f64;
                // A gentle wobble so outlines are not perfectly straight.
                let wob = if i == 0 {
                    0.0
                } else {
                    (t * 6.0).sin() * s * 0.01
                };
                let x = ax + (bx - ax) * t;
                let y = ay + (by - ay) * t;
                r.push([x0 + x * s + wob * (by - ay), y0 + y * s + wob * (ax - bx)]);
            }
        }
        r.push(r[0]);
        r
    };
    for j in 0..n {
        for i in 0..n {
            let (x0, y0) = (origin.0 + i as f64 * size, origin.1 + j as f64 * size);
            let s = size * 0.9;
            let mut rings = vec![side(x0, y0, s)];
            let id = (j * n + i) as i64;
            if id % 3 == 0 {
                rings.push(side(x0 + s * 0.3, y0 + s * 0.3, s * 0.4));
            }
            out.push(PolygonFeature {
                rings,
                id,
                name: format!("Parcel {id}"),
            });
        }
    }
    out
}

/// A 256 × 256 PNG tile, a flat colour from its address with a dark one-pixel frame (so a
/// mosaic shows its tile edges).
pub fn tile_png(z: u8, x: u32, y: u32) -> Vec<u8> {
    let colour = [
        (40 + 50 * u32::from(z)) as u8,
        (x * 67 % 200 + 40) as u8,
        (y * 89 % 200 + 40) as u8,
    ];
    png_rgb(256, 256, |px, py| {
        if px == 0 || py == 0 {
            [20, 20, 20]
        } else {
            colour
        }
    })
}

/// An RGB PNG (filter 0, deflated) from a pixel function.
pub fn png_rgb(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(((w * 3 + 1) * h) as usize);
    for y in 0..h {
        raw.push(0);
        for x in 0..w {
            raw.extend_from_slice(&f(x, y));
        }
    }
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    z.write_all(&raw).expect("in memory");
    let idat = z.finish().expect("in memory");
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let chunk = |out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let mut c = flate2::Crc::new();
        c.update(kind);
        c.update(data);
        out.extend_from_slice(&c.sum().to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &idat);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn tile_bytes(format: crate::tiles::TileFormat, z: u8, x: u32, y: u32) -> Vec<u8> {
    match format {
        crate::tiles::TileFormat::Mvt => mvt_tile_at(z, x, y),
        _ => tile_png(z, x, y),
    }
}

/// An MBTiles file at `path` with every tile of zooms 0..=`max_zoom` (PNG, or vector tiles
/// gzip-compressed as MBTiles writers store them). Written with SQLite (feature
/// `sqlite-fixtures`; the readers never use it).
/// Switches the GeoPackage at `path` to WAL mode and adds `extra` points to `places` without
/// a checkpoint: they are only in the `-wal` file while the returned writer is open (as when
/// QGIS has the file open).
#[cfg(any(test, feature = "sqlite-fixtures"))]
pub fn geopackage_wal_writer(path: &std::path::Path, extra: usize) -> rusqlite::Connection {
    let c = rusqlite::Connection::open(path).expect("opens");
    c.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))
        .expect("wal");
    c.execute_batch("PRAGMA wal_autocheckpoint = 0;")
        .expect("no checkpoints");
    let n: i64 = c
        .query_row("SELECT max(fid) FROM places", [], |r| r.get(0))
        .expect("max");
    c.execute_batch("BEGIN;").expect("begin");
    for i in 0..extra as i64 {
        let fid = n + 1 + i;
        let mut g = b"GP\0\x01".to_vec();
        g.extend_from_slice(&4326i32.to_le_bytes());
        g.push(1);
        g.extend_from_slice(&1u32.to_le_bytes());
        g.extend_from_slice(&(20.0 + fid as f64 * 0.01).to_le_bytes());
        g.extend_from_slice(&40.0f64.to_le_bytes());
        c.execute(
            "INSERT INTO places VALUES (?1, ?2, ?3)",
            rusqlite::params![fid, g, format!("w{fid}")],
        )
        .expect("row");
    }
    c.execute_batch("COMMIT;").expect("commit");
    c
}

/// [`geopackage`], plus a feature view `even` (layer "Even places") of the points with an even
/// id.
#[cfg(any(test, feature = "sqlite-fixtures"))]
pub fn geopackage_with_view(path: &std::path::Path, n: usize) {
    geopackage(path, n, false);
    let c = rusqlite::Connection::open(path).expect("opens");
    c.execute_batch(
        "CREATE VIEW even AS SELECT fid AS id, geom, name FROM places WHERE fid % 2 = 0;
        INSERT INTO gpkg_contents (table_name, data_type, identifier, srs_id) VALUES ('even', 'features', 'Even places', 4326);
        INSERT INTO gpkg_geometry_columns VALUES ('even', 'geom', 'POINT', 4326, 0, 0);",
    )
    .expect("view");
}

/// A GeoPackage at `path` with one WGS 84 point layer `places` of `n` points. `hostile`: the
/// layer is instead an endless recursive view (a reader must refuse it, not loop). Written
/// with SQLite (feature `sqlite-fixtures`).
#[cfg(any(test, feature = "sqlite-fixtures"))]
pub fn geopackage(path: &std::path::Path, n: usize, hostile: bool) {
    let _ = std::fs::remove_file(path);
    let c = rusqlite::Connection::open(path).expect("creates");
    c.execute_batch(
        "PRAGMA application_id = 1196444487;
        CREATE TABLE gpkg_spatial_ref_sys (srs_name TEXT NOT NULL, srs_id INTEGER PRIMARY KEY, organization TEXT NOT NULL, organization_coordsys_id INTEGER NOT NULL, definition TEXT NOT NULL, description TEXT);
        INSERT INTO gpkg_spatial_ref_sys VALUES ('WGS 84', 4326, 'EPSG', 4326, 'undefined', NULL);
        CREATE TABLE gpkg_contents (table_name TEXT NOT NULL PRIMARY KEY, data_type TEXT NOT NULL, identifier TEXT, description TEXT DEFAULT '', last_change DATETIME, min_x DOUBLE, min_y DOUBLE, max_x DOUBLE, max_y DOUBLE, srs_id INTEGER);
        INSERT INTO gpkg_contents (table_name, data_type, identifier, srs_id) VALUES ('places', 'features', 'Places', 4326);
        CREATE TABLE gpkg_geometry_columns (table_name TEXT NOT NULL, column_name TEXT NOT NULL, geometry_type_name TEXT NOT NULL, srs_id INTEGER NOT NULL, z TINYINT NOT NULL, m TINYINT NOT NULL);
        INSERT INTO gpkg_geometry_columns VALUES ('places', 'geom', 'POINT', 4326, 0, 0);",
    )
    .expect("schema");
    if hostile {
        c.execute_batch(
            "CREATE VIEW places AS WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c) \
             SELECT x AS fid, NULL AS geom, 'p' || x AS name FROM c;",
        )
        .expect("view");
        return;
    }
    c.execute_batch("CREATE TABLE places (fid INTEGER PRIMARY KEY, geom POINT, name TEXT); BEGIN;")
        .expect("table");
    for i in 0..n {
        // GeoPackage header (no envelope, little-endian, SRS 4326), then a WKB point.
        let mut g = b"GP\0\x01".to_vec();
        g.extend_from_slice(&4326i32.to_le_bytes());
        g.push(1);
        g.extend_from_slice(&1u32.to_le_bytes());
        g.extend_from_slice(&(10.0 + i as f64 * 0.01).to_le_bytes());
        g.extend_from_slice(&(50.0 + i as f64 * 0.01).to_le_bytes());
        c.execute(
            "INSERT INTO places VALUES (?1, ?2, ?3)",
            rusqlite::params![i as i64 + 1, g, format!("p{i}")],
        )
        .expect("row");
    }
    c.execute_batch("COMMIT;").expect("commit");
}

#[cfg(any(test, feature = "sqlite-fixtures"))]
pub fn mbtiles(path: &std::path::Path, format: crate::tiles::TileFormat, max_zoom: u8) {
    let _ = std::fs::remove_file(path);
    let c = rusqlite::Connection::open(path).expect("creates");
    c.execute_batch(
        "PRAGMA application_id = 1297105496;
         CREATE TABLE metadata (name TEXT, value TEXT);
         CREATE TABLE tiles (zoom_level INTEGER, tile_column INTEGER, tile_row INTEGER, tile_data BLOB);
         CREATE UNIQUE INDEX tile_index ON tiles (zoom_level, tile_column, tile_row);",
    )
    .expect("schema");
    let vector = format == crate::tiles::TileFormat::Mvt;
    let mut meta = vec![
        ("name", "Test tiles".to_string()),
        ("format", if vector { "pbf" } else { "png" }.to_string()),
        ("bounds", "-180,-85.0511,180,85.0511".to_string()),
        ("minzoom", "0".to_string()),
        ("maxzoom", max_zoom.to_string()),
        (
            "attribution",
            r#"<a href="https://example.org">&copy; Test contributors</a>"#.to_string(),
        ),
    ];
    if vector {
        meta.push((
            "json",
            r#"{"vector_layers":[{"id":"land","fields":{}},{"id":"places","fields":{"name":"String"}}]}"#
                .to_string(),
        ));
    }
    for (k, v) in meta {
        c.execute("INSERT INTO metadata VALUES (?1, ?2)", [k, v.as_str()])
            .expect("metadata");
    }
    for z in 0..=max_zoom {
        let n = 1u32 << z;
        for y in 0..n {
            for x in 0..n {
                let mut b = tile_bytes(format, z, x, y);
                if vector {
                    b = gzip(&b);
                }
                c.execute(
                    "INSERT INTO tiles VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![i64::from(z), i64::from(x), i64::from(n - 1 - y), b],
                )
                .expect("tile");
            }
        }
    }
}

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7F) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

/// A PMTiles directory: (tile id, offset, length, run length).
fn pm_directory(entries: &[(u64, u64, u64, u64)]) -> Vec<u8> {
    let mut d = Vec::new();
    put_varint(&mut d, entries.len() as u64);
    let mut last = 0;
    for e in entries {
        put_varint(&mut d, e.0 - last);
        last = e.0;
    }
    for e in entries {
        put_varint(&mut d, e.3);
    }
    for e in entries {
        put_varint(&mut d, e.2);
    }
    for (i, e) in entries.iter().enumerate() {
        if i > 0 && e.1 == entries[i - 1].1 + entries[i - 1].2 {
            put_varint(&mut d, 0);
        } else {
            put_varint(&mut d, e.1 + 1);
        }
    }
    d
}

fn gzip(b: &[u8]) -> Vec<u8> {
    let mut g = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    g.write_all(b).expect("in memory");
    g.finish().expect("in memory")
}

/// A PMTiles v3 archive with every tile of zooms 0..=`max_zoom`. `internal`: directory
/// compression (1 none, 2 gzip); `leaves`: a root of leaf directories of 8 entries each.
/// Vector tiles are gzip-compressed.
pub fn pmtiles(
    format: crate::tiles::TileFormat,
    max_zoom: u8,
    internal: u8,
    leaves: bool,
) -> Vec<u8> {
    use crate::pmtiles::tile_id;
    let vector = format == crate::tiles::TileFormat::Mvt;
    let mut tiles: Vec<(u64, Vec<u8>)> = Vec::new();
    for z in 0..=max_zoom {
        let n = 1u32 << z;
        for y in 0..n {
            for x in 0..n {
                let b = tile_bytes(format, z, x, y);
                tiles.push((tile_id(z, x, y), if vector { gzip(&b) } else { b }));
            }
        }
    }
    tiles.sort_by_key(|t| t.0);
    let mut data = Vec::new();
    let mut entries = Vec::new();
    for (id, b) in &tiles {
        entries.push((*id, data.len() as u64, b.len() as u64, 1u64));
        data.extend_from_slice(b);
    }
    let comp = |b: Vec<u8>| if internal == 2 { gzip(&b) } else { b };
    let (root, leaf_bytes) = if leaves {
        let mut leaf_bytes = Vec::new();
        let mut root = Vec::new();
        for chunk in entries.chunks(8) {
            let d = comp(pm_directory(chunk));
            root.push((chunk[0].0, leaf_bytes.len() as u64, d.len() as u64, 0u64));
            leaf_bytes.extend_from_slice(&d);
        }
        (comp(pm_directory(&root)), leaf_bytes)
    } else {
        (comp(pm_directory(&entries)), Vec::new())
    };
    let meta = comp(
        r#"{"name":"Test tiles","attribution":"<a href=\"https://example.org\">&copy; Test contributors</a>","vector_layers":[{"id":"land"},{"id":"places"}]}"#
            .as_bytes()
            .to_vec(),
    );
    let root_off = 127u64;
    let meta_off = root_off + root.len() as u64;
    let leaf_off = meta_off + meta.len() as u64;
    let data_off = leaf_off + leaf_bytes.len() as u64;
    let mut h = Vec::with_capacity(127);
    h.extend_from_slice(b"PMTiles\x03");
    for v in [
        root_off,
        root.len() as u64,
        meta_off,
        meta.len() as u64,
        leaf_off,
        leaf_bytes.len() as u64,
        data_off,
        data.len() as u64,
        tiles.len() as u64,
        tiles.len() as u64,
        tiles.len() as u64,
    ] {
        h.extend_from_slice(&v.to_le_bytes());
    }
    h.push(1); // clustered
    h.push(internal);
    h.push(if vector { 2 } else { 1 });
    h.push(if vector { 1 } else { 2 });
    h.push(0);
    h.push(max_zoom);
    for v in [-1_800_000_000i32, -850_511_287, 1_800_000_000, 850_511_287] {
        h.extend_from_slice(&v.to_le_bytes());
    }
    h.push(0);
    h.extend_from_slice(&0i32.to_le_bytes());
    h.extend_from_slice(&0i32.to_le_bytes());
    debug_assert_eq!(h.len(), 127);
    let mut out = h;
    out.extend_from_slice(&root);
    out.extend_from_slice(&meta);
    out.extend_from_slice(&leaf_bytes);
    out.extend_from_slice(&data);
    out
}

fn pb_key(out: &mut Vec<u8>, field: u64, wire: u64) {
    put_varint(out, (field << 3) | wire);
}

fn pb_bytes(out: &mut Vec<u8>, field: u64, b: &[u8]) {
    pb_key(out, field, 2);
    put_varint(out, b.len() as u64);
    out.extend_from_slice(b);
}

fn zz(v: i32) -> u32 {
    ((v << 1) ^ (v >> 31)) as u32
}

fn pb_packed(out: &mut Vec<u8>, field: u64, v: &[u32]) {
    let mut b = Vec::new();
    for &x in v {
        put_varint(&mut b, u64::from(x));
    }
    pb_bytes(out, field, &b);
}

pub(crate) fn pb_feature(id: u64, kind: u64, tags: &[u32], geom: &[u32]) -> Vec<u8> {
    let mut f = Vec::new();
    pb_key(&mut f, 1, 0);
    put_varint(&mut f, id);
    pb_packed(&mut f, 2, tags);
    pb_key(&mut f, 3, 0);
    put_varint(&mut f, kind);
    pb_packed(&mut f, 4, geom);
    f
}

pub(crate) fn pb_layer(
    name: &str,
    features: &[Vec<u8>],
    keys: &[&str],
    values: &[&str],
) -> Vec<u8> {
    let mut l = Vec::new();
    pb_key(&mut l, 15, 0);
    put_varint(&mut l, 2);
    pb_bytes(&mut l, 1, name.as_bytes());
    for f in features {
        pb_bytes(&mut l, 2, f);
    }
    for k in keys {
        pb_bytes(&mut l, 3, k.as_bytes());
    }
    for v in values {
        let mut vb = Vec::new();
        pb_bytes(&mut vb, 1, v.as_bytes());
        pb_bytes(&mut l, 4, &vb);
    }
    pb_key(&mut l, 5, 0);
    put_varint(&mut l, 4096);
    l
}

/// A vector tile: layer "places" with a point at the tile's centre named "Centre", and
/// layer "land" with a square with a hole and a line.
pub fn mvt_tile() -> Vec<u8> {
    mvt_tile_named("Centre")
}

fn mvt_tile_at(z: u8, x: u32, y: u32) -> Vec<u8> {
    mvt_tile_named(&format!("{z}/{x}/{y}"))
}

fn mvt_tile_named(name: &str) -> Vec<u8> {
    let cmd = |id: u32, n: u32| (id & 7) | (n << 3);
    let point = pb_feature(1, 1, &[0, 0], &[cmd(1, 1), zz(2048), zz(2048)]);
    // A square (clockwise on screen: positive area with y down) and a hole inside it.
    let square = [
        cmd(1, 1),
        zz(512),
        zz(512),
        cmd(2, 3),
        zz(3072),
        zz(0),
        zz(0),
        zz(3072),
        zz(-3072),
        zz(0),
        cmd(7, 1),
        cmd(1, 1),
        zz(512),
        zz(-2560),
        cmd(2, 3),
        zz(0),
        zz(1024),
        zz(1024),
        zz(0),
        zz(0),
        zz(-1024),
        cmd(7, 1),
    ];
    let poly = pb_feature(2, 3, &[], &square);
    let line = pb_feature(
        3,
        2,
        &[],
        &[cmd(1, 1), zz(0), zz(4096), cmd(2, 1), zz(4096), zz(-4096)],
    );
    let mut t = Vec::new();
    pb_bytes(&mut t, 3, &pb_layer("places", &[point], &["name"], &[name]));
    pb_bytes(&mut t, 3, &pb_layer("land", &[poly, line], &[], &[]));
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapefile_reads_back() {
        let feats = parcels(3, 100.0, 4, (500_000.0, 5_000_000.0));
        let p = shapefile_polygons(&feats, UTM33N_WKT);
        let ds = crate::shp::read(
            crate::shp::Parts {
                shp: &p.shp,
                shx: Some(&p.shx),
                dbf: Some(&p.dbf),
                prj: Some(&p.prj),
                cpg: None,
            },
            "parcels",
        )
        .expect("reads");
        assert_eq!(ds.feature_count(), 9);
        assert_eq!(ds.crs.as_ref().and_then(|c| c.epsg), Some(32633));
        let a = ds.layers[0].attributes(4);
        assert!(
            a.iter().any(|(k, v)| k == "NAME" && v.text() == "Parcel 4"),
            "{a:?}"
        );
        match &ds.layers[0].features[0].geometry {
            Some(crate::Geometry::Polygon(r)) => assert_eq!(r.len(), 2),
            g => panic!("{g:?}"),
        }
    }

    /// Opening a Shapefile's index (.shx), which starts as the .shp does, opens the shapes
    /// beside it; without them it says what the file is.
    #[test]
    fn the_index_opens_its_shapefile() {
        let dir = std::env::temp_dir().join(format!("specular-gis-shx-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p = shapefile_polygons(&parcels(2, 100.0, 4, (500_000.0, 5_000_000.0)), UTM33N_WKT);
        std::fs::write(dir.join("p.SHP"), &p.shp).unwrap();
        std::fs::write(dir.join("p.shx"), &p.shx).unwrap();
        std::fs::write(dir.join("p.dbf"), &p.dbf).unwrap();
        let ds = crate::read(crate::VectorKind::Shapefile, &dir.join("p.shx"), &p.shx).unwrap();
        assert_eq!(ds.feature_count(), 4);
        assert_eq!(ds.layers[0].name, "p");
        std::fs::remove_file(dir.join("p.SHP")).unwrap();
        let e = crate::read(crate::VectorKind::Shapefile, &dir.join("p.shx"), &p.shx).unwrap_err();
        assert!(e.contains(".shp is missing"), "{e}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
