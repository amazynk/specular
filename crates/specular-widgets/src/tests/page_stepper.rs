use crate::accesskit::Role;
use crate::ctx::Target;
use crate::{Chrome, Command, HudStats, Input, Overlays, PageBadge, Playback, ViewerState};
use specular_gfx::{DrawList, Rect, Text};
use specular_input::{Action, ActionState, ImageState, Keymap};
use specular_settings::{Settings, SettingsStore, ToolbarItem};
use specular_theme::Palette;

struct S<'a> {
    pages: Option<PageBadge<'a>>,
    playback: Option<Playback>,
    file: &'a str,
    loading: bool,
}

fn s(pages: Option<PageBadge<'_>>) -> S<'_> {
    S {
        pages,
        playback: None,
        file: "icon.icns",
        loading: false,
    }
}

fn with<R>(s: S, f: impl FnOnce(&ViewerState) -> R) -> R {
    let hud = HudStats::default();
    let keymap = Keymap::default();
    let store = SettingsStore::in_memory();
    let settings = Settings::default();
    let st = ViewerState {
        actions: ActionState {
            image: ImageState::Ready,
            count: 3,
            pages: s.pages.map_or(0, |p| p.count),
            animated: s.playback.is_some(),
            video: false,
            sound: false,
            sound_file: false,
            ..ActionState::default()
        },
        has_image: true,
        index: 0,
        count: 3,
        zoom: 1.0,
        at_fit: true,
        fullscreen: false,
        file_name: Some(s.file),
        dims: Some((256, 256)),
        loading: s.loading,
        error: None,
        truncated: false,
        metadata: None,
        hud: &hud,
        keymap: &keymap,
        store: &store,
        settings: &settings,
        overlays: Overlays::default(),
        playback: s.playback,
        motion: None,
        spread: None,
        pages: s.pages,
    };
    f(&st)
}

fn page(index: u32, label: Option<&str>) -> Option<PageBadge<'_>> {
    Some(PageBadge {
        index,
        count: 5,
        label,
    })
}

fn chrome() -> Chrome {
    let mut c = Chrome::new();
    c.set_size(1000.0, 700.0);
    c.set_accessibility(true);
    c.poke(1.0);
    c
}

fn draw(c: &mut Chrome, text: &mut Text, st: &ViewerState, now: f64) {
    let mut dl = DrawList::new(1.0);
    for i in 0..3 {
        dl.clear(1.0);
        c.draw(&mut dl, text, st, &Palette::dark(), now + i as f64 * 0.016);
    }
}

fn hit(c: &Chrome, t: Target) -> Option<Rect> {
    c.hits.iter().rev().find(|(_, h)| *h == t).map(|h| h.0)
}

fn plain(s: String) -> String {
    specular_i18n::strip_isolates(&s)
}

const PREV: Target = Target::Transport(Action::PrevPage);
const NEXT: Target = Target::Transport(Action::NextPage);

#[test]
fn texts_and_ends() {
    let p = page(2, Some("Face +Z")).unwrap();
    assert_eq!(plain(crate::pages::position_text(&p)), "Page 3 of 5");
    assert_eq!(plain(crate::pages::spoken(&p)), "Page 3 of 5, Face +Z");
    let p = page(0, None).unwrap();
    assert_eq!(plain(crate::pages::spoken(&p)), "Page 1 of 5");
    assert!(!crate::pages::can_step(&p, Action::PrevPage));
    assert!(!crate::pages::can_step(&p, Action::FirstPage));
    assert!(crate::pages::can_step(&p, Action::NextPage));
    let last = page(4, None).unwrap();
    assert!(!crate::pages::can_step(&last, Action::NextPage));
    assert!(!crate::pages::can_step(&last, Action::LastPage));
    assert!(crate::pages::can_step(&last, Action::PrevPage));
}

/// Drawn only for files with pages; above the toolbar, above the transport bar when an
/// animation shows one; its buttons run the page actions.
#[test]
fn drawn_above_the_toolbar_and_runs_the_page_actions() {
    let mut text = Text::new();
    with(s(None), |st| {
        let mut c = chrome();
        draw(&mut c, &mut text, st, 1.0);
        assert!(hit(&c, NEXT).is_none() && hit(&c, PREV).is_none());
    });
    let alone = with(s(page(2, Some("Layer 3"))), |st| {
        let mut c = chrome();
        draw(&mut c, &mut text, st, 1.0);
        let (p, n) = (hit(&c, PREV).unwrap(), hit(&c, NEXT).unwrap());
        assert!(p.x < n.x, "previous on the left: {p:?} {n:?}");
        assert!((p.y - n.y).abs() < 0.5);
        let bar = hit(&c, Target::Toolbar(ToolbarItem::Next)).unwrap();
        assert!(n.bottom() < bar.y, "{n:?} above the toolbar {bar:?}");
        // A click runs the action.
        let (x, y) = n.center();
        let mut cmds = c.input(&Input::PointerMove { x, y }, st, 1.1).commands;
        for down in [true, false] {
            let ev = Input::PointerButton {
                x,
                y,
                down,
                button: crate::Button::Primary,
            };
            cmds.extend(c.input(&ev, st, 1.1).commands);
        }
        assert!(
            cmds.iter()
                .any(|c| matches!(c, Command::Action(Action::NextPage))),
            "{cmds:?}"
        );
        // The tooltip names the action.
        let tip = NEXT.tooltip(st).unwrap();
        assert!(tip.contains("Next page"), "{tip}");
        n
    });
    let pb = Playback {
        frame: 0,
        count: Some(8),
        playing: false,
        speed: 1.0,
        video: None,
    };
    let anim = S {
        playback: Some(pb),
        ..s(page(2, None))
    };
    with(anim, |st| {
        let mut c = chrome();
        draw(&mut c, &mut text, st, 1.0);
        let n = hit(&c, NEXT).unwrap();
        let frame = hit(&c, Target::Transport(Action::NextFrame)).unwrap();
        assert!(n.bottom() < frame.y, "stacked above the transport bar");
        assert!(n.y < alone.y);
    });
}

/// A toast (the playback speed, say) stacks above the transport bar and the stepper, so it
/// never covers their buttons.
#[test]
fn toasts_stack_above_the_bars() {
    let mut text = Text::new();
    let anim = S {
        playback: Some(Playback {
            frame: 0,
            count: Some(8),
            playing: true,
            speed: 2.0,
            video: None,
        }),
        ..s(page(2, None))
    };
    with(anim, |st| {
        let mut c = chrome();
        c.toast("Playback speed 2×".into(), None, 0.5);
        draw(&mut c, &mut text, st, 1.0);
        let toast = hit(&c, Target::Toast).expect("toast");
        for t in [
            Target::Transport(Action::FasterPlayback),
            Target::Transport(Action::SlowerPlayback),
            NEXT,
        ] {
            let b = hit(&c, t.clone()).unwrap();
            assert!(toast.bottom() <= b.y, "{t:?}: {toast:?} over {b:?}");
        }
    });
}

/// Right to left: previous sits on the right. The buttons carry the page as their value.
#[test]
fn right_to_left_and_the_value() {
    let mut text = Text::new();
    with(s(page(0, None)), |st| {
        let mut c = chrome();
        c.force_rtl = true;
        draw(&mut c, &mut text, st, 1.0);
        let (p, n) = (hit(&c, PREV).unwrap(), hit(&c, NEXT).unwrap());
        assert!(p.x > n.x, "previous on the right in RTL: {p:?} {n:?}");
        let t = c.accessibility_tree(st, "x", 1.0);
        let next = t
            .nodes
            .iter()
            .find(|(_, n)| n.role() == Role::Button && n.label() == Some("Next page"))
            .expect("next page button");
        assert_eq!(
            next.1.value().map(|v| plain(v.to_owned())).as_deref(),
            Some("Page 1 of 5")
        );
    });
}

/// A page change in the same file is announced with its label, once it has opened; a new
/// file is not.
#[test]
fn page_changes_are_announced() {
    let mut text = Text::new();
    let mut c = chrome();
    with(s(page(0, Some("512×512 @2x"))), |st| {
        draw(&mut c, &mut text, st, 1.0)
    });
    assert!(c.announce.is_none());
    // Opening: wait for the label.
    let opening = S {
        loading: true,
        ..s(page(1, None))
    };
    with(opening, |st| draw(&mut c, &mut text, st, 1.1));
    assert!(c.announce.is_none());
    with(s(page(1, Some("256×256"))), |st| {
        draw(&mut c, &mut text, st, 1.2)
    });
    assert_eq!(
        c.announce.as_ref().map(|a| plain(a.1.clone())).as_deref(),
        Some("Page 2 of 5, 256×256")
    );
    c.announce = None;
    // Another file: no announcement.
    let other = S {
        file: "other.ktx",
        ..s(page(3, None))
    };
    with(other, |st| draw(&mut c, &mut text, st, 1.3));
    assert!(c.announce.is_none());
}
