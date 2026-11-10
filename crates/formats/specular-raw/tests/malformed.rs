//! Truncated and corrupted files must never panic (and never allocate unboundedly).

mod common;

use common::{
    ArwSpec, DngCodec, DngSpec, demo_profile, samples_dir, test_pattern, write_arw, write_dng,
};
use specular_raw::{DecodeOptions, OutputSize};

/// Parse and, if possible, decode everything; errors are fine, panics are not.
fn exercise(file: &[u8], decode: bool) {
    let _ = specular_raw::detect(file);
    let Ok(raw) = specular_raw::parse(file) else {
        return;
    };
    let info = raw.info();
    for p in &info.previews {
        assert!(p.range.end <= file.len(), "preview range out of bounds");
    }
    if !decode {
        return;
    }
    if let Some(p) = info.thumbnail() {
        let _ = specular_raw::decode_preview(file, p, 0);
    }
    let opts = DecodeOptions {
        size: OutputSize::Half,
        max_pixels: 1 << 22,
        ..Default::default()
    };
    if let Ok(img) = raw.decode_raw(file, &opts) {
        let _ = specular_raw::develop(&img, &opts);
        let _ = specular_raw::develop(
            &img,
            &DecodeOptions {
                size: OutputSize::Full,
                ..opts
            },
        );
    }
}

fn synthetic() -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for codec in [
        DngCodec::U16,
        DngCodec::Packed(12),
        DngCodec::Lj92Strip,
        DngCodec::Lj92Tiles(16),
    ] {
        let mut s = DngSpec::new(36, 20, codec);
        if codec != DngCodec::U16 {
            s.bits = 12;
            s.white = 4095;
        }
        s.active_area = Some([0, 2, 20, 34]);
        out.push(write_dng(&s, &test_pattern(36, 20, 1, s.bits, 5)));
    }
    // A camera profile with look tables: corrupted table tags must not panic either.
    let mut s = DngSpec::new(24, 16, DngCodec::U16);
    s.extra = demo_profile();
    out.push(write_dng(&s, &test_pattern(24, 16, 1, 16, 6)));
    // Sony lossless ARW: quad tiles with a partial edge, and the general (restart) path.
    for restart in [0, 3] {
        let mut s = ArwSpec::new(36, 20, 16);
        s.restart = restart;
        out.push(write_arw(&s, &test_pattern(36, 20, 1, 14, 8)));
    }
    out
}

#[test]
fn synthetic_truncation() {
    for f in synthetic() {
        for len in 0..f.len() {
            exercise(&f[..len], true);
        }
    }
}

#[test]
fn synthetic_bit_flips() {
    let mut s = 7u32;
    for f in synthetic() {
        for _ in 0..3000 {
            let mut m = f.clone();
            for _ in 0..1 + s % 4 {
                s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
                let i = (s >> 4) as usize % m.len();
                m[i] = match s % 3 {
                    0 => m[i] ^ (1 << ((s >> 20) % 8)),
                    1 => 0xff,
                    _ => (s >> 24) as u8,
                };
            }
            exercise(&m, true);
        }
    }
}

#[test]
fn garbage_and_magic_only() {
    for f in [
        &b""[..],
        b"II*\0",
        b"MM\0*",
        b"II*\0\x08\0\0\0",
        b"FUJIFILMCCD-RAW ",
        b"\0\0\0\x18ftypcrx ",
        b"IIRO\x08\0\0\0",
        b"IIU\0\x08\0\0\0",
    ] {
        exercise(f, true);
    }
}

/// Real samples: truncations at many points (header parse only for most of them, full decode for
/// a few) and header corruption. Skipped when the sample directory is absent.
#[test]
fn samples_truncation_and_corruption() {
    let Some(dir) = samples_dir() else {
        eprintln!("raw samples not present; skipping");
        return;
    };
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    paths.sort();
    for p in paths {
        let data = std::fs::read(&p).unwrap();
        // Header region: every byte boundary in the first 4 KiB is too slow; use a stride.
        for len in (0..data.len().min(65536)).step_by(97) {
            exercise(&data[..len], false);
        }
        for k in 1..8 {
            exercise(&data[..data.len() * k / 8], k % 3 == 0);
        }
        let mut s = 99u32;
        for _ in 0..300 {
            let mut m = data[..data.len().min(1 << 20)].to_vec();
            s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let i = (s >> 4) as usize % m.len().min(65536);
            m[i] ^= 1 << (s % 8);
            exercise(&m, false);
        }
        eprintln!("{}: ok", p.display());
    }
}
