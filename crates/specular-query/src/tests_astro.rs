//! Astronomy subframe fields: header facts and star metrics.

use super::*;
use crate::date::{DAY, days_from_civil};
use crate::facet::{FacetField, Tally, is_active, toggle};

fn m(q: &str) -> Matcher {
    let q = parse(q);
    assert!(q.errors.is_empty(), "{q:?}");
    Matcher::new(&q, ascii_fold)
}

fn photo() -> Facts<'static> {
    Facts {
        name: "DSCF1234.JPG",
        ext: "JPG",
        known: true,
        width: 6240,
        height: 4160,
        camera: Some("X-T5"),
        ..Facts::default()
    }
}

fn subframe(object: &'static str, filter: &'static str, fwhm: Option<f32>) -> Facts<'static> {
    Facts {
        name: "Light_0001.fits",
        ext: "fits",
        known: true,
        width: 6248,
        height: 4176,
        exposure: Some(300.0),
        taken: Some(days_from_civil(2025, 8, 1) * DAY + 22 * 3600),
        subframe: true,
        object: Some(object),
        filter: Some(filter),
        frame_type: Some("light"),
        gain: Some(100.0),
        ccd_temp: Some(-10.0),
        stars: fwhm.map(|f| StarFacts {
            count: 850,
            hfd: Some(f * 1.1),
            fwhm: Some(f),
            seeing: Some(f * 1.45),
            ecc: Some(0.42),
            background: 1210.0,
            snr: Some(1.8),
        }),
        ..Facts::default()
    }
}

#[test]
fn subframe_fields() {
    let s = subframe("NGC 7000", "Ha", Some(3.0));
    for q in [
        "object:7000",
        r#"object="NGC 7000""#,
        "target:ngc",
        "filter=Ha",
        "imagetyp:light",
        "frame=light",
        "exptime>=300",
        "exposure:300s",
        "gain:100",
        "temp<-5",
        "ccdtemp:-10c",
        "fwhm<3.5",
        "fwhm<3.5px",
        "hfd:3..3.5",
        "seeing<5arcsec",
        "ecc<0.5",
        "starcount>500",
        "nstars>=850",
        "bg<2000",
        "snr>1.5",
        "dateobs:2025-08",
    ] {
        assert!(m(q).matches(&s), "{q}");
    }
    for q in [
        "object:m31",
        "filter=oiii",
        "imagetyp:dark",
        "fwhm>3.5",
        "ecc>0.5",
        "starcount<500",
        "temp>0",
        "gain:120",
    ] {
        assert!(!m(q).matches(&s), "{q}");
    }
    // Not measured yet: the metrics are unknown (they pass until known), the header is not.
    let pending = subframe("NGC 7000", "Ha", None);
    assert!(m("fwhm>9").matches(&pending));
    assert!(!m("filter:oiii").matches(&pending));
    // Not a subframe: no metrics, no header.
    assert!(!m("fwhm<9").matches(&photo()));
    assert!(!m("object:ngc").matches(&photo()));
    assert!(m("fwhm<9").needs_meta());
    assert_eq!(Field::from_name("starcount"), Some(Field::StarCount));
    // `stars` stays the rating.
    assert_eq!(Field::from_name("stars"), Some(Field::Rating));
}

#[test]
fn subframe_facets_group_targets_and_filters() {
    let files = [
        subframe("NGC 7000", "Ha", Some(3.0)),
        subframe("NGC 7000", "OIII", Some(3.2)),
        subframe("M 31", "Ha", Some(2.8)),
    ];
    let q = parse("filter=Ha");
    let mat = Matcher::new(&q, ascii_fold);
    let mut t = Tally::default();
    for f in &files {
        t.add(f, mat.fail_mask(f));
    }
    let facets = t.finish(&q);
    let get = |field: FacetField| {
        let mut v: Vec<(String, u32)> = facets
            .iter()
            .find(|f| f.field == field)
            .map(|f| f.values.iter().map(|v| (v.key.clone(), v.count)).collect())
            .unwrap_or_default();
        v.sort();
        v
    };
    // The filter facet ignores its own selection; the others count the Ha frames.
    assert_eq!(
        get(FacetField::Filter),
        vec![("Ha".to_owned(), 2), ("OIII".to_owned(), 1)]
    );
    assert_eq!(
        get(FacetField::Object),
        vec![("M 31".to_owned(), 1), ("NGC 7000".to_owned(), 1)]
    );
    assert_eq!(get(FacetField::FrameType), vec![("light".to_owned(), 2)]);
    assert!(is_active(&q, FacetField::Filter, "ha"));
    assert_eq!(toggle("", FacetField::Object, "M 31"), r#"object="M 31""#);
}
