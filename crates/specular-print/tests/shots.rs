//! Rendered pages on the software rasteriser (lavapipe), end to end: layout, GPU bands, PDF,
//! read back by the test parser and compared with goldens in `tests/shots/`.
//!
//! `SPECULAR_BLESS=1 cargo test -p specular-print --test shots` rewrites the goldens.

mod support;

use std::path::Path;

use specular_core::Orientation;
use specular_print::layout::{Caption, Fit, Item, Kind, PageOrientation, PageSetup, Paper};
use specular_print::pdf::Meta;
use specular_print::{Document, PageRenderer, layout};
use specular_shots::{Goldens, Harness, Shot};
use support::{Synthetic, SyntheticSource};

fn items(src: &SyntheticSource) -> Vec<Item> {
    src.images
        .iter()
        .enumerate()
        .map(|(i, s)| Item {
            size: s.as_ref().map_or((300, 200), Synthetic::display_size),
            name: if i == 5 {
                "a_rather_long_file_name_from_a_camera_0005.jpg".into()
            } else {
                format!("IMG_{:04}.jpg", i + 1)
            },
            date: Some("10 Oct 2026".into()),
            scale: None,
        })
        .collect()
}

fn renderer() -> Option<PageRenderer> {
    let h = Harness::new()?;
    Some(PageRenderer::new(
        h.gpu.clone(),
        specular_gfx::Text::bundled_only(),
    ))
}

/// Renders the whole document to a PDF and reads its pages back.
fn pdf_pages(r: &mut PageRenderer, doc: &Document, src: &SyntheticSource) -> Vec<Shot> {
    let meta = Meta {
        title: "shots".into(),
        date: String::new(),
        ..Default::default()
    };
    let (bytes, stats, _) = r
        .write_pdf(doc, &meta, Vec::new(), src, &mut |_| {}, &|| false)
        .unwrap();
    assert_eq!(stats.pages, doc.pages.len());
    support::parse(&bytes)
        .pages
        .into_iter()
        .map(|p| Shot {
            width: p.width,
            height: p.height,
            rgb: p.rgb,
            language: "en".into(),
            strays: Vec::new(),
            signs: Vec::new(),
        })
        .collect()
}

fn sheet() -> (Document, SyntheticSource) {
    let mut images = support::mixed(23);
    images[7] = None; // unreadable: a grey box
    let src = SyntheticSource { images };
    let setup = PageSetup {
        paper: Paper::A4,
        orientation: PageOrientation::Portrait,
        margin: 12.0,
        dpi: 90,
    };
    let kind = Kind::Sheet {
        cols: 4,
        rows: 5,
        caption: Caption::NameDate,
        header: true,
    };
    let doc = layout(
        setup,
        kind,
        &items(&src),
        "Holiday 2026 / Lake",
        &|n, of| format!("Page {n} of {of}"),
    );
    (doc, src)
}

#[test]
fn contact_sheet_pages() {
    let Some(mut r) = renderer() else { return };
    let (doc, src) = sheet();
    assert_eq!(doc.pages.len(), 2);
    let pages = pdf_pages(&mut r, &doc, &src);
    assert_eq!((pages[0].width, pages[0].height), (744, 1052));

    // The preview is the printed page at another size: the same at the same size.
    let (w, h, rgba) = r.render_rgba(&doc, 1, 90.0, &src, &|| false).unwrap();
    assert_eq!((w, h), (744, 1052));
    let rgb: Vec<u8> = rgba
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    let d = specular_shots::delta(
        &pages[1],
        &Shot {
            width: w,
            height: h,
            rgb,
            language: "en".into(),
            strays: Vec::new(),
            signs: Vec::new(),
        },
    );
    assert_eq!(d.differing, 0, "preview and PDF differ (max {})", d.max);

    // Margins stay white.
    let p = &pages[0];
    let m = (12.0 / 25.4 * 90.0) as usize - 1;
    for y in [0, m, p.height as usize - 1 - m] {
        for x in 0..p.width as usize {
            let i = (y * p.width as usize + x) * 3;
            assert_eq!(&p.rgb[i..i + 3], &[255, 255, 255], "margin at {x},{y}");
        }
    }

    let mut g = Goldens::new(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/shots"),
        "specular-print",
    );
    g.check("contact_sheet_page1", &pages[0]);
    g.check("contact_sheet_page2", &pages[1]);
    g.finish();
}

/// A contact sheet printed from a Hebrew UI: Hebrew and Arabic file names (long ones cut in
/// the middle, keeping their extension), a Hebrew title at the right and the page number at
/// the left; the sheet reads from the right.
#[test]
fn contact_sheet_right_to_left() {
    let Some(mut r) = renderer() else { return };
    let src = SyntheticSource {
        images: support::mixed(8),
    };
    let names = [
        "צילום משפחתי ארוך מאוד מהחופשה בים המלח 0001.jpg",
        "חוף.jpg",
        "صورة العائلة على شاطئ البحر في الصيف.png",
        "IMG_0004 (עותק).jpg",
        "شروق.heic",
        "2026-08-14 שקיעה.jpg",
        "IMG_0007.jpg",
        "ים_המלח_panorama_final_v2.tif",
    ];
    let items: Vec<Item> = items(&src)
        .into_iter()
        .zip(names)
        .map(|(i, n)| Item {
            name: n.into(),
            date: Some("14 באוג׳ 2026".into()),
            ..i
        })
        .collect();
    let setup = PageSetup {
        paper: Paper::A4,
        orientation: PageOrientation::Portrait,
        margin: 12.0,
        dpi: 90,
    };
    let kind = Kind::Sheet {
        cols: 3,
        rows: 4,
        caption: Caption::NameDate,
        header: true,
    };
    // "Summer holiday 2026 / Dead Sea, chosen photos from every day of the trip".
    let ltr = layout(
        setup,
        kind,
        &items,
        "חופשת הקיץ 2026 / ים המלח, צילומים נבחרים מכל ימי הטיול",
        &|n, of| format!("עמוד {n} מתוך {of}"),
    );
    let doc = ltr.clone().right_to_left();
    // The first image is at the top right, in the cell the third has left to right.
    let first = &doc.pages[0].images[0];
    assert!(first.image.x > ltr.pages[0].images[0].image.x);
    let cell = |i: usize| ltr.pages[0].images[i].caption_rect.right();
    assert!((first.caption_rect.right() - cell(2)).abs() < 1e-9);
    let pages = pdf_pages(&mut r, &doc, &src);
    assert_eq!(pages.len(), 1);
    let p = &pages[0];
    // Header ink: the title (bold, long) fills the right part, the page number the left.
    let m = (12.0 / 25.4 * 90.0) as usize + 2;
    let ink_cols = |x0: usize, x1: usize| {
        (x0..x1)
            .filter(|&x| {
                (m..m + 12).any(|y| {
                    let i = (y * p.width as usize + x) * 3;
                    p.rgb[i] < 128
                })
            })
            .count()
    };
    let w = p.width as usize;
    let (left, right) = (ink_cols(0, w / 2), ink_cols(w / 2, w));
    assert!(
        right > left * 2,
        "title at the right: left {left}, right {right}"
    );
    let lp = &pdf_pages(&mut r, &ltr, &src)[0];
    let d = specular_shots::delta(p, lp);
    assert!(d.differing > 0, "the header mirrors");

    let mut g = Goldens::new(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/shots"),
        "specular-print",
    );
    g.check(
        "contact_sheet_rtl",
        &Shot {
            language: "he".into(),
            ..p.clone()
        },
    );
    g.finish();
}

#[test]
fn single_pages_fit_fill_and_size() {
    let Some(mut r) = renderer() else { return };
    let src = SyntheticSource {
        images: vec![
            Some(Synthetic {
                size: (300, 200),
                orientation: Orientation(1),
                alpha: true,
            }),
            Some(Synthetic {
                size: (300, 200),
                orientation: Orientation(8),
                alpha: false,
            }),
        ],
    };
    let setup = PageSetup {
        paper: Paper::Custom { w: 148.0, h: 210.0 },
        orientation: PageOrientation::Landscape,
        margin: 8.0,
        dpi: 72,
    };
    let it = items(&src);
    let mut g = Goldens::new(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/shots"),
        "specular-print",
    );
    for (name, fit) in [
        ("single_fit", Fit::Fit),
        ("single_fill", Fit::Fill),
        ("single_size_8cm", Fit::Size { width: 80.0 }),
    ] {
        let kind = Kind::Single {
            fit,
            caption: Caption::Name,
        };
        let doc = layout(setup, kind, &it, "", &|_, _| String::new());
        let pages = pdf_pages(&mut r, &doc, &src);
        assert_eq!(pages.len(), 2);
        assert_eq!((pages[0].width, pages[0].height), (595, 420));
        g.check(&format!("{name}_1"), &pages[0]);
        g.check(&format!("{name}_2"), &pages[1]);
    }
    g.finish();
}

/// Small tiles and bands give the same page as one tile: no seams.
#[test]
fn tiles_and_bands_join_without_seams() {
    let Some(mut r) = renderer() else { return };
    let (doc, src) = sheet();
    let (w, h, whole) = r.render_rgba(&doc, 0, 60.0, &src, &|| false).unwrap();
    r.set_max_tile(173, 61);
    let (w2, h2, tiled) = r.render_rgba(&doc, 0, 60.0, &src, &|| false).unwrap();
    assert_eq!((w, h), (w2, h2));
    let differing = whole
        .chunks_exact(4)
        .zip(tiled.chunks_exact(4))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 1))
        .count();
    assert_eq!(
        differing, 0,
        "pixels differ between whole and tiled rendering"
    );
}

/// A cancelled document stops between tiles.
#[test]
fn cancelling_stops_the_page() {
    let Some(mut r) = renderer() else { return };
    let (doc, src) = sheet();
    let calls = std::cell::Cell::new(0);
    let res = r.render_rgba(&doc, 0, 60.0, &src, &|| {
        calls.set(calls.get() + 1);
        calls.get() > 3
    });
    assert!(matches!(res, Err(specular_print::RenderError::Cancelled)));
}
