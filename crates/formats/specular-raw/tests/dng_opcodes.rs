//! DNG opcode lists: lists 1 and 2 on the sensor samples, list 3 in the develop, unknown
//! opcodes reported (region equality is in `region.rs`).

mod common;

use common::{
    DngCodec, DngSpec, OpParams, op_gain_map, op_vignette, op_warp, opcode_list, write_dng,
};
use specular_raw::{DecodeOptions, OutputSize, Rect, RgbImage};

fn flat(w: u32, h: u32, v: u16, extra: Vec<common::Tag>) -> Vec<u8> {
    let mut s = DngSpec::new(w, h, DngCodec::U16);
    s.extra = extra;
    write_dng(&s, &vec![v; (w * h) as usize])
}

fn at(raw: &specular_raw::RawImage, x: u32, y: u32) -> u16 {
    raw.data[(y * raw.width + x) as usize]
}

#[test]
fn vignette_and_gain_map_scale_the_samples() {
    // Gain 1 + 0.5 r^2: 1 at the centre, ~1.5 in the corners.
    let f = flat(
        64,
        48,
        20000,
        vec![opcode_list(
            2,
            vec![op_vignette([0.5, 0.0, 0.0, 0.0, 0.0], [0.5, 0.5])],
        )],
    );
    let raw = specular_raw::parse(&f).unwrap();
    assert_eq!(
        raw.info().sensor.as_ref().unwrap().opcodes,
        ["FixVignetteRadial"]
    );
    let img = raw.decode_raw(&f, &DecodeOptions::default()).unwrap();
    assert_eq!((img.black, img.white), ([0.0; 4], 65535.0));
    let (c, k) = (at(&img, 32, 24) as f64, at(&img, 0, 0) as f64);
    assert!((c / 20000.0 - 1.0).abs() < 0.001, "{c}");
    // The corner pixel's centre is at r = sqrt(31.5^2 + 23.5^2) / 40.
    let r2 = (31.5f64 * 31.5 + 23.5 * 23.5) / 1600.0;
    assert!((k - 20000.0 * (1.0 + 0.5 * r2)).abs() < 2.0, "{k}");
    let stored = raw
        .decode_raw_as_stored(&f, &DecodeOptions::default())
        .unwrap();
    assert!(stored.data.iter().all(|&v| v == 20000) && stored.opcodes.is_none());

    // A 2x2 gain map 1, 2 / 3, 4 spanning the image, bilinear between the corner pixels.
    let f = flat(
        41,
        21,
        10000,
        vec![opcode_list(
            2,
            vec![op_gain_map([0, 0, 21, 41], 1, 2, 2, &[1.0, 2.0, 3.0, 4.0])],
        )],
    );
    let raw = specular_raw::parse(&f).unwrap();
    let img = raw.decode_raw(&f, &DecodeOptions::default()).unwrap();
    let want = |x: u32, y: u32| {
        let (u, v) = ((x as f64 + 0.5) / 41.0, (y as f64 + 0.5) / 21.0);
        let g = 1.0 + u + 2.0 * v;
        (10000.0 * g).min(65535.0)
    };
    for (x, y) in [(0, 0), (40, 0), (0, 20), (40, 20), (20, 10), (7, 13)] {
        assert!(
            (at(&img, x, y) as f64 - want(x, y)).abs() < 2.0,
            "({x}, {y}): {} vs {}",
            at(&img, x, y),
            want(x, y)
        );
    }
}

#[test]
fn bad_pixels_take_their_neighbours() {
    let mut data = vec![10000u16; 32 * 32];
    data[5 * 32 + 6] = 0;
    data[20 * 32 + 21] = 60000;
    for y in 10..12 {
        for x in 10..13 {
            data[y * 32 + x] = 50000;
        }
    }
    let mut s = DngSpec::new(32, 32, DngCodec::U16);
    s.extra = vec![opcode_list(
        1,
        vec![
            (4, 0, OpParams::default().u32(&[0, 0])),
            (
                5,
                0,
                OpParams::default().u32(&[0, 1, 1, 20, 21, 10, 10, 12, 13]),
            ),
        ],
    )];
    let f = write_dng(&s, &data);
    let raw = specular_raw::parse(&f).unwrap();
    let img = raw.decode_raw(&f, &DecodeOptions::default()).unwrap();
    assert!(img.data.iter().all(|&v| v == 10000), "bad pixels left");
}

#[test]
fn unknown_and_misplaced_opcodes_are_reported() {
    let f = flat(
        16,
        16,
        1000,
        vec![
            opcode_list(
                2,
                vec![
                    (99, 1, OpParams::default().u32(&[1])),
                    (98, 0, OpParams::default().u32(&[1])),
                    op_warp(&[[0.9, 0.0, 0.0, 0.0, 0.0, 0.0]], [0.5, 0.5]),
                    // Truncated parameters.
                    (3, 0, OpParams::default().f64(&[0.1, 0.2])),
                ],
            ),
            opcode_list(3, vec![(14, 0, OpParams::default().u32(&[0]))]),
        ],
    );
    let raw = specular_raw::parse(&f).unwrap();
    let si = raw.info().sensor.clone().unwrap();
    assert_eq!(si.opcodes, Vec::<&str>::new());
    assert_eq!(
        si.opcodes_skipped,
        [
            "unknown opcode",
            "WarpRectilinear",
            "FixVignetteRadial",
            "WarpRectilinear2"
        ]
    );
    // Still decodes (list 2 normalises the samples).
    let img = raw.decode(&f, &DecodeOptions::default()).unwrap();
    assert_eq!((img.width, img.height), (16, 16));
}

/// LinearRaw with value `100 * x` at pixel centre `x` (all channels): a warp's output is the
/// ramp at the source position the DNG formula gives, at full and half size.
#[test]
fn warp_samples_where_the_formula_says() {
    let (w, h) = (200u32, 120u32);
    let mut data = Vec::new();
    for _ in 0..h {
        for x in 0..w {
            data.extend([(100 * x + 50) as u16; 3]);
        }
    }
    // kr0 0.8 (zoom in), a radial term and tangential terms; centre off the middle.
    let k = [0.8, 0.05, 0.0, 0.0, 0.002, -0.001];
    let c = [0.45, 0.6];
    let mut s = DngSpec::new(w, h, DngCodec::U16);
    s.linear_raw = true;
    s.extra = vec![opcode_list(3, vec![op_warp(&[k], c)])];
    let f = write_dng(&s, &data);
    let raw = specular_raw::parse(&f).unwrap();
    let (cx, cy) = (c[0] * w as f64, c[1] * h as f64);
    let m = [
        (0.0, 0.0),
        (w as f64, 0.0),
        (0.0, h as f64),
        (w as f64, h as f64),
    ]
    .iter()
    .map(|&(x, y): &(f64, f64)| ((x - cx).powi(2) + (y - cy).powi(2)).sqrt())
    .fold(0.0, f64::max);
    let src_x = |x: f64, y: f64| {
        let (dx, dy) = ((x - cx) / m, (y - cy) / m);
        let r2 = dx * dx + dy * dy;
        let f = k[0] + k[1] * r2;
        cx + m * (f * dx + k[4] * 2.0 * dx * dy + k[5] * (r2 + 2.0 * dx * dx))
    };
    for size in [OutputSize::Full, OutputSize::Half] {
        let img = raw
            .decode(
                &f,
                &DecodeOptions {
                    size,
                    ..Default::default()
                },
            )
            .unwrap();
        let (step, off) = match size {
            OutputSize::Full => (1.0, 0.5),
            OutputSize::Half => (2.0, 1.0),
        };
        let mut worst = 0f64;
        for j in 0..img.height {
            for i in 0..img.width {
                let (x, y) = (i as f64 * step + off, j as f64 * step + off);
                let sx = src_x(x, y);
                // Interior only (edges clamp), in output pixel units of the source.
                let so = (sx - off) / step;
                if so < 2.0 || so > img.width as f64 - 3.0 {
                    continue;
                }
                // The half-size image averages two columns: same ramp value at its centre.
                let want = 100.0 * sx;
                let got = img.data[((j * img.width + i) * 3 + 1) as usize] as f64;
                worst = worst.max((got - want).abs());
            }
        }
        assert!(worst < 3.0, "{size:?}: worst {worst}");
    }
}

#[test]
fn trim_bounds_crop_and_as_stored() {
    let f = flat(
        40,
        30,
        5000,
        vec![opcode_list(
            3,
            vec![(6, 0, OpParams::default().u32(&[2, 4, 28, 30]))],
        )],
    );
    let raw = specular_raw::parse(&f).unwrap();
    let si = raw.info().sensor.clone().unwrap();
    assert_eq!(
        si.crop,
        Rect {
            x: 4,
            y: 2,
            w: 26,
            h: 26
        }
    );
    let img = raw.decode(&f, &DecodeOptions::default()).unwrap();
    assert_eq!((img.width, img.height), (26, 26));
    let s = raw
        .decode_raw_as_stored(&f, &DecodeOptions::default())
        .unwrap();
    assert_eq!(
        s.crop,
        Rect {
            x: 0,
            y: 0,
            w: 40,
            h: 30
        }
    );
}

fn large_corpus(rel: &str) -> Option<Vec<u8>> {
    let p = std::env::var_os("SPECULAR_LARGE_CORPUS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/large")
        })
        .join(rel);
    let d = std::fs::read(&p).ok();
    if d.is_none() {
        eprintln!("{} not present; skipping", p.display());
    }
    d
}

/// Mean green of a patch of `img` around relative position `(fx, fy)`.
fn patch(img: &RgbImage, fx: f64, fy: f64) -> f64 {
    let r = (img.width.min(img.height) / 40) as i64;
    let (cx, cy) = (
        (img.width as f64 * fx) as i64,
        (img.height as f64 * fy) as i64,
    );
    let (mut a, mut n) = (0f64, 0f64);
    for y in (cy - r).max(0)..(cy + r).min(img.height as i64) {
        for x in (cx - r).max(0)..(cx + r).min(img.width as i64) {
            a += img.data[((y * img.width as i64 + x) * 3 + 1) as usize] as f64;
            n += 1.0;
        }
    }
    a / n
}

/// Real DNGs with FixVignetteRadial (list 2) and WarpRectilinear (list 3): list 2 brightens
/// the corners relative to the centre by more than 15 %.
#[test]
fn corpus_vignette_brightens_the_corners() {
    for rel in [
        "raw/Adobe DNG Converter/Nikon Z 6/DSC_0757.dng",
        "raw/Adobe DNG Converter/Fujifilm X-T4/DSCF0268.dng",
    ] {
        let Some(d) = large_corpus(rel) else {
            continue;
        };
        let raw = specular_raw::parse(&d).unwrap();
        let si = raw.info().sensor.clone().unwrap();
        assert!(
            si.opcodes.contains(&"FixVignetteRadial") && si.opcodes.contains(&"WarpRectilinear"),
            "{rel}: {:?}",
            si.opcodes
        );
        assert!(si.opcodes_skipped.is_empty(), "{rel}");
        let o = DecodeOptions {
            size: OutputSize::Half,
            ..Default::default()
        };
        let a = specular_raw::develop(&raw.decode_raw_as_stored(&d, &o).unwrap(), &o).unwrap();
        // List 2 only (the warp moves scene content between the patches).
        let mut r2 = raw.decode_raw(&d, &o).unwrap();
        r2.opcodes = None;
        let b = specular_raw::develop(&r2, &o).unwrap();
        assert_eq!((a.width, a.height), (b.width, b.height));
        for (fx, fy) in [(0.04, 0.04), (0.96, 0.04), (0.04, 0.96), (0.96, 0.96)] {
            let ga = patch(&a, fx, fy) / patch(&a, 0.5, 0.5);
            let gb = patch(&b, fx, fy) / patch(&b, 0.5, 0.5);
            assert!(gb / ga > 1.15, "{rel} ({fx}, {fy}): {ga:.3} -> {gb:.3}");
        }
    }
}
