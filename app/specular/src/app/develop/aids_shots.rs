//! Screenshot tests of Develop's darkroom aids: the scope dock with each scope
//! (histogram, waveform luma and RGB, parade, vectorscope with the proof profile's marks)
//! over the Clipping and Soft proof cards, and the clipping overlays over a picture. English
//! dark left to right and Hebrew light right to left, at 1× and 1.5×.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular develop::aids_shots`, then review
//! and commit the rewritten PNGs.

use std::path::PathBuf;

use specular_develop::clip::{self, ClipFlags, ClipParams};
use specular_develop::model::History;
use specular_develop::proof::{ProofSettings, Proofer, synthetic};
use specular_develop::scopes::{self, Bins};
use specular_gfx::{DrawList, Rect};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;
use specular_widgets::develop_panel::DevelopPanel;
use specular_widgets::scopes::{Marks, Scope, ScopeDock};

use super::aids::{self, Aids, CLIP_CARD, PROOF_CARD};
use super::controls::{self, Session};

/// A stand-in photo, display-linear Rec.709: sky, hills, a red sail, skin, a bright sun, a
/// deep shadow and a saturated green beyond the screen.
fn photo(w: u32, h: u32) -> Vec<f32> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
            let hill = 0.62 + 0.08 * (fx * 9.0).sin();
            let mut c = if fy < hill {
                [0.25 + 0.3 * fy, 0.45 + 0.25 * fy, 0.85 - 0.2 * fy]
            } else {
                [0.10 + 0.1 * fx, 0.25 + 0.15 * (1.0 - fy), 0.06]
            };
            let d = |cx: f32, cy: f32| ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
            if d(0.8, 0.18) < 0.07 {
                c = [1.0, 1.0, 0.97];
            }
            if d(0.3, 0.48) < 0.09 {
                c = [0.62, 0.38, 0.29];
            }
            if fx > 0.5 && fx < 0.6 && fy > 0.35 && fy < 0.55 {
                c = [0.85, 0.06, 0.05];
            }
            if fx < 0.12 && fy > 0.85 {
                c = [0.0, 0.0, 0.0];
            }
            if fx > 0.88 && fy > 0.8 {
                c = [0.05, 0.75, 0.02];
            }
            v.extend_from_slice(&[c[0], c[1], c[2], 1.0]);
        }
    }
    v
}

fn encode8(px: &[f32]) -> Vec<u8> {
    px.chunks_exact(4)
        .flat_map(|p| {
            [
                (scopes::encode(p[0]) * 255.0).round() as u8,
                (scopes::encode(p[1]) * 255.0).round() as u8,
                (scopes::encode(p[2]) * 255.0).round() as u8,
                255,
            ]
        })
        .collect()
}

fn session(a: &Aids) -> Session {
    let p = PathBuf::from("/photos/IMG_0042.CR3");
    let mut s = Session::new(p.clone(), controls::new_edit(&p), History::default());
    aids::place_cards(&mut s.panel, a);
    // Only the two aids cards, open, so they show under the scopes.
    s.panel
        .cards
        .retain(|c| c.id == CLIP_CARD || c.id == PROOF_CARD);
    for c in &mut s.panel.cards {
        c.expanded = true;
    }
    s.panel.show_history = false;
    s
}

fn aids_state() -> Aids {
    let mut a = Aids::default();
    a.dock = ScopeDock::new();
    a.clip = ClipFlags {
        scene: false,
        display: true,
        gamut: true,
    };
    a.proof.on = true;
    a.proof.settings = ProofSettings {
        bpc: true,
        ..Default::default()
    };
    a.proof.found = Some(vec![specular_develop::proof::FoundProfile {
        path: "/usr/share/color/icc/test-printer.icc".into(),
        name: synthetic::DESCRIPTION.into(),
        printer: true,
    }]);
    a
}

#[test]
fn scopes_and_proof_controls_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-aids",
    );
    let size = (760.0f32, 1000.0f32);
    let (w, hgt) = (900u32, 600u32);
    let bins: Bins = scopes::cpu(&photo(w, hgt), w, hgt, 1.0);
    let proof = Proofer::new(&synthetic::icc(), ProofSettings::default()).unwrap();
    let proof_marks = proof.targets().map(|t| t.ab);
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    for (scope, rgb, tag) in [
        (Scope::Histogram, false, "histogram"),
        (Scope::Waveform, false, "waveform"),
        (Scope::Waveform, true, "waveform-rgb"),
        (Scope::Parade, false, "parade"),
        (Scope::Vectorscope, false, "vectorscope"),
    ] {
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
                let pw = DevelopPanel::width(size.0, 1.0);
                let mut dock = ScopeDock::new();
                dock.scope = scope;
                dock.rgb = rgb;
                let (sw, sh) = dock.picture_size(pw, 1.0);
                let px = ((sw * scale).round() as u32, (sh * scale).round() as u32);
                let img = match scope {
                    Scope::Histogram => scopes::raster::histogram(&bins, px.0, px.1, dock.log),
                    Scope::Waveform => scopes::raster::waveform(&bins, px.0, px.1, rgb),
                    Scope::Parade => scopes::raster::parade(&bins, px.0, px.1),
                    Scope::Vectorscope => scopes::raster::vectorscope(&bins, px.1),
                };
                let (iw, ih) = if scope == Scope::Vectorscope {
                    (px.1, px.1)
                } else {
                    px
                };
                let (_tex, id) = super::preview::upload(&h.gpu, &mut h.ui, iw, ih, &img);
                let marks = Marks {
                    gamut: scopes::targets().ab,
                    proof: proof_marks,
                    skin: scopes::skin_hue(),
                    range: scopes::VEC_RANGE,
                };
                let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                    let a = aids_state();
                    let mut s = session(&a);
                    let window = Rect::new(0.0, 0.0, size.0, size.1);
                    h.render(
                        size,
                        scale,
                        |dl: &mut DrawList| dl.rect(window, 0.0, pal.canvas),
                        |dl, text| {
                            s.panel.dock = dock.height(text, pw, 1.0);
                            s.panel.draw(dl, text, &pal, window, rtl, 1.0);
                            let area = s.panel.dock_rect();
                            dock.draw(dl, text, &pal, area, Some(id), &marks, rtl, 1.0);
                        },
                    )
                });
                goldens.check(&format!("develop-aids-{tag}-{name}-{suffix}"), &shot);
                h.ui.unregister_texture(id);
            }
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}

/// The Clipping and Soft proof cards alone (the panel scrolled to them).
#[test]
fn aids_cards_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-aids",
    );
    let size = (760.0f32, 640.0f32);
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
                let a = aids_state();
                let mut s = session(&a);
                let window = Rect::new(0.0, 0.0, size.0, size.1);
                h.render(
                    size,
                    scale,
                    |dl: &mut DrawList| dl.rect(window, 0.0, pal.canvas),
                    |dl, text| s.panel.draw(dl, text, &pal, window, rtl, 1.0),
                )
            });
            goldens.check(&format!("develop-aids-cards-{name}-{suffix}"), &shot);
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}

/// The clipping overlays over the stand-in photo (display, scene and gamut on), with the
/// panel beside it.
#[test]
fn clipping_overlays_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-aids",
    );
    let size = (1000.0f32, 640.0f32);
    let (w, hgt) = (540u32, 360u32);
    let main = photo(w, hgt);
    // The scene before the look: the photo brighter, so the sun and the sky's top are over
    // the look's white; display-referred Rec.2020: the green outside Rec.709.
    let scene: Vec<f32> = main.iter().map(|v| v * 4.0).collect();
    let to2020 = specular_develop::colour::rec709_to_rec2020();
    let disp: Vec<f32> = main
        .chunks_exact(4)
        .flat_map(|p| {
            let mut v = specular_develop::colour::mulv(
                &to2020,
                [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])],
            )
            .map(|x| x as f32);
            if p[1] > 0.7 && p[0] < 0.1 {
                v = [0.0, 0.8, 0.0];
            }
            [v[0], v[1], v[2], 1.0]
        })
        .collect();
    let from = specular_develop::colour::rec2020_to_rec709();
    let p = ClipParams {
        scene_white: 3.6,
        scene_black: 0.0005,
        peak: 1.0,
        proof_gamut: false,
        to_output: std::array::from_fn(|i| from[i / 3][i % 3] as f32),
    };
    let all = ClipFlags {
        scene: true,
        display: true,
        gamut: true,
    };
    let overlay = clip::cpu(&main, Some(&scene), Some(&disp), &p, all);
    let (_t1, pic_id) = super::preview::upload(&h.gpu, &mut h.ui, w, hgt, &encode8(&main));
    let (_t2, ov_id) = super::preview::upload(&h.gpu, &mut h.ui, w, hgt, &overlay);
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
                let mut a = aids_state();
                a.clip = all;
                let mut s = session(&a);
                let window = Rect::new(0.0, 0.0, size.0, size.1);
                let pw = DevelopPanel::width(size.0, 1.0);
                let image = if rtl {
                    Rect::new(pw, 0.0, size.0 - pw, size.1)
                } else {
                    Rect::new(0.0, 0.0, size.0 - pw, size.1)
                };
                let pic = image.centered(w as f32, hgt as f32);
                h.render(
                    size,
                    scale,
                    |dl: &mut DrawList| dl.rect(window, 0.0, pal.canvas),
                    |dl, text| {
                        dl.image(pic, [0.0, 0.0, 1.0, 1.0], pic_id, 0.0);
                        dl.image(pic, [0.0, 0.0, 1.0, 1.0], ov_id, 0.0);
                        s.panel.draw(dl, text, &pal, window, rtl, 1.0);
                    },
                )
            });
            goldens.check(&format!("develop-aids-clipping-{name}-{suffix}"), &shot);
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}

/// A merge's ghost mask over a cropped and straightened picture, with the Clipping
/// card's "Show ghosts" row on.
#[test]
fn ghost_overlay_matches_goldens() {
    use specular_develop::mask::Geometry;
    use specular_develop::merge::GhostMask;
    use specular_develop::model::params::CropOrientation;
    use specular_develop::model::{Edit, ModuleKind, Params, Registry};
    use specular_develop::ops::geometry::{Context, Plan};
    use specular_develop::preview::Orient;
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-aids",
    );
    let size = (1000.0f32, 640.0f32);
    // The merged photo, its mask on 12 px blocks: the red sail moved between frames.
    let (fw, fh) = (720u32, 480u32);
    let photo_px = photo(fw, fh);
    let block = 12u32;
    let (gw, gh) = (fw.div_ceil(block), fh.div_ceil(block));
    let mut mask = vec![0u8; (gw * gh) as usize];
    for gy in 0..gh {
        for gx in 0..gw {
            let (fx, fy) = (
                (gx as f32 + 0.5) * block as f32 / fw as f32,
                (gy as f32 + 0.5) * block as f32 / fh as f32,
            );
            if fx > 0.46 && fx < 0.66 && fy > 0.3 && fy < 0.6 {
                mask[(gy * gw + gx) as usize] = 255;
            } else if fx > 0.43 && fx < 0.69 && fy > 0.27 && fy < 0.63 {
                mask[(gy * gw + gx) as usize] = 110;
            }
        }
    }
    let ghosts = GhostMask {
        width: gw,
        height: gh,
        block,
        mask,
    };
    // Cropped and straightened.
    let mut e = Edit::default_for(Registry::builtin(), Default::default(), true);
    if let Some(m) = e.module_mut(ModuleKind::CROP_ORIENTATION, 0) {
        m.params = Params::CropOrientation(CropOrientation {
            angle_deg: 6.0,
            crop: [0.12, 0.1, 0.92, 0.9],
            ..CropOrientation::default()
        });
    }
    let plan = Plan::new(&e, &Context::image(fw, fh, Orient::default()), true);
    let c = plan.crop_rect();
    let (w, hgt) = (c[2].round() as u32, c[3].round() as u32);
    // The picture through the warp (nearest photo pixel).
    let g = Geometry::Warp(&plan);
    let mut pic = vec![0f32; (w * hgt * 4) as usize];
    for y in 0..hgt {
        for x in 0..w {
            let f = g.frame_point(
                (f64::from(x) + 0.5) / f64::from(w),
                (f64::from(y) + 0.5) / f64::from(hgt),
            );
            let sx = ((f.0 * f64::from(fw)) as u32).min(fw - 1);
            let sy = ((f.1 * f64::from(fh)) as u32).min(fh - 1);
            let i = ((y * w + x) * 4) as usize;
            let j = ((sy * fw + sx) * 4) as usize;
            pic[i..i + 4].copy_from_slice(&photo_px[j..j + 4]);
        }
    }
    let overlay = super::ghosts::overlay(&ghosts, (fw, fh), &plan, w, hgt);
    let (_t1, pic_id) = super::preview::upload(&h.gpu, &mut h.ui, w, hgt, &encode8(&pic));
    let (_t2, ov_id) = super::preview::upload(&h.gpu, &mut h.ui, w, hgt, &overlay);
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
                let mut a = aids_state();
                a.clip = ClipFlags::default();
                let mut gh = super::ghosts::Ghosts::new(ghosts.clone(), (fw, fh));
                gh.on = true;
                a.ghosts = Some(gh);
                let mut s = session(&a);
                s.panel.cards.retain(|c| c.id == CLIP_CARD);
                let window = Rect::new(0.0, 0.0, size.0, size.1);
                let pw = DevelopPanel::width(size.0, 1.0);
                let image = if rtl {
                    Rect::new(pw, 0.0, size.0 - pw, size.1)
                } else {
                    Rect::new(0.0, 0.0, size.0 - pw, size.1)
                };
                let shown = image.centered(w as f32, hgt as f32);
                h.render(
                    size,
                    scale,
                    |dl: &mut DrawList| dl.rect(window, 0.0, pal.canvas),
                    |dl, text| {
                        dl.image(shown, [0.0, 0.0, 1.0, 1.0], pic_id, 0.0);
                        dl.image(shown, [0.0, 0.0, 1.0, 1.0], ov_id, 0.0);
                        s.panel.draw(dl, text, &pal, window, rtl, 1.0);
                    },
                )
            });
            goldens.check(&format!("develop-aids-ghosts-{name}-{suffix}"), &shot);
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}
