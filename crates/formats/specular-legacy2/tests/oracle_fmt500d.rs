//! The fmt500d corpus against Pillow (tools/corpus/fetch-fmt500d.sh, then
//! tools/corpus/fmt500d-oracle.py). Run with `SPECULAR_FMT500D_CORPUS=$CARGO_TARGET_DIR/corpus/fmt500d
//! cargo test -p specular-legacy2 --test oracle_fmt500d -- --ignored --nocapture`.

use specular_legacy2::{
    Image, Pixels, gd, ggr, gimp, gpl, imt, labeye, mcidas, msp, pgx, pxr, xvthumb,
};
use std::path::Path;

type DecodeFn = fn(&[u8], u32) -> specular_legacy2::Result<Image>;

fn rgba8(img: &Image) -> Vec<u8> {
    match &img.pixels {
        Pixels::Gray8(v) => v.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        Pixels::Rgba8(v) => v.clone(),
        Pixels::Gray16(v) => v
            .iter()
            .flat_map(|&g| {
                let g = (g >> 8) as u8;
                [g, g, g, 255]
            })
            .collect(),
        other => panic!("unexpected pixels {:?}", std::mem::discriminant(other)),
    }
}

/// (bytes differing, largest difference).
fn diff(a: &[u8], b: &[u8]) -> (usize, u8) {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).fold((0, 0), |(n, m), (&x, &y)| {
        let d = x.abs_diff(y);
        (n + usize::from(d > 0), m.max(d))
    })
}

fn f64s(p: &Path) -> Vec<f64> {
    std::fs::read(p)
        .unwrap()
        .chunks_exact(8)
        .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

fn with(p: &Path, ext: &str) -> std::path::PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(ext);
    s.into()
}

#[test]
#[ignore]
fn corpus_matches_pillow() {
    let Some(root) = std::env::var_os("SPECULAR_FMT500D_CORPUS") else {
        eprintln!("SPECULAR_FMT500D_CORPUS not set");
        return;
    };
    let root = Path::new(&root);
    let mut bad = Vec::new();
    let mut check = |name: &str, f: DecodeFn, page: u32, oracle: &str, tolerance: u8| {
        let path = root.join(name);
        let data = std::fs::read(&path).unwrap();
        let img = f(&data, page).unwrap_or_else(|e| panic!("{name}: {e}"));
        let want = std::fs::read(root.join(oracle)).unwrap();
        let (n, m) = diff(&rgba8(&img), &want);
        println!(
            "{name}: {}×{}, {n} of {} bytes differ (max {m})",
            img.width,
            img.height,
            want.len()
        );
        if m > tolerance {
            bad.push(name.to_owned());
        }
    };
    check("hopper.msp", msp::decode, 0, "hopper.msp.rgba", 0);
    check("gen/hopper1.msp", msp::decode, 0, "gen/hopper1.msp.rgba", 0);
    check("hopper.pxr", pxr::decode, 0, "hopper.pxr.rgba", 0);
    check("hopper.im", labeye::decode, 0, "hopper.im.rgba", 0);
    for m in ["1", "L", "P", "LA", "RGB", "RGBA", "CMYK"] {
        let n = format!("gen/hopper_{m}.im");
        check(&n, labeye::decode, 0, &format!("{n}.rgba"), 0);
    }
    // Pillow converts YCbCr with its own fixed-point coefficients.
    check(
        "gen/hopper_YCbCr.im",
        labeye::decode,
        0,
        "gen/hopper_YCbCr.im.rgba",
        2,
    );
    check("gen/two.im", labeye::decode, 1, "gen/two.im.1.rgba", 0);
    check("bw_gradient.imt", imt::decode, 0, "bw_gradient.imt.rgba", 0);
    check("hopper.p7", xvthumb::decode, 0, "hopper.p7.rgba", 0);
    check("gbr.gbr", gimp::gbr::decode, 0, "gbr.gbr.rgba", 0);

    // GD: Pillow reads the palette's colours but not gd's alpha channel; compare colours.
    {
        let d = std::fs::read(root.join("hopper.gd")).unwrap();
        let ours = rgba8(&gd::image::decode(&d, 0).unwrap());
        let want = std::fs::read(root.join("hopper.gd.rgba")).unwrap();
        let strip =
            |v: &[u8]| -> Vec<u8> { v.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect() };
        let (n, m) = diff(&strip(&ours), &strip(&want));
        println!("hopper.gd: {n} colour bytes differ (max {m})");
        if m > 0 {
            bad.push("hopper.gd".into());
        }
    }

    // 16-bit IM: the stored values.
    for m in ["I_16", "I_16B"] {
        let n = format!("gen/hopper_{m}.im");
        let d = std::fs::read(root.join(&n)).unwrap();
        let Pixels::Gray16(v) = labeye::decode(&d, 0).unwrap().pixels else {
            panic!("{n}: not 16-bit")
        };
        let want = f64s(&root.join(format!("{n}.f64")));
        let miss = v
            .iter()
            .zip(&want)
            .filter(|(a, b)| f64::from(**a) != **b)
            .count();
        println!("{n}: {miss} of {} values differ", want.len());
        if miss > 0 {
            bad.push(n);
        }
    }
    // 32-bit IM: stretched from the oracle's values the same way.
    for m in ["I", "F"] {
        let n = format!("gen/hopper_{m}.im");
        let d = std::fs::read(root.join(&n)).unwrap();
        let Pixels::Gray16(v) = labeye::decode(&d, 0).unwrap().pixels else {
            panic!("{n}: not 16-bit")
        };
        let want = f64s(&root.join(format!("{n}.f64")));
        let (lo, hi) = want
            .iter()
            .fold((f64::MAX, f64::MIN), |(a, b), &x| (a.min(x), b.max(x)));
        let miss = v
            .iter()
            .zip(&want)
            .filter(|(a, b)| {
                let s = ((**b - lo) / (hi - lo) * 65535.0 + 0.5) as u16;
                **a != s
            })
            .count();
        println!("{n}: {miss} of {} values differ", want.len());
        if miss > 0 {
            bad.push(n);
        }
    }

    // McIDAS AREA and PGX: the stored values.
    {
        let n = "cmx3g8_wv_1998.260_0745_mcidas.ara";
        let d = std::fs::read(root.join(n)).unwrap();
        let (w, h, v, short) = mcidas::samples(&d, 0).unwrap();
        let want = f64s(&with(&root.join(n), ".f64"));
        let miss = v
            .iter()
            .zip(&want)
            .filter(|(a, b)| f64::from(**a) != **b)
            .count();
        println!("{n}: {w}×{h}, {miss} of {} values differ", want.len());
        if miss > 0 || short || v.len() != want.len() {
            bad.push(n.into());
        }
    }
    {
        let n = "c1p0_01_0.pgx";
        let d = std::fs::read(root.join(n)).unwrap();
        let (_, v, _) = pgx::samples(&d).unwrap();
        let want = f64s(&with(&root.join(n), ".f64"));
        let miss = v
            .iter()
            .zip(&want)
            .filter(|(a, b)| **a as f64 != **b)
            .count();
        println!("{n}: {miss} of {} values differ", want.len());
        if miss > 0 || v.len() != want.len() {
            bad.push(n.into());
        }
    }

    // Palettes and gradients.
    for n in ["full_gimp_palette.gpl", "custom_gimp_palette.gpl"] {
        let d = std::fs::read(root.join(n)).unwrap();
        let p = gpl::parse(&d).unwrap();
        let want = std::fs::read(with(&root.join(n), ".rgb")).unwrap();
        let ours: Vec<u8> = p
            .swatches
            .iter()
            .flat_map(|s| [s.rgba[0], s.rgba[1], s.rgba[2]])
            .collect();
        let k = ours.len().min(want.len());
        let (miss, m) = diff(&ours[..k], &want[..k]);
        println!(
            "{n}: {} colours, {miss} bytes differ (max {m})",
            p.swatches.len()
        );
        if m > 0 {
            bad.push(n.into());
        }
    }
    // Pillow's own two test gradients have overlapping segments that leave 0–1 (colours
    // above 1, segments out of order): Pillow extrapolates past a segment's ends and wraps
    // the bytes, we clamp. Their counts are reported, the well-formed one must match.
    for n in [
        "gimp_gradient.ggr",
        "gimp_gradient_with_name.ggr",
        "gen/blends.ggr",
    ] {
        let d = std::fs::read(root.join(n)).unwrap();
        let g = ggr::parse(&d).unwrap();
        let want = std::fs::read(with(&root.join(n), ".rgba256")).unwrap();
        let ours: Vec<u8> = (0..256)
            .flat_map(|i| {
                g.sample(f64::from(i) / 255.0)
                    .map(specular_legacy2::swatch::unit8)
            })
            .collect();
        let (miss, m) = diff(&ours, &want);
        println!("{n}: {miss} of 1024 bytes differ (max {m})");
        if m > 1 && n.starts_with("gen/") {
            bad.push(n.into());
        }
    }
    assert!(bad.is_empty(), "mismatches: {bad:?}");
}

/// Aseprite's test sprites, every frame, against aseprite-py's compositor (Normal blending
/// only: sprites with other blend modes are reported, not failed).
#[test]
#[ignore]
fn aseprite_matches_aseprite_py() {
    let Some(root) = std::env::var_os("SPECULAR_FMT500D_CORPUS") else {
        eprintln!("SPECULAR_FMT500D_CORPUS not set");
        return;
    };
    let root = Path::new(&root);
    let mut names: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .filter_map(|e| {
            let n = e.ok()?.file_name().into_string().ok()?;
            n.ends_with(".aseprite").then_some(n)
        })
        .collect();
    names.sort();
    assert!(!names.is_empty());
    let mut bad = Vec::new();
    for name in names {
        let data = std::fs::read(root.join(&name)).unwrap();
        let s = specular_legacy2::aseprite::parse(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
        let summary = std::fs::read_to_string(root.join(format!("gen/ase/{name}.txt"))).unwrap();
        let mut lines = summary.lines();
        let head: Vec<usize> = lines
            .next()
            .unwrap()
            .split(' ')
            .map(|v| v.parse().unwrap())
            .collect();
        let ours = [
            s.width as usize,
            s.height as usize,
            s.frames.len(),
            s.layers.len(),
            s.tags.len(),
        ];
        if head != ours {
            println!("{name}: structure {ours:?}, oracle {head:?}");
            bad.push(name.clone());
            continue;
        }
        let layers: Vec<String> = s
            .layers
            .iter()
            .map(|l| format!("layer {}|{}|{}", l.name, u8::from(l.visible()), l.level))
            .collect();
        let want_layers: Vec<&str> = summary
            .lines()
            .filter(|l| l.starts_with("layer "))
            .collect();
        if layers != want_layers {
            println!("{name}: layers {layers:?}, oracle {want_layers:?}");
            bad.push(name.clone());
        }
        let normal = s.layers.iter().all(|l| l.blend == 0);
        let (mut differ, mut max, mut total) = (0usize, 0u8, 0usize);
        for k in 0..s.frames.len() {
            let want = std::fs::read(root.join(format!("gen/ase/{name}.{k}.rgba"))).unwrap();
            let got = s.render(k);
            assert_eq!(got.len(), want.len(), "{name} frame {k}");
            for (a, b) in got.chunks_exact(4).zip(want.chunks_exact(4)) {
                // Fully transparent pixels: their colour does not matter.
                let (a, b) = if a[3] == 0 && b[3] == 0 {
                    ([0u8; 4], [0u8; 4])
                } else {
                    ([a[0], a[1], a[2], a[3]], [b[0], b[1], b[2], b[3]])
                };
                for c in 0..4 {
                    let d = a[c].abs_diff(b[c]);
                    differ += usize::from(d > 0);
                    max = max.max(d);
                }
            }
            total += want.len();
        }
        println!(
            "{name}: {}×{}, {} frames, {differ} of {total} bytes differ (max {max}){}",
            s.width,
            s.height,
            s.frames.len(),
            if normal { "" } else { ", other blend modes" }
        );
        if max > 1 && normal {
            bad.push(name);
        }
    }
    assert!(bad.is_empty(), "mismatches: {bad:?}");
}
