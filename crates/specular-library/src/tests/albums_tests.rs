//! Albums: manual and smart albums in search, `album:` terms, and the date timeline.

use super::*;
use crate::albums::Kind;
use specular_query::rules::{Combine, Rule, Rules};

fn found(lib: &Library, req: Search) -> Vec<String> {
    let mut v: Vec<String> = lib
        .search(&req)
        .unwrap()
        .rows
        .into_iter()
        .map(|r| r.name)
        .collect();
    v.sort();
    v
}

#[test]
fn manual_albums_hold_files_and_answer_album_terms() {
    let d = Dir::new("albums-manual");
    for i in 0..6 {
        d.put(
            &format!("shoot/img_{i}.jpg"),
            &format!("camera=X\niso={}", 100 * (i + 1)),
        );
    }
    let lib = open(&d, false);
    lib.add_location(&d.0.join("shoot"));
    idle(&lib);
    let p = |i: u32| d.0.join(format!("shoot/img_{i}.jpg"));
    let sel = lib
        .album_create(" Client X / selects ", Kind::Manual, &Rules::default())
        .unwrap();
    let all = lib
        .album_create("Client X/all", Kind::Manual, &Rules::default())
        .unwrap();
    // A taken name fails in the catalogue too (the UI checks first).
    assert!(
        lib.album_create("client x/SELECTS", Kind::Manual, &Rules::default())
            .is_err()
    );
    let (n, missing) = lib
        .album_add(sel, &[p(0), p(1), p(1), d.0.join("elsewhere.jpg")])
        .unwrap();
    assert_eq!(n, 2);
    assert_eq!(missing, vec![d.0.join("elsewhere.jpg")]);
    lib.album_add(all, &[p(0), p(1), p(2), p(3)]).unwrap();
    let albums = lib.albums().unwrap();
    assert_eq!(
        albums
            .iter()
            .map(|a| (a.name.as_str(), a.files))
            .collect::<Vec<_>>(),
        vec![("Client X/all", 4), ("Client X/selects", 2)]
    );
    let only = |album: Option<i64>, q: &str| {
        found(
            &lib,
            Search {
                query: q.into(),
                album,
                ..Search::default()
            },
        )
    };
    assert_eq!(only(Some(sel), ""), vec!["img_0.jpg", "img_1.jpg"]);
    assert_eq!(only(Some(all), "iso:>=300"), vec!["img_2.jpg", "img_3.jpg"]);
    assert_eq!(only(None, "album:selects"), vec!["img_0.jpg", "img_1.jpg"]);
    assert_eq!(
        only(None, "album:\"client x\" -album:selects"),
        vec!["img_2.jpg", "img_3.jpg"]
    );
    assert_eq!(only(None, "-album:client").len(), 2);
    assert_eq!(
        row(&lib, "img_0.jpg").albums.as_deref(),
        Some("Client X/all\nClient X/selects")
    );
    // Out again; deleting an album leaves its files.
    assert_eq!(lib.album_remove(sel, &[p(1), p(5)]).unwrap(), 1);
    assert_eq!(only(Some(sel), ""), vec!["img_0.jpg"]);
    lib.album_delete(&[sel]).unwrap();
    assert_eq!(only(None, "album:selects"), Vec::<String>::new());
    assert_eq!(only(None, "").len(), 6);
    // A group renamed moves its albums.
    assert_eq!(lib.album_rename_group("client x", "Client Y").unwrap(), 1);
    assert_eq!(lib.albums().unwrap()[0].name, "Client Y/all");
    lib.album_rename(all, "Archive / 2025").unwrap();
    assert_eq!(only(None, "album=\"archive/2025\"").len(), 4);
    // A file leaving the catalogue leaves its albums.
    std::fs::remove_file(p(0)).unwrap();
    lib.rescan(None);
    idle(&lib);
    assert_eq!(lib.albums().unwrap()[0].files, 3);
}

#[test]
fn smart_albums_run_their_rules_with_the_query_on_top() {
    let d = Dir::new("albums-smart");
    for i in 0..10u32 {
        let cam = if i % 2 == 0 { "Canon" } else { "Nikon" };
        d.put(
            &format!("s/img_{i}.jpg"),
            &format!(
                "camera={cam}\niso={}\nkw={}",
                100 * (i + 1),
                if i < 3 { "client" } else { "other" }
            ),
        );
    }
    let lib = open(&d, false);
    lib.add_location(&d.0.join("s"));
    idle(&lib);
    let rules = Rules(vec![
        Rule::new(Combine::And, "camera:canon"),
        Rule::new(Combine::Or, "tag:client"),
        Rule::new(Combine::Except, "iso:>=900"),
    ]);
    let id = lib
        .album_create("Client X/smart", Kind::Smart, &rules)
        .unwrap();
    let a = &lib.albums().unwrap()[0];
    assert_eq!((a.id, a.kind, &a.rules), (id, Kind::Smart, &rules));
    let run = |q: &str, push: bool| {
        found(
            &lib,
            Search {
                query: q.into(),
                rules: Some(rules.clone()),
                no_pushdown: !push,
                ..Search::default()
            },
        )
    };
    let want = vec![
        "img_0.jpg",
        "img_1.jpg",
        "img_2.jpg",
        "img_4.jpg",
        "img_6.jpg",
    ];
    assert_eq!(run("", true), want);
    assert_eq!(run("", false), want, "pushdown changes nothing");
    assert_eq!(run("iso:<=200", true), vec!["img_0.jpg", "img_1.jpg"]);
    // Only and-rules narrow in SQL: these all go through.
    let and_only = Rules(vec![
        Rule::new(Combine::And, "camera:nikon"),
        Rule::new(Combine::And, "iso:>=500"),
    ]);
    let f = lib
        .search(&Search {
            rules: Some(and_only.clone()),
            ..Search::default()
        })
        .unwrap();
    assert_eq!(f.total, 3);
    assert!(f.candidates < 10, "narrowed in SQL: {}", f.candidates);
    lib.album_set_rules(id, &and_only).unwrap();
    assert_eq!(lib.albums().unwrap()[0].rules, and_only);
    // Facets count inside the album.
    let f = lib
        .search(&Search {
            rules: Some(rules.clone()),
            facets: true,
            ..Search::default()
        })
        .unwrap();
    assert_eq!(f.total, 5);
}

#[test]
fn the_timeline_counts_days_without_its_own_date_terms() {
    let d = Dir::new("albums-timeline");
    let day = |y, m, dd| specular_query::date::days_from_civil(y, m, dd);
    let at = |y, m, dd| day(y, m, dd) * 86_400 + 3600;
    let shots = [
        (at(2024, 5, 1), 2),
        (at(2024, 5, 2), 1),
        (at(2025, 1, 9), 3),
    ];
    let mut k = 0;
    for (t, n) in shots {
        for _ in 0..n {
            d.put(&format!("t/img_{k}.jpg"), &format!("taken={t}\niso=100"));
            k += 1;
        }
    }
    let lib = open(&d, false);
    lib.add_location(&d.0.join("t"));
    idle(&lib);
    let tl = |q: &str| {
        lib.search(&Search {
            query: q.into(),
            timeline: true,
            ..Search::default()
        })
        .unwrap()
    };
    let f = tl("");
    assert_eq!(
        f.timeline,
        vec![
            (day(2024, 5, 1), 2),
            (day(2024, 5, 2), 1),
            (day(2025, 1, 9), 3)
        ]
    );
    // A date picked narrows the rows, not the timeline.
    let f = tl("date:2025");
    assert_eq!(f.total, 3);
    assert_eq!(f.timeline.len(), 3);
    // Other terms narrow both.
    let f = tl("date:2025 img_0");
    assert_eq!(f.total, 0);
    assert_eq!(f.timeline, vec![(day(2024, 5, 1), 1)]);
    let bars = specular_query::timeline::bars(&tl("").timeline, 20);
    assert_eq!(bars.len(), 9, "May 2024 to January 2025");
    assert_eq!(bars[0].count, 3);
}

#[test]
fn albums_migrate_and_read_only_catalogues_refuse_writes() {
    let d = Dir::new("albums-ro");
    let path = d.0.join("db/library.sqlite3");
    {
        let lib = open(&d, false);
        lib.album_create("A", Kind::Manual, &Rules::default())
            .unwrap();
    }
    let c = rusqlite::Connection::open(&path).unwrap();
    c.pragma_update(None, "user_version", schema::VERSION + 1)
        .unwrap();
    drop(c);
    let lib = open(&d, false);
    assert!(lib.read_only());
    assert_eq!(lib.albums().unwrap().len(), 1);
    assert!(
        lib.album_create("B", Kind::Manual, &Rules::default())
            .is_err()
    );
}

#[test]
fn search_sees_album_changes_made_after_the_last_search() {
    // The search keeps the side tables between searches; a change must still show.
    let d = Dir::new("albums-side-cache");
    for i in 0..3 {
        d.put(&format!("s/img_{i}.jpg"), "camera=X");
    }
    let lib = open(&d, false);
    lib.add_location(&d.0.join("s"));
    idle(&lib);
    let a = lib
        .album_create("Keep", Kind::Manual, &Rules::default())
        .unwrap();
    let albums_of = |name: &str| {
        lib.search(&Search::default())
            .unwrap()
            .rows
            .into_iter()
            .find(|r| r.name == name)
            .and_then(|r| r.albums)
            .unwrap_or_default()
    };
    assert_eq!(albums_of("img_1.jpg"), "");
    assert_eq!(albums_of("img_1.jpg"), "", "searched again unchanged");
    lib.album_add(a, &[d.0.join("s/img_1.jpg")]).unwrap();
    assert_eq!(albums_of("img_1.jpg"), "Keep");
}
