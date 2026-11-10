//! Screenshot tests of hooks: the question before a hook's first run (dark, left to right;
//! light, right to left in Hebrew) and the hooks' panel in Settings › Integration (an
//! allowed hook, one switched off and one that cannot be used; dark and light). Rendered offscreen on
//! lavapipe and compared with the PNGs in `app/specular/tests/shots/`; the screen-reader audit
//! runs over each.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular hooks::shots`, then review and
//! commit the rewritten PNGs.

use specular_gfx::{DrawList, Rect};
use specular_settings::commands::parse_hooks;
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;

use super::*;

const SHEET: (f32, f32) = (720.0, 440.0);
const PAGE: (f32, f32) = (1100.0, 760.0);

const HOOKS: &str = r#"
[[hooks]]
name = "Upload picks"
on = ["marked"]
when = { flag = "pick" }
program = "/home/me/bin/upload"
args = ["--album", "Picks", "{files}"]

[[hooks]]
name = "Log exports"
on = ["exported", "imported"]
program = "/usr/local/bin/log-photos"
args = ["{event}", "{outputs}"]
enabled = false

[[hooks]]
name = "Old"
on = ["clicked"]
program = "x"
"#;

fn listed() -> (Vec<Hook>, Vec<HookProblem>) {
    parse_hooks(&HOOKS.parse().unwrap())
}

#[test]
fn hook_consent_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-hooks",
    );
    let hook = listed().0.remove(0);
    let he = specular_i18n::langid!("he");
    for (name, dark, scale, rtl) in [
        ("hook-consent-dark-1x", true, 1.0, false),
        ("hook-consent-he-light-1.5x", false, 1.5, true),
    ] {
        let pal = if dark {
            Palette::dark()
        } else {
            Palette::light()
        };
        let lang = if rtl {
            he.clone()
        } else {
            specular_i18n::langid!("en-US")
        };
        let (shot, tree) = specular_i18n::I18n::with_thread_language(lang, || {
            let (title, detail) = consent_text(&hook);
            let mut c = specular_widgets::Chrome::new();
            c.reduce_motion = true;
            c.set_size(SHEET.0, SHEET.1);
            c.poke(1.0);
            c.confirm_choice(
                title,
                detail,
                t!("hook-consent-allow"),
                ALLOW_ITEM,
                t!("hook-consent-not-now"),
                NOT_NOW_ITEM,
            );
            crate::app::develop::interop_shots::with_state(|st| {
                let shot = h.render(
                    SHEET,
                    scale,
                    |dl: &mut DrawList| {
                        dl.rect(Rect::new(0.0, 0.0, SHEET.0, SHEET.1), 0.0, pal.canvas)
                    },
                    |dl, text| {
                        let mut warm = DrawList::new(scale);
                        c.draw(&mut warm, text, st, &pal, 1.0);
                        c.draw(dl, text, st, &pal, 1.0);
                    },
                );
                (shot, c.accessibility_tree(st, "Specular", 1.0))
            })
        });
        goldens.check(name, &shot);
        crate::app::a11y_audit::expect_clean(name, &tree, rtl);
    }
    goldens.finish();
}

#[test]
fn hooks_panel_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-hooks",
    );
    let (list, problems) = listed();
    let mut trusted = HashSet::new();
    trusted.insert(fingerprint(&list[0]));
    let he = specular_i18n::langid!("he");
    for (name, dark, scale, rtl) in [
        ("hooks-panel-dark-1x", true, 1.0, false),
        // English only: the Settings page draws strings it made outside the language
        // (`local-what-keywords`), which the right-to-left check refuses.
        ("hooks-panel-light-1.5x", false, 1.5, false),
    ] {
        let pal = if dark {
            Palette::dark()
        } else {
            Palette::light()
        };
        let lang = if rtl {
            he.clone()
        } else {
            specular_i18n::langid!("en-US")
        };
        let (shot, tree) = specular_i18n::I18n::with_thread_language(lang, || {
            let mut c = specular_widgets::Chrome::new();
            c.reduce_motion = true;
            c.set_size(PAGE.0, PAGE.1);
            c.poke(1.0);
            c.set_setting_panel(UNDER, Some(panel(&list, &problems, &trusted)));
            c.open_settings_at(UNDER);
            crate::app::develop::interop_shots::with_state(|st| {
                let shot = h.render(
                    PAGE,
                    scale,
                    |dl: &mut DrawList| {
                        dl.rect(Rect::new(0.0, 0.0, PAGE.0, PAGE.1), 0.0, pal.canvas)
                    },
                    |dl, text| {
                        let mut warm = DrawList::new(scale);
                        c.draw(&mut warm, text, st, &pal, 1.0);
                        c.draw(dl, text, st, &pal, 1.0);
                    },
                );
                (shot, c.accessibility_tree(st, "Specular", 1.0))
            })
        });
        goldens.check(name, &shot);
        crate::app::a11y_audit::expect_clean(name, &tree, rtl);
    }
    goldens.finish();
}
