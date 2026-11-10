use super::*;

use crate::{DownloadRow, DownloadState, Downloads, DownloadsForm};

fn downloads() -> Downloads {
    let row = |title: &str, fraction, stop: bool| DownloadRow {
        title: title.into(),
        detail: "45 MB of 120 MB · 2.3 MB/s".into(),
        status: "About 35 seconds left".into(),
        fraction,
        state: DownloadState::Running,
        stop: stop.then(|| ("Stop".into(), format!("stop:{title}"))),
    };
    Downloads {
        rows: vec![row("tiles", None, true), row("model", Some(0.375), true)],
        short: "2".into(),
        name: "Downloads: model, 45 MB of 120 MB".into(),
        fraction: None,
    }
}

fn stop_keys(r: &Response) -> Vec<String> {
    r.commands
        .iter()
        .filter_map(|c| match c {
            Command::Downloads(k) => Some(k.clone()),
            _ => None,
        })
        .collect()
}

/// Nothing while nothing downloads; in full beside the toolbar with room (mirrored in
/// right-to-left); the small indicator in a narrow window and with the toolbar hidden.
#[test]
fn downloads_show_in_full_with_room_and_small_without() {
    let mut text = Text::new();
    with_image(|st| {
        for rtl in [false, true] {
            let mut c = chrome(1400.0, rtl);
            // The toolbar comes and goes at once.
            c.reduce_motion = true;
            draw(&mut c, &mut text, st);
            assert_eq!(c.downloads_form(), None);
            assert!(!c.hits.iter().any(|(_, h)| *h == Target::Downloads));
            c.set_downloads(Some(downloads()));
            draw(&mut c, &mut text, st);
            assert_eq!(c.downloads_form(), Some(DownloadsForm::Inline));
            let r = hit(&c, &Target::Downloads);
            let bar = c.bottom_bar().expect("toolbar");
            assert!(!overlaps(r, bar), "beside the toolbar: {r:?} {bar:?}");
            if rtl {
                assert!(r.x > bar.right(), "{r:?}");
            } else {
                assert!(r.right() < bar.x, "{r:?}");
            }
            // Narrow: the indicator, above the toolbar.
            c.set_size(500.0, 700.0);
            draw(&mut c, &mut text, st);
            assert_eq!(c.downloads_form(), Some(DownloadsForm::Compact));
            let r = hit(&c, &Target::Downloads);
            assert!(
                r.w < 100.0 && r.bottom() < c.bottom_bar().unwrap().y,
                "{r:?}"
            );
            // The toolbar hidden (a slideshow): still there, in the corner.
            c.set_size(1400.0, 700.0);
            c.set_presenting(true);
            draw(&mut c, &mut text, st);
            assert_eq!(c.downloads_form(), Some(DownloadsForm::Compact));
            let r = hit(&c, &Target::Downloads);
            assert!(r.bottom() > 700.0 - 60.0, "{r:?}");
            if rtl {
                assert!(r.right() > 1300.0, "{r:?}");
            } else {
                assert!(r.x < 100.0, "{r:?}");
            }
            c.set_downloads(None);
            draw(&mut c, &mut text, st);
            assert_eq!(c.downloads_form(), None);
        }
    });
}

/// The indicator is a named button that opens the panel; the panel lists the downloads,
/// newest first, with progress bars for screen readers and Stop buttons the keyboard
/// reaches.
#[test]
fn the_downloads_panel_is_reached_and_worked_by_keyboard() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        c.set_downloads(Some(downloads()));
        draw(&mut c, &mut text, st);
        let t = c.accessibility_tree(st, "x", 1.0);
        assert!(t.nodes.iter().any(|(_, n)| n.role() == Role::Button
            && n.label() == Some("Downloads: model, 45 MB of 120 MB")));
        assert!(c.indicator_targets().contains(&Target::Downloads));
        c.set_kbd(Some(Target::Downloads));
        draw(&mut c, &mut text, st);
        assert!(press(&mut c, st, NamedKey::Enter, Mods::NONE).consumed);
        assert!(c.downloads_open());
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
                (Some("model".to_owned()), Some(38.0)),
                (Some("tiles".to_owned()), None)
            ]
        );
        // Tab: the newest's Stop first, as on screen.
        tab(&mut c, st);
        assert_eq!(c.modal_focus, Some(Target::DownloadStop(1)));
        let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(stop_keys(&r), ["stop:model"]);
        // It ended while the keyboard was on its Stop: on to Close.
        let mut d = downloads();
        d.rows[1].stop = None;
        d.rows[1].state = DownloadState::Done;
        c.set_downloads(Some(d));
        assert_eq!(c.modal_focus, Some(Target::ModalClose));
        draw(&mut c, &mut text, st);
        assert!(press(&mut c, st, NamedKey::Escape, Mods::NONE).consumed);
        assert!(!c.downloads_open());
        assert_eq!(c.kbd, Some(Target::Downloads));
        // Nothing left: the indicator goes and takes the keyboard with it.
        c.set_downloads(None);
        assert_eq!(c.kbd, None);
        draw(&mut c, &mut text, st);
        assert!(!c.hits.iter().any(|(_, h)| *h == Target::Downloads));
    });
}

/// A bar with no total moves (the chrome asks for frames) unless motion is reduced; with
/// nothing downloading nothing asks for frames.
#[test]
fn only_a_moving_bar_asks_for_frames() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(1400.0, false);
        let mut dl = DrawList::new(1.0);
        let mut wake = |c: &mut Chrome| {
            let mut w = None;
            for i in 0..4 {
                dl.clear(1.0);
                w = c.draw(
                    &mut dl,
                    &mut text,
                    st,
                    &Palette::dark(),
                    1.0 + i as f64 * 0.016,
                );
            }
            w
        };
        let idle = wake(&mut c);
        assert!(idle.is_none_or(|w| w > 0.05), "{idle:?}");
        c.set_downloads(Some(downloads()));
        assert!(wake(&mut c).is_some_and(|w| w <= 0.05));
        c.reduce_motion = true;
        let still = wake(&mut c);
        assert!(still.is_none_or(|w| w > 0.05), "{still:?}");
    });
}
