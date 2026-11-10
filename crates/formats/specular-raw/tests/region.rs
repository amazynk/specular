//! Region decodes ([`DecodeOptions::region`]): equal, sample for sample, to the same pixels of
//! the whole develop, decoding only the sensor tiles behind the region where possible.

mod common;

use common::{ArwSpec, DngCodec, DngSpec, test_pattern, write_arw, write_dng};
use specular_raw::{DecodeOptions, DevelopStats, OutputSize, Rect, RgbImage, TileCache};

fn rect(x: u32, y: u32, w: u32, h: u32) -> Rect {
    Rect { x, y, w, h }
}

fn cut(img: &RgbImage, r: Rect) -> Vec<u16> {
    let mut v = Vec::new();
    for y in r.y..r.y + r.h {
        let i = (y * img.width + r.x) as usize * 3;
        v.extend_from_slice(&img.data[i..i + r.w as usize * 3]);
    }
    v
}

/// Regions over the whole output of `w x h`: corners, edges, single pixels, tile seams.
fn regions(w: u32, h: u32, tile: u32) -> Vec<Rect> {
    let mut v = vec![
        rect(0, 0, w, h),
        rect(0, 0, 1, 1),
        rect(w - 1, h - 1, 1, 1),
        rect(0, 0, w.min(7), h.min(5)),
        rect(w - w.min(9), 0, w.min(9), h.min(3)),
        rect(0, h - h.min(4), w.min(3), h.min(4)),
        rect(w / 3, h / 3, (w / 3).max(1), (h / 3).max(1)),
        rect(w.saturating_sub(2), h.saturating_sub(2), 2, 2),
        // Larger than the output: clipped.
        rect(w / 2, h / 2, w, h),
    ];
    if tile > 2 && tile < w && tile < h {
        // Straddling a tile seam, and starting exactly on one.
        v.push(rect(tile - 2, tile - 1, 4, 3));
        v.push(rect(tile, tile, (w - tile).min(5), (h - tile).min(5)));
        v.push(rect(tile - 1, 0, 1, h));
    }
    v
}

/// Every region of `file` at both sizes equals the same pixels of the whole develop, decoded
/// for the region or developed from the whole sensor data with its statistics taken once; with
/// `partial` the region decode holds less than the whole sensor.
fn check(file: &[u8], tile: u32, partial: bool) {
    let raw = specular_raw::parse(file).expect("parse");
    assert_eq!(raw.region_decodable(), partial, "{:?}", raw.info());
    let kept = raw
        .decode_raw(file, &DecodeOptions::default())
        .expect("whole decode_raw");
    let stats = DevelopStats::of(&kept).expect("stats");
    for size in [OutputSize::Full, OutputSize::Half] {
        let whole = raw
            .decode(
                file,
                &DecodeOptions {
                    size,
                    ..Default::default()
                },
            )
            .expect("whole decode");
        let t = match size {
            OutputSize::Full => tile,
            OutputSize::Half => tile / 2,
        };
        let cache = TileCache::new(u64::MAX);
        for r in regions(whole.width, whole.height, t) {
            let want = rect(
                r.x,
                r.y,
                r.w.min(whole.width - r.x),
                r.h.min(whole.height - r.y),
            );
            for (parallel, tiles) in [(true, Some(&cache)), (false, None)] {
                let opts = DecodeOptions {
                    size,
                    region: Some(r),
                    tiles,
                    parallel,
                    ..Default::default()
                };
                let sensor = raw.decode_raw(file, &opts).expect("region decode_raw");
                assert!(!sensor.truncated);
                let all = sensor.window == rect(0, 0, sensor.width, sensor.height);
                if !partial {
                    assert!(all);
                } else if want.w * 4 < whole.width && want.h * 4 < whole.height {
                    assert!(!all, "{r:?}: whole sensor decoded");
                }
                let img = specular_raw::develop(&sensor, &opts).expect("region develop");
                assert_eq!((img.width, img.height), (want.w, want.h), "{r:?}");
                assert!(
                    img.data == cut(&whole, want),
                    "{size:?} {r:?} parallel {parallel}: differs from the whole develop"
                );
                assert_eq!(img.orientation, whole.orientation);
                assert_eq!(img.wb_source, whole.wb_source);
                let held = specular_raw::develop_with(&kept, &stats, &opts).expect("held");
                assert!(
                    held.data == img.data,
                    "{size:?} {r:?}: held develop differs"
                );
                assert_eq!(held.wb_source, whole.wb_source);
            }
        }
    }
}

/// A lossless ARW spec with an as-shot white balance.
fn spec(w: u32, h: u32, tile: u32) -> ArwSpec {
    let mut s = ArwSpec::new(w, h, tile);
    s.wb = Some([2400, 1024, 1024, 1700]);
    s
}

fn arw(s: &ArwSpec) -> Vec<u8> {
    let data = test_pattern(
        s.width as usize,
        s.height as usize,
        1,
        s.precision as u32,
        s.width,
    );
    write_arw(s, &data)
}

#[test]
fn arw_regions_match_the_whole_develop() {
    for (w, h, t) in [(64, 64, 32), (100, 70, 32), (33, 18, 16), (40, 2, 16)] {
        let mut s = spec(w, h, t);
        if w > 20 && h > 4 {
            s.crop = Some([2, 1, w - 5, h - 3]);
        }
        check(&arw(&s), t, true);
    }
    // Two-component frames, restart intervals, 12-bit data.
    let mut s = spec(70, 40, 32);
    s.quad = false;
    check(&arw(&s), 32, true);
    let mut s = spec(50, 36, 16);
    s.restart = 5;
    check(&arw(&s), 16, true);
    let mut s = spec(48, 32, 16);
    s.precision = 12;
    s.white = 4095;
    s.black = 128;
    check(&arw(&s), 16, true);
    // Without a white balance the grey-world estimate needs the whole sensor.
    let s = ArwSpec::new(64, 48, 16);
    check(&arw(&s), 16, false);
}

fn dng(s: &DngSpec) -> Vec<u8> {
    let cpp = if s.linear_raw { 3 } else { 1 };
    let data = test_pattern(s.width as usize, s.height as usize, cpp, s.bits, 7);
    write_dng(s, &data)
}

#[test]
fn dng_regions_match_the_whole_develop() {
    for (w, h) in [(64, 64), (100, 70), (33, 17)] {
        let mut s = DngSpec::new(w, h, DngCodec::Lj92Tiles(32));
        s.bits = 12;
        s.white = 4095;
        s.black = 64;
        s.neutral = [0.5, 1.0, 0.7];
        s.orientation = 6;
        if w > 40 {
            s.active_area = Some([2, 4, h - 1, w - 3]);
        }
        check(&dng(&s), 32, true);
    }
    // LinearRaw tiles.
    let mut s = DngSpec::new(48, 40, DngCodec::Lj92Tiles(16));
    s.linear_raw = true;
    check(&dng(&s), 16, true);
    // Uncompressed and packed strips, a linearization table.
    let mut s = DngSpec::new(90, 150, DngCodec::Packed(14));
    s.bits = 14;
    s.white = 16383;
    s.black = 300;
    check(&dng(&s), 0, true);
    let mut s = DngSpec::new(40, 30, DngCodec::U16);
    s.linearization = Some((0..65535u32).map(|v| (v / 2) as u16).collect());
    check(&dng(&s), 0, true);
    // Other 2x2 patterns.
    for cfa in [[2, 1, 1, 0], [1, 0, 2, 1]] {
        let mut s = DngSpec::new(40, 24, DngCodec::Lj92Tiles(16));
        s.cfa = cfa;
        check(&dng(&s), 16, true);
    }
}

/// DNG opcode lists (all point opcodes, bad pixels and TrimBounds in list 2, warps and a
/// vignette in list 3): regions still equal the whole develop and decode only a window.
#[test]
fn dng_opcode_regions_match_the_whole_develop() {
    use common::{OpParams, op_gain_map, op_vignette, op_warp, opcode_list};
    let (w, h) = (100u32, 70u32);
    // Relative to the active area (2, 4) .. (h - 1, w - 3).
    let (aw, ah) = (w - 7, h - 3);
    let warp = op_warp(
        &[
            [0.97, 0.05, -0.01, 0.0, 0.001, -0.002],
            [0.975, 0.04, 0.0, 0.0, 0.0, 0.0],
            [0.98, 0.03, 0.01, 0.0, -0.001, 0.0],
        ],
        [0.45, 0.55],
    );
    let gains: Vec<f32> = (0..12).map(|i| 0.8 + 0.05 * i as f32).collect();
    let list2 = opcode_list(
        2,
        vec![
            op_vignette([0.3, 0.1, 0.0, 0.0, 0.0], [0.45, 0.55]),
            op_gain_map([0, 0, ah, aw], 1, 3, 4, &gains),
            (
                8,
                0,
                OpParams::default()
                    .u32(&[0, 0, ah, aw, 0, 1, 1, 1, 2])
                    .f64(&[0.0, 0.9, 0.1]),
            ),
            (4, 0, OpParams::default().u32(&[0, 0])),
            (
                5,
                0,
                OpParams::default().u32(&[0, 2, 1, 10, 11, 30, 40, 50, 60, 52, 63]),
            ),
            (
                12,
                0,
                OpParams::default()
                    .u32(&[0, 0, ah, aw, 0, 1, 2, 1, ah.div_ceil(2)])
                    .f32(
                        &(0..ah.div_ceil(2))
                            .map(|i| 1.0 - 0.002 * i as f32)
                            .collect::<Vec<_>>(),
                    ),
            ),
            (
                11,
                0,
                OpParams::default()
                    .u32(&[0, 0, ah, aw, 0, 1, 1, 3, aw.div_ceil(3)])
                    .f32(
                        &(0..aw.div_ceil(3))
                            .map(|i| 0.0005 * (i % 5) as f32)
                            .collect::<Vec<_>>(),
                    ),
            ),
            (
                7,
                0,
                OpParams::default()
                    .u32(&[1, 1, ah - 1, aw - 1, 0, 1, 1, 1, 4096])
                    .u16(&(0..4096u32).map(|v| (v * 15) as u16).collect::<Vec<_>>()),
            ),
            (6, 0, OpParams::default().u32(&[1, 2, ah - 2, aw - 1])),
            (99, 1, OpParams::default().u32(&[1, 2, 3])),
        ],
    );
    let list3 = opcode_list(
        3,
        vec![warp, op_vignette([-0.1, 0.0, 0.0, 0.0, 0.0], [0.5, 0.5])],
    );
    let mut s = DngSpec::new(w, h, DngCodec::Lj92Tiles(32));
    s.bits = 12;
    s.white = 4095;
    s.black = 64;
    s.neutral = [0.5, 1.0, 0.7];
    s.active_area = Some([2, 4, h - 1, w - 3]);
    s.extra = vec![list2.clone(), list3.clone()];
    let file = dng(&s);
    let raw = specular_raw::parse(&file).unwrap();
    let si = raw.info().sensor.clone().unwrap();
    assert_eq!(si.opcodes_skipped, Vec::<&str>::new());
    assert_eq!(
        si.opcodes,
        [
            "FixVignetteRadial",
            "GainMap",
            "MapPolynomial",
            "FixBadPixelsConstant",
            "FixBadPixelsList",
            "ScalePerRow",
            "DeltaPerColumn",
            "MapTable",
            "TrimBounds",
            "WarpRectilinear"
        ]
    );
    // TrimBounds: 1 row and 2 columns off each side but the right one (one column).
    assert_eq!(si.crop, rect(6, 3, aw - 3, ah - 3));
    check(&file, 32, true);
    // LinearRaw, a gain map per plane and a single-plane warp.
    let mut s = DngSpec::new(48, 40, DngCodec::Lj92Tiles(16));
    s.linear_raw = true;
    let gains: Vec<f32> = (0..2 * 3 * 3).map(|i| 0.9 + 0.01 * i as f32).collect();
    s.extra = vec![
        opcode_list(2, vec![op_gain_map([0, 0, 40, 48], 3, 2, 3, &gains)]),
        opcode_list(
            3,
            vec![op_warp(&[[1.02, -0.04, 0.0, 0.0, 0.0, 0.0]], [0.5, 0.5])],
        ),
    ];
    check(&dng(&s), 16, true);
}

/// A single LJ92 strip is one tile.
#[test]
fn single_tile_strip() {
    let mut s = DngSpec::new(40, 24, DngCodec::Lj92Strip);
    s.bits = 12;
    s.white = 4095;
    check(&dng(&s), 0, true);
}

/// X-Trans (6x6) sensor data held whole: every region at both sizes, developed alone or with
/// statistics taken once, equals the same pixels of the whole develop; as shot and estimated
/// white balance, with and without a linearization table, odd crops.
#[test]
fn xtrans_regions_match_the_whole_develop() {
    const XTRANS: [u8; 36] = [
        1, 1, 0, 1, 1, 2, //
        1, 1, 2, 1, 1, 0, //
        2, 0, 1, 0, 2, 1, //
        1, 1, 2, 1, 1, 0, //
        1, 1, 0, 1, 1, 2, //
        0, 2, 1, 2, 0, 1,
    ];
    for (w, h, crop, wb, lin) in [
        (60, 48, rect(0, 0, 60, 48), Some([1.9, 1.0, 1.4]), false),
        (71, 53, rect(3, 2, 61, 47), None, false),
        (40, 30, rect(1, 1, 37, 27), Some([2.2, 1.0, 1.6]), true),
        (13, 9, rect(1, 0, 11, 9), None, true),
    ] {
        let cfa = specular_raw::Cfa::new(6, 6, XTRANS.to_vec()).unwrap();
        assert!(cfa.developable() && cfa.bayer2x2().is_none());
        let raw = specular_raw::RawImage {
            width: w,
            height: h,
            cpp: 1,
            data: common::test_pattern(w as usize, h as usize, 1, 14, w),
            window: rect(0, 0, w, h),
            cfa: Some(cfa),
            crop,
            black: [512.0; 4],
            white: 16383.0,
            linearization: lin.then(|| (0..16384u32).map(|v| (v * 3 / 4) as u16).collect()),
            wb,
            rgb_cam: [[1.6, -0.4, -0.2], [-0.2, 1.5, -0.3], [0.0, -0.5, 1.5]],
            matrix_source: specular_raw::MatrixSource::Generic,
            profile: None,
            orientation: 1,
            truncated: false,
            opcodes: None,
        };
        let stats = DevelopStats::of(&raw).unwrap();
        for size in [OutputSize::Full, OutputSize::Half] {
            let whole = specular_raw::develop(
                &raw,
                &DecodeOptions {
                    size,
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(
                specular_raw::develop_with(
                    &raw,
                    &stats,
                    &DecodeOptions {
                        size,
                        ..Default::default()
                    }
                )
                .unwrap()
                .data,
                whole.data
            );
            for r in regions(whole.width, whole.height, 6) {
                let want = rect(
                    r.x,
                    r.y,
                    r.w.min(whole.width - r.x),
                    r.h.min(whole.height - r.y),
                );
                for parallel in [true, false] {
                    let opts = DecodeOptions {
                        size,
                        region: Some(r),
                        parallel,
                        ..Default::default()
                    };
                    let img = specular_raw::develop(&raw, &opts).unwrap();
                    assert_eq!((img.width, img.height), (want.w, want.h), "{r:?}");
                    assert!(
                        img.data == cut(&whole, want),
                        "{w}x{h} {size:?} {r:?}: differs from the whole develop"
                    );
                    let held = specular_raw::develop_with(&raw, &stats, &opts).unwrap();
                    assert!(held.data == img.data, "{w}x{h} {size:?} {r:?}: held");
                    assert_eq!(held.wb_source, whole.wb_source);
                }
            }
        }
    }
}

fn large_corpus(rel: &str) -> Option<Vec<u8>> {
    let p = std::env::var_os("SPECULAR_LARGE_CORPUS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/large")
        })
        .join(rel);
    let d = std::fs::read(&p).ok();
    if d.is_none() {
        eprintln!("{} not present; skipping", p.display());
    }
    d
}

/// Real files of the large corpus (read in place, never copied): tiled DNGs decode only the
/// region's tiles; masked black (CR2), X-Trans (RAF) and other codecs decode the whole sensor;
/// all crop exactly, also developed from whole sensor data kept with its statistics. Skipped
/// when absent.
#[test]
fn corpus_regions_match_the_whole_develop() {
    for (rel, partial) in [
        ("raw/Adobe DNG Converter/Nikon D6/D06_0316.dng", true),
        // FixVignetteRadial (list 2) and WarpRectilinear (list 3).
        ("raw/Adobe DNG Converter/Nikon Z 6/DSC_0757.dng", true),
        ("raw/Adobe DNG Converter/Fujifilm X-T4/DSCF0268.dng", false),
        ("raw/Canon/EOS 1300D/IMG_0778.CR2", false),
        ("raw/Fujifilm/X-T30 II/compressed_X-T30II.RAF", false),
        ("raw/Nikon/D6/D06_0316.NEF", false),
    ] {
        let Some(data) = large_corpus(rel) else {
            continue;
        };
        let raw = specular_raw::parse(&data).unwrap();
        assert_eq!(raw.region_decodable(), partial, "{rel}");
        let all = raw.decode_raw(&data, &DecodeOptions::default()).unwrap();
        let stats = DevelopStats::of(&all).unwrap();
        for size in [OutputSize::Full, OutputSize::Half] {
            let o = DecodeOptions {
                size,
                ..Default::default()
            };
            let whole = raw.decode(&data, &o).unwrap();
            let (w, h) = (whole.width, whole.height);
            for r in [
                rect(0, 0, 300, 200),
                rect(w - 257, h - 129, 257, 129),
                rect(w / 2 - 100, h / 3, 255, 1),
            ] {
                let o = DecodeOptions {
                    size,
                    region: Some(r),
                    ..Default::default()
                };
                let img = raw.decode(&data, &o).unwrap();
                assert!(img.data == cut(&whole, r), "{rel} {size:?} {r:?}");
                let held = specular_raw::develop_with(&all, &stats, &o).unwrap();
                assert!(held.data == img.data, "{rel} {size:?} {r:?}: held");
            }
        }
    }
}

#[test]
fn tiles_are_reused_and_bounded() {
    let s = spec(128, 128, 32);
    let file = arw(&s);
    let raw = specular_raw::parse(&file).unwrap();
    let cache = TileCache::new(u64::MAX);
    let opts = |r| DecodeOptions {
        region: Some(r),
        tiles: Some(&cache),
        ..Default::default()
    };
    // Output (40..50, 40..50) needs sensor 38..52: tile (1, 1) only.
    let a = raw.decode(&file, &opts(rect(40, 40, 10, 10))).unwrap();
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.memory_bytes(), 32 * 32 * 2);
    // Across the seam at 64: four tiles, one of them held.
    raw.decode(&file, &opts(rect(60, 60, 8, 8))).unwrap();
    assert_eq!(cache.len(), 4);
    let b = raw.decode(&file, &opts(rect(40, 40, 10, 10))).unwrap();
    assert_eq!(a.data, b.data);
    assert_eq!(cache.len(), 4);
    // A one-tile budget keeps the latest tile.
    let small = TileCache::new(32 * 32 * 2);
    let o = DecodeOptions {
        region: Some(rect(60, 60, 8, 8)),
        tiles: Some(&small),
        ..Default::default()
    };
    let c = raw.decode(&file, &o).unwrap();
    assert!(small.memory_bytes() <= 32 * 32 * 2);
    let whole = raw.decode(&file, &DecodeOptions::default()).unwrap();
    assert_eq!(c.data, cut(&whole, rect(60, 60, 8, 8)));
    // Another file empties the cache.
    let other = arw(&spec(96, 64, 32));
    let raw2 = specular_raw::parse(&other).unwrap();
    raw2.decode(&other, &opts(rect(1, 1, 2, 2))).unwrap();
    assert_eq!(cache.len(), 1);
    // Memory estimate: window, tiles and output.
    let m = raw
        .region_memory(rect(40, 40, 10, 10), OutputSize::Full)
        .unwrap();
    assert!(m >= 32 * 32 * 2 + 10 * 10 * 6, "{m}");
    assert!(m < 128 * 128 * 2, "{m}");
}

#[test]
fn damaged_tiles_mark_the_region_truncated() {
    let s = spec(64, 64, 32);
    let mut file = arw(&s);
    let raw = specular_raw::parse(&file).unwrap();
    // Cut the file inside the last tile: only regions touching it are truncated.
    let whole = raw.decode_raw(&file, &DecodeOptions::default()).unwrap();
    assert!(!whole.truncated);
    // The tiles come first, then the raw IFD (IFD 0's SubIFDs entry points at it).
    let le32 = |p: usize| u32::from_le_bytes(file[p..p + 4].try_into().unwrap()) as usize;
    let ifd0 = le32(4);
    let entries = u16::from_le_bytes([file[ifd0], file[ifd0 + 1]]) as usize;
    let sub = (0..entries)
        .map(|i| ifd0 + 2 + 12 * i)
        .find(|&p| file[p..p + 2] == 0x014au16.to_le_bytes())
        .unwrap();
    let tiles_end = le32(sub + 8);
    file.truncate(tiles_end - 40);
    let near = DecodeOptions {
        region: Some(rect(0, 0, 8, 8)),
        ..Default::default()
    };
    let far = DecodeOptions {
        region: Some(rect(56, 56, 8, 8)),
        ..Default::default()
    };
    let full = raw.decode(&file, &DecodeOptions::default()).unwrap();
    assert!(full.truncated);
    let a = raw.decode(&file, &near).unwrap();
    assert!(!a.truncated);
    assert_eq!(a.data, cut(&full, rect(0, 0, 8, 8)));
    let b = raw.decode(&file, &far).unwrap();
    assert!(b.truncated);
    assert_eq!(b.data, cut(&full, rect(56, 56, 8, 8)));
    // Empty or outside regions are refused.
    for r in [rect(0, 0, 0, 5), rect(64, 0, 4, 4), rect(0, u32::MAX, 4, 4)] {
        let o = DecodeOptions {
            region: Some(r),
            ..Default::default()
        };
        assert!(raw.decode(&file, &o).is_err(), "{r:?}");
    }
}

#[test]
fn region_cancel_flag() {
    let file = arw(&spec(64, 64, 32));
    let raw = specular_raw::parse(&file).unwrap();
    let stop = std::sync::atomic::AtomicBool::new(true);
    let r = raw.decode_raw(
        &file,
        &DecodeOptions {
            region: Some(rect(0, 0, 40, 40)),
            cancel: Some(&stop),
            ..Default::default()
        },
    );
    assert!(matches!(r, Err(specular_raw::RawError::Cancelled)));
}

/// 1920x1080 regions of the ILCE-7RM5 lossless sample (large corpus, read in place, never
/// copied) equal the same pixels of the whole develop. Skipped when absent.
#[test]
fn corpus_7rm5_regions() {
    let Some(data) = large_corpus("raw/Sony/ILCE-7RM5/7RM5-LosslessCompressedLarge.ARW") else {
        return;
    };
    let raw = specular_raw::parse(&data).unwrap();
    assert!(raw.region_decodable());
    let whole = raw.decode(&data, &DecodeOptions::default()).unwrap();
    let cache = TileCache::new(256 << 20);
    let (w, h) = (whole.width, whole.height);
    for r in [
        rect(0, 0, 1920, 1080),
        rect(w - 1920, h - 1080, 1920, 1080),
        rect(w / 2 - 960, h / 2 - 540, 1920, 1080),
        rect(511, 255, 1920, 1080),
        rect(w - 7, 3, 7, 5),
    ] {
        let o = DecodeOptions {
            region: Some(r),
            tiles: Some(&cache),
            ..Default::default()
        };
        let sensor = raw.decode_raw(&data, &o).unwrap();
        assert!(sensor.window.w < 2400 && sensor.window.h < 1500);
        let img = specular_raw::develop(&sensor, &o).unwrap();
        assert!(img.data == cut(&whole, r), "{r:?}");
    }
}
