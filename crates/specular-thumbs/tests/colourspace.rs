//! The colour space recorded with each file's header facts ([`meta_of`]): read from what the
//! decoders already know (CICP, ICC, the colour model, Exif), and spelled as the query's words.

use std::path::PathBuf;

use specular_decode::Registry;
use specular_thumbs::generate::meta_of;

#[test]
fn the_decoders_words_are_the_querys() {
    assert_eq!(specular_decode::COLOUR_SPACES, specular_query::SPACE_WORDS);
    for w in specular_decode::COLOUR_SPACES {
        assert_eq!(specular_query::space_word(w), Some(w));
    }
}

#[test]
fn fixtures_record_their_space() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/gpsmeta");
    for (name, want) in [
        ("gps.jpg", ""),
        ("gps.png", ""),
        ("gps.webp", ""),
        ("gps.heic", ""),
        ("gps.avif", ""),
        ("gps.jxl", "srgb"),
        ("gps.psd", ""),
        ("gps.tif", ""),
    ] {
        let s = Registry::new()
            .open_path(&dir.join(name))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let m = meta_of(s.as_ref());
        assert_eq!(m.space.as_deref(), Some(want), "{name}");
    }
}

/// In-tree images of the format crates (relative to the workspace root) and their spaces.
const SPACED: [(&str, &str); 17] = [
    ("crates/formats/specular-avif/tests/data/gray.avif", "grey"),
    ("crates/formats/specular-avif/tests/data/pq10.avif", "pq"),
    (
        "crates/specular-render/tests/data/colour_e2e/bt2020-pq.avif",
        "pq",
    ),
    (
        "crates/specular-render/tests/data/colour_e2e/bt709-limited.avif",
        "srgb",
    ),
    (
        "crates/specular-render/tests/data/colour_e2e/xyb.jxl",
        "srgb",
    ),
    ("crates/formats/specular-jpeg/fuzz/seeds/cmyk.jpg", "cmyk"),
    ("crates/formats/specular-jpeg/fuzz/seeds/gray.jpg", "grey"),
    ("crates/formats/specular-jpeg/fuzz/seeds/iccp.jpg", "srgb"),
    ("crates/formats/specular-jxl/tests/data/gray.jxl", "grey"),
    (
        "crates/formats/specular-jxl/tests/data/hdr_hlg_2020_alpha.jxl",
        "hlg",
    ),
    ("crates/formats/specular-jxl/tests/data/hdr_pq_p3.jxl", "pq"),
    (
        "crates/formats/specular-jxl/tests/data/linear_f16_2020_lossy.jxl",
        "Linear Rec. 2020",
    ),
    ("crates/formats/specular-png/fuzz/seeds/gray16.png", "grey"),
    ("crates/formats/specular-png/fuzz/seeds/iccp.png", "srgb"),
    ("crates/formats/specular-tiff/fuzz/seeds/cmyk.tif", "cmyk"),
    (
        "crates/formats/specular-tiff/tests/fixtures/lab/rose-lab8-icc.tif",
        "lab",
    ),
    ("crates/formats/specular-webp/fuzz/seeds/iccp.webp", "srgb"),
];

#[test]
fn format_crates_images_record_their_space() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut wrong = Vec::new();
    for (name, want) in SPACED {
        let s = Registry::new()
            .open_path(&root.join(name))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let m = meta_of(s.as_ref());
        if m.space.as_deref() != Some(want) {
            wrong.push(format!("{name}: {:?}, want {want:?}", m.space));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}
