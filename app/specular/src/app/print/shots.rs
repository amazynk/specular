//! Screenshot tests of the print dialog at 1× and 1.5×: a contact sheet of a folder with its
//! preview, the same while the PDF is being written, one image per page at an exact width
//! right-to-left in the light theme, and the dialog before the images are read. Rendered
//! offscreen on lavapipe and compared with the PNGs in `app/specular/tests/shots/`; see
//! `specular-shots` for the tolerance.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular print::shots`, then review and
//! commit the rewritten PNGs.

use super::*;
use specular_shots::{Goldens, Harness};

const SIZE: (f32, f32) = (900.0, 620.0);
const SCALES: [(f32, &str); 2] = [(1.0, "1x"), (1.5, "1.5x")];

/// What stands for the viewer under the scrim.
fn backdrop(dl: &mut DrawList) {
    dl.rect(
        Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
        0.0,
        Color::rgba(0.2, 0.32, 0.45, 1.0),
    );
    dl.rect(
        Rect::new(0.0, SIZE.1 * 0.6, SIZE.0, SIZE.1 * 0.4),
        0.0,
        Color::rgba(0.25, 0.38, 0.22, 1.0),
    );
}

/// A synthetic A4 contact sheet page (header, a 4 × 6 grid of photos with caption lines),
/// standing for the print thread's raster.
fn sheet_page(w: u32, h: u32) -> Vec<u8> {
    let mut px = vec![255u8; (w * h * 4) as usize];
    let mut fill = |x0: u32, y0: u32, x1: u32, y1: u32, c: [u8; 3]| {
        for y in y0..y1.min(h) {
            for x in x0..x1.min(w) {
                let i = ((y * w + x) * 4) as usize;
                px[i..i + 3].copy_from_slice(&c);
            }
        }
    };
    let m = w / 14;
    fill(m, m, w / 2, m + h / 90, [60, 60, 60]);
    let top = m + h / 30;
    let (cols, rows) = (4, 6);
    let cw = (w - 2 * m) / cols;
    let ch = (h - top - m) / rows;
    for r in 0..rows {
        for c in 0..cols {
            let x = m + c * cw;
            let y = top + r * ch;
            let k = (r * cols + c) as f32 / 24.0;
            let sky = [
                (90.0 + 120.0 * k) as u8,
                (150.0 - 40.0 * k) as u8,
                (220.0 - 90.0 * k) as u8,
            ];
            let iw = cw * 9 / 10;
            let ih = iw * 2 / 3;
            fill(x, y, x + iw, y + ih * 6 / 10, sky);
            fill(x, y + ih * 6 / 10, x + iw, y + ih, [70, 110, 60]);
            fill(
                x + iw / 6,
                y + ih + 4,
                x + iw * 5 / 6,
                y + ih + 7,
                [120, 120, 120],
            );
        }
    }
    px
}

fn sheet_choices() -> Choices {
    Choices {
        sheet: true,
        free: false,
        free_unit: Unit::Mm,
        setup: PageSetup::default(),
        unit: Unit::Cm,
        fit: Fit::Fit,
        width: 100.0,
        ratio: None,
        single_caption: Caption::Off,
        cols: 4,
        rows: 6,
        header: true,
        sheet_caption: Caption::Name,
        copies: 1,
    }
}

fn view(c: &Choices, source: String, preview: Option<TexId>) -> DialogView {
    let (pw, ph) = c.setup.size_mm();
    DialogView {
        rows: row_views(c, source),
        page: (1, 3),
        aspect: (pw / ph) as f32,
        preview,
        status: None,
        progress: None,
        busy: false,
        can_print: true,
        focus: Some(1),
        editor: None,
    }
}

#[test]
fn print_dialog_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-print-dialog",
    );
    let (pw, ph) = (420, 594);
    let (_tex, tex) = upload(&h.gpu, &mut h.ui, pw, ph, &sheet_page(pw, ph));
    let c = sheet_choices();
    let sheet = view(&c, source_label(Source::Folder, 70), Some(tex));
    let mut busy = sheet.clone();
    busy.busy = true;
    busy.focus = Some(busy.focusables().len() - 3);
    busy.status = Some(t!("print-writing", page = 2, pages = 3));
    busy.progress = Some(1.0 / 3.0);
    let single = Choices {
        sheet: false,
        fit: Fit::Size { width: 120.0 },
        width: 120.0,
        unit: Unit::In,
        setup: PageSetup {
            paper: Paper::Letter,
            orientation: PageOrientation::Landscape,
            ..PageSetup::default()
        },
        ..c
    };
    // Right to left in Hebrew: made and drawn in the language.
    let he = specular_i18n::langid!("he");
    let mut single = specular_i18n::I18n::with_thread_language(he.clone(), || {
        view(&single, source_label(Source::Current, 1), None)
    });
    single.page = (1, 1);
    single.can_print = false;
    single.focus = None;
    let mut reading = view(&c, source_label(Source::Selection, 12), None);
    reading.page = (1, 0);
    reading.status = Some(t!("print-preparing"));
    // A drawing printed to scale, as drawn.
    let mut scaled = view(
        &Choices {
            sheet: false,
            fit: Fit::Scale { ratio: None },
            ..c
        },
        source_label(Source::Current, 1),
        None,
    );
    scaled.page = (2, 6);
    let pal_dark = Palette::dark();
    let pal_light = Palette::light();
    for (scale, suffix) in SCALES {
        for (name, v, pal, rtl) in [
            ("sheet", &sheet, &pal_dark, false),
            ("busy", &busy, &pal_dark, false),
            ("single-rtl-light", &single, &pal_light, true),
            ("reading", &reading, &pal_dark, false),
            ("to-scale", &scaled, &pal_dark, false),
        ] {
            let lang = if rtl {
                he.clone()
            } else {
                specular_i18n::langid!("en-US")
            };
            let (shot, tree) = specular_i18n::I18n::with_thread_language(lang, || {
                let mut drawn = None;
                let shot = h.render(SIZE, scale, backdrop, |dl, text| {
                    drawn = Some(draw(dl, text, pal, v, SIZE, rtl, 1.0));
                });
                let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
                a11y(&mut tree, v, &drawn.expect("drawn"), 1.0);
                (shot, tree)
            });
            goldens.check(&format!("print-dialog-{name}-{suffix}"), &shot);
            super::super::a11y_audit::expect_clean(
                &format!("print-dialog-{name}-{suffix}"),
                &tree,
                rtl,
            );
        }
    }
    goldens.finish();
}

/// Flat two-colour test photos for the real page renderer.
pub(super) struct Photos;

impl specular_print::ImageSource for Photos {
    fn key(&self, index: usize) -> u64 {
        index as u64 + 1
    }

    fn load(&self, index: usize, _long_px: u32) -> Result<specular_print::Loaded, String> {
        let (w, h) = (90u32, 60u32);
        let k = index as f32 / 8.0;
        let sky = [
            (90.0 + 120.0 * k) as u8,
            (150.0 - 40.0 * k) as u8,
            (220.0 - 90.0 * k) as u8,
            255,
        ];
        let mut data = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for _ in 0..w {
                data.extend_from_slice(if y < h * 6 / 10 {
                    &sky
                } else {
                    &[70, 110, 60, 255]
                });
            }
        }
        Ok(specular_print::Loaded {
            raster: specular_core::Raster {
                width: w,
                height: h,
                origin: (0, 0),
                scale: 1,
                layout: specular_core::PixelLayout::Rgba,
                planes: vec![specular_core::RasterPlane {
                    width: w,
                    height: h,
                    stride: w as usize * 4,
                    data,
                    h_div: 1,
                    v_div: 1,
                }],
                truncated: false,
            },
            stored: (w, h),
            orientation: specular_core::Orientation(1),
            lut: None,
        })
    }
}

/// The preview of a contact sheet printed from a Hebrew UI, rendered by the real page
/// renderer: Hebrew and Arabic file names (long ones cut in the middle, keeping their
/// extension), a Hebrew folder title at the right and the page number at the left.
#[test]
fn print_preview_right_to_left_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-print-dialog",
    );
    let he = specular_i18n::langid!("he");
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
    let items: Vec<Item> = names
        .iter()
        .map(|n| Item {
            size: (90, 60),
            name: (*n).to_owned(),
            date: Some("14 באוג׳ 2026".into()),
            scale: None,
        })
        .collect();
    let c = Choices {
        cols: 3,
        rows: 4,
        sheet_caption: Caption::NameDate,
        ..sheet_choices()
    };
    let (doc, mut v) = specular_i18n::I18n::with_thread_language(he.clone(), || {
        // "Summer holiday 2026 / Dead Sea".
        let doc = specular_print::layout(
            c.setup,
            c.kind(),
            &items,
            "חופשת הקיץ 2026 / ים המלח, צילומים נבחרים",
            &|page, pages| t!("print-page", page = page as i64, pages = pages as i64),
        );
        (doc, view(&c, source_label(Source::Folder, 8), None))
    });
    let doc = doc.right_to_left();
    let mut r =
        specular_print::PageRenderer::new(h.gpu.clone(), specular_gfx::Text::bundled_only());
    let (pw, ph, rgba) = r
        .render_rgba(&doc, 0, 50.0, &Photos, &|| false)
        .expect("page");
    let (_tex, tex) = upload(&h.gpu, &mut h.ui, pw, ph, &rgba);
    v.preview = Some(tex);
    v.page = (1, 1);
    let pal = Palette::light();
    for (scale, suffix) in SCALES {
        let shot = specular_i18n::I18n::with_thread_language(he.clone(), || {
            h.render(SIZE, scale, backdrop, |dl, text| {
                draw(dl, text, &pal, &v, SIZE, true, 1.0);
            })
        });
        goldens.check(&format!("print-dialog-sheet-he-light-{suffix}"), &shot);
    }
    goldens.finish();
}

/// Changing the UI language while the dialog is open lays the page out again in the new
/// one: the page number in its words and, for Hebrew, the sheet mirrored (first image top
/// right). The dialog notices by the language's generation.
#[test]
fn page_is_laid_out_again_in_the_new_language() {
    let items: Vec<Item> = (0..3)
        .map(|i| Item {
            size: (90, 60),
            name: format!("IMG_{i}.jpg"),
            date: None,
            scale: None,
        })
        .collect();
    let c = sheet_choices();
    let en = specular_i18n::I18n::with_thread_language(specular_i18n::langid!("en-US"), || {
        document(&c, &items, "Trip", false, None)
    });
    let he = specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        document(&c, &items, "Trip", true, None)
    });
    assert!(!en.rtl && he.rtl);
    let (eh, hh) = (
        en.pages[0].header.as_ref().unwrap(),
        he.pages[0].header.as_ref().unwrap(),
    );
    assert_ne!(eh.page, hh.page, "page number in the new language");
    let (e0, h0) = (&en.pages[0].images[0].image, &he.pages[0].images[0].image);
    let (pw, _) = c.setup.size_mm();
    assert!(
        (h0.x - (pw - e0.x - e0.w)).abs() < 1e-3,
        "first image mirrored"
    );
}

/// Every control is inside the sheet, the steppers and buttons do not overlap, and
/// right-to-left mirrors them.
#[test]
fn controls_sit_in_the_sheet_and_mirror() {
    let Some(mut h) = Harness::new() else { return };
    let c = sheet_choices();
    let v = view(&c, source_label(Source::Folder, 70), None);
    let pal = Palette::dark();
    let mut ltr = None;
    let mut rtl = None;
    h.render(SIZE, 1.0, backdrop, |dl, text| {
        ltr = Some(draw(dl, text, &pal, &v, SIZE, false, 1.0));
        rtl = Some(draw(dl, text, &pal, &v, SIZE, true, 1.0));
    });
    let (l, r) = (ltr.unwrap(), rtl.unwrap());
    assert_eq!(l.sheet, r.sheet);
    let s = l.sheet;
    let buttons: Vec<_> = l
        .hits
        .iter()
        .filter(|(_, h)| !matches!(h, Hit::Row(_)))
        .collect();
    for (i, (a, ha)) in buttons.iter().enumerate() {
        assert!(a.x >= s.x && a.right() <= s.right() && a.y >= s.y && a.bottom() <= s.bottom());
        for (b, hb) in &buttons[i + 1..] {
            let apart =
                a.right() <= b.x || b.right() <= a.x || a.bottom() <= b.y || b.bottom() <= a.y;
            assert!(apart, "{ha:?} {a:?} overlaps {hb:?} {b:?}");
        }
    }
    assert!(
        l.page.right() <= l.rows[0].0.x,
        "the preview is before the rows"
    );
    for ((a, ha), (b, hb)) in l.hits.iter().zip(&r.hits) {
        assert_eq!(ha, hb);
        assert!((a.x - (SIZE.0 - b.right())).abs() < 0.01, "{ha:?}");
    }
    // Every row has its own stepper, and the focus order ends with the buttons.
    for rv in &v.rows {
        assert!(l.hits.iter().any(|(_, h)| *h == Hit::Inc(rv.row)));
    }
    assert_eq!(
        v.focusables()[v.rows.len()..],
        [Hit::Cancel, Hit::Save, Hit::Print]
    );
}

#[test]
fn choices_step_and_wrap() {
    let mut c = sheet_choices();
    assert!(c.step(Row::Columns, 1));
    assert_eq!(c.cols, 5);
    c.cols = 10;
    assert!(!c.step(Row::Columns, 1), "columns stop at 10");
    assert!(c.step(Row::Paper, 1));
    assert_eq!(c.setup.paper, Paper::Letter);
    assert_eq!(c.unit, Unit::In, "the unit follows the paper");
    assert!(c.step(Row::Paper, -1));
    assert_eq!(c.unit, Unit::Cm);
    assert!(c.step(Row::Units, 1));
    assert!(c.step(Row::Paper, 1));
    assert_eq!(c.unit, Unit::In, "a unit chosen by hand stays");
    c.setup.margin = 0.0;
    assert!(!c.step(Row::Margins, -1), "no negative margins");
    assert!(c.step(Row::Resolution, 1));
    assert_eq!(c.setup.dpi, 600);
    assert!(c.step(Row::Resolution, 1));
    assert_eq!(c.setup.dpi, 150);
    c.copies = 1;
    assert!(!c.step(Row::Copies, -1));
    // The rows follow the layout: an exact width shows the width row.
    c.sheet = false;
    assert!(!c.rows().contains(&Row::Width));
    // Back from fit: to scale (with its scale row), then an exact width.
    c.step(Row::Size, -1);
    assert_eq!(c.fit, Fit::Scale { ratio: None });
    assert!(c.rows().contains(&Row::Scale));
    assert!(c.step(Row::Scale, 1));
    assert_eq!(c.fit, Fit::Scale { ratio: Some(1.0) });
    assert!(c.step(Row::Scale, -1) && c.step(Row::Scale, -1));
    assert_eq!(
        c.fit,
        Fit::Scale {
            ratio: Some(2500.0)
        },
        "wraps past as drawn"
    );
    let v = row_views(&c, String::new());
    let scale = v.iter().find(|r| r.row == Row::Scale).unwrap();
    assert_eq!(scale.value, "1:2500");
    c.step(Row::Size, -1);
    assert!(matches!(c.fit, Fit::Size { .. }));
    assert!(!c.rows().contains(&Row::Scale));
    assert!(c.rows().contains(&Row::Width));
    assert!(!c.rows().contains(&Row::Columns));
    assert!(c.step(Row::Width, 1));
    assert_eq!(c.fit, Fit::Size { width: c.width });
    // Captions are kept apart for each layout.
    c.step(Row::Captions, 1);
    assert_eq!(c.single_caption, Caption::Name);
    assert_eq!(c.sheet_caption, Caption::Name);
    assert!(matches!(
        c.kind(),
        Kind::Single {
            caption: Caption::Name,
            ..
        }
    ));
}

#[test]
fn a11y_nodes_map_back_to_controls() {
    let Some(mut h) = Harness::new() else { return };
    let c = sheet_choices();
    let mut v = view(&c, source_label(Source::Folder, 70), None);
    v.focus = Some(v.rows.len() + 1);
    let pal = Palette::dark();
    let mut d = None;
    h.render(SIZE, 1.0, backdrop, |dl, text| {
        d = Some(draw(dl, text, &pal, &v, SIZE, false, 1.0));
    });
    let d = d.unwrap();
    let mut tree = accesskit::TreeUpdate {
        nodes: vec![(
            accesskit::NodeId(1),
            accesskit::Node::new(accesskit::Role::Window),
        )],
        tree: None,
        tree_id: accesskit::TreeId::ROOT,
        focus: accesskit::NodeId(1),
    };
    a11y(&mut tree, &v, &d, 1.0);
    let focus = tree.focus.0;
    assert_eq!(a11y_target(&v, &d, focus), Some(Hit::Save));
    let spin = tree
        .nodes
        .iter()
        .filter(|(_, n)| n.role() == accesskit::Role::SpinButton)
        .count();
    assert_eq!(spin, v.rows.len());
    assert_eq!(
        a11y_target(&v, &d, A11Y_BASE + 1),
        Some(Hit::Row(Row::Images))
    );
    assert_eq!(tree.nodes[0].1.children(), &[accesskit::NodeId(A11Y_BASE)]);
}
