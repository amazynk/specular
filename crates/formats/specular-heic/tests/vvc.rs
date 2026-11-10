//! VVC (`vvc1`) items: HEIF files written here around JVET conformance pictures (single
//! items, a grid, an alpha auxiliary, a thumbnail), and libheif's VVC sample.
//!
//! The corpus is gitignored (tools/corpus/fetch-vvc.sh); every test passes with a note
//! when it is absent. The `*_matches_ffmpeg` tests compare the decoded planes with the first
//! picture of ffmpeg's native `vvc` decoder (`<stream>.f0.yuv`) and print the mismatch
//! counts; the others only check the container side.

use std::path::PathBuf;

use specular_heic::heif::{Chroma, HeifFrame};
use specular_heic::{Codec, DecodeOptions, HeicFile, Layout};

fn corpus() -> Option<PathBuf> {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/conformance/vvc");
    if d.join("manifest.tsv").exists() {
        Some(d)
    } else {
        eprintln!("VVC corpus absent (tools/corpus/fetch-vvc.sh): skipped");
        None
    }
}

fn opts() -> DecodeOptions<'static> {
    DecodeOptions::default()
}

// ------------------------------------------------------------------ Annex B → items

/// The parameter sets (VPS, SPS, PPS) and the other NAL units of the first picture of a
/// conformance stream.
struct FirstPicture {
    sets: Vec<Vec<u8>>,
    units: Vec<Vec<u8>>,
}

fn first_picture(stream: &[u8]) -> FirstPicture {
    let mut sets = Vec::new();
    let mut units = Vec::new();
    let mut seen_vcl = false;
    for u in specular_vvc::nal::split_annexb(stream) {
        let t = u[1] >> 3;
        let vcl = t <= 11;
        if seen_vcl
            && !(vcl && u.get(2).is_some_and(|b| b & 0x80 == 0))
            && ![18, 24, 25].contains(&t)
        {
            break;
        }
        match t {
            14..=16 => sets.push(u.to_vec()),
            12 | 13 | 20..=22 => {}
            _ => units.push(u.to_vec()),
        }
        seen_vcl |= vcl;
    }
    FirstPicture { sets, units }
}

// ------------------------------------------------------------------ HEIF writer

fn bx(t: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut v = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
    v.extend_from_slice(t);
    v.extend_from_slice(payload);
    v
}

fn full(t: &[u8; 4], version: u8, payload: &[u8]) -> Vec<u8> {
    bx(t, &[&[version, 0, 0, 0][..], payload].concat())
}

/// A `vvcC` box (FullBox, length size 4, no profile record) carrying `sets`.
fn vvcc(sets: &[Vec<u8>]) -> Vec<u8> {
    let mut r = vec![0xfe, 0];
    let mut n = 0u8;
    for t in 14..=16u8 {
        let of: Vec<&Vec<u8>> = sets.iter().filter(|u| u[1] >> 3 == t).collect();
        if of.is_empty() {
            continue;
        }
        n += 1;
        r.push(0x80 | t);
        r.extend((of.len() as u16).to_be_bytes());
        for u in of {
            r.extend((u.len() as u16).to_be_bytes());
            r.extend(u);
        }
    }
    r[1] = n;
    full(b"vvcC", 0, &r)
}

fn length_prefixed(units: &[Vec<u8>]) -> Vec<u8> {
    units
        .iter()
        .flat_map(|u| [(u.len() as u32).to_be_bytes().to_vec(), u.clone()].concat())
        .collect()
}

struct Item {
    id: u16,
    ty: [u8; 4],
    data: Vec<u8>,
    /// Property boxes; the first is essential.
    props: Vec<Vec<u8>>,
}

impl Item {
    fn vvc(id: u16, pic: &FirstPicture, w: u32, h: u32) -> Item {
        Item {
            id,
            ty: *b"vvc1",
            data: length_prefixed(&pic.units),
            props: vec![vvcc(&pic.sets), ispe(w, h)],
        }
    }
}

fn ispe(w: u32, h: u32) -> Vec<u8> {
    full(b"ispe", 0, &[w.to_be_bytes(), h.to_be_bytes()].concat())
}

/// A HEIF file: `items` (the first is primary), `refs` as (type, from, to).
fn heif_file(items: &[Item], refs: &[(&[u8; 4], u16, Vec<u16>)]) -> Vec<u8> {
    let ftyp = bx(b"ftyp", b"mif1\0\0\0\0mif1vvicmiaf");
    let meta = |base: u32| {
        let hdlr = full(b"hdlr", 0, &[&[0; 4][..], b"pict", &[0; 13]].concat());
        let pitm = full(b"pitm", 0, &items[0].id.to_be_bytes());
        let mut iinf = (items.len() as u16).to_be_bytes().to_vec();
        for it in items {
            let mut e = it.id.to_be_bytes().to_vec();
            e.extend([0, 0]);
            e.extend(it.ty);
            e.push(0);
            iinf.extend(full(b"infe", 2, &e));
        }
        let iinf = full(b"iinf", 0, &iinf);
        let mut loc = vec![0x44, 0];
        loc.extend((items.len() as u16).to_be_bytes());
        let mut off = base;
        for it in items {
            loc.extend(it.id.to_be_bytes());
            loc.extend([0, 0, 0, 1]);
            loc.extend(off.to_be_bytes());
            loc.extend((it.data.len() as u32).to_be_bytes());
            off += it.data.len() as u32;
        }
        let iloc = full(b"iloc", 0, &loc);
        let mut iref = Vec::new();
        for (t, from, to) in refs {
            let mut b = from.to_be_bytes().to_vec();
            b.extend((to.len() as u16).to_be_bytes());
            for t in to {
                b.extend(t.to_be_bytes());
            }
            iref.extend(bx(t, &b));
        }
        let mut ipco = Vec::new();
        let mut assoc = (items.len() as u32).to_be_bytes().to_vec();
        let mut k = 0u8;
        for it in items {
            assoc.extend(it.id.to_be_bytes());
            assoc.push(it.props.len() as u8);
            for (j, p) in it.props.iter().enumerate() {
                ipco.extend(p);
                k += 1;
                assoc.push(if j == 0 { 0x80 | k } else { k });
            }
        }
        let iprp = bx(
            b"iprp",
            &[bx(b"ipco", &ipco), full(b"ipma", 0, &assoc)].concat(),
        );
        let mut boxes = vec![hdlr, pitm, iinf, iloc];
        if !refs.is_empty() {
            boxes.push(full(b"iref", 0, &iref));
        }
        boxes.push(iprp);
        full(b"meta", 0, &boxes.concat())
    };
    let base = (ftyp.len() + meta(0).len() + 8) as u32;
    let mdat: Vec<u8> = items.iter().flat_map(|i| i.data.clone()).collect();
    [ftyp, meta(base), bx(b"mdat", &mdat)].concat()
}

// ------------------------------------------------------------------ the oracle

struct Stream {
    name: &'static str,
    w: u32,
    h: u32,
    chroma: Chroma,
    depth: u8,
}

const STILL: Stream = Stream {
    name: "STILL_A_1",
    w: 416,
    h: 240,
    chroma: Chroma::Yuv420,
    depth: 10,
};
const STILL444: Stream = Stream {
    name: "STILL444_A_1",
    w: 416,
    h: 240,
    chroma: Chroma::Yuv444,
    depth: 10,
};
const MONO8: Stream = Stream {
    name: "8b400_A_2",
    w: 832,
    h: 480,
    chroma: Chroma::Mono,
    depth: 8,
};
const YUV8: Stream = Stream {
    name: "8b420_A_2",
    w: 832,
    h: 480,
    chroma: Chroma::Yuv420,
    depth: 8,
};

fn load(dir: &std::path::Path, s: &Stream) -> Option<FirstPicture> {
    let d = std::fs::read(dir.join(format!("{}.bit", s.name))).ok()?;
    Some(first_picture(&d))
}

/// The planes of ffmpeg's first picture.
fn oracle(dir: &std::path::Path, s: &Stream) -> Option<Vec<Vec<u16>>> {
    let raw = std::fs::read(dir.join(format!("{}.f0.yuv", s.name))).ok()?;
    let (cw, ch) = s.chroma.chroma_dims(s.w, s.h);
    let mut sizes = vec![(s.w * s.h) as usize];
    if s.chroma != Chroma::Mono {
        sizes.extend([(cw * ch) as usize; 2]);
    }
    let bps = if s.depth > 8 { 2 } else { 1 };
    let mut p = 0;
    let mut out = Vec::new();
    for n in sizes {
        let b = raw.get(p..p + n * bps)?;
        p += n * bps;
        out.push(if bps == 2 {
            b.chunks(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect()
        } else {
            b.iter().map(|&v| u16::from(v)).collect()
        });
    }
    Some(out)
}

/// Mismatching samples of `planes` of `f` (offset by `dx` luma samples) against the oracle.
fn mismatches(f: &HeifFrame, planes: &[usize], want: &[Vec<u16>], s: &Stream, dx: u32) -> usize {
    let mut bad = 0;
    for (k, &p) in planes.iter().enumerate() {
        let (w, h) = if k == 0 {
            (s.w, s.h)
        } else {
            s.chroma.chroma_dims(s.w, s.h)
        };
        let ox = if k == 0 { dx } else { dx * w / s.w };
        for y in 0..h {
            for x in 0..w {
                if f.sample(p, ox + x, y) != want[k][(y * w + x) as usize] {
                    bad += 1;
                }
            }
        }
    }
    bad
}

fn colour_planes(s: &Stream) -> Vec<usize> {
    if s.chroma == Chroma::Mono {
        vec![0]
    } else {
        vec![0, 1, 2]
    }
}

// ------------------------------------------------------------------ container side

#[test]
fn vvc_items_open_with_their_format() {
    let Some(dir) = corpus() else { return };
    for s in [&STILL, &STILL444, &MONO8] {
        let Some(pic) = load(&dir, s) else { continue };
        let f = HeicFile::open(heif_file(&[Item::vvc(1, &pic, s.w, s.h)], &[])).unwrap();
        let i = f.info();
        assert_eq!(i.codec, Codec::Vvc);
        assert_eq!(i.codec.name(), "VVC");
        assert_eq!((i.width, i.height), (s.w, s.h), "{}", s.name);
        assert_eq!((i.chroma, i.bit_depth), (s.chroma, s.depth), "{}", s.name);
    }
}

#[test]
fn vvc_grid_alpha_and_thumbnail_open() {
    let Some(dir) = corpus() else { return };
    let (Some(still), Some(mono), Some(yuv)) =
        (load(&dir, &STILL), load(&dir, &MONO8), load(&dir, &YUV8))
    else {
        return;
    };
    let f = HeicFile::open(grid_file(&still)).unwrap();
    assert!(matches!(
        f.info().layout,
        Layout::Grid {
            rows: 1,
            columns: 2,
            ..
        }
    ));
    assert_eq!((f.info().width, f.info().height), (832, 240));
    assert_eq!(f.tile_count(), 2);
    let f = HeicFile::open(alpha_thumb_file(&yuv, &mono, &still)).unwrap();
    assert!(f.info().has_alpha);
    assert_eq!(f.thumbnail(), Some((3, 416, 240)));
}

fn grid_file(still: &FirstPicture) -> Vec<u8> {
    let mut g = vec![0, 0, 0, 1];
    g.extend(832u16.to_be_bytes());
    g.extend(240u16.to_be_bytes());
    let grid = Item {
        id: 1,
        ty: *b"grid",
        data: g,
        props: vec![ispe(832, 240)],
    };
    heif_file(
        &[
            grid,
            Item::vvc(2, still, 416, 240),
            Item::vvc(3, still, 416, 240),
        ],
        &[(b"dimg", 1, vec![2, 3])],
    )
}

fn alpha_thumb_file(yuv: &FirstPicture, mono: &FirstPicture, still: &FirstPicture) -> Vec<u8> {
    let mut alpha = Item::vvc(2, mono, 832, 480);
    alpha.props.push(full(
        b"auxC",
        0,
        b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha\0",
    ));
    heif_file(
        &[
            Item::vvc(1, yuv, 832, 480),
            alpha,
            Item::vvc(3, still, 416, 240),
        ],
        &[(b"auxl", 2, vec![1]), (b"thmb", 3, vec![1])],
    )
}

// ------------------------------------------------------------------ pixels (decoder)

#[test]
fn vvc_single_items_match_ffmpeg() {
    let Some(dir) = corpus() else { return };
    let mut total = 0;
    for s in [&STILL, &STILL444, &MONO8, &YUV8] {
        let (Some(pic), Some(want)) = (load(&dir, s), oracle(&dir, s)) else {
            continue;
        };
        let f = HeicFile::open(heif_file(&[Item::vvc(1, &pic, s.w, s.h)], &[])).unwrap();
        let img = f
            .decode(&opts())
            .unwrap_or_else(|e| panic!("{}: {e}", s.name));
        assert_eq!((img.width(), img.height()), (s.w, s.h));
        let bad = mismatches(&img.frame, &colour_planes(s), &want, s, 0);
        eprintln!("{}: {bad} mismatching samples against ffmpeg", s.name);
        total += bad;
    }
    assert_eq!(total, 0, "samples differ from ffmpeg");
}

#[test]
fn vvc_grid_alpha_and_thumbnail_match_ffmpeg() {
    let Some(dir) = corpus() else { return };
    let (Some(still), Some(mono), Some(yuv)) =
        (load(&dir, &STILL), load(&dir, &MONO8), load(&dir, &YUV8))
    else {
        return;
    };
    let (Some(w_still), Some(w_mono), Some(w_yuv)) = (
        oracle(&dir, &STILL),
        oracle(&dir, &MONO8),
        oracle(&dir, &YUV8),
    ) else {
        return;
    };
    let f = HeicFile::open(grid_file(&still)).unwrap();
    let img = f.decode(&opts()).unwrap();
    let bad = mismatches(&img.frame, &[0, 1, 2], &w_still, &STILL, 0)
        + mismatches(&img.frame, &[0, 1, 2], &w_still, &STILL, 416);
    eprintln!("grid: {bad} mismatching samples");
    assert_eq!(bad, 0);

    let f = HeicFile::open(alpha_thumb_file(&yuv, &mono, &still)).unwrap();
    let img = f.decode(&opts()).unwrap();
    assert!(img.frame.format.has_alpha);
    let a = img.frame.format.alpha_plane().unwrap();
    let bad_c = mismatches(&img.frame, &[0, 1, 2], &w_yuv, &YUV8, 0);
    let bad_a = mismatches(&img.frame, &[a], &w_mono, &MONO8, 0);
    let thumb = f.decode_thumbnail(3, &opts()).unwrap();
    let bad_t = mismatches(&thumb.frame, &[0, 1, 2], &w_still, &STILL, 0);
    eprintln!("alpha file: colour {bad_c}, alpha {bad_a}, thumbnail {bad_t} mismatching samples");
    assert_eq!(bad_c + bad_a + bad_t, 0);
}

#[test]
fn libheif_vvc_sample_decodes() {
    let Some(dir) = corpus() else { return };
    let Ok(d) = std::fs::read(dir.join("vvc32.heif")) else {
        return;
    };
    let f = HeicFile::open(d).unwrap();
    assert_eq!(f.info().codec, Codec::Vvc);
    assert_eq!((f.info().width, f.info().height), (32, 32));
    match f.decode(&opts()) {
        Ok(img) => assert_eq!((img.width(), img.height()), (32, 32)),
        Err(e) => panic!("vvc32.heif: {e}"),
    }
}
