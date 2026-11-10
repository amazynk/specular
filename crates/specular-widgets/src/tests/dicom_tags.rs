use super::enabled_state::with_state;
use crate::accesskit::{Node, Role, TreeUpdate};
use crate::ctx::Target;
use crate::{Button, Chrome, DicomTags, Input, Modal, TagLine, ViewerState};
use specular_gfx::{DrawList, Text};
use specular_input::{ActionState, ImageState, Key, KeyCombo, Mods, NamedKey};
use specular_theme::Palette;

fn with_dicom<R>(f: impl FnOnce(&ViewerState) -> R) -> R {
    with_state(
        ActionState {
            image: ImageState::Ready,
            count: 1,
            dicom: true,
            ..ActionState::default()
        },
        f,
    )
}

fn line(depth: u16, tag: &str, keyword: &str, value: &str) -> TagLine {
    TagLine {
        depth,
        tag: tag.into(),
        keyword: keyword.into(),
        vr: "LO".into(),
        length: value.len().to_string(),
        value: value.into(),
        descendants: 0,
        identifying: false,
        jump: false,
    }
}

/// Modality, a hidden patient name, a sequence of one item holding a description, and
/// Rows. No real patient's data.
fn sheet() -> DicomTags {
    let mut name = line(0, "(0010,0010)", "PatientName", "SECRET^VALUE");
    name.identifying = true;
    let mut seq = line(0, "(0040,0275)", "RequestAttributesSequence", "1 item");
    seq.descendants = 2;
    let mut item = line(1, "", "Item 1", "");
    item.descendants = 1;
    DicomTags {
        title: "x.dcm".into(),
        rows: vec![
            line(0, "(0008,0060)", "Modality", "CT"),
            name,
            seq,
            item,
            line(
                2,
                "(0040,0007)",
                "ScheduledProcedureStepDescription",
                "THORAX",
            ),
            line(0, "(0028,0010)", "Rows", "512"),
        ],
        status: None,
        note: None,
        heads: None,
    }
}

fn chrome() -> Chrome {
    let mut c = Chrome::new();
    c.set_size(900.0, 700.0);
    c.poke(1.0);
    c
}

fn draw(c: &mut Chrome, text: &mut Text, st: &ViewerState) {
    let mut dl = DrawList::new(1.0);
    for i in 0..3 {
        dl.clear(1.0);
        c.draw(&mut dl, text, st, &Palette::dark(), 1.0 + i as f64 * 0.016);
    }
}
fn press(c: &mut Chrome, st: &ViewerState, k: NamedKey) {
    let r = c.input(
        &Input::Key {
            combo: KeyCombo::new(Key::Named(k), Mods::NONE),
            repeat: false,
        },
        st,
        2.0,
    );
    assert!(r.consumed, "{k:?}");
}

fn typed(c: &mut Chrome, st: &ViewerState, s: &str) {
    c.input(&Input::Text(s.into()), st, 2.0);
}

fn shown(c: &Chrome) -> Vec<String> {
    let s = &c.dicom_tags;
    s.visible
        .iter()
        .map(|&i| s.sheet.rows[i].keyword.clone())
        .collect()
}

fn tree(c: &mut Chrome, text: &mut Text, st: &ViewerState) -> TreeUpdate {
    draw(c, text, st);
    c.accessibility_tree(st, "x.dcm", 1.0)
}

fn find(t: &TreeUpdate, role: Role) -> Vec<&Node> {
    t.nodes
        .iter()
        .filter(|(_, n)| n.role() == role)
        .map(|(_, n)| n)
        .collect()
}

/// Sequences open and close from the keyboard; the filter keeps the ancestors of what
/// matches; Escape closes the panel and drops the rows.
#[test]
fn browse_filter_and_close() {
    let mut text = Text::new();
    with_dicom(|st| {
        let mut c = chrome();
        c.show_dicom_tags(sheet());
        draw(&mut c, &mut text, st);
        assert_eq!(
            shown(&c),
            [
                "Modality",
                "PatientName",
                "RequestAttributesSequence",
                "Rows"
            ]
        );
        // Onto the sequence, open it, into its item, open that.
        press(&mut c, st, NamedKey::ArrowDown);
        press(&mut c, st, NamedKey::ArrowDown);
        press(&mut c, st, NamedKey::ArrowRight);
        assert_eq!(shown(&c).len(), 5);
        press(&mut c, st, NamedKey::ArrowDown);
        press(&mut c, st, NamedKey::Enter);
        assert_eq!(shown(&c).len(), 6);
        // Left on the item closes it; again goes to the sequence; again closes that.
        press(&mut c, st, NamedKey::ArrowLeft);
        press(&mut c, st, NamedKey::ArrowLeft);
        assert_eq!(c.dicom_tags.selected, 2);
        press(&mut c, st, NamedKey::ArrowLeft);
        assert_eq!(shown(&c).len(), 4);

        // A match inside the sequence brings its ancestors.
        typed(&mut c, st, "procedure");
        draw(&mut c, &mut text, st);
        assert_eq!(
            shown(&c),
            [
                "RequestAttributesSequence",
                "Item 1",
                "ScheduledProcedureStepDescription"
            ]
        );
        for _ in 0.."procedure".len() {
            press(&mut c, st, NamedKey::Backspace);
        }
        // A hidden value is not matched; its keyword is.
        typed(&mut c, st, "secret");
        draw(&mut c, &mut text, st);
        assert!(shown(&c).is_empty());
        c.toggle_patient_data();
        draw(&mut c, &mut text, st);
        assert_eq!(shown(&c), ["PatientName"]);

        // Escape clears the filter first, then closes.
        press(&mut c, st, NamedKey::Escape);
        assert_eq!(c.modal, Some(Modal::DicomTags));
        assert_eq!(c.dicom_tags.query, "");
        press(&mut c, st, NamedKey::Escape);
        assert_eq!(c.modal, None);
        assert!(c.dicom_tags.sheet.rows.is_empty());
        assert!(!c.dicom_tags.patient);
    });
}

/// A structured report in the tag browser: its own column heads, a jump button on the
/// rows that point at an image (clicked or Enter), announced as a button.
#[test]
fn report_rows_jump_to_their_image() {
    use crate::Command;
    let mut text = Text::new();
    with_dicom(|st| {
        let mut c = chrome();
        let mut s = sheet();
        s.heads = Some((
            [
                "Item".into(),
                "Concept".into(),
                "Type".into(),
                "Relationship".into(),
                "Value".into(),
            ],
            "Report (Comprehensive SR)".into(),
        ));
        s.rows[3].jump = false;
        s.rows[5].jump = true;
        let all = s.rows.len();
        c.show_dicom_tags(s);
        draw(&mut c, &mut text, st);
        // A report opens fully expanded: its row 5 is the sixth shown. Onto it, Enter
        // asks for its jump.
        assert_eq!(c.dicom_tags.visible.len(), all);
        for _ in 0..5 {
            press(&mut c, st, NamedKey::ArrowDown);
        }
        let r = c.input(
            &Input::Key {
                combo: KeyCombo::new(Key::Named(NamedKey::Enter), Mods::NONE),
                repeat: false,
            },
            st,
            2.0,
        );
        assert!(
            r.commands.contains(&Command::TagJump(5)),
            "{:?}",
            r.commands
        );
        // The button is drawn and clickable.
        let t = tree(&mut c, &mut text, st);
        assert!(
            find(&t, Role::Button)
                .iter()
                .any(|n| n.label() == Some("Show the image"))
        );
        let r = hit_rect(&c, &Target::TagJump(5)).expect("jump button drawn");
        let p = (r.x + r.w * 0.5, r.y + r.h * 0.5);
        let mut cmds = Vec::new();
        for down in [true, false] {
            let r = c.input(
                &Input::PointerButton {
                    x: p.0,
                    y: p.1,
                    down,
                    button: Button::Primary,
                },
                st,
                2.0,
            );
            cmds.extend(r.commands);
        }
        assert_eq!(cmds, [Command::TagJump(5)]);
        // A row without a jump has no button.
        assert!(hit_rect(&c, &Target::TagJump(0)).is_none());
    });
}

fn hit_rect(c: &Chrome, t: &Target) -> Option<specular_gfx::Rect> {
    c.hits.iter().rev().find(|(_, h)| h == t).map(|h| h.0)
}

fn plot_sheet(adjustable: bool) -> crate::PlotSheet {
    use crate::{CaliperKind, PlotCell, PlotGrid, PlotSheet, PlotTrace};
    // One 200 × 100 cell at 25 mm/s: 8 s across, a beat every second.
    let points = (0..=2000)
        .map(|i| {
            let x = i as f32 * 0.1;
            [x, if i % 250 == 0 { 20.0 } else { 50.0 }]
        })
        .collect();
    PlotSheet {
        title: "ECG".into(),
        page: [200.0, 100.0],
        grid: PlotGrid::Paper {
            minor: 1.0,
            major: 5.0,
        },
        cells: vec![PlotCell {
            rect: [0.0, 0.0, 200.0, 100.0],
            label: "II".into(),
            traces: vec![PlotTrace {
                points,
                srgb: [0, 0, 0],
                name: "II".into(),
            }],
            x_value: (0.0, 1.0 / 25.0),
            y_value: (0.0, 0.1),
            x_unit: "s".into(),
            y_unit: "mV".into(),
            ..PlotCell::default()
        }],
        caliper: CaliperKind::Time,
        subtitle: "25 mm/s · 10 mm/mV".into(),
        paper: true,
        adjustable,
        table: vec![
            vec!["Lead".into(), "Max".into()],
            vec!["II".into(), "3".into()],
        ],
        ..PlotSheet::default()
    }
}

/// The plot sheet: a drag across a cell sets the caliper, which reads the interval and
/// rate; the keys ask for another gain and speed; Delete clears the caliper; the screen
/// reader gets the title, settings, reading and table.
#[test]
fn plot_calipers_and_keys() {
    use crate::Command;
    let mut text = Text::new();
    with_dicom(|st| {
        let mut c = chrome();
        c.show_plot(plot_sheet(true));
        draw(&mut c, &mut text, st);
        assert!(c.plot_open());
        let r = hit_rect(&c, &Target::PlotArea).expect("page drawn");
        // From 1/8 to 5/8 of the page: 4 s apart... 2 s with page 8 s wide... 25 mm/s × 100
        // mm = 4 s: a 50 mm drag is 2 s, 30 /min.
        let y = r.y + r.h * 0.5;
        let (xa, xb) = (r.x + r.w * 0.125, r.x + r.w * 0.375);
        c.input(
            &Input::PointerButton {
                x: xa,
                y,
                down: true,
                button: Button::Primary,
            },
            st,
            2.0,
        );
        c.input(&Input::PointerMove { x: xb, y }, st, 2.0);
        c.input(
            &Input::PointerButton {
                x: xb,
                y,
                down: false,
                button: Button::Primary,
            },
            st,
            2.0,
        );
        let (i, a, b) = c.plot_caliper().expect("caliper");
        assert_eq!(i, 0);
        assert!((a - 25.0).abs() < 0.5 && (b - 75.0).abs() < 0.5, "{a} {b}");
        let t = tree(&mut c, &mut text, st);
        let texts: Vec<String> = t
            .nodes
            .iter()
            .filter_map(|(_, n)| n.value().or(n.label()).map(str::to_owned))
            .collect();
        assert!(
            texts.iter().any(|l| l.contains("/min") && l.contains("ms")),
            "{texts:?}"
        );
        assert!(texts.iter().any(|l| l == "II, 3"), "{texts:?}");

        let key = |c: &mut Chrome, ch: char| {
            c.input(
                &Input::Key {
                    combo: KeyCombo::new(Key::Char(ch), Mods::NONE),
                    repeat: false,
                },
                st,
                2.0,
            )
            .commands
        };
        assert_eq!(
            key(&mut c, '+'),
            [Command::PlotAdjust { gain: 1, speed: 0 }]
        );
        assert_eq!(
            key(&mut c, '['),
            [Command::PlotAdjust { gain: 0, speed: -1 }]
        );
        // A new layout of the same page keeps the caliper; Delete clears it.
        c.set_plot(plot_sheet(true));
        assert!(c.plot_caliper().is_some());
        press(&mut c, st, NamedKey::Delete);
        assert!(c.plot_caliper().is_none());
        // A plot that is not adjustable ignores the keys.
        c.set_plot(plot_sheet(false));
        assert!(key(&mut c, '+').is_empty());
        press(&mut c, st, NamedKey::Escape);
        assert!(!c.plot_open());
    });
}

/// The filter is a text field: Left and Right move its caret while it holds a filter
/// (and open and close sequences while it is empty), Shift selects, typing replaces the
/// selection, the clipboard goes through the app, Ctrl+Z undoes, Home and End stay the
/// tree's, a click puts the caret and Shift+click selects; the list follows every edit.
#[test]
fn filter_is_a_text_field() {
    use crate::Command;
    let mut text = Text::new();
    let key = |c: &mut Chrome, st: &ViewerState, k: Key, mods: Mods| {
        c.input(
            &Input::Key {
                combo: KeyCombo::new(k, mods),
                repeat: false,
            },
            st,
            2.0,
        )
    };
    with_dicom(|st| {
        let mut c = chrome();
        c.primary_selection = false;
        c.show_dicom_tags(sheet());
        draw(&mut c, &mut text, st);
        // Empty field: Right and Left are the tree's.
        press(&mut c, st, NamedKey::ArrowDown);
        press(&mut c, st, NamedKey::ArrowDown);
        press(&mut c, st, NamedKey::ArrowRight);
        assert_eq!(shown(&c).len(), 5);
        press(&mut c, st, NamedKey::ArrowLeft);
        assert_eq!(shown(&c).len(), 4);
        // With a filter they move the caret.
        typed(&mut c, st, "rows");
        draw(&mut c, &mut text, st);
        assert_eq!(shown(&c), ["Rows"]);
        press(&mut c, st, NamedKey::ArrowLeft);
        press(&mut c, st, NamedKey::ArrowLeft);
        assert_eq!(c.field_edit().unwrap().caret(), 2);
        typed(&mut c, st, "X");
        assert_eq!(c.dicom_tags.query, "roXws");
        draw(&mut c, &mut text, st);
        assert!(shown(&c).is_empty());
        // Shift+Left selects; Ctrl+C asks the app to copy; typing replaces.
        key(&mut c, st, Key::Named(NamedKey::ArrowLeft), Mods::SHIFT);
        key(&mut c, st, Key::Named(NamedKey::ArrowLeft), Mods::SHIFT);
        assert_eq!(c.field_edit().unwrap().selected_text(), "oX");
        let r = key(&mut c, st, Key::Char('c'), Mods::CTRL);
        assert_eq!(r.commands, [Command::EditorCopy("oX".into())]);
        typed(&mut c, st, "o");
        assert_eq!(c.dicom_tags.query, "rows");
        draw(&mut c, &mut text, st);
        assert_eq!(shown(&c), ["Rows"]);
        // Undo, one step at a time.
        key(&mut c, st, Key::Char('z'), Mods::CTRL);
        assert_eq!(c.dicom_tags.query, "roXws");
        key(&mut c, st, Key::Char('z'), Mods::CTRL);
        assert_eq!(c.dicom_tags.query, "rows");
        // A paste goes over the selection, as one line.
        key(&mut c, st, Key::Char('a'), Mods::CTRL);
        let r = key(&mut c, st, Key::Char('v'), Mods::CTRL);
        assert_eq!(r.commands, [Command::EditorPaste]);
        c.paste_text("modal\nity");
        assert_eq!(c.dicom_tags.query, "modal ity");
        // Home and End move in the tree; Backspace and Delete edit the field.
        key(&mut c, st, Key::Char('a'), Mods::CTRL);
        typed(&mut c, st, "o");
        draw(&mut c, &mut text, st);
        let n = shown(&c).len();
        assert!(n > 1, "{:?}", shown(&c));
        press(&mut c, st, NamedKey::End);
        assert_eq!(c.dicom_tags.selected, n - 1);
        press(&mut c, st, NamedKey::Home);
        assert_eq!(c.dicom_tags.selected, 0);
        assert_eq!(c.field_edit().unwrap().caret(), 1);
        // A click puts the caret, Shift+click selects to it.
        typed(&mut c, st, "dality");
        draw(&mut c, &mut text, st);
        let g = c.field_geom.clone().expect("the filter is drawn");
        let (b, x0) = g.text.clone().expect("its text is drawn");
        let y = g.rect.center().1;
        let click = |c: &mut Chrome, i: usize, t: f64| {
            for down in [true, false] {
                let ev = Input::PointerButton {
                    x: x0 + b.caret_x(i) + 0.5,
                    y,
                    button: crate::Button::Primary,
                    down,
                };
                c.input(&ev, st, t);
            }
        };
        click(&mut c, 1, 3.0);
        assert_eq!(c.field_edit().unwrap().caret(), 1);
        c.input(&Input::Modifiers(Mods::SHIFT), st, 3.5);
        click(&mut c, 4, 4.0);
        c.input(&Input::Modifiers(Mods::NONE), st, 4.5);
        assert_eq!(c.field_edit().unwrap().selected_text(), "dal");
        press(&mut c, st, NamedKey::Backspace);
        assert_eq!(c.dicom_tags.query, "oity");
        // The screen reader sees the selection.
        key(&mut c, st, Key::Named(NamedKey::Home), Mods::SHIFT);
        let t = tree(&mut c, &mut text, st);
        let field = find(&t, Role::SearchInput);
        let sel = field[0].text_selection().expect("a selection");
        assert_eq!(
            (sel.anchor.character_index, sel.focus.character_index),
            (1, 0)
        );
    });
}

/// Tab goes from the field to Show patient data and Close; Space there turns patient
/// data on, for this opening only.
#[test]
fn patient_switch_by_keyboard() {
    let mut text = Text::new();
    with_dicom(|st| {
        let mut c = chrome();
        c.show_dicom_tags(sheet());
        draw(&mut c, &mut text, st);
        assert_eq!(c.dicom_tags.hidden(), 1);
        press(&mut c, st, NamedKey::Tab);
        assert_eq!(c.modal_focus, Some(Target::TagPatient));
        press(&mut c, st, NamedKey::Space);
        assert!(c.dicom_tags.patient);
        assert_eq!(c.dicom_tags.hidden(), 0);
        press(&mut c, st, NamedKey::Tab);
        assert_eq!(c.modal_focus, Some(Target::ModalClose));
        press(&mut c, st, NamedKey::Tab);
        assert_eq!(c.modal_focus, None);
        press(&mut c, st, NamedKey::Escape);
        c.show_dicom_tags(sheet());
        assert!(!c.dicom_tags.patient, "hidden again on the next opening");
    });
}

/// The tree for screen readers: a tree of items with their level and state, the switch,
/// the filter field; a hidden value is announced as hidden, never read out.
#[test]
fn accessibility_tree() {
    let mut text = Text::new();
    with_dicom(|st| {
        let mut c = chrome();
        c.show_dicom_tags(sheet());
        let t = tree(&mut c, &mut text, st);
        assert_eq!(find(&t, Role::Tree).len(), 1);
        let items = find(&t, Role::TreeItem);
        assert_eq!(items.len(), 4);
        for n in &items {
            assert!(!n.label().unwrap_or_default().contains("SECRET"));
        }
        let seq = items
            .iter()
            .find(|n| n.label().is_some_and(|l| l.contains("RequestAttributes")))
            .unwrap();
        assert_eq!(seq.is_expanded(), Some(false));
        assert_eq!(seq.level(), Some(1));
        let sw = find(&t, Role::Switch);
        assert_eq!(sw.len(), 1);
        assert_eq!(find(&t, Role::SearchInput).len(), 1);
        assert!(
            t.nodes
                .iter()
                .all(|(_, n)| !n.value().unwrap_or_default().contains("SECRET"))
        );
        // Shown once asked for.
        c.toggle_patient_data();
        let t = tree(&mut c, &mut text, st);
        assert!(
            find(&t, Role::TreeItem)
                .iter()
                .any(|n| n.label().unwrap_or_default().contains("SECRET"))
        );
        assert_eq!(c.modal, Some(Modal::DicomTags));
    });
}
