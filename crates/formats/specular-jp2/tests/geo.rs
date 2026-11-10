//! GeoJP2 and GMLJP2 boxes are found where GDAL writes them (`tools/corpus/geojp2-fixtures.py`).

const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");

fn read(name: &str) -> Vec<u8> {
    std::fs::read(format!("{DATA}{name}")).unwrap()
}

#[test]
fn finds_geojp2_and_gmljp2_boxes() {
    let d = read("geo/utm_geojp2.jp2");
    let b = specular_jp2::geo_boxes(&d);
    let tiff = b.geojp2.expect("GeoJP2 box");
    assert!(tiff.starts_with(b"II*\0") || tiff.starts_with(b"MM\0*"));
    assert!(b.gml.is_empty());

    let d = read("geo/utm_gml.jp2");
    let b = specular_jp2::geo_boxes(&d);
    assert!(b.geojp2.is_none());
    assert!(
        b.gml
            .iter()
            .any(|x| String::from_utf8_lossy(x).contains("RectifiedGrid"))
    );

    let d = read("geo/mercator_both.jp2");
    let b = specular_jp2::geo_boxes(&d);
    assert!(b.geojp2.is_some() && !b.gml.is_empty());

    // A plain JP2 and a raw codestream have none.
    assert_eq!(
        specular_jp2::geo_boxes(&read("gray_rev.jp2")),
        Default::default()
    );
    assert_eq!(
        specular_jp2::geo_boxes(&read("gray_97.j2k")),
        Default::default()
    );
    // Truncated anywhere: never panics.
    let d = read("geo/mercator_both.jp2");
    for n in 0..d.len() {
        let _ = specular_jp2::geo_boxes(&d[..n]);
    }
}
