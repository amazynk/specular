//! Fuzz the HEIC container + HEVC decode path. Run from this directory
//! with `cargo +nightly fuzz run decode <corpus dir> -- -max_total_time=900`
//! (seed the corpus with small HEICs, e.g. the specular-decode-host
//! fixtures).
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_heic::{DecodeOptions, HeicFile};

fuzz_target!(|data: &[u8]| {
    let Ok(f) = HeicFile::open(data.to_vec()) else {
        return;
    };
    let info = f.info();
    // Keep each run cheap: large images decode the same way, slower.
    if info.width as u64 * info.height as u64 > 1 << 22 {
        return;
    }
    let _ = f.exif();
    let _ = f.xmp();
    let opts = DecodeOptions {
        parallel: data.len() % 2 == 1,
        ..Default::default()
    };
    for i in 0..f.tile_count().min(4) {
        let _ = f.decode_tile(i, &opts);
    }
    if let Ok(img) = f.decode(&opts) {
        let rgba = img.to_rgba8().expect("rgba of a decoded image");
        assert_eq!(rgba.len(), img.width() as usize * img.height() as usize * 4);
    }
});
