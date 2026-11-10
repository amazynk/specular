//! Fuzz the TIFF / BigTIFF IFD walker, SubIFD pyramid levels, strips and tiles, the LZW,
//! Deflate, PackBits and JPEG codecs, predictors, colour conversion, scaling and regions. Run
//! from this directory with `cargo +nightly fuzz run decode corpus/decode -- -dict=fuzz.dict`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_tiff::{DecodeOptions, Region, Tiff};

fuzz_target!(|data: &[u8]| {
    let Ok(t) = Tiff::open(data) else { return };
    let parallel = data.len() % 2 == 1;
    for (i, p) in t.pages().iter().enumerate().take(4) {
        let _ = (p.ifd().description(), p.level_sizes(), p.icc_profile(data));
        let _ = (p.orientation(), p.output_format(), p.has_alpha(), p.tiled());
        if p.check_supported().is_err() {
            continue;
        }
        // Keep each run cheap: large images decode the same way, slower.
        let (w, h) = (p.width(), p.height());
        if w as u64 * h as u64 > 1 << 20 {
            continue;
        }
        for (scale, region) in [
            (1, None),
            (
                2,
                Some(Region {
                    x: w / 7,
                    y: h / 5,
                    w: w / 3 + 1,
                    h: h / 2 + 1,
                }),
            ),
            (8, None),
        ] {
            let opts = DecodeOptions {
                scale,
                region,
                parallel,
                ..Default::default()
            };
            if t.memory_estimate(i, &opts) > 1 << 28 {
                continue;
            }
            if let Ok(d) = t.decode(data, i, &opts) {
                let row = d.width as usize * d.format.bytes_per_pixel();
                assert!(d.stride >= row, "stride shorter than a row");
                assert!(
                    d.data.len() >= d.stride * d.height as usize,
                    "buffer shorter than stride × height"
                );
                if region.is_none() {
                    assert_eq!((d.width, d.height), (w.div_ceil(scale), h.div_ceil(scale)));
                }
            }
        }
    }
});
