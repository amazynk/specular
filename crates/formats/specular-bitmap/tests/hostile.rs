//! Hostile inputs built in place: each must parse (or fail) quickly, without a panic.

use specular_bitmap::ico::Ico;

/// 65535 directory entries all pointing at one 1 MiB 32-bit image: each entry's header scan
/// read the whole image again (65535 × 1 MiB). The scans now stop at a few times the file.
#[test]
fn ico_entries_sharing_one_image() {
    const N: usize = 65535;
    const SIDE: u32 = 512;
    let dib_at = 6 + N * 16;
    let pixels = (SIDE * SIDE * 4) as usize;
    let mask = (SIDE / 8 * SIDE) as usize;
    let dib_len = 40 + pixels + mask;
    let mut f = vec![0, 0, 1, 0];
    f.extend((N as u16).to_le_bytes());
    for _ in 0..N {
        f.extend([0, 0, 0, 0, 1, 0, 32, 0]);
        f.extend((dib_len as u32).to_le_bytes());
        f.extend((dib_at as u32).to_le_bytes());
    }
    // BITMAPINFOHEADER: the height counts the AND mask too.
    f.extend(40u32.to_le_bytes());
    f.extend(SIDE.to_le_bytes());
    f.extend((2 * SIDE).to_le_bytes());
    f.extend(1u16.to_le_bytes());
    f.extend(32u16.to_le_bytes());
    f.extend([0; 24]);
    f.resize(f.len() + pixels + mask, 0);
    assert_eq!(f.len(), dib_at + dib_len);

    let ico = Ico::parse(&f).expect("the first entries parse");
    assert_eq!(ico.count, N);
    assert!(!ico.pages.is_empty() && ico.pages.len() <= 64);
    assert_eq!((ico.pages[0].width, ico.pages[0].height), (SIDE, SIDE));
}

/// Headers alone asking for 16384×16384 pixels (1 GiB of RGBA): the output was allocated
/// and filled before any pixel data. Refused as truncated now; a 4096×4096 header (under
/// the 16 M pixel floor) still decodes, grey.
mod header_only {
    use specular_bitmap::{Decode, Error, Options};

    const NO: &(dyn Fn() -> bool + Sync) = &|| false;

    fn refused(d: &dyn Decode, data: &[u8]) {
        let r = d.decode(data, &Options::default(), NO);
        assert!(matches!(r, Err(Error::Truncated(_))), "{:?}", r.err());
    }

    fn qoi_file(side: u32) -> Vec<u8> {
        let mut f = b"qoif".to_vec();
        f.extend(side.to_be_bytes());
        f.extend(side.to_be_bytes());
        f.extend([4, 0]);
        f.extend([0, 0, 0, 0, 0, 0, 0, 1]);
        f
    }

    #[test]
    fn qoi() {
        let f = qoi_file(16384);
        refused(&specular_bitmap::qoi::Qoi::parse(&f).unwrap(), &f);
        let f = qoi_file(4096);
        let q = specular_bitmap::qoi::Qoi::parse(&f).unwrap();
        assert!(q.decode(&f, &Options::default(), NO).is_ok());
    }

    #[test]
    fn tga_rle() {
        let mut f = vec![0, 0, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        f.extend(16384u16.to_le_bytes());
        f.extend(16384u16.to_le_bytes());
        f.extend([32, 8]);
        f.extend([0xff, 1, 2, 3, 4]);
        refused(&specular_bitmap::tga::Tga::parse(&f).unwrap(), &f);
    }

    #[test]
    fn bmp_rle8_and_raw() {
        for (bpp, comp) in [(8u16, 1u32), (24, 0)] {
            let mut f = b"BM".to_vec();
            f.extend(0u32.to_le_bytes());
            f.extend(0u32.to_le_bytes());
            let pixels = 14 + 40 + if bpp == 8 { 1024 } else { 0 };
            f.extend((pixels as u32).to_le_bytes());
            f.extend(40u32.to_le_bytes());
            f.extend(16384i32.to_le_bytes());
            f.extend(16384i32.to_le_bytes());
            f.extend(1u16.to_le_bytes());
            f.extend(bpp.to_le_bytes());
            f.extend(comp.to_le_bytes());
            f.extend([0; 20]);
            f.resize(pixels + 16, 0);
            refused(&specular_bitmap::bmp::Bmp::parse(&f).unwrap(), &f);
        }
    }

    #[test]
    fn pnm_binary_and_plain() {
        for magic in ["P6", "P3"] {
            let f = format!("{magic}\n16384 16384\n255\n1 2 3\n").into_bytes();
            let p = specular_bitmap::pnm::Pnm::parse(&f).unwrap();
            refused(&p, &f);
            let r = p.decode_wide(&f, &Options::default(), NO);
            assert!(matches!(r, Err(Error::Truncated(_))));
        }
    }

    #[test]
    fn gif_screen() {
        // A 16384×16384 logical screen with one 1×1 frame.
        let mut f = b"GIF89a".to_vec();
        f.extend(16384u16.to_le_bytes());
        f.extend(16384u16.to_le_bytes());
        f.extend([0, 0, 0]);
        f.extend([0x2c, 0, 0, 0, 0, 1, 0, 1, 0, 0]);
        f.extend([2, 2, 0x4c, 0x01, 0, 0x3b]);
        assert!(matches!(
            specular_bitmap::Gif::parse(&f),
            Err(Error::Truncated(_))
        ));
    }
}
