use super::*;
use specular_vector::Shape;

const SITE: &[u8] = include_bytes!("../tests/data/site.xml");

fn oracle() -> serde_json::Value {
    serde_json::from_str(include_str!("../tests/data/oracle.json")).unwrap()
}

#[test]
fn reads_the_site() {
    assert!(is_landxml(SITE));
    let x = parse(SITE).unwrap();
    assert_eq!(x.version, "1.2");
    assert_eq!(x.units.linear, "meter");
    let crs = x.crs.as_ref().unwrap().describe().unwrap();
    assert_eq!(crs, "ETRS89 / UTM zone 32N, EPSG:25832, ETRS89, DHHN2016");
    let s = &x.surfaces[0];
    assert_eq!((s.points.len(), s.faces.len(), s.hidden_faces), (49, 72, 1));
    assert_eq!(s.breaklines.len(), 1);
    assert_eq!(s.boundaries[0].kind, "outer");
    // Northing first in the file: x is the easting.
    assert_eq!(s.points[0][..2], [1000.0, 2000.0]);
    let a = &x.alignments[0];
    assert_eq!(a.elements.len(), 5);
    assert_eq!(a.profiles.len(), 2);
}

/// Areas, volume and elevations against the independent oracle (tools/corpus/landxml-oracle.py).
#[test]
fn measures_match_the_oracle() {
    let x = parse(SITE).unwrap();
    let o = oracle();
    let w = &o["site.xml"]["surfaces"][0];
    let m = tin::measures(&x.surfaces[0]).unwrap();
    let close = |a: f64, k: &str| (a - w[k].as_f64().unwrap()).abs() < 1e-6;
    assert!(close(m.area_2d, "area_2d"), "{m:?}");
    assert!(close(m.area_3d, "area_3d"), "{m:?}");
    assert!(close(m.volume, "volume"), "{m:?}");
    assert!(close(m.z_min, "z_min") && close(m.z_max, "z_max"));
    let wl = o["site.xml"]["alignments"][0]["length"].as_f64().unwrap();
    assert!((x.alignments[0].computed_length() - wl).abs() < 1e-6);
}

/// Every element ends where the file says (the spirals' ends were computed by the fixture
/// generator with its own integration), and the next element starts there.
#[test]
fn clothoids_end_where_stated() {
    let x = parse(SITE).unwrap();
    let a = &x.alignments[0];
    let mut worst: f64 = 0.0;
    for e in &a.elements {
        let (got, want) = e.ends();
        let want = want.unwrap();
        worst = worst.max((got[0] - want[0]).hypot(got[1] - want[1]));
    }
    assert!(worst < 1e-5, "{worst}");
    // The stated length agrees with the computed one.
    assert!((a.computed_length() - a.length.unwrap()).abs() < 1e-5);
    // Stations: the arc starts at 1000 + 100 + 40.
    let (p, _) = a.at_station(1140.0).unwrap();
    let (q, _) = a.elements[2].at(0.0);
    assert!((p[0] - q[0]).hypot(p[1] - q[1]) < 1e-5);
    assert!(a.at_station(999.0).is_none());
}

#[test]
fn the_scene() {
    let x = parse(SITE).unwrap();
    let s = scene(&x);
    assert_eq!(s.format, "LandXML");
    assert_eq!(s.units.code, 6);
    assert_eq!(s.sheets.len(), 2, "plan and one profile chart");
    assert!(s.sheets[1].chart.is_some());
    let info = s.survey.as_ref().unwrap();
    assert_eq!(info.surfaces[0].triangles, 72);
    assert!((info.surfaces[0].contour_interval - 0.5).abs() < 1e-12);
    assert!((info.alignments[0].sta_end - 1334.719755).abs() < 1e-5);
    let texts: Vec<String> = s.sheets[0]
        .items
        .iter()
        .filter_map(|i| match &i.shape {
            Shape::Text(t) => Some(t.plain()),
            _ => None,
        })
        .collect();
    assert!(texts.contains(&"1+000".to_string()), "{texts:?}");
    assert!(texts.contains(&"1+200".to_string()), "{texts:?}");
    let names: Vec<&str> = s.layers.iter().map(|l| l.name.as_str()).collect();
    for want in [
        "Surface Existing ground",
        "Contours Existing ground",
        "Alignment Main road",
    ] {
        assert!(names.contains(&want), "{names:?}");
    }
    assert!(s.warnings.is_empty(), "{:?}", s.warnings);
}

#[test]
fn refusals_and_damage() {
    assert_eq!(parse(b"<svg/>").err(), Some(Error::NotLandXml));
    match parse(br#"<LandXML version="1.2"><Units/></LandXML>"#) {
        Err(Error::Unsupported(m)) => assert!(m.contains("no TIN surface")),
        r => panic!("{r:?}"),
    }
    let grid = br#"<LandXML><Surfaces><Surface name="g"><Definition surfType="grid"/></Surface></Surfaces><Alignments><Alignment name="a"><CoordGeom><Line><Start>0 0</Start><End>0 10</End></Line></CoordGeom></Alignment></Alignments></LandXML>"#;
    let x = parse(grid).unwrap();
    assert!(x.warnings.iter().any(|w| w.contains("only TIN")));
    let dtd = b"<!DOCTYPE x [<!ENTITY a \"b\">]><LandXML/>";
    assert!(parse(dtd).is_err());
    for n in (0..SITE.len()).step_by(211) {
        if let Ok(x) = parse(&SITE[..n]) {
            let _ = scene(&x);
        }
    }
}

#[test]
fn parcels_and_survey_points() {
    let f = br#"<LandXML version="1.2"><Units><Metric linearUnit="meter"/></Units>
<CgPoints><CgPoint name="1" code="IP">100 200 5</CgPoint><CgPoint name="2" desc="BM">110 210</CgPoint>
<CgPoints name="ref"><CgPoint name="3" pntRef="1"/><CgPoint name="4" pntRef="nosuch"/></CgPoints></CgPoints>
<Parcels><Parcel name="Lot 1" area="100"><Center>105 205</Center><CoordGeom>
<Line><Start>100 200</Start><End>100 210</End></Line><Line><Start>100 210</Start><End>110 210</End></Line>
<Curve rot="cw" radius="5"><Start>110 210</Start><Center>105 210</Center><End>100 210</End></Curve>
</CoordGeom></Parcel></Parcels></LandXML>"#;
    let x = parse(f).unwrap();
    // Northing first: x is the easting.
    assert_eq!(x.points.len(), 3);
    assert_eq!(x.points[0].at, [200.0, 100.0]);
    assert_eq!(x.points[0].z, Some(5.0));
    assert_eq!(x.points[1].code.as_deref(), Some("BM"));
    assert_eq!(
        (x.points[2].name.as_str(), x.points[2].at),
        ("3", [200.0, 100.0])
    );
    let p = &x.parcels[0];
    assert_eq!(
        (p.name.as_str(), p.area, p.center),
        ("Lot 1", Some(100.0), Some([205.0, 105.0]))
    );
    assert_eq!(p.boundary.len(), 3);
    assert!(!x.warnings.iter().any(|w| w.contains("Parcels")));
    let s = scene(&x);
    let names: Vec<&str> = s.layers.iter().map(|l| l.name.as_str()).collect();
    for n in [
        "Parcels",
        "Parcel labels",
        "Survey points",
        "Survey point names",
    ] {
        assert!(names.contains(&n), "{names:?}");
    }
    let texts: Vec<String> = s.sheets[0]
        .items
        .iter()
        .filter_map(|i| match &i.shape {
            Shape::Text(t) => Some(t.plain()),
            _ => None,
        })
        .collect();
    assert!(texts.contains(&"Lot 1 (100.0)".to_string()), "{texts:?}");
    assert!(texts.contains(&"3".to_string()));
    let counts: std::collections::HashMap<&str, u64> =
        s.counts.iter().map(|(k, n)| (k.as_str(), *n)).collect();
    assert_eq!((counts["Parcel"], counts["Survey point"]), (1, 3));
    // The parcel's boundary: two lines and the half circle, one path.
    let l = s.layers.iter().position(|l| l.name == "Parcels").unwrap() as u32;
    let paths: Vec<_> = s.sheets[0].items.iter().filter(|i| i.layer == l).collect();
    assert_eq!(paths.len(), 1);
    let Shape::Stroke { path, .. } = &paths[0].shape else {
        panic!()
    };
    use specular_vector::kurbo::Shape as _;
    let len = path.perimeter(1e-6);
    let want = 20.0 + std::f64::consts::PI * 5.0;
    assert!((len - want).abs() < 0.01, "{len} {want}");
}
