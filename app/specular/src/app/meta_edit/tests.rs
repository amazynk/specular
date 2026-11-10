use super::*;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_owned()).collect()
}

fn rec(fields: &[(Field, &[&str])]) -> Record {
    let mut r = Record::new();
    for (f, v) in fields {
        r.set(*f, s(v));
    }
    r
}

/// A panel on `photos`, every one read, the keyboard in its first field.
pub(super) fn panel(photos: &[(&str, Record)]) -> MetaEdit {
    let mut m = MetaEdit {
        open: true,
        ..MetaEdit::default()
    };
    for (p, r) in photos {
        let p = PathBuf::from(p);
        m.targets.push(p.clone());
        m.files.insert(p, r.clone());
    }
    m.refresh();
    m.rows = m.build_rows();
    m
}

fn focus_field(m: &mut MetaEdit, f: Field) {
    let k = m.rows.iter().position(|r| *r == Row::Field(f)).unwrap();
    m.focus_row(Some(k));
}

fn typed(m: &mut MetaEdit, t: &str) {
    let mut clip = PendingClipboard::default();
    for c in t.chars() {
        let x = c.to_string();
        assert_eq!(
            m.key(Key::Char(c), Mods::default(), Some(&x), &mut clip),
            KeyOut::Used
        );
    }
}

fn press(m: &mut MetaEdit, n: NamedKey) -> KeyOut {
    m.key(
        Key::Named(n),
        Mods::default(),
        None,
        &mut PendingClipboard::default(),
    )
}

#[test]
fn agreeing_fields_show_their_value_and_differing_ones_are_mixed() {
    let m = panel(&[
        (
            "/a.jpg",
            rec(&[
                (Field::Copyright, &["© Ann"]),
                (Field::Title, &["One"]),
                (Field::Keywords, &["sea", "Places|Lisbon"]),
            ]),
        ),
        (
            "/b.jpg",
            rec(&[
                (Field::Copyright, &["© Ann"]),
                (Field::Title, &["Two"]),
                (Field::Keywords, &["sea"]),
            ]),
        ),
    ]);
    assert_eq!(m.slot(Field::Copyright).edit.text(), "© Ann");
    assert!(!m.slot(Field::Copyright).mixed);
    assert!(m.slot(Field::Title).mixed);
    assert!(m.slot(Field::Title).edit.is_empty());
    // Keywords: those all have, and a note that some have others.
    assert_eq!(m.slot(Field::Keywords).edit.text(), "sea");
    assert!(m.slot(Field::Keywords).others);
    // Nothing selected: a note, no fields.
    let none = panel(&[]);
    assert!(none.rows.contains(&Row::Note(t!("metaedit-none-selected"))));
    assert!(!none.rows.iter().any(|r| matches!(r, Row::Field(_))));
}

/// Leaving a changed field writes that field alone; an untouched mixed field is not written.
#[test]
fn only_the_changed_field_is_written() {
    let mut m = panel(&[
        ("/a.jpg", rec(&[(Field::Title, &["One"])])),
        ("/b.jpg", rec(&[(Field::Title, &["Two"])])),
    ]);
    focus_field(&mut m, Field::Title);
    // Through a mixed field without typing: nothing.
    assert_eq!(press(&mut m, NamedKey::Tab), KeyOut::Used);
    assert_eq!(m.rows[m.focus.unwrap()], Row::Field(Field::Headline));
    typed(&mut m, "Big news");
    let out = press(&mut m, NamedKey::Enter);
    assert_eq!(
        out,
        KeyOut::Write(Edit {
            set: vec![(Field::Headline, s(&["Big news"]))],
            keywords: None,
        })
    );
    // The photos are assumed to have it; the titles still differ.
    for p in ["/a.jpg", "/b.jpg"] {
        assert_eq!(
            m.files[Path::new(p)].values(Field::Headline),
            s(&["Big news"])
        );
    }
    m.refresh();
    assert!(m.slot(Field::Title).mixed);
    assert!(!m.slot(Field::Headline).changed());
}

#[test]
fn a_mixed_field_is_cleared_on_all_only_when_asked() {
    let mut m = panel(&[
        ("/a.jpg", rec(&[(Field::Title, &["One"])])),
        ("/b.jpg", rec(&[(Field::Title, &["Two"])])),
    ]);
    focus_field(&mut m, Field::Title);
    assert_eq!(press(&mut m, NamedKey::Backspace), KeyOut::Used);
    assert!(m.slot(Field::Title).clearing);
    // Esc keeps them.
    press(&mut m, NamedKey::Escape);
    assert!(!m.slot(Field::Title).changed());
    press(&mut m, NamedKey::Backspace);
    let out = press(&mut m, NamedKey::Enter);
    assert_eq!(
        out,
        KeyOut::Write(Edit {
            set: vec![(Field::Title, vec![])],
            keywords: None,
        })
    );
}

/// Keywords typed in add and take off only what changed; each photo keeps its others.
#[test]
fn keywords_change_only_what_was_typed() {
    let mut m = panel(&[
        (
            "/a.jpg",
            rec(&[(Field::Keywords, &["sea", "Places|Lisbon"])]),
        ),
        ("/b.jpg", rec(&[(Field::Keywords, &["sea", "boat"])])),
    ]);
    focus_field(&mut m, Field::Keywords);
    // "sea" → "sky; Places > Paris".
    m.slot_mut(Field::Keywords)
        .edit
        .set_text("sky; Places > Paris");
    let Some(KeyOut::Write(e)) = m.commit() else {
        panic!("a write")
    };
    assert_eq!(
        e.keywords,
        Some(KeywordEdit::Changes(vec![
            kw::Change::Add("sky".into()),
            kw::Change::Add("Places|Paris".into()),
            kw::Change::Remove("sea".into()),
        ]))
    );
    assert_eq!(
        m.files[Path::new("/a.jpg")].values(Field::Keywords),
        s(&["Places|Lisbon", "sky", "Places|Paris"])
    );
    assert_eq!(
        m.files[Path::new("/b.jpg")].values(Field::Keywords),
        s(&["boat", "sky", "Places|Paris"])
    );
}

/// Esc puts back what was there; a second Esc gives the keyboard back.
#[test]
fn escape_reverts_then_leaves() {
    let mut m = panel(&[("/a.jpg", rec(&[(Field::Credit, &["AP"])]))]);
    focus_field(&mut m, Field::Credit);
    typed(&mut m, " Photo");
    assert!(m.slot(Field::Credit).changed());
    press(&mut m, NamedKey::Escape);
    assert_eq!(m.slot(Field::Credit).edit.text(), "AP");
    assert!(m.has_focus());
    press(&mut m, NamedKey::Escape);
    assert!(!m.has_focus());
    assert_eq!(m.commit(), None);
}

/// A new template is edited with the fields; applying it writes its fields, its keywords
/// added or in place of the photos' own.
#[test]
fn templates_are_made_edited_applied_and_removed() {
    let mut m = panel(&[("/a.jpg", rec(&[(Field::Keywords, &["old"])]))]);
    assert_eq!(m.button(Button::NewTemplate), KeyOut::Templates);
    assert_eq!(m.mode, Mode::Template(0));
    assert_eq!(m.rows[m.focus.unwrap()], Row::Name);
    // The name, selected, is typed over.
    typed(&mut m, "Agency");
    assert_eq!(press(&mut m, NamedKey::Enter), KeyOut::Templates);
    assert_eq!(m.templates[0].name, "Agency");
    focus_field(&mut m, Field::Copyright);
    typed(&mut m, "© Agency");
    assert_eq!(press(&mut m, NamedKey::Tab), KeyOut::Templates);
    focus_field(&mut m, Field::Keywords);
    typed(&mut m, "news; Places > Rome");
    assert_eq!(press(&mut m, NamedKey::Enter), KeyOut::Templates);
    assert_eq!(
        m.templates[0].get(Field::Keywords),
        Some(&s(&["news", "Places|Rome"])[..])
    );
    // Keyword mode: replace.
    let k = m.rows.iter().position(|r| *r == Row::KeywordMode).unwrap();
    assert_eq!(m.click(Hit::Row(k)), KeyOut::Templates);
    assert_eq!(m.templates[0].keyword_mode, KeywordMode::Replace);
    m.button(Button::Done);
    assert_eq!(m.mode, Mode::Photos);
    m.rows = m.build_rows();
    assert!(m.rows.contains(&Row::Template(0)));
    // Applied: a click on its row.
    let k = m.rows.iter().position(|r| *r == Row::Template(0)).unwrap();
    assert_eq!(m.click(Hit::Row(k)), KeyOut::Apply(0));
    let e = m.templates[0].edit();
    assert_eq!(e.set, vec![(Field::Copyright, s(&["© Agency"]))]);
    assert_eq!(
        e.keywords,
        Some(KeywordEdit::Replace(s(&["news", "Places|Rome"])))
    );
    // Removing asks first, names it, and photos keep their metadata.
    m.current = Some(0);
    m.button(Button::RemoveTemplate);
    assert_eq!(m.confirm, Some(0));
    assert_eq!(
        press(&mut m, NamedKey::Escape),
        KeyOut::Button(Button::Cancel)
    );
    m.button(Button::Cancel);
    assert_eq!(m.templates.len(), 1);
    m.button(Button::RemoveTemplate);
    assert_eq!(m.button(Button::ConfirmRemove), KeyOut::Templates);
    assert!(m.templates.is_empty());
    assert_eq!(
        m.files[Path::new("/a.jpg")].values(Field::Keywords),
        s(&["old"])
    );
}

#[test]
fn every_field_has_a_label() {
    for f in Field::all() {
        let id = format!("metaedit-field-{}", f.key());
        assert!(
            specular_i18n::I18n::global().localizer().has_message(&id),
            "{id}"
        );
    }
}
