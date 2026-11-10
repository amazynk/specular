//! Vector maps (GeoJSON, Shapefile, GeoPackage, KML/KMZ, GPX, FlatGeobuf, GeoParquet and
//! GeoArrow, GML, OpenStreetMap, FIT and TCX, File Geodatabase, MapInfo, S-57 charts,
//! TopoJSON, IGC flight logs and NMEA logs; own readers in
//! `specular-gis`)
//! drawn by `specular-vector` like a DXF drawing, and GeoPackage tile pyramids shown as an
//! image.
//!
//! A vector map is one "Map" sheet in the dataset's own coordinates: the session's pixels are
//! the sheet rasterised on the CPU at a fixed size (thumbnails, the overview, the first frame)
//! and the viewer draws it on the GPU at the exact zoom from [`Session::drawing`]. Its
//! [`Session::geo`] maps stored pixels to map coordinates and carries the hit-test index for
//! the feature under the pointer. A GeoPackage with tiles and no features shows the most
//! detailed zoom level whose mosaic fits [`MOSAIC_MAX`] pixels on the long side.

use std::sync::Arc;

use specular_core::{CancelToken, Orientation, PixelLayout, Raster, RasterPlane, Source};
use specular_gis::VectorKind;
use specular_gis::geotiff::Affine2;
use specular_gis::model::{Dataset, TileSet};
use specular_gis::scene::FeatureIndex;
use specular_vector::draw::View;
use specular_vector::fonts::Fonts;
use specular_vector::kurbo::Affine;
use specular_vector::prep::{PrepLimits, prepare};

use crate::drawing::{DrawingPage, paper_colour};
use crate::geo::{GeoFacts, GeoRef, TilesGeo};
use crate::whole::Levels;
use crate::{
    DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Registry, Result, Session,
};

/// The longest side of a tile mosaic, pixels.
const MOSAIC_MAX: i64 = 8192;
/// Bytes the probe looks at (GeoJSON, KML and GPX are recognised by their first elements).
const SNIFF: usize = 4096;

pub struct GisDecoder(pub VectorKind);

/// A reader's error: a variant the readers decline by name (compressed geodatabase tables,
/// seamless MapInfo tables, a Parquet table without geometry, a FIT file without positions)
/// is unsupported; anything else is a damaged file.
fn refusal(e: String) -> DecodeError {
    let declined = [
        "not supported",
        "no geometry column",
        "no GPS positions",
        "not a vector table",
    ];
    if declined.iter().any(|d| e.contains(d)) {
        DecodeError::Unsupported(e)
    } else {
        DecodeError::Malformed(e)
    }
}

fn format_of(kind: VectorKind) -> Format {
    match kind {
        VectorKind::GeoJson => Format::GeoJson,
        VectorKind::Shapefile => Format::Shapefile,
        VectorKind::GeoPackage => Format::GeoPackage,
        VectorKind::Kml => Format::Kml,
        VectorKind::Kmz => Format::Kmz,
        VectorKind::Gpx => Format::Gpx,
        VectorKind::FlatGeobuf => Format::FlatGeobuf,
        VectorKind::GeoParquet => Format::GeoParquet,
        VectorKind::GeoArrow => Format::GeoArrow,
        VectorKind::Gml => Format::Gml,
        VectorKind::Osm => Format::OsmXml,
        VectorKind::OsmPbf => Format::OsmPbf,
        VectorKind::Fit => Format::Fit,
        VectorKind::Tcx => Format::Tcx,
        VectorKind::FileGdb => Format::FileGdb,
        VectorKind::MapInfoTab => Format::MapInfoTab,
        VectorKind::MapInfoMif => Format::MapInfoMif,
        VectorKind::S57 => Format::S57,
        VectorKind::TopoJson => Format::TopoJson,
        VectorKind::Igc => Format::Igc,
        VectorKind::Nmea => Format::Nmea,
    }
}

impl Decoder for GisDecoder {
    fn format(&self) -> Format {
        format_of(self.0)
    }
    fn probe(&self, head: &[u8]) -> bool {
        specular_gis::sniff(head) == Some(self.0)
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        specular_gis::sniff(&data[..data.len().min(SNIFF)]) == Some(self.0)
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            VectorKind::GeoJson => &["geojson"],
            VectorKind::Shapefile => &["shp"],
            VectorKind::GeoPackage => &["gpkg"],
            VectorKind::Kml => &["kml"],
            VectorKind::Kmz => &["kmz"],
            VectorKind::Gpx => &["gpx"],
            VectorKind::FlatGeobuf => &["fgb"],
            VectorKind::GeoParquet => &["parquet", "geoparquet"],
            VectorKind::GeoArrow => &["arrow", "feather", "arrows"],
            VectorKind::Gml => &["gml"],
            VectorKind::Osm => &["osm"],
            VectorKind::OsmPbf => &["osm.pbf"],
            VectorKind::Fit => &["fit"],
            VectorKind::Tcx => &["tcx"],
            VectorKind::FileGdb => &["gdb", "gdbtable", "gdbtablx"],
            VectorKind::MapInfoTab => &["tab"],
            VectorKind::MapInfoMif => &["mif"],
            // Update files (`.001`…) change a cell; they are not opened alone.
            VectorKind::S57 => &["000"],
            VectorKind::TopoJson => &["topojson"],
            VectorKind::Igc => &["igc"],
            VectorKind::Nmea => &["nmea"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let t0 = std::time::Instant::now();
        // GeoPackage is an SQLite database: SQLite runs in the sandboxed decode host.
        let db = if self.0 == VectorKind::GeoPackage {
            Some(Arc::new(crate::sql_host::HostDb::open(&source)?))
        } else {
            None
        };
        let ds = match &db {
            Some(db) => specular_gis::gpkg::read(db.as_ref()),
            None => specular_gis::read(self.0, source.path(), source.bytes()),
        }
        .map_err(refusal)?;
        let read_ms = t0.elapsed().as_secs_f64() * 1e3;
        if ds.feature_count() == 0
            && let Some(t) = ds.tiles.first()
            && let Some(db) = db
        {
            return TileSession::open(source, db, &ds, t.clone(), format_of(self.0));
        }
        // A KML or KMZ of ground overlays only: the first overlay's image on the map.
        if ds.bounds().is_none()
            && matches!(self.0, VectorKind::Kml | VectorKind::Kmz)
            && let Some(o) = ds.overlays.first()
        {
            return overlay_session(
                &source,
                &ds,
                o,
                format_of(self.0),
                self.0 == VectorKind::Kmz,
            );
        }
        if ds.bounds().is_none() {
            return Err(DecodeError::Malformed(if ds.feature_count() == 0 {
                // Why a layer was left out (a view not read for safety), when one was.
                match ds.warnings.first() {
                    Some(w) => format!("no features ({w})"),
                    None => "no features".into(),
                }
            } else {
                "no feature has a geometry".into()
            }));
        }
        map_session(source, ds, format_of(self.0), read_ms, None)
    }
}

/// Most bytes of a ground overlay's image read beside a KML.
const MAX_OVERLAY: u64 = 1 << 30;

/// The georeference of a ground overlay's image of `w` × `h` pixels: its LatLonBox (rotated
/// counter-clockwise about its centre) or the affine fit of its gx:LatLonQuad.
fn overlay_georef(o: &specular_gis::model::GroundOverlay, w: u32, h: u32) -> Option<GeoRef> {
    let (wf, hf) = (f64::from(w), f64::from(h));
    let to_map = match o.quad {
        Some(q) => {
            // Corners counter-clockwise from the lower left.
            let px = [[0.0, hf], [wf, hf], [wf, 0.0], [0.0, 0.0]];
            Affine2::fit(&px.iter().copied().zip(q).collect::<Vec<_>>())?
        }
        None => {
            let (cx, cy) = ((o.east + o.west) / 2.0, (o.north + o.south) / 2.0);
            let (sx, sy) = ((o.east - o.west) / wf, (o.south - o.north) / hf);
            let (c, s) = (o.rotation.to_radians().cos(), o.rotation.to_radians().sin());
            // Pixel to unrotated map offsets from the centre, then rotated.
            let (a0, e0) = (sx, sy);
            let (ox, oy) = (-wf / 2.0 * sx, -hf / 2.0 * sy);
            Affine2 {
                a: c * a0,
                b: -s * e0,
                c: cx + c * ox - s * oy,
                d: s * a0,
                e: c * e0,
                f: cy + s * ox + c * oy,
            }
        }
    };
    let ok = [to_map.a, to_map.b, to_map.c, to_map.d, to_map.e, to_map.f]
        .iter()
        .all(|v| v.is_finite())
        && to_map.inverse().is_some();
    ok.then(|| {
        crate::geo::raster(
            to_map,
            specular_gis::crs::from_epsg(4326),
            None,
            (w, h),
            crate::geo::RasterGeo {
                levels: vec![(w, h)],
                keys: vec![
                    ("GroundOverlay".into(), o.name.clone().unwrap_or_default()),
                    ("href".into(), o.href.clone()),
                ],
                keys_kind: "header",
                ..Default::default()
            },
        )
    })
}

/// A ground overlay's image (from the KMZ, or beside the KML; never from a server) shown
/// on the map.
fn overlay_session(
    source: &Arc<Source>,
    ds: &Dataset,
    o: &specular_gis::model::GroundOverlay,
    format: Format,
    kmz: bool,
) -> Result<Arc<dyn Session>> {
    let href = o.href.trim();
    let lower = href.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Err(DecodeError::Unsupported(
            "the ground overlay's image is on a server; it is not downloaded".into(),
        ));
    }
    let bytes = if kmz {
        specular_gis::kml::kmz_member(source.bytes(), href)
    } else if crate::geo::on_disk(source) {
        let p =
            source.path().parent().map(|d| d.join(href)).filter(|p| {
                std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.len() <= MAX_OVERLAY)
            });
        p.and_then(|p| std::fs::read(p).ok())
    } else {
        None
    }
    .ok_or_else(|| {
        DecodeError::Malformed(format!("the ground overlay's image {href} is missing"))
    })?;
    let inner = Registry::default().open(Arc::new(Source::from_vec(href, bytes)))?;
    let (w, h) = (inner.info().width, inner.info().height);
    let geo = overlay_georef(o, w, h)
        .ok_or_else(|| DecodeError::Malformed("the ground overlay has no usable box".into()))?;
    let mut info = inner.info().clone();
    info.format = format;
    info.coding = if ds.overlays.len() > 1 {
        format!(
            "ground overlay 1 of {}, {}",
            ds.overlays.len(),
            inner.info().coding
        )
    } else {
        format!("ground overlay, {}", inner.info().coding)
    };
    Ok(Arc::new(OverlaySession {
        source: source.clone(),
        inner,
        info,
        geo: Arc::new(geo),
    }))
}

/// A ground overlay image: the image's session with the KML's format and the overlay's
/// georeference.
struct OverlaySession {
    source: Arc<Source>,
    inner: Arc<dyn Session>,
    info: ImageInfo,
    geo: Arc<GeoRef>,
}

impl Session for OverlaySession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn alpha(&self) -> Option<crate::Alpha> {
        self.inner.alpha()
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        self.inner.decode_preview()
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.inner.decode(req, cancel)
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        self.inner.decode_memory(req)
    }
    fn memory_bytes(&self) -> usize {
        self.inner.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.inner.trim_memory()
    }
    fn geo(&self) -> Option<Arc<GeoRef>> {
        Some(self.geo.clone())
    }
}

/// A vector dataset as a map session: drawn as one sheet, with the hit-test index;
/// `tile_meta` for vector tile sets.
pub(crate) fn map_session(
    source: Arc<Source>,
    ds: Dataset,
    format: Format,
    read_ms: f64,
    tile_meta: Option<specular_gis::tiles::TileMeta>,
) -> Result<Arc<dyn Session>> {
    {
        let t1 = std::time::Instant::now();
        let scene = specular_gis::scene::to_scene(&ds);
        let prepared = Arc::new(prepare(
            Arc::new(scene),
            &Fonts::shared(),
            PrepLimits::default(),
        ));
        let ds = Arc::new(ds);
        let index = Arc::new(FeatureIndex::new(ds.clone()));
        let prep_ms = t1.elapsed().as_secs_f64() * 1e3;
        specular_log::debug!(
            target: specular_log::DECODE,
            "{} {}: {} layers, {} features, read in {read_ms:.1} ms, prepared in {prep_ms:.1} ms",
            ds.format,
            source.path().display(),
            ds.layers.len(),
            ds.feature_count(),
        );
        if prepared.sheets.is_empty() {
            return Err(DecodeError::Malformed("nothing to draw".into()));
        }
        let ((w, h), page) = DrawingPage::new(prepared, 0, read_ms + prep_ms);
        let mut coding = ds.format.to_string();
        if ds.layers.len() > 1 {
            coding += &format!(", {} layers", ds.layers.len());
        }
        let info = ImageInfo {
            format,
            width: w,
            height: h,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: 8,
            progressive: false,
            coding,
            embedded_preview: None,
            fast_regions: true,
            scales: &[1, 2, 4, 8],
        };
        let geo = Arc::new(GeoRef {
            crs: ds.crs.clone(),
            crs_note: ds.crs_note.clone(),
            to_map: crate::geo::from_kurbo_inverse(page.to_stored),
            bounds: ds.bounds().unwrap_or_default(),
            features: Some(index),
            facts: GeoFacts::Vector(crate::geo::VectorGeo {
                tile_meta,
                ..crate::geo::vector_facts(&ds)
            }),
        });
        Ok(Arc::new(MapSession {
            source,
            info,
            page,
            geo,
        }))
    }
}

struct MapSession {
    source: Arc<Source>,
    info: ImageInfo,
    page: DrawingPage,
    geo: Arc<GeoRef>,
}

impl Session for MapSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn alpha(&self) -> Option<crate::Alpha> {
        None
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let scale = req.scale.clamp(1, 8);
        let (sw, sh) = (
            self.info.width.div_ceil(scale),
            self.info.height.div_ceil(scale),
        );
        let (x, y, w, h) = match req.region {
            Some(r) => {
                let x = r.x.min(sw);
                let y = r.y.min(sh);
                (x, y, r.w.min(sw - x), r.h.min(sh - y))
            }
            None => (0, 0, sw, sh),
        };
        if w == 0 || h == 0 {
            return Err(DecodeError::Unsupported("empty region".into()));
        }
        let sheet = self
            .page
            .sheet()
            .ok_or(DecodeError::Malformed("no sheet".into()))?;
        let layers: Vec<bool> = self
            .page
            .prepared
            .scene
            .layers
            .iter()
            .map(|l| l.visible())
            .collect();
        let view = View {
            transform: Affine::translate((-f64::from(x), -f64::from(y)))
                * Affine::scale(1.0 / f64::from(scale))
                * self.page.to_stored,
            size: (w, h),
            dark: false,
            background: paper_colour(false),
            lineweights: false,
            dpi_scale: crate::drawing::raster_line_scale(f64::from(sw.max(sh))),
            layers: &layers,
        };
        let (data, _) =
            specular_vector::raster::render(sheet, &view).ok_or(DecodeError::TooLarge)?;
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        Ok(Raster {
            width: w,
            height: h,
            origin: (x, y),
            scale,
            layout: PixelLayout::Rgba,
            planes: vec![RasterPlane {
                width: w,
                height: h,
                stride: w as usize * 4,
                data,
                h_div: 1,
                v_div: 1,
            }],
            truncated: false,
        })
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let s = req.scale.clamp(1, 8);
        let px = match req.region {
            Some(r) => r.w as u64 * r.h as u64,
            None => self.info.width.div_ceil(s) as u64 * self.info.height.div_ceil(s) as u64,
        };
        px * 4 * 2
    }
    fn memory_bytes(&self) -> usize {
        self.page
            .prepared
            .sheets
            .iter()
            .flat_map(|s| s.passes.iter())
            .map(|p| p.geometry.heap_bytes())
            .sum()
    }
    fn page_label(&self) -> Option<PageLabel> {
        let s = self.page.sheet()?;
        Some(PageLabel::Sheet {
            name: s.name.as_str().into(),
            model: true,
        })
    }
    fn drawing(&self) -> Option<&DrawingPage> {
        Some(&self.page)
    }
    fn geo(&self) -> Option<Arc<GeoRef>> {
        Some(self.geo.clone())
    }
}

/// A GeoPackage tile pyramid at one zoom level, as one image.
struct TileSession {
    source: Arc<Source>,
    db: Arc<crate::sql_host::HostDb>,
    info: ImageInfo,
    zoom: i64,
    table: String,
    /// Tile size and the tile columns and rows of the mosaic.
    tile: (u32, u32),
    geo: Arc<GeoRef>,
    levels: Levels,
}

impl TileSession {
    fn open(
        source: Arc<Source>,
        db: Arc<crate::sql_host::HostDb>,
        ds: &Dataset,
        t: TileSet,
        format: Format,
    ) -> Result<Arc<dyn Session>> {
        let mut levels = t.levels.clone();
        levels.sort_by_key(|l| l.zoom);
        let level = levels
            .iter()
            .rev()
            .find(|l| {
                l.matrix_w * i64::from(l.tile_w) <= MOSAIC_MAX
                    && l.matrix_h * i64::from(l.tile_h) <= MOSAIC_MAX
            })
            .or(levels.first())
            .ok_or(DecodeError::Malformed("no tile matrix".into()))?
            .clone();
        let w = (level.matrix_w * i64::from(level.tile_w)).clamp(1, MOSAIC_MAX) as u32;
        let h = (level.matrix_h * i64::from(level.tile_h)).clamp(1, MOSAIC_MAX) as u32;
        // The tile matrix set's bounds: its top-left is the top-left of tile (0, 0).
        let to_map = Affine2 {
            a: level.pixel_x,
            b: 0.0,
            c: t.bounds[0],
            d: 0.0,
            e: -level.pixel_y,
            f: t.bounds[3],
        };
        let crs = ds.crs.clone().or_else(|| {
            u32::try_from(t.srs_id)
                .ok()
                .and_then(specular_gis::crs::from_epsg)
        });
        let tiles = specular_gis::gpkg::tile_count(db.as_ref(), &t.table, level.zoom).unwrap_or(0);
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
                "tiles, zoom {} of {}, {}×{} tiles",
                level.zoom,
                levels.len(),
                level.matrix_w,
                level.matrix_h
            ),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        let geo = Arc::new(GeoRef {
            crs,
            crs_note: ds.crs_note.clone(),
            to_map,
            bounds: t.bounds,
            features: None,
            facts: GeoFacts::Tiles(TilesGeo {
                table: t.table.clone(),
                identifier: t.identifier.clone(),
                levels: t.levels.clone(),
                zoom: level.zoom,
                tiles,
                tile_meta: None,
            }),
        });
        Ok(Arc::new(TileSession {
            source,
            db,
            info,
            zoom: level.zoom,
            table: t.table,
            tile: (level.tile_w, level.tile_h),
            geo,
            levels: Levels::new(),
        }))
    }

    /// The whole mosaic at full size: every tile decoded and placed (missing tiles stay
    /// transparent).
    fn mosaic(&self, cancel: &CancelToken) -> Result<Raster> {
        let tiles = specular_gis::gpkg::tiles(self.db.as_ref(), &self.table, self.zoom, 512 << 20)
            .map_err(DecodeError::Malformed)?;
        let (w, h) = (self.info.width as usize, self.info.height as usize);
        let (tw, th) = (self.tile.0 as usize, self.tile.1 as usize);
        let mut out = vec![0u8; w * h * 4];
        let registry = Registry::default();
        let mut truncated = false;
        for (col, row, bytes) in tiles {
            if cancel.is_cancelled() {
                return Err(DecodeError::Cancelled);
            }
            let (Ok(col), Ok(row)) = (usize::try_from(col), usize::try_from(row)) else {
                continue;
            };
            let (x0, y0) = (col * tw, row * th);
            if x0 >= w || y0 >= h {
                continue;
            }
            let decoded = crate::open_nested(
                &registry,
                "tile",
                bytes,
                &self.source,
                crate::tiles_dec::MAX_TILE_PIXELS,
            )
            .and_then(|s| s.decode(&DecodeRequest::full(1), cancel));
            let r = match decoded {
                Ok(r) if !r.layout.is_wide() => crate::gainmap::to_rgba8(&r),
                _ => {
                    truncated = true;
                    continue;
                }
            };
            let pl = &r.planes[0];
            let cw = (r.width as usize).min(tw).min(w - x0);
            for y in 0..(r.height as usize).min(th).min(h - y0) {
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

impl Session for TileSession {
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
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let _ = req;
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

    const GEOJSON: &str = r#"{"type":"FeatureCollection","features":[
      {"type":"Feature","properties":{"name":"square","n":1},
       "geometry":{"type":"Polygon","coordinates":[[[10,50],[11,50],[11,51],[10,51],[10,50]],
                                                  [[10.4,50.4],[10.6,50.4],[10.6,50.6],[10.4,50.6],[10.4,50.4]]]}},
      {"type":"Feature","properties":{"name":"road"},
       "geometry":{"type":"LineString","coordinates":[[10,50],[11,51]]}},
      {"type":"Feature","properties":{"name":"town"},
       "geometry":{"type":"Point","coordinates":[10.2,50.8]}}]}"#;

    /// GeoPackage through SQLite in the decode host (needs `specular-decode-host` built next to
    /// the test binaries' folder): more points than one page of rows, from disk and from
    /// memory; then a hostile layer (an endless recursive view) refused quickly.
    #[test]
    fn geopackage_through_the_host() {
        let dir = std::env::temp_dir().join(format!("specular-gpkg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.gpkg");
        specular_gis::fixtures::geopackage(&path, 60_000, false);
        let s = Registry::default()
            .open(Arc::new(Source::open(&path).unwrap()))
            .expect("opens");
        assert_eq!(s.info().format, Format::GeoPackage);
        let g = s.geo().unwrap();
        let idx = g.features.as_ref().expect("features");
        assert_eq!(idx.dataset.layers[0].features.len(), 60_000);
        assert_eq!(g.crs.as_ref().and_then(|c| c.epsg), Some(4326));
        // Not on disk (as inside an archive): a private copy goes to the host.
        let bytes = std::fs::read(&path).unwrap();
        let s = Registry::default()
            .open(Arc::new(Source::from_vec("p.gpkg", bytes)))
            .expect("opens from memory");
        let n = s.geo().unwrap().features.as_ref().unwrap().dataset.layers[0]
            .features
            .len();
        assert_eq!(n, 60_000);
        let bad = dir.join("bad.gpkg");
        specular_gis::fixtures::geopackage(&bad, 0, true);
        let t = std::time::Instant::now();
        let r = Registry::default().open(Arc::new(Source::open(&bad).unwrap()));
        assert!(r.is_err(), "the recursive view was read");
        assert!(t.elapsed() < std::time::Duration::from_secs(10));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Through the host: a feature view is a layer, and points committed only to the `-wal`
    /// file (the writer still has it open) are read, with neither file written.
    #[test]
    fn geopackage_view_and_wal_through_the_host() {
        let dir = std::env::temp_dir().join(format!("specular-gpkg-wal-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("v.gpkg");
        specular_gis::fixtures::geopackage_with_view(&path, 100);
        let writer = specular_gis::fixtures::geopackage_wal_writer(&path, 50);
        let wal = dir.join("v.gpkg-wal");
        let before = (std::fs::read(&path).unwrap(), std::fs::read(&wal).unwrap());
        let s = Registry::default()
            .open(Arc::new(Source::open(&path).unwrap()))
            .expect("opens");
        let g = s.geo().unwrap();
        let ds = &g.features.as_ref().expect("features").dataset;
        assert_eq!(ds.layers.len(), 2, "{:?}", ds.warnings);
        assert_eq!(ds.layers[0].features.len(), 150);
        assert_eq!(ds.layers[1].name, "Even places");
        assert_eq!(ds.layers[1].features.len(), 75);
        assert_eq!(
            (std::fs::read(&path).unwrap(), std::fs::read(&wal).unwrap()),
            before
        );
        drop(writer);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn geojson_opens_as_a_map() {
        let src = Arc::new(Source::from_vec("t.geojson", GEOJSON.as_bytes().to_vec()));
        let s = Registry::default().open(src).expect("opens");
        assert_eq!(s.info().format, Format::GeoJson);
        assert_eq!(s.info().width, crate::drawing::LONG_SIDE as u32);
        let g = s.geo().expect("georeferenced");
        assert_eq!(g.crs.as_ref().and_then(|c| c.epsg), Some(4326));
        // The town is a point: under the pointer at its stored position.
        let to_stored = g.to_map.inverse().unwrap();
        let (px, py) = to_stored.apply(10.2, 50.8);
        let (mx, my) = g.map_at(px, py);
        assert!((mx - 10.2).abs() < 1e-9 && (my - 50.8).abs() < 1e-9);
        let idx = g.features.as_ref().unwrap();
        let hit = idx.hit([mx, my], 0.001).expect("a feature");
        let attrs = idx.dataset.layers[hit.layer].attributes(hit.feature);
        assert!(
            attrs.iter().any(|(k, v)| k == "name" && v.text() == "town"),
            "{attrs:?}"
        );
        // Inside the hole: no polygon there, only nothing.
        assert!(idx.hit([10.45, 50.55], 1e-6).is_none());
        // Inside the square, away from the road.
        let h = idx.hit([10.9, 50.1 + 0.05], 1e-6).expect("the square");
        let a = idx.dataset.layers[h.layer].attributes(h.feature);
        assert!(a.iter().any(|(_, v)| v.text() == "square"));
        let r = s
            .decode(&DecodeRequest::full(8), &CancelToken::new())
            .expect("decodes");
        let ink = r.planes[0].data.chunks(4).filter(|p| p[0] < 200).count();
        assert!(ink > 100, "{ink}");
    }

    #[test]
    fn extension_without_signature_is_not_claimed() {
        // A `.geojson` that is not GeoJSON fails rather than opening as something else.
        let src = Arc::new(Source::from_vec("t.geojson", b"[1,2,3]".to_vec()));
        assert!(Registry::default().open(src).is_err());
    }
}
