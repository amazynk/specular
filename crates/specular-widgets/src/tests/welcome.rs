use crate::ctx::Target;
use crate::welcome::WelcomeTarget as W;
use crate::{Chrome, Command, HudStats, Input, Overlays, Response, ViewerState};
use crate::{RecentFolder, TipKind, Welcome, WelcomeCommand, tips};
use specular_gfx::{DrawList, Text};
use specular_input::{
    Action, ActionState, Capabilities, Capability, Key, KeyCombo, Keymap, Mods, NamedKey, Profile,
};
use specular_settings::{OnStart, Settings, SettingsStore};
use specular_theme::Palette;

fn empty(profile: Profile) -> ActionState {
    ActionState {
        profile,
        ..Default::default()
    }
}

fn with<R>(
    keymap: Keymap,
    actions: ActionState,
    settings: Settings,
    f: impl FnOnce(&ViewerState) -> R,
) -> R {
    let hud = HudStats::default();
    let store = SettingsStore::in_memory();
    let st = ViewerState {
        actions,
        has_image: false,
        index: 0,
        count: 0,
        zoom: 1.0,
        at_fit: true,
        fullscreen: false,
        file_name: None,
        dims: None,
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

fn general() -> Profile {
    Profile::of(Capabilities::GENERAL)
}

fn keymap(profile: Profile) -> Keymap {
    let mut k = Keymap::default();
    k.set_profile(profile);
    k
}

fn draw(c: &mut Chrome, text: &mut Text, st: &ViewerState) {
    let mut dl = DrawList::new(1.0);
    for i in 0..4 {
        dl.clear(1.0);
        c.draw(&mut dl, text, st, &Palette::dark(), 1.0 + i as f64 * 0.016);
    }
}

fn press(c: &mut Chrome, st: &ViewerState, k: NamedKey) -> Response {
    c.input(
        &Input::Key {
            combo: KeyCombo::new(Key::Named(k), Mods::NONE),
            repeat: false,
        },
        st,
        2.0,
    )
}

fn kinds(t: &[crate::Tip]) -> Vec<TipKind> {
    t.iter().map(|t| t.kind).collect()
}

fn is_medical(k: TipKind) -> bool {
    matches!(
        k,
        TipKind::Studies | TipKind::DicomServers | TipKind::DicomTags | TipKind::WindowLevel
    )
}

/// The cards show the keymap's own keys; a card whose action is unbound, or hidden by
/// the profile, is left out; a profile that is on adds its first card with a key.
#[test]
fn cards_follow_the_keymap_and_the_profile() {
    let p = general();
    let all = tips(&keymap(p), &empty(p), false);
    assert_eq!(
        kinds(&all),
        [
            TipKind::Gallery,
            TipKind::Compare,
            TipKind::Cull,
            TipKind::Slideshow,
            TipKind::Palette
        ]
    );
    let key = |t: &[crate::Tip], k| t.iter().find(|t| t.kind == k).map(|t| t.key.clone());
    assert_eq!(key(&all, TipKind::Slideshow).as_deref(), Some("S"));
    // Rebound: the card shows the new key. Unbound: the card is gone.
    let mut km = keymap(p);
    km.unbind(KeyCombo::parse("s").unwrap());
    km.bind(KeyCombo::parse("F7").unwrap(), Action::Slideshow);
    let t = tips(&km, &empty(p), false);
    assert_eq!(key(&t, TipKind::Slideshow).as_deref(), Some("F7"));
    km.unbind(KeyCombo::parse("F7").unwrap());
    let t = tips(&km, &empty(p), false);
    assert!(!kinds(&t).contains(&TipKind::Slideshow), "{t:?}");
    // Without an image, only what needs none runs; the others say what to do first.
    let pal = all.iter().find(|t| t.kind == TipKind::Palette).unwrap();
    assert_eq!(pal.run, Some(Action::Palette));
    let show = all.iter().find(|t| t.kind == TipKind::Slideshow).unwrap();
    assert_eq!(show.run, None);
    assert!(crate::tip_later(show).contains('s'));
    assert!(!all.iter().any(|t| is_medical(t.kind)));

    // Medical on: a medical card, and never more than six.
    let med = Profile::of(Capabilities::GENERAL.with(Capability::Medical));
    let t = tips(&keymap(med), &empty(med), false);
    assert!(t.len() <= 6);
    assert!(t.iter().any(|t| is_medical(t.kind)), "{t:?}");
}

/// Every control is a Tab stop in reading order; Enter presses it. Open runs Open,
/// Browse, Your pictures and a recent folder ask the application, a card that needs an
/// image says so, and "Hide these tips" stores the setting.
#[test]
fn keyboard_and_commands() {
    let mut text = Text::new();
    let p = general();
    with(keymap(p), empty(p), Settings::default(), |st| {
        let mut c = Chrome::new();
        c.set_size(1000.0, 800.0);
        c.set_welcome(Welcome {
            recents: vec![RecentFolder {
                path: "/photos/2026".into(),
                name: "2026".into(),
                parent: "/photos".into(),
                thumb: None,
            }],
            pictures_offer: None,
        });
        draw(&mut c, &mut text, st);
        assert!(c.welcome_shown());
        let stops = crate::welcome::stops(&c.hits);
        assert_eq!(
            stops[..4],
            [
                Target::Welcome(W::Open),
                Target::Welcome(W::Browse),
                Target::Welcome(W::Pictures),
                Target::Welcome(W::Recent(0)),
            ]
        );
        assert!(stops.contains(&Target::Welcome(W::HideTips)));
        press(&mut c, st, NamedKey::Tab);
        assert_eq!(c.kbd, Some(Target::Welcome(W::Open)));
        let r = press(&mut c, st, NamedKey::Enter);
        assert_eq!(r.commands, [Command::Action(Action::Open)]);

        let r = c.activate(Target::Welcome(W::Browse), st);
        assert_eq!(r.commands, [Command::Welcome(WelcomeCommand::Browse)]);
        let r = c.activate(Target::Welcome(W::Pictures), st);
        assert_eq!(r.commands, [Command::Welcome(WelcomeCommand::Pictures)]);
        let r = c.activate(Target::Welcome(W::Recent(0)), st);
        assert_eq!(
            r.commands,
            [Command::Welcome(WelcomeCommand::Folder(
                "/photos/2026".into()
            ))]
        );

        // The palette card runs; the slideshow card explains (no image yet).
        let t = c.welcome_tips(st);
        let at = |k| t.iter().position(|t| t.kind == k).unwrap();
        let r = c.activate(Target::Welcome(W::Tip(at(TipKind::Palette))), st);
        assert_eq!(r.commands, [Command::Action(Action::Palette)]);
        let before = c.toasts.len();
        let r = c.activate(Target::Welcome(W::Tip(at(TipKind::Slideshow))), st);
        assert!(r.commands.is_empty());
        assert_eq!(c.toasts.len(), before + 1);
        assert!(
            c.toasts
                .last()
                .unwrap()
                .text
                .contains("Open an image first")
        );

        c.activate(Target::Welcome(W::HideTips), st);
        assert!(!st.store.snapshot().general.start_tips);
    });
    // Tips hidden: no cards and no link.
    let mut s = Settings::default();
    s.general.start_tips = false;
    with(keymap(p), empty(p), s, |st| {
        let mut c = Chrome::new();
        c.set_size(1000.0, 800.0);
        draw(&mut c, &mut text, st);
        let stops = crate::welcome::stops(&c.hits);
        assert!(
            !stops
                .iter()
                .any(|s| matches!(s, Target::Welcome(W::Tip(_) | W::HideTips)))
        );
        assert!(stops.contains(&Target::Welcome(W::Open)));
    });
}

/// The first-run sheet's start choice: ↓ chooses the next, Continue stores it; Skip
/// stores nothing.
#[test]
fn first_run_start_choice() {
    let mut text = Text::new();
    let p = Profile::UNRESTRICTED;
    with(keymap(p), empty(p), Settings::default(), |st| {
        let mut c = Chrome::new();
        c.set_size(900.0, 700.0);
        c.set_profile_start(OnStart::StartScreen);
        c.ask_profile(Capabilities::GENERAL);
        draw(&mut c, &mut text, st);
        c.modal_focus = Some(Target::ProfileStart(OnStart::StartScreen));
        press(&mut c, st, NamedKey::ArrowDown);
        assert_eq!(c.modal_focus, Some(Target::ProfileStart(OnStart::Gallery)));
        press(&mut c, st, NamedKey::ArrowDown);
        assert_eq!(c.modal_focus, Some(Target::ProfileStart(OnStart::Library)));
        c.activate(Target::ProfileDone, st);
        assert_eq!(c.modal(), None);
        assert_eq!(st.store.snapshot().general.on_start, OnStart::Library);
    });
    with(keymap(p), empty(p), Settings::default(), |st| {
        let mut c = Chrome::new();
        c.set_size(900.0, 700.0);
        c.ask_profile(Capabilities::GENERAL);
        draw(&mut c, &mut text, st);
        c.activate(Target::ProfileStart(OnStart::Gallery), st);
        c.modal_focus = Some(Target::ProfileSkip);
        press(&mut c, st, NamedKey::Enter);
        assert_eq!(c.modal(), None);
        let s = st.store.snapshot();
        assert_eq!(s.general.on_start, OnStart::StartScreen);
        assert_eq!(
            s.profile.capabilities,
            Settings::default().profile.capabilities
        );
    });
}

/// A hint: "Turn off tips" sends its key and × just closes it; neither takes the
/// keyboard.
#[test]
fn hint_buttons() {
    let mut text = Text::new();
    let p = general();
    with(keymap(p), empty(p), Settings::default(), |st| {
        let mut c = Chrome::new();
        c.set_size(1000.0, 800.0);
        let id = c.hint("Press g.".into(), 77, 1.0);
        draw(&mut c, &mut text, st);
        assert_eq!(c.kbd, None);
        assert!(c.hits.iter().any(|(_, h)| *h == Target::ToastClose(id)));
        let r = c.activate(Target::ToastUndo(id, 77), st);
        assert_eq!(r.commands, [Command::Undo(77)]);
        let id = c.hint("Press g.".into(), 78, 1.0);
        let r = c.activate(Target::ToastClose(id), st);
        assert!(r.commands.is_empty());
        assert!(c.toasts.is_empty());
    });
}
