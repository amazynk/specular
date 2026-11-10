use super::*;
use specular_drawpage::{Op, Paint, RenderRequest, render};

/// A WordPerfect prefix for a WPG of `major` version.
fn header(major: u8) -> Vec<u8> {
    let mut h = b"\xffWPC".to_vec();
    h.extend_from_slice(&16u32.to_le_bytes());
    h.extend_from_slice(&[1, 0x16, major, 0, 0, 0, 0, 0]);
    h
}

fn varint(out: &mut Vec<u8>, n: usize) {
    if n < 0xFF {
        out.push(n as u8);
    } else if n < 0x8000 {
        out.push(0xFF);
        out.extend_from_slice(&(n as u16).to_le_bytes());
    } else {
        out.push(0xFF);
        out.extend_from_slice(&((((n >> 16) as u16) | 0x8000).to_le_bytes()));
        out.extend_from_slice(&((n & 0xFFFF) as u16).to_le_bytes());
    }
}

fn rec1(out: &mut Vec<u8>, t: u8, body: &[u8]) {
    out.push(t);
    varint(out, body.len());
    out.extend_from_slice(body);
}

fn rec2(out: &mut Vec<u8>, t: u8, ext: usize, body: &[u8]) {
    out.push(4);
    out.push(t);
    varint(out, ext);
    varint(out, body.len());
    out.extend_from_slice(body);
}

fn le16(v: &[i32]) -> Vec<u8> {
    v.iter().flat_map(|&x| (x as i16).to_le_bytes()).collect()
}

fn draw(page: &specular_drawpage::Page, scale: f64) -> (u32, u32, Vec<u8>) {
    let w = (page.size.0 * scale).round() as u32;
    let h = (page.size.1 * scale).round() as u32;
    let req = RenderRequest {
        scale_x: scale,
        scale_y: scale,
        x: 0,
        y: 0,
        width: w,
        height: h,
    };
    (w, h, render(page, req, &|| false).unwrap())
}

fn px(img: &(u32, u32, Vec<u8>), x: u32, y: u32) -> [u8; 3] {
    let i = ((y * img.0 + x) * 4) as usize;
    [img.2[i], img.2[i + 1], img.2[i + 2]]
}

/// A 1-inch-square WPG 1: a red filled rectangle in the bottom-left quarter, a blue line,
/// a 2×2 4-bit bitmap (type 2) in the top-right quarter.
fn wpg1() -> Vec<u8> {
    let mut f = header(1);
    rec1(&mut f, 0x0F, &[1, 0, 0xB0, 0x04, 0xB0, 0x04]); // 1200 × 1200
    // Colour map: index 1 red, 2 blue (start 1, count 2).
    rec1(&mut f, 0x0E, &[1, 0, 2, 0, 255, 0, 0, 0, 0, 255]);
    rec1(&mut f, 0x01, &[1, 1]); // solid fill, colour 1
    rec1(&mut f, 0x02, &[0, 0, 0, 0]); // no outline
    rec1(&mut f, 0x07, &le16(&[0, 0, 600, 600])); // x, y (bottom left), w, h
    rec1(&mut f, 0x02, &[1, 2, 24, 0]); // solid blue, 24 units wide
    rec1(&mut f, 0x05, &le16(&[0, 1100, 1200, 1100])); // a line near the top
    // Bitmap type 2: rotation, box (x1, y1, x2, y2), 2×2, 4 bits, 72 dpi; RLE rows:
    // row 0 = colours 1, 2 (0x12), row 1 = repeat row 0 once.
    let mut bm = le16(&[0, 600, 600, 1200, 1200]);
    bm.extend_from_slice(&le16(&[2, 2, 4, 72, 72]));
    bm.extend_from_slice(&[0x01, 0x12, 0x00, 0x01]);
    rec1(&mut f, 0x14, &bm);
    rec1(&mut f, 0x11, &[0; 8]); // PostScript: not drawn
    rec1(&mut f, 0x10, &[]);
    f
}

#[test]
fn version1_shapes_and_bitmap() {
    let d = wpg1();
    assert!(probe(&d));
    let doc = open(&d).unwrap();
    assert_eq!(doc.facts.version, 1);
    assert_eq!(doc.facts.bitmaps, 1);
    assert!(doc.facts.postscript);
    assert_eq!(doc.pages[0].size, (72.0, 72.0));
    assert!(
        doc.describe(0)
            .starts_with("WordPerfect Graphics 1, 1 bitmap")
    );
    let img = draw(&doc.pages[0], 2.0); // 144 × 144
    assert_eq!((img.0, img.1), (144, 144));
    // Rectangle: page bottom-left quarter.
    assert_eq!(px(&img, 20, 120), [255, 0, 0]);
    // Line: 100 units below the top (6 pt = 12 px), blue.
    assert_eq!(px(&img, 30, 12), [0, 0, 255]);
    // Bitmap: top-right quarter, left column red, right column blue.
    assert_eq!(px(&img, 80, 40), [255, 0, 0]);
    assert_eq!(px(&img, 130, 40), [0, 0, 255]);
    // Elsewhere white.
    assert_eq!(px(&img, 20, 40), [255, 255, 255]);
}

/// A bitmap the memory budget refuses is not drawn, and the description says so (and how
/// much free RAM it needs) instead of leaving it out silently.
#[test]
fn bitmap_refused_for_memory_is_reported() {
    let d = wpg1();
    let doc = specular_core::budget::with_limit(8, || open(&d)).unwrap();
    assert_eq!((doc.facts.bitmaps, doc.facts.refused_bitmaps), (0, 1));
    assert!(doc.facts.needs_memory.is_some());
    let s = doc.describe(0);
    assert!(s.contains("1 bitmap not drawn: needs"), "{s}");
    let doc = open(&d).unwrap();
    assert_eq!(
        (doc.facts.refused_bitmaps, doc.facts.needs_memory),
        (0, None)
    );
}

/// A 2-inch WPG 1 with a polygon, an ellipse, an arc, dashed and wide lines, a curve and
/// text.
fn wpg1_shapes() -> Vec<u8> {
    let mut f = header(1);
    rec1(&mut f, 0x0F, &[1, 0, 0x60, 0x09, 0x60, 0x09]); // 2400 × 2400 = 144 pt
    rec1(&mut f, 0x01, &[1, 4]); // solid, default palette 4 (maroon)
    rec1(&mut f, 0x02, &[1, 0, 0, 0]);
    let mut poly = 3u16.to_le_bytes().to_vec();
    poly.extend_from_slice(&le16(&[0, 0, 1200, 0, 0, 1200]));
    rec1(&mut f, 0x08, &poly);
    // Ellipse at (1800, 1800), radii 300, full.
    rec1(&mut f, 0x09, &le16(&[1800, 1800, 300, 300, 0, 0, 360, 0]));
    // Curve: 4 points.
    let mut c = 0u32.to_le_bytes().to_vec();
    c.extend_from_slice(&4u16.to_le_bytes());
    c.extend_from_slice(&le16(&[1300, 100, 1500, 600, 1900, 600, 2300, 100]));
    rec1(&mut f, 0x13, &c);
    let mut t = 2u16.to_le_bytes().to_vec();
    t.extend_from_slice(&le16(&[100, 2000]));
    t.extend_from_slice(b"Hi");
    rec1(&mut f, 0x0C, &t);
    // A dashed green line, a wide one, and a pie (90° to 180°) in light blue.
    rec1(&mut f, 0x02, &[2, 2, 12, 0]);
    rec1(&mut f, 0x05, &le16(&[100, 1500, 1100, 1500]));
    rec1(&mut f, 0x02, &[1, 1, 60, 0]);
    rec1(&mut f, 0x05, &le16(&[100, 1700, 1100, 1700]));
    rec1(&mut f, 0x01, &[1, 9]);
    rec1(&mut f, 0x02, &[1, 0, 0, 0]);
    rec1(&mut f, 0x09, &le16(&[1800, 900, 400, 300, 0, 90, 180, 1]));
    rec1(&mut f, 0x10, &[]);
    f
}

#[test]
fn version1_polygon_ellipse_curve_text() {
    let doc = open(&wpg1_shapes()).unwrap();
    let ops = &doc.pages[0].ops;
    assert!(ops.iter().any(|o| matches!(o, Op::Text(_))));
    let fills = ops
        .iter()
        .filter(|o| matches!(o, Op::Fill { paint: Paint::Solid(c), .. } if *c == [127, 0, 0, 255]))
        .count();
    assert_eq!(fills, 2, "polygon and ellipse");
    assert!(
        ops.iter()
            .any(|o| matches!(o, Op::Stroke { stroke, .. } if stroke.dash_pattern.len() == 2)),
        "dashed line"
    );
    let img = draw(&doc.pages[0], 1.0);
    assert_eq!(px(&img, 108, 36), [127, 0, 0], "ellipse centre");
    assert_eq!(px(&img, 10, 134), [127, 0, 0], "triangle corner");
}

#[test]
fn version1_one_bit_bitmap_and_padding() {
    // Type 1 bitmap, 8×2 at 1200 dpi on a 1/150-inch page: one row black, one white.
    let mut f = header(1);
    rec1(&mut f, 0x0F, &[1, 0, 8, 0, 2, 0]);
    let mut bm = le16(&[8, 2, 1, 1200, 1200]);
    bm.extend_from_slice(&[0x81, 0x00, 0x80, 0x01]);
    rec1(&mut f, 0x0B, &bm);
    rec1(&mut f, 0x10, &[]);
    let doc = open(&f).unwrap();
    let img = draw(&doc.pages[0], 150.0 / 0.48); // 8 pt * ... whatever: ≥ 2 px tall
    let h = img.1;
    assert_eq!(px(&img, 1, 0), [0, 0, 0]);
    assert_eq!(px(&img, 1, h - 1), [255, 255, 255]);
    // Rows padded by one byte (a 4-bit 3-pixel row is 2 bytes, written as 3).
    let (raw, _) = bitmap::rle1(&[0x03, 1, 2, 3, 0x03, 4, 5, 6], 3, 2).unwrap();
    assert_eq!(raw, [1, 2, 3, 4, 5, 6]);
    assert_eq!(
        bitmap::rle1_stride(&[0x03, 1, 2, 3, 0x03, 4, 5, 6], 2, 2),
        3
    );
}

/// A WPG 2 picture 1200 units (1 inch) square: a group translated by (600, 0) holding a
/// filled, framed green triangle; a compound polygon (square with a square hole, even-odd);
/// a 2×1 24-bit bitmap.
fn wpg2() -> Vec<u8> {
    let mut f = header(2);
    let mut start = vec![0xB0, 0x04, 0xB0, 0x04, 0];
    start.extend_from_slice(&le16(&[0, 0, 1200, 1200, 1200, 1200]));
    rec2(&mut f, 0x01, 0, &start);
    rec2(&mut f, 0x25, 0, &[0, 0, 0, 0]); // pen black
    rec2(&mut f, 0x2B, 0, &le16(&[12, 12]));
    rec2(&mut f, 0x31, 0, &[0, 0, 200, 0, 0]); // brush green
    // Group: translate flag 0x02; fraction 0, integer 600; fraction 0, integer 0.
    let mut g = 0x0002u16.to_le_bytes().to_vec();
    g.extend_from_slice(&[0, 0]);
    g.extend_from_slice(&600i16.to_le_bytes());
    g.extend_from_slice(&[0, 0, 0, 0]);
    rec2(&mut f, 0x20, 1, &g);
    // Polyline: filled | closed | framed.
    let mut p = (0xE000u16).to_le_bytes().to_vec();
    p.extend_from_slice(&3u16.to_le_bytes());
    p.extend_from_slice(&le16(&[0, 0, 600, 0, 0, 600]));
    rec2(&mut f, 0x15, 0, &p);
    // Compound polygon: filled | closed, two members.
    rec2(&mut f, 0x31, 0, &[0, 0, 0, 255, 0]); // brush blue
    rec2(&mut f, 0x1A, 2, &0x6000u16.to_le_bytes());
    for r in [[0, 600, 600, 1200], [200, 800, 400, 1000]] {
        let mut p = 0u16.to_le_bytes().to_vec();
        p.extend_from_slice(&4u16.to_le_bytes());
        p.extend_from_slice(&le16(&[r[0], r[1], r[2], r[1], r[2], r[3], r[0], r[3]]));
        rec2(&mut f, 0x15, 0, &p);
    }
    // Bitmap in the top-right quarter: 2×1, 24-bit, run-length: red, then white run.
    let mut b = 0u16.to_le_bytes().to_vec();
    b.extend_from_slice(&le16(&[600, 600, 1200, 1200]));
    b.extend_from_slice(&le16(&[72, 72]));
    rec2(&mut f, 0x1B, 1, &b);
    let mut bd = le16(&[2, 1]);
    bd.extend_from_slice(&[8, 1]);
    bd.extend_from_slice(&[0x7D, 3, 0x80, 255, 0, 0, 0xFF, 0]);
    rec2(&mut f, 0x0E, 0, &bd);
    rec2(&mut f, 0x1C, 1, &[0; 14]); // text line: not drawn
    rec2(&mut f, 0x0F, 0, &[0; 4]);
    rec2(&mut f, 0x02, 0, &[]);
    f
}

#[test]
fn version2_groups_compound_bitmap() {
    let d = wpg2();
    assert!(probe(&d));
    let doc = open(&d).unwrap();
    assert_eq!(doc.facts.version, 2);
    assert_eq!(doc.facts.bitmaps, 1);
    assert_eq!(doc.pages[0].size, (72.0, 72.0));
    assert!(doc.facts.skipped.iter().any(|(n, _)| n == "text"));
    let img = draw(&doc.pages[0], 2.0);
    // The triangle, moved right by half the page: bottom-right quarter's corner.
    assert_eq!(px(&img, 80, 135), [0, 200, 0]);
    assert_eq!(px(&img, 20, 135), [255, 255, 255]);
    // Compound: top-left quarter blue, its centre (the hole) white.
    assert_eq!(px(&img, 6, 6), [0, 0, 255]);
    assert_eq!(px(&img, 36, 36), [255, 255, 255]);
    // Bitmap: top-right quarter, left half red, right half white.
    let red = px(&img, 88, 30);
    assert!(red[0] == 255 && red[1] < 16 && red[2] < 16, "{red:?}");
    assert_eq!(px(&img, 136, 30), [255, 255, 255]);
}

#[test]
fn version2_double_precision_and_curves() {
    let mut f = header(2);
    let mut start = vec![0xB0, 0x04, 0xB0, 0x04, 1];
    for v in [0i32, 0, 1200, 1200, 1200, 1200] {
        start.extend_from_slice(&(v << 16).to_le_bytes());
    }
    rec2(&mut f, 0x01, 0, &start);
    rec2(&mut f, 0x31, 0, &[0, 255, 0, 0, 0]);
    // Full ellipse (start point = end point), filled.
    let mut a = 0x6000u16.to_le_bytes().to_vec();
    for v in [600i32, 600, 300, 300, 900, 600, 900, 600] {
        a.extend_from_slice(&(v << 16).to_le_bytes());
    }
    rec2(&mut f, 0x19, 0, &a);
    rec2(&mut f, 0x02, 0, &[]);
    let doc = open(&f).unwrap();
    assert!(doc.facts.double_precision);
    let img = draw(&doc.pages[0], 2.0);
    assert_eq!(px(&img, 72, 72), [255, 0, 0]);
    assert_eq!(px(&img, 5, 5), [255, 255, 255]);
}

#[test]
fn refusals() {
    assert_eq!(open(b"hello").unwrap_err(), Error::NotWpg);
    let mut enc = wpg1();
    enc[12] = 0x34;
    let e = open(&enc).unwrap_err();
    assert_eq!(e, Error::Encrypted);
    assert_eq!(
        e.to_string(),
        "password-protected WordPerfect graphic: not supported"
    );
    let mut doc = wpg1();
    doc[9] = 0x0A;
    assert!(!probe(&doc));
    assert!(matches!(open(&doc), Err(Error::Unsupported(_))));
    let mut v3 = wpg1();
    v3[10] = 3;
    assert!(!probe(&v3));
    assert!(open(&v3).unwrap_err().to_string().contains("version 3"));
}

#[test]
fn hostile_inputs_do_not_panic() {
    for base in [wpg1(), wpg2()] {
        // Every truncation.
        for n in 0..base.len() {
            let _ = open(&base[..n]);
        }
        // Single-byte corruptions past the header.
        for i in 16..base.len() {
            for v in [0x00, 0x7F, 0x80, 0xFF] {
                let mut d = base.clone();
                d[i] = v;
                if let Ok(doc) = open(&d) {
                    let p = &doc.pages[0];
                    let s = (64.0 / p.size.0.max(p.size.1)).min(4.0);
                    if (p.size.0 * s) >= 1.0 && (p.size.1 * s) >= 1.0 {
                        let _ = draw(p, s);
                    }
                }
            }
        }
    }
    // Huge counts and lengths.
    let mut f = header(1);
    rec1(&mut f, 0x0F, &[1, 0, 0xB0, 0x04, 0xB0, 0x04]);
    rec1(&mut f, 0x06, &[0xFF, 0xFF, 1, 2]);
    rec1(&mut f, 0x0B, &le16(&[-1, -1, 8, 1, 1]));
    f.extend_from_slice(&[0x05, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
    let doc = open(&f).unwrap();
    assert!(doc.facts.truncated || doc.facts.limited || doc.pages[0].ops.is_empty());
}

/// Opens every file of the dev corpus (tools/corpus/fetch-design2-wpg.sh) without panicking;
/// skipped when the corpus is absent.
#[test]
fn corpus() {
    let dir = std::env::var_os("SPECULAR_WPG_CORPUS").map_or_else(
        || {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tools/corpus/design2/wpg")
        },
        std::path::PathBuf::from,
    );
    let Ok(entries) = std::fs::read_dir(&dir) else {
        eprintln!("WPG corpus absent: {}", dir.display());
        return;
    };
    let mut opened = 0;
    for e in entries.flatten() {
        let p = e.path();
        if !p.extension().is_some_and(|x| x.eq_ignore_ascii_case("wpg")) {
            continue;
        }
        let data = std::fs::read(&p).unwrap();
        match open(&data) {
            Ok(doc) => {
                opened += 1;
                assert!(!doc.pages.is_empty());
                let page = &doc.pages[0];
                let s = 96.0 / page.size.0.max(page.size.1);
                let _ = draw(page, s);
            }
            Err(e) => assert!(!probe(&data), "{}: {e}", p.display()),
        }
    }
    eprintln!("WPG corpus: {opened} files opened");
}

/// With SPECULAR_WPG_WRITE_FIXTURES=DIR, writes the synthetic files for the oracle script.
#[test]
fn write_fixtures() {
    let Some(dir) = std::env::var_os("SPECULAR_WPG_WRITE_FIXTURES") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, data) in [
        ("gen-v1-basic.wpg", wpg1()),
        ("gen-v1-shapes.wpg", wpg1_shapes()),
        ("gen-v2-groups.wpg", wpg2()),
    ] {
        std::fs::write(dir.join(name), data).unwrap();
    }
}
