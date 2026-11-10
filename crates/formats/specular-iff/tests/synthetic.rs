//! Hand-built files for every layout and colour mode, checked against hand-computed pixels.

use specular_iff::{Error, Form, Kind, Masking, decode, info, probe};

fn chunk(id: &[u8; 4], d: &[u8]) -> Vec<u8> {
    let mut v = id.to_vec();
    v.extend_from_slice(&(d.len() as u32).to_be_bytes());
    v.extend_from_slice(d);
    if d.len() % 2 == 1 {
        v.push(0);
    }
    v
}

fn bmhd(w: u16, h: u16, planes: u8, masking: u8, comp: u8, transparent: u16) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&w.to_be_bytes());
    b.extend_from_slice(&h.to_be_bytes());
    b.extend_from_slice(&[0, 0, 0, 0, planes, masking, comp, 0]);
    b.extend_from_slice(&transparent.to_be_bytes());
    b.extend_from_slice(&[1, 1]);
    b.extend_from_slice(&w.to_be_bytes());
    b.extend_from_slice(&h.to_be_bytes());
    b
}

fn form(kind: &[u8; 4], chunks: &[Vec<u8>]) -> Vec<u8> {
    let body = chunks.concat();
    let mut f = b"FORM".to_vec();
    f.extend_from_slice(&((body.len() + 4) as u32).to_be_bytes());
    f.extend_from_slice(kind);
    f.extend_from_slice(&body);
    f
}

/// Interleaved ILBM rows for `vals` (`w` per row) with `planes` planes, plus a mask plane
/// built from `mask` when given.
fn planar(vals: &[u32], w: usize, planes: usize, mask: Option<&[bool]>) -> Vec<u8> {
    let rb = w.div_ceil(16) * 2;
    let mut out = Vec::new();
    for (y, row) in vals.chunks(w).enumerate() {
        for p in 0..planes {
            let mut line = vec![0u8; rb];
            for (x, &v) in row.iter().enumerate() {
                if v >> p & 1 == 1 {
                    line[x / 8] |= 0x80 >> (x % 8);
                }
            }
            out.extend(line);
        }
        if let Some(m) = mask {
            let mut line = vec![0u8; rb];
            for x in 0..w {
                if m[y * w + x] {
                    line[x / 8] |= 0x80 >> (x % 8);
                }
            }
            out.extend(line);
        }
    }
    out
}

/// ByteRun1 over the whole buffer as one stream (runs crossing rows on purpose).
fn byterun1(d: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < d.len() {
        let run = d[i..].iter().take(128).take_while(|&&b| b == d[i]).count();
        if run >= 2 {
            out.extend([(257 - run) as u8, d[i]]);
            i += run;
        } else {
            let s = i;
            while i < d.len() && i - s < 128 && !(i + 1 < d.len() && d[i] == d[i + 1]) {
                i += 1;
            }
            out.push((i - s - 1) as u8);
            out.extend_from_slice(&d[s..i]);
        }
    }
    out
}

fn px(img: &specular_iff::Image, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * img.width + x) * 4) as usize;
    img.pixels[i..i + 4].try_into().unwrap()
}

fn cmap(n: usize) -> Vec<u8> {
    (0..n)
        .flat_map(|i| [(i * 8) as u8, (255 - i * 4) as u8, (i * 3) as u8])
        .collect()
}

#[test]
fn probe_needs_a_known_form() {
    assert!(probe(b"FORM\0\0\0\x04ILBM"));
    assert!(probe(b"FORM\0\0\0\x04PBM "));
    assert!(probe(b"FORM\0\0\0\x04ACBM"));
    assert!(!probe(b"FORM\0\0\0\x04AIFF"));
    assert!(probe(b"FORM\0\0\0\x04DEEP"));
    assert!(probe(b"FORM\0\0\0\x04RGB8"));
    assert!(probe(b"FORM\0\0\0\x04RGBN"));
    assert!(probe(b"FORM\0\0\0\x04ANIM"));
    assert!(!probe(b"FORM\0\0\0\x048SVX"));
    assert!(!probe(b"FORM\0\0\0"));
    assert!(!probe(b""));
}

#[test]
fn compressed_and_raw_agree_with_runs_across_rows() {
    let (w, h) = (37usize, 5usize);
    let vals: Vec<u32> = (0..w * h).map(|i| ((i / 9) % 32) as u32).collect();
    let raw = planar(&vals, w, 5, None);
    let pal = cmap(32);
    for (comp, body) in [(0u8, raw.clone()), (1, byterun1(&raw))] {
        let f = form(
            b"ILBM",
            &[
                chunk(b"BMHD", &bmhd(w as u16, h as u16, 5, 0, comp, 0)),
                chunk(b"CMAP", &pal),
                chunk(b"BODY", &body),
            ],
        );
        let img = decode(&f).unwrap();
        assert!(!img.truncated);
        for (i, &v) in vals.iter().enumerate() {
            let v = v as usize;
            let p = &img.pixels[i * 4..i * 4 + 4];
            assert_eq!(p, [pal[v * 3], pal[v * 3 + 1], pal[v * 3 + 2], 255]);
        }
    }
}

#[test]
fn mask_plane_and_transparent_colour_become_alpha() {
    let (w, h) = (20usize, 2usize);
    let vals: Vec<u32> = (0..w * h).map(|i| (i % 4) as u32).collect();
    let mask: Vec<bool> = (0..w * h).map(|i| i % 3 != 0).collect();
    let f = form(
        b"ILBM",
        &[
            chunk(b"BMHD", &bmhd(w as u16, h as u16, 2, 1, 1, 0)),
            chunk(b"CMAP", &cmap(4)),
            chunk(b"BODY", &byterun1(&planar(&vals, w, 2, Some(&mask)))),
        ],
    );
    let i = info(&f).unwrap();
    assert_eq!(i.masking, Masking::Mask);
    assert!(i.has_alpha);
    let img = decode(&f).unwrap();
    for (k, m) in mask.iter().enumerate() {
        assert_eq!(img.pixels[k * 4 + 3], if *m { 255 } else { 0 }, "pixel {k}");
    }
    // Transparent colour 2.
    let f = form(
        b"ILBM",
        &[
            chunk(b"BMHD", &bmhd(w as u16, h as u16, 2, 2, 0, 2)),
            chunk(b"CMAP", &cmap(4)),
            chunk(b"BODY", &planar(&vals, w, 2, None)),
        ],
    );
    assert_eq!(info(&f).unwrap().masking, Masking::TransparentColor(2));
    let img = decode(&f).unwrap();
    for (k, v) in vals.iter().enumerate() {
        assert_eq!(img.pixels[k * 4 + 3], if *v == 2 { 0 } else { 255 });
    }
}

#[test]
fn extra_half_brite_halves_the_first_32() {
    let vals: Vec<u32> = vec![5, 37, 31, 63];
    let mut pal = cmap(32);
    pal[15..18].copy_from_slice(&[201, 100, 51]);
    // Junk in entries 32–63 must be ignored.
    pal.extend(std::iter::repeat_n(0xee, 96));
    let f = form(
        b"ILBM",
        &[
            chunk(b"BMHD", &bmhd(4, 1, 6, 0, 0, 0)),
            chunk(b"CMAP", &pal),
            chunk(b"CAMG", &0x80u32.to_be_bytes()),
            chunk(b"BODY", &planar(&vals, 4, 6, None)),
        ],
    );
    assert_eq!(info(&f).unwrap().kind, Kind::Ehb);
    let img = decode(&f).unwrap();
    assert_eq!(px(&img, 0, 0), [201, 100, 51, 255]);
    assert_eq!(px(&img, 1, 0), [100, 50, 25, 255]);
    assert_eq!(
        px(&img, 3, 0),
        [pal[93] >> 1, pal[94] >> 1, pal[95] >> 1, 255]
    );
}

#[test]
fn ham6_and_ham8_modify_the_previous_pixel() {
    // HAM6: palette 1, modify blue to 0xF, red to 0x3, green to 0x8.
    let vals: Vec<u32> = vec![1, 0x10 | 0xF, 0x20 | 0x3, 0x30 | 0x8, 2];
    let pal = [0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0x90];
    let f = form(
        b"ILBM",
        &[
            chunk(b"BMHD", &bmhd(5, 1, 6, 0, 0, 0)),
            chunk(b"CMAP", &pal),
            chunk(b"CAMG", &0x800u32.to_be_bytes()),
            chunk(b"BODY", &planar(&vals, 5, 6, None)),
        ],
    );
    assert_eq!(info(&f).unwrap().kind, Kind::Ham);
    let img = decode(&f).unwrap();
    // 4-bit CMAP widened: 0x40 → 0x44.
    assert_eq!(px(&img, 0, 0), [0x44, 0x55, 0x66, 255]);
    assert_eq!(px(&img, 1, 0), [0x44, 0x55, 0xff, 255]);
    assert_eq!(px(&img, 2, 0), [0x33, 0x55, 0xff, 255]);
    assert_eq!(px(&img, 3, 0), [0x33, 0x88, 0xff, 255]);
    assert_eq!(px(&img, 4, 0), [0x77, 0x88, 0x99, 255]);
    // HAM8: six data bits replace the top six, the low two stay.
    let vals: Vec<u32> = vec![1, 0x40 | 0x3F, 0x80 | 0x01];
    let pal = [0, 0, 0, 0x13, 0x27, 0x3b];
    let f = form(
        b"ILBM",
        &[
            chunk(b"BMHD", &bmhd(3, 1, 8, 0, 0, 0)),
            chunk(b"CMAP", &pal),
            chunk(b"CAMG", &0x800u32.to_be_bytes()),
            chunk(b"BODY", &planar(&vals, 3, 8, None)),
        ],
    );
    let img = decode(&f).unwrap();
    assert_eq!(px(&img, 0, 0), [0x13, 0x27, 0x3b, 255]);
    assert_eq!(px(&img, 1, 0), [0x13, 0x27, 0xff, 255]);
    assert_eq!(px(&img, 2, 0), [0x07, 0x27, 0xff, 255]);
}

#[test]
fn ham_and_ehb_guessed_without_camg() {
    let vals = vec![0u32; 4];
    for (n, kind) in [(16, Kind::Ham), (32, Kind::Ehb), (64, Kind::Indexed)] {
        let f = form(
            b"ILBM",
            &[
                chunk(b"BMHD", &bmhd(4, 1, 6, 0, 0, 0)),
                chunk(b"CMAP", &cmap(n)),
                chunk(b"BODY", &planar(&vals, 4, 6, None)),
            ],
        );
        assert_eq!(info(&f).unwrap().kind, kind, "{n} colours");
    }
}

#[test]
fn deep_24_and_32() {
    let vals: Vec<u32> = vec![0x00_11_22_33, 0x80_ff_00_7f, 0x00_00_00_00];
    for planes in [24u8, 32] {
        let f = form(
            b"ILBM",
            &[
                chunk(b"BMHD", &bmhd(3, 1, planes, 0, 1, 0)),
                chunk(b"BODY", &byterun1(&planar(&vals, 3, planes.into(), None))),
            ],
        );
        let img = decode(&f).unwrap();
        let a = |v: u8| if planes == 32 { v } else { 255 };
        assert_eq!(px(&img, 0, 0), [0x33, 0x22, 0x11, a(0)]);
        assert_eq!(px(&img, 1, 0), [0x7f, 0x00, 0xff, a(0x80)]);
    }
    // All-zero alpha planes mean "no alpha".
    let f = form(
        b"ILBM",
        &[
            chunk(b"BMHD", &bmhd(1, 1, 32, 0, 0, 0)),
            chunk(b"BODY", &planar(&[0x00_01_02_03], 1, 32, None)),
        ],
    );
    assert_eq!(px(&decode(&f).unwrap(), 0, 0), [3, 2, 1, 255]);
}

#[test]
fn pbm_and_acbm_layouts() {
    let (w, h) = (5usize, 3usize);
    let vals: Vec<u32> = (0..w * h).map(|i| (i * 7 % 16) as u32).collect();
    let pal = cmap(16);
    let expect = |img: &specular_iff::Image| {
        for (i, &v) in vals.iter().enumerate() {
            let v = v as usize;
            assert_eq!(
                &img.pixels[i * 4..i * 4 + 4],
                [pal[v * 3], pal[v * 3 + 1], pal[v * 3 + 2], 255]
            );
        }
    };
    // PBM: rows padded to even width.
    let mut chunky = Vec::new();
    for row in vals.chunks(w) {
        chunky.extend(row.iter().map(|&v| v as u8));
        chunky.push(0);
    }
    for (comp, body) in [(0u8, chunky.clone()), (1, byterun1(&chunky))] {
        let f = form(
            b"PBM ",
            &[
                chunk(b"BMHD", &bmhd(w as u16, h as u16, 8, 0, comp, 0)),
                chunk(b"CMAP", &pal),
                chunk(b"BODY", &body),
            ],
        );
        assert_eq!(info(&f).unwrap().form, Form::Pbm);
        expect(&decode(&f).unwrap());
    }
    // ACBM: plane 0 rows, then plane 1 rows, ...
    let rb = 2;
    let mut abit = Vec::new();
    for p in 0..4 {
        for row in vals.chunks(w) {
            let mut line = vec![0u8; rb];
            for (x, &v) in row.iter().enumerate() {
                if v >> p & 1 == 1 {
                    line[x / 8] |= 0x80 >> (x % 8);
                }
            }
            abit.extend(line);
        }
    }
    let f = form(
        b"ACBM",
        &[
            chunk(b"BMHD", &bmhd(w as u16, h as u16, 4, 0, 0, 0)),
            chunk(b"CMAP", &pal),
            chunk(b"ABIT", &abit),
        ],
    );
    assert_eq!(info(&f).unwrap().form, Form::Acbm);
    expect(&decode(&f).unwrap());
}

#[test]
fn short_body_is_flagged_and_grey() {
    let (w, h) = (16usize, 4usize);
    let vals = vec![1u32; w * h];
    let body = planar(&vals, w, 1, None);
    let f = form(
        b"ILBM",
        &[
            chunk(b"BMHD", &bmhd(w as u16, h as u16, 1, 0, 0, 0)),
            chunk(b"CMAP", &[0, 0, 0, 255, 255, 255]),
            chunk(b"BODY", &body[..body.len() / 2]),
        ],
    );
    let img = decode(&f).unwrap();
    assert!(img.truncated);
    assert_eq!(px(&img, 0, 0), [255, 255, 255, 255]);
    assert_eq!(px(&img, 0, 3), [128, 128, 128, 255]);
}

#[test]
fn unsupported_and_malformed() {
    let f = |b: Vec<u8>| form(b"ILBM", &[chunk(b"BMHD", &b), chunk(b"BODY", &[0; 64])]);
    assert!(matches!(
        decode(&f(bmhd(4, 4, 4, 0, 3, 0))),
        Err(Error::Unsupported(_))
    ));
    assert!(matches!(
        decode(&f(bmhd(4, 4, 12, 0, 0, 0))),
        Err(Error::Unsupported(_))
    ));
    assert!(matches!(
        decode(&f(bmhd(0, 4, 4, 0, 0, 0))),
        Err(Error::Malformed(_))
    ));
    assert!(matches!(
        decode(&form(b"ILBM", &[chunk(b"BODY", &[0; 4])])),
        Err(Error::Malformed(_))
    ));
    assert_eq!(
        decode(&form(b"ILBM", &[chunk(b"BMHD", &[0; 10])])),
        Err(Error::Truncated("BMHD"))
    );
    // 65535 × 65535 is over the pixel budget; refused before any allocation.
    assert_eq!(
        decode(&f(bmhd(65535, 65535, 8, 0, 1, 0))),
        Err(Error::TooLarge)
    );
}
