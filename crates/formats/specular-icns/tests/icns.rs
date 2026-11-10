//! ICNS container and legacy icon decoding against hand-computed pixels.

use specular_icns::{Error, Kind, decode, detect, parse};

fn entry(cc: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut e = cc.to_vec();
    e.extend_from_slice(&((data.len() + 8) as u32).to_be_bytes());
    e.extend_from_slice(data);
    e
}

fn file(entries: &[Vec<u8>]) -> Vec<u8> {
    let body: Vec<u8> = entries.concat();
    let mut f = b"icns".to_vec();
    f.extend_from_slice(&((body.len() + 8) as u32).to_be_bytes());
    f.extend_from_slice(&body);
    f
}

/// Packs `channel` with the ICNS run-length scheme (runs of 3+ repeat, else literals).
fn rle(channel: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < channel.len() {
        let v = channel[i];
        let run = channel[i..]
            .iter()
            .take(130)
            .take_while(|&&b| b == v)
            .count();
        if run >= 3 {
            out.extend_from_slice(&[(run - 3 + 0x80) as u8, v]);
            i += run;
        } else {
            let start = i;
            while i < channel.len()
                && i - start < 128
                && !(i + 2 < channel.len()
                    && channel[i] == channel[i + 1]
                    && channel[i] == channel[i + 2])
            {
                i += 1;
            }
            out.push((i - start - 1) as u8);
            out.extend_from_slice(&channel[start..i]);
        }
    }
    out
}

/// A 16×16 test pattern: R = x * 16, G = y * 16, B = constant 7.
fn pattern(w: usize, h: usize) -> [Vec<u8>; 3] {
    let r = (0..w * h).map(|p| ((p % w) * 16) as u8).collect();
    let g = (0..w * h).map(|p| ((p / w) * 16) as u8).collect();
    [r, g, vec![7; w * h]]
}

#[test]
fn detects_by_magic() {
    assert!(detect(b"icns\0\0\0\x08"));
    assert!(!detect(b"icnx\0\0\0\x08"));
    assert!(!detect(b"icns"));
}

#[test]
fn is32_with_mask_matches_hand_computed_pixels() {
    let [r, g, b] = pattern(16, 16);
    let rgb = [rle(&r), rle(&g), rle(&b)].concat();
    let mask: Vec<u8> = (0..256).map(|p| p as u8).collect();
    let f = file(&[entry(b"is32", &rgb), entry(b"s8mk", &mask)]);
    let icns = parse(&f).unwrap();
    assert_eq!(icns.icons.len(), 1);
    let icon = &icns.icons[0];
    assert_eq!((icon.width, icon.height, icon.kind), (16, 16, Kind::Rgb24));
    let img = decode(&f, icon).unwrap();
    assert!(!img.truncated);
    for (x, y) in [(0, 0), (5, 3), (15, 15), (9, 12)] {
        let p = (y * 16 + x) * 4;
        assert_eq!(
            &img.pixels[p..p + 4],
            &[(x * 16) as u8, (y * 16) as u8, 7, (y * 16 + x) as u8],
            "pixel {x},{y}"
        );
    }
}

#[test]
fn it32_skips_its_zero_prefix_and_no_mask_is_opaque() {
    let [r, g, b] = pattern(128, 128);
    let mut data = vec![0u8; 4];
    data.extend([rle(&r), rle(&g), rle(&b)].concat());
    let f = file(&[entry(b"it32", &data)]);
    let icns = parse(&f).unwrap();
    let img = decode(&f, &icns.icons[0]).unwrap();
    assert_eq!((img.width, img.height), (128, 128));
    let p = (100 * 128 + 3) * 4;
    assert_eq!(&img.pixels[p..p + 4], &[48, (100 * 16) as u8, 7, 255]);
    assert!(!img.truncated);
}

#[test]
fn uncompressed_rgb_is_read_interleaved() {
    let data: Vec<u8> = (0..16 * 16).flat_map(|p| [p as u8, 1, 2]).collect();
    let f = file(&[entry(b"is32", &data)]);
    let icns = parse(&f).unwrap();
    let img = decode(&f, &icns.icons[0]).unwrap();
    assert_eq!(&img.pixels[4 * 37..4 * 38], &[37, 1, 2, 255]);
}

#[test]
fn argb_run_length() {
    let [r, g, b] = pattern(32, 32);
    let a: Vec<u8> = (0..32 * 32)
        .map(|p| if p % 2 == 0 { 255 } else { 0 })
        .collect();
    let mut data = b"ARGB".to_vec();
    data.extend([rle(&a), rle(&r), rle(&g), rle(&b)].concat());
    let f = file(&[entry(b"ic05", &data)]);
    let icns = parse(&f).unwrap();
    assert_eq!(icns.icons[0].kind, Kind::Argb);
    let img = decode(&f, &icns.icons[0]).unwrap();
    let p = (2 * 32 + 4) * 4;
    assert_eq!(&img.pixels[p..p + 4], &[64, 32, 7, 255]);
    assert_eq!(img.pixels[p + 7], 0);
}

#[test]
fn one_bit_with_mask() {
    // 32×32: a black left half on white, mask opaque only in the top half.
    let mut data = Vec::new();
    for _ in 0..32 {
        data.extend_from_slice(&[0xff, 0xff, 0, 0]);
    }
    for y in 0..32 {
        data.extend_from_slice(&if y < 16 { [0xff; 4] } else { [0; 4] });
    }
    let f = file(&[entry(b"ICN#", &data)]);
    let icns = parse(&f).unwrap();
    let img = decode(&f, &icns.icons[0]).unwrap();
    let px = |x: usize, y: usize| &img.pixels[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4];
    assert_eq!(px(0, 0), &[0, 0, 0, 255]);
    assert_eq!(px(20, 0), &[255, 255, 255, 255]);
    assert_eq!(px(0, 20)[3], 0);
}

#[test]
fn pages_largest_first_with_retina_labels() {
    let png = |n: u8| {
        let mut p = b"\x89PNG\r\n\x1a\n".to_vec();
        p.push(n);
        p
    };
    let f = file(&[
        entry(b"icnV", &[0, 0, 0, 1]),
        entry(b"ic11", &png(1)),
        entry(b"ic08", &png(2)),
        entry(b"ic13", &png(3)),
        entry(b"ic10", &png(4)),
        entry(b"ic05", &png(5)),
        entry(b"il32", &[0; 10]),
    ]);
    let icns = parse(&f).unwrap();
    let labels: Vec<(String, String)> = icns.icons.iter().map(|i| (i.name(), i.label())).collect();
    assert_eq!(
        labels,
        [
            ("ic10".into(), "512×512 @2x".into()),
            ("ic08".into(), "256×256".into()),
            ("ic13".into(), "128×128 @2x".into()),
            // Same pixel size: 1x before 2x, then file order.
            ("ic05".into(), "32×32".into()),
            ("il32".into(), "32×32".into()),
            ("ic11".into(), "16×16 @2x".into()),
        ]
    );
    let kinds: Vec<_> = icns.icons.iter().map(|i| i.kind).collect();
    assert_eq!(
        kinds,
        [
            Kind::Png,
            Kind::Png,
            Kind::Png,
            Kind::Png,
            Kind::Rgb24,
            Kind::Png
        ]
    );
    assert_eq!(decode(&f, &icns.icons[0]), Err(Error::Embedded));
}

#[test]
fn truncation_is_filled_not_a_panic() {
    let [r, g, b] = pattern(16, 16);
    let rgb = [rle(&r), rle(&g), rle(&b)].concat();
    let mut f = file(&[entry(b"is32", &rgb)]);
    f.truncate(f.len() - 20);
    let icns = parse(&f).unwrap();
    assert!(icns.truncated);
    let img = decode(&f, &icns.icons[0]).unwrap();
    assert!(img.truncated);
    assert_eq!(img.pixels.len(), 16 * 16 * 4);
}

#[test]
fn hostile_inputs_do_not_panic() {
    assert_eq!(parse(b"icns").unwrap_err(), Error::Truncated);
    assert_eq!(parse(b"nope\0\0\0\x08").unwrap_err(), Error::NotIcns);
    assert_eq!(parse(&file(&[])).unwrap_err(), Error::NoIcons);
    // Lengths that overflow, point past the end or below the header size.
    let mut f = b"icns\xff\xff\xff\xff".to_vec();
    f.extend_from_slice(b"it32\xff\xff\xff\xf0\x00");
    let icns = parse(&f).unwrap();
    let img = decode(&f, &icns.icons[0]).unwrap();
    assert!(img.truncated);
    let mut f = b"icns\0\0\0\x20".to_vec();
    f.extend_from_slice(b"is32\0\0\0\x02");
    assert!(parse(&f).is_err());
    // Runs that overrun the plane.
    let f = file(&[
        entry(b"is32", &[0xff, 1].repeat(200)),
        entry(b"s8mk", &[1, 2]),
    ]);
    let icns = parse(&f).unwrap();
    let img = decode(&f, &icns.icons[0]).unwrap();
    assert!(img.truncated, "mask is short");
}
