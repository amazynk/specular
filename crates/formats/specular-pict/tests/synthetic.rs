//! Hand-built pictures covering each bitmap opcode, pixel layout and pack type, regions,
//! scaling, QuickTime fallbacks, opcode skipping, and truncated or garbage input.

use specular_pict::{Error, decode, info, plausible, probe, probe_file, sole_jpeg};

fn be16(v: &mut Vec<u8>, x: u16) {
    v.extend(x.to_be_bytes());
}

fn rect(v: &mut Vec<u8>, t: i16, l: i16, b: i16, r: i16) {
    for x in [t, l, b, r] {
        v.extend(x.to_be_bytes());
    }
}

/// A version 2 picture of `w`×`h` with the extended header, drawing `ops` (each padded to
/// a word boundary), optionally behind the 512-byte file header.
fn v2(w: i16, h: i16, ops: &[Vec<u8>], header: bool) -> Vec<u8> {
    let mut v = if header { vec![0; 512] } else { Vec::new() };
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

/// PixMap fields after rowBytes and bounds (36 bytes).
fn pixmap_fields(v: &mut Vec<u8>, pack_type: u16, pixel_size: u16, cmp_count: u16) {
    be16(v, 0); // pmVersion
    be16(v, pack_type);
    v.extend([0; 4]); // packSize
    v.extend(0x0048_0000u32.to_be_bytes());
    v.extend(0x0048_0000u32.to_be_bytes());
    be16(v, if pixel_size >= 16 { 16 } else { 0 }); // pixelType
    be16(v, pixel_size);
    be16(v, cmp_count);
    be16(
        v,
        if pixel_size == 16 {
            5
        } else {
            pixel_size.min(8)
        },
    );
    v.extend([0; 12]); // planeBytes, pmTable, reserved
}

fn ctab(v: &mut Vec<u8>, colours: &[[u8; 3]]) {
    v.extend([0; 4]);
    be16(v, 0);
    be16(v, colours.len() as u16 - 1);
    for (i, c) in colours.iter().enumerate() {
        be16(v, i as u16);
        for &x in c {
            v.extend([x, x]);
        }
    }
}

/// PackBits encoding of `row` in `unit`-byte units (runs where the data repeats).
fn packbits(row: &[u8], unit: usize) -> Vec<u8> {
    let units: Vec<&[u8]> = row.chunks(unit).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < units.len() {
        let mut n = 1;
        while i + n < units.len() && units[i + n] == units[i] && n < 128 {
            n += 1;
        }
        if n >= 2 {
            out.push((257 - n) as u8);
            out.extend(units[i]);
            i += n;
        } else {
            let s = i;
            while i < units.len()
                && i - s < 128
                && !(i + 1 < units.len() && units[i + 1] == units[i])
            {
                i += 1;
            }
            out.push((i - s - 1) as u8);
            for u in &units[s..i] {
                out.extend(*u);
            }
        }
    }
    out
}

/// Rows of a packed bitmap opcode: each PackBits row behind its byte or word count.
fn packed_rows(v: &mut Vec<u8>, rows: &[Vec<u8>], row_bytes: usize, unit: usize) {
    for row in rows {
        let p = packbits(row, unit);
        if row_bytes > 250 {
            be16(v, p.len() as u16);
        } else {
            v.push(p.len() as u8);
        }
        v.extend(p);
    }
}

fn tail(v: &mut Vec<u8>, w: i16, h: i16, dst: (i16, i16, i16, i16)) {
    rect(v, 0, 0, h, w);
    rect(v, dst.0, dst.1, dst.2, dst.3);
    be16(v, 0); // srcCopy
}

const PAL: [[u8; 3]; 4] = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [10, 20, 30]];

/// An 8-bit PackBitsRect (or PackBitsRgn with `rgn`) of `w`×`h`, pixel (x, y) = index
/// (x + y) % 4, drawn at `dst`.
fn indexed_op(w: i16, h: i16, dst: (i16, i16, i16, i16), rgn: Option<&[u8]>) -> Vec<u8> {
    let mut v = Vec::new();
    be16(&mut v, if rgn.is_some() { 0x99 } else { 0x98 });
    let rb = (w as usize).max(8).next_multiple_of(2);
    be16(&mut v, 0x8000 | rb as u16);
    rect(&mut v, 0, 0, h, w);
    pixmap_fields(&mut v, 0, 8, 1);
    ctab(&mut v, &PAL);
    tail(&mut v, w, h, dst);
    if let Some(r) = rgn {
        v.extend(r);
    }
    let rows: Vec<Vec<u8>> = (0..h as usize)
        .map(|y| (0..rb).map(|x| ((x + y) % 4) as u8).collect())
        .collect();
    packed_rows(&mut v, &rows, rb, 1);
    v
}

fn px(img: &specular_pict::Image, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * img.width + x) * 4) as usize;
    [img.pixels[i], img.pixels[i + 1], img.pixels[i + 2]]
}

#[test]
fn indexed_packbits_with_and_without_header() {
    for header in [false, true] {
        let pict = v2(12, 5, &[indexed_op(12, 5, (0, 0, 5, 12), None)], header);
        assert_eq!(probe(&pict), !header);
        assert!(probe_file(&pict) && plausible(&pict));
        let i = info(&pict).unwrap();
        assert_eq!(
            (i.width, i.height, i.version, i.has_header),
            (12, 5, 2, header)
        );
        let img = decode(&pict).unwrap();
        assert!(!img.truncated);
        assert_eq!(img.bitmaps, 1);
        for y in 0..5 {
            for x in 0..12 {
                assert_eq!(px(&img, x, y), PAL[((x + y) % 4) as usize], "({x}, {y})");
            }
        }
        assert!(img.pixels.chunks_exact(4).all(|p| p[3] == 255));
    }
}

#[test]
fn low_depths_and_v1_bitmap() {
    // Version 1: byte opcodes, BitsRect of a 1-bit BitMap (black on white).
    let mut v = Vec::new();
    be16(&mut v, 0);
    rect(&mut v, 0, 0, 2, 10);
    v.extend([0x11, 0x01, 0x90]);
    be16(&mut v, 2); // rowBytes, BitMap
    rect(&mut v, 0, 0, 2, 10);
    tail(&mut v, 10, 2, (0, 0, 2, 10));
    v.extend([0b1010_0000, 0b0100_0000, 0b0000_0000, 0b1100_0000]);
    v.push(0xff);
    let img = decode(&v).unwrap();
    assert_eq!(info(&v).unwrap().version, 1);
    let row0: Vec<_> = (0..10).map(|x| px(&img, x, 0)[0]).collect();
    assert_eq!(row0, [0, 255, 0, 255, 255, 255, 255, 255, 255, 0]);
    let row1: Vec<_> = (0..10).map(|x| px(&img, x, 1)[0]).collect();
    assert_eq!(row1, [255, 255, 255, 255, 255, 255, 255, 255, 0, 0]);

    // 2- and 4-bit PixMaps, unpacked (rowBytes < 8).
    for bits in [2u16, 4] {
        let mut op = Vec::new();
        be16(&mut op, 0x98);
        be16(&mut op, 0x8000 | 4);
        rect(&mut op, 0, 0, 1, 4);
        pixmap_fields(&mut op, 0, bits, 1);
        ctab(&mut op, &PAL);
        tail(&mut op, 4, 1, (0, 0, 1, 4));
        op.extend(if bits == 2 {
            [0b00_01_10_11, 0, 0, 0]
        } else {
            [0x01, 0x23, 0, 0]
        });
        let img = decode(&v2(4, 1, &[op], false)).unwrap();
        for x in 0..4 {
            assert_eq!(px(&img, x, 0), PAL[x as usize], "{bits}-bit x={x}");
        }
    }
}

/// A DirectBitsRect of `w`×1 with the given pixel layout and row bytes as written.
fn direct_op(w: i16, pack_type: u16, pixel_size: u16, cmp: u16, rb: u16, row: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    be16(&mut v, 0x9a);
    v.extend([0, 0, 0, 0xff]); // baseAddr
    be16(&mut v, 0x8000 | rb);
    rect(&mut v, 0, 0, 1, w);
    pixmap_fields(&mut v, pack_type, pixel_size, cmp);
    tail(&mut v, w, 1, (0, 0, 1, w));
    v.extend(row);
    v
}

#[test]
fn direct_pixels_all_pack_types() {
    let w = 10i16;
    let cols: Vec<[u8; 3]> = (0..w as u8)
        .map(|i| [i * 20, 200 - i * 9, i ^ 0x5a])
        .collect();
    // 32-bit, pack type 4: PackBits of the planes (alpha, red, green, blue).
    let mut planes = vec![0u8; w as usize];
    for c in 0..3 {
        planes.extend(cols.iter().map(|p| p[c]));
    }
    let p = packbits(&planes, 1);
    let mut row = vec![p.len() as u8];
    row.extend(p);
    let t4 = direct_op(w, 4, 32, 4, 40, &row);
    // 32-bit, pack type 4 with three planes.
    let p = packbits(&planes[w as usize..], 1);
    let mut row = vec![p.len() as u8];
    row.extend(p);
    let t4c3 = direct_op(w, 4, 32, 3, 40, &row);
    // Pack type 2: unpacked RGB triples.
    let t2 = direct_op(w, 2, 32, 3, 40, &cols.concat());
    // Pack type 1: unpacked xRGB.
    let xrgb: Vec<u8> = cols.iter().flat_map(|c| [0, c[0], c[1], c[2]]).collect();
    let t1 = direct_op(w, 1, 32, 3, 40, &xrgb);
    for (name, op) in [("t4", t4), ("t4c3", t4c3), ("t2", t2), ("t1", t1)] {
        let img = decode(&v2(w, 1, &[op], false)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!img.truncated, "{name}");
        for x in 0..w as u32 {
            assert_eq!(px(&img, x, 0), cols[x as usize], "{name} x={x}");
        }
    }

    // 16-bit, pack type 3: PackBits in 2-byte units, 5-bit channels widened by replication.
    let words: [u16; 10] = [
        0x7fff, 0x7fff, 0x7fff, 0x7c00, 0x03e0, 0x001f, 0x4210, 0, 0, 0,
    ];
    let raw: Vec<u8> = words.iter().flat_map(|w| w.to_be_bytes()).collect();
    let p = packbits(&raw, 2);
    let mut row = vec![p.len() as u8];
    row.extend(p);
    let img = decode(&v2(10, 1, &[direct_op(10, 3, 16, 3, 20, &row)], false)).unwrap();
    let want = [
        [255, 255, 255],
        [255, 255, 255],
        [255, 255, 255],
        [255, 0, 0],
        [0, 255, 0],
        [0, 0, 255],
        [132, 132, 132],
        [0, 0, 0],
    ];
    for (x, c) in want.iter().enumerate() {
        assert_eq!(px(&img, x as u32, 0), *c, "16-bit x={x}");
    }
}

#[test]
fn word_counts_for_wide_rows() {
    // rowBytes > 250: counts are words.
    let w = 300i16;
    let pict = v2(w, 3, &[indexed_op(w, 3, (0, 0, 3, w), None)], false);
    let img = decode(&pict).unwrap();
    assert!(!img.truncated);
    assert_eq!(px(&img, 299, 2), PAL[(299 + 2) % 4]);
}

#[test]
fn region_mask_scaling_and_placement() {
    // A rectangular region (size 10: bounding box only) over columns 2..5, rows 1..3.
    let mut rgn = Vec::new();
    be16(&mut rgn, 10);
    rect(&mut rgn, 1, 2, 3, 5);
    let img = decode(&v2(
        8,
        4,
        &[indexed_op(8, 4, (0, 0, 4, 8), Some(&rgn))],
        false,
    ))
    .unwrap();
    for y in 0..4 {
        for x in 0..8 {
            let inside = (1..3).contains(&y) && (2..5).contains(&x);
            let want = if inside {
                PAL[((x + y) % 4) as usize]
            } else {
                [255; 3]
            };
            assert_eq!(px(&img, x, y), want, "({x}, {y})");
        }
    }

    // A non-rectangular region: rows 0..2 cover x 0..2, row 2 onwards x 0..4.
    let mut rgn = Vec::new();
    let data: [i16; 9] = [0, 0, 2, 0x7fff, 2, 2, 4, 0x7fff, 0x7fff];
    be16(&mut rgn, 10 + 2 * data.len() as u16);
    rect(&mut rgn, 0, 0, 4, 4);
    for d in data {
        rgn.extend(d.to_be_bytes());
    }
    let img = decode(&v2(
        8,
        4,
        &[indexed_op(8, 4, (0, 0, 4, 8), Some(&rgn))],
        false,
    ))
    .unwrap();
    for y in 0..4u32 {
        let lit: Vec<bool> = (0..8).map(|x| px(&img, x, y) != [255; 3]).collect();
        let n = if y < 2 { 2 } else { 4 };
        assert_eq!(lit, (0..8).map(|x| x < n).collect::<Vec<_>>(), "row {y}");
    }

    // 2× scaling into the middle of a larger canvas, nearest neighbour.
    let img = decode(&v2(12, 12, &[indexed_op(4, 2, (4, 2, 8, 10), None)], false)).unwrap();
    assert_eq!(px(&img, 0, 0), [255; 3]);
    for y in 4..8 {
        for x in 2..10 {
            let (sx, sy) = ((x - 2) / 2, (y - 4) / 2);
            assert_eq!(px(&img, x, y), PAL[((sx + sy) % 4) as usize], "({x}, {y})");
        }
    }
    assert_eq!(px(&img, 10, 5), [255; 3]);
}

#[test]
fn skips_unknown_opcodes_and_draws_vectors() {
    // LongComment with data, a reserved 0x0100-range op (2 words), a long-data op, a clip
    // region, PaintRect, and DHText.
    let ops = [
        vec![0x00, 0xa1, 0x00, 0x64, 0x00, 0x03, 1, 2, 3],
        vec![0x01, 0x00, 9, 9],
        vec![0x81, 0x00, 0, 0, 0, 3, 7, 7, 7],
        vec![0x00, 0x01, 0x00, 0x0a, 0, 0, 0, 0, 0, 4, 0, 4],
        vec![0x00, 0x31, 0, 0, 0, 0, 0, 2, 0, 2],
        vec![0x00, 0x29, 3, 2, b'h', b'i'],
        indexed_op(4, 4, (0, 0, 4, 4), None),
    ];
    let img = decode(&v2(4, 4, &ops, false)).unwrap();
    assert_eq!(img.bitmaps, 1);
    assert_eq!(px(&img, 1, 2), PAL[3]);

    // A bitmap drawn into an empty rectangle: a blank canvas of the full size (found by
    // fuzzing: the canvas was never allocated).
    let img = decode(&v2(4, 4, &[indexed_op(4, 4, (0, 0, 0, 0), None)], false)).unwrap();
    assert_eq!(img.pixels, vec![255; 4 * 4 * 4]);

    // Without the bitmap: the black 2×2 PaintRect (and the text) on white.
    let vector = v2(4, 4, &ops[..ops.len() - 1], false);
    let img = decode(&vector).unwrap();
    assert_eq!((img.bitmaps, img.shapes), (0, 2));
    assert_eq!(px(&img, 0, 0), [0, 0, 0]);
    assert_eq!(px(&img, 1, 1), [0, 0, 0]);
    assert_eq!(px(&img, 3, 3), [255, 255, 255]);
    assert!(sole_jpeg(&vector).is_none());

    // Only comments: nothing to show.
    let e = decode(&v2(4, 4, &ops[..3], false)).unwrap_err();
    assert!(
        matches!(&e, Error::Unsupported(m) if m.contains("draws nothing")),
        "{e}"
    );
}

/// A CompressedQuickTime op for a `codec` image of `w`×`h` at the origin, unit matrix.
fn quicktime(codec: &[u8; 4], w: i16, h: i16, payload: &[u8]) -> Vec<u8> {
    let mut b = vec![0u8; 68];
    for (at, v) in [(2, 0x0001_0000u32), (18, 0x0001_0000), (34, 0x4000_0000)] {
        b[at..at + 4].copy_from_slice(&v.to_be_bytes());
    }
    let mut desc = vec![0u8; 86];
    desc[0..4].copy_from_slice(&86u32.to_be_bytes());
    desc[4..8].copy_from_slice(codec);
    desc[32..34].copy_from_slice(&w.to_be_bytes());
    desc[34..36].copy_from_slice(&h.to_be_bytes());
    desc[44..48].copy_from_slice(&(payload.len() as u32).to_be_bytes());
    desc[82..84].copy_from_slice(&24i16.to_be_bytes());
    b.extend(desc);
    b.extend(payload);
    let mut v = vec![0x82, 0x00];
    v.extend((b.len() as u32).to_be_bytes());
    v.extend(b);
    v
}

#[test]
fn quicktime_fallback_is_not_drawn() {
    // QuickTime writes its own placeholder (a bitmap, or "QuickTime … needed" text) after the
    // compressed data for readers without QuickTime; it is not the picture.
    let ops = [
        quicktime(b"SVQ1", 4, 4, &[0; 32]),
        indexed_op(4, 4, (0, 0, 4, 4), None),
        vec![0x00, 0x31, 0, 0, 0, 0, 0, 2, 0, 2],
    ];
    let pict = v2(4, 4, &ops, false);
    let e = decode(&pict).unwrap_err();
    assert!(
        matches!(&e, Error::Unsupported(m) if m.contains("'SVQ1'")),
        "{e}"
    );
    assert!(sole_jpeg(&pict).is_none());

    // A bitmap before the QuickTime op draws.
    let ops = [
        indexed_op(4, 4, (0, 0, 4, 4), None),
        quicktime(b"SVQ1", 4, 4, &[0; 32]),
        indexed_op(4, 4, (0, 0, 4, 4), None),
    ];
    let img = decode(&v2(4, 4, &ops, false)).unwrap();
    assert_eq!(img.bitmaps, 1);
    assert_eq!(px(&img, 3, 3), PAL[2]);

    // A known codec whose data does not decode.
    let ops = [quicktime(b"tiff", 4, 4, &[0; 32])];
    assert!(matches!(
        decode(&v2(4, 4, &ops, false)),
        Err(Error::Malformed(_))
    ));

    // QuickTime 'raw ' (24-bit RGB rows) draws, its fallback text does not.
    let rgb: Vec<u8> = (0..16u8).flat_map(|i| [i * 16, 255 - i * 16, 7]).collect();
    let ops = [
        quicktime(b"raw ", 4, 4, &rgb),
        vec![0x00, 0x31, 0, 0, 0, 0, 0, 2, 0, 2],
    ];
    let pict = v2(4, 4, &ops, false);
    let img = decode(&pict).unwrap();
    assert_eq!((img.quicktime, img.shapes), (1, 0));
    assert_eq!(px(&img, 1, 0), [16, 239, 7]);
    assert_eq!(px(&img, 0, 0), [0, 255, 7]);

    // A corrupt 'jpeg' payload with nothing else drawn.
    let ops = [quicktime(b"jpeg", 4, 4, &[0xff, 0xd8, 0xff, 0xdb, 0, 3])];
    let pict = v2(4, 4, &ops, false);
    assert!(matches!(decode(&pict), Err(Error::Malformed(_))));
    assert!(sole_jpeg(&pict).is_none());
}

#[test]
fn truncation_garbage_and_limits() {
    let pict = v2(
        40,
        30,
        &[
            indexed_op(40, 30, (0, 0, 30, 40), None),
            direct_op(4, 1, 32, 3, 16, &[0; 16]),
        ],
        true,
    );
    let first_end = (512 + 40 + indexed_op(40, 30, (0, 0, 30, 40), None).len()).next_multiple_of(2);
    let full = decode(&pict).unwrap();
    assert!(!full.truncated);
    // Every prefix: an error, or the rows that arrived with the rest white and flagged.
    for n in 0..pict.len() {
        match decode(&pict[..n]) {
            Ok(img) => {
                assert_eq!((img.width, img.height), (40, 30));
                assert_eq!(img.pixels.len(), 40 * 30 * 4);
                // Only a cut between opcodes goes unflagged.
                assert!(
                    img.truncated || n == first_end || n == pict.len() - 2,
                    "prefix {n} of {}, op ends {first_end}",
                    pict.len()
                );
            }
            Err(e) => assert!(!matches!(e, Error::TooLarge), "prefix {n}: {e}"),
        }
        let _ = info(&pict[..n]);
        let _ = sole_jpeg(&pict[..n]);
    }
    // Half of the pixel rows: the top half matches.
    let img = decode(&pict[..512 + 40 + 20 + 1100]).unwrap();
    assert!(img.truncated);
    assert_eq!(px(&img, 5, 2), px(&full, 5, 2));
    assert_eq!(px(&img, 5, 29), [255; 3]);

    // Garbage after the header, and byte flips through the whole file.
    let mut x = 0x2545_f491_4f6c_dd1du64;
    let mut rnd = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    for _ in 0..300 {
        let mut d = pict.clone();
        for _ in 0..1 + rnd() % 8 {
            let at = (rnd() as usize) % d.len();
            d[at] = rnd() as u8;
        }
        let _ = decode(&d);
        let _ = sole_jpeg(&d);
        let mut g = pict[..512 + 40].to_vec();
        g.extend((0..rnd() % 400).map(|_| rnd() as u8));
        let _ = decode(&g);
    }

    // Not a picture; an enormous frame.
    assert!(matches!(decode(&[0; 600]), Err(Error::NotPict)));
    let mut big = v2(30000, 30000, &[], false);
    big[2..10].copy_from_slice(&[0, 0, 0, 0, 0x7f, 0xff, 0x7f, 0xff]);
    big[28..36].copy_from_slice(&[0, 0, 0, 0, 0x7f, 0xff, 0x7f, 0xff]);
    assert!(matches!(decode(&big), Err(Error::TooLarge)));
    assert!(matches!(info(&big), Err(Error::TooLarge)));
    // A huge destination rectangle on a small canvas stays within the work budget.
    let pict = v2(
        8,
        8,
        &[indexed_op(8, 8, (-32000, -32000, 32000, 32000), None)],
        false,
    );
    let _ = decode(&pict);
}

#[test]
fn scaled_blits_clipped_by_the_canvas() {
    // A 4×4 source scaled to 12×18 hanging over the top and left of an 8×8 canvas, and one
    // wholly below it.
    let pict = v2(
        8,
        8,
        &[
            indexed_op(4, 4, (-5, -3, 13, 9), None),
            indexed_op(4, 4, (8, 0, 20, 8), None),
        ],
        false,
    );
    let img = decode(&pict).unwrap();
    assert!(!img.truncated);
    assert_eq!(img.bitmaps, 2);
    for y in 0..8 {
        for x in 0..8 {
            let (sx, sy) = ((x + 3) * 4 / 12, (y + 5) * 4 / 18);
            assert_eq!(px(&img, x, y), PAL[((sx + sy) % 4) as usize], "({x}, {y})");
        }
    }
}

#[test]
fn hostile_many_off_canvas_blits() {
    // 100,000 one-pixel bitmaps (about 110 bytes each) stretched over 65,535 rows beside the
    // canvas: each walked every destination row.
    let n = 100_000;
    let op = indexed_op(1, 1, (-32768, -32768, 32767, -32767), None);
    let pict = v2(8, 8, &vec![op; n], false);
    let t = std::time::Instant::now();
    let img = decode(&pict).unwrap();
    let el = t.elapsed();
    assert!(el < std::time::Duration::from_secs(5), "{el:?}");
    assert!(!img.truncated);
    assert_eq!(img.bitmaps, n);
    assert!(img.pixels.iter().all(|&v| v == 255));
}
