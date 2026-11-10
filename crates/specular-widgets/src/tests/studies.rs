use super::enabled_state::with_state;
use crate::accesskit::{Node, Role, TreeUpdate};
use crate::ctx::Target;
use crate::{Chrome, Command, Input, Modal, Studies, StudyKind, StudyLine, ViewerState};
use specular_gfx::{DrawList, Text};
use specular_input::{ActionState, ImageState, Key, KeyCombo, Mods, NamedKey};
use specular_theme::Palette;

fn with<R>(f: impl FnOnce(&ViewerState) -> R) -> R {
    with_state(
        ActionState {
            image: ImageState::Ready,
            count: 1,
            ..ActionState::default()
        },
        f,
    )
}

fn row(kind: StudyKind, title: &str, detail: &str, id: u64) -> StudyLine {
    StudyLine {
        kind,
        title: title.into(),
        detail: detail.into(),
        id,
        badge: "CT".into(),
        ..StudyLine::default()
    }
}

/// One patient (hidden), two studies, three series; the last series' files are missing.
pub(crate) fn sheet() -> Studies {
    let mut patient = row(StudyKind::Patient, "SECRET^NAME", "19700101", 0);
    patient.identifying = true;
    patient.hidden_title = "Patient 1".into();
    let mut gone = row(StudyKind::Series, "Series 3", "MR · 2 images missing", 3);
    gone.missing = true;
    Studies {
        title: "DISC".into(),
        rows: vec![
            patient,
            row(StudyKind::Study, "CT Thorax", "2 Jan 2024 · CT", 0),
            row(StudyKind::Series, "Series 1", "CT · 120 images", 1),
            row(StudyKind::Series, "Series 2", "CT · 3 images", 2),
            row(StudyKind::Study, "MR Knee", "1 Jan 2023 · MR", 0),
            gone,
        ],
        status: None,
        note: Some("2 files missing".into()),
        cancellable: false,
        sendable: false,
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

fn press(c: &mut Chrome, st: &ViewerState, k: NamedKey) -> Vec<Command> {
    let r = c.input(
        &Input::Key {
            combo: KeyCombo::new(Key::Named(k), Mods::NONE),
            repeat: false,
        },
        st,
        2.0,
    );
    assert!(r.consumed, "{k:?}");
    r.commands
}

fn find(t: &TreeUpdate, role: Role) -> Vec<&Node> {
    t.nodes
        .iter()
        .filter(|(_, n)| n.role() == role)
        .map(|(_, n)| n)
        .collect()
}

/// The keyboard goes from series to series; Enter opens one, not a missing one; updates
/// keep the selection; Escape closes and drops the rows.
#[test]
fn browse_open_and_close() {
    let mut text = Text::new();
    with(|st| {
        let mut c = chrome();
        c.show_studies(sheet());
        draw(&mut c, &mut text, st);
        assert_eq!(c.studies_selected(), Some(1));
        assert_eq!(
            press(&mut c, st, NamedKey::Enter),
            [Command::OpenStudySeries(1)]
        );
        press(&mut c, st, NamedKey::ArrowDown);
        assert_eq!(c.studies_selected(), Some(2));
        press(&mut c, st, NamedKey::ArrowDown);
        press(&mut c, st, NamedKey::ArrowDown);
        assert_eq!(c.studies_selected(), Some(3));
        assert!(
            press(&mut c, st, NamedKey::Enter).is_empty(),
            "missing files"
        );
        press(&mut c, st, NamedKey::Home);
        press(&mut c, st, NamedKey::ArrowDown);
        // More of the disc read: series 2 moves down, the selection follows it.
        let mut more = sheet();
        more.rows
            .insert(2, row(StudyKind::Series, "Series 0", "CT", 9));
        c.set_studies(more);
        assert_eq!(c.studies.selected, 4);
        assert_eq!(c.studies_selected(), Some(2));
        // A click on a study selects its first series; on a series opens it.
        draw(&mut c, &mut text, st);
        assert!(c.activate(Target::StudyRow(5), st).commands.is_empty());
        assert_eq!(c.studies_selected(), Some(3));
        assert_eq!(
            c.activate(Target::StudyRow(2), st).commands,
            [Command::OpenStudySeries(9)]
        );
        press(&mut c, st, NamedKey::Escape);
        assert_eq!(c.modal, None);
        assert!(c.studies.sheet.rows.is_empty());
        assert!(!c.studies.patient);
        // Closed: updates are ignored.
        c.set_studies(sheet());
        assert!(c.studies.sheet.rows.is_empty());
    });
}

/// While the disc is read, Cancel is in the footer and on the Tab path; Esc stops the
/// reading (the browser stays), and once it stopped Esc closes.
#[test]
fn cancel_stops_reading_then_esc_closes() {
    let mut text = Text::new();
    with(|st| {
        let mut c = chrome();
        let mut reading = sheet();
        reading.status = Some("Reading the disc: 2 of 9 files".into());
        reading.cancellable = true;
        c.show_studies(reading.clone());
        draw(&mut c, &mut text, st);
        assert!(c.hits.iter().any(|(_, h)| *h == Target::StudiesCancel));
        assert_eq!(
            press(&mut c, st, NamedKey::Escape),
            [Command::StudiesCancel]
        );
        assert_eq!(c.modal, Some(Modal::Studies));
        // Tab: the switch, Cancel, Close.
        press(&mut c, st, NamedKey::Tab);
        press(&mut c, st, NamedKey::Tab);
        assert_eq!(c.modal_focus, Some(Target::StudiesCancel));
        assert_eq!(press(&mut c, st, NamedKey::Enter), [Command::StudiesCancel]);
        let t = c.accessibility_tree(st, "x", 1.0);
        assert!(
            t.nodes
                .iter()
                .any(|(_, n)| n.role() == Role::Button && n.label() == Some("Cancel"))
        );
        // Stopped: no Cancel; Esc closes.
        reading.cancellable = false;
        reading.status = Some("Reading stopped".into());
        c.set_studies(reading);
        draw(&mut c, &mut text, st);
        assert!(!c.hits.iter().any(|(_, h)| *h == Target::StudiesCancel));
        assert!(press(&mut c, st, NamedKey::Escape).is_empty());
        assert_eq!(c.modal, None);
    });
}

/// Patient data is hidden until switched on (by keyboard here), never in Debug output,
/// never read out while hidden.
#[test]
fn patient_data_hidden_by_default() {
    let mut text = Text::new();
    with(|st| {
        let mut c = chrome();
        c.show_studies(sheet());
        assert!(!format!("{:?}", sheet()).contains("SECRET"));
        assert!(!format!("{:?}", sheet()).contains("1970"));
        draw(&mut c, &mut text, st);
        let t = c.accessibility_tree(st, "x", 1.0);
        assert_eq!(find(&t, Role::Tree).len(), 1);
        let items = find(&t, Role::TreeItem);
        assert_eq!(items.len(), 6);
        for n in t.nodes.iter().map(|(_, n)| n) {
            let all = format!(
                "{} {}",
                n.label().unwrap_or_default(),
                n.value().unwrap_or_default()
            );
            assert!(!all.contains("SECRET") && !all.contains("1970"), "{all}");
        }
        assert!(items.iter().any(|n| n.label() == Some("Patient 1, Hidden")));
        assert_eq!(items[2].level(), Some(3));
        assert_eq!(items[2].is_selected(), Some(true));
        // Tab onto the switch, Space turns it on.
        press(&mut c, st, NamedKey::Tab);
        assert_eq!(c.modal_focus, Some(Target::StudiesPatient));
        press(&mut c, st, NamedKey::Space);
        assert!(c.studies.patient);
        draw(&mut c, &mut text, st);
        let t = c.accessibility_tree(st, "x", 1.0);
        assert!(
            find(&t, Role::TreeItem)
                .iter()
                .any(|n| n.label() == Some("SECRET^NAME, 19700101"))
        );
        let sw = find(&t, Role::Switch);
        assert_eq!(sw.len(), 1);
        assert_eq!(c.modal, Some(Modal::Studies));
    });
}

/// Long lists scroll to keep the selection in view, at large text too.
#[test]
fn scrolls_to_the_selection() {
    let mut text = Text::new();
    with(|st| {
        let mut c = chrome();
        let mut s = sheet();
        for i in 10..60 {
            s.rows.push(row(StudyKind::Series, "Series", "CT", i));
        }
        c.show_studies(s);
        draw(&mut c, &mut text, st);
        assert_eq!(c.studies.scroll, 0.0);
        press(&mut c, st, NamedKey::End);
        draw(&mut c, &mut text, st);
        assert!(c.studies.scroll > 1000.0, "{}", c.studies.scroll);
        assert_eq!(c.studies_selected(), Some(59));
        press(&mut c, st, NamedKey::Home);
        draw(&mut c, &mut text, st);
        // The first series with its patient and study above it.
        assert_eq!(c.studies.scroll, 0.0);
    });
}

/// Once the medium is read, "Send to PACS…" is in the footer and on the Tab path, and sends
/// the selected series' id; never while reading.
#[test]
fn send_offered_once_read() {
    let mut text = Text::new();
    with(|st| {
        let mut c = chrome();
        let mut s = sheet();
        s.sendable = true;
        s.cancellable = true;
        c.show_studies(s.clone());
        draw(&mut c, &mut text, st);
        assert!(!c.hits.iter().any(|(_, h)| *h == Target::StudiesSend));
        s.cancellable = false;
        c.set_studies(s);
        draw(&mut c, &mut text, st);
        assert!(c.hits.iter().any(|(_, h)| *h == Target::StudiesSend));
        // Tab: the switch, Send.
        press(&mut c, st, NamedKey::Tab);
        press(&mut c, st, NamedKey::Tab);
        assert_eq!(c.modal_focus, Some(Target::StudiesSend));
        let sel = c.studies_selected();
        assert!(sel.is_some());
        assert_eq!(
            press(&mut c, st, NamedKey::Enter),
            [Command::StudiesSend(sel)]
        );
        let t = c.accessibility_tree(st, "x", 1.0);
        assert!(t.nodes.iter().any(|(_, n)| n.role() == Role::Button
            && n.label().is_some_and(|l| l.starts_with("Send to PACS"))));
    });
}
