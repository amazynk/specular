use super::*;

/// The info panel's All-fields rows: one Tab stop; ↑/↓, Page keys and Home/End move along
/// them, the focused row scrolls into view, and Enter or Ctrl+C copy its value.
#[test]
fn tab_reaches_the_info_rows() {
    use super::super::enabled_state::{STATES, with_metadata};
    use specular_metadata::{Field, FieldGroup, MetadataReport};
    let field = |i: usize| Field {
        tag: format!("Tag{i}"),
        raw: format!("value {i}"),
        display: format!("value {i}"),
        empty: false,
        full: None,
    };
    let mut fields: Vec<Field> = (0..120).map(field).collect();
    fields[1].empty = true;
    fields[1].display.clear();
    let meta = MetadataReport {
        all_fields: vec![FieldGroup {
            title: "Exif".into(),
            fields,
        }],
        language: "en".into(),
        ..Default::default()
    };
    let copied = |r: &Response| -> Vec<String> {
        r.commands
            .iter()
            .filter_map(|c| match c {
                Command::CopyText(s) => Some(s.clone()),
                _ => None,
            })
            .collect()
    };
    let mut text = Text::new();
    with_metadata(STATES[3], Some(&meta), |st| {
        let mut c = chrome(1000.0, false);
        c.set_accessibility(true);
        c.set_info(true, true);
        draw(&mut c, &mut text, st);
        // Tab reaches the rows after the panel's tabs, on the first row.
        let mut seen = Vec::new();
        for _ in 0..30 {
            tab(&mut c, st);
            draw(&mut c, &mut text, st);
            match &c.kbd {
                Some(t @ Target::InfoCopy(..)) => {
                    seen.push(t.clone());
                    break;
                }
                Some(t) => seen.push(t.clone()),
                None => break,
            }
        }
        assert_eq!(seen.last(), Some(&Target::InfoCopy(0, 0)), "{seen:?}");
        assert!(seen.contains(&Target::InfoTab(true)));
        let full = hit(&c, &Target::InfoCopy(0, 0)).h;
        // Down skips the empty field; Up at the top stays.
        press(&mut c, st, NamedKey::ArrowDown, Mods::NONE);
        assert_eq!(c.kbd, Some(Target::InfoCopy(0, 2)));
        press(&mut c, st, NamedKey::ArrowUp, Mods::NONE);
        press(&mut c, st, NamedKey::ArrowUp, Mods::NONE);
        assert_eq!(c.kbd, Some(Target::InfoCopy(0, 0)));
        // End: the last row, scrolled fully into view.
        press(&mut c, st, NamedKey::End, Mods::NONE);
        assert_eq!(c.kbd, Some(Target::InfoCopy(0, 119)));
        draw(&mut c, &mut text, st);
        assert!(c.info_scroll > 0.0);
        let r = hit(&c, &Target::InfoCopy(0, 119));
        assert!((r.h - full).abs() < 0.5, "{r:?} vs {full}");
        assert_eq!(c.info_reveal, 0, "settled");
        // Page keys move by a view's worth; Home goes back to the top.
        press(&mut c, st, NamedKey::PageUp, Mods::NONE);
        let Some(Target::InfoCopy(0, f)) = c.kbd.clone() else {
            panic!("{:?}", c.kbd)
        };
        assert!((100..118).contains(&f), "{f}");
        draw(&mut c, &mut text, st);
        let r = hit(&c, &Target::InfoCopy(0, f));
        assert!((r.h - full).abs() < 0.5, "{r:?}");
        press(&mut c, st, NamedKey::Home, Mods::NONE);
        draw(&mut c, &mut text, st);
        assert_eq!(c.info_scroll, 0.0);
        // A row far down, past the shaped part: reached in steps.
        for _ in 0..60 {
            press(&mut c, st, NamedKey::ArrowDown, Mods::NONE);
        }
        assert_eq!(c.kbd, Some(Target::InfoCopy(0, 61)));
        draw(&mut c, &mut text, st);
        let r = hit(&c, &Target::InfoCopy(0, 61));
        assert!((r.h - full).abs() < 0.5, "{r:?}");
        // Enter, Space and Ctrl+C copy the value; a held key copies once.
        let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(copied(&r), ["value 61"]);
        assert!(copied(&repeat(&mut c, st, NamedKey::Enter, 2.1)).is_empty());
        let r = press(&mut c, st, NamedKey::Space, Mods::NONE);
        assert_eq!(copied(&r), ["value 61"]);
        let ctrl_c = Input::Key {
            combo: KeyCombo::new(Key::Char('c'), Mods::CTRL),
            repeat: false,
        };
        let r = c.input(&ctrl_c, st, 2.0);
        assert!(r.consumed);
        assert_eq!(copied(&r), ["value 61"]);
        // In the tree the row has the focus.
        let t = c.accessibility_tree(st, "x", 1.0);
        assert_eq!(focused_node(&t).description(), Some("Tag61: value 61"));
        // Tab goes on (to the image: no indicator); Shift+Tab comes back to the last row
        // in view.
        tab(&mut c, st);
        assert_eq!(c.kbd, None);
        press(&mut c, st, NamedKey::Tab, Mods::SHIFT);
        assert!(
            matches!(c.kbd, Some(Target::InfoCopy(0, f)) if f >= 61),
            "{:?}",
            c.kbd
        );
        // Esc gives the image the keyboard; the summary tab has no rows.
        press(&mut c, st, NamedKey::Escape, Mods::NONE);
        assert_eq!(c.kbd, None);
        c.set_info(true, false);
        draw(&mut c, &mut text, st);
        assert!(c.info_rows.is_empty());
        assert!(
            c.stops()
                .iter()
                .all(|s| !matches!(s.targets[0], Target::InfoCopy(..)))
        );
    });
}
