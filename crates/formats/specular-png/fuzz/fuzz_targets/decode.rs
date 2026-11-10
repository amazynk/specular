//! Fuzz PNG decoding: chunk parsing and metadata, the resumable inflater, unfiltering, every
//! output layout, scaled decodes (`decode`, `decode_scales`), the Adam7 preview, iDOT
//! parallel parts and checksum verification. Run from this directory with
//! `cargo +nightly fuzz run decode corpus/decode -- -dict=fuzz.dict`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_png::{
    decode, decode_adam7_preview, decode_scales, read_info, DecodeOptions, Image, Output,
    Parallelism,
};

fn check(img: &Image) {
    for p in &img.planes {
        assert!(
            p.data.len() >= p.stride * p.height.saturating_sub(1) as usize,
            "plane shorter than its rows"
        );
    }
    let _ = img.to_rgba8();
}

fuzz_target!(|data: &[u8]| {
    let Ok(info) = read_info(data) else { return };
    let _ = (
        info.has_alpha(),
        info.auto_layout(),
        info.coding(),
        info.exif(data),
    );
    // Keep each run cheap: large images decode the same way, slower.
    if info.width as u64 * info.height as u64 > 1 << 20 {
        return;
    }
    let sel = data.len() % 3;
    let par = [Parallelism::Single, Parallelism::Threads, Parallelism::Auto][sel];
    for (output, scale, verify) in [
        (Output::Auto8, 1, false),
        (Output::Rgba16, 3, sel == 0),
        (Output::Rgba8, 8, true),
    ] {
        let opts = DecodeOptions {
            output,
            scale,
            parallelism: par,
            verify_checksums: verify,
            ..Default::default()
        };
        if let Ok(img) = decode(data, &opts) {
            assert_eq!(img.width, info.width.div_ceil(scale));
            assert_eq!(img.height, info.height.div_ceil(scale));
            check(&img);
        }
    }
    let opts = DecodeOptions {
        parallelism: par,
        ..Default::default()
    };
    if let Ok(v) = decode_scales(data, &opts, &[2, 1, 2, 5]) {
        assert_eq!(v.len(), 4);
        v.iter().for_each(check);
    }
    if info.interlaced {
        for scale in [2, 4, 8] {
            if let Ok(img) = decode_adam7_preview(data, &DecodeOptions { scale, ..opts }) {
                check(&img);
            }
        }
    }
});
