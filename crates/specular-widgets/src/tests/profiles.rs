use crate::cheatsheet::SheetMode;
use crate::ctx::Target;
use crate::palette::{PaletteState, Run, rebuild};
use crate::{Chrome, Command, HudStats, Input, Modal, Overlays, ViewerState};
use specular_gfx::{DrawList, Text};
use specular_input::{
    Action, ActionState, Capabilities, Capability, ImageState, Key, KeyCombo, Keymap, Mods,
    NamedKey, Profile,
};
use specular_settings::{Settings, SettingsStore, Value};
use specular_theme::Palette;

/// A CT slice under `profile`.
fn ct(profile: Profile) -> ActionState {
    ActionState {
        comparing: false,
        compare_images: 0,
        volume: true,
        mpr: false,
        image: ImageState::Ready,
        count: 1,
        gallery_files: 0,
        animated: false,
        video: false,
        sound: false,
        sound_file: false,
        in_archive: false,
        has_alpha: false,
        gallery_views: None,
        medical: None,
        dicom: true,
        series: 0,
        presentation_states: 0,
        pages: 0,
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
        rotatable: false,
        wallpaper: false,
        share: false,
        email: false,
        profile,
    }
}

fn with<R>(actions: ActionState, f: impl FnOnce(&ViewerState) -> R) -> R {
    let hud = HudStats::default();
    let mut keymap = Keymap::default();
    keymap.set_profile(actions.content_profile());
    let store = SettingsStore::in_memory();
    let settings = Settings::default();
    let st = ViewerState {
        actions,
        has_image: true,
        index: 0,
        count: 1,
        zoom: 1.0,
        at_fit: true,
        fullscreen: false,
        file_name: Some("ct.dcm"),
        dims: Some((512, 512)),
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

fn palette(st: &ViewerState, query: &str) -> PaletteState {
    let mut p = PaletteState {
        dirty: true,
        query: query.into(),
        ..Default::default()
    };
    rebuild(&mut p, st);
    p
}

fn draw(c: &mut Chrome, text: &mut Text, st: &ViewerState) {
    let mut dl = DrawList::new(1.0);
    for i in 0..4 {
        dl.clear(1.0);
        c.draw(&mut dl, text, st, &Palette::dark(), 1.0 + i as f64 * 0.016);
    }
}

fn press(c: &mut Chrome, st: &ViewerState, k: NamedKey, mods: Mods) -> crate::Response {
    c.input(
        &Input::Key {
            combo: KeyCombo::new(Key::Named(k), mods),
            repeat: false,
        },
        st,
        2.0,
    )
}

/// Hidden commands are left out of the full list, found greyed by a search (saying
/// why) with the two rows that turn the tools on; tier 1 commands are listed as usual.
#[test]
fn palette_greys_hidden_commands_and_offers_their_tools() {
    with(ct(general()), |st| {
        let p = palette(st, "");
        let has = |p: &PaletteState, a| {
            p.items
                .iter()
                .any(|e| matches!(e.run, Run::Action(x) if x == a))
        };
        assert!(!has(&p, Action::Fusion));
        // The full list is capped: look the preset up by name.
        assert!(has(&palette(st, "lung"), Action::WindowPreset(1)));
        // MPR is tier 1 for any volume, in every profile.
        let mpr = palette(st, "multiplanar");
        let mpr = mpr
            .items
            .iter()
            .find(|e| matches!(e.run, Run::Action(x) if x == Action::Mpr))
            .expect("listed");
        assert_eq!(mpr.hidden, None);
        assert!(!mpr.greyed());
        assert!(!p.items.iter().any(|e| matches!(e.run, Run::Capability(..))));

        let p = palette(st, "fuse the series");
        let mpr = p
            .items
            .iter()
            .find(|e| matches!(e.run, Run::Action(x) if x == Action::Fusion))
            .expect("found by a search");
        assert_eq!(mpr.hidden, Some(Capability::Medical));
        assert!(mpr.greyed());
        assert_eq!(mpr.reason().as_deref(), Some("Medical tools are off"));
        let turn: Vec<_> = p
            .items
            .iter()
            .filter_map(|e| match e.run {
                Run::Capability(c, always) => {
                    Some((c, always, e.label.replace(['\u{2068}', '\u{2069}'], "")))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            turn,
            [
                (
                    Capability::Medical,
                    false,
                    "Turn on Medical tools for this session".to_owned()
                ),
                (
                    Capability::Medical,
                    true,
                    "Turn on Medical tools always".to_owned()
                ),
            ]
        );
        // By the tools' own name too.
        let p = palette(st, "medical tools");
        assert!(p.items.iter().any(|e| matches!(e.run, Run::Capability(..))));

        // Choosing a hidden command turns its tools on for the session, then runs it.
        let mut c = Chrome::new();
        assert!(c.handle_action(Action::Palette));
        c.palette.query = "fuse the series".into();
        c.palette.dirty = true;
        rebuild(&mut c.palette, st);
        let i = c
            .palette
            .items
            .iter()
            .position(|e| matches!(e.run, Run::Action(x) if x == Action::Fusion))
            .unwrap();
        let r = c.run_palette(i, st);
        assert_eq!(
            r.commands,
            [Command::Capability {
                capability: Capability::Medical,
                always: false,
                then: Some(Action::Fusion),
            }]
        );
        assert_eq!(c.modal(), None);
    });
    // With the tools on (for the session), nothing is greyed and nothing is offered.
    let mut on = general();
    on.session = Capabilities::GENERAL.with(Capability::Medical);
    with(ct(on), |st| {
        let p = palette(st, "fuse the series");
        let mpr = p
            .items
            .iter()
            .find(|e| matches!(e.run, Run::Action(x) if x == Action::Fusion));
        assert_eq!(mpr.map(|e| e.hidden), Some(None));
        assert!(!p.items.iter().any(|e| matches!(e.run, Run::Capability(..))));
    });
}

/// The cheat sheet has no Medical tab while Medical is off.
#[test]
fn cheat_sheet_leaves_out_what_is_off() {
    let mut text = Text::new();
    for (profile, medical) in [
        (general(), false),
        (
            Profile::of(Capabilities::GENERAL.with(Capability::Medical)),
            true,
        ),
    ] {
        let mut actions = ct(profile);
        actions.dicom = false;
        with(actions, |st| {
            let mut c = Chrome::new();
            c.set_size(1200.0, 800.0);
            c.open_modal(Modal::CheatSheet);
            draw(&mut c, &mut text, st);
            assert_eq!(c.cheat_tabs.contains(&SheetMode::Medical), medical);
            assert_eq!(
                SheetMode::shown(st.keymap).contains(&SheetMode::Medical),
                medical
            );
        });
    }
}

/// The chip shows over the toolbar while a CT slice is on screen with Medical off, is a
/// named button the keyboard reaches, and turns the tools on for the session.
#[test]
fn toolbar_chip() {
    let mut text = Text::new();
    with(ct(general()), |st| {
        let mut c = Chrome::new();
        c.set_size(1200.0, 800.0);
        c.poke(1.0);
        draw(&mut c, &mut text, st);
        let t = Target::ProfileChip(Capability::Medical);
        assert!(c.hits.iter().any(|(_, h)| *h == t), "chip drawn");
        assert!(c.bar_stops.contains(&t));
        let r = c.activate(t.clone(), st);
        assert_eq!(
            r.commands,
            [Command::Capability {
                capability: Capability::Medical,
                always: false,
                then: None,
            }]
        );
        let tree = c.accessibility_tree(st, "ct.dcm", 1.0);
        assert!(
            tree.nodes
                .iter()
                .any(|(_, n)| n.label() == Some("Medical tools")),
            "the chip is a named button"
        );
    });
    // No chip for a photo, or once the tools are on.
    let mut photo = ct(general());
    photo.dicom = false;
    let mut on = general();
    on.session = Capabilities::GENERAL.with(Capability::Medical);
    for actions in [photo, ct(on)] {
        with(actions, |st| {
            let mut c = Chrome::new();
            c.set_size(1200.0, 800.0);
            c.poke(1.0);
            draw(&mut c, &mut text, st);
            assert!(
                !c.hits
                    .iter()
                    .any(|(_, h)| matches!(h, Target::ProfileChip(_)))
            );
        });
    }
}

/// The first-run question: General is checked and stays so, Tab goes through the other
/// boxes, Space checks one and Enter stores the choice; right to left, mirrored.
#[test]
fn first_run_question() {
    let mut text = Text::new();
    for rtl in [false, true] {
        with(ct(Profile::UNRESTRICTED), |st| {
            let mut c = Chrome::new();
            c.set_size(900.0, 700.0);
            c.force_rtl = rtl;
            c.ask_profile(Capabilities::GENERAL);
            assert_eq!(c.modal(), Some(Modal::Profile));
            draw(&mut c, &mut text, st);
            c.activate(Target::ProfileCheck(Capability::General), st);
            assert!(c.profile_pick.contains(Capability::General));
            // The keyboard starts on Continue; Tab wraps to Engineering, then Medical.
            assert_eq!(c.modal_focus, Some(Target::ProfileDone));
            press(&mut c, st, NamedKey::Tab, Mods::NONE);
            press(&mut c, st, NamedKey::Tab, Mods::NONE);
            assert_eq!(
                c.modal_focus,
                Some(Target::ProfileCheck(Capability::Medical))
            );
            press(&mut c, st, NamedKey::Space, Mods::NONE);
            assert!(c.profile_pick.contains(Capability::Medical));
            draw(&mut c, &mut text, st);
            let rect = |t: &Target| c.hits.iter().rev().find(|(_, h)| h == t).unwrap().0;
            let done = rect(&Target::ProfileDone);
            let row = rect(&Target::ProfileCheck(Capability::Science));
            // Continue sits at the end edge: right, or left right to left.
            assert_eq!(done.center().0 < row.center().0, rtl);
            press(&mut c, st, NamedKey::Enter, Mods::NONE);
            assert_eq!(c.modal(), None);
            assert_eq!(
                st.store.get("profile.capabilities").unwrap(),
                Value::List(vec!["general".into(), "medical".into()])
            );
        });
    }
}

/// The Settings window: the General chip of the Profile page cannot be turned off, the
/// bookkeeping setting is never listed, and a section whose settings are all hidden
/// leaves the sidebar.
#[test]
fn settings_profile_page() {
    use crate::settings::SettingsTarget as S;
    with(ct(Profile::UNRESTRICTED), |st| {
        let mut c = Chrome::new();
        assert!(c.setting_hidden("profile.shown"));
        assert!(c.sections().iter().any(|s| s.key() == "profile"));
        c.open_modal(Modal::Settings);
        c.activate(
            Target::Settings(S::ListItem("profile.capabilities", "general")),
            st,
        );
        assert_eq!(
            st.store.get("profile.capabilities").unwrap(),
            Value::List(vec!["general".into()])
        );
        c.activate(
            Target::Settings(S::ListItem("profile.capabilities", "science")),
            st,
        );
        assert_eq!(
            st.store.get("profile.capabilities").unwrap(),
            Value::List(vec!["general".into(), "science".into()])
        );
        assert!(c.sections().iter().any(|s| s.key() == "medical"));
        for m in specular_settings::in_section(specular_settings::Section::Medical) {
            c.set_setting_hidden(m.key, true);
        }
        assert!(!c.sections().iter().any(|s| s.key() == "medical"));
    });
}
