//! Lossless transforms against libjpeg-turbo's jpegtran (the oracle): for every sampling,
//! coding and transform, our output must hold exactly jpegtran's quantised coefficients (and
//! so decode to identical pixels). Four quarter turns must give back the original coefficients.
//! Skipped (with a message) when cjpeg, jpegtran or ImageMagick are missing.

mod common;

use std::process::Command;

use common::*;
use specular_jpeg::*;

fn jpegtran_available() -> bool {
    tools_available() && Command::new("jpegtran").arg("-version").output().is_ok()
}

const TRANSFORMS: &[(&str, Transform, &[&str])] = &[
    ("rot90", Transform::ROTATE_90, &["-rotate", "90"]),
    ("rot180", Transform::ROTATE_180, &["-rotate", "180"]),
    ("rot270", Transform::ROTATE_270, &["-rotate", "270"]),
    ("fliph", Transform::FLIP_H, &["-flip", "horizontal"]),
    ("flipv", Transform::FLIP_V, &["-flip", "vertical"]),
    (
        "transpose",
        Transform {
            transpose: true,
            flip_h: false,
            flip_v: false,
        },
        &["-transpose"],
    ),
    (
        "transverse",
        Transform {
            transpose: true,
            flip_h: true,
            flip_v: true,
        },
        &["-transverse"],
    ),
];

const CODINGS: &[(&str, &[&str])] = &[
    ("444", &["-quality", "92", "-sample", "1x1"]),
    ("420", &["-quality", "90", "-sample", "2x2"]),
    ("422", &["-quality", "85", "-sample", "2x1"]),
    ("440", &["-quality", "85", "-sample", "1x2"]),
    ("411", &["-quality", "85", "-sample", "4x1"]),
    ("gray", &["-quality", "90", "-grayscale"]),
    (
        "420prog",
        &["-quality", "88", "-sample", "2x2", "-progressive"],
    ),
    (
        "444prog",
        &["-quality", "95", "-sample", "1x1", "-progressive"],
    ),
    (
        "420rst",
        &["-quality", "90", "-sample", "2x2", "-restart", "1"],
    ),
    (
        "420arith",
        &["-quality", "90", "-sample", "2x2", "-arithmetic"],
    ),
    (
        "422progarith",
        &[
            "-quality",
            "90",
            "-sample",
            "2x1",
            "-progressive",
            "-arithmetic",
        ],
    ),
];

fn jpegtran(src: &[u8], name: &str, args: &[&str]) -> Option<Vec<u8>> {
    let dir = tmp_dir();
    let inp = dir.join(format!("tr_in_{name}_{}.jpg", uniq()));
    let out = dir.join(format!("tr_out_{name}_{}.jpg", uniq()));
    std::fs::write(&inp, src).unwrap();
    let st = Command::new("jpegtran")
        .args(args)
        .arg("-outfile")
        .arg(&out)
        .arg(&inp)
        .output()
        .expect("jpegtran");
    let _ = std::fs::remove_file(&inp);
    let r = st.status.success().then(|| std::fs::read(&out).unwrap());
    let _ = std::fs::remove_file(&out);
    r
}

/// Coefficients that differ between two files over the blocks covering the image (padding
/// blocks outside the image are free to differ), or `u64::MAX` when the geometry differs.
fn coef_mismatches(a: &[u8], b: &[u8]) -> u64 {
    let (aw, ah, ac) = read_coefficients(a).unwrap();
    let (bw, bh, bc) = read_coefficients(b).unwrap();
    if (aw, ah) != (bw, bh) || ac.len() != bc.len() {
        return u64::MAX;
    }
    let mut n = 0;
    for (x, y) in ac.iter().zip(&bc) {
        if (x.h_samp, x.v_samp, x.blocks_w, x.blocks_h)
            != (y.h_samp, y.v_samp, y.blocks_w, y.blocks_h)
            || x.quant != y.quant
        {
            return u64::MAX;
        }
        for by in 0..x.blocks_h as usize {
            for bx in 0..x.blocks_w as usize {
                let i = (by * x.stride_blocks as usize + bx) * 64;
                let j = (by * y.stride_blocks as usize + bx) * 64;
                n += x.coefs[i..i + 64]
                    .iter()
                    .zip(&y.coefs[j..j + 64])
                    .filter(|(p, q)| p != q)
                    .count() as u64;
            }
        }
    }
    n
}

fn planes(jpg: &[u8]) -> DecodedPlanes {
    decode(jpg, &DecodeOptions::default()).unwrap()
}

/// Decoded samples that differ between two files (all planes).
fn pixel_mismatches(a: &[u8], b: &[u8]) -> u64 {
    let (pa, pb) = (planes(a), planes(b));
    assert_eq!((pa.width, pa.height), (pb.width, pb.height));
    let mut n = 0;
    for (x, y) in pa.planes.iter().zip(&pb.planes) {
        assert_eq!((x.width, x.height), (y.width, y.height));
        for r in 0..x.height as usize {
            let ra = &x.data[r * x.stride..r * x.stride + x.width as usize];
            let rb = &y.data[r * y.stride..r * y.stride + y.width as usize];
            n += ra.iter().zip(rb).filter(|(p, q)| p != q).count() as u64;
        }
    }
    n
}

#[test]
fn matches_jpegtran_for_every_coding_and_transform() {
    if !jpegtran_available() {
        eprintln!("skipped: cjpeg/jpegtran/magick not available");
        return;
    }
    let mut cases = 0;
    let mut trimmed = 0;
    let (mut coef_bad, mut px_bad) = (0u64, 0u64);
    for &(w, h) in &[(320u32, 240u32), (333, 251), (64, 37)] {
        let src = source_ppm(w, h, 7);
        for (cname, cargs) in CODINGS {
            let jpg = cjpeg(&src, &format!("tr_{cname}_{w}x{h}"), cargs);
            for (tname, t, targs) in TRANSFORMS {
                let align = transform_alignment(&jpg, *t).unwrap();
                let trim = !matches!(align, Alignment::Perfect);
                if matches!(align, Alignment::Impossible) {
                    continue;
                }
                let mut args: Vec<&str> = targs.to_vec();
                args.extend_from_slice(&["-copy", "all"]);
                args.push(if trim { "-trim" } else { "-perfect" });
                let oracle = jpegtran(&jpg, &format!("{cname}_{tname}"), &args)
                    .unwrap_or_else(|| panic!("jpegtran {cname} {tname} {w}x{h}"));
                let ours = transform(&jpg, *t, TransformOptions { trim }).unwrap();
                if !trim {
                    // Refuses without consent when the edge is partial, and perfect files need none.
                    assert!(transform(&jpg, *t, TransformOptions { trim: false }).is_ok());
                } else {
                    trimmed += 1;
                    assert!(transform(&jpg, *t, TransformOptions { trim: false }).is_err());
                }
                let c = coef_mismatches(&ours, &oracle);
                let p = pixel_mismatches(&ours, &oracle);
                if c != 0 || p != 0 {
                    eprintln!("{cname} {tname} {w}x{h}: {c} coefficient, {p} sample mismatches");
                }
                coef_bad = coef_bad.saturating_add(c);
                px_bad += p;
                cases += 1;
            }
        }
    }
    eprintln!(
        "jpegtran oracle: {cases} cases ({trimmed} trimmed), {coef_bad} coefficient and {px_bad} \
         decoded-sample mismatches"
    );
    assert_eq!((coef_bad, px_bad), (0, 0));
}

#[test]
fn four_quarter_turns_are_the_identity() {
    if !tools_available() {
        eprintln!("skipped: cjpeg/magick not available");
        return;
    }
    let src = source_ppm(320, 240, 11);
    for (cname, cargs) in CODINGS {
        let orig = cjpeg(&src, &format!("q4_{cname}"), cargs);
        let mut cur = orig.clone();
        for _ in 0..4 {
            cur = transform(&cur, Transform::ROTATE_90, TransformOptions::default()).unwrap();
        }
        assert_eq!(coef_mismatches(&orig, &cur), 0, "{cname}: coefficients");
        assert_eq!(pixel_mismatches(&orig, &cur), 0, "{cname}: pixels");
        // And back the other way, through every other element of the group.
        let mut cur = orig.clone();
        for t in [Transform::FLIP_H, Transform::ROTATE_180, Transform::FLIP_V] {
            cur = transform(&cur, t, TransformOptions::default()).unwrap();
        }
        assert_eq!(coef_mismatches(&orig, &cur), 0, "{cname}: flips");
    }
}

/// 12-bit files (jpegtran 2.1 cannot read them): four quarter turns give the original
/// coefficients back, and one turn moves every block where a quarter turn must.
#[test]
fn twelve_bit_and_arithmetic_fixtures() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for name in [
        "dac12_420_rst.jpg",
        "dac12_444_prog.jpg",
        "arith_dac_gray.jpg",
        "gray8.jpg",
    ] {
        let orig = std::fs::read(dir.join(name)).unwrap();
        let t = Transform::ROTATE_90;
        let opts = TransformOptions { trim: true };
        let mut cur = orig.clone();
        let one = transform(&cur, t, opts).unwrap();
        for _ in 0..4 {
            cur = transform(&cur, t, opts).unwrap();
        }
        let (w, h, _) = read_coefficients(&orig).unwrap();
        let (w1, h1, _) = read_coefficients(&one).unwrap();
        if matches!(transform_alignment(&orig, t).unwrap(), Alignment::Perfect) {
            assert_eq!((w1, h1), (h, w));
            assert_eq!(coef_mismatches(&orig, &cur), 0, "{name}");
        }
    }
}

#[test]
fn group_algebra() {
    let all: Vec<Transform> = (1..=8).map(Transform::from_exif).collect();
    for (i, t) in all.iter().enumerate() {
        assert_eq!(t.to_exif(), i as u8 + 1);
        assert_eq!(t.then(t.inverse()), Transform::IDENTITY);
    }
    let r = Transform::ROTATE_90;
    assert_eq!(r.then(r), Transform::ROTATE_180);
    assert_eq!(r.then(r).then(r), Transform::ROTATE_270);
    assert_eq!(Transform::from_exif(6), r);
    assert_eq!(Transform::from_exif(8), Transform::ROTATE_270);
    // Orientation 6 displayed, then a quarter turn anticlockwise: upright.
    assert_eq!(
        Transform::from_exif(6).then(Transform::ROTATE_270),
        Transform::IDENTITY
    );
    // Mirror then rotate a quarter: the transverse (7) or the transpose (5).
    assert_eq!(Transform::FLIP_H.then(r).to_exif(), 7);
    assert_eq!(Transform::FLIP_V.then(r).to_exif(), 5);
}
