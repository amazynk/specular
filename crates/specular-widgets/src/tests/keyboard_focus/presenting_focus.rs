use super::*;

/// A slideshow on another display: the main window's "Presenting on" pill sits at the
/// bottom end corner (mirrored in right-to-left), with Pause / Resume and Stop that ask
/// the app, named for screen readers, and one keyboard stop after the toolbar.
#[test]
fn the_presenting_control_pauses_and_stops_the_show_on_the_other_display() {
    let mut text = Text::new();
    with_image(|st| {
        for rtl in [false, true] {
            let mut c = chrome(1000.0, rtl);
            draw(&mut c, &mut text, st);
            assert!(presenting_hit(&c, &Target::PresentingStop(true)).is_none());
            c.set_presenting_on(Some(crate::Presenting {
                display: "HDMI-A-1".into(),
                paused: Some(false),
            }));
            draw(&mut c, &mut text, st);
            let pause = presenting_hit(&c, &Target::PresentingPause(false)).expect("pause");
            let stop = presenting_hit(&c, &Target::PresentingStop(true)).expect("stop");
            assert!(pause.bottom() <= 700.0 && pause.x >= 0.0 && pause.right() <= 1000.0);
            if rtl {
                assert!(
                    stop.right() <= 500.0 && stop.right() <= pause.x,
                    "{stop:?} {pause:?}"
                );
            } else {
                assert!(
                    stop.x >= 500.0 && pause.right() <= stop.x,
                    "{pause:?} {stop:?}"
                );
            }
            let r = c.activate(Target::PresentingPause(false), st);
            assert_eq!(r.commands, [Command::PresentingPause]);
            let r = c.activate(Target::PresentingStop(true), st);
            assert_eq!(r.commands, [Command::PresentingStop]);

            let t = c.accessibility_tree(st, "x", 1.0);
            let pause_node = t
                .nodes
                .iter()
                .find(|(_, n)| n.label() == Some("Pause slideshow"))
                .expect("pause button node");
            assert_eq!(
                pause_node.1.description(),
                Some("Presenting on \u{2068}\u{2066}HDMI-A-1\u{2069}\u{2069}")
            );
            assert!(
                t.nodes
                    .iter()
                    .any(|(_, n)| n.label() == Some("Stop slideshow"))
            );

            // Tab: the toolbar, then the pill (one stop: ←/→ along it), then the image.
            c.set_kbd(None);
            tab(&mut c, st);
            assert_eq!(c.kbd, Some(Target::Position));
            tab(&mut c, st);
            assert_eq!(c.kbd, Some(Target::PresentingPause(false)));
            let next = if rtl {
                NamedKey::ArrowLeft
            } else {
                NamedKey::ArrowRight
            };
            press(&mut c, st, next, Mods::NONE);
            assert_eq!(c.kbd, Some(Target::PresentingStop(true)));
            let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
            assert_eq!(r.commands, [Command::PresentingStop]);
            tab(&mut c, st);
            assert_eq!(c.kbd, None);

            // Paused: the button resumes.
            c.set_presenting_on(Some(crate::Presenting {
                display: "HDMI-A-1".into(),
                paused: Some(true),
            }));
            draw(&mut c, &mut text, st);
            assert!(presenting_hit(&c, &Target::PresentingPause(true)).is_some());
            let t = c.accessibility_tree(st, "x", 1.0);
            assert!(
                t.nodes
                    .iter()
                    .any(|(_, n)| n.label() == Some("Resume slideshow"))
            );

            // Gone when the show ends.
            c.set_presenting_on(None);
            draw(&mut c, &mut text, st);
            assert!(presenting_hit(&c, &Target::PresentingStop(true)).is_none());
        }
    });
}

/// The viewer on another display: the pill has only a Close button.
#[test]
fn the_viewer_on_another_display_has_only_a_close_button() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        c.set_presenting_on(Some(crate::Presenting {
            display: "DP-2".into(),
            paused: None,
        }));
        draw(&mut c, &mut text, st);
        assert!(presenting_hit(&c, &Target::PresentingPause(false)).is_none());
        assert!(presenting_hit(&c, &Target::PresentingPause(true)).is_none());
        assert!(presenting_hit(&c, &Target::PresentingStop(false)).is_some());
        let r = c.activate(Target::PresentingStop(false), st);
        assert_eq!(r.commands, [Command::PresentingStop]);
        let t = c.accessibility_tree(st, "x", 1.0);
        assert!(
            t.nodes
                .iter()
                .any(|(_, n)| n.label() == Some("Close the viewer on the other display"))
        );
    });
}

/// Settings lists the connected displays for `[slideshow] monitor`, and keeps a chosen
/// one that is unplugged, marked as not connected.
#[test]
fn the_monitor_setting_lists_connected_displays() {
    let mut c = chrome(1000.0, false);
    c.set_monitors(vec!["eDP-1".into(), "HDMI-A-1".into()]);
    let m = specular_settings::find("slideshow.monitor").expect("meta");
    let i18n = specular_i18n::I18n::global();
    let opts = crate::settings::monitor_options(m, i18n, "DP-9");
    let values: Vec<_> = opts.iter().map(|o| o.0.as_str()).collect();
    assert_eq!(values, ["auto", "eDP-1", "HDMI-A-1", "DP-9"]);
    assert_eq!(opts[0].1, "Automatic");
    assert_eq!(opts[3].2.as_deref(), Some("Not connected"));
    let opts = crate::settings::monitor_options(m, i18n, "HDMI-A-1");
    assert_eq!(opts.len(), 3);
}

/// The indicator is a named button that opens the panel; the panel is a keyboard dialog
/// whose buttons send their keys, a list of jobs with progress bars for screen readers.
#[test]
fn the_work_indicator_opens_a_keyboard_panel() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        draw(&mut c, &mut text, st);
        assert!(!c.hits.iter().any(|(_, h)| *h == Target::Work), "hidden");
        c.set_work_chip(Some(work_chip()));
        draw(&mut c, &mut text, st);
        let r = hit(&c, &Target::Work);
        assert!(r.bottom() < 700.0 - 60.0, "above the toolbar: {r:?}");
        let t = c.accessibility_tree(st, "x", 1.0);
        assert!(t.nodes.iter().any(|(_, n)| n.role() == Role::Button
            && n.label() == Some("Background work: AI search · 1,240 of 9,800")));
        c.set_kbd(Some(Target::Work));
        draw(&mut c, &mut text, st);
        assert!(press(&mut c, st, NamedKey::Enter, Mods::NONE).consumed);
        assert!(c.work_open());
        c.set_work(work_panel());
        draw(&mut c, &mut text, st);
        let t = c.accessibility_tree(st, "x", 1.0);
        assert!(t.nodes.iter().any(|(_, n)| n.role() == Role::List));
        let bars: Vec<_> = t
            .nodes
            .iter()
            .filter(|(_, n)| n.role() == Role::ProgressIndicator)
            .map(|(_, n)| (n.label().map(str::to_owned), n.numeric_value()))
            .collect();
        assert_eq!(
            bars,
            [
                (Some("search".to_owned()), Some(13.0)),
                (Some("library".to_owned()), None)
            ]
        );
        // Tab: Pause, Settings…, then the second job's Settings… (it cannot pause).
        tab(&mut c, st);
        assert_eq!(c.modal_focus, Some(Target::WorkPause(0)));
        let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(work_keys(&r), ["pause:search"]);
        tab(&mut c, st);
        assert_eq!(c.modal_focus, Some(Target::WorkSettings(0)));
        tab(&mut c, st);
        assert_eq!(c.modal_focus, Some(Target::WorkSettings(1)));
        let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(work_keys(&r), ["settings:library"]);
        // The job ends while the keyboard is on its button: on to Close.
        let mut p = work_panel();
        p.rows.pop();
        c.set_work(p);
        assert_eq!(c.modal_focus, Some(Target::ModalClose));
        draw(&mut c, &mut text, st);
        assert!(press(&mut c, st, NamedKey::Escape, Mods::NONE).consumed);
        assert!(!c.work_open());
        assert_eq!(c.kbd, Some(Target::Work));
        // Everything done: the indicator goes and takes the keyboard with it.
        c.set_work_chip(None);
        assert_eq!(c.kbd, None);
        draw(&mut c, &mut text, st);
        assert!(!c.hits.iter().any(|(_, h)| *h == Target::Work));
    });
}

/// In the gallery the indicator sits in the top bar where the gallery leaves room, and
/// reports its width; mirrored in right-to-left layouts.
#[test]
fn in_the_gallery_the_work_indicator_sits_in_the_top_bar() {
    let mut text = Text::new();
    with_image(|st| {
        for rtl in [false, true] {
            let mut c = chrome(1000.0, rtl);
            c.gallery = true;
            c.gallery_bar = 44.0;
            c.gallery_work_end = Some(800.0);
            c.set_work_chip(Some(work_chip()));
            draw(&mut c, &mut text, st);
            let r = hit(&c, &Target::Work);
            assert!(r.y >= 0.0 && r.bottom() <= 44.0, "{r:?}");
            if rtl {
                assert!((r.x - 200.0).abs() < 1.0, "{r:?}");
            } else {
                assert!((r.right() - 800.0).abs() < 1.0, "{r:?}");
            }
            assert!((c.work_chip_width() - r.w).abs() < 1.0);
            // Without a slot: above the gallery's bottom bar.
            c.gallery_work_end = None;
            draw(&mut c, &mut text, st);
            let r = hit(&c, &Target::Work);
            assert!(r.bottom() <= 700.0 - 44.0 && r.y > 44.0, "{r:?}");
            assert_eq!(c.work_chip_width(), 0.0);
        }
    });
}
