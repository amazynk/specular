use crate::accesskit::{
    Action as AkAction, ActionRequest, Live, Node, NodeId, Role, TreeId, TreeUpdate,
};
use crate::{Chrome, Command, HudStats, Modal, Overlays, ViewerState};
use specular_gfx::{DrawList, Text};
use specular_input::{Action, ActionState, ImageState, Keymap};
use specular_settings::{Settings, SettingsStore};
use specular_theme::Palette;

fn with_image<R>(f: impl FnOnce(&ViewerState) -> R) -> R {
    let hud = HudStats::default();
    let keymap = Keymap::default();
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

fn tree(c: &mut Chrome, text: &mut Text, st: &ViewerState) -> TreeUpdate {
    let mut dl = DrawList::new(1.0);
    for i in 0..40 {
        dl.clear(1.0);
        c.draw(&mut dl, text, st, &Palette::dark(), 1.0 + i as f64 * 0.016);
    }
    c.accessibility_tree(st, "beach.jpg", 1.0)
}

fn find(t: &TreeUpdate, f: impl Fn(&Node) -> bool) -> Vec<(NodeId, &Node)> {
    t.nodes
        .iter()
        .filter(|(_, n)| f(n))
        .map(|(id, n)| (*id, n))
        .collect()
}

/// Every child is defined once, and every node but the root has one parent.
fn check_structure(t: &TreeUpdate) {
    let ids: std::collections::HashSet<_> = t.nodes.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids.len(), t.nodes.len(), "duplicate node ids");
    let mut parents = std::collections::HashMap::new();
    for (id, n) in &t.nodes {
        for c in n.children() {
            assert!(ids.contains(c), "{id:?} has an undefined child {c:?}");
            assert!(parents.insert(*c, *id).is_none(), "{c:?} has two parents");
        }
    }
    let root = t.tree.as_ref().expect("a full tree").root;
    assert!(!parents.contains_key(&root));
    assert_eq!(parents.len() + 1, t.nodes.len(), "orphan nodes");
    assert!(ids.contains(&t.focus));
}

fn chrome() -> Chrome {
    let mut c = Chrome::new();
    c.set_size(1000.0, 700.0);
    c.set_accessibility(true);
    c.poke(1.0);
    c
}

#[test]
fn image_toolbar_toasts_and_dialogs() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome();
        let t = tree(&mut c, &mut text, st);
        check_structure(&t);
        let img = find(&t, |n| n.role() == Role::Image);
        assert_eq!(img.len(), 1);
        let label = img[0].1.label().unwrap_or_default();
        // Numbers come localized ("4,000"); keep only the digits to compare.
        let digits: String = label
            .chars()
            .map(|c| if c.is_ascii_digit() { c } else { ' ' })
            .filter(|c| *c != ' ')
            .collect();
        assert!(
            label.contains("beach.jpg") && digits.contains("40003000"),
            "{label}"
        );
        let buttons = find(&t, |n| n.role() == Role::Button);
        assert!(buttons.len() >= 8, "{} toolbar buttons", buttons.len());
        for (id, b) in &buttons {
            assert!(
                b.label().is_some_and(|l| !l.is_empty()),
                "button {id:?} has no name"
            );
        }

        // Toasts: a polite live region.
        c.toast("Copied to the clipboard".into(), None, 1.0);
        let t = tree(&mut c, &mut text, st);
        check_structure(&t);
        // Two polite regions: the channel view status and the toasts.
        let live = find(&t, |n| n.live() == Some(Live::Polite));
        assert_eq!(live.len(), 2);
        assert!(
            live.iter()
                .any(|(_, n)| n.role() == Role::Status
                    && n.label().is_some_and(|l| l == "All channels")),
            "channel view status"
        );
        let toasts: Vec<_> = live
            .iter()
            .filter(|(_, n)| !n.children().is_empty())
            .collect();
        assert_eq!(toasts.len(), 1);
        let found = toasts[0].1.children().iter().any(|id| {
            t.nodes
                .iter()
                .any(|(i, n)| i == id && n.label().is_some_and(|l| l.contains("Copied")))
        });
        assert!(found, "toast text in the live region");

        // A modal: a dialog, and nothing behind it.
        for m in [Modal::Settings, Modal::Palette, Modal::About] {
            c.open_modal(m);
            let t = tree(&mut c, &mut text, st);
            check_structure(&t);
            let d = find(&t, |n| n.role() == Role::Dialog);
            assert_eq!(d.len(), 1, "{m:?}");
            assert!(d[0].1.is_modal());
            assert!(d[0].1.label().is_some_and(|l| !l.is_empty()), "{m:?} title");
            // The controls behind it are left out.
            let bar = find(&t, |n| n.role() == Role::Toolbar);
            assert!(bar.iter().all(|(_, n)| n.children().is_empty()), "{m:?}");
            c.close_modal();
        }
    });
}

#[test]
fn click_runs_the_button() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome();
        let t = tree(&mut c, &mut text, st);
        let next = crate::toolbar::item_label(specular_settings::ToolbarItem::Next, st).0;
        let (id, _) = find(&t, |n| {
            n.role() == Role::Button && n.label() == Some(next.as_str())
        })[0];
        let r = c.accessibility_action(
            &ActionRequest {
                action: AkAction::Click,
                target_tree: TreeId::ROOT,
                target_node: id,
                data: None,
            },
            st,
        );
        assert!(
            r.commands
                .iter()
                .any(|c| matches!(c, Command::Action(Action::NextImage))),
            "{:?}",
            r.commands
        );
    });
}

/// The toolbar offers its list in Settings as a custom action.
#[test]
fn customising_the_toolbar() {
    use crate::accesskit::{ActionData, Role};
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome();
        let t = tree(&mut c, &mut text, st);
        let (bar, n) = find(&t, |n| n.role() == Role::Toolbar)[0];
        let label = specular_i18n::t!("toolbar-customise");
        assert!(
            n.custom_actions()
                .iter()
                .any(|a| a.id == 10 && *a.description == *label)
        );
        c.accessibility_action(
            &ActionRequest {
                action: AkAction::CustomAction,
                target_tree: TreeId::ROOT,
                target_node: bar,
                data: Some(ActionData::CustomAction(10)),
            },
            st,
        );
        assert_eq!(c.modal(), Some(Modal::Settings));
        assert_eq!(
            c.settings.scroll_to,
            Some(crate::toolbar_edit::KEY),
            "at the toolbar's list"
        );
        check_structure(&tree(&mut c, &mut text, st));
    });
}

/// The toolbar list's row controls and a row's menu, for screen readers.
#[test]
fn toolbar_list_row_menu() {
    use crate::settings::SettingsTarget;
    use crate::toolbar_edit::RowOp;
    use specular_i18n::t;
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome();
        c.open_settings_at("viewing.toolbar_items");
        let list = crate::toolbar_edit::current(st.store);
        let first = crate::toolbar::item_name(list[0]);
        let t = tree(&mut c, &mut text, st);
        check_structure(&t);
        let up = t!("settings-toolbar-move-up");
        let ups = find(&t, |n| {
            n.label() == Some(up.as_str()) && n.description() == Some(first.as_str())
        });
        assert_eq!(ups.len(), 1);
        assert!(ups[0].1.is_disabled(), "the first row cannot move up");
        let menu_label = t!("settings-toolbar-row-menu", item = first.clone());
        let (button, n) = find(&t, |n| n.label() == Some(menu_label.as_str()))[0];
        assert_eq!(n.role(), Role::Button);
        assert_eq!(n.has_popup(), Some(crate::accesskit::HasPopup::Menu));
        assert_eq!(n.is_expanded(), Some(false));

        // Opened from the keyboard: a menu of three entries, Move to start disabled, the
        // focus on the first that can run.
        c.open_row_menu(0, st, true);
        let t = tree(&mut c, &mut text, st);
        check_structure(&t);
        let n = t.nodes.iter().find(|(id, _)| *id == button).map(|(_, n)| n);
        assert_eq!(n.and_then(|n| n.is_expanded()), Some(true));
        let menus = find(&t, |n| n.role() == Role::Menu);
        assert_eq!(menus.len(), 1);
        assert_eq!(menus[0].1.label(), Some(menu_label.as_str()));
        let items: Vec<_> = menus[0]
            .1
            .children()
            .iter()
            .map(|id| t.nodes.iter().find(|(i, _)| i == id).unwrap().1.clone())
            .collect();
        assert_eq!(items.len(), 3);
        assert!(items.iter().all(|n| n.role() == Role::MenuItem));
        assert_eq!(
            items[0].label(),
            Some(t!("settings-toolbar-move-start").as_str())
        );
        assert!(items[0].is_disabled());
        assert!(!items[1].is_disabled());
        assert_eq!(
            items[2].label(),
            Some(t!("settings-toolbar-menu-hide").as_str())
        );
        let end = find(&t, |n| {
            n.label() == Some(t!("settings-toolbar-move-end").as_str())
        })[0]
            .0;
        assert_eq!(t.focus, end);
        assert_eq!(
            c.toolbar_focus(st),
            Some(crate::ctx::Target::Settings(SettingsTarget::ToolMenuItem(
                0,
                RowOp::End
            )))
        );

        // Clicked: runs, closes, keeps the focus on the row's menu button.
        c.accessibility_action(
            &ActionRequest {
                action: AkAction::Click,
                target_tree: TreeId::ROOT,
                target_node: end,
                data: None,
            },
            st,
        );
        let now = crate::toolbar_edit::current(st.store);
        assert_eq!(*now.last().unwrap(), list[0]);
        let t = tree(&mut c, &mut text, st);
        check_structure(&t);
        assert!(find(&t, |n| n.role() == Role::Menu).is_empty());
        let moved = find(&t, |n| n.label() == Some(menu_label.as_str()))[0].0;
        assert_eq!(t.focus, moved);
    });
}

/// The statuses read out in the polite live region (the toasts' group), by id and text.
fn spoken_statuses(t: &TreeUpdate) -> Vec<(NodeId, String)> {
    let live = find(t, |n| {
        n.live() == Some(Live::Polite) && n.role() == Role::Group
    });
    assert_eq!(live.len(), 1, "one polite group");
    live[0]
        .1
        .children()
        .iter()
        .filter_map(|id| {
            t.nodes
                .iter()
                .find(|(i, _)| i == id)
                .filter(|(_, n)| n.role() == Role::Status)
                .map(|(i, n)| (*i, n.label().unwrap_or_default().to_owned()))
        })
        .collect()
}

#[test]
fn the_value_under_the_cursor_is_read_once_it_rests() {
    use crate::cursor_readout::{CURSOR_REST_S, CursorAnnouncer};
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome();
        let mut a = CursorAnnouncer::default();
        // One frame: the app feeds the readout shown and sends what is due.
        let frame =
            |c: &mut Chrome, a: &mut CursorAnnouncer, text: &mut Text, readout: &str, now: f64| {
                a.update(Some(readout.to_owned()), now);
                if let Some(s) = a.due(now) {
                    c.announce_status(s);
                }
                let t = tree(c, text, st);
                check_structure(&t);
                spoken_statuses(&t)
            };
        // The pointer sweeps across: nothing is read while it moves (60 frames a second).
        for i in 0..30 {
            let s = frame(
                &mut c,
                &mut a,
                &mut text,
                &format!("-{i}0 HU"),
                f64::from(i) / 60.0,
            );
            assert!(s.is_empty(), "read while moving: {s:?}");
        }
        // It stops: the readout is read once, as shown.
        let stop = 29.0 / 60.0;
        let s = frame(&mut c, &mut a, &mut text, "-290 HU", stop + CURSOR_REST_S);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].1, "-290 HU");
        let first = s[0].0;
        // Still there: the same node, not read again.
        let s = frame(&mut c, &mut a, &mut text, "-290 HU", stop + 3.0);
        assert_eq!(s, vec![(first, "-290 HU".to_owned())]);
        // The announce key reads the same text again: a new node, so it is spoken again.
        a.read_now("-290 HU");
        c.announce_status("-290 HU".into());
        let t = tree(&mut c, &mut text, st);
        let s = spoken_statuses(&t);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].1, "-290 HU");
        assert_ne!(s[0].0, first, "a repeat must be a new node to be spoken");
        // Moving on to another value and resting reads that one.
        let s = frame(&mut c, &mut a, &mut text, "35.2 °C", stop + 10.0);
        assert!(s.iter().all(|(_, l)| l != "35.2 °C"), "{s:?}");
        let s = frame(
            &mut c,
            &mut a,
            &mut text,
            "35.2 °C",
            stop + 10.0 + CURSOR_REST_S,
        );
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].1, "35.2 °C");
    });
}

/// The GPU-reset toasts are read out like any toast: their text is in the
/// polite live region, and the tree stays whole.
#[test]
fn gpu_reset_toasts_are_read_out() {
    let mut text = Text::new();
    with_image(|st| {
        let mut c = chrome();
        let said = [
            specular_i18n::t!("toast-gpu-reset"),
            specular_i18n::t!("toast-gpu-reset-retry"),
        ];
        for s in &said {
            c.toast(s.clone(), None, 1.0);
        }
        let t = tree(&mut c, &mut text, st);
        check_structure(&t);
        let live = find(&t, |n| {
            n.live() == Some(Live::Polite) && n.role() == Role::Group
        });
        assert_eq!(live.len(), 1);
        for want in &said {
            let found = live[0].1.children().iter().any(|id| {
                t.nodes
                    .iter()
                    .any(|(i, n)| i == id && n.label() == Some(want.as_str()))
            });
            assert!(found, "{want} in the live region");
        }
    });
}

/// With reduce motion a toast is there at once and goes at once; without, it fades.
#[test]
fn toasts_respect_reduce_motion() {
    use crate::toast_opacity;
    assert_eq!(toast_opacity(0.0, 5.0, true), 1.0);
    assert_eq!(toast_opacity(4.99, 0.01, true), 1.0);
    assert_eq!(toast_opacity(5.0, 0.0, true), 0.0);
    assert!(toast_opacity(0.0, 5.0, false) < 0.01);
    assert!(toast_opacity(0.05, 4.95, false) < 1.0);
    assert_eq!(toast_opacity(1.0, 4.0, false), 1.0);
    assert!(toast_opacity(4.95, 0.05, false) < 1.0);
}
