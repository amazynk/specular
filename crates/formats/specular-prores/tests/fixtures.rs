//! Every fixture frame (tests/data, made by gen_fixtures.sh with FFmpeg's encoders) against
//! FFmpeg's decode: sample for sample, alpha included, serial and parallel alike.

use std::path::Path;

use specular_prores::{Alpha, Chroma, Interlace, Options};

fn read(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name),
    )
    .unwrap()
}

/// The frames of a `.frames` file (each starts with its 32-bit size).
fn split(data: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 8 <= data.len() {
        let n = u32::from_be_bytes(data[at..at + 4].try_into().unwrap()) as usize;
        out.push(&data[at..at + n]);
        at += n;
    }
    out
}

fn check(name: &str) -> specular_prores::Header {
    let data = read(&format!("{name}.frames"));
    let want = read(&format!("{name}.yuv"));
    let mut at = 0;
    let mut header = None;
    for (i, f) in split(&data).into_iter().enumerate() {
        for parallel in [false, true] {
            let o = Options {
                parallel,
                ..Options::default()
            };
            let d = specular_prores::decode(f, &o).unwrap();
            assert_eq!(d.damaged_slices, 0, "{name} frame {i}");
            let mut got = Vec::new();
            for p in d.planes.iter().chain(d.alpha.iter()) {
                got.extend_from_slice(p);
            }
            assert!(
                want.get(at..at + got.len()) == Some(&got[..]),
                "{name}: frame {i} differs from FFmpeg (parallel {parallel})"
            );
            header = Some(d.header);
            if parallel {
                at += got.len();
            }
        }
    }
    assert_eq!(at, want.len(), "{name}: frame count");
    header.unwrap()
}

#[test]
fn profiles_422() {
    for name in ["proxy", "lt", "std", "hq", "odd", "aw422"] {
        let h = check(name);
        assert_eq!(
            (h.chroma, h.bit_depth(), h.alpha),
            (Chroma::Yuv422, 10, Alpha::None)
        );
    }
}

#[test]
fn interlaced_both_field_orders() {
    assert_eq!(check("il-tff").interlace, Interlace::TopFirst);
    assert_eq!(check("il-bff").interlace, Interlace::BottomFirst);
}

#[test]
fn profiles_4444_and_alpha() {
    for name in ["p4444", "xq", "aw4444"] {
        let h = check(name);
        assert_eq!(
            (h.chroma, h.bit_depth(), h.alpha),
            (Chroma::Yuv444, 12, Alpha::None)
        );
    }
    assert_eq!(check("alpha8").alpha, Alpha::Bits8);
    assert_eq!(check("alpha16").alpha, Alpha::Bits16);
}

/// Damaged frames decode what they can and never panic: truncations and flipped bytes.
#[test]
fn damage_is_contained() {
    let data = read("hq.frames");
    let f = split(&data)[0].to_vec();
    let o = Options::default();
    for cut in [9, 30, 200, f.len() / 2, f.len() - 1] {
        let _ = specular_prores::decode(&f[..cut], &o);
    }
    let mut seed = 7u32;
    for _ in 0..300 {
        let mut g = f.clone();
        for _ in 0..4 {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let i = (seed >> 8) as usize % g.len();
            g[i] ^= (seed >> 3) as u8 | 1;
        }
        if let Ok(d) = specular_prores::decode(&g, &o) {
            assert_eq!(d.planes[0].len(), d.width as usize * d.height as usize * 2);
        }
    }
}
