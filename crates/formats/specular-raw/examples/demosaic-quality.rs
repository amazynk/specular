//! Demosaic quality check for non-Bayer patterns: mosaics a full-RGB reference image with the
//! X-Trans pattern, develops it (identity colour, unit white balance) and writes the result.
//! Usage:
//!
//! ```text
//! cargo run -p specular-raw --example demosaic-quality -- REFERENCE OUTPUT
//! ```
//!
//! Both files are `u32 LE width, u32 LE height`, then `width * height * 3` u16 LE linear RGB.
//! `tools/quality/raw_variants_oracle.py xtrans` makes the reference, runs this and LibRaw on
//! the same mosaic (as a DNG), and reports PSNR against the reference.

use std::path::Path;

use specular_raw::{Cfa, DecodeOptions, MatrixSource, RawImage, Rect};

/// Fujifilm X-Trans (X-T2 and most X-Trans bodies), rows top to bottom.
const XTRANS: [&str; 6] = ["GGRGGB", "GGBGGR", "BRGRBG", "GGBGGR", "GGRGGB", "RBGBRG"];

fn read(path: &Path) -> (u32, u32, Vec<u16>) {
    let b = std::fs::read(path).expect("read reference");
    let w = u32::from_le_bytes(b[0..4].try_into().expect("header"));
    let h = u32::from_le_bytes(b[4..8].try_into().expect("header"));
    let px = b[8..]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect::<Vec<_>>();
    assert_eq!(px.len(), (w * h * 3) as usize, "reference size");
    (w, h, px)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [reference, output] = args.as_slice() else {
        eprintln!("usage: demosaic-quality REFERENCE OUTPUT");
        std::process::exit(2);
    };
    let (w, h, rgb) = read(Path::new(reference));
    let colors: Vec<u8> = XTRANS
        .iter()
        .flat_map(|r| r.bytes())
        .map(|c| match c {
            b'R' => 0,
            b'G' => 1,
            _ => 2,
        })
        .collect();
    let cfa = Cfa::new(6, 6, colors).expect("pattern");
    let mosaic: Vec<u16> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (y, x)))
        .map(|(y, x)| rgb[((y * w + x) * 3) as usize + cfa.color(y, x) as usize])
        .collect();
    let full = Rect { x: 0, y: 0, w, h };
    let raw = RawImage {
        width: w,
        height: h,
        cpp: 1,
        data: mosaic,
        window: full,
        cfa: Some(cfa),
        crop: full,
        black: [0.0; 4],
        white: 65535.0,
        linearization: None,
        wb: Some([1.0; 3]),
        rgb_cam: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        matrix_source: MatrixSource::Identity,
        profile: None,
        orientation: 1,
        truncated: false,
        opcodes: None,
    };
    let t = std::time::Instant::now();
    let img = specular_raw::develop(&raw, &DecodeOptions::default()).expect("develop");
    eprintln!(
        "develop {}x{}: {:.1} ms",
        w,
        h,
        t.elapsed().as_secs_f64() * 1e3
    );
    let mut out = Vec::with_capacity(8 + img.data.len() * 2);
    out.extend_from_slice(&img.width.to_le_bytes());
    out.extend_from_slice(&img.height.to_le_bytes());
    for v in &img.data {
        out.extend_from_slice(&v.to_le_bytes());
    }
    std::fs::write(output, out).expect("write output");
}
