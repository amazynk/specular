//! ZSTD (compression 13) against OpenEXR: files written by OpenEXR 3.5 with
//! `tools/corpus/exr-zstd.py`, each next to an uncompressed twin holding the same pixels. Every
//! channel must decode to the same bit patterns (flat and deep, scanline and tiled; deep
//! files are compared after flattening). The small set in `tests/data/zstd` always runs; the
//! full set (`<conformance>/exr-zstd`, `SPECULAR_CONFORMANCE_DIR` overriding the root) when
//! present.

use std::path::{Path, PathBuf};

use specular_exr::{ChannelSelect, Compression, DecodeOptions, PixelType, Pixels, SampleFormat};

/// The channel's samples as bit patterns: `f16` bits for HALF, `f32` bits otherwise.
fn decode_channel(file: &[u8], name: &str, half: bool) -> (Vec<u32>, usize) {
    let opts = DecodeOptions {
        channels: ChannelSelect::Slots([Some(name.into()), None, None, None]),
        format: if half {
            SampleFormat::F16
        } else {
            SampleFormat::F32
        },
        ..Default::default()
    };
    let img = specular_exr::decode(file, &opts).unwrap_or_else(|e| panic!("{name}: {e}"));
    let v: Vec<u32> = match img.pixels {
        Pixels::F16(v) => v.chunks_exact(4).map(|p| p[0] as u32).collect(),
        Pixels::F32(v) => v.chunks_exact(4).map(|p| p[0].to_bits()).collect(),
    };
    (v, img.damaged_chunks)
}

/// Compares every channel of `<stem>.zstd.exr` with `<stem>.raw.exr`: (samples, mismatches).
fn compare(dir: &Path, stem: &str) -> (usize, usize) {
    let z = std::fs::read(dir.join(format!("{stem}.zstd.exr"))).unwrap();
    let raw = std::fs::read(dir.join(format!("{stem}.raw.exr"))).unwrap();
    let info = specular_exr::read_info(&z).unwrap();
    let part = &info.parts[0];
    assert_eq!(part.compression, Compression::Zstd, "{stem}");
    let (mut n, mut bad) = (0, 0);
    for ch in &part.channels {
        let half = ch.pixel_type == PixelType::Half;
        let (a, damaged) = decode_channel(&z, &ch.name, half);
        let (b, _) = decode_channel(&raw, &ch.name, half);
        assert_eq!(damaged, 0, "{stem} {}", ch.name);
        assert_eq!(a.len(), b.len());
        n += a.len();
        bad += a.iter().zip(&b).filter(|(x, y)| x != y).count();
    }
    (n, bad)
}

fn stems(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| {
            let name = e.ok()?.file_name().into_string().ok()?;
            name.strip_suffix(".zstd.exr").map(str::to_owned)
        })
        .collect();
    v.sort();
    v
}

fn small() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/zstd")
}

#[test]
fn small_set_matches_openexr() {
    let stems = stems(&small());
    assert_eq!(stems.len(), 5);
    for stem in stems {
        let (n, bad) = compare(&small(), &stem);
        assert!(n > 0, "{stem}");
        assert_eq!(bad, 0, "{stem}: {bad} of {n} samples differ");
    }
}

#[test]
fn specials_survive() {
    let f = std::fs::read(small().join("rgba_half_z_float.zstd.exr")).unwrap();
    let (z, _) = decode_channel(&f, "Z", false);
    assert_eq!(z[0], 0);
    assert_eq!(z[1], 0x8000_0000);
    assert_eq!((z[4], z[5]), (0x7F80_0000, 0xFF80_0000));
    assert!(f32::from_bits(z[6]).is_nan());
}

#[test]
fn corrupt_chunks_are_damaged_not_panics() {
    for name in ["rgba_half_z_float", "deep_scanline"] {
        let z = std::fs::read(small().join(format!("{name}.zstd.exr"))).unwrap();
        let start = z.windows(8).position(|w| w == b"zstd-exr").unwrap();
        for (k, i) in (start..z.len()).step_by(5).enumerate() {
            let mut f = z.clone();
            f[i] ^= 1 << (k % 8);
            let _ = specular_exr::decode(&f, &DecodeOptions::default());
            f.truncate(i);
            let _ = specular_exr::decode(&f, &DecodeOptions::default());
        }
        // A wrong wire format is a damaged chunk.
        let mut f = z.clone();
        f[start + 8] = 9;
        let img = specular_exr::decode(&f, &DecodeOptions::default()).unwrap();
        assert_eq!(img.damaged_chunks, 1, "{name}");
    }
}

#[test]
fn full_set_matches_openexr() {
    let root = std::env::var_os("SPECULAR_CONFORMANCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/conformance")
        });
    let dir = root.join("exr-zstd");
    if !dir.is_dir() {
        eprintln!("no {}; skipped", dir.display());
        return;
    }
    let (mut n, mut bad, mut files) = (0, 0, 0);
    for stem in stems(&dir) {
        let (a, b) = compare(&dir, &stem);
        eprintln!("{stem}: {b} of {a} samples differ");
        n += a;
        bad += b;
        files += 1;
    }
    eprintln!("exr-zstd: {files} files, {bad} of {n} samples differ");
    assert_eq!(bad, 0);
}
