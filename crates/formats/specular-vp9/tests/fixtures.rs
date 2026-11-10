//! The committed fixtures (`tools/corpus/specular-vp9-fixtures.sh`): libvpx-vp9 encodes of
//! testsrc2 against libvpx's per-frame MD5s, single- and multi-threaded.

use std::path::PathBuf;

use specular_vp9::{Decoder, Error};

mod common;
use common::{decode_md5s, ivf_frames};

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

fn check(name: &str, threads: usize) {
    let dir = data_dir();
    let data = std::fs::read(dir.join(format!("{name}.ivf"))).unwrap();
    let want: Vec<String> = std::fs::read_to_string(dir.join(format!("{name}.md5")))
        .unwrap()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let (got, err) = decode_md5s(&data, threads);
    assert!(err.is_none(), "{name}: {err:?} after {} frames", got.len());
    let bad = got.iter().zip(&want).filter(|(a, b)| a != b).count();
    let first = got.iter().zip(&want).position(|(a, b)| a != b);
    assert_eq!(got.len(), want.len(), "{name}: frame count");
    assert_eq!(
        bad,
        0,
        "{name}: {bad} of {} frames differ, first {first:?}",
        want.len()
    );
}

macro_rules! fixture {
    ($t:ident, $name:expr) => {
        #[test]
        fn $t() {
            check($name, 1);
            check($name, 4);
        }
    };
}

fixture!(basic, "basic-176x144");
fixture!(odd_size, "odd-178x146");
fixture!(tiles, "tiles-1280x96");
fixture!(ten_bit, "p2-10bit-176x144");
fixture!(twelve_bit, "p2-12bit-96x64");
fixture!(lossless, "lossless-96x64");
fixture!(chroma_422, "p1-422-176x144");
fixture!(chroma_440, "p1-440-96x64");
fixture!(error_resilient, "errres-176x144");
fixture!(frame_parallel, "fpar-176x144");
fixture!(high_quality, "hq-176x144");

fixture!(chroma_444, "p1-444-64x64");
fixture!(chroma_444_altref, "p1-444-176x144");
fixture!(chroma_444_10bit, "p3-444-10bit-96x64");
fixture!(chroma_444_12bit, "p3-444-12bit-64x64");
fixture!(chroma_440_10bit, "p3-440-10bit-96x64");
fixture!(rgb, "p1-gbr-96x64");

#[test]
fn probe_444() {
    let data = std::fs::read(data_dir().join("p1-444-64x64.ivf")).unwrap();
    let first = ivf_frames(&data)[0];
    let info = specular_vp9::probe(first).unwrap();
    assert!(!info.ss_x && !info.ss_y);
    assert_eq!(info.profile, 1);
}

/// Every frame (current and references) is charged to the memory budget: with no room, a
/// frame is refused instead of allocated (a 16384² header-only stream asked for 1.6 GB a
/// frame before).
#[test]
fn frames_charged_to_the_budget() {
    let data = std::fs::read(data_dir().join("basic-176x144.ivf")).unwrap();
    let first = ivf_frames(&data)[0];
    let r = specular_core::budget::with_limit(16 << 10, || {
        Decoder::new(1).decode(first, &mut Vec::new())
    });
    assert!(matches!(r, Err(Error::Unsupported(m)) if m.contains("VP9 video")));
    assert!(Decoder::new(1).decode(first, &mut Vec::new()).is_ok());
}

#[test]
fn probe_and_keyframes() {
    let data = std::fs::read(data_dir().join("p2-10bit-176x144.ivf")).unwrap();
    let frames = ivf_frames(&data);
    let info = specular_vp9::probe(frames[0]).unwrap();
    assert_eq!((info.width, info.height, info.bit_depth), (176, 144, 10));
    assert_eq!(info.profile, 2);
    assert!(specular_vp9::is_keyframe(frames[0]));
    assert!(!specular_vp9::is_keyframe(frames[1]));
    assert!(specular_vp9::probe(frames[1]).is_none());
}

/// Damaged input never panics; after an error the decoder waits for a keyframe.
#[test]
fn damaged_input() {
    let data = std::fs::read(data_dir().join("basic-176x144.ivf")).unwrap();
    let frames = ivf_frames(&data);
    let mut seed = 12345u32;
    for round in 0..200 {
        let mut d = Decoder::new(if round % 2 == 0 { 1 } else { 3 });
        let mut out = Vec::new();
        for (i, f) in frames.iter().enumerate() {
            let mut f = f.to_vec();
            if i == round % frames.len() || round % 7 == 0 {
                for _ in 0..1 + round % 5 {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    let k = seed as usize % f.len();
                    f[k] ^= (seed >> 8) as u8;
                }
                if round % 3 == 0 {
                    f.truncate(f.len() / 2);
                }
            }
            let _ = d.decode(&f, &mut out);
        }
    }
    // A non-keyframe after an error is refused.
    let mut d = Decoder::new(1);
    let mut out = Vec::new();
    assert!(d.decode(&[0x80, 0, 0], &mut out).is_err());
    assert!(d.decode(frames[1], &mut out).is_err());
    assert!(d.decode(frames[0], &mut out).is_ok());
}
