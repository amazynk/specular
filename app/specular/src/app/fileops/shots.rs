//! Screenshot tests of the file operation sheets: the rename field with the stem selected and
//! its sidecars, a name that is taken (light), the batch rename preview with a problem row
//! (dark, and right to left in Hebrew, light), copy to a folder with recent folders and taken
//! names, and the settings import preview. Rendered offscreen on lavapipe and compared with
//! the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular fileops::shots`, then review and
//! commit the rewritten PNGs.

use super::*;
use specular_gfx::Rect;
use specular_i18n::t;
use specular_shots::{Goldens, Harness};
use specular_theme::Color;

const SIZE: (f32, f32) = (900.0, 640.0);

fn backdrop(dl: &mut DrawList) {
    dl.rect(
        Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
        0.0,
        Color::rgba(0.18, 0.24, 0.3, 1.0),
    );
    dl.rect(
        Rect::new(0.0, SIZE.1 * 0.55, SIZE.0, SIZE.1 * 0.45),
        0.0,
        Color::rgba(0.3, 0.26, 0.2, 1.0),
    );
}

/// A scratch folder with `names` in it (empty files), removed on drop.
struct Folder(PathBuf);

impl Folder {
    fn new(tag: &str, names: &[&str]) -> Folder {
        let d = std::env::temp_dir().join(format!("specular-fileops-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        for n in names {
            std::fs::write(d.join(n), b"x").unwrap();
        }
        Folder(d)
    }

    fn file(&self, n: &str) -> PathBuf {
        self.0.join(n)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Folder paths are the machine's: the goldens show a fixed one.
const SHOWN_DIR: &str = "/home/me/Pictures/2026 Lisbon";

fn rename(f: &Folder, name: &str, typed: Option<&str>) -> SheetView {
    let file = f.file(name);
    let mut field = LineEdit::new(name);
    match typed {
        Some(t) => {
            field.set_text(t);
        }
        None => field.select(names::stem_range(name)),
    }
    let mut v = rename_view(&file, &field, Part::Field);
    v.subtitle = Some(SHOWN_DIR.into());
    v
}

fn batch(f: &Folder) -> Batch {
    let files: Vec<PathBuf> = [
        "IMG_2041.HEIC",
        "IMG_2042.HEIC",
        "IMG_2043.HEIC",
        "DSC_0007.NEF",
        "notes.jpg",
    ]
    .iter()
    .map(|n| f.file(n))
    .collect();
    let mut probes = HashMap::new();
    for (i, p) in files.iter().enumerate().take(4) {
        let camera = if i < 3 {
            "iPhone 15 Pro"
        } else {
            "NIKON Z 6_2"
        };
        probes.insert(
            p.clone(),
            (
                (4032, 3024),
                Some([2026, 7, 14, 9 + i as u32, 3, 9]),
                Some(camera.to_owned()),
            ),
        );
    }
    let mut b = Batch {
        files,
        field: LineEdit::new("{date:%Y-%m-%d} {n:3}"),
        probes,
        probing: false,
        rows: Vec::new(),
        rows_for: None,
        top: 0,
    };
    rebuild_rows(&mut b);
    b
}

fn transfer(f: &Folder) -> TransferSheet {
    let dest = f.file("Portfolio");
    std::fs::create_dir_all(&dest).unwrap();
    std::fs::write(dest.join("IMG_2041.HEIC"), b"y").unwrap();
    let other = f.file("Family");
    std::fs::create_dir_all(&other).unwrap();
    let mut t = TransferSheet {
        mode: Mode::Copy,
        files: vec![f.file("IMG_2041.HEIC"), f.file("IMG_2042.HEIC")],
        folders: vec![dest.clone(), other],
        chosen: Some(dest),
        conflict: Conflict::KeepBoth,
        confirm: false,
        picking: false,
        items: Vec::new(),
    };
    replan(&mut t);
    t
}

fn transfer_shown(t: &TransferSheet) -> SheetView {
    let mut v = transfer_view(t, Part::Row(0));
    for r in &mut v.rows {
        if r.end.is_some() {
            r.end = Some(SHOWN_DIR.into());
        }
    }
    v
}

fn import() -> ImportSheet {
    let text = "version = 2\n[viewing]\nzoom_animation = false\nbackground = \"black\"\n\
                [gallery]\nthumbnail_size = 240\nlayout = \"sideways\"\n\
                [dicomweb]\npassword = \"x\"\n";
    let preview =
        specular_settings::preview_import(&specular_settings::Settings::default(), text).unwrap();
    ImportSheet {
        path: PathBuf::from("/home/me/specular-settings.toml"),
        preview,
        keys: vec![(t!("palette-rename"), "F2".to_owned(), "Ctrl+E".to_owned())],
        key_problems: Vec::new(),
        top: 0,
    }
}

#[test]
fn fileops_sheets_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-fileops",
    );
    let f = Folder::new(
        "shots",
        &[
            "IMG_2041.HEIC",
            "IMG_2042.HEIC",
            "IMG_2043.HEIC",
            "DSC_0007.NEF",
            "DSC_0007.JPG",
            "DSC_0007.xmp",
            "notes.jpg",
            "beach.jpg",
            "2026-07-14 005.jpg",
        ],
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    let in_lang = |lang: &specular_i18n::LanguageIdentifier, f: &dyn Fn() -> SheetView| {
        specular_i18n::I18n::with_thread_language(lang.clone(), f)
    };
    let rename_raw = in_lang(&en, &|| rename(&f, "DSC_0007.NEF", None));
    let rename_taken = in_lang(&en, &|| rename(&f, "notes.jpg", Some("beach.jpg")));
    let b = batch(&f);
    let batch_en = in_lang(&en, &|| batch_view(&b, Part::Field));
    let batch_he = in_lang(&he, &|| batch_view(&b, Part::Field));
    let t = transfer(&f);
    let transfer_en = in_lang(&en, &|| transfer_shown(&t));
    let i = import();
    let import_en = in_lang(&en, &|| import_view(&i, Part::Button(0)));
    let (dark, light) = (Palette::dark(), Palette::light());
    for (name, v, pal, rtl) in [
        ("rename", &rename_raw, &dark, false),
        ("rename-taken-light", &rename_taken, &light, false),
        ("batch", &batch_en, &dark, false),
        ("batch-he-rtl-light", &batch_he, &light, true),
        ("copy", &transfer_en, &dark, false),
        ("import", &import_en, &dark, false),
    ] {
        let lang = if rtl { &he } else { &en };
        let mut drawn = None;
        let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
            h.render(SIZE, 1.0, backdrop, |dl, text| {
                drawn = Some(view::draw(dl, text, pal, v, SIZE, rtl, 1.0));
            })
        });
        goldens.check(&format!("fileops-{name}-1x"), &shot);
        let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
        view::a11y(&mut tree, v, &drawn.expect("drawn"), 1.0);
        super::super::a11y_audit::expect_clean(name, &tree, rtl);
    }
    goldens.finish();
}

/// What the sheets say, checked as data: the stem is selected, sidecars and the RAW's JPEG
/// partner follow, a taken name says so, the batch preview marks the clash with a file
/// outside the batch, and the import preview lists changes but never the password.
#[test]
fn sheets_say_what_will_happen() {
    let f = Folder::new(
        "data",
        &[
            "IMG_2041.HEIC",
            "IMG_2042.HEIC",
            "IMG_2043.HEIC",
            "DSC_0007.NEF",
            "DSC_0007.JPG",
            "DSC_0007.xmp",
            "notes.jpg",
            "beach.jpg",
            "2026-07-14 005.jpg",
        ],
    );
    let en = specular_i18n::langid!("en-US");
    specular_i18n::I18n::with_thread_language(en, || {
        let v = rename(&f, "DSC_0007.NEF", None);
        let fv = v.field.as_ref().unwrap();
        assert_eq!(fv.selection, (0, "DSC_0007".len()));
        assert!(v.error.is_none());
        assert!(v.notes[0].contains("DSC_0007.JPG") && v.notes[0].contains("DSC_0007.xmp"));
        // Unchanged: nothing to do yet.
        assert!(!v.buttons[0].enabled);

        let v = rename(&f, "notes.jpg", Some("beach.jpg"));
        assert_eq!(
            v.error.as_deref(),
            Some(t!("rename-problem-exists").as_str())
        );
        assert!(!v.buttons[0].enabled);
        let v = rename(&f, "notes.jpg", Some("notes.png"));
        assert!(v.error.is_none() && v.buttons[0].enabled);
        assert!(v.notes.iter().any(|n| *n == t!("rename-extension-changed")));
        let v = rename(&f, "notes.jpg", Some("a/b.jpg"));
        assert!(v.error.as_deref().unwrap().contains('/'));

        let b = batch(&f);
        let names: Vec<&str> = b.rows.iter().map(|r| r.new_name.as_str()).collect();
        assert_eq!(
            names,
            [
                "2026-07-14 001.HEIC",
                "2026-07-14 002.HEIC",
                "2026-07-14 003.HEIC",
                "2026-07-14 004.NEF",
                "undated 005.jpg"
            ]
        );
        assert!(b.rows.iter().all(|r| r.problem.is_none()));
        // A pattern whose names collide with a file outside the batch.
        let mut b = batch(&f);
        b.field.set_text("2026-07-14 {n:3}");
        rebuild_rows(&mut b);
        let v = batch_view(&b, Part::Field);
        assert_eq!(
            b.rows[4].problem,
            Some(batch::RowProblem::Name(NameProblem::Exists))
        );
        assert!(v.rows[4].problem.is_some());
        assert!(v.buttons[0].enabled);
        b.field.set_text("{nope}");
        rebuild_rows(&mut b);
        let v = batch_view(&b, Part::Field);
        assert!(v.error.is_some() && !v.buttons[0].enabled);

        let mut t = transfer(&f);
        assert_eq!(transfer::conflicts(&t.items), 1);
        let v = transfer_view(&t, Part::Row(0));
        assert!(v.rows[0].chosen);
        assert_eq!(v.rows.len(), 3);
        t.conflict = Conflict::Replace;
        t.confirm = true;
        let v = transfer_view(&t, Part::Button(1));
        assert_eq!(v.buttons[0].label, t!("transfer-replace"));

        let i = import();
        let v = import_view(&i, Part::Button(0));
        let all = format!("{v:?}");
        assert!(!all.contains("\"x\""));
        assert!(v.notes.iter().any(|n| n.contains("never imported")));
        // Two settings, one key binding, one invalid value (and the unknown table).
        assert!(v.rows.len() >= 4, "{:?}", v.rows);
        assert!(v.buttons[0].enabled);
    });
}

/// Parts sit in the sheet without overlapping, right to left mirrors, large text fits, and
/// the accessibility tree names every part.
#[test]
fn parts_sit_in_the_sheet_and_mirror() {
    let Some(mut h) = Harness::new() else { return };
    let f = Folder::new("layout", &["IMG_2041.HEIC", "IMG_2042.HEIC", "beach.jpg"]);
    let pal = Palette::dark();
    let t = transfer(&f);
    let v = transfer_shown(&t);
    let mut out = Vec::new();
    h.render(SIZE, 1.0, backdrop, |dl, text| {
        out.push(view::draw(dl, text, &pal, &v, SIZE, false, 1.0));
        out.push(view::draw(dl, text, &pal, &v, SIZE, true, 1.0));
        out.push(view::draw(dl, text, &pal, &v, SIZE, false, 2.0));
    });
    let (l, r) = (&out[0], &out[1]);
    assert_eq!(l.sheet, r.sheet);
    for d in [&out[0], &out[2]] {
        let s = d.sheet;
        for (i, (a, ha)) in d.hits.iter().enumerate() {
            assert!(
                a.x >= s.x - 0.01
                    && a.right() <= s.right() + 0.01
                    && a.y >= s.y - 0.01
                    && a.bottom() <= s.bottom() + 0.01,
                "{ha:?} {a:?} outside {s:?}"
            );
            for (b, hb) in &d.hits[i + 1..] {
                let apart = a.right() <= b.x + 0.01
                    || b.right() <= a.x + 0.01
                    || a.bottom() <= b.y + 0.01
                    || b.bottom() <= a.y + 0.01;
                assert!(apart, "{ha:?} {a:?} overlaps {hb:?} {b:?}");
            }
        }
    }
    for ((a, ha), (b, hb)) in l.hits.iter().zip(&r.hits) {
        assert_eq!(ha, hb);
        assert!((a.x - (SIZE.0 - b.right())).abs() < 0.01, "{ha:?}");
    }
    let mut tree = accesskit::TreeUpdate {
        nodes: vec![(
            accesskit::NodeId(1),
            accesskit::Node::new(accesskit::Role::Window),
        )],
        tree: None,
        tree_id: accesskit::TreeId::ROOT,
        focus: accesskit::NodeId(1),
    };
    view::a11y(&mut tree, &v, l, 1.0);
    assert_eq!(view::a11y_target(tree.focus.0), Some(Part::Row(0)));
    let mut parts = Vec::new();
    for (id, _) in &tree.nodes {
        parts.extend(view::a11y_target(id.0));
    }
    for p in v.focusables() {
        assert!(
            parts.contains(&p) || matches!(p, Part::Choice(_)),
            "{p:?} has no node"
        );
    }
    for i in 0..3 {
        assert!(parts.contains(&Part::Choice(i)));
    }
}

/// An input method composing in the rename field (synthetic preedit): the composition is
/// drawn at the caret, the candidate window's area spans it inside the field, in both
/// directions; nothing composed, the area is the caret.
#[test]
fn rename_field_shows_a_composition_at_the_caret() {
    let f = Folder::new("ime", &["notes.jpg"]);
    let pal = Palette::dark();
    let mut text = specular_gfx::Text::bundled_only();
    for rtl in [false, true] {
        let mut v = rename(&f, "notes.jpg", Some("notes.jpg"));
        let mut dl = DrawList::new(1.0);
        let plain = view::draw(&mut dl, &mut text, &pal, &v, SIZE, rtl, 1.0);
        let field = plain
            .hits
            .iter()
            .find(|(_, p)| *p == Part::Field)
            .map(|h| h.0)
            .unwrap();
        let caret = plain
            .ime
            .expect("an IME area while the field has the keyboard");
        assert!(caret.w < 4.0, "nothing composed: the caret {caret:?}");
        assert!(plain.field_text.is_some());

        v.field.as_mut().unwrap().preedit = "めも".into();
        let mut dl = DrawList::new(1.0);
        let d = view::draw(&mut dl, &mut text, &pal, &v, SIZE, rtl, 1.0);
        let area = d.ime.unwrap();
        assert!(area.w > 15.0, "rtl {rtl}: {area:?}");
        assert!(
            area.x >= field.x && area.right() <= field.right() && area.y >= field.y,
            "rtl {rtl}: {area:?} in {field:?}"
        );
        // A click while composing would land in the composition: none is mapped.
        assert!(d.field_text.is_none());
        // Not into the text (it is typed when the input method commits).
        assert_eq!(v.field.as_ref().unwrap().text, "notes.jpg");
    }
}
