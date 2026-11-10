//! Small libvpx-encoded clips (made by `tools/corpus/specular-vp8-fixtures.sh`) against the
//! per-frame MD5s of ffmpeg's VP8 decoder, with 1 and 3 threads; and damaged input.

mod common;

use std::path::PathBuf;

use specular_vp8::{Decoder, Error, is_keyframe, probe};

fn data(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name);
    std::fs::read(p).unwrap()
}

fn check(name: &str) {
    let ivf = data(&format!("{name}.ivf"));
    let want = common::expected(&String::from_utf8(data(&format!("{name}.framemd5"))).unwrap());
    let frames = common::ivf_frames(&ivf);
    assert!(!want.is_empty());
    for threads in [1, 3] {
        let (got, err) = common::decode_md5s(&frames, threads);
        assert!(err.is_none(), "{name}: {err:?}");
        let m = common::mismatches(&got, &want);
        assert_eq!(
            m,
            0,
            "{name} ({threads} threads): {m} of {} frames differ",
            want.len()
        );
    }
}

#[test]
fn odd_size_four_partitions() {
    check("odd-partitions");
}

#[test]
fn altref_hidden_frames() {
    check("altref");
}

#[test]
fn error_resilient_eight_partitions() {
    check("resilient");
}

#[test]
fn low_bitrate_realtime() {
    check("lowrate");
}

#[test]
fn version_1() {
    check("profile1");
}

#[test]
fn version_2() {
    check("profile2");
}

#[test]
fn version_3() {
    check("profile3");
}

#[test]
fn high_quality() {
    check("highq");
}

#[test]
fn probe_and_keyframes() {
    let ivf = data("odd-partitions.ivf");
    let frames = common::ivf_frames(&ivf);
    let info = probe(frames[0]).unwrap();
    assert_eq!(
        (info.width, info.height, info.h_scale, info.v_scale),
        (176, 142, 0, 0)
    );
    assert!(is_keyframe(frames[0]));
    assert!(!is_keyframe(frames[1]));
    assert!(probe(frames[1]).is_none());
    let mut d = Decoder::new(1);
    let p = d.decode(frames[0]).unwrap().unwrap();
    assert_eq!((p.width, p.height), (176, 142));
    assert_eq!(p.planes[0].len(), 176 * 142);
    assert_eq!(p.planes[1].len(), 88 * 71);
    assert!(!p.reserved_color_space);
}

#[test]
fn interframe_first_is_refused() {
    let ivf = data("resilient.ivf");
    let frames = common::ivf_frames(&ivf);
    let mut d = Decoder::new(1);
    assert!(matches!(d.decode(frames[1]), Err(Error::Malformed(_))));
    assert!(d.decode(frames[0]).unwrap().is_some());
    assert!(d.decode(frames[1]).unwrap().is_some());
    d.reset();
    assert!(d.decode(frames[2]).is_err());
}

/// Truncated and corrupted frames return errors (or garbage pictures), never panic, and
/// the next key frame decodes exactly again.
#[test]
fn damaged_frames() {
    for name in ["odd-partitions", "altref", "lowrate", "profile1"] {
        let ivf = data(&format!("{name}.ivf"));
        let frames = common::ivf_frames(&ivf);
        let want = common::expected(&String::from_utf8(data(&format!("{name}.framemd5"))).unwrap());
        let mut seed = 0x1234_5678u32;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        for threads in [1, 4] {
            let mut d = Decoder::new(threads);
            let mut errors = 0;
            for f in &frames {
                // Truncate, flip bytes, or both.
                let mut v = f.to_vec();
                match rnd() % 4 {
                    0 => v.truncate(rnd() as usize % (v.len() + 1)),
                    1 => {
                        for _ in 0..1 + rnd() % 8 {
                            let i = rnd() as usize % v.len();
                            v[i] ^= 1 << (rnd() % 8);
                        }
                    }
                    2 => v.truncate(v.len() / 2),
                    _ => {}
                }
                errors += d.decode(&v).is_err() as usize;
            }
            assert!(errors > 0, "{name}: damage went unnoticed");
            // Recovery at the key frame.
            let (got, err) = {
                d.reset();
                let mut out = Vec::new();
                let mut err = None;
                for f in &frames {
                    match d.decode(f) {
                        Ok(Some(p)) => {
                            let mut h = common::md5::Md5::new();
                            for pl in &p.planes {
                                h.update(pl);
                            }
                            out.push(h.hex());
                        }
                        Ok(None) => {}
                        Err(e) => err = Some(e),
                    }
                }
                (out, err)
            };
            assert!(err.is_none());
            assert_eq!(common::mismatches(&got, &want), 0);
        }
    }
    // Garbage of every short length.
    let mut d = Decoder::new(2);
    for n in 0..64 {
        let v: Vec<u8> = (0..n).map(|i| (i * 37 + 11) as u8).collect();
        let _ = d.decode(&v);
        let mut k = v.clone();
        if n >= 10 {
            k[0] &= !1;
            k[3..6].copy_from_slice(&[0x9d, 0x01, 0x2a]);
            let _ = d.decode(&k);
        }
    }
}
