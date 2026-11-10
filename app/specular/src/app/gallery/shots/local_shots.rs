//! Local copies (`local.rs`): the offline banner over the Library's grid (a card offline,
//! changes waiting; then back, with files that changed meanwhile and Review), and the
//! conflict question showing both versions. Dark at 1× and light at 1.5×.

use super::*;
use crate::app::gallery::local::{Conflict, conflict_text, marks_text};
use specular_library::local as lc;

fn offline_gallery(conflicts: bool) -> Gallery {
    use specular_library::Location;
    let mut g = gallery();
    g.filter_open = false;
    let lv = &mut g.library;
    lv.on = true;
    lv.panel = true;
    lv.searched = true;
    lv.locations = vec![
        Location {
            id: 1,
            path: PathBuf::from("/shots/Pictures"),
            offline: false,
            files: 18_204,
            pending: 0,
            last_scan: None,
        },
        Location {
            id: 2,
            path: PathBuf::from("/media/card/DCIM"),
            offline: !conflicts,
            files: 312,
            pending: 0,
            last_scan: None,
        },
    ];
    if conflicts {
        lv.local.conflicts.push_back(conflict());
        lv.local.conflicts.push_back(conflict());
    } else {
        lv.local.waiting = 14;
    }
    g.rebuild();
    g
}

fn conflict() -> Conflict {
    Conflict {
        change: lc::Change {
            id: 1,
            path: PathBuf::from("/media/card/DCIM/IMG_2041.CR3"),
            identity: lc::Identity::default(),
            sidecars: None,
            xmp_mtime: None,
            kind: lc::ChangeKind::Marks,
            version: 0,
            payload: Vec::new(),
            queued: 0,
        },
        file: "IMG_2041.CR3".into(),
        yours: marks_text(Mark {
            rating: 5,
            label: 3,
            flag: 1,
        }),
        theirs: marks_text(Mark {
            rating: 2,
            label: 0,
            flag: 0,
        }),
    }
}

#[test]
fn local_copies_banner_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    for (look, dark, scale, suffix) in [("dark", true, 1.0, "1x"), ("light", false, 1.5, "1.5x")] {
        let pal = if dark {
            Palette::dark()
        } else {
            Palette::light()
        };
        for (state, conflicts) in [("offline", false), ("back", true)] {
            let mut g = offline_gallery(conflicts);
            assert!(g.local_banner_h() > 0.0);
            let f = Frame {
                pal: &pal,
                now: 1.0,
                reduce_motion: true,
                size: SIZE,
                scale,
                rtl: false,
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
            goldens.check(&format!("local-banner-{state}-{look}-{suffix}"), &shot);
            let review = g
                .hits
                .iter()
                .any(|h| h.1 == Hit::Library(super::library::Side::LocalReview));
            assert_eq!(review, conflicts, "Review only with conflicts");
        }
    }
    goldens.finish();
}

#[test]
fn local_copies_conflict_matches_goldens() {
    const SHEET: (f32, f32) = (720.0, 420.0);
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    for (look, dark, scale, suffix) in [("dark", true, 1.0, "1x"), ("light", false, 1.5, "1.5x")] {
        let pal = if dark {
            Palette::dark()
        } else {
            Palette::light()
        };
        let (title, detail) = conflict_text(&conflict());
        let mut c = specular_widgets::Chrome::new();
        c.reduce_motion = true;
        c.set_size(SHEET.0, SHEET.1);
        c.poke(1.0);
        c.confirm_choice(
            title,
            detail,
            t!("local-keep-mine"),
            1,
            t!("local-keep-theirs"),
            2,
        );
        let shot = crate::app::develop::interop_shots::with_state(|st| {
            h.render(
                SHEET,
                scale,
                |dl: &mut DrawList| dl.rect(Rect::new(0.0, 0.0, SHEET.0, SHEET.1), 0.0, pal.canvas),
                |dl, text| {
                    let mut warm = DrawList::new(scale);
                    c.draw(&mut warm, text, st, &pal, 1.0);
                    c.draw(dl, text, st, &pal, 1.0);
                },
            )
        });
        goldens.check(&format!("local-conflict-{look}-{suffix}"), &shot);
    }
    goldens.finish();
}
