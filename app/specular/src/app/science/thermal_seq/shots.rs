//! Screenshot tests of a thermal sequence's panel: playback, where it is, and two
//! measurements' temperatures over time with the cursor at the frame shown; dark left to
//! right and light right to left (Hebrew), at 1× and 1.5×; and the panel before any
//! measurement (the hint). Rendered offscreen on lavapipe and compared with the PNGs in
//! `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular thermal_seq::shots`, then review
//! and commit the rewritten PNGs.

use super::*;
use specular_shots::{Goldens, Harness};

const SIZE: (f32, f32) = (900.0, 600.0);
const SCALES: [(f32, &str); 2] = [(1.0, "1x"), (1.5, "1.5x")];

/// An iron-palette blob standing for the frame shown.
fn backdrop(dl: &mut DrawList) {
    dl.rect(
        Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
        0.0,
        Color::rgba(0.08, 0.02, 0.18, 1.0),
    );
    for (k, c) in [
        (0.45, Color::rgba(0.45, 0.02, 0.55, 1.0)),
        (0.3, Color::rgba(0.9, 0.25, 0.08, 1.0)),
        (0.15, Color::rgba(1.0, 0.8, 0.2, 1.0)),
    ] {
        let r = SIZE.1 * k;
        dl.rect(
            Rect::new(SIZE.0 * 0.4 - r, SIZE.1 * 0.45 - r, 2.0 * r, 2.0 * r),
            r,
            c,
        );
    }
}

/// A 6-second sequence at 25 frames a second: a spot warming up and an area cooling.
fn series() -> Series {
    let n = 150;
    let times: Vec<f64> = (0..n).map(|k| f64::from(k) / 25.0).collect();
    let spot: Vec<f64> = times
        .iter()
        .map(|t| 36.0 + 6.0 * (1.0 - (-t / 2.0).exp()) + 0.3 * (t * 3.0).sin())
        .collect();
    let area: Vec<f64> = times.iter().map(|t| 31.0 - 1.5 * t / 6.0).collect();
    Series {
        times,
        unit: TemperatureUnit::Celsius,
        lines: vec![
            Line {
                id: 1,
                name: t!("measure-tool-point"),
                mean: spot.clone(),
                min: spot.clone(),
                max: spot,
            },
            Line {
                id: 2,
                name: t!("measure-tool-rect"),
                min: area.iter().map(|v| v - 2.0).collect(),
                max: area.iter().map(|v| v + 3.0).collect(),
                mean: area,
            },
        ],
    }
}

fn panel(playing: bool, with_series: bool) -> Panel {
    Panel {
        playing,
        position: t!(
            "thermal-seq-frame",
            frame = 61,
            frames = 150,
            time = "2.4 s".to_owned()
        ),
        at: 2.4,
        series: with_series.then(|| Arc::new(series())),
        note: (!with_series).then(|| t!("thermal-seq-hint")),
    }
}

#[test]
fn thermal_sequence_panel_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-thermal-seq",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    for (scale, suffix) in SCALES {
        for (name, playing, with_series, light, rtl) in [
            ("plot", true, true, false, false),
            ("plot-he-rtl-light", false, true, true, true),
            ("hint", false, false, false, false),
        ] {
            if name == "hint" && suffix != "1x" {
                continue;
            }
            let lang = if rtl { &he } else { &en };
            let pal = if light {
                Palette::light()
            } else {
                Palette::dark()
            };
            let (shot, tree) = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let p = panel(playing, with_series);
                let mut drawn = Drawn::default();
                let shot = h.render(SIZE, scale, backdrop, |dl, text| {
                    let area = (0.0, 0.0, SIZE.0, SIZE.1);
                    drawn = draw(dl, text, &pal, &p, area, None, rtl, 1.0);
                });
                let mut tree = super::super::super::a11y_audit::window(SIZE, 1.0);
                a11y(&mut tree, &p, &drawn, 60, 1.0);
                (shot, tree)
            });
            let n = format!("thermal-seq-{name}-{suffix}");
            goldens.check(&n, &shot);
            super::super::super::a11y_audit::expect_clean(&n, &tree, rtl);
        }
    }
    goldens.finish();
}

/// The panel sits in the end corner (start corner in Hebrew), the buttons inside it, and
/// at text scale 2 it still fits a small window.
#[test]
fn panel_fits_and_mirrors() {
    let Some(mut h) = Harness::new() else { return };
    let pal = Palette::dark();
    let p = panel(false, true);
    let mut out = Vec::new();
    h.render(SIZE, 1.0, backdrop, |dl, text| {
        let area = (0.0, 0.0, SIZE.0, SIZE.1);
        out.push(draw(dl, text, &pal, &p, area, None, false, 1.0));
        out.push(draw(dl, text, &pal, &p, area, None, true, 1.0));
        out.push(draw(
            dl,
            text,
            &pal,
            &p,
            (0.0, 0.0, 480.0, 320.0),
            None,
            false,
            2.0,
        ));
    });
    let (l, r) = (out[0], out[1]);
    let (pl, pr) = (l.panel.unwrap(), r.panel.unwrap());
    assert!(pl.x > SIZE.0 / 2.0 && pr.right() < SIZE.0 / 2.0);
    assert!((pl.x - (SIZE.0 - pr.right())).abs() < 0.01);
    for d in &out {
        let panel = d.panel.unwrap();
        for b in [d.play.unwrap(), d.csv.unwrap(), d.chart.unwrap()] {
            assert!(
                b.x >= panel.x && b.right() <= panel.right() + 0.01,
                "{b:?} {panel:?}"
            );
            assert!(b.y >= panel.y && b.bottom() <= panel.bottom() + 0.01);
        }
    }
    let small = out[2].panel.unwrap();
    assert!(small.right() <= 480.0 && small.x >= 0.0);
}
