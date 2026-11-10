//! Fuzz JPEG XL (bare codestream and container): headers, ICC and EXIF boxes, VarDCT and
//! modular frames, the 1/8 LF preview, scaled decodes and animation frames. Run from this
//! directory with `cargo +nightly fuzz run decode corpus/decode -- -dict=fuzz.dict`.
//!
//! The crate contains panics from its in-tree jxl-oxide fork with `catch_unwind`; libFuzzer's
//! panic hook aborts on those too, so this target reports them as well.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_jxl::{probe, JxlFile};

fuzz_target!(|data: &[u8]| {
    let _ = probe(data);
    let Ok(f) = JxlFile::open(data) else { return };
    let info = f.info();
    // Keep each run cheap: large images decode the same way, slower.
    if info.width as u64 * info.height as u64 > 1 << 18 || f.decode_memory(1) > 1 << 28 {
        return;
    }
    let parallel = data.len() % 4 == 1;
    let check = |d: &specular_jxl::Decoded, scale: u32| {
        assert_eq!(d.width, info.width.div_ceil(scale), "scaled width");
        assert_eq!(d.height, info.height.div_ceil(scale), "scaled height");
        let n = d.width as usize * d.height as usize * d.layout.bytes_per_pixel();
        assert_eq!(d.data.len(), n, "buffer is not width × height × pixel size");
    };
    if let Ok(d) = f.decode(1, parallel) {
        check(&d, 1);
    }
    if let Ok(d) = f.decode(3, parallel) {
        check(&d, 3);
    }
    if let Ok(d) = f.decode_dc(parallel) {
        check(&d, 8);
    }
    if info.animated {
        if let Ok(frames) = f.frames(2, parallel) {
            for d in frames.take(4).flatten() {
                check(&d, 2);
            }
        }
    }
});
