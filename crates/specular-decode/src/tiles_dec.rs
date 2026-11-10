//! Web map tile sets opened as maps: MBTiles and PMTiles (own readers in `specular-gis`).
//!
//! Raster tiles (PNG, JPEG, WebP, AVIF) show as one image: the deepest zoom level whose
//! mosaic over the set's bounds fits [`MOSAIC_MAX`] pixels on the long side, georeferenced in
//! Web Mercator (EPSG:3857). Vector tiles (MVT) are read at the deepest zoom that needs at
//! most [`VECTOR_TILES`] tiles and drawn like any vector map, with hover and click on their
//! features.

use std::sync::Arc;

use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_gis::geotiff::Affine2;
use specular_gis::model::{Dataset, TileLevel};
use specular_gis::tiles::{self, TileFormat, TileMeta, TileStore};

use crate::geo::{GeoFacts, GeoRef, TilesGeo};
use crate::whole::Levels;
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, Registry, Result, Session};

/// The longest side of a raster mosaic, pixels.
const MOSAIC_MAX: u64 = 8192;
/// A tile's own header may claim at most this many pixels (tiles are 256 to 1024 wide).
pub(crate) const MAX_TILE_PIXELS: u64 = 4096 * 4096;
/// Vector tiles read at most for the map.
const VECTOR_TILES: u64 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    MbTiles,
    PmTiles,
}

pub struct TilesDecoder(pub Kind);

/// A PMTiles file in memory (mapped), read by ranges.
struct SourceRange(Arc<Source>);

impl specular_gis::pmtiles::RangeRead for SourceRange {
    fn read_range(&self, offset: u64, len: u64) -> std::result::Result<Vec<u8>, String> {
        let b = self.0.bytes();
        let (Ok(o), Ok(l)) = (usize::try_from(offset), usize::try_from(len)) else {
            return Err("offset out of range".into());
        };
        o.checked_add(l)
            .and_then(|e| b.get(o..e))
            .map(<[u8]>::to_vec)
            .ok_or_else(|| "PMTiles section past the end of the file".into())
    }
}

/// Opens a tile store from a session source.
pub(crate) fn store(kind: Kind, source: &Arc<Source>) -> Result<Arc<dyn TileStore>> {
    Ok(match kind {
        Kind::MbTiles => {
            if !crate::geo::on_disk(source) {
                return Err(DecodeError::Unsupported(
                    "an MBTiles file must be opened from disk, not from an archive".into(),
                ));
            }
            // SQLite runs in the sandboxed decode host.
            let db = crate::sql_host::HostDb::open(source)?;
            Arc::new(
                specular_gis::mbtiles::MbTiles::open(Box::new(db))
                    .map_err(DecodeError::Malformed)?,
            )
        }
        Kind::PmTiles => Arc::new(
            specular_gis::pmtiles::PmTiles::open(Box::new(SourceRange(source.clone())))
                .map_err(DecodeError::Malformed)?,
        ),
    })
}

impl Decoder for TilesDecoder {
    fn format(&self) -> Format {
        match self.0 {
            Kind::MbTiles => Format::MbTiles,
            Kind::PmTiles => Format::PmTiles,
        }
    }
    fn probe(&self, head: &[u8]) -> bool {
        match self.0 {
            Kind::MbTiles => specular_gis::mbtiles::sniff(head),
            Kind::PmTiles => specular_gis::pmtiles::sniff(head),
        }
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Kind::MbTiles => &["mbtiles"],
            Kind::PmTiles => &["pmtiles"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let t0 = std::time::Instant::now();
        let st = store(self.0, &source)?;
        let meta = st.meta().clone();
        match meta.format {
            TileFormat::Mvt => open_vector(source, st, self.format(), t0),
            f if f.is_raster() => RasterTiles::open(source, st, self.format()),
            TileFormat::Unknown => Err(DecodeError::Unsupported(
                "the tile set does not say what its tiles are".into(),
            )),
            _ => Err(DecodeError::Unsupported("unknown tile type".into())),
        }
    }
}

/// A tile set file (MBTiles or PMTiles, by content, then by extension) for the map view's
/// base map.
pub fn open_tile_file(path: &std::path::Path) -> Result<Arc<dyn TileStore>> {
    let source = Arc::new(Source::open(path)?);
    let head = &source.bytes()[..source.len().min(128)];
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let kind = if specular_gis::pmtiles::sniff(head) || ext == "pmtiles" {
        Kind::PmTiles
    } else if specular_gis::mbtiles::sniff(head) || ext == "mbtiles" {
        Kind::MbTiles
    } else {
        return Err(DecodeError::UnsupportedFormat);
    };
    store(kind, &source)
}

/// A raster tile's pixels: (width, height, straight sRGB RGBA8).
pub fn tile_rgba(bytes: Vec<u8>) -> Option<(u32, u32, Vec<u8>)> {
    let cancel = CancelToken::new();
    let s = Registry::default()
        .open(Arc::new(Source::from_vec("tile", bytes)))
        .ok()?;
    let r = s.decode(&DecodeRequest::full(1), &cancel).ok()?;
    if r.layout.is_wide() {
        return None;
    }
    let r = crate::gainmap::to_rgba8(&r);
    let pl = r.planes.first()?;
    let (w, h) = (r.width as usize, r.height as usize);
    let mut out = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        out.extend_from_slice(pl.data.get(y * pl.stride..y * pl.stride + w * 4)?);
    }
    Some((r.width, r.height, out))
}

/// A vector tile drawn as a `size`-pixel raster tile in the simple map style (each layer
/// its colour) over `background`: straight sRGB RGBA8.
pub fn render_vector_tile(
    bytes: &[u8],
    key: (u8, u32, u32),
    size: u32,
    dark: bool,
    background: [u8; 3],
) -> Option<Vec<u8>> {
    use specular_vector::fonts::Fonts;
    use specular_vector::kurbo::Affine;
    use specular_vector::prep::{PrepLimits, prepare};
    let mut ds = Dataset::default();
    let mut index = std::collections::HashMap::new();
    specular_gis::mvt::decode_into(bytes, key, &mut ds.layers, &mut index).ok()?;
    let b = tiles::tile_bounds(key.0, key.1, key.2);
    let scene = specular_gis::scene::to_scene(&ds);
    let prepared = prepare(Arc::new(scene), &Fonts::shared(), PrepLimits::default());
    let s = f64::from(size) / (b[2] - b[0]);
    let layers: Vec<bool> = vec![true; ds.layers.len()];
    let view = specular_vector::draw::View {
        transform: Affine::new([s, 0.0, 0.0, -s, -b[0] * s, b[3] * s]),
        size: (size, size),
        dark,
        background,
        lineweights: false,
        dpi_scale: 1.0,
        layers: &layers,
    };
    match prepared.sheets.first() {
        Some(sheet) => specular_vector::raster::render(sheet, &view).map(|(d, _)| d),
        None => {
            let mut px = Vec::with_capacity((size * size * 4) as usize);
            for _ in 0..size * size {
                px.extend_from_slice(&[background[0], background[1], background[2], 255]);
            }
            Some(px)
        }
    }
}

/// Tiles covering `bounds` (degrees) at zoom `z`.
fn count(z: u8, b: [f64; 4]) -> (u64, u64) {
    let (x0, y0, x1, y1) = tiles::range(z, b);
    (u64::from(x1 - x0) + 1, u64::from(y1 - y0) + 1)
}

/// The zoom levels of a set as tile matrices (for the info panel's overviews).
fn levels(meta: &TileMeta, tile: u32) -> Vec<TileLevel> {
    (meta.min_zoom..=meta.max_zoom)
        .map(|z| {
            let (w, h) = count(z, meta.bounds);
            let px = 2.0 * tiles::HALF / f64::from(1u32 << z.min(31)) / f64::from(tile);
            TileLevel {
                zoom: i64::from(z),
                matrix_w: w as i64,
                matrix_h: h as i64,
                tile_w: tile,
                tile_h: tile,
                pixel_x: px,
                pixel_y: px,
            }
        })
        .collect()
}

fn open_vector(
    source: Arc<Source>,
    st: Arc<dyn TileStore>,
    format: Format,
    t0: std::time::Instant,
) -> Result<Arc<dyn Session>> {
    let meta = st.meta().clone();
    let mut z = meta.min_zoom;
    for c in meta.min_zoom..=meta.max_zoom.min(30) {
        let (w, h) = count(c, meta.bounds);
        if w * h <= VECTOR_TILES {
            z = c;
        }
    }
    let range = tiles::range(z, meta.bounds);
    let found = st
        .tiles_in(z, range, VECTOR_TILES as usize)
        .map_err(DecodeError::Malformed)?;
    let mut ds = Dataset {
        format: meta.container,
        crs: specular_gis::crs::from_epsg(3857),
        ..Dataset::default()
    };
    let mut index = std::collections::HashMap::new();
    for (x, y, b) in &found {
        if let Err(e) = specular_gis::mvt::decode_into(b, (z, *x, *y), &mut ds.layers, &mut index) {
            ds.warn(format!("tile {z}/{x}/{y}: {e}"));
        }
    }
    if found.is_empty() {
        return Err(DecodeError::Malformed(format!("no tiles at zoom {z}")));
    }
    if ds.bounds().is_none() {
        return Err(DecodeError::Malformed(
            "the vector tiles hold no features".into(),
        ));
    }
    let read_ms = t0.elapsed().as_secs_f64() * 1e3;
    crate::gis_dec::map_session(source, ds, format, read_ms, Some(meta))
}

/// A raster tile set at one zoom level, as one image.
struct RasterTiles {
    source: Arc<Source>,
    store: Arc<dyn TileStore>,
    info: ImageInfo,
    zoom: u8,
    /// Tile range (x0, y0, x1, y1) and tile size.
    range: (u32, u32, u32, u32),
    tile: u32,
    geo: Arc<GeoRef>,
    levels: Levels,
}

impl RasterTiles {
    fn open(
        source: Arc<Source>,
        st: Arc<dyn TileStore>,
        format: Format,
    ) -> Result<Arc<dyn Session>> {
        let meta = st.meta().clone();
        // The tile size from a tile at the shallowest zoom.
        let tile = {
            let (x0, y0, x1, y1) = tiles::range(meta.min_zoom, meta.bounds);
            st.tiles_in(meta.min_zoom, (x0, y0, x1, y1), 1)
                .ok()
                .and_then(|v| v.into_iter().next())
                .and_then(|(_, _, b)| {
                    Registry::default()
                        .open(Arc::new(Source::from_vec("tile", b)))
                        .ok()
                })
                .map(|s| s.info().width.max(s.info().height))
                .filter(|&s| (64..=1024).contains(&s))
                .unwrap_or(256)
        };
        let mut z = meta.min_zoom;
        for c in meta.min_zoom..=meta.max_zoom.min(30) {
            let (w, h) = count(c, meta.bounds);
            if w.max(h) * u64::from(tile) <= MOSAIC_MAX {
                z = c;
            }
        }
        let range = tiles::range(z, meta.bounds);
        let (nw, nh) = count(z, meta.bounds);
        let (w, h) = (
            (nw * u64::from(tile)).min(MOSAIC_MAX) as u32,
            (nh * u64::from(tile)).min(MOSAIC_MAX) as u32,
        );
        let tb = tiles::tile_bounds(z, range.0, range.1);
        let px = (tb[2] - tb[0]) / f64::from(tile);
        let to_map = Affine2 {
            a: px,
            b: 0.0,
            c: tb[0],
            d: 0.0,
            e: -px,
            f: tb[3],
        };
        let bounds = [
            tb[0],
            tb[3] - f64::from(h) * px,
            tb[0] + f64::from(w) * px,
            tb[3],
        ];
        let info = ImageInfo {
            format,
            width: w,
            height: h,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: true,
            bit_depth: 8,
            progressive: false,
            coding: format!(
                "{} tiles, zoom {z} of {}–{}, {nw}×{nh} tiles",
                meta.format.name(),
                meta.min_zoom,
                meta.max_zoom
            ),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        let geo = Arc::new(GeoRef {
            crs: specular_gis::crs::from_epsg(3857),
            crs_note: None,
            to_map,
            bounds,
            features: None,
            facts: GeoFacts::Tiles(TilesGeo {
                table: meta.container.to_string(),
                identifier: meta.name.clone(),
                levels: levels(&meta, tile),
                zoom: i64::from(z),
                tiles: (nw * nh) as usize,
                tile_meta: Some(meta),
            }),
        });
        Ok(Arc::new(RasterTiles {
            source,
            store: st,
            info,
            zoom: z,
            range,
            tile,
            geo,
            levels: Levels::new(),
        }))
    }

    fn mosaic(&self, cancel: &CancelToken) -> Result<Raster> {
        let (w, h) = (self.info.width as usize, self.info.height as usize);
        let t = self.tile as usize;
        let mut out = vec![0u8; w * h * 4];
        let registry = Registry::default();
        let mut truncated = false;
        let found = self
            .store
            .tiles_in(self.zoom, self.range, 1 << 12)
            .map_err(DecodeError::Malformed)?;
        for (tx, ty, bytes) in found {
            if cancel.is_cancelled() {
                return Err(DecodeError::Cancelled);
            }
            let x0 = (tx - self.range.0) as usize * t;
            let y0 = (ty - self.range.1) as usize * t;
            if x0 >= w || y0 >= h {
                continue;
            }
            let decoded =
                crate::open_nested(&registry, "tile", bytes, &self.source, MAX_TILE_PIXELS)
                    .and_then(|s| s.decode(&DecodeRequest::full(1), cancel));
            let r = match decoded {
                Ok(r) if !r.layout.is_wide() => crate::gainmap::to_rgba8(&r),
                _ => {
                    truncated = true;
                    continue;
                }
            };
            let pl = &r.planes[0];
            let cw = (r.width as usize).min(t).min(w - x0);
            for y in 0..(r.height as usize).min(t).min(h - y0) {
                let src = &pl.data[y * pl.stride..y * pl.stride + cw * 4];
                let o = ((y0 + y) * w + x0) * 4;
                out[o..o + cw * 4].copy_from_slice(src);
            }
        }
        Ok(crate::whole::interleaved(
            self.info.width,
            self.info.height,
            PixelLayout::Rgba,
            out,
            truncated,
        ))
    }
}

impl Session for RasterTiles {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| self.mosaic(cancel),
            cancel,
        )
    }
    fn decode_memory(&self, _req: &DecodeRequest) -> u64 {
        u64::from(self.info.width) * u64::from(self.info.height) * 4 * 2
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear()
    }
    fn geo(&self) -> Option<Arc<GeoRef>> {
        Some(self.geo.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_gis::fixtures;

    #[test]
    fn pmtiles_raster_mosaic() {
        let file = fixtures::pmtiles(TileFormat::Png, 2, 2, true);
        let s = Registry::default()
            .open(Arc::new(Source::from_vec("world.pmtiles", file)))
            .expect("opens");
        assert_eq!(s.info().format, Format::PmTiles);
        // Zoom 2: 4 × 4 tiles of 256 px.
        assert_eq!((s.info().width, s.info().height), (1024, 1024));
        let g = s.geo().unwrap();
        assert_eq!(g.crs.as_ref().and_then(|c| c.epsg), Some(3857));
        let (mx, my) = g.map_at(0.0, 0.0);
        assert!((mx + tiles::HALF).abs() < 1e-6 && (my - tiles::HALF).abs() < 1e-6);
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        // Tile (2, 3, 1)'s colour in its middle.
        let px = |x: usize, y: usize| {
            let o = (y * 1024 + x) * 4;
            r.planes[0].data[o..o + 3].to_vec()
        };
        let want = [140u8, (3 * 67 % 200 + 40) as u8, 129];
        assert_eq!(px(3 * 256 + 128, 256 + 128), want);
    }

    #[test]
    fn mbtiles_vector_map() {
        let dir = std::env::temp_dir().join(format!("specular-tiles-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("v.mbtiles");
        fixtures::mbtiles(&path, TileFormat::Mvt, 1);
        let s = Registry::default()
            .open(Arc::new(Source::open(&path).unwrap()))
            .expect("opens");
        assert_eq!(s.info().format, Format::MbTiles);
        let g = s.geo().unwrap();
        let idx = g.features.as_ref().expect("features");
        // Zoom 1: four tiles, each with its point named after it.
        let places = idx
            .dataset
            .layers
            .iter()
            .find(|l| l.name == "places")
            .unwrap();
        assert_eq!(places.features.len(), 4);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
