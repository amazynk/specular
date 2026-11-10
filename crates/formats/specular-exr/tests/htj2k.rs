//! HTJ2K256 / HTJ2K32 against OpenEXR: files written by OpenEXR 3.5 with `tools/corpus/exr-htj2k.py`,
//! each next to an uncompressed twin holding the same pixels. Every channel must decode to the
//! same bit patterns (signed zeros, denormals, infinities and NaN included; UINT channels are
//! compared through the `f32` output, exact below 2^24). The small set
//! in `tests/data/htj2k` always runs; the full set (`<conformance>/exr-htj2k`, where
//! `SPECULAR_CONFORMANCE_DIR` overrides the conformance root) when present.

use std::path::{Path, PathBuf};

use specular_exr::{
    ChannelSelect, Compression, DecodeOptions, ExrError, PixelType, Pixels, SampleFormat,
};

/// The channel's samples as bit patterns: `f16` bits for HALF, `f32` bits otherwise.
fn decode_channel(file: &[u8], name: &str, half: bool) -> Result<(Vec<u32>, usize), ExrError> {
    let opts = DecodeOptions {
        channels: ChannelSelect::Slots([Some(name.into()), None, None, None]),
        format: if half {
            SampleFormat::F16
        } else {
            SampleFormat::F32
        },
        ..Default::default()
    };
    let img = specular_exr::decode(file, &opts)?;
    let v: Vec<u32> = match img.pixels {
        Pixels::F16(v) => v.chunks_exact(4).map(|p| p[0] as u32).collect(),
        Pixels::F32(v) => v.chunks_exact(4).map(|p| p[0].to_bits()).collect(),
    };
    Ok((v, img.damaged_chunks))
}

/// Compares every channel of `<stem>.ht.exr` with `<stem>.raw.exr`; returns the number
/// of samples compared and the mismatches.
fn compare(dir: &Path, stem: &str) -> (usize, usize) {
    let ht = std::fs::read(dir.join(format!("{stem}.ht.exr"))).unwrap();
    let raw = std::fs::read(dir.join(format!("{stem}.raw.exr"))).unwrap();
    let info = specular_exr::read_info(&ht).unwrap();
    let part = &info.parts[0];
    assert!(
        matches!(
            part.compression,
            Compression::Htj2k256 | Compression::Htj2k32
        ),
        "{stem}"
    );
    let (mut n, mut bad) = (0, 0);
    for ch in &part.channels {
        let half = ch.pixel_type == PixelType::Half;
        let (a, damaged) = decode_channel(&ht, &ch.name, half).unwrap();
        let (b, _) = decode_channel(&raw, &ch.name, half).unwrap();
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
            name.strip_suffix(".ht.exr").map(str::to_owned)
        })
        .collect();
    v.sort();
    v
}

#[test]
fn small_set_matches_openexr() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/htj2k");
    let stems = stems(&dir);
    assert_eq!(stems.len(), 5);
    for stem in stems {
        let (n, bad) = compare(&dir, &stem);
        assert!(n > 0, "{stem}");
        assert_eq!(bad, 0, "{stem}: {bad} of {n} samples differ");
    }
}

#[test]
fn float_and_uint_channels_are_exact() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/htj2k");
    let ht = std::fs::read(dir.join("float_uint_32.ht.exr")).unwrap();
    let (z, damaged) = decode_channel(&ht, "Z", false).unwrap();
    assert_eq!(damaged, 0);
    // The first row holds 0, -0, the smallest denormals, infinities, NaN and the largest float.
    assert_eq!(z[0], 0);
    assert_eq!(z[1], 0x8000_0000);
    assert_eq!((z[2], z[3]), (1, 0x8000_0001));
    assert_eq!((z[4], z[5]), (0x7F80_0000, 0xFF80_0000));
    assert!(f32::from_bits(z[6]).is_nan());
    assert_eq!(f32::from_bits(z[7]), 3.4e38);
    let (id, _) = decode_channel(&ht, "id", false).unwrap();
    for (i, v) in id.iter().enumerate() {
        assert_eq!(
            f32::from_bits(*v),
            (i as u64 * 7919 % 100003) as f32,
            "id {i}"
        );
    }
}

#[test]
fn corrupt_chunks_are_damaged_not_panics() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/htj2k");
    let ht = std::fs::read(dir.join("rgba_half_256.ht.exr")).unwrap();
    let start = ht.windows(2).position(|w| w == b"HT").unwrap();
    for (k, i) in (start..ht.len()).step_by(7).enumerate() {
        let mut f = ht.clone();
        f[i] ^= 1 << (k % 8);
        let _ = specular_exr::decode(&f, &DecodeOptions::default());
        f.truncate(i);
        let _ = specular_exr::decode(&f, &DecodeOptions::default());
    }
    // A wrong magic is a damaged chunk.
    let mut f = ht.clone();
    f[start] = b'X';
    let img = specular_exr::decode(&f, &DecodeOptions::default()).unwrap();
    assert_eq!(img.damaged_chunks, 1);
}

fn corpus() -> Option<PathBuf> {
    let root = std::env::var_os("SPECULAR_CONFORMANCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/conformance")
        });
    let dir = root.join("exr-htj2k");
    dir.is_dir().then_some(dir)
}

#[test]
fn full_set_matches_openexr() {
    let Some(dir) = corpus() else { return };
    let (mut n, mut bad, mut files) = (0, 0, 0);
    for stem in stems(&dir) {
        let (a, b) = compare(&dir, &stem);
        n += a;
        bad += b;
        files += 1;
    }
    eprintln!("exr-htj2k: {files} files, {n} samples, {bad} mismatches");
    assert_eq!(bad, 0);
}
