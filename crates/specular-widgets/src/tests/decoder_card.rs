use super::enabled_state::{STATES, with_state};
use crate::accesskit::Role;
use crate::ctx::Target;
use crate::{Chrome, Command, DecoderCard, ViewerState};
use specular_gfx::{DrawList, Text};
use specular_theme::Palette;

fn card() -> DecoderCard {
    DecoderCard {
        id: "braw".into(),
        title: "Blackmagic RAW needs Blackmagic Design\u{2019}s free decoder".into(),
        lines: vec![
            "Install it once.".into(),
            "Specular never downloads it.".into(),
        ],
        buttons: vec![
            "Get it".into(),
            "I\u{2019}ve installed it".into(),
            "Locate\u{2026}".into(),
        ],
    }
}

/// The card replaces the error message of a file that needs a decoder: its buttons are
/// drawn in a row, named, one Tab stop, and each sends its position back.
#[test]
fn the_card_offers_its_buttons() {
    let mut text = Text::new();
    with_state(STATES[1], |base| {
        let st = ViewerState {
            error: Some("Can\u{2019}t open A001.braw"),
            ..*base
        };
        let mut c = Chrome::new();
        c.set_size(1000.0, 700.0);
        c.set_accessibility(true);
        c.poke(1.0);
        assert!(c.set_decoder_card(Some(card())));
        assert!(!c.set_decoder_card(Some(card())), "unchanged");
        let mut dl = DrawList::new(1.0);
        c.draw(&mut dl, &mut text, &st, &Palette::dark(), 1.0);
        let rects: Vec<_> = (0..3u8)
            .map(|i| {
                c.hits
                    .iter()
                    .find(|(_, t)| *t == Target::DecoderCard(i))
                    .map(|h| h.0)
                    .expect("button drawn")
            })
            .collect();
        assert!(rects[0].right() <= rects[1].x && rects[1].right() <= rects[2].x);
        assert!(rects.iter().all(|r| r.y == rects[0].y), "one row");
        let t = c.accessibility_tree(&st, "x", 1.0);
        for name in ["Get it", "I\u{2019}ve installed it", "Locate\u{2026}"] {
            assert!(
                t.nodes
                    .iter()
                    .any(|(_, n)| n.role() == Role::Button && n.label() == Some(name)),
                "{name} is a named button"
            );
        }
        let stops = c.stops();
        assert!(stops.iter().any(|s| {
            s.targets.len() == 3
                && s.targets
                    .iter()
                    .all(|t| matches!(t, Target::DecoderCard(_)))
        }));
        let r = c.activate(Target::DecoderCard(2), &st);
        assert!(r.commands.iter().any(|c| matches!(
            c,
            Command::DecoderCard { id, button: 2 } if id == "braw"
        )));
        // No card: the plain message again, no buttons.
        assert!(c.set_decoder_card(None));
        dl.clear(1.0);
        c.draw(&mut dl, &mut text, &st, &Palette::dark(), 1.0);
        assert!(
            !c.hits
                .iter()
                .any(|(_, t)| matches!(t, Target::DecoderCard(_)))
        );
        assert!(c.activate(Target::DecoderCard(0), &st).commands.is_empty());
    });
}

/// In a narrow window the buttons stack.
#[test]
fn narrow_windows_stack_the_buttons() {
    let mut text = Text::new();
    with_state(STATES[1], |base| {
        let st = ViewerState {
            error: Some("x"),
            ..*base
        };
        let mut c = Chrome::new();
        c.set_size(260.0, 600.0);
        c.set_decoder_card(Some(card()));
        let mut dl = DrawList::new(1.0);
        c.draw(&mut dl, &mut text, &st, &Palette::dark(), 1.0);
        let ys: Vec<f32> = (0..3u8)
            .filter_map(|i| {
                c.hits
                    .iter()
                    .find(|(_, t)| *t == Target::DecoderCard(i))
                    .map(|h| h.0.y)
            })
            .collect();
        assert_eq!(ys.len(), 3);
        assert!(ys[0] < ys[1] && ys[1] < ys[2], "{ys:?}");
    });
}
