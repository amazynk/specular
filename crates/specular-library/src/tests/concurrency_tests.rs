use super::*;

/// Debug builds: opening the catalogue, searching it and the AI pack's connection all panic on
/// the thread marked as the UI's, and work from any other thread.
#[test]
#[cfg(debug_assertions)]
fn catalogue_refuses_the_ui_thread() {
    let dir = Dir::new("ui-thread");
    let db = dir.0.join("db/library.sqlite3");
    let trips = |what: &str, f: &dyn Fn()| {
        let _ui = specular_platform::UiThreadScope::enter();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        let msg = r.expect_err(what);
        let msg = msg.downcast_ref::<String>().cloned().unwrap_or_default();
        assert!(msg.contains("on the UI thread"), "{what}: {msg}");
    };
    trips("open", &|| {
        let _ = Library::open(config(db.clone(), false));
    });
    trips("ai::open", &|| {
        let _ = ai::open(&db);
    });
    // Off the UI thread all of it works; on it, search and counts trip too.
    let lib = Library::open(config(db.clone(), false)).unwrap();
    idle(&lib);
    assert!(lib.search(&Search::default()).is_ok());
    trips("search", &|| {
        let _ = lib.search(&Search::default());
    });
    trips("counts", &|| {
        let _ = lib.counts();
    });
    assert!(ai::open(&db).is_ok());
}

/// Two connections that waited out another writer migrate a new catalogue at once (the
/// Library and the AI pack opening while it was held): the second finds the work done instead
/// of failing on "table already exists".
#[test]
fn concurrent_migrations_after_a_held_lock() {
    let dir = Dir::new("migrate-race");
    let db = dir.0.join("library.sqlite3");
    {
        let c = Connection::open(&db).unwrap();
        c.pragma_update(None, "journal_mode", "WAL").unwrap();
    }
    let holder = Connection::open(&db).unwrap();
    holder
        .execute_batch("BEGIN IMMEDIATE; CREATE TABLE held(x);")
        .unwrap();
    let both: Vec<_> = (0..2)
        .map(|_| {
            let db = db.clone();
            std::thread::spawn(move || {
                let mut c = Connection::open(&db).unwrap();
                schema::configure(&c).unwrap();
                schema::migrate_checked(&mut c).map_err(|e| e.to_string())
            })
        })
        .collect();
    std::thread::sleep(Duration::from_millis(300));
    holder.execute_batch("ROLLBACK").unwrap();
    for t in both {
        assert_eq!(t.join().unwrap(), Ok(0));
    }
    let c = Connection::open(&db).unwrap();
    let v: u32 = c
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(v, schema::VERSION);
}
/// The AI pack's writes wait out another writer instead of failing with "database is locked".
/// `ai::install` reads its version first and then writes: as a deferred transaction, SQLite
/// refused the upgrade to a write at once while the lock was held (the busy handler does not
/// run for that), which made the Library tests flaky under load.
#[test]
fn ai_writes_wait_out_another_writer() {
    let dir = Dir::new("ai-busy");
    let db = dir.0.join("library.sqlite3");
    drop(ai::open(&db).unwrap());
    let holder = Connection::open(&db).unwrap();
    holder
        .execute_batch("BEGIN IMMEDIATE; CREATE TABLE held(x);")
        .unwrap();
    let t = {
        let db = db.clone();
        std::thread::spawn(move || {
            let mut c = ai::open(&db).map_err(|e| e.to_string())?;
            let found = ai::install(&mut c, "people").map_err(|e| e.to_string())?;
            ai::forget_all_faces(&mut c).map_err(|e| e.to_string())?;
            Ok::<_, String>(found)
        })
    };
    std::thread::sleep(Duration::from_millis(300));
    holder.execute_batch("ROLLBACK").unwrap();
    assert_eq!(t.join().unwrap(), Ok(0));
}

/// Measurement (not a gate): the walk of `SPECULAR_WALK_TREE` with the app's `is_image`
/// (`Registry::supports_file`, which sniffs some files) and with names only.
#[test]
#[ignore]
fn walk_speed_of_a_tree() {
    let Some(tree) = std::env::var_os("SPECULAR_WALK_TREE") else {
        return;
    };
    let tree = PathBuf::from(tree);
    let reg = Arc::new(specular_decode::Registry::new());
    let mut all = Vec::new();
    let mut stack = vec![tree.clone()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let ft = e.file_type().unwrap();
            if ft.is_dir() {
                stack.push(e.path());
            } else {
                all.push(e.path());
            }
        }
    }
    let t = Instant::now();
    let n = all.iter().filter(|p| reg.supports_extension(p)).count();
    let ext = t.elapsed();
    let t = Instant::now();
    let m = all.iter().filter(|p| reg.supports_file(p)).count();
    let file = t.elapsed();
    let t = Instant::now();
    let _ = all.iter().filter(|p| reg.supports_file(p)).count();
    let file2 = t.elapsed();
    eprintln!(
        "{} files: supports_extension {n} in {ext:.2?} ({:.0}/s); supports_file {m} in {file:.2?} ({:.0}/s), again {file2:.2?}",
        all.len(),
        all.len() as f64 / ext.as_secs_f64(),
        all.len() as f64 / file.as_secs_f64(),
    );
    for (what, sniff) in [("names", false), ("supports_file", true)] {
        let d = Dir::new(&format!("walkspeed-{what}"));
        let mut c = config(d.0.join("db/library.sqlite3"), false);
        let r = reg.clone();
        c.is_image = if sniff {
            Arc::new(move |p: &Path| r.supports_file(p))
        } else {
            Arc::new(move |p: &Path| r.supports_extension(p))
        };
        c.signer = None;
        let lib = Library::open(c).unwrap();
        let t = Instant::now();
        lib.add_location(&tree);
        assert!(eventually(|| lib.progress().phase == Phase::Walking));
        let mut last = (Instant::now(), 0u64);
        let mut longest = Duration::ZERO;
        while lib.progress().phase == Phase::Walking || lib.progress().location.is_some() {
            let w = lib.progress().walked;
            if w != last.1 {
                longest = longest.max(last.0.elapsed());
                last = (Instant::now(), w);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let walk = t.elapsed();
        eprintln!(
            "walk ({what}): {} images of {} files in {walk:.2?} ({:.0} files/s), longest still count {longest:.0?}",
            lib.counts().unwrap().0,
            all.len(),
            all.len() as f64 / walk.as_secs_f64()
        );
    }
}

/// Removing a location mid-walk: it leaves the locations and the results at once, and the
/// walk stops and the rows go within 200 ms (they once waited for the whole walk).
#[test]
fn removing_a_location_mid_walk_takes_effect_at_once() {
    use std::sync::atomic::Ordering::SeqCst;
    let d = Dir::new("remove-walk");
    let (c, listed) = slow_tree(&d, "big", 60, 40);
    let lib = Library::open(c).unwrap();
    lib.add_location(&d.0.join("big"));
    reach(&listed, 200);
    assert!(eventually(|| !lib.locations().is_empty()));
    let id = lib.locations()[0].id;
    let t = Instant::now();
    lib.remove_location(id);
    let call = t.elapsed();
    assert!(
        lib.locations().is_empty(),
        "gone from the locations at once"
    );
    assert!(names(&lib, "").is_empty(), "gone from the results at once");
    let done = within(Duration::from_secs(5), || {
        lib.counts().unwrap().0 == 0 && lib.progress().phase == Phase::Idle
    })
    .expect("the removal was done");
    eprintln!("remove_location returned in {call:?}; walk stopped and rows gone in {done:?}");
    assert!(call < Duration::from_millis(20), "{call:?}");
    assert!(done < Duration::from_millis(200), "removal took {done:?}");
    let at = listed.load(SeqCst);
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(listed.load(SeqCst), at, "the walk did not go on");
    assert!(at < 60 * 40);
    assert!(lib.locations().is_empty());
}

/// Removing one location while another is walked: the walk steps aside for the removal and
/// is then walked again to its end.
#[test]
fn another_locations_walk_goes_on_after_a_removal() {
    let d = Dir::new("remove-other");
    let (c, listed) = slow_tree(&d, "big", 30, 30);
    d.put("small/a.jpg", "kw=sea");
    let lib = Library::open(c).unwrap();
    lib.add_location(&d.0.join("small"));
    idle(&lib);
    lib.add_location(&d.0.join("big"));
    reach(&listed, 200);
    let small = lib
        .locations()
        .into_iter()
        .find(|l| l.path.ends_with("small"))
        .unwrap();
    lib.remove_location(small.id);
    idle(&lib);
    assert_eq!(lib.locations().len(), 1);
    assert_eq!(lib.counts().unwrap().0, 30 * 30, "the big walk finished");
    assert!(names(&lib, "a.jpg").is_empty());
}

/// Removing a location while its facts are read: the readers step aside at their next file.
#[test]
fn removing_a_location_while_its_facts_are_read() {
    let d = Dir::new("remove-facts");
    for i in 0..400 {
        d.put(&format!("photos/f{}/{i}.jpg", i / 50), "camera=Canon");
    }
    let probed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut c = config(d.0.join("db/library.sqlite3"), false);
    c.prober = Arc::new(SlowProber(probed.clone()));
    c.threads = 2;
    let lib = Library::open(c).unwrap();
    lib.add_location(&d.0.join("photos"));
    reach(&probed, 20);
    let id = lib.locations()[0].id;
    lib.remove_location(id);
    assert!(lib.locations().is_empty());
    let done = within(Duration::from_secs(5), || {
        lib.counts().unwrap().0 == 0 && lib.progress().phase == Phase::Idle
    })
    .expect("the removal was done");
    eprintln!("removal during facts reading done in {done:?}");
    // ~10 ms alone; the whole suite running beside it can take it past 200 ms. Readers that
    // did not step aside would still be probing all 400 files (checked below).
    assert!(done < Duration::from_secs(1), "removal took {done:?}");
    idle(&lib);
    assert!(probed.load(std::sync::atomic::Ordering::SeqCst) < 400);
}

/// Rows reach the results while the walk goes on (committed about every half second), not
/// only at its end.
#[test]
fn rows_are_searchable_before_the_walk_ends() {
    let d = Dir::new("rows-early");
    let (c, _listed) = slow_tree(&d, "big", 40, 50);
    let lib = Library::open(c).unwrap();
    lib.add_location(&d.0.join("big"));
    let seen = within(Duration::from_secs(20), || {
        lib.progress().phase == Phase::Walking
            && !names(&lib, "").is_empty()
            && lib.progress().phase == Phase::Walking
    });
    let seen = seen.expect("no rows were searchable during the walk");
    eprintln!("first rows searchable {seen:?} into the walk");
    idle(&lib);
    assert_eq!(lib.counts().unwrap().0, 2000);
}

/// The duty cycle: a hold renewed without a break waits 300 ms and works 100 ms, so
/// background work keeps a quarter of the time; browsing is said all along.
#[test]
fn a_continuous_hold_leaves_a_quarter_of_the_time() {
    let d = Dir::new("duty");
    let lib = open(&d, false);
    let (mut free, mut all) = (0u32, 0u32);
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(1600) {
        lib.shared.hold(Duration::from_millis(50));
        assert!(lib.shared.browsing());
        all += 1;
        free += u32::from(!lib.shared.held());
        std::thread::sleep(Duration::from_millis(2));
    }
    let share = f64::from(free) / f64::from(all);
    eprintln!("free {share:.2} of a continuous hold");
    assert!((0.15..0.40).contains(&share), "free {share:.2}");
    std::thread::sleep(Duration::from_millis(60));
    assert!(
        !lib.shared.held() && !lib.shared.browsing(),
        "a lapsed hold is gone"
    );
}

/// Browsing without a break slows the walk; it never stops it (a redraw that renewed the
/// hold 20 times a second once kept a walk at the same count).
#[test]
fn a_hold_cannot_starve_the_walk() {
    use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
    let d = Dir::new("starve");
    let (c, listed) = slow_tree(&d, "big", 100, 20);
    let lib = Library::open(c).unwrap();
    lib.add_location(&d.0.join("big"));
    reach(&listed, 40);
    let stop = Arc::new(AtomicBool::new(false));
    let (shared, s2) = (lib.shared.clone(), stop.clone());
    #[allow(clippy::disallowed_methods)]
    let holder = std::thread::spawn(move || {
        while !s2.load(SeqCst) {
            shared.hold(Duration::from_millis(50));
            std::thread::sleep(Duration::from_millis(10));
        }
    });
    std::thread::sleep(Duration::from_millis(20));
    let a = lib.progress().walked;
    let t = Instant::now();
    let mut last = (a, Instant::now());
    let mut longest = Duration::ZERO;
    while t.elapsed() < Duration::from_millis(2000) {
        let w = lib.progress().walked;
        if w != last.0 {
            longest = longest.max(last.1.elapsed());
            last = (w, Instant::now());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    longest = longest.max(last.1.elapsed());
    let b = lib.progress().walked;
    assert!(lib.held(), "browsing was said");
    stop.store(true, SeqCst);
    holder.join().unwrap();
    eprintln!("held 2 s: walked {a} -> {b}, longest with the count still {longest:?}");
    assert!(b > a + 50, "the walk moved while held: {a} -> {b}");
    assert!(
        longest < Duration::from_millis(1000),
        "still for {longest:?}"
    );
    idle(&lib);
    assert_eq!(lib.counts().unwrap().0, 2000);
}
