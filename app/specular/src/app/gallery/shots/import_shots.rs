use super::*;

/// The import view: a card's new shots (all but one selected) with RAW+JPEG, RAW and video
/// badges, beside the side panel (destination, folder pattern, event, rename, options, free
/// space, Import). Dark at 1× and 1.5×, light at 1×; and the plain-language refusal when the
/// destination lacks free space.
#[test]
fn import_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let (dark, light) = (Palette::dark(), Palette::light());
    for (name, pal, scale, refused) in [
        ("import-dark-1x", &dark, 1.0, false),
        ("import-dark-1.5x", &dark, 1.5, false),
        ("import-light-1x", &light, 1.0, false),
        ("import-no-space-dark-1x", &dark, 1.0, true),
    ] {
        let (mut g, shots) = import_shots();
        let mount = PathBuf::from("/media/me/EOS_DIGITAL");
        super::import::testing::show(&mut g, "EOS_DIGITAL", &mount, shots, 37);
        super::import::testing::deselect(&mut g, 2);
        // A template, keywords and a style for every photo (dark at 1×).
        if name == "import-dark-1x" {
            super::import::testing::set_metadata(
                &mut g,
                "Studio copyright",
                "Family; Places > Lisbon",
                "Warm portrait",
            );
        }
        if refused {
            let msg = super::import::testing::no_space(
                Path::new("/home/me/Pictures"),
                512_000_000,
                120_000_000,
            );
            assert!(msg.contains("free space"), "{msg}");
            super::import::testing::set_error(&mut g, msg);
        }
        let f = Frame {
            pal,
            now: 1.0,
            reduce_motion: true,
            size: SIZE,
            scale,
            rtl: false,
            text_scale: 1.0,
            backdrop: Backdrop::None,
        };
        let shot = h.render(
            SIZE,
            scale,
            |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
            |dl, text| {
                g.draw(dl, text, &f);
            },
        );
        assert!(!g.import.wanted.is_empty(), "the tiles want thumbnails");
        goldens.check(name, &shot);
        crate::app::a11y_audit::audit_screen(name, SIZE, f.rtl, |t| g.a11y(t, 1.0));
    }
    goldens.finish();
}

/// The import view in Hebrew (right to left) passes the screen-reader audit.
#[test]
fn import_rtl_passes_the_audit() {
    let Some(mut h) = Harness::new() else { return };
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        let (mut g, shots) = import_shots();
        let mount = PathBuf::from("/media/me/EOS_DIGITAL");
        super::import::testing::show(&mut g, "EOS_DIGITAL", &mount, shots, 37);
        super::import::testing::deselect(&mut g, 2);
        let pal = Palette::light();
        let f = Frame {
            pal: &pal,
            now: 1.0,
            reduce_motion: true,
            size: SIZE,
            scale: 1.0,
            rtl: true,
            text_scale: 1.0,
            backdrop: Backdrop::None,
        };
        h.render(
            SIZE,
            1.0,
            |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
            |dl, text| {
                g.draw(dl, text, &f);
            },
        );
        crate::app::a11y_audit::audit_screen("import-he", SIZE, true, |t| g.a11y(t, 1.0));
    });
}
