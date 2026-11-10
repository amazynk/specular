//! An animated CMYK JPEG XL (a black extra channel) plays every frame through the registry,
//! not just the first, at every player scale (the file: `common/jxl_cmyk.rs`).

use std::sync::Arc;

use specular_core::{CancelToken, PixelLayout, Raster, Source};
use specular_decode::{DecodeRequest, Registry};

#[path = "common/jxl_cmyk.rs"]
mod jxl_cmyk;

use jxl_cmyk::animated_cmyk;

/// The RGB of pixel (3, 5) of a frame (planar or interleaved 8-bit).
fn rgb(r: &Raster) -> [u8; 3] {
    let (x, y) = (3usize, 5usize);
    match r.layout {
        PixelLayout::Rgb => std::array::from_fn(|k| r.planes[k].data[y * r.planes[k].stride + x]),
        PixelLayout::Rgba => {
            let p = &r.planes[0];
            std::array::from_fn(|k| p.data[y * p.stride + x * 4 + k])
        }
        l => panic!("{l:?}"),
    }
}

#[test]
fn animated_cmyk_jxl_plays_every_frame() {
    let file = animated_cmyk(&[64, 192, 128]);
    // The codestream as the crate reads it.
    let f = specular_jxl::JxlFile::open(file.clone()).expect("crate opens it");
    assert!(f.info().cmyk && f.info().animated, "{:?}", f.info().cmyk);
    let direct: Vec<_> = f
        .frames(1, false)
        .unwrap()
        .map(|d| d.expect("frame decodes"))
        .collect();
    assert_eq!(direct.len(), 3);

    let s = Registry::default()
        .open(Arc::new(Source::from_vec("anim.jxl", file)))
        .unwrap();
    assert!(s.animation().is_some(), "{}", s.info().coding);
    let mut frames = s.frames(1).unwrap();
    let cancel = CancelToken::new();
    let mut seen = Vec::new();
    while let Some(fr) = frames.next_frame(&cancel) {
        let fr = fr.unwrap();
        assert_eq!(fr.delay.as_millis(), 100);
        assert_eq!((fr.raster.width, fr.raster.height), (8, 8));
        seen.push(rgb(&fr.raster));
    }
    assert_eq!(seen.len(), 3, "{seen:?}");
    // Each frame its own colour; more ink is darker; the still is the first frame.
    assert!(seen[0] != seen[1] && seen[1] != seen[2], "{seen:?}");
    let sum = |c: [u8; 3]| c.iter().map(|&v| u32::from(v)).sum::<u32>();
    assert!(
        sum(seen[1]) > sum(seen[2]) && sum(seen[2]) > sum(seen[0]),
        "{seen:?}"
    );
    let still = s.decode(&DecodeRequest::full(1), &cancel).unwrap();
    assert_eq!(rgb(&still), seen[0]);
    // The player's reduced scales too.
    for scale in [2, 4, 8] {
        let mut fs = s.frames(scale).unwrap();
        let mut n = 0;
        while let Some(fr) = fs.next_frame(&cancel) {
            let fr = fr.unwrap();
            assert_eq!(fr.raster.width, 8u32.div_ceil(scale), "1/{scale}");
            n += 1;
        }
        assert_eq!(n, 3, "1/{scale}");
    }
    // Rewound, it plays again from the first frame.
    frames.rewind().unwrap();
    let again = frames.next_frame(&cancel).unwrap().unwrap();
    assert_eq!(rgb(&again.raster), seen[0]);
}
