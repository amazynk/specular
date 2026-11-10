//! Truncation and corruption loops over oracle-written files: decoding must never panic, and
//! any image returned must have a consistent size.

mod common;

use common::*;
use exr::prelude::*;
use specular_exr as sx;

fn check(bytes: &[u8]) {
    for format in [sx::SampleFormat::F32, sx::SampleFormat::F16] {
        let o = sx::DecodeOptions {
            format,
            ..Default::default()
        };
        if let Ok(img) = sx::decode(bytes, &o) {
            let n = img.width as usize * img.height as usize * 4;
            match &img.pixels {
                sx::Pixels::F32(v) => assert_eq!(v.len(), n),
                sx::Pixels::F16(v) => assert_eq!(v.len(), n),
            }
        }
    }
    if let Ok(info) = sx::read_info(bytes) {
        for (pi, p) in info.parts.iter().enumerate().take(4) {
            for li in [1, p.levels.len().saturating_sub(1)] {
                let o = sx::DecodeOptions {
                    part: pi,
                    level: sx::LevelSelect::Index(li),
                    parallel: false,
                    ..Default::default()
                };
                let _ = sx::decode_with_info(bytes, &info, &o);
            }
        }
    }
}

fn hammer(bytes: &[u8], seed: u64, mutations: usize) {
    // Truncations: every byte in the header region, then evenly spaced.
    let header_end = bytes.len().min(600);
    for n in 0..header_end {
        check(&bytes[..n]);
    }
    let step = (bytes.len() / 300).max(1);
    for n in (header_end..bytes.len()).step_by(step) {
        check(&bytes[..n]);
    }
    let mut rng = Rng(seed | 1);
    for i in 0..mutations {
        let mut b = bytes.to_vec();
        for _ in 0..1 + rng.next() % 6 {
            // Half of the mutations hit the header and offset tables.
            let range = if i % 2 == 0 { header_end } else { b.len() };
            let at = (rng.next() % range as u64) as usize;
            b[at] = match rng.next() % 4 {
                0 => 0,
                1 => 0xff,
                2 => b[at] ^ (1 << (rng.next() % 8)),
                _ => rng.next() as u8,
            };
        }
        check(&b);
    }
}

fn layer(
    c: Compression,
    tiles: Option<(usize, usize)>,
    w: usize,
    h: usize,
) -> Layer<AnyChannels<FlatSamples>> {
    let list = vec![
        channel("R", Ty::H, w, h, 1, true),
        channel("G", Ty::F, w, h, 2, true),
        channel("B", Ty::U, w, h, 3, false),
        channel("A", Ty::H, w, h, 4, false),
    ];
    Layer::new(
        (w, h),
        LayerAttributes::named(""),
        encoding(c, tiles, LineOrder::Increasing),
        chans(list),
    )
}

#[test]
fn scanline_files() {
    for (i, c) in [
        Compression::Uncompressed,
        Compression::RLE,
        Compression::ZIP1,
        Compression::ZIP16,
        Compression::PIZ,
        Compression::PXR24,
        Compression::B44,
        Compression::B44A,
    ]
    .into_iter()
    .enumerate()
    {
        hammer(&write_one(layer(c, None, 48, 40)), 100 + i as u64, 400);
    }
}

#[test]
fn tiled_and_multipart_files() {
    for (i, c) in [
        Compression::ZIP16,
        Compression::PIZ,
        Compression::B44A,
        Compression::RLE,
    ]
    .into_iter()
    .enumerate()
    {
        hammer(
            &write_one(layer(c, Some((16, 16)), 40, 33)),
            200 + i as u64,
            300,
        );
    }
    let mut a = layer(Compression::PIZ, None, 20, 20);
    a.attributes.layer_name = Some("a".into());
    let mut b = layer(Compression::ZIP1, Some((8, 8)), 17, 9);
    b.attributes.layer_name = Some("b".into());
    let attrs = ImageAttributes::new(IntegerBounds::from_dimensions((20, 20)));
    hammer(
        &write_exr!(Image::empty(attrs).with_layer(a).with_layer(b)),
        300,
        400,
    );
}

#[test]
fn garbage_and_header_edge_cases() {
    assert!(!sx::probe(b""));
    assert!(sx::read_info(b"").is_err());
    assert!(sx::read_info(&[0x76, 0x2f, 0x31, 0x01]).is_err());
    assert!(sx::read_info(&[0x76, 0x2f, 0x31, 0x01, 2, 0, 0, 0, 0]).is_err());
    let mut rng = Rng(42);
    for len in [8usize, 64, 1000] {
        for _ in 0..200 {
            let mut b: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
            b[..4].copy_from_slice(&[0x76, 0x2f, 0x31, 0x01]);
            b[4] = 2;
            check(&b);
        }
    }
    // A header promising a huge image over a tiny body is refused before allocating.
    let bytes = write_one(layer(Compression::ZIP16, None, 8, 8));
    let pos = bytes.windows(10).position(|w| w == b"dataWindow").unwrap();
    let mut huge = bytes.clone();
    // name\0 "box2i\0" size(4) then xMin yMin xMax yMax.
    let v = pos + 11 + 6 + 4;
    huge[v + 8..v + 12].copy_from_slice(&200_000i32.to_le_bytes());
    huge[v + 12..v + 16].copy_from_slice(&200_000i32.to_le_bytes());
    match sx::decode(&huge, &Default::default()) {
        Err(sx::ExrError::TooLarge { .. }) | Err(sx::ExrError::InvalidHeader(_)) => {}
        other => panic!(
            "expected refusal, got {:?}",
            other.map(|i| (i.width, i.height))
        ),
    }
}

#[test]
fn dwa_files() {
    for (i, c) in [common::dwa::DWAA, common::dwa::DWAB]
        .into_iter()
        .enumerate()
    {
        let f = common::dwa::sample_file(c);
        assert_eq!(
            sx::decode(&f, &Default::default()).unwrap().damaged_chunks,
            0
        );
        hammer(&f, 400 + i as u64, 600);
    }
}

/// DWA and deep files of the OpenEXR-written corpus (`tools/corpus/exr-dwa.sh`), when present.
#[test]
fn corpus_dwa_and_deep_files() {
    let root = std::env::var_os("SPECULAR_CONFORMANCE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tools/corpus/conformance")
        })
        .join("exr-dwa");
    if !root.is_dir() {
        eprintln!("{} missing; skipped", root.display());
        return;
    }
    for (i, name) in [
        "dwa/syn_mixed.dwaa.l45.exr",
        "dwa/syn_tiled.dwab.l45.exr",
        "dwa/syn_sub.dwaa.l45.exr",
        "deep/syn_scan_zips.exr",
        "deep/syn_tile_rle.exr",
        "deep/syn_scan_none.exr",
    ]
    .into_iter()
    .enumerate()
    {
        let f = std::fs::read(root.join(name)).unwrap();
        hammer(&f, 500 + i as u64, 300);
    }
}

#[test]
fn missing_chunks_are_counted() {
    let mut bytes = write_one(layer(Compression::ZIP16, None, 32, 64));
    let n = bytes.len();
    bytes.truncate(n - n / 3);
    let img = sx::decode(&bytes, &Default::default()).expect("partial decode");
    assert!(img.damaged_chunks > 0);
    assert_eq!(img.height, 64);
}
