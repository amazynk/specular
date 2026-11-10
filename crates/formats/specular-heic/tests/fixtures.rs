//! Always-on checks of the HEIF items coded other than HEVC, `tili`, `cfen`
//! and inter-coded sequences, on the small fixtures of `tests/data/`
//! (`tools/corpus/heif-gaps.py --fixtures` wrote them and their oracles:
//! libavcodec's decode of the AVC / JPEG streams and of the sequence, the
//! lossless source planes of the `cfen` image) and on `unci` files built
//! here.

use specular_heic::heif::HeifFrame;
use specular_heic::seq::{SeqPlayer, Sequence, find_moov};
use specular_heic::{Codec, DecodeOptions, Error, HeicFile, Layout};

fn data(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn opts() -> DecodeOptions<'static> {
    DecodeOptions {
        parallel: true,
        ..Default::default()
    }
}

/// The tightly packed planes of an 8-bit frame, in order.
fn planes(f: &HeifFrame) -> Vec<u8> {
    let mut out = Vec::new();
    for (i, p) in f.planes.iter().enumerate().take(f.format.plane_count()) {
        let (w, h) = f.format.plane_dims(i, f.width, f.height);
        let row = w as usize * f.format.bytes_per_sample();
        for y in 0..h as usize {
            out.extend_from_slice(&p.data[y * p.stride..y * p.stride + row]);
        }
    }
    out
}

fn max_diff(a: &[u8], b: &[u8]) -> u8 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(x, y)| x.abs_diff(*y))
        .max()
        .unwrap_or(0)
}

#[test]
fn avc_item_matches_libavcodec() {
    let f = HeicFile::open(data("avc.heif")).unwrap();
    let i = f.info();
    assert_eq!(
        (i.codec, i.bit_depth, i.width, i.height),
        (Codec::Avc, 8, 64, 48)
    );
    assert_eq!(i.chroma, specular_heic::heif::Chroma::Yuv420);
    let img = f.decode(&opts()).unwrap();
    assert_eq!(planes(&img.frame), data("avc.yuv"));
}

#[test]
fn jpeg_item_matches_libavcodec() {
    let f = HeicFile::open(data("jpeg.heif")).unwrap();
    assert_eq!(f.info().codec, Codec::Jpeg);
    let img = f.decode(&opts()).unwrap();
    // The IDCTs may round differently by one.
    assert!(max_diff(&planes(&img.frame), &data("jpeg.yuv")) <= 1);
    // JFIF full range, the nclx's BT.601.
    let y = img.colour.ycbcr();
    assert_eq!((y.matrix, y.full_range), (6, true));
}

#[test]
fn tili_matches_the_grid_it_was_built_from() {
    let grid = HeicFile::open(data("tili_src.heic")).unwrap();
    let tili = HeicFile::open(data("tili.heif")).unwrap();
    assert_eq!(tili.info().layout, Layout::Derived);
    assert_eq!(tili.info().codec, Codec::Hevc);
    let a = grid.decode(&opts()).unwrap();
    let b = tili.decode(&opts()).unwrap();
    assert_eq!((b.width(), b.height()), (112, 80));
    assert_eq!(a.frame, b.frame);

    // Tile 1 (top right) marked empty in the offset table: grey there, the rest unchanged.
    let mut bytes = data("tili.heif");
    let table = bytes.len() - 32;
    bytes[table + 8..table + 12].copy_from_slice(&u32::MAX.to_be_bytes());
    let c = HeicFile::open(bytes).unwrap().decode(&opts()).unwrap();
    assert!(c.warnings.is_empty(), "{:?}", c.warnings);
    for (x, y) in [(70, 10), (100, 60), (10, 70)] {
        let empty = x >= 64 && y < 64;
        assert_eq!(c.frame.sample(0, x, y) == 128, empty, "({x},{y})");
        if !empty {
            assert_eq!(c.frame.sample(0, x, y), a.frame.sample(0, x, y));
        }
    }

    // External tiles are refused by name.
    let mut bytes = data("tili.heif");
    let at = bytes.windows(4).position(|w| w == b"deti").unwrap();
    bytes[at + 7] |= 0x80;
    let e = HeicFile::open(bytes).unwrap().decode(&opts()).unwrap_err();
    assert!(e.to_string().contains("tiles"), "{e}");
}

#[test]
fn cfen_composes_lossless_channels() {
    let f = HeicFile::open(data("cfen.heif")).unwrap();
    let img = f.decode(&opts()).unwrap();
    // G, B, R planes, exactly the sources.
    assert_eq!(planes(&img.frame), data("cfen.yuv"));
    assert_eq!(img.colour.ycbcr().matrix, 0);
    let rgba = img.to_rgba8().unwrap();
    let src = data("cfen.yuv");
    let n = 64 * 48;
    for p in [0, 77, n - 1] {
        let rgb = [src[2 * n + p], src[p], src[n + p]];
        assert_eq!(&rgba[p * 4..p * 4 + 3], &rgb);
    }
}

fn fnv(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf2_9ce4_8422_2325, |h, &x| {
        (h ^ u64::from(x)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[test]
fn inter_coded_sequence_plays_in_display_order() {
    let bytes = data("inter.heics");
    let seq = Sequence::parse(find_moov(&bytes).unwrap()).unwrap();
    assert!(!seq.all_sync());
    assert!(
        seq.samples.iter().any(|s| s.cts_offset != 0),
        "B pictures reorder"
    );
    let want: Vec<u64> = String::from_utf8(data("inter.fnv"))
        .unwrap()
        .lines()
        .map(|l| u64::from_str_radix(l, 16).unwrap())
        .collect();
    let mut p = SeqPlayer::new(&seq, &bytes).unwrap();
    for round in 0..2 {
        let mut got = Vec::new();
        while let Some(f) = p.next_frame(&bytes, None) {
            let f = f.unwrap();
            assert_eq!(f.index, got.len());
            assert_eq!(f.delay, std::time::Duration::from_millis(100));
            got.push(fnv(&planes(&f.image.frame)));
        }
        assert_eq!(got, want, "round {round}");
        p.rewind();
    }
    // Cancelled before the first picture.
    let stop = std::sync::atomic::AtomicBool::new(true);
    assert!(matches!(
        p.next_frame(&bytes, Some(&stop)),
        Some(Err(Error::Cancelled))
    ));
    // Truncated anywhere: errors or fewer pictures, never a panic.
    for n in (0..bytes.len()).step_by(97) {
        let cut = &bytes[..n];
        if let Some(s) = find_moov(cut).and_then(Sequence::parse)
            && let Ok(mut p) = SeqPlayer::new(&s, cut)
        {
            while let Some(r) = p.next_frame(cut, None) {
                let _ = r;
            }
        }
    }
}

#[test]
fn other_codecs_are_refused_by_name() {
    // A VVC item without its vvcC is reported as such (VVC itself decodes).
    let mut bytes = data("avc.heif");
    let at = bytes.windows(4).position(|w| w == b"avc1").unwrap();
    bytes[at..at + 4].copy_from_slice(b"vvc1");
    let e = HeicFile::open(bytes).unwrap_err();
    assert!(e.to_string().contains("vvc1 item without vvcC"), "{e}");
    let mut bytes = data("avc.heif");
    bytes[at..at + 4].copy_from_slice(b"j2k1");
    let e = HeicFile::open(bytes).unwrap_err();
    assert!(e.to_string().contains("JPEG 2000"), "{e}");
}

// ---------------------------------------------------------------- unci built here

fn bx(t: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut v = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
    v.extend_from_slice(t);
    v.extend_from_slice(payload);
    v
}

fn full(t: &[u8; 4], version: u8, payload: &[u8]) -> Vec<u8> {
    bx(t, &[&[version, 0, 0, 0][..], payload].concat())
}

/// A one-item `unci` file: `props` (whole boxes, all associated, the first essential), the
/// item `body`.
fn unci_file(w: u32, h: u32, props: &[Vec<u8>], body: &[u8]) -> Vec<u8> {
    let ftyp = bx(b"ftyp", b"mif1\0\0\0\0mif1heif");
    let meta = |off: u32| {
        let hdlr = full(b"hdlr", 0, &[&[0; 4][..], b"pict", &[0; 13]].concat());
        let pitm = full(b"pitm", 0, &1u16.to_be_bytes());
        let infe = full(b"infe", 2, b"\0\x01\0\0unci\0");
        let iinf = full(b"iinf", 0, &[&1u16.to_be_bytes()[..], &infe].concat());
        let mut loc = vec![0x44, 0, 0, 1, 0, 1, 0, 0, 0, 1];
        loc.extend_from_slice(&off.to_be_bytes());
        loc.extend_from_slice(&(body.len() as u32).to_be_bytes());
        let iloc = full(b"iloc", 0, &loc);
        let mut ipco = full(b"ispe", 0, &[w.to_be_bytes(), h.to_be_bytes()].concat());
        for p in props {
            ipco.extend_from_slice(p);
        }
        let n = props.len() as u8 + 1;
        let mut assoc = vec![0, 0, 0, 1, 0, 1, n, 1];
        assoc.extend((2..=n).map(|k| if k == 2 { 0x80 | k } else { k }));
        let iprp = bx(
            b"iprp",
            &[bx(b"ipco", &ipco), full(b"ipma", 0, &assoc)].concat(),
        );
        full(b"meta", 0, &[hdlr, pitm, iinf, iloc, iprp].concat())
    };
    let off = (ftyp.len() + meta(0).len() + 8) as u32;
    [ftyp, meta(off), bx(b"mdat", body)].concat()
}

/// `uncC` version 0: components `(index, bits)`, then sampling, interleave, block size.
fn uncc(comps: &[(u16, u8)], sampling: u8, interleave: u8, block: u8) -> Vec<u8> {
    let mut b = b"\0\0\0\0".to_vec();
    b.extend_from_slice(&(comps.len() as u32).to_be_bytes());
    for &(i, bits) in comps {
        b.extend_from_slice(&i.to_be_bytes());
        b.extend_from_slice(&[bits - 1, 0, 0]);
    }
    b.extend_from_slice(&[sampling, interleave, block, 0]);
    b.extend_from_slice(&[0; 12]);
    b.extend_from_slice(&[0; 8]);
    full(b"uncC", 0, &b)
}

fn cmpd(types: &[u16]) -> Vec<u8> {
    let mut b = (types.len() as u32).to_be_bytes().to_vec();
    for t in types {
        b.extend_from_slice(&t.to_be_bytes());
    }
    bx(b"cmpd", &b)
}

#[test]
fn unci_rgb_plain_and_deflated() {
    // 3x2 RGB, pixel interleaved.
    let px: Vec<u8> = (0..18u8).map(|v| v * 13).collect();
    let props = vec![uncc(&[(0, 8), (1, 8), (2, 8)], 0, 1, 0), cmpd(&[4, 5, 6])];
    let f = HeicFile::open(unci_file(3, 2, &props, &px)).unwrap();
    assert_eq!(f.info().codec, Codec::Uncompressed);
    assert!(f.info().rgb);
    let rgba = f.decode(&opts()).unwrap().to_rgba8().unwrap();
    let want: Vec<u8> = px.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect();
    assert_eq!(rgba, want);

    // The same, deflated as one unit (cmpC 'defl', whole item).
    let mut props = props;
    props.push(full(b"cmpC", 0, b"defl\0"));
    let z = miniz_oxide::deflate::compress_to_vec(&px, 6);
    let f = HeicFile::open(unci_file(3, 2, &props, &z)).unwrap();
    assert_eq!(f.decode(&opts()).unwrap().to_rgba8().unwrap(), want);
}

#[test]
fn unci_mono_16_bit_and_rgba_profile() {
    // 2x2 monochrome, 16-bit big-endian.
    let body = [0x12, 0x34, 0xff, 0xff, 0, 0, 0x80, 0];
    let props = vec![uncc(&[(0, 16)], 0, 0, 0), cmpd(&[0])];
    let img = HeicFile::open(unci_file(2, 2, &props, &body))
        .unwrap()
        .decode(&opts())
        .unwrap();
    assert_eq!(img.frame.format.bit_depth, 16);
    assert_eq!(img.frame.sample(0, 0, 0), 0x1234);
    assert_eq!(img.frame.sample(0, 1, 1), 0x8000);

    // uncC version 1, profile 'rgba': no cmpd needed.
    let props = vec![full(b"uncC", 1, b"rgba")];
    let body = [1, 2, 3, 4, 5, 6, 7, 8];
    let f = HeicFile::open(unci_file(2, 1, &props, &body)).unwrap();
    assert!(f.info().has_alpha);
    let rgba = f.decode(&opts()).unwrap().to_rgba8().unwrap();
    assert_eq!(rgba, body);
}

#[test]
fn unci_refusals_name_the_feature() {
    let cases: [(Vec<Vec<u8>>, &str); 5] = [
        (vec![uncc(&[(0, 8)], 0, 0, 4), cmpd(&[0])], "block packing"),
        (
            vec![uncc(&[(0, 8)], 0, 0, 0), cmpd(&[11])],
            "colour filter array",
        ),
        (
            vec![uncc(&[(0, 8), (1, 8), (2, 8)], 1, 5, 0), cmpd(&[1, 2, 3])],
            "multi-Y",
        ),
        (vec![uncc(&[(0, 8)], 0, 0, 0), cmpd(&[13])], "cyan"),
        (vec![full(b"uncC", 1, b"v210")], "profile 'v210'"),
    ];
    for (props, what) in cases {
        let e = HeicFile::open(unci_file(2, 2, &props, &[0; 64])).unwrap_err();
        assert!(matches!(e, Error::Unsupported(_)), "{e}");
        assert!(e.to_string().contains(what), "{e} (wanted {what})");
    }
    // Truncated data is an error, not a panic.
    let props = vec![uncc(&[(0, 8), (1, 8), (2, 8)], 0, 1, 0), cmpd(&[4, 5, 6])];
    let f = HeicFile::open(unci_file(3, 2, &props, &[1, 2, 3])).unwrap();
    assert!(f.decode(&opts()).is_err());
}
