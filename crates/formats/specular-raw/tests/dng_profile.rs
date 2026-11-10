//! DNG camera profiles with look tables (ProfileHueSatMap, ProfileLookTable): applied when
//! valid, ColorMatrix fallback when an Adobe profile's tables are missing or malformed.

mod common;

use common::{
    DngCodec, DngSpec, SRGB_TO_XYZD50, Tag, XYZ_TO_SRGB, demo_profile, hsv_table, tag_ascii,
    tag_floats, tag_longs, write_dng,
};
use specular_raw::{DecodeOptions, MatrixSource, RgbImage};

const W: u32 = 8;
const H: u32 = 8;

/// LinearRaw DNG whose camera space is linear sRGB, filled with `rgb` (0..1), with `extra` tags.
fn render(rgb: [f32; 3], extra: Vec<Tag>) -> RgbImage {
    let mut s = DngSpec::new(W, H, DngCodec::U16);
    s.linear_raw = true;
    s.color_matrix = XYZ_TO_SRGB;
    s.extra = extra;
    let px = rgb.map(|v| (v * 65535.0).round() as u16);
    let data: Vec<u16> = (0..W * H).flat_map(|_| px).collect();
    let file = write_dng(&s, &data);
    specular_raw::parse(&file)
        .unwrap()
        .decode(&file, &DecodeOptions::default())
        .unwrap()
}

fn centre(img: &RgbImage) -> [f32; 3] {
    let i = ((img.height / 2 * img.width + img.width / 2) * 3) as usize;
    std::array::from_fn(|c| img.data[i + c] as f32 / 65535.0)
}

fn close(a: [f32; 3], b: [f32; 3], tol: f32) -> bool {
    a.iter().zip(&b).all(|(x, y)| (x - y).abs() <= tol)
}

fn fm_only() -> Vec<Tag> {
    vec![
        tag_floats(0xc714, &SRGB_TO_XYZD50),
        tag_ascii(0xc6f8, "Camera Embedded"),
    ]
}

/// Profile with a uniform HueSatMap entry.
fn with_hsm(entry: [f32; 3], name: &str) -> Vec<Tag> {
    let dims = [6, 2, 1];
    vec![
        tag_floats(0xc714, &SRGB_TO_XYZD50),
        tag_ascii(0xc6f8, name),
        tag_longs(0xc6f9, &dims),
        tag_floats(
            0xc6fa,
            &hsv_table(dims, |_, s, _| if s == 0 { [0.0, 1.0, 1.0] } else { entry }),
        ),
    ]
}

#[test]
fn forward_matrix_of_srgb_camera_is_identity() {
    let c = [0.5, 0.2, 0.1];
    let img = render(c, fm_only());
    assert_eq!(img.matrix_source, MatrixSource::DngForwardMatrix);
    assert!(close(centre(&img), c, 0.01), "{:?}", centre(&img));
}

#[test]
fn identity_tables_match_the_matrix() {
    let c = [0.5, 0.2, 0.1];
    let img = render(c, with_hsm([0.0, 1.0, 1.0], "Adobe Standard"));
    assert_eq!(img.matrix_source, MatrixSource::DngProfile);
    assert!(close(centre(&img), c, 0.01), "{:?}", centre(&img));
}

#[test]
fn hue_sat_map_shifts_hue() {
    // +120 degrees (in ProPhoto RGB): red becomes green.
    let img = render(
        [0.5, 0.1, 0.1],
        with_hsm([120.0, 1.0, 1.0], "Adobe Standard"),
    );
    assert_eq!(img.matrix_source, MatrixSource::DngProfile);
    let o = centre(&img);
    assert!(o[1] > 1.5 * o[0] && o[1] > 1.5 * o[2], "{o:?}");
    // Saturation scale falling from 1 (s = 0) to 0 (s = 1): much less saturated, same hue.
    let img = render([0.5, 0.1, 0.1], with_hsm([0.0, 0.0, 1.0], "Adobe Standard"));
    let o = centre(&img);
    assert!(
        o[0] - o[1] < 0.2 && o[0] > o[1] && (o[1] - o[2]).abs() < 0.01,
        "{o:?}"
    );
    // Neutral input is untouched by any table.
    let img = render([0.3; 3], demo_profile());
    assert_eq!(img.matrix_source, MatrixSource::DngProfile);
    assert!(close(centre(&img), [0.3; 3], 0.01), "{:?}", centre(&img));
}

#[test]
fn adobe_profile_without_tables_uses_color_matrix() {
    let mut tags = fm_only();
    tags[1] = tag_ascii(0xc6f8, "Adobe Standard");
    assert_eq!(
        render([0.4; 3], tags).matrix_source,
        MatrixSource::DngColorMatrix
    );
    // Copyright names Adobe, profile name does not.
    let mut tags = fm_only();
    tags.push(tag_ascii(0xc6fe, "Copyright 2019 Adobe Systems, Inc."));
    assert_eq!(
        render([0.4; 3], tags).matrix_source,
        MatrixSource::DngColorMatrix
    );
}

#[test]
fn malformed_tables_fall_back_to_color_matrix() {
    let good = with_hsm([10.0, 1.1, 1.0], "Camera Embedded");
    type Corrupt = Box<dyn Fn(&mut Vec<Tag>)>;
    let cases: Vec<(&str, Corrupt)> = vec![
        ("short data", Box::new(|t| t[3].2 -= 1)),
        (
            "long data",
            Box::new(|t| t[3] = tag_floats(0xc6fa, &[1.0; 6 * 2 * 3 + 3])),
        ),
        (
            "sat dims 1",
            Box::new(|t| t[2] = tag_longs(0xc6f9, &[12, 1, 1])),
        ),
        (
            "zero dims",
            Box::new(|t| t[2] = tag_longs(0xc6f9, &[0, 0, 0])),
        ),
        (
            "huge dims",
            Box::new(|t| t[2] = tag_longs(0xc6f9, &[u32::MAX, u32::MAX, 3])),
        ),
        ("two dims", Box::new(|t| t[2] = tag_longs(0xc6f9, &[6, 2]))),
        (
            "no dims",
            Box::new(|t| t[2] = tag_longs(0xc5f9, &[6, 2, 1])),
        ),
        (
            "NaN entry",
            Box::new(|t| t[3] = tag_floats(0xc6fa, &[f32::NAN; 6 * 2 * 3])),
        ),
        (
            "negative scale",
            Box::new(|t| t[3] = tag_floats(0xc6fa, &[-1.0; 6 * 2 * 3])),
        ),
        (
            "bad encoding",
            Box::new(|t| t.push(tag_longs(0xc7a3, &[7]))),
        ),
        (
            "bad look table",
            Box::new(|t| {
                t.push(tag_longs(0xc725, &[4, 2, 3]));
                t.push(tag_floats(0xc726, &[1.0; 5]));
            }),
        ),
    ];
    assert_eq!(
        render([0.4, 0.2, 0.1], good.clone()).matrix_source,
        MatrixSource::DngProfile
    );
    for (name, f) in cases {
        let mut t = good.clone();
        f(&mut t);
        let img = render([0.4, 0.2, 0.1], t);
        assert_eq!(img.matrix_source, MatrixSource::DngColorMatrix, "{name}");
        // The ColorMatrix of an sRGB camera renders the input unchanged.
        assert!(close(centre(&img), [0.4, 0.2, 0.1], 0.01), "{name}");
    }
}

#[test]
fn tables_interpolate_between_illuminants() {
    // Illuminant 1 = A (+60 degrees), 2 = D65 (0 degrees); the shot is D65-neutral, so table 2.
    let dims = [6, 2, 1];
    let table = |shift: f32| hsv_table(dims, move |_, s, _| [shift * s as f32, 1.0, 1.0]);
    let tags = vec![
        tag_floats(0xc621, &XYZ_TO_SRGB.map(|v| v as f32)),
        tag_floats(0xc622, &XYZ_TO_SRGB.map(|v| v as f32)),
        (0xc65a, 3, 1, 17u16.to_le_bytes().to_vec()),
        (0xc65b, 3, 1, 21u16.to_le_bytes().to_vec()),
        tag_floats(0xc714, &SRGB_TO_XYZD50),
        tag_floats(0xc715, &SRGB_TO_XYZD50),
        tag_longs(0xc6f9, &dims),
        tag_floats(0xc6fa, &table(60.0)),
        tag_floats(0xc6fb, &table(0.0)),
    ];
    let c = [0.5, 0.1, 0.1];
    let img = render(c, tags.clone());
    assert_eq!(img.matrix_source, MatrixSource::DngProfile);
    assert!(close(centre(&img), c, 0.01), "{:?}", centre(&img));
    // Swapping the tables: the full shift applies, as with a single 60-degree table.
    let mut swapped = tags.clone();
    swapped[7] = tag_floats(0xc6fa, &table(0.0));
    swapped[8] = tag_floats(0xc6fb, &table(60.0));
    let o = centre(&render(c, swapped));
    let mut single = tags;
    single.truncate(8);
    single[7] = tag_floats(0xc6fa, &table(60.0));
    let want = centre(&render(c, single));
    assert!(!close(want, c, 0.05), "{want:?}");
    assert!(close(o, want, 0.005), "{o:?} {want:?}");
}

// ---------------------------------------------------------------------------------------------
// Oracle: Adobe's dng_validate (DNG SDK) against our rendering of the same synthetic DNG.

type M3 = [[f64; 3]; 3];

fn mulv(m: &M3, v: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| (0..3).map(|k| m[i][k] * v[k]).sum())
}

/// Linear sRGB (D65) → XYZ (D50), Bradford.
const SRGB_TO_D50: M3 = [
    [0.4360747, 0.3850649, 0.1430804],
    [0.2225045, 0.7168786, 0.0606169],
    [0.0139322, 0.0971045, 0.7141733],
];

const CHECKER_LAB: [[f64; 3]; 24] = [
    [37.54, 14.37, 14.92],
    [64.66, 19.27, 17.5],
    [49.32, -3.82, -22.54],
    [43.46, -12.74, 22.72],
    [54.94, 9.61, -24.79],
    [70.48, -32.26, -0.37],
    [62.73, 35.83, 56.5],
    [39.43, 10.75, -45.17],
    [50.57, 48.64, 16.67],
    [30.1, 22.54, -20.87],
    [71.77, -24.13, 58.19],
    [71.51, 18.24, 67.37],
    [28.37, 15.42, -49.8],
    [54.38, -39.72, 32.27],
    [42.43, 51.05, 28.62],
    [81.8, 2.67, 80.41],
    [50.63, 51.28, -14.12],
    [49.57, -29.71, -28.32],
    [95.19, -1.03, 2.93],
    [81.29, -0.57, 0.44],
    [66.89, -0.75, -0.06],
    [50.76, -0.13, 0.14],
    [35.63, -0.46, -0.48],
    [20.64, 0.07, -0.46],
];
const D50: [f64; 3] = [0.9642, 1.0, 0.8249];

fn lab_to_xyz(l: [f64; 3]) -> [f64; 3] {
    let fy = (l[0] + 16.0) / 116.0;
    let f = [fy + l[1] / 500.0, fy, fy - l[2] / 200.0];
    std::array::from_fn(|i| {
        let t = f[i];
        let v = if t > 6.0 / 29.0 {
            t * t * t
        } else {
            3.0 * (6.0f64 / 29.0).powi(2) * (t - 4.0 / 29.0)
        };
        v * D50[i]
    })
}

fn rgb_to_lab(rgb: [f64; 3]) -> [f64; 3] {
    let x = mulv(&SRGB_TO_D50, rgb);
    let f = |t: f64| {
        if t > (6.0f64 / 29.0).powi(3) {
            t.cbrt()
        } else {
            t / (3.0 * (6.0f64 / 29.0).powi(2)) + 4.0 / 29.0
        }
    };
    let [fx, fy, fz] = std::array::from_fn(|i| f(x[i] / D50[i]));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

fn delta_e(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

/// IFD 0 tags of a TIFF as `(tag, type, count, little-endian value bytes)`.
fn tiff_tags(d: &[u8]) -> Vec<Tag> {
    let le = d[0] == b'I';
    let u16_at = |o: usize| {
        let b = [d[o], d[o + 1]];
        if le {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        }
    };
    let u32_at = |o: usize| {
        let b = [d[o], d[o + 1], d[o + 2], d[o + 3]];
        if le {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        }
    };
    let ifd = u32_at(4) as usize;
    let mut out = Vec::new();
    for i in 0..u16_at(ifd) as usize {
        let e = ifd + 2 + i * 12;
        let (tag, typ, count) = (u16_at(e), u16_at(e + 2), u32_at(e + 4));
        let unit = match typ {
            1 | 2 | 6 | 7 => 1,
            3 | 8 => 2,
            4 | 9 | 11 | 13 => 4,
            5 | 10 => 4, // two 32-bit halves
            12 => 8,
            _ => continue,
        };
        let len = count as usize * if matches!(typ, 5 | 10) { 8 } else { unit };
        let off = if len <= 4 {
            e + 8
        } else {
            u32_at(e + 8) as usize
        };
        let Some(raw) = d.get(off..off + len) else {
            continue;
        };
        let mut v = raw.to_vec();
        if !le {
            for c in v.chunks_mut(unit) {
                c.reverse();
            }
        }
        out.push((tag, typ, count, v));
    }
    out
}

fn tag_f64s(t: &Tag) -> Vec<f64> {
    let b = &t.3;
    match t.1 {
        10 => b
            .chunks_exact(8)
            .map(|c| {
                i32::from_le_bytes(c[..4].try_into().unwrap()) as f64
                    / i32::from_le_bytes(c[4..].try_into().unwrap()) as f64
            })
            .collect(),
        5 => b
            .chunks_exact(8)
            .map(|c| {
                u32::from_le_bytes(c[..4].try_into().unwrap()) as f64
                    / u32::from_le_bytes(c[4..].try_into().unwrap()) as f64
            })
            .collect(),
        11 => b
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()) as f64)
            .collect(),
        3 => b
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes(c.try_into().unwrap()) as f64)
            .collect(),
        _ => Vec::new(),
    }
}

/// An uncompressed 16-bit RGB TIFF (dng_validate output) as `(width, height, samples)`.
fn read_tiff16(d: &[u8]) -> (usize, usize, Vec<u16>) {
    let tags = tiff_tags(d);
    let le = d[0] == b'I';
    let get = |tag: u16| -> Vec<u32> {
        let t = tags.iter().find(|t| t.0 == tag).unwrap();
        match t.1 {
            3 => {
                t.3.chunks_exact(2)
                    .map(|c| u16::from_le_bytes(c.try_into().unwrap()) as u32)
                    .collect()
            }
            _ => {
                t.3.chunks_exact(4)
                    .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                    .collect()
            }
        }
    };
    let (w, h) = (get(0x100)[0] as usize, get(0x101)[0] as usize);
    assert_eq!(get(0x103)[0], 1, "compressed TIFF");
    assert_eq!(get(0x102)[0], 16);
    assert_eq!(get(0x115)[0], 3);
    let mut px = Vec::with_capacity(w * h * 3);
    for (o, n) in get(0x111).iter().zip(get(0x117)) {
        for c in d[*o as usize..(*o + n) as usize].chunks_exact(2) {
            let b = [c[0], c[1]];
            px.push(if le {
                u16::from_le_bytes(b)
            } else {
                u16::from_be_bytes(b)
            });
        }
    }
    px.truncate(w * h * 3);
    (w, h, px)
}

const PATCH: u32 = 8;

/// Mean colour of each ColorChecker patch (6 x 4 grid of `PATCH` pixels), centre 4 x 4.
fn patches(w: usize, px: &[f64]) -> Vec<[f64; 3]> {
    (0..24)
        .map(|i| {
            let (x0, y0) = ((i % 6) * PATCH as usize + 2, (i / 6) * PATCH as usize + 2);
            let mut s = [0.0; 3];
            for y in y0..y0 + 4 {
                for x in x0..x0 + 4 {
                    for c in 0..3 {
                        s[c] += px[(y * w + x) * 3 + c] / 16.0;
                    }
                }
            }
            s
        })
        .collect()
}

fn ours(file: &[u8], gain: f64) -> Vec<[f64; 3]> {
    let img = specular_raw::parse(file)
        .unwrap()
        .decode(file, &DecodeOptions::default())
        .unwrap();
    let lin: Vec<f64> = img
        .data
        .iter()
        .map(|&v| (v as f64 / 65535.0 * gain).min(1.0))
        .collect();
    patches(img.width as usize, &lin)
}

/// Converter DNG profiles rendered by Adobe's DNG SDK (`dng_validate`, path in
/// `SPECULAR_DNG_VALIDATE`) and by us, on synthetic LinearRaw DNGs that carry the profile, an
/// identity ProfileToneCurve and DefaultBlackRender = None so both pipelines are comparable.
/// Prints mean / max CIE76 ΔE per body for our full profile, ForwardMatrix only and ColorMatrix
/// only: at BaselineExposure 0 and at the file's BaselineExposure under a daylight white, and
/// under a warm white that interpolates between the calibrations. `dng_validate` is built from
/// Adobe's DNG SDK (1.7.1 was used; it needs libjxl, expat and zlib).
/// Negative exposure is printed but not asserted: the SDK compresses highlights there with an
/// exposure curve that belongs to a tone stage we do not have.
#[test]
#[ignore = "needs dng_validate (SPECULAR_DNG_VALIDATE) and the corpus"]
fn matches_dng_sdk_rendering() {
    let Ok(exe) = std::env::var("SPECULAR_DNG_VALIDATE") else {
        eprintln!("skipped: SPECULAR_DNG_VALIDATE unset (dng_validate from Adobe's DNG SDK)");
        return;
    };
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("dng-profile-oracle");
    std::fs::create_dir_all(&dir).unwrap();
    let corpus =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/large/raw");
    let only = std::env::var("SPECULAR_ORACLE_ONLY").ok();
    let tsv = include_str!("../../../../tools/dng-colour-sources.tsv");
    const COPY: [u16; 15] = [
        0xc621, 0xc622, 0xc714, 0xc715, 0xc65a, 0xc65b, 0xc6f8, 0xc6f9, 0xc6fa, 0xc6fb, 0xc6fe,
        0xc725, 0xc726, 0xc7a3, 0xc7a4,
    ];
    let mut sums = [0.0f64; 3];
    let mut worst = 0.0f64;
    let mut n = 0;
    let mut bad = Vec::new();
    for line in tsv.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
        let cols: Vec<&str> = line.split('\t').collect();
        let [prefix, dng_path, ..] = cols[..] else {
            continue;
        };
        if only.as_deref().is_some_and(|o| !prefix.contains(o)) {
            continue;
        }
        let Ok(src) = std::fs::read(corpus.join(dng_path)) else {
            continue;
        };
        let tags = tiff_tags(&src);
        let profile: Vec<_> = tags
            .iter()
            .filter(|t| COPY.contains(&t.0))
            .cloned()
            .collect();
        if !profile.iter().any(|t| t.0 == 0xc6fa) {
            continue;
        }
        let find = |tag: u16| tags.iter().find(|t| t.0 == tag).map(tag_f64s);
        let cm2 = find(0xc622).or_else(|| find(0xc621)).unwrap();
        let cm: M3 = std::array::from_fn(|i| std::array::from_fn(|j| cm2[i * 3 + j]));
        let be = find(0xc62a).map_or(0.0, |v| v[0]);
        // Patch camera values through the D65 ColorMatrix.
        let d50_to_d65: M3 = [
            [0.9555766, -0.0230393, 0.0631636],
            [-0.0282895, 1.0099416, 0.0210077],
            [0.0122982, -0.0204830, 1.3299098],
        ];
        let cams: Vec<[f64; 3]> = CHECKER_LAB
            .iter()
            .map(|&l| mulv(&cm, mulv(&d50_to_d65, lab_to_xyz(l))))
            .collect();
        // A daylight white balance, and a warm one (about 3200 K) that makes both renderers
        // interpolate between the two calibration illuminants.
        let neutral_of = |xyz: [f64; 3]| {
            let n = mulv(&cm, xyz);
            let nmax = n.iter().copied().fold(0.0, f64::max);
            n.map(|v| v / nmax)
        };
        let neutrals = [
            neutral_of([0.95047, 1.0, 1.08883]),
            neutral_of([1.0611, 1.0, 0.4452]),
        ];
        let peak = cams
            .iter()
            .flat_map(|c| {
                neutrals
                    .iter()
                    .flat_map(move |n| (0..3).map(move |i| c[i] / n[i]))
            })
            .fold(0.0, f64::max);
        let (w, h) = (6 * PATCH, 4 * PATCH);
        let mut data = vec![0u16; (w * h * 3) as usize];
        for y in 0..h {
            for x in 0..w {
                let c = cams[(y / PATCH * 6 + x / PATCH) as usize];
                for k in 0..3 {
                    let v = (c[k] / peak * 0.9).clamp(0.0, 1.0);
                    data[((y * w + x) * 3) as usize + k] = (v * 65535.0).round() as u16;
                }
            }
        }
        let synth = |extra: &[Tag], exposure: f64, neutral: [f64; 3]| {
            let mut s = DngSpec::new(w, h, DngCodec::U16);
            s.linear_raw = true;
            s.neutral = neutral;
            s.extra = extra.to_vec();
            s.extra.push(tag_floats(0xc6fc, &[0.0, 0.0, 1.0, 1.0]));
            s.extra.push(tag_longs(0xc7a6, &[1]));
            s.extra.push((0xc62a, 10, 1, {
                let mut b = ((exposure * 1e4).round() as i32).to_le_bytes().to_vec();
                b.extend(10000i32.to_le_bytes());
                b
            }));
            write_dng(&s, &data)
        };
        for (exposure, warm) in [(0.0, false), (be, false), (0.0, true)] {
            let neutral = neutrals[warm as usize];
            let full = synth(&profile, exposure, neutral);
            let name = format!(
                "{}-ev{:+04}{}",
                prefix.replace(' ', "_"),
                (exposure * 100.0) as i32,
                if warm { "-warm" } else { "" }
            );
            let base = dir.join(&name);
            let path = dir.join(format!("{name}.dng"));
            std::fs::write(&path, &full).unwrap();
            let st = std::process::Command::new(&exe)
                .args(["-16", "-cs1", "-tif"])
                .arg(&base)
                .arg(&path)
                .output()
                .unwrap();
            assert!(
                st.status.success(),
                "{}",
                String::from_utf8_lossy(&st.stderr)
            );
            let tif = std::fs::read(dir.join(format!("{name}.tif"))).unwrap();
            let (tw, _, px) = read_tiff16(&tif);
            let decode = |v: u16| {
                let e = v as f64 / 65535.0;
                if e <= 0.04045 {
                    e / 12.92
                } else {
                    ((e + 0.055) / 1.055).powf(2.4)
                }
            };
            let sdk = patches(tw, &px.iter().map(|&v| decode(v)).collect::<Vec<_>>());
            // Our renderings: full profile, ForwardMatrix alone, ColorMatrix alone.
            let no_tables: Vec<_> = profile
                .iter()
                .filter(|t| !matches!(t.0, 0xc6f8 | 0xc6fe | 0xc6f9..=0xc6fb | 0xc725 | 0xc726))
                .cloned()
                .collect();
            let no_fm: Vec<_> = no_tables
                .iter()
                .filter(|t| !matches!(t.0, 0xc714 | 0xc715))
                .cloned()
                .collect();
            let gain = 2f64.powf(exposure.clamp(-4.0, 4.0));
            let mut line = format!(
                "{prefix:28} EV {exposure:+.2} {}",
                if warm { "warm" } else { "D65 " }
            );
            let mut status = "pass";
            for (k, tags) in [&profile, &no_tables, &no_fm].into_iter().enumerate() {
                let o = ours(&synth(tags, exposure, neutral), gain);
                let de: Vec<f64> = (0..24)
                    .map(|i| delta_e(rgb_to_lab(o[i]), rgb_to_lab(sdk[i])))
                    .collect();
                let mean = de.iter().sum::<f64>() / 24.0;
                let max = de.iter().copied().fold(0.0, f64::max);
                line += &format!("  {} {mean:5.2}/{max:5.2}", ["full", "FM", "CM"][k]);
                sums[k] += mean;
                // Negative exposure goes through the SDK's highlight-compressing exposure
                // curve, which belongs to the tone stage we do not have; the profile itself
                // must match exactly.
                // A warm white interpolates between the calibrations; our white temperature
                // differs from the SDK's by a few kelvin there.
                if k == 0 && exposure >= 0.0 {
                    worst = worst.max(max);
                    let (lm, lx) = if warm { (0.15, 0.6) } else { (0.05, 0.1) };
                    if !(mean < lm && max < lx) {
                        status = "fail";
                        bad.push(format!("{prefix} EV {exposure}: {mean}/{max}"));
                    }
                }
            }
            eprintln!("{line}");
            // One line per rendering for tools/ci/oracle-pass-rates.sh.
            println!("ORACLE\tDNG profile\t{name}\t{status}");
            n += 1;
        }
    }
    assert!(n > 0, "no converter DNGs found");
    assert!(bad.is_empty(), "{bad:#?}");
    let n = n as f64;
    eprintln!(
        "{n} renderings: mean ΔE vs DNG SDK: full profile {:.2} (worst patch at EV ≥ 0: {worst:.2}), \
         ForwardMatrix only {:.2}, ColorMatrix only {:.2}",
        sums[0] / n,
        sums[1] / n,
        sums[2] / n
    );
}

/// A NoiseProfile tag (DOUBLE pairs) as stored.
fn noise_tag(v: &[f64]) -> Tag {
    let d = v.iter().flat_map(|x| x.to_le_bytes()).collect();
    (0xc761, 12, v.len() as u32, d)
}

fn noise_profile(extra: Vec<Tag>) -> Vec<[f64; 2]> {
    let mut s = DngSpec::new(W, H, DngCodec::U16);
    s.extra = extra;
    let data = vec![1000u16; (W * H) as usize];
    let file = write_dng(&s, &data);
    let sensor = specular_raw::parse(&file)
        .unwrap()
        .decode_sensor(&file, &DecodeOptions::default())
        .unwrap();
    sensor.meta.dng.expect("DNG colour").noise_profile
}

#[test]
fn noise_profile_is_read_per_plane_or_shared() {
    assert_eq!(
        noise_profile(vec![noise_tag(&[2e-4, 3e-7])]),
        vec![[2e-4, 3e-7]]
    );
    let three = [1e-4, 1e-7, 2e-4, 2e-7, 3e-4, 3e-7];
    assert_eq!(
        noise_profile(vec![noise_tag(&three)]),
        vec![[1e-4, 1e-7], [2e-4, 2e-7], [3e-4, 3e-7]]
    );
    // Absent, odd counts and non-positive scales are no profile.
    assert!(noise_profile(Vec::new()).is_empty());
    assert!(noise_profile(vec![noise_tag(&[2e-4, 3e-7, 1e-4])]).is_empty());
    assert!(noise_profile(vec![noise_tag(&[0.0, 3e-7])]).is_empty());
}
