//! Fuzz RAW parsing, sensor decode and development, DNG colour profiles (ForwardMatrix,
//! HueSatMap, LookTable) included. Run from this directory with
//! `cargo +nightly fuzz run decode <corpus dir> -- -max_total_time=600`; the DNGs written by
//! the crate's tests (e.g. the profiled one in `tests/malformed.rs`) make good seeds, and
//! `SPECULAR_RAW_FUZZ_SEEDS=<corpus dir> cargo test -p specular-raw --test arw write_fuzz_seeds`
//! writes small Sony lossless ARWs (LJ92 tiles) there.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_raw::*;

fuzz_target!(|data: &[u8]| {
    let _ = detect(data);
    let Ok(raw) = parse(data) else { return };
    let info = raw.info();
    for p in &info.previews {
        assert!(p.range.end <= data.len(), "preview range out of bounds");
    }
    if let Some(p) = info.thumbnail() {
        let _ = decode_preview(data, p, 0);
    }
    // Keep each run cheap: small sensors only.
    let opts = DecodeOptions {
        size: OutputSize::Half,
        parallel: data.len() % 2 == 1,
        max_pixels: 1 << 20,
        ..Default::default()
    };
    if let Ok(img) = raw.decode_raw(data, &opts) {
        for size in [OutputSize::Half, OutputSize::Full] {
            let o = DecodeOptions { size, ..opts };
            if let Ok(out) = develop(&img, &o) {
                assert_eq!(out.data.len(), out.width as usize * out.height as usize * 3);
                region(&raw, data, &img, &out, &o);
            }
        }
    }
});

/// A region picked from the input's last bytes equals the same pixels of the whole develop
/// (the levels must be sane: with black above white the LUT depends on the samples seen).
fn region(raw: &RawFile, data: &[u8], img: &RawImage, whole: &RgbImage, o: &DecodeOptions) {
    let [a, b, c, d] = match data {
        [.., a, b, c, d] => [*a, *b, *c, *d].map(u32::from),
        _ => return,
    };
    let (w, h) = (whole.width, whole.height);
    let r = Rect {
        x: a * w / 256,
        y: b * h / 256,
        w: c % 64 + 1,
        h: d % 64 + 1,
    };
    let cache = TileCache::new(if a % 2 == 0 { u64::MAX } else { 4096 });
    let ro = DecodeOptions {
        region: Some(r),
        tiles: Some(&cache),
        ..*o
    };
    let Ok(part) = raw.decode(data, &ro) else {
        return;
    };
    let (rw, rh) = (r.w.min(w - r.x), r.h.min(h - r.y));
    assert_eq!((part.width, part.height), (rw, rh));
    if img.black.iter().all(|&b| b < img.white) {
        for y in 0..rh {
            let s = ((r.y + y) * w + r.x) as usize * 3;
            let t = (y * rw) as usize * 3;
            assert!(
                part.data[t..t + rw as usize * 3] == whole.data[s..s + rw as usize * 3],
                "region {r:?} differs from the whole develop"
            );
        }
    }
}
