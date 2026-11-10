use super::nrbf::write::*;
use super::*;
use std::io::Write as _;
use std::path::PathBuf;

fn gzip(d: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    e.write_all(d).unwrap();
    e.finish().unwrap()
}

/// A synthetic layer: name, visible, opacity, blend mode value, BGRA pixels.
struct L<'a>(&'a str, bool, u8, i32, Vec<u8>);

/// A Paint.NET 4.x-style document of `w`×`h` with `layers` (bottom first). `chunk` is the
/// deferred chunk size; `compressed` gzips the chunks; `whole_gzip` gzips the stream.
fn document(
    w: u32,
    h: u32,
    layers: &[L],
    chunk: u32,
    compressed: bool,
    whole_gzip: bool,
) -> Vec<u8> {
    let mut s = Vec::new();
    header(&mut s, 1);
    let n = layers.len() as i32;
    class_record(
        &mut s,
        1,
        "PaintDotNet.Document",
        &[
            M::Bool("isDisposed", false),
            M::Ref("layers", 3),
            M::Int("width", w as i32),
            M::Int("height", h as i32),
            M::Null("savedWith"),
        ],
    );
    class_record(
        &mut s,
        3,
        "PaintDotNet.LayerList",
        &[
            M::Ref("parent", 1),
            M::Ref("ArrayList+_items", 4),
            M::Int("ArrayList+_size", n),
            M::Int("ArrayList+_version", n),
        ],
    );
    let ids: Vec<Option<i32>> = (0..n).map(|i| Some(100 + i * 10)).chain([None]).collect();
    object_array(&mut s, 4, &ids);
    for (i, l) in layers.iter().enumerate() {
        let b = 100 + i as i32 * 10;
        class_record(
            &mut s,
            b,
            "PaintDotNet.BitmapLayer",
            &[
                M::Ref("properties", b + 1),
                M::Ref("surface", b + 2),
                M::Bool("Layer+isDisposed", false),
                M::Int("Layer+width", w as i32),
                M::Int("Layer+height", h as i32),
                M::Ref("Layer+properties", b + 3),
            ],
        );
        class_record(
            &mut s,
            b + 1,
            "PaintDotNet.BitmapLayer+BitmapLayerProperties",
            &[M::Ref("blendOp", b + 4)],
        );
        class_record(
            &mut s,
            b + 2,
            "PaintDotNet.Surface",
            &[
                M::Int("width", w as i32),
                M::Int("height", h as i32),
                M::Int("stride", w as i32 * 4),
                M::Ref("scan0", b + 5),
            ],
        );
        class_record(
            &mut s,
            b + 3,
            "PaintDotNet.Layer+LayerProperties",
            &[
                M::Str("name", b + 6, l.0),
                M::Bool("visible", l.1),
                M::Bool("isBackground", i == 0),
                M::Byte("opacity", l.2),
                M::Ref("blendMode", -(b + 7)),
            ],
        );
        class_record(&mut s, b + 4, "PaintDotNet.UserBlendOps+NormalBlendOp", &[]);
        class_record(
            &mut s,
            b + 5,
            "PaintDotNet.MemoryBlock",
            &[
                M::Long("length64", l.4.len() as i64),
                M::Bool("hasParent", false),
                M::Bool("deferred", true),
            ],
        );
        class_record(
            &mut s,
            -(b + 7),
            "PaintDotNet.LayerBlendMode",
            &[M::Int("value__", l.3)],
        );
    }
    s.push(11);
    for l in layers {
        s.push(if compressed { 0 } else { 1 });
        s.extend_from_slice(&chunk.to_be_bytes());
        let chunks: Vec<&[u8]> = l.4.chunks(chunk as usize).collect();
        // Chunks in reverse order: the format allows any order.
        for (k, c) in chunks.iter().enumerate().rev() {
            let body = if compressed { gzip(c) } else { c.to_vec() };
            s.extend_from_slice(&(k as u32).to_be_bytes());
            s.extend_from_slice(&(body.len() as u32).to_be_bytes());
            s.extend_from_slice(&body);
        }
    }
    let thumb = specular_png::encode(1, 1, specular_png::ColorType::Rgba, &[1, 2, 3, 4]).unwrap();
    let b64 = {
        const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut o = String::new();
        for c in thumb.chunks(3) {
            let v = (u32::from(c[0]) << 16)
                | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
                | u32::from(*c.get(2).unwrap_or(&0));
            for k in 0..4 {
                if k <= c.len() {
                    o.push(T[(v >> (18 - 6 * k) & 63) as usize] as char);
                } else {
                    o.push('=');
                }
            }
        }
        o
    };
    let xml = format!(
        "<pdnImage width=\"{w}\" height=\"{h}\" layers=\"{n}\" savedWithVersion=\"4.21.6589.7045\"><custom><thumb png=\"{b64}\" /></custom></pdnImage>"
    );
    let mut d = b"PDN3".to_vec();
    let len = xml.len() as u32;
    d.extend_from_slice(&len.to_le_bytes()[..3]);
    d.extend_from_slice(xml.as_bytes());
    if whole_gzip {
        d.extend_from_slice(&gzip(&s));
    } else {
        d.extend_from_slice(&[0, 1]);
        d.extend_from_slice(&s);
    }
    d
}

fn solid(w: u32, h: u32, bgra: [u8; 4]) -> Vec<u8> {
    (0..w * h).flat_map(|_| bgra).collect()
}

fn opts() -> Options<'static> {
    Options {
        parallel: true,
        cancel: None,
    }
}

#[test]
fn synthetic_document_layers_and_composite() {
    let (w, h) = (37, 11);
    let mut top = solid(w, h, [0, 0, 255, 128]); // red, half transparent
    top[0..4].copy_from_slice(&[0, 0, 0, 0]);
    for (chunk, compressed, whole) in [(64, true, false), (4096, false, false), (100, true, true)] {
        let d = document(
            w,
            h,
            &[
                L("Background", true, 255, 0, solid(w, h, [255, 0, 0, 255])),
                L("Top", true, 255, 0, top.clone()),
                L("Hidden", false, 255, 0, solid(w, h, [0, 255, 0, 255])),
            ],
            chunk,
            compressed,
            whole,
        );
        assert!(probe(&d));
        let hd = read_header(&d).unwrap();
        assert_eq!((hd.width, hd.height, hd.layers), (w, h, 3));
        assert_eq!(hd.saved_with.as_deref(), Some("4.21.6589.7045"));
        assert!(hd.thumbnail_png.as_ref().unwrap().starts_with(b"\x89PNG"));
        let p = parse(&d).unwrap();
        assert_eq!(p.layers().len(), 3);
        assert_eq!(p.layers()[1].name, "Top");
        assert!(!p.layers()[2].visible);
        let img = p.render(&d, &opts()).unwrap();
        assert!(!img.truncated);
        // Pixel 0: the background only (blue); others: red over blue at alpha 128.
        assert_eq!(&img.data[0..4], &[0, 0, 255, 255]);
        assert_eq!(&img.data[4..8], &[128, 0, 127, 255]);
        let l = p.render_layer(&d, 2, &opts()).unwrap();
        assert_eq!(&l.data[0..4], &[0, 255, 0, 255]);
        // Serial path gives the same pixels.
        let serial = p
            .render(
                &d,
                &Options {
                    parallel: false,
                    cancel: None,
                },
            )
            .unwrap();
        assert_eq!(serial.data, img.data);
    }
}

#[test]
fn blend_functions() {
    use BlendMode::*;
    assert_eq!(blend_channel(Multiply, 255, 128), 128);
    assert_eq!(blend_channel(Screen, 0, 77), 77);
    assert_eq!(blend_channel(Additive, 200, 100), 255);
    assert_eq!(blend_channel(Difference, 10, 200), 190);
    assert_eq!(blend_channel(Negation, 200, 200), 110);
    assert_eq!(blend_channel(ColorBurn, 100, 0), 0);
    assert_eq!(blend_channel(ColorDodge, 100, 255), 255);
    assert_eq!(blend_channel(Xor, 0b1100, 0b1010), 0b0110);
    // Opacity 0 and a transparent source leave the destination.
    let mut d = [1, 2, 3, 4];
    blend_pixel(Normal, 0, &mut d, &[9, 9, 9, 255]);
    assert_eq!(d, [1, 2, 3, 4]);
}

#[test]
fn damaged_documents_fail_cleanly_or_truncate() {
    let (w, h) = (9, 7);
    let good = document(
        w,
        h,
        &[
            L("A", true, 255, 0, solid(w, h, [1, 2, 3, 255])),
            L("B", true, 200, 7, solid(w, h, [4, 5, 6, 100])),
        ],
        40,
        true,
        false,
    );
    assert!(parse(&good).is_ok());
    // Cut short in the pixel data: the document opens and says it is truncated.
    let cut = &good[..good.len() - 30];
    let p = parse(cut).unwrap();
    let img = p.render(cut, &opts()).unwrap();
    assert!(img.truncated);
    for n in 0..good.len() {
        if let Ok(p) = parse(&good[..n]) {
            let _ = p.render(&good[..n], &opts());
        }
    }
    let mut seed = 7u32;
    for _ in 0..1500 {
        let mut b = good.clone();
        for _ in 0..4 {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let at = seed as usize % b.len();
            b[at] = (seed >> 8) as u8;
        }
        if let Ok(p) = parse(&b) {
            let _ = p.render(&b, &opts());
            for i in 0..p.layers().len() {
                let _ = p.render_layer(&b, i, &opts());
            }
        }
    }
    assert!(matches!(parse(b"PDN3"), Err(Error::Truncated(_))));
    assert!(matches!(parse(b"GIF89a"), Err(Error::NotPdn)));
}

#[test]
fn huge_canvas_is_refused() {
    // A 2^20 × 2^20 canvas.
    let mut s2 = Vec::new();
    header(&mut s2, 1);
    class_record(
        &mut s2,
        1,
        "PaintDotNet.Document",
        &[
            M::Ref("layers", 3),
            M::Int("width", 1 << 20),
            M::Int("height", 1 << 20),
        ],
    );
    class_record(
        &mut s2,
        3,
        "PaintDotNet.LayerList",
        &[M::Null("ArrayList+_items"), M::Int("ArrayList+_size", 0)],
    );
    s2.push(11);
    let mut f = b"PDN3\x0b\x00\x00<pdnImage/>\x00\x01".to_vec();
    f.extend_from_slice(&s2);
    assert_eq!(parse(&f).err(), Some(Error::TooLarge));
}

#[test]
fn cancel_stops_render() {
    let (w, h) = (64, 64);
    let d = document(
        w,
        h,
        &[L("A", true, 255, 0, solid(w, h, [1, 2, 3, 255]))],
        256,
        true,
        false,
    );
    let p = parse(&d).unwrap();
    let stop = || true;
    let o = Options {
        parallel: true,
        cancel: Some(&stop),
    };
    assert_eq!(p.render(&d, &o).err(), Some(Error::Cancelled));
}

// ---- corpus (tools/corpus/fetch-editors.sh pdn_*; skipped when absent) ----

fn corpus() -> Option<PathBuf> {
    let p = std::env::var_os("SPECULAR_EDITORS_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/editors")
        })
        .join("pdn");
    p.is_dir().then_some(p)
}

fn pdn_files(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdn")) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

fn png_rgba(path: &std::path::Path) -> (u32, u32, Vec<u8>) {
    let d = std::fs::read(path).unwrap();
    let img = specular_png::decode(
        &d,
        &specular_png::DecodeOptions {
            output: specular_png::Output::Rgba8,
            ..Default::default()
        },
    )
    .unwrap();
    (img.width, img.height, img.to_rgba8())
}

/// Pixels equal, and pixels within `tol` per channel, of two straight-alpha RGBA images;
/// colour is ignored where both are fully transparent.
fn compare(a: &[u8], b: &[u8], tol: u8) -> (usize, usize, u8) {
    let (mut exact, mut close, mut max) = (0, 0, 0u8);
    for (p, q) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let d = if p[3] == 0 && q[3] == 0 {
            0
        } else {
            p.iter()
                .zip(q)
                .map(|(x, y)| x.abs_diff(*y))
                .max()
                .unwrap_or(0)
        };
        max = max.max(d);
        exact += usize::from(d == 0);
        close += usize::from(d <= tol);
    }
    (exact, close, max)
}

#[test]
fn corpus_layers_match_pypdn() {
    let Some(dir) = corpus() else {
        eprintln!("PDN corpus absent: skipped");
        return;
    };
    let (mut files, mut layers_ok, mut layers_bad) = (0, 0, 0);
    for f in pdn_files(&dir) {
        let j = PathBuf::from(format!("{}.layers.json", f.display()));
        let Ok(meta) = std::fs::read_to_string(&j) else {
            continue;
        };
        let meta: serde_json::Value = serde_json::from_str(&meta).unwrap();
        let raw = std::fs::read(format!("{}.layers.rgba", f.display())).unwrap();
        let d = std::fs::read(&f).unwrap();
        let p = parse(&d).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let ml = meta["layers"].as_array().unwrap();
        assert_eq!(p.layers().len(), ml.len(), "{}", f.display());
        let n = (p.width() * p.height() * 4) as usize;
        files += 1;
        for (i, (l, m)) in p.layers().iter().zip(ml).enumerate() {
            assert_eq!(l.name, m["name"].as_str().unwrap(), "{}", f.display());
            assert_eq!(l.visible, m["visible"].as_bool().unwrap());
            assert_eq!(i64::from(l.opacity), m["opacity"].as_i64().unwrap());
            assert_eq!(
                Some(l.blend),
                BlendMode::from_value(m["blend"].as_i64().unwrap()),
                "{} layer {i}",
                f.display()
            );
            // The layer's own pixels: drawn opaque onto transparent, they are copied.
            let mut probe = p.clone_for_test();
            probe.layers[i].opacity = 255;
            let img = probe.render_layer(&d, i, &opts()).unwrap();
            let want = &raw[i * n..(i + 1) * n];
            let (exact, _, max) = compare(&img.data, want, 0);
            if exact == n / 4 {
                layers_ok += 1;
            } else {
                layers_bad += 1;
                eprintln!(
                    "{} layer {i}: {} of {} pixels differ (max {max})",
                    f.display(),
                    n / 4 - exact,
                    n / 4
                );
            }
        }
    }
    eprintln!("pypdn layer oracle: {files} files, {layers_ok} layers equal, {layers_bad} differ");
    assert_eq!(layers_bad, 0);
}

#[test]
fn corpus_flatten_matches_paint_net() {
    let Some(dir) = corpus() else {
        eprintln!("PDN corpus absent: skipped");
        return;
    };
    let py = dir.join("pypdn");
    let Ok(d) = std::fs::read(py.join("FlattenBlendTest.pdn")) else {
        return;
    };
    let p = parse(&d).unwrap();
    let mut worst = 0u8;
    let mut report = Vec::new();
    let mut cases: Vec<(PathBuf, Vec<u8>, Vec<usize>)> = Vec::new();
    for (i, l) in p.layers().iter().enumerate().skip(1) {
        let png = py.join(format!("Flatten{}Test.png", l.name));
        let png2 = py.join(format!("flatten{}Test.png", l.name));
        let png = if png.exists() { png } else { png2 };
        cases.push((png, d.clone(), vec![0, i]));
    }
    // As pypdn's tests pair them: Untitled2's background alone, then both layers.
    for (pdn, png, layers) in [
        ("Untitled2.pdn", "flattenNormalTest2.png", vec![0]),
        ("Untitled2.pdn", "flattenNormalTest.png", vec![0, 1]),
        ("Untitled3.pdn", "flattenOpacityTest.png", vec![]),
        ("oldPDN3510.pdn", "oldPDN3510.png", vec![]),
    ] {
        if let Ok(b) = std::fs::read(py.join(pdn)) {
            cases.push((py.join(png), b, layers));
        }
    }
    for (png, data, layers) in cases {
        if !png.exists() {
            continue;
        }
        let p = parse(&data).unwrap();
        let img = if layers.is_empty() {
            p.render(&data, &opts()).unwrap()
        } else {
            p.render_layers(&data, &layers, &opts()).unwrap()
        };
        let (w, h, want) = png_rgba(&png);
        assert_eq!((w, h), (img.width, img.height), "{}", png.display());
        let (exact, close, max) = compare(&img.data, &want, 1);
        let n = (w * h) as usize;
        worst = worst.max(max);
        report.push(format!(
            "{}: {:.2} % exact, {:.2} % within 1, max {max}",
            png.file_name().unwrap().to_string_lossy(),
            100.0 * exact as f64 / n as f64,
            100.0 * close as f64 / n as f64
        ));
    }
    for r in &report {
        eprintln!("{r}");
    }
    assert!(worst <= 2, "a flattened image is off by {worst}");
}

#[test]
fn corpus_render_matches_embedded_thumbnails() {
    let Some(dir) = corpus() else {
        eprintln!("PDN corpus absent: skipped");
        return;
    };
    let mut worst = 0f64;
    for f in pdn_files(&dir) {
        let d = std::fs::read(&f).unwrap();
        let p = parse(&d).unwrap();
        let t0 = std::time::Instant::now();
        let img = p.render(&d, &opts()).unwrap();
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        let Some(thumb) = p.header().thumbnail_png.as_ref() else {
            continue;
        };
        let t = specular_png::decode(
            thumb,
            &specular_png::DecodeOptions {
                output: specular_png::Output::Rgba8,
                ..Default::default()
            },
        )
        .unwrap();
        let want = t.to_rgba8();
        // Paint.NET 3.x flattened its thumbnails onto white.
        let on_white = want.chunks_exact(4).all(|p| p[3] == 255);
        // Box-average our composite to the thumbnail's size (alpha-weighted colour) and
        // compare the mean channel difference of premultiplied values.
        let (tw, th) = (t.width as usize, t.height as usize);
        let (w, h) = (img.width as usize, img.height as usize);
        let mut sum = 0f64;
        for ty in 0..th {
            for tx in 0..tw {
                let (x0, x1) = (tx * w / tw, ((tx + 1) * w / tw).max(tx * w / tw + 1));
                let (y0, y1) = (ty * h / th, ((ty + 1) * h / th).max(ty * h / th + 1));
                let mut acc = [0f64; 4];
                for y in y0..y1.min(h) {
                    for x in x0..x1.min(w) {
                        let px = &img.data[(y * w + x) * 4..][..4];
                        let a = f64::from(px[3]) / 255.0;
                        for c in 0..3 {
                            acc[c] += f64::from(px[c]) * a;
                        }
                        acc[3] += f64::from(px[3]);
                    }
                }
                let n = ((x1.min(w) - x0) * (y1.min(h) - y0)).max(1) as f64;
                if on_white {
                    for c in 0..3 {
                        acc[c] += 255.0 * n - acc[3];
                    }
                    acc[3] = 255.0 * n;
                }
                let q = &want[(ty * tw + tx) * 4..][..4];
                let qa = f64::from(q[3]) / 255.0;
                for c in 0..3 {
                    sum += (acc[c] / n - f64::from(q[c]) * qa).abs();
                }
                sum += (acc[3] / n - f64::from(q[3])).abs();
            }
        }
        let mean = sum / (tw * th * 4) as f64;
        worst = worst.max(mean);
        eprintln!(
            "{}: {}x{} {} layers, render {ms:.1} ms, thumbnail {tw}x{th} mean diff {mean:.2}",
            f.file_name().unwrap().to_string_lossy(),
            w,
            h,
            p.layers().len()
        );
    }
    assert!(
        worst < 3.0,
        "a composite strays from its thumbnail ({worst:.2})"
    );
}

impl Pdn {
    fn clone_for_test(&self) -> Pdn {
        Pdn {
            header: self.header.clone(),
            width: self.width,
            height: self.height,
            layers: self.layers.clone(),
            blocks: self.blocks.clone(),
            unpacked: self.unpacked.clone(),
            body: self.body,
            truncated: self.truncated,
        }
    }
}
