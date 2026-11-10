//! The large JPEG XR corpus from tools/corpus/fetch-jxr.sh (other encoders' files and 24 MP
//! jxrlib files), checked against the manifest written by JxrDecApp. Skips without
//! SPECULAR_LARGE_CORPUS.

use std::path::PathBuf;

use specular_jxr::{DecodeOptions, Samples, decode, decode_native, read_info};

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

/// Applies an Exif orientation to `bpp`-byte pixels (JxrDecApp writes the oriented image).
fn orient(src: &[u8], w: usize, h: usize, bpp: usize, o: u8) -> (Vec<u8>, usize, usize) {
    let (ow, oh) = if o >= 5 { (h, w) } else { (w, h) };
    let mut out = vec![0; src.len()];
    for y in 0..oh {
        for x in 0..ow {
            let (sx, sy) = match o {
                2 => (w - 1 - x, y),
                3 => (w - 1 - x, h - 1 - y),
                4 => (x, h - 1 - y),
                5 => (y, x),
                6 => (y, h - 1 - x),
                7 => (w - 1 - y, h - 1 - x),
                8 => (w - 1 - y, x),
                _ => (x, y),
            };
            let s = (sy * w + sx) * bpp;
            let d = (y * ow + x) * bpp;
            out[d..d + bpp].copy_from_slice(&src[s..s + bpp]);
        }
    }
    (out, ow, oh)
}

fn corpus() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("SPECULAR_LARGE_CORPUS")?).join("jxr");
    dir.join("manifest.txt").exists().then_some(dir)
}

#[test]
fn large_corpus_matches_oracle() {
    let Some(dir) = corpus() else {
        eprintln!(
            "SPECULAR_LARGE_CORPUS/jxr/manifest.txt not found; skipping (tools/corpus/fetch-jxr.sh)"
        );
        return;
    };
    let manifest = std::fs::read_to_string(dir.join("manifest.txt")).unwrap();
    let mut bad = Vec::new();
    let mut n = 0;
    for l in manifest
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<&str> = l.split_whitespace().collect();
        let (w, h, bpp): (usize, usize, usize) = (
            f[1].parse().unwrap(),
            f[2].parse().unwrap(),
            f[3].parse().unwrap(),
        );
        let want = u64::from_str_radix(f[4], 16).unwrap();
        let data = std::fs::read(dir.join(f[0])).unwrap();
        read_info(&data).unwrap_or_else(|e| panic!("{}: {e}", f[0]));
        let native = match decode_native(&data, &DecodeOptions::default()) {
            Ok(n) => n,
            Err(e) => {
                bad.push(format!("{}: {e}", f[0]));
                continue;
            }
        };
        // JxrDecApp writes the stored orientation (the tag only goes into its TIFF).
        let got = bytes(&native.samples);
        let (gw, gh) = (native.width as usize, native.height as usize);
        if (gw, gh) != (w, h) || got.len() != w * h * bpp || fnv(&got) != want {
            bad.push(format!("{} ({gw}x{gh}, {} bytes)", f[0], got.len()));
        }
        // The display conversion accepts every file.
        decode(&data, &DecodeOptions::default()).unwrap_or_else(|e| panic!("{}: {e}", f[0]));
        n += 1;
    }
    eprintln!("{n} corpus files decoded");
    assert!(bad.is_empty(), "mismatches: {bad:?}");
}

#[test]
fn large_corpus_headers() {
    let Some(dir) = corpus() else { return };
    let info = |n: &str| read_info(&std::fs::read(dir.join(n)).unwrap()).unwrap();
    for o in 1..=8u8 {
        assert_eq!(
            info(&format!("kde/orientation{o}.jxr")).orientation,
            o,
            "orientation{o}"
        );
    }
    // The eight KDE files hold one picture stored in each orientation: applying the tag gives
    // the same upright image (lossy coding, so within a small tolerance).
    let upright = |o: u8| {
        let d = std::fs::read(dir.join(format!("kde/orientation{o}.jxr"))).unwrap();
        let n = decode_native(&d, &DecodeOptions::default()).unwrap();
        let raw = bytes(&n.samples);
        orient(&raw, n.width as usize, n.height as usize, n.channels, o)
    };
    let (want, ww, wh) = upright(1);
    for o in 2..=8u8 {
        let (got, gw, gh) = upright(o);
        assert_eq!((gw, gh), (ww, wh), "orientation{o}");
        let err = got
            .iter()
            .zip(&want)
            .map(|(&a, &b)| (a as i32 - b as i32).unsigned_abs() as u64)
            .sum::<u64>()
            / want.len() as u64;
        assert!(err <= 4, "orientation{o}: mean error {err}");
    }
    let path = dir.join("gen/photo24-tiles-alpha.jxr");
    if path.exists() {
        let i = info("gen/photo24-tiles-alpha.jxr");
        assert_eq!(i.tiles, (4, 4));
        assert!(i.has_alpha && !i.planar_alpha);
    }
}
