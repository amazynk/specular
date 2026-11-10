//! Develop from the keyboard alone: enter, reach every control, change each, undo it all,
//! redo from the panel, leave. And the screen-reader audit of the panel and filmstrip.

use std::collections::HashSet;
use std::path::PathBuf;

use specular_gfx::{DrawList, Rect, Text};
use specular_input::{Key, KeyCombo, Mods, NamedKey};
use specular_theme::Palette;
use specular_widgets::develop_panel::{Event, Item, PanelButton};

use super::controls::{self, Request, Session};
use super::filmstrip::Strip;
use super::*;

const WINDOW: Rect = Rect {
    x: 0.0,
    y: 0.0,
    w: 1200.0,
    h: 800.0,
};

fn key(k: NamedKey) -> KeyCombo {
    KeyCombo::new(Key::Named(k), Mods::NONE)
}

fn draw(s: &mut Session, text: &mut Text, rtl: bool) {
    let mut dl = DrawList::new(1.0);
    s.panel
        .draw(&mut dl, text, &Palette::dark(), WINDOW, rtl, 1.0);
}

fn open() -> Session {
    let p = PathBuf::from("/photos/IMG_0042.CR3");
    Session::new(p.clone(), controls::new_edit(&p), History::default())
}

/// The panel goes by task: white balance and exposure first, then tone, colour and detail;
/// the repairs and effects after, the output look last; crop and the lens start closed.
#[test]
fn cards_go_by_task() {
    use specular_develop::model::ModuleKind as K;
    let s = open();
    let ids: Vec<u32> = s.panel.cards.iter().map(|c| c.id).collect();
    let m = |k: K| u32::from(k.0);
    let at = |id: u32| {
        ids.iter()
            .position(|&c| c == id)
            .unwrap_or_else(|| panic!("no card {id:#x} in {ids:x?}"))
    };
    // Lightroom's order: Light (with the profile) first, then white balance.
    assert_eq!(ids[..2], [m(K::EXPOSURE), m(K::COLOUR_CALIBRATION)]);
    let order = [
        m(K::EXPOSURE),
        controls::ZONES_CARD,
        m(K::CURVES),
        m(K::COLOUR_BALANCE),
        m(K::BLACK_WHITE),
        controls::GRADING_CARD,
        controls::DETAIL_CARD,
        m(K::CROP_ORIENTATION),
        m(K::LENS),
        controls::CORRECTIONS_CARD,
        m(K::RETOUCH),
        m(K::GRAIN),
        m(K::LUT_3D),
        controls::HDR_CARD,
    ];
    // The view transform has no card of its own: its method is Light's profile.
    assert!(!ids.contains(&m(K::VIEW_TRANSFORM)));
    for w in order.windows(2) {
        assert!(
            at(w[0]) < at(w[1]),
            "{:#x} before {:#x} in {ids:x?}",
            w[0],
            w[1]
        );
    }
    for k in [K::CROP_ORIENTATION, K::LENS] {
        assert!(!s.panel.cards[at(m(k))].expanded, "{k:?} starts closed");
    }
    // No Tone card (its highlights and shadows are Light's); the zones right under Light and
    // white balance, closed: the fine control is one level deeper.
    assert!(!ids.contains(&m(K::TONE_EQUALIZER)));
    assert_eq!(at(controls::ZONES_CARD), at(m(K::COLOUR_CALIBRATION)) + 1);
    assert!(!s.panel.cards[at(controls::ZONES_CARD)].expanded);
    assert!(s.panel.cards[at(m(K::EXPOSURE))].expanded);
}

/// A fresh photo has no changed cards, even once the raw's as-shot white is known; an edit
/// marks its card, a shared module only the card whose fields changed, and undoing it clears
/// the mark.
#[test]
fn changed_cards_follow_the_edit() {
    use specular_develop::model::{ModuleKind as K, Value};
    let mut s = open();
    assert!(s.panel.changed.is_empty(), "{:x?}", s.panel.changed);
    s.set_as_shot([0.3457, 0.3585]);
    assert!(
        s.panel.changed.is_empty(),
        "as shot: {:x?}",
        s.panel.changed
    );
    let m = |k: K| u32::from(k.0);
    let set = |s: &mut Session, k: K, field: &str, v: f32, t: i64| {
        let f = s.edit.module(k, 0).unwrap().params.to_fields();
        assert!(s.set_fields(k, f.with(field, Value::Float(v)), t));
    };
    set(&mut s, K::EXPOSURE, "exposure_ev", 1.0, 1_000);
    assert_eq!(s.panel.changed, [m(K::EXPOSURE)]);
    s.undo();
    assert!(s.panel.changed.is_empty(), "{:x?}", s.panel.changed);
    // The HDR limit is the HDR card's, not the view transform's.
    let hdr = s
        .edit
        .module(K::VIEW_TRANSFORM, 0)
        .unwrap()
        .params
        .to_fields()
        .f32_or("hdr_limit_ev", 0.0);
    set(&mut s, K::VIEW_TRANSFORM, "hdr_limit_ev", hdr + 1.0, 5_000);
    assert!(s.panel.changed.contains(&controls::HDR_CARD));
    assert!(!s.panel.changed.contains(&m(K::VIEW_TRANSFORM)));
    // Smoothing is Tone zones', not Light's.
    set(&mut s, K::TONE_EQUALIZER, "smoothing", 0.05, 9_000);
    assert!(s.panel.changed.contains(&controls::ZONES_CARD));
    assert!(!s.panel.changed.contains(&m(K::EXPOSURE)));
    s.undo();
    s.undo();
    assert!(s.panel.changed.is_empty(), "{:x?}", s.panel.changed);
    // Shadows and contrast are Light's, though other modules hold them.
    set(&mut s, K::TONE_EQUALIZER, "shadows_ev", 1.0, 11_000);
    assert_eq!(s.panel.changed, [m(K::EXPOSURE)]);
    s.undo();
    set(&mut s, K::VIEW_TRANSFORM, "contrast", 2.0, 12_000);
    assert_eq!(s.panel.changed, [m(K::EXPOSURE)]);
    s.undo();
    assert!(s.panel.changed.is_empty(), "{:x?}", s.panel.changed);
}

/// The rare tools stay out of the panel until added from Add a tool, or used by the edit;
/// either way they stay for the session. The basic cards have no switch.
#[test]
fn rare_tools_wait_in_add_a_tool() {
    use specular_develop::model::{ModuleKind as K, Value};
    let mut s = open();
    let m = |k: K| u32::from(k.0);
    let tools: Vec<u32> = controls::TOOLS.iter().map(|k| m(*k)).collect();
    assert_eq!(s.panel.hidden, tools, "every tool waits");
    for k in [
        K::COLOUR_CALIBRATION,
        K::LOCAL_CONTRAST,
        K::CURVES,
        K::COLOUR_EQUALIZER,
    ] {
        let c = s.panel.cards.iter().find(|c| c.id == m(k)).unwrap();
        assert_eq!(c.enabled, None, "{k:?} has no switch");
    }
    let add = s
        .panel
        .cards
        .iter()
        .position(|c| c.id == controls::TOOLS_CARD)
        .expect("Add a tool");
    assert_eq!(add, s.panel.cards.len() - 1, "last");
    assert_eq!(s.panel.cards[add].controls.len(), tools.len());
    // Its first button: point colour, open, the keyboard on its header.
    let ev = vec![Event::Value {
        control: controls::ADD_TOOL,
        part: 0,
        done: true,
    }];
    let steps = s.history.steps.len();
    s.handle(&ev, 1_000);
    assert_eq!(s.history.steps.len(), steps, "not an edit");
    let at = s
        .panel
        .cards
        .iter()
        .position(|c| c.id == m(K::POINT_COLOUR))
        .unwrap();
    assert!(!s.panel.hidden.contains(&m(K::POINT_COLOUR)));
    assert!(s.panel.cards[at].expanded);
    assert_eq!(s.panel.focus(), Some(Item::Header(at)));
    assert_eq!(s.panel.cards[add].controls.len(), tools.len() - 1);
    // Glow used by the edit (a paste, a preset): shown, and still after undo.
    let f = s
        .edit
        .module(K::GLOW, 0)
        .unwrap()
        .params
        .to_fields()
        .with("amount", Value::Float(0.4));
    assert!(s.set_fields(K::GLOW, f, 2_000));
    assert!(!s.panel.hidden.contains(&m(K::GLOW)));
    s.undo();
    assert!(
        !s.panel.hidden.contains(&m(K::GLOW)),
        "stays for the session"
    );
}

#[test]
fn keyboard_only_walkthrough() {
    let mut text = Text::new();
    // Enter Develop (Shift+D) on a raw file, then the keyboard to the panel (`p`).
    let mut s = open();
    let fresh = s.edit.clone();
    // Every tool added (Add a tool is left out once nothing is left to add).
    for i in 0..controls::TOOLS.len() {
        s.add_tool(i);
    }
    assert_eq!(s.panel.hidden, [controls::TOOLS_CARD]);
    // Every card open, the ones that start closed too.
    for c in &mut s.panel.cards {
        c.expanded = true;
    }
    draw(&mut s, &mut text, false);
    s.panel.set_focus(None);
    let order = s.panel.order();
    let mut buttons = 0;
    let mut masked = 0;
    let mut t = 0i64;
    let mut handle = |s: &mut Session, ev: Vec<Event>| {
        t += 2_000;
        s.handle(&ev, t)
    };
    let mut visited = Vec::new();
    let mut changed = HashSet::new();
    for _ in 0..order.len() {
        let item = s.panel.focus().expect("the panel keeps the keyboard");
        visited.push(item);
        match item {
            // Retouch's controls set the next spot or the selected one, with tests of their
            // own (`retouch.rs`): reached, not pressed.
            Item::Control { card, control, .. }
                if super::retouch::is_retouch(s.panel.cards[card].controls[control].id) =>
            {
                masked += 1;
            }
            // The Masks section has its own tests (`masks.rs`): reached, not pressed.
            Item::Control { card, control, .. }
                if super::masks::is_mask(s.panel.cards[card].controls[control].id) =>
            {
                masked += 1;
            }
            Item::Control { card, control, .. }
                if matches!(
                    &s.panel.cards[card].controls[control].body,
                    specular_widgets::develop_panel::Body::Choice(c) if c.is_button()
                ) =>
            {
                // Buttons ask for something (a pick, a file dialog); they change nothing.
                let ev = s.panel.key(key(NamedKey::Enter)).expect("a button's key");
                let r = handle(&mut s, ev);
                assert!(
                    matches!(
                        r.as_slice(),
                        [Request::Pick
                            | Request::ChooseLut
                            | Request::ChartProfile
                            | Request::ChooseFrame(_)
                            | Request::PickFor(_)
                            | Request::Say(_)
                            | Request::WatermarkText(_)
                            | Request::ChooseWatermark
                            | Request::CropGuides
                            | Request::CropCard(_)]
                            // Reset perspective: a step when something was set.
                            | [Request::Render, Request::Say(_)]
                    ),
                    "{item:?}: {r:?}"
                );
                buttons += 1;
            }
            // Perspective's Upright runs on the picture's lines: it asks for them.
            Item::Control { card, control, .. }
                if s.panel.cards[card].controls[control].id == super::perspective::id::MODE =>
            {
                let ev = s
                    .panel
                    .key(key(NamedKey::ArrowRight))
                    .expect("a choice's key");
                let r = handle(&mut s, ev);
                assert!(
                    matches!(r.first(), Some(Request::Upright(_))),
                    "{item:?}: {r:?}"
                );
                changed.insert(item);
            }
            // The crop aspect is the application's (it knows where the photo is).
            Item::Control { card, control, .. }
                if s.panel.cards[card].controls[control].id == super::controls::id::CROP_ASPECT =>
            {
                let ev = s
                    .panel
                    .key(key(NamedKey::ArrowRight))
                    .expect("a choice's key");
                let r = handle(&mut s, ev);
                assert_eq!(
                    r,
                    vec![Request::CropCard(super::controls::id::CROP_ASPECT)],
                    "{item:?}"
                );
                changed.insert(item);
            }
            Item::Control { card, control, .. } => {
                let k = match &s.panel.cards[card].controls[control].body {
                    specular_widgets::develop_panel::Body::Choice(_) => NamedKey::ArrowRight,
                    _ => NamedKey::ArrowUp,
                };
                let mut ev = s.panel.key(key(k)).expect("a control's key");
                if ev.is_empty() {
                    // At the top of its range (LUT strength): down instead.
                    ev = s
                        .panel
                        .key(key(NamedKey::ArrowDown))
                        .expect("a control's key");
                }
                assert!(
                    matches!(ev.as_slice(), [Event::Value { .. }]),
                    "{item:?}: {ev:?}"
                );
                assert_eq!(handle(&mut s, ev), vec![Request::Render], "{item:?}");
                changed.insert(item);
            }
            Item::Switch(_) => {
                for _ in 0..2 {
                    let ev = s.panel.key(key(NamedKey::Space)).unwrap();
                    assert!(matches!(ev.as_slice(), [Event::Enabled { .. }]));
                    handle(&mut s, ev);
                }
            }
            Item::Header(_) => {
                // Closed and opened again.
                for _ in 0..2 {
                    assert_eq!(s.panel.key(key(NamedKey::Enter)), Some(Vec::new()));
                }
            }
            Item::History => {
                let ev = s.panel.key(key(NamedKey::ArrowUp)).unwrap();
                assert!(matches!(ev.as_slice(), [Event::GoTo(_)]), "{ev:?}");
                handle(&mut s, ev);
                let ev = s.panel.key(key(NamedKey::End)).unwrap();
                handle(&mut s, ev);
            }
            // Its menu is the app's (Event::Menu); tests of the menu are in batch.rs.
            Item::Button(_) | Item::More(_) | Item::HistoryRow(_) => {}
        }
        draw(&mut s, &mut text, false);
        s.panel.key(key(NamedKey::Tab)).expect("Tab is the panel's");
    }
    assert_eq!(visited, order, "Tab reaches every control in order");
    // Every control (each wheel and its luminance) was reached and changed.
    let parts: usize = s
        .panel
        .cards
        .iter()
        .flat_map(|c| c.controls.iter())
        // Denoise's AI share appears when the walk turns the mode to AI (after
        // the order was taken): not in the walk.
        .filter(|c| c.id != super::controls::id::NOISE_BLEND)
        .map(|c| match &c.body {
            specular_widgets::develop_panel::Body::Wheels(w) => 2 * w.len(),
            // Notes are read, not reached.
            specular_widgets::develop_panel::Body::Note(_) => 0,
            _ => 1,
        })
        .sum();
    assert_eq!(changed.len() + buttons + masked, parts);
    assert_eq!(
        buttons, 19,
        "point colour's pick, the LUT file, the flat and dark frames, the film base, \
         the graduated filter's handles, the watermark text and SVG, the tone zones' \
         adjusting on the picture, the depth blur's focus pick, perspective's guides and \
         reset, make profile from chart, the crop card's typed ratio, swap, turns and flips"
    );
    // Undo (Ctrl+Z) back to the start: the edit is as it was.
    let steps = s.history.steps.len();
    assert!(
        // "Show on picture" (borders and watermark) is a view, not a step.
        steps >= parts - masked - 1,
        "{steps} steps for {parts} controls"
    );
    while s.history.can_undo() {
        assert!(!s.undo().is_empty());
    }
    assert_eq!(s.edit, fresh);
    // Redo from the panel's button.
    s.panel.set_focus(Some(Item::Button(PanelButton::Redo)));
    let ev = s.panel.key(key(NamedKey::Enter)).unwrap();
    let r = handle(&mut s, ev);
    assert!(r.contains(&Request::Render));
    assert_eq!(s.history.position, 1);
    // Leave: Esc gives the keyboard back, Close leaves.
    assert_eq!(s.panel.key(key(NamedKey::Escape)), Some(vec![Event::Left]));
    s.panel.set_focus(Some(Item::Button(PanelButton::Close)));
    let ev = s.panel.key(key(NamedKey::Space)).unwrap();
    assert_eq!(handle(&mut s, ev), vec![Request::Leave]);
}

#[test]
fn speed_edit_actions_drive_their_sliders() {
    let mut s = open();
    for a in specular_input::DEVELOP_SPEED {
        let c = speed_control(a).expect("every speed-edit action has a control");
        assert!(s.panel.slider_mut(c).is_some(), "{a:?}");
        assert!(s.speed(c, 2.0, 0), "{a:?}");
    }
}

/// The screen-reader audit of Develop as drawn, both ways round, with the filmstrip.
#[test]
fn develop_screen_passes_the_audit() {
    for rtl in [false, true] {
        let mut text = Text::new();
        let mut s = open();
        s.panel.set_focus(Some(Item::Header(1)));
        draw(&mut s, &mut text, rtl);
        let entries: Vec<PathBuf> = (0..9)
            .map(|i| PathBuf::from(format!("/photos/IMG_{i:04}.CR3")))
            .collect();
        let mut strip = Strip::default();
        let mut dl = DrawList::new(1.0);
        let pw = DevelopPanel::width(WINDOW.w, 1.0);
        let r = Rect::new(
            if rtl { pw } else { 0.0 },
            WINDOW.h - filmstrip::HEIGHT,
            WINDOW.w - pw,
            filmstrip::HEIGHT,
        );
        // Right to left with the filmstrip holding the keyboard (F6).
        if rtl {
            s.panel.blur();
        }
        strip.draw(
            &mut dl,
            &Palette::dark(),
            r,
            &entries,
            Some(4),
            rtl,
            1.0,
            rtl,
        );
        assert_eq!(strip.hit(r.center()), Some(4));
        let dev = Develop {
            session: Some(s),
            strip,
            strip_focus: rtl,
            ..Default::default()
        };
        super::super::a11y_audit::audit_screen(
            if rtl { "develop-rtl" } else { "develop" },
            (WINDOW.w, WINDOW.h),
            rtl,
            |t| a11y(&dev, t, &entries, Some(4), 1.0),
        );
    }
}

/// F6 steps picture → panel → filmstrip and round; Shift+F6 the other way. The focused
/// filmstrip gives the screen reader the open image.
#[test]
fn f6_cycles_the_develop_regions() {
    use super::region_step;
    assert_eq!([0, 1, 2].map(|r| region_step(r, true)), [1, 2, 0]);
    assert_eq!([0, 1, 2].map(|r| region_step(r, false)), [2, 0, 1]);
    let entries: Vec<PathBuf> = (0..5)
        .map(|i| PathBuf::from(format!("/photos/IMG_{i:04}.CR3")))
        .collect();
    let mut strip = Strip::default();
    let mut dl = DrawList::new(1.0);
    let r = Rect::new(0.0, 0.0, 800.0, filmstrip::HEIGHT);
    strip.draw(
        &mut dl,
        &Palette::dark(),
        r,
        &entries,
        Some(2),
        false,
        1.0,
        true,
    );
    let mut tree = super::super::a11y_audit::window((800.0, 600.0), 1.0);
    strip.a11y(&mut tree, A11Y_STRIP, &entries, Some(2), true, 1.0);
    assert_eq!(tree.focus, accesskit::NodeId(A11Y_STRIP + 1 + 2));
    let mut tree = super::super::a11y_audit::window((800.0, 600.0), 1.0);
    strip.a11y(&mut tree, A11Y_STRIP, &entries, Some(2), false, 1.0);
    assert_eq!(tree.focus, accesskit::NodeId(1));
}

/// The audit with everything open: every card expanded, a dimmed control (its reason is
/// its description), the history, the scopes, the selected spot's handles and the labels
/// over the picture ("Refining…" a polite status). No unnamed control, no orphan.
#[test]
fn develop_everything_open_passes_the_audit() {
    use specular_widgets::mask_handles::{Handle, Role as HandleRole};
    for rtl in [false, true] {
        let mut text = Text::new();
        let mut s = open();
        for c in &mut s.panel.cards {
            c.expanded = true;
        }
        let first = s
            .panel
            .cards
            .iter()
            .flat_map(|c| c.controls.iter())
            .find(|c| matches!(c.body, specular_widgets::develop_panel::Body::Slider(_)))
            .map(|c| c.id)
            .unwrap();
        s.panel.dimmed.push((first, "Needs a raw file".into()));
        s.panel.show_history = true;
        s.panel.set_focus(Some(Item::Header(0)));
        draw(&mut s, &mut text, rtl);
        let mut dev = Develop {
            session: Some(s),
            ..Default::default()
        };
        // The scopes as drawn at the panel's top.
        dev.aids.dock.shown = true;
        let mut dl = DrawList::new(1.0);
        let pw = DevelopPanel::width(WINDOW.w, 1.0);
        let area = Rect::new(if rtl { 0.0 } else { WINDOW.w - pw }, 48.0, pw, 220.0);
        dev.aids.dock.draw(
            &mut dl,
            &mut text,
            &Palette::dark(),
            area,
            None,
            &Default::default(),
            rtl,
            1.0,
        );
        // A spot's handles, top to bottom as they read.
        let mid = if rtl { 300.0 } else { 900.0 } - pw / 2.0;
        dev.rt_handles.set(
            vec![
                Handle {
                    role: HandleRole::Size,
                    at: (mid, 300.0),
                    name: "Spot 1 size".into(),
                },
                Handle {
                    role: HandleRole::Centre,
                    at: (mid, 400.0),
                    name: "Spot 1".into(),
                },
            ],
            Vec::new(),
        );
        dev.labels.push((
            t!("develop-refining"),
            Rect::new(mid - 40.0, 12.0, 80.0, 20.0),
            true,
        ));
        let mut tree = super::super::a11y_audit::window((WINDOW.w, WINDOW.h), 1.0);
        a11y(&dev, &mut tree, &[], None, 1.0);
        super::super::a11y_audit::expect_clean(
            if rtl {
                "develop-open-rtl"
            } else {
                "develop-open"
            },
            &tree,
            rtl,
        );
        let node =
            |pred: &dyn Fn(&accesskit::Node) -> bool| tree.nodes.iter().any(|(_, n)| pred(n));
        assert!(node(&|n| n.description() == Some("Needs a raw file")));
        assert!(node(&|n| n.role() == accesskit::Role::Status
            && n.label() == Some(t!("develop-refining").as_str())
            && n.live() == Some(accesskit::Live::Polite)));
        assert!(node(&|n| n.label() == Some("Spot 1 size")));
        // The scope picker (a menu since the scopes stopped being tabs).
        assert!(node(&|n| n.label() == Some(t!("develop-scopes").as_str())
            && n.has_popup() == Some(accesskit::HasPopup::Menu)
            && n.is_expanded() == Some(false)));
    }
}

/// The split's divider: the middle until dragged, mirrored right to left, kept inside.
#[test]
fn the_split_divider_drags_and_mirrors() {
    use super::{split_fraction, split_x};
    let image = Rect::new(100.0, 0.0, 800.0, 600.0);
    assert_eq!(split_x(0.0, image, false), 500.0);
    assert_eq!(split_x(0.0, image, true), 500.0);
    assert_eq!(split_x(0.25, image, false), 300.0);
    assert_eq!(split_x(0.25, image, true), 700.0);
    assert_eq!(split_fraction(300.0, image, false), 0.25);
    assert_eq!(split_fraction(300.0, image, true), 0.75);
    assert_eq!(split_fraction(-50.0, image, false), 0.02);
    assert_eq!(split_fraction(5000.0, image, false), 0.98);
    for rtl in [false, true] {
        let f = split_fraction(640.0, image, rtl);
        assert!((split_x(f, image, rtl) - 640.0).abs() < 1e-3);
    }
}

/// The wake is a delay from now: a speed edit's readout 1.5 s away at 40 s into the session
/// wakes in 1.5 s, not in 41.5 s (the window stayed busy for as long as the app had run).
#[test]
fn develop_wake_is_a_delay_not_a_time() {
    let now = 40.0;
    assert_eq!(wake_in(now, [Some(41.5), None, Some(42.0)]), Some(1.5));
    // A save already due wakes at once.
    assert_eq!(wake_in(now, [None, Some(39.0), None]), Some(0.0));
    assert_eq!(wake_in(now, [None, None, None]), None);
}
