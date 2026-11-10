//! Fuzz WebP: the RIFF / VP8X container, VP8 (lossy) and VP8L (lossless) bitstreams, ALPH,
//! planar YCbCr output, 1/2 … 1/8 scaled decodes, RGBA conversion and animation compositing.
//! Run from this directory with `cargo +nightly fuzz run decode corpus/decode -- -dict=fuzz.dict`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_webp::{image, Animation, Decoded, Pixels, WebpImage};

fn check(d: &Decoded, w: u32, h: u32, scale: u32) {
    assert_eq!(
        (d.width, d.height),
        (w.div_ceil(scale), h.div_ceil(scale)),
        "scaled size"
    );
    let (dw, dh) = (d.width as usize, d.height as usize);
    match &d.pixels {
        Pixels::YCbCr420 { y, cb, cr } => {
            assert!(
                y.stride >= dw && y.data.len() >= y.stride * dh,
                "Y plane too small"
            );
            for c in [cb, cr] {
                let (cw, ch) = (dw.div_ceil(2), dh.div_ceil(2));
                assert!(
                    c.stride >= cw && c.data.len() >= c.stride * ch,
                    "chroma plane too small"
                );
            }
        }
        Pixels::Rgba(p) => assert!(
            p.stride >= dw * 4 && p.data.len() >= p.stride * dh,
            "RGBA plane too small"
        ),
    }
}

fuzz_target!(|data: &[u8]| {
    let Ok(img) = WebpImage::parse(data) else {
        return;
    };
    let info = img.info().clone();
    // Keep each run cheap: large images decode the same way, slower.
    if info.width as u64 * info.height as u64 > 1 << 18 || img.decode_memory(1) > 1 << 28 {
        return;
    }
    let parallel = data.len() % 2 == 1;
    for scale in [1, 2, 8] {
        if let Ok(d) = img.decode(scale, parallel) {
            check(&d, info.width, info.height, scale);
            if scale == 1 {
                let rgba = image::to_rgba(d);
                check(&rgba, info.width, info.height, 1);
                let small = image::downscale_rgba(rgba, 3, parallel);
                check(&small, info.width, info.height, 3);
            }
        }
    }
    if info.animated {
        if let Ok(mut a) = Animation::new(data) {
            for pass in 0..2 {
                for f in std::iter::from_fn(|| a.next_frame()).take(if pass == 0 { 6 } else { 2 }) {
                    if let Ok(f) = f {
                        check(&f.image, info.width, info.height, 1);
                    }
                }
                a.rewind();
            }
        }
    }
});
