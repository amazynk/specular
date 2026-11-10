//! Goldens for text (bundled fonts only, so the render does not depend on the machine) and for
//! embedded rasters. `SPECULAR_BLESS=1` rewrites them.

use super::*;

fn open(xml: &str, set: FontSet) -> Arc<dyn Session> {
    open_svg(
        Arc::new(Source::from_vec("t.svg", xml.as_bytes().to_vec())),
        set,
    )
    .unwrap()
}

fn render(s: &dyn Session, scale: u32) -> Raster {
    s.decode(&DecodeRequest::full(scale), &CancelToken::new())
        .unwrap()
}

fn px(r: &Raster, x: u32, y: u32) -> [u8; 4] {
    let p = &r.planes[0];
    let i = y as usize * p.stride + x as usize * 4;
    p.data[i..i + 4].try_into().unwrap()
}

/// Compares with `tests/svg_goldens/<name>.png`: at most 1 in 2000 pixels may differ by more
/// than 16 in a channel (rasteriser and font-hinting noise).
fn golden(name: &str, r: &Raster) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/svg_goldens")
        .join(format!("{name}.png"));
    let (w, h, got) = to_rgba8(r.clone()).unwrap();
    if std::env::var_os("SPECULAR_BLESS").is_some_and(|v| v == "1") {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let png = specular_png::encode(w, h, specular_png::ColorType::Rgba, &got).unwrap();
        std::fs::write(&path, png).unwrap();
        return;
    }
    let want = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("{}: {e} (bless with SPECULAR_BLESS=1)", path.display()));
    let (ww, wh, want) = decode_rgba(&want).unwrap();
    assert_eq!((w, h), (ww, wh), "{name}: size");
    let bad = got
        .chunks_exact(4)
        .zip(want.chunks_exact(4))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 16))
        .count();
    assert!(
        bad * 2000 <= (w * h) as usize,
        "{name}: {bad} of {} pixels differ",
        w * h
    );
}

fn base64(data: &[u8]) -> String {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in data.chunks(3) {
        let n = c
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..4 {
            s.push(if i <= c.len() {
                A[(n >> (18 - 6 * i) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    s
}

#[test]
fn text_golden() {
    let xml = r##"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="72" viewBox="0 0 240 72">
  <text x="8" y="40" font-family="sans-serif" font-size="36" fill="#1a1a1a">Specular</text>
  <text x="8" y="64" font-size="16" fill="#c00000">Serif? שלום · Ωμέγα</text>
</svg>"##;
    assert!(needs_fonts(xml.as_bytes()));
    let s = open(xml, FontSet::BundledOnly);
    assert!(s.info().coding.contains("text"), "{}", s.info().coding);
    let r = render(s.as_ref(), 4);
    let inked = r.planes[0]
        .data
        .chunks_exact(4)
        .filter(|p| p[3] > 128)
        .count();
    assert!(inked > 500, "text drawn: {inked} px");
    golden("text", &r);
}

#[test]
fn data_png_golden_and_cache() {
    // 4×4: opaque red/blue checks, a half-transparent green row, a transparent pixel.
    let mut rgba = Vec::new();
    for y in 0..4u32 {
        for x in 0..4u32 {
            rgba.extend_from_slice(&match (x, y) {
                (3, 3) => [0, 0, 0, 0],
                (_, 3) => [0, 255, 0, 128],
                _ if (x + y) % 2 == 0 => [255, 0, 0, 255],
                _ => [0, 0, 255, 255],
            });
        }
    }
    let png = specular_png::encode(4, 4, specular_png::ColorType::Rgba, &rgba).unwrap();
    let xml = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="64" height="64">
  <rect x="0" y="32" width="64" height="32" fill="#ffff00"/>
  <image x="0" y="0" width="64" height="64" image-rendering="optimizeSpeed" xlink:href="data:image/png;base64,{}"/>
</svg>"##,
        base64(&png)
    );
    let s = open(&xml, FontSet::BundledOnly);
    assert!(!s.info().coding.contains("text"));
    let r = render(s.as_ref(), 8);
    assert_eq!((r.width, r.height), (128, 128));
    assert_eq!(px(&r, 8, 8), [255, 0, 0, 255]);
    assert_eq!(px(&r, 40, 8), [0, 0, 255, 255]);
    // Half-transparent green over yellow.
    let g = px(&r, 8, 120);
    assert!(
        g[0].abs_diff(127) <= 2 && g[1] == 255 && g[2] == 0 && g[3] == 255,
        "{g:?}"
    );
    assert_eq!(px(&r, 120, 120), [255, 255, 0, 255]);
    golden("data-png", &r);

    // Decoded once per session, kept for later renders, dropped by a trim.
    assert_eq!(s.memory_bytes(), 4 * 4 * 4);
    let _ = render(s.as_ref(), 4);
    assert_eq!(s.memory_bytes(), 4 * 4 * 4);
    assert_eq!(s.trim_memory(), 64);
    assert_eq!(s.memory_bytes(), 0);
    assert_eq!(px(&render(s.as_ref(), 8), 8, 8), [255, 0, 0, 255]);
}

/// Open + render times: a text-less SVG through the old path (usvg with default options) and
/// the new one, then the first and second opens of an SVG with text (the first loads the
/// system fonts). Run alone, so the font database is cold:
/// `cargo test -p specular-decode --lib svg::tests::timings -- --ignored --nocapture`, with
/// `SPECULAR_SVG_BENCH_TEXT=<file.svg>` for the text SVG.
#[test]
#[ignore]
fn timings() {
    let best = |f: &mut dyn FnMut()| {
        (0..7)
            .map(|_| {
                let t = std::time::Instant::now();
                f();
                t.elapsed().as_secs_f64() * 1e3
            })
            .fold(f64::MAX, f64::min)
    };
    let mut plain = String::from(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600"><style>.k{stroke:#123}</style>"#,
    );
    for i in 0..4000u32 {
        let (x, y) = (i % 80 * 10, i / 80 * 12);
        plain.push_str(&format!(
            r##"<path class="k" d="M{x} {y} l8 0 l-4 9 z" fill="#{:06x}"/>"##,
            i.wrapping_mul(2654435761) & 0xffffff
        ));
    }
    plain.push_str("</svg>");
    let old = best(&mut || {
        let tree = usvg::Tree::from_data(plain.as_bytes(), &usvg::Options::default()).unwrap();
        let mut pm = tiny_skia::Pixmap::new(400, 300).unwrap();
        resvg::render(
            &tree,
            tiny_skia::Transform::from_scale(0.5, 0.5),
            &mut pm.as_mut(),
        );
    });
    let new = best(&mut || {
        let s = open(&plain, FontSet::System);
        assert!(!s.info().coding.contains("text"));
        let _ = render(s.as_ref(), 4);
    });
    let scan = best(&mut || {
        assert!(!needs_fonts(plain.as_bytes()) && svg_css::resolve_vars(&plain).is_none());
    });
    eprintln!(
        "text-less SVG ({} KB, 4000 paths), open + render 400×300: old path {old:.2} ms, new {new:.2} ms (pre-scans {scan:.3} ms)",
        plain.len() / 1024
    );
    let small = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="40"><text x="4" y="30" font-size="24">Hello, world</text></svg>"#;
    let t = std::time::Instant::now();
    let s = open(small, FontSet::System);
    let first = t.elapsed().as_secs_f64() * 1e3;
    let _ = render(s.as_ref(), 1);
    let later = best(&mut || {
        let _ = render(open(small, FontSet::System).as_ref(), 1);
    });
    eprintln!(
        "small text SVG: first open {first:.1} ms (builds the font database: {} faces), later open + render {later:.2} ms",
        fonts(FontSet::System).db.len()
    );
    if let Some(path) = std::env::var_os("SPECULAR_SVG_BENCH_TEXT") {
        let src = Arc::new(Source::open(&path).unwrap());
        let t = std::time::Instant::now();
        let s = open_svg(src.clone(), FontSet::System).unwrap();
        let first = t.elapsed().as_secs_f64() * 1e3;
        let t = std::time::Instant::now();
        let _ = render(s.as_ref(), 8);
        let render_ms = t.elapsed().as_secs_f64() * 1e3;
        let second = best(&mut || {
            let _ = open_svg(src.clone(), FontSet::System).unwrap();
        });
        eprintln!(
            "text SVG {} ({} KB): first open {first:.1} ms (system fonts loaded), later opens {second:.1} ms, render at 1/8 {render_ms:.1} ms; {} faces",
            Path::new(&path).display(),
            src.len() / 1024,
            fonts(FontSet::System).db.len()
        );
    }
}

#[test]
fn font_detection() {
    assert!(!needs_fonts(
        br#"<svg><rect class="text-x" id="text"/></svg>"#
    ));
    assert!(!needs_fonts(
        br#"<svg><style>.text{fill:red}</style></svg>"#
    ));
    assert!(needs_fonts(br#"<svg><text>a</text></svg>"#));
    assert!(needs_fonts(
        br#"<svg:svg><svg:text x="1">a</svg:text></svg:svg>"#
    ));
    assert!(needs_fonts(br#"<svg><text/></svg>"#));
    assert!(needs_fonts(
        br#"<svg><image href="data:image/svg+xml;base64,AA"/></svg>"#
    ));
    assert!(needs_fonts(br#"<svg><image href="b.svg"/></svg>"#));
}

#[test]
fn references() {
    for refused in [
        "http://example.com/a.png",
        "https://example.com/a.png",
        "file:///etc/passwd",
        "//example.com/a.png",
        "/etc/passwd",
        "../a.png",
        "sub/../../a.png",
        "a%2F..%2F..%2Fb.png",
        "C:\\a.png",
        "",
    ] {
        assert!(local_reference(refused).is_none(), "{refused}");
    }
    assert_eq!(
        local_reference("sub/my%20pic.png?v=2#frag"),
        Some(PathBuf::from("sub/my pic.png"))
    );
    assert_eq!(local_reference("./a.png"), Some(PathBuf::from("./a.png")));
}

fn gzip(data: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

/// SVG `data:` images nested 80 deep (gzip'd) stop at
/// [`MAX_NESTED_SVG`]: on a small stack the file opens instead of overflowing it.
#[test]
fn nested_svg_images_stop() {
    let mut svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="red"/></svg>"#.to_owned();
    for _ in 0..80 {
        svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><image width="8" height="8" href="data:image/svg+xml;base64,{}"/></svg>"#,
            base64(&gzip(svg.as_bytes()))
        );
    }
    assert!(svg.len() < 4_000_000, "{}", svg.len());
    specular_thread::on_stack(256 << 10, || {
        let s = open(&svg, FontSet::BundledOnly);
        render(s.as_ref(), 1);
    });
}

/// usvg refuses elements nested deeper than 1024 itself; its converter and resvg's renderer
/// recurse once per level below that, which must fit a decode thread's 2 MiB stack.
#[test]
fn deeply_nested_groups_fit_the_stack() {
    let svg = |depth: usize| {
        let mut s = r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">"#.to_owned();
        s.push_str(&r#"<g opacity="0.9">"#.repeat(depth));
        s.push_str(r#"<rect width="8" height="8" fill="red"/>"#);
        s.push_str(&"</g>".repeat(depth));
        s.push_str("</svg>");
        s
    };
    specular_thread::on_stack(2 << 20, || {
        let open = |depth| {
            open_svg(
                Arc::new(Source::from_vec("t.svg", svg(depth).into_bytes())),
                FontSet::BundledOnly,
            )
        };
        if let Ok(s) = open(1000) {
            let _ = s.decode(&DecodeRequest::full(1), &CancelToken::new());
        }
        assert!(open(100_000).is_err());
    });
}

/// `@font-face` with a `data:` font: text in the rule's family draws exactly as with the
/// font's own family name, and not as the fallback face does without the rule.
#[test]
fn font_face_data_url() {
    // The bundled face least like the default (the last one listed).
    let fonts = specular_theme::fonts::all();
    let data = fonts[fonts.len() - 1];
    let mut db = fontdb::Database::new();
    db.load_font_data(data.to_vec());
    let real = db.faces().next().unwrap().families[0].0.clone();
    let doc = |rule: bool, family: &str| {
        let style = if rule {
            format!(
                "<style>@font-face {{ font-family: 'Zed Face'; src: url(\"data:font/ttf;base64,{}\") format('truetype'); }}</style>",
                base64(data)
            )
        } else {
            String::new()
        };
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="60">{style}<text x="4" y="44" font-family="{family}" font-size="36">Wy0Ag</text></svg>"##
        )
    };
    let draw = |xml: String| {
        let s = open(&xml, FontSet::BundledOnly);
        (
            s.info().coding.clone(),
            render(s.as_ref(), 1).planes[0].data.clone(),
        )
    };
    let (coding, with_rule) = draw(doc(true, "Zed Face"));
    assert!(coding.contains("embedded fonts"), "{coding}");
    let (_, by_name) = draw(doc(false, &real));
    let (_, fallback) = draw(doc(false, "Zed Face"));
    assert_eq!(with_rule, by_name, "the @font-face face is the font's own");
    assert_ne!(with_rule, fallback, "without the rule another face draws");
}
