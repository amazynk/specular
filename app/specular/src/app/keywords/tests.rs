use super::*;

fn panel(targets: &[(&str, &[&str])]) -> Keywords {
    let mut k = Keywords {
        open: true,
        field_focus: true,
        ..Keywords::default()
    };
    for (p, kws) in targets {
        let p = PathBuf::from(p);
        k.targets.push(p.clone());
        k.files
            .insert(p, kws.iter().map(|s| (*s).to_owned()).collect());
    }
    k.vocab.add("Places|France|Paris");
    k.vocab.add("Places|Spain");
    k.vocab.add("sea");
    k.rows = k.build_rows();
    k
}

fn keyword_rows(k: &Keywords) -> Vec<(String, Kind, Check)> {
    k.rows
        .iter()
        .filter_map(|r| match r {
            Row::Keyword {
                path, kind, check, ..
            } => Some((path.clone(), *kind, *check)),
            _ => None,
        })
        .collect()
}

fn typed(k: &mut Keywords, s: &str) -> KeyOut {
    let mut clip = PendingClipboard::default();
    let mut last = KeyOut::Used;
    for c in s.chars() {
        let t = c.to_string();
        last = k.key(Key::Char(c), Mods::default(), Some(&t), &mut clip);
    }
    k.rows = k.build_rows();
    last
}

fn press(k: &mut Keywords, n: NamedKey) -> KeyOut {
    let out = k.key(
        Key::Named(n),
        Mods::default(),
        None,
        &mut PendingClipboard::default(),
    );
    k.rows = k.build_rows();
    out
}

#[test]
fn rows_show_what_all_and_some_photos_have() {
    let k = panel(&[("/a.jpg", &["sea", "Places|Spain"]), ("/b.jpg", &["sea"])]);
    let rows = keyword_rows(&k);
    assert!(rows.contains(&("sea".into(), Kind::Assigned, Check::On)));
    assert!(rows.contains(&("Places|Spain".into(), Kind::Assigned, Check::Some)));
    // The list as a tree, each with its state.
    let listed: Vec<_> = rows.iter().filter(|r| r.1 == Kind::Listed).collect();
    assert_eq!(listed.len(), 5);
    assert_eq!(listed[0].0, "Places");
    assert!(listed.contains(&&("Places|France|Paris".into(), Kind::Listed, Check::Off)));
}

#[test]
fn nothing_selected_says_so_and_changes_nothing() {
    let mut k = panel(&[]);
    assert!(k.rows.contains(&Row::Note(t!("keywords-none-selected"))));
    assert_eq!(typed(&mut k, "sea"), KeyOut::Used);
    assert_eq!(press(&mut k, NamedKey::Enter), KeyOut::Used);
}

#[test]
fn typing_suggests_and_enter_adds() {
    let mut k = panel(&[("/a.jpg", &[])]);
    typed(&mut k, "par");
    let rows = keyword_rows(&k);
    assert_eq!(
        rows[0],
        ("Places|France|Paris".into(), Kind::Suggested, Check::Off)
    );
    // A new keyword is offered for creation.
    assert!(k.rows.contains(&Row::Create("par".into())));
    // Tab takes the first suggestion, Enter adds it.
    press(&mut k, NamedKey::Tab);
    assert_eq!(k.edit.text(), "Places > France > Paris");
    assert_eq!(
        press(&mut k, NamedKey::Enter),
        KeyOut::Apply(vec![Change::Add("Places|France|Paris".into())])
    );
    assert!(k.edit.is_empty());
}

#[test]
fn a_synonym_typed_whole_adds_its_keyword() {
    let mut k = panel(&[("/a.jpg", &[])]);
    k.vocab
        .set_synonyms("Places|France|Paris", &["Lutetia".to_owned()]);
    typed(&mut k, "lutetia");
    assert_eq!(
        press(&mut k, NamedKey::Enter),
        KeyOut::Apply(vec![Change::Add("Places|France|Paris".into())])
    );
}

#[test]
fn rows_toggle_and_delete_takes_off() {
    let mut k = panel(&[("/a.jpg", &["sea"]), ("/b.jpg", &[])]);
    // Down from the field to the first keyword: "sea", on some.
    press(&mut k, NamedKey::ArrowDown);
    assert!(!k.field_focus);
    assert_eq!(k.current.as_deref(), Some("sea"));
    assert_eq!(
        press(&mut k, NamedKey::Space),
        KeyOut::Apply(vec![Change::Add("sea".into())])
    );
    k.assume(&[Change::Add("sea".into())]);
    k.rows = k.build_rows();
    assert_eq!(k.files[Path::new("/b.jpg")], ["sea"]);
    assert_eq!(k.recents.list(), ["sea"]);
    // On all now: Enter takes it off, as Delete does.
    assert_eq!(
        press(&mut k, NamedKey::Enter),
        KeyOut::Apply(vec![Change::Remove("sea".into())])
    );
    assert_eq!(
        press(&mut k, NamedKey::Delete),
        KeyOut::Apply(vec![Change::Remove("sea".into())])
    );
    // Typing on a row goes back to the field.
    typed(&mut k, "x");
    assert!(k.field_focus && k.focus.is_none());
    assert_eq!(k.edit.text(), "x");
    // Escape clears, then gives the keyboard back.
    press(&mut k, NamedKey::Escape);
    assert!(k.edit.is_empty() && k.field_focus);
    press(&mut k, NamedKey::Escape);
    assert!(!k.has_focus());
}

#[test]
fn private_synonyms_and_forget_ask_first() {
    let mut k = panel(&[("/a.jpg", &[])]);
    k.current = Some("Places".into());
    assert!(k.button(Button::Private));
    assert!(k.vocab.is_private("Places"));
    // Synonyms are edited in the field.
    k.current = Some("Places|Spain".into());
    k.button(Button::Synonyms);
    assert_eq!(k.mode, Mode::Synonyms("Places|Spain".into()));
    typed(&mut k, "España, Iberia");
    press(&mut k, NamedKey::Enter);
    assert_eq!(
        k.vocab.get("Places|Spain").unwrap().synonyms,
        ["España", "Iberia"]
    );
    assert_eq!(k.mode, Mode::Add);
    // Removing from the list asks; Escape keeps it, Enter removes it and what is below.
    k.current = Some("Places".into());
    k.button(Button::Forget);
    assert_eq!(
        press(&mut k, NamedKey::Escape),
        KeyOut::Button(Button::Cancel)
    );
    k.button(Button::Cancel);
    assert!(k.vocab.contains("Places"));
    k.button(Button::Forget);
    assert_eq!(
        press(&mut k, NamedKey::Enter),
        KeyOut::Button(Button::ConfirmForget)
    );
    k.button(Button::ConfirmForget);
    assert!(!k.vocab.contains("Places|Spain") && k.vocab.contains("sea"));
}

#[test]
fn recents_are_offered_when_nothing_is_typed() {
    let mut k = panel(&[("/a.jpg", &["sea"])]);
    k.recents.used("Places|Spain");
    k.recents.used("sea");
    k.rows = k.build_rows();
    let recent: Vec<_> = keyword_rows(&k)
        .into_iter()
        .filter(|r| r.1 == Kind::Recent)
        .map(|r| r.0)
        .collect();
    // "sea" is on every photo already.
    assert_eq!(recent, ["Places|Spain"]);
}

#[test]
fn keyword_list_files_in_other_encodings() {
    assert_eq!(decode_text(b"A\r\n"), "A\r\n");
    assert_eq!(decode_text(&[0xFF, 0xFE, b'A', 0, b'\n', 0]), "A\n");
    assert_eq!(decode_text(&[0xE9]), "é");
}

#[test]
fn rename_and_move_are_typed_in_the_field() {
    let mut k = panel(&[("/a.jpg", &["Places|France|Paris"])]);
    // Rename: the field starts with the name; the levels above stay.
    k.current = Some("Places|France|Paris".into());
    assert!(k.button(Button::Rename));
    assert_eq!(k.mode, Mode::Rename("Places|France|Paris".into()));
    assert_eq!(k.edit.text(), "Paris");
    k.edit.clear();
    typed(&mut k, "Paris 2024");
    assert_eq!(
        press(&mut k, NamedKey::Enter),
        KeyOut::Rename(
            "Places|France|Paris".into(),
            "Places|France|Paris 2024".into()
        )
    );
    assert_eq!(k.mode, Mode::Add);
    // Move: the field starts with where it is; empty puts it at the top.
    k.current = Some("Places|Spain".into());
    k.button(Button::Move);
    assert_eq!(k.edit.text(), "Places");
    k.edit.clear();
    typed(&mut k, "Trips > 2024");
    assert_eq!(
        press(&mut k, NamedKey::Enter),
        KeyOut::Rename("Places|Spain".into(), "Trips|2024|Spain".into())
    );
    k.current = Some("Places|Spain".into());
    k.button(Button::Move);
    k.edit.clear();
    assert_eq!(
        press(&mut k, NamedKey::Enter),
        KeyOut::Rename("Places|Spain".into(), "Spain".into())
    );
    // Under itself: refused, said why, the field stays.
    k.current = Some("Places".into());
    k.button(Button::Move);
    k.edit.clear();
    typed(&mut k, "Places > France");
    assert_eq!(press(&mut k, NamedKey::Enter), KeyOut::Used);
    assert!(k.status.is_some() && k.mode == Mode::Move("Places".into()));
    // Escape gives up.
    press(&mut k, NamedKey::Escape);
    assert_eq!(k.mode, Mode::Add);
}
