//! Fuzz GIF animation: play every frame through the compositing `Animator` (disposal,
//! interlacing, partial frames), then rewind and scale a frame. Run from this directory with
//! `cargo +nightly fuzz run gif_anim corpus/gif_anim -- -dict=fuzz.dict`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_bitmap::gif::{scale_rgba, Animator, Gif};
use specular_bitmap::{Decode, Options, Region};
use std::sync::Arc;

fuzz_target!(|data: &[u8]| {
    let Ok(g) = Gif::parse(data) else { return };
    let (w, h) = (g.info().width, g.info().height);
    // Small canvases and few frames keep each run cheap.
    if w as u64 * h as u64 > 1 << 20 || g.animation().frames.len() > 64 {
        return;
    }
    let _ = g.memory_bytes();
    let never = || false;
    let mut a = Animator::new(Arc::new(g));
    let mut n = 0;
    while let Some(r) = a.next_frame(data, &never) {
        let (canvas, _, _) = r.expect("only cancellation fails");
        assert_eq!(canvas.len(), w as usize * h as usize * 4);
        n += 1;
    }
    a.rewind();
    if let Some(Ok((canvas, _, _))) = a.next_frame(data, &never) {
        let canvas = canvas.to_vec();
        let opts = Options {
            scale: 2,
            region: Some(Region {
                x: 1,
                y: 1,
                w: w / 2 + 1,
                h: h / 2 + 1,
            }),
            parallel: n % 2 == 1,
        };
        if let Ok(img) = scale_rgba(&canvas, w, h, &opts, &never) {
            assert_eq!(img.data.len(), img.width as usize * img.height as usize * 4);
        }
    }
});
