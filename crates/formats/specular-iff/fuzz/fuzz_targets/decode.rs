//! Fuzz the IFF chunk walk, ByteRun1 and every colour mode (planes, EHB, HAM, deep, PBM,
//! ACBM, masks): a decoded image must match its header and its promised size. Run from this
//! directory with `cargo +nightly fuzz run decode corpus/decode -- -dict=fuzz.dict`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_iff::{decode, info, probe};

fuzz_target!(|data: &[u8]| {
    let _ = probe(data);
    let Ok(i) = info(data) else { return };
    let _ = i.coding();
    // Large but valid headers only cost time.
    if u64::from(i.width) * u64::from(i.height) > 1 << 22 {
        return;
    }
    let img = decode(data).expect("info succeeded, so decode must");
    assert_eq!((img.width, img.height), (i.width, i.height));
    assert_eq!(img.pixels.len(), i.width as usize * i.height as usize * 4);
});
