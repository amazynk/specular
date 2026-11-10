//! Develop's panel: the keyboard reaches every control in reading order, the pointer drags,
//! double-clicks and scrolls, and the accessibility tree passes the audit both ways round.

use specular_gfx::{DrawList, Rect, Text};
use specular_input::{Key, KeyCombo, Mods, NamedKey};
use specular_theme::Palette;

use super::*;
use crate::choice;
use crate::slider::{SliderSpec, Unit};

const AREA: Rect = Rect {
    x: 0.0,
    y: 0.0,
    w: 900.0,
    h: 2000.0,
};

pub(crate) fn sample() -> DevelopPanel {
    let ev = SliderSpec::new("Exposure", -5.0, 5.0, 0.0)
        .steps(0.01, 0.5)
        .unit(Unit::Ev, 2)
        .signed();
    let contrast = SliderSpec::new("Contrast", -1.0, 1.0, 0.0).unit(Unit::Percent, 0);
    let lum = |l: &str| {
        SliderSpec::new(l, -1.0, 1.0, 0.0)
            .unit(Unit::Percent, 0)
            .compact()
    };
    let cards = vec![
        Card {
            id: 1,
            title: "Exposure".into(),
            enabled: Some(true),
            expanded: true,
            controls: vec![
                Control {
                    id: 10,
                    body: Body::Slider(Slider::new(ev)),
                },
                Control {
                    id: 11,
                    body: Body::Slider(Slider::new(contrast)),
                },
            ],
        },
        Card {
            id: 2,
            title: "Curve".into(),
            enabled: Some(true),
            expanded: true,
            controls: vec![Control {
                id: 20,
                body: Body::Curve(Curve::identity("Curve")),
            }],
        },
        Card {
            id: 3,
            title: "Colour grading".into(),
            enabled: Some(false),
            expanded: true,
            controls: vec![Control {
                id: 30,
                body: Body::Wheels(
                    ["Shadows", "Midtones", "Highlights"]
                        .iter()
                        .map(|l| (Wheel::new(*l), Slider::new(lum(l))))
                        .collect(),
                ),
            }],
        },
        Card {
            id: 4,
            title: "Crop".into(),
            enabled: None,
            expanded: false,
            controls: vec![Control {
                id: 40,
                body: Body::Slider(Slider::new(SliderSpec::new("Straighten", -45.0, 45.0, 0.0))),
            }],
        },
    ];
    let mut p = DevelopPanel::new("Develop", cards);
    p.history = vec!["Original".into(), "Exposure".into(), "Contrast".into()];
    p.history_at = 2;
    p.can_undo = true;
    p
}

fn draw(p: &mut DevelopPanel, text: &mut Text, rtl: bool) {
    let mut dl = DrawList::new(1.0);
    p.draw(&mut dl, text, &Palette::dark(), AREA, rtl, 1.0);
}

fn key(k: NamedKey) -> KeyCombo {
    KeyCombo::new(Key::Named(k), Mods::NONE)
}

fn tab(p: &mut DevelopPanel) -> Vec<Event> {
    p.key(key(NamedKey::Tab)).expect("Tab is the panel's")
}

#[test]
fn tab_walks_every_control_in_order_and_wraps() {
    let mut p = sample();
    p.set_focus(None);
    let order = p.order();
    // Buttons, then each card's header, switch and controls (the closed card's controls
    // are not stops), then the history.
    assert_eq!(order[0], Item::Button(PanelButton::Undo));
    assert_eq!(order.len(), 5 + (2 + 2) + (2 + 1) + (2 + 6) + 1 + 2);
    let mut seen = vec![p.focus().unwrap()];
    for _ in 1..order.len() {
        tab(&mut p);
        seen.push(p.focus().unwrap());
    }
    assert_eq!(seen, order);
    tab(&mut p);
    assert_eq!(p.focus(), Some(order[0]));
    p.key(KeyCombo::new(Key::Named(NamedKey::Tab), Mods::SHIFT));
    assert_eq!(p.focus(), order.last().copied());
}

#[test]
fn keys_change_values_toggle_cards_and_leave() {
    let mut p = sample();
    let mut text = Text::new();
    draw(&mut p, &mut text, false);
    let ev_item = Item::Control {
        card: 0,
        control: 0,
        part: 0,
    };
    p.set_focus(Some(ev_item));
    let e = p.key(key(NamedKey::ArrowRight)).unwrap();
    assert_eq!(
        e,
        vec![Event::Value {
            control: 10,
            part: 0,
            done: true
        }]
    );
    let Some(Body::Slider(s)) = p.control(10).map(|c| &c.body) else {
        panic!()
    };
    assert!((s.value - 0.01).abs() < 1e-6);
    // Typed, then Tab sets it.
    for c in ['1', '.', '5'] {
        p.key(KeyCombo::new(Key::Char(c), Mods::NONE)).unwrap();
    }
    let e = tab(&mut p);
    assert_eq!(e.len(), 1);
    let Some(Body::Slider(s)) = p.control(10).map(|c| &c.body) else {
        panic!()
    };
    assert_eq!(s.value, 1.5);
    // The switch and the header.
    p.set_focus(Some(Item::Switch(2)));
    let e = p.key(key(NamedKey::Space)).unwrap();
    assert_eq!(e, vec![Event::Enabled { card: 3, on: true }]);
    p.set_focus(Some(Item::Header(3)));
    p.key(key(NamedKey::Enter)).unwrap();
    assert!(p.cards[3].expanded);
    // A wheel and its luminance.
    p.set_focus(Some(Item::Control {
        card: 2,
        control: 0,
        part: 2,
    }));
    let e = p.key(key(NamedKey::ArrowUp)).unwrap();
    assert_eq!(
        e,
        vec![Event::Value {
            control: 30,
            part: 2,
            done: true
        }]
    );
    // History: Up goes back one step.
    p.set_focus(Some(Item::History));
    assert_eq!(p.key(key(NamedKey::ArrowUp)).unwrap(), vec![Event::GoTo(1)]);
    assert_eq!(p.key(key(NamedKey::Home)).unwrap(), vec![Event::GoTo(0)]);
    // A button.
    p.set_focus(Some(Item::Button(PanelButton::BeforeAfter)));
    assert_eq!(
        p.key(key(NamedKey::Enter)).unwrap(),
        vec![Event::Button(PanelButton::BeforeAfter)]
    );
    // Not the panel's key: falls through.
    assert!(p.key(KeyCombo::new(Key::Char('z'), Mods::CTRL)).is_none());
    // Esc gives the keyboard back.
    assert_eq!(p.key(key(NamedKey::Escape)).unwrap(), vec![Event::Left]);
    assert_eq!(p.focus(), None);
    assert!(p.key(key(NamedKey::Tab)).is_none());
}

#[test]
fn escape_while_typing_cancels_first() {
    let mut p = sample();
    p.set_focus(Some(Item::Control {
        card: 0,
        control: 1,
        part: 0,
    }));
    p.key(KeyCombo::new(Key::Char('5'), Mods::NONE)).unwrap();
    assert_eq!(p.key(key(NamedKey::Escape)).unwrap(), Vec::new());
    assert!(p.focus().is_some());
}

#[test]
fn pointer_drags_double_clicks_and_presses_buttons() {
    let mut p = sample();
    let mut text = Text::new();
    draw(&mut p, &mut text, false);
    let row = p
        .rect_of(Item::Control {
            card: 0,
            control: 0,
            part: 0,
        })
        .unwrap();
    let track = crate::slider::track_of(row, 1.0);
    // The thumb sits at the middle (0 EV): press on it and drag right.
    let thumb = (track.center().0, track.center().1);
    let e = p.press(thumb, false, 10.0).unwrap();
    assert!(e.is_empty(), "{e:?}");
    let e = p.pointer_move(Some((thumb.0 + track.w * 0.1, thumb.1)), false);
    assert_eq!(e.len(), 1);
    let e = p.release().unwrap();
    assert_eq!(
        e,
        vec![Event::Value {
            control: 10,
            part: 0,
            done: true
        }]
    );
    let v = |p: &DevelopPanel| match &p.control(10).unwrap().body {
        Body::Slider(s) => s.value,
        _ => unreachable!(),
    };
    assert!((v(&p) - 1.0).abs() < 0.02, "{}", v(&p));
    // Shift: a tenth (pressed on the thumb where it is now).
    let thumb = (thumb.0 + track.w * 0.1, thumb.1);
    p.press(thumb, false, 20.0);
    p.pointer_move(Some((thumb.0 + track.w * 0.1, thumb.1)), true);
    p.release();
    assert!((v(&p) - 1.1).abs() < 0.02, "{}", v(&p));
    // Double-click resets.
    p.press((track.x + 5.0, track.center().1), false, 30.0);
    p.release();
    let e = p
        .press((track.x + 5.0, track.center().1), false, 30.2)
        .unwrap();
    assert_eq!(v(&p), 0.0);
    assert_eq!(e.len(), 1);
    // A header button.
    let b = p.rect_of(Item::Button(PanelButton::Close)).unwrap();
    assert_eq!(
        p.press(b.center(), false, 40.0).unwrap(),
        vec![Event::Button(PanelButton::Close)]
    );
    // A history row.
    let r = p.rect_of(Item::HistoryRow(0)).unwrap();
    assert_eq!(
        p.press(r.center(), false, 50.0).unwrap(),
        vec![Event::GoTo(0)]
    );
    assert_eq!(p.focus(), Some(Item::History));
    // Outside: not the panel's.
    assert!(p.press((10.0, 10.0), false, 60.0).is_none());
}

#[test]
fn a_short_window_scrolls_to_the_focused_control() {
    let mut p = sample();
    let mut text = Text::new();
    let short = Rect::new(0.0, 0.0, 900.0, 400.0);
    let mut dl = DrawList::new(1.0);
    p.draw(&mut dl, &mut text, &Palette::dark(), short, false, 1.0);
    p.set_focus(Some(Item::History));
    p.draw(&mut dl, &mut text, &Palette::dark(), short, false, 1.0);
    let r = p.rect_of(Item::History).unwrap();
    assert!(
        r.bottom() <= short.bottom() && r.y >= p.layout.body.y,
        "{r:?}"
    );
    assert!(p.wheel((850.0, 200.0), 5.0));
    assert!(!p.wheel((10.0, 200.0), 5.0));
}

#[test]
fn scrollbar_drags_pages_and_widens_under_the_pointer() {
    for rtl in [false, true] {
        let mut p = sample();
        let mut text = Text::new();
        let short = Rect::new(0.0, 0.0, 900.0, 400.0);
        let mut redraw = |p: &mut DevelopPanel| {
            let mut dl = DrawList::new(1.0);
            p.draw(&mut dl, &mut text, &Palette::dark(), short, rtl, 1.0);
        };
        redraw(&mut p);
        let g = p.layout.bar.expect("the cards overflow a short window");
        assert!(p.rect().contains((g.track.x, g.track.y)), "{rtl}");
        let x = g.track.x + g.track.w * 0.5;
        let top = g.thumb_top(0.0);

        // Hovering widens it; leaving narrows it again.
        assert!(!p.bar_hot());
        p.pointer_move(Some((x, top + 5.0)), false);
        assert!(p.bar_hot());
        p.pointer_move(Some((x - 100.0 * if rtl { -1.0 } else { 1.0 }, top)), false);
        assert!(!p.bar_hot());

        // Dragging the thumb follows the pointer, also outside the panel, and captures it.
        assert_eq!(p.press((x, top + 5.0), false, 0.0), Some(Vec::new()));
        assert!(p.dragging());
        p.pointer_move(Some((-50.0, top + 5.0 + 60.0)), false);
        assert!((p.scroll - g.scroll_for_top(top + 60.0)).abs() < 0.01);
        p.pointer_move(Some((x, 10_000.0)), false);
        assert_eq!(p.scroll, g.max);
        assert_eq!(p.release(), Some(Vec::new()));
        assert!(!p.dragging());
        redraw(&mut p);

        // Pressing the track above the thumb pages up.
        p.press((x, g.track.y + 1.0), false, 1.0);
        assert!((p.scroll - (g.max - g.page()).max(0.0)).abs() < 0.01);
        assert!(p.release().is_none());
    }
}

#[test]
fn panel_mirrors_in_right_to_left() {
    let mut p = sample();
    let mut text = Text::new();
    draw(&mut p, &mut text, true);
    assert_eq!(p.rect().x, 0.0);
    let undo = p.rect_of(Item::Button(PanelButton::Undo)).unwrap();
    let close = p.rect_of(Item::Button(PanelButton::Close)).unwrap();
    assert!(undo.x > close.x, "reading order runs right to left");
    draw(&mut p, &mut text, false);
    assert_eq!(p.rect().right(), AREA.right());
}

fn audit(rtl: bool, ts: f32) {
    let mut p = sample();
    p.cards[3].expanded = true;
    // A card with a ⋯ menu button too, and changed cards (one of them off).
    p.menus = vec![1];
    p.changed = vec![1, 3];
    p.set_focus(Some(Item::Control {
        card: 1,
        control: 0,
        part: 0,
    }));
    let mut text = Text::new();
    let mut dl = DrawList::new(1.0);
    p.draw(&mut dl, &mut text, &Palette::light(), AREA, rtl, ts);
    let mut root = accesskit::Node::new(accesskit::Role::Window);
    root.set_label("Specular");
    root.set_bounds(accesskit::Rect::new(0.0, 0.0, 900.0, 2000.0));
    let mut tree = accesskit::TreeUpdate {
        nodes: vec![(accesskit::NodeId(1), root)],
        tree: Some(accesskit::TreeInfo::new(accesskit::NodeId(1))),
        tree_id: accesskit::TreeId::ROOT,
        focus: accesskit::NodeId(1),
    };
    p.a11y(&mut tree, 0x7700_0000_0000_0000, 1.0);
    let f = crate::a11y_audit::audit(&tree, rtl);
    assert!(f.is_empty(), "rtl {rtl}, ts {ts}:\n{}", f.join("\n"));
    // Screen-reader requests find their controls.
    let id = node_id(
        0x7700_0000_0000_0000,
        Item::Control {
            card: 0,
            control: 0,
            part: 0,
        },
    );
    let e = p
        .a11y_action(
            0x7700_0000_0000_0000,
            id,
            accesskit::Action::Increment,
            None,
        )
        .unwrap();
    assert_eq!(e.len(), 1);
    let e = p
        .a11y_action(
            0x7700_0000_0000_0000,
            id,
            accesskit::Action::SetValue,
            Some(2.0),
        )
        .unwrap();
    assert_eq!(e.len(), 1);
    assert!(
        p.a11y_action(0x7700_0000_0000_0000, 5, accesskit::Action::Click, None)
            .is_none()
    );
}

/// A card listed in `menus` has a ⋯ button between its header and its switch: a keyboard
/// stop of its own, clear of both, and a click or Enter on it asks for the card's menu.
#[test]
fn card_menu_button_asks_for_the_menu() {
    for rtl in [false, true] {
        let mut p = sample();
        p.menus = vec![1];
        let mut text = Text::new();
        draw(&mut p, &mut text, rtl);
        let order = p.order();
        let h = order.iter().position(|i| *i == Item::Header(0)).unwrap();
        assert_eq!(order[h + 1], Item::More(0));
        assert_eq!(order[h + 2], Item::Switch(0));
        assert!(!order.contains(&Item::More(1)), "card 2 has no menu");
        let more = p.rect_of(Item::More(0)).unwrap();
        let apart = |a: Rect, b: Rect| a.right() <= b.x + 0.5 || b.right() <= a.x + 0.5;
        for other in [Item::Header(0), Item::Switch(0)] {
            let r = p.rect_of(other).unwrap();
            assert!(
                apart(more, r),
                "rtl {rtl}: ⋯ {more:?} overlaps {other:?} {r:?}"
            );
        }
        let is_menu = |e: &[Event]| {
            matches!(
                e,
                [Event::Menu {
                    item: Item::Header(0),
                    ..
                }]
            )
        };
        let e = p.press(more.center(), false, 0.0).unwrap();
        assert!(is_menu(&e), "rtl {rtl}: {e:?}");
        let _ = p.release();
        p.set_focus(Some(Item::More(0)));
        let e = p.key(key(NamedKey::Enter)).unwrap();
        assert!(is_menu(&e), "rtl {rtl}: {e:?}");
    }
}

/// A click puts the keyboard where it lands without a focus ring (the pointer already shows
/// the place); the next key brings the ring back.
#[test]
fn a_click_focuses_without_a_ring() {
    let mut p = sample();
    let mut text = Text::new();
    draw(&mut p, &mut text, false);
    let switch = p.rect_of(Item::Switch(0)).unwrap();
    let _ = p.press(switch.center(), false, 0.0);
    let _ = p.release();
    assert_eq!(p.focus(), Some(Item::Switch(0)));
    assert!(!p.ringed(Item::Switch(0)), "no ring after a click");
    // Put back where it was (a rebuilt panel), still no ring.
    p.set_focus(Some(Item::Switch(0)));
    assert!(!p.ringed(Item::Switch(0)));
    tab(&mut p);
    let at = p.focus().unwrap();
    assert!(p.ringed(at), "Tab rings {at:?}");
    p.blur();
    p.set_focus(None);
    assert!(p.ringed(p.focus().unwrap()), "the keyboard coming in rings");
}

/// A hidden card takes no room, no keyboard stop and no pointer: the cards after it move up.
#[test]
fn hidden_cards_are_left_out() {
    let mut p = sample();
    let mut text = Text::new();
    draw(&mut p, &mut text, false);
    let third = p.rect_of(Item::Header(2)).unwrap();
    p.hidden = vec![2];
    draw(&mut p, &mut text, false);
    assert!(p.rect_of(Item::Header(1)).is_none());
    assert!(!p.order().iter().any(|i| matches!(
        i,
        Item::Header(1) | Item::Switch(1) | Item::Control { card: 1, .. }
    )));
    assert!(p.rect_of(Item::Header(2)).unwrap().y < third.y, "moved up");
    // Its controls still hold values.
    assert!(p.control(20).is_some());
}

/// A changed card says so (a dot, and the header's description), off or on.
#[test]
fn changed_and_off_cards() {
    for rtl in [false, true] {
        let mut p = sample();
        p.changed = vec![1, 3];
        assert_eq!(p.cards[2].enabled, Some(false));
        assert!(p.cards[2].expanded);
        let mut text = Text::new();
        let mut dl = DrawList::new(1.0);
        p.draw(&mut dl, &mut text, &Palette::dark(), AREA, rtl, 1.0);
        let mut tree = accesskit::TreeUpdate {
            nodes: Vec::new(),
            tree: None,
            tree_id: accesskit::TreeId::ROOT,
            focus: accesskit::NodeId(1),
        };
        let base = 0x7700_0000_0000_0000;
        p.a11y(&mut tree, base, 1.0);
        let header = |ci: usize| {
            let id = accesskit::NodeId(node_id(base, Item::Header(ci)));
            let n = &tree.nodes.iter().find(|(i, _)| *i == id).unwrap().1;
            n.description().map(str::to_owned)
        };
        assert_eq!(
            header(0).as_deref(),
            Some(t!("develop-card-changed").as_str())
        );
        assert_eq!(header(1), None);
        assert!(header(2).is_some(), "an off card can be changed too");
    }
}

#[test]
fn accessibility_audit_is_clean_both_ways_round() {
    audit(false, 1.0);
    audit(true, 1.0);
    audit(false, 1.5);
    audit(true, 1.5);
}

/// In a narrow panel a label that wraps makes its row taller (the track and the next
/// row stay clear of it), and a button grows to show its whole label.
#[test]
fn wrapped_labels_grow_their_rows() {
    let narrow = Rect::new(0.0, 0.0, 480.0, 2000.0);
    let long = "Frequency separation radius between the coarse and fine layers (pixels)";
    let mut p = sample();
    p.cards[0].controls.push(Control {
        id: 12,
        body: Body::Slider(Slider::new(SliderSpec::new(long, 0.0, 10.0, 4.0))),
    });
    p.cards[0].controls.push(Control {
        id: 13,
        body: Body::Choice(Choice::button(
            "Open the Local AI settings to download the depth model…",
        )),
    });
    p.cards[0].controls.push(Control {
        id: 14,
        body: Body::Choice(Choice::button("Reset")),
    });
    let mut text = Text::new();
    for (rtl, ts) in [(false, 1.0), (true, 1.0), (false, 1.5), (true, 1.5)] {
        let mut dl = DrawList::new(1.0);
        p.draw(&mut dl, &mut text, &Palette::dark(), narrow, rtl, ts);
        let r = |k: usize| {
            p.item_rect(Item::Control {
                card: 0,
                control: k,
                part: 0,
            })
            .expect("drawn")
        };
        let row = (slider::ROW * ts.max(1.0)).round();
        let button = (choice::ROW * ts.max(1.0)).round();
        if ts == 1.0 {
            assert_eq!(r(0).h, row, "a short label keeps one row");
        } else {
            // At 1.5× "Exposure" no longer fits beside "−5.00 EV" in this panel
            // (it was cut): the value goes under it, one line more.
            assert!(r(0).h > row, "rtl {rtl}: {}", r(0).h);
        }
        assert!(r(2).h > row, "rtl {rtl} ts {ts}: {}", r(2).h);
        assert!(r(3).h > button, "rtl {rtl} ts {ts}: {}", r(3).h);
        assert_eq!(r(4).h, button);
        for k in 0..4 {
            assert!(
                r(k + 1).y >= r(k).bottom() - 0.5,
                "rows {k} and {} overlap",
                k + 1
            );
        }
    }
}

/// A dimmed control says why to the screen reader (and on hover or focus), and it
/// still works.
#[test]
fn dimmed_controls_say_why() {
    let mut p = sample();
    p.dimmed = vec![(11, "Turn on Contrast first.".into())];
    assert_eq!(p.dimmed_reason(11), Some("Turn on Contrast first."));
    assert_eq!(p.dimmed_reason(10), None);
    let it = Item::Control {
        card: 0,
        control: 1,
        part: 0,
    };
    p.set_focus(Some(it));
    let mut text = Text::new();
    draw(&mut p, &mut text, false);
    let mut tree = accesskit::TreeUpdate {
        nodes: Vec::new(),
        tree: None,
        tree_id: accesskit::TreeId::ROOT,
        focus: accesskit::NodeId(1),
    };
    p.a11y(&mut tree, 0x7700_0000_0000_0000, 1.0);
    let id = accesskit::NodeId(node_id(0x7700_0000_0000_0000, it));
    let n = &tree.nodes.iter().find(|(i, _)| *i == id).expect("node").1;
    assert_eq!(n.description(), Some("Turn on Contrast first."));
    // Its keys still move it.
    let ev = p.key(key(NamedKey::ArrowUp)).expect("its key");
    assert!(matches!(ev.as_slice(), [Event::Value { control: 11, .. }]));
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
}

/// A dimmed control's reason waits out the tooltip delay (pointer and keyboard;
/// the screen-reader description is there at once) and never covers its row or the row
/// above: under the row, or beside the panel when there is no room under it.
#[test]
fn dimmed_reason_waits_and_leaves_the_rows_free() {
    let it = |k: usize| Item::Control {
        card: 0,
        control: k,
        part: 0,
    };
    let mut text = Text::new();
    for rtl in [false, true] {
        let mut p = sample();
        p.dimmed = vec![(11, "Turn on Contrast first, then move this slider.".into())];
        p.clock = 10.0;
        p.set_focus(Some(it(1)));
        draw(&mut p, &mut text, rtl);
        assert_eq!(p.tip_rect(), None, "at once for the keyboard");
        let due = p.tip_wake().expect("waits");
        assert!((due - 10.0 - f64::from(durations::TOOLTIP_DELAY)).abs() < 1e-9);
        p.clock = due + 0.01;
        draw(&mut p, &mut text, rtl);
        let tip = p.tip_rect().expect("shown after the wait");
        assert_eq!(p.tip_wake(), None);
        let row = p.item_rect(it(1)).unwrap();
        let above = p.item_rect(it(0)).unwrap();
        assert!(
            !overlaps(tip, row) && !overlaps(tip, above),
            "rtl {rtl}: {tip:?}"
        );
        assert!(tip.y >= row.bottom(), "under the row");
        // The pointer on it: the same wait from when it got there.
        p.set_focus(Some(it(0)));
        p.pointer_move(Some(row.center()), false);
        p.clock = 20.0;
        draw(&mut p, &mut text, rtl);
        assert_eq!(p.tip_rect(), None, "at once for the pointer");
        p.clock = 20.6;
        draw(&mut p, &mut text, rtl);
        assert!(p.tip_rect().is_some());
        // A panel only as tall as down to the row: beside the panel, level with the row.
        let short = Rect::new(0.0, 0.0, AREA.w, row.bottom() + 6.0);
        p.clock = 30.0;
        let mut dl = DrawList::new(1.0);
        p.draw(&mut dl, &mut text, &Palette::dark(), short, rtl, 1.0);
        p.clock = 30.6;
        p.draw(&mut dl, &mut text, &Palette::dark(), short, rtl, 1.0);
        let row = p.item_rect(it(1)).unwrap();
        let above = p.item_rect(it(0)).unwrap();
        if let Some(tip) = p.tip_rect() {
            assert!(
                !overlaps(tip, row) && !overlaps(tip, above),
                "rtl {rtl}: {tip:?} {row:?}"
            );
        }
    }
}
