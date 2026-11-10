//! DjVu through the registry: probing, pages, scales and regions, the indirect-document
//! resolver, the G4 (Smmr) mask hook, embedded thumbnails and the info facts. Fixtures and
//! DjVuLibre reference renders come from `specular-djvu`.

use specular_core::{CancelToken, PixelLayout, Raster, Source};
use specular_decode::{DecodeError, DecodeRequest, Format, Rect, Registry};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../formats/specular-djvu/fixtures")
        .join(name)
}

fn open(name: &str) -> specular_decode::Result<Arc<dyn specular_decode::Session>> {
    Registry::default().open(Arc::new(Source::open(fixture(name)).unwrap()))
}

/// RGB samples of an 8-bit grey, planar RGB or RGBA raster.
fn rgb(r: &Raster) -> Vec<u8> {
    let (w, h) = (r.width as usize, r.height as usize);
    let at = |pl: usize, x: usize, y: usize, bpp: usize, k: usize| {
        let p = &r.planes[pl];
        p.data[y * p.stride + x * bpp + k]
    };
    let mut out = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            match r.layout {
                PixelLayout::Rgba => out.extend((0..3).map(|k| at(0, x, y, 4, k))),
                PixelLayout::Gray => out.extend([at(0, x, y, 1, 0); 3]),
                PixelLayout::Rgb => out.extend((0..3).map(|k| at(k, x, y, 1, 0))),
                l => panic!("layout {l:?}"),
            }
        }
    }
    out
}

/// A DjVuLibre reference PNG as RGB, through our PNG decoder.
fn reference(name: &str) -> (u32, u32, Vec<u8>) {
    let s = open(&format!("ref/{name}.png")).unwrap();
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    (r.width, r.height, rgb(&r))
}

#[test]
fn djvu_page_matches_ddjvu_and_has_pages() {
    let s = open("doc.djvu").unwrap();
    let i = s.info();
    assert_eq!(i.format, Format::Djvu);
    assert_eq!((i.width, i.height), (640, 400));
    assert!(
        i.coding.contains("bundled") && i.coding.contains("300 dpi"),
        "{}",
        i.coding
    );
    assert!(i.coding.contains("text layer"), "{}", i.coding);
    assert_eq!(s.page_count(), 4);
    let p2 = s.open_page(1).unwrap();
    assert_eq!(p2.page_index(), 1);
    let r = p2
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    let (w, h, want) = reference("doc_p2");
    assert_eq!((r.width, r.height), (w, h));
    assert_eq!(rgb(&r), want);
    // The page's embedded thumbnail.
    let t = p2.decode_preview().unwrap().unwrap();
    assert!(t.width <= 48 && t.height <= 48);
    // Producer and title from the shared annotations' metadata.
    let xmp = String::from_utf8(s.embedded_xmp().unwrap()).unwrap();
    assert!(
        xmp.contains("Fixture document") && xmp.contains("DjVuLibre djvm"),
        "{xmp}"
    );
}

#[test]
fn djvu_regions_and_scales() {
    let s = open("photo.djvu").unwrap();
    let c = CancelToken::new();
    let whole = s.decode(&DecodeRequest::full(2), &c).unwrap();
    assert_eq!((whole.width, whole.height), (106, 79));
    let mut req = DecodeRequest::full(2);
    req.region = Some(Rect {
        x: 10,
        y: 20,
        w: 30,
        h: 15,
    });
    let part = s.decode(&req, &c).unwrap();
    assert_eq!((part.width, part.height, part.origin), (30, 15, (10, 20)));
    let (a, b) = (rgb(&part), rgb(&whole));
    for y in 0..15 {
        for x in 0..30 {
            let o = ((20 + y) * 106 + 10 + x) * 3;
            assert_eq!(&a[(y * 30 + x) * 3..][..3], &b[o..o + 3]);
        }
    }
}

#[test]
fn djvu_indirect_document_reads_its_sibling_files() {
    let s = open("indirect/index.djvu").unwrap();
    assert_eq!(s.page_count(), 4);
    let r = s
        .open_page(1)
        .unwrap()
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert_eq!(rgb(&r), reference("doc_p2").2);
}

#[test]
fn djvu_g4_mask_through_the_tiff_decoder() {
    let s = open("mmr.djvu").unwrap();
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert_eq!(rgb(&r), reference("mmr").2);
}

#[test]
fn djvu_refusals_are_plain() {
    let reg = Registry::default();
    let e = reg
        .open(Arc::new(Source::from_vec(
            "part.djvu",
            b"AT&TFORM\0\0\0\x04DJVI".to_vec(),
        )))
        .err()
        .unwrap();
    assert!(matches!(e, DecodeError::Unsupported(_)), "{e}");
    assert!(e.to_string().contains("shared part"), "{e}");
}

#[test]
fn xps_and_openxps_pages() {
    for (openxps, name) in [(false, "doc.xps"), (true, "doc.oxps")] {
        let data = specular_xps::fixture::package(
            openxps,
            &[
                (
                    96.0,
                    48.0,
                    r##"<Path Data="M 0,0 H 48 V 48 H 0 Z" Fill="#FF0000"/>"##,
                ),
                (192.0, 96.0, ""),
            ],
        );
        let s = Registry::default()
            .open(Arc::new(Source::from_vec(name, data.clone())))
            .unwrap();
        let i = s.info();
        assert_eq!(i.format, Format::Xps);
        let kind = if openxps {
            "OpenXPS, 72×36 pt"
        } else {
            "XPS, 72×36 pt"
        };
        assert!(i.coding.starts_with(kind), "{}", i.coding);
        assert_eq!(s.page_count(), 2);
        // 72×36 pt doubled up to a 2048 px long side: 32 px per point.
        assert_eq!((i.width, i.height), (2304, 1152));
        let mut req = DecodeRequest::full(8);
        req.region = Some(Rect {
            x: 100,
            y: 50,
            w: 200,
            h: 20,
        });
        let r = s.decode(&req, &CancelToken::new()).unwrap();
        let px = rgb(&r);
        // Left of x = 36 pt (144 px at 1/8) is red, right of it white.
        assert_eq!(r.width, 188, "clamped to the page");
        assert_eq!(&px[(10 * 188 + 10) * 3..][..3], [255, 0, 0]);
        assert_eq!(&px[(10 * 188 + 150) * 3..][..3], [255, 255, 255]);
        let xmp = String::from_utf8(s.embedded_xmp().unwrap()).unwrap();
        assert!(xmp.contains(specular_xps::fixture::TITLE), "{xmp}");
        // The content probe finds it without the extension.
        let s = Registry::default()
            .open(Arc::new(Source::from_vec("noext", data)))
            .unwrap();
        assert_eq!(s.info().format, Format::Xps);
    }
}

#[test]
fn cgm_pictures_are_pages() {
    let data = specular_cgm::fixture::two_pictures();
    let s = Registry::default()
        .open(Arc::new(Source::from_vec("drawing.cgm", data.clone())))
        .unwrap();
    let i = s.info();
    assert_eq!(i.format, Format::Cgm);
    assert!(i.coding.starts_with("CGM (binary)"), "{}", i.coding);
    assert!(
        i.coding.contains("WebCGM 2.0") && i.coding.contains("\"metric\""),
        "{}",
        i.coding
    );
    assert_eq!(s.page_count(), 2);
    // 283.46×141.73 pt at 576 dpi (doubled until the long side reaches 2048 px).
    assert_eq!((i.width, i.height), (2268, 1134));
    let r = s
        .decode(&DecodeRequest::full(8), &CancelToken::new())
        .unwrap();
    let px = rgb(&r);
    let at = |x: usize, y: usize| px[(y * r.width as usize + x) * 3..][..3].to_vec();
    // At 1/8 the page is 1 px per point: the red square, and white paper.
    assert_eq!(at(70, 70), [255, 0, 0]);
    assert_eq!(at(5, 5), [255, 255, 255]);
    let p2 = s.open_page(1).unwrap();
    assert!(p2.info().width > p2.info().height);
    // Also found by content under another name.
    let s = Registry::default()
        .open(Arc::new(Source::from_vec("noext", data)))
        .unwrap();
    assert_eq!(s.info().format, Format::Cgm);
}
