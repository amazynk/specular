//! Screenshot tests of cull mode's strip at 1× and 1.5×: a RAW+JPEG pair picked with three
//! stars and a label, with the confirmation above it, left-to-right and right-to-left, and an
//! unmarked image, and a collapsed burst. Rendered offscreen on lavapipe and compared with
//! the PNGs in `app/specular/tests/shots/`; see `specular-shots` for the tolerance.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular cull::shots`, then review and
//! commit the rewritten PNGs.

use super::*;
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;

/// Logical window size, as for the chrome's shots.
const SIZE: (f32, f32) = (720.0, 450.0);
const SCALES: [(f32, &str); 2] = [(1.0, "1x"), (1.5, "1.5x")];

/// A synthetic 3:2 photo (sky, ground and a sun) fitted into the window on black.
fn photo(dl: &mut DrawList) {
    dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, Color::BLACK);
    let h = SIZE.1;
    let w = h * 1.5;
    let x = (SIZE.0 - w) * 0.5;
    dl.rect(
        Rect::new(x, 0.0, w, h * 0.62),
        0.0,
        Color::rgba(0.42, 0.66, 0.9, 1.0),
    );
    dl.rect(
        Rect::new(x, h * 0.62, w, h * 0.38),
        0.0,
        Color::rgba(0.3, 0.45, 0.25, 1.0),
    );
    let r = 46.0;
    dl.rect(
        Rect::new(x + w * 0.7 - r, h * 0.3 - r, 2.0 * r, 2.0 * r),
        r,
        Color::rgba(1.0, 0.86, 0.45, 1.0),
    );
}

fn area() -> (f32, f32, f32, f32) {
    (0.0, 0.0, SIZE.0, SIZE.1)
}

fn strip(title: &str, mark: Mark, flash: Option<&str>) -> Strip {
    Strip {
        title: title.to_owned(),
        position: (3, 128),
        exposure: Some("1/250 s · f/8 · ISO 100 · 35 mm".to_owned()),
        flags: Vec::new(),
        mark,
        burst: None,
        flash: flash.map(|f| (f.to_owned(), 1.0)),
        hover: None,
        tip: None,
        pal: Palette::dark(),
    }
}

/// Two stars set and the pointer resting on the fourth: stars 1–4 lit as a preview, the
/// fourth's hover look, and its tooltip above the strip.
fn hovered(title: &str) -> Strip {
    let mut s = strip(
        title,
        Mark {
            rating: 2,
            label: 0,
            flag: 0,
        },
        None,
    );
    s.hover = Some(Hit::Star(4));
    s.tip = Some(tip_text(
        Hit::Star(4),
        &s,
        &specular_input::Keymap::default(),
    ));
    s
}

#[test]
fn cull_strip_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-cull",
    );
    let marked = Mark {
        rating: 3,
        label: 3,
        flag: 1,
    };
    let pair = strip("IMG_0412 · JPG + CR3", marked, Some("3 stars"));
    let plain = strip("DSC_0099.NEF", Mark::default(), None);
    let mut burst = strip("DSC_0100.NEF", Mark::default(), None);
    burst.burst = Some((2, 6, false));
    for (scale, suffix) in SCALES {
        let shot = h.render(SIZE, scale, photo, |dl, text| {
            draw_strip(dl, text, &pair, area(), false, 1.0);
        });
        goldens.check(&format!("cull-strip-ltr-{suffix}"), &shot);
        // In Hebrew: the strip's words and the flash made in the language they are drawn in.
        let shot = specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
            let flash = t!("gallery-stars", stars = 3);
            let rtl = strip("תמונה_0412 · JPG + CR3", marked, Some(&flash));
            h.render(SIZE, scale, photo, |dl, text| {
                draw_strip(dl, text, &rtl, area(), true, 1.0);
            })
        });
        goldens.check(&format!("cull-strip-rtl-{suffix}"), &shot);
        let shot = h.render(SIZE, scale, photo, |dl, text| {
            draw_strip(dl, text, &plain, area(), false, 1.0);
        });
        goldens.check(&format!("cull-strip-unmarked-{suffix}"), &shot);
        let shot = h.render(SIZE, scale, photo, |dl, text| {
            draw_strip(dl, text, &burst, area(), false, 1.0);
        });
        goldens.check(&format!("cull-strip-burst-{suffix}"), &shot);
        let hover = hovered("DSC_0101.NEF");
        let shot = h.render(SIZE, scale, photo, |dl, text| {
            draw_strip(dl, text, &hover, area(), false, 1.0);
        });
        goldens.check(&format!("cull-strip-hover-{suffix}"), &shot);
    }
    let shot = specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        let hover = hovered("תמונה_0101.NEF");
        h.render(SIZE, 1.0, photo, |dl, text| {
            draw_strip(dl, text, &hover, area(), true, 1.0);
        })
    });
    goldens.check("cull-strip-hover-rtl-1x", &shot);
    goldens.finish();
}

/// Every control is inside the strip, none overlap, and right-to-left mirrors them; a burst
/// adds its stack button first.
#[test]
fn controls_sit_in_the_strip_and_mirror() {
    let Some(mut h) = Harness::new() else { return };
    for burst in [None, Some((1, 4, true))] {
        let mut s = strip("IMG_0412 · JPG + CR3", Mark::default(), None);
        s.burst = burst;
        let mut ltr = None;
        let mut rtl = None;
        h.render(SIZE, 1.0, photo, |dl, text| {
            ltr = Some(draw_strip(dl, text, &s, area(), false, 1.0));
            rtl = Some(draw_strip(dl, text, &s, area(), true, 1.0));
        });
        let (bar, hits) = ltr.unwrap();
        let (rbar, rhits) = rtl.unwrap();
        assert_eq!(bar, rbar);
        let first = usize::from(burst.is_some());
        // Five stars, the flags, the label (a tooltip, no click) and the way out.
        assert_eq!(hits.len(), 9 + first);
        for (i, (r, _)) in hits.iter().enumerate() {
            assert!(
                r.x >= bar.x
                    && r.right() <= bar.right()
                    && r.y >= bar.y
                    && r.bottom() <= bar.bottom()
            );
            for (q, _) in &hits[i + 1..] {
                assert!(r.right() <= q.x || q.right() <= r.x, "{r:?} overlaps {q:?}");
            }
        }
        for ((r, a), (q, b)) in hits.iter().zip(&rhits) {
            assert_eq!(a, b);
            assert!((r.x - (SIZE.0 - q.right())).abs() < 0.01);
        }
        if burst.is_some() {
            assert_eq!(hits[0].1, Hit::Stack);
        }
        assert_eq!(hits[first].1, Hit::Star(1));
        assert_eq!(hits.last().unwrap().1, Hit::Exit);
        assert_eq!(hits.iter().filter(|h| !h.1.is_button()).count(), 1);
    }
}

/// The tooltip of a hovered control sits above the strip, inside the window, centred on its
/// control (mirrored right to left), even for the control nearest the edge.
#[test]
fn tooltip_sits_above_the_strip_inside_the_window() {
    let Some(mut h) = Harness::new() else { return };
    for rtl in [false, true] {
        let s = hovered("IMG_0412 · JPG + CR3");
        let mut out = None;
        h.render(SIZE, 1.0, photo, |dl, text| {
            out = Some(draw_strip(dl, text, &s, area(), rtl, 1.0));
        });
        let (bar, hits) = out.unwrap();
        for (r, hit) in &hits {
            let mut probe = None;
            h.render(SIZE, 1.0, photo, |dl, text| {
                let tip = tip_text(*hit, &s, &specular_input::Keymap::default());
                probe = Some(specular_widgets::draw_tooltip(
                    dl,
                    text,
                    &Palette::dark(),
                    &tip,
                    tip_anchor(bar, *r),
                    Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
                    1.0,
                ));
            });
            let t = probe.unwrap();
            assert!(t.bottom() <= bar.y, "{hit:?}: {t:?} over the strip {bar:?}");
            assert!(
                t.x >= 0.0 && t.right() <= SIZE.0 && t.y >= 0.0,
                "{hit:?}: {t:?} off screen"
            );
            assert!(
                t.x <= r.right() && t.right() >= r.x,
                "{hit:?}: {t:?} away from {r:?}"
            );
        }
    }
}

/// The strip as a screen reader finds it, in each look (marked, unmarked, a burst, hovered,
/// right to left): named controls with their state, in the order they are seen.
#[test]
fn cull_strip_passes_the_screen_reader_audit() {
    let mut text = specular_gfx::Text::bundled_only();
    let marked = Mark {
        rating: 3,
        label: 3,
        flag: 1,
    };
    let mut burst = strip("DSC_0100.NEF", Mark::default(), None);
    burst.burst = Some((2, 6, false));
    for (name, s, rtl) in [
        (
            "marked",
            strip("IMG_0412 · JPG + CR3", marked, Some("3 stars")),
            false,
        ),
        (
            "unmarked",
            strip("DSC_0099.NEF", Mark::default(), None),
            false,
        ),
        ("burst", burst, false),
        ("hover", hovered("DSC_0101.NEF"), false),
        ("rtl", strip("IMG_0412", marked, None), true),
    ] {
        let mut dl = DrawList::new(1.0);
        let (bar, hits) = draw_strip(&mut dl, &mut text, &s, area(), rtl, 1.0);
        let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
        a11y(&mut tree, &s, Some(bar), &hits, 1.0);
        super::super::a11y_audit::expect_clean(name, &tree, rtl);
    }
}
