use super::*;
use crate::Decoder;

fn open(kind: Kind, bytes: Vec<u8>) -> Result<Arc<dyn Session>> {
    let name = if kind == Kind::Wmf { "t.wmf" } else { "t.emf" };
    MetafileDecoder(kind).open(Arc::new(Source::from_vec(name, bytes)))
}

fn render(s: &dyn Session, scale: u32) -> Raster {
    s.decode(&DecodeRequest::full(scale), &CancelToken::new())
        .unwrap()
}

fn px(r: &Raster, x: u32, y: u32) -> [u8; 4] {
    let p = &r.planes[0];
    let o = y as usize * p.stride + x as usize * 4;
    p.data[o..o + 4].try_into().unwrap()
}

/// A 2×2 24-bit bottom-up DIB: green on top, blue below.
fn dib2() -> Vec<u8> {
    let mut d = Vec::new();
    for v in [40u32, 2, 2] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    d.extend_from_slice(&1u16.to_le_bytes());
    d.extend_from_slice(&24u16.to_le_bytes());
    d.extend_from_slice(&[0; 24]);
    // Bottom row (blue), then the top row (green); rows padded to 4 bytes.
    d.extend_from_slice(&[255, 0, 0, 255, 0, 0, 0, 0]);
    d.extend_from_slice(&[0, 255, 0, 0, 255, 0, 0, 0]);
    d
}

fn wmf_rec(out: &mut Vec<u8>, func: u16, words: &[i16], extra: &[u8]) {
    let size = 3 + words.len() + extra.len().div_ceil(2);
    out.extend_from_slice(&(size as u32).to_le_bytes());
    out.extend_from_slice(&func.to_le_bytes());
    for w in words {
        out.extend_from_slice(&w.to_le_bytes());
    }
    out.extend_from_slice(extra);
    if extra.len() % 2 == 1 {
        out.push(0);
    }
}

/// A placeable WMF, 100 × 50 at 96 dpi: a red square on the left, a 2×2 bitmap stretched over
/// the right half, a blue ellipse outline, and a line of text.
fn wmf() -> Vec<u8> {
    let mut d = Vec::new();
    d.extend_from_slice(&wmf::PLACEABLE_KEY);
    d.extend_from_slice(&[0, 0]);
    for v in [0i16, 0, 1000, 500] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    d.extend_from_slice(&960u16.to_le_bytes());
    d.extend_from_slice(&[0; 6]);
    // Header: memory metafile, 9 words, version 0x300; sizes are not checked.
    for v in [1u16, 9, 0x300, 0, 0, 3, 0, 0, 0] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    let mut r = Vec::new();
    wmf_rec(&mut r, 0x020b, &[0, 0], &[]); // window org
    wmf_rec(&mut r, 0x020c, &[500, 1000], &[]); // window ext (y, x)
    // Brush 0: solid red; pen 1: null.
    wmf_rec(&mut r, 0x02fc, &[0], &[0xff, 0, 0, 0, 0, 0]);
    wmf_rec(&mut r, 0x012d, &[0], &[]);
    wmf_rec(&mut r, 0x02fa, &[5, 0, 0], &[0, 0, 0, 0]);
    wmf_rec(&mut r, 0x012d, &[1], &[]);
    // RECTANGLE (bottom, right, top, left)
    wmf_rec(&mut r, 0x041b, &[500, 500, 0, 0], &[]);
    // STRETCHDIB over the right half: rop SRCCOPY, usage, srcH, srcW, ySrc, xSrc, destH,
    // destW, yDest, xDest.
    wmf_rec(
        &mut r,
        0x0f43,
        &[0x0020, 0x00cc, 0, 2, 2, 0, 0, 500, 500, 0, 500],
        &dib2(),
    );
    // A blue 20-unit pen and a hollow brush, then an ellipse in the left square.
    wmf_rec(&mut r, 0x02fa, &[0, 20, 0], &[0, 0, 0xff, 0]);
    wmf_rec(&mut r, 0x012d, &[2], &[]);
    wmf_rec(&mut r, 0x02fc, &[1], &[0, 0, 0, 0, 0, 0]);
    wmf_rec(&mut r, 0x012d, &[3], &[]);
    wmf_rec(&mut r, 0x0418, &[400, 400, 100, 100], &[]);
    // TEXTOUT
    wmf_rec(&mut r, 0x0521, &[2], b"hi\0\0\x0a\0\x0a\0");
    wmf_rec(&mut r, 0x0000, &[], &[]);
    d.extend_from_slice(&r);
    d
}

fn emf_rec(out: &mut Vec<u8>, typ: u32, payload: &[u8]) {
    let n = payload.len().div_ceil(4) * 4;
    out.extend_from_slice(&typ.to_le_bytes());
    out.extend_from_slice(&(8 + n as u32).to_le_bytes());
    out.extend_from_slice(payload);
    out.resize(out.len() + n - payload.len(), 0);
}

fn le(v: &[i32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// An EMF one inch square (96 × 96 at 96 dpi, the device 96 dpi too), with `body` records.
fn emf(body: &[u8]) -> Vec<u8> {
    let mut h = Vec::new();
    h.extend_from_slice(&le(&[1, 108]));
    h.extend_from_slice(&le(&[0, 0, 95, 95])); // bounds
    h.extend_from_slice(&le(&[0, 0, 2540, 2540])); // frame, 0.01 mm
    h.extend_from_slice(b" EMF");
    h.extend_from_slice(&le(&[0x10000, 0, 0]));
    h.extend_from_slice(&4u16.to_le_bytes());
    h.extend_from_slice(&[0, 0]);
    h.extend_from_slice(&le(&[0, 0, 0]));
    h.extend_from_slice(&le(&[960, 960, 254, 254])); // device px, mm
    h.extend_from_slice(&le(&[0, 0, 0, 254_000, 254_000]));
    assert_eq!(h.len(), 108);
    h.extend_from_slice(body);
    emf_rec(&mut h, 14, &le(&[0, 16, 20]));
    let len = h.len() as u32;
    h[48..52].copy_from_slice(&len.to_le_bytes());
    h
}

/// A GDI scene: a blue triangle over the top half and the 2×2 bitmap over the bottom half.
fn emf_gdi() -> Vec<u8> {
    let mut b = Vec::new();
    emf_rec(&mut b, 39, &le(&[1, 0, 0xff_0000, 0])); // brush 1: blue
    emf_rec(&mut b, 37, &le(&[1]));
    emf_rec(&mut b, 37, &(0x8000_0008u32).to_le_bytes()); // NULL_PEN
    let mut poly = le(&[0, 0, 96, 48, 3]);
    for v in [0i16, 0, 96, 0, 48, 48] {
        poly.extend_from_slice(&v.to_le_bytes());
    }
    emf_rec(&mut b, 86, &poly);
    // STRETCHDIBITS: bounds, dest x y, src x y w h, offBmi cbBmi offBits cbBits, usage, rop,
    // dest w h; the BITMAPINFO and bits follow the 80-byte fixed part.
    let dib = dib2();
    let mut sd = le(&[0, 0, 95, 95, 0, 48, 0, 0, 2, 2]);
    sd.extend_from_slice(&le(&[
        80,
        40,
        120,
        (dib.len() - 40) as i32,
        0,
        0x00cc_0020,
        96,
        48,
    ]));
    sd.extend_from_slice(&dib);
    emf_rec(&mut b, 81, &sd);
    emf(&b)
}

/// An EMF+ dual file: a GDI fallback red rectangle (not drawn) and an EMF+ green fill.
fn emf_plus() -> Vec<u8> {
    let plus_rec = |typ: u16, flags: u16, data: &[u8]| {
        let mut r = Vec::new();
        r.extend_from_slice(&typ.to_le_bytes());
        r.extend_from_slice(&flags.to_le_bytes());
        r.extend_from_slice(&(12 + data.len() as u32).to_le_bytes());
        r.extend_from_slice(&(data.len() as u32).to_le_bytes());
        r.extend_from_slice(data);
        r
    };
    let comment = |recs: &[Vec<u8>]| {
        let data: Vec<u8> = b"EMF+".iter().copied().chain(recs.concat()).collect();
        let mut p = (data.len() as u32).to_le_bytes().to_vec();
        p.extend_from_slice(&data);
        p
    };
    let mut b = Vec::new();
    let header = plus_rec(0x4001, 1, &le(&[0xdbc0_1002u32 as i32, 1, 96, 96]));
    emf_rec(&mut b, 70, &comment(&[header]));
    // FillRects with a solid ARGB colour (S flag): opaque green over the left half.
    let mut fr = (0xff00_ff00u32).to_le_bytes().to_vec();
    fr.extend_from_slice(&1u32.to_le_bytes());
    for v in [0f32, 0.0, 48.0, 96.0] {
        fr.extend_from_slice(&v.to_le_bytes());
    }
    emf_rec(&mut b, 70, &comment(&[plus_rec(0x400a, 0x8000, &fr)]));
    // The GDI fallback: a red rectangle over everything.
    emf_rec(&mut b, 39, &le(&[1, 0, 0x0000_00ff, 0]));
    emf_rec(&mut b, 37, &le(&[1]));
    emf_rec(&mut b, 43, &le(&[0, 0, 96, 96]));
    emf_rec(&mut b, 70, &comment(&[plus_rec(0x4002, 0, &[])]));
    emf(&b)
}

#[test]
fn wmf_shapes_bitmap_and_text() {
    let bytes = wmf();
    assert!(probe_wmf(&bytes));
    let s = open(Kind::Wmf, bytes).unwrap();
    let i = s.info();
    // 100 × 50 px, rendered ×16.
    assert_eq!((i.width, i.height), (1600, 800));
    assert!(i.coding.starts_with("WMF (placeable)"), "{}", i.coding);
    assert!(i.coding.contains("text not shown"), "{}", i.coding);
    let r = render(&*s, 8);
    assert_eq!((r.width, r.height), (200, 100));
    // Red square (inside the ellipse outline), the ellipse's blue outline, the bitmap.
    assert_eq!(px(&r, 50, 50), [255, 0, 0, 255]);
    let edge = px(&r, 50, 20);
    assert!(edge[2] > 200 && edge[0] < 60, "{edge:?}");
    assert_eq!(px(&r, 150, 5), [0, 255, 0, 255]);
    assert_eq!(px(&r, 150, 95), [0, 0, 255, 255]);
    // A region renders the same pixels.
    let reg = s
        .decode(
            &DecodeRequest {
                region: Some(crate::Rect {
                    x: 140,
                    y: 90,
                    w: 20,
                    h: 10,
                }),
                ..DecodeRequest::full(8)
            },
            &CancelToken::new(),
        )
        .unwrap();
    assert_eq!(px(&reg, 10, 5), px(&r, 150, 95));
}

#[test]
fn emf_gdi_scene() {
    let bytes = emf_gdi();
    assert!(probe_emf(&bytes) && !probe_wmf(&bytes));
    let s = open(Kind::Emf, bytes).unwrap();
    let i = s.info();
    assert_eq!((i.width, i.height), (1536, 1536));
    assert!(i.coding.starts_with("EMF, vector 96×96"), "{}", i.coding);
    // At ×8 down, two pixels per picture unit.
    let r = render(&*s, 8);
    assert_eq!((r.width, r.height), (192, 192));
    // Triangle apex at the bottom centre of the top half; its corners stay clear.
    assert_eq!(px(&r, 96, 4), [0, 0, 255, 255]);
    assert_eq!(px(&r, 4, 60)[3], 0);
    // The bitmap: green, then blue, over the bottom half.
    assert_eq!(px(&r, 96, 100), [0, 255, 0, 255]);
    assert_eq!(px(&r, 96, 188), [0, 0, 255, 255]);
}

#[test]
fn emf_plus_dual_skips_gdi_fallback() {
    let s = open(Kind::Emf, emf_plus()).unwrap();
    assert!(
        s.info().coding.starts_with("EMF+ (dual)"),
        "{}",
        s.info().coding
    );
    let r = render(&*s, 8);
    assert_eq!(px(&r, 20, 64), [0, 255, 0, 255]);
    // The fallback rectangle is not drawn.
    assert_eq!(px(&r, 140, 64)[3], 0);
}

#[test]
fn malformed_metafiles_fail_cleanly() {
    let cases = [
        (Kind::Wmf, wmf()),
        (Kind::Emf, emf_gdi()),
        (Kind::Emf, emf_plus()),
    ];
    let mut seed = 0x2545_f491_u32;
    for (kind, good) in cases {
        let try_one = |b: Vec<u8>| {
            if let Ok(s) = open(kind, b) {
                let _ = s.decode(&DecodeRequest::full(8), &CancelToken::new());
            }
        };
        for n in 0..good.len() {
            try_one(good[..n].to_vec());
        }
        for _ in 0..300 {
            let mut b = good.clone();
            for _ in 0..4 {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let at = seed as usize % b.len();
                b[at] = (seed >> 8) as u8;
            }
            try_one(b);
        }
    }
    // Enormous counts and sizes are refused or clipped to the record, quickly.
    let t = std::time::Instant::now();
    let mut b = Vec::new();
    emf_rec(&mut b, 86, &le(&[0, 0, 1, 1, i32::MAX]));
    emf_rec(&mut b, 91, &le(&[0, 0, 1, 1, i32::MAX, i32::MAX, 5]));
    emf_rec(
        &mut b,
        81,
        &le(&[0, 0, 1, 1, 0, 0, 0, 0, 1 << 30, 1 << 30, 80, 40, 120, -1]),
    );
    emf_rec(&mut b, 9, &le(&[0, 0]));
    emf_rec(&mut b, 17, &le(&[8]));
    emf_rec(&mut b, 43, &le(&[0, 0, 10, 10]));
    let s = open(Kind::Emf, emf(&b)).unwrap();
    let _ = s.decode(&DecodeRequest::full(1), &CancelToken::new());
    assert!(t.elapsed() < std::time::Duration::from_secs(5));
    // Not metafiles.
    assert!(open(Kind::Emf, b"not an emf at all".to_vec()).is_err());
    assert!(open(Kind::Wmf, vec![1, 0, 9, 0, 0, 3]).is_err());
}

fn gzip(b: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(b).unwrap();
    e.finish().unwrap()
}

/// `.wmz` and `.emz`: the same pictures as their uncompressed metafiles; the wrong kind or
/// damaged gzip refused with the reason.
#[test]
fn compressed_metafiles() {
    let open_named = |kind: Kind, name: &str, bytes: Vec<u8>| {
        MetafileDecoder(kind).open(Arc::new(Source::from_vec(name, bytes)))
    };
    for (kind, plain, name) in [(Kind::Wmf, wmf(), "t.wmz"), (Kind::Emf, emf_gdi(), "t.emz")] {
        let a = open(kind, plain.clone()).unwrap();
        let b = open_named(kind, name, gzip(&plain)).unwrap();
        assert!(
            b.info().coding.starts_with("gzip-compressed"),
            "{}",
            b.info().coding
        );
        assert_eq!(
            (a.info().width, a.info().height),
            (b.info().width, b.info().height)
        );
        let (ra, rb) = (render(a.as_ref(), 4), render(b.as_ref(), 4));
        assert_eq!(ra.planes[0].data, rb.planes[0].data);
    }
    match open_named(Kind::Wmf, "t.wmz", gzip(&emf_gdi())) {
        Err(DecodeError::Unsupported(s)) => assert!(s.contains(".emz"), "{s}"),
        r => panic!("{:?}", r.map(|s| s.info().clone())),
    }
    match open_named(Kind::Emf, "t.emz", gzip(b"hello")) {
        Err(DecodeError::Unsupported(s)) => assert!(s.contains("does not hold")),
        r => panic!("{:?}", r.map(|s| s.info().clone())),
    }
    let mut bad = gzip(&wmf());
    bad.truncate(bad.len() / 2);
    assert!(matches!(
        open_named(Kind::Wmf, "t.wmz", bad),
        Err(DecodeError::Malformed(_))
    ));
    // Through the registry, by extension.
    let r = crate::Registry::new()
        .open(Arc::new(Source::from_vec("x.emz", gzip(&emf_gdi()))))
        .unwrap();
    assert_eq!(r.info().format, Format::Emf);
}
