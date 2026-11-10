//! PDF, Illustrator and EPS through the registry: page size choice, pages, regions, the
//! protected-PDF error, EPS previews and malformed input.

use specular_core::{CancelToken, Raster, Source};
use specular_decode::{DecodeError, DecodeRequest, Format, Rect, Registry};
use specular_pdf::fixture::{Page, build, encrypted, fill};
use std::sync::Arc;

fn open(name: &str, data: Vec<u8>) -> specular_decode::Result<Arc<dyn specular_decode::Session>> {
    Registry::default().open(Arc::new(Source::from_vec(name, data)))
}

fn px(r: &Raster, x: u32, y: u32) -> Vec<u8> {
    let p = &r.planes[0];
    let bpp = r.layout.pixel_bytes();
    let i = y as usize * p.stride + x as usize * bpp;
    p.data[i..i + bpp].to_vec()
}

/// Page 1: US Letter, red left half, text-free. Page 2: A4-ish, green top band.
fn doc() -> Vec<u8> {
    let p1 = "1 0 0 rg 0 0 306 792 re f";
    let p2 = "0 1 0 rg 0 742 595 100 re f";
    build(
        &[
            Page {
                width: 612.0,
                height: 792.0,
                content: p1,
            },
            Page {
                width: 595.0,
                height: 842.0,
                content: p2,
            },
        ],
        "",
        Some("/Title (Quarterly report) /Author (A. Writer) /Creator (Hand)"),
        "",
        &[],
    )
}

#[test]
fn pdf_pages_scales_and_regions() {
    // Leading bytes before the header are allowed.
    let mut data = b"junk before the header\n".to_vec();
    data.extend(doc());
    let s = open("a.pdf", data).unwrap();
    let i = s.info();
    assert_eq!(i.format, Format::Pdf);
    // 288 dpi: 612×792 pt → 2448×3168.
    assert_eq!((i.width, i.height), (2448, 3168));
    assert!(!i.has_alpha && i.fast_regions);
    assert!(i.coding.starts_with("PDF 1.7, 612×792 pt"), "{}", i.coding);
    assert_eq!((s.page_count(), s.page_index()), (2, 0));

    let c = CancelToken::new();
    let r = s.decode(&DecodeRequest::full(8), &c).unwrap();
    assert_eq!((r.width, r.height, r.scale), (306, 396, 8));
    assert_eq!(px(&r, 10, 10), [255, 0, 0, 255]);
    assert_eq!(px(&r, 300, 390), [255, 255, 255, 255]);

    // A region at full size matches the same pixels of a scale-2 render, up to the
    // anti-aliased edge between the halves.
    let req = DecodeRequest {
        scale: 1,
        region: Some(Rect {
            x: 1200,
            y: 1000,
            w: 48,
            h: 16,
        }),
        ..DecodeRequest::full(1)
    };
    let r = s.decode(&req, &c).unwrap();
    assert_eq!((r.width, r.height, r.origin), (48, 16, (1200, 1000)));
    assert_eq!(px(&r, 0, 0), [255, 0, 0, 255]);
    assert_eq!(px(&r, 47, 15), [255, 255, 255, 255]);
    // Regions clip to the page.
    let req = DecodeRequest {
        scale: 1,
        region: Some(Rect {
            x: 2440,
            y: 3160,
            w: 100,
            h: 100,
        }),
        ..DecodeRequest::full(1)
    };
    let r = s.decode(&req, &c).unwrap();
    assert_eq!((r.width, r.height), (8, 8));

    let p2 = s.open_page(1).unwrap();
    assert_eq!(p2.page_index(), 1);
    assert_eq!(p2.page_count(), 2);
    assert_eq!((p2.info().width, p2.info().height), (2380, 3368));
    let r = p2.decode(&DecodeRequest::full(8), &c).unwrap();
    assert_eq!(px(&r, 100, 2), [0, 255, 0, 255]);
    assert_eq!(px(&r, 100, 200), [255, 255, 255, 255]);
    assert!(s.open_page(2).is_err());

    // Document information reaches the info panel as XMP (dc:title, dc:creator).
    let xmp = String::from_utf8(s.embedded_xmp().unwrap()).unwrap();
    assert!(xmp.contains("Quarterly report") && xmp.contains("A. Writer"));
}

#[test]
fn cancelled_decode() {
    let s = open("a.pdf", doc()).unwrap();
    let c = CancelToken::new();
    c.cancel();
    assert!(matches!(
        s.decode(&DecodeRequest::full(1), &c),
        Err(DecodeError::Cancelled)
    ));
}

#[test]
fn small_pages_reach_the_minimum_size() {
    let content = fill(10.0, 5.0, [0.0, 0.0, 1.0]);
    let data = build(
        &[Page {
            width: 10.0,
            height: 5.0,
            content: &content,
        }],
        "",
        None,
        "",
        &[],
    );
    let s = open("tiny.pdf", data).unwrap();
    assert_eq!((s.info().width, s.info().height), (2560, 1280));
}

#[test]
fn protected_pdf_is_not_shown() {
    let e = open("secret.pdf", encrypted()).err().unwrap();
    assert!(matches!(e, DecodeError::Unsupported(_)));
    assert_eq!(
        e.to_string(),
        "unsupported: password-protected PDF: not shown"
    );
}

#[test]
fn illustrator_pdf_compatible() {
    let s = open("logo.ai", doc()).unwrap();
    assert_eq!(s.info().format, Format::Illustrator);
    assert_eq!(s.page_count(), 2);
}

/// A minimal little-endian 8-bit greyscale TIFF.
fn tiff_gray(w: u32, h: u32, pixels: &[u8]) -> Vec<u8> {
    let entries: [(u16, u16, u32); 9] = [
        (256, 4, w),
        (257, 4, h),
        (258, 3, 8),
        (259, 3, 1),
        (262, 3, 1),
        (273, 4, 0), // patched below
        (277, 3, 1),
        (278, 4, h),
        (279, 4, w * h),
    ];
    let ifd_len = 2 + entries.len() * 12 + 4;
    let data_at = (8 + ifd_len) as u32;
    let mut t = b"II*\0".to_vec();
    t.extend(8u32.to_le_bytes());
    t.extend((entries.len() as u16).to_le_bytes());
    for (tag, ty, v) in entries {
        let v = if tag == 273 { data_at } else { v };
        t.extend(tag.to_le_bytes());
        t.extend(ty.to_le_bytes());
        t.extend(1u32.to_le_bytes());
        if ty == 3 {
            t.extend((v as u16).to_le_bytes());
            t.extend([0, 0]);
        } else {
            t.extend(v.to_le_bytes());
        }
    }
    t.extend(0u32.to_le_bytes());
    t.extend_from_slice(pixels);
    t
}

fn dos_eps(ps: &[u8], tiff: Option<&[u8]>, wmf: Option<&[u8]>) -> Vec<u8> {
    let mut f = vec![0u8; 30];
    f[..4].copy_from_slice(&[0xC5, 0xD0, 0xD3, 0xC6]);
    let put = |f: &mut Vec<u8>, at: usize, part: Option<&[u8]>| {
        if let Some(p) = part {
            let off = f.len() as u32;
            f[at..at + 4].copy_from_slice(&off.to_le_bytes());
            f[at + 4..at + 8].copy_from_slice(&(p.len() as u32).to_le_bytes());
            f.extend_from_slice(p);
        }
    };
    put(&mut f, 4, Some(ps));
    put(&mut f, 12, wmf);
    put(&mut f, 20, tiff);
    f[28] = 0xFF;
    f[29] = 0xFF;
    f
}

const PS: &[u8] =
    b"%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 4 2\n%%EndComments\n0 0 4 2 rectfill\n";

#[test]
fn eps_tiff_preview() {
    let tiff = tiff_gray(4, 2, &[0, 64, 128, 255, 10, 20, 30, 40]);
    let s = open("fig.eps", dos_eps(PS, Some(&tiff), None)).unwrap();
    assert_eq!(s.info().format, Format::Eps);
    assert_eq!((s.info().width, s.info().height), (4, 2));
    assert!(s.info().coding.starts_with("TIFF preview"));
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert_eq!(px(&r, 1, 0)[0], 64);
    assert_eq!(px(&r, 3, 1)[0], 40);
}

#[test]
fn eps_epsi_preview() {
    let ps = b"%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 8 2\n%%EndComments\n\
               %%BeginPreview: 8 2 1 2\n%F0\n%0F\n%%EndPreview\n0 0 8 2 rectfill\n";
    let s = open("fig.epsi", ps.to_vec()).unwrap();
    assert_eq!(s.info().format, Format::Eps);
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert_eq!((r.width, r.height), (8, 2));
    assert_eq!(px(&r, 0, 0)[0], 0);
    assert_eq!(px(&r, 7, 0)[0], 255);
    assert_eq!(px(&r, 0, 1)[0], 255);
    assert_eq!(px(&r, 7, 1)[0], 0);
}

/// A placeable WMF of 100 × 50 units at 96 dpi: a red rectangle over the left half.
fn wmf_red_left() -> Vec<u8> {
    let rec = |out: &mut Vec<u8>, func: u16, words: &[i16], extra: &[u8]| {
        let size = 3 + words.len() + extra.len().div_ceil(2);
        out.extend_from_slice(&(size as u32).to_le_bytes());
        out.extend_from_slice(&func.to_le_bytes());
        for w in words {
            out.extend_from_slice(&w.to_le_bytes());
        }
        out.extend_from_slice(extra);
    };
    let mut d = vec![0xd7, 0xcd, 0xc6, 0x9a, 0, 0];
    for v in [0i16, 0, 100, 50] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    d.extend_from_slice(&96u16.to_le_bytes());
    d.extend_from_slice(&[0; 6]);
    for v in [1u16, 9, 0x300, 0, 0, 3, 0, 0, 0] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    rec(&mut d, 0x020b, &[0, 0], &[]);
    rec(&mut d, 0x020c, &[50, 100], &[]);
    rec(&mut d, 0x02fc, &[0], &[0xff, 0, 0, 0, 0, 0]);
    rec(&mut d, 0x012d, &[0], &[]);
    rec(&mut d, 0x02fa, &[5, 0, 0], &[0, 0, 0, 0]);
    rec(&mut d, 0x012d, &[1], &[]);
    rec(&mut d, 0x041b, &[50, 50, 0, 0], &[]);
    rec(&mut d, 0x0000, &[], &[]);
    d
}

#[test]
fn eps_wmf_preview() {
    let s = open("fig.eps", dos_eps(PS, None, Some(&wmf_red_left()))).unwrap();
    assert_eq!(s.info().format, Format::Eps);
    assert!(
        s.info().coding.starts_with("WMF preview"),
        "{}",
        s.info().coding
    );
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    // Drawn at the metafile decoder's size: 2 : 1 as the frame says.
    assert_eq!(r.width, 2 * r.height);
    let left = px(&r, r.width / 10, r.height / 2);
    assert!(left[0] > 200 && left[1] < 60, "{left:?}");
    let right = px(&r, r.width * 9 / 10, r.height / 2);
    // Unpainted: transparent.
    assert_eq!(right[3], 0, "{right:?}");
}

#[test]
fn eps_without_preview() {
    let e = open("fig.eps", PS.to_vec()).err().unwrap();
    assert_eq!(
        e.to_string(),
        "unsupported: EPS without a preview: PostScript is not rendered"
    );
    let e = open("fig.eps", dos_eps(PS, None, Some(b"\xD7\xCD\xC6\x9A wmf")))
        .err()
        .unwrap();
    assert!(e.to_string().contains("Windows Metafile"), "{e}");
    // A PostScript-only Illustrator file without a preview.
    let e = open(
        "old.ai",
        b"%!PS-Adobe-3.0\n%%Creator: Adobe Illustrator(TM) 8.0\n".to_vec(),
    )
    .err()
    .unwrap();
    assert!(e.to_string().contains("Illustrator"), "{e}");
}

#[test]
fn malformed_never_panics() {
    let good = doc();
    let c = CancelToken::new();
    for cut in (0..good.len()).step_by(13) {
        for name in ["a.pdf", "a.ai"] {
            if let Ok(s) = open(name, good[..cut].to_vec()) {
                let _ = s.decode(&DecodeRequest::full(8), &c);
            }
        }
    }
    let tiff = tiff_gray(4, 2, &[0; 8]);
    let eps = dos_eps(PS, Some(&tiff), None);
    for cut in 0..eps.len() {
        if let Ok(s) = open("a.eps", eps[..cut].to_vec()) {
            let _ = s.decode(&DecodeRequest::full(1), &c);
        }
    }
    assert!(open("a.pdf", b"%PDF-".to_vec()).is_err());
    assert!(
        open(
            "a.eps",
            b"%!PS-Adobe-3.0 EPSF-3.0\n%%BeginPreview: 9 9 8 9\n".to_vec()
        )
        .is_ok()
    );
}

/// A page with text in two of the embedded standard fonts (Helvetica, Times-Roman), a stroked
/// curve and filled shapes, against `tests/pdf/text-page.png` (rewritten with
/// `SPECULAR_UPDATE_PDF_GOLDEN=1`). Small differences are tolerated (anti-aliasing at other
/// SIMD levels).
#[test]
fn golden_text_page() {
    let content = "0.95 0.95 0.8 rg 0 0 200 100 re f \
                   0 0 0.6 rg BT /F1 18 Tf 10 70 Td (Specular PDF) Tj ET \
                   0.6 0 0 rg BT /F2 11 Tf 10 50 Td (Times-Roman \\(hayro\\)) Tj ET \
                   0 0.5 0 RG 2 w 10 15 m 60 45 110 -15 190 30 c S \
                   1 0.5 0 rg 150 60 30 30 re f";
    let data = build(
        &[Page {
            width: 200.0,
            height: 100.0,
            content,
        }],
        "",
        None,
        "",
        &[],
    );
    let s = open("text.pdf", data).unwrap();
    // 200×100 pt → 3200×1600 full (4 px/pt, doubled to reach 2048); scale 4 → 800×400.
    let r = s
        .decode(&DecodeRequest::full(4), &CancelToken::new())
        .unwrap();
    assert_eq!((r.width, r.height), (800, 400));
    let data = &r.planes[0].data;
    // The text is drawn: dark blue pixels of the title.
    assert!(
        data.chunks_exact(4)
            .any(|p| p[0] < 40 && p[1] < 40 && p[2] > 100)
    );

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pdf/text-page.png");
    if std::env::var_os("SPECULAR_UPDATE_PDF_GOLDEN").is_some() {
        let png =
            specular_png::encode(r.width, r.height, specular_png::ColorType::Rgba, data).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, png).unwrap();
        return;
    }
    let golden = Registry::default().open_path(&path).unwrap();
    let g = golden
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert_eq!((g.width, g.height), (r.width, r.height));
    let gd = &g.planes[0].data;
    let bpp = g.layout.pixel_bytes();
    let (mut off, mut worst) = (0usize, 0u8);
    for (i, p) in data.chunks_exact(4).enumerate() {
        let q = &gd[i * bpp..i * bpp + 3];
        let d = (0..3).map(|c| p[c].abs_diff(q[c])).max().unwrap();
        worst = worst.max(d);
        if d > 24 {
            off += 1;
        }
    }
    assert!(
        off * 1000 <= data.len() / 4,
        "{off} pixels differ (worst {worst})"
    );
}

/// Timing on real PDFs (not run by default): `SPECULAR_PDF_TIMING=a.pdf:b.pdf cargo test
/// --release -p specular-decode --test pdf -- --ignored --nocapture timing`. Prints open, the
/// thumbnail decode (scale 8, what a 256 px thumbnail asks for), a 512² tile at full size
/// and the whole first page at full size.
#[test]
#[ignore]
fn timing() {
    let Some(list) = std::env::var_os("SPECULAR_PDF_TIMING") else {
        return;
    };
    for path in std::env::split_paths(&list) {
        let t = std::time::Instant::now();
        let s = Registry::default().open_path(&path).unwrap();
        let open = t.elapsed();
        let (w, h) = (s.info().width, s.info().height);
        let t = std::time::Instant::now();
        let thumb = s
            .decode(&DecodeRequest::full(8), &CancelToken::new())
            .unwrap();
        let thumb_t = t.elapsed();
        let mut tile = DecodeRequest::full(1);
        tile.region = Some(Rect {
            x: w / 4,
            y: h / 4,
            w: 512.min(w),
            h: 512.min(h),
        });
        let t = std::time::Instant::now();
        s.decode(&tile, &CancelToken::new()).unwrap();
        let tile_t = t.elapsed();
        let t = std::time::Instant::now();
        s.decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        let full_t = t.elapsed();
        println!(
            "{}: {} pages, {w}×{h}; open {open:?}, thumb {}×{} {thumb_t:?}, tile {tile_t:?}, \
             full {full_t:?}",
            path.display(),
            s.page_count(),
            thumb.width,
            thumb.height
        );
    }
}
