use super::*;

/// The map view: Natural Earth's land under clusters and pins of the photos' positions, the
/// pointer over the largest cluster (its card shows the first photo's placeholder), in dark
/// and light at 1× and 1.5×, in Hebrew (the header and credit mirror, the map does not) and
/// with large text.
#[test]
fn map_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let (dark, light) = (Palette::dark(), Palette::light());
    let looks = [
        ("map-dark-1x", &dark, false, 1.0, 1.0),
        ("map-dark-1.5x", &dark, false, 1.0, 1.5),
        ("map-light-1x", &light, false, 1.0, 1.0),
        ("map-light-1.5x", &light, false, 1.0, 1.5),
        ("map-he-light-1x", &light, true, 1.0, 1.0),
        ("map-dark-text2-1x", &dark, false, 1.5, 1.0),
    ];
    for (name, pal, rtl, text_scale, scale) in looks {
        let lang = if rtl {
            specular_i18n::langid!("he")
        } else {
            specular_i18n::langid!("en-US")
        };
        specular_i18n::I18n::with_thread_language(lang, || {
            let mut g = map_gallery();
            let f = Frame {
                pal,
                now: 1.0,
                reduce_motion: true,
                size: SIZE,
                scale,
                rtl,
                text_scale,
                backdrop: Backdrop::None,
            };
            map_frame(&mut h, &mut g, &f);
            assert_eq!(g.map.photos(), 20, "every placed photo is on the map");
            let big = g
                .map
                .markers
                .iter()
                .max_by_key(|m| m.count)
                .copied()
                .expect("markers");
            assert!(big.count >= 8, "Lisbon clusters: {big:?}");
            g.pointer = Some(big.at);
            map_frame(&mut h, &mut g, &f);
            assert!(g.map.tex.is_some());
            let shot = h.render(
                SIZE,
                scale,
                |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
                |dl, text| {
                    g.draw(dl, text, &f);
                },
            );
            goldens.check(name, &shot);
            crate::app::a11y_audit::audit_screen(name, SIZE, f.rtl, |t| g.a11y(t, 1.0));
            g.map.open = false;
            g.prepare_map(&h.gpu, &mut h.ui, scale, SIZE);
            assert!(g.map.tex.is_none(), "the texture goes with the map");
        });
    }
    goldens.finish();
}

/// The online base map's one-time explanation over the map, before any tile is fetched:
/// dark at 1× and Hebrew light at 1×. Its two buttons are hit targets and screen-reader
/// buttons.
#[test]
fn basemap_consent_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let (dark, light) = (Palette::dark(), Palette::light());
    for (name, pal, rtl) in [
        ("map-basemap-consent-dark-1x", &dark, false),
        ("map-basemap-consent-he-light-1x", &light, true),
    ] {
        let lang = if rtl {
            specular_i18n::langid!("he")
        } else {
            specular_i18n::langid!("en-US")
        };
        specular_i18n::I18n::with_thread_language(lang, || {
            let mut g = map_gallery();
            g.map.basemap.ask_for_test(256);
            let f = Frame {
                pal,
                now: 1.0,
                reduce_motion: true,
                size: SIZE,
                scale: 1.0,
                rtl,
                text_scale: 1.0,
                backdrop: Backdrop::None,
            };
            map_frame(&mut h, &mut g, &f);
            map_frame(&mut h, &mut g, &f);
            let shot = h.render(
                SIZE,
                1.0,
                |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
                |dl, text| {
                    g.draw(dl, text, &f);
                },
            );
            assert!(
                g.map
                    .hits
                    .iter()
                    .any(|h| h.1 == super::map::MAct::BasemapAllow),
                "the agreement is a button"
            );
            assert!(!g.map.basemap.active(), "nothing fetched before the answer");
            goldens.check(name, &shot);
            crate::app::a11y_audit::audit_screen(name, SIZE, f.rtl, |t| g.a11y(t, 1.0));
            g.map.open = false;
            g.prepare_map(&h.gpu, &mut h.ui, 1.0, SIZE);
        });
    }
    goldens.finish();
}

/// The base map from the user's own MBTiles file: its tiles come in from the fetch thread,
/// are uploaded and drawn over the offline map, with the file's credit (dark at 1×).
#[test]
fn basemap_file_matches_goldens() {
    use specular_decode::geo::specular_gis::{fixtures, tiles::TileFormat};
    let Some(mut h) = Harness::new() else { return };
    // MBTiles is SQLite, read in the decode host only.
    crate::test_host::ensure();
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let dir = std::env::temp_dir().join(format!("specular-basemap-shot-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("world.mbtiles");
    fixtures::mbtiles(&path, TileFormat::Png, 3);
    let (loader, events) = crate::loader::Loader::new(1);
    let pal = Palette::dark();
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
    let mut g = map_gallery();
    g.map.basemap.configure(
        super::basemap::Config {
            choice: specular_settings::Basemap::File,
            url: String::new(),
            path: Some(path.clone()),
            service: String::new(),
            cache_mb: 256,
            allowed: Vec::new(),
            dark: true,
        },
        &loader,
    );
    assert!(g.map.basemap.active() && g.map.basemap.consent.is_none());
    for _ in 0..50 {
        map_frame(&mut h, &mut g, &f);
        while let Ok(e) = events.recv_timeout(std::time::Duration::from_millis(50)) {
            if let crate::loader::Event::BasemapTile {
                generation,
                key,
                tile,
            } = e
            {
                g.map.basemap.tile_ready(generation, key, tile);
            }
        }
        map_frame(&mut h, &mut g, &f);
        if g.map.basemap.tiles_on_gpu() >= 4 {
            break;
        }
    }
    assert!(g.map.basemap.tiles_on_gpu() >= 4, "tiles drawn");
    assert_eq!(
        g.map.basemap.credit().as_deref(),
        Some("© Test contributors")
    );
    let shot = h.render(
        SIZE,
        1.0,
        |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
        |dl, text| {
            g.draw(dl, text, &f);
        },
    );
    goldens.check("map-basemap-file-dark-1x", &shot);
    crate::app::a11y_audit::audit_screen("map-basemap-file-dark-1x", SIZE, f.rtl, |t| {
        g.a11y(t, 1.0)
    });
    g.map.open = false;
    g.prepare_map(&h.gpu, &mut h.ui, 1.0, SIZE);
    assert_eq!(g.map.basemap.tiles_on_gpu(), 0, "the tiles go with the map");
    goldens.finish();
    let _ = std::fs::remove_dir_all(&dir);
}

/// Frame time of the map with 50 000 photos (run by hand:
/// `cargo test -p specular --bin specular --release map_frame_time -- --ignored --nocapture`):
/// clustering them, a frame's markers at a few zooms, and the base map on the GPU.
#[test]
#[ignore = "a measurement"]
fn map_frame_time_50k() {
    let Some(mut h) = Harness::new() else { return };
    const N: usize = 50_000;
    let mut g = Gallery {
        active: true,
        ..Gallery::default()
    };
    let entries: Vec<Entry> = (0..N)
        .map(|i| {
            let name = format!("p{i:06}.jpg");
            Entry {
                path: PathBuf::from(format!("/bench/{name}")),
                name,
                kind: EntryKind::Image,
                len: 1,
                modified: None,
                hidden: false,
                taken: None,
                dims: Some((4000, 3000)),
            }
        })
        .collect();
    g.add_entries(entries);
    g.resort();
    g.rebuild();
    g.atlas = Some(Atlas::new(4));
    // Photos round 40 cities, spread up to ~50 km (xorshift for the spread).
    let mut x = 0x9e37_79b9_7f4a_7c15u64;
    let mut rnd = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x >> 11) as f64 / (1u64 << 53) as f64
    };
    let paths: Vec<PathBuf> = g.entries.iter().map(|e| e.path.clone()).collect();
    for p in paths {
        let c = (rnd() * 40.0) as usize;
        let (clat, clon) = (-40.0 + c as f64 * 2.1, -170.0 + c as f64 * 8.5);
        let meta = Meta {
            position: Some(((clat + rnd() - 0.5) as f32, (clon + rnd() - 0.5) as f32)),
            ..Meta::default()
        };
        g.metas.insert(p.into(), Arc::new(meta));
    }
    // Every position known (as once the metadata pass ends): fitted once, then left alone.
    g.meta_complete = Some((g.generation, N, N, 0));
    g.map.open = true;
    let pal = Palette::dark();
    let size = (1600.0, 1000.0);
    let f = Frame {
        pal: &pal,
        now: 1.0,
        reduce_motion: true,
        size,
        scale: 1.0,
        rtl: false,
        text_scale: 1.0,
        backdrop: Backdrop::None,
    };
    let t = Instant::now();
    g.map_refresh(true);
    let cluster = t.elapsed();
    map_frame(&mut h, &mut g, &f);
    let mut report = format!(
        "{N} photos: clustered in {:.1} ms",
        cluster.as_secs_f64() * 1e3
    );
    for zoom in [2.0, 5.0, 8.0, 9.0, 10.0, 11.0, 12.0, 18.0] {
        g.map.cam.zoom = zoom;
        g.map.cam.center = specular_map::project(-40.0 + 20.0 * 2.1, -170.0 + 20.0 * 8.5);
        // Each level's clusters are built on first use: warm it, then time frames.
        let mut dl = DrawList::new(1.0);
        g.draw(&mut dl, &mut h.text, &f);
        let n = 20;
        let t = Instant::now();
        for _ in 0..n {
            let mut dl = DrawList::new(1.0);
            h.text.begin_frame(1.0);
            g.draw(&mut dl, &mut h.text, &f);
        }
        let cpu = t.elapsed().as_secs_f64() * 1e3 / f64::from(n);
        // The whole frame through the UI renderer, read back (an upper bound: the readback).
        let t = Instant::now();
        let _ = h.render(
            size,
            1.0,
            |dl| dl.rect(Rect::new(0.0, 0.0, size.0, size.1), 0.0, pal.canvas),
            |dl, text| {
                g.draw(dl, text, &f);
            },
        );
        let ui = t.elapsed().as_secs_f64() * 1e3;
        // The base map on the GPU (moved a hair so it re-renders), waited for.
        let t = Instant::now();
        for i in 0..n {
            g.map.cam.center.0 += 1e-9 * f64::from(i + 1);
            g.prepare_map(&h.gpu, &mut h.ui, 1.0, f.size);
            let mut enc = h.gpu.device.create_command_encoder(&Default::default());
            g.render_map(&h.gpu, &mut enc);
            h.gpu.queue.submit([enc.finish()]);
            h.gpu
                .device
                .poll(specular_gfx::wgpu::PollType::wait_indefinitely())
                .ok();
        }
        let gpu = t.elapsed().as_secs_f64() * 1e3 / f64::from(n);
        report += &format!(
            "\nzoom {zoom}: {} markers, gallery+map draw list {cpu:.2} ms, \
             UI render+readback {ui:.1} ms, base map {gpu:.2} ms ({})",
            g.map.markers.len(),
            h.gpu.info.name
        );
    }
    eprintln!("{report}");
}
