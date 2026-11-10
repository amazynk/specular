//! JPEG XS thumbnails: codestreams, image files and HEIF `jxs1` items, YCbCr, RGB with alpha and Bayer mosaics.

use specular_core::CancelToken;
use specular_decode::{Format, Registry};
use specular_thumbs::generate::{GenOptions, generate};
use std::path::Path;

#[test]
fn jpeg_xs_thumbnails() {
    let dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../formats/specular-jpegxs/tests/fixtures");
    let c = CancelToken::new();
    for name in [
        "main422-10.jxs",
        "main422-10-file.jxs",
        "main420-12.jxs",
        "high4444-12.jxs",
        "highbayer-bggr.jxs",
        "grey-8.jxs",
        "main422-10.jxsi",
        "main420-12-idat.jxsi",
    ] {
        let s = Registry::new()
            .open_path(&dir.join(name))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(s.info().format, Format::JpegXs, "{name}");
        let px = generate(&*s, 32, GenOptions::default(), &c, &mut |_| {}).unwrap();
        assert_eq!((px.width, px.height), (32, 16), "{name}");
        assert_eq!(px.rgba.len(), 32 * 16 * 4, "{name}");
        // Not a flat picture.
        let first = &px.rgba[..4];
        assert!(px.rgba.chunks(4).any(|p| p != first), "{name}");
    }
}
