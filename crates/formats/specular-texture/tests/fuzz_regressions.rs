//! Inputs found by `fuzz/` that once panicked. Each must open/decode (or fail) without
//! panicking.

use specular_texture::{DecodeOptions, Texture};

fn dds(w: u32, h: u32, mips: u32, pf: [u32; 8]) -> Vec<u8> {
    let mut d = b"DDS ".to_vec();
    let mut put = |v: u32| d.extend_from_slice(&v.to_le_bytes());
    for v in [124, 0x2_1007, h, w, 0, 0, mips] {
        put(v);
    }
    for _ in 0..11 {
        put(0);
    }
    for v in pf {
        put(v);
    }
    for v in [0x1000, 0, 0, 0, 0] {
        put(v);
    }
    d.extend_from_slice(&[0x5a; 64]);
    d
}

/// A DDS header claiming 2^32−1 × 2^32−1 pixels of 32-bit RGBA: sizing its levels
/// (blocks × bytes per block) overflowed u64.
#[test]
fn dds_level_size_overflow() {
    let rgba = [32, 0x41, 0, 32, 0xff, 0xff00, 0xff_0000, 0xff00_0000];
    for (w, h) in [(u32::MAX, u32::MAX), (u32::MAX, 0x5854_4bab)] {
        let data = dds(w, h, 3, rgba);
        if let Ok(t) = Texture::open(&data) {
            let opts = DecodeOptions::default();
            let _ = t.memory_estimate(&opts);
            let _ = t.level_sizes();
        }
    }
}

/// A Firaxis texture (FTEX) claiming 2^32−1 × 2^32−1 pixels: sizing its first level overflowed.
#[test]
fn ftex_level_size_overflow() {
    let data = include_bytes!("../fuzz/seeds/ftex-size-overflow.ftex");
    if let Ok(t) = Texture::open(data) {
        let _ = t.decode(data, &DecodeOptions::default());
    }
}
