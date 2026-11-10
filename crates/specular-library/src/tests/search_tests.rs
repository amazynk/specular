use super::*;

#[test]
fn pushdown_never_changes_results_or_facets() {
    let d = Dir::new("pushdown");
    let cams = ["Canon EOS R5", "Nikon Z8", "Pixel 8"];
    for i in 0..60 {
        let body = format!(
            "camera={}\niso={}\nw={}\nh={}\ntaken={}{}{}{}",
            cams[i % 3],
            100 << (i % 5),
            20 + i,
            60 - i,
            1_600_000_000 + i as i64 * 86_400 * 40,
            if i % 7 == 0 { "\nhdr" } else { "" },
            if i % 4 == 0 { "\nkw=Trip,Mountain" } else { "" },
            ["\nspace=p3", "\nspace=srgb", "", "\nspace=ColorMatch RGB"][i % 4],
        ) + [
            "\npos=38.7223,-9.1393",
            "\npos=41.1496,-8.611",
            "",
            "\npos=35.6895,139.6917",
            "\npos=0.0,-30.0",
        ][i % 5]
            // Movies and sound files: by their content (an MP4 of sound only, an Ogg
            // movie), else unread by their extension.
            + ["", "", "", "\nav=movie", "", "\nav=sound"][i % 6];
        let ext = ["jpg", "png", "cr2", "mp4", "ogg", "flac", "jpg"][i % 7];
        d.put(&format!("p/{}/img_{i:03}.{ext}", i % 4), &body);
    }
    let lib = open(&d, false);
    lib.add_location(&d.0.join("p"));
    idle(&lib);
    let marks: Vec<(PathBuf, Mark)> = lib
        .search(&Search::default())
        .unwrap()
        .rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            (
                r.path.clone(),
                Mark {
                    rating: (i % 6) as u8,
                    label: (i % 5) as u8,
                    flag: (i % 3) as i8 - 1,
                },
            )
        })
        .collect();
    lib.set_marks(marks);
    idle(&lib);
    // Leave some unread, so unknown facts are part of the check.
    let c = Connection::open(d.0.join("db/library.sqlite3")).unwrap();
    c.execute("UPDATE files SET state = 0 WHERE id % 5 = 0", [])
        .unwrap();
    // And some whose position was not looked for (read before positions were kept).
    c.execute("UPDATE files SET geo = NULL WHERE id % 7 = 0", [])
        .unwrap();
    drop(c);
    let queries = [
        "",
        "img",
        "img_01",
        "-img_01",
        "ex",
        "canon",
        "camera:nikon,pixel",
        "camera=\"Pixel 8\"",
        "lens:foo",
        "tag:trip",
        "-tag:trip",
        "mountain",
        "in:p/1",
        "folder:3",
        "ext:jpg,png",
        "-ext:cr2",
        "rating>=3",
        "-rating:2..4",
        "label:red,blue",
        "-label:green",
        "size>10",
        "iso>=400",
        "iso:100..200",
        "w>40 h<50",
        "mp>0.001",
        "is:landscape",
        "orientation:portrait",
        "is:hdr",
        "is:pick,reject",
        "-is:pick",
        "is:video",
        "is:sound",
        "-is:sound",
        "is:video,sound",
        "has:camera",
        "has:gps",
        "date:2021",
        "date>2022-06-01",
        "modified<2000-01-01",
        "-modified<2000-01-01",
        "pages:1",
        "bits:8",
        "space:p3",
        "-space:srgb",
        "profile:colormatch",
        "space=\"display p3\" camera:canon",
        "dupes:",
        "canon is:landscape rating>1 -ext:png",
        "lisbon",
        "portugal",
        "-lisbon",
        "place:portugal",
        "place:tokyo,porto",
        "place=Portugal",
        "-place:japan",
        "country:jp",
        "place:atlantis",
        "has:gps -place:portugal",
    ];
    for facets in [false, true] {
        for sort in [Sort::Name, Sort::Date, Sort::Rating, Sort::Folder] {
            for q in queries {
                let req = Search {
                    query: q.into(),
                    sort,
                    facets,
                    ..Search::default()
                };
                let a = lib.search(&req).unwrap();
                let b = lib
                    .search(&Search {
                        no_pushdown: true,
                        ..req.clone()
                    })
                    .unwrap();
                let ids = |f: &Found| f.rows.iter().map(|r| r.id).collect::<Vec<_>>();
                assert_eq!(ids(&a), ids(&b), "{q:?} sort {sort:?}");
                assert_eq!(a.facets, b.facets, "{q:?} facets");
                assert!(a.candidates <= b.candidates);
            }
        }
    }
    // Pushdown does narrow.
    let r = lib
        .search(&Search {
            query: "ext:png rating:5".into(),
            ..Search::default()
        })
        .unwrap();
    assert!(r.candidates < 60 / 3, "{}", r.candidates);
}

#[test]
fn sorting_limits_and_facets() {
    let d = Dir::new("sort");
    d.put("s/b.jpg", "taken=300\ncamera=Canon");
    d.put("s/a.jpg", "taken=100\ncamera=Canon");
    d.put("s/c.png", "taken=200\ncamera=Nikon");
    let lib = open(&d, false);
    lib.add_location(&d.0.join("s"));
    idle(&lib);
    let order = |sort, descending| {
        lib.search(&Search {
            sort,
            descending,
            ..Search::default()
        })
        .unwrap()
        .rows
        .into_iter()
        .map(|r| r.name)
        .collect::<Vec<_>>()
    };
    assert_eq!(order(Sort::Name, false), ["a.jpg", "b.jpg", "c.png"]);
    assert_eq!(order(Sort::Date, true), ["b.jpg", "c.png", "a.jpg"]);
    // Unordered: the same rows, in no promised order (a caller that sorts them itself).
    let mut any = order(Sort::Unordered, true);
    any.sort();
    assert_eq!(any, ["a.jpg", "b.jpg", "c.png"]);
    let f = lib
        .search(&Search {
            query: "camera:canon".into(),
            limit: Some(1),
            facets: true,
            ..Search::default()
        })
        .unwrap();
    assert_eq!((f.rows.len(), f.total), (1, 2));
    let fmt = f
        .facets
        .iter()
        .find(|x| x.field == specular_query::facet::FacetField::Format)
        .unwrap();
    assert_eq!(fmt.values[0].key, "jpg");
    assert_eq!(fmt.values[0].count, 2);
    // `in:` is the Library's: the gallery reports it unavailable (`tag:` works in both).
    assert!(!specular_query::parse("in:x").errors.is_empty());
    assert!(f.query.errors.is_empty());
}

#[test]
fn a_ranked_search_keeps_its_order_and_the_other_terms() {
    let d = Dir::new("ranked");
    d.put("r/b.jpg", "taken=300\ncamera=Canon");
    d.put("r/a.jpg", "taken=100\ncamera=Canon");
    d.put("r/c.png", "taken=200\ncamera=Nikon");
    let lib = open(&d, false);
    lib.add_location(&d.0.join("r"));
    idle(&lib);
    let id = |n: &str| row(&lib, n).id;
    let ranked = std::sync::Arc::new(vec![
        (id("c.png"), 0.9),
        (id("a.jpg"), 0.5),
        (id("b.jpg"), 0.4),
    ]);
    let names = |query: &str, limit, ranked: Vec<(i64, f32)>| {
        lib.search(&Search {
            query: query.into(),
            sort: Sort::Name,
            limit,
            ranked: Some(std::sync::Arc::new(ranked)),
            ..Search::default()
        })
        .unwrap()
        .rows
        .into_iter()
        .map(|r| r.name)
        .collect::<Vec<_>>()
    };
    // Relevance, not name order.
    assert_eq!(
        names("", None, ranked.to_vec()),
        ["c.png", "a.jpg", "b.jpg"]
    );
    assert_eq!(names("", Some(2), ranked.to_vec()), ["c.png", "a.jpg"]);
    // The other terms narrow.
    assert_eq!(
        names("camera:canon", None, ranked.to_vec()),
        ["a.jpg", "b.jpg"]
    );
    // Only what the description found.
    assert_eq!(names("", None, ranked[1..2].to_vec()), ["a.jpg"]);
    assert!(names("", None, Vec::new()).is_empty());
}

#[test]
fn the_fold_is_the_indexs() {
    fn upper(s: &str) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Owned(s.to_uppercase())
    }
    let d = Dir::new("fold");
    d.put("f/Straße.jpg", "");
    let lib = open(&d, false);
    lib.add_location(&d.0.join("f"));
    idle(&lib);
    assert_eq!(names(&lib, "straße"), ["Straße.jpg"]);
    drop(lib);
    // Another fold rebuilds the full-text index for it.
    let mut cfg = config(d.0.join("db/library.sqlite3"), false);
    cfg.fold = upper;
    cfg.fold_id = "upper";
    let lib = Library::open(cfg).unwrap();
    idle(&lib);
    assert_eq!(names(&lib, "STRASSE"), ["Straße.jpg"]);
}

#[test]
fn search_is_right_while_the_index_is_rebuilt() {
    let d = Dir::new("fts-parts");
    for i in 0..40 {
        d.put(&format!("f/photo{i:02}.jpg"), "");
    }
    d.put("f/sub/zebra.jpg", "");
    let db = d.0.join("db/library.sqlite3");
    let lib = open(&d, false);
    lib.add_location(&d.0.join("f"));
    idle(&lib);
    drop(lib);
    let c = Connection::open(&db).unwrap();
    // A row of a file that is no more (the rebuild drops it too).
    c.execute(
        "INSERT INTO fts_index (rowid, name, camera, lens, tags) VALUES (999999, 'photo', '', '', '')",
        [],
    )
    .unwrap();
    // The indexer's first step for another fold, then one part of eight files.
    c.execute("DELETE FROM info WHERE key = 'fold'", [])
        .unwrap();
    let at = store::fts_rebuild_some(&c, store::FtsRebuild::START, 8, reversed).unwrap();
    assert!(matches!(at, Some(store::FtsRebuild::Files(_))), "{at:?}");
    let find = |q: &str, no_pushdown: bool| {
        let f = search::run(
            &c,
            &Search {
                query: q.into(),
                no_pushdown,
                ..Search::default()
            },
            reversed,
            "reversed",
        )
        .unwrap();
        (f.total, f.candidates)
    };
    // Mid-rebuild: every file is checked by the matcher, and all are found.
    assert_eq!(find("photo", false), (40, 41));
    assert_eq!(find("zebra", false), (1, 41));
    assert_eq!(find("in:sub", false), (1, 41));
    // Had the half-rebuilt index been trusted, most would be missed.
    store::info_set(&c, "fold", "reversed").unwrap();
    assert!(find("photo", false).0 < 40);
    c.execute("DELETE FROM info WHERE key = 'fold'", [])
        .unwrap();
    // The rest, as the indexer goes on.
    let mut at = at;
    while let Some(a) = at {
        at = store::fts_rebuild_some(&c, a, 8, reversed).unwrap();
    }
    store::info_set(&c, "fold", "reversed").unwrap();
    assert_eq!(fts_rows(&c), (41, 2), "one row per file and folder");
    assert_eq!(find("photo", false), (40, 40));
    assert_eq!(find("zebra", false), (1, 1));
    assert_eq!(find("in:sub", false), (1, 1));
    assert_eq!(find("photo", true).0, 40);
}

#[test]
fn commands_are_answered_while_the_index_is_rebuilt() {
    const N: usize = 30_000;
    let d = Dir::new("fts-commands");
    let db = unindexed_catalogue(&d, N);
    let mut cfg = config(db.clone(), false);
    cfg.signer = None;
    let lib = Library::open(cfg).unwrap();
    lib.rescan(Some(-1));
    let t = Instant::now();
    while lib.shared.queued.load(std::sync::atomic::Ordering::Acquire) > 0 {
        assert!(t.elapsed() < Duration::from_secs(20), "command not taken");
        std::thread::sleep(Duration::from_micros(200));
    }
    let c = Connection::open(&db).unwrap();
    assert_eq!(
        store::info_get(&c, "fold").unwrap(),
        None,
        "the command was answered before the rebuild was through"
    );
    // Right meanwhile, if slower (or already with the index: right either way).
    let want = (1..=N).filter(|i| i.to_string().starts_with("123")).count();
    let f = lib
        .search(&Search {
            query: "photo123".into(),
            ..Search::default()
        })
        .unwrap();
    assert_eq!(f.total, want);
    idle(&lib);
    assert_eq!(
        store::info_get(&c, "fold").unwrap().as_deref(),
        Some("ascii")
    );
    assert_eq!(fts_rows(&c), (N as i64, 1));
    let f = lib
        .search(&Search {
            query: "photo123".into(),
            ..Search::default()
        })
        .unwrap();
    assert_eq!(f.total, want);
    assert_eq!(f.candidates, want, "the index is used again");
}

#[test]
fn starts_cover_each_change_once() {
    let d = Dir::new("starts");
    d.put("r/a/x.jpg", "");
    d.put("r/b/y.jpg", "");
    let r = d.0.join("r");
    let s = indexer::starts(
        &r,
        vec![
            r.join("a/x.jpg"),
            r.join("a"),
            r.join("a/gone/deeper.jpg"),
            r.join("b/y.jpg"),
            r.join(".git/index"),
            r.join("a/node_modules/pkg/n.jpg"),
            PathBuf::from("/elsewhere/z.jpg"),
        ],
        &Skip::default(),
    );
    assert_eq!(s, [(r.join("a"), true), (r.join("b"), false)]);
}

/// A change in or under a folder that is now a link (to `/`) is walked from the folder
/// above the link, never through it.
#[cfg(unix)]
#[test]
fn starts_never_go_through_a_link() {
    let d = Dir::new("starts-link");
    d.put("r/a/x.jpg", "");
    let r = d.0.join("r");
    std::os::unix::fs::symlink("/", r.join("c")).unwrap();
    std::os::unix::fs::symlink("/", r.join("a/up")).unwrap();
    for p in ["c", "c/4.jpg", "c/usr", "c/usr/lib/x.jpg"] {
        let s = indexer::starts(&r, vec![r.join(p)], &Skip::default());
        assert_eq!(s, [(r.clone(), false)], "{p}");
    }
    let s = indexer::starts(&r, vec![r.join("a/up/usr")], &Skip::default());
    assert_eq!(s, [(r.join("a"), false)]);
}

#[test]
fn embedded_keywords_are_found_in_the_head() {
    let mut head = b"\xff\xd8\xff\xe1junk http://ns.adobe.com/xap/1.0/\0".to_vec();
    head.extend_from_slice(SIDECAR.as_bytes());
    head.extend_from_slice(b"\xff\xdbmore");
    assert_eq!(embedded_keywords(&head), ["Holiday", "Beach"]);
    assert!(embedded_keywords(b"no packet").is_empty());
}

/// Rows handed out one by one are those a collected search finds, in its order, within its
/// limit, and in rank order for a search by description.
#[test]
fn rows_one_by_one_match_a_collected_search() {
    let d = Dir::new("search-each");
    let db = unindexed_catalogue(&d, 50);
    let mut cfg = config(db, false);
    cfg.signer = None;
    let lib = Library::open(cfg).unwrap();
    idle(&lib);
    let each = |req: &Search| {
        let mut v = Vec::new();
        let found = lib.search_each(req, &mut |r| v.push(r.id)).unwrap();
        assert!(found.rows.is_empty(), "nothing collected");
        (v, found.total)
    };
    let reqs = [
        Search {
            query: "photo1".into(),
            sort: Sort::Name,
            descending: true,
            ..Search::default()
        },
        Search {
            limit: Some(7),
            ..Search::default()
        },
        Search {
            ranked: Some(Arc::new(vec![(30, 0.9), (4, 0.8), (17, 0.5)])),
            limit: Some(2),
            ..Search::default()
        },
    ];
    for req in &reqs {
        let all = lib.search(req).unwrap();
        let want: Vec<i64> = all.rows.iter().map(|r| r.id).collect();
        assert_eq!(each(req), (want, all.total), "{req:?}");
    }
    assert_eq!(
        each(&reqs[2]),
        (vec![30, 4], 3),
        "best first, cut to the limit"
    );
    let (got, total) = each(&reqs[1]);
    assert_eq!((got.len(), total), (7, 50));
    assert_eq!(each(&reqs[0]).0.len(), 11, "photo1 and photo10–19");
}

/// A new, changed or removed `.lrc` beside a song has its words read again, though the
/// song itself is unchanged.
#[test]
fn lyrics_sidecar_changes_reindex() {
    let d = Dir::new("lyrics-sidecar");
    d.put("m/song.jpg", "av=sound");
    let lib = open(&d, false);
    lib.add_location(&d.0.join("m"));
    idle(&lib);
    assert!(names(&lib, "moonlight").is_empty());
    let lrc = d.0.join("m/song.lrc");
    let mut stamp = std::time::SystemTime::now();
    let mut write = |text: &str| {
        std::fs::write(&lrc, text).unwrap();
        // a distinct modification time even on coarse clocks
        stamp += Duration::from_secs(2);
        std::fs::File::options()
            .write(true)
            .open(&lrc)
            .unwrap()
            .set_modified(stamp)
            .unwrap();
        lib.rescan(None);
        idle(&lib);
    };
    write("Fly me to the Moonlight");
    assert_eq!(names(&lib, "moonlight"), ["song.jpg"]);
    write("Under the starlight");
    assert!(names(&lib, "moonlight").is_empty());
    assert_eq!(names(&lib, "starlight"), ["song.jpg"]);
    std::fs::remove_file(&lrc).unwrap();
    lib.rescan(None);
    idle(&lib);
    assert!(names(&lib, "starlight").is_empty());
}

#[test]
fn lyrics_are_searched_as_plain_words() {
    let d = Dir::new("lyrics-words");
    d.put(
        "m/song.jpg",
        "av=sound\nlyrics=Fly me to the Moonlight|and back again",
    );
    d.put("m/other.jpg", "av=sound");
    d.put("m/photo.jpg", "camera=Canon");
    let lib = open(&d, false);
    lib.add_location(&d.0.join("m"));
    idle(&lib);
    {
        let c = Connection::open(d.0.join("db/library.sqlite3")).unwrap();
        let n: i64 = c
            .query_row("SELECT COUNT(*) FROM file_text", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "{:?}", names(&lib, ""));
    }
    assert_eq!(names(&lib, "moonlight"), ["song.jpg"]);
    assert_eq!(names(&lib, "back again"), ["song.jpg"]);
    assert_eq!(names(&lib, "-moonlight"), ["other.jpg", "photo.jpg"]);
    // Only plain words: not a name.
    assert!(names(&lib, "name:moonlight").is_empty());
    let c = Connection::open(d.0.join("db/library.sqlite3")).unwrap();
    let n: i64 = c
        .query_row("SELECT COUNT(*) FROM file_text", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
}

/// The desktop search's view: read-only, the same results as the Library's own search,
/// nothing written, and none without a catalogue (or with one too old for this build).
#[test]
fn snapshot_reads_without_writing() {
    let d = Dir::new("snapshot");
    let db = d.0.join("db/library.sqlite3");
    assert!(Snapshot::open(&db, specular_query::ascii_fold, "ascii").is_err());
    assert!(!db.exists(), "never made");
    for (i, name) in ["beach a.jpg", "beach b.png", "hill.jpg"]
        .iter()
        .enumerate()
    {
        d.put(
            &format!("p/{name}"),
            &format!("taken={}", 1_600_000_000 + i as i64 * 86_400),
        );
    }
    let lib = open(&d, false);
    lib.add_location(&d.0.join("p"));
    idle(&lib);
    let snap = Snapshot::open(&db, specular_query::ascii_fold, "ascii").unwrap();
    let search = |q: &str, budget: Duration| {
        let req = Search {
            query: q.into(),
            sort: Sort::Date,
            descending: true,
            ..Search::default()
        };
        let mut names = Vec::new();
        let found = snap
            .search_each(&req, budget, &mut |r| names.push(r.name))
            .unwrap();
        (names, found.total)
    };
    let five = Duration::from_secs(5);
    assert_eq!(
        search("beach -ext:png", five),
        (vec!["beach a.jpg".into()], 1)
    );
    assert_eq!(
        search("beach", five).0,
        ["beach b.png", "beach a.jpg"],
        "newest first"
    );
    // A budget already spent: cut off, not an error.
    assert!(search("", Duration::ZERO).0.len() <= 3);
    drop(lib);
    // Searching after the Library closed writes nothing either.
    let before = std::fs::metadata(&db).unwrap().modified().unwrap();
    assert_eq!(search("hill", five).0, ["hill.jpg"]);
    assert_eq!(std::fs::metadata(&db).unwrap().modified().unwrap(), before);
    // A catalogue older than this build is not searched (it would need a migration).
    let old = d.0.join("old.sqlite3");
    let c = Connection::open(&old).unwrap();
    c.pragma_update(None, "user_version", 1).unwrap();
    drop(c);
    assert!(Snapshot::open(&old, specular_query::ascii_fold, "ascii").is_err());
}
