//! Robustness of the compressed sensor codecs (Fujifilm RAF, Canon CRX) against corrupted and
//! truncated streams: errors or zero-filled output are fine, panics are not. Uses real samples
//! (never committed; skipped when absent).

mod common;

use std::path::{Path, PathBuf};

use specular_raw::{DecodeOptions, OutputSize};

fn decode_all(file: &[u8]) {
    let Ok(raw) = specular_raw::parse(file) else {
        return;
    };
    let opts = DecodeOptions {
        max_pixels: 1 << 26,
        ..Default::default()
    };
    if let Ok(img) = raw.decode_raw(file, &opts) {
        assert!(img.data.len() >= (img.width * img.height * img.cpp) as usize);
        for size in [OutputSize::Half, OutputSize::Full] {
            let _ = specular_raw::develop(&img, &DecodeOptions { size, ..opts });
        }
    }
}

fn lcg(s: &mut u32) -> u32 {
    *s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
    *s >> 1
}

fn corpus_raws(ext: &str) -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut out = Vec::new();
    let mut stack = vec![root.join("../../../tools/corpus/large/raw")];
    if let Some(d) = common::samples_dir() {
        stack.push(d);
    }
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case(ext))
            {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Offset of the Fujifilm compressed-stream header inside a RAF, if the file uses one.
fn fuji_header(d: &[u8]) -> Option<usize> {
    let be32 = |o: usize| -> Option<usize> {
        Some(u32::from_be_bytes(d.get(o..o + 4)?.try_into().ok()?) as usize)
    };
    let start = be32(100)?;
    let end = (start + be32(104)?).min(d.len());
    (start..end.saturating_sub(16)).find(|&i| {
        d[i] == 0x49
            && d[i + 1] == 0x53
            && d[i + 2] <= 1
            && matches!(d[i + 3], 0 | 16)
            && (12..=16).contains(&d[i + 4])
            && d[i + 11..i + 13] == [0x03, 0x00]
    })
}

fn small_raf(path: &Path) -> Option<Vec<u8>> {
    let mut d = std::fs::read(path).ok()?;
    let h = fuji_header(&d)?;
    // Lossless streams only (header byte 2 = 1): shortened lossy streams decode as truncated
    // (X-T4, X-H2, X-E4 lossy and others in the corpus; their full files decode completely).
    if d[h + 2] != 1 {
        return None;
    }
    // Keep 4 line groups (24 rows): every block stream is still read from its start.
    d[h + 5..h + 7].copy_from_slice(&24u16.to_be_bytes());
    d[h + 14..h + 16].copy_from_slice(&4u16.to_be_bytes());
    Some(d)
}

#[test]
fn raf_compressed_mutation_and_truncation() {
    let files = corpus_raws("raf");
    let mut tested = 0;
    for p in files {
        let Some(d) = small_raf(&p) else {
            continue;
        };
        let raw = specular_raw::parse(&d).unwrap();
        let img = raw
            .decode_raw(&d, &DecodeOptions::default())
            .unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        assert!(!img.truncated, "{}", p.display());
        let h = fuji_header(&d).unwrap();
        let mut s = 7u32;
        for _ in 0..40 {
            let mut m = d.clone();
            // Corrupt the block-size table and the first bytes of the streams.
            for _ in 0..4 {
                let i = h + 16 + lcg(&mut s) as usize % 200_000;
                if i < m.len() {
                    m[i] ^= 1 << (lcg(&mut s) % 8);
                }
            }
            decode_all(&m);
        }
        for k in [h + 8, h + 16, h + 40, h + 1000, h + 100_000] {
            decode_all(&d[..k.min(d.len())]);
        }
        tested += 1;
        eprintln!("{}: ok", p.display());
    }
    if tested == 0 {
        eprintln!("no compressed RAF samples; skipping");
    }
}

/// Every unmodified corpus CR3 decodes without error. Tiles with right / bottom neighbours
/// borrow coefficients by the 3-level extension table; a wrong row there (EOS 250D, tile
/// width & 7 = 4) shows up as "CRX: bad run length".
#[test]
fn cr3_corpus_decodes() {
    let opts = DecodeOptions {
        max_pixels: 1 << 26,
        ..Default::default()
    };
    for p in corpus_raws("cr3") {
        let d = std::fs::read(&p).unwrap();
        let raw = specular_raw::parse(&d).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        if let Err(e) = raw.decode_raw(&d, &opts) {
            panic!("{}: {e}", p.display());
        }
        eprintln!("{}: ok", p.display());
    }
}

#[test]
fn cr3_mutation() {
    let files = corpus_raws("cr3");
    for p in files {
        let d = std::fs::read(&p).unwrap();
        // Only small files: a debug-build decode of a 45 MP CRX image takes seconds.
        if d.len() > 8 << 20 {
            continue;
        }
        let mut s = 11u32;
        for _ in 0..12 {
            let mut m = d.clone();
            for _ in 0..8 {
                let i = 65536 + lcg(&mut s) as usize % (m.len() - 65536);
                m[i] ^= 1 << (lcg(&mut s) % 8);
            }
            decode_all(&m);
        }
        eprintln!("{}: ok", p.display());
    }
}

/// Olympus compressed ORF and Panasonic RW2 (formats 5/6): bit flips in the sensor data and
/// truncation at a few points.
#[test]
fn orf_rw2_mutation_and_truncation() {
    let mut files = corpus_raws("orf");
    files.extend(corpus_raws("rw2"));
    for p in files {
        let d = std::fs::read(&p).unwrap();
        if d.len() > 40 << 20 || d.len() < 1 << 20 {
            continue;
        }
        let mut s = 13u32;
        for _ in 0..4 {
            let mut m = d.clone();
            for _ in 0..16 {
                let i = (1 << 20) + lcg(&mut s) as usize % (m.len() - (1 << 20));
                m[i] ^= 1 << (lcg(&mut s) % 8);
            }
            decode_all(&m);
        }
        for k in [d.len() / 3, d.len() - 1000] {
            decode_all(&d[..k]);
        }
        eprintln!("{}: ok", p.display());
    }
}
