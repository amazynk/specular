//! Fuzz JPEG XR: the TIFF-like container (pixel formats, planar alpha, ICC, orientation), the
//! image and plane headers, tiling, the entropy decoder (DC/LP/HP bands, adaptive VLC,
//! flexbits), quantisation, the inverse transforms with overlap filtering and every output
//! conversion. Run from this directory with `cargo +nightly fuzz run decode corpus/decode`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_jxr::{decode, probe, read_info, DecodeOptions, Pixels};

fuzz_target!(|data: &[u8]| {
    let _ = probe(data);
    let Ok(info) = read_info(data) else { return };
    let opts = DecodeOptions {
        cancel: None,
        // Keep each run cheap: large images decode the same way, slower.
        max_samples: 1 << 20,
        serial: data.len() % 2 == 0,
    };
    if let Ok(img) = decode(data, &opts) {
        assert_eq!((img.width, img.height), (info.width, info.height), "size");
        let n = img.width as usize * img.height as usize * img.channels();
        let len = match &img.pixels {
            Pixels::U8(v) => v.len(),
            Pixels::U16(v) => v.len(),
            Pixels::F32(v) => v.len(),
        };
        assert_eq!(len, n, "pixel buffer size");
    }
});
