//! Hand-built DPX and Cineon files covering every sample layout, packed with an encoder
//! written here from the format descriptions (independently of the decoder's unpacker).

use specular_film::{Error, Kind, Model, Pixels, cineon, decode, detect, dpx, info};

/// A deterministic pattern of `n` codes below `1 << bits`.
fn pattern(n: usize, bits: u8) -> Vec<u16> {
    let max = (1u32 << bits) - 1;
    (0..n)
        .map(|i| ((i as u32).wrapping_mul(2654435761) >> 7) % (max + 1))
        .map(|v| v as u16)
        .collect()
}

#[derive(Clone, Copy)]
enum Pack {
    /// LSB-first (DPX) or MSB-first (Cineon tight) bit stream in 32-bit words.
    Stream(bool),
    /// Units of `bytes`, samples from the most significant end, padding at LSB (`left`) or MSB.
    Units(usize, bool),
    /// The same with the first sample at the least significant end (one-component 10-bit DPX).
    UnitsLow(usize, bool),
}

fn put_word(out: &mut Vec<u8>, w: u32, be: bool) {
    out.extend_from_slice(&if be { w.to_be_bytes() } else { w.to_le_bytes() });
}

/// One row of samples, padded to 32 bits.
fn pack_row(codes: &[u16], bits: usize, pack: Pack, be: bool) -> Vec<u8> {
    let mut out = Vec::new();
    match pack {
        Pack::Stream(lsb) => {
            let mut acc: u64 = 0;
            let mut n = 0usize;
            for &c in codes {
                if lsb {
                    acc |= (c as u64) << n;
                    n += bits;
                    while n >= 32 {
                        put_word(&mut out, acc as u32, be);
                        acc >>= 32;
                        n -= 32;
                    }
                } else {
                    acc = acc << bits | c as u64;
                    n += bits;
                    while n >= 32 {
                        put_word(&mut out, (acc >> (n - 32)) as u32, be);
                        n -= 32;
                        acc &= (1u64 << n) - 1;
                    }
                }
            }
            if n > 0 {
                let w = if lsb {
                    acc as u32
                } else {
                    (acc << (32 - n)) as u32
                };
                put_word(&mut out, w, be);
            }
        }
        Pack::Units(bytes, left) | Pack::UnitsLow(bytes, left) => {
            let low = matches!(pack, Pack::UnitsLow(..));
            let ubits = bytes * 8;
            let per = ubits / bits;
            let pad = ubits - per * bits;
            for chunk in codes.chunks(per) {
                let mut u: u32 = 0;
                for (k, &c) in chunk.iter().enumerate() {
                    let shift = if low {
                        bits * k + if left { pad } else { 0 }
                    } else {
                        ubits - bits * (k + 1) - if left { 0 } else { pad }
                    };
                    u |= (c as u32) << shift;
                }
                match bytes {
                    1 => out.push(u as u8),
                    2 => out.extend_from_slice(&if be {
                        (u as u16).to_be_bytes()
                    } else {
                        (u as u16).to_le_bytes()
                    }),
                    _ => put_word(&mut out, u, be),
                }
            }
            while out.len() % 4 != 0 {
                out.push(0);
            }
        }
    }
    out
}

struct DpxElem {
    descriptor: u8,
    transfer: u8,
    colorimetric: u8,
    bits: u8,
    packing: u16,
    encoding: u16,
    refs: Option<(u32, u32)>,
    data: Vec<u8>,
}

fn elem(descriptor: u8, bits: u8, packing: u16, data: Vec<u8>) -> DpxElem {
    DpxElem {
        descriptor,
        transfer: 4,
        colorimetric: 4,
        bits,
        packing,
        encoding: 0,
        refs: None,
        data,
    }
}

/// A DPX file with the generic and industry headers, `elems` stored one after another.
fn dpx_file(be: bool, w: u32, h: u32, orientation: u16, elems: &[DpxElem]) -> Vec<u8> {
    let mut f = vec![0xffu8; 2048];
    let w16 = |f: &mut Vec<u8>, o: usize, v: u16| {
        f[o..o + 2].copy_from_slice(&if be { v.to_be_bytes() } else { v.to_le_bytes() })
    };
    let w32 = |f: &mut Vec<u8>, o: usize, v: u32| {
        f[o..o + 4].copy_from_slice(&if be { v.to_be_bytes() } else { v.to_le_bytes() })
    };
    let s = |f: &mut Vec<u8>, o: usize, len: usize, t: &str| {
        f[o..o + len].fill(0);
        f[o..o + t.len()].copy_from_slice(t.as_bytes());
    };
    f[0..4].copy_from_slice(if be { b"SDPX" } else { b"XPDS" });
    w32(&mut f, 4, 2048);
    s(&mut f, 8, 8, "V2.0");
    w32(&mut f, 24, 1664);
    w32(&mut f, 28, 384);
    w32(&mut f, 32, 0);
    s(&mut f, 36, 100, "shot_0001.dpx");
    s(&mut f, 136, 24, "2024:05:06:07:08:09Z");
    s(&mut f, 160, 100, "Test scanner");
    s(&mut f, 260, 200, "Project X");
    s(&mut f, 460, 200, "");
    w16(&mut f, 768, orientation);
    w16(&mut f, 770, elems.len() as u16);
    w32(&mut f, 772, w);
    w32(&mut f, 776, h);
    let mut off = 2048u32;
    for (i, e) in elems.iter().enumerate() {
        let o = 780 + 72 * i;
        w32(&mut f, o, 0);
        if let Some((l, hc)) = e.refs {
            w32(&mut f, o + 4, l);
            w32(&mut f, o + 12, hc);
        }
        f[o + 20] = e.descriptor;
        f[o + 21] = e.transfer;
        f[o + 22] = e.colorimetric;
        f[o + 23] = e.bits;
        w16(&mut f, o + 24, e.packing);
        w16(&mut f, o + 26, e.encoding);
        w32(&mut f, o + 28, off);
        w32(&mut f, o + 32, 0);
        w32(&mut f, o + 36, 0);
        s(&mut f, o + 40, 32, &format!("element {i}"));
        off += e.data.len() as u32;
    }
    s(&mut f, 1556, 32, "Scanity");
    s(&mut f, 1588, 32, "SN42");
    // Film: KeyKode KU 22 123456 7890+12, frame 12 of 100, 24 fps.
    s(&mut f, 1664, 2, "KU");
    s(&mut f, 1666, 2, "22");
    s(&mut f, 1668, 2, "12");
    s(&mut f, 1670, 6, "123456");
    s(&mut f, 1676, 4, "7890");
    s(&mut f, 1680, 32, "Academy");
    w32(&mut f, 1712, 12);
    w32(&mut f, 1716, 100);
    w32(&mut f, 1724, 24f32.to_bits());
    s(&mut f, 1764, 100, "Scene 4 take 2");
    w32(&mut f, 1920, 0x01020304);
    for e in elems {
        f.extend_from_slice(&e.data);
    }
    let len = f.len() as u32;
    w32(&mut f, 16, len);
    f
}

fn pack_rows(codes: &[u16], n: usize, bits: usize, pack: Pack, be: bool) -> Vec<u8> {
    codes
        .chunks(n)
        .flat_map(|r| pack_row(r, bits, pack, be))
        .collect()
}

/// How a writer packs `comps`-component samples (one-component 10-bit: first datum low).
fn dpx_pack(bits: u8, packing: u16, comps: usize) -> Pack {
    match (bits, packing) {
        (10, 1 | 2) if comps == 1 => Pack::UnitsLow(4, packing == 1),
        (8, _) => Pack::Units(1, true),
        (16, _) => Pack::Units(2, true),
        (_, 0) | (1, _) => Pack::Stream(true),
        (10, 1) => Pack::Units(4, true),
        (10, 2) => Pack::Units(4, false),
        (12, 1) => Pack::Units(2, true),
        (12, 2) => Pack::Units(2, false),
        _ => unreachable!(),
    }
}

#[test]
fn dpx_every_integer_layout() {
    let (w, h) = (7usize, 5usize);
    for be in [true, false] {
        for (bits, packings) in [
            (1u8, &[0u16][..]),
            (8, &[0, 1][..]),
            (10, &[0, 1, 2][..]),
            (12, &[0, 1, 2][..]),
            (16, &[0][..]),
        ] {
            for &packing in packings {
                for (desc, comps) in [(50u8, 3usize), (51, 4), (6, 1), (52, 4)] {
                    let n = w * comps;
                    let codes = pattern(n * h, bits);
                    let data =
                        pack_rows(&codes, n, bits as usize, dpx_pack(bits, packing, comps), be);
                    let f = dpx_file(
                        be,
                        w as u32,
                        h as u32,
                        0,
                        &[elem(desc, bits, packing, data)],
                    );
                    let what = format!("be {be} bits {bits} packing {packing} desc {desc}");
                    assert_eq!(detect(&f), Some(Kind::Dpx));
                    let (cw, ch, cn, got) = dpx::decode_codes(&f, 0).expect(&what);
                    assert_eq!((cw, ch, cn), (w as u32, h as u32, n), "{what}");
                    assert_eq!(got, codes, "{what}");
                    let img = decode(&f, 0).expect(&what);
                    assert!(!img.truncated, "{what}");
                    let i = info(&f, 0).unwrap();
                    assert_eq!(i.bit_depth, bits);
                    assert_eq!(i.has_alpha, comps == 4, "{what}");
                    // Display path: full range, so code c maps to c / max.
                    let max = ((1u32 << bits) - 1) as f64;
                    let px0 = |k: usize| codes[k] as f64 / max;
                    match (&img.pixels, desc) {
                        (Pixels::Gray8(p), 6) => {
                            assert_eq!(p.len(), w * h);
                            assert!(((p[1] as f64 / 255.0) - px0(1)).abs() < 0.003, "{what}");
                        }
                        (Pixels::Gray16(p), 6) => {
                            assert!(((p[1] as f64 / 65535.0) - px0(1)).abs() < 1e-4, "{what}");
                            assert_eq!(
                                i.model,
                                if bits == 1 {
                                    Model::Bilevel
                                } else {
                                    Model::Grey
                                }
                            );
                        }
                        (Pixels::Rgba8(p), 52) => {
                            // ABGR: red is the fourth sample.
                            assert!(((p[0] as f64 / 255.0) - px0(3)).abs() < 0.003, "{what}");
                            assert!(((p[3] as f64 / 255.0) - px0(0)).abs() < 0.003, "{what}");
                        }
                        (Pixels::Rgba16(p), 52) => {
                            assert!(((p[0] as f64 / 65535.0) - px0(3)).abs() < 1e-4, "{what}");
                        }
                        (Pixels::Rgba8(p), _) => {
                            assert!(((p[4] as f64 / 255.0) - px0(comps)).abs() < 0.003, "{what}");
                        }
                        (Pixels::Rgba16(p), _) => {
                            assert!(
                                ((p[4] as f64 / 65535.0) - px0(comps)).abs() < 1e-4,
                                "{what}"
                            );
                        }
                        (p, _) => panic!("{what}: {:?}", std::mem::discriminant(p)),
                    }
                }
            }
        }
    }
}

#[test]
fn dpx_float_and_continuous_rows() {
    let (w, h) = (5usize, 3usize);
    let vals: Vec<f32> = (0..w * h * 3).map(|i| i as f32 / 10.0).collect();
    for be in [true, false] {
        let data: Vec<u8> = vals
            .iter()
            .flat_map(|v| if be { v.to_be_bytes() } else { v.to_le_bytes() })
            .collect();
        let mut e = elem(50, 32, 0, data);
        e.transfer = 2;
        let f = dpx_file(be, w as u32, h as u32, 0, &[e]);
        let Pixels::RgbaF32(p) = decode(&f, 0).unwrap().pixels else {
            panic!()
        };
        assert_eq!(p[4..7], vals[3..6]);
        assert_eq!(p[7], 1.0);
    }
    // 10-bit method A written as one stream with no row padding (7 samples per row: rows
    // would need 3 words each, the stream 7 words in all).
    let codes = pattern(7 * 3, 10);
    let data = pack_row(&codes, 10, Pack::UnitsLow(4, true), true);
    let f = dpx_file(true, 7, 3, 0, &[elem(6, 10, 1, data)]);
    let (_, _, _, got) = dpx::decode_codes(&f, 0).unwrap();
    assert_eq!(got, codes);
    assert!(!decode(&f, 0).unwrap().truncated);
}

#[test]
fn dpx_ycbcr() {
    let (w, h) = (6usize, 2usize);
    // Mid grey (Y 512 in video range 64..940), neutral chroma, and a saturated red pixel.
    for (desc, per2) in [(100u8, 4usize), (102, 6), (103, 8), (101, 6)] {
        let n = w / 2 * per2;
        let mut codes = Vec::new();
        for _ in 0..h {
            for x in 0..w / 2 {
                let (y0, cb, cr) = if x == 0 {
                    (502, 512, 512)
                } else {
                    // BT.709 red: Y 0.2126, Cb -0.1146, Cr 0.5 in video range.
                    (64 + (0.2126f32 * 876.0) as u16, 512 - 103, 960)
                };
                match desc {
                    100 => codes.extend([cb, y0, cr, y0]),
                    101 => codes.extend([cb, y0, 1023, cr, y0, 1023]),
                    102 => codes.extend([cb, y0, cr, cb, y0, cr]),
                    _ => codes.extend([cb, y0, cr, 1023, cb, y0, cr, 1023]),
                }
            }
        }
        let mut e = elem(
            desc,
            10,
            1,
            pack_rows(&codes, n, 10, Pack::Units(4, true), true),
        );
        e.colorimetric = 6;
        let f = dpx_file(true, w as u32, h as u32, 0, &[e]);
        let i = info(&f, 0).unwrap();
        assert_eq!(i.model, Model::YCbCr);
        let Pixels::Rgba16(p) = decode(&f, 0).unwrap().pixels else {
            panic!()
        };
        let g = (502.0 - 64.0) / 876.0 * 65535.0;
        for c in 0..3 {
            assert!(
                (p[c] as f32 - g).abs() < 200.0,
                "desc {desc}: {:?}",
                &p[..4]
            );
        }
        assert_eq!(p[3], 65535);
        let red = &p[2 * 4 * 2..2 * 4 * 2 + 4];
        assert!(
            red[0] > 63000 && red[1] < 2500 && red[2] < 2500,
            "desc {desc}: {red:?}"
        );
    }
}

#[test]
fn dpx_log_to_linear() {
    let codes: Vec<u16> = vec![95, 685, 1023, 445];
    let data = pack_row(&codes, 10, Pack::UnitsLow(4, true), true);
    let mut e = elem(6, 10, 1, data);
    e.transfer = 1;
    let f = dpx_file(true, 4, 1, 0, &[e]);
    let Pixels::RgbaF32(p) = decode(&f, 0).unwrap().pixels else {
        panic!()
    };
    assert!(p[0].abs() < 1e-5);
    assert!((p[4] - 1.0).abs() < 1e-4);
    assert!(p[8] > 13.0 && p[8] < 14.0);
    // 445: 10^((445-685)·0.002/0.6) = 0.158, black offset 0.0108.
    let want = (10f32.powf(-0.8) - 10f32.powf(-1.966_666_6)) / (1.0 - 10f32.powf(-1.966_666_6));
    assert!((p[12] - want).abs() < 1e-4, "{} {want}", p[12]);
    assert!(info(&f, 0).unwrap().coding.contains("printing density"));
}

#[test]
fn dpx_facts_pages_orientation() {
    let codes = pattern(4 * 2 * 3, 8);
    let data = pack_rows(&codes, 12, 8, Pack::Units(1, true), false);
    let f = dpx_file(
        false,
        4,
        2,
        5,
        &[
            elem(50, 8, 1, data.clone()),
            elem(1, 8, 1, data[..4].to_vec()),
        ],
    );
    let i = info(&f, 0).unwrap();
    assert_eq!(i.pages, 2);
    assert_eq!(i.orientation, 6);
    let get = |k: &str| {
        i.summary
            .iter()
            .find(|(s, _)| *s == k)
            .map(|(_, v)| v.clone())
    };
    assert_eq!(get("timecode").as_deref(), Some("01:02:03:04"));
    assert_eq!(get("edge-code").as_deref(), Some("KU 22 123456 7890+12"));
    assert_eq!(get("frame-rate").as_deref(), Some("24 fps"));
    assert_eq!(get("frame").as_deref(), Some("12 / 100"));
    // Format and slate as two values of one fact (the app joins them in its direction).
    let film: Vec<&str> = i
        .summary
        .iter()
        .filter(|(s, _)| *s == "film")
        .map(|(_, v)| v.as_str())
        .collect();
    assert_eq!(film, ["Academy", "Scene 4 take 2"]);
    assert_eq!(get("created").as_deref(), Some("2024-05-06 07:08:09 Z"));
    assert_eq!(get("creator").as_deref(), Some("Test scanner"));
    assert_eq!(get("project").as_deref(), Some("Project X"));
    assert_eq!(get("device").as_deref(), Some("Scanity (SN42)"));
    assert_eq!(get("comment").as_deref(), Some("element 0"));
    assert!(get("copyright").is_none());
    assert!(
        i.fields
            .iter()
            .any(|(k, v)| k == "Slate" && v == "Scene 4 take 2")
    );
    // The second element: red only, as grey.
    let i2 = info(&f, 1).unwrap();
    assert_eq!(i2.model, Model::Grey);
    let img = decode(&f, 1).unwrap();
    assert!(matches!(img.pixels, Pixels::Gray8(_)));
    assert!(img.truncated);
    assert!(info(&f, 2).is_err());
}

#[test]
fn dpx_rle() {
    // Grey 8-bit: a run of 5 × 200, then 3 literals.
    let stream: Vec<u16> = vec![5 << 1 | 1, 200, 3 << 1, 10, 20, 30];
    let data = pack_row(&stream, 8, Pack::Units(1, true), true);
    let mut e = elem(6, 8, 1, data);
    e.encoding = 1;
    let f = dpx_file(true, 4, 2, 0, &[e]);
    let (_, _, _, codes) = dpx::decode_codes(&f, 0).unwrap();
    assert_eq!(codes, [200, 200, 200, 200, 200, 10, 20, 30]);
    let img = decode(&f, 0).unwrap();
    assert!(!img.truncated);
    assert!(info(&f, 0).unwrap().coding.ends_with("RLE"));
}

/// A 16-bit grey RLE element of `w × h` whose repeat runs (4 bytes for 32767 pixels) cover
/// the image, padded to `len` bytes so the payload check lets it through.
fn rle_bomb(w: u32, h: u32, len: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(len);
    while data.len() < len {
        // Flag 0xffff: repeat one pixel 32767 times; the pixel is 0x1234.
        data.extend_from_slice(&[0xff, 0xff, 0x12, 0x34]);
    }
    let mut e = elem(6, 16, 0, data);
    e.encoding = 1;
    dpx_file(true, w, h, 0, &[e])
}

#[test]
fn dpx_rle_reserves_its_expansion() {
    // 4096² 16-bit grey: 32 MiB expanded from 129 KiB.
    let f = rle_bomb(4096, 4096, 129 << 10);
    assert!(f.len() < 140 << 10);
    let r = specular_core::budget::with_limit(16 << 20, || dpx::decode_codes(&f, 0));
    assert!(matches!(r, Err(Error::NeedsMemory(_))), "{r:?}");
    // The expansion (32 MiB) and the unpacked runs (129 KiB) fit 40 MiB; the codes are
    // the test's own.
    let budget = std::sync::Arc::new(specular_core::budget::MemoryBudget::fixed(40 << 20));
    let (_, _, _, codes) =
        specular_core::budget::with_budget(budget.clone(), || dpx::decode_codes(&f, 0)).unwrap();
    assert_eq!(codes.len(), 4096 * 4096);
    assert!(codes.iter().all(|&c| c == 0x1234));
    assert_eq!(budget.reserved(), 0);
}

#[test]
fn dpx_rle_sized_by_its_runs() {
    // Runs for 1000 pixels of a 4096² image: only those are expanded (and reserved).
    let mut stream: Vec<u16> = vec![1001 << 1 | 1, 7];
    // Zero-length literal flags pad the element past the payload check.
    stream.resize(66 << 10, 0);
    let data = pack_row(&stream, 16, Pack::Units(2, true), true);
    let mut e = elem(6, 16, 0, data);
    e.encoding = 1;
    let f = dpx_file(true, 4096, 4096, 0, &[e]);
    let budget = std::sync::Arc::new(specular_core::budget::MemoryBudget::fixed(1 << 20));
    let img = specular_core::budget::with_budget(budget.clone(), || decode(&f, 0)).unwrap();
    assert!(img.truncated);
    assert_eq!(budget.reserved(), 0);
}

#[test]
fn dpx_refusals_and_damage() {
    let data = vec![0u8; 64];
    for (desc, word) in [
        (0u8, "does not say how many channels"),
        (5, "reserved"),
        (7, "without a luma element"),
    ] {
        let f = dpx_file(true, 4, 2, 0, &[elem(desc, 8, 1, data.clone())]);
        match info(&f, 0) {
            Err(Error::Unsupported(s)) => assert!(s.contains(word), "{s}"),
            r => panic!("{desc}: {r:?}"),
        }
    }
    // User-defined channels and composite video are shown.
    for (desc, model, coding) in [
        (150u8, Model::Grey, "first shown as grey"),
        (151, Model::Rgb, "first three shown as RGB"),
        (9, Model::Grey, "not demodulated"),
    ] {
        let f = dpx_file(true, 4, 2, 0, &[elem(desc, 8, 1, data.clone())]);
        let i = info(&f, 0).unwrap();
        assert_eq!(i.model, model);
        assert!(i.coding.contains(coding), "{}", i.coding);
        decode(&f, 0).unwrap();
    }
    let mut e = elem(50, 10, 7, data.clone());
    e.packing = 7;
    let f = dpx_file(true, 4, 2, 0, &[e]);
    assert!(matches!(info(&f, 0), Err(Error::Unsupported(_))));
    // Signed YCbCr.
    let mut f = dpx_file(true, 4, 2, 0, &[elem(102, 8, 1, data.clone())]);
    f[780..784].copy_from_slice(&1u32.to_be_bytes());
    match info(&f, 0) {
        Err(Error::Unsupported(s)) => assert!(s.contains("signed"), "{s}"),
        r => panic!("{r:?}"),
    }
    // Every truncation of a valid file decodes or fails cleanly.
    let codes = pattern(9 * 4 * 3, 10);
    let full = dpx_file(
        true,
        9,
        4,
        0,
        &[elem(
            50,
            10,
            1,
            pack_rows(&codes, 27, 10, Pack::Units(4, true), true),
        )],
    );
    for cut in (0..full.len()).step_by(7) {
        let _ = info(&full[..cut], 0);
        if let Ok(img) = decode(&full[..cut], 0) {
            assert!(img.truncated || cut == full.len());
        }
    }
    // Absurd sizes are refused without allocating.
    let mut big = full.clone();
    big[772..776].copy_from_slice(&60000u32.to_be_bytes());
    big[776..780].copy_from_slice(&60000u32.to_be_bytes());
    assert!(decode(&big, 0).is_err());
}

#[allow(clippy::too_many_arguments)]
/// A Cineon file: `channels` of `bits`, packing and interleave as given.
fn cin_file(
    be: bool,
    w: u32,
    h: u32,
    channels: u8,
    bits: u8,
    packing: u8,
    interleave: u8,
    data: &[u8],
) -> Vec<u8> {
    let mut f = vec![0u8; 2048];
    let w32 = |f: &mut Vec<u8>, o: usize, v: u32| {
        f[o..o + 4].copy_from_slice(&if be { v.to_be_bytes() } else { v.to_le_bytes() })
    };
    w32(&mut f, 0, 0x802a5fd7);
    w32(&mut f, 4, 2048);
    f[24..28].copy_from_slice(b"V4.5");
    f[132..142].copy_from_slice(b"2023:01:02");
    f[144..152].copy_from_slice(b"03:04:05");
    f[193] = channels;
    for c in 0..channels as usize {
        let o = 196 + 28 * c;
        f[o + 1] = if channels == 1 { 0 } else { c as u8 + 1 };
        f[o + 2] = bits;
        w32(&mut f, o + 4, w);
        w32(&mut f, o + 8, h);
        w32(&mut f, o + 12, 0f32.to_bits());
        w32(&mut f, o + 16, 0f32.to_bits());
        w32(&mut f, o + 20, (((1u32 << bits) - 1) as f32).to_bits());
        w32(&mut f, o + 24, 2.048f32.to_bits());
    }
    f[680] = interleave;
    f[681] = packing;
    f[844..852].copy_from_slice(b"Genesis\0");
    w32(&mut f, 1028, 123456);
    w32(&mut f, 1032, 7890);
    f[1024] = 1;
    f[1025] = 22;
    f[1026] = 5;
    w32(&mut f, 1068, 33);
    w32(&mut f, 1072, 24f32.to_bits());
    f.extend_from_slice(data);
    f
}

#[test]
fn cineon_layouts() {
    let (w, h) = (5usize, 3usize);
    for be in [true, false] {
        for bits in [8u8, 10, 12, 16] {
            for packing in 0u8..=6 {
                for channels in [1u8, 3] {
                    for interleave in [0u8, 1, 2] {
                        if bits == 16 && packing == 0 {
                            continue;
                        }
                        let c = channels as usize;
                        let codes = pattern(w * h * c, bits);
                        let pack = match packing {
                            0 => Pack::Stream(false),
                            1 | 2 => Pack::Units(if bits <= 8 { 1 } else { 2 }, packing == 1),
                            3 | 4 => Pack::Units(2, packing == 3),
                            _ => Pack::Units(4, packing == 5),
                        };
                        // Stored rows: pixel-interleaved rows, or one channel's line each.
                        let mut data = Vec::new();
                        let line = |y: usize, ch: usize| -> Vec<u16> {
                            (0..w).map(|x| codes[(y * w + x) * c + ch]).collect()
                        };
                        match interleave {
                            0 => data = pack_rows(&codes, w * c, bits as usize, pack, be),
                            1 => {
                                for y in 0..h {
                                    for ch in 0..c {
                                        data.extend(pack_row(
                                            &line(y, ch),
                                            bits as usize,
                                            pack,
                                            be,
                                        ));
                                    }
                                }
                            }
                            _ => {
                                for ch in 0..c {
                                    for y in 0..h {
                                        data.extend(pack_row(
                                            &line(y, ch),
                                            bits as usize,
                                            pack,
                                            be,
                                        ));
                                    }
                                }
                            }
                        }
                        let f = cin_file(
                            be, w as u32, h as u32, channels, bits, packing, interleave, &data,
                        );
                        let what = format!(
                            "be {be} bits {bits} packing {packing} ch {channels} il {interleave}"
                        );
                        assert_eq!(detect(&f), Some(Kind::Cineon), "{what}");
                        let (_, _, cc, got, _) = cineon::decode_codes(&f).expect(&what);
                        assert_eq!(cc, c);
                        assert_eq!(got, codes, "{what}");
                        let img = decode(&f, 0).expect(&what);
                        assert!(!img.truncated, "{what}");
                        let Pixels::RgbaF32(p) = img.pixels else {
                            panic!()
                        };
                        assert_eq!(p.len(), w * h * 4);
                    }
                }
            }
        }
    }
}

#[test]
fn cineon_log_and_facts() {
    let codes: Vec<u16> = vec![95, 95, 95, 685, 685, 685];
    let data = pack_rows(&codes, 6, 10, Pack::Units(4, true), true);
    let f = cin_file(true, 2, 1, 3, 10, 5, 0, &data);
    let Pixels::RgbaF32(p) = decode(&f, 0).unwrap().pixels else {
        panic!()
    };
    assert!(p[0].abs() < 1e-4, "{}", p[0]);
    assert!((p[4] - 1.0).abs() < 2e-3, "{}", p[4]);
    let i = info(&f, 0).unwrap();
    assert_eq!(i.model, Model::Rgb);
    let get = |k: &str| {
        i.summary
            .iter()
            .find(|(s, _)| *s == k)
            .map(|(_, v)| v.clone())
    };
    assert_eq!(get("created").as_deref(), Some("2023-01-02 03:04:05"));
    assert_eq!(get("edge-code").as_deref(), Some("01 22 123456 7890+05"));
    assert_eq!(get("frame-rate").as_deref(), Some("24 fps"));
    assert_eq!(get("frame").as_deref(), Some("33"));
    assert_eq!(get("device").as_deref(), Some("Genesis"));
    assert_eq!(get("transfer").as_deref(), Some("printing density"));
    let mut bad = f.clone();
    bad[193] = 4;
    assert!(matches!(info(&bad, 0), Err(Error::Unsupported(_))));
    for cut in 0..f.len() {
        let _ = decode(&f[..cut], 0);
    }
}

#[test]
fn committed_fixtures() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut seen = 0;
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        let Some(ext) = p.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if ext != "dpx" && ext != "cin" {
            continue;
        }
        let d = std::fs::read(&p).unwrap();
        let i = info(&d, 0).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let img = decode(&d, 0).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        assert_eq!((img.width, img.height), (i.width, i.height));
        assert!(!img.truncated, "{}", p.display());
        seen += 1;
    }
    assert!(seen >= 3);
}

/// `codes` (`c` per pixel) split into one element per component.
fn planes(codes: &[u16], c: usize) -> Vec<Vec<u16>> {
    (0..c)
        .map(|k| codes.iter().skip(k).step_by(c).copied().collect())
        .collect()
}

#[test]
fn dpx_separate_elements_combine() {
    let (w, h) = (7usize, 3usize);
    for (bits, pack, packing) in [
        (12u8, Pack::Units(2, true), 1u16),
        (16, Pack::Units(2, true), 0),
        (10, Pack::UnitsLow(4, true), 1),
    ] {
        let rgba = pattern(w * h * 4, bits);
        let p = planes(&rgba, 4);
        let one = |d: u8, k: usize| {
            elem(
                d,
                bits,
                packing,
                pack_rows(&p[k], w, bits as usize, pack, true),
            )
        };
        let rgb: Vec<u16> = rgba.chunks(4).flat_map(|px| px[..3].to_vec()).collect();
        let whole = |d: u8, c: &[u16], n: usize| {
            let pk = if bits == 10 {
                Pack::Units(4, true)
            } else {
                pack
            };
            elem(d, bits, packing, pack_rows(c, n, bits as usize, pk, true))
        };
        let (wu, hu) = (w as u32, h as u32);
        // R, G, B: one RGB page.
        let f = dpx_file(true, wu, hu, 0, &[one(1, 0), one(2, 1), one(3, 2)]);
        let i = info(&f, 0).unwrap();
        assert_eq!((i.pages, i.model, i.has_alpha), (1, Model::Rgb, false));
        assert!(
            i.coding.contains("red + green + blue elements"),
            "{}",
            i.coding
        );
        let (_, _, n, codes) = dpx::decode_codes(&f, 0).unwrap();
        assert_eq!((n, &codes), (w * 3, &rgb));
        let want = decode(&dpx_file(true, wu, hu, 0, &[whole(50, &rgb, w * 3)]), 0).unwrap();
        assert_eq!(decode(&f, 0).unwrap(), want, "{bits}-bit RGB");
        // B, A, G, R and a depth element: RGBA, then the depth page.
        let f = dpx_file(
            true,
            wu,
            hu,
            0,
            &[one(3, 2), one(4, 3), one(2, 1), one(1, 0), one(8, 0)],
        );
        let i = info(&f, 0).unwrap();
        assert_eq!((i.pages, i.has_alpha), (2, true));
        assert_eq!(info(&f, 1).unwrap().model, Model::Grey);
        let want = decode(&dpx_file(true, wu, hu, 0, &[whole(51, &rgba, w * 4)]), 0).unwrap();
        assert_eq!(decode(&f, 0).unwrap(), want, "{bits}-bit RGBA");
        // RGB plus an alpha element.
        let f = dpx_file(true, wu, hu, 0, &[whole(50, &rgb, w * 3), one(4, 3)]);
        assert_eq!(info(&f, 0).unwrap().pages, 1);
        assert_eq!(decode(&f, 0).unwrap(), want, "{bits}-bit RGB + A");
    }
    // Depths that differ stay separate grey pages.
    let a = pack_rows(&pattern(w * h, 12), w, 12, Pack::Units(2, true), true);
    let b = pack_rows(&pattern(w * h, 8), w, 8, Pack::Units(1, true), true);
    let f = dpx_file(
        true,
        w as u32,
        h as u32,
        0,
        &[
            elem(1, 12, 1, a.clone()),
            elem(2, 8, 1, b),
            elem(3, 12, 1, a),
        ],
    );
    assert_eq!(info(&f, 0).unwrap().pages, 3);
    assert_eq!(info(&f, 1).unwrap().model, Model::Grey);
}

#[test]
fn dpx_luma_and_colour_difference_combine() {
    let (w, h) = (6usize, 2usize);
    let ycc = pattern(w / 2 * 4 * h, 16);
    // Cb Y0 Cr Y1 → luma Y0 Y1, colour difference Cb Cr.
    let luma: Vec<u16> = ycc.chunks(4).flat_map(|q| [q[1], q[3]]).collect();
    let cbcr: Vec<u16> = ycc.chunks(4).flat_map(|q| [q[0], q[2]]).collect();
    let u = Pack::Units(2, true);
    let mut y = elem(6, 16, 0, pack_rows(&luma, w, 16, u, false));
    let mut c = elem(7, 16, 0, pack_rows(&cbcr, w, 16, u, false));
    let mut whole = elem(100, 16, 0, pack_rows(&ycc, w * 2, 16, u, false));
    for e in [&mut y, &mut c, &mut whole] {
        e.transfer = 6;
        e.colorimetric = 6;
    }
    let f = dpx_file(false, w as u32, h as u32, 0, &[c, y]);
    let i = info(&f, 0).unwrap();
    assert_eq!((i.pages, i.model), (1, Model::YCbCr));
    let want = decode(&dpx_file(false, w as u32, h as u32, 0, &[whole]), 0).unwrap();
    assert_eq!(decode(&f, 0).unwrap(), want);
}

#[test]
fn dpx_signed_samples() {
    // 16-bit grey, linear: −32767, 0, 32767, −32768 (clipped to −1).
    let codes: Vec<u16> = [-32767i16, 0, 32767, -32768]
        .iter()
        .map(|&v| v as u16)
        .collect();
    let mut e = elem(
        6,
        16,
        0,
        pack_rows(&codes, 4, 16, Pack::Units(2, true), true),
    );
    e.transfer = 2;
    let mut f = dpx_file(true, 4, 1, 0, &[e]);
    f[780..784].copy_from_slice(&1u32.to_be_bytes());
    let i = info(&f, 0).unwrap();
    assert!(i.coding.contains("16-bit signed luma"), "{}", i.coding);
    assert_eq!(dpx::decode_codes(&f, 0).unwrap().3, codes);
    let Pixels::RgbaF32(px) = decode(&f, 0).unwrap().pixels else {
        panic!("not float");
    };
    let grey: Vec<f32> = px.chunks(4).map(|p| p[0]).collect();
    assert_eq!(grey, [-1.0, 0.0, 1.0, -1.0]);
    // 8-bit RGB.
    let codes: Vec<u16> = [127i8, -127, 0].iter().map(|&v| v as u8 as u16).collect();
    let mut e = elem(
        50,
        8,
        1,
        pack_rows(&codes, 3, 8, Pack::Units(1, true), true),
    );
    e.transfer = 2;
    let mut f = dpx_file(true, 1, 1, 0, &[e]);
    f[780..784].copy_from_slice(&1u32.to_be_bytes());
    let Pixels::RgbaF32(px) = decode(&f, 0).unwrap().pixels else {
        panic!("not float");
    };
    assert_eq!(px, [1.0, -1.0, 0.0, 1.0]);
}

/// Float pixels run-length encoded: (sample, whether it is a flag).
fn rle_float_stream(px: &[[f32; 3]]) -> Vec<(f32, bool)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < px.len() {
        let mut j = i;
        while j + 1 < px.len() && px[j + 1] == px[i] {
            j += 1;
        }
        if j > i {
            out.push(((((j - i + 1) << 1) | 1) as f32, true));
            i = j + 1;
        } else {
            out.push((2.0, true));
            i += 1;
        }
        out.extend(px[i - 1].iter().map(|&v| (v, false)));
    }
    out
}

#[test]
fn dpx_rle_float() {
    let (w, h) = (9usize, 3usize);
    let px: Vec<[f32; 3]> = (0..w * h)
        .map(|i| {
            let b = (i / 4) as f32 / 8.0;
            [b, 0.5, 1.0 - b]
        })
        .collect();
    let plain: Vec<u8> = px.iter().flatten().flat_map(|v| v.to_be_bytes()).collect();
    let mut e = elem(50, 32, 0, plain);
    e.transfer = 2;
    let want = decode(&dpx_file(true, w as u32, h as u32, 0, &[e]), 0).unwrap();
    for bits_flags in [false, true] {
        let data: Vec<u8> = rle_float_stream(&px)
            .iter()
            .flat_map(|&(v, flag)| {
                // Flags as floats, or as integers in the word's bits.
                let bits = if bits_flags && flag {
                    v as u32
                } else {
                    v.to_bits()
                };
                bits.to_be_bytes()
            })
            .collect();
        let mut e = elem(50, 32, 0, data);
        e.transfer = 2;
        e.encoding = 1;
        let f = dpx_file(true, w as u32, h as u32, 0, &[e]);
        let got = decode(&f, 0).unwrap();
        assert_eq!(got, want, "flags in bits: {bits_flags}");
    }
}

#[test]
fn dpx_user_defined_channels() {
    let (w, h) = (5usize, 2usize);
    let u = Pack::Units(2, true);
    let codes = pattern(w * h * 5, 16);
    let e = elem(153, 16, 0, pack_rows(&codes, w * 5, 16, u, true));
    let f = dpx_file(true, w as u32, h as u32, 0, &[e]);
    let (_, _, n, got) = dpx::decode_codes(&f, 0).unwrap();
    assert_eq!((n, got), (w * 5, codes.clone()));
    let rgb: Vec<u16> = codes.chunks(5).flat_map(|p| p[..3].to_vec()).collect();
    let e = elem(50, 16, 0, pack_rows(&rgb, w * 3, 16, u, true));
    let want = decode(&dpx_file(true, w as u32, h as u32, 0, &[e]), 0).unwrap();
    assert_eq!(decode(&f, 0).unwrap(), want);
}

/// A 5×3 Cineon file of channels (designator, bits) at the given packing and interleave.
fn cin_mixed(
    chans: &[(u8, u8)],
    packing: u8,
    interleave: u8,
    signed: bool,
    data: &[u8],
) -> Vec<u8> {
    let (w, h) = (5u32, 3u32);
    let n = chans.len() as u8;
    let mut f = cin_file(true, w, h, n, chans[0].1, packing, interleave, data);
    for (c, &(d, bits)) in chans.iter().enumerate() {
        let o = 196 + 28 * c;
        let max = ((1u32 << bits) - 1) as f32;
        f[o] = 0;
        f[o + 1] = d;
        f[o + 2] = bits;
        f[o + 4..o + 8].copy_from_slice(&w.to_be_bytes());
        f[o + 8..o + 12].copy_from_slice(&h.to_be_bytes());
        f[o + 20..o + 24].copy_from_slice(&max.to_bits().to_be_bytes());
        f[o + 24..o + 28].copy_from_slice(&2.048f32.to_bits().to_be_bytes());
    }
    f[682] = signed as u8;
    f
}

/// MSB-first fields (value, width), rows padded to 32 bits.
fn msb_fields(rows: &[Vec<(u16, u8)>]) -> Vec<u8> {
    let mut out = Vec::new();
    for r in rows {
        let (mut acc, mut n, mut row) = (0u64, 0u32, Vec::new());
        for &(v, b) in r {
            acc = (acc << b) | v as u64;
            n += b as u32;
            while n >= 8 {
                n -= 8;
                row.push((acc >> n) as u8);
            }
        }
        if n > 0 {
            row.push((acc << (8 - n)) as u8);
        }
        while !row.len().is_multiple_of(4) {
            row.push(0);
        }
        out.extend(row);
    }
    out
}

#[test]
fn cineon_more_channels() {
    let (w, h) = (5usize, 3usize);
    let fields = |codes: &[u16], n: usize| -> Vec<u8> {
        let rows: Vec<Vec<(u16, u8)>> = codes
            .chunks(w * n)
            .map(|r| r.iter().map(|&v| (v, 10)).collect())
            .collect();
        msb_fields(&rows)
    };
    // Four channels (an extra one first): RGB by designator.
    let codes = pattern(w * h * 4, 10);
    let chans = [(0, 10), (1, 10), (2, 10), (3, 10)];
    let f = cin_mixed(&chans, 0, 0, false, &fields(&codes, 4));
    let i = info(&f, 0).unwrap();
    assert_eq!(i.model, Model::Rgb);
    assert!(i.coding.contains("4-channel (RGB shown)"), "{}", i.coding);
    let (_, _, c, got, bits) = cineon::decode_codes(&f).unwrap();
    assert_eq!((c, bits, &got), (4, 10, &codes));
    let rgb: Vec<u16> = codes.chunks(4).flat_map(|p| p[1..].to_vec()).collect();
    let three = cin_mixed(&chans[1..], 0, 0, false, &fields(&rgb, 3));
    assert_eq!(decode(&f, 0).unwrap(), decode(&three, 0).unwrap());
}

#[test]
fn cineon_mixed_depths() {
    let (w, h) = (5usize, 3usize);
    let depths = [10u8, 12, 8];
    let ch: Vec<Vec<u16>> = depths.iter().map(|&b| pattern(w * h, b)).collect();
    let widen = |k: usize, v: u16| {
        let max = ((1u32 << depths[k]) - 1) as f64;
        (v as f64 * 65535.0 / max).round() as u16
    };
    let want: Vec<u16> = (0..w * h)
        .flat_map(|i| (0..3).map(move |k| (k, i)))
        .map(|(k, i)| widen(k, ch[k][i]))
        .collect();
    let chans = [(1, 10), (2, 12), (3, 8)];
    let line = |k: usize, y: usize| -> Vec<(u16, u8)> {
        (0..w).map(|x| (ch[k][y * w + x], depths[k])).collect()
    };
    let px_rows: Vec<Vec<(u16, u8)>> = (0..h)
        .map(|y| {
            (0..w)
                .flat_map(|x| (0..3).map(move |k| (k, x)))
                .map(|(k, x)| (ch[k][y * w + x], depths[k]))
                .collect()
        })
        .collect();
    let line_rows: Vec<Vec<(u16, u8)>> = (0..h)
        .flat_map(|y| (0..3).map(move |k| (k, y)))
        .map(|(k, y)| line(k, y))
        .collect();
    let chan_rows: Vec<Vec<(u16, u8)>> = (0..3)
        .flat_map(|k| (0..h).map(move |y| (k, y)))
        .map(|(k, y)| line(k, y))
        .collect();
    let units: Vec<u8> = line_rows
        .iter()
        .flat_map(|r| {
            // As many samples as fit in each 16-bit unit (two 8-bit ones).
            let per = if r[0].1 <= 8 { 2 } else { 1 };
            let mut b: Vec<u8> = r
                .chunks(per)
                .flat_map(|u| {
                    let bits = u[0].1 as u32;
                    let mut word = 0u32;
                    for (k, &(v, _)) in u.iter().enumerate() {
                        word |= (v as u32) << (16 - bits * (k as u32 + 1));
                    }
                    (word as u16).to_be_bytes()
                })
                .collect();
            while !b.len().is_multiple_of(4) {
                b.push(0);
            }
            b
        })
        .collect();
    for (il, packing, data) in [
        (0u8, 0u8, msb_fields(&px_rows)),
        (1, 0, msb_fields(&line_rows)),
        (2, 0, msb_fields(&chan_rows)),
        (1, 3, units),
    ] {
        let f = cin_mixed(&chans, packing, il, false, &data);
        let i = info(&f, 0).unwrap();
        assert!(i.coding.starts_with("10/12/8-bit RGB"), "{}", i.coding);
        assert_eq!(i.bit_depth, 12);
        let (_, _, _, got, bits) = cineon::decode_codes(&f).unwrap();
        assert_eq!(
            (bits, &got),
            (16, &want),
            "interleave {il} packing {packing}"
        );
        assert!(!decode(&f, 0).unwrap().truncated);
    }
    // Pixel interleave in units with mixed depths is refused.
    let f = cin_mixed(&chans, 3, 0, false, &[0; 64]);
    match info(&f, 0) {
        Err(Error::Unsupported(s)) => assert!(s.contains("tightly packed"), "{s}"),
        r => panic!("{r:?}"),
    }
}

#[test]
fn cineon_signed() {
    let (w, h) = (6usize, 3usize);
    // 0, −100, 300 repeated: zero is code 0, negatives below it.
    let vals = [0i16, -100, 300];
    let rows: Vec<Vec<(u16, u8)>> = (0..h)
        .map(|_| (0..w).map(|x| ((vals[x % 3] as u16) & 0x3ff, 10)).collect())
        .collect();
    let f = cin_mixed(&[(0, 10)], 0, 0, true, &msb_fields(&rows));
    assert!(info(&f, 0).unwrap().coding.contains("signed"));
    let Pixels::RgbaF32(px) = decode(&f, 0).unwrap().pixels else {
        panic!("not float");
    };
    assert!(px[4] < px[0] && px[8] > px[0], "{:?}", &px[..12]);
}
