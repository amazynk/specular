//! Committed fixtures against NBIS's decoding (tools/corpus/fetch-raster-wsq.sh made them).

mod common;

use common::*;
use specular_wsq::{Error, Model, Pixels, decode, info, probe};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/");

fn read(name: &str) -> Vec<u8> {
    std::fs::read(format!("{DIR}{name}")).unwrap()
}

fn gray(img: &specular_wsq::Image) -> &[u8] {
    match &img.pixels {
        Pixels::Gray8(p) => p,
        _ => panic!("not grey"),
    }
}

#[test]
fn matches_nbis_exactly() {
    for name in [
        "gen-fp-biomsharp-fp01-77x100-r4.0",
        "gen-wizard-77x100-r0.3",
        "gen-rose-70x46-r0.75",
        "gen-granite-64x64-r4.0",
    ] {
        let data = read(&format!("{name}.wsq"));
        assert!(probe(&data[..64]), "{name}");
        let (w, h, want) = pgm(&read(&format!("{name}.pgm")));
        let img = decode(&data, 0).unwrap();
        assert_eq!((img.width, img.height), (w, h), "{name}");
        assert!(!img.truncated, "{name}");
        assert_eq!(diff(gray(&img), &want), (0, 0), "{name}");
    }
}

/// A real 500 ppi fingerprint (jnbis's sample), whose NBIS decoding is known by checksum.
#[test]
fn real_fingerprint() {
    let data = read("jnbis-sample.wsq");
    let i = info(&data, 0).unwrap();
    assert_eq!(
        (i.width, i.height, i.model, i.bit_depth),
        (545, 622, Model::Grey, 8)
    );
    assert!(
        i.coding.starts_with("WSQ, 8-bit grey, 9/7-tap wavelet"),
        "{}",
        i.coding
    );
    let img = decode(&data, 0).unwrap();
    assert!(!img.truncated);
    assert_eq!(fnv(gray(&img)), 0x17c6_0411_3414_7420);
}

#[test]
fn nistcom_facts() {
    let i = info(&read("gen-wizard-77x100-r0.3.wsq"), 0).unwrap();
    let field = |k: &str| {
        i.fields
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(field("PIX_WIDTH"), Some("77"));
    assert_eq!(field("WSQ_BITRATE"), Some("0.300000"));
    assert!(i.summary.iter().any(|(k, _)| *k == "compression"));
    assert!(i.summary.iter().any(|(k, _)| *k == "creator"));
}

#[test]
fn truncated_files_give_partial_images() {
    let data = read("gen-fp-biomsharp-fp01-77x100-r4.0.wsq");
    let (_, _, want) = pgm(&read("gen-fp-biomsharp-fp01-77x100-r4.0.pgm"));
    // Cut inside the coded data: a partial image, flagged.
    let img = decode(&data[..data.len() * 3 / 4], 0).unwrap();
    assert!(img.truncated);
    assert_eq!(gray(&img).len(), want.len());
    // Every prefix and a few corruptions decode or fail without panicking.
    for n in 0..data.len() {
        let _ = decode(&data[..n], 0);
        let _ = info(&data[..n], 0);
    }
    let mut seed = 1u32;
    for _ in 0..300 {
        let mut d = data.clone();
        for _ in 0..4 {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let at = (seed >> 8) as usize % d.len();
            d[at] = (seed >> 16) as u8;
        }
        let _ = decode(&d, 0);
    }
}

#[test]
fn decode_reserves_from_the_budget() {
    // 8000 × 8000 claimed by a small file passes the size checks but needs 700 MB.
    let mut d = read("gen-rose-70x46-r0.75.wsq");
    let sof = d.windows(2).position(|w| w == [0xFF, 0xA2]).unwrap();
    d[sof + 6..sof + 10].copy_from_slice(&[0x1F, 0x40, 0x1F, 0x40]);
    d.resize(64 << 10, 0);
    let r = specular_core::budget::with_limit(64 << 20, || decode(&d, 0));
    assert!(matches!(r, Err(Error::NeedsMemory(_))), "{:?}", r.err());
}

#[test]
fn refusals() {
    assert_eq!(decode(b"\xFF\xD8\xFF\xE0", 0), Err(Error::NotRecognised));
    let data = read("gen-rose-70x46-r0.75.wsq");
    assert!(decode(&data, 1).is_err());
    // Header promising a huge image from a tiny file.
    let mut d = data.clone();
    let sof = d.windows(2).position(|w| w == [0xFF, 0xA2]).unwrap();
    d[sof + 6..sof + 10].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
    assert!(matches!(
        decode(&d, 0),
        Err(Error::TooLarge | Error::Truncated(_))
    ));
}
