//! Basis Universal HDR files written by basisu (BinomialLLC/basis_universal 99f52d63, built as
//! in tools/corpus/texture/basis.sh) from a 37×29 Radiance picture with values up to ~40:
//!
//! * `hdr4` — UASTC HDR 4×4 (`-hdr_4x4`), `hdr6` — ASTC HDR 6×6 (`-hdr_6x6`), each as KTX2
//!   (`-ktx2`, Zstandard) and `.basis` (`-basis`);
//! * `hdr6i` — UASTC HDR 6×6 intermediate (`-hdr_6x6i`), transcoded to ASTC HDR 6×6 (its
//!   blocks are checked bit for bit in `basis/hdr6x6i.rs`);
//! * `<name>.rgbah` — astcenc's `-dh` decode of the KTX2 file's blocks (copied into a
//!   `.astc` file), as raw RGBA half floats (alpha 1.0). (The Basis transcoder's own
//!   RGBA_HALF output at this commit is one colour per block, so it is not the reference.)
//!
//! The half-float bits are compared exactly in `ktx2.rs`'s unit test; here the public decode
//! (8-bit sRGB of linear light, clamped) of both containers is checked against the reference
//! put through the same conversion.

use specular_texture::{DecodeOptions, Texture};

fn data(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn half_to_f32(h: u16) -> f32 {
    let e = i32::from((h >> 10) & 0x1f);
    let m = f32::from(h & 0x3ff);
    let v = match e {
        0 => m * 2f32.powi(-24),
        31 => f32::INFINITY,
        _ => (1.0 + m / 1024.0) * 2f32.powi(e - 15),
    };
    if h & 0x8000 != 0 { -v } else { v }
}

fn srgb8(v: f32) -> u8 {
    let s = if v.is_nan() || v <= 0.0 {
        0.0
    } else if v >= 1.0 {
        1.0
    } else if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0 + 0.5) as u8
}

#[test]
fn uastc_hdr_and_astc_hdr_6x6_in_ktx2_and_basis() {
    for name in ["hdr4", "hdr6"] {
        let refs = data(&format!("{name}.rgbah"));
        let want: Vec<u8> = refs
            .chunks_exact(8)
            .flat_map(|p| {
                let c = |i: usize| srgb8(half_to_f32(u16::from_le_bytes([p[2 * i], p[2 * i + 1]])));
                [c(0), c(1), c(2), 255]
            })
            .collect();
        for ext in ["ktx2", "basis"] {
            let d = data(&format!("{name}.{ext}"));
            let t = Texture::open(&d).unwrap_or_else(|e| panic!("{name}.{ext}: {e}"));
            assert_eq!((t.width, t.height), (37, 29), "{name}.{ext}");
            let out = t.decode(&d, &DecodeOptions::default()).unwrap();
            assert!(!out.truncated, "{name}.{ext}");
            let mut worst = 0;
            for (a, b) in out.data.iter().zip(&want) {
                worst = worst.max(a.abs_diff(*b));
            }
            assert!(worst <= 1, "{name}.{ext}: max difference {worst}");
        }
    }
}

#[test]
fn uastc_hdr_6x6_intermediate_decodes() {
    for ext in ["ktx2", "basis"] {
        let d = data(&format!("hdr6i.{ext}"));
        let t = Texture::open(&d).unwrap_or_else(|e| panic!("hdr6i.{ext}: {e}"));
        assert_eq!((t.width, t.height), (37, 29), "hdr6i.{ext}");
        assert!(
            t.description().contains("6×6 intermediate"),
            "{}",
            t.description()
        );
        let out = t.decode(&d, &DecodeOptions::default()).unwrap();
        assert!(!out.truncated, "hdr6i.{ext}");
        assert_eq!(out.data.len(), 37 * 29 * 4);
        // A damaged stream is refused, not drawn from garbage.
        let mut bad = d.clone();
        let n = bad.len();
        bad[n - 2] ^= 0xff;
        assert!(
            t.decode(&bad, &DecodeOptions::default()).is_err(),
            "hdr6i.{ext}"
        );
    }
}
