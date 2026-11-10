use crate::*;

fn ax8_planck() -> Planck {
    Planck {
        r1: 16951.796875,
        r2: 0.0101,
        b: 1435.1,
        f: 1.0,
        o: -2463.0,
        alpha1: 0.006569,
        alpha2: 0.01262,
        beta1: -0.002276,
        beta2: -0.00667,
        x: 1.9,
    }
}

#[test]
fn planck_round_trip() {
    let p = ax8_planck();
    for t in [-20.0, 0.0, 25.0, 100.0, 400.0] {
        assert!((p.blackbody(p.counts(t)) - t).abs() < 1e-9, "{t}");
    }
}

#[test]
fn ideal_conditions_are_the_blackbody() {
    let p = ax8_planck();
    let c = Conditions {
        emissivity: Some(1.0),
        distance: Some(0.0),
        window_transmission: Some(1.0),
        ..Default::default()
    };
    let s = p.solver(&c);
    for raw in [5000.0, 12000.0, 30000.0] {
        assert!((s.celsius(raw) - p.blackbody(raw)).abs() < 1e-9);
    }
}

#[test]
fn lower_emissivity_reads_hotter_above_ambient() {
    let p = ax8_planck();
    let raw = p.counts(80.0);
    let base = Conditions {
        emissivity: Some(1.0),
        distance: Some(1.0),
        reflected: Some(20.0),
        atmospheric: Some(20.0),
        humidity: Some(0.5),
        ..Default::default()
    };
    let t1 = p.solver(&base).celsius(raw);
    let t2 = p
        .solver(&Conditions {
            emissivity: Some(0.8),
            ..base
        })
        .celsius(raw);
    assert!(t2 > t1 + 5.0, "{t1} {t2}");
}

#[test]
fn dates() {
    assert_eq!(flir::civil(0), (1970, 1, 1));
    assert_eq!(flir::civil(19_723), (2024, 1, 1));
    assert_eq!(flir::civil(-1), (1969, 12, 31));
}

fn segment(marker: u8, payload: &[u8]) -> Vec<u8> {
    let mut s = vec![0xff, marker];
    s.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
    s.extend_from_slice(payload);
    s
}

fn flir_app1(idx: u8, last: u8, body: &[u8]) -> Vec<u8> {
    let mut p = b"FLIR\0\x01".to_vec();
    p.extend_from_slice(&[idx, last]);
    p.extend_from_slice(body);
    segment(0xe1, &p)
}

#[test]
fn jpeg_walk() {
    let mut plain = vec![0xff, 0xd8];
    plain.extend(segment(0xe0, b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0"));
    plain.extend(segment(0xe1, b"Exif\0\0rest"));
    plain.extend(segment(0xdb, &[0; 65]));
    plain.extend(segment(0xda, &[0; 10]));
    // A FLIR-looking payload after the scan header must not count.
    plain.extend(flir_app1(0, 0, b"xx"));
    assert!(!is_flir_jpeg(&plain));
    assert_eq!(fff_from_jpeg(&plain), Err(Error::NotThermal));

    let mut j = vec![0xff, 0xd8];
    j.extend(segment(0xe0, b"JFIF\0"));
    j.extend(flir_app1(1, 1, b"WORLD"));
    j.extend(flir_app1(0, 1, b"HELLO "));
    j.extend(segment(0xda, &[0; 4]));
    assert!(is_flir_jpeg(&j));
    assert_eq!(fff_from_jpeg(&j).unwrap(), b"HELLO WORLD");
    assert!(!is_flir_jpeg(&j[..12]));
    assert!(!is_flir_jpeg(b""));
}

/// The compression-2 bit stream for `v`, written the way the decoder reads it.
fn code_deltas(v: &[u16]) -> Vec<u8> {
    let mut bits: Vec<bool> = Vec::new();
    let put = |x: u32, n: u32, bits: &mut Vec<bool>| {
        for i in (0..n).rev() {
            bits.push(x >> i & 1 == 1);
        }
    };
    for w in v.windows(2) {
        let d = i32::from(w[1]) - i32::from(w[0]);
        let u = if d < 0 {
            ((-d) << 1 | 1) as u32
        } else {
            (d << 1) as u32
        };
        if u < 192 {
            put(u, 8, &mut bits);
        } else {
            let c = u + 192 * 2047;
            put(c >> 11, 8, &mut bits);
            put(c & 2047, 11, &mut bits);
        }
    }
    while !bits.len().is_multiple_of(16) {
        bits.push(false);
    }
    let mut out = v[0].to_le_bytes().to_vec();
    for w in bits.chunks(16) {
        let x = w.iter().fold(0u16, |a, &b| a << 1 | u16::from(b));
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

#[test]
fn irb_delta_coding() {
    let v: Vec<u16> = vec![
        29315, 29316, 29300, 29400, 31000, 29000, 29001, 29001, 0, 65535,
    ];
    let coded = code_deltas(&v);
    assert_eq!(irb::deltas(&coded, v.len()).unwrap(), v);
    assert!(irb::deltas(&coded[..4], v.len()).is_err());
}

#[test]
fn csv_and_tiff() {
    let v = [1.0, 2.5, f32::NAN, -3.25];
    assert_eq!(csv(2, &v, 2), "1.00,2.50\n,-3.25\n");
    let t = tiff_f32(2, 2, &v, "temperature");
    assert!(t.starts_with(b"II*\0"));
    let data = &t[t.len() - 16..];
    assert_eq!(f32::from_le_bytes(data[4..8].try_into().unwrap()), 2.5);
    assert_eq!(f32::from_le_bytes(data[12..16].try_into().unwrap()), -3.25);
}

#[test]
fn palette_resampling() {
    let p = Palette {
        name: "x".into(),
        colours: vec![[0, 0, 0], [128, 0, 0], [255, 255, 255]],
        above: None,
        below: None,
        isotherm: None,
    };
    let r = p.resampled(256);
    assert_eq!(r.len(), 256);
    assert_eq!(r[0], [0, 0, 0]);
    assert_eq!(r[255], [255, 255, 255]);
    assert_eq!(r[128], [128, 0, 0]);
}

#[test]
fn not_thermal() {
    assert_eq!(sniff(b"GIF89a"), None);
    assert_eq!(read(b"\xff\xd8\xff\xd9"), Err(Error::NotThermal));
    assert_eq!(sniff(b"FFF\0"), None, "too short for a header");
}
