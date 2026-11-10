//! Fuzz the PICT opcode walk: bitmap and pixmap opcodes with every pack type, regions,
//! scaling, QuickTime images and the skipping of everything else. A decoded picture must
//! match its header; a sole JPEG range must lie inside the input. Run from this directory
//! with `cargo +nightly fuzz run decode corpus/decode -- -dict=fuzz.dict`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_pict::{decode, info, plausible, probe, probe_file, sole_jpeg};

fuzz_target!(|data: &[u8]| {
    let _ = probe(data);
    let _ = probe_file(data);
    let _ = plausible(data);
    let Ok(i) = info(data) else { return };
    // Large but valid headers only cost time.
    if u64::from(i.width) * u64::from(i.height) > 1 << 22 {
        return;
    }
    if let Some(r) = sole_jpeg(data) {
        assert!(r.start <= r.end && r.end <= data.len());
    }
    if let Ok(img) = decode(data) {
        assert_eq!((img.width, img.height), (i.width, i.height));
        assert_eq!(img.pixels.len(), i.width as usize * i.height as usize * 4);
    }
});
