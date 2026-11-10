//! Inputs found by `fuzz/` that once panicked. Each must decode (or fail) without panicking.

use specular_bitmap::{Decode, Options, Region, bmp::Bmp};

/// 32-bit bit-field BMP whose pixel offset lies past the end of the file: every row is
/// empty, and a region starting right of x = 0 used to slice `row[x0..x0]` out of range.
#[test]
fn bmp_region_on_rows_cut_before_x0() {
    let data: &[u8] = &[
        0x42, 0x4d, 0x16, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x8a, 0x00, 0x00, 0x00, 0x7c,
        0x00, 0x00, 0x00, 0x0b, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x01, 0x00, 0x20, 0x00,
        0x03, 0x00, 0x00, 0x00, 0x8c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7a, 0xe5, 0x98, 0xff,
        0xc8, 0x3f, 0x7a, 0xff, 0xc8, 0xc7, 0xca, 0xff, 0x7a, 0xe5, 0x98, 0xff, 0xc8, 0xc7, 0xca,
        0xff, 0x9e, 0x3f, 0x7a, 0xd4, 0xe0, 0x3f, 0x3d, 0x01, 0x00, 0x00, 0x00, 0x98, 0xff, 0xc8,
        0xc7, 0xca, 0xff,
    ];
    let b = Bmp::parse(data).expect("header parses");
    let opts = Options {
        scale: 3,
        region: Some(Region {
            x: 1,
            y: 0,
            w: 3,
            h: 2,
        }),
        parallel: false,
    };
    let img = b
        .decode(data, &opts, &|| false)
        .expect("decodes as truncated");
    assert!(img.truncated);
    assert_eq!(img.data.len(), img.width as usize * img.height as usize * 4);
}

/// A 7×5 canvas whose only frame claims 65535×65535 pixels backed by two bytes of LZW data:
/// the index buffer used to be sized from the frame header (4 GiB). It is now bounded by the
/// rows the canvas shows and by what the LZW data can produce.
#[test]
fn gif_frame_far_larger_than_canvas() {
    let mut data = b"GIF89a".to_vec();
    data.extend_from_slice(&[7, 0, 5, 0, 0, 0, 0]);
    data.extend_from_slice(&[0x2c, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0x40]); // interlaced
    data.extend_from_slice(&[2, 2, 0x4c, 0x01, 0, 0x3b]);
    let g = specular_bitmap::Gif::parse(&data).expect("header parses");
    for scale in [1, 2] {
        let img = g
            .decode(&data, &Options::scaled(scale), &|| false)
            .expect("decodes");
        assert_eq!(img.data.len(), img.width as usize * img.height as usize * 4);
    }
    let mut a = specular_bitmap::gif::Animator::new(std::sync::Arc::new(g));
    let (canvas, _, _) = a.next_frame(&data, &|| false).unwrap().unwrap();
    assert_eq!(canvas.len(), 7 * 5 * 4);
}
