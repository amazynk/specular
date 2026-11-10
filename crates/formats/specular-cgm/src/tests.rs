use crate::writer::{P, W};
use crate::*;

fn never() -> bool {
    false
}

/// Renders a whole picture at `k` pixels per point.
fn render_all(doc: &Document, page: usize, k: f64) -> (u32, u32, Vec<u8>) {
    let (w, h) = doc.page_size(page).unwrap();
    let (pw, ph) = ((w * k).round() as u32, (h * k).round() as u32);
    let px = doc
        .render(
            RenderRequest {
                page,
                scale_x: k,
                scale_y: k,
                x: 0,
                y: 0,
                width: pw,
                height: ph,
            },
            &never,
        )
        .unwrap();
    (pw, ph, px)
}

fn at(img: &(u32, u32, Vec<u8>), x: u32, y: u32) -> [u8; 3] {
    let i = ((y * img.0 + x) * 4) as usize;
    [img.2[i], img.2[i + 1], img.2[i + 2]]
}

/// A 100×100 VDC picture: a red rectangle on the left half, a blue polyline on the right.
fn basic() -> Vec<u8> {
    let mut w = W::begin("basic");
    w.picture("one", [0, 0, 100, 100]);
    // Interior solid, fill colour index 2 (red in the default table).
    w.el(5, 22, P::new().e(1));
    w.el(5, 23, P::new().u8(2).align());
    w.el(4, 11, P::new().pt(0, 0).pt(50, 100));
    // A thick blue (index 4) vertical line at x = 75, absolute width mode not set: scaled 20.
    w.el(5, 4, P::new().u8(4).align());
    w.el(5, 3, P::new().r(20.0));
    w.el(4, 1, P::new().pt(75, 10).pt(75, 90));
    w.end_picture();
    w.finish()
}

#[test]
fn probes() {
    assert!(looks_like_cgm(&basic()));
    assert!(looks_like_cgm(b"  %comment% BEGMF 'x';"));
    assert!(looks_like_cgm(b"BEG_MF 'x';"));
    assert!(!looks_like_cgm(b"\x89PNG\r\n\x1a\n"));
    assert!(!looks_like_cgm(b"BEGIN"));
    // A QuickTime movie starting with a 2.1 MB `mdat` atom (samples.ffmpeg.org's
    // Minolta-MJPEG.mov) shares the binary header's first two bytes.
    assert!(!looks_like_cgm(
        b"\x00\x21\xd4\x00mdat\xff\xd8\xff\xdb\x00\x84"
    ));
}

#[test]
fn abstract_picture_fills_and_lines() {
    let doc = Document::open(&basic()).unwrap();
    assert_eq!(doc.page_count(), 1);
    assert_eq!(doc.page_size(0), Some((792.0, 792.0)));
    let img = render_all(&doc, 0, 0.25);
    assert_eq!((img.0, img.1), (198, 198));
    // VDC y is up: the rectangle covers the left half, top to bottom.
    assert_eq!(at(&img, 20, 20), [255, 0, 0]);
    assert_eq!(at(&img, 20, 180), [255, 0, 0]);
    assert_eq!(at(&img, 120, 100), [255, 255, 255]);
    assert_eq!(at(&img, 148, 100), [0, 0, 255]);
    // The line ends 10 units from the top.
    assert_eq!(at(&img, 148, 5), [255, 255, 255]);
    let f = doc.facts();
    assert_eq!(f.encoding, "binary");
    assert_eq!(f.name.as_deref(), Some("basic"));
    assert_eq!(f.pictures, vec!["one".to_string()]);
    assert_eq!(f.colour, "indexed");
    assert!(f.skipped.is_empty());
}

#[test]
fn regions_match_the_whole() {
    let doc = Document::open(&basic()).unwrap();
    let whole = render_all(&doc, 0, 0.5);
    let (x, y, w, h) = (100, 150, 120, 90);
    let part = doc
        .render(
            RenderRequest {
                page: 0,
                scale_x: 0.5,
                scale_y: 0.5,
                x,
                y,
                width: w,
                height: h,
            },
            &never,
        )
        .unwrap();
    for j in 0..h {
        for i in 0..w {
            let a = ((j * w + i) * 4) as usize;
            let b = (((y + j) * whole.0 + x + i) * 4) as usize;
            assert_eq!(part[a..a + 4], whole.2[b..b + 4], "at {i},{j}");
        }
    }
}

#[test]
fn metric_size_and_direct_colour() {
    let mut w = W::begin("metric");
    w.el(0, 3, P::new().s("a4"));
    // Metric, 1 mm per VDC unit.
    w.el(2, 1, P::new().e(1).r(1.0));
    w.el(2, 2, P::new().e(1));
    w.el(2, 6, P::new().pt(0, 0).pt(210, 297));
    w.el(2, 7, P::new().rgb(0, 255, 0));
    w.el(0, 4, P::new());
    w.el(5, 22, P::new().e(1));
    w.el(5, 23, P::new().rgb(10, 20, 30));
    w.el(4, 12, P::new().pt(105, 148).i(50));
    w.end_picture();
    let doc = Document::open(&w.finish()).unwrap();
    let (pw, ph) = doc.page_size(0).unwrap();
    assert!((pw - 210.0 * 72.0 / 25.4).abs() < 1e-6, "{pw}");
    assert!((ph - 297.0 * 72.0 / 25.4).abs() < 1e-6, "{ph}");
    let img = render_all(&doc, 0, 0.5);
    assert_eq!(at(&img, 5, 5), [0, 255, 0]);
    let c = (img.0 / 2, img.1 / 2);
    assert_eq!(at(&img, c.0, c.1), [10, 20, 30]);
    let f = doc.facts();
    assert!(f.metric);
    assert_eq!(f.colour, "direct");
}

#[test]
fn colour_table_and_cell_arrays() {
    let mut w = W::begin("cells");
    w.picture("cells", [0, 0, 100, 100]);
    // Colour table from index 10: two colours.
    w.el(5, 34, P::new().u8(10).rgb(200, 0, 0).rgb(0, 0, 200).align());
    // A 2×2 packed cell array over the bottom-left quarter: P (0,0), Q (50,50), R (50,0).
    w.el(
        4,
        9,
        P::new()
            .pt(0, 0)
            .pt(50, 50)
            .pt(50, 0)
            .i(2)
            .i(2)
            .i(0)
            .e(1)
            .u8(10)
            .u8(11)
            .u8(11)
            .u8(10),
    );
    // A run-length cell array over the top-right quarter, 4 cells wide, 1 row: 3×10, 1×11.
    w.el(
        4,
        9,
        P::new()
            .pt(50, 100)
            .pt(100, 50)
            .pt(100, 100)
            .i(4)
            .i(1)
            .i(0)
            .e(0)
            .i(3)
            .u8(10)
            .i(1)
            .u8(11)
            .align(),
    );
    w.end_picture();
    let doc = Document::open(&w.finish()).unwrap();
    let img = render_all(&doc, 0, 0.25);
    // Page 198 px: the first cell array's first row (y = 0..25 VDC) is the bottom of the page.
    assert_eq!(at(&img, 20, 190), [200, 0, 0]);
    assert_eq!(at(&img, 70, 190), [0, 0, 200]);
    assert_eq!(at(&img, 20, 120), [0, 0, 200]);
    assert_eq!(at(&img, 70, 120), [200, 0, 0]);
    // The run-length array: first three quarters red, last quarter blue.
    assert_eq!(at(&img, 110, 50), [200, 0, 0]);
    assert_eq!(at(&img, 190, 50), [0, 0, 200]);
}

/// Four 4096 × 4096 cell arrays holding one cell each (256 MiB of transparent pixels
/// before) are not drawn.
#[test]
fn sparse_cell_arrays_are_skipped() {
    let mut w = W::begin("sparse");
    w.picture("sparse", [0, 0, 100, 100]);
    for _ in 0..4 {
        w.el(
            4,
            9,
            P::new()
                .pt(0, 0)
                .pt(50, 50)
                .pt(50, 0)
                .i(4096)
                .i(4096)
                .i(0)
                .e(1)
                .u8(10)
                .align(),
        );
    }
    w.end_picture();
    let doc = Document::open(&w.finish()).unwrap();
    let mut e = W::begin("empty");
    e.picture("empty", [0, 0, 100, 100]);
    e.end_picture();
    let empty = Document::open(&e.finish()).unwrap();
    assert_eq!(doc.op_count(0), empty.op_count(0));
}

#[test]
fn several_pictures_are_pages() {
    let mut w = W::begin("two");
    w.picture("first", [0, 0, 100, 50]);
    w.end_picture();
    w.picture("second", [0, 0, 50, 100]);
    w.end_picture();
    let doc = Document::open(&w.finish()).unwrap();
    assert_eq!(doc.page_count(), 2);
    assert_eq!(doc.page_size(0), Some((792.0, 396.0)));
    assert_eq!(doc.page_size(1), Some((396.0, 792.0)));
    assert_eq!(doc.facts().pictures, vec!["first", "second"]);
    assert!(matches!(
        doc.render(
            RenderRequest {
                page: 2,
                scale_x: 1.0,
                scale_y: 1.0,
                x: 0,
                y: 0,
                width: 1,
                height: 1
            },
            &never
        ),
        Err(RenderError::NoSuchPage(2))
    ));
}

#[test]
fn attributes_reset_between_pictures() {
    let mut w = W::begin("reset");
    w.picture("a", [0, 0, 100, 100]);
    w.el(5, 22, P::new().e(1));
    w.el(5, 23, P::new().u8(2).align());
    w.end_picture();
    w.picture("b", [0, 0, 100, 100]);
    // Default interior is hollow: no fill.
    w.el(4, 11, P::new().pt(0, 0).pt(100, 100));
    w.end_picture();
    let doc = Document::open(&w.finish()).unwrap();
    let img = render_all(&doc, 1, 0.25);
    assert_eq!(at(&img, 100, 100), [255, 255, 255]);
}

#[test]
fn defaults_replacement_sets_real_vdc() {
    let mut w = W::begin("mdr");
    // VDC TYPE real inside METAFILE DEFAULTS REPLACEMENT, with a real VDC extent 0..1.
    let mut inner = W::new();
    inner.el(1, 3, P::new().e(1));
    w.el(1, 12, P::new().raw(&inner.0));
    w.el(0, 3, P::new().s("p"));
    w.el(2, 6, P::new().r(0.0).r(0.0).r(2.0).r(1.0));
    w.el(0, 4, P::new());
    w.el(5, 22, P::new().e(1));
    w.el(4, 11, P::new().r(0.0).r(0.0).r(1.0).r(1.0));
    w.end_picture();
    let doc = Document::open(&w.finish()).unwrap();
    assert_eq!(doc.page_size(0), Some((792.0, 396.0)));
    assert!(doc.facts().vdc_real);
    let img = render_all(&doc, 0, 0.25);
    assert_eq!(at(&img, 50, 50), [0, 0, 0]);
    assert_eq!(at(&img, 150, 50), [255, 255, 255]);
}

#[test]
fn text_draws_ink_where_aligned() {
    let mut w = W::begin("text");
    w.picture("t", [0, 0, 1000, 1000]);
    w.el(5, 15, P::new().i(100));
    // Centred horizontally, half vertically.
    w.el(5, 18, P::new().e(2).e(3).r(0.0).r(0.0));
    w.el(4, 4, P::new().pt(500, 500).e(1).s("HHHH"));
    w.end_picture();
    let doc = Document::open(&w.finish()).unwrap();
    let img = render_all(&doc, 0, 0.25);
    let ink = |x0: u32, x1: u32, y0: u32, y1: u32| {
        let mut n = 0;
        for y in y0..y1 {
            for x in x0..x1 {
                if at(&img, x, y)[0] < 128 {
                    n += 1;
                }
            }
        }
        n
    };
    // 198 px page: text 20 px high centred at (99, 99).
    assert!(ink(80, 118, 90, 108) > 50);
    assert_eq!(ink(0, 30, 0, 198), 0);
    assert_eq!(ink(0, 198, 0, 70), 0);
}

#[test]
fn clear_text_matches_binary() {
    let ct = b"BEGMF 'basic'; mfversion 1;\n\
        BEGPIC 'one'; VDCEXT (0,0) (100,100); BEGPICBODY;\n\
        INTSTYLE SOLID; FILLCOLR 2; RECT (0,0) (50,100);\n\
        LINECOLR 4; LINEWIDTH 20.0; LINE (75,10) (75,90);\n\
        ENDPIC; ENDMF;";
    let a = Document::open(ct).unwrap();
    let b = Document::open(&basic()).unwrap();
    assert_eq!(a.facts().encoding, "clear text");
    assert_eq!(a.facts().name.as_deref(), Some("basic"));
    assert_eq!(render_all(&a, 0, 0.25), render_all(&b, 0, 0.25));
}

#[test]
fn refusals() {
    assert_eq!(Document::open(b"hello").unwrap_err(), OpenError::NotCgm);
    assert_eq!(
        Document::open(&[0x30, 0x20, 0x41]).unwrap_err(),
        OpenError::CharacterEncoding
    );
    let mut w = W::begin("empty");
    assert_eq!(
        Document::open(&w.finish()).unwrap_err(),
        OpenError::NoPictures
    );
    assert_eq!(
        OpenError::CharacterEncoding.to_string(),
        "CGM in the character encoding: not supported, only binary and clear text"
    );
}

#[test]
fn truncated_files_draw_what_came_before() {
    let full = basic();
    // Cut inside the polyline element.
    let cut = &full[..full.len() - 10];
    let doc = Document::open(cut).unwrap();
    assert!(doc.facts().truncated);
    let img = render_all(&doc, 0, 0.25);
    assert_eq!(at(&img, 20, 20), [255, 0, 0]);
}

#[test]
fn hostile_lists_do_not_panic() {
    let mut w = W::begin("hostile");
    w.picture("h", [0, 0, 100, 100]);
    // A cell array that claims 60000×60000 cells with four bytes of data.
    w.el(
        4,
        9,
        P::new()
            .pt(0, 0)
            .pt(100, 100)
            .pt(100, 0)
            .i(30000)
            .i(30000)
            .i(0)
            .e(1)
            .raw(&[1, 2, 3, 4]),
    );
    // A run-length array with a zero run.
    w.el(
        4,
        9,
        P::new()
            .pt(0, 0)
            .pt(100, 100)
            .pt(100, 0)
            .i(4)
            .i(4)
            .i(0)
            .e(0)
            .i(0)
            .u8(1)
            .align(),
    );
    // Degenerate geometry.
    w.el(4, 12, P::new().pt(10, 10).i(0));
    w.el(4, 17, P::new().pt(10, 10).pt(10, 10).pt(10, 10));
    w.el(4, 13, P::new().pt(0, 0).pt(1, 1).pt(2, 2));
    w.el(5, 15, P::new().i(-32768));
    w.el(4, 4, P::new().pt(0, 0).e(1).raw(&[255, 0xff, 0xff]));
    w.el(4, 1, P::new().pt(1, 1));
    w.end_picture();
    let doc = Document::open(&w.finish()).unwrap();
    let _ = render_all(&doc, 0, 0.1);
    // Random bytes after a valid header never panic.
    let mut seed = 0x1234_5678u32;
    for _ in 0..200 {
        let mut d = basic();
        for _ in 0..8 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let i = 2 + (seed as usize >> 8) % (d.len() - 2);
            d[i] = (seed >> 3) as u8;
        }
        if let Ok(doc) = Document::open(&d) {
            for p in 0..doc.page_count().min(2) {
                let (pw, ph) = doc.page_size(p).unwrap();
                let k = 64.0 / pw.max(ph);
                let _ = doc.render(
                    RenderRequest {
                        page: p,
                        scale_x: k,
                        scale_y: k,
                        x: 0,
                        y: 0,
                        width: 64,
                        height: 64,
                    },
                    &never,
                );
            }
        }
    }
}

#[test]
fn cancel_and_bounds() {
    let doc = Document::open(&basic()).unwrap();
    let req = RenderRequest {
        page: 0,
        scale_x: 1.0,
        scale_y: 1.0,
        x: 0,
        y: 0,
        width: 10,
        height: 10,
    };
    assert_eq!(doc.render(req, &|| true), Err(RenderError::Cancelled));
    assert_eq!(
        doc.render(
            RenderRequest {
                width: 70_000,
                ..req
            },
            &never
        ),
        Err(RenderError::TooLarge(70_000, 10))
    );
}

#[test]
fn profile_from_description() {
    assert_eq!(
        profile("\"ProfileId:WebCGM\" \"ProfileEd:2.0\" \"Source:x\"").as_deref(),
        Some("WebCGM 2.0")
    );
    assert_eq!(profile("plain"), None);
}

#[test]
fn hatch_and_edges() {
    let mut w = W::begin("hatch");
    w.picture("h", [0, 0, 100, 100]);
    w.el(5, 22, P::new().e(3));
    w.el(5, 24, P::new().i(1));
    w.el(5, 30, P::new().e(1));
    w.el(5, 29, P::new().u8(2).align());
    w.el(4, 7, P::new().pt(10, 10).pt(90, 10).pt(90, 90).pt(10, 90));
    w.end_picture();
    let doc = Document::open(&w.finish()).unwrap();
    let img = render_all(&doc, 0, 0.5);
    // Horizontal hatch: some rows inside are inked, some are not; nothing outside.
    let row_ink = |y: u32| (40..350).filter(|&x| at(&img, x, y)[0] < 128).count();
    let inked = (100..300).filter(|&y| row_ink(y) > 200).count();
    assert!(inked > 5 && inked < 150, "{inked}");
    assert_eq!(at(&img, 20, 20), [255, 255, 255]);
    // The red edge on the left side (x = 10 VDC = 39.6 px, a thin line).
    let e = at(&img, 39, 200);
    assert!(e[0] > 200 && e[1] < 128, "{e:?}");
}

#[test]
fn fixture_two_pictures() {
    let doc = Document::open(&crate::fixture::two_pictures()).unwrap();
    assert_eq!(doc.page_count(), 2);
    let (w, h) = doc.page_size(0).unwrap();
    assert!((w - 100.0 * 72.0 / 25.4).abs() < 1e-6 && (h - 50.0 * 72.0 / 25.4).abs() < 1e-6);
    assert_eq!(doc.page_size(1), Some((792.0, 396.0)));
    let img = render_all(&doc, 0, 1.0);
    assert_eq!(at(&img, 70, 70), [255, 0, 0]);
    assert_eq!(at(&img, 5, 5), [255, 255, 255]);
    assert_eq!(doc.facts().profile.as_deref(), Some("WebCGM 2.0"));
    assert_eq!(doc.facts().pictures, ["metric", "abstract"]);
}
