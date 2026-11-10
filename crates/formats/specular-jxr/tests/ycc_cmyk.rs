//! YCC pixel formats (gen/make_ycc.py) and 16-bit CMYK (gen/make_cmyk16.py).

use std::path::Path;

use specular_jxr::{
    BitDepth, DecodeOptions, Kind, Native, OutFmt, Pixels, Samples, decode, decode_native,
    read_info,
};

fn bytes(s: &Samples) -> Vec<u8> {
    match s {
        Samples::U8(v) => v.clone(),
        Samples::U16(v) => v.iter().flat_map(|x| x.to_le_bytes()).collect(),
        Samples::U32(v) => v.iter().flat_map(|x| x.to_le_bytes()).collect(),
    }
}

fn fnv(data: &[u8]) -> u64 {
    data.iter().fold(0xCBF2_9CE4_8422_2325u64, |h, &b| {
        (h ^ b as u64).wrapping_mul(0x100_0000_01B3)
    })
}

fn dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors")
}

/// Manifest lines, split on whitespace.
fn manifest(name: &str) -> Vec<Vec<String>> {
    std::fs::read_to_string(dir().join(name))
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| l.split_whitespace().map(str::to_string).collect())
        .collect()
}

/// Byte offsets of the pixel-format GUID's last byte and of the codestream's
/// OUTPUT_CLR_FMT / OUTPUT_BITDEPTH byte.
fn label_offsets(d: &[u8]) -> (usize, usize) {
    let u16_at = |p: usize| u16::from_le_bytes([d[p], d[p + 1]]) as usize;
    let u32_at = |p: usize| u32::from_le_bytes(d[p..p + 4].try_into().unwrap()) as usize;
    let ifd = u32_at(4);
    let (mut guid, mut image) = (0, 0);
    for k in 0..u16_at(ifd) {
        let e = ifd + 2 + 12 * k;
        match u16_at(e) {
            0xBC01 => guid = u32_at(e + 8) + 15,
            0xBCC0 => image = u32_at(e + 8) + 11,
            _ => {}
        }
    }
    assert!(guid > 0 && image > 0);
    (guid, image)
}

/// Integer samples and channels per pixel, with packed BD10 RGB unpacked.
fn ints(n: &Native) -> (Vec<i64>, usize) {
    match &n.samples {
        Samples::U8(v) => (v.iter().map(|&x| x as i64).collect(), n.channels),
        Samples::U16(v) if n.bit_depth == BitDepth::Bd16S => {
            (v.iter().map(|&x| x as i16 as i64).collect(), n.channels)
        }
        Samples::U16(v) => (v.iter().map(|&x| x as i64).collect(), n.channels),
        Samples::U32(v) => (
            v.iter()
                .flat_map(|&p| [p & 1023, (p >> 10) & 1023, (p >> 20) & 1023])
                .map(|x| x as i64)
                .collect(),
            3,
        ),
    }
}

/// jxrlib can neither write nor fully read the YCC formats, so the vectors are jxrlib files
/// re-labelled as YCC. Checks: with the labels restored the file decodes bit-exact with
/// JxrDecApp; the Y channel matches JxrDecApp's grey decode of the YCC file; RGB rebuilt from
/// the YCC samples with T.832's inverse colour transform matches the RGB decode (exactly when
/// lossless; lossy files are scaled after the colour transform, so they may differ slightly).
#[test]
fn ycc_vectors() {
    let mut count = 0;
    for f in manifest("ycc.txt") {
        let name = f[0].as_str();
        let data = std::fs::read(dir().join(format!("{name}.jxr"))).unwrap();
        let info = read_info(&data).unwrap();
        assert_eq!(info.pixel_format, Some("YCC"), "{name}");
        assert!(
            matches!(
                info.output,
                OutFmt::Yuv420 | OutFmt::Yuv422 | OutFmt::Yuv444
            ),
            "{name}"
        );
        let ycc = decode_native(&data, &DecodeOptions::default()).unwrap();
        let alpha = ycc.alpha as usize;
        assert_eq!(ycc.channels, 3 + alpha, "{name}");
        let serial = DecodeOptions {
            serial: true,
            ..Default::default()
        };
        assert_eq!(
            decode_native(&data, &serial).unwrap().samples,
            ycc.samples,
            "{name}"
        );

        // Restore RGB labels (24bppRGB, 32bppBGRA, 48bppRGB, 64bppRGBA, 32bppBGR101010,
        // 48bppRGBFixedPoint) and check the coded data against jxrlib.
        let (g, o) = label_offsets(&data);
        let mut rgb_file = data.clone();
        rgb_file[g] = match (ycc.bit_depth, ycc.alpha) {
            (BitDepth::Bd8, false) => 0x0D,
            (BitDepth::Bd8, true) => 0x0F,
            (BitDepth::Bd16, false) => 0x15,
            (BitDepth::Bd16, true) => 0x16,
            (BitDepth::Bd10, _) => 0x14,
            (BitDepth::Bd16S, _) => 0x12,
            other => panic!("{name}: {other:?}"),
        };
        rgb_file[o] = (7 << 4) | (rgb_file[o] & 15);
        let rgb = decode_native(&rgb_file, &DecodeOptions::default()).unwrap();
        assert_eq!(
            fnv(&bytes(&rgb.samples)),
            u64::from_str_radix(&f[3], 16).unwrap(),
            "{name}: RGB decode"
        );

        // Luma against JxrDecApp's grey output.
        let (y, ys) = ints(&ycc);
        if f[4] != "-" {
            let luma: Vec<u8> = match &ycc.samples {
                Samples::U8(v) => v.iter().step_by(ys).copied().collect(),
                Samples::U16(v) => v.iter().step_by(ys).flat_map(|x| x.to_le_bytes()).collect(),
                Samples::U32(_) => unreachable!(),
            };
            assert_eq!(
                fnv(&luma),
                u64::from_str_radix(&f[4], 16).unwrap(),
                "{name}: luma"
            );
        }

        // Chroma: inverse colour transform of the YCC samples against the RGB samples,
        // skipping pixels clipped in either.
        let (c, cs) = ints(&rgb);
        let (bias, max): (i64, i64) = match ycc.bit_depth {
            BitDepth::Bd8 => (128, 255),
            BitDepth::Bd10 => (512, 1023),
            BitDepth::Bd16 => (32768, 65535),
            _ => (0, 32767),
        };
        let lo = if bias == 0 { -32768 } else { 0 };
        let (mut worst, mut used) = (0i64, 0usize);
        for (p, q) in y.chunks_exact(ys).zip(c.chunks_exact(cs)) {
            if p[..3].iter().chain(&q[..3]).any(|&v| v <= lo || v >= max) {
                continue;
            }
            let (yy, u, v) = (p[0] - bias, p[1] - bias, p[2] - bias);
            let t = -u;
            let gg = yy - (t >> 1);
            let r = t + gg - ((v + 1) >> 1);
            let b = v + r;
            for (got, want) in [r, gg, b].iter().zip(&q[..3]) {
                worst = worst.max((got + bias - want).abs());
            }
            used += 1;
        }
        assert!(
            used * 2 > y.len() / ys,
            "{name}: only {used} unclipped pixels"
        );
        // Lossless 4:4:4 is unscaled, so exact; elsewhere the reference scales (rounds) after
        // the colour transform, one unit at most.
        let exact = name.contains("lossless") && info.output == OutFmt::Yuv444;
        assert!(worst <= !exact as i64, "{name}: chroma off by {worst}");
        if alpha == 1 {
            let a: Vec<i64> = y.chunks_exact(ys).map(|p| p[3]).collect();
            let b: Vec<i64> = c.chunks_exact(cs).map(|p| p[3]).collect();
            assert_eq!(a, b, "{name}: alpha");
        }

        // Display: RGB, alpha kept.
        let img = decode(&data, &DecodeOptions::default()).unwrap();
        assert_eq!((img.kind, img.alpha), (Kind::Rgb, ycc.alpha), "{name}");
        count += 1;
    }
    assert!(count >= 14);
}

/// Display conversion is full-range BT.601 (JFIF) YCbCr.
#[test]
fn ycc_display_matrix() {
    let data = std::fs::read(dir().join("ycc444-8-lossless.jxr")).unwrap();
    let n = decode_native(&data, &DecodeOptions::default()).unwrap();
    let img = decode(&data, &DecodeOptions::default()).unwrap();
    let (Samples::U8(s), Pixels::U8(p)) = (&n.samples, &img.pixels) else {
        panic!("8-bit expected")
    };
    for (y, o) in s.chunks_exact(3).zip(p.chunks_exact(3)) {
        let (l, cb, cr) = (y[0] as f32, y[1] as f32 - 128.0, y[2] as f32 - 128.0);
        let want = [
            l + 1.402 * cr,
            l - 0.344_136 * cb - 0.714_136 * cr,
            l + 1.772 * cb,
        ];
        for (w, &g) in want.iter().zip(o) {
            assert_eq!(w.round().clamp(0.0, 255.0) as u8, g);
        }
    }
}

/// 64bppCMYK and 80bppCMYKAlpha, decoded bit-exact with JxrDecApp.
#[test]
fn cmyk16_vectors() {
    let mut count = 0;
    for f in manifest("cmyk16.txt") {
        let name = f[0].as_str();
        let data = std::fs::read(dir().join(format!("{name}.jxr"))).unwrap();
        let n = decode_native(&data, &DecodeOptions::default()).unwrap();
        let want: (u32, u32, usize) = (
            f[1].parse().unwrap(),
            f[2].parse().unwrap(),
            f[3].parse().unwrap(),
        );
        assert_eq!((n.width, n.height, n.channels), want, "{name}");
        assert_eq!(n.bit_depth, BitDepth::Bd16, "{name}");
        assert_eq!(
            fnv(&bytes(&n.samples)),
            u64::from_str_radix(&f[4], 16).unwrap(),
            "{name}"
        );
        let img = decode(&data, &DecodeOptions::default()).unwrap();
        assert_eq!(
            (img.kind, img.alpha),
            (Kind::Cmyk, n.channels == 5),
            "{name}"
        );
        assert!(matches!(img.pixels, Pixels::U16(_)), "{name}");
        count += 1;
    }
    assert!(count >= 4);
}
