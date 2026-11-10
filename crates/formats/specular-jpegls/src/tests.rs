use super::*;

/// The worked example of T.87 Annex H.3 (4×4, 8-bit, lossless), with the final SOF55 code.
const H3: [u8; 57] = [
    0xFF, 0xD8, 0xFF, 0xF7, 0x00, 0x0B, 0x08, 0x00, 0x04, 0x00, 0x04, 0x01, 0x01, 0x11, 0x00, 0xFF,
    0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x6C, 0x80, 0x20, 0x8E,
    0x01, 0xC0, 0x00, 0x00, 0x57, 0x40, 0x00, 0x00, 0x6E, 0xE6, 0x00, 0x00, 0x01, 0xBC, 0x18, 0x00,
    0x00, 0x05, 0xD8, 0x00, 0x00, 0x91, 0x60, 0xFF, 0xD9,
];
const H3_PIXELS: [u16; 16] = [
    0, 0, 90, 74, 68, 50, 43, 205, 64, 145, 145, 145, 100, 145, 145, 145,
];

#[test]
fn annex_h3_example() {
    assert!(probe(&H3));
    let info = read_info(&H3).unwrap();
    assert_eq!((info.width, info.height, info.precision), (4, 4, 8));
    assert_eq!((info.near, info.interleave, info.maxval), (0, 0, 255));
    let img = decode(&H3, &DecodeOptions::default()).unwrap();
    assert!(!img.truncated);
    assert_eq!(img.planes.len(), 1);
    assert_eq!(img.planes[0].data, H3_PIXELS);
}

#[test]
fn other_jpeg_processes_are_not_jpeg_ls() {
    let mut d = H3;
    d[3] = 0xC3;
    assert!(!probe(&d));
    assert_eq!(read_info(&d), Err(Error::NotJpegLs));
    assert_eq!(decode(&d, &DecodeOptions::default()), Err(Error::NotJpegLs));
    assert_eq!(read_info(b"GIF89a"), Err(Error::NotJpegLs));
}

#[test]
fn truncation_gives_partial_images() {
    // Cut inside the scan data: some lines decode, the rest are 0.
    for cut in 26..55 {
        match decode(&H3[..cut], &DecodeOptions::default()) {
            Ok(img) => {
                assert!(img.truncated, "cut {cut}");
                let p = &img.planes[0].data;
                assert_eq!(p.len(), 16);
                // Decoded lines are exact.
                for row in 0..4 {
                    let r = &p[row * 4..row * 4 + 4];
                    assert!(
                        r == &H3_PIXELS[row * 4..row * 4 + 4] || r == [0; 4],
                        "cut {cut}"
                    );
                }
            }
            Err(e) => assert!(
                matches!(e, Error::Truncated | Error::Malformed(_)),
                "cut {cut}: {e}"
            ),
        }
    }
    for cut in 0..26 {
        assert!(decode(&H3[..cut], &DecodeOptions::default()).is_err());
    }
}

#[test]
fn mutations_never_panic() {
    let mut seed = 0x1234_5678_u32;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    for _ in 0..20_000 {
        let mut d = H3.to_vec();
        for _ in 0..1 + rnd() % 4 {
            let i = rnd() as usize % d.len();
            d[i] = rnd() as u8;
        }
        let _ = decode(&d, &DecodeOptions::default());
    }
}

#[test]
fn huge_claimed_sizes_fail_before_allocating() {
    let mut d = H3.to_vec();
    // 65535 × 65535, one component: above the default sample cap.
    d[7..11].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
    assert!(matches!(
        decode(&d, &DecodeOptions::default()),
        Err(Error::TooLarge(_))
    ));
    // 30000 × 30000 under the cap: the scan runs out and the tiny file does not get a 1.8 GB
    // zero-filled image.
    d[7..11].copy_from_slice(&[0x75, 0x30, 0x75, 0x30]);
    assert!(decode(&d, &DecodeOptions::default()).is_err());
}

#[test]
fn planes_reserve_from_the_budget() {
    // 20000 × 20000 is under the sample cap, but its planes need 800 MB.
    let mut d = H3.to_vec();
    d[7..11].copy_from_slice(&[0x4E, 0x20, 0x4E, 0x20]);
    let r = specular_core::budget::with_limit(64 << 20, || decode(&d, &DecodeOptions::default()));
    assert!(matches!(r, Err(Error::NeedsMemory(_))), "{r:?}");
}

#[test]
fn cancel_is_honoured() {
    let flag = AtomicBool::new(true);
    let opts = DecodeOptions {
        cancel: Some(&flag),
        ..Default::default()
    };
    assert_eq!(decode(&H3, &opts), Err(Error::Cancelled));
}
