//! Hand-built Radiance and PFM files with known answers, and
//! truncation/corruption loops.

use specular_radiance::{
    ColorSpace, DecodeOptions, Encoding, Error, Format, Limits, OutputChannels, decode, detect,
    probe, read_info,
};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn byte(&mut self) -> u8 {
        (self.next() >> 32) as u8
    }
}

/// Logical image of RGBE pixels (row-major, top-down, left-to-right) with runs.
fn rgbe_image(w: usize, h: usize, seed: u64) -> Vec<[u8; 4]> {
    let mut rng = Rng(seed | 1);
    let mut v = Vec::with_capacity(w * h);
    let mut cur = [0u8; 4];
    for i in 0..w * h {
        if i == 0 || !rng.next().is_multiple_of(4) {
            cur = loop {
                let e = if rng.next().is_multiple_of(50) {
                    0
                } else {
                    120 + (rng.next() % 20) as u8
                };
                let p = [rng.byte() | 0x80, rng.byte(), rng.byte(), e];
                // Never look like an RLE marker or a new-RLE scanline header.
                if !(p[0] == 1 && p[1] == 1 && p[2] == 1) && !(p[0] == 2 && p[1] == 2) {
                    break p;
                }
            };
        }
        v.push(cur);
    }
    v
}

fn to_f32(p: [u8; 4]) -> [f32; 3] {
    if p[3] == 0 {
        return [0.0; 3];
    }
    let m = (p[3] as f32 - 136.0).exp2();
    [p[0] as f32 * m, p[1] as f32 * m, p[2] as f32 * m]
}

#[derive(Clone, Copy, Debug)]
enum Rle {
    Flat,
    Old,
    New,
}

fn encode_line(line: &[[u8; 4]], rle: Rle, out: &mut Vec<u8>) {
    match rle {
        Rle::Flat => line.iter().for_each(|p| out.extend_from_slice(p)),
        Rle::Old => {
            let mut i = 0;
            while i < line.len() {
                out.extend_from_slice(&line[i]);
                let mut j = i + 1;
                while j < line.len() && line[j] == line[i] && j - i < 256 {
                    j += 1;
                }
                if j - i > 1 {
                    out.extend_from_slice(&[1, 1, 1, (j - i - 1) as u8]);
                }
                i = j;
            }
        }
        // Radiance only uses new RLE for lengths 8..=0x7fff.
        Rle::New if !(8..=0x7fff).contains(&line.len()) => encode_line(line, Rle::Flat, out),
        Rle::New => {
            let n = line.len();
            out.extend_from_slice(&[2, 2, (n >> 8) as u8, n as u8]);
            for c in 0..4 {
                let comp: Vec<u8> = line.iter().map(|p| p[c]).collect();
                let mut i = 0;
                while i < n {
                    let mut run = 1;
                    while i + run < n && comp[i + run] == comp[i] && run < 127 {
                        run += 1;
                    }
                    if run >= 3 {
                        out.extend_from_slice(&[128 + run as u8, comp[i]]);
                        i += run;
                        continue;
                    }
                    // Literal up to the next run of 3 (max 128).
                    let start = i;
                    while i < n && i - start < 128 {
                        if i + 2 < n && comp[i] == comp[i + 1] && comp[i] == comp[i + 2] {
                            break;
                        }
                        i += 1;
                    }
                    out.push((i - start) as u8);
                    out.extend_from_slice(&comp[start..i]);
                }
            }
        }
    }
}

/// Write a logical image in one of the eight orientations. `res` like "-Y {h} +X {w}".
fn write_hdr(
    img: &[[u8; 4]],
    w: usize,
    h: usize,
    axes: (&str, &str),
    rle: Rle,
    header: &str,
) -> Vec<u8> {
    let (a1, a2) = axes;
    let x_major = a1.ends_with('X');
    let (ya, xa) = if x_major { (a2, a1) } else { (a1, a2) };
    let top_down = ya == "-Y";
    let ltr = xa == "+X";
    let mut out =
        format!("#?RADIANCE\n# made by a test\n{header}FORMAT=32-bit_rle_rgbe\n\n").into_bytes();
    let res = if x_major {
        format!("{a1} {w} {a2} {h}\n")
    } else {
        format!("{a1} {h} {a2} {w}\n")
    };
    out.extend_from_slice(res.as_bytes());
    let px = |x: usize, y: usize| img[y * w + x];
    if x_major {
        for i in 0..w {
            let x = if ltr { i } else { w - 1 - i };
            let col: Vec<_> = (0..h)
                .map(|j| px(x, if top_down { j } else { h - 1 - j }))
                .collect();
            encode_line(&col, rle, &mut out);
        }
    } else {
        for j in 0..h {
            let y = if top_down { j } else { h - 1 - j };
            let row: Vec<_> = (0..w)
                .map(|i| px(if ltr { i } else { w - 1 - i }, y))
                .collect();
            encode_line(&row, rle, &mut out);
        }
    }
    out
}

fn expect_rgba(img: &[[u8; 4]]) -> Vec<f32> {
    img.iter()
        .flat_map(|&p| {
            let [r, g, b] = to_f32(p);
            [r, g, b, 1.0]
        })
        .collect()
}

fn opts(channels: OutputChannels, parallel: bool) -> DecodeOptions {
    DecodeOptions {
        channels,
        parallel,
        ..Default::default()
    }
}

const ORIENTATIONS: [(&str, &str); 8] = [
    ("-Y", "+X"),
    ("-Y", "-X"),
    ("+Y", "+X"),
    ("+Y", "-X"),
    ("+X", "-Y"),
    ("-X", "-Y"),
    ("+X", "+Y"),
    ("-X", "+Y"),
];

#[test]
fn all_orientations_and_encodings() {
    for (w, h) in [(37, 19), (8, 8), (5, 3), (300, 2)] {
        let img = rgbe_image(w, h, (w * 31 + h) as u64);
        let want = expect_rgba(&img);
        for axes in ORIENTATIONS {
            for rle in [Rle::Flat, Rle::Old, Rle::New] {
                let bytes = write_hdr(&img, w, h, axes, rle, "");
                for parallel in [true, false] {
                    let what = format!("{w}x{h} {axes:?} {rle:?} parallel={parallel}");
                    let d = decode(&bytes, &opts(OutputChannels::Rgba, parallel))
                        .unwrap_or_else(|e| panic!("{what}: {e}"));
                    assert_eq!((d.width, d.height), (w as u32, h as u32), "{what}");
                    assert!(!d.truncated, "{what}");
                    assert_eq!(d.data, want, "{what}");
                }
                let d = decode(&bytes, &opts(OutputChannels::Rgb, true)).unwrap();
                let rgb: Vec<f32> = want
                    .chunks_exact(4)
                    .flat_map(|p| [p[0], p[1], p[2]])
                    .collect();
                assert_eq!(d.data, rgb);
            }
        }
    }
}

#[test]
fn known_values() {
    // Radiance's colr_color: m * 2^(e - 136), no half-step offset; e = 0 is black.
    let mut bytes = b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y 1 +X 4\n".to_vec();
    bytes.extend_from_slice(&[128, 64, 0, 129, 255, 1, 2, 136, 9, 9, 9, 0, 1, 2, 3, 120]);
    let d = decode(&bytes, &opts(OutputChannels::Rgb, false)).unwrap();
    let k = 2f32.powi(-16);
    assert_eq!(
        d.data,
        [
            1.0,
            0.5,
            0.0,
            255.0,
            1.0,
            2.0,
            0.0,
            0.0,
            0.0,
            k,
            2.0 * k,
            3.0 * k
        ]
    );
}

#[test]
fn old_rle_shifted_counts() {
    // One pixel, then markers 2 and 1: 2 + (1 << 8) = 258 repeats; total 259 pixels.
    let p = [200u8, 100, 50, 130];
    let mut bytes = b"#?RGBE\n\n-Y 1 +X 259\n".to_vec();
    bytes.extend_from_slice(&p);
    bytes.extend_from_slice(&[1, 1, 1, 2, 1, 1, 1, 1]);
    let d = decode(&bytes, &opts(OutputChannels::Rgb, false)).unwrap();
    assert!(!d.truncated);
    let want = to_f32(p);
    assert!(d.data.chunks_exact(3).all(|c| c == want));
}

#[test]
fn header_fields() {
    let img = rgbe_image(10, 4, 3);
    let header = "EXPOSURE= 2.0\nEXPOSURE=0.5e1\nCOLORCORR=1 2 0.5\nPIXASPECT=1.5\nPRIMARIES= 0.64 0.33 0.3 0.6 0.15 0.06 0.3127 0.329\n";
    let mut bytes = write_hdr(&img, 10, 4, ("-Y", "+X"), Rle::New, header);
    let info = read_info(&bytes).unwrap();
    assert_eq!(info.format, Format::Radiance);
    assert_eq!(info.encoding, Encoding::Rgbe);
    assert_eq!(info.color_space, ColorSpace::Rgb);
    assert_eq!(info.exposure, 10.0);
    assert_eq!(info.color_correction, [1.0, 2.0, 0.5]);
    assert_eq!(info.pixel_aspect, 1.5);
    assert_eq!(
        info.primaries,
        Some([0.64, 0.33, 0.3, 0.6, 0.15, 0.06, 0.3127, 0.329])
    );
    assert_eq!((info.width, info.height), (10, 4));
    // XYZE
    let s = String::from_utf8_lossy(&bytes).replacen("32-bit_rle_rgbe", "32-bit_rle_xyze", 1);
    let n = s.find("\n-Y").unwrap();
    let mut xyz = s.as_bytes()[..n].to_vec();
    xyz.extend_from_slice(&bytes[n..]);
    assert_eq!(read_info(&xyz).unwrap().color_space, ColorSpace::Xyz);
    // Unknown format.
    let s = String::from_utf8_lossy(&bytes[..n]).replacen("32-bit_rle_rgbe", "16-bit_weird", 1);
    let mut bad = s.into_bytes();
    bad.extend_from_slice(&bytes[n..]);
    assert!(matches!(read_info(&bad), Err(Error::Unsupported(_))));
    // CRLF line endings in the header.
    bytes = b"#?RADIANCE\r\nFORMAT=32-bit_rle_rgbe\r\n\r\n-Y 1 +X 2\r\n\x80\x80\x80\x80\x40\x40\x40\x81".to_vec();
    let d = decode(&bytes, &opts(OutputChannels::Rgb, false)).unwrap();
    assert_eq!(d.data, [0.5, 0.5, 0.5, 0.5, 0.5, 0.5]);
}

#[test]
fn probe_and_detect() {
    assert!(probe(b"#?RADIANCE\n"));
    assert!(probe(b"PF\n1 1\n-1\n"));
    assert!(probe(b"Pf\n1 1\n-1\n"));
    assert!(!probe(b"P6\n1 1\n255\n"));
    assert!(!probe(b"#"));
    assert_eq!(detect(b"#?RGBE\n"), Some(Format::Radiance));
    assert_eq!(detect(b"PF4\n"), Some(Format::Pfm));
}

fn write_pfm(magic: &str, w: usize, h: usize, ch: usize, le: bool, vals: &[f32]) -> Vec<u8> {
    let mut out = format!("{magic}\n{w} {h}\n{}\n", if le { "-1.0" } else { "1.0" }).into_bytes();
    // Rows bottom-up.
    for y in (0..h).rev() {
        for v in &vals[y * w * ch..(y + 1) * w * ch] {
            out.extend_from_slice(&if le { v.to_le_bytes() } else { v.to_be_bytes() });
        }
    }
    out
}

#[test]
fn pfm_variants() {
    let (w, h) = (13, 7);
    for (magic, ch) in [("PF", 3), ("Pf", 1), ("PF4", 4)] {
        let vals: Vec<f32> = (0..w * h * ch).map(|i| i as f32 * 0.25 - 3.0).collect();
        for le in [true, false] {
            let bytes = write_pfm(magic, w, h, ch, le, &vals);
            let info = read_info(&bytes).unwrap();
            assert_eq!(info.format, Format::Pfm);
            assert_eq!(
                info.encoding,
                if le { Encoding::F32Le } else { Encoding::F32Be }
            );
            assert_eq!(info.scale, 1.0);
            for parallel in [true, false] {
                let d = decode(&bytes, &opts(OutputChannels::Rgba, parallel)).unwrap();
                assert!(!d.truncated);
                let want: Vec<f32> = vals
                    .chunks_exact(ch)
                    .flat_map(|p| match ch {
                        1 => [p[0], p[0], p[0], 1.0],
                        3 => [p[0], p[1], p[2], 1.0],
                        _ => [p[0], p[1], p[2], p[3]],
                    })
                    .collect();
                assert_eq!(d.data, want, "{magic} le={le}");
                let d = decode(&bytes, &opts(OutputChannels::Rgb, parallel)).unwrap();
                let rgb: Vec<f32> = want
                    .chunks_exact(4)
                    .flat_map(|p| [p[0], p[1], p[2]])
                    .collect();
                assert_eq!(d.data, rgb, "{magic} le={le} rgb");
            }
        }
    }
    // CRLF after the scale, and a truncated body.
    let mut bytes = b"PF\r\n2 1\r\n-1.0\r\n".to_vec();
    bytes.extend_from_slice(&1.5f32.to_le_bytes());
    let d = decode(&bytes, &opts(OutputChannels::Rgb, false)).unwrap();
    assert!(d.truncated);
    assert_eq!(d.data[0], 1.5);
}

#[test]
fn limits() {
    let bytes = b"#?RADIANCE\n\n-Y 100000 +X 100000\n".to_vec();
    assert!(matches!(
        decode(&bytes, &DecodeOptions::default()),
        Err(Error::TooLarge { .. })
    ));
    let small = Limits {
        max_width: 4,
        max_height: 4,
        max_pixels: 16,
    };
    let img = rgbe_image(5, 2, 1);
    let bytes = write_hdr(&img, 5, 2, ("-Y", "+X"), Rle::Flat, "");
    let o = DecodeOptions {
        limits: small,
        ..Default::default()
    };
    assert!(matches!(decode(&bytes, &o), Err(Error::TooLarge { .. })));
    // A huge PFM header over a tiny body: refused by the limits, not allocated.
    assert!(
        decode(
            b"PF\n4000000000 4000000000\n-1\n",
            &DecodeOptions::default()
        )
        .is_err()
    );
}

/// Every truncation and many random corruptions: never panic, output size always consistent.
fn hammer(bytes: &[u8], seed: u64) {
    let o = opts(OutputChannels::Rgba, true);
    let step = (bytes.len() / 400).max(1);
    for n in (0..bytes.len()).step_by(step) {
        if let Ok(d) = decode(&bytes[..n], &o) {
            assert_eq!(d.data.len(), d.width as usize * d.height as usize * 4);
        }
    }
    let mut rng = Rng(seed);
    for _ in 0..600 {
        let mut b = bytes.to_vec();
        for _ in 0..1 + rng.next() % 8 {
            let i = (rng.next() % b.len() as u64) as usize;
            b[i] = rng.byte();
        }
        if let Ok(d) = decode(&b, &o) {
            assert_eq!(d.data.len(), d.width as usize * d.height as usize * 4);
        }
    }
}

#[test]
fn robustness() {
    let img = rgbe_image(40, 12, 5);
    for rle in [Rle::Flat, Rle::Old, Rle::New] {
        for axes in [("-Y", "+X"), ("+X", "+Y")] {
            hammer(&write_hdr(&img, 40, 12, axes, rle, "EXPOSURE=1\n"), 99);
        }
    }
    let vals: Vec<f32> = (0..30 * 9 * 3).map(|i| i as f32).collect();
    hammer(&write_pfm("PF", 30, 9, 3, true, &vals), 7);
    hammer(&write_pfm("Pf", 90, 9, 1, false, &vals), 8);
}
