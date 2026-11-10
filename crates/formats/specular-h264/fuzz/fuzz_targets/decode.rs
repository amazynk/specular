//! Fuzz H.264: NAL splitting and emulation prevention, SPS/PPS/slice header parsing, CAVLC
//! and CABAC macroblock layers (frames, fields and MBAFF; 8 to 14 bits; 4:0:0 to 4:2:2),
//! intra/inter prediction, the transforms, deblocking, POC, reference list construction, MMCO
//! and the DPB, on one thread or across frame threads, in display or decoding order. Run
//! from this directory with `cargo +nightly fuzz run decode corpus/decode -- -dict=fuzz.dict`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_h264::{Chroma, Decoder, Options, OutputOrder, Picture, split_annexb};

fn check(p: &Picture) {
    let (w, h) = (p.width as usize, p.height as usize);
    assert!((8..=14).contains(&p.bit_depth), "bit depth");
    let bps = if p.bit_depth > 8 { 2 } else { 1 };
    let (cw, ch) = match p.chroma {
        Chroma::Yuv420 => (w.div_ceil(2), h.div_ceil(2)),
        Chroma::Yuv422 => (w.div_ceil(2), h),
        Chroma::Yuv444 => (w, h),
    };
    let c = cw * ch * bps;
    assert_eq!(p.planes[0].len(), w * h * bps, "luma plane size");
    assert_eq!(p.planes[1].len(), c, "Cb plane size");
    assert_eq!(p.planes[2].len(), c, "Cr plane size");
}

fuzz_target!(|data: &[u8]| {
    let opts = Options {
        threads: if data.len() % 2 == 1 { 2 } else { 1 },
        output: if data.len() % 3 == 0 {
            OutputOrder::Decode
        } else {
            OutputOrder::Display
        },
    };
    let mut d = Decoder::new(opts);
    for (i, nal) in split_annexb(data).into_iter().enumerate() {
        if d.decode_nal(nal, i as i64).is_err() {
            // Unsupported or malformed: the decoder must stay usable for what follows.
        }
        while let Some(p) = d.next_picture(false) {
            check(&p);
        }
    }
    d.flush();
    while let Some(p) = d.next_picture(true) {
        check(&p);
    }
});
