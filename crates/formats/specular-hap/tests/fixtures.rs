//! HAP fixtures (`tests/data/gen_fixtures.sh`): FFmpeg 8.0's HAP, HAP Alpha and HAP Q streams
//! against its RGBA decode; own HAP Q Alpha, Alpha-Only and HAP R frames against their writer's
//! exact block decode.

use specular_hap::{Kind, decode, probe};

fn load(name: &str) -> (u32, u32, Vec<Vec<u8>>, Vec<u8>) {
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
    (w, h, frames, out)
}

/// (mismatching bytes, largest difference) over all frames.
fn check(name: &str, kind: Kind) -> (usize, u8) {
    let (w, h, frames, out) = load(name);
    let n = (w * h * 4) as usize;
    assert_eq!(out.len(), n * frames.len(), "{name}");
    let (mut bad, mut max) = (0, 0u8);
    for (f, frame) in frames.iter().enumerate() {
        assert_eq!(probe(frame).unwrap(), kind, "{name}");
        let img = decode(frame, w, h).unwrap();
        assert_eq!(img.kind, kind);
        for (a, b) in img.rgba.iter().zip(&out[f * n..(f + 1) * n]) {
            if a != b {
                bad += 1;
                max = max.max(a.abs_diff(*b));
            }
        }
    }
    eprintln!("{name}: {bad} mismatching bytes, max {max}");
    (bad, max)
}

// BC1/BC3 colour interpolation is implementation-defined to within one step: the BCn
// decoder in specular-texture (bit-exact against bcdec) rounds the 1/3 and 2/3 points
// differently from FFmpeg's, so FFmpeg's output is matched to within 1 (2 after the
// Scaled YCoCg conversion, which multiplies the rounding through the scale).

#[test]
fn hap_bc1_matches_ffmpeg() {
    assert!(check("hap", Kind::Hap).1 <= 1);
    assert!(check("hap-none-odd", Kind::Hap).1 <= 1);
}

#[test]
fn hap_alpha_bc3_matches_ffmpeg() {
    assert!(check("hapa-chunks", Kind::HapAlpha).1 <= 1);
    assert!(check("hapa-none", Kind::HapAlpha).1 <= 1);
}

#[test]
fn hap_q_ycocg_matches_ffmpeg() {
    assert!(check("hapq", Kind::HapQ).1 <= 2);
    assert!(check("hapq-none-odd", Kind::HapQ).1 <= 2);
}

#[test]
fn hap_q_alpha_alpha_only_and_r_match_their_writer() {
    assert!(check("hapm", Kind::HapQAlpha).1 <= 2);
    assert_eq!(check("hapalpha-only", Kind::HapAlphaOnly), (0, 0));
    assert_eq!(check("hapr", Kind::HapR), (0, 0));
}

#[test]
fn truncated_and_mangled_fixtures_do_not_panic() {
    for name in ["hapq", "hapa-chunks", "hapm", "hapr"] {
        let (w, h, frames, _) = load(name);
        let f = &frames[0];
        for n in (0..f.len()).step_by(7) {
            let _ = decode(&f[..n], w, h);
        }
        let mut m = f.clone();
        for i in (0..m.len()).step_by(13) {
            m[i] ^= 0x5A;
            let _ = decode(&m, w, h);
        }
    }
}
