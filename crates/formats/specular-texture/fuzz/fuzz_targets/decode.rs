//! Fuzz DDS (DX9/DX10) and KTX2 containers, KTX2 supercompression (zstd, zlib), the BCn and
//! uncompressed pixel decoders, mip-level selection, scaling and regions. Run from this
//! directory with `cargo +nightly fuzz run decode corpus/decode -- -dict=fuzz.dict`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_texture::{is_dds, is_ktx2, DecodeOptions, Region, Texture};

fuzz_target!(|data: &[u8]| {
    let _ = (is_dds(data), is_ktx2(data));
    let Ok(t) = Texture::open(data) else { return };
    let _ = (t.description(), t.level_sizes(), t.is_supercompressed());
    // Keep each run cheap: large textures decode the same way, slower.
    if t.width as u64 * t.height as u64 > 1 << 18 {
        return;
    }
    let parallel = data.len() % 2 == 1;
    for (scale, region) in [
        (1, None),
        (
            2,
            Some(Region {
                x: t.width / 7,
                y: t.height / 5,
                w: t.width / 3 + 1,
                h: t.height / 2 + 1,
            }),
        ),
        (3, None),
    ] {
        let opts = DecodeOptions {
            scale,
            region,
            parallel,
            ..Default::default()
        };
        if t.memory_estimate(&opts) > 1 << 28 {
            continue;
        }
        if let Ok(d) = t.decode(data, &opts) {
            assert!(
                d.stride >= d.width as usize * 4,
                "stride shorter than a row"
            );
            assert!(
                d.data.len() >= d.stride * d.height as usize,
                "buffer shorter than stride × height"
            );
        }
    }
});
