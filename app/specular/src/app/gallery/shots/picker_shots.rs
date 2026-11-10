use super::*;

/// The search field's value picker open under `camera=X-T5,` (one camera selected, the list
/// waiting for another, a row active): dark at 1× and 1.5×, light, Hebrew right to left, and
/// text at 200 %.
#[test]
fn picker_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let looks = [
        ("dark", Palette::dark(), false, 1.0, SIZE, 1.0, "1x"),
        ("dark", Palette::dark(), false, 1.0, SIZE, 1.5, "1.5x"),
        ("light", Palette::light(), false, 1.0, SIZE, 1.0, "1x"),
        ("he", Palette::light(), true, 1.0, SIZE, 1.0, "1x"),
        (
            "text2",
            Palette::dark(),
            false,
            2.0,
            (720.0, 640.0),
            1.0,
            "1x",
        ),
    ];
    let cameras = [
        "Fujifilm X-T5",
        "Canon EOS R5",
        "iPhone 15 Pro",
        "Sony α7 IV",
    ];
    for (look, pal, rtl, text_scale, size, scale, suffix) in looks {
        let lang = if rtl {
            specular_i18n::langid!("he")
        } else {
            specular_i18n::langid!("en-US")
        };
        specular_i18n::I18n::with_thread_language(lang, || {
            let mut g = gallery();
            let paths: Vec<PathBuf> = g.entries.iter().map(|e| e.path.clone()).collect();
            for (i, p) in paths.into_iter().enumerate() {
                let meta = Meta {
                    camera: Some(cameras[[0, 0, 1, 2, 3, 0, 1][i % 7]].into()),
                    iso: Some(100 << (i % 4)),
                    ..Meta::default()
                };
                g.metas.insert(p.into(), Arc::new(meta));
            }
            g.filter.text = "beach camera=\"Fujifilm X-T5\",".into();
            g.filter_focus = true;
            g.rebuild();
            g.picker.active = Some(1);
            let f = Frame {
                pal: &pal,
                now: 1.0,
                reduce_motion: true,
                size,
                scale,
                rtl,
                text_scale,
                backdrop: Backdrop::None,
            };
            // Frames until the captions are shaped and the counts are in.
            for _ in 0..20 {
                let mut warm = DrawList::new(scale);
                h.text.begin_frame(scale);
                g.draw(&mut warm, &mut h.text, &f);
                if !g.cap_more && !g.facets.counting() && !g.picker.counting {
                    break;
                }
            }
            assert!(!g.facets.counting(), "still counting");
            let shot = h.render(
                size,
                scale,
                |dl| dl.rect(Rect::new(0.0, 0.0, size.0, size.1), 0.0, pal.canvas),
                |dl, text| {
                    g.draw(dl, text, &f);
                },
            );
            assert!(g.picker.rect.is_some(), "the picker is shown");
            goldens.check(&format!("gallery-picker-{look}-{suffix}"), &shot);
            if scale != 1.0 {
                return;
            }
            // The `is:` words, with the caret in the middle of the text (after `is:hdr,`).
            let paths: Vec<PathBuf> = g.entries.iter().map(|e| e.path.clone()).collect();
            for (i, p) in paths.into_iter().enumerate() {
                let meta = Meta {
                    camera: Some(cameras[i % 4].into()),
                    lens: (i % 2 == 0).then(|| "XF 33mm".into()),
                    width: if i % 3 == 0 { 3000 } else { 4000 },
                    height: if i % 3 == 0 { 4000 } else { 3000 },
                    hdr: i % 3 == 1,
                    raw: i % 5 == 0,
                    has_profile: i % 2 == 1,
                    gps: Some(i % 4 == 0),
                    ..Meta::default()
                };
                g.metas.insert(p.into(), Arc::new(meta));
            }
            g.filter.text = "is:hdr, beach".into();
            g.set_filter_caret(7);
            g.picker.active = Some(2);
            g.rebuild();
            for _ in 0..20 {
                let mut warm = DrawList::new(scale);
                h.text.begin_frame(scale);
                g.draw(&mut warm, &mut h.text, &f);
                if !g.cap_more && !g.facets.counting() && !g.picker.counting {
                    break;
                }
            }
            let shot = h.render(
                size,
                scale,
                |dl| dl.rect(Rect::new(0.0, 0.0, size.0, size.1), 0.0, pal.canvas),
                |dl, text| {
                    g.draw(dl, text, &f);
                },
            );
            assert!(g.picker.rect.is_some(), "the picker is shown");
            goldens.check(&format!("gallery-picker-flags-{look}-{suffix}"), &shot);
            // A click at the text's left edge puts the caret before `is:` (the term then
            // being typed is a field name).
            let x0 = g
                .filter_drawn
                .as_ref()
                .map(|d| d.1)
                .expect("the text is drawn");
            g.filter_click(x0 + 0.5);
            assert_eq!(g.filter_caret(), 0);
            // A number under `iso>=`: the operator buttons (≥ on) and each ISO with the count
            // at least it.
            let paths: Vec<PathBuf> = g.entries.iter().map(|e| e.path.clone()).collect();
            for (i, p) in paths.into_iter().enumerate() {
                let meta = Meta {
                    camera: Some(cameras[i % 4].into()),
                    iso: Some(100 << (i % 5)),
                    ..Meta::default()
                };
                g.metas.insert(p.into(), Arc::new(meta));
            }
            g.filter.text = "beach iso>=".into();
            g.set_filter_caret(g.filter.text.len());
            g.picker.active = Some(1);
            g.rebuild();
            for _ in 0..20 {
                let mut warm = DrawList::new(scale);
                h.text.begin_frame(scale);
                g.draw(&mut warm, &mut h.text, &f);
                if !g.cap_more && !g.facets.counting() && !g.picker.counting {
                    break;
                }
            }
            let shot = h.render(
                size,
                scale,
                |dl| dl.rect(Rect::new(0.0, 0.0, size.0, size.1), 0.0, pal.canvas),
                |dl, text| {
                    g.draw(dl, text, &f);
                },
            );
            assert!(g.picker.rect.is_some(), "the picker is shown");
            assert_eq!(g.picker.ops.len(), 3, "the operator buttons are drawn");
            goldens.check(&format!("gallery-picker-numeric-{look}-{suffix}"), &shot);
            if look == "text2" {
                return;
            }
            // The colour spaces under `space=`: proper names, the colour models in the UI
            // language, a profile by its description; files without one are not listed.
            let spaces = [
                "srgb",
                "p3",
                "srgb",
                "ColorMatch RGB",
                "grey",
                "",
                "adobergb",
                "cmyk",
                "pq",
            ];
            let paths: Vec<PathBuf> = g.entries.iter().map(|e| e.path.clone()).collect();
            for (i, p) in paths.into_iter().enumerate() {
                let meta = Meta {
                    camera: Some(cameras[i % 4].into()),
                    space: Some(spaces[i % spaces.len()].into()),
                    ..Meta::default()
                };
                g.metas.insert(p.into(), Arc::new(meta));
            }
            g.filter.text = "beach space=srgb,".into();
            g.set_filter_caret(g.filter.text.len());
            g.picker.active = Some(1);
            g.rebuild();
            for _ in 0..20 {
                let mut warm = DrawList::new(scale);
                h.text.begin_frame(scale);
                g.draw(&mut warm, &mut h.text, &f);
                if !g.cap_more && !g.facets.counting() && !g.picker.counting {
                    break;
                }
            }
            let shot = h.render(
                size,
                scale,
                |dl| dl.rect(Rect::new(0.0, 0.0, size.0, size.1), 0.0, pal.canvas),
                |dl, text| {
                    g.draw(dl, text, &f);
                },
            );
            assert!(g.picker.rect.is_some(), "the picker is shown");
            goldens.check(&format!("gallery-picker-space-{look}-{suffix}"), &shot);
        });
    }
    goldens.finish();
}

/// A selection in the filter field, drawn under the text in the theme's selection colour:
/// Latin text, Hebrew and Latin mixed in an English window, and the same in a Hebrew one
/// (the selection runs across the change of direction, so it is drawn in pieces).
#[test]
fn filter_selection_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let cases = [
        (
            "ltr-dark",
            Palette::dark(),
            false,
            "beach iso:100 sunset",
            2..17,
        ),
        (
            "mixed-dark",
            Palette::dark(),
            false,
            "sunset שלום עולם beach",
            3..18,
        ),
        ("he-light", Palette::light(), true, "חוף beach שקיעה", 2..10),
    ];
    for (name, pal, rtl, text, sel) in cases {
        let lang = if rtl {
            specular_i18n::langid!("he")
        } else {
            specular_i18n::langid!("en-US")
        };
        specular_i18n::I18n::with_thread_language(lang, || {
            let mut g = gallery();
            g.filter.text = text.into();
            g.filter_focus = true;
            g.field().select(sel.clone());
            g.picker_dismiss();
            g.rebuild();
            assert_eq!(g.filter_selection(), sel, "{name}");
            let f = Frame {
                pal: &pal,
                now: 1.0,
                reduce_motion: true,
                size: SIZE,
                scale: 1.0,
                rtl,
                text_scale: 1.0,
                backdrop: Backdrop::None,
            };
            for _ in 0..20 {
                let mut warm = DrawList::new(1.0);
                h.text.begin_frame(1.0);
                g.draw(&mut warm, &mut h.text, &f);
                if !g.cap_more && !g.facets.counting() && !g.picker.counting {
                    break;
                }
            }
            let shot = h.render(
                SIZE,
                1.0,
                |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
                |dl, text| {
                    g.draw(dl, text, &f);
                },
            );
            assert_eq!(g.filter_selection(), sel, "{name}");
            goldens.check(&format!("gallery-filter-selection-{name}-1x"), &shot);
        });
    }
    goldens.finish();
}
