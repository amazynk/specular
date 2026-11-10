use super::*;

/// The similar-shots review comparing two frames of a burst, zoomed to 100 % (Z) about their
/// centre: each pane through the image renderer, its caption saying 100 %; then the same in
/// the red channel.
#[test]
fn similar_review_zoom_matches_goldens() {
    use specular_similar::Item;
    use specular_similar::scan::Unit;
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let (w, hh) = (800, 600);
    let n = 3;
    let paths: Vec<PathBuf> = (1..=n)
        .map(|k| PathBuf::from(format!("/shots/burst-{k}.jpg")))
        .collect();
    let units: Vec<Unit> = paths
        .iter()
        .map(|p| Unit {
            main: p.clone(),
            files: vec![p.clone()],
            sig_path: p.clone(),
        })
        .collect();
    let items: Vec<Item> = paths
        .iter()
        .map(|p| Item {
            name: p.file_name().unwrap().to_string_lossy().into_owned(),
            width: w,
            height: hh,
            bit_depth: 8,
            bytes: 2_000_000,
            len: 2_000_000,
            ..Item::default()
        })
        .collect();
    let images: Vec<_> = (0..n)
        .map(|k| {
            let mut img = renderer.create_image(&h.gpu, k as u64 + 1, w, hh, None);
            renderer.upload_overview(&h.gpu, &mut img, &burst(k as u32, w, hh));
            img
        })
        .collect();
    let pal = Palette::dark();
    for (scale, suffix) in [(1.0, "1x"), (1.5, "1.5x")] {
        for (channel, view) in [
            (ChannelView::Normal, "zoom"),
            (ChannelView::Red, "zoom-red"),
        ] {
            let mut g = gallery();
            g.filter_open = false;
            g.similar = similar::Similar::for_shots(
                units.clone(),
                items.clone(),
                vec![0, 1],
                SIZE,
                scale,
                (false, 1.0),
            );
            let rp = g.similar.review_panes(SIZE.0, SIZE.1).unwrap();
            assert!(rp.zoom > 1.0 && rp.members.len() == 2);
            let ks: Vec<usize> = rp
                .members
                .iter()
                .map(|(sig, _, _)| paths.iter().position(|p| p == sig).unwrap())
                .collect();
            let panes: Vec<QuickPane> = rp
                .members
                .iter()
                .zip(&ks)
                .map(|((sig, _, cell), &k)| {
                    let mut pane = QuickPane::new(
                        sig.clone(),
                        k as u64 + 1,
                        (w, hh),
                        Orientation::NORMAL,
                        *cell,
                        scale,
                    );
                    pane.zoom = rp.zoom;
                    pane.at = rp.at;
                    pane.channel = channel;
                    pane.opts = RenderOptions {
                        hq: true,
                        canvas: [0.0; 4],
                        checker_a: [0.0; 4],
                        checker_b: [0.0; 4],
                        checker_cell: 1.0e6,
                        channel,
                        ..RenderOptions::default()
                    };
                    pane
                })
                .collect();
            let plan = QuickPlan {
                panes,
                diffs: Vec::new(),
            };
            g.prepare_panes(&h.gpu, &mut h.ui, Some(&plan));
            let mut enc = h.gpu.device.create_command_encoder(&Default::default());
            for ((pane, t), &k) in plan.panes.iter().zip(&g.quick_targets).zip(&ks) {
                render_pane(&mut renderer, &h.gpu, &mut enc, t, &images[k], pane);
            }
            h.gpu.queue.submit([enc.finish()]);
            let f = Frame {
                pal: &pal,
                now: 1.0,
                reduce_motion: true,
                size: SIZE,
                scale,
                rtl: false,
                text_scale: 1.0,
                backdrop: Backdrop::None,
            };
            assert_eq!(g.channel_shown(), channel);
            let shot = h.render(
                SIZE,
                scale,
                |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
                |dl, text| {
                    g.draw(dl, text, &f);
                },
            );
            goldens.check(&format!("gallery-similar-{view}-{suffix}"), &shot);
            crate::app::a11y_audit::audit_screen(
                &format!("gallery-similar-{view}-{suffix}"),
                SIZE,
                f.rtl,
                |t| g.a11y(t, 1.0),
            );
            g.prepare_panes(&h.gpu, &mut h.ui, None);
        }
    }
    goldens.finish();
}

/// The group list while the focus pass refines best-pick: the groups with their picks, and
/// "Checking sharpness… 7 of 12" in the header.
#[test]
fn similar_sharpness_progress_matches_goldens() {
    use specular_similar::Item;
    use specular_similar::scan::Unit;
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let paths: Vec<PathBuf> = (1..=3)
        .map(|k| PathBuf::from(format!("/shots/burst-{k}.jpg")))
        .collect();
    let units: Vec<Unit> = paths
        .iter()
        .map(|p| Unit {
            main: p.clone(),
            files: vec![p.clone()],
            sig_path: p.clone(),
        })
        .collect();
    let items: Vec<Item> = paths
        .iter()
        .map(|p| Item {
            name: p.file_name().unwrap().to_string_lossy().into_owned(),
            width: 800,
            height: 600,
            bit_depth: 8,
            bytes: 2_000_000,
            len: 2_000_000,
            ..Item::default()
        })
        .collect();
    let pal = Palette::dark();
    // A narrow window too: the summary is cut, the progress stays whole.
    for (size, name) in [(SIZE, "sharpness"), ((380.0, 300.0), "sharpness-narrow")] {
        for (scale, suffix) in [(1.0, "1x"), (1.5, "1.5x")] {
            let mut g = gallery();
            g.filter_open = false;
            g.similar = similar::Similar::for_shots_list(units.clone(), items.clone(), 7, 12);
            let f = Frame {
                pal: &pal,
                now: 1.0,
                reduce_motion: true,
                size,
                scale,
                rtl: false,
                text_scale: 1.0,
                backdrop: Backdrop::None,
            };
            let shot = h.render(
                size,
                scale,
                |dl| dl.rect(Rect::new(0.0, 0.0, size.0, size.1), 0.0, pal.canvas),
                |dl, text| {
                    g.draw(dl, text, &f);
                },
            );
            goldens.check(&format!("gallery-similar-{name}-{suffix}"), &shot);
            crate::app::a11y_audit::audit_screen(
                &format!("gallery-similar-{name}-{suffix}"),
                SIZE,
                f.rtl,
                |t| g.a11y(t, 1.0),
            );
        }
    }
    goldens.finish();
}

/// The similar-shots view in Hebrew (mirrored: tabs, Done, the slider with strict on the
/// right, the rows' thumbnails and text, the review's images first on the right, the strip
/// and the footer) and at text scale 2 in English (rows, header, slider, strip and footer
/// grown with the text): the group list while best-pick is refined, and the review of the
/// group (placeholders, no thumbnails). Text scale 2 in a taller window: the review's large
/// images need about 600 px under doubled text.
#[test]
fn similar_rtl_and_text_scale_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let (units, items) = burst_fixture();
    let looks = [
        ("he", Palette::light(), true, 1.0, SIZE),
        ("text2", Palette::dark(), false, 2.0, (720.0, 640.0)),
    ];
    for (look, pal, rtl, text_scale, size) in looks {
        let lang = if rtl {
            specular_i18n::langid!("he")
        } else {
            specular_i18n::langid!("en-US")
        };
        specular_i18n::I18n::with_thread_language(lang, || {
            for view in ["list", "review"] {
                for (scale, suffix) in [(1.0, "1x"), (1.5, "1.5x")] {
                    let mut g = gallery();
                    g.filter_open = false;
                    g.similar = if view == "list" {
                        similar::Similar::for_shots_list(units.clone(), items.clone(), 7, 12)
                    } else {
                        similar::Similar::for_shots_review(units.clone(), items.clone())
                    };
                    // Part way, so the fill shows which end is strict.
                    g.similar.set_sensitivity_for_shots(0.35);
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
                    let shot = h.render(
                        size,
                        scale,
                        |dl| dl.rect(Rect::new(0.0, 0.0, size.0, size.1), 0.0, pal.canvas),
                        |dl, text| {
                            g.draw(dl, text, &f);
                        },
                    );
                    goldens.check(&format!("gallery-similar-{look}-{view}-{suffix}"), &shot);
                    crate::app::a11y_audit::audit_screen(
                        &format!("gallery-similar-{look}-{view}-{suffix}"),
                        SIZE,
                        f.rtl,
                        |t| g.a11y(t, 1.0),
                    );
                }
            }
        });
    }
    goldens.finish();
}

/// The review at text scale 2 in a short window (720 × 560): compact (a smaller strip
/// without its lines, a tighter footer) with every footer button on two rows, their keys
/// left out; in English with the pointer on Compare (its key in a tooltip) and in Hebrew
/// (mirrored).
#[test]
fn similar_short_review_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let (units, items) = burst_fixture();
    let size = (720.0, 560.0);
    for (look, pal, rtl) in [
        ("text2-short", Palette::dark(), false),
        ("he-text2-short", Palette::light(), true),
    ] {
        let lang = if rtl {
            specular_i18n::langid!("he")
        } else {
            specular_i18n::langid!("en-US")
        };
        specular_i18n::I18n::with_thread_language(lang, || {
            for (scale, suffix) in [(1.0, "1x"), (1.5, "1.5x")] {
                let mut g = gallery();
                g.filter_open = false;
                g.similar = similar::Similar::for_shots_review(units.clone(), items.clone());
                let f = Frame {
                    pal: &pal,
                    now: 1.0,
                    reduce_motion: true,
                    size,
                    scale,
                    rtl,
                    text_scale: 2.0,
                    backdrop: Backdrop::None,
                };
                // A first frame lays the footer out; then the pointer goes on Compare.
                let mut warm = DrawList::new(scale);
                h.text.begin_frame(scale);
                g.draw(&mut warm, &mut h.text, &f);
                if !rtl {
                    g.pointer = g.similar.compare_center_for_shots();
                    assert!(g.pointer.is_some());
                }
                let shot = h.render(
                    size,
                    scale,
                    |dl| dl.rect(Rect::new(0.0, 0.0, size.0, size.1), 0.0, pal.canvas),
                    |dl, text| {
                        g.draw(dl, text, &f);
                    },
                );
                goldens.check(&format!("gallery-similar-{look}-review-{suffix}"), &shot);
                crate::app::a11y_audit::audit_screen(
                    &format!("gallery-similar-{look}-review-{suffix}"),
                    SIZE,
                    f.rtl,
                    |t| g.a11y(t, 1.0),
                );
            }
        });
    }
    goldens.finish();
}

/// The review of a burst of small (96 × 72) frames, the first one rated: its Discard and
/// "100 %" (the frames already show larger than 100 %) dimmed as unavailable, with the
/// pointer on "100 %" and its reason in the tooltip above the footer.
#[test]
fn similar_unavailable_controls_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let (units, mut items) = burst_fixture();
    for it in &mut items {
        (it.width, it.height) = (96, 72);
    }
    items[0].protect = Some(specular_similar::Protect::Rated);
    let pal = Palette::dark();
    for (scale, suffix) in [(1.0, "1x"), (1.5, "1.5x")] {
        let mut g = gallery();
        g.filter_open = false;
        g.similar = similar::Similar::for_shots_review(units.clone(), items.clone());
        let f = Frame {
            pal: &pal,
            now: 1.0,
            reduce_motion: true,
            size: SIZE,
            scale,
            rtl: false,
            text_scale: 1.0,
            backdrop: Backdrop::None,
        };
        // A first frame lays the footer out; then the pointer goes on "100 %".
        let mut warm = DrawList::new(scale);
        h.text.begin_frame(scale);
        g.draw(&mut warm, &mut h.text, &f);
        g.pointer = g.similar.zoom_center_for_shots();
        assert!(g.pointer.is_some());
        let shot = h.render(
            SIZE,
            scale,
            |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
            |dl, text| {
                g.draw(dl, text, &f);
            },
        );
        goldens.check(&format!("gallery-similar-unavailable-{suffix}"), &shot);
        crate::app::a11y_audit::audit_screen(
            &format!("gallery-similar-unavailable-{suffix}"),
            SIZE,
            f.rtl,
            |t| g.a11y(t, 1.0),
        );
    }
    goldens.finish();
}

/// The group list at text scale 2 in a short window (720 × 560) with a skipped group: the
/// footer's three buttons (Accept all, Discard with, Show skipped) wrap onto two rows
/// without their keys and the key hints; the list keeps a whole row. In English with the
/// pointer on Accept all (its key in a tooltip above the footer) and in Hebrew (mirrored).
#[test]
fn similar_list_footer_text2_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let (units, items) = burst_fixture();
    let size = (720.0, 560.0);
    for (look, pal, rtl) in [
        ("text2-skipped", Palette::dark(), false),
        ("he-text2-skipped", Palette::light(), true),
    ] {
        let lang = if rtl {
            specular_i18n::langid!("he")
        } else {
            specular_i18n::langid!("en-US")
        };
        specular_i18n::I18n::with_thread_language(lang, || {
            for (scale, suffix) in [(1.0, "1x"), (1.5, "1.5x")] {
                let mut g = gallery();
                g.filter_open = false;
                g.similar = similar::Similar::for_shots_list(units.clone(), items.clone(), 7, 12);
                g.similar.add_skipped_for_shots();
                g.similar.set_sensitivity_for_shots(0.35);
                let f = Frame {
                    pal: &pal,
                    now: 1.0,
                    reduce_motion: true,
                    size,
                    scale,
                    rtl,
                    text_scale: 2.0,
                    backdrop: Backdrop::None,
                };
                // A first frame lays the footer out; then the pointer goes on Accept all.
                let mut warm = DrawList::new(scale);
                h.text.begin_frame(scale);
                g.draw(&mut warm, &mut h.text, &f);
                if !rtl {
                    g.pointer = g.similar.accept_all_center_for_shots();
                    assert!(g.pointer.is_some());
                }
                let shot = h.render(
                    size,
                    scale,
                    |dl| dl.rect(Rect::new(0.0, 0.0, size.0, size.1), 0.0, pal.canvas),
                    |dl, text| {
                        g.draw(dl, text, &f);
                    },
                );
                goldens.check(&format!("gallery-similar-{look}-list-{suffix}"), &shot);
                crate::app::a11y_audit::audit_screen(
                    &format!("gallery-similar-{look}-list-{suffix}"),
                    SIZE,
                    f.rtl,
                    |t| g.a11y(t, 1.0),
                );
            }
        });
    }
    goldens.finish();
}
