//! Kindle, Mobipocket and FictionBook books through the registry (the fixtures of
//! specular-epub's tests/data): recognised by content, paged like EPUB, thumbnailed by their
//! cover; locked and KFX books refused in words.

use std::path::PathBuf;
use std::sync::Arc;

use specular_core::{CancelToken, Source};
use specular_decode::{DecodeRequest, Format, Registry};

fn data(name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../formats/specular-epub/tests/data")
            .join(name),
    )
    .unwrap()
}

fn source(name: &str, bytes: Vec<u8>) -> Arc<Source> {
    Arc::new(Source::from_vec(name, bytes))
}

#[test]
fn books_open_as_pages() {
    for (file, format) in [
        ("ebook-mobi6.mobi", Format::Mobi),
        ("ebook-joint.mobi", Format::Mobi),
        ("ebook-kf8.azw3", Format::Mobi),
        ("ebook.fb2", Format::Fb2),
        ("ebook-zip.fb2.zip", Format::Fb2),
    ] {
        // By content: the name says nothing.
        let s = Registry::default()
            .open(source("book.bin", data(file)))
            .unwrap_or_else(|e| panic!("{file}: {e}"));
        assert_eq!(s.info().format, format, "{file}");
        assert!(s.page_count() >= 4, "{file}: {}", s.page_count());
        let m = s.book().expect("book facts");
        assert_eq!(m.title.as_deref(), Some("The Lighthouse Keeper"), "{file}");
        assert_eq!(m.authors, ["Mara Quill"], "{file}");
        let toc = s.contents().unwrap();
        assert!(
            toc.iter().any(|e| &*e.title == "Storm" && e.page.is_some()),
            "{file}"
        );
        let r = s
            .decode(&DecodeRequest::full(8), &CancelToken::new())
            .unwrap();
        assert!(r.width > 0 && r.height > 0);
        // The thumbnail is the cover, without laying out pages.
        let t = Registry::default()
            .open_for_thumbnail(source(file, data(file)))
            .unwrap();
        assert_eq!((t.info().width, t.info().height), (120, 160), "{file}");
        assert!(
            t.info().coding.ends_with("cover"),
            "{file}: {}",
            t.info().coding
        );
    }
}

#[test]
fn locked_and_kfx_books_are_refused() {
    // The MOBI's encryption field (record 0, byte 12) set: Mobipocket DRM.
    let mut d = data("ebook-mobi6.mobi");
    let rec0 = u32::from_be_bytes(d[78..82].try_into().unwrap()) as usize;
    d[rec0 + 13] = 2;
    let e = Registry::default()
        .open(source("locked.azw", d))
        .err()
        .unwrap();
    assert!(e.to_string().contains("Kindle DRM"), "{e}");
    let e = Registry::default()
        .open(source(
            "book.kfx",
            b"CONT\x02\x00\x08\x00\x00\x00rest".to_vec(),
        ))
        .err()
        .unwrap();
    assert!(e.to_string().contains("KFX"), "{e}");
}
