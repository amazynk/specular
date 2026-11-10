//! Every fixture stream (tests/data, made by gen_fixtures.sh with FFmpeg's encoder) against
//! FFmpeg's decode: sample for sample, alpha included, slices serial and parallel alike.

use std::path::Path;

use specular_ffv1::{ColourSpace, Decoder, Layout};

fn read(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name),
    )
    .unwrap()
}

struct Stream {
    width: u32,
    height: u32,
    config: Vec<u8>,
    frames: Vec<Vec<u8>>,
}

fn stream(name: &str) -> Stream {
    let d = read(&format!("{name}.frames"));
    let be = |at: usize| u32::from_be_bytes(d[at..at + 4].try_into().unwrap()) as usize;
    let (width, height, n) = (be(0) as u32, be(4) as u32, be(8));
    let config = d[12..12 + n].to_vec();
    let mut at = 12 + n;
    let mut frames = Vec::new();
    while at + 4 <= d.len() {
        let size = be(at);
        frames.push(d[at + 4..at + 4 + size].to_vec());
        at += 4 + size;
    }
    Stream {
        width,
        height,
        config,
        frames,
    }
}

/// Decodes every frame both ways and compares with FFmpeg; the mismatching samples.
fn check(name: &str) -> specular_ffv1::Params {
    let s = stream(name);
    let want = read(&format!("{name}.yuv"));
    let mut params = None;
    for parallel in [false, true] {
        let mut d = Decoder::new(s.width, s.height, &s.config).unwrap();
        d.parallel = parallel;
        let mut at = 0;
        for (i, f) in s.frames.iter().enumerate() {
            let p = d
                .decode(f)
                .unwrap_or_else(|e| panic!("{name} frame {i}: {e}"));
            assert_eq!(p.damaged_slices, 0, "{name} frame {i}");
            let mut got = Vec::new();
            for pl in p.planes.iter().chain(p.alpha.iter()) {
                got.extend_from_slice(pl);
            }
            let exp = &want[at..(at + got.len()).min(want.len())];
            let bad = got.iter().zip(exp).filter(|(a, b)| a != b).count();
            assert!(
                bad == 0 && exp.len() == got.len(),
                "{name}: frame {i} differs from FFmpeg in {bad} of {} bytes (parallel {parallel})",
                got.len()
            );
            at += got.len();
        }
        assert_eq!(at, want.len(), "{name}: frame count");
        params = d.params().cloned();
    }
    params.unwrap()
}

#[test]
fn version_0() {
    let p = check("v0-rice");
    assert_eq!((p.version, p.coder_type), (0, 0));
    let p = check("v0-range");
    assert_eq!((p.version, p.coder_type), (0, 1));
}

#[test]
fn version_1() {
    let p = check("v1-tab10");
    assert_eq!((p.version, p.coder_type, p.bits), (1, 2, 10));
    let p = check("v1-rgb");
    assert_eq!(p.colour_space, ColourSpace::Rgb);
    let p = check("v1-16");
    assert_eq!(p.bits, 16);
}

#[test]
fn version_3_ycbcr() {
    for name in [
        "v3-420", "v3-rice", "v3-ctx", "v3-411", "v3-440", "v3-12", "v3-16",
    ] {
        let p = check(name);
        assert_eq!(p.version, 3, "{name}");
    }
    assert!(check("v3-420").ec);
}

#[test]
fn version_3_alpha_and_grey() {
    assert!(check("v3-alpha").extra_plane);
    for name in ["v3-gray", "v3-gray16"] {
        let p = check(name);
        assert!(!p.chroma_planes, "{name}");
    }
}

#[test]
fn version_3_rgb() {
    for name in ["v3-rgb", "v3-rgba", "v3-rgb10", "v3-rgba12", "v3-rgb16"] {
        let p = check(name);
        assert_eq!(p.colour_space, ColourSpace::Rgb, "{name}");
    }
}

#[test]
fn odd_sizes() {
    check("v3-odd");
    check("v3-odd-rice");
}

#[test]
fn damaged_slice_is_counted_not_fatal() {
    let s = stream("v3-420");
    let mut d = Decoder::new(s.width, s.height, &s.config).unwrap();
    let mut f = s.frames[0].clone();
    let mid = f.len() / 2;
    f[mid] ^= 0x55;
    let p = d.decode(&f).unwrap();
    assert_eq!(p.damaged_slices, 1);
    assert!(matches!(
        p.layout,
        Layout::Ycbcr {
            log2_h: 1,
            log2_v: 1
        }
    ));
}

#[test]
fn bad_config_crc_refused() {
    let s = stream("v3-420");
    let mut c = s.config.clone();
    c[3] ^= 1;
    assert!(Decoder::new(s.width, s.height, &c).is_err());
}

#[test]
fn truncated_frames_do_not_panic() {
    for name in ["v0-rice", "v3-rice", "v3-ctx", "v1-tab10"] {
        let s = stream(name);
        for cut in [1, 2, 7, 40, 200] {
            let mut d = Decoder::new(s.width, s.height, &s.config).unwrap();
            let f = &s.frames[0];
            let _ = d.decode(&f[..f.len().saturating_sub(cut).max(1)]);
        }
    }
}
