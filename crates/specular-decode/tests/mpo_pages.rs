//! Multi-Picture (MPO / MPF) JPEGs: the further images of the MPF index are pages after the
//! primary; large thumbnails and gain maps are not. Synthetic files from the in-tree encoder,
//! plus the Pillow MPO samples of the conformance corpus checked against ImageMagick when it is
//! installed.

use specular_core::{CancelToken, PixelLayout, Raster, Source};
use specular_decode::{DecodeError, DecodeRequest, PageLabel, Registry, Session};
use specular_jpeg as sj;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn jpeg(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
    let px: Vec<u8> = (0..w * h).flat_map(|_| rgb).collect();
    sj::encode::encode(w, h, sj::encode::Samples::Rgb(&px), &Default::default()).unwrap()
}

/// `primary` with an MPF APP2 segment after SOI indexing it and `others` (attribute, JPEG),
/// which follow it in the file.
fn mpo(primary: &[u8], others: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let n = 1 + others.len() as u32;
    // TIFF header, IFD (3 entries), MP entries.
    let entries_at = 8 + 2 + 3 * 12 + 4;
    let tiff_len = entries_at + 16 * n as usize;
    let seg_len = 2 + 2 + 4 + tiff_len; // marker, length, "MPF\0", payload
    let primary_len = primary.len() + seg_len;
    let mut t = Vec::new();
    t.extend_from_slice(b"MM\0*");
    t.extend_from_slice(&8u32.to_be_bytes());
    t.extend_from_slice(&3u16.to_be_bytes());
    let entry = |t: &mut Vec<u8>, tag: u16, ty: u16, count: u32, val: [u8; 4]| {
        t.extend_from_slice(&tag.to_be_bytes());
        t.extend_from_slice(&ty.to_be_bytes());
        t.extend_from_slice(&count.to_be_bytes());
        t.extend_from_slice(&val);
    };
    entry(&mut t, 0xB000, 7, 4, *b"0100");
    entry(&mut t, 0xB001, 4, 1, n.to_be_bytes());
    entry(&mut t, 0xB002, 7, 16 * n, (entries_at as u32).to_be_bytes());
    t.extend_from_slice(&0u32.to_be_bytes());
    // The TIFF header sits 8 bytes into the primary (SOI, marker, length, "MPF\0").
    let tiff_at = 2 + 2 + 2 + 4;
    let mut at = primary_len;
    let mp = |t: &mut Vec<u8>, attr: u32, size: usize, offset: u32| {
        t.extend_from_slice(&attr.to_be_bytes());
        t.extend_from_slice(&(size as u32).to_be_bytes());
        t.extend_from_slice(&offset.to_be_bytes());
        t.extend_from_slice(&[0; 4]);
    };
    mp(&mut t, 0x2003_0000, primary_len, 0);
    for (attr, j) in others {
        mp(&mut t, *attr, j.len(), (at - tiff_at) as u32);
        at += j.len();
    }
    assert_eq!(t.len(), tiff_len);
    let mut out = primary[..2].to_vec();
    out.extend_from_slice(&[0xFF, 0xE2]);
    out.extend_from_slice(&((seg_len - 2) as u16).to_be_bytes());
    out.extend_from_slice(b"MPF\0");
    out.extend_from_slice(&t);
    out.extend_from_slice(&primary[2..]);
    assert_eq!(out.len(), primary_len);
    for (_, j) in others {
        out.extend_from_slice(j);
    }
    out
}

/// A JPEG with an ISO 21496-1 APP2 segment: what a gain map carries.
fn gain_map_like(j: &[u8]) -> Vec<u8> {
    let sig = b"urn:iso:std:iso:ts:21496:-1\0";
    let mut out = j[..2].to_vec();
    out.extend_from_slice(&[0xFF, 0xE2]);
    out.extend_from_slice(&((2 + sig.len() + 4) as u16).to_be_bytes());
    out.extend_from_slice(sig);
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&j[2..]);
    out
}

fn open(data: Vec<u8>) -> Arc<dyn Session> {
    Registry::new()
        .open(Arc::new(Source::from_vec("t.mpo", data)))
        .unwrap()
}

fn full(s: &dyn Session) -> Raster {
    s.decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap()
}

/// Packed RGB8 of a decoded JPEG raster (YCbCr to RGB with JFIF coefficients, nearest
/// chroma).
fn rgb(r: &Raster) -> Vec<u8> {
    let (w, h) = (r.width as usize, r.height as usize);
    let mut out = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            let s = |i: usize| {
                let p = &r.planes[i];
                let (px, py) = (x / p.h_div as usize, y / p.v_div as usize);
                f32::from(p.data[py * p.stride + px])
            };
            match r.layout {
                PixelLayout::YCbCr => {
                    let (yy, cb, cr) = (s(0), s(1) - 128.0, s(2) - 128.0);
                    for v in [
                        yy + 1.402 * cr,
                        yy - 0.344_136 * cb - 0.714_136 * cr,
                        yy + 1.772 * cb,
                    ] {
                        out.push(v.round().clamp(0.0, 255.0) as u8);
                    }
                }
                PixelLayout::Gray => out.extend_from_slice(&[s(0) as u8; 3]),
                PixelLayout::Rgb => out.extend_from_slice(&[s(0) as u8, s(1) as u8, s(2) as u8]),
                other => panic!("unexpected layout {other:?}"),
            }
        }
    }
    out
}

fn size(w: u32, h: u32) -> Option<PageLabel> {
    Some(PageLabel::Image {
        width: w,
        height: h,
        bits: None,
    })
}

#[test]
fn further_images_are_pages() {
    let primary = jpeg(32, 16, [200, 50, 50]);
    let view = jpeg(24, 16, [50, 200, 50]);
    let thumb = jpeg(8, 8, [9, 9, 9]);
    let map = gain_map_like(&jpeg(8, 4, [128, 128, 128]));
    let third = jpeg(16, 8, [50, 50, 200]);
    let file = mpo(
        &primary,
        &[
            (0x0002_0002, view),
            (0x0001_0001, thumb),
            (0, map),
            (0x0002_0002, third),
        ],
    );
    let s = open(file.clone());
    assert_eq!((s.page_count(), s.page_index()), (3, 0));
    assert_eq!(s.page_label(), size(32, 16));
    assert!(
        s.info().coding.contains("MPF image 1 of 3"),
        "{}",
        s.info().coding
    );
    let centre = |s: &dyn Session| {
        let r = full(s);
        let p = rgb(&r);
        let i = ((r.height / 2 * r.width + r.width / 2) * 3) as usize;
        [p[i], p[i + 1], p[i + 2]]
    };
    let near = |a: [u8; 3], b: [u8; 3]| a.iter().zip(b).all(|(&x, y)| x.abs_diff(y) <= 3);
    assert!(near(centre(s.as_ref()), [200, 50, 50]));
    let p1 = s.open_page(1).unwrap();
    assert_eq!((p1.page_index(), p1.page_label()), (1, size(24, 16)));
    assert!(near(centre(p1.as_ref()), [50, 200, 50]));
    assert!(!p1.has_gain_map());
    let p2 = p1.open_page(2).unwrap();
    assert_eq!(p2.page_label(), size(16, 8));
    assert!(near(centre(p2.as_ref()), [50, 50, 200]));
    let back = p2.open_page(0).unwrap();
    assert_eq!(back.page_label(), size(32, 16));
    assert!(near(centre(back.as_ref()), [200, 50, 50]));
    assert!(matches!(s.open_page(3), Err(DecodeError::Malformed(_))));
    assert!(p2.decode_memory(&DecodeRequest::full(1)) >= 16 * 8 * 3);
    let gone = CancelToken::new();
    gone.cancel();
    assert!(matches!(
        p1.decode(&DecodeRequest::full(1), &gone),
        Err(DecodeError::Cancelled)
    ));

    // Cut in the last image: it reads as truncated (or fails), never panics. Every prefix
    // opens or fails cleanly, with its pages.
    let cut = open(file[..file.len() - 4].to_vec());
    assert_eq!(cut.page_count(), 3);
    if let Ok(r) = cut
        .open_page(2)
        .and_then(|p| p.decode(&DecodeRequest::full(1), &CancelToken::new()))
    {
        assert!(r.truncated);
    }
    for n in (0..file.len()).step_by(13) {
        let Ok(s) = Registry::new().open(Arc::new(Source::from_vec("t.mpo", file[..n].to_vec())))
        else {
            continue;
        };
        for page in 0..s.page_count() {
            if let Ok(p) = s.open_page(page) {
                let _ = p.decode(&DecodeRequest::full(1), &CancelToken::new());
            }
        }
    }
}

#[test]
fn plain_jpegs_have_one_page() {
    let s = open(jpeg(8, 8, [1, 2, 3]));
    assert_eq!(s.page_count(), 1);
    assert_eq!(s.page_label(), None);
    assert!(s.open_page(1).is_err());
    // An MPF index of only previews (as cameras write) is not paged either.
    let s = open(mpo(
        &jpeg(32, 16, [1, 2, 3]),
        &[(0x0001_0002, jpeg(8, 8, [0; 3]))],
    ));
    assert_eq!(s.page_count(), 1);
    assert_eq!(s.page_label(), None);
}

fn corpus() -> PathBuf {
    std::env::var_os("SPECULAR_CONFORMANCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/conformance")
        })
}

/// ImageMagick's RGB8 of frame `page`, if `magick` is installed.
fn magick(path: &Path, page: u32) -> Option<Vec<u8>> {
    let out = std::process::Command::new("magick")
        .arg(format!("{}[{page}]", path.display()))
        .args(["-depth", "8", "rgb:-"])
        .output()
        .ok()?;
    out.status.success().then_some(out.stdout)
}

#[test]
fn corpus_mpo_pages_match_imagemagick() {
    let dir = corpus().join("pillow/Tests/images");
    let expect = [
        ("frozenpond.mpo", 2),
        ("sugarshack.mpo", 2),
        ("sugarshack_ifd_offset.mpo", 2),
        ("fujifilm.mpo", 2),
    ];
    if !dir.join("frozenpond.mpo").exists() {
        eprintln!("MPO corpus absent; skipped");
        return;
    }
    for (name, pages) in expect {
        let path = dir.join(name);
        let data = std::fs::read(&path).unwrap();
        let s = open(data);
        assert_eq!(s.page_count(), pages, "{name}");
        for page in 0..pages {
            let p = s.open_page(page).unwrap();
            let r = full(p.as_ref());
            assert!(!r.truncated, "{name} page {page}");
            let ours = rgb(&r);
            let Some(theirs) = magick(&path, page) else {
                eprintln!("magick missing; pixel comparison skipped");
                continue;
            };
            assert_eq!(ours.len(), theirs.len(), "{name} page {page}");
            let sum: u64 = ours
                .iter()
                .zip(&theirs)
                .map(|(&a, &b)| u64::from(a.abs_diff(b)))
                .sum();
            let mean = sum as f64 / ours.len() as f64;
            eprintln!("{name} page {page}: mean |Δ| {mean:.3} vs ImageMagick");
            assert!(mean < 2.0, "{name} page {page}: mean |Δ| {mean}");
        }
        // The two views differ (a stereo pair, not the same image twice).
        if pages == 2 {
            let a = rgb(&full(s.as_ref()));
            let b = rgb(&full(s.open_page(1).unwrap().as_ref()));
            assert_ne!(a, b, "{name}");
        }
    }
    // The second MPF image here is an Apple HDR gain map (Pillow shows it as frame 2): not a
    // page, but the primary's gain map.
    let s = open(std::fs::read(dir.join("frame_size.mpo")).unwrap());
    assert_eq!(s.page_count(), 1);
    assert_eq!(s.page_label(), None);
    assert!(s.has_gain_map());
    // A disparity (stereo) frame of its own size: a page (ImageMagick, scanning for the
    // next SOI rather than reading the index, misses it).
    let s = open(std::fs::read(dir.join("ignore_frame_size.mpo")).unwrap());
    assert_eq!(s.page_count(), 2);
    let p = s.open_page(1).unwrap();
    eprintln!("ignore_frame_size.mpo page 1: {:?}", p.page_label());
    assert!(!full(p.as_ref()).truncated);
    // Damaged or unusual indexes: open, page and decode without panicking.
    for name in ["sugarshack_no_data.mpo"] {
        let Ok(data) = std::fs::read(dir.join(name)) else {
            continue;
        };
        let Ok(s) = Registry::new().open(Arc::new(Source::from_vec(name, data))) else {
            continue;
        };
        eprintln!("{name}: {} pages", s.page_count());
        for page in 0..s.page_count() {
            if let Ok(p) = s.open_page(page) {
                let _ = p.decode(&DecodeRequest::full(1), &CancelToken::new());
            }
        }
    }
}
