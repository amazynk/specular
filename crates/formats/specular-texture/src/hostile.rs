//! Hostile containers: header fields that claim far more than the file holds.

use crate::{DecodeOptions, Region, Texture};

fn seed(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/fuzz/seeds/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

/// A zlib KTX2 level of 35 bytes declaring an 18-exabyte uncompressed length, the selected
/// layer 14 GiB in: the inflate buffer was sized from the declared length.
#[test]
fn ktx2_zlib_declared_length_is_not_allocated() {
    let mut d = seed("rgba8_zlib.ktx2");
    d[20..24].copy_from_slice(&32768u32.to_le_bytes()); // width
    d[24..28].copy_from_slice(&16384u32.to_le_bytes()); // height
    d[32..36].copy_from_slice(&7u32.to_le_bytes()); // layers
    d[40..44].copy_from_slice(&1u32.to_le_bytes()); // levels
    d[96..104].copy_from_slice(&u64::MAX.to_le_bytes()); // level 0 uncompressed length
    let mut t = Texture::open(&d).unwrap();
    assert!(t.select_image(6));
    let o = DecodeOptions {
        region: Some(Region {
            x: 0,
            y: 0,
            w: 1,
            h: 1,
        }),
        ..DecodeOptions::default()
    };
    let out = t.decode(&d, &o).unwrap();
    assert!(out.truncated);
}

/// A 16384 × 16384 zlib KTX2 (1 GiB of RGBA out, under the 4 GiB cap) from a tiny file: refused
/// under a 64 MiB memory budget before anything is allocated.
#[test]
fn ktx2_output_within_memory_budget() {
    let mut d = seed("rgba8_zlib.ktx2");
    d[20..24].copy_from_slice(&16384u32.to_le_bytes()); // width
    d[24..28].copy_from_slice(&16384u32.to_le_bytes()); // height
    d[40..44].copy_from_slice(&1u32.to_le_bytes()); // levels
    let t = Texture::open(&d).unwrap();
    let e = specular_core::budget::with_limit(64 << 20, || t.decode(&d, &DecodeOptions::default()))
        .unwrap_err();
    let crate::Error::NeedsMemory(n) = e else {
        panic!("{e:?}")
    };
    assert!(n.needed >= 1 << 30 && n.needed > n.free, "{n:?}");
    // A 1 × 1 region of it fits.
    let o = DecodeOptions {
        region: Some(Region {
            x: 0,
            y: 0,
            w: 1,
            h: 1,
        }),
        ..DecodeOptions::default()
    };
    assert!(specular_core::budget::with_limit(64 << 20, || t.decode(&d, &o)).is_ok());
}

/// A `.basis` file of 60000 one-block UASTC images: looking up each image's slices scanned the
/// whole table per image (3.6 billion steps).
#[test]
fn basis_many_slices_open_quickly() {
    const HEADER: usize = 77;
    const SLICE: usize = 23;
    let images = 60_000usize;
    let mut d = vec![0u8; HEADER];
    d[..2].copy_from_slice(b"sB");
    d[2..4].copy_from_slice(&0x13u16.to_le_bytes());
    d[4] = HEADER as u8;
    d[14..17].copy_from_slice(&(images as u32).to_le_bytes()[..3]);
    d[17..20].copy_from_slice(&(images as u32).to_le_bytes()[..3]);
    d[20] = 1; // UASTC
    d[23] = 1;
    d[65..69].copy_from_slice(&(HEADER as u32).to_le_bytes());
    let data = HEADER + images * SLICE;
    for i in 0..images {
        let mut s = [0u8; SLICE];
        s[..3].copy_from_slice(&(i as u32).to_le_bytes()[..3]);
        s[5..7].copy_from_slice(&4u16.to_le_bytes());
        s[7..9].copy_from_slice(&4u16.to_le_bytes());
        s[9..11].copy_from_slice(&1u16.to_le_bytes());
        s[11..13].copy_from_slice(&1u16.to_le_bytes());
        s[13..17].copy_from_slice(&((data + i * 16) as u32).to_le_bytes());
        s[17..21].copy_from_slice(&16u32.to_le_bytes());
        d.extend_from_slice(&s);
    }
    d.resize(data + images * 16, 0);
    let t = Texture::open(&d).unwrap();
    assert_eq!(t.image_count(), images as u32);
}
