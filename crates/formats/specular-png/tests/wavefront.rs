//! The wavefront switch changes how rows are unfiltered, never the pixels.

use specular_png::{ColorType, DecodeOptions, Output, Parallelism, Wavefront, decode, encode};

#[test]
fn wavefront_on_off_and_auto_agree() {
    let (w, h) = (1500u32, 700u32);
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            px.extend_from_slice(&[
                (x * 7 + y) as u8,
                (y * 3) as u8,
                ((x ^ y) & 0xf0) as u8,
                255 - (x / 6) as u8,
            ]);
        }
    }
    let png = encode(w, h, ColorType::Rgba, &px).expect("encode");
    let run = |wavefront| {
        let o = DecodeOptions {
            output: Output::Rgba8,
            parallelism: Parallelism::Threads,
            wavefront,
            ..Default::default()
        };
        let img = decode(&png, &o).expect("decode");
        assert!(!img.truncated);
        img.to_rgba8()
    };
    let off = run(Wavefront::Off);
    assert_eq!(off, px);
    assert_eq!(run(Wavefront::On), off);
    assert_eq!(run(Wavefront::Auto), off);
}
