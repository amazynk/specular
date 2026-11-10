//! Hostile headers: tiny synthetic files whose fields ask for huge allocations, overflow
//! arithmetic or point past the end. They must fail cleanly (or decode something sane) without
//! panicking in a debug build.

use specular_tiff::science::Stk;
use specular_tiff::{DecodeOptions, Directories, Tiff};

/// One entry: tag, type, then either values (inline or out of line) or a raw value field.
#[derive(Clone)]
enum Val {
    Vals(Vec<u64>),
    /// The value field as is (an offset the file does not back).
    At(u64),
}

fn type_size(typ: u16) -> usize {
    match typ {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 => 4,
        _ => 8,
    }
}

/// A little-endian TIFF (BigTIFF if `big`) with a chain of `ifds`, each written with its
/// out-of-line values right after it.
fn tiff(big: bool, ifds: &[Vec<(u16, u16, u64, Val)>]) -> Vec<u8> {
    let (head, cnt_len, entry_len, ptr_len) = if big { (16, 8, 20, 8) } else { (8, 2, 12, 4) };
    let inline = if big { 8 } else { 4 };
    let put = |out: &mut Vec<u8>, v: u64, n: usize| out.extend_from_slice(&v.to_le_bytes()[..n]);
    let mut out = if big {
        let mut h = b"II\x2B\x00\x08\x00\x00\x00".to_vec();
        h.extend_from_slice(&16u64.to_le_bytes());
        h
    } else {
        b"II\x2A\x00\x08\x00\x00\x00".to_vec()
    };
    assert_eq!(out.len(), head);
    for (k, entries) in ifds.iter().enumerate() {
        let start = out.len();
        let ifd_len = cnt_len + entries.len() * entry_len + ptr_len;
        let mut extra = Vec::new();
        put(&mut out, entries.len() as u64, cnt_len);
        for (tag, typ, count, val) in entries {
            put(&mut out, u64::from(*tag), 2);
            put(&mut out, u64::from(*typ), 2);
            put(&mut out, *count, ptr_len);
            let mut field = Vec::new();
            match val {
                Val::At(o) => put(&mut field, *o, inline),
                Val::Vals(v) => {
                    let sz = type_size(*typ);
                    let mut bytes = Vec::new();
                    for &x in v {
                        put(&mut bytes, x, sz);
                    }
                    if bytes.len() <= inline {
                        bytes.resize(inline, 0);
                        field = bytes;
                    } else {
                        put(&mut field, (start + ifd_len + extra.len()) as u64, inline);
                        extra.extend_from_slice(&bytes);
                        extra.resize(extra.len().next_multiple_of(2), 0);
                    }
                }
            }
            out.extend_from_slice(&field);
        }
        let next = if k + 1 < ifds.len() {
            (start + ifd_len + extra.len()) as u64
        } else {
            0
        };
        put(&mut out, next, ptr_len);
        out.extend_from_slice(&extra);
    }
    out
}

fn v(x: &[u64]) -> Val {
    Val::Vals(x.to_vec())
}

#[test]
fn tiny_ifds_making_millions_of_virtual_strips_are_bounded() {
    // 131072 × 4194304, 4 planar 8-bit planes in one strip each: every IFD (~200 bytes) would
    // become 2^21 virtual sub-strips (32 MB of offsets and counts). Many of them stop being
    // split once the file's value budget is spent.
    let ifd = vec![
        (256, 4, 1, v(&[131072])),
        (257, 4, 1, v(&[1 << 22])),
        (258, 3, 1, v(&[8])),
        (262, 3, 1, v(&[1])),
        (273, 4, 4, v(&[8, 8, 8, 8])),
        (277, 3, 1, v(&[4])),
        (279, 4, 4, v(&[1 << 30; 4])),
        (284, 3, 1, v(&[2])),
    ];
    let file = tiff(false, &vec![ifd; 40]);
    let t = Tiff::open(&file).unwrap();
    assert_eq!(t.pages().len(), 40);
    let first = t.pages()[0].levels()[0].unit_h;
    let last = t.pages()[39].levels()[0].unit_h;
    assert_eq!((first, last), (8, 1 << 22));
}

#[test]
fn many_ifds_sharing_one_offsets_array_are_bounded() {
    // Every IFD points at the same 2^20-value BYTE offsets array: the file's value budget,
    // not the IFD count, bounds the work.
    let n = 1u64 << 20;
    let mut file = tiff(false, &[]);
    file.truncate(8);
    let arr = file.len() as u64;
    file.resize(file.len() + n as usize, 8);
    let first = file.len() as u64;
    file[4..8].copy_from_slice(&(first as u32).to_le_bytes());
    let count = 600u64;
    for k in 0..count {
        let e: [(u16, u16, u32, u32); 6] = [
            (256, 4, 1, 1),
            (257, 4, 1, 1 << 20),
            (258, 3, 1, 8),
            (273, 1, n as u32, arr as u32),
            (278, 4, 1, 1),
            (279, 1, n as u32, arr as u32),
        ];
        file.extend_from_slice(&(e.len() as u16).to_le_bytes());
        for (tag, typ, c, val) in e {
            file.extend_from_slice(&tag.to_le_bytes());
            file.extend_from_slice(&typ.to_le_bytes());
            file.extend_from_slice(&c.to_le_bytes());
            file.extend_from_slice(&val.to_le_bytes());
        }
        let next = if k + 1 < count {
            file.len() as u32 + 4
        } else {
            0
        };
        file.extend_from_slice(&next.to_le_bytes());
    }
    // Some IFDs open, the rest are refused past the budget; nothing holds 600 × 2^20 values.
    let t = Tiff::open(&file).unwrap();
    assert!(t.pages().len() < 100, "{}", t.pages().len());
}

#[test]
fn planar_strip_counts_near_u64_max_do_not_overflow() {
    // Old-ImageJ planar layout check sums BigTIFF byte counts of u64::MAX.
    let ifd = vec![
        (256, 4, 1, v(&[4])),
        (257, 4, 1, v(&[2])),
        (258, 3, 1, v(&[8])),
        (262, 3, 1, v(&[1])),
        (273, 16, 2, v(&[64, 64])),
        (277, 3, 1, v(&[3])),
        (278, 4, 1, v(&[1])),
        (279, 16, 2, v(&[u64::MAX, u64::MAX])),
        (284, 3, 1, v(&[2])),
    ];
    let file = tiff(true, &[ifd]);
    let _ = Tiff::open(&file);
}

#[test]
fn wide_odd_strip_at_a_large_scale_does_not_overflow() {
    // Strip width 4194303 (odd) at 1/2048: the decode geometry's 2048 × 4194303 overflowed u32.
    let ifd = vec![
        (256, 4, 1, v(&[(1 << 22) - 1])),
        (257, 4, 1, v(&[1])),
        (258, 3, 1, v(&[8])),
        (262, 3, 1, v(&[1])),
        (273, 4, 1, v(&[8])),
        (279, 4, 1, v(&[(1 << 22) - 1])),
    ];
    let file = tiff(false, &[ifd]);
    let t = Tiff::open(&file).unwrap();
    let page = &t.pages()[0];
    let opts = DecodeOptions {
        scale: 2048,
        parallel: false,
        ..Default::default()
    };
    let _ = page.memory_estimate(&opts);
    let d = page.decode(&file, true, &opts).unwrap();
    assert_eq!((d.width, d.height), (2048, 1));
}

#[test]
fn old_jpeg_table_offsets_near_u64_max_do_not_overflow() {
    // OJPEG quantisation and Huffman table offsets as LONG8 u64::MAX.
    let ifd = vec![
        (256, 4, 1, v(&[8])),
        (257, 4, 1, v(&[8])),
        (258, 3, 1, v(&[8])),
        (259, 3, 1, v(&[6])),
        (262, 3, 1, v(&[1])),
        (273, 16, 1, v(&[16])),
        (277, 3, 1, v(&[1])),
        (279, 16, 1, v(&[16])),
        (519, 16, 1, v(&[u64::MAX])),
        (520, 16, 1, v(&[u64::MAX])),
        (521, 16, 1, v(&[u64::MAX])),
    ];
    let file = tiff(true, &[ifd]);
    if let Ok(t) = Tiff::open(&file) {
        let _ = t.pages()[0].decode(&file, true, &DecodeOptions::default());
    }
}

#[test]
fn stk_uic_offsets_near_u64_max_do_not_overflow() {
    // MetaMorph UIC2/UIC3/UIC1 values at offsets near u64::MAX (BigTIFF).
    let ifd = vec![
        (256, 4, 1, v(&[1])),
        (257, 4, 1, v(&[1])),
        (273, 4, 1, v(&[0])),
        (279, 4, 1, v(&[1])),
        (33628, 4, 3, Val::At(u64::MAX - 1)),
        (33629, 5, 3, Val::At(u64::MAX - 1)),
        (33630, 5, 3, Val::At(u64::MAX - 1)),
    ];
    let file = tiff(true, &[ifd]);
    let dirs = Directories::read(&file).unwrap();
    let _ = Stk::read(&dirs, &file, 0);
}

#[test]
fn jpeg_tile_claiming_a_huge_frame_is_not_decoded() {
    // A 32 × 32 image of 16 × 16 JPEG tiles whose one stored JPEG says 65535 × 65535 in its
    // frame header: decoded as it says, every tile would ask for 4 GiB.
    let mut jpeg = Vec::new();
    jpeg_encoder::Encoder::new(&mut jpeg, 90)
        .encode(&[128u8; 256], 16, 16, jpeg_encoder::ColorType::Luma)
        .unwrap();
    let sof = jpeg.windows(2).position(|w| w == [0xff, 0xc0]).unwrap();
    jpeg[sof + 5..sof + 9].copy_from_slice(&[0xff, 0xff, 0xff, 0xff]);
    let ifd = |at: u64| {
        vec![
            (256, 4, 1, v(&[32])),
            (257, 4, 1, v(&[32])),
            (258, 3, 1, v(&[8])),
            (259, 3, 1, v(&[7])),
            (262, 3, 1, v(&[1])),
            (277, 3, 1, v(&[1])),
            (322, 4, 1, v(&[16])),
            (323, 4, 1, v(&[16])),
            (324, 4, 4, v(&[at; 4])),
            (325, 4, 4, v(&[jpeg.len() as u64; 4])),
        ]
    };
    let at = tiff(false, &[ifd(0)]).len() as u64;
    let mut file = tiff(false, &[ifd(at)]);
    file.extend_from_slice(&jpeg);
    let t = Tiff::open(&file).unwrap();
    let opts = DecodeOptions {
        parallel: false,
        ..Default::default()
    };
    if let Ok(d) = t.pages()[0].decode(&file, true, &opts) {
        assert_eq!((d.width, d.height), (32, 32));
    }
}
