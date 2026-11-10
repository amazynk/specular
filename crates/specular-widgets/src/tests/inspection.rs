use super::enabled_state::with_state;
use crate::accesskit::Role;
use crate::inspect::{pixel_rows, source_row};
use crate::{Chrome, Overlays, PixelInfo, SourceState, ViewerState};
use specular_gfx::{DrawList, Text};
use specular_input::{Action, ActionState, ImageState, KeyCombo, Keymap};
use specular_inspect::{DisplayValue, Histogram, SourceValue};
use specular_theme::{Palette, metrics};

const READY: ActionState = ActionState {
    comparing: false,
    compare_images: 0,
    volume: false,
    mpr: false,
    image: ImageState::Ready,
    count: 3,
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
};

fn chrome() -> Chrome {
    let mut c = Chrome::new();
    c.set_size(1000.0, 700.0);
    c.set_accessibility(true);
    c.poke(1.0);
    c
}

fn draw(c: &mut Chrome, text: &mut Text, st: &ViewerState) {
    let mut dl = DrawList::new(1.0);
    c.draw(&mut dl, text, st, &Palette::dark(), 1.0);
}

fn pixel(display: Option<DisplayValue>) -> PixelInfo {
    PixelInfo {
        x: 1203,
        y: 77,
        source: SourceState::Value(SourceValue::YCbCr {
            ycc: [150, 44, 201],
            rgb: [255, 128, 0],
        }),
        display,
    }
}

fn orange(exact: bool) -> DisplayValue {
    DisplayValue::from_premultiplied([1.0, 0.2158605, 0.0, 1.0], if exact { 0 } else { 2 })
}

#[test]
fn actions_toggle_the_tools() {
    let mut c = Chrome::new();
    let t = |c: &Chrome| c.inspect.tools;
    assert!(c.handle_action(Action::ToggleClipping));
    assert!(t(&c).clipping && !t(&c).peaking);
    assert!(c.handle_action(Action::ToggleFocusPeaking));
    assert!(c.handle_action(Action::ToggleHistogram));
    assert!(c.handle_action(Action::TogglePixelInspector));
    assert!(t(&c).peaking && t(&c).histogram && t(&c).pixel);
    assert!(c.handle_action(Action::ToggleClipping));
    assert!(!t(&c).clipping);
    // Disabled without an image, like the other view tools.
    let none = ActionState {
        comparing: false,
        compare_images: 0,
        volume: false,
        mpr: false,
        image: ImageState::None,
        count: 0,
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
    };
    assert!(Action::ToggleHistogram.disabled(&none).is_some());
    assert!(Action::ToggleHistogram.disabled(&READY).is_none());
}

#[test]
fn default_keys_are_bound_and_distinct() {
    let km = Keymap::viewer_default();
    for (key, a) in [
        ("j", Action::ToggleClipping),
        ("Shift+f", Action::ToggleFocusPeaking),
        ("Shift+g", Action::ToggleHistogram),
        ("Shift+i", Action::TogglePixelInspector),
    ] {
        let combo = KeyCombo::parse(key).unwrap();
        assert_eq!(km.lookup(&combo), Some(a), "{key}");
    }
    // `g` is Viewer ↔ Gallery (DESIGN §10); the pixel grid moved to Alt+G.
    assert_eq!(
        km.lookup(&KeyCombo::parse("g").unwrap()),
        Some(Action::ToggleGallery)
    );
    assert_eq!(
        km.lookup(&KeyCombo::parse("Alt+g").unwrap()),
        Some(Action::TogglePixelGrid)
    );
    // The lower-case keys keep their own actions.
    assert_eq!(
        km.lookup(&KeyCombo::parse("i").unwrap()),
        Some(Action::ToggleInfo)
    );
    assert_eq!(
        km.lookup(&KeyCombo::parse("f").unwrap()),
        Some(Action::ZoomFit)
    );
}

/// Moving a held loupe to the bottom edge or the sides reveals neither the toolbar nor
/// the edge arrows; without the loupe the same move reveals both.
#[test]
fn held_loupe_does_not_reveal_the_toolbar() {
    let mut text = Text::new();
    with_state(READY, |st| {
        let loupe = ViewerState {
            overlays: Overlays {
                loupe: Some(((500.0, 690.0), 80.0)),
                ..Overlays::default()
            },
            ..*st
        };
        for (state, tool) in [(st, false), (&loupe, true)] {
            let mut c = Chrome::new();
            c.set_size(1000.0, 700.0);
            c.reduce_motion = true;
            // Long idle: only the pointer position can reveal anything.
            c.poke(-100.0);
            c.pointer = Some((500.0, 690.0));
            draw(&mut c, &mut text, state);
            assert_eq!(
                c.toolbar_alpha,
                if tool { 0.0 } else { 1.0 },
                "bottom, tool {tool}"
            );
            c.pointer = Some((20.0, 350.0));
            draw(&mut c, &mut text, state);
            assert_eq!(
                c.edge_alpha[0],
                if tool { 0.0 } else { 1.0 },
                "edge, tool {tool}"
            );
        }
    });
}

/// Develop's panel takes the end side of the window: the next arrow moves in beside it, and
/// shows with the pointer near it over the image, not over the panel.
#[test]
fn next_arrow_sits_beside_a_side_panel() {
    let mut text = Text::new();
    with_state(READY, |st| {
        let mut c = Chrome::new();
        c.set_size(1000.0, 700.0);
        c.set_panel_inset(300.0);
        c.reduce_motion = true;
        c.poke(-100.0);
        c.pointer = Some((850.0, 350.0));
        draw(&mut c, &mut text, st);
        assert_eq!(c.edge_alpha[1], 0.0, "over the panel");
        c.pointer = Some((650.0, 350.0));
        draw(&mut c, &mut text, st);
        assert_eq!(c.edge_alpha[1], 1.0, "over the image");
        let r = c
            .hits
            .iter()
            .find(|(_, t)| *t == crate::ctx::Target::EdgeNext)
            .map(|(r, _)| *r)
            .expect("next arrow");
        assert!(r.right() <= 700.0 - 16.0, "{r:?}");
    });
}

#[test]
fn loupe_sits_in_the_inspector_above_the_toolbar() {
    let mut text = Text::new();
    with_state(READY, |st| {
        let mut c = chrome();
        draw(&mut c, &mut text, st);
        assert_eq!(c.pixel_loupe(), None, "tools off");
        c.handle_action(Action::TogglePixelInspector);
        draw(&mut c, &mut text, st);
        assert_eq!(c.pixel_loupe(), None, "pointer off the image: no loupe");
        c.inspect.pixel = Some(pixel(Some(orange(true))));
        draw(&mut c, &mut text, st);
        let l = c.pixel_loupe().expect("loupe");
        assert_eq!((l.w, l.h), (crate::PIXEL_LOUPE, crate::PIXEL_LOUPE));
        // Start side (left in English), clear of the toolbar.
        assert!(l.x < 500.0);
        assert!(l.bottom() < 700.0 - metrics::TOOLBAR_HEIGHT - metrics::TOOLBAR_MARGIN);
        // The histogram stacks above without moving it.
        c.handle_action(Action::ToggleHistogram);
        c.inspect.histogram = Some(Box::default());
        draw(&mut c, &mut text, st);
        assert_eq!(c.pixel_loupe(), Some(l));
        let mut h = Histogram::new();
        for i in 0..1000 {
            let v = i as f32 / 999.0;
            h.add_linear([v, v * 0.5, 1.0 - v], 1, 254);
        }
        c.inspect.histogram = Some(Box::new(h));
        draw(&mut c, &mut text, st);
        assert_eq!(c.pixel_loupe(), Some(l));
        // A science image's plane sliders in the same corner: the panels stack above
        // them (they overlapped before), and come back down without them.
        let slider_top = 560.0;
        c.inspect.floor = Some(slider_top);
        draw(&mut c, &mut text, st);
        let raised = c.pixel_loupe().expect("loupe");
        assert!(raised.bottom() < slider_top - metrics::GAP, "{raised:?}");
        assert!(raised.y < l.y);
        c.inspect.floor = Some(690.0);
        draw(&mut c, &mut text, st);
        assert_eq!(c.pixel_loupe(), Some(l), "a low floor changes nothing");
        c.inspect.floor = None;
        draw(&mut c, &mut text, st);
        assert_eq!(c.pixel_loupe(), Some(l));
    });
}

/// Colour-bar labels below zero use the minus sign, not a hyphen.
#[test]
fn colour_bar_labels_use_the_minus_sign() {
    let l = specular_inspect::Legend {
        map: specular_inspect::ColourMap::Viridis,
        lo: -40.0,
        hi: 40.0,
        unit: String::new(),
    };
    let ticks = crate::inspect::colour_bar_ticks(&l, 5);
    let negative: Vec<_> = ticks.iter().filter(|(v, _)| *v < 0.0).collect();
    assert!(!negative.is_empty(), "{ticks:?}");
    for (_, s) in &negative {
        assert!(s.starts_with('\u{2212}') && !s.contains('-'), "{s}");
    }
}

#[test]
fn inspector_rows() {
    let rows = pixel_rows(&pixel(Some(orange(true))));
    let values: Vec<&str> = rows.iter().map(|(_, v)| v.as_str()).collect();
    assert_eq!(values[0], "1203, 77");
    assert_eq!(values[1], "150 44 201");
    assert_eq!(values[2], "255 128 0");
    assert_eq!(values[3], "#FF8000");
    assert!(
        values[4].starts_with("1.0000 0.2159 0.0000"),
        "{}",
        values[4]
    );
    assert_eq!(rows.len(), 5);
    // Not yet probed: position and source only; unavailable source: no row.
    assert_eq!(pixel_rows(&pixel(None)).len(), 2);
    assert_eq!(source_row(&SourceState::Unavailable), None);
    assert!(source_row(&SourceState::Pending).is_some());
    // Translucent pixels show alpha.
    let half = DisplayValue::from_premultiplied([0.5, 0.5, 0.5, 0.5], 0);
    let rows = pixel_rows(&PixelInfo {
        display: Some(half),
        ..pixel(None)
    });
    assert_eq!(rows[2].1, "255 255 255 128");
}

#[test]
fn sixteen_bit_source_rows() {
    let row = |v| source_row(&SourceState::Value(v)).expect("row");
    let (gl, gv) = row(SourceValue::Gray16(65535));
    assert_eq!(gv, "65535");
    let (gal, gav) = row(SourceValue::GrayAlpha16([40000, 1]));
    assert_eq!(gav, "40000 1");
    // Opaque RGB drops the alpha; translucent keeps it.
    let (rl, rv) = row(SourceValue::Rgba16([1, 300, 65000, 65535]));
    assert_eq!(rv, "1 300 65000");
    let (ral, rav) = row(SourceValue::Rgba16([1, 300, 65000, 32768]));
    assert_eq!(rav, "1 300 65000 32768");
    // Each names its sample model, apart from the 8-bit labels.
    let (g8, _) = row(SourceValue::Gray(255));
    let labels = [&g8, &gl, &gal, &rl, &ral];
    for (i, a) in labels.iter().enumerate() {
        assert!(!a.is_empty());
        for b in &labels[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

#[test]
fn panels_are_announced() {
    let mut text = Text::new();
    with_state(READY, |st| {
        let mut c = chrome();
        c.handle_action(Action::ToggleHistogram);
        c.handle_action(Action::TogglePixelInspector);
        let mut h = Histogram::new();
        h.add_linear([1.0, 1.0, 1.0], 1, 254);
        h.add_linear([0.2, 0.2, 0.2], 1, 254);
        c.inspect.histogram = Some(Box::new(h));
        c.inspect.pixel = Some(pixel(Some(orange(false))));
        draw(&mut c, &mut text, st);
        let t = c.accessibility_tree(st, "x", 1.0);
        let group = |name: &str| {
            t.nodes
                .iter()
                .find(|(_, n)| n.role() == Role::Group && n.label() == Some(name))
                .map(|(_, n)| n.children().to_vec())
                .unwrap_or_else(|| panic!("no {name} group"))
        };
        let value_of = |id| {
            t.nodes
                .iter()
                .find(|(i, _)| *i == id)
                .and_then(|(_, n)| n.value().map(str::to_owned))
                .unwrap_or_default()
        };
        let hist = group(&specular_i18n::t!("histogram-title"));
        let s = value_of(hist[0]);
        assert!(s.contains("50"), "half the pixels clip: {s}");
        let px = group(&specular_i18n::t!("inspector-title"));
        let s = value_of(px[0]);
        assert!(s.contains("1203") && s.contains("255 128 0"), "{s}");
        assert_eq!(px.len(), 6);
    });
}
