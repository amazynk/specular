//! Printing drawings to scale: a line drawn 100 mm long on the sheet prints 100 mm long, a
//! sheet larger than the paper tiles over several pages, and each page carries a scale bar
//! and a note.
//!
//! `SPECULAR_BLESS=1 cargo test -p specular-print --test to_scale` rewrites the golden.

mod support;

use std::path::Path;

use specular_core::{Orientation, PixelLayout, Raster, RasterPlane};
use specular_print::layout::{
    Caption, Fit, Item, ItemScale, Kind, PageOrientation, PageSetup, Paper, RectMm,
};
use specular_print::pdf::Meta;
use specular_print::{ImageSource, Loaded, PageRenderer, layout};
use specular_shots::{Goldens, Harness, Shot};

/// A white 1000 × 400 px sheet at 0.2 mm per pixel (200 × 80 mm as drawn) with a black
/// line 4 px thick from x = 100 to 600 at y = 200: 100 mm on the paper.
struct Sheet;

const W: u32 = 1000;
const H: u32 = 400;
const MM_PER_PX: f64 = 0.2;

impl ImageSource for Sheet {
    fn key(&self, _: usize) -> u64 {
        1
    }

    fn load(&self, _: usize, _: u32) -> Result<Loaded, String> {
        let mut data = vec![255u8; W as usize * H as usize * 4];
        for y in 198..202 {
            for x in 100..600 {
                let i = (y * W as usize + x) * 4;
                data[i..i + 3].copy_from_slice(&[0, 0, 0]);
            }
        }
        Ok(Loaded {
            raster: Raster {
                width: W,
                height: H,
                origin: (0, 0),
                scale: 1,
                layout: PixelLayout::Rgba,
                planes: vec![RasterPlane {
                    width: W,
                    height: H,
                    stride: W as usize * 4,
                    data,
                    h_div: 1,
                    v_div: 1,
                }],
                truncated: false,
            },
            stored: (W, H),
            orientation: Orientation(1),
            lut: None,
        })
    }
}

fn item() -> Item {
    Item {
        size: (W, H),
        name: "plan.pdf".into(),
        date: None,
        scale: Some(ItemScale {
            paper_mm_per_px: MM_PER_PX,
            ratio: 50.0,
            unit: "m".into(),
            unit_mm: 1000.0,
        }),
    }
}

fn setup(dpi: u32) -> PageSetup {
    PageSetup {
        paper: Paper::A4,
        orientation: PageOrientation::Landscape,
        margin: 10.0,
        dpi,
    }
}

fn single(ratio: Option<f64>) -> Kind {
    Kind::Single {
        fit: Fit::Scale { ratio },
        caption: Caption::Off,
    }
}

fn pdf_pages(r: &mut PageRenderer, doc: &specular_print::Document) -> Vec<support::ParsedPage> {
    let meta = Meta {
        title: "to scale".into(),
        date: String::new(),
        ..Default::default()
    };
    let (bytes, stats, _) = r
        .write_pdf(doc, &meta, Vec::new(), &Sheet, &mut |_| {}, &|| false)
        .unwrap();
    assert_eq!(stats.pages, doc.pages.len());
    support::parse(&bytes).pages
}

#[test]
fn as_drawn_is_actual_size_and_fits_one_page() {
    let doc = layout(setup(300), single(None), &[item()], "", &|_, _| {
        String::new()
    });
    assert_eq!(doc.pages.len(), 1);
    let p = &doc.pages[0].images[0];
    assert!((p.image.w - 200.0).abs() < 1e-9 && (p.image.h - 80.0).abs() < 1e-9);
    assert_eq!(p.clip, p.image, "nothing cropped");
    let m = doc.pages[0].scale.as_ref().expect("a scale bar");
    assert_eq!((m.ratio.as_str(), m.tiles), ("1:50", (1, 1)));
    // A third of the 277 mm width at 1:50 is 4.6 m: a 2 m bar, 40 mm long.
    assert_eq!(m.length, "2 m");
    assert!((m.bar.w - 40.0).abs() < 1e-9, "{}", m.bar.w);
}

#[test]
fn larger_scales_tile_over_pages() {
    // 1:12.5 is four times as large: 800 × 320 mm over 277 × 181 mm of each page.
    let doc = layout(setup(150), single(Some(12.5)), &[item()], "", &|_, _| {
        String::new()
    });
    assert_eq!(doc.pages.len(), 6);
    let mut covered = 0.0;
    for (i, page) in doc.pages.iter().enumerate() {
        let m = page.scale.as_ref().unwrap();
        assert_eq!(m.tiles, (2, 3));
        assert_eq!(m.tile, ((i / 3) as u32, (i % 3) as u32));
        let p = &page.images[0];
        assert!((p.image.w - 800.0).abs() < 1e-9 && (p.image.h - 320.0).abs() < 1e-9);
        covered += p.clip.w * p.clip.h;
        // Tiles stay inside the margins and above the strip.
        let area = RectMm::new(10.0, 10.0, 277.0, 190.0 - 9.0);
        assert!(area.contains(&p.clip), "{:?}", p.clip);
    }
    assert!((covered - 800.0 * 320.0).abs() < 1e-6, "{covered}");
    // The bar at 1:12.5: 1 m is 80 mm.
    let m = doc.pages[0].scale.as_ref().unwrap();
    assert_eq!((m.ratio.as_str(), m.length.as_str()), ("1:12.5", "1 m"));
    assert!((m.bar.w - 80.0).abs() < 1e-9);
    // Items without a scale are fitted on one page.
    let plain = Item {
        scale: None,
        ..item()
    };
    let doc = layout(setup(150), single(None), &[plain], "", &|_, _| {
        String::new()
    });
    assert_eq!(doc.pages.len(), 1);
    assert!(doc.pages[0].scale.is_none());
}

/// The longest run of dark pixels in a row of the image area, in pixels.
fn longest_dark_run(p: &support::ParsedPage, rows: std::ops::Range<usize>) -> usize {
    let w = p.width as usize;
    let mut best = 0;
    for y in rows {
        let mut run = 0;
        for x in 0..w {
            let i = (y * w + x) * 3;
            if p.rgb[i] < 128 {
                run += 1;
                best = best.max(run);
            } else {
                run = 0;
            }
        }
    }
    best
}

#[test]
fn a_100_mm_line_prints_100_mm_long() {
    let Some(h) = Harness::new() else { return };
    let mut r = PageRenderer::new(h.gpu.clone(), specular_gfx::Text::bundled_only());
    let dpi = 300;
    let doc = layout(setup(dpi), single(None), &[item()], "", &|_, _| {
        String::new()
    });
    let pages = pdf_pages(&mut r, &doc);
    let p = &pages[0];
    // A4 landscape at 300 dpi; the page box is the paper in points.
    assert_eq!((p.width, p.height), (3508, 2480));
    assert!((p.media_box[2] - 841.89).abs() < 0.01, "{:?}", p.media_box);
    // The rows of the line: the image's top plus 40 mm, ± a millimetre.
    let img = doc.pages[0].images[0].image;
    let px = |mm: f64| (mm * f64::from(dpi) / 25.4).round() as usize;
    let y = px(img.y + 40.0);
    let run = longest_dark_run(p, y - px(1.0)..y + px(1.0));
    let mm = run as f64 * 25.4 / f64::from(dpi);
    assert!((mm - 100.0).abs() <= 0.2, "the line prints {mm:.3} mm long");

    // At 1:100 it prints half as long.
    let doc = layout(setup(dpi), single(Some(100.0)), &[item()], "", &|_, _| {
        String::new()
    });
    let pages = pdf_pages(&mut r, &doc);
    let img = doc.pages[0].images[0].image;
    let y = px(img.y + 20.0);
    let run = longest_dark_run(&pages[0], y - px(1.0)..y + px(1.0));
    let mm = run as f64 * 25.4 / f64::from(dpi);
    assert!(
        (mm - 50.0).abs() <= 0.2,
        "at 1:100 the line prints {mm:.3} mm long"
    );
}

/// A PDF sheet with viewport measures probes as a drawing at 1:50 whose page prints at its
/// own size.
#[test]
fn pdf_sheets_probe_their_scale() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("to-scale");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("sheet.pdf");
    std::fs::write(&path, specular_pdf::fixture::drawing_scales()).unwrap();
    let src = specular_print::files::FileSource::new(
        std::sync::Arc::new(specular_decode::Registry::default()),
        vec![path],
    );
    let p = &src.probe()[0];
    let sc = p.scale.as_ref().expect("a drawing scale");
    assert!((sc.ratio - 50.0).abs() < 1e-6, "{}", sc.ratio);
    assert_eq!((sc.unit.as_str(), sc.unit_mm), ("m", 1000.0));
    let (w, _) = p.size.unwrap();
    let mm = sc.paper_mm_per_px * f64::from(w);
    assert!((mm - 800.0 * 25.4 / 72.0).abs() < 1e-6, "{mm}");
    // As drawn on A3 landscape: one page, the sheet at its own size.
    let item = Item {
        size: p.size.unwrap(),
        name: p.name.clone(),
        date: None,
        scale: p.scale.clone(),
    };
    let setup = PageSetup {
        paper: Paper::A3,
        ..setup(150)
    };
    let doc = layout(setup, single(None), &[item], "", &|_, _| String::new());
    assert_eq!(doc.pages.len(), 1);
    assert!((doc.pages[0].images[0].image.w - mm).abs() < 1e-9);
}

#[test]
fn to_scale_page_matches_golden() {
    let Some(h) = Harness::new() else { return };
    let mut r = PageRenderer::new(h.gpu.clone(), specular_gfx::Text::bundled_only());
    let mut doc = layout(setup(90), single(None), &[item()], "", &|_, _| {
        String::new()
    });
    // The app writes the note in the reader's language.
    if let Some(m) = doc.pages[0].scale.as_mut() {
        m.note = format!("plan.pdf — printed at {} on A4", m.ratio);
    }
    let pages = pdf_pages(&mut r, &doc);
    let shot = Shot {
        width: pages[0].width,
        height: pages[0].height,
        rgb: pages[0].rgb.clone(),
        language: "en".into(),
        strays: Vec::new(),
        signs: Vec::new(),
    };
    let mut g = Goldens::new(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/shots"),
        "specular-print",
    );
    g.check("to_scale_a4", &shot);
    g.finish();
}
