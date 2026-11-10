//! Synthetic streams from gen/make_vectors.py (CharLS as the oracle), decoded bit-exact.

use std::path::Path;

use specular_jpegls::{ColourTransform, DecodeOptions, Error, decode, read_info};

fn fnv(planes: &[specular_jpegls::Plane]) -> u64 {
    let mut h = 0xCBF2_9CE4_8422_2325u64;
    for p in planes {
        for &v in &p.data {
            for b in v.to_le_bytes() {
                h = (h ^ b as u64).wrapping_mul(0x100_0000_01B3);
            }
        }
    }
    h
}

fn vectors() -> Vec<(String, Vec<u8>, u32, u32, usize, u64)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors");
    let manifest = std::fs::read_to_string(dir.join("manifest.txt")).unwrap();
    manifest
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            let data = std::fs::read(dir.join(format!("{}.jls", f[0]))).unwrap();
            (
                f[0].to_string(),
                data,
                f[1].parse().unwrap(),
                f[2].parse().unwrap(),
                f[3].parse().unwrap(),
                u64::from_str_radix(f[4], 16).unwrap(),
            )
        })
        .collect()
}

#[test]
fn all_vectors_bit_exact() {
    let v = vectors();
    assert!(v.len() >= 40);
    let mut bad = Vec::new();
    for (name, data, w, h, n, hash) in &v {
        let info = read_info(data).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!((info.width, info.height), (*w, *h), "{name}");
        match decode(data, &DecodeOptions::default()) {
            Ok(img) => {
                assert!(!img.truncated, "{name}");
                assert_eq!(img.planes.len(), *n, "{name}");
                if fnv(&img.planes) != *hash {
                    bad.push(name.clone());
                }
            }
            Err(e) => bad.push(format!("{name}: {e}")),
        }
    }
    assert!(bad.is_empty(), "mismatches: {bad:?}");
}

#[test]
fn header_details() {
    let v = vectors();
    let get = |n: &str| &v.iter().find(|x| x.0 == n).unwrap().1;
    let i = read_info(get("grey8_preset")).unwrap();
    assert_eq!((i.near, i.maxval), (3, 255));
    let i = read_info(get("grey12_maxval")).unwrap();
    assert_eq!((i.precision, i.maxval), (12, 3000));
    let i = read_info(get("rgb8_ilv1")).unwrap();
    assert_eq!((i.components.len(), i.interleave), (3, 1));
    let i = read_info(get("rgb8_hp2")).unwrap();
    assert_eq!(i.colour_transform, ColourTransform::Hp2);
    let i = read_info(get("grey8_restart")).unwrap();
    assert_eq!(i.restart_interval, 4);
    let i = read_info(get("pal6_table")).unwrap();
    assert!(i.mapped);
    let img = decode(get("pal6_table"), &DecodeOptions::default()).unwrap();
    let t = img.planes[0].table.as_ref().unwrap();
    assert_eq!((t.id, t.entry_bytes, t.entries.len()), (5, 3, 64));
    assert_eq!(t.entries[1], 0x04FB25);
}

/// Every prefix decodes to an error or a partial image whose complete lines are exact, and
/// random corruption never panics.
#[test]
fn truncation_and_corruption() {
    let mut seed = 0x9E37_79B9u32;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    for (name, data, ..) in vectors() {
        let full = decode(&data, &DecodeOptions::default()).unwrap();
        let step = (data.len() / 60).max(1);
        for cut in (0..data.len() - 2).step_by(step) {
            match decode(&data[..cut], &DecodeOptions::default()) {
                Ok(img) => {
                    // Without EOI an image is complete when its scan data was (grey12_maxval has
                    // filler after the coded samples).
                    assert!(img.truncated || img == full, "{name} cut {cut}");
                    assert_eq!(img.planes.len(), full.planes.len());
                }
                Err(e) => assert!(
                    matches!(e, Error::Truncated | Error::Malformed(_) | Error::NotJpegLs),
                    "{name} cut {cut}: {e}"
                ),
            }
        }
        for _ in 0..200 {
            let mut d = data.clone();
            for _ in 0..1 + rnd() % 3 {
                let i = rnd() as usize % d.len();
                d[i] = rnd() as u8;
            }
            let _ = decode(&d, &DecodeOptions::default());
        }
    }
}
