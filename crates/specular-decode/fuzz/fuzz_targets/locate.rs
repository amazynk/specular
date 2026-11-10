//! Fuzz the metadata locators that read headers and boxes without decoding pixels: Exif
//! (`exif_tiff`, `read_exif_tiff`, including PSD image resources, CR3's merged CMT blocks and
//! the rebuilt IFDs of TIFF-based files), XMP and ICC profiles (`embedded_metadata`, including
//! PSD resources, CR3's XMP box and compressed HEIF items), JPEG XL's XMP (`jxl_xmp`) and the
//! Multi-Picture Format index of MPO pages (`mpo_pages`). Run from this directory with
//! `cargo +nightly fuzz run locate corpus/locate -- -dict=fuzz.dict`.
#![no_main]

use std::io::Cursor;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let exif = specular_decode::exif_tiff(data);
    // The reader-based walk must find what the slice-based one finds.
    let read = specular_decode::read_exif_tiff(Cursor::new(data));
    assert_eq!(exif.as_deref(), read.as_deref());
    let found = specular_decode::embedded_metadata(data);
    let _ = found.into_owned();
    let _ = specular_decode::jxl_xmp(data);
    // Every MPO page is a non-empty in-bounds range that starts at a JPEG header.
    for r in specular_decode::mpo_pages(data) {
        assert!(r.start < r.end && r.end <= data.len());
        assert!(data[r.start..].starts_with(&[0xFF, 0xD8, 0xFF]));
    }
});
