//! Screenshot tests of Develop in the gallery: "Edited" badges, a photo's versions
//! badge and its versions stacked under it (one stack open, each version its own tile), in
//! English dark and Hebrew light at 1× and 1.5×. Rendered offscreen on lavapipe and compared
//! with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular gallery::develop_shots`.

use super::draw::Frame;
use super::*;
use crate::app::develop::edited::Facts;
use specular_gfx::DrawList;
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;

const SIZE: (f32, f32) = (720.0, 560.0);

fn facts(version: u32, name: &str, modules: &str) -> Facts {
    let reg = specular_develop::model::Registry::builtin();
    Facts {
        version,
        name: name.into(),
        hash: [version as u8; 32],
        modules: modules.into(),
        styles: String::new(),
        edit: Arc::new(specular_develop::model::Edit::default_for(
            reg,
            specular_develop::model::SourceKey::default(),
            true,
        )),
    }
}

/// Six photos: two edited, one of them with two more versions (its stack open).
fn gallery() -> Gallery {
    let mut g = Gallery {
        active: true,
        ..Gallery::default()
    };
    let colours: [u32; 4] = [0x3d78b8, 0x6aa84f, 0xe69138, 0xc27ba0];
    let entries: Vec<Entry> = (0..6usize)
        .map(|i| {
            let name = format!("IMG_{:04}.CR2", i + 1);
            let path = PathBuf::from(format!("/shots/{name}"));
            let c = colours[i % colours.len()];
            g.averages.insert(
                path.as_path().into(),
                [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255],
            );
            Entry {
                path,
                name,
                kind: EntryKind::Image,
                len: 24_000_000,
                modified: None,
                hidden: false,
                taken: None,
                dims: Some((6000, 4000)),
            }
        })
        .collect();
    g.add_entries(entries);
    let p = |i: usize| PathBuf::from(format!("/shots/IMG_{:04}.CR2", i));
    g.edited.listed = true;
    g.edited.facts.insert(p(2), facts(0, "", "exposure"));
    g.edited
        .facts
        .insert(p(5), facts(0, "", "exposure\ncolour_balance"));
    for (v, name) in [(1, ""), (2, "Black & white")] {
        let key = crate::app::develop::edited::version_key(&p(5), v);
        g.edited.photo_of.insert(key.clone(), p(5));
        g.edited.facts.insert(key, facts(v, name, "black_white"));
    }
    g.versions.insert(p(5), 3);
    g.develop_version_entries();
    g.expanded.insert(p(5));
    g.resort();
    g.rebuild();
    g.atlas = Some(super::atlas::Atlas::new(4));
    g
}

#[test]
fn develop_badges_and_versions_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let runs: [(&str, bool, Option<specular_i18n::LanguageIdentifier>); 2] = [
        ("gallery-develop-dark", true, None),
        (
            "gallery-develop-he-light",
            false,
            Some(specular_i18n::langid!("he")),
        ),
    ];
    for (name, dark, lang) in runs {
        let pal = if dark {
            Palette::dark()
        } else {
            Palette::light()
        };
        let rtl = lang.is_some();
        let mut run = || {
            for (scale, suffix) in [(1.0, "1x"), (1.5, "1.5x")] {
                let mut g = gallery();
                // Versions stack under their photo; the open stack shows each.
                assert_eq!(g.tiles.len(), 8, "{name}");
                let f = Frame {
                    pal: &pal,
                    now: 1.0,
                    reduce_motion: true,
                    size: SIZE,
                    scale,
                    rtl,
                    text_scale: 1.0,
                    backdrop: Backdrop::None,
                };
                for _ in 0..20 {
                    let mut warm = DrawList::new(scale);
                    h.text.begin_frame(scale);
                    g.draw(&mut warm, &mut h.text, &f);
                    if !g.cap_more {
                        break;
                    }
                }
                let shot = h.render(
                    SIZE,
                    scale,
                    |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
                    |dl, text| {
                        g.draw(dl, text, &f);
                    },
                );
                goldens.check(&format!("{name}-{suffix}"), &shot);
                let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
                g.a11y(&mut tree, 1.0);
                super::super::a11y_audit::expect_clean(&format!("{name}-{suffix}"), &tree, rtl);
            }
        };
        match lang {
            Some(l) => specular_i18n::I18n::with_thread_language(l, run),
            None => run(),
        }
    }
    goldens.finish();
}

/// The facts the filter sees: edited or not, a version is one, modules by id.
#[test]
fn the_filter_finds_edited_photos_and_versions() {
    let mut g = gallery();
    let shown = |g: &mut Gallery, q: &str| {
        g.filter.text = q.into();
        g.rebuild();
        let mut v: Vec<String> = g
            .tiles
            .iter()
            .map(|t| g.entries[t.entry].name.clone())
            .collect();
        v.sort();
        v
    };
    assert_eq!(shown(&mut g, "edited:yes").len(), 4);
    assert_eq!(shown(&mut g, "edited:no").len(), 4);
    let copies = shown(&mut g, "version:copy");
    assert_eq!(copies.len(), 2, "{copies:?}");
    assert!(
        copies
            .iter()
            .all(|n| specular_i18n::strip_isolates(n).starts_with("IMG_0005.CR2 · ")),
        "{copies:?}"
    );
    assert_eq!(shown(&mut g, "module:colour_balance"), ["IMG_0005.CR2"]);
    // Versions are not files to move or open in the viewer.
    g.filter.text.clear();
    g.rebuild();
    assert_eq!(g.image_order().len(), 6);
    assert_eq!(g.files().len(), 6);
}
