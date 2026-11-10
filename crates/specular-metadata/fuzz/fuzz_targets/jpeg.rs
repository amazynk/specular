#![no_main]
//! JPEG marker scan (EXIF APP1, XMP, chunked ICC APP2, C2PA APP11, frame facts) and the
//! report built from what it finds.

use libfuzzer_sys::fuzz_target;
use specular_metadata::{jpeg, FileFacts};

#[path = "common.rs"]
mod common;

fuzz_target!(|data: &[u8]| {
    let Ok(seg) = jpeg::scan(data) else { return };
    let input = seg.input(FileFacts::default());
    common::report(&input, common::localizer(data.len() as u8));
});
