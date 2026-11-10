//! The synthetic books of `fixtures/` (written by `fixtures/make.py`): package, contents,
//! cover, pagination, rendering, font obfuscation, DRM refusals and damaged input.

mod common;

use std::sync::Arc;

use common::{Stub, read_fixture};
use specular_epub::{Book, Direction, DrmScheme, Error, Layout, Options, PageSetup, is_epub};

fn open(name: &str) -> Book {
    Book::open(read_fixture(name), &Options::default()).unwrap()
}

fn paginate(b: &Book) -> specular_epub::Pages {
    b.paginate(&PageSetup::default(), Arc::new(Stub), &|| false)
        .unwrap()
}

/// Parts read from the book keep their share of the memory budget as long as their bytes
/// live (a registered font as long as the pages), and a part the budget cannot hold is not
/// read at all.
#[test]
fn parts_keep_their_reservation() {
    use specular_core::budget::{MemoryBudget, with_budget};
    let font_len = read_fixture("sample.ttf").len() as u64;
    let budget = Arc::new(MemoryBudget::fixed(64 << 20));
    let base = with_budget(budget.clone(), || {
        let b = open("fonts-idpf.epub");
        let base = budget.reserved();
        let font = b.read("OEBPS/fonts/sample.ttf").unwrap();
        assert_eq!(font.reserved(), font_len);
        assert_eq!(budget.reserved(), base + font_len);
        drop(font);
        assert_eq!(budget.reserved(), base);
        let p = paginate(&b);
        assert!(p.font_data_lens().contains(&(font_len as usize)));
        assert!(
            budget.reserved() >= base + font_len,
            "the kept font is not counted"
        );
        drop(p);
        base
    });
    // Room for the book but not for the font: it is not read.
    let tight = Arc::new(MemoryBudget::fixed(base + font_len - 1));
    with_budget(tight, || {
        let b = open("fonts-idpf.epub");
        assert!(b.read("OEBPS/fonts/sample.ttf").is_none());
    });
}

fn ink(img: &specular_epub::Image) -> usize {
    img.rgba
        .chunks_exact(4)
        .filter(|p| p[0] < 200 || p[1] < 200 || p[2] < 200)
        .count()
}

#[test]
fn epub3_package_contents_and_cover() {
    assert!(is_epub(&read_fixture("reflow3.epub")));
    let b = open("reflow3.epub");
    let m = b.metadata();
    assert_eq!(m.title.as_deref(), Some("A Small Reflowable Book"));
    assert_eq!(m.version, "3.0");
    assert_eq!(m.creators.len(), 2);
    assert_eq!(m.creators[0].role.as_deref(), Some("aut"));
    assert_eq!(m.creators[0].file_as.as_deref(), Some("Writer, Ada"));
    assert_eq!(m.creators[1].role.as_deref(), Some("ill"));
    assert_eq!(m.languages, ["en"]);
    assert_eq!(
        m.identifier.as_deref(),
        Some("urn:uuid:8f1c3c2a-6a4e-4d6b-9a39-2f0a7d1e5b10")
    );
    assert_eq!(m.description.as_deref(), Some("A book for tests."));
    assert_eq!(m.series.as_deref(), Some("Fixture Series"));
    assert_eq!(m.modified.as_deref(), Some("2026-10-19T00:00:00Z"));
    assert_eq!(m.layout, Layout::Reflowable);
    // Non-linear documents come last.
    assert_eq!(
        b.spine(),
        [
            "OEBPS/text/ch1.xhtml",
            "OEBPS/text/ch2.xhtml",
            "OEBPS/text/notes.xhtml"
        ]
    );
    let toc: Vec<(&str, u32, Option<usize>, Option<&str>)> = b
        .toc()
        .iter()
        .map(|t| (t.title.as_str(), t.depth, t.spine, t.fragment.as_deref()))
        .collect();
    assert_eq!(
        toc,
        [
            ("Chapter One", 0, Some(0), None),
            ("Lists", 1, Some(0), Some("lists")),
            ("A table", 1, Some(0), Some("table")),
            ("Chapter Two", 0, Some(1), None),
            ("Deep in chapter two", 1, Some(1), Some("deep")),
            ("Back matter", 0, None, None),
            ("Notes", 1, Some(2), None),
        ]
    );
    assert_eq!(b.cover_path().as_deref(), Some("OEBPS/images/cover.png"));
    let (bytes, mt) = b.cover().unwrap();
    assert!(bytes.starts_with(b"\x89PNG"));
    assert_eq!(mt, "image/png");
}

#[test]
fn epub3_paginates_from_the_cover() {
    let b = open("reflow3.epub");
    let p = paginate(&b);
    assert!(p.len() >= 6, "{} pages", p.len());
    // The spine does not start with the cover: page 0 is the cover picture.
    assert_eq!(p.spine_of(0), None);
    assert_eq!(p.item_counts(0), Some((0, 1)));
    assert_eq!(p.spine_of(1), Some(0));
    let ch2 = p.page_of(1, None).unwrap();
    let deep = p.page_of(1, Some("deep")).unwrap();
    assert!(deep > ch2, "deep {deep} ch2 {ch2}");
    assert_eq!(p.toc_page(4), Some(deep));
    assert_eq!(p.toc_page(3), Some(ch2));
    assert_eq!(p.toc_page(5), None);
    let notes = p.page_of(2, None).unwrap();
    assert_eq!(notes, p.len() - 1);
    // Chapter one: text, two pictures (block and inline), a table.
    let (glyphs, images) = p.item_counts(1).unwrap();
    assert!(glyphs > 300, "{glyphs} glyphs");
    assert!(images >= 1);
    for i in 0..p.len() {
        assert_eq!(p.page_size(i), Some((600.0, 900.0)));
    }
    // Drawing: ink on the page, the same at the same scale, regions match the whole.
    let full = p.render(1, 1.0, None).unwrap();
    assert_eq!((full.width, full.height), (600, 900));
    assert!(ink(&full) > 2000);
    let again = p.render(1, 1.0, None).unwrap();
    assert_eq!(full, again);
    let region = p.render(1, 1.0, Some((100, 200, 50, 40))).unwrap();
    assert_eq!((region.width, region.height), (50, 40));
    for y in 0..40 {
        let a = &region.rgba[y * 50 * 4..(y + 1) * 50 * 4];
        let o = ((200 + y) * 600 + 100) * 4;
        let b2 = &full.rgba[o..o + 200];
        let diff = a
            .iter()
            .zip(b2)
            .filter(|(x, y)| x.abs_diff(**y) > 2)
            .count();
        assert!(diff < 8, "row {y}: {diff}");
    }
    let big = p.render(1, 2.0, None).unwrap();
    assert_eq!((big.width, big.height), (1200, 1800));
    assert!(p.render(p.len(), 1.0, None).is_err());
}

#[test]
fn epub2_ncx_guide_cover_and_latin1() {
    let b = open("ncx2.epub");
    let m = b.metadata();
    assert_eq!(m.version, "2.0");
    assert_eq!(m.title.as_deref(), Some("An EPUB 2 Book"));
    assert_eq!(m.creators[0].role.as_deref(), Some("aut"));
    assert_eq!(m.creators[0].file_as.as_deref(), Some("Author, Old"));
    assert_eq!(m.languages, ["fr"]);
    assert_eq!(m.series.as_deref(), Some("Old Series"));
    assert_eq!(b.spine(), ["OEBPS/cover.xhtml", "OEBPS/chapter 1.html"]);
    let toc: Vec<(&str, u32, Option<usize>)> = b
        .toc()
        .iter()
        .map(|t| (t.title.as_str(), t.depth, t.spine))
        .collect();
    assert_eq!(
        toc,
        [
            ("Cover", 0, Some(0)),
            ("Chapitre un", 0, Some(1)),
            ("Section", 1, Some(1))
        ]
    );
    assert_eq!(b.cover_path().as_deref(), Some("OEBPS/cover.png"));
    let p = paginate(&b);
    // The cover page is the spine's first document, shown as its picture.
    assert_eq!(p.spine_of(0), Some(0));
    assert_eq!(p.item_counts(0), Some((0, 1)));
    assert_eq!(p.len(), 2);
    assert_eq!(p.toc_page(2), Some(1));
    let (glyphs, _) = p.item_counts(1).unwrap();
    assert!(glyphs > 100);
}

#[test]
fn fixed_layout_pages_keep_their_size() {
    let b = open("fixed.epub");
    assert_eq!(b.metadata().layout, Layout::PrePaginated);
    assert_eq!(b.metadata().direction, Direction::Rtl);
    let p = paginate(&b);
    assert_eq!(p.len(), 4);
    for i in 0..4 {
        assert_eq!(p.page_size(i), Some((800.0, 1200.0)), "page {i}");
        assert_eq!(p.spine_of(i), Some(i));
    }
    assert_eq!(p.item_counts(0), Some((0, 1)));
    assert_eq!(p.item_counts(1), Some((0, 1)));
    assert_eq!(p.item_counts(2), Some((0, 1)));
    let (g, _) = p.item_counts(3).unwrap();
    assert!(g > 10);
    assert_eq!(p.toc_page(1), Some(3));
    let img = p.render(0, 0.25, None).unwrap();
    assert_eq!((img.width, img.height), (200, 300));
    // The picture fills the page.
    assert!(ink(&img) > 200 * 300 * 9 / 10);
}

#[test]
fn obfuscated_fonts_are_restored_and_used() {
    let original = read_fixture("sample.ttf");
    for name in ["fonts-idpf.epub", "fonts-adobe.epub"] {
        let b = open(name);
        assert_eq!(b.obfuscated_fonts(), ["OEBPS/fonts/sample.ttf"], "{name}");
        assert_eq!(
            b.read("OEBPS/fonts/sample.ttf").unwrap(),
            original,
            "{name}"
        );
        let p = paginate(&b);
        assert!(
            p.font_data_lens().contains(&original.len()),
            "{name}: the book's font is not used ({:?})",
            p.font_data_lens()
        );
    }
}

#[test]
fn locked_books_are_refused_by_scheme() {
    for (name, scheme) in [
        ("drm-adobe.epub", DrmScheme::Adobe),
        ("drm-fairplay.epub", DrmScheme::FairPlay),
        ("drm-lcp.epub", DrmScheme::Lcp),
        ("drm-unknown.epub", DrmScheme::Unknown),
    ] {
        let e = Book::open(read_fixture(name), &Options::default()).unwrap_err();
        assert_eq!(e, Error::Drm(scheme), "{name}");
        assert!(!e.to_string().is_empty());
    }
    let e = Error::Drm(DrmScheme::Adobe).to_string();
    assert_eq!(
        e,
        "the book is locked with Adobe DRM (ADEPT) and can only be read in the shop's own app"
    );
}

#[test]
fn right_to_left_book() {
    let b = open("rtl.epub");
    assert_eq!(b.metadata().direction, Direction::Rtl);
    assert_eq!(b.toc()[0].title, "פרק ראשון");
    let p = paginate(&b);
    assert_eq!(p.len(), 1);
    let (g, _) = p.item_counts(0).unwrap();
    assert!(g > 20);
    let img = p.render(0, 1.0, None).unwrap();
    // Right-aligned: more ink in the right half.
    let (mut l, mut r) = (0, 0);
    for (i, px) in img.rgba.chunks_exact(4).enumerate() {
        if px[0] < 128 {
            if (i % 600) < 300 { l += 1 } else { r += 1 }
        }
    }
    assert!(r > l * 2, "left {l} right {r}");
}

#[test]
fn not_books_and_damaged_books_never_panic() {
    assert_eq!(
        Book::open(b"not a zip".to_vec(), &Options::default()).unwrap_err(),
        Error::NotEpub
    );
    assert!(!is_epub(b"PK\x03\x04garbage"));
    for name in [
        "reflow3.epub",
        "ncx2.epub",
        "fixed.epub",
        "fonts-idpf.epub",
        "rtl.epub",
    ] {
        let full = read_fixture(name);
        let mut cut = 0;
        while cut < full.len() {
            let mut d = full[..cut].to_vec();
            if let Ok(b) = Book::open(d.clone(), &Options::default())
                && let Ok(p) = b.paginate(&PageSetup::default(), Arc::new(Stub), &|| false)
            {
                let _ = p.render(0, 0.5, None);
            }
            // Flip a byte inside too.
            if cut > 0 {
                d.extend_from_slice(&full[cut..]);
                d[cut / 2] ^= 0x5A;
                if let Ok(b) = Book::open(d, &Options::default())
                    && let Ok(p) = b.paginate(&PageSetup::default(), Arc::new(Stub), &|| false)
                {
                    let _ = p.render(p.len() - 1, 0.5, None);
                }
            }
            cut += 211;
        }
    }
}

#[test]
fn hostile_markup_stays_bounded() {
    let t0 = std::time::Instant::now();
    // Deep nesting, a long descendant selector, nested tables, absurd sizes.
    let deep = format!(
        "{}x{}",
        "<div><span>".repeat(3000),
        "</span></div>".repeat(3000)
    );
    let sel = "div ".repeat(16) + "span { color: red }";
    let mut tables = String::new();
    for _ in 0..8 {
        tables.push_str("<table><tr><td>a</td><td>b</td><td>");
    }
    for _ in 0..8 {
        tables.push_str("</td></tr></table>");
    }
    let body = format!(
        "{deep}{tables}<img src='x.png' width='99999999' height='1e30'/><p style='font-size:1e30px;margin:-1e30px;text-indent:1e9px'>big</p><ol start='-5'><li>x</li></ol>"
    );
    let b = Book::open(common::book(&body, &sel), &Options::default()).unwrap();
    let p = b
        .paginate(&PageSetup::default(), Arc::new(Stub), &|| false)
        .unwrap();
    for i in 0..p.len() {
        p.render(i, 0.5, None).unwrap();
    }
    assert!(t0.elapsed().as_secs() < 60, "{:?}", t0.elapsed());
}

#[test]
fn cancel_stops_pagination() {
    let b = open("reflow3.epub");
    let r = b.paginate(&PageSetup::default(), Arc::new(Stub), &|| true);
    assert_eq!(r.unwrap_err(), Error::Cancelled);
}

/// Elements nested as deep as the parser keeps (256) are laid out and drawn on a small
/// stack: layout and drawing stop descending at their own depth cap.
#[test]
fn deep_nesting_fits_a_small_stack() {
    let deep = format!(
        "{}x{}",
        "<div><table><tr><td><span>".repeat(1000),
        "</span></td></tr></table></div>".repeat(1000)
    );
    let b = Book::open(common::book(&deep, ""), &Options::default()).unwrap();
    specular_thread::on_stack(256 * 1024, || {
        let p = b
            .paginate(&PageSetup::default(), Arc::new(Stub), &|| false)
            .unwrap();
        for i in 0..p.len() {
            p.render(i, 0.5, None).unwrap();
        }
    });
}
