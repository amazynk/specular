//! The IPTC fields: `title:`, `credit:` and the others, and IPTC's creator, copyright,
//! caption, locations and persons before Exif's and the catalogue's.

use super::*;
use crate::facet::{FacetField, Tally};

fn lib(q: &str) -> Matcher {
    let q = parse_in(q, Scope::Library);
    assert!(q.errors.is_empty(), "{:?}", q.errors);
    Matcher::new(&q, ascii_fold)
}

const LINES: &str = "title=Harbour at dawn\nheadline=Fleet returns\ndescription=Boats coming in\n\
creator=Ann Smith\ncreator=Bo Ek\ncopyright=(c) Ann\ncredit=AP\nsource=Agency\n\
usage-terms=Editorial only\ninstructions=No crops\njob-id=J-42\nevent=Festa\n\
shown-city=Lisbon\ncreated-country=Portugal\npersons-shown=Zoe\ncontact-email=ann@example.org";

fn facts<'a>(iptc: Option<&'a str>, exif: Option<ExifFacts<'a>>) -> Facts<'a> {
    Facts {
        name: "a.jpg",
        ext: "jpg",
        known: true,
        iptc,
        exif,
        place: Some("Porto\nPortugal"),
        people: Some(""),
        ..Facts::default()
    }
}

#[test]
fn iptc_fields_match() {
    let f = facts(Some(LINES), None);
    for q in [
        "title:harbour",
        "headline:fleet",
        "caption:boats",
        "creator:bo",
        "artist:ann",
        "copyright:ann",
        "credit=ap",
        "source:agency",
        "usage:editorial",
        "instructions:crops",
        "job:j-42",
        "event:festa",
        "contact:example",
        "city:lisbon",
        "city:porto",
        "country:portugal",
        "person:zoe",
    ] {
        assert_eq!(lib(q).tri(&f), Tri::Yes, "{q}");
    }
    for q in [
        "title:night",
        "credit=a",
        "-title:harbour",
        "city:madrid",
        "person:ann",
    ] {
        assert_eq!(lib(q).tri(&f), Tri::No, "{q}");
    }
    // Not read yet: unknown, so kept.
    let unread = facts(None, None);
    assert_eq!(lib("title:harbour").tri(&unread), Tri::Unknown);
    assert_eq!(lib("city:porto").tri(&unread), Tri::Yes);
    // Library only.
    assert!(!parse_in("title:x", Scope::Gallery).errors.is_empty());
}

/// Where IPTC has the creator, copyright or caption it wins over Exif; else Exif counts.
#[test]
fn iptc_before_exif() {
    let e = ExifFacts {
        artist: Some("Exif Artist"),
        copyright: Some("Exif rights"),
        description: Some("Exif caption"),
        ..ExifFacts::default()
    };
    let f = facts(Some(LINES), Some(e));
    assert_eq!(lib("artist:exif").tri(&f), Tri::No);
    assert_eq!(lib("copyright:exif").tri(&f), Tri::No);
    let none = facts(Some("title=T"), Some(e));
    assert_eq!(lib("artist:exif").tri(&none), Tri::Yes);
    assert_eq!(lib("caption:exif").tri(&none), Tri::Yes);
}

#[test]
fn iptc_facets_count() {
    let mut t = Tally::default();
    let e = ExifFacts {
        artist: Some("Exif Artist"),
        ..ExifFacts::default()
    };
    t.add(&facts(Some(LINES), Some(e)), 0);
    t.add(&facts(Some("credit=AP"), None), 0);
    let facets = t.finish(&parse_in("", Scope::Library));
    let get = |ff: FacetField| -> Vec<(String, u32)> {
        facets
            .iter()
            .find(|f| f.field == ff)
            .map(|f| f.values.iter().map(|v| (v.key.clone(), v.count)).collect())
            .unwrap_or_default()
    };
    assert_eq!(get(FacetField::Credit), [("AP".to_owned(), 2)]);
    let artists = get(FacetField::Artist);
    assert!(
        artists.contains(&("Ann Smith".to_owned(), 1)),
        "{artists:?}"
    );
    assert!(
        !artists.iter().any(|(k, _)| k == "Exif Artist"),
        "{artists:?}"
    );
    assert_eq!(get(FacetField::JobId), [("J-42".to_owned(), 1)]);
    assert_eq!(Field::from_name("creator"), Some(Field::Artist));
    assert_eq!(Field::from_name("job"), Some(Field::JobId));
}
