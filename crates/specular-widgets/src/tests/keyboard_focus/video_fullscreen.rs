use super::*;
use crate::{Playback, VideoPlayback};

/// A movie on screen (in full screen or not, playing or paused).
fn with_movie<R>(fullscreen: bool, playing: bool, f: impl FnOnce(&mut ViewerState) -> R) -> R {
    with_sound(fullscreen, playing, Some((0.8, false)), 2.0, f)
}

/// A movie on screen with `sound` (volume, muted) and the volume slider's top `max`.
pub(super) fn with_sound<R>(
    fullscreen: bool,
    playing: bool,
    sound: Option<(f32, bool)>,
    max: f32,
    f: impl FnOnce(&mut ViewerState) -> R,
) -> R {
    let hud = HudStats::default();
    let store = SettingsStore::in_memory();
    let settings = Settings::default();
    let keymap = Keymap::default();
    let mut st = ViewerState {
        actions: ActionState {
            image: ImageState::Ready,
            count: 4,
            animated: true,
            video: true,
            sound: true,
            sound_file: false,
            profile: specular_input::Profile::UNRESTRICTED,
            ..ActionState::default()
        },
        has_image: true,
        index: 1,
        count: 4,
        zoom: 1.0,
        at_fit: true,
        fullscreen,
        file_name: Some("harbour.mp4"),
        dims: Some((1920, 1080)),
        loading: false,
        error: None,
        truncated: false,
        metadata: None,
        hud: &hud,
        keymap: &keymap,
        store: &store,
        settings: &settings,
        overlays: Overlays::default(),
        playback: Some(Playback {
            frame: 10,
            count: Some(100),
            playing,
            speed: 1.0,
            video: Some(VideoPlayback {
                position: 4.0,
                duration: 40.0,
                looping: false,
                sound,
                volume_max: max,
                picture: true,
            }),
        }),
        motion: None,
        spread: None,
        pages: None,
    };
    f(&mut st)
}

fn draw_at(c: &mut Chrome, text: &mut Text, st: &ViewerState, now: f64) {
    let mut dl = DrawList::new(1.0);
    c.draw(&mut dl, text, st, &Palette::dark(), now);
}

fn shown(c: &Chrome, t: &Target) -> bool {
    c.hits.iter().any(|(_, h)| h == t)
}

/// The zoom presets end with Full screen: it asks the app to enter full screen, is ticked
/// for screen readers while the window is full screen, and choosing it then changes nothing.
#[test]
fn the_zoom_presets_offer_full_screen() {
    let mut text = Text::new();
    for fullscreen in [false, true] {
        with_movie(fullscreen, false, |st| {
            let mut c = chrome(1000.0, false);
            draw(&mut c, &mut text, st);
            c.input(&Input::PointerMove { x: 500.0, y: 699.0 }, st, 1.0);
            draw(&mut c, &mut text, st);
            let zl = Target::Toolbar(T::ZoomLevel);
            focus_bar(&mut c, st, &zl);
            press(&mut c, st, NamedKey::Space, Mods::NONE);
            assert!(c.zoom_menu);
            press(&mut c, st, NamedKey::End, Mods::NONE);
            assert_eq!(c.zoom_sel, Some(6));
            c.set_accessibility(true);
            draw(&mut c, &mut text, st);
            let t = c.accessibility_tree(st, "x", 1.0);
            let n = focused_node(&t);
            assert_eq!(n.role(), Role::MenuItemCheckBox);
            assert_eq!(n.label(), Some("Full screen"));
            assert_eq!(
                n.toggled(),
                Some(if fullscreen {
                    crate::accesskit::Toggled::True
                } else {
                    crate::accesskit::Toggled::False
                })
            );
            let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
            assert!(!c.zoom_menu);
            if fullscreen {
                assert!(r.consumed);
                assert!(r.commands.is_empty());
            } else {
                assert_eq!(r.commands, [Command::EnterFullscreen]);
            }
        });
    }
    // A fit entry while full screen is only the fit (full screen stays).
    with_movie(true, false, |st| {
        let mut c = chrome(1000.0, false);
        draw(&mut c, &mut text, st);
        let r = c.activate(Target::ZoomPreset(Action::ZoomFit), st);
        assert_eq!(actions(&r), [Action::ZoomFit]);
    });
}

/// Opening a track or moving over the media leaves the bottom controls hidden.
#[test]
fn media_controls_follow_the_bottom_edge_while_playing_or_paused() {
    let mut text = Text::new();
    for (sound_file, full) in [(true, false), (false, false), (false, true)] {
        for playing in [false, true] {
            with_movie(full, playing, |st| {
                st.actions.sound_file = sound_file;
                st.playback
                    .as_mut()
                    .unwrap()
                    .video
                    .as_mut()
                    .unwrap()
                    .picture = !sound_file;
                let mut c = chrome(1000.0, false);
                c.set_video_fullscreen(full, 1.0);
                let play = Target::Transport(Action::PlayPause);
                let toolbar = Target::Toolbar(T::ZoomLevel);
                c.input(&Input::PointerMove { x: 500.0, y: 150.0 }, st, 1.0);
                draw_at(&mut c, &mut text, st, 1.0);
                assert!(!shown(&c, &play) && !shown(&c, &toolbar));
                c.reduce_motion = true;
                c.poke(2.0);
                draw_at(&mut c, &mut text, st, 2.0);
                assert!(!shown(&c, &play));
                // The activation area includes the seek row at the top of the bar.
                let top = crate::transport::top(c.size.1, true, f32::from(full));
                c.input(
                    &Input::PointerMove {
                        x: 500.0,
                        y: top + 2.0,
                    },
                    st,
                    3.0,
                );
                draw_at(&mut c, &mut text, st, 3.0);
                assert!(shown(&c, &play));
                if sound_file || !playing || full {
                    assert_eq!(shown(&c, &toolbar), !full);
                }
                draw_at(&mut c, &mut text, st, 30.0);
                assert!(shown(&c, &play), "a resting pointer keeps the controls up");
                c.input(&Input::PointerMove { x: 500.0, y: 150.0 }, st, 31.0);
                draw_at(&mut c, &mut text, st, 31.0);
                assert!(!shown(&c, &play) && !shown(&c, &toolbar));
                c.input(&Input::PointerMove { x: 500.0, y: 699.0 }, st, 32.0);
                draw_at(&mut c, &mut text, st, 32.0);
                c.input(&Input::PointerLeave, st, 33.0);
                draw_at(&mut c, &mut text, st, 33.0);
                assert!(!shown(&c, &play));
            });
        }
    }
}

#[test]
fn player_controls_stay_available_to_the_keyboard_and_during_drags() {
    let mut text = Text::new();
    for full in [false, true] {
        with_movie(full, true, |st| {
            let mut c = chrome(1000.0, false);
            c.reduce_motion = true;
            c.set_info(true, false);
            c.set_video_fullscreen(full, 1.0);
            draw_at(&mut c, &mut text, st, 1.0);
            let play = Target::Transport(Action::PlayPause);
            assert!(!shown(&c, &play));
            tab(&mut c, st);
            draw_at(&mut c, &mut text, st, 3.0);
            assert!(
                shown(&c, &play),
                "Tab reveals controls before the first hover"
            );
            c.set_kbd(None);
            c.set_player(Some(crate::PlayerView::default()));
            c.toggle_player_menu(crate::PlayerMenu::Speed);
            draw_at(&mut c, &mut text, st, 3.5);
            assert!(shown(&c, &play), "an open player menu keeps its anchor");
            c.toggle_player_menu(crate::PlayerMenu::Speed);
            for handle in [Target::SeekBar, Target::Volume] {
                c.input(&Input::PointerMove { x: 500.0, y: 699.0 }, st, 4.0);
                draw_at(&mut c, &mut text, st, 4.0);
                let (x, y) = hit(&c, &handle).center();
                let button = |x, y, down| Input::PointerButton {
                    x,
                    y,
                    down,
                    button: Button::Primary,
                };
                c.input(&button(x, y, true), st, 5.0);
                c.input(&Input::PointerMove { x, y: 100.0 }, st, 5.1);
                draw_at(&mut c, &mut text, st, 20.0);
                assert!(shown(&c, &handle), "drag kept {handle:?} visible");
                c.input(&button(x, 100.0, false), st, 21.0);
                draw_at(&mut c, &mut text, st, 21.0);
                assert!(!shown(&c, &handle), "released outside the controls");
            }
            if full {
                assert!(!shown(&c, &Target::InfoClose));
                c.set_video_fullscreen(false, 22.0);
                draw_at(&mut c, &mut text, st, 22.0);
                assert!(shown(&c, &Target::InfoClose));
            }
        });
    }
}

/// The transport's full-screen button is named for what it does.
#[test]
fn the_full_screen_button_says_enter_or_exit() {
    with_movie(false, true, |st| {
        assert_eq!(
            crate::transport::label(Action::VideoFullscreen, st),
            "Enter full screen"
        );
    });
    with_movie(true, true, |st| {
        assert_eq!(
            crate::transport::label(Action::VideoFullscreen, st),
            "Exit full screen"
        );
    });
}
