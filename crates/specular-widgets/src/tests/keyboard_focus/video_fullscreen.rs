use super::*;
use crate::{PLAY_CONTROLS_IDLE, Playback, VideoPlayback};

/// A movie on screen (in full screen or not, playing or paused).
fn with_movie<R>(fullscreen: bool, playing: bool, f: impl FnOnce(&ViewerState) -> R) -> R {
    with_sound(fullscreen, playing, Some((0.8, false)), 2.0, f)
}

/// A movie on screen with `sound` (volume, muted) and the volume slider's top `max`.
pub(super) fn with_sound<R>(
    fullscreen: bool,
    playing: bool,
    sound: Option<(f32, bool)>,
    max: f32,
    f: impl FnOnce(&ViewerState) -> R,
) -> R {
    let hud = HudStats::default();
    let store = SettingsStore::in_memory();
    let settings = Settings::default();
    let keymap = Keymap::default();
    let st = ViewerState {
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
    f(&st)
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

/// Full-screen play mode: no bar, panel or edge arrows; the transport at the bottom with
/// the full-screen button, hidden 2 s after the last move while playing, back on a move or a
/// key, kept while paused or under the pointer; the panel comes back on leaving.
#[test]
fn play_mode_shows_only_the_transport_and_hides_it_when_idle() {
    let mut text = Text::new();
    with_movie(true, true, |st| {
        let mut c = chrome(1000.0, false);
        c.reduce_motion = true;
        c.set_info(true, false);
        draw_at(&mut c, &mut text, st, 1.0);
        // Playing in the window: the toolbar gives its place to the transport.
        assert!(!shown(&c, &Target::Toolbar(T::ZoomLevel)));
        assert!(shown(&c, &Target::Transport(Action::PlayPause)));
        assert!(c.info_inset() > 0.0);

        c.set_video_fullscreen(true, 1.0);
        assert!(c.video_fullscreen());
        draw_at(&mut c, &mut text, st, 1.1);
        assert!(!c.hits.iter().any(|(_, h)| matches!(h, Target::Toolbar(_))));
        assert!(!c.hits.iter().any(|(_, h)| matches!(h, Target::InfoClose)));
        assert_eq!(c.info_inset(), 0.0);
        assert!(shown(&c, &Target::Transport(Action::VideoFullscreen)));
        let play = c
            .hits
            .iter()
            .find(|(_, h)| *h == Target::Transport(Action::PlayPause))
            .unwrap()
            .0;
        // At the bottom of the screen, where the toolbar was.
        assert!(play.bottom() > 700.0 - 70.0, "{play:?}");
        assert!(!c.chrome_hidden());

        // Idle while playing: gone, and the pointer may hide.
        draw_at(&mut c, &mut text, st, 1.0 + PLAY_CONTROLS_IDLE + 0.1);
        assert!(!shown(&c, &Target::Transport(Action::PlayPause)));
        assert!(c.chrome_hidden());
        // A key brings them back; so does a move.
        press_at(&mut c, st, NamedKey::F(9), 4.0);
        draw_at(&mut c, &mut text, st, 4.0);
        assert!(shown(&c, &Target::Transport(Action::PlayPause)));
        draw_at(&mut c, &mut text, st, 7.0);
        assert!(!shown(&c, &Target::Transport(Action::PlayPause)));
        c.input(&Input::PointerMove { x: 100.0, y: 100.0 }, st, 7.5);
        draw_at(&mut c, &mut text, st, 7.5);
        assert!(shown(&c, &Target::Transport(Action::PlayPause)));
        // Under the pointer they stay.
        let r = c
            .hits
            .iter()
            .find(|(_, h)| *h == Target::Transport(Action::PlayPause))
            .unwrap()
            .0;
        c.input(
            &Input::PointerMove {
                x: r.center().0,
                y: r.center().1,
            },
            st,
            8.0,
        );
        draw_at(&mut c, &mut text, st, 20.0);
        assert!(shown(&c, &Target::Transport(Action::PlayPause)));

        // Faded out, Tab still reaches them (and only them).
        c.input(&Input::PointerMove { x: 100.0, y: 100.0 }, st, 21.0);
        draw_at(&mut c, &mut text, st, 30.0);
        assert!(!shown(&c, &Target::Transport(Action::PlayPause)));
        tab(&mut c, st);
        assert!(matches!(c.kbd, Some(Target::Transport(_))), "{:?}", c.kbd);
        draw_at(&mut c, &mut text, st, 40.0);
        assert!(shown(&c, &Target::Transport(Action::PlayPause)));

        c.set_video_fullscreen(false, 41.0);
        draw_at(&mut c, &mut text, st, 41.0);
        assert!(c.info_inset() > 0.0);
        assert!(shown(&c, &Target::InfoClose));
    });
    // Paused: the controls stay.
    with_movie(true, false, |st| {
        let mut c = chrome(1000.0, false);
        c.reduce_motion = true;
        c.set_video_fullscreen(true, 1.0);
        draw_at(&mut c, &mut text, st, 30.0);
        assert!(shown(&c, &Target::Transport(Action::PlayPause)));
        assert!(!c.chrome_hidden());
    });
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

fn press_at(c: &mut Chrome, st: &ViewerState, k: NamedKey, now: f64) -> Response {
    c.input(
        &Input::Key {
            combo: combo(k, Mods::NONE),
            repeat: false,
        },
        st,
        now,
    )
}
