//! Synthetic files from gen/make_vectors.py (jxrlib as the oracle), decoded bit-exact.

use std::path::Path;

use specular_jxr::{
    Bands, BitDepth, ClrFmt, DecodeOptions, Error, Kind, OutFmt, Pixels, Samples, decode,
    decode_native, probe, read_info,
};

fn bytes(s: &Samples) -> Vec<u8> {
    match s {
        Samples::U8(v) => v.clone(),
        Samples::U16(v) => v.iter().flat_map(|x| x.to_le_bytes()).collect(),
        Samples::U32(v) => v.iter().flat_map(|x| x.to_le_bytes()).collect(),
    }
}

fn fnv(data: &[u8]) -> u64 {
    data.iter().fold(0xCBF2_9CE4_8422_2325u64, |h, &b| {
        (h ^ b as u64).wrapping_mul(0x100_0000_01B3)
    })
}

struct Vector {
    name: String,
    data: Vec<u8>,
    width: u32,
    height: u32,
    bpp: usize,
    hash: u64,
}

fn vectors() -> Vec<Vector> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors");
    let manifest = std::fs::read_to_string(dir.join("manifest.txt")).unwrap();
    manifest
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            Vector {
                name: f[0].to_string(),
                data: std::fs::read(dir.join(format!("{}.jxr", f[0]))).unwrap(),
                width: f[1].parse().unwrap(),
                height: f[2].parse().unwrap(),
                bpp: f[3].parse().unwrap(),
                hash: u64::from_str_radix(f[4], 16).unwrap(),
            }
        })
        .collect()
}

#[test]
fn all_vectors_bit_exact() {
    let v = vectors();
    assert!(v.len() >= 50);
    let mut bad = Vec::new();
    for t in &v {
        assert!(probe(&t.data), "{}", t.name);
        let info = read_info(&t.data).unwrap_or_else(|e| panic!("{}: {e}", t.name));
        assert_eq!((info.width, info.height), (t.width, t.height), "{}", t.name);
        for serial in [false, true] {
            let opts = DecodeOptions {
                serial,
                ..Default::default()
            };
            match decode_native(&t.data, &opts) {
                Ok(n) => {
                    assert!(!n.truncated, "{}", t.name);
                    let b = bytes(&n.samples);
                    assert_eq!(b.len(), (t.width * t.height) as usize * t.bpp, "{}", t.name);
                    if fnv(&b) != t.hash {
                        bad.push(format!("{} (serial {serial})", t.name));
                    }
                }
                Err(e) => bad.push(format!("{}: {e}", t.name)),
            }
        }
        // The display conversion accepts every vector.
        let img = decode(&t.data, &DecodeOptions::default())
            .unwrap_or_else(|e| panic!("{}: {e}", t.name));
        let n = (img.width * img.height) as usize * img.channels();
        let len = match &img.pixels {
            Pixels::U8(p) => p.len(),
            Pixels::U16(p) => p.len(),
            Pixels::F32(p) => p.len(),
        };
        assert_eq!(len, n, "{}", t.name);
    }
    assert!(bad.is_empty(), "mismatches: {bad:?}");
}

#[test]
fn header_details() {
    let v = vectors();
    let get = |n: &str| &v.iter().find(|x| x.name == n).unwrap().data;
    let i = read_info(get("gray8-tiles")).unwrap();
    assert_eq!(i.tiles, (3, 2));
    assert_eq!(
        (i.internal, i.output, i.bit_depth),
        (ClrFmt::YOnly, OutFmt::YOnly, BitDepth::Bd8)
    );
    let i = read_info(get("gray8-spatial")).unwrap();
    assert!(!i.frequency);
    let i = read_info(get("gray8-l2")).unwrap();
    assert_eq!(i.overlap, 2);
    let i = read_info(get("gray8-dconly")).unwrap();
    assert_eq!(i.bands, Bands::DcOnly);
    let i = read_info(get("rgb24-d420")).unwrap();
    assert_eq!(i.internal, ClrFmt::Yuv420);
    let i = read_info(get("bgra32-q-a2")).unwrap();
    assert!(i.has_alpha && i.planar_alpha);
    let i = read_info(get("bgra32-q-a3")).unwrap();
    assert!(i.has_alpha && !i.planar_alpha);
    let i = read_info(get("rgb128f-q")).unwrap();
    assert_eq!(i.bit_depth, BitDepth::Bd32F);
    let i = read_info(get("cmyk32-lossless")).unwrap();
    assert_eq!((i.internal, i.output), (ClrFmt::Yuvk, OutFmt::Cmyk));
    let img = decode(get("cmyk32-lossless"), &DecodeOptions::default()).unwrap();
    assert_eq!((img.kind, img.alpha), (Kind::Cmyk, false));
    let img = decode(get("rgbe-q"), &DecodeOptions::default()).unwrap();
    assert!(matches!(img.pixels, Pixels::F32(_)));
    let img = decode(get("rgba64half-q-a2"), &DecodeOptions::default()).unwrap();
    assert_eq!((img.kind, img.alpha), (Kind::Rgb, true));
}

/// Every prefix decodes to an error or a (possibly truncated) image, random corruption never
/// panics, and cancellation is honoured.
#[test]
fn truncation_and_corruption() {
    let mut seed = 0x9E37_79B9u32;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    // SPECULAR_JXR_FUZZ raises the corruption rounds per vector for a longer local run.
    let rounds: u32 = std::env::var("SPECULAR_JXR_FUZZ")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    for t in vectors() {
        let step = (t.data.len() / 40).max(1);
        for cut in (0..t.data.len()).step_by(step) {
            match decode_native(&t.data[..cut], &DecodeOptions::default()) {
                Ok(n) => assert_eq!(n.width, t.width, "{} cut {cut}", t.name),
                Err(e) => assert!(
                    matches!(
                        e,
                        Error::NotJxr
                            | Error::Truncated(_)
                            | Error::Malformed(_)
                            | Error::Unsupported(_)
                    ),
                    "{} cut {cut}: {e}",
                    t.name
                ),
            }
        }
        for _ in 0..rounds {
            let mut d = t.data.clone();
            for _ in 0..1 + rnd() % 3 {
                let i = rnd() as usize % d.len();
                d[i] = rnd() as u8;
            }
            let _ = decode(&d, &DecodeOptions::default());
        }
    }
    let cancel = std::sync::atomic::AtomicBool::new(true);
    let v = vectors();
    let opts = DecodeOptions {
        cancel: Some(&cancel),
        ..Default::default()
    };
    assert_eq!(
        decode_native(&v[0].data, &opts).unwrap_err(),
        Error::Cancelled
    );
}

/// Oversized images are refused before allocation.
#[test]
fn sample_limit() {
    let v = vectors();
    let opts = DecodeOptions {
        max_samples: 100,
        ..Default::default()
    };
    assert_eq!(
        decode_native(&v[0].data, &opts).unwrap_err(),
        Error::TooLarge
    );
}

#[test]
fn planes_and_samples_reserve_from_the_budget() {
    // Refused before the planes are allocated, as a memory need rather than a bad file.
    let v = vectors();
    let r = specular_core::budget::with_limit(1024, || {
        decode_native(&v[0].data, &DecodeOptions::default())
    });
    assert!(matches!(r, Err(Error::NeedsMemory(_))), "{:?}", r.err());
    // Nothing is held once the decode is done.
    let b = std::sync::Arc::new(specular_core::budget::MemoryBudget::fixed(1 << 30));
    specular_core::budget::with_budget(b.clone(), || decode(&v[0].data, &DecodeOptions::default()))
        .unwrap();
    assert_eq!(b.reserved(), 0);
}
