//! The volume slider beside the mute button (volume.rs).

use super::video_fullscreen::with_sound;
use super::*;

fn volumes(r: &Response) -> Vec<(f32, bool)> {
    r.commands
        .iter()
        .filter_map(|c| match c {
            Command::Volume { volume, done } => Some((*volume, *done)),
            _ => None,
        })
        .collect()
}

fn pointer(c: &mut Chrome, st: &ViewerState, p: (f32, f32), down: Option<bool>) -> Response {
    let r = c.input(&Input::PointerMove { x: p.0, y: p.1 }, st, 2.0);
    match down {
        Some(down) => c.input(
            &Input::PointerButton {
                x: p.0,
                y: p.1,
                down,
                button: Button::Primary,
            },
            st,
            2.0,
        ),
        None => r,
    }
}

/// The part of the window the transport's controls take.
fn transport_span(c: &Chrome) -> (f32, f32) {
    c.hits
        .iter()
        .filter(|(_, t)| matches!(t, Target::Transport(_) | Target::Volume | Target::SeekBar))
        .fold((f32::MAX, f32::MIN), |(a, b), (r, _)| {
            (a.min(r.x), b.max(r.right()))
        })
}

/// Shown right after the mute button wherever it is: a movie, full-screen play, a sound
/// file; not without sound; dropped (the mute button kept) when the window is too narrow,
/// and the bar then still fits.
#[test]
fn sits_beside_the_mute_button_while_there_is_room() {
    let mut text = Text::new();
    for fullscreen in [false, true] {
        with_sound(fullscreen, false, Some((0.8, false)), 2.0, |st| {
            let mut c = chrome(1000.0, false);
            if fullscreen {
                c.set_video_fullscreen(true, 1.0);
            }
            draw(&mut c, &mut text, st);
            let mute = hit(&c, &Target::Transport(Action::ToggleMute));
            let v = hit(&c, &Target::Volume);
            assert!((v.x - mute.right()).abs() < 4.0, "{mute:?} {v:?}");
            assert_eq!(v.w, crate::volume::WIDTH);
            let (a, b) = transport_span(&c);
            assert!(a >= 0.0 && b <= 1000.0);
        });
    }
    with_sound(false, false, None, 2.0, |st| {
        let mut c = chrome(1000.0, false);
        draw(&mut c, &mut text, st);
        assert!(!c.hits.iter().any(|(_, t)| *t == Target::Volume));
    });
    // Narrow: no slider, the mute button stays and the bar fits.
    with_sound(false, false, Some((0.8, false)), 2.0, |st| {
        let mut wide = chrome(1000.0, false);
        draw(&mut wide, &mut text, st);
        let (a, b) = transport_span(&wide);
        let need = b - a;
        let w = need + 2.0 * 16.0 + 20.0 - crate::volume::WIDTH;
        let mut c = chrome(w, false);
        draw(&mut c, &mut text, st);
        assert!(!c.hits.iter().any(|(_, t)| *t == Target::Volume), "{w}");
        hit(&c, &Target::Transport(Action::ToggleMute));
        let (a, b) = transport_span(&c);
        assert!(a >= 0.0 && b <= w, "{a} {b} {w}");
    });
}

/// Press sets the volume under the pointer, a drag follows it (the knob too, before the
/// sound reports it), letting go ends it; the middle of the track is normal.
#[test]
fn press_and_drag_set_the_volume() {
    let mut text = Text::new();
    with_sound(false, false, Some((0.8, false)), 2.0, |st| {
        let mut c = chrome(1000.0, false);
        draw(&mut c, &mut text, st);
        let r = hit(&c, &Target::Volume);
        let t = crate::volume::track(r);
        let y = r.center().1;
        let got = volumes(&pointer(&mut c, st, (t.x + t.w * 0.5, y), Some(true)));
        assert_eq!(got, [(1.0, false)]);
        assert_eq!(c.pressed, Some(Target::Volume));
        let got = volumes(&pointer(&mut c, st, (t.right() + 40.0, y + 30.0), None));
        assert_eq!(got, [(2.0, false)], "a drag keeps going off the slider");
        assert_eq!(c.volume_now(st).map(|v| v.0), Some(2.0));
        let got = volumes(&pointer(&mut c, st, (t.x + t.w * 0.25, y), Some(false)));
        assert_eq!(got, [(0.5, true)]);
        assert!(c.volume_drag.is_none() && c.pressed.is_none());
        // A plain move afterwards changes nothing.
        assert!(volumes(&pointer(&mut c, st, (t.x, y), None)).is_empty());
    });
    // A cast device's volume: the track is its 0..=100 %.
    with_sound(false, false, Some((0.3, false)), 1.0, |st| {
        let mut c = chrome(1000.0, false);
        draw(&mut c, &mut text, st);
        let r = hit(&c, &Target::Volume);
        let t = crate::volume::track(r);
        let got = volumes(&pointer(&mut c, st, (t.right(), r.center().1), Some(true)));
        assert_eq!(got, [(1.0, false)]);
    });
}

/// The wheel over it: 5 % a notch, away louder; a touchpad's small turns add up.
#[test]
fn the_wheel_steps_it() {
    let mut text = Text::new();
    with_sound(false, false, Some((0.8, false)), 2.0, |st| {
        let mut c = chrome(1000.0, false);
        draw(&mut c, &mut text, st);
        let (x, y) = hit(&c, &Target::Volume).center();
        let wheel = |c: &mut Chrome, dy: f32| c.input(&Input::Wheel { x, y, dx: 0.0, dy }, st, 2.0);
        let r = wheel(&mut c, 1.0);
        assert!(r.consumed);
        assert_eq!(volumes(&r), [(0.85, true)]);
        c.volume_set = None;
        assert_eq!(volumes(&wheel(&mut c, -2.0)), [(0.7, true)]);
        c.volume_set = None;
        assert!(volumes(&wheel(&mut c, 0.1)).is_empty());
        assert_eq!(volumes(&wheel(&mut c, 0.15)), [(0.81, true)]);
    });
}

/// A Tab stop of its own after the transport's buttons; arrows step 5 % (→ louder in
/// right-to-left languages too: media controls run left to right), Home and End go to the
/// ends.
#[test]
fn keys_step_it_when_focused() {
    let mut text = Text::new();
    for rtl in [false, true] {
        with_sound(false, false, Some((0.8, false)), 2.0, |st| {
            let mut c = chrome(1000.0, rtl);
            draw(&mut c, &mut text, st);
            let stops = c.stops();
            let i = stops
                .iter()
                .position(|s| s.targets == [Target::Volume])
                .expect("a stop of its own");
            assert!(
                stops[i - 1]
                    .targets
                    .contains(&Target::Transport(Action::ToggleMute))
            );
            c.set_kbd(Some(Target::Volume));
            let r = press(&mut c, st, NamedKey::ArrowRight, Mods::NONE);
            assert_eq!(volumes(&r), [(0.85, true)]);
            // Steps add up before the sound reports them.
            let r = press(&mut c, st, NamedKey::ArrowUp, Mods::NONE);
            assert_eq!(volumes(&r), [(0.9, true)]);
            c.volume_set = None;
            assert_eq!(
                volumes(&press(&mut c, st, NamedKey::ArrowLeft, Mods::NONE)),
                [(0.75, true)]
            );
            assert_eq!(
                volumes(&press(&mut c, st, NamedKey::End, Mods::NONE)),
                [(2.0, true)]
            );
            assert_eq!(
                volumes(&press(&mut c, st, NamedKey::Home, Mods::NONE)),
                [(0.0, true)]
            );
            assert_eq!(c.kbd, Some(Target::Volume), "the focus stays on it");
        });
    }
}

/// Screen readers: a slider named Volume whose value is the percentage; it steps and takes
/// a value.
#[test]
fn screen_readers_read_and_set_it() {
    let mut text = Text::new();
    for (muted, value) in [(false, "80%"), (true, "80% · Sound muted")] {
        with_sound(false, false, Some((0.8, muted)), 2.0, |st| {
            let mut c = chrome(1000.0, false);
            c.set_accessibility(true);
            draw(&mut c, &mut text, st);
            c.set_kbd(Some(Target::Volume));
            draw(&mut c, &mut text, st);
            let t = c.accessibility_tree(st, "x", 1.0);
            let n = focused_node(&t);
            assert_eq!(n.role(), Role::Slider);
            assert_eq!(n.label(), Some("Volume"));
            assert_eq!(n.value(), Some(value));
            assert_eq!(n.numeric_value(), Some(80.0));
            assert_eq!(n.max_numeric_value(), Some(200.0));
            assert!(n.supports_action(AkAction::Increment));
            let id = t.focus;
            let req = |action, data| ActionRequest {
                action,
                target_tree: TreeId::ROOT,
                target_node: id,
                data,
            };
            let r = c.accessibility_action(&req(AkAction::Increment, None), st);
            assert_eq!(volumes(&r), [(0.85, true)]);
            c.volume_set = None;
            let r = c.accessibility_action(
                &req(
                    AkAction::SetValue,
                    Some(crate::accesskit::ActionData::NumericValue(120.0)),
                ),
                st,
            );
            assert_eq!(volumes(&r), [(1.2, true)]);
        });
    }
}

/// In full-screen play with the controls faded out, Tab reaches the volume slider
/// as well as the buttons (it used to wait until they came back), and the focus brings it
/// back on screen.
#[test]
fn tab_reaches_it_when_full_screen_controls_have_faded() {
    let mut text = Text::new();
    with_sound(true, true, Some((0.8, false)), 2.0, |st| {
        let mut c = chrome(1000.0, false);
        c.reduce_motion = true;
        c.set_video_fullscreen(true, 1.0);
        let draw_at = |c: &mut Chrome, text: &mut Text, now: f64| {
            let mut dl = DrawList::new(1.0);
            c.draw(&mut dl, text, st, &Palette::dark(), now);
        };
        draw_at(&mut c, &mut text, 1.1);
        assert!(c.hits.iter().any(|(_, t)| *t == Target::Volume));
        draw_at(&mut c, &mut text, 30.0);
        assert!(!c.hits.iter().any(|(_, t)| *t == Target::Volume), "faded");
        let mut seen = Vec::new();
        for _ in 0..6 {
            tab(&mut c, st);
            seen.push(c.kbd.clone());
            if c.kbd == Some(Target::Volume) {
                break;
            }
        }
        assert_eq!(c.kbd, Some(Target::Volume), "{seen:?}");
        draw_at(&mut c, &mut text, 31.0);
        assert!(c.hits.iter().any(|(_, t)| *t == Target::Volume), "back");
    });
}
