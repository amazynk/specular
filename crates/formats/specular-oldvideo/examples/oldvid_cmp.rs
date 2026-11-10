//! `oldvid_cmp CODEC STREAM.frames [REF.out]`: decodes a fixture-layout stream (see
//! tests/data/gen_fixtures.sh) and compares each frame with FFmpeg's output; prints per-frame
//! mismatches, the first mismatching pixel, and the best and mean decode times.
//! CODEC: qtrle, cinepak, msvideo1, msrle, smc, rpza.

use std::time::Instant;

use specular_oldvideo::{Codec, Decoder, Pixels};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let codec = match args[1].as_str() {
        "qtrle" => Codec::QtRle,
        "cinepak" => Codec::Cinepak,
        "msvideo1" => Codec::MsVideo1,
        "msrle" => Codec::MsRle,
        "smc" => Codec::Smc,
        _ => Codec::Rpza,
    };
    let d = std::fs::read(&args[2]).expect("stream");
    let want = args.get(3).map(|p| std::fs::read(p).expect("reference"));
    let be = |at: usize| u32::from_be_bytes(d[at..at + 4].try_into().unwrap());
    let (w, h, depth) = (be(0), be(4), be(8) as u16);
    let mut dec = Decoder::new(codec, w, h, depth).expect("decoder");
    let (mut at, mut ref_at, mut frames) = (12, 0, 0);
    let (mut best, mut total) = (f64::MAX, 0.0);
    while at + 4 <= d.len() {
        let n = be(at) as usize;
        let t = Instant::now();
        let pic = dec.decode(&d[at + 4..at + 4 + n]).expect("frame");
        let s = t.elapsed().as_secs_f64();
        best = best.min(s);
        total += s;
        at += 4 + n;
        let (got, bpp): (Vec<u8>, usize) = match pic {
            Pixels::Pal8(v) => (v.clone(), 1),
            Pixels::Rgb24(v) => (v.clone(), 3),
            Pixels::Argb(v) => (v.clone(), 4),
            Pixels::Rgb555(v) => (v.iter().flat_map(|c| c.to_le_bytes()).collect(), 2),
        };
        if let Some(want) = &want {
            let exp = &want[ref_at..ref_at + got.len()];
            let bad: Vec<usize> = (0..got.len()).filter(|&i| got[i] != exp[i]).collect();
            if let Some(&i) = bad.first() {
                let p = i / bpp;
                println!(
                    "frame {frames}: {} bytes differ, first at pixel ({}, {}): got {:?} want {:?}",
                    bad.len(),
                    p % w as usize,
                    p / w as usize,
                    &got[p * bpp..p * bpp + bpp],
                    &exp[p * bpp..p * bpp + bpp]
                );
            }
            ref_at += got.len() + if bpp == 1 && depth != 24 { 1024 } else { 0 };
        }
        frames += 1;
    }
    println!(
        "{w}×{h}: {frames} frames, best {:.3} ms, mean {:.3} ms",
        best * 1e3,
        total * 1e3 / f64::from(frames.max(1))
    );
}
