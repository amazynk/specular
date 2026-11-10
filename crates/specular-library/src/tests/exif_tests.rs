use super::*;

#[test]
fn exif_fields_and_tags_are_searched() {
    let d = Dir::new("exif");
    d.put(
        "e/a.jpg",
        "camera=Canon EOS R5\nmake=Canon\nflash=89\ntag=a406:3",
    );
    d.put(
        "e/b.jpg",
        "camera=Nikon Z8\nmake=Nikon\nflash=16\ntag=a406:1\ntag=a408:2",
    );
    d.put("e/c.jpg", "camera=X");
    let lib = open(&d, false);
    lib.add_location(&d.0.join("e"));
    idle(&lib);
    assert_eq!(names(&lib, "make:canon"), ["a.jpg"]);
    assert_eq!(names(&lib, "flash:yes"), ["a.jpg"]);
    assert_eq!(names(&lib, "flash:no"), ["b.jpg"]);
    assert_eq!(names(&lib, "exif:SceneCaptureType=night"), ["a.jpg"]);
    assert_eq!(names(&lib, "exif:SceneCaptureType=1,3"), ["a.jpg", "b.jpg"]);
    assert_eq!(names(&lib, "exif:Contrast"), ["b.jpg"]);
    assert_eq!(names(&lib, "exif:Contrast=hard"), ["b.jpg"]);
    assert_eq!(names(&lib, "-exif:SceneCaptureType"), ["c.jpg"]);
    assert_eq!(names(&lib, "exif:Flash=redeye"), ["a.jpg"]);
    // Without Exif: known, empty.
    let c = row(&lib, "c.jpg");
    let e = c.meta.unwrap().exif.unwrap();
    assert_eq!(e.make, None);
    // The tags are loaded for `exif:` searches only.
    assert_eq!(row(&lib, "a.jpg").meta.unwrap().exif.unwrap().tags, None);
    // The facets: the tag names and enumerated words, for the picker.
    let found = lib
        .search(&Search {
            query: "exif:".into(),
            facets: true,
            ..Search::default()
        })
        .unwrap();
    let ex: Vec<(String, u32)> = found
        .facets
        .iter()
        .find(|f| f.field == specular_query::facet::FacetField::Exif)
        .map(|f| f.values.iter().map(|v| (v.key.clone(), v.count)).collect())
        .unwrap_or_default();
    assert!(ex.contains(&("SceneCaptureType".into(), 2)), "{ex:?}");
    assert!(ex.contains(&("SceneCaptureType=night".into(), 1)), "{ex:?}");
    assert!(ex.contains(&("Contrast=hard".into(), 1)), "{ex:?}");
    let make: Vec<(String, u32)> = found
        .facets
        .iter()
        .find(|f| f.field == specular_query::facet::FacetField::Make)
        .map(|f| f.values.iter().map(|v| (v.key.clone(), v.count)).collect())
        .unwrap_or_default();
    assert!(make.contains(&("Canon".into(), 1)) && make.contains(&("Nikon".into(), 1)));
    // A smart album's rules load the tags too.
    let found = lib
        .search(&Search {
            query: String::new(),
            rules: Some(specular_query::rules::Rules::one("exif:Contrast")),
            ..Search::default()
        })
        .unwrap();
    assert_eq!(found.total, 1);
}

#[test]
fn files_read_before_exif_was_kept_are_read_again() {
    let d = Dir::new("exif-lazy");
    d.put("e/a.jpg", "camera=X\nmake=Fuji\ntag=a406:3");
    d.put("e/b.jpg", "camera=Y");
    let lib = open(&d, false);
    lib.add_location(&d.0.join("e"));
    idle(&lib);
    assert_eq!(names(&lib, "make:fuji"), ["a.jpg"]);
    drop(lib);
    // As a catalogue before migration 15 has them: read, the Exif not looked at.
    let c = Connection::open(d.0.join("db/library.sqlite3")).unwrap();
    c.execute("UPDATE files SET exif = NULL, exif_tags = NULL", [])
        .unwrap();
    assert_eq!(
        crate::facts::pending(&c).unwrap(),
        2,
        "both wait for their Exif"
    );
    drop(c);
    let lib = open(&d, false);
    // Before they are read again they are unknown: kept by any Exif term, with their other
    // facts in place (no blocking rescan).
    // (The background pass may already be under way: either answer is right here.)
    let early = names(&lib, "make:nikon");
    assert!(early.is_empty() || early == ["a.jpg", "b.jpg"], "{early:?}");
    lib.rescan(None);
    idle(&lib);
    assert_eq!(names(&lib, "make:fuji"), ["a.jpg"]);
    assert!(names(&lib, "make:nikon").is_empty());
    assert_eq!(names(&lib, "exif:SceneCaptureType=night"), ["a.jpg"]);
    assert_eq!(
        row(&lib, "b.jpg").meta.unwrap().camera.as_deref(),
        Some("Y")
    );
    let c = Connection::open(d.0.join("db/library.sqlite3")).unwrap();
    assert_eq!(crate::facts::pending(&c).unwrap(), 0);
}

#[test]
fn migration_15_adds_the_exif_columns() {
    let d = Dir::new("migrate15");
    let mut c = Connection::open(d.0.join("c.sqlite3")).unwrap();
    schema::configure(&c).unwrap();
    for m in &schema::MIGRATIONS[..14] {
        c.execute_batch(m.sql).unwrap();
    }
    c.pragma_update(None, "user_version", 14).unwrap();
    c.execute_batch(
        "INSERT INTO folders (id, path, added) VALUES (1, '/p', 0);
         INSERT INTO dirs (id, folder_id, path) VALUES (1, 1, '/p');
         INSERT INTO files (id, dir_id, name, ext, size, mtime_ns, state, space, geo)
            VALUES (1, 1, 'a.jpg', 'jpg', 1, 0, 1, '', 0);",
    )
    .unwrap();
    assert_eq!(schema::migrate(&mut c).unwrap(), 14);
    assert_eq!(
        schema::MIGRATIONS[14].what,
        "Exif fields and tags for search"
    );
    let (e, t): (Option<Vec<u8>>, Option<String>) = c
        .query_row("SELECT exif, exif_tags FROM files", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!((e, t), (None, None));
}
