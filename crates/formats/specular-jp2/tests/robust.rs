//! Malformed input never panics: truncations and byte mutations of every fixture, decoded at
//! several reductions and with a region. Errors and `truncated` images are both fine.

mod common;

use common::*;
use specular_jp2::{DecodeOptions, Decoder, Depth, Parallelism, Region};

fn try_all(data: &[u8]) {
    let Ok(dec) = Decoder::new(data) else {
        return;
    };
    let info = dec.info().clone();
    // Keep mutated headers from requesting huge outputs in this test.
    if (info.width as u64) * (info.height as u64) > 4 << 20 {
        return;
    }
    for reduce in [0, 1, 3] {
        let _ = dec.decode(&DecodeOptions {
            reduce,
            parallelism: Parallelism::Single,
            ..Default::default()
        });
    }
    let _ = dec.decode(&DecodeOptions {
        region: Some(Region {
            x: info.width / 3,
            y: info.height / 4,
            w: (info.width / 2).max(1),
            h: (info.height / 2).max(1),
        }),
        depth: Depth::Sixteen,
        max_layers: Some(1),
        ..Default::default()
    });
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

#[test]
fn truncations_do_not_panic() {
    for f in fixtures() {
        let data = std::fs::read(&f).unwrap();
        let n = data.len();
        let mut cuts: Vec<usize> = (0..n.min(160)).collect();
        cuts.extend((160..n).step_by((n / 40).max(1)));
        for cut in cuts {
            try_all(&data[..cut]);
        }
    }
}

#[test]
fn truncated_streams_report_truncation() {
    let data = std::fs::read(data_dir().join("rgb_rev.j2k")).unwrap();
    let img = specular_jp2::decode(&data[..data.len() * 2 / 3], &DecodeOptions::default()).unwrap();
    assert!(img.truncated);
    assert_eq!((img.width, img.height), (203, 157));
}

#[test]
fn mutations_do_not_panic() {
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    for f in fixtures() {
        let data = std::fs::read(&f).unwrap();
        for i in 0..80 {
            let mut d = data.clone();
            let k = 1 + (rng.next() % 4) as usize;
            for _ in 0..k {
                // Bias half of the mutations into the headers.
                let lim = if i % 2 == 0 {
                    d.len().min(300)
                } else {
                    d.len()
                };
                let p = (rng.next() % lim as u64) as usize;
                d[p] = match rng.next() % 4 {
                    0 => 0xFF,
                    1 => 0,
                    2 => d[p] ^ (1 << (rng.next() % 8)),
                    _ => rng.next() as u8,
                };
            }
            try_all(&d);
        }
    }
}

#[test]
fn random_bytes_do_not_panic() {
    let mut rng = Rng(7);
    for len in [0usize, 1, 2, 4, 12, 40, 200, 1000] {
        for _ in 0..20 {
            let mut d: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
            try_all(&d);
            // With valid signatures in front.
            let mut cs = vec![0xFF, 0x4F, 0xFF, 0x51];
            cs.append(&mut d.clone());
            try_all(&cs);
            let mut jp2 = b"\0\0\0\x0cjP  \r\n\x87\n".to_vec();
            jp2.append(&mut d);
            try_all(&jp2);
        }
    }
}

/// HTJ2K conformance files (when the corpus is present): truncations and body mutations hit
/// the HT cleanup / SigProp / MagRef decoders with garbage MEL, VLC and MagSgn streams.
#[test]
fn htj2k_damage_does_not_panic() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tools/corpus/conformance/openjpeg-data/input/nonregression/htj2k");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        eprintln!("skipping: {} missing", dir.display());
        return;
    };
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for e in rd.flatten() {
        let data = std::fs::read(e.path()).unwrap();
        let n = data.len();
        for cut in (0..n).step_by((n / 60).max(1)) {
            try_all(&data[..cut]);
        }
        for _ in 0..150 {
            let mut d = data.clone();
            for _ in 0..1 + rng.next() % 6 {
                let p = (rng.next() % n as u64) as usize;
                d[p] = match rng.next() % 3 {
                    0 => d[p] ^ (1 << (rng.next() % 8)),
                    1 => 0xFF,
                    _ => rng.next() as u8,
                };
            }
            try_all(&d);
        }
    }
}

/// The committed HTJ2K vectors: truncations everywhere, and mutations aimed past the first SOD
/// so most land in HT code-block data (MEL, VLC, MagSgn, SigProp and MagRef segments) and the
/// Lcup/Scup lengths the cleanup pass splits them with.
#[test]
fn htj2k_vectors_damage_does_not_panic() {
    let mut rng = Rng(0x6a09_e667_f3bc_c908);
    for f in ht_fixtures() {
        let data = std::fs::read(&f).unwrap();
        let n = data.len();
        let body = data
            .windows(2)
            .position(|w| w == [0xFF, 0x93])
            .map_or(0, |p| p + 2);
        assert!(body > 0 && body < n, "{}: no SOD", name(&f));
        for cut in (0..n).step_by((n / 50).max(1)) {
            try_all(&data[..cut]);
        }
        for _ in 0..60 {
            let mut d = data.clone();
            for _ in 0..1 + rng.next() % 5 {
                let p = body + (rng.next() % (n - body) as u64) as usize;
                d[p] = match rng.next() % 4 {
                    0 => d[p] ^ (1 << (rng.next() % 8)),
                    1 => 0xFF,
                    2 => 0,
                    _ => rng.next() as u8,
                };
            }
            try_all(&d);
        }
    }
}

/// A cut HT stream still decodes, flagged as truncated, at full size.
#[test]
fn htj2k_truncated_streams_report_truncation() {
    let data = std::fs::read(data_dir().join("htj2k/rgb_rev.jph")).unwrap();
    let img = specular_jp2::decode(&data[..data.len() * 2 / 3], &DecodeOptions::default()).unwrap();
    assert!(img.truncated);
    assert_eq!((img.width, img.height), (83, 61));
}
