//! Settings' text and path fields (setting_field.rs): editing in place, Enter and leaving
//! keep the text, Esc puts it back, refused text shows why; the pointer, the input method,
//! the clipboard, a path's Choose… and Clear, and screen readers.

use super::*;
use crate::accesskit::ActionData;
use specular_settings::{Origin, Value};

const SKIP: &str = "library.skip_folders";
const NAME: &str = "general.your_name";
const RENAME: &str = "import.rename_pattern";
const CACHE: &str = "gallery.cache_location";

fn field(k: &'static str) -> S {
    S::Field(k)
}

fn typed(c: &mut Chrome, st: &ViewerState, s: &str) {
    c.input(&Input::Text(s.into()), st, 2.0);
}

fn key(c: &mut Chrome, st: &ViewerState, k: Key, mods: Mods) -> Response {
    c.input(
        &Input::Key {
            combo: KeyCombo::new(k, mods),
            repeat: false,
        },
        st,
        2.0,
    )
}

fn text(st: &ViewerState, k: &str) -> String {
    match st.store.get(k).unwrap() {
        Value::Text(s) => s,
        v => panic!("{k}: {v:?}"),
    }
}

/// Settings at `k`'s section, drawn, the keyboard on its field.
fn open_at(c: &mut Chrome, tx: &mut Text, st: &ViewerState, k: &'static str) {
    c.set_accessibility(true);
    c.open_settings_at(k);
    draw(c, tx, st);
    tab_to(c, tx, st, &field(k));
}

/// The node of `k`'s field in the accessibility tree, and its id.
fn node(c: &Chrome, st: &ViewerState, k: &'static str) -> (NodeId, crate::accesskit::Node) {
    let t = c.accessibility_tree(st, "x", 1.0);
    let label = specular_settings::find(k)
        .unwrap()
        .label(specular_i18n::I18n::global());
    t.nodes
        .iter()
        .find(|(_, n)| n.role() == Role::TextInput && n.label() == Some(label.as_str()))
        .map(|(id, n)| (*id, n.clone()))
        .unwrap_or_else(|| panic!("{k}: no field in the tree"))
}

/// Tab reaches the Library's fields; what is typed goes to the focused one, not to the
/// search; Enter keeps it in settings.toml; Esc first puts back what was typed since, then
/// does what it does elsewhere.
#[test]
fn type_enter_keeps_escape_reverts() {
    let mut tx = Text::new();
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        open_at(&mut c, &mut tx, st, SKIP);
        let before = text(st, SKIP);
        assert!(!before.is_empty(), "the default list");
        // The caret starts at the end.
        typed(&mut c, st, ",photos");
        draw(&mut c, &mut tx, st);
        assert_eq!(c.settings.search, "", "typing went to the field");
        assert_eq!(text(st, SKIP), before, "not kept before Enter");
        let (_, n) = node(&c, st, SKIP);
        assert_eq!(n.value(), Some(format!("{before},photos").as_str()));
        assert_eq!(
            focused_node(&c.accessibility_tree(st, "x", 1.0)).role(),
            Role::TextInput
        );

        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(text(st, SKIP), format!("{before},photos"));
        assert!(st.store.is_modified(SKIP));
        assert_eq!(c.modal_focus, Some(Target::Settings(field(SKIP))), "stays");

        // Esc: what was typed since Enter goes; the stored value stays.
        typed(&mut c, st, ",x");
        assert!(press(&mut c, st, NamedKey::Escape, Mods::NONE).consumed);
        draw(&mut c, &mut tx, st);
        assert_eq!(
            node(&c, st, SKIP).1.value(),
            Some(format!("{before},photos").as_str())
        );
        assert_eq!(c.modal(), Some(crate::Modal::Settings));
        assert_eq!(text(st, SKIP), format!("{before},photos"));

        // The row's Reset still resets (the field follows).
        c.settings_click(S::Reset(SKIP), st);
        draw(&mut c, &mut tx, st);
        assert_eq!(text(st, SKIP), before);
        assert_eq!(node(&c, st, SKIP).1.value(), Some(before.as_str()));
    });
}

/// Tab and a click elsewhere keep the text (the keyboard left the field); Backspace and the
/// arrows edit the field, not the search; Ctrl+Z undoes in the field first.
#[test]
fn leaving_keeps_and_keys_edit_the_field() {
    let mut tx = Text::new();
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        open_at(&mut c, &mut tx, st, NAME);
        typed(&mut c, st, "Ana Lima");
        press(&mut c, st, NamedKey::Backspace, Mods::NONE);
        press(&mut c, st, NamedKey::ArrowLeft, Mods::NONE);
        press(&mut c, st, NamedKey::Home, Mods::NONE);
        typed(&mut c, st, "Dr ");
        assert_eq!(c.settings.search, "");
        let ctrl = Mods::CTRL;
        key(&mut c, st, Key::Char('z'), ctrl);
        draw(&mut c, &mut tx, st);
        assert_eq!(node(&c, st, NAME).1.value(), Some("Ana Lim"));
        tab(&mut c, st);
        draw(&mut c, &mut tx, st);
        assert_eq!(text(st, NAME), "Ana Lim", "kept on Tab");
        assert_ne!(c.modal_focus, Some(Target::Settings(field(NAME))));

        // A click on the field puts the keyboard there; a click elsewhere keeps the text.
        click_on(&mut c, st, &Target::Settings(field(NAME)));
        assert_eq!(c.modal_focus, Some(Target::Settings(field(NAME))));
        typed(&mut c, st, "a");
        draw(&mut c, &mut tx, st);
        click_on(&mut c, st, &Target::Settings(S::Section(0)));
        draw(&mut c, &mut tx, st);
        assert_eq!(text(st, NAME), "Ana Lima", "kept on leaving");
        // Closing Settings while editing keeps it too.
        c.open_settings_at(NAME);
        draw(&mut c, &mut tx, st);
        tab_to(&mut c, &mut tx, st, &field(NAME));
        typed(&mut c, st, "!");
        c.close_modal();
        c.input(&Input::PointerMove { x: 1.0, y: 1.0 }, st, 3.0);
        assert_eq!(text(st, NAME), "Ana Lima!");
    });
}

fn no_braces(s: &str) -> Result<(), String> {
    if s.contains('{') && !s.contains("{name}") {
        Err("Only {name} here.".into())
    } else {
        Ok(())
    }
}

/// A text the setting's check refuses stays in the field with the reason under it (also
/// for screen readers); the stored value is not touched. Esc puts the value back.
#[test]
fn refused_text_shows_why() {
    specular_settings::set_text_check(RENAME, no_braces);
    let mut tx = Text::new();
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        open_at(&mut c, &mut tx, st, RENAME);
        let field_r = hit(&c, &Target::Settings(field(RENAME)));
        typed(&mut c, st, "{bogus}");
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        draw(&mut c, &mut tx, st);
        assert_eq!(text(st, RENAME), "");
        let d = c.setting_draft(RENAME).expect("a draft");
        assert_eq!(d.error.as_deref(), Some("Only {name} here."));
        let (_, n) = node(&c, st, RENAME);
        assert_eq!(n.invalid(), Some(crate::accesskit::Invalid::True));
        assert_eq!(n.description(), Some("Only {name} here."));
        // The row grew for the reason; the field stayed where it was.
        assert_eq!(hit(&c, &Target::Settings(field(RENAME))).y, field_r.y);

        // Left: still there, refused.
        tab(&mut c, st);
        draw(&mut c, &mut tx, st);
        assert!(c.setting_draft(RENAME).is_some_and(|d| d.error.is_some()));
        assert_eq!(node(&c, st, RENAME).1.value(), Some("{bogus}"));
        // Back (Shift+Tab) and Esc: the value again, no reason.
        press(&mut c, st, NamedKey::Tab, Mods::SHIFT);
        assert_eq!(c.modal_focus, Some(Target::Settings(field(RENAME))));
        press(&mut c, st, NamedKey::Escape, Mods::NONE);
        draw(&mut c, &mut tx, st);
        assert_eq!(node(&c, st, RENAME).1.value(), Some(""));
        assert!(node(&c, st, RENAME).1.invalid().is_none());

        typed(&mut c, st, "{name}-trip");
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(text(st, RENAME), "{name}-trip");
        assert!(c.setting_draft(RENAME).unwrap().error.is_none());
    });
}

fn no_spaces(s: &str) -> Result<(), String> {
    if s.contains(' ') {
        Err("No spaces here.".into())
    } else {
        Ok(())
    }
}

/// Two fields refused: each keeps its own text and reason (the second does not replace the
/// first); putting one back or fixing it leaves the other as it was.
#[test]
fn every_refused_field_keeps_its_own_draft() {
    const FOLDER: &str = "import.folder_pattern";
    specular_settings::set_text_check(RENAME, no_braces);
    specular_settings::set_text_check(FOLDER, no_spaces);
    let mut tx = Text::new();
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        open_at(&mut c, &mut tx, st, RENAME);
        typed(&mut c, st, "{bogus}");
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        tab_to(&mut c, &mut tx, st, &field(FOLDER));
        press(&mut c, st, NamedKey::End, Mods::NONE);
        typed(&mut c, st, " x");
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        tab(&mut c, st);
        draw(&mut c, &mut tx, st);
        let reason = |c: &Chrome, k| c.setting_draft(k).and_then(|d| d.error.clone());
        assert_eq!(reason(&c, RENAME).as_deref(), Some("Only {name} here."));
        assert_eq!(reason(&c, FOLDER).as_deref(), Some("No spaces here."));
        for (k, why) in [(RENAME, "Only {name} here."), (FOLDER, "No spaces here.")] {
            let (_, n) = node(&c, st, k);
            assert_eq!(n.invalid(), Some(crate::accesskit::Invalid::True), "{k}");
            assert_eq!(n.description(), Some(why), "{k}");
        }
        assert_eq!(node(&c, st, RENAME).1.value(), Some("{bogus}"));
        assert!(node(&c, st, FOLDER).1.value().unwrap().ends_with(" x"));
        // Back on the first: its own text; Esc puts it back, the second stays refused.
        tab_to(&mut c, &mut tx, st, &field(RENAME));
        assert_eq!(c.settings.field.as_ref().unwrap().edit.text(), "{bogus}");
        press(&mut c, st, NamedKey::Escape, Mods::NONE);
        tab_to(&mut c, &mut tx, st, &field(FOLDER));
        draw(&mut c, &mut tx, st);
        assert!(reason(&c, RENAME).is_none());
        assert_eq!(node(&c, st, RENAME).1.value(), Some(""));
        assert_eq!(reason(&c, FOLDER).as_deref(), Some("No spaces here."));
        // Fixed: kept, no draft left anywhere.
        press(&mut c, st, NamedKey::Backspace, Mods::NONE);
        press(&mut c, st, NamedKey::Backspace, Mods::NONE);
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        tab(&mut c, st);
        draw(&mut c, &mut tx, st);
        assert_eq!(text(st, FOLDER), "{year}/{year}-{month}-{day}");
        assert!(reason(&c, FOLDER).is_none());
        assert!(c.settings.refused.is_empty());
    });
}

/// The input method composes in the focused field (its area in the field), and what it
/// commits is typed there; copy and paste go through the clipboard commands.
#[test]
fn compose_and_clipboard_in_a_field() {
    let mut tx = Text::new();
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        open_at(&mut c, &mut tx, st, NAME);
        let f = c.text_field().expect("the field is the focused one");
        assert_eq!(f, hit(&c, &Target::Settings(field(NAME))));
        c.input(&Input::Preedit("やまだ".into()), st, 2.0);
        draw(&mut c, &mut tx, st);
        assert_eq!(c.modal_focus, Some(Target::Settings(field(NAME))), "stays");
        let area = c.ime_cursor_area().unwrap();
        assert!(area.x >= f.x && area.right() <= f.right() + 0.5 && area.w > 10.0);
        // The input method's keys are its own.
        assert!(press(&mut c, st, NamedKey::Enter, Mods::NONE).consumed);
        c.input(&Input::Preedit(String::new()), st, 2.0);
        typed(&mut c, st, "山田");
        draw(&mut c, &mut tx, st);
        assert_eq!(node(&c, st, NAME).1.value(), Some("山田"));
        assert_eq!(c.settings.search, "");

        let ctrl = Mods::CTRL;
        key(&mut c, st, Key::Char('a'), ctrl);
        let r = key(&mut c, st, Key::Char('c'), ctrl);
        assert!(has_cmd(
            &r,
            |c| matches!(c, Command::EditorCopy(s) if s == "山田")
        ));
        let r = key(&mut c, st, Key::Char('v'), ctrl);
        assert!(has_cmd(&r, |c| matches!(c, Command::EditorPaste)));
        c.paste_text("Yamada\nTaro");
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(text(st, NAME), "Yamada Taro", "one line");
    });
}

/// A press in the field puts the caret where it was pressed; a drag selects.
#[test]
fn press_places_the_caret_and_drag_selects() {
    let mut tx = Text::new();
    with_image(|st| {
        st.store
            .set_text(NAME, "abcdefgh", Origin::SettingsWindow)
            .unwrap();
        let mut c = chrome(1000.0, false);
        c.set_accessibility(true);
        c.open_settings_at(NAME);
        draw(&mut c, &mut tx, st);
        let (_, g) = c
            .settings
            .field_geoms
            .iter()
            .find(|(k, _)| *k == NAME)
            .cloned()
            .unwrap();
        let (b, x0) = g.text.clone().unwrap();
        let y = g.rect.center().1;
        let x_at = |i: usize| x0 + b.caret_x(i);
        let down = |c: &mut Chrome, x: f32, d: bool| {
            c.input(
                &Input::PointerButton {
                    x,
                    y,
                    down: d,
                    button: Button::Primary,
                },
                st,
                2.0,
            )
        };
        down(&mut c, x_at(3), true);
        assert_eq!(c.modal_focus, Some(Target::Settings(field(NAME))));
        c.input(&Input::PointerMove { x: x_at(6), y }, st, 2.0);
        down(&mut c, x_at(6), false);
        draw(&mut c, &mut tx, st);
        assert_eq!(c.setting_draft(NAME).unwrap().edit.selection(), 3..6);
        typed(&mut c, st, "X");
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(text(st, NAME), "abcXgh");
    });
}

/// A path: its field, Choose… (asks the application for the platform's folder dialog) and,
/// once set, Clear (back to unset); typing a path keeps it too.
#[test]
fn path_choose_and_clear() {
    let mut tx = Text::new();
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        c.set_accessibility(true);
        c.open_settings_at(CACHE);
        draw(&mut c, &mut tx, st);
        assert!(
            c.settings_stops()
                .contains(&Target::Settings(S::PathChoose(CACHE)))
        );
        assert!(
            !c.settings_stops()
                .contains(&Target::Settings(S::PathClear(CACHE)))
        );
        tab_to(&mut c, &mut tx, st, &S::PathChoose(CACHE));
        let r = press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert!(has_cmd(&r, |c| matches!(
            c,
            Command::ChooseSettingPath {
                key: CACHE,
                directory: true
            }
        )));
        // The application stores what was picked: the field shows it, Clear is offered.
        let dir = std::path::PathBuf::from("/nowhere/thumbs");
        st.store
            .set(
                CACHE,
                Value::Path(Some(dir.clone())),
                Origin::SettingsWindow,
            )
            .unwrap();
        draw(&mut c, &mut tx, st);
        assert_eq!(node(&c, st, CACHE).1.value(), Some(dir.to_str().unwrap()));
        tab_to(&mut c, &mut tx, st, &S::PathClear(CACHE));
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        draw(&mut c, &mut tx, st);
        assert_eq!(st.store.get(CACHE).unwrap(), Value::Path(None));
        assert_eq!(c.modal_focus, Some(Target::Settings(field(CACHE))));
        // Unset: the placeholder says what that means.
        let (_, n) = node(&c, st, CACHE);
        assert_eq!(n.value(), Some(""));
        assert!(n.placeholder().is_some_and(|p| !p.is_empty()));
        typed(&mut c, st, "  /elsewhere  ");
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(
            st.store.get(CACHE).unwrap(),
            Value::Path(Some("/elsewhere".into()))
        );
    });
}

/// A colour: a swatch sets it in one click (a radio button for screen readers, the one in
/// use checked); its field takes `#RRGGBB` and refuses what is not a colour.
#[test]
fn colour_swatches_and_hex_field() {
    const SUB: &str = "viewing.subtitle_custom_color";
    let colour = |st: &ViewerState| match st.store.get(SUB).unwrap() {
        Value::Color(c) => [c.r, c.g, c.b, c.a],
        v => panic!("{v:?}"),
    };
    let mut tx = Text::new();
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        c.set_accessibility(true);
        c.open_settings_at(SUB);
        draw(&mut c, &mut tx, st);
        let yellow = S::Swatch(SUB, 5);
        assert!(
            c.settings_stops()
                .contains(&Target::Settings(yellow.clone()))
        );
        tab_to(&mut c, &mut tx, st, &yellow);
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        draw(&mut c, &mut tx, st);
        assert_eq!(colour(st), [0xFF, 0xFF, 0x00, 0xFF]);
        let t = c.accessibility_tree(st, "x", 1.0);
        let n = focused_node(&t);
        assert_eq!(n.role(), Role::RadioButton);
        assert_eq!(n.toggled(), Some(crate::accesskit::Toggled::True));
        assert!(
            n.label().is_some_and(|l| l.starts_with("Yellow")),
            "{:?}",
            n.label()
        );
        assert_eq!(node(&c, st, SUB).1.value(), Some("#FFFF00"));
        // The field: the hex text, kept on Enter.
        tab_to(&mut c, &mut tx, st, &field(SUB));
        for _ in 0.."FFFF00".len() {
            press(&mut c, st, NamedKey::Backspace, Mods::NONE);
        }
        typed(&mut c, st, "00ff80");
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(colour(st), [0x00, 0xFF, 0x80, 0xFF]);
        // Not a colour: refused, the colour stays.
        typed(&mut c, st, "zz");
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        draw(&mut c, &mut tx, st);
        assert_eq!(colour(st), [0x00, 0xFF, 0x80, 0xFF]);
        assert!(c.setting_draft(SUB).is_some_and(|d| d.error.is_some()));
        // A click on a swatch puts the field back to what it sets.
        c.settings_click(S::Swatch(SUB, 0), st);
        draw(&mut c, &mut tx, st);
        assert_eq!(colour(st), [0xFF; 4]);
        assert!(c.setting_draft(SUB).is_none_or(|d| d.error.is_none()));
        assert_eq!(node(&c, st, SUB).1.value(), Some("#FFFFFF"));
    });
}

/// Screen readers: SetValue keeps the value at once, ReplaceSelectedText edits,
/// SetTextSelection selects; each puts the keyboard on the field.
#[test]
fn screen_reader_edits_a_field() {
    let mut tx = Text::new();
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        c.set_accessibility(true);
        c.open_settings_at(NAME);
        draw(&mut c, &mut tx, st);
        let (id, n) = node(&c, st, NAME);
        for a in [
            AkAction::SetValue,
            AkAction::ReplaceSelectedText,
            AkAction::SetTextSelection,
            AkAction::Focus,
        ] {
            assert!(n.supports_action(a), "{a:?}");
        }
        let req = |action, data| ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: id,
            data: Some(data),
        };
        c.accessibility_action(
            &req(AkAction::SetValue, ActionData::Value("Kim".into())),
            st,
        );
        assert_eq!(text(st, NAME), "Kim");
        assert_eq!(c.modal_focus, Some(Target::Settings(field(NAME))));
        draw(&mut c, &mut tx, st);
        let sel = |a, f| {
            let run = c.accessibility_tree(st, "x", 1.0);
            let (rid, _) = run
                .nodes
                .iter()
                .find(|(_, n)| n.role() == Role::TextRun && n.value() == Some("Kim"))
                .cloned()
                .expect("the field's text run");
            ActionData::SetTextSelection(crate::accesskit::TextSelection {
                anchor: crate::accesskit::TextPosition {
                    node: rid,
                    character_index: a,
                },
                focus: crate::accesskit::TextPosition {
                    node: rid,
                    character_index: f,
                },
            })
        };
        let s = sel(0, 1);
        c.accessibility_action(&req(AkAction::SetTextSelection, s), st);
        c.accessibility_action(
            &req(AkAction::ReplaceSelectedText, ActionData::Value("T".into())),
            st,
        );
        draw(&mut c, &mut tx, st);
        assert_eq!(node(&c, st, NAME).1.value(), Some("Tim"));
        assert_eq!(text(st, NAME), "Kim", "kept on Enter or leaving");
        press(&mut c, st, NamedKey::Enter, Mods::NONE);
        assert_eq!(text(st, NAME), "Tim");
    });
}

/// Right to left: the field and its buttons are mirrored, and a long value scrolls to keep
/// the caret in view.
#[test]
fn right_to_left_and_long_values() {
    let mut tx = Text::new();
    with_image(|st| {
        let long = "/very/long/path/".repeat(12);
        st.store
            .set(
                CACHE,
                Value::Path(Some(long.clone().into())),
                Origin::SettingsWindow,
            )
            .unwrap();
        let mut c = chrome(1000.0, true);
        c.set_accessibility(true);
        c.open_settings_at(CACHE);
        draw(&mut c, &mut tx, st);
        let f = hit(&c, &Target::Settings(field(CACHE)));
        let choose = hit(&c, &Target::Settings(S::PathChoose(CACHE)));
        let clear = hit(&c, &Target::Settings(S::PathClear(CACHE)));
        assert!(
            clear.right() <= choose.x + 0.5 && choose.right() <= f.x + 0.5,
            "mirrored"
        );
        tab_to(&mut c, &mut tx, st, &field(CACHE));
        for k in [NamedKey::End, NamedKey::Home] {
            press(&mut c, st, k, Mods::NONE);
            draw(&mut c, &mut tx, st);
            let area = c.ime_cursor_area().unwrap();
            assert!(
                area.x >= f.x - 0.5 && area.right() <= f.right() + 0.5,
                "{k:?}: {area:?} in {f:?}"
            );
        }
    });
}

/// Settings' own records (hints given, folders used last, hosts agreed to, …) and the
/// settings other panels show never get a field.
#[test]
fn internal_settings_have_no_field() {
    let mut tx = Text::new();
    with_image(|st| {
        let mut c = chrome(1000.0, false);
        c.open_modal(crate::Modal::Settings);
        let mut shown = Vec::new();
        for i in 0..c.sections().len() {
            c.settings.section = i;
            c.settings.scroll = 0.0;
            draw(&mut c, &mut tx, st);
            shown.extend(c.settings.stops.iter().filter_map(|s| match s.t {
                Target::Settings(S::Field(k)) => Some(k),
                _ => None,
            }));
        }
        for k in [
            "general.tips_shown",
            "profile.shown",
            "browsing.recent_folders",
            "viewing.sound_visuals",
            "maps.services_allowed",
            "metadata.basemap_allowed",
            "metadata.basemap_service",
            "integration.extra_default_types",
            "decoders.braw_path",
            "decoders.dwg_path",
            "decoders.occt_path",
            "decoders.czi_path",
            "colour.proof_profile",
            "colour.proof_intent",
            "colour.proof_bpc",
            "colour.proof_paper",
            "colour.proof_ink",
        ] {
            assert!(!shown.contains(&k), "{k} has a field");
        }
        for k in [
            SKIP,
            "library.skip_marked",
            NAME,
            RENAME,
            CACHE,
            "metadata.basemap_url",
        ] {
            assert!(shown.contains(&k), "{k} has no field");
        }
    });
}
