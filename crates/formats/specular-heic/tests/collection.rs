//! The top-level images of a HEIF collection are pages ([`HeicFile::page_count`]): the
//! primary first, then the further images, never thumbnails, alpha planes or grid tiles.
//!
//! `collection.heic` (specular-decode-host test data) was made with
//! `heif-enc -q 90 -t 8 -o collection.heic p1.png p2.png p3.png`: 32×24 red, 16×16 green and a
//! 40×20 translucent blue image, each with an 8-px thumbnail. The conformance files are used
//! when the corpus has been fetched (`tools/corpus/fetch-conformance.sh`).

use std::path::{Path, PathBuf};

use specular_heic::{DecodeOptions, HeicFile};

fn fixture(name: &str) -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../specular-decode-host/tests/data")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn conformance() -> PathBuf {
    std::env::var_os("SPECULAR_CONFORMANCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/conformance")
        })
}

#[test]
fn collection_images_are_pages() {
    let f = HeicFile::open(fixture("collection.heic")).unwrap();
    assert_eq!((f.page_count(), f.page()), (3, 0));
    let opts = DecodeOptions::default();
    let mut thumbs = Vec::new();
    let expect = [
        (32, 24, false, [220, 40, 40, 255]),
        (16, 16, false, [40, 200, 60, 255]),
        (40, 20, true, [30, 60, 220, 160]),
    ];
    for (n, (w, h, alpha, px)) in expect.into_iter().enumerate() {
        let p = f.reopen_page(n as u32).unwrap();
        assert_eq!((p.page(), p.page_count()), (n as u32, 3));
        let i = p.info();
        assert_eq!((i.width, i.height, i.has_alpha), (w, h, alpha), "page {n}");
        // Each page has its own 8-px thumbnail, not the primary's.
        let (id, tw, th) = p.thumbnail().unwrap();
        assert_eq!(tw.max(th), 8, "page {n} thumbnail");
        assert!(!thumbs.contains(&id));
        thumbs.push(id);
        let rgba = p.decode(&opts).unwrap().to_rgba8().unwrap();
        let at = ((h / 2 * w + w / 2) * 4) as usize;
        let got = &rgba[at..at + 4];
        assert!(
            got.iter().zip(px).all(|(a, b)| a.abs_diff(b) <= 12),
            "page {n}: {got:?}"
        );
    }
    assert!(f.reopen_page(3).is_err());
    // A single image (with a thumbnail and alpha) has one page.
    assert_eq!(
        HeicFile::open(fixture("alpha.heic")).unwrap().page_count(),
        1
    );
    assert_eq!(
        HeicFile::open(fixture("grad.heic")).unwrap().page_count(),
        1
    );
}

/// Real collections, stereo pairs and bursts: as many pages as libheif lists top-level
/// images (`heif-info`), each opening at its size.
#[test]
fn real_collections() {
    let base = conformance().join("nokia-heif/content");
    let cases = [
        ("images/season_collection_1440x960.heic", 4, (1440, 960)),
        ("images/random_collection_1440x960.heic", 4, (1440, 960)),
        ("heifv2/stereo_1200x800.heic", 2, (1200, 800)),
        ("image_sequences/bird_burst.heic", 4, (1280, 720)),
        ("image_sequences/rally_burst.heic", 4, (1280, 720)),
    ];
    let mut seen = 0;
    for (name, pages, size) in cases {
        let Ok(data) = std::fs::read(base.join(name)) else {
            continue;
        };
        seen += 1;
        let f = HeicFile::open(data).unwrap();
        assert_eq!(f.page_count(), pages, "{name}");
        for n in 0..pages {
            let p = f.reopen_page(n).unwrap();
            assert_eq!((p.info().width, p.info().height), size, "{name} page {n}");
        }
        // The last page decodes.
        let img = f
            .reopen_page(pages - 1)
            .unwrap()
            .decode(&DecodeOptions::default())
            .unwrap();
        assert_eq!((img.width(), img.height()), size, "{name}");
    }
    if seen == 0 {
        eprintln!("conformance corpus not fetched; skipped");
    }
}
