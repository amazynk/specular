//! Hostile header fields (huge dimensions, offsets and counts) must fail cleanly: no overflow
//! panic in debug builds, no allocation sized from the header alone.

mod common;

use common::{DngCodec, DngSpec, tag_floats, tag_longs, test_pattern, write_dng};
use specular_raw::{DecodeOptions, OutputSize, Rect};

const MAX: u32 = u32::MAX;

/// Parse, estimate and decode whatever parses; errors are fine, panics are not.
fn exercise(file: &[u8]) {
    let _ = specular_raw::detect(file);
    let Ok(raw) = specular_raw::parse(file) else {
        return;
    };
    for size in [OutputSize::Full, OutputSize::Half] {
        let region = Rect {
            x: 0,
            y: 0,
            w: MAX,
            h: MAX,
        };
        let _ = raw.region_memory(region, size);
    }
    let opts = DecodeOptions {
        size: OutputSize::Half,
        max_pixels: 1 << 22,
        ..Default::default()
    };
    if let Ok(img) = raw.decode_raw(file, &opts) {
        let _ = specular_raw::develop(&img, &opts);
    }
}

fn dng(spec: &DngSpec) -> Vec<u8> {
    let cpp = if spec.linear_raw { 3 } else { 1 };
    let (w, h) = (spec.width as usize, spec.height as usize);
    write_dng(spec, &test_pattern(w, h, cpp, spec.bits, 1))
}

// ---------------------------------------------------------------------------------------------
// A minimal TIFF writer for non-DNG containers.

enum V {
    Short(Vec<u16>),
    Long(Vec<u32>),
    Ascii(&'static str),
}

/// An IFD placed at `at` (offsets relative to the TIFF block start), out-of-line values after
/// it, then `next` as the next-IFD offset.
fn ifd(le: bool, at: usize, entries: &[(u16, V)], next: u32) -> Vec<u8> {
    let u16b = |v: u16| if le { v.to_le_bytes() } else { v.to_be_bytes() };
    let u32b = |v: u32| if le { v.to_le_bytes() } else { v.to_be_bytes() };
    let mut head = u16b(entries.len() as u16).to_vec();
    let mut data = Vec::new();
    let data_at = at + 2 + 12 * entries.len() + 4;
    for (tag, v) in entries {
        let (typ, count, bytes): (u16, u32, Vec<u8>) = match v {
            V::Short(s) => (3, s.len() as u32, s.iter().flat_map(|&x| u16b(x)).collect()),
            V::Long(s) => (4, s.len() as u32, s.iter().flat_map(|&x| u32b(x)).collect()),
            V::Ascii(s) => {
                let mut b = s.as_bytes().to_vec();
                b.push(0);
                (2, b.len() as u32, b)
            }
        };
        head.extend_from_slice(&u16b(*tag));
        head.extend_from_slice(&u16b(typ));
        head.extend_from_slice(&u32b(count));
        if bytes.len() <= 4 {
            let mut b = bytes;
            b.resize(4, 0);
            head.extend_from_slice(&b);
        } else {
            head.extend_from_slice(&u32b((data_at + data.len()) as u32));
            data.extend_from_slice(&bytes);
        }
    }
    head.extend_from_slice(&u32b(next));
    head.extend_from_slice(&data);
    head
}

/// A little-endian TIFF with one IFD and `pad` zero bytes of "image data" after it.
fn tiff(magic: u16, entries: &[(u16, V)], pad: usize) -> Vec<u8> {
    let mut out = vec![b'I', b'I'];
    out.extend_from_slice(&magic.to_le_bytes());
    out.extend_from_slice(&8u32.to_le_bytes());
    out.extend_from_slice(&ifd(true, 8, entries, 0));
    out.resize(out.len() + pad, 0);
    out
}

/// A raw IFD of `MAX x MAX` pixels whose single strip is 64 bytes at offset 512.
fn huge_raw_ifd(extra: Vec<(u16, V)>) -> Vec<(u16, V)> {
    let mut e = vec![
        (0x100, V::Long(vec![MAX])),
        (0x101, V::Long(vec![MAX])),
        (0x102, V::Short(vec![12])),
        (0x103, V::Short(vec![2])),
        (0x111, V::Long(vec![512])),
        (0x117, V::Long(vec![64])),
    ];
    e.extend(extra);
    e.sort_by_key(|(t, _)| *t);
    e
}

// ---------------------------------------------------------------------------------------------
// DNG

#[test]
fn dng_cfa_repeat_dim_overflow() {
    // 65536 x 65536 overflows the u32 pattern size.
    let mut s = DngSpec::new(16, 16, DngCodec::U16);
    s.extra = vec![tag_longs(0x828d, &[65536, 65536])];
    exercise(&dng(&s));
}

#[test]
fn dng_default_crop_overflow() {
    // Origin 4e9 + size 1e9 overflows u32.
    let mut s = DngSpec::new(16, 16, DngCodec::U16);
    s.extra = vec![
        tag_floats(0xc61f, &[4e9, 0.0]),
        tag_floats(0xc620, &[1e9, 1.0]),
    ];
    exercise(&dng(&s));
}

#[test]
fn dng_black_level_repeat_dim_overflow() {
    // MAX x MAX x 3 black levels overflow usize.
    let mut s = DngSpec::new(8, 8, DngCodec::U16);
    s.linear_raw = true;
    s.black = 16;
    s.extra = vec![tag_longs(0xc619, &[MAX, MAX])];
    exercise(&dng(&s));
}

#[test]
fn dng_lj92_huge_tile_size() {
    // TileWidth/TileLength u32::MAX on a 32x32 image: per-tile row lists sized from the
    // header (64 GB) and the frame-size check overflowed.
    let mut s = DngSpec::new(32, 32, DngCodec::Lj92Tiles(16));
    s.extra = vec![tag_longs(0x142, &[MAX]), tag_longs(0x143, &[MAX])];
    exercise(&dng(&s));
}

#[test]
fn dng_lj92_frame_larger_than_data() {
    // A predictor-2 tile whose SOF3 claims 65535x65535x2 samples: 17 GB of zeros before.
    let mut s = DngSpec::new(
        32,
        32,
        DngCodec::Lj92TilesFrame {
            tile: 16,
            wide: false,
            predictor: 2,
            restart: 0,
        },
    );
    s.extra = vec![tag_longs(0x142, &[MAX]), tag_longs(0x143, &[MAX])];
    let mut f = dng(&s);
    let mut patched = 0;
    for i in 0..f.len() - 9 {
        // SOF3: precision 16, height 16, width 8 (two components of an 16x16 tile).
        if f[i..i + 2] == [0xff, 0xc3] && f[i + 4..i + 9] == [16, 0, 16, 0, 8] {
            f[i + 5..i + 9].copy_from_slice(&[0xff; 4]);
            patched += 1;
        }
    }
    assert_eq!(patched, 4);
    let raw = specular_raw::parse(&f).unwrap();
    let opts = DecodeOptions {
        size: OutputSize::Full,
        ..Default::default()
    };
    // Every tile is refused, so the image decodes as truncated (or fails), quickly.
    if let Ok(img) = raw.decode_raw(&f, &opts) {
        assert!(img.truncated);
    }
}

// ---------------------------------------------------------------------------------------------
// Other TIFF-based containers

#[test]
fn arw_huge_dims() {
    // MAX x MAX pixels: `px * 2` overflowed u64.
    let e = huge_raw_ifd(vec![(0x10f, V::Ascii("SONY"))]);
    exercise(&tiff(42, &e, 1024));
}

#[test]
fn orf_huge_dims() {
    let e = huge_raw_ifd(vec![]);
    exercise(&tiff(0x4f52, &e, 1024));
}

#[test]
fn rgb_preview_huge_dims() {
    // An uncompressed RGB8 IFD of MAX x MAX: the preview size check overflowed usize.
    let e = vec![
        (0x100, V::Long(vec![MAX])),
        (0x101, V::Long(vec![MAX])),
        (0x102, V::Short(vec![8, 8, 8])),
        (0x103, V::Short(vec![1])),
        (0x111, V::Long(vec![512])),
        (0x115, V::Short(vec![3])),
        (0x117, V::Long(vec![64])),
    ];
    let f = tiff(0x4f52, &e, 1024);
    if let Ok(raw) = specular_raw::parse(&f) {
        assert!(raw.info().previews.is_empty());
    }
    exercise(&f);
}

// ---------------------------------------------------------------------------------------------
// Fujifilm RAF

#[test]
fn raf_huge_dims() {
    // Raw directory 0xF001/0xF002 = MAX x MAX: `len >= px * 2` overflowed usize.
    let mut f = vec![0u8; 1024];
    f[..16].copy_from_slice(b"FUJIFILMCCD-RAW ");
    f[28..32].copy_from_slice(b"X-T1");
    let be = |f: &mut Vec<u8>, at: usize, v: u32| f[at..at + 4].copy_from_slice(&v.to_be_bytes());
    be(&mut f, 92, 120); // CFA header directory
    be(&mut f, 96, 1);
    be(&mut f, 100, 200); // raw directory base
    // Directory: one record, 0x100 = height 16, width 16.
    be(&mut f, 120, 1);
    f[124..132].copy_from_slice(&[0x01, 0x00, 0, 4, 0, 16, 0, 16]);
    // Raw directory: a big-endian TIFF whose root IFD points (0xF000) at the Fuji IFD.
    let mut t = b"MM\0\x2a".to_vec();
    t.extend_from_slice(&8u32.to_be_bytes());
    t.extend_from_slice(&ifd(false, 8, &[(0xf000, V::Long(vec![26]))], 0));
    let fuji = [
        (0xf001, V::Long(vec![MAX])),
        (0xf002, V::Long(vec![MAX])),
        (0xf003, V::Long(vec![14])),
        (0xf007, V::Long(vec![400])),
        (0xf008, V::Long(vec![64])),
    ];
    t.extend_from_slice(&ifd(false, 26, &fuji, 0));
    f[200..200 + t.len()].copy_from_slice(&t);
    exercise(&f);
}

// ---------------------------------------------------------------------------------------------
// Canon CR3

fn bx(typ: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut b = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
    b.extend_from_slice(typ);
    b.extend_from_slice(payload);
    b
}

/// A CR3 with a maker-note sensor area and one CRAW track whose first sample is at `offset`
/// and whose CMP1 header claims `width` x 2.
fn cr3(offset: u64, width: u32) -> Vec<u8> {
    const CANON_UUID: [u8; 16] = [
        0x85, 0xc0, 0xb6, 0x87, 0x82, 0x0f, 0x11, 0xe0, 0x81, 0x11, 0xf4, 0xce, 0x46, 0x2b, 0x6a,
        0x48,
    ];
    // CMT3: a maker note with SensorInfo (0x00e0).
    let mut cmt3 = b"II\x2a\0".to_vec();
    cmt3.extend_from_slice(&8u32.to_le_bytes());
    cmt3.extend_from_slice(&ifd(
        true,
        8,
        &[(0x00e0, V::Long(vec![0, 100, 100, 0, 0, 0, 0, 10, 10]))],
        0,
    ));
    let mut canon = ((8 + 16 + cmt3.len() + 8) as u32).to_be_bytes().to_vec();
    canon.extend_from_slice(b"uuid");
    canon.extend_from_slice(&CANON_UUID);
    canon.extend_from_slice(&bx(b"CMT3", &cmt3));
    // CMP1: version 0x100, width x 2, tiles 2x2, 14 bits, 4 planes.
    let mut cmp1 = vec![0u8; 64];
    cmp1[4..6].copy_from_slice(&0x100u16.to_be_bytes());
    cmp1[8..12].copy_from_slice(&width.to_be_bytes());
    cmp1[12..16].copy_from_slice(&2u32.to_be_bytes());
    cmp1[16..20].copy_from_slice(&2u32.to_be_bytes());
    cmp1[20..24].copy_from_slice(&2u32.to_be_bytes());
    cmp1[24] = 14;
    cmp1[25] = 0x40;
    cmp1[28..32].copy_from_slice(&1u32.to_be_bytes());
    let mut craw = vec![0u8; 82];
    craw.extend_from_slice(&bx(b"CMP1", &cmp1));
    let mut stsd = vec![0, 0, 0, 0, 0, 0, 0, 1];
    stsd.extend_from_slice(&bx(b"CRAW", &craw));
    let mut stsz = vec![0u8; 4];
    stsz.extend_from_slice(&16u32.to_be_bytes());
    let mut co64 = vec![0, 0, 0, 0, 0, 0, 0, 1];
    co64.extend_from_slice(&offset.to_be_bytes());
    let stbl = [bx(b"stsd", &stsd), bx(b"stsz", &stsz), bx(b"co64", &co64)].concat();
    let trak = bx(b"trak", &bx(b"mdia", &bx(b"minf", &bx(b"stbl", &stbl))));
    let mut f = bx(b"ftyp", b"crx \0\0\0\x01crx ");
    f.extend_from_slice(&bx(b"moov", &[canon, trak].concat()));
    f
}

#[test]
fn cr3_sample_offset_u64_max() {
    // co64 offset u64::MAX: `off + 2` overflowed.
    exercise(&cr3(u64::MAX, 64));
}

#[test]
fn tiles_smaller_than_16_are_refused() {
    // 2 × 2 tiles: a tile record (and its row list) per four sensor pixels. TIFF tiles are
    // multiples of 16.
    let s = DngSpec::new(32, 32, DngCodec::Lj92Tiles(2));
    let file = dng(&s);
    let raw = specular_raw::parse(&file).unwrap();
    assert!(!raw.info().sensor.as_ref().is_some_and(|s| s.decodable));
    exercise(&file);
}

#[test]
fn cr3_cmp1_width_near_u32_max() {
    // CMP1 width 0xfffffffe: `width + 64` overflowed against the maker-note sensor area.
    exercise(&cr3(0, 0xffff_fffe));
}
