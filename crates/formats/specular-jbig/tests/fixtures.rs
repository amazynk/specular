//! Committed fixtures made by JBIG-KIT's pbmtojbg (see tools/corpus/fetch-raster-jbig.sh) from
//! the PBM/PGM beside them: every one must decode to its source exactly.

mod common;

use common::read_pnm;
use specular_jbig::{Error, Model, Pixels, decode, info, probe};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn check(file: &str, src: &str) {
    let data = fixture(file);
    assert!(probe(&data), "{file}: probe");
    let want = read_pnm(&fixture(src));
    let img = decode(&data, 0).unwrap_or_else(|e| panic!("{file}: {e}"));
    assert!(!img.truncated, "{file}: truncated");
    assert_eq!((img.width, img.height), (want.width, want.height), "{file}");
    let got: Vec<u16> = match &img.pixels {
        Pixels::Gray8(v) => v.iter().map(|&x| u16::from(x)).collect(),
        Pixels::Gray16(v) => v.clone(),
        p => panic!("{file}: unexpected {p:?}"),
    };
    let max = if matches!(img.pixels, Pixels::Gray16(_)) {
        65535
    } else {
        255
    };
    let want = want.scaled(max);
    let bad = got.iter().zip(&want).filter(|(a, b)| a != b).count();
    assert_eq!(bad, 0, "{file}: {bad} pixels differ");
}

#[test]
fn every_variant_decodes_to_its_source() {
    for f in [
        "rose_fs_default",
        "rose_fs_two",
        "rose_fs_d3",
        "rose_fs_d2_o4",
        "rose_fs_d2_o8",
        "rose_fs_d2_o13",
        "rose_fs_d2_p0",
        "rose_fs_d2_p92",
        "rose_fs_at_c",
        "rose_fs_rst",
        "rose_fs_newlen",
        "rose_fs_fax",
        "rose_fs_comment",
    ] {
        check(&format!("{f}.jbg"), "rose_fs.pbm");
    }
    check("odd_1x1_default.jbg", "odd_1x1.pbm");
    check("odd_3x7_d5_s2.jbg", "odd_3x7.pbm");
    check("grad_g2_d2_o3.jbg", "grad_g2.pgm");
    check("grad_g12_d2_o6.jbg", "grad_g12.pgm");
}

#[test]
fn info_facts() {
    let i = info(&fixture("rose_fs_comment.jbg"), 0).unwrap();
    assert_eq!(i.model, Model::Bilevel);
    assert_eq!(i.bit_depth, 1);
    assert_eq!(i.pages, 1);
    assert!(
        i.summary
            .iter()
            .any(|(k, v)| *k == "comment" && v == "Specular JBIG test rose_fs")
    );
    let i = info(&fixture("rose_fs_d3.jbg"), 0).unwrap();
    assert!(i.coding.contains("4 resolution layers"), "{}", i.coding);
    assert!(i.fields.iter().any(|(k, _)| k == "Layer sizes"));
    let i = info(&fixture("grad_g12_d2_o6.jbg"), 0).unwrap();
    assert_eq!((i.model, i.bit_depth), (Model::Grey, 12));
    // NEWLEN: the header announces more lines than the image has.
    let d = fixture("rose_fs_newlen.jbg");
    let i = info(&d, 0).unwrap();
    assert_eq!(i.height, read_pnm(&fixture("rose_fs.pbm")).height);
    assert!(i.fields.iter().any(|(k, _)| k.contains("NEWLEN")));
}

#[test]
fn refuses_a_continuation_bie() {
    let d = fixture("refuse_dl1.jbg");
    assert!(probe(&d));
    assert!(matches!(decode(&d, 0), Err(Error::Unsupported(_))));
    assert!(matches!(info(&d, 0), Err(Error::Unsupported(_))));
}

#[test]
fn cut_files_and_garbage_never_panic() {
    let d = fixture("rose_fs_d3.jbg");
    for n in [0, 5, 19, 20, 21, 40, d.len() / 2, d.len() - 3, d.len() - 1] {
        let r = decode(&d[..n], 0);
        if let Ok(img) = r {
            assert!(img.truncated || n >= d.len() - 1, "{n}");
        }
    }
    // Every byte flipped in turn.
    for i in 0..d.len() {
        let mut e = d.clone();
        e[i] ^= 0x5a;
        let _ = info(&e, 0);
        let _ = decode(&e, 0);
    }
    assert!(!probe(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01"));
    assert!(!probe(b"short"));
}

#[test]
fn decode_reserves_from_the_budget() {
    // 16000 × 16000 is under the caps but needs 768 MB.
    let mut h = vec![0u8, 0, 1, 0];
    h.extend(16_000u32.to_be_bytes());
    h.extend(16_000u32.to_be_bytes());
    h.extend(128u32.to_be_bytes());
    h.extend([8, 0, 3, 0x1c]);
    h.extend([0xff, 0x02]);
    let r = specular_core::budget::with_limit(64 << 20, || decode(&h, 0));
    assert!(matches!(r, Err(Error::NeedsMemory(_))), "{:?}", r.err());
}

#[test]
fn too_large_is_refused_before_allocating() {
    let mut h = vec![0u8, 0, 1, 0];
    h.extend(1_000_000u32.to_be_bytes());
    h.extend(1_000_000u32.to_be_bytes());
    h.extend(128u32.to_be_bytes());
    h.extend([8, 0, 3, 0x1c]);
    h.extend([0xff, 0x02]);
    assert!(probe(&h));
    assert_eq!(decode(&h, 0).unwrap_err(), Error::TooLarge);
}
