//! X-Trans and other non-2x2 CFA DNGs: synthetic round trips (pattern phase through ActiveArea,
//! BlackLevelRepeatDim, refusals) and the Adobe DNG Converter X-Trans DNGs of the large corpus
//! (`tools/corpus/large`, fetched, never committed; skipped when absent) checked against
//! LibRaw's unpacked mosaic and against the RAFs they were converted from.

mod common;

use common::{DngCodec, DngSpec, Tag, XYZ_TO_SRGB, tag_longs, test_pattern, write_dng};
use specular_raw::{Cfa, DecodeOptions, OutputSize, Rect, RgbImage};

/// The X-T3 / X-T4 / X-T30 layout (CFAPattern of their DNGs).
const XTRANS: [u8; 36] = [
    1, 1, 2, 1, 1, 0, //
    2, 0, 1, 0, 2, 1, //
    1, 1, 2, 1, 1, 0, //
    1, 1, 0, 1, 1, 2, //
    0, 2, 1, 2, 0, 1, //
    1, 1, 0, 1, 1, 2, //
];

fn tag_shorts(tag: u16, v: &[u16]) -> Tag {
    (
        tag,
        3,
        v.len() as u32,
        v.iter().flat_map(|x| x.to_le_bytes()).collect(),
    )
}

fn tag_bytes(tag: u16, v: &[u8]) -> Tag {
    (tag, 1, v.len() as u32, v.to_vec())
}

/// A DNG spec with a `w`x`h` CFAPattern.
fn spec_with_cfa(width: u32, height: u32, codec: DngCodec, w: u16, h: u16, p: &[u8]) -> DngSpec {
    // 14-bit samples in 16-bit containers, as Adobe DNG Converter writes them.
    let mut s = DngSpec::new(width, height, codec);
    s.white = 16383;
    s.black = 512;
    s.color_matrix = XYZ_TO_SRGB;
    s.neutral = [0.6, 1.0, 0.8];
    // CFARepeatPatternDim is rows, columns.
    s.extra.push(tag_shorts(0x828d, &[h, w]));
    s.extra.push(tag_bytes(0x828e, p));
    s
}

fn develop(file: &[u8], size: OutputSize) -> RgbImage {
    let raw = specular_raw::parse(file).unwrap();
    raw.decode(
        file,
        &DecodeOptions {
            size,
            ..Default::default()
        },
    )
    .unwrap()
}

/// X-Trans DNGs parse, decode bit-exactly (uncompressed and LJ92 tiles) and develop at both
/// sizes; the pattern reaches the develop unchanged.
#[test]
fn xtrans_roundtrip() {
    for codec in [DngCodec::U16, DngCodec::Lj92Tiles(24), DngCodec::Lj92Strip] {
        let (w, h) = (66u32, 50u32);
        let s = spec_with_cfa(w, h, codec, 6, 6, &XTRANS);
        let data = test_pattern(w as usize, h as usize, 1, 14, 7);
        let file = write_dng(&s, &data);
        let raw = specular_raw::parse(&file).unwrap();
        let si = raw.info().sensor.clone().unwrap();
        assert!(si.decodable, "{:?}", si.unsupported_reason);
        let cfa = si.cfa.unwrap();
        assert_eq!(cfa, Cfa::new(6, 6, XTRANS.to_vec()).unwrap());
        assert!(cfa.bayer2x2().is_none() && cfa.developable());
        let img = raw.decode_raw(&file, &DecodeOptions::default()).unwrap();
        assert!(img.data == data && !img.truncated);
        assert_eq!(img.black, [512.0; 4]);
        for size in [OutputSize::Full, OutputSize::Half] {
            let rgb = specular_raw::develop(
                &img,
                &DecodeOptions {
                    size,
                    ..Default::default()
                },
            )
            .unwrap();
            let div = if size == OutputSize::Half { 2 } else { 1 };
            assert_eq!((rgb.width, rgb.height), (w / div, h / div));
            let mean = rgb.data.iter().map(|&v| v as f64).sum::<f64>() / rgb.data.len() as f64;
            assert!(mean > 5000.0 && mean < 60000.0, "mean {mean}");
        }
    }
}

/// CFAPattern repeats from the ActiveArea's corner: a DNG whose ActiveArea starts off the 6x6
/// grid develops exactly like the same visible samples stored without margins.
#[test]
fn xtrans_pattern_phase_follows_active_area() {
    let (w, h) = (60usize, 48usize);
    let data = test_pattern(w, h, 1, 14, 3);
    let plain = spec_with_cfa(w as u32, h as u32, DngCodec::U16, 6, 6, &XTRANS);
    let reference = write_dng(&plain, &data);
    for (top, left) in [(1usize, 4usize), (3, 5), (6, 6), (2, 0)] {
        let (fw, fh) = (w + left + 3, h + top + 2);
        let mut full = vec![300u16; fw * fh];
        for y in 0..h {
            full[(y + top) * fw + left..(y + top) * fw + left + w]
                .copy_from_slice(&data[y * w..(y + 1) * w]);
        }
        let mut s = spec_with_cfa(fw as u32, fh as u32, DngCodec::U16, 6, 6, &XTRANS);
        s.active_area = Some([top as u32, left as u32, (top + h) as u32, (left + w) as u32]);
        let file = write_dng(&s, &full);
        let raw = specular_raw::parse(&file).unwrap();
        let si = raw.info().sensor.clone().unwrap();
        assert_eq!(
            si.crop,
            Rect {
                x: left as u32,
                y: top as u32,
                w: w as u32,
                h: h as u32
            }
        );
        // The stored image's pattern, seen from the ActiveArea corner, is the file's.
        let cfa = si.cfa.unwrap();
        assert_eq!(
            cfa.shifted(left as u32, top as u32),
            Cfa::new(6, 6, XTRANS.to_vec()).unwrap()
        );
        for size in [OutputSize::Full, OutputSize::Half] {
            let a = develop(&file, size);
            let b = develop(&reference, size);
            assert!(a.data == b.data, "({top}, {left}) {size:?}");
        }
    }
}

/// BlackLevel with BlackLevelRepeatDim repeats from the ActiveArea's corner too (2x2 values
/// land on the stored positions); a 6x6 repeat is averaged.
#[test]
fn black_level_repeat_follows_active_area() {
    let (w, h) = (24u32, 20u32);
    let data = vec![2000u16; (w * h) as usize];
    let mut s = DngSpec::new(w, h, DngCodec::U16);
    s.extra.push(tag_shorts(0xc619, &[2, 2]));
    s.extra.push(tag_longs(0xc61a, &[100, 200, 300, 400]));
    s.active_area = Some([1, 1, h - 1, w - 1]);
    let file = write_dng(&s, &data);
    let raw = specular_raw::parse(&file).unwrap();
    let img = raw.decode_raw(&file, &DecodeOptions::default()).unwrap();
    // Stored (0, 0) is ActiveArea (-1, -1): the (1, 1) value.
    assert_eq!(img.black, [400.0, 300.0, 200.0, 100.0]);

    let mut s = spec_with_cfa(w + 6, h, DngCodec::U16, 6, 6, &XTRANS);
    s.extra.push(tag_shorts(0xc619, &[6, 6]));
    let bl: Vec<u32> = (0..36).map(|i| 500 + (i % 3) * 10).collect();
    s.extra.push(tag_longs(0xc61a, &bl));
    let data = vec![2000u16; ((w + 6) * h) as usize];
    let file = write_dng(&s, &data);
    let raw = specular_raw::parse(&file).unwrap();
    let img = raw.decode_raw(&file, &DecodeOptions::default()).unwrap();
    assert_eq!(img.black, [510.0; 4]);
}

/// Patterns the develop cannot handle are refused with a reason; RGB patterns with all three
/// colours in every 3x3 window (here a 2x2 RGBG, and a 3x3 diagonal layout) develop.
#[test]
fn other_patterns() {
    let reason = |w: u16, h: u16, p: &[u8]| {
        let s = spec_with_cfa(36, 36, DngCodec::U16, w, h, p);
        let file = write_dng(&s, &test_pattern(36, 36, 1, 14, 1));
        let raw = specular_raw::parse(&file).unwrap();
        let si = raw.info().sensor.clone().unwrap();
        if si.decodable {
            assert!(raw.decode(&file, &DecodeOptions::default()).is_ok());
        }
        si.unsupported_reason
    };
    // CYGM (CFAPlaneColor default is RGB, so colour 3 is outside it).
    let cygm = reason(2, 2, &[3, 1, 4, 5]);
    assert!(
        cygm.is_some_and(|r| r.contains("other than red")),
        "{cygm:?}"
    );
    let mut xtrans_cyan = XTRANS;
    xtrans_cyan[7] = 3;
    assert!(reason(6, 6, &xtrans_cyan).is_some());
    // Stripes: rows 1..=3 of this 4x4 have no red; demosaiced before the develop.
    let stripes = [0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 1, 1, 1, 1];
    assert_eq!(reason(4, 4, &stripes), None);
    assert_eq!(reason(2, 2, &[0, 1, 2, 1]), None);
    assert_eq!(reason(3, 3, &[0, 1, 2, 2, 0, 1, 1, 2, 0]), None);
}

/// A four-colour (cyan, magenta, yellow, green) DNG decodes to camera RGB and develops; a
/// grey card stays grey.
#[test]
fn cmyg_dng_develops() {
    let mut s = spec_with_cfa(36, 36, DngCodec::U16, 2, 2, &[0, 1, 2, 3]);
    // CFAPlaneColor: planes are cyan, magenta, yellow, green.
    s.extra.push(tag_bytes(0xc616, &[3, 4, 5, 1]));
    s.neutral = [1.0; 3];
    let file = write_dng(&s, &vec![4000u16; 36 * 36]);
    let raw = specular_raw::parse(&file).unwrap();
    let si = raw.info().sensor.clone().unwrap();
    assert!(si.decodable, "{:?}", si.unsupported_reason);
    let img = raw.decode_raw(&file, &DecodeOptions::default()).unwrap();
    assert_eq!((img.cpp, img.cfa.is_none()), (3, true));
    let mid = &img.data[(18 * 36 + 18) * 3..][..3];
    assert!(mid.iter().all(|&v| v == mid[0]), "{mid:?}");
    let rgb = develop(&file, OutputSize::Full);
    assert_eq!((rgb.width, rgb.height), (36, 36));
}

/// Dual-pixel DNG: two samples per photosite, added up.
#[test]
fn dual_pixel_dng_adds_halves() {
    let (w, h) = (16u32, 8u32);
    let mut s = spec_with_cfa(2 * w, h, DngCodec::U16, 2, 2, &[0, 1, 1, 2]);
    s.extra.push(tag_longs(0x100, &[w]));
    s.extra.push(tag_shorts(0x115, &[2]));
    let data: Vec<u16> = (0..2 * w * h).map(|i| 600 + (i % 7) as u16).collect();
    let file = write_dng(&s, &data);
    let raw = specular_raw::parse(&file).unwrap();
    let si = raw.info().sensor.clone().unwrap();
    assert!(si.decodable, "{:?}", si.unsupported_reason);
    let img = raw.decode_raw(&file, &DecodeOptions::default()).unwrap();
    assert_eq!(img.cpp, 1);
    let want: Vec<u16> = data.chunks_exact(2).map(|p| p[0] + p[1]).collect();
    assert_eq!(img.data, want);
    assert_eq!((img.white, img.black), (2.0 * 16383.0, [1024.0; 4]));
    develop(&file, OutputSize::Full);
}

// ---------------------------------------------------------------------------------------------
// Large corpus

/// A file of the large corpus: `SPECULAR_LARGE_CORPUS` (a worktree has no corpus of its own),
/// else the repository's `tools/corpus/large`.
fn corpus(rel: &str) -> Option<Vec<u8>> {
    let p = std::env::var_os("SPECULAR_LARGE_CORPUS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/large")
        })
        .join("raw")
        .join(rel);
    std::fs::read(&p)
        .inspect_err(|_| eprintln!("{} not present; skipping", p.display()))
        .ok()
}

/// (camera, DNG file name, RAF path, black, LibRaw's sum and position-weighted sum of the
/// unpacked mosaic).
const CORPUS: [(&str, &str, &str, f32, u64, u64); 12] = [
    (
        "X-T3",
        "AFXT2721.dng",
        "X-T3/AFXT2721.RAF",
        1022.0,
        82823209359,
        2702480941330391,
    ),
    (
        "X-T30 II",
        "compressed_X-T30II.dng",
        "X-T30 II/compressed_X-T30II.RAF",
        1022.0,
        106373044909,
        3481672776764182,
    ),
    (
        "X-H2",
        "Fuji-X-H2-Compressed-Raw.dng",
        "X-H2/Fuji-X-H2-Compressed-Raw.RAF",
        1020.0,
        82908630398,
        2715439729777199,
    ),
    (
        "X100VI",
        "DSCF0076.dng",
        "X100VI/DSCF0076.RAF",
        1023.0,
        117519762844,
        3849105358452034,
    ),
    (
        "X-S20",
        "DSCF2529.dng",
        "X-S20/DSCF2529.RAF",
        1019.0,
        61531516998,
        2017165653162446,
    ),
    (
        "X-T4",
        "DSCF0268.dng",
        "X-T4/DSCF0268.RAF",
        1022.0,
        46151186381,
        1509231531490877,
    ),
    (
        "X-T30",
        "DSCF0066.dng",
        "X-T30/DSCF0066.RAF",
        1022.0,
        95406936798,
        3126873755465059,
    ),
    (
        "X-T5",
        "DSCF0021.dng",
        "X-T5/DSCF0021.RAF",
        1020.0,
        108564851096,
        3554779385478613,
    ),
    (
        "X-E4",
        "fuji_xe4_32_lossy.dng",
        "X-E4/fuji_xe4_32_lossy.RAF",
        1022.0,
        61812638262,
        2023828587657394,
    ),
    (
        "X-H2S",
        "IMG-20221005-0667.dng",
        "X-H2S/IMG-20221005-0667.raf",
        1024.0,
        45895390069,
        1498854763599543,
    ),
    (
        "X100V",
        "DSCF0734.dng",
        "X100V/DSCF0734.RAF",
        1022.0,
        72930166201,
        2389613374171740,
    ),
    (
        "X-S10",
        "Fujifilm-X-S10-lossless-compressed.dng",
        "X-S10/Fujifilm-X-S10-lossless-compressed.RAF",
        1022.0,
        54889270383,
        1799392027475743,
    ),
];

/// Every X-Trans DNG decodes (as stored, before its opcode lists) to LibRaw's mosaic (sums of
/// the whole stored image), with its 6x6 pattern, black and white; the samples equal the
/// source RAF's inside its crop, with the same pattern phase.
#[test]
fn corpus_xtrans_dngs_match_libraw_and_their_rafs() {
    for (i, &(cam, name, raf, black, sum, weighted)) in CORPUS.iter().enumerate() {
        let Some(dng) = corpus(&format!("Adobe DNG Converter/Fujifilm {cam}/{name}")) else {
            continue;
        };
        let raw = specular_raw::parse(&dng).unwrap();
        let si = raw.info().sensor.clone().unwrap();
        assert!(si.decodable, "{cam}: {:?}", si.unsupported_reason);
        let img = raw
            .decode_raw_as_stored(&dng, &DecodeOptions::default())
            .unwrap();
        assert!(!img.truncated, "{cam}");
        let cfa = img.cfa.clone().unwrap();
        assert!(
            (cfa.width, cfa.height) == (6, 6) && cfa.developable(),
            "{cam}"
        );
        assert_eq!((img.black, img.white), ([black; 4], 16383.0), "{cam}");
        let (mut s, mut ws) = (0u64, 0u64);
        for (i, &v) in img.data.iter().enumerate() {
            s += v as u64;
            ws += v as u64 * (i as u64 % 65521 + 1);
        }
        assert_eq!((s, ws), (sum, weighted), "{cam}");
        // The RAF comparison (a second full decode) on a few bodies keeps the test short.
        if i % 4 != 1 {
            continue;
        }
        let Some(rd) = corpus(&format!("Fujifilm/{raf}")) else {
            continue;
        };
        let rr = specular_raw::parse(&rd).unwrap();
        let ri = rr.decode_raw(&rd, &DecodeOptions::default()).unwrap();
        let c = ri.crop;
        assert_eq!((c.w, c.h), (img.width, img.height), "{cam}");
        assert_eq!(
            ri.cfa.as_ref().unwrap().shifted(c.x, c.y),
            cfa,
            "{cam}: pattern phase"
        );
        let (w, rw) = (img.width as usize, ri.width as usize);
        for y in 0..img.height as usize {
            let a = &img.data[y * w..(y + 1) * w];
            let o = (c.y as usize + y) * rw + c.x as usize;
            assert!(a == &ri.data[o..o + w], "{cam}: row {y}");
        }
    }
}

/// An X-Trans DNG and its source RAF develop alike at half size (same samples and white
/// balance; the colour differs only by the DNG profile vs the RAF's matrix): mean levels per
/// channel within 8 %.
#[test]
fn corpus_xtrans_dng_develops_like_its_raf() {
    let (Some(dng), Some(raf)) = (
        corpus("Adobe DNG Converter/Fujifilm X-T30 II/compressed_X-T30II.dng"),
        corpus("Fujifilm/X-T30 II/compressed_X-T30II.RAF"),
    ) else {
        return;
    };
    let a = develop(&dng, OutputSize::Half);
    let b = develop(&raf, OutputSize::Half);
    assert_eq!((a.width, a.height), (b.width, b.height));
    let mean = |d: &[u16]| {
        let mut m = [0f64; 3];
        for p in d.chunks_exact(3) {
            for k in 0..3 {
                m[k] += p[k] as f64;
            }
        }
        m.map(|v| v / (d.len() / 3) as f64)
    };
    let (ma, mb) = (mean(&a.data), mean(&b.data));
    for k in 0..3 {
        assert!((ma[k] / mb[k] - 1.0).abs() < 0.08, "{ma:?} vs {mb:?}");
    }
}
