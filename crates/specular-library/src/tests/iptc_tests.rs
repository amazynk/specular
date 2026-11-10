use super::*;
use specular_metadata::edit::{self, Field, Record};

fn sidecar(d: &Dir, rel: &str, fields: &[(Field, &[&str])]) {
    let mut r = Record::new();
    let mut names = Vec::new();
    for (f, v) in fields {
        r.set(*f, v.iter().map(|s| (*s).to_owned()).collect());
        names.push(*f);
    }
    d.put(rel, &edit::write_xmp(None, &r, &names, None, true));
}

#[test]
fn iptc_fields_are_searched() {
    let d = Dir::new("iptc");
    d.put("i/a.jpg", "camera=X");
    d.put("i/b.jpg", "camera=Y");
    d.put("i/c.jpg", "camera=Z");
    sidecar(
        &d,
        "i/a.xmp",
        &[
            (Field::Title, &["Harbour at dawn"]),
            (Field::Headline, &["Fleet returns"]),
            (Field::Description, &["Boats coming in"]),
            (Field::Creator, &["Ann Smith"]),
            (Field::Copyright, &["© Ann"]),
            (Field::Credit, &["AP"]),
            (Field::Source, &["Agency"]),
            (Field::JobId, &["J-42"]),
            (Field::Event, &["Festa"]),
            (Field::ShownCity, &["Lisbon"]),
            (Field::CreatedCountry, &["Portugal"]),
            (Field::PersonsShown, &["Zoë"]),
            (Field::ContactEmail, &["ann@example.org"]),
            (Field::UsageTerms, &["Editorial only"]),
            (Field::Instructions, &["No crops"]),
        ],
    );
    sidecar(&d, "i/b.xmp", &[(Field::Credit, &["Reuters"])]);
    let lib = open(&d, false);
    lib.add_location(&d.0.join("i"));
    idle(&lib);
    for q in [
        "title:harbour",
        "headline:fleet",
        "caption:boats",
        "creator:ann",
        "artist:smith",
        "copyright:ann",
        "credit=AP",
        "source:agency",
        "job:j-42",
        "event:festa",
        "city:lisbon",
        "country:portugal",
        "person:zoë",
        "contact:example.org",
        "usage:editorial",
        "instructions:crops",
    ] {
        assert_eq!(names(&lib, q), ["a.jpg"], "{q}");
    }
    assert_eq!(names(&lib, "credit:reuters"), ["b.jpg"]);
    assert_eq!(names(&lib, "-credit:ap"), ["b.jpg", "c.jpg"]);
    // Written by the app: the catalogue follows at once.
    let mut r = Record::new();
    r.set(Field::Title, vec!["Night market".into()]);
    lib.set_iptc(vec![(d.0.join("i/c.jpg"), r.to_lines())]);
    idle(&lib);
    assert_eq!(names(&lib, "title:market"), ["c.jpg"]);
    // The facets: credit lines by count.
    let found = lib
        .search(&Search {
            query: "credit:".into(),
            facets: true,
            ..Search::default()
        })
        .unwrap();
    let credit: Vec<(String, u32)> = found
        .facets
        .iter()
        .find(|f| f.field == specular_query::facet::FacetField::Credit)
        .map(|f| f.values.iter().map(|v| (v.key.clone(), v.count)).collect())
        .unwrap_or_default();
    assert!(credit.contains(&("AP".into(), 1)), "{credit:?}");
    assert!(credit.contains(&("Reuters".into(), 1)), "{credit:?}");
}

#[test]
fn files_read_before_iptc_was_kept_are_read_again() {
    let d = Dir::new("iptc-lazy");
    d.put("i/a.jpg", "camera=X");
    sidecar(&d, "i/a.xmp", &[(Field::Title, &["Kept"])]);
    let lib = open(&d, false);
    lib.add_location(&d.0.join("i"));
    idle(&lib);
    drop(lib);
    // As a catalogue before the migration has them.
    let c = Connection::open(d.0.join("db/library.sqlite3")).unwrap();
    c.execute("UPDATE files SET iptc = NULL", []).unwrap();
    assert_eq!(crate::facts::pending(&c).unwrap(), 1);
    drop(c);
    let lib = open(&d, false);
    lib.rescan(None);
    idle(&lib);
    assert_eq!(names(&lib, "title:kept"), ["a.jpg"]);
    let c = Connection::open(d.0.join("db/library.sqlite3")).unwrap();
    assert_eq!(crate::facts::pending(&c).unwrap(), 0);
}

#[test]
fn migration_16_adds_the_iptc_column() {
    let d = Dir::new("migrate16");
    let mut c = Connection::open(d.0.join("c.sqlite3")).unwrap();
    schema::configure(&c).unwrap();
    for m in &schema::MIGRATIONS[..15] {
        c.execute_batch(m.sql).unwrap();
    }
    c.pragma_update(None, "user_version", 15).unwrap();
    c.execute_batch(
        "INSERT INTO folders (id, path, added) VALUES (1, '/p', 0);
         INSERT INTO dirs (id, folder_id, path) VALUES (1, 1, '/p');
         INSERT INTO files (id, dir_id, name, ext, size, mtime_ns, state, space, geo)
            VALUES (1, 1, 'a.jpg', 'jpg', 1, 0, 1, '', 0);",
    )
    .unwrap();
    schema::migrate(&mut c).unwrap();
    let i: Option<String> = c
        .query_row("SELECT iptc FROM files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(i, None);
}
