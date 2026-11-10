//! OpenRaster / Krita documents built in code: detection, header-only sizes, bounded
//! members and compositing (blend modes, operators, isolation) against hand-computed pixels
//! and the W3C formulas.

use std::io::Write;

use specular_ora::{Document, Error, Kind, detect, detect_file};

/// A ZIP writer for fixtures: `(name, data, deflate)` members in order.
fn zip(members: &[(&str, &[u8], bool)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut cd = Vec::new();
    for &(name, data, deflate) in members {
        let body = if deflate {
            let mut e =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            e.write_all(data).unwrap();
            e.finish().unwrap()
        } else {
            data.to_vec()
        };
        let mut crc = flate2::Crc::new();
        crc.update(data);
        let method: u16 = if deflate { 8 } else { 0 };
        let off = out.len() as u32;
        let common = |v: &mut Vec<u8>| {
            v.extend_from_slice(&[20, 0, 0, 0]);
            v.extend_from_slice(&method.to_le_bytes());
            v.extend_from_slice(&[0, 0, 0, 0]);
            v.extend_from_slice(&crc.sum().to_le_bytes());
            v.extend_from_slice(&(body.len() as u32).to_le_bytes());
            v.extend_from_slice(&(data.len() as u32).to_le_bytes());
            v.extend_from_slice(&(name.len() as u16).to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes());
        };
        out.extend_from_slice(b"PK\x03\x04");
        common(&mut out);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&body);
        cd.extend_from_slice(b"PK\x01\x02\x14\x00");
        common(&mut cd);
        cd.extend_from_slice(&[0; 10]);
        cd.extend_from_slice(&off.to_le_bytes());
        cd.extend_from_slice(name.as_bytes());
    }
    let cd_off = out.len() as u32;
    out.extend_from_slice(&cd);
    out.extend_from_slice(b"PK\x05\x06\0\0\0\0");
    out.extend_from_slice(&(members.len() as u16).to_le_bytes());
    out.extend_from_slice(&(members.len() as u16).to_le_bytes());
    out.extend_from_slice(&(cd.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn png(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    let f = &f;
    let px: Vec<u8> = (0..h)
        .flat_map(|y| (0..w).flat_map(move |x| f(x, y)))
        .collect();
    specular_png::encode(w, h, specular_png::ColorType::Rgba, &px).unwrap()
}

fn ora(stack: &str, layers: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut m: Vec<(&str, &[u8], bool)> = vec![
        ("mimetype", b"image/openraster", false),
        ("stack.xml", stack.as_bytes(), true),
    ];
    for (n, d) in layers {
        m.push((n, d, true));
    }
    zip(&m)
}

fn composite(f: &[u8]) -> Result<specular_ora::Rgba, Error> {
    Document::open(f).unwrap().composite(f, &|| false)
}

fn at(img: &specular_ora::Rgba, x: u32, y: u32) -> [u8; 4] {
    let p = ((y * img.width + x) * 4) as usize;
    img.pixels[p..p + 4].try_into().unwrap()
}

#[test]
fn detects_both_kinds_from_the_first_member() {
    let o = zip(&[("mimetype", b"image/openraster", false)]);
    let k = zip(&[("mimetype", b"application/x-krita", false)]);
    assert_eq!(detect(&o[..64]), Some(Kind::Ora));
    assert_eq!(detect(&k[..64]), Some(Kind::Kra));
    // Deflated or misnamed first members and other ZIPs are not documents.
    assert_eq!(
        detect(&zip(&[("mimetype", b"image/openraster", true)])),
        None
    );
    assert_eq!(
        detect(&zip(&[("mimetypf", b"image/openraster", false)])),
        None
    );
    assert_eq!(
        detect(&zip(&[("mimetype", b"application/epub+zip", false)])),
        None
    );
    assert_eq!(detect(b"PK\x03\x04"), None);
}

#[test]
fn directory_fallback_finds_a_late_or_deflated_mimetype() {
    let merged = png(3, 2, |_, _| [1, 2, 3, 255]);
    // Krita templates: maindoc.xml first, mimetype deflated later.
    let k = zip(&[
        (
            "maindoc.xml",
            br#"<DOC><IMAGE width="3" height="2"/></DOC>"#,
            true,
        ),
        ("mimetype", b"application/x-krita", true),
        ("mergedimage.png", &merged, false),
    ]);
    // OpenRaster tools that write a directory entry first.
    let o = zip(&[
        ("data/", b"", false),
        ("mimetype", b"image/openraster\n", false),
    ]);
    assert_eq!(detect(&k), None);
    assert_eq!(detect_file(&k), Some(Kind::Kra));
    assert_eq!(detect_file(&o), Some(Kind::Ora));
    let doc = Document::open(&k).unwrap();
    assert_eq!((doc.kind, doc.canvas), (Kind::Kra, Some((3, 2))));
    // Other ZIPs stay other ZIPs: no mimetype, or another one (EPUB, ODF).
    assert_eq!(detect_file(&zip(&[("a.png", &merged, false)])), None);
    assert_eq!(
        detect_file(&zip(&[
            ("x", b"", false),
            ("mimetype", b"application/epub+zip", true)
        ])),
        None
    );
    assert_eq!(
        Document::open(&zip(&[("a.png", &merged, false)])).unwrap_err(),
        Error::NotDocument
    );
    // Old Krita templates: an empty mimetype member, the Krita DTD in maindoc.xml.
    let doc = br#"<?xml version="1.0"?><!DOCTYPE DOC PUBLIC '-//KDE//DTD krita 2.0//EN' 'http://www.calligra.org/DTD/krita-2.0.dtd'><DOC/>"#;
    let old = zip(&[("mimetype", b"", false), ("maindoc.xml", doc, true)]);
    assert_eq!(detect_file(&old), Some(Kind::Kra));
    let plain = zip(&[("mimetype", b"", false), ("maindoc.xml", b"<DOC/>", true)]);
    assert_eq!(detect_file(&plain), None);
}

#[test]
fn sizes_come_from_the_xml_and_png_header_only() {
    // A merged image whose pixel data is garbage: its size still reads from the header.
    let mut merged = png(300, 200, |_, _| [1, 2, 3, 255]);
    let idat = merged.windows(4).position(|w| w == b"IDAT").unwrap();
    for b in &mut merged[idat + 6..idat + 40] {
        *b = 0xaa;
    }
    let f = zip(&[
        ("mimetype", b"application/x-krita", false),
        ("maindoc.xml", br#"<?xml version="1.0"?><DOC><IMAGE width="300" height="200" mime="application/x-kra"/></DOC>"#, true),
        ("mergedimage.png", &merged, true),
    ]);
    let doc = Document::open(&f).unwrap();
    assert_eq!(doc.canvas, Some((300, 200)));
    let e = doc.merged().unwrap();
    let head = doc.read_prefix(&f, e, 4096).unwrap();
    let info = specular_png::read_info(&head).unwrap();
    assert_eq!((info.width, info.height), (300, 200));
    let s = r#"<image w="640" h="480"><stack><layer src="data/a.png"/></stack></image>"#;
    let doc = Document::open(&ora(s, &[])).unwrap();
    assert_eq!(doc.canvas, Some((640, 480)));
}

#[test]
fn merged_member_reads_back_exactly() {
    let merged = png(17, 9, |x, y| [x as u8 * 9, y as u8 * 20, 5, 200]);
    for deflate in [false, true] {
        let f = zip(&[
            ("mimetype", b"image/openraster", false),
            (
                "stack.xml",
                br#"<image w="17" h="9"><stack/></image>"#,
                deflate,
            ),
            ("mergedimage.png", &merged, deflate),
            ("Thumbnails/thumbnail.png", &merged, deflate),
        ]);
        let doc = Document::open(&f).unwrap();
        assert_eq!(&*doc.read(&f, doc.merged().unwrap()).unwrap(), &merged[..]);
        assert_eq!(&*doc.read(&f, doc.preview().unwrap()).unwrap(), &merged[..]);
        assert_eq!(
            doc.stored_range(&f, doc.merged().unwrap())
                .unwrap()
                .is_some(),
            !deflate
        );
    }
}

#[test]
fn composites_offsets_opacity_and_visibility() {
    let red = png(4, 4, |_, _| [255, 0, 0, 255]);
    let blue = png(4, 4, |_, _| [0, 0, 255, 255]);
    let green = png(8, 8, |_, _| [0, 255, 0, 255]);
    let s = r#"<image w="8" h="8"><stack>
        <layer src="data/hidden.png" visibility="hidden"/>
        <layer src="data/blue.png" x="2" y="2" opacity="0.5"/>
        <layer src="data/red.png" x="-2" y="-2"/>
        <stack opacity="0.5" x="4" y="4"><layer src="data/red.png"/></stack>
    </stack></image>"#;
    let f = ora(
        s,
        &[
            ("data/red.png", red),
            ("data/blue.png", blue),
            ("data/hidden.png", green),
        ],
    );
    let img = composite(&f).unwrap();
    assert_eq!((img.width, img.height), (8, 8));
    // Red at (-2,-2) covers 0..2; blue at half opacity over it at (2..4) is over nothing
    // except where red is.
    assert_eq!(at(&img, 0, 0), [255, 0, 0, 255]);
    assert_eq!(at(&img, 1, 1), [255, 0, 0, 255]);
    // Half-transparent blue alone.
    assert_eq!(at(&img, 3, 3), [0, 0, 255, 128]);
    // The half-opacity group of red at (4,4), under blue at (4..6)? No: blue covers 2..6.
    // Blue (0.5) over red (0.5): a = 0.5 + 0.5·0.5 = 0.75, colour = (0.5·blue + 0.25·red) / 0.75.
    assert_eq!(at(&img, 5, 5), [85, 0, 170, 191]);
    // Group alone.
    assert_eq!(at(&img, 7, 7), [255, 0, 0, 128]);
    // Uncovered.
    assert_eq!(at(&img, 7, 0), [0, 0, 0, 0]);
}

/// The OpenRaster specification's blend modes against the W3C formulas: a 4×1 layer of
/// grey levels over a 4×1 layer of others, opaque, then half-transparent over opaque.
#[test]
fn openraster_blend_modes_follow_the_w3c_formulas() {
    use specular_blend::{Mode, PorterDuff};
    let top = [40u8, 100, 160, 230];
    let bottom = [200u8, 30, 128, 90];
    let modes = [
        "svg:multiply",
        "svg:screen",
        "svg:overlay",
        "svg:darken",
        "svg:lighten",
        "svg:color-dodge",
        "svg:color-burn",
        "svg:hard-light",
        "svg:soft-light",
        "svg:difference",
        "svg:exclusion",
        "svg:color",
        "svg:luminosity",
        "svg:hue",
        "svg:saturation",
    ];
    for op in modes {
        for alpha in [255u8, 128] {
            let s = format!(
                r#"<image w="4" h="1"><stack><layer src="t.png" composite-op="{op}"/><layer src="b.png"/></stack></image>"#
            );
            let t = png(4, 1, |x, _| {
                let v = top[x as usize];
                [v, 255 - v, v / 2, alpha]
            });
            let b = png(4, 1, |x, _| {
                let v = bottom[x as usize];
                [v, v / 3, 255 - v, 255]
            });
            let img = composite(&ora(&s, &[("t.png", t), ("b.png", b)])).unwrap();
            let m = Mode::from_openraster(op).unwrap();
            for x in 0..4 {
                let (tv, bv) = (top[x as usize], bottom[x as usize]);
                let cs = [tv, 255 - tv, tv / 2].map(|v| f32::from(v) / 255.0);
                let cb = [bv, bv / 3, 255 - bv].map(|v| f32::from(v) / 255.0);
                let (c, a) = specular_blend::composite(
                    m,
                    PorterDuff::SourceOver,
                    cb,
                    1.0,
                    cs,
                    f32::from(alpha) / 255.0,
                );
                let got = at(&img, x, 0);
                assert_eq!(got[3], (a * 255.0).round() as u8, "{op}");
                for k in 0..3 {
                    let want = c[k] * 255.0;
                    assert!(
                        (f32::from(got[k]) - want).abs() <= 1.0,
                        "{op} alpha {alpha} x {x}: {got:?} want {c:?}"
                    );
                }
            }
        }
    }
}

/// Porter-Duff operators, unknown operators, `krita:` ones and stack isolation.
#[test]
fn compositing_operators_and_isolation() {
    let red = png(2, 1, |_, _| [255, 0, 0, 255]);
    // A half-transparent blue layer covering only the left pixel.
    let blue = png(1, 1, |_, _| [0, 0, 255, 128]);
    let run = |op: &str| {
        let s = format!(
            r#"<image w="2" h="1"><stack><layer src="b.png" composite-op="{op}"/><layer src="r.png"/></stack></image>"#
        );
        let f = ora(&s, &[("b.png", blue.clone()), ("r.png", red.clone())]);
        let img = composite(&f).unwrap();
        (at(&img, 0, 0), at(&img, 1, 0))
    };
    // dst-out cuts the backdrop where the source is.
    assert_eq!(run("svg:dst-out"), ([255, 0, 0, 127], [255, 0, 0, 255]));
    // dst-in keeps the backdrop only where the source is: the right pixel goes.
    assert_eq!(run("svg:dst-in"), ([255, 0, 0, 128], [0, 0, 0, 0]));
    // src-atop keeps the backdrop's alpha.
    let (l, r) = run("svg:src-atop");
    assert_eq!((l[3], r), (255, [255, 0, 0, 255]));
    assert!(l[0].abs_diff(127) <= 1 && l[2].abs_diff(128) <= 1, "{l:?}");
    // dst-atop: the backdrop where the source is, the source elsewhere (nothing here).
    assert_eq!(run("svg:dst-atop").1, [0, 0, 0, 0]);
    // plus: the sum.
    let (l, _) = run("svg:plus");
    assert_eq!(l[3], 255);
    // Unknown operators are drawn as src-over; Krita's erase is dst-out.
    assert_eq!(run("svg:no-such-op"), run("svg:src-over"));
    assert_eq!(run("krita:erase"), run("svg:dst-out"));
    assert_eq!(run("krita:linear_burn").1, [255, 0, 0, 255]);
    // A stack at full opacity with src-over passes through: its multiply child blends with
    // what is under the stack. Isolated, the child has nothing under it inside the stack.
    let grey = png(1, 1, |_, _| [128, 128, 128, 255]);
    let stack = |iso: &str| {
        format!(
            r#"<image w="1" h="1"><stack><stack {iso}><layer src="g.png" composite-op="svg:multiply"/></stack><layer src="r.png"/></stack></image>"#
        )
    };
    let f = |s: &str| {
        let img = composite(&ora(s, &[("g.png", grey.clone()), ("r.png", red.clone())])).unwrap();
        at(&img, 0, 0)
    };
    assert_eq!(f(&stack("")), [128, 0, 0, 255]);
    assert_eq!(f(&stack(r#"isolation="isolate""#)), [128, 128, 128, 255]);
    // A multiply stack is isolated: its children composite on their own first.
    assert_eq!(
        f(&stack(r#"composite-op="svg:multiply""#)),
        [128, 0, 0, 255]
    );
}

#[test]
fn refuses_layer_kinds_it_cannot_draw() {
    let s = r#"<image w="2" h="2"><stack><layer src="a.jpg"/></stack></image>"#;
    let e = composite(&ora(s, &[("a.jpg", b"\xff\xd8\xff\xe0".to_vec())])).unwrap_err();
    assert!(e.to_string().contains("only PNG layers"), "{e}");
    let s = r#"<image w="2" h="2"><stack><text>hi</text></stack></image>"#;
    assert!(
        composite(&ora(s, &[]))
            .unwrap_err()
            .to_string()
            .contains("text")
    );
    let s = r#"<image w="2" h="2"><stack><layer src="missing.png"/></stack></image>"#;
    assert!(composite(&ora(s, &[])).is_err());
    let s = r#"<image w="2" h="2"><stack/></image>"#;
    assert_eq!(
        composite(&ora(s, &[])).unwrap_err(),
        Error::NoImage(Kind::Ora)
    );
}

#[test]
fn limits_layers_nesting_and_canvas() {
    let many = format!(
        r#"<image w="2" h="2"><stack>{}</stack></image>"#,
        r#"<layer src="a.png"/>"#.repeat(specular_ora::MAX_LAYERS + 1)
    );
    assert!(matches!(
        composite(&ora(&many, &[])),
        Err(Error::Unsupported(_))
    ));
    let deep = format!(
        r#"<image w="2" h="2">{}{}</image>"#,
        "<stack>".repeat(specular_ora::MAX_DEPTH + 1),
        "</stack>".repeat(specular_ora::MAX_DEPTH + 1)
    );
    assert!(matches!(
        composite(&ora(&deep, &[])),
        Err(Error::Unsupported(_))
    ));
    let huge = r#"<image w="100000" h="100000"><stack><layer src="a.png"/></stack></image>"#;
    assert!(matches!(
        composite(&ora(huge, &[])),
        Err(Error::TooLarge(_))
    ));
}

#[test]
fn hostile_zips_are_refused_without_panics() {
    let good = zip(&[
        ("mimetype", b"image/openraster", false),
        ("mergedimage.png", &png(2, 2, |_, _| [9; 4]), true),
    ]);
    // Every truncation either opens or errors.
    for n in 0..good.len() {
        if let Ok(doc) = Document::open(&good[..n])
            && let Some(e) = doc.merged()
        {
            let _ = doc.read(&good[..n], e);
            let _ = doc.read_prefix(&good[..n], e, 10);
        }
    }
    // A declared size beyond what DEFLATE can produce from the stored bytes.
    let mut f = good.clone();
    let cd = f.windows(4).rposition(|w| w == b"PK\x01\x02").unwrap();
    f[cd + 24..cd + 28].copy_from_slice(&0x7fff_ffffu32.to_le_bytes());
    let doc = Document::open(&f).unwrap();
    assert!(doc.read(&f, doc.merged().unwrap()).is_err());
    // A declared size smaller than the stream: refused, not overrun.
    let mut f = good.clone();
    f[cd + 24..cd + 28].copy_from_slice(&3u32.to_le_bytes());
    let doc = Document::open(&f).unwrap();
    assert!(doc.read(&f, doc.merged().unwrap()).is_err());
    // A stored member whose sizes disagree (fuzz find): refused, not returned short.
    let stored = zip(&[
        ("mimetype", b"image/openraster", false),
        ("mergedimage.png", &png(2, 2, |_, _| [9; 4]), false),
    ]);
    let mut f = stored.clone();
    let cd = f.windows(4).rposition(|w| w == b"PK\x01\x02").unwrap();
    let size = u32::from_le_bytes(f[cd + 24..cd + 28].try_into().unwrap());
    f[cd + 20..cd + 24].copy_from_slice(&(size - 4).to_le_bytes());
    let doc = Document::open(&f).unwrap();
    assert!(doc.read(&f, doc.merged().unwrap()).is_err());
    assert!(doc.stored_range(&f, doc.merged().unwrap()).is_err());
    // A directory claiming more entries than it holds.
    let mut f = good.clone();
    let eocd = f.len() - 22;
    f[eocd + 10..eocd + 12].copy_from_slice(&500u16.to_le_bytes());
    assert!(Document::open(&f).is_err());
}

/// LZF for fixtures: literal runs, and runs of one repeated byte as overlapping
/// back-references at distance 1.
fn lzf(src: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(src.len() + src.len() / 32 + 1);
    let flush = |out: &mut Vec<u8>, lit: &[u8]| {
        for c in lit.chunks(32) {
            out.push(c.len() as u8 - 1);
            out.extend_from_slice(c);
        }
    };
    let (mut i, mut lit) = (0, 0);
    while i < src.len() {
        let mut run = 0;
        while i > 0 && i + run < src.len() && src[i + run] == src[i - 1] && run < 264 {
            run += 1;
        }
        if run >= 3 {
            flush(&mut out, &src[lit..i]);
            let len = run - 2;
            if len < 7 {
                out.extend_from_slice(&[(len as u8) << 5, 0]);
            } else {
                out.extend_from_slice(&[7 << 5, (len - 7) as u8, 0]);
            }
            i += run;
            lit = i;
        } else {
            i += 1;
        }
    }
    flush(&mut out, &src[lit..]);
    out
}

/// A version-2 Krita layer file: 64×64 tiles `(x, y, interleaved pixels)`, LZF-compressed
/// (byte-planar) or raw.
fn kra_layer(pixel: usize, tiles: &[(i64, i64, Vec<u8>)], packed: bool) -> Vec<u8> {
    let mut out = format!(
        "VERSION 2\nTILEWIDTH 64\nTILEHEIGHT 64\nPIXELSIZE {pixel}\nDATA {}\n",
        tiles.len()
    )
    .into_bytes();
    for (x, y, px) in tiles {
        let body = if packed {
            let planar: Vec<u8> = (0..pixel)
                .flat_map(|c| px.chunks(pixel).map(move |p| p[c]))
                .collect();
            let mut b = vec![1];
            b.extend(lzf(&planar));
            b
        } else {
            let mut b = vec![0];
            b.extend_from_slice(px);
            b
        };
        out.extend_from_slice(format!("{x},{y},LZF,{}\n", body.len()).as_bytes());
        out.extend_from_slice(&body);
    }
    out
}

/// A Krita document of one paint layer.
fn kra(w: u32, h: u32, space: &str, profile: &str, opacity: u8, layer: &[u8]) -> Vec<u8> {
    let doc = format!(
        r#"<DOC><IMAGE name="i" width="{w}" height="{h}" colorspacename="{space}" profile="{profile}"><layers><layer name="L" nodetype="paintlayer" filename="l" colorspacename="{space}" opacity="{opacity}" visible="1" x="0" y="0"/></layers></IMAGE></DOC>"#
    );
    zip(&[
        ("mimetype", b"application/x-krita", false),
        ("maindoc.xml", doc.as_bytes(), true),
        ("i/layers/l", layer, false),
    ])
}

#[test]
fn krita_layers_decode_every_pixel_kind() {
    use specular_ora::{KraOutput, KraPixel};
    // Exact halves only.
    let h = |v: f32| -> [u8; 2] {
        let bits: u16 = if v == 0.25 {
            0x3400
        } else if v == 0.5 {
            0x3800
        } else {
            assert_eq!(v, 1.0);
            0x3c00
        };
        bits.to_le_bytes()
    };
    let f = |v: f32| v.to_le_bytes();
    let u = |v: u16| v.to_le_bytes();
    let srgb_half = ((1.055 * 0.5f64.powf(1.0 / 2.4) - 0.055) * 65535.0 + 0.5) as u16;
    let linear = "sRGB-elle-V2-g10.icc";
    let gamma = "sRGB-elle-V2-srgbtrc.icc";
    type Case = (
        &'static str,
        &'static str,
        u8,
        Vec<u8>,
        KraPixel,
        KraOutput,
        [u16; 4],
    );
    #[rustfmt::skip]
    let cases: [Case; 8] = [
        ("RGBA", gamma, 255, vec![30, 20, 10, 255], KraPixel::Rgba8, KraOutput::Rgba8, [0; 4]),
        ("RGBA16", gamma, 255, [u(3), u(2), u(1), u(65535)].concat(), KraPixel::Rgba16,
            KraOutput::Rgba16, [1, 2, 3, 65535]),
        ("GRAYA", gamma, 255, vec![100, 255], KraPixel::GrayA8, KraOutput::GrayAlpha16,
            [100 * 257, 65535, 0, 0]),
        ("GRAYAU16", gamma, 128, [u(1000), u(65535)].concat(), KraPixel::GrayA16,
            KraOutput::GrayAlpha16, [1000, 32896, 0, 0]),
        ("RGBAF16", linear, 255, [h(0.5), h(1.0), h(0.25), h(1.0)].concat(), KraPixel::RgbaF16,
            KraOutput::Rgba16F, [0x3800, 0x3c00, 0x3400, 0x3c00]),
        ("RGBAF32", gamma, 255, [f(0.5), f(1.0), f(0.0), f(1.0)].concat(), KraPixel::RgbaF32,
            KraOutput::Rgba16, [32768, 65535, 0, 65535]),
        ("GRAYAF32", linear, 255, [f(0.5), f(1.0)].concat(), KraPixel::GrayAF32,
            KraOutput::GrayAlpha16, [srgb_half, 65535, 0, 0]),
        ("GRAYAF16", gamma, 255, [h(0.5), h(1.0)].concat(), KraPixel::GrayAF16,
            KraOutput::GrayAlpha16, [32768, 65535, 0, 0]),
    ];
    for (space, profile, opacity, px, pixel, output, want) in cases {
        for packed in [false, true] {
            let tile: Vec<u8> = px
                .iter()
                .copied()
                .cycle()
                .take(px.len() * 64 * 64)
                .collect();
            let layer = kra_layer(px.len(), &[(0, 0, tile)], packed);
            let f = kra(3, 2, space, profile, opacity, &layer);
            let doc = Document::open(&f).unwrap();
            assert_eq!(doc.kra_layers.len(), 1, "{space}");
            let l = &doc.kra_layers[0];
            assert_eq!((l.pixel, l.output()), (pixel, output), "{space}");
            let img = doc.kra_layer_page(&f, l, &|| false).unwrap();
            assert_eq!((img.width, img.height, img.format), (3, 2, output));
            assert!(!img.truncated);
            assert_eq!(img.data.len(), 6 * output.bytes());
            let last = &img.data[5 * output.bytes()..];
            if output == KraOutput::Rgba8 {
                assert_eq!(last, [10, 20, 30, 255]);
                continue;
            }
            let got: Vec<u16> = last
                .chunks(2)
                .map(|b| u16::from_ne_bytes([b[0], b[1]]))
                .collect();
            assert_eq!(got, want[..got.len()], "{space} packed {packed}");
        }
    }
}

#[test]
fn krita_layers_skip_non_pixel_layers_and_keep_order() {
    let doc = br#"<DOC><IMAGE name="i" width="4" height="4" colorspacename="RGBA"><layers>
        <layer name="A" nodetype="paintlayer" filename="a" visible="0" opacity="255"/>
        <layer name="G" nodetype="grouplayer" filename="g"><layers>
          <layer name="B" nodetype="paintlayer" filename="b" colorspacename="RGBA16" x="-3" y="7"/>
          <layer name="F" nodetype="adjustmentlayer" filename="f"/>
        </layers></layer>
        <layer name="V" nodetype="shapelayer" filename="v"/>
        <layer name="C" nodetype="clonelayer" filename="c"/>
        <layer name="Lab" nodetype="paintlayer" filename="lab" colorspacename="LABA"/>
        <layer name="D" nodetype="paintlayer" filename="d"><masks><mask name="M" filename="m"/></masks></layer>
        <layer name="Gone" nodetype="paintlayer" filename="gone"/>
      </layers></IMAGE></DOC>"#;
    let empty = kra_layer(4, &[], true);
    let f = zip(&[
        ("mimetype", b"application/x-krita", false),
        ("maindoc.xml", doc, true),
        ("i/layers/a", &empty, true),
        ("i/layers/b", &empty, true),
        ("i/layers/f", &empty, true),
        ("i/layers/g", &empty, true),
        ("i/layers/v", b"<svg/>", true),
        ("i/layers/c", &empty, true),
        ("i/layers/lab", &empty, true),
        ("i/layers/d", &empty, true),
        ("i/layers/m", &empty, true),
    ]);
    let d = Document::open(&f).unwrap();
    let names: Vec<_> = d.kra_layers.iter().map(|l| l.name.as_str()).collect();
    // The group holding a paint layer is a page too, before its children.
    assert_eq!(names, ["A", "G", "B", "D"]);
    assert!(!d.kra_layers[0].visible);
    assert!(d.kra_layers[1].group && d.kra_layers[1].member.is_empty());
    assert_eq!((d.kra_layers[2].x, d.kra_layers[2].y), (-3, 7));
    // The untyped mask is noted, not applied; the filter, shape, clone and Lab layers too.
    assert!(d.kra_layers[3].masks.is_empty());
    assert_eq!(d.kra_layers[3].skipped_masks, [("mask".into(), "M".into())]);
    let notes = d.kra_notes(&f, None, None).join("\n");
    for n in ["F", "V", "C", "Lab", "M"] {
        assert!(notes.contains(&format!("“{n}”")), "{n}: {notes}");
    }
    // An empty layer is its default pixel: transparent.
    let img = d.kra_layer_page(&f, &d.kra_layers[3], &|| false).unwrap();
    assert!(img.data.iter().all(|&b| b == 0) && !img.truncated);
    // Cancelled before any tile row.
    let tiles: Vec<_> = (0..4)
        .map(|ty| (0, ty * 64, vec![9u8; 64 * 64 * 4]))
        .collect();
    let f = kra(64, 256, "RGBA", "", 255, &kra_layer(4, &tiles, true));
    let d = Document::open(&f).unwrap();
    assert!(matches!(
        d.kra_layer_page(&f, &d.kra_layers[0], &|| true),
        Err(Error::Cancelled)
    ));
}

/// Wall-clock of large layer pages, all tiles LZF-compressed. Run with
/// `cargo test --release -p specular-ora --test ora -- --ignored --nocapture kra_speed`.
#[test]
#[ignore = "timing"]
fn kra_speed() {
    use std::time::Instant;
    let side = 8192u32;
    for (space, pixel) in [("RGBA", 4usize), ("RGBA16", 8), ("RGBAF16", 8)] {
        let t = Instant::now();
        let mut tiles = Vec::new();
        for ty in 0..i64::from(side) / 64 {
            for tx in 0..i64::from(side) / 64 {
                let px: Vec<u8> = (0..64 * 64i64)
                    .flat_map(|i| {
                        let (x, y) = (tx * 64 + i % 64, ty * 64 + i / 64);
                        let v = [(x / 32) as u8, (y / 32) as u8, ((x ^ y) / 64) as u8, 255];
                        (0..pixel).map(move |c| if pixel == 4 { v[c] } else { v[c / 2] })
                    })
                    .collect();
                tiles.push((tx * 64, ty * 64, px));
            }
        }
        let layer = kra_layer(pixel, &tiles, true);
        drop(tiles);
        let f = kra(side, side, space, "sRGB-elle-V2-g10.icc", 200, &layer);
        eprintln!(
            "{space}: fixture {:.0} MB in {:.1} s",
            f.len() as f64 / 1e6,
            t.elapsed().as_secs_f64()
        );
        let t = Instant::now();
        let d = Document::open(&f).unwrap();
        let open = t.elapsed();
        let mut best = f64::MAX;
        for _ in 0..3 {
            let t = Instant::now();
            let img = d.kra_layer_page(&f, &d.kra_layers[0], &|| false).unwrap();
            best = best.min(t.elapsed().as_secs_f64());
            assert!(!img.truncated);
        }
        eprintln!(
            "{space} {side}×{side}: open {:.2} ms, layer page {:.0} ms ({:.0} MP/s)",
            open.as_secs_f64() * 1e3,
            best * 1e3,
            f64::from(side) * f64::from(side) / best / 1e6
        );
    }
}

/// A Krita document of `layers` (the `<layers>` body of `maindoc.xml`, top-most first) and
/// members `(file under i/layers/, data)`.
fn kra_tree(
    w: u32,
    h: u32,
    space: &str,
    profile: &str,
    layers: &str,
    members: &[(&str, Vec<u8>)],
) -> Vec<u8> {
    let doc = format!(
        r#"<DOC><IMAGE name="i" width="{w}" height="{h}" colorspacename="{space}" profile="{profile}"><layers>{layers}</layers></IMAGE></DOC>"#
    );
    let names: Vec<String> = members
        .iter()
        .map(|(n, _)| format!("i/layers/{n}"))
        .collect();
    let mut all: Vec<(&str, &[u8], bool)> = vec![
        ("mimetype", b"application/x-krita", false),
        ("maindoc.xml", doc.as_bytes(), true),
    ];
    for (n, (_, d)) in names.iter().zip(members) {
        all.push((n, d, true));
    }
    zip(&all)
}

/// One 64×64 tile at the layer origin, every pixel `px`.
fn solid(px: &[u8]) -> Vec<u8> {
    kra_layer(px.len(), &[(0, 0, px.repeat(64 * 64))], true)
}

/// A paint layer element: visible and opaque unless `extra` says otherwise.
fn paint(name: &str, extra: &str) -> String {
    let mut a = extra.to_owned();
    for (k, v) in [("visible", "1"), ("opacity", "255")] {
        if !extra.contains(&format!("{k}=")) {
            a += &format!(r#" {k}="{v}""#);
        }
    }
    format!(r#"<layer name="{name}" nodetype="paintlayer" filename="{name}" {a}/>"#)
}

fn rgba8(img: &specular_ora::KraImage, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * img.width + x) * 4) as usize;
    img.data[i..i + 4].try_into().unwrap()
}

#[test]
fn krita_blend_modes_match_their_formulas() {
    // Krita's separable ops on normalised straight values; `s` over `d`.
    let hard = |s: f64, d: f64| {
        if s > 0.5 {
            1.0 - 2.0 * (1.0 - s) * (1.0 - d)
        } else {
            2.0 * s * d
        }
    };
    let expect = |op: &str, s: f64, d: f64| -> f64 {
        match op {
            "normal" => s,
            "multiply" => s * d,
            "screen" => 1.0 - (1.0 - s) * (1.0 - d),
            "overlay" => hard(d, s),
            "hard_light" => hard(s, d),
            "darken" => s.min(d),
            "lighten" => s.max(d),
            "add" => (s + d).min(1.0),
            "subtract" => (d - s).max(0.0),
            "diff" => (s - d).abs(),
            "dodge" => (d / (1.0 - s)).min(1.0),
            "burn" => 1.0 - ((1.0 - d) / s).min(1.0),
            "soft_light" if s > 0.5 => d + (2.0 * s - 1.0) * (d.sqrt() - d),
            "soft_light_svg" if s > 0.5 && d > 0.25 => d + (2.0 * s - 1.0) * (d.sqrt() - d),
            "soft_light" | "soft_light_svg" => d - (1.0 - 2.0 * s) * d * (1.0 - d),
            _ => unreachable!(),
        }
    };
    let ops = [
        "normal",
        "multiply",
        "screen",
        "overlay",
        "darken",
        "lighten",
        "add",
        "subtract",
        "diff",
        "dodge",
        "burn",
        "soft_light",
        "soft_light_svg",
        "hard_light",
    ];
    for op in ops {
        for (s, d) in [(153u8, 77u8), (51, 179), (230, 140)] {
            let layers = format!(
                "{}{}",
                paint("s", &format!(r#"compositeop="{op}""#)),
                paint("d", "")
            );
            // Grey pixels (BGRA), opaque.
            let f = kra_tree(
                4,
                4,
                "RGBA",
                "",
                &layers,
                &[("s", solid(&[s, s, s, 255])), ("d", solid(&[d, d, d, 255]))],
            );
            let doc = Document::open(&f).unwrap();
            assert!(doc.kra_can_composite() && doc.kra_notes(&f, None, None).is_empty());
            let img = doc.kra_composite(&f, &|| false).unwrap();
            let want = expect(op, f64::from(s) / 255.0, f64::from(d) / 255.0) * 255.0;
            let got = rgba8(&img, 3, 3);
            assert!(
                (f64::from(got[0]) - want).abs() <= 1.0 && got[3] == 255,
                "{op} {s} {d}: {got:?} want {want:.1}"
            );
            assert_eq!(got[0], got[1]);
            assert_eq!(got[1], got[2]);
        }
    }
    // Half opacity over half alpha: the union of the alphas, the op weighted by both.
    let layers = format!(
        "{}{}",
        paint("s", r#"compositeop="multiply" opacity="128""#),
        paint("d", "")
    );
    let f = kra_tree(
        2,
        2,
        "RGBA",
        "",
        &layers,
        &[
            ("s", solid(&[0, 0, 255, 255])),
            ("d", solid(&[255, 255, 255, 128])),
        ],
    );
    let doc = Document::open(&f).unwrap();
    let px = rgba8(&doc.kra_composite(&f, &|| false).unwrap(), 0, 0);
    let (sa, da) = (128.0 / 255.0, 128.0 / 255.0);
    let a = sa + da - sa * da;
    // Red: s = 1, d = 1, B = 1; green and blue: s = 0, d = 1, B = 0.
    let g = ((1.0 - sa) * da) / a * 255.0;
    assert!((f64::from(px[3]) - a * 255.0).abs() <= 1.0, "{px:?}");
    assert!(
        px[0] == 255 && (f64::from(px[1]) - g).abs() <= 1.0,
        "{px:?} {g}"
    );
    // An op that is not applied is drawn as normal, with a note.
    let layers = format!(
        "{}{}",
        paint("s", r#"compositeop="modulo_continuous""#),
        paint("d", "")
    );
    let f = kra_tree(
        2,
        2,
        "RGBA",
        "",
        &layers,
        &[
            ("s", solid(&[10, 20, 30, 255])),
            ("d", solid(&[9, 9, 9, 255])),
        ],
    );
    let doc = Document::open(&f).unwrap();
    let img = doc.kra_composite(&f, &|| false).unwrap();
    assert_eq!(rgba8(&img, 1, 1), [30, 20, 10, 255]);
    assert_eq!(
        doc.kra_notes(&f, None, None),
        ["blend mode “modulo_continuous” of “s” drawn as normal"]
    );
}

#[test]
fn krita_transparency_masks_scale_alpha() {
    // Mask values 0, 64, 128, 255, … per column, placed one column right: column 0 is outside
    // the mask's tile, so its default pixel (fully selected).
    let mask: Vec<u8> = (0..64 * 64).map(|i| [0u8, 64, 128, 255][i % 4]).collect();
    let mask = kra_layer(1, &[(0, 0, mask)], true);
    let layers = format!(
        r#"<layer name="r" nodetype="paintlayer" filename="r" visible="1" opacity="255"><masks>
          <mask name="T" nodetype="transparencymask" filename="m" x="1" y="0" visible="1"/>
          <mask name="Off" nodetype="transparencymask" filename="z" visible="0"/>
          <mask name="Sel" nodetype="selectionmask" filename="s" visible="1"/>
          <mask name="Blur" nodetype="filtermask" filename="b" visible="1"/>
        </masks></layer>{}"#,
        paint("bg", "")
    );
    let members = [
        ("r", solid(&[0, 0, 255, 255])),
        ("bg", solid(&[0, 255, 0, 255])),
        ("m.pixelselection", mask),
        ("m.pixelselection.defaultpixel", vec![255]),
        ("z.pixelselection", solid(&[0])),
    ];
    let f = kra_tree(5, 3, "RGBA", "", &layers, &members);
    let doc = Document::open(&f).unwrap();
    let r = &doc.kra_layers[0];
    assert_eq!(r.masks.len(), 1);
    assert_eq!(r.masks[0].member, "i/layers/m.pixelselection");
    // The selection mask does not change pixels, so only the filter mask is noted.
    assert_eq!(
        doc.kra_notes(&f, None, None),
        ["filter mask “Blur” skipped"]
    );
    // The layer page: red, alpha from the mask.
    let page = doc.kra_layer_page(&f, r, &|| false).unwrap();
    let alphas: Vec<u8> = (0..5).map(|x| rgba8(&page, x, 2)[3]).collect();
    assert_eq!(alphas, [255, 0, 64, 128, 255]);
    assert_eq!(rgba8(&page, 3, 0), [255, 0, 0, 128]);
    // The composite: red over green by the mask.
    let img = doc.kra_composite(&f, &|| false).unwrap();
    assert_eq!(rgba8(&img, 0, 0), [255, 0, 0, 255]);
    assert_eq!(rgba8(&img, 1, 0), [0, 255, 0, 255]);
    assert_eq!(rgba8(&img, 3, 1), [128, 127, 0, 255]);
    assert!(!img.truncated);
}

#[test]
fn krita_groups_nest_fade_and_pass_through() {
    // White below a half-opaque group holding a nested group with red in it; a hidden green
    // layer and a filter layer inside are left out.
    let layers = format!(
        r#"<layer name="G" nodetype="grouplayer" visible="1" opacity="128"><layers>
          {}
          <layer name="Inv" nodetype="adjustmentlayer" filename="inv" visible="1"/>
          <layer name="G2" nodetype="grouplayer" visible="1" opacity="255"><layers>{}</layers></layer>
        </layers></layer>{}"#,
        paint("g", r#"visible="0""#),
        paint("r", ""),
        paint("w", "")
    );
    let members = [
        ("g", solid(&[0, 255, 0, 255])),
        ("r", solid(&[0, 0, 255, 255])),
        ("w", solid(&[255, 255, 255, 255])),
        ("inv", Vec::new()),
    ];
    let f = kra_tree(3, 70, "RGBA", "", &layers, &members);
    let doc = Document::open(&f).unwrap();
    let names: Vec<_> = doc.kra_layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["G", "g", "G2", "r", "w"]);
    assert_eq!(
        doc.kra_notes(&f, None, None),
        ["filter layer “Inv” skipped"]
    );
    let img = doc.kra_composite(&f, &|| false).unwrap();
    assert_eq!(rgba8(&img, 2, 10), [255, 127, 127, 255]);
    // Row 64 is past the layers' only tile row: every default pixel is transparent.
    assert_eq!(rgba8(&img, 2, 64), [0, 0, 0, 0]);
    // The group page: its children with its own opacity, nothing below.
    let page = doc
        .kra_layer_page(&f, &doc.kra_layers[0], &|| false)
        .unwrap();
    assert_eq!(rgba8(&page, 0, 0), [255, 0, 0, 128]);
    assert_eq!(
        doc.kra_notes(&f, Some(doc.kra_layers[0].node), None),
        ["filter layer “Inv” skipped"]
    );

    // Multiply in a pass-through group reaches the grey below; in an isolated group it
    // multiplies only the group's own (empty) pixels.
    for (pass, want) in [("1", 64u8), ("0", 128)] {
        let layers = format!(
            r#"<layer name="P" nodetype="grouplayer" passthrough="{pass}" visible="1" opacity="255"><layers>{}</layers></layer>{}"#,
            paint("s", r#"compositeop="multiply""#),
            paint("d", "")
        );
        let f = kra_tree(
            2,
            2,
            "RGBA",
            "",
            &layers,
            &[
                ("s", solid(&[128, 128, 128, 255])),
                ("d", solid(&[128, 128, 128, 255])),
            ],
        );
        let doc = Document::open(&f).unwrap();
        let px = rgba8(&doc.kra_composite(&f, &|| false).unwrap(), 1, 1);
        assert!(px[0].abs_diff(want) <= 1 && px[3] == 255, "{pass}: {px:?}");
    }
}

#[test]
fn krita_deep_composites_keep_their_range() {
    // 16-bit: screen of 0.5 over 0.25 is 0.625.
    let px16 = |v: u16| {
        [v, v, v, 65535]
            .iter()
            .flat_map(|c| c.to_le_bytes())
            .collect::<Vec<u8>>()
    };
    let layers = format!(
        "{}{}",
        paint("s", r#"compositeop="screen""#),
        paint("d", "")
    );
    let f = kra_tree(
        2,
        2,
        "RGBA16",
        "",
        &layers,
        &[("s", solid(&px16(32768))), ("d", solid(&px16(16384)))],
    );
    let doc = Document::open(&f).unwrap();
    let img = doc.kra_composite(&f, &|| false).unwrap();
    assert_eq!(img.format, specular_ora::KraOutput::Rgba16);
    let r = u16::from_ne_bytes([img.data[0], img.data[1]]);
    assert!(r.abs_diff(40960) <= 2, "{r}");
    // Linear float: add goes past 1; a half-transparent black layer covers row 0 only.
    let pxf = |v: f32, a: f32| {
        [v, v, v, a]
            .iter()
            .flat_map(|c| c.to_le_bytes())
            .collect::<Vec<u8>>()
    };
    let layers = format!(
        "{}{}{}",
        paint("n", ""),
        paint("s", r#"compositeop="add""#),
        paint("d", "")
    );
    let mut row0 = pxf(0.0, 0.5).repeat(64);
    row0.extend(vec![0u8; 63 * 64 * 16]);
    let f = kra_tree(
        2,
        2,
        "RGBAF32",
        "Rec2020-elle-V4-g10.icc",
        &layers,
        &[
            ("n", kra_layer(16, &[(0, 0, row0)], true)),
            ("s", solid(&pxf(0.75, 1.0))),
            ("d", solid(&pxf(0.75, 1.0))),
        ],
    );
    let doc = Document::open(&f).unwrap();
    let img = doc.kra_composite(&f, &|| false).unwrap();
    assert_eq!(img.format, specular_ora::KraOutput::Rgba16F);
    let half = |i: usize| u16::from_ne_bytes([img.data[i], img.data[i + 1]]);
    // Row 0: 1.5 halved is 0.75 (0x3a00), opaque (0x3c00).
    assert_eq!((half(0), half(6)), (0x3a00, 0x3c00));
    // Row 1: 1.5 (0x3e00).
    let row1 = 2 * 8;
    assert_eq!((half(row1), half(row1 + 6)), (0x3e00, 0x3c00));
}

#[test]
fn krita_composite_cancel_and_refusals() {
    let tiles: Vec<_> = (0..4)
        .map(|ty| (0, ty * 64, vec![9u8; 64 * 64 * 4]))
        .collect();
    let f = kra(64, 256, "RGBA", "", 255, &kra_layer(4, &tiles, true));
    let d = Document::open(&f).unwrap();
    assert!(matches!(
        d.kra_composite(&f, &|| true),
        Err(Error::Cancelled)
    ));
    assert!(!d.kra_composite(&f, &|| false).unwrap().truncated);
    // Lab: no colour model to composite in.
    let layers = paint("l", r#"colorspacename="RGBA""#);
    let f = kra_tree(2, 2, "LABA", "", &layers, &[("l", solid(&[1, 2, 3, 255]))]);
    let d = Document::open(&f).unwrap();
    assert!(!d.kra_can_composite() && d.kra_composite(&f, &|| false).is_err());
    // Only hidden layers.
    let layers = paint("l", r#"visible="0""#);
    let f = kra_tree(2, 2, "RGBA", "", &layers, &[("l", solid(&[1, 2, 3, 255]))]);
    assert!(!Document::open(&f).unwrap().kra_can_composite());
}

/// Wall-clock of a large composite: three full layers (a masked screen, multiply in a
/// pass-through group, normal). Run with
/// `cargo test --release -p specular-ora --test ora -- --ignored --nocapture kra_composite_speed`.
#[test]
#[ignore = "timing"]
fn kra_composite_speed() {
    use std::time::Instant;
    let side = 4096i64;
    let layer = |seed: u8| {
        let tiles: Vec<_> = (0..side / 64)
            .flat_map(|ty| (0..side / 64).map(move |tx| (tx, ty)))
            .map(|(tx, ty)| {
                let px: Vec<u8> = (0..64 * 64i64)
                    .flat_map(|i| {
                        let (x, y) = (tx * 64 + i % 64, ty * 64 + i / 64);
                        [(x / 16) as u8 ^ seed, (y / 16) as u8, seed, 255]
                    })
                    .collect();
                (tx * 64, ty * 64, px)
            })
            .collect();
        kra_layer(4, &tiles, true)
    };
    let mask: Vec<_> = (0..side / 64)
        .flat_map(|ty| (0..side / 64).map(move |tx| (tx * 64, ty * 64, vec![(tx * 4) as u8; 4096])))
        .collect();
    let layers = format!(
        r#"<layer name="a" nodetype="paintlayer" filename="a" compositeop="screen" visible="1" opacity="200"><masks><mask nodetype="transparencymask" filename="m" visible="1"/></masks></layer>
        <layer name="P" nodetype="grouplayer" passthrough="1" visible="1" opacity="255"><layers>{}</layers></layer>{}"#,
        paint("b", r#"compositeop="multiply""#),
        paint("c", "")
    );
    let f = kra_tree(
        side as u32,
        side as u32,
        "RGBA",
        "",
        &layers,
        &[
            ("a", layer(1)),
            ("b", layer(90)),
            ("c", layer(200)),
            ("m.pixelselection", kra_layer(1, &mask, true)),
        ],
    );
    let d = Document::open(&f).unwrap();
    let mut best = f64::MAX;
    for _ in 0..3 {
        let t = Instant::now();
        let img = d.kra_composite(&f, &|| false).unwrap();
        best = best.min(t.elapsed().as_secs_f64());
        assert!(!img.truncated);
    }
    let mp = (side * side) as f64 / 1e6;
    eprintln!(
        "composite {side}×{side}, 3 layers + mask: {:.0} ms ({:.0} MP/s)",
        best * 1e3,
        mp / best
    );
}

#[test]
fn krita_invert_filter_layers_and_animation_notes() {
    // Invert over half-transparent black, selected from column 1 on: Krita draws the inverted
    // pixels over the originals, so alpha grows (0.5 over 0.5 is 0.75).
    let layers = format!(
        r#"<layer name="Inv" nodetype="adjustmentlayer" filtername="invert" filename="v" x="1" visible="1" opacity="255"/>{}{}"#,
        paint("d", ""),
        paint("a", r#"keyframes="a.keyframes.xml""#)
    );
    let f = kra_tree(
        3,
        1,
        "RGBA",
        "",
        &layers,
        &[
            ("v.pixelselection", solid(&[255])),
            ("d", solid(&[0, 0, 0, 128])),
            ("a", kra_layer(4, &[], true)),
        ],
    );
    let doc = Document::open(&f).unwrap();
    assert_eq!(
        doc.kra_notes(&f, None, None),
        ["animated layer “a” drawn at its first keyframe"]
    );
    let img = doc.kra_composite(&f, &|| false).unwrap();
    assert_eq!(rgba8(&img, 0, 0), [0, 0, 0, 128]);
    let px = rgba8(&img, 2, 0);
    assert!(
        px[0].abs_diff(170) <= 1 && px[3].abs_diff(192) <= 1,
        "{px:?}"
    );
}

/// A paint layer whose data is a plain PNG instead of Krita's tiles: placed at the layer's
/// offset on its page and in the composite, in 8- and 16-bit RGBA documents.
#[test]
fn krita_layer_data_stored_as_png() {
    let p = png(3, 2, |x, y| [10 * x as u8 + 50, 100, 20 * y as u8, 255]);
    for space in ["RGBA", "RGBA16"] {
        let layers = paint("s", r#"x="1" y="2""#);
        let f = kra_tree(6, 5, space, "", &layers, &[("s", p.clone())]);
        let doc = Document::open(&f).unwrap();
        let page = doc
            .kra_layer_page(&f, &doc.kra_layers[0], &|| false)
            .unwrap();
        let img = doc.kra_composite(&f, &|| false).unwrap();
        if space == "RGBA" {
            assert_eq!(rgba8(&page, 0, 0), [0, 0, 0, 0]);
            assert_eq!(rgba8(&page, 1, 2), [50, 100, 0, 255]);
            assert_eq!(rgba8(&page, 3, 3), [70, 100, 20, 255]);
            assert_eq!(rgba8(&img, 3, 3), [70, 100, 20, 255]);
            assert_eq!(rgba8(&img, 4, 4), [0, 0, 0, 0]);
        } else {
            assert_eq!(page.width, 6);
            assert!(!img.truncated && !page.truncated);
        }
    }
}

/// Krita's ops beyond the common ones: arithmetic modes, whole-colour modes and erase.
#[test]
fn krita_further_ops() {
    let run = |op: &str, s: [u8; 4], d: [u8; 4]| {
        let layers = format!(
            "{}{}",
            paint("s", &format!(r#"compositeop="{op}""#)),
            paint("d", "")
        );
        let f = kra_tree(
            2,
            2,
            "RGBA",
            "",
            &layers,
            &[("s", solid(&s)), ("d", solid(&d))],
        );
        let doc = Document::open(&f).unwrap();
        assert!(doc.kra_notes(&f, None, None).is_empty(), "{op}");
        rgba8(&doc.kra_composite(&f, &|| false).unwrap(), 1, 1)
    };
    // Pixels are B, G, R, A.
    let grey = |v: u8| [v, v, v, 255];
    assert_eq!(run("exclusion", grey(255), grey(64)), [191, 191, 191, 255]);
    assert_eq!(run("linear_burn", grey(128), grey(192)), [65, 65, 65, 255]);
    assert_eq!(run("allanon", grey(100), grey(200)), [150, 150, 150, 255]);
    // Erasing with a half-transparent layer halves the coverage below.
    assert_eq!(run("erase", [0, 0, 0, 128], grey(200))[3], 127);
    // Colour of pure red over grey keeps the grey's luminosity.
    let c = run("color", [0, 0, 255, 255], grey(128));
    assert!(c[0] > c[1] && c[1] == c[2], "{c:?}");
}
