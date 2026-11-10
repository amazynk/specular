//! Every fixture of `tests/data/fixtures.txt` (see `gen_fixtures.sh`) decoded data unit by data
//! unit and compared with FFmpeg's decode, or, for the four where FFmpeg departs from the
//! specification's lifting (Daubechies (9,7), transforms with one- or two-sample bands), with
//! `vc2_model.py`, a transcription of the specification that agrees with FFmpeg everywhere
//! else: every sample must match.

use std::path::Path;

use specular_vc2::{Chroma, Decoder, Picture};

fn data(name: &str) -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The data units of a raw stream, each with its parse info header.
fn units(d: &[u8]) -> Vec<&[u8]> {
    let mut v = Vec::new();
    let mut at = 0;
    while at + 13 <= d.len() {
        let next = u32::from_be_bytes(d[at + 5..at + 9].try_into().unwrap()) as usize;
        let end = if next == 0 { d.len() } else { at + next };
        v.push(&d[at..end]);
        at = end;
    }
    v
}

/// The planes FFmpeg writes per frame: (width, height).
fn plane_sizes(w: usize, h: usize, chroma: &str) -> [(usize, usize); 3] {
    let c = match chroma {
        "444" => (w, h),
        "422" => (w.div_ceil(2), h),
        _ => (w.div_ceil(2), h.div_ceil(2)),
    };
    [(w, h), c, c]
}

fn check(name: &str, w: usize, h: usize, chroma: &str, bits: u8) -> usize {
    let stream = data(&format!("{name}.drc"));
    let want = data(&format!("{name}.yuv"));
    let bps = if bits > 8 { 2 } else { 1 };
    let sizes = plane_sizes(w, h, chroma);
    let frame_bytes: usize = sizes.iter().map(|(a, b)| a * b * bps).sum();
    let mut dec = Decoder::new();
    let mut frames: Vec<Picture> = Vec::new();
    for u in units(&stream) {
        if let Some(p) = dec.decode(u).unwrap_or_else(|e| panic!("{name}: {e}")) {
            frames.push(p);
        }
    }
    assert_eq!(
        frames.len(),
        want.len() / frame_bytes,
        "{name}: frame count"
    );
    let mut mismatches = 0;
    for (i, p) in frames.iter().enumerate() {
        assert_eq!((p.width as usize, p.height as usize), (w, h), "{name}");
        assert_eq!(p.bit_depth, bits, "{name}");
        let c = match chroma {
            "444" => Chroma::C444,
            "422" => Chroma::C422,
            _ => Chroma::C420,
        };
        assert_eq!(p.chroma, c, "{name}");
        let mut at = i * frame_bytes;
        for (k, &(pw, ph)) in sizes.iter().enumerate() {
            assert_eq!(p.planes[k].len(), pw * ph, "{name} plane {k}");
            for (j, &v) in p.planes[k].iter().enumerate() {
                let o = if bps == 1 {
                    u16::from(want[at + j])
                } else {
                    u16::from_le_bytes([want[at + 2 * j], want[at + 2 * j + 1]])
                };
                if o != v {
                    if mismatches < 5 {
                        eprintln!(
                            "{name} frame {i} plane {k} ({}, {}): ours {v}, FFmpeg {o}",
                            j % pw,
                            j / pw
                        );
                    }
                    mismatches += 1;
                }
            }
            at += pw * ph * bps;
        }
    }
    mismatches
}

#[test]
fn fixtures_match_ffmpeg() {
    let list = String::from_utf8(data("fixtures.txt")).unwrap();
    let mut failed = Vec::new();
    for line in list.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let (w, h, bits) = (
            f[1].parse().unwrap(),
            f[2].parse().unwrap(),
            f[4].parse().unwrap(),
        );
        let n = check(f[0], w, h, f[3], bits);
        eprintln!("{}: {n} mismatching samples", f[0]);
        if n != 0 {
            failed.push(f[0].to_owned());
        }
    }
    assert!(failed.is_empty(), "mismatches in {failed:?}");
}

#[test]
fn probe_and_refusal() {
    let s = data("ld-legall-422-10.drc");
    let info = specular_vc2::probe(&s).unwrap();
    assert_eq!((info.width, info.height, info.bit_depth), (64, 48, 10));
    assert_eq!(info.chroma, Chroma::C422);
    assert_eq!(info.profile, "VC-2 low delay");
    assert_eq!(specular_vc2::refusal(&s), None);
    let s = data("ff-dd97-420.drc");
    assert_eq!(
        specular_vc2::probe(&s).unwrap().profile,
        "VC-2 high quality"
    );
    let s = data("ld-haar-fields.drc");
    assert!(specular_vc2::probe(&s).unwrap().interlaced);
    // A core-syntax picture (arithmetic-coded intra) after a valid sequence header.
    let mut core = units(&data("ff-dd97-420.drc"))[0].to_vec();
    core.extend_from_slice(b"BBCD\x0c\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00");
    let why = specular_vc2::refusal(&core).unwrap();
    assert!(why.contains("Dirac core"), "{why}");
    let mut d = Decoder::new();
    assert!(d.decode(&core).is_err());
}

/// Timing of a large stream given by `SPECULAR_VC2_BENCH` (a raw VC-2 file); ignored by
/// default: `cargo test -p specular-vc2 -- --ignored --nocapture bench`.
#[test]
#[ignore]
fn bench() {
    let Ok(path) = std::env::var("SPECULAR_VC2_BENCH") else {
        return;
    };
    let s = std::fs::read(path).unwrap();
    let mut d = Decoder::new();
    let t = std::time::Instant::now();
    let mut n = 0u32;
    for u in units(&s) {
        if d.decode(u).unwrap().is_some() {
            n += 1;
        }
    }
    eprintln!(
        "{n} frames, {:.1} ms per frame",
        t.elapsed().as_secs_f64() * 1e3 / f64::from(n.max(1))
    );
}

#[test]
fn truncated_and_damaged_streams_do_not_panic() {
    let s = data("hq-dd137-deep.drc");
    let us = units(&s);
    let mut d = Decoder::new();
    d.decode(us[0]).unwrap();
    let pic = us[1];
    for cut in [14, 20, 40, pic.len() / 2, pic.len() - 1] {
        let _ = d.decode(&pic[..cut]);
    }
    let mut bad = pic.to_vec();
    for (i, b) in bad.iter_mut().enumerate().skip(13) {
        *b ^= (i * 37) as u8;
    }
    let _ = d.decode(&bad);
    // An absurd sequence header: 2^31-wide pictures are refused, not allocated.
    let mut seq = Vec::from(&b"BBCD\x00\x00\x00\x00\x00\x00\x00\x00\x00"[..]);
    // major 2, minor 0, profile 3, level 0, base 0, custom size (huge), then zeros.
    seq.extend_from_slice(&[0b0110_0000, 0b0100_0000, 0, 0, 0, 0, 0, 0, 0, 1, 0xff]);
    assert!(Decoder::new().decode(&seq).is_err());
}
