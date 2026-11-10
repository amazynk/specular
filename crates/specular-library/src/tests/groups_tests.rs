use super::*;

/// Forgets the matcher's stored links and state: the next matcher takes in everything.
fn forget_state(c: &Connection) {
    c.execute_batch("DELETE FROM match_links; DELETE FROM info WHERE key = 'match_state'")
        .unwrap();
}

#[test]
fn duplicates_and_similar_shots_are_grouped_across_folders() {
    use specular_query::facet::FacetField;
    let d = Dir::new("dupes");
    // `a` twice, byte for byte, in two folders; `b` the same pixels in another file; `c` a
    // second shot of the same scene a second later; `x` something else; `n` unreadable.
    d.put("p/2024/a.jpg", "look=1\ntaken=1000");
    d.put("p/backup/a.jpg", "look=1\ntaken=1000");
    d.put("p/2024/b.jpg", "look=1\ntaken=1000\ncamera=resaved");
    d.put("p/2024/c.jpg", "near=1\ntaken=1001");
    d.put("p/2024/x.jpg", "look=7\ntaken=99999");
    d.put("p/2024/n.jpg", "taken=1000");
    let lib = open(&d, false);
    lib.add_location(&d.0.join("p"));
    idle(&lib);
    assert_eq!(names(&lib, "dupes:any"), ["a.jpg", "a.jpg", "b.jpg"]);
    assert_eq!(names(&lib, "dupes:keep").len(), 1);
    assert_eq!(names(&lib, "dupes:discard").len(), 2);
    assert_eq!(group_counts(&lib, FacetField::Dupes), [3, 1, 2]);
    // The similar shots: the duplicates' keeper and `c` (the copies to discard are stood for
    // by their keeper, as in the gallery).
    let sim = names(&lib, "similar:any");
    assert_eq!(sim.len(), 2, "{sim:?}");
    assert!(sim.contains(&"c.jpg".to_owned()));
    assert_eq!(names(&lib, "similar:keep").len(), 1);
    assert_eq!(group_counts(&lib, FacetField::Similar), [2, 1, 1]);
    assert!(names(&lib, "x.jpg dupes:any").is_empty());
    assert!(names(&lib, "x.jpg similar:any").is_empty());
    assert!(names(&lib, "n.jpg dupes:any").is_empty());
    // The checks are remembered: the byte-equal pair and the pixel-equal one.
    let c = Connection::open(d.0.join("db/library.sqlite3")).unwrap();
    let checks: Vec<(i64, f64)> = c
        .prepare("SELECT tier, score FROM match_verify ORDER BY tier")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(checks.contains(&(0, 1.0)), "{checks:?}");
    assert!(
        checks.iter().any(|&(t, s)| t == 1 && s > 0.99),
        "{checks:?}"
    );
    let hashed: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM image_hashes WHERE blake3 IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        hashed, 2,
        "only files of equal size and content key are read whole"
    );

    // A mark protects: a copy suggested for discarding is kept once it is rated.
    let p = paths(&lib, "dupes:discard")[0].clone();
    lib.set_marks(vec![(
        p.clone(),
        Mark {
            rating: 4,
            label: 0,
            flag: 0,
        },
    )]);
    idle(&lib);
    assert!(paths(&lib, "dupes:keep").contains(&p));

    // Departures and changes regroup: the backup goes, `x` becomes `b`'s pixels.
    std::fs::remove_dir_all(d.0.join("p/backup")).unwrap();
    std::fs::write(d.0.join("p/2024/x.jpg"), "look=1\ntaken=1000\ncamera=other").unwrap();
    lib.rescan(None);
    idle(&lib);
    assert_eq!(names(&lib, "dupes:any"), ["a.jpg", "b.jpg", "x.jpg"]);
    assert_eq!(group_counts(&lib, FacetField::Dupes)[0], 3);
    let rows = |c: &Connection, cat: i64| -> i64 {
        c.query_row(
            "SELECT COUNT(*) FROM match_groups WHERE cat = ?1",
            [cat],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(rows(&c, 0), 3);
    let similar = names(&lib, "similar:any");
    assert!(similar.contains(&"c.jpg".to_owned()), "{similar:?}");

    // Reopened, the groups come from the stored signatures: nothing is signed again.
    drop(lib);
    let mut cfg = config(d.0.join("db/library.sqlite3"), false);
    let signer = Arc::new(TextSigner {
        made: Default::default(),
    });
    cfg.signer = Some(signer.clone());
    let lib = Library::open(cfg).unwrap();
    idle(&lib);
    assert_eq!(signer.made.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(names(&lib, "dupes:any"), ["a.jpg", "b.jpg", "x.jpg"]);
    assert_eq!(names(&lib, "similar:any"), similar);

    // A removed location takes its groups with it.
    let id = lib.locations()[0].id;
    lib.remove_location(id);
    idle(&lib);
    assert_eq!(rows(&c, 0) + rows(&c, 1), 0);
    assert!(names(&lib, "dupes:any").is_empty());
}

#[test]
fn matching_in_parts_equals_a_whole_pass_and_a_scan() {
    let (_d, c, cfg) = matcher_catalogue("parts");
    synthetic_catalogue(&c, 700, 7);
    let signer = BenchSigner;
    matches::Matches::new().sync(&c, &cfg, &signer).unwrap();
    let whole = match_rows(&c);
    assert!(whole.iter().any(|r| r.1 == 0) && whole.iter().any(|r| r.1 == 1));
    assert_eq!(groups_of(&whole), scan_groups(&c));

    // The same from nothing, seven signatures at a time: no groups are seen before the pass
    // is through.
    c.execute_batch(
        "DELETE FROM match_groups; DELETE FROM match_verify; UPDATE image_hashes SET blake3 = NULL;
         DELETE FROM match_links; DELETE FROM info WHERE key = 'match_state'",
    )
    .unwrap();
    let small = matches::Budget {
        time: Duration::ZERO,
        rows: 7,
        yield_now: &|| false,
    };
    let mut m = matches::Matches::new();
    let mut calls = 0;
    while !m.sync_some(&c, &cfg, &signer, &small).unwrap().done {
        calls += 1;
        assert!(m.part_way());
        assert!(match_rows(&c).is_empty(), "half-formed groups written");
    }
    assert!(calls >= 100, "{calls}");
    assert!(!m.part_way());
    // The roles were staged in parts and swapped in at once.
    assert!(m.staged_parts > 3, "{}", m.staged_parts);
    assert!(m.last_swap.is_some());
    assert_eq!(match_rows(&c), whole);

    // Changes in parts: the last pass's roles stay until the next is through.
    let mut r = 99u64;
    resign(&c, &mut (1..=700).step_by(3), &mut r);
    assert!(!m.sync_some(&c, &cfg, &signer, &small).unwrap().done);
    assert_eq!(match_rows(&c), whole);
    while !m.sync_some(&c, &cfg, &signer, &small).unwrap().done {
        assert_eq!(match_rows(&c), whole);
    }
    let after = match_rows(&c);
    assert_ne!(after, whole);
    // A whole pass from the stored state, and one from nothing, find what the parts wrote,
    // and so does a scan.
    assert_eq!(matches::Matches::new().sync(&c, &cfg, &signer).unwrap(), 0);
    forget_state(&c);
    assert_eq!(matches::Matches::new().sync(&c, &cfg, &signer).unwrap(), 0);
    assert_eq!(groups_of(&after), scan_groups(&c));
}

#[test]
fn a_reader_sees_whole_passes_only() {
    let (d, c, cfg) = matcher_catalogue("swap");
    synthetic_catalogue(&c, 700, 3);
    let db = d.0.join("c.sqlite3");
    let signer = BenchSigner;
    let small = matches::Budget {
        time: Duration::ZERO,
        rows: 7,
        yield_now: &|| false,
    };
    let mut m = matches::Matches::new();
    // The first pass: nothing, then all of it.
    let seen = observed(&db, || {
        while !m.sync_some(&c, &cfg, &signer, &small).unwrap().done {
            std::thread::sleep(Duration::from_micros(200));
        }
    });
    let whole = match_rows(&c);
    assert!(!whole.is_empty());
    for s in &seen {
        assert!(
            s.is_empty() || *s == whole,
            "a reader saw {} of {} roles",
            s.len(),
            whole.len()
        );
    }
    let staged = m.staged_parts;
    assert!(staged > 3, "{staged}");

    // Half the catalogue signed again: the old groups, then the new ones.
    let mut r = 5u64;
    resign(&c, &mut (1..=700).step_by(2), &mut r);
    let seen = observed(&db, || {
        while !m.sync_some(&c, &cfg, &signer, &small).unwrap().done {
            std::thread::sleep(Duration::from_micros(200));
        }
    });
    let after = match_rows(&c);
    assert_ne!(after, whole);
    assert!(m.staged_parts > staged + 1, "the changes were staged");
    for s in &seen {
        assert!(
            *s == whole || *s == after,
            "a reader saw a mix of two passes"
        );
    }
    // Foreign keys are back on after the swap.
    assert!(
        c.pragma_query_value(None, "foreign_keys", |r| r.get::<_, bool>(0))
            .unwrap()
    );
}

#[test]
fn marks_and_renames_reach_the_groups_without_reading_everything() {
    let (_d, c, cfg) = matcher_catalogue("marks");
    synthetic_catalogue(&c, 400, 11);
    let signer = BenchSigner;
    let mut m = matches::Matches::new();
    m.sync(&c, &cfg, &signer).unwrap();
    assert_eq!(m.protect_reads, 1);
    let rows0 = m.protect_rows;
    // At rest nothing is read again.
    m.sync(&c, &cfg, &signer).unwrap();
    m.sync(&c, &cfg, &signer).unwrap();
    assert_eq!(m.protect_reads, 1);
    let role = |id: i64| -> Option<(i64, bool)> {
        match_rows(&c)
            .into_iter()
            .find(|r| r.0 == id && r.1 == 0)
            .map(|r| (r.2, r.3))
    };
    let discards: Vec<i64> = match_rows(&c)
        .into_iter()
        .filter(|r| r.1 == 0 && !r.3)
        .map(|r| r.0)
        .collect();
    assert!(discards.len() >= 3, "{discards:?}");

    // Rated, a copy suggested for discarding is kept.
    let a = discards[0];
    c.execute("UPDATE files SET rating = 3 WHERE id = ?1", [a])
        .unwrap();
    m.sync(&c, &cfg, &signer).unwrap();
    assert_eq!(m.protect_reads, 2);
    // Only that file's marks are read again.
    assert_eq!(m.protect_rows, rows0 + 1);
    assert_eq!(role(a).map(|r| r.1), Some(true));
    // The same mark written again (a sidecar read again) is no change.
    c.execute("UPDATE files SET rating = 3 WHERE id = ?1", [a])
        .unwrap();
    m.sync(&c, &cfg, &signer).unwrap();
    assert_eq!(m.protect_reads, 2);
    // A tag of the user's protects; a file's own keywords do not.
    let b = discards[1];
    c.execute_batch("INSERT INTO tags (name) VALUES ('mine'), ('camera')")
        .unwrap();
    c.execute(
        "INSERT INTO file_tags (file_id, tag_id, source)
         SELECT ?1, id, 0 FROM tags WHERE name = 'camera'",
        [discards[2]],
    )
    .unwrap();
    m.sync(&c, &cfg, &signer).unwrap();
    assert_eq!(m.protect_reads, 2);
    c.execute(
        "INSERT INTO file_tags (file_id, tag_id, source)
         SELECT ?1, id, 2 FROM tags WHERE name = 'mine'",
        [b],
    )
    .unwrap();
    m.sync(&c, &cfg, &signer).unwrap();
    assert_eq!(m.protect_reads, 3);
    assert_eq!(m.protect_rows, rows0 + 2);
    assert_eq!(role(b).map(|r| r.1), Some(true));
    assert_eq!(role(discards[2]).map(|r| r.1), Some(false));
    // Unrated, back to the suggestion.
    c.execute("UPDATE files SET rating = 0 WHERE id = ?1", [a])
        .unwrap();
    m.sync(&c, &cfg, &signer).unwrap();
    assert_eq!(m.protect_reads, 4);
    assert_eq!(m.protect_rows, rows0 + 3);
    assert_eq!(role(a).map(|r| r.1), Some(false));
    // A hand tag taken off is read alone too.
    c.execute(
        "DELETE FROM file_tags WHERE file_id = ?1 AND source >= 1",
        [b],
    )
    .unwrap();
    m.sync(&c, &cfg, &signer).unwrap();
    assert_eq!(m.protect_reads, 5);
    assert_eq!(m.protect_rows, rows0 + 4);
    assert_eq!(role(b).map(|r| r.1), Some(false));

    // Renamed as a copy, the kept file of a pair hands keeping to the other; nothing is
    // signed again.
    let rows = match_rows(&c);
    let (keeper, other) = rows
        .iter()
        .filter(|r| r.1 == 0 && r.3 && r.0 != a && r.0 != b)
        .find_map(|k| {
            let members: Vec<_> = rows.iter().filter(|r| r.1 == 0 && r.2 == k.2).collect();
            (members.len() == 2).then(|| (k.0, members.iter().find(|r| r.0 != k.0).unwrap().0))
        })
        .expect("a pair of duplicates");
    assert_eq!(role(other).map(|r| r.1), Some(false));
    let seq: i64 = c
        .query_row("SELECT MAX(seq) FROM image_hashes", [], |r| r.get(0))
        .unwrap();
    c.execute(
        "UPDATE files SET name = 'IMG_1 copy.jpg' WHERE id = ?1",
        [keeper],
    )
    .unwrap();
    m.sync(&c, &cfg, &signer).unwrap();
    assert_eq!(role(keeper).map(|r| r.1), Some(false));
    assert_eq!(role(other).map(|r| r.1), Some(true));
    assert_eq!(
        c.query_row("SELECT MAX(seq) FROM image_hashes", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        seq
    );
    assert_eq!(m.protect_reads, 5);
    // And a matcher started afresh agrees with all of it, from the stored state or from
    // nothing.
    assert_eq!(matches::Matches::new().sync(&c, &cfg, &signer).unwrap(), 0);
    forget_state(&c);
    assert_eq!(matches::Matches::new().sync(&c, &cfg, &signer).unwrap(), 0);
}

/// Work counted, not timed: a matcher started on a catalogue with nothing changed since the
/// last pass loads nothing and matches nothing; after one changed file it loads the stored
/// links and matches only around that file; marks made and files gone while it was not
/// loaded reach the groups. Each time the roles are those a pass from nothing writes.
#[test]
fn a_restart_matches_nothing_and_a_change_only_around_it() {
    let (_d, c, cfg) = matcher_catalogue("restart");
    synthetic_catalogue(&c, 3000, 13);
    let signer = BenchSigner;
    let mut first = matches::Matches::new();
    first.sync(&c, &cfg, &signer).unwrap();
    let all = first.work;
    assert!(all > 10_000, "{all}");
    let whole = match_rows(&c);
    assert!(!whole.is_empty());
    let links: i64 = c
        .query_row("SELECT COUNT(*) FROM match_links", [], |r| r.get(0))
        .unwrap();
    assert!(links > 100, "{links} files with stored links");
    // Agrees with a pass from nothing (and so with a scan, per the test above).
    let from_nothing = |c: &Connection| {
        let rows = match_rows(c);
        forget_state(c);
        assert_eq!(matches::Matches::new().sync(c, &cfg, &signer).unwrap(), 0);
        assert_eq!(match_rows(c), rows);
    };

    // A restart, nothing changed: nothing loaded, nothing matched, nothing written.
    let mut m = matches::Matches::new();
    for _ in 0..3 {
        assert_eq!(m.sync(&c, &cfg, &signer).unwrap(), 0);
    }
    assert_eq!((m.work, m.restores, m.resident()), (0, 0, false));
    assert_eq!(match_rows(&c), whole);

    // One file signed again: the stored state is loaded once, the work is around the file.
    let mut r = 31u64;
    resign(&c, &mut std::iter::once(1500), &mut r);
    m.sync(&c, &cfg, &signer).unwrap();
    assert_eq!((m.restores, m.resident()), (1, true));
    assert!(m.work > 0 && m.work < all / 50, "{} of {all}", m.work);
    let w = m.work;
    resign(&c, &mut std::iter::once(700), &mut r);
    m.sync(&c, &cfg, &signer).unwrap();
    assert_eq!(m.restores, 1, "loaded once");
    assert!(m.work - w < all / 50, "{} of {all}", m.work - w);
    from_nothing(&c);

    // A mark made while no matcher was loaded (the log keeps it): the file is kept.
    let discard = match_rows(&c)
        .into_iter()
        .find(|r| r.1 == 0 && !r.3)
        .expect("a copy to discard")
        .0;
    let mut m = matches::Matches::new();
    m.sync(&c, &cfg, &signer).unwrap();
    assert!(!m.resident());
    c.execute("UPDATE files SET rating = 5 WHERE id = ?1", [discard])
        .unwrap();
    let mut m = matches::Matches::new();
    m.sync(&c, &cfg, &signer).unwrap();
    assert!(m.work < all / 50, "{} of {all}", m.work);
    let kept = match_rows(&c)
        .into_iter()
        .any(|r| r.0 == discard && r.1 == 0 && r.3);
    assert!(kept, "the rated copy is kept");
    let mut again = matches::Matches::new();
    again.sync(&c, &cfg, &signer).unwrap();
    assert!(!again.resident(), "the log was cleared with the roles");
    from_nothing(&c);

    // Files that left while no matcher was loaded: their links and groups go.
    let grouped: Vec<i64> = match_rows(&c)
        .into_iter()
        .filter(|r| r.1 == 0)
        .map(|r| r.0)
        .step_by(7)
        .take(20)
        .collect();
    for id in &grouped {
        c.execute("DELETE FROM files WHERE id = ?1", [id]).unwrap();
    }
    let mut m = matches::Matches::new();
    m.check_gone = true;
    m.sync(&c, &cfg, &signer).unwrap();
    assert!(m.work < all / 10, "{} of {all}", m.work);
    let stale: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM match_links WHERE file_id NOT IN (SELECT id FROM files)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(stale, 0);
    from_nothing(&c);
}

/// The matcher's memory comes out of the process's budget: refused, it lets go of what it
/// holds and pauses, and the groups stay those of the last finished pass.
#[test]
fn the_memory_budget_caps_the_matcher() {
    use specular_core::budget::{MemoryBudget, with_budget};
    let (_d, c, cfg) = matcher_catalogue("budget");
    synthetic_catalogue(&c, 2000, 19);
    let signer = BenchSigner;
    let tight = Arc::new(MemoryBudget::fixed(1 << 20));
    let mut m = matches::Matches::new();
    let n = with_budget(tight.clone(), || m.sync(&c, &cfg, &signer).unwrap());
    assert_eq!(n, 0);
    assert!(!m.resident());
    assert!(match_rows(&c).is_empty(), "no half-formed groups");
    assert_eq!(tight.reserved(), 0, "what it held is given back");
    // Paused: asked again at once, it does nothing.
    assert_eq!(with_budget(tight, || m.sync(&c, &cfg, &signer).unwrap()), 0);
    assert!(!m.resident());
    // With room it forms the groups, and holds its share while it keeps them.
    let roomy = Arc::new(MemoryBudget::fixed(1 << 32));
    let mut m = matches::Matches::new();
    with_budget(roomy.clone(), || m.sync(&c, &cfg, &signer).unwrap());
    assert!(!match_rows(&c).is_empty());
    assert!(roomy.reserved() > 1 << 20, "{}", roomy.reserved());
    drop(m);
    assert_eq!(roomy.reserved(), 0);
}

#[test]
fn a_renamed_copy_is_judged_by_its_new_name() {
    let d = Dir::new("rename-dupes");
    d.put("p/a.jpg", "look=3\ntaken=1000");
    d.put("p/b/a copy.jpg", "look=3\ntaken=1000");
    let mut cfg = config(d.0.join("db/library.sqlite3"), false);
    let signer = Arc::new(TextSigner {
        made: Default::default(),
    });
    cfg.signer = Some(signer.clone());
    let lib = Library::open(cfg).unwrap();
    lib.add_location(&d.0.join("p"));
    idle(&lib);
    assert_eq!(names(&lib, "dupes:keep"), ["a.jpg"]);
    assert_eq!(names(&lib, "dupes:discard"), ["a copy.jpg"]);
    let made = signer.made.load(std::sync::atomic::Ordering::Relaxed);
    std::fs::rename(d.0.join("p/b/a copy.jpg"), d.0.join("p/b/original.jpg")).unwrap();
    std::fs::rename(d.0.join("p/a.jpg"), d.0.join("p/a copy.jpg")).unwrap();
    lib.rescan(None);
    idle(&lib);
    assert_eq!(names(&lib, "dupes:keep"), ["original.jpg"]);
    assert_eq!(names(&lib, "dupes:discard"), ["a copy.jpg"]);
    assert_eq!(
        signer.made.load(std::sync::atomic::Ordering::Relaxed),
        made,
        "renames are not signed again"
    );
}

/// Catalogue-wide matching at 100k files with synthetic signatures: the first pass (whole,
/// in the indexer's parts, and in the indexer itself with commands arriving), arrivals and
/// departures, marks, a sync at rest, and memory.
///
/// ```text
/// cargo test --release -p specular-library -- --ignored --nocapture matching_100k
/// ```
#[test]
#[ignore = "bench"]
fn matching_100k() {
    const N: usize = 100_000;
    let (_d, c, mut cfg) = matcher_catalogue("bench");
    let t = Instant::now();
    let copies = synthetic_catalogue(&c, N, 42);
    println!(
        "{N} files ({copies} byte copies) written in {:.1} s",
        t.elapsed().as_secs_f64()
    );
    let signer = BenchSigner;
    let clear = |c: &Connection| {
        c.execute_batch(
            "DELETE FROM match_groups; DELETE FROM match_verify; UPDATE image_hashes SET blake3 = NULL;
         DELETE FROM match_links; DELETE FROM info WHERE key = 'match_state'",
        )
        .unwrap();
    };

    // The first pass at once.
    let (rss0, _) = rss_kib();
    let t = Instant::now();
    let n = matches::Matches::new().sync(&c, &cfg, &signer).unwrap();
    let full = t.elapsed();
    let whole = match_rows(&c);
    let groups = |c: &Connection, cat: i64| -> (i64, i64) {
        c.query_row(
            "SELECT COUNT(DISTINCT grp), COUNT(*) FROM match_groups WHERE cat = ?1",
            [cat],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
    };
    let (dg, dm) = groups(&c, 0);
    let (sg, sm) = groups(&c, 1);
    println!(
        "first pass at once: {:.0} ms, {n} rows written; {dg} duplicate groups ({dm} files), {sg} similar groups ({sm} files)",
        full.as_secs_f64() * 1e3
    );

    // The first pass in the indexer's parts (30 ms or 128 signatures).
    clear(&c);
    let budget = matches::Budget {
        time: Duration::from_millis(30),
        rows: 128,
        yield_now: &|| false,
    };
    let mut m = matches::Matches::new();
    let mut calls = Vec::new();
    let t = Instant::now();
    loop {
        let t = Instant::now();
        let s = m.sync_some(&c, &cfg, &signer, &budget).unwrap();
        calls.push(t.elapsed().as_secs_f64() * 1e3);
        if s.done {
            break;
        }
    }
    let total = t.elapsed();
    let (rss1, hwm1) = rss_kib();
    let last = *calls.last().unwrap();
    let ncalls = calls.len();
    let max = calls.iter().copied().fold(0.0, f64::max);
    let p99 = percentile(&mut calls, 99);
    println!(
        "first pass in parts: {:.0} ms in {ncalls} parts; longest part {max:.1} ms, p99 {p99:.1} ms; the last (writing the roles) {last:.1} ms",
        total.as_secs_f64() * 1e3
    );
    assert_eq!(match_rows(&c), whole, "parts and a whole pass agree");
    println!(
        "  roles staged in {} parts, swapped into match_groups in {:.1} ms, then {:.2} ms to settle",
        m.staged_parts,
        m.last_swap.unwrap_or_default().as_secs_f64() * 1e3,
        m.last_settle.unwrap_or_default().as_secs_f64() * 1e3
    );
    println!(
        "memory: RSS {} → {} MiB (+{:.1} MiB for the matcher and its reads), peak {} MiB",
        rss0 / 1024,
        rss1 / 1024,
        (rss1 as f64 - rss0 as f64) / 1024.0,
        hwm1 / 1024
    );

    // Arrivals: new signatures for some files (a re-edit), one at a time and a batch.
    let mut r = 7u64;
    let mut one = Vec::new();
    for k in 0..200 {
        resign(&c, &mut std::iter::once(1 + (k * 499) % N as i64), &mut r);
        let t = Instant::now();
        m.sync(&c, &cfg, &signer).unwrap();
        one.push(t.elapsed().as_secs_f64() * 1e3);
    }
    println!(
        "one changed file: median {:.2} ms, p95 {:.2} ms",
        percentile(&mut one, 50),
        percentile(&mut one, 95)
    );
    resign(&c, &mut (0..1000).map(|k| 1 + (k * 97) % N as i64), &mut r);
    let t = Instant::now();
    m.sync(&c, &cfg, &signer).unwrap();
    println!(
        "1000 changed files: {:.1} ms",
        t.elapsed().as_secs_f64() * 1e3
    );
    // Departures.
    c.execute("DELETE FROM files WHERE id % 100 = 7", [])
        .unwrap();
    m.check_gone = true;
    let t = Instant::now();
    m.sync(&c, &cfg, &signer).unwrap();
    println!("1000 files gone: {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    // Marks: one rating, then a sync at rest.
    let rows = m.protect_rows;
    c.execute("UPDATE files SET rating = 2 WHERE id = 12", [])
        .unwrap();
    let t = Instant::now();
    m.sync(&c, &cfg, &signer).unwrap();
    println!(
        "one mark changed: {:.2} ms",
        t.elapsed().as_secs_f64() * 1e3
    );
    c.execute("UPDATE files SET rating = 0 WHERE id = 12", [])
        .unwrap();
    let t = Instant::now();
    m.sync(&c, &cfg, &signer).unwrap();
    println!(
        "  and back: {:.2} ms ({} protections read for the two)",
        t.elapsed().as_secs_f64() * 1e3,
        m.protect_rows - rows
    );
    let reads = m.protect_reads;
    let mut rest = Vec::new();
    for _ in 0..50 {
        let t = Instant::now();
        m.sync(&c, &cfg, &signer).unwrap();
        rest.push(t.elapsed().as_secs_f64() * 1e3);
    }
    assert_eq!(m.protect_reads, reads, "a sync at rest reads no marks");
    println!(
        "nothing changed: median {:.3} ms, max {:.3} ms",
        percentile(&mut rest, 50),
        percentile(&mut rest, 100)
    );
    let (rss2, hwm2) = rss_kib();
    println!(
        "memory after: RSS {} MiB, peak {} MiB",
        rss2 / 1024,
        hwm2 / 1024
    );
    drop(m);

    // The first pass in the indexer, a command every 5 ms meanwhile: how long each waits.
    // The synthetic catalogue has no full-text index yet (no fold named), so the indexer
    // builds one first, in parts, as for a catalogue whose fold changed.
    clear(&c);
    let db = cfg.path.clone();
    let lib = Library::open(cfg.clone()).unwrap();
    let t = Instant::now();
    let mut waits = Vec::new();
    let mut rebuilt: Option<(Duration, usize)> = None;
    while !c
        .query_row("SELECT EXISTS (SELECT 1 FROM match_groups)", [], |r| {
            r.get::<_, bool>(0)
        })
        .unwrap()
    {
        assert!(t.elapsed() < Duration::from_secs(120), "no groups");
        let s = Instant::now();
        lib.rescan(Some(-1));
        while lib.shared.queued.load(std::sync::atomic::Ordering::Acquire) > 0 {
            std::thread::sleep(Duration::from_micros(100));
        }
        waits.push(s.elapsed().as_secs_f64() * 1e3);
        if rebuilt.is_none() && store::info_get(&c, "fold").unwrap().is_some() {
            rebuilt = Some((t.elapsed(), waits.len()));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let pass = t.elapsed();
    let (rebuilt, during) = rebuilt.unwrap_or_default();
    let mut rebuild_waits = waits[..during].to_vec();
    println!(
        "full-text index built by the indexer in {:.0} ms; the {during} commands meanwhile waited median {:.2} ms, max {:.1} ms (the first {:.1} ms)",
        rebuilt.as_secs_f64() * 1e3,
        percentile(&mut rebuild_waits, 50),
        percentile(&mut rebuild_waits, 100),
        waits.first().copied().unwrap_or(0.0)
    );
    let n = waits.len();
    println!(
        "first pass in the indexer: {:.0} ms; the {n} commands meanwhile waited median {:.2} ms, p99 {:.1} ms, max {:.1} ms",
        pass.as_secs_f64() * 1e3,
        percentile(&mut waits, 50),
        percentile(&mut waits, 99),
        percentile(&mut waits, 100)
    );
    drop(lib);

    // Search with the roles: the pre-pass over `match_groups`.
    drop(c);
    cfg = config(db, false);
    cfg.signer = None;
    let lib = Library::open(cfg).unwrap();
    let t = Instant::now();
    let f = lib
        .search(&Search {
            query: "dupes:discard".into(),
            ..Search::default()
        })
        .unwrap();
    println!(
        "search dupes:discard over {}: {} found in {:.0} ms",
        N - 1000,
        f.total,
        t.elapsed().as_secs_f64() * 1e3
    );
}
