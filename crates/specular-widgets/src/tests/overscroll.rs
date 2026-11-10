use super::enabled_state::{STATES, with_metadata};
use crate::scrollbar::Area;
use crate::{Chrome, Input, Modal, ViewerState};
use specular_gfx::{DrawList, Text};
use specular_metadata::{Field, FieldGroup, MetadataReport};
use specular_theme::Palette;

fn report() -> MetadataReport {
    let fields = |g: usize| {
        (0..12)
            .map(|i| Field {
                tag: format!("Tag{g}x{i}"),
                raw: format!("{i}"),
                display: format!("value {i}"),
                empty: false,
                full: None,
            })
            .collect()
    };
    MetadataReport {
        all_fields: (0..8)
            .map(|g| FieldGroup {
                title: format!("Group {g}"),
                fields: fields(g),
            })
            .collect(),
        language: "en".into(),
        ..Default::default()
    }
}

struct Frames {
    c: Chrome,
    dl: DrawList,
    text: Text,
    pal: Palette,
    t: f64,
}

impl Frames {
    /// Draws a frame; returns where the area's text was drawn (every glyph clipped to the
    /// scrolled view). Row hover highlights may change with the pointer; text must not.
    fn frame(&mut self, st: &ViewerState, area: Area) -> Vec<[f32; 4]> {
        self.t += 0.016;
        self.dl.clear(1.0);
        self.c
            .draw(&mut self.dl, &mut self.text, st, &self.pal, self.t);
        let v = self.c.bars.view(area);
        let inside = |c: &[f32; 4]| {
            c[0] >= v.x - 0.5
                && c[1] >= v.y - 0.5
                && c[2] <= v.right() + 0.5
                && c[3] <= v.bottom() + 0.5
        };
        self.dl
            .prims
            .iter()
            .filter(|p| p.params[2] == specular_gfx::draw::kind::GLYPH && inside(&p.clip))
            .map(|p| p.rect)
            .collect()
    }
    fn input(&mut self, ev: Input, st: &ViewerState) {
        self.c.input(&ev, st, self.t);
    }
}

fn check(area: Area, open: impl Fn(&mut Chrome), st: &ViewerState, text: &mut Option<Text>) {
    let mut f = Frames {
        c: Chrome::new(),
        dl: DrawList::new(1.0),
        text: text.take().unwrap(),
        pal: Palette::dark(),
        t: 1.0,
    };
    f.c.set_size(640.0, 300.0);
    open(&mut f.c);
    // Let the opening animation finish.
    for _ in 0..60 {
        f.frame(st, area);
    }
    let max =
        f.c.bars
            .geometry(area)
            .unwrap_or_else(|| panic!("{area:?} does not overflow"))
            .max;
    assert!(max > 0.0);
    // A point inside the area: just inside its scrollbar.
    let track = f.c.bars.geometry(area).unwrap().track;
    let p = (track.x - 30.0, track.y + track.h * 0.5);
    for (dy, want) in [(40.0, 0.0), (-40.0, max), (40.0, 0.0)] {
        f.input(Input::PointerMove { x: p.0, y: p.1 }, st);
        for _ in 0..4 {
            f.input(
                Input::Wheel {
                    x: p.0,
                    y: p.1,
                    dx: 0.0,
                    dy,
                },
                st,
            );
            let s = *f.c.scroll_mut(area);
            assert!(
                (0.0..=max).contains(&s),
                "{area:?}: {s} clamped at input time"
            );
        }
        assert_eq!(*f.c.scroll_mut(area), want, "{area:?}: at the end");
        let first = f.frame(st, area);
        assert!(!first.is_empty(), "{area:?}: content found");
        assert_eq!(*f.c.scroll_mut(area), want, "{area:?}: drawn at the end");
        for i in 1..=3 {
            f.input(
                Input::PointerMove {
                    x: p.0 + i as f32,
                    y: p.1 - i as f32,
                },
                st,
            );
            let again = f.frame(st, area);
            assert_eq!(*f.c.scroll_mut(area), want, "{area:?}: pointer move {i}");
            if again != first {
                let a: Vec<_> = first
                    .iter()
                    .filter(|h| !again.contains(h))
                    .take(4)
                    .collect();
                let b: Vec<_> = again
                    .iter()
                    .filter(|h| !first.contains(h))
                    .take(4)
                    .collect();
                panic!("{area:?}: content moved after a pointer move (dy {dy}): {a:?} -> {b:?}");
            }
        }
    }
    *text = Some(f.text);
}

#[test]
fn overscrolled_areas_never_settle_back() {
    let meta = report();
    with_metadata(STATES[3], Some(&meta), |st| {
        let mut text = Some(Text::new());
        check(Area::Info, |c| c.set_info(true, true), st, &mut text);
        check(
            Area::CheatSheet,
            |c| c.open_modal(Modal::CheatSheet),
            st,
            &mut text,
        );
        check(
            Area::Palette,
            |c| c.open_modal(Modal::Palette),
            st,
            &mut text,
        );
        check(
            Area::Settings,
            |c| c.open_modal(Modal::Settings),
            st,
            &mut text,
        );
        check(
            Area::Language,
            |c| c.open_modal(Modal::Language),
            st,
            &mut text,
        );
        check(Area::About, |c| c.open_modal(Modal::About), st, &mut text);
        let mut long = crate::report::sample();
        long.sections[0]
            .rows
            .extend((0..60).map(|i| crate::ReportRow::new(format!("Row {i}"), "Value")));
        check(Area::Report, |c| c.show_report(long.clone()), st, &mut text);
        let long = crate::Insights {
            bottleneck: "Reading the files holds images up".into(),
            now: (0..30)
                .map(|i| crate::InsightItem {
                    text: format!("Something to do now, number {i}"),
                    gain: Some("About 2 to 8 times faster".into()),
                    action: None,
                })
                .collect(),
            ..crate::Insights::default()
        };
        check(
            Area::Insights,
            |c| c.show_insights(long.clone()),
            st,
            &mut text,
        );
    });
}
