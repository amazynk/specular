//! Rarer sensor layouts, against real files of the rawrest corpus when present
//! (`tools/corpus/fetch-rawrest.sh`, `SPECULAR_RAWREST_CORPUS`): GoPro GPR split into several
//! VC-5 tiles (built here from a real one-tile GPR, as no camera file with several tiles is
//! known), Fujifilm SuperCCD SR (F700: S rows read, R rows skipped) and Phase One IIQ in its
//! uncompressed formats (calibrated like IIQ L / S). Each test skips without its file.

mod common;

use std::path::PathBuf;

use common::{DngCodec, DngSpec, write_dng};
use specular_raw::{DecodeOptions, parse};

fn corpus_file(rel: &str) -> Option<Vec<u8>> {
    let dir = std::env::var_os("SPECULAR_RAWREST_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/rawrest")
        });
    let data = std::fs::read(dir.join(rel)).ok();
    if data.is_none() {
        eprintln!("skipped: {rel} not in the corpus");
    }
    data
}

/// Width, height, the VC-5 bitstream, the CFA pattern and the white level.
type Vc5Ifd = (u32, u32, Vec<u8>, [u8; 4], u32);

/// A little-endian or big-endian TIFF reader for the test: the IFD (IFD 0 or a SubIFD) with
/// compression 9, as (width, height, data, CFA pattern, white level).
fn vc5_ifd(d: &[u8]) -> Option<Vc5Ifd> {
    let le = d.get(..2)? == b"II";
    let u16_at = |o: usize| -> Option<u32> {
        let b: [u8; 2] = d.get(o..o + 2)?.try_into().ok()?;
        Some(if le {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        } as u32)
    };
    let u32_at = |o: usize| -> Option<u32> {
        let b: [u8; 4] = d.get(o..o + 4)?.try_into().ok()?;
        Some(if le {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    };
    let mut todo = vec![u32_at(4)? as usize];
    while let Some(off) = todo.pop() {
        let n = u16_at(off)? as usize;
        let mut tags = std::collections::HashMap::new();
        for i in 0..n {
            let p = off + 2 + 12 * i;
            let (tag, typ, cnt) = (u16_at(p)?, u16_at(p + 2)?, u32_at(p + 4)?);
            let one = if typ == 3 {
                u16_at(p + 8)?
            } else {
                u32_at(p + 8)?
            };
            tags.insert(tag, (typ, cnt, one, p + 8));
        }
        if let Some(&(_, cnt, first, at)) = tags.get(&330) {
            if cnt == 1 {
                todo.push(first as usize);
            } else {
                let base = u32_at(at)? as usize;
                for k in 0..cnt as usize {
                    todo.push(u32_at(base + 4 * k)? as usize);
                }
            }
        }
        if tags.get(&259).map(|t| t.2) != Some(9) {
            continue;
        }
        let w = tags.get(&256)?.2;
        let h = tags.get(&257)?.2;
        let (off_tag, len_tag) = if tags.contains_key(&324) {
            (324, 325)
        } else {
            (273, 279)
        };
        let (_, cnt, start, _) = *tags.get(&off_tag)?;
        let len = tags.get(&len_tag)?.2;
        if cnt != 1 {
            return None;
        }
        let data = d.get(start as usize..(start + len) as usize)?.to_vec();
        let (_, _, _, cfa_at) = *tags.get(&0x828e)?;
        let cfa: [u8; 4] = d.get(cfa_at..cfa_at + 4)?.try_into().ok()?;
        let white = tags.get(&0xc61d).map_or(4095, |t| t.2);
        return Some((w, h, data, cfa, white));
    }
    None
}

#[test]
fn gpr_in_several_vc5_tiles() {
    let Some(file) = corpus_file("GoPro/HERO8 Black/GOPR0009.GPR") else {
        return;
    };
    let (w, h, stream, cfa, white) = vc5_ifd(&file).expect("a VC-5 image");
    let opts = DecodeOptions::default();
    let one = parse(&file)
        .unwrap()
        .decode_raw_mosaic(&file, &opts)
        .unwrap();
    assert_eq!((one.width, one.height), (w, h));
    // The same bitstream twice, as two tiles side by side.
    let mut spec = DngSpec::new(
        2 * w,
        h,
        DngCodec::PreTiled {
            tile_w: w,
            tile_h: h,
            compression: 9,
        },
    );
    spec.cfa = cfa;
    spec.white = white;
    spec.bits = 16;
    spec.pre = vec![stream.clone(), stream];
    let two = write_dng(&spec, &[]);
    let f = parse(&two).unwrap();
    assert!(f.can_decode_sensor(), "{:?}", f.info().sensor);
    let raw = f.decode_raw_mosaic(&two, &opts).unwrap();
    assert!(!raw.truncated);
    assert_eq!((raw.width, raw.height), (2 * w, h));
    let (w, h) = (w as usize, h as usize);
    for y in (0..h).step_by(7) {
        let row = &raw.data[y * 2 * w..(y + 1) * 2 * w];
        let want = &one.data[y * w..(y + 1) * w];
        assert_eq!(&row[..w], want, "left tile, row {y}");
        assert_eq!(&row[w..], want, "right tile, row {y}");
    }
    // A tile missing: the rest decodes, flagged as truncated.
    let mut short = spec;
    short.pre.pop();
    let one_tile = write_dng(&short, &[]);
    let raw = parse(&one_tile)
        .unwrap()
        .decode_raw_mosaic(&one_tile, &opts)
        .unwrap();
    assert!(raw.truncated);
}

#[test]
fn superccd_sr_reads_the_s_rows() {
    let Some(file) = corpus_file("Fujifilm/FinePix F700/RAW_FUJI_F700.RAF") else {
        return;
    };
    let f = parse(&file).unwrap();
    assert!(f.can_decode_sensor(), "{:?}", f.info().sensor);
    let raw = f.decode_raw(&file, &DecodeOptions::default()).unwrap();
    // LibRaw's upright size for this camera.
    assert_eq!((raw.width, raw.height, raw.cpp), (2035, 1533, 3));
    assert!(!raw.truncated);
    // The R rows are about 1/14 as bright: had they been mixed in, alternate diagonals would
    // differ by that much. Neighbouring output pixels stay close instead.
    let w = raw.width as usize;
    let (mut big, mut n) = (0usize, 0usize);
    for y in (400..1100).step_by(3) {
        for x in (500..1500).step_by(3) {
            let a = raw.data[(y * w + x) * 3 + 1] as f64;
            let b = raw.data[(y * w + x + 1) * 3 + 1] as f64;
            if a > 500.0 && b > 500.0 {
                n += 1;
                big += (a / b > 3.0 || b / a > 3.0) as usize;
            }
        }
    }
    assert!(
        n > 1000 && big * 100 < n,
        "{big} of {n} neighbours differ threefold"
    );
}

#[test]
fn uncompressed_iiq_is_calibrated() {
    let Some(file) = corpus_file("Phase One/H 25/H25_IT8.7-2_Card.TIF") else {
        return;
    };
    let f = parse(&file).unwrap();
    let opts = DecodeOptions::default();
    let cal = f.decode_raw(&file, &opts).unwrap();
    let stored = f.decode_raw_as_stored(&file, &opts).unwrap();
    // The black tables and the calibration block (flat fields, defects) are applied by the
    // decode: most photosites change, by a few units on average.
    assert!(cal.black.iter().all(|&b| b == 0.0), "{:?}", cal.black);
    assert_eq!(cal.data.len(), stored.data.len());
    let changed = cal
        .data
        .iter()
        .zip(&stored.data)
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        changed * 2 > cal.data.len(),
        "{changed} of {} changed",
        cal.data.len()
    );
}
