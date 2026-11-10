//! Integration tests for specular-settings. Temporary files live under CARGO_TARGET_TMPDIR.
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use specular_i18n::{I18n, langid};
use specular_settings::*;

fn scratch(name: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "specular-settings-{name}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const USER_FILE: &str = r#"# My Specular settings
version = 5

# How images open
[viewing]
initial_zoom = "fit"   # I like it big
zoom_animation = false

# Grey, please
background = "dark_grey"

[browsing]
loop_folder = true
"#;

// ----- Round trip ---------------------------------------------------------

#[test]
fn round_trip_keeps_comments_and_order() {
    let dir = scratch("roundtrip");
    let path = dir.join("settings.toml");
    std::fs::write(&path, USER_FILE).unwrap();

    let store = SettingsStore::open(&path).unwrap();
    assert!(store.warnings().is_empty(), "{:?}", store.warnings());
    let s = store.snapshot();
    assert_eq!(s.viewing.initial_zoom, InitialZoom::Fit);
    assert!(!s.viewing.zoom_animation);
    assert_eq!(s.viewing.background, Background::DarkGrey);
    assert!(s.browsing.loop_folder);
    assert_eq!(
        s.colour.rendering_intent,
        RenderingIntent::RelativeColorimetric
    );

    // Nothing changed → nothing written.
    store.save().unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), USER_FILE);

    store
        .set(
            "viewing.initial_zoom",
            Value::Text("actual_size".into()),
            Origin::SettingsWindow,
        )
        .unwrap();
    store
        .set(
            "colour.dithering",
            Value::Bool(false),
            Origin::SettingsWindow,
        )
        .unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let expected = USER_FILE.replace(
        r#"initial_zoom = "fit"   # I like it big"#,
        r#"initial_zoom = "actual_size"   # I like it big"#,
    ) + "\n[colour]\ndithering = false\n";
    assert_eq!(text, expected);

    // Reopening gives the same values.
    let again = SettingsStore::open(&path).unwrap();
    assert_eq!(*again.snapshot(), *store.snapshot());
    assert!(again.warnings().is_empty());
}

#[test]
fn profile_round_trips() {
    use ProfileCapability as P;
    let dir = scratch("profile");
    let path = dir.join("settings.toml");
    let store = SettingsStore::open(&path).unwrap();
    // General only, and nothing offered yet: the first run asks.
    assert_eq!(store.snapshot().profile.capabilities, [P::General]);
    assert_eq!(store.snapshot().profile.shown, "");
    store
        .set(
            "profile.capabilities",
            Value::List(vec!["general".into(), "medical".into(), "science".into()]),
            Origin::SettingsWindow,
        )
        .unwrap();
    store
        .set(
            "profile.shown",
            Value::Text("question".into()),
            Origin::SettingsWindow,
        )
        .unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("[profile]")
            && text.contains(r#"capabilities = ["general", "medical", "science"]"#)
            && text.contains(r#"shown = "question""#),
        "{text}"
    );
    let again = SettingsStore::open(&path).unwrap();
    assert!(again.warnings().is_empty(), "{:?}", again.warnings());
    assert_eq!(
        again.snapshot().profile.capabilities,
        [P::General, P::Medical, P::Science]
    );
    assert_eq!(*again.snapshot(), *store.snapshot());
    // An unknown profile is refused.
    assert!(parse_set_arg("profile.capabilities=general,astrology").is_err());
}

#[test]
fn toolbar_spacers_and_separators_round_trip() {
    use ToolbarItem as T;
    let dir = scratch("toolbar");
    let path = dir.join("settings.toml");
    // A file from before spacers and separators existed still loads as written.
    std::fs::write(
        &path,
        "version = 5\n\n[viewing]\ntoolbar_items = [\"prev\", \"next\", \"info\"]\n",
    )
    .unwrap();
    let store = SettingsStore::open(&path).unwrap();
    assert!(store.warnings().is_empty(), "{:?}", store.warnings());
    assert_eq!(
        store.snapshot().viewing.toolbar_items,
        [T::Prev, T::Next, T::Info]
    );

    // Spacers and separators may repeat; buttons still appear once.
    let items = vec![
        T::Prev,
        T::Separator,
        T::Next,
        T::Spacer,
        T::Info,
        T::Spacer,
        T::Separator,
        T::Settings,
    ];
    let mut with_twice: Vec<String> = items.iter().map(|i| i.to_string()).collect();
    with_twice.push("info".into());
    store
        .set(
            "viewing.toolbar_items",
            Value::List(with_twice),
            Origin::SettingsWindow,
        )
        .unwrap();
    assert_eq!(store.snapshot().viewing.toolbar_items, items);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains(
            r#"toolbar_items = ["prev", "separator", "next", "spacer", "info", "spacer", "separator", "settings"]"#
        ),
        "{text}"
    );
    let again = SettingsStore::open(&path).unwrap();
    assert!(again.warnings().is_empty(), "{:?}", again.warnings());
    assert_eq!(*again.snapshot(), *store.snapshot());
    assert_eq!(
        parse_set_arg("viewing.toolbar_items=prev,spacer,spacer,next")
            .unwrap()
            .1,
        Value::List(vec![
            "prev".into(),
            "spacer".into(),
            "spacer".into(),
            "next".into()
        ])
    );
}

#[test]
fn defaults_are_not_written_but_existing_keys_are_updated() {
    let dir = scratch("defaults");
    let path = dir.join("settings.toml");
    let store = SettingsStore::open(&path).unwrap();
    assert!(!path.exists(), "opening must not create the file");
    // Setting a value equal to the default changes nothing and writes nothing.
    assert!(
        !store
            .set(
                "browsing.loop_folder",
                Value::Bool(false),
                Origin::SettingsWindow
            )
            .unwrap()
    );
    assert!(!path.exists());
    store
        .set(
            "browsing.loop_folder",
            Value::Bool(true),
            Origin::SettingsWindow,
        )
        .unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with("# Specular settings."), "{text}");
    assert!(text.contains(&format!("version = {SCHEMA_VERSION}")));
    assert!(text.contains("[browsing]\nloop_folder = true"));
    // Back to default: the key is kept (with value false), not removed.
    store
        .set(
            "browsing.loop_folder",
            Value::Bool(false),
            Origin::SettingsWindow,
        )
        .unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("loop_folder = false"), "{text}");
}

/// Sharper raw thumbnails: on by default, a plain toggle in the Gallery section with its
/// label and help, and turned off from the window, `--set` or the file.
#[test]
fn sharper_raw_thumbnails_is_a_gallery_toggle_on_by_default() {
    let meta = find("gallery.sharper_raw_thumbnails").expect("the setting");
    assert_eq!(meta.section, Section::Gallery);
    assert_eq!(meta.kind, Kind::Bool);
    assert_eq!(meta.default, Value::Bool(true));
    assert!(meta.stored && !meta.needs_restart && meta.os.is_empty());
    assert_eq!(meta.label_id, "settings-gallery-sharper-raw-thumbnails");
    let i18n = I18n::for_language(langid!("en"));
    assert_eq!(meta.label(&i18n), "Sharper raw thumbnails");
    assert!(meta.help(&i18n).contains("1024"));
    assert!(Settings::default().gallery.sharper_raw_thumbnails);

    let store = SettingsStore::in_memory();
    assert!(store.snapshot().gallery.sharper_raw_thumbnails);
    store
        .set_text(
            "gallery.sharper_raw_thumbnails",
            "false",
            Origin::SettingsWindow,
        )
        .unwrap();
    assert!(!store.snapshot().gallery.sharper_raw_thumbnails);

    let dir = scratch("sharper-raw");
    let path = dir.join("settings.toml");
    std::fs::write(&path, "[gallery]\nsharper_raw_thumbnails = false\n").unwrap();
    let store = SettingsStore::open(&path).unwrap();
    assert!(!store.snapshot().gallery.sharper_raw_thumbnails);
    assert!(store.warnings().is_empty(), "{:?}", store.warnings());
}

/// Preview while dragging: Automatic by default, three options with labels and help in the
/// Performance section, set from the window or the file.
#[test]
fn drag_preview_is_a_performance_choice_automatic_by_default() {
    let meta = find("performance.drag_preview").expect("the setting");
    assert_eq!(meta.section, Section::Performance);
    assert_eq!(meta.options(), &["automatic", "sharp", "fast"]);
    assert_eq!(meta.default, Value::Text("automatic".into()));
    assert!(meta.stored && !meta.needs_restart && meta.os.is_empty());
    assert_eq!(meta.label_id, "settings-performance-drag-preview");
    let i18n = I18n::for_language(langid!("en"));
    assert_eq!(meta.label(&i18n), "Preview while dragging");
    assert!(meta.help(&i18n).contains("Develop"));
    for (o, l) in [
        ("automatic", "Automatic"),
        ("sharp", "Always sharp"),
        ("fast", "Always fast"),
    ] {
        assert_eq!(i18n.tr(&meta.option_id(o)), l);
    }
    assert_eq!(
        Settings::default().performance.drag_preview,
        DragPreview::Automatic
    );
    let store = SettingsStore::in_memory();
    store
        .set_text("performance.drag_preview", "sharp", Origin::SettingsWindow)
        .unwrap();
    assert_eq!(
        store.snapshot().performance.drag_preview,
        DragPreview::Sharp
    );
    assert!(
        store
            .set_text("performance.drag_preview", "smooth", Origin::SettingsWindow)
            .is_err()
    );

    let dir = scratch("drag-preview");
    let path = dir.join("settings.toml");
    std::fs::write(&path, "[performance]\ndrag_preview = \"fast\"\n").unwrap();
    let store = SettingsStore::open(&path).unwrap();
    assert_eq!(store.snapshot().performance.drag_preview, DragPreview::Fast);
    assert!(store.warnings().is_empty(), "{:?}", store.warnings());
}

// ----- Invalid values and unknown keys ------------------------------------------

#[test]
fn invalid_values_warn_and_use_defaults_without_touching_the_file() {
    let dir = scratch("invalid");
    let path = dir.join("settings.toml");
    let text = r##"
[viewing]
initial_zoom = "gigantic"
pixel_grid_threshold = 50
toolbar_autohide_delay = "soon"
background_color = "#12345"
zoom_animation = true

[accessibility]
text_scale = 9.5

[advanced]
portable_mode = true
"##;
    std::fs::write(&path, text).unwrap();
    let store = SettingsStore::open(&path).unwrap();
    let w = store.warnings();
    let reason = |key: &str| {
        w.iter()
            .find_map(|w| match w {
                Warning::InvalidValue { key: k, reason } if k == key => Some(reason.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no warning for {key}: {w:?}"))
    };
    assert!(matches!(
        reason("viewing.initial_zoom"),
        SettingError::UnknownOption { .. }
    ));
    assert_eq!(
        reason("viewing.pixel_grid_threshold"),
        SettingError::OutOfRange {
            min: "200".into(),
            max: "6400".into(),
            auto: false
        }
    );
    assert_eq!(
        reason("viewing.toolbar_autohide_delay"),
        SettingError::NotNumber
    );
    assert_eq!(reason("viewing.background_color"), SettingError::BadColour);
    assert!(matches!(
        reason("accessibility.text_scale"),
        SettingError::OutOfRange { .. }
    ));
    assert_eq!(reason("advanced.portable_mode"), SettingError::NotSettable);
    assert_eq!(w.len(), 6, "{w:?}");

    let s = store.snapshot();
    let d = Settings::default();
    assert_eq!(s.viewing.initial_zoom, d.viewing.initial_zoom);
    assert_eq!(s.viewing.pixel_grid_threshold, 800);
    assert_eq!(s.accessibility.text_scale, 1.0);
    assert!(s.viewing.zoom_animation);

    // Messages name the key and the reason, in English and German.
    let msg = w[0].to_string();
    assert!(msg.contains("viewing.initial_zoom"), "{msg}");
    let de = I18n::for_language(langid!("de"));
    let range = w
        .iter()
        .find(|w| w.key() == Some("viewing.pixel_grid_threshold"))
        .unwrap();
    let de_msg = range.message(&de);
    assert!(de_msg.contains("viewing.pixel_grid_threshold"), "{de_msg}");
    assert!(
        de_msg.contains("200") && de_msg.contains("6400"),
        "{de_msg}"
    );
    assert_ne!(de_msg, range.message(&I18n::for_language(langid!("en"))));

    // Saving an unrelated change keeps the invalid lines as they were.
    store
        .set(
            "browsing.loop_folder",
            Value::Bool(true),
            Origin::SettingsWindow,
        )
        .unwrap();
    let after = std::fs::read_to_string(&path).unwrap();
    assert!(after.starts_with(text), "{after}");
}

#[test]
fn unknown_keys_are_kept_and_reported() {
    let dir = scratch("unknown");
    let path = dir.join("settings.toml");
    let text = r#"version = 5
future_flag = 3

[viewing]
initial_zoom = "fill"
sparkles = "lots" # from a newer version

[plugins]
enabled = ["x"]
"#;
    std::fs::write(&path, text).unwrap();
    let store = SettingsStore::open(&path).unwrap();
    let keys: Vec<_> = store
        .warnings()
        .iter()
        .filter_map(|w| match w {
            Warning::UnknownKey { key } => Some(key.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(keys, ["future_flag", "viewing.sparkles", "plugins"]);
    assert_eq!(store.snapshot().viewing.initial_zoom, InitialZoom::Fill);

    store
        .set(
            "viewing.initial_zoom",
            Value::Text("fit".into()),
            Origin::Palette,
        )
        .unwrap();
    let after = std::fs::read_to_string(&path).unwrap();
    assert_eq!(after, text.replace(r#""fill""#, r#""fit""#));
}

#[test]
fn syntax_errors_never_destroy_the_file() {
    let dir = scratch("syntax");
    let path = dir.join("settings.toml");
    let text = "[viewing]\ninitial_zoom = \"fit\"\nbroken = = 3\n";
    std::fs::write(&path, text).unwrap();
    let store = SettingsStore::open(&path).unwrap();
    assert!(matches!(
        store.warnings()[..],
        [Warning::Syntax { line: 3, .. }]
    ));
    assert_eq!(*store.snapshot(), Settings::default());
    // Changes still apply in memory, but the file is not overwritten.
    store
        .set(
            "browsing.loop_folder",
            Value::Bool(true),
            Origin::SettingsWindow,
        )
        .unwrap();
    assert!(store.snapshot().browsing.loop_folder);
    assert!(matches!(
        store.save(),
        Err(SettingsError::FileHasErrors { .. })
    ));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);

    // Once the user fixes the file, a reload picks it up.
    std::fs::write(&path, "[viewing]\ninitial_zoom = \"fill\"\n").unwrap();
    assert!(store.reload().unwrap());
    assert_eq!(store.snapshot().viewing.initial_zoom, InitialZoom::Fill);
    assert!(store.warnings().is_empty());
}

#[test]
fn newer_version_is_reported() {
    let l = load_str("version = 7\n[viewing]\ninitial_zoom = \"fill\"\n");
    assert_eq!(
        l.warnings,
        [Warning::NewerVersion {
            found: 7,
            supported: SCHEMA_VERSION
        }]
    );
    assert_eq!(l.settings.viewing.initial_zoom, InitialZoom::Fill);
    assert!(!l.migrated);
}

// ----- Migration ------------------------------------------------------------

fn rename_zoom_key(doc: &mut toml_edit::DocumentMut) {
    // Version 1 called it `view.zoom_on_open` with values "window" / "pixels".
    let Some(old) = doc.get_mut("view").and_then(|t| t.as_table_like_mut()) else {
        return;
    };
    let value = old.remove("zoom_on_open");
    if old.is_empty() {
        doc.remove("view");
    }
    if let Some(v) = value.and_then(|v| v.as_str().map(str::to_owned)) {
        let new = match v.as_str() {
            "window" => "fit",
            "pixels" => "actual_size",
            other => other,
        }
        .to_owned();
        if !doc.contains_key("viewing") {
            doc["viewing"] = toml_edit::table();
        }
        doc["viewing"]["initial_zoom"] = toml_edit::value(new);
    }
}

#[test]
fn migration_hook_upgrades_old_files() {
    static MIGRATIONS_V2: &[Migration] = &[Migration {
        from: 1,
        migrate: rename_zoom_key,
    }];
    let old = "# mine\nversion = 1\n\n[view]\nzoom_on_open = \"pixels\"\n\n[browsing]\n# keep me\nloop_folder = true\n";
    let l = load_str_with(old, MIGRATIONS_V2, 2);
    assert!(l.migrated);
    assert!(l.warnings.is_empty(), "{:?}", l.warnings);
    assert_eq!(l.settings.viewing.initial_zoom, InitialZoom::ActualSize);
    assert!(l.settings.browsing.loop_folder);
    let doc = l.doc.unwrap().to_string();
    assert!(doc.contains("version = 2"), "{doc}");
    assert!(doc.starts_with("# mine\nversion = 2\n"), "{doc}");
    assert!(
        doc.contains("[viewing]\ninitial_zoom = \"actual_size\""),
        "{doc}"
    );
    assert!(doc.contains("# mine") && doc.contains("# keep me"), "{doc}");
    assert!(!doc.contains("zoom_on_open"), "{doc}");

    // Already current: nothing to do.
    let l = load_str_with("version = 2\n", MIGRATIONS_V2, 2);
    assert!(!l.migrated);
}

// ----- --set ----------------------------------------------------------------

#[test]
fn set_args() {
    let (m, v) = parse_set_arg("viewing.initial_zoom=fit").unwrap();
    assert_eq!(m.key, "viewing.initial_zoom");
    assert_eq!(v, Value::Text("fit".into()));
    assert_eq!(
        parse_set_arg("viewing.initial_zoom = Fit-If-Larger")
            .unwrap()
            .1,
        Value::Text("fit_if_larger".into())
    );
    assert_eq!(
        parse_set_arg("viewing.zoom_animation=off").unwrap().1,
        Value::Bool(false)
    );
    assert_eq!(
        parse_set_arg("performance.ram_cache=auto").unwrap().1,
        Value::Int(0)
    );
    assert_eq!(
        parse_set_arg("performance.ram_cache=2048").unwrap().1,
        Value::Int(2048)
    );
    assert_eq!(
        parse_set_arg("accessibility.text_scale=1.25").unwrap().1,
        Value::Float(1.25)
    );
    assert_eq!(
        parse_set_arg("viewing.background_color=\"#336699\"")
            .unwrap()
            .1,
        Value::Color(Color::rgb(0x33, 0x66, 0x99))
    );
    assert_eq!(
        parse_set_arg("general.language=de").unwrap().1,
        Value::Text("de".into())
    );
    assert_eq!(
        parse_set_arg("general.region_formats=en-GB").unwrap().1,
        Value::Text("en-GB".into())
    );
    assert_eq!(
        parse_set_arg("viewing.toolbar_items=prev, next,info")
            .unwrap()
            .1,
        Value::List(vec!["prev".into(), "next".into(), "info".into()])
    );
    assert_eq!(
        parse_set_arg("colour.display_profile=").unwrap().1,
        Value::Path(None)
    );

    assert_eq!(
        parse_set_arg("viewing.initial_zoom").unwrap_err(),
        SettingError::Syntax
    );
    assert_eq!(parse_set_arg("=3").unwrap_err(), SettingError::Syntax);
    assert_eq!(
        parse_set_arg("viewing.zoom_animation=maybe").unwrap_err(),
        SettingError::NotBool
    );
    assert!(matches!(
        parse_set_arg("performance.prefetch_depth=99").unwrap_err(),
        SettingError::OutOfRange { .. }
    ));
    assert_eq!(
        parse_set_arg("advanced.reset_all=1").unwrap_err(),
        SettingError::NotSettable
    );

    // Suggestions for typos and missing sections.
    let sugg = |arg: &str| match parse_set_arg(arg).unwrap_err() {
        SettingError::UnknownKey { suggestion, .. } => suggestion,
        e => panic!("{e:?}"),
    };
    assert_eq!(sugg("initial_zoom=fit"), Some("viewing.initial_zoom"));
    assert_eq!(
        sugg("viewing.inital_zoom=fit"),
        Some("viewing.initial_zoom")
    );
    assert_eq!(
        sugg("browsing.loop-folder=on"),
        Some("browsing.loop_folder")
    );
    assert_eq!(sugg("colour.dithring=on"), Some("colour.dithering"));
    assert_eq!(sugg("completely.unrelated=1"), None);
    let e = parse_set_arg("viewing.inital_zoom=fit").unwrap_err();
    assert!(e.to_string().contains("viewing.initial_zoom"), "{e}");
}

#[test]
fn set_args_apply_for_the_session_only() {
    let dir = scratch("cli");
    let path = dir.join("settings.toml");
    std::fs::write(&path, USER_FILE).unwrap();
    let store = SettingsStore::open(&path).unwrap();
    let errors = store.apply_set_args(&[
        "viewing.initial_zoom=fill",
        "bogus=1",
        "colour.dithering=no",
    ]);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].0, "bogus=1");
    let s = store.snapshot();
    assert_eq!(s.viewing.initial_zoom, InitialZoom::Fill);
    assert!(!s.colour.dithering);
    assert_eq!(store.last_change().unwrap().origin, Origin::CommandLine);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), USER_FILE);
    assert!(!store.can_undo());
}

#[test]
fn set_args_survive_a_reload_of_the_file() {
    let dir = scratch("cli-reload");
    let path = dir.join("settings.toml");
    std::fs::write(&path, USER_FILE).unwrap();
    let store = SettingsStore::open(&path).unwrap();
    assert!(
        store
            .apply_set_args(&["viewing.initial_zoom=fill"])
            .is_empty()
    );

    // The file is edited elsewhere, the --set key among others: the session value wins.
    let edited = USER_FILE
        .replace("loop_folder = true", "loop_folder = false")
        .replace("initial_zoom = \"fit\"", "initial_zoom = \"actual_size\"");
    std::fs::write(&path, &edited).unwrap();
    assert!(store.reload().unwrap());
    let s = store.snapshot();
    assert_eq!(s.viewing.initial_zoom, InitialZoom::Fill, "--set kept");
    assert!(!s.browsing.loop_folder, "the file's other edits applied");
    // The built-in editor's save is a reload too.
    store.write_text(USER_FILE).unwrap();
    assert_eq!(store.snapshot().viewing.initial_zoom, InitialZoom::Fill);
    assert!(store.snapshot().browsing.loop_folder);
    // Never written to the file.
    assert_eq!(std::fs::read_to_string(&path).unwrap(), USER_FILE);

    // A change made in the app replaces the --set value; the next reload takes the file's.
    store
        .set_text(
            "viewing.initial_zoom",
            "actual_size",
            Origin::SettingsWindow,
        )
        .unwrap();
    std::fs::write(&path, USER_FILE).unwrap();
    store.reload().unwrap();
    assert_eq!(store.snapshot().viewing.initial_zoom, InitialZoom::Fit);
}

// ----- Store: notification, typed updates, undo ----------------------------------

#[test]
fn generation_and_changed_keys() {
    let store = SettingsStore::in_memory();
    let g0 = store.generation();
    assert_eq!(store.changes_since(g0), Some(vec![]));
    store
        .set(
            "viewing.zoom_animation",
            Value::Bool(false),
            Origin::Program,
        )
        .unwrap();
    // Same value again: no new generation.
    assert!(
        !store
            .set(
                "viewing.zoom_animation",
                Value::Bool(false),
                Origin::Program
            )
            .unwrap()
    );
    let g1 = store.generation();
    assert_eq!(g1, g0 + 1);
    let keys = store.update(Origin::Program, |s| {
        s.browsing.sort_order = SortOrder::Modified;
        s.accessibility.ui_scale = 1.5;
    });
    assert_eq!(keys, ["browsing.sort_order", "accessibility.ui_scale"]);
    assert_eq!(store.generation(), g1 + 1);
    assert_eq!(store.changes_since(g1).unwrap(), keys);
    assert_eq!(
        store.changes_since(g0).unwrap(),
        [
            "viewing.zoom_animation",
            "browsing.sort_order",
            "accessibility.ui_scale"
        ]
    );
    // Long ago: unknown, so everything must be treated as changed.
    for i in 0..100 {
        store
            .set(
                "performance.prefetch_depth",
                Value::Int(i % 3 + 1),
                Origin::Program,
            )
            .unwrap();
    }
    assert_eq!(store.changes_since(g0), None);

    // Invalid values in a typed update are rejected, the rest applies.
    let keys = store.update(Origin::Program, |s| {
        s.accessibility.text_scale = 50.0;
        s.colour.dithering = false;
    });
    assert_eq!(keys, ["colour.dithering"]);
    assert_eq!(store.snapshot().accessibility.text_scale, 1.0);

    assert!(store.is_modified("colour.dithering"));
    assert!(!store.is_modified("colour.management"));
    assert_eq!(store.get("colour.dithering").unwrap(), Value::Bool(false));
    assert!(matches!(
        store.get("nope"),
        Err(SettingError::UnknownKey { .. })
    ));
}

#[test]
fn snapshots_are_shared_between_threads() {
    let store = SettingsStore::in_memory();
    let handles: Vec<_> = (0..4)
        .map(|i| {
            let store = store.clone();
            std::thread::spawn(move || {
                for j in 0..50 {
                    store
                        .set(
                            "performance.prefetch_depth",
                            Value::Int((i + j) % 8),
                            Origin::Program,
                        )
                        .unwrap();
                    let _ = store.snapshot().performance.prefetch_depth;
                }
            })
        })
        .collect();
    handles.into_iter().for_each(|h| h.join().unwrap());
    assert!(store.generation() > 0);
}

#[test]
fn undo_and_redo_in_the_store() {
    let store = SettingsStore::in_memory();
    assert!(!store.can_undo());
    store
        .set(
            "viewing.zoom_animation",
            Value::Bool(false),
            Origin::SettingsWindow,
        )
        .unwrap();
    // A slider drag: many quick changes to one key become one step.
    for v in [1.1, 1.2, 1.3, 1.4] {
        store
            .set(
                "accessibility.ui_scale",
                Value::Float(v),
                Origin::SettingsWindow,
            )
            .unwrap();
    }
    // Palette and program changes are not undoable from the window.
    store
        .set("colour.dithering", Value::Bool(false), Origin::Palette)
        .unwrap();

    // What the next undo and redo would change can be asked first.
    assert_eq!(store.next_undo_keys(), ["accessibility.ui_scale"]);
    assert!(store.next_redo_keys().is_empty());
    assert_eq!(store.undo(), ["accessibility.ui_scale"]);
    assert_eq!(store.next_redo_keys(), ["accessibility.ui_scale"]);
    assert_eq!(store.next_undo_keys(), ["viewing.zoom_animation"]);
    assert_eq!(store.snapshot().accessibility.ui_scale, 1.0);
    assert_eq!(store.last_change().unwrap().origin, Origin::Undo);
    assert_eq!(store.undo(), ["viewing.zoom_animation"]);
    assert!(store.snapshot().viewing.zoom_animation);
    assert!(!store.can_undo());
    assert!(!store.snapshot().colour.dithering);

    assert_eq!(store.redo(), ["viewing.zoom_animation"]);
    assert!(!store.snapshot().viewing.zoom_animation);
    assert!(store.can_redo());

    // A reset is one step.
    store
        .set(
            "browsing.loop_folder",
            Value::Bool(true),
            Origin::SettingsWindow,
        )
        .unwrap();
    let reset = store.reset_all(Origin::SettingsWindow);
    assert_eq!(
        reset,
        [
            "viewing.zoom_animation",
            "browsing.loop_folder",
            "colour.dithering"
        ]
    );
    assert_eq!(*store.snapshot(), Settings::default());
    let restored = store.undo();
    assert_eq!(restored.len(), 3);
    assert!(store.snapshot().browsing.loop_folder);
    assert!(!store.snapshot().colour.dithering);

    // Explicit groups.
    store.begin_undo_group();
    store
        .set(
            "metadata.hide_gps",
            Value::Bool(true),
            Origin::SettingsWindow,
        )
        .unwrap();
    store
        .set(
            "metadata.units",
            Value::Text("metric".into()),
            Origin::SettingsWindow,
        )
        .unwrap();
    store.end_undo_group();
    assert_eq!(store.undo().len(), 2);
    assert!(!store.snapshot().metadata.hide_gps);

    assert!(
        store
            .reset("advanced.reset_all", Origin::SettingsWindow)
            .is_err()
    );
    store.clear_undo();
    assert!(!store.can_undo() && !store.can_redo());
}

#[test]
fn reset_section() {
    let store = SettingsStore::in_memory();
    store.update(Origin::Program, |s| {
        s.viewing.zoom_animation = false;
        s.viewing.background = Background::White;
        s.colour.dithering = false;
    });
    let keys = store.reset_section(Section::Viewing, Origin::SettingsWindow);
    assert_eq!(keys, ["viewing.zoom_animation", "viewing.background"]);
    assert!(!store.snapshot().colour.dithering);
    assert_eq!(store.reset("colour.dithering", Origin::Program), Ok(true));
}

// ----- Schema and catalogue ---------------------------------------------------

#[test]
fn every_message_id_exists_in_english() {
    let mut missing = Vec::new();
    let mut check = |id: &str| {
        if !specular_i18n::english_has(id) {
            missing.push(id.to_owned());
        }
    };
    for m in all() {
        check(m.label_id);
        if !m.is_action() {
            check(m.help_id);
        }
        for o in m.options() {
            check(&m.option_id(o));
        }
    }
    for s in Section::ALL {
        check(s.label_id());
    }
    for id in [
        "settings-value-on",
        "settings-value-off",
        "settings-value-auto",
        "settings-value-default-location",
        "settings-value-system-language",
        "settings-value-follow-language",
    ] {
        check(id);
    }
    assert!(missing.is_empty(), "missing in en: {missing:#?}");
}

#[test]
fn schema_shape() {
    let keys: Vec<_> = all().iter().map(|m| m.key).collect();
    let mut sorted = keys.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), keys.len(), "duplicate keys");
    let pm = find("advanced.portable_mode").unwrap();
    assert!(!pm.stored && pm.needs_restart);
    assert!(find("general.single_instance").unwrap().needs_restart);
    assert!(find("advanced.reset_all").unwrap().is_action());
    assert!(!find("advanced.reset_all").unwrap().stored);
    assert_eq!(in_section(Section::Keys).count(), 2);
    assert!(find("keys.bindings").unwrap().is_action());
    // Every default is valid and round-trips through the meta table.
    let d = Settings::default();
    for m in all().iter().filter(|m| !m.is_action()) {
        assert_eq!(d.get(m.key).as_ref(), Some(&m.default), "{}", m.key);
        assert_eq!(
            m.validate(m.default.clone()).as_ref(),
            Ok(&m.default),
            "{}",
            m.key
        );
    }
    // An exported default file reloads to the defaults with no warnings.
    let l = load_str(&to_full_document(&d).to_string());
    assert!(l.warnings.is_empty(), "{:?}", l.warnings);
    assert_eq!(l.settings, d);
}

#[test]
fn display_values_are_localised() {
    let en = I18n::for_language(langid!("en"));
    let de = I18n::for_language(langid!("de"));
    let zoom = find("viewing.initial_zoom").unwrap();
    assert_eq!(
        zoom.display_value(&Value::Text("fit".into()), &en),
        "Fit to window"
    );
    assert_eq!(zoom.label(&de), "Anfänglicher Zoom");
    let anim = find("viewing.zoom_animation").unwrap();
    assert_eq!(anim.display_value(&Value::Bool(true), &en), "On");
    let ram = find("performance.ram_cache").unwrap();
    assert_eq!(
        ram.display_value(&Value::Int(0), &en),
        en.tr("settings-value-auto")
    );
    let delay = find("viewing.toolbar_autohide_delay").unwrap();
    let shown = delay.display_value(&Value::Float(1.5), &en);
    assert!(shown.contains("1.5") && shown.ends_with(" s"), "{shown}");
    let lang = find("general.language").unwrap();
    assert_eq!(lang.display_value(&Value::Text("de".into()), &en), "German");
    assert_eq!(
        lang.display_value(&Value::Text("de".into()), &de),
        "Deutsch"
    );
}

// ----- Search -----------------------------------------------------------------

fn keys(found: Vec<&SettingMeta>) -> Vec<&'static str> {
    found.into_iter().map(|m| m.key).collect()
}

#[test]
fn search_in_english() {
    let en = I18n::for_language(langid!("en"));
    let r = keys(search("dither", &en));
    assert_eq!(r.first(), Some(&"colour.dithering"), "{r:?}");
    let r = keys(search("Loop", &en));
    assert_eq!(r.first(), Some(&"browsing.loop_folder"), "{r:?}");
    // Keys match too.
    let r = keys(search("initial_zoom", &en));
    assert_eq!(r.first(), Some(&"viewing.initial_zoom"), "{r:?}");
    // Option labels: "fit to window" finds the setting that offers it.
    let r = keys(search("fit to window", &en));
    assert!(r.contains(&"viewing.initial_zoom"), "{r:?}");
    // Every word must match.
    assert!(search("dither zebra", &en).is_empty());
    // Labels outrank help text: "background" matches the background settings first.
    let r = keys(search("background", &en));
    assert!(
        r[0].starts_with("viewing.") && r[0].contains("background"),
        "{r:?}"
    );
    // Empty query lists everything that applies on this OS, in order.
    let here = all().iter().filter(|m| m.applies_here()).count();
    assert_eq!(search("  ", &en).len(), here);
}

#[test]
fn settings_for_other_platforms_are_hidden() {
    let en = I18n::for_language(langid!("en"));
    let m = find("viewing.fullscreen_mode").unwrap();
    assert_eq!(m.applies_here(), cfg!(target_os = "macos"));
    let found = keys(search("full-screen style", &en)).contains(&"viewing.fullscreen_mode");
    assert_eq!(found, cfg!(target_os = "macos"));
    // Platform wording: ⌘ on macOS, Ctrl elsewhere, never both.
    let help = find("viewing.wheel_behaviour").unwrap().help(&en);
    assert_eq!(help.contains('⌘'), cfg!(target_os = "macos"), "{help}");
    assert!(!help.contains("macOS"), "{help}");
}

#[test]
fn search_in_german_also_matches_english() {
    let de = I18n::for_language(langid!("de"));
    let r = keys(search("Ordner", &de));
    assert!(r.contains(&"browsing.loop_folder"), "{r:?}");
    // Case and umlaut folding: "anfanglicher" finds "Anfänglicher Zoom".
    let r = keys(search("anfanglicher", &de));
    assert_eq!(r.first(), Some(&"viewing.initial_zoom"), "{r:?}");
    // English words still work in a German UI (tutorials, muscle memory).
    let r = keys(search("transparent", &de));
    assert!(r.contains(&"viewing.transparency"), "{r:?}");
    let r = keys(search("sort", &de));
    assert!(r.contains(&"browsing.sort_order"), "{r:?}");
}

// ----- Files: atomic writes, import/export, watching, portable mode ----------------------

#[test]
fn atomic_write_replaces_whole_file_and_leaves_no_temporaries() {
    let dir = scratch("atomic");
    let path = dir.join("settings.toml");
    std::fs::write(&path, "old = true\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    write_atomic(&path, "new = true\n").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new = true\n");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
    let names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, ["settings.toml"]);

    // Writing into a folder that doesn't exist yet creates it.
    let nested = dir.join("a/b/settings.toml");
    write_atomic(&nested, "x = 1\n").unwrap();
    assert_eq!(std::fs::read_to_string(&nested).unwrap(), "x = 1\n");

    // Concurrent readers only ever see a complete file.
    let big_a = "a = 1\n".repeat(20_000);
    let big_b = "b = 2\n".repeat(20_000);
    write_atomic(&path, &big_a).unwrap();
    let reader = {
        let path = path.clone();
        let (a, b) = (big_a.clone(), big_b.clone());
        std::thread::spawn(move || {
            for _ in 0..200 {
                let t = std::fs::read_to_string(&path).unwrap();
                assert!(t == a || t == b, "partial file seen ({} bytes)", t.len());
            }
        })
    };
    for i in 0..50 {
        write_atomic(&path, if i % 2 == 0 { &big_b } else { &big_a }).unwrap();
    }
    reader.join().unwrap();
}

#[test]
fn export_and_import() {
    let dir = scratch("export");
    let a = SettingsStore::in_memory();
    a.update(Origin::Program, |s| {
        s.viewing.background = Background::Custom;
        s.viewing.background_color = Color::rgb(1, 2, 3);
        s.viewing.toolbar_items = vec![ToolbarItem::Open, ToolbarItem::Info];
        s.performance.ram_cache = 2048;
        s.colour.display_profile = Some(dir.join("display.icc"));
        s.general.language = "fr".parse().unwrap();
    });
    let file = dir.join("exported.toml");
    a.export_to(&file).unwrap();
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains(&format!("version = {SCHEMA_VERSION}")));
    assert!(
        text.contains("[performance]") && text.contains("ram_cache = 2048"),
        "{text}"
    );
    assert!(
        !text.contains("portable_mode") && !text.contains("reset_all"),
        "{text}"
    );

    let b = SettingsStore::in_memory();
    let warnings = b.import_from(&file).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(*b.snapshot(), *a.snapshot());
    assert_eq!(b.last_change().unwrap().origin, Origin::Import);
    // An import is one undo step.
    b.undo();
    assert_eq!(*b.snapshot(), Settings::default());

    // A partial file only changes what it mentions; bad values are reported.
    let c = SettingsStore::in_memory();
    c.set("colour.dithering", Value::Bool(false), Origin::Program)
        .unwrap();
    let w = c
        .import_str("[viewing]\ninitial_zoom = \"fill\"\nzoom_animation = 7\n")
        .unwrap();
    assert_eq!(w.len(), 1);
    assert_eq!(c.snapshot().viewing.initial_zoom, InitialZoom::Fill);
    assert!(!c.snapshot().colour.dithering);
    // Not TOML at all.
    assert!(c.import_str("this is not = = toml").is_err());
    assert!(matches!(
        c.import_from(&dir.join("missing.toml")),
        Err(SettingsError::Read { .. })
    ));
}

#[test]
fn watcher_reloads_external_edits() {
    let dir = scratch("watch");
    let path = dir.join("settings.toml");
    std::fs::write(&path, USER_FILE).unwrap();
    let store = SettingsStore::open(&path).unwrap();
    store.watch().unwrap();
    let g = store.generation();

    // Our own write is not reported back as a file change.
    store
        .set(
            "colour.dithering",
            Value::Bool(false),
            Origin::SettingsWindow,
        )
        .unwrap();
    let own = store.generation();
    assert_eq!(own, g + 1);

    // An external edit (as an editor would save it).
    std::thread::sleep(Duration::from_millis(50));
    let edited = std::fs::read_to_string(&path).unwrap().replace(
        "loop_folder = true",
        "loop_folder = false\nsort_order = \"size\"",
    );
    std::fs::write(&path, edited).unwrap();

    let deadline = Instant::now() + Duration::from_secs(10);
    while store.generation() == own && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    let change = store.last_change().expect("reload");
    assert_eq!(change.origin, Origin::File);
    let mut changed = change.keys.clone();
    changed.sort();
    assert_eq!(changed, ["browsing.loop_folder", "browsing.sort_order"]);
    let s = store.snapshot();
    assert!(!s.browsing.loop_folder);
    assert_eq!(s.browsing.sort_order, SortOrder::Size);
    assert!(!s.colour.dithering, "in-file value kept");

    // A broken edit keeps the current values and warns.
    std::fs::write(&path, "[browsing\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !store
        .warnings()
        .iter()
        .any(|w| matches!(w, Warning::Syntax { .. }))
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        store
            .warnings()
            .iter()
            .any(|w| matches!(w, Warning::Syntax { .. }))
    );
    assert_eq!(store.snapshot().browsing.sort_order, SortOrder::Size);
    store.unwatch();
}

#[test]
fn reload_wakes_the_ui() {
    let dir = scratch("watch-wake");
    let path = dir.join("settings.toml");
    std::fs::write(&path, USER_FILE).unwrap();
    let store = SettingsStore::open(&path).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    {
        // What the hook sees: the reload is already published when it runs.
        let s = store.clone();
        let tx = std::sync::Mutex::new(tx);
        store.set_on_reload(std::sync::Arc::new(move || {
            let _ = tx
                .lock()
                .unwrap()
                .send((s.generation(), s.file_generation()));
        }));
    }
    store.watch().unwrap();
    let quiet = |what: &str| {
        assert!(
            rx.recv_timeout(Duration::from_millis(600)).is_err(),
            "woken by {what}"
        );
    };

    // Our own write wakes nothing (the UI made it).
    store
        .set(
            "colour.dithering",
            Value::Bool(false),
            Origin::SettingsWindow,
        )
        .unwrap();
    quiet("our own write");

    // An external edit: one wake, after the new values are in.
    let g = store.generation();
    let edited = std::fs::read_to_string(&path)
        .unwrap()
        .replace("loop_folder = true", "loop_folder = false");
    let t0 = Instant::now();
    std::fs::write(&path, &edited).unwrap();
    let (gen_, _) = rx.recv_timeout(Duration::from_secs(10)).expect("woken");
    assert!(gen_ > g, "woken before the reload was published");
    assert!(!store.snapshot().browsing.loop_folder);
    assert!(
        t0.elapsed() < Duration::from_secs(2),
        "{:?} after the write",
        t0.elapsed()
    );
    quiet("the same edit twice");

    // Only `[[commands]]` changed: no setting did, but the commands are re-read.
    let fg = store.file_generation();
    std::fs::write(
        &path,
        format!("{edited}\n[[commands]]\nname = \"Edit\"\nprogram = \"gimp\"\n"),
    )
    .unwrap();
    let (_, fgen) = rx.recv_timeout(Duration::from_secs(10)).expect("woken");
    assert!(fgen > fg);

    // A broken edit changes nothing: no wake.
    std::fs::write(&path, "[browsing\n").unwrap();
    quiet("a broken edit");
    store.unwatch();
}

#[test]
fn portable_locations() {
    let dir = scratch("portable");
    let normal = Locations::for_exe_dir(Some(&dir));
    assert!(!normal.portable);
    assert_ne!(normal.config_dir, dir);
    assert!(normal.settings_path().ends_with(SETTINGS_FILE));

    std::fs::write(dir.join(PORTABLE_MARKER), "").unwrap();
    let portable = Locations::for_exe_dir(Some(&dir));
    assert!(portable.portable);
    assert_eq!(portable.config_dir, dir);
    assert_eq!(portable.cache_dir, dir.join("cache"));
    assert_eq!(portable.settings_path(), dir.join("settings.toml"));

    // A folder named "portable" is not the marker.
    let other = scratch("portable-dir");
    std::fs::create_dir(other.join(PORTABLE_MARKER)).unwrap();
    assert!(!Locations::for_exe_dir(Some(&other)).portable);
}

#[test]
fn version_1_performance_switches_become_overrides() {
    let old = "version = 1\n\n[performance]\n# mine\nfast_yuv_upload = false\nhardware_decode = true # try it\n";
    let l = load_str(old);
    assert!(l.migrated && l.warnings.is_empty(), "{:?}", l.warnings);
    assert_eq!(l.settings.performance.fast_yuv_upload, PathOverride::Off);
    assert_eq!(l.settings.performance.hardware_decode, PathOverride::On);
    let text = l.doc.unwrap().to_string();
    assert!(text.contains("# mine\nfast_yuv_upload = \"off\""), "{text}");
    assert!(text.contains("hardware_decode = \"on\" # try it"), "{text}");
    let l =
        load_str("version = 1\n[performance]\nfast_yuv_upload = true\nhardware_decode = false\n");
    assert_eq!(l.settings.performance.fast_yuv_upload, PathOverride::Auto);
    assert_eq!(l.settings.performance.hardware_decode, PathOverride::Auto);
    assert_eq!(l.settings.performance.processing, PathMode::Automatic);
}

/// Local AI model sizes: Recommended and the named sizes round-trip for every feature; the
/// old relative values still load (for the app to migrate) but nothing offers them.
#[test]
fn model_sizes_round_trip_and_old_values_still_load() {
    let keys = [
        "ai.search_size",
        "ai.text_size",
        "ai.culling_size",
        "ai.people_size",
        "ai.speech_size",
        "ai.masks_size",
        "ai.denoise_size",
        "ai.remove_size",
        "ai.depth_size",
    ];
    for key in keys {
        let m = find(key).unwrap();
        assert_eq!(m.options(), ["recommended", "small", "standard", "large"]);
        assert_eq!(m.default, Value::Text("recommended".into()));
    }
    for size in ModelSize::ALL {
        let mut s = Settings::default();
        s.ai.search_size = *size;
        s.ai.people_size = *size;
        let text = to_full_document(&s).to_string();
        let back = load_str(&text);
        assert!(back.warnings.is_empty(), "{:?}", back.warnings);
        assert_eq!(back.settings.ai.search_size, *size);
        assert_eq!(back.settings.ai.people_size, *size);
        assert!(!size.is_legacy());
        // `--set` and the palette take the same names.
        assert_eq!(size.as_str().parse::<ModelSize>().unwrap(), *size);
    }
    for (name, old) in [
        ("smaller", ModelSize::Smaller),
        ("larger", ModelSize::Larger),
    ] {
        let text = format!("version = {SCHEMA_VERSION}\n[ai]\nsearch_size = \"{name}\" # mine\n");
        let l = load_str(&text);
        assert!(l.warnings.is_empty(), "{:?}", l.warnings);
        assert!(lint(&text).is_empty());
        assert_eq!(l.settings.ai.search_size, old);
        assert!(old.is_legacy());
        // Written back as read until the app migrates it.
        let mut doc = l.doc.unwrap();
        update_document(&mut doc, &l.settings, &["ai.search_size"]);
        assert!(
            doc.to_string()
                .contains(&format!("search_size = \"{name}\" # mine"))
        );
        let m = find("ai.search_size").unwrap();
        assert_eq!(m.parse(name), Ok(Value::Text(name.into())));
        assert!(!m.options().contains(&name));
        let en = I18n::for_language(langid!("en"));
        assert_eq!(
            m.display_value(&Value::Text(name.into()), &en),
            specular_i18n::isolate_ltr(name)
        );
    }
    let l = load_str("version = 2\n[ai]\nsearch_size = \"huge\"\n");
    assert_eq!(l.warnings.len(), 1);
    assert_eq!(l.settings.ai.search_size, ModelSize::Recommended);
}

#[test]
fn version_2_start_choices_become_the_start_screen() {
    for old in ["empty", "last_image", "open_dialog"] {
        let text = format!("version = 2\n[general]\non_start = \"{old}\" # mine\n");
        let l = load_str(&text);
        assert!(l.migrated && l.warnings.is_empty(), "{:?}", l.warnings);
        assert_eq!(l.settings.general.on_start, OnStart::StartScreen);
        let doc = l.doc.unwrap().to_string();
        assert!(doc.contains("on_start = \"start_screen\" # mine"), "{doc}");
    }
    let l = load_str("version = 5\n[general]\non_start = \"library\"\n");
    assert!(!l.migrated && l.warnings.is_empty(), "{:?}", l.warnings);
    assert_eq!(l.settings.general.on_start, OnStart::Library);
}

/// Version 3 profiles: Engineering covered maps (vector maps were drawings), so it gains
/// Maps; other profiles stay as they were, and a profile with Maps already is not doubled.
#[test]
fn version_3_engineering_gains_maps() {
    use specular_settings::ProfileCapability as P;
    let l = load_str(
        "version = 3\n[profile]\n# mine\ncapabilities = [\"general\", \"engineering\"] # kept\n",
    );
    assert!(l.migrated && l.warnings.is_empty(), "{:?}", l.warnings);
    assert_eq!(
        l.settings.profile.capabilities,
        [P::General, P::Engineering, P::Maps]
    );
    let doc = l.doc.unwrap().to_string();
    assert!(doc.contains("# mine\ncapabilities = ["), "{doc}");
    assert!(doc.contains("# kept"), "{doc}");
    assert!(doc.contains("version = 5"), "{doc}");
    for (old, want) in [
        ("[\"general\", \"medical\"]", vec![P::General, P::Medical]),
        (
            "[\"general\", \"maps\", \"engineering\"]",
            vec![P::General, P::Maps, P::Engineering],
        ),
        ("[\"general\"]", vec![P::General]),
    ] {
        let l = load_str(&format!("version = 3\n[profile]\ncapabilities = {old}\n"));
        assert!(l.warnings.is_empty(), "{:?}", l.warnings);
        assert_eq!(l.settings.profile.capabilities, want, "{old}");
    }
    // No profile table: nothing to do, defaults apply.
    let l = load_str("version = 3\n[viewing]\nzoom_animation = false\n");
    assert_eq!(l.settings.profile.capabilities, [P::General]);
    // A current file is left alone.
    let l = load_str("version = 5\n[profile]\ncapabilities = [\"general\", \"engineering\"]\n");
    assert!(!l.migrated);
    assert_eq!(
        l.settings.profile.capabilities,
        [P::General, P::Engineering]
    );
}

/// Version 4: the DWG converter's two viewing settings move to the optional decoders,
/// choices and comments kept; without a choice the converter is used when found.
#[test]
fn version_4_dwg_converter_moves_to_optional_decoders() {
    let l = load_str(
        "version = 4\n[viewing]\nzoom_animation = false\ndwg_converter = false # mine\n\
         dwg_converter_path = \"/opt/oda/ODAFileConverter\"\nocct = false\n\
         occt_path = \"/opt/occt/lib\"\n",
    );
    assert!(l.migrated && l.warnings.is_empty(), "{:?}", l.warnings);
    assert!(!l.settings.decoders.dwg);
    assert_eq!(
        l.settings.decoders.dwg_path.as_deref(),
        Some(std::path::Path::new("/opt/oda/ODAFileConverter"))
    );
    assert!(!l.settings.viewing.zoom_animation);
    assert!(!l.settings.decoders.occt);
    assert_eq!(
        l.settings.decoders.occt_path.as_deref(),
        Some(std::path::Path::new("/opt/occt/lib"))
    );
    let doc = l.doc.unwrap().to_string();
    assert!(doc.contains("[decoders]"), "{doc}");
    assert!(doc.contains("dwg = false # mine"), "{doc}");
    assert!(!doc.contains("dwg_converter"), "{doc}");
    assert!(doc.contains("version = 5"), "{doc}");
    // Nothing written: the defaults, which use what is found.
    let l = load_str("version = 4\n[viewing]\nzoom_animation = false\n");
    assert!(l.settings.decoders.dwg && l.settings.decoders.braw && l.settings.decoders.occt);
    assert_eq!(l.settings.decoders.dwg_path, None);
    // A current file is left alone.
    let l = load_str("version = 5\n[decoders]\nbraw = false\n");
    assert!(!l.migrated && !l.settings.decoders.braw);
}

#[test]
fn maps_settings_round_trip() {
    use specular_settings::{CoordinateFormat, Units};
    let d = Settings::default();
    assert_eq!(d.maps.coordinate_format, CoordinateFormat::Decimal);
    assert_eq!(d.maps.units, Units::Auto);
    let l = load_str("version = 5\n[maps]\ncoordinate_format = \"mgrs\"\nunits = \"imperial\"\n");
    assert!(l.warnings.is_empty(), "{:?}", l.warnings);
    assert_eq!(l.settings.maps.coordinate_format, CoordinateFormat::Mgrs);
    assert_eq!(l.settings.maps.units, Units::Imperial);
}

// ----- The built-in editor: exact writes, lint ------------------------------------------

#[test]
fn write_text_writes_exactly_and_applies_live() {
    let dir = scratch("write-text");
    let path = dir.join("settings.toml");
    std::fs::write(&path, USER_FILE).unwrap();
    let store = SettingsStore::open(&path).unwrap();
    assert_eq!(store.file_text(), USER_FILE);
    let gen0 = store.generation();
    let tgen = store.text_generation();

    // Odd formatting is kept byte for byte; dropped keys return to their defaults.
    let text = "version=2\n\n[viewing]\n   initial_zoom   =   'fill'  # mine\n";
    let changed = store.write_text(text).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    assert!(changed.contains(&"viewing.initial_zoom"));
    assert!(changed.contains(&"browsing.loop_folder"));
    assert_eq!(store.snapshot().viewing.initial_zoom, InitialZoom::Fill);
    assert!(store.generation() > gen0);
    assert_eq!(store.last_change().unwrap().origin, Origin::Editor);
    assert!(store.text_generation() > tgen);
    assert_eq!(store.file_text(), text);
    // Our own write is not an edit made elsewhere.
    assert!(!store.reload().unwrap());

    // Text that does not parse is refused and nothing is written.
    assert!(matches!(
        store.write_text("[viewing\n"),
        Err(SettingsError::FileHasErrors { .. })
    ));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);

    // Later changes from the Settings window still save into the same file.
    store
        .set(
            "browsing.loop_folder",
            Value::Bool(true),
            Origin::SettingsWindow,
        )
        .unwrap();
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(
        saved.contains("initial_zoom   =   'fill'  # mine"),
        "{saved}"
    );
    assert!(saved.contains("loop_folder = true"), "{saved}");
    assert_eq!(store.file_text(), saved);
}

#[test]
fn file_text_of_an_in_memory_store_shows_changed_settings() {
    let store = SettingsStore::in_memory();
    assert!(store.file_text().contains("version = "));
    store
        .set_text("viewing.initial_zoom", "fill", Origin::Palette)
        .unwrap();
    let t = store.file_text();
    assert!(
        t.contains("[viewing]") && t.contains("initial_zoom = \"fill\""),
        "{t}"
    );
    assert!(lint(&t).is_empty(), "{:?}", lint(&t));
    store
        .write_text("[viewing]\ninitial_zoom = \"fit\"\n")
        .unwrap();
    assert_eq!(store.snapshot().viewing.initial_zoom, InitialZoom::Fit);
}

#[test]
fn lint_spans_and_localised_messages() {
    let text = "[viewing]\ninitial_zoom = \"huge\"\nbogus = 1\n";
    let d = lint(text);
    assert_eq!(d.len(), 2, "{d:#?}");
    assert_eq!(&text[d[0].span.clone()], "\"huge\"");
    assert_eq!(&text[d[1].span.clone()], "bogus");
    assert!(d.iter().all(|d| d.severity == Severity::Error));
    let de = I18n::for_language(langid!("de"));
    for d in &d {
        assert!(specular_i18n::english_has(d.message_id()));
        assert!(!d.message(&de).is_empty());
    }
    // The loader reports a place for every warning.
    let loaded = load_str(text);
    assert_eq!(loaded.spans.len(), loaded.warnings.len());
    assert!(loaded.spans.iter().all(Option::is_some));
}

/// A reload hook that panics (app code) costs that one wake-up: the watch thread restarts
/// after its back-off and the next external edit is read and reported as usual.
#[test]
fn watch_survives_a_panicking_reload_hook() {
    let dir = scratch("watch-panic");
    let path = dir.join("settings.toml");
    std::fs::write(&path, USER_FILE).unwrap();
    let store = SettingsStore::open(&path).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    {
        let s = store.clone();
        let tx = std::sync::Mutex::new(tx);
        let first = std::sync::atomic::AtomicBool::new(true);
        store.set_on_reload(std::sync::Arc::new(move || {
            if first.swap(false, std::sync::atomic::Ordering::Relaxed) {
                panic!("test panic in the reload hook");
            }
            let _ = tx.lock().unwrap().send(s.snapshot().browsing.sort_order);
        }));
    }
    store.watch().unwrap();
    let edit = |from: &str, to: &str| {
        let text = std::fs::read_to_string(&path).unwrap().replace(from, to);
        std::fs::write(&path, text).unwrap();
    };
    edit("loop_folder = true", "loop_folder = false");
    // The first reload is published, then the hook panics: nothing is sent.
    let deadline = Instant::now() + Duration::from_secs(10);
    while store.snapshot().browsing.loop_folder && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!store.snapshot().browsing.loop_folder);
    assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
    // After the restart the next edit comes through.
    std::thread::sleep(Duration::from_millis(1200));
    edit(
        "loop_folder = false",
        "loop_folder = false\nsort_order = \"size\"",
    );
    let order = rx.recv_timeout(Duration::from_secs(10)).expect("woken");
    assert_eq!(order, SortOrder::Size);
    store.unwatch();
}
