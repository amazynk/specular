//! Screenshot tests of perspective: the Perspective card (Vertical upright, sliders set, two
//! guides), and the crop tool drawing guides over a keystoned photo (its outline and the
//! two guides). English dark left to right and Hebrew light right to left, at 1×, 1.5× and
//! 2×. Plus the card's behaviour: modes, Reset asking first, guides from the tool.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular develop::persp_shots`, then
//! review and commit the rewritten PNGs.

use std::path::PathBuf;

use specular_develop::model::params::CropOrientation;
use specular_develop::model::{History, ModuleKind};
use specular_develop::ops::geometry::persp::{Mode, PerspParams};
use specular_develop::ops::geometry::{Context, Plan};
use specular_develop::preview::{Orient, UprightLines};
use specular_gfx::{DrawList, Rect};
use specular_shots::{Goldens, Harness};
use specular_theme::{Color, Palette};
use specular_widgets::develop_panel::{Body, Event};

use super::controls::{self, Request, Session};
use super::crop::{CropTool, GuideMap, Layout, Outcome};
use super::perspective::{MODES, id};

const GUIDES: [[f32; 4]; 2] = [[0.2, 0.15, 0.24, 0.85], [0.78, 0.15, 0.74, 0.85]];

fn session() -> Session {
    let p = PathBuf::from("/photos/IMG_0042.CR3");
    let mut s = Session::new(p.clone(), controls::new_edit(&p), History::default());
    let pp = PerspParams {
        mode: Mode::Vertical,
        solved: Some([8.5, 0.0, -0.6]),
        horizontal: -12.0,
        rotate: 0.5,
        guides: GUIDES.to_vec(),
        ..PerspParams::default()
    };
    assert!(s.set_persp(&pp, 0));
    s.sync_panel();
    for c in &mut s.panel.cards {
        c.expanded = c.id == u32::from(ModuleKind::PERSPECTIVE.0);
    }
    s.panel.history_expanded = false;
    s
}

fn press(s: &mut Session, control: u32, ms: i64) -> Vec<Request> {
    s.handle(
        &[Event::Value {
            control,
            part: 0,
            done: true,
        }],
        ms,
    )
}

fn choose(s: &mut Session, m: Mode, ms: i64) -> Vec<Request> {
    if let Some(Body::Choice(c)) = s.panel.control_mut(id::MODE).map(|c| &mut c.body) {
        c.set(MODES.iter().position(|x| *x == m).unwrap());
    }
    press(s, id::MODE, ms)
}

#[test]
fn the_card_reads_the_edit() {
    let s = session();
    match s.panel.control(id::MODE).map(|c| &c.body) {
        Some(Body::Choice(c)) => assert_eq!(MODES[c.selected], Mode::Vertical),
        b => panic!("{b:?}"),
    }
    match s.panel.control(id::HORIZONTAL).map(|c| &c.body) {
        Some(Body::Slider(sl)) => assert_eq!(sl.value, -12.0),
        b => panic!("{b:?}"),
    }
    match s.panel.control(id::GUIDES).map(|c| &c.body) {
        Some(Body::Choice(c)) => assert!(c.label.contains('2'), "{}", c.label),
        b => panic!("{b:?}"),
    }
}

#[test]
fn modes_reset_and_guides() {
    let mut s = session();
    // An automatic mode asks for the picture's lines; the answer is one step.
    let r = choose(&mut s, Mode::Full, 1_000);
    assert!(
        r.iter().any(|q| matches!(q, Request::Upright(Mode::Full))),
        "{r:?}"
    );
    let at = s.history.position;
    let found = UprightLines {
        lines: Vec::new(),
        oriented: (6000.0, 4000.0),
        straighten: 0.0,
        focal35: None,
    };
    // Too few lines: nothing changes, and it says why.
    let r = s.upright_done(Mode::Full, Ok(found.clone()), 2_000);
    assert_eq!(s.history.position, at);
    assert!(
        r.iter()
            .any(|q| matches!(q, Request::Say(t) if t.contains("two"))),
        "{r:?}"
    );
    assert_eq!(s.persp().mode, Mode::Vertical);
    // Lines that lean: solved, one step, the sliders back at 0.
    let lines = (0..6)
        .map(|i| {
            let x = -2000.0 + 800.0 * f64::from(i);
            specular_develop::ops::geometry::upright::Line::new(
                (x, 1500.0),
                (x * 0.9, -1500.0),
                1.0,
            )
        })
        .collect();
    let r = s.upright_done(Mode::Vertical, Ok(UprightLines { lines, ..found }), 3_000);
    assert!(r.iter().any(|q| matches!(q, Request::Render)), "{r:?}");
    assert_eq!(s.history.position, at + 1);
    let p = s.persp();
    assert_eq!((p.mode, p.horizontal, p.rotate), (Mode::Vertical, 0.0, 0.0));
    assert!(p.solved.is_some_and(|v| v[0] > 1.0), "{p:?}");
    // Reset asks first while there are guides, then clears all.
    let r = press(&mut s, id::RESET, 4_000);
    assert!(matches!(r.first(), Some(Request::Confirm(..))), "{r:?}");
    assert_eq!(s.persp().guides.len(), 2);
    s.persp_confirmed(5_000);
    assert_eq!(s.persp(), PerspParams::default());
    // Without guides it resets at once.
    let r = press(&mut s, id::RESET, 6_000);
    assert!(!r.iter().any(|q| matches!(q, Request::Confirm(..))));
    // Guides from the crop tool: mode Guided.
    assert!(s.set_guides(GUIDES.to_vec(), 7_000));
    assert_eq!(s.persp().mode, Mode::Guided);
    // Guided without guides opens the tool on them.
    s.persp_confirmed(0);
    let mut s2 = session();
    s2.set_persp(&PerspParams::default(), 8_000);
    let r = choose(&mut s2, Mode::Guided, 9_000);
    assert!(r.iter().any(|q| matches!(q, Request::CropGuides)), "{r:?}");
}

/// A keystoned plan of a 6000 × 4000 photo.
fn plan(p: &PerspParams, crop: &CropOrientation) -> Plan {
    let mut e = controls::new_edit(&PathBuf::from("/photos/IMG_0042.JPG"));
    let reg = specular_develop::model::Registry::builtin();
    if e.module(ModuleKind::PERSPECTIVE, 0).is_none() {
        e.insert(
            reg,
            specular_develop::model::ModuleInstance::new(reg, ModuleKind::PERSPECTIVE, 0),
        );
    }
    use specular_develop::model::ModuleParams as _;
    e.module_mut(ModuleKind::PERSPECTIVE, 0).unwrap().params =
        specular_develop::model::Params::Other(ModuleKind::PERSPECTIVE, p.to_fields());
    e.module_mut(ModuleKind::CROP_ORIENTATION, 0)
        .unwrap()
        .params = specular_develop::model::Params::CropOrientation(crop.clone());
    Plan::new(&e, &Context::image(6000, 4000, Orient::default()), false)
}

#[test]
fn the_tool_adds_guides_and_keeps_the_crop_inside() {
    let pp = PerspParams {
        vertical: 60.0,
        auto_scale: false,
        ..PerspParams::default()
    };
    let crop = CropOrientation::default();
    let pl = plan(&pp, &crop);
    let pic = Rect::new(100.0, 50.0, 600.0, 400.0);
    let lay = Layout::new(pic, (6000, 4000), Orient::default(), &crop).with_plan(Some(&pl));
    assert!(lay.keystone.is_some());
    // Arrows keep the crop inside the warped photo.
    let mut t = CropTool::new(0);
    let fr = lay.frame_of(&crop);
    let full = CropOrientation {
        crop: [0.3, 0.3, 0.7, 0.7],
        ..crop.clone()
    };
    let k = specular_input::KeyCombo {
        key: specular_input::Key::Named(specular_input::NamedKey::ArrowUp),
        mods: specular_input::Mods {
            shift: true,
            ..specular_input::Mods::NONE
        },
    };
    let mut p = full;
    for _ in 0..20 {
        if let Some(Outcome::Set(n, _)) = t.key(k, &lay, &p) {
            p = n;
        }
    }
    assert!(fr.crop_inside(p.crop.map(f64::from)), "{:?}", p.crop);
    // Drawing a guide: press, move, release gives the guides with it.
    let mut t = CropTool::new(0).with_guides();
    let map = GuideMap::new(pl, lay.frame, Vec::new());
    let (took, o) = t.guide_press((300.0, 120.0), &map);
    assert!(took && o.is_none());
    t.moved((310.0, 380.0), &lay, &crop);
    let Some(Outcome::Guides(g, _)) = t.release(&lay, &crop, Some(&map)) else {
        panic!("no guide");
    };
    assert_eq!(g.len(), 1);
    // Its ends map back to where they were drawn.
    let a = map.to_screen([g[0][0], g[0][1]]).unwrap();
    assert!(
        (a.0 - 300.0).abs() < 0.01 && (a.1 - 120.0).abs() < 0.01,
        "{a:?}"
    );
    // Delete asks about the guide last touched.
    let del = specular_input::KeyCombo {
        key: specular_input::Key::Named(specular_input::NamedKey::Delete),
        mods: specular_input::Mods::NONE,
    };
    assert_eq!(t.key(del, &lay, &crop), Some(Outcome::RemoveGuide(0)));
    // Alt+Up: more vertical keystone.
    let alt = specular_input::KeyCombo {
        key: specular_input::Key::Named(specular_input::NamedKey::ArrowUp),
        mods: specular_input::Mods {
            alt: true,
            ..specular_input::Mods::NONE
        },
    };
    assert_eq!(t.key(alt, &lay, &crop), Some(Outcome::Keystone(1.0, 0.0)));
}

#[test]
fn perspective_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-persp",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    let size = (1000.0f32, 1100.0f32);
    for (name, lang, rtl) in [("en-dark", en, false), ("he-light", he, true)] {
        h.text.set_locale(Some(lang.language.as_str().to_owned()));
        let pal = if rtl {
            Palette::light()
        } else {
            Palette::dark()
        };
        for (scale, suffix) in [(1.0f32, "1x"), (1.5, "1.5x"), (2.0, "2x")] {
            let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let mut s = session();
                let crop = CropOrientation {
                    angle_deg: 1.0,
                    crop: [0.1, 0.1, 0.9, 0.9],
                    ..Default::default()
                };
                let pp = PerspParams {
                    mode: Mode::Guided,
                    vertical: 35.0,
                    auto_scale: false,
                    guides: GUIDES.to_vec(),
                    ..PerspParams::default()
                };
                let pl = plan(&pp, &crop);
                let window = Rect::new(0.0, 0.0, size.0, size.1);
                let pw = specular_widgets::develop_panel::DevelopPanel::width(size.0, 1.0);
                let image = if rtl {
                    Rect::new(pw, 0.0, size.0 - pw, size.1)
                } else {
                    Rect::new(0.0, 0.0, size.0 - pw, size.1)
                };
                let pic = image.centered(480.0, 320.0);
                let lay =
                    Layout::new(pic, (6000, 4000), Orient::default(), &crop).with_plan(Some(&pl));
                let map = GuideMap::new(pl, lay.frame, GUIDES.to_vec());
                let tool = CropTool::new(0).with_guides();
                h.render(
                    size,
                    scale,
                    |dl: &mut DrawList| {
                        dl.rect(window, 0.0, pal.canvas);
                        // A stand-in photo: sky and a building.
                        let f = lay.frame;
                        dl.rect(f, 0.0, Color::hex(0x6f9fd8));
                        dl.rect(
                            Rect::new(f.x + f.w * 0.18, f.y + f.h * 0.12, f.w * 0.64, f.h * 0.88),
                            0.0,
                            Color::hex(0xb59a7a),
                        );
                    },
                    |dl, text| {
                        tool.draw(dl, text, &pal, &lay, &crop, rtl, 1.0, Some(&map), None);
                        s.panel.draw(dl, text, &pal, window, rtl, 1.0);
                    },
                )
            });
            goldens.check(&format!("develop-persp-{name}-{suffix}"), &shot);
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}
