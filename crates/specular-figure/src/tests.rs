//! Figures written and read back: SVG through roxmltree (and its vectors through resvg), PDF
//! through hayro (specular-pdf) and its own content stream, the raster sample for sample, the
//! PPTX through the ZIP reader and roxmltree, and the GPU readback against the source codes.

use std::collections::HashMap;
use std::io::Read as _;

use crate::compose::{InsetSpec, LegendSpec, Mark, Panel, ScaleSpec, Spec, compose, inset_size};
use crate::text::Boxes;
use crate::*;

/// 8-bit codes (×257) in a pattern that shows any shift or flip.
fn pattern(w: u32, h: u32, salt: u32) -> Rgb16 {
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let v = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77) ^ salt;
            rgb.extend_from_slice(&[(v >> 24) as u8, (v >> 16) as u8, (x * 255 / w.max(1)) as u8]);
        }
    }
    Rgb16::from_rgb8(w, h, &rgb)
}

fn viridis_ramp() -> Vec<[u16; 3]> {
    (0..256u32)
        .map(|i| {
            let t = i as f32 / 255.0;
            [(t * 65535.0) as u16, ((1.0 - t) * 65535.0) as u16, 32768]
        })
        .collect()
}

fn full_spec() -> Spec {
    let main = pattern(320, 240, 0);
    let region = [40.0, 30.0, 64.0, 48.0];
    let (iw, ih) = inset_size((320, 240), region);
    Spec {
        title: Some("Mouse retina, DAPI & GFP".into()),
        main: Some(Panel {
            px: main,
            label: Some("Merge".into()),
        }),
        channels: vec![
            Panel {
                px: pattern(320, 240, 1),
                label: Some("DAPI".into()),
            },
            Panel {
                px: pattern(320, 240, 2),
                label: Some("GFP".into()),
            },
        ],
        inset: Some(InsetSpec {
            px: pattern(iw, ih, 3),
            region,
        }),
        // 0.5 µm per px.
        scale: Some(ScaleSpec {
            per_px: 0.5,
            unit: "µm".into(),
            metres: Some(1e-6),
            approximate: false,
        }),
        legend: Some(LegendSpec {
            ramp: viridis_ramp(),
            lo: 0.0,
            hi: 4095.0,
            unit: "counts".into(),
        }),
        marks: vec![Mark {
            path: vec![El::Move([50.0, 200.0]), El::Line([150.0, 200.0])],
            closed: false,
            label: Some("50.0 µm".into()),
        }],
        meta: vec![
            ("file".into(), "retina.ome.tif".into()),
            ("calibration".into(), "OME physical size".into()),
            ("display".into(), "min 0, max 4095, viridis".into()),
            ("software".into(), "Specular 0.1.0".into()),
        ],
        caption: vec![
            "retina.ome.tif".into(),
            "Calibration: OME physical size".into(),
        ],
        deep: true,
    }
}

#[test]
fn panels_land_on_whole_pixels_and_the_bar_is_calibrated() {
    let f = compose(&full_spec(), &mut Boxes).unwrap();
    assert_eq!(f.pictures.len(), 5, "3 panels, the inset, the colour bar");
    for p in &f.pictures[..4] {
        assert!(
            p.is_exact(),
            "{} at {},{} {}x{}",
            p.name,
            p.x,
            p.y,
            p.w,
            p.h
        );
    }
    // Panels in a row, the merge last of the three.
    assert!(f.pictures[0].x < f.pictures[1].x && f.pictures[1].x < f.pictures[2].x);
    assert_eq!(f.pictures[2].name, "Merge");
    let bar: Vec<_> = f.items_of(Role::ScaleBar).collect();
    assert_eq!(bar.len(), 1);
    let b = bounds(&bar[0].path).unwrap();
    // 20% of 320 px at 0.5 µm/px is 32 µm → a 20 µm bar of 40 px.
    assert_eq!(bar[0].data("length"), Some("20"));
    assert_eq!(bar[0].data("unit"), Some("µm"));
    assert!(((b[2] - b[0]) - 40.0).abs() < 1e-3, "{b:?}");
    // On the main panel, inside it.
    let m = &f.pictures[2];
    assert!(b[0] >= m.x && b[2] <= m.x + m.w && b[1] >= m.y && b[3] <= m.y + m.h);
    // The label says so.
    assert!(f.items.iter().any(|i| i.text.as_deref() == Some("20 µm")));
    // The inset's outline sits on the main panel at the region.
    let outlines: Vec<_> = f.items_of(Role::InsetOutline).collect();
    assert_eq!(outlines.len(), 2);
    let o = bounds(&outlines[0].path).unwrap();
    assert_eq!(o, [m.x + 40.0, m.y + 30.0, m.x + 104.0, m.y + 78.0]);
    // Ticks of the colour bar.
    let ticks: Vec<&str> = f
        .items_of(Role::Tick)
        .filter_map(|i| i.data("value"))
        .collect();
    assert_eq!(ticks, ["0", "1000", "2000", "3000", "4000"]);
    // Everything is on the page.
    for it in &f.items {
        if let Some(b) = bounds(&it.path) {
            assert!(b[0] >= 0.0 && b[1] >= 0.0, "{:?} {b:?}", it.role);
            assert!(
                b[2] <= f.width as f32 && b[3] <= f.height as f32,
                "{:?} {b:?}",
                it.role
            );
        }
    }
    assert_eq!(f.meta("calibration"), Some("OME physical size"));
}

#[test]
fn approximate_and_missing_calibrations() {
    let mut s = full_spec();
    s.scale.as_mut().unwrap().approximate = true;
    let f = compose(&s, &mut Boxes).unwrap();
    assert!(f.items.iter().any(|i| i.text.as_deref() == Some("≈ 20 µm")));
    s.scale = None;
    let f = compose(&s, &mut Boxes).unwrap();
    assert_eq!(f.items_of(Role::ScaleBar).count(), 0);
    s.main = None;
    assert!(compose(&s, &mut Boxes).is_none());
}

/// The numbers of an SVG path's `d` (x, y pairs).
fn svg_points(d: &str) -> Vec<[f32; 2]> {
    let nums: Vec<f32> = d
        .split(|c: char| c.is_ascii_alphabetic() || c == ' ')
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap())
        .collect();
    nums.chunks_exact(2).map(|c| [c[0], c[1]]).collect()
}

#[test]
fn svg_reads_back() {
    let f = compose(&full_spec(), &mut Boxes).unwrap();
    let svg = svg::write(&f).unwrap();
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let root = doc.root_element();
    assert_eq!(root.attribute("width"), Some(f.width.to_string().as_str()));
    // The scale bar: its drawn width is its calibrated length.
    let bar = doc
        .descendants()
        .find(|n| n.attribute("data-role") == Some("scale-bar"))
        .unwrap();
    let px: f32 = bar.attribute("data-length-px").unwrap().parse().unwrap();
    let pts = svg_points(bar.attribute("d").unwrap());
    let (x0, x1) = pts
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[0]), b.max(p[0])));
    assert!(((x1 - x0) - px).abs() < 2e-3, "{} vs {px}", x1 - x0);
    assert!((px - 20.0 / 0.5).abs() < 1e-3);
    assert_eq!(bar.attribute("data-length"), Some("20"));
    assert_eq!(bar.attribute("data-unit"), Some("µm"));
    // The legend's values.
    let ticks: Vec<&str> = doc
        .descendants()
        .filter(|n| n.attribute("data-role") == Some("tick"))
        .filter_map(|n| n.attribute("data-value"))
        .collect();
    assert_eq!(ticks, ["0", "1000", "2000", "3000", "4000"]);
    let cb = doc
        .descendants()
        .find(|n| n.attribute("data-role") == Some("colour-bar"))
        .unwrap();
    assert_eq!(cb.attribute("data-hi"), Some("4095"));
    assert_eq!(cb.attribute("data-unit"), Some("counts"));
    // Provenance.
    let fields: HashMap<&str, &str> = doc
        .descendants()
        .filter(|n| n.tag_name().name() == "field" && n.tag_name().namespace() == Some(svg::NS))
        .map(|n| (n.attribute("key").unwrap(), n.text().unwrap_or("")))
        .collect();
    assert_eq!(fields["file"], "retina.ome.tif");
    assert_eq!(fields["calibration"], "OME physical size");
    assert_eq!(fields["display"], "min 0, max 4095, viridis");
    // Text is labelled for screen readers.
    assert!(
        doc.descendants()
            .any(|n| n.attribute("aria-label") == Some("Mouse retina, DAPI & GFP"))
    );
    // Pictures: placed at their size, their samples as given, pixelated.
    let imgs: Vec<_> = doc
        .descendants()
        .filter(|n| n.tag_name().name() == "image")
        .collect();
    assert_eq!(imgs.len(), f.pictures.len());
    for (n, p) in imgs.iter().zip(&f.pictures) {
        assert_eq!(n.attribute("x").unwrap().parse::<f32>().unwrap(), p.x);
        assert_eq!(n.attribute("width").unwrap().parse::<f32>().unwrap(), p.w);
        let href = n.attribute("href").unwrap();
        let png = base64_decode(href.strip_prefix("data:image/png;base64,").unwrap());
        let img = specular_png::decode(&png, &specular_png::DecodeOptions::default()).unwrap();
        assert_eq!((img.width, img.height), (p.px.width, p.px.height));
        let rgba = img.to_rgba8();
        let want = p.px.to_rgb8();
        for (i, px) in rgba.chunks_exact(4).enumerate() {
            assert_eq!(&px[..3], &want[i * 3..i * 3 + 3], "{} pixel {i}", p.name);
        }
        if !p.smooth {
            assert_eq!(n.attribute("image-rendering"), Some("pixelated"));
        }
    }
}

fn base64_decode(s: &str) -> Vec<u8> {
    let v = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        _ => 63,
    };
    let b: Vec<u8> = s.bytes().filter(|&c| c != b'=').collect();
    let mut o = Vec::new();
    for c in b.chunks(4) {
        let n = c
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &x)| n | (u32::from(v(x)) << (18 - 6 * i)));
        o.push((n >> 16) as u8);
        if c.len() > 2 {
            o.push((n >> 8) as u8);
        }
        if c.len() > 3 {
            o.push(n as u8);
        }
    }
    o
}

#[test]
fn base64_round_trip() {
    for n in 0..20 {
        let d: Vec<u8> = (0..n).map(|i| (i * 37 + 5) as u8).collect();
        assert_eq!(base64_decode(&base64(&d)), d);
    }
    assert_eq!(base64(b"Man"), "TWFu");
    assert_eq!(base64(b"Ma"), "TWE=");
}

/// The items alone (no pictures, which resvg here cannot decode) through resvg and through
/// the raster writer: the same shapes, up to anti-aliasing.
#[test]
fn svg_vectors_render_like_the_raster() {
    let mut f = compose(&full_spec(), &mut Boxes).unwrap();
    f.pictures.clear();
    let svg = svg::write(&f).unwrap();
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).unwrap();
    let mut pm = resvg::tiny_skia::Pixmap::new(f.width, f.height).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pm.as_mut(),
    );
    let ours = raster::render(&f, 1.0);
    let theirs = pm.data();
    // Compared in 3 × 3 blocks: renderers place a hairline's ink differently between the two
    // columns it straddles, but not how much ink there is.
    let (bw, bh) = (f.width / 3, f.height / 3);
    let (mut far, mut inked) = (0usize, 0usize);
    for by in 0..bh {
        for bx in 0..bw {
            let (mut a, mut b) = ([0i32; 3], [0i32; 3]);
            for y in by * 3..by * 3 + 3 {
                for x in bx * 3..bx * 3 + 3 {
                    let i = (y * f.width + x) as usize;
                    for k in 0..3 {
                        a[k] += i32::from(to8(ours.data[i * 3 + k]));
                        b[k] += i32::from(theirs[i * 4 + k]);
                    }
                }
            }
            if a.iter().any(|&v| v < 9 * 240) {
                inked += 1;
            }
            if a.iter().zip(&b).any(|(x, y)| (x - y).abs() > 9 * 24) {
                far += 1;
            }
        }
    }
    eprintln!("svg vs raster: {far} of {inked} inked 3x3 blocks differ by more than 24 codes");
    assert!(inked > 500, "{inked}");
    assert!(far * 100 < inked, "{far} of {inked}");
}

#[test]
fn raster_copies_panels_sample_for_sample() {
    let f = compose(&full_spec(), &mut Boxes).unwrap();
    let r = raster::render(&f, 1.0);
    assert_eq!((r.width, r.height), (f.width, f.height));
    // The DAPI panel has nothing drawn over it.
    let p = &f.pictures[0];
    for y in 0..p.px.height {
        for x in 0..p.px.width {
            assert_eq!(r.px(p.x as u32 + x, p.y as u32 + y), p.px.px(x, y));
        }
    }
    // At twice the size, each sample is a 2 × 2 block.
    let r2 = raster::render(&f, 2.0);
    assert_eq!((r2.width, r2.height), (f.width * 2, f.height * 2));
    for (x, y) in [(0, 0), (17, 33), (319, 239)] {
        let want = p.px.px(x, y);
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            assert_eq!(
                r2.px((p.x as u32 + x) * 2 + dx, (p.y as u32 + y) * 2 + dy),
                want
            );
        }
    }
}

#[test]
fn pdf_reads_back() {
    let f = compose(&full_spec(), &mut Boxes).unwrap();
    let opts = pdf::Options {
        dpi: 144.0,
        producer: "Specular 0.1.0".into(),
        created: Some("D:20261024120000Z".into()),
        ..pdf::Options::default()
    };
    let bytes = pdf::write(&f, &opts).unwrap();
    let doc = specular_pdf::Document::open(std::sync::Arc::new(bytes.clone())).unwrap();
    assert_eq!(doc.page_count(), 1);
    let (pw, ph) = doc.page_size(0).unwrap();
    assert!((pw - f.width as f32 * 0.5).abs() < 0.01 && (ph - f.height as f32 * 0.5).abs() < 0.01);
    let info = doc.info();
    assert_eq!(info.title.as_deref(), Some("Mouse retina, DAPI & GFP"));
    assert_eq!(info.producer.as_deref(), Some("Specular 0.1.0"));
    let xmp = String::from_utf8(doc.xmp().unwrap()).unwrap();
    assert!(
        xmp.contains("<specular:file>retina.ome.tif</specular:file>"),
        "{xmp}"
    );
    assert!(xmp.contains("<specular:calibration>OME physical size</specular:calibration>"));
    // The information dictionary has the provenance too.
    let raw = String::from_utf8_lossy(&bytes);
    assert!(raw.contains("/Specular-display (min 0, max 4095, viridis)"));
    // The scale bar: its marked content names its length, and its path is that long.
    let c = pdf::content(&bytes).unwrap();
    let at = c.find("/scale-bar <<").unwrap();
    let dict = &c[at..c[at..].find(">> BDC").unwrap() + at];
    let px_at = dict.find("/length-px ").unwrap() + 11;
    let px: f32 = pdf::parse_string(
        &dict[px_at..]
            .split_whitespace()
            .next()
            .unwrap()
            .replace(">>", ""),
    )
    .unwrap()
    .parse()
    .unwrap();
    assert!((px - 40.0).abs() < 1e-3);
    assert!(dict.contains(&format!("/unit {}", pdf::string("µm"))));
    let body = &c[at..at + c[at..].find("EMC").unwrap()];
    let xs: Vec<f32> = body
        .lines()
        .filter(|l| l.ends_with(" m") || l.ends_with(" l"))
        .map(|l| l.split(' ').next().unwrap().parse().unwrap())
        .collect();
    let w =
        xs.iter().cloned().fold(f32::MIN, f32::max) - xs.iter().cloned().fold(f32::MAX, f32::min);
    assert!((w - px).abs() < 2e-3, "{w} vs {px}");
    // Legend values and text are there for readers.
    for v in ["0", "1000", "4000"] {
        assert!(c.contains(&format!("/tick <</value ({v})>> BDC")), "{v}");
    }
    assert!(c.contains(&format!("/ActualText {}", pdf::string("20 µm"))));
    // Rendered by hayro at one pixel per figure px: the panels are the pixels given (hayro
    // samples images at pixel centres, so 1:1 is exact up to its 8-bit output).
    let req = specular_pdf::RenderRequest {
        page: 0,
        scale_x: 2.0,
        scale_y: 2.0,
        x: 0,
        y: 0,
        width: f.width,
        height: f.height,
    };
    let rgba = doc.render(req, &|| false).unwrap();
    let p = &f.pictures[0];
    let (mut off, mut n) = (0usize, 0usize);
    for y in 2..p.px.height - 2 {
        for x in 2..p.px.width - 2 {
            let i = (((p.y as u32 + y) * f.width + p.x as u32 + x) * 4) as usize;
            let want = p.px.px(x, y);
            n += 1;
            if (0..3).any(|k| (i32::from(rgba[i + k]) - i32::from(to8(want[k]))).abs() > 1) {
                off += 1;
            }
        }
    }
    eprintln!("pdf via hayro: {off} of {n} panel px off by more than one code");
    assert_eq!(off, 0);
    // The whole page, vectors and all, against the raster writer in 3 × 3 blocks.
    let ours = raster::render(&f, 1.0);
    let (mut far, mut blocks) = (0usize, 0usize);
    for by in 0..f.height / 3 {
        for bx in 0..f.width / 3 {
            let (mut a, mut b) = ([0i32; 3], [0i32; 3]);
            for y in by * 3..by * 3 + 3 {
                for x in bx * 3..bx * 3 + 3 {
                    let i = (y * f.width + x) as usize;
                    for k in 0..3 {
                        a[k] += i32::from(to8(ours.data[i * 3 + k]));
                        b[k] += i32::from(rgba[i * 4 + k]);
                    }
                }
            }
            blocks += 1;
            if a.iter().zip(&b).any(|(x, y)| (x - y).abs() > 9 * 24) {
                far += 1;
            }
        }
    }
    eprintln!("pdf via hayro: {far} of {blocks} 3x3 blocks differ by more than 24 codes");
    assert!(far * 200 < blocks, "{far} of {blocks}");
}

#[test]
fn pdf_strings() {
    assert_eq!(pdf::string("abc (x)"), "(abc \\(x\\))");
    assert_eq!(pdf::string("µm"), "<FEFF00B5006D>");
    for s in ["µm", "שלום", "a\\b(c)"] {
        assert_eq!(pdf::parse_string(&pdf::string(s)).as_deref(), Some(s));
    }
}

fn slides(n: usize) -> pptx::Deck {
    let png = crate::png(&pattern(64, 48, 9), false, &[]).unwrap();
    pptx::Deck {
        title: "Key images".into(),
        dark: true,
        slides: (0..n)
            .map(|i| pptx::Slide {
                title: format!("CT CHEST · series 3 · image {}", i + 1),
                png: png.clone(),
                size: (64, 48),
                alt: "Axial CT".into(),
                notes: vec![format!("Length {}.0 mm", 10 + i), "HU mean 42".into()],
            })
            .collect(),
    }
}

#[test]
fn pptx_opens_as_a_package() {
    let bytes = pptx::write(&slides(3)).unwrap();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let names: Vec<String> = z.file_names().flatten().map(String::from).collect();
    let mut parts = HashMap::new();
    for n in &names {
        let mut f = z.by_name(n).unwrap();
        let mut v = Vec::new();
        f.read_to_end(&mut v).unwrap();
        parts.insert(n.clone(), v);
    }
    let xml = |n: &str| String::from_utf8(parts[n].clone()).unwrap();
    // Every XML part parses, and every relationship points at a part that exists.
    for n in &names {
        if n.ends_with(".xml") || n.ends_with(".rels") {
            let s = xml(n);
            let doc = roxmltree::Document::parse(&s).unwrap_or_else(|e| panic!("{n}: {e}"));
            if n.ends_with(".rels") {
                let dir = n.trim_end_matches(|c| c != '/').trim_end_matches("_rels/");
                for r in doc.descendants().filter(|e| e.has_tag_name("Relationship")) {
                    let t = r.attribute("Target").unwrap();
                    let mut path: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
                    for seg in t.split('/') {
                        match seg {
                            ".." => {
                                path.pop();
                            }
                            s => path.push(s),
                        }
                    }
                    let full = path.join("/");
                    assert!(parts.contains_key(&full), "{n} → {t} ({full})");
                }
            }
        }
    }
    // Content types list every slide and notes page.
    let ct = xml("[Content_Types].xml");
    // Three slides, each with its picture and its notes.
    let slides: Vec<_> = names
        .iter()
        .filter(|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml"))
        .collect();
    assert_eq!(slides.len(), 3);
    let pres = xml("ppt/presentation.xml");
    assert_eq!(pres.matches("<p:sldId ").count(), 3);
    for i in 1..=3 {
        assert!(ct.contains(&format!("/ppt/slides/slide{i}.xml")));
        assert!(ct.contains(&format!("/ppt/notesSlides/notesSlide{i}.xml")));
        assert!(parts[&format!("ppt/media/image{i}.png")].starts_with(b"\x89PNG"));
        let s = xml(&format!("ppt/slides/slide{i}.xml"));
        assert!(s.contains(&format!("image {i}</a:t>")));
        assert!(s.contains("r:embed=\"rId2\""));
        let notes = xml(&format!("ppt/notesSlides/notesSlide{i}.xml"));
        let doc = roxmltree::Document::parse(&notes).unwrap();
        let texts: Vec<&str> = doc
            .descendants()
            .filter(|n| n.has_tag_name("t"))
            .filter_map(|n| n.text())
            .collect();
        assert_eq!(
            texts,
            [format!("Length {}.0 mm", 9 + i).as_str(), "HU mean 42"]
        );
    }
    // An empty deck is still a package.
    assert!(pptx::write(&pptx::Deck::default()).is_ok());
}

#[test]
fn pictures_fit_the_slide() {
    for size in [(64, 48), (48, 640), (4000, 10)] {
        let (x, y, cx, cy) = pptx::picture_box(size);
        assert!(x >= 0 && y >= 0 && x + cx <= pptx::SLIDE.0 && y + cy <= pptx::SLIDE.1);
        let ratio = cx as f64 / cy as f64;
        let want = f64::from(size.0) / f64::from(size.1);
        assert!((ratio / want - 1.0).abs() < 0.01, "{size:?}");
    }
}

/// Text shaped by the viewer's own fonts lands inside the figure, in any script.
#[test]
fn viewer_text_as_outlines() {
    let mut text = specular_gfx::Text::bundled_only();
    let mut s = full_spec();
    s.title = Some("רשתית עכבר, 20 מיקרומטר".into());
    let f = compose(&s, &mut text).unwrap();
    let title = f
        .items
        .iter()
        .find(|i| i.text.as_deref() == s.title.as_deref())
        .unwrap();
    assert!(title.path.len() > 50);
    let b = bounds(&title.path).unwrap();
    assert!(
        b[0] >= 0.0 && b[2] <= f.width as f32 && b[1] >= 0.0,
        "{b:?}"
    );
    let svg = svg::write(&f).unwrap();
    roxmltree::Document::parse(&svg).unwrap();
}

/// The display path read back: an 8-bit image drawn at 1:1 through the viewer's renderer
/// comes back with its own codes; a region comes back as that region.
#[test]
fn gpu_readback_keeps_the_codes() {
    use specular_core::{Orientation, PixelLayout, Raster, RasterPlane};
    let Some(h) = specular_shots::Harness::new() else {
        return;
    };
    let gpu = &h.gpu;
    let (w, hh) = (97u32, 61u32);
    let src = pattern(w, hh, 7).to_rgb8();
    let planes = (0..3)
        .map(|c| RasterPlane {
            width: w,
            height: hh,
            stride: w as usize,
            data: src.chunks_exact(3).map(|p| p[c]).collect(),
            h_div: 1,
            v_div: 1,
        })
        .collect();
    let raster = Raster {
        width: w,
        height: hh,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Rgb,
        planes,
        truncated: false,
    };
    let mut r = specular_render::ImageRenderer::new(gpu, 64 << 20);
    let mut img = r.create_image(gpu, 1, w, hh, None);
    r.upload_overview(gpu, &mut img, &raster);
    let opts = specular_render::RenderOptions {
        nearest_from: 1.0,
        ..Default::default()
    };
    let shot = render::Shot::region((w, hh), Orientation::NORMAL, [0, 0, w, hh]);
    let out = render::read(gpu, &mut r, &img, &shot, &opts).unwrap();
    assert_eq!(out.to_rgb8(), src, "whole image");
    // A region at an odd offset.
    let shot = render::Shot::region((w, hh), Orientation::NORMAL, [13, 7, 13 + 40, 7 + 30]);
    let out = render::read(gpu, &mut r, &img, &shot, &opts).unwrap();
    for y in 0..30 {
        for x in 0..40 {
            let i = (((7 + y) * w + 13 + x) * 3) as usize;
            assert_eq!(
                &out.to_rgb8()[((y * 40 + x) * 3) as usize..][..3],
                &src[i..i + 3],
                "{x},{y}"
            );
        }
    }
}

/// A figure read back on a device the driver lost returns an error, never panics.
#[test]
fn a_lost_device_fails_the_readback() {
    use specular_core::{Orientation, PixelLayout, Raster, RasterPlane};
    let gpu = match specular_gfx::Gpu::new() {
        Ok(g) => g,
        Err(e) if std::env::var_os("SPECULAR_REQUIRE_GPU").is_some() => panic!("{e}"),
        Err(_) => return,
    };
    let (w, h) = (32u32, 16u32);
    let raster = Raster {
        width: w,
        height: h,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Gray,
        planes: vec![RasterPlane {
            width: w,
            height: h,
            stride: w as usize,
            data: vec![128; (w * h) as usize],
            h_div: 1,
            v_div: 1,
        }],
        truncated: false,
    };
    let mut r = specular_render::ImageRenderer::new(&gpu, 16 << 20);
    let mut img = r.create_image(&gpu, 1, w, h, None);
    r.upload_overview(&gpu, &mut img, &raster);
    let shot = render::Shot::region((w, h), Orientation::NORMAL, [0, 0, w, h]);
    let opts = specular_render::RenderOptions::default();
    assert!(render::read(&gpu, &mut r, &img, &shot, &opts).is_ok());
    gpu.device.destroy();
    let e = render::read(&gpu, &mut r, &img, &shot, &opts).unwrap_err();
    assert!(e.contains("GPU readback"), "{e}");
}
