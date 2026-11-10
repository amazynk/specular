//! JPM (JPEG 2000 Part 6) through the registry: pages, scales, and objects of the other
//! coders drawn through the foreign-coder hook by Specular's own JBIG2, fax (MMR / MH), JBIG,
//! JPEG and JPEG-LS decoders. The JPM files are built here around the sibling crates' test
//! files (and a hand-coded MMR strip); each page is checked against the object's own decode.

use std::path::Path;
use std::sync::Arc;

use specular_core::{CancelToken, PixelLayout, Raster, Source};
use specular_decode::{DecodeError, DecodeRequest, Format, Registry, Session};

fn sibling(path: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../formats")
            .join(path),
    )
    .unwrap()
}

fn bx(t: &[u8; 4], c: &[u8]) -> Vec<u8> {
    let mut v = ((c.len() + 8) as u32).to_be_bytes().to_vec();
    v.extend_from_slice(t);
    v.extend_from_slice(c);
    v
}

fn ihdr(w: u32, h: u32, nc: u16, bits: u8, coder: u8) -> Vec<u8> {
    let mut c = h.to_be_bytes().to_vec();
    c.extend(w.to_be_bytes());
    c.extend(nc.to_be_bytes());
    c.extend([bits - 1, coder, 0, 0]);
    bx(b"ihdr", &c)
}

/// One object filling its page: (codestream, object width, height, components, bits, coder,
/// 0 mask / 1 image).
struct Obj {
    cs: Vec<u8>,
    w: u32,
    h: u32,
    nc: u16,
    bits: u8,
    coder: u8,
    ty: u8,
}

/// A JPM file of white pages, each with one layout object holding `obj` at its top left; a
/// mask object shows its base colour, black.
fn jpm(objs: &[Obj], orientation: u16) -> Vec<u8> {
    let mut f = vec![0, 0, 0, 12, b'j', b'P', b' ', b' ', 0x0D, 0x0A, 0x87, 0x0A];
    f.extend(bx(b"ftyp", b"jpm \0\0\0\0jpm "));
    let mut mh = (objs.len() as u32).to_be_bytes().to_vec();
    mh.extend([1, 1]);
    mh.extend([0u8; 12]);
    mh.extend([0x10, 0x10, 0]);
    f.extend(bx(b"mhdr", &mh));
    let mut at = Vec::new();
    for o in objs {
        at.push((f.len(), o.cs.len() as u32));
        f.extend(bx(b"jp2c", &o.cs));
    }
    for (o, (off, len)) in objs.iter().zip(at) {
        let mut ph = 1u16.to_be_bytes().to_vec();
        ph.extend(o.h.to_be_bytes());
        ph.extend(o.w.to_be_bytes());
        ph.extend(orientation.to_be_bytes());
        ph.extend(1u16.to_be_bytes()); // white page
        let mut pc = bx(b"phdr", &ph);
        let mut lh = 1u16.to_be_bytes().to_vec();
        for v in [o.h, o.w, 0, 0] {
            lh.extend(v.to_be_bytes());
        }
        lh.push(0);
        let mut lc = bx(b"lhdr", &lh);
        let mut oh = vec![o.ty, 0];
        oh.extend(0u32.to_be_bytes());
        oh.extend(0u32.to_be_bytes());
        oh.extend((off as u64).to_be_bytes());
        oh.extend(len.to_be_bytes());
        oh.extend(0u16.to_be_bytes());
        let mut oc = bx(b"ohdr", &oh);
        oc.extend(bx(b"jp2h", &ihdr(o.w, o.h, o.nc, o.bits, o.coder)));
        lc.extend(bx(b"objc", &oc));
        pc.extend(bx(b"lobj", &lc));
        f.extend(bx(b"page", &pc));
    }
    f
}

fn open(name: &str, d: Vec<u8>) -> Arc<dyn Session> {
    let reg = Registry::new();
    let dec = reg.probe(&d[..64]).expect("probed");
    assert_eq!(dec.format(), Format::Jpeg2000);
    dec.open(Arc::new(Source::from_vec(name, d))).unwrap()
}

fn decode(s: &dyn Session, scale: u32) -> Raster {
    s.decode(&DecodeRequest::full(scale), &CancelToken::new())
        .unwrap()
}

/// The RGBA bytes of a one-plane raster, rows packed.
fn rgba(r: &Raster) -> Vec<u8> {
    assert_eq!(r.layout, PixelLayout::Rgba);
    let p = &r.planes[0];
    let row = r.width as usize * 4;
    p.data
        .chunks(p.stride)
        .take(r.height as usize)
        .flat_map(|l| &l[..row])
        .copied()
        .collect()
}

/// Count of pixels whose R, G or B differs between a page and the expected grey or RGB.
fn mismatches(page: &[u8], want: impl Fn(usize) -> [u8; 3]) -> usize {
    page.chunks_exact(4)
        .enumerate()
        .filter(|(i, p)| p[..3] != want(*i) || p[3] != 255)
        .count()
}

/// A bi-level object's own decode (white 255, black 0) as the page should show it.
fn grey_want(px: &[u8]) -> impl Fn(usize) -> [u8; 3] + '_ {
    |i| [px[i]; 3]
}

/// MSB-first bit writer for the hand-coded MMR strip.
struct Bits(Vec<u8>, u32);

impl Bits {
    fn put(&mut self, code: &str) {
        for c in code.bytes() {
            if self.1.is_multiple_of(8) {
                self.0.push(0);
            }
            if c == b'1' {
                *self.0.last_mut().unwrap() |= 0x80 >> (self.1 % 8);
            }
            self.1 += 1;
        }
    }
}

/// An 8×6 T.6 (MMR) image, every row white 2, black 3, white 3: row 0 in horizontal mode
/// (`001`, white run 2 `0111`, black run 3 `10`) then V0 to the end, the rows after it V0
/// three times; then EOFB.
fn mmr_8x6() -> Vec<u8> {
    let mut b = Bits(Vec::new(), 0);
    b.put("001");
    b.put("0111");
    b.put("10");
    b.put("1");
    for _ in 1..6 {
        b.put("111");
    }
    b.put("000000000001000000000001");
    b.0
}

#[test]
fn mmr_mask_through_the_tiff_ccitt_decoder() {
    let d = jpm(
        &[Obj {
            cs: mmr_8x6(),
            w: 8,
            h: 6,
            nc: 1,
            bits: 1,
            coder: 3,
            ty: 0,
        }],
        1,
    );
    let s = open("fax.jpm", d);
    assert_eq!((s.info().width, s.info().height), (8, 6));
    assert!(s.info().coding.starts_with("JPM"), "{}", s.info().coding);
    let page = rgba(&decode(s.as_ref(), 1));
    let want = |i: usize| [if (2..5).contains(&(i % 8)) { 0 } else { 255 }; 3];
    assert_eq!(mismatches(&page, want), 0);
}

#[test]
fn jbig2_masks_embedded_and_whole_files() {
    // A sequential file without its 13-byte header is an embedded stream, as JPM stores
    // JBIG2; a whole file is taken too.
    for (name, embedded) in [
        ("bitmap-mmr.jbig2", true),
        ("bitmap-tpgdon.jbig2", false),
        ("bitmap-symbol-texthuffrefinecustom.jbig2", true),
    ] {
        let file = sibling(&format!("specular-jbig2/tests/fixtures/{name}"));
        let own = specular_jbig2::decode(&file, 0).unwrap();
        let specular_jbig2::Pixels::Gray8(px) = &own.pixels else {
            unreachable!()
        };
        let cs = if embedded {
            file[13..].to_vec()
        } else {
            file.clone()
        };
        let d = jpm(
            &[Obj {
                cs,
                w: own.width,
                h: own.height,
                nc: 1,
                bits: 1,
                coder: 8,
                ty: 0,
            }],
            1,
        );
        let s = open("t.jpm", d);
        let page = rgba(&decode(s.as_ref(), 1));
        assert_eq!(mismatches(&page, grey_want(px)), 0, "{name}");
        assert!(px.contains(&0) && px.contains(&255), "{name}");
    }
}

#[test]
fn jbig_mask() {
    let file = sibling("specular-jbig/tests/fixtures/rose_fs_default.jbg");
    let own = specular_jbig::decode(&file, 0).unwrap();
    let specular_jbig::Pixels::Gray8(px) = &own.pixels else {
        unreachable!()
    };
    let d = jpm(
        &[Obj {
            cs: file,
            w: own.width,
            h: own.height,
            nc: 1,
            bits: 1,
            coder: 4,
            ty: 0,
        }],
        1,
    );
    let page = rgba(&decode(open("t.jpm", d).as_ref(), 1));
    assert_eq!(mismatches(&page, grey_want(px)), 0);
}

/// An image object's page against the standalone file's own decode through the registry.
fn image_matches(file: Vec<u8>, name: &str, coder: u8, nc: u16) {
    let alone = open_any(name, file.clone());
    let want = rgba_of(&alone);
    let (w, h) = (alone.info().width, alone.info().height);
    let d = jpm(
        &[Obj {
            cs: file,
            w,
            h,
            nc,
            bits: 8,
            coder,
            ty: 1,
        }],
        1,
    );
    let page = rgba(&decode(open("t.jpm", d).as_ref(), 1));
    let n = mismatches(&page, |i| [want[i * 4], want[i * 4 + 1], want[i * 4 + 2]]);
    assert_eq!(n, 0, "{name}");
}

fn open_any(name: &str, d: Vec<u8>) -> Arc<dyn Session> {
    let reg = Registry::new();
    let dec = reg.probe(&d[..64.min(d.len())]).expect(name);
    dec.open(Arc::new(Source::from_vec(name, d))).unwrap()
}

/// A standalone decode as packed RGBA (grey replicated, planar RGB interleaved).
fn rgba_of(s: &Arc<dyn Session>) -> Vec<u8> {
    let r = decode(s.as_ref(), 1);
    let n = r.width as usize * r.height as usize;
    let at = |p: usize, i: usize| {
        let pl = &r.planes[p];
        let w = r.width as usize;
        pl.data[(i / w) * pl.stride + i % w]
    };
    match r.layout {
        PixelLayout::Rgba => rgba(&r),
        PixelLayout::Gray => (0..n)
            .flat_map(|i| [at(0, i), at(0, i), at(0, i), 255])
            .collect(),
        PixelLayout::Rgb => (0..n)
            .flat_map(|i| [at(0, i), at(1, i), at(2, i), 255])
            .collect(),
        l => panic!("{l:?}"),
    }
}

#[test]
fn jpeg_and_jpeg_ls_images() {
    image_matches(
        sibling("specular-jpeg/tests/fixtures/gray8.jpg"),
        "gray8.jpg",
        5,
        1,
    );
    image_matches(
        sibling("specular-jpegls/tests/vectors/grey8.jls"),
        "grey8.jls",
        6,
        1,
    );
    image_matches(
        sibling("specular-jpegls/tests/vectors/rgb8_ilv2.jls"),
        "rgb8_ilv2.jls",
        6,
        3,
    );
}

#[test]
fn pages_scales_orientation_and_refusals() {
    let fax = || Obj {
        cs: mmr_8x6(),
        w: 8,
        h: 6,
        nc: 1,
        bits: 1,
        coder: 3,
        ty: 0,
    };
    let file = jpm(&[fax(), fax(), fax()], 1);
    let reg = Registry::new();
    // By extension as well as by content.
    assert!(reg.extensions().contains(&"jpm") && reg.extensions().contains(&"jpgm"));
    let s = open("p.jpm", file);
    assert_eq!((s.page_count(), s.page_index()), (3, 0));
    assert!(
        s.info().coding.ends_with("page 1 of 3"),
        "{}",
        s.info().coding
    );
    let p2 = s.open_page(2).unwrap();
    assert_eq!(p2.page_index(), 2);
    assert!(p2.open_page(3).is_err());
    let half = decode(p2.as_ref(), 2);
    assert_eq!((half.width, half.height), (4, 3));

    // Turned a quarter: 6 wide, 8 high.
    let s = open("r.jpm", jpm(&[fax()], 2));
    assert_eq!((s.info().width, s.info().height), (6, 8));
    let r = decode(s.as_ref(), 1);
    assert_eq!((r.width, r.height), (6, 8));

    // A coder nothing decodes is refused by name; so is damaged JBIG2 data.
    let mut bad = fax();
    bad.coder = 8;
    bad.cs = vec![0xFF; 7];
    let s = open("b.jpm", jpm(&[bad], 1));
    match s.decode(&DecodeRequest::full(1), &CancelToken::new()) {
        Err(DecodeError::Unsupported(m)) => assert!(m.contains("JBIG2"), "{m}"),
        r => panic!("{:?}", r.map(|r| r.width)),
    }
}
