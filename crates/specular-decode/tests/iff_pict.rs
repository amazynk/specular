//! IFF ILBM and PICT through the registry: content probing (PICT with and without its
//! 512-byte header, version 1 by extension only), errors at open for pictures with nothing
//! to show, a PICT wrapping one JPEG opened as that JPEG, trimming, and a sweep of the
//! gitignored conformance corpus when present (`SPECULAR_CONFORMANCE_DIR` overrides it).

#[path = "common/trim.rs"]
mod trim;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{CancelToken, PixelLayout, Raster, Source};
use specular_decode::{DecodeError, DecodeRequest, Format, Registry, Session};

fn open(name: &str, data: Vec<u8>) -> Result<Arc<dyn Session>, DecodeError> {
    Registry::new().open(Arc::new(Source::from_vec(name, data)))
}

fn full(s: &dyn Session, scale: u32) -> Raster {
    s.decode(&DecodeRequest::full(scale), &CancelToken::new())
        .unwrap()
}

fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut v = id.to_vec();
    v.extend((body.len() as u32).to_be_bytes());
    v.extend(body);
    if body.len() % 2 == 1 {
        v.push(0);
    }
    v
}

/// A 16×2 one-plane ILBM: red where the bit is set, black elsewhere.
fn ilbm() -> Vec<u8> {
    let mut bmhd = Vec::new();
    for v in [16u16, 2, 0, 0] {
        bmhd.extend(v.to_be_bytes());
    }
    bmhd.extend([1, 0, 0, 0, 0, 0, 1, 1, 0, 16, 0, 2]);
    let mut body = chunk(b"BMHD", &bmhd);
    body.extend(chunk(b"CMAP", &[0, 0, 0, 255, 0, 0]));
    body.extend(chunk(b"BODY", &[0xf0, 0x0f, 0x00, 0xff]));
    let mut v = b"ILBM".to_vec();
    v.extend(body);
    chunk(b"FORM", &v)
}

/// A version 2 PICT, `w`×`h`, one unpacked 32-bit DirectBitsRect of grey `g` (pack type 1),
/// optionally behind the 512-byte header.
fn pict(w: u16, h: u16, g: u8, header: bool) -> Vec<u8> {
    let mut v = if header { vec![0; 512] } else { Vec::new() };
    let rect = |v: &mut Vec<u8>| {
        for x in [0, 0, h, w] {
            v.extend(x.to_be_bytes());
        }
    };
    v.extend([0, 0]);
    rect(&mut v);
    v.extend([0x00, 0x11, 0x02, 0xff, 0x0c, 0x00, 0xff, 0xfe, 0, 0]);
    v.extend([0, 0x48, 0, 0, 0, 0x48, 0, 0]);
    rect(&mut v);
    v.extend([0; 4]);
    v.extend([0, 0x9a, 0, 0, 0, 0xff]);
    v.extend((0x8000 | (w * 4)).to_be_bytes());
    rect(&mut v);
    v.extend([
        0, 0, 0, 1, 0, 0, 0, 0, 0, 0x48, 0, 0, 0, 0x48, 0, 0, 0, 16, 0, 32, 0, 3, 0, 8,
    ]);
    v.extend([0; 12]);
    rect(&mut v);
    rect(&mut v);
    v.extend([0, 0]);
    for _ in 0..w * h {
        v.extend([0, g, g, g]);
    }
    v.extend([0, 0xff]);
    v
}

#[test]
fn iff_by_content() {
    let s = open("noext", ilbm()).unwrap();
    assert_eq!(s.info().format, Format::Iff);
    assert_eq!((s.info().width, s.info().height), (16, 2));
    assert!(
        s.info().coding.starts_with("ILBM 1 bitplane"),
        "{}",
        s.info().coding
    );
    assert_eq!(s.colour_model(), Some("Palette"));
    let r = full(&*s, 1);
    assert_eq!(r.layout, PixelLayout::Rgba);
    let p = &r.planes[0];
    let px = |x: usize, y: usize| &p.data[y * p.stride + x * 4..][..4];
    assert_eq!(px(0, 0), [255, 0, 0, 255]);
    assert_eq!(px(4, 0), [0, 0, 0, 255]);
    assert_eq!(px(15, 1), [255, 0, 0, 255]);
    trim::assert_trims(&*s, &DecodeRequest::full(1));
}

#[test]
fn pict_with_and_without_header() {
    for (header, name) in [(false, "noext"), (true, "noext"), (true, "x.pct")] {
        let s = open(name, pict(9, 4, 77, header)).unwrap();
        assert_eq!(s.info().format, Format::Pict);
        assert_eq!(s.info().coding, "PICT version 2, 1 bitmap");
        let r = full(&*s, 1);
        assert_eq!((r.width, r.height), (9, 4));
        assert_eq!(&r.planes[0].data[..4], [77, 77, 77, 255]);
        trim::assert_trims(&*s, &DecodeRequest::full(1));
    }
}

#[test]
fn pict_version1_only_by_extension_and_vector_drawn() {
    // Version 1, a 1-bit BitsRect.
    let mut v1 = vec![0; 512];
    v1.extend([
        0, 0, 0, 0, 0, 0, 0, 1, 0, 8, 0x11, 0x01, 0x90, 0, 2, 0, 0, 0, 0, 0, 1, 0, 8,
    ]);
    v1.extend([
        0, 0, 0, 0, 0, 1, 0, 8, 0, 0, 0, 0, 0, 1, 0, 8, 0, 0, 0xaa, 0, 0xff,
    ]);
    assert!(matches!(
        open("x.bin", v1.clone()),
        Err(DecodeError::UnsupportedFormat)
    ));
    let s = open("x.pict", v1).unwrap();
    assert_eq!(s.info().coding, "PICT version 1, 1 bitmap");
    let r = full(&*s, 1);
    let row: Vec<u8> = r.planes[0].data.chunks(4).map(|p| p[0]).collect();
    assert_eq!(row, [0, 255, 0, 255, 0, 255, 0, 255]);

    // Not a picture at all, named like one.
    assert!(matches!(
        open("x.pct", vec![7; 600]),
        Err(DecodeError::UnsupportedFormat)
    ));

    // Only a PaintRect: drawn (black 2×2 on white).
    let mut vector = pict(9, 4, 0, false);
    let ops = 10 + 4 + 26;
    vector.truncate(ops);
    let mut only_comment = vector.clone();
    vector.extend([0, 0x31, 0, 0, 0, 0, 0, 2, 0, 2, 0, 0xff]);
    let s = open("x.pict", vector).unwrap();
    assert_eq!(s.info().coding, "PICT version 2, 1 drawing operation");
    let r = full(&*s, 1);
    assert_eq!(&r.planes[0].data[..4], [0, 0, 0, 255]);
    assert_eq!(&r.planes[0].data[8 * 4..9 * 4], [255; 4]);

    // Nothing drawn: refused when opened, with the reason.
    only_comment.extend([0, 0xa0, 0, 1, 0, 0xff]);
    match open("x.pict", only_comment) {
        Err(DecodeError::Unsupported(m)) => assert!(m.contains("draws nothing"), "{m}"),
        r => panic!("{:?}", r.map(|s| s.info().clone())),
    }
}

fn corpus() -> PathBuf {
    std::env::var_os("SPECULAR_CONFORMANCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/conformance")
        })
}

fn files(dir: &Path, exts: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, exts, out);
        } else if p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| exts.contains(&e.to_ascii_lowercase().as_str()))
        {
            out.push(p);
        }
    }
}

/// Every corpus IFF and PICT file opens (or is refused as unsupported) and decodes to its
/// size at 1:1 and 1:2; the PICTs wrapping one JPEG open through the JPEG decoder.
#[test]
fn corpus_files() {
    let root = corpus();
    let mut paths = Vec::new();
    files(
        &root.join("twelvemonkeys"),
        &["iff", "lbm", "ilbm", "pict", "pct"],
        &mut paths,
    );
    files(
        &root.join("ffmpeg-samples"),
        &["iff", "lbm", "ilbm", "pict", "pct"],
        &mut paths,
    );
    if paths.is_empty() {
        eprintln!("corpus not fetched; skipped");
        return;
    }
    paths.sort();
    let (mut opened, mut jpegs) = (0, 0);
    for path in &paths {
        let data = std::fs::read(path).unwrap();
        let s = match Registry::new().open(Arc::new(Source::from_vec(path, data))) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{}: {e}", path.display());
                assert!(
                    matches!(
                        e,
                        DecodeError::Unsupported(_) | DecodeError::UnsupportedFormat
                    ),
                    "{}: {e}",
                    path.display()
                );
                continue;
            }
        };
        let i = s.info().clone();
        assert!(
            matches!(i.format, Format::Iff | Format::Pict),
            "{}",
            path.display()
        );
        jpegs += usize::from(i.coding.contains("QuickTime JPEG ("));
        for scale in [1, 2] {
            let r = full(&*s, scale);
            assert_eq!(
                (r.width, r.height),
                (i.width.div_ceil(scale), i.height.div_ceil(scale)),
                "{} at 1:{scale}",
                path.display()
            );
        }
        eprintln!("{}: {}×{} {}", path.display(), i.width, i.height, i.coding);
        opened += 1;
    }
    eprintln!("{opened} of {} opened, {jpegs} as their JPEG", paths.len());
}

/// Colour cycling plays: a one-plane ILBM whose two colours swap 30 times a second (CRNG
/// rate 8192, active) is an animation of two frames of 33 ms, frame 0 the still.
#[test]
fn ilbm_colour_cycling_plays() {
    let mut f = ilbm();
    // Insert a CRNG chunk (entries 0..=1) after the FORM header and type.
    let mut c = 0i16.to_be_bytes().to_vec();
    c.extend(8192i16.to_be_bytes());
    c.extend(1i16.to_be_bytes());
    c.extend([0, 1]);
    let crng = chunk(b"CRNG", &c);
    f.splice(12..12, crng.iter().copied());
    let len = (f.len() - 8) as u32;
    f[4..8].copy_from_slice(&len.to_be_bytes());
    let s = open("cycle.iff", f).unwrap();
    let a = s.animation().expect("animated");
    assert_eq!((a.frame_count, a.loop_count), (Some(2), 0));
    assert!(
        s.info().coding.contains("colour cycling (2 frames)"),
        "{}",
        s.info().coding
    );
    let still = full(&*s, 1);
    let mut fr = s.frames(1).unwrap();
    let c = CancelToken::new();
    let f0 = fr.next_frame(&c).unwrap().unwrap();
    let f1 = fr.next_frame(&c).unwrap().unwrap();
    assert!(fr.next_frame(&c).is_none());
    assert_eq!(f0.delay, std::time::Duration::from_millis(33));
    assert_eq!(f0.raster.planes[0].data, still.planes[0].data);
    // Red and black swapped.
    let px = |r: &Raster, i: usize| r.planes[0].data[i * 4..i * 4 + 3].to_vec();
    assert_eq!(
        (px(&f0.raster, 0), px(&f1.raster, 0)),
        (vec![255, 0, 0], vec![0, 0, 0])
    );
    assert_eq!(
        (px(&f0.raster, 4), px(&f1.raster, 4)),
        (vec![0, 0, 0], vec![255, 0, 0])
    );
    // Half size frames.
    let mut half = s.frames(2).unwrap();
    let h0 = half.next_frame(&c).unwrap().unwrap();
    assert_eq!((h0.raster.width, h0.raster.height), (8, 1));
}

/// An IFF ANIM opens as an animation (the still is its first frame) and plays every frame.
#[test]
fn anim_plays_its_frames() {
    let p = corpus().join("ffmpeg-iffx/anim5_1bpp.anim");
    let Ok(data) = std::fs::read(&p) else {
        eprintln!("ANIM sample not fetched; skipped");
        return;
    };
    let s = open("x.anim", data).unwrap();
    assert!(
        s.info().coding.starts_with("ANIM of 123 frames"),
        "{}",
        s.info().coding
    );
    assert_eq!(s.animation().unwrap().frame_count, Some(123));
    let still = full(&*s, 1);
    let c = CancelToken::new();
    let mut fr = s.frames(1).unwrap();
    assert_eq!(fr.delays().unwrap().len(), 123);
    let f0 = fr.next_frame(&c).unwrap().unwrap();
    assert_eq!(f0.raster.planes[0].data, still.planes[0].data);
    let mut n = 1;
    while let Some(f) = fr.next_frame(&c) {
        f.unwrap();
        n += 1;
    }
    assert_eq!(n, 123);
    fr.rewind().unwrap();
    let again = fr.next_frame(&c).unwrap().unwrap();
    assert_eq!(again.raster.planes[0].data, still.planes[0].data);
}
