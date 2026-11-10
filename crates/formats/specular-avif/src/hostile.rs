//! Regression tests for hostile containers: none of these may panic (debug builds check
//! overflow), allocate from unchecked header fields or take quadratic time.

use std::time::{Duration, Instant};

use crate::Error;
use crate::backend::{Cicp, Picture, Planes};
use crate::convert::{Conv, Dst, Tone};
use crate::heif::{Grid, Meta};
use crate::seq::Sequence;

fn bx(typ: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut v = u32::try_from(payload.len() + 8)
        .unwrap()
        .to_be_bytes()
        .to_vec();
    v.extend_from_slice(typ);
    v.extend_from_slice(payload);
    v
}

fn full(v: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![v, 0, 0, 0];
    out.extend_from_slice(payload);
    out
}

fn hdlr_pict() -> Vec<u8> {
    bx(b"hdlr", &full(0, &[0, 0, 0, 0, b'p', b'i', b'c', b't']))
}

/// `iinf` (v0) with version-2 `infe`s for `ids`, all of type `typ`.
fn iinf(ids: impl Iterator<Item = u16>, typ: &[u8; 4]) -> Vec<u8> {
    let mut p = full(0, &[0, 0]);
    for id in ids {
        let mut e = id.to_be_bytes().to_vec();
        e.extend_from_slice(&[0, 0]);
        e.extend_from_slice(typ);
        p.extend(bx(b"infe", &full(2, &e)));
    }
    bx(b"iinf", &p)
}

/// A `meta` payload of the given child boxes.
fn meta(children: &[Vec<u8>]) -> Vec<u8> {
    let mut p = full(0, &[]);
    for c in children {
        p.extend_from_slice(c);
    }
    p
}

#[test]
fn iloc_zero_byte_extents_are_capped() {
    // Offset and length fields of 0 bytes: every extent is free in the file but costs 16
    // bytes in memory; 17 entries of 65535 extents each go past the cap.
    let mut p = full(0, &[0x00, 0x00]);
    p.extend_from_slice(&17u16.to_be_bytes());
    for id in 1..=17u16 {
        p.extend_from_slice(&id.to_be_bytes());
        p.extend_from_slice(&[0, 0]);
        p.extend_from_slice(&u16::MAX.to_be_bytes());
    }
    let m = meta(&[hdlr_pict(), bx(b"iloc", &p)]);
    assert!(matches!(Meta::parse(&m), Err(Error::Unsupported(_))));
}

/// `iloc` v0 with 4-byte offsets and 8-byte lengths: item 1 with `extents` (offset, length).
fn iloc_item1(extents: &[(u32, u64)]) -> Vec<u8> {
    let mut p = full(0, &[0x48, 0x00]);
    p.extend_from_slice(&1u16.to_be_bytes());
    p.extend_from_slice(&1u16.to_be_bytes());
    p.extend_from_slice(&[0, 0]);
    p.extend_from_slice(&u16::try_from(extents.len()).unwrap().to_be_bytes());
    for &(o, l) in extents {
        p.extend_from_slice(&o.to_be_bytes());
        p.extend_from_slice(&l.to_be_bytes());
    }
    bx(b"iloc", &p)
}

#[test]
fn extent_length_overflow_is_an_error() {
    let m = meta(&[
        hdlr_pict(),
        iinf(std::iter::once(1), b"av01"),
        iloc_item1(&[(10, u64::MAX)]),
    ]);
    let m = Meta::parse(&m).unwrap();
    let file = [0u8; 64];
    assert!(matches!(m.item_data(&file, 1), Err(Error::Truncated)));
}

#[test]
fn repeated_whole_file_extents_are_rejected() {
    // Each extent of length 0 is "the rest of the file": 65535 of them would copy the file
    // 65535 times.
    let m = meta(&[
        hdlr_pict(),
        iinf(std::iter::once(1), b"av01"),
        iloc_item1(&[(0, 0); 1000]),
    ]);
    let m = Meta::parse(&m).unwrap();
    let file = vec![0u8; 4096];
    assert!(matches!(m.item_data(&file, 1), Err(Error::Malformed(_))));
    // Disjoint extents still concatenate.
    let m = meta(&[
        hdlr_pict(),
        iinf(std::iter::once(1), b"av01"),
        iloc_item1(&[(4, 2), (0, 2)]),
    ]);
    let m = Meta::parse(&m).unwrap();
    assert_eq!(
        &*m.item_data(&[1, 2, 3, 4, 5, 6], 1).unwrap(),
        &[5, 6, 1, 2]
    );
}

#[test]
fn top_level_is_not_quadratic() {
    // 30000 items, all inputs of one derived item: a per-item scan of the references costs
    // 9e8 comparisons.
    let n = 30_000u16;
    let mut dimg = 1u16.to_be_bytes().to_vec();
    dimg.extend_from_slice(&n.to_be_bytes());
    for id in 1..=n {
        dimg.extend_from_slice(&id.to_be_bytes());
    }
    let iref = bx(b"iref", &full(0, &bx(b"dimg", &dimg)));
    let pitm = bx(b"pitm", &full(0, &1u16.to_be_bytes()));
    let m = meta(&[hdlr_pict(), pitm, iinf(1..=n, b"av01"), iref]);
    let m = Meta::parse(&m).unwrap();
    let t = Instant::now();
    assert_eq!(m.top_level(), vec![1]);
    assert!(t.elapsed() < Duration::from_secs(5));
}

/// A `moov` with one AV1 track whose sample table holds `stbl` (child boxes).
fn moov(stbl: &[Vec<u8>]) -> Vec<u8> {
    let mut av01 = vec![0u8; 78];
    av01.extend(bx(b"av1C", &[0x81, 0, 0, 0]));
    let mut stsd = full(0, &1u32.to_be_bytes());
    stsd.extend(bx(b"av01", &av01));
    let mut st = bx(b"stsd", &stsd);
    for b in stbl {
        st.extend_from_slice(b);
    }
    let minf = bx(b"minf", &bx(b"stbl", &st));
    let mut mdia = bx(b"hdlr", &full(0, &[0, 0, 0, 0, b'p', b'i', b'c', b't']));
    mdia.extend(minf);
    bx(b"trak", &bx(b"mdia", &mdia))
}

fn stsz(sizes: &[u32]) -> Vec<u8> {
    let mut p = full(0, &0u32.to_be_bytes());
    p.extend_from_slice(&u32::try_from(sizes.len()).unwrap().to_be_bytes());
    for s in sizes {
        p.extend_from_slice(&s.to_be_bytes());
    }
    bx(b"stsz", &p)
}

fn stco(offsets: &[u32]) -> Vec<u8> {
    let mut p = full(0, &u32::try_from(offsets.len()).unwrap().to_be_bytes());
    for o in offsets {
        p.extend_from_slice(&o.to_be_bytes());
    }
    bx(b"stco", &p)
}

fn stsc(runs: &[(u32, u32)]) -> Vec<u8> {
    let mut p = full(0, &u32::try_from(runs.len()).unwrap().to_be_bytes());
    for &(first, per) in runs {
        p.extend_from_slice(&first.to_be_bytes());
        p.extend_from_slice(&per.to_be_bytes());
        p.extend_from_slice(&1u32.to_be_bytes());
    }
    bx(b"stsc", &p)
}

#[test]
fn sample_to_chunk_runs_are_walked_not_searched() {
    // 200k empty chunks against 100k runs: a search per chunk costs 2e10 steps.
    let chunks = vec![0u32; 200_000];
    let runs: Vec<(u32, u32)> = (1..=100_000).map(|i| (i, 0)).collect();
    let m = moov(&[stsz(&[1]), stco(&chunks), stsc(&runs)]);
    let t = Instant::now();
    let s = Sequence::parse(&m).map(|s| s.color.samples.len());
    assert!(matches!(s, Err(Error::Malformed(_)) | Ok(0)));
    assert!(t.elapsed() < Duration::from_secs(5));
    // Valid tables lay samples out as before.
    let m = moov(&[
        stsz(&[1, 2, 3, 4, 5]),
        stco(&[100, 200, 300]),
        stsc(&[(1, 2), (3, 3)]),
    ]);
    let s = Sequence::parse(&m).unwrap();
    let offs: Vec<u64> = s.color.samples.iter().map(|s| s.offset).collect();
    assert_eq!(offs, [100, 101, 200, 203, 300]);
}

struct Tall {
    y: Vec<u8>,
    c: Vec<u8>,
    w: u32,
    h: u32,
}

impl Picture for Tall {
    fn planes(&self) -> Planes<'_> {
        let cs = (self.w as usize).div_ceil(2);
        Planes {
            planes: [&self.y, &self.c, &self.c],
            strides: [self.w as usize, cs, cs],
            width: self.w,
            height: self.h,
            bits: 8,
            hbd: false,
            ssx: true,
            ssy: true,
            cosited_x: false,
            cosited_y: false,
            mono: false,
            cicp: Cicp {
                primaries: 1,
                transfer: 1,
                matrix: 1,
                full_range: true,
            },
        }
    }
}

fn tall(w: u32, h: u32) -> Box<dyn Picture> {
    let (w_, h_) = (w as usize, h as usize);
    Box::new(Tall {
        y: vec![128; w_ * h_],
        c: vec![128; w_.div_ceil(2) * h_.div_ceil(2)],
        w,
        h,
    })
}

#[test]
fn grid_bands_are_sized_by_the_canvas() {
    // 256 tiles of 2×4M samples over a 512×2 canvas: bands sized from the tile height would
    // take 3 GB each.
    let (tw, th) = (2u32, 1u32 << 22);
    let grid = Grid {
        rows: 1,
        cols: 256,
        width: 512,
        height: 2,
    };
    let mut decode = |_: usize| -> Result<Box<dyn Picture>, Error> { Ok(tall(tw, th)) };
    let conv_for = |_: &Planes<'_>| Conv::new(1, true, 1, 8, Tone::Sdr);
    let mut buf = vec![0u8; 512 * 2 * 3];
    let mut dst = Dst {
        data: &mut buf,
        width: 512,
        height: 2,
        channels: 3,
        wide: false,
        x: 0,
        y: 0,
    };
    crate::grid::render_stitched(
        &grid,
        tall(tw, th),
        &mut decode,
        &conv_for,
        1,
        &mut dst,
        false,
        &|| false,
    )
    .unwrap();
    assert!(buf.iter().all(|&v| v > 100 && v < 160));
}

/// A file of `n` top-level grid items (ids 1..=n), each of one tile (item n + 1) and stored
/// in two extents: the 8-byte grid descriptor and `pad` bytes after it.
fn many_grids(n: u16, pad: u32) -> Vec<u8> {
    let ftyp = bx(b"ftyp", b"avifavifmif1");
    let build = |at: u32| {
        let mut iloc = full(0, &[0x44, 0x00]);
        iloc.extend_from_slice(&(n + 1).to_be_bytes());
        for id in 1..=n {
            iloc.extend_from_slice(&id.to_be_bytes());
            iloc.extend_from_slice(&[0, 0, 0, 2]);
            for (o, l) in [(at, 8u32), (at + 8, pad)] {
                iloc.extend_from_slice(&o.to_be_bytes());
                iloc.extend_from_slice(&l.to_be_bytes());
            }
        }
        iloc.extend_from_slice(&(n + 1).to_be_bytes());
        iloc.extend_from_slice(&[0, 0, 0, 1]);
        iloc.extend_from_slice(&at.to_be_bytes());
        iloc.extend_from_slice(&8u32.to_be_bytes());
        let mut iref = full(0, &[]);
        for id in 1..=n {
            let mut d = id.to_be_bytes().to_vec();
            d.extend_from_slice(&1u16.to_be_bytes());
            d.extend_from_slice(&(n + 1).to_be_bytes());
            iref.extend(bx(b"dimg", &d));
        }
        // The tile: a further `infe` appended to the `iinf` payload, with the box size fixed.
        let mut infos = iinf(1..=n, b"grid");
        let mut e = (n + 1).to_be_bytes().to_vec();
        e.extend_from_slice(&[0, 0]);
        e.extend_from_slice(b"av01");
        infos.extend(bx(b"infe", &full(2, &e)));
        let size = u32::try_from(infos.len()).unwrap().to_be_bytes();
        infos[..4].copy_from_slice(&size);
        let pitm = bx(b"pitm", &full(0, &1u16.to_be_bytes()));
        bx(
            b"meta",
            &meta(&[
                hdlr_pict(),
                pitm,
                infos,
                bx(b"iloc", &iloc),
                bx(b"iref", &iref),
            ]),
        )
    };
    let at = u32::try_from(ftyp.len() + build(0).len() + 8).unwrap();
    let mut grid = vec![0u8, 0, 0, 0];
    grid.extend_from_slice(&64u16.to_be_bytes());
    grid.extend_from_slice(&64u16.to_be_bytes());
    grid.resize(8 + pad as usize, 0);
    let mut file = ftyp;
    file.extend(build(at));
    file.extend(bx(b"free", &grid));
    file
}

#[test]
fn many_grid_pages_are_not_quadratic() {
    // 30000 grid items of 1 MiB each: checking every page copied each grid's bytes (30 GB)
    // and scanned every reference for its tiles (9e8 comparisons).
    let n = 30_000u16;
    let file = many_grids(n, 1 << 24);
    let t = Instant::now();
    let f = crate::AvifFile::open(file).unwrap();
    assert_eq!(f.page_count(), u32::from(n));
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
}

#[test]
fn item_prefix_matches_the_whole_item() {
    let file = many_grids(3, 100);
    assert_eq!(crate::AvifFile::open(file.clone()).unwrap().page_count(), 3);
    let b = crate::Boxes::top_level(&file).nth(1).unwrap().unwrap();
    let m = Meta::parse(b.payload).unwrap();
    for id in 1..=4 {
        let whole = m.item_data(&file, id).unwrap();
        for k in [0, 1, 7, 8, 9, 12, 108, 200] {
            let p = m.item_prefix(&file, id, k).unwrap();
            assert_eq!(&p[..], &whole[..k.min(whole.len())]);
        }
    }
    // A prefix fails as the whole item does, even when the bytes it needs are there.
    let short = &file[..file.len() - 1];
    for id in 1..=3 {
        let whole = m.item_data(short, id).unwrap_err().to_string();
        for k in [0, 8, Grid::MAX_LEN] {
            assert_eq!(m.item_prefix(short, id, k).unwrap_err().to_string(), whole);
        }
    }
    assert!(m.item_prefix(short, 4, 8).is_ok());
}

#[test]
fn grid_of_other_codec_tiles_is_refused_by_name() {
    for (t, name) in [
        (b"hvc1", "HEVC"),
        (b"jpeg", "JPEG"),
        (b"unci", "uncompressed samples"),
        (b"vvc1", "VVC"),
    ] {
        let mut file = many_grids(1, 4);
        let at = file.windows(4).position(|w| w == b"av01").unwrap();
        file[at..at + 4].copy_from_slice(t);
        let e = crate::AvifFile::open(file).unwrap_err();
        assert_eq!(e, Error::OtherCodec(name.into()));
        assert!(e.to_string().contains(name), "{e}");
    }
}
