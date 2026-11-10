//! VP3.1 (headerless, the tables of the specification's Appendix B) and Theora from FFmpeg's
//! FATE samples, frame-exact against ffmpeg (`tools/corpus/fetch-vpx3.sh` writes the packets
//! as NAME.pkts and ffmpeg's NAME.framemd5). Skips without the corpus
//! (`SPECULAR_VPX3_CORPUS`, default $CARGO_TARGET_DIR/corpus/vpx3).

mod md5;

use std::path::{Path, PathBuf};

use specular_theora::{Decoder, Picture, is_keyframe, is_vp3_keyframe};

fn corpus() -> PathBuf {
    std::env::var_os("SPECULAR_VPX3_CORPUS").map_or_else(
        || {
            option_env!("CARGO_TARGET_DIR")
                .map_or_else(
                    || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target"),
                    PathBuf::from,
                )
                .join("corpus/vpx3")
        },
        PathBuf::from,
    )
}

fn md5_of(p: &Picture) -> String {
    let mut m = md5::Md5::new();
    for plane in &p.planes {
        m.update(plane);
    }
    m.hex()
}

/// ffmpeg's frame MD5s by presentation index; dropped (empty) packets have none.
fn expected(path: &Path) -> Vec<Option<String>> {
    let mut out = Vec::new();
    for l in std::fs::read_to_string(path).unwrap_or_default().lines() {
        if l.starts_with('#') || l.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        let pts: usize = f[1].parse().unwrap();
        out.resize(pts, None);
        out.push(Some(f[f.len() - 1].to_owned()));
    }
    out
}

fn packets(path: &Path) -> Vec<Vec<u8>> {
    let d = std::fs::read(path).unwrap();
    let mut out = Vec::new();
    let mut p = 0;
    while p + 4 <= d.len() {
        let n = u32::from_le_bytes(d[p..p + 4].try_into().unwrap()) as usize;
        out.push(d[p + 4..p + 4 + n].to_vec());
        p += 4 + n;
    }
    out
}

/// Decodes every packet; the frames that differ from ffmpeg's and the frame count.
fn run(mut d: Decoder, frames: &[Vec<u8>], want: &[Option<String>]) -> (usize, usize) {
    let mut bad = 0;
    for (i, f) in frames.iter().enumerate() {
        let p = match d.decode(f) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("  frame {i}: {e}");
                bad += 1;
                continue;
            }
        };
        if !f.is_empty() && d.info().vp3 {
            assert_eq!(p.keyframe, is_vp3_keyframe(f));
        } else if !f.is_empty() {
            assert_eq!(p.keyframe, is_keyframe(f));
        }
        if let Some(Some(w)) = want.get(i)
            && *w != md5_of(&p)
        {
            if bad == 0 {
                eprintln!("  first difference at frame {i}");
            }
            bad += 1;
        }
    }
    (bad, want.iter().flatten().count())
}

#[test]
#[ignore = "needs tools/corpus/fetch-vpx3.sh"]
fn fate_vp3_and_theora() {
    let dir = corpus().join("vp3");
    if !dir.is_dir() {
        eprintln!("skipped: run tools/corpus/fetch-vpx3.sh");
        return;
    }
    let mut failed = 0;
    for name in ["vp31.avi", "coeff_level64.mkv", "offset_test.ogv"] {
        let pk = dir.join(format!("{name}.pkts"));
        if !pk.exists() {
            println!("ORACLE\tVP3\t{name}\tskip");
            continue;
        }
        let pk = packets(&pk);
        let want = expected(&dir.join(format!("{name}.framemd5")));
        let (dec, frames) = if name.ends_with(".avi") {
            (Decoder::new_vp3(640, 272).unwrap(), &pk[..])
        } else {
            (Decoder::new(&pk[0], &pk[2]).unwrap(), &pk[3..])
        };
        let (bad, n) = run(dec, frames, &want);
        println!(
            "ORACLE\t{}\t{name}\t{}",
            if name.ends_with(".avi") {
                "VP3"
            } else {
                "Theora"
            },
            if bad == 0 { "pass" } else { "fail" }
        );
        eprintln!("{name}: {bad} of {n} frames differ");
        failed += usize::from(bad > 0);
    }
    assert_eq!(failed, 0);
}

/// The built-in VP3 tables parse as a setup header; a VP3 decoder refuses garbage without
/// panicking and needs a key frame first.
#[test]
fn vp3_decoder_without_corpus() {
    let mut d = Decoder::new_vp3(64, 48).unwrap();
    assert_eq!(d.info().frame_width, 64);
    assert!(d.info().vp3);
    assert!(d.decode(&[0x80, 0, 0, 0]).is_err(), "inter frame first");
    let mut seed = 7u32;
    for n in 0..200usize {
        let mut f = vec![0u8; 1 + n % 97];
        for b in f.iter_mut() {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            *b = seed as u8;
        }
        let _ = d.decode(&f);
    }
    assert!(Decoder::new_vp3(0, 16).is_err());
    // Past Theora's largest frame; a large one within it opens when memory allows.
    assert!(Decoder::new_vp3(0x10_0000, 16).is_err());
    assert!(Decoder::new_vp3(9000, 16).is_ok());
}
