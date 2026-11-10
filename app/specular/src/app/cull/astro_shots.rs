//! Screenshot tests of cull mode on astronomy subframes: the strip with a frame's metrics
//! and the subframe table and chart (sorted by HFD, an auto-reject suggestion, a rejected
//! frame), left to right in English and right to left in Hebrew, at 1× and 1.5×; the
//! panel's controls and its screen-reader audit.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular cull::astro_shots`.

use super::astro::{self, Column, Frame, Panel, RowView, draw_panel};
use super::*;
use specular_decode::astro::{FrameMetrics, Metric};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;

const SIZE: (f32, f32) = (1000.0, 640.0);

/// A dark star field.
fn sky(dl: &mut DrawList) {
    dl.rect(
        Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
        0.0,
        Color::rgba(0.04, 0.05, 0.07, 1.0),
    );
    let mut s: u32 = 12345;
    for _ in 0..140 {
        s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
        let x = (s >> 8) % SIZE.0 as u32;
        s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
        let y = (s >> 8) % SIZE.1 as u32;
        let r = 0.8 + ((s >> 4) % 3) as f32 * 0.6;
        dl.circle((x as f32, y as f32), r, Color::rgba(0.9, 0.92, 1.0, 0.85));
    }
}

fn area() -> (f32, f32, f32, f32) {
    (0.0, 0.0, SIZE.0, SIZE.1)
}

fn rows(prefix: &str) -> Vec<RowView> {
    (0..24)
        .map(|i| {
            let hfd = match i {
                9 => 5.6,
                17 => 4.9,
                _ => 3.1 + ((i * 7) % 5) as f32 * 0.06,
            };
            RowView {
                name: format!("{prefix}_{:04}.fits", i + 1),
                frame: Some(Frame {
                    object: Some("NGC 7000".into()),
                    filter: Some(if i % 2 == 0 { "Ha" } else { "OIII" }.into()),
                    exposure: Some(300.0),
                    temp: Some(-10.0),
                    metrics: Some(FrameMetrics {
                        stars: if i == 9 {
                            310
                        } else {
                            820 + (i as u32 * 13) % 60
                        },
                        measured: 300,
                        hfd: Some(hfd),
                        fwhm: Some(hfd * 0.88),
                        eccentricity: Some(0.38 + (i % 4) as f32 * 0.03),
                        background: 1180.0 + i as f32 * 4.0,
                        noise: 12.0,
                        snr: Some(1.9 - (i % 3) as f32 * 0.1),
                        scale: Some(1.94),
                    }),
                }),
                flag: if i == 4 { -1 } else { 0 },
            }
        })
        .collect()
}

fn panel(prefix: &str) -> Panel {
    let rows = rows(prefix);
    let values: Vec<Option<f32>> = rows
        .iter()
        .map(|r| r.frame.as_ref()?.metrics?.get(Metric::Hfd))
        .collect();
    Panel {
        suggestion: specular_decode::astro::suggest(&values, Metric::Hfd, 2.0),
        rows,
        current: Some(6),
        sort: Some((Column::Name, false)),
        metric: Metric::Hfd,
        k: 2.0,
        pending: 0,
        same_stretch: true,
        pal: Palette::dark(),
    }
}

fn strip_for(title: &str, line: &str) -> Strip {
    Strip {
        title: title.to_owned(),
        position: (7, 24),
        exposure: Some(line.to_owned()),
        flags: Vec::new(),
        mark: Mark::default(),
        burst: None,
        flash: None,
        hover: None,
        tip: None,
        pal: Palette::dark(),
    }
}

fn line() -> String {
    [
        "300 s".to_owned(),
        "Ha".to_owned(),
        t!("cull-astro-line-hfd", value = "3.22"),
        t!(
            "cull-astro-line-fwhm-arcsec",
            value = "2.83",
            arcsec = "5.49"
        ),
        t!("cull-astro-line-ecc", value = "0.44"),
        t!("cull-astro-line-stars", count = 847),
    ]
    .join(" · ")
}

#[test]
fn cull_subframes_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-cull",
    );
    for (scale, suffix) in [(1.0, "1x"), (1.5, "1.5x")] {
        let p = panel("Light_NGC7000");
        let s = strip_for("Light_NGC7000_0007.fits", &line());
        let shot = h.render(SIZE, scale, sky, |dl, text| {
            let (bar, _) = draw_strip(dl, text, &s, area(), false, 1.0);
            draw_panel(dl, text, &p, area(), bar.y, false, 1.0);
        });
        goldens.check(&format!("cull-subframes-ltr-{suffix}"), &shot);
    }
    let shot = specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        let p = panel("אור_NGC7000");
        let s = strip_for("אור_NGC7000_0007.fits", &line());
        h.render(SIZE, 1.0, sky, |dl, text| {
            let (bar, _) = draw_strip(dl, text, &s, area(), true, 1.0);
            draw_panel(dl, text, &p, area(), bar.y, true, 1.0);
        })
    });
    goldens.check("cull-subframes-rtl-1x", &shot);
    goldens.finish();
}

/// The panel sits above the strip inside the window, its controls inside it without
/// overlapping, mirrored right to left; the suggestion is the two worst frames.
#[test]
fn panel_controls_sit_inside_and_mirror() {
    let Some(mut h) = Harness::new() else { return };
    let p = panel("Light");
    let s = p.suggestion.as_ref().unwrap();
    assert_eq!(s.frames, vec![9, 17]);
    let st = strip_for("Light_0007.fits", "300 s");
    let mut out = Vec::new();
    for rtl in [false, true] {
        h.render(SIZE, 1.0, sky, |dl, text| {
            let (bar, _) = draw_strip(dl, text, &st, area(), rtl, 1.0);
            out.push((bar, draw_panel(dl, text, &p, area(), bar.y, rtl, 1.0)));
        });
    }
    let (bar, (panel, hits)) = &out[0];
    let (_, (rpanel, rhits)) = &out[1];
    assert!(panel.bottom() <= bar.y && panel.y >= 0.0 && panel.right() <= SIZE.0);
    assert!((rpanel.x - (SIZE.0 - panel.right())).abs() < 0.01);
    for (i, (r, a)) in hits.iter().enumerate() {
        assert!(
            r.x >= panel.x - 0.01 && r.right() <= panel.right() + 0.01,
            "{a:?} {r:?}"
        );
        assert!(
            r.y >= panel.y && r.bottom() <= panel.bottom() + 0.01,
            "{a:?} {r:?}"
        );
        for (q, b) in &hits[i + 1..] {
            let apart = r.right() <= q.x + 0.01
                || q.right() <= r.x + 0.01
                || r.bottom() <= q.y + 0.01
                || q.bottom() <= r.y + 0.01;
            // A row spans its cells; the header and the rows do not overlap.
            assert!(apart, "{a:?} {r:?} overlaps {b:?} {q:?}");
        }
    }
    for ((r, a), (q, b)) in hits.iter().zip(rhits) {
        assert_eq!(a, b);
        assert!((r.x - (SIZE.0 - q.right())).abs() < 0.5, "{a:?}");
    }
    for c in Column::ALL {
        assert!(
            hits.iter().any(|h| h.1 == astro::PanelHit::Column(c)),
            "{c:?}"
        );
    }
    assert!(hits.iter().any(|h| h.1 == astro::PanelHit::Row(6)));
    assert!(p.suggestion_text().contains("HFD"));
}

#[test]
fn subframe_panel_passes_the_screen_reader_audit() {
    let mut text = specular_gfx::Text::bundled_only();
    for rtl in [false, true] {
        let p = panel("Light");
        let mut dl = DrawList::new(1.0);
        let st = strip_for("Light_0007.fits", "300 s");
        let (bar, shits) = draw_strip(&mut dl, &mut text, &st, area(), rtl, 1.0);
        let (r, hits) = draw_panel(&mut dl, &mut text, &p, area(), bar.y, rtl, 1.0);
        let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
        a11y(&mut tree, &st, Some(bar), &shits, 1.0);
        astro::a11y(&mut tree, &p, r, &hits, 1.0);
        super::super::a11y_audit::expect_clean("subframes", &tree, rtl);
    }
}
