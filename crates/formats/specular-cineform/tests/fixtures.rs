//! CineForm fixtures (`tests/data/gen_fixtures.sh`): FFmpeg 8.0's cfhd streams against its
//! own decode. FFmpeg's decoder rounds the prescaled wavelets differently from the CineForm
//! SDK's filters, which this decoder follows, so its pictures differ by at most a step or two
//! at 10 and 12 bits (on synthetic pictures ours is the closer to the source); the counts are
//! printed and the PSNR must stay above 60 dB at 10 bits and 70 dB at 12.

use specular_cineform::{Decoder, Layout, probe};

fn load(name: &str) -> (u32, u32, Vec<Vec<u8>>, Vec<u16>) {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");
    let d = std::fs::read(format!("{dir}{name}.frames")).unwrap();
    let be = |i: usize| u32::from_be_bytes([d[i], d[i + 1], d[i + 2], d[i + 3]]);
    let (w, h) = (be(0), be(4));
    let mut frames = Vec::new();
    let mut i = 12;
    while i < d.len() {
        let n = be(i) as usize;
        frames.push(d[i + 4..i + 4 + n].to_vec());
        i += 4 + n;
    }
    let out = std::fs::read(format!("{dir}{name}.out")).unwrap();
    let out = out
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect();
    (w, h, frames, out)
}

/// Decodes a fixture; (mismatching samples, largest difference, PSNR in dB).
fn check(name: &str, layout: Layout, bits: u8) -> (usize, u16, f64) {
    let (w, h, frames, out) = load(name);
    let (w, h) = (w as usize, h as usize);
    let mut dec = Decoder::new();
    let (mut bad, mut max, mut se, mut n) = (0usize, 0u16, 0f64, 0usize);
    let mut at = 0;
    for f in &frames {
        let info = probe(f).unwrap();
        assert_eq!(
            (info.width as usize, info.height as usize),
            (w, h),
            "{name}"
        );
        assert_eq!(info.layout, layout);
        let pic = dec.decode(f).unwrap();
        assert_eq!(pic.bits, bits);
        // FFmpeg's plane order: Y, Cb, Cr; or G, B, R (, A).
        let order: Vec<(usize, usize)> = match layout {
            Layout::Yuv422 => vec![(0, w), (1, w.div_ceil(2)), (2, w.div_ceil(2))],
            Layout::Rgb444 => vec![(1, w), (2, w), (0, w)],
            _ => vec![(1, w), (2, w), (0, w), (3, w)],
        };
        for (p, pw) in order {
            let want = &out[at..at + pw * h];
            at += pw * h;
            for (a, b) in pic.planes[p].iter().zip(want) {
                let d = a.abs_diff(*b);
                if d != 0 {
                    bad += 1;
                    max = max.max(d);
                }
                se += f64::from(d) * f64::from(d);
                n += 1;
            }
        }
    }
    assert_eq!(at, out.len(), "{name}");
    let peak = f64::from((1u32 << bits) - 1);
    let psnr = 10.0 * (peak * peak / (se / n as f64).max(1e-12)).log10();
    eprintln!("{name}: {bad} of {n} samples differ, max {max}, PSNR {psnr:.1} dB");
    (bad, max, psnr)
}

#[test]
fn yuv_422_matches_ffmpeg() {
    for name in ["cf-422", "cf-422-low"] {
        let (_, max, psnr) = check(name, Layout::Yuv422, 10);
        assert!(max <= 2 && psnr > 60.0, "{name}");
    }
}

#[test]
fn rgb_and_rgba_match_ffmpeg() {
    for name in ["cf-rgb", "cf-rgb-mid"] {
        let (_, max, psnr) = check(name, Layout::Rgb444, 12);
        assert!(max <= 3 && psnr > 70.0, "{name}");
    }
    let (_, max, psnr) = check("cf-rgba", Layout::Rgba4444, 12);
    assert!(max <= 3 && psnr > 70.0);
}

#[test]
fn damaged_samples_do_not_panic() {
    let (_, _, frames, _) = load("cf-rgba");
    let f = &frames[0];
    for cut in (0..f.len()).step_by(97) {
        let _ = Decoder::new().decode(&f[..cut]);
    }
    let mut x = 0x9E37_79B9u32;
    for i in 0..200 {
        let mut g = f.clone();
        for _ in 0..4 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let at = x as usize % g.len();
            g[at] ^= (x >> 24) as u8 | 1;
        }
        let _ = Decoder::new().decode(&g);
        let _ = i;
    }
}
