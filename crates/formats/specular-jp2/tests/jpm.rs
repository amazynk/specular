//! JPM (JPEG 2000 Part 6) pages built here from the JPEG 2000 fixtures, drawn by
//! `decode_jpm_page` and checked against the T.805 clause 5 composition computed by hand from
//! the objects' own decodes.

use specular_jp2::{
    DecodeOptions, ForeignObject, Image, Jp2Error, JpmCoder, JpmOptions, PixelFormat, Pixels,
    decode_jpm_page, is_jpm, read_jpm_info,
};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn bx(t: &[u8; 4], c: &[u8]) -> Vec<u8> {
    let mut v = ((c.len() + 8) as u32).to_be_bytes().to_vec();
    v.extend_from_slice(t);
    v.extend_from_slice(c);
    v
}

/// The `(type, contents)` boxes of a box list.
fn boxes(mut d: &[u8]) -> Vec<([u8; 4], Vec<u8>)> {
    let mut v = Vec::new();
    while d.len() >= 8 {
        let l = u32::from_be_bytes(d[..4].try_into().unwrap()) as usize;
        v.push((d[4..8].try_into().unwrap(), d[8..l].to_vec()));
        d = &d[l..];
    }
    v
}

fn find(d: &[u8], t: &[u8; 4]) -> Vec<u8> {
    boxes(d).into_iter().find(|b| &b.0 == t).unwrap().1
}

/// (JP2 header contents, codestream) of a JP2 fixture.
fn jp2_parts(name: &str) -> (Vec<u8>, Vec<u8>) {
    let d = fixture(name);
    (find(&d[12..], b"jp2h"), find(&d[12..], b"jp2c"))
}

fn ihdr(w: u32, h: u32, nc: u16, bits: u8, coder: u8) -> Vec<u8> {
    let mut c = h.to_be_bytes().to_vec();
    c.extend(w.to_be_bytes());
    c.extend(nc.to_be_bytes());
    c.extend([bits - 1, coder, 0, 0]);
    bx(b"ihdr", &c)
}

/// A JP2 header for a raw codestream fixture.
fn j2k_parts(name: &str) -> (Vec<u8>, Vec<u8>, u32, u32) {
    let cs = fixture(name);
    let info = specular_jp2::read_info(&cs).unwrap();
    let nc = info.components.len() as u16;
    let mut h = ihdr(info.width, info.height, nc, 8, 7);
    let colr: &[u8] = if nc >= 3 {
        &[1, 0, 0, 0, 0, 0, 16]
    } else {
        &[1, 0, 0, 0, 0, 0, 17]
    };
    h.extend(bx(b"colr", colr));
    (h, cs, info.width, info.height)
}

/// 1-bit uncompressed object data, rows padded to bytes.
fn bits(w: u32, h: u32, f: impl Fn(u32, u32) -> bool) -> Vec<u8> {
    let row = w.div_ceil(8) as usize;
    let mut v = vec![0u8; row * h as usize];
    for y in 0..h {
        for x in 0..w {
            if f(x, y) {
                v[y as usize * row + x as usize / 8] |= 0x80 >> (x % 8);
            }
        }
    }
    v
}

#[derive(Clone, Default)]
struct Obj {
    /// 0 mask, 1 image, 2 both.
    ty: u8,
    /// Index into `Writer::streams`, or `None` for no codestream.
    stream: Option<usize>,
    /// OVoff, OHoff.
    off: (u32, u32),
    /// VRN, VRD, HRN, HRD.
    scale: Option<[u16; 4]>,
    base: Option<[u8; 3]>,
    jp2h: Vec<u8>,
    /// Refer to the JP2 header through shared data `sdat` id 7 instead.
    shared_hdr: bool,
}

#[derive(Clone)]
struct Lobj {
    id: u16,
    /// LHeight, LWidth, LVoff, LHoff.
    win: [u32; 4],
    objs: Vec<Obj>,
}

struct Page {
    w: u32,
    h: u32,
    orientation: u16,
    colour: u16,
    base: Option<[u8; 3]>,
    lobjs: Vec<Lobj>,
}

fn bclr(c: [u8; 3]) -> Vec<u8> {
    let mut v = 3u16.to_be_bytes().to_vec();
    v.push(7);
    for x in c {
        v.extend((x as u16).to_be_bytes());
    }
    let mut b = bx(b"bcvl", &v);
    b.extend(bx(b"colr", &[1, 0, 0, 0, 0, 0, 16]));
    bx(b"bclr", &b)
}

/// Writes a JPM file: codestreams first (as contiguous codestream boxes, or a fragment table
/// in three pieces when `fragment`), then a shared JP2 header and the pages.
fn write(streams: &[Vec<u8>], pages: &[Page], fragment: bool, shared: Option<&[u8]>) -> Vec<u8> {
    let mut f = vec![0, 0, 0, 12, b'j', b'P', b' ', b' ', 0x0D, 0x0A, 0x87, 0x0A];
    f.extend(bx(b"ftyp", b"jpm \0\0\0\0jpm "));
    let mut mh = (pages.len() as u32).to_be_bytes().to_vec();
    mh.extend([1, 1]);
    mh.extend([0u8; 12]);
    mh.extend([0x10, 0x10, 0]);
    f.extend(bx(b"mhdr", &mh));
    let mut at = Vec::new();
    for s in streams {
        if fragment {
            // The codestream in three media data boxes, listed by a fragment table.
            let cuts = [0, s.len() / 3, s.len() * 2 / 3, s.len()];
            let mut list = 3u16.to_be_bytes().to_vec();
            for k in 0..3 {
                let piece = &s[cuts[k]..cuts[k + 1]];
                let off = f.len() + 8;
                f.extend(bx(b"mdat", piece));
                list.extend((off as u64).to_be_bytes());
                list.extend((piece.len() as u32).to_be_bytes());
                list.extend(0u16.to_be_bytes());
            }
            at.push((f.len(), 0u32));
            f.extend(bx(b"ftbl", &bx(b"flst", &list)));
        } else {
            at.push((f.len(), s.len() as u32));
            f.extend(bx(b"jp2c", s));
        }
    }
    if let Some(h) = shared {
        let mut c = 7u16.to_be_bytes().to_vec();
        c.extend(bx(b"jp2h", h));
        f.extend(bx(b"sdat", &c));
    }
    for p in pages {
        let mut ph = (p.lobjs.len() as u16).to_be_bytes().to_vec();
        ph.extend(p.h.to_be_bytes());
        ph.extend(p.w.to_be_bytes());
        ph.extend(p.orientation.to_be_bytes());
        ph.extend(p.colour.to_be_bytes());
        let mut pc = bx(b"phdr", &ph);
        if let Some(c) = p.base {
            pc.extend(bclr(c));
        }
        for l in &p.lobjs {
            let mut lh = l.id.to_be_bytes().to_vec();
            for v in l.win {
                lh.extend(v.to_be_bytes());
            }
            lh.push(0);
            let mut lc = bx(b"lhdr", &lh);
            for o in &l.objs {
                let mut oh = vec![o.ty, o.stream.is_none() as u8];
                oh.extend(o.off.0.to_be_bytes());
                oh.extend(o.off.1.to_be_bytes());
                let (off, len) = o.stream.map_or((0, 0), |s| at[s]);
                oh.extend((off as u64).to_be_bytes());
                oh.extend(len.to_be_bytes());
                oh.extend(0u16.to_be_bytes());
                let mut oc = bx(b"ohdr", &oh);
                if let Some(s) = o.scale {
                    let v: Vec<u8> = s.iter().flat_map(|x| x.to_be_bytes()).collect();
                    oc.extend(bx(b"scal", &v));
                }
                if let Some(c) = o.base {
                    oc.extend(bclr(c));
                }
                if o.shared_hdr {
                    oc.extend(bx(b"sref", &7u16.to_be_bytes()));
                } else if o.stream.is_some() {
                    oc.extend(bx(b"jp2h", &o.jp2h));
                }
                lc.extend(bx(b"objc", &oc));
            }
            pc.extend(bx(b"lobj", &lc));
        }
        f.extend(bx(b"page", &pc));
    }
    f
}

fn rgba(img: &Image) -> &[u8] {
    assert_eq!(img.format, PixelFormat::Rgba8);
    let Pixels::U8(px) = &img.pixels else {
        unreachable!()
    };
    px
}

/// An object's own decode as RGBA (grey replicated).
fn own(file: &[u8]) -> (u32, u32, Vec<u8>) {
    let img = specular_jp2::decode(file, &DecodeOptions::default()).unwrap();
    let Pixels::U8(px) = img.pixels else {
        unreachable!()
    };
    let v = match img.format {
        PixelFormat::Gray8 => px.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        _ => px,
    };
    (img.width, img.height, v)
}

fn page(w: u32, h: u32, colour: u16, lobjs: Vec<Lobj>) -> Page {
    Page {
        w,
        h,
        orientation: 1,
        colour,
        base: None,
        lobjs,
    }
}

/// "over" of straight 8-bit colour `c` with opacity `m` onto opaque `under`.
fn over(under: u8, c: u8, m: u8) -> u8 {
    let m = m as f32 / 255.0;
    (m * c as f32 + (1.0 - m) * under as f32).round() as u8
}

#[test]
fn single_image_on_a_white_page() {
    let (h, cs, w0, h0) = j2k_parts("small_rev.j2k");
    let lobj = Lobj {
        id: 1,
        win: [h0, w0, 3, 5],
        objs: vec![Obj {
            ty: 1,
            stream: Some(0),
            jp2h: h,
            ..Default::default()
        }],
    };
    let file = write(
        std::slice::from_ref(&cs),
        &[page(w0 + 9, h0 + 4, 1, vec![lobj])],
        false,
        None,
    );
    assert!(is_jpm(&file));
    let info = read_jpm_info(&file).unwrap();
    assert_eq!(info.pages.len(), 1);
    assert_eq!(
        (info.pages[0].width, info.pages[0].height),
        (w0 + 9, h0 + 4)
    );
    // The single-image API refuses it by name.
    assert!(matches!(
        specular_jp2::decode(&file, &DecodeOptions::default()),
        Err(Jp2Error::Unsupported(_))
    ));
    let img = decode_jpm_page(&file, 0, &JpmOptions::default()).unwrap();
    assert_eq!((img.width, img.height), (w0 + 9, h0 + 4));
    let (_, _, src) = own(&cs);
    let px = rgba(&img);
    let mut bad = 0;
    for y in 0..img.height {
        for x in 0..img.width {
            let o = ((y * img.width + x) * 4) as usize;
            let want: [u8; 4] = if (5..5 + w0).contains(&x) && (3..3 + h0).contains(&y) {
                let s = (((y - 3) * w0 + x - 5) * 4) as usize;
                src[s..s + 4].try_into().unwrap()
            } else {
                [255; 4]
            };
            bad += (px[o..o + 4] != want) as usize;
        }
    }
    assert_eq!(bad, 0);
}

/// MRC: a grey background image, then black-and-red text through a 1-bit mask with offsets and
/// a layout window smaller than the objects; the same through a fragment table and a shared
/// JP2 header.
#[test]
fn mask_and_image_layers_compose() {
    let (gh, gcs) = jp2_parts("gray_rev.jp2");
    let (bw, bh) = (131u32, 97u32);
    let (mw, mh) = (60u32, 40u32);
    let mbits = bits(mw, mh, |x, y| (x / 3 + y / 2).is_multiple_of(3));
    let mask_hdr = ihdr(mw, mh, 1, 1, 0);
    // The foreground image: a flat 60x40 RGB uncompressed object.
    let fg: Vec<u8> = (0..mw * mh)
        .flat_map(|i| [200, (i % 256) as u8, 10])
        .collect();
    let mut fg_hdr = ihdr(mw, mh, 3, 8, 0);
    fg_hdr.extend(bx(b"colr", &[1, 0, 0, 0, 0, 0, 16]));
    let (lvoff, lhoff, lh, lw) = (20u32, 30u32, 25u32, 50u32);
    let (ovoff, ohoff) = (4u32, 7u32);
    let pages = |shared: bool| {
        vec![page(
            bw,
            bh,
            1,
            vec![
                Lobj {
                    id: 1,
                    win: [bh, bw, 0, 0],
                    objs: vec![Obj {
                        ty: 1,
                        stream: Some(0),
                        jp2h: gh.clone(),
                        shared_hdr: shared,
                        ..Default::default()
                    }],
                },
                Lobj {
                    id: 2,
                    win: [lh, lw, lvoff, lhoff],
                    objs: vec![
                        Obj {
                            ty: 0,
                            stream: Some(1),
                            off: (ovoff, ohoff),
                            jp2h: mask_hdr.clone(),
                            ..Default::default()
                        },
                        Obj {
                            ty: 1,
                            stream: Some(2),
                            off: (ovoff, ohoff),
                            jp2h: fg_hdr.clone(),
                            ..Default::default()
                        },
                    ],
                },
                // Black text (mask only, no image object: base black) over the corner.
                Lobj {
                    id: 3,
                    win: [10, 10, 0, 0],
                    objs: vec![Obj {
                        ty: 0,
                        stream: Some(1),
                        jp2h: mask_hdr.clone(),
                        ..Default::default()
                    }],
                },
            ],
        )]
    };
    let streams = [gcs.clone(), mbits.clone(), fg.clone()];
    let (_, _, bg) = own(&fixture("gray_rev.jp2"));
    let mbit = |x: u32, y: u32| (x / 3 + y / 2).is_multiple_of(3);
    for (fragment, shared) in [(false, false), (true, false), (false, true)] {
        let file = write(
            &streams,
            &pages(shared),
            fragment,
            shared.then_some(&gh[..]),
        );
        let img = decode_jpm_page(&file, 0, &JpmOptions::default()).unwrap();
        let px = rgba(&img);
        let mut bad = 0;
        for y in 0..bh {
            for x in 0..bw {
                let o = ((y * bw + x) * 4) as usize;
                let mut want: [u8; 4] = bg[o..o + 4].try_into().unwrap();
                if (lhoff..lhoff + lw).contains(&x) && (lvoff..lvoff + lh).contains(&y) {
                    let (u, v) = (x - lhoff + ohoff, y - lvoff + ovoff);
                    // Inside the window: the object, or opaque base (black) past its edge.
                    let (c, m) = if u < mw && v < mh {
                        let i = ((v * mw + u) * 3) as usize;
                        (
                            [fg[i], fg[i + 1], fg[i + 2]],
                            if mbit(u, v) { 255 } else { 0 },
                        )
                    } else {
                        ([0, 0, 0], 255)
                    };
                    for k in 0..3 {
                        want[k] = over(want[k], c[k], m);
                    }
                }
                if x < 10 && y < 10 && mbit(x, y) {
                    want = [0, 0, 0, 255];
                }
                bad += (px[o..o + 4] != want) as usize;
            }
        }
        assert_eq!(bad, 0, "fragment {fragment} shared {shared}");
    }
}

#[test]
fn image_and_mask_object_blends_by_its_opacity() {
    let (h, cs) = jp2_parts("rgba_rev.jp2");
    let file_a = fixture("rgba_rev.jp2");
    let (w0, h0, src) = own(&file_a);
    let lobj = Lobj {
        id: 1,
        win: [h0, w0, 0, 0],
        objs: vec![Obj {
            ty: 2,
            stream: Some(0),
            jp2h: h,
            ..Default::default()
        }],
    };
    let mut p = page(w0, h0, 255, vec![lobj]);
    p.base = Some([10, 120, 250]);
    let file = write(&[cs], &[p], false, None);
    let img = decode_jpm_page(&file, 0, &JpmOptions::default()).unwrap();
    let px = rgba(&img);
    let mut bad = 0;
    for (o, s) in px.chunks_exact(4).zip(src.chunks_exact(4)) {
        let want = [
            over(10, s[0], s[3]),
            over(120, s[1], s[3]),
            over(250, s[2], s[3]),
            255,
        ];
        bad += (o != want) as usize;
    }
    assert_eq!(bad, 0);
}

#[test]
fn transparent_page_keeps_alpha() {
    let (mw, mh) = (16u32, 8u32);
    let lobj = Lobj {
        id: 1,
        win: [mh, mw, 0, 0],
        objs: vec![
            Obj {
                ty: 0,
                stream: Some(0),
                jp2h: ihdr(mw, mh, 1, 1, 0),
                ..Default::default()
            },
            Obj {
                ty: 1,
                stream: None,
                base: Some([0, 0, 255]),
                ..Default::default()
            },
        ],
    };
    let file = write(
        &[bits(mw, mh, |x, _| x < 8)],
        &[page(mw, mh, 0, vec![lobj])],
        false,
        None,
    );
    let img = decode_jpm_page(&file, 0, &JpmOptions::default()).unwrap();
    for (i, p) in rgba(&img).chunks_exact(4).enumerate() {
        let want = if (i as u32 % mw) < 8 {
            [0, 0, 255, 255]
        } else {
            [0, 0, 0, 0]
        };
        assert_eq!(p, want, "pixel {i}");
    }
}

#[test]
fn scaled_object_is_bilinear_and_reduce_shrinks_the_page() {
    // A 4x2 grey image scaled 3:1 onto a 12x6 page.
    let g = [0u8, 60, 120, 180, 30, 90, 150, 210];
    let lobj = Lobj {
        id: 1,
        win: [6, 12, 0, 0],
        objs: vec![Obj {
            ty: 1,
            stream: Some(0),
            scale: Some([3, 1, 3, 1]),
            jp2h: ihdr(4, 2, 1, 8, 0),
            ..Default::default()
        }],
    };
    let file = write(&[g.to_vec()], &[page(12, 6, 1, vec![lobj])], false, None);
    let img = decode_jpm_page(&file, 0, &JpmOptions::default()).unwrap();
    let px = rgba(&img);
    for y in 0..6usize {
        for x in 0..12usize {
            // Centre-aligned: page pixel centre (x + 0.5) samples object x' = (x + 0.5) / 3 - 0.5.
            let sx = ((x as f32 + 0.5) / 3.0 - 0.5).max(0.0);
            let sy = ((y as f32 + 0.5) / 3.0 - 0.5).max(0.0);
            let (x0, y0) = (sx.floor() as usize, sy.floor() as usize);
            let (x1, y1) = ((x0 + 1).min(3), (y0 + 1).min(1));
            let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
            let at = |xx: usize, yy: usize| g[yy * 4 + xx] as f32;
            let top = at(x0, y0) + (at(x1, y0) - at(x0, y0)) * fx;
            let bot = at(x0, y1) + (at(x1, y1) - at(x0, y1)) * fx;
            let want = (top + (bot - top) * fy).round() as u8;
            assert_eq!(px[(y * 12 + x) * 4], want, "({x}, {y})");
        }
    }
    let half = decode_jpm_page(
        &file,
        0,
        &JpmOptions {
            reduce: 1,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!((half.width, half.height, half.reduce), (6, 3, 1));

    // A JPEG 2000 background at 1/4: the object is decoded with two resolution levels
    // dropped, which is exactly the page at 1/4.
    let (gh, gcs) = jp2_parts("gray_rev.jp2");
    let lobj = Lobj {
        id: 1,
        win: [97, 131, 0, 0],
        objs: vec![Obj {
            ty: 1,
            stream: Some(0),
            jp2h: gh,
            ..Default::default()
        }],
    };
    let file = write(&[gcs], &[page(131, 97, 1, vec![lobj])], false, None);
    let q = decode_jpm_page(
        &file,
        0,
        &JpmOptions {
            reduce: 2,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!((q.width, q.height), (33, 25));
    let own_q = specular_jp2::decode(
        &fixture("gray_rev.jp2"),
        &DecodeOptions {
            reduce: 2,
            ..Default::default()
        },
    )
    .unwrap();
    let Pixels::U8(g) = own_q.pixels else {
        unreachable!()
    };
    let qp: Vec<u8> = rgba(&q).chunks_exact(4).map(|p| p[0]).collect();
    assert_eq!(qp, g);
}

#[test]
fn orientation_rotates_the_page() {
    let g: Vec<u8> = (0..6u8).map(|i| i * 40).collect();
    let mk = |or: u16| {
        let lobj = Lobj {
            id: 1,
            win: [2, 3, 0, 0],
            objs: vec![Obj {
                ty: 1,
                stream: Some(0),
                jp2h: ihdr(3, 2, 1, 8, 0),
                ..Default::default()
            }],
        };
        let mut p = page(3, 2, 1, vec![lobj]);
        p.orientation = or;
        let img = decode_jpm_page(
            &write(std::slice::from_ref(&g), &[p], false, None),
            0,
            &Default::default(),
        )
        .unwrap();
        let v: Vec<u8> = rgba(&img).chunks_exact(4).map(|p| p[0]).collect();
        (img.width, img.height, v)
    };
    assert_eq!(mk(1), (3, 2, vec![0, 40, 80, 120, 160, 200]));
    // 90 degrees clockwise: the left column becomes the top row, read bottom-up.
    assert_eq!(mk(2), (2, 3, vec![120, 0, 160, 40, 200, 80]));
    assert_eq!(mk(3), (3, 2, vec![200, 160, 120, 80, 40, 0]));
    assert_eq!(mk(4), (2, 3, vec![80, 200, 40, 160, 0, 120]));
}

#[test]
fn other_coders_go_to_the_foreign_decoder() {
    let lobj = Lobj {
        id: 1,
        win: [4, 4, 0, 0],
        objs: vec![Obj {
            ty: 0,
            stream: Some(0),
            jp2h: ihdr(4, 4, 1, 1, 3),
            ..Default::default()
        }],
    };
    let file = write(&[vec![1, 2, 3]], &[page(4, 4, 1, vec![lobj])], false, None);
    assert_eq!(
        decode_jpm_page(&file, 0, &JpmOptions::default()),
        Err(Jp2Error::Unsupported("JPM object coded with fax T.6 (MMR)"))
    );
    let hook = |o: &ForeignObject<'_>| -> Option<Image> {
        assert_eq!(o.coder, JpmCoder::Mmr);
        assert_eq!(o.data, &[1, 2, 3]);
        Some(Image {
            width: 4,
            height: 4,
            origin: (0, 0),
            format: PixelFormat::Gray8,
            pixels: Pixels::U8((0..16).map(|i| if i % 2 == 0 { 255 } else { 0 }).collect()),
            truncated: false,
            reduce: 0,
        })
    };
    let img = decode_jpm_page(
        &file,
        0,
        &JpmOptions {
            foreign: Some(&hook),
            ..Default::default()
        },
    )
    .unwrap();
    let v: Vec<u8> = rgba(&img).chunks_exact(4).map(|p| p[0]).collect();
    // Black (1 = opaque, base black) on white.
    assert_eq!(
        v,
        (0..16)
            .map(|i| if i % 2 == 0 { 0 } else { 255 })
            .collect::<Vec<u8>>()
    );
}

#[test]
fn pages_are_counted_and_damage_never_panics() {
    let (h, cs, w0, h0) = j2k_parts("small_rev.j2k");
    let mk = |id| Lobj {
        id,
        win: [h0, w0, 0, 0],
        objs: vec![Obj {
            ty: 1,
            stream: Some(0),
            jp2h: h.clone(),
            ..Default::default()
        }],
    };
    let file = write(
        &[cs],
        &[
            page(w0, h0, 1, vec![mk(1)]),
            page(w0 + 1, h0, 2, vec![mk(1)]),
        ],
        false,
        None,
    );
    let info = read_jpm_info(&file).unwrap();
    assert_eq!(info.pages.len(), 2);
    assert_eq!(info.profile, 1);
    assert_eq!(
        decode_jpm_page(&file, 1, &Default::default())
            .unwrap()
            .width,
        w0 + 1
    );
    assert!(matches!(
        decode_jpm_page(&file, 2, &Default::default()),
        Err(Jp2Error::InvalidRequest(_))
    ));
    for cut in (0..file.len()).step_by(97) {
        let _ = decode_jpm_page(&file[..cut], 0, &Default::default());
        let _ = read_jpm_info(&file[..cut]);
    }
    let mut f = file.clone();
    for i in (12..f.len()).step_by(13) {
        f[i] ^= 0x5A;
        let _ = decode_jpm_page(&f, 0, &Default::default());
        let _ = decode_jpm_page(&f, 1, &Default::default());
    }
}
