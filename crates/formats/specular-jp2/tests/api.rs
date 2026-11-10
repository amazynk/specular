//! Self-consistency of the public API: regions are exact crops of the full decode, thread
//! counts do not change results, and request validation.

mod common;

use common::*;
use specular_jp2::{
    Container, DecodeOptions, Decoder, Depth, Image, Jp2Error, Parallelism, PixelFormat, Pixels,
    Progression, Region,
};
use std::sync::atomic::AtomicBool;

fn crop(full: &Image, r: Region) -> Vec<u8> {
    let Pixels::U8(px) = &full.pixels else {
        unreachable!()
    };
    let ch = full.format.channels();
    let mut out = Vec::new();
    for y in r.y..r.y + r.h {
        let s = (y * full.width + r.x) as usize * ch;
        out.extend_from_slice(&px[s..s + r.w as usize * ch]);
    }
    out
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u32) -> u32 {
        (self.next() % n.max(1) as u64) as u32
    }
}

#[test]
fn regions_are_exact_crops() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for f in fixtures() {
        let n = name(&f);
        let data = std::fs::read(&f).unwrap();
        let dec = Decoder::new(&data).unwrap();
        for reduce in 0..=dec.info().levels.min(3) {
            let full = dec
                .decode(&DecodeOptions {
                    reduce,
                    ..Default::default()
                })
                .unwrap();
            let (fw, fh) = dec.info().reduced_size(reduce);
            assert_eq!((full.width, full.height), (fw, fh), "{n} r{reduce}");
            for _ in 0..6 {
                let x = rng.below(fw);
                let y = rng.below(fh);
                let w = 1 + rng.below(fw - x);
                let h = 1 + rng.below(fh - y);
                let r = Region { x, y, w, h };
                let img = dec
                    .decode(&DecodeOptions {
                        reduce,
                        region: Some(r),
                        ..Default::default()
                    })
                    .unwrap();
                assert_eq!((img.width, img.height, img.origin), (w, h, (x, y)));
                let Pixels::U8(px) = &img.pixels else {
                    unreachable!()
                };
                assert!(*px == crop(&full, r), "{n} reduce {reduce} region {r:?}");
            }
        }
    }
}

#[test]
fn thread_count_does_not_change_output() {
    for f in fixtures() {
        let data = std::fs::read(&f).unwrap();
        let dec = Decoder::new(&data).unwrap();
        let a = dec
            .decode(&DecodeOptions {
                parallelism: Parallelism::Single,
                ..Default::default()
            })
            .unwrap();
        let b = dec
            .decode(&DecodeOptions {
                parallelism: Parallelism::Threads(3),
                ..Default::default()
            })
            .unwrap();
        assert!(a == b, "{}", name(&f));
    }
}

#[test]
fn info_fields() {
    let read = |n: &str| std::fs::read(data_dir().join(n)).unwrap();
    let i = specular_jp2::read_info(&read("rgb_tiles.j2k")).unwrap();
    assert_eq!((i.width, i.height), (203, 157));
    assert_eq!(i.container, Container::Codestream);
    assert!(i.reversible && !i.gray && !i.has_alpha);
    assert!(i.tiles_x > 1 && i.tiles_y > 1);

    let i = specular_jp2::read_info(&read("gray_rev.jp2")).unwrap();
    assert_eq!(i.container, Container::Jp2);
    assert!(i.gray);
    assert_eq!(i.components.len(), 1);

    let i = specular_jp2::read_info(&read("gray12_rev.j2k")).unwrap();
    assert_eq!(i.components[0].precision, 12);

    let i = specular_jp2::read_info(&read("rgb_rpcl.j2k")).unwrap();
    assert_eq!(i.progression, Progression::Rpcl);

    let i = specular_jp2::read_info(&read("rgb_tileorigin.j2k")).unwrap();
    assert_eq!((i.x0, i.y0, i.width, i.height), (9, 13, 203, 157));

    // All components subsampled 2x2: output at component resolution, like OpenJPEG.
    let i = specular_jp2::read_info(&read("rgb_sub.jp2")).unwrap();
    assert_eq!(i.components[0].dx, 2);
    assert_eq!((i.width, i.height), (70, 66));
}

#[test]
fn formats_and_choose_reduce() {
    let data = std::fs::read(data_dir().join("gray_rev.jp2")).unwrap();
    let img = specular_jp2::decode(&data, &DecodeOptions::default()).unwrap();
    assert_eq!(img.format, PixelFormat::Gray8);
    let img = specular_jp2::decode(
        &data,
        &DecodeOptions {
            depth: Depth::Sixteen,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(img.format, PixelFormat::Gray16);

    let data = std::fs::read(data_dir().join("rgb_rev.j2k")).unwrap();
    let dec = Decoder::new(&data).unwrap();
    let info = dec.info();
    assert_eq!(specular_jp2::choose_reduce(info, 203, 157), 0);
    assert_eq!(specular_jp2::choose_reduce(info, 102, 79), 1);
    assert_eq!(specular_jp2::choose_reduce(info, 100, 60), 1);
    assert_eq!(specular_jp2::choose_reduce(info, 50, 39), 2);
    assert_eq!(specular_jp2::choose_reduce(info, 1, 1), info.levels);
}

#[test]
fn invalid_requests() {
    let data = std::fs::read(data_dir().join("rgb_rev.j2k")).unwrap();
    let dec = Decoder::new(&data).unwrap();
    let levels = dec.info().levels;
    let bad = |o: DecodeOptions| matches!(dec.decode(&o), Err(Jp2Error::InvalidRequest(_)));
    assert!(bad(DecodeOptions {
        reduce: levels + 1,
        ..Default::default()
    }));
    for r in [
        Region {
            x: 0,
            y: 0,
            w: 0,
            h: 5,
        },
        Region {
            x: 200,
            y: 0,
            w: 10,
            h: 5,
        },
        Region {
            x: 0,
            y: 150,
            w: 10,
            h: 8,
        },
    ] {
        assert!(bad(DecodeOptions {
            region: Some(r),
            ..Default::default()
        }));
    }
    let cancel = AtomicBool::new(true);
    assert!(matches!(
        dec.decode(&DecodeOptions {
            cancel: Some(&cancel),
            ..Default::default()
        }),
        Err(Jp2Error::Cancelled)
    ));
    assert!(matches!(
        specular_jp2::read_info(b"not a jpeg 2000 file"),
        Err(Jp2Error::NotJp2)
    ));
}

#[test]
fn alpha_palette_and_deep_info() {
    let read = |n: &str| std::fs::read(data_dir().join(n)).unwrap();
    let i = specular_jp2::read_info(&read("rgba_rev.jp2")).unwrap();
    assert!(i.has_alpha && !i.gray);
    let i = specular_jp2::read_info(&read("graya_rev.jp2")).unwrap();
    assert!(i.has_alpha && !i.gray, "grey + alpha is output as RGBA");
    let i = specular_jp2::read_info(&read("palette.jp2")).unwrap();
    assert!(i.has_palette && !i.gray && !i.has_alpha);
    let i = specular_jp2::read_info(&read("gray16_rev.j2k")).unwrap();
    assert_eq!(i.components[0].precision, 16);
    assert!(i.gray);
}
