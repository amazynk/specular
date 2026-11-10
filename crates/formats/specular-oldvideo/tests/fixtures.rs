//! Every fixture stream (tests/data, made by gen_fixtures.sh with FFmpeg's encoders) against
//! FFmpeg's decode, frame by frame and pixel for pixel in the codec's own format.

use std::path::Path;

use specular_oldvideo::{Codec, Decoder, Pixels};

fn read(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name),
    )
    .unwrap()
}

/// The picture as FFmpeg writes it (palette indices without the palette).
fn bytes(p: &Pixels) -> Vec<u8> {
    match p {
        Pixels::Pal8(v) | Pixels::Rgb24(v) | Pixels::Argb(v) => v.clone(),
        Pixels::Rgb555(v) => v.iter().flat_map(|c| c.to_le_bytes()).collect(),
    }
}

/// RGB555's unused top bit is left out of the comparison (FFmpeg passes it through).
fn same(p: &Pixels, i: usize, a: u8, b: u8) -> bool {
    match p {
        Pixels::Rgb555(_) if !i.is_multiple_of(2) => a & 0x7F == b & 0x7F,
        _ => a == b,
    }
}

/// Decodes `name` and compares every frame; the mismatching bytes in all.
fn mismatches(name: &str, codec: Codec) -> usize {
    let d = read(&format!("{name}.frames"));
    let want = read(&format!("{name}.out"));
    let be = |at: usize| u32::from_be_bytes(d[at..at + 4].try_into().unwrap());
    let (w, h, depth) = (be(0), be(4), be(8) as u16);
    let mut dec = Decoder::new(codec, w, h, depth).unwrap();
    let (mut at, mut ref_at, mut bad, mut frames) = (12, 0, 0, 0);
    while at + 4 <= d.len() {
        let n = be(at) as usize;
        let pic = dec
            .decode(&d[at + 4..at + 4 + n])
            .unwrap_or_else(|e| panic!("{name} frame {frames}: {e}"));
        at += 4 + n;
        let got = bytes(pic);
        let exp = &want[ref_at..ref_at + got.len()];
        bad += (0..got.len())
            .filter(|&i| !same(pic, i, got[i], exp[i]))
            .count();
        ref_at += got.len();
        if matches!(pic, Pixels::Pal8(_)) {
            ref_at += 1024;
        }
        frames += 1;
    }
    assert_eq!(ref_at, want.len(), "{name}: frame count");
    assert!(frames >= 3, "{name}");
    bad
}

#[test]
fn quicktime_animation() {
    for name in ["qtrle24", "qtrle16", "qtrle32", "qtrle8"] {
        assert_eq!(mismatches(name, Codec::QtRle), 0, "{name}");
    }
}

#[test]
fn cinepak() {
    for name in ["cinepak", "cinepak-grey", "cinepak-strips"] {
        assert_eq!(mismatches(name, Codec::Cinepak), 0, "{name}");
    }
}

#[test]
fn smc() {
    for name in ["smc", "smc-odd"] {
        assert_eq!(mismatches(name, Codec::Smc), 0, "{name}");
    }
}

#[test]
fn rpza() {
    for name in ["rpza", "rpza-odd"] {
        assert_eq!(mismatches(name, Codec::Rpza), 0, "{name}");
    }
}

#[test]
fn microsoft_video_1() {
    assert_eq!(mismatches("msvideo1", Codec::MsVideo1), 0);
}

#[test]
fn microsoft_rle() {
    assert_eq!(mismatches("msrle", Codec::MsRle), 0);
}

#[test]
fn flash_screen_video() {
    assert_eq!(mismatches("flashsv", Codec::FlashSv1), 0);
    // Version 2: hybrid colours, diff blocks and primed blocks from FFmpeg's encoder.
    for name in ["flashsv2", "flashsv2-big"] {
        assert_eq!(mismatches(name, Codec::FlashSv2), 0, "{name}");
    }
}

#[test]
fn damaged_frames_do_not_panic() {
    for (name, codec) in [
        ("qtrle24", Codec::QtRle),
        ("qtrle8", Codec::QtRle),
        ("cinepak", Codec::Cinepak),
        ("smc", Codec::Smc),
        ("rpza", Codec::Rpza),
        ("msvideo1", Codec::MsVideo1),
        ("msrle", Codec::MsRle),
        ("flashsv", Codec::FlashSv1),
        ("flashsv2", Codec::FlashSv2),
    ] {
        let d = read(&format!("{name}.frames"));
        let be = |at: usize| u32::from_be_bytes(d[at..at + 4].try_into().unwrap());
        let (w, h, depth) = (be(0), be(4), be(8) as u16);
        let n = be(12) as usize;
        let f = &d[16..16 + n];
        for seed in 0..40usize {
            let mut g = f.to_vec();
            let cut = (seed * 7919) % g.len().max(1);
            if seed.is_multiple_of(2) {
                g.truncate(cut.max(1));
            } else {
                g[cut] ^= 0xA5;
                let at = (cut * 3) % g.len();
                g[at] = 0xFF;
            }
            let mut dec = Decoder::new(codec, w, h, depth).unwrap();
            let _ = dec.decode(&g);
        }
    }
}
