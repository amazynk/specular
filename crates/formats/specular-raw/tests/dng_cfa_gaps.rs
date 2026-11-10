//! DNG CFA data beyond the plain rectangular red/green/blue mosaic, as the DNG specification
//! (1.7) defines it: staggered `CFALayout`s 2..=9, four colour planes with the file's own
//! 4 x 3 `ColorMatrix`, and more than two samples per photosite. Synthetic files whose true
//! scene is known, so the decode is checked against the scene itself.

mod common;

use common::{DngCodec, DngSpec, Tag, XYZ_TO_SRGB, tag_longs, write_dng};
use specular_raw::{DecodeOptions, MatrixSource, RawImage};

/// (S)RATIONAL values over 10000, as DNG writes ColorMatrix (type 10) and AsShotNeutral (5).
fn tag_ratios(tag: u16, typ: u16, v: &[f32]) -> Tag {
    let d = v
        .iter()
        .flat_map(|x| {
            [
                ((x * 10000.0).round() as i32).to_le_bytes(),
                10000i32.to_le_bytes(),
            ]
        })
        .flatten()
        .collect();
    (tag, typ, v.len() as u32, d)
}

fn tag_shorts(tag: u16, v: &[u16]) -> Tag {
    (
        tag,
        3,
        v.len() as u32,
        v.iter().flat_map(|x| x.to_le_bytes()).collect(),
    )
}

fn tag_bytes(tag: u16, v: &[u8]) -> Tag {
    (tag, 1, v.len() as u32, v.to_vec())
}

fn reason(file: &[u8]) -> Option<&'static str> {
    specular_raw::parse(file)
        .unwrap()
        .info()
        .sensor
        .clone()
        .unwrap()
        .unsupported_reason
}

fn decode_raw(file: &[u8]) -> RawImage {
    let raw = specular_raw::parse(file).unwrap();
    raw.decode_raw(file, &DecodeOptions::default()).unwrap()
}

// ---------------------------------------------------------------------------------------------
// Staggered layouts

/// Where the DNG specification puts the photosite stored at (row, col) for each layout, in
/// stored cells, written out from the specification's wording ("even" from 0).
fn spec_position(layout: u32, row: usize, col: usize) -> (f64, f64) {
    let (er, ec) = (row.is_multiple_of(2), col.is_multiple_of(2));
    let h = |b: bool| if b { 0.5 } else { 0.0 };
    let (y, x) = (row as f64, col as f64);
    match layout {
        1 => (y, x),
        2 => (y + h(ec), x),         // A: even columns offset down by 1/2 row
        3 => (y - h(ec), x),         // B: even columns offset up by 1/2 row
        4 => (y, x + h(er)),         // C: even rows offset right by 1/2 column
        5 => (y, x - h(er)),         // D: even rows offset left by 1/2 column
        6 => (y - h(er), x - h(ec)), // E: even rows up, even columns left
        7 => (y - h(er), x + h(ec)), // F: even rows up, even columns right
        8 => (y + h(er), x - h(ec)), // G: even rows down, even columns left
        9 => (y + h(er), x + h(ec)), // H: even rows down, even columns right
        _ => unreachable!(),
    }
}

/// The scene: smooth, different in each channel, steep along both axes.
fn scene(y: f64, x: f64) -> [f64; 3] {
    [
        3000.0 + 90.0 * y + 25.0 * x,
        5000.0 + 40.0 * y - 60.0 * x + 8.0 * (y * 0.7).sin() * 10.0,
        4000.0 - 70.0 * y + 55.0 * x,
    ]
}

const RGGB: [u8; 4] = [0, 1, 1, 2];
const BLACK: u32 = 256;

/// A Bayer DNG of `scene` sampled at each photosite's true position for `layout`, written as
/// `written` (the layout tag), so a file can also lie about its layout.
fn staggered_dng(w: u32, h: u32, layout: u32, written: u32) -> Vec<u8> {
    let mut s = DngSpec::new(w, h, DngCodec::U16);
    s.cfa = RGGB;
    s.black = BLACK;
    s.white = 65535;
    s.extra.push(tag_shorts(0xc617, &[written as u16]));
    let mut data = Vec::with_capacity((w * h) as usize);
    for row in 0..h as usize {
        for col in 0..w as usize {
            let (y, x) = spec_position(layout, row, col);
            let c = RGGB[(row & 1) * 2 + (col & 1)] as usize;
            data.push((scene(y, x)[c] + BLACK as f64).round() as u16);
        }
    }
    write_dng(&s, &data)
}

/// Mean absolute error of the decoded camera RGB against the scene at each photosite's true
/// position, away from the borders.
fn scene_error(img: &RawImage, layout: u32) -> f64 {
    assert_eq!((img.cpp, img.cfa.is_none()), (3, true));
    let (w, h) = (img.width as usize, img.height as usize);
    let (mut sum, mut n) = (0.0, 0usize);
    for row in 4..h - 4 {
        for col in 4..w - 4 {
            let (y, x) = spec_position(layout, row, col);
            let want = scene(y, x);
            for (c, &want) in want.iter().enumerate() {
                sum += (img.data[(row * w + col) * 3 + c] as f64 - want).abs();
                n += 1;
            }
        }
    }
    sum / n as f64
}

/// Every staggered layout decodes to camera RGB on the stored grid, with each colour
/// estimated at the photosite's true position: much closer to the scene than the same samples
/// taken as a rectangular grid, and close in absolute terms. It develops too.
#[test]
fn staggered_layouts_demosaic_by_true_position() {
    let (w, h) = (40, 32);
    for layout in 2..=9 {
        let file = staggered_dng(w, h, layout, layout);
        assert_eq!(reason(&file), None, "layout {layout}");
        let img = decode_raw(&file);
        assert_eq!((img.width, img.height), (w, h));
        let good = scene_error(&img, layout);
        // The same samples read as a rectangular grid (the file claiming layout 1) put each
        // neighbour half a cell off.
        let flat = decode_raw(&staggered_dng(w, h, layout, 1));
        assert_eq!(flat.cpp, 1);
        let rect = mosaic_rect_error(&flat, layout);
        eprintln!("layout {layout}: {good:.2} vs rectangular {rect:.2}");
        assert!(good < 3.0, "layout {layout}: error {good:.1}");
        assert!(
            good * 5.0 < rect,
            "layout {layout}: {good:.1} vs rectangular {rect:.1}"
        );
        let raw = specular_raw::parse(&file).unwrap();
        let rgb = raw.decode(&file, &DecodeOptions::default()).unwrap();
        assert_eq!((rgb.width, rgb.height), (w, h));
    }
}

/// The error of bilinear interpolation on the stored grid taken as rectangular (what a
/// decoder ignoring `CFALayout` does), measured against the true positions.
fn mosaic_rect_error(img: &RawImage, layout: u32) -> f64 {
    let (w, h) = (img.width as usize, img.height as usize);
    let at = |r: usize, c: usize| img.data[r * w + c] as f64 - BLACK as f64;
    let colour = |r: usize, c: usize| RGGB[(r & 1) * 2 + (c & 1)] as usize;
    let (mut sum, mut n) = (0.0, 0usize);
    for row in 4..h - 4 {
        for col in 4..w - 4 {
            let want = scene(
                spec_position(layout, row, col).0,
                spec_position(layout, row, col).1,
            );
            for (ch, &want) in want.iter().enumerate() {
                let (mut s, mut k) = (0.0, 0.0);
                for dy in -1i64..=1 {
                    for dx in -1i64..=1 {
                        let (r, c) = ((row as i64 + dy) as usize, (col as i64 + dx) as usize);
                        if colour(r, c) == ch {
                            s += at(r, c);
                            k += 1.0;
                        }
                    }
                }
                sum += (s / k - want).abs();
                n += 1;
            }
        }
    }
    sum / n as f64
}

/// A flat field stays flat on every layout; layouts the specification does not define, and
/// staggered patterns whose colours give no red, green and blue, are refused by name.
#[test]
fn staggered_flat_field_and_refusals() {
    for layout in 2..=9u16 {
        let mut s = DngSpec::new(24, 24, DngCodec::U16);
        s.black = 0;
        s.extra.push(tag_shorts(0xc617, &[layout]));
        let file = write_dng(&s, &vec![3000u16; 24 * 24]);
        let img = decode_raw(&file);
        assert!(img.data.iter().all(|&v| v == 3000), "layout {layout}");
    }
    let mut s = DngSpec::new(24, 24, DngCodec::U16);
    s.extra.push(tag_shorts(0xc617, &[10]));
    let file = write_dng(&s, &vec![3000u16; 24 * 24]);
    assert!(reason(&file).is_some_and(|r| r.contains("does not define")));
    // Cyan and magenta only: no green.
    let mut s = DngSpec::new(24, 24, DngCodec::U16);
    s.cfa = [0, 1, 1, 0];
    s.extra.push(tag_bytes(0xc616, &[3, 4, 2]));
    s.extra.push(tag_shorts(0xc617, &[2]));
    let file = write_dng(&s, &vec![3000u16; 24 * 24]);
    assert!(
        reason(&file).is_some_and(|r| r.contains("other than red")),
        "{:?}",
        reason(&file)
    );
}

/// The stagger's parity counts from the active area: a one-row margin above swaps which
/// stored rows are "even", and the decode follows.
#[test]
fn staggered_parity_follows_active_area() {
    let (w, h, layout) = (40u32, 33u32, 4u32);
    let mut s = DngSpec::new(w, h, DngCodec::U16);
    s.cfa = RGGB;
    s.black = BLACK;
    s.white = 65535;
    s.active_area = Some([1, 0, h, w]);
    s.extra.push(tag_shorts(0xc617, &[layout as u16]));
    let mut data = Vec::new();
    for row in 0..h as usize {
        for col in 0..w as usize {
            // Active row r = row - 1; the margin row repeats the first active row.
            let r = row.saturating_sub(1);
            let (y, x) = spec_position(layout, r, col);
            let c = RGGB[(r & 1) * 2 + (col & 1)] as usize;
            data.push((scene(y, x)[c] + BLACK as f64).round() as u16);
        }
    }
    let file = write_dng(&s, &data);
    let img = decode_raw(&file);
    let wi = w as usize;
    let mut err = 0.0;
    let mut n = 0;
    for row in 5..h as usize - 4 {
        for col in 4..wi - 4 {
            let (y, x) = spec_position(layout, row - 1, col);
            for (c, want) in scene(y, x).iter().enumerate() {
                err += (img.data[(row * wi + col) * 3 + c] as f64 - want).abs();
                n += 1;
            }
        }
    }
    let err = err / n as f64;
    assert!(err < 3.0, "error {err:.1}");
}

// ---------------------------------------------------------------------------------------------
// Four colour planes

/// Ideal filter responses to linear sRGB, by DNG plane colour code.
const RESPONSE: [[f64; 3]; 6] = [
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
    [0.0, 1.0, 1.0],
    [1.0, 0.0, 1.0],
    [1.0, 1.0, 0.0],
];

/// Cyan, magenta, yellow, green; and red, green, blue, cyan (whose plane indices equal their
/// colour codes, which LibRaw's DNG reader assumes).
const CMYG: [u8; 4] = [3, 4, 5, 1];
const RGBC: [u8; 4] = [0, 1, 2, 3];

/// A four-plane DNG (`planes` = CFAPlaneColor, `pattern` = 2x2 CFAPattern of plane indices)
/// of one linear sRGB colour, with its own 4 x 3 ColorMatrix (each filter's response times
/// sRGB's XYZ inverse) and a four-value AsShotNeutral for D65 white.
fn four_plane_dng(planes: [u8; 4], pattern: [u8; 4], rgb: [f64; 3]) -> Vec<u8> {
    let resp = planes.map(|c| RESPONSE[c as usize]);
    let mut m = Vec::new();
    for r in resp {
        for j in 0..3 {
            m.push((0..3).map(|k| r[k] * XYZ_TO_SRGB[k * 3 + j]).sum::<f64>() as f32);
        }
    }
    let mut s = DngSpec::new(32, 32, DngCodec::U16);
    s.black = 0;
    s.white = 65535;
    s.cfa = pattern;
    s.extra.push(tag_bytes(0xc616, &planes));
    s.extra.push(tag_ratios(0xc621, 10, &m));
    // White reads the sum of each filter's responses.
    let neutral = resp.map(|r| (r.iter().sum::<f64>() / 2.0) as f32);
    s.extra.push(tag_ratios(0xc628, 5, &neutral));
    let read = resp.map(|r| (0..3).map(|k| r[k] * rgb[k]).sum::<f64>());
    let data: Vec<u16> = (0..32 * 32)
        .map(|i| read[pattern[(i / 32 % 2) * 2 + i % 2] as usize].round() as u16)
        .collect();
    write_dng(&s, &data)
}

/// (planes, pattern, colour) of the four-plane checks: grey, orange, teal on CMYG and RGBC.
fn four_plane_cases() -> Vec<([u8; 4], [u8; 4], [f64; 3])> {
    let colours = [
        [8000.0, 8000.0, 8000.0],
        [12000.0, 6000.0, 2000.0],
        [1500.0, 9000.0, 4000.0],
    ];
    [(CMYG, [0, 1, 2, 3]), (RGBC, [0, 1, 3, 2])]
        .into_iter()
        .flat_map(|(p, q)| colours.map(|c| (p, q, c)))
        .collect()
}

/// A four-plane DNG converts by its own ColorMatrix: the scene's linear sRGB comes back in
/// the right proportions, and white stays white.
#[test]
fn four_plane_dng_uses_its_own_matrix() {
    for (planes, pattern, rgb) in four_plane_cases() {
        let file = four_plane_dng(planes, pattern, rgb);
        assert_eq!(reason(&file), None);
        let img = decode_raw(&file);
        assert_eq!(img.matrix_source, MatrixSource::Published);
        assert_eq!(img.cpp, 3);
        let px = &img.data[(16 * 32 + 16) * 3..][..3];
        let k = px[1] as f64 / rgb[1];
        for c in 0..3 {
            let want = rgb[c] * k;
            assert!(
                (px[c] as f64 - want).abs() <= 0.01 * want + 2.0,
                "{rgb:?}: {px:?}"
            );
        }
    }
}

/// Without a 4 x 3 matrix a four-plane DNG still decodes (ideal filter responses); its 3 x 3
/// colour tags are not misread as a camera matrix.
#[test]
fn four_plane_dng_without_matrix() {
    let mut s = DngSpec::new(32, 32, DngCodec::U16);
    s.black = 0;
    s.cfa = [0, 1, 2, 3];
    s.extra.push(tag_bytes(0xc616, &[3, 4, 5, 1]));
    let file = write_dng(&s, &vec![4000u16; 32 * 32]);
    let img = decode_raw(&file);
    assert_eq!(img.cpp, 3);
    assert_ne!(img.matrix_source, MatrixSource::DngColorMatrix);
}

/// Red, green, blue and white (an RGBW sensor) decode too.
#[test]
fn rgbw_dng_decodes() {
    let mut s = DngSpec::new(32, 32, DngCodec::U16);
    s.black = 0;
    s.cfa = [0, 1, 3, 2];
    s.extra.push(tag_bytes(0xc616, &[0, 1, 2, 6]));
    let file = write_dng(&s, &vec![4000u16; 32 * 32]);
    assert_eq!(reason(&file), None);
    let img = decode_raw(&file);
    let px = &img.data[(16 * 32 + 16) * 3..][..3];
    assert!(px.iter().all(|&v| v == px[0]), "{px:?}");
}

// ---------------------------------------------------------------------------------------------
// Several samples per photosite

/// A DNG with `n` samples per photosite (`w` x `h` photosites).
fn multi_sample_dng(n: u32, black: &[u32]) -> (Vec<u8>, Vec<u16>) {
    let (w, h) = (16u32, 8u32);
    let mut s = DngSpec::new(n * w, h, DngCodec::U16);
    s.white = 16383;
    s.extra.push(tag_longs(0x100, &[w]));
    s.extra.push(tag_shorts(0x115, &[n as u16]));
    s.extra.push(tag_longs(0xc61a, black));
    let data: Vec<u16> = (0..n * w * h).map(|i| 600 + (i % 11) as u16).collect();
    (write_dng(&s, &data), data)
}

/// Three and four samples per photosite are added up like dual-pixel data; black (given per
/// sample) and white scale with them; past 16 bits the mean is kept instead.
#[test]
fn more_than_two_samples_add_up() {
    for (n, black) in [(3u32, vec![500, 510, 520]), (4, vec![512])] {
        let (file, data) = multi_sample_dng(n, &black);
        assert_eq!(reason(&file), None, "{n}");
        let img = decode_raw(&file);
        assert_eq!(img.cpp, 1);
        let want: Vec<u16> = data
            .chunks_exact(n as usize)
            .map(|p| p.iter().sum())
            .collect();
        assert_eq!(img.data, want, "{n}");
        let b = black.iter().sum::<u32>() as f32 / black.len() as f32 * n as f32;
        assert_eq!((img.white, img.black), (16383.0 * n as f32, [b; 4]), "{n}");
        let raw = specular_raw::parse(&file).unwrap();
        raw.decode(&file, &DecodeOptions::default()).unwrap();
    }
    // Five 14-bit samples overflow 16 bits: their mean.
    let (file, data) = multi_sample_dng(5, &[512]);
    let img = decode_raw(&file);
    let want: Vec<u16> = data
        .chunks_exact(5)
        .map(|p| ((p.iter().map(|&v| v as u32).sum::<u32>() + 2) / 5) as u16)
        .collect();
    assert_eq!(img.data, want);
    assert_eq!(img.white, 16383.0);
    // Past 16 samples: refused by name.
    let (file, _) = multi_sample_dng(17, &[0]);
    assert!(reason(&file).is_some_and(|r| r.contains("16 samples")));
}

/// Develop's sensor path gets these as already demosaiced (linear camera RGB, as LinearRaw
/// DNGs), and multi-sample data as one Bayer sample per photosite.
#[test]
fn develop_sensor_path_layouts() {
    use specular_raw::sensor::Layout;
    let opts = DecodeOptions::default();
    let linear = [
        staggered_dng(40, 32, 2, 2),
        four_plane_dng(CMYG, [0, 1, 2, 3], [9000.0, 5000.0, 3000.0]),
    ];
    for file in linear {
        let s = specular_raw::parse(&file)
            .unwrap()
            .decode_sensor(&file, &opts)
            .unwrap();
        assert_eq!((s.cpp, &s.meta.layout), (3, &Layout::Linear));
    }
    let (file, _) = multi_sample_dng(4, &[512]);
    let s = specular_raw::parse(&file)
        .unwrap()
        .decode_sensor(&file, &opts)
        .unwrap();
    assert_eq!((s.cpp, s.width), (1, 16));
    assert!(matches!(s.meta.layout, Layout::Bayer(_)));
}

/// Writes the synthetic files and this decoder's developed centre pixel (linear Rec.709) to
/// `SPECULAR_RAWGAPS_DUMP` for the LibRaw comparison (`tools/quality/raw_gaps_oracle.py`).
#[test]
#[ignore = "writes oracle inputs; run by tools/quality/raw_gaps_oracle.py"]
fn dump_oracle_inputs() {
    let Some(dir) = std::env::var_os("SPECULAR_RAWGAPS_DUMP").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let mut lines = String::new();
    for (i, (planes, pattern, rgb)) in four_plane_cases().into_iter().enumerate() {
        let file = four_plane_dng(planes, pattern, rgb);
        let kind = if planes == CMYG { "cmyg" } else { "rgbc" };
        let name = format!("{kind}{i}.dng");
        std::fs::write(dir.join(&name), &file).unwrap();
        let out = specular_raw::parse(&file)
            .unwrap()
            .decode(&file, &DecodeOptions::default())
            .unwrap();
        let px = &out.data[(16 * 32 + 16) * 3..][..3];
        lines += &format!("{name} {} {} {}\n", px[0], px[1], px[2]);
    }
    std::fs::write(dir.join("specular.txt"), lines).unwrap();
}
