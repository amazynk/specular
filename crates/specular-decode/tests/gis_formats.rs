//! The vector formats of the GIS formats lane open through the registry as maps: the format
//! is recognised by content (or folder), the session is georeferenced with its features, and
//! a reduced decode draws ink. Small documents are written here; the binary formats come
//! from the corpus (`tools/corpus/fetch-gis-formats.sh`, read from
//! SPECULAR_GIS_FORMATS_CORPUS or tools/corpus/gis-formats; skipped without it).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{CancelToken, Source};
use specular_decode::geo::GeoFacts;
use specular_decode::{DecodeRequest, Format, Registry, Session};

fn check(s: &dyn Session, want: Format, min_features: usize) {
    assert_eq!(s.info().format, want);
    let g = s.geo().expect("georeferenced");
    let GeoFacts::Vector(v) = &g.facts else {
        panic!("not a vector map")
    };
    assert!(v.features >= min_features, "{} features", v.features);
    let r = s
        .decode(&DecodeRequest::full(4), &CancelToken::new())
        .expect("decodes");
    // Ink: pixels unlike the paper (the corner).
    let d = &r.planes[0].data;
    let paper = &d[..4];
    let ink = d.chunks(4).filter(|p| *p != paper).count();
    assert!(ink > 0, "nothing drawn");
}

fn open_bytes(name: &str, data: &[u8]) -> Arc<dyn Session> {
    Registry::default()
        .open(Arc::new(Source::from_vec(name, data.to_vec())))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn text_formats_open_by_content() {
    let gml = br#"<?xml version="1.0"?>
<wfs:FeatureCollection xmlns:wfs="http://www.opengis.net/wfs/2.0" xmlns:gml="http://www.opengis.net/gml/3.2" xmlns:a="urn:a">
 <wfs:member><a:road gml:id="r1"><a:name>A1</a:name><a:geom><gml:LineString srsName="urn:ogc:def:crs:EPSG::4326"><gml:posList>46 7 47 8</gml:posList></gml:LineString></a:geom></a:road></wfs:member>
</wfs:FeatureCollection>"#;
    let s = open_bytes("roads.xml", gml);
    check(s.as_ref(), Format::Gml, 1);
    assert_eq!(
        s.geo().unwrap().crs.as_ref().and_then(|c| c.epsg),
        Some(4326)
    );

    let osm = br#"<?xml version="1.0"?><osm version="0.6">
 <node id="1" lat="46" lon="7"/><node id="2" lat="46.1" lon="7.2"/>
 <way id="3"><nd ref="1"/><nd ref="2"/><tag k="highway" v="track"/></way></osm>"#;
    check(open_bytes("x.osm", osm).as_ref(), Format::OsmXml, 1);

    let tcx = br#"<?xml version="1.0"?>
<TrainingCenterDatabase xmlns="http://www.garmin.com/xmlschemas/TrainingCenterDatabase/v2">
 <Activities><Activity Sport="Biking"><Id>2024-05-01T10:00:00Z</Id><Lap StartTime="2024-05-01T10:00:00Z"><Track>
  <Trackpoint><Time>2024-05-01T10:00:00Z</Time><Position><LatitudeDegrees>46.0</LatitudeDegrees><LongitudeDegrees>7.0</LongitudeDegrees></Position></Trackpoint>
  <Trackpoint><Time>2024-05-01T10:00:10Z</Time><Position><LatitudeDegrees>46.01</LatitudeDegrees><LongitudeDegrees>7.02</LongitudeDegrees></Position></Trackpoint>
 </Track></Lap></Activity></Activities></TrainingCenterDatabase>"#;
    check(open_bytes("ride.tcx", tcx).as_ref(), Format::Tcx, 1);
}

#[test]
fn mif_with_its_mid_opens() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("gis-formats-mif");
    std::fs::create_dir_all(&dir).unwrap();
    let mif = dir.join("towns.mif");
    std::fs::write(
        &mif,
        "Version 300\nCharset \"Neutral\"\nDelimiter \",\"\nCoordSys Earth Projection 1, 104\nColumns 1\n  name Char(20)\nData\n\nPoint 7.45 46.95\nLine 7 46 8 47\n",
    )
    .unwrap();
    std::fs::write(dir.join("towns.mid"), "\"Bern\"\n\"Road\"\n").unwrap();
    let s = Registry::default().open_path(&mif).expect("opens");
    check(s.as_ref(), Format::MapInfoMif, 2);
}

#[test]
fn kml_ground_overlay_opens_georeferenced() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("gis-formats-overlay");
    std::fs::create_dir_all(&dir).unwrap();
    let png = specular_decode::geo::specular_gis::fixtures::png_rgb(40, 20, |x, y| {
        [(x * 6) as u8, (y * 12) as u8, 90]
    });
    std::fs::write(dir.join("scan.png"), png).unwrap();
    let kml = dir.join("overlay.kml");
    std::fs::write(
        &kml,
        r#"<?xml version="1.0"?><kml xmlns="http://www.opengis.net/kml/2.2"><GroundOverlay><name>Scan</name>
<Icon><href>scan.png</href></Icon><LatLonBox><north>47</north><south>46</south><east>9</east><west>7</west></LatLonBox></GroundOverlay></kml>"#,
    )
    .unwrap();
    let s = Registry::default().open_path(&kml).expect("opens");
    assert_eq!(s.info().format, Format::Kml);
    assert_eq!((s.info().width, s.info().height), (40, 20));
    let g = s.geo().expect("georeferenced");
    assert_eq!(g.map_at(0.0, 0.0), (7.0, 47.0));
    assert_eq!(g.map_at(40.0, 20.0), (9.0, 46.0));
}

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_GIS_FORMATS_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/gis-formats")
        });
    if dir.is_dir() {
        Some(dir)
    } else {
        eprintln!("no GIS formats corpus at {}; skipping", dir.display());
        None
    }
}

#[test]
fn corpus_samples_open_as_maps() {
    let Some(dir) = corpus() else { return };
    let samples: &[(&str, Format, usize)] = &[
        ("fgb/UScounties.fgb", Format::FlatGeobuf, 3000),
        ("fgb/countries.fgb", Format::FlatGeobuf, 100),
        ("geoparquet/examples/example.parquet", Format::GeoParquet, 5),
        (
            "geoparquet/geoarrow/example_polygon_native.parquet",
            Format::GeoParquet,
            1,
        ),
        ("gdal/arrow/test.feather", Format::GeoArrow, 1),
        ("osm/monaco-250101.osm.pbf", Format::OsmPbf, 5000),
        ("osm/monaco-250101.osm", Format::OsmXml, 5000),
        ("gdal/gml/gnis_pop_100.gml", Format::Gml, 10),
        ("fit/garmin-edge-500-activity.fit", Format::Fit, 1),
        ("tcx/sup_activity_1.tcx", Format::Tcx, 1),
        ("gdal/filegdb/curves.gdb", Format::FileGdb, 5),
        ("gdal/mitab/all_geoms.tab", Format::MapInfoTab, 5),
        ("gdal/mitab/all_geoms.mif", Format::MapInfoMif, 5),
    ];
    for (rel, want, n) in samples {
        let path = dir.join(rel);
        if !path.exists() {
            eprintln!("{rel}: missing, skipped");
            continue;
        }
        let t0 = std::time::Instant::now();
        let s = Registry::default()
            .open_path(&path)
            .unwrap_or_else(|e| panic!("{rel}: {e}"));
        let open_ms = t0.elapsed().as_secs_f64() * 1e3;
        check(s.as_ref(), *want, *n);
        eprintln!("{rel}: opened in {open_ms:.1} ms (under load)");
    }
}

#[test]
fn files_without_a_map_are_refused_plainly() {
    let Some(dir) = corpus() else { return };
    for (rel, want) in [
        ("gdal/parquet/uint32_parquet2.parquet", "no geometry column"),
        ("fit/Settings.fit", "no GPS positions"),
        (
            "gdal/filegdb/with_cdf.gdb",
            "compressed File Geodatabase tables (CDF)",
        ),
        ("gdal/mitab/seamless.tab", "seamless MapInfo table"),
    ] {
        let path = dir.join(rel);
        if !path.exists() {
            continue;
        }
        let e = Registry::default()
            .open_path(&path)
            .err()
            .unwrap_or_else(|| panic!("{rel}: opened"));
        assert!(e.to_string().contains(want), "{rel}: {e}");
    }
}
