//! Screenshot tests of the markup tool: its bar, the list, and each kind of markup over a
//! drawing; left to right, in Hebrew (right to left) and with the text field open. Rendered
//! offscreen on lavapipe and compared with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular markup::shots`, then review the
//! rewritten PNGs.

use super::*;
use specular_markup::P;
use specular_shots::{Goldens, Harness};

const SIZE: (f32, f32) = (960.0, 680.0);

/// What stands for a drawing: white paper with a frame and a few walls.
fn backdrop(dl: &mut DrawList) {
    dl.rect(
        Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
        0.0,
        Color::rgba(0.2, 0.2, 0.22, 1.0),
    );
    dl.rect(Rect::new(40.0, 40.0, 880.0, 600.0), 0.0, Color::WHITE);
    let ink = Color::rgba(0.1, 0.1, 0.1, 1.0);
    dl.stroke_rect(Rect::new(60.0, 60.0, 840.0, 560.0), 0.0, ink, 2.0);
    for (a, b) in [
        ((120.0, 200.0), (520.0, 200.0)),
        ((120.0, 200.0), (120.0, 560.0)),
        ((320.0, 200.0), (320.0, 420.0)),
        ((120.0, 420.0), (520.0, 420.0)),
        ((520.0, 200.0), (520.0, 560.0)),
    ] {
        dl.line(a, b, 3.0, ink);
    }
    // The title block, bottom right (under the list in the left-to-right shots); ruled so it
    // does not read as a stray box where the right-to-left list leaves it bare.
    dl.stroke_rect(Rect::new(700.0, 520.0, 190.0, 90.0), 0.0, ink, 1.0);
    dl.line((700.0, 550.0), (890.0, 550.0), 1.0, ink);
    dl.line((700.0, 580.0), (890.0, 580.0), 1.0, ink);
    dl.line((800.0, 550.0), (800.0, 610.0), 1.0, ink);
}

fn item(shape: Shape, text: &str, colour: usize) -> Markup {
    let mut m = Markup::new(shape, COLOURS[colour].0, 2.0, 16.0, "Dana Levi");
    m.text = text.to_owned();
    m.date = "2026-10-24T09:30:00Z".to_owned();
    m
}

/// Every kind of markup, sheet units being logical px (zoom 1).
fn sample_items() -> Vec<Markup> {
    vec![
        item(
            Shape::Cloud(vec![
                P::new(340.0, 230.0),
                P::new(500.0, 230.0),
                P::new(500.0, 330.0),
                P::new(340.0, 330.0),
            ]),
            "",
            0,
        ),
        item(
            Shape::Text(P::new(140.0, 440.0)),
            "Check beam size\nsee detail 4",
            1,
        ),
        item(
            Shape::Arrow {
                tail: P::new(240.0, 160.0),
                head: P::new(330.0, 196.0),
            },
            "New door",
            0,
        ),
        item(
            Shape::Stamp {
                at: P::new(605.0, 270.0),
                kind: Stamp::Approved,
            },
            "",
            2,
        ),
        item(
            Shape::Length(P::new(120.0, 590.0), P::new(520.0, 590.0)),
            "8.000 m",
            5,
        ),
        item(
            Shape::Area(vec![
                P::new(130.0, 210.0),
                P::new(310.0, 210.0),
                P::new(310.0, 410.0),
                P::new(130.0, 410.0),
            ]),
            "Area 18.00 m²",
            3,
        ),
        item(
            Shape::Pin {
                at: P::new(600.0, 380.0),
                photo: PathBuf::from("/site/2026-10-24/IMG_0042.jpg"),
            },
            "IMG_0042.jpg",
            4,
        ),
    ]
}

fn sample(prompt: bool) -> MarkupView {
    let items: Vec<ItemView> = sample_items()
        .iter()
        .enumerate()
        .map(|(i, m)| item_view(i + 1, m, &|p| (p.x as f32, p.y as f32), 1.0))
        .collect();
    let marks = vec![(600.0, 470.0), (680.0, 455.0), (690.0, 500.0)];
    let mut drawing = marks.clone();
    drawing.push((620.0, 510.0));
    MarkupView {
        open: true,
        kind: if prompt { Kind::Text } else { Kind::Cloud },
        colour: 0,
        width: 1,
        stamp: Stamp::Approved,
        items,
        selected: Some(1),
        drawing: if prompt { Vec::new() } else { drawing },
        drawing_closed: true,
        marks: if prompt { Vec::new() } else { marks },
        cursor: (!prompt).then_some((620.0, 510.0)),
        status: if prompt {
            Kind::Text.hint()
        } else {
            Kind::Cloud.hint()
        },
        prompt: prompt.then(|| {
            // Two lines, the caret in the first.
            let mut p = Prompt::new(Asking::Text(P::new(0.0, 0.0)), "Fire door\nEI 30", true);
            p.move_line(-1);
            p.view()
        }),
        can_undo: true,
        can_redo: false,
        has_measurements: false,
        focus: (!prompt).then_some(1),
    }
}

#[test]
fn markup_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-markup",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    let area = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
    let dark = Palette::dark();
    let light = Palette::light();
    for (name, lang, pal, rtl, prompt) in [
        ("ltr", &en, &dark, false, false),
        ("he-rtl", &he, &dark, true, false),
        ("text-field", &en, &light, false, true),
    ] {
        let (shot, tree) = specular_i18n::I18n::with_thread_language(lang.clone(), || {
            let v = sample(prompt);
            let mut drawn = None;
            let shot = h.render(SIZE, 1.0, backdrop, |dl, text| {
                drawn = Some(draw(dl, text, pal, &v, area, rtl, 1.0));
            });
            let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
            a11y(&mut tree, &v, &drawn.expect("drawn"), 1.0);
            (shot, tree)
        });
        goldens.check(&format!("markup-{name}"), &shot);
        super::super::a11y_audit::expect_clean(&format!("markup-{name}"), &tree, rtl);
    }
    goldens.finish();
}

/// The buttons sit in the bar without overlapping, every control maps back from its node,
/// and a click on an item picks it (the topmost; inside a cloud counts).
#[test]
fn controls_and_picking() {
    let Some(mut h) = Harness::new() else { return };
    let v = sample(false);
    let pal = Palette::dark();
    let area = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
    let mut drawn = None;
    let _ = h.render(SIZE, 1.0, backdrop, |dl, text| {
        drawn = Some(draw(dl, text, &pal, &v, area, false, 1.0));
    });
    let d = drawn.unwrap();
    for (i, (a, _)) in d.hits.iter().enumerate() {
        for (b, _) in &d.hits[i + 1..] {
            let overlap =
                a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom();
            assert!(!overlap, "{a:?} overlaps {b:?}");
        }
    }
    for f in v.focusables() {
        assert_eq!(target(A11Y_BASE + node(f), &v), Some(f));
    }
    // The cloud (0), its inside, the area's edge (5), the pin (6), nothing.
    assert_eq!(pick(&v.items, (420.0, 280.0)), Some(0));
    assert_eq!(pick(&v.items, (130.0, 300.0)), Some(5));
    assert_eq!(pick(&v.items, (600.0, 365.0)), Some(6));
    assert_eq!(pick(&v.items, (880.0, 600.0)), None);
}

#[test]
fn stamps_carry_the_date_and_name() {
    let m = &sample_items()[3];
    assert_eq!(m.stamp_line(), "2026-10-24 · Dana Levi");
    let words = specular_i18n::I18n::with_thread_language(specular_i18n::langid!("en-US"), || {
        stamp_words(m)
    });
    assert_eq!(words, "APPROVED");
}

/// A black or white item's swatch shows on the list panel: its outline stands out from the
/// panel in both themes (it was a bare black square on the dark panel).
#[test]
fn swatches_have_a_visible_edge() {
    let contrast = |a: Color, b: Color| {
        let (la, lb) = (a.luminance() + 0.05, b.luminance() + 0.05);
        la.max(lb) / la.min(lb)
    };
    for pal in [Palette::dark(), Palette::light()] {
        let c = contrast(swatch_edge(&pal), pal.surface);
        assert!(c >= 3.0, "swatch edge contrast {c}");
    }
    assert!(COLOURS.iter().any(|c| c.0 == [33, 33, 33]));
}

/// The export sets Hebrew with the viewer's text system: outlines above the baseline, as
/// wide as the line (it printed `?` in Helvetica).
#[test]
fn export_outlines_hebrew_with_the_viewers_fonts() {
    let text = std::cell::RefCell::new(Some(Text::bundled_only()));
    let o = outline_text(&text, "שלום עולם", 20.0, false).expect("outlines");
    assert!(o.width > 40.0 && o.width < 200.0, "{}", o.width);
    let ys: Vec<f64> = o
        .path
        .iter()
        .filter_map(|e| match e {
            mk::pdf::PathEl::Move(p) | mk::pdf::PathEl::Line(p) => Some(p.y),
            _ => None,
        })
        .collect();
    assert!(ys.len() > 20);
    let top = ys.iter().copied().fold(f64::INFINITY, f64::min);
    assert!(
        top < -8.0 && top > -25.0,
        "letters stand on the baseline: {top}"
    );
}

/// A text markup's field takes several lines; labels and stamps one (it was one line, and
/// editing a two-line text joined its lines).
#[test]
fn text_field_takes_several_lines() {
    let at = P::new(0.0, 0.0);
    let mut p = Prompt::new(Asking::Text(at), "Check beam size\nsee detail 4", true);
    assert_eq!(p.caret(), (1, 12));
    assert_eq!(p.text(), "Check beam size\nsee detail 4");
    // Up keeps the byte; Shift+Enter splits there; Backspace at a line start joins.
    assert!(p.move_line(-1));
    assert_eq!(p.caret(), (0, 12));
    assert!(p.new_line());
    assert_eq!(p.lines(), ["Check beam s", "ize", "see detail 4"]);
    assert_eq!(p.caret(), (1, 0));
    assert!(p.join(true));
    assert_eq!(p.text(), "Check beam size\nsee detail 4");
    assert_eq!(p.caret(), (0, 12));
    assert!(!p.move_line(-1));
    // Delete at a line's end takes the next line up.
    p.field.set_caret(15, false);
    assert!(p.join(false));
    assert_eq!(p.text(), "Check beam sizesee detail 4");
    assert!(!p.join(false));
    // Pasted lines stay lines.
    p.field.set_caret(15, false);
    p.paste("\r\nA\nB ");
    assert_eq!(p.text(), "Check beam size\nA\nB see detail 4");
    assert_eq!(p.view().lines.len(), 3);
    let en = specular_i18n::langid!("en-US");
    let view = specular_i18n::I18n::with_thread_language(en, || p.view());
    assert!(view.label.contains("Shift+Enter"), "{}", view.label);
    // A label keeps one line.
    let mut l = Prompt::new(Asking::Arrow(at, at), "a\nb", false);
    assert!(!l.new_line());
    l.paste("c\nd");
    assert_eq!(l.lines(), ["a bc d"]);
}

/// A press in the field puts the caret on the line and at the byte it lands on (another
/// line moves the caret there, the text unchanged); a drag then selects on that line, and
/// the bar draws the selection's lines and their bands.
#[test]
fn the_field_takes_presses_and_drags() {
    let at = P::new(0.0, 0.0);
    let mut p = Prompt::new(Asking::Text(at), "Check beam size\nsee detail 4", true);
    p.preedit = "x".into();
    p.press(0, 6, 50.0, 1.0, false);
    assert_eq!(p.caret(), (0, 6));
    assert_eq!(p.preedit, "", "a press drops the composition");
    assert_eq!(p.text(), "Check beam size\nsee detail 4");
    p.field.drag_to(10);
    assert_eq!(p.field.selected_text(), "beam");
    assert_eq!(p.view().selection, (6, 10));
    p.field.release();
    // Shift on another line only moves there.
    p.press(1, 4, 60.0, 2.0, true);
    assert_eq!(p.caret(), (1, 4));
    assert_eq!(p.field.selection(), 4..4);
    assert_eq!(p.lines(), ["Check beam size", "see detail 4"]);
    // Past the line's end: its end.
    p.press(1, 99, 70.0, 3.0, false);
    assert_eq!(p.caret(), (1, 12));

    let Some(mut h) = Harness::new() else { return };
    p.press(0, 6, 50.0, 4.0, false);
    p.field.drag_to(10);
    let v = p.view();
    let f = Field {
        label: &v.label,
        lines: &v.lines,
        caret: v.caret,
        preedit: &v.preedit,
        selection: v.selection,
    };
    let mut lines = Vec::new();
    let area = Rect::new(0.0, 0.0, 800.0, 500.0);
    h.render(
        (800.0, 500.0),
        1.0,
        |_| {},
        |dl, text| {
            let pal = Palette::dark();
            lines = bar(
                dl,
                text,
                &pal,
                "Markup",
                &[],
                Some(f),
                "",
                0.0,
                area,
                false,
                1.0,
            )
            .3;
        },
    );
    assert_eq!(lines.len(), 2);
    let (a, b) = (lines[0].0, lines[1].0);
    assert!(
        a.bottom() <= b.y + 0.01 && a.x == b.x && a.w == b.w,
        "{a:?} {b:?}"
    );
    let (_, block, x0) = &lines[0];
    let x = x0 + block.caret_x(6) + 0.5;
    assert_eq!(block.hit_byte(x - x0), 6);
}
