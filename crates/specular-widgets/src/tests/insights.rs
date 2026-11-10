use super::enabled_state::{STATES, with_state};
use crate::accesskit::Role;
use crate::ctx::Target;
use crate::{Chrome, Command, InsightItem, Insights, Modal};
use specular_gfx::{DrawList, Text};
use specular_theme::Palette;

fn chrome() -> Chrome {
    let mut c = Chrome::new();
    c.set_size(1000.0, 700.0);
    c.set_accessibility(true);
    c.poke(1.0);
    c
}

fn content() -> Insights {
    Insights {
        bottleneck: "Reading the files holds images up".into(),
        detail: "5 of the last 5 slow images waited for the storage".into(),
        now: vec![
            InsightItem {
                text: "Copy the folder to an internal drive".into(),
                gain: Some("About 2 to 8 times faster".into()),
                action: None,
            },
            InsightItem {
                text: "Turn on the display cache".into(),
                gain: None,
                action: Some(("Turn on".into(), "set:display-cache".into())),
            },
        ],
        dismiss: Some((
            "Don’t show again for storage".into(),
            "dismiss:storage".into(),
        )),
        switch: Some(("Turn insights off".into(), "off".into())),
        ..Insights::default()
    }
}

fn keys(r: crate::Response) -> Vec<String> {
    r.commands
        .into_iter()
        .filter_map(|c| match c {
            Command::Insight(k) => Some(k),
            _ => None,
        })
        .collect()
}

#[test]
fn the_indicator_is_announced_once_and_named() {
    let mut text = Text::new();
    with_state(STATES[3], |st| {
        let mut c = chrome();
        let ind = || Some(("Why is this slow?".to_owned(), "Insight ready".to_owned()));
        c.set_insight_indicator(ind());
        assert_eq!(
            c.announce.as_ref().map(|a| a.1.as_str()),
            Some("Insight ready")
        );
        c.announce = None;
        // Still up after the next report: not announced again.
        c.set_insight_indicator(ind());
        assert!(c.announce.is_none());
        let mut dl = DrawList::new(1.0);
        c.draw(&mut dl, &mut text, st, &Palette::dark(), 1.0);
        let t = c.accessibility_tree(st, "x", 1.0);
        assert!(
            t.nodes
                .iter()
                .any(|(_, n)| n.role() == Role::Button && n.label() == Some("Why is this slow?")),
            "the indicator is a named button"
        );
        assert_eq!(keys(c.activate(Target::Insight, st)), ["open"]);
        assert_eq!(keys(c.activate(Target::InsightHide, st)), ["hide"]);
        assert!(!c.insight_indicator_shown());
    });
}

#[test]
fn the_panel_buttons_send_their_keys() {
    let mut text = Text::new();
    with_state(STATES[3], |st| {
        let mut c = chrome();
        c.show_insights(content());
        assert!(c.insights_open());
        let mut dl = DrawList::new(1.0);
        c.draw(&mut dl, &mut text, st, &Palette::dark(), 1.0);
        let t = c.accessibility_tree(st, "x", 1.0);
        assert!(t.nodes.iter().any(|(_, n)| n.role() == Role::Dialog));
        // The first item has no action; the second turns the display cache on.
        assert!(keys(c.activate(Target::InsightAction(0), st)).is_empty());
        assert_eq!(
            keys(c.activate(Target::InsightAction(1), st)),
            ["set:display-cache"]
        );
        assert_eq!(keys(c.activate(Target::InsightSwitch, st)), ["off"]);
        assert!(c.insights_open());
        // "Don't show again" closes the panel.
        assert_eq!(
            keys(c.activate(Target::InsightDismiss, st)),
            ["dismiss:storage"]
        );
        assert_ne!(c.modal, Some(Modal::Insights));
    });
}

fn hit(c: &Chrome, t: &Target) -> Option<specular_gfx::Rect> {
    c.hits.iter().rev().find(|(_, h)| h == t).map(|h| h.0)
}

#[test]
fn in_the_gallery_the_indicator_sits_above_its_bottom_bar() {
    let mut text = Text::new();
    with_state(STATES[3], |st| {
        let mut c = chrome();
        c.set_insight_indicator(Some(("Why is this slow?".into(), "Insight".into())));
        let mut dl = DrawList::new(1.0);
        c.draw(&mut dl, &mut text, st, &Palette::dark(), 1.0);
        let viewer = hit(&c, &Target::Insight).expect("drawn in the viewer");
        // The gallery has no toolbar: the indicator stays, above the gallery's own bar.
        c.gallery = true;
        c.gallery_bar = 44.0;
        dl.clear(1.0);
        c.draw(&mut dl, &mut text, st, &Palette::dark(), 1.1);
        let r = hit(&c, &Target::Insight).expect("drawn in the gallery");
        assert!(r.bottom() <= 700.0 - 44.0, "{r:?}");
        assert!(r.bottom() > viewer.bottom(), "{r:?} against {viewer:?}");
        assert!(hit(&c, &Target::InsightHide).is_some());
        assert_eq!(keys(c.activate(Target::Insight, st)), ["open"]);
        let t = c.accessibility_tree(st, "x", 1.0);
        assert!(
            t.nodes
                .iter()
                .any(|(_, n)| n.label() == Some("Why is this slow?"))
        );
    });
}

#[test]
fn hidden_insights_can_be_shown_again_from_the_panel() {
    let mut text = Text::new();
    with_state(STATES[3], |st| {
        let mut c = chrome();
        c.show_insights(Insights {
            restore: Some(("Show hidden insights again".into(), "unhide".into())),
            ..content()
        });
        let mut dl = DrawList::new(1.0);
        c.draw(&mut dl, &mut text, st, &Palette::dark(), 1.0);
        // All three footer buttons are drawn, inside the sheet, without overlapping.
        let foot: Vec<_> = [
            Target::InsightDismiss,
            Target::InsightRestore,
            Target::InsightSwitch,
        ]
        .iter()
        .map(|t| hit(&c, t).unwrap_or_else(|| panic!("{t:?}")))
        .collect();
        for (i, a) in foot.iter().enumerate() {
            assert!(a.x >= 0.0 && a.right() <= 1000.0, "{a:?}");
            for b in &foot[i + 1..] {
                let apart =
                    a.right() <= b.x || b.right() <= a.x || a.bottom() <= b.y || b.bottom() <= a.y;
                assert!(apart, "{a:?} overlaps {b:?}");
            }
        }
        let t = c.accessibility_tree(st, "x", 1.0);
        assert!(
            t.nodes.iter().any(|(_, n)| n.role() == Role::Button
                && n.label() == Some("Show hidden insights again"))
        );
        assert_eq!(keys(c.activate(Target::InsightRestore, st)), ["unhide"]);
        assert!(c.insights_open());
    });
}
