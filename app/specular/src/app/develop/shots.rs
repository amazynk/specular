//! Screenshot tests of Develop's tone controls as the app builds them: Light (auto exposure on,
//! highlights and shadows set), Tone zones closed with its dot (smoothing set), local contrast
//! (texture, clarity) and the curve with its levels, the keyboard on Texture. English dark left to
//! right and Hebrew light right to left, at 1× and 1.5×. Rendered offscreen on lavapipe and
//! compared with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular develop::shots`, then review and
//! commit the rewritten PNGs.

use std::path::PathBuf;

use specular_develop::model::{History, ModuleKind, Value};
use specular_gfx::{DrawList, Rect};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;
use specular_widgets::develop_panel::{Body, Item};

use super::controls::{self, Session, id};

const SIZE: (f32, f32) = (480.0, 1000.0);

fn set(s: &mut Session, kind: ModuleKind, name: &str, v: Value) {
    let m = s.edit.module_mut(kind, 0).expect("module");
    let mut f = m.params.to_fields();
    f.set(name, v);
    m.params.set_fields(f);
}

/// The panel with tone edits, only the tone cards open.
fn session() -> Session {
    let p = PathBuf::from("/photos/IMG_0042.CR3");
    let mut s = Session::new(p.clone(), controls::new_edit(&p), History::default());
    set(&mut s, ModuleKind::EXPOSURE, "auto", Value::Bool(true));
    set(
        &mut s,
        ModuleKind::EXPOSURE,
        "exposure_ev",
        Value::Float(0.3),
    );
    set(
        &mut s,
        ModuleKind::TONE_EQUALIZER,
        "highlights_ev",
        Value::Float(-0.8),
    );
    set(
        &mut s,
        ModuleKind::TONE_EQUALIZER,
        "shadows_ev",
        Value::Float(1.1),
    );
    set(
        &mut s,
        ModuleKind::TONE_EQUALIZER,
        "smoothing",
        Value::Float(0.04),
    );
    set(
        &mut s,
        ModuleKind::LOCAL_CONTRAST,
        "texture",
        Value::Float(0.35),
    );
    set(
        &mut s,
        ModuleKind::LOCAL_CONTRAST,
        "clarity",
        Value::Float(-0.2),
    );
    set(
        &mut s,
        ModuleKind::CURVES,
        "points",
        Value::Floats(vec![0.0, 0.0, 0.3, 0.24, 0.7, 0.78, 1.0, 1.0]),
    );
    set(
        &mut s,
        ModuleKind::CURVES,
        "levels_black",
        Value::Float(0.02),
    );
    set(
        &mut s,
        ModuleKind::CURVES,
        "levels_white",
        Value::Float(0.95),
    );
    s.sync_panel();
    let open = [
        u32::from(ModuleKind::EXPOSURE.0),
        u32::from(ModuleKind::TONE_EQUALIZER.0),
        u32::from(ModuleKind::LOCAL_CONTRAST.0),
        u32::from(ModuleKind::CURVES.0),
    ];
    for c in &mut s.panel.cards {
        c.expanded = open.contains(&c.id);
    }
    s.panel.history_expanded = false;
    let (card, control) = s
        .panel
        .cards
        .iter()
        .enumerate()
        .find_map(|(ci, c)| {
            c.controls
                .iter()
                .position(|k| k.id == id::TEXTURE)
                .map(|k| (ci, k))
        })
        .expect("texture");
    s.panel.set_focus(Some(Item::Control {
        card,
        control,
        part: 0,
    }));
    s
}

/// The panel with detail edits, only the Detail card open, the keyboard on Masking.
fn detail_session() -> Session {
    let p = PathBuf::from("/photos/IMG_0042.CR3");
    let mut s = Session::new(p.clone(), controls::new_edit(&p), History::default());
    for (kind, name, v) in [
        (ModuleKind::DENOISE, "luminance", 0.35),
        (ModuleKind::DENOISE, "colour", 0.5),
        (ModuleKind::SHARPEN, "amount", 0.8),
        (ModuleKind::SHARPEN, "radius", 1.2),
        (ModuleKind::SHARPEN, "masking", 0.4),
        (ModuleKind::CAPTURE_SHARPENING, "amount", 0.6),
        (ModuleKind::RAW_FRONT_END, "dual_threshold", 0.3),
    ] {
        set(&mut s, kind, name, Value::Float(v));
    }
    set(
        &mut s,
        ModuleKind::RAW_FRONT_END,
        "demosaic",
        specular_develop::model::params::Demosaic::Dual.into(),
    );
    s.sync_panel();
    for c in &mut s.panel.cards {
        c.expanded = c.id == controls::DETAIL_CARD;
    }
    s.panel.history_expanded = false;
    let (card, control) = s
        .panel
        .cards
        .iter()
        .enumerate()
        .find_map(|(ci, c)| {
            c.controls
                .iter()
                .position(|k| k.id == id::SHARPEN_MASKING)
                .map(|k| (ci, k))
        })
        .expect("masking");
    s.panel.set_focus(Some(Item::Control {
        card,
        control,
        part: 0,
    }));
    s
}

#[test]
fn detail_controls_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-detail",
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
        for (scale, suffix) in [(1.0f32, "1x"), (1.5, "1.5x")] {
            let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let mut s = detail_session();
                let window = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
                h.render(
                    SIZE,
                    scale,
                    |dl: &mut DrawList| dl.rect(window, 0.0, pal.canvas),
                    |dl, text| s.panel.draw(dl, text, &pal, window, rtl, 1.0),
                )
            });
            goldens.check(&format!("develop-detail-{name}-{suffix}"), &shot);
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}

#[test]
fn tone_controls_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop",
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
        for (scale, suffix) in [(1.0f32, "1x"), (1.5, "1.5x")] {
            let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let mut s = session();
                // Exposure's Auto row reads the edit.
                assert!(matches!(
                    s.panel.control(id::AUTO_EXPOSURE).map(|c| &c.body),
                    Some(Body::Choice(c)) if c.selected == 1
                ));
                let window = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
                h.render(
                    SIZE,
                    scale,
                    |dl: &mut DrawList| dl.rect(window, 0.0, pal.canvas),
                    |dl, text| s.panel.draw(dl, text, &pal, window, rtl, 1.0),
                )
            });
            goldens.check(&format!("develop-tone-{name}-{suffix}"), &shot);
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}

/// The panel with colour edits: the four colour wheels, point colour with a picked
/// colour, black and white on, and the LUT card; those cards open, the keyboard on the
/// LUT space choice.
fn colour_session() -> Session {
    let p = PathBuf::from("/photos/IMG_0042.CR3");
    let mut s = Session::new(p.clone(), controls::new_edit(&p), History::default());
    for (kind, name, v) in [
        (ModuleKind::COLOUR_BALANCE, "vibrance", 0.25),
        (ModuleKind::COLOUR_BALANCE, "brilliance", -0.1),
        (ModuleKind::COLOUR_BALANCE, "shadows_hue", 210.0),
        (ModuleKind::COLOUR_BALANCE, "shadows_chroma", 0.4),
        (ModuleKind::COLOUR_BALANCE, "highlights_hue", 40.0),
        (ModuleKind::COLOUR_BALANCE, "highlights_chroma", 0.3),
        (ModuleKind::COLOUR_BALANCE, "global_luminance", 0.1),
        (ModuleKind::POINT_COLOUR, "target_hue", 50.0),
        (ModuleKind::POINT_COLOUR, "target_saturation", 0.3),
        (ModuleKind::POINT_COLOUR, "shift_hue", -12.0),
        (ModuleKind::POINT_COLOUR, "range", 0.7),
        (ModuleKind::BLACK_WHITE, "mix_red", 0.4),
        (ModuleKind::BLACK_WHITE, "mix_blue", -0.6),
        (ModuleKind::LUT_3D, "strength", 0.8),
    ] {
        set(&mut s, kind, name, Value::Float(v));
    }
    set(
        &mut s,
        ModuleKind::POINT_COLOUR,
        "picked",
        Value::Bool(true),
    );
    set(
        &mut s,
        ModuleKind::LUT_3D,
        "space",
        Value::Text("acescct".into()),
    );
    if let Some(m) = s.edit.module_mut(ModuleKind::BLACK_WHITE, 0) {
        m.enabled = true;
    }
    s.sync_panel();
    let open = [
        u32::from(ModuleKind::COLOUR_BALANCE.0),
        controls::GRADING_CARD,
        u32::from(ModuleKind::POINT_COLOUR.0),
        u32::from(ModuleKind::BLACK_WHITE.0),
        u32::from(ModuleKind::LUT_3D.0),
    ];
    for c in &mut s.panel.cards {
        c.expanded = open.contains(&c.id);
    }
    s.panel.history_expanded = false;
    let (card, control) = s
        .panel
        .cards
        .iter()
        .enumerate()
        .find_map(|(ci, c)| {
            c.controls
                .iter()
                .position(|k| k.id == id::LUT_SPACE)
                .map(|k| (ci, k))
        })
        .expect("LUT space");
    s.panel.set_focus(Some(Item::Control {
        card,
        control,
        part: 0,
    }));
    s
}

#[test]
fn colour_controls_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-colour",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    // Tall enough for every open card.
    let size = (480.0, 2600.0);
    for (name, lang, rtl) in [("en-dark", en, false), ("he-light", he, true)] {
        h.text.set_locale(Some(lang.language.as_str().to_owned()));
        let pal = if rtl {
            Palette::light()
        } else {
            Palette::dark()
        };
        for (scale, suffix) in [(1.0f32, "1x"), (1.5, "1.5x")] {
            let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let mut s = colour_session();
                let window = Rect::new(0.0, 0.0, size.0, size.1);
                h.render(
                    size,
                    scale,
                    |dl: &mut DrawList| dl.rect(window, 0.0, pal.canvas),
                    |dl, text| s.panel.draw(dl, text, &pal, window, rtl, 1.0),
                )
            });
            goldens.check(&format!("develop-colour-{name}-{suffix}"), &shot);
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}

/// The crop tool over a photo (a 3:2 crop of a photo straightened by 4°, thirds) beside the
/// panel with the crop and lens cards open; English dark and Hebrew light.
#[test]
fn crop_overlay_matches_goldens() {
    use specular_develop::model::params::CropOrientation;
    use specular_develop::preview::Orient;
    use specular_theme::Color;

    use super::crop::{CropTool, Layout};

    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop",
    );
    let size = (1000.0f32, 640.0f32);
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    for (name, lang, rtl) in [("en-dark", en, false), ("he-light", he, true)] {
        h.text.set_locale(Some(lang.language.as_str().to_owned()));
        let pal = if rtl {
            Palette::light()
        } else {
            Palette::dark()
        };
        let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
            let p = PathBuf::from("/photos/IMG_0042.ARW");
            let mut s = Session::new(p.clone(), controls::new_edit(&p), History::default());
            let crop = CropOrientation {
                angle_deg: 4.0,
                crop: [0.12, 0.14, 0.86, 0.80],
                aspect: "3:2".into(),
                ..Default::default()
            };
            assert!(s.set_crop(&crop, 0));
            set(&mut s, ModuleKind::LENS, "distortion", Value::Float(12.0));
            s.sync_panel();
            let open = [
                u32::from(ModuleKind::CROP_ORIENTATION.0),
                u32::from(ModuleKind::LENS.0),
            ];
            for c in &mut s.panel.cards {
                c.expanded = open.contains(&c.id);
            }
            s.panel.history_expanded = false;
            // The Crop card's rows in view: the keyboard on its last row (Flip top to bottom).
            let card = s
                .panel
                .cards
                .iter()
                .position(|c| c.id == open[0])
                .expect("the crop card");
            let control = s.panel.cards[card].controls.len() - 1;
            s.panel
                .set_focus(Some(specular_widgets::develop_panel::Item::Control {
                    card,
                    control,
                    part: 0,
                }));
            let window = Rect::new(0.0, 0.0, size.0, size.1);
            let pw = specular_widgets::develop_panel::DevelopPanel::width(size.0, 1.0);
            let image = if rtl {
                Rect::new(pw, 0.0, size.0 - pw, size.1)
            } else {
                Rect::new(0.0, 0.0, size.0 - pw, size.1)
            };
            let pic = image.centered(540.0, 360.0);
            let lay = Layout::new(pic, (6000, 4000), Orient::default(), &crop);
            let tool = CropTool::new(0);
            h.render(
                size,
                1.0,
                |dl: &mut DrawList| {
                    dl.rect(window, 0.0, pal.canvas);
                    // A stand-in photo: sky, hills, ground and the sun.
                    let f = lay.frame;
                    dl.rect(f, 0.0, Color::hex(0x6f9fd8));
                    dl.rect(
                        Rect::new(f.x, f.y + f.h * 0.45, f.w, f.h * 0.2),
                        0.0,
                        Color::hex(0x4f7a3a),
                    );
                    dl.rect(
                        Rect::new(f.x, f.y + f.h * 0.65, f.w, f.h * 0.35),
                        0.0,
                        Color::hex(0x8a6a45),
                    );
                    dl.circle(
                        (f.x + f.w * 0.7, f.y + f.h * 0.22),
                        30.0,
                        Color::hex(0xf2e6a0),
                    );
                },
                |dl, text| {
                    tool.draw(dl, text, &pal, &lay, &s.crop(), rtl, 1.0, None, None);
                    s.panel.draw(dl, text, &pal, window, rtl, 1.0);
                },
            )
        });
        goldens.check(&format!("develop-crop-{name}"), &shot);
    }
    h.text.set_locale(None);
    goldens.finish();
}

/// The panel's Light (its profile: AgX, "Natural") and HDR cards: the HDR limit at
/// 3 EV, the SDR preview on, the keyboard on the HDR limit.
fn hdr_session() -> Session {
    let p = PathBuf::from("/photos/IMG_0042.CR3");
    let mut s = Session::new(p.clone(), controls::new_edit(&p), History::default());
    set(
        &mut s,
        ModuleKind::VIEW_TRANSFORM,
        "method",
        Value::Text("agx".into()),
    );
    set(
        &mut s,
        ModuleKind::VIEW_TRANSFORM,
        "hdr_limit_ev",
        Value::Float(3.0),
    );
    set(
        &mut s,
        ModuleKind::VIEW_TRANSFORM,
        "contrast",
        Value::Float(1.7),
    );
    s.sdr_preview = true;
    s.sync_panel();
    let open = [u32::from(ModuleKind::EXPOSURE.0), controls::HDR_CARD];
    for c in &mut s.panel.cards {
        c.expanded = open.contains(&c.id);
    }
    s.panel.history_expanded = false;
    let (card, control) = s
        .panel
        .cards
        .iter()
        .enumerate()
        .find_map(|(ci, c)| {
            c.controls
                .iter()
                .position(|k| k.id == id::HDR_LIMIT)
                .map(|k| (ci, k))
        })
        .expect("HDR limit");
    s.panel.set_focus(Some(Item::Control {
        card,
        control,
        part: 0,
    }));
    s
}

#[test]
fn view_and_hdr_controls_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-hdr",
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
        for (scale, suffix) in [(1.0f32, "1x"), (1.5, "1.5x")] {
            let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let mut s = hdr_session();
                let window = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
                h.render(
                    SIZE,
                    scale,
                    |dl: &mut DrawList| dl.rect(window, 0.0, pal.canvas),
                    |dl, text| s.panel.draw(dl, text, &pal, window, rtl, 1.0),
                )
            });
            goldens.check(&format!("develop-hdr-{name}-{suffix}"), &shot);
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}

/// The panel with corrections set, only the Optics and Corrections cards open, the keyboard
/// on Defringe.
fn corrections_session() -> Session {
    let p = PathBuf::from("/photos/IMG_0042.CR3");
    let mut s = Session::new(p.clone(), controls::new_edit(&p), History::default());
    set(
        &mut s,
        ModuleKind::RAW_FRONT_END,
        "highlights",
        Value::Text("rebuild".into()),
    );
    for (kind, name, v) in [
        (ModuleKind::RAW_FRONT_END, "highlight_threshold", 0.97),
        (ModuleKind::HOT_PIXELS, "threshold", 1.5),
        (ModuleKind::CHROMATIC_ABERRATION, "red", 0.06),
        (ModuleKind::CHROMATIC_ABERRATION, "blue", -0.04),
        (ModuleKind::DEFRINGE, "amount", 0.6),
    ] {
        set(&mut s, kind, name, Value::Float(v));
    }
    set(
        &mut s,
        ModuleKind::CHROMATIC_ABERRATION,
        "auto",
        Value::Bool(false),
    );
    set(
        &mut s,
        ModuleKind::FLAT_FIELD,
        "flat",
        Value::Text("/calibration/flats/FLAT_0001.CR3".into()),
    );
    for kind in [ModuleKind::CHROMATIC_ABERRATION, ModuleKind::FLAT_FIELD] {
        if let Some(m) = s.edit.module_mut(kind, 0) {
            m.enabled = true;
        }
    }
    s.sync_panel();
    for c in &mut s.panel.cards {
        c.expanded = c.id == controls::CORRECTIONS_CARD || c.id == u32::from(ModuleKind::LENS.0);
    }
    s.panel.history_expanded = false;
    let (card, control) = s
        .panel
        .cards
        .iter()
        .enumerate()
        .find_map(|(ci, c)| {
            c.controls
                .iter()
                .position(|k| k.id == id::DEFRINGE)
                .map(|k| (ci, k))
        })
        .expect("defringe");
    s.panel.set_focus(Some(Item::Control {
        card,
        control,
        part: 0,
    }));
    s
}

#[test]
fn corrections_controls_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-corrections",
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
        for (scale, suffix) in [(1.0f32, "1x"), (1.5, "1.5x")] {
            let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let mut s = corrections_session();
                let window = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
                h.render(
                    SIZE,
                    scale,
                    |dl: &mut DrawList| dl.rect(window, 0.0, pal.canvas),
                    |dl, text| s.panel.draw(dl, text, &pal, window, rtl, 1.0),
                )
            });
            goldens.check(&format!("develop-corrections-{name}-{suffix}"), &shot);
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}

/// The Masks section: exposure masked by an ellipse minus a luminance range, the
/// ellipse's handles over a stand-in photo, the keyboard on the second row.
#[test]
fn masks_match_goldens() {
    use specular_develop::ops::geometry::{Context, Plan};
    use specular_develop::preview::Orient;
    use specular_theme::Color;
    use specular_widgets::mask_handles::MaskHandles;

    use super::masks::{Map, NewKind};

    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-masks",
    );
    let size = (1000.0f32, 900.0f32);
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    for (name, lang, rtl) in [("en-dark", en, false), ("he-light", he, true)] {
        h.text.set_locale(Some(lang.language.as_str().to_owned()));
        let pal = if rtl {
            Palette::light()
        } else {
            Palette::dark()
        };
        for (scale, suffix) in [(1.0f32, "1x"), (1.5, "1.5x")] {
            let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let p = PathBuf::from("/photos/IMG_0042.ARW");
                let mut s = Session::new(p.clone(), controls::new_edit(&p), History::default());
                let k = ModuleKind::EXPOSURE;
                s.add_row(k, NewKind::Ellipse, 0);
                s.add_row(k, NewKind::Luminance, 10_000);
                s.sync_panel();
                for c in &mut s.panel.cards {
                    c.expanded = c.id == u32::from(k.0);
                }
                s.panel.history_expanded = false;
                let window = Rect::new(0.0, 0.0, size.0, size.1);
                let pw = specular_widgets::develop_panel::DevelopPanel::width(size.0, 1.0);
                let image = if rtl {
                    Rect::new(pw, 0.0, size.0 - pw, size.1)
                } else {
                    Rect::new(0.0, 0.0, size.0 - pw, size.1)
                };
                let shown = image.centered(540.0, 360.0);
                let ctx = Context::image(6000, 4000, Orient::default());
                let map = Map {
                    plan: Plan::new(&s.edit, &ctx, true),
                    shown,
                };
                let (hs, ol) = s.handles(&map);
                let mut handles = MaskHandles::default();
                handles.set(hs, ol);
                handles.focus = Some(1);
                h.render(
                    size,
                    scale,
                    |dl: &mut DrawList| {
                        dl.rect(window, 0.0, pal.canvas);
                        let f = shown;
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
            goldens.check(&format!("develop-masks-{name}-{suffix}"), &shot);
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}

/// Masks 2: a polygon with its second point chosen (its own feather), a details
/// row, a borrowed row, edge radius, grow, blur and keep-detail, mask presets.
#[test]
fn mask_tools_match_goldens() {
    use specular_develop::ops::geometry::{Context, Plan};
    use specular_develop::preview::Orient;
    use specular_theme::Color;
    use specular_widgets::mask_handles::MaskHandles;

    use super::masks::{Map, NewKind};

    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-mask-tools",
    );
    let size = (1000.0f32, 1100.0f32);
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    for (name, lang, rtl) in [("en-dark", en, false), ("he-light", he, true)] {
        h.text.set_locale(Some(lang.language.as_str().to_owned()));
        let pal = if rtl {
            Palette::light()
        } else {
            Palette::dark()
        };
        for (scale, suffix) in [(1.0f32, "1x"), (1.5, "1.5x")] {
            let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let p = PathBuf::from("/photos/IMG_0042.ARW");
                let mut s = Session::new(p.clone(), controls::new_edit(&p), History::default());
                let k = ModuleKind::EXPOSURE;
                s.add_row(k, NewKind::Ellipse, 0);
                s.borrow_into_new_instance(k, 5_000);
                s.add_row(k, NewKind::Polygon, 10_000);
                s.add_row(k, NewKind::Details, 20_000);
                s.masks.preset_names = vec!["Sky".into(), "Face".into()];
                s.set_refine_for_shots(k);
                s.choose_handle(Some(2));
                for c in &mut s.panel.cards {
                    c.expanded = c.id == u32::from(k.0);
                }
                s.panel.history_expanded = false;
                let window = Rect::new(0.0, 0.0, size.0, size.1);
                let pw = specular_widgets::develop_panel::DevelopPanel::width(size.0, 1.0);
                let image = if rtl {
                    Rect::new(pw, 0.0, size.0 - pw, size.1)
                } else {
                    Rect::new(0.0, 0.0, size.0 - pw, size.1)
                };
                let shown = image.centered(540.0, 360.0);
                let ctx = Context::image(6000, 4000, Orient::default());
                let map = Map {
                    plan: Plan::new(&s.edit, &ctx, true),
                    shown,
                };
                let (hs, ol) = s.handles(&map);
                let mut handles = MaskHandles::default();
                handles.set(hs, ol);
                handles.focus = Some(2);
                // Only the Exposure card, laid out once, then scrolled to its end: the mask
                // rows and the chosen polygon point's own feather show.
                s.panel.cards.retain(|c| c.id == u32::from(k.0));
                h.render(
                    size,
                    scale,
                    |_: &mut DrawList| {},
                    |dl, text| s.panel.draw(dl, text, &pal, window, rtl, 1.0),
                );
                let edge = if rtl { 10.0 } else { size.0 - 10.0 };
                s.panel.wheel((edge, size.1 / 2.0), -1000.0);
                h.render(
                    size,
                    scale,
                    |dl: &mut DrawList| {
                        dl.rect(window, 0.0, pal.canvas);
                        dl.rect(shown, 0.0, Color::hex(0x8a7a62));
                    },
                    |dl, text| {
                        handles.draw(dl, &pal, 1.0);
                        s.panel.draw(dl, text, &pal, window, rtl, 1.0);
                    },
                )
            });
            goldens.check(&format!("develop-mask-tools-{name}-{suffix}"), &shot);
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}

/// The Retouch card: three spots (heal, clone on tones, a brushed blur), the
/// second selected with its handles over a stand-in photo.
#[test]
fn retouch_matches_goldens() {
    use specular_develop::model::Params;
    use specular_develop::ops::geometry::{Context, Plan};
    use specular_develop::ops::retouch::{Layer, Retouch, Spot, SpotMode};
    use specular_develop::preview::Orient;
    use specular_theme::Color;
    use specular_widgets::mask_handles::MaskHandles;

    use super::masks::Map;

    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-retouch",
    );
    let size = (1000.0f32, 900.0f32);
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    for (name, lang, rtl) in [("en-dark", en, false), ("he-light", he, true)] {
        h.text.set_locale(Some(lang.language.as_str().to_owned()));
        let pal = if rtl {
            Palette::light()
        } else {
            Palette::dark()
        };
        for (scale, suffix) in [(1.0f32, "1x"), (1.5, "1.5x")] {
            let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let p = PathBuf::from("/photos/IMG_0042.ARW");
                let mut s = Session::new(p.clone(), controls::new_edit(&p), History::default());
                let r = Retouch {
                    split_radius: 4.0,
                    spots: vec![
                        Spot {
                            target: [0.3, 0.35],
                            source: [0.42, 0.3],
                            size: 0.03,
                            ..Spot::default()
                        },
                        Spot {
                            mode: SpotMode::Clone,
                            layer: Layer::Low,
                            target: [0.6, 0.7],
                            source: [0.75, 0.62],
                            size: 0.04,
                            ..Spot::default()
                        },
                        Spot {
                            mode: SpotMode::Blur,
                            target: [0.5, 0.2],
                            source: [0.5, 0.2],
                            size: 0.015,
                            path: vec![[0.42, 0.18], [0.5, 0.22], [0.58, 0.19]],
                            ..Spot::default()
                        },
                    ],
                };
                let reg = specular_develop::model::Registry::builtin();
                let mut m =
                    specular_develop::model::ModuleInstance::new(reg, ModuleKind::RETOUCH, 0);
                m.params = Params::from_fields(ModuleKind::RETOUCH, r.to_fields());
                s.edit.insert(reg, m);
                s.retouch_state.selected = Some(1);
                s.sync_panel();
                for c in &mut s.panel.cards {
                    c.expanded = c.id == u32::from(ModuleKind::RETOUCH.0);
                }
                s.panel.history_expanded = false;
                let window = Rect::new(0.0, 0.0, size.0, size.1);
                let pw = specular_widgets::develop_panel::DevelopPanel::width(size.0, 1.0);
                let image = if rtl {
                    Rect::new(pw, 0.0, size.0 - pw, size.1)
                } else {
                    Rect::new(0.0, 0.0, size.0 - pw, size.1)
                };
                let shown = image.centered(540.0, 360.0);
                let ctx = Context::image(6000, 4000, Orient::default());
                let map = Map {
                    plan: Plan::new(&s.edit, &ctx, true),
                    shown,
                };
                let (hs, ol) = s.retouch_handles(&map);
                let mut handles = MaskHandles::default();
                handles.set(hs, ol);
                handles.focus = Some(2);
                h.render(
                    size,
                    scale,
                    |dl: &mut DrawList| {
                        dl.rect(window, 0.0, pal.canvas);
                        let f = shown;
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
            goldens.check(&format!("develop-retouch-{name}-{suffix}"), &shot);
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}

/// AI masks: a Subject row whose add-on is missing (with its link), a Sky row being
/// worked out, an Object row with two clicks and a box, edited (its points on the picture).
#[test]
fn ai_masks_match_goldens() {
    use specular_develop::mask::ai;
    use specular_develop::model::mask::MaskExpr;
    use specular_develop::ops::geometry::{Context, Plan};
    use specular_develop::preview::Orient;
    use specular_theme::Color;
    use specular_widgets::mask_handles::MaskHandles;

    use super::ai_masks::RowState;
    use super::masks::{Map, NewKind};

    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-ai-masks",
    );
    let size = (1000.0f32, 1000.0f32);
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    for (name, lang, rtl) in [("en-dark", en, false), ("he-light", he, true)] {
        h.text.set_locale(Some(lang.language.as_str().to_owned()));
        let pal = if rtl {
            Palette::light()
        } else {
            Palette::dark()
        };
        for (scale, suffix) in [(1.0f32, "1x"), (1.5, "1.5x")] {
            let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let p = PathBuf::from("/photos/IMG_0042.ARW");
                let mut s = Session::new(p.clone(), controls::new_edit(&p), History::default());
                s.ai.photo = Some([0x42; 32]);
                s.ai.models = [None, None, Some("efficientsam-ti".into())];
                let k = ModuleKind::EXPOSURE;
                s.add_row(k, NewKind::Subject, 0);
                s.add_row(k, NewKind::Sky, 1_000);
                s.add_row(k, NewKind::Object, 2_000);
                let window = Rect::new(0.0, 0.0, size.0, size.1);
                let pw = specular_widgets::develop_panel::DevelopPanel::width(size.0, 1.0);
                let image = if rtl {
                    Rect::new(pw, 0.0, size.0 - pw, size.1)
                } else {
                    Rect::new(0.0, 0.0, size.0 - pw, size.1)
                };
                let shown = image.centered(540.0, 360.0);
                let ctx = Context::image(6000, 4000, Orient::default());
                let map = Map {
                    plan: Plan::new(&s.edit, &ctx, true),
                    shown,
                };
                let at = |x: f32, y: f32| (shown.x + x * shown.w, shown.y + y * shown.h);
                s.ai_add(&map, at(0.3, 0.3), at(0.7, 0.8), true, 3_000);
                s.ai_add(&map, at(0.5, 0.5), at(0.5, 0.5), true, 4_000);
                s.ai_add(&map, at(0.62, 0.35), at(0.62, 0.35), false, 5_000);
                let rows = ai::rows(&s.edit);
                let mut state = std::collections::HashMap::new();
                for m in &rows {
                    let st = match m.kind {
                        specular_develop::model::mask::AiKind::Subject => {
                            RowState::Missing("IS-Net".into())
                        }
                        specular_develop::model::mask::AiKind::Sky => RowState::Computing,
                        specular_develop::model::mask::AiKind::Object => RowState::Ready,
                    };
                    state.insert(m.key, st);
                }
                assert_eq!(rows.len(), 3);
                assert!(matches!(&rows[2], m if m.points.len() == 2 && m.rect.is_some()));
                assert!(
                    s.edit
                        .modules
                        .iter()
                        .filter_map(|m| m.blend.as_ref())
                        .any(|b| matches!(&b.mask, MaskExpr::Combine(..)))
                );
                s.ai.state = state;
                s.sync_panel();
                for c in &mut s.panel.cards {
                    c.expanded = c.id == u32::from(k.0);
                }
                s.panel.history_expanded = false;
                // Scrolled down to the AI rows (the last control in view).
                let card = s
                    .panel
                    .cards
                    .iter()
                    .position(|c| c.id == u32::from(k.0))
                    .expect("exposure card");
                let last = s.panel.cards[card].controls.len() - 1;
                s.panel.set_focus(Some(Item::Control {
                    card,
                    control: last,
                    part: 0,
                }));
                let (hs, ol) = s.handles(&map);
                assert_eq!(hs.len(), 2);
                let mut handles = MaskHandles::default();
                handles.set(hs, ol);
                h.render(
                    size,
                    scale,
                    |dl: &mut DrawList| {
                        dl.rect(window, 0.0, pal.canvas);
                        dl.rect(shown, 0.0, Color::hex(0x6a8aa2));
                    },
                    |dl, text| {
                        handles.draw(dl, &pal, 1.0);
                        s.panel.draw(dl, text, &pal, window, rtl, 1.0);
                    },
                )
            });
            goldens.check(&format!("develop-ai-masks-{name}-{suffix}"), &shot);
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}
