//! Fuzz JP2 / J2K / HTJ2K: the box and marker parsers, tiles and tile-parts, packet headers
//! (tag trees, PPM/PPT/PLT), the MQ and HT block decoders, the inverse wavelets, colour
//! conversion, reduced-resolution, region and layer-limited decodes, a JPX file's later codestreams. Run from this directory
//! with `cargo +nightly fuzz run decode corpus/decode -- -dict=fuzz.dict`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_jp2::{
    choose_reduce, codestream_count, probe, DecodeOptions, Decoder, Depth, Parallelism, Pixels,
    Region,
};

fuzz_target!(|data: &[u8]| {
    let _ = probe(data);
    // A JPX file's later codestreams, with their own codestream and layer headers.
    for i in 1..codestream_count(data).min(4) {
        if let Ok(d) = Decoder::new_at(data, i) {
            let (w, h) = d.info().reduced_size(0);
            if w as u64 * h as u64 <= 1 << 16 && d.info().components.len() <= 4 {
                let _ = d.decode(&DecodeOptions::default());
            }
        }
    }
    let Ok(dec) = Decoder::new(data) else { return };
    let info = dec.info().clone();
    let parallelism = if data.len() % 2 == 1 {
        Parallelism::Auto
    } else {
        Parallelism::Single
    };
    let full = info.reduced_size(0);
    let small = choose_reduce(&info, full.0 / 4, full.1 / 4);
    let (sw, sh) = info.reduced_size(small);
    let mut runs = vec![
        (0, None, None, Depth::Eight),
        (small, None, Some(1), Depth::Sixteen),
        (
            small,
            Some(Region {
                x: sw / 5,
                y: sh / 3,
                w: sw / 2 + 1,
                h: sh / 2 + 1,
            }),
            None,
            Depth::Eight,
        ),
    ];
    if info.levels > 0 {
        runs.push((info.levels, None, None, Depth::Eight));
    }
    for (reduce, region, max_layers, depth) in runs {
        // Keep each run cheap: large images decode the same way, slower.
        let (w, h) = info.reduced_size(reduce);
        if w as u64 * h as u64 > 1 << 16 || info.components.len() > 4 {
            continue;
        }
        let opts = DecodeOptions {
            reduce,
            region,
            max_layers,
            depth,
            parallelism,
            cancel: None,
        };
        if let Ok(img) = dec.decode(&opts) {
            let n = img.width as usize * img.height as usize * img.format.channels();
            let len = match &img.pixels {
                Pixels::U8(v) => v.len(),
                Pixels::U16(v) => v.len(),
            };
            assert_eq!(len, n, "pixel buffer is not width × height × channels");
            if region.is_none() {
                assert_eq!(
                    (img.width, img.height),
                    (w, h),
                    "size differs from reduced_size"
                );
            }
        }
    }
});
