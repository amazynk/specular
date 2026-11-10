//! Synthetic PaintShop Pro files written here, for every structure the corpus lacks: groups,
//! user masks, mask layers, blend modes, 48-bit, palettes, both header layouts, composites,
//! hostile input.

use specular_psp as sp;
use std::io::Write;

#[derive(Clone, Copy)]
struct W {
    major: u16,
    comp: u16,
}

fn u16le(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn u32le(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn rect(v: &mut Vec<u8>, r: (i32, i32, i32, i32)) {
    for x in [r.0, r.1, r.2, r.3] {
        v.extend_from_slice(&x.to_le_bytes());
    }
}

fn rle(d: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < d.len() {
        let mut run = 1;
        while i + run < d.len() && run < 127 && d[i + run] == d[i] {
            run += 1;
        }
        if run >= 3 {
            out.push(128 + run as u8);
            out.push(d[i]);
            i += run;
        } else {
            let n = (d.len() - i).min(100);
            out.push(n as u8);
            out.extend_from_slice(&d[i..i + n]);
            i += n;
        }
    }
    out
}

fn zlib(d: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(d).unwrap();
    e.finish().unwrap()
}

impl W {
    fn blk(&self, id: u16, first: &[u8], rest: &[u8]) -> Vec<u8> {
        let mut body = Vec::new();
        if self.major <= 3 {
            body.extend_from_slice(first);
        } else {
            u32le(&mut body, first.len() as u32 + 4);
            body.extend_from_slice(first);
        }
        body.extend_from_slice(rest);
        let mut out = b"~BK\0".to_vec();
        u16le(&mut out, id);
        if self.major <= 3 {
            u32le(&mut out, first.len() as u32);
        }
        u32le(&mut out, body.len() as u32);
        out.extend_from_slice(&body);
        out
    }

    fn pack(&self, d: &[u8], comp: u16) -> Vec<u8> {
        match comp {
            1 => rle(d),
            2 => zlib(d),
            _ => d.to_vec(),
        }
    }

    fn channel(&self, d: &[u8], dib: u16, kind: u16, comp: u16) -> Vec<u8> {
        let p = self.pack(d, comp);
        let mut info = Vec::new();
        u32le(&mut info, p.len() as u32);
        u32le(&mut info, d.len() as u32);
        u16le(&mut info, dib);
        u16le(&mut info, kind);
        self.blk(5, &info, &p)
    }

    fn header(&self, w: u32, h: u32, bd: u16, grey: bool, layers: u16) -> Vec<u8> {
        let mut f = sp::SIGNATURE.to_vec();
        u16le(&mut f, self.major);
        u16le(&mut f, 0);
        let mut a = Vec::new();
        a.extend_from_slice(&(w as i32).to_le_bytes());
        a.extend_from_slice(&(h as i32).to_le_bytes());
        a.extend_from_slice(&72.0f64.to_le_bytes());
        a.push(1);
        u16le(&mut a, self.comp);
        u16le(&mut a, bd);
        u16le(&mut a, 1);
        u32le(&mut a, 1 << bd.min(24));
        a.push(grey as u8);
        u32le(&mut a, 0);
        u32le(&mut a, 0);
        u16le(&mut a, layers);
        if self.major > 3 {
            u32le(&mut a, 1);
        }
        f.extend(self.blk(0, &a, &[]));
        f
    }
}

type Rect = (i32, i32, i32, i32);

#[derive(Clone)]
struct L {
    name: &'static str,
    typ: u8,
    rect: (i32, i32, i32, i32),
    opacity: u8,
    blend: u8,
    visible: bool,
    /// Planes: (dib, kind, bytes).
    chans: Vec<(u16, u16, Vec<u8>)>,
    /// Mask rectangle, disabled, inverted.
    mask: Option<(Rect, bool, bool)>,
    group: Option<u32>,
}

fn layer(name: &'static str, rect: (i32, i32, i32, i32), rgb: [u8; 3], alpha: u8) -> L {
    let n = ((rect.2 - rect.0) * (rect.3 - rect.1)) as usize;
    L {
        name,
        typ: 1,
        rect,
        opacity: 255,
        blend: 0,
        visible: true,
        chans: vec![
            (0, 1, vec![rgb[0]; n]),
            (0, 2, vec![rgb[1]; n]),
            (0, 3, vec![rgb[2]; n]),
            (1, 0, vec![alpha; n]),
        ],
        mask: None,
        group: None,
    }
}

impl W {
    fn layer_block(&self, l: &L) -> Vec<u8> {
        let mut i = Vec::new();
        if self.major <= 3 {
            let mut name = [0u8; 256];
            name[..l.name.len()].copy_from_slice(l.name.as_bytes());
            i.extend_from_slice(&name);
            i.push(if l.typ == 1 { 0 } else { l.typ });
        } else {
            u16le(&mut i, l.name.len() as u16);
            i.extend_from_slice(l.name.as_bytes());
            i.push(l.typ);
        }
        // Image rectangle = saved rectangle (relative saved rect at 0,0).
        rect(&mut i, l.rect);
        rect(&mut i, (0, 0, l.rect.2 - l.rect.0, l.rect.3 - l.rect.1));
        i.push(l.opacity);
        i.push(l.blend);
        i.push(if self.major <= 3 {
            l.visible as u8
        } else {
            (l.visible as u8) | (l.mask.is_some() as u8) << 1
        });
        i.push(0);
        i.push(0);
        let (mr, disabled, invert) = l.mask.unwrap_or(((0, 0, 0, 0), false, false));
        rect(&mut i, mr);
        rect(&mut i, (0, 0, mr.2 - mr.0, mr.3 - mr.1));
        i.push(1);
        i.push(disabled as u8);
        i.push(invert as u8);
        u16le(&mut i, 5);
        i.extend_from_slice(&[0; 40]);
        let mut rest = Vec::new();
        if self.major <= 3 {
            u16le(&mut i, 1);
            u16le(&mut i, l.chans.len() as u16);
        } else {
            i.push(0);
            u32le(&mut i, 0);
            if let Some(n) = l.group {
                let mut g = Vec::new();
                u32le(&mut g, n);
                g.push(0);
                rest.extend(self.blk(25, &g, &[]));
            }
            if l.typ == 3 {
                let mut v = Vec::new();
                u32le(&mut v, 0);
                rest.extend(self.blk(13, &v, &[]));
            }
            u32le(&mut rest, 8);
            u16le(&mut rest, 1);
            u16le(&mut rest, l.chans.len() as u16);
        }
        for (dib, kind, d) in &l.chans {
            rest.extend(self.channel(d, *dib, *kind, self.comp));
        }
        self.blk(4, &i, &rest)
    }

    /// A file of `layers` (bottom first) with an optional lossless composite.
    fn file(&self, w: u32, h: u32, bd: u16, layers: &[L], composite: Option<[u8; 3]>) -> Vec<u8> {
        let mut f = self.header(w, h, bd, false, layers.len() as u16);
        if let Some(c) = composite
            && self.major > 3
        {
            let n = (w * h) as usize;
            let mut attrs = Vec::new();
            for (kind, cw, ch) in [(1u16, 2u32, 2u32), (0, w, h)] {
                let mut a = Vec::new();
                a.extend_from_slice(&(cw as i32).to_le_bytes());
                a.extend_from_slice(&(ch as i32).to_le_bytes());
                u16le(&mut a, 24);
                u16le(&mut a, 2);
                u16le(&mut a, 1);
                u32le(&mut a, 1 << 24);
                u16le(&mut a, kind);
                attrs.extend(self.blk(17, &a, &[]));
            }
            let mut images = Vec::new();
            for (dib, m) in [(5u16, 4usize), (8, n)] {
                let mut ci = Vec::new();
                u16le(&mut ci, 1);
                u16le(&mut ci, 3);
                let mut chans = Vec::new();
                for k in 1..=3u16 {
                    chans.extend(self.channel(&vec![c[k as usize - 1]; m], dib, k, 2));
                }
                images.extend(self.blk(9, &ci, &chans));
            }
            let mut bi = Vec::new();
            u32le(&mut bi, 2);
            attrs.extend(images);
            f.extend(self.blk(16, &bi, &attrs));
        }
        let mut bank = Vec::new();
        for l in layers {
            bank.extend(self.layer_block(l));
        }
        f.extend(self.blk(3, &[], &bank));
        f
    }
}

fn px(img: &sp::Image, x: u32, y: u32) -> [u8; 4] {
    match img.format {
        sp::PixelFormat::Rgba8 => {
            let i = ((y * img.width + x) * 4) as usize;
            img.data[i..i + 4].try_into().unwrap()
        }
        sp::PixelFormat::Gray8 => {
            let g = img.data[(y * img.width + x) as usize];
            [g, g, g, 255]
        }
        sp::PixelFormat::Rgba16 => {
            let i = ((y * img.width + x) * 8) as usize;
            let c = |k: usize| {
                (u16::from_ne_bytes([img.data[i + 2 * k], img.data[i + 2 * k + 1]]) >> 8) as u8
            };
            [c(0), c(1), c(2), c(3)]
        }
    }
}

fn layers_only() -> sp::Options<'static> {
    sp::Options {
        prefer_composite: false,
        ..Default::default()
    }
}

fn near(a: [u8; 4], b: [u8; 4]) -> bool {
    a.iter()
        .zip(b)
        .all(|(x, y)| (i32::from(*x) - i32::from(y)).abs() <= 1)
}

#[test]
fn two_layers_offsets_opacity_all_compressions_both_layouts() {
    for major in [3u16, 4, 6, 14] {
        for comp in [0u16, 1, 2] {
            let w = W { major, comp };
            let bg = layer("Background", (0, 0, 8, 6), [200, 100, 0], 255);
            let mut top = layer("Top", (2, 1, 6, 4), [0, 0, 255], 255);
            top.opacity = 128;
            let f = w.file(8, 6, 24, &[bg, top], None);
            let p = sp::parse(&f).unwrap();
            assert_eq!(p.info().layer_count, 2);
            assert_eq!(p.layers()[0].name, "Top");
            assert_eq!(p.layers()[0].rect, (2, 1, 4, 3));
            assert_eq!(p.pixel_layers(), vec![0, 1]);
            let img = p.render(&f, &sp::Options::default()).unwrap();
            assert_eq!((img.width, img.height), (8, 6));
            assert!(!img.truncated, "v{major} comp {comp}");
            assert_eq!(px(&img, 0, 0), [200, 100, 0, 255]);
            assert!(
                near(px(&img, 3, 2), [100, 50, 128, 255]),
                "{:?}",
                px(&img, 3, 2)
            );
            // The layer page: alone, with its opacity.
            let l = p.render_layer(&f, 0, &sp::Options::default()).unwrap();
            assert_eq!(px(&l, 0, 0)[3], 0);
            assert!(near(px(&l, 3, 2), [0, 0, 255, 128]));
        }
    }
}

#[test]
fn blend_modes() {
    let w = W { major: 6, comp: 2 };
    // Backdrop (64, 128, 192), source (128, 128, 128).
    let cases: [(u8, [u8; 3]); 8] = [
        (1, [64, 128, 128]),   // darken
        (2, [128, 128, 192]),  // lighten
        (7, [32, 64, 96]),     // multiply
        (8, [160, 192, 224]),  // screen
        (13, [64, 0, 64]),     // difference
        (16, [128, 128, 128]), // exclusion: b + s - 2bs at s = 0.5 is 0.5
        (11, [64, 128, 192]),  // hard light at 0.5 is the backdrop
        (0, [128, 128, 128]),  // normal
    ];
    for (mode, want) in cases {
        let bg = layer("bg", (0, 0, 2, 2), [64, 128, 192], 255);
        let mut top = layer("top", (0, 0, 2, 2), [128, 128, 128], 255);
        top.blend = mode;
        let f = w.file(2, 2, 24, &[bg, top], None);
        let p = sp::parse(&f).unwrap();
        let img = p.render(&f, &layers_only()).unwrap();
        let got = px(&img, 1, 1);
        assert!(
            near(got, [want[0], want[1], want[2], 255]),
            "mode {mode}: {got:?}"
        );
    }
    // Every mode, including the colour modes and unknown numbers, renders.
    for mode in (0..=24).chain([255]) {
        let bg = layer("bg", (0, 0, 2, 2), [64, 128, 192], 255);
        let mut top = layer("top", (0, 0, 2, 2), [200, 30, 90], 200);
        top.blend = mode;
        let f = w.file(2, 2, 24, &[bg, top], None);
        sp::parse(&f).unwrap().render(&f, &layers_only()).unwrap();
    }
}

#[test]
fn groups_masks_and_mask_layers() {
    let w = W { major: 6, comp: 2 };
    let bg = layer("bg", (0, 0, 4, 4), [0, 0, 0], 255);
    let mut group = layer("Group", (0, 0, 0, 0), [0; 3], 0);
    group.typ = 5;
    group.chans.clear();
    group.group = Some(1);
    group.opacity = 128;
    let child = layer("child", (0, 0, 4, 4), [255, 255, 255], 255);
    let f = w.file(4, 4, 24, &[bg.clone(), group, child], None);
    let p = sp::parse(&f).unwrap();
    let names: Vec<_> = p
        .layers()
        .iter()
        .map(|l| (l.name.as_str(), l.depth))
        .collect();
    assert_eq!(names, [("Group", 0), ("child", 1), ("bg", 0)]);
    assert_eq!(p.pixel_layers(), vec![1, 2]);
    let img = p.render(&f, &layers_only()).unwrap();
    assert!(
        near(px(&img, 0, 0), [128, 128, 128, 255]),
        "{:?}",
        px(&img, 0, 0)
    );

    // A user mask hiding the left half; inverted; disabled.
    for (invert, disabled, left, right) in [
        (false, false, 0u8, 255u8),
        (true, false, 255, 0),
        (false, true, 255, 255),
    ] {
        let mut top = layer("top", (0, 0, 4, 4), [255, 0, 0], 255);
        let mut m = vec![0u8; 16];
        for y in 0..4 {
            for x in 2..4 {
                m[y * 4 + x] = 255;
            }
        }
        top.chans.push((2, 0, m));
        top.mask = Some(((0, 0, 4, 4), disabled, invert));
        let f = w.file(4, 4, 24, &[bg.clone(), top], None);
        let p = sp::parse(&f).unwrap();
        assert!(p.layers()[0].has_mask);
        let img = p.render(&f, &layers_only()).unwrap();
        assert_eq!(
            px(&img, 0, 0)[0],
            left,
            "invert {invert} disabled {disabled}"
        );
        assert_eq!(px(&img, 3, 0)[0], right);
    }

    // A mask layer above a layer hides it where the mask is black.
    let red = layer("red", (0, 0, 4, 1), [255, 0, 0], 255);
    let mut mask = layer("mask", (0, 0, 4, 1), [0; 3], 255);
    mask.typ = 6;
    mask.chans = vec![(0, 0, vec![0, 0, 255, 255]), (1, 0, vec![255; 4])];
    let f = w.file(4, 1, 8, &[red.clone(), mask], None);
    let p = sp::parse(&f);
    // An 8-bit file with RGB channels is not what writers make; use 24-bit for this one.
    drop(p);
    let mut mask = layer("mask", (0, 0, 4, 1), [0; 3], 255);
    mask.typ = 6;
    mask.chans = vec![
        (0, 1, vec![0, 0, 255, 255]),
        (0, 2, vec![0, 0, 255, 255]),
        (0, 3, vec![0, 0, 255, 255]),
        (1, 0, vec![255; 4]),
    ];
    let f = w.file(4, 1, 24, &[red, mask], None);
    let p = sp::parse(&f).unwrap();
    let img = p.render(&f, &layers_only()).unwrap();
    assert_eq!(px(&img, 0, 0)[3], 0);
    assert_eq!(px(&img, 3, 0), [255, 0, 0, 255]);
}

#[test]
fn palette_and_grey_documents() {
    for comp in [0u16, 1, 2] {
        let w = W { major: 5, comp };
        // 4-bit, 5 wide: 3 bytes per row padded to 4.
        let mut f = w.header(5, 2, 4, false, 1);
        let mut pal = Vec::new();
        u32le(&mut pal, 16);
        let mut entries = Vec::new();
        for i in 0..16u8 {
            entries.extend_from_slice(&[i, 0, 255 - i * 10, 0]); // B, G, R, 0
        }
        f.extend(w.blk(2, &pal, &entries));
        let rows = [[0x12u8, 0x34, 0x50, 0], [0xff, 0xee, 0xd0, 0]];
        let mut l = layer("Background", (0, 0, 5, 2), [0; 3], 255);
        l.chans = vec![(0, 0, rows.concat())];
        f.extend(w.blk(3, &[], &w.layer_block(&l)));
        let p = sp::parse(&f).unwrap();
        assert!(p.info().palette);
        let img = p.render(&f, &sp::Options::default()).unwrap();
        assert_eq!(px(&img, 0, 0), [255 - 10, 0, 1, 255]);
        assert_eq!(px(&img, 4, 0), [255 - 50, 0, 5, 255]);
        assert_eq!(px(&img, 4, 1), [255 - 130, 0, 13, 255]);
    }
    // 1-bit with a 2-colour palette, 10 wide (2 bytes per row padded to 4).
    let w = W { major: 6, comp: 0 };
    let mut f = w.header(10, 1, 1, false, 1);
    let mut pal = Vec::new();
    u32le(&mut pal, 2);
    f.extend(w.blk(2, &pal, &[0, 0, 0, 0, 255, 255, 255, 0]));
    let mut l = layer("Background", (0, 0, 10, 1), [0; 3], 255);
    l.chans = vec![(0, 0, vec![0b1010_0000, 0b0100_0000, 0, 0])];
    f.extend(w.blk(3, &[], &w.layer_block(&l)));
    let img = sp::parse(&f)
        .unwrap()
        .render(&f, &sp::Options::default())
        .unwrap();
    assert_eq!(px(&img, 0, 0)[0], 255);
    assert_eq!(px(&img, 1, 0)[0], 0);
    assert_eq!(px(&img, 9, 0)[0], 255);
    // 8-bit grey without a palette: grey output.
    let mut f = w.header(3, 1, 8, true, 1);
    let mut l = layer("Background", (0, 0, 3, 1), [0; 3], 255);
    l.chans = vec![(0, 0, vec![0, 128, 255])];
    f.extend(w.blk(3, &[], &w.layer_block(&l)));
    let p = sp::parse(&f).unwrap();
    assert!(p.info().greyscale);
    let img = p.render(&f, &sp::Options::default()).unwrap();
    assert_eq!(img.format, sp::PixelFormat::Gray8);
    assert_eq!(img.data, vec![0, 128, 255]);
}

#[test]
fn deep_colour_documents_render_16_bit() {
    let w = W { major: 8, comp: 2 };
    let mut l = layer("Background", (0, 0, 2, 1), [0; 3], 255);
    let s = |v: [u16; 2]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    l.chans = vec![
        (0, 1, s([65535, 1000])),
        (0, 2, s([0, 2000])),
        (0, 3, s([32768, 3000])),
        (1, 0, s([65535, 65535])),
    ];
    let f = w.file(2, 1, 48, &[l], None);
    let p = sp::parse(&f).unwrap();
    let img = p.render(&f, &sp::Options::default()).unwrap();
    assert_eq!(img.format, sp::PixelFormat::Rgba16);
    let v: Vec<u16> = img
        .data
        .chunks_exact(2)
        .map(|b| u16::from_ne_bytes([b[0], b[1]]))
        .collect();
    assert_eq!(v, [65535, 0, 32768, 65535, 1000, 2000, 3000, 65535]);
}

#[test]
fn stored_composite_is_preferred_and_vector_layers_are_named() {
    let w = W { major: 6, comp: 2 };
    let bg = layer("bg", (0, 0, 3, 3), [10, 20, 30], 255);
    let mut vec_layer = layer("Vector 1", (0, 0, 0, 0), [0; 3], 0);
    vec_layer.typ = 3;
    vec_layer.chans.clear();
    let f = w.file(3, 3, 24, &[bg, vec_layer], Some([1, 2, 3]));
    let p = sp::parse(&f).unwrap();
    let i = p.info();
    assert!(i.has_composite && i.has_thumbnail);
    assert_eq!(i.not_drawn.as_deref(), Some("vector layers are not drawn"));
    assert_eq!(p.pixel_layers(), vec![1]);
    let img = p.render(&f, &sp::Options::default()).unwrap();
    assert_eq!(px(&img, 2, 2), [1, 2, 3, 255]);
    let layered = p.render(&f, &layers_only()).unwrap();
    assert_eq!(px(&layered, 2, 2), [10, 20, 30, 255]);
    let t = p.thumbnail(&f).unwrap().unwrap();
    assert_eq!((t.width, t.height), (2, 2));
    assert_eq!(px(&t, 0, 0), [1, 2, 3, 255]);
}

#[test]
fn cancel_and_refusals() {
    let w = W { major: 6, comp: 2 };
    let f = w.file(4, 4, 24, &[layer("bg", (0, 0, 4, 4), [1, 2, 3], 255)], None);
    let p = sp::parse(&f).unwrap();
    let stop = || true;
    let o = sp::Options {
        cancel: Some(&stop),
        ..Default::default()
    };
    assert_eq!(p.render(&f, &o).unwrap_err(), sp::Error::Cancelled);
    assert_eq!(sp::parse(b"not a psp").unwrap_err(), sp::Error::NotPsp);
    // A file with only a vector layer and no composite says why it shows nothing.
    let mut v = layer("Vector 1", (0, 0, 0, 0), [0; 3], 0);
    v.typ = 3;
    v.chans.clear();
    let f = w.file(4, 4, 24, &[v], None);
    let e = sp::parse(&f)
        .unwrap()
        .render(&f, &sp::Options::default())
        .unwrap_err();
    assert_eq!(
        e.to_string(),
        "unsupported: PaintShop Pro file without a composite image: vector layers are not drawn"
    );
    // Over the size cap.
    let f = w.header(1 << 20, 1 << 20, 24, false, 0);
    assert_eq!(sp::parse(&f).unwrap_err(), sp::Error::TooLarge);
}

#[test]
fn hostile_input_never_panics() {
    let w = W { major: 6, comp: 2 };
    let mut top = layer("top", (1, 1, 3, 3), [9, 8, 7], 100);
    top.mask = Some(((0, 0, 4, 4), false, false));
    top.chans.push((2, 0, vec![128; 16]));
    let good = w.file(
        4,
        4,
        24,
        &[layer("bg", (0, 0, 4, 4), [1, 2, 3], 255), top],
        Some([4, 5, 6]),
    );
    let v3 =
        W { major: 3, comp: 1 }.file(4, 4, 24, &[layer("bg", (0, 0, 4, 4), [1, 2, 3], 255)], None);
    // Corrupt sizes may ask for huge canvases: a small budget refuses them quickly.
    let try_one = |b: &[u8]| specular_core::budget::with_limit(64 << 20, || try_inner(b));
    fn try_inner(b: &[u8]) {
        if let Ok(p) = sp::parse(b) {
            let _ = p.render(b, &sp::Options::default());
            let _ = p.render(b, &layers_only());
            let _ = p.thumbnail(b);
            for i in 0..p.layers().len() {
                let _ = p.render_layer(b, i, &sp::Options::default());
            }
        }
    }
    let mut seed = 0x2545_f491u32;
    for good in [good, v3] {
        for n in 0..good.len() {
            try_one(&good[..n]);
        }
        for _ in 0..2000 {
            let mut b = good.clone();
            for _ in 0..4 {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let at = seed as usize % b.len();
                b[at] = (seed >> 8) as u8;
            }
            try_one(&b);
        }
    }
    // Truncated channel data: the pixels decoded so far, marked truncated.
    let w0 = W { major: 6, comp: 0 };
    let f = w0.file(4, 4, 24, &[layer("bg", (0, 0, 4, 4), [1, 2, 3], 255)], None);
    let cut = &f[..f.len() - 6];
    let img = sp::parse(cut)
        .unwrap()
        .render(cut, &sp::Options::default())
        .unwrap();
    assert!(img.truncated);
}

/// Timing on a 12 MP, 6-layer file (run with --ignored --nocapture).
#[test]
#[ignore]
fn timing_large_document() {
    let w = W { major: 14, comp: 2 };
    let (cw, ch) = (4000i32, 3000i32);
    let mut layers = Vec::new();
    for k in 0..6u8 {
        let (x0, y0) = (k as i32 * 50, k as i32 * 40);
        let mut l = layer("layer", (x0, y0, cw - 100, ch - 100), [0; 3], 255);
        let n = ((cw - 100 - x0) * (ch - 100 - y0)) as usize;
        let mut s = 0x1234_5678u32 ^ u32::from(k);
        let mut noise = || {
            (0..n)
                .map(|i| {
                    s ^= s << 13;
                    s ^= s >> 17;
                    s ^= s << 5;
                    ((i / 7) as u8).wrapping_add((s & 7) as u8)
                })
                .collect::<Vec<u8>>()
        };
        l.chans = vec![
            (0, 1, noise()),
            (0, 2, noise()),
            (0, 3, noise()),
            (1, 0, vec![200; n]),
        ];
        l.blend = [0, 7, 8, 10, 13, 18][k as usize];
        l.opacity = 180;
        layers.push(l);
    }
    let f = w.file(cw as u32, ch as u32, 24, &layers, Some([1, 2, 3]));
    let p = sp::parse(&f).unwrap();
    for (what, o) in [
        ("stored composite", sp::Options::default()),
        ("layers", layers_only()),
    ] {
        let t = std::time::Instant::now();
        let img = p.render(&f, &o).unwrap();
        println!(
            "{what}: {}×{} in {:.0} ms",
            img.width,
            img.height,
            t.elapsed().as_secs_f64() * 1e3
        );
    }
}
