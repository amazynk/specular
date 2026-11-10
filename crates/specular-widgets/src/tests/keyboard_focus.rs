use crate::accesskit::{Action as AkAction, ActionRequest, NodeId, Role, TreeId};
use crate::ctx::Target;
use crate::{Button, Chrome, Command, HudStats, Input, InsightItem, Insights};
use crate::{Overlays, Response, ViewerState};
use specular_gfx::{DrawList, Rect, Text};
use specular_input::{Action, ActionState, ImageState, Key, KeyCombo, Keymap, Mods, NamedKey};
use specular_settings::{Settings, SettingsStore, ToolbarItem as T};
use specular_theme::Palette;

fn with_keymap<R>(keymap: Keymap, f: impl FnOnce(&ViewerState) -> R) -> R {
    let hud = HudStats::default();
    let store = SettingsStore::in_memory();
    let settings = Settings::default();
    let st = ViewerState {
        actions: ActionState {
            comparing: false,
            compare_images: 0,
            volume: false,
            mpr: false,
            image: ImageState::Ready,
            count: 4,
            gallery_files: 0,
            animated: false,
            video: false,
            sound: false,
            sound_file: false,
            in_archive: false,
            has_alpha: false,
            gallery_views: None,
            medical: None,
            dicom: false,
            series: 0,
            presentation_states: 0,
            science: None,
            debayer: false,
            annotations: false,
            stain: None,
            stain_region: false,
            slide_annotations: false,
            thermal_visible: false,
            drawing: false,
            mesh: false,
            points: false,
            molecule: false,
            tracts: false,
            relight: false,
            map: None,
            import_sources: false,
            pages: 0,
            rotatable: false,
            wallpaper: false,
            share: false,
            email: false,
            profile: specular_input::Profile::UNRESTRICTED,
        },
        has_image: true,
        index: 1,
        count: 4,
        zoom: 1.0,
        at_fit: true,
        fullscreen: false,
        file_name: Some("beach.jpg"),
        dims: Some((4000, 3000)),
        loading: false,
        error: None,
        truncated: false,
        metadata: None,
        hud: &hud,
        keymap: &keymap,
        store: &store,
        settings: &settings,
        overlays: Overlays::default(),
        playback: None,
        motion: None,
        spread: None,
        pages: None,
    };
    f(&st)
}

fn with_image<R>(f: impl FnOnce(&ViewerState) -> R) -> R {
    with_keymap(Keymap::default(), f)
}

fn chrome(w: f32, rtl: bool) -> Chrome {
    let mut c = Chrome::new();
    c.set_size(w, 700.0);
    c.force_rtl = rtl;
    c.poke(1.0);
    c
}

fn draw(c: &mut Chrome, text: &mut Text, st: &ViewerState) {
    let mut dl = DrawList::new(1.0);
    for i in 0..4 {
        dl.clear(1.0);
        c.draw(&mut dl, text, st, &Palette::dark(), 1.0 + i as f64 * 0.016);
    }
}

fn combo(k: NamedKey, mods: Mods) -> KeyCombo {
    KeyCombo::new(Key::Named(k), mods)
}

fn press(c: &mut Chrome, st: &ViewerState, k: NamedKey, mods: Mods) -> Response {
    c.input(
        &Input::Key {
            combo: combo(k, mods),
            repeat: false,
        },
        st,
        2.0,
    )
}

fn repeat(c: &mut Chrome, st: &ViewerState, k: NamedKey, now: f64) -> Response {
    c.input(
        &Input::Key {
            combo: combo(k, Mods::NONE),
            repeat: true,
        },
        st,
        now,
    )
}

fn release(c: &mut Chrome, st: &ViewerState, k: NamedKey, now: f64) -> Response {
    c.input(
        &Input::KeyUp {
            combo: combo(k, Mods::NONE),
        },
        st,
        now,
    )
}

fn tab(c: &mut Chrome, st: &ViewerState) -> Response {
    press(c, st, NamedKey::Tab, Mods::NONE)
}

fn hit(c: &Chrome, t: &Target) -> Rect {
    c.hits
        .iter()
        .rev()
        .find(|(_, h)| h == t)
        .unwrap_or_else(|| panic!("{t:?} not drawn"))
        .0
}

fn actions(r: &Response) -> Vec<Action> {
    r.commands
        .iter()
        .filter_map(|c| match c {
            Command::Action(a) => Some(*a),
            _ => None,
        })
        .collect()
}

/// Puts the keyboard on `t` of the bar with Tab and the arrows.
fn focus_bar(c: &mut Chrome, st: &ViewerState, t: &Target) {
    if !matches!(
        c.kbd,
        Some(Target::Position | Target::Toolbar(_) | Target::ToolbarMore)
    ) {
        c.set_kbd(None);
        assert!(tab(c, st).consumed);
    }
    press(c, st, NamedKey::Home, Mods::NONE);
    let fwd = if c.last_rtl {
        NamedKey::ArrowLeft
    } else {
        NamedKey::ArrowRight
    };
    for _ in 0..c.bar_stops.len() {
        if c.kbd.as_ref() == Some(t) {
            return;
        }
        press(c, st, fwd, Mods::NONE);
    }
    assert_eq!(c.kbd.as_ref(), Some(t), "{t:?} not on the bar");
}

#[test]
fn tab_brings_back_an_auto_hidden_bar() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        let frames = |c: &mut Chrome, text: &mut Text, t0: f64| {
            let mut dl = DrawList::new(1.0);
            for i in 0..40 {
                dl.clear(1.0);
                c.draw(&mut dl, text, st, &Palette::dark(), t0 + i as f64 * 0.016);
            }
        };
        frames(&mut c, &mut text, 1.0);
        frames(&mut c, &mut text, 60.0);
        assert!(c.toolbar_alpha < 0.01, "auto-hidden");
        // Tab still reaches the bar, which comes back while it has the keyboard.
        assert!(tab(&mut c, st).consumed);
        assert_eq!(c.kbd, Some(Target::Position));
        frames(&mut c, &mut text, 61.0);
        assert!(c.toolbar_alpha > 0.99);
        assert_eq!(c.kbd, Some(Target::Position), "still focused once drawn");
        // Back to the image: it hides again.
        assert!(tab(&mut c, st).consumed);
        assert_eq!(c.kbd, None);
        frames(&mut c, &mut text, 120.0);
        assert!(c.toolbar_alpha < 0.01);
        // F6 too.
        press(&mut c, st, NamedKey::F(6), Mods::NONE);
        assert_eq!(c.kbd, Some(Target::Position));
    });
}

fn focused_node(t: &crate::accesskit::TreeUpdate) -> &crate::accesskit::Node {
    &t.nodes.iter().find(|(i, _)| *i == t.focus).unwrap().1
}

#[test]
fn tab_and_f6_cycle_the_regions() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        draw(&mut c, &mut text, st);
        assert_eq!(c.kbd, None);
        // Tab from the image enters the bar, on its first control (the position readout).
        assert!(tab(&mut c, st).consumed);
        assert_eq!(c.kbd, Some(Target::Position));
        assert_eq!(c.bar_stops[0], Target::Position);
        assert!(!c.bar_stops.contains(&Target::Toolbar(T::Spacer)));
        // The bar is one stop: the next Tab goes back to the image.
        assert!(tab(&mut c, st).consumed);
        assert_eq!(c.kbd, None);

        // With the info panel and the indicator up: bar, close, tabs, indicator, hide.
        c.set_info(true, false);
        c.set_insight_indicator(Some(("Why is this slow?".into(), "x".into())));
        draw(&mut c, &mut text, st);
        let mut seen = Vec::new();
        for _ in 0..6 {
            tab(&mut c, st);
            seen.push(c.kbd.clone());
        }
        assert_eq!(
            seen,
            [
                Some(Target::Position),
                Some(Target::InfoClose),
                Some(Target::InfoTab(false)),
                Some(Target::Insight),
                Some(Target::InsightHide),
                None,
            ]
        );
        // Shift+Tab goes back the same way.
        press(&mut c, st, NamedKey::Tab, Mods::SHIFT);
        assert_eq!(c.kbd, Some(Target::InsightHide));
        press(&mut c, st, NamedKey::Tab, Mods::SHIFT);
        assert_eq!(c.kbd, Some(Target::Insight));

        // F6 goes region by region: bar, info panel, indicator, image.
        c.set_kbd(None);
        let mut seen = Vec::new();
        for _ in 0..4 {
            assert!(press(&mut c, st, NamedKey::F(6), Mods::NONE).consumed);
            seen.push(c.kbd.clone());
        }
        assert_eq!(
            seen,
            [
                Some(Target::Position),
                Some(Target::InfoClose),
                Some(Target::Insight),
                None,
            ]
        );
        press(&mut c, st, NamedKey::F(6), Mods::SHIFT);
        assert_eq!(c.kbd, Some(Target::Insight));
        press(&mut c, st, NamedKey::F(6), Mods::SHIFT);
        assert_eq!(c.kbd, Some(Target::InfoClose));

        // The info tabs are one stop: ←/→ move between them, Enter picks one.
        tab(&mut c, st);
        assert_eq!(c.kbd, Some(Target::InfoTab(false)));
        press(&mut c, st, NamedKey::ArrowRight, Mods::NONE);
        assert_eq!(c.kbd, Some(Target::InfoTab(true)));
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert!(c.info_all);

        // Esc puts the keyboard back on the image first, then closes the panel.
        assert!(press(&mut c, st, NamedKey::Escape, Mods::NONE).consumed);
        assert_eq!(c.kbd, None);
        assert!(c.info_open);
        assert!(press(&mut c, st, NamedKey::Escape, Mods::NONE).consumed);
        assert!(!c.info_open);
    });
}

/// A Tab or F6 the keymap binds (DESIGN.md reserves Tab for Viewer ↔ Library) stays the
/// keymap's while the image has the keyboard.
#[test]
fn bound_keys_are_not_taken_from_the_image() {
    let mut text = Text::new();
    let mut km = Keymap::default();
    km.bind(combo(NamedKey::Tab, Mods::NONE), Action::Palette);
    with_keymap(km, |st| {
        let mut c = chrome(1000.0, false);
        draw(&mut c, &mut text, st);
        assert!(!tab(&mut c, st).consumed, "Tab is the keymap's");
        assert_eq!(c.kbd, None);
        assert!(press(&mut c, st, NamedKey::F(6), Mods::NONE).consumed);
        assert_eq!(c.kbd, Some(Target::Position));
        // In the chrome Tab moves on.
        assert!(tab(&mut c, st).consumed);
        assert_eq!(c.kbd, None);
    });
    let mut km = Keymap::default();
    km.bind(combo(NamedKey::F(6), Mods::NONE), Action::Palette);
    with_keymap(km, |st| {
        let mut c = chrome(1000.0, false);
        draw(&mut c, &mut text, st);
        assert!(!press(&mut c, st, NamedKey::F(6), Mods::NONE).consumed);
        assert_eq!(c.kbd, None);
    });
    // Nothing else is taken: without focus the arrows, Enter and Space are the viewer's.
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        draw(&mut c, &mut text, st);
        for k in [
            NamedKey::ArrowLeft,
            NamedKey::ArrowRight,
            NamedKey::Home,
            NamedKey::End,
            NamedKey::Enter,
            NamedKey::Space,
            NamedKey::ArrowUp,
        ] {
            assert!(!press(&mut c, st, k, Mods::NONE).consumed, "{k:?}");
        }
    });
}

#[test]
fn arrows_move_along_the_bar_mirrored_in_rtl() {
    let mut text = Text::new();
    for rtl in [false, true] {
        with_image(|st| {
            let mut c = chrome(1000.0, rtl);
            draw(&mut c, &mut text, st);
            let stops = c.bar_stops.clone();
            assert!(stops.len() > 4);
            tab(&mut c, st);
            // Home and End: the ends of the bar, in its order.
            press(&mut c, st, NamedKey::End, Mods::NONE);
            assert_eq!(c.kbd.as_ref(), stops.last());
            press(&mut c, st, NamedKey::Home, Mods::NONE);
            assert_eq!(c.kbd.as_ref(), stops.first());
            focus_bar(&mut c, st, &Target::Toolbar(T::ZoomOut));
            // → goes to the control on the right on screen, ← to the left, either way.
            let at = |c: &Chrome| hit(c, c.kbd.as_ref().unwrap()).x;
            let x0 = at(&c);
            assert!(press(&mut c, st, NamedKey::ArrowRight, Mods::NONE).consumed);
            let x1 = at(&c);
            assert!(x1 > x0, "rtl {rtl}: {x1} > {x0}");
            press(&mut c, st, NamedKey::ArrowLeft, Mods::NONE);
            press(&mut c, st, NamedKey::ArrowLeft, Mods::NONE);
            assert!(at(&c) < x0, "rtl {rtl}");
            // A Tab away and back returns to the control it was on.
            let on = c.kbd.clone();
            tab(&mut c, st);
            assert_eq!(c.kbd, None);
            tab(&mut c, st);
            assert_eq!(c.kbd, on);
            // The bar stays up while it has the keyboard.
            let mut dl = DrawList::new(1.0);
            c.draw(&mut dl, &mut text, st, &Palette::dark(), 60.0);
            c.draw(&mut dl, &mut text, st, &Palette::dark(), 61.0);
            assert!(c.toolbar_alpha > 0.99, "shown while focused");
            // Keys the bar doesn't use do what they usually do (↑/↓ open only the
            // zoom and More menus).
            focus_bar(&mut c, st, &Target::Toolbar(T::Fit));
            assert!(!press(&mut c, st, NamedKey::ArrowUp, Mods::NONE).consumed);
            assert!(!press(&mut c, st, NamedKey::PageDown, Mods::NONE).consumed);
            // A click hands over to the pointer.
            c.input(
                &Input::PointerButton {
                    x: 500.0,
                    y: 200.0,
                    down: true,
                    button: Button::Primary,
                },
                st,
                62.0,
            );
            assert_eq!(c.kbd, None);
        });
    }
}

#[test]
fn enter_and_space_press_and_hold() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        draw(&mut c, &mut text, st);
        focus_bar(&mut c, st, &Target::Toolbar(T::Next));
        let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(actions(&r), [Action::NextImage]);
        // Let go at once: one step, nothing to end.
        assert!(
            release(&mut c, st, NamedKey::Enter, 2.1)
                .commands
                .is_empty()
        );
        // Held: the key's repeats step on, as holding the button does; letting go ends
        // the flipping.
        let r = press(&mut c, st, NamedKey::Space, Mods::NONE);
        assert_eq!(actions(&r), [Action::NextImage]);
        let r = repeat(&mut c, st, NamedKey::Space, 2.5);
        assert_eq!(
            r.commands,
            [Command::NavHold {
                next: true,
                held: 0.5
            }]
        );
        let r = repeat(&mut c, st, NamedKey::Space, 3.0);
        assert!(matches!(r.commands[..], [Command::NavHold { next: true, held }] if held == 1.0));
        let up = release(&mut c, st, NamedKey::Space, 3.1);
        assert_eq!(up.commands, [Command::NavHoldEnd]);
        // Previous holds backwards.
        focus_bar(&mut c, st, &Target::Toolbar(T::Prev));
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        let r = repeat(&mut c, st, NamedKey::Enter, 2.6);
        assert!(matches!(
            r.commands[..],
            [Command::NavHold { next: false, .. }]
        ));
        release(&mut c, st, NamedKey::Enter, 2.7);
        // A rotation doesn't repeat.
        focus_bar(&mut c, st, &Target::Toolbar(T::RotateLeft));
        let r = repeat(&mut c, st, NamedKey::Enter, 2.8);
        assert!(r.consumed && r.commands.is_empty());
        // Enter on the position readout opens the go-to field.
        focus_bar(&mut c, st, &Target::Position);
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert!(c.goto.is_some());
    });
}

/// More and the zoom readout open their menus from the keyboard; the menus work with
/// ↑/↓, Home/End, Enter/Space and Esc, and Esc leaves the keyboard on their button.
#[test]
fn menus_by_keyboard() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(380.0, false);
        draw(&mut c, &mut text, st);
        let n = c.toolbar_overflow.len();
        assert!(n > 1);
        focus_bar(&mut c, st, &Target::ToolbarMore);
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(c.overflow_menu, Some(Some(0)));
        press(&mut c, st, NamedKey::ArrowDown, Mods::NONE);
        assert_eq!(c.overflow_menu, Some(Some(1)));
        assert!(press(&mut c, st, NamedKey::Escape, Mods::NONE).consumed);
        assert_eq!(c.overflow_menu, None);
        assert_eq!(c.kbd, Some(Target::ToolbarMore));
        press(&mut c, st, NamedKey::ArrowUp, Mods::NONE);
        assert_eq!(c.overflow_menu, Some(Some(n - 1)));
        press(&mut c, st, NamedKey::Escape, Mods::NONE);
        assert_eq!(c.kbd, Some(Target::ToolbarMore));
    });
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        draw(&mut c, &mut text, st);
        let zl = Target::Toolbar(T::ZoomLevel);
        focus_bar(&mut c, st, &zl);
        press(&mut c, st, NamedKey::Space, Mods::NONE);
        assert!(c.zoom_menu);
        assert_eq!(c.zoom_sel, Some(0));
        let mut sels = Vec::new();
        for k in [
            NamedKey::ArrowDown,
            NamedKey::ArrowUp,
            NamedKey::ArrowUp,
            NamedKey::Home,
            NamedKey::End,
            NamedKey::ArrowDown,
        ] {
            assert!(press(&mut c, st, k, Mods::NONE).consumed, "{k:?}");
            sels.push(c.zoom_sel.unwrap());
        }
        assert_eq!(sels, [1, 0, 6, 0, 6, 0]);

        // In the tree the presets are a menu and the selected one has the focus.
        c.set_accessibility(true);
        draw(&mut c, &mut text, st);
        let t = c.accessibility_tree(st, "x", 1.0);
        let menu = t
            .nodes
            .iter()
            .find(|(_, n)| n.role() == Role::Menu)
            .expect("the presets menu");
        assert_eq!(menu.1.children().len(), 7);
        let f = focused_node(&t);
        assert_eq!(f.role(), Role::MenuItem);
        assert_eq!(f.label(), Some("Fit to window"));

        // End is Full screen (tests/keyboard_focus/video_fullscreen.rs); 800 % above it.
        press(&mut c, st, NamedKey::End, Mods::NONE);
        press(&mut c, st, NamedKey::ArrowUp, Mods::NONE);
        let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(actions(&r), [Action::ZoomTo(800.0)]);
        assert!(!c.zoom_menu);
        assert_eq!(c.kbd, Some(zl.clone()));
        // ↑ opens it on the last preset; Esc closes it and keeps the keyboard on the
        // readout; a second Esc goes back to the image.
        press(&mut c, st, NamedKey::ArrowUp, Mods::NONE);
        assert!(c.zoom_menu);
        assert_eq!(c.zoom_sel, Some(6));
        press(&mut c, st, NamedKey::Escape, Mods::NONE);
        assert!(!c.zoom_menu);
        assert_eq!(c.kbd, Some(zl.clone()));
        press(&mut c, st, NamedKey::Escape, Mods::NONE);
        assert_eq!(c.kbd, None);

        // Opened with the pointer, the keys work too.
        c.activate(zl.clone(), st);
        assert!(c.zoom_menu);
        assert_eq!(c.zoom_sel, None);
        press(&mut c, st, NamedKey::ArrowDown, Mods::NONE);
        assert_eq!(c.zoom_sel, Some(0));
        let r = press(&mut c, st, NamedKey::Space, Mods::NONE);
        assert_eq!(actions(&r), [Action::ZoomFit]);
    });
}

#[test]
fn focus_is_in_the_accessibility_tree() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        c.set_accessibility(true);
        draw(&mut c, &mut text, st);
        let before = c.accessibility_tree(st, "x", 1.0).focus;
        focus_bar(&mut c, st, &Target::Toolbar(T::Next));
        draw(&mut c, &mut text, st);
        let t = c.accessibility_tree(st, "x", 1.0);
        assert_ne!(t.focus, before);
        let n = focused_node(&t);
        assert_eq!(n.role(), Role::Button);
        assert_eq!(n.label(), Some("Next image"));
        assert!(n.supports_action(AkAction::Focus));
        // A screen reader moving focus moves the keyboard there.
        let id: NodeId = t
            .nodes
            .iter()
            .find(|(_, n)| n.label() == Some("Rotate anticlockwise"))
            .unwrap()
            .0;
        let r = c.accessibility_action(
            &ActionRequest {
                action: AkAction::Focus,
                target_tree: TreeId::ROOT,
                target_node: id,
                data: None,
            },
            st,
        );
        assert!(r.consumed);
        assert_eq!(c.kbd, Some(Target::Toolbar(T::RotateLeft)));
        // Back on the image the window has the focus again.
        press(&mut c, st, NamedKey::Escape, Mods::NONE);
        draw(&mut c, &mut text, st);
        assert_eq!(c.accessibility_tree(st, "x", 1.0).focus, before);
    });
}

fn content() -> Insights {
    Insights {
        bottleneck: "Reading the files holds images up".into(),
        now: vec![
            InsightItem {
                text: "Copy the folder to an internal drive".into(),
                gain: None,
                action: None,
            },
            InsightItem {
                text: "Turn on the display cache".into(),
                gain: None,
                action: Some(("Turn on".into(), "set:display-cache".into())),
            },
        ],
        dismiss: Some(("Don’t show again".into(), "dismiss:storage".into())),
        restore: Some(("Show hidden insights again".into(), "unhide".into())),
        switch: Some(("Turn insights off".into(), "off".into())),
        ..Insights::default()
    }
}

fn insight_keys(r: &Response) -> Vec<String> {
    r.commands
        .iter()
        .filter_map(|c| match c {
            Command::Insight(k) => Some(k.clone()),
            _ => None,
        })
        .collect()
}

fn has_cmd(r: &Response, f: impl Fn(&Command) -> bool) -> bool {
    r.commands.iter().any(f)
}

use crate::settings::SettingsTarget as S;

/// Draws Settings at each section until a control matching `f` is there.
fn settings_find(c: &mut Chrome, text: &mut Text, st: &ViewerState, f: impl Fn(&S) -> bool) -> S {
    for i in 0..crate::settings::sections().len() {
        c.settings.section = i;
        c.settings.scroll = 0.0;
        draw(c, text, st);
        if let Some(s) = c.settings.stops.iter().find_map(|s| match &s.t {
            Target::Settings(s) if f(s) => Some(s.clone()),
            _ => None,
        }) {
            return s;
        }
    }
    panic!("no such control in Settings");
}

/// Tabs (drawing in between) until the keyboard is on `t`.
fn tab_to(c: &mut Chrome, text: &mut Text, st: &ViewerState, t: &S) {
    for _ in 0..200 {
        if c.modal_focus == Some(Target::Settings(t.clone())) {
            return;
        }
        tab(c, st);
        draw(c, text, st);
    }
    panic!("{t:?} not reached with Tab");
}

/// A primary click on `t` as last drawn.
fn click_on(c: &mut Chrome, st: &ViewerState, t: &Target) -> Response {
    let r = hit(c, t);
    let (x, y) = (r.x + r.w * 0.5, r.y + r.h * 0.5);
    c.input(&Input::PointerMove { x, y }, st, 2.0);
    c.input(
        &Input::PointerButton {
            x,
            y,
            down: true,
            button: Button::Primary,
        },
        st,
        2.0,
    );
    c.input(
        &Input::PointerButton {
            x,
            y,
            down: false,
            button: Button::Primary,
        },
        st,
        2.0,
    )
}

/// Draws a few frames in `pal`; returns the last one.
fn frame(c: &mut Chrome, text: &mut Text, st: &ViewerState, pal: &Palette) -> DrawList {
    let mut dl = DrawList::new(1.0);
    for i in 0..20 {
        dl.clear(1.0);
        c.draw(&mut dl, text, st, pal, 1.0 + i as f64 * 0.05);
    }
    dl
}

/// The outlines (a border, no fill) drawn just around `r`: x, y, w, h, border width.
fn outlines_around(dl: &DrawList, r: Rect) -> Vec<[f32; 5]> {
    dl.prims
        .iter()
        .filter(|p| {
            let [x, y, w, h] = p.shape;
            p.params[2] == specular_gfx::draw::kind::RECT
                && p.params[0] > 0.0
                && p.color[3] == 0.0
                && p.color2[3] > 0.0
                && x < r.x
                && y < r.y
                && x + w > r.right()
                && y + h > r.bottom()
                && x > r.x - 12.0
                && x + w < r.right() + 12.0
        })
        .map(|p| [p.shape[0], p.shape[1], p.shape[2], p.shape[3], p.params[0]])
        .collect()
}

/// Where the selection of the palette or the language list is.
fn selected(c: &Chrome) -> usize {
    match c.modal {
        Some(crate::Modal::Palette) => c.palette.selected,
        _ => c.language.selected,
    }
}

/// What was typed in the palette's or the language list's field.
fn query(c: &Chrome) -> String {
    match c.modal {
        Some(crate::Modal::Palette) => c.palette.query.clone(),
        _ => c.language.query.clone(),
    }
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
}

fn presenting_hit(c: &Chrome, t: &Target) -> Option<Rect> {
    c.hits.iter().rev().find(|(_, h)| h == t).map(|h| h.0)
}

fn work_chip() -> crate::WorkChip {
    crate::WorkChip {
        label: "AI search · 1,240 of 9,800".into(),
        name: "Background work: AI search · 1,240 of 9,800".into(),
        paused: false,
    }
}

fn work_panel() -> crate::WorkPanel {
    let row = |title: &str, fraction, pause: bool| crate::WorkRow {
        title: title.into(),
        step: "Indexing photos for AI search".into(),
        count: "1,240 of 9,800 photos · 8,560 to go".into(),
        fraction,
        status: "About 25 minutes left".into(),
        paused: false,
        then: Some("Then: Finding faces".into()),
        pause: pause.then(|| ("Pause".into(), format!("pause:{title}"))),
        settings: Some(("Settings…".into(), format!("settings:{title}"))),
    };
    crate::WorkPanel {
        rows: vec![
            row("search", Some(0.1265), true),
            row("library", None, false),
        ],
    }
}

fn work_keys(r: &Response) -> Vec<String> {
    r.commands
        .iter()
        .filter_map(|c| match c {
            Command::Work(k) => Some(k.clone()),
            _ => None,
        })
        .collect()
}

mod dialogs_focus;
mod downloads_focus;
mod focus_rings;
mod goto_field;
mod info_rows;
mod insights_focus;
mod palette_focus;
mod presenting_focus;
mod setting_fields;
mod settings_focus;
mod settings_keys;
mod text_input;
mod video_fullscreen;
mod volume_slider;
