#[cfg(unix)]
use std::sync::atomic::AtomicBool;

use specular_astro::hips as ah;
use specular_fits::Header;
use specular_shots::{Goldens, Harness};

use super::*;

/// A fresh folder under the target folder.
#[cfg_attr(not(unix), allow(dead_code))] // only the Unix tests use it
fn scratch(name: &str) -> PathBuf {
    let d = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("astro-app-tests")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    // A relative CARGO_TARGET_DIR would break the lock-file URL and the solver path.
    std::path::absolute(d).unwrap()
}

/// A rotated TAN WCS of a `w × h` image at 1.5″ per pixel.
fn wcs(w: usize, h: usize) -> Wcs {
    let r = 20f64.to_radians();
    let s = 1.5 / 3600.0;
    let cards = [
        ("CTYPE1", "'RA---TAN'".to_owned()),
        ("CTYPE2", "'DEC--TAN'".to_owned()),
        ("CRVAL1", "202.4696".to_owned()),
        ("CRVAL2", "47.1952".to_owned()),
        ("CRPIX1", format!("{}", w as f64 / 2.0 + 0.5)),
        ("CRPIX2", format!("{}", h as f64 / 2.0 + 0.5)),
        ("CD1_1", format!("{:E}", -s * r.cos())),
        ("CD1_2", format!("{:E}", s * r.sin())),
        ("CD2_1", format!("{:E}", s * r.sin())),
        ("CD2_2", format!("{:E}", s * r.cos())),
    ];
    let h2 = Header::from_pairs(cards.iter().map(|(k, v)| (*k, v.as_str(), "")));
    Wcs::from_header(&h2, &[w, h]).unwrap()
}

#[test]
fn regions_become_measurements_and_back() {
    let placed = [
        Placed::Ellipse {
            c: (10.0, 20.0),
            r: (5.0, 3.0),
            angle: 90.0,
        },
        Placed::Ellipse {
            c: (10.0, 20.0),
            r: (5.0, 3.0),
            angle: 33.0,
        },
        Placed::Rect {
            c: (50.0, 40.0),
            size: (10.0, 4.0),
            angle: 0.0,
        },
        Placed::Rect {
            c: (50.0, 40.0),
            size: (10.0, 4.0),
            angle: 45.0,
        },
        Placed::Polygon(vec![(0.0, 0.0), (4.0, 0.0), (2.0, 3.0)]),
        Placed::Line {
            a: (1.0, 2.0),
            b: (3.0, 4.0),
            arrows: (false, true),
        },
        Placed::Text { c: (7.0, 8.0) },
    ];
    let shapes: Vec<Shape> = placed.iter().map(measure_shape).collect();
    assert_eq!(
        shapes[0],
        Shape::Ellipse {
            cx: 10.0,
            cy: 20.0,
            rx: 3.0,
            ry: 5.0
        }
    );
    assert!(matches!(&shapes[1], Shape::Polygon(v) if v.len() == 72));
    assert_eq!(
        shapes[2],
        Shape::Rect {
            x0: 45.0,
            y0: 38.0,
            x1: 55.0,
            y1: 42.0
        }
    );
    assert!(matches!(&shapes[3], Shape::Polygon(v) if v.len() == 4));
    assert_eq!(shapes[6], Shape::Point(P::new(7.0, 8.0)));
    // Back to regions: what maps comes back as it was placed.
    let back = drawn_of(&shapes[2], "box");
    assert_eq!(back[0].shape, placed[2]);
    assert_eq!(back[0].text, "box");
    let back = drawn_of(&shapes[4], "");
    assert_eq!(back[0].shape, placed[4]);
    let back = drawn_of(
        &Shape::Polyline(vec![P::new(0.0, 0.0), P::new(1.0, 0.0), P::new(1.0, 1.0)]),
        "",
    );
    assert_eq!(back.len(), 2);
}

#[test]
fn regions_are_written_in_the_image_frame() {
    let w = wcs(200, 150);
    let rest: [usize; 0] = [];
    let t = Target {
        height: 150.0,
        wcs: Some((&w, &rest)),
        physical: None,
    };
    let shapes = drawn_of(
        &Shape::Ellipse {
            cx: 100.0,
            cy: 75.0,
            rx: 10.0,
            ry: 10.0,
        },
        "M51",
    );
    let (text, skipped) = regions::write(&shapes, &t);
    assert_eq!(skipped, 0);
    assert!(
        text.contains("\nicrs\ncircle(202.4696,47.1952,15\") # text={M51}\n"),
        "{text}"
    );
}

/// A stand-in solver: `MODE` is `astap` (writes `-o BASE` .ini and .wcs), `anet` (writes
/// `--dir D --out B` .wcs and .solved), `fail` (ASTAP saying no) or `hang`.
#[cfg(unix)]
fn fake_solver(dir: &Path, mode: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let p = dir.join(format!("fake-{mode}"));
    let cards = "CTYPE1  = 'RA---TAN'\nCTYPE2  = 'DEC--TAN'\nCRVAL1  = 202.4696\n\
                 CRVAL2  = 47.1952\nCRPIX1  = 80.5\nCRPIX2  = 60.5\nCD1_1   = -4.1666667E-4\n\
                 CD1_2   = 0.0\nCD2_1   = 0.0\nCD2_2   = 4.1666667E-4\nEND\n";
    let script = match mode {
        "astap" => format!(
            "#!/bin/sh\nwhile [ $# -gt 0 ]; do [ \"$1\" = -o ] && base=\"$2\"; shift; done\n\
             printf 'PLTSOLVD=T\\n' > \"$base.ini\"\nprintf '{}' > \"$base.wcs\"\n",
            cards.replace('\n', "\\n")
        ),
        "anet" => format!(
            "#!/bin/sh\nwhile [ $# -gt 0 ]; do case \"$1\" in --dir) d=\"$2\";; --out) b=\"$2\";; \
             esac; shift; done\nprintf '{}' > \"$d/$b.wcs\"\n: > \"$d/$b.solved\"\n",
            cards.replace('\n', "\\n")
        ),
        "fail" => "#!/bin/sh\nwhile [ $# -gt 0 ]; do [ \"$1\" = -o ] && base=\"$2\"; shift; done\n\
                   printf 'PLTSOLVD=F\\nERROR=No stars detected\\n' > \"$base.ini\"\nexit 1\n"
            .to_owned(),
        _ => "#!/bin/sh\nsleep 30\n".to_owned(),
    };
    std::fs::write(&p, script).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

#[cfg(unix)]
#[test]
fn plate_solving_with_stand_in_solvers() {
    let d = scratch("solve");
    let image = d.join("m51.fits");
    std::fs::write(&image, b"SIMPLE  =                    T").unwrap();
    let cancel = AtomicBool::new(false);
    for (mode, solver) in [("astap", Solver::Astap), ("anet", Solver::AstrometryNet)] {
        let out = d.join(format!("out-{mode}"));
        std::fs::create_dir_all(&out).unwrap();
        let prog = fake_solver(&d, mode);
        let h = run_solver(
            &prog,
            solver,
            &image,
            &out,
            &Hint::default(),
            &cancel,
            &|| {},
        )
        .unwrap();
        let w = solve::solution_wcs(&h, 160, 120).unwrap();
        let (ra, dec) = w.sky_at(80.0, 60.0, &[]).unwrap();
        assert!(
            (ra - 202.4696).abs() < 1e-6 && (dec - 47.1952).abs() < 1e-6,
            "{mode}"
        );
        assert!((w.arcsec_per_pixel().unwrap() - 1.5).abs() < 1e-3);
    }
    // ASTAP found nothing: its reason.
    let out = d.join("out-fail");
    std::fs::create_dir_all(&out).unwrap();
    let e = run_solver(
        &fake_solver(&d, "fail"),
        Solver::Astap,
        &image,
        &out,
        &Hint::default(),
        &cancel,
        &|| {},
    )
    .unwrap_err();
    assert_eq!(e, "no match was found (No stars detected)");
    // Stopped while running.
    let out = d.join("out-hang");
    std::fs::create_dir_all(&out).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let s2 = stop.clone();
    #[allow(clippy::disallowed_methods)]
    let t = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        s2.store(true, Ordering::Relaxed);
    });
    let started = Instant::now();
    let e = run_solver(
        &fake_solver(&d, "hang"),
        Solver::Astap,
        &image,
        &out,
        &Hint::default(),
        &stop,
        &|| {},
    )
    .unwrap_err();
    t.join().unwrap();
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(e, t!("astro-solve-stopped"));
    // A program picked in settings wins over the search.
    let picked = fake_solver(&d, "anet");
    assert_eq!(
        find_solver(None, Some(&picked)),
        Some((Solver::AstrometryNet, picked.clone()))
    );
    // The sidecar a save writes reads back to the same WCS.
    let h = solve::read_solution(Solver::AstrometryNet, &d.join("out-anet")).unwrap();
    let side = d.join("m51.wcs");
    std::fs::write(&side, solve::sidecar(&h)).unwrap();
    assert_eq!(sidecar_path(&image), side);
    let back = solve::header_file(&std::fs::read(&side).unwrap()).unwrap();
    assert_eq!(
        solve::solution_wcs(&back, 160, 120).unwrap(),
        solve::solution_wcs(&h, 160, 120).unwrap()
    );
}

/// The HiPS oracle's view painted from tiles served over HTTP by the in-process test
/// server, fetched and decoded the app's way.
#[test]
fn survey_from_a_local_server_matches_the_oracle() {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/specular-astro/tests/data/hips"
    );
    let text = std::fs::read_to_string(format!("{dir}/view.hdr")).unwrap();
    let pairs: Vec<(String, String)> = text
        .lines()
        .filter_map(|l| l.split_once(" = "))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    let h = Header::from_pairs(pairs.iter().map(|(k, v)| (k.as_str(), v.as_str(), "")));
    let w = Wcs::from_header(&h, &[48, 48]).unwrap();
    let (rgba, fetched) = hips::testing::paint_from_server(&w, 48, 48);
    assert!(fetched > 0);
    let oracle = std::fs::read_to_string(format!("{dir}/view.tsv")).unwrap();
    let mut bad = 0;
    for l in oracle.lines() {
        let v: Vec<usize> = l.split('\t').map(|s| s.parse().unwrap()).collect();
        let i = (v[1] * 48 + v[0]) * 4;
        if rgba[i..i + 3] != [v[2] as u8, v[3] as u8, v[4] as u8] {
            bad += 1;
        }
    }
    eprintln!("survey via HTTP: {bad} of 2304 pixels differ from the oracle, {fetched} tiles");
    assert!(bad <= 2304 / 50, "{bad}");
    assert_eq!(ah::SURVEYS.len(), 4);
}

/// A made-up survey tile: a dim sky with a few stars, by tile and pixel.
fn sky_tile(n: u64, width: u32) -> Vec<u8> {
    let w = width as usize;
    let mut t = vec![0u8; w * w * 4];
    for r in 0..w {
        for c in 0..w {
            let hsh = (n.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ ((r * 977 + c * 131) as u64))
                .wrapping_mul(0xbf58_476d_1ce4_e5b9);
            let star = hsh.is_multiple_of(97);
            let base = 18 + (hsh >> 60) as u8;
            let px = if star {
                [230, 225, 200, 255]
            } else {
                [base, base + 4, base + 14, 255]
            };
            t[(r * w + c) * 4..(r * w + c) * 4 + 4].copy_from_slice(&px);
        }
    }
    t
}

fn shot(h: &mut Harness, goldens: &mut Goldens, name: &str, rtl: bool) {
    const SIZE: (f32, f32) = (480.0, 300.0);
    let (iw, ih) = (160usize, 120usize);
    let w = wcs(iw, ih);
    // The image: a galaxy and stars, grey, drawn as a texture in the middle.
    let mut img = vec![0u8; iw * ih * 4];
    for y in 0..ih {
        for x in 0..iw {
            let (dx, dy) = (x as f32 - 80.0, y as f32 - 60.0);
            let g = 40.0 + 170.0 * (-(dx * dx / 900.0 + dy * dy / 400.0)).exp();
            let s = [(30.0, 25.0), (130.0, 90.0), (110.0, 20.0)]
                .iter()
                .map(|(sx, sy)| {
                    255.0 * (-((x as f32 - sx).powi(2) + (y as f32 - sy).powi(2)) / 4.0).exp()
                })
                .sum::<f32>();
            let v = (g + s).min(255.0) as u8;
            img[(y * iw + x) * 4..(y * iw + x) * 4 + 4].copy_from_slice(&[v, v, v, 255]);
        }
    }
    // The view: the image 1.5× in the middle; stored px = (screen − origin) / 1.5.
    let (k, ox, oy) = (
        1.5f32,
        (SIZE.0 - iw as f32 * 1.5) / 2.0,
        (SIZE.1 - ih as f32 * 1.5) / 2.0,
    );
    let to_screen = |p: (f64, f64)| (ox + p.0 as f32 * k, oy + p.1 as f32 * k);
    // The survey around it, painted the app's way at the screen's resolution.
    let props =
        ah::parse_properties("hips_order = 9\nhips_tile_width = 64\nhips_tile_format = png\n")
            .unwrap();
    let (sw, sh) = (SIZE.0 as usize, SIZE.1 as usize);
    let deg = 1.5 / 3600.0 / f64::from(k);
    let order = props.order_for(deg);
    let mut tiles = HashMap::new();
    let sky = |x: usize, y: usize| {
        let s = ((x as f32 + 0.5 - ox) / k, (y as f32 + 0.5 - oy) / k);
        if s.0 >= 0.0 && s.1 >= 0.0 && s.0 < iw as f32 && s.1 < ih as f32 {
            return None;
        }
        let (a, b) = w.sky_at(f64::from(s.0), f64::from(s.1), &[])?;
        Some((a, b, specular_astro::frames::Sky::Icrs))
    };
    for y in 0..sh {
        for x in 0..sw {
            if let Some((a, b, _)) = sky(x, y) {
                let n = ah::locate(order, 64, a, b).0;
                tiles.entry((order, n)).or_insert_with(|| sky_tile(n, 64));
            }
        }
    }
    let mut survey = vec![0u8; sw * sh * 4];
    let missing = ah::paint(
        &mut survey,
        sw,
        sh,
        specular_astro::frames::Sky::Icrs,
        &props,
        order,
        sky,
        |kk, n| tiles.get(&(kk, n)).map(Vec::as_slice),
    );
    assert!(missing.is_empty());
    let (_t1, img_tex) =
        crate::app::studies::upload_rgba(&h.gpu, &mut h.ui, iw as u32, ih as u32, &img);
    let (_t2, sky_tex) =
        crate::app::studies::upload_rgba(&h.gpu, &mut h.ui, sw as u32, sh as u32, &survey);
    // Regions in sky and image coordinates.
    let parsed = regions::parse(concat!(
        "# Region file format: DS9 version 4.1\n",
        "fk5\n",
        "circle(202.4696,47.1952,30\") # color=yellow text={M51}\n",
        "ellipse(202.4790,47.2010,40\",15\",30) # color=cyan dash=1\n",
        "-circle(202.4600,47.1900,12\") # color=red\n",
        "point(202.4640,47.2040) # point=cross color=magenta text={SN}\n",
        "image\n",
        "box(130,90,30,20,0) # color=green width=2\n",
        "polygon(20,100,50,110,35,80) # color=orange\n",
        "line(10,10,60,30) # line=0 1 color=white\n",
    ));
    assert!(parsed.refused.is_empty());
    let rest: [usize; 0] = [];
    let t = Target {
        height: ih as f64,
        wcs: Some((&w, &rest)),
        physical: None,
    };
    let screen: Vec<ScreenRegion> = parsed
        .regions
        .iter()
        .flat_map(|r| {
            r.place(&t)
                .into_iter()
                .map(|p| on_screen(&p, r, &to_screen))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(screen.len(), 7);
    let credit = t!(
        "astro-survey-credit",
        name = ah::SURVEYS[0].name,
        credit = ah::SURVEYS[0].credit
    );
    h.data_text(ah::SURVEYS[0].name);
    h.data_text(ah::SURVEYS[0].credit);
    let o = AstroOverlay {
        survey: Some((sky_tex, Rect::new(0.0, 0.0, SIZE.0, SIZE.1))),
        credit: Some(credit),
        regions: screen,
        highlights: vec![to_screen((110.0, 20.0))],
    };
    let shot = h.render(
        SIZE,
        1.0,
        |dl| {
            dl.rect(
                Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
                0.0,
                Color::rgba(0.08, 0.08, 0.09, 1.0),
            )
        },
        |dl, text| {
            let area = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
            draw_survey(dl, text, &o, area, 1.0, rtl);
            dl.image(
                Rect::new(ox, oy, iw as f32 * k, ih as f32 * k),
                [0.0, 0.0, 1.0, 1.0],
                img_tex,
                0.0,
            );
            draw_regions(dl, text, &o.regions, area, 1.0);
            draw_highlights(dl, &o.highlights, area);
        },
    );
    goldens.check(name, &shot);
}

/// Regions, a highlighted source and a sky survey around a sky image, with the survey's
/// credit; once right to left.
#[test]
fn astronomy_overlays_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-astro",
    );
    shot(&mut h, &mut goldens, "astro-regions-survey-dark-1x", false);
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        shot(&mut h, &mut goldens, "astro-regions-survey-he-1x", true);
    });
    goldens.finish();
}
