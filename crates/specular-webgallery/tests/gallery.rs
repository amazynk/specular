//! A gallery made end to end on the GPU (lavapipe in CI): the folder's files, the page parses,
//! nothing in it points outside the folder, GPS is gone from the photos unless kept, the
//! watermark is drawn, and the client's picks come back to the right photos.
//!
//!     SPECULAR_REQUIRE_GPU=1 cargo test -p specular-webgallery --test gallery

use std::path::{Path, PathBuf};

use specular_convert::encode::{self, Meta};
use specular_convert::exif::{Entry, Exif};
use specular_convert::pixels::{self, OutTransfer, Pixels, Samples, Target, ToLinear};
use specular_core::{CancelToken, Orientation};
use specular_develop::export::batch::{self, Job};
use specular_develop::gpu::pool::Budget;
use specular_develop::model::{Edit, Registry, SourceKey};
use specular_metadata::Localizer;
use specular_webgallery::build::{self, Built};
use specular_webgallery::{Captions, ImageFormat, Options, Theme, picks, site};

const XMP: &str = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:exif="http://ns.adobe.com/exif/1.0/" exif:GPSLatitude="48,51.5N" exif:GPSLongitude="2,21.0E"><dc:title><rdf:Alt><rdf:li xml:lang="x-default">Dunes &amp; "dawn" &lt;5 am&gt;</rdf:li></rdf:Alt></dc:title><dc:description><rdf:Alt><rdf:li xml:lang="x-default">Erg Chebbi</rdf:li></rdf:Alt></dc:description></rdf:Description></rdf:RDF></x:xmpmeta>"#;

fn rationals(tag: u16, v: &[(u32, u32)]) -> Entry {
    Entry {
        tag,
        ty: 5,
        count: v.len() as u32,
        data: v
            .iter()
            .flat_map(|(n, d)| n.to_le_bytes().into_iter().chain(d.to_le_bytes()))
            .collect(),
    }
}

/// A smooth `w × h` JPEG with a camera, a capture date, an exposure, GPS and (with `xmp`) a
/// title, a caption and XMP GPS.
fn source_jpeg(path: &Path, w: u32, h: u32, xmp: bool) {
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            rgb.extend_from_slice(&[
                (40 + 160 * x / w) as u8,
                (60 + 120 * y / h) as u8,
                (180 - 100 * x / w) as u8,
            ]);
        }
    }
    let exif = Exif {
        ifd0: vec![
            Entry::ascii(0x010F, "TestMaker"),
            Entry::ascii(0x0110, "TestCam 9"),
        ],
        exif: vec![
            Entry::rational(0x829A, 1, 250),
            Entry::rational(0x829D, 28, 10),
            Entry::short(0x8827, &[200]),
            Entry::ascii(0x9003, "2026:05:01 10:20:30"),
            Entry::rational(0x920A, 35, 1),
        ],
        gps: vec![
            Entry::ascii(0x0001, "N"),
            rationals(0x0002, &[(48, 1), (51, 1), (30, 1)]),
            Entry::ascii(0x0003, "E"),
            rationals(0x0004, &[(2, 1), (21, 1), (0, 1)]),
        ],
    };
    let px = Pixels {
        width: w,
        height: h,
        channels: 3,
        samples: Samples::Eight(rgb),
    };
    let bytes = encode::encode(
        &specular_convert::Options {
            jpeg_quality: 95,
            ..specular_convert::Options::default()
        },
        &px,
        &Meta {
            exif: Some(&exif),
            xmp: xmp.then_some(XMP.as_bytes()),
            icc: None,
            cicp: None,
        },
    )
    .expect("encode the source");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

struct Dev {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

fn dev() -> Option<Dev> {
    match batch::open_device() {
        Ok((device, queue, _)) => Some(Dev { device, queue }),
        Err(e) if std::env::var_os("SPECULAR_REQUIRE_GPU").is_none() => {
            eprintln!("no GPU ({e}): skipped");
            None
        }
        Err(e) => panic!("SPECULAR_REQUIRE_GPU is set but there is no adapter: {e}"),
    }
}

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("webgallery-{name}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn jobs(paths: &[PathBuf]) -> Vec<Job> {
    paths
        .iter()
        .map(|p| Job {
            source: p.clone(),
            edit: Edit::default_for(Registry::builtin(), SourceKey::default(), false),
            orient: None,
        })
        .collect()
}

fn run(d: &Dev, paths: &[PathBuf], opts: &Options, out: &Path, cancel: &CancelToken) -> Built {
    let reg = specular_decode::Registry::new();
    build::build(
        &d.device,
        &d.queue,
        Budget::fixed(1 << 30),
        jobs(paths),
        opts,
        &reg,
        out,
        &|n| format!("{n} photos"),
        &Localizer::english(),
        cancel,
        &|_| {},
    )
    .expect("build")
}

/// Every file under `dir`, relative, sorted.
fn files(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, d: &Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.push(
                    p.strip_prefix(base)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    let mut v = Vec::new();
    walk(dir, dir, &mut v);
    v.sort();
    v
}

/// An image file decoded to 8-bit RGB.
fn rgb(path: &Path) -> (u32, u32, Vec<u8>) {
    let s = specular_decode::Registry::new().open_path(path).unwrap();
    let r = s
        .decode(
            &specular_decode::DecodeRequest::full(1),
            &CancelToken::new(),
        )
        .unwrap();
    let t = Target {
        width: r.width,
        height: r.height,
        gray: false,
        alpha: false,
        sixteen: false,
        transfer: OutTransfer::Srgb,
        tone_map: false,
    };
    let px = pixels::render(
        &r,
        &ToLinear::Srgb,
        &t,
        Orientation::NORMAL,
        &CancelToken::new(),
    )
    .ok()
    .unwrap();
    let Samples::Eight(v) = px.samples else {
        panic!("8-bit")
    };
    (px.width, px.height, v)
}

/// Mean absolute difference of two RGB images over a box (fractions of the size).
fn box_diff(a: &(u32, u32, Vec<u8>), b: &(u32, u32, Vec<u8>), bx: [f32; 4]) -> f64 {
    assert_eq!((a.0, a.1), (b.0, b.1));
    let (w, h) = (a.0 as usize, a.1 as usize);
    let (x0, y0) = ((bx[0] * w as f32) as usize, (bx[1] * h as f32) as usize);
    let (x1, y1) = ((bx[2] * w as f32) as usize, (bx[3] * h as f32) as usize);
    let (mut sum, mut n) = (0f64, 0f64);
    for y in y0..y1 {
        for x in x0..x1 {
            for c in 0..3 {
                let i = (y * w + x) * 3 + c;
                sum += (f64::from(a.2[i]) - f64::from(b.2[i])).abs();
                n += 1.0;
            }
        }
    }
    sum / n
}

/// Every `src`/`href` in the page is a relative path to a file in the folder, and no text
/// file names a remote address or a way to reach one.
fn assert_self_contained(dir: &Path, doc: &roxmltree::Document) {
    for n in doc.descendants().filter(|n| n.is_element()) {
        for a in n.attributes() {
            let name = a.name();
            let v = a.value();
            // The script's data attributes hold the photo paths too.
            if matches!(name, "src" | "href" | "data-src") {
                assert!(
                    !v.contains(':') && !v.starts_with('/') && !v.starts_with("//"),
                    "{name}={v} is not a relative path"
                );
                let p = dir.join(v.replace("%20", " "));
                assert!(p.is_file(), "{name}={v}: no such file in the gallery");
            }
            assert!(
                !v.contains("http:") && !v.contains("https:"),
                "{name}={v} points outside"
            );
        }
    }
    for f in files(dir) {
        if !(f.ends_with(".html") || f.ends_with(".css") || f.ends_with(".js")) {
            continue;
        }
        let text = std::fs::read_to_string(dir.join(&f)).unwrap();
        for bad in [
            "http:",
            "https:",
            "//cdn",
            "@import",
            "url(",
            "fetch(",
            "XMLHttpRequest",
            "WebSocket",
            "sendBeacon",
            "EventSource",
            "@font-face",
            "<iframe",
        ] {
            assert!(!text.contains(bad), "{f} contains {bad:?}");
        }
    }
}

#[test]
fn gallery_folder_page_metadata_watermark_and_picks() {
    let Some(d) = dev() else { return };
    let root = scratch("full");
    let src = root.join("src");
    // Two photos share a name in different folders; the third has a title and caption.
    let paths = vec![
        src.join("a").join("IMG_1.jpg"),
        src.join("b").join("IMG_1.jpg"),
        src.join("a").join("IMG_2.jpg"),
    ];
    source_jpeg(&paths[0], 1200, 800, false);
    source_jpeg(&paths[1], 800, 1200, false);
    source_jpeg(&paths[2], 1200, 800, true);
    let out = root.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let opts = Options {
        title: "Smith & Co <proofs>".into(),
        theme: Theme::Dark,
        long_edge: 1000,
        format: ImageFormat::Jpeg,
        captions: Captions::All,
        exif: true,
        keep_location: false,
        watermark: "PROOF".into(),
        ..Options::default()
    };
    let t0 = std::time::Instant::now();
    let b = run(&d, &paths, &opts, &out, &CancelToken::new());
    eprintln!("3 photos in {:.2} s", t0.elapsed().as_secs_f64());
    assert!(b.failed.is_empty(), "{:?}", b.failed);
    assert_eq!(b.written, 3);
    assert!(!b.cancelled);
    assert_eq!(b.dir, out.join("Smith___Co__proofs"));

    // The folder: the page, its two assets, each photo and thumbnail, nothing else.
    assert_eq!(
        files(&b.dir),
        [
            "assets/gallery.css",
            "assets/gallery.js",
            "images/IMG_1-2.jpg",
            "images/IMG_1.jpg",
            "images/IMG_2.jpg",
            "index.html",
            "thumbs/IMG_1-2.jpg",
            "thumbs/IMG_1.jpg",
            "thumbs/IMG_2.jpg",
        ]
    );
    for (f, size) in [
        ("images/IMG_1.jpg", (1000, 667)),
        ("images/IMG_1-2.jpg", (667, 1000)),
        ("thumbs/IMG_1.jpg", (600, 400)),
        ("thumbs/IMG_1-2.jpg", (400, 600)),
    ] {
        let (w, h, _) = rgb(&b.dir.join(f));
        assert!(
            w.abs_diff(size.0) <= 1 && h.abs_diff(size.1) <= 1,
            "{f}: {w}×{h}"
        );
    }

    // GPS is gone from the photos (Exif and XMP); the rest of the Exif stays.
    for f in ["images/IMG_1.jpg", "images/IMG_2.jpg"] {
        let bytes = std::fs::read(b.dir.join(f)).unwrap();
        let exif = specular_decode::exif_tiff(&bytes).expect("Exif kept");
        assert!(specular_decode::exif_position(&exif).is_none(), "{f}: GPS");
        assert_eq!(
            specular_decode::exif_date_taken(&exif).as_deref(),
            Some("2026:05:01 10:20:30")
        );
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            !text.contains("GPSLatitude") && !text.contains("48,51.5N"),
            "{f}"
        );
        // Thumbnails carry nothing.
        let thumb = std::fs::read(b.dir.join(f.replace("images", "thumbs"))).unwrap();
        assert!(specular_decode::exif_tiff(&thumb).is_none(), "{f} thumb");
    }

    // The page parses (polyglot XHTML), lists the three photos with their facts, and points
    // at nothing outside the folder.
    let html = std::fs::read_to_string(b.dir.join("index.html")).unwrap();
    let doc = roxmltree::Document::parse_with_options(
        &html,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )
    .expect("index.html is well-formed");
    assert_eq!(doc.root_element().attribute("lang"), Some("en"));
    let title = doc
        .descendants()
        .find(|n| n.has_tag_name("title"))
        .and_then(|n| n.text());
    assert_eq!(title, Some("Smith & Co <proofs>"));
    let tiles: Vec<_> = doc
        .descendants()
        .filter(|n| n.attribute("class") == Some("tile"))
        .collect();
    assert_eq!(tiles.len(), 3);
    let names: Vec<_> = tiles
        .iter()
        .filter_map(|t| t.attribute("data-name"))
        .collect();
    // The shared name is told apart by its folder (the photo names stay as they are).
    assert_eq!(names, ["a/IMG_1.jpg", "b/IMG_1.jpg", "IMG_2.jpg"]);
    let t2 = tiles[2];
    assert_eq!(
        t2.attribute("data-label"),
        Some("IMG_2.jpg — Dunes & \"dawn\" <5 am> — Erg Chebbi")
    );
    let exif = t2.attribute("data-exif").unwrap();
    assert!(
        exif.contains("TestCam 9") && exif.contains("f/2.8"),
        "{exif}"
    );
    assert!(!exif.contains("48"), "{exif}");
    let count = doc
        .descendants()
        .find(|n| n.attribute("class") == Some("count"))
        .and_then(|n| n.text());
    assert_eq!(count, Some("3 photos"));
    assert_self_contained(&b.dir, &doc);

    // The client picks two photos (one by its folder, as the page names it); the page's
    // files come back. A bare IMG_1.jpg names photos in two folders: it is not applied.
    let id = doc
        .root_element()
        .descendants()
        .find(|n| n.has_tag_name("body"));
    let id = id.and_then(|n| n.attribute("data-gallery")).unwrap();
    assert_eq!(id.len(), 16);
    assert!(site::JS.contains(r##""# Specular client picks\n# gallery: ""##));
    assert!(site::JS.contains(r#"format: "specular-picks""#));
    let txt = format!(
        "# Specular client picks\n# gallery: {}\n# id: {id}\nIMG_2.jpg\nb/IMG_1.jpg\nIMG_1.jpg\n",
        opts.title
    );
    let json = format!(
        r#"{{"format":"specular-picks","version":1,"gallery":"{}","id":"{id}","picks":["IMG_2.jpg","b/IMG_1.jpg","IMG_1.jpg"]}}"#,
        opts.title.replace('"', "\\\"")
    );
    for text in [txt, json] {
        let p = picks::parse(&text);
        assert_eq!(p.gallery.as_deref(), Some(opts.title.as_str()));
        let m = picks::match_photos(&p, &paths);
        assert_eq!(m.photos(), [paths[2].clone(), paths[1].clone()]);
        assert_eq!(m.ambiguous, ["IMG_1.jpg"]);
        assert!(m.missing.is_empty());
    }

    // Without the watermark, location kept, as WebP: GPS stays, and the photos differ only
    // where the watermark was (bottom right).
    let opts2 = Options {
        title: "Kept".into(),
        format: ImageFormat::Webp,
        keep_location: true,
        watermark: String::new(),
        ..opts.clone()
    };
    let b2 = run(&d, &paths[2..], &opts2, &out, &CancelToken::new());
    assert_eq!(b2.written, 1);
    let webp = std::fs::read(b2.dir.join("images/IMG_2.webp")).unwrap();
    let exif = specular_decode::exif_tiff(&webp).expect("Exif kept");
    let (lat, lon) = specular_decode::exif_position(&exif).expect("GPS kept");
    assert!(
        (lat - 48.858).abs() < 0.01 && (lon - 2.35).abs() < 0.01,
        "{lat} {lon}"
    );
    let marked = rgb(&b.dir.join("images/IMG_2.jpg"));
    let plain = rgb(&b2.dir.join("images/IMG_2.webp"));
    let corner = box_diff(&marked, &plain, [0.70, 0.85, 0.98, 0.98]);
    let away = box_diff(&marked, &plain, [0.02, 0.02, 0.4, 0.4]);
    eprintln!("watermark: {corner:.2} in the corner, {away:.2} away from it");
    assert!(corner > 4.0 * away.max(0.5), "{corner} vs {away}");

    // A gallery of the same title goes in a folder of its own.
    assert_eq!(
        build::gallery_dir(&out, &opts.title),
        out.join("Smith___Co__proofs-2")
    );
    // WEBGALLERY_KEEP=1 keeps the galleries to look at in a browser.
    if std::env::var_os("WEBGALLERY_KEEP").is_none() {
        let _ = std::fs::remove_dir_all(&root);
    }
}

#[test]
fn stopped_or_failed_runs_leave_nothing() {
    let Some(d) = dev() else { return };
    let root = scratch("stop");
    let p = root.join("IMG_9.jpg");
    source_jpeg(&p, 320, 200, false);
    let cancel = CancelToken::new();
    cancel.cancel();
    let b = run(
        &d,
        std::slice::from_ref(&p),
        &Options::default(),
        &root,
        &cancel,
    );
    assert!(b.cancelled);
    assert!(!b.dir.exists());
    // A file that does not decode: nothing written, the folder goes.
    let bad = root.join("bad.jpg");
    std::fs::write(&bad, b"not a jpeg").unwrap();
    let b = run(&d, &[bad], &Options::default(), &root, &CancelToken::new());
    assert_eq!(b.written, 0);
    assert_eq!(b.failed.len(), 1);
    assert!(!b.dir.exists());
    let _ = std::fs::remove_dir_all(&root);
}

/// The page's script in Node.js (skipped when it is not installed) against a stand-in DOM
/// (`tests/fake_dom.js`): two hearts clicked, picks.txt and picks.json downloaded and read
/// back by the picks reader, the picks still there after a reload, and the lightbox's keys.
#[test]
fn script_picks_round_trip() {
    let dir = scratch("js");
    let js = dir.join("gallery.js");
    std::fs::write(&js, site::JS).unwrap();
    let case = dir.join("case.json");
    std::fs::write(
        &case,
        r#"{"id":"0123456789abcdef","title":"Smith \"proofs\"","rtl":false,
            "names":["a/IMG_1.jpg","b/IMG_1.jpg","IMG_2.jpg"],"pick":[0,2]}"#,
    )
    .unwrap();
    let harness = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fake_dom.js");
    let out = match std::process::Command::new("node")
        .arg(harness)
        .arg(&js)
        .arg(&case)
        .env("HOME", &dir)
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            eprintln!("no node ({e}): skipped");
            return;
        }
    };
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("the harness's JSON");
    assert_eq!(v["count"], "Picked: 2");
    for k in ["txt", "json"] {
        let p = picks::parse(v[k].as_str().expect(k));
        assert_eq!(p.names, ["a/IMG_1.jpg", "IMG_2.jpg"], "{k}");
        assert_eq!(p.gallery.as_deref(), Some("Smith \"proofs\""), "{k}");
    }
    let json: serde_json::Value = serde_json::from_str(v["json"].as_str().unwrap()).unwrap();
    assert_eq!(json["format"], "specular-picks");
    assert_eq!(json["id"], "0123456789abcdef");
    // The storage after the lightbox picked the second photo too, in the gallery's order.
    assert_eq!(v["stored"], r#"["a/IMG_1.jpg","b/IMG_1.jpg","IMG_2.jpg"]"#);
    assert_eq!(v["pressed"], serde_json::json!([true, false, true]));
    assert_eq!(v["opened"], true);
    assert_eq!(v["second"], "2 of 3");
    assert_eq!(v["wrapped"], "3 of 3");
    assert_eq!(v["closed"], true);
    assert_eq!(v["after"], "Picked: 3");
    let _ = std::fs::remove_dir_all(&dir);
}
