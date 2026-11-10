//! Fonts and EPUB books through the registry: probing by content and extension, a
//! collection's faces as pages, the font model and preview raster, a book's pages from the
//! cover, its contents, chapter labels, regions, fixed-layout pages and the refusal of
//! locked books. Fixtures come from `specular-font` and `specular-epub`.

use specular_core::{CancelToken, PixelLayout, Source};
use specular_decode::{DecodeError, DecodeRequest, Format, PageLabel, Rect, Registry};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn fixture(krate: &str, name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../formats")
        .join(krate)
        .join("fixtures")
        .join(name)
}

fn open(krate: &str, name: &str) -> specular_decode::Result<Arc<dyn specular_decode::Session>> {
    Registry::default().open(Arc::new(Source::open(fixture(krate, name)).unwrap()))
}

/// Opens the fixture's bytes under a name without its extension (content sniffing only).
fn open_unnamed(
    krate: &str,
    name: &str,
) -> specular_decode::Result<Arc<dyn specular_decode::Session>> {
    let bytes = std::fs::read(fixture(krate, name)).unwrap();
    Registry::default().open(Arc::new(Source::from_vec("unnamed", bytes)))
}

#[test]
fn fonts_are_found_by_content_and_extension() {
    for (name, format) in [
        ("sans.ttf", Format::TrueType),
        ("serif.otf", Format::OpenType),
        ("var-cff2.otf", Format::OpenType),
        ("pair.ttc", Format::FontCollection),
        ("pair.otc", Format::FontCollection),
        ("sans.woff", Format::Woff),
        ("sans.woff2", Format::Woff2),
        ("pair.woff2", Format::Woff2),
    ] {
        for s in [
            open("specular-font", name),
            open_unnamed("specular-font", name),
        ] {
            let s = s.unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(s.info().format, format, "{name}");
            let m = s.font().unwrap_or_else(|| panic!("{name}: no font model"));
            assert!(m.face().glyph_count > 0, "{name}");
            assert!(!s.info().coding.is_empty());
        }
    }
}

#[test]
fn a_collection_steps_its_faces() {
    for name in ["pair.ttc", "pair.otc", "pair.woff2"] {
        let s = open("specular-font", name).unwrap();
        assert_eq!(s.page_count(), 2, "{name}");
        let second = s.open_page(1).unwrap();
        assert_eq!(second.page_index(), 1);
        assert_eq!(second.font().unwrap().face, 1);
        let names: Vec<_> = [s.page_label(), second.page_label()]
            .into_iter()
            .map(|l| match l {
                Some(PageLabel::Face { name }) => name.to_string(),
                l => panic!("{name}: {l:?}"),
            })
            .collect();
        assert_ne!(names[0], names[1], "{name}: faces named alike");
        assert!(s.open_page(2).is_err());
    }
    // A single font has no face label.
    assert!(
        open("specular-font", "sans.ttf")
            .unwrap()
            .page_label()
            .is_none()
    );
}

#[test]
fn the_font_preview_is_opaque_and_cut_to_regions() {
    let s = open("specular-font", "serif.otf").unwrap();
    let (w, h) = (s.info().width, s.info().height);
    let cancel = CancelToken::new();
    let full = s.decode(&DecodeRequest::full(2), &cancel).unwrap();
    assert_eq!((full.width, full.height), (w / 2, h / 2));
    assert_eq!(full.layout, PixelLayout::Rgba);
    let px = &full.planes[0].data;
    assert!(px.chunks_exact(4).all(|p| p[3] == 255), "opaque");
    // Black glyphs on white: some ink.
    assert!(px.chunks_exact(4).any(|p| p[0] < 64));
    let r = Rect {
        x: 10,
        y: 20,
        w: 30,
        h: 40,
    };
    let part = s
        .decode(
            &DecodeRequest {
                region: Some(r),
                ..DecodeRequest::full(2)
            },
            &cancel,
        )
        .unwrap();
    assert_eq!((part.width, part.height, part.origin), (30, 40, (10, 20)));
    let row = |d: &[u8], stride: usize, x: usize, y: usize| d[y * stride + x * 4..][..4].to_vec();
    for (x, y) in [(0, 0), (29, 39), (12, 7)] {
        assert_eq!(
            row(&part.planes[0].data, part.planes[0].stride, x, y),
            row(px, full.planes[0].stride, x + 10, y + 20)
        );
    }
}

#[test]
fn garbage_with_a_font_name_is_refused() {
    let r = Registry::default().open(Arc::new(Source::from_vec(
        "broken.ttf",
        b"\x00\x01\x00\x00\x00\x05garbage".to_vec(),
    )));
    assert!(r.is_err());
}

#[test]
fn a_book_opens_at_its_cover_with_contents_and_chapters() {
    for name in ["reflow3.epub", "ncx2.epub"] {
        let s = open("specular-epub", name).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(s.info().format, Format::Epub);
        assert!(s.page_count() >= 2, "{name}: {} pages", s.page_count());
        assert_eq!(s.page_index(), 0);
        let c = s
            .contents()
            .unwrap_or_else(|| panic!("{name}: no contents"));
        assert!(!c.is_empty());
        // Every entry that leads somewhere leads to a page of the book, in order.
        let pages: Vec<u32> = c.iter().filter_map(|e| e.page).collect();
        assert!(!pages.is_empty(), "{name}");
        assert!(pages.iter().all(|&p| p < s.page_count()));
        assert!(pages.windows(2).all(|w| w[0] <= w[1]), "{name}: {pages:?}");
        // A page after the first chapter's start carries its title.
        let first = pages[0];
        let page = s.open_page(first).unwrap();
        match page.page_label() {
            Some(PageLabel::Chapter { title }) => assert!(!title.is_empty()),
            l => panic!("{name}: page {first} label {l:?}"),
        }
        let b = s.book().unwrap();
        assert!(b.title.is_some(), "{name}");
        assert_eq!(b.pages as u32, s.page_count());
        // Pages render opaque at every scale.
        let cancel = CancelToken::new();
        for scale in [1, 4] {
            let r = page.decode(&DecodeRequest::full(scale), &cancel).unwrap();
            assert_eq!(r.width, page.info().width.div_ceil(scale));
            assert!(r.planes[0].data.chunks_exact(4).all(|p| p[3] == 255));
        }
    }
}

#[test]
fn a_book_page_region_matches_the_whole_page() {
    let s = open("specular-epub", "reflow3.epub").unwrap();
    let p = s.open_page(s.page_count().min(2) - 1).unwrap();
    let cancel = CancelToken::new();
    let full = p.decode(&DecodeRequest::full(4), &cancel).unwrap();
    let r = Rect {
        x: 17,
        y: 33,
        w: 60,
        h: 50,
    };
    let part = p
        .decode(
            &DecodeRequest {
                region: Some(r),
                ..DecodeRequest::full(4)
            },
            &cancel,
        )
        .unwrap();
    let (fs, ps) = (full.planes[0].stride, part.planes[0].stride);
    let mut differ = 0;
    for y in 0..50usize {
        for x in 0..60usize {
            let a = &full.planes[0].data[(y + 33) * fs + (x + 17) * 4..][..4];
            let b = &part.planes[0].data[y * ps + x * 4..][..4];
            differ += usize::from(a.iter().zip(b).any(|(a, b)| a.abs_diff(*b) > 2));
        }
    }
    assert!(differ <= 30, "{differ} of 3000 pixels differ");
}

#[test]
fn fixed_layout_pages_keep_their_size() {
    let s = open("specular-epub", "fixed.epub").unwrap();
    assert!(s.book().unwrap().fixed_layout);
    assert!(s.page_count() >= 2);
    let cancel = CancelToken::new();
    for i in 0..s.page_count() {
        let p = s.open_page(i).unwrap();
        assert!(
            p.decode(&DecodeRequest::full(8), &cancel).is_ok(),
            "page {i}"
        );
    }
}

#[test]
fn locked_books_are_refused_by_name() {
    for (name, words) in [
        ("drm-adobe.epub", "Adobe"),
        ("drm-fairplay.epub", "FairPlay"),
        ("drm-lcp.epub", "LCP"),
        ("drm-unknown.epub", "unknown"),
    ] {
        match open("specular-epub", name) {
            Err(DecodeError::Unsupported(m)) => assert!(m.contains(words), "{name}: {m}"),
            Err(e) => panic!("{name}: {e:?}"),
            Ok(_) => panic!("{name} opened"),
        }
    }
}

#[test]
fn obfuscated_fonts_are_counted() {
    for name in ["fonts-idpf.epub", "fonts-adobe.epub"] {
        let s = open("specular-epub", name).unwrap();
        let b = s.book().unwrap();
        assert!(b.obfuscated_fonts >= 1, "{name}: {b:?}");
    }
}

#[test]
fn a_book_thumbnail_is_its_cover_without_layout() {
    let src = Arc::new(Source::open(fixture("specular-epub", "reflow3.epub")).unwrap());
    let s = Registry::default().open_for_thumbnail(src).unwrap();
    assert_eq!(s.info().format, Format::Epub);
    assert!(s.info().coding.contains("cover"), "{}", s.info().coding);
    assert!(s.contents().is_none(), "no pages laid out");
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert!(r.width > 0 && r.height > 0);
    // Anything else opens as usual.
    let f = Arc::new(Source::open(fixture("specular-font", "sans.ttf")).unwrap());
    assert!(
        Registry::default()
            .open_for_thumbnail(f)
            .unwrap()
            .font()
            .is_some()
    );
}
