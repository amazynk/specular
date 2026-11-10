//! Fixtures made with DjVuLibre's encoders (fixtures/make.sh) and compared with its
//! renderer's output (fixtures/refs.sh, `ddjvu` at full size).

use super::*;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

fn bytes(name: &str) -> Bytes {
    Arc::new(std::fs::read(fixture(name)).unwrap())
}

fn open(name: &str) -> Document {
    Document::open(bytes(name), Options::default()).unwrap()
}

/// A reference PNG as RGB.
fn reference(name: &str) -> (u32, u32, Vec<u8>) {
    let f = std::fs::File::open(fixture(&format!("ref/{name}.png"))).unwrap();
    let mut d = png::Decoder::new(std::io::BufReader::new(f));
    d.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut r = d.read_info().unwrap();
    let mut buf = vec![0; r.output_buffer_size().unwrap()];
    let info = r.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    let rgb = match info.color_type {
        png::ColorType::Rgb => buf,
        png::ColorType::Rgba => buf.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g]).collect(),
        png::ColorType::GrayscaleAlpha => buf.chunks(2).flat_map(|p| [p[0], p[0], p[0]]).collect(),
        png::ColorType::Indexed => unreachable!("expanded"),
    };
    (info.width, info.height, rgb)
}

fn render_page(doc: &Document, page: usize, scale: u32) -> (u32, u32, Vec<u8>) {
    let info = doc.page_info(page).unwrap();
    let (w, h) = info.display_size();
    let (w, h) = (w.div_ceil(scale), h.div_ceil(scale));
    let rgba = doc
        .render(
            RenderRequest {
                page,
                scale,
                x: 0,
                y: 0,
                width: w,
                height: h,
            },
            &|| false,
        )
        .unwrap();
    (
        w,
        h,
        rgba.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect(),
    )
}

/// Mean and max absolute channel difference.
fn diff(a: &[u8], b: &[u8]) -> (f64, u8) {
    assert_eq!(a.len(), b.len());
    let mut sum = 0u64;
    let mut max = 0u8;
    for (x, y) in a.iter().zip(b) {
        let d = x.abs_diff(*y);
        sum += u64::from(d);
        max = max.max(d);
    }
    (sum as f64 / a.len() as f64, max)
}

fn check(file: &str, page: usize, refname: &str, max_mean: f64, max_max: u8) {
    let doc = open(file);
    let (w, h, ours) = render_page(&doc, page, 1);
    let (rw, rh, theirs) = reference(refname);
    assert_eq!((w, h), (rw, rh), "{file} size");
    let (mean, max) = diff(&ours, &theirs);
    assert!(
        mean <= max_mean && max <= max_max,
        "{file} p{page}: mean {mean:.3} max {max} (allowed {max_mean} / {max_max})"
    );
}

#[test]
fn bzz_round_trip_through_the_directory() {
    let doc = open("doc.djvu");
    assert_eq!(doc.page_count(), 4);
    assert_eq!(doc.facts().structure, "bundled");
    assert_eq!(doc.facts().thumbnails, 4);
}

#[test]
fn jb2_bitonal_matches_ddjvu_exactly() {
    check("text.djvu", 0, "text", 0.0, 0);
    check("text_refine.djvu", 0, "text_refine", 0.0, 0);
}

#[test]
fn iw44_colour_and_grey_match_ddjvu() {
    check("photo.djvu", 0, "photo", 0.0, 0);
    check("photo_full.djvu", 0, "photo_full", 0.0, 0);
    check("gray.djvu", 0, "gray", 0.0, 0);
}

/// The IW44 image file (`FORM:PM44` / `FORM:BM44`, chunks of that name, no INFO) holding a
/// photo page's `BG44` chunks.
fn iw44_file(name: &str, kind: &[u8; 4]) -> Bytes {
    let d = std::fs::read(fixture(name)).unwrap();
    assert_eq!(&d[12..16], b"DJVU");
    let mut body = kind.to_vec();
    let mut p = 16;
    while p + 8 <= d.len() {
        let len = u32::from_be_bytes(d[p + 4..p + 8].try_into().unwrap()) as usize;
        let id = &d[p..p + 4];
        if id == b"BG44" {
            body.extend_from_slice(kind);
            body.extend_from_slice(&d[p + 4..p + 8 + len]);
            if len % 2 == 1 {
                body.push(0);
            }
        }
        p += 8 + len + len % 2;
    }
    let mut f = b"AT&TFORM".to_vec();
    f.extend((body.len() as u32).to_be_bytes());
    f.extend(body);
    Arc::new(f)
}

#[test]
fn iw44_image_files_match_ddjvu() {
    // The pictures of photo.djvu and gray.djvu as IW44 files: drawn as ddjvu draws the pages.
    for (file, kind, refname) in [
        ("photo.djvu", b"PM44", "photo"),
        ("gray.djvu", b"BM44", "gray"),
    ] {
        let data = iw44_file(file, kind);
        assert!(looks_like_djvu((*data).as_ref()));
        let doc = Document::open(data, Options::default()).unwrap();
        assert_eq!((doc.page_count(), doc.facts().structure), (1, "IW44 image"));
        let i = doc.page_info(0).unwrap();
        assert_eq!(i.dpi, 100);
        assert!(i.layers.starts_with("IW44 picture"), "{}", i.layers);
        let (w, h, ours) = render_page(&doc, 0, 1);
        let (rw, rh, theirs) = reference(refname);
        assert_eq!((w, h), (rw, rh), "{file}");
        assert_eq!(diff(&ours, &theirs), (0.0, 0), "{file}");
    }
}

#[test]
fn rotations_match_ddjvu() {
    check("text_rot.djvu", 0, "text_rot", 0.0, 0);
    check("photo_rot90.djvu", 0, "photo_rot90", 0.0, 0);
    check("photo_rot180.djvu", 0, "photo_rot180", 0.0, 0);
    check("photo_rot270.djvu", 0, "photo_rot270", 0.0, 0);
}

#[test]
fn bundled_pages_match_ddjvu() {
    for p in 0..4 {
        check("doc.djvu", p, &format!("doc_p{}", p + 1), 0.0, 0);
    }
}

#[test]
fn indirect_document_resolves_its_files() {
    let dir = fixture("indirect");
    let resolver: Resolver = Arc::new(move |name: &str| {
        let p = dir.join(name);
        std::fs::read(p).ok().map(|v| Arc::new(v) as Bytes)
    });
    let doc = Document::open(
        bytes("indirect/index.djvu"),
        Options {
            resolver: Some(resolver),
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(doc.facts().structure, "indirect");
    assert_eq!(doc.page_count(), 4);
    let (_, _, ours) = render_page(&doc, 1, 1);
    let (_, _, theirs) = reference("doc_p2");
    assert_eq!(diff(&ours, &theirs).1, 0);
    // Without a resolver, the missing part is named.
    let bare = Document::open(bytes("indirect/index.djvu"), Options::default()).unwrap();
    let e = bare.page_info(0).unwrap_err();
    assert!(matches!(e, Error::Missing(_)), "{e}");
}

#[test]
fn text_layer_words_and_rectangles() {
    let doc = open("text_txt.djvu");
    let t = doc.page_text(0).unwrap().unwrap();
    let words: Vec<_> = t
        .words()
        .map(|(s, z)| (s.to_string(), z.x, z.y, z.width, z.height))
        .collect();
    // djvused coordinates are bottom-up: (word 20 335 90 370 "DjVu") on a 400-row page.
    assert_eq!(words[0], ("DjVu".to_string(), 20, 30, 70, 35));
    assert_eq!(words.len(), 5);
    assert!(t.plain().contains("jumps over"), "{:?}", t.plain());
    assert!(doc.page_info(0).unwrap().has_text);
}

#[test]
fn metadata_outline_and_thumbnails() {
    let doc = open("doc.djvu");
    let f = doc.facts();
    assert!(
        f.metadata
            .iter()
            .any(|(k, v)| k == "Producer" && v == "DjVuLibre djvm"),
        "{:?}",
        f.metadata
    );
    assert_eq!(f.outline_entries, 3);
    assert_eq!(doc.outline()[1].children[0].title, "Compound");
    assert_eq!(doc.resolve_target(&doc.outline()[1].url), Some(1));
    let th = doc.thumbnail(1, &|| false).unwrap();
    assert!(th.width <= 48 && th.height <= 48 && th.width.max(th.height) >= 40);
}

#[test]
fn page_info_facts() {
    let doc = open("photo.djvu");
    let i = doc.page_info(0).unwrap();
    assert_eq!((i.width, i.height, i.dpi, i.gamma), (211, 157, 100, 22));
    assert!(i.layers.contains("IW44"), "{}", i.layers);
    let doc = open("text_rot.djvu");
    let i = doc.page_info(0).unwrap();
    assert_eq!(i.rotation, Rotation::Ccw90);
    assert_eq!(i.display_size(), (400, 640));
}

#[test]
fn compound_palette_page_renders() {
    // cpaldjvu's own output that this DjVuLibre build refuses ("corrupted palette"): drawn
    // with the colours of the shapes it has.
    let doc = open("compound.djvu");
    let (w, h, px) = render_page(&doc, 0, 1);
    assert_eq!((w, h), (300, 190));
    // The red bar.
    let at = |x: usize, y: usize| &px[(y * w as usize + x) * 3..(y * w as usize + x) * 3 + 3];
    assert_eq!(at(100, 25), [200, 0, 0]);
    assert_eq!(at(5, 5), [255, 255, 255]);
}

#[test]
fn regions_and_scales_agree_with_the_whole_page() {
    for file in [
        "photo.djvu",
        "text.djvu",
        "photo_rot90.djvu",
        "text_rot.djvu",
    ] {
        let doc = open(file);
        for scale in [1, 2, 4] {
            let (w, h, whole) = render_page(&doc, 0, scale);
            let (x, y) = (w / 3, h / 4);
            let (rw, rh) = (w / 2, h / 2);
            let part = doc
                .render(
                    RenderRequest {
                        page: 0,
                        scale,
                        x,
                        y,
                        width: rw,
                        height: rh,
                    },
                    &|| false,
                )
                .unwrap();
            for row in 0..rh {
                for col in 0..rw {
                    let a = &part[((row * rw + col) * 4) as usize..][..3];
                    let o = (((y + row) * w + x + col) * 3) as usize;
                    assert_eq!(a, &whole[o..o + 3], "{file} scale {scale} at {col},{row}");
                }
            }
        }
    }
}

#[test]
fn reduced_scales_average_the_page() {
    let doc = open("text.djvu");
    let (_, _, full) = render_page(&doc, 0, 1);
    let (w, h, half) = render_page(&doc, 0, 2);
    let fw = 640usize;
    // A 2×2 block average (rounded) equals the half-scale pixel.
    for (y, x) in [
        (10usize, 10usize),
        (30, 40),
        (h as usize - 1, w as usize - 1),
    ] {
        let mut s = 0u32;
        for dy in 0..2 {
            for dx in 0..2 {
                s += u32::from(full[((2 * y + dy) * fw + 2 * x + dx) * 3]);
            }
        }
        let want = (s as f32 / 4.0).round() as u8;
        assert_eq!(half[(y * w as usize + x) * 3], want);
    }
}

#[test]
fn g4_masks_need_the_hook() {
    let doc = open("mmr.djvu");
    assert!(doc.page_info(0).unwrap().layers.contains("G4"));
    let e = doc
        .render(
            RenderRequest {
                page: 0,
                scale: 1,
                x: 0,
                y: 0,
                width: 8,
                height: 8,
            },
            &|| false,
        )
        .unwrap_err();
    assert_eq!(e, Error::Unsupported("G4 masks"));
    // A hook that reads the strip as all one colour (this fixture's header flags it inverted).
    let doc = Document::open(
        bytes("mmr.djvu"),
        Options {
            mmr: Some(Arc::new(|_d: &[u8], w: u32, h: u32| {
                Some(vec![1; (w * h) as usize])
            })),
            ..Options::default()
        },
    )
    .unwrap();
    let (_, _, px) = render_page(&doc, 0, 4);
    assert!(px.iter().all(|&v| v == 255));
}

#[test]
fn cancellation_stops_a_decode() {
    let doc = open("photo.djvu");
    let r = doc.render(
        RenderRequest {
            page: 0,
            scale: 1,
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        },
        &|| true,
    );
    assert_eq!(r.unwrap_err(), Error::Cancelled);
}

#[test]
fn refusals_have_plain_reasons() {
    let e = Document::open(
        Arc::new(b"AT&TFORM\0\0\0\x04DJVI".to_vec()),
        Options::default(),
    )
    .unwrap_err();
    assert!(e.to_string().contains("shared part"), "{e}");
    let e = Document::open(Arc::new(b"not djvu".to_vec()), Options::default()).unwrap_err();
    assert!(e.to_string().contains("not a DjVu"), "{e}");
    // A page with only an unknown (e.g. encrypted) chunk.
    let mut d = b"AT&TFORM\0\0\0\x22DJVUINFO\0\0\0\x0a\0\x10\0\x10\x18\0\x2c\x01\x16\x01".to_vec();
    d.extend_from_slice(b"XENC\0\0\0\x04abcd");
    let doc = Document::open(Arc::new(d), Options::default()).unwrap();
    let e = doc.page_info(0).unwrap_err();
    assert!(e.to_string().contains("XENC"), "{e}");
}

#[test]
fn hostile_inputs_fail_without_panicking() {
    let base = std::fs::read(fixture("doc.djvu")).unwrap();
    let mut seed = 0x1234_5678u32;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    for _ in 0..150 {
        let mut d = base.clone();
        let n = 1 + rnd() % 8;
        for _ in 0..n {
            let p = rnd() as usize % d.len();
            d[p] = rnd() as u8;
        }
        if rnd() % 4 == 0 {
            let cut = rnd() as usize % d.len();
            d.truncate(cut);
        }
        let Ok(doc) = Document::open(Arc::new(d), Options::default()) else {
            continue;
        };
        for p in 0..doc.page_count() {
            if let Ok(i) = doc.page_info(p) {
                let (w, h) = i.display_size();
                let _ = doc.render(
                    RenderRequest {
                        page: p,
                        scale: 4,
                        x: 0,
                        y: 0,
                        width: w.div_ceil(4),
                        height: h.div_ceil(4),
                    },
                    &|| false,
                );
            }
            let _ = doc.page_text(p);
        }
    }
}

/// Shared dictionaries through `INCL`: components that each include the next one 64 times
/// (and the last includes the first) used to be resolved once per path, 64^8 times.
#[test]
fn includes_are_resolved_once() {
    let n = 10;
    let component = |i: usize| {
        let target = format!("c{}", (i + 1) % n);
        let mut body = b"DJVI".to_vec();
        for _ in 0..64 {
            body.extend_from_slice(b"INCL");
            body.extend_from_slice(&(target.len() as u32).to_be_bytes());
            body.extend_from_slice(target.as_bytes());
            if body.len() % 2 == 1 {
                body.push(0);
            }
        }
        let mut d = b"AT&TFORM".to_vec();
        d.extend_from_slice(&(body.len() as u32).to_be_bytes());
        d.extend_from_slice(&body);
        let len = d.len();
        let bytes: Bytes = Arc::new(d);
        let blob = OnceLock::new();
        let _ = blob.set(Some(Blob {
            bytes,
            start: 0,
            end: len,
        }));
        Component {
            id: format!("c{i}"),
            name: format!("c{i}"),
            title: String::new(),
            kind: ComponentKind::Include,
            blob,
        }
    };
    let doc = Document::build(
        Options::default(),
        "bundled",
        (0..n).map(component).collect(),
        Vec::new(),
    );
    let t = std::time::Instant::now();
    assert!(doc.dict(0, 0, &|| false).unwrap().is_none());
    assert!(t.elapsed().as_secs() < 5);
}

/// A 9-byte IW44 header declaring 16384 × 16384 grey pixels used to allocate 512 MB of
/// coefficient blocks before any data.
#[test]
fn iw44_blocks_wait_for_data() {
    let head = [0u8, 1, 0x81, 2, 0x40, 0, 0x40, 0, 0];
    let budget = Arc::new(specular_core::budget::MemoryBudget::fixed(4 << 30));
    let l = specular_core::budget::with_budget(budget.clone(), || iw44::Layer::new(&head)).unwrap();
    assert_eq!(l.size(), (16384, 16384));
    assert_eq!(l.allocated_blocks(), 0);
    // What the layer may come to hold is reserved up front: 6 bytes a pixel for grey.
    assert_eq!(budget.reserved(), 6 << 28);
    drop(l);
    assert_eq!(budget.reserved(), 0);
    // A budget that cannot hold it refuses the layer before anything is decoded.
    let small = Arc::new(specular_core::budget::MemoryBudget::fixed(64 << 20));
    let r = specular_core::budget::with_budget(small.clone(), || iw44::Layer::new(&head));
    assert!(matches!(r, Err(Error::NeedsMemory(_))));
    assert_eq!(small.reserved(), 0);
}

/// A cached page holds its pictures in the memory budget until it is trimmed or the
/// document dropped.
#[test]
fn cached_pages_hold_their_pictures() {
    use specular_core::budget::{MemoryBudget, with_budget};
    let doc = open("photo.djvu");
    let budget = Arc::new(MemoryBudget::fixed(1 << 30));
    with_budget(budget.clone(), || render_page(&doc, 0, 1));
    let kept = doc.memory_bytes();
    assert!(kept > 0);
    assert_eq!(budget.reserved(), kept as u64);
    assert_eq!(doc.trim(), kept);
    assert_eq!(budget.reserved(), 0);
    // Rendering again decodes the page again, and holds it again.
    with_budget(budget.clone(), || render_page(&doc, 0, 1));
    assert_eq!(budget.reserved(), kept as u64);
    drop(doc);
    assert_eq!(budget.reserved(), 0);
}

/// A G4 mask header declaring 65535 × 65535 (a byte per pixel: 4 GB).
#[test]
fn huge_g4_mask_is_refused() {
    let opts = Options {
        mmr: Some(Arc::new(|_: &[u8], _, _| None)),
        ..Options::default()
    };
    let doc = Document::build(opts, "single", Vec::new(), Vec::new());
    let mut d = b"MMR\0".to_vec();
    d.extend_from_slice(&[0xff, 0xff, 0xff, 0xff, 0, 0]);
    assert!(matches!(doc.mmr(&d), Err(Error::TooLarge)));
}

/// 200 000 blits of one page-high shape on a 65000-row page: each listed in all 508 bands
/// (100 million entries) before the cap.
#[test]
fn blit_bins_are_capped() {
    let shape = Arc::new(jb2::Shape {
        width: 1,
        height: 65_000,
        bits: Vec::new(),
    });
    let img = jb2::Image {
        width: 10,
        height: 65_000,
        shapes: vec![shape],
        blits: (0..200_000)
            .map(|_| jb2::Blit {
                shape: 0,
                left: 0,
                top: 0,
            })
            .collect(),
        damaged: false,
    };
    assert!(matches!(bin_blits(&img, 10, 65_000), Err(Error::TooLarge)));
    let img = jb2::Image {
        blits: img.blits[..10].to_vec(),
        ..img
    };
    assert_eq!(bin_blits(&img, 10, 65_000).unwrap()[0].len(), 10);
}
