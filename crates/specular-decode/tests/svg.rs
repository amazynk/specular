//! SVG/SVGZ through the registry: size choice, scales, regions, transparency.

use specular_core::{CancelToken, Source};
use specular_decode::{DecodeRequest, Format, Rect, Registry};
use std::io::Write;
use std::sync::Arc;

const SVG: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<!-- a comment that pushes the svg element past the first 64 bytes of the file -->
<svg xmlns="http://www.w3.org/2000/svg" width="32" height="16" viewBox="0 0 32 16">
  <rect x="0" y="0" width="16" height="16" fill="#ff0000"/>
  <rect x="16" y="0" width="16" height="16" fill="#0000ff" fill-opacity="0.5"/>
</svg>"##;

fn px(r: &specular_core::Raster, x: u32, y: u32) -> [u8; 4] {
    let p = &r.planes[0];
    let i = y as usize * p.stride + x as usize * 4;
    p.data[i..i + 4].try_into().unwrap()
}

#[test]
fn svg_scales_and_regions() {
    let reg = Registry::default();
    let s = reg
        .open(Arc::new(Source::from_vec("a.svg", SVG.as_bytes().to_vec())))
        .unwrap();
    assert_eq!(s.info().format, Format::Svg);
    // 32×16 intrinsic → ×32 so the long side reaches 1024.
    assert_eq!((s.info().width, s.info().height), (1024, 512));
    let c = CancelToken::new();
    let r = s.decode(&DecodeRequest::full(8), &c).unwrap();
    assert_eq!((r.width, r.height, r.scale), (128, 64, 8));
    assert_eq!(px(&r, 10, 10), [255, 0, 0, 255]);
    let b = px(&r, 100, 10);
    assert_eq!((b[0], b[1], b[3]), (0, 0, 128));
    assert!(b[2] >= 254, "straight alpha: {b:?}");
    let req = DecodeRequest {
        scale: 1,
        region: Some(Rect {
            x: 500,
            y: 100,
            w: 48,
            h: 8,
        }),
        parallel: true,
        prepare_regions: false,
    };
    let reg_r = s.decode(&req, &c).unwrap();
    assert_eq!(
        (reg_r.origin, reg_r.width, reg_r.height),
        ((500, 100), 48, 8)
    );
    assert_eq!(px(&reg_r, 0, 0), [255, 0, 0, 255]);
    assert_eq!(px(&reg_r, 47, 7)[3], 128);

    // Transparent background where nothing is drawn.
    let t = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><circle cx="5" cy="5" r="1"/></svg>"#;
    let s = reg
        .open(Arc::new(Source::from_vec("t.svg", t.as_bytes().to_vec())))
        .unwrap();
    assert_eq!(
        px(&s.decode(&DecodeRequest::full(8), &c).unwrap(), 0, 0),
        [0, 0, 0, 0]
    );
}

#[test]
fn svgz_and_malformed() {
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(SVG.as_bytes()).unwrap();
    let data = gz.finish().unwrap();
    let reg = Registry::default();
    let s = reg
        .open(Arc::new(Source::from_vec("a.svgz", data)))
        .unwrap();
    assert_eq!(s.info().width, 1024);
    for n in 0..SVG.len() {
        if let Ok(s) = reg.open(Arc::new(Source::from_vec(
            "x.svg",
            SVG.as_bytes()[..n].to_vec(),
        ))) {
            let _ = s.decode(&DecodeRequest::full(8), &CancelToken::new());
        }
    }
    assert!(
        reg.open(Arc::new(Source::from_vec("x.svg", b"<svg".to_vec())))
            .is_err()
    );
}

fn render(reg: &Registry, path: &str, xml: &str) -> specular_core::Raster {
    let s = reg
        .open(Arc::new(Source::from_vec(path, xml.as_bytes().to_vec())))
        .unwrap();
    s.decode(&DecodeRequest::full(8), &CancelToken::new())
        .unwrap()
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
fn css_variables_and_tailwind_current() {
    let reg = Registry::default();
    // A custom property in a rule, one in a style attribute, a fallback, a chain, and a rule
    // whose other declarations follow a custom property (simplecss would drop them).
    let xml = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="10">
  <style>
    :root { --base: #00ff00; --accent: var(--base); }
    @media (prefers-color-scheme: dark) { :root { --base: #ffffff; } }
    .a { --unused: 1px; fill: var(--accent); }
    .b { fill: var(--nope, #0000ff); }
  </style>
  <rect class="a" x="0" width="10" height="10"/>
  <rect class="b" x="10" width="10" height="10"/>
  <rect style="--c: #ff0000; fill: var(--c)" x="20" width="10" height="10"/>
  <g fill="#ff00ff"><rect fill="var(--undefined)" x="30" width="10" height="10"/></g>
</svg>"##;
    let r = render(&reg, "v.svg", xml);
    const W: u32 = 40;
    assert_eq!(px(&r, 4 * r.width / W, 4 * r.width / W), [0, 255, 0, 255]);
    assert_eq!(px(&r, 14 * r.width / W, 4 * r.width / W), [0, 0, 255, 255]);
    assert_eq!(px(&r, 24 * r.width / W, 4 * r.width / W), [255, 0, 0, 255]);
    // No value and no fallback: inherited.
    assert_eq!(
        px(&r, 34 * r.width / W, 4 * r.width / W),
        [255, 0, 255, 255]
    );

    let xml = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10" color="#0000ff">
  <rect class="w-4 fill-current" width="10" height="10"/>
  <rect class="stroke-current" x="12" y="2" width="6" height="6" fill="none" stroke-width="2"/>
</svg>"##;
    let r = render(&reg, "t.svg", xml);
    assert_eq!(px(&r, 4 * r.width / 20, 4 * r.width / 20), [0, 0, 255, 255]);
    assert_eq!(
        px(&r, 12 * r.width / 20, 5 * r.width / 20),
        [0, 0, 255, 255]
    );
}

#[test]
fn jpeg_data_uri() {
    let mut jpeg = Vec::new();
    let rgb: Vec<u8> = [200u8, 40, 40].repeat(16 * 16);
    jpeg_encoder::Encoder::new(&mut jpeg, 95)
        .encode(&rgb, 16, 16, jpeg_encoder::ColorType::Rgb)
        .unwrap();
    // An unhelpful MIME type: the content is probed.
    let xml = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><image width="16" height="16" href="data:application/octet-stream;base64,{}"/></svg>"#,
        base64(&jpeg)
    );
    let r = render(&Registry::default(), "j.svg", &xml);
    let p = px(&r, r.width / 2, r.height / 2);
    assert!(
        p[0].abs_diff(200) <= 3 && p[1].abs_diff(40) <= 3 && p[2].abs_diff(40) <= 3 && p[3] == 255,
        "{p:?}"
    );
}

#[test]
fn external_references() {
    let root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("svg-refs");
    let _ = std::fs::remove_dir_all(&root);
    let dir = root.join("art");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    let png = |c: [u8; 4]| {
        specular_png::encode(2, 2, specular_png::ColorType::Rgba, &c.repeat(4)).unwrap()
    };
    std::fs::write(dir.join("sub/in.png"), png([0, 255, 0, 255])).unwrap();
    std::fs::write(root.join("outside.png"), png([255, 0, 0, 255])).unwrap();
    std::fs::write(
        dir.join("nested.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4" fill="#0000ff"/></svg>"##,
    )
    .unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(root.join("outside.png"), dir.join("link.png")).unwrap();
    let outside = root.join("outside.png").to_str().unwrap().to_owned();
    let file_url = format!("file://{outside}");
    let hrefs: [(&str, Option<[u8; 4]>); 11] = [
        ("sub/in.png", Some([0, 255, 0, 255])),
        ("./sub/in%2Epng", Some([0, 255, 0, 255])),
        ("nested.svg", Some([0, 0, 255, 255])),
        ("../outside.png", None),
        ("sub/../../outside.png", None),
        (&outside, None),
        (&file_url, None),
        ("link.png", None),
        ("http://127.0.0.1:9/a.png", None),
        ("https://example.invalid/a.png", None),
        ("missing.png", None),
    ];
    let reg = Registry::default();
    let path = dir.join("doc.svg");
    for (href, want) in hrefs {
        let xml = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><image width="16" height="16" href="{href}"/></svg>"#
        );
        let s = reg
            .open(Arc::new(Source::from_vec(&path, xml.into_bytes())))
            .unwrap();
        let r = s
            .decode(&DecodeRequest::full(8), &CancelToken::new())
            .unwrap();
        assert_eq!(
            px(&r, r.width / 2, r.height / 2),
            want.unwrap_or([0, 0, 0, 0]),
            "{href}"
        );
    }
    // An SVG without a folder (in memory) reads no files at all.
    let xml = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><image width="16" height="16" href="sub/in.png"/></svg>"#;
    let r = render(&reg, "doc.svg", xml);
    assert_eq!(px(&r, r.width / 2, r.height / 2), [0, 0, 0, 0]);
}

/// Opens and renders (at 1/8) every SVG/SVGZ under `SPECULAR_SVG_SWEEP_DIR` (e.g. the resvg
/// test suite in tools/corpus/conformance/resvg): no panics, and the counts.
#[test]
#[ignore]
fn sweep() {
    let Some(root) = std::env::var_os("SPECULAR_SVG_SWEEP_DIR") else {
        return;
    };
    let mut files = Vec::new();
    let mut dirs = vec![std::path::PathBuf::from(root)];
    while let Some(d) = dirs.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                dirs.push(p);
            } else if p
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("svg") || x.eq_ignore_ascii_case("svgz"))
            {
                files.push(p);
            }
        }
    }
    let reg = Registry::default();
    let (mut ok, mut failed) = (0, 0);
    let t = std::time::Instant::now();
    for p in &files {
        match reg.open_path(p) {
            Ok(s) => {
                let _ = s.decode(&DecodeRequest::full(8), &CancelToken::new());
                ok += 1;
            }
            Err(_) => failed += 1,
        }
    }
    eprintln!(
        "{} files: {ok} rendered, {failed} rejected, {:.1} s",
        files.len(),
        t.elapsed().as_secs_f64()
    );
}
