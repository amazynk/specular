//! Correctness against libjpeg-turbo's djpeg (islow IDCT).

mod common;

use common::*;
use specular_jpeg::*;

const SIZES: &[(u32, u32)] = &[
    (333, 251),
    (1021, 769),
    (17, 9),
    (1, 1),
    (8, 8),
    (16, 16),
    (31, 47),
    (250, 3),
];

struct Variant {
    name: &'static str,
    args: &'static [&'static str],
    /// 4:4:4 or grayscale (no chroma upsampling).
    unsubsampled: bool,
}

const VARIANTS: &[Variant] = &[
    Variant {
        name: "444",
        args: &["-quality", "92", "-sample", "1x1"],
        unsubsampled: true,
    },
    Variant {
        name: "420",
        args: &["-quality", "92", "-sample", "2x2"],
        unsubsampled: false,
    },
    Variant {
        name: "422",
        args: &["-quality", "85", "-sample", "2x1"],
        unsubsampled: false,
    },
    Variant {
        name: "440",
        args: &["-quality", "85", "-sample", "1x2"],
        unsubsampled: false,
    },
    Variant {
        name: "411",
        args: &["-quality", "85", "-sample", "4x1"],
        unsubsampled: false,
    },
    Variant {
        name: "gray",
        args: &["-quality", "90", "-grayscale"],
        unsubsampled: true,
    },
    Variant {
        name: "rgb",
        args: &["-quality", "90", "-rgb"],
        unsubsampled: true,
    },
    Variant {
        name: "420_q100",
        args: &["-quality", "100", "-sample", "2x2"],
        unsubsampled: false,
    },
    Variant {
        name: "444_q30",
        args: &["-quality", "30", "-sample", "1x1"],
        unsubsampled: true,
    },
    Variant {
        name: "420_rst1",
        args: &["-quality", "92", "-sample", "2x2", "-restart", "1"],
        unsubsampled: false,
    },
    Variant {
        name: "444_rst7b",
        args: &["-quality", "92", "-sample", "1x1", "-restart", "7B"],
        unsubsampled: true,
    },
    Variant {
        name: "420_opt",
        args: &["-quality", "92", "-sample", "2x2", "-optimize"],
        unsubsampled: false,
    },
    Variant {
        name: "420_prog",
        args: &["-quality", "92", "-sample", "2x2", "-progressive"],
        unsubsampled: false,
    },
    Variant {
        name: "444_prog",
        args: &["-quality", "92", "-sample", "1x1", "-progressive"],
        unsubsampled: true,
    },
    Variant {
        name: "gray_prog",
        args: &["-quality", "90", "-grayscale", "-progressive"],
        unsubsampled: true,
    },
    Variant {
        name: "420_prog_rst",
        args: &[
            "-quality",
            "92",
            "-sample",
            "2x2",
            "-progressive",
            "-restart",
            "2",
        ],
        unsubsampled: false,
    },
];

fn all_images() -> Vec<(String, Vec<u8>, &'static Variant)> {
    let mut v = Vec::new();
    for (i, &(w, h)) in SIZES.iter().enumerate() {
        let src = source_ppm(w, h, 7 + i as u32);
        for var in VARIANTS {
            let name = format!("{}_{w}x{h}", var.name);
            v.push((name.clone(), cjpeg(&src, &name, var.args), var));
        }
    }
    v
}

fn opts(scale: Scale, par: Parallelism) -> DecodeOptions<'static> {
    DecodeOptions {
        scale,
        region: None,
        cancel: None,
        parallelism: par,
    }
}

#[test]
fn full_scale_matches_djpeg() {
    if !tools_available() {
        eprintln!("skipping: cjpeg/djpeg/magick not found");
        return;
    }
    for (name, jpg, var) in all_images() {
        for par in [Parallelism::Single, Parallelism::Threads(4)] {
            let d = decode(&jpg, &opts(Scale::Full, par)).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!d.truncated, "{name}");
            // Fancy (bilinear) upsampling vs djpeg default.
            let ours = planes_to_rgba(&d);
            let refi = djpeg(&jpg, &["-dct", "int", "-ppm"]);
            let (max, mean) = diff_rgba_pnm(&ours, d.width as usize, d.height as usize, &refi);
            // libjpeg-turbo only "fancy"-upsamples 2x factors; it replicates for 4:1:1, where our
            // centered bilinear filter legitimately differs. Compare those with Nearest only.
            let limit = if var.name == "411" { 255 } else { 2 };
            assert!(
                max <= limit,
                "{name} {par:?}: fancy max diff {max} (mean {mean:.4})"
            );
            // Nearest upsampling vs djpeg -nosmooth.
            let mut near = vec![0u8; ours.len()];
            planes_to_rgba_into(&d, Upsampling::Nearest, &mut near, d.width as usize * 4);
            let refn = djpeg(&jpg, &["-dct", "int", "-nosmooth", "-ppm"]);
            let (max, mean) = diff_rgba_pnm(&near, d.width as usize, d.height as usize, &refn);
            assert!(
                max <= 2,
                "{name} {par:?}: nearest max diff {max} (mean {mean:.4})"
            );
        }
    }
}

#[test]
fn scaled_luma_matches_djpeg() {
    if !tools_available() {
        return;
    }
    for (name, jpg, var) in all_images() {
        for (scale, frac) in [
            (Scale::Half, "1/2"),
            (Scale::Quarter, "1/4"),
            (Scale::Eighth, "1/8"),
        ] {
            for par in [Parallelism::Single, Parallelism::Threads(4)] {
                let d = decode(&jpg, &opts(scale, par)).unwrap_or_else(|e| panic!("{name}: {e}"));
                let refy = djpeg(&jpg, &["-dct", "int", "-scale", frac, "-grayscale", "-pnm"]);
                let y = &d.planes[0];
                if var.name.starts_with("rgb") {
                    // djpeg -grayscale on RGB computes luma from RGB; compare full colour instead.
                    let refc = djpeg(&jpg, &["-dct", "int", "-scale", frac, "-ppm"]);
                    let ours = planes_to_rgba(&d);
                    let (max, _) = diff_rgba_pnm(&ours, d.width as usize, d.height as usize, &refc);
                    assert!(max <= 3, "{name} {frac}: rgb max {max}");
                    continue;
                }
                let (max, mean) = diff_plane_pgm(
                    &y.data,
                    y.stride,
                    y.width as usize,
                    y.height as usize,
                    &refy,
                );
                assert!(
                    max <= 3,
                    "{name} {frac} {par:?}: Y max diff {max} mean {mean:.4}"
                );
                // Full colour: 4:4:4 at every scale; subsampled at 1/2 and 1/4, where we use
                // libjpeg's per-component IDCT scaling (4:1:1 excepted: libjpeg replicates there,
                // and at 1/8 we stay DC-only while libjpeg uses a 2x2 IDCT for chroma).
                // libjpeg also replicates instead of interpolating when a plane is <= 2 samples wide.
                let narrow = d
                    .planes
                    .iter()
                    .any(|p| p.width <= 2 && p.h_samp < d.planes[0].h_samp);
                let colour_ok = if var.unsubsampled {
                    true
                } else {
                    scale != Scale::Eighth && var.name != "411" && !narrow
                };
                if colour_ok && !var.name.starts_with("gray") {
                    let refc = djpeg(&jpg, &["-dct", "int", "-scale", frac, "-ppm"]);
                    let ours = planes_to_rgba(&d);
                    let (max, _) = diff_rgba_pnm(&ours, d.width as usize, d.height as usize, &refc);
                    assert!(max <= 3, "{name} {frac}: rgb max {max}");
                }
            }
        }
    }
}

#[test]
fn progressive_equals_baseline() {
    if !tools_available() {
        return;
    }
    for (i, &(w, h)) in SIZES.iter().enumerate() {
        let src = source_ppm(w, h, 7 + i as u32);
        for (b, p) in [
            ("420", "420_prog"),
            ("444", "444_prog"),
            ("gray", "gray_prog"),
        ] {
            let vb = VARIANTS.iter().find(|v| v.name == b).unwrap();
            let vp = VARIANTS.iter().find(|v| v.name == p).unwrap();
            let jb = cjpeg(&src, &format!("{b}_{w}x{h}"), vb.args);
            let jp = cjpeg(&src, &format!("{p}_{w}x{h}"), vp.args);
            for scale in [Scale::Full, Scale::Half, Scale::Quarter, Scale::Eighth] {
                let db = decode(&jb, &opts(scale, Parallelism::Single)).unwrap();
                let dp = decode(&jp, &opts(scale, Parallelism::Threads(4))).unwrap();
                assert_eq!(db.planes.len(), dp.planes.len());
                for (a, c) in db.planes.iter().zip(&dp.planes) {
                    assert_eq!((a.width, a.height, a.stride), (c.width, c.height, c.stride));
                    let max = a
                        .data
                        .iter()
                        .zip(&c.data)
                        .map(|(x, y)| x.abs_diff(*y))
                        .max()
                        .unwrap_or(0);
                    assert!(max <= 1, "{p} {w}x{h} {scale:?}: max {max}");
                }
            }
        }
    }
}

fn crop_equal(full: &DecodedPlanes, reg: &DecodedPlanes, what: &str) {
    assert_eq!(full.planes.len(), reg.planes.len());
    let mh = full.planes.iter().map(|p| p.h_samp).max().unwrap() as u32;
    let mv = full.planes.iter().map(|p| p.v_samp).max().unwrap() as u32;
    for (fp, rp) in full.planes.iter().zip(&reg.planes) {
        let (fx, fy) = (mh / fp.h_samp as u32, mv / fp.v_samp as u32);
        let (ox, oy) = (reg.origin.0 / fx, reg.origin.1 / fy);
        for y in 0..rp.height {
            for x in 0..rp.width {
                let a = fp.data[((oy + y) as usize) * fp.stride + (ox + x) as usize];
                let b = rp.data[y as usize * rp.stride + x as usize];
                assert_eq!(a, b, "{what}: plane mismatch at {x},{y}");
            }
        }
        assert_eq!(
            rp.width.min(fp.width - ox),
            rp.width,
            "{what}: region plane too wide"
        );
    }
}

#[test]
fn region_equals_crop() {
    if !tools_available() {
        return;
    }
    for (name, jpg, _) in all_images() {
        let info = read_info(&jpg).unwrap();
        let ix = build_index(&jpg).unwrap();
        for scale in [Scale::Full, Scale::Half, Scale::Quarter, Scale::Eighth] {
            let full = decode(&jpg, &opts(scale, Parallelism::Single)).unwrap();
            let (ow, oh) = (
                info.width.div_ceil(scale as u32),
                info.height.div_ceil(scale as u32),
            );
            let regions = [
                Region {
                    x: 0,
                    y: 0,
                    w: ow,
                    h: oh,
                },
                Region {
                    x: ow / 3,
                    y: oh / 4,
                    w: (ow / 3).max(1),
                    h: (oh / 2).max(1),
                },
                Region {
                    x: ow - 1,
                    y: oh - 1,
                    w: 5,
                    h: 5,
                },
                Region {
                    x: ow / 2,
                    y: 0,
                    w: 1,
                    h: oh,
                },
            ];
            for r in regions {
                for par in [Parallelism::Single, Parallelism::Threads(3)] {
                    let o = DecodeOptions {
                        scale,
                        region: Some(r),
                        cancel: None,
                        parallelism: par,
                    };
                    let d = decode(&jpg, &o).unwrap_or_else(|e| panic!("{name} {r:?}: {e}"));
                    assert!(d.origin.0 <= r.x && d.origin.1 <= r.y);
                    assert!(
                        d.origin.0 + d.width >= (r.x + r.w).min(ow)
                            && d.origin.1 + d.height >= (r.y + r.h).min(oh)
                    );
                    crop_equal(&full, &d, &format!("{name} {scale:?} {r:?}"));
                    let d2 = decode_with_index(&jpg, &ix, &o).unwrap();
                    assert!(d == d2, "{name} {scale:?} {r:?}: indexed region differs");
                }
            }
            let d3 = decode_with_index(&jpg, &ix, &opts(scale, Parallelism::Threads(4))).unwrap();
            assert!(full == d3, "{name} {scale:?}: indexed full decode differs");
            let (d4, ix2) = decode_and_index(&jpg, &opts(scale, Parallelism::Threads(4))).unwrap();
            assert!(full == d4, "{name} {scale:?}: decode_and_index differs");
            assert_eq!(ix2.len(), ix.len());
            // The progressive index-only build (nonzero masks, no coefficients) records
            // exactly the checkpoints of a full decode. (Sequential checkpoints may differ
            // in bit-buffer fill while being equivalent, so they are not compared.)
            let progressive = matches!(info.coding, Coding::Progressive);
            for par in [Parallelism::Single, Parallelism::Threads(4)] {
                let o = opts(scale, par);
                let (_, a) = decode_and_index(&jpg, &o).unwrap();
                let b = build_index_with(&jpg, &o).unwrap();
                assert!(
                    !progressive || a == b,
                    "{name} {scale:?} {par:?}: build_index differs\n{a:?}\n{b:?}"
                );
            }
        }
    }
}

/// Region decodes with an index start rows at column checkpoints recorded by earlier region
/// decodes with the same index: many regions in pseudo-random order (cold and warm rows,
/// before and after the region's columns) must all equal the crop of a full decode.
#[test]
fn region_column_checkpoints_equal_crop() {
    if !tools_available() {
        return;
    }
    let src = source_ppm(1021, 769, 8);
    for var in VARIANTS {
        let name = format!("{}_cols", var.name);
        let jpg = cjpeg(&src, &name, var.args);
        let info = read_info(&jpg).unwrap();
        for scale in [Scale::Full, Scale::Quarter] {
            let ix = build_index(&jpg).unwrap();
            let rows_only = ix.memory_bytes();
            let full = decode(&jpg, &opts(scale, Parallelism::Single)).unwrap();
            let (ow, oh) = (
                info.width.div_ceil(scale as u32),
                info.height.div_ceil(scale as u32),
            );
            let mut seed = 0x2545_f491_u32;
            let mut rnd = |n: u32| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                seed % n.max(1)
            };
            for i in 0..24 {
                let w = 1 + rnd(ow / 2);
                let h = 1 + rnd(oh / 3);
                let r = Region {
                    x: rnd(ow - w + 1),
                    y: rnd(oh - h + 1),
                    w,
                    h,
                };
                let par = if i % 2 == 0 {
                    Parallelism::Single
                } else {
                    Parallelism::Threads(3)
                };
                let o = DecodeOptions {
                    scale,
                    region: Some(r),
                    cancel: None,
                    parallelism: par,
                };
                let d = decode_with_index(&jpg, &ix, &o).unwrap();
                crop_equal(&full, &d, &format!("{name} {scale:?} {r:?} #{i}"));
            }
            assert!(
                ix.memory_bytes() > rows_only,
                "{name}: no column checkpoints recorded"
            );
        }
    }
}

#[test]
fn choose_scale_semantics() {
    let mut info = read_info(&minimal()).unwrap();
    info.width = 8000;
    info.height = 6000;
    assert_eq!(choose_scale(&info, 1000, 1000), Scale::Eighth);
    assert_eq!(choose_scale(&info, 1920, 1080), Scale::Quarter);
    assert_eq!(choose_scale(&info, 3840, 2160), Scale::Half);
    assert_eq!(choose_scale(&info, 7000, 7000), Scale::Full);
    assert_eq!(choose_scale(&info, 0, 10), Scale::Eighth);
}

fn minimal() -> Vec<u8> {
    // 8x8 gray baseline made by cjpeg, embedded so this test needs no tools.
    include_bytes!("fixtures/gray8.jpg").to_vec()
}
