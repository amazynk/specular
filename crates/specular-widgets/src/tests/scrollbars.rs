use super::enabled_state::{STATES, with_state};
use crate::ctx::Target;
use crate::scrollbar::{Area, Geometry, Grab, MIN_THUMB, REPEAT_DELAY, REPEAT_INTERVAL};
use crate::{Button, Chrome, Input};
use specular_gfx::Rect;
use specular_input::{Key, KeyCombo, Mods, NamedKey};

/// A 400 px track for a 400 px view over 2000 px of content: an 80 px thumb, 320 px of travel.
fn geo() -> Geometry {
    Geometry {
        track: Rect::new(586.0, 100.0, 14.0, 400.0),
        view: 400.0,
        max: 1600.0,
    }
}

/// An info panel whose scrollbar was drawn last frame.
fn chrome() -> Chrome {
    let mut c = Chrome::new();
    c.info_open = true;
    c.bars.set_geometry(Area::Info, geo());
    c.hits
        .push((Rect::new(300.0, 0.0, 300.0, 600.0), Target::InfoPanel));
    c.hits.push((geo().track, Target::Scrollbar(Area::Info)));
    c
}

fn button(x: f32, y: f32, down: bool) -> Input {
    Input::PointerButton {
        x,
        y,
        down,
        button: Button::Primary,
    }
}

#[test]
fn geometry_keeps_a_minimum_thumb() {
    assert_eq!(geo().thumb_len(), 80.0);
    let long = Geometry { max: 1e6, ..geo() };
    assert_eq!(long.thumb_len(), MIN_THUMB);
    // Ends of the travel map to the ends of the scroll range, both ways.
    let g = geo();
    assert_eq!(g.thumb_top(0.0), 100.0);
    assert_eq!(g.thumb_top(1600.0), 420.0);
    assert_eq!(g.scroll_for_top(420.0), 1600.0);
    assert_eq!(g.scroll_for_top(-50.0), 0.0);
}

/// The thumb follows the pointer 1:1, keeping the point where it was grabbed under the
/// pointer, and keeps following outside the track (pointer capture) until release.
#[test]
fn dragging_the_thumb_follows_the_pointer() {
    with_state(STATES[3], |st| {
        let mut c = chrome();
        let g = geo();
        // Grab 30 px below the thumb's top.
        let r = c.input(&button(593.0, 130.0, true), st, 1.0);
        assert!(r.consumed, "the press is the scrollbar's, not a pan");
        assert_eq!(
            c.bars.grab,
            Some(Grab::Drag {
                area: Area::Info,
                offset: 30.0
            })
        );
        for dy in [10.0, 64.0, 160.0, 250.0] {
            let r = c.input(
                &Input::PointerMove {
                    x: 593.0,
                    y: 130.0 + dy,
                },
                st,
                1.1,
            );
            assert!(r.consumed);
            assert!(
                (g.thumb_top(c.info_scroll) - (100.0 + dy)).abs() < 0.01,
                "dy {dy}: scroll {}",
                c.info_scroll
            );
        }
        // Far outside the track and the window: still dragging, clamped at the end.
        let r = c.input(&Input::PointerMove { x: 10.0, y: 900.0 }, st, 1.2);
        assert!(r.consumed, "captured");
        assert_eq!(c.info_scroll, 1600.0);
        let r = c.input(&Input::PointerMove { x: 10.0, y: 260.0 }, st, 1.3);
        assert!(r.consumed);
        assert!((g.thumb_top(c.info_scroll) - 230.0).abs() < 0.01);
        // Released over the image: the drag ends there, and the release is consumed too.
        let r = c.input(&button(10.0, 260.0, false), st, 1.4);
        assert!(r.consumed);
        assert_eq!(c.bars.grab, None);
        let before = c.info_scroll;
        let r = c.input(&Input::PointerMove { x: 10.0, y: 500.0 }, st, 1.5);
        assert!(!r.consumed, "no capture after the release");
        assert_eq!(c.info_scroll, before);
    });
}

/// A click on the track pages towards the pointer; holding repeats after a delay until the
/// thumb reaches the pointer.
#[test]
fn track_clicks_page_and_holding_repeats() {
    with_state(STATES[3], |st| {
        let mut c = chrome();
        let g = geo();
        let page = g.page();
        assert_eq!(page, 360.0);
        let r = c.input(&button(593.0, 400.0, true), st, 1.0);
        assert!(r.consumed);
        assert_eq!(c.info_scroll, page);
        // Nothing more before the repeat delay.
        c.bar_tick(1.0 + REPEAT_DELAY * 0.5);
        assert_eq!(c.info_scroll, page);
        let mut t = 1.0 + REPEAT_DELAY;
        for _ in 0..20 {
            c.bar_tick(t);
            t += REPEAT_INTERVAL;
        }
        // Stops at the first page whose thumb covers the pointer (y 400 → scroll ≥ 1100).
        assert_eq!(c.info_scroll, 4.0 * page);
        let top = g.thumb_top(c.info_scroll);
        assert!(top <= 400.0 && 400.0 <= top + g.thumb_len());
        assert!(c.input(&button(593.0, 400.0, false), st, t).consumed);
        assert_eq!(c.bars.grab, None);
        // Above the thumb pages back.
        c.input(&button(593.0, 110.0, true), st, t);
        assert_eq!(c.info_scroll, 3.0 * page);
        c.input(&button(593.0, 110.0, false), st, t);
    });
}

#[test]
fn paging_keys_scroll_the_focused_panel() {
    with_state(STATES[3], |st| {
        let key = |k| Input::Key {
            combo: KeyCombo::new(Key::Named(k), Mods::NONE),
            repeat: false,
        };
        let mut c = chrome();
        // Not focused: the keys are the viewer's (PgDn = next image).
        assert!(!c.input(&key(NamedKey::PageDown), st, 1.0).consumed);
        // A click in the panel focuses it.
        c.input(&button(400.0, 300.0, true), st, 1.0);
        c.input(&button(400.0, 300.0, false), st, 1.0);
        assert!(c.input(&key(NamedKey::PageDown), st, 1.1).consumed);
        assert_eq!(c.info_scroll, geo().page());
        c.input(&key(NamedKey::End), st, 1.2);
        assert_eq!(c.info_scroll, 1600.0);
        c.input(&key(NamedKey::Home), st, 1.3);
        assert_eq!(c.info_scroll, 0.0);
        // Nothing to scroll: the key goes back to the viewer.
        c.bars.begin_frame();
        assert!(!c.input(&key(NamedKey::PageDown), st, 1.4).consumed);
    });
}
