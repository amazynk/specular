//! Geospatial data for Specular.
//!
//! - [`crs`], [`wkt`], [`proj`]: coordinate reference systems: a small EPSG table, names
//!   and parameters from WKT1/WKT2/PROJJSON, and exact in-tree forward and inverse
//!   projections (Transverse Mercator/UTM, Mercator, Web Mercator, Lambert conformal
//!   conic, Albers equal-area), so map coordinates read out as longitude/latitude.
//! - [`geotiff`]: GeoTIFF GeoKeys and the GDAL metadata and nodata tags, read from the TIFF
//!   tags `specular-tiff` collects.
//! - Vector readers into one feature [`model`]: [`geojson`], [`shp`] (Shapefile with DBF,
//!   PRJ and CPG), [`kml`] (KML and KMZ), [`gpx`] and [`gpkg`] (GeoPackage features and
//!   tile pyramids).
//! - [`scene`]: a dataset as a `specular-vector` drawing, and a hit-test index;
//!   [`style`]: the feature styles a file carries (simplestyle, KML styles).
//! - [`sidecar`]: companion files found beside an opened file, with [`worldfile`] (world
//!   files and `.prj`) and [`auxxml`] (GDAL `.aux.xml`) giving any raster a georeference.
//! - Gridded rasters: [`asc`] (Esri ASCII grid), [`dted`] (DTED levels 0–2), [`envi`]
//!   (ENVI and ESRI `.hdr` labelled raw bands, BSQ/BIL/BIP), [`hfa`] (ERDAS IMAGINE), with
//!   [`rawgrid`] reading their bands as values; [`ers`] (ER Mapper), [`saga`] (SAGA grids),
//!   [`aig`] (Arc/Info binary grids), [`bsb`] (BSB/KAP nautical charts) and [`vrt`] (GDAL
//!   virtual rasters, parsed; their sources are read by the decoder).
//! - More vectors: [`s57`] (S-57 ENC cells over [`iso8211`]), [`topojson`], [`igc`] (glider
//!   flight logs) and [`nmea`] (NMEA 0183 GPS logs).
//!
//! - [`epsg`] and [`datum`]: the EPSG dataset's geographic and projected CRSs and their
//!   Helmert shifts to WGS 84, so coordinates read out on WGS 84.
//! - Web map tiles: [`mbtiles`], [`pmtiles`] (version 3, by byte ranges) and [`mvt`]
//!   (Mapbox Vector Tiles), with the XYZ grid in [`tiles`].
//!
//! Nothing here touches the network (a PMTiles archive on a server is read through a
//! [`pmtiles::RangeRead`] the caller supplies).

pub mod aig;
pub mod arrow;
pub mod asc;
pub mod auxxml;
pub mod bsb;
pub mod coastline;
pub mod codec;
pub mod cog;
pub mod crs;
pub mod datum;
pub mod dted;
pub mod envi;
pub mod epsg;
pub mod epsg_axes;
pub mod ers;
pub mod fgb;
pub mod filegdb;
pub mod fit;
pub mod fixtures;
pub mod flatbuf;
pub mod geojson;
pub mod geoparquet;
pub mod geotiff;
pub mod gml;
pub mod gpkg;
pub mod gpx;
pub mod hfa;
pub mod igc;
pub mod iso8211;
pub mod kml;
pub mod mbtiles;
pub mod mitab;
pub mod model;
pub mod mvt;
pub mod nitf;
pub mod nmea;
pub mod osm;
pub mod parquet;
pub mod pmtiles;
pub mod proj;
pub mod rawgrid;
pub mod s57;
pub mod s57_catalogue;
pub mod saga;
pub mod scene;
pub mod sentinel2;
pub mod shp;
pub mod sidecar;
pub mod sql;
pub mod stac;
pub mod style;
pub mod tcx;
pub mod tiles;
pub mod topojson;
pub mod usgsdem;
pub mod vrt;
pub mod wkt;
pub mod wkt_geom;
pub mod worldfile;

pub use model::{Dataset, Geometry, Layer, Value};
use std::path::Path;

/// The vector formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VectorKind {
    GeoJson,
    Shapefile,
    GeoPackage,
    Kml,
    Kmz,
    Gpx,
    FlatGeobuf,
    GeoParquet,
    GeoArrow,
    Gml,
    Osm,
    OsmPbf,
    Fit,
    Tcx,
    FileGdb,
    MapInfoTab,
    MapInfoMif,
    S57,
    TopoJson,
    Igc,
    Nmea,
}

impl VectorKind {
    pub fn name(self) -> &'static str {
        match self {
            VectorKind::GeoJson => "GeoJSON",
            VectorKind::Shapefile => "Shapefile",
            VectorKind::GeoPackage => "GeoPackage",
            VectorKind::Kml => "KML",
            VectorKind::Kmz => "KMZ",
            VectorKind::Gpx => "GPX",
            VectorKind::FlatGeobuf => "FlatGeobuf",
            VectorKind::GeoParquet => "GeoParquet",
            VectorKind::GeoArrow => "GeoArrow",
            VectorKind::Gml => "GML",
            VectorKind::Osm => "OSM XML",
            VectorKind::OsmPbf => "OSM PBF",
            VectorKind::Fit => "FIT",
            VectorKind::Tcx => "TCX",
            VectorKind::FileGdb => "File Geodatabase",
            VectorKind::MapInfoTab => "MapInfo TAB",
            VectorKind::MapInfoMif => "MapInfo MIF",
            VectorKind::S57 => "S-57",
            VectorKind::TopoJson => "TopoJSON",
            VectorKind::Igc => "IGC",
            VectorKind::Nmea => "NMEA",
        }
    }
}

/// The format from the first bytes, else (for GeoJSON's bare `.json` cousins) nothing.
pub fn sniff(head: &[u8]) -> Option<VectorKind> {
    if shp::sniff(head) {
        Some(VectorKind::Shapefile)
    } else if s57::sniff(head) {
        Some(VectorKind::S57)
    } else if fgb::sniff(head) {
        Some(VectorKind::FlatGeobuf)
    } else if parquet::sniff(head) {
        Some(VectorKind::GeoParquet)
    } else if arrow::sniff_file(head) {
        Some(VectorKind::GeoArrow)
    } else if gpkg::sniff(head) {
        Some(VectorKind::GeoPackage)
    } else if kml::sniff_kmz(head) {
        Some(VectorKind::Kmz)
    } else if kml::sniff(head) {
        Some(VectorKind::Kml)
    } else if gpx::sniff(head) {
        Some(VectorKind::Gpx)
    } else if mitab::sniff_tab(head) {
        Some(VectorKind::MapInfoTab)
    } else if mitab::sniff_mif(head) {
        Some(VectorKind::MapInfoMif)
    } else if fit::sniff(head) {
        Some(VectorKind::Fit)
    } else if tcx::sniff(head) {
        Some(VectorKind::Tcx)
    } else if osm::sniff_pbf(head) {
        Some(VectorKind::OsmPbf)
    } else if osm::sniff_xml(head) {
        Some(VectorKind::Osm)
    } else if gml::sniff(head) {
        Some(VectorKind::Gml)
    } else if igc::sniff(head) {
        Some(VectorKind::Igc)
    } else if nmea::sniff(head) {
        Some(VectorKind::Nmea)
    } else if topojson::sniff(head) {
        Some(VectorKind::TopoJson)
    } else if geojson::sniff(head) {
        Some(VectorKind::GeoJson)
    } else {
        None
    }
}

/// The format from a file name's extension.
pub fn kind_from_extension(ext: &str) -> Option<VectorKind> {
    Some(match ext.to_ascii_lowercase().as_str() {
        "geojson" => VectorKind::GeoJson,
        "shp" => VectorKind::Shapefile,
        "gpkg" => VectorKind::GeoPackage,
        "kml" => VectorKind::Kml,
        "kmz" => VectorKind::Kmz,
        "gpx" => VectorKind::Gpx,
        "fgb" => VectorKind::FlatGeobuf,
        "gml" => VectorKind::Gml,
        "osm" => VectorKind::Osm,
        "pbf" => VectorKind::OsmPbf,
        "fit" => VectorKind::Fit,
        "tcx" => VectorKind::Tcx,
        "gdb" | "gdbtable" | "gdbtablx" => VectorKind::FileGdb,
        "tab" => VectorKind::MapInfoTab,
        "mif" => VectorKind::MapInfoMif,
        "000" => VectorKind::S57,
        "topojson" => VectorKind::TopoJson,
        "igc" => VectorKind::Igc,
        "nmea" => VectorKind::Nmea,
        "parquet" | "geoparquet" => VectorKind::GeoParquet,
        "arrow" | "feather" | "arrows" | "ipc" => VectorKind::GeoArrow,
        _ => return None,
    })
}

/// XML bytes as text: UTF-8 (a BOM dropped), UTF-16 with a BOM, else the encoding the XML
/// declaration names (ISO-8859-1, Windows-1252, …).
pub(crate) fn xml_text(data: &[u8]) -> Result<std::borrow::Cow<'_, str>, String> {
    if let Some((enc, bom)) = encoding_rs::Encoding::for_bom(data) {
        let (t, _, bad) = enc.decode(&data[bom..]);
        if bad && enc == encoding_rs::UTF_8 {
            return Err("the XML is not valid UTF-8".into());
        }
        return Ok(t.into_owned().into());
    }
    let head = String::from_utf8_lossy(&data[..data.len().min(200)]);
    let declared = head
        .split_once("encoding=")
        .and_then(|(_, r)| {
            let q = r.chars().next()?;
            r[1..].split(q).next().map(str::to_string)
        })
        .and_then(|e| encoding_rs::Encoding::for_label(e.trim().as_bytes()));
    match declared {
        Some(enc) if enc != encoding_rs::UTF_8 => Ok(enc.decode(data).0.into_owned().into()),
        _ => std::str::from_utf8(data)
            .map(std::borrow::Cow::Borrowed)
            .map_err(|_| "the XML is not valid UTF-8".into()),
    }
}

/// Gzip-compressed data inflated (capped), else the data as is.
pub(crate) fn gunzip(data: &[u8]) -> Result<std::borrow::Cow<'_, [u8]>, String> {
    if data.starts_with(&[0x1F, 0x8B]) {
        codec::inflate(data, 1 << 31).map(std::borrow::Cow::Owned)
    } else {
        Ok(std::borrow::Cow::Borrowed(data))
    }
}

/// A sidecar next to `path` with extension `ext`, matched case-insensitively.
fn sidecar(path: &Path, ext: &str) -> Option<Vec<u8>> {
    // Shapefile parts can be large (a .dbf of millions of records).
    std::fs::read(sidecar::find(path, ext)?).ok()
}

/// Reads a vector file. `data` is the file's bytes; Shapefile sidecars are read from `path`.
/// GeoPackage is an SQLite database: [`gpkg::read`] reads it through an [`sql::SqlDb`].
pub fn read(kind: VectorKind, path: &Path, data: &[u8]) -> Result<Dataset, String> {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("layer")
        .to_string();
    match kind {
        VectorKind::GeoJson => geojson::read(data, &name),
        // A Shapefile's index (.shx) starts as its .shp does: open the shapes beside it.
        VectorKind::Shapefile
            if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("shx")) =>
        {
            let missing = || "the index (.shx) of a Shapefile whose .shp is missing".to_string();
            let p = sidecar::find(path, "shp").ok_or_else(missing)?;
            let shp = std::fs::read(&p).map_err(|_| missing())?;
            read(kind, &p, &shp)
        }
        VectorKind::Shapefile => {
            let shx = sidecar(path, "shx");
            let dbf = sidecar(path, "dbf");
            let prj = sidecar(path, "prj").map(|b| String::from_utf8_lossy(&b).into_owned());
            let cpg = sidecar(path, "cpg").map(|b| String::from_utf8_lossy(&b).into_owned());
            let mut ds = shp::read(
                shp::Parts {
                    shp: data,
                    shx: shx.as_deref(),
                    dbf: dbf.as_deref(),
                    prj: prj.as_deref(),
                    cpg: cpg.as_deref(),
                },
                &name,
            )?;
            if dbf.is_none() {
                ds.warn("no attribute table (.dbf) beside the file");
            }
            if prj.is_none() {
                ds.warn("no projection file (.prj) beside the file");
            }
            Ok(ds)
        }
        VectorKind::GeoPackage => {
            Err("a GeoPackage is read through its SQLite database (gpkg::read)".into())
        }
        VectorKind::Kml => kml::read(data, &name),
        VectorKind::Kmz => kml::read_kmz(data, &name),
        VectorKind::Gpx => gpx::read(data),
        VectorKind::FlatGeobuf => fgb::read(data, &name),
        VectorKind::GeoParquet => geoparquet::read(data, &name),
        VectorKind::GeoArrow => geoparquet::read_ipc(data, &name),
        VectorKind::Gml => gml::read(&gunzip(data)?, &name),
        VectorKind::Osm => osm::read_xml_file(&gunzip(data)?),
        VectorKind::OsmPbf => osm::read_pbf_file(data),
        VectorKind::Fit => fit::read(data, &name),
        VectorKind::Tcx => tcx::read(data, &name),
        VectorKind::FileGdb => filegdb::read(path),
        VectorKind::MapInfoTab => mitab::read_tab(path, data),
        VectorKind::MapInfoMif => mitab::read_mif(path, data),
        VectorKind::S57 => s57::read(data),
        VectorKind::TopoJson => topojson::read(data),
        VectorKind::Igc => igc::read(data, &name),
        VectorKind::Nmea => nmea::read(data, &name),
    }
}
