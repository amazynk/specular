//! Screenshot tests of linked views on synthetic series whose slices are drawn from one
//! analytic phantom (body, lungs, spine, a lesion) at each pixel's patient position, so the
//! series agree where they cross: thin and thick axial CT (with a gap), a sagittal MR and an
//! aligned prior in 2 × 2 (left to right and Hebrew right to left), 3 × 3 with empty
//! viewports, 2 × 1 on the light palette at 1.5×, and 2 × 1 of a quadruped (veterinary
//! orientation letters: Rt/Le, D/V, Cr/Cd); each with reference lines, the 3D
//! cursor, flags, the bar and the series strip, and the screen-reader audit. Rendered
//! offscreen on lavapipe and compared with the PNGs in `app/specular/tests/shots/`;
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular linked::shots` rewrites them.

use std::collections::HashMap;
use std::sync::Arc;

use specular_dicom::linked::{Link, Plane};
use specular_render::{VoiShape, Window as WlWindow};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;
use specular_widgets::Button;

use super::draw::{self, Grid, layout};
use super::load::{Pixels, Slice};
use super::tests::{prior, quadruped, sagittal, thick, thin};
use super::*;

/// Hounsfield units of the phantom at patient point `p` (mm).
fn phantom(p: [f64; 3]) -> f32 {
    let [x, y, z] = p;
    let inside = |cx: f64, cy: f64, rx: f64, ry: f64| {
        ((x - cx) / rx).powi(2) + ((y - cy) / ry).powi(2) <= 1.0
    };
    if !(-5.0..=105.0).contains(&z) || !inside(0.0, 0.0, 115.0, 85.0) {
        return -1000.0;
    }
    let lesion = (x - 40.0).powi(2) + (y + 20.0).powi(2) + (z - 45.0).powi(2) <= 100.0;
    if lesion {
        return 140.0;
    }
    if inside(0.0, 52.0, 14.0, 14.0) {
        return 750.0;
    }
    if (15.0..=90.0).contains(&z)
        && (inside(-48.0, -8.0, 34.0, 46.0) || inside(48.0, -8.0, 34.0, 46.0))
    {
        return -820.0;
    }
    // Ribs: a thin ring near the skin.
    if !inside(0.0, 0.0, 108.0, 78.0) && (z as i64).rem_euclid(12) < 5 {
        return 500.0;
    }
    40.0
}

fn slice(pl: &Plane, mr: bool) -> Slice {
    let (w, h) = (pl.size[0], pl.size[1]);
    let mut v = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let hu = phantom(pl.at(f64::from(x), f64::from(y)));
            // An MR-like look: fat and soft tissue bright, bone and air dark.
            v.push(if mr {
                match hu {
                    h if h < -500.0 => 0.0,
                    h if h > 400.0 => 120.0,
                    h if h > 100.0 => 900.0,
                    _ => 600.0,
                }
            } else {
                hu
            });
        }
    }
    let window = if mr {
        WlWindow {
            center: 500.0,
            width: 1000.0,
            shape: VoiShape::Linear,
            invert: false,
        }
    } else {
        WlWindow {
            center: 40.0,
            width: 400.0,
            shape: VoiShape::Linear,
            invert: false,
        }
    };
    Slice {
        width: w,
        height: h,
        pixels: Pixels::Grey(v),
        window: Some(window),
        range: if mr { (0.0, 900.0) } else { (-1000.0, 750.0) },
        inverted: false,
    }
}

struct Case {
    name: &'static str,
    scale: f32,
    rtl: bool,
    light: bool,
    /// The thin CT and the sagittal MR as a quadruped's trunk.
    quadruped: bool,
    grid: Grid,
    size: (f32, f32),
}

#[test]
fn linked_views_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-linked",
    );
    let cases = [
        Case {
            name: "linked-2x2-1x",
            scale: 1.0,
            rtl: false,
            light: false,
            quadruped: false,
            grid: Grid::new(2, 2),
            size: (960.0, 720.0),
        },
        Case {
            name: "linked-2x2-he-1x",
            scale: 1.0,
            rtl: true,
            light: false,
            quadruped: false,
            grid: Grid::new(2, 2),
            size: (960.0, 720.0),
        },
        Case {
            name: "linked-3x3-1x",
            scale: 1.0,
            rtl: false,
            light: false,
            quadruped: false,
            grid: Grid::new(3, 3),
            size: (1100.0, 800.0),
        },
        Case {
            name: "linked-2x1-light-1_5x",
            scale: 1.5,
            rtl: false,
            light: true,
            quadruped: false,
            grid: Grid::new(2, 1),
            size: (900.0, 520.0),
        },
        Case {
            name: "linked-2x1-quadruped-1x",
            scale: 1.0,
            rtl: false,
            light: false,
            quadruped: true,
            grid: Grid::new(2, 1),
            size: (900.0, 520.0),
        },
    ];
    for c in cases {
        let lang = if c.rtl {
            specular_i18n::langid!("he")
        } else {
            specular_i18n::langid!("en-US")
        };
        let pal = if c.light {
            Palette::light()
        } else {
            Palette::dark()
        };
        for d in ["Series 1", "Series 2", "Series 3"] {
            h.data_text(d);
        }
        let shot = specular_i18n::I18n::with_thread_language(lang, || {
            let mut aligned: HashMap<(String, String), Link> = HashMap::new();
            let entries = if c.quadruped {
                vec![
                    quadruped(thin, "ABDOMEN"),
                    thick(&[6, 7, 8]),
                    quadruped(sagittal, "ABDOMEN"),
                    prior(),
                ]
            } else {
                vec![thin(), thick(&[6, 7, 8]), sagittal(), prior()]
            };
            let mut s = Session::new(entries, 0, load::Loader::start(Arc::new(|| {})));
            s.grid = c.grid;
            if c.grid.count() == 2 {
                // Thin CT beside the sagittal MR.
                s.ports[1].set = Some(2);
            }
            let ts = 1.0;
            let area = [0.0, 0.0, f64::from(c.size.0), f64::from(c.size.1)];
            s.layout = Some(layout(
                area,
                f64::from(c.scale),
                c.rtl,
                s.grid,
                draw::bar_height(ts),
                draw::strip_height(ts),
            ));
            // The prior lined up by hand: its slice 4 (z 47.25) is thin z 10.
            s.go_to(0, 10, &aligned);
            if c.grid.count() >= 4 {
                s.ports[3].slice = 4;
                s.align(&mut aligned);
            }
            // Thin slice 37 falls in the thick series' gap; the 3D cursor on the lesion.
            s.go_to(0, 37, &aligned);
            s.place_cursor(0, (167.5, 107.5), &aligned);
            s.go_to(0, 45, &aligned);
            if c.grid.count() >= 4 {
                // Show the gap flag: the thick viewport at the gap's position.
                s.go_to(0, 37, &aligned);
            }
            for i in 0..s.visible() {
                if let (Some(k), Some(pl)) = (s.key(i), s.plane(i).copied()) {
                    let mr = s.entry(i).is_some_and(|e| e.series.modality == "MR");
                    s.insert(k, Arc::new(slice(&pl, mr)));
                }
            }
            s.upload(&h.gpu, &mut h.ui);
            let sc = s.scene(&aligned, true, &|i| s.tex_id(i)).expect("scene");
            // A first pass places the bar and the strip; the audit reads them.
            let mut placed = None;
            h.render(
                c.size,
                c.scale,
                |_| {},
                |dl, text| {
                    placed = Some(draw::draw(dl, text, &sc, &pal, c.rtl, ts));
                },
            );
            let placed = placed.expect("placed");
            super::super::a11y_audit::audit_screen(c.name, c.size, c.rtl, |t| {
                draw::a11y(t, &sc, &placed, 1.0);
            });
            // The bar's buttons and the strip's series are hit where they were drawn.
            s.placed = placed.clone();
            let b = placed.controls[1].1;
            assert_eq!(
                s.press(
                    (f64::from(b.x + 2.0), f64::from(b.y + 2.0)),
                    Button::Primary,
                    false,
                    &aligned
                ),
                Some(Control::Lines)
            );
            let shot = h.render(
                c.size,
                c.scale,
                |_| {},
                |dl, text| {
                    draw::draw(dl, text, &sc, &pal, c.rtl, ts);
                },
            );
            s.free_textures(&mut h.ui);
            shot
        });
        goldens.check(c.name, &shot);
    }
    goldens.finish();
}
