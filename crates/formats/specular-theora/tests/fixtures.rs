//! Frame-exact against ffmpeg (`-f framemd5`) on the clips of `tests/data` (see
//! `gen_fixtures.sh`), plus header, keyframe, lacing and damaged-packet checks.

mod md5;
mod ogg;

use std::path::{Path, PathBuf};

use specular_theora::{Chroma, Decoder, Error, Info, Picture, is_keyframe, split_xiph_laced};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

fn md5_of(p: &Picture) -> String {
    let mut m = md5::Md5::new();
    for plane in &p.planes {
        m.update(plane);
    }
    m.hex()
}

/// ffmpeg's frame MD5s by packet index (its pts); repeat (empty) packets have none.
fn expected(path: &Path) -> Vec<Option<String>> {
    let mut out = Vec::new();
    for l in std::fs::read_to_string(path).unwrap().lines() {
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

fn open(name: &str) -> (Decoder, Vec<Vec<u8>>) {
    let pk = ogg::packets(&std::fs::read(data_dir().join(name)).unwrap());
    let d = Decoder::new(&pk[0], &pk[2]).expect("headers");
    (d, pk[3..].to_vec())
}

/// (bit-exact frames, frames ffmpeg output, packets decoded). Empty packets must repeat
/// the previous frame (ffmpeg outputs nothing for them).
fn check(name: &str) -> (usize, usize, usize) {
    let (mut d, frames) = open(name);
    let want = expected(&data_dir().join(name).with_extension("framemd5"));
    let mut good = 0;
    let mut prev: Option<Picture> = None;
    for (i, f) in frames.iter().enumerate() {
        let p = d
            .decode(f)
            .unwrap_or_else(|e| panic!("{name} frame {i}: {e}"));
        assert_eq!(p.keyframe, is_keyframe(f));
        match want.get(i) {
            Some(Some(w)) if *w == md5_of(&p) => good += 1,
            Some(None) if f.is_empty() => {
                let q = prev.as_ref().expect("a frame to repeat");
                assert_eq!(q.planes, p.planes, "{name}: repeat {i}");
            }
            _ => eprintln!("{name}: frame {i} differs"),
        }
        prev = Some(p);
    }
    (good, want.iter().flatten().count(), frames.len())
}

#[test]
fn clips_match_ffmpeg() {
    let mut failed = Vec::new();
    for name in [
        "c420.ogv", "odd.ogv", "low.ogv", "c444.ogv", "c422.ogv", "cbr.ogv", "wide.ogv",
    ] {
        let (good, want, packets) = check(name);
        eprintln!("{name}: {good} of {want} frames bit-exact ({packets} packets)");
        if good != want {
            failed.push(name);
        }
    }
    assert!(failed.is_empty(), "mismatches in {failed:?}");
}

#[test]
fn headers() {
    let pk = ogg::packets(&std::fs::read(data_dir().join("odd.ogv")).unwrap());
    let i = Info::parse(&pk[0]).unwrap();
    assert_eq!((i.frame_width, i.frame_height), (176, 112));
    assert_eq!((i.pic_width, i.pic_height), (170, 98));
    assert_eq!(i.chroma, Chroma::C420);
    assert_eq!(i.fps, (25, 1));
    assert_eq!((i.version.0, i.version.1), (3, 2));
    let pk = ogg::packets(&std::fs::read(data_dir().join("c422.ogv")).unwrap());
    assert_eq!(Info::parse(&pk[0]).unwrap().chroma, Chroma::C422);
    assert!(matches!(Info::parse(&pk[1]), Err(Error::Malformed(_))));
}

#[test]
fn decoder_reserves_from_the_budget() {
    // An 8192 × 8192 frame needs about 230 MB of buffers.
    let pk = ogg::packets(&std::fs::read(data_dir().join("c420.ogv")).unwrap());
    let mut ident = pk[0].clone();
    ident[10..14].copy_from_slice(&[2, 0, 2, 0]);
    let r =
        specular_core::budget::with_limit(64 << 20, || Decoder::new(&ident, &pk[2]).map(|_| ()));
    assert!(
        matches!(&r, Err(Error::NeedsMemory(n)) if n.needed > 200_000_000),
        "{r:?}"
    );
    // Held while the decoder lives.
    let b = std::sync::Arc::new(specular_core::budget::MemoryBudget::fixed(1 << 30));
    let d = specular_core::budget::with_budget(b.clone(), || Decoder::new(&pk[0], &pk[2]).unwrap());
    assert!(b.reserved() > 0);
    drop(d);
    assert_eq!(b.reserved(), 0);
}

#[test]
fn keyframes_reset_and_repeat() {
    let (mut d, frames) = open("c420.ogv");
    let keys: Vec<usize> = (0..frames.len())
        .filter(|&i| is_keyframe(&frames[i]))
        .collect();
    assert_eq!(keys, [0, 12, 24]);
    let want = expected(&data_dir().join("c420.framemd5"));
    // Inter frames before the first keyframe are refused, and so is an empty packet.
    assert!(matches!(d.decode(&frames[1]), Err(Error::Malformed(_))));
    assert!(d.decode(&[]).is_err());
    let p = d.decode(&frames[0]).unwrap();
    let again = d.decode(&[]).unwrap();
    assert_eq!(again.planes, p.planes);
    assert!(!again.keyframe);
    // After a seek: wait for the keyframe at 12.
    d.reset();
    assert!(d.decode(&frames[5]).is_err());
    for i in 12..frames.len() {
        let p = d.decode(&frames[i]).unwrap();
        assert_eq!(Some(md5_of(&p)), want[i], "frame {i} after reset");
    }
}

#[test]
fn xiph_lacing() {
    let pk = ogg::packets(&std::fs::read(data_dir().join("c444.ogv")).unwrap());
    let mut private = vec![2u8];
    for p in &pk[..2] {
        let mut n = p.len();
        while n >= 255 {
            private.push(255);
            n -= 255;
        }
        private.push(n as u8);
    }
    for p in &pk[..3] {
        private.extend_from_slice(p);
    }
    let [a, b, c] = split_xiph_laced(&private).unwrap();
    assert_eq!((a, b, c), (&pk[0][..], &pk[1][..], &pk[2][..]));
    assert!(split_xiph_laced(&private[..10]).is_none());
    assert!(split_xiph_laced(&[]).is_none());
}

/// Damaged packets and headers never panic.
#[test]
fn damaged_input_does_not_panic() {
    for name in ["c420.ogv", "c444.ogv", "c422.ogv", "odd.ogv"] {
        let pk = ogg::packets(&std::fs::read(data_dir().join(name)).unwrap());
        let mut seed = 0x1234_5678u32;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        // Damaged setup headers.
        for _ in 0..40 {
            let mut s = pk[2].clone();
            let n = rnd() as usize % s.len();
            s[n] ^= 1 << (rnd() % 8);
            if rnd() % 3 == 0 {
                s.truncate(rnd() as usize % s.len());
            }
            let _ = Decoder::new(&pk[0], &s);
        }
        let mut d = Decoder::new(&pk[0], &pk[2]).unwrap();
        for f in &pk[3..] {
            for _ in 0..6 {
                let mut g = f.clone();
                match rnd() % 3 {
                    0 => g.truncate(rnd() as usize % g.len().max(1)),
                    1 => {
                        for _ in 0..4 {
                            let n = rnd() as usize % g.len().max(1);
                            if let Some(b) = g.get_mut(n) {
                                *b = rnd() as u8;
                            }
                        }
                    }
                    _ => {
                        let n = rnd() as usize % g.len().max(1);
                        for b in g.iter_mut().skip(n) {
                            *b = rnd() as u8;
                        }
                    }
                }
                let _ = d.decode(&g);
            }
            let _ = d.decode(f);
        }
        // Random junk as frames.
        for len in [1usize, 2, 7, 100, 3000] {
            let junk: Vec<u8> = (0..len).map(|_| rnd() as u8 & 0x3f).collect();
            let _ = d.decode(&junk);
        }
    }
}
