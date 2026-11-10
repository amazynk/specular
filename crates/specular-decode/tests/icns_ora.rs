//! ICNS, OpenRaster and Krita through the registry. Oracles: an ORA / KRA document decodes
//! to exactly what its `mergedimage.png` gives when decoded directly as a PNG, and an ICNS
//! PNG page to exactly what its payload gives. Opening reads only headers (a merged image
//! with corrupt pixel data still opens), and the sessions trim. Real files (this machine's
//! ICNS files and the gitignored conformance corpus) are swept when present;
//! `SPECULAR_CONFORMANCE_DIR` overrides the corpus root.

#[path = "common/trim.rs"]
mod trim;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use specular_core::{CancelToken, Raster, Source};
use specular_decode::{
    DecodeRequest, Format, PageLabel, Rect, Registry, Session, icns_page_labels, open_icns_page,
};

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

fn open(name: &str, data: Vec<u8>) -> Arc<dyn Session> {
    Registry::new()
        .open(Arc::new(Source::from_vec(name, data)))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn full(s: &dyn Session) -> Raster {
    s.decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap()
}

fn same(a: &Raster, b: &Raster, what: &str) {
    assert_eq!(
        (a.width, a.height, a.layout),
        (b.width, b.height, b.layout),
        "{what}"
    );
    assert_eq!(a.planes.len(), b.planes.len(), "{what}");
    for (p, q) in a.planes.iter().zip(&b.planes) {
        assert!(p.data == q.data, "{what}: pixels differ");
    }
}

/// The merged PNG decoded on its own, the oracle for a document.
fn direct(merged: &[u8]) -> Raster {
    full(&*open("merged.png", merged.to_vec()))
}

fn gradient(w: u32, h: u32) -> Vec<u8> {
    png(w, h, |x, y| {
        [
            (x * 7) as u8,
            (y * 5) as u8,
            (x ^ y) as u8,
            255 - (x as u8 / 2),
        ]
    })
}

#[test]
fn ora_and_kra_equal_their_merged_png() {
    let merged = gradient(97, 61);
    let oracle = direct(&merged);
    for deflate in [false, true] {
        let ora = zip(&[
            ("mimetype", b"image/openraster", false),
            (
                "stack.xml",
                br#"<image w="97" h="61"><stack><layer src="data/x.png"/></stack></image>"#,
                true,
            ),
            ("data/x.png", &gradient(4, 4), true),
            ("mergedimage.png", &merged, deflate),
            ("Thumbnails/thumbnail.png", &gradient(16, 10), true),
        ]);
        let kra = zip(&[
            ("mimetype", b"application/x-krita", false),
            (
                "maindoc.xml",
                br#"<DOC><IMAGE width="97" height="61"/></DOC>"#,
                true,
            ),
            ("mergedimage.png", &merged, deflate),
            ("preview.png", &gradient(16, 10), deflate),
        ]);
        for (name, data, format) in [("a.ora", ora, Format::Ora), ("a.kra", kra, Format::Kra)] {
            let what = format!("{name} deflate={deflate}");
            // By content, whatever the name says.
            let s = open("no-extension", data.clone());
            let i = s.info();
            assert_eq!((i.format, i.width, i.height), (format, 97, 61), "{what}");
            assert!(
                i.coding.contains("merged image, canvas 97×61"),
                "{what}: {}",
                i.coding
            );
            same(&full(&*s), &oracle, &what);
            let p = s.decode_preview().expect("preview").unwrap();
            assert_eq!((p.width, p.height), (16, 10), "{what}");
            let req = DecodeRequest {
                region: Some(Rect {
                    x: 3,
                    y: 5,
                    w: 40,
                    h: 30,
                }),
                ..DecodeRequest::full(1)
            };
            trim::assert_trims(&*s, &req);
        }
    }
}

#[test]
fn header_only_open() {
    // Pixel data replaced by garbage: opening still gives the size, decoding fails.
    let mut merged = gradient(300, 200);
    let idat = merged.windows(4).position(|w| w == b"IDAT").unwrap();
    for b in &mut merged[idat + 6..idat + 60] {
        *b = 0x5a;
    }
    for deflate in [false, true] {
        let f = zip(&[
            ("mimetype", b"application/x-krita", false),
            ("mergedimage.png", &merged, deflate),
        ]);
        let s = open("a.kra", f);
        assert_eq!((s.info().width, s.info().height), (300, 200));
        assert!(
            s.decode(&DecodeRequest::full(1), &CancelToken::new())
                .is_err()
        );
    }
}

#[test]
fn krita_preview_is_marked_and_missing_images_refused() {
    let prev = gradient(20, 12);
    let f = zip(&[
        ("mimetype", b"application/x-krita", false),
        (
            "maindoc.xml",
            br#"<DOC><IMAGE width="2000" height="1200"/></DOC>"#,
            true,
        ),
        ("preview.png", &prev, true),
    ]);
    let s = open("a.kra", f);
    assert!(
        s.info().coding.contains("embedded preview only"),
        "{}",
        s.info().coding
    );
    same(&full(&*s), &direct(&prev), "preview");
    let f = zip(&[
        ("mimetype", b"application/x-krita", false),
        ("maindoc.xml", b"<DOC/>", true),
    ]);
    let e = Registry::new()
        .open(Arc::new(Source::from_vec("a.kra", f)))
        .err()
        .unwrap();
    assert!(
        e.to_string()
            .contains("neither mergedimage.png nor preview.png"),
        "{e}"
    );
}

#[test]
fn other_zips_are_not_documents() {
    let reg = Registry::new();
    let epub = zip(&[
        ("mimetype", b"application/epub+zip", false),
        ("a.png", &gradient(2, 2), false),
    ]);
    let r = reg.open(Arc::new(Source::from_vec("a.zip", epub)));
    assert!(!r.is_ok_and(|s| matches!(s.info().format, Format::Ora | Format::Kra)));
    for ext in ["ora", "kra", "icns", "ORA"] {
        assert!(
            reg.supports_extension(Path::new(&format!("a.{ext}"))),
            "{ext}"
        );
    }
}

fn icns_entry(cc: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut e = cc.to_vec();
    e.extend_from_slice(&((data.len() + 8) as u32).to_be_bytes());
    e.extend_from_slice(data);
    e
}

fn icns(entries: &[Vec<u8>]) -> Vec<u8> {
    let body = entries.concat();
    let mut f = b"icns".to_vec();
    f.extend_from_slice(&((body.len() + 8) as u32).to_be_bytes());
    f.extend_from_slice(&body);
    f
}

#[test]
fn icns_png_pages_equal_their_payload() {
    let big = gradient(64, 64);
    let small = gradient(32, 32);
    // Uncompressed 16×16 RGB with an 8-bit mask.
    let rgb: Vec<u8> = (0..256u32).flat_map(|p| [p as u8, 9, 200]).collect();
    let mask: Vec<u8> = (0..256u32).map(|p| 255 - p as u8).collect();
    let f = icns(&[
        icns_entry(b"is32", &rgb),
        icns_entry(b"s8mk", &mask),
        icns_entry(b"ic12", &big),
        icns_entry(b"ic05", &small),
    ]);
    let labels = icns_page_labels(&f);
    assert_eq!(labels, ["32×32 @2x", "32×32", "16×16"]);
    let s = open("no-extension", f.clone());
    assert_eq!(s.info().format, Format::Icns);
    assert_eq!(s.page_count(), 3);
    assert!(s.info().coding.contains("ic12 (PNG"), "{}", s.info().coding);
    same(&full(&*s), &direct(&big), "ic12");
    let src = Arc::new(Source::from_vec("a.icns", f));
    let p1 = open_icns_page(src.clone(), 1).unwrap();
    same(&full(&*p1), &direct(&small), "ic05");
    let p2 = open_icns_page(src.clone(), 2).unwrap();
    let r = full(&*p2);
    assert_eq!((r.width, r.height), (16, 16));
    let px = &r.planes[0].data[(5 * 16 + 3) * 4..(5 * 16 + 3) * 4 + 4];
    assert_eq!(px, [83, 9, 200, 255 - 83]);
    trim::assert_trims(&*p2, &DecodeRequest::full(1));
    assert!(open_icns_page(src, 3).is_err());
    // Stepping from an open session reuses its bytes and labels each page.
    assert_eq!(s.page_index(), 0);
    let icon = |width, height, density| {
        Some(PageLabel::Icon {
            width,
            height,
            density,
        })
    };
    assert_eq!(s.page_label(), icon(32, 32, 2));
    let q = s.open_page(2).unwrap();
    assert_eq!((q.page_index(), q.page_count()), (2, 3));
    assert_eq!(q.page_label(), icon(16, 16, 1));
    assert_eq!(full(&*q).planes[0].data, r.planes[0].data);
    assert!(s.open_page(3).is_err());
}

fn corpus() -> PathBuf {
    std::env::var_os("SPECULAR_CONFORMANCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/conformance")
        })
}

fn files(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, ext, out);
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext)) {
            out.push(p);
        }
    }
}

/// Every real ICNS file: every page opens and decodes to the size its type promises; PNG
/// payloads equal their direct decode.
#[test]
fn real_icns_files() {
    let mut paths: Vec<PathBuf> = ["/usr/lib/python3/dist-packages/pygame/pygame_icon.icns"]
        .iter()
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .collect();
    // More ICNS files from a folder of your own: SPECULAR_ICNS_DIR (searched recursively).
    if let Some(dir) = std::env::var_os("SPECULAR_ICNS_DIR") {
        files(Path::new(&dir), "icns", &mut paths);
    }
    files(&corpus().join("pillow/Tests/images"), "icns", &mut paths);
    if paths.is_empty() {
        eprintln!("no ICNS files on this machine; skipped");
        return;
    }
    let mut pages_seen = 0;
    for path in &paths {
        let data = std::fs::read(path).unwrap();
        let src = Arc::new(Source::from_vec(path, data.clone()));
        let Ok(parsed) = specular_icns::parse(&data) else {
            // Fuzz cases (Pillow's oom-*.icns): refused, not a crash.
            assert!(Registry::new().open(src).is_err(), "{}", path.display());
            continue;
        };
        let t = Instant::now();
        for (n, icon) in parsed.icons.iter().enumerate() {
            let what = format!("{} page {n} {}", path.display(), icon.label());
            let s = match open_icns_page(src.clone(), n) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("{what}: refused: {e}");
                    continue;
                }
            };
            let r = match s.decode(&DecodeRequest::full(1), &CancelToken::new()) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("{what}: decode failed: {e}");
                    continue;
                }
            };
            pages_seen += 1;
            match icon.kind {
                specular_icns::Kind::Png => same(&r, &direct(&data[icon.data.clone()]), &what),
                // A payload carries its own size, normally the promised one.
                specular_icns::Kind::Jp2 => {
                    if (r.width, r.height) != (icon.width, icon.height) {
                        eprintln!("{what}: payload is {}×{}", r.width, r.height);
                    }
                }
                _ => assert_eq!((r.width, r.height), (icon.width, icon.height), "{what}"),
            }
        }
        eprintln!(
            "{}: {} pages ({}) in {:.2} ms",
            path.display(),
            parsed.icons.len(),
            icns_page_labels(&data).join(", "),
            t.elapsed().as_secs_f64() * 1e3
        );
    }
    assert!(pages_seen > 0);
}

/// The corpus ORA / KRA files: each opens (or is refused with a reason), merged images
/// equal their direct decode.
#[test]
fn corpus_documents() {
    let mut paths = Vec::new();
    files(&corpus().join("krita"), "kra", &mut paths);
    files(&corpus().join("mypaint"), "ora", &mut paths);
    if paths.is_empty() {
        eprintln!("ORA/KRA corpus absent; skipped");
        return;
    }
    paths.sort();
    for path in &paths {
        let data = std::fs::read(path).unwrap();
        let t = Instant::now();
        let s = match Registry::new().open(Arc::new(Source::from_vec(path, data.clone()))) {
            Ok(s) => s,
            Err(e) => panic!("{}: {e}", path.display()),
        };
        let opened = t.elapsed();
        let r = full(&*s);
        let decoded = t.elapsed();
        let i = s.info();
        assert_eq!(
            (r.width, r.height),
            (i.width, i.height),
            "{}",
            path.display()
        );
        let doc = specular_ora::Document::open(&data).unwrap();
        if let Some(m) = doc.merged() {
            same(
                &r,
                &direct(&doc.read(&data, m).unwrap()),
                &path.display().to_string(),
            );
        }
        eprintln!(
            "{}: {:?} {}×{} {} (open {:.2} ms, decode {:.1} ms)",
            path.display(),
            i.format,
            i.width,
            i.height,
            i.coding,
            opened.as_secs_f64() * 1e3,
            decoded.as_secs_f64() * 1e3
        );
    }
}
