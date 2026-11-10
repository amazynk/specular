//! QuickDraw vector opcodes and 32-bit alpha on built pictures, checked at pixels away from
//! antialiased edges against Inside Macintosh's drawing model (frames inside the shape, the
//! pen hanging below and right of a line, patterns aligned to the local origin). The real
//! files are compared with TwelveMonkeys in tests/twelvemonkeys.rs.

use specular_pict::decode;

fn be16(v: &mut Vec<u8>, x: u16) {
    v.extend(x.to_be_bytes());
}

fn rect(v: &mut Vec<u8>, t: i16, l: i16, b: i16, r: i16) {
    for x in [t, l, b, r] {
        v.extend(x.to_be_bytes());
    }
}

/// A version 2 picture `w`×`h` playing `ops` (each starting with its 16-bit opcode).
fn v2(w: i16, h: i16, ops: &[Vec<u8>]) -> Vec<u8> {
    let mut v = Vec::new();
    be16(&mut v, 0);
    rect(&mut v, 0, 0, h, w);
    v.extend([0x00, 0x11, 0x02, 0xff, 0x0c, 0x00]);
    be16(&mut v, 0xfffe);
    be16(&mut v, 0);
    v.extend(0x0048_0000u32.to_be_bytes());
    v.extend(0x0048_0000u32.to_be_bytes());
    rect(&mut v, 0, 0, h, w);
    v.extend([0; 4]);
    for op in ops {
        v.extend(op);
        if v.len() % 2 == 1 {
            v.push(0);
        }
    }
    be16(&mut v, 0x00ff);
    v
}

fn op(code: u16, data: &[i16]) -> Vec<u8> {
    let mut v = Vec::new();
    be16(&mut v, code);
    for &d in data {
        v.extend(d.to_be_bytes());
    }
    v
}

fn rgb_fg(r: u16, g: u16, b: u16) -> Vec<u8> {
    let mut v = Vec::new();
    for x in [0x1a, r, g, b] {
        be16(&mut v, x);
    }
    v
}

fn px(img: &specular_pict::Image, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * img.width + x) * 4) as usize;
    img.pixels[i..i + 4].try_into().unwrap()
}

const RED: [u8; 4] = [255, 0, 0, 255];
const WHITE: [u8; 4] = [255; 4];
const BLACK: [u8; 4] = [0, 0, 0, 255];

#[test]
fn shapes_lines_patterns_invert() {
    let mut fill_pat = Vec::new();
    be16(&mut fill_pat, 0x0a);
    fill_pat.extend([0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55]);
    let ops = [
        rgb_fg(0xffff, 0, 0),
        // PnSize 3×3, FrameRect: a 3-pixel outline inside (2, 2)–(20, 20).
        op(0x07, &[3, 3]),
        op(0x30, &[2, 2, 20, 20]),
        // PaintOval in (22, 22)–(38, 38).
        op(0x51, &[22, 22, 38, 38]),
        // Line from (h 0, v 30) to (h 20, v 30): the 3×3 pen below and right of it.
        op(0x20, &[30, 0, 30, 20]),
        // FillPat checkerboard, FillRect (h 0–8, v 22–28).
        fill_pat,
        op(0x34, &[22, 0, 28, 8]),
        // InvertRect over white (h 30–40, v 0–10) and over the frame (h 0–4, v 0–4).
        op(0x33, &[0, 30, 10, 40]),
        // A same-rect PaintRect after setting a new rect: (h 24–28, v 2–6).
        op(0x31, &[2, 24, 6, 28]),
        op(0x07, &[1, 1]),
        op(0x38, &[]),
    ];
    let img = decode(&v2(40, 40, &ops)).unwrap();
    assert_eq!((img.width, img.height), (40, 40));
    assert!(img.shapes >= 7, "{}", img.shapes);
    // Frame: inside the outline, its interior and outside.
    assert_eq!(px(&img, 3, 10), RED);
    assert_eq!(px(&img, 18, 10), RED);
    assert_eq!(px(&img, 10, 10), WHITE);
    assert_eq!(px(&img, 1, 10), WHITE);
    assert_eq!(px(&img, 20, 10), WHITE);
    // Oval: centre and outside corner.
    assert_eq!(px(&img, 30, 30), RED);
    assert_eq!(px(&img, 22, 22), WHITE);
    // Line: rows 30–32, columns 0–22.
    assert_eq!(px(&img, 10, 31), RED);
    assert_eq!(px(&img, 10, 29), WHITE);
    assert_eq!(px(&img, 10, 33), WHITE);
    // Inverted white.
    assert_eq!(px(&img, 35, 5), BLACK);
    // The pattern: bits set at even x on even rows.
    let (a, b) = (px(&img, 2, 24), px(&img, 3, 24));
    assert!(a != b, "{a:?} {b:?}");
    assert!([a, b].contains(&RED), "{a:?} {b:?}");
    // PaintRect, then its frame in the same place.
    assert_eq!(px(&img, 25, 3), RED);
}

/// A 32-bit DirectBitsRect with alpha (four components, pack type 4) drawn first: the canvas
/// is transparent elsewhere and the pixels keep their alpha.
#[test]
fn direct_pixels_with_alpha() {
    let (w, h) = (6i16, 2i16);
    let mut v = Vec::new();
    be16(&mut v, 0x9a);
    v.extend([0, 0, 0, 0xff]); // baseAddr
    be16(&mut v, 0x8000 | (w as u16 * 4));
    rect(&mut v, 0, 0, h, w);
    // version, packType 4, packSize, hRes, vRes, pixelType 16, pixelSize 32, cmpCount 4,
    // cmpSize 8, planeBytes, pmTable, reserved.
    be16(&mut v, 0);
    be16(&mut v, 4);
    v.extend([0; 4]);
    v.extend(0x0048_0000u32.to_be_bytes());
    v.extend(0x0048_0000u32.to_be_bytes());
    be16(&mut v, 16);
    be16(&mut v, 32);
    be16(&mut v, 4);
    be16(&mut v, 8);
    v.extend([0; 12]);
    rect(&mut v, 0, 0, h, w);
    rect(&mut v, 0, 0, h, w);
    be16(&mut v, 0);
    for y in 0..h as u8 {
        // Planes A, R, G, B; rows under 8 bytes are not packed... these are 24 bytes: one
        // literal run each.
        let mut row = Vec::new();
        for c in 0..4u8 {
            for x in 0..w as u8 {
                row.push(match c {
                    0 => x * 51,
                    1 => 200,
                    2 => 10 * y,
                    _ => x,
                });
            }
        }
        v.push(row.len() as u8 + 1);
        v.push(row.len() as u8 - 1);
        v.extend(row);
    }
    let img = decode(&v2(8, 2, &[v])).unwrap();
    assert!(img.alpha);
    assert_eq!(px(&img, 5, 1), [200, 10, 5, 255]);
    assert_eq!(px(&img, 2, 0)[3], 102);
    // Outside the pixel map: transparent.
    assert_eq!(px(&img, 7, 0)[3], 0);
}
