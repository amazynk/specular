//! Fuzz the ICNS container and its own icon decoders (RLE RGB + mask, ARGB, 1-bit): every
//! icon of a parsed file is decoded and must come out exactly its promised size. PNG and
//! JPEG 2000 payloads belong to those decoders' fuzzers. Run from this directory with
//! `cargo +nightly fuzz run decode corpus/decode -- -dict=fuzz.dict`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_icns::{decode, detect, parse};

fuzz_target!(|data: &[u8]| {
    let _ = detect(data);
    let Ok(file) = parse(data) else { return };
    for icon in file.icons.iter().take(64) {
        let _ = (icon.name(), icon.label());
        assert!(icon.data.end <= data.len());
        match decode(data, icon) {
            Ok(img) => {
                assert_eq!((img.width, img.height), (icon.width, icon.height));
                assert_eq!(
                    img.pixels.len(),
                    icon.width as usize * icon.height as usize * 4
                );
            }
            // PNG / JPEG 2000 payloads (`Error::Embedded`) and damaged icons.
            Err(_) => {}
        }
    }
});
