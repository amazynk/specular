//! Arithmetic-coded JPEGs (SOF9 / SOF10) against djpeg, and against the Huffman-coded file
//! with the same coefficients (`cjpeg` with and without `-arithmetic` quantises identically,
//! so every decode path must give bit-identical planes). Generated with cjpeg at test time;
//! the small checked-in fixtures (`tests/fixtures/arith_*.jpg`, see `tests/fixtures/make-arith.sh`)
//! are checked without tools.

mod common;

use common::*;
use specular_jpeg::*;

const SIZES: &[(u32, u32)] = &[(333, 251), (1021, 769), (17, 9), (1, 1), (16, 16), (250, 3)];

struct Variant {
    name: &'static str,
    args: &'static [&'static str],
    unsubsampled: bool,
}

const VARIANTS: &[Variant] = &[
    Variant {
        name: "420",
        args: &["-quality", "92", "-sample", "2x2"],
        unsubsampled: false,
    },
    Variant {
        name: "444",
        args: &["-quality", "92", "-sample", "1x1"],
        unsubsampled: true,
    },
    Variant {
        name: "422_q40",
        args: &["-quality", "40", "-sample", "2x1"],
        unsubsampled: false,
    },
    Variant {
        name: "gray",
        args: &["-quality", "90", "-grayscale"],
        unsubsampled: true,
    },
    Variant {
        name: "420_q100",
        args: &["-quality", "100", "-sample", "2x2"],
        unsubsampled: false,
    },
    Variant {
        name: "420_rst1",
        args: &["-quality", "92", "-sample", "2x2", "-restart", "1"],
        unsubsampled: false,
    },
    Variant {
        name: "444_rst5b",
        args: &["-quality", "85", "-sample", "1x1", "-restart", "5B"],
        unsubsampled: true,
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
            "3B",
        ],
        unsubsampled: false,
    },
];

/// (name, arithmetic file, Huffman file with the same coefficients, variant).
fn all_images() -> Vec<(String, Vec<u8>, Vec<u8>, &'static Variant)> {
    let mut v = Vec::new();
    for (i, &(w, h)) in SIZES.iter().enumerate() {
        let src = source_ppm(w, h, 31 + i as u32);
        for var in VARIANTS {
            let name = format!("arith_{}_{w}x{h}", var.name);
            let mut args = var.args.to_vec();
            let huff = cjpeg(&src, &format!("huff_{}_{w}x{h}", var.name), &args);
            args.push("-arithmetic");
            let a = cjpeg(&src, &name, &args);
            v.push((name, a, huff, var));
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

const SCALES: [Scale; 4] = [Scale::Full, Scale::Half, Scale::Quarter, Scale::Eighth];

#[test]
fn header() {
    if !tools_available() {
        eprintln!("skipping: cjpeg/djpeg/magick not found");
        return;
    }
    for (name, a, _, var) in all_images() {
        let i = read_info(&a).unwrap();
        assert_eq!(i.coding, Coding::Arithmetic, "{name}");
        let prog = var.name.contains("prog");
        assert_eq!(i.sof_marker, if prog { 0xCA } else { 0xC9 }, "{name}");
    }
}

#[test]
fn full_scale_matches_djpeg() {
    if !tools_available() {
        eprintln!("skipping: cjpeg/djpeg/magick not found");
        return;
    }
    for (name, jpg, _, _) in all_images() {
        for par in [Parallelism::Single, Parallelism::Threads(4)] {
            let d = decode(&jpg, &opts(Scale::Full, par)).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!d.truncated, "{name}");
            let ours = planes_to_rgba(&d);
            let refi = djpeg(&jpg, &["-dct", "int", "-ppm"]);
            let (max, mean) = diff_rgba_pnm(&ours, d.width as usize, d.height as usize, &refi);
            assert!(max <= 2, "{name} {par:?}: fancy max {max} (mean {mean:.4})");
            let mut near = vec![0u8; ours.len()];
            planes_to_rgba_into(&d, Upsampling::Nearest, &mut near, d.width as usize * 4);
            let refn = djpeg(&jpg, &["-dct", "int", "-nosmooth", "-ppm"]);
            let (max, mean) = diff_rgba_pnm(&near, d.width as usize, d.height as usize, &refn);
            assert!(
                max <= 2,
                "{name} {par:?}: nearest max {max} (mean {mean:.4})"
            );
        }
    }
}

#[test]
fn scaled_matches_djpeg() {
    if !tools_available() {
        return;
    }
    for (name, jpg, _, var) in all_images() {
        for (scale, frac) in [
            (Scale::Half, "1/2"),
            (Scale::Quarter, "1/4"),
            (Scale::Eighth, "1/8"),
        ] {
            let d = decode(&jpg, &opts(scale, Parallelism::Threads(3)))
                .unwrap_or_else(|e| panic!("{name} {frac}: {e}"));
            let refy = djpeg(&jpg, &["-dct", "int", "-scale", frac, "-grayscale", "-pnm"]);
            let y = &d.planes[0];
            let (max, mean) = diff_plane_pgm(
                &y.data,
                y.stride,
                y.width as usize,
                y.height as usize,
                &refy,
            );
            assert!(max <= 3, "{name} {frac}: Y max {max} mean {mean:.4}");
            let narrow = d
                .planes
                .iter()
                .any(|p| p.width <= 2 && p.h_samp < d.planes[0].h_samp);
            let colour_ok = var.unsubsampled || (scale != Scale::Eighth && !narrow);
            if colour_ok && !var.name.starts_with("gray") {
                let refc = djpeg(&jpg, &["-dct", "int", "-scale", frac, "-ppm"]);
                let ours = planes_to_rgba(&d);
                let (max, _) = diff_rgba_pnm(&ours, d.width as usize, d.height as usize, &refc);
                assert!(max <= 3, "{name} {frac}: rgb max {max}");
            }
        }
    }
}

/// Same coefficients, other entropy coding: identical planes at every scale.
#[test]
fn equals_huffman_coded() {
    if !tools_available() {
        return;
    }
    for (name, a, h, _) in all_images() {
        for scale in SCALES {
            let da = decode(&a, &opts(scale, Parallelism::Threads(4))).unwrap();
            let dh = decode(&h, &opts(scale, Parallelism::Single)).unwrap();
            assert!(
                da == dh,
                "{name} {scale:?}: differs from the Huffman-coded file"
            );
        }
    }
}

/// Restart intervals decode in parallel bands: every band count gives the serial planes, at
/// every scale and for regions.
#[test]
fn restart_bands_equal_serial() {
    if !tools_available() {
        return;
    }
    for (name, a, _, var) in all_images() {
        if !var.name.contains("rst") {
            continue;
        }
        let info = read_info(&a).unwrap();
        for scale in SCALES {
            let one = decode(&a, &opts(scale, Parallelism::Single)).unwrap();
            for n in [2, 3, 5, 8] {
                let d = decode(&a, &opts(scale, Parallelism::Threads(n))).unwrap();
                assert!(
                    d == one,
                    "{name} {scale:?} threads {n}: differs from serial"
                );
            }
            let (ow, oh) = (
                info.width.div_ceil(scale as u32),
                info.height.div_ceil(scale as u32),
            );
            let r = Region {
                x: ow / 5,
                y: oh / 3,
                w: (ow / 2).max(1),
                h: (oh / 2).max(1),
            };
            let o = DecodeOptions {
                scale,
                region: Some(r),
                cancel: None,
                parallelism: Parallelism::Threads(6),
            };
            let d = decode(&a, &o).unwrap();
            crop_equal(&one, &d, &format!("{name} {scale:?} {r:?}"));
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
    }
}

/// Region decodes (plain, with an index from `build_index` and from `decode_and_index`)
/// equal the crop of the full decode; the indexed full decode equals the plain one.
#[test]
fn region_and_index() {
    if !tools_available() {
        return;
    }
    for (name, jpg, _, _) in all_images() {
        let info = read_info(&jpg).unwrap();
        let ix = build_index(&jpg).unwrap();
        assert!(!ix.is_empty() && ix.memory_bytes() > 0, "{name}");
        for scale in SCALES {
            let full = decode(&jpg, &opts(scale, Parallelism::Single)).unwrap();
            let (d4, ix2) = decode_and_index(&jpg, &opts(scale, Parallelism::Threads(4))).unwrap();
            assert!(full == d4, "{name} {scale:?}: decode_and_index differs");
            assert!(ix == ix2, "{name} {scale:?}: indexes differ");
            let d3 = decode_with_index(&jpg, &ix, &opts(scale, Parallelism::Threads(4))).unwrap();
            assert!(full == d3, "{name} {scale:?}: indexed full decode differs");
            let (ow, oh) = (
                info.width.div_ceil(scale as u32),
                info.height.div_ceil(scale as u32),
            );
            let regions = [
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
                    x: 0,
                    y: oh / 2,
                    w: ow,
                    h: 1,
                },
            ];
            for r in regions {
                let o = DecodeOptions {
                    scale,
                    region: Some(r),
                    cancel: None,
                    parallelism: Parallelism::Threads(3),
                };
                let d = decode(&jpg, &o).unwrap_or_else(|e| panic!("{name} {r:?}: {e}"));
                assert!(d.origin.0 <= r.x && d.origin.1 <= r.y);
                crop_equal(&full, &d, &format!("{name} {scale:?} {r:?}"));
                let d2 = decode_with_index(&jpg, &ix, &o).unwrap();
                assert!(d == d2, "{name} {scale:?} {r:?}: indexed region differs");
            }
        }
    }
}

/// Cut-short and corrupted files decode without panicking; a cut file is flagged.
#[test]
fn truncated_and_corrupt() {
    if !tools_available() {
        return;
    }
    for (name, jpg, _, _) in all_images() {
        let info = read_info(&jpg).unwrap();
        if info.width * info.height < 10_000 {
            continue;
        }
        for cut in [info.header_len + 40, jpg.len() / 2, jpg.len() - 3] {
            for par in [Parallelism::Single, Parallelism::Threads(4)] {
                let d = decode(&jpg[..cut], &opts(Scale::Full, par))
                    .unwrap_or_else(|e| panic!("{name} cut {cut}: {e}"));
                assert!(d.truncated, "{name} cut at {cut} {par:?} not flagged");
            }
            let _ = build_index(&jpg[..cut]);
        }
        let mut bad = jpg.clone();
        let mut x = 0x9e37_79b9u32;
        for _ in 0..40 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let at = info.header_len + 20 + (x as usize % (jpg.len() - info.header_len - 22));
            bad[at] = (x >> 8) as u8;
        }
        for scale in SCALES {
            for n in [2, 5] {
                let _ = decode(&bad, &opts(scale, Parallelism::Threads(n)));
            }
        }
        if let Ok(ix) = build_index(&bad) {
            let _ = decode_with_index(&bad, &ix, &opts(Scale::Half, Parallelism::Single));
        }
    }
}

/// Memory estimates cover the actual coefficient buffers (within the usual slack).
#[test]
fn memory_estimate_sane() {
    if !tools_available() {
        return;
    }
    for (name, jpg, _, _) in all_images() {
        let info = read_info(&jpg).unwrap();
        for scale in SCALES {
            let e = memory_estimate(&info, scale, None);
            let d = decode(&jpg, &opts(scale, Parallelism::Single)).unwrap();
            let out: usize = d.planes.iter().map(|p| p.data.len()).sum();
            assert!(e.output_bytes as usize >= out, "{name} {scale:?}: {e:?}");
            assert!(e.index_bytes > 0, "{name} {scale:?}: {e:?}");
        }
    }
}

/// The checked-in fixtures (no tools needed): decode at every scale, region and index.
#[test]
fn fixtures() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut n = 0;
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        let fname = p.file_name().unwrap().to_string_lossy().to_string();
        if !fname.starts_with("arith_") || !fname.ends_with(".jpg") {
            continue;
        }
        n += 1;
        let jpg = std::fs::read(&p).unwrap();
        let info = read_info(&jpg).unwrap();
        assert_eq!(info.coding, Coding::Arithmetic, "{fname}");
        let ix = build_index(&jpg).unwrap();
        for scale in SCALES {
            let d = decode(&jpg, &opts(scale, Parallelism::Single)).unwrap();
            assert!(!d.truncated, "{fname}");
            assert_eq!(d.width, info.width.div_ceil(scale as u32), "{fname}");
            let d2 = decode_with_index(&jpg, &ix, &opts(scale, Parallelism::Threads(2))).unwrap();
            assert!(d == d2, "{fname} {scale:?}");
            let d4 = decode(&jpg, &opts(scale, Parallelism::Threads(4))).unwrap();
            assert!(d == d4, "{fname} {scale:?}: 4 threads differ from serial");
            for r in [
                Region {
                    x: d.width - 1,
                    y: d.height - 1,
                    w: 1,
                    h: 1,
                },
                Region {
                    x: d.width / 3,
                    y: d.height / 3,
                    w: d.width / 3,
                    h: d.height / 4,
                },
            ] {
                let o = DecodeOptions {
                    region: Some(r),
                    ..opts(scale, Parallelism::Single)
                };
                let what = format!("{fname} {scale:?} {r:?}");
                crop_equal(&d, &decode(&jpg, &o).unwrap(), &what);
                crop_equal(&d, &decode_with_index(&jpg, &ix, &o).unwrap(), &what);
            }
            // Not flat grey: the fixtures are photo-like.
            let y = &d.planes[0].data;
            let (lo, hi) = (y.iter().min().unwrap(), y.iter().max().unwrap());
            assert!(hi - lo > 40, "{fname} {scale:?}: flat ({lo}..{hi})");
        }
        if tools_available() {
            let d = decode(&jpg, &opts(Scale::Full, Parallelism::Single)).unwrap();
            let refi = djpeg_file(&p, &["-dct", "int", "-ppm"]);
            let (max, _) = diff_rgba_pnm(
                &planes_to_rgba(&d),
                d.width as usize,
                d.height as usize,
                &refi,
            );
            assert!(max <= 2, "{fname}: max {max}");
        }
    }
    assert!(n >= 4, "arith fixtures missing ({n})");
}

/// Relabels an 8-bit arithmetic-coded frame (SOF9 / SOF10) as 12-bit.
fn as_12bit(jpg: &[u8]) -> Vec<u8> {
    let mut v = jpg.to_vec();
    let sof = v
        .windows(2)
        .position(|w| w[0] == 0xFF && matches!(w[1], 0xC0 | 0xC2 | 0xC9 | 0xCA))
        .expect("SOF");
    if v[sof + 1] == 0xC0 {
        v[sof + 1] = 0xC1;
    }
    assert_eq!(v[sof + 4], 8);
    v[sof + 4] = 12;
    v
}

/// Non-default conditioning (DAC: DC L/U, AC Kx; see `tests/fixtures/make-arith-dac.sh`):
/// every decode path gives the planes of the Huffman-coded file the coefficients came from,
/// bit for bit, also relabelled 12-bit; djpeg decodes both files identically too. The 12-bit
/// files (coefficients times 16) decode to the 8-bit file's samples.
#[test]
fn dac_conditioning() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let read = |f: &str| std::fs::read(dir.join(f)).unwrap_or_else(|e| panic!("{f}: {e}"));
    let paths = |jpg: &[u8], name: &str| {
        let ix = build_index(jpg).unwrap();
        let mut out = Vec::new();
        for scale in SCALES {
            let d = decode(jpg, &opts(scale, Parallelism::Single)).unwrap();
            assert!(!d.truncated, "{name} {scale:?}");
            for par in [Parallelism::Threads(2), Parallelism::Threads(5)] {
                assert!(
                    decode(jpg, &opts(scale, par)).unwrap() == d,
                    "{name} {scale:?} {par:?}"
                );
                let di = decode_with_index(jpg, &ix, &opts(scale, par)).unwrap();
                assert!(di == d, "{name} {scale:?} {par:?}: indexed");
            }
            let r = Region {
                x: d.width / 4,
                y: d.height / 3,
                w: (d.width / 2).max(1),
                h: (d.height / 3).max(1),
            };
            let o = DecodeOptions {
                region: Some(r),
                ..opts(scale, Parallelism::Threads(3))
            };
            crop_equal(
                &d,
                &decode(jpg, &o).unwrap(),
                &format!("{name} {scale:?} {r:?}"),
            );
            out.push(d);
        }
        out
    };
    for (arith, huff) in [
        ("arith_dac_420.jpg", "huff_dac_420.jpg"),
        ("arith_dac_444_prog_rst.jpg", "huff_dac_444.jpg"),
        ("arith_dac_gray.jpg", "huff_dac_gray.jpg"),
    ] {
        let (a, h) = (read(arith), read(huff));
        assert_eq!(read_info(&a).unwrap().coding, Coding::Arithmetic, "{arith}");
        assert!(
            a.windows(2).any(|w| w == [0xFF, 0xCC]),
            "{arith}: no DAC segment"
        );
        let da = paths(&a, arith);
        for (scale, d) in SCALES.iter().zip(&da) {
            let dh = decode(&h, &opts(*scale, Parallelism::Single)).unwrap();
            assert!(*d == dh, "{arith} {scale:?}: differs from {huff}");
        }
        let (a12, h12) = (as_12bit(&a), as_12bit(&h));
        let d12 = paths(&a12, &format!("{arith} as 12-bit"));
        for (scale, d) in SCALES.iter().zip(&d12) {
            let dh = decode(&h12, &opts(*scale, Parallelism::Single)).unwrap();
            assert!(*d == dh, "{arith} as 12-bit {scale:?}: differs from {huff}");
        }
        if tools_available() {
            let ra = djpeg_file(&dir.join(arith), &["-dct", "int", "-pnm"]);
            let rh = djpeg_file(&dir.join(huff), &["-dct", "int", "-pnm"]);
            assert!(
                ra.data == rh.data,
                "{arith}: djpeg decodes differ from {huff}"
            );
            let d = &da[0];
            let (max, mean) = if d.planes.len() == 1 {
                let y = &d.planes[0];
                diff_plane_pgm(&y.data, y.stride, y.width as usize, y.height as usize, &ra)
            } else {
                diff_rgba_pnm(&planes_to_rgba(d), d.width as usize, d.height as usize, &ra)
            };
            assert!(max <= 2, "{arith}: djpeg max {max} mean {mean:.4}");
        }
    }
    for (arith, huff) in [
        ("dac12_420_rst.jpg", "huff_dac_420.jpg"),
        ("dac12_444_prog.jpg", "huff_dac_444.jpg"),
    ] {
        let a = read(arith);
        let info = read_info(&a).unwrap();
        assert_eq!(
            (info.coding, info.precision),
            (Coding::Arithmetic, 12),
            "{arith}"
        );
        let d = paths(&a, arith).swap_remove(0);
        let h = decode(&read(huff), &opts(Scale::Full, Parallelism::Single)).unwrap();
        let mut max = 0;
        for (pa, ph) in d.planes.iter().zip(&h.planes) {
            assert_eq!((pa.width, pa.height), (ph.width, ph.height), "{arith}");
            for y in 0..pa.height as usize {
                for x in 0..pa.width as usize {
                    let (s12, s8) = (pa.data[y * pa.stride + x], ph.data[y * ph.stride + x]);
                    max = max.max(s12.abs_diff(s8));
                }
            }
        }
        assert!(
            max <= 1,
            "{arith}: 12-bit samples off the 8-bit ones by {max}"
        );
    }
}

/// Scan scripts (`cjpeg -scans`) for the pipelined progressive decode: non-interleaved DC
/// first scans, split and out-of-order AC bands, chroma refined before luma is complete, a
/// DC refinement between AC scans, four successive-approximation steps.
const SCAN_SCRIPTS: &[(&str, &str, bool)] = &[
    (
        "odd",
        "0: 0 0 0 2;\n1: 0 0 0 2;\n2: 0 0 0 2;\n2: 1 63 0 2;\n0: 1 9 0 3;\n0: 10 63 0 3;\n\
         0 1 2: 0 0 2 1;\n1: 1 5 0 1;\n0: 1 63 3 2;\n1: 6 63 0 1;\n0: 1 63 2 1;\n\
         2: 1 63 2 1;\n1: 1 63 1 0;\n0 1 2: 0 0 1 0;\n0: 1 63 1 0;\n2: 1 63 1 0;\n",
        false,
    ),
    (
        "chroma_first",
        "1 2: 0 0 0 0;\n2: 1 63 0 0;\n1: 1 63 0 0;\n0: 0 0 0 0;\n0: 40 63 0 0;\n\
         0: 1 39 0 1;\n0: 1 39 1 0;\n",
        false,
    ),
    (
        "gray",
        "0: 0 0 0 1;\n0: 1 2 0 0;\n0: 3 63 0 2;\n0: 0 0 1 0;\n0: 3 63 2 1;\n0: 3 63 1 0;\n",
        true,
    ),
];

fn scans_file(sname: &str, script: &str) -> String {
    let sp = tmp_dir().join(format!("scans_{sname}.txt"));
    if std::fs::read_to_string(&sp).ok().as_deref() != Some(script) {
        let tmp = sp.with_extension(format!("{}.tmp", uniq()));
        std::fs::write(&tmp, script).unwrap();
        std::fs::rename(&tmp, &sp).unwrap();
    }
    sp.to_str().unwrap().to_string()
}

/// Progressive files without restart markers decode their scans concurrently (pipelined by
/// MCU row): bit-identical to the serial decode at every scale, thread count and for regions,
/// for cjpeg's default script and for unusual scan orders, and equal to the Huffman twins.
#[test]
fn progressive_pipeline_equals_serial() {
    if !tools_available() {
        return;
    }
    let mut files: Vec<(String, Vec<u8>, Option<Vec<u8>>)> = all_images()
        .into_iter()
        .filter(|(_, _, _, v)| v.name.contains("prog") && !v.name.contains("rst"))
        .map(|(n, a, _, _)| (n, a, None))
        .collect();
    for (i, &(w, h)) in [(1021, 769), (333, 251), (17, 9)].iter().enumerate() {
        let src = source_ppm(w, h, 77 + i as u32);
        for &(sname, script, gray) in SCAN_SCRIPTS {
            let sp = scans_file(sname, script);
            let mut args = vec!["-quality", "90", "-sample", "2x2", "-scans", &sp];
            if gray {
                args.push("-grayscale");
            }
            let huff = cjpeg(&src, &format!("huff_scans_{sname}_{w}x{h}"), &args);
            args.push("-arithmetic");
            let name = format!("arith_scans_{sname}_{w}x{h}");
            let a = cjpeg(&src, &name, &args);
            files.push((name, a, Some(huff)));
        }
    }
    for (name, a, huff) in &files {
        let info = read_info(a).unwrap();
        assert_eq!(info.sof_marker, 0xCA, "{name}");
        let ix = build_index(a).unwrap();
        for scale in SCALES {
            let one = decode(a, &opts(scale, Parallelism::Single)).unwrap();
            assert!(!one.truncated, "{name} {scale:?}");
            if let Some(h) = huff {
                let dh = decode(h, &opts(scale, Parallelism::Single)).unwrap();
                assert!(one == dh, "{name} {scale:?}: differs from the Huffman twin");
            }
            for n in [2, 3, 4, 7, 16] {
                let d = decode(a, &opts(scale, Parallelism::Threads(n))).unwrap();
                assert!(
                    d == one,
                    "{name} {scale:?} threads {n}: differs from serial"
                );
            }
            let (ow, oh) = (
                info.width.div_ceil(scale as u32),
                info.height.div_ceil(scale as u32),
            );
            let r = Region {
                x: ow / 4,
                y: oh / 3,
                w: (ow / 2).max(1),
                h: (oh / 3).max(1),
            };
            let o = DecodeOptions {
                region: Some(r),
                ..opts(scale, Parallelism::Threads(5))
            };
            crop_equal(
                &one,
                &decode(a, &o).unwrap(),
                &format!("{name} {scale:?} {r:?}"),
            );
            // Indexed region decodes: every scan from its row checkpoint, pipelined, equal
            // to the serial indexed decode at every thread count.
            let regions = [
                r,
                Region {
                    x: 0,
                    y: oh - oh.min(3),
                    w: ow,
                    h: oh.min(3),
                },
                Region {
                    x: ow / 2,
                    y: oh / 2,
                    w: 1,
                    h: 1,
                },
            ];
            for r in regions {
                let o1 = DecodeOptions {
                    region: Some(r),
                    ..opts(scale, Parallelism::Single)
                };
                let serial = decode_with_index(a, &ix, &o1).unwrap();
                crop_equal(&one, &serial, &format!("{name} {scale:?} {r:?} indexed"));
                for n in [2, 3, 4, 7, 16] {
                    let o = DecodeOptions {
                        region: Some(r),
                        ..opts(scale, Parallelism::Threads(n))
                    };
                    let d = decode_with_index(a, &ix, &o).unwrap();
                    assert!(
                        d == serial,
                        "{name} {scale:?} {r:?} threads {n}: indexed region differs from serial"
                    );
                }
            }
        }
    }
}

/// Cut-short and corrupted progressive files without restarts: the pipelined decode gives the
/// serial decode's planes and truncation flag (every scan stops where the serial one does).
#[test]
fn progressive_pipeline_cut_and_corrupt() {
    if !tools_available() {
        return;
    }
    let src = source_ppm(1021, 769, 31);
    let sp = scans_file(SCAN_SCRIPTS[0].0, SCAN_SCRIPTS[0].1);
    let odd = cjpeg(
        &src,
        "arith_scans_odd_cut_src",
        &["-quality", "90", "-scans", &sp, "-arithmetic"],
    );
    let def = cjpeg(
        &src,
        "arith_prog_cut_src",
        &["-quality", "92", "-progressive", "-arithmetic"],
    );
    for (name, jpg) in [("odd", odd), ("default", def)] {
        let info = read_info(&jpg).unwrap();
        let ix = build_index(&jpg).unwrap();
        let body = jpg.len() - info.header_len;
        for i in 1..12 {
            let cut = info.header_len + body * i / 12;
            for scale in [Scale::Full, Scale::Quarter] {
                let one = decode(&jpg[..cut], &opts(scale, Parallelism::Single)).unwrap();
                assert!(one.truncated, "{name} cut {cut} not flagged");
                for n in [2, 6] {
                    let d = decode(&jpg[..cut], &opts(scale, Parallelism::Threads(n))).unwrap();
                    assert!(d.truncated, "{name} cut {cut} threads {n} not flagged");
                    assert!(d == one, "{name} cut {cut} {scale:?} threads {n}: differs");
                }
                // The intact file's index on the cut file: a region from the checkpoints.
                let (ow, oh) = (
                    info.width.div_ceil(scale as u32),
                    info.height.div_ceil(scale as u32),
                );
                let r = Region {
                    x: ow / 5,
                    y: oh / 3,
                    w: ow / 2,
                    h: oh / 4,
                };
                let o1 = DecodeOptions {
                    region: Some(r),
                    ..opts(scale, Parallelism::Single)
                };
                let serial = decode_with_index(&jpg[..cut], &ix, &o1);
                for n in [2, 6] {
                    let o = DecodeOptions {
                        region: Some(r),
                        ..opts(scale, Parallelism::Threads(n))
                    };
                    let d = decode_with_index(&jpg[..cut], &ix, &o);
                    match (&serial, &d) {
                        (Ok(a), Ok(b)) => {
                            assert!(
                                a == b,
                                "{name} cut {cut} {scale:?} threads {n}: indexed region differs"
                            );
                            assert_eq!(a.truncated, b.truncated);
                        }
                        (Err(_), Err(_)) => {}
                        _ => panic!(
                            "{name} cut {cut} {scale:?} threads {n}: indexed outcome differs"
                        ),
                    }
                }
            }
        }
        let mut x = 0x2545_f491u32;
        for round in 0..6 {
            let mut bad = jpg.clone();
            for _ in 0..(1 + round * 8) {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                let at = info.header_len + 20 + (x as usize % (body - 22));
                bad[at] = (x >> 8) as u8;
            }
            let Ok(one) = decode(&bad, &opts(Scale::Full, Parallelism::Single)) else {
                continue;
            };
            for n in [2, 5, 16] {
                let d = decode(&bad, &opts(Scale::Full, Parallelism::Threads(n))).unwrap();
                assert!(
                    d == one,
                    "{name} corrupt round {round} threads {n}: differs"
                );
                assert_eq!(d.truncated, one.truncated, "{name} corrupt round {round}");
            }
        }
    }
}

/// A cancelled pipelined decode stops with `Cancelled` (no worker left waiting).
#[test]
fn progressive_pipeline_cancel() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let jpg = std::fs::read(dir.join("arith_420_prog.jpg")).unwrap();
    let stop = std::sync::atomic::AtomicBool::new(true);
    for n in [2, 4, 16] {
        let o = DecodeOptions {
            cancel: Some(&stop),
            ..opts(Scale::Full, Parallelism::Threads(n))
        };
        assert!(
            matches!(decode(&jpg, &o), Err(JpegError::Cancelled)),
            "threads {n}"
        );
    }
}
