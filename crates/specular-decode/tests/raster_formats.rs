//! DPX, Cineon, XWD, MIFF, TIM, WSQ, JBIG, JBIG2 and JPEG XS through the registry: each fixture of the
//! format crates is recognised by content (under a neutral name too, where the format has a
//! signature), opens with its header facts and decodes at every level.

use specular_core::{CancelToken, PixelLayout, Source};
use specular_decode::{DecodeRequest, Format, Registry};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn fixture(krate: &str, name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../formats")
        .join(krate)
        .join("tests/fixtures")
        .join(name)
}

fn open_as(path: &Path, name: &str) -> Arc<dyn specular_decode::Session> {
    let data = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    Registry::new()
        .open(Arc::new(Source::from_vec(name, data)))
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// (crate, fixture, format, has a signature).
const CASES: &[(&str, &str, Format, bool)] = &[
    ("specular-film", "im-rgb10-be.dpx", Format::Dpx, true),
    ("specular-film", "im-grey16-le.dpx", Format::Dpx, true),
    ("specular-film", "im-log10.dpx", Format::Dpx, true),
    ("specular-film", "oiio-rgb10-methodb.dpx", Format::Dpx, true),
    ("specular-film", "im-rgb10.cin", Format::Cineon, true),
    ("specular-legacy2", "rose.xwd", Format::Xwd, true),
    ("specular-legacy2", "rose-bzip-16.miff", Format::Miff, true),
    ("specular-legacy2", "rose-zip-pal.miff", Format::Miff, true),
    ("specular-legacy2", "tim8.tim", Format::Tim, false),
    ("specular-wsq", "jnbis-sample.wsq", Format::Wsq, true),
    ("specular-jbig", "rose_fs_d3.jbg", Format::Jbig, false),
    ("specular-jbig2", "annex-h.jbig2", Format::Jbig2, true),
    ("specular-jbig2", "bitmap-mmr.jbig2", Format::Jbig2, true),
    ("specular-jpegxs", "main422-10.jxs", Format::JpegXs, true),
    (
        "specular-jpegxs",
        "main422-10-file.jxs",
        Format::JpegXs,
        true,
    ),
    (
        "specular-jpegxs",
        "mainbayer-grbg.jxs",
        Format::JpegXs,
        true,
    ),
    ("specular-jpegxs", "high4444-12.jxs", Format::JpegXs, true),
    ("specular-jpegxs", "main422-10.jxsi", Format::JpegXs, true),
    (
        "specular-jpegxs",
        "main420-12-idat.jxsi",
        Format::JpegXs,
        true,
    ),
    (
        "specular-jpegxs",
        "high444-12-turned.jxsi",
        Format::JpegXs,
        true,
    ),
];

#[test]
fn every_fixture_opens_and_decodes() {
    for &(krate, name, format, signed) in CASES {
        let path = fixture(krate, name);
        let s = open_as(&path, name);
        let i = s.info();
        assert_eq!(i.format, format, "{name}");
        assert!(i.width > 0 && i.height > 0, "{name}");
        assert!(!i.coding.is_empty(), "{name}");
        assert!(s.colour_model().is_some(), "{name}");
        for scale in [1, 2, 8] {
            let r = s
                .decode(&DecodeRequest::full(scale), &CancelToken::new())
                .unwrap_or_else(|e| panic!("{name} at 1/{scale}: {e}"));
            assert_eq!(
                (r.width, r.height),
                (i.width.div_ceil(scale), i.height.div_ceil(scale)),
                "{name}"
            );
            assert!(!r.truncated, "{name}");
        }
        if signed {
            // Recognised by content alone.
            let s = open_as(&path, "unnamed.bin");
            assert_eq!(s.info().format, format, "{name} by content");
        }
    }
}

/// A HEIF file with a `jxs1` primary item is JPEG XS, not HEIC or a movie; its `irot` and
/// `imir` become the orientation and its `clap` the size.
#[test]
fn jpeg_xs_in_heif_is_recognised_and_oriented() {
    let s = open_as(
        &fixture("specular-jpegxs", "high444-12-turned.jxsi"),
        "c.heif",
    );
    let i = s.info();
    assert_eq!(i.format, Format::JpegXs);
    assert_eq!((i.width, i.height), (48, 24));
    assert_eq!(i.orientation, specular_core::Orientation::from_exif(7));
    let h = s.header().unwrap();
    assert!(
        h.fields
            .iter()
            .any(|(k, v)| k == "Container" && v.contains("jxs1")),
        "{:?}",
        h.fields
    );
}

#[test]
fn jpeg_xs_carries_profile_level_and_transform() {
    let s = open_as(&fixture("specular-jpegxs", "main422-10-file.jxs"), "a.jxs");
    assert_eq!(s.info().bit_depth, 10);
    assert_eq!(s.colour_model(), Some("YCbCr"));
    let h = s.header().expect("JPEG XS header facts");
    let get = |k: &str| {
        h.summary
            .iter()
            .find(|(key, _)| *key == k)
            .map(|(_, v)| v.clone())
    };
    assert_eq!(get("profile").as_deref(), Some("Main 422.10"));
    assert_eq!(
        get("decomposition").as_deref(),
        Some("5 horizontal, 1 vertical")
    );
    assert_eq!(get("colour-transform").as_deref(), Some("None"));
    assert!(get("level").is_some());
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert!(matches!(r.layout, PixelLayout::Rgba16(_)), "{:?}", r.layout);

    let s = open_as(&fixture("specular-jpegxs", "mainbayer-grbg.jxs"), "b.jxs");
    let h = s.header().unwrap();
    assert!(
        h.summary
            .iter()
            .any(|(k, v)| *k == "colour-transform" && v.contains("Star-Tetrix")),
        "{:?}",
        h.summary
    );
}

#[test]
fn film_scans_are_wide_and_carry_header_facts() {
    let s = open_as(&fixture("specular-film", "im-log10.dpx"), "a.dpx");
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    // Printing density becomes linear light.
    assert!(
        matches!(r.layout, PixelLayout::Rgba16F(_)),
        "{:?}",
        r.layout
    );
    let h = s.header().expect("DPX header facts");
    assert!(!h.fields.is_empty());
    assert!(h.summary.iter().any(|(k, _)| *k == "transfer"), "{h:?}");
    let s = open_as(&fixture("specular-film", "im-grey16-le.dpx"), "g.dpx");
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert!(r.layout.is_wide(), "{:?}", r.layout);
}

#[test]
fn refusals_are_plain() {
    let path = fixture("specular-jbig", "refuse_dl1.jbg");
    let data = std::fs::read(path).unwrap();
    match Registry::new().open(Arc::new(Source::from_vec("a.jbg", data))) {
        Err(specular_decode::DecodeError::Unsupported(why)) => {
            assert!(why.contains("another file"), "{why}")
        }
        Err(e) => panic!("{e}"),
        Ok(_) => panic!("opened a JBIG continuation file"),
    }
}

#[test]
fn damaged_files_do_not_panic() {
    let reg = Registry::new();
    for &(krate, name, _, _) in CASES {
        let f = std::fs::read(fixture(krate, name)).unwrap();
        let step = (f.len() / 40).max(1);
        for n in (0..f.len()).step_by(step) {
            for data in [f[..n].to_vec(), {
                let mut d = f.clone();
                d[n] ^= 0x5a;
                d
            }] {
                if let Ok(s) = reg.open(Arc::new(Source::from_vec(name, data))) {
                    let _ = s.decode(&DecodeRequest::full(2), &CancelToken::new());
                }
            }
        }
    }
}
