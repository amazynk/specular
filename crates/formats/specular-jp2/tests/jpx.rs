//! JPX composition: synthetic files built here from the committed codestreams, drawn by
//! `decode_composite` and checked against a plain reference composite of the codestreams'
//! own decodes (and hayro's decode of one layer).

mod common;

use common::*;
use specular_jp2::{
    Compositor, DecodeOptions, Depth, Image, Jp2Error, Pixels, animation, codestream_count,
    composition, decode_at, decode_composite, defragment,
};
use std::sync::atomic::AtomicBool;

fn jbox(t: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut b = ((8 + payload.len()) as u32).to_be_bytes().to_vec();
    b.extend_from_slice(t);
    b.extend_from_slice(payload);
    b
}

/// The raw codestream of a fixture (`.j2k` as is, a `.jp2`'s `jp2c` box).
fn codestream(name: &str) -> Vec<u8> {
    let d = std::fs::read(data_dir().join(name)).unwrap();
    if name.ends_with(".j2k") {
        return d;
    }
    let mut pos = 12;
    while pos + 8 <= d.len() {
        let len = u32::from_be_bytes(d[pos..pos + 4].try_into().unwrap()) as usize;
        let len = if len == 0 { d.len() - pos } else { len };
        if &d[pos + 4..pos + 8] == b"jp2c" {
            return d[pos + 8..pos + len].to_vec();
        }
        pos += len;
    }
    panic!("{name}: no codestream")
}

fn size(cs: &[u8]) -> (u32, u32) {
    let i = specular_jp2::read_info(cs).unwrap();
    (i.width, i.height)
}

const SRGB: [u8; 7] = [1, 0, 0, 0, 0, 0, 16];
const GREY: [u8; 7] = [1, 0, 0, 0, 0, 0, 17];

/// A layer header: its colour and extra boxes (`cdef`, `opct`, `creg`).
fn jplh(colr: &[u8], extra: &[u8]) -> Vec<u8> {
    let mut c = jbox(b"cgrp", &jbox(b"colr", colr));
    c.extend_from_slice(extra);
    jbox(b"jplh", &c)
}

/// An `inst` box of instructions with every field (`Ityp` 0x27): `[XO, YO, WIDTH, HEIGHT,
/// LIFE, NEXT-USE, XC, YC, WC, HC]` each.
fn inst(rept: u16, ins: &[[u32; 10]]) -> Vec<u8> {
    let mut c = 0x27u16.to_be_bytes().to_vec();
    c.extend(rept.to_be_bytes());
    c.extend(0u32.to_be_bytes());
    for i in ins {
        c.extend(i.iter().flat_map(|v| v.to_be_bytes()));
    }
    jbox(b"inst", &c)
}

fn comp(w: u32, h: u32, insts: &[Vec<u8>]) -> Vec<u8> {
    let mut copt = h.to_be_bytes().to_vec();
    copt.extend(w.to_be_bytes());
    copt.push(0);
    let mut c = jbox(b"copt", &copt);
    for i in insts {
        c.extend_from_slice(i);
    }
    jbox(b"comp", &c)
}

/// A JPX file: the JP2 header from codestream 0 (sRGB, 3 × 8 bits), then `boxes`, then the
/// codestreams.
fn jpx(streams: &[&[u8]], boxes: &[Vec<u8>]) -> Vec<u8> {
    let (w, h) = size(streams[0]);
    let mut ihdr = h.to_be_bytes().to_vec();
    ihdr.extend(w.to_be_bytes());
    ihdr.extend(3u16.to_be_bytes());
    ihdr.extend([7, 7, 0, 0]);
    let mut hdr = jbox(b"ihdr", &ihdr);
    hdr.extend(jbox(b"colr", &SRGB));
    let mut f = jbox(b"jP  ", b"\r\n\x87\n");
    f.extend(jbox(b"ftyp", b"jpx \0\0\0\0jpx jp2 "));
    f.extend(jbox(b"jp2h", &hdr));
    for b in boxes {
        f.extend_from_slice(b);
    }
    for s in streams {
        f.extend(jbox(b"jp2c", s));
    }
    f
}

fn rgba8(img: &Image) -> &[u8] {
    match &img.pixels {
        Pixels::U8(p) => p,
        Pixels::U16(_) => panic!("16-bit"),
    }
}

/// Straight RGBA8 of a decoded layer (grey replicated, opaque without alpha).
fn layer_px(img: &Image, x: u32, y: u32) -> [f64; 4] {
    let p = rgba8(img);
    let i = (y * img.width + x) as usize;
    let v: [u8; 4] = match img.format.channels() {
        1 => [p[i], p[i], p[i], 255],
        _ => p[i * 4..i * 4 + 4].try_into().unwrap(),
    };
    v.map(|v| f64::from(v) / 255.0)
}

/// Reference composite at full resolution: every canvas pixel, every instruction, in f64.
fn reference(data: &[u8], w: u32, h: u32, ins: &[Drawn], key: Option<(usize, u8)>) -> Vec<u8> {
    let layers: Vec<Image> = (0..3)
        .map_while(|i| decode_at(data, i, &DecodeOptions::default()).ok())
        .collect();
    let mut out = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let mut a = [0f64; 4];
            for &(l, [xo, yo, dw, dh, _, _, xc, yc, wc, hc]) in ins {
                if x < xo || y < yo || x >= xo + dw || y >= yo + dh {
                    continue;
                }
                // Pixel centres: (x + 0.5 - xo) · wc / dw.
                let sx = xc + ((2 * (x - xo) + 1) * wc / (2 * dw));
                let sy = yc + ((2 * (y - yo) + 1) * hc / (2 * dh));
                let mut s = layer_px(&layers[l], sx, sy);
                if key.is_some_and(|(kl, v)| kl == l && (s[0] * 255.0).round() as u8 == v) {
                    s = [0.0; 4];
                }
                for c in 0..3 {
                    a[c] = s[c] * s[3] + a[c] * (1.0 - s[3]);
                }
                a[3] = s[3] + a[3] * (1.0 - s[3]);
            }
            let o = ((y * w + x) * 4) as usize;
            if a[3] > 0.0 {
                for c in 0..3 {
                    out[o + c] = ((a[c] / a[3]).min(1.0) * 255.0 + 0.5) as u8;
                }
                out[o + 3] = (a[3] * 255.0 + 0.5) as u8;
            }
        }
    }
    out
}

fn max_diff(a: &[u8], b: &[u8]) -> u8 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0)
}

/// Three layers: an opaque RGB image placed whole, a crop of a second scaled up, an RGBA
/// layer (`cdef` alpha) on top; a fourth instruction after the frame ends draws the first
/// layer again (`NEXT-USE` 3): a second frame.
/// A layer and its instruction fields (see [`inst`]).
type Drawn = (usize, [u32; 10]);

fn three_layers() -> (Vec<u8>, Vec<Drawn>, (u32, u32)) {
    let (a, b, c) = (
        codestream("rgb_rev.j2k"),
        codestream("small_rev.j2k"),
        codestream("rgba_rev.jp2"),
    );
    let ((aw, ah), (bw, bh), (cw, ch)) = (size(&a), size(&b), size(&c));
    let canvas = (aw + 40, ah + 30);
    let ins = vec![
        (0, [10, 5, aw, ah, 0, 3, 0, 0, aw, ah]),
        (
            1,
            [
                30,
                20,
                3 * (bw / 2),
                2 * (bh / 3),
                0,
                0,
                bw / 4,
                bh / 5,
                bw / 2,
                bh / 3,
            ],
        ),
        (
            2,
            [canvas.0 - cw, canvas.1 - ch, cw, ch, 5, 0, 0, 0, cw, ch],
        ),
        (0, [0, 0, aw, ah, 5, 0, 0, 0, aw, ah]),
    ];
    let mut cdef = 4u16.to_be_bytes().to_vec();
    for e in [[0u16, 0, 1], [1, 0, 2], [2, 0, 3], [3, 1, 0]] {
        cdef.extend(e.iter().flat_map(|v| v.to_be_bytes()));
    }
    let f = jpx(
        &[&a, &b, &c],
        &[
            jplh(&SRGB, &[]),
            jplh(&SRGB, &[]),
            jplh(&SRGB, &jbox(b"cdef", &cdef)),
            comp(
                canvas.0,
                canvas.1,
                &[inst(0, &ins.iter().map(|i| i.1).collect::<Vec<_>>())],
            ),
        ],
    );
    (f, ins, canvas)
}

#[test]
fn layers_are_cropped_scaled_and_placed() {
    let (f, ins, (w, h)) = three_layers();
    let comp = composition(&f).unwrap();
    assert_eq!((comp.width, comp.height), (w, h));
    assert_eq!(comp.placements.len(), 3);
    assert!(comp.animated);
    assert_eq!(comp.placements[1].layer, 1);
    assert_eq!(comp.placements[1].crop.map(|c| c[0]), Some(ins[1].1[6]));
    assert!(!comp.plain((w, h)));
    let img = decode_composite(&f, &comp, &DecodeOptions::default()).unwrap();
    assert_eq!((img.width, img.height, img.truncated), (w, h, false));
    let want = reference(&f, w, h, &ins[..3], None);
    assert!(max_diff(rgba8(&img), &want) <= 1);
    // Uncovered canvas stays transparent; the RGBA layer's alpha shows.
    assert_eq!(&rgba8(&img)[..4], &[0, 0, 0, 0]);
    assert!(rgba8(&img).chunks(4).any(|p| p[3] > 0 && p[3] < 255));

    // 16 bits: the same image, wider.
    let deep = decode_composite(
        &f,
        &comp,
        &DecodeOptions {
            depth: Depth::Sixteen,
            ..Default::default()
        },
    )
    .unwrap();
    let Pixels::U16(d) = &deep.pixels else {
        panic!()
    };
    let narrowed: Vec<u8> = d
        .iter()
        .map(|&v| ((u32::from(v) * 255 + 32767) / 65535) as u8)
        .collect();
    assert!(max_diff(&narrowed, rgba8(&img)) <= 1);

    // Reduced: the canvas at 1/2^r, drawn from layers decoded at that reduction.
    for r in 1..=3u8 {
        let small = decode_composite(
            &f,
            &comp,
            &DecodeOptions {
                reduce: r,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            (small.width, small.height),
            (w.div_ceil(1 << r), h.div_ceil(1 << r))
        );
        assert!(!small.truncated);
    }
}

/// Prints `ORACLE<TAB>JPX<TAB>case<TAB>pass|fail` when dropped (fail while a check panics),
/// for tools/ci/oracle-pass-rates.sh.
struct Verdict(&'static str);

impl Drop for Verdict {
    fn drop(&mut self) {
        let status = if std::thread::panicking() {
            "fail"
        } else {
            "pass"
        };
        println!("ORACLE\tJPX\t{}\t{status}", self.0);
    }
}

/// hayro-jpeg2000 as the oracle for one layer: the whole, unscaled first layer of the
/// composite is its lossless decode exactly.
#[test]
fn oracle_placed_layer_matches_hayro() {
    let _v = Verdict("placed layer vs hayro (rgb_rev.j2k)");
    let (f, ins, (w, _)) = three_layers();
    let comp = composition(&f).unwrap();
    let img = decode_composite(&f, &comp, &DecodeOptions::default()).unwrap();
    let a = codestream("rgb_rev.j2k");
    let h = hayro_jpeg2000::Image::new(&a, &hayro_jpeg2000::DecodeSettings::default()).unwrap();
    let px = h.decode().unwrap();
    let (aw, ah) = (h.width(), h.height());
    let [xo, yo, ..] = ins[0].1;
    let (bx0, by0) = (ins[1].1[0], ins[1].1[1]);
    let (bx1, by1) = (bx0 + ins[1].1[2], by0 + ins[1].1[3]);
    let out = rgba8(&img);
    let mut n = 0;
    for y in 0..ah {
        for x in 0..aw {
            let (cx, cy) = (x + xo, y + yo);
            // Not under the other layers.
            if (cx >= bx0 && cx < bx1 && cy >= by0 && cy < by1) || cx + 200 >= w {
                continue;
            }
            let o = ((cy * w + cx) * 4) as usize;
            let i = ((y * aw + x) * 3) as usize;
            assert_eq!(&out[o..o + 3], &px[i..i + 3], "({x}, {y})");
            n += 1;
        }
    }
    assert!(n > 1000);
}

#[test]
fn chroma_keys_are_transparent() {
    let g = codestream("gray_rev.jp2");
    let (gw, gh) = size(&g);
    let plain = decode_at(&g, 0, &DecodeOptions::default()).unwrap();
    let key = rgba8(&plain)[0];
    let ins = [(0, [0, 0, gw, gh, 0, 0, 0, 0, gw, gh])];
    // Grey, 8 bits: the key is one byte.
    let mut f = jpx(
        &[&g],
        &[
            jplh(&GREY, &jbox(b"opct", &[2, 1, key])),
            comp(gw, gh, &[inst(0, &[ins[0].1])]),
        ],
    );
    // The JP2 header says sRGB with 3 components; this layer is grey.
    let at = f.windows(4).position(|w| w == b"ihdr").unwrap() + 12;
    f[at..at + 2].copy_from_slice(&1u16.to_be_bytes());
    let comp = composition(&f).unwrap();
    assert!(comp.keyed(0) && !comp.plain((gw, gh)));
    let img = decode_composite(&f, &comp, &DecodeOptions::default()).unwrap();
    let want = reference(&f, gw, gh, &ins, Some((0, key)));
    assert_eq!(max_diff(rgba8(&img), &want), 0);
    let out = rgba8(&img);
    assert_eq!(out[3], 0);
    let keyed = out.chunks(4).filter(|p| p[3] == 0).count();
    let opaque = out.chunks(4).filter(|p| p[3] == 255).count();
    assert_eq!(keyed + opaque, (gw * gh) as usize);
    assert!(opaque > 0);
}

/// Files whose composition is missing, empty or registered in unsupported ways have none;
/// layers past the codestreams end the frame.
#[test]
fn unsupported_compositions_fall_back() {
    let a = codestream("small_rev.j2k");
    let (w, h) = size(&a);
    let whole = [0, 0, w, h, 1, 0, 0, 0, w, h];
    assert!(composition(&jpx(&[&a], &[])).is_none());
    let one = jpx(&[&a], &[comp(w, h, &[inst(0, &[whole])])]);
    let c = composition(&one).unwrap();
    assert!(c.plain((w, h)) && !c.animated);
    // Not JPX.
    let mut jp2 = one.clone();
    let at = jp2.windows(4).position(|w| w == b"ftyp").unwrap() + 4;
    jp2[at..at + 4].copy_from_slice(b"jp2 ");
    assert!(composition(&jp2).is_none());
    // An empty canvas, an instruction set without fields.
    assert!(composition(&jpx(&[&a], &[comp(0, h, &[inst(0, &[whole])])])).is_none());
    let mut bare = 0u16.to_be_bytes().to_vec();
    bare.extend([0; 6]);
    assert!(composition(&jpx(&[&a], &[comp(w, h, &[jbox(b"inst", &bare)])])).is_none());
    // A layer registered with an offset: drawn 4 pixels to the right, its first 4 columns
    // transparent.
    let mut creg = vec![0, 1, 0, 1, 0, 0, 1, 1, 4, 0];
    let off = jpx(
        &[&a],
        &[
            jplh(&SRGB, &jbox(b"creg", &creg)),
            comp(w, h, &[inst(0, &[whole])]),
        ],
    );
    let c = composition(&off).unwrap();
    assert!(!c.plain((w, h)));
    let img = decode_composite(&off, &c, &DecodeOptions::default()).unwrap();
    let plain = decode_at(&off, 0, &DecodeOptions::default()).unwrap();
    for y in [0, h / 2, h - 1] {
        assert_eq!(rgba8(&img)[(y * w * 4) as usize + 3], 0);
        for x in 4..w {
            let got = &rgba8(&img)[((y * w + x) * 4) as usize..][..3];
            let want = layer_px(&plain, x - 4, y);
            for c in 0..3 {
                assert!(
                    (f64::from(got[c]) - want[c] * 255.0).abs() <= 1.0,
                    "{x},{y}"
                );
            }
        }
    }
    // Several codestreams in one layer: not taken.
    let mut several = creg.clone();
    several.extend([0, 0, 1, 1, 0, 0]);
    let two = jpx(
        &[&a],
        &[
            jplh(&SRGB, &jbox(b"creg", &several)),
            comp(w, h, &[inst(0, &[whole])]),
        ],
    );
    assert!(composition(&two).is_none());
    creg[8] = 0;
    let own = jpx(
        &[&a],
        &[
            jplh(&SRGB, &jbox(b"creg", &creg)),
            comp(w, h, &[inst(0, &[whole])]),
        ],
    );
    assert!(composition(&own).is_some());
    // Two instructions but one codestream: the second has no layer.
    let short = jpx(
        &[&a],
        &[comp(
            2 * w,
            h,
            &[inst(
                0,
                &[
                    [0, 0, w, h, 0, 0, 0, 0, w, h],
                    [w, 0, w, h, 1, 0, 0, 0, w, h],
                ],
            )],
        )],
    );
    assert_eq!(composition(&short).unwrap().placements.len(), 1);
    // NEXT-USE: the first layer drawn again by the second instruction.
    let again = jpx(
        &[&a],
        &[comp(
            2 * w,
            h,
            &[inst(
                0,
                &[
                    [0, 0, w, h, 0, 1, 0, 0, w, h],
                    [w, 0, w, h, 1, 0, 0, 0, w, h],
                ],
            )],
        )],
    );
    let c = composition(&again).unwrap();
    assert_eq!(
        c.placements.iter().map(|p| p.layer).collect::<Vec<_>>(),
        [0, 0]
    );
    let img = decode_composite(&again, &c, &DecodeOptions::default()).unwrap();
    let p = rgba8(&img);
    let row = (2 * w * 4) as usize;
    assert_eq!(&p[..(w * 4) as usize], &p[(w * 4) as usize..row]);
    // Regions are refused; cancel stops.
    let region = DecodeOptions {
        region: Some(specular_jp2::Region {
            x: 0,
            y: 0,
            w: 1,
            h: 1,
        }),
        ..Default::default()
    };
    assert!(matches!(
        decode_composite(&again, &c, &region),
        Err(Jp2Error::InvalidRequest(_))
    ));
    let stop = AtomicBool::new(true);
    let cancel = DecodeOptions {
        cancel: Some(&stop),
        ..Default::default()
    };
    assert!(matches!(
        decode_composite(&again, &c, &cancel),
        Err(Jp2Error::Cancelled)
    ));
}

#[test]
fn damaged_compositions_never_panic() {
    let (f, _, _) = three_layers();
    for n in (0..f.len()).step_by(97) {
        if let Some(c) = composition(&f[..n]) {
            let _ = decode_composite(
                &f[..n],
                &c,
                &DecodeOptions {
                    reduce: 1,
                    ..Default::default()
                },
            );
        }
    }
    // Damaged bytes inside the composition box.
    let at = f.windows(4).position(|w| w == b"inst").unwrap();
    for i in at..at + 60 {
        let mut g = f.clone();
        g[i] ^= 0xa5;
        if let Some(c) = composition(&g)
            && u64::from(c.width) * u64::from(c.height) <= 1 << 22
        {
            let _ = decode_composite(&g, &c, &DecodeOptions::default());
        }
    }
}

/// `cargo test --release -p specular-jp2 --test jpx -- --ignored --nocapture`: three layers
/// scaled onto a 4096² canvas.
#[test]
#[ignore]
fn composite_speed() {
    let (a, b) = (codestream("rgb_rev.j2k"), codestream("rgba_rev.jp2"));
    let ((aw, ah), (bw, bh)) = (size(&a), size(&b));
    let n = 4096;
    let f = jpx(
        &[&a, &b, &a],
        &[
            jplh(&SRGB, &[]),
            jplh(&SRGB, &[]),
            jplh(&SRGB, &[]),
            comp(
                n,
                n,
                &[inst(
                    0,
                    &[
                        [0, 0, n, n, 0, 0, 0, 0, aw, ah],
                        [100, 100, n / 2, n / 2, 0, 0, 0, 0, bw, bh],
                        [n / 2, n / 2, n / 2, n / 2, 1, 0, 0, 0, aw, ah],
                    ],
                )],
            ),
        ],
    );
    let c = composition(&f).unwrap();
    let _ = decode_composite(&f, &c, &DecodeOptions::default()).unwrap();
    let t = std::time::Instant::now();
    let runs = 5;
    for _ in 0..runs {
        std::hint::black_box(decode_composite(&f, &c, &DecodeOptions::default()).unwrap());
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0 / f64::from(runs);
    println!(
        "JPX composite {n}²: {ms:.1} ms ({:.0} MP/s)",
        f64::from(n * n) / ms / 1000.0
    );
}

/// A layer registered to another codestream, at half scale: two layer pixels per codestream
/// sample become one (`XS` 2, `XR` 1).
#[test]
fn layers_registered_to_other_codestreams() {
    let (a, b) = (codestream("small_rev.j2k"), codestream("rgb_rev.j2k"));
    let (bw, bh) = size(&b);
    // Layer 0 shows codestream 1 halved.
    let creg = [0u8, 2, 0, 2, 0, 1, 1, 1, 0, 0];
    let (lw, lh) = (bw.div_ceil(2), bh.div_ceil(2));
    let f = jpx(
        &[&a, &b],
        &[
            jplh(&SRGB, &jbox(b"creg", &creg)),
            comp(lw, lh, &[inst(0, &[[0, 0, lw, lh, 1, 0, 0, 0, lw, lh]])]),
        ],
    );
    let c = composition(&f).unwrap();
    assert_eq!(c.codestream(0), 1);
    let img = decode_composite(&f, &c, &DecodeOptions::default()).unwrap();
    let full = decode_at(&f, 1, &DecodeOptions::default()).unwrap();
    for y in (0..lh).step_by(3) {
        for x in (0..lw).step_by(3) {
            // Layer pixel x's centre is grid point 2x + 1: sample 2x + 1 (the last clamps).
            let (sx, sy) = ((2 * x + 1).min(bw - 1), (2 * y + 1).min(bh - 1));
            let got = &rgba8(&img)[((y * lw + x) * 4) as usize..][..4];
            let want = layer_px(&full, sx, sy);
            for c in 0..3 {
                assert!(
                    (f64::from(got[c]) - want[c] * 255.0).abs() <= 1.0,
                    "{x},{y}"
                );
            }
            assert_eq!(got[3], 255);
        }
    }
}

/// Frames: times from `LIFE` × `TICK`, persistence, reuse across frames and endless
/// repetition, drawn with one [`Compositor`] that keeps the decoded layers.
#[test]
fn animation_frames_play_with_persistence() {
    let a = codestream("small_rev.j2k");
    let (w, h) = size(&a);
    let canvas = (2 * w, h);
    // Frame 1: layer 0 on the left, kept (persistence bit), 3 ticks. Frame 2: layer 1 on the
    // right, not kept, 2 ticks. Frame 3: layer 2 (codestream 2) on the right, shown for ever.
    let keep = 0x8000_0000u32;
    let ins = [
        [0, 0, w, h, keep | 3, 0, 0, 0, w, h],
        [w, 0, w, h, 2, 0, 0, 0, w, h],
        [w, 0, w, h, 0x7fff_ffff, 0, 0, 0, w, h],
    ];
    let mut set = inst(0, &ins);
    // TICK: 40 ms (the set's bytes 4..8, after the box header).
    set[12..16].copy_from_slice(&40u32.to_be_bytes());
    let f = jpx(
        &[&a, &a, &a],
        &[
            jplh(&SRGB, &[]),
            jplh(&SRGB, &[]),
            jplh(&SRGB, &[]),
            comp(canvas.0, canvas.1, &[set.clone()]),
        ],
    );
    let anim = animation(&f).unwrap();
    assert_eq!(anim.frames.len(), 3);
    assert_eq!(anim.loop_count, 1);
    let delays: Vec<_> = anim.frames.iter().map(|c| c.delay_ms).collect();
    assert_eq!(delays, [Some(120), Some(80), None]);
    let layers =
        |i: usize| -> Vec<usize> { anim.frames[i].placements.iter().map(|p| p.layer).collect() };
    assert_eq!(layers(0), [0]);
    assert_eq!(layers(1), [0, 1]);
    // Frame 2's layer was not kept: frame 3 has the kept layer 0 and its own.
    assert_eq!(layers(2), [0, 2]);
    assert!(composition(&f).unwrap().animated);
    let mut drawer = Compositor::default();
    let opts = DecodeOptions::default();
    let frames: Vec<Image> = anim
        .frames
        .iter()
        .map(|c| drawer.draw(&f, c, &opts).unwrap())
        .collect();
    assert!(drawer.memory_bytes() > 0);
    let left = |img: &Image| rgba8(img)[3];
    let right = |img: &Image| rgba8(img)[(w * 4 + 3) as usize];
    assert_eq!((left(&frames[0]), right(&frames[0])), (255, 0));
    assert_eq!((left(&frames[1]), right(&frames[1])), (255, 255));
    assert_eq!((left(&frames[2]), right(&frames[2])), (255, 255));
    // Drawn alone, each frame is the same.
    for (c, img) in anim.frames.iter().zip(&frames) {
        let again = decode_composite(&f, c, &opts).unwrap();
        assert_eq!(rgba8(&again), rgba8(img));
    }
    // An instruction set repeated without end loops the animation.
    let mut forever = inst(u16::MAX, &ins[..2]);
    forever[12..16].copy_from_slice(&40u32.to_be_bytes());
    let f = jpx(
        &[&a, &a],
        &[
            jplh(&SRGB, &[]),
            jplh(&SRGB, &[]),
            comp(canvas.0, canvas.1, &[forever]),
        ],
    );
    let anim = animation(&f).unwrap();
    assert_eq!((anim.frames.len(), anim.loop_count), (2, 0));
}

/// A codestream split into fragments (`ftbl` / `flst`) inside the file is joined; one whose
/// fragments lie in another file is refused.
#[test]
fn fragment_tables_are_joined() {
    let (a, b) = (codestream("small_rev.j2k"), codestream("rgb_rev.j2k"));
    // The file: header, codestream 0 whole, a fragment table for codestream 1, then the
    // three fragments of codestream 1 in a free box, out of order.
    let cut = [0, b.len() / 3, 2 * b.len() / 3, b.len()];
    let head = jpx(&[&a], &[]);
    let flst_len = 8 + 2 + 3 * 14;
    let ftbl_len = 8 + flst_len;
    let free_at = head.len() + ftbl_len;
    // Fragment k at free + 8 + its place in the box (stored as 2, 0, 1).
    let order = [2usize, 0, 1];
    let mut free = Vec::new();
    let mut at = [0usize; 3];
    for &k in &order {
        at[k] = free_at + 8 + free.len();
        free.extend_from_slice(&b[cut[k]..cut[k + 1]]);
    }
    let mut flst = 3u16.to_be_bytes().to_vec();
    for k in 0..3 {
        flst.extend((at[k] as u64).to_be_bytes());
        flst.extend(((cut[k + 1] - cut[k]) as u32).to_be_bytes());
        flst.extend(0u16.to_be_bytes());
    }
    let mut f = head.clone();
    f.extend(jbox(b"ftbl", &jbox(b"flst", &flst)));
    assert_eq!(f.len(), free_at);
    f.extend(jbox(b"free", &free));
    assert_eq!(codestream_count(&f), 2);
    let joined = defragment(&f).unwrap();
    assert_eq!(codestream_count(&joined), 2);
    let want = specular_jp2::decode(&b, &DecodeOptions::default()).unwrap();
    for data in [&f, &joined] {
        let got = decode_at(data, 1, &DecodeOptions::default()).unwrap();
        assert_eq!(got.pixels, want.pixels);
    }
    assert!(defragment(&head).is_none());
    // Fragments in another file (data reference 1), or past the end of the file: nothing
    // joined, that codestream refused, the other decodes.
    let mut other = f.clone();
    let dr = head.len() + 8 + 8 + 2 + 12;
    other[dr..dr + 2].copy_from_slice(&1u16.to_be_bytes());
    let mut past = f.clone();
    let off = head.len() + 8 + 8 + 2;
    past[off..off + 8].copy_from_slice(&(u64::MAX / 2).to_be_bytes());
    for bad in [&other, &past] {
        assert!(defragment(bad).is_none());
        assert!(matches!(
            decode_at(bad, 1, &DecodeOptions::default()),
            Err(Jp2Error::Unsupported(_))
        ));
        assert!(decode_at(bad, 0, &DecodeOptions::default()).is_ok());
    }
}

/// hayro's decode (RGB, 8-bit) of a codestream.
fn hayro(cs: &[u8]) -> (u32, u32, Vec<u8>) {
    let h = hayro_jpeg2000::Image::new(cs, &hayro_jpeg2000::DecodeSettings::default()).unwrap();
    let px = h.decode().unwrap();
    (h.width(), h.height(), px)
}

/// hayro as the oracle for a registered layer: codestream 1 drawn at half scale and an
/// offset of 3 grid points samples hayro's pixels exactly.
#[test]
fn oracle_registered_layer_matches_hayro() {
    let _v = Verdict("registered layer (other codestream, half scale, offset) vs hayro");
    let (a, b) = (codestream("small_rev.j2k"), codestream("rgb_rev.j2k"));
    let (bw, bh, px) = hayro(&b);
    let creg = [0u8, 2, 0, 2, 0, 1, 1, 1, 3, 0];
    let (lw, lh) = ((bw + 3).div_ceil(2), bh.div_ceil(2));
    let f = jpx(
        &[&a, &b],
        &[
            jplh(&SRGB, &jbox(b"creg", &creg)),
            comp(lw, lh, &[inst(0, &[[0, 0, lw, lh, 1, 0, 0, 0, lw, lh]])]),
        ],
    );
    let c = composition(&f).unwrap();
    let img = decode_composite(&f, &c, &DecodeOptions::default()).unwrap();
    let out = rgba8(&img);
    for y in 0..lh {
        for x in 0..lw {
            let o = ((y * lw + x) * 4) as usize;
            // Layer pixel x spans grid [2x, 2x + 2); the codestream starts at grid 3.
            if 2 * x + 2 <= 3 {
                assert_eq!(out[o + 3], 0, "({x}, {y}) is left of the codestream");
                continue;
            }
            // Its centre, grid point 2x + 1, is sample 2x + 1 - 3 (one grid point each).
            let sx = (2 * x + 1).saturating_sub(3).min(bw - 1);
            let sy = (2 * y + 1).min(bh - 1);
            let i = ((sy * bw + sx) * 3) as usize;
            assert_eq!(&out[o..o + 3], &px[i..i + 3], "({x}, {y})");
            assert_eq!(out[o + 3], 255);
        }
    }
}

/// hayro as the oracle for a fragmented codestream: joined, it decodes to hayro's pixels.
#[test]
fn oracle_fragmented_codestream_matches_hayro() {
    let _v = Verdict("fragment table joined vs hayro (rgb_rev.j2k)");
    let (a, b) = (codestream("small_rev.j2k"), codestream("rgb_rev.j2k"));
    let (bw, bh, px) = hayro(&b);
    let head = jpx(&[&a], &[]);
    // Two fragments, stored in reverse order after the table.
    let half = b.len() / 2;
    let table_len = 8 + 8 + 2 + 2 * 14;
    let free_at = head.len() + table_len + 8;
    let mut flst = 2u16.to_be_bytes().to_vec();
    for (off, len) in [
        (free_at + (b.len() - half), half),
        (free_at, b.len() - half),
    ] {
        flst.extend((off as u64).to_be_bytes());
        flst.extend((len as u32).to_be_bytes());
        flst.extend(0u16.to_be_bytes());
    }
    let mut free = b[half..].to_vec();
    free.extend_from_slice(&b[..half]);
    let mut f = head;
    f.extend(jbox(b"ftbl", &jbox(b"flst", &flst)));
    f.extend(jbox(b"free", &free));
    let img = decode_at(&f, 1, &DecodeOptions::default()).unwrap();
    assert_eq!((img.width, img.height), (bw, bh));
    let out = rgba8(&img);
    for (o, i) in out.chunks_exact(4).zip(px.chunks_exact(3)) {
        assert_eq!(&o[..3], i);
    }
}
