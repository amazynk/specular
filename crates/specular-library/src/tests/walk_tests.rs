use super::*;

#[test]
fn a_location_is_walked_and_its_facts_read() {
    let d = Dir::new("walk");
    let root = d.0.join("photos");
    d.put("photos/a.jpg", "camera=Canon EOS R5\niso=100\nw=60\nh=40");
    d.put("photos/year2024/b.png", "camera=Pixel 8\nw=30\nh=60");
    d.put("photos/year2024/deep/c.cr2", "camera=Canon EOS R5\nhdr");
    d.put("photos/year2024/notes.txt", "not an image");
    d.put("photos/.hidden/d.jpg", "hidden");
    d.put("photos/year2024/broken.jpg", "broken");
    let lib = open(&d, false);
    lib.add_location(&root);
    idle(&lib);
    let locs = lib.locations();
    assert_eq!(locs.len(), 1);
    assert_eq!(locs[0].files, 4);
    assert_eq!(locs[0].pending, 0);
    assert!(locs[0].last_scan.is_some());
    assert_eq!(lib.counts().unwrap(), (4, 4));
    assert_eq!(names(&lib, ""), ["a.jpg", "b.png", "broken.jpg", "c.cr2"]);
    assert_eq!(names(&lib, "camera:canon"), ["a.jpg", "c.cr2"]);
    assert_eq!(names(&lib, "canon"), ["a.jpg", "c.cr2"]);
    assert_eq!(names(&lib, "is:portrait"), ["b.png"]);
    assert_eq!(names(&lib, "in:deep"), ["c.cr2"]);
    assert_eq!(names(&lib, "year2024 -ext:cr2"), ["b.png", "broken.jpg"]);
    let a = row(&lib, "a.jpg");
    assert_eq!(a.state, 1);
    assert!(a.content.is_some());
    assert_eq!(a.meta.as_ref().unwrap().iso, Some(100));
    assert_eq!(row(&lib, "broken.jpg").state, 2);
    assert_eq!(lib.progress().phase, Phase::Idle);
}

/// A file the decoders panic on costs that file only: it is catalogued as unreadable (not
/// read again on every pass), one that panics when its pixels are signed is left without a
/// signature, and every other file is read and signed; the indexer settles.
#[test]
fn a_file_the_decoder_panics_on_costs_only_itself() {
    let d = Dir::new("panic");
    d.put("p/a.jpg", "look=1\ntaken=1000");
    d.put(
        &format!("p/b-{}.jpg", specular_core::isolate::TEST_PANIC_MARK),
        "look=2",
    );
    d.put("p/c.jpg", "look=3\npanic-on-pixels");
    d.put("p/e.jpg", "look=1\ntaken=1000");
    let lib = open(&d, false);
    lib.add_location(&d.0.join("p"));
    idle(&lib);
    let marked = format!("b-{}.jpg", specular_core::isolate::TEST_PANIC_MARK);
    assert_eq!(row(&lib, &marked).state, 2);
    for n in ["a.jpg", "c.jpg", "e.jpg"] {
        assert_eq!(row(&lib, n).state, 1, "{n}");
    }
    // `a` and `e` are signed and found to be duplicates; `c` has a row without a signature.
    assert_eq!(names(&lib, "dupes:any"), ["a.jpg", "e.jpg"]);
    let c = Connection::open(d.0.join("db/library.sqlite3")).unwrap();
    let (signed, unsigned): (i64, i64) = c
        .query_row(
            "SELECT COUNT(sig), COUNT(*) - COUNT(sig) FROM image_hashes",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((signed, unsigned), (2, 1));
    // Nothing is retried: another pass finds nothing to do.
    lib.add_location(&d.0.join("p"));
    idle(&lib);
    assert_eq!(row(&lib, &marked).state, 2);
}

/// A panic in the indexer itself (not in a file's reader) stops it with a reason the UI shows
/// ("Indexing stopped: …"); Retry starts it afresh, and what was asked meanwhile follows.
#[test]
fn a_stopped_indexer_says_why_and_retry_starts_it_again() {
    let d = Dir::new("halt");
    d.put("p/a.jpg", "look=1");
    d.put("q/b.jpg", "look=2");
    let lib = open(&d, false);
    let bad =
        d.0.join(format!("x-{}", specular_core::isolate::TEST_PANIC_MARK));
    std::fs::create_dir_all(&bad).unwrap();
    lib.add_location(&bad);
    assert!(eventually(|| lib.progress().halt.is_some()), "no halt");
    let p = lib.progress();
    assert!(!p.busy(), "a stopped indexer is not busy");
    match p.halt {
        Some(Halt::Failed { class, .. }) => assert_eq!(class, ErrorClass::Other),
        h => panic!("{h:?}"),
    }
    // Asked while stopped: kept, and done after Retry.
    lib.add_location(&d.0.join("p"));
    std::thread::sleep(Duration::from_millis(100));
    assert!(lib.progress().halt.is_some());
    lib.retry();
    assert!(eventually(|| lib.progress().halt.is_none()));
    idle(&lib);
    assert_eq!(names(&lib, ""), ["a.jpg"]);
    lib.add_location(&d.0.join("q"));
    idle(&lib);
    assert_eq!(names(&lib, ""), ["a.jpg", "b.jpg"]);
    assert!(lib.progress().halt.is_none());
}

/// A reader that stops answering (a hung drive) shows as stalled, never as an endless busy
/// bar, and the status clears by itself once it moves again.
#[test]
fn a_stalled_indexer_is_reported_and_clears_when_it_moves() {
    let d = Dir::new("stall");
    d.put("p/a.jpg", "look=1\nsleep-ms=1500");
    let mut cfg = config(d.0.join("db/library.sqlite3"), false);
    cfg.stall_after = Duration::from_millis(300);
    let lib = Library::open(cfg).unwrap();
    lib.add_location(&d.0.join("p"));
    assert!(
        eventually(|| matches!(lib.progress().halt, Some(Halt::Stalled { .. }))),
        "no stall reported"
    );
    assert!(!lib.progress().busy());
    idle(&lib);
    assert!(lib.progress().halt.is_none());
    assert_eq!(row(&lib, "a.jpg").state, 1);
}

/// A folder the walk panics on is skipped with what is below it; its siblings, the files
/// beside it and the folders after it are catalogued, and the indexer settles.
#[test]
fn a_folder_the_walk_panics_on_costs_only_itself() {
    let d = Dir::new("panic-folder");
    let mark = specular_core::isolate::TEST_SNIFF_MARK;
    d.put("p/a.jpg", "look=1");
    d.put(&format!("p/m-{mark}/x.jpg"), "look=2");
    d.put(&format!("p/m-{mark}/deeper/y.jpg"), "look=3");
    d.put("p/z/b.jpg", "look=4");
    let lib = open(&d, false);
    lib.add_location(&d.0.join("p"));
    idle(&lib);
    assert_eq!(names(&lib, ""), ["a.jpg", "b.jpg"]);
    assert_eq!(row(&lib, "b.jpg").state, 1);
    assert_eq!(lib.progress().phase, Phase::Idle);
}

#[test]
fn nested_locations_are_one() {
    let d = Dir::new("nested");
    d.put("all/inner/a.jpg", "");
    d.put("all/b.jpg", "");
    let lib = open(&d, false);
    lib.add_location(&d.0.join("all/inner"));
    idle(&lib);
    let a = row(&lib, "a.jpg").id;
    // Inside a location: nothing to add.
    lib.add_location(&d.0.join("all/inner"));
    idle(&lib);
    assert_eq!(lib.locations().len(), 1);
    // Around it: takes it over, rows and all.
    lib.add_location(&d.0.join("all"));
    idle(&lib);
    let locs = lib.locations();
    assert_eq!(locs.len(), 1);
    assert_eq!(
        locs[0].path,
        specular_platform::path::canonicalize(&d.0.join("all")).unwrap()
    );
    assert_eq!(locs[0].files, 2);
    assert_eq!(row(&lib, "a.jpg").id, a);
}

#[test]
fn removing_a_location_forgets_its_files_only() {
    let d = Dir::new("remove");
    d.put("one/a.jpg", "kw=sea");
    d.put("two/b.jpg", "kw=sky");
    let lib = open(&d, false);
    lib.add_location(&d.0.join("one"));
    lib.add_location(&d.0.join("two"));
    idle(&lib);
    assert_eq!(names(&lib, "tag:sea"), ["a.jpg"]);
    let one = lib
        .locations()
        .into_iter()
        .find(|l| l.path.ends_with("one"))
        .unwrap();
    lib.remove_location(one.id);
    idle(&lib);
    assert_eq!(names(&lib, ""), ["b.jpg"]);
    assert!(names(&lib, "a.jpg").is_empty());
    assert!(d.0.join("one/a.jpg").exists(), "files are never touched");
    let c = Connection::open(d.0.join("db/library.sqlite3")).unwrap();
    let fts: i64 = c
        .query_row("SELECT COUNT(*) FROM fts_index", [], |r| r.get(0))
        .unwrap();
    let tags: i64 = c
        .query_row("SELECT COUNT(*) FROM tags", [], |r| r.get(0))
        .unwrap();
    assert_eq!((fts, tags), (1, 1));
}

#[test]
fn moves_and_renames_keep_identity_and_marks() {
    let d = Dir::new("moves");
    let a = d.put("lib/x/a.jpg", "camera=Nikon Z8");
    d.put("lib/x/keep.jpg", "");
    let lib = open(&d, false);
    lib.add_location(&d.0.join("lib"));
    idle(&lib);
    let before = row(&lib, "a.jpg");
    lib.set_marks(vec![(
        specular_platform::path::canonicalize(&a).unwrap(),
        Mark {
            rating: 4,
            label: 2,
            flag: 1,
        },
    )]);
    idle(&lib);
    assert_eq!(names(&lib, "rating:4 is:pick"), ["a.jpg"]);
    // Rename and move into a new folder, and rename the old folder too.
    std::fs::create_dir_all(d.0.join("lib/y/z")).unwrap();
    std::fs::rename(&a, d.0.join("lib/y/z/renamed.jpg")).unwrap();
    std::fs::rename(d.0.join("lib/x"), d.0.join("lib/x2")).unwrap();
    lib.rescan(None);
    idle(&lib);
    let after = row(&lib, "renamed.jpg");
    assert_eq!(after.id, before.id);
    assert_eq!(after.mark.rating, 4);
    // Folders are stored with the platform's separator.
    assert!(Path::new(&after.folder).ends_with(Path::new("y").join("z")));
    assert_eq!(
        Path::new(&row(&lib, "keep.jpg").folder)
            .file_name()
            .and_then(|n| n.to_str()),
        Some("x2")
    );
    assert_eq!(names(&lib, ""), ["keep.jpg", "renamed.jpg"]);
    assert_eq!(names(&lib, "renamed"), ["renamed.jpg"]);
    assert_eq!(names(&lib, "in:x2"), ["keep.jpg"]);
    assert!(names(&lib, "in:lib/x/").is_empty());
}

/// Develop's rows (every version) follow a photo renamed and moved outside the app,
/// with its sidecars beside it; the sidecars are not catalogued as photos.
#[test]
fn moves_and_renames_keep_develop_edits() {
    let d = Dir::new("moves-edits");
    let a = d.put("lib/x/a.jpg", "camera=Nikon Z8");
    d.put("lib/x/a.jpg.specular.xmp", "<x/>");
    d.put("lib/x/a.jpg.v2.specular.xmp", "<x/>");
    let lib = open(&d, false);
    lib.add_location(&d.0.join("lib"));
    idle(&lib);
    let a = specular_platform::path::canonicalize(&a).unwrap();
    let stored = |v: u8| crate::edits::StoredEdit {
        edit: vec![1, 2, v],
        hash: vec![v; 32],
        steps: vec![vec![v]],
        position: 1,
        updated: 1_761_998_400,
        name: if v == 2 {
            "Square".into()
        } else {
            String::new()
        },
        modules: None,
        styles: None,
    };
    lib.save_edit(a.clone(), 0, stored(0));
    lib.save_edit(a.clone(), 2, stored(2));
    idle(&lib);
    assert_eq!(lib.edit_versions(&a).unwrap().len(), 2);
    std::fs::create_dir_all(d.0.join("lib/y")).unwrap();
    let to = d.0.join("lib/y/b.jpg");
    std::fs::rename(&a, &to).unwrap();
    for v in ["", ".v2"] {
        std::fs::rename(
            d.0.join(format!("lib/x/a.jpg{v}.specular.xmp")),
            d.0.join(format!("lib/y/b.jpg{v}.specular.xmp")),
        )
        .unwrap();
    }
    lib.rescan(None);
    idle(&lib);
    let to = specular_platform::path::canonicalize(&to).unwrap();
    assert_eq!(lib.load_edit(&to, 0).unwrap(), Some(stored(0)));
    assert_eq!(lib.load_edit(&to, 2).unwrap(), Some(stored(2)));
    assert_eq!(
        lib.edit_versions(&to).unwrap(),
        vec![(0, String::new()), (2, "Square".to_owned())]
    );
    assert_eq!(names(&lib, ""), ["b.jpg"]);
}

#[test]
fn a_missing_root_is_offline_not_emptied() {
    let d = Dir::new("offline");
    d.put("vol/a.jpg", "");
    d.put("vol/b.jpg", "");
    let lib = open(&d, false);
    lib.add_location(&d.0.join("vol"));
    idle(&lib);
    std::fs::rename(d.0.join("vol"), d.0.join("away")).unwrap();
    lib.rescan(None);
    idle(&lib);
    assert!(eventually(|| lib.locations()[0].offline));
    let found = lib.search(&Search::default()).unwrap();
    assert_eq!(found.rows.len(), 2, "offline files stay catalogued");
    assert!(found.rows.iter().all(|r| r.offline));
    std::fs::rename(d.0.join("away"), d.0.join("vol")).unwrap();
    assert!(eventually(|| !lib.locations()[0].offline));
    idle(&lib);
    assert_eq!(names(&lib, ""), ["a.jpg", "b.jpg"]);
}

/// A location whose folder cannot be listed is offline (the gallery says so) rather than an
/// empty location with no feedback, and comes back when it can be read.
#[cfg(unix)]
#[test]
fn an_unreadable_root_is_offline() {
    use std::os::unix::fs::PermissionsExt;
    let d = Dir::new("unreadable");
    d.put("vol/a.jpg", "");
    let vol = d.0.join("vol");
    std::fs::set_permissions(&vol, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read_dir(&vol).is_ok() {
        // Running with CAP_DAC_OVERRIDE: nothing is unreadable.
        std::fs::set_permissions(&vol, std::fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }
    let lib = open(&d, false);
    lib.add_location(&vol);
    idle(&lib);
    let offline = eventually(|| lib.locations()[0].offline);
    std::fs::set_permissions(&vol, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(offline, "an unreadable root is offline");
    assert!(eventually(|| !lib.locations()[0].offline));
    idle(&lib);
    assert_eq!(names(&lib, ""), ["a.jpg"]);
}

/// A FIFO named like a sidecar or a cache tag is not opened (opening one blocks until a writer
/// comes, which stalled the walk and the facts pass for good).
#[cfg(unix)]
#[test]
fn fifos_named_like_sidecars_or_cache_tags_do_not_block() {
    let d = Dir::new("fifos");
    d.put("f/a.jpg", "kw=Sunset");
    d.put("f/sub/b.jpg", "");
    for n in ["f/a.xmp", "f/sub/CACHEDIR.TAG"] {
        let ok = std::process::Command::new("mkfifo")
            .arg(d.0.join(n))
            .status()
            .is_ok_and(|s| s.success());
        assert!(ok, "mkfifo");
    }
    let lib = open(&d, false);
    lib.add_location(&d.0.join("f"));
    idle(&lib);
    assert_eq!(names(&lib, ""), ["a.jpg", "b.jpg"]);
    assert_eq!(names(&lib, "sunset"), ["a.jpg"]);
}

#[test]
fn watching_follows_changes() {
    let d = Dir::new("watch");
    d.put("w/a.jpg", "");
    let lib = open(&d, true);
    lib.add_location(&d.0.join("w"));
    idle(&lib);
    let a = row(&lib, "a.jpg").id;
    d.put("w/new/b.jpg", "camera=Sony A7");
    assert!(eventually(|| names(&lib, "camera:sony") == ["b.jpg"]));
    std::fs::rename(d.0.join("w/a.jpg"), d.0.join("w/new/a2.jpg")).unwrap();
    assert!(eventually(|| names(&lib, "") == ["a2.jpg", "b.jpg"]));
    assert_eq!(row(&lib, "a2.jpg").id, a, "a watched move keeps identity");
    std::fs::remove_file(d.0.join("w/new/b.jpg")).unwrap();
    assert!(eventually(|| names(&lib, "") == ["a2.jpg"]));
}

#[test]
fn tool_folders_and_folder_links_are_not_walked() {
    let d = Dir::new("tools");
    d.put("home/a.jpg", "");
    d.put("home/venv/pyvenv.cfg", "home = /usr/bin");
    d.put("home/venv/lib/v.jpg", "");
    d.put("home/cache/CACHEDIR.TAG", CACHEDIR_TAG);
    d.put("home/cache/sub/c.jpg", "");
    d.put("home/notacache/CACHEDIR.TAG", "just a file");
    d.put("home/notacache/k.jpg", "");
    d.put("home/web/node_modules/pkg/n.jpg", "");
    d.put("home/code/__pycache__/p.jpg", "");
    d.put("home/py/site-packages/s.jpg", "");
    d.put("outside/o.jpg", "");
    #[cfg(unix)]
    std::os::unix::fs::symlink(d.0.join("outside"), d.0.join("home/link")).unwrap();
    let lib = open(&d, false);
    lib.add_location(&d.0.join("home"));
    idle(&lib);
    assert_eq!(names(&lib, ""), ["a.jpg", "k.jpg"]);
    // A catalogued folder that becomes a cache goes.
    d.put("home/later/l.jpg", "");
    lib.rescan(None);
    idle(&lib);
    assert_eq!(names(&lib, ""), ["a.jpg", "k.jpg", "l.jpg"]);
    d.put("home/later/CACHEDIR.TAG", CACHEDIR_TAG);
    lib.rescan(None);
    idle(&lib);
    assert_eq!(names(&lib, ""), ["a.jpg", "k.jpg"]);
    assert!(names(&lib, "in:later").is_empty());
}

#[test]
fn developer_folders_are_skipped_and_the_list_can_change() {
    let d = Dir::new("devdirs");
    d.put("home/a.jpg", "");
    d.put("home/proj/.git/HEAD", "ref: refs/heads/main");
    d.put("home/proj/docs/p.jpg", "");
    // A git worktree or submodule has a `.git` file.
    d.put("home/wt/.git", "gitdir: ../proj/.git/worktrees/wt");
    d.put("home/wt/w.jpg", "");
    d.put("home/rust/target/debug/t.jpg", "");
    d.put("home/cpp/build/b.jpg", "");
    d.put("home/data/corpus/c.jpg", "");
    d.put("home/trips/2024/trip.jpg", "");
    let lib = open(&d, false);
    // The location itself is never skipped, even when it is a checkout.
    d.put("home/.git/HEAD", "ref: refs/heads/main");
    lib.add_location(&d.0.join("home"));
    idle(&lib);
    assert_eq!(names(&lib, ""), ["a.jpg", "trip.jpg"]);
    // Taking `build` off the list and the `.git` mark: walked again, they come in.
    lib.set_skip(Skip::parse("target, corpus", "pyvenv.cfg"));
    idle(&lib);
    assert_eq!(
        names(&lib, ""),
        ["a.jpg", "b.jpg", "p.jpg", "trip.jpg", "w.jpg"]
    );
    // And go again when added back (`trips` named too).
    lib.set_skip(Skip::parse("target, corpus, build, trips", ".git"));
    idle(&lib);
    assert_eq!(names(&lib, ""), ["a.jpg"]);
    assert_eq!(lib.skip().names(), ["target", "corpus", "build", "trips"]);
}

#[test]
fn a_stopped_indexer_takes_new_skip_rules_at_the_next_rescan() {
    let d = Dir::new("devdirs-stopped");
    d.put("home/a.jpg", "");
    d.put("home/build/b.jpg", "");
    let lib = open(&d, false);
    lib.add_location(&d.0.join("home"));
    idle(&lib);
    assert_eq!(names(&lib, ""), ["a.jpg"]);
    lib.cancel();
    idle(&lib);
    lib.set_skip(Skip::none());
    idle(&lib);
    assert_eq!(names(&lib, ""), ["a.jpg"]);
    assert!(lib.progress().stopped);
    lib.rescan(None);
    idle(&lib);
    assert_eq!(names(&lib, ""), ["a.jpg", "b.jpg"]);
}

#[test]
fn names_ending_in_wide_characters_are_listed() {
    let d = Dir::new("wide");
    d.put("r/фото_б.jpg", "");
    d.put("r/фото_б.xmp", &SIDECAR.replace("RATING", "3"));
    // Names whose last four bytes begin inside a character.
    for n in ["аб", "ааб", "б.xmp", "xб", "фото_б"] {
        d.put(&format!("r/{n}"), "");
    }
    let lib = open(&d, false);
    lib.add_location(&d.0.join("r"));
    idle(&lib);
    assert_eq!(names(&lib, ""), ["фото_б.jpg"]);
    assert_eq!(row(&lib, "фото_б.jpg").mark.rating, 3);
}

#[test]
fn the_walk_shows_its_progress_and_rows_as_it_goes() {
    let d = Dir::new("progress");
    // Three folders of 1100: a commit falls within the walk whichever order they are listed.
    for f in ["a", "b", "c"] {
        for i in 0..1100 {
            d.put(&format!("big/{f}/{i}.jpg"), "");
        }
    }
    let shared: Arc<std::sync::OnceLock<Arc<Shared>>> = Arc::default();
    let seen: Arc<parking_lot::Mutex<Vec<Progress>>> = Arc::default();
    let mut c = config(d.0.join("db/library.sqlite3"), false);
    let (s2, seen2) = (shared.clone(), seen.clone());
    c.is_image = Arc::new(move |p: &Path| {
        if let Some(s) = s2.get() {
            seen2.lock().push(s.progress.lock().clone());
        }
        p.extension().is_some_and(|e| e == "jpg")
    });
    let lib = Library::open(c).unwrap();
    shared.set(lib.shared.clone()).ok().unwrap();
    lib.add_location(&d.0.join("big"));
    idle(&lib);
    let seen = seen.lock();
    assert_eq!(seen.len(), 3300);
    assert!(
        seen.iter()
            .all(|p| p.phase == Phase::Walking && p.location.is_some()),
        "the phase is Walking while the walk lists files"
    );
    assert!(seen.last().unwrap().walked >= 2200, "the count moves");
    assert!(
        seen.last().unwrap().changes > seen[0].changes,
        "rows committed during the walk are announced"
    );
    assert_eq!(lib.counts().unwrap().0, 3300);
}

/// Stopping the indexing: a walk stops at its next folder and the reading at each worker's
/// next file; what was catalogued stays (searchable, the rest counted as pending) and
/// nothing more is read until a rescan, which finishes the job.
#[test]
fn cancel_stops_walking_and_reading_within_bounded_items() {
    use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
    const FOLDERS: usize = 40;
    const PER: usize = 10;
    let d = Dir::new("cancel");
    for f in 0..FOLDERS {
        for i in 0..PER {
            d.put(&format!("photos/f{f}/{i}.jpg"), "camera=Canon");
        }
    }
    let listed = Arc::new(AtomicUsize::new(0));
    let probed = Arc::new(AtomicUsize::new(0));
    let mut c = config(d.0.join("db/library.sqlite3"), false);
    let l2 = listed.clone();
    c.is_image = Arc::new(move |p: &Path| {
        l2.fetch_add(1, SeqCst);
        std::thread::sleep(Duration::from_millis(1));
        p.extension().is_some_and(|e| e == "jpg")
    });
    c.prober = Arc::new(SlowProber(probed.clone()));
    c.threads = 2;
    c.batch = 64;
    let threads = c.threads;
    let lib = Library::open(c).unwrap();
    let stopped = |lib: &Library| {
        eventually(|| {
            let p = lib.progress();
            p.stopped && p.phase == Phase::Idle
        })
    };

    // During the walk: at most the folder being listed is finished.
    lib.add_location(&d.0.join("photos"));
    reach(&listed, 3 * PER);
    let at = listed.load(SeqCst);
    lib.cancel();
    assert!(stopped(&lib), "the indexer took the cancel in");
    let walked = listed.load(SeqCst);
    assert!(walked <= at + PER, "listed {walked} after cancel at {at}");
    assert!(walked < FOLDERS * PER);
    idle(&lib);
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(listed.load(SeqCst), walked, "no walk after the cancel");
    assert_eq!(probed.load(SeqCst), 0, "no reading after the cancel");
    let (files, read) = lib.counts().unwrap();
    assert!(files > 0 && files as usize <= walked, "{files} catalogued");
    assert_eq!(read, 0);
    assert_eq!(
        lib.locations()[0].pending,
        files,
        "the rest is marked pending"
    );
    assert_eq!(
        names(&lib, "").len() as u64,
        files,
        "what was walked is searchable"
    );

    // Rescan: the walk finishes; then stop the reading part way.
    lib.rescan(None);
    reach(&probed, 20);
    let at = probed.load(SeqCst);
    lib.cancel();
    assert!(stopped(&lib));
    let read_n = probed.load(SeqCst);
    assert!(
        read_n <= at + threads,
        "probed {read_n} after cancel at {at}"
    );
    idle(&lib);
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(probed.load(SeqCst), read_n, "no reading after the cancel");
    let (files, read) = lib.counts().unwrap();
    assert_eq!(files as usize, FOLDERS * PER);
    assert!(
        read > 0 && (read as usize) <= read_n,
        "{read} read of {read_n}"
    );
    assert_eq!(lib.locations()[0].pending, files - read);
    assert_eq!(names(&lib, "").len() as u64, files);

    // Rescan again: everything is read and the mark goes.
    lib.rescan(None);
    idle(&lib);
    assert!(eventually(
        || lib.counts().unwrap().1 as usize == FOLDERS * PER
    ));
    idle(&lib);
    let p = lib.progress();
    assert!(!p.stopped);
    assert_eq!(lib.locations()[0].pending, 0);
}

#[test]
fn indexed_folders_are_watched_and_tool_folders_not() {
    let d = Dir::new("watchdirs");
    d.put("w/x/y/z/a.jpg", "");
    d.put("w/web/node_modules/pkg/n.jpg", "");
    if specular_thumbs::watch::FolderWatch::folders(&d.0, Arc::new(|| {})).is_err() {
        // No inotify (sandboxed CI): nothing to test.
        return;
    }
    let lib = open(&d, true);
    lib.add_location(&d.0.join("w"));
    idle(&lib);
    assert_eq!(names(&lib, ""), ["a.jpg"]);
    // Changes deep down are seen without a rescan (each folder has its watch).
    d.put("w/x/y/z/b.jpg", "");
    assert!(eventually(|| names(&lib, "") == ["a.jpg", "b.jpg"]));
    // In a tool folder they are not, and nothing walks it.
    d.put("w/web/node_modules/pkg/m.jpg", "");
    d.put("w/web/node_modules/new.jpg", "");
    d.put("w/x/c.jpg", "");
    assert!(eventually(|| names(&lib, "") == ["a.jpg", "b.jpg", "c.jpg"]));
    idle(&lib);
    std::thread::sleep(Duration::from_millis(400));
    idle(&lib);
    assert_eq!(names(&lib, ""), ["a.jpg", "b.jpg", "c.jpg"]);
}
