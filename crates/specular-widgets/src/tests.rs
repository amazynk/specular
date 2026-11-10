use crate::line_edit::LineEdit;
use crate::palette::fuzzy;
use crate::{Chrome, Modal};
use specular_input::Action;

#[test]
fn fuzzy_prefers_substrings_and_word_starts() {
    let sub = fuzzy("zoom in", "zoom").unwrap();
    let inner = fuzzy("unzoomed", "zoom").unwrap();
    let scattered = fuzzy("zero offset", "zo").unwrap();
    assert!(sub > inner);
    assert!(inner > scattered || scattered < 100.0);
    assert!(fuzzy("next image", "nxt").is_some());
    assert!(fuzzy("next image", "xyz").is_none());
    assert_eq!(fuzzy("anything", ""), Some(0.0));
}

#[test]
fn modals_toggle_and_back_closes_innermost() {
    let mut c = Chrome::new();
    assert!(c.handle_action(Action::Palette));
    assert_eq!(c.modal(), Some(Modal::Palette));
    assert!(c.handle_action(Action::Palette));
    assert_eq!(c.modal(), None);
    assert!(c.handle_action(Action::ToggleInfo));
    assert!(c.info_open());
    assert!(c.handle_action(Action::CheatSheet));
    assert!(c.back());
    assert_eq!(c.modal(), None);
    assert!(c.info_open());
    assert!(c.back());
    assert!(!c.info_open());
    assert!(!c.back());
}

#[test]
fn cheat_sheet_opens_on_the_current_mode() {
    use crate::SheetMode as SM;
    use specular_input::{Keymap, Mode};
    let mut c = Chrome::new();
    let mut km = Keymap::viewer_default();
    assert_eq!(c.cheat_context(&km), SM::Viewer);
    km.set_active(&[Mode::Medical]);
    assert_eq!(c.cheat_context(&km), SM::Medical);
    km.set_active(&[Mode::Cull, Mode::Medical]);
    assert_eq!(c.cheat_context(&km), SM::Cull);
    c.gallery = true;
    assert_eq!(c.cheat_context(&km), SM::Gallery);
    c.comparing = true;
    assert_eq!(c.cheat_context(&km), SM::Compare);
    // A tab picked wins until the sheet opens again.
    assert!(c.handle_action(Action::CheatSheet));
    c.cheat_pick(SM::Medical);
    assert_eq!(c.cheat_shown(&km), SM::Medical);
    assert!(c.back());
    assert!(c.handle_action(Action::CheatSheet));
    assert_eq!(c.cheat_shown(&km), SM::Compare);
    c.gallery = false;
    assert_eq!(c.cheat_shown(&km), SM::Cull);
}

/// Every cheat sheet label measured against its slot, on every tab, in every language, at the
/// standard text size and at 2×: group titles fit their column; a line that does not fit
/// its room beside the keys is cut with "…" and its tooltip is the whole line.
#[test]
fn cheat_sheet_labels_fit_or_say_it_all() {
    use crate::ctx::Target;
    use enabled_state::{STATES, with_state};
    use specular_gfx::{DrawList, Text, TextStyle};
    use specular_theme::{Palette, metrics};
    let mut text = Text::new();
    let langs: Vec<_> = specular_i18n::available_languages()
        .into_iter()
        .filter(|l| !l.pseudo)
        .collect();
    assert_eq!(langs.len(), 19);
    let (mut lines, mut cut) = (0usize, 0usize);
    let mut worst: Vec<String> = Vec::new();
    with_state(STATES[3], |st| {
        for l in &langs {
            specular_i18n::I18n::with_thread_language(l.id.clone(), || {
                for ts in [1.0f32, 2.0] {
                    for (w, h) in [(1280.0, 800.0), (1024.0, 768.0)] {
                        for m in crate::SheetMode::ALL {
                            let mut c = Chrome::new();
                            c.text_scale = ts;
                            c.set_size(w, h);
                            assert!(c.handle_action(Action::CheatSheet));
                            c.cheat_pick(m);
                            let mut dl = DrawList::new(1.0);
                            let pal = Palette::dark();
                            c.draw(&mut dl, &mut text, st, &pal, 1.0);
                            // A tab the profile hides is not shown.
                            if c.cheat_shown(st.keymap) != m {
                                continue;
                            }
                            assert!(!c.cheat_slots.is_empty(), "{} {m:?}: drawn", l.id);
                            for (slot, label, at) in &c.cheat_slots {
                                let Some((g, i)) = *at else {
                                    let hs = TextStyle::new(metrics::FONT_SMALL * ts).weight(650.0);
                                    let tw = text.measure(label, hs);
                                    assert!(
                                        tw <= slot.w + 0.5,
                                        "{} {ts}× {m:?}: title “{label}” {tw} > {}",
                                        l.id,
                                        slot.w
                                    );
                                    continue;
                                };
                                lines += 1;
                                let lw =
                                    text.measure(label, TextStyle::new(metrics::FONT_BODY * ts));
                                if lw <= slot.w {
                                    continue;
                                }
                                cut += 1;
                                if ts == 1.0 && w == 1280.0 && l.id.language.as_str() == "en" {
                                    worst.push(label.clone());
                                }
                                assert!(
                                    c.cheat_cut.contains(&(m, g, i)),
                                    "{label}: not marked cut"
                                );
                                let tip = Target::CheatLine(m, g as u16, i as u16).tooltip(st);
                                let tip = tip.expect("a tooltip");
                                let bare = tip.replace(['\u{2068}', '\u{2069}'], "");
                                let label_bare = label.replace(['\u{2068}', '\u{2069}'], "");
                                assert!(bare.contains(&label_bare), "{tip} / {label}");
                                // Hovered, it shows: the target is hit where the line is.
                                let tgt = Target::CheatLine(m, g as u16, i as u16);
                                let hit = c.hits.iter().find(|(_, t)| *t == tgt).map(|h| h.0);
                                if slot.y >= 0.0 && slot.bottom() <= h * 0.5 {
                                    assert!(hit.is_some(), "{label}: no hover target");
                                }
                                if let Some(r) = hit {
                                    assert_eq!(c.hit_at(r.center()), Some(tgt), "{label}");
                                }
                            }
                        }
                    }
                }
            });
        }
    });
    eprintln!(
        "cheat sheet: {lines} lines measured, {cut} cut with a full tooltip; English cut at 1× in 1280 px: {worst:?}"
    );
}

/// Every command in the palette says what it does: an English description that is a
/// sentence, not its name again; the palette entry offers it.
#[test]
fn every_palette_action_has_a_description() {
    let en = specular_i18n::Localizer::english();
    let mut missing = Vec::new();
    for a in Action::LISTED {
        let id = a.help_id();
        if !en.has_message(&id) {
            missing.push(id);
            continue;
        }
        let help = en.tr(&id);
        let name = a.name_with(&en);
        assert_ne!(help, name, "{id} repeats the name");
        assert!(help.ends_with('.'), "{id} is not a sentence: {help}");
        assert!(help.chars().count() <= 110, "{id} is long: {help}");
        let e = crate::palette::Entry {
            label: name,
            key: None,
            run: crate::palette::Run::Action(*a),
            disabled: None,
            unavailable: None,
            hidden: None,
        };
        assert!(e.help().is_some(), "{id}");
    }
    assert!(missing.is_empty(), "no description: {missing:?}");
}

#[test]
fn viewer_actions_pass_through() {
    let mut c = Chrome::new();
    assert!(!c.handle_action(Action::NextImage));
    assert!(!c.handle_action(Action::ZoomIn));
}

mod enabled_state;

mod tooltips;

mod scrollbars;

/// Scrolling past either end and then nudging the pointer must not move anything: input
/// clamps to the last layout's range, so no frame is drawn overscrolled and none "settles"
/// back later (the info panel used to jump back a row on the first pointer move).
mod overscroll;

/// Long info values (digests, hashes, unspaced names, prose) wrap inside the panel: every
/// glyph stays within the same margin on both sides, at the standard and the largest text.
#[test]
fn info_values_wrap_inside_the_panel() {
    use crate::scrollbar::Area;
    use enabled_state::{STATES, with_metadata};
    use specular_gfx::{DrawList, Text};
    use specular_metadata::{Field, FieldGroup, MetadataReport};
    use specular_theme::Palette;
    let digest = format!(
        "{};{}",
        (36864..36944)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(","),
        "A7F3C9E1B2D4F6A8".repeat(4)
    );
    let prose = "An ordinary sentence that wraps at its spaces like prose does. ".repeat(4);
    let field = |tag: &str, v: &str| Field {
        tag: tag.into(),
        raw: v.into(),
        display: v.into(),
        empty: false,
        full: None,
    };
    let meta = MetadataReport {
        all_fields: vec![FieldGroup {
            title: "XMP".into(),
            fields: vec![
                field("exif:NativeDigest", &digest),
                field(
                    "xmpMM:DocumentIDWithAVeryLongTagNameThatHasNoSpacesAtAll",
                    &"9f8e7d6c5b4a3928".repeat(8),
                ),
                field("dc:description", &prose),
            ],
        }],
        language: "en".into(),
        ..Default::default()
    };
    with_metadata(STATES[3], Some(&meta), |st| {
        let mut text = Text::new();
        for ts in [1.0, 2.0] {
            let mut c = crate::Chrome::new();
            c.text_scale = ts;
            c.set_size(900.0, 2400.0);
            c.set_info(true, true);
            let mut dl = DrawList::new(1.0);
            let pal = Palette::dark();
            for i in 0..60 {
                dl.clear(1.0);
                c.draw(&mut dl, &mut text, st, &pal, 1.0 + i as f64 * 0.016);
            }
            let v = c.bars.view(Area::Info);
            let glyphs: Vec<[f32; 4]> = dl
                .prims
                .iter()
                .filter(|p| {
                    // The panel body's glyphs: clipped to the body.
                    p.params[2] == specular_gfx::draw::kind::GLYPH
                        && p.clip[0] >= v.x - 0.5
                        && p.clip[1] >= v.y - 0.5
                        && p.clip[2] <= v.right() + 0.5
                        && p.clip[3] <= v.bottom() + 0.5
                })
                .map(|p| p.rect)
                .collect();
            assert!(glyphs.len() > digest.len(), "text {ts}: values drawn");
            for g in &glyphs {
                assert!(
                    g[0] >= v.x + 16.0 - 2.0 && g[2] <= v.right() - 16.0 + 2.0,
                    "text {ts}: glyph {g:?} outside the margins of {v:?}"
                );
            }
        }
    });
}

/// "Copy value" on an All-fields row hands the clipboard the whole value, not the shortened
/// text the panel shows; by pointer and through the accessibility tree alike.
#[test]
fn copy_value_copies_the_untruncated_value() {
    use crate::ctx::Target;
    use crate::{Command, Input};
    use accesskit::{Action as AkAction, ActionRequest, Role, TreeId};
    use enabled_state::{STATES, with_metadata};
    use specular_gfx::{DrawList, Text};
    use specular_metadata::{Field, FieldGroup, MetadataReport};
    use specular_theme::Palette;
    let whole = "0123456789".repeat(30);
    let shown = format!("{}…", &whole[..160]);
    let field = |tag: &str, display: &str, full: Option<&str>| Field {
        tag: tag.into(),
        raw: display.into(),
        display: display.into(),
        empty: display.is_empty(),
        full: full.map(str::to_owned),
    };
    let meta = MetadataReport {
        all_fields: vec![FieldGroup {
            title: "Exif".into(),
            fields: vec![
                field("Hidden", "", None),
                field("LongTag", &shown, Some(&whole)),
                field("Make", "Test", None),
            ],
        }],
        language: "en".into(),
        ..Default::default()
    };
    let click = |c: &mut crate::Chrome, st: &crate::ViewerState, x: f32, y: f32, t: f64| {
        let mut cmds = c.input(&Input::PointerMove { x, y }, st, t).commands;
        for down in [true, false] {
            let ev = Input::PointerButton {
                x,
                y,
                down,
                button: crate::Button::Primary,
            };
            cmds.extend(c.input(&ev, st, t).commands);
        }
        cmds
    };
    with_metadata(STATES[3], Some(&meta), |st| {
        let mut text = Text::new();
        for ts in [1.0, 2.0] {
            let mut c = crate::Chrome::new();
            c.text_scale = ts;
            c.set_size(1200.0, 900.0);
            c.set_info(true, true);
            let mut dl = DrawList::new(1.0);
            let pal = Palette::dark();
            let mut t = 1.0;
            for _ in 0..60 {
                t += 0.016;
                dl.clear(1.0);
                c.draw(&mut dl, &mut text, st, &pal, t);
            }
            // Indices are into the group's fields (the empty one is hidden, not renumbered).
            let row = |c: &crate::Chrome, f: usize| {
                c.hits
                    .iter()
                    .find(|(_, h)| *h == Target::InfoCopy(0, f))
                    .map(|(r, _)| *r)
            };
            assert!(row(&c, 0).is_none(), "text {ts}: hidden field has no row");
            let r = row(&c, 1).expect("long row is copyable");
            let cmds = click(&mut c, st, r.x + r.w * 0.5, r.y + r.h * 0.5, t);
            assert_eq!(cmds, vec![Command::CopyText(whole.clone())], "text {ts}");

            // The same through the accessibility tree: a named button on the info panel.
            let tree = c.accessibility_tree(st, "x", 1.0);
            let (id, n) = tree
                .nodes
                .iter()
                .find(|(_, n)| {
                    n.role() == Role::Button && n.label().is_some_and(|l| l.contains("LongTag"))
                })
                .expect("copy button in the tree");
            assert!(n.description().is_some_and(|d| d.contains("LongTag")));
            let r = c.accessibility_action(
                &ActionRequest {
                    action: AkAction::Click,
                    target_tree: TreeId::ROOT,
                    target_node: *id,
                    data: None,
                },
                st,
            );
            assert_eq!(r.commands, vec![Command::CopyText(whole.clone())]);

            // A value shown whole copies as shown.
            let r = row(&c, 2).expect("short row is copyable");
            let cmds = click(&mut c, st, r.x + 4.0, r.y + 4.0, t);
            assert_eq!(cmds, vec![Command::CopyText("Test".into())], "text {ts}");
        }
    });
}

/// The go-to field: digits in any script, Backspace, Enter goes (clamped to the folder),
/// Esc and a digit-free Enter go nowhere, the digit keys don't reach the viewer.
#[test]
fn go_to_image_field() {
    use crate::{Command, Input};
    use specular_input::{ActionState, ImageState, Key, KeyCombo, Mods, NamedKey};
    let key = |k| Input::Key {
        combo: KeyCombo::new(Key::Named(k), Mods::NONE),
        repeat: false,
    };
    let actions = ActionState {
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
    enabled_state::with_state(actions, |st| {
        let mut c = Chrome::new();
        assert!(c.handle_action(Action::GoToImage));
        assert!(c.wants_text());
        // The "1" key is "zoom to 100%" for the viewer; the field takes it.
        let one = Input::Key {
            combo: KeyCombo::new(Key::Char('1'), Mods::NONE),
            repeat: false,
        };
        assert!(c.input(&one, st, 0.0).consumed);
        c.input(&Input::Text("1x٢".into()), st, 0.0);
        assert_eq!(c.goto.as_ref().map(LineEdit::text), Some("12"));
        c.input(&key(NamedKey::Backspace), st, 0.0);
        c.input(&Input::Text("５".into()), st, 0.0);
        let r = c.input(&key(NamedKey::Enter), st, 0.0);
        assert!(matches!(r.commands[..], [Command::GoTo(14)]));
        assert!(!c.wants_text());
        // Past the end goes to the last image; a leading zero is dropped.
        c.handle_action(Action::GoToImage);
        c.input(&Input::Text("0999".into()), st, 0.0);
        let r = c.input(&key(NamedKey::Enter), st, 0.0);
        assert!(matches!(r.commands[..], [Command::GoTo(39)]));
        // Esc cancels.
        c.handle_action(Action::GoToImage);
        c.input(&Input::Text("3".into()), st, 0.0);
        assert!(c.back());
        assert!(c.goto.is_none());
    });
}

#[test]
fn list_toggle_keeps_order() {
    use crate::settings::toggle_item;
    let opts = ["a", "b", "c", "d"];
    let l = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(toggle_item(&opts, &l(&["d", "a"]), "a"), l(&["d"]));
    assert_eq!(
        toggle_item(&opts, &l(&["d", "a"]), "b"),
        l(&["d", "a", "b"])
    );
    assert_eq!(
        toggle_item(&opts, &l(&["c", "d"]), "a"),
        l(&["a", "c", "d"])
    );
    assert_eq!(
        toggle_item(&opts, &l(&["d", "b"]), "c"),
        l(&["d", "b", "c"])
    );
}

/// The pseudo-locales are offered only in developer mode, or while one is in use.
#[test]
fn pseudo_languages_need_developer_mode() {
    use crate::ViewerState;
    use specular_settings::Settings;
    enabled_state::with_state(enabled_state::STATES[0], |st| {
        let ids = |st: &ViewerState, current: &str| {
            let mut c = Chrome::new();
            c.language.rebuild(st, current);
            c.language.ids
        };
        let plain = ids(st, "system");
        assert!(plain.iter().any(|i| i == "de"));
        assert!(!plain.iter().any(|i| i == "en-XA" || i == "ar-XB"));
        let current = ids(st, "ar-XB");
        assert!(current.iter().any(|i| i == "ar-XB"));
        assert!(!current.iter().any(|i| i == "en-XA"));
        let mut dev = Settings::default();
        dev.advanced.developer_mode = true;
        let st = ViewerState {
            settings: &dev,
            ..*st
        };
        let all = ids(&st, "system");
        assert!(all.iter().any(|i| i == "en-XA") && all.iter().any(|i| i == "ar-XB"));
    });
}

/// The screen-reader tree (`a11y.rs`): built from the chrome as drawn.
mod accessibility;

/// Inspection tools: actions, keys, panel layout, value formatting and screen-reader text.
mod inspection;

/// The text field's placeholder: clipped to the field, never under the caret, hidden by text.
mod text_field;

/// Customising the toolbar: the list operations, the bar's layout, the list in Settings by
/// keyboard and pointer, and the ways to it (the bar's right-click menu, the palette).
mod toolbar_editing;

/// The toolbar list's row controls in Settings (Move up, Move down, the ⋯ menu), dragging
/// items on the real toolbar, holding Previous or Next (and its timings), and whether the
/// list previews the bar or the real one shows the changes.
mod bar_rearranging;

/// Input methods (IME) in the chrome's text fields, driven by the events the application
/// turns winit's `Ime::Preedit`, `Ime::Commit` and `Ime::Disabled` into (app.rs): a
/// composition is shown in the focused field, the candidate window is put at the caret (not
/// at the start of the field), cancelling leaves the text as it was, committing types the
/// result, and a composition never outlives its field.
mod ime;

/// Consecutive trees, as a screen reader's adapter applies them: AccessKit's consumer panics
/// (and takes the app down) on an update it cannot apply, which a single tree's structure
/// check does not show.
mod tree_updates;

/// Performance insights: the indicator comes once with a polite announcement, is in the
/// accessibility tree with a name, and its buttons and the panel's send the app's keys.
mod insights;

/// The keyboard path through the chrome (focus.rs): Tab and F6 between the toolbar, the info
/// panel and the indicator, arrows along the bar (mirrored right to left), Enter and Space
/// pressing (Previous and Next repeating while held), the More menu and the zoom presets by
/// keyboard, the insights panel's buttons, and the focus in the accessibility tree.
mod keyboard_focus;

/// Settings → Keys: the list by keyboard, the recorder, conflicts, search by key, and what
/// assistive technology is told.
mod key_bindings;

mod dicom_tags;

/// The studies browser (studies.rs). Synthetic rows; no real patient's data.
mod studies;

/// The page stepper of multi-image files (pages.rs).
mod page_stepper;

/// Profiles: what a profile that is off hides, and how its tools are turned on.
mod profiles;

#[test]
fn display_aids_toggle_step_and_follow_settings() {
    use specular_inspect::{ChannelColours, ColourMap, Cvd};
    use specular_settings::{ColourVision, FalseColourMap, Settings};
    let mut c = Chrome::new();
    let mut s = Settings::default();
    c.inspect.sync_settings(&s);
    assert_eq!(c.inspect.false_colour(), None);
    assert!(c.handle_action(Action::ToggleFalseColour));
    assert_eq!(c.inspect.false_colour(), Some(ColourMap::Viridis));
    assert!(c.handle_action(Action::ToggleFalseColour));
    // The next map turns false colour on.
    assert!(c.handle_action(Action::NextColourMap));
    assert_eq!(c.inspect.false_colour(), Some(ColourMap::Viridis.next()));
    assert!(c.handle_action(Action::NextChannelColours));
    assert_eq!(
        c.inspect.tools.channel_colours,
        ChannelColours::RedGreen.next()
    );
    assert!(c.handle_action(Action::ToggleHiLo));
    assert!(c.inspect.tools.hilo);
    // A simulation in particular, and again to turn it off; the next one steps.
    assert!(c.handle_action(Action::SimulateTritanopia));
    assert_eq!(c.inspect.tools.cvd, Cvd::Tritanopia);
    assert!(c.handle_action(Action::SimulateTritanopia));
    assert_eq!(c.inspect.tools.cvd, Cvd::Off);
    assert!(c.handle_action(Action::NextColourVision));
    assert_eq!(c.inspect.tools.cvd, Cvd::Off.next());
    // Unchanged settings keep what the keys chose; a changed setting wins.
    c.inspect.sync_settings(&s);
    assert_eq!(c.inspect.tools.cvd, Cvd::Off.next());
    s.inspection.colour_vision = ColourVision::Achromatopsia;
    s.inspection.false_colour_map = FalseColourMap::Batlow;
    c.inspect.sync_settings(&s);
    assert_eq!(c.inspect.tools.cvd, Cvd::Achromatopsia);
    assert_eq!(c.inspect.false_colour(), Some(ColourMap::Batlow));
}

/// The start screen, its "Things to try" cards, the first-run start choice and the hints.
mod welcome;

/// The help panel, the crash prompt and the diagnostics dialog.
mod help_panel;

mod decoder_card;
