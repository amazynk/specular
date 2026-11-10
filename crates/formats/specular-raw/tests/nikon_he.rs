//! Nikon High Efficiency (HE, HE*: intoPIX TicoRAW) against its oracles, on the corpus of
//! tools/corpus/fetch-nikon-he.sh (skipped without it):
//! - Adobe DNG Converter 18.4's decode of the same file (`--adobe`): the CFA values, whose
//!   black, white and grid are the same as ours.
//! - The camera's embedded full-size JPEG: structure similarity of our develop.
//! - A lossless NEF of the same scene (tripod series): PSNR of the sensor values.
//!
//! HE is lossy, so these are error statistics, not bit-exactness. Run with `--nocapture` for
//! the per-file numbers.

use std::path::{Path, PathBuf};

use specular_raw::{DecodeOptions, OutputSize, SensorCompression};

fn corpus() -> Option<PathBuf> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/nikon-he");
    p.is_dir().then_some(p)
}

/// (file, HE*, lossless NEF of the same scene).
const FILES: &[(&str, bool, Option<&str>)] = &[
    ("Z f/DSC_0043.NEF", false, None),
    ("Z f/DSC_0042.NEF", true, None),
    (
        "Z6_3/Nikon_Z6__3_High_Efficiency_1_1.NEF",
        false,
        Some("Z6_3/Nikon_Z6__3_Lossless_compressed_1_1.NEF"),
    ),
    (
        "Z6_3/Nikon_Z6__3_High_Efficiency_Star_1_1.NEF",
        true,
        Some("Z6_3/Nikon_Z6__3_Lossless_compressed_1_1.NEF"),
    ),
    (
        "Z50_2/DSC_0062_DX_HE.NEF",
        false,
        Some("Z50_2/DSC_0060_DX_Lossless.NEF"),
    ),
    (
        "Z50_2/DSC_0061_DX_HE_Star.NEF",
        true,
        Some("Z50_2/DSC_0060_DX_Lossless.NEF"),
    ),
    ("Z5_2/High_Efficiency_DX.NEF", false, None),
];

fn psnr(mse: f64, peak: f64) -> f64 {
    10.0 * (peak * peak / mse.max(1e-12)).log10()
}

/// Mean SSIM over 8×8 blocks of two equally sized grey images (values 0..1).
fn ssim(a: &[f64], b: &[f64], w: usize, h: usize) -> f64 {
    let (c1, c2) = (0.01f64.powi(2), 0.03f64.powi(2));
    let (mut sum, mut n) = (0.0, 0.0);
    for by in (0..h.saturating_sub(7)).step_by(4) {
        for bx in (0..w.saturating_sub(7)).step_by(4) {
            let (mut ma, mut mb, mut va, mut vb, mut cv) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for y in by..by + 8 {
                for x in bx..bx + 8 {
                    ma += a[y * w + x];
                    mb += b[y * w + x];
                }
            }
            ma /= 64.0;
            mb /= 64.0;
            for y in by..by + 8 {
                for x in bx..bx + 8 {
                    let (da, db) = (a[y * w + x] - ma, b[y * w + x] - mb);
                    va += da * da;
                    vb += db * db;
                    cv += da * db;
                }
            }
            let (va, vb, cv) = (va / 63.0, vb / 63.0, cv / 63.0);
            sum += ((2.0 * ma * mb + c1) * (2.0 * cv + c2))
                / ((ma * ma + mb * mb + c1) * (va + vb + c2));
            n += 1.0;
        }
    }
    sum / n
}

/// Box-downscales an interleaved image to `tw`×`th` luma (Rec.709 weights).
fn luma(
    px: impl Fn(usize, usize) -> [f64; 3],
    w: usize,
    h: usize,
    tw: usize,
    th: usize,
) -> Vec<f64> {
    let mut out = vec![0.0; tw * th];
    for ty in 0..th {
        for tx in 0..tw {
            let (x0, x1) = (tx * w / tw, ((tx + 1) * w / tw).max(tx * w / tw + 1));
            let (y0, y1) = (ty * h / th, ((ty + 1) * h / th).max(ty * h / th + 1));
            let mut s = 0.0;
            for y in y0..y1 {
                for x in x0..x1 {
                    let [r, g, b] = px(x, y);
                    s += 0.2126 * r + 0.7152 * g + 0.0722 * b;
                }
            }
            out[ty * tw + tx] = s / ((x1 - x0) * (y1 - y0)) as f64;
        }
    }
    out
}

/// SSIM of an image's luma (sRGB-encoded, the middle `preview`-sized part of a half-size
/// develop) against the preview's, both at 1/8 of the preview size, means matched.
fn preview_ssim(img: &specular_raw::RgbImage, pv: &specular_raw::PreviewImage) -> f64 {
    let (pw, ph) = (pv.width as usize, pv.height as usize);
    let (tw, th) = (pw / 8, ph / 8);
    let srgb = |v: f64| {
        let v = v.clamp(0.0, 1.0);
        if v <= 0.003_130_8 {
            12.92 * v
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        }
    };
    let (iw, ih) = (img.width as usize, img.height as usize);
    let (cw, ch) = ((pw / 2).min(iw), (ph / 2).min(ih));
    let (x0, y0) = ((iw - cw) / 2, (ih - ch) / 2);
    let mut ours = luma(
        |x, y| {
            let i = 3 * ((y + y0) * iw + x + x0);
            [0, 1, 2].map(|c| srgb(f64::from(img.data[i + c]) / 65535.0))
        },
        cw,
        ch,
        tw,
        th,
    );
    let theirs = luma(
        |x, y| {
            let i = 4 * (y * pw + x);
            [0, 1, 2].map(|c| f64::from(pv.rgba[i + c]) / 255.0)
        },
        pw,
        ph,
        tw,
        th,
    );
    let (ma, mb) = (
        ours.iter().sum::<f64>() / ours.len() as f64,
        theirs.iter().sum::<f64>() / theirs.len() as f64,
    );
    ours.iter_mut().for_each(|v| *v *= mb / ma.max(1e-9));
    ssim(&ours, &theirs, tw, th)
}

/// PSNR of two exposures of one scene: full size, and of 16x16 block means per CFA site;
/// black-subtracted, `b` scaled to `a`'s mean.
fn same_scene(a: &specular_raw::RawImage, b: &specular_raw::RawImage) -> (f64, f64) {
    let w = a.width as usize;
    let h = a.height as usize;
    let lin = |r: &specular_raw::RawImage, i: usize| {
        let k = ((i / w) & 1) * 2 + ((i % w) & 1);
        f64::from(r.data[i]) - f64::from(r.black[k])
    };
    let n = w * h;
    let (sa, sb) = (0..n).fold((0.0, 0.0), |(x, y), i| (x + lin(a, i), y + lin(b, i)));
    let g = sa / sb;
    let full = (0..n)
        .map(|i| (lin(a, i) - g * lin(b, i)).powi(2))
        .sum::<f64>()
        / n as f64;
    let (bw, bh) = (w / 32, h / 32);
    let mut se = 0.0;
    for by in 0..bh {
        for bx in 0..bw {
            for site in 0..4 {
                let (mut ma, mut mb) = (0.0, 0.0);
                for y in 0..16 {
                    for x in 0..16 {
                        let i = (by * 32 + 2 * y + site / 2) * w + bx * 32 + 2 * x + site % 2;
                        ma += lin(a, i);
                        mb += lin(b, i);
                    }
                }
                se += ((ma - g * mb) / 256.0).powi(2);
            }
        }
    }
    let blocks = se / (bw * bh * 4) as f64;
    let peak = 15892.0 - 1008.0;
    (psnr(full, peak), psnr(blocks, peak))
}

#[test]
fn high_efficiency_against_oracles() {
    let Some(root) = corpus() else {
        eprintln!("skipped: no tools/corpus/nikon-he (tools/corpus/fetch-nikon-he.sh)");
        return;
    };
    let opts = DecodeOptions::default();
    let mut checked = 0;
    for &(rel, star, lossless) in FILES {
        let Ok(d) = std::fs::read(root.join(rel)) else {
            continue;
        };
        let f = specular_raw::parse(&d).unwrap();
        let s = f.info().sensor.as_ref().unwrap();
        assert_eq!(
            s.compression,
            SensorCompression::NikonHighEfficiency { star },
            "{rel}"
        );
        assert!(s.decodable, "{rel}: {:?}", s.unsupported_reason);
        let t = std::time::Instant::now();
        let raw = f.decode_raw(&d, &opts).unwrap();
        let ms = t.elapsed().as_millis();
        assert!(!raw.truncated, "{rel}");
        assert_eq!(raw.white, 15892.0, "{rel}");
        let mut line = format!("{rel}: {}x{} decoded in {ms} ms", raw.width, raw.height);

        // (a) Adobe DNG Converter's decode: same grid, black 1008, white 15892.
        let dng_path = root.join("adobe").join(rel).with_extension("dng");
        let adobe = std::fs::read(&dng_path);
        if adobe.is_err() {
            eprintln!(
                "{rel}: no Adobe DNG, the Adobe checks are skipped (fetch-nikon-he.sh --adobe)"
            );
        }
        if let Ok(dd) = adobe {
            let a = specular_raw::parse(&dd).unwrap();
            let ar = a.decode_raw_as_stored(&dd, &opts).unwrap();
            assert_eq!((ar.width, ar.height), (raw.width, raw.height), "{rel}");
            let (mut max, mut diff, mut se) = (0i32, 0usize, 0f64);
            for (&x, &y) in raw.data.iter().zip(&ar.data) {
                let e = i32::from(x) - i32::from(y);
                max = max.max(e.abs());
                diff += usize::from(e != 0);
                se += f64::from(e * e);
            }
            let n = raw.data.len();
            let mse = se / n as f64;
            line += &format!(
                " | Adobe: max |e| {max}, {:.2}% differ, PSNR {:.1} dB",
                100.0 * diff as f64 / n as f64,
                psnr(mse, 15892.0 - 1008.0)
            );
            assert!(max <= 1, "{rel}: {max}");
            assert!(diff * 100 < n * 8, "{rel}: {diff} of {n}");
        }

        // (b) The embedded JPEG against our develop, and against the develop of Adobe's
        // decode (same develop, no lens opcodes) as the baseline: tone, sharpening and
        // Picture Control keep both well under 1.
        if let Some(p) = f.info().largest_preview()
            && p.width >= 2000
        {
            let pv = specular_raw::decode_preview(&d, p, 0).unwrap();
            let half = DecodeOptions {
                size: OutputSize::Half,
                ..Default::default()
            };
            let ours = preview_ssim(&f.decode(&d, &half).unwrap(), &pv);
            line += &format!(" | preview SSIM {ours:.3}");
            if let Ok(dd) = std::fs::read(&dng_path) {
                let a = specular_raw::parse(&dd).unwrap();
                let ar = a.decode_raw_as_stored(&dd, &half).unwrap();
                let base = preview_ssim(&specular_raw::develop(&ar, &half).unwrap(), &pv);
                line += &format!(" (Adobe's decode {base:.3})");
                assert!(ours > base - 0.01, "{rel}: SSIM {ours} vs {base}");
            }
        }

        // (c) A lossless NEF of the same scene (a separate exposure: noise and small scene
        // changes differ): sensor values black-subtracted, at full size and as 16x16 block
        // means per CFA site, after matching the overall gain.
        if let Some(l) = lossless
            && let Ok(ld) = std::fs::read(root.join(l))
        {
            let lf = specular_raw::parse(&ld).unwrap();
            let lr = lf.decode_raw(&ld, &opts).unwrap();
            assert_eq!((lr.width, lr.height), (raw.width, raw.height), "{rel}");
            let (full, blocks) = same_scene(&raw, &lr);
            line += &format!(" | lossless: PSNR {full:.1} dB, block means {blocks:.1} dB");
            assert!(blocks > 28.0, "{rel}: {blocks}");
        }
        eprintln!("{line}");
        checked += 1;
    }
    eprintln!("{checked} High Efficiency files checked");
}
