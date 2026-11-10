//! Where an image or a map sits on the Earth ([`crate::Session::geo`]): its coordinate
//! reference system, the map coordinates of its stored pixels and, for vector maps, the
//! features under the pointer. GeoTIFF rasters, GeoPackage tile pyramids and the vector
//! formats of `specular-gis` (GeoJSON, Shapefile, GeoPackage, KML/KMZ, GPX) carry one.

use std::sync::Arc;

pub use specular_gis;
use specular_gis::crs::Crs;
use specular_gis::geotiff::{self, Affine2, GdalMetadata};
use specular_gis::model::TileLevel;
use specular_gis::scene::FeatureIndex;

/// The georeference of a session's stored pixels.
#[derive(Clone)]
pub struct GeoRef {
    pub crs: Option<Crs>,
    /// How the CRS was found, or why it was not (".prj missing", "assumed WGS 84").
    pub crs_note: Option<String>,
    /// Stored pixel coordinates (continuous, (0, 0) the top-left corner of the top-left
    /// pixel, y down) to map coordinates.
    pub to_map: Affine2,
    /// Map bounds: min x, min y, max x, max y.
    pub bounds: [f64; 4],
    /// The features of a vector map, for hover and click.
    pub features: Option<Arc<FeatureIndex>>,
    pub facts: GeoFacts,
}

impl std::fmt::Debug for GeoRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GeoRef")
            .field("crs", &self.crs.as_ref().map(|c| &c.name))
            .field("to_map", &self.to_map)
            .field("bounds", &self.bounds)
            .finish_non_exhaustive()
    }
}

/// What kind of geodata it is, with the facts the info panel lists.
#[derive(Clone, Debug)]
pub enum GeoFacts {
    Raster(RasterGeo),
    Vector(VectorGeo),
    Tiles(TilesGeo),
}

/// A GeoTIFF.
#[derive(Clone, Debug, Default)]
pub struct RasterGeo {
    /// Pixel size of each level (full resolution first).
    pub levels: Vec<(u32, u32)>,
    /// GDAL_NODATA.
    pub nodata: Option<f64>,
    pub pixel_is_point: bool,
    /// Ground control points without a transform.
    pub gcps: usize,
    pub citation: Option<String>,
    pub vertical: Option<String>,
    /// Every GeoKey, as text.
    pub keys: Vec<(String, String)>,
    /// GeoKey directory version.
    pub version: Option<String>,
    pub gdal: GdalMetadata,
    /// Shown as values through the science view (a single-band elevation or measurement).
    pub values: bool,
    /// GDAL scale and offset of band 1 (physical = stored × scale + offset).
    pub scale_offset: Option<(f64, f64)>,
    /// GDAL unit of band 1 ("m", "metre").
    pub unit: Option<String>,
    /// GDAL description of band 1.
    pub band_name: Option<String>,
    /// What `keys` are: "" for GeoKeys, "header" for a format's header lines (ENVI, ASCII
    /// grid, DTED, ERDAS IMAGINE).
    pub keys_kind: &'static str,
    /// The companion files the georeference came from ("scene.jgw", "scene.prj").
    pub sidecars: Vec<String>,
}

/// A vector map.
#[derive(Clone, Debug, Default)]
pub struct VectorGeo {
    pub format: &'static str,
    pub layers: Vec<LayerFacts>,
    /// The text encoding of the attributes (Shapefile .cpg or DBF code page).
    pub encoding: Option<String>,
    pub warnings: Vec<String>,
    pub features: usize,
    pub vertices: usize,
    /// Tile pyramids beside the features (GeoPackage), by table.
    pub tile_tables: Vec<String>,
    /// What a vector tile set (MBTiles, PMTiles) says about itself.
    pub tile_meta: Option<specular_gis::tiles::TileMeta>,
}

#[derive(Clone, Debug, Default)]
pub struct LayerFacts {
    pub name: String,
    pub features: usize,
    /// Geometry types with their counts, most first.
    pub types: Vec<(&'static str, usize)>,
    pub fields: usize,
}

/// A GeoPackage tile pyramid.
#[derive(Clone, Debug, Default)]
pub struct TilesGeo {
    pub table: String,
    pub identifier: Option<String>,
    pub levels: Vec<TileLevel>,
    /// The zoom level shown.
    pub zoom: i64,
    pub tiles: usize,
    /// What a web tile set (MBTiles, PMTiles) says about itself.
    pub tile_meta: Option<specular_gis::tiles::TileMeta>,
}

impl GeoRef {
    /// Map coordinates of a stored pixel position.
    pub fn map_at(&self, x: f64, y: f64) -> (f64, f64) {
        self.to_map.apply(x, y)
    }

    /// Ground metres per stored pixel along x near a stored position: `None` without a
    /// CRS or with one in unknown units.
    pub fn metres_per_pixel(&self, x: f64, y: f64) -> Option<f64> {
        let crs = self.crs.as_ref()?;
        let (mx, my) = self.map_at(x, y);
        let m = crs.metres_per_unit_x(mx, my)?;
        let (px, _) = self.to_map.pixel_size();
        let r = m * px;
        (r.is_finite() && r > 0.0).then_some(r)
    }

    /// The ground distance in metres from stored pixel (`x`, `y`) to the next one east along
    /// the row and north down the column (negative when rows run south), for slopes
    /// (hillshade). `None` without a CRS, or for a rotated or sheared transform.
    pub fn ground_step(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let crs = self.crs.as_ref()?;
        let t = &self.to_map;
        if !t.is_north_up() {
            return None;
        }
        let (mx, my) = self.map_at(x, y);
        let sx = crs.metres_per_unit_x(mx, my)? * t.a;
        let sy = crs.metres_per_unit_y(mx, my)? * t.e;
        (sx.is_finite() && sy.is_finite() && sx != 0.0 && sy != 0.0).then_some((sx, sy))
    }
}

/// The GeoTIFF georeference of a TIFF page, from the tags `specular-tiff` collects;
/// `None` when the page has no GeoTIFF transform.
pub(crate) fn from_geotiff(
    tags: &specular_tiff::GeoTags,
    width: u32,
    height: u32,
    levels: Vec<(u32, u32)>,
) -> Option<GeoRef> {
    let info = geotiff::interpret(
        &tags.pixel_scale,
        &tags.tiepoints,
        &tags.transformation,
        &tags.key_directory,
        &tags.double_params,
        &tags.ascii_params,
    );
    let to_map = info.transform?;
    let gdal = tags
        .gdal_metadata
        .as_deref()
        .map(GdalMetadata::parse)
        .unwrap_or_default();
    let nodata = tags.gdal_nodata.as_deref().and_then(geotiff::parse_nodata);
    let bounds = bounds_of(&to_map, width, height);
    let (scale_offset, unit, band_name) = band_roles(&gdal);
    Some(GeoRef {
        crs: info.crs,
        crs_note: None,
        to_map,
        bounds,
        features: None,
        facts: GeoFacts::Raster(RasterGeo {
            levels,
            nodata,
            pixel_is_point: info.pixel_is_point,
            gcps: info.gcps,
            citation: info.citation,
            vertical: info.vertical,
            keys: info.keys,
            version: info.version,
            gdal,
            values: false,
            scale_offset,
            unit,
            band_name,
            ..Default::default()
        }),
    })
}

/// Map bounds (min x, min y, max x, max y) of a `width × height` raster.
pub(crate) fn bounds_of(to_map: &Affine2, width: u32, height: u32) -> [f64; 4] {
    let (w, h) = (f64::from(width), f64::from(height));
    let corners = [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)].map(|(x, y)| to_map.apply(x, y));
    [
        corners.iter().map(|c| c.0).fold(f64::INFINITY, f64::min),
        corners.iter().map(|c| c.1).fold(f64::INFINITY, f64::min),
        corners
            .iter()
            .map(|c| c.0)
            .fold(f64::NEG_INFINITY, f64::max),
        corners
            .iter()
            .map(|c| c.1)
            .fold(f64::NEG_INFINITY, f64::max),
    ]
}

/// Band 1's GDAL scale and offset (when not the identity), unit and description.
pub(crate) fn band_roles(
    gdal: &GdalMetadata,
) -> (Option<(f64, f64)>, Option<String>, Option<String>) {
    let scale_offset = match (gdal.band_role(0, "scale"), gdal.band_role(0, "offset")) {
        (None, None) => None,
        (s, o) => {
            let s = s.and_then(|v| v.trim().parse().ok()).unwrap_or(1.0);
            let o = o.and_then(|v| v.trim().parse().ok()).unwrap_or(0.0);
            (s != 1.0 || o != 0.0).then_some((s, o))
        }
    };
    let text = |role| {
        gdal.band_role(0, role)
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .map(str::to_string)
    };
    (scale_offset, text("unittype"), text("description"))
}

/// A raster's georeference from a transform and CRS found outside GeoTIFF tags (a format's
/// own header, or sidecar files).
pub(crate) fn raster(
    to_map: Affine2,
    crs: Option<Crs>,
    crs_note: Option<String>,
    (width, height): (u32, u32),
    facts: RasterGeo,
) -> GeoRef {
    GeoRef {
        crs,
        crs_note,
        bounds: bounds_of(&to_map, width, height),
        to_map,
        features: None,
        facts: GeoFacts::Raster(facts),
    }
}

/// Whether a transform maps pixels to a plane (finite, not singular).
fn usable(t: &Affine2) -> bool {
    let det = t.a * t.e - t.b * t.d;
    [t.a, t.b, t.c, t.d, t.e, t.f].iter().all(|v| v.is_finite()) && det != 0.0
}

/// The georeference of a raster at `path` from its sidecar files: a GDAL `.aux.xml`
/// (geotransform, SRS, no-data, band metadata and statistics), else a world file (when
/// `world`), with a `.prj` for the CRS. `None` without a transform.
pub fn from_sidecars(
    path: &std::path::Path,
    width: u32,
    height: u32,
    levels: Vec<(u32, u32)>,
    world: bool,
) -> Option<GeoRef> {
    let s = specular_gis::sidecar::georef(path, world);
    let to_map = s.transform.filter(usable)?;
    let gdal = s.aux.as_ref().map(|a| a.to_gdal()).unwrap_or_default();
    let nodata = s
        .aux
        .as_ref()
        .and_then(|a| a.band(1))
        .and_then(|b| b.nodata);
    let (scale_offset, unit, band_name) = band_roles(&gdal);
    Some(raster(
        to_map,
        s.crs,
        None,
        (width, height),
        RasterGeo {
            levels,
            nodata,
            gdal,
            scale_offset,
            unit,
            band_name,
            sidecars: s.files,
            ..Default::default()
        },
    ))
}

/// A sidecar georeference of a TIFF completed by the TIFF's own GDAL tags (GDAL_NODATA,
/// GDAL_METADATA), which GDAL reads with or without GeoKeys; the `.aux.xml` wins.
pub(crate) fn with_gdal_tags(mut g: GeoRef, tags: Option<&specular_tiff::GeoTags>) -> GeoRef {
    let (Some(tags), GeoFacts::Raster(r)) = (tags, &mut g.facts) else {
        return g;
    };
    if r.nodata.is_none() {
        r.nodata = tags.gdal_nodata.as_deref().and_then(geotiff::parse_nodata);
    }
    if r.gdal.items.is_empty()
        && let Some(m) = tags.gdal_metadata.as_deref()
    {
        r.gdal = GdalMetadata::parse(m);
        (r.scale_offset, r.unit, r.band_name) = band_roles(&r.gdal);
    }
    g
}

/// Whether a session's file is a regular file on disk (not in memory or an archive).
pub fn on_disk_path(src: &specular_core::Source) -> bool {
    on_disk(src)
}

/// Whether a session's file can have sidecars: read from disk (not from memory or an
/// archive entry), a regular file.
pub(crate) fn on_disk(src: &specular_core::Source) -> bool {
    src.modified().is_some()
        && !src.is_folder()
        && std::fs::metadata(src.path()).is_ok_and(|m| m.is_file() && m.len() == src.len() as u64)
}

/// The sidecar georeference of a session that has none of its own (a JPEG or PNG beside a
/// world file), for [`crate::Registry::open`]: only for plain rasters on disk.
pub(crate) fn sidecar_for(session: &dyn crate::Session) -> Option<GeoRef> {
    use crate::Format as F;
    let info = session.info();
    let plain = matches!(
        info.format,
        F::Jpeg
            | F::Png
            | F::Gif
            | F::WebP
            | F::Avif
            | F::Jxl
            | F::Jpeg2000
            | F::Heic
            | F::Bmp
            | F::Tga
            | F::Qoi
            | F::Pnm
            | F::JpegLs
            | F::JpegXr
            | F::Pcx
            | F::Sgi
            | F::SunRaster
            | F::Exr
            | F::Psd
    );
    if !plain || session.science().is_some() || !on_disk(session.source()) {
        return None;
    }
    // World files describe the stored pixels, before any EXIF rotation.
    from_sidecars(
        session.source().path(),
        info.width,
        info.height,
        vec![(info.width, info.height)],
        true,
    )
}
/// The facts of a vector dataset.
pub(crate) fn vector_facts(ds: &specular_gis::Dataset) -> VectorGeo {
    let layers = ds
        .layers
        .iter()
        .map(|l| {
            let mut types: Vec<(&'static str, usize)> = Vec::new();
            for f in &l.features {
                if let Some(g) = &f.geometry {
                    let t = g.type_name();
                    match types.iter_mut().find(|(n, _)| *n == t) {
                        Some((_, c)) => *c += 1,
                        None => types.push((t, 1)),
                    }
                }
            }
            types.sort_by_key(|t| std::cmp::Reverse(t.1));
            LayerFacts {
                name: l.name.clone(),
                features: l.features.len(),
                types,
                fields: l.fields().len(),
            }
        })
        .collect();
    VectorGeo {
        format: ds.format,
        layers,
        encoding: ds.encoding.clone(),
        warnings: ds.warnings.clone(),
        features: ds.feature_count(),
        vertices: ds
            .layers
            .iter()
            .flat_map(|l| l.features.iter())
            .filter_map(|f| f.geometry.as_ref())
            .map(|g| g.vertex_count())
            .sum(),
        tile_tables: ds.tiles.iter().map(|t| t.table.clone()).collect(),
        tile_meta: None,
    }
}

/// A kurbo affine (map, y up → stored pixels) inverted as an [`Affine2`] (stored → map).
pub(crate) fn from_kurbo_inverse(to_stored: specular_vector::kurbo::Affine) -> Affine2 {
    let k = to_stored.inverse().as_coeffs();
    Affine2 {
        a: k[0],
        b: k[2],
        c: k[4],
        d: k[1],
        e: k[3],
        f: k[5],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kurbo_inverse_round_trips() {
        use specular_vector::kurbo::Affine;
        let to_stored = Affine::new([2.0, 0.0, 0.0, -2.0, 10.0, 500.0]);
        let m = from_kurbo_inverse(to_stored);
        let p = to_stored * specular_vector::kurbo::Point::new(3.0, 7.0);
        let (x, y) = m.apply(p.x, p.y);
        assert!((x - 3.0).abs() < 1e-9 && (y - 7.0).abs() < 1e-9, "{x} {y}");
    }

    #[test]
    fn geotiff_utm() {
        // UTM 33N, 30 m pixels, origin (500000, 4000000).
        let tags = specular_tiff::GeoTags {
            pixel_scale: vec![30.0, 30.0, 0.0],
            tiepoints: vec![0.0, 0.0, 0.0, 500_000.0, 4_000_000.0, 0.0],
            transformation: vec![],
            key_directory: vec![1, 1, 0, 2, 1024, 0, 1, 1, 3072, 0, 1, 32633],
            double_params: vec![],
            ascii_params: String::new(),
            gdal_metadata: None,
            gdal_nodata: Some("-9999".into()),
        };
        let g = from_geotiff(&tags, 100, 50, vec![(100, 50)]).expect("georeferenced");
        assert_eq!(g.crs.as_ref().and_then(|c| c.epsg), Some(32633));
        assert_eq!(g.bounds, [500_000.0, 3_998_500.0, 503_000.0, 4_000_000.0]);
        let (lon, lat) = g
            .crs
            .as_ref()
            .unwrap()
            .to_lonlat(500_000.0, 4_000_000.0)
            .unwrap();
        assert!(
            (lon - 15.0).abs() < 1e-9 && (lat - 36.14).abs() < 0.01,
            "{lon} {lat}"
        );
        let m = g.metres_per_pixel(0.0, 0.0).unwrap();
        assert!((m - 30.0).abs() < 1e-9);
        match g.facts {
            GeoFacts::Raster(r) => assert_eq!(r.nodata, Some(-9999.0)),
            _ => panic!(),
        }
    }
}
