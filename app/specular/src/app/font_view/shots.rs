//! Screenshot tests of the font specimen: the synthetic fixtures of specular-font drawn by
//! the text stack on the GPU: a variable font at its default and at a slider moved, its grid
//! scrolled into view, a COLR version 0 font with its colour row, a COLR version 1 font with
//! the line saying what is not drawn in colour, at 1× and 1.5×, light and dark, and right to
//! left in Hebrew. Rendered offscreen on lavapipe and compared with the PNGs in
//! `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular font_view::shots`, then review and
//! commit the rewritten PNGs.

use specular_shots::{Goldens, Harness};
use specular_theme::{Mode, Palette};

use super::*;

const SIZE: (f32, f32) = (560.0, 420.0);

fn fixture(name: &str) -> FontModel {
    let path = format!(
        "{}/../../crates/formats/specular-font/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let file = specular_decode::specular_font::open(&bytes, Default::default()).unwrap();
    FontModel {
        file: Arc::new(file),
        face: 0,
    }
}

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    font: &'static str,
    scale: f32,
    dark: bool,
    rtl: bool,
    /// Moves the first axis to this fraction of its range.
    axis: Option<f32>,
    /// Logical px scrolled.
    scroll: f32,
}

fn shoot(h: &mut Harness, goldens: &mut Goldens, c: &Case) {
    let pal = if c.dark {
        Palette::for_mode(Mode::Dark)
    } else {
        Palette::for_mode(Mode::Light)
    };
    let mut v = FontView::default();
    v.show(1, &fixture(c.font));
    if let (Some(f), Some(a)) = (c.axis, v.spec.as_ref().and_then(|s| s.axes.first())) {
        v.values[0] = (a.min + f * (a.max - a.min)).round();
    }
    v.scroll = c.scroll;
    v.prepare(&h.gpu, &mut h.ui, c.scale, pal.canvas);
    let rtl = c.rtl;
    let shot = h.render(
        SIZE,
        c.scale,
        |_| {},
        |dl, text| {
            draw(
                dl,
                text,
                &pal,
                pal.canvas,
                &mut v,
                (0.0, 0.0, SIZE.0, SIZE.1),
                rtl,
                1.0,
            );
        },
    );
    goldens.check(&format!("{}-{}x", c.name, c.scale), &shot);
    super::super::a11y_audit::audit_screen(c.name, SIZE, rtl, |t| a11y(t, &v, 1.0));
}

#[test]
fn specimens_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    // A named instance of the variable font, from the font itself.
    h.data_text("Light");
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-fonts",
    );
    let base = Case {
        name: "",
        font: "var.ttf",
        scale: 1.0,
        dark: false,
        rtl: false,
        axis: None,
        scroll: 0.0,
    };
    let cases = [
        Case {
            name: "font-variable",
            ..base
        },
        Case {
            name: "font-variable-heavy-dark",
            axis: Some(1.0),
            dark: true,
            scale: 1.5,
            ..base
        },
        Case {
            name: "font-grid",
            font: "sans.ttf",
            scroll: 330.0,
            ..base
        },
        Case {
            name: "font-colr0",
            font: "colr0.ttf",
            ..base
        },
        Case {
            name: "font-colr1-dark",
            font: "colr1.ttf",
            dark: true,
            ..base
        },
        Case {
            name: "font-cff2",
            font: "var-cff2.otf",
            axis: Some(0.25),
            ..base
        },
    ];
    for c in &cases {
        shoot(&mut h, &mut goldens, c);
    }
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        let c = Case {
            name: "font-variable-he",
            rtl: true,
            ..base
        };
        shoot(&mut h, &mut goldens, &c);
    });
    goldens.finish();
}
