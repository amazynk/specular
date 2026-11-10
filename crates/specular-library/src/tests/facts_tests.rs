use super::*;

/// A catalogue of `n` files read through (every fact in place), the first `stale` of them
/// read before the Exif and IPTC fields were kept.
fn read_catalogue(d: &Dir, n: usize, stale: usize) -> Connection {
    let db = d.0.join("c.sqlite3");
    let mut c = Connection::open(&db).unwrap();
    schema::configure(&c).unwrap();
    schema::migrate(&mut c).unwrap();
    c.execute_batch("BEGIN").unwrap();
    let root = d.0.join("photos");
    let root = root.to_str().unwrap();
    c.execute("INSERT INTO folders (path, added) VALUES (?1, 0)", [root])
        .unwrap();
    c.execute("INSERT INTO dirs (folder_id, path) VALUES (1, ?1)", [root])
        .unwrap();
    let mut st = c
        .prepare(
            "INSERT INTO files (id, dir_id, name, ext, size, mtime_ns, state, format, space, geo,
                 exif, iptc)
             VALUES (?1, 1, ?2, 'jpg', 1000, 0, 1, 'JPEG', 'sRGB', 0, ?3, ?4)",
        )
        .unwrap();
    for i in 1..=n {
        let read = i > stale;
        st.execute(params![
            i as i64,
            format!("photo{i}.jpg"),
            read.then_some(&b""[..]),
            read.then_some("")
        ])
        .unwrap();
    }
    drop(st);
    c.execute_batch("COMMIT").unwrap();
    c
}

/// Finding the next facts batch walks only the files that wait (partial indexes), not every
/// file read: the step count stays flat as the catalogue grows.
#[test]
fn finding_facts_work_walks_only_the_files_that_wait() {
    let steps = |n: usize, stale: usize| {
        let d = Dir::new(&format!("todo-{n}-{stale}"));
        let c = read_catalogue(&d, n, stale);
        let t0 = Instant::now();
        let (got, steps) = facts::todo_counted(&c, 500).unwrap();
        eprintln!(
            "{n} files, {stale} waiting: {} found, {steps} steps, {:.2?}",
            got.len(),
            t0.elapsed()
        );
        assert_eq!(got.len(), stale.min(500));
        steps
    };
    // Before the indexes were pinned: 5.6M steps at 200k files with none waiting (28 per
    // file read, six queries), 4.6M with 50 waiting. Now a few per file waiting, whatever
    // the catalogue's size.
    let (small, large) = (steps(2_000, 0), steps(200_000, 0));
    assert!(
        large <= small + 100,
        "{small} steps at 2k files, {large} at 200k"
    );
    assert!(large <= 1_000, "{large} steps with nothing waiting");
    let (small, large) = (steps(2_000, 50), steps(200_000, 50));
    assert!(
        large <= small + 100,
        "{small} steps at 2k files, {large} at 200k"
    );
    assert!(large <= 50 * 100, "{large} steps for 50 files waiting");
}

#[test]
fn subframes_are_searched_by_header_and_star_metrics() {
    let d = Dir::new("astro");
    d.put("n/a.jpg", "object=NGC 7000\nfilter=Ha\nfwhm=2.5");
    d.put("n/b.jpg", "object=NGC 7000\nfilter=OIII\nfwhm=4");
    d.put("n/c.jpg", "object=M 31\nfilter=Ha");
    d.put("n/plain.jpg", "camera=X-T5");
    let lib = open(&d, false);
    lib.add_location(&d.0.join("n"));
    idle(&lib);
    assert_eq!(names(&lib, "filter=Ha"), ["a.jpg", "c.jpg"]);
    assert_eq!(names(&lib, "object:7000"), ["a.jpg", "b.jpg"]);
    assert_eq!(names(&lib, "fwhm<3"), ["a.jpg"]);
    assert_eq!(names(&lib, "starcount>100 hfd>4"), ["b.jpg"]);
    let a = row(&lib, "a.jpg");
    let astro = a.meta.as_ref().unwrap().astro.as_ref().unwrap();
    assert_eq!(astro.metrics.unwrap().fwhm, Some(2.5));
    // c could not be measured: no stars (not metrics still to come, which would pass).
    assert_eq!(
        row(&lib, "c.jpg")
            .meta
            .unwrap()
            .astro
            .unwrap()
            .metrics
            .unwrap()
            .stars,
        0
    );
    assert!(row(&lib, "plain.jpg").meta.unwrap().astro.is_none());
    // Grouped by target in the facets.
    let found = lib
        .search(&Search {
            query: "filter=Ha".into(),
            facets: true,
            ..Search::default()
        })
        .unwrap();
    let objects: Vec<(String, u32)> = found
        .facets
        .iter()
        .find(|f| f.field == specular_query::facet::FacetField::Object)
        .map(|f| f.values.iter().map(|v| (v.key.clone(), v.count)).collect())
        .unwrap_or_default();
    assert!(objects.contains(&("M 31".to_owned(), 1)), "{objects:?}");
}

#[test]
fn sidecars_and_embedded_keywords_become_marks_and_tags() {
    let d = Dir::new("sidecar");
    d.put("s/a.jpg", "kw=Sunset,Family");
    d.put("s/a.xmp", &SIDECAR.replace("RATING", "3"));
    // Embedded keywords without a sidecar.
    d.put("s/b.jpg", "kw=Sunset,Family");
    // RAW+JPEG: the stem sidecar is the RAW's, the JPEG has none.
    d.put("s/pair.cr2", "");
    d.put("s/pair.jpg", "");
    d.put("s/pair.xmp", &SIDECAR.replace("RATING", "5"));
    let lib = open(&d, false);
    lib.add_location(&d.0.join("s"));
    idle(&lib);
    let a = row(&lib, "a.jpg");
    assert_eq!((a.mark.rating, a.mark.label), (3, 3));
    // A sidecar that names keywords has the last word on them (as Specular writes it).
    let mut tags: Vec<&str> = a.tags.lines().collect();
    tags.sort();
    assert_eq!(tags, ["Beach", "Holiday"]);
    assert_eq!(names(&lib, "tag:holiday"), ["a.jpg", "pair.cr2"]);
    assert_eq!(names(&lib, "sunset"), ["b.jpg"]);
    assert_eq!(names(&lib, "rating:5"), ["pair.cr2"]);
    // A newer sidecar is read again.
    std::thread::sleep(Duration::from_millis(20));
    d.put(
        "s/a.xmp",
        &SIDECAR.replace("RATING", "1").replace("Holiday", "Work"),
    );
    lib.rescan(None);
    idle(&lib);
    let a = row(&lib, "a.jpg");
    assert_eq!(a.mark.rating, 1);
    assert_eq!(names(&lib, "tag:work"), ["a.jpg"]);
    assert_eq!(names(&lib, "tag:holiday"), ["pair.cr2"]);
}

#[test]
fn a_changed_file_is_read_again() {
    let d = Dir::new("changed");
    let p = d.put("c/a.jpg", "camera=Old");
    let lib = open(&d, false);
    lib.add_location(&d.0.join("c"));
    idle(&lib);
    assert_eq!(names(&lib, "camera:old"), ["a.jpg"]);
    std::fs::write(&p, "camera=Brand New").unwrap();
    lib.rescan(None);
    idle(&lib);
    assert_eq!(names(&lib, "camera:new"), ["a.jpg"]);
    assert!(names(&lib, "camera:old").is_empty());
}

#[test]
fn places_are_found_by_town_region_and_country() {
    let d = Dir::new("places");
    d.put("g/lisbon.jpg", "pos=38.7223,-9.1393");
    d.put("g/porto.jpg", "pos=41.1496,-8.611");
    d.put("g/tokyo.jpg", "pos=35.6895,139.6917");
    d.put("g/sea.jpg", "pos=0.0,-30.0");
    d.put("g/none.jpg", "camera=X");
    let lib = open(&d, false);
    lib.add_location(&d.0.join("g"));
    idle(&lib);
    assert_eq!(names(&lib, "Lisbon"), ["lisbon.jpg"]);
    assert_eq!(names(&lib, "portugal"), ["lisbon.jpg", "porto.jpg"]);
    assert_eq!(names(&lib, "place:portugal"), ["lisbon.jpg", "porto.jpg"]);
    assert_eq!(names(&lib, "country=Japan"), ["tokyo.jpg"]);
    assert_eq!(names(&lib, "place:JP"), ["tokyo.jpg"]);
    // The region (Tokyo is its own prefecture; Porto's district is Porto).
    assert_eq!(names(&lib, "city:porto"), ["porto.jpg"]);
    assert_eq!(
        names(&lib, "-place:portugal"),
        ["none.jpg", "sea.jpg", "tokyo.jpg"]
    );
    assert!(names(&lib, "place:atlantis").is_empty());
    // Out at sea: a position, no place.
    let sea = row(&lib, "sea.jpg");
    assert_eq!(sea.place.as_deref(), Some(""));
    assert_eq!(sea.meta.unwrap().position, Some((0.0, -30.0)));
    let l = row(&lib, "lisbon.jpg");
    let (lat, lon) = l.meta.as_ref().unwrap().position.unwrap();
    assert!((lat - 38.7223).abs() < 1e-4 && (lon + 9.1393).abs() < 1e-4);
    assert!(
        l.place.as_deref().unwrap().starts_with("Lisbon\n"),
        "{:?}",
        l.place
    );
    assert_eq!(row(&lib, "none.jpg").place.as_deref(), Some(""));
    // Countries in the viewer's language as well as in English.
    let it = |q: &str| -> Vec<String> {
        let mut v: Vec<String> = lib
            .search(&Search {
                query: q.into(),
                language: Some("it".parse().unwrap()),
                ..Search::default()
            })
            .unwrap()
            .rows
            .into_iter()
            .map(|r| r.name)
            .collect();
        v.sort();
        v
    };
    assert_eq!(it("Giappone"), ["tokyo.jpg"]);
    assert_eq!(it("place:portogallo"), ["lisbon.jpg", "porto.jpg"]);
    assert_eq!(it("Japan"), ["tokyo.jpg"]);
    assert!(names(&lib, "Giappone").is_empty());
    // The place facet counts towns.
    let f = lib
        .search(&Search {
            facets: true,
            ..Search::default()
        })
        .unwrap()
        .facets;
    let p = f
        .iter()
        .find(|f| f.field == specular_query::facet::FacetField::Place)
        .expect("a place facet");
    let mut v: Vec<&str> = p.values.iter().map(|v| v.key.as_str()).collect();
    v.sort_unstable();
    assert_eq!(v.len(), 3, "{v:?}");
    assert!(v.contains(&"Lisbon") && v.contains(&"Porto"), "{v:?}");
    // Read before positions were kept: read again, by the facts pass.
    drop(lib);
    let c = Connection::open(d.0.join("db/library.sqlite3")).unwrap();
    c.execute(
        "UPDATE files SET geo = NULL, place_id = NULL, lat = NULL, lon = NULL",
        [],
    )
    .unwrap();
    drop(c);
    let lib = open(&d, false);
    lib.rescan(None);
    idle(&lib);
    assert_eq!(names(&lib, "place:portugal"), ["lisbon.jpg", "porto.jpg"]);
}

#[test]
fn migration_5_marks_files_without_a_position() {
    let d = Dir::new("migrate5");
    let mut c = Connection::open(d.0.join("c.sqlite3")).unwrap();
    schema::configure(&c).unwrap();
    // A catalogue at schema 4 with three read files: one with a position, one without, one
    // read before positions were looked for.
    for m in &schema::MIGRATIONS[..4] {
        c.execute_batch(m.sql).unwrap();
    }
    c.pragma_update(None, "user_version", 4).unwrap();
    c.execute_batch(
        "INSERT INTO folders (id, path, added) VALUES (1, '/p', 0);
         INSERT INTO dirs (id, folder_id, path) VALUES (1, 1, '/p');
         INSERT INTO files (id, dir_id, name, ext, size, mtime_ns, state, props)
            VALUES (1, 1, 'a.jpg', 'jpg', 1, 0, 1, 96), (2, 1, 'b.jpg', 'jpg', 1, 0, 1, 32),
                   (3, 1, 'c.jpg', 'jpg', 1, 0, 1, 0), (4, 1, 'd.jpg', 'jpg', 1, 0, 0, 0);",
    )
    .unwrap();
    assert_eq!(schema::migrate(&mut c).unwrap(), 4);
    let geo: Vec<Option<i64>> = c
        .prepare("SELECT geo FROM files ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    // The one with a position and the one never looked at are read again.
    assert_eq!(geo, [None, Some(0), None, None]);
    c.prepare("SELECT id, city, admin1, admin2, cc FROM places")
        .unwrap();
}

#[test]
fn files_read_before_spaces_were_kept_are_read_again() {
    let d = Dir::new("spaces");
    d.put("s/a.jpg", "camera=X\nspace=p3");
    d.put("s/b.jpg", "camera=Y");
    let lib = open(&d, false);
    lib.add_location(&d.0.join("s"));
    idle(&lib);
    assert_eq!(names(&lib, "space:p3"), ["a.jpg"]);
    // `b` has none: never read again, and no key finds it.
    assert_eq!(row(&lib, "b.jpg").meta.unwrap().space.as_deref(), Some(""));
    assert!(names(&lib, "space:srgb").is_empty());
    let mut lists = lib
        .search(&Search {
            query: "".into(),
            facets: true,
            ..Search::default()
        })
        .unwrap()
        .facets;
    lists.retain(|f| f.field == specular_query::facet::FacetField::Space);
    assert_eq!(lists.len(), 1);
    let v: Vec<(&str, u32)> = lists[0]
        .values
        .iter()
        .map(|v| (v.key.as_str(), v.count))
        .collect();
    assert_eq!(v, [("p3", 1)]);
    drop(lib);
    // As a catalogue from before migration 3 has them: read, with the space not looked at.
    let c = Connection::open(d.0.join("db/library.sqlite3")).unwrap();
    c.execute("UPDATE files SET space = NULL", []).unwrap();
    drop(c);
    // Unchanged files: only the missing space has them read again (unread would pass any
    // key, as unknown).
    let lib = open(&d, false);
    lib.rescan(None);
    idle(&lib);
    assert_eq!(names(&lib, "space:p3"), ["a.jpg"]);
    assert!(names(&lib, "space:srgb").is_empty());
    assert_eq!(row(&lib, "b.jpg").meta.unwrap().space.as_deref(), Some(""));
    assert_eq!(
        row(&lib, "a.jpg").meta.unwrap().camera.as_deref(),
        Some("X")
    );
}

#[test]
fn hierarchical_keywords_and_keywords_written_by_the_app() {
    let d = Dir::new("hier");
    d.put("h/a.jpg", "kw=Old");
    d.put(
        "h/a.xmp",
        r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:lr="http://ns.adobe.com/lightroom/1.0/"><dc:subject><rdf:Bag><rdf:li>Paris</rdf:li><rdf:li>France</rdf:li></rdf:Bag></dc:subject><lr:hierarchicalSubject><rdf:Bag><rdf:li>Places|France|Paris</rdf:li></rdf:Bag></lr:hierarchicalSubject></rdf:Description></rdf:RDF></x:xmpmeta>"#,
    );
    d.put("h/b.jpg", "");
    let lib = open(&d, false);
    lib.add_location(&d.0.join("h"));
    idle(&lib);
    assert_eq!(row(&lib, "a.jpg").tags, "Places|France|Paris");
    assert_eq!(names(&lib, "tag:paris"), ["a.jpg"]);
    assert!(names(&lib, "tag:old").is_empty());
    // The levels are tags too, linked by parent.
    let tags = lib.tag_names().unwrap();
    assert_eq!(tags, ["Places", "Places|France", "Places|France|Paris"]);
    {
        let c = lib.reader.lock();
        let parent: String = c
            .query_row(
                "SELECT p.name FROM tags t JOIN tags p ON p.id = t.parent WHERE t.name = ?1",
                ["Places|France|Paris"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(parent, "Places|France");
    }
    // Keywords the app wrote are mirrored at once.
    lib.set_keywords(vec![(
        d.0.join("h/b.jpg"),
        vec!["Sea".into(), "Places|Spain".into()],
    )]);
    idle(&lib);
    assert_eq!(names(&lib, "tag:sea"), ["b.jpg"]);
    assert_eq!(names(&lib, "tag:places"), ["a.jpg", "b.jpg"]);
    // The files a keyword rename goes to: the keyword and those below it, any case.
    let with = |k: &str| {
        lib.files_with_tag(k)
            .unwrap()
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(with("places|france"), ["a.jpg"]);
    assert_eq!(with("Places"), ["a.jpg", "b.jpg"]);
    assert_eq!(with("Sea"), ["b.jpg"]);
    assert!(with("Place").is_empty());
}
