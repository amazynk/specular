//! DNL (T.81 B.2.5): a frame whose SOF says height 0 takes its line count from the DNL segment
//! after the first scan. Each case rewrites a file into that form (SOF height 0, DNL inserted
//! where the first scan's data ends) and must decode exactly like the original on every path.

use std::path::PathBuf;

use specular_jpeg::*;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
    )
    .unwrap()
}

/// End of the entropy-coded data that starts at `p` (the next marker other than RSTn).
fn scan_end(d: &[u8], mut p: usize) -> usize {
    while p + 1 < d.len() {
        if d[p] == 0xFF && d[p + 1] != 0 && !(0xD0..=0xD7).contains(&d[p + 1]) && d[p + 1] != 0xFF {
            return p;
        }
        p += 1;
    }
    d.len()
}

/// `jpg` with the SOF height set to 0 and a DNL segment after the first scan.
fn with_dnl(jpg: &[u8]) -> Vec<u8> {
    let mut out = jpg.to_vec();
    let mut p = 2;
    let mut height = 0u16;
    loop {
        assert_eq!(out[p], 0xFF);
        let m = out[p + 1];
        let len = u16::from_be_bytes([out[p + 2], out[p + 3]]) as usize;
        if (0xC0..=0xCF).contains(&m) && !matches!(m, 0xC4 | 0xC8 | 0xCC) {
            height = u16::from_be_bytes([out[p + 5], out[p + 6]]);
            out[p + 5] = 0;
            out[p + 6] = 0;
        }
        if m == 0xDA {
            let end = scan_end(&out, p + 2 + len);
            let mut dnl = vec![0xFF, 0xDC, 0, 4];
            dnl.extend_from_slice(&height.to_be_bytes());
            out.splice(end..end, dnl);
            return out;
        }
        p += 2 + len;
    }
}

fn check(name: &str, jpg: &[u8]) {
    let dnl = with_dnl(jpg);
    let a = read_info(jpg).unwrap();
    let b = read_info(&dnl).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!((a.width, a.height), (b.width, b.height), "{name}");
    for scale in [Scale::Full, Scale::Half, Scale::Eighth] {
        let opts = DecodeOptions {
            scale,
            ..Default::default()
        };
        let want = decode(jpg, &opts).unwrap();
        assert_eq!(decode(&dnl, &opts).unwrap(), want, "{name} {scale:?}");
    }
    let opts = DecodeOptions {
        region: Some(Region {
            x: 3,
            y: a.height / 2,
            w: a.width / 2,
            h: a.height / 3 + 1,
        }),
        ..Default::default()
    };
    let want = decode(jpg, &opts).unwrap();
    if let Ok(ix) = build_index(&dnl) {
        assert_eq!(
            decode_with_index(&dnl, &ix, &opts).unwrap(),
            want,
            "{name} index"
        );
    }
    if let Ok((w, h, c)) = read_coefficients(jpg) {
        assert_eq!(
            read_coefficients(&dnl).unwrap(),
            (w, h, c),
            "{name} coefficients"
        );
    }
}

#[test]
fn dnl_height_decodes_like_the_sof_height() {
    for f in [
        "gray8.jpg",
        "huff_dac_420.jpg",
        "arith_420.jpg",
        "arith_420_prog.jpg",
        "arith_444_rst.jpg",
        "arith_444_prog_rst.jpg",
    ] {
        check(f, &fixture(f));
    }
    // Huffman progressive and baseline with restart markers.
    let (w, h) = (75u16, 41u16);
    let px: Vec<u8> = (0..w as usize * h as usize * 3)
        .map(|i| (i * 7 % 251) as u8)
        .collect();
    for progressive in [false, true] {
        let mut jpg = Vec::new();
        let mut e = jpeg_encoder::Encoder::new(&mut jpg, 85);
        e.set_progressive(progressive);
        e.set_restart_interval(3);
        e.encode(&px, w, h, jpeg_encoder::ColorType::Rgb).unwrap();
        check(&format!("jpeg-encoder progressive={progressive}"), &jpg);
    }
}

#[test]
fn height_zero_without_dnl_is_malformed() {
    let mut jpg = fixture("gray8.jpg");
    let p = (2..jpg.len())
        .find(|&i| jpg[i - 1] == 0xFF && jpg[i] == 0xC0)
        .unwrap()
        - 1;
    jpg[p + 5] = 0;
    jpg[p + 6] = 0;
    assert!(matches!(read_info(&jpg), Err(JpegError::Malformed(_))));
}
