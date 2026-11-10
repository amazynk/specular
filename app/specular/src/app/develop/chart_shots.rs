//! Screenshot tests of "Make profile from chart…": the panel while the corners are marked
//! (one in) and with its report (split squares, ΔE lines, a patch left out, the camera
//! switch). English dark left to right and Hebrew light right to left, at 1×, 1.5× and 2×.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular develop::chart_shots`, then
//! review and commit the rewritten PNGs.

use specular_develop::chart::colour;
use specular_develop::chart::layout::{ChartKind, Layout};
use specular_develop::chart::sample::{Flag, Sampled};
use specular_develop::chart::solve;
use specular_gfx::{DrawList, Rect};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;

use super::chart_profile::{Flow, Measured, Step, tests::cc24_file};

const SIZE: (f32, f32) = (520.0, 760.0);

fn marking() -> Flow {
    let mut f = Flow::new(true, "Canon EOS R5".into());
    f.load_reference("ColorChecker24_After_Nov2014.txt".into(), &cc24_file());
    f.click((0.2, 0.3));
    f
}

/// A camera a matrix away from the reference, with one glare patch.
fn report() -> Flow {
    let mut f = marking();
    for c in [(0.8, 0.3), (0.8, 0.7), (0.2, 0.7)] {
        f.click(c);
    }
    let layout: Layout = f.layout.clone().expect("layout");
    assert_eq!(layout.kind, ChartKind::ColorChecker24);
    let cam = [
        [0.6722, 0.1350, -0.0613],
        [-0.0910, 0.9200, 0.1721],
        [0.0140, 0.0850, 0.5530],
    ];
    let samples: Vec<Sampled> = layout
        .patches
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let raw = colour::mulv(&cam, colour::lab_to_xyz(p.reference));
            let k = 1.0 + 0.01 * ((i * 7 % 5) as f64 - 2.0);
            Sampled {
                centre: (0.0, 0.0),
                rgb: raw.map(|v| v * 0.8 * k),
                spread: 0.0,
                flag: (i == 9).then_some(Flag::Uneven),
            }
        })
        .collect();
    let fit = solve::solve(&solve::Input {
        layout: &layout,
        samples: &samples,
        before: colour::IDENTITY,
        table: true,
    })
    .expect("fit");
    f.measured(Ok(Measured {
        fit,
        ids: layout.patches.iter().map(|p| p.id.clone()).collect(),
        cols: layout.cols,
        rows: layout.rows,
        at: layout.patches.iter().map(|p| p.at).collect(),
        meta: None,
    }));
    f.use_for_camera = true;
    f
}

#[test]
fn panel_buttons_are_hit_where_drawn() {
    let Some(mut h) = Harness::new() else { return };
    let mut f = report();
    assert_eq!(f.step, Step::Report);
    let pal = Palette::dark();
    let window = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
    let _ = h.render(
        SIZE,
        1.0,
        |_| {},
        |dl, text| f.draw(dl, text, &pal, window, false, 1.0),
    );
    let r = f.rect;
    assert!(r.w > 0.0 && r.h > 0.0);
    assert!(f.contains((r.x + 4.0, r.y + 4.0)));
    // The last row holds the buttons: Save at the end side, then Mark again and Close.
    use super::chart_profile::Button;
    let y = r.bottom() - 20.0;
    assert_eq!(f.hit((r.right() - 30.0, y)), Some(Button::Save));
    let order: Vec<Button> = (0..r.w as i32)
        .rev()
        .filter_map(|dx| f.hit((r.x + dx as f32, y)))
        .fold(Vec::new(), |mut v, b| {
            if v.last() != Some(&b) {
                v.push(b);
            }
            v
        });
    assert_eq!(order, [Button::Save, Button::Again, Button::Close]);
}

#[test]
fn chart_panel_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-chart",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    for (name, lang, rtl) in [("en-dark", en, false), ("he-light", he, true)] {
        h.text.set_locale(Some(lang.language.as_str().to_owned()));
        let pal = if rtl {
            Palette::light()
        } else {
            Palette::dark()
        };
        for (scale, suffix) in [(1.0f32, "1x"), (1.5, "1.5x"), (2.0, "2x")] {
            for (state, make) in [("marking", marking as fn() -> Flow), ("report", report)] {
                let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                    let mut f = make();
                    let window = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
                    h.render(
                        SIZE,
                        scale,
                        |dl: &mut DrawList| dl.rect(window, 0.0, pal.canvas),
                        |dl, text| f.draw(dl, text, &pal, window, rtl, 1.0),
                    )
                });
                goldens.check(&format!("develop-chart-{state}-{name}-{suffix}"), &shot);
            }
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}
