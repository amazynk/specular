use crate::accesskit::Role;
use crate::{Chrome, Input, Modal, ViewerState};
use specular_gfx::{DrawList, Rect, Text};
use specular_input::{Action, ActionState, ImageState, Key, KeyCombo, Mods, NamedKey};
use specular_theme::Palette;

const READY: ActionState = ActionState {
    comparing: false,
    compare_images: 0,
    volume: false,
    mpr: false,
    image: ImageState::Ready,
    count: 40,
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
    for i in 0..3 {
        dl.clear(1.0);
        c.draw(&mut dl, text, st, &Palette::dark(), 1.0 + i as f64 * 0.016);
    }
}

fn key(k: NamedKey) -> Input {
    Input::Key {
        combo: KeyCombo::new(Key::Named(k), Mods::NONE),
        repeat: false,
    }
}

fn inside(inner: Rect, outer: Rect) -> bool {
    inner.x >= outer.x - 0.5
        && inner.right() <= outer.right() + 0.5
        && inner.y >= outer.y - 0.5
        && inner.bottom() <= outer.bottom() + 0.5
}

/// The text of the focused field, as a screen reader reads it.
fn field_value(c: &Chrome, st: &ViewerState) -> Option<String> {
    let t = c.accessibility_tree(st, "beach.jpg", 1.0);
    let (_, n) = t.nodes.iter().find(|(id, _)| *id == t.focus)?;
    assert!(
        matches!(n.role(), Role::SearchInput | Role::TextInput),
        "focus on {:?}",
        n.role()
    );
    assert!(n.label().is_some_and(|l| !l.is_empty()), "field has a name");
    Some(n.value().unwrap_or_default().to_owned())
}

/// Preedit, cursor area, cancel and commit in each search field of a dialog.
#[test]
fn compose_cancel_and_commit_in_every_search_field() {
    let mut text = Text::new();
    crate::tests::enabled_state::with_state(READY, |st| {
        for m in [Modal::Palette, Modal::Settings] {
            let mut c = chrome();
            c.open_modal(m);
            draw(&mut c, &mut text, st);
            let field = c.text_field().expect("a focused field");
            let caret = c.ime_cursor_area().expect("an IME area with the field");
            assert!(inside(caret, field), "{m:?}: caret {caret:?} in {field:?}");
            assert!(
                caret.w < 8.0,
                "{m:?}: nothing composed, the area is the caret"
            );

            // Composing: shown in the field; the area spans it and ends at the caret.
            c.input(&Input::Preedit("にほんご".into()), st, 1.1);
            draw(&mut c, &mut text, st);
            assert_eq!(c.preedit(), "にほんご");
            let area = c.ime_cursor_area().unwrap();
            assert!(inside(area, field), "{m:?}: {area:?} in {field:?}");
            assert!(
                area.w > 30.0 && area.x > caret.x - 1.0,
                "{m:?}: composition area {area:?} after the caret {caret:?}"
            );
            assert_eq!(field_value(&c, st).as_deref(), Some(""), "{m:?}");

            // Escape and Enter belong to the input method while it composes.
            assert!(c.input(&key(NamedKey::Escape), st, 1.2).consumed);
            assert!(c.input(&key(NamedKey::Enter), st, 1.2).consumed);
            assert_eq!(c.modal(), Some(m), "{m:?}: still open");

            // Cancelled: nothing typed, the area is back at the caret.
            c.input(&Input::Preedit(String::new()), st, 1.3);
            draw(&mut c, &mut text, st);
            assert_eq!(c.preedit(), "");
            assert_eq!(c.ime_cursor_area(), Some(caret), "{m:?}");
            assert_eq!(field_value(&c, st).as_deref(), Some(""), "{m:?}");

            // Committed: the text is typed and the caret moves past it.
            c.input(&Input::Preedit("にほんご".into()), st, 1.4);
            c.input(&Input::Preedit(String::new()), st, 1.4);
            c.input(&Input::Text("日本語".into()), st, 1.4);
            draw(&mut c, &mut text, st);
            assert_eq!(field_value(&c, st).as_deref(), Some("日本語"), "{m:?}");
            let after = c.ime_cursor_area().unwrap();
            assert!(after.x > caret.x + 20.0, "{m:?}: {after:?} vs {caret:?}");

            // Escape without a composition clears the field, then closes the dialog.
            assert!(c.input(&key(NamedKey::Escape), st, 1.5).consumed);
            assert_eq!(field_value(&c, st).as_deref(), Some(""), "{m:?}");
            assert_eq!(c.modal(), Some(m), "{m:?}");
            assert!(c.input(&key(NamedKey::Escape), st, 1.5).consumed);
            assert_eq!(c.modal(), None, "{m:?}");
        }
    });
}

/// A dialog closed in the middle of a composition does not hand it to the next field,
/// and with no field the input method is off (no area).
#[test]
fn composition_does_not_outlive_its_field() {
    let mut text = Text::new();
    crate::tests::enabled_state::with_state(READY, |st| {
        let mut c = chrome();
        c.open_modal(Modal::Palette);
        draw(&mut c, &mut text, st);
        c.input(&Input::Preedit("ㅎㅏ".into()), st, 1.1);
        draw(&mut c, &mut text, st);
        c.close_modal();
        draw(&mut c, &mut text, st);
        assert_eq!(c.text_field(), None);
        assert_eq!(c.ime_cursor_area(), None);
        assert_eq!(c.preedit(), "");
        c.open_modal(Modal::Settings);
        draw(&mut c, &mut text, st);
        assert_eq!(c.preedit(), "");
        assert_eq!(field_value(&c, st).as_deref(), Some(""));
    });
}

/// Right to left: the composition is at the left end of the text, so the area grows
/// leftwards from the caret; emoji (with a skin-tone modifier) are typed whole.
#[test]
fn right_to_left_and_emoji() {
    let mut text = Text::new();
    crate::tests::enabled_state::with_state(READY, |st| {
        let mut c = chrome();
        c.force_rtl = true;
        c.open_modal(Modal::Palette);
        c.input(&Input::Text("שלום".into()), st, 1.0);
        draw(&mut c, &mut text, st);
        let field = c.text_field().unwrap();
        let caret = c.ime_cursor_area().unwrap();
        // Right-to-left text is right-aligned and ends (the caret) at its left edge.
        assert!(caret.x > field.center().0 && caret.right() < field.right() - 20.0);
        c.input(&Input::Preedit("مرحبا".into()), st, 1.1);
        draw(&mut c, &mut text, st);
        let area = c.ime_cursor_area().unwrap();
        assert!(inside(area, field) && area.w > 10.0, "{area:?}");
        assert!(area.x < caret.x, "{area:?} left of the old caret {caret:?}");
        c.input(&Input::Preedit(String::new()), st, 1.2);
        c.input(&Input::Text("👍🏽".into()), st, 1.2);
        draw(&mut c, &mut text, st);
        assert_eq!(field_value(&c, st).as_deref(), Some("שלום👍🏽"));
    });
}

/// The go-to field: a full-width number being composed is shown, the area is at its
/// caret, Enter while composing is the input method's, and the committed digits count.
#[test]
fn go_to_field_shows_a_composed_number() {
    let mut text = Text::new();
    crate::tests::enabled_state::with_state(READY, |st| {
        let mut c = chrome();
        assert!(c.handle_action(Action::GoToImage));
        draw(&mut c, &mut text, st);
        let field = c.text_field().expect("the go-to field");
        let caret = c.ime_cursor_area().unwrap();
        assert!(
            inside(caret, field) && caret.w < 4.0,
            "{caret:?} in {field:?}"
        );
        c.input(&Input::Preedit("１２".into()), st, 1.1);
        draw(&mut c, &mut text, st);
        let composing = c.ime_cursor_area().unwrap();
        assert!(composing.x > caret.x, "{composing:?} after {caret:?}");
        assert!(c.input(&key(NamedKey::Enter), st, 1.2).consumed);
        assert!(c.wants_text(), "Enter went to the input method");
        c.input(&Input::Preedit(String::new()), st, 1.3);
        c.input(&Input::Text("１２".into()), st, 1.3);
        assert_eq!(
            c.goto.as_ref().map(crate::line_edit::LineEdit::text),
            Some("12")
        );
    });
}

/// The help panel's search field: the composition shows, the area follows it, and a
/// screen reader finds the field by its own name ("Search the manual").
#[test]
fn help_search_takes_a_composition() {
    let mut text = Text::new();
    crate::tests::enabled_state::with_state(READY, |st| {
        let mut c = chrome();
        c.show_help("accessibility");
        assert_eq!(c.modal(), Some(Modal::Help));
        draw(&mut c, &mut text, st);
        let field = c.text_field().expect("the search field has the keyboard");
        let caret = c.ime_cursor_area().expect("an IME area");
        assert!(inside(caret, field), "{caret:?} in {field:?}");
        c.input(&Input::Preedit("けんさく".into()), st, 1.1);
        draw(&mut c, &mut text, st);
        let area = c.ime_cursor_area().unwrap();
        assert!(
            inside(area, field) && area.w > 30.0,
            "{area:?} in {field:?}"
        );
        c.input(&Input::Preedit(String::new()), st, 1.2);
        c.input(&Input::Text("検索".into()), st, 1.2);
        draw(&mut c, &mut text, st);
        let t = c.accessibility_tree(st, "beach.jpg", 1.0);
        let (_, n) = t.nodes.iter().find(|(id, _)| *id == t.focus).unwrap();
        assert_eq!(n.role(), Role::SearchInput);
        assert_eq!(n.label(), Some("Search the manual"));
        assert_eq!(n.value(), Some("検索"));
    });
}

/// The text of the settings file editor, as a screen reader reads it.
fn editor_value(c: &Chrome, st: &ViewerState) -> String {
    let t = c.accessibility_tree(st, "beach.jpg", 1.0);
    let (_, n) = t
        .nodes
        .iter()
        .find(|(_, n)| n.role() == Role::MultilineTextInput)
        .expect("the editor's text");
    n.value().unwrap_or_default().to_owned()
}

/// The settings file editor: the composition is drawn in the caret's line, the candidate
/// window opens under it, and committing types it at the caret.
#[test]
fn config_editor_composes_in_the_caret_line() {
    let mut text = Text::new();
    crate::tests::enabled_state::with_state(READY, |st| {
        let mut c = chrome();
        c.open_config_editor_with(st.store, Some("a = 1\n"));
        assert_eq!(c.modal(), Some(Modal::ConfigEditor));
        draw(&mut c, &mut text, st);
        let view = c.text_field().expect("the text has the keyboard");
        let caret = c.ime_cursor_area().expect("an IME area at the caret");
        assert!(inside(caret, view), "{caret:?} in {view:?}");
        assert!(caret.w < 4.0, "nothing composed: the caret {caret:?}");

        c.input(&Input::Preedit("かな".into()), st, 1.1);
        draw(&mut c, &mut text, st);
        assert_eq!(c.preedit(), "かな");
        let area = c.ime_cursor_area().unwrap();
        assert!(inside(area, view), "{area:?} in {view:?}");
        assert!(
            area.w > 15.0 && (area.y - caret.y).abs() < 0.5 && area.x >= caret.x - 1.0,
            "composition {area:?} at the caret {caret:?}"
        );
        // Not typed yet.
        assert_eq!(editor_value(&c, st), "a = 1\n");

        c.input(&Input::Preedit(String::new()), st, 1.2);
        c.input(&Input::Text("仮名".into()), st, 1.2);
        draw(&mut c, &mut text, st);
        let v = editor_value(&c, st);
        assert!(v.starts_with("仮名"), "{v:?}");
        let after = c.ime_cursor_area().unwrap();
        assert!(after.x > caret.x + 10.0, "{after:?} vs {caret:?}");
    });
}
