//! The sound page from the keyboard and a screen reader: Tab from the picture enters the
//! choices, then the synced lyrics; arrows, Home and End move; Enter or Space acts; Esc
//! leaves. The tree names every control, has no orphans and follows the keyboard.

use specular_video::lyrics::{From, Line, Lyrics};

use super::*;

fn line(at: f64, text: &str) -> Line {
    Line {
        at: Some(at),
        text: text.into(),
        words: Vec::new(),
    }
}

/// A page as drawn: three choices in a row, synced lyrics with an empty line.
fn page() -> SoundPage {
    let chips = [
        Chip::Visual(Visual::Waveform),
        Chip::Visual(Visual::Spectrum),
        Chip::Lyrics,
    ]
    .iter()
    .enumerate()
    .map(|(i, c)| {
        (
            *c,
            Rect::new(10.0 + 90.0 * i as f32, 8.0, 80.0, 24.0),
            i == 0,
        )
    })
    .collect();
    let lyrics = Lyrics {
        lines: vec![
            line(0.0, "One"),
            line(2.0, ""),
            line(4.0, "Two"),
            line(6.0, "Three"),
        ],
        synced: true,
        from: From::Sidecar,
    };
    SoundPage {
        drawn: Some(Drawn {
            area: Rect::new(0.0, 0.0, 900.0, 560.0),
            layout: PageLayout {
                lyrics: Some(Area::new(590.0, 90.0, 300.0, 400.0)),
                ..Default::default()
            },
            chips,
            lines: [0usize, 2, 3]
                .iter()
                .enumerate()
                .map(|(k, i)| (*i, Rect::new(600.0, 100.0 + 30.0 * k as f32, 280.0, 24.0)))
                .collect(),
            ..Default::default()
        }),
        lyrics: Some(Arc::new(lyrics)),
        show_lyrics: true,
        ..Default::default()
    }
}

/// The page laid out right to left: the row of choices from the right.
fn mirrored(mut p: SoundPage, rtl: bool) -> SoundPage {
    if let Some(d) = p.drawn.as_mut().filter(|_| rtl) {
        for (_, r, _) in &mut d.chips {
            *r = r.mirror_x(900.0);
        }
    }
    p
}

fn named(k: NamedKey) -> Key {
    Key::Named(k)
}

#[test]
fn tab_walks_the_choices_and_the_lyrics() {
    let mut p = page();
    let tab = named(NamedKey::Tab);
    // Not entered while something else has the keyboard.
    assert_eq!(p.key(&tab, false, false, false), None);
    assert_eq!(p.key(&tab, false, false, true), Some(KeyOut::Moved));
    assert_eq!(p.focus, Some(Focus::Chip(0)));
    // Arrows along the row, stopping at the ends; mirrored right to left.
    let right = named(NamedKey::ArrowRight);
    let left = named(NamedKey::ArrowLeft);
    assert_eq!(p.key(&right, false, false, true), Some(KeyOut::Moved));
    assert_eq!(p.focus, Some(Focus::Chip(1)));
    p.key(&named(NamedKey::End), false, false, true);
    assert_eq!(p.focus, Some(Focus::Chip(2)));
    p.key(&right, false, false, true);
    assert_eq!(p.focus, Some(Focus::Chip(2)));
    p.key(&left, false, true, true);
    assert_eq!(p.focus, Some(Focus::Chip(2)), "left is on in RTL");
    p.key(&right, false, true, true);
    assert_eq!(p.focus, Some(Focus::Chip(1)));
    assert_eq!(p.focus_name(), Some(t!("sound-visual-spectrum")));
    // Space acts on the choice.
    assert_eq!(
        p.key(&named(NamedKey::Space), false, false, true),
        Some(KeyOut::Activate(Focus::Chip(1)))
    );
    // Tab on to the lyrics, at the sung line; the empty line is skipped.
    p.key(&tab, false, false, true);
    assert_eq!(p.focus, Some(Focus::Line(0)));
    let down = named(NamedKey::ArrowDown);
    p.key(&down, false, false, true);
    assert_eq!(p.focus, Some(Focus::Line(2)));
    assert_eq!(p.focus_name().as_deref(), Some("Two"));
    p.key(&down, false, false, true);
    p.key(&down, false, false, true);
    assert_eq!(p.focus, Some(Focus::Line(3)));
    assert_eq!(
        p.key(&named(NamedKey::Enter), false, false, true),
        Some(KeyOut::Activate(Focus::Line(3)))
    );
    // Shift+Tab back to the choices; Tab twice leaves, the key going on to the chrome.
    p.key(&tab, true, false, true);
    assert_eq!(p.focus, Some(Focus::Chip(0)));
    p.key(&tab, false, false, true);
    assert_eq!(
        p.key(&tab, false, false, true),
        Some(KeyOut::Left { taken: false })
    );
    assert_eq!(p.focus, None);
    // Esc leaves too, and keeps the key.
    p.key(&tab, false, false, true);
    assert_eq!(
        p.key(&named(NamedKey::Escape), false, false, true),
        Some(KeyOut::Left { taken: true })
    );
    assert_eq!(p.focus, None);
}

#[test]
fn without_synced_lyrics_tab_leaves_from_the_choices() {
    let mut p = page();
    p.show_lyrics = false;
    let tab = named(NamedKey::Tab);
    p.key(&tab, false, false, true);
    assert_eq!(
        p.key(&tab, false, false, true),
        Some(KeyOut::Left { taken: false })
    );
}

/// The tree: every control named and focusable, the keyboard's place is the tree's focus,
/// "Reading the sound…" is a status, and the audit passes both ways round.
#[test]
fn the_tree_follows_the_keyboard() {
    use crate::app::a11y_audit::{expect_clean, window};
    for rtl in [false, true] {
        let mut p = mirrored(page(), rtl);
        p.focus = Some(Focus::Line(2));
        let mut tree = window((900.0, 560.0), 1.0);
        a11y(&mut tree, &p, 1.0);
        assert_eq!(tree.focus, accesskit::NodeId(A11Y_LYRICS + 3));
        expect_clean("sound-keyboard", &tree, rtl);
        for (_, n) in &tree.nodes {
            if n.role() == accesskit::Role::Button {
                assert!(n.supports_action(accesskit::Action::Focus), "{n:?}");
            }
        }
        // While reading.
        let mut p = mirrored(page(), rtl);
        if let Some(d) = p.drawn.as_mut() {
            d.status = Some((
                t!("sound-page-reading"),
                Rect::new(300.0, 300.0, 200.0, 20.0),
            ));
        }
        p.focus = Some(Focus::Chip(1));
        let mut tree = window((900.0, 560.0), 1.0);
        a11y(&mut tree, &p, 1.0);
        assert_eq!(tree.focus, accesskit::NodeId(A11Y_CHIP + 1));
        let status = tree
            .nodes
            .iter()
            .find(|(id, _)| *id == accesskit::NodeId(A11Y_STATUS))
            .map(|(_, n)| n)
            .expect("a status node");
        assert_eq!(status.role(), accesskit::Role::Status);
        assert_eq!(status.label(), Some(t!("sound-page-reading").as_str()));
        expect_clean("sound-reading", &tree, rtl);
    }
}
