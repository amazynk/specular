//! Screenshot tests of the Effects cards: negative, graduated filter and vignette
//! open with the graduated filter's handles over a stand-in photo; glow, grain and borders
//! with watermark open. English dark left to right and Hebrew light right to left, at 1× and
//! 1.5×. `SPECULAR_BLESS=1 cargo test -p specular --bin specular develop::effects_shots`
//! rewrites the PNGs in `app/specular/tests/shots/`.

use std::path::PathBuf;

use specular_develop::model::{History, ModuleKind, Value};
use specular_gfx::{DrawList, Rect};
use specular_shots::{Goldens, Harness};
use specular_theme::{Color, Palette};
use specular_widgets::develop_panel::Item;
use specular_widgets::mask_handles::MaskHandles;

use super::controls::{self, Session, id};

fn set(s: &mut Session, kind: ModuleKind, name: &str, v: Value) {
    let m = s.edit.module_mut(kind, 0).expect("module");
    let mut f = m.params.to_fields();
    f.set(name, v);
    m.params.set_fields(f);
    m.enabled = true;
}

fn session(page: u8) -> Session {
    let p = PathBuf::from("/photos/IMG_0042.CR3");
    let mut s = Session::new(p.clone(), controls::new_edit(&p), History::default());
    let f = |v: f32| Value::Float(v);
    let open: &[ModuleKind] = if page == 0 {
        set(&mut s, ModuleKind::NEGATIVE, "grade", f(3.0));
        set(&mut s, ModuleKind::GRADUATED_ND, "amount", f(-1.5));
        set(&mut s, ModuleKind::GRADUATED_ND, "angle", f(12.0));
        set(&mut s, ModuleKind::GRADUATED_ND, "tint_amount", f(0.3));
        set(&mut s, ModuleKind::VIGNETTE, "amount", f(-0.8));
        set(&mut s, ModuleKind::VIGNETTE, "protect", f(0.5));
        s.nd_edit = true;
        &[
            ModuleKind::NEGATIVE,
            ModuleKind::GRADUATED_ND,
            ModuleKind::VIGNETTE,
        ]
    } else {
        set(&mut s, ModuleKind::GLOW, "amount", f(0.4));
        set(
            &mut s,
            ModuleKind::GLOW,
            "mode",
            Value::Text("soften".into()),
        );
        set(&mut s, ModuleKind::GRAIN, "amount", f(0.35));
        set(&mut s, ModuleKind::GRAIN, "colour", Value::Bool(true));
        set(&mut s, ModuleKind::FRAME, "border", f(0.03));
        set(&mut s, ModuleKind::FRAME, "aspect", f(1.0));
        set(&mut s, ModuleKind::FRAME, "watermark", Value::Bool(true));
        s.frame_preview = true;
        &[ModuleKind::GLOW, ModuleKind::GRAIN, ModuleKind::FRAME]
    };
    s.sync_panel();
    for c in &mut s.panel.cards {
        c.expanded = open.iter().any(|k| c.id == u32::from(k.0));
    }
    s.panel.history_expanded = false;
    let target = if page == 0 {
        id::ND_ANGLE
    } else {
        id::GRAIN_SIZE
    };
    let (card, control) = s
        .panel
        .cards
        .iter()
        .enumerate()
        .find_map(|(ci, c)| {
            c.controls
                .iter()
                .position(|k| k.id == target)
                .map(|k| (ci, k))
        })
        .expect("control");
    s.panel.set_focus(Some(Item::Control {
        card,
        control,
        part: 0,
    }));
    s
}

#[test]
fn effects_match_goldens() {
    use specular_develop::ops::geometry::{Context, Plan};
    use specular_develop::preview::Orient;

    use super::masks::Map;

    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-effects",
    );
    let size = (1000.0f32, 1000.0f32);
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    for page in 0..2u8 {
        for (name, lang, rtl) in [
            ("en-dark", en.clone(), false),
            ("he-light", he.clone(), true),
        ] {
            h.text.set_locale(Some(lang.language.as_str().to_owned()));
            let pal = if rtl {
                Palette::light()
            } else {
                Palette::dark()
            };
            for (scale, suffix) in [(1.0f32, "1x"), (1.5, "1.5x")] {
                let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                    let mut s = session(page);
                    let window = Rect::new(0.0, 0.0, size.0, size.1);
                    let pw = specular_widgets::develop_panel::DevelopPanel::width(size.0, 1.0);
                    let image = if rtl {
                        Rect::new(pw, 0.0, size.0 - pw, size.1)
                    } else {
                        Rect::new(0.0, 0.0, size.0 - pw, size.1)
                    };
                    let shown = image.centered(480.0, 320.0);
                    let ctx = Context::image(6000, 4000, Orient::default());
                    let map = Map {
                        plan: Plan::new(&s.edit, &ctx, true),
                        shown,
                    };
                    let (hs, ol) = s.handles(&map);
                    let mut handles = MaskHandles::default();
                    handles.set(hs, ol);
                    handles.focus = (page == 0).then_some(1);
                    // Page 1: the square canvas with its white border around the photo.
                    let frame = s.frame_shown().map(|fr| {
                        let l = fr.layout(6000, 4000, None);
                        let f = (shown.w / l.canvas.0 as f32).min(shown.h / l.canvas.1 as f32);
                        let (cw, ch) = (l.canvas.0 as f32 * f, l.canvas.1 as f32 * f);
                        let o = (
                            shown.x + (shown.w - cw) / 2.0,
                            shown.y + (shown.h - ch) / 2.0,
                        );
                        let [x, y, w, hh] = l.image;
                        (
                            Rect::new(o.0, o.1, cw, ch),
                            Rect::new(
                                o.0 + x as f32 * f,
                                o.1 + y as f32 * f,
                                w as f32 * f,
                                hh as f32 * f,
                            ),
                        )
                    });
                    h.render(
                        size,
                        scale,
                        |dl: &mut DrawList| {
                            dl.rect(window, 0.0, pal.canvas);
                            let mut f = shown;
                            if let Some((canvas, photo)) = frame {
                                dl.rect(canvas, 0.0, Color::hex(0xffffff));
                                f = photo;
                            }
                            dl.rect(f, 0.0, Color::hex(0x6f9fd8));
                            dl.rect(
                                Rect::new(f.x, f.y + f.h * 0.55, f.w, f.h * 0.45),
                                0.0,
                                Color::hex(0x4f7a3a),
                            );
                        },
                        |dl, text| {
                            handles.draw(dl, &pal, 1.0);
                            s.panel.draw(dl, text, &pal, window, rtl, 1.0);
                        },
                    )
                });
                goldens.check(&format!("develop-effects-{page}-{name}-{suffix}"), &shot);
            }
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}
