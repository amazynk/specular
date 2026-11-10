//! Fuzz Radiance RGBE/XYZE and PFM: header parsing, every scanline encoding (new/old RLE,
//! flat), both output layouts, serial and parallel. Small `Limits` keep runs cheap. Run from
//! this directory with `cargo +nightly fuzz run decode corpus/decode -- -dict=fuzz.dict`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_radiance::{decode, detect, read_info, DecodeOptions, Limits, OutputChannels};

fuzz_target!(|data: &[u8]| {
    let _ = detect(data);
    let Ok(info) = read_info(data) else { return };
    assert!(
        info.data_offset <= data.len(),
        "pixel data offset past the end"
    );
    let limits = Limits {
        max_width: 1 << 14,
        max_height: 1 << 14,
        max_pixels: 1 << 20,
    };
    for (channels, parallel) in [
        (OutputChannels::Rgba, data.len() % 2 == 1),
        (OutputChannels::Rgb, false),
    ] {
        let opts = DecodeOptions {
            channels,
            limits,
            parallel,
        };
        if let Ok(img) = decode(data, &opts) {
            assert_eq!(
                img.data.len(),
                img.width as usize * img.height as usize * channels.count()
            );
        }
    }
});
