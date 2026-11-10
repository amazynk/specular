use super::enabled_state::STATES;
use crate::bar_drag::{PICKUP_HOLD, REPEAT_DELAY, REPEAT_EVERY};
use crate::ctx::Target;
use crate::settings::SettingsTarget;
use crate::toolbar_edit::{Part, RowOp, commit, current, part_enabled, preview_needed, row_ops};
use crate::{Button, Chrome, Command, HudStats, Input, Modal, Overlays, Response, ViewerState};
use specular_gfx::{DrawList, Rect, Text};
use specular_input::{Action, ActionState, Key, KeyCombo, Keymap, Mods, NamedKey};
use specular_settings::{Settings, SettingsStore, ToolbarItem as T};
use specular_theme::Palette;

/// A viewer with an image (the first of four) whose toolbar is `items`.
fn with_items<R>(items: Option<&[T]>, f: impl FnOnce(&ViewerState) -> R) -> R {
    let hud = HudStats::default();
    let keymap = Keymap::default();
    let store = SettingsStore::in_memory();
    let mut settings = Settings::default();
    if let Some(l) = items {
        commit(&store, l);
        settings.viewing.toolbar_items = l.to_vec();
    }
    let actions: ActionState = STATES[3];
    let st = ViewerState {
        actions,
        has_image: true,
        index: 0,
        count: actions.count,
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

fn chrome(w: f32, h: f32, rtl: bool) -> Chrome {
    let mut c = Chrome::new();
    c.set_size(w, h);
    c.force_rtl = rtl;
    c.poke(1.0);
    c
}

/// Frames from `t0`, 16 ms apart.
fn frames(c: &mut Chrome, text: &mut Text, st: &ViewerState, t0: f64, n: usize) {
    let mut dl = DrawList::new(1.0);
    for i in 0..n {
        dl.clear(1.0);
        c.draw(&mut dl, text, st, &Palette::dark(), t0 + i as f64 * 0.016);
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

fn press(c: &mut Chrome, st: &ViewerState, p: (f32, f32), now: f64) -> Response {
    let ev = Input::PointerButton {
        x: p.0,
        y: p.1,
        down: true,
        button: Button::Primary,
    };
    c.input(&ev, st, now)
}

fn release(c: &mut Chrome, st: &ViewerState, p: (f32, f32), now: f64) -> Response {
    let ev = Input::PointerButton {
        x: p.0,
        y: p.1,
        down: false,
        button: Button::Primary,
    };
    c.input(&ev, st, now)
}

fn to(c: &mut Chrome, st: &ViewerState, p: (f32, f32), now: f64) -> Response {
    c.input(&Input::PointerMove { x: p.0, y: p.1 }, st, now)
}

fn key(k: NamedKey, mods: Mods) -> Input {
    Input::Key {
        combo: KeyCombo::new(Key::Named(k), mods),
        repeat: false,
    }
}

fn defaults() -> Vec<T> {
    current(&SettingsStore::in_memory())
}

#[test]
fn row_operations_and_their_ends() {
    let l = defaults();
    let n = l.len();
    // Move up and Move down are on the shown rows only, and not at the ends.
    assert!(!part_enabled(n, 0, Part::Up));
    assert!(part_enabled(n, 0, Part::Down));
    assert!(part_enabled(n, n - 1, Part::Up));
    assert!(!part_enabled(n, n - 1, Part::Down));
    assert!(!part_enabled(n, n + 1, Part::Up));
    assert!(!part_enabled(n, n + 1, Part::Down));
    // The menu: Move to start and Move to end are not offered where the row already is;
    // a hidden row can only be shown.
    assert_eq!(
        row_ops(&l, 0),
        vec![
            (RowOp::Start, false),
            (RowOp::End, true),
            (RowOp::Toggle, true)
        ]
    );
    assert_eq!(
        row_ops(&l, n - 1),
        vec![
            (RowOp::Start, true),
            (RowOp::End, false),
            (RowOp::Toggle, true)
        ]
    );
    assert_eq!(row_ops(&l, n), vec![(RowOp::Toggle, true)]);

    let mut text = Text::new();
    with_items(None, |st| {
        let mut c = chrome(1000.0, 800.0, false);
        c.open_settings_at("viewing.toolbar_items");
        frames(&mut c, &mut text, st, 1.0, 20);
        // The first row has Move up drawn but disabled; hidden rows have none.
        hit(&c, &Target::Settings(SettingsTarget::ToolUp(0)));
        hit(&c, &Target::Settings(SettingsTarget::ToolMenu(0)));
        assert!(
            !c.hits
                .iter()
                .any(|(_, t)| *t == Target::Settings(SettingsTarget::ToolUp(n)))
        );

        // Move up at the start and Move down at the end change nothing.
        c.tools_click(SettingsTarget::ToolUp(0), st);
        c.tools_click(SettingsTarget::ToolDown(n - 1), st);
        assert_eq!(current(st.store), l);
        // Move down: one undo step, announced, the focus on the moved row's button.
        c.tools_click(SettingsTarget::ToolDown(0), st);
        assert_eq!(current(st.store)[1], l[0]);
        assert_eq!(c.settings.tools.focus, Some(1));
        assert_eq!(c.settings.tools.part, Part::Down);
        assert!(c.announce.as_ref().is_some_and(|a| !a.1.is_empty()));
        st.store.undo();
        assert_eq!(current(st.store), l);
        // Move up.
        c.tools_click(SettingsTarget::ToolUp(3), st);
        assert_eq!(current(st.store)[2], l[3]);
        assert_eq!(c.settings.tools.focus, Some(2));
        st.store.undo();

        // The menu: Move to end, then Move to start, each one undo step; the focus stays
        // on the moved row (its menu button).
        c.tools_click(SettingsTarget::ToolMenu(2), st);
        assert_eq!(c.settings.tools.menu, Some(2));
        c.tools_click(SettingsTarget::ToolMenuItem(2, RowOp::End), st);
        assert_eq!(c.settings.tools.menu, None);
        assert_eq!(current(st.store)[n - 1], l[2]);
        assert_eq!(c.settings.tools.focus, Some(n - 1));
        assert_eq!(c.settings.tools.part, Part::Menu);
        c.tools_click(SettingsTarget::ToolMenu(n - 1), st);
        c.tools_click(SettingsTarget::ToolMenuItem(n - 1, RowOp::Start), st);
        assert_eq!(current(st.store)[0], l[2]);
        assert_eq!(c.settings.tools.focus, Some(0));
        st.store.undo();
        st.store.undo();
        assert_eq!(current(st.store), l);
        // A disabled entry does nothing and leaves the menu open.
        c.tools_click(SettingsTarget::ToolMenu(0), st);
        c.tools_click(SettingsTarget::ToolMenuItem(0, RowOp::Start), st);
        assert_eq!(c.settings.tools.menu, Some(0));
        assert_eq!(current(st.store), l);
        // Hide, from the menu: the row goes to the hidden ones.
        c.tools_click(SettingsTarget::ToolMenuItem(0, RowOp::Toggle), st);
        assert!(!current(st.store).contains(&l[0]));
        st.store.undo();
    });
}

/// The `--show` hooks for screenshots: a row's menu open, the bar mid-drag.
#[test]
fn screenshot_hooks() {
    let mut text = Text::new();
    with_items(None, |st| {
        let mut c = chrome(1000.0, 800.0, false);
        frames(&mut c, &mut text, st, 1.0, 10);
        c.show_bar_drag(1, 4);
        frames(&mut c, &mut text, st, 1.1, 10);
        assert!(c.bar_press.as_ref().is_some_and(|d| d.active));
        // A new window size drops a press made on the old bar (the hook picks up again).
        let old = c.bar_press.as_ref().unwrap().geom.bar;
        c.set_size(1200.0, 900.0);
        frames(&mut c, &mut text, st, 1.3, 3);
        let d = c.bar_press.as_ref().unwrap();
        assert_eq!(d.geom.window, (1200.0, 900.0));
        // The stored order, without MPR (no volume on screen).
        let shown: Vec<_> = defaults().into_iter().filter(|t| *t != T::Mpr).collect();
        assert_eq!(d.geom.items, shown, "picked up from the stored order");
        assert_eq!(d.item, defaults()[1]);
        assert_ne!(d.geom.bar, old);
        assert!(d.drop_at().is_some(), "held over the bar");
        assert!(c.back());
        frames(&mut c, &mut text, st, 1.4, 2);
        assert!(c.bar_press.is_none());
        c.open_settings_at("viewing.toolbar_items");
        c.show_toolbar_row_menu(2);
        frames(&mut c, &mut text, st, 2.0, 10);
        assert_eq!(c.settings.tools.menu, Some(2));
        hit(
            &c,
            &Target::Settings(SettingsTarget::ToolMenuItem(2, RowOp::End)),
        );
    });
}

#[test]
fn row_menu_by_keyboard() {
    let mut text = Text::new();
    with_items(None, |st| {
        let l = defaults();
        let mut c = chrome(1000.0, 800.0, false);
        c.open_settings_at("viewing.toolbar_items");
        frames(&mut c, &mut text, st, 1.0, 20);
        c.input(&key(NamedKey::Tab, Mods::NONE), st, 2.0);
        assert_eq!(c.settings.tools.focus, Some(0));
        // → walks the row's parts in reading order, past the disabled Move up.
        c.input(&key(NamedKey::ArrowRight, Mods::NONE), st, 2.0);
        assert_eq!(c.settings.tools.part, Part::Down);
        c.input(&key(NamedKey::ArrowRight, Mods::NONE), st, 2.0);
        assert_eq!(c.settings.tools.part, Part::Menu);
        // Enter opens the menu on its first entry that can run (Move to end here).
        c.input(&key(NamedKey::Enter, Mods::NONE), st, 2.0);
        assert_eq!(c.settings.tools.menu, Some(0));
        assert_eq!(c.settings.tools.menu_sel, Some(1));
        frames(&mut c, &mut text, st, 2.0, 2);
        hit(
            &c,
            &Target::Settings(SettingsTarget::ToolMenuItem(0, RowOp::End)),
        );
        // ↓ goes to Hide, ↑ back; ↑ at the top stays on the first that can run.
        c.input(&key(NamedKey::ArrowDown, Mods::NONE), st, 2.0);
        assert_eq!(c.settings.tools.menu_sel, Some(2));
        c.input(&key(NamedKey::ArrowUp, Mods::NONE), st, 2.0);
        assert_eq!(c.settings.tools.menu_sel, Some(1));
        // Esc closes the menu only (Settings stays open).
        assert!(c.back());
        assert_eq!(c.settings.tools.menu, None);
        assert_eq!(c.modal, Some(Modal::Settings));
        // Shift+F10 opens it from the row; Enter runs Move to end.
        c.input(&key(NamedKey::ArrowLeft, Mods::NONE), st, 2.0);
        let shift = Mods {
            shift: true,
            ..Mods::NONE
        };
        c.input(&key(NamedKey::F(10), shift), st, 2.0);
        assert_eq!(c.settings.tools.menu, Some(0));
        c.input(&key(NamedKey::Enter, Mods::NONE), st, 2.0);
        let now = current(st.store);
        assert_eq!(now[now.len() - 1], l[0]);
        assert_eq!(c.settings.tools.focus, Some(now.len() - 1));
        // Alt+Home moves it back to the start.
        let alt = Mods {
            alt: true,
            ..Mods::NONE
        };
        c.input(&key(NamedKey::Home, alt), st, 2.0);
        assert_eq!(current(st.store), l);
        assert_eq!(c.settings.tools.focus, Some(0));
    });
}

#[test]
fn row_controls_mirror_and_fit() {
    let mut text = Text::new();
    for (rtl, scale) in [(false, 1.0), (true, 1.0), (false, 2.0)] {
        with_items(None, |st| {
            let mut c = chrome(1000.0, 800.0, rtl);
            c.text_scale = scale;
            c.open_settings_at("viewing.toolbar_items");
            frames(&mut c, &mut text, st, 1.0, 20);
            let row = hit(&c, &Target::Settings(SettingsTarget::ToolRow(1)));
            let up = hit(&c, &Target::Settings(SettingsTarget::ToolUp(1)));
            let down = hit(&c, &Target::Settings(SettingsTarget::ToolDown(1)));
            let menu = hit(&c, &Target::Settings(SettingsTarget::ToolMenu(1)));
            let switch = hit(&c, &Target::Settings(SettingsTarget::ToolShow(1)));
            for r in [up, down, menu, switch] {
                assert!(
                    r.x >= row.x - 0.5 && r.right() <= row.right() + 0.5,
                    "{r:?} in {row:?}"
                );
            }
            // In reading order: Move up, Move down, ⋯, then the switch at the end.
            let order = [up, down, menu, switch];
            for w in order.windows(2) {
                if rtl {
                    assert!(w[0].x > w[1].right() - 0.5, "{order:?}");
                } else {
                    assert!(w[0].right() < w[1].x + 0.5, "{order:?}");
                }
            }
        });
    }
}

/// A press on the bar that moves less than the threshold is a click; past it, the item is
/// picked up, the bar reflows, and the release drops it (one undo step).
#[test]
fn click_or_drag_on_the_bar() {
    let mut text = Text::new();
    with_items(None, |st| {
        let l = defaults();
        let mut c = chrome(1000.0, 800.0, false);
        frames(&mut c, &mut text, st, 1.0, 10);
        let rl = hit(&c, &Target::Toolbar(T::RotateLeft));
        let p = rl.center();
        press(&mut c, st, p, 2.0);
        to(&mut c, st, (p.0 + 3.0, p.1 + 2.0), 2.05);
        let r = release(&mut c, st, (p.0 + 3.0, p.1 + 2.0), 2.1);
        assert_eq!(r.commands, vec![Command::Action(Action::RotateLeft)]);
        assert_eq!(current(st.store), l);

        // Dragged past Fit and Zoom in: the bar shows it there before the release.
        let zi = hit(&c, &Target::Toolbar(T::ZoomIn));
        let at = (zi.x + 4.0, zi.center().1 - 10.0);
        press(&mut c, st, p, 3.0);
        to(&mut c, st, (p.0 - 8.0, p.1), 3.02);
        assert!(c.bar_press.as_ref().is_some_and(|d| d.active));
        let r = to(&mut c, st, at, 3.05);
        assert!(r.consumed, "the pointer is captured");
        frames(&mut c, &mut text, st, 3.06, 2);
        // The bar shows the empty place where Zoom in was (Zoom in and Fit move along).
        let socket = c.bar_press.as_ref().unwrap().shown().1;
        let at_ix = l.iter().position(|t| *t == T::ZoomIn).unwrap();
        assert_eq!(socket, at_ix);
        // No hiding while dragging.
        frames(&mut c, &mut text, st, 30.0, 3);
        assert!(c.toolbar_alpha > 0.99);
        let r = release(&mut c, st, at, 30.1);
        assert!(r.commands.is_empty(), "a drop is not a click");
        let after = current(st.store);
        assert_eq!(after.len(), l.len());
        assert_eq!(after[at_ix], T::RotateLeft, "{after:?}");
        assert!(c.announce.as_ref().is_some_and(|a| !a.1.is_empty()));
        st.store.undo();
        assert_eq!(current(st.store), l);
    });
}

/// Esc, or a release off the bar, puts the item back; holding a button still picks it up.
#[test]
fn cancel_and_hold_to_pick_up() {
    let mut text = Text::new();
    with_items(None, |st| {
        let l = defaults();
        let mut c = chrome(1000.0, 800.0, false);
        frames(&mut c, &mut text, st, 1.0, 10);
        let p = hit(&c, &Target::Toolbar(T::Info)).center();
        let first = hit(&c, &Target::Toolbar(T::Prev)).center();

        press(&mut c, st, p, 2.0);
        to(&mut c, st, first, 2.1);
        assert!(c.back(), "Esc cancels the drag");
        assert!(c.bar_press.is_none());
        let r = release(&mut c, st, first, 2.2);
        assert!(r.commands.is_empty());
        assert_eq!(current(st.store), l);

        // Off the bar (well above it).
        press(&mut c, st, p, 3.0);
        to(&mut c, st, (p.0, p.1 - 200.0), 3.1);
        frames(&mut c, &mut text, st, 3.1, 2);
        release(&mut c, st, (p.0, p.1 - 200.0), 3.2);
        assert_eq!(current(st.store), l, "nothing is hidden by dragging");
        frames(&mut c, &mut text, st, 3.3, 2);

        // Held still: picked up without moving.
        press(&mut c, st, p, 4.0);
        frames(&mut c, &mut text, st, 4.0 + PICKUP_HOLD * 0.5, 1);
        assert!(c.bar_press.as_ref().is_some_and(|d| !d.active));
        frames(&mut c, &mut text, st, 4.0 + PICKUP_HOLD + 0.01, 1);
        assert!(c.bar_press.as_ref().is_some_and(|d| d.active));
        let r = release(&mut c, st, p, 4.6);
        assert!(r.commands.is_empty(), "a held button is not clicked");
        assert_eq!(current(st.store), l);

        // Turned off: a drag on the bar does nothing.
        st.store
            .set(
                "viewing.toolbar_drag",
                specular_settings::Value::Bool(false),
                specular_settings::Origin::SettingsWindow,
            )
            .unwrap();
        press(&mut c, st, p, 5.0);
        to(&mut c, st, first, 5.1);
        assert!(c.bar_press.is_none());
        release(&mut c, st, first, 5.2);
        assert_eq!(current(st.store), l);
    });
}

/// Held still, Previous and Next repeat (the app flips through the folder) and are never
/// picked up; their release is not a click. Moved, they are dragged like the rest.
#[test]
fn previous_and_next_repeat_when_held() {
    let mut text = Text::new();
    with_items(None, |st| {
        let l = defaults();
        let mut c = chrome(1000.0, 800.0, false);
        frames(&mut c, &mut text, st, 1.0, 10);
        let p = hit(&c, &Target::Toolbar(T::Next)).center();
        press(&mut c, st, p, 2.0);
        frames(&mut c, &mut text, st, 2.0, 1);
        assert!(c.take_commands().is_empty());
        // Past the hold-to-pick-up time and the repeat delay: repeats, no drag.
        frames(&mut c, &mut text, st, 2.0 + REPEAT_DELAY + 0.01, 30);
        assert!(c.bar_press.is_none(), "a held Next is not picked up");
        let cmds = c.take_commands();
        assert!(cmds.len() > 3, "{cmds:?}");
        assert!(
            cmds.iter()
                .all(|c| matches!(c, Command::NavHold { next: true, .. }))
        );
        assert!(c.toolbar_alpha > 0.99, "the bar stays while held");
        let r = release(&mut c, st, p, 3.0);
        assert_eq!(r.commands, vec![Command::NavHoldEnd]);
        assert_eq!(current(st.store), l);

        // A quick click is a click.
        press(&mut c, st, p, 4.0);
        let r = release(&mut c, st, p, 4.1);
        assert_eq!(r.commands, vec![Command::Action(Action::NextImage)]);
        assert!(c.take_commands().is_empty());

        // Moved: dragged (and no repeat).
        let end = hit(&c, &Target::Toolbar(T::Discord));
        press(&mut c, st, p, 5.0);
        to(&mut c, st, (end.right() - 2.0, end.center().1), 5.05);
        frames(&mut c, &mut text, st, 5.06, 40);
        assert!(c.take_commands().is_empty());
        release(&mut c, st, (end.right() - 2.0, end.center().1), 5.8);
        let after = current(st.store);
        assert_eq!(*after.last().unwrap(), T::Next, "{after:?}");
    });
}

/// The hold-to-repeat timings are decided: the first repeat 0.4 s after the press, then one
/// every 0.08 s.
#[test]
fn hold_to_repeat_timings() {
    assert_eq!((REPEAT_DELAY, REPEAT_EVERY), (0.4, 0.08));
    let mut text = Text::new();
    with_items(None, |st| {
        let mut c = chrome(1000.0, 800.0, false);
        frames(&mut c, &mut text, st, 1.0, 10);
        let p = hit(&c, &Target::Toolbar(T::Next)).center();
        press(&mut c, st, p, 2.0);
        let held = |c: &mut Chrome| -> Vec<f64> {
            c.take_commands()
                .into_iter()
                .map(|cmd| match cmd {
                    Command::NavHold { next: true, held } => held,
                    other => panic!("{other:?}"),
                })
                .collect()
        };
        let first = 2.0 + REPEAT_DELAY;
        let wake = c.bar_hold_tick(first - 0.005);
        assert!(held(&mut c).is_empty(), "nothing before 0.4 s");
        assert!(wake.is_some_and(|w| w <= 0.005 + 1e-9), "{wake:?}");
        c.bar_hold_tick(first);
        let h = held(&mut c);
        assert_eq!(h.len(), 1);
        assert!((h[0] - 0.4).abs() < 1e-9, "{h:?}");
        let mut due = first;
        for _ in 0..3 {
            due += REPEAT_EVERY;
            c.bar_hold_tick(due - 0.005);
            assert!(held(&mut c).is_empty(), "nothing before the next 0.08 s");
            c.bar_hold_tick(due);
            assert_eq!(held(&mut c).len(), 1);
        }
        assert_eq!(
            release(&mut c, st, p, due + 0.01).commands,
            vec![Command::NavHoldEnd]
        );
    });
}

/// Right to left the bar is mirrored, and so are the drops; spacers and separators are
/// dragged too, and nothing goes in or out of the More menu.
#[test]
fn right_to_left_separators_and_overflow() {
    let mut text = Text::new();
    let items = [T::Prev, T::Next, T::Separator, T::Fit, T::Info];
    with_items(Some(&items), |st| {
        let mut c = chrome(1000.0, 800.0, true);
        frames(&mut c, &mut text, st, 1.0, 10);
        let prev = hit(&c, &Target::Toolbar(T::Prev));
        let info = hit(&c, &Target::Toolbar(T::Info));
        assert!(prev.x > info.x, "Previous is on the right");
        let sep = hit(&c, &Target::BarSlot(2)).center();
        // The separator dragged to the left end: the end of the list.
        press(&mut c, st, sep, 2.0);
        to(&mut c, st, (info.x + 2.0, info.center().1), 2.1);
        frames(&mut c, &mut text, st, 2.1, 2);
        release(&mut c, st, (info.x + 2.0, info.center().1), 2.2);
        assert_eq!(
            current(st.store),
            vec![T::Prev, T::Next, T::Fit, T::Info, T::Separator]
        );
    });

    // A narrow bar: dropped past its end, an item stays on the bar.
    with_items(None, |st| {
        let l = defaults();
        let mut c = chrome(380.0, 700.0, false);
        frames(&mut c, &mut text, st, 1.0, 10);
        assert!(!c.toolbar_overflow.is_empty());
        let shown = c.bar_geom.as_ref().unwrap().slots.len();
        let more = hit(&c, &Target::ToolbarMore);
        let p = hit(&c, &Target::Toolbar(T::Prev)).center();
        press(&mut c, st, p, 2.0);
        to(&mut c, st, (more.right() + 10.0, more.center().1), 2.1);
        frames(&mut c, &mut text, st, 2.1, 2);
        release(&mut c, st, (more.right() + 10.0, more.center().1), 2.2);
        let after = current(st.store);
        assert_eq!(after[shown - 1], T::Prev, "{after:?}");
        assert_eq!(after[shown..], l[shown..], "the More menu keeps its items");
        // A row of the More menu is not picked up.
        st.store.undo();
        assert_eq!(current(st.store), l);
        c.show_overflow_menu();
        frames(&mut c, &mut text, st, 3.0, 2);
        let over = c.toolbar_overflow[0];
        let row = hit(&c, &Target::Toolbar(over)).center();
        press(&mut c, st, row, 3.1);
        to(&mut c, st, p, 3.2);
        assert!(c.bar_press.is_none());
    });
}

#[test]
fn preview_or_live_bar() {
    let sheet = Rect::new(50.0, 70.0, 900.0, 660.0);
    // No bar on screen.
    assert!(preview_needed(None, sheet));
    // Below the sheet: uncovered.
    assert!(!preview_needed(
        Some(Rect::new(300.0, 738.0, 400.0, 46.0)),
        sheet
    ));
    // Overlapping its bottom edge: covered.
    assert!(preview_needed(
        Some(Rect::new(300.0, 700.0, 400.0, 46.0)),
        sheet
    ));
    // Beside it.
    assert!(!preview_needed(
        Some(Rect::new(960.0, 600.0, 30.0, 46.0)),
        sheet
    ));

    let mut text = Text::new();
    with_items(None, |st| {
        // A tall window: the bar is below the sheet; the list shows no preview, the real
        // bar stays (past the auto-hide delay) and takes the list's moves.
        let mut c = chrome(1000.0, 1080.0, false);
        c.open_settings_at("viewing.toolbar_items");
        frames(&mut c, &mut text, st, 1.0, 10);
        assert!(c.settings.tools.live);
        frames(&mut c, &mut text, st, 60.0, 30);
        assert!(c.settings.tools.live);
        assert!(c.toolbar_alpha > 0.99, "kept from auto-hiding");
        let bar = c.bar_geom.as_ref().unwrap().bar;
        // The sheet is as tall as its sidebar needs, centred, and ends above the bar.
        assert!(bar.y >= c.settings.side.bottom(), "{bar:?}");
        c.tools_click(SettingsTarget::ToolDown(0), st);
        frames(&mut c, &mut text, st, 61.0, 2);
        assert_eq!(
            c.bar_geom.as_ref().unwrap().items,
            crate::toolbar::for_content(current(st.store), st)
        );
        // The live bar takes no clicks: the dialog is modal.
        let p = c.bar_geom.as_ref().unwrap().slots[0].center();
        assert!(!c.hits.iter().any(
                |(r, t)| r.contains(p) && matches!(t, Target::Toolbar(_) | Target::BarSlot(_))
            ));
        st.store.undo();

        // Another page: the bar hides as usual.
        c.open_modal(Modal::Palette);
        c.open_settings_at("performance");
        frames(&mut c, &mut text, st, 120.0, 60);
        assert!(!c.settings.tools.live);
        assert!(c.toolbar_alpha < 0.01);

        // A shorter window: the sheet covers the bar, the list previews it.
        let mut c = chrome(1000.0, 720.0, false);
        c.open_settings_at("viewing.toolbar_items");
        frames(&mut c, &mut text, st, 1.0, 10);
        assert!(!c.settings.tools.live);
        // The gallery hides the bar: a preview too.
        let mut c = chrome(1000.0, 800.0, false);
        c.gallery = true;
        c.open_settings_at("viewing.toolbar_items");
        frames(&mut c, &mut text, st, 1.0, 10);
        assert!(!c.settings.tools.live);
    });
}
