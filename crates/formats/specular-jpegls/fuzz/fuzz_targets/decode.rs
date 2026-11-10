//! Fuzz JPEG-LS: markers, the frame and scan headers, LSE presets and mapping tables, the
//! regular and run modes (lossless and near-lossless, every interleave), restart intervals
//! and the HP colour transforms. Run from this directory with
//! `cargo +nightly fuzz run decode corpus/decode`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_jpegls::{decode, probe, read_info, DecodeOptions};

fuzz_target!(|data: &[u8]| {
    let _ = probe(data);
    let _ = read_info(data);
    let opts = DecodeOptions {
        cancel: None,
        // Keep each run cheap: large images decode the same way, slower.
        max_samples: 1 << 20,
    };
    if let Ok(img) = decode(data, &opts) {
        for p in &img.planes {
            assert_eq!(
                p.data.len(),
                p.width as usize * p.height as usize,
                "plane size"
            );
        }
    }
});
