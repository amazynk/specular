//! Malformed input: every truncation and random mutations of one file per format/coding.
//! Must never panic; decoded sizes must match the reported header.

mod common;

use common::*;
use specular_legacy::{decode, info};

fn corpus() -> Vec<Vec<u8>> {
    let (w, h) = (21usize, 9usize);
    let idx = pattern(w * h, 1, 255);
    let pal: Vec<[u8; 3]> = (0..16u8).map(|i| [i, i * 2, i * 3]).collect();
    let vga = [[7u8, 8, 9]; 256];
    let lines8: Vec<Vec<u8>> = idx.chunks(w).map(|r| [r, &[0][..]].concat()).collect();
    let lines4: Vec<Vec<u8>> = idx
        .chunks(w)
        .map(|r| {
            let mut l = vec![0u8; 12];
            for (x, &v) in r.iter().enumerate() {
                l[x / 2] |= (v & 15) << (4 - 4 * (x % 2));
            }
            l
        })
        .collect();
    let c16: Vec<u16> = idx.iter().map(|&v| v as u16 * 257).collect();
    let raw8: Vec<u8> = lines8.concat();
    let bits: Vec<bool> = idx.iter().map(|&v| v & 1 == 1).collect();
    vec![
        pcx(w as u16, h as u16, 8, 1, 22, true, &[], &lines8, Some(&vga)),
        pcx(w as u16, h as u16, 4, 1, 12, true, &pal, &lines4, None),
        pcx(w as u16, h as u16, 8, 1, 22, false, &[], &lines8, None),
        sgi(w as u16, h as u16, 1, true, &vec![c16.iter().map(|&v| v >> 8).collect::<Vec<u16>>(); 3]),
        sgi(w as u16, h as u16, 2, true, &[c16.clone(), c16.clone()]),
        sgi(w as u16, h as u16, 2, false, std::slice::from_ref(&c16)),
        sun(w as u32, h as u32, 8, 2, &pal, &raw8),
        sun(w as u32, h as u32, 8, 1, &[], &raw8),
        xbm(w, h, &bits, false),
        xbm(w, h, &bits, true),
        br#"/* XPM */ static char *x[] = {"3 2 3 1","a c red","b c #00f","c s x c None","abc","cba"};"#
            .to_vec(),
        wbmp(w as u32, h as u32, &bits),
        farbfeld(w as u32, h as u32, &c16.repeat(4)),
    ]
}

fn check(d: &[u8]) {
    let i = info(d);
    if let Ok(img) = decode(d) {
        let i = i.expect("decode ok but info failed");
        assert_eq!((img.width, img.height), (i.width, i.height));
        assert_eq!(img.format, i.pixel_format);
        assert_eq!(
            img.data.len(),
            img.width as usize * img.height as usize * img.format.channels()
        );
    }
}

#[test]
fn corpus_decodes_cleanly() {
    for (n, f) in corpus().iter().enumerate() {
        let img = decode(f).unwrap_or_else(|e| panic!("corpus {n}: {e}"));
        assert!(!img.truncated, "corpus {n}");
    }
}

#[test]
fn every_truncation() {
    for f in corpus() {
        for cut in 0..f.len() {
            check(&f[..cut]);
        }
    }
}

#[test]
fn truncated_pixels_flagged() {
    // Binary formats with a length-independent header: a cut body decodes, flagged.
    for (n, f) in corpus().iter().enumerate() {
        if matches!(n, 0 | 8..=12) {
            continue; // VGA palette is at the end; text / no-magic formats
        }
        let img = decode(&f[..f.len() - 8]).unwrap_or_else(|e| panic!("corpus {n}: {e}"));
        assert!(img.truncated, "corpus {n}");
    }
}

#[test]
fn mutations() {
    let mut r = rng(5);
    for f in corpus() {
        for _ in 0..2000 {
            let mut m = f.clone();
            for _ in 0..1 + r() % 4 {
                let i = (r() as usize) % m.len();
                m[i] = match r() % 4 {
                    0 => 0,
                    1 => 0xFF,
                    _ => r() as u8,
                };
            }
            check(&m);
        }
    }
}
