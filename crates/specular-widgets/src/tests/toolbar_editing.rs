use super::enabled_state::{STATES, with_state};
use crate::accesskit::Role;
use crate::ctx::Target;
use crate::settings::SettingsTarget;
use crate::toolbar::{GAP, PAD, bar_layout};
use crate::toolbar_edit::{
    Change, Src, bar_drop, current, drop_on, hidden, insert, move_to, row_drop, show,
};
use crate::{Button, Chrome, Input, Modal, ViewerState};
use specular_gfx::{DrawList, Rect, Text};
use specular_input::{Action, Key, KeyCombo, Mods, NamedKey};
use specular_settings::{SettingEnum, SettingsStore, ToolbarItem as T};
use specular_theme::Palette;

fn defaults() -> Vec<T> {
    current(&SettingsStore::in_memory())
}

#[test]
fn list_operations() {
    let mut l = vec![T::Open, T::Prev, T::Next, T::Info];
    assert_eq!(move_to(&mut l, 0, 2), 2);
    assert_eq!(l, [T::Prev, T::Next, T::Open, T::Info]);
    assert_eq!(move_to(&mut l, 3, 99), 3, "clamped to the end");
    assert_eq!(move_to(&mut l, 9, 0), 9, "out of range: unchanged");
    assert_eq!(l, [T::Prev, T::Next, T::Open, T::Info]);

    // A button is on the bar once: inserting it again moves it.
    let mut l = vec![T::Open, T::Prev, T::Next];
    assert_eq!(insert(&mut l, 3, T::Open), 2);
    assert_eq!(l, [T::Prev, T::Next, T::Open]);
    assert_eq!(insert(&mut l, 0, T::Next), 0);
    assert_eq!(l, [T::Next, T::Prev, T::Open]);
    // Spacers and separators repeat.
    insert(&mut l, 1, T::Spacer);
    insert(&mut l, usize::MAX, T::Spacer);
    insert(&mut l, 2, T::Separator);
    insert(&mut l, 2, T::Separator);
    assert_eq!(
        l,
        [
            T::Next,
            T::Spacer,
            T::Separator,
            T::Separator,
            T::Prev,
            T::Open,
            T::Spacer
        ]
    );

    // Hidden: the buttons not on the bar, in the schema's order, never layout items.
    let h = hidden(&l);
    assert!(!h.contains(&T::Next) && !h.contains(&T::Spacer) && h.contains(&T::Info));
    assert_eq!(
        h.len(),
        T::ALL.iter().filter(|t| !t.is_layout()).count() - 3
    );

    // Showing a button puts it after the nearest earlier one (in the schema) on the bar,
    // so a custom order is kept.
    let mut l = vec![T::Settings, T::Prev, T::Info];
    assert_eq!(show(&mut l, T::Next), 2);
    assert_eq!(l, [T::Settings, T::Prev, T::Next, T::Info]);
    assert_eq!(show(&mut l, T::Open), 0, "nothing before it: first");
    assert_eq!(show(&mut l, T::Next), 3, "already shown: stays");
}

#[test]
fn drops() {
    let l = vec![T::Open, T::Prev, T::Next];
    // Positions count the dragged item: dropping it just before or after itself is no change.
    assert!(drop_on(&l, Src::Bar(1), Some(1)).is_none());
    assert!(drop_on(&l, Src::Bar(1), Some(2)).is_none());
    let (m, c) = drop_on(&l, Src::Bar(0), Some(3)).unwrap();
    assert_eq!(
        (m, c),
        (vec![T::Prev, T::Next, T::Open], Change::Moved(T::Open, 2))
    );
    let (m, c) = drop_on(&l, Src::Bar(2), Some(0)).unwrap();
    assert_eq!(
        (m, c),
        (vec![T::Next, T::Open, T::Prev], Change::Moved(T::Next, 0))
    );
    // Off the bar: removed. From the hidden ones: added; off the bar: nothing.
    let (m, c) = drop_on(&l, Src::Bar(1), None).unwrap();
    assert_eq!((m, c), (vec![T::Open, T::Next], Change::Removed(T::Prev)));
    let (m, c) = drop_on(&l, Src::Hidden(T::Info), Some(1)).unwrap();
    assert_eq!(
        (m, c),
        (
            vec![T::Open, T::Info, T::Prev, T::Next],
            Change::Added(T::Info, 1)
        )
    );
    let (m, _) = drop_on(&l, Src::Hidden(T::Spacer), Some(0)).unwrap();
    assert_eq!(m, [T::Spacer, T::Open, T::Prev, T::Next]);
    assert!(drop_on(&l, Src::Hidden(T::Info), None).is_none());

    // The bar: three 40 px slots from x = 100.
    let slots: Vec<Rect> = (0..3)
        .map(|i| Rect::new(100.0 + 40.0 * i as f32, 600.0, 40.0, 40.0))
        .collect();
    let bar = Rect::new(94.0, 596.0, 132.0, 48.0);
    assert_eq!(bar_drop(&slots, bar, (110.0, 620.0), false), Some(0));
    assert_eq!(bar_drop(&slots, bar, (150.0, 620.0), false), Some(1));
    assert_eq!(bar_drop(&slots, bar, (225.0, 620.0), false), Some(3));
    // RTL: reading order runs from the right.
    assert_eq!(bar_drop(&slots, bar, (225.0, 620.0), true), Some(0));
    assert_eq!(bar_drop(&slots, bar, (150.0, 620.0), true), Some(2));
    assert_eq!(bar_drop(&slots, bar, (110.0, 620.0), true), Some(3));
    // A little above the bar still lands; well above it is off.
    assert_eq!(bar_drop(&slots, bar, (150.0, 580.0), false), Some(1));
    assert_eq!(bar_drop(&slots, bar, (150.0, 500.0), false), None);

    // The list: three shown rows 30 px tall from y = 0, the hidden ones from y = 120.
    let rows: Vec<Rect> = (0..5)
        .map(|i| {
            Rect::new(
                0.0,
                30.0 * i as f32 + if i >= 3 { 30.0 } else { 0.0 },
                300.0,
                30.0,
            )
        })
        .collect();
    assert_eq!(row_drop(&rows, 3, 120.0, 5.0), Some(0));
    assert_eq!(row_drop(&rows, 3, 120.0, 50.0), Some(2));
    assert_eq!(row_drop(&rows, 3, 120.0, 100.0), Some(3));
    assert_eq!(row_drop(&rows, 3, 120.0, 130.0), None);
}

#[test]
fn spacers_share_the_room() {
    let items = [
        (36.0, false),
        (12.0, true),
        (36.0, false),
        (9.0, false),
        (12.0, true),
    ];
    let natural = 36.0 * 2.0 + 12.0 * 2.0 + 9.0 + GAP * 4.0 + PAD * 2.0;
    // Narrower room than the items: natural width, spacers at their minimum.
    let l = bar_layout(0.0, &items, 100.0);
    assert_eq!(l.width, natural);
    assert_eq!(l.slots[1].1, 12.0);
    // Wider: the bar takes the room, the spacers share it equally, and slots don't overlap.
    let l = bar_layout(50.0, &items, 600.0);
    assert_eq!(l.width, 600.0);
    assert_eq!(l.slots[1].1, l.slots[4].1);
    assert!(l.slots[1].1 > 12.0);
    assert_eq!(l.slots[0].0, PAD + 50.0);
    for w in l.slots.windows(2) {
        assert!((w[0].0 + w[0].1 + GAP - w[1].0).abs() < 1e-3);
    }
    let end = l.slots[4].0 + l.slots[4].1 + PAD;
    assert!((end - 600.0).abs() < 1e-3, "{end}");
    // No spacers: never wider than the items.
    let l = bar_layout(0.0, &[(36.0, false)], 600.0);
    assert_eq!(l.width, 36.0 + PAD * 2.0);
}

fn key(k: NamedKey, mods: Mods) -> Input {
    Input::Key {
        combo: KeyCombo::new(Key::Named(k), mods),
        repeat: false,
    }
}

fn alt() -> Mods {
    Mods {
        alt: true,
        ..Mods::NONE
    }
}

fn ctrl_z() -> Input {
    Input::Key {
        combo: KeyCombo::new(
            Key::Char('z'),
            Mods {
                ctrl: true,
                ..Mods::NONE
            },
        ),
        repeat: false,
    }
}

fn draw(c: &mut Chrome, text: &mut Text, st: &ViewerState) {
    let mut dl = DrawList::new(1.0);
    for i in 0..30 {
        dl.clear(1.0);
        c.draw(&mut dl, text, st, &Palette::dark(), 1.0 + i as f64 * 0.016);
    }
}

fn hit(c: &Chrome, t: &Target) -> Rect {
    c.hits
        .iter()
        .rev()
        .find(|(_, h)| h == t)
        .unwrap_or_else(|| panic!("{t:?} not drawn"))
        .0
}

fn button(x: f32, y: f32, down: bool, button: Button) -> Input {
    Input::PointerButton { x, y, down, button }
}

fn chrome() -> Chrome {
    let mut c = Chrome::new();
    c.set_size(1000.0, 800.0);
    c.poke(1.0);
    c
}

#[test]
fn settings_list_by_keyboard() {
    let mut text = Text::new();
    with_state(STATES[3], |st| {
        let mut c = chrome();
        c.open_settings_at("viewing.toolbar_items");
        draw(&mut c, &mut text, st);
        assert!(
            c.hits
                .iter()
                .any(|(_, t)| *t == Target::Settings(SettingsTarget::ToolRow(0)))
        );
        let start = current(st.store);
        assert_eq!(start, defaults());

        // Tab leaves the search field for the list.
        c.input(&key(NamedKey::Tab, Mods::NONE), st, 2.0);
        assert_eq!(c.settings.tools.focus, Some(0));
        // Alt+↓ moves the first button down, one undo step.
        c.input(&key(NamedKey::ArrowDown, alt()), st, 2.0);
        assert_eq!(c.settings.tools.focus, Some(1));
        let moved = current(st.store);
        assert_eq!((moved[0], moved[1]), (start[1], start[0]));
        assert!(c.announce.as_ref().is_some_and(|a| !a.1.is_empty()));
        c.input(&ctrl_z(), st, 2.0);
        assert_eq!(current(st.store), start);

        // Space hides the focused button: the focus follows it to the hidden ones; Space
        // again shows it where it was.
        c.input(&key(NamedKey::ArrowUp, Mods::NONE), st, 2.0);
        assert_eq!(c.settings.tools.focus, Some(0));
        c.input(&key(NamedKey::Space, Mods::NONE), st, 2.0);
        let l = current(st.store);
        assert!(!l.contains(&start[0]));
        let f = c.settings.tools.focus.unwrap();
        assert_eq!(hidden(&l)[f - l.len()], start[0]);
        c.input(&key(NamedKey::Space, Mods::NONE), st, 2.0);
        assert_eq!(current(st.store), start);

        // Adding a spacer twice, then Reset.
        c.tools_click(SettingsTarget::ToolAdd(T::Spacer), st);
        c.tools_click(SettingsTarget::ToolAdd(T::Spacer), st);
        assert_eq!(
            current(st.store)
                .iter()
                .filter(|t| **t == T::Spacer)
                .count(),
            2
        );
        c.tools_click(SettingsTarget::ToolReset, st);
        assert_eq!(current(st.store), start);

        // The list is drawn with its own rows and switches, not the old chips.
        draw(&mut c, &mut text, st);
        assert!(
            c.hits
                .iter()
                .any(|(_, t)| *t == Target::Settings(SettingsTarget::ToolShow(0)))
        );
        // Tab goes back to the search field.
        c.input(&key(NamedKey::Tab, Mods::NONE), st, 2.0);
        assert_eq!(c.settings.tools.focus, None);
    });
}

#[test]
fn settings_list_by_dragging_a_row() {
    let mut text = Text::new();
    with_state(STATES[3], |st| {
        let mut c = chrome();
        c.open_settings_at("viewing.toolbar_items");
        draw(&mut c, &mut text, st);
        let start = current(st.store);
        let r0 = hit(&c, &Target::Settings(SettingsTarget::ToolRow(0)));
        let r2 = hit(&c, &Target::Settings(SettingsTarget::ToolRow(2)));
        let (x, y) = (r0.x + 8.0, r0.center().1);
        c.input(&button(x, y, true, Button::Primary), st, 2.0);
        c.input(
            &Input::PointerMove {
                x,
                y: r2.y + r2.h * 0.75,
            },
            st,
            2.0,
        );
        draw(&mut c, &mut text, st);
        c.input(
            &button(x, r2.y + r2.h * 0.75, false, Button::Primary),
            st,
            2.0,
        );
        let l = current(st.store);
        assert_eq!(l[2], start[0], "{l:?}");
        assert_eq!(l.len(), start.len());
    });
}

/// Right-click on the toolbar: a menu whose one entry opens the toolbar's list in Settings;
/// the palette's Customise toolbar does the same.
#[test]
fn customise_opens_the_settings_list() {
    let mut text = Text::new();
    with_state(STATES[3], |st| {
        let mut c = chrome();
        draw(&mut c, &mut text, st);
        let bar = hit(&c, &Target::Toolbar(T::Next));
        let (x, y) = bar.center();
        c.input(&button(x, y, true, Button::Secondary), st, 2.0);
        c.input(&button(x, y, false, Button::Secondary), st, 2.0);
        assert!(c.toolbar_menu.is_some());
        draw(&mut c, &mut text, st);
        let m = hit(&c, &Target::ToolbarMenu).center();
        c.input(&button(m.0, m.1, true, Button::Primary), st, 3.0);
        c.input(&button(m.0, m.1, false, Button::Primary), st, 3.0);
        assert!(c.toolbar_menu.is_none());
        assert_eq!(c.modal(), Some(Modal::Settings));
        assert_eq!(c.settings.scroll_to, Some(crate::toolbar_edit::KEY));

        // By keyboard: Enter on the open menu.
        let mut c = chrome();
        draw(&mut c, &mut text, st);
        c.input(&button(x, y, true, Button::Secondary), st, 2.0);
        c.input(&button(x, y, false, Button::Secondary), st, 2.0);
        c.input(&key(NamedKey::Enter, Mods::NONE), st, 2.0);
        assert_eq!(c.modal(), Some(Modal::Settings));
        assert_eq!(c.settings.scroll_to, Some(crate::toolbar_edit::KEY));

        // The palette's action.
        let mut c = chrome();
        c.handle_action(Action::CustomiseToolbar);
        assert_eq!(c.modal(), Some(Modal::Settings));
        assert_eq!(c.settings.scroll_to, Some(crate::toolbar_edit::KEY));
    });
}

fn narrow(rtl: bool) -> Chrome {
    let mut c = chrome();
    c.set_size(380.0, 700.0);
    c.force_rtl = rtl;
    c.set_accessibility(true);
    c
}

/// In a narrow window the buttons that do not fit go into a More menu, drawn, hit-tested,
/// mirrored and navigable like the bar.
#[test]
fn overflow_menu() {
    let mut text = Text::new();
    for rtl in [false, true] {
        with_state(STATES[3], |st| {
            let mut c = narrow(rtl);
            draw(&mut c, &mut text, st);
            let more = hit(&c, &Target::ToolbarMore);
            assert!(more.x >= 0.0 && more.right() <= 380.0);
            let over = c.toolbar_overflow.clone();
            assert!(!over.is_empty());
            assert!(over.iter().all(|t| !t.is_layout()));
            for it in &over {
                assert!(
                    !c.hits.iter().any(|(_, t)| *t == Target::Toolbar(*it)),
                    "{it:?} drawn while the menu is shut"
                );
            }
            // The bar and the menu hold every button between them.
            let shown = current(st.store)
                .into_iter()
                .filter(|t| !t.is_layout() && *t != specular_settings::ToolbarItem::Mpr)
                .count();
            let on_bar = c
                .hits
                .iter()
                .filter(|(_, t)| matches!(t, Target::Toolbar(_)))
                .count();
            assert_eq!(on_bar + over.len(), shown);

            let (x, y) = more.center();
            c.input(&button(x, y, true, Button::Primary), st, 2.0);
            c.input(&button(x, y, false, Button::Primary), st, 2.0);
            assert_eq!(c.overflow_menu, Some(None));
            draw(&mut c, &mut text, st);
            let rows: Vec<Rect> = over
                .iter()
                .map(|it| hit(&c, &Target::Toolbar(*it)))
                .collect();
            for (r, n) in rows.iter().zip(rows.iter().skip(1)) {
                assert!(n.y > r.y, "one row under the other");
            }
            for r in &rows {
                assert!(r.x >= 0.0 && r.right() <= 380.0 && r.y >= 0.0);
                assert!(r.bottom() < more.y, "above the button");
                if rtl {
                    assert!((r.x - more.x).abs() <= 12.0, "{r:?} {more:?}");
                } else {
                    assert!((r.right() - more.right()).abs() <= 12.0, "{r:?} {more:?}");
                }
            }

            // Accessibility: More says it has a menu and that it is open; the rows are
            // menu items in a menu.
            let t = c.accessibility_tree(st, "x", 1.0);
            let more_node = t
                .nodes
                .iter()
                .find(|(_, n)| n.role() == Role::Button && n.label() == Some("More"))
                .expect("More in the tree");
            assert_eq!(more_node.1.is_expanded(), Some(true));
            let menu = t
                .nodes
                .iter()
                .find(|(_, n)| n.role() == Role::Menu)
                .expect("the menu");
            assert_eq!(menu.1.children().len(), over.len());
            for id in menu.1.children() {
                let n = &t.nodes.iter().find(|(i, _)| i == id).unwrap().1;
                assert!(matches!(n.role(), Role::MenuItem | Role::MenuItemCheckBox));
            }

            // Keys: ↓ selects the first row, ↑ wraps to the last, Esc closes.
            c.input(&key(NamedKey::ArrowDown, Mods::NONE), st, 3.0);
            assert_eq!(c.overflow_menu, Some(Some(0)));
            c.input(&key(NamedKey::ArrowUp, Mods::NONE), st, 3.0);
            assert_eq!(c.overflow_menu, Some(Some(over.len() - 1)));
            c.input(&key(NamedKey::Home, Mods::NONE), st, 3.0);
            assert_eq!(c.overflow_menu, Some(Some(0)));
            assert!(c.back());
            assert_eq!(c.overflow_menu, None);

            // Enter on a row runs it and closes the menu.
            c.input(&button(x, y, true, Button::Primary), st, 4.0);
            c.input(&button(x, y, false, Button::Primary), st, 4.0);
            c.input(&key(NamedKey::End, Mods::NONE), st, 4.0);
            let r = c.input(&key(NamedKey::Enter, Mods::NONE), st, 4.0);
            assert!(r.consumed || !r.commands.is_empty());
            assert_eq!(c.overflow_menu, None);

            // A click elsewhere closes it too.
            c.input(&button(x, y, true, Button::Primary), st, 5.0);
            c.input(&button(x, y, false, Button::Primary), st, 5.0);
            assert!(c.overflow_menu.is_some());
            c.input(&button(190.0, 100.0, true, Button::Primary), st, 5.0);
            assert_eq!(c.overflow_menu, None);
        });
    }
}

/// MPR's toolbar button shows only while the image is a volume (any volume: tier 1), and
/// as on while MPR is open; Settings still lists it.
#[test]
fn mpr_button_only_for_volumes() {
    let mut text = Text::new();
    let has = |c: &Chrome| c.hits.iter().any(|(_, t)| *t == Target::Toolbar(T::Mpr));
    with_state(STATES[3], |st| {
        let mut c = chrome();
        draw(&mut c, &mut text, st);
        assert!(!has(&c), "no volume, no MPR button");
        assert!(current(st.store).contains(&T::Mpr), "in the stored list");
    });
    for mpr in [false, true] {
        let s = specular_input::ActionState {
            volume: true,
            mpr,
            ..STATES[3]
        };
        with_state(s, |st| {
            let mut c = chrome();
            draw(&mut c, &mut text, st);
            assert!(has(&c), "a volume: the MPR button");
            let (label, a) = crate::toolbar::item_label(T::Mpr, st);
            assert_eq!(a, Some(Action::Mpr));
            assert_eq!(label.contains("Close"), mpr, "{label}");
        });
    }
}

/// The Discord button: last on the default bar, as big as Settings beside it; it runs Join
/// the Specular Discord, which its tooltip and accessible name say. In the narrowest window
/// (360 px, the window's minimum) it is the first to go into the More menu, which still fits.
#[test]
fn discord_button() {
    let mut text = Text::new();
    assert_eq!(specular_settings::DEFAULT_TOOLBAR.last(), Some(&T::Discord));
    // Enabled with nothing open (the bar shows once an image is).
    with_state(STATES[0], |st| {
        let r = Chrome::new().activate(Target::Toolbar(T::Discord), st);
        assert!(
            matches!(
                r.commands[..],
                [crate::Command::Action(Action::JoinDiscord)]
            ),
            "{:?}",
            r.commands
        );
    });
    with_state(STATES[3], |st| {
        let mut c = chrome();
        c.set_accessibility(true);
        draw(&mut c, &mut text, st);
        let d = hit(&c, &Target::Toolbar(T::Discord));
        let s = hit(&c, &Target::Toolbar(T::Settings));
        assert_eq!(
            (d.w, d.h, d.y),
            (s.w, s.h, s.y),
            "same size and row as Settings"
        );
        assert!(d.x > s.x, "after Settings");
        // It asks the app to open the invite.
        let r = c.activate(Target::Toolbar(T::Discord), st);
        assert!(
            matches!(
                r.commands[..],
                [crate::Command::Action(Action::JoinDiscord)]
            ),
            "{:?}",
            r.commands
        );
        let tip = Target::Toolbar(T::Discord).tooltip(st).unwrap_or_default();
        assert!(tip.contains("Join the") && tip.contains("Discord"), "{tip}");
        let t = c.accessibility_tree(st, "x", 1.0);
        let n = t
            .nodes
            .iter()
            .find(|(_, n)| {
                n.label()
                    .is_some_and(|l| l.contains("Join the") && l.contains("Discord"))
            })
            .map(|(_, n)| n.clone());
        assert_eq!(n.map(|n| n.role()), Some(Role::Button));

        let mut c = chrome();
        c.set_size(360.0, 260.0);
        draw(&mut c, &mut text, st);
        assert!(
            c.toolbar_overflow.contains(&T::Discord),
            "in More at 360 px"
        );
        assert!(
            !c.hits
                .iter()
                .any(|(_, t)| *t == Target::Toolbar(T::Discord))
        );
        let more = hit(&c, &Target::ToolbarMore);
        assert!(more.x >= 0.0 && more.right() <= 360.0);
    });
}
