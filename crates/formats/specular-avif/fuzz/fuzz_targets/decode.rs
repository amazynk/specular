//! Fuzz AVIF: the ISOBMFF / HEIF box parser (items, grids, transforms, auxiliary alpha), AVIS
//! sequences, the in-tree rav1d AV1 decoder, scaled conversion and animation playback. Run from
//! this directory with `cargo +nightly fuzz run decode corpus/decode -- -dict=fuzz.dict`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_avif::{probe, AvifFile, Decoded};

fn check(d: &Decoded) {
    let n = d.width as usize * d.height as usize * d.layout.channels();
    assert_eq!(d.data.len(), n, "buffer is not width × height × channels");
}

fuzz_target!(|data: &[u8]| {
    let _ = probe(data);
    let Ok(f) = AvifFile::open(data.to_vec()) else {
        return;
    };
    let info = f.info();
    // Keep each run cheap: large images decode the same way, slower.
    if info.width as u64 * info.height as u64 > 1 << 18 || f.decode_memory(1) > 1 << 28 {
        return;
    }
    let parallel = data.len() % 4 == 1;
    for scale in [1, 2, 8] {
        if let Ok(d) = f.decode(scale, parallel) {
            check(&d);
        }
    }
    if let Ok(mut frames) = f.frames(2, parallel) {
        for pass in 0..2 {
            for _ in 0..if pass == 0 { 4 } else { 1 } {
                match frames.next_with(&|| false) {
                    Some(Ok(d)) => check(&d),
                    Some(Err(_)) => {}
                    None => break,
                }
            }
            frames.rewind();
        }
    }
});
