use super::*;

/// The folder watches ran out: under the locations, "Some folders aren't watched" says when
/// changes there show up, and opens the help on raising the limit (a hit area of its own).
#[test]
fn library_unwatched_matches_goldens() {
    use super::library::Side;
    use specular_library::{Location, Progress};
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let pal = Palette::dark();
    let mut g = Gallery {
        active: true,
        ..Gallery::default()
    };
    let lv = &mut g.library;
    lv.on = true;
    lv.panel = true;
    lv.searched = true;
    lv.locations = vec![Location {
        id: 1,
        path: PathBuf::from("/shots/Pictures"),
        offline: false,
        files: 18_204,
        pending: 0,
        last_scan: None,
    }];
    lv.progress = Progress {
        out_of_watches: true,
        ..Progress::default()
    };
    g.rebuild();
    let f = Frame {
        pal: &pal,
        now: 1.0,
        reduce_motion: true,
        size: SIZE,
        scale: 1.0,
        rtl: false,
        text_scale: 1.0,
        backdrop: Backdrop::None,
    };
    let shot = h.render(
        SIZE,
        1.0,
        |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
        |dl, text| {
            g.draw(dl, text, &f);
        },
    );
    goldens.check("library-unwatched-dark-1x", &shot);
    assert!(g.hits.iter().any(|h| h.1 == Hit::Library(Side::Watches)));
    goldens.finish();
}

/// The Library: its title and result count in the top bar, the locations sidebar (all
/// locations selected, one offline, one still reading) and the indexer's progress at its
/// foot, beside the grid of results; in dark at 1× and 1.5×, light at 1×.
#[test]
fn library_matches_goldens() {
    use specular_library::{Location, Phase, Progress};
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    for (look, dark, scale, suffix) in [
        ("dark", true, 1.0, "1x"),
        ("dark", true, 1.5, "1.5x"),
        ("light", false, 1.0, "1x"),
    ] {
        let pal = if dark {
            Palette::dark()
        } else {
            Palette::light()
        };
        let mut g = gallery();
        g.filter.text = "beach rating:>=0".into();
        g.filter_open = false;
        let lv = &mut g.library;
        lv.on = true;
        lv.panel = true;
        lv.searched = true;
        lv.was_busy = true;
        lv.locations = vec![
            Location {
                id: 1,
                path: PathBuf::from("/shots/Pictures"),
                offline: false,
                files: 18_204,
                pending: 1_250,
                last_scan: None,
            },
            Location {
                id: 2,
                path: PathBuf::from("/media/archive/Photos 2019"),
                offline: true,
                files: 4_410,
                pending: 0,
                last_scan: None,
            },
        ];
        lv.progress = Progress {
            phase: Phase::Reading,
            to_read: 5_000,
            read: 3_750,
            ..Progress::default()
        };
        g.rebuild();
        let f = Frame {
            pal: &pal,
            now: 1.0,
            reduce_motion: true,
            size: SIZE,
            scale,
            rtl: false,
            text_scale: 1.0,
            backdrop: Backdrop::None,
        };
        for _ in 0..20 {
            let mut warm = DrawList::new(scale);
            h.text.begin_frame(scale);
            g.draw(&mut warm, &mut h.text, &f);
            if !g.cap_more {
                break;
            }
        }
        assert!(!g.cap_more, "captions still pending");
        let shot = h.render(
            SIZE,
            scale,
            |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
            |dl, text| {
                g.draw(dl, text, &f);
            },
        );
        goldens.check(&format!("library-{look}-{suffix}"), &shot);
    }
    goldens.finish();
}

/// The Library after its indexer stopped by itself: the foot says why in two lines
/// ("Indexing stopped: something went wrong inside Specular. …") with Save diagnostics… and
/// Retry, and no busy bar; a stalled one says nothing moved. Dark at 1×, light at 1.5×.
#[test]
fn library_halted_matches_goldens() {
    use super::library::Side;
    use specular_library::{ErrorClass, Halt, Location, Phase, Progress};
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let root = std::env::temp_dir().join(format!("specular-halted-shot-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut cfg = specular_library::Config::new(root.join("library.sqlite3"));
    cfg.watch = false;
    let lib = Arc::new(specular_library::Library::open(cfg).unwrap());
    for (name, dark, scale, halt) in [
        (
            "library-halted-dark-1x",
            true,
            1.0,
            Halt::Failed {
                class: ErrorClass::Other,
                io: None,
                detail: "test".into(),
            },
        ),
        (
            "library-stalled-light-1.5x",
            false,
            1.5,
            Halt::Stalled { secs: 180 },
        ),
    ] {
        let pal = if dark {
            Palette::dark()
        } else {
            Palette::light()
        };
        let mut g = gallery();
        g.filter.text.clear();
        g.filter_open = false;
        let lv = &mut g.library;
        lv.on = true;
        lv.panel = true;
        lv.searched = true;
        lv.was_busy = true;
        lv.lib = Some(lib.clone());
        lv.locations = vec![Location {
            id: 1,
            path: PathBuf::from("/shots/Pictures"),
            offline: false,
            files: 18_204,
            pending: 1_250,
            last_scan: None,
        }];
        lv.progress = Progress {
            phase: Phase::Reading,
            to_read: 5_000,
            read: 3_750,
            halt: Some(halt),
            ..Progress::default()
        };
        assert_eq!(lv.foot_buttons(), vec![Side::Diagnostics, Side::Retry]);
        g.rebuild();
        let f = Frame {
            pal: &pal,
            now: 1.0,
            reduce_motion: true,
            size: SIZE,
            scale,
            rtl: false,
            text_scale: 1.0,
            backdrop: Backdrop::None,
        };
        for _ in 0..20 {
            let mut warm = DrawList::new(scale);
            h.text.begin_frame(scale);
            g.draw(&mut warm, &mut h.text, &f);
            if !g.cap_more {
                break;
            }
        }
        let shot = h.render(
            SIZE,
            scale,
            |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
            |dl, text| {
                g.draw(dl, text, &f);
            },
        );
        goldens.check(name, &shot);
        // Both buttons are hit areas, in reading order for the keyboard.
        for s in [Side::Diagnostics, Side::Retry] {
            assert!(g.hits.iter().any(|h| h.1 == Hit::Library(s)), "{s:?}");
        }
        let rows: Vec<Side> = g.library.rows.iter().map(|r| r.0).collect();
        assert!(
            rows.ends_with(&[Side::Diagnostics, Side::Retry]),
            "{rows:?}"
        );
    }
    drop(lib);
    let _ = std::fs::remove_dir_all(&root);
    goldens.finish();
}

/// The Library walking a new location, the grid still empty: the empty grid says where
/// it looks and how many it found, and the foot's bar is a stripe that moves (frozen at one
/// instant here) or, under reduce-motion, a still soft track. Also: a click on a location's
/// X removes it (the row's hit, pushed after it, once took the click).
#[test]
fn library_walking_matches_goldens() {
    use super::library::Side;
    use specular_library::{Location, Phase, Progress};
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let pal = Palette::dark();
    for (look, reduce_motion) in [("moving", false), ("still", true)] {
        let mut g = Gallery {
            active: true,
            ..Gallery::default()
        };
        let lv = &mut g.library;
        lv.on = true;
        lv.panel = true;
        lv.searched = true;
        lv.was_busy = true;
        lv.locations = vec![Location {
            id: 1,
            path: PathBuf::from("/shots/Pictures"),
            offline: false,
            files: 0,
            pending: 0,
            last_scan: None,
        }];
        lv.progress = Progress {
            phase: Phase::Walking,
            location: Some(PathBuf::from("/shots/Pictures")),
            walked: 1_234,
            ..Progress::default()
        };
        g.rebuild();
        let f = Frame {
            pal: &pal,
            now: 1.0,
            reduce_motion,
            size: SIZE,
            scale: 1.0,
            rtl: false,
            text_scale: 1.0,
            backdrop: Backdrop::None,
        };
        let shot = h.render(
            SIZE,
            1.0,
            |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
            |dl, text| {
                g.draw(dl, text, &f);
            },
        );
        goldens.check(&format!("library-walking-{look}-dark-1x"), &shot);
        // The X shows on the focused (or hovered) location.
        g.library.focus = Some(1);
        h.render(
            SIZE,
            1.0,
            |_| {},
            |dl, text| {
                g.draw(dl, text, &f);
            },
        );
        let x = g
            .hits
            .iter()
            .find(|h| h.1 == Hit::Library(Side::Remove(1)))
            .map(|h| h.0)
            .expect("the location has its X");
        let c = (x.x + x.w * 0.5, x.y + x.h * 0.5);
        assert_eq!(g.hit_at(c), Some(Hit::Library(Side::Remove(1))));
        let row = g
            .hits
            .iter()
            .find(|h| h.1 == Hit::Library(Side::Location(1)))
            .map(|h| h.0)
            .expect("the location's row");
        assert_eq!(
            g.hit_at((row.x + 4.0, row.y + row.h * 0.5)),
            Some(Hit::Library(Side::Location(1)))
        );
    }
    goldens.finish();
}

/// The Library with albums: a group with a manual album (shown, and where adds go), a
/// smart album and a nested group, and the date timeline over the grid with June 2025
/// picked. Dark at 1×, light at 1.5× (with the keyboard on the timeline).
#[test]
fn library_albums_match_goldens() {
    use specular_library::Location;
    use specular_library::albums::{Album, Kind};
    use specular_query::date::days_from_civil;
    use specular_query::rules::Rules;
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let root = std::env::temp_dir().join(format!("specular-albums-shot-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut cfg = specular_library::Config::new(root.join("library.sqlite3"));
    cfg.watch = false;
    let lib = Arc::new(specular_library::Library::open(cfg).unwrap());
    let album = |id: i64, name: &str, kind: Kind, files: u64| Album {
        id,
        name: name.into(),
        kind,
        rules: Rules::one("rating:>=4"),
        files,
    };
    // Two years of shooting, busiest in summer.
    let mut days = Vec::new();
    for m in 1..=12u32 {
        for (y, n) in [(2024, 3u64), (2025, 5u64)] {
            let busy = if (5..=8).contains(&m) { 4 } else { 1 };
            days.push((days_from_civil(y, m, 10), n * busy * u64::from(m % 3 + 1)));
        }
    }
    for (name, dark, scale) in [
        ("library-albums-dark-1x", true, 1.0),
        ("library-albums-light-1.5x", false, 1.5),
    ] {
        let pal = if dark {
            Palette::dark()
        } else {
            Palette::light()
        };
        let mut g = gallery();
        g.filter.text = "date:2025-06".into();
        g.filter_open = false;
        let lv = &mut g.library;
        lv.on = true;
        lv.panel = true;
        lv.searched = true;
        lv.lib = Some(lib.clone());
        lv.locations = vec![Location {
            id: 1,
            path: PathBuf::from("/shots/Pictures"),
            offline: false,
            files: 18_204,
            pending: 0,
            last_scan: None,
        }];
        lv.albums.list = vec![
            album(1, "Client X/all", Kind::Manual, 412),
            album(2, "Client X/selects", Kind::Manual, 38),
            album(3, "Client X/web/finals", Kind::Smart, 0),
            album(4, "Five stars", Kind::Smart, 0),
        ];
        lv.albums.loaded = true;
        lv.albums.open = Some(2);
        lv.albums.target = Some(2);
        lv.albums.days = days.clone();
        g.rebuild();
        let f = Frame {
            pal: &pal,
            now: 1.0,
            reduce_motion: true,
            size: SIZE,
            scale,
            rtl: false,
            text_scale: 1.0,
            backdrop: Backdrop::None,
        };
        for _ in 0..20 {
            let mut warm = DrawList::new(scale);
            h.text.begin_frame(scale);
            g.draw(&mut warm, &mut h.text, &f);
            if !g.cap_more {
                break;
            }
        }
        // In light, the keyboard is on the timeline (its focus ring on June).
        if !dark {
            g.go(filterbar::Stop::Timeline);
        }
        let shot = h.render(
            SIZE,
            scale,
            |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
            |dl, text| {
                g.draw(dl, text, &f);
            },
        );
        goldens.check(name, &shot);
        // The timeline's bars are hit areas; June 2025 is picked.
        assert!(g.library.albums.bars.len() > 12);
        let (lo, hi) = specular_query::timeline::selected(&g.filter.text).unwrap();
        assert_eq!(
            g.library
                .albums
                .bars
                .iter()
                .filter(|(b, _)| b.within(lo, hi))
                .count(),
            1
        );
        if dark {
            timeline_keys(&mut g);
        }
    }
    drop(lib);
    let _ = std::fs::remove_dir_all(&root);
    goldens.finish();
}

/// The timeline from the keyboard: Tab stop, arrows between bars, Enter picks, Shift+arrows
/// extend, Esc back to the grid.
fn timeline_keys(g: &mut Gallery) {
    use filterbar::Stop;
    use specular_input::NamedKey as N;
    let picked = |g: &Gallery| {
        let Some((lo, hi)) = specular_query::timeline::selected(&g.filter.text) else {
            return 0;
        };
        g.library
            .albums
            .bars
            .iter()
            .filter(|(b, _)| b.within(lo, hi))
            .count()
    };
    // Tab from the grid (the bar closed) lands on the period picked.
    assert_eq!(g.stop(), Stop::Grid);
    g.go(Stop::Timeline);
    assert_eq!(g.stop(), Stop::Timeline);
    let june = g.library.albums.kb.unwrap();
    let text = g.filter.text.clone();
    // An arrow moves the focus only.
    assert!(g.timeline_key(N::ArrowRight, false));
    assert_eq!(g.library.albums.kb, Some(june + 1));
    assert_eq!(g.filter.text, text);
    // Enter picks July.
    assert!(g.timeline_key(N::Enter, false));
    assert_ne!(g.filter.text, text);
    assert_eq!(picked(g), 1);
    // Shift+→ extends to August.
    assert!(g.timeline_key(N::ArrowRight, true));
    assert_eq!(picked(g), 2);
    // Esc gives the keyboard back to the grid.
    assert!(g.timeline_key(N::Escape, false));
    assert_eq!(g.stop(), Stop::Grid);
    assert_eq!(g.library.albums.kb, None);
    // Hidden by the setting: no strip, no stop.
    g.library_show_timeline(false);
    assert_eq!(g.timeline_h(), 0.0);
    g.go(Stop::Timeline);
    assert_eq!(g.stop(), Stop::Grid);
    g.library_show_timeline(true);
}
