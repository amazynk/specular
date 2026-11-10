//! Fuzz the H.264 encoder against the in-tree decoder: small pictures of any even size, every
//! profile, QP, slice count, trellis, IDR cuts, fades (weighted P, B cross-fades) and motion
//! hints. Every packet must decode without error to exactly the encoder's reconstruction.
//! The first eight bytes pick the settings, the rest are the samples (cycled). Run from
//! this directory with `cargo +nightly fuzz run roundtrip corpus/roundtrip`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_h264::{Decoder, Options, OutputOrder};
use specular_h264enc::{Config, Encoder, Frame, Hints, MotionHint, Profile};

fuzz_target!(|data: &[u8]| {
    let Some((head, body)) = data.split_first_chunk::<8>() else {
        return;
    };
    let w = 2 * (1 + head[0] as u32 % 40);
    let h = 2 * (1 + head[1] as u32 % 40);
    let mut cfg = Config::new(w, h, 25, 1, 10 + head[2] % 42);
    cfg.profile = [Profile::Baseline, Profile::Main, Profile::High][head[3] as usize % 3];
    cfg.trellis = head[3] & 4 != 0;
    cfg.slices = (head[3] >> 3) as usize % 4;
    cfg.i_qp_offset = head[3] >> 5;
    cfg.keyint = 1 + head[4] as u32 % 8;
    let frames = 1 + head[5] as usize % 6;
    let Ok(mut enc) = Encoder::new(cfg) else {
        return;
    };
    let src: &[u8] = if body.is_empty() { &[128] } else { body };
    let (ws, hs) = (w as usize, h as usize);
    let (cw, ch) = (ws / 2, hs / 2);
    let mut packets = Vec::new();
    let mut recon = Vec::new();
    for f in 0..frames {
        // A per-frame control byte: cut, fade step, motion hint.
        let c = src[(f * 7 + 3) % src.len()] ^ head[6].rotate_left(f as u32);
        let shift = f * (head[7] as usize % 16);
        let gain = if c & 8 != 0 { (f as i32 * 9) - 20 } else { 0 };
        let px = |i: usize| -> u8 {
            let v = src[(i + shift) % src.len()] as i32 + gain;
            v.clamp(0, 255) as u8
        };
        let y: Vec<u8> = (0..ws * hs).map(px).collect();
        let u: Vec<u8> = (0..cw * ch).map(|i| px(i * 3 + 1)).collect();
        let v: Vec<u8> = (0..cw * ch).map(|i| px(i * 5 + 2)).collect();
        let motion = (c & 16 != 0).then(|| MotionHint {
            scale: 1.0 + (head[7] as i8 as f32) / 512.0,
            dx: (c as i8 as f32) / 8.0,
            dy: (head[6] as i8 as f32) / 8.0,
        });
        let fading = match c >> 6 {
            0 => None,
            1 => Some(false),
            _ => Some(true),
        };
        let frame = Frame {
            y: &y,
            u: &u,
            v: &v,
            y_stride: ws,
            c_stride: cw,
        };
        packets.push(enc.encode_with(&frame, c & 1 != 0, Hints { motion, fading }));
        recon.push(enc.reconstruction());
    }
    let opts = Options {
        threads: 1,
        output: OutputOrder::Decode,
    };
    let mut d = Decoder::with_avcc(&enc.avcc(), opts).expect("the encoder's avcC");
    let mut out = Vec::new();
    for (i, p) in packets.iter().enumerate() {
        d.decode_sample(&p.data, i as i64)
            .expect("the decoder takes every packet");
        while let Some(p) = d.next_picture(false) {
            out.push(p);
        }
    }
    d.flush();
    while let Some(p) = d.next_picture(true) {
        out.push(p);
    }
    assert_eq!(out.len(), packets.len(), "one picture per packet");
    for p in &out {
        let r = &recon[p.tag as usize];
        assert_eq!(
            (p.width, p.height, p.bit_depth),
            (w, h, 8),
            "picture format"
        );
        for k in 0..3 {
            assert!(
                p.planes[k] == r[k],
                "frame {} plane {k} differs from the reconstruction ({:?})",
                p.tag,
                packets[p.tag as usize].stats
            );
        }
    }
});
