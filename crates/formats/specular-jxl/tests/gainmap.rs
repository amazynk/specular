//! JPEG XL gain maps (`jhgm` box) on the fixtures made by `tests/gen_gainmap.py`. The
//! base and map codestreams are checked against upstream jxl-oxide (the oracle) and against
//! the values the script encoded.

use specular_jxl::{Error, JxlFile, Layout, Primaries};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

/// Upstream render of the first frame, interleaved f32, in the file's own colour encoding.
fn upstream(data: &[u8]) -> (usize, Vec<f32>) {
    let img = jxl_oxide::JxlImage::builder().read(data).unwrap();
    let r = img.render_frame(0).unwrap();
    let mut s = r.stream();
    let (w, h, c) = (s.width(), s.height(), s.channels() as usize);
    let mut buf = vec![0f32; w as usize * h as usize * c];
    s.write_to_buffer(&mut buf);
    (c, buf)
}

/// The SDR alternate of gainmap_hdr.jxl (gen_gainmap.py `sdr_px`).
fn sdr_px(x: u32, y: u32) -> [u8; 3] {
    [(40 + 6 * x) as u8, (30 + 12 * y) as u8, (200 - 4 * x) as u8]
}

fn srgb_to_linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn pq_oetf(nits: f64) -> f64 {
    let (m1, m2) = (2610.0 / 16384.0, 2523.0 / 4096.0 * 128.0);
    let (c1, c2, c3) = (
        3424.0 / 4096.0,
        2413.0 / 4096.0 * 32.0,
        2392.0 / 4096.0 * 32.0,
    );
    let p = (nits.max(0.0) / 10000.0).powf(m1);
    ((c1 + c2 * p) / (1.0 + c3 * p)).powf(m2)
}

#[test]
fn sdr_base() {
    let data = fixture("gainmap_sdr.jxl");
    let f = JxlFile::open(&data[..]).unwrap();
    let i = f.info();
    assert!(i.gain_map);
    assert_eq!(i.hdr, None);
    assert_eq!(i.primaries, Some(Primaries::Bt709));
    let g = f.gain_map().unwrap().unwrap();
    assert!(g.colour_encoding.is_empty() && g.alt_icc.is_empty());
    assert_eq!(g.alt_primaries(), None);
    // Common denominator 10000, base headroom 0, alternate 2.
    assert_eq!(&g.metadata[..5], &[0, 0, 0, 0, 0x08]);
    assert_eq!(
        &g.metadata[5..17],
        &[0, 0, 0x27, 0x10, 0, 0, 0, 0, 0, 0, 0x4e, 0x20]
    );
    // The base decodes as written, and as upstream does (the jhgm box does not get in the way).
    let d = f.decode(1, false).unwrap();
    assert_eq!((d.width, d.height, d.layout), (32, 16, Layout::Rgb));
    for (n, px) in d.data.chunks_exact(3).enumerate() {
        let (x, y) = (n % 32, n / 32);
        assert_eq!(px, [(x * 8) as u8, (y * 16) as u8, 128], "({x}, {y})");
    }
    let (c, up) = upstream(&data);
    assert_eq!(c, 3);
    assert!(
        d.data
            .iter()
            .zip(&up)
            .all(|(&a, &b)| a == (b * 255.0).round() as u8)
    );
    // The map: 16×8 grey ramp.
    assert_eq!(f.gain_map_size().unwrap(), (16, 8));
    let m = f.decode_gain_map(true).unwrap();
    assert_eq!((m.width, m.height, m.layout), (16, 8, Layout::Gray));
    for (n, &v) in m.data.iter().enumerate() {
        assert_eq!(v as usize, ((n % 16) * 255 + 7) / 15, "{n}");
    }
    let (_, up) = upstream(g.codestream);
    assert!(
        m.data
            .iter()
            .zip(&up)
            .all(|(&a, &b)| a == (b * 255.0).round() as u8)
    );
}

#[test]
fn hdr_base() {
    let data = fixture("gainmap_hdr.jxl");
    let f = JxlFile::open(&data[..]).unwrap();
    let i = f.info().clone();
    assert!(i.gain_map);
    assert_eq!(i.hdr, Some(specular_jxl::Hdr::Pq));
    assert_eq!(i.primaries, Some(Primaries::Bt2020));
    let g = f.gain_map().unwrap().unwrap();
    // The alternate is sRGB, written out field by field.
    assert_eq!(g.colour_encoding.len(), 3);
    assert_eq!(g.alt_primaries(), Some(Primaries::Bt709));
    // Multichannel, base colour space.
    assert_eq!(g.metadata[4], 0xc0);

    // The PQ signal, unmapped, against the values written and against upstream.
    let to2020 = [
        [0.6274, 0.3293, 0.0433],
        [0.0691, 0.9195, 0.0114],
        [0.0164, 0.0880, 0.8956],
    ];
    let (c, up) = upstream(&data);
    assert_eq!(c, 3);
    for parallel in [false, true] {
        let d = f.decode_hdr(1, parallel).unwrap();
        assert_eq!((d.width, d.height, d.layout), (32, 16, Layout::Rgba16));
        assert_eq!(d.data.len(), 32 * 16 * 8);
        for (n, px) in d.data.chunks_exact(8).enumerate() {
            let (x, y) = ((n % 32) as u32, (n / 32) as u32);
            let k = 2f64.powf(2.0 * y as f64 / 15.0);
            let s709 = sdr_px(x, y).map(|v| srgb_to_linear(v as f64 / 255.0));
            let u = |c: usize| u16::from_ne_bytes([px[2 * c], px[2 * c + 1]]);
            for (c, row) in to2020.iter().enumerate() {
                let lin: f64 = row.iter().zip(&s709).map(|(m, v)| m * v).sum();
                let want = (pq_oetf(lin * k * 203.0) * 65535.0).round();
                assert!((u(c) as f64 - want).abs() <= 1.0, "({x}, {y}) {c}");
                let oracle = (up[n * 3 + c] * 65535.0).round();
                assert!(
                    (u(c) as f32 - oracle).abs() <= 1.0,
                    "({x}, {y}) {c} vs upstream"
                );
            }
            assert_eq!(u(3), 65535);
        }
    }
    // Reduced: box average of the signal.
    let full = f.decode_hdr(1, false).unwrap();
    let half = f.decode_hdr(2, true).unwrap();
    assert_eq!((half.width, half.height), (16, 8));
    let at = |d: &specular_jxl::Decoded, x: usize, y: usize, c: usize| {
        let o = (y * d.width as usize + x) * 8 + 2 * c;
        u16::from_ne_bytes([d.data[o], d.data[o + 1]]) as f32
    };
    let avg =
        (at(&full, 2, 2, 1) + at(&full, 3, 2, 1) + at(&full, 2, 3, 1) + at(&full, 3, 3, 1)) / 4.0;
    assert!((at(&half, 1, 1, 1) - avg).abs() <= 1.0);
    // The ordinary decode still tone maps to 8-bit sRGB.
    assert_eq!(f.decode(1, false).unwrap().layout, Layout::Rgb);

    let m = f.decode_gain_map(false).unwrap();
    assert_eq!((m.width, m.height, m.layout), (32, 16, Layout::Rgb));
    let (_, up) = upstream(g.codestream);
    assert!(
        m.data
            .iter()
            .zip(&up)
            .all(|(&a, &b)| a == (b * 255.0).round() as u8)
    );
}

/// gainmap_hdr.jxl with its `jhgm` box Brotli-compressed into a `brob` box (gen_brob.py): the
/// same bundle and gain map.
#[test]
fn brotli_compressed_bundle() {
    let plain = JxlFile::open(fixture("gainmap_hdr.jxl")).unwrap();
    let data = fixture("gainmap_hdr_brob.jxl");
    assert!(data.windows(8).any(|w| w == b"brobjhgm"));
    let f = JxlFile::open(&data[..]).unwrap();
    assert!(f.info().gain_map);
    let (a, b) = (
        plain.gain_map().unwrap().unwrap(),
        f.gain_map().unwrap().unwrap(),
    );
    assert_eq!(a.metadata, b.metadata);
    assert_eq!(a.colour_encoding, b.colour_encoding);
    assert_eq!(a.alt_icc, b.alt_icc);
    assert_eq!(a.codestream, b.codestream);
    assert_eq!(f.gain_map_size().unwrap(), (32, 16));
    assert_eq!(
        f.decode_gain_map(false).unwrap().data,
        plain.decode_gain_map(false).unwrap().data
    );
}

#[test]
fn sdr_files_have_no_hdr_decode() {
    let f = JxlFile::open(fixture("gainmap_sdr.jxl")).unwrap();
    assert!(matches!(f.decode_hdr(1, false), Err(Error::Unsupported(_))));
    let f = JxlFile::open(fixture("vardct.jxl")).unwrap();
    assert!(!f.info().gain_map);
    assert!(f.gain_map().is_none());
    assert!(f.decode_gain_map(false).is_err());
}

/// The `jhgm` box moved after the codestream, compressed, of another version, cut short, or
/// with a broken map: the image itself still opens.
#[test]
fn box_variants() {
    let data = fixture("gainmap_sdr.jxl");
    // Signature (12) + ftyp (20), then jhgm.
    let at = 32;
    let len = u32::from_be_bytes(data[at..at + 4].try_into().unwrap()) as usize;
    assert_eq!(&data[at + 4..at + 8], b"jhgm");
    let jhgm = &data[at..at + len];
    let rest = &data[at + len..];

    let mut after = data[..at].to_vec();
    after.extend_from_slice(rest);
    after.extend_from_slice(jhgm);
    let f = JxlFile::open(&after[..]).unwrap();
    assert_eq!(f.decode_gain_map(false).unwrap().width, 16);

    let mut brob = data[..at].to_vec();
    brob.extend_from_slice(&((len + 4) as u32).to_be_bytes());
    brob.extend_from_slice(b"brobjhgm");
    brob.extend_from_slice(&jhgm[8..]);
    brob.extend_from_slice(rest);
    // Its body is not a Brotli stream.
    let f = JxlFile::open(&brob[..]).unwrap();
    assert!(f.info().gain_map);
    assert!(matches!(f.gain_map(), Some(Err(Error::Malformed(_)))));
    assert!(f.decode(1, false).is_ok());

    let mut v1 = data.clone();
    v1[at + 8] = 1;
    let f = JxlFile::open(&v1[..]).unwrap();
    assert!(matches!(f.gain_map(), Some(Err(Error::Unsupported(_)))));

    // A corrupt map codestream: an error, never a panic.
    let mut bad = data.clone();
    for b in &mut bad[at + len - 12..at + len] {
        *b ^= 0x5a;
    }
    if let Ok(f) = JxlFile::open(&bad[..]) {
        let _ = f.decode_gain_map(false);
    }

    // Every cut of the file: no panics.
    for n in 0..data.len() {
        if let Ok(f) = JxlFile::open(&data[..n]) {
            let _ = f.gain_map().map(|g| g.map(|g| g.alt_primaries()));
            let _ = f.gain_map_size();
            let _ = f.decode_gain_map(false);
        }
    }
}
