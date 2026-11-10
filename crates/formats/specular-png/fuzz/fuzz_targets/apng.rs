//! Fuzz APNG: frame table parsing, per-frame decode (`decode_frame`) and the compositing
//! `Animator` (dispose/blend ops, frames outside the canvas, rewind). Run from this directory
//! with `cargo +nightly fuzz run apng corpus/apng -- -dict=fuzz.dict`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_png::{decode_frame, read_info, Animator, DecodeOptions, Output};

fuzz_target!(|data: &[u8]| {
    let Ok(info) = read_info(data) else { return };
    // Small canvases and few frames keep each run cheap.
    if info.width as u64 * info.height as u64 > 1 << 18 || info.frames.len() > 32 {
        return;
    }
    for i in 0..info.frames.len().min(4) {
        let f = &info.frames[i];
        if f.width as u64 * f.height as u64 > 1 << 18 {
            continue;
        }
        let _ = decode_frame(data, &info, i, &DecodeOptions::default());
    }
    let output = if data.len() % 2 == 0 {
        Output::Rgba8
    } else {
        Output::Rgba16
    };
    let scale = 1 + (data.len() % 3) as u32;
    let Ok(mut a) = Animator::with_info(data, info, output, scale) else {
        return;
    };
    let n = a.frame_count();
    for _ in 0..2 {
        for _ in 0..n.min(16) {
            match a.next_frame(None) {
                Some(Ok(f)) => {
                    assert!(f.index < n);
                    let _ = f.delay_secs();
                }
                Some(Err(_)) | None => break,
            }
        }
        a.rewind();
    }
});
