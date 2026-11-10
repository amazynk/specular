//! FlatGeobuf, GeoParquet/GeoArrow, GML and OpenStreetMap files against the oracle
//! (`gen/formats_oracle.py`: GDAL through pyogrio, GeoPandas through pyarrow and shapely):
//! feature counts, attributes and coordinates within 1 mm or 1e-8 degrees.
//!
//! The corpus comes from `tools/corpus/fetch-gis-formats.sh`; it is read from
//! SPECULAR_GIS_FORMATS_CORPUS, else tools/corpus/gis-formats, and the test skips without
//! its `expected/formats.jsonl`.

mod common;

use specular_gis::VectorKind;
use std::path::{Path, PathBuf};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_GIS_FORMATS_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/gis-formats")
        });
    if dir.join("expected/formats.jsonl").is_file() {
        Some(dir)
    } else {
        eprintln!("no formats oracle in {}; skipping", dir.display());
        None
    }
}

fn kind_of(path: &Path) -> Option<VectorKind> {
    let head = std::fs::read(path).ok()?;
    let ext = path.extension()?.to_str()?;
    specular_gis::kind_from_extension(ext)
        .or_else(|| specular_gis::sniff(&head[..head.len().min(4096)]))
}

const XSD: &str = "GDAL builds the layers from the .xsd or .gfs schema beside the file";
const TOPOLOGY: &str = "GML topology (TopoCurve/TopoSurface through edges and faces) is not read";
const TRUNCATED: &str = "a truncated document: GDAL keeps the features read before the break";

/// Files the oracle reads differently by design, with the reason.
const VECTOR_SKIP: &[(&str, &str)] = &[
    ("gdal/gml/GmlTopo-sample.xml", TOPOLOGY),
    ("gdal/gml/sample_gml_face_hole_negative_no.xml", TOPOLOGY),
    ("gdal/gml/curveProperty.xml", TOPOLOGY),
    ("gdal/gml/testfeaturepropertylist.gml", XSD),
    ("gdal/gml/wfsjointlayer.gml", XSD),
    ("gdal/gml/gmlsubfeature.gml", XSD),
    ("gdal/gml/ogr_gml_56.gml", XSD),
    ("gdal/gml/multiple_geometry_fields_srs_detection.gml", XSD),
    ("gdal/gml/min_example/minimal_example.gml", XSD),
    (
        "gdal/gml/too_nested.gml",
        "GDAL stops at its nesting limit and reads no feature",
    ),
    ("gdal/gml/huge_attribute_start.gml", TRUNCATED),
    ("gdal/gml/huge_geom_start.gml", TRUNCATED),
    (
        "gdal/gml/cswresults.xml",
        "a CSW catalogue response, not a feature collection",
    ),
    (
        "gdal/gml/paris_typical_strike_demonstration.xml",
        "an OpenLS route response: GDAL reads it through a schema of its own",
    ),
    (
        "gdal/gml/fake_mtkgml.xml",
        "Finnish MTK GML: GDAL has a reader of its own for it",
    ),
];

/// Geometries the oracle reads differently by design: (file, feature index, reason).
const GEOMETRY_SKIP: &[(&str, usize, &str)] = &[];

/// Layers the oracle makes that the viewer does not.
const LAYER_SKIP: &[(&str, &str)] = &[];

#[test]
fn formats_match_the_oracle() {
    let Some(dir) = corpus() else { return };
    let r = common::compare(
        &dir,
        common::records(&dir, "formats.jsonl"),
        kind_of,
        &common::Skips {
            vector: VECTOR_SKIP,
            geometry: GEOMETRY_SKIP,
            layer: LAYER_SKIP,
        },
    );
    r.print("formats");
    assert!(r.mismatches.is_empty(), "{} mismatches", r.mismatches.len());
}
